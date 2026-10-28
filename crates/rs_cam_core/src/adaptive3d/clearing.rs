//! 3D Z-level clearing engine for adaptive3d: region detection,
//! per-level contour-parallel and curvature-adaptive clearing,
//! stamping, and waterline cleanup.

use crate::dexel_stock::{StockCutDirection, TriDexelStock};
use crate::geo::{P2, P3};
use crate::geometry::contour_extract::marching_squares_bool_grid;
use crate::interrupt::{CancelCheck, Cancelled, check_cancel};
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::ops::waterline::waterline_contours_with_cancel;
use crate::stock::radial_profile::RadialProfileLUT;
use crate::surface::slope::{SlopeMap, SurfaceHeightmap};
use crate::tool::MillingCutter;
use crate::trace::debug_trace::{DebugSpanKind, ToolpathDebugContext};
use std::collections::VecDeque;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;
use tracing::debug;

use super::centre_clip::{CentreClip, clip_segment};
use super::path::{
    Adaptive3dSegment, RAPID_DESCENT_BUFFER_MM, StayDownProof, descent_start_z,
    drape_path_to_leave, drape_point,
};
use super::search::{blend_corners_3d, is_clear_path_3d};
use super::{
    Adaptive3dRuntimeEvent, ClearingStrategy3d, ZLevelPlanMetrics, stock_has_material_above,
    stock_top_z_at,
};
use crate::toolpath::simplify_path_3d;

// ── Constants ─────────────────────────────────────────────────────────

/// Minimum cell count of remaining material at a Z level for the planner
/// to bother running the clearing pass. Below this we treat the level as
/// "done" and move on.
///
/// Absolute count (not fraction) — at small `depth_per_pass` a thin
/// per-level slab contributes few cells even when there's a real island
/// to clear, so a fraction-based gate (the historical `remaining < 0.005`)
/// would skip it. Matches `min_cells = 4` in `detect_material_regions`.
pub(super) const MIN_CELLS_TO_CLEAR: u64 = 4;

/// The fallback for a non-finite or non-positive `trochoid_cap_mult`. The
/// slice's 2D planner (`PathStrategy2d::Agent`) does not read the cap; the
/// removed ContourSpiral strategy read it (removal step R4). The value is
/// the default of the load-only `Adaptive3dConfig::trochoid_cap_mult`.
const TROCHOID_CAP_MULT_3D: f64 = 1.6;

// ── Region detection ──────────────────────────────────────────────────

/// A connected region of material detected by flood fill on the heightmap.
pub(super) struct MaterialRegion {
    pub(super) row_min: usize,
    pub(super) row_max: usize,
    pub(super) col_min: usize,
    pub(super) col_max: usize,
    pub(super) cell_count: usize,
    pub(super) surface_z_min: f64,
    pub(super) surface_z_max: f64,
    /// The flood-fill id of the region in the label grid of
    /// [`detect_material_regions_labeled`].
    pub(super) bfs_label: usize,
}

/// Detect connected material regions via 8-connected BFS flood fill.
///
/// A cell "has material" if the top-Z of the dexel ray exceeds
/// `surface_z + stock_to_leave + 0.01`.
/// Regions with fewer than `min_cells` (default 4) are filtered out.
/// Returns regions sorted by cell_count descending (largest first).
/// The unit tests read this form; the planner reads
/// [`detect_material_regions_labeled`].
#[cfg(test)]
pub(super) fn detect_material_regions(
    material_stock: &TriDexelStock,
    surface_hm: &SurfaceHeightmap,
    stock_to_leave: f64,
) -> Vec<MaterialRegion> {
    detect_material_regions_labeled(material_stock, surface_hm, stock_to_leave).0
}

/// [`detect_material_regions`] and its flood-fill label grid (row-major,
/// one id per cell; `usize::MAX` is a cell with no material). Each region
/// carries its id in `bfs_label`. The By Area overlay reads the grid.
#[expect(
    clippy::indexing_slicing,
    reason = "bounded indexing in algorithmic code; every index is below rows * cols."
)]
pub(super) fn detect_material_regions_labeled(
    material_stock: &TriDexelStock,
    surface_hm: &SurfaceHeightmap,
    stock_to_leave: f64,
) -> (Vec<MaterialRegion>, Vec<usize>) {
    let rows = material_stock.z_grid.rows;
    let cols = material_stock.z_grid.cols;
    let min_cells = 4usize;

    // Label grid: 0 = unlabeled, usize::MAX = no-material
    let mut labels = vec![0usize; rows * cols];

    // Mark cells that have no material
    for row in 0..rows {
        for col in 0..cols {
            let surf_z = surface_hm.z_or_bbox_floor_at(row, col);
            let floor = surf_z + stock_to_leave + 0.01;
            if !stock_has_material_above(material_stock, row, col, floor) {
                labels[row * cols + col] = usize::MAX;
            }
        }
    }

    let mut regions = Vec::new();
    let mut region_id = 1usize;
    let mut queue = VecDeque::new();

    for start_row in 0..rows {
        for start_col in 0..cols {
            let idx = start_row * cols + start_col;
            if labels[idx] != 0 {
                continue; // Already labeled or no material
            }

            // BFS flood fill for this region
            let mut rmin = start_row;
            let mut rmax = start_row;
            let mut cmin = start_col;
            let mut cmax = start_col;
            let mut count = 0usize;
            let mut sz_min = f64::INFINITY;
            let mut sz_max = f64::NEG_INFINITY;

            labels[idx] = region_id;
            queue.push_back((start_row, start_col));

            while let Some((r, c)) = queue.pop_front() {
                count += 1;
                rmin = rmin.min(r);
                rmax = rmax.max(r);
                cmin = cmin.min(c);
                cmax = cmax.max(c);
                let sz = surface_hm.z_or_bbox_floor_at(r, c);
                sz_min = sz_min.min(sz);
                sz_max = sz_max.max(sz);

                // 8-connected neighbors
                for dr in [-1i32, 0, 1] {
                    for dc in [-1i32, 0, 1] {
                        if dr == 0 && dc == 0 {
                            continue;
                        }
                        let nr = r as i32 + dr;
                        let nc = c as i32 + dc;
                        if nr < 0 || nr >= rows as i32 || nc < 0 || nc >= cols as i32 {
                            continue;
                        }
                        let nr = nr as usize;
                        let nc = nc as usize;
                        let ni = nr * cols + nc;
                        if labels[ni] == 0 {
                            labels[ni] = region_id;
                            queue.push_back((nr, nc));
                        }
                    }
                }
            }

            if count >= min_cells {
                regions.push(MaterialRegion {
                    row_min: rmin,
                    row_max: rmax,
                    col_min: cmin,
                    col_max: cmax,
                    cell_count: count,
                    surface_z_min: sz_min,
                    surface_z_max: sz_max,
                    bfs_label: region_id,
                });
            }

            region_id += 1;
        }
    }

    // Sort largest first
    regions.sort_by_key(|a| std::cmp::Reverse(a.cell_count));
    (regions, labels)
}

/// The cells that one By Area job owns (PLAN §3.2).
///
/// `owned` holds one flag per planner cell (row-major, `rows * cols`). The
/// row/col bound encloses every owned cell, so a loop can skip the rest of
/// the grid. The mask confines a job to its cells, not to its bounding box:
/// a cell of another job inside the box is not owned.
pub(super) struct AreaMask {
    /// The job id: the 0-based index of the job in the planner order.
    pub(super) job: usize,
    pub(super) owned: Vec<bool>,
    pub(super) cols: usize,
    pub(super) row_min: usize,
    pub(super) row_max: usize,
    pub(super) col_min: usize,
    pub(super) col_max: usize,
    /// The number of owned cells.
    pub(super) owned_count: usize,
}

impl AreaMask {
    /// The mask of every cell whose flag in `owned` is true. `None` when
    /// no cell is owned.
    pub(super) fn from_owned(job: usize, owned: Vec<bool>, cols: usize) -> Option<Self> {
        let mut bound: Option<(usize, usize, usize, usize)> = None;
        let mut owned_count = 0usize;
        for (i, _) in owned.iter().enumerate().filter(|(_, o)| **o) {
            let (row, col) = (i / cols.max(1), i % cols.max(1));
            owned_count += 1;
            bound = Some(match bound {
                None => (row, row, col, col),
                Some((r0, r1, c0, c1)) => (r0.min(row), r1.max(row), c0.min(col), c1.max(col)),
            });
        }
        let (row_min, row_max, col_min, col_max) = bound?;
        Some(Self {
            job,
            owned,
            cols,
            row_min,
            row_max,
            col_min,
            col_max,
            owned_count,
        })
    }

    /// True when the job owns the cell at `(row, col)`.
    pub(super) fn owns(&self, row: usize, col: usize) -> bool {
        self.owned
            .get(row * self.cols + col)
            .copied()
            .unwrap_or(false)
    }

    /// The number of owned cells that have material above the effective
    /// floor at `z_level`. This is the count of `true` cells in the grid of
    /// [`build_material_bool_grid`] with this mask: the level gate reads
    /// the same test that the rings read (PLAN §3.4).
    pub(super) fn material_cells(
        &self,
        material_stock: &TriDexelStock,
        surface_hm: &SurfaceHeightmap,
        z_level: f64,
        stock_to_leave: f64,
    ) -> u64 {
        let mut count = 0u64;
        for row in self.row_min..=self.row_max {
            for col in self.col_min..=self.col_max {
                if self.owns(row, col)
                    && cell_has_material(
                        material_stock,
                        surface_hm,
                        row,
                        col,
                        z_level,
                        stock_to_leave,
                    )
                {
                    count += 1;
                }
            }
        }
        count
    }
}

/// The per-cell material test of the level grid: material stands above
/// `max(surface + stock_to_leave, z_level)`.
fn cell_has_material(
    material_stock: &TriDexelStock,
    surface_hm: &SurfaceHeightmap,
    row: usize,
    col: usize,
    z_level: f64,
    stock_to_leave: f64,
) -> bool {
    let surf_z = surface_hm.z_or_bbox_floor_at(row, col);
    let effective_floor = super::intended_floor::level_floor_z(surf_z, stock_to_leave, z_level);
    stock_has_material_above(material_stock, row, col, effective_floor + 0.01)
}

/// The level gate: the number of cells with material to cut at
/// `z_level`, and the fraction for the debug counters.
///
/// - With a mask, the count is [`AreaMask::material_cells`]: every owned
///   cell with material above its effective floor. A job of high ground or
///   a job whose floor is between two levels still has cells to cut.
/// - Without a mask (Global), the count is the same test on every cell of
///   the grid. A level below every floor still drapes onto the floors, so
///   the gate must not ask for a floor at or below the level.
fn level_gate(
    material_stock: &TriDexelStock,
    surface_hm: &SurfaceHeightmap,
    z_level: f64,
    stock_to_leave: f64,
    mask: Option<&AreaMask>,
) -> (u64, f64) {
    match mask {
        Some(m) => {
            let cells = m.material_cells(material_stock, surface_hm, z_level, stock_to_leave);
            (cells, cells as f64 / m.owned_count.max(1) as f64)
        }
        None => {
            let grid = &material_stock.z_grid;
            let mut cells = 0u64;
            for row in 0..grid.rows {
                for col in 0..grid.cols {
                    if cell_has_material(
                        material_stock,
                        surface_hm,
                        row,
                        col,
                        z_level,
                        stock_to_leave,
                    ) {
                        cells += 1;
                    }
                }
            }
            (cells, cells as f64 / (grid.rows * grid.cols).max(1) as f64)
        }
    }
}

// ── Z-level clearing helper ──────────────────────────────────────────

/// Parameters for a single Z-level clearing pass, extracted to avoid
/// threading dozens of locals through the helper.
pub(super) struct ClearZLevelContext<'a> {
    pub(super) mesh: &'a TriangleMesh,
    pub(super) index: &'a SpatialIndex,
    pub(super) cutter: &'a dyn MillingCutter,
    pub(super) lut: &'a RadialProfileLUT,
    pub(super) debug: Option<ToolpathDebugContext>,
    pub(super) tool_radius: f64,
    pub(super) stepover: f64,
    pub(super) stock_to_leave: f64,
    pub(super) depth_per_pass: f64,
    pub(super) tolerance: f64,
    /// The RDP tolerance of a cut, for the stamp
    /// (`Adaptive3dGeometry::cut_simplify_tolerance`, G-PLANSIMGAP). Every
    /// `push_segment_with_stamp` reads this, never `tolerance`.
    pub(super) cut_tolerance: f64,
    /// Op cutting feed (mm/min) — used for the feed-vs-rapid air-run
    /// crossover, not for emission (segments carry no feeds here).
    pub(super) feed_rate: f64,
    /// Op plunge feed (mm/min) — same crossover use.
    pub(super) plunge_rate: f64,
    pub(super) step_len: f64,
    pub(super) max_link_dist: f64,
    /// Safe-Z used by `segments_to_toolpath` for retracts and the start
    /// of peck-plunges in `Adaptive3dSegment::Rapid`. Plumbed into the
    /// clearing layer so the planner can mirror those plunge stamps in
    /// its internal dexel state (parity with the simulator replay).
    pub(super) safe_z: f64,
    pub(super) clearing_strategy: ClearingStrategy3d,
    /// Load-only: passed to the slice's 2D params, which do not read it
    /// (`PathStrategy2d::Agent`).
    pub(super) trochoid_cap_mult: f64,
    /// Load-only: passed to the slice's 2D params, which do not read it
    /// (`KeepDownLinks::WithinPassLoad`, `adaptive::path::pass_load_for`).
    pub(super) engagement_measure: crate::adaptive::EngagementMeasure,
    /// Minimum corner radius for `blend_corners_3d` — needed inside
    /// `stamp_emitted_segment` so the planner stamps the SAME path the
    /// simulator will replay (segments_to_toolpath blends Cut paths
    /// before emitting feeds).
    pub(super) min_cutting_radius: f64,
    /// F-038: minimum forecast horizontal cutting length (mm) a marching-
    /// squares region must produce in its 2D adaptive sub-pass before the
    /// slice commits an entry plunge to it. Set to 0.0 to
    /// disable. See `clear_z_level_slice` for the apply site.
    pub(super) min_region_cut_length_mm: f64,
    /// The disc radius of an entry's rapid floor read; see
    /// `path.rs::entry_floor_radius`.
    pub(super) entry_floor_radius: f64,
    /// The resolved stay-down distance (`path.rs::resolved_stay_down_mm`).
    /// The planner writes a keep-down stock proof only inside it.
    pub(super) stay_down_mm: f64,
    /// The boundary clip's containment (G-BOUNDARYPHANTOM); see
    /// [`StampDrape::centre_clip`].
    pub(super) centre_clip: Option<&'a CentreClip>,
    /// The stay-down clearance (`params.linking.stay_down_clearance_mm`,
    /// at least 0): the dexel allowance of the step split and the routed
    /// keep-down travel.
    pub(super) stay_down_clearance_mm: f64,
    /// The step split in force ([`super::StepSplit`];
    /// `super::effective_step_split`).
    pub(super) step_split: super::StepSplit,
    /// The routed keep-down travel in force ([`super::KeepDownRoute`];
    /// `super::effective_keep_down_route`).
    pub(super) keep_down_route: super::KeepDownRoute,
    /// Geodesic Adaptive (research C38 §11, G-CLIPAIR): the boundary mask
    /// of the region grid, when the boundary clear kept the reachable band.
    pub(super) region_clip: Option<super::outer_band::RegionClip>,
    /// Geodesic Adaptive (research C38 §16): the pieces the arm left to the
    /// rest pass (the field does not apply, or the F-038 forecast skipped
    /// them), each with its Z level. The waterline cleanup of that level
    /// skips a contour inside one.
    pub(super) left_pieces: std::cell::RefCell<Vec<(f64, crate::polygon::Polygon2)>>,
    /// C4 (`Adaptive3dLinking::level_contour`): the reach band and its
    /// contour on each level. `None`: the option is off, or the rough has
    /// no band.
    pub(super) band_edge: std::cell::RefCell<Option<super::level_contour::BandEdge>>,
}

// ── Contour-parallel clearing ─────────────────────────────────────────

/// Build a padded boolean grid of material cells at a given Z level.
///
/// A cell is `true` if the stock has material above the effective floor
/// (max of surface_z + stock_to_leave, z_level). The grid is padded with
/// a 1-cell false border so marching squares and EDT detect edge boundaries.
/// With a mask, only the owned cells can be `true`.
///
/// Returns `(padded_grid, padded_rows, padded_cols, origin_x, origin_y, cell_size)`.
#[expect(
    clippy::indexing_slicing,
    reason = "padded grid indices bounded by loop ranges"
)]
fn build_material_bool_grid(
    material_stock: &TriDexelStock,
    surface_hm: &SurfaceHeightmap,
    z_level: f64,
    stock_to_leave: f64,
    mask: Option<&AreaMask>,
) -> (Vec<bool>, usize, usize, f64, f64, f64) {
    let grid = &material_stock.z_grid;
    let rows = grid.rows;
    let cols = grid.cols;
    let cell_size = grid.cell_size;
    let origin_u = grid.origin_u;
    let origin_v = grid.origin_v;

    // Build padded boolean grid (1-cell false border so marching squares
    // detects edge boundaries).
    let padded_rows = rows + 2;
    let padded_cols = cols + 2;
    let mut padded_grid = vec![false; padded_rows * padded_cols];

    for row in 0..rows {
        for col in 0..cols {
            // Skip the cells that the job does not own.
            if let Some(m) = mask
                && !m.owns(row, col)
            {
                continue;
            }

            if cell_has_material(
                material_stock,
                surface_hm,
                row,
                col,
                z_level,
                stock_to_leave,
            ) {
                // +1 offset for the border padding
                padded_grid[(row + 1) * padded_cols + (col + 1)] = true;
            }
        }
    }

    (
        padded_grid,
        padded_rows,
        padded_cols,
        origin_u - cell_size,
        origin_v - cell_size,
        cell_size,
    )
}

/// Stamp dexel stock along a 3D cutting path with **swept** segment
/// stamps between consecutive points.
///
/// Mirrors how `simulate_toolpath_with_metrics_with_cancel` replays a
/// `Cut` — each consecutive pair becomes a `stamp_linear_segment` (a
/// stadium-swept stamp). Earlier this function point-stamped at each
/// path point with `stamp_tool_at`, which over-removes on sloped Cut
/// segments where the deeper-Z cylinder dominates the cylinder union
/// at midpoints (Bug 1 in the planner-↔-simulator parity work).
fn stamp_along_path(
    material_stock: &mut TriDexelStock,
    lut: &RadialProfileLUT,
    tool_radius: f64,
    path: &[P3],
) {
    if path.len() < 2 {
        if let Some(p) = path.first() {
            material_stock.stamp_tool_at(
                lut,
                tool_radius,
                p.x,
                p.y,
                p.z,
                StockCutDirection::FromTop,
            );
        }
        return;
    }
    for pair in path.windows(2) {
        if let [a, b] = pair {
            material_stock.stamp_linear_segment(
                lut,
                tool_radius,
                *a,
                *b,
                StockCutDirection::FromTop,
            );
        }
    }
}

/// The mesh geometry `segments_to_toolpath` needs in order to drape an
/// emitted move up to `surface + stock_to_leave` (the `fad3a56` gouge
/// guard). Carried into the planner's mirror stamp so both sides see the
/// same path.
///
/// Every field is already on [`ClearZLevelContext`]; the struct exists
/// only so the mirror can be handed the same four values from
/// `waterline_cleanup`, which has no context.
pub(super) struct StampDrape<'a> {
    pub(super) mesh: &'a TriangleMesh,
    pub(super) index: &'a SpatialIndex,
    pub(super) cutter: &'a dyn MillingCutter,
    pub(super) stock_to_leave: f64,
    /// The disc radius of the entry rapid floor read (`plan_entry`).
    pub(super) entry_floor_radius: f64,
    /// The stay-down distance inside which `plan_entry` writes a keep-down
    /// stock proof. `0.0` writes none.
    pub(super) stay_down_mm: f64,
    /// The containment of the session's post-generation boundary clip, or
    /// `None` with no boundary. Every pushed segment passes it first, so the
    /// planner stamps only what the clip keeps (G-BOUNDARYPHANTOM).
    pub(super) centre_clip: Option<&'a CentreClip>,
    /// The routed keep-down travel at an entry ([`routed_entry`]), and the
    /// stay-down clearance it reads. `Off` for the waterline.
    pub(super) keep_down_route: super::KeepDownRoute,
    pub(super) stay_down_clearance_mm: f64,
}

impl<'a> ClearZLevelContext<'a> {
    pub(super) fn stamp_drape(&self) -> StampDrape<'_> {
        StampDrape {
            mesh: self.mesh,
            index: self.index,
            cutter: self.cutter,
            stock_to_leave: self.stock_to_leave,
            entry_floor_radius: self.entry_floor_radius,
            stay_down_mm: self.stay_down_mm,
            centre_clip: self.centre_clip,
            keep_down_route: self.keep_down_route,
            stay_down_clearance_mm: self.stay_down_clearance_mm,
        }
    }
}

/// Where the planner stands between two segments.
///
/// G-PHANTOMSTAMP: two positions, because the emitter moves the tool to a
/// point the raw segment does not name. `last_pos` is the raw end of the
/// last stamped segment: the keep-down proof's `from`, the link gates and
/// the region order read it, as before. `tool_pos` is where
/// `segments_to_toolpath` leaves the tool after that same segment (a draped
/// entry, the last blended cut point, the draped link end); the `Cut` and
/// `Link` mirrors stamp their first feed from it, as the emitter feeds it.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct PlannerCursor {
    pub(super) last_pos: Option<P3>,
    pub(super) tool_pos: Option<P3>,
    /// G-BOUNDARYPHANTOM: the boundary clip dropped a segment (or a cut's
    /// tail) since the last pushed one, so the tool is not where the next
    /// segment starts and its first piece needs an entry
    /// (`centre_clip::clip_segment`).
    pub(super) clip_gap: bool,
}

/// Mirror in the planner's `material_stock` the swept-tube stamps that
/// the simulator will produce when it replays the toolpath emitted by
/// `segments_to_toolpath` for `segment`, and return where the emitter
/// leaves the tool after it (`None`: the emitter emits no move for it).
///
/// `tool_pos` is where the emitter's tool stands BEFORE `segment`
/// ([`PlannerCursor::tool_pos`]): a `Link` and a `Cut` feed from it.
///
/// `safe_z` / `tolerance` / `min_cutting_radius` match
/// `Adaptive3dParams`. The invariant this function exists to hold is
/// that it applies **every** transformation `segments_to_toolpath`
/// applies, so the planner stamps the SAME path the simulator will
/// replay. Today that is three transformations, in the emitter's order:
///
/// 1. `drape_path_to_leave` / `drape_point` — the `fad3a56` gouge guard,
///    which densifies to `<= cutter.radius()` and raises every point to
///    `drop_cutter(x, y) + stock_to_leave`;
/// 2. `simplify_path_3d` (RDP at `tolerance`, which callers pass as
///    `Adaptive3dGeometry::cut_simplify_tolerance`: the segment merge
///    happens here, not after the planner — G-PLANSIMGAP);
/// 3. `blend_corners_3d` (at `min_cutting_radius`).
///
/// If a fourth is ever added to the emitter it must be added here in the
/// same commit. `fad3a56` added (1) to the emitter and not here, and the
/// planner spent seven weeks believing it had removed material its own
/// emitted toolpath leaves standing — see
/// `planning/review_2026-08-04/ADAPTIVE3D_RED_BASELINE.md` §3.
/// The path the emitter ships for a cut `path`: `drape_path_to_leave`,
/// then `simplify_path_3d` at `tolerance`, then `blend_corners_3d` at
/// `min_cutting_radius` (the order of [`stamp_emitted_segment`]'s list).
/// C4's band state is lowered along it, as the stamp is.
pub(super) fn shipped_cut_path(
    path: &[P3],
    tolerance: f64,
    min_cutting_radius: f64,
    drape: &StampDrape<'_>,
) -> Vec<P3> {
    let draped = drape_path_to_leave(
        path,
        drape.mesh,
        drape.index,
        drape.cutter,
        drape.stock_to_leave,
        drape.cutter.radius(),
    );
    let simplified = simplify_path_3d(&draped, tolerance);
    blend_corners_3d(&simplified, min_cutting_radius)
}

#[expect(
    clippy::too_many_arguments,
    reason = "each parameter is a separate input that the callers supply"
)]
fn stamp_emitted_segment(
    material_stock: &mut TriDexelStock,
    lut: &RadialProfileLUT,
    tool_radius: f64,
    tool_pos: Option<P3>,
    segment: &Adaptive3dSegment,
    safe_z: f64,
    tolerance: f64,
    min_cutting_radius: f64,
    drape: &StampDrape<'_>,
) -> Option<P3> {
    match segment {
        Adaptive3dSegment::Cut(path) => {
            // G-PHANTOMSTAMP: the emitter skips a cut of fewer than two
            // points (`if path.len() < 2 { continue; }`), so its mirror
            // stamps nothing and the tool does not move.
            if path.len() < 2 {
                return None;
            }
            // Mirror segments_to_toolpath's path transformation —
            // drape_path_to_leave, then simplify_path_3d (RDP), then
            // blend_corners_3d — so the planner's swept stamps cover
            // the SAME tubes the simulator will stamp from the emitted
            // feeds. Order matters: the emitter drapes BEFORE
            // simplifying, so the RDP sees the densified, lifted path.
            let blended = shipped_cut_path(path, tolerance, min_cutting_radius, drape);
            // G-PHANTOMSTAMP: the emitter feeds `blended.iter().skip(1)`
            // from where the tool stands, not from `blended[0]`. Stamp the
            // same polyline: the tool position, then `blended[1..]`. With
            // no tool position (nothing emitted yet) `blended[0]` stands in.
            let mut fed: Vec<P3> = Vec::with_capacity(blended.len());
            fed.extend(tool_pos.or_else(|| blended.first().copied()));
            fed.extend(blended.iter().skip(1).copied());
            stamp_along_path(material_stock, lut, tool_radius, &fed);
            blended.last().copied()
        }
        Adaptive3dSegment::Rapid(entry) => {
            // Toolpath: rapid lift to safe_z, rapid XY at safe_z,
            // peck-plunge from safe_z down to entry.z. Only the
            // peck-plunge feeds get stamped — and the net swept-tube
            // of the interleaved peck/retract feeds is just the full
            // vertical descent from safe_z to entry.
            //
            // The emitter shadow-rebinds `entry` through `drape_point`
            // on this arm before it plunges, so the descent stops at the
            // draped Z, not the raw one. Mirror that. The `RapidWithFloor`
            // arm below gets the same rebind, as in the emitter.
            let entry = drape_point(
                entry,
                drape.mesh,
                drape.index,
                drape.cutter,
                drape.stock_to_leave,
            );
            let start = P3::new(entry.x, entry.y, safe_z);
            material_stock.stamp_linear_segment(
                lut,
                tool_radius,
                start,
                entry,
                StockCutDirection::FromTop,
            );
            Some(emitted_entry_end(entry, safe_z))
        }
        Adaptive3dSegment::RapidWithFloor {
            entry,
            rapid_floor_z,
            ..
        } => {
            // Toolpath: rapid descent from safe_z (or a keep-down link) down
            // to ~rapid_floor_z (cleared air, no stamp), then the style entry
            // from there to entry. `descent_start_z` is the emitter's own
            // descent floor, so we don't stamp BELOW entry.z
            // (which would happen if rapid_floor_z < entry.z, e.g. previous
            // pass already cut DEEPER than this entry — clearing function
            // sampled the post-stamp top).
            //
            // The emitter drapes this entry as it drapes a plain `Rapid`
            // (the `fad3a56` gouge guard); mirror that too.
            let entry = &drape_point(
                entry,
                drape.mesh,
                drape.index,
                drape.cutter,
                drape.stock_to_leave,
            );
            let descent_floor = descent_start_z(Some(*rapid_floor_z), safe_z, entry.z);
            let start = P3::new(entry.x, entry.y, descent_floor);
            material_stock.stamp_linear_segment(
                lut,
                tool_radius,
                start,
                *entry,
                StockCutDirection::FromTop,
            );
            Some(emitted_entry_end(*entry, descent_floor))
        }
        Adaptive3dSegment::Link(target) => {
            // Toolpath: feed from the tool position to target, draped to
            // hold the leave as the emitter drapes it. G-PHANTOMSTAMP: the
            // emitter's `from` is its tool position (`tp.moves.last()`),
            // falling back to the target itself when nothing was emitted.
            let from = tool_pos.unwrap_or(*target);
            let draped = drape_path_to_leave(
                &[from, *target],
                drape.mesh,
                drape.index,
                drape.cutter,
                drape.stock_to_leave,
                drape.cutter.radius(),
            );
            stamp_along_path(material_stock, lut, tool_radius, &draped);
            draped.last().copied()
        }
        Adaptive3dSegment::Marker(_) => None,
    }
}

/// Where `segments_to_toolpath` leaves the tool after an entry: at the
/// draped `entry` when the style emitter ran (`descent_floor > entry.z +
/// 1e-6`: the peck, helix and ramp all end with a feed to `entry`), else at
/// the descent floor over the entry XY, where the rapid (or the keep-down
/// link) stopped.
fn emitted_entry_end(entry: P3, descent_floor: f64) -> P3 {
    if descent_floor > entry.z + 1e-6 {
        entry
    } else {
        P3::new(entry.x, entry.y, descent_floor)
    }
}

/// Read an entry's rapid floor and its keep-down proof from the planner
/// stock, BEFORE the entry is stamped.
///
/// - The floor is the conservative stock top over a disc of
///   `drape.entry_floor_radius` at the entry XY: the highest the material
///   can stand anywhere under the tool (and under the helix turns). Above it
///   the emitter rapids; below it the entry style emitter feeds. This is the
///   read the descent optimiser uses (A/M10), so a rib narrower than one cell
///   cannot hide from it.
/// - The proof ([`StayDownProof`]) is the conservative stock top on the
///   swept tool disc from the previous planner position to the entry. It is
///   written only inside the stay-down distance.
///
/// With no stock under the disc (off the grid), the entry stays a plain
/// `Rapid`: the emitter then feeds from safe Z and takes no keep-down link.
fn plan_entry(
    material_stock: &TriDexelStock,
    tool_radius: f64,
    last_pos: Option<P3>,
    entry: P3,
    drape: &StampDrape<'_>,
) -> Adaptive3dSegment {
    let Some(rapid_floor_z) =
        material_stock.max_conservative_top_z_in_disc(entry.x, entry.y, drape.entry_floor_radius)
    else {
        return Adaptive3dSegment::Rapid(entry);
    };
    let stay_down = last_pos.and_then(|from| {
        let dx = entry.x - from.x;
        let dy = entry.y - from.y;
        let dist = (dx * dx + dy * dy).sqrt();
        if drape.stay_down_mm <= 0.0 || dist > drape.stay_down_mm {
            return None;
        }
        Some(StayDownProof {
            from,
            ceiling_z: super::keep_down::corridor_ceiling_z(
                material_stock,
                from,
                entry,
                tool_radius,
            ),
        })
    });
    // The helix or ramp starts above the cell-centre read; the rapid floor
    // keeps the sliver-safe one.
    let contact_z = material_stock
        .max_top_z_in_disc(entry.x, entry.y, drape.entry_floor_radius)
        .map_or(rapid_floor_z, |z| z.min(rapid_floor_z));
    Adaptive3dSegment::RapidWithFloor {
        entry,
        rapid_floor_z,
        contact_z,
        stay_down,
    }
}

/// The planner's link gate: may the tool feed from `lp` to `first` at
/// depth instead of an entry? The distance cap is
/// `path.rs::default_max_link_dist`; `is_clear_path_3d` admits a bite of one
/// Depth/Pass (a side entry cuts like a cut); a link that goes down must go
/// through air (`link_footprint_ok`).
fn side_link_ok(
    ctx: &ClearZLevelContext<'_>,
    material_stock: &TriDexelStock,
    surface_hm: &SurfaceHeightmap,
    lp: P3,
    first: P3,
) -> bool {
    let dx = first.x - lp.x;
    let dy = first.y - lp.y;
    (dx * dx + dy * dy).sqrt() < ctx.max_link_dist
        && is_clear_path_3d(
            material_stock,
            surface_hm,
            lp,
            first,
            ctx.stock_to_leave,
            ctx.depth_per_pass,
        )
        && link_footprint_ok(
            material_stock,
            lp,
            first,
            ctx.tool_radius,
            ctx.depth_per_pass,
        )
}

/// The footprint rule of a planner `Link`.
///
/// `is_clear_path_3d` reads the tool CENTRE line only. This test reads the
/// whole tool footprint (cell centres inside `radius` minus one cell, so the
/// wall that the tool just cut beside is not read as a bite) along the link:
///
/// - A level link (drop within the dexel noise) may bite at most `max_bite`
///   (one Depth/Pass) above the link line: a side entry cuts like a cut.
/// - A link that goes down may bite nothing: a steep link into stock is a
///   plunge at the cutting feed.
fn link_footprint_ok(stock: &TriDexelStock, from: P3, to: P3, radius: f64, max_bite: f64) -> bool {
    let bite = if to.z >= from.z - LINK_DESCENT_NOISE_MM {
        max_bite.max(0.0) + LINK_DESCENT_NOISE_MM
    } else {
        LINK_DESCENT_NOISE_MM
    };
    let cs = stock.z_grid.cell_size;
    let probe = (radius - cs).max(cs);
    let dx = to.x - from.x;
    let dy = to.y - from.y;
    let n = ((dx * dx + dy * dy).sqrt() / (cs * 0.5)).ceil().max(1.0) as usize;
    (0..=n).all(|k| {
        let t = k as f64 / n as f64;
        let z = from.z + (to.z - from.z) * t;
        stock
            .max_top_z_in_disc(from.x + dx * t, from.y + dy * t, probe)
            .is_none_or(|top| top <= z + bite)
    })
}

/// Dexel noise on a link height reading (the value
/// `search.rs::is_clear_path_3d` uses).
const LINK_DESCENT_NOISE_MM: f64 = 0.5;

/// [`super::StepSplit::StockDepth`]: the deepest axial engagement (mm) of a
/// feed move from `p` to `q` with a tool of `radius`: at each sample along
/// the move (every half cell, as `keep_down::corridor_ceiling_z`), the
/// conservative stock top over the disc less the tool's Z there.
/// `NEG_INFINITY` when the move is off the grid.
pub(super) fn step_axial_depth(stock: &TriDexelStock, p: P3, q: P3, radius: f64) -> f64 {
    let dx = q.x - p.x;
    let dy = q.y - p.y;
    let len = (dx * dx + dy * dy).sqrt();
    let step = (stock.z_grid.cell_size * 0.5).max(1e-3);
    let n = (len / step).ceil().max(1.0) as usize;
    (0..=n)
        .filter_map(|k| {
            let t = k as f64 / n as f64;
            let z = p.z + t * (q.z - p.z);
            stock
                .max_conservative_top_z_in_disc(p.x + dx * t, p.y + dy * t, radius)
                .map(|top| top - z)
        })
        .fold(f64::NEG_INFINITY, f64::max)
}

/// D65: whether a draped step that the stock-read split would split is cut
/// instead at a capped feed ([`capped_step_factor`]; the slice runs only
/// Geodesic Adaptive): a step that is not `deep` (its axial engagement at most one
/// Depth/Pass plus the clearance), whose chip-area load is over the ceiling
/// (the split's own reason) and at most the full slot
/// ([`crate::adaptive::path::FULL_SLOT_LOAD`]), inside one engaged run
/// (`in_engaged_run`: no engagement change at the step). Every other split
/// stays a split.
pub(super) fn admits_capped_split(
    deep: bool,
    load: f64,
    ceiling: f64,
    in_engaged_run: bool,
) -> bool {
    !deep && load > ceiling && load <= crate::adaptive::path::FULL_SLOT_LOAD && in_engaged_run
}

/// D65: the feed factor of a draped step admitted as a capped cut (Geodesic
/// Adaptive): the load factor `c / L` (`geodesic::feed_caps`'s law), at
/// most the factor that holds the step's descent rate at the plunge rate
/// (`plunge_rate · |step| / (|dz| · F)`, so a steep step goes down no
/// faster than the plunge it replaces), and at most the 2D cap `outer` of
/// the cut it lies on. Never above 1.
pub(super) fn capped_step_factor(
    ceiling: f64,
    load: f64,
    prev: P3,
    curr: P3,
    feed_rate: f64,
    plunge_rate: f64,
    outer: Option<f64>,
) -> f64 {
    let mut factor = if load > 0.0 { ceiling / load } else { 1.0 };
    let dz = curr.z - prev.z;
    if dz < 0.0 && feed_rate > 0.0 {
        let len = ((curr.x - prev.x).powi(2) + (curr.y - prev.y).powi(2) + dz * dz).sqrt();
        factor = factor.min(plunge_rate * len / (-dz * feed_rate));
    }
    if let Some(outer) = outer {
        factor = factor.min(outer);
    }
    factor.min(1.0)
}

/// The cutter and the pass a step's load is read against: the cutter's
/// radial profile, its chip cross-section contract, its diameter, and the
/// op's Depth/Pass.
pub(super) struct StepLoadCutter<'a> {
    pub(super) lut: &'a RadialProfileLUT,
    pub(super) chip: crate::feeds::ToolGeometryHint,
    pub(super) diameter: f64,
    pub(super) depth_per_pass: f64,
}

/// [`super::StepSplit::StockDepth`]: the highest chip-area load of a feed
/// move from `p` to `q` with a tool of `radius`, read on the 3D planner stock
/// by the one load ([`crate::tool_load::load_fraction::chip_load_fraction`]).
/// At each sample along the move (every half cell, as [`step_axial_depth`]),
/// each grid cell inside the disc whose stock stands above the cutter there
/// (its tip plus the profile height at the cell, plus `clearance`, the
/// dexel allowance) engages its depth above the cutter; the cells are binned
/// in lateral strips one cell apart across the move's XY heading, the
/// deepest per strip. A cell an earlier sample of this move passed within R
/// counts at most at that sample's Z (an earlier sample cut it down to its
/// own Z, and no lower): a descent sweeps nothing below its own path, so the
/// stock it descends into still counts. 0 for a move with no XY length (its
/// depth is [`step_axial_depth`]'s).
pub(super) fn step_load(
    stock: &TriDexelStock,
    p: P3,
    q: P3,
    radius: f64,
    clearance: f64,
    cutter: &StepLoadCutter<'_>,
) -> f64 {
    step_load_on(
        stock,
        &|row, col| super::stock_top_z_at(stock, row, col),
        p,
        q,
        radius,
        clearance,
        cutter,
    )
}

/// [`step_load`] with the column top of each cell read by `top_at` in place
/// of the planner stock. C4's contour on each level reads the planner stock
/// together with the reach band, which the boundary clear took out of it.
pub(super) fn step_load_on(
    stock: &TriDexelStock,
    top_at: &dyn Fn(usize, usize) -> f64,
    p: P3,
    q: P3,
    radius: f64,
    clearance: f64,
    cutter: &StepLoadCutter<'_>,
) -> f64 {
    let grid = &stock.z_grid;
    let cs = grid.cell_size;
    let (dx, dy) = (q.x - p.x, q.y - p.y);
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1e-9 || cs <= 0.0 {
        return 0.0;
    }
    let (ux, uy) = (dx / len, dy / len);
    let (nx, ny) = (-uy, ux);
    let n = (len / (cs * 0.5).max(1e-3)).ceil().max(1.0) as usize;
    let r_sq = radius * radius;
    let r_cells = (radius / cs).ceil() as isize;
    let mut highest = 0.0_f64;
    for k in 0..=n {
        let t = k as f64 / n as f64;
        let (x, y, z) = (p.x + dx * t, p.y + dy * t, p.z + t * (q.z - p.z));
        let col0 = ((x - grid.origin_u) / cs).round() as isize;
        let row0 = ((y - grid.origin_v) / cs).round() as isize;
        let mut strips: std::collections::BTreeMap<i64, f64> = std::collections::BTreeMap::new();
        for row in (row0 - r_cells)..=(row0 + r_cells) {
            for col in (col0 - r_cells)..=(col0 + r_cells) {
                if row < 0 || col < 0 || row as usize >= grid.rows || col as usize >= grid.cols {
                    continue;
                }
                let (cx, cy) = (
                    grid.origin_u + col as f64 * cs,
                    grid.origin_v + row as f64 * cs,
                );
                let (ex, ey) = (cx - x, cy - y);
                let d_sq = ex * ex + ey * ey;
                let Some(h) = cutter.lut.height_at_dist_sq(d_sq) else {
                    continue;
                };
                if d_sq > r_sq {
                    continue;
                }
                let mut top = top_at(row as usize, col as usize);
                for j in 0..k {
                    let tj = j as f64 / n as f64;
                    let (jx, jy) = (p.x + dx * tj, p.y + dy * tj);
                    if (cx - jx).powi(2) + (cy - jy).powi(2) <= r_sq {
                        top = top.min(p.z + tj * (q.z - p.z));
                    }
                }
                let floor = z + h;
                if top <= floor + clearance {
                    continue;
                }
                let strip = ((ex * nx + ey * ny) / cs).round() as i64;
                let deepest = strips.entry(strip).or_insert(0.0);
                *deepest = deepest.max(top - floor);
            }
        }
        let load = crate::tool_load::load_fraction::chip_load_fraction(
            cutter.chip,
            cutter.depth_per_pass,
            cutter.diameter,
            strips,
            cs,
        );
        highest = highest.max(load);
    }
    highest
}

/// Helper to push a segment and stamp its simulator-equivalent swept
/// material removal in one call.
///
/// With a boundary the segment first passes the clip
/// (`centre_clip::clip_segment`): the planner pushes and stamps only the
/// pieces the session's boundary clip keeps (G-BOUNDARYPHANTOM).
#[expect(
    clippy::too_many_arguments,
    reason = "each parameter is a separate input that the callers supply"
)]
pub(super) fn push_segment_with_stamp(
    segments: &mut Vec<Adaptive3dSegment>,
    material_stock: &mut TriDexelStock,
    lut: &RadialProfileLUT,
    tool_radius: f64,
    cursor: &mut PlannerCursor,
    segment: Adaptive3dSegment,
    safe_z: f64,
    tolerance: f64,
    min_cutting_radius: f64,
    drape: &StampDrape<'_>,
) {
    let Some(clip) = drape.centre_clip else {
        push_segment_with_stamp_unclipped(
            segments,
            material_stock,
            lut,
            tool_radius,
            cursor,
            segment,
            safe_z,
            tolerance,
            min_cutting_radius,
            drape,
        );
        return;
    };
    for piece in clip_segment(clip, cursor, segment) {
        push_segment_with_stamp_unclipped(
            segments,
            material_stock,
            lut,
            tool_radius,
            cursor,
            piece,
            safe_z,
            tolerance,
            min_cutting_radius,
            drape,
        );
    }
}

/// [`push_segment_with_stamp`] for a segment that has passed the clip.
#[expect(
    clippy::too_many_arguments,
    reason = "each parameter is a separate input that the callers supply"
)]
fn push_segment_with_stamp_unclipped(
    segments: &mut Vec<Adaptive3dSegment>,
    material_stock: &mut TriDexelStock,
    lut: &RadialProfileLUT,
    tool_radius: f64,
    cursor: &mut PlannerCursor,
    segment: Adaptive3dSegment,
    safe_z: f64,
    tolerance: f64,
    min_cutting_radius: f64,
    drape: &StampDrape<'_>,
) {
    // An entry the routed keep-down travel can replace goes out as the
    // route (`KeepDownRoute::Off`, every shipped strategy: never).
    if let Adaptive3dSegment::Rapid(entry) | Adaptive3dSegment::RapidWithFloor { entry, .. } =
        &segment
        && let Some(route) = routed_entry(material_stock, tool_radius, cursor, *entry, drape)
    {
        for piece in route {
            let piece = stamp_and_advance(
                material_stock,
                lut,
                tool_radius,
                cursor,
                piece,
                safe_z,
                tolerance,
                min_cutting_radius,
                drape,
            );
            segments.push(piece);
        }
        return;
    }
    let segment = stamp_and_advance(
        material_stock,
        lut,
        tool_radius,
        cursor,
        segment,
        safe_z,
        tolerance,
        min_cutting_radius,
        drape,
    );
    segments.push(segment);
}

/// The segments of a routed keep-down travel that replaces the entry
/// `entry` (`keep_down::keep_down_route`), or `None`: the travel is off,
/// the tool has no position, or no route holds.
///
/// - The travel runs at the entry's Z as `Link`s (cut feed), over cells
///   whose whole tool disc stands at most the stay-down clearance above it:
///   the full-disc proof.
/// - §16.7: only an entry whose own disc is free (it cuts nothing) is
///   replaced: a keep-down travel replaces the travel, never the entry's
///   cut. The route ends on the entry, with no side leg.
/// - The route's length is held to the stay-down distance (`stay_down_mm`),
///   the one keep-down dial (CUT-05).
/// - A tool standing above the travel's Z drops to it only when its disc
///   there is free (through air); a tool below it rises at its own XY
///   first.
fn routed_entry(
    stock: &TriDexelStock,
    tool_radius: f64,
    cursor: &PlannerCursor,
    entry: P3,
    drape: &StampDrape<'_>,
) -> Option<Vec<Adaptive3dSegment>> {
    let clearance = match drape.keep_down_route {
        super::KeepDownRoute::Off => return None,
        super::KeepDownRoute::CutFeed => drape.stay_down_clearance_mm,
    };
    let from = cursor.tool_pos?;
    if drape.stay_down_mm <= 0.0 {
        return None;
    }
    let travel_z = entry.z;
    let ceiling_z = entry.z + clearance;
    let free_at = |p: P2| {
        stock
            .max_conservative_top_z_in_disc(p.x, p.y, tool_radius)
            .is_none_or(|top| top <= ceiling_z)
    };
    // A tool above the travel drops to it only through air; a tool below
    // it rises at its own XY first (a rise cuts nothing).
    if from.z > travel_z + clearance && !free_at(P2::new(from.x, from.y)) {
        return None;
    }
    // §16.7: a keep-down travel replaces the travel, never the entry's cut.
    // An entry whose own footprint (the entry floor disc, `plan_entry`'s
    // read: the tool, or the helix's sweep) is not free cuts its column;
    // it stays.
    let entry_free = stock
        .max_conservative_top_z_in_disc(entry.x, entry.y, drape.entry_floor_radius)
        .is_none_or(|top| top <= ceiling_z);
    if !entry_free {
        return None;
    }
    let allowed = |p: P2| drape.centre_clip.is_none_or(|c| c.contains(p.x, p.y));
    // The route ends on the free cell of the entry (within one cell of its
    // point); the last move to the entry point runs between two free discs.
    let side_ok = |_: P2| true;
    let req = super::keep_down::RouteRequest {
        from: P2::new(from.x, from.y),
        to: P2::new(entry.x, entry.y),
        radius: tool_radius,
        ceiling_z,
        max_len: drape.stay_down_mm,
        side_max: stock.z_grid.cell_size,
        side_ok: &side_ok,
        allowed: &allowed,
    };
    let path = super::keep_down::straight_runs(&super::keep_down::keep_down_route(stock, &req)?);
    let last = *path.last()?;
    let mut out: Vec<Adaptive3dSegment> = Vec::with_capacity(path.len() + 2);
    if from.z < travel_z - 1e-9 {
        out.push(Adaptive3dSegment::Link(P3::new(from.x, from.y, travel_z)));
    }
    for p in &path {
        out.push(Adaptive3dSegment::Link(P3::new(p.x, p.y, travel_z)));
    }
    // The last move: to the entry point at the travel's Z (both discs free).
    if (entry.x - last.x).hypot(entry.y - last.y) > 1e-9 {
        out.push(Adaptive3dSegment::Link(entry));
    }
    Some(out)
}

/// Plan an entry (its floor and keep-down proof), stamp the segment into the
/// planner stock and move the cursor past it. Returns the segment to emit.
#[expect(
    clippy::too_many_arguments,
    reason = "each parameter is a separate input that the callers supply"
)]
fn stamp_and_advance(
    material_stock: &mut TriDexelStock,
    lut: &RadialProfileLUT,
    tool_radius: f64,
    cursor: &mut PlannerCursor,
    segment: Adaptive3dSegment,
    safe_z: f64,
    tolerance: f64,
    min_cutting_radius: f64,
    drape: &StampDrape<'_>,
) -> Adaptive3dSegment {
    // Every entry the planner emits passes this door, so every entry gets
    // its floor and its keep-down proof from the same stock read.
    let segment = match segment {
        Adaptive3dSegment::Rapid(entry) | Adaptive3dSegment::RapidWithFloor { entry, .. } => {
            plan_entry(material_stock, tool_radius, cursor.last_pos, entry, drape)
        }
        other => other,
    };
    let emitted_end = stamp_emitted_segment(
        material_stock,
        lut,
        tool_radius,
        cursor.tool_pos,
        &segment,
        safe_z,
        tolerance,
        min_cutting_radius,
        drape,
    );
    // Move the cursor to the segment's terminal XYZ. G-PHANTOMSTAMP: a cut
    // the emitter skips (fewer than two points) moves neither position.
    match &segment {
        Adaptive3dSegment::Cut(path) => {
            if path.len() >= 2
                && let Some(p) = path.last()
            {
                cursor.last_pos = Some(*p);
            }
        }
        Adaptive3dSegment::Rapid(entry) | Adaptive3dSegment::RapidWithFloor { entry, .. } => {
            cursor.last_pos = Some(*entry);
        }
        Adaptive3dSegment::Link(target) => {
            cursor.last_pos = Some(*target);
        }
        Adaptive3dSegment::Marker(_) => {}
    }
    if emitted_end.is_some() {
        cursor.tool_pos = emitted_end;
    }
    segment
}

/// Clear a Z level using EDT-based contour-parallel strategy.
///
/// 1. Build a boolean material grid at the given z_level.
/// 2. Compute a Euclidean Distance Transform on the inverted (air) grid,
///    giving distance-to-nearest-air for each material cell.
/// 3. Threshold the EDT at successive stepover intervals to produce
///    concentric contour rings via marching squares.
/// 4. Surface-drape each 2D contour to 3D using the surface heightmap.
/// 5. Stamp dexel stock along each cutting path.
///
/// This replaces the polygon-offset approach which hung on fine tools
/// due to iterative `offset_polygon` on high-vertex polygons.
#[expect(
    clippy::too_many_arguments,
    reason = "each parameter is a separate input that the callers supply"
)]
pub(super) fn clear_z_level_contour_parallel(
    ctx: &ClearZLevelContext<'_>,
    material_stock: &mut TriDexelStock,
    surface_hm: &SurfaceHeightmap,
    z_level: f64,
    segments: &mut Vec<Adaptive3dSegment>,
    cursor: &mut PlannerCursor,
    area_mask: Option<&AreaMask>,
    cancel: &dyn CancelCheck,
) -> Result<(), Cancelled> {
    // Check material remaining — skip if negligible. Gate on absolute
    // cell count, not fraction: at small DPP a real island contributes
    // very few cells per level, and a fraction-based gate would skip it.
    let (remaining_cells, _) = level_gate(
        material_stock,
        surface_hm,
        z_level,
        ctx.stock_to_leave,
        area_mask,
    );
    if remaining_cells < MIN_CELLS_TO_CLEAR {
        debug!(
            z = z_level,
            cells = remaining_cells,
            "CP: skipping — no material remaining"
        );
        return Ok(());
    }

    // 1. Build boolean material grid (material = true)
    let (material_grid, rows, cols, origin_x, origin_y, cell_size) = build_material_bool_grid(
        material_stock,
        surface_hm,
        z_level,
        ctx.stock_to_leave,
        area_mask,
    );

    let mat_count = material_grid.iter().filter(|&&b| b).count();
    // Check if any material exists
    if mat_count == 0 {
        debug!(z = z_level, "CP: skipping — empty material grid");
        return Ok(());
    }

    // 2. Compute EDT on the INVERTED grid (air = true as source).
    //    This gives distance to nearest air cell for each material cell.
    //    Material cells near the boundary have small distance.
    //    Interior material cells have large distance.
    let air_grid: Vec<bool> = material_grid.iter().map(|&b| !b).collect();
    let edt = crate::geometry::grid_field::distance_transform_2d(&air_grid, rows, cols);

    // 3. Find max distance (determines number of offset levels)
    let max_dist = edt.iter().copied().fold(0.0f64, f64::max);

    // 4. Generate contours at each stepover threshold
    let tool_radius_cells = ctx.tool_radius / cell_size;
    let stepover_cells = ctx.stepover / cell_size;

    debug!(
        z = z_level,
        remaining_cells,
        mat_count,
        rows,
        cols,
        max_dist_cells = max_dist,
        tool_radius_cells = tool_radius_cells,
        stepover_cells = stepover_cells,
        "Contour-parallel EDT: generating offset contours"
    );

    // The starting threshold determines the outermost contour offset. For wide
    // material regions (max_dist >> tool_radius), start at tool_radius_cells so
    // the tool's outer edge just reaches the boundary. For narrower or annular
    // regions, start lower so contours exist even in the narrowest sections where
    // EDT is small. Using min(tool_radius, stepover * 0.5) keeps the first
    // contour close enough to the boundary that even 2-3-cell-wide strips of
    // material have cells above the threshold.
    let mut threshold = tool_radius_cells.min(stepover_cells * 0.5).max(1.0);
    while threshold < max_dist {
        check_cancel(cancel)?;

        // Threshold the EDT: cells with distance > threshold are "inside" the offset
        let mask: Vec<bool> = edt.iter().map(|&d| d > threshold).collect();
        let loops = marching_squares_bool_grid(&mask, rows, cols, origin_x, origin_y, cell_size);

        for loop_pts in &loops {
            if loop_pts.len() < 3 {
                continue;
            }

            // Each ring cuts at its level, or on the surface (plus the stock
            // to leave) where the surface is above the level. G-A (operator
            // ruling D-3, 2026-10-28): the Z Blend lerp toward the surface is
            // gone; it cut inner rings deeper than one Depth/Pass.
            let mut path_3d: Vec<P3> = Vec::with_capacity(loop_pts.len());
            for p in loop_pts {
                let surf_z = surface_hm.z_or_bbox_floor_at_world(p.x, p.y);
                let target_z = if surf_z == f64::NEG_INFINITY {
                    z_level
                } else {
                    surf_z + ctx.stock_to_leave // actual terrain — may be below z_level
                };
                let z = z_level.max(target_z);
                path_3d.push(P3::new(p.x, p.y, z));
            }

            // Pick the entry point that requires the shallowest plunge
            // through fresh material. The natural starting point of the
            // marching-squares contour is wherever the algorithm's pixel
            // walk happened to begin, which can land on full-height stock
            // for a deeper Z-level pass — leading to a deep vertical
            // plunge through material from safe_z down to z_level.
            //
            // Rotating the closed loop so the point with the *lowest
            // current stock_top* is first means the plunge passes through
            // mostly already-cleared air, then bites only the depth-of-cut
            // worth of material at the bottom. Same total cut area, same
            // contour shape — just a kinder entry XY for closed loops.
            //
            // Only applies to closed contour loops (>= 3 pts); open
            // single-segment cleanup paths are left alone.
            if path_3d.len() >= 3 {
                let stock_top_at_path_idx = |idx: usize| -> f64 {
                    #[expect(
                        clippy::indexing_slicing,
                        reason = "idx < path_3d.len() by construction"
                    )]
                    let p = &path_3d[idx];
                    match material_stock.z_grid.world_to_cell(p.x, p.y) {
                        Some((row, col)) => stock_top_z_at(material_stock, row, col),
                        None => f64::INFINITY,
                    }
                };
                let best = (0..path_3d.len()).min_by(|&a, &b| {
                    let ta = stock_top_at_path_idx(a);
                    let tb = stock_top_at_path_idx(b);
                    ta.partial_cmp(&tb).unwrap_or(std::cmp::Ordering::Equal)
                });
                if let Some(idx) = best
                    && idx > 0
                {
                    path_3d.rotate_left(idx);
                }
            }

            // Emit entry (link or rapid) + cut segment
            if let Some(first) = path_3d.first() {
                // Stay-down link if close to previous position AND the link
                // path is clear of material. Without the is_clear_path_3d
                // gate, z_blend=true produced Link moves that crossed
                // uncut terrain between rings at different Z heights
                // (F-5 in planning/adaptive_review_2026-04.md). The gate
                // matches the one in clear_z_level_slice.
                let should_link = cursor
                    .last_pos
                    .is_some_and(|lp| side_link_ok(ctx, material_stock, surface_hm, lp, *first));
                let entry_seg = if should_link {
                    Adaptive3dSegment::Link(*first)
                } else {
                    Adaptive3dSegment::Rapid(*first)
                };
                push_segment_with_stamp(
                    segments,
                    material_stock,
                    ctx.lut,
                    ctx.tool_radius,
                    cursor,
                    entry_seg,
                    ctx.safe_z,
                    ctx.cut_tolerance,
                    ctx.min_cutting_radius,
                    &ctx.stamp_drape(),
                );
                push_segment_with_stamp(
                    segments,
                    material_stock,
                    ctx.lut,
                    ctx.tool_radius,
                    cursor,
                    Adaptive3dSegment::Cut(path_3d),
                    ctx.safe_z,
                    ctx.cut_tolerance,
                    ctx.min_cutting_radius,
                    &ctx.stamp_drape(),
                );
            }
        }

        threshold += stepover_cells;
    }

    // Cleanup: narrow sections of the material region (annular rings near steep
    // walls) may have EDT below the starting threshold, leaving them without a
    // contour pass. Identify remaining material cells and stamp a raster cleanup.
    let (cleanup_grid, cr, cc, co_x, co_y, c_cs) = build_material_bool_grid(
        material_stock,
        surface_hm,
        z_level,
        ctx.stock_to_leave,
        area_mask,
    );
    let cleanup_count = cleanup_grid.iter().filter(|&&b| b).count();
    if cleanup_count > 0 {
        // Raster through remaining material rows.  For each row with material
        // cells, build contiguous runs and emit one cut per run.
        //
        // F-029: clamp per-cell cut depth to `depth_per_pass` (plus tolerance)
        // above the cell's current stock top. Without this, cells where the
        // stock top is far above `z_level` (e.g. cells outside the mesh XY
        // footprint, which `SurfaceHeightmap` reports `surf_z = min_z` for
        // and never get covered by the contour iso-lines because their EDT
        // is dominated by the boundary) would receive a single cleanup cut
        // at `z_level` that the simulator faithfully replays as one swept
        // tube from the virgin stock top down to `z_level` — yielding
        // axial-engagement readings of ~50 mm on a 3 mm-commanded DPP and
        // tripping the deflection gate. Padded-grid (row, col) maps to
        // stock-grid (row-1, col-1); border cells (row=0/cr-1 or
        // col=0/cc-1) are always false in `cleanup_grid` so the inner
        // mapping is safe.
        let z_for_cell = |row: usize, col: usize| -> f64 {
            let wx = co_x + col as f64 * c_cs;
            let wy = co_y + row as f64 * c_cs;
            let surf_z = surface_hm.z_or_bbox_floor_at_world(wx, wy);
            // Lower bound (the "leave stock above the surface" rule).
            let lower = if surf_z == f64::NEG_INFINITY {
                z_level
            } else {
                super::intended_floor::level_floor_z(surf_z, ctx.stock_to_leave, z_level)
            };
            // Per-cell stock-top clamp. Padded coords (row, col) → stock
            // coords (row-1, col-1).
            if row == 0 || col == 0 {
                return lower;
            }
            let s_row = row - 1;
            let s_col = col - 1;
            let grid = &material_stock.z_grid;
            if s_row >= grid.rows || s_col >= grid.cols {
                return lower;
            }
            let stock_top = stock_top_z_at(material_stock, s_row, s_col);
            // Don't cut more than depth_per_pass + tolerance below the
            // current stock top in a single pass. Tolerance lets surface-
            // adjacent cells still bottom out at `lower` without leaving a
            // sliver.
            let dpp_clamp = stock_top - ctx.depth_per_pass - ctx.tolerance;
            lower.max(dpp_clamp)
        };
        let mut cleanup_pts: Vec<Vec<P3>> = Vec::new();
        for row in 0..cr {
            let mut run_start: Option<usize> = None;
            for col in 0..cc {
                #[expect(
                    clippy::indexing_slicing,
                    reason = "row*cc+col bounded by grid dimensions"
                )]
                let is_mat = cleanup_grid[row * cc + col];
                if is_mat {
                    if run_start.is_none() {
                        run_start = Some(col);
                    }
                } else if let Some(start) = run_start.take() {
                    let mut path = Vec::new();
                    let mut c = start;
                    while c < col {
                        let wx = co_x + c as f64 * c_cs;
                        let wy = co_y + row as f64 * c_cs;
                        let z = z_for_cell(row, c);
                        path.push(P3::new(wx, wy, z));
                        c += 1;
                    }
                    if path.len() >= 2 {
                        cleanup_pts.push(path);
                    }
                }
            }
            // Close any run that reached the end of the row
            if let Some(start) = run_start {
                let mut path = Vec::new();
                let mut c = start;
                while c < cc {
                    let wx = co_x + c as f64 * c_cs;
                    let wy = co_y + row as f64 * c_cs;
                    let z = z_for_cell(row, c);
                    path.push(P3::new(wx, wy, z));
                    c += 1;
                }
                if path.len() >= 2 {
                    cleanup_pts.push(path);
                }
            }
        }

        for run in &cleanup_pts {
            // Start the run at the end nearer to the tool, so the link gate
            // below sees the short hop from the run before.
            let mut path = run.clone();
            if let (Some(lp), Some(a), Some(b)) = (cursor.last_pos, path.first(), path.last()) {
                let d = |p: &P3| (p.x - lp.x).powi(2) + (p.y - lp.y).powi(2);
                if d(b) < d(a) {
                    path.reverse();
                }
            }
            if let Some(first) = path.first() {
                let should_link = cursor
                    .last_pos
                    .is_some_and(|lp| side_link_ok(ctx, material_stock, surface_hm, lp, *first));
                let entry_seg = if should_link {
                    Adaptive3dSegment::Link(*first)
                } else {
                    Adaptive3dSegment::Rapid(*first)
                };
                push_segment_with_stamp(
                    segments,
                    material_stock,
                    ctx.lut,
                    ctx.tool_radius,
                    cursor,
                    entry_seg,
                    ctx.safe_z,
                    ctx.cut_tolerance,
                    ctx.min_cutting_radius,
                    &ctx.stamp_drape(),
                );
                if path.len() >= 2 {
                    push_segment_with_stamp(
                        segments,
                        material_stock,
                        ctx.lut,
                        ctx.tool_radius,
                        cursor,
                        Adaptive3dSegment::Cut(path.clone()),
                        ctx.safe_z,
                        ctx.cut_tolerance,
                        ctx.min_cutting_radius,
                        &ctx.stamp_drape(),
                    );
                } else {
                    // Single-point run: emit as a tiny cut segment.
                    let end = P3::new(first.x + ctx.step_len, first.y, first.z);
                    push_segment_with_stamp(
                        segments,
                        material_stock,
                        ctx.lut,
                        ctx.tool_radius,
                        cursor,
                        Adaptive3dSegment::Cut(vec![*first, end]),
                        ctx.safe_z,
                        ctx.cut_tolerance,
                        ctx.min_cutting_radius,
                        &ctx.stamp_drape(),
                    );
                }
            }
        }
    }

    Ok(())
}

#[expect(
    clippy::indexing_slicing,
    reason = "bounded indexing in algorithmic code"
)]
/// Run waterline boundary cleanup at a given Z level.
///
/// When `slope_map` is provided, only traces contours through steep regions
/// (slope angle > 30°). This avoids re-tracing shallow areas that the
/// adaptive spiral already cleared.
#[expect(
    clippy::too_many_arguments,
    reason = "each parameter is a separate input that the callers supply"
)]
pub(super) fn waterline_cleanup(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    lut: &RadialProfileLUT,
    slope_map: &SlopeMap,
    material_stock: &mut TriDexelStock,
    z_level: f64,
    tool_radius: f64,
    entry_floor_radius: f64,
    cell_size: f64,
    safe_z: f64,
    tolerance: f64,
    min_cutting_radius: f64,
    stock_to_leave: f64,
    ctx: &ClearZLevelContext<'_>,
    segments: &mut Vec<Adaptive3dSegment>,
    cursor: &mut PlannerCursor,
    debug_ctx: Option<&ToolpathDebugContext>,
    cancel: &dyn CancelCheck,
) -> Result<(), Cancelled> {
    // Same drape the emitter applies to these segments; see
    // `stamp_emitted_segment`.
    let drape = StampDrape {
        mesh,
        index,
        cutter,
        stock_to_leave,
        // The operation's entry style enters a waterline contour. A helix
        // sweeps the tool radius plus the helix radius from the entry XY,
        // so the planner reads the entry floor over that disc
        // (`path.rs::entry_floor_radius`, G-WLENTRYDISC). A waterline
        // contour gets no keep-down link.
        entry_floor_radius,
        stay_down_mm: 0.0,
        centre_clip: ctx.centre_clip,
        keep_down_route: super::KeepDownRoute::Off,
        stay_down_clearance_mm: 0.0,
    };
    #[cfg(not(target_arch = "wasm32"))]
    let t_waterline = Instant::now();
    let waterline_scope = debug_ctx.map(|ctx| {
        ctx.start_span(
            DebugSpanKind::WaterlineCleanup,
            format!("Waterline cleanup Z {:.3}", z_level),
        )
    });
    let sampling = tool_radius.max(cell_size * 4.0);
    let contours = waterline_contours_with_cancel(mesh, index, cutter, z_level, sampling, cancel)?;

    // Threshold for steep-only waterline: only trace contours where slope > 30°.
    // This eliminates redundant shallow-area waterline passes.
    let steep_threshold = 30.0_f64.to_radians();

    let mut traced = 0u32;
    for contour in &contours {
        check_cancel(cancel)?;
        if contour.len() < 3 {
            continue;
        }

        // Check if this contour is predominantly in a steep region.
        // Sample a few points and check the slope. If most are shallow, skip.
        let sample_step = 1.max(contour.len() / 10);
        let steep_samples = contour
            .iter()
            .step_by(sample_step)
            .filter(|p| {
                slope_map
                    .angle_at_world(p.x, p.y)
                    .is_some_and(|a| a >= steep_threshold)
            })
            .count();
        let total_samples = contour.len().div_ceil(sample_step);
        if total_samples > 0 && steep_samples * 3 < total_samples {
            // Less than 1/3 of samples are steep — skip this contour
            continue;
        }
        // Geodesic Adaptive (research C38 §16): a contour that lies in a piece the arm
        // left to the rest contour pass (at least half its samples inside
        // one at this level) is the rest pass's work too. Only the arm
        // leaves pieces, so a Contour Parallel run reads no piece here.
        let left = ctx.left_pieces.borrow();
        if !left.is_empty() {
            let inside = contour
                .iter()
                .step_by(sample_step)
                .filter(|p| {
                    left.iter().any(|(z, piece)| {
                        (z - z_level).abs() < 1e-9 && piece.contains_point(&P2::new(p.x, p.y))
                    })
                })
                .count();
            if inside * 2 >= total_samples {
                continue;
            }
        }

        push_segment_with_stamp(
            segments,
            material_stock,
            lut,
            tool_radius,
            cursor,
            Adaptive3dSegment::Rapid(contour[0]),
            safe_z,
            tolerance,
            min_cutting_radius,
            &drape,
        );

        let mut cleanup_path = vec![contour[0]];
        for i in 0..contour.len() {
            let a = contour[i];
            let b = contour[(i + 1) % contour.len()];
            let dx = b.x - a.x;
            let dy = b.y - a.y;
            let len = (dx * dx + dy * dy).sqrt();
            let n_steps = (len / (cell_size * 1.5)).ceil() as usize;
            for j in 1..=n_steps {
                let t = j as f64 / n_steps.max(1) as f64;
                let x = a.x + t * dx;
                let y = a.y + t * dy;
                let z = a.z + t * (b.z - a.z);
                cleanup_path.push(P3::new(x, y, z));
            }
        }
        cleanup_path.push(contour[0]);
        push_segment_with_stamp(
            segments,
            material_stock,
            lut,
            tool_radius,
            cursor,
            Adaptive3dSegment::Cut(cleanup_path),
            safe_z,
            tolerance,
            min_cutting_radius,
            &drape,
        );
        traced += 1;
    }
    if !contours.is_empty() {
        #[cfg(not(target_arch = "wasm32"))]
        let wl_ms = t_waterline.elapsed().as_millis() as u64;
        #[cfg(target_arch = "wasm32")]
        let wl_ms = 0u64;
        debug!(
            total = contours.len(),
            traced = traced,
            z = z_level,
            elapsed_ms = wl_ms,
            "Waterline cleanup (slope-filtered)"
        );
    }
    if let Some(scope) = waterline_scope.as_ref() {
        scope.set_z_level(z_level);
        scope.set_counter("contours", contours.len() as f64);
        scope.set_counter("traced", traced as f64);
    }

    Ok(())
}

// ── 2.5D slice adaptive (GeodesicAdaptive strategy) ────────────────────

/// Centroid (vertex average) of a polygon's exterior, as an (x, y)
/// anchor for region-ordering. Vertex-average, not the area-weighted
/// centroid — cheaper and adequate as a travel-ordering proxy.
fn polygon_centroid_xy(poly: &crate::polygon::Polygon2) -> (f64, f64) {
    let n = poly.exterior.len().max(1) as f64;
    let (sx, sy) = poly
        .exterior
        .iter()
        .fold((0.0, 0.0), |(ax, ay), p| (ax + p.x, ay + p.y));
    (sx / n, sy / n)
}

/// Greedy nearest-neighbor tour over 2D `anchors`, starting from
/// `start`. Returns the visit order as indices into `anchors`. Used to
/// order disjoint machinable regions so the cutter hops to the nearest
/// one next instead of following marching-squares scan order.
///
/// Shares [`crate::geometry::nn_order::NearestPicker`] with the other four G5 sites
/// (PERF_REVIEW G5) — the same lexicographic `(d², index)` answer
/// the hand-rolled Θ(n²) scan gave, without the quadratic. A Z-level usually
/// produces a handful of regions, for which the picker scans linearly and
/// this is a consolidation rather than a speed-up; the guarantee is that no
/// level can produce enough regions to hang.
#[expect(clippy::indexing_slicing, reason = "anchors indexed by enumerate idx")]
fn nearest_neighbor_order(anchors: &[(f64, f64)], start: (f64, f64)) -> Vec<usize> {
    let mut picker = crate::geometry::nn_order::NearestPicker::new(
        crate::geometry::nn_order::Metric::EuclidSq,
        anchors.len(),
    );
    for (i, a) in anchors.iter().enumerate() {
        picker.push(i, a.0, a.1);
    }
    picker.build();
    let mut order: Vec<usize> = Vec::with_capacity(anchors.len());
    let mut cur = start;
    for _ in 0..anchors.len() {
        // The scan had no fallback: a step that found nothing emitted
        // nothing and left `cur` where it was, so the order came back short.
        // `f64::INFINITY` was its sentinel; both are reproduced.
        let i = match picker.nearest(cur.0, cur.1) {
            Some((i, d)) if d < f64::INFINITY => i,
            _ => continue,
        };
        picker.remove(i);
        order.push(i);
        cur = anchors[i];
    }
    order
}

/// Signed polygon area via the shoelace formula. Positive = CCW.
fn polygon_signed_area(points: &[P2]) -> f64 {
    let n = points.len();
    if n < 3 {
        return 0.0;
    }
    let mut acc = 0.0;
    for i in 0..n {
        #[expect(clippy::indexing_slicing, reason = "i < n, (i+1) % n < n")]
        let a = points[i];
        #[expect(clippy::indexing_slicing, reason = "i < n, (i+1) % n < n")]
        let b = points[(i + 1) % n];
        acc += a.x * b.y - b.x * a.y;
    }
    0.5 * acc
}

/// Geodesic Adaptive via 2.5D slices: at each Z-level, extract the 2D
/// material polygon via marching squares, then plan each piece with the
/// 2D geodesic plan (`plan_one_outline`). The F-038 forecast still runs
/// the 2D agent planner (`adaptive_segments_with_debug`). Lift the
/// resulting 2D path back to 3D (Z clamped to `z_level` or
/// surface+stock_to_leave) and stamp the dexel stock along it.
///
/// The trade-off
/// is that the tool stays at a fixed Z within each slab (no per-step
/// terrain follow) — acceptable for roughing; finish passes handle
/// the staircase.
/// Machine rapid rate (mm/min) assumed for the feed-vs-rapid crossover.
/// The planner has no machine context at this layer; 5000 mm/min matches
/// the Shapeoko-class grbl default the retired 70 mm constant was tuned
/// against. Worst case of a wrong guess is a suboptimal link/retract
/// choice, never an unsafe move.
const ASSUMED_RAPID_MM_MIN: f64 = 5000.0;

/// XY length above which demoting an in-slice air run to a
/// retract + rapid + re-plunge cycle is faster than feeding through it.
///
/// Solves `len/feed = len/rapid + overhead(retract_depth)` for `len`,
/// where the overhead is the retract cycle: climb `retract_depth` at
/// rapid, rapid back down to the cleared floor + buffer, final buffer at
/// plunge rate. Replaces the hardcoded `MIN_AIR_RUN_MM = 70.0` (tuned to
/// a 6 mm tool at 3150 mm/min feed — Stage 0, algorithm review
/// 2026-06-12 F3).
pub(super) fn air_run_crossover_mm(
    feed_mm_min: f64,
    plunge_mm_min: f64,
    retract_depth_mm: f64,
) -> f64 {
    let feed = feed_mm_min.max(1.0) / 60.0;
    let rapid = ASSUMED_RAPID_MM_MIN / 60.0;
    let plunge = plunge_mm_min.max(1.0) / 60.0;
    if feed >= rapid {
        // Feeding is at least as fast as rapiding: a demotion never pays.
        return f64::INFINITY;
    }
    let depth = retract_depth_mm.max(0.0);
    let overhead_s = depth / rapid
        + (depth - RAPID_DESCENT_BUFFER_MM).max(0.0) / rapid
        + RAPID_DESCENT_BUFFER_MM.min(depth) / plunge;
    overhead_s / (1.0 / feed - 1.0 / rapid)
}

/// The regions one Z level clears, in the order the cutter visits them, and
/// the plan metrics the detection stage already knows.
///
/// CUT-09: the product of stage 1 of `clear_z_level_slice`.
struct OrderedRegions {
    regions: Vec<crate::polygon::Polygon2>,
    metrics: ZLevelPlanMetrics,
}

/// Stage 1 — find the regions this Z level must clear and order them.
///
/// The material boolean grid at `z_level`, its marching-squares contours,
/// the containment grouping into outer regions with their holes, the
/// sub-tool area filter and the nearest-neighbour visit order. Reads the
/// stock; changes nothing.
///
/// `None` means there is nothing left to cut at this level.
fn detect_and_order_regions(
    ctx: &ClearZLevelContext<'_>,
    material_stock: &TriDexelStock,
    surface_hm: &SurfaceHeightmap,
    z_level: f64,
    area_mask: Option<&AreaMask>,
    last_pos: Option<P3>,
) -> Option<OrderedRegions> {
    // 1. Material boolean grid at this Z-level (includes 1-cell air padding).
    let (material_grid, rows, cols, origin_x, origin_y, cell_size) = build_material_bool_grid(
        material_stock,
        surface_hm,
        z_level,
        ctx.stock_to_leave,
        area_mask,
    );
    // Geodesic Adaptive (research C38 §11, G-CLIPAIR): regions stay inside the boundary,
    // and include the centres whose disc meets the reachable band.
    let material_grid = {
        let mut grid = material_grid;
        if let Some(clip) = ctx.region_clip.as_ref() {
            clip.restrict_regions(&mut grid, rows, cols);
        }
        grid
    };
    if !material_grid.iter().any(|&b| b) {
        return None;
    }

    // 2. Marching squares → polygon contours.
    let contours = crate::geometry::contour_extract::marching_squares_bool_grid(
        &material_grid,
        rows,
        cols,
        origin_x,
        origin_y,
        cell_size,
    );
    if contours.is_empty() {
        return None;
    }

    // 3. Group contours into disjoint regions with their contained holes.
    //
    //    Marching squares emits one contour per material/air boundary, both
    //    outer boundaries (CCW, positive signed area) and hole boundaries
    //    (CW, negative). For multi-region slices — e.g. terrain hills
    //    emerging as separate islands at shallow Z — there are multiple
    //    outer boundaries that must each be cleared independently.
    //
    //    The previous implementation flattened signed area to absolute
    //    value, treated the largest contour as the only outer, and pushed
    //    every other contour into that one polygon's `holes` list. Disjoint
    //    islands got misclassified as holes — the 2D adaptive then treated
    //    them as already-cleared interior pockets, walking around them and
    //    plunging into them at "safe" XYs. Visible symptom: drilled holes
    //    through fresh stock, low engagement, high air-cut on terrain-shaped
    //    geometry.
    //
    //    `polygon::detect_containment` does the right thing: builds each
    //    contour as a single-loop polygon, runs containment tests, and
    //    returns N outer regions each with their nested holes (CW-flipped)
    //    attached.
    let single_loops: Vec<crate::polygon::Polygon2> = contours
        .into_iter()
        .filter(|pts| polygon_signed_area(pts).abs() > cell_size * cell_size)
        .map(crate::polygon::Polygon2::new)
        .collect();
    if single_loops.is_empty() {
        return None;
    }
    let mut regions = crate::polygon::detect_containment(single_loops);
    let mut region_areas_mm2: Vec<f64> = regions.iter().map(|r| r.area().abs()).collect();
    region_areas_mm2.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
    region_areas_mm2.truncate(10);

    // Fusion-style "ignore stock smaller than X" filter. Heightmap-style
    // models (e.g. terrain.stl) emit many micro-peaks at top Z levels
    // — each becomes its own region with its own perimeter sweep + 2D
    // adaptive entry/exit. The cutter spends most of its in-cut time
    // travelling between them, technically at feed_rate but barely
    // engaging material. On wanaka this drove 81% air-cut at every Z.
    //
    // Nominal threshold = (2 × tool_diameter)² ≈ "the tool footprint plus
    // an offset ring fits". Anything smaller is sub-tool noise the cutter
    // can't address efficiently anyway.
    //
    // DPP scaling: the threshold has to scale with depth-per-pass because
    // at small DPP the same real island gets sliced into many thin layers,
    // and the perimeter sweep erodes its area progressively at each level
    // until it drops below the threshold — at which point the unmilled
    // core is never addressed (the level skips it, and the next level sees
    // an even smaller region). Without DPP scaling, small-DPP roughing
    // leaves visible unmilled islands. We scale linearly with
    // DPP / baseline_dpp where baseline = tool_radius (a reasonable max
    // aggressive rough), with a floor of (tool_diameter/2)² so even very
    // small DPP still drops obvious sub-tool noise.
    let tool_diameter = ctx.tool_radius * 2.0;
    let baseline_dpp = ctx.tool_radius.max(0.5);
    let nominal_min_area = (tool_diameter * 2.0).powi(2);
    let dpp_factor = (ctx.depth_per_pass / baseline_dpp).clamp(0.0, 1.0);
    let area_floor = tool_diameter.powi(2) * 0.25;
    let min_region_area_mm2 = (nominal_min_area * dpp_factor).max(area_floor);
    let region_count_before = regions.len();
    regions.retain(|r| r.area().abs() >= min_region_area_mm2);
    let dropped_micro = region_count_before - regions.len();
    if dropped_micro > 0 {
        debug!(
            z = z_level,
            dropped = dropped_micro,
            kept = regions.len(),
            threshold_mm2 = min_region_area_mm2,
            "Dropped sub-tool regions (Fusion-style ignore-features filter)"
        );
    }
    if regions.is_empty() {
        return None;
    }

    // Nearest-neighbor region ordering. Marching squares yields regions
    // in scan order (row-major), so the cutter can rapid back and forth
    // across the stock as it hops between them. Greedily visiting the
    // nearest unvisited region from the cutter's current XY cuts that
    // inter-region rapid travel. Pure ordering — it doesn't change which
    // material each region clears, so coverage is unaffected; safe for
    // every strategy. Seeded from `last_pos` (the cutter's position
    // entering this Z-level) or the stock origin on the first pass.
    if regions.len() > 2 {
        let start = last_pos.map(|p| (p.x, p.y)).unwrap_or((0.0, 0.0));
        let anchors: Vec<(f64, f64)> = regions.iter().map(polygon_centroid_xy).collect();
        let order = nearest_neighbor_order(&anchors, start);
        #[expect(
            clippy::indexing_slicing,
            reason = "order entries are valid region indices"
        )]
        let reordered: Vec<_> = order.into_iter().map(|i| regions[i].clone()).collect();
        regions = reordered;
    }

    let level_metrics = ZLevelPlanMetrics {
        available: true,
        marching_squares_regions: region_count_before,
        region_areas_mm2,
        dropped_micro_region_count: dropped_micro,
        perimeter_sweep_length_mm: 0.0,
        agent_walk_cut_length_mm: 0.0,
        residual_cleanup_cell_count: 0,
        dropped_short_region_count: 0,
    };

    Some(OrderedRegions {
        regions,
        metrics: level_metrics,
    })
}

/// The accumulators a Z level appends to, plus the two counters and
/// the metrics record its stages share.
///
/// CUT-09: `clear_z_level_slice` used to carry these as four
/// separate `&mut` parameters, which is what made it need
/// `clippy::too_many_arguments`.
struct LevelEmission<'a> {
    sink: LevelSink<'a>,
    level_metrics: &'a mut ZLevelPlanMetrics,
    cut_count: &'a mut u32,
    dropped_short_regions: &'a mut usize,
}

/// The one door every segment of a Geodesic Adaptive level goes
/// through into the plan and the planner stock.
///
/// G-PHANTOMSTAMP: with the F-038 gate on (`min_region_cut_length_mm > 0`)
/// an entry is stamped on COMMIT, not on push. The sink pushes an entry
/// unplanned and holds it pending. The next cut or link commits it: the
/// sink plans it (`plan_entry`), stamps it and moves the cursor, then
/// stamps the cut. The next entry instead REPLACES it: the pending entry
/// leaves the plan, never stamped, and the cursor does not move. So the
/// planner stock holds exactly the segments the emitter will emit, in
/// order. (Before, every entry was stamped at push and the level-end
/// coalescing pass deleted some of them afterwards; their columns stayed in
/// the planner stock, and a later entry's rapid floor read that phantom
/// cut — a rapid into stock on Wanaka "3D Rough 6".) Nothing is stamped
/// between the push and the commit, so the commit reads the stock the push
/// would have read.
struct LevelSink<'a> {
    segments: &'a mut Vec<Adaptive3dSegment>,
    material_stock: &'a mut TriDexelStock,
    cursor: &'a mut PlannerCursor,
    /// Index in `segments` of the entry pushed but not yet committed.
    pending_entry: Option<usize>,
    /// Entries a later entry replaced before a cut or link committed them
    /// (the F-038 `coalesced_entries_f038` counter).
    replaced_entries: usize,
    /// The F-038 gate. Off, every segment is stamped at push, as before.
    defer_entries: bool,
}

impl LevelSink<'_> {
    fn emit(&mut self, ctx: &ClearZLevelContext<'_>, segment: Adaptive3dSegment) {
        let Some(clip) = ctx.centre_clip else {
            self.emit_clipped(ctx, segment);
            return;
        };
        // A link's clip reads the planner position, so the pending entry it
        // would commit is committed first, as `emit_clipped` would.
        if matches!(segment, Adaptive3dSegment::Link(_)) {
            self.commit_pending(ctx);
        }
        for piece in clip_segment(clip, self.cursor, segment) {
            self.emit_clipped(ctx, piece);
        }
    }

    /// [`Self::emit`] for a segment that has passed the boundary clip.
    fn emit_clipped(&mut self, ctx: &ClearZLevelContext<'_>, segment: Adaptive3dSegment) {
        if !self.defer_entries {
            push_segment_with_stamp_unclipped(
                self.segments,
                self.material_stock,
                ctx.lut,
                ctx.tool_radius,
                self.cursor,
                segment,
                ctx.safe_z,
                ctx.cut_tolerance,
                ctx.min_cutting_radius,
                &ctx.stamp_drape(),
            );
            return;
        }
        match segment {
            Adaptive3dSegment::Rapid(_) | Adaptive3dSegment::RapidWithFloor { .. } => {
                if let Some(i) = self.pending_entry.take() {
                    // Only markers stand after the pending entry, so the
                    // removal keeps every other segment in order.
                    self.segments.remove(i);
                    self.replaced_entries += 1;
                }
                self.pending_entry = Some(self.segments.len());
                self.segments.push(segment);
            }
            // The emitter emits no move for a marker or for a cut of fewer
            // than two points: they neither commit nor replace an entry.
            Adaptive3dSegment::Marker(_) => self.segments.push(segment),
            Adaptive3dSegment::Cut(ref path) if path.len() < 2 => self.segments.push(segment),
            Adaptive3dSegment::Cut(_) | Adaptive3dSegment::Link(_) => {
                self.commit_pending(ctx);
                push_segment_with_stamp_unclipped(
                    self.segments,
                    self.material_stock,
                    ctx.lut,
                    ctx.tool_radius,
                    self.cursor,
                    segment,
                    ctx.safe_z,
                    ctx.cut_tolerance,
                    ctx.min_cutting_radius,
                    &ctx.stamp_drape(),
                );
            }
        }
    }

    /// Plan, stamp and keep the pending entry in its place. The level end
    /// calls this too: a trailing entry stays in the plan, as it did.
    fn commit_pending(&mut self, ctx: &ClearZLevelContext<'_>) {
        let Some(i) = self.pending_entry.take() else {
            return;
        };
        let Some(slot) = self.segments.get_mut(i) else {
            return;
        };
        let entry = match slot {
            Adaptive3dSegment::Rapid(entry) | Adaptive3dSegment::RapidWithFloor { entry, .. } => {
                *entry
            }
            _ => return,
        };
        // An entry the routed keep-down travel can replace goes out as the
        // route, in the entry's place (only markers stand after a pending
        // entry, so the order holds). `KeepDownRoute::Off`: never.
        let drape = ctx.stamp_drape();
        if let Some(route) = routed_entry(
            self.material_stock,
            ctx.tool_radius,
            self.cursor,
            entry,
            &drape,
        ) {
            let mut stamped = Vec::with_capacity(route.len());
            for piece in route {
                stamped.push(stamp_and_advance(
                    self.material_stock,
                    ctx.lut,
                    ctx.tool_radius,
                    self.cursor,
                    piece,
                    ctx.safe_z,
                    ctx.cut_tolerance,
                    ctx.min_cutting_radius,
                    &drape,
                ));
            }
            self.segments.splice(i..=i, stamped);
            return;
        }
        *slot = stamp_and_advance(
            self.material_stock,
            ctx.lut,
            ctx.tool_radius,
            self.cursor,
            Adaptive3dSegment::Rapid(entry),
            ctx.safe_z,
            ctx.cut_tolerance,
            ctx.min_cutting_radius,
            &drape,
        );
    }
}

/// What every region of one Z level reads and none of them changes.
struct RegionClearPlan<'a> {
    surface_hm: &'a SurfaceHeightmap,
    z_level: f64,
    params_2d: &'a crate::adaptive::AdaptiveParams,
    level_ctx: &'a Option<ToolpathDebugContext>,
    region_total: usize,
}

/// The inward offset (mm) of the shipped region smoothing
/// ([`smoothed_region`]).
const SIMPLIFY_INSET: f64 = 0.3;
/// The Douglas-Peucker tolerance (mm) of the shipped region smoothing
/// ([`smoothed_region`]).
const SIMPLIFY_TOLERANCE: f64 = 0.3;

/// The region polygon smoothed for the 2D planner, or empty under the
/// Legacy cleanup (the planner then plans the region itself).
fn smoothed_region(
    region_polygon: &crate::polygon::Polygon2,
    params_2d: &crate::adaptive::AdaptiveParams,
) -> Vec<crate::polygon::Polygon2> {
    // Simplify the per-region polygon to smooth the marching-squares
    // staircase pattern before the 2D adaptive runs. Two-step:
    //   1. Inward offset by SIMPLIFY_INSET so the simplified result
    //      stays inside the original — guarantees we never remove
    //      more stock than the original polygon allows.
    //   2. Douglas-Peucker on each contour with SIMPLIFY_TOLERANCE,
    //      collapsing stair-step vertices into straight lines.
    // Stock left at the boundary is bounded by SIMPLIFY_INSET +
    // SIMPLIFY_TOLERANCE ≈ 0.6 mm — well within typical 0.5–1 mm
    // radial-stock-to-leave allowances for a roughing pass.
    if !matches!(
        params_2d.cleanup_strategy,
        crate::adaptive::CleanupStrategy::Legacy
    ) {
        let inset = crate::polygon::offset_polygon(region_polygon, SIMPLIFY_INSET);
        inset
            .into_iter()
            .map(|mut poly| {
                poly.exterior =
                    crate::adaptive::path::simplify_path(&poly.exterior, SIMPLIFY_TOLERANCE);
                poly.holes = poly
                    .holes
                    .into_iter()
                    .map(|h| crate::adaptive::path::simplify_path(&h, SIMPLIFY_TOLERANCE))
                    .filter(|h| h.len() >= 3)
                    .collect();
                // `exterior` replaced — see the mutation contract on
                // `Polygon2`.
                poly.invalidate_bbox();
                poly
            })
            .filter(|p| p.exterior.len() >= 3)
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    }
}

/// The outlines the 2D planner plans for one region: each smoothed
/// polygon of the region, split into one outline per piece of its inset
/// by the tool radius (G-ADAPT3DPIECES, D2).
fn planner_outlines(
    region_polygon: &crate::polygon::Polygon2,
    params_2d: &crate::adaptive::AdaptiveParams,
) -> Vec<crate::polygon::Polygon2> {
    let smoothed_polygons = smoothed_region(region_polygon, params_2d);
    // G-ADAPT3DPIECES (D2): every smoothed polygon is planned, not only
    // the first, and each one whose inset by the tool radius splits is
    // planned piece by piece (`adaptive::inset_piece_outlines`, the one
    // split the 2D adapter also uses). One polygon of one piece gives the
    // polygon itself, so an unsplit region plans exactly as before.
    let polygons = if smoothed_polygons.is_empty() {
        vec![region_polygon.clone()]
    } else {
        smoothed_polygons
    };
    polygons
        .into_iter()
        .flat_map(|poly| {
            let pieces = crate::adaptive::inset_piece_outlines(&poly, params_2d.tool_radius);
            if pieces.is_empty() {
                vec![poly]
            } else {
                pieces
            }
        })
        .collect()
}

/// Geodesic Adaptive (research C38 §16): record a piece the arm leaves to
/// the job's rest contour pass (`why`: the F-038 forecast skipped it, or the field does
/// not apply), with one `info` line (its level, region and area), so that
/// the rest pass's coverage can be checked against it. The level's
/// waterline cleanup skips a contour inside it (`waterline_cleanup`).
fn leave_piece(
    ctx: &ClearZLevelContext<'_>,
    z_level: f64,
    region_idx: usize,
    piece: &crate::polygon::Polygon2,
    why: &str,
) {
    tracing::info!(
        z = z_level,
        region = region_idx + 1,
        area_mm2 = %format!("{:.1}", piece.area().abs()),
        why,
        "C38 piece left to the rest pass"
    );
    ctx.left_pieces.borrow_mut().push((z_level, piece.clone()));
}

/// Plan one outline: the geodesic plan (`adaptive::geodesic`), its residue
/// cleanup and the F-038 short-group filter, then the feed caps of its
/// turns and departures. A piece where the field does not apply is left to
/// the rest pass (`leave_piece`).
fn plan_one_outline(
    ctx: &ClearZLevelContext<'_>,
    params_2d: &crate::adaptive::AdaptiveParams,
    polygon_for_adaptive: &crate::polygon::Polygon2,
    (z_level, region_idx): (f64, usize),
    dropped_short_regions: &mut usize,
    cancel: &dyn CancelCheck,
) -> Result<(Vec<crate::adaptive::AdaptiveSegment>, Vec<Option<f64>>), Cancelled> {
    // The geodesic field plans the outline, with its planned clearing
    // turns. Research C38 §16 (lead ruling 2026-10-27): a piece where the
    // field does not apply (its seed of radius R does not fit) is edge
    // work. The arm plans nothing there and leaves the piece to the job's
    // rest pass; one `info` line per piece, so that the rest pass's coverage
    // can be checked against it.
    let Some(plan) =
        crate::adaptive::geodesic::slice_segments(polygon_for_adaptive, params_2d, cancel)?
    else {
        leave_piece(ctx, z_level, region_idx, polygon_for_adaptive, "no field");
        return Ok((Vec::new(), Vec::new()));
    };
    let (segs_2d_raw, turns) = (plan.segments, plan.turns);
    // The residue cleanup the 2D top-level entry point runs (boundary walks,
    // contour-parallel offsets, mop patches). Research C22: a plan with
    // clearing turns takes the cleanup whose replay admits them (the same
    // turn list and bound); with no turns the shipped cleanup runs.
    let segs_2d_raw = if turns.is_empty() {
        slice_cleanup(polygon_for_adaptive, params_2d, &segs_2d_raw)
    } else {
        crate::adaptive::geodesic::slice_cleanup(
            polygon_for_adaptive,
            params_2d,
            segs_2d_raw,
            &turns,
        )
    };
    // Research C12, C22: under the 2D pass load the replay admits a link
    // only within the load and stamps it on the planner grid, as a cut at
    // depth. The links stay `Link`s here; `clear_one_region` emits each one
    // as a 3D keep-down link at the level (`geodesic::HeldLinks`).

    // F-038: filter out tiny entry-plunge → cut groups before emission
    // (`filter_short_groups`).
    let segs_2d: Vec<crate::adaptive::AdaptiveSegment> = if ctx.min_region_cut_length_mm > 0.0 {
        let (kept_segs, dropped_first, group_total) =
            filter_short_groups(segs_2d_raw, ctx.min_region_cut_length_mm, params_2d);
        // Research C22a: the cleanup replayed its kept plan
        // (`replay_kept_segments`) BEFORE this filter, so a link or a cut
        // after a dropped group was admitted on that group's stamps. The
        // kept plan is replayed again, as the cleanup does after its own
        // filter, and filtered again, until no group drops: then the replay
        // has read the plan that goes out. Each round drops at least one
        // group, so the rounds are at most the groups there were.
        let mut kept = kept_segs;
        let mut dropped_round = dropped_first;
        let mut dropped_more = 0usize;
        let mut rounds = 0usize;
        while dropped_round > 0 && rounds < group_total {
            let replayed = crate::adaptive::geodesic::slice_replay(
                polygon_for_adaptive,
                params_2d,
                kept,
                &turns,
            );
            let (next, dropped, _) =
                filter_short_groups(replayed, ctx.min_region_cut_length_mm, params_2d);
            kept = next;
            dropped_round = dropped;
            dropped_more += dropped;
            rounds += 1;
        }
        let dropped_groups = dropped_first + dropped_more;
        if dropped_groups > 0 {
            debug!(
                z = z_level,
                region = region_idx + 1,
                dropped_groups,
                group_total,
                threshold_mm = ctx.min_region_cut_length_mm,
                "F-038: dropped short cut groups inside 2D adaptive output"
            );
            *dropped_short_regions += dropped_groups;
        }
        kept
    } else {
        segs_2d_raw
    };
    // Integration step 7.4, design part F: each planned turn and each
    // departure window of the plan that goes out is a cut of its own, with
    // the feed factor of its reading (`geodesic::feed_caps`).
    Ok(crate::adaptive::geodesic::feed_caps(
        polygon_for_adaptive,
        params_2d,
        segs_2d,
        &turns,
    ))
}

/// F-038: the groups of `segments` (a run from one `Rapid` to the next)
/// less each one that cuts under `min_cut_mm`; returns the kept segments,
/// the dropped groups and the groups there were.
///
/// The 2D adaptive emits its output as a sequence of segments shaped
/// like:
///
///   [Marker?, Rapid(entry), Cut(path), Cut(path), ..., Rapid(entry), ...]
///
/// Each `Rapid` is the start of a new "pass" — downstream this becomes
/// a retract + rapid-XY + peck-plunge + Cut sequence. On terrain-shaped
/// models the planner emits many of these passes, and a significant
/// fraction cut < 5 mm of material before the next retract. On the
/// Wanaka Back Rough .nc this manifests as 149 F750 entry plunges, 90
/// of which cut ≤ 10 mm. Each one spends 0.3–0.5 s on retract/plunge
/// overhead per 100 ms or less of real cutting.
///
/// Group segments by Rapid boundary; if a group's total Cut XY length
/// is below `min_region_cut_length_mm`, drop the whole group (the
/// entry Rapid + its Cuts). The cutter never visits that micro-area;
/// material is left for finishing passes (which is fine — these
/// micro-bridges are below roughing resolution).
///
/// Always keep the very first group (it provides the region's initial
/// tool position). Always keep the last group's residual Marker/Link
/// segments so the per-region debug spans stay coherent.
fn filter_short_groups(
    segs_2d_raw: Vec<crate::adaptive::AdaptiveSegment>,
    min_cut_mm: f64,
    params_2d: &crate::adaptive::AdaptiveParams,
) -> (Vec<crate::adaptive::AdaptiveSegment>, usize, usize) {
    let mut groups: Vec<Vec<crate::adaptive::AdaptiveSegment>> = Vec::new();
    let mut current: Vec<crate::adaptive::AdaptiveSegment> = Vec::new();
    for s in segs_2d_raw {
        if matches!(s, crate::adaptive::AdaptiveSegment::Rapid(_)) && !current.is_empty() {
            groups.push(std::mem::take(&mut current));
        }
        current.push(s);
    }
    if !current.is_empty() {
        groups.push(current);
    }
    let mut kept_segs: Vec<crate::adaptive::AdaptiveSegment> = Vec::new();
    let mut dropped_groups: usize = 0;
    let group_total = groups.len();
    for (gi, group) in groups.into_iter().enumerate() {
        let mut cut_len = 0.0_f64;
        for seg in &group {
            if let crate::adaptive::AdaptiveSegment::Cut(path_2d) = seg {
                cut_len += crate::geo::polyline_length_2d(path_2d);
            }
        }
        // Geodesic Adaptive (research C12/C22): the arm cuts each link (`HeldLinks`), so
        // a link's length counts as cut length.
        if params_2d.keep_down_links == crate::adaptive::KeepDownLinks::WithinPassLoad {
            cut_len += crate::adaptive::geodesic::link_length(&group);
        }
        // Always keep the first group — it sets up the region's
        // starting position. Drop interior + trailing groups
        // below threshold; the trailing group on terrain runs
        // is usually a sub-tool residual the finishing pass
        // will cover anyway.
        let drop = gi > 0
            && cut_len < min_cut_mm
            && group
                .iter()
                .any(|s| matches!(s, crate::adaptive::AdaptiveSegment::Rapid(_)));
        if drop {
            dropped_groups += 1;
            continue;
        }
        kept_segs.extend(group);
    }
    (kept_segs, dropped_groups, group_total)
}

/// The shipped cleanup of one outline's 2D plan: the post-process the 2D
/// top-level entry point runs, by `cleanup_strategy`.
fn slice_cleanup(
    polygon_for_adaptive: &crate::polygon::Polygon2,
    params_2d: &crate::adaptive::AdaptiveParams,
    segs_2d_raw: &[crate::adaptive::AdaptiveSegment],
) -> Vec<crate::adaptive::AdaptiveSegment> {
    match params_2d.cleanup_strategy {
        crate::adaptive::CleanupStrategy::Legacy => segs_2d_raw.to_vec(),
        crate::adaptive::CleanupStrategy::ResidueMop
        | crate::adaptive::CleanupStrategy::ContourParallelNarrow => {
            crate::adaptive::path::apply_residue_mop_cleanup(
                polygon_for_adaptive,
                params_2d,
                segs_2d_raw,
            )
        }
        crate::adaptive::CleanupStrategy::ContourParallelHybrid => {
            crate::adaptive::path::apply_contour_parallel_residue_cleanup(
                polygon_for_adaptive,
                params_2d,
                segs_2d_raw,
            )
        }
    }
}

/// The margin (mm) beyond the tool radius by which the perimeter sweep of
/// `clear_one_region` sits inside its region, so that the boundary clip
/// cannot flip its points to rapids (O5b/O5c; see the sweep's comment).
/// Geodesic Adaptive's grown 2D outline keeps the same margin (C31).
const PERIMETER_INSET_MARGIN_MM: f64 = 0.25;

/// Lift a 2D point of a level's plan to 3D: the level, raised over terrain
/// that stands above it (the ONE level floor, `intended_floor::level_floor_z`).
fn lift_to_level(
    ctx: &ClearZLevelContext<'_>,
    surface_hm: &SurfaceHeightmap,
    z_level: f64,
    p: P2,
) -> P3 {
    let surf_z = surface_hm.z_or_bbox_floor_at_world(p.x, p.y);
    let z = if surf_z == f64::NEG_INFINITY {
        z_level
    } else {
        super::intended_floor::level_floor_z(surf_z, ctx.stock_to_leave, z_level)
    };
    P3::new(p.x, p.y, z)
}

/// The 2D planner's outlines of a grown region (Geodesic Adaptive, research C31): each
/// grown polygon split into the pieces of its inset by the tool radius
/// (G-ADAPT3DPIECES, the one split `planner_outlines` uses), with no
/// smoothing.
fn grown_planner_outlines(
    grown: &[crate::polygon::Polygon2],
    tool_radius: f64,
) -> Vec<crate::polygon::Polygon2> {
    grown
        .iter()
        .flat_map(|poly| {
            let pieces = crate::adaptive::inset_piece_outlines(poly, tool_radius);
            if pieces.is_empty() {
                vec![poly.clone()]
            } else {
                pieces
            }
        })
        .collect()
}

/// Geodesic Adaptive (research C31): the region's smoothed outline grown by
/// `R - PERIMETER_INSET_MARGIN_MM`, as the 2D planner's material outline,
/// or `None` (the shipped path: the smoothed region and the perimeter
/// sweep).
///
/// The region is in tool-centre space at a wall (`build_material_bool_grid`
/// reads the drop-cutter floor), and the 2D planner insets its outline by
/// R. Given the smoothed region, its centres stay R plus the smoothing from
/// the region boundary, and the shipped slice cuts the band between with
/// the perimeter sweep, a full slot (C30: 1026 mm over the bound on Corne).
/// Given the smoothed region grown by `R - m`, the 2D tool-centre region is
/// the smoothed region inset by `m`, so the field and the 2D cleanup open
/// the band under the 2D pass load and no sweep is cut.
///
/// Why the smoothing comes first (D19, P3_C22 11.7): the region boundary
/// is the tool-centre limit only to the grid. Marching squares puts it at
/// the midpoint of an in and an out cell centre, so the true limit is
/// within half a planner cell of it; the lift reads the nearest heightmap
/// cell. Grown from the raw region, the closing also fills the staircase
/// notches, and centres at the 2D boundary read the out cells (the rim).
/// The smoothing (`SIMPLIFY_INSET`, then the RDP) moves the outline inside
/// and removes the staircase.
///
/// The region keeps the shipped path, with one `info` line that says why,
/// when:
/// - the smoothing gives no outline, or the grow gives none;
/// - the inside margin `SIMPLIFY_INSET + m` is under one planner cell
///   (`cell`): the reach of an out cell's nearest read, half a cell, plus
///   the boundary error, up to half a cell (the lead's condition);
/// - the inset outline splits into more polygons (the shortfall inset,
///   below, cut a neck);
/// - the measured margin, the least distance between the smoothed and the
///   raw boundaries plus `m`, is under one planner cell (the same reach as
///   the nominal condition) after the shortfall inset. The RDP can move
///   the smoothed outline back out by up to its tolerance, so the nominal
///   inset is not a bound. Where the measured margin is short, the smoothed
///   outline is first inset by the shortfall (`cell - measured`), once,
///   region-wide, and measured again;
/// - the closing guard: growing by `R - m` and insetting by R closes any
///   gap narrower than about 2R. The 2D tool-centre region (the grown
///   outline inset by R) may exceed the smoothed region inset by `m` by at
///   most one planner cell times the smoothed boundary length.
///
/// No new constant: R, `m`, the two smoothing values and the planner cell.
fn grown_outlines(
    ctx: &ClearZLevelContext<'_>,
    region_polygon: &crate::polygon::Polygon2,
    params_2d: &crate::adaptive::AdaptiveParams,
    cell: f64,
    (z_level, region_idx): (f64, usize),
) -> Option<Vec<crate::polygon::Polygon2>> {
    let r = ctx.tool_radius;
    let m = PERIMETER_INSET_MARGIN_MM;
    let separation = |polys: &[crate::polygon::Polygon2]| {
        polys
            .iter()
            .map(|poly| boundary_separation(poly, region_polygon))
            .fold(f64::INFINITY, f64::min)
    };
    let smoothed = smoothed_region(region_polygon, params_2d);
    // The shortfall inset: where the RDP has brought the smoothed outline
    // back within `cell - m` of the raw boundary, inset it by the
    // shortfall, so that every point of it stands at least `cell - m`
    // inside the raw region.
    let measured_before = separation(smoothed.as_slice()) + m;
    // The offset flattens its arc joins on the input side of the true
    // offset, up to `FlattenPolicy::UNTOLERANCED_MM` (polygon.rs, P6.0): an
    // inset can stand that much less inside than asked. So the inset adds
    // that budget, else `measured_after` lands a few micrometres under the
    // cell (D19b).
    let shortfall_mm = if measured_before < cell {
        cell - measured_before + crate::polygon::FlattenPolicy::UNTOLERANCED_MM
    } else {
        0.0
    };
    let pieces_before = smoothed.len();
    let smoothed: Vec<crate::polygon::Polygon2> = if shortfall_mm > 0.0 {
        smoothed
            .iter()
            .flat_map(|poly| crate::polygon::offset_polygon(poly, shortfall_mm))
            .collect()
    } else {
        smoothed
    };
    let inset_split = smoothed.len() > pieces_before;
    let measured_after = separation(smoothed.as_slice()) + m;
    let grown: Vec<crate::polygon::Polygon2> = smoothed
        .iter()
        .flat_map(|poly| {
            crate::polygon::OffsetRingSet::from_polygon(poly)
                .offset(-(r - m))
                .to_polygons(crate::polygon::FlattenPolicy::untoleranced())
        })
        .collect();
    let centre_area: f64 = grown
        .iter()
        .flat_map(|g| crate::polygon::offset_polygon(g, r))
        .map(|p| p.area())
        .sum();
    let target_area: f64 = smoothed
        .iter()
        .flat_map(|poly| crate::polygon::offset_polygon(poly, m))
        .map(|p| p.area())
        .sum();
    let boundary_mm: f64 = smoothed
        .iter()
        .flat_map(polygon_rings)
        .map(ring_length)
        .sum();
    let allowed_mm2 = cell * boundary_mm;
    let excess_mm2 = centre_area - target_area;
    let nominal_margin = SIMPLIFY_INSET + m;
    let decision = if smoothed.is_empty() || grown.is_empty() {
        "no smoothed or grown outline: shipped sweep"
    } else if inset_split {
        "the shortfall inset split the outline: shipped sweep"
    } else if nominal_margin < cell {
        "inside margin under one planner cell: shipped sweep"
    } else if measured_after < cell {
        "measured margin under one planner cell: shipped sweep"
    } else if excess_mm2 > allowed_mm2 {
        "closing guard: shipped sweep"
    } else {
        "grown, no sweep"
    };
    let keep_grown = decision == "grown, no sweep";
    tracing::info!(
        z = z_level,
        region = region_idx + 1,
        cell_mm = cell,
        nominal_margin_mm = %format!("{nominal_margin:.3}"),
        measured_before_mm = %format!("{measured_before:.3}"),
        shortfall_inset_mm = %format!("{shortfall_mm:.3}"),
        measured_after_mm = %format!("{measured_after:.3}"),
        region_area_mm2 = %format!("{:.1}", region_polygon.area()),
        centre_area_mm2 = %format!("{centre_area:.1}"),
        target_area_mm2 = %format!("{target_area:.1}"),
        excess_mm2 = %format!("{excess_mm2:.2}"),
        allowed_mm2 = %format!("{allowed_mm2:.2}"),
        decision,
        "C31 region opening"
    );
    keep_grown.then_some(grown)
}

/// The rings of a polygon: its exterior, then its holes.
pub(super) fn polygon_rings(poly: &crate::polygon::Polygon2) -> impl Iterator<Item = &[P2]> {
    std::iter::once(poly.exterior.as_slice()).chain(poly.holes.iter().map(Vec::as_slice))
}

/// The length (mm) of a closed ring.
fn ring_length(ring: &[P2]) -> f64 {
    ring.iter()
        .zip(ring.iter().cycle().skip(1))
        .map(|(a, b)| (b.x - a.x).hypot(b.y - a.y))
        .sum()
}

/// The least distance (mm) between the boundaries of `a` and `b` (every
/// ring of each, closed). Two boundaries that do not cross come nearest at
/// a vertex of one of them, so each vertex of each is read against every
/// segment of the other.
fn boundary_separation(a: &crate::polygon::Polygon2, b: &crate::polygon::Polygon2) -> f64 {
    let one_way = |from: &crate::polygon::Polygon2, to: &crate::polygon::Polygon2| {
        polygon_rings(from)
            .flatten()
            .map(|p| {
                polygon_rings(to)
                    .flat_map(|ring| ring.iter().zip(ring.iter().cycle().skip(1)))
                    .map(|(s, e)| crate::geo::point_to_segment_distance(p, s, e))
                    .fold(f64::INFINITY, f64::min)
            })
            .fold(f64::INFINITY, f64::min)
    };
    one_way(a, b).min(one_way(b, a))
}

/// Stage 2 — clear ONE region: the cut-length forecast, the perimeter
/// sweep, the 2D adaptive walk and the residual cleanup, each lifted to 3D
/// and stamped into the planner's dexel state.
///
/// Each region's first 2D segment is a Rapid (the entry into that region),
/// which becomes a 3D Rapid: retract to safe_z, traverse XY, plunge. Between
/// disjoint regions that retract is correct; the 2D adaptive cannot link
/// across regions because each call sees only its own region's polygon.
#[expect(
    clippy::indexing_slicing,
    reason = "every `path_2d[i]` and `pts[i]` below indexes a vector this function just built, at an index its own `len()` guard admits."
)]
fn clear_one_region(
    ctx: &ClearZLevelContext<'_>,
    plan: &RegionClearPlan<'_>,
    emission: &mut LevelEmission<'_>,
    region_polygon: &crate::polygon::Polygon2,
    region_idx: usize,
    cancel: &dyn CancelCheck,
) -> Result<(), Cancelled> {
    let sink = &mut emission.sink;
    let level_metrics = &mut *emission.level_metrics;
    let cut_count = &mut *emission.cut_count;
    let dropped_short_regions = &mut *emission.dropped_short_regions;
    let surface_hm = plan.surface_hm;
    let z_level = plan.z_level;
    let params_2d = plan.params_2d;
    let level_ctx = plan.level_ctx;
    let region_total = plan.region_total;

    // 5. Lift 2D points to 3D, respecting terrain peaks above z_level.
    let lift = |p: P2| lift_to_level(ctx, surface_hm, z_level, p);

    let region_scope = level_ctx.as_ref().map(|debug_ctx| {
        debug_ctx.start_span(
            DebugSpanKind::Agent2dRegion,
            format!(
                "region {}/{} @ Z {:.3}",
                region_idx + 1,
                region_total,
                z_level
            ),
        )
    });

    // F-038: forecast the region's total cutting length BEFORE emitting
    // any entry/perimeter/adaptive segments. If the forecast is below
    // `min_region_cut_length_mm`, skip the region entirely — the entry
    // plunge + tiny cut + retract would otherwise spend more cycle time
    // on travel than on material removal. This is the dominant cause of
    // the Wanaka Back Rough "perimeter micro-plunge" symptom (149 entry
    // plunges, 90 of which cut ≤ 10 mm before retract). Forecasting is
    // a dry-run: the 2D adaptive call below does NOT mutate the dexel
    // stock — it operates on its own internal material grid — so we can
    // call it twice (here, and again after the perimeter sweep is
    // emitted) at no risk of state divergence. We accept the doubled
    // 2D-adaptive cost as the price for the up-front decision.
    //
    // C38 §16: Geodesic Adaptive takes this forecast too. It clears the
    // body only, and the strips the forecast reads as 0 go to the job's
    // edge contour pass.
    if ctx.min_region_cut_length_mm > 0.0 {
        const PERIMETER_INSET_MARGIN_MM_FORECAST: f64 = 0.25;
        let inset_polygons_forecast = crate::polygon::offset_polygon(
            region_polygon,
            ctx.tool_radius + PERIMETER_INSET_MARGIN_MM_FORECAST,
        );
        let mut forecast_mm: f64 = 0.0;
        for inset in &inset_polygons_forecast {
            if inset.exterior.len() >= 3 {
                let mut path_2d = inset.exterior.clone();
                if path_2d.len() >= 2
                    && (path_2d[0].x - path_2d[path_2d.len() - 1].x).abs() < 1e-9
                    && (path_2d[0].y - path_2d[path_2d.len() - 1].y).abs() < 1e-9
                {
                    path_2d.pop();
                }
                forecast_mm += crate::geo::polyline_length_2d(&path_2d);
            }
            for hole in &inset.holes {
                if hole.len() < 3 {
                    continue;
                }
                let mut path_2d = hole.clone();
                if path_2d.len() >= 2
                    && (path_2d[0].x - path_2d[path_2d.len() - 1].x).abs() < 1e-9
                    && (path_2d[0].y - path_2d[path_2d.len() - 1].y).abs() < 1e-9
                {
                    path_2d.pop();
                }
                forecast_mm += crate::geo::polyline_length_2d(&path_2d);
            }
        }
        // Quick exit: if even the perimeter alone clears the bar, skip
        // the (more expensive) 2D adaptive forecast.
        if forecast_mm < ctx.min_region_cut_length_mm {
            let segs_forecast = crate::adaptive::adaptive_segments_with_debug(
                region_polygon,
                params_2d,
                cancel,
                None,
                None,
            )?;
            for seg in &segs_forecast {
                if let crate::adaptive::AdaptiveSegment::Cut(path_2d) = seg {
                    forecast_mm += crate::geo::polyline_length_2d(path_2d);
                    if forecast_mm >= ctx.min_region_cut_length_mm {
                        break;
                    }
                }
            }
        }
        if forecast_mm < ctx.min_region_cut_length_mm {
            debug!(
                z = z_level,
                region = region_idx + 1,
                region_total,
                forecast_mm,
                threshold_mm = ctx.min_region_cut_length_mm,
                area_mm2 = region_polygon.area().abs(),
                "F-038: skipping micro-region (forecast cut length below threshold)"
            );
            // Research C38 §16: the arm leaves this region to the rest
            // contour pass, as a piece where the field does not apply.
            leave_piece(ctx, z_level, region_idx, region_polygon, "F-038 forecast");
            *dropped_short_regions += 1;
            if let Some(scope) = region_scope.as_ref() {
                scope.set_counter("skipped_f038", 1.0);
                scope.set_counter("forecast_cut_length_mm", forecast_mm);
            }
            return Ok(());
        }
    }

    // Perimeter sweep: trace the polygon boundary inset by tool_radius.
    //
    // The 2D adaptive insets the polygon by `tool_radius` to compute
    // its machinable region, then walks an "agent search" path inside.
    // That walk doesn't reliably sweep the full machinable boundary —
    // its spiral starts at an entry and works inward, leaving cells
    // in the outermost band (within tool_radius of polygon boundary)
    // touched only sporadically. Successive z-levels miss the same
    // cells, leaving stock that gets cleared at the deepest pass with
    // a single sample's axial DOC equal to the full uncleared depth
    // (~18mm on a 25mm stock).
    //
    // Pre-emit a Cut segment that follows the inset boundary for each
    // outer loop (and each hole). Cutter walks this contour at z_level,
    // stamping cells within tool_radius of the contour — fully covering
    // the polygon's outer band before the 2D adaptive starts. Same
    // dexel update strategy as the 2D adaptive's Cut segments.
    // Inset by tool_radius + a small margin so the perimeter sweep
    // sits SAFELY INSIDE the effective_boundary (silhouette inset by
    // tool_radius for containment=Inside). Without the margin, the
    // perimeter sits ON the effective_boundary edge and the post-
    // generation `clip_toolpath_to_boundary` flips its `contains_point`
    // result on edge-case points, converting these cuts to rapids and
    // leaving the boundary band unstamped — which is exactly the
    // wanaka 18mm-DOC failure mode (O5b/O5c).
    //
    // Geodesic Adaptive (research C31): when the 2D planner gets the region grown by
    // `R - PERIMETER_INSET_MARGIN_MM` (`grown_outlines`), its own
    // plan opens this band under the 2D pass load, and no sweep is cut.
    let grown = grown_outlines(
        ctx,
        region_polygon,
        params_2d,
        sink.material_stock.z_grid.cell_size,
        (z_level, region_idx),
    )
    // Geodesic Adaptive (research C38 item B): the grow stops at the stock box, where
    // no material stands beyond the edge.
    .and_then(|grown| super::outer_band::clip_to_stock_box(grown, &*sink.material_stock))
    // C38 §13.7: and at the reach of the boundary clip, so that nothing is
    // planned where the clip would drop it.
    .and_then(|grown| {
        super::outer_band::clip_to_centre_reach(
            grown,
            ctx.centre_clip,
            ctx.tool_radius,
            PERIMETER_INSET_MARGIN_MM,
        )
    });
    let inset_polygons = if grown.is_some() {
        Vec::new()
    } else {
        crate::polygon::offset_polygon(region_polygon, ctx.tool_radius + PERIMETER_INSET_MARGIN_MM)
    };
    for inset in &inset_polygons {
        if inset.exterior.len() >= 3 {
            // simplify_path_3d (RDP) early-returns on zero-length
            // baselines (first == last), which collapses any closed
            // loop to two coincident points → no-op feed. cavalier-
            // contours' parallel_offset output INCLUDES the closing
            // duplicate vertex, so we must drop it explicitly to
            // keep the path open for RDP. Without this drop, the
            // entire perimeter sweep silently became a no-op.
            let mut path_2d = inset.exterior.clone();
            if path_2d.len() >= 2
                && (path_2d[0].x - path_2d[path_2d.len() - 1].x).abs() < 1e-9
                && (path_2d[0].y - path_2d[path_2d.len() - 1].y).abs() < 1e-9
            {
                path_2d.pop();
            }
            let path_3d: Vec<P3> = path_2d.iter().map(|&p| lift(p)).collect();
            if let Some(first) = path_3d.first().copied() {
                // `LevelSink::emit` plans the entry (`plan_entry`) when a
                // cut commits it, BEFORE it stamps it, so the floor
                // reflects material state from committed segments only.
                let entry_seg = Adaptive3dSegment::Rapid(first);
                sink.emit(ctx, entry_seg);
            }
            if path_3d.len() >= 2 {
                level_metrics.perimeter_sweep_length_mm += crate::geo::polyline_length(&path_3d);
                sink.emit(ctx, Adaptive3dSegment::Cut(path_3d));
                *cut_count += 1;
            }
        }
        // Sweep each hole's boundary too — cutter must clear material
        // around each hole's perimeter, not just the outer.
        for hole in &inset.holes {
            if hole.len() < 3 {
                continue;
            }
            // Same closing-duplicate handling as the exterior sweep:
            // drop it if present to keep simplify_path_3d's RDP working.
            let mut path_2d = hole.clone();
            if path_2d.len() >= 2
                && (path_2d[0].x - path_2d[path_2d.len() - 1].x).abs() < 1e-9
                && (path_2d[0].y - path_2d[path_2d.len() - 1].y).abs() < 1e-9
            {
                path_2d.pop();
            }
            let path_3d: Vec<P3> = path_2d.iter().map(|&p| lift(p)).collect();
            if let Some(first) = path_3d.first().copied() {
                // `LevelSink::emit` plans the entry (`plan_entry`) on commit.
                let entry_seg = Adaptive3dSegment::Rapid(first);
                sink.emit(ctx, entry_seg);
            }
            if path_3d.len() >= 2 {
                level_metrics.perimeter_sweep_length_mm += crate::geo::polyline_length(&path_3d);
                sink.emit(ctx, Adaptive3dSegment::Cut(path_3d));
                *cut_count += 1;
            }
        }
    }

    // G-ADAPT3DPIECES (D2): the 2D plan of the region and its emission.
    // The outlines are the region's own (`planner_outlines`), or (research
    // arm C31) the grown region split into its pieces.
    let outlines = match grown {
        Some(grown) => grown_planner_outlines(&grown, ctx.tool_radius),
        None => planner_outlines(region_polygon, params_2d),
    };
    plan_and_emit_region(ctx, plan, emission, &outlines, region_idx, cancel)
}

/// The region body after the perimeter sweep: the geodesic plan of each of
/// the region's `outlines` (`plan_one_outline`), then its lift into 3D
/// through the level's sink, with the stamping and the held links.
#[expect(
    clippy::indexing_slicing,
    reason = "every `path_2d[i]` and `pts[i]` below indexes a vector this function just built, at an index its own `len()` guard admits."
)]
fn plan_and_emit_region(
    ctx: &ClearZLevelContext<'_>,
    plan: &RegionClearPlan<'_>,
    emission: &mut LevelEmission<'_>,
    outlines: &[crate::polygon::Polygon2],
    region_idx: usize,
    cancel: &dyn CancelCheck,
) -> Result<(), Cancelled> {
    let sink = &mut emission.sink;
    let level_metrics = &mut *emission.level_metrics;
    let cut_count = &mut *emission.cut_count;
    let dropped_short_regions = &mut *emission.dropped_short_regions;
    let surface_hm = plan.surface_hm;
    let z_level = plan.z_level;
    let params_2d = plan.params_2d;
    let lift = |p: P2| lift_to_level(ctx, surface_hm, z_level, p);

    // G-ADAPT3DPIECES (D2): the planner plans one piece of the inset of
    // the polygon it gets, so the region is planned outline by outline.
    let mut segs_2d: Vec<crate::adaptive::AdaptiveSegment> = Vec::new();
    // Geodesic Adaptive: the feed factor of each 2D segment (`None`: no cap).
    let mut caps_2d: Vec<Option<f64>> = Vec::new();
    for outline in outlines {
        let (segs, caps) = plan_one_outline(
            ctx,
            params_2d,
            outline,
            (z_level, region_idx),
            dropped_short_regions,
            cancel,
        )?;
        segs_2d.extend(segs);
        caps_2d.extend(caps);
    }

    // Geodesic Adaptive (research C22, C26): each 2D keep-down link after a
    // known position is held at depth, as the 2D plan has it
    // (`geodesic::HeldLinks`).
    // C26b: it goes out as one 3D keep-down link (a feed tagged `Linking`,
    // stamped as every link is) only when every lifted point of it is at
    // the level. A link that crosses terrain above the level would climb
    // the wall in one draped move, so it is lifted as a cut held at depth
    // (C22): the large-Z split below retracts it at the wall.
    let mut held_links = (params_2d.keep_down_links
        == crate::adaptive::KeepDownLinks::WithinPassLoad)
        .then(|| crate::adaptive::geodesic::HeldLinks::new(params_2d));
    // Geodesic Adaptive (design part F): a capped 2D cut goes out between a
    // `FeedCapStart` and a `FeedCapEnd` marker, and the emitter writes the
    // cap on the moves of the 3D cuts between them (`segments_to_toolpath`).
    let mut cap_open = false;
    for (seg, cap) in segs_2d.into_iter().zip(caps_2d) {
        if std::mem::take(&mut cap_open) {
            sink.emit(
                ctx,
                Adaptive3dSegment::Marker(super::Adaptive3dRuntimeEvent::FeedCapEnd),
            );
        }
        if let Some(factor) = cap {
            sink.emit(
                ctx,
                Adaptive3dSegment::Marker(super::Adaptive3dRuntimeEvent::FeedCapStart { factor }),
            );
            cap_open = true;
        }
        let (seg, held) = match held_links.as_mut() {
            Some(links) => match links.next(seg) {
                (_, Some(path_2d)) => {
                    let lifted: Vec<P3> = path_2d.iter().map(|&p| lift(p)).collect();
                    if lifted.iter().all(|p| p.z <= z_level) {
                        if let Some(&end) = lifted.last() {
                            // C38 §14: the keep-down proof on the swept
                            // disc (`keep_down_excess`). A held link whose
                            // disc meets a column above its slab goes out
                            // as an entry instead.
                            let from = sink.cursor.tool_pos.or(lifted.first().copied());
                            let refused = from.is_some_and(|from| {
                                super::keep_down::keep_down_excess(
                                    sink.material_stock,
                                    from,
                                    end,
                                    ctx.tool_radius,
                                    z_level + ctx.depth_per_pass + ctx.stay_down_clearance_mm,
                                )
                                .is_some()
                            });
                            if refused {
                                sink.emit(ctx, Adaptive3dSegment::Rapid(end));
                            } else {
                                sink.emit(ctx, Adaptive3dSegment::Link(end));
                            }
                        }
                        continue;
                    }
                    (crate::adaptive::AdaptiveSegment::Cut(path_2d), true)
                }
                (seg, None) => (seg, false),
            },
            None => (seg, false),
        };
        match seg {
            crate::adaptive::AdaptiveSegment::Cut(path_2d) => {
                if path_2d.is_empty() {
                    continue;
                }
                let path_3d: Vec<P3> = path_2d.iter().map(|&p| lift(p)).collect();
                level_metrics.agent_walk_cut_length_mm += crate::geo::polyline_length(&path_3d);
                // Per-point classification (BEFORE any stamping):
                // cutter is "engaged" if the current dexel ray top at
                // this XY is above the cutter Z. "Air" points get
                // demoted to rapids — and crucially we DON'T stamp
                // them, so the dexel state stays consistent with what
                // the actual machine would produce (rapids don't cut).
                // Without this, generation-time dexel diverges from
                // simulator-time dexel: the planner thinks it cleared
                // cells the actual G-code never cuts, leaving
                // visible streaks of uncut material in the sim.
                const AIR_THRESHOLD_MM: f64 = 0.2;
                // Geodesic Adaptive (research C12, C18): a point is engaged when stock
                // stands above it anywhere under the cutter disc. The
                // centre read calls a cut air where its centre runs over
                // cleared stock and its disc still cuts (the starter spiral
                // inside its plunge hole); the demoted run then leaves stock
                // that the next cut meets at 0.75 of D.
                let mut engaged: Vec<bool> = path_3d
                    .iter()
                    .map(|p| {
                        sink.material_stock
                            .max_top_z_in_disc(p.x, p.y, ctx.tool_radius)
                            .is_some_and(|z| z > p.z + AIR_THRESHOLD_MM)
                    })
                    .collect();
                // Geodesic Adaptive (research C22/C26b): a held link that crosses terrain
                // above the level is a feed through the band the 2D plan has
                // cut; it is never "air". The large-Z split still applies.
                if held {
                    engaged.fill(true);
                }
                // Re-promote short air runs back to engaged: rapid-mode
                // is only faster than feed-mode above a crossover
                // length, because each demotion carries a full
                // retract + rapid-reposition + re-plunge overhead.
                // Computed from the op's actual feed/plunge rates and
                // this level's retract depth (replaces a 70 mm
                // constant tuned to one 6 mm/3150/5000 combo).
                // Threshold is XY toolpath distance, so we walk
                // forward summing segment lengths until we exceed it
                // or change classification.
                let min_air_run_mm = air_run_crossover_mm(
                    ctx.feed_rate,
                    ctx.plunge_rate,
                    (ctx.safe_z - z_level).max(0.0),
                );
                if !engaged.is_empty() {
                    let mut i = 0;
                    while i < engaged.len() {
                        if engaged[i] {
                            i += 1;
                            continue;
                        }
                        // Find end of this air run.
                        let run_start = i;
                        let mut run_end = i;
                        let mut run_len_mm = 0.0_f64;
                        while run_end + 1 < engaged.len() && !engaged[run_end + 1] {
                            let dx = path_3d[run_end + 1].x - path_3d[run_end].x;
                            let dy = path_3d[run_end + 1].y - path_3d[run_end].y;
                            run_len_mm += (dx * dx + dy * dy).sqrt();
                            run_end += 1;
                        }
                        // Add the entry transition length too (from
                        // last engaged point to first air point) so a
                        // tiny air "wedge" between two engaged points
                        // also counts.
                        if run_start > 0 {
                            let dx = path_3d[run_start].x - path_3d[run_start - 1].x;
                            let dy = path_3d[run_start].y - path_3d[run_start - 1].y;
                            run_len_mm += (dx * dx + dy * dy).sqrt();
                        }
                        if run_len_mm < min_air_run_mm {
                            for is_engaged in engaged.iter_mut().take(run_end + 1).skip(run_start) {
                                *is_engaged = true;
                            }
                        }
                        i = run_end + 1;
                    }
                }
                // F-038: complementary pass — demote SHORT engaged runs
                // to air. The 2D adaptive's path crosses cleared territory
                // many times on terrain-shaped models (Wanaka rough: 149
                // entry plunges, 90 of which cut <= 10mm before retract).
                // Each tiny engaged run gets a full retract+plunge+cut+
                // retract cycle that's almost all overhead. Below this
                // threshold the cut is roughing-irrelevant — finishing
                // passes clean the residual.
                //
                // Asymmetric with min_air_run_mm on purpose: the air-run
                // smoother promotes air → engaged to avoid retract
                // overhead on short bridges; this engaged-run filter
                // demotes engaged → air to merge the bordering air runs
                // into one rapid. Two opposite-direction filters that
                // together select for "long engaged runs separated by
                // long air runs".
                // G-PHANTOMSTAMP: no cursor write here. The planner position
                // moves only when a segment is stamped; setting it to the
                // path end gave an air-first path's entry a keep-down proof
                // from a point the tool never reached.
                // Split the lifted path at:
                //   - large Z transitions (existing safety: peak→valley
                //     bridges that drag the cutter diagonally through
                //     uncut stock, see PLUNGE_SLOPE_LIMIT below);
                //   - engagement transitions (NEW: air→engaged or vice
                //     versa demarcates a real cut from a transit run).
                //
                // 1mm floor: at small depth_per_pass (e.g. 0.5mm) the
                // 1.1× factor gives a 0.55mm threshold, and any natural
                // terrain undulation > 0.55mm between consecutive
                // samples chops the path. The slice then plans many
                // short paths instead of one long sweep, and the
                // linker may not stitch them all up. The floor
                // preserves the safety logic without over-splitting.
                const PLUNGE_SLOPE_LIMIT: f64 = 0.3;
                const Z_DROP_FLOOR_MM: f64 = 1.0;
                let z_drop_threshold = (ctx.depth_per_pass * 1.1).max(Z_DROP_FLOOR_MM);
                let mut sub_start = 0usize;
                let mut sub_engaged = engaged.first().copied().unwrap_or(true);
                for i in 1..path_3d.len() {
                    let prev = path_3d[i - 1];
                    let curr = path_3d[i];
                    let dz = curr.z - prev.z;
                    let dx = curr.x - prev.x;
                    let dy = curr.y - prev.y;
                    let xy_dist = (dx * dx + dy * dy).sqrt();
                    let abs_slope = if xy_dist > 1e-6 {
                        dz.abs() / xy_dist
                    } else if dz.abs() > 1e-6 {
                        f64::INFINITY
                    } else {
                        0.0
                    };
                    let shape_split = dz.abs() > z_drop_threshold || abs_slope > PLUNGE_SLOPE_LIMIT;
                    // StepSplit::StockDepth (D43): a step the path rule flags
                    // is cut at feed unless the tool's swept disc meets stock
                    // deeper than one pass (`step_axial_depth`), or its
                    // chip-area load (`step_load`, the one load) passes the
                    // pass ceiling c (the radial fraction of
                    // `PassLoad::swept_width`). A thin layer reads light; a
                    // full-depth drop into fresh stock reads near a slot.
                    let engagement_change = engaged[i] != sub_engaged;
                    // D65: the feed factor of a load split admitted as a
                    // capped cut (`None`: split as before, or no split).
                    let mut capped_step: Option<f64> = None;
                    let large_z = match ctx.step_split {
                        super::StepSplit::PathShape => shape_split,
                        super::StepSplit::StockDepth if shape_split => {
                            let ap =
                                step_axial_depth(sink.material_stock, prev, curr, ctx.tool_radius);
                            let deep = ap > ctx.depth_per_pass + ctx.stay_down_clearance_mm;
                            let ceiling =
                                crate::ops::adaptive_shared::radial_woc_fraction_from_leading_arc(
                                    crate::adaptive::pass_engagement_limit(
                                        ctx.stepover,
                                        ctx.tool_radius,
                                    ),
                                );
                            let load = step_load(
                                sink.material_stock,
                                prev,
                                curr,
                                ctx.tool_radius,
                                ctx.stay_down_clearance_mm,
                                &StepLoadCutter {
                                    lut: ctx.lut,
                                    chip: ctx.cutter.geometry_hint(),
                                    diameter: 2.0 * ctx.tool_radius,
                                    depth_per_pass: ctx.depth_per_pass,
                                },
                            );
                            // D65 (Geodesic Adaptive only): a load split that
                            // is not deep and reads at most a slot is cut at
                            // a capped feed, not split into an entry.
                            if admits_capped_split(
                                deep,
                                load,
                                ceiling,
                                !engagement_change && sub_engaged,
                            ) {
                                capped_step = Some(capped_step_factor(
                                    ceiling,
                                    load,
                                    prev,
                                    curr,
                                    ctx.feed_rate,
                                    ctx.plunge_rate,
                                    cap,
                                ));
                                false
                            } else {
                                deep || load > ceiling
                            }
                        }
                        super::StepSplit::StockDepth => false,
                    };
                    if let Some(factor) = capped_step {
                        // The run so far, then the step on its own between
                        // cap markers, then the open 2D cap again.
                        if i - sub_start >= 2 {
                            sink.emit(ctx, Adaptive3dSegment::Cut(path_3d[sub_start..i].to_vec()));
                            *cut_count += 1;
                        }
                        if cap.is_some() {
                            sink.emit(
                                ctx,
                                Adaptive3dSegment::Marker(
                                    super::Adaptive3dRuntimeEvent::FeedCapEnd,
                                ),
                            );
                        }
                        sink.emit(
                            ctx,
                            Adaptive3dSegment::Marker(
                                super::Adaptive3dRuntimeEvent::FeedCapStart { factor },
                            ),
                        );
                        sink.emit(ctx, Adaptive3dSegment::Cut(vec![prev, curr]));
                        *cut_count += 1;
                        sink.emit(
                            ctx,
                            Adaptive3dSegment::Marker(super::Adaptive3dRuntimeEvent::FeedCapEnd),
                        );
                        if let Some(outer) = cap {
                            sink.emit(
                                ctx,
                                Adaptive3dSegment::Marker(
                                    super::Adaptive3dRuntimeEvent::FeedCapStart { factor: outer },
                                ),
                            );
                        }
                        sub_start = i;
                        continue;
                    }
                    if large_z || engagement_change {
                        // Flush the current run.
                        let run_len = i - sub_start;
                        if run_len >= 1 {
                            if sub_engaged {
                                if run_len >= 2 {
                                    sink.emit(
                                        ctx,
                                        Adaptive3dSegment::Cut(path_3d[sub_start..i].to_vec()),
                                    );
                                    *cut_count += 1;
                                }
                            } else {
                                // Air run → single RapidWithFloor to
                                // the end of the run (the cutter just
                                // traverses cleared territory). No
                                // stamping (rapids don't cut).
                                let end_pt = path_3d[i.saturating_sub(1)];
                                // `LevelSink::emit` plans the entry (`plan_entry`) on commit.
                                let entry_seg = Adaptive3dSegment::Rapid(end_pt);
                                sink.emit(ctx, entry_seg);
                            }
                        }
                        // After a large_z split, also rapid-position
                        // to the new point so the cutter retracts
                        // before continuing.
                        if large_z {
                            let p3 = path_3d[i];
                            // `LevelSink::emit` plans the entry (`plan_entry`) on commit.
                            let entry_seg = Adaptive3dSegment::Rapid(p3);
                            sink.emit(ctx, entry_seg);
                        }
                        sub_start = i;
                        sub_engaged = engaged[i];
                    }
                }
                // Flush final run.
                let run_len = path_3d.len() - sub_start;
                if run_len >= 1 {
                    if sub_engaged {
                        if run_len >= 2 {
                            sink.emit(ctx, Adaptive3dSegment::Cut(path_3d[sub_start..].to_vec()));
                            *cut_count += 1;
                        }
                    } else if let Some(end_pt) = path_3d.last().copied() {
                        // `LevelSink::emit` plans the entry (`plan_entry`) on commit.
                        let entry_seg = Adaptive3dSegment::Rapid(end_pt);
                        sink.emit(ctx, entry_seg);
                    }
                }
            }
            crate::adaptive::AdaptiveSegment::Rapid(p) => {
                let p3 = lift(p);
                // Sample current stock_top at the entry XY. When the
                // previous Z-level cleared above this XY, the dexel
                // column has a low top and the peck-plunge from
                // safe_z down would burn time feeding through air.
                // Pass the sampled top to the path emitter so it
                // can rapid through the air gap before pecking.
                // `LevelSink::emit` plans the entry (`plan_entry`) on commit.
                let entry_seg = Adaptive3dSegment::Rapid(p3);
                sink.emit(ctx, entry_seg);
            }
            crate::adaptive::AdaptiveSegment::Link(p) => {
                // 2D Link = feed at cut depth assuming the path is
                // clear in the 2D material grid. In 3D, the linear path
                // between two link endpoints can collide with terrain
                // peaks that rise between them, even though both
                // endpoints are at safe Z. Treat as Rapid (retract to
                // safe_z) to guarantee no material collision. Same
                // rapid-floor optimisation as the Rapid case.
                let p3 = lift(p);
                // `LevelSink::emit` plans the entry (`plan_entry`) on commit.
                let entry_seg = Adaptive3dSegment::Rapid(p3);
                sink.emit(ctx, entry_seg);
            }
            crate::adaptive::AdaptiveSegment::Marker(_) => {
                // 2D runtime events don't translate cleanly to 3D; swallow.
                // The debug trace captured them already under agent2d_region.
            }
        }
    }
    if cap_open {
        sink.emit(
            ctx,
            Adaptive3dSegment::Marker(super::Adaptive3dRuntimeEvent::FeedCapEnd),
        );
    }

    Ok(())
}

/// Stage 3 — report the entries the level replaced, and check that no
/// back-to-back entry is left.
///
/// F-038: the region loop above yields one entry per 2D-adaptive entry plus
/// one for each engagement transition the lifter detects, and some of them
/// are followed by another entry with no cut between (the engagement
/// subdivider demoted the whole following cut to air, a wall climb split
/// every step, or the entry landed on cleared stock). Each would cost a
/// full retract + rapid + peck-plunge that cuts nothing.
///
/// G-PHANTOMSTAMP: `LevelSink` now drops such an entry before it is
/// stamped (`replaced_entries`), so nothing is left to coalesce here. This
/// used to delete them from the finished level, after the planner had
/// stamped them. The check stays as a `debug_assert`.
fn coalesce_level_entries(
    segments: &[Adaptive3dSegment],
    level_marker_index: Option<usize>,
    min_region_cut_length_mm: f64,
    z_level: f64,
    replaced_entries: usize,
) -> usize {
    if min_region_cut_length_mm <= 0.0 {
        return 0;
    }
    debug_assert_eq!(
        redundant_entry_count(segments, level_marker_index),
        0,
        "G-PHANTOMSTAMP: a level left an entry followed by another entry"
    );
    if replaced_entries > 0 {
        debug!(
            z = z_level,
            coalesced_entries = replaced_entries,
            "F-038: coalesced redundant back-to-back entries"
        );
    }
    replaced_entries
}

#[expect(
    clippy::too_many_arguments,
    reason = "each parameter is a separate input that the callers supply"
)]
pub(super) fn clear_z_level_slice(
    ctx: &ClearZLevelContext<'_>,
    material_stock: &mut TriDexelStock,
    surface_hm: &SurfaceHeightmap,
    z_level: f64,
    segments: &mut Vec<Adaptive3dSegment>,
    cursor: &mut PlannerCursor,
    area_mask: Option<&AreaMask>,
    level_marker: Option<Adaptive3dRuntimeEvent>,
    cancel: &dyn CancelCheck,
) -> Result<(), Cancelled> {
    let (remaining_cells, remaining_fraction) = level_gate(
        material_stock,
        surface_hm,
        z_level,
        ctx.stock_to_leave,
        area_mask,
    );
    if remaining_cells < MIN_CELLS_TO_CLEAR {
        return Ok(());
    }

    let level_scope = ctx.debug.as_ref().map(|debug_ctx| {
        let label = if let Some(m) = area_mask {
            format!(
                "Z {:.3} job {} ({} cells)",
                z_level,
                m.job + 1,
                m.owned_count
            )
        } else {
            format!("Z {:.3}", z_level)
        };
        debug_ctx.start_span(DebugSpanKind::ZLevel, label)
    });
    if let Some(scope) = level_scope.as_ref() {
        scope.set_z_level(z_level);
        scope.set_counter("remaining_before", remaining_fraction);
    }
    let level_ctx = level_scope.as_ref().map(|scope| scope.context());

    let Some(OrderedRegions {
        regions,
        metrics: mut level_metrics,
    }) = detect_and_order_regions(
        ctx,
        material_stock,
        surface_hm,
        z_level,
        area_mask,
        cursor.last_pos,
    )
    else {
        return Ok(());
    };

    let level_marker_index = level_marker.map(|event| {
        segments.push(Adaptive3dSegment::Marker(event));
        segments.len().saturating_sub(1)
    });

    // 4. Build 2D adaptive params from 3D context.
    let params_2d = crate::adaptive::AdaptiveParams {
        tool_radius: ctx.tool_radius,
        stepover: ctx.stepover,
        // cut_depth / feed_rate / plunge_rate / safe_z are unused by
        // `adaptive_segments_with_debug`; only the final `segments_to_toolpath`
        // consumes them. We're using segments directly here.
        cut_depth: 0.0,
        feed_rate: 0.0,
        plunge_rate: 0.0,
        safe_z: 0.0,
        tolerance: ctx.tolerance,
        slot_clearing: false,
        min_cutting_radius: 0.0,
        standing_material: None,
        cleanup_strategy: crate::adaptive::CleanupStrategy::ContourParallelHybrid,
        engagement_measure: ctx.engagement_measure,
        // The 2D agent planner: the F-038 forecast plans with it, and the
        // geodesic door sets the same value (`adaptive/geodesic/mod.rs`).
        // ContourSpiral (3D) used the spiral here until removal step R4.
        path_strategy: crate::adaptive::PathStrategy2d::Agent,
        // Not read by the agent planner; the guard keeps the value valid.
        trochoid_cap_mult: if ctx.trochoid_cap_mult.is_finite() && ctx.trochoid_cap_mult > 0.0 {
            ctx.trochoid_cap_mult
        } else {
            TROCHOID_CAP_MULT_3D
        },
        // Research C12: the arm plans under the 2D pass load and emits its
        // links as cuts (`plan_one_outline`).
        keep_down_links: crate::adaptive::KeepDownLinks::WithinPassLoad,
        entry_helix_radius: 0.0,
    };

    if let Some(scope) = level_scope.as_ref() {
        scope.set_counter("region_count", regions.len() as f64);
    }

    // 6. Run 2D adaptive on each region independently, lift segments,
    //    stamp dexel stock. Each region's first segment from the 2D adaptive
    //    is a Rapid (= entry into that region), which becomes a 3D Rapid
    //    (retract to safe_z, traverse XY, plunge). Between disjoint regions
    //    that retract is correct; the 2D adaptive can't link across regions
    //    because each call sees only its own region's polygon.
    let mut cut_count = 0u32;
    let mut dropped_short_regions = 0usize;
    let region_total = regions.len();
    let mut emission = LevelEmission {
        sink: LevelSink {
            segments,
            material_stock,
            cursor,
            pending_entry: None,
            replaced_entries: 0,
            defer_entries: ctx.min_region_cut_length_mm > 0.0,
        },
        level_metrics: &mut level_metrics,
        cut_count: &mut cut_count,
        dropped_short_regions: &mut dropped_short_regions,
    };
    let plan = RegionClearPlan {
        surface_hm,
        z_level,
        params_2d: &params_2d,
        level_ctx: &level_ctx,
        region_total,
    };
    for (region_idx, region_polygon) in regions.iter().enumerate() {
        clear_one_region(
            ctx,
            &plan,
            &mut emission,
            region_polygon,
            region_idx,
            cancel,
        )?;
    }

    // The level end commits a trailing entry, as the coalescing pass kept it.
    emission.sink.commit_pending(ctx);
    let replaced_entries = emission.sink.replaced_entries;
    let coalesced_entries = coalesce_level_entries(
        segments,
        level_marker_index,
        ctx.min_region_cut_length_mm,
        z_level,
        replaced_entries,
    );
    level_metrics.dropped_short_region_count = dropped_short_regions + coalesced_entries;
    if let Some(scope) = level_scope.as_ref() {
        scope.set_counter("cut_segments", cut_count as f64);
        scope.set_counter(
            "perimeter_sweep_length_mm",
            level_metrics.perimeter_sweep_length_mm,
        );
        scope.set_counter(
            "agent_walk_cut_length_mm",
            level_metrics.agent_walk_cut_length_mm,
        );
        scope.set_counter("dropped_short_regions_f038", dropped_short_regions as f64);
        scope.set_counter("coalesced_entries_f038", coalesced_entries as f64);
    }
    update_level_marker_metrics(segments, level_marker_index, level_metrics);

    Ok(())
}

/// F-038 check. Count the `Rapid`/`RapidWithFloor` segments in
/// `segments[start..]` whose only successors before the next entry are
/// `Marker` events, i.e. each `[Rapid, (Marker)*, Rapid]` pair. The
/// emitter would spend a retract + plunge on the first rapid only to
/// retract again for the second. Since G-PHANTOMSTAMP `LevelSink` never
/// leaves one, so this returns 0 on every level the sink planned.
fn redundant_entry_count(
    segments: &[Adaptive3dSegment],
    level_marker_index: Option<usize>,
) -> usize {
    let start = level_marker_index.map_or(0, |i| i + 1);
    let is_entry = |s: &Adaptive3dSegment| {
        matches!(
            s,
            Adaptive3dSegment::Rapid(_) | Adaptive3dSegment::RapidWithFloor { .. }
        )
    };
    let mut count = 0usize;
    let mut previous_was_entry = false;
    for seg in segments.iter().skip(start) {
        if matches!(seg, Adaptive3dSegment::Marker(_)) {
            continue;
        }
        let entry = is_entry(seg);
        if entry && previous_was_entry {
            count += 1;
        }
        previous_was_entry = entry;
    }
    count
}

fn update_level_marker_metrics(
    segments: &mut [Adaptive3dSegment],
    marker_index: Option<usize>,
    metrics: ZLevelPlanMetrics,
) {
    let Some(index) = marker_index else {
        return;
    };
    if let Some(Adaptive3dSegment::Marker(event)) = segments.get_mut(index) {
        event.set_z_level_metrics(metrics);
    }
}

#[cfg(test)]
mod region_order_tests {
    use super::{air_run_crossover_mm, nearest_neighbor_order};

    #[test]
    fn crossover_reproduces_the_retired_70mm_anchor() {
        // The old constant assumed ~0.5 s of retract overhead at
        // 3150 mm/min feed / 5000 mm/min rapid → ~71 mm crossover.
        // overhead(d) = 2d/rapid − buf/rapid + buf/plunge = 0.5 s at
        // d ≈ 18.7 mm (plunge 500).
        let got = air_run_crossover_mm(3150.0, 500.0, 18.7);
        assert!(
            (got - 71.0).abs() < 3.0,
            "expected ≈71 mm at the old constant's operating point, got {got:.1}"
        );
    }

    #[test]
    fn crossover_scales_with_retract_depth_and_feed() {
        // Deeper retract ⇒ more overhead ⇒ longer crossover.
        let shallow = air_run_crossover_mm(3150.0, 500.0, 5.0);
        let deep = air_run_crossover_mm(3150.0, 500.0, 30.0);
        assert!(shallow < deep, "shallow {shallow:.1} !< deep {deep:.1}");

        // Faster feed narrows the feed-vs-rapid gap ⇒ longer crossover.
        let slow_feed = air_run_crossover_mm(1000.0, 500.0, 10.0);
        let fast_feed = air_run_crossover_mm(4500.0, 500.0, 10.0);
        assert!(
            slow_feed < fast_feed,
            "slow {slow_feed:.1} !< fast {fast_feed:.1}"
        );
    }

    #[test]
    fn crossover_is_infinite_when_feed_beats_rapid() {
        // Feed ≥ assumed rapid: demoting to a rapid never pays.
        assert!(air_run_crossover_mm(5000.0, 500.0, 10.0).is_infinite());
        assert!(air_run_crossover_mm(8000.0, 500.0, 10.0).is_infinite());
    }

    #[test]
    fn nn_visits_nearest_first_from_start() {
        // Four anchors in a row at x = 0,1,2,3 (y=0). Starting at x=10
        // (right side), the nearest-first tour must be 3,2,1,0.
        let anchors = [(0.0, 0.0), (1.0, 0.0), (2.0, 0.0), (3.0, 0.0)];
        let order = nearest_neighbor_order(&anchors, (10.0, 0.0));
        assert_eq!(order, vec![3, 2, 1, 0]);
    }

    #[test]
    fn nn_starts_at_closest_to_seed_then_chains() {
        // Seed near the origin; two clusters. Expect origin cluster
        // first, then hop to the far cluster and stay local.
        let anchors = [
            (0.0, 0.0),  // 0
            (50.0, 0.0), // 1 far
            (1.0, 0.0),  // 2 near 0
            (51.0, 0.0), // 3 near 1
        ];
        let order = nearest_neighbor_order(&anchors, (0.2, 0.0));
        // 0 (closest to seed) → 2 (1mm away) → 1 (49mm) → 3 (1mm)
        assert_eq!(order, vec![0, 2, 1, 3]);
    }

    #[test]
    fn nn_handles_empty_and_single() {
        assert!(nearest_neighbor_order(&[], (0.0, 0.0)).is_empty());
        assert_eq!(nearest_neighbor_order(&[(5.0, 5.0)], (0.0, 0.0)), vec![0]);
    }

    #[test]
    fn nn_is_a_permutation() {
        // Every index appears exactly once, regardless of layout.
        let anchors = [(3.0, 7.0), (-2.0, 1.0), (8.0, -4.0), (0.0, 0.0), (5.0, 5.0)];
        let mut order = nearest_neighbor_order(&anchors, (1.0, 1.0));
        assert_eq!(order.len(), anchors.len());
        order.sort_unstable();
        assert_eq!(order, vec![0, 1, 2, 3, 4]);
    }
}
