//! The By Area region map: the jobs that `RegionOrdering::ByArea` ran,
//! as a label grid in the planner's cell frame.
//!
//! The planner builds the pocket tree ONCE, before the first level, and
//! makes its jobs from it (`area_plan.rs`): one job per valley and one rest
//! job. This map records those jobs for the viewport overlay and for MCP
//! `inspect_spans`. It is evidence only: no planner stage reads it.
//!
//! The region order is the planner's order. Region `k` (1-based) is the
//! `k`-th job the planner runs, and it is the `region_id` of the
//! `SpanKind::Region` span that the planner emits for it. Nearest next fixes
//! the valley order only at run time, so the map is built after the jobs
//! run.

use super::area_plan::{AreaPlan, JobKind};
use crate::dexel_stock::TriDexelStock;

/// What a region is: a valley of the pocket tree, or the rest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AreaRegionKind {
    /// Every cell that no valley owns: the high ground, the bands between
    /// the valleys and the stock beside the model.
    Rest,
    /// A pocket of the tree with no children.
    Valley,
}

impl AreaRegionKind {
    /// The lower-case name, as MCP writes it.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Rest => "rest",
            Self::Valley => "valley",
        }
    }
}

/// One job of a By Area run.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct AreaRegion {
    /// The planner order, 1-based. The same number is the `region_id` of
    /// the region's span and the value of its cells in
    /// [`AreaRegionMap::labels`].
    pub order: u16,
    /// A valley or the rest.
    pub kind: AreaRegionKind,
    /// The number of grid cells with material in the region.
    pub cell_count: usize,
    /// The world XY box of the region cells: `[x_min, y_min, x_max, y_max]`.
    /// The box is cell-exact. It does not include the tool radius.
    ///
    /// The box is evidence only. The planner confines the region to its
    /// labelled cells (`clearing.rs::AreaMask`), not to this box.
    pub bbox_xy: [f64; 4],
    /// The lowest and the highest tool-CL Z under the region cells.
    pub surface_z_range: [f64; 2],
    /// The highest and the lowest planned level Z of the region. `None`
    /// when no level cuts the region.
    pub level_z_range: Option<[f64; 2]>,
    /// The number of planned levels of the region.
    pub level_count: usize,
    /// A world XY point inside the region for its order label: the region
    /// cell that is nearest to the region centroid. For a valley it is also
    /// the anchor that the nearest-next order reads.
    pub anchor_xy: [f64; 2],
    /// The Z where the valley joins its neighbour (its rim). `None` for the
    /// rest.
    pub saddle_z: Option<f64>,
    /// A valley: its depth below the saddle. The rest: the level top minus
    /// its lowest tool-CL Z.
    pub depth_mm: f64,
    /// A valley: the area of its tree cells. The rest: the area of its
    /// labelled cells.
    pub area_mm2: f64,
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
    /// The highest material top over the region cells before the first
    /// job. The overlay draws at this Z.
    pub top_z: f64,
    /// The regions in planner order.
    pub regions: Vec<AreaRegion>,
    /// True when the rest job ran before the valleys (or there is no
    /// valley); false when it ran after them.
    pub rest_first: bool,
    /// The tree dial that was used: a valley less deep than this below its
    /// rim joins its neighbour (mm).
    pub pocket_min_depth_mm: f64,
    /// The tree dial that was used: a valley smaller than this at its rim
    /// joins its neighbour (mm²).
    pub pocket_min_area_mm2: f64,
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
    /// Build the map from the plan and the jobs that ran, in cut order:
    /// `ran[k]` is the job of order `k + 1`, with its levels.
    ///
    /// A valley cell gets the valley's order. Every other cell with material
    /// before the first job gets the rest's order (PLAN §6.1).
    pub(super) fn from_plan(
        stock: &TriDexelStock,
        plan: &AreaPlan,
        ran: &[(JobKind, Vec<f64>)],
    ) -> Self {
        let grid = &stock.z_grid;
        let (rows, cols, cell_mm) = (grid.rows, grid.cols, grid.cell_size);
        let (origin_x, origin_y) = (grid.origin_u, grid.origin_v);
        let order_of = |job: JobKind| -> u16 {
            ran.iter()
                .position(|(j, _)| *j == job)
                .map_or(0, |k| u16::try_from(k + 1).unwrap_or(u16::MAX))
        };
        let labels: Vec<u16> = plan
            .cell_job
            .iter()
            .map(|job| job.map_or(0, order_of))
            .collect();

        let half = cell_mm * 0.5;
        let cell_area = cell_mm * cell_mm;
        let regions = ran
            .iter()
            .enumerate()
            .map(|(k, (job, levels))| {
                let order = u16::try_from(k + 1).unwrap_or(u16::MAX);
                let stats = RegionCells::of(&labels, &plan.cl_z, cols, order);
                let (ac, ar) = stats.nearest_to_centroid(&labels, cols, order);
                let (kind, saddle_z, depth_mm, area_mm2, anchor_xy) = match job {
                    JobKind::Valley(v) => {
                        let valley = plan.valleys.get(*v);
                        (
                            AreaRegionKind::Valley,
                            valley.map(|x| x.saddle_z),
                            valley.map_or(0.0, |x| x.depth_mm),
                            valley.map_or(0.0, |x| x.area_mm2),
                            valley.map_or(
                                [
                                    origin_x + ac as f64 * cell_mm,
                                    origin_y + ar as f64 * cell_mm,
                                ],
                                |x| [x.anchor.x, x.anchor.y],
                            ),
                        )
                    }
                    JobKind::Rest => (
                        AreaRegionKind::Rest,
                        None,
                        if stats.count > 0 {
                            plan.level_top - stats.z_min
                        } else {
                            0.0
                        },
                        stats.count as f64 * cell_area,
                        [
                            origin_x + ac as f64 * cell_mm,
                            origin_y + ar as f64 * cell_mm,
                        ],
                    ),
                };
                AreaRegion {
                    order,
                    kind,
                    cell_count: stats.count,
                    bbox_xy: [
                        origin_x + stats.col_min as f64 * cell_mm - half,
                        origin_y + stats.row_min as f64 * cell_mm - half,
                        origin_x + stats.col_max as f64 * cell_mm + half,
                        origin_y + stats.row_max as f64 * cell_mm + half,
                    ],
                    surface_z_range: if stats.count > 0 {
                        [stats.z_min, stats.z_max]
                    } else {
                        [0.0, 0.0]
                    },
                    level_z_range: match (levels.first(), levels.last()) {
                        (Some(top), Some(bottom)) => Some([*top, *bottom]),
                        _ => None,
                    },
                    level_count: levels.len(),
                    anchor_xy,
                    saddle_z,
                    depth_mm,
                    area_mm2,
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
            top_z: plan.top_material_z,
            regions,
            rest_first: plan.rest_first(),
            pocket_min_depth_mm: plan.params.persistence_h_mm,
            pocket_min_area_mm2: plan.params.min_area_mm2,
        }
    }

    /// The region order (label) of the cell at `(row, col)`; 0 outside the
    /// grid. It is not named `label_at`, because that name is the gated
    /// `TierMap` test door (`the_test_doors_are_gated_fld0405`).
    pub fn region_order_at(&self, row: usize, col: usize) -> u16 {
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
                let order = self.region_order_at(row, col);
                let start = col;
                while col < self.cols && self.region_order_at(row, col) == order {
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
            if let Some(z) = r.saddle_z.as_mut() {
                *z += dz;
            }
        }
    }
}

/// The cells of one region in the label grid.
struct RegionCells {
    count: usize,
    row_min: usize,
    row_max: usize,
    col_min: usize,
    col_max: usize,
    /// The sums of the rows and the columns, for the centroid.
    row_sum: f64,
    col_sum: f64,
    z_min: f64,
    z_max: f64,
}

impl RegionCells {
    fn of(labels: &[u16], cl_z: &[f64], cols: usize, order: u16) -> Self {
        let mut s = Self {
            count: 0,
            row_min: usize::MAX,
            row_max: 0,
            col_min: usize::MAX,
            col_max: 0,
            row_sum: 0.0,
            col_sum: 0.0,
            z_min: f64::INFINITY,
            z_max: f64::NEG_INFINITY,
        };
        for (i, _) in labels.iter().enumerate().filter(|&(_, &l)| l == order) {
            let (row, col) = (i / cols.max(1), i % cols.max(1));
            s.count += 1;
            s.row_min = s.row_min.min(row);
            s.row_max = s.row_max.max(row);
            s.col_min = s.col_min.min(col);
            s.col_max = s.col_max.max(col);
            s.row_sum += row as f64;
            s.col_sum += col as f64;
            if let Some(&z) = cl_z.get(i) {
                s.z_min = s.z_min.min(z);
                s.z_max = s.z_max.max(z);
            }
        }
        if s.count == 0 {
            (s.row_min, s.col_min) = (0, 0);
        }
        s
    }

    /// The cell of region `order` nearest to the centroid of its cells, as
    /// `(col, row)`. The search reads the label grid inside the region box.
    fn nearest_to_centroid(&self, labels: &[u16], cols: usize, order: u16) -> (usize, usize) {
        let n = self.count.max(1) as f64;
        let (cc, cr) = (self.col_sum / n, self.row_sum / n);
        let mut best = (self.col_min, self.row_min);
        let mut best_d = f64::INFINITY;
        for row in self.row_min..=self.row_max {
            for col in self.col_min..=self.col_max {
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
}
