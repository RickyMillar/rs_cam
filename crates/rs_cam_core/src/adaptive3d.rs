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

use crate::adaptive_shared::{
    angle_diff, average_angles, blend_corners, refine_angle_bracket, target_engagement_fraction,
};
use crate::contour_extract::{edt_curvature_field, marching_squares_bool_grid, smooth_grid};
use crate::debug_trace::{HotspotRecord, ToolpathDebugBounds2, ToolpathDebugContext};
use crate::dexel::{ray_subtract_above, ray_top};
use crate::dexel_stock::{StockCutDirection, TriDexelStock};
use crate::dropcutter::point_drop_cutter;
use crate::geo::{P2, P3};
use crate::interrupt::{CancelCheck, Cancelled, check_cancel};
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::radial_profile::RadialProfileLUT;
use crate::slope::{SlopeMap, SurfaceHeightmap};
use crate::tool::MillingCutter;
use crate::toolpath::{Toolpath, simplify_path_3d};
use crate::waterline::waterline_contours_with_cancel;

use std::collections::VecDeque;
use std::f64::consts::{PI, TAU};
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;
use tracing::{debug, info};

#[derive(Debug, Clone, Copy)]
struct SearchDirection3dResult {
    angle: f64,
    z_next: f64,
    evaluations: u32,
}

fn path_bounds_3d(path: &[P3]) -> Option<ToolpathDebugBounds2> {
    let points: Vec<(f64, f64)> = path.iter().map(|point| (point.x, point.y)).collect();
    ToolpathDebugBounds2::from_points(points.iter())
}

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

/// Clearing strategy for each Z level.
///
/// Roughing strategy for 3D clearing.
///
/// `ContourParallel` — fast, predictable contour-offset pocketing via EDT.
///   Fixed stepover, concentric contours from boundary inward. Best for
///   bulk roughing where speed matters more than constant engagement.
///
/// `Adaptive` — true constant-engagement clearing with variable stepover.
///   Arcs around corners to maintain the engagement setpoint. Slower but
///   better tool life and surface quality. (TODO: not yet implemented —
///   falls back to ContourParallel)
///
/// `AgentSearch` — legacy per-step direction search. Retained for testing.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ClearingStrategy3d {
    /// Legacy per-step direction search (for testing).
    AgentSearch,
    /// Fast contour-parallel offset clearing via EDT (default).
    #[default]
    ContourParallel,
    /// Constant-engagement adaptive clearing via curvature-adjusted EDT offsets.
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
    pub tool_radius: f64,
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
    /// Clearing strategy per Z level (default: AgentSearch for backward compat).
    pub clearing_strategy: ClearingStrategy3d,
    /// Blend Z toward terrain surface across contour offsets.
    /// When true, outer contours stay near z_level and inner contours
    /// progressively descend toward the surface. Best for terrain/relief.
    /// When false (default), all contours cut at z_level. Best for pockets.
    pub z_blend: bool,
}

// SurfaceHeightmap is now in crate::slope (shared across finishing strategies)

// ── Helpers mapping TriDexelStock to f64 world used by adaptive ────────

/// Top Z at (row, col) from the Z-grid, as f64. Returns `bottom_z` if the
/// ray is empty (no material).
#[inline]
fn stock_top_z_at(stock: &TriDexelStock, row: usize, col: usize) -> f64 {
    ray_top(stock.z_grid.ray(row, col))
        .map(|z| z as f64)
        .unwrap_or(stock.stock_bbox.min.z)
}

/// Whether the Z-grid ray at (row, col) has material above `floor` (f64).
#[inline]
fn stock_has_material_above(stock: &TriDexelStock, row: usize, col: usize, floor: f64) -> bool {
    let ray = stock.z_grid.ray(row, col);
    // Any segment whose exit > floor means material above floor.
    ray.iter().any(|seg| seg.exit as f64 > floor)
}

/// Sum of top-Z values in a local area around (cx, cy) within radius.
/// Used for cheap O(local_cells) idle detection instead of summing entire grid.
#[inline]
fn local_material_sum(stock: &TriDexelStock, cx: f64, cy: f64, radius: f64) -> f64 {
    let grid = &stock.z_grid;
    let cs = grid.cell_size;
    let r = radius * 1.5; // Slightly wider to catch changes from the stamp
    let col_min = ((cx - r - grid.origin_u) / cs).floor().max(0.0) as usize;
    let col_max = ((cx + r - grid.origin_u) / cs)
        .ceil()
        .min((grid.cols - 1) as f64) as usize;
    let row_min = ((cy - r - grid.origin_v) / cs).floor().max(0.0) as usize;
    let row_max = ((cy + r - grid.origin_v) / cs)
        .ceil()
        .min((grid.rows - 1) as f64) as usize;

    let bottom_z = stock.stock_bbox.min.z;
    let mut sum = 0.0;
    for row in row_min..=row_max {
        for col in col_min..=col_max {
            sum += ray_top(grid.ray(row, col))
                .map(|z| z as f64)
                .unwrap_or(bottom_z);
        }
    }
    sum
}

// ── Region detection ──────────────────────────────────────────────────

/// A connected region of material detected by flood fill on the heightmap.
struct MaterialRegion {
    row_min: usize,
    row_max: usize,
    col_min: usize,
    col_max: usize,
    /// World-space bounding box (expanded by tool_radius for direction search).
    world_x_min: f64,
    world_x_max: f64,
    world_y_min: f64,
    world_y_max: f64,
    cell_count: usize,
    surface_z_min: f64,
    surface_z_max: f64,
}

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// Detect connected material regions via 8-connected BFS flood fill.
///
/// A cell "has material" if the top-Z of the dexel ray exceeds
/// `surface_z + stock_to_leave + 0.01`.
/// Regions with fewer than `min_cells` (default 4) are filtered out.
/// Returns regions sorted by cell_count descending (largest first).
fn detect_material_regions(
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
            let surf_z = surface_hm.surface_z_at(row, col);
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
                let sz = surface_hm.surface_z_at(r, c);
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

// ── 3D engagement ─────────────────────────────────────────────────────

/// Compute 3D engagement at position (cx, cy) for a given z_level.
///
/// For each cell in the tool circle, material exists if:
///   material_heightmap_z > max(surface_z + stock_to_leave, z_level) + epsilon
///
/// Returns fraction of cells with material in [0.0, 1.0].
fn compute_engagement_3d(
    material_stock: &TriDexelStock,
    surface_hm: &SurfaceHeightmap,
    cx: f64,
    cy: f64,
    tool_radius: f64,
    z_level: f64,
    stock_to_leave: f64,
) -> f64 {
    let r_sq = tool_radius * tool_radius;
    let grid = &material_stock.z_grid;
    let cs = grid.cell_size;

    let col_min = ((cx - tool_radius - grid.origin_u) / cs).floor().max(0.0) as usize;
    let col_max = ((cx + tool_radius - grid.origin_u) / cs).ceil() as usize;
    let row_min = ((cy - tool_radius - grid.origin_v) / cs).floor().max(0.0) as usize;
    let row_max = ((cy + tool_radius - grid.origin_v) / cs).ceil() as usize;

    let col_max = col_max.min(grid.cols.saturating_sub(1));
    let row_max = row_max.min(grid.rows.saturating_sub(1));

    let mut material_cells = 0u32;
    let mut total_cells = 0u32;

    for row in row_min..=row_max {
        let cell_y = grid.origin_v + row as f64 * cs;
        let dy = cell_y - cy;
        let dy_sq = dy * dy;
        if dy_sq > r_sq {
            continue;
        }

        for col in col_min..=col_max {
            let cell_x = grid.origin_u + col as f64 * cs;
            let dx = cell_x - cx;
            if dx * dx + dy_sq > r_sq {
                continue;
            }

            total_cells += 1;
            let surf_z = surface_hm.surface_z_at(row, col);
            let effective_floor = (surf_z + stock_to_leave).max(z_level);

            if stock_has_material_above(material_stock, row, col, effective_floor + 0.01) {
                material_cells += 1;
            }
        }
    }

    if total_cells == 0 {
        0.0
    } else {
        material_cells as f64 / total_cells as f64
    }
}

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// Fraction of grid cells where material remains above the effective floor
/// at a given z_level. Used to decide when a level is done.
fn material_remaining_at_level(
    material_stock: &TriDexelStock,
    surface_hm: &SurfaceHeightmap,
    z_level: f64,
    stock_to_leave: f64,
) -> f64 {
    let grid = &material_stock.z_grid;
    let mut above = 0u64;
    let mut total = 0u64;
    for row in 0..grid.rows {
        for col in 0..grid.cols {
            let i = row * grid.cols + col;
            let surf_z = surface_hm.z_values[i];
            let floor = (surf_z + stock_to_leave).max(z_level);
            // Only count cells where the surface is actually below the current level
            // (cells where surface is above z_level were handled at higher levels)
            if surf_z + stock_to_leave <= z_level + 0.01 {
                total += 1;
                if stock_has_material_above(material_stock, row, col, floor + 0.01) {
                    above += 1;
                }
            }
        }
    }
    if total == 0 {
        0.0
    } else {
        above as f64 / total as f64
    }
}

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// Bbox-restricted version of `material_remaining_at_level()`.
/// Only scans cells within the region's row/col bounding box.
fn material_remaining_in_region(
    material_stock: &TriDexelStock,
    surface_hm: &SurfaceHeightmap,
    z_level: f64,
    stock_to_leave: f64,
    region: &MaterialRegion,
) -> f64 {
    let grid = &material_stock.z_grid;
    let mut above = 0u64;
    let mut total = 0u64;
    for row in region.row_min..=region.row_max.min(grid.rows - 1) {
        for col in region.col_min..=region.col_max.min(grid.cols - 1) {
            let i = row * grid.cols + col;
            let surf_z = surface_hm.z_values[i];
            let floor = (surf_z + stock_to_leave).max(z_level);
            if surf_z + stock_to_leave <= z_level + 0.01 {
                total += 1;
                if stock_has_material_above(material_stock, row, col, floor + 0.01) {
                    above += 1;
                }
            }
        }
    }
    if total == 0 {
        0.0
    } else {
        above as f64 / total as f64
    }
}

// ── 3D direction search ───────────────────────────────────────────────

/// Search for the best direction to move from (cx, cy) that achieves
/// target engagement. Returns (angle, z_at_next_position).
///
/// Three-phase search (same as 2D adaptive):
/// 1. Narrow interpolation (7 candidates near prev_angle + bracket refinement)
/// 2. Forward sweep +/-90 (19 candidates)
/// 3. Full 360 (36 candidates)
#[allow(clippy::too_many_arguments)]
#[allow(dead_code)]
fn search_direction_3d(
    material_stock: &TriDexelStock,
    surface_hm: &SurfaceHeightmap,
    cx: f64,
    cy: f64,
    tool_radius: f64,
    step_len: f64,
    target_frac: f64,
    prev_angle: f64,
    z_level: f64,
    stock_to_leave: f64,
    bbox_x_min: f64,
    bbox_x_max: f64,
    bbox_y_min: f64,
    bbox_y_max: f64,
) -> Option<(f64, f64)> {
    search_direction_3d_with_metrics(
        material_stock,
        surface_hm,
        cx,
        cy,
        tool_radius,
        step_len,
        target_frac,
        prev_angle,
        z_level,
        stock_to_leave,
        bbox_x_min,
        bbox_x_max,
        bbox_y_min,
        bbox_y_max,
    )
    .map(|result| (result.angle, result.z_next))
}

#[allow(clippy::too_many_arguments)]
fn search_direction_3d_with_metrics(
    material_stock: &TriDexelStock,
    surface_hm: &SurfaceHeightmap,
    cx: f64,
    cy: f64,
    tool_radius: f64,
    step_len: f64,
    target_frac: f64,
    prev_angle: f64,
    z_level: f64,
    stock_to_leave: f64,
    bbox_x_min: f64,
    bbox_x_max: f64,
    bbox_y_min: f64,
    bbox_y_max: f64,
) -> Option<SearchDirection3dResult> {
    // Evaluate a candidate direction: returns (engagement, score, z_at_next) or None.
    // Uses precomputed surface heightmap for Z lookups (O(1)) instead of drop-cutter queries.
    let mut evaluations = 0u32;
    let mut evaluate = |angle: f64| -> Option<(f64, f64, (f64, f64))> {
        evaluations += 1;
        let (sin_a, cos_a) = angle.sin_cos();
        let nx = cx + step_len * cos_a;
        let ny = cy + step_len * sin_a;

        // Bounds check
        if nx < bbox_x_min || nx > bbox_x_max || ny < bbox_y_min || ny > bbox_y_max {
            return None;
        }

        // Z from precomputed surface heightmap (O(1) vs O(k) for drop-cutter)
        let surf_z = surface_hm.surface_z_at_world(nx, ny);
        if surf_z == f64::NEG_INFINITY {
            return None;
        }
        let z = (surf_z + stock_to_leave).max(z_level);

        // Engagement at candidate
        let eng = compute_engagement_3d(
            material_stock,
            surface_hm,
            nx,
            ny,
            tool_radius,
            z_level,
            stock_to_leave,
        );

        if eng < 0.001 {
            return None;
        }

        let error = (eng - target_frac).abs();
        let ad = angle_diff(angle, prev_angle).abs() / PI;
        let score = error + ad * 0.12;

        Some((angle, eng, (score, z)))
    };

    // Phase 1: Narrow interpolation — 7 candidates near prev_angle
    let narrow_offsets = [0.0, 15.0_f64, -15.0, 30.0, -30.0, 45.0, -45.0];
    let mut best: Option<(f64, f64, f64)> = None; // (score, angle, z)

    // Track brackets for interpolation
    let mut bracket_lo: Option<(f64, f64, (f64, f64))> = None; // (angle, eng, (score, z))
    let mut bracket_hi: Option<(f64, f64, (f64, f64))> = None; // (angle, eng, (score, z))

    for &offset_deg in &narrow_offsets {
        let angle = prev_angle + offset_deg.to_radians();
        let (sin_a, cos_a) = angle.sin_cos();
        let nx = cx + step_len * cos_a;
        let ny = cy + step_len * sin_a;

        if nx < bbox_x_min || nx > bbox_x_max || ny < bbox_y_min || ny > bbox_y_max {
            continue;
        }

        let surf_z = surface_hm.surface_z_at_world(nx, ny);
        if surf_z == f64::NEG_INFINITY {
            continue;
        }
        let z = (surf_z + stock_to_leave).max(z_level);
        let eng = compute_engagement_3d(
            material_stock,
            surface_hm,
            nx,
            ny,
            tool_radius,
            z_level,
            stock_to_leave,
        );

        if eng < 0.001 {
            continue;
        }

        let error = (eng - target_frac).abs();
        let ad = angle_diff(angle, prev_angle).abs() / PI;
        let score = error + ad * 0.12;

        if best.is_none_or(|b| score < b.0) {
            best = Some((score, angle, z));
        }

        // Track brackets for interpolation
        if eng < target_frac {
            if bracket_lo.is_none_or(|b| (eng - target_frac).abs() < (b.1 - target_frac).abs()) {
                bracket_lo = Some((angle, eng, (score, z)));
            }
        } else if bracket_hi.is_none_or(|b| (eng - target_frac).abs() < (b.1 - target_frac).abs()) {
            bracket_hi = Some((angle, eng, (score, z)));
        }
    }

    if let (Some(lo), Some(hi)) = (bracket_lo, bracket_hi)
        && let Some((angle, _eng, (score, z))) =
            refine_angle_bracket(lo, hi, target_frac, 1, &mut evaluate)
        && best.is_none_or(|b| score < b.0)
    {
        best = Some((score, angle, z));
    }

    // If narrow search found a good result, return it
    if let Some((score, angle, z)) = best
        && score < 0.15
    {
        return Some(SearchDirection3dResult {
            angle,
            z_next: z,
            evaluations,
        });
    }

    // ── Phase 2: Coarse 360° scan + bracket refinement ────────────────
    // 18 candidates at 20° intervals (vs 55 in the old Phase 2+3)
    {
        let n_coarse = 18;
        let mut fallback: Option<(f64, f64, f64)> = best; // carry over from Phase 1
        let mut coarse_lo: Option<(f64, f64, (f64, f64))> = None; // (angle, eng, (score, z))
        let mut coarse_hi: Option<(f64, f64, (f64, f64))> = None;

        for i in 0..n_coarse {
            let angle = (i as f64 / n_coarse as f64) * TAU;
            if let Some((angle, eng, (score, z))) = evaluate(angle) {
                if fallback.is_none_or(|b| score < b.0) {
                    fallback = Some((score, angle, z));
                }
                if eng < target_frac {
                    if coarse_lo
                        .is_none_or(|b| (eng - target_frac).abs() < (b.1 - target_frac).abs())
                    {
                        coarse_lo = Some((angle, eng, (score, z)));
                    }
                } else if coarse_hi
                    .is_none_or(|b| (eng - target_frac).abs() < (b.1 - target_frac).abs())
                {
                    coarse_hi = Some((angle, eng, (score, z)));
                }
            }
        }

        if let (Some(lo), Some(hi)) = (coarse_lo, coarse_hi)
            && let Some((angle, _eng, (score, z))) =
                refine_angle_bracket(lo, hi, target_frac, 2, evaluate)
            && fallback.is_none_or(|b| score < b.0)
        {
            fallback = Some((score, angle, z));
        }

        fallback.map(|(_, angle, z)| SearchDirection3dResult {
            angle,
            z_next: z,
            evaluations,
        })
    }
}

// ── 3D entry point finding ────────────────────────────────────────────

/// Find the next entry point: a cell where material remains above the
/// effective floor at z_level.
///
/// When `scan_bbox` is `Some((row_min, row_max, col_min, col_max))`, only
/// cells within that bounding box are considered. When `None`, uses
/// growing-radius search from the reference position for O(local) instead
/// of O(rows×cols).
#[allow(clippy::too_many_arguments)]
fn find_entry_3d(
    material_stock: &TriDexelStock,
    surface_hm: &SurfaceHeightmap,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    z_level: f64,
    stock_to_leave: f64,
    last_pos: Option<P2>,
    pass_endpoints: &[P2],
    tool_radius: f64,
    scan_bbox: Option<(usize, usize, usize, usize)>,
) -> Option<(P2, f64)> {
    let min_endpoint_dist_sq = (tool_radius * 3.0) * (tool_radius * 3.0);
    let grid = &material_stock.z_grid;

    // Reference position for nearest search
    let ref_pos = last_pos.unwrap_or_else(|| {
        let cx = grid.origin_u + (grid.cols as f64 / 2.0) * grid.cell_size;
        let cy = grid.origin_v + (grid.rows as f64 / 2.0) * grid.cell_size;
        P2::new(cx, cy)
    });

    // Growing-radius search when no explicit bbox
    if scan_bbox.is_none() {
        let cs = grid.cell_size;
        let initial_radius = tool_radius * 4.0;
        let max_extent = (grid.cols as f64 * cs).max(grid.rows as f64 * cs);

        let mut radius = initial_radius;
        while radius <= max_extent * 1.5 {
            let row_lo = ((ref_pos.y - radius - grid.origin_v) / cs).floor().max(0.0) as usize;
            let row_hi = ((ref_pos.y + radius - grid.origin_v) / cs)
                .ceil()
                .min(grid.rows.saturating_sub(1) as f64) as usize;
            let col_lo = ((ref_pos.x - radius - grid.origin_u) / cs).floor().max(0.0) as usize;
            let col_hi = ((ref_pos.x + radius - grid.origin_u) / cs)
                .ceil()
                .min(grid.cols.saturating_sub(1) as f64) as usize;

            if let Some(result) = scan_entry_3d_bounds(
                material_stock,
                surface_hm,
                mesh,
                index,
                cutter,
                z_level,
                stock_to_leave,
                &ref_pos,
                pass_endpoints,
                min_endpoint_dist_sq,
                row_lo,
                row_hi,
                col_lo,
                col_hi,
            ) {
                return Some(result);
            }
            radius *= 2.0;
        }
    }

    // Full scan (explicit bbox or growing radius exhausted)
    let (row_lo, row_hi, col_lo, col_hi) = scan_bbox.unwrap_or((
        0,
        grid.rows.saturating_sub(1),
        0,
        grid.cols.saturating_sub(1),
    ));

    scan_entry_3d_bounds(
        material_stock,
        surface_hm,
        mesh,
        index,
        cutter,
        z_level,
        stock_to_leave,
        &ref_pos,
        pass_endpoints,
        min_endpoint_dist_sq,
        row_lo,
        row_hi,
        col_lo,
        col_hi,
    )
}

/// Scan a bounded region of the stock grid for the nearest entry point.
#[allow(clippy::too_many_arguments)]
fn scan_entry_3d_bounds(
    material_stock: &TriDexelStock,
    surface_hm: &SurfaceHeightmap,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    z_level: f64,
    stock_to_leave: f64,
    ref_pos: &P2,
    pass_endpoints: &[P2],
    min_endpoint_dist_sq: f64,
    row_lo: usize,
    row_hi: usize,
    col_lo: usize,
    col_hi: usize,
) -> Option<(P2, f64)> {
    let grid = &material_stock.z_grid;
    let row_hi = row_hi.min(grid.rows.saturating_sub(1));
    let col_hi = col_hi.min(grid.cols.saturating_sub(1));

    let mut best: Option<(f64, usize, usize)> = None; // (dist_sq, row, col)

    for row in row_lo..=row_hi {
        for col in col_lo..=col_hi {
            let surf_z = surface_hm.surface_z_at(row, col);
            let floor = (surf_z + stock_to_leave).max(z_level);

            if !stock_has_material_above(material_stock, row, col, floor + 0.01) {
                continue;
            }

            let (x, y) = grid.cell_to_world(row, col);

            let too_close = pass_endpoints.iter().any(|ep| {
                let dx = x - ep.x;
                let dy = y - ep.y;
                dx * dx + dy * dy < min_endpoint_dist_sq
            });
            if too_close && pass_endpoints.len() < 50 {
                continue;
            }

            let dx = x - ref_pos.x;
            let dy = y - ref_pos.y;
            let dist_sq = dx * dx + dy * dy;

            if best.is_none_or(|b| dist_sq < b.0) {
                best = Some((dist_sq, row, col));
            }
        }
    }

    // If spreading excluded everything, retry without spreading
    if best.is_none() {
        for row in row_lo..=row_hi {
            for col in col_lo..=col_hi {
                let surf_z = surface_hm.surface_z_at(row, col);
                let floor = (surf_z + stock_to_leave).max(z_level);

                if !stock_has_material_above(material_stock, row, col, floor + 0.01) {
                    continue;
                }

                let (x, y) = grid.cell_to_world(row, col);
                let dx = x - ref_pos.x;
                let dy = y - ref_pos.y;
                let dist_sq = dx * dx + dy * dy;

                if best.is_none_or(|b| dist_sq < b.0) {
                    best = Some((dist_sq, row, col));
                }
            }
        }
    }

    best.map(|(_, row, col)| {
        let (x, y) = grid.cell_to_world(row, col);
        let cl = point_drop_cutter(x, y, mesh, index, cutter);
        let z = (cl.z + stock_to_leave).max(z_level);
        (P2::new(x, y), z)
    })
}

// ── Link vs retract ───────────────────────────────────────────────────

/// Check if the tool can safely feed from `from` to `to` without hitting
/// excessive material above the cutting plane.
fn is_clear_path_3d(
    material_stock: &TriDexelStock,
    _surface_hm: &SurfaceHeightmap,
    from: P3,
    to: P3,
    _z_level: f64,
    _stock_to_leave: f64,
) -> bool {
    let dx = to.x - from.x;
    let dy = to.y - from.y;
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1e-10 {
        return true;
    }

    let grid = &material_stock.z_grid;
    let n_samples = (len / (grid.cell_size * 2.0)).ceil() as usize;
    let n_samples = n_samples.max(2);
    let mut blocked = 0u32;

    for i in 0..=n_samples {
        let t = i as f64 / n_samples as f64;
        let x = from.x + t * dx;
        let y = from.y + t * dy;
        let z = from.z + t * (to.z - from.z);

        if let Some((row, col)) = grid.world_to_cell(x, y) {
            let mat_z = stock_top_z_at(material_stock, row, col);
            // Material significantly above our travel Z means collision
            if mat_z > z + 1.0 {
                blocked += 1;
            }
        }
    }

    let blocked_frac = blocked as f64 / (n_samples + 1) as f64;
    blocked_frac < 0.2
}

// ── 3D path simplification ───────────────────────────────────────────

/// Blend corners on a 3D path. Projects to 2D for geometry, interpolates Z.
fn blend_corners_3d(path: &[P3], min_radius: f64) -> Vec<P3> {
    if min_radius <= 0.0 || path.len() < 3 {
        return path.to_vec();
    }

    // Project to 2D, blend, then re-attach Z by parameter interpolation
    let path_2d: Vec<P2> = path.iter().map(|p| P2::new(p.x, p.y)).collect();
    let blended_2d = blend_corners(&path_2d, min_radius);

    if blended_2d.len() == path_2d.len() {
        // No blending happened, return original
        return path.to_vec();
    }

    // Re-attach Z: for each blended 2D point, find nearest original point's Z
    // and interpolate. Walk the original path to find the closest segment.
    blended_2d
        .iter()
        .map(|bp| {
            let z = interpolate_z_from_path(path, bp.x, bp.y);
            P3::new(bp.x, bp.y, z)
        })
        .collect()
}

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// Find Z at (x, y) by finding the nearest segment on the original 3D path
/// and interpolating linearly.
fn interpolate_z_from_path(path: &[P3], x: f64, y: f64) -> f64 {
    if path.is_empty() {
        return 0.0;
    }
    if path.len() == 1 {
        return path[0].z;
    }

    let mut best_dist_sq = f64::INFINITY;
    let mut best_z = path[0].z;

    for i in 0..path.len() - 1 {
        let a = &path[i];
        let b = &path[i + 1];
        let dx = b.x - a.x;
        let dy = b.y - a.y;
        let seg_len_sq = dx * dx + dy * dy;

        let t = if seg_len_sq < 1e-20 {
            0.0
        } else {
            ((x - a.x) * dx + (y - a.y) * dy) / seg_len_sq
        };
        let t = t.clamp(0.0, 1.0);

        let px = a.x + t * dx;
        let py = a.y + t * dy;
        let dist_sq = (x - px) * (x - px) + (y - py) * (y - py);

        if dist_sq < best_dist_sq {
            best_dist_sq = dist_sq;
            best_z = a.z + t * (b.z - a.z);
        }
    }

    best_z
}

// ── Segment types ─────────────────────────────────────────────────────

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
    },
    GlobalZLevel {
        z_level: f64,
        level_index: usize,
        level_total: usize,
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
            } => format!(
                "Region {region_index} — Z {:.1} ({level_index}/{level_total})",
                z_level
            ),
            Self::GlobalZLevel {
                z_level,
                level_index,
                level_total,
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

enum Adaptive3dSegment {
    /// 3D cutting path with variable Z
    Cut(Vec<P3>),
    /// Retract to safe_z, rapid XY, plunge to entry
    Rapid(P3),
    /// Feed directly at cutting depth (no retract)
    Link(P3),
    /// Structured runtime marker at the current point in the toolpath
    Marker(Adaptive3dRuntimeEvent),
}

// ── Z-level clearing helper ──────────────────────────────────────────

/// Parameters for a single Z-level clearing pass, extracted to avoid
/// threading dozens of locals through the helper.
struct ClearZLevelContext<'a> {
    mesh: &'a TriangleMesh,
    index: &'a SpatialIndex,
    cutter: &'a dyn MillingCutter,
    lut: &'a RadialProfileLUT,
    slope_map: &'a SlopeMap,
    debug: Option<ToolpathDebugContext>,
    tool_radius: f64,
    stepover: f64,
    stock_to_leave: f64,
    depth_per_pass: f64,
    tolerance: f64,
    target_frac: f64,
    step_len: f64,
    max_link_dist: f64,
    bbox_x_min: f64,
    bbox_x_max: f64,
    bbox_y_min: f64,
    bbox_y_max: f64,
    clearing_strategy: ClearingStrategy3d,
    z_blend: bool,
}

/// Pre-stamp thin material bands that appear at each Z level on steep walls.
///
/// After cutting at a previous Z level, wall cells retain material_z equal to that
#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// level. At the new (lower) Z level, these cells have a thin band of material
/// (material_z - effective_floor) that is technically real but produces unproductive
/// contour passes. This function directly cuts those thin bands at the cell level,
/// leaving waterline cleanup to handle the actual wall boundaries.
///
/// Returns the number of cells pre-stamped.
fn pre_stamp_thin_bands(
    material_stock: &mut TriDexelStock,
    surface_hm: &SurfaceHeightmap,
    slope_map: &SlopeMap,
    z_level: f64,
    stock_to_leave: f64,
    depth_per_pass: f64,
    region: Option<&MaterialRegion>,
) -> u32 {
    let thin_threshold = depth_per_pass * 0.3;
    // Only pre-stamp on steep walls (>60°). Shallow areas have thin bands
    // that represent real productive material worth clearing with the adaptive spiral.
    let steep_threshold = 60.0_f64.to_radians();
    let mut stamped = 0u32;

    let grid = &material_stock.z_grid;
    let cols = grid.cols;
    let (row_min, row_max, col_min, col_max) = if let Some(r) = region {
        (
            r.row_min,
            r.row_max.min(grid.rows - 1),
            r.col_min,
            r.col_max.min(grid.cols - 1),
        )
    } else {
        (0, grid.rows - 1, 0, grid.cols - 1)
    };

    for row in row_min..=row_max {
        let base = row * cols;
        for col in col_min..=col_max {
            let i = base + col;

            // Skip shallow cells — only pre-stamp steep wall bands
            if slope_map.angles[i] < steep_threshold {
                continue;
            }

            let mat_z = ray_top(material_stock.z_grid.ray(row, col))
                .map(|z| z as f64)
                .unwrap_or(material_stock.stock_bbox.min.z);
            let surf_z = surface_hm.z_values[i];
            let effective_floor = (surf_z + stock_to_leave).max(z_level);
            let thickness = mat_z - effective_floor;
            if thickness > 0.01 && thickness < thin_threshold {
                ray_subtract_above(
                    material_stock.z_grid.ray_mut(row, col),
                    effective_floor as f32,
                );
                stamped += 1;
            }
        }
    }

    stamped
}

// ── Contour-parallel clearing ─────────────────────────────────────────

/// Build a padded boolean grid of material cells at a given Z level.
///
/// A cell is `true` if the stock has material above the effective floor
/// (max of surface_z + stock_to_leave, z_level). The grid is padded with
/// a 1-cell false border so marching squares and EDT detect edge boundaries.
///
/// Returns `(padded_grid, padded_rows, padded_cols, origin_x, origin_y, cell_size)`.
#[allow(clippy::indexing_slicing)] // SAFETY: padded grid indices bounded by loop ranges
fn build_material_bool_grid(
    material_stock: &TriDexelStock,
    surface_hm: &SurfaceHeightmap,
    z_level: f64,
    stock_to_leave: f64,
    region: Option<&MaterialRegion>,
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

            let surf_z = surface_hm.surface_z_at(row, col);
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

/// Stamp dexel stock along a 3D cutting path at `step_len` intervals.
fn stamp_along_path(
    material_stock: &mut TriDexelStock,
    lut: &RadialProfileLUT,
    tool_radius: f64,
    path: &[P3],
    step_len: f64,
) {
    let first = match path.first() {
        Some(p) => *p,
        None => return,
    };
    let mut travel = 0.0;
    let mut prev = first;
    for pt in path {
        let dx = pt.x - prev.x;
        let dy = pt.y - prev.y;
        let seg_len = (dx * dx + dy * dy).sqrt();
        travel += seg_len;
        if travel >= step_len {
            material_stock.stamp_tool_at(
                lut,
                tool_radius,
                pt.x,
                pt.y,
                pt.z,
                StockCutDirection::FromTop,
            );
            travel = 0.0;
        }
        prev = *pt;
    }
    // Stamp at the last point regardless.
    material_stock.stamp_tool_at(
        lut,
        tool_radius,
        prev.x,
        prev.y,
        prev.z,
        StockCutDirection::FromTop,
    );
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
fn clear_z_level_contour_parallel(
    ctx: &ClearZLevelContext<'_>,
    material_stock: &mut TriDexelStock,
    surface_hm: &SurfaceHeightmap,
    z_level: f64,
    segments: &mut Vec<Adaptive3dSegment>,
    last_pos: &mut Option<P3>,
    region: Option<&MaterialRegion>,
    cancel: &dyn CancelCheck,
) -> Result<(), Cancelled> {
    // Check material remaining — skip if negligible.
    let remaining = if let Some(r) = region {
        material_remaining_in_region(material_stock, surface_hm, z_level, ctx.stock_to_leave, r)
    } else {
        material_remaining_at_level(material_stock, surface_hm, z_level, ctx.stock_to_leave)
    };
    if remaining < 0.005 {
        debug!(
            z = z_level,
            remaining, "CP: skipping — no material remaining"
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
    let edt = crate::contour_extract::distance_transform_2d(&air_grid, rows, cols);

    // 3. Find max distance (determines number of offset levels)
    let max_dist = edt.iter().copied().fold(0.0f64, f64::max);

    // 4. Generate contours at each stepover threshold
    let tool_radius_cells = ctx.tool_radius / cell_size;
    let stepover_cells = ctx.stepover / cell_size;

    debug!(
        z = z_level,
        remaining,
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

    let mut threshold = tool_radius_cells;
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
                let surf_z = surface_hm.surface_z_at_world(p.x, p.y);
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

            // Emit entry (link or rapid) + cut segment
            if let Some(first) = path_3d.first() {
                // Stay-down link if close to previous position, rapid otherwise
                let link_dist = ctx.tool_radius * 3.0;
                let should_link = last_pos.is_some_and(|lp| {
                    let dx = first.x - lp.x;
                    let dy = first.y - lp.y;
                    (dx * dx + dy * dy).sqrt() < link_dist
                });
                if should_link {
                    segments.push(Adaptive3dSegment::Link(*first));
                } else {
                    segments.push(Adaptive3dSegment::Rapid(*first));
                }
                // Stamp dexel stock along the cutting path before moving path_3d
                stamp_along_path(
                    material_stock,
                    ctx.lut,
                    ctx.tool_radius,
                    &path_3d,
                    ctx.step_len,
                );

                *last_pos = path_3d.last().copied();
                segments.push(Adaptive3dSegment::Cut(path_3d));
            }
        }

        threshold += stepover_cells;
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
fn clear_z_level_adaptive(
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
    let remaining = if let Some(r) = region {
        material_remaining_in_region(material_stock, surface_hm, z_level, ctx.stock_to_leave, r)
    } else {
        material_remaining_at_level(material_stock, surface_hm, z_level, ctx.stock_to_leave)
    };
    if remaining < 0.005 {
        return Ok(());
    }

    // ── 1. Build boolean material grid ─────────────────────────────────
    let (material_grid, rows, cols, origin_x, origin_y, cell_size) = build_material_bool_grid(
        material_stock,
        surface_hm,
        z_level,
        ctx.stock_to_leave,
        region,
    );

    if !material_grid.iter().any(|&b| b) {
        return Ok(());
    }

    // ── 2. EDT on inverted grid (distance to nearest air) ──────────────
    let air_grid: Vec<bool> = material_grid.iter().map(|&b| !b).collect();
    let edt = crate::contour_extract::distance_transform_2d(&air_grid, rows, cols);
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
                let surf_z = surface_hm.surface_z_at_world(p.x, p.y);
                let target_z = if surf_z == f64::NEG_INFINITY {
                    z_level
                } else {
                    surf_z + ctx.stock_to_leave
                };
                let z = (z_level + blend * (target_z - z_level)).max(target_z);
                path_3d.push(P3::new(p.x, p.y, z));
            }

            // Entry (link or rapid) + cut segment
            if let Some(first) = path_3d.first() {
                let link_dist = ctx.tool_radius * 3.0;
                let should_link = last_pos.is_some_and(|lp| {
                    let dx = first.x - lp.x;
                    let dy = first.y - lp.y;
                    (dx * dx + dy * dy).sqrt() < link_dist
                });
                if should_link {
                    segments.push(Adaptive3dSegment::Link(*first));
                } else {
                    segments.push(Adaptive3dSegment::Rapid(*first));
                }
                stamp_along_path(
                    material_stock,
                    ctx.lut,
                    ctx.tool_radius,
                    &path_3d,
                    ctx.step_len,
                );

                *last_pos = path_3d.last().copied();
                segments.push(Adaptive3dSegment::Cut(path_3d));
            }
        }

        threshold += base_step_cells;
    }

    Ok(())
}

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// Clear material at a single Z level, optionally restricted to a region.
///
/// When `region` is `Some`, entry point search and material-remaining checks
/// are restricted to the region's bounding box, and direction search bbox
/// is clamped to the region's world extent.
#[allow(clippy::too_many_arguments)]
fn clear_z_level(
    ctx: &ClearZLevelContext<'_>,
    material_stock: &mut TriDexelStock,
    surface_hm: &SurfaceHeightmap,
    z_level: f64,
    segments: &mut Vec<Adaptive3dSegment>,
    last_pos: &mut Option<P3>,
    region: Option<&MaterialRegion>,
    cancel: &dyn CancelCheck,
) -> Result<(), Cancelled> {
    let tool_radius = ctx.tool_radius;

    let scan_bbox = region.map(|r| (r.row_min, r.row_max, r.col_min, r.col_max));

    let dir_x_min = region.map_or(ctx.bbox_x_min, |r| r.world_x_min.max(ctx.bbox_x_min));
    let dir_x_max = region.map_or(ctx.bbox_x_max, |r| r.world_x_max.min(ctx.bbox_x_max));
    let dir_y_min = region.map_or(ctx.bbox_y_min, |r| r.world_y_min.max(ctx.bbox_y_min));
    let dir_y_max = region.map_or(ctx.bbox_y_max, |r| r.world_y_max.min(ctx.bbox_y_max));

    let remaining = if let Some(r) = region {
        material_remaining_in_region(material_stock, surface_hm, z_level, ctx.stock_to_leave, r)
    } else {
        material_remaining_at_level(material_stock, surface_hm, z_level, ctx.stock_to_leave)
    };
    if remaining < 0.005 {
        return Ok(());
    }

    let level_scope = ctx.debug.as_ref().map(|debug_ctx| {
        let label = if let Some(region) = region {
            format!(
                "Z {:.3} region rows {}..{} cols {}..{}",
                z_level, region.row_min, region.row_max, region.col_min, region.col_max
            )
        } else {
            format!("Z {:.3}", z_level)
        };
        debug_ctx.start_span("z_level", label)
    });
    if let Some(scope) = level_scope.as_ref() {
        scope.set_z_level(z_level);
        scope.set_counter("remaining_before", remaining);
    }
    let level_ctx = level_scope.as_ref().map(|scope| scope.context());

    // Pre-stamp thin bands on steep walls to avoid unproductive contour re-tracing.
    let pre_stamp_scope = level_ctx
        .as_ref()
        .map(|debug_ctx| debug_ctx.start_span("pre_stamp", format!("Pre-stamp Z {:.3}", z_level)));
    let pre_stamped = pre_stamp_thin_bands(
        material_stock,
        surface_hm,
        ctx.slope_map,
        z_level,
        ctx.stock_to_leave,
        ctx.depth_per_pass,
        region,
    );
    if let Some(scope) = pre_stamp_scope.as_ref() {
        scope.set_z_level(z_level);
        scope.set_counter("cells", pre_stamped as f64);
    }
    if pre_stamped > 0 {
        debug!(
            cells = pre_stamped,
            z = z_level,
            "Pre-stamped thin wall bands"
        );
        // Re-check remaining after pre-stamp — skip level if negligible
        let remaining_after = if let Some(r) = region {
            material_remaining_in_region(material_stock, surface_hm, z_level, ctx.stock_to_leave, r)
        } else {
            material_remaining_at_level(material_stock, surface_hm, z_level, ctx.stock_to_leave)
        };
        if let Some(scope) = level_scope.as_ref() {
            scope.set_counter("remaining_after_prestamp", remaining_after);
        }
        if remaining_after < 0.005 {
            debug!(
                z = z_level,
                "Skipping Z level — thin bands consumed all remaining material"
            );
            if let Some(scope) = level_scope.as_ref() {
                scope.set_exit_reason("pre-stamp exhausted");
            }
            return Ok(());
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    let t_level = Instant::now();
    let mut pass_endpoints: Vec<P2> = Vec::new();
    let max_passes = 500;
    let mut pass_count = 0;
    let mut short_pass_streak = 0u32;
    let min_productive_steps = 8;
    let warmup_passes = 50;
    let mut total_steps = 0u64;
    let mut long_passes = 0u32;
    let mut short_passes = 0u32;
    let mut skipped_preflight = 0u32;

    loop {
        check_cancel(cancel)?;
        pass_count += 1;
        if pass_count > max_passes {
            break;
        }
        if pass_count > warmup_passes && short_pass_streak > 8 {
            debug!(
                short_passes = short_pass_streak,
                z = z_level,
                pass = pass_count,
                "Bailing from Z level"
            );
            break;
        }
        if pass_count % 20 == 1 {
            let rem = if let Some(r) = region {
                material_remaining_in_region(
                    material_stock,
                    surface_hm,
                    z_level,
                    ctx.stock_to_leave,
                    r,
                )
            } else {
                material_remaining_at_level(material_stock, surface_hm, z_level, ctx.stock_to_leave)
            };
            if rem < 0.01 {
                break;
            }
        }

        let last_2d = last_pos.map(|p| P2::new(p.x, p.y));
        let pass_started = Instant::now();
        let pass_scope = level_ctx
            .as_ref()
            .map(|debug_ctx| debug_ctx.start_span("adaptive_pass", format!("Pass {pass_count}")));
        if let Some(scope) = pass_scope.as_ref() {
            scope.set_z_level(z_level);
        }
        let pass_ctx = pass_scope.as_ref().map(|scope| scope.context());
        let entry_scope = pass_ctx
            .as_ref()
            .map(|debug_ctx| debug_ctx.start_span("entry_search", format!("Entry {pass_count}")));
        let Some((entry_xy, entry_z)) = find_entry_3d(
            material_stock,
            surface_hm,
            ctx.mesh,
            ctx.index,
            ctx.cutter,
            z_level,
            ctx.stock_to_leave,
            last_2d,
            &pass_endpoints,
            tool_radius,
            scan_bbox,
        ) else {
            if let Some(scope) = pass_scope.as_ref() {
                scope.set_exit_reason("no entry");
            }
            break;
        };
        if let Some(scope) = entry_scope.as_ref() {
            scope.set_xy_bbox(ToolpathDebugBounds2 {
                min_x: entry_xy.x,
                max_x: entry_xy.x,
                min_y: entry_xy.y,
                max_y: entry_xy.y,
            });
            scope.set_z_level(entry_z);
        }

        let entry_3d = P3::new(entry_xy.x, entry_xy.y, entry_z);

        let preflight_scope = pass_ctx
            .as_ref()
            .map(|debug_ctx| debug_ctx.start_span("preflight", format!("Preflight {pass_count}")));
        let preflight_dir = search_direction_3d_with_metrics(
            material_stock,
            surface_hm,
            entry_xy.x,
            entry_xy.y,
            tool_radius,
            ctx.step_len,
            ctx.target_frac,
            0.0,
            z_level,
            ctx.stock_to_leave,
            dir_x_min,
            dir_x_max,
            dir_y_min,
            dir_y_max,
        );
        if let Some(scope) = preflight_scope.as_ref() {
            scope.set_z_level(z_level);
            scope.set_counter(
                "evaluations",
                preflight_dir
                    .as_ref()
                    .map_or(0.0, |result| result.evaluations as f64),
            );
            if preflight_dir.is_none() {
                scope.set_exit_reason("no viable direction");
            }
        }
        if preflight_dir.is_none() {
            material_stock.stamp_tool_at(
                ctx.lut,
                ctx.tool_radius,
                entry_xy.x,
                entry_xy.y,
                entry_z,
                StockCutDirection::FromTop,
            );
            for a in 0..8 {
                let angle = (a as f64 / 8.0) * TAU;
                let (sin_a, cos_a) = angle.sin_cos();
                let px = entry_xy.x + tool_radius * 0.5 * cos_a;
                let py = entry_xy.y + tool_radius * 0.5 * sin_a;
                material_stock.stamp_tool_at(
                    ctx.lut,
                    ctx.tool_radius,
                    px,
                    py,
                    entry_z,
                    StockCutDirection::FromTop,
                );
            }
            pass_endpoints.push(entry_xy);
            short_pass_streak += 1;
            skipped_preflight += 1;
            segments.push(Adaptive3dSegment::Marker(
                Adaptive3dRuntimeEvent::PassPreflightSkip {
                    pass_index: pass_count,
                },
            ));
            if let Some(scope) = pass_scope.as_ref() {
                scope.set_exit_reason("preflight skip");
                scope.set_counter("skipped_preflight", 1.0);
            }
            continue;
        }

        segments.push(Adaptive3dSegment::Marker(
            Adaptive3dRuntimeEvent::PassEntry {
                pass_index: pass_count,
                entry_x: entry_xy.x,
                entry_y: entry_xy.y,
                entry_z,
            },
        ));

        if let Some(last) = *last_pos {
            let dx = entry_3d.x - last.x;
            let dy = entry_3d.y - last.y;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist < ctx.max_link_dist
                && is_clear_path_3d(
                    material_stock,
                    surface_hm,
                    last,
                    entry_3d,
                    z_level,
                    ctx.stock_to_leave,
                )
            {
                segments.push(Adaptive3dSegment::Link(entry_3d));
            } else {
                segments.push(Adaptive3dSegment::Rapid(entry_3d));
            }
        } else {
            segments.push(Adaptive3dSegment::Rapid(entry_3d));
        }

        let mut path = vec![entry_3d];
        let mut cx = entry_xy.x;
        let mut cy = entry_xy.y;
        let mut cz = entry_z;

        let mut prev_angle = if let Some(last) = *last_pos {
            (entry_xy.y - last.y).atan2(entry_xy.x - last.x)
        } else {
            0.0
        };

        material_stock.stamp_tool_at(
            ctx.lut,
            ctx.tool_radius,
            cx,
            cy,
            cz,
            StockCutDirection::FromTop,
        );

        const SMOOTH_BUF_LEN: usize = 3;
        let mut angle_buf: Vec<f64> = Vec::with_capacity(SMOOTH_BUF_LEN);

        let max_steps = 5000;
        let mut idle_count = 0;
        let mut step_count = 0u32;
        let mut looped = false;
        let mut pass_removal_sum = 0.0f64;
        let mut search_evaluations = preflight_dir
            .as_ref()
            .map_or(0u32, |result| result.evaluations);

        // Loop detection: after enough steps to form a meaningful loop,
        // check if we've returned near the entry point. The minimum step
        // threshold avoids false positives at the start of a pass.
        let loop_min_steps = (tool_radius * 4.0 / ctx.step_len).ceil() as u32;
        let loop_close_dist_sq = (tool_radius * 1.5) * (tool_radius * 1.5);
        // Track the farthest we've been from entry — only trigger loop
        // detection once we've actually moved away first.
        let mut max_dist_from_entry_sq = 0.0f64;
        let min_excursion_sq = (tool_radius * 4.0) * (tool_radius * 4.0);

        for _ in 0..max_steps {
            check_cancel(cancel)?;
            let local_before = local_material_sum(material_stock, cx, cy, tool_radius);

            let smoothed = if angle_buf.len() >= 2 {
                average_angles(&angle_buf)
            } else {
                prev_angle
            };

            let Some(search_result) = search_direction_3d_with_metrics(
                material_stock,
                surface_hm,
                cx,
                cy,
                tool_radius,
                ctx.step_len,
                ctx.target_frac,
                smoothed,
                z_level,
                ctx.stock_to_leave,
                dir_x_min,
                dir_x_max,
                dir_y_min,
                dir_y_max,
            ) else {
                break;
            };
            search_evaluations += search_result.evaluations;
            let angle = search_result.angle;
            let z_next = search_result.z_next;

            let (sin_a, cos_a) = angle.sin_cos();
            cx += ctx.step_len * cos_a;
            cy += ctx.step_len * sin_a;
            let max_z_step = ctx.depth_per_pass;
            cz = z_next.max(cz - max_z_step);
            path.push(P3::new(cx, cy, cz));

            material_stock.stamp_tool_at(
                ctx.lut,
                ctx.tool_radius,
                cx,
                cy,
                cz,
                StockCutDirection::FromTop,
            );

            if angle_buf.len() >= SMOOTH_BUF_LEN {
                angle_buf.remove(0);
            }
            angle_buf.push(angle);
            step_count += 1;

            // Loop detection: have we returned near the entry after travelling far enough?
            let dx_entry = cx - entry_xy.x;
            let dy_entry = cy - entry_xy.y;
            let dist_from_entry_sq = dx_entry * dx_entry + dy_entry * dy_entry;
            if dist_from_entry_sq > max_dist_from_entry_sq {
                max_dist_from_entry_sq = dist_from_entry_sq;
            }
            if step_count > loop_min_steps
                && max_dist_from_entry_sq > min_excursion_sq
                && dist_from_entry_sq < loop_close_dist_sq
            {
                looped = true;
                break;
            }

            let local_after = local_material_sum(material_stock, cx, cy, tool_radius);
            let local_delta = (local_before - local_after).abs();
            pass_removal_sum += local_delta;
            let engagement_here = compute_engagement_3d(
                material_stock,
                surface_hm,
                cx,
                cy,
                tool_radius,
                z_level,
                ctx.stock_to_leave,
            );
            if local_delta < 0.001 && engagement_here < 0.05 {
                idle_count += 1;
                if idle_count > 20 {
                    break;
                }
            } else {
                idle_count = 0;
            }

            prev_angle = angle;
        }

        let was_idle = idle_count > 20;

        let pass_steps = path.len();
        // Keep a reference to the path for post-pass widening before moving it
        let should_widen = (looped || pass_steps >= min_productive_steps) && pass_steps >= 4;
        let widen_path: Vec<P3> = if should_widen {
            // Denser sampling to avoid missing tight contours
            let skip = 1.max(path.len() / 500);
            path.iter().step_by(skip).copied().collect()
        } else {
            Vec::new()
        };

        let path_debug_bounds = path_bounds_3d(&path);

        if pass_steps >= 2 {
            // SAFETY: pass_steps >= 2 checked on line above
            #[allow(clippy::expect_used)]
            let endpoint = *path.last().expect("path is non-empty after loop");
            *last_pos = Some(endpoint);
            pass_endpoints.push(P2::new(endpoint.x, endpoint.y));
            segments.push(Adaptive3dSegment::Cut(path));
        } else {
            *last_pos = Some(entry_3d);
            pass_endpoints.push(entry_xy);
        }

        total_steps += pass_steps as u64;
        let exit_reason = if looped {
            "loop closed"
        } else if was_idle {
            "idle"
        } else {
            "no material"
        };

        // Low-yield detection: bail on passes that trace lots of steps but remove
        // negligible material (typical of thin wall contour re-tracing).
        let yield_ratio = if pass_steps > 1 {
            let expected = pass_steps as f64
                * ctx.stepover
                * ctx.depth_per_pass
                * material_stock.z_grid.cell_size;
            if expected > 0.0 {
                pass_removal_sum / expected
            } else {
                1.0
            }
        } else {
            1.0
        };
        let is_low_yield = pass_steps < min_productive_steps || yield_ratio < 0.05;
        if let Some(scope) = pass_scope.as_ref() {
            scope.set_counter("step_count", pass_steps as f64);
            scope.set_counter("idle_count", idle_count as f64);
            scope.set_counter("search_evaluations", search_evaluations as f64);
            scope.set_counter("yield_ratio", yield_ratio);
            scope.set_counter("preflight_skipped", 0.0);
            scope.set_exit_reason(exit_reason);
            if let Some(bounds) = path_debug_bounds {
                scope.set_xy_bbox(bounds);
                let (center_x, center_y) = bounds.center();
                if let Some(debug_ctx) = pass_ctx.as_ref() {
                    debug_ctx.record_hotspot(&HotspotRecord {
                        kind: "adaptive3d_pass".into(),
                        center_x,
                        center_y,
                        z_level: Some(z_level),
                        bucket_size_xy: tool_radius * 2.0,
                        bucket_size_z: Some(ctx.tolerance.max(ctx.depth_per_pass * 0.5)),
                        elapsed_us: pass_started.elapsed().as_micros() as u64,
                        pass_count: 1,
                        step_count: pass_steps as u64,
                        low_yield_exit_count: u32::from(is_low_yield),
                    });
                }
            }
        }

        if is_low_yield {
            short_passes += 1;
            short_pass_streak += 1;
            segments.push(Adaptive3dSegment::Marker(
                Adaptive3dRuntimeEvent::PassSummary {
                    pass_index: pass_count,
                    step_count: pass_steps,
                    exit_reason: exit_reason.to_owned(),
                    yield_ratio,
                    short: true,
                },
            ));
        } else {
            long_passes += 1;
            short_pass_streak = 0;
            segments.push(Adaptive3dSegment::Marker(
                Adaptive3dRuntimeEvent::PassSummary {
                    pass_index: pass_count,
                    step_count: pass_steps,
                    exit_reason: exit_reason.to_owned(),
                    yield_ratio,
                    short: false,
                },
            ));
        }

        if was_idle {
            material_stock.stamp_tool_at(
                ctx.lut,
                ctx.tool_radius,
                cx,
                cy,
                cz,
                StockCutDirection::FromTop,
            );
            for a in 0..8 {
                let angle = (a as f64 / 8.0) * TAU;
                let (sin_a, cos_a) = angle.sin_cos();
                let px = cx + tool_radius * cos_a;
                let py = cy + tool_radius * sin_a;
                let surf_z = surface_hm.surface_z_at_world(px, py);
                let pz = if surf_z == f64::NEG_INFINITY {
                    cz
                } else {
                    (surf_z + ctx.stock_to_leave).max(z_level)
                };
                material_stock.stamp_tool_at(
                    ctx.lut,
                    ctx.tool_radius,
                    px,
                    py,
                    pz,
                    StockCutDirection::FromTop,
                );
            }
        }

        // Widen the cleared band after loop-close or long contour passes.
        // Stamp perpendicular offsets at 1× and 2× stepover distance (double ring)
        // so adjacent parallel contours are also marked as cleared.
        if !widen_path.is_empty() {
            let widen_scope = pass_ctx
                .as_ref()
                .map(|debug_ctx| debug_ctx.start_span("widen_band", format!("Widen {pass_count}")));
            let widen_offset = ctx.stepover;
            for i in 1..widen_path.len() {
                let prev = &widen_path[i - 1];
                let curr = &widen_path[i];
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
                        let px = curr.x + sign * mult * widen_offset * nx;
                        let py = curr.y + sign * mult * widen_offset * ny;
                        let sz = surface_hm.surface_z_at_world(px, py);
                        if sz != f64::NEG_INFINITY {
                            let pz = (sz + ctx.stock_to_leave).max(z_level);
                            material_stock.stamp_tool_at(
                                ctx.lut,
                                ctx.tool_radius,
                                px,
                                py,
                                pz,
                                StockCutDirection::FromTop,
                            );
                        }
                    }
                }
            }
            if let Some(scope) = widen_scope.as_ref() {
                scope.set_z_level(z_level);
                scope.set_counter("sample_points", widen_path.len() as f64);
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    let level_ms = t_level.elapsed().as_millis() as u64;
    #[cfg(target_arch = "wasm32")]
    let level_ms = 0u64;
    debug!(
        passes = pass_count,
        long = long_passes,
        short = short_passes,
        skipped = skipped_preflight,
        total_steps = total_steps,
        z = z_level,
        elapsed_ms = level_ms,
        "Completed Z level"
    );
    if let Some(scope) = level_scope.as_ref() {
        scope.set_counter("passes", pass_count as f64);
        scope.set_counter("long_passes", long_passes as f64);
        scope.set_counter("short_passes", short_passes as f64);
        scope.set_counter("skipped_preflight", skipped_preflight as f64);
        scope.set_counter("total_steps", total_steps as f64);
        scope.set_z_level(z_level);
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
fn waterline_cleanup(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    lut: &RadialProfileLUT,
    slope_map: &SlopeMap,
    material_stock: &mut TriDexelStock,
    z_level: f64,
    tool_radius: f64,
    cell_size: f64,
    segments: &mut Vec<Adaptive3dSegment>,
    last_pos: &mut Option<P3>,
    debug_ctx: Option<&ToolpathDebugContext>,
    cancel: &dyn CancelCheck,
) -> Result<(), Cancelled> {
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

        segments.push(Adaptive3dSegment::Rapid(contour[0]));

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
                material_stock.stamp_tool_at(lut, tool_radius, x, y, z, StockCutDirection::FromTop);
                cleanup_path.push(P3::new(x, y, z));
            }
        }
        cleanup_path.push(contour[0]);
        material_stock.stamp_tool_at(
            lut,
            tool_radius,
            contour[0].x,
            contour[0].y,
            contour[0].z,
            StockCutDirection::FromTop,
        );
        segments.push(Adaptive3dSegment::Cut(cleanup_path));
        *last_pos = Some(contour[0]);
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

// ── Main loop ─────────────────────────────────────────────────────────

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
fn adaptive_3d_segments(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &Adaptive3dParams,
    debug_ctx: Option<&ToolpathDebugContext>,
    cancel: &dyn CancelCheck,
) -> Result<Vec<Adaptive3dSegment>, Cancelled> {
    let tool_radius = params.tool_radius;
    let r = cutter.radius();

    // Grid geometry: expand mesh bbox by cutter radius
    let bbox = &mesh.bbox;
    let origin_x = bbox.min.x - r;
    let origin_y = bbox.min.y - r;
    let extent_x = bbox.max.x + r;
    let extent_y = bbox.max.y + r;
    let cell_size = (tool_radius / 6.0).max(params.tolerance);

    // Initialize tri-dexel material stock
    let mut material_stock = match &params.initial_stock {
        Some(stock) => stock.clone(),
        None => TriDexelStock::from_stock(
            origin_x,
            origin_y,
            extent_x,
            extent_y,
            bbox.min.z,
            params.stock_top_z,
            cell_size,
        ),
    };

    // Precompute surface heightmap (rayon parallel drop-cutter)
    #[cfg(not(target_arch = "wasm32"))]
    let t_surface = Instant::now();
    let surface_scope =
        debug_ctx.map(|ctx| ctx.start_span("surface_heightmap", "Surface heightmap"));
    debug!(
        cols = material_stock.z_grid.cols,
        rows = material_stock.z_grid.rows,
        "Precomputing surface heightmap"
    );
    let surface_hm = SurfaceHeightmap::from_mesh_with_cancel(
        mesh,
        index,
        cutter,
        material_stock.z_grid.origin_u,
        material_stock.z_grid.origin_v,
        material_stock.z_grid.rows,
        material_stock.z_grid.cols,
        material_stock.z_grid.cell_size,
        bbox.min.z,
        cancel,
    )?;
    #[cfg(not(target_arch = "wasm32"))]
    info!(
        elapsed_ms = t_surface.elapsed().as_millis() as u64,
        "Surface heightmap complete"
    );
    #[cfg(target_arch = "wasm32")]
    info!("Surface heightmap complete");
    if let Some(scope) = surface_scope.as_ref() {
        scope.set_counter("rows", material_stock.z_grid.rows as f64);
        scope.set_counter("cols", material_stock.z_grid.cols as f64);
    }

    // Compute slope map for slope-aware pre-stamping and selective waterline cleanup.
    let slope_map = surface_hm.slope_map();

    // Clear material at cells outside the mesh XY footprint.
    // Drop-cutter returns min_z for cells beyond the mesh edge, creating phantom
    // "deep material" that the tool can never reach. Mark these as already cleared
    // so the adaptive doesn't waste passes trying to cut in empty space.
    // Only clear cells whose XY center is outside the mesh bbox (with tolerance).
    let border_scope = debug_ctx.map(|ctx| ctx.start_span("border_clear", "Border clear"));
    let border_margin = r * 0.5;
    let mut border_cleared = 0u32;
    for row in 0..material_stock.z_grid.rows {
        if row % 16 == 0 {
            check_cancel(cancel)?;
        }
        for col in 0..material_stock.z_grid.cols {
            let (x, y) = material_stock.z_grid.cell_to_world(row, col);
            if x < bbox.min.x - border_margin
                || x > bbox.max.x + border_margin
                || y < bbox.min.y - border_margin
                || y > bbox.max.y + border_margin
            {
                // Clear material above the surface Z at this cell
                let i = row * material_stock.z_grid.cols + col;
                let clear_z = surface_hm.z_values[i] as f32;
                ray_subtract_above(material_stock.z_grid.ray_mut(row, col), clear_z);
                border_cleared += 1;
            }
        }
    }
    if border_cleared > 0 {
        debug!(
            cells = border_cleared,
            "Cleared border cells outside mesh footprint"
        );
    }
    if let Some(scope) = border_scope.as_ref() {
        scope.set_counter("cells", border_cleared as f64);
    }

    // Compute Z levels: stock_top down to surface bottom + stock_to_leave
    let z_plan_scope = debug_ctx.map(|ctx| ctx.start_span("z_level_plan", "Compute Z levels"));
    let surface_bottom = surface_hm.min_z();
    let z_bottom = surface_bottom + params.stock_to_leave;
    let mut z_levels = Vec::new();
    let mut z = params.stock_top_z - params.depth_per_pass;
    while z > z_bottom {
        z_levels.push(z);
        z -= params.depth_per_pass;
    }
    z_levels.push(z_bottom); // Always include final level at the surface

    // Fix 5: Flat area detection — histogram surface Z, insert levels at shelves
    if params.detect_flat_areas {
        let total_cells = surface_hm.z_values.len();
        if total_cells > 0 {
            // Build histogram of surface Z values binned at tolerance resolution
            let bin_size = params.tolerance.max(0.05);
            let z_min_surf = surface_bottom;
            let z_max_surf = params.stock_top_z;
            let n_bins = ((z_max_surf - z_min_surf) / bin_size).ceil() as usize + 1;
            let mut histogram = vec![0u32; n_bins];
            for &sz in &surface_hm.z_values {
                let bin = ((sz - z_min_surf) / bin_size).floor() as usize;
                if bin < n_bins {
                    histogram[bin] += 1;
                }
            }
            // Bins with >2% of total cells represent flat features
            let threshold = (total_cells as f64 * 0.02) as u32;
            let mut flat_levels = Vec::new();
            for (i, &count) in histogram.iter().enumerate() {
                if count > threshold {
                    let flat_z = z_min_surf + (i as f64 + 0.5) * bin_size + params.stock_to_leave;
                    // Only insert if within the working range and not too close to existing levels
                    if flat_z > z_bottom + bin_size && flat_z < params.stock_top_z - bin_size {
                        let too_close = z_levels.iter().any(|&zl| (zl - flat_z).abs() < bin_size);
                        if !too_close {
                            flat_levels.push(flat_z);
                        }
                    }
                }
            }
            if !flat_levels.is_empty() {
                debug!(count = flat_levels.len(), "Detected flat area Z levels");
                z_levels.extend(flat_levels);
                z_levels.sort_by(|a, b| b.total_cmp(a)); // Top-down order
                z_levels.dedup_by(|a, b| (*a - *b).abs() < 0.01);
            }
        }
    }

    // Fix 4: Fine stepdown — insert intermediate Z levels between major levels
    if let Some(fine_step) = params.fine_stepdown
        && fine_step > 0.0
        && fine_step < params.depth_per_pass
    {
        let major_levels = z_levels.clone();
        let mut all_levels = Vec::new();
        // Insert intermediates between stock_top and first level
        let first_start = params.stock_top_z;
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
            all_levels.push(z_bot); // Always include the major level
        }
        all_levels.sort_by(|a, b| b.total_cmp(a));
        all_levels.dedup_by(|a, b| (*a - *b).abs() < 0.01);
        debug!(
            from = z_levels.len(),
            to = all_levels.len(),
            fine_step = fine_step,
            "Fine stepdown expanded Z levels"
        );
        z_levels = all_levels;
    }

    info!(
        count = z_levels.len(),
        z_top = z_levels.first().copied().unwrap_or(0.0),
        z_bottom = z_levels.last().copied().unwrap_or(0.0),
        depth_per_pass = params.depth_per_pass,
        "Z levels computed"
    );
    if let Some(scope) = z_plan_scope.as_ref() {
        scope.set_counter("count", z_levels.len() as f64);
        scope.set_counter("z_top", z_levels.first().copied().unwrap_or(0.0));
        scope.set_counter("z_bottom", z_levels.last().copied().unwrap_or(0.0));
    }

    let target_frac = target_engagement_fraction(params.stepover, tool_radius);
    let step_len = cell_size * 1.5;
    let max_link_dist = params.max_stay_down_dist.unwrap_or(tool_radius * 6.0);

    let bbox_x_min = origin_x + tool_radius;
    let bbox_x_max = extent_x - tool_radius;
    let bbox_y_min = origin_y + tool_radius;
    let bbox_y_max = extent_y - tool_radius;

    let lut = RadialProfileLUT::from_cutter(cutter, 256);
    let ctx = ClearZLevelContext {
        mesh,
        index,
        cutter,
        lut: &lut,
        slope_map: &slope_map,
        debug: debug_ctx.cloned(),
        tool_radius,
        stepover: params.stepover,
        stock_to_leave: params.stock_to_leave,
        depth_per_pass: params.depth_per_pass,
        tolerance: params.tolerance,
        target_frac,
        step_len,
        max_link_dist,
        bbox_x_min,
        bbox_x_max,
        bbox_y_min,
        bbox_y_max,
        clearing_strategy: params.clearing_strategy,
        z_blend: params.z_blend,
    };

    let mut segments = Vec::new();
    let mut last_pos: Option<P3> = None;

    match params.region_ordering {
        RegionOrdering::ByArea => {
            let region_scope =
                debug_ctx.map(|ctx| ctx.start_span("region_detect", "Detect regions"));
            let regions = detect_material_regions(
                &material_stock,
                &surface_hm,
                params.stock_to_leave,
                tool_radius,
            );
            info!(
                regions = regions.len(),
                "Detected material regions for by-area ordering"
            );
            if let Some(scope) = region_scope.as_ref() {
                scope.set_counter("regions", regions.len() as f64);
            }

            for (region_idx, region) in regions.iter().enumerate() {
                check_cancel(cancel)?;
                debug!(
                    region = region_idx,
                    cells = region.cell_count,
                    z_min = format!("{:.1}", region.surface_z_min),
                    z_max = format!("{:.1}", region.surface_z_max),
                    "Processing region"
                );
                segments.push(Adaptive3dSegment::Marker(
                    Adaptive3dRuntimeEvent::RegionStart {
                        region_index: region_idx + 1,
                        region_total: regions.len(),
                        cell_count: region.cell_count,
                    },
                ));

                let region_z_levels: Vec<f64> = z_levels
                    .iter()
                    .copied()
                    .filter(|&z| z >= region.surface_z_min + params.stock_to_leave - 0.01)
                    .collect();

                for (li, &z_level) in region_z_levels.iter().enumerate() {
                    check_cancel(cancel)?;
                    segments.push(Adaptive3dSegment::Marker(
                        Adaptive3dRuntimeEvent::RegionZLevel {
                            region_index: region_idx + 1,
                            z_level,
                            level_index: li + 1,
                            level_total: region_z_levels.len(),
                        },
                    ));
                    match ctx.clearing_strategy {
                        ClearingStrategy3d::ContourParallel => {
                            clear_z_level_contour_parallel(
                                &ctx,
                                &mut material_stock,
                                &surface_hm,
                                z_level,
                                &mut segments,
                                &mut last_pos,
                                Some(region),
                                cancel,
                            )?;
                        }
                        ClearingStrategy3d::Adaptive => {
                            clear_z_level_adaptive(
                                &ctx,
                                &mut material_stock,
                                &surface_hm,
                                z_level,
                                &mut segments,
                                &mut last_pos,
                                Some(region),
                                cancel,
                            )?;
                        }
                        ClearingStrategy3d::AgentSearch => {
                            clear_z_level(
                                &ctx,
                                &mut material_stock,
                                &surface_hm,
                                z_level,
                                &mut segments,
                                &mut last_pos,
                                Some(region),
                                cancel,
                            )?;
                        }
                    }
                }
            }

            // Waterline cleanup once at bottom Z
            if let Some(&z_bottom_level) = z_levels.last() {
                segments.push(Adaptive3dSegment::Marker(
                    Adaptive3dRuntimeEvent::WaterlineCleanup,
                ));
                waterline_cleanup(
                    mesh,
                    index,
                    cutter,
                    &lut,
                    &slope_map,
                    &mut material_stock,
                    z_bottom_level,
                    tool_radius,
                    cell_size,
                    &mut segments,
                    &mut last_pos,
                    debug_ctx,
                    cancel,
                )?;
            }
        }
        RegionOrdering::Global => {
            for (level_idx, &z_level) in z_levels.iter().enumerate() {
                check_cancel(cancel)?;
                segments.push(Adaptive3dSegment::Marker(
                    Adaptive3dRuntimeEvent::GlobalZLevel {
                        z_level,
                        level_index: level_idx + 1,
                        level_total: z_levels.len(),
                    },
                ));
                match ctx.clearing_strategy {
                    ClearingStrategy3d::ContourParallel => {
                        clear_z_level_contour_parallel(
                            &ctx,
                            &mut material_stock,
                            &surface_hm,
                            z_level,
                            &mut segments,
                            &mut last_pos,
                            None,
                            cancel,
                        )?;
                    }
                    ClearingStrategy3d::Adaptive => {
                        clear_z_level_adaptive(
                            &ctx,
                            &mut material_stock,
                            &surface_hm,
                            z_level,
                            &mut segments,
                            &mut last_pos,
                            None,
                            cancel,
                        )?;
                    }
                    ClearingStrategy3d::AgentSearch => {
                        clear_z_level(
                            &ctx,
                            &mut material_stock,
                            &surface_hm,
                            z_level,
                            &mut segments,
                            &mut last_pos,
                            None,
                            cancel,
                        )?;
                    }
                }

                let is_last_level = level_idx == z_levels.len() - 1;
                if is_last_level {
                    segments.push(Adaptive3dSegment::Marker(
                        Adaptive3dRuntimeEvent::WaterlineCleanup,
                    ));
                    waterline_cleanup(
                        mesh,
                        index,
                        cutter,
                        &lut,
                        &slope_map,
                        &mut material_stock,
                        z_level,
                        tool_radius,
                        cell_size,
                        &mut segments,
                        &mut last_pos,
                        debug_ctx,
                        cancel,
                    )?;
                }
            }
        }
    }

    Ok(segments)
}

// ── Public API ────────────────────────────────────────────────────────

/// Convert segments to a toolpath and collect annotations.
fn segments_to_toolpath(
    segments: &[Adaptive3dSegment],
    params: &Adaptive3dParams,
) -> (Toolpath, Vec<Adaptive3dRuntimeAnnotation>) {
    let mut tp = Toolpath::new();
    let mut annotations = Vec::new();

    for segment in segments {
        match segment {
            Adaptive3dSegment::Marker(event) => {
                annotations.push(Adaptive3dRuntimeAnnotation {
                    move_index: tp.moves.len(),
                    event: event.clone(),
                });
            }
            Adaptive3dSegment::Rapid(entry) => match params.entry_style {
                EntryStyle3d::Plunge => {
                    tp.rapid_to(P3::new(entry.x, entry.y, params.safe_z));
                    tp.feed_to(*entry, params.plunge_rate);
                }
                EntryStyle3d::Helix { radius, pitch } => {
                    tp.rapid_to(P3::new(entry.x, entry.y, params.safe_z));
                    let helix_start = P3::new(entry.x, entry.y, params.safe_z);
                    crate::dressup::emit_helix(
                        &mut tp,
                        &helix_start,
                        entry,
                        radius,
                        pitch,
                        params.plunge_rate,
                    );
                }
                EntryStyle3d::Ramp { max_angle_deg } => {
                    tp.rapid_to(P3::new(entry.x, entry.y, params.safe_z));
                    let ramp_start = P3::new(entry.x, entry.y, params.safe_z);
                    crate::dressup::emit_ramp(
                        &mut tp,
                        &ramp_start,
                        entry,
                        (1.0, 0.0),
                        max_angle_deg,
                        params.plunge_rate,
                    );
                }
            },
            Adaptive3dSegment::Link(target) => {
                tp.feed_to(*target, params.feed_rate);
            }
            Adaptive3dSegment::Cut(path) => {
                if path.len() < 2 {
                    continue;
                }
                let simplified = simplify_path_3d(path, params.tolerance);
                let blended = blend_corners_3d(&simplified, params.min_cutting_radius);
                for pt in blended.iter().skip(1) {
                    tp.feed_to(*pt, params.feed_rate);
                }
            }
        }
    }

    if let Some(last) = tp.moves.last() {
        tp.rapid_to(P3::new(last.target.x, last.target.y, params.safe_z));
    }

    (tp, annotations)
}

fn runtime_annotations_to_labels(
    annotations: &[Adaptive3dRuntimeAnnotation],
) -> Vec<(usize, String)> {
    annotations
        .iter()
        .map(|annotation| (annotation.move_index, annotation.event.label()))
        .collect()
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
    let segments = adaptive_3d_segments(mesh, index, cutter, params, debug, cancel)?;
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
    use super::*;
    use crate::dexel::DexelSegment;
    use crate::mesh::SpatialIndex;
    use crate::tool::FlatEndmill;

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
            clearing_strategy: ClearingStrategy3d::AgentSearch,
            z_blend: false,
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

    // ── Engagement tests ──────────────────────────────────────────────

    #[test]
    fn test_engagement_3d_full_material() {
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

        // Stock at 20, surface near 0, z_level=10: everything above 10
        let eng = compute_engagement_3d(&material_stock, &surface_hm, 0.0, 0.0, 3.175, 10.0, 0.5);
        assert!(
            eng > 0.9,
            "Full material should give high engagement, got {:.2}",
            eng
        );
    }

    #[test]
    fn test_engagement_3d_cleared() {
        let (mesh, si) = make_flat_mesh();
        let cutter = flat_cutter();
        let cell_size = 1.0;

        // Stock already at surface level
        let material_stock = make_stock(-30.0, -30.0, 30.0, 30.0, 0.5, cell_size);
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

        let eng = compute_engagement_3d(&material_stock, &surface_hm, 0.0, 0.0, 3.175, 0.0, 0.5);
        assert!(
            eng < 0.1,
            "Cleared material should give low engagement, got {:.2}",
            eng
        );
    }

    #[test]
    fn test_engagement_3d_partial() {
        let (mesh, si) = make_flat_mesh();
        let cutter = flat_cutter();
        let cell_size = 1.0;

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

        // Stamp tool at (3, 0) to clear half the area near (0, 0)
        let lut = RadialProfileLUT::from_cutter(&cutter, 256);
        material_stock.stamp_tool_at(
            &lut,
            cutter.radius(),
            3.0,
            0.0,
            10.0,
            StockCutDirection::FromTop,
        );

        let eng = compute_engagement_3d(&material_stock, &surface_hm, 0.0, 0.0, 3.175, 10.0, 0.5);
        assert!(
            eng > 0.2 && eng < 0.9,
            "Partial material should give mid engagement, got {:.2}",
            eng
        );
    }

    // ── Direction search tests ─────────────────────────────────────────

    #[test]
    fn test_direction_search_finds_material() {
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

        let target = target_engagement_fraction(2.0, 3.175);
        let result = search_direction_3d(
            &material_stock,
            &surface_hm,
            0.0,
            0.0,
            3.175,
            1.5,
            target,
            0.0,
            10.0,
            0.5,
            -25.0,
            25.0,
            -25.0,
            25.0,
        );
        assert!(
            result.is_some(),
            "Should find a direction with full material"
        );
    }

    #[test]
    fn test_direction_search_no_material() {
        let (mesh, si) = make_flat_mesh();
        let cutter = flat_cutter();
        let cell_size = 1.0;

        // Stock at surface level — nothing to cut
        let material_stock = make_stock(-30.0, -30.0, 30.0, 30.0, 0.5, cell_size);
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

        let target = target_engagement_fraction(2.0, 3.175);
        let result = search_direction_3d(
            &material_stock,
            &surface_hm,
            0.0,
            0.0,
            3.175,
            1.5,
            target,
            0.0,
            0.0,
            0.5,
            -25.0,
            25.0,
            -25.0,
            25.0,
        );
        assert!(
            result.is_none(),
            "Should find no direction when all cleared"
        );
    }

    // ── Entry point tests ───────────────────────────────────────────────

    #[test]
    fn test_entry_3d_finds_remaining() {
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

        let result = find_entry_3d(
            &material_stock,
            &surface_hm,
            &mesh,
            &si,
            &cutter,
            10.0,
            0.5,
            None,
            &[],
            3.175,
            None,
        );
        assert!(result.is_some(), "Should find entry with full material");
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
            .expect("test helper should not cancel");

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

    #[test]
    fn test_find_entry_3d_with_bbox() {
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

        // Restrict scan to a small bbox in the top-right quadrant
        let half_rows = material_stock.z_grid.rows / 2;
        let half_cols = material_stock.z_grid.cols / 2;
        let scan_bbox = Some((
            half_rows,
            material_stock.z_grid.rows - 1,
            half_cols,
            material_stock.z_grid.cols - 1,
        ));

        let result = find_entry_3d(
            &material_stock,
            &surface_hm,
            &mesh,
            &si,
            &cutter,
            10.0,
            0.5,
            None,
            &[],
            3.175,
            scan_bbox,
        );
        assert!(result.is_some(), "Should find entry within bbox constraint");

        // Verify the entry point is within the bbox
        let (entry_xy, _) = result.unwrap();
        let (_, min_world_y) = material_stock.z_grid.cell_to_world(half_rows, half_cols);
        let (min_world_x, _) = material_stock.z_grid.cell_to_world(half_rows, half_cols);
        assert!(
            entry_xy.x >= min_world_x - cell_size && entry_xy.y >= min_world_y - cell_size,
            "Entry ({:.1}, {:.1}) should be within scan bbox (x>={:.1}, y>={:.1})",
            entry_xy.x,
            entry_xy.y,
            min_world_x,
            min_world_y
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

    // ── Pre-stamp thin bands tests ──────────────────────────────────────

    #[test]
    fn test_pre_stamp_clears_thin_bands() {
        use crate::slope::SlopeMap;
        // Thin material (0.5mm) on steep walls should be pre-stamped;
        // thick material (5mm) preserved.
        let cell_size = 1.0;
        let rows = 10;
        let cols = 10;
        let z_level = 7.0;
        let stock_to_leave = 0.5;
        let depth_per_pass = 3.0;

        // Surface: rows 0-4 are a steep ramp (z increases steeply with col),
        // rows 5-9 are flat at z=0. This makes rows 0-4 steep (>60°).
        let mut z_values = vec![0.0; rows * cols];
        for row in 0..5 {
            for col in 0..cols {
                z_values[row * cols + col] = col as f64 * 3.0; // dz/dx=3 → ~72° slope
            }
        }
        let surface_hm = SurfaceHeightmap {
            z_values: z_values.clone(),
            rows,
            cols,
            origin_x: 0.0,
            origin_y: 0.0,
            cell_size,
        };
        let slope_map = SlopeMap::from_z_grid(&z_values, rows, cols, 0.0, 0.0, cell_size);

        // Material at z=7.5 (thin: 0.5mm above floor) for steep cells (rows 0-4),
        // z=12.0 (thick: 5mm above floor) for flat cells (rows 5-9).
        let mut mat_cells = vec![7.5; rows * cols];
        for row in 5..rows {
            for col in 0..cols {
                mat_cells[row * cols + col] = 12.0;
            }
        }
        let mut material_stock =
            make_stock_with_cells(rows, cols, 0.0, 0.0, cell_size, -10.0, &mat_cells);

        let stamped = pre_stamp_thin_bands(
            &mut material_stock,
            &surface_hm,
            &slope_map,
            z_level,
            stock_to_leave,
            depth_per_pass,
            None,
        );

        // Only steep cells with thin bands should be stamped
        assert!(
            stamped > 0,
            "Should pre-stamp thin steep cells, stamped {}",
            stamped
        );

        // Thick cells should be unchanged
        for row in 5..rows {
            for col in 0..cols {
                let z = stock_top_z_at(&material_stock, row, col);
                assert!(
                    (z - 12.0).abs() < 0.1,
                    "Thick cell ({},{}) should be unchanged at 12.0, got {:.2}",
                    row,
                    col,
                    z
                );
            }
        }
    }

    #[test]
    fn test_pre_stamp_no_op_on_flat() {
        use crate::slope::SlopeMap;
        // Flat stock 5mm above surface — no cells should be pre-stamped
        // (flat surface = not steep, so pre-stamp skips all).
        let cell_size = 1.0;
        let rows = 10;
        let cols = 10;
        let z_level = 15.0;
        let stock_to_leave = 0.5;
        let depth_per_pass = 3.0;

        // Surface at z=0 → flat slope everywhere
        let z_values = vec![0.0; rows * cols];
        let surface_hm = SurfaceHeightmap {
            z_values: z_values.clone(),
            rows,
            cols,
            origin_x: 0.0,
            origin_y: 0.0,
            cell_size,
        };
        let slope_map = SlopeMap::from_z_grid(&z_values, rows, cols, 0.0, 0.0, cell_size);
        let mut material_stock = make_stock_with_cells(
            rows,
            cols,
            0.0,
            0.0,
            cell_size,
            -10.0,
            &vec![20.0; rows * cols],
        );

        let stamped = pre_stamp_thin_bands(
            &mut material_stock,
            &surface_hm,
            &slope_map,
            z_level,
            stock_to_leave,
            depth_per_pass,
            None,
        );

        assert_eq!(
            stamped, 0,
            "Flat stock well above surface should not be pre-stamped"
        );
    }

    #[test]
    fn test_pre_stamp_steep_only() {
        use crate::slope::SlopeMap;
        // Mixed terrain: steep cells (rows 0-4) and shallow cells (rows 5-9).
        // Both have thin bands, but only steep cells should be pre-stamped.
        let cell_size = 1.0;
        let rows = 10;
        let cols = 10;
        let z_level = 5.0;
        let stock_to_leave = 0.0;
        let depth_per_pass = 3.0;

        // Surface: rows 0-4 steep ramp (dz/dx=3 → ~72°), rows 5-9 flat
        let mut z_values = vec![0.0; rows * cols];
        for row in 0..5 {
            for col in 0..cols {
                z_values[row * cols + col] = col as f64 * 3.0;
            }
        }
        let surface_hm = SurfaceHeightmap {
            z_values: z_values.clone(),
            rows,
            cols,
            origin_x: 0.0,
            origin_y: 0.0,
            cell_size,
        };
        let slope_map = SlopeMap::from_z_grid(&z_values, rows, cols, 0.0, 0.0, cell_size);

        // All cells have thin material (0.5mm above floor)
        let mut mat_cells = vec![0.0; rows * cols];
        for row in 0..rows {
            for col in 0..cols {
                let surf_z = z_values[row * cols + col];
                let floor = (surf_z + stock_to_leave).max(z_level);
                mat_cells[row * cols + col] = floor + 0.5; // 0.5mm thin band everywhere
            }
        }
        let mut material_stock =
            make_stock_with_cells(rows, cols, 0.0, 0.0, cell_size, -10.0, &mat_cells);

        let stamped = pre_stamp_thin_bands(
            &mut material_stock,
            &surface_hm,
            &slope_map,
            z_level,
            stock_to_leave,
            depth_per_pass,
            None,
        );

        // Steep cells should be stamped, shallow cells should NOT
        assert!(stamped > 0, "Should stamp some steep thin bands");

        // Verify shallow cells (rows 7-9) are untouched (skip boundary rows 5-6
        // where finite differences see the steep→flat transition as steep)
        for row in 7..rows {
            for col in 0..cols {
                let original = mat_cells[row * cols + col];
                let current = stock_top_z_at(&material_stock, row, col);
                assert!(
                    (current - original).abs() < 0.1,
                    "Shallow cell ({},{}) should be untouched: was {:.2}, now {:.2}",
                    row,
                    col,
                    original,
                    current
                );
            }
        }
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
            .expect("test helper should not cancel");

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
        let params = default_params();
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
        assert!(
            trace
                .hotspots
                .iter()
                .any(|hotspot| hotspot.kind == "adaptive3d_pass"),
            "trace should record at least one adaptive 3D hotspot"
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
}
