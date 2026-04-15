//! 3D direction search, engagement computation, entry-point finding, and
//! path-validation helpers for adaptive3d clearing.

use crate::adaptive_shared::blend_corners;
use crate::dexel_stock::TriDexelStock;
use crate::geo::{P2, P3};
use crate::slope::SurfaceHeightmap;

use super::clearing::MaterialRegion;
use super::{stock_has_material_above, stock_top_z_at};

// ── 3D engagement ─────────────────────────────────────────────────────

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
