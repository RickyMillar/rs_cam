//! Scalar grid-field math shared by the adaptive clearing engines.
//!
//! This is pure numerical work on flat `rows * cols` grids — Euclidean
//! distance transform, level-set curvature, and box blur. None of it is
//! contour extraction; it moved out of `contour_extract` (which is about
//! turning boolean grids into polygons) so that module could stay scoped
//! to marching squares. See `planning/finishing_stack_review_2026-07.md`
//! item R1.2.

// ---------------------------------------------------------------------------
// Euclidean Distance Transform (Felzenszwalb & Huttenlocher 2004)
// ---------------------------------------------------------------------------

/// 1D parabola-envelope distance transform.
///
/// Input: `f[i] = 0.0` for source cells, `f[i] = very_large` for others.
/// Output: `f[i] = squared_distance_to_nearest_source`.
///
/// Reference: Felzenszwalb & Huttenlocher, "Distance Transforms of Sampled
/// Functions", Theory of Computing 2012.
#[allow(clippy::indexing_slicing)] // SAFETY: all indices bounded by loop variables and n
fn edt_1d(f: &mut [f64]) {
    let n = f.len();
    if n == 0 {
        return;
    }
    let mut v = vec![0usize; n];
    let mut z = vec![0.0f64; n + 1];
    let mut k = 0usize;
    z[0] = f64::NEG_INFINITY;
    z[1] = f64::INFINITY;

    for q in 1..n {
        loop {
            let vk = v[k];
            let s = ((f[q] + (q * q) as f64) - (f[vk] + (vk * vk) as f64))
                / (2.0 * (q as f64 - vk as f64));
            if s > z[k] {
                k += 1;
                v[k] = q;
                z[k] = s;
                z[k + 1] = f64::INFINITY;
                break;
            }
            if k == 0 {
                v[0] = q;
                z[1] = f64::INFINITY;
                break;
            }
            k -= 1;
        }
    }

    // Output pass must read the ORIGINAL f values: writing in place
    // corrupts f[v[k]] for parabola centers whose own cell is won by a
    // neighboring parabola (q = v[k] lies left of the envelope segment
    // where v[k] wins), and every later q on that segment then reads the
    // already-lowered value and under-reports the distance. Found via
    // the diamond-polygon oracle in the Stage 0 EDT-sharing work
    // (algorithm review 2026-06-12, F4): the 45°-edge diamond's center
    // read 5.83 instead of h/√2 ≈ 7.07.
    let orig: Vec<f64> = f.to_vec();
    k = 0;
    for (q, out) in f.iter_mut().enumerate() {
        while z[k + 1] < q as f64 {
            k += 1;
        }
        let vk = v[k];
        *out = (q as f64 - vk as f64).powi(2) + orig[vk];
    }
}

/// 2D Euclidean Distance Transform on a boolean grid.
///
/// Returns the Euclidean distance (in cell units) from each cell to the
/// nearest `true` cell. Uses two-pass separable 1D EDT
/// (Felzenszwalb & Huttenlocher 2004) — O(rows * cols) total.
#[allow(clippy::indexing_slicing)] // SAFETY: all indices bounded by rows/cols loop variables
pub fn distance_transform_2d(grid: &[bool], rows: usize, cols: usize) -> Vec<f64> {
    let total = rows * cols;
    let big = (rows * rows + cols * cols) as f64; // larger than any possible distance^2
    let mut dist = vec![0.0f64; total];

    // Initialize: source cells (true) = 0, others = big
    for (d, g) in dist.iter_mut().zip(grid.iter()) {
        *d = if *g { 0.0 } else { big };
    }

    // Horizontal pass
    for r in 0..rows {
        let start = r * cols;
        edt_1d(&mut dist[start..start + cols]);
    }

    // Vertical pass (column by column with temp buffer)
    let mut col_buf = vec![0.0f64; rows];
    for c in 0..cols {
        for r in 0..rows {
            col_buf[r] = dist[r * cols + c];
        }
        edt_1d(&mut col_buf);
        for r in 0..rows {
            dist[r * cols + c] = col_buf[r];
        }
    }

    // Convert squared distances to actual distances
    for d in &mut dist {
        *d = d.sqrt();
    }

    dist
}

/// Compute the curvature of EDT level-set curves at each grid cell.
///
/// Uses the standard level-set curvature formula:
///   κ = (d_xx·d_y² − 2·d_xy·d_x·d_y + d_yy·d_x²) / (d_x² + d_y²)^(3/2)
///
/// where d_x, d_y are first partial derivatives and d_xx, d_yy, d_xy are
/// second partial derivatives of the distance field, computed via finite
/// differences.
///
/// Returns curvature in cell⁻¹ units (positive = convex, negative = concave).
/// Cells where the gradient magnitude is near zero (flat regions, medial axis)
/// are set to κ = 0.
#[allow(clippy::indexing_slicing)] // SAFETY: all indices bounded by row/col loop ranges
pub fn edt_curvature_field(edt: &[f64], rows: usize, cols: usize) -> Vec<f64> {
    let total = rows * cols;
    let mut curvature = vec![0.0f64; total];

    if rows < 3 || cols < 3 {
        return curvature;
    }

    // Interior cells: central differences (rows 1..rows-1, cols 1..cols-1)
    for r in 1..rows - 1 {
        for c in 1..cols - 1 {
            let idx = r * cols + c;
            let zc = edt[idx];
            let zl = edt[idx - 1];
            let zr = edt[idx + 1];
            let zu = edt[(r - 1) * cols + c];
            let zd = edt[(r + 1) * cols + c];
            let zul = edt[(r - 1) * cols + c - 1];
            let zur = edt[(r - 1) * cols + c + 1];
            let zdl = edt[(r + 1) * cols + c - 1];
            let zdr = edt[(r + 1) * cols + c + 1];

            // First derivatives (central differences, cell_size = 1)
            let dx = (zr - zl) * 0.5;
            let dy = (zd - zu) * 0.5;

            // Second derivatives
            let dxx = zr - 2.0 * zc + zl;
            let dyy = zd - 2.0 * zc + zu;
            let dxy = (zdr - zdl - zur + zul) * 0.25;

            // Gradient magnitude squared
            let grad_sq = dx * dx + dy * dy;
            if grad_sq < 1e-12 {
                // Near medial axis or flat region — curvature undefined
                continue;
            }

            // Level-set curvature
            let numer = dxx * dy * dy - 2.0 * dxy * dx * dy + dyy * dx * dx;
            let denom = grad_sq * grad_sq.sqrt(); // (grad_sq)^(3/2)
            curvature[idx] = numer / denom;
        }
    }

    curvature
}

/// Box-blur smooth a 2D grid in place.
///
/// `radius` is the half-width of the kernel (e.g. radius=2 → 5×5 kernel).
/// Boundary cells within `radius` of the edge are left unchanged.
#[allow(clippy::indexing_slicing)] // SAFETY: all indices bounded by row/col loop ranges
pub fn smooth_grid(grid: &mut [f64], rows: usize, cols: usize, radius: usize) {
    if radius == 0 || rows <= 2 * radius || cols <= 2 * radius {
        return;
    }
    let total = rows * cols;
    let mut tmp = vec![0.0f64; total];
    let side = 2 * radius + 1;
    let inv_area = 1.0 / (side * side) as f64;

    for r in radius..rows - radius {
        for c in radius..cols - radius {
            let mut sum = 0.0;
            for dr in 0..side {
                let row_off = (r + dr - radius) * cols;
                for dc in 0..side {
                    sum += grid[row_off + c + dc - radius];
                }
            }
            tmp[r * cols + c] = sum * inv_area;
        }
    }

    // Copy smoothed interior back; boundary retains original values
    for r in radius..rows - radius {
        let start = r * cols + radius;
        let end = r * cols + cols - radius;
        grid[start..end].copy_from_slice(&tmp[start..end]);
    }
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

    // -----------------------------------------------------------------------
    // Tests for Euclidean Distance Transform
    // -----------------------------------------------------------------------

    #[test]
    fn test_edt_1d_simple() {
        // Source at center (index 5 of 11), all others large
        let n = 11;
        let big = (n * n) as f64;
        let mut f = vec![big; n];
        f[5] = 0.0;
        edt_1d(&mut f);
        // After EDT, f[i] should be squared distance to index 5
        for (i, val) in f.iter().enumerate() {
            let expected = ((i as f64) - 5.0).powi(2);
            assert!(
                (val - expected).abs() < 1e-9,
                "edt_1d: f[{}] = {}, expected {}",
                i,
                val,
                expected
            );
        }
    }

    #[test]
    fn test_distance_transform_single_point() {
        // 11x11 grid, single true cell at (5,5)
        let rows = 11;
        let cols = 11;
        let mut grid = vec![false; rows * cols];
        grid[5 * cols + 5] = true;

        let dist = distance_transform_2d(&grid, rows, cols);

        // Distance at (5,5) should be 0
        assert!(
            dist[5 * cols + 5].abs() < 1e-9,
            "Distance at source should be 0"
        );

        // Distance at (5,6) should be 1.0
        assert!(
            (dist[5 * cols + 6] - 1.0).abs() < 1e-9,
            "Distance one cell away should be 1.0, got {}",
            dist[5 * cols + 6]
        );

        // Distance at (6,6) should be sqrt(2)
        let expected_diag = std::f64::consts::SQRT_2;
        assert!(
            (dist[6 * cols + 6] - expected_diag).abs() < 1e-9,
            "Distance diagonally should be sqrt(2), got {}",
            dist[6 * cols + 6]
        );

        // Distance at (0,0) should be sqrt(50) = 5*sqrt(2)
        let expected_corner = (50.0f64).sqrt();
        assert!(
            (dist[0] - expected_corner).abs() < 1e-9,
            "Distance at corner (0,0) should be {}, got {}",
            expected_corner,
            dist[0]
        );
    }

    #[test]
    fn test_distance_transform_rectangle() {
        // 10x10 grid, 6x6 true rectangle at rows 2..8, cols 2..8
        let rows = 10;
        let cols = 10;
        let mut grid = vec![false; rows * cols];
        for r in 2..8 {
            for c in 2..8 {
                grid[r * cols + c] = true;
            }
        }

        let dist = distance_transform_2d(&grid, rows, cols);

        // All true cells should have distance 0
        for r in 2..8 {
            for c in 2..8 {
                assert!(
                    dist[r * cols + c].abs() < 1e-9,
                    "True cell ({},{}) should have distance 0, got {}",
                    r,
                    c,
                    dist[r * cols + c]
                );
            }
        }

        // Cell at (1,5) is 1 row above the rectangle — distance = 1.0
        let idx_1_5 = cols + 5;
        assert!(
            (dist[idx_1_5] - 1.0).abs() < 1e-9,
            "Cell one row above rect should have distance 1.0, got {}",
            dist[idx_1_5]
        );

        // Cell at (0,5) is 2 rows above — distance = 2.0
        assert!(
            (dist[5] - 2.0).abs() < 1e-9,
            "Cell two rows above rect should have distance 2.0, got {}",
            dist[5]
        );

        // Cell at (0,0) should be sqrt((2-0)^2 + (2-0)^2) = sqrt(8) = 2*sqrt(2)
        let expected = (8.0f64).sqrt();
        assert!(
            (dist[0] - expected).abs() < 1e-9,
            "Corner cell should have distance {}, got {}",
            expected,
            dist[0]
        );
    }
}
