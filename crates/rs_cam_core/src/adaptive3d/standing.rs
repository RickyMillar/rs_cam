//! P44: the ONE read of the stock that stands where the tool fits.
//!
//! A cell of the level grid is a tool-centre position. The level grid
//! (`clearing.rs::build_material_bool_grid`) reads the stock only at the
//! centre cell. So stock in a strip beside a wall makes no region: the tool
//! centre cannot stand over it, and a centre up to one tool radius away
//! holds no stock at its own cell. [`standing_cells`] reads both:
//!
//! - a material cell of the level (`cell_has_material`), and
//! - with `strips`, a strip cell: the cell holds no material, and the tool
//!   disc at the cell covers in full a cell whose stock stands more than
//!   one planner cell above the cutter. This is the strip read of the wall
//!   pass (`level_contour::wall_read`, pass 2). The part under the disc is
//!   at or under the floor of the centre (the floor is the drop-cutter
//!   height), so that stock is wood. A coarse planner cell cannot read a
//!   strip, so the caller sets `strips` only on remaining stock.
//!
//! Two callers: the strip pass of an op on remaining stock
//! (`level_contour::strip_pass`, P45), which cuts the strip cells, and the
//! read of the places that stand at the end of the plan (`left_stock`).

use crate::dexel_stock::TriDexelStock;
use crate::surface::slope::SurfaceHeightmap;

use super::clearing::{ClearZLevelContext, cell_has_material};
use super::intended_floor::level_floor_z;
use super::stock_top_z_at;

/// The cells of [`standing_cells`], as a padded grid (one cell of `false`
/// border, the layout of `build_material_bool_grid`).
pub(super) struct Standing {
    pub padded: Vec<bool>,
    pub padded_rows: usize,
    pub padded_cols: usize,
    /// The stock above the cutter at each standing cell (mm; not padded):
    /// the stock at a material cell above its floor, or the tallest stock
    /// that the disc of a strip cell covers in full above its floor. 0
    /// elsewhere.
    pub height: Vec<f32>,
    /// The number of standing cells.
    pub count: usize,
}

impl Standing {
    pub(super) fn at(&self, row: usize, col: usize) -> bool {
        self.padded
            .get((row + 1) * self.padded_cols + (col + 1))
            .copied()
            .unwrap_or(false)
    }
}

/// The highest value of `values` in the window of `half` cells each side,
/// along one row of length `n` and stride `stride` from `start` (a sliding
/// maximum; the cost is linear in `n`).
#[expect(
    clippy::indexing_slicing,
    reason = "each index is start + i * stride with i < n, inside the slice by the caller's layout"
)]
fn sliding_max(
    values: &[f32],
    out: &mut [f32],
    start: usize,
    stride: usize,
    n: usize,
    half: usize,
) {
    let mut queue: std::collections::VecDeque<usize> = std::collections::VecDeque::new();
    let at = |i: usize| values[start + i * stride];
    let mut next = 0usize;
    for i in 0..n {
        let hi = (i + half).min(n - 1);
        while next <= hi {
            while queue.back().is_some_and(|&b| at(b) <= at(next)) {
                queue.pop_back();
            }
            queue.push_back(next);
            next += 1;
        }
        while queue.front().is_some_and(|&f| f + half < i) {
            queue.pop_front();
        }
        out[start + i * stride] = queue.front().map_or(f32::NEG_INFINITY, |&f| at(f));
    }
}

/// The cells where the tool fits and stock stands at `z_level`, inside the
/// boundary of the op. See the module text.
#[expect(
    clippy::indexing_slicing,
    reason = "each index is row * cols + col with row < rows and col < cols"
)]
pub(super) fn standing_cells(
    ctx: &ClearZLevelContext<'_>,
    stock: &TriDexelStock,
    surface_hm: &SurfaceHeightmap,
    z_level: f64,
    strips: bool,
) -> Standing {
    let grid = &stock.z_grid;
    let (rows, cols, cs) = (grid.rows, grid.cols, grid.cell_size);
    let (padded_rows, padded_cols) = (rows + 2, cols + 2);
    let mut padded = vec![false; padded_rows * padded_cols];
    let mut height = vec![0.0f32; rows * cols];
    if rows == 0 || cols == 0 || cs <= 0.0 {
        return Standing {
            padded,
            padded_rows,
            padded_cols,
            height,
            count: 0,
        };
    }
    for row in 0..rows {
        for col in 0..cols {
            if cell_has_material(stock, surface_hm, row, col, z_level, ctx.stock_to_leave) {
                padded[(row + 1) * padded_cols + (col + 1)] = true;
                let floor = level_floor_z(
                    surface_hm.z_or_bbox_floor_at(row, col),
                    ctx.stock_to_leave,
                    z_level,
                );
                #[expect(clippy::cast_possible_truncation, reason = "a stock height in mm")]
                {
                    height[row * cols + col] = (stock_top_z_at(stock, row, col) - floor) as f32;
                }
            }
        }
    }
    if strips {
        let r = ctx.tool_radius;
        let r_sq = r * r;
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a radius over a cell size, positive and small"
        )]
        let half = (r / cs).floor().max(0.0) as usize;
        // An upper bound of the stock in the disc: the highest top in the
        // square of the disc (two sliding maxima). Only a cell that passes
        // this bound takes the exact read below.
        let tops: Vec<f32> = (0..rows * cols)
            .map(|i| {
                #[expect(clippy::cast_possible_truncation, reason = "a stock height in mm")]
                {
                    stock_top_z_at(stock, i / cols, i % cols) as f32
                }
            })
            .collect();
        let mut by_row = vec![f32::NEG_INFINITY; rows * cols];
        for row in 0..rows {
            sliding_max(&tops, &mut by_row, row * cols, 1, cols, half);
        }
        let mut bound = vec![f32::NEG_INFINITY; rows * cols];
        for col in 0..cols {
            sliding_max(&by_row, &mut bound, col, cols, rows, half);
        }
        #[expect(
            clippy::cast_possible_wrap,
            reason = "a cell count under the grid size"
        )]
        let reach = half as isize;
        // The offsets of the cells that the disc covers in full, and the
        // profile height of the cutter over each (the coverage of the
        // stamp, read once).
        let side = 2 * half + 1;
        let mut full: Vec<Option<f64>> = vec![None; side * side];
        for (i, slot) in full.iter_mut().enumerate() {
            let dv = ((i / side) as f64 - half as f64) * cs;
            let du = ((i % side) as f64 - half as f64) * cs;
            if TriDexelStock::disc_cell_coverage(du, dv, r_sq, cs) >= 1.0 {
                *slot = Some(
                    ctx.lut
                        .height_at_dist_sq((du * du + dv * dv).min(ctx.lut.radius_sq()))
                        .unwrap_or(0.0),
                );
            }
        }
        for row in 0..rows {
            for col in 0..cols {
                if padded[(row + 1) * padded_cols + (col + 1)] {
                    continue;
                }
                let floor = level_floor_z(
                    surface_hm.z_or_bbox_floor_at(row, col),
                    ctx.stock_to_leave,
                    z_level,
                );
                if f64::from(bound[row * cols + col]) <= floor + cs {
                    continue;
                }
                // The exact read: a cell that the disc covers in full, with
                // stock more than one planner cell above the cutter surface
                // over it.
                let mut tallest = f64::NEG_INFINITY;
                for dr in -reach..=reach {
                    let Some(r2) = row.checked_add_signed(dr).filter(|&v| v < rows) else {
                        continue;
                    };
                    for dc in -reach..=reach {
                        let Some(c2) = col.checked_add_signed(dc).filter(|&v| v < cols) else {
                            continue;
                        };
                        let top = f64::from(tops[r2 * cols + c2]);
                        if top <= floor + cs {
                            continue;
                        }
                        let k = (dr + reach).unsigned_abs() * side + (dc + reach).unsigned_abs();
                        if let Some(profile) = full[k] {
                            tallest = tallest.max(top - profile);
                        }
                    }
                }
                if tallest > floor + cs {
                    padded[(row + 1) * padded_cols + (col + 1)] = true;
                    #[expect(clippy::cast_possible_truncation, reason = "a stock height in mm")]
                    {
                        height[row * cols + col] = (tallest - floor) as f32;
                    }
                }
            }
        }
    }
    // The boundary of the op: the region clip of the level grid, and the
    // containment of the tool centre.
    if let Some(clip) = ctx.region_clip.as_ref() {
        clip.restrict_regions(&mut padded, padded_rows, padded_cols);
    }
    for row in 0..rows {
        for col in 0..cols {
            let i = (row + 1) * padded_cols + (col + 1);
            if !padded[i] {
                height[row * cols + col] = 0.0;
                continue;
            }
            let (x, y) = grid.cell_to_world(row, col);
            if ctx.centre_clip.is_some_and(|clip| !clip.contains(x, y)) {
                padded[i] = false;
                height[row * cols + col] = 0.0;
            }
        }
    }
    let count = padded.iter().filter(|&&c| c).count();
    Standing {
        padded,
        padded_rows,
        padded_cols,
        height,
        count,
    }
}

#[cfg(test)]
mod tests {
    use super::sliding_max;

    /// The sliding maximum is the maximum of each window, at the ends too.
    #[test]
    fn the_sliding_maximum_reads_each_window_p44() {
        let values = [1.0_f32, 5.0, 2.0, 0.0, 0.0, 0.0, 3.0, 0.0];
        let mut out = [0.0_f32; 8];
        sliding_max(&values, &mut out, 0, 1, 8, 2);
        assert_eq!(out, [5.0, 5.0, 5.0, 5.0, 3.0, 3.0, 3.0, 3.0]);
        // A column of a 2-column layout (stride 2).
        let mut col = [0.0_f32; 8];
        sliding_max(&values, &mut col, 1, 2, 4, 1);
        assert_eq!([col[1], col[3], col[5], col[7]], [5.0, 5.0, 0.0, 0.0]);
    }
}
