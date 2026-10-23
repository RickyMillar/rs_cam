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
}

/// Group the cells of `field` that stand more than `above_mm` above the
/// target into 4-connected islands, in row-major order of their first cell.
#[must_use]
pub fn standing_islands(field: &StandingField, above_mm: f64) -> Vec<StandingIsland> {
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

/// Where the tool of a rough can reach: the tool-centre polygons of its
/// boundary and the tool radius.
#[derive(Clone, Copy)]
pub struct RoughReach<'a> {
    /// The containment polygons the boundary clip keeps the tool centre in
    /// (the session's `boundary_centre`, G-BOUNDARYPHANTOM).
    pub centre: &'a [Polygon2],
    /// The cutting radius of the tool (mm): half its named diameter, the
    /// same radius the boundary clip uses.
    pub tool_radius_mm: f64,
}

/// The target of one 3D Rough, in the frame of its checkpoint stock.
#[derive(Clone, Copy)]
pub struct RoughTarget<'a> {
    pub mesh: &'a TriangleMesh,
    pub index: &'a SpatialIndex,
    /// The op's axial stock to leave (`stock_to_leave_axial`, mm).
    pub stock_to_leave_mm: f64,
    /// A pinned `bottom_z`, which floors the planner's level plan.
    pub pinned_floor_z: Option<f64>,
    /// `None`: the boundary is off, and every column of the grid is in the
    /// population.
    pub reach: Option<RoughReach<'a>>,
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
    /// Every island, tallest first. Empty is a measured zero.
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
                Some(tallest) => format!(
                    "coverage: {} island(s) stand more than {:.2} mm above the target \
                     (model + {:.2} mm stock to leave), {:.1} mm2 in all; the tallest is \
                     {:.2} mm high and {:.2} mm wide at ({:.1}, {:.1}).",
                    c.islands.len(),
                    c.bar_mm,
                    c.stock_to_leave_mm,
                    c.above_spec_area_mm2,
                    tallest.max_standing_mm,
                    tallest.min_width_mm,
                    tallest.peak[0],
                    tallest.peak[1],
                ),
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
        }
    }
}

/// Measure one 3D Rough on its checkpoint stock (G-ROUGHCOV).
///
/// # The target (the rough's own)
///
/// The 3D Rough planner (`adaptive3d/path.rs`, "Compute Z levels") cuts to
/// `surface + stock_to_leave`, and no level goes below
/// `max(surface_bottom + stock_to_leave, pinned bottom_z)`. Its surface
/// heightmap reads the mesh-bbox floor where the mesh has no surface
/// (through holes, outside the silhouette). So the target of a column is:
///
/// ```text
/// s      = topmost model surface Z at the column centre, or mesh.bbox.min.z
/// target = max(s + stock_to_leave, pinned bottom_z)
/// ```
///
/// The target of a through hole or of the band outside the part is the
/// floor, so a core left in a hole (G-HOLECORE) or a band left outside the
/// wall (G-OUTERBAND) reads as an island.
///
/// # Walls
///
/// The planner offsets in Z alone: `stock_to_leave_axial` is its only
/// leave dial, and L2 deleted the radial one (`compute/execute/rough_3d.rs`,
/// `adaptive3d_effective_stock_to_leave`). This reading is vertical too, so
/// it compares like with like. On a wall at slope `θ` the leave along the
/// normal is `stock_to_leave × cos θ`, and an island on a wall is
/// `max_standing × cos θ` thick along the normal. Material the tool shape
/// cannot reach (an inside corner narrower than the tool, a fillet under a
/// ball) stands above this target and reads as an island: it is material
/// the rough leaves, and its area and width say how much.
///
/// # The population (the rough's reach)
///
/// With the boundary on, a column is in the population when its centre is
/// within `tool_radius − cell` of the tool-centre polygons: the tool centre
/// stays inside them, and the cutter reaches one radius further. The cell
/// is taken off because the polygon mask is quantised to cell centres, so
/// the distance on the grid can exceed the true distance by up to one cell;
/// the population never holds a column the tool cannot reach. With the
/// boundary off, every column of the grid is in the population.
///
/// # The bar
///
/// PLAN.md's bar: "no uncut island bigger than the stock to leave". A cell
/// is above spec when `standing > stock_to_leave + one cell`. The one cell
/// is the G-OVERCUTREF tolerance (`stock/sim_triage.rs`,
/// `commanded_tolerance_mm`): a dexel column top is quantised to its cell.
/// There is no other threshold.
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
    let reach = target.reach.map(|reach| reach_mask(grid, reach));
    let (mesh, index, pinned) = (target.mesh, target.index, target.pinned_floor_z);
    let floor_z = mesh.bbox.min.z;
    let cols = grid.cols;
    let field = StandingField::walk(grid, |row, col, p| {
        if let Some(mask) = &reach
            && !mask.get(row * cols + col).copied().unwrap_or(false)
        {
            return None;
        }
        let surface =
            crate::maps::reach_map::surface_z_at(p.x, p.y, mesh, index).unwrap_or(floor_z);
        let z = surface + stl;
        Some(pinned.map_or(z, |floor| z.max(floor)))
    });
    let compared_cells = field.standing.iter().filter(|s| !s.is_nan()).count();
    let mut islands = standing_islands(&field, bar);
    islands.sort_by(|a, b| b.max_standing_mm.total_cmp(&a.max_standing_mm));
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

/// The population mask of a rough: the columns within `tool_radius − cell`
/// of the tool-centre polygons (see [`measure_rough_coverage`]).
fn reach_mask(grid: &DexelGrid, reach: RoughReach<'_>) -> Vec<bool> {
    let (rows, cols) = (grid.rows, grid.cols);
    let inside = centre_mask(grid, reach.centre);
    if !inside.iter().any(|c| *c) {
        return inside;
    }
    let dist = crate::geometry::grid_field::distance_transform_2d(&inside, rows, cols);
    let reach_cells = (reach.tool_radius_mm - grid.cell_size).max(0.0) / grid.cell_size;
    dist.iter().map(|d| *d <= reach_cells).collect()
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
            for pair in crossings.chunks_exact(2) {
                let [x0, x1] = [pair.first(), pair.get(1)].map(|x| x.copied().unwrap_or(0.0));
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
        RoughCoverageAbstention, RoughCoverageOutcome, RoughReach, RoughTarget, StandingField,
        measure_rough_coverage, standing_islands,
    };
    use crate::dexel_stock::TriDexelStock;
    use crate::geo::{BoundingBox3, P2, P3};
    use crate::mesh::{SpatialIndex, TriangleMesh};
    use crate::polygon::Polygon2;

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
        assert!((island.min_width_mm - 3.0 * cell).abs() < 1e-9, "{island:?}");
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
        let target = RoughTarget {
            mesh: &model,
            index: &index,
            stock_to_leave_mm: 0.5,
            pinned_floor_z: None,
            reach: Some(RoughReach {
                centre: &centre,
                tool_radius_mm: 3.0,
            }),
        };
        let RoughCoverageOutcome::Measured(c) = measure_rough_coverage(&stock, &target) else {
            panic!("expected a measured reading");
        };
        assert!(c.compared_cells > 0);
        assert!(c.islands.is_empty(), "{:?}", c.islands);
        // The reach is the centre square grown by R − cell = 2.75 mm. The
        // cells are counted on their centres, so the side holds one more
        // cell than the length.
        let side = 8.0 + 2.0 * 2.75 + cell;
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
        let target = RoughTarget {
            mesh: &model,
            index: &index,
            stock_to_leave_mm: 0.2,
            pinned_floor_z: None,
            reach: None,
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
}
