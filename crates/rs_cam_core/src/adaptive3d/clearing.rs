//! 3D Z-level clearing engine for adaptive3d: region detection,
//! per-level contour-parallel and curvature-adaptive clearing,
//! stamping, and waterline cleanup.

use crate::contour_extract::marching_squares_bool_grid;
use crate::debug_trace::ToolpathDebugContext;
use crate::dexel_stock::{StockCutDirection, TriDexelStock};
use crate::geo::{P2, P3};
use crate::grid_field::{edt_curvature_field, smooth_grid};
use crate::interrupt::{CancelCheck, Cancelled, check_cancel};
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::radial_profile::RadialProfileLUT;
use crate::slope::{SlopeMap, SurfaceHeightmap};
use crate::tool::MillingCutter;
use crate::waterline::waterline_contours_with_cancel;
use std::collections::VecDeque;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;
use tracing::debug;

use super::path::{Adaptive3dSegment, drape_path_to_leave, drape_point};
use super::search::{
    blend_corners_3d, is_clear_path_3d, material_remaining_at_level, material_remaining_in_region,
};
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

/// In-engine fallback for the 3D ContourSpiral trochoid trigger cap.
/// Loops fire when predicted leading-arc engagement exceeds
/// `target × this`: 1.2 is flattest-possible load but high travel; 1.6 is
/// the balanced knee — load still flat (p99 well under the spiky
/// strategies) while cutting ~25-30% less distance. See the trochoid-cap
/// sweep in `planning/ADAPTIVE_CLEARING_ALGO_REVIEW_2026-06-12.md`.
///
/// As of the "Optimal load + Nibble" UI reframe this is operator-tunable
/// via `Adaptive3dConfig::trochoid_cap_mult` (GUI "Nibble" dial); the
/// const remains the default and a sanity fallback for any context built
/// with a non-finite or non-positive cap.
const TROCHOID_CAP_MULT_3D: f64 = 1.6;

/// Stage 4 — quantise a world coordinate to a fixed-point key (0.001 mm)
/// for the planner-engagement position lookup. Distinct spiral sample
/// points are spaced far wider than this, so the key is collision-free
/// while tolerating any benign float round-trip between the 2D emit and
/// the 3D lift.
#[inline]
fn quantize_coord(v: f64) -> i64 {
    (v * 1000.0).round() as i64
}

// ── Strategy-agnostic dispatch ────────────────────────────────────────

/// Run a single Z-level clear pass via the strategy on `ctx`, with no
/// per-level marker (used for shallow sub-passes that should slot under
/// the parent major-Z level's marker, not emit their own).
///
/// The main per-Z-level loops in `path.rs` push their own markers
/// directly for the major levels — this helper exists only for the
/// shallow sub-pass loop, which calls into the same clear function the
/// strategy already uses for its main pass.
#[allow(clippy::too_many_arguments)]
pub(super) fn clear_z_level_dispatch_no_marker(
    ctx: &ClearZLevelContext<'_>,
    material_stock: &mut TriDexelStock,
    surface_hm: &SurfaceHeightmap,
    z_level: f64,
    segments: &mut Vec<Adaptive3dSegment>,
    last_pos: &mut Option<P3>,
    // Stage 4 — forwarded to the spiral arm only (the other strategies
    // produce no planner engagement).
    planner_eng: &mut Vec<(P3, f64)>,
    region: Option<&MaterialRegion>,
    cancel: &dyn CancelCheck,
) -> Result<(), Cancelled> {
    match ctx.clearing_strategy {
        ClearingStrategy3d::ContourParallel => clear_z_level_contour_parallel(
            ctx,
            material_stock,
            surface_hm,
            z_level,
            segments,
            last_pos,
            region,
            cancel,
        ),
        ClearingStrategy3d::Adaptive => clear_z_level_adaptive(
            ctx,
            material_stock,
            surface_hm,
            z_level,
            segments,
            last_pos,
            region,
            cancel,
        ),
        ClearingStrategy3d::AgentSearch | ClearingStrategy3d::ContourSpiral => {
            clear_z_level_agent_2d_slice(
                ctx,
                material_stock,
                surface_hm,
                z_level,
                segments,
                last_pos,
                planner_eng,
                region,
                None,
                cancel,
            )
        }
    }
}

// ── Region detection ──────────────────────────────────────────────────

/// A connected region of material detected by flood fill on the heightmap.
#[allow(dead_code)] // Some fields are strategy-specific and only read by some strategies.
pub(super) struct MaterialRegion {
    pub(super) row_min: usize,
    pub(super) row_max: usize,
    pub(super) col_min: usize,
    pub(super) col_max: usize,
    /// World-space bounding box (expanded by tool_radius for direction search).
    pub(super) world_x_min: f64,
    pub(super) world_x_max: f64,
    pub(super) world_y_min: f64,
    pub(super) world_y_max: f64,
    pub(super) cell_count: usize,
    pub(super) surface_z_min: f64,
    pub(super) surface_z_max: f64,
}

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// Detect connected material regions via 8-connected BFS flood fill.
///
/// A cell "has material" if the top-Z of the dexel ray exceeds
/// `surface_z + stock_to_leave + 0.01`.
/// Regions with fewer than `min_cells` (default 4) are filtered out.
/// Returns regions sorted by cell_count descending (largest first).
pub(super) fn detect_material_regions(
    material_stock: &TriDexelStock,
    surface_hm: &SurfaceHeightmap,
    stock_to_leave: f64,
    tool_radius: f64,
) -> Vec<MaterialRegion> {
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
                let cs = material_stock.z_grid.cell_size;
                regions.push(MaterialRegion {
                    row_min: rmin,
                    row_max: rmax,
                    col_min: cmin,
                    col_max: cmax,
                    world_x_min: material_stock.z_grid.origin_u + cmin as f64 * cs - tool_radius,
                    world_x_max: material_stock.z_grid.origin_u + cmax as f64 * cs + tool_radius,
                    world_y_min: material_stock.z_grid.origin_v + rmin as f64 * cs - tool_radius,
                    world_y_max: material_stock.z_grid.origin_v + rmax as f64 * cs + tool_radius,
                    cell_count: count,
                    surface_z_min: sz_min,
                    surface_z_max: sz_max,
                });
            }

            region_id += 1;
        }
    }

    // Sort largest first
    regions.sort_by(|a, b| b.cell_count.cmp(&a.cell_count));
    regions
}

// ── Z-level clearing helper ──────────────────────────────────────────

/// Parameters for a single Z-level clearing pass, extracted to avoid
/// threading dozens of locals through the helper.
#[allow(dead_code)] // Some fields are strategy-specific (ContourParallel, Adaptive, AgentSearch-2d).
pub(super) struct ClearZLevelContext<'a> {
    pub(super) mesh: &'a TriangleMesh,
    pub(super) index: &'a SpatialIndex,
    pub(super) cutter: &'a dyn MillingCutter,
    pub(super) lut: &'a RadialProfileLUT,
    pub(super) slope_map: &'a SlopeMap,
    pub(super) debug: Option<ToolpathDebugContext>,
    pub(super) tool_radius: f64,
    pub(super) stepover: f64,
    pub(super) stock_to_leave: f64,
    pub(super) depth_per_pass: f64,
    pub(super) tolerance: f64,
    /// Op cutting feed (mm/min) — used for the feed-vs-rapid air-run
    /// crossover, not for emission (segments carry no feeds here).
    pub(super) feed_rate: f64,
    /// Op plunge feed (mm/min) — same crossover use.
    pub(super) plunge_rate: f64,
    pub(super) target_frac: f64,
    pub(super) step_len: f64,
    pub(super) max_link_dist: f64,
    /// Safe-Z used by `segments_to_toolpath` for retracts and the start
    /// of peck-plunges in `Adaptive3dSegment::Rapid`. Plumbed into the
    /// clearing layer so the planner can mirror those plunge stamps in
    /// its internal dexel state (parity with the simulator replay).
    pub(super) safe_z: f64,
    pub(super) bbox_x_min: f64,
    pub(super) bbox_x_max: f64,
    pub(super) bbox_y_min: f64,
    pub(super) bbox_y_max: f64,
    pub(super) clearing_strategy: ClearingStrategy3d,
    /// Trochoid trigger cap for the ContourSpiral slice ("Nibble" dial).
    /// Replaces the historical `TROCHOID_CAP_MULT_3D` const so the value
    /// is operator-tunable; the const survives as the in-engine fallback.
    pub(super) trochoid_cap_mult: f64,
    /// Engagement quantity for the AgentSearch 2D sub-pass (F1).
    pub(super) engagement_measure: crate::adaptive::EngagementMeasure,
    pub(super) z_blend: bool,
    /// Minimum corner radius for `blend_corners_3d` — needed inside
    /// `stamp_emitted_segment` so the planner stamps the SAME path the
    /// simulator will replay (segments_to_toolpath blends Cut paths
    /// before emitting feeds).
    pub(super) min_cutting_radius: f64,
    /// When `Some`, restricts `build_material_bool_grid` to cells where
    /// the mask is true. Row-major, indexed `row * surface_cols + col`
    /// (matches `SurfaceHeightmap::z_values`). Used by the "mill shallow
    /// areas" feature to run sub-passes only on low-slope cells without
    /// disturbing the steep-side dexel state.
    pub(super) shallow_mask: Option<&'a [bool]>,
    /// F-038: minimum forecast horizontal cutting length (mm) a marching-
    /// squares region must produce in its 2D adaptive sub-pass before the
    /// AgentSearch dispatch commits an entry plunge to it. Set to 0.0 to
    /// disable. See `clear_z_level_agent_2d_slice` for the apply site.
    pub(super) min_region_cut_length_mm: f64,
}

// ── Contour-parallel clearing ─────────────────────────────────────────

/// Build a padded boolean grid of material cells at a given Z level.
///
/// A cell is `true` if the stock has material above the effective floor
/// (max of surface_z + stock_to_leave, z_level). The grid is padded with
/// a 1-cell false border so marching squares and EDT detect edge boundaries.
///
/// Returns `(padded_grid, padded_rows, padded_cols, origin_x, origin_y, cell_size)`.
///
/// `shallow_mask`, when `Some`, is an additional row-major filter
/// indexed `row * cols + col` (matches `SurfaceHeightmap::z_values` /
/// `SlopeMap` layout). Cells where the mask is `false` are treated as
/// "no material" regardless of stock state — used by the
/// `mill_shallow_areas` sub-pass to restrict clearing to low-slope cells.
#[allow(clippy::indexing_slicing)] // SAFETY: padded grid indices bounded by loop ranges
fn build_material_bool_grid(
    material_stock: &TriDexelStock,
    surface_hm: &SurfaceHeightmap,
    z_level: f64,
    stock_to_leave: f64,
    region: Option<&MaterialRegion>,
    shallow_mask: Option<&[bool]>,
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
            // Skip cells outside the region if one is specified.
            if let Some(r) = region
                && (row < r.row_min || row > r.row_max || col < r.col_min || col > r.col_max)
            {
                continue;
            }

            // Skip cells excluded by the shallow-area mask.
            if let Some(mask) = shallow_mask {
                let idx = row * cols + col;
                if idx >= mask.len() || !mask[idx] {
                    continue;
                }
            }

            let surf_z = surface_hm.z_or_bbox_floor_at(row, col);
            let effective_floor = (surf_z + stock_to_leave).max(z_level);

            if stock_has_material_above(material_stock, row, col, effective_floor + 0.01) {
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
/// emitted move up to `surface + stock_to_leave` (the `fa27b08` gouge
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
}

impl<'a> ClearZLevelContext<'a> {
    pub(super) fn stamp_drape(&self) -> StampDrape<'a> {
        StampDrape {
            mesh: self.mesh,
            index: self.index,
            cutter: self.cutter,
            stock_to_leave: self.stock_to_leave,
        }
    }
}

/// Mirror in the planner's `material_stock` the swept-tube stamps that
/// the simulator will produce when it replays the toolpath emitted by
/// `segments_to_toolpath` for `segment`.
///
/// Call this AFTER updating `last_pos` for the previous segment but
/// BEFORE updating it for `segment` (we need the pre-segment XY/Z to
/// know where a `Link` feed starts from).
///
/// `safe_z` / `tolerance` / `min_cutting_radius` match
/// `Adaptive3dParams`. The invariant this function exists to hold is
/// that it applies **every** transformation `segments_to_toolpath`
/// applies, so the planner stamps the SAME path the simulator will
/// replay. Today that is three transformations, in the emitter's order:
///
/// 1. `drape_path_to_leave` / `drape_point` — the `fa27b08` gouge guard,
///    which densifies to `<= cutter.radius()` and raises every point to
///    `drop_cutter(x, y) + stock_to_leave`;
/// 2. `simplify_path_3d` (RDP at `tolerance`);
/// 3. `blend_corners_3d` (at `min_cutting_radius`).
///
/// If a fourth is ever added to the emitter it must be added here in the
/// same commit. `fa27b08` added (1) to the emitter and not here, and the
/// planner spent seven weeks believing it had removed material its own
/// emitted toolpath leaves standing — see
/// `planning/review_2026-08-04/ADAPTIVE3D_RED_BASELINE.md` §3.
#[allow(clippy::too_many_arguments)]
fn stamp_emitted_segment(
    material_stock: &mut TriDexelStock,
    lut: &RadialProfileLUT,
    tool_radius: f64,
    last_pos: &Option<P3>,
    segment: &Adaptive3dSegment,
    safe_z: f64,
    tolerance: f64,
    min_cutting_radius: f64,
    drape: &StampDrape<'_>,
) {
    match segment {
        Adaptive3dSegment::Cut(path) => {
            // Mirror segments_to_toolpath's path transformation —
            // drape_path_to_leave, then simplify_path_3d (RDP), then
            // blend_corners_3d — so the planner's swept stamps cover
            // the SAME tubes the simulator will stamp from the emitted
            // feeds. Order matters: the emitter drapes BEFORE
            // simplifying, so the RDP sees the densified, lifted path.
            if path.len() >= 2 {
                let draped = drape_path_to_leave(
                    path,
                    drape.mesh,
                    drape.index,
                    drape.cutter,
                    drape.stock_to_leave,
                    drape.cutter.radius(),
                );
                let simplified = simplify_path_3d(&draped, tolerance);
                let blended = blend_corners_3d(&simplified, min_cutting_radius);
                stamp_along_path(material_stock, lut, tool_radius, &blended);
            } else {
                stamp_along_path(material_stock, lut, tool_radius, path);
            }
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
            // draped Z, not the raw one. Mirror that. (The
            // `RapidWithFloor` arm below has no such rebind in the
            // emitter, so it must not get one here either.)
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
        }
        Adaptive3dSegment::RapidWithFloor {
            entry,
            rapid_floor_z,
        } => {
            // Toolpath: rapid descent from safe_z down to ~rapid_floor_z
            // (cleared air, no stamp), then peck-plunge from there to
            // entry. Mirror segments_to_toolpath's `descent_floor` calc
            // exactly so we don't stamp BELOW entry.z (which would
            // happen if rapid_floor_z < entry.z, e.g. previous pass
            // already cut DEEPER than this entry — clearing function
            // sampled the post-stamp top).
            const RAPID_DESCENT_BUFFER_MM: f64 = 0.5;
            let descent_floor = (*rapid_floor_z + RAPID_DESCENT_BUFFER_MM)
                .min(safe_z)
                .max(entry.z);
            let start = P3::new(entry.x, entry.y, descent_floor);
            material_stock.stamp_linear_segment(
                lut,
                tool_radius,
                start,
                *entry,
                StockCutDirection::FromTop,
            );
        }
        Adaptive3dSegment::Link(target) => {
            // Toolpath: feed at constant Z from last_pos to target.
            if let Some(prev) = last_pos {
                material_stock.stamp_linear_segment(
                    lut,
                    tool_radius,
                    *prev,
                    *target,
                    StockCutDirection::FromTop,
                );
            }
        }
        Adaptive3dSegment::Marker(_) => {}
    }
}

/// Helper to push a segment and stamp its simulator-equivalent swept
/// material removal in one call.
#[allow(clippy::too_many_arguments)]
fn push_segment_with_stamp(
    segments: &mut Vec<Adaptive3dSegment>,
    material_stock: &mut TriDexelStock,
    lut: &RadialProfileLUT,
    tool_radius: f64,
    last_pos: &mut Option<P3>,
    segment: Adaptive3dSegment,
    safe_z: f64,
    tolerance: f64,
    min_cutting_radius: f64,
    drape: &StampDrape<'_>,
) {
    stamp_emitted_segment(
        material_stock,
        lut,
        tool_radius,
        last_pos,
        &segment,
        safe_z,
        tolerance,
        min_cutting_radius,
        drape,
    );
    // Update last_pos based on segment's terminal XYZ before pushing.
    match &segment {
        Adaptive3dSegment::Cut(path) => {
            if let Some(p) = path.last() {
                *last_pos = Some(*p);
            }
        }
        Adaptive3dSegment::Rapid(entry) | Adaptive3dSegment::RapidWithFloor { entry, .. } => {
            *last_pos = Some(*entry);
        }
        Adaptive3dSegment::Link(target) => {
            *last_pos = Some(*target);
        }
        Adaptive3dSegment::Marker(_) => {}
    }
    segments.push(segment);
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
#[allow(clippy::too_many_arguments)]
pub(super) fn clear_z_level_contour_parallel(
    ctx: &ClearZLevelContext<'_>,
    material_stock: &mut TriDexelStock,
    surface_hm: &SurfaceHeightmap,
    z_level: f64,
    segments: &mut Vec<Adaptive3dSegment>,
    last_pos: &mut Option<P3>,
    region: Option<&MaterialRegion>,
    cancel: &dyn CancelCheck,
) -> Result<(), Cancelled> {
    // Check material remaining — skip if negligible. Gate on absolute
    // cell count, not fraction: at small DPP a real island contributes
    // very few cells per level, and a fraction-based gate would skip it.
    let remaining = if let Some(r) = region {
        material_remaining_in_region(material_stock, surface_hm, z_level, ctx.stock_to_leave, r)
    } else {
        material_remaining_at_level(material_stock, surface_hm, z_level, ctx.stock_to_leave)
    };
    if remaining.cells_with_material < MIN_CELLS_TO_CLEAR {
        debug!(
            z = z_level,
            cells = remaining.cells_with_material,
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
        region,
        ctx.shallow_mask,
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
    let edt = crate::grid_field::distance_transform_2d(&air_grid, rows, cols);

    // 3. Find max distance (determines number of offset levels)
    let max_dist = edt.iter().copied().fold(0.0f64, f64::max);

    // 4. Generate contours at each stepover threshold
    let tool_radius_cells = ctx.tool_radius / cell_size;
    let stepover_cells = ctx.stepover / cell_size;

    debug!(
        z = z_level,
        remaining_cells = remaining.cells_with_material,
        mat_count,
        rows,
        cols,
        max_dist_cells = max_dist,
        tool_radius_cells = tool_radius_cells,
        stepover_cells = stepover_cells,
        "Contour-parallel EDT: generating offset contours"
    );

    // Z-blend: when enabled, outer contours stay flat at z_level and inner
    // contours progressively descend toward the terrain surface.
    let offset_range = max_dist - tool_radius_cells;
    let z_blend_enabled = ctx.z_blend;

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

        // Blend factor: 0.0 at outermost contour, 1.0 at innermost
        // Only active when z_blend is enabled; otherwise all passes cut at z_level.
        let blend = if z_blend_enabled && offset_range > 1e-6 {
            ((threshold - tool_radius_cells) / offset_range).clamp(0.0, 1.0)
        } else {
            0.0
        };

        // Threshold the EDT: cells with distance > threshold are "inside" the offset
        let mask: Vec<bool> = edt.iter().map(|&d| d > threshold).collect();
        let loops = marching_squares_bool_grid(&mask, rows, cols, origin_x, origin_y, cell_size);

        for loop_pts in &loops {
            if loop_pts.len() < 3 {
                continue;
            }

            // Z-blended surface drape: outer passes cut near z_level (flat),
            // inner passes progressively descend toward the terrain surface.
            // This spreads Z movement across all passes instead of a sudden
            // plunge on the innermost pass.
            let mut path_3d: Vec<P3> = Vec::with_capacity(loop_pts.len());
            for p in loop_pts {
                let surf_z = surface_hm.z_or_bbox_floor_at_world(p.x, p.y);
                let target_z = if surf_z == f64::NEG_INFINITY {
                    z_level
                } else {
                    surf_z + ctx.stock_to_leave // actual terrain — may be below z_level
                };
                // Lerp: blend=0 → z_level (flat), blend=1 → target_z (terrain)
                // Clamp so we never cut below the next Z level's floor.
                let z = (z_level + blend * (target_z - z_level)).max(target_z);
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
                    #[allow(clippy::indexing_slicing)] // idx < path_3d.len() by construction
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
                // matches the one in clear_z_level (the AgentSearch path).
                let link_dist = ctx.max_link_dist;
                let should_link = last_pos.is_some_and(|lp| {
                    let dx = first.x - lp.x;
                    let dy = first.y - lp.y;
                    (dx * dx + dy * dy).sqrt() < link_dist
                        && is_clear_path_3d(
                            material_stock,
                            surface_hm,
                            lp,
                            *first,
                            ctx.stock_to_leave,
                            ctx.depth_per_pass,
                        )
                });
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
                    last_pos,
                    entry_seg,
                    ctx.safe_z,
                    ctx.tolerance,
                    ctx.min_cutting_radius,
                    &ctx.stamp_drape(),
                );
                push_segment_with_stamp(
                    segments,
                    material_stock,
                    ctx.lut,
                    ctx.tool_radius,
                    last_pos,
                    Adaptive3dSegment::Cut(path_3d),
                    ctx.safe_z,
                    ctx.tolerance,
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
        region,
        ctx.shallow_mask,
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
                (surf_z + ctx.stock_to_leave).max(z_level)
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
                // SAFETY: row*cc+col bounded by grid dimensions
                #[allow(clippy::indexing_slicing)]
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

        for path in &cleanup_pts {
            if let Some(first) = path.first() {
                push_segment_with_stamp(
                    segments,
                    material_stock,
                    ctx.lut,
                    ctx.tool_radius,
                    last_pos,
                    Adaptive3dSegment::Rapid(*first),
                    ctx.safe_z,
                    ctx.tolerance,
                    ctx.min_cutting_radius,
                    &ctx.stamp_drape(),
                );
                if path.len() >= 2 {
                    push_segment_with_stamp(
                        segments,
                        material_stock,
                        ctx.lut,
                        ctx.tool_radius,
                        last_pos,
                        Adaptive3dSegment::Cut(path.clone()),
                        ctx.safe_z,
                        ctx.tolerance,
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
                        last_pos,
                        Adaptive3dSegment::Cut(vec![*first, end]),
                        ctx.safe_z,
                        ctx.tolerance,
                        ctx.min_cutting_radius,
                        &ctx.stamp_drape(),
                    );
                }
            }
        }
    }

    Ok(())
}

/// Adaptive clearing: variable-offset EDT for constant tool engagement.
///
/// Same structure as `clear_z_level_contour_parallel` but uses a spatially-
/// varying threshold based on EDT level-set curvature.  At convex boundary
/// sections the stepover shrinks (preventing engagement spikes); at concave
/// sections it grows (avoiding wasted light passes).
#[allow(clippy::too_many_arguments)]
pub(super) fn clear_z_level_adaptive(
    ctx: &ClearZLevelContext<'_>,
    material_stock: &mut TriDexelStock,
    surface_hm: &SurfaceHeightmap,
    z_level: f64,
    segments: &mut Vec<Adaptive3dSegment>,
    last_pos: &mut Option<P3>,
    region: Option<&MaterialRegion>,
    cancel: &dyn CancelCheck,
) -> Result<(), Cancelled> {
    // ── Material check ─────────────────────────────────────────────────
    // Absolute cell-count gate (not fraction) — see clear_z_level_concentric.
    let remaining = if let Some(r) = region {
        material_remaining_in_region(material_stock, surface_hm, z_level, ctx.stock_to_leave, r)
    } else {
        material_remaining_at_level(material_stock, surface_hm, z_level, ctx.stock_to_leave)
    };
    if remaining.cells_with_material < MIN_CELLS_TO_CLEAR {
        return Ok(());
    }

    // ── 1. Build boolean material grid ─────────────────────────────────
    let (material_grid, rows, cols, origin_x, origin_y, cell_size) = build_material_bool_grid(
        material_stock,
        surface_hm,
        z_level,
        ctx.stock_to_leave,
        region,
        ctx.shallow_mask,
    );

    if !material_grid.iter().any(|&b| b) {
        return Ok(());
    }

    // ── 2. EDT on inverted grid (distance to nearest air) ──────────────
    let air_grid: Vec<bool> = material_grid.iter().map(|&b| !b).collect();
    let edt = crate::grid_field::distance_transform_2d(&air_grid, rows, cols);
    let max_dist = edt.iter().copied().fold(0.0f64, f64::max);

    // ── 3. Curvature field from EDT level sets ─────────────────────────
    let mut curvature = edt_curvature_field(&edt, rows, cols);
    // Smooth to suppress finite-difference noise near the medial axis.
    // Scale with tool radius so the kernel covers ~1 tool diameter.
    let tool_radius_cells = ctx.tool_radius / cell_size;
    let smooth_r = (tool_radius_cells as usize).max(3);
    smooth_grid(&mut curvature, rows, cols, smooth_r);

    // ── 4. Precompute per-cell curvature offset ────────────────────────
    // The offset is a CONSTANT shift per cell (does not scale with level N).
    // This keeps contour topology stable across levels while adjusting
    // local spacing based on curvature.
    //   offset = base_step * (−κR / (1 + κR))  clamped for stability
    //   Concave κ < 0: offset > 0 → contour recedes → wider pass
    //   Convex  κ > 0: offset < 0 → contour advances → tighter pass
    let total = rows * cols;
    let alpha = ctx.target_frac * std::f64::consts::TAU;
    let base_step = ctx.tool_radius * (1.0 - alpha.cos());
    let base_step_cells = base_step / cell_size;

    let mut curvature_offset = vec![0.0f64; total];
    for (off, &kappa) in curvature_offset.iter_mut().zip(curvature.iter()) {
        let kr = kappa * tool_radius_cells;
        let denom = (1.0 + kr).clamp(0.5, 2.0);
        // offset = base_step * (1/denom - 1), clamped to ±0.5 * base_step
        *off = (base_step_cells * (1.0 / denom - 1.0))
            .clamp(-0.5 * base_step_cells, 0.5 * base_step_cells);
    }

    // Z-blend setup (identical to contour-parallel)
    let offset_range = max_dist - tool_radius_cells;
    let z_blend_enabled = ctx.z_blend;

    debug!(
        z = z_level,
        max_dist_cells = max_dist,
        tool_radius_cells = tool_radius_cells,
        base_step_cells = base_step_cells,
        "Adaptive EDT: generating curvature-adjusted contours"
    );

    // ── 5. Offset loop: fixed base progression + constant curvature shift
    let mut threshold = tool_radius_cells;

    while threshold < max_dist {
        check_cancel(cancel)?;

        // Blend factor: 0.0 at outermost contour, 1.0 at innermost
        let blend = if z_blend_enabled && offset_range > 1e-6 {
            ((threshold - tool_radius_cells) / offset_range).clamp(0.0, 1.0)
        } else {
            0.0
        };

        // Variable mask: base threshold + per-cell curvature offset
        let mask: Vec<bool> = edt
            .iter()
            .zip(curvature_offset.iter())
            .map(|(&d, &off)| d > threshold + off)
            .collect();
        let loops = marching_squares_bool_grid(&mask, rows, cols, origin_x, origin_y, cell_size);

        for loop_pts in &loops {
            if loop_pts.len() < 3 {
                continue;
            }

            // Z-blended surface drape (identical to contour-parallel)
            let mut path_3d: Vec<P3> = Vec::with_capacity(loop_pts.len());
            for p in loop_pts {
                let surf_z = surface_hm.z_or_bbox_floor_at_world(p.x, p.y);
                let target_z = if surf_z == f64::NEG_INFINITY {
                    z_level
                } else {
                    surf_z + ctx.stock_to_leave
                };
                let z = (z_level + blend * (target_z - z_level)).max(target_z);
                path_3d.push(P3::new(p.x, p.y, z));
            }

            // Entry (link or rapid) + cut segment. Matches the gate in
            // clear_z_level_contour_parallel — see F-5 rationale there.
            if let Some(first) = path_3d.first() {
                let link_dist = ctx.max_link_dist;
                let should_link = last_pos.is_some_and(|lp| {
                    let dx = first.x - lp.x;
                    let dy = first.y - lp.y;
                    (dx * dx + dy * dy).sqrt() < link_dist
                        && is_clear_path_3d(
                            material_stock,
                            surface_hm,
                            lp,
                            *first,
                            ctx.stock_to_leave,
                            ctx.depth_per_pass,
                        )
                });
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
                    last_pos,
                    entry_seg,
                    ctx.safe_z,
                    ctx.tolerance,
                    ctx.min_cutting_radius,
                    &ctx.stamp_drape(),
                );
                push_segment_with_stamp(
                    segments,
                    material_stock,
                    ctx.lut,
                    ctx.tool_radius,
                    last_pos,
                    Adaptive3dSegment::Cut(path_3d),
                    ctx.safe_z,
                    ctx.tolerance,
                    ctx.min_cutting_radius,
                    &ctx.stamp_drape(),
                );
            }
        }

        threshold += base_step_cells;
    }

    Ok(())
}

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// Run waterline boundary cleanup at a given Z level.
///
/// When `slope_map` is provided, only traces contours through steep regions
/// (slope angle > 30°). This avoids re-tracing shallow areas that the
/// adaptive spiral already cleared.
#[allow(clippy::too_many_arguments)]
pub(super) fn waterline_cleanup(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    lut: &RadialProfileLUT,
    slope_map: &SlopeMap,
    material_stock: &mut TriDexelStock,
    z_level: f64,
    tool_radius: f64,
    cell_size: f64,
    safe_z: f64,
    tolerance: f64,
    min_cutting_radius: f64,
    stock_to_leave: f64,
    segments: &mut Vec<Adaptive3dSegment>,
    last_pos: &mut Option<P3>,
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
    };
    #[cfg(not(target_arch = "wasm32"))]
    let t_waterline = Instant::now();
    let waterline_scope = debug_ctx.map(|ctx| {
        ctx.start_span(
            "waterline_cleanup",
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

        push_segment_with_stamp(
            segments,
            material_stock,
            lut,
            tool_radius,
            last_pos,
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
            last_pos,
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

/// Sample the current dexel stock top at world XY. Returns `None`
/// when the XY falls outside the grid (caller should fall back to
/// the conservative full-safe_z plunge). Returns `f64::NEG_INFINITY`
/// for the "everything cleared" case is collapsed to `None` too —
/// the caller can't usefully rapid down to negative infinity.
fn sample_stock_top_at(material_stock: &TriDexelStock, x: f64, y: f64) -> Option<f64> {
    let (row, col) = material_stock.z_grid.world_to_cell(x, y)?;
    let top_z = stock_top_z_at(material_stock, row, col);
    top_z.is_finite().then_some(top_z)
}

// ── 2.5D slice adaptive (AgentSearch strategy) ─────────────────────────

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
/// one next instead of following marching-squares scan order. O(n²),
/// fine for the handful of regions a Z-level produces.
#[allow(clippy::indexing_slicing)] // visited/anchors indexed by enumerate idx
fn nearest_neighbor_order(anchors: &[(f64, f64)], start: (f64, f64)) -> Vec<usize> {
    let mut visited = vec![false; anchors.len()];
    let mut order: Vec<usize> = Vec::with_capacity(anchors.len());
    let mut cur = start;
    for _ in 0..anchors.len() {
        let mut best: Option<usize> = None;
        let mut best_d = f64::INFINITY;
        for (i, a) in anchors.iter().enumerate() {
            if visited[i] {
                continue;
            }
            let d = (a.0 - cur.0).powi(2) + (a.1 - cur.1).powi(2);
            if d < best_d {
                best_d = d;
                best = Some(i);
            }
        }
        if let Some(i) = best {
            visited[i] = true;
            order.push(i);
            cur = anchors[i];
        }
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
        #[allow(clippy::indexing_slicing)] // SAFETY: i < n, (i+1) % n < n
        let a = points[i];
        #[allow(clippy::indexing_slicing)]
        let b = points[(i + 1) % n];
        acc += a.x * b.y - b.x * a.y;
    }
    0.5 * acc
}

/// AgentSearch via 2.5D slices: at each Z-level, extract the 2D
/// material polygon via marching squares, then run the proven 2D
/// `adaptive_segments_with_debug` on it. Lift the resulting 2D path
/// back to 3D (Z clamped to `z_level` or surface+stock_to_leave) and
/// stamp the dexel stock along it.
///
/// This replaces the ~700-line 3D agent-based search that struggled
/// with surface-following, axial engagement, and boundary walking by
/// delegating to the working 2D adaptive implementation. The trade-off
/// is that the tool stays at a fixed Z within each slab (no per-step
/// terrain follow) — acceptable for roughing; finish passes handle
/// the staircase.
/// Machine rapid rate (mm/min) assumed for the feed-vs-rapid crossover.
/// The planner has no machine context at this layer; 5000 mm/min matches
/// the Shapeoko-class grbl default the retired 70 mm constant was tuned
/// against. Worst case of a wrong guess is a suboptimal link/retract
/// choice, never an unsafe move.
const ASSUMED_RAPID_MM_MIN: f64 = 5000.0;

/// Final-approach distance descended at plunge rate after the rapid
/// descent (mirrors `RAPID_DESCENT_BUFFER_MM` in `path.rs`).
const CROSSOVER_PLUNGE_BUFFER_MM: f64 = 0.5;

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
        + (depth - CROSSOVER_PLUNGE_BUFFER_MM).max(0.0) / rapid
        + CROSSOVER_PLUNGE_BUFFER_MM.min(depth) / plunge;
    overhead_s / (1.0 / feed - 1.0 / rapid)
}

#[allow(clippy::too_many_arguments, clippy::indexing_slicing)]
pub(super) fn clear_z_level_agent_2d_slice(
    ctx: &ClearZLevelContext<'_>,
    material_stock: &mut TriDexelStock,
    surface_hm: &SurfaceHeightmap,
    z_level: f64,
    segments: &mut Vec<Adaptive3dSegment>,
    last_pos: &mut Option<P3>,
    // Stage 4 — per-toolpath planner-engagement sampler accumulator. The
    // ContourSpiral strategy appends `(lifted_point, leading_arc_frac)` for
    // every emitted cut point; other strategies leave it untouched.
    planner_eng: &mut Vec<(P3, f64)>,
    region: Option<&MaterialRegion>,
    level_marker: Option<Adaptive3dRuntimeEvent>,
    cancel: &dyn CancelCheck,
) -> Result<(), Cancelled> {
    let remaining = if let Some(r) = region {
        material_remaining_in_region(material_stock, surface_hm, z_level, ctx.stock_to_leave, r)
    } else {
        material_remaining_at_level(material_stock, surface_hm, z_level, ctx.stock_to_leave)
    };
    if remaining.cells_with_material < MIN_CELLS_TO_CLEAR {
        return Ok(());
    }

    let level_scope = ctx.debug.as_ref().map(|debug_ctx| {
        let label = if let Some(r) = region {
            format!(
                "Z {:.3} region rows {}..{} cols {}..{}",
                z_level, r.row_min, r.row_max, r.col_min, r.col_max
            )
        } else {
            format!("Z {:.3}", z_level)
        };
        debug_ctx.start_span("z_level", label)
    });
    if let Some(scope) = level_scope.as_ref() {
        scope.set_z_level(z_level);
        scope.set_counter("remaining_before", remaining.fraction());
    }
    let level_ctx = level_scope.as_ref().map(|scope| scope.context());

    // 1. Material boolean grid at this Z-level (includes 1-cell air padding).
    let (material_grid, rows, cols, origin_x, origin_y, cell_size) = build_material_bool_grid(
        material_stock,
        surface_hm,
        z_level,
        ctx.stock_to_leave,
        region,
        ctx.shallow_mask,
    );
    if !material_grid.iter().any(|&b| b) {
        return Ok(());
    }

    // 2. Marching squares → polygon contours.
    let contours = crate::contour_extract::marching_squares_bool_grid(
        &material_grid,
        rows,
        cols,
        origin_x,
        origin_y,
        cell_size,
    );
    if contours.is_empty() {
        return Ok(());
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
        .map(|pts| crate::polygon::Polygon2 {
            exterior: pts,
            holes: Vec::new(),
            closed: true,
        })
        .collect();
    if single_loops.is_empty() {
        return Ok(());
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
        return Ok(());
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
        #[allow(clippy::indexing_slicing)] // order entries are valid region indices
        let reordered: Vec<_> = order.into_iter().map(|i| regions[i].clone()).collect();
        regions = reordered;
    }

    let mut level_metrics = ZLevelPlanMetrics {
        available: true,
        marching_squares_regions: region_count_before,
        region_areas_mm2,
        dropped_micro_region_count: dropped_micro,
        perimeter_sweep_length_mm: 0.0,
        agent_walk_cut_length_mm: 0.0,
        residual_cleanup_cell_count: 0,
        dropped_short_region_count: 0,
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
        initial_stock: None,
        cleanup_strategy: crate::adaptive::CleanupStrategy::ContourParallelHybrid,
        engagement_measure: ctx.engagement_measure,
        // ContourSpiral (3D) routes through this same dispatch with the
        // spiral as the per-slice generator; AgentSearch keeps the agent.
        path_strategy: if matches!(ctx.clearing_strategy, ClearingStrategy3d::ContourSpiral) {
            crate::adaptive::PathStrategy2d::ContourSpiral
        } else {
            crate::adaptive::PathStrategy2d::Agent
        },
        // Operator-tunable trochoid cap ("Nibble" dial). Fall back to the
        // tuned const for any non-finite / non-positive value so a bad
        // config can never disable load capping outright.
        trochoid_cap_mult: if ctx.trochoid_cap_mult.is_finite() && ctx.trochoid_cap_mult > 0.0 {
            ctx.trochoid_cap_mult
        } else {
            TROCHOID_CAP_MULT_3D
        },
    };

    // 5. Lift 2D points to 3D, respecting terrain peaks above z_level.
    let lift = |p: P2| -> P3 {
        let surf_z = surface_hm.z_or_bbox_floor_at_world(p.x, p.y);
        let z = if surf_z == f64::NEG_INFINITY {
            z_level
        } else {
            (surf_z + ctx.stock_to_leave).max(z_level)
        };
        P3::new(p.x, p.y, z)
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
    for (region_idx, region_polygon) in regions.iter().enumerate() {
        let region_scope = level_ctx.as_ref().map(|debug_ctx| {
            debug_ctx.start_span(
                "agent2d_region",
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
                    forecast_mm += polyline_xy_length(&path_2d);
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
                    forecast_mm += polyline_xy_length(&path_2d);
                }
            }
            // Quick exit: if even the perimeter alone clears the bar, skip
            // the (more expensive) 2D adaptive forecast.
            if forecast_mm < ctx.min_region_cut_length_mm {
                let segs_forecast = crate::adaptive::adaptive_segments_with_debug(
                    region_polygon,
                    &params_2d,
                    cancel,
                    None,
                    None,
                )?;
                for seg in &segs_forecast {
                    if let crate::adaptive::AdaptiveSegment::Cut(path_2d) = seg {
                        forecast_mm += polyline_xy_length(path_2d);
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
                dropped_short_regions += 1;
                if let Some(scope) = region_scope.as_ref() {
                    scope.set_counter("skipped_f038", 1.0);
                    scope.set_counter("forecast_cut_length_mm", forecast_mm);
                }
                continue;
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
        const PERIMETER_INSET_MARGIN_MM: f64 = 0.25;
        let inset_polygons = crate::polygon::offset_polygon(
            region_polygon,
            ctx.tool_radius + PERIMETER_INSET_MARGIN_MM,
        );
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
                    // Sample the stock top BEFORE we stamp the rapid /
                    // cut for this loop, so the rapid-floor reflects
                    // material state from prior passes only.
                    let rapid_floor = sample_stock_top_at(material_stock, first.x, first.y);
                    let entry_seg = match rapid_floor {
                        Some(top_z) => Adaptive3dSegment::RapidWithFloor {
                            entry: first,
                            rapid_floor_z: top_z,
                        },
                        None => Adaptive3dSegment::Rapid(first),
                    };
                    push_segment_with_stamp(
                        segments,
                        material_stock,
                        ctx.lut,
                        ctx.tool_radius,
                        last_pos,
                        entry_seg,
                        ctx.safe_z,
                        ctx.tolerance,
                        ctx.min_cutting_radius,
                        &ctx.stamp_drape(),
                    );
                }
                if path_3d.len() >= 2 {
                    level_metrics.perimeter_sweep_length_mm += polyline_length_3d(&path_3d);
                    push_segment_with_stamp(
                        segments,
                        material_stock,
                        ctx.lut,
                        ctx.tool_radius,
                        last_pos,
                        Adaptive3dSegment::Cut(path_3d),
                        ctx.safe_z,
                        ctx.tolerance,
                        ctx.min_cutting_radius,
                        &ctx.stamp_drape(),
                    );
                    cut_count += 1;
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
                    let rapid_floor = sample_stock_top_at(material_stock, first.x, first.y);
                    let entry_seg = match rapid_floor {
                        Some(top_z) => Adaptive3dSegment::RapidWithFloor {
                            entry: first,
                            rapid_floor_z: top_z,
                        },
                        None => Adaptive3dSegment::Rapid(first),
                    };
                    push_segment_with_stamp(
                        segments,
                        material_stock,
                        ctx.lut,
                        ctx.tool_radius,
                        last_pos,
                        entry_seg,
                        ctx.safe_z,
                        ctx.tolerance,
                        ctx.min_cutting_radius,
                        &ctx.stamp_drape(),
                    );
                }
                if path_3d.len() >= 2 {
                    level_metrics.perimeter_sweep_length_mm += polyline_length_3d(&path_3d);
                    push_segment_with_stamp(
                        segments,
                        material_stock,
                        ctx.lut,
                        ctx.tool_radius,
                        last_pos,
                        Adaptive3dSegment::Cut(path_3d),
                        ctx.safe_z,
                        ctx.tolerance,
                        ctx.min_cutting_radius,
                        &ctx.stamp_drape(),
                    );
                    cut_count += 1;
                }
            }
        }

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
        const SIMPLIFY_INSET: f64 = 0.3;
        const SIMPLIFY_TOLERANCE: f64 = 0.3;
        let smoothed_polygons = if !matches!(
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
                    poly
                })
                .filter(|p| p.exterior.len() >= 3)
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let polygon_for_adaptive: &crate::polygon::Polygon2 = if smoothed_polygons.is_empty() {
            region_polygon
        } else {
            #[allow(clippy::indexing_slicing)] // checked non-empty above
            &smoothed_polygons[0]
        };

        // Stage 4 — collect the contour-spiral's per-point predicted
        // leading-arc engagement for this slice (empty for non-spiral
        // strategies). Keyed by 2D position below so it survives the
        // residue-cleanup segment reshuffling, then lifted to 3D and
        // appended to the per-toolpath planner-engagement sampler.
        let mut slice_eng_2d: Vec<(P2, f64)> = Vec::new();
        let segs_2d_raw = crate::adaptive::adaptive_segments_with_debug(
            polygon_for_adaptive,
            &params_2d,
            cancel,
            region_scope.as_ref().map(|s| s.context()).as_ref(),
            Some(&mut slice_eng_2d),
        )?;
        // Spatial lookup: quantised 2D position → predicted engagement.
        // Positions are exact f64 from the spiral's own emit, so a
        // fixed-point key reproduces them without float-equality hazard.
        let eng_lookup: std::collections::HashMap<(i64, i64), f64> = slice_eng_2d
            .iter()
            .map(|(p, e)| ((quantize_coord(p.x), quantize_coord(p.y)), *e))
            .collect();

        // For non-Legacy cleanup strategies, run the same cleanup
        // post-process the 2D top-level entry point runs. adaptive3d
        // calls `adaptive_segments_with_debug` directly (not the
        // toolpath-level entry), so without this step the new
        // strategies (helical entry + narrow-exit + cap-pass-1) exit
        // early and leave residue uncleared — the cleanup catches it
        // with boundary walks + contour-parallel offsets + mop
        // patches.
        let segs_2d_raw = match params_2d.cleanup_strategy {
            crate::adaptive::CleanupStrategy::Legacy => segs_2d_raw,
            crate::adaptive::CleanupStrategy::ResidueMop
            | crate::adaptive::CleanupStrategy::ContourParallelNarrow => {
                crate::adaptive::path::apply_residue_mop_cleanup(
                    polygon_for_adaptive,
                    &params_2d,
                    &segs_2d_raw,
                )
            }
            crate::adaptive::CleanupStrategy::ContourParallelHybrid => {
                crate::adaptive::path::apply_contour_parallel_residue_cleanup(
                    polygon_for_adaptive,
                    &params_2d,
                    &segs_2d_raw,
                )
            }
        };

        // F-038: filter out tiny entry-plunge → cut groups before emission.
        //
        // The 2D adaptive emits its output as a sequence of segments shaped
        // like:
        //
        //   [Marker?, Rapid(entry), Cut(path), Cut(path), ..., Rapid(entry), ...]
        //
        // Each `Rapid` is the start of a new "pass" — downstream this becomes
        // a retract + rapid-XY + peck-plunge + Cut sequence. On terrain-shaped
        // models the planner emits many of these passes, and a significant
        // fraction cut < 5 mm of material before the next retract. On the
        // Wanaka Back Rough .nc this manifests as 149 F750 entry plunges, 90
        // of which cut ≤ 10 mm. Each one spends 0.3–0.5 s on retract/plunge
        // overhead per 100 ms or less of real cutting.
        //
        // Group segments by Rapid boundary; if a group's total Cut XY length
        // is below `min_region_cut_length_mm`, drop the whole group (the
        // entry Rapid + its Cuts). The cutter never visits that micro-area;
        // material is left for finishing passes (which is fine — these
        // micro-bridges are below roughing resolution).
        //
        // Always keep the very first group (it provides the region's initial
        // tool position). Always keep the last group's residual Marker/Link
        // segments so the per-region debug spans stay coherent.
        let segs_2d: Vec<crate::adaptive::AdaptiveSegment> = if ctx.min_region_cut_length_mm > 0.0 {
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
                        cut_len += polyline_xy_length(path_2d);
                    }
                }
                // Always keep the first group — it sets up the region's
                // starting position. Drop interior + trailing groups
                // below threshold; the trailing group on terrain runs
                // is usually a sub-tool residual the finishing pass
                // will cover anyway.
                let _ = group_total;
                let drop = gi > 0
                    && cut_len < ctx.min_region_cut_length_mm
                    && group
                        .iter()
                        .any(|s| matches!(s, crate::adaptive::AdaptiveSegment::Rapid(_)));
                if drop {
                    dropped_groups += 1;
                    continue;
                }
                kept_segs.extend(group);
            }
            if dropped_groups > 0 {
                debug!(
                    z = z_level,
                    region = region_idx + 1,
                    dropped_groups,
                    group_total,
                    threshold_mm = ctx.min_region_cut_length_mm,
                    "F-038: dropped short cut groups inside 2D adaptive output"
                );
                dropped_short_regions += dropped_groups;
            }
            kept_segs
        } else {
            segs_2d_raw
        };

        for seg in segs_2d {
            match seg {
                crate::adaptive::AdaptiveSegment::Cut(path_2d) => {
                    if path_2d.is_empty() {
                        continue;
                    }
                    let path_3d: Vec<P3> = path_2d.iter().map(|&p| lift(p)).collect();
                    // Stage 4 — record the planner's predicted leading-arc
                    // engagement at each lifted cut point (looked up by 2D
                    // position; misses, e.g. residue-mop cleanup cuts, are
                    // simply absent and the modulator falls back there).
                    if !eng_lookup.is_empty() {
                        for (&p2, &p3) in path_2d.iter().zip(path_3d.iter()) {
                            if let Some(&e) =
                                eng_lookup.get(&(quantize_coord(p2.x), quantize_coord(p2.y)))
                            {
                                planner_eng.push((p3, e));
                            }
                        }
                    }
                    level_metrics.agent_walk_cut_length_mm += polyline_length_3d(&path_3d);
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
                    let mut engaged: Vec<bool> = path_3d
                        .iter()
                        .map(|p| match sample_stock_top_at(material_stock, p.x, p.y) {
                            Some(top) => top > p.z + AIR_THRESHOLD_MM,
                            None => false,
                        })
                        .collect();
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
                                for is_engaged in
                                    engaged.iter_mut().take(run_end + 1).skip(run_start)
                                {
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
                    if let Some(last) = path_3d.last().copied() {
                        *last_pos = Some(last);
                    }
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
                    // samples chops the path. AgentSearch then plans many
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
                        let large_z = dz.abs() > z_drop_threshold || abs_slope > PLUNGE_SLOPE_LIMIT;
                        let engagement_change = engaged[i] != sub_engaged;
                        if large_z || engagement_change {
                            // Flush the current run.
                            let run_len = i - sub_start;
                            if run_len >= 1 {
                                if sub_engaged {
                                    if run_len >= 2 {
                                        push_segment_with_stamp(
                                            segments,
                                            material_stock,
                                            ctx.lut,
                                            ctx.tool_radius,
                                            last_pos,
                                            Adaptive3dSegment::Cut(path_3d[sub_start..i].to_vec()),
                                            ctx.safe_z,
                                            ctx.tolerance,
                                            ctx.min_cutting_radius,
                                            &ctx.stamp_drape(),
                                        );
                                        cut_count += 1;
                                    }
                                } else {
                                    // Air run → single RapidWithFloor to
                                    // the end of the run (the cutter just
                                    // traverses cleared territory). No
                                    // stamping (rapids don't cut).
                                    let end_pt = path_3d[i.saturating_sub(1)];
                                    let rapid_floor =
                                        sample_stock_top_at(material_stock, end_pt.x, end_pt.y);
                                    let entry_seg = match rapid_floor {
                                        Some(top_z) => Adaptive3dSegment::RapidWithFloor {
                                            entry: end_pt,
                                            rapid_floor_z: top_z,
                                        },
                                        None => Adaptive3dSegment::Rapid(end_pt),
                                    };
                                    push_segment_with_stamp(
                                        segments,
                                        material_stock,
                                        ctx.lut,
                                        ctx.tool_radius,
                                        last_pos,
                                        entry_seg,
                                        ctx.safe_z,
                                        ctx.tolerance,
                                        ctx.min_cutting_radius,
                                        &ctx.stamp_drape(),
                                    );
                                }
                            }
                            // After a large_z split, also rapid-position
                            // to the new point so the cutter retracts
                            // before continuing.
                            if large_z {
                                let p3 = path_3d[i];
                                let rapid_floor = sample_stock_top_at(material_stock, p3.x, p3.y);
                                let entry_seg = match rapid_floor {
                                    Some(top_z) => Adaptive3dSegment::RapidWithFloor {
                                        entry: p3,
                                        rapid_floor_z: top_z,
                                    },
                                    None => Adaptive3dSegment::Rapid(p3),
                                };
                                push_segment_with_stamp(
                                    segments,
                                    material_stock,
                                    ctx.lut,
                                    ctx.tool_radius,
                                    last_pos,
                                    entry_seg,
                                    ctx.safe_z,
                                    ctx.tolerance,
                                    ctx.min_cutting_radius,
                                    &ctx.stamp_drape(),
                                );
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
                                push_segment_with_stamp(
                                    segments,
                                    material_stock,
                                    ctx.lut,
                                    ctx.tool_radius,
                                    last_pos,
                                    Adaptive3dSegment::Cut(path_3d[sub_start..].to_vec()),
                                    ctx.safe_z,
                                    ctx.tolerance,
                                    ctx.min_cutting_radius,
                                    &ctx.stamp_drape(),
                                );
                                cut_count += 1;
                            }
                        } else if let Some(end_pt) = path_3d.last().copied() {
                            let rapid_floor =
                                sample_stock_top_at(material_stock, end_pt.x, end_pt.y);
                            let entry_seg = match rapid_floor {
                                Some(top_z) => Adaptive3dSegment::RapidWithFloor {
                                    entry: end_pt,
                                    rapid_floor_z: top_z,
                                },
                                None => Adaptive3dSegment::Rapid(end_pt),
                            };
                            push_segment_with_stamp(
                                segments,
                                material_stock,
                                ctx.lut,
                                ctx.tool_radius,
                                last_pos,
                                entry_seg,
                                ctx.safe_z,
                                ctx.tolerance,
                                ctx.min_cutting_radius,
                                &ctx.stamp_drape(),
                            );
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
                    let rapid_floor = sample_stock_top_at(material_stock, p3.x, p3.y);
                    let entry_seg = match rapid_floor {
                        Some(top_z) => Adaptive3dSegment::RapidWithFloor {
                            entry: p3,
                            rapid_floor_z: top_z,
                        },
                        None => Adaptive3dSegment::Rapid(p3),
                    };
                    push_segment_with_stamp(
                        segments,
                        material_stock,
                        ctx.lut,
                        ctx.tool_radius,
                        last_pos,
                        entry_seg,
                        ctx.safe_z,
                        ctx.tolerance,
                        ctx.min_cutting_radius,
                        &ctx.stamp_drape(),
                    );
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
                    let rapid_floor = sample_stock_top_at(material_stock, p3.x, p3.y);
                    let entry_seg = match rapid_floor {
                        Some(top_z) => Adaptive3dSegment::RapidWithFloor {
                            entry: p3,
                            rapid_floor_z: top_z,
                        },
                        None => Adaptive3dSegment::Rapid(p3),
                    };
                    push_segment_with_stamp(
                        segments,
                        material_stock,
                        ctx.lut,
                        ctx.tool_radius,
                        last_pos,
                        entry_seg,
                        ctx.safe_z,
                        ctx.tolerance,
                        ctx.min_cutting_radius,
                        &ctx.stamp_drape(),
                    );
                }
                crate::adaptive::AdaptiveSegment::Marker(_) => {
                    // 2D runtime events don't translate cleanly to 3D; swallow.
                    // The debug trace captured them already under agent2d_region.
                }
            }
        }
    }

    // F-038: post-emission coalescing pass.
    //
    // The per-region loop above pushes one `Rapid`/`RapidWithFloor` per
    // 2D-adaptive entry plus one for each engagement transition the lifter
    // detects. A subset of those entries reach the .nc as full peck-plunges
    // that cut zero material before the next entry — either because the
    // engagement subdivider demoted the entire following `Cut` to air (no
    // engaged sub-runs) or because the entry simply landed on already-
    // cleared dexel territory.
    //
    // Walk the level-marker range and drop any entry-style segment that's
    // immediately followed by another entry-style segment (no `Cut` or
    // `Link` in between). The downstream `segments_to_toolpath` would emit
    // a full retract+rapid+peck-plunge for both — pure overhead on the
    // first one because its plunge gets re-stamped at the second's XY
    // before any cutting happens.
    //
    // This complements the in-loop group filter: that one drops short-cut
    // *passes*, this one drops degenerate zero-cut *entries* the lifter
    // produces after engagement subdivision.
    let coalesced_entries = if ctx.min_region_cut_length_mm > 0.0 {
        coalesce_redundant_entries(segments, level_marker_index)
    } else {
        0
    };
    if coalesced_entries > 0 {
        debug!(
            z = z_level,
            coalesced_entries, "F-038: coalesced redundant back-to-back entries"
        );
    }
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

/// F-038 helper. Walk `segments[start..]` and drop `Rapid`/`RapidWithFloor`
/// segments whose only successors before the next entry are `Marker` events.
/// I.e. collapse `[Rapid, (Marker)*, Rapid, ...]` to `[Rapid, ...]` so the
/// downstream emitter doesn't burn a full retract+plunge cycle on the first
/// rapid only to immediately retract again for the second.
///
/// Returns the number of redundant entries removed.
fn coalesce_redundant_entries(
    segments: &mut Vec<Adaptive3dSegment>,
    level_marker_index: Option<usize>,
) -> usize {
    let start = level_marker_index.map(|i| i + 1).unwrap_or(0);
    if start >= segments.len() {
        return 0;
    }
    let mut removed = 0usize;
    let mut i = start;
    while i < segments.len() {
        let Some(seg_i) = segments.get(i) else {
            break;
        };
        let is_entry_i = matches!(
            seg_i,
            Adaptive3dSegment::Rapid(_) | Adaptive3dSegment::RapidWithFloor { .. }
        );
        if !is_entry_i {
            i += 1;
            continue;
        }
        // Scan forward, skipping markers, looking for the next non-marker.
        let mut j = i + 1;
        while let Some(seg_j) = segments.get(j) {
            if matches!(seg_j, Adaptive3dSegment::Marker(_)) {
                j += 1;
            } else {
                break;
            }
        }
        let Some(seg_j) = segments.get(j) else {
            break;
        };
        let next_is_entry = matches!(
            seg_j,
            Adaptive3dSegment::Rapid(_) | Adaptive3dSegment::RapidWithFloor { .. }
        );
        if next_is_entry {
            // Drop segment[i]; keep markers (they have annotation value)
            // and the following entry.
            segments.remove(i);
            removed += 1;
            // Stay at i — the new occupant may itself be a redundant entry.
            continue;
        }
        i += 1;
    }
    removed
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

/// F-038: 2D XY polyline length used by the AgentSearch forecaster to decide
/// whether a marching-squares region's expected cut footprint is large enough
/// to justify an entry plunge. XY-only on purpose — the 3D-lift later folds
/// terrain Z in, but for the "is this worth the entry" question only the
/// horizontal footprint matters (the cutter still descends + retracts even
/// on flat terrain).
fn polyline_xy_length(path: &[P2]) -> f64 {
    path.windows(2)
        .map(|pair| {
            let Some(a) = pair.first() else {
                return 0.0;
            };
            let Some(b) = pair.get(1) else {
                return 0.0;
            };
            let dx = b.x - a.x;
            let dy = b.y - a.y;
            (dx * dx + dy * dy).sqrt()
        })
        .sum()
}

fn polyline_length_3d(path: &[P3]) -> f64 {
    path.windows(2)
        .map(|pair| {
            let Some(a) = pair.first() else {
                return 0.0;
            };
            let Some(b) = pair.get(1) else {
                return 0.0;
            };
            (*b - *a).norm()
        })
        .sum()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
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
