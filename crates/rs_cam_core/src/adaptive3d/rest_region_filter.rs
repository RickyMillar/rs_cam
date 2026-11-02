//! The rest region filter ([`super::RestRegionFilter`]): the ONE place that
//! removes the regions of a level that are too large for a rest rough.
//!
//! `clearing.rs::build_material_bool_grid` calls [`skip_large_regions`] on
//! each material grid it builds. Every strategy and the Contour Parallel
//! cleanup read that grid, so a skipped region gets no cut at that level.

use super::RestRegionFilter;

/// Remove from `grid` each connected region of material that fails a bound
/// of `filter`, and return the number of regions that it removed.
///
/// `grid` is a row-major material grid of `rows` x `cols` cells of side
/// `cell_size` (mm). A region is a 4-connected set of `true` cells.
///
/// - The area of a region is its cell count times `cell_size²`.
/// - The width of a region is the diameter of the largest circle inside
///   it. The distance transform gives the distance `d` (in cells) from a
///   material cell centre to the nearest air cell centre. The edge of the
///   material is half a cell nearer, so the radius is `d - 0.5` cells and
///   the width is `(2 d_max - 1)` cells, at least one cell. The measure is
///   exact for an odd cell count and one cell low for an even count, so it
///   never reads a region wider than it is.
///
/// A region that fails is removed whole; the filter never clips a region.
#[expect(
    clippy::indexing_slicing,
    reason = "every index is below rows * cols = grid.len(), checked at the top"
)]
pub(super) fn skip_large_regions(
    grid: &mut [bool],
    rows: usize,
    cols: usize,
    cell_size: f64,
    filter: &RestRegionFilter,
) -> usize {
    if filter.max_area_mm2.is_none() && filter.max_width_mm.is_none() {
        return 0;
    }
    if grid.len() != rows * cols {
        return 0;
    }
    let edt = filter.max_width_mm.map(|_| {
        let air: Vec<bool> = grid.iter().map(|&m| !m).collect();
        crate::geometry::grid_field::distance_transform_2d(&air, rows, cols)
    });
    let cell_area = cell_size * cell_size;
    let mut seen = vec![false; grid.len()];
    let mut region: Vec<usize> = Vec::new();
    let mut stack: Vec<usize> = Vec::new();
    let mut skipped = 0usize;
    for start in 0..grid.len() {
        if !grid[start] || seen[start] {
            continue;
        }
        // Flood fill one region (4-connected).
        region.clear();
        stack.push(start);
        seen[start] = true;
        let mut max_d = 0.0_f64;
        while let Some(i) = stack.pop() {
            region.push(i);
            if let Some(d) = edt.as_ref() {
                max_d = max_d.max(d[i]);
            }
            let (r, c) = (i / cols, i % cols);
            let mut visit = |j: usize| {
                if grid[j] && !seen[j] {
                    seen[j] = true;
                    stack.push(j);
                }
            };
            if r > 0 {
                visit(i - cols);
            }
            if r + 1 < rows {
                visit(i + cols);
            }
            if c > 0 {
                visit(i - 1);
            }
            if c + 1 < cols {
                visit(i + 1);
            }
        }
        let area = region.len() as f64 * cell_area;
        let width = (2.0 * max_d - 1.0).max(1.0) * cell_size;
        let too_broad = filter.max_area_mm2.is_some_and(|a| area > a)
            || filter.max_width_mm.is_some_and(|w| width > w);
        if too_broad {
            for &i in &region {
                grid[i] = false;
            }
            skipped += 1;
        }
    }
    skipped
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A grid with a 20 x 20 block and a 4 x 20 strip, 1 mm cells.
    #[expect(clippy::indexing_slicing, reason = "indices are inside the grid")]
    fn two_regions() -> (Vec<bool>, usize, usize) {
        let (rows, cols) = (30, 40);
        let mut grid = vec![false; rows * cols];
        for r in 5..25 {
            for c in 2..22 {
                grid[r * cols + c] = true;
            }
            for c in 30..34 {
                grid[r * cols + c] = true;
            }
        }
        (grid, rows, cols)
    }

    fn cells_in(grid: &[bool], cols: usize, c_lo: usize, c_hi: usize) -> usize {
        grid.iter()
            .enumerate()
            .filter(|(i, m)| **m && (c_lo..c_hi).contains(&(i % cols)))
            .count()
    }

    #[test]
    fn the_width_bound_keeps_only_the_narrow_region_rest_filter() {
        let (mut grid, rows, cols) = two_regions();
        let filter = RestRegionFilter {
            max_area_mm2: None,
            max_width_mm: Some(8.0),
        };
        assert_eq!(skip_large_regions(&mut grid, rows, cols, 1.0, &filter), 1);
        assert_eq!(cells_in(&grid, cols, 0, 25), 0);
        assert_eq!(cells_in(&grid, cols, 25, 40), 80);
    }

    #[test]
    fn the_area_bound_keeps_only_the_small_region_rest_filter() {
        let (mut grid, rows, cols) = two_regions();
        let filter = RestRegionFilter {
            max_area_mm2: Some(100.0),
            max_width_mm: None,
        };
        assert_eq!(skip_large_regions(&mut grid, rows, cols, 1.0, &filter), 1);
        assert_eq!(cells_in(&grid, cols, 0, 25), 0);
        assert_eq!(cells_in(&grid, cols, 25, 40), 80);
    }

    #[test]
    fn the_width_of_a_strip_reads_within_one_cell_rest_filter() {
        // A 4-cell strip reads 3 cells wide (one cell low for an even
        // count): it passes a bound of 3 and fails a bound of 2.9.
        let (grid, rows, cols) = two_regions();
        let mut strip_only = grid;
        for (i, m) in strip_only.iter_mut().enumerate() {
            if i % cols < 25 {
                *m = false;
            }
        }
        let mut pass = strip_only.clone();
        let f3 = RestRegionFilter {
            max_area_mm2: None,
            max_width_mm: Some(3.0),
        };
        assert_eq!(skip_large_regions(&mut pass, rows, cols, 1.0, &f3), 0);
        let f29 = RestRegionFilter {
            max_area_mm2: None,
            max_width_mm: Some(2.9),
        };
        assert_eq!(
            skip_large_regions(&mut strip_only, rows, cols, 1.0, &f29),
            1
        );
    }
}
