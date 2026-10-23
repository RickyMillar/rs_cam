//! Material that stands above a target surface: ONE column walk and ONE
//! island grouping, for two callers (G-ROUGHCOV, G-THINSTAND).
//!
//! # The callers
//!
//! * `metrology::union_coverage` (behind `test-support`): the whole-board
//!   audit of the final stock against `model + stock_to_leave`.
//! * [`measure_rough_coverage`] (product path): one 3D Rough, read on the
//!   op's own checkpoint stock against the op's own target. The simulation
//!   triage reports it as `project.rough_leaves_islands`.
//!
//! The walk lives here and not in `metrology/` because `lib.rs` gates
//! `metrology` behind `test-support` (F085), and the triage is a product
//! path. `metrology` calls this module; this module never calls
//! `metrology`.
//!
//! # The measure
//!
//! For each column of the stock's Z dexel grid that the caller's target
//! covers:
//!
//! ```text
//! standing = stock_top_z − target_z
//! ```
//!
//! `standing` is VERTICAL (the column frame). On a wall at slope `θ` the
//! excess along the surface normal is `standing × cos θ`. A column with no
//! target is outside the population (`NAN`). A column with a target and no
//! material is cut through (`NEG_INFINITY`).
//!
//! # Islands
//!
//! An island is a 4-connected group of cells whose `standing` is above the
//! caller's bar. 4-connected is the grouping the union audit has used since
//! it shipped (`union_coverage_m1` asserts its `hotspot_count`), so both
//! callers keep it. Two cells that touch only at a corner are two islands.
//!
//! Each island reports its area, its tallest `standing`, and its width
//! ([`StandingIsland::min_width_mm`]).

use serde::{Deserialize, Serialize};

use crate::dexel_stock::TriDexelStock;
use crate::geo::P2;
use crate::ids::ToolpathId;
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::polygon::Polygon2;
use crate::stock::dexel::DexelGrid;
use crate::tool::MillingCutter;

#[cfg(feature = "parallel")]
use rayon::prelude::*;

/// The per-column `standing` of one stock against one target, row-major on
/// the stock's Z grid.
#[derive(Debug, Clone)]
pub struct StandingField {
    rows: usize,
    cols: usize,
    origin_u: f64,
    origin_v: f64,
    cell_mm: f64,
    /// `stock_top − target`; `NAN` = no target; `NEG_INFINITY` = no
    /// material.
    standing: Vec<f64>,
    /// The stock top of each column (mm); `NAN` when the column holds no
    /// material.
    top: Vec<f64>,
}

impl StandingField {
    /// Walk every column of `grid`. `target_z(row, col, centre)` gives the
    /// target Z of the column, or `None` when the column is outside the
    /// population.
    #[must_use]
    pub fn walk<F>(grid: &DexelGrid, target_z: F) -> Self
    where
        F: Fn(usize, usize, P2) -> Option<f64> + Sync,
    {
        let (rows, cols) = (grid.rows, grid.cols);
        let row_of = |row: usize| -> Vec<(f64, f64)> {
            (0..cols)
                .map(|col| {
                    let (u, v) = grid.cell_to_world(row, col);
                    let top = grid.top_z_at(row, col).map(f64::from);
                    let Some(target) = target_z(row, col, P2::new(u, v)) else {
                        return (f64::NAN, top.unwrap_or(f64::NAN));
                    };
                    match top {
                        Some(top) => (top - target, top),
                        None => (f64::NEG_INFINITY, f64::NAN),
                    }
                })
                .collect()
        };
        #[cfg(feature = "parallel")]
        let by_row: Vec<Vec<(f64, f64)>> = (0..rows).into_par_iter().map(row_of).collect();
        #[cfg(not(feature = "parallel"))]
        let by_row: Vec<Vec<(f64, f64)>> = (0..rows).map(row_of).collect();
        let (standing, top): (Vec<f64>, Vec<f64>) = by_row.into_iter().flatten().unzip();
        Self {
            rows,
            cols,
            origin_u: grid.origin_u,
            origin_v: grid.origin_v,
            cell_mm: grid.cell_size,
            standing,
            top,
        }
    }

    #[must_use]
    pub fn rows(&self) -> usize {
        self.rows
    }

    #[must_use]
    pub fn cols(&self) -> usize {
        self.cols
    }

    /// The grid cell (mm). Every area scales with its square.
    #[must_use]
    pub fn cell_mm(&self) -> f64 {
        self.cell_mm
    }

    /// `standing` at `(row, col)`; `NAN` outside the grid.
    #[must_use]
    pub fn at(&self, row: usize, col: usize) -> f64 {
        if row >= self.rows || col >= self.cols {
            return f64::NAN;
        }
        self.standing
            .get(row * self.cols + col)
            .copied()
            .unwrap_or(f64::NAN)
    }

    /// The centre of cell `(row, col)` in the stock's frame.
    #[must_use]
    pub fn cell_to_world(&self, row: usize, col: usize) -> P2 {
        P2::new(
            self.origin_u + col as f64 * self.cell_mm,
            self.origin_v + row as f64 * self.cell_mm,
        )
    }

    fn top_at(&self, row: usize, col: usize) -> f64 {
        self.top
            .get(row * self.cols + col)
            .copied()
            .unwrap_or(f64::NAN)
    }
}

/// One connected group of cells that stand above the bar.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct StandingIsland {
    pub cells: usize,
    /// XY-projected area (mm²): `cells × cell²`.
    pub area_mm2: f64,
    /// The tallest vertical `standing` in the island (mm above the target).
    pub max_standing_mm: f64,
    /// The centre of the tallest cell and the stock top there (mm).
    pub peak: [f64; 3],
    /// The area centroid `[x, y]` (mm).
    pub centroid: [f64; 2],
    /// The XY box of the cell centres, `[min_x, min_y, max_x, max_y]` (mm).
    pub bbox: [f64; 4],
    /// The width of the island where it is widest across: the diameter of
    /// the largest disk inside it (mm).
    ///
    /// Measure: `(2·d − 1) × cell`, where `d` is the largest distance (in
    /// cells, centre to centre) from a cell of the island to the nearest
    /// cell outside it (a Euclidean distance transform; the grid edge
    /// counts as outside). A strip of `w` cells (odd `w`) reads `w` cells.
    ///
    /// Limits:
    ///
    /// * The value is quantised to the cell: an even strip reads one cell
    ///   narrow, and the floor is one cell.
    /// * It is the thickest part of the island, not the thinnest. A thin
    ///   spur that stands on a wide low patch above the bar reads the
    ///   patch's width (the same limit as the disk opening of
    ///   `phase0/tools/spurs.py`). Read it together with the area and the
    ///   bbox.
    /// * Two islands that touch only at a corner can read up to one cell
    ///   wide each at that corner.
    pub min_width_mm: f64,
    /// The island stands against a wall that is at least as tall as the
    /// island (A4). The rule:
    ///
    /// ```text
    /// rise(i)      = max over the 8 neighbours j of cell i of  wall_top(j) − target(i)
    /// wall_top(j)  = target(j), or the stock top of j when j is outside the reach
    /// touches_wall = max over the island's cells of rise(i) > max_standing − one cell
    /// ```
    ///
    /// Derivation: material that leans on something taller than itself is a
    /// strip against a wall; material with nothing at least as tall within
    /// one cell stands free (a hole core, a spur). The island's own height,
    /// `max_standing`, is the scale, so the tag does not read the op's depth
    /// per pass, and two ops tag the same sliver the same way. A strip left
    /// at the wall top stands exactly as high as the wall rises, so the test
    /// allows one cell: the G-OVERCUTREF tolerance, the same quantisation of
    /// a column top that the bar allows. An unreachable column is left whole
    /// by the plan, so its stock top is its floor.
    ///
    /// No class reads it. Always `false` from [`standing_islands`]; only
    /// [`measure_rough_coverage`] knows the target.
    pub touches_wall: bool,
}

/// Group the cells of `field` that stand more than `above_mm` above the
/// target into 4-connected islands, in row-major order of their first cell.
#[must_use]
pub fn standing_islands(field: &StandingField, above_mm: f64) -> Vec<StandingIsland> {
    group_islands(field, above_mm, None)
}

/// [`standing_islands`], with `wall` = (the rise of each cell, row-major,
/// from [`wall_rise`]; the tolerance) setting
/// [`StandingIsland::touches_wall`].
fn group_islands(
    field: &StandingField,
    above_mm: f64,
    wall: Option<(&[f64], f64)>,
) -> Vec<StandingIsland> {
    let (rows, cols) = (field.rows, field.cols);
    let cell = field.cell_mm;
    let above = |row: usize, col: usize| -> bool { field.at(row, col) > above_mm };
    let width = inscribed_distance(field, above_mm);

    let mut visited = vec![false; rows * cols];
    let mut islands: Vec<StandingIsland> = Vec::new();
    let mut stack: Vec<(usize, usize)> = Vec::new();
    for row in 0..rows {
        for col in 0..cols {
            let idx = row * cols + col;
            if visited.get(idx).copied().unwrap_or(true) || !above(row, col) {
                continue;
            }
            let mut cells = 0usize;
            let mut worst = f64::NEG_INFINITY;
            let mut peak = [0.0f64; 3];
            let mut deepest_inside = 0.0f64;
            let mut max_rise = f64::NEG_INFINITY;
            let mut sum_x = 0.0f64;
            let mut sum_y = 0.0f64;
            let mut bbox = [
                f64::INFINITY,
                f64::INFINITY,
                f64::NEG_INFINITY,
                f64::NEG_INFINITY,
            ];
            stack.push((row, col));
            if let Some(v) = visited.get_mut(idx) {
                *v = true;
            }
            while let Some((r, c)) = stack.pop() {
                cells += 1;
                let s = field.at(r, c);
                let p = field.cell_to_world(r, c);
                if s > worst {
                    worst = s;
                    peak = [p.x, p.y, field.top_at(r, c)];
                }
                deepest_inside = deepest_inside.max(width.at(r, c));
                if let Some(rise) = wall.and_then(|(rise, _)| rise.get(r * cols + c)) {
                    max_rise = max_rise.max(*rise);
                }
                sum_x += p.x;
                sum_y += p.y;
                bbox[0] = bbox[0].min(p.x);
                bbox[1] = bbox[1].min(p.y);
                bbox[2] = bbox[2].max(p.x);
                bbox[3] = bbox[3].max(p.y);
                let neighbours = [
                    (r.wrapping_sub(1), c),
                    (r + 1, c),
                    (r, c.wrapping_sub(1)),
                    (r, c + 1),
                ];
                for (nr, nc) in neighbours {
                    if nr >= rows || nc >= cols {
                        continue;
                    }
                    let nidx = nr * cols + nc;
                    if visited.get(nidx).copied().unwrap_or(true) || !above(nr, nc) {
                        continue;
                    }
                    if let Some(v) = visited.get_mut(nidx) {
                        *v = true;
                    }
                    stack.push((nr, nc));
                }
            }
            islands.push(StandingIsland {
                cells,
                area_mm2: cells as f64 * cell * cell,
                max_standing_mm: worst,
                peak,
                centroid: [sum_x / cells as f64, sum_y / cells as f64],
                bbox,
                min_width_mm: (2.0 * deepest_inside - 1.0).max(1.0) * cell,
                touches_wall: wall.is_some_and(|(_, tol)| max_rise > worst - tol),
            });
        }
    }
    islands
}

/// The distance (cells) from each above-bar cell to the nearest cell that
/// is not above the bar, with the grid edge counted as outside. The ONE EDT
/// is `geometry::grid_field::distance_transform_2d`; this pads the grid by
/// one cell of "outside" on each side.
struct InscribedDistance {
    cols: usize,
    /// Row-major over the padded grid, `(rows + 2) × (cols + 2)`.
    dist: Vec<f64>,
}

impl InscribedDistance {
    fn at(&self, row: usize, col: usize) -> f64 {
        let padded_cols = self.cols + 2;
        self.dist
            .get((row + 1) * padded_cols + (col + 1))
            .copied()
            .unwrap_or(0.0)
    }
}

fn inscribed_distance(field: &StandingField, above_mm: f64) -> InscribedDistance {
    let (rows, cols) = (field.rows, field.cols);
    let (pr, pc) = (rows + 2, cols + 2);
    let mut outside = vec![true; pr * pc];
    let mut any = false;
    for row in 0..rows {
        for col in 0..cols {
            if field.at(row, col) > above_mm
                && let Some(o) = outside.get_mut((row + 1) * pc + (col + 1))
            {
                *o = false;
                any = true;
            }
        }
    }
    let dist = if any {
        crate::geometry::grid_field::distance_transform_2d(&outside, pr, pc)
    } else {
        Vec::new()
    };
    InscribedDistance { cols, dist }
}

// ── G-ROUGHCOV: the coverage of one 3D Rough ─────────────────────────────

/// The target of one 3D Rough, in the frame of its checkpoint stock.
#[derive(Clone, Copy)]
pub struct RoughTarget<'a> {
    pub mesh: &'a TriangleMesh,
    pub index: &'a SpatialIndex,
    /// The op's cutter. Its drop-cutter surface is the planner's surface,
    /// and its profile (`height_at_radius`) erodes the floor.
    pub cutter: &'a dyn MillingCutter,
    /// The op's axial stock to leave (`stock_to_leave_axial`, mm).
    pub stock_to_leave_mm: f64,
    /// A pinned `bottom_z`, which floors the planner's level plan.
    pub pinned_floor_z: Option<f64>,
    /// The containment polygons the boundary clip keeps the tool centre in
    /// (the session's `boundary_centre`, G-BOUNDARYPHANTOM). `None`: the
    /// boundary is off, and every centre of the grid is a centre the tool
    /// can take.
    pub centre: Option<&'a [Polygon2]>,
}

/// Why a rough's coverage was not measured. The reading abstains: no
/// verdict, no finding.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum RoughCoverageAbstention {
    /// The simulation cell is coarser than the stock to leave, so the bar
    /// (`stock_to_leave + one cell`) is mostly cell noise: the reading cannot
    /// tell an island of the spec height from quantisation.
    CellCoarserThanStockToLeave {
        cell_mm: f64,
        stock_to_leave_mm: f64,
    },
    /// The op's setup is not the identity setup. Its checkpoint stock is in
    /// the setup-local frame; this pass reads the identity frame only.
    SetupNotIdentity,
    /// The op has no model mesh to measure against.
    NoModel,
    /// The op's generation inputs did not resolve (the session error text).
    TargetUnresolved { error: String },
    /// The run holds the op but no checkpoint stock for it, so there is
    /// nothing to read (G-ROUGHCOV A2: a missing stock is an abstention,
    /// never a measured zero).
    NoCheckpoint,
    /// The session does not hold the run the caller reads (another run, or
    /// none), and the caller gave no readings of its own.
    RunNotStored,
}

/// The measured coverage of one 3D Rough.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoughCoverage {
    /// The simulation cell (mm). Every area and width is on this grid.
    pub cell_mm: f64,
    pub stock_to_leave_mm: f64,
    /// One simulation cell: the G-OVERCUTREF tolerance
    /// (`sim_triage::commanded_tolerance_mm`).
    pub tolerance_mm: f64,
    /// `stock_to_leave + tolerance`: a cell is above spec when it stands
    /// more than this above the target. The target already holds one stock
    /// to leave (`model + stock_to_leave`), so an above-spec cell carries
    /// more than twice the stock to leave over the model.
    pub bar_mm: f64,
    /// Columns in the population (inside the op's reach).
    pub compared_cells: usize,
    pub compared_area_mm2: f64,
    pub above_spec_cells: usize,
    pub above_spec_area_mm2: f64,
    /// Every island: the free ones first, then the ones at a wall
    /// ([`StandingIsland::touches_wall`]), tallest first within each group.
    /// Empty is a measured zero.
    pub islands: Vec<StandingIsland>,
}

/// The coverage outcome of one 3D Rough.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "state", content = "reading")]
pub enum RoughCoverageOutcome {
    Measured(RoughCoverage),
    NotMeasurable(RoughCoverageAbstention),
}

/// The coverage reading of one toolpath.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoughCoverageReading {
    pub toolpath_id: ToolpathId,
    pub outcome: RoughCoverageOutcome,
}

impl RoughCoverageReading {
    /// The one narration sentence of this reading (`narrate_toolpath`):
    /// a measured zero, the island count with the tallest island, or the
    /// abstention reason.
    #[must_use]
    pub fn narration_sentence(&self) -> String {
        match &self.outcome {
            RoughCoverageOutcome::Measured(c) => match c.islands.first() {
                None => format!(
                    "coverage: no standing island above the stock to leave ({:.2} mm + one \
                     {:.3} mm cell) on {:.0} mm2 compared.",
                    c.stock_to_leave_mm, c.cell_mm, c.compared_area_mm2
                ),
                Some(first) => {
                    // The islands are free first (A4), so `first` is the
                    // tallest free island when there is one.
                    let at_wall = c.islands.iter().filter(|i| i.touches_wall).count();
                    let free = c.islands.len() - at_wall;
                    let target = format!(
                        "above the target (the floor the plan can reach, with {:.2} mm stock \
                         to leave)",
                        c.stock_to_leave_mm
                    );
                    let place = format!(
                        "{:.2} mm high and {:.2} mm wide at ({:.1}, {:.1})",
                        first.max_standing_mm, first.min_width_mm, first.peak[0], first.peak[1],
                    );
                    if free > 0 {
                        format!(
                            "coverage: {free} free island(s) stand more than {:.2} mm {target}; \
                             the tallest is {place}; {at_wall} more touch a wall; {:.1} mm2 \
                             in all.",
                            c.bar_mm, c.above_spec_area_mm2,
                        )
                    } else {
                        format!(
                            "coverage: no free island stands more than {:.2} mm {target}; \
                             {at_wall} island(s) touch a wall, the tallest {place}; {:.1} mm2 \
                             in all.",
                            c.bar_mm, c.above_spec_area_mm2,
                        )
                    }
                }
            },
            RoughCoverageOutcome::NotMeasurable(why) => {
                format!("coverage: not measured ({}).", why.describe())
            }
        }
    }
}

impl RoughCoverageAbstention {
    /// One line that states the reason.
    #[must_use]
    pub fn describe(&self) -> String {
        match self {
            Self::CellCoarserThanStockToLeave {
                cell_mm,
                stock_to_leave_mm,
            } => format!(
                "the {cell_mm:.3} mm simulation cell is coarser than the \
                 {stock_to_leave_mm:.3} mm stock to leave; simulate at a cell at or under it"
            ),
            Self::SetupNotIdentity => {
                "the op's setup is not an identity setup (face up Top, no rotation), which \
                 this reading needs"
                    .to_owned()
            }
            Self::NoModel => "the op has no model mesh".to_owned(),
            Self::TargetUnresolved { error } => {
                format!("the op's generation inputs did not resolve: {error}")
            }
            Self::NoCheckpoint => "the run kept no checkpoint stock for this op".to_owned(),
            Self::RunNotStored => "the session does not hold this run; simulate again".to_owned(),
        }
    }
}

/// Measure one 3D Rough on its checkpoint stock (G-ROUGHCOV).
///
/// # The target: the plan's reachable floor (A3)
///
/// The target is the floor the rough's OWN plan can leave, from the
/// planner's own rule (`adaptive3d::intended_floor`, the functions the
/// planner calls):
///
/// ```text
/// floor(c)  = max(dc(c) + stock_to_leave, z_bottom)        per tool centre c
/// z_bottom  = max(min dc + stock_to_leave, pinned bottom_z)  the last level
/// target(x) = min over centres c with |x − c| ≤ R of floor(c) + h(|x − c|)
/// ```
///
/// * `dc` is the drop-cutter tip Z of the op's cutter, floored at the mesh
///   bbox floor where the cutter meets nothing (through holes, outside the
///   part): the planner's `SurfaceHeightmap`.
/// * `h(r)` is the cutter profile above its tip at radius `r`
///   (`height_at_radius`), `R` the cutter radius. A flat cutter has
///   `h = 0`, so the target is the erosion of the floor by the tool disc
///   (the morphological opening of the model by the tool, plus the leave).
/// * The centres are the cell centres of the stock grid. With the boundary
///   on, only those inside the tool-centre polygons. A column with no
///   centre within `R` is outside the population (`NAN`).
///
/// So the target holds the leave already, and it holds the tool shape: an
/// inside corner narrower than the tool, a valley a flat tool cannot follow,
/// and the slope under a flat tool read as the plan's own floor. A hole
/// core, the band outside the wall and a spur stand above it (the centres
/// there take the floor). Material within `R` of a vertical wall is
/// reachable (a centre `R` from the wall puts the cutter edge on it), so a
/// strip left against a wall stands above the target and reads as an
/// island ([`StandingIsland::touches_wall`] marks it).
///
/// Every level drapes in this planner and none holds a level on a slope,
/// so a Z-level terrace above the draped floor is not the plan's floor:
/// it reads as an island.
///
/// Limits:
///
/// * The centres are grid points, a subset of the centres the tool can
///   take. The minimum over a subset is never lower than the true one, so
///   the target can stand up to about one cell of slope above the true
///   reach. It never stands below it.
/// * With the boundary off, centres off the stock grid are not counted.
///   A column at the grid edge can read a higher target than the tool
///   could leave there.
/// * `dc` is sampled on this grid, not on the planner's grid
///   (`intended_floor_grid`).
///
/// # The cost
///
/// One drop-cutter query per cell (the planner's own heightmap cost at this
/// cell), then the erosion with `k = ⌊R / cell⌋`:
///
/// * a flat profile: per row, one sliding minimum per row offset of the
///   disc, `cells × (2k + 1)` window steps (each amortised O(1));
/// * any other profile: a bounded window, `cells × disc cells`, about
///   `cells × π k²`.
///
/// # The bar
///
/// PLAN.md's bar: "no uncut island bigger than the stock to leave". A cell
/// is above spec when
///
/// ```text
/// standing = stock_top − target > stock_to_leave + one cell
/// ```
///
/// The target already holds one stock to leave, so an above-spec cell
/// carries more than twice the stock to leave over the plan's floor less
/// the leave. The one cell is the G-OVERCUTREF tolerance
/// (`stock/sim_triage.rs`, `commanded_tolerance_mm`): a dexel column top is
/// quantised to its cell. There is no other threshold.
///
/// # Measurability
///
/// `NotMeasurable` when the cell is coarser than the stock to leave
/// (`cell > stock_to_leave`). The bar is then more cell than spec, and an
/// island of the spec height cannot be told from quantisation (stock
/// `CLAUDE.md`: a `NotMeasurable` metric abstains).
#[must_use]
pub fn measure_rough_coverage(
    stock: &TriDexelStock,
    target: &RoughTarget<'_>,
) -> RoughCoverageOutcome {
    let grid = &stock.z_grid;
    let cell = grid.cell_size;
    let stl = target.stock_to_leave_mm;
    if !(cell.is_finite() && cell > 0.0 && stl.is_finite() && cell <= stl) {
        return RoughCoverageOutcome::NotMeasurable(
            RoughCoverageAbstention::CellCoarserThanStockToLeave {
                cell_mm: cell,
                stock_to_leave_mm: stl,
            },
        );
    }
    let tolerance = crate::stock::sim_triage::commanded_tolerance_mm(Some(cell));
    let bar = stl + tolerance;
    let floor = crate::adaptive3d::intended_floor_grid(
        target.mesh,
        target.index,
        target.cutter,
        crate::adaptive3d::FloorGrid {
            origin_u: grid.origin_u,
            origin_v: grid.origin_v,
            rows: grid.rows,
            cols: grid.cols,
            cell_mm: cell,
        },
        stl,
        target.pinned_floor_z,
    );
    let centres = target.centre.map(|polygons| centre_mask(grid, polygons));
    let reach = reachable_floor(
        &floor,
        centres.as_deref(),
        grid.rows,
        grid.cols,
        cell,
        target.cutter,
    );
    let cols = grid.cols;
    let field = StandingField::walk(grid, |row, col, _| {
        reach
            .get(row * cols + col)
            .copied()
            .filter(|t| t.is_finite())
    });
    let rise = wall_rise(&field, &reach);
    let compared_cells = field.standing.iter().filter(|s| !s.is_nan()).count();
    let mut islands = group_islands(&field, bar, Some((&rise, tolerance)));
    // Free islands first (a hole core, a spur), then the ones at a wall;
    // tallest first within each group (A4).
    islands.sort_by(|a, b| {
        a.touches_wall
            .cmp(&b.touches_wall)
            .then(b.max_standing_mm.total_cmp(&a.max_standing_mm))
    });
    let above_spec_cells: usize = islands.iter().map(|i| i.cells).sum();
    let cell_area = cell * cell;
    RoughCoverageOutcome::Measured(RoughCoverage {
        cell_mm: cell,
        stock_to_leave_mm: stl,
        tolerance_mm: tolerance,
        bar_mm: bar,
        compared_cells,
        compared_area_mm2: compared_cells as f64 * cell_area,
        above_spec_cells,
        above_spec_area_mm2: above_spec_cells as f64 * cell_area,
        islands,
    })
}

/// The cells of one disc row: the row offset and the cutter profile height
/// at each column offset, for every offset with `(dx² + dy²) · cell² ≤ R²`.
struct DiscRow {
    dy: isize,
    /// `(dx, h)`, `dx` ascending.
    cells: Vec<(isize, f64)>,
}

/// The disc of `cutter` on a grid of `cell`: one [`DiscRow`] per row
/// offset. A cell where the profile has no height is left out.
fn disc_rows(cutter: &dyn MillingCutter, cell: f64) -> Vec<DiscRow> {
    let r = cutter.radius();
    if !(r.is_finite() && r >= 0.0) {
        return vec![DiscRow {
            dy: 0,
            cells: vec![(0, 0.0)],
        }];
    }
    let k = (r / cell).floor() as isize;
    let r2 = r * r + 1e-9 * cell * cell;
    (-k..=k)
        .map(|dy| {
            let cells = (-k..=k)
                .filter_map(|dx| {
                    let d2 = ((dx * dx + dy * dy) as f64) * cell * cell;
                    if d2 > r2 {
                        return None;
                    }
                    cutter.height_at_radius(d2.sqrt()).map(|h| (dx, h))
                })
                .collect();
            DiscRow { dy, cells }
        })
        .collect()
}

/// The reachable floor of every column (see [`measure_rough_coverage`]):
/// the minimum over the centres within the disc of `floor + h`. `NAN` where
/// no centre is within the disc. `centres`: the cells a centre may take
/// (`None`: all).
fn reachable_floor(
    floor: &[f64],
    centres: Option<&[bool]>,
    rows: usize,
    cols: usize,
    cell: f64,
    cutter: &dyn MillingCutter,
) -> Vec<f64> {
    let disc = disc_rows(cutter, cell);
    let source: Vec<f64> = floor
        .iter()
        .enumerate()
        .map(|(i, &f)| {
            let taken = centres.is_none_or(|m| m.get(i).copied().unwrap_or(false));
            if taken && f.is_finite() {
                f
            } else {
                f64::INFINITY
            }
        })
        .collect();
    // A flat profile with full rows: one sliding minimum per disc row.
    let flat = disc.iter().all(|row| {
        row.cells.iter().all(|&(_, h)| h == 0.0)
            && row
                .cells
                .windows(2)
                .all(|w| matches!(w, [a, b] if b.0 == a.0 + 1))
    });
    let row_of = |r: usize| -> Vec<f64> {
        let mut out = vec![f64::INFINITY; cols];
        let mut window = vec![f64::INFINITY; cols];
        for disc_row in &disc {
            let Some(src) = r.checked_add_signed(disc_row.dy).filter(|&s| s < rows) else {
                continue;
            };
            let Some(src_row) = source.get(src * cols..(src + 1) * cols) else {
                continue;
            };
            if flat {
                let (Some(&(lo, _)), Some(&(hi, _))) =
                    (disc_row.cells.first(), disc_row.cells.last())
                else {
                    continue;
                };
                sliding_min(src_row, lo, hi, &mut window);
                for (o, w) in out.iter_mut().zip(&window) {
                    *o = o.min(*w);
                }
            } else {
                for (c, o) in out.iter_mut().enumerate() {
                    for &(dx, h) in &disc_row.cells {
                        if let Some(v) = c.checked_add_signed(dx).and_then(|sc| src_row.get(sc)) {
                            *o = o.min(v + h);
                        }
                    }
                }
            }
        }
        out
    };
    #[cfg(feature = "parallel")]
    let by_row: Vec<Vec<f64>> = (0..rows).into_par_iter().map(row_of).collect();
    #[cfg(not(feature = "parallel"))]
    let by_row: Vec<Vec<f64>> = (0..rows).map(row_of).collect();
    by_row
        .into_iter()
        .flatten()
        .map(|t| if t.is_finite() { t } else { f64::NAN })
        .collect()
}

/// `out[c] = min(row[c + lo ..= c + hi])` over the indices inside the row
/// (`INFINITY` when none is), by a monotone queue: each index enters and
/// leaves the queue once.
fn sliding_min(row: &[f64], lo: isize, hi: isize, out: &mut [f64]) {
    let n = row.len();
    let mut queue: std::collections::VecDeque<usize> = std::collections::VecDeque::new();
    let mut next = 0usize;
    let value = |i: usize| row.get(i).copied().unwrap_or(f64::INFINITY);
    for (c, o) in out.iter_mut().enumerate().take(n) {
        // Push every index up to c + hi.
        if let Some(top) = c.checked_add_signed(hi) {
            let top = top.min(n.saturating_sub(1));
            while next <= top && next < n {
                let v = value(next);
                while queue.back().is_some_and(|&b| value(b) >= v) {
                    queue.pop_back();
                }
                queue.push_back(next);
                next += 1;
            }
        }
        // Drop every index below c + lo.
        let start = c.checked_add_signed(lo).unwrap_or(0);
        while queue.front().is_some_and(|&f| f < start) {
            queue.pop_front();
        }
        *o = queue.front().map_or(f64::INFINITY, |&f| value(f));
    }
}

/// The rise of each cell (see [`StandingIsland::touches_wall`]): the most
/// any of its 8 neighbours stands above the cell's target, where a
/// neighbour stands at its target, or at its stock top when it is outside
/// the reach. `NEG_INFINITY` for a cell outside the reach or with no
/// neighbour that has a height.
fn wall_rise(field: &StandingField, reach: &[f64]) -> Vec<f64> {
    let (rows, cols) = (field.rows, field.cols);
    let target_at = |r: usize, c: usize| reach.get(r * cols + c).copied().unwrap_or(f64::NAN);
    let mut rise = vec![f64::NEG_INFINITY; rows * cols];
    for row in 0..rows {
        for col in 0..cols {
            let own = target_at(row, col);
            if !own.is_finite() {
                continue;
            }
            let highest = (-1isize..=1)
                .flat_map(|dr| (-1isize..=1).map(move |dc| (dr, dc)))
                .filter(|&d| d != (0, 0))
                .filter_map(|(dr, dc)| {
                    let r = row.checked_add_signed(dr).filter(|&r| r < rows)?;
                    let c = col.checked_add_signed(dc).filter(|&c| c < cols)?;
                    let t = target_at(r, c);
                    let top = if t.is_finite() { t } else { field.top_at(r, c) };
                    top.is_finite().then_some(top)
                })
                .fold(f64::NEG_INFINITY, f64::max);
            if let Some(out) = rise.get_mut(row * cols + col) {
                *out = highest - own;
            }
        }
    }
    rise
}

/// The cells whose centre is inside any of `polygons` (even-odd over the
/// exterior and the holes of each polygon, a union over the polygons), by
/// a scanline per grid row.
fn centre_mask(grid: &DexelGrid, polygons: &[Polygon2]) -> Vec<bool> {
    let (rows, cols) = (grid.rows, grid.cols);
    let cell = grid.cell_size;
    let mut mask = vec![false; rows * cols];
    let mut crossings: Vec<f64> = Vec::new();
    for row in 0..rows {
        let y = grid.origin_v + row as f64 * cell;
        for polygon in polygons {
            crossings.clear();
            for ring in std::iter::once(&polygon.exterior).chain(polygon.holes.iter()) {
                let n = ring.len();
                for i in 0..n {
                    let (Some(a), Some(b)) = (ring.get(i), ring.get((i + 1) % n)) else {
                        continue;
                    };
                    if (a.y > y) != (b.y > y) {
                        crossings.push(a.x + (y - a.y) * (b.x - a.x) / (b.y - a.y));
                    }
                }
            }
            crossings.sort_by(f64::total_cmp);
            for &[x0, x1] in crossings.as_chunks::<2>().0 {
                let first = ((x0 - grid.origin_u) / cell).ceil().max(0.0);
                let last = ((x1 - grid.origin_u) / cell).floor();
                if last < first {
                    continue;
                }
                let first = first as usize;
                let last = (last as usize).min(cols.saturating_sub(1));
                for col in first..=last {
                    if let Some(m) = mask.get_mut(row * cols + col) {
                        *m = true;
                    }
                }
            }
        }
    }
    mask
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]
mod tests {
    use super::{
        RoughCoverageAbstention, RoughCoverageOutcome, RoughTarget, StandingField,
        measure_rough_coverage, reachable_floor, standing_islands,
    };
    use crate::dexel_stock::TriDexelStock;
    use crate::geo::{BoundingBox3, P2, P3};
    use crate::mesh::{SpatialIndex, TriangleMesh};
    use crate::polygon::Polygon2;
    use crate::tool::{BallEndmill, FlatEndmill, MillingCutter};

    fn flat_model(size: f64, z: f64) -> TriangleMesh {
        TriangleMesh::from_raw(
            vec![
                P3::new(0.0, 0.0, z),
                P3::new(size, 0.0, z),
                P3::new(size, size, z),
                P3::new(0.0, size, z),
            ],
            vec![[0, 1, 2], [0, 2, 3]],
        )
    }

    /// A stock over `[0, size]²` cut to `z` everywhere except the cells
    /// whose centre is in `[x0, x1] × [y0, y1]`.
    fn stock_with_block(size: f64, cell: f64, z: f32, block: [f64; 4]) -> TriDexelStock {
        let bbox = BoundingBox3 {
            min: P3::new(0.0, 0.0, 0.0),
            max: P3::new(size, size, 10.0),
        };
        let mut stock = TriDexelStock::from_bounds(&bbox, cell);
        for row in 0..stock.z_grid.rows {
            for col in 0..stock.z_grid.cols {
                let (x, y) = stock.z_grid.cell_to_world(row, col);
                let in_block = x >= block[0] && x <= block[2] && y >= block[1] && y <= block[3];
                if !in_block {
                    stock.clear_above_at(row, col, z);
                }
            }
        }
        stock
    }

    /// New pin: a strip three cells wide reads three cells; a single cell
    /// reads one cell (the floor).
    #[test]
    fn the_width_is_the_inscribed_disk_in_cells() {
        let cell = 0.5;
        let stock = stock_with_block(10.0, cell, 2.0, [4.0, 2.0, 5.0, 8.0]);
        let field = StandingField::walk(&stock.z_grid, |_, _, _| Some(2.0));
        let islands = standing_islands(&field, 1.0);
        assert_eq!(islands.len(), 1);
        let island = islands.first().unwrap();
        assert!(
            (island.min_width_mm - 3.0 * cell).abs() < 1e-9,
            "{island:?}"
        );
        assert!((island.max_standing_mm - 8.0).abs() < 1e-6, "{island:?}");

        let one = stock_with_block(10.0, cell, 2.0, [5.0, 5.0, 5.0, 5.0]);
        let field = StandingField::walk(&one.z_grid, |_, _, _| Some(2.0));
        let islands = standing_islands(&field, 1.0);
        assert_eq!(islands.len(), 1);
        assert!((islands.first().unwrap().min_width_mm - cell).abs() < 1e-9);
    }

    /// New pin: a column outside the rough's reach is not in the
    /// population, so a block there is not an island.
    #[test]
    fn a_block_outside_the_reach_is_not_compared() {
        let size = 20.0;
        let cell = 0.25;
        let model = flat_model(size, 2.0);
        let index = SpatialIndex::build_auto(&model);
        let stock = stock_with_block(size, cell, 2.5, [16.0, 16.0, 19.0, 19.0]);
        let centre = vec![Polygon2::new(vec![
            P2::new(2.0, 2.0),
            P2::new(10.0, 2.0),
            P2::new(10.0, 10.0),
            P2::new(2.0, 10.0),
        ])];
        let cutter = FlatEndmill::new(6.0, 20.0);
        let target = RoughTarget {
            mesh: &model,
            index: &index,
            cutter: &cutter,
            stock_to_leave_mm: 0.5,
            pinned_floor_z: None,
            centre: Some(&centre),
        };
        let RoughCoverageOutcome::Measured(c) = measure_rough_coverage(&stock, &target) else {
            panic!("expected a measured reading");
        };
        assert!(c.compared_cells > 0);
        assert!(c.islands.is_empty(), "{:?}", c.islands);
        // The reach is the cell centres within R = 3 mm of a centre inside
        // the square. The cells are counted on their centres, so the side
        // holds one more cell than the length.
        let side = 8.0 + 2.0 * 3.0 + cell;
        assert!(
            c.compared_area_mm2 < side * side + 1e-6,
            "{} vs {}",
            c.compared_area_mm2,
            side * side
        );
    }

    /// New pin: the narration states a measured zero, the tallest island,
    /// or the abstention.
    #[test]
    fn the_narration_sentence_states_zero_islands_or_the_reason() {
        use super::{RoughCoverage, RoughCoverageReading};
        let zero = RoughCoverageReading {
            toolpath_id: crate::ids::ToolpathId::from_raw(1),
            outcome: RoughCoverageOutcome::Measured(RoughCoverage {
                cell_mm: 0.25,
                stock_to_leave_mm: 0.5,
                tolerance_mm: 0.25,
                bar_mm: 0.75,
                compared_cells: 16,
                compared_area_mm2: 1.0,
                above_spec_cells: 0,
                above_spec_area_mm2: 0.0,
                islands: vec![],
            }),
        };
        assert!(
            zero.narration_sentence()
                .contains("no standing island above the stock to leave")
        );
        let abstained = RoughCoverageReading {
            toolpath_id: crate::ids::ToolpathId::from_raw(1),
            outcome: RoughCoverageOutcome::NotMeasurable(RoughCoverageAbstention::NoModel),
        };
        assert!(abstained.narration_sentence().contains("not measured"));
    }

    /// New pin: a cell coarser than the stock to leave abstains.
    #[test]
    fn a_cell_coarser_than_the_stock_to_leave_abstains() {
        let model = flat_model(10.0, 2.0);
        let index = SpatialIndex::build_auto(&model);
        let stock = stock_with_block(10.0, 0.5, 2.0, [4.0, 4.0, 6.0, 6.0]);
        let cutter = FlatEndmill::new(6.0, 20.0);
        let target = RoughTarget {
            mesh: &model,
            index: &index,
            cutter: &cutter,
            stock_to_leave_mm: 0.2,
            pinned_floor_z: None,
            centre: None,
        };
        assert_eq!(
            measure_rough_coverage(&stock, &target),
            RoughCoverageOutcome::NotMeasurable(
                RoughCoverageAbstention::CellCoarserThanStockToLeave {
                    cell_mm: 0.5,
                    stock_to_leave_mm: 0.2,
                }
            )
        );
    }

    /// The reachable floor by brute force: the minimum over every centre
    /// cell within `R` of `floor + h`.
    #[expect(
        clippy::indexing_slicing,
        reason = "test code: indices bounded by the grid"
    )]
    fn brute_reach(
        floor: &[f64],
        centres: &[bool],
        rows: usize,
        cols: usize,
        cell: f64,
        cutter: &dyn MillingCutter,
    ) -> Vec<f64> {
        let r = cutter.radius();
        let mut out = vec![f64::NAN; rows * cols];
        for row in 0..rows {
            for col in 0..cols {
                let mut best = f64::INFINITY;
                for sr in 0..rows {
                    for sc in 0..cols {
                        let d = ((sr as f64 - row as f64).powi(2)
                            + (sc as f64 - col as f64).powi(2))
                        .sqrt()
                            * cell;
                        if d > r + 1e-9 || !centres[sr * cols + sc] {
                            continue;
                        }
                        let Some(h) = cutter.height_at_radius(d) else {
                            continue;
                        };
                        best = best.min(floor[sr * cols + sc] + h);
                    }
                }
                if best.is_finite() {
                    out[row * cols + col] = best;
                }
            }
        }
        out
    }

    fn sample_floor(rows: usize, cols: usize) -> (Vec<f64>, Vec<bool>) {
        let floor = (0..rows * cols)
            .map(|i| {
                let (r, c) = ((i / cols) as f64, (i % cols) as f64);
                (r * 0.7).sin() * 3.0 + (c * 0.45).cos() * 2.0 + (r * c * 0.013)
            })
            .collect();
        // Centres only in the left two thirds, and not on every 5th row:
        // columns on the right edge have no centre in reach.
        let centres = (0..rows * cols)
            .map(|i| (i % cols) < cols * 2 / 3 && !(i / cols).is_multiple_of(5))
            .collect();
        (floor, centres)
    }

    #[expect(
        clippy::indexing_slicing,
        reason = "test code: indices bounded by the grid"
    )]
    fn assert_same_reach(cutter: &dyn MillingCutter) {
        let (rows, cols, cell) = (23, 31, 0.25);
        let (floor, centres) = sample_floor(rows, cols);
        let fast = reachable_floor(&floor, Some(&centres), rows, cols, cell, cutter);
        let slow = brute_reach(&floor, &centres, rows, cols, cell, cutter);
        let mut unreachable = 0;
        for i in 0..rows * cols {
            if slow[i].is_nan() {
                unreachable += 1;
                assert!(fast[i].is_nan(), "cell {i}: {} vs NAN", fast[i]);
            } else {
                assert!(
                    (fast[i] - slow[i]).abs() < 1e-12,
                    "cell {i}: {} vs {}",
                    fast[i],
                    slow[i]
                );
            }
        }
        assert!(unreachable > 0, "the fixture holds columns out of reach");
    }

    /// New pin (A3): the separable path (flat cutter, one sliding minimum
    /// per disc row) equals the minimum over the disc.
    #[test]
    fn the_flat_erosion_is_the_minimum_over_the_disc() {
        assert_same_reach(&FlatEndmill::new(2.0, 10.0));
    }

    /// New pin (A3): a ball profile adds its height at each radius (the
    /// bounded-window path).
    #[test]
    fn the_ball_erosion_adds_the_profile() {
        assert_same_reach(&BallEndmill::new(2.0, 10.0));
    }
}
