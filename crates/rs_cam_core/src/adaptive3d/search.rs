//! 3D direction search, engagement computation, entry-point finding, and
//! path-validation helpers for adaptive3d clearing.

use crate::adaptive_shared::{angle_diff, blend_corners, refine_angle_bracket};
use crate::dexel_stock::TriDexelStock;
use crate::dropcutter::point_drop_cutter;
use crate::geo::{P2, P3};
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::slope::SurfaceHeightmap;
use crate::tool::MillingCutter;
use std::f64::consts::{PI, TAU};

use super::{MaterialRegion, stock_has_material_above, stock_top_z_at};

#[derive(Debug, Clone, Copy)]
pub(super) struct SearchDirection3dResult {
    pub(super) angle: f64,
    pub(super) z_next: f64,
    pub(super) evaluations: u32,
}

// ── 3D engagement ─────────────────────────────────────────────────────

/// Compute 3D engagement at position (cx, cy) for a given z_level.
///
/// For each cell in the tool circle, material exists if:
///   material_heightmap_z > max(surface_z + stock_to_leave, z_level) + epsilon
///
/// Returns fraction of cells with material in [0.0, 1.0].
pub(super) fn compute_engagement_3d(
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
pub(super) fn material_remaining_at_level(
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
pub(super) fn material_remaining_in_region(
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
#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn search_direction_3d(
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
pub(super) fn search_direction_3d_with_metrics(
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
pub(super) fn find_entry_3d(
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
pub(super) fn is_clear_path_3d(
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
pub(super) fn blend_corners_3d(path: &[P3], min_radius: f64) -> Vec<P3> {
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
