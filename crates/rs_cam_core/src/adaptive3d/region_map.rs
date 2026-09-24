//! The By Area region map: the regions that `RegionOrdering::ByArea`
//! detected, as a label grid in the planner's cell frame.
//!
//! The planner detects the regions ONCE, from the stock before the first
//! level (see `clearing.rs::detect_material_regions`). This map records
//! that result for the viewport overlay and for MCP `inspect_spans`. It is
//! evidence only: no planner stage reads it.
//!
//! The region order is the planner's order. Region `k` (1-based) is the
//! `k`-th region the planner clears, and it is the `region_id` of the
//! `SpanKind::Region` span that the planner emits for it.

use super::clearing::MaterialRegion;
use crate::dexel_stock::TriDexelStock;
use crate::stock::dexel::ray_top;

/// One detected region.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct AreaRegion {
    /// The planner order, 1-based. The same number is the `region_id` of
    /// the region's span and the value of its cells in
    /// [`AreaRegionMap::labels`].
    pub order: u16,
    /// The number of grid cells with material in the region.
    pub cell_count: usize,
    /// The world XY box of the region cells: `[x_min, y_min, x_max, y_max]`.
    /// The box is cell-exact. It does not include the tool radius.
    ///
    /// F4 (known limit): the planner filters a level by this box, not by
    /// the label, so a cell of another region inside the box is cut with
    /// this region.
    pub bbox_xy: [f64; 4],
    /// The lowest and the highest surface Z under the region cells.
    pub surface_z_range: [f64; 2],
    /// The highest and the lowest planned level Z of the region. `None`
    /// when no level cuts the region.
    pub level_z_range: Option<[f64; 2]>,
    /// The number of planned levels of the region.
    pub level_count: usize,
    /// A world XY point inside the region for its order label: the region
    /// cell that is nearest to the region centroid.
    pub anchor_xy: [f64; 2],
}

/// The label grid and the region list of one By Area run.
///
/// Frame contract: emission-frame coordinates, the same frame as the
/// toolpath moves. `AnnotatedToolpath::translated` shifts it with the
/// moves.
#[derive(Debug, Clone, PartialEq)]
pub struct AreaRegionMap {
    /// The world X of the centre of cell column 0.
    pub origin_x: f64,
    /// The world Y of the centre of cell row 0.
    pub origin_y: f64,
    pub cell_mm: f64,
    pub rows: usize,
    pub cols: usize,
    /// Row-major, `rows * cols` values. `0` is a cell in no region; `k` is
    /// a cell of the region with order `k`.
    pub labels: Vec<u16>,
    /// The highest material top over the region cells at detection. The
    /// overlay draws at this Z.
    pub top_z: f64,
    /// The regions in planner order.
    pub regions: Vec<AreaRegion>,
}

/// One run of cells in a grid row that have the same region label.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AreaRegionRun {
    pub row: usize,
    /// The first column of the run.
    pub col_start: usize,
    /// One past the last column of the run.
    pub col_end: usize,
    pub order: u16,
}

impl AreaRegionMap {
    /// Build the map from the BFS labels of `detect_material_regions`.
    ///
    /// `bfs_labels` holds the flood-fill id of each cell; `regions` holds
    /// the kept regions in planner order, each with its flood-fill id. A
    /// cell whose id belongs to no kept region gets label 0.
    pub(super) fn from_detection(
        stock: &TriDexelStock,
        bfs_labels: &[usize],
        regions: &[MaterialRegion],
    ) -> Self {
        let grid = &stock.z_grid;
        let (rows, cols, cell_mm) = (grid.rows, grid.cols, grid.cell_size);
        let (origin_x, origin_y) = (grid.origin_u, grid.origin_v);

        // Flood-fill id -> planner order.
        let order_of = |bfs: usize| -> u16 {
            regions
                .iter()
                .position(|r| r.bfs_label == bfs)
                .map_or(0, |i| u16::try_from(i + 1).unwrap_or(u16::MAX))
        };
        let mut labels = vec![0u16; rows * cols];
        let mut top_z = f64::NEG_INFINITY;
        let mut sums = vec![(0.0f64, 0.0f64); regions.len()];
        for row in 0..rows {
            for col in 0..cols {
                let i = row * cols + col;
                let Some(&bfs) = bfs_labels.get(i) else {
                    continue;
                };
                let order = order_of(bfs);
                if order == 0 {
                    continue;
                }
                if let Some(slot) = labels.get_mut(i) {
                    *slot = order;
                }
                if let Some(top) = ray_top(grid.ray(row, col)) {
                    top_z = top_z.max(f64::from(top));
                }
                if let Some(s) = sums.get_mut(usize::from(order) - 1) {
                    s.0 += col as f64;
                    s.1 += row as f64;
                }
            }
        }
        if !top_z.is_finite() {
            top_z = 0.0;
        }

        let half = cell_mm * 0.5;
        let regions = regions
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let order = u16::try_from(i + 1).unwrap_or(u16::MAX);
                let n = r.cell_count.max(1) as f64;
                let (sc, sr) = sums.get(i).copied().unwrap_or((0.0, 0.0));
                let (cc, cr) = (sc / n, sr / n);
                let (ac, ar) = nearest_cell(&labels, cols, r, order, cc, cr);
                AreaRegion {
                    order,
                    cell_count: r.cell_count,
                    bbox_xy: [
                        origin_x + r.col_min as f64 * cell_mm - half,
                        origin_y + r.row_min as f64 * cell_mm - half,
                        origin_x + r.col_max as f64 * cell_mm + half,
                        origin_y + r.row_max as f64 * cell_mm + half,
                    ],
                    surface_z_range: [r.surface_z_min, r.surface_z_max],
                    level_z_range: None,
                    level_count: 0,
                    anchor_xy: [
                        origin_x + ac as f64 * cell_mm,
                        origin_y + ar as f64 * cell_mm,
                    ],
                }
            })
            .collect();

        Self {
            origin_x,
            origin_y,
            cell_mm,
            rows,
            cols,
            labels,
            top_z,
            regions,
        }
    }

    /// Record the planned levels of the region with `order`.
    pub(super) fn set_levels(&mut self, order: u16, level_z: &[f64]) {
        if let Some(region) = self.regions.iter_mut().find(|r| r.order == order) {
            region.level_count = level_z.len();
            region.level_z_range = match (level_z.first(), level_z.last()) {
                (Some(top), Some(bottom)) => Some([*top, *bottom]),
                _ => None,
            };
        }
    }

    /// The label of the cell at `(row, col)`; 0 outside the grid.
    pub fn label_at(&self, row: usize, col: usize) -> u16 {
        if row >= self.rows || col >= self.cols {
            return 0;
        }
        self.labels.get(row * self.cols + col).copied().unwrap_or(0)
    }

    /// The runs of same-label cells, row by row. Cells with label 0 make
    /// no run.
    pub fn row_runs(&self) -> Vec<AreaRegionRun> {
        let mut runs = Vec::new();
        for row in 0..self.rows {
            let mut col = 0;
            while col < self.cols {
                let order = self.label_at(row, col);
                let start = col;
                while col < self.cols && self.label_at(row, col) == order {
                    col += 1;
                }
                if order != 0 {
                    runs.push(AreaRegionRun {
                        row,
                        col_start: start,
                        col_end: col,
                        order,
                    });
                }
            }
        }
        runs
    }

    /// The world XY rectangle `[x0, y0, x1, y1]` of a run.
    pub fn run_rect(&self, run: &AreaRegionRun) -> [f64; 4] {
        let half = self.cell_mm * 0.5;
        [
            self.origin_x + run.col_start as f64 * self.cell_mm - half,
            self.origin_y + run.row as f64 * self.cell_mm - half,
            self.origin_x + run.col_end as f64 * self.cell_mm - half,
            self.origin_y + run.row as f64 * self.cell_mm + half,
        ]
    }

    /// Shift every coordinate by `(dx, dy, dz)`.
    pub fn translate(&mut self, dx: f64, dy: f64, dz: f64) {
        self.origin_x += dx;
        self.origin_y += dy;
        self.top_z += dz;
        for r in &mut self.regions {
            r.bbox_xy[0] += dx;
            r.bbox_xy[1] += dy;
            r.bbox_xy[2] += dx;
            r.bbox_xy[3] += dy;
            r.anchor_xy[0] += dx;
            r.anchor_xy[1] += dy;
            r.surface_z_range[0] += dz;
            r.surface_z_range[1] += dz;
            if let Some(z) = r.level_z_range.as_mut() {
                z[0] += dz;
                z[1] += dz;
            }
        }
    }
}

/// The cell of region `order` that is nearest to `(cc, cr)`, in grid
/// units. The search stays inside the region's row/col box.
fn nearest_cell(
    labels: &[u16],
    cols: usize,
    r: &MaterialRegion,
    order: u16,
    cc: f64,
    cr: f64,
) -> (usize, usize) {
    let mut best = (r.col_min, r.row_min);
    let mut best_d = f64::INFINITY;
    for row in r.row_min..=r.row_max {
        for col in r.col_min..=r.col_max {
            if labels.get(row * cols + col).copied() != Some(order) {
                continue;
            }
            let d = (col as f64 - cc).powi(2) + (row as f64 - cr).powi(2);
            if d < best_d {
                best_d = d;
                best = (col, row);
            }
        }
    }
    best
}
