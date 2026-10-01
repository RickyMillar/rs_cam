//! By Area: the pocket tree of the tool-CL grid becomes the planner's jobs.
//!
//! `planning/by_area_merge_tree_2026-09-25/PLAN.md` (WP2). The planner
//! builds the join tree ([`crate::surface::merge_tree`]) once per operation,
//! from its own tool-CL grid (`surface_hm`, on `material_stock.z_grid`), so
//! a tree label is a planner cell and no resampling is needed (PLAN §1).
//!
//! # The jobs (PLAN §2)
//!
//! - A **valley** job: each pocket with no children, also a root with no
//!   children ([`PocketTree::valleys`]).
//! - The **rest** job: every cell that no valley owns at that level. This
//!   holds the internal bands, the root, the cells at or above the top and
//!   the masked cells with no label (the F-027 stock beside the model).
//! - A tree with one pocket has no split, so it has no valley. The one job
//!   is the rest, and By Area cuts as Global does (sentry S1).
//!
//! # Ownership (PLAN §3.2)
//!
//! A per-cell [`AreaMask`] confines each job. At every level the jobs
//! partition the grid, so every cell gets the Global level sequence; only
//! the order of the (job, level) pairs changes (sentry S3).
//!
//! - [`AreaOrder::RestFirst`]: valley `L` owns the cells labelled `L` at a
//!   level below its saddle. At a level at or above the saddle, the rest
//!   owns those cells. So the rest cuts the high ground down to the saddle
//!   first, and each valley starts from an open rim (PLAN §4.3).
//! - [`AreaOrder::RestLast`]: valley `L` owns its cells at every level. The
//!   rest owns every cell (Global on the remaining stock, PLAN §4.4).
//!
//! # The order (PLAN §4)
//!
//! Depth first per job. After each job, the next valley is the one whose
//! anchor is nearest to the tool; a tie goes to the lower `min_z`, then to
//! the lower valley id. The order is known only at run time, so the region
//! map is built after the jobs run ([`super::region_map::AreaRegionMap::from_plan`]).

use std::f64::consts::PI;

use super::clearing::AreaMask;
use crate::dexel_stock::TriDexelStock;
use crate::geo::{P2, P3};
use crate::polygon::Polygon2;
use crate::surface::flow_accum::FlowField;
use crate::surface::merge_tree::{MergeTreeParams, PocketTree, build_pocket_tree};
use crate::surface::slope::SurfaceHeightmap;

/// The minimum valley depth as a fraction of the tool diameter.
///
/// Operator ruling 2026-09-26 (`planning/PROGRESS.md`, By Area defaults):
/// the minimum depth is a percentage of the tool diameter, not a fixed
/// 2 mm. No printed source gives the percentage, so it is a named repo
/// rule: 1/3 x D. On the Ø6 reference tool it gives the 2 mm that
/// `RESULTS.md` proposed (6 / 3 = 2).
pub(super) const POCKET_MIN_DEPTH_PER_TOOL_DIAMETER: f64 = 1.0 / 3.0;

/// The minimum valley area as a multiple of the tool disc area `π (D/2)²`.
///
/// The operator has not confirmed this rule yet (`planning/PROGRESS.md`,
/// By Area defaults, 2026-09-26). Until then the multiple gives the
/// 400 mm² of `RESULTS.md` on the Ø6 reference tool:
/// 400 / (π · 3²) = 400 / 28.274 = 14.147 tool discs.
pub(super) const POCKET_MIN_AREA_TOOL_DISCS: f64 = 400.0 / (PI * 3.0 * 3.0);

/// The tree dials for a tool of diameter `tool_diameter` (mm).
pub(super) fn pocket_params(tool_diameter: f64) -> MergeTreeParams {
    let r = tool_diameter * 0.5;
    MergeTreeParams {
        persistence_h_mm: POCKET_MIN_DEPTH_PER_TOOL_DIAMETER * tool_diameter,
        min_area_mm2: POCKET_MIN_AREA_TOOL_DISCS * PI * r * r,
    }
}

/// Where the rest job goes in the cut order (PLAN §4.3-§4.5).
///
/// Private, with no config surface: WP5 measures both orders and WP6
/// deletes the slower one. The product order is [`AREA_ORDER`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AreaOrder {
    /// The rest first, then the valleys nearest next. The waterline cleanup
    /// runs once, at the bottom Z, after every job (F9).
    RestFirst,
    /// The valleys nearest next, then the rest with a waterline cleanup per
    /// level, as Global runs it.
    RestLast,
    /// XEXP (temporary, WP5 cause measurement): the valleys nearest next,
    /// then the rest masked to the cells of no valley, one cleanup at the end.
    RestLastMasked,
}

/// XEXP (temporary): `XEXP_AREA_ORDER` = `rest_first` | `rest_last` |
/// `rest_last_masked` overrides [`AREA_ORDER`] for the cause measurement.
pub(super) fn area_order_xexp() -> AreaOrder {
    match std::env::var("XEXP_AREA_ORDER").as_deref() {
        Ok("rest_first") => AreaOrder::RestFirst,
        Ok("rest_last") => AreaOrder::RestLast,
        Ok("rest_last_masked") => AreaOrder::RestLastMasked,
        _ => AREA_ORDER,
    }
}

/// The By Area order of the product (PLAN §4.3: rest first is the
/// default). A WP5 build flips it to measure the other arm.
pub(super) const AREA_ORDER: AreaOrder = AreaOrder::RestFirst;

/// One job of the plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum JobKind {
    Rest,
    /// An index into [`AreaPlan::valleys`].
    Valley(usize),
}

/// One valley of the tree, as the planner reads it.
#[derive(Debug, Clone)]
pub(super) struct Valley {
    /// The pocket id in the tree.
    pub(super) pocket: usize,
    pub(super) min_z: f64,
    pub(super) saddle_z: f64,
    pub(super) depth_mm: f64,
    pub(super) area_mm2: f64,
    /// The valley cell nearest to the centroid of its cells (world XY).
    pub(super) anchor: P2,
}

/// The inputs of [`AreaPlan::build`] that are not the stock and the grid.
pub(super) struct AreaPlanInputs<'a> {
    /// The level top (`path.rs::level_anchor_z`): cells at or above it have
    /// no material and get no pocket (PLAN §1.3).
    pub(super) top_z: f64,
    /// The pinned floor (`Adaptive3dDepth::z_floor`): a CL below it reads it.
    pub(super) z_floor: Option<f64>,
    /// The boundary polygon (`Adaptive3dGeometry::boundary`). A cell whose
    /// centre is outside it is masked, the same test as the boundary clear.
    pub(super) boundary: Option<&'a Polygon2>,
    pub(super) stock_to_leave: f64,
    pub(super) tool_diameter: f64,
    pub(super) order: AreaOrder,
}

/// The jobs of one By Area run.
pub(super) struct AreaPlan {
    pub(super) order: AreaOrder,
    pub(super) params: MergeTreeParams,
    pub(super) rows: usize,
    pub(super) cols: usize,
    /// The tree's pocket count (valleys, bands and roots).
    pub(super) pocket_count: usize,
    pub(super) valleys: Vec<Valley>,
    /// The tool-CL Z of each cell, as the tree read it.
    pub(super) cl_z: Vec<f64>,
    /// The valley of each cell (an index into `valleys`), or `None`.
    valley_of_cell: Vec<Option<usize>>,
    /// The job of each cell for the region map: the valley when the cell has
    /// material that the valley cuts, else the rest when the cell has
    /// material, else `None`.
    pub(super) cell_job: Vec<Option<JobKind>>,
    /// True when the rest job runs.
    rest_runs: bool,
    /// The highest material top over the cells with a job, before the first
    /// job. The overlay draws at this Z.
    pub(super) top_material_z: f64,
    pub(super) level_top: f64,
    stock_to_leave: f64,
}

/// True when the ray has material above `lo` (plus the 0.01 mm of the level
/// grid test) that starts below `hi`.
fn material_in_band(stock: &TriDexelStock, row: usize, col: usize, lo: f64, hi: f64) -> bool {
    stock
        .z_grid
        .ray(row, col)
        .iter()
        .any(|seg| f64::from(seg.exit) > lo + 0.01 && f64::from(seg.enter) < hi)
}

/// The tree input (PLAN §1.2): `z = max(CL, z_floor)`, and a cell is masked
/// when the mesh does not cover it or its centre is outside the boundary.
pub(super) fn flow_field(
    stock: &TriDexelStock,
    surface_hm: &SurfaceHeightmap,
    z_floor: Option<f64>,
    boundary: Option<&Polygon2>,
) -> FlowField {
    let grid = &stock.z_grid;
    let floor = z_floor.unwrap_or(f64::NEG_INFINITY);
    let z: Vec<f64> = surface_hm
        .z_or_bbox_floor_values()
        .iter()
        .map(|&v| v.max(floor))
        .collect();
    let covered = surface_hm.covered_flags();
    let mut nodata = vec![true; grid.rows * grid.cols];
    for row in 0..grid.rows {
        for col in 0..grid.cols {
            let i = row * grid.cols + col;
            let inside = boundary.is_none_or(|b| {
                let (x, y) = grid.cell_to_world(row, col);
                b.contains_point(&P2::new(x, y))
            });
            if let Some(slot) = nodata.get_mut(i) {
                *slot = !(covered.get(i).copied().unwrap_or(false) && inside);
            }
        }
    }
    FlowField {
        nx: grid.cols,
        ny: grid.rows,
        ox: grid.origin_u,
        oy: grid.origin_v,
        cell: grid.cell_size,
        z,
        nodata,
    }
}

impl AreaPlan {
    /// Build the tree and the jobs, once, before the first level (PLAN §1.4).
    pub(super) fn build(
        stock: &TriDexelStock,
        surface_hm: &SurfaceHeightmap,
        inputs: &AreaPlanInputs<'_>,
    ) -> Self {
        let field = flow_field(stock, surface_hm, inputs.z_floor, inputs.boundary);
        let params = pocket_params(inputs.tool_diameter);
        let tree = build_pocket_tree(&field, inputs.top_z, &params);
        Self::from_tree(stock, &tree, field.z, params, inputs)
    }

    /// The jobs of `tree` (the tree of the planner grid of `stock`).
    pub(super) fn from_tree(
        stock: &TriDexelStock,
        tree: &PocketTree,
        cl_z: Vec<f64>,
        params: MergeTreeParams,
        inputs: &AreaPlanInputs<'_>,
    ) -> Self {
        let grid = &stock.z_grid;
        let (rows, cols) = (grid.rows, grid.cols);
        let n = rows * cols;
        let stl = inputs.stock_to_leave;

        // One pocket is no split: no valley (sentry S1).
        let valley_ids = if tree.pockets.len() < 2 {
            Vec::new()
        } else {
            tree.valleys()
        };
        let mut valley_index = vec![None; tree.pockets.len()];
        for (k, &id) in valley_ids.iter().enumerate() {
            if let Some(slot) = valley_index.get_mut(id) {
                *slot = Some(k);
            }
        }
        let valley_of_cell: Vec<Option<usize>> = (0..n)
            .map(|i| {
                tree.labels
                    .get(i)
                    .copied()
                    .flatten()
                    .and_then(|p| valley_index.get(p).copied().flatten())
            })
            .collect();

        let valleys: Vec<Valley> = valley_ids
            .iter()
            .enumerate()
            .filter_map(|(k, &id)| {
                let p = tree.pockets.get(id)?;
                let anchor = anchor_of(&valley_of_cell, rows, cols, k).map_or_else(
                    || P2::new(p.bbox_xy[0], p.bbox_xy[1]),
                    |(row, col)| {
                        let (x, y) = grid.cell_to_world(row, col);
                        P2::new(x, y)
                    },
                );
                Some(Valley {
                    pocket: id,
                    min_z: p.min_z,
                    saddle_z: p.saddle_z,
                    depth_mm: p.depth_mm,
                    area_mm2: p.area_mm2,
                    anchor,
                })
            })
            .collect();

        let mut plan = Self {
            order: inputs.order,
            params,
            rows,
            cols,
            pocket_count: tree.pockets.len(),
            valleys,
            cl_z,
            valley_of_cell,
            cell_job: vec![None; n],
            rest_runs: false,
            top_material_z: f64::NEG_INFINITY,
            level_top: inputs.top_z,
            stock_to_leave: stl,
        };

        // The job of each cell, from the stock before the first job.
        let mut any_material = false;
        let mut rest_above_saddle = false;
        for row in 0..rows {
            for col in 0..cols {
                let i = row * cols + col;
                let floor = plan.cl_z.get(i).copied().unwrap_or(f64::NEG_INFINITY) + stl;
                if !material_in_band(stock, row, col, floor, f64::INFINITY) {
                    continue;
                }
                any_material = true;
                let job = match plan.valley_of_cell.get(i).copied().flatten() {
                    Some(v) => {
                        let saddle = plan.valleys.get(v).map_or(f64::INFINITY, |x| x.saddle_z);
                        if plan.order == AreaOrder::RestFirst
                            && material_in_band(stock, row, col, floor.max(saddle), f64::INFINITY)
                        {
                            rest_above_saddle = true;
                        }
                        if material_in_band(stock, row, col, floor, plan.valley_upper(v)) {
                            JobKind::Valley(v)
                        } else {
                            JobKind::Rest
                        }
                    }
                    None => JobKind::Rest,
                };
                if let Some(slot) = plan.cell_job.get_mut(i) {
                    *slot = Some(job);
                }
                if let Some(top) = crate::stock::dexel::ray_top(stock.z_grid.ray(row, col)) {
                    plan.top_material_z = plan.top_material_z.max(f64::from(top));
                }
            }
        }
        if !plan.top_material_z.is_finite() {
            plan.top_material_z = inputs.top_z;
        }
        let rest_cells = plan.job_cells(JobKind::Rest);
        plan.rest_runs = if plan.valleys.is_empty() || plan.order == AreaOrder::RestLast {
            any_material
        } else {
            rest_cells > 0 || rest_above_saddle
        };
        plan
    }

    /// The upper Z of valley `v`'s levels: its saddle when the rest runs
    /// first, else no bound.
    fn valley_upper(&self, v: usize) -> f64 {
        match self.order {
            AreaOrder::RestFirst => self.valleys.get(v).map_or(f64::INFINITY, |x| x.saddle_z),
            AreaOrder::RestLast | AreaOrder::RestLastMasked => f64::INFINITY,
        }
    }

    /// True when valley `v` owns its cells at `z_level`.
    fn valley_owns_at(&self, v: usize, z_level: f64) -> bool {
        z_level < self.valley_upper(v)
    }

    /// The number of cells the region map gives to `job`.
    pub(super) fn job_cells(&self, job: JobKind) -> usize {
        self.cell_job.iter().filter(|&&j| j == Some(job)).count()
    }

    /// True when `job` has material to cut and runs.
    pub(super) fn runs(&self, job: JobKind) -> bool {
        match job {
            JobKind::Rest => self.rest_runs,
            JobKind::Valley(_) => self.job_cells(job) > 0,
        }
    }

    /// The number of jobs that run.
    pub(super) fn job_total(&self) -> usize {
        usize::from(self.rest_runs)
            + (0..self.valleys.len())
                .filter(|&v| self.runs(JobKind::Valley(v)))
                .count()
    }

    /// True when the rest runs before the valleys. A plan with no valley
    /// has only the rest.
    pub(super) fn rest_first(&self) -> bool {
        self.order == AreaOrder::RestFirst || self.valleys.is_empty()
    }

    /// True when the rest job runs the waterline cleanup after each of its
    /// levels, as Global does: the rest runs last, or there is no valley.
    /// Otherwise the cleanup runs once after every job (F9).
    pub(super) fn rest_cleans_per_level(&self) -> bool {
        self.order == AreaOrder::RestLast || self.valleys.is_empty()
    }

    /// The levels of `job`, top down (PLAN §3.4, defect 2): the global
    /// levels below the job's upper Z, down to and including the first
    /// level at or below the job's floor plus the leave.
    pub(super) fn job_levels(&self, job: JobKind, z_levels: &[f64]) -> Vec<f64> {
        match job {
            JobKind::Valley(v) => {
                let Some(valley) = self.valleys.get(v) else {
                    return Vec::new();
                };
                let below: Vec<f64> = z_levels
                    .iter()
                    .copied()
                    .filter(|&z| self.valley_owns_at(v, z))
                    .collect();
                job_levels(&below, valley.min_z + self.stock_to_leave)
            }
            JobKind::Rest => {
                if self.rest_cleans_per_level() {
                    // Global's levels, exactly.
                    return z_levels.to_vec();
                }
                // The rest owns the cells of no valley at every level, and the
                // valley cells at and above each saddle.
                let mut floor = f64::INFINITY;
                for (i, &z) in self.cl_z.iter().enumerate() {
                    if self.valley_of_cell.get(i).copied().flatten().is_none() {
                        floor = floor.min(z + self.stock_to_leave);
                    }
                }
                for v in &self.valleys {
                    floor = floor.min(v.saddle_z);
                }
                job_levels(z_levels, floor)
            }
        }
    }

    /// The cells that `job` owns at `z_level`, as the mask of run order
    /// `run_index` (0-based). `None` when it owns no cell.
    pub(super) fn mask(&self, job: JobKind, run_index: usize, z_level: f64) -> Option<AreaMask> {
        let owned: Vec<bool> = match job {
            JobKind::Valley(v) => self
                .valley_of_cell
                .iter()
                .map(|&c| c == Some(v) && self.valley_owns_at(v, z_level))
                .collect(),
            JobKind::Rest => {
                if self.rest_cleans_per_level() {
                    // Rest last: Global on the remaining stock.
                    vec![true; self.rows * self.cols]
                } else {
                    self.valley_of_cell
                        .iter()
                        .map(|&c| c.is_none_or(|v| !self.valley_owns_at(v, z_level)))
                        .collect()
                }
            }
        };
        AreaMask::from_owned(run_index, owned, self.cols)
    }

    /// The next valley: of `left`, the one whose anchor is nearest to
    /// `from` in XY. A tie goes to the lower `min_z`, then to the lower
    /// valley index (the tree's deepest-first order). With no tool position
    /// the deepest valley goes first.
    pub(super) fn nearest_valley(&self, left: &[usize], from: Option<P3>) -> Option<usize> {
        left.iter().copied().min_by(|&a, &b| {
            let key = |v: usize| {
                let valley = self.valleys.get(v);
                let d = match (from, valley) {
                    (Some(p), Some(x)) => (x.anchor.x - p.x).powi(2) + (x.anchor.y - p.y).powi(2),
                    _ => 0.0,
                };
                (d, valley.map_or(f64::INFINITY, |x| x.min_z))
            };
            let (da, za) = key(a);
            let (db, zb) = key(b);
            da.total_cmp(&db).then(za.total_cmp(&zb)).then(a.cmp(&b))
        })
    }
}

/// The levels of one job, top down (PLAN §3.4, defect 2).
///
/// The job takes the levels down to and including the first level at or
/// below `floor_z` (the job's lowest CL plus stock-to-leave). That level
/// drapes onto the floor, so a floor between two levels is cut. When no
/// level is at or below `floor_z`, the job takes every level.
pub(super) fn job_levels(z_levels: &[f64], floor_z: f64) -> Vec<f64> {
    let end = z_levels
        .iter()
        .position(|&z| z <= floor_z + 0.01)
        .map_or(z_levels.len(), |i| i + 1);
    z_levels.iter().take(end).copied().collect()
}

/// The cell of valley `v` nearest to the centroid of its cells, as
/// `(row, col)`. `None` when the valley has no cell.
fn anchor_of(
    valley_of_cell: &[Option<usize>],
    rows: usize,
    cols: usize,
    v: usize,
) -> Option<(usize, usize)> {
    let cells = || {
        (0..rows * cols)
            .filter(move |&i| valley_of_cell.get(i).copied().flatten() == Some(v))
            .map(move |i| (i / cols, i % cols))
    };
    let (mut sr, mut sc, mut count) = (0.0, 0.0, 0usize);
    for (row, col) in cells() {
        sr += row as f64;
        sc += col as f64;
        count += 1;
    }
    if count == 0 {
        return None;
    }
    let (cr, cc) = (sr / count as f64, sc / count as f64);
    cells().min_by(|&(ra, ca), &(rb, cb)| {
        let da = (ra as f64 - cr).powi(2) + (ca as f64 - cc).powi(2);
        let db = (rb as f64 - cr).powi(2) + (cb as f64 - cc).powi(2);
        da.total_cmp(&db)
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn the_dials_scale_with_the_tool_and_give_the_reference_at_six_mm() {
        let p = pocket_params(6.0);
        assert!((p.persistence_h_mm - 2.0).abs() < 1e-12);
        assert!((p.min_area_mm2 - 400.0).abs() < 1e-9);
        let p = pocket_params(12.0);
        assert!((p.persistence_h_mm - 4.0).abs() < 1e-12);
        assert!((p.min_area_mm2 - 1600.0).abs() < 1e-9);
    }

    /// Two V troughs along Y (floors Z 0 at x = 10 and Z 1 at x = 30) split
    /// by a ridge at Z 7, rims at Z 9, under a stock top at Z 10. The tree
    /// has two valleys and a root band above Z 7.
    fn two_trough_plan(order: AreaOrder) -> (AreaPlan, Vec<f64>) {
        let stock = TriDexelStock::from_stock(0.0, 0.0, 40.0, 20.0, -1.0, 10.0, 1.0);
        let grid = &stock.z_grid;
        let (rows, cols) = (grid.rows, grid.cols);
        let mut z = Vec::with_capacity(rows * cols);
        for row in 0..rows {
            for col in 0..cols {
                let (x, _) = grid.cell_to_world(row, col);
                z.push(if x <= 10.0 {
                    9.0 - 0.9 * x
                } else if x <= 20.0 {
                    7.0 * (x - 10.0) / 10.0
                } else if x <= 30.0 {
                    7.0 - 6.0 * (x - 20.0) / 10.0
                } else {
                    1.0 + 0.8 * (x - 30.0)
                });
            }
        }
        let field = FlowField {
            nx: cols,
            ny: rows,
            ox: grid.origin_u,
            oy: grid.origin_v,
            cell: grid.cell_size,
            z: z.clone(),
            nodata: vec![false; rows * cols],
        };
        let params = MergeTreeParams {
            persistence_h_mm: 2.0,
            min_area_mm2: 10.0,
        };
        let tree = build_pocket_tree(&field, 10.0, &params);
        let inputs = AreaPlanInputs {
            top_z: 10.0,
            z_floor: None,
            boundary: None,
            stock_to_leave: 0.5,
            tool_diameter: 6.0,
            order,
        };
        let plan = AreaPlan::from_tree(&stock, &tree, z, params, &inputs);
        (plan, vec![7.0, 4.0, 1.0, 0.5])
    }

    /// PLAN §2.4 (the S3 invariant, cell by cell): at every level each cell
    /// has exactly one owner when the rest runs first, and a cell owned at a
    /// level gets that level from its owner, down to the owner's floor. When
    /// the rest runs last, the valleys partition their cells and the rest
    /// owns every cell.
    #[test]
    fn the_jobs_partition_the_grid_at_every_level() {
        for order in [AreaOrder::RestFirst, AreaOrder::RestLast] {
            let (plan, levels) = two_trough_plan(order);
            assert_eq!(plan.valleys.len(), 2, "{order:?}: two valleys");
            assert!(
                plan.runs(JobKind::Rest),
                "{order:?}: the ridge band is rest"
            );
            let jobs: Vec<JobKind> = std::iter::once(JobKind::Rest)
                .chain((0..plan.valleys.len()).map(JobKind::Valley))
                .collect();
            for &z in &levels {
                let masks: Vec<Option<AreaMask>> =
                    jobs.iter().map(|&j| plan.mask(j, 0, z)).collect();
                for i in 0..plan.rows * plan.cols {
                    let (row, col) = (i / plan.cols, i % plan.cols);
                    let owners: Vec<JobKind> = jobs
                        .iter()
                        .zip(&masks)
                        .filter(|(_, m)| m.as_ref().is_some_and(|m| m.owns(row, col)))
                        .map(|(&j, _)| j)
                        .collect();
                    let valley_owners = owners.iter().filter(|j| **j != JobKind::Rest).count();
                    assert!(
                        valley_owners <= 1,
                        "{order:?}: cell {i} at Z {z}: {owners:?}"
                    );
                    match order {
                        AreaOrder::RestFirst => {
                            assert_eq!(owners.len(), 1, "{order:?}: cell {i} at Z {z}: {owners:?}");
                            let owner = owners[0];
                            let job_levels = plan.job_levels(owner, &levels);
                            let floor = job_levels.last().copied().unwrap_or(f64::INFINITY);
                            assert!(
                                job_levels.contains(&z) || z < floor,
                                "{order:?}: cell {i} is owned by {owner:?} at Z {z}, which is \
                                 not one of its levels {job_levels:?}"
                            );
                        }
                        AreaOrder::RestLast => {
                            assert!(owners.contains(&JobKind::Rest), "{order:?}: cell {i}");
                        }
                        AreaOrder::RestLastMasked => {} // XEXP
                    }
                }
            }
        }
    }

    /// Rest first: a valley's levels stop above at its saddle; the rest
    /// takes the levels at and above the saddle. Rest last: a valley takes
    /// every level down to its floor.
    #[test]
    fn a_valley_starts_below_its_saddle_only_when_the_rest_runs_first() {
        let (first, levels) = two_trough_plan(AreaOrder::RestFirst);
        for v in 0..first.valleys.len() {
            let saddle = first.valleys[v].saddle_z;
            let lv = first.job_levels(JobKind::Valley(v), &levels);
            assert!(
                !lv.is_empty() && lv.iter().all(|&z| z < saddle),
                "{lv:?} {saddle}"
            );
        }
        assert!(first.job_levels(JobKind::Rest, &levels).contains(&7.0));
        let (last, levels) = two_trough_plan(AreaOrder::RestLast);
        for v in 0..last.valleys.len() {
            let lv = last.job_levels(JobKind::Valley(v), &levels);
            assert_eq!(lv.first(), Some(&7.0), "{lv:?}");
        }
        assert_eq!(last.job_levels(JobKind::Rest, &levels), levels);
    }

    /// Nearest next, with the tie rule: equal distance goes to the lower
    /// floor; no tool position goes to the deepest.
    #[test]
    fn the_next_valley_is_the_nearest_and_a_tie_takes_the_lower_floor() {
        let (plan, _) = two_trough_plan(AreaOrder::RestFirst);
        let deep = (0..2).min_by(|&a, &b| plan.valleys[a].min_z.total_cmp(&plan.valleys[b].min_z));
        assert_eq!(plan.nearest_valley(&[0, 1], None), deep);
        for v in 0..2 {
            let a = plan.valleys[v].anchor;
            let at = Some(P3::new(a.x, a.y, 5.0));
            assert_eq!(plan.nearest_valley(&[0, 1], at), Some(v));
        }
        let (a0, a1) = (plan.valleys[0].anchor, plan.valleys[1].anchor);
        let mid = Some(P3::new((a0.x + a1.x) * 0.5, (a0.y + a1.y) * 0.5, 5.0));
        assert_eq!(
            plan.nearest_valley(&[0, 1], mid),
            deep,
            "a tie takes the lower floor"
        );
    }

    #[test]
    fn a_job_takes_the_levels_down_to_its_floor() {
        let levels = [10.0, 6.0, 2.0, -2.0];
        assert_eq!(job_levels(&levels, 3.5), vec![10.0, 6.0, 2.0]);
        assert_eq!(job_levels(&levels, 6.0), vec![10.0, 6.0]);
        assert_eq!(job_levels(&levels, -9.0), levels.to_vec());
    }
}
