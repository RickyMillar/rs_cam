//! Surface slope analysis and heightmap infrastructure for 3D finishing strategies.
//!
//! Provides `SurfaceHeightmap` (precomputed mesh Z heights) and `SlopeMap` (surface normals,
//! slope angles, curvature). These are the shared foundation for scallop finishing,
//! steep & shallow, ramp finishing, and slope-aware adaptive improvements.

use crate::dropcutter::point_drop_cutter;
use crate::geo::V3;
use crate::interrupt::{CancelCheck, Cancelled, check_cancel};
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::tool::MillingCutter;

// ── Surface heightmap ─────────────────────────────────────────────────

/// Precomputed mesh surface Z heights at grid resolution.
/// One parallel batch of drop-cutter queries at init, then O(1) lookups.
pub struct SurfaceHeightmap {
    pub z_values: Vec<f64>,
    pub rows: usize,
    pub cols: usize,
    pub origin_x: f64,
    pub origin_y: f64,
    pub cell_size: f64,
}

impl SurfaceHeightmap {
    /// Build via rayon-parallelized drop-cutter queries at each grid cell.
    // infallible: cancel closure always returns false, so Cancelled is unreachable
    #[allow(clippy::too_many_arguments, clippy::expect_used)]
    pub fn from_mesh(
        mesh: &TriangleMesh,
        index: &SpatialIndex,
        cutter: &dyn MillingCutter,
        origin_x: f64,
        origin_y: f64,
        rows: usize,
        cols: usize,
        cell_size: f64,
        min_z: f64,
    ) -> Self {
        let never_cancel = || false;
        Self::from_mesh_with_cancel(
            mesh,
            index,
            cutter,
            origin_x,
            origin_y,
            rows,
            cols,
            cell_size,
            min_z,
            &never_cancel,
        )
        .expect("non-cancellable surface heightmap should never be cancelled")
    }

    #[allow(clippy::too_many_arguments)]
    pub fn from_mesh_with_cancel(
        mesh: &TriangleMesh,
        index: &SpatialIndex,
        cutter: &dyn MillingCutter,
        origin_x: f64,
        origin_y: f64,
        rows: usize,
        cols: usize,
        cell_size: f64,
        min_z: f64,
        cancel: &dyn CancelCheck,
    ) -> Result<Self, Cancelled> {
        let total = rows * cols;
        let compute_z = |i: usize| {
            let row = i / cols;
            let col = i % cols;
            let x = origin_x + col as f64 * cell_size;
            let y = origin_y + row as f64 * cell_size;
            let cl = point_drop_cutter(x, y, mesh, index, cutter);
            cl.z.max(min_z)
        };

        // Parallel drop-cutter: each cell is independent.
        #[cfg(not(target_arch = "wasm32"))]
        let z_values = {
            use rayon::prelude::*;
            let results: Vec<f64> = (0..total).into_par_iter().map(compute_z).collect();
            // Check cancel after parallel work completes
            check_cancel(cancel)?;
            results
        };
        #[cfg(target_arch = "wasm32")]
        let z_values = {
            let mut vals = Vec::with_capacity(total);
            for i in 0..total {
                if i % 64 == 0 {
                    check_cancel(cancel)?;
                }
                vals.push(compute_z(i));
            }
            vals
        };

        Ok(Self {
            z_values,
            rows,
            cols,
            origin_x,
            origin_y,
            cell_size,
        })
    }

    #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
    /// O(1) surface Z lookup by cell indices.
    #[inline]
    pub fn surface_z_at(&self, row: usize, col: usize) -> f64 {
        self.z_values[row * self.cols + col]
    }

    #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
    /// Surface Z at world coordinates. Returns NEG_INFINITY for out-of-bounds.
    pub fn surface_z_at_world(&self, x: f64, y: f64) -> f64 {
        let col_f = (x - self.origin_x) / self.cell_size;
        let row_f = (y - self.origin_y) / self.cell_size;
        if col_f < -0.5 || row_f < -0.5 {
            return f64::NEG_INFINITY;
        }
        let col = col_f.round() as isize;
        let row = row_f.round() as isize;
        if col < 0 || row < 0 || col >= self.cols as isize || row >= self.rows as isize {
            return f64::NEG_INFINITY;
        }
        self.z_values[row as usize * self.cols + col as usize]
    }

    /// Minimum surface Z across all cells (bottom of the mesh surface).
    pub fn min_z(&self) -> f64 {
        self.z_values.iter().copied().fold(f64::INFINITY, f64::min)
    }

    /// Compute a slope map from this surface heightmap.
    pub fn slope_map(&self) -> SlopeMap {
        SlopeMap::from_z_grid(
            &self.z_values,
            self.rows,
            self.cols,
            self.origin_x,
            self.origin_y,
            self.cell_size,
        )
    }
}

// ── Slope map ─────────────────────────────────────────────────────────

/// Grid of slope angles, surface normals, and mean curvature computed from a Z heightmap.
pub struct SlopeMap {
    /// Unit surface normal at each cell. Row-major.
    pub normals: Vec<V3>,
    /// Slope angle from horizontal at each cell, in radians [0, PI/2].
    /// 0 = horizontal flat, PI/2 = vertical wall.
    pub angles: Vec<f64>,
    /// Mean curvature at each cell (second derivatives).
    /// Positive = convex, negative = concave.
    pub curvatures: Vec<f64>,
    pub rows: usize,
    pub cols: usize,
    pub origin_x: f64,
    pub origin_y: f64,
    pub cell_size: f64,
}

impl SlopeMap {
    #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
    /// Build a SlopeMap from a grid of Z values using finite differences.
    ///
    /// Central differences for interior cells, forward/backward at boundaries.
    pub fn from_z_grid(
        z_values: &[f64],
        rows: usize,
        cols: usize,
        origin_x: f64,
        origin_y: f64,
        cell_size: f64,
    ) -> Self {
        let total = rows * cols;
        let mut normals = vec![V3::new(0.0, 0.0, 1.0); total];
        let mut angles = vec![0.0f64; total];
        let mut curvatures = vec![0.0f64; total];

        let cs = cell_size;
        let inv_cs = 1.0 / cs;
        let inv_cs2 = 1.0 / (cs * 2.0);
        let inv_cs_sq = 1.0 / (cs * cs);

        // Helper: compute normal, angle, curvature from derivatives and store.
        #[inline(always)]
        #[allow(clippy::indexing_slicing)] // SAFETY: idx bounded by caller loop ranges
        #[allow(clippy::needless_pass_by_value)] // tuple of mut refs is the natural pattern
        fn store_cell(
            out: (&mut [V3], &mut [f64], &mut [f64]),
            idx: usize,
            dz_dx: f64,
            dz_dy: f64,
            d2z_dx2: f64,
            d2z_dy2: f64,
        ) {
            let n = V3::new(-dz_dx, -dz_dy, 1.0).normalize();
            out.0[idx] = n;
            out.1[idx] = n.z.clamp(0.0, 1.0).acos();
            out.2[idx] = (d2z_dx2 + d2z_dy2) * 0.5;
        }

        // ── Interior cells: no boundary checks, full central differences ──
        for row in 1..rows.saturating_sub(1) {
            let row_base = row * cols;
            let row_above = (row - 1) * cols;
            let row_below = (row + 1) * cols;
            for col in 1..cols.saturating_sub(1) {
                // SAFETY: row in 1..rows-1 and col in 1..cols-1 ensures all offsets are in bounds
                #[allow(clippy::indexing_slicing)]
                let (dz_dx, dz_dy, d2z_dx2, d2z_dy2) = {
                    let zc = z_values[row_base + col];
                    let zl = z_values[row_base + col - 1];
                    let zr = z_values[row_base + col + 1];
                    let zu = z_values[row_above + col];
                    let zd = z_values[row_below + col];
                    (
                        (zr - zl) * inv_cs2,
                        (zd - zu) * inv_cs2,
                        (zr - 2.0 * zc + zl) * inv_cs_sq,
                        (zd - 2.0 * zc + zu) * inv_cs_sq,
                    )
                };
                store_cell(
                    (&mut normals, &mut angles, &mut curvatures),
                    row_base + col,
                    dz_dx,
                    dz_dy,
                    d2z_dx2,
                    d2z_dy2,
                );
            }
        }

        // ── Boundary cells: use forward/backward differences ──────────────
        for row in 0..rows {
            for col in 0..cols {
                // Skip interior (already processed)
                if row > 0 && row < rows - 1 && col > 0 && col < cols - 1 {
                    continue;
                }
                #[allow(clippy::indexing_slicing)]
                let dz_dx = if col == 0 && cols > 1 {
                    (z_values[row * cols + 1] - z_values[row * cols]) * inv_cs
                } else if col == cols - 1 && cols > 1 {
                    (z_values[row * cols + col] - z_values[row * cols + col - 1]) * inv_cs
                } else if cols > 1 {
                    (z_values[row * cols + col + 1] - z_values[row * cols + col - 1]) * inv_cs2
                } else {
                    0.0
                };

                #[allow(clippy::indexing_slicing)]
                let dz_dy = if row == 0 && rows > 1 {
                    (z_values[cols + col] - z_values[col]) * inv_cs
                } else if row == rows - 1 && rows > 1 {
                    (z_values[row * cols + col] - z_values[(row - 1) * cols + col]) * inv_cs
                } else if rows > 1 {
                    (z_values[(row + 1) * cols + col] - z_values[(row - 1) * cols + col]) * inv_cs2
                } else {
                    0.0
                };

                let d2z_dx2 = if col > 0 && col < cols - 1 {
                    #[allow(clippy::indexing_slicing)]
                    let v = (z_values[row * cols + col + 1] - 2.0 * z_values[row * cols + col]
                        + z_values[row * cols + col - 1])
                        * inv_cs_sq;
                    v
                } else {
                    0.0
                };

                let d2z_dy2 = if row > 0 && row < rows - 1 {
                    #[allow(clippy::indexing_slicing)]
                    let v = (z_values[(row + 1) * cols + col] - 2.0 * z_values[row * cols + col]
                        + z_values[(row - 1) * cols + col])
                        * inv_cs_sq;
                    v
                } else {
                    0.0
                };

                store_cell(
                    (&mut normals, &mut angles, &mut curvatures),
                    row * cols + col,
                    dz_dx,
                    dz_dy,
                    d2z_dx2,
                    d2z_dy2,
                );
            }
        }

        Self {
            normals,
            angles,
            curvatures,
            rows,
            cols,
            origin_x,
            origin_y,
            cell_size,
        }
    }

    #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
    /// Slope angle at cell indices (radians from horizontal).
    #[inline]
    pub fn angle_at(&self, row: usize, col: usize) -> f64 {
        self.angles[row * self.cols + col]
    }

    #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
    /// Surface normal at cell indices.
    #[inline]
    pub fn normal_at(&self, row: usize, col: usize) -> V3 {
        self.normals[row * self.cols + col]
    }

    #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
    /// Mean curvature at cell indices.
    #[inline]
    pub fn curvature_at(&self, row: usize, col: usize) -> f64 {
        self.curvatures[row * self.cols + col]
    }

    #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
    /// Slope angle at world coordinates. Returns None for out-of-bounds.
    pub fn angle_at_world(&self, x: f64, y: f64) -> Option<f64> {
        let (row, col) = self.world_to_cell(x, y)?;
        Some(self.angles[row * self.cols + col])
    }

    #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
    /// Mean curvature at world coordinates. Returns None for out-of-bounds.
    pub fn curvature_at_world(&self, x: f64, y: f64) -> Option<f64> {
        let (row, col) = self.world_to_cell(x, y)?;
        Some(self.curvatures[row * self.cols + col])
    }

    pub fn world_to_cell(&self, x: f64, y: f64) -> Option<(usize, usize)> {
        let col_f = (x - self.origin_x) / self.cell_size;
        let row_f = (y - self.origin_y) / self.cell_size;
        if col_f < -0.5 || row_f < -0.5 {
            return None;
        }
        let col = col_f.round() as isize;
        let row = row_f.round() as isize;
        if col < 0 || row < 0 || col >= self.cols as isize || row >= self.rows as isize {
            return None;
        }
        Some((row as usize, col as usize))
    }
}

/// Classify cells into steep vs shallow based on threshold angle.
///
/// Returns a boolean grid (row-major): `true` = steep (angle >= threshold).
/// `threshold_deg` is in degrees from horizontal (0-90).
pub fn classify_steep_shallow(slope_map: &SlopeMap, threshold_deg: f64) -> Vec<bool> {
    let threshold_rad = threshold_deg.to_radians();
    slope_map
        .angles
        .iter()
        .map(|&a| a >= threshold_rad)
        .collect()
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
    use std::f64::consts::FRAC_PI_4;

    fn make_flat_z_grid(rows: usize, cols: usize) -> Vec<f64> {
        vec![0.0; rows * cols]
    }

    fn make_ramp_z_grid(rows: usize, cols: usize, cell_size: f64) -> Vec<f64> {
        // Linear ramp: z = x, so dz/dx = 1 → 45-degree slope
        let mut z = vec![0.0; rows * cols];
        for row in 0..rows {
            for col in 0..cols {
                z[row * cols + col] = col as f64 * cell_size;
            }
        }
        z
    }

    fn make_dome_z_grid(rows: usize, cols: usize, cell_size: f64, radius: f64) -> Vec<f64> {
        // Hemisphere dome: z = sqrt(R^2 - x^2 - y^2), centered in grid
        let cx = (cols - 1) as f64 * cell_size * 0.5;
        let cy = (rows - 1) as f64 * cell_size * 0.5;
        let mut z = vec![0.0; rows * cols];
        for row in 0..rows {
            for col in 0..cols {
                let x = col as f64 * cell_size - cx;
                let y = row as f64 * cell_size - cy;
                let r_sq = radius * radius - x * x - y * y;
                z[row * cols + col] = if r_sq > 0.0 { r_sq.sqrt() } else { 0.0 };
            }
        }
        z
    }

    // ── SurfaceHeightmap tests ──────────────────────────────────────

    #[test]
    fn test_surface_heightmap_z_lookup() {
        let z_values = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]; // 2 rows × 3 cols
        let shm = SurfaceHeightmap {
            z_values,
            rows: 2,
            cols: 3,
            origin_x: 0.0,
            origin_y: 0.0,
            cell_size: 1.0,
        };
        assert_eq!(shm.surface_z_at(0, 0), 1.0);
        assert_eq!(shm.surface_z_at(0, 2), 3.0);
        assert_eq!(shm.surface_z_at(1, 1), 5.0);
    }

    #[test]
    fn test_surface_heightmap_world_lookup() {
        let z_values = vec![10.0, 20.0, 30.0, 40.0];
        let shm = SurfaceHeightmap {
            z_values,
            rows: 2,
            cols: 2,
            origin_x: 5.0,
            origin_y: 10.0,
            cell_size: 2.0,
        };
        assert_eq!(shm.surface_z_at_world(5.0, 10.0), 10.0);
        assert_eq!(shm.surface_z_at_world(7.0, 10.0), 20.0);
        assert_eq!(shm.surface_z_at_world(0.0, 0.0), f64::NEG_INFINITY); // out of bounds
    }

    #[test]
    fn test_surface_heightmap_min_z() {
        let shm = SurfaceHeightmap {
            z_values: vec![5.0, 2.0, 8.0, 1.0],
            rows: 2,
            cols: 2,
            origin_x: 0.0,
            origin_y: 0.0,
            cell_size: 1.0,
        };
        assert_eq!(shm.min_z(), 1.0);
    }

    // ── SlopeMap tests ──────────────────────────────────────────────

    #[test]
    fn test_slope_flat_surface() {
        let z = make_flat_z_grid(10, 10);
        let sm = SlopeMap::from_z_grid(&z, 10, 10, 0.0, 0.0, 1.0);
        for row in 0..10 {
            for col in 0..10 {
                let angle = sm.angle_at(row, col);
                assert!(
                    angle.abs() < 0.01,
                    "Flat surface angle at ({},{}) should be ~0, got {:.4}",
                    row,
                    col,
                    angle
                );
                let n = sm.normal_at(row, col);
                assert!(
                    (n.z - 1.0).abs() < 0.01,
                    "Flat normal Z at ({},{}) should be ~1, got {:.4}",
                    row,
                    col,
                    n.z
                );
            }
        }
    }

    #[test]
    fn test_slope_45_degree_ramp() {
        let z = make_ramp_z_grid(10, 10, 1.0);
        let sm = SlopeMap::from_z_grid(&z, 10, 10, 0.0, 0.0, 1.0);
        // Interior cells should have ~45 degree slope (dz/dx = 1)
        for row in 1..9 {
            for col in 1..9 {
                let angle = sm.angle_at(row, col);
                assert!(
                    (angle - FRAC_PI_4).abs() < 0.01,
                    "Ramp angle at ({},{}) should be ~45° ({:.4}), got {:.4} ({:.1}°)",
                    row,
                    col,
                    FRAC_PI_4,
                    angle,
                    angle.to_degrees()
                );
            }
        }
    }

    #[test]
    fn test_slope_hemisphere() {
        let z = make_dome_z_grid(20, 20, 1.0, 8.0);
        let sm = SlopeMap::from_z_grid(&z, 20, 20, 0.0, 0.0, 1.0);

        // Center should be nearly flat
        let center_angle = sm.angle_at(10, 10);
        assert!(
            center_angle < 15.0_f64.to_radians(),
            "Hemisphere center should be nearly flat, got {:.1}°",
            center_angle.to_degrees()
        );

        // Edge cells (near radius boundary) should be steep
        // Find a cell that's on the slope
        let edge_angle = sm.angle_at(3, 10); // Near the edge in Y direction
        assert!(
            edge_angle > 30.0_f64.to_radians(),
            "Hemisphere edge should be steep, got {:.1}°",
            edge_angle.to_degrees()
        );
    }

    #[test]
    fn test_curvature_convex() {
        // Dome (convex upward) should have negative d2z/dx2 at the peak
        // (surface curves downward from peak → concave in math terms,
        // but convex in the physical sense of a hill)
        let z = make_dome_z_grid(20, 20, 1.0, 8.0);
        let sm = SlopeMap::from_z_grid(&z, 20, 20, 0.0, 0.0, 1.0);
        let center_k = sm.curvature_at(10, 10);
        // For a dome, d2z/dx2 < 0 (concave down), so kappa < 0
        // This means "convex" in physical terms = negative curvature in our heightmap convention
        assert!(
            center_k < 0.0,
            "Dome peak curvature should be negative (convex surface), got {:.6}",
            center_k
        );
    }

    #[test]
    fn test_curvature_flat() {
        let z = make_flat_z_grid(10, 10);
        let sm = SlopeMap::from_z_grid(&z, 10, 10, 0.0, 0.0, 1.0);
        for row in 1..9 {
            for col in 1..9 {
                let k = sm.curvature_at(row, col);
                assert!(
                    k.abs() < 0.001,
                    "Flat surface curvature at ({},{}) should be ~0, got {:.6}",
                    row,
                    col,
                    k
                );
            }
        }
    }

    #[test]
    fn test_curvature_bowl() {
        // Bowl (concave): z = x^2 + y^2 → d2z/dx2 = 2, d2z/dy2 = 2 → kappa = 2
        let rows = 10;
        let cols = 10;
        let cs = 1.0;
        let cx = 4.5;
        let cy = 4.5;
        let mut z = vec![0.0; rows * cols];
        for row in 0..rows {
            for col in 0..cols {
                let x = col as f64 * cs - cx;
                let y = row as f64 * cs - cy;
                z[row * cols + col] = x * x + y * y;
            }
        }
        let sm = SlopeMap::from_z_grid(&z, rows, cols, 0.0, 0.0, cs);
        // Interior cells should have positive curvature (concave up = bowl)
        let k = sm.curvature_at(5, 5);
        assert!(
            (k - 2.0).abs() < 0.1,
            "Bowl curvature should be ~2.0, got {:.4}",
            k
        );
    }

    // ── Classification tests ────────────────────────────────────────

    #[test]
    fn test_classify_hemisphere() {
        let z = make_dome_z_grid(20, 20, 1.0, 8.0);
        let sm = SlopeMap::from_z_grid(&z, 20, 20, 0.0, 0.0, 1.0);
        let steep = classify_steep_shallow(&sm, 40.0);

        // Center should be shallow (not steep)
        assert!(
            !steep[10 * 20 + 10],
            "Hemisphere center should be classified as shallow"
        );

        // Count steep vs shallow
        let steep_count = steep.iter().filter(|&&s| s).count();
        let shallow_count = steep.iter().filter(|&&s| !s).count();
        assert!(
            steep_count > 0 && shallow_count > 0,
            "Should have both steep ({}) and shallow ({}) cells",
            steep_count,
            shallow_count
        );
    }

    #[test]
    fn test_classify_flat_all_shallow() {
        let z = make_flat_z_grid(10, 10);
        let sm = SlopeMap::from_z_grid(&z, 10, 10, 0.0, 0.0, 1.0);
        let steep = classify_steep_shallow(&sm, 10.0);
        assert!(
            steep.iter().all(|&s| !s),
            "Flat surface should be all shallow"
        );
    }

    #[test]
    fn test_classify_ramp_threshold() {
        // 45-degree ramp, threshold at 30 → all steep interior cells
        let z = make_ramp_z_grid(10, 10, 1.0);
        let sm = SlopeMap::from_z_grid(&z, 10, 10, 0.0, 0.0, 1.0);
        let steep_30 = classify_steep_shallow(&sm, 30.0);
        // Interior cells are ~45°, which is > 30° → steep
        let interior_steep = (1..9).all(|r| (1..9).all(|c| steep_30[r * 10 + c]));
        assert!(interior_steep, "45° ramp should be steep at 30° threshold");

        // Threshold at 50 → all shallow
        let steep_50 = classify_steep_shallow(&sm, 50.0);
        let interior_shallow = (1..9).all(|r| (1..9).all(|c| !steep_50[r * 10 + c]));
        assert!(
            interior_shallow,
            "45° ramp should be shallow at 50° threshold"
        );
    }

    // ── World coordinate accessor tests ─────────────────────────────

    #[test]
    fn test_slope_world_accessors() {
        let z = make_ramp_z_grid(10, 10, 2.0);
        let sm = SlopeMap::from_z_grid(&z, 10, 10, 0.0, 0.0, 2.0);

        // Interior point
        let angle = sm
            .angle_at_world(10.0, 10.0)
            .expect("interior point should return Some");
        assert!(
            (angle - FRAC_PI_4).abs() < 0.05,
            "Should be ~45°, got {:.1}°",
            angle.to_degrees()
        );

        // Out of bounds
        assert!(sm.angle_at_world(-100.0, -100.0).is_none());

        // Curvature accessor
        let k = sm.curvature_at_world(10.0, 10.0);
        assert!(k.is_some());
    }
}
