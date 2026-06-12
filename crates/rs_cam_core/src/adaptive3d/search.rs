//! 3D direction search, engagement computation, entry-point finding, and
//! path-validation helpers for adaptive3d clearing.

use crate::adaptive_shared::blend_corners;
use crate::dexel_stock::TriDexelStock;
use crate::geo::{P2, P3};
use crate::slope::SurfaceHeightmap;

use super::clearing::MaterialRegion;
use super::{stock_has_material_above, stock_top_z_at};

// ── 3D engagement ─────────────────────────────────────────────────────

/// Per-Z floor and material diagnostics. Returned by
/// [`material_remaining_at_level_diag`] for debug-trace consumers; the
/// scalar [`material_remaining_at_level`] keeps its existing signature
/// for the planner's cheap exit check.
///
/// Cell-bucket semantics (matches the planner's `floor` calc
/// `(surf_z + stock_to_leave).max(z_level)`):
/// - `cells_total`: every cell in the grid (or region bbox).
/// - `cells_at_z`: cells where the effective floor IS z_level — the
///   planner can cut down to z_level here.
/// - `cells_surf_above`: cells where `surf+stock_to_leave > z_level` — the
///   planner CANNOT cut to z_level here; the surface (or its
///   stock-to-leave offset) sits above the cut plane.
/// - `cells_with_material`: cells where material remains above the
///   effective floor. Subset of `cells_at_z`.
///
/// `fraction = cells_with_material / cells_at_z` (matches the legacy
/// scalar reading; equals 0 when `cells_at_z == 0`).
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct MaterialFloorDiagnostic {
    pub fraction: f64,
    pub cells_total: u64,
    pub cells_at_z: u64,
    pub cells_surf_above: u64,
    pub cells_with_material: u64,
}

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// Diagnostic counterpart to [`material_remaining_at_level`]. Only call
/// when a debug span is open — this is the slightly-more-expensive path
/// that records the floor-cell histogram.
pub(super) fn material_remaining_at_level_diag(
    material_stock: &TriDexelStock,
    surface_hm: &SurfaceHeightmap,
    z_level: f64,
    stock_to_leave: f64,
) -> MaterialFloorDiagnostic {
    let grid = &material_stock.z_grid;
    let mut diag = MaterialFloorDiagnostic::default();
    for row in 0..grid.rows {
        for col in 0..grid.cols {
            let i = row * grid.cols + col;
            let surf_z = surface_hm.z_values[i];
            let floor = (surf_z + stock_to_leave).max(z_level);
            diag.cells_total += 1;
            if surf_z + stock_to_leave <= z_level + 0.01 {
                diag.cells_at_z += 1;
                if stock_has_material_above(material_stock, row, col, floor + 0.01) {
                    diag.cells_with_material += 1;
                }
            } else {
                diag.cells_surf_above += 1;
            }
        }
    }
    diag.fraction = if diag.cells_at_z == 0 {
        0.0
    } else {
        diag.cells_with_material as f64 / diag.cells_at_z as f64
    };
    diag
}

/// Compact result for the planner's per-level early-exit check.
/// Returned by [`material_remaining_at_level`] and
/// [`material_remaining_in_region`].
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct MaterialRemaining {
    pub cells_with_material: u64,
    pub cells_at_z: u64,
}

impl MaterialRemaining {
    pub fn fraction(self) -> f64 {
        if self.cells_at_z == 0 {
            0.0
        } else {
            self.cells_with_material as f64 / self.cells_at_z as f64
        }
    }
}

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// Cells where material remains above the effective floor at a given
/// z_level. Used to decide when a level is done — callers should gate on
/// the absolute `cells_with_material` count (not the fraction), because at
/// small depth-per-pass a real island contributes very few cells per level
/// and a fraction-based gate would silently skip it.
pub(super) fn material_remaining_at_level(
    material_stock: &TriDexelStock,
    surface_hm: &SurfaceHeightmap,
    z_level: f64,
    stock_to_leave: f64,
) -> MaterialRemaining {
    let grid = &material_stock.z_grid;
    let mut out = MaterialRemaining::default();
    for row in 0..grid.rows {
        for col in 0..grid.cols {
            let i = row * grid.cols + col;
            let surf_z = surface_hm.z_values[i];
            let floor = (surf_z + stock_to_leave).max(z_level);
            if surf_z + stock_to_leave <= z_level + 0.01 {
                out.cells_at_z += 1;
                if stock_has_material_above(material_stock, row, col, floor + 0.01) {
                    out.cells_with_material += 1;
                }
            }
        }
    }
    out
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
) -> MaterialRemaining {
    let grid = &material_stock.z_grid;
    let mut out = MaterialRemaining::default();
    for row in region.row_min..=region.row_max.min(grid.rows - 1) {
        for col in region.col_min..=region.col_max.min(grid.cols - 1) {
            let i = row * grid.cols + col;
            let surf_z = surface_hm.z_values[i];
            let floor = (surf_z + stock_to_leave).max(z_level);
            if surf_z + stock_to_leave <= z_level + 0.01 {
                out.cells_at_z += 1;
                if stock_has_material_above(material_stock, row, col, floor + 0.01) {
                    out.cells_with_material += 1;
                }
            }
        }
    }
    out
}

// ── Link vs retract ───────────────────────────────────────────────────

/// Vertical allowance absorbing dexel/heightmap discretisation noise
/// (~0.5 mm at standard sim resolution — same rationale as the F-038b
/// `stay_down_clearance_mm` default). Stock within this band of the
/// travel line is treated as free travel, not a cut.
const LINK_DEXEL_NOISE_MM: f64 = 0.5;

/// Check if the tool can safely feed from `from` to `to` without hitting
/// excessive material above the travel line.
///
/// Three-tier predicate per sample (algorithm review 2026-06-12, Stage 0
/// fix — pre-fix the surface heightmap and `stock_to_leave` parameters
/// were accepted and ignored, the block test was a hardcoded
/// `travel + 1.0 mm`, and any <20% of the samples could sit arbitrarily
/// high above the line):
/// - **protected surface** (`surf_z + stock_to_leave`) above the travel
///   line ⇒ reject outright. Feeding through would gouge the finished
///   surface, not removable residue — the stock top alone cannot tell
///   the two apart.
/// - stock more than `max_bite` (depth-per-pass) above the travel line ⇒
///   reject outright. The link would take a bigger axial bite than any
///   planned pass, regardless of how few samples it covers.
/// - stock within `(noise, max_bite]` above ⇒ a skim of removable
///   residue at feed rate. Allowed while skims cover < 20% of the
///   samples — a link that mostly cuts should be a retract instead.
pub(super) fn is_clear_path_3d(
    material_stock: &TriDexelStock,
    surface_hm: &SurfaceHeightmap,
    from: P3,
    to: P3,
    stock_to_leave: f64,
    max_bite: f64,
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
    let mut skims = 0u32;

    for i in 0..=n_samples {
        let t = i as f64 / n_samples as f64;
        let x = from.x + t * dx;
        let y = from.y + t * dy;
        let z = from.z + t * (to.z - from.z);

        if let Some((row, col)) = grid.world_to_cell(x, y) {
            let surf_z = surface_hm.surface_z_at_world(x, y);
            if surf_z.is_finite() && surf_z + stock_to_leave > z + LINK_DEXEL_NOISE_MM {
                return false;
            }
            let mat_z = stock_top_z_at(material_stock, row, col);
            if mat_z > z + max_bite.max(LINK_DEXEL_NOISE_MM) {
                return false;
            }
            if mat_z > z + LINK_DEXEL_NOISE_MM {
                skims += 1;
            }
        }
    }

    let skim_frac = skims as f64 / (n_samples + 1) as f64;
    skim_frac < 0.2
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

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod link_gate_tests {
    //! Sentries for the `is_clear_path_3d` floor fix (algorithm review
    //! 2026-06-12, Stage 0). Pre-fix the function ignored the surface
    //! heightmap and `stock_to_leave`, used a hardcoded 1.0 mm clearance,
    //! and allowed any <20% of samples to sit arbitrarily high — a tall
    //! uncut ridge covering a sliver of the link was fed through at full
    //! depth, and links could gouge the protected (stock-to-leave)
    //! surface.

    use super::is_clear_path_3d;
    use crate::dexel_stock::TriDexelStock;
    use crate::geo::P3;
    use crate::slope::SurfaceHeightmap;

    const STOCK_TOP: f64 = 10.0;
    const CELL: f64 = 1.0;

    /// 60×60 stock, everything carved down to `carved_z`, with a flat
    /// "surface" heightmap at `surf_z` matching the dexel grid layout.
    fn scene(carved_z: f32, surf_z: f64) -> (TriDexelStock, SurfaceHeightmap) {
        let mut stock = TriDexelStock::from_stock(0.0, 0.0, 60.0, 60.0, 0.0, STOCK_TOP, CELL);
        let (rows, cols) = (stock.z_grid.rows, stock.z_grid.cols);
        for row in 0..rows {
            for col in 0..cols {
                stock.clear_above_at(row, col, carved_z);
            }
        }
        let hm = SurfaceHeightmap {
            z_values: vec![surf_z; rows * cols],
            covered: vec![true; rows * cols],
            rows,
            cols,
            origin_x: stock.z_grid.origin_u,
            origin_y: stock.z_grid.origin_v,
            cell_size: stock.z_grid.cell_size,
        };
        (stock, hm)
    }

    /// Re-raise stock to `top` on a band of columns (a ridge crossing
    /// the link line). `clear_above_at` only removes material, so the
    /// ridge is built by carving everything else in `scene` and
    /// rebuilding the stock fresh for ridge cells.
    fn scene_with_ridge(
        carved_z: f32,
        surf_z: f64,
        ridge_cols: std::ops::Range<usize>,
    ) -> (TriDexelStock, SurfaceHeightmap) {
        let mut stock = TriDexelStock::from_stock(0.0, 0.0, 60.0, 60.0, 0.0, STOCK_TOP, CELL);
        let (rows, cols) = (stock.z_grid.rows, stock.z_grid.cols);
        for row in 0..rows {
            for col in 0..cols {
                if !ridge_cols.contains(&col) {
                    stock.clear_above_at(row, col, carved_z);
                }
            }
        }
        let hm = SurfaceHeightmap {
            z_values: vec![surf_z; rows * cols],
            covered: vec![true; rows * cols],
            rows,
            cols,
            origin_x: stock.z_grid.origin_u,
            origin_y: stock.z_grid.origin_v,
            cell_size: stock.z_grid.cell_size,
        };
        (stock, hm)
    }

    fn link_from() -> P3 {
        P3::new(5.0, 30.0, 5.0)
    }
    fn link_to() -> P3 {
        P3::new(55.0, 30.0, 5.0)
    }
    const STOCK_TO_LEAVE: f64 = 0.5;
    const MAX_BITE: f64 = 3.0; // depth_per_pass

    #[test]
    fn clear_carved_path_links() {
        // Everything carved to 4.0, travel at 5.0, surface far below.
        let (stock, hm) = scene(4.0, 0.0);
        assert!(is_clear_path_3d(
            &stock,
            &hm,
            link_from(),
            link_to(),
            STOCK_TO_LEAVE,
            MAX_BITE
        ));
    }

    #[test]
    fn tall_ridge_on_a_sliver_of_the_link_rejects() {
        // Pre-fix sentry: an uncut full-height ridge (top = 10.0, travel
        // at 5.0) covering ~3 of 50 mm — well under the 20% sample
        // fraction — was fed through at a 5 mm bite. Must reject.
        let (stock, hm) = scene_with_ridge(4.0, 0.0, 28..31);
        assert!(!is_clear_path_3d(
            &stock,
            &hm,
            link_from(),
            link_to(),
            STOCK_TO_LEAVE,
            MAX_BITE
        ));
    }

    #[test]
    fn protected_surface_above_travel_rejects() {
        // Stock carved to the travel height, but the *surface* sits at
        // 5.2 with stock_to_leave 0.5 ⇒ protected top 5.7 > 5.0 + noise.
        // Pre-fix this linked (surface param was ignored); the feed
        // would gouge the finished surface. Must reject.
        let (stock, hm) = scene(4.0, 5.2);
        assert!(!is_clear_path_3d(
            &stock,
            &hm,
            link_from(),
            link_to(),
            STOCK_TO_LEAVE,
            MAX_BITE
        ));
    }

    #[test]
    fn shallow_residue_skim_on_a_sliver_links() {
        // A 1.5 mm-proud residue strip (top 6.5, travel 5.0, bite ≤ 3.0,
        // surface far below) covering ~6% of the link: a legitimate
        // feed-rate skim of removable material — must still link.
        let (mut stock, hm) = scene_with_ridge(4.0, 0.0, 28..31);
        let rows = stock.z_grid.rows;
        for row in 0..rows {
            for col in 28..31 {
                stock.clear_above_at(row, col, 6.5);
            }
        }
        assert!(is_clear_path_3d(
            &stock,
            &hm,
            link_from(),
            link_to(),
            STOCK_TO_LEAVE,
            MAX_BITE
        ));
    }
}
