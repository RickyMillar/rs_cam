//! 3D adaptive clearing with constant engagement on mesh surfaces.
//!
//! Maintains constant tool engagement while following an STL mesh surface.
//! Uses tri-dexel stock for volumetric material tracking, drop-cutter
//! queries for Z following, and precomputed surface heightmap for fast
//! engagement computation.
//!
//! Key differences from 2D adaptive:
//! - Material state: tri-dexel stock (volumetric interval lists)
//! - Z at each step: from point_drop_cutter (not constant)
//! - Engagement: "material above surface" not "material vs cleared"
//! - Multi-level: Z levels from stock_top down to mesh surface
//! - Boundary cleanup: waterline contours (not polygon offset contours)

use crate::debug_trace::ToolpathDebugContext;
use crate::dexel::ray_top;
use crate::dexel_stock::TriDexelStock;
use crate::interrupt::{CancelCheck, Cancelled};
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::tool::MillingCutter;
use crate::toolpath::Toolpath;

use tracing::info;

mod clearing;
mod path;
mod search;
use path::{adaptive_3d_segments, runtime_annotations_to_labels, segments_to_toolpath};

/// Region ordering strategy for 3D adaptive clearing.
///
/// `Global` clears all areas at each Z level before moving to the next (default).
/// `ByArea` detects connected material regions via flood fill and clears each
/// region fully (all Z levels) before moving to the next, reducing tool travel.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RegionOrdering {
    /// Clear all areas at each Z level globally (default, backward compat).
    #[default]
    Global,
    /// Detect connected pockets and clear each fully before moving to the next.
    ByArea,
}

/// Roughing strategy for 3D clearing.
///
/// `ContourParallel` — fast, predictable contour-offset pocketing via EDT.
///   Fixed stepover, concentric contours from boundary inward. Best for
///   bulk roughing where speed matters more than constant engagement.
///   The default.
///
/// `Adaptive` — curvature-adjusted EDT clearing with variable stepover.
///   Produces shorter cutting distance than ContourParallel on curved
///   terrain by adapting the offset spacing to local curvature, without
///   paying the per-step direction-search cost of AgentSearch.
///
/// `AgentSearch` — per-step direction search with preflight skip and
///   widen-band recovery. Slower to generate than ContourParallel or
///   Adaptive but offers finer per-step control; retained for advanced
///   cases where the geometry defeats the EDT-based strategies.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ClearingStrategy3d {
    /// Per-step direction search with preflight skip and widen-band
    /// recovery. Slow to generate; reach for it when ContourParallel or
    /// Adaptive produce uncut bands on difficult geometry.
    AgentSearch,
    /// Fast contour-parallel offset clearing via EDT (default).
    #[default]
    ContourParallel,
    /// Curvature-adjusted adaptive clearing via variable-offset EDT.
    Adaptive,
}

/// Entry strategy for 3D adaptive (replaces vertical plunge).
#[derive(Debug, Clone, Copy, Default)]
pub enum EntryStyle3d {
    /// Vertical plunge (default prior behavior).
    #[default]
    Plunge,
    /// Helical entry: spiral down with given radius and pitch (mm/rev).
    Helix { radius: f64, pitch: f64 },
    /// Ramp entry: descend at a shallow angle along the next cutting direction.
    Ramp { max_angle_deg: f64 },
}

/// Parameters for 3D adaptive clearing.
pub struct Adaptive3dParams {
    /// Engagement radius — the cutter's actual contact radius at `depth_per_pass`
    /// below the tip. Used for stepover, region detection, and material clearing
    /// modeling. For flat/ball cutters this equals the nominal radius; for
    /// tapered cutters it's narrower than the shank radius.
    pub tool_radius: f64,
    /// Envelope radius — the widest extent of the cutter at any height (shank
    /// radius for tapered tools). Used only for keep-out / bbox margins so the
    /// tool's shank doesn't overrun the workpiece footprint.
    pub envelope_radius: f64,
    pub stepover: f64,
    pub depth_per_pass: f64,
    pub stock_to_leave: f64,
    pub feed_rate: f64,
    pub plunge_rate: f64,
    pub safe_z: f64,
    pub tolerance: f64,
    pub min_cutting_radius: f64,
    pub stock_top_z: f64,
    /// Entry strategy (default: Plunge for backward compat).
    pub entry_style: EntryStyle3d,
    /// Fine stepdown: when set, insert intermediate Z levels at this interval.
    pub fine_stepdown: Option<f64>,
    /// Detect flat areas in the mesh and insert Z levels at shelf heights.
    pub detect_flat_areas: bool,
    /// Maximum distance to stay down between passes (default: tool_radius * 6).
    pub max_stay_down_dist: Option<f64>,
    /// Region ordering strategy (default: Global for backward compat).
    pub region_ordering: RegionOrdering,
    /// Pre-machined stock for two-sided machining.
    /// When Some, used as starting material instead of a fresh block at stock_top_z.
    pub initial_stock: Option<TriDexelStock>,
    /// Clearing strategy per Z level (default: ContourParallel).
    pub clearing_strategy: ClearingStrategy3d,
    /// Blend Z toward terrain surface across contour offsets.
    /// When true, outer contours stay near z_level and inner contours
    /// progressively descend toward the surface. Best for terrain/relief.
    /// When false (default), all contours cut at z_level. Best for pockets.
    pub z_blend: bool,
    /// Optional 2D boundary polygon (e.g. model silhouette) the cutter
    /// center must stay inside. Cells outside this boundary are pre-cleared
    /// in the internal material stock so the bool-grid polygon at every
    /// z-level reflects the boundary, not just the stock bbox. Without this,
    /// AgentSearch's polygon at top z-levels covers the full stock; the
    /// downstream toolpath clip then converts the outside-boundary cuts to
    /// rapids, leaving stock unstamped, and deeper z-levels then bite
    /// through fresh stock with full-depth axial DOC. See investigation
    /// log O5b for the wanaka repro.
    pub boundary: Option<crate::polygon::Polygon2>,
}

// SurfaceHeightmap is now in crate::slope (shared across finishing strategies)

// ── Helpers mapping TriDexelStock to f64 world used by adaptive ────────

/// Top Z at (row, col) from the Z-grid, as f64. Returns `bottom_z` if the
/// ray is empty (no material).
#[inline]
pub(super) fn stock_top_z_at(stock: &TriDexelStock, row: usize, col: usize) -> f64 {
    ray_top(stock.z_grid.ray(row, col))
        .map(|z| z as f64)
        .unwrap_or(stock.stock_bbox.min.z)
}

/// Whether the Z-grid ray at (row, col) has material above `floor` (f64).
#[inline]
pub(super) fn stock_has_material_above(
    stock: &TriDexelStock,
    row: usize,
    col: usize,
    floor: f64,
) -> bool {
    let ray = stock.z_grid.ray(row, col);
    // Any segment whose exit > floor means material above floor.
    ray.iter().any(|seg| seg.exit as f64 > floor)
}

// ── Segment types ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ZLevelPlanMetrics {
    pub available: bool,
    pub marching_squares_regions: usize,
    pub region_areas_mm2: Vec<f64>,
    pub dropped_micro_region_count: usize,
    pub perimeter_sweep_length_mm: f64,
    pub agent_walk_cut_length_mm: f64,
    pub residual_cleanup_cell_count: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Adaptive3dRuntimeEvent {
    RegionStart {
        region_index: usize,
        region_total: usize,
        cell_count: usize,
    },
    RegionZLevel {
        region_index: usize,
        z_level: f64,
        level_index: usize,
        level_total: usize,
        metrics: ZLevelPlanMetrics,
    },
    GlobalZLevel {
        z_level: f64,
        level_index: usize,
        level_total: usize,
        metrics: ZLevelPlanMetrics,
    },
    WaterlineCleanup,
    PassEntry {
        pass_index: usize,
        entry_x: f64,
        entry_y: f64,
        entry_z: f64,
    },
    PassPreflightSkip {
        pass_index: usize,
    },
    PassSummary {
        pass_index: usize,
        step_count: usize,
        exit_reason: String,
        yield_ratio: f64,
        short: bool,
    },
}

impl Adaptive3dRuntimeEvent {
    pub fn set_z_level_metrics(&mut self, metrics: ZLevelPlanMetrics) {
        match self {
            Self::RegionZLevel { metrics: slot, .. } | Self::GlobalZLevel { metrics: slot, .. } => {
                *slot = metrics;
            }
            Self::RegionStart { .. }
            | Self::WaterlineCleanup
            | Self::PassEntry { .. }
            | Self::PassPreflightSkip { .. }
            | Self::PassSummary { .. } => {}
        }
    }

    pub fn label(&self) -> String {
        match self {
            Self::RegionStart {
                region_index,
                region_total,
                cell_count,
            } => format!("Region {region_index}/{region_total} ({cell_count} cells)"),
            Self::RegionZLevel {
                region_index,
                z_level,
                level_index,
                level_total,
                metrics: _,
            } => format!(
                "Region {region_index} — Z {:.1} ({level_index}/{level_total})",
                z_level
            ),
            Self::GlobalZLevel {
                z_level,
                level_index,
                level_total,
                metrics: _,
            } => format!("Adaptive Z {:.1} ({level_index}/{level_total})", z_level),
            Self::WaterlineCleanup => "Waterline cleanup".to_owned(),
            Self::PassEntry {
                pass_index,
                entry_x,
                entry_y,
                entry_z,
            } => {
                format!("Pass {pass_index} — entry at ({entry_x:.1}, {entry_y:.1}) Z {entry_z:.1}")
            }
            Self::PassPreflightSkip { pass_index } => {
                format!("Pass {pass_index} — preflight skip (no viable direction)")
            }
            Self::PassSummary {
                pass_index,
                step_count,
                exit_reason,
                yield_ratio,
                short,
            } => {
                if *short {
                    format!(
                        "Pass {pass_index} — short ({step_count} steps, {exit_reason}, yield {yield_ratio:.3})"
                    )
                } else {
                    format!(
                        "Pass {pass_index} — {step_count} steps ({exit_reason}, yield {yield_ratio:.3})"
                    )
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Adaptive3dRuntimeAnnotation {
    pub move_index: usize,
    pub event: Adaptive3dRuntimeEvent,
}

/// Generate a 3D adaptive clearing toolpath for roughing a mesh surface.
///
/// Starting from flat stock at `stock_top_z`, roughs out material following
/// the STL mesh surface with constant engagement control. Multi-level
/// passes from top to bottom, waterline boundary cleanup at each level.
#[tracing::instrument(skip(mesh, index, cutter, params), fields(tool_radius = params.tool_radius, stepover = params.stepover))]
// infallible: cancel closure always returns false, so Cancelled is unreachable
#[allow(clippy::expect_used)]
pub fn adaptive_3d_toolpath(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &Adaptive3dParams,
) -> Toolpath {
    let never_cancel = || false;
    adaptive_3d_toolpath_with_cancel(mesh, index, cutter, params, &never_cancel)
        .expect("non-cancellable adaptive3d should never be cancelled")
}

pub fn adaptive_3d_toolpath_with_cancel(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &Adaptive3dParams,
    cancel: &dyn CancelCheck,
) -> Result<Toolpath, Cancelled> {
    let (tp, _) = adaptive_3d_toolpath_annotated_traced_with_cancel(
        mesh, index, cutter, params, cancel, None,
    )?;
    Ok(tp)
}

pub fn adaptive_3d_toolpath_traced_with_cancel(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &Adaptive3dParams,
    cancel: &dyn CancelCheck,
    debug: Option<&ToolpathDebugContext>,
) -> Result<Toolpath, Cancelled> {
    let (tp, _) = adaptive_3d_toolpath_annotated_traced_with_cancel(
        mesh, index, cutter, params, cancel, debug,
    )?;
    Ok(tp)
}

/// Like `adaptive_3d_toolpath` but also returns annotations for simulation display.
/// Each annotation is `(move_index, label)`.
#[tracing::instrument(skip(mesh, index, cutter, params), fields(tool_radius = params.tool_radius, stepover = params.stepover))]
// infallible: cancel closure always returns false, so Cancelled is unreachable
#[allow(clippy::expect_used)]
pub fn adaptive_3d_toolpath_annotated(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &Adaptive3dParams,
) -> (Toolpath, Vec<(usize, String)>) {
    let never_cancel = || false;
    adaptive_3d_toolpath_annotated_with_cancel(mesh, index, cutter, params, &never_cancel)
        .expect("non-cancellable adaptive3d should never be cancelled")
}

pub fn adaptive_3d_toolpath_annotated_with_cancel(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &Adaptive3dParams,
    cancel: &dyn CancelCheck,
) -> Result<(Toolpath, Vec<(usize, String)>), Cancelled> {
    adaptive_3d_toolpath_annotated_traced_with_cancel(mesh, index, cutter, params, cancel, None)
}

pub fn adaptive_3d_toolpath_structured_annotated_traced_with_cancel(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &Adaptive3dParams,
    cancel: &dyn CancelCheck,
    debug: Option<&ToolpathDebugContext>,
) -> Result<(Toolpath, Vec<Adaptive3dRuntimeAnnotation>), Cancelled> {
    let result = adaptive_3d_segments(mesh, index, cutter, params, debug, cancel)?;
    let segments = result.segments;
    let (tp, annotations) = segments_to_toolpath(&segments, params);
    if let Some(debug_ctx) = debug {
        for annotation in &annotations {
            debug_ctx.add_annotation(annotation.move_index, annotation.event.label());
        }
    }

    info!(
        moves = tp.moves.len(),
        annotations = annotations.len(),
        cutting_mm = tp.total_cutting_distance(),
        rapid_mm = tp.total_rapid_distance(),
        "3D adaptive toolpath complete"
    );

    Ok((tp, annotations))
}

pub fn adaptive_3d_toolpath_annotated_traced_with_cancel(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &Adaptive3dParams,
    cancel: &dyn CancelCheck,
    debug: Option<&ToolpathDebugContext>,
) -> Result<(Toolpath, Vec<(usize, String)>), Cancelled> {
    let (tp, annotations) = adaptive_3d_toolpath_structured_annotated_traced_with_cancel(
        mesh, index, cutter, params, cancel, debug,
    )?;
    Ok((tp, runtime_annotations_to_labels(&annotations)))
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::clearing::{MaterialRegion, detect_material_regions};
    use super::path::Adaptive3dSegment;
    use super::search::material_remaining_in_region;
    use super::*;
    use crate::dexel::{DexelSegment, ray_subtract_above};
    use crate::dexel_stock::StockCutDirection;
    use crate::geo::P3;
    use crate::mesh::SpatialIndex;
    use crate::radial_profile::RadialProfileLUT;
    use crate::slope::SurfaceHeightmap;
    use crate::tool::FlatEndmill;
    use crate::toolpath::simplify_path_3d;

    /// Helper: create a TriDexelStock from explicit dimensions (matching old Heightmap::from_stock).
    /// Uses `z_min = -10.0` as default bottom Z unless specified.
    fn make_stock(
        x_min: f64,
        y_min: f64,
        x_max: f64,
        y_max: f64,
        z_top: f64,
        cell_size: f64,
    ) -> TriDexelStock {
        TriDexelStock::from_stock(x_min, y_min, x_max, y_max, -10.0, z_top, cell_size)
    }

    /// Helper: create a TriDexelStock with custom per-cell Z-top values.
    /// `cell_top_z` is row-major; each cell gets a single segment [z_min, cell_z].
    fn make_stock_with_cells(
        rows: usize,
        cols: usize,
        origin_x: f64,
        origin_y: f64,
        cell_size: f64,
        z_min: f64,
        cell_top_z: &[f64],
    ) -> TriDexelStock {
        use smallvec::SmallVec;
        let z_max = cell_top_z.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let bbox = crate::geo::BoundingBox3 {
            min: P3::new(origin_x, origin_y, z_min),
            max: P3::new(
                origin_x + (cols - 1) as f64 * cell_size,
                origin_y + (rows - 1) as f64 * cell_size,
                z_max,
            ),
        };
        let mut rays = Vec::with_capacity(rows * cols);
        for &z in cell_top_z {
            if z <= z_min + 1e-9 {
                // No material
                rays.push(SmallVec::new());
            } else {
                let seg = DexelSegment::new(z_min as f32, z as f32);
                rays.push(SmallVec::from_buf([seg]));
            }
        }
        let grid = crate::dexel::DexelGrid {
            rays,
            rows,
            cols,
            origin_u: origin_x,
            origin_v: origin_y,
            cell_size,
            axis: crate::dexel::DexelAxis::Z,
        };
        TriDexelStock {
            z_grid: grid,
            x_grid: None,
            y_grid: None,
            stock_bbox: bbox,
        }
    }

    fn make_flat_mesh() -> (TriangleMesh, SpatialIndex) {
        let mesh = crate::mesh::make_test_flat(50.0);
        let si = SpatialIndex::build(&mesh, 10.0);
        (mesh, si)
    }

    fn make_hemisphere_mesh() -> (TriangleMesh, SpatialIndex) {
        let mesh = crate::mesh::make_test_hemisphere(20.0, 16);
        let si = SpatialIndex::build(&mesh, 10.0);
        (mesh, si)
    }

    fn flat_cutter() -> FlatEndmill {
        FlatEndmill::new(6.35, 25.0)
    }

    fn default_params() -> Adaptive3dParams {
        Adaptive3dParams {
            tool_radius: 3.175,
            envelope_radius: 3.175,
            stepover: 2.0,
            depth_per_pass: 3.0,
            stock_to_leave: 0.5,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            safe_z: 30.0,
            tolerance: 0.1,
            min_cutting_radius: 0.0,
            stock_top_z: 25.0,
            entry_style: EntryStyle3d::Plunge,
            fine_stepdown: None,
            detect_flat_areas: false,
            max_stay_down_dist: None,
            region_ordering: RegionOrdering::Global,
            initial_stock: None,
            // Matches the GUI/MCP default (ContourParallel) so the bulk of
            // adaptive3d unit tests exercise the code path most users reach
            // in production. Tests that specifically validate AgentSearch
            // or Adaptive override this field explicitly.
            clearing_strategy: ClearingStrategy3d::ContourParallel,
            z_blend: false,
            boundary: None,
        }
    }

    // ── Surface heightmap tests ──────────────────────────────────────

    #[test]
    fn test_surface_heightmap_flat() {
        let (mesh, si) = make_flat_mesh();
        let cutter = flat_cutter();
        // Grid within mesh footprint (mesh is 50x50, centered at origin)
        let shm = SurfaceHeightmap::from_mesh(&mesh, &si, &cutter, -20.0, -20.0, 8, 8, 5.0, -10.0);
        // Interior cells should have surface Z near 0 (flat mesh at z=0)
        // Edge cells might get min_z if outside mesh footprint
        let mut interior_count = 0;
        for row in 1..shm.rows - 1 {
            for col in 1..shm.cols - 1 {
                let z = shm.surface_z_at(row, col);
                assert!(
                    (-1.0..=1.0).contains(&z),
                    "Interior flat mesh Z should be near 0, got {:.2} at ({}, {})",
                    z,
                    row,
                    col
                );
                interior_count += 1;
            }
        }
        assert!(interior_count > 10, "Should have checked interior cells");
    }

    #[test]
    fn test_surface_heightmap_hemisphere() {
        let (mesh, si) = make_hemisphere_mesh();
        let cutter = flat_cutter();
        let bbox = &mesh.bbox;
        let shm = SurfaceHeightmap::from_mesh(
            &mesh,
            &si,
            &cutter,
            bbox.min.x - 5.0,
            bbox.min.y - 5.0,
            20,
            20,
            3.0,
            bbox.min.z,
        );

        // Center should be higher than edges
        let center_row = shm.rows / 2;
        let center_col = shm.cols / 2;
        let center_z = shm.surface_z_at(center_row, center_col);
        let edge_z = shm.surface_z_at(0, 0);
        assert!(
            center_z > edge_z,
            "Hemisphere center ({:.1}) should be higher than edge ({:.1})",
            center_z,
            edge_z
        );
    }

    // ── Z level computation ─────────────────────────────────────────────

    #[test]
    fn test_z_level_computation() {
        let stock_top = 20.0;
        let depth_per_pass = 5.0;
        let surface_bottom = 0.0;
        let stock_to_leave = 0.5;
        let z_bottom = surface_bottom + stock_to_leave;

        let mut z_levels: Vec<f64> = Vec::new();
        let mut z = stock_top - depth_per_pass;
        while z > z_bottom {
            z_levels.push(z);
            z -= depth_per_pass;
        }
        z_levels.push(z_bottom);

        assert_eq!(z_levels.len(), 4, "Should have 4 levels: [15, 10, 5, 0.5]");
        assert!((z_levels[0] - 15.0_f64).abs() < 0.01);
        assert!((z_levels[1] - 10.0_f64).abs() < 0.01);
        assert!((z_levels[2] - 5.0_f64).abs() < 0.01);
        assert!((z_levels[3] - 0.5_f64).abs() < 0.01);
    }

    // ── Path simplification ─────────────────────────────────────────────

    #[test]
    fn test_simplify_path_3d() {
        // Collinear 3D points should simplify
        let path = vec![
            P3::new(0.0, 0.0, 0.0),
            P3::new(1.0, 0.0, 1.0),
            P3::new(2.0, 0.0, 2.0),
            P3::new(3.0, 0.0, 3.0),
        ];
        let simplified = simplify_path_3d(&path, 0.01);
        assert_eq!(
            simplified.len(),
            2,
            "Collinear 3D points should reduce to 2"
        );

        // Non-collinear should be preserved
        let path2 = vec![
            P3::new(0.0, 0.0, 0.0),
            P3::new(1.0, 5.0, 1.0),
            P3::new(2.0, 0.0, 2.0),
        ];
        let simplified2 = simplify_path_3d(&path2, 0.01);
        assert_eq!(simplified2.len(), 3, "Non-collinear should be preserved");
    }

    // ── Integration tests ───────────────────────────────────────────────

    #[test]
    fn test_adaptive_3d_flat_produces_toolpath() {
        let (mesh, si) = make_flat_mesh();
        let cutter = flat_cutter();
        let params = Adaptive3dParams {
            stock_top_z: 5.0,    // 5mm above flat mesh at z=0
            depth_per_pass: 5.0, // Single level
            stock_to_leave: 0.0,
            tolerance: 0.5, // Coarse for speed
            ..default_params()
        };

        let tp = adaptive_3d_toolpath(&mesh, &si, &cutter, &params);
        assert!(
            tp.moves.len() > 10,
            "Should produce a non-trivial toolpath, got {} moves",
            tp.moves.len()
        );
        assert!(
            tp.total_cutting_distance() > 10.0,
            "Should have meaningful cutting distance, got {:.1}mm",
            tp.total_cutting_distance()
        );
    }

    #[test]
    fn test_adaptive_3d_hemisphere_multi_level() {
        let (mesh, si) = make_hemisphere_mesh();
        let cutter = flat_cutter();
        let params = Adaptive3dParams {
            stock_top_z: 25.0, // Above hemisphere peak (~20)
            depth_per_pass: 5.0,
            stock_to_leave: 0.5,
            tolerance: 0.5, // Coarse for speed
            ..default_params()
        };

        let tp = adaptive_3d_toolpath(&mesh, &si, &cutter, &params);
        assert!(
            tp.moves.len() > 20,
            "Hemisphere should produce multi-level passes, got {} moves",
            tp.moves.len()
        );

        // Z values should span from near stock_top down to near surface
        let min_z = tp
            .moves
            .iter()
            .filter(|m| matches!(m.move_type, crate::toolpath::MoveType::Linear { .. }))
            .map(|m| m.target.z)
            .fold(f64::INFINITY, f64::min);
        assert!(
            min_z < 15.0,
            "Should cut down to lower Z levels, min feed Z = {:.1}",
            min_z
        );
    }

    /// DIAGNOSTIC: dump per-Cut-segment Z statistics for AgentSearch on a
    /// hemisphere. Used to verify that the Z-drop split fix in
    /// `clear_z_level_agent_2d_slice` is working — and to detect cases
    /// the threshold misses (e.g., cumulative descent across many small
    /// dz steps).
    ///
    /// Prints to stdout via `println!` (test-mode only; not subject to
    /// the production-code `print_stdout` lint). Run with:
    ///   cargo test -p rs_cam_core --lib agent_search_z_drop_diag -- --nocapture
    #[test]
    fn agent_search_z_drop_diag() {
        let (mesh, si) = make_hemisphere_mesh();
        let cutter = flat_cutter();
        let params = Adaptive3dParams {
            stock_top_z: 25.0,
            depth_per_pass: 3.0,
            stock_to_leave: 0.5,
            tolerance: 0.5,
            stepover: 1.0,
            clearing_strategy: ClearingStrategy3d::AgentSearch,
            ..default_params()
        };

        let never_cancel = || false;
        let segments = adaptive_3d_segments(&mesh, &si, &cutter, &params, None, &never_cancel)
            .expect("segments")
            .segments;

        let mut max_per_step_dz = 0.0f64;
        let mut max_path_total_descent = 0.0f64;
        let mut paths_with_descent_gt_dpp = 0usize;
        let mut total_cut_paths = 0usize;
        let mut largest_path_len = 0usize;
        let mut largest_path_descent_summary = String::new();

        for seg in &segments {
            if let Adaptive3dSegment::Cut(path) = seg {
                if path.len() < 2 {
                    continue;
                }
                total_cut_paths += 1;
                let mut path_max_dz = 0.0f64;
                let mut path_total_descent = 0.0f64;
                let mut path_max_contig_descent = 0.0f64;
                let mut current_descent = 0.0f64;
                for w in path.windows(2) {
                    let dz = w[1].z - w[0].z;
                    path_max_dz = path_max_dz.max(dz.abs());
                    if dz < 0.0 {
                        path_total_descent += -dz;
                        current_descent += -dz;
                        path_max_contig_descent = path_max_contig_descent.max(current_descent);
                    } else {
                        current_descent = 0.0;
                    }
                }
                max_per_step_dz = max_per_step_dz.max(path_max_dz);
                if path_total_descent > max_path_total_descent {
                    max_path_total_descent = path_total_descent;
                    largest_path_len = path.len();
                    largest_path_descent_summary = format!(
                        "len={} max_step_dz={:.3} total_descent={:.3} max_contig={:.3}",
                        path.len(),
                        path_max_dz,
                        path_total_descent,
                        path_max_contig_descent
                    );
                }
                if path_total_descent > params.depth_per_pass {
                    paths_with_descent_gt_dpp += 1;
                }
            }
        }

        println!("\n=== AgentSearch Z-drop diagnostics (hemisphere) ===");
        println!("total Cut paths: {}", total_cut_paths);
        println!("max per-step |dz|: {:.3} mm", max_per_step_dz);
        println!(
            "max path total descent: {:.3} mm (over {} pts)",
            max_path_total_descent, largest_path_len
        );
        println!(
            "paths with total descent > depth_per_pass ({:.1}): {}",
            params.depth_per_pass, paths_with_descent_gt_dpp
        );
        println!("worst path: {}", largest_path_descent_summary);

        // The split fix should keep per-step |dz| under depth_per_pass × 1.1.
        let threshold = params.depth_per_pass * 1.1;
        assert!(
            max_per_step_dz <= threshold + 0.01,
            "split fix failed: max per-step |dz| = {:.3} mm exceeds threshold {:.3} mm",
            max_per_step_dz,
            threshold
        );
    }

    #[test]
    fn test_adaptive_3d_z_follows_surface() {
        let (mesh, si) = make_flat_mesh();
        let cutter = flat_cutter();
        let params = Adaptive3dParams {
            stock_top_z: 5.0,
            depth_per_pass: 5.0,
            stock_to_leave: 0.5,
            tolerance: 0.5,
            ..default_params()
        };

        let tp = adaptive_3d_toolpath(&mesh, &si, &cutter, &params);

        // All cutting moves should be at or above stock_to_leave
        for m in &tp.moves {
            if let crate::toolpath::MoveType::Linear { .. } = m.move_type
                && m.target.z < params.safe_z - 1.0
            {
                assert!(
                    m.target.z >= params.stock_to_leave - 1.0,
                    "Cut Z ({:.2}) should be >= stock_to_leave ({:.1}) - tolerance",
                    m.target.z,
                    params.stock_to_leave
                );
            }
        }
    }

    // ── Fix 1: Z-rate clamping test ────────────────────────────────────

    #[test]
    fn test_z_rate_clamp_limits_descent() {
        // Verify that Z-rate clamping works in the internal stepping loop.
        // We test by calling adaptive_3d_segments directly and inspecting Cut paths
        // before simplification/blending.
        let (mesh, si) = make_hemisphere_mesh();
        let cutter = flat_cutter();
        let depth_per_pass = 3.0;
        let params = Adaptive3dParams {
            stock_top_z: 25.0,
            depth_per_pass,
            stock_to_leave: 0.5,
            tolerance: 0.5,
            ..default_params()
        };

        let never_cancel = || false;
        let segments = adaptive_3d_segments(&mesh, &si, &cutter, &params, None, &never_cancel)
            .expect("test helper should not cancel")
            .segments;

        // Check raw Cut segments: consecutive points should not drop > depth_per_pass
        let mut checked = 0;
        for seg in &segments {
            if let Adaptive3dSegment::Cut(path) = seg {
                for window in path.windows(2) {
                    let z_drop = window[0].z - window[1].z;
                    if z_drop > 0.0 {
                        assert!(
                            z_drop <= depth_per_pass + 0.1,
                            "Raw path Z drop {:.2} exceeds depth_per_pass {:.1}",
                            z_drop,
                            depth_per_pass,
                        );
                        checked += 1;
                    }
                }
            }
        }
        assert!(checked > 0, "Should have checked some downward Z moves");
    }

    // ── Fix 2: Helix entry test ────────────────────────────────────────

    #[test]
    fn test_helix_entry_no_vertical_plunge() {
        let (mesh, si) = make_flat_mesh();
        let cutter = flat_cutter();
        let params = Adaptive3dParams {
            stock_top_z: 5.0,
            depth_per_pass: 5.0,
            stock_to_leave: 0.0,
            tolerance: 0.5,
            entry_style: EntryStyle3d::Helix {
                radius: cutter.radius() * 0.8,
                pitch: 1.0,
            },
            ..default_params()
        };

        let tp = adaptive_3d_toolpath(&mesh, &si, &cutter, &params);
        assert!(tp.moves.len() > 5, "Should produce a toolpath");

        // With helix entry, there should be feed moves that descend while
        // moving in XY (helix spiral). Individual helix steps are small,
        // so check for any downward-feed with XY motion.
        let mut has_helix_moves = false;
        for window in tp.moves.windows(2) {
            if let crate::toolpath::MoveType::Linear { .. } = window[1].move_type {
                let dx = (window[1].target.x - window[0].target.x).abs();
                let dy = (window[1].target.y - window[0].target.y).abs();
                let dz = window[0].target.z - window[1].target.z;
                // A helix step descends while moving in XY
                if dz > 0.005 && (dx > 0.01 || dy > 0.01) {
                    has_helix_moves = true;
                    break;
                }
            }
        }
        assert!(
            has_helix_moves,
            "Helix entry should produce moves with simultaneous XY+Z motion"
        );
    }

    // ── Fix 4: Fine stepdown test ──────────────────────────────────────

    #[test]
    fn test_fine_stepdown_inserts_levels() {
        // Verify that fine_stepdown produces more Z levels
        let stock_top: f64 = 20.0;
        let depth_per_pass: f64 = 5.0;
        let fine_step: f64 = 1.0;
        let surface_bottom: f64 = 0.0;
        let stock_to_leave: f64 = 0.5;
        let z_bottom = surface_bottom + stock_to_leave;

        // Major levels only
        let mut major_levels = Vec::new();
        let mut z = stock_top - depth_per_pass;
        while z > z_bottom {
            major_levels.push(z);
            z -= depth_per_pass;
        }
        major_levels.push(z_bottom);
        let n_major = major_levels.len(); // Should be 4: [15, 10, 5, 0.5]

        // Fine stepdown levels
        let mut all_levels = Vec::new();
        let first_start = stock_top;
        for window in std::iter::once(&first_start)
            .chain(major_levels.iter())
            .collect::<Vec<_>>()
            .windows(2)
        {
            let z_top = *window[0];
            let z_bot = *window[1];
            let mut iz = z_top - fine_step;
            while iz > z_bot + fine_step * 0.5 {
                all_levels.push(iz);
                iz -= fine_step;
            }
            all_levels.push(z_bot);
        }
        all_levels.sort_by(|a, b| b.total_cmp(a));
        all_levels.dedup_by(|a, b| (*a - *b).abs() < 0.01);

        assert!(
            all_levels.len() > n_major * 3,
            "Fine stepdown should produce significantly more levels: {} vs {}",
            all_levels.len(),
            n_major
        );
        // With fine_step=1 and depth_per_pass=5, each major interval gets ~4 intermediates
        // Total should be around 19-20 levels
        assert!(
            all_levels.len() >= 15,
            "Expected at least 15 fine levels, got {}",
            all_levels.len()
        );
    }

    // ── Fix 5: Flat area detection test ────────────────────────────────

    #[test]
    fn test_flat_area_detection_finds_shelf() {
        // Build a surface heightmap where many cells sit at z=10 (a shelf)
        // and the rest sit at z=0 (floor)
        let cell_size = 1.0;
        let rows = 20;
        let cols = 20;
        let mut z_values = vec![0.0; rows * cols];
        // Create a shelf: rows 5..15, cols 5..15 at z=10
        for row in 5..15 {
            for col in 5..15 {
                z_values[row * cols + col] = 10.0;
            }
        }

        let shm = SurfaceHeightmap {
            z_values,
            rows,
            cols,
            origin_x: 0.0,
            origin_y: 0.0,
            cell_size,
        };

        // Histogram detection logic (same as in adaptive_3d_segments)
        let tolerance: f64 = 0.1;
        let stock_to_leave: f64 = 0.5;
        let stock_top: f64 = 25.0;
        let total_cells = shm.z_values.len();
        let bin_size = tolerance.max(0.05);
        let z_min_surf = 0.0;
        let z_max_surf = stock_top;
        let n_bins = ((z_max_surf - z_min_surf) / bin_size).ceil() as usize + 1;
        let mut histogram = vec![0u32; n_bins];
        for &sz in &shm.z_values {
            let bin = ((sz - z_min_surf) / bin_size).floor() as usize;
            if bin < n_bins {
                histogram[bin] += 1;
            }
        }
        let threshold = (total_cells as f64 * 0.02) as u32;
        let mut flat_levels = Vec::new();
        let z_bottom = 0.0 + stock_to_leave;
        for (i, &count) in histogram.iter().enumerate() {
            if count > threshold {
                let flat_z = z_min_surf + (i as f64 + 0.5) * bin_size + stock_to_leave;
                if flat_z > z_bottom + bin_size && flat_z < stock_top - bin_size {
                    flat_levels.push(flat_z);
                }
            }
        }

        // Should detect the shelf at z≈10 (+stock_to_leave=0.5 → 10.5)
        let found_shelf = flat_levels.iter().any(|&z| (z - 10.5).abs() < 1.0);
        assert!(
            found_shelf,
            "Should detect shelf near z=10.5, found levels: {:?}",
            flat_levels
        );
    }

    // ── Region detection tests ───────────────────────────────────────────

    #[test]
    fn test_detect_regions_single_block() {
        // Full material → 1 region covering entire grid
        let (mesh, si) = make_flat_mesh();
        let cutter = flat_cutter();
        let cell_size = 2.0;

        let material_stock = make_stock(-30.0, -30.0, 30.0, 30.0, 20.0, cell_size);
        let surface_hm = SurfaceHeightmap::from_mesh(
            &mesh,
            &si,
            &cutter,
            material_stock.z_grid.origin_u,
            material_stock.z_grid.origin_v,
            material_stock.z_grid.rows,
            material_stock.z_grid.cols,
            cell_size,
            -10.0,
        );

        let regions = detect_material_regions(&material_stock, &surface_hm, 0.5, 3.175);
        assert!(
            !regions.is_empty(),
            "Full material should produce at least 1 region"
        );
        // Largest region should cover most of the grid
        let total_cells = material_stock.z_grid.rows * material_stock.z_grid.cols;
        assert!(
            regions[0].cell_count > total_cells / 2,
            "Largest region should cover most cells: {} / {}",
            regions[0].cell_count,
            total_cells
        );
    }

    #[test]
    fn test_detect_regions_two_islands() {
        // Two separated blocks → 2 regions, sorted by area
        let cell_size = 1.0;
        let material_stock = make_stock(0.0, 0.0, 30.0, 10.0, 20.0, cell_size);
        let rows = material_stock.z_grid.rows;
        let cols = material_stock.z_grid.cols;

        // Surface at z=0 everywhere
        let surface_hm = SurfaceHeightmap {
            z_values: vec![0.0; rows * cols],
            rows,
            cols,
            origin_x: material_stock.z_grid.origin_u,
            origin_y: material_stock.z_grid.origin_v,
            cell_size,
        };

        // Create two islands by clearing a gap in the middle
        let mut hm = material_stock;
        for row in 0..rows {
            for col in 0..cols {
                let (x, _y) = hm.z_grid.cell_to_world(row, col);
                if (13.0..=17.0).contains(&x) {
                    // Clear the gap — remove all material
                    ray_subtract_above(hm.z_grid.ray_mut(row, col), hm.stock_bbox.min.z as f32);
                }
            }
        }

        let regions = detect_material_regions(&hm, &surface_hm, 0.5, 3.175);
        assert!(
            regions.len() >= 2,
            "Should detect at least 2 separate regions, got {}",
            regions.len()
        );
        // Sorted by area descending
        assert!(
            regions[0].cell_count >= regions[1].cell_count,
            "Regions should be sorted by area descending"
        );
    }

    #[test]
    fn test_detect_regions_diagonal_connected() {
        // Diagonal-touching blocks → 1 region (8-connected)
        let cell_size = 1.0;
        let rows = 10;
        let cols = 10;

        // Surface at z=0, material at z=20 only on diagonal cells
        let mut mat_cells = vec![0.0f64; rows * cols];
        for i in 0..rows.min(cols) {
            mat_cells[i * cols + i] = 20.0;
        }

        let hm = make_stock_with_cells(rows, cols, 0.0, 0.0, cell_size, -10.0, &mat_cells);
        let surface_hm = SurfaceHeightmap {
            z_values: vec![0.0; rows * cols],
            rows,
            cols,
            origin_x: 0.0,
            origin_y: 0.0,
            cell_size,
        };

        let regions = detect_material_regions(&hm, &surface_hm, 0.5, 3.175);
        assert_eq!(
            regions.len(),
            1,
            "Diagonal cells should form 1 region with 8-connectivity, got {}",
            regions.len()
        );
    }

    #[test]
    fn test_detect_regions_small_filtered() {
        // Isolated cells (< 4) should be filtered out
        let cell_size = 1.0;
        let rows = 10;
        let cols = 10;

        // Only 2 adjacent cells have material
        let mut mat_cells = vec![0.0f64; rows * cols];
        mat_cells[0] = 20.0;
        mat_cells[1] = 20.0;

        let hm = make_stock_with_cells(rows, cols, 0.0, 0.0, cell_size, -10.0, &mat_cells);
        let surface_hm = SurfaceHeightmap {
            z_values: vec![0.0; rows * cols],
            rows,
            cols,
            origin_x: 0.0,
            origin_y: 0.0,
            cell_size,
        };

        let regions = detect_material_regions(&hm, &surface_hm, 0.5, 3.175);
        assert!(
            regions.is_empty(),
            "Tiny regions (< 4 cells) should be filtered out, got {} regions",
            regions.len()
        );
    }

    #[test]
    fn test_material_remaining_in_region() {
        let (mesh, si) = make_flat_mesh();
        let cutter = flat_cutter();
        let cell_size = 1.0;

        let material_stock = make_stock(-30.0, -30.0, 30.0, 30.0, 20.0, cell_size);
        let surface_hm = SurfaceHeightmap::from_mesh(
            &mesh,
            &si,
            &cutter,
            material_stock.z_grid.origin_u,
            material_stock.z_grid.origin_v,
            material_stock.z_grid.rows,
            material_stock.z_grid.cols,
            cell_size,
            -10.0,
        );

        // A region covering a quarter of the grid
        let region = MaterialRegion {
            row_min: 0,
            row_max: material_stock.z_grid.rows / 2,
            col_min: 0,
            col_max: material_stock.z_grid.cols / 2,
            world_x_min: -30.0,
            world_x_max: 0.0,
            world_y_min: -30.0,
            world_y_max: 0.0,
            cell_count: (material_stock.z_grid.rows / 2) * (material_stock.z_grid.cols / 2),
            surface_z_min: 0.0,
            surface_z_max: 0.0,
        };

        let rem = material_remaining_in_region(&material_stock, &surface_hm, 10.0, 0.5, &region);
        assert!(
            rem > 0.5,
            "Full material in region should show high remaining, got {:.2}",
            rem
        );
    }

    // ── Integration: ByArea ordering ─────────────────────────────────────

    #[test]
    fn test_adaptive_3d_by_area_flat() {
        let (mesh, si) = make_flat_mesh();
        let cutter = flat_cutter();
        let params = Adaptive3dParams {
            stock_top_z: 5.0,
            depth_per_pass: 5.0,
            stock_to_leave: 0.0,
            tolerance: 0.5,
            region_ordering: RegionOrdering::ByArea,
            ..default_params()
        };

        let tp = adaptive_3d_toolpath(&mesh, &si, &cutter, &params);
        assert!(
            tp.moves.len() > 10,
            "ByArea on flat mesh should produce toolpath, got {} moves",
            tp.moves.len()
        );
        assert!(
            tp.total_cutting_distance() > 10.0,
            "ByArea should have meaningful cutting distance, got {:.1}mm",
            tp.total_cutting_distance()
        );
    }

    #[test]
    fn test_adaptive_3d_by_area_hemisphere() {
        let (mesh, si) = make_hemisphere_mesh();
        let cutter = flat_cutter();
        let params = Adaptive3dParams {
            stock_top_z: 25.0,
            depth_per_pass: 5.0,
            stock_to_leave: 0.5,
            tolerance: 0.5,
            region_ordering: RegionOrdering::ByArea,
            ..default_params()
        };

        let tp = adaptive_3d_toolpath(&mesh, &si, &cutter, &params);
        assert!(
            tp.moves.len() > 20,
            "ByArea on hemisphere should produce multi-level passes, got {} moves",
            tp.moves.len()
        );

        // Z values should span a useful range
        let min_z = tp
            .moves
            .iter()
            .filter(|m| matches!(m.move_type, crate::toolpath::MoveType::Linear { .. }))
            .map(|m| m.target.z)
            .fold(f64::INFINITY, f64::min);
        assert!(
            min_z < 15.0,
            "ByArea should cut down to lower Z levels, min feed Z = {:.1}",
            min_z
        );
    }

    // ── Widening coverage test ──────────────────────────────────────────

    #[test]
    fn test_widening_covers_stepover() {
        // Verify that path widening stamps cells at stepover distance.
        let (mesh, si) = make_flat_mesh();
        let cutter = flat_cutter();
        let cell_size = 0.5;
        let stepover = 2.0;

        let mut material_stock = make_stock(-30.0, -30.0, 30.0, 30.0, 20.0, cell_size);
        let surface_hm = SurfaceHeightmap::from_mesh(
            &mesh,
            &si,
            &cutter,
            material_stock.z_grid.origin_u,
            material_stock.z_grid.origin_v,
            material_stock.z_grid.rows,
            material_stock.z_grid.cols,
            cell_size,
            -10.0,
        );

        // Simulate a straight horizontal path at y=0, from x=-10 to x=10
        let z_level = 10.0;
        let path: Vec<P3> = (0..=40)
            .map(|i| P3::new(-10.0 + i as f64 * 0.5, 0.0, z_level))
            .collect();

        // Stamp along the path itself
        let lut = RadialProfileLUT::from_cutter(&cutter, 256);
        for p in &path {
            material_stock.stamp_tool_at(
                &lut,
                cutter.radius(),
                p.x,
                p.y,
                p.z,
                StockCutDirection::FromTop,
            );
        }

        // Now apply widening with double ring at stepover distance
        for i in 1..path.len() {
            let prev = &path[i - 1];
            let curr = &path[i];
            let dx = curr.x - prev.x;
            let dy = curr.y - prev.y;
            let seg_len = (dx * dx + dy * dy).sqrt();
            if seg_len < 1e-10 {
                continue;
            }
            let nx = -dy / seg_len;
            let ny = dx / seg_len;
            for &mult in &[1.0f64, 2.0] {
                for &sign in &[1.0f64, -1.0] {
                    let px = curr.x + sign * mult * stepover * nx;
                    let py = curr.y + sign * mult * stepover * ny;
                    let sz = surface_hm.surface_z_at_world(px, py);
                    if sz != f64::NEG_INFINITY {
                        let pz = (sz + 0.5).max(z_level);
                        material_stock.stamp_tool_at(
                            &lut,
                            cutter.radius(),
                            px,
                            py,
                            pz,
                            StockCutDirection::FromTop,
                        );
                    }
                }
            }
        }

        // Check that cells at y = +/- stepover are cleared (material lowered from 20)
        for &y_off in &[stepover, -stepover, 2.0 * stepover, -2.0 * stepover] {
            if let Some((row, col)) = material_stock.z_grid.world_to_cell(0.0, y_off) {
                let z = stock_top_z_at(&material_stock, row, col);
                assert!(
                    z < 20.0 - 0.1,
                    "Cell at y={:.1} should be widened (z lowered from 20), got z={:.2}",
                    y_off,
                    z
                );
            }
        }
    }

    // ── Low-yield bail test ─────────────────────────────────────────────

    #[test]
    fn test_low_yield_bail() {
        // Thin-film material (just above floor) — adaptive should bail quickly.
        let (mesh, si) = make_flat_mesh();
        let cutter = flat_cutter();

        // Stock barely above surface: 0.2mm of material (below thin_threshold)
        // Pre-stamp should eliminate this, so adaptive should do minimal work.
        let params = Adaptive3dParams {
            stock_top_z: 0.2, // Only 0.2mm above flat mesh at z=0
            depth_per_pass: 3.0,
            stock_to_leave: 0.0,
            tolerance: 0.5,
            ..default_params()
        };

        let never_cancel = || false;
        let segments = adaptive_3d_segments(&mesh, &si, &cutter, &params, None, &never_cancel)
            .expect("test helper should not cancel")
            .segments;

        // Count actual cutting passes
        let cut_count = segments
            .iter()
            .filter(|s| matches!(s, Adaptive3dSegment::Cut(_)))
            .count();

        // With thin-film material, should bail quickly (few or no passes)
        assert!(
            cut_count < 20,
            "Thin film should produce few cutting passes, got {}",
            cut_count
        );
    }

    #[test]
    fn traced_adaptive3d_emits_spans_hotspots_and_annotations() {
        let (mesh, si) = make_hemisphere_mesh();
        let cutter = flat_cutter();
        // This test validates AgentSearch's observability — z_level /
        // adaptive_pass spans and adaptive3d_pass hotspots are only
        // emitted by clear_z_level() (the AgentSearch path), not by
        // clear_z_level_contour_parallel or clear_z_level_adaptive.
        let params = Adaptive3dParams {
            clearing_strategy: ClearingStrategy3d::AgentSearch,
            ..default_params()
        };
        let recorder = crate::debug_trace::ToolpathDebugRecorder::new("Adaptive 3D", "3D Rough");
        let ctx = recorder.root_context();
        let never_cancel = || false;

        let tp = adaptive_3d_toolpath_traced_with_cancel(
            &mesh,
            &si,
            &cutter,
            &params,
            &never_cancel,
            Some(&ctx),
        )
        .expect("debug run should complete");
        let trace = recorder.finish();

        assert!(!tp.moves.is_empty(), "expected a non-empty toolpath");
        assert!(
            trace
                .spans
                .iter()
                .any(|span| span.kind == "surface_heightmap"),
            "trace should include surface heightmap timing"
        );
        assert!(
            trace.spans.iter().any(|span| span.kind == "z_level"),
            "trace should include Z-level spans"
        );
        assert!(
            trace.spans.iter().any(|span| span.kind == "adaptive_pass"),
            "trace should include adaptive pass spans"
        );
        // Hotspot kind is "adaptive_pass" (emitted by the 2D adaptive
        // which now drives AgentSearch slices) — the old 3D agent code
        // emitted "adaptive3d_pass"; both are valid.
        assert!(
            trace
                .hotspots
                .iter()
                .any(|hotspot| hotspot.kind == "adaptive_pass"
                    || hotspot.kind == "adaptive3d_pass"),
            "trace should record at least one adaptive hotspot"
        );
        assert!(
            !trace.annotations.is_empty(),
            "adaptive 3D trace should carry generated annotations"
        );
    }

    // ── Contour-parallel EDT tests ───────────────────────────────────

    #[test]
    fn test_contour_parallel_edt_flat_mesh() {
        let (mesh, si) = make_flat_mesh();
        let cutter = flat_cutter();
        let params = Adaptive3dParams {
            stock_top_z: 5.0,
            depth_per_pass: 5.0,
            stock_to_leave: 0.0,
            tolerance: 0.5,
            clearing_strategy: ClearingStrategy3d::ContourParallel,
            ..default_params()
        };

        let tp = adaptive_3d_toolpath(&mesh, &si, &cutter, &params);
        assert!(
            tp.moves.len() > 5,
            "Contour-parallel EDT on flat mesh should produce moves, got {}",
            tp.moves.len()
        );

        // Check that there are actual cutting moves (Linear with non-rapid feed)
        let cut_moves = tp
            .moves
            .iter()
            .filter(|m| matches!(m.move_type, crate::toolpath::MoveType::Linear { .. }))
            .count();
        assert!(
            cut_moves > 0,
            "Contour-parallel EDT should produce cutting moves, got 0"
        );
    }

    #[test]
    fn test_contour_parallel_edt_hemisphere() {
        let (mesh, si) = make_hemisphere_mesh();
        let cutter = flat_cutter();
        let params = Adaptive3dParams {
            stock_top_z: 25.0,
            depth_per_pass: 5.0,
            stock_to_leave: 0.5,
            tolerance: 0.5,
            clearing_strategy: ClearingStrategy3d::ContourParallel,
            ..default_params()
        };

        let tp = adaptive_3d_toolpath(&mesh, &si, &cutter, &params);
        assert!(
            tp.moves.len() > 10,
            "Contour-parallel EDT on hemisphere should produce multi-level passes, got {} moves",
            tp.moves.len()
        );

        // Z values should span multiple levels
        let min_z = tp
            .moves
            .iter()
            .filter(|m| matches!(m.move_type, crate::toolpath::MoveType::Linear { .. }))
            .map(|m| m.target.z)
            .fold(f64::INFINITY, f64::min);
        let max_z = tp
            .moves
            .iter()
            .filter(|m| matches!(m.move_type, crate::toolpath::MoveType::Linear { .. }))
            .map(|m| m.target.z)
            .fold(f64::NEG_INFINITY, f64::max);
        assert!(
            max_z - min_z > 3.0,
            "Hemisphere contour-parallel should span multiple Z levels, range = {:.1}",
            max_z - min_z
        );
    }

    /// Validate that contour-parallel clearing produces complete material removal
    /// on a flat mesh (single Z level, no terrain variation).
    #[test]
    fn test_contour_parallel_complete_clearing() {
        use crate::dexel_stock::StockCutDirection;
        use crate::radial_profile::RadialProfileLUT;

        let (mesh, si) = make_flat_mesh(); // 50x50mm flat at z=0
        let cutter = flat_cutter(); // 6.35mm diameter
        let tool_radius = cutter.radius();
        let stock_top_z = 5.0;
        let stock_to_leave = 0.0;

        let params = Adaptive3dParams {
            tool_radius,
            stepover: 2.5,
            depth_per_pass: 5.0,
            stock_to_leave,
            tolerance: 0.3,
            stock_top_z,
            clearing_strategy: ClearingStrategy3d::ContourParallel,
            ..default_params()
        };

        let tp = adaptive_3d_toolpath(&mesh, &si, &cutter, &params);
        assert!(tp.moves.len() > 10, "Should produce moves");

        // Simulate the toolpath on a fresh stock
        let cell_size = 0.3;
        let mut sim_stock =
            TriDexelStock::from_stock(-25.5, -25.5, 25.5, 25.5, -1.0, stock_top_z, cell_size);
        let lut = RadialProfileLUT::from_cutter(&cutter, 256);
        sim_stock
            .simulate_toolpath_with_lut_cancel(
                &tp,
                &lut,
                tool_radius,
                StockCutDirection::FromTop,
                &|| false,
            )
            .unwrap();

        assert_clearing_complete(&sim_stock, tool_radius, cell_size, stock_to_leave, "flat");
    }

    /// Validate clearing completeness on a hemisphere (multi-Z-level, terrain).
    ///
    /// Only checks cells WITHIN the mesh bounding box (the adaptive3d internal stock
    /// only covers mesh_bbox ± tool_radius). Cells outside that range are expected uncleared.
    #[test]
    fn test_contour_parallel_complete_clearing_hemisphere() {
        use crate::dexel_stock::StockCutDirection;
        use crate::radial_profile::RadialProfileLUT;

        let (mesh, si) = make_hemisphere_mesh(); // radius=20, centered at origin
        let cutter = flat_cutter(); // 6.35mm diameter
        let tool_radius = cutter.radius();
        let stock_top_z = 25.0;
        let stock_to_leave = 0.5;

        let params = Adaptive3dParams {
            tool_radius,
            stepover: 2.5,
            depth_per_pass: 3.0,
            stock_to_leave,
            tolerance: 0.3,
            stock_top_z,
            clearing_strategy: ClearingStrategy3d::ContourParallel,
            ..default_params()
        };

        let tp = adaptive_3d_toolpath(&mesh, &si, &cutter, &params);
        assert!(tp.moves.len() > 10, "Should produce moves");

        // Use the mesh bbox for the simulation stock (matching internal adaptive3d stock).
        let bbox = &mesh.bbox;
        let r = cutter.radius();
        let sim_x_min = bbox.min.x - r - 0.5;
        let sim_x_max = bbox.max.x + r + 0.5;
        let sim_y_min = bbox.min.y - r - 0.5;
        let sim_y_max = bbox.max.y + r + 0.5;

        let cell_size = 0.3;
        let mut sim_stock = TriDexelStock::from_stock(
            sim_x_min,
            sim_y_min,
            sim_x_max,
            sim_y_max,
            -1.0,
            stock_top_z,
            cell_size,
        );
        let lut = RadialProfileLUT::from_cutter(&cutter, 256);
        sim_stock
            .simulate_toolpath_with_lut_cancel(
                &tp,
                &lut,
                tool_radius,
                StockCutDirection::FromTop,
                &|| false,
            )
            .unwrap();

        let grid = &sim_stock.z_grid;
        let margin_cells = (tool_radius / cell_size).ceil() as usize + 2;
        let mut uncleared_count = 0usize;
        let mut total_checked = 0usize;
        let max_excess = 3.5; // allow up to depth_per_pass + tolerance
        let mut worst_excess = 0.0f64;

        for row in margin_cells..grid.rows.saturating_sub(margin_cells) {
            for col in margin_cells..grid.cols.saturating_sub(margin_cells) {
                let x = grid.origin_u + col as f64 * cell_size;
                let y = grid.origin_v + row as f64 * cell_size;
                total_checked += 1;
                if let Some(tz) = grid.top_z_at(row, col) {
                    let r_sq = 20.0 * 20.0 - x * x - y * y;
                    let surface_z = if r_sq > 0.0 { r_sq.sqrt() } else { 0.0 };
                    let expected_max = (surface_z + stock_to_leave + max_excess) as f32;
                    if tz > expected_max {
                        uncleared_count += 1;
                        let excess = (tz as f64) - (surface_z + stock_to_leave);
                        if excess > worst_excess {
                            worst_excess = excess;
                        }
                    }
                }
            }
        }

        let uncleared_pct = if total_checked > 0 {
            uncleared_count as f64 / total_checked as f64 * 100.0
        } else {
            0.0
        };

        // Allow up to 5% uncleared — remaining cells are near steep/vertical
        // walls at the hemisphere edge where the coarse mesh (16 subdivisions)
        // and tool geometry limit clearing accuracy.
        assert!(
            uncleared_pct < 5.0,
            "Hemisphere contour-parallel should clear >95% of cells, but {:.1}% ({}/{}) have excess material",
            uncleared_pct,
            uncleared_count,
            total_checked,
        );
    }

    /// Helper: assert that a simulated stock has been fully cleared (for flat surfaces).
    fn assert_clearing_complete(
        stock: &TriDexelStock,
        tool_radius: f64,
        cell_size: f64,
        stock_to_leave: f64,
        label: &str,
    ) {
        let grid = &stock.z_grid;
        let margin_cells = (tool_radius / cell_size).ceil() as usize + 2;
        let z_threshold = (stock_to_leave + 0.5) as f32;
        let mut uncleared_count = 0usize;
        let mut total_checked = 0usize;

        for row in margin_cells..grid.rows.saturating_sub(margin_cells) {
            for col in margin_cells..grid.cols.saturating_sub(margin_cells) {
                total_checked += 1;
                if let Some(tz) = grid.top_z_at(row, col)
                    && tz > z_threshold
                {
                    uncleared_count += 1;
                }
            }
        }

        let uncleared_pct = if total_checked > 0 {
            uncleared_count as f64 / total_checked as f64 * 100.0
        } else {
            0.0
        };

        assert!(
            uncleared_pct < 1.0,
            "[{label}] Contour-parallel should clear >99% of interior, but {:.1}% ({}/{}) remain above z={:.1}",
            uncleared_pct,
            uncleared_count,
            total_checked,
            z_threshold,
        );
    }

    // ── Planner ↔ simulator dexel parity ───────────────────────────────
    //
    // Probe for the wanaka Back Rough Z=10/Z=7 anomaly: the planner
    // believes it cleared each Z level (`material_remaining_post → 0`)
    // and emits 4585mm of Cut path per level, but a fresh simulator
    // replay of the same toolpath against the same initial stock
    // reads zero engagement on those passes — meaning the planner's
    // INTERNAL stock state diverges from the state the emitted moves
    // would actually produce.
    //
    // These tests run the planner and an independent simulator
    // against the same initial dexel and assert cell-by-cell parity
    // of the resulting stock tops. They're ignored by default
    // (slow + currently-failing) — use:
    //   cargo test -p rs_cam_core --lib planner_sim_dexel -- --ignored --nocapture
    // for full divergence reports; attach gdb/lldb via
    //   cargo test -p rs_cam_core --lib planner_sim_dexel --no-run
    // to localize the bug.

    /// (divergent, total, interior, max_dz, violations[(row, col, planner_top, sim_top, surface, dz)])
    type ParityResult = (
        u64,
        u64,
        u64,
        f64,
        Vec<(usize, usize, f64, f64, f64, f64)>,
    );

    fn run_planner_sim_parity(
        strategy: ClearingStrategy3d,
        label: &str,
    ) -> ParityResult {
        run_planner_sim_parity_with_mesh(strategy, label, make_hemisphere_mesh())
    }

    fn run_planner_sim_parity_flat(
        strategy: ClearingStrategy3d,
        label: &str,
    ) -> ParityResult {
        run_planner_sim_parity_with_mesh(strategy, label, make_flat_mesh())
    }

    fn run_planner_sim_parity_with_mesh(
        strategy: ClearingStrategy3d,
        label: &str,
        mesh_pair: (TriangleMesh, SpatialIndex),
    ) -> ParityResult {
        let (mesh, si) = mesh_pair;
        let mesh_bbox_for_interior = mesh.bbox;
        let cutter = flat_cutter();
        let r = cutter.radius();
        let bbox = mesh.bbox;
        let origin_x = bbox.min.x - r;
        let origin_y = bbox.min.y - r;
        let extent_x = bbox.max.x + r;
        let extent_y = bbox.max.y + r;
        let tool_radius = 3.175_f64;
        let stock_top_z = 25.0_f64;
        let cell_size = (tool_radius / 6.0).max(0.1);

        let initial_stock = TriDexelStock::from_stock(
            origin_x,
            origin_y,
            extent_x,
            extent_y,
            bbox.min.z,
            stock_top_z,
            cell_size,
        );

        let params = Adaptive3dParams {
            initial_stock: Some(initial_stock.clone()),
            clearing_strategy: strategy,
            tool_radius,
            envelope_radius: tool_radius,
            stock_top_z,
            depth_per_pass: 3.0,
            stock_to_leave: 0.5,
            stepover: 1.0,
            tolerance: 0.5,
            ..default_params()
        };

        let never_cancel = || false;
        let result = adaptive_3d_segments(&mesh, &si, &cutter, &params, None, &never_cancel)
            .expect("planner should succeed");
        let planner_stock = result.final_material_stock;
        let surface_hm = result.surface_heightmap;
        let segments = result.segments;
        let (toolpath, _) = segments_to_toolpath(&segments, &params);

        let mut sim_stock = initial_stock;
        sim_stock
            .simulate_toolpath_with_metrics_with_cancel(
                &toolpath,
                &cutter,
                StockCutDirection::FromTop,
                0,
                12_000,
                2,
                3000.0,
                0.5,
                None,
                true,
                &never_cancel,
            )
            .expect("simulator should succeed");

        // Cell-by-cell ray-top comparison. Tolerance = cell_size
        // (sub-cell stamping noise is OK; bigger gaps are real bugs).
        let tol_mm = cell_size;
        let grid = &planner_stock.z_grid;
        let total_cells = (grid.rows * grid.cols) as u64;
        let mut divergent = 0u64;
        let mut interior_divergent = 0u64;
        let mut planner_higher = 0u64; // sim removed more
        let mut sim_higher = 0u64; // planner removed more
        let mut max_dz = 0.0_f64;
        let mut violations: Vec<(usize, usize, f64, f64, f64, f64)> = Vec::new();
        let interior_x_lo = mesh_bbox_for_interior.min.x + 1.0;
        let interior_x_hi = mesh_bbox_for_interior.max.x - 1.0;
        let interior_y_lo = mesh_bbox_for_interior.min.y + 1.0;
        let interior_y_hi = mesh_bbox_for_interior.max.y - 1.0;
        for row in 0..grid.rows {
            for col in 0..grid.cols {
                let p = stock_top_z_at(&planner_stock, row, col);
                let s = stock_top_z_at(&sim_stock, row, col);
                let dz = (p - s).abs();
                if dz > tol_mm {
                    divergent += 1;
                    max_dz = max_dz.max(dz);
                    if p > s + tol_mm {
                        planner_higher += 1;
                    } else if s > p + tol_mm {
                        sim_higher += 1;
                    }
                    let (x, y) = grid.cell_to_world(row, col);
                    let is_interior = x > interior_x_lo
                        && x < interior_x_hi
                        && y > interior_y_lo
                        && y < interior_y_hi;
                    if is_interior {
                        interior_divergent += 1;
                        // Only collect INTERIOR violations — boundary
                        // pre-clear noise drowns out the real bug
                        // otherwise.
                        if violations.len() < 20 {
                            let i = row * grid.cols + col;
                            let surf = surface_hm.z_values[i];
                            violations.push((row, col, p, s, surf, dz));
                        }
                    }
                }
            }
        }

        eprintln!(
            "[{label}] PARITY: {divergent}/{total_cells} cells differ > {tol_mm:.2}mm; \
             interior {interior_divergent}; planner_higher {planner_higher} (sim removed more); \
             sim_higher {sim_higher} (planner removed more); max dz {max_dz:.3}mm",
        );
        for (row, col, p, s, surf, dz) in &violations {
            let (x, y) = grid.cell_to_world(*row, *col);
            eprintln!(
                "  ({row:>3}, {col:>3}) world ({x:>6.2}, {y:>6.2}) surface {surf:>6.2}: planner top {p:>6.2}  sim top {s:>6.2}  Δ {dz:>5.2}mm",
            );
        }

        (
            divergent,
            interior_divergent,
            total_cells,
            max_dz,
            violations,
        )
    }

    /// Hypothesis 2 check: count divergent cells whose CELL CENTER is
    /// inside the mesh footprint (so boundary pre-clear cells don't
    /// inflate the number). Returns the interior-only count.
    #[allow(dead_code)]
    fn count_interior_divergence(
        violations: &[(usize, usize, f64, f64, f64, f64)],
        mesh: &TriangleMesh,
        grid_origin_u: f64,
        grid_origin_v: f64,
        cell_size: f64,
    ) -> usize {
        let bbox = &mesh.bbox;
        violations
            .iter()
            .filter(|(row, col, _, _, _, _)| {
                let x = grid_origin_u + (*col as f64) * cell_size;
                let y = grid_origin_v + (*row as f64) * cell_size;
                // "Interior" = comfortably inside the mesh XY bbox
                x > bbox.min.x + 1.0
                    && x < bbox.max.x - 1.0
                    && y > bbox.min.y + 1.0
                    && y < bbox.max.y - 1.0
            })
            .count()
    }

    #[test]
    #[ignore = "Probe: emitted Cut path point spacing on AgentSearch flat — distinguishes \
                spacing-too-coarse vs stamp-at-wrong-location"]
    fn agent_search_cut_path_point_spacing_probe() {
        let (mesh, si) = make_flat_mesh();
        let cutter = flat_cutter();
        let tool_radius = 3.175_f64;
        let stock_top_z = 25.0_f64;
        let cell_size = (tool_radius / 6.0).max(0.1);
        let r = cutter.radius();
        let bbox = mesh.bbox;
        let initial_stock = TriDexelStock::from_stock(
            bbox.min.x - r,
            bbox.min.y - r,
            bbox.max.x + r,
            bbox.max.y + r,
            bbox.min.z,
            stock_top_z,
            cell_size,
        );
        let params = Adaptive3dParams {
            initial_stock: Some(initial_stock),
            clearing_strategy: ClearingStrategy3d::AgentSearch,
            tool_radius,
            envelope_radius: tool_radius,
            stock_top_z,
            depth_per_pass: 3.0,
            stock_to_leave: 0.5,
            stepover: 1.0,
            tolerance: 0.5,
            ..default_params()
        };

        let never_cancel = || false;
        let result = adaptive_3d_segments(&mesh, &si, &cutter, &params, None, &never_cancel)
            .expect("planner should succeed");
        let segments = result.segments;

        // Histogram bins for consecutive-point planar spacing in units of
        // tool_radius. The planner's stamp uses a point stamp at each
        // sub-sampled location with `step_len = cell_size * 1.5` ≈
        // tool_radius * 0.25. If consecutive Cut path points are
        // > 2 * tool_radius apart there's a real gap between point
        // stamps that the simulator's swept stamp would fill.
        let mut bins = [0u64; 8]; // 0..0.25R, ..0.5R, ..1R, ..2R, ..4R, ..8R, ..16R, >16R
        let bin_thresholds = [0.25_f64, 0.5, 1.0, 2.0, 4.0, 8.0, 16.0, f64::INFINITY];
        let mut total_gaps = 0u64;
        let mut max_gap_mm = 0.0_f64;
        let mut max_gap_loc: Option<(P3, P3)> = None;
        let mut total_path_points = 0u64;
        let mut cut_segments_seen = 0u64;
        for seg in &segments {
            if let Adaptive3dSegment::Cut(path) = seg {
                cut_segments_seen += 1;
                total_path_points += path.len() as u64;
                for pair in path.windows(2) {
                    if let [a, b] = pair {
                        let dx = b.x - a.x;
                        let dy = b.y - a.y;
                        let gap_mm = (dx * dx + dy * dy).sqrt();
                        if gap_mm > max_gap_mm {
                            max_gap_mm = gap_mm;
                            max_gap_loc = Some((*a, *b));
                        }
                        total_gaps += 1;
                        let gap_in_radii = gap_mm / tool_radius;
                        for (i, &thr) in bin_thresholds.iter().enumerate() {
                            if gap_in_radii < thr {
                                bins[i] += 1;
                                break;
                            }
                        }
                    }
                }
            }
        }

        eprintln!(
            "AgentSearch flat-plane Cut-path spacing probe (tool_radius = {tool_radius:.3} mm):"
        );
        eprintln!(
            "  Cut segments: {cut_segments_seen}, total path points: {total_path_points}, total gaps: {total_gaps}"
        );
        eprintln!(
            "  Max consecutive-point gap: {max_gap_mm:.3} mm = {:.2}× tool_radius",
            max_gap_mm / tool_radius
        );
        if let Some((a, b)) = max_gap_loc {
            eprintln!(
                "    from ({:.2}, {:.2}, {:.2}) → ({:.2}, {:.2}, {:.2})",
                a.x, a.y, a.z, b.x, b.y, b.z
            );
        }
        eprintln!("  Gap distribution (in units of tool_radius):");
        let bin_labels = [
            "  < 0.25R",
            " < 0.5R ",
            " < 1.0R ",
            " < 2.0R ",
            " < 4.0R ",
            " < 8.0R ",
            " < 16.0R",
            " >= 16R ",
        ];
        for (label, count) in bin_labels.iter().zip(bins.iter()) {
            let pct = if total_gaps > 0 {
                *count as f64 / total_gaps as f64 * 100.0
            } else {
                0.0
            };
            eprintln!("    {label}: {count:>6}  ({pct:>5.1}%)");
        }
        let large_gaps = bins[3] + bins[4] + bins[5] + bins[6] + bins[7];
        let large_pct = if total_gaps > 0 {
            large_gaps as f64 / total_gaps as f64 * 100.0
        } else {
            0.0
        };
        eprintln!(
            "  Gaps > 1.0× tool_radius (point-stamp leaves bigger holes than radius): {large_gaps} ({large_pct:.1}%)"
        );
    }

    #[test]
    #[ignore = "Hypothesis 1: pure-flat mesh — should reveal whether the divergence is surface-coupled"]
    fn planner_sim_dexel_parity_flat_agent_search() {
        let (divergent, _interior, total, max_dz, _) =
            run_planner_sim_parity_flat(ClearingStrategy3d::AgentSearch, "AgentSearch flat");
        eprintln!("FLAT AgentSearch: {divergent}/{total} cells diverge (max Δ {max_dz:.3}mm)");
        // No assertion — diagnostic output. The hemisphere variants
        // assert; this test just prints so we can compare flat vs
        // hemisphere divergence rates side-by-side.
    }

    #[test]
    #[ignore = "Hypothesis 1: pure-flat mesh — should reveal whether the divergence is surface-coupled"]
    fn planner_sim_dexel_parity_flat_contour_parallel() {
        let (divergent, _interior, total, max_dz, _) = run_planner_sim_parity_flat(
            ClearingStrategy3d::ContourParallel,
            "ContourParallel flat",
        );
        eprintln!("FLAT ContourParallel: {divergent}/{total} cells diverge (max Δ {max_dz:.3}mm)");
    }

    /// Probe A — Bug 2 isolation. Build a toolpath that contains ONLY
    /// the Cut segments (skipping Rapid/Link/Marker), so the simulator
    /// only stamps cells the planner also stamped. If the divergence
    /// drops to near-zero on this filtered toolpath, the original
    /// divergence is dominated by Rapid/Link-derived feed moves
    /// (peck-plunges, stay-down links) which the simulator stamps but
    /// the planner does not.
    fn segments_to_cut_only_toolpath(segments: &[Adaptive3dSegment]) -> Toolpath {
        let mut tp = Toolpath::new();
        for seg in segments {
            if let Adaptive3dSegment::Cut(path) = seg {
                if let Some(first) = path.first() {
                    tp.rapid_to(*first);
                    for p in path.iter().skip(1) {
                        tp.feed_to(*p, 1000.0);
                    }
                }
            }
        }
        tp
    }

    fn run_planner_sim_parity_cut_only(
        strategy: ClearingStrategy3d,
        label: &str,
        mesh_pair: (TriangleMesh, SpatialIndex),
    ) -> (u64, u64, u64) {
        let (mesh, si) = mesh_pair;
        let mesh_bbox = mesh.bbox;
        let cutter = flat_cutter();
        let r = cutter.radius();
        let tool_radius = 3.175_f64;
        let stock_top_z = 25.0_f64;
        let cell_size = (tool_radius / 6.0).max(0.1);
        let initial_stock = TriDexelStock::from_stock(
            mesh_bbox.min.x - r,
            mesh_bbox.min.y - r,
            mesh_bbox.max.x + r,
            mesh_bbox.max.y + r,
            mesh_bbox.min.z,
            stock_top_z,
            cell_size,
        );

        let params = Adaptive3dParams {
            initial_stock: Some(initial_stock.clone()),
            clearing_strategy: strategy,
            tool_radius,
            envelope_radius: tool_radius,
            stock_top_z,
            depth_per_pass: 3.0,
            stock_to_leave: 0.5,
            stepover: 1.0,
            tolerance: 0.5,
            ..default_params()
        };

        let never_cancel = || false;
        let result = adaptive_3d_segments(&mesh, &si, &cutter, &params, None, &never_cancel)
            .expect("planner should succeed");
        let planner_stock = result.final_material_stock;
        let segments = result.segments;

        // CUT-ONLY toolpath: skip Rapid/Link/Marker entirely.
        let cut_only_toolpath = segments_to_cut_only_toolpath(&segments);

        let mut sim_stock = initial_stock;
        sim_stock
            .simulate_toolpath_with_metrics_with_cancel(
                &cut_only_toolpath,
                &cutter,
                StockCutDirection::FromTop,
                0,
                12_000,
                2,
                3000.0,
                0.5,
                None,
                true,
                &never_cancel,
            )
            .expect("simulator should succeed");

        let tol_mm = cell_size;
        let grid = &planner_stock.z_grid;
        let total = (grid.rows * grid.cols) as u64;
        let mut divergent = 0u64;
        let mut interior = 0u64;
        let interior_x_lo = mesh_bbox.min.x + 1.0;
        let interior_x_hi = mesh_bbox.max.x - 1.0;
        let interior_y_lo = mesh_bbox.min.y + 1.0;
        let interior_y_hi = mesh_bbox.max.y - 1.0;
        for row in 0..grid.rows {
            for col in 0..grid.cols {
                let p = stock_top_z_at(&planner_stock, row, col);
                let s = stock_top_z_at(&sim_stock, row, col);
                if (p - s).abs() > tol_mm {
                    divergent += 1;
                    let (x, y) = grid.cell_to_world(row, col);
                    if x > interior_x_lo
                        && x < interior_x_hi
                        && y > interior_y_lo
                        && y < interior_y_hi
                    {
                        interior += 1;
                    }
                }
            }
        }
        eprintln!("[{label} CUT-ONLY] {divergent}/{total} cells diverge; interior {interior}",);
        (divergent, total, interior)
    }

    #[test]
    #[ignore = "Probe A: Cut-only toolpath isolates Bug 2 (Rapid/Link/peck-plunge stamping)"]
    fn planner_sim_parity_cut_only_agent_search_flat() {
        let _ = run_planner_sim_parity_cut_only(
            ClearingStrategy3d::AgentSearch,
            "AgentSearch flat",
            make_flat_mesh(),
        );
    }

    #[test]
    #[ignore = "Probe A: Cut-only toolpath isolates Bug 2 (Rapid/Link/peck-plunge stamping)"]
    fn planner_sim_parity_cut_only_agent_search_hemisphere() {
        let _ = run_planner_sim_parity_cut_only(
            ClearingStrategy3d::AgentSearch,
            "AgentSearch hemisphere",
            make_hemisphere_mesh(),
        );
    }

    #[test]
    #[ignore = "Probe A: Cut-only toolpath isolates Bug 2 (Rapid/Link/peck-plunge stamping)"]
    fn planner_sim_parity_cut_only_contour_parallel_flat() {
        let _ = run_planner_sim_parity_cut_only(
            ClearingStrategy3d::ContourParallel,
            "ContourParallel flat",
            make_flat_mesh(),
        );
    }

    #[test]
    #[ignore = "Probe A: Cut-only toolpath isolates Bug 2 (Rapid/Link/peck-plunge stamping)"]
    fn planner_sim_parity_cut_only_contour_parallel_hemisphere() {
        let _ = run_planner_sim_parity_cut_only(
            ClearingStrategy3d::ContourParallel,
            "ContourParallel hemisphere",
            make_hemisphere_mesh(),
        );
    }

    #[test]
    fn planner_sim_dexel_parity_agent_search() {
        let (_divergent, interior, total, max_dz, _violations) =
            run_planner_sim_parity(ClearingStrategy3d::AgentSearch, "AgentSearch hemisphere");
        // Interior threshold: < 1% interior cells. Boundary divergence
        // (cells outside the mesh footprint) is a separate, known issue
        // outside the scope of this parity test — see the planner-↔-sim
        // stamping fix notes (Bug 1 / Bug 2). What this test guards is
        // INSIDE-the-mesh stamping consistency.
        let threshold = total / 100;
        assert!(
            interior <= threshold,
            "Planner and simulator dexels diverged on {interior} INTERIOR cells \
             (total {total}, threshold {threshold}, max Δ {max_dz:.3}mm). The \
             planner's internal stamping is producing a stock state inconsistent \
             with replaying its own emitted moves — this is the wanaka Back \
             Rough Z=10/Z=7 anomaly. See test source for debugger entry point.",
        );
    }

    #[test]
    fn planner_sim_dexel_parity_contour_parallel() {
        let (_divergent, interior, total, max_dz, _violations) = run_planner_sim_parity(
            ClearingStrategy3d::ContourParallel,
            "ContourParallel hemisphere",
        );
        let threshold = total / 100;
        assert!(
            interior <= threshold,
            "Planner and simulator dexels diverged on {interior} INTERIOR cells \
             (total {total}, threshold {threshold}, max Δ {max_dz:.3}mm). If \
             AgentSearch's parity test passes but this one fails, the divergence \
             is in ContourParallel's stamping path (and vice versa) — useful \
             first bisection.",
        );
    }
}
