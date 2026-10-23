//! Research candidate (a) of the adaptive research 2026-10-23: the
//! geodesic-wavefront field for 2D adaptive clearing
//! (`planning/adaptive_research_2026-10-23/PLAN.md`, "Phase 2", row (a)).
//! Behind the `research` feature. No product path calls it.
//!
//! # The field
//!
//! `T(x)` is the geodesic distance from the entry disc to `x` inside the
//! tool-centre region. The holes (islands) and the outside are unreachable,
//! so the front goes round an island and grows into each lobe from the
//! cleared side. The EDT that `spiral.rs` contours measures the distance to
//! the WALL instead, so its wraps follow the walls (defect V3).
//!
//! - Solver ([`FieldSolver`], G1-3): source tracking. Each node keeps the
//!   point its straight path to the seed starts from (the seed centre, or
//!   the lattice node where the path bends round a wall) and reads its
//!   distance exactly from there, with the planner's own straight-move test
//!   (`ToolCentreRegion::contains_segment`). Where the seed centre sees a
//!   node the error is zero; behind a wall the bend sits on a lattice node,
//!   an over-read of at most one cell diagonal (measured 0.55 to 1.14
//!   cells), never an under-read. The G1 solver, first-order Godunov fast
//!   sweeping (`finish::scallop_isofield::fast_sweep`), stays selectable: its
//!   error depends on the direction to the grid (0 to 0.84 of a cell along
//!   the star's arm axes, up to 6.1 cells behind the pocket's island). The
//!   third-order WENO sweep of Zhang-Zhao-Qian 2005 was prototyped: 0.01
//!   cell in the open, but 3.9 cells behind the island (the rarefaction at
//!   the island's side is first order whatever the stencil). `T` is held in
//!   stepovers, so the integer levels are one stepover apart.
//! - Seed: the entry disc. The starter pocket (`path.rs`,
//!   `emit_helical_starter_pocket`) plunges at the EDT-max entry
//!   (`search.rs`, `find_entry_via_distance_transform`) and ends with one lap
//!   of the cutter centre at radius `R` (`starter_spiral_points`). That lap is
//!   the cleared boundary the front starts from, so the seed is the disc of
//!   radius `R` about the plunge point. Each node within one cell of the disc
//!   holds its exact signed distance `(|x - c| - R) / s` and is fixed, the way
//!   `seed_boundary` pins the region edge in the iso-scallop field.
//! - Grid: the planner's material-grid lattice, cell `max(R / 6, tolerance)`
//!   (`path.rs`, `planner_cell_size`, the one source of the planner
//!   lattice). A node is
//!   reachable when it is in the planner's machinable mask AND inside the
//!   exact tool-centre region (`ToolCentreRegion`), so a wrap point is legal
//!   to both readers.
//!
//! # The wraps
//!
//! Wrap `k` (k = 1, 2, ...) is the level set `T = k` stepovers: one stepover
//! outside the starter lap, then one stepover apart. This is the spacing
//! rule of the contour spiral (`spiral.rs`: `first_offset` is the starter rim,
//! then `off += stepover`). The level sets come from the interpolated
//! marching squares of the iso-scallop field (`iso_contour_where`), not the
//! midpoint marching squares of a boolean grid, because `T` is a scalar field
//! and the interpolation places each crossing to the field's precision.
//! A cell with an unreachable corner gives no segment, so a level set ENDS
//! where it meets a wall or an island: a wrap is an open arc there, and a
//! closed loop only while the front has not reached a wall. The strip within
//! about one cell of the wall is the wall pass's work by design.
//!
//! # The meeting line ([`MeetingRule`], G1-2)
//!
//! Behind an island the fronts that went round its two sides meet on a
//! line (a shock, R3 Source 3), and each level set has a kink there that
//! points back to the seed. A cutter that turns through the kink stands in
//! an inside corner of the stock. Source tracking names the side each node's
//! path went round (its front branch), so the meeting line is known exactly:
//! the cells whose corners lie in two branches. The rule: **the front
//! continues from one side only.** A level set is cut at the meeting line;
//! the branch cut first runs to the line, and the other branch ends one
//! stepover short of it. The trimmed piece lies within `√2 · s` of the first
//! branch's last cutter centre, inside its disc when `s ≤ R / √2`. The G1
//! form (`MeetingRule::Through`) stays selectable for the comparison.
//!
//! # The join and the order ([`JoinRule`])
//!
//! Under the swept-width measure a radial step of one stepover into the
//! next band reads about 0.94 of D, so every G1 seam was a re-entry. The
//! ramp join (`JoinRule::Ramp`) enters each band at a tilt the pass load
//! allows at the depth reached ([`EntryLaw`]: `β(d)` from `R`, `s` and the
//! ceiling):
//!
//! - The wraps that are whole circles round the seed (the circle of radius
//!   `R + k·s` lies inside the region, `k ≤ (clearance - R) / s`) are one
//!   spiral out of the starter rim, the radius growing by the law against
//!   the previous turn, then one closing lap. Inside that disc `T` is the
//!   Euclidean distance, so these wraps ARE the circles.
//! - Every other wrap is entered from its parent (the cut wrap one level
//!   lower nearest its start, or the closing lap): its points are pulled
//!   toward the parent until the law's depth reaches the gap. At s 2, R 3
//!   the ramp reaches full depth in 7.7 mm.
//! - The open arcs of a lobe are cut one way. A ramp leaves a wedge at the
//!   arc's start; the child arc starts at that end and first runs back over
//!   the parent from its full-depth point to its start (the back-run, which
//!   cuts the wedge at most one stepover deep), then ramps out. Zig-zag arcs
//!   would meet their parent's wedge with two stepovers of stock.
//! - The cutter goes from an arc's end to the next start by a keep-down
//!   `Link` over the cut stock; the replay admits it under the pass load or
//!   turns it into a re-entry.
//! - The order rule (grow outward from the cleared side): a wrap at level `k`
//!   is cut only after every wrap at level `k - 1` within the reach of it.
//!   Under the ramp join a child of the wrap just cut goes next; else the
//!   wrap nearest the cutter is next.
//! - The G1 join (`JoinRule::Seam`) stays selectable: a closed wrap lapped
//!   from a radial seam (`spiral::nearest_index`), an arc from its end
//!   nearer the cutter, a gap over the reach a keep-down `Link`.
//! - A wrap leaves the region nowhere: a vertex outside the tool-centre
//!   region or a segment that leaves it cuts the wrap (`inside_pieces`).
//!
//! # Engagement, the wall pass and the rest
//!
//! The segments go through the same pipeline as the agent and the contour
//! spiral: the cleanup strategy of the params (`apply_*_cleanup` in
//! `path.rs`) replays every `Cut` and `Link` under the pass load
//! (`emit_capped_walk_with`, `step_within_pass_load` along each step), splits
//! a step over the ceiling into a re-entry, then runs the residue contours,
//! the wall pass (`finish_walls_within_pass_load`) as the final pass, and the
//! mop. This module adds no insert. The contour spiral's trochoid insert is
//! not used: its loop radius `max(s, 0.4 R)` and pitch `0.6 s` are tuned
//! values with no derivation (operator ruling 2026-10-23), so an over-load
//! step here takes the replay split, the fallback every 2D producer has.
//!
//! # Derived constants
//!
//! - `reach = s + (1 + √2) · cell`: one level spacing, plus the field error
//!   (at most about one cell; the disc unit test pins one cell at 20 mm),
//!   plus one cell diagonal (the longest contour segment in one cell).
//! - A chain is closed when its two ends are at most one cell diagonal
//!   (`√2 · cell`) apart: `chain_segments` drops the closing point of a
//!   loop, so the gap is one contour segment.
//! - Fragment ends join within `CHAIN_EPS`, the chaining tolerance of
//!   `geometry::marching_squares`.
//! - The readout steps are the pass step of the planner (`path.rs`,
//!   `pass_step_len`: `3 · cell`).
//! - The entry law `β(d) = φ₀ + asin((c·D - d - R) / ρ)`, `ρ = √(h² + R²)`,
//!   `φ₀ = atan2(R, h)`, `h = √(R² - (R - s)²)` ([`EntryLaw`]): from `R`,
//!   `s` and the pass ceiling `c`. The ramp advances by it at the lattice
//!   resolution (each wrap is subdivided to one cell).
//! - The spiral levels `⌊(clearance - R) / s⌋`: the circles of radius
//!   `R + k·s` about the seed that the region holds.
//! - The spiral's angle step keeps each chord within the op tolerance of
//!   its arc: `Δφ = 2π / ⌈π / acos(1 - tolerance / ρ_outer)⌉`. Its turn
//!   cap is the radial span over the slowest growth per turn,
//!   `min(s, 2π R tan β(s))`, plus two turns (the closing lap and the
//!   rounding).
//! - The meeting-line trim is one stepover (the brief's rule); it is
//!   covered by the first branch's disc when `s ≤ R / √2`.
//!
//! # Known risks (R3, Source 3)
//!
//! - Rarefaction at the source: the front fans out of the entry disc. The
//!   exact seeding within one cell of the disc holds it.
//! - Rarefaction at an island's convex corner: the front fans round the
//!   corner. The first-order update over-reads there (2.5 cells behind the
//!   unit test's island, 6.1 behind the G1 pocket's); source tracking reads
//!   the fan exactly from its bend node (at most one cell diagonal over).
//! - Shocks: where two fronts meet (behind an island) the level set has a
//!   kink; [`MeetingRule::Split`] ends the wraps there. A meeting of two
//!   fronts that are not split by a wall (a lobe junction with no island)
//!   is one branch and is not split.
//! - The sweep cap: source tracking stops after `rows + cols` rounds (the
//!   longest lattice path; 3 rounds on the G1 shapes in the prototype);
//!   `fast_sweep` after 24 (`MAX_ROUNDS`, inherited).
//!   [`FieldReport::converged`] says when a cap stopped the sweep.
//! - An arc shorter than its ramp (7.7 mm at s 2, R 3) never reaches full
//!   depth; the next level then meets more than one stepover there, and the
//!   replay splits that step (the fallback every 2D producer has).

use std::f64::consts::SQRT_2;

use crate::finish::scallop_isofield::{IsoScallopField, fast_sweep, iso_contour_where};
use crate::geo::{P2, point_to_segment_distance};
use crate::geometry::marching_squares::CHAIN_EPS;
use crate::interrupt::{CancelCheck, Cancelled, check_cancel};
use crate::polygon::{Polygon2, offset_polygon};
use crate::toolpath::{MoveIntent, Toolpath};

use super::path::{
    AdaptiveSegment, LinkLoad, apply_contour_parallel_residue_cleanup, apply_residue_mop_cleanup,
    emit_helical_starter_pocket, pass_load_for, pass_step_len, planner_cell_size,
    segments_to_toolpath, subdivide,
};
use super::search::{ToolCentreRegion, measure_step, step_within_pass_load};
use super::{
    AdaptiveParams, AdaptiveRuntimeEvent, CleanupStrategy, KeepDownLinks, MaterialGrid,
    PathStrategy2d,
};

// ── The field ──────────────────────────────────────────────────────────

/// The lattice the field is solved on: node `(row, col)` stands at
/// `(origin_x + col · cell, origin_y + row · cell)`, the convention of
/// `MaterialGrid`.
#[derive(Debug, Clone, Copy)]
struct Lattice {
    rows: usize,
    cols: usize,
    origin_x: f64,
    origin_y: f64,
    cell: f64,
}

impl Lattice {
    /// The lattice `MaterialGrid::from_polygon` builds over `region`: its
    /// bounding box with a margin of one cell on each side.
    fn over(region: &Polygon2, cell: f64) -> Self {
        let (mut lo_x, mut lo_y) = (f64::INFINITY, f64::INFINITY);
        let (mut hi_x, mut hi_y) = (f64::NEG_INFINITY, f64::NEG_INFINITY);
        for p in &region.exterior {
            lo_x = lo_x.min(p.x);
            lo_y = lo_y.min(p.y);
            hi_x = hi_x.max(p.x);
            hi_y = hi_y.max(p.y);
        }
        let cols = ((hi_x - lo_x + 2.0 * cell) / cell).ceil() as usize + 1;
        let rows = ((hi_y - lo_y + 2.0 * cell) / cell).ceil() as usize + 1;
        Self {
            rows,
            cols,
            origin_x: lo_x - cell,
            origin_y: lo_y - cell,
            cell,
        }
    }

    fn of_grid(grid: &MaterialGrid) -> Self {
        Self {
            rows: grid.rows,
            cols: grid.cols,
            origin_x: grid.origin_x,
            origin_y: grid.origin_y,
            cell: grid.cell_size,
        }
    }

    fn node(&self, index: usize) -> P2 {
        let (row, col) = (index / self.cols, index % self.cols);
        P2::new(
            self.origin_x + col as f64 * self.cell,
            self.origin_y + row as f64 * self.cell,
        )
    }
}

/// Which solver computes the field (G1-3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldSolver {
    /// First-order Godunov upwind fast sweeping
    /// (`finish::scallop_isofield::fast_sweep`), the G1 solver. Its error
    /// depends on the direction to the grid axes: 0 on an axis, up to 0.84
    /// of a cell at 37 mm on the demo star's 54-degree arm (prototype,
    /// 2026-10-23), and up to 6.1 cells behind the G1 pocket's island.
    FirstOrder,
    /// Source tracking: each node keeps the point its straight path to the
    /// seed comes from (the seed centre, or the lattice node where the path
    /// bends round a wall) and reads its distance exactly from there. Zero
    /// error where the seed centre sees the node; behind a wall the bend
    /// sits on a lattice node, an over-read of at most one cell diagonal
    /// (measured 0.55 cell behind a 6 mm island, 1.14 cells behind the G1
    /// pocket's island). Never an under-read.
    SourceTracking,
}

/// The straight path of a node to the seed disc starts at `at`, where the
/// distance is `t_mm` (the seed centre, `-R`; or a bend node). `branch`
/// names the side of each wall the path went round ([`Branches`]).
#[derive(Debug, Clone, Copy)]
struct Pivot {
    at: P2,
    t_mm: f64,
    branch: u32,
}

/// The branches of the front (G1-2). Branch 0 is every node the seed centre
/// sees. A path that bends round a wall ring on one side starts a new
/// branch, keyed by (parent branch, ring, left turn); further bends round
/// the same ring on the same side stay in it. Two neighbouring nodes in
/// different branches lie on the two sides of a meeting line (a shock: the
/// fronts that went round the two sides of an island meet there).
#[derive(Debug, Default)]
struct Branches {
    keys: Vec<(u32, usize, bool)>,
}

impl Branches {
    fn key(&self, id: u32) -> Option<(u32, usize, bool)> {
        let index = (id as usize).checked_sub(1)?;
        self.keys.get(index).copied()
    }

    fn bend(&mut self, parent: u32, ring: usize, left: bool) -> u32 {
        if self
            .key(parent)
            .is_some_and(|(_, r, l)| r == ring && l == left)
        {
            return parent;
        }
        let key = (parent, ring, left);
        let index = match self.keys.iter().position(|&k| k == key) {
            Some(i) => i,
            None => {
                self.keys.push(key);
                self.keys.len() - 1
            }
        };
        (index + 1) as u32
    }
}

/// The walls a straight path must not cross: the exact tool-centre region,
/// the planner's own test (`ToolCentreRegion::contains_segment`).
struct Walls<'a> {
    region: &'a ToolCentreRegion,
}

impl Walls<'_> {
    fn visible(&self, a: P2, b: P2) -> bool {
        self.region.contains_segment(a, b)
    }

    /// The index of the ring (exterior or hole, over every piece) nearest
    /// `p`.
    fn ring_near(&self, p: P2) -> usize {
        let mut best = (f64::INFINITY, 0usize);
        let rings = self
            .region
            .pieces()
            .iter()
            .flat_map(|piece| std::iter::once(&piece.exterior).chain(piece.holes.iter()));
        for (index, ring) in rings.enumerate() {
            let n = ring.len();
            for k in 0..n {
                let (Some(a), Some(b)) = (ring.get(k), ring.get((k + 1) % n)) else {
                    continue;
                };
                let d = point_to_segment_distance(&p, a, b);
                if d < best.0 {
                    best = (d, index);
                }
            }
        }
        best.1
    }
}

/// The eight lattice neighbours.
const NEIGHBOURS: [(isize, isize); 8] = [
    (-1, -1),
    (-1, 0),
    (-1, 1),
    (0, -1),
    (0, 1),
    (1, -1),
    (1, 0),
    (1, 1),
];

/// [`FieldSolver::SourceTracking`] on `lattice`. `t_mm` holds the seeded
/// distances (mm) and `INFINITY` elsewhere; `fixed` marks the seed nodes,
/// whose pivot is the seed centre. Gauss-Seidel sweeps in the four orders
/// of `fast_sweep`. A node takes, from each neighbour, the neighbour's
/// pivot when the pivot sees the node (the exact distance from it), else
/// the neighbour itself as a new bend pivot (when it sees the node). A
/// value only falls, and the candidates are finite, so the sweep ends; the
/// round cap is the lattice's `rows + cols`, the longest 4-connected
/// lattice path, since a round moves the front at least one node. Returns
/// the branch of each node and true when a round changed nothing.
fn track_sources(
    lattice: &Lattice,
    inside: &[bool],
    fixed: &[bool],
    t_mm: &mut [f64],
    seed: P2,
    seed_radius: f64,
    walls: &Walls<'_>,
) -> (Vec<u32>, bool) {
    let (rows, cols) = (lattice.rows, lattice.cols);
    let mut branches = Branches::default();
    let mut pivots: Vec<Option<Pivot>> = fixed
        .iter()
        .map(|&f| {
            f.then_some(Pivot {
                at: seed,
                t_mm: -seed_radius,
                branch: 0,
            })
        })
        .collect();
    let dist = |a: P2, b: P2| (a.x - b.x).hypot(a.y - b.y);
    let orders = [(true, true), (true, false), (false, true), (false, false)];
    let mut converged = false;
    for _round in 0..rows + cols {
        let mut changed = false;
        for &(row_up, col_up) in &orders {
            for ri in 0..rows {
                let row = if row_up { ri } else { rows - 1 - ri };
                for ci in 0..cols {
                    let col = if col_up { ci } else { cols - 1 - ci };
                    let i = row * cols + col;
                    if !inside.get(i).copied().unwrap_or(false)
                        || fixed.get(i).copied().unwrap_or(true)
                    {
                        continue;
                    }
                    let x = lattice.node(i);
                    let mut best = t_mm.get(i).copied().unwrap_or(f64::INFINITY);
                    let mut best_pivot: Option<Pivot> = None;
                    for (dr, dc) in NEIGHBOURS {
                        let (Some(nr), Some(nc)) =
                            (row.checked_add_signed(dr), col.checked_add_signed(dc))
                        else {
                            continue;
                        };
                        if nr >= rows || nc >= cols {
                            continue;
                        }
                        let j = nr * cols + nc;
                        let Some(Some(pivot)) = pivots.get(j).copied() else {
                            continue;
                        };
                        let via = pivot.t_mm + dist(x, pivot.at);
                        if via < best && walls.visible(pivot.at, x) {
                            best = via;
                            best_pivot = Some(pivot);
                            continue;
                        }
                        let n = lattice.node(j);
                        let t_n = t_mm.get(j).copied().unwrap_or(f64::INFINITY);
                        let bend = t_n + dist(x, n);
                        if bend < best && walls.visible(n, x) {
                            // The turn at `n` from the pivot's leg to `x`.
                            let turn =
                                (n.x - pivot.at.x) * (x.y - n.y) - (n.y - pivot.at.y) * (x.x - n.x);
                            let branch = if turn == 0.0 {
                                pivot.branch
                            } else {
                                branches.bend(pivot.branch, walls.ring_near(n), turn > 0.0)
                            };
                            best = bend;
                            best_pivot = Some(Pivot {
                                at: n,
                                t_mm: t_n,
                                branch,
                            });
                        }
                    }
                    if let (Some(pivot), Some(t), Some(slot)) =
                        (best_pivot, t_mm.get_mut(i), pivots.get_mut(i))
                    {
                        *t = best;
                        *slot = Some(pivot);
                        changed = true;
                    }
                }
            }
        }
        if !changed {
            converged = true;
            break;
        }
    }
    let branch = pivots.iter().map(|p| p.map_or(0, |p| p.branch)).collect();
    (branch, converged)
}

/// The geodesic distance from an entry disc over a region whose holes and
/// outside are unreachable, in stepovers.
pub struct GeodesicField {
    /// The solved field. `passes` holds `T / stepover`, negative inside the
    /// seed disc and `INFINITY` at an unreachable node; `ring_count` is the
    /// number of wraps, `⌊max T / stepover⌋`.
    field: IsoScallopField,
    inside: Vec<bool>,
    /// The front branch of each node ([`Branches`]); all 0 under
    /// [`FieldSolver::FirstOrder`].
    branch: Vec<u32>,
    stepover: f64,
    converged: bool,
}

impl GeodesicField {
    /// Solve the field over the tool-centre region of `part` for a cutter of
    /// `tool_radius` (the region the planner reads, `ToolCentreRegion`),
    /// from the disc of `seed_radius` about `seed`, on the planner's
    /// lattice of `cell` over `part` (`MaterialGrid::from_polygon`). `None`
    /// when no reachable node lies within one cell of the disc.
    #[must_use]
    pub fn over_part(
        part: &Polygon2,
        tool_radius: f64,
        cell: f64,
        stepover: f64,
        seed: P2,
        seed_radius: f64,
        solver: FieldSolver,
    ) -> Option<Self> {
        let region = ToolCentreRegion::new(part, tool_radius);
        let lattice = Lattice::over(part, cell);
        let inside = (0..lattice.rows * lattice.cols)
            .map(|i| region.contains(&lattice.node(i)))
            .collect();
        let walls = Walls { region: &region };
        Self::on_lattice(lattice, inside, seed, seed_radius, stepover, solver, &walls)
    }

    fn on_lattice(
        lattice: Lattice,
        inside: Vec<bool>,
        seed: P2,
        seed_radius: f64,
        stepover: f64,
        solver: FieldSolver,
        walls: &Walls<'_>,
    ) -> Option<Self> {
        let n = lattice.rows * lattice.cols;
        let mut t_mm = vec![f64::INFINITY; n];
        let mut fixed = vec![false; n];
        // The seed: every reachable node within one cell of the disc holds
        // its exact signed distance from the disc rim and is fixed.
        let mut seeded = false;
        for (i, ((t, f), &reach)) in t_mm
            .iter_mut()
            .zip(fixed.iter_mut())
            .zip(&inside)
            .enumerate()
        {
            if !reach {
                continue;
            }
            let p = lattice.node(i);
            let d = (p.x - seed.x).hypot(p.y - seed.y);
            if d <= seed_radius + lattice.cell {
                *t = d - seed_radius;
                *f = true;
                seeded = true;
            }
        }
        if !seeded {
            return None;
        }
        let speed: Vec<f64> = inside
            .iter()
            .map(|&reach| if reach { stepover } else { f64::NAN })
            .collect();
        let (passes, branch, converged) = match solver {
            FieldSolver::FirstOrder => {
                let mut passes: Vec<f64> = t_mm.iter().map(|t| t / stepover).collect();
                let converged = fast_sweep(
                    &mut passes,
                    &inside,
                    &fixed,
                    &speed,
                    lattice.rows,
                    lattice.cols,
                    lattice.cell,
                );
                (passes, vec![0; n], converged)
            }
            FieldSolver::SourceTracking => {
                let (branch, converged) = track_sources(
                    &lattice,
                    &inside,
                    &fixed,
                    &mut t_mm,
                    seed,
                    seed_radius,
                    walls,
                );
                (
                    t_mm.iter().map(|t| t / stepover).collect(),
                    branch,
                    converged,
                )
            }
        };
        let max_pass = passes
            .iter()
            .copied()
            .filter(|v| v.is_finite())
            .fold(0.0_f64, f64::max);
        let field = IsoScallopField {
            rows: lattice.rows,
            cols: lattice.cols,
            origin_x: lattice.origin_x,
            origin_y: lattice.origin_y,
            cell: lattice.cell,
            stepover_mm: speed,
            passes,
            ring_count: max_pass.floor() as usize,
        };
        Some(Self {
            field,
            inside,
            branch,
            stepover,
            converged,
        })
    }

    /// The geodesic distance (mm) at the lattice node nearest `p`, `None`
    /// at an unreachable or unsolved node.
    #[must_use]
    pub fn distance_mm(&self, p: P2) -> Option<f64> {
        let i = self.nearest_node(p)?;
        let t = *self.field.passes.get(i)?;
        (self.inside.get(i).copied() == Some(true) && t.is_finite()).then_some(t * self.stepover)
    }

    fn nearest_node(&self, p: P2) -> Option<usize> {
        let f = &self.field;
        let col = ((p.x - f.origin_x) / f.cell).round();
        let row = ((p.y - f.origin_y) / f.cell).round();
        if col < 0.0 || row < 0.0 {
            return None;
        }
        let (row, col) = (row as usize, col as usize);
        (row < f.rows && col < f.cols).then_some(row * f.cols + col)
    }

    /// The number of wraps: `⌊max T / stepover⌋`.
    #[must_use]
    pub fn level_count(&self) -> usize {
        self.field.ring_count
    }

    /// False when the sweep cap stopped the solve before it converged.
    #[must_use]
    pub fn converged(&self) -> bool {
        self.converged
    }

    /// The lattice cell (mm).
    #[must_use]
    pub fn cell(&self) -> f64 {
        self.field.cell
    }

    /// The number of distinct front branches over the reachable nodes.
    #[must_use]
    pub fn branch_count(&self) -> usize {
        let mut seen: Vec<u32> = self
            .branch
            .iter()
            .zip(&self.inside)
            .filter(|&(_, &reach)| reach)
            .map(|(&b, _)| b)
            .collect();
        seen.sort_unstable();
        seen.dedup();
        seen.len()
    }

    fn reachable(&self, row: usize, col: usize) -> bool {
        self.inside.get(row * self.field.cols + col).copied() == Some(true)
    }

    /// True when the four corners of the cell at `(row, col)` are reachable.
    fn cell_reachable(&self, row: usize, col: usize) -> bool {
        self.reachable(row, col)
            && self.reachable(row, col + 1)
            && self.reachable(row + 1, col)
            && self.reachable(row + 1, col + 1)
    }

    /// True when the corners of the cell at `(row, col)` lie in more than one
    /// front branch: the cell holds a meeting line.
    fn cell_on_shock(&self, row: usize, col: usize) -> bool {
        let cols = self.field.cols;
        let at = |r: usize, c: usize| self.branch.get(r * cols + c).copied();
        let first = at(row, col);
        [at(row, col + 1), at(row + 1, col), at(row + 1, col + 1)]
            .iter()
            .any(|&b| b != first)
    }

    /// The level set `T = level` stepovers, over the cells whose four
    /// corners are reachable, with the fragments of each open arc joined.
    /// Under [`MeetingRule::Split`] the cells on a meeting line are left out
    /// too, so a wrap ends at the meeting line instead of turning through
    /// its kink.
    fn level_chains(&self, level: usize, meeting: MeetingRule) -> Vec<Vec<P2>> {
        let kept = |row: usize, col: usize| {
            self.cell_reachable(row, col)
                && (meeting == MeetingRule::Through || !self.cell_on_shock(row, col))
        };
        join_fragments(iso_contour_where(&self.field, level as f64, &kept))
    }
}

/// True when `a` and `b` are the same chaining point.
fn coincide(a: P2, b: P2) -> bool {
    (a.x - b.x).hypot(a.y - b.y) <= CHAIN_EPS
}

/// `chain_segments` extends a chain at its tail only, so an open arc whose
/// first segment lies mid-arc comes back as two or more pieces. Join the
/// pieces whose ends coincide.
fn join_fragments(mut pool: Vec<Vec<P2>>) -> Vec<Vec<P2>> {
    let mut out = Vec::with_capacity(pool.len());
    while let Some(mut chain) = pool.pop() {
        while let (Some(&head), Some(&tail)) = (chain.first(), chain.last()) {
            let found = pool.iter().position(|other| {
                let (Some(&a), Some(&b)) = (other.first(), other.last()) else {
                    return false;
                };
                coincide(tail, a) || coincide(tail, b) || coincide(head, a) || coincide(head, b)
            });
            let Some(at) = found else {
                break;
            };
            let mut other = pool.swap_remove(at);
            let (Some(&a), Some(&b)) = (other.first(), other.last()) else {
                continue;
            };
            if coincide(tail, a) {
                chain.extend(other.into_iter().skip(1));
            } else if coincide(tail, b) {
                other.reverse();
                chain.extend(other.into_iter().skip(1));
            } else if coincide(head, b) {
                other.extend(chain.into_iter().skip(1));
                chain = other;
            } else {
                other.reverse();
                other.extend(chain.into_iter().skip(1));
                chain = other;
            }
        }
        out.push(chain);
    }
    out
}

// ── The wrap order ─────────────────────────────────────────────────────

/// One wrap piece: a closed loop or an open arc of one level set.
struct Chain {
    level: usize,
    pts: Vec<P2>,
    closed: bool,
    lo: P2,
    hi: P2,
    /// Whether the first and the last point end at a meeting line
    /// ([`MeetingRule::Split`]); false for a loop.
    shock: [bool; 2],
}

impl Chain {
    fn new(level: usize, pts: Vec<P2>, closed: bool) -> Self {
        let mut lo = P2::new(f64::INFINITY, f64::INFINITY);
        let mut hi = P2::new(f64::NEG_INFINITY, f64::NEG_INFINITY);
        for p in &pts {
            lo = P2::new(lo.x.min(p.x), lo.y.min(p.y));
            hi = P2::new(hi.x.max(p.x), hi.y.max(p.y));
        }
        Self {
            level,
            pts,
            closed,
            lo,
            hi,
            shock: [false; 2],
        }
    }

    /// The segments of the chain, with the closing one for a loop.
    fn segments(&self) -> impl Iterator<Item = (P2, P2)> + '_ {
        let closing = self
            .closed
            .then(|| self.pts.last().copied().zip(self.pts.first().copied()))
            .flatten();
        self.pts
            .windows(2)
            .filter_map(|w| match w {
                &[a, b] => Some((a, b)),
                _ => None,
            })
            .chain(closing)
    }

    /// True when a point of `self` lies within `reach` of `other`.
    fn within(&self, other: &Self, reach: f64) -> bool {
        if self.lo.x > other.hi.x + reach
            || other.lo.x > self.hi.x + reach
            || self.lo.y > other.hi.y + reach
            || other.lo.y > self.hi.y + reach
        {
            return false;
        }
        self.pts.iter().any(|p| {
            other
                .segments()
                .any(|(a, b)| point_to_segment_distance(p, &a, &b) <= reach)
        })
    }

    /// The distance from `p` to where the chain would be entered: the
    /// nearer end of an arc, the nearest point of a loop.
    fn entry_distance(&self, p: P2) -> f64 {
        let d = |q: &P2| (q.x - p.x).hypot(q.y - p.y);
        if self.closed {
            self.pts.iter().map(d).fold(f64::INFINITY, f64::min)
        } else {
            let a = self.pts.first().map_or(f64::INFINITY, d);
            let b = self.pts.last().map_or(f64::INFINITY, d);
            a.min(b)
        }
    }

    /// The cutting order of the chain entered from `cur`: an arc from its
    /// nearer end; a loop rotated to its point nearest `cur` and closed on
    /// that seam (`spiral.rs`).
    fn oriented(&self, cur: P2) -> Vec<P2> {
        if self.closed {
            let seam = super::spiral::nearest_index(&self.pts, cur);
            let n = self.pts.len();
            (0..=n)
                .filter_map(|k| self.pts.get((seam + k) % n).copied())
                .collect()
        } else {
            let d = |q: Option<&P2>| q.map_or(f64::INFINITY, |q| (q.x - cur.x).hypot(q.y - cur.y));
            let mut pts = self.pts.clone();
            if d(self.pts.last()) < d(self.pts.first()) {
                pts.reverse();
            }
            pts
        }
    }
}

/// True when a level chain is a closed loop: its two ends are at most one
/// cell diagonal apart (`chain_segments` drops the closing point of a loop,
/// so the gap is one contour segment).
fn is_closed(pts: &[P2], cell: f64) -> bool {
    match (pts.first(), pts.last()) {
        (Some(&a), Some(&b)) => (a.x - b.x).hypot(a.y - b.y) <= SQRT_2 * cell + CHAIN_EPS,
        _ => false,
    }
}

/// The pieces of a level chain that lie inside the tool-centre region (G1-1).
///
/// The level set is contoured over cells whose four corners are reachable,
/// so every vertex lies on a cell edge between two legal nodes. Where the
/// region boundary is concave inside one cell (an island's inset arc, a
/// reflex corner's arc) that edge can still leave the region. The chain is
/// cut there: a vertex outside the region, or a segment that leaves it
/// (`ToolCentreRegion::contains_segment`, the planner's own test), ends the
/// piece. A loop that loses no vertex stays one closed piece; every other
/// piece is an open arc. Pieces shorter than two points are dropped.
fn inside_pieces(pts: Vec<P2>, closed: bool, region: &ToolCentreRegion) -> Vec<(Vec<P2>, bool)> {
    let n = pts.len();
    let ok_point: Vec<bool> = pts.iter().map(|p| region.contains(p)).collect();
    // Segment k joins point k to point k + 1 (and, for a loop, n - 1 to 0).
    let segs = if closed { n } else { n.saturating_sub(1) };
    let ok_seg: Vec<bool> = (0..segs)
        .map(|k| match (pts.get(k), pts.get((k + 1) % n)) {
            (Some(&a), Some(&b)) => region.contains_segment(a, b),
            _ => false,
        })
        .collect();
    if ok_point.iter().all(|&v| v) && ok_seg.iter().all(|&v| v) {
        return vec![(pts, closed)];
    }
    // Walk from a point after a break, so a loop's pieces do not wrap the
    // index end.
    let start = if closed {
        (0..n)
            .find(|&k| {
                let prev = (k + n - 1) % n;
                ok_point.get(k).copied().unwrap_or(false)
                    && !(ok_point.get(prev).copied().unwrap_or(false)
                        && ok_seg.get(prev).copied().unwrap_or(false))
            })
            .unwrap_or(0)
    } else {
        0
    };
    let mut out = Vec::new();
    let mut run: Vec<P2> = Vec::new();
    let flush = |run: &mut Vec<P2>, out: &mut Vec<(Vec<P2>, bool)>| {
        if run.len() >= 2 {
            out.push((std::mem::take(run), false));
        } else {
            run.clear();
        }
    };
    for step in 0..n {
        let k = (start + step) % n;
        let (Some(&p), Some(&inside)) = (pts.get(k), ok_point.get(k)) else {
            continue;
        };
        if !inside {
            flush(&mut run, &mut out);
            continue;
        }
        let prev = (k + n - 1) % n;
        let joined = step > 0 && ok_seg.get(prev).copied().unwrap_or(false);
        if !joined {
            flush(&mut run, &mut out);
        }
        run.push(p);
    }
    flush(&mut run, &mut out);
    out
}

/// How a wrap meets the line where two fronts meet behind an island (G1-2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeetingRule {
    /// The G1 form: the level set runs through the kink on the meeting line
    /// as one wrap, and the cutter turns there.
    Through,
    /// The front continues from one side only. A level set is cut at the
    /// meeting line into its two branches (the cells whose corners lie in
    /// two front branches give no segment). The branch cut first runs to
    /// the line; the other ends one stepover short of it. The trimmed
    /// piece lies within `√2 · s` of the first branch's last cutter centre
    /// (one stepover along the line, one stepover across the band), inside
    /// its disc when `s ≤ R / √2` (2 ≤ 2.12 at s 2, R 3), so the first
    /// branch has cut it. Any rest is an ordinary pass of the next level.
    Split,
}

/// How one wrap joins the next (the plunge per wrap).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JoinRule {
    /// The G1 form: a closed wrap is lapped from a radial seam (the
    /// shipped spiral's seam, `spiral::nearest_index`); an open arc is cut
    /// from its end nearer the cutter; a gap of more than the reach is a
    /// keep-down link. The radial seam step reads about 0.94 of D under
    /// the swept-width measure, so the replay re-enters there.
    Seam,
    /// The entry law ([`EntryLaw`]): the closed wraps round the seed are one
    /// spiral out of the starter rim and a closing lap; every other wrap is
    /// entered from its parent wrap by a ramp whose tilt holds the pass
    /// load at each depth; the open arcs of a lobe are cut one way.
    Ramp,
}

/// The research options of candidate (a).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GeodesicOptions {
    pub solver: FieldSolver,
    pub meeting: MeetingRule,
    pub join: JoinRule,
}

impl GeodesicOptions {
    /// The form the operator saw at G1 (2026-10-23).
    pub const G1: Self = Self {
        solver: FieldSolver::FirstOrder,
        meeting: MeetingRule::Through,
        join: JoinRule::Seam,
    };
    /// The G1 fixes of editor C2.
    pub const C2: Self = Self {
        solver: FieldSolver::SourceTracking,
        meeting: MeetingRule::Split,
        join: JoinRule::Ramp,
    };
}

/// Where a wrap's ramp starts from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParentRef {
    /// No parent wrap within the reach: the wrap is entered as it is.
    Free,
    /// The closing lap of the spiral (or the starter rim).
    Rim,
    /// An earlier wrap, by its index in the cutting order.
    Wrap(usize),
}

/// One wrap in cutting order.
struct Wrap {
    /// The level set, oriented for cutting (a loop closed on its seam).
    pts: Vec<P2>,
    closed: bool,
    parent: ParentRef,
}

/// True when `p` (a chain end) lies next to a cell on a meeting line: the
/// 3 x 3 cells round the cell that holds `p`.
fn end_at_shock(field: &GeodesicField, p: P2) -> bool {
    let f = &field.field;
    let col = ((p.x - f.origin_x) / f.cell).floor();
    let row = ((p.y - f.origin_y) / f.cell).floor();
    if col < 0.0 || row < 0.0 {
        return false;
    }
    let (row, col) = (row as usize, col as usize);
    (row.saturating_sub(1)..=row + 1).any(|r| {
        (col.saturating_sub(1)..=col + 1).any(|c| {
            r + 1 < f.rows
                && c + 1 < f.cols
                && field.cell_reachable(r, c)
                && field.cell_on_shock(r, c)
        })
    })
}

/// `pts` with arc length `len` removed from its end.
fn trim_end(pts: &mut Vec<P2>, mut len: f64) {
    while len > 0.0 && pts.len() >= 2 {
        let (Some(&b), Some(&a)) = (pts.last(), pts.get(pts.len() - 2)) else {
            return;
        };
        let seg = (b.x - a.x).hypot(b.y - a.y);
        if seg > len {
            let t = (seg - len) / seg;
            if let Some(last) = pts.last_mut() {
                *last = P2::new(a.x + t * (b.x - a.x), a.y + t * (b.y - a.y));
            }
            return;
        }
        pts.pop();
        len -= seg;
    }
}

fn polyline_length(pts: &[P2]) -> f64 {
    pts.windows(2)
        .filter_map(|w| match w {
            &[a, b] => Some((b.x - a.x).hypot(b.y - a.y)),
            _ => None,
        })
        .sum()
}

/// The point of the polyline `poly` nearest `p`, and its distance.
fn nearest_on(poly: &[P2], p: P2) -> Option<(P2, f64)> {
    let mut best: Option<(P2, f64)> = None;
    let mut take = |q: P2| {
        let d = (q.x - p.x).hypot(q.y - p.y);
        if best.is_none_or(|(_, b)| d < b) {
            best = Some((q, d));
        }
    };
    if let [only] = poly {
        take(*only);
    }
    for w in poly.windows(2) {
        let &[a, b] = w else {
            continue;
        };
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        let len_sq = dx * dx + dy * dy;
        let t = if len_sq > 0.0 {
            (((p.x - a.x) * dx + (p.y - a.y) * dy) / len_sq).clamp(0.0, 1.0)
        } else {
            0.0
        };
        take(P2::new(a.x + t * dx, a.y + t * dy));
    }
    best
}

/// The wraps of `field` from level `min_level` up, in cutting order from
/// `start` (module doc, "The join and the order"), each oriented for
/// cutting, every vertex and segment inside `region` ([`inside_pieces`]).
#[expect(
    clippy::too_many_arguments,
    reason = "each parameter is a separate input of one planning call"
)]
fn ordered_wraps(
    field: &GeodesicField,
    region: &ToolCentreRegion,
    start: P2,
    reach: f64,
    stepover: f64,
    min_level: usize,
    options: GeodesicOptions,
    cancel: &dyn CancelCheck,
) -> Result<Vec<Wrap>, Cancelled> {
    let cell = field.cell();
    let mut chains: Vec<Chain> = Vec::new();
    for level in min_level.max(1)..=field.level_count() {
        check_cancel(cancel)?;
        for pts in field.level_chains(level, options.meeting) {
            if pts.len() < 2 {
                continue;
            }
            let closed = is_closed(&pts, cell);
            for (piece, closed) in inside_pieces(pts, closed, region) {
                let mut chain = Chain::new(level, piece, closed);
                if options.meeting == MeetingRule::Split && !closed {
                    chain.shock = [
                        chain.pts.first().is_some_and(|&p| end_at_shock(field, p)),
                        chain.pts.last().is_some_and(|&p| end_at_shock(field, p)),
                    ];
                }
                chains.push(chain);
            }
        }
    }
    // The wraps one level lower within reach: each must be cut first.
    let mut parents: Vec<Vec<usize>> = Vec::with_capacity(chains.len());
    for chain in &chains {
        check_cancel(cancel)?;
        parents.push(
            chains
                .iter()
                .enumerate()
                .filter(|(_, p)| p.level + 1 == chain.level && chain.within(p, reach))
                .map(|(j, _)| j)
                .collect(),
        );
    }
    let ramp = options.join == JoinRule::Ramp;
    let mut done = vec![false; chains.len()];
    // Chain index -> its index in the output.
    let mut placed: Vec<Option<usize>> = vec![None; chains.len()];
    // The shock ends already cut, per level.
    let mut cut_shock_ends: Vec<(usize, P2)> = Vec::new();
    let mut out: Vec<Wrap> = Vec::with_capacity(chains.len());
    let mut cur = start;
    let mut last: Option<usize> = None;
    for _ in 0..chains.len() {
        let is_done = |j: &usize| done.get(*j).copied().unwrap_or(true);
        let ready =
            |i: usize| !is_done(&i) && parents.get(i).is_some_and(|ps| ps.iter().all(is_done));
        // Under the ramp join a child of the wrap just cut goes next, so a
        // lobe is cut to its end before the cutter leaves it.
        let child_of_last = |i: usize| {
            ramp && last.is_some_and(|l| parents.get(i).is_some_and(|ps| ps.contains(&l)))
        };
        let pick = |only_children: bool| {
            chains
                .iter()
                .enumerate()
                .filter(|&(i, _)| ready(i) && (!only_children || child_of_last(i)))
                .map(|(i, c)| (i, c.entry_distance(cur)))
                .min_by(|a, b| a.1.total_cmp(&b.1))
        };
        // The lowest level left always qualifies (its parents are all cut),
        // so this ends only when every chain is cut.
        let Some((i, _)) = pick(true).or_else(|| pick(false)) else {
            break;
        };
        let (Some(flag), Some(chain), Some(ps)) = (done.get_mut(i), chains.get(i), parents.get(i))
        else {
            break;
        };
        *flag = true;
        // Orientation.
        // One way: a child of the open arc just cut starts at the end
        // nearer that arc's start, where the back-run and the ramp are.
        let one_way_from = match last {
            Some(l) if ramp && !chain.closed && ps.contains(&l) => chains
                .get(l)
                .filter(|c| !c.closed)
                .and_then(|_| placed.get(l).copied().flatten())
                .and_then(|o| out.get(o))
                .and_then(|w| w.pts.first().copied()),
            _ => None,
        };
        let mut pts = match one_way_from {
            Some(parent_start) => chain.oriented(parent_start),
            None => chain.oriented(cur),
        };
        let reversed = !chain.closed && pts.first() != chain.pts.first();
        // The meeting-line rule: the branch cut second ends one stepover
        // short of the line.
        if options.meeting == MeetingRule::Split && !chain.closed {
            let [first_shock, last_shock] = if reversed {
                [chain.shock[1], chain.shock[0]]
            } else {
                chain.shock
            };
            let met = |p: Option<&P2>| {
                p.is_some_and(|&p| {
                    cut_shock_ends.iter().any(|&(level, q)| {
                        level == chain.level && (q.x - p.x).hypot(q.y - p.y) <= reach
                    })
                })
            };
            let trim_last = last_shock && met(pts.last());
            let trim_first = first_shock && met(pts.first());
            for (end, shock) in [
                (pts.first().copied(), first_shock),
                (pts.last().copied(), last_shock),
            ] {
                if let (Some(p), true) = (end, shock) {
                    cut_shock_ends.push((chain.level, p));
                }
            }
            let keep = polyline_length(&pts)
                - stepover * f64::from(u8::from(trim_last) + u8::from(trim_first));
            if keep > 0.0 {
                if trim_last {
                    trim_end(&mut pts, stepover);
                }
                if trim_first {
                    pts.reverse();
                    trim_end(&mut pts, stepover);
                    pts.reverse();
                }
            }
        }
        // The parent the wrap is entered from: the cut parent nearest its
        // first point; the rim for the lowest level.
        let parent = match pts.first() {
            Some(&p0) if ramp => ps
                .iter()
                .filter_map(|&j| placed.get(j).copied().flatten())
                .filter_map(|o| {
                    out.get(o)
                        .and_then(|w| nearest_on(&w.pts, p0))
                        .map(|(_, d)| (o, d))
                })
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map_or(
                    if chain.level == min_level {
                        ParentRef::Rim
                    } else {
                        ParentRef::Free
                    },
                    |(o, _)| ParentRef::Wrap(o),
                ),
            _ => ParentRef::Free,
        };
        if let Some(&end) = pts.last() {
            cur = end;
        }
        if let Some(slot) = placed.get_mut(i) {
            *slot = Some(out.len());
        }
        out.push(Wrap {
            pts,
            closed: chain.closed,
            parent,
        });
        last = Some(i);
    }
    Ok(out)
}

/// The entry law of the ramp join: how steeply the cutter may cross into
/// the next band at each depth and still hold the pass load.
///
/// A step that heads at tilt `β` into a band of depth `d` (the cleared edge
/// parallel to the wrap) holds the cap of the disc beyond `R - d`. Its
/// sideways extent, the swept width the planner reads (`compute_swept_width`),
/// is at most `d + R (1 - cos β) + h sin β`, with `h = √(R² - (R - s)²)` the
/// half chord at the full depth `s` (the cap's far end projects to at most
/// `R`, its near corner to at least `(R - d) cos β - h sin β`). The step
/// holds the ceiling `c` (a_e / D) when that is at most `c · D`:
/// `h sin β - R cos β ≤ c·D - d - R`, so
/// `β(d) = φ₀ + asin((c·D - d - R) / ρ)`, `ρ = √(h² + R²)`, `φ₀ = atan2(R, h)`.
/// At s 2, R 3, c 0.3626: `β(0)` = 0.61 rad, `β(s)` = 0.060 rad; a ramp
/// that keeps to `β(d)` reaches the full depth in 7.7 mm (a straight ramp
/// at `β(s)` needs 33 mm). Every value comes from `R`, `s` and the ceiling.
#[derive(Debug, Clone, Copy)]
struct EntryLaw {
    radius: f64,
    stepover: f64,
    /// The ceiling as a width (mm), `c · D`.
    ceiling_width: f64,
    rho: f64,
    phi0: f64,
}

impl EntryLaw {
    /// `None` when the ceiling is not above the stepover (no tilt holds at
    /// the full depth).
    fn new(params: &AdaptiveParams, ceiling: f64) -> Option<Self> {
        let r = params.tool_radius;
        let s = params.stepover.min(2.0 * r);
        let ceiling_width = ceiling * 2.0 * r;
        let h = (r * r - (r - s).max(0.0).powi(2)).max(0.0).sqrt();
        let law = Self {
            radius: r,
            stepover: s,
            ceiling_width,
            rho: h.hypot(r),
            phi0: r.atan2(h),
        };
        (law.tilt(s) > 0.0).then_some(law)
    }

    /// The largest tilt (rad) that holds the load at depth `d` (mm).
    fn tilt(&self, d: f64) -> f64 {
        let arg = ((self.ceiling_width - d - self.radius) / self.rho).clamp(-1.0, 1.0);
        (self.phi0 + arg.asin()).max(0.0)
    }

    /// The depth after a run of `ds` (mm) from depth `d`: semi-implicit,
    /// so the tilt used is the one allowed at the larger, predicted depth
    /// (`tilt` falls as the depth grows, so the step holds the law).
    fn advance(&self, d: f64, ds: f64) -> f64 {
        let predicted = (d + self.tilt(d).tan() * ds).min(self.stepover);
        (d + self.tilt(predicted).tan() * ds).min(self.stepover)
    }

    /// The ramp of `nominal` out of `parent`: each point is pulled toward
    /// its nearest point on `parent` until the law's depth reaches the gap
    /// between them, then the wrap runs as it is. Returns the ramped points
    /// and the index of the first point at full depth (`None` when the
    /// wrap ends first).
    fn ramp(&self, nominal: &[P2], parent: &[P2]) -> (Vec<P2>, Option<usize>) {
        let mut out = Vec::with_capacity(nominal.len());
        let mut depth = 0.0_f64;
        let mut full: Option<usize> = None;
        let mut prev: Option<P2> = None;
        for (i, &w) in nominal.iter().enumerate() {
            if full.is_some() {
                out.push(w);
                continue;
            }
            if let Some(q) = prev {
                depth = self.advance(depth, (w.x - q.x).hypot(w.y - q.y));
            }
            prev = Some(w);
            let Some((foot, gap)) = nearest_on(parent, w) else {
                full = Some(i);
                out.push(w);
                continue;
            };
            if depth >= gap {
                full = Some(i);
                out.push(w);
            } else {
                let t = depth / gap;
                out.push(P2::new(
                    foot.x + t * (w.x - foot.x),
                    foot.y + t * (w.y - foot.y),
                ));
            }
        }
        (out, full)
    }

    /// The spiral out of the starter rim (radius `R` about `seed`, the cutter
    /// at angle `phi0`) to radius `outer`, then one closing lap at `outer`.
    /// The radius grows by the law against the previous turn (the rim for
    /// the first): `dρ/dφ = ρ · tan β(d)`, `d = ρ - ρ_prev`, at most
    /// `ρ_prev + s`. So the pitch is `s` per turn where the law allows it
    /// (ρ ≥ s / (2π tan β(s)), 5.3 mm at s 2, R 3) and less inside. The
    /// angle step keeps every chord within `tolerance` of the arc at
    /// `outer`: `Δφ = 2π / ⌈π / acos(1 - tolerance / outer)⌉`.
    fn spiral(&self, seed: P2, phi0: f64, outer: f64, tolerance: f64) -> Vec<P2> {
        let r0 = self.radius;
        let sag = (1.0 - (tolerance / outer).min(1.0))
            .acos()
            .max(f64::EPSILON);
        let per_turn = (std::f64::consts::PI / sag).ceil().max(3.0) as usize;
        let dphi = std::f64::consts::TAU / per_turn as f64;
        // The slowest growth per turn is at the rim with the depth at `s`:
        // `2π R tan β(s)`, or `s` when the cap binds first.
        let growth =
            (std::f64::consts::TAU * r0 * self.tilt(self.stepover).tan()).min(self.stepover);
        let turns = ((outer - r0).max(0.0) / growth.max(f64::EPSILON)).ceil() as usize + 2;
        let mut rho: Vec<f64> = vec![r0];
        let prev = |rho: &[f64], i: usize| {
            i.checked_sub(per_turn)
                .and_then(|j| rho.get(j).copied())
                .unwrap_or(r0)
        };
        let mut reached: Option<usize> = None;
        for i in 0..per_turn * turns {
            if reached.is_some_and(|k| i >= k + per_turn) {
                break;
            }
            let Some(&now) = rho.get(i) else {
                break;
            };
            let d = (now - prev(rho.as_slice(), i)).max(0.0);
            let predicted = now + now * self.tilt(d).tan() * dphi;
            let d_next = (predicted - prev(rho.as_slice(), i + 1)).clamp(0.0, self.stepover);
            let next = (now + now * self.tilt(d_next).tan() * dphi)
                .min(prev(rho.as_slice(), i + 1) + self.stepover)
                .min(outer);
            rho.push(next);
            if reached.is_none() && next >= outer {
                reached = Some(i + 1);
            }
        }
        rho.iter()
            .enumerate()
            .map(|(i, &r)| {
                let a = phi0 + i as f64 * dphi;
                P2::new(seed.x + r * a.cos(), seed.y + r * a.sin())
            })
            .collect()
    }
}

/// A closed circle of `radius` about `c`, starting at angle `phi0`, its
/// chords within `tolerance` of the arc.
fn circle_from(c: P2, radius: f64, phi0: f64, tolerance: f64) -> Vec<P2> {
    let sag = (1.0 - (tolerance / radius).min(1.0))
        .acos()
        .max(f64::EPSILON);
    let n = (std::f64::consts::PI / sag).ceil().max(3.0) as usize;
    (0..=n)
        .map(|k| {
            let a = phi0 + std::f64::consts::TAU * k as f64 / n as f64;
            P2::new(c.x + radius * a.cos(), c.y + radius * a.sin())
        })
        .collect()
}

// ── The producer ───────────────────────────────────────────────────────

/// What candidate (a) solved on one piece.
#[derive(Debug, Clone, Default)]
pub struct FieldReport {
    /// False when the piece has no starter pocket (no EDT-max entry with
    /// room for it) or no seeded node; the piece is then not planned.
    pub applied: bool,
    /// The plunge point of the starter pocket, the seed centre.
    pub seed: Option<P2>,
    /// The seed disc radius (mm): the starter's last lap, `R`.
    pub seed_radius: f64,
    /// The number of levels (wraps), `⌊max T / s⌋`.
    pub levels: usize,
    /// The number of wrap pieces (loops and arcs) over every level.
    pub chains: usize,
    /// False when the sweep cap stopped the solve.
    pub converged: bool,
    /// The number of front branches ([`FieldSolver::SourceTracking`]; 1
    /// under the first-order solver).
    pub branches: usize,
    /// The levels the spiral replaces ([`JoinRule::Ramp`]): the wraps that
    /// are whole circles round the seed, `⌊(clearance - R) / s⌋`.
    pub spiral_levels: usize,
}

/// The planner input every producer of one piece shares.
struct PieceSetup {
    grid: MaterialGrid,
    mask: Vec<bool>,
    link_load: LinkLoad,
}

impl PieceSetup {
    /// The set-up of `adaptive_segments_with_debug` (`path.rs`) for
    /// `polygon`: the material grid, its fringe, the machinable mask and the
    /// link load. `None` when the inset by the tool radius is empty.
    fn new(polygon: &Polygon2, params: &AdaptiveParams, cell: f64) -> Option<Self> {
        let machinable = offset_polygon(polygon, params.tool_radius)
            .into_iter()
            .next()?;
        let mut grid = MaterialGrid::from_polygon(polygon, cell);
        if params.keep_down_links == KeepDownLinks::WithinPassLoad {
            grid.keep_fringe(polygon);
        }
        if let Some(mask) = &params.standing_material {
            grid.seed_from_material_mask(mask);
        }
        let mask = MaterialGrid::build_machinable_mask(
            &machinable,
            grid.origin_x,
            grid.origin_y,
            grid.rows,
            grid.cols,
            grid.cell_size,
        );
        let link_load = LinkLoad::new(params, &grid, polygon);
        Some(Self {
            grid,
            mask,
            link_load,
        })
    }
}

/// The planner cell of `params` (`path.rs`, [`planner_cell_size`], the one
/// source of the planner lattice).
fn planner_cell(params: &AdaptiveParams) -> f64 {
    planner_cell_size(params.tool_radius, params.tolerance)
}

/// What the producer of candidate (a) gives for one piece.
struct Produced {
    /// The starter pocket, then the wraps, before the cleanup.
    segments: Vec<AdaptiveSegment>,
    report: FieldReport,
    /// The wraps in cutting order, oriented.
    wraps: Vec<Vec<P2>>,
}

/// The clearance (mm) from `p` to the nearest ring of `region`: the largest
/// circle about `p` inside the region when `p` is inside.
fn clearance(region: &ToolCentreRegion, p: P2) -> f64 {
    region
        .pieces()
        .iter()
        .flat_map(|piece| std::iter::once(&piece.exterior).chain(piece.holes.iter()))
        .flat_map(|ring| {
            let n = ring.len();
            (0..n).filter_map(move |k| match (ring.get(k), ring.get((k + 1) % n)) {
                (Some(a), Some(b)) => Some((*a, *b)),
                _ => None,
            })
        })
        .map(|(a, b)| point_to_segment_distance(&p, &a, &b))
        .fold(f64::INFINITY, f64::min)
}

/// Candidate (a) on one piece outline: the starter pocket, then the wraps
/// of the geodesic field as `Cut`s and `Link`s, before the cleanup. `None`
/// when the field does not apply (see [`FieldReport::applied`]).
fn geodesic_segments(
    polygon: &Polygon2,
    params: &AdaptiveParams,
    options: GeodesicOptions,
    cancel: &dyn CancelCheck,
) -> Result<Option<Produced>, Cancelled> {
    let r = params.tool_radius;
    let s = params.stepover;
    let cell = planner_cell(params);
    let Some(mut setup) = PieceSetup::new(polygon, params, cell) else {
        return Ok(None);
    };
    let boundary_distances = setup.grid.compute_boundary_distances();
    let holds = params.keep_down_links == KeepDownLinks::WithinPassLoad;
    let pass = pass_load_for(params, cell);
    let Some((mut segments, starter_end)) = emit_helical_starter_pocket(
        &mut setup.grid,
        &setup.mask,
        &boundary_distances,
        r,
        holds.then_some((&pass, s, &setup.link_load)),
    ) else {
        return Ok(None);
    };
    let Some(seed) = segments.iter().find_map(|seg| match seg {
        AdaptiveSegment::Marker(AdaptiveRuntimeEvent::StarterPocket { center_x, center_y }) => {
            Some(P2::new(*center_x, *center_y))
        }
        _ => None,
    }) else {
        return Ok(None);
    };

    let region = ToolCentreRegion::new(polygon, r);
    let walls = Walls { region: &region };
    let lattice = Lattice::of_grid(&setup.grid);
    let inside: Vec<bool> = setup
        .mask
        .iter()
        .enumerate()
        .map(|(i, &m)| m && region.contains(&lattice.node(i)))
        .collect();
    let Some(field) =
        GeodesicField::on_lattice(lattice, inside, seed, r, s, options.solver, &walls)
    else {
        return Ok(None);
    };

    let reach = s + (1.0 + SQRT_2) * cell;
    // The ramp join: the levels whose wrap is a whole circle round the seed
    // (the circle of radius R + k·s about the seed lies inside the region)
    // are one spiral and a closing lap.
    let law = (options.join == JoinRule::Ramp)
        .then(|| EntryLaw::new(params, pass.ceiling))
        .flatten();
    let spiral_levels = match law {
        Some(_) => ((clearance(&region, seed) - r) / s).floor().max(0.0) as usize,
        None => 0,
    };
    let phi0 = (starter_end.y - seed.y).atan2(starter_end.x - seed.x);
    let outer = r + spiral_levels as f64 * s;
    let spiral: Vec<P2> = match law {
        Some(law) if spiral_levels > 0 => law.spiral(seed, phi0, outer, params.tolerance),
        _ => Vec::new(),
    };
    let wraps = ordered_wraps(
        &field,
        &region,
        spiral.last().copied().unwrap_or(starter_end),
        reach,
        s,
        spiral_levels + 1,
        options,
        cancel,
    )?;
    let report = FieldReport {
        applied: true,
        seed: Some(seed),
        seed_radius: r,
        levels: field.level_count(),
        chains: wraps.len(),
        converged: field.converged(),
        branches: field.branch_count(),
        spiral_levels,
    };
    let nominal: Vec<Vec<P2>> = wraps.iter().map(|w| w.pts.clone()).collect();

    let first = match law {
        Some(_) => Some(starter_end),
        None => wraps.iter().find_map(|w| w.pts.first().copied()),
    };
    if let Some(first) = first {
        segments.push(AdaptiveSegment::Marker(AdaptiveRuntimeEvent::PassEntry {
            pass_index: 1,
            entry_x: first.x,
            entry_y: first.y,
        }));
    }
    let mut run: Vec<P2> = vec![starter_end];
    let mut cur = starter_end;
    // Append `path` to the run: a gap no longer than the reach is a cutting
    // step inside the `Cut` (the spiral's seam step); a longer one is a
    // keep-down `Link`, which the replay admits under the pass load or
    // turns into a re-entry.
    let join =
        |path: &[P2], run: &mut Vec<P2>, cur: &mut P2, segments: &mut Vec<AdaptiveSegment>| {
            let Some(&start) = path.first() else {
                return;
            };
            let gap = (start.x - cur.x).hypot(start.y - cur.y);
            if gap <= reach {
                if gap > CHAIN_EPS {
                    run.push(start);
                }
                run.extend(path.iter().skip(1).copied());
            } else {
                if run.len() >= 2 {
                    segments.push(AdaptiveSegment::Cut(std::mem::take(run)));
                }
                segments.push(AdaptiveSegment::Link(start));
                *run = path.to_vec();
            }
            if let Some(&end) = run.last() {
                *cur = end;
            }
        };

    match law {
        None => {
            for path in &wraps {
                join(&path.pts, &mut run, &mut cur, &mut segments);
            }
        }
        Some(law) => {
            let rim = circle_from(seed, outer, phi0, params.tolerance);
            join(&spiral, &mut run, &mut cur, &mut segments);
            // Each cut wrap at its nominal points, and the index of its first
            // point at full depth when its start was ramped (an open arc).
            let mut cut: Vec<(Vec<P2>, Option<usize>)> = Vec::with_capacity(wraps.len());
            for wrap in &wraps {
                let nominal = subdivide(&wrap.pts, cell);
                let parent: Option<(&[P2], Option<usize>)> = match wrap.parent {
                    ParentRef::Free => None,
                    ParentRef::Rim => Some((rim.as_slice(), None)),
                    ParentRef::Wrap(o) => cut.get(o).map(|(pts, full)| (pts.as_slice(), *full)),
                };
                let mut path: Vec<P2> = Vec::new();
                let (ramped, full) = match parent {
                    Some((parent_pts, parent_full)) => {
                        // The back-run: when this wrap starts beside the
                        // parent's ramp (its start projects onto the parent
                        // before the parent's full-depth point), cut the
                        // parent's ramp wedge first, from the full-depth
                        // point back to the parent's start, so the ramp
                        // below starts on cut stock. The parent is
                        // subdivided to one cell, so its nearest vertex
                        // places the projection within one cell.
                        if let (Some(k), Some(&p0)) = (parent_full, nominal.first())
                            && super::spiral::nearest_index(parent_pts, p0) <= k
                        {
                            path.extend(parent_pts.iter().take(k + 1).rev().copied());
                        }
                        // The ramp's first point is the parent point nearest
                        // the wrap's start (depth 0).
                        law.ramp(&nominal, parent_pts)
                    }
                    None => (nominal.clone(), None),
                };
                path.extend(ramped);
                join(&path, &mut run, &mut cur, &mut segments);
                cut.push((nominal, if wrap.closed { None } else { full }));
            }
        }
    }
    if run.len() >= 2 {
        segments.push(AdaptiveSegment::Cut(run));
    }
    Ok(Some(Produced {
        segments,
        report,
        wraps: nominal,
    }))
}

// ── The pass-load readout ──────────────────────────────────────────────

/// Which producer planned a segment: the kind of the last marker before it
/// in the cleaned plan (`StarterPocket`, `PassEntry`, `ResidueContour`,
/// `BoundaryCleanup`, `ResidueMop`, `SlotClearing`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Producer {
    /// Before any marker.
    #[default]
    Unmarked,
    /// The helical starter pocket.
    Starter,
    /// The clearing passes: candidate (a)'s wraps, the agent's passes, the
    /// contour spiral.
    Clearing,
    /// A contour-parallel residue loop.
    ResidueContour,
    /// The final wall pass (`finish_walls_within_pass_load`).
    WallPass,
    /// A residue-mop patch.
    ResidueMop,
    /// The opt-in slot-clearing lines.
    SlotLines,
}

impl Producer {
    /// The producer that a segment after `event` belongs to. A pass summary
    /// and a forced clear do not start a new producer.
    fn after(self, event: &AdaptiveRuntimeEvent) -> Self {
        match event {
            AdaptiveRuntimeEvent::StarterPocket { .. } => Self::Starter,
            AdaptiveRuntimeEvent::PassEntry { .. } => Self::Clearing,
            AdaptiveRuntimeEvent::ResidueContour { .. } => Self::ResidueContour,
            AdaptiveRuntimeEvent::BoundaryCleanup { .. } => Self::WallPass,
            AdaptiveRuntimeEvent::ResidueMop { .. } => Self::ResidueMop,
            AdaptiveRuntimeEvent::SlotClearing { .. } => Self::SlotLines,
            AdaptiveRuntimeEvent::PassSummary { .. } | AdaptiveRuntimeEvent::ForcedClear { .. } => {
                self
            }
        }
    }

    /// A short name for a summary line.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Unmarked => "unmarked",
            Self::Starter => "starter",
            Self::Clearing => "clearing",
            Self::ResidueContour => "residue contour",
            Self::WallPass => "wall pass",
            Self::ResidueMop => "residue mop",
            Self::SlotLines => "slot lines",
        }
    }
}

/// One step that the readout reads over the ceiling.
#[derive(Debug, Clone, Copy)]
pub struct OverStep {
    /// Where the step starts and ends (mm).
    pub from: P2,
    pub to: P2,
    /// The reading (`measure_step`, a_e / D) of the step.
    pub reading: f64,
    /// The producer of the segment that holds the step.
    pub producer: Producer,
    /// True for a keep-down `Link` step, false for a `Cut` step.
    pub link: bool,
    /// The index of the segment in the cleaned plan of its piece.
    pub segment: usize,
}

/// The planner's own pass-load reading of a plan, replayed on a fresh grid.
///
/// Every `Cut` step (split to the planner's pass step) and every `Link` step
/// is read with `measure_step` and held with `step_within_pass_load` on the
/// grid as cut so far, and then stamped, the way `replay_kept_segments` and
/// `feed_link_checked` (`path.rs`) read them. A `Rapid` stamps the entry hole
/// (`LinkLoad::stamp_entry`). The steps out of a straight plunge are exempt
/// (`PassLoad::departing`) and counted apart. Slot-clearing lines (opt-in)
/// are stamped, not read. This is the planner predicate on the planner grid,
/// not the simulation oracle of `a_clearing_cut_holds_the_pass_load`.
///
/// Each step over the ceiling is kept in [`Self::over`] with its position,
/// its producer and its reading (verifier D, 2026-10-23: the readout gave
/// no place, so an over-step could not be told from shipped code).
#[derive(Debug, Clone, Default)]
pub struct PassLoadReadout {
    /// The ceiling (radial fraction a_e / D).
    pub ceiling: f64,
    /// The largest reading of a step that is not exempt.
    pub max_reading: f64,
    /// Steps read.
    pub steps: usize,
    /// Steps over the ceiling (`step_within_pass_load` false).
    pub steps_over: usize,
    /// Steps out of a straight plunge, exempt from the ceiling.
    pub exempt_steps: usize,
    /// Every step over the ceiling, in plan order.
    pub over: Vec<OverStep>,
}

impl PassLoadReadout {
    fn merge(&mut self, other: &Self) {
        self.ceiling = self.ceiling.max(other.ceiling);
        self.max_reading = self.max_reading.max(other.max_reading);
        self.steps += other.steps;
        self.steps_over += other.steps_over;
        self.exempt_steps += other.exempt_steps;
        self.over.extend_from_slice(&other.over);
    }
}

/// Where a step sits in the plan, for an [`OverStep`].
#[derive(Clone, Copy)]
struct StepTag {
    producer: Producer,
    link: bool,
    segment: usize,
}

fn pass_load_readout(
    polygon: &Polygon2,
    params: &AdaptiveParams,
    segments: &[AdaptiveSegment],
) -> PassLoadReadout {
    let cell = planner_cell(params);
    let r = params.tool_radius;
    let pass = pass_load_for(params, cell);
    let mut out = PassLoadReadout {
        ceiling: pass.ceiling,
        ..PassLoadReadout::default()
    };
    let Some(mut setup) = PieceSetup::new(polygon, params, cell) else {
        return out;
    };
    let step_len = pass_step_len(cell);
    let mut here: Option<P2> = None;
    let mut plunged_at: Option<P2> = None;
    let mut producer = Producer::Unmarked;
    // The replay's own rule (`replay_kept_segments`): a segment after a
    // `SlotClearing` marker, up to the next marker of any kind.
    let mut in_slot_lines = false;
    let read = |grid: &MaterialGrid,
                a: P2,
                b: P2,
                departing: bool,
                tag: StepTag,
                out: &mut PassLoadReadout| {
        if departing {
            out.exempt_steps += 1;
            return;
        }
        let angle = (b.y - a.y).atan2(b.x - a.x);
        let load = pass;
        let reading = measure_step(grid, a, b.x, b.y, r, angle, &load);
        out.steps += 1;
        out.max_reading = out.max_reading.max(reading);
        if !step_within_pass_load(grid, a, b.x, b.y, r, angle, &load) {
            out.steps_over += 1;
            out.over.push(OverStep {
                from: a,
                to: b,
                reading,
                producer: tag.producer,
                link: tag.link,
                segment: tag.segment,
            });
        }
    };
    for (index, seg) in segments.iter().enumerate() {
        match seg {
            AdaptiveSegment::Marker(event) => {
                producer = producer.after(event);
                in_slot_lines = matches!(event, AdaptiveRuntimeEvent::SlotClearing { .. });
            }
            AdaptiveSegment::Rapid(p) => {
                setup.link_load.stamp_entry(&mut setup.grid, *p);
                plunged_at = setup.link_load.plunge_departure_exempt().then_some(*p);
                here = Some(*p);
            }
            AdaptiveSegment::Link(p) => {
                if let Some(from) = here {
                    let len = (p.x - from.x).hypot(p.y - from.y);
                    let n = ((len / step_len).ceil() as usize).max(1);
                    setup.grid.clear_circle(from.x, from.y, r);
                    let tag = StepTag {
                        producer,
                        link: true,
                        segment: index,
                    };
                    let mut at = from;
                    for k in 1..=n {
                        let t = k as f64 / n as f64;
                        let next =
                            P2::new(from.x + t * (p.x - from.x), from.y + t * (p.y - from.y));
                        read(&setup.grid, at, next, false, tag, &mut out);
                        setup.grid.clear_segment(at, next, r);
                        at = next;
                    }
                }
                plunged_at = None;
                here = Some(*p);
            }
            AdaptiveSegment::Cut(path) => {
                let pts = subdivide(path, step_len);
                if let Some(&p0) = pts.first() {
                    setup.grid.clear_circle(p0.x, p0.y, r);
                }
                let departure = plunged_at.take();
                let tag = StepTag {
                    producer,
                    link: false,
                    segment: index,
                };
                for w in pts.windows(2) {
                    let &[a, b] = w else {
                        continue;
                    };
                    if !in_slot_lines {
                        let departing = departure.is_some_and(|d| (a.x - d.x).hypot(a.y - d.y) < r);
                        read(&setup.grid, a, b, departing, tag, &mut out);
                    }
                    setup.grid.clear_segment(a, b, r);
                }
                if let Some(&end) = pts.last() {
                    here = Some(end);
                }
            }
        }
    }
    out
}

/// What a drawn span of the plan is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpanKind {
    /// A `Cut` polyline.
    Cut,
    /// A keep-down `Link`: a straight move at cut depth.
    Link,
    /// A `Rapid` re-entry: the travel at safe Z to the entry, then the entry.
    Rapid,
}

/// One span of the cleaned plan, for the render: what it is, who planned
/// it, and its cutter-centre points. A `Link` or `Rapid` span holds its
/// start and its end.
#[derive(Debug, Clone)]
pub struct Span2d {
    pub kind: SpanKind,
    pub producer: Producer,
    pub pts: Vec<P2>,
}

fn spans_of(segments: &[AdaptiveSegment]) -> Vec<Span2d> {
    let mut out = Vec::new();
    let mut producer = Producer::Unmarked;
    let mut here: Option<P2> = None;
    for seg in segments {
        match seg {
            AdaptiveSegment::Marker(event) => producer = producer.after(event),
            AdaptiveSegment::Cut(path) => {
                out.push(Span2d {
                    kind: SpanKind::Cut,
                    producer,
                    pts: path.clone(),
                });
                here = path.last().copied().or(here);
            }
            AdaptiveSegment::Link(p) | AdaptiveSegment::Rapid(p) => {
                let kind = if matches!(seg, AdaptiveSegment::Link(_)) {
                    SpanKind::Link
                } else {
                    SpanKind::Rapid
                };
                out.push(Span2d {
                    kind,
                    producer,
                    pts: here.into_iter().chain(std::iter::once(*p)).collect(),
                });
                here = Some(*p);
            }
        }
    }
    out
}

// ── The run (harness door) ─────────────────────────────────────────────

/// Which 2D clearing a research run plans.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Candidate2d {
    /// A shipped strategy, through the product pipeline.
    Shipped(PathStrategy2d),
    /// Candidate (a), the geodesic wavefront, with its research options
    /// ([`GeodesicOptions::G1`], the form of the G1 look;
    /// [`GeodesicOptions::C2`], the G1 fixes).
    Geodesic(GeodesicOptions),
}

/// Counts of one run, from the planner segments (before the toolpath
/// simplification and the corner blend) and the emitted toolpath.
#[derive(Debug, Clone, Default)]
pub struct Summary2d {
    /// Inset pieces planned.
    pub pieces: usize,
    /// Length (mm) of every `Cut` polyline.
    pub cut_length_mm: f64,
    /// Length (mm) of every keep-down `Link`.
    pub link_length_mm: f64,
    /// Keep-down links (`Link` segments).
    pub keep_down_links: usize,
    /// Entries: plunges (`Rapid` segments), the first one of each piece
    /// included.
    pub entries: usize,
    /// Lifts to the safe Z in the emitted toolpath (`MoveIntent::Retract`),
    /// the last one of each piece included.
    pub retracts: usize,
    /// The planner's pass-load reading.
    pub pass_load: PassLoadReadout,
}

/// One research run over every inset piece of an outline.
pub struct Run2d {
    /// The emitted toolpath of every piece, in order.
    pub toolpath: Toolpath,
    pub summary: Summary2d,
    /// Candidate (a) only: one report per piece.
    pub fields: Vec<FieldReport>,
    /// Candidate (a) only: the wraps in cutting order, oriented.
    pub wraps: Vec<Vec<P2>>,
    /// The cleaned plan of every piece as drawn spans (planner output,
    /// before the toolpath emission), in order.
    pub spans: Vec<Span2d>,
}

/// `params` with `path_strategy` set to `strategy`.
fn params_with(params: &AdaptiveParams, strategy: PathStrategy2d) -> AdaptiveParams {
    AdaptiveParams {
        tool_radius: params.tool_radius,
        stepover: params.stepover,
        cut_depth: params.cut_depth,
        feed_rate: params.feed_rate,
        plunge_rate: params.plunge_rate,
        safe_z: params.safe_z,
        tolerance: params.tolerance,
        slot_clearing: params.slot_clearing,
        min_cutting_radius: params.min_cutting_radius,
        standing_material: params.standing_material.as_ref().map(std::sync::Arc::clone),
        cleanup_strategy: params.cleanup_strategy,
        engagement_measure: params.engagement_measure,
        path_strategy: strategy,
        trochoid_cap_mult: params.trochoid_cap_mult,
        keep_down_links: params.keep_down_links,
        entry_helix_radius: params.entry_helix_radius,
    }
}

/// The cleanup of `adaptive_toolpath_structured_annotated_traced_with_cancel`
/// (`mod.rs`): the same dispatch on the cleanup strategy.
fn cleaned(
    polygon: &Polygon2,
    params: &AdaptiveParams,
    segments: Vec<AdaptiveSegment>,
) -> Vec<AdaptiveSegment> {
    match params.cleanup_strategy {
        CleanupStrategy::Legacy => segments,
        CleanupStrategy::ResidueMop | CleanupStrategy::ContourParallelNarrow => {
            apply_residue_mop_cleanup(polygon, params, &segments)
        }
        CleanupStrategy::ContourParallelHybrid => {
            apply_contour_parallel_residue_cleanup(polygon, params, &segments)
        }
    }
}

/// Plan `outline` with `candidate` and the shared `params`, piece by piece
/// as the 2D adapter does (`inset_piece_outlines`). A shipped strategy runs
/// the product pipeline (`adaptive_segments_with_debug`, the cleanup, the
/// toolpath emission). Candidate (a) runs its producer and then the same
/// cleanup and emission, with `PathStrategy2d::Agent` in the params so the
/// contour spiral's default corner blend does not apply. A piece where the
/// field does not apply is not planned; its [`FieldReport`] says so.
#[must_use]
pub fn run_2d(outline: &Polygon2, params: &AdaptiveParams, candidate: Candidate2d) -> Run2d {
    let never: &dyn CancelCheck = &|| false;
    let strategy = match candidate {
        Candidate2d::Shipped(s) => s,
        Candidate2d::Geodesic(_) => PathStrategy2d::Agent,
    };
    let params = params_with(params, strategy);
    let pieces = super::inset_piece_outlines(outline, params.tool_radius);
    let pieces = if pieces.is_empty() {
        vec![outline.clone()]
    } else {
        pieces
    };
    let mut run = Run2d {
        toolpath: Toolpath::new(),
        summary: Summary2d {
            pieces: pieces.len(),
            ..Summary2d::default()
        },
        fields: Vec::new(),
        wraps: Vec::new(),
        spans: Vec::new(),
    };
    for piece in &pieces {
        let raw = match candidate {
            Candidate2d::Shipped(_) => {
                super::adaptive_segments_with_debug(piece, &params, never, None, None).ok()
            }
            Candidate2d::Geodesic(options) => {
                match geodesic_segments(piece, &params, options, never) {
                    Ok(Some(Produced {
                        segments,
                        report,
                        wraps,
                    })) => {
                        run.fields.push(report);
                        run.wraps.extend(wraps);
                        Some(segments)
                    }
                    Ok(None) | Err(Cancelled) => {
                        run.fields.push(FieldReport::default());
                        None
                    }
                }
            }
        };
        let Some(raw) = raw else {
            continue;
        };
        let segments = cleaned(piece, &params, raw);
        let region = (params.keep_down_links == KeepDownLinks::WithinPassLoad)
            .then(|| ToolCentreRegion::new(piece, params.tool_radius));
        let (tp, _) = segments_to_toolpath(&segments, &params, region.as_ref());
        add_counts(&mut run.summary, &segments);
        run.spans.extend(spans_of(&segments));
        run.summary
            .pass_load
            .merge(&pass_load_readout(piece, &params, &segments));
        run.summary.retracts += tp
            .moves
            .iter()
            .filter(|m| m.intent == MoveIntent::Retract)
            .count();
        run.toolpath.moves.extend(tp.moves);
    }
    run
}

fn add_counts(summary: &mut Summary2d, segments: &[AdaptiveSegment]) {
    let mut here: Option<P2> = None;
    for seg in segments {
        match seg {
            AdaptiveSegment::Cut(path) => {
                summary.cut_length_mm += path
                    .windows(2)
                    .filter_map(|w| match w {
                        &[a, b] => Some((b.x - a.x).hypot(b.y - a.y)),
                        _ => None,
                    })
                    .sum::<f64>();
                if let Some(&end) = path.last() {
                    here = Some(end);
                }
            }
            AdaptiveSegment::Link(p) => {
                summary.keep_down_links += 1;
                if let Some(from) = here {
                    summary.link_length_mm += (p.x - from.x).hypot(p.y - from.y);
                }
                here = Some(*p);
            }
            AdaptiveSegment::Rapid(p) => {
                summary.entries += 1;
                here = Some(*p);
            }
            AdaptiveSegment::Marker(_) => {}
        }
    }
}

#[cfg(test)]
#[expect(
    clippy::panic,
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]
mod tests {
    use super::*;
    use std::f64::consts::{PI, TAU};

    /// The 6 mm end mill of the 2D fixtures (`test_data/ux_2d_*.toml`).
    const R: f64 = 3.0;

    /// The 2D Adaptive defaults (`AdaptiveConfig::default`).
    fn op_defaults() -> (f64, f64) {
        let cfg = crate::compute::operation_configs::AdaptiveConfig::default();
        (cfg.stepover, cfg.tolerance)
    }

    /// A circle of radius `radius` about `c`, with enough sides that a chord
    /// lies within `sag` of the arc: `n = ⌈π / acos(1 - sag / radius)⌉`.
    fn circle(c: P2, radius: f64, sag: f64) -> Vec<P2> {
        let n = (PI / (1.0 - sag / radius).acos()).ceil() as usize;
        (0..n)
            .map(|i| {
                let a = TAU * i as f64 / n as f64;
                P2::new(c.x + radius * a.cos(), c.y + radius * a.sin())
            })
            .collect()
    }

    /// On a disc the geodesic distance is the Euclidean distance from the
    /// seed disc. Source tracking reads it exactly (every node sees the
    /// seed centre). The first-order solve over-reads it by less than one
    /// cell within 20 mm of the seed (0.72 cell, prototype 2026-10-23) and
    /// never under-reads it.
    #[test]
    fn the_field_on_a_disc_is_the_euclidean_distance() {
        let (s, tol) = op_defaults();
        let cell = planner_cell_size(R, tol);
        let disc_radius = 20.0;
        // The part is the disc grown by R, so its tool-centre region is the
        // disc of `disc_radius`.
        let part = Polygon2::new(circle(P2::new(0.0, 0.0), disc_radius + R, cell));
        let region = ToolCentreRegion::new(&part, R);
        for solver in [FieldSolver::FirstOrder, FieldSolver::SourceTracking] {
            for seed in [P2::new(0.0, 0.0), P2::new(7.3, -4.1)] {
                let field = GeodesicField::over_part(&part, R, cell, s, seed, R, solver)
                    .unwrap_or_else(|| panic!("the seed {seed:?} lies in the disc"));
                assert!(field.converged(), "a disc converges inside the cap");
                assert_eq!(field.branch_count(), 1, "a disc has one front branch");
                let lattice = Lattice::over(&part, cell);
                let mut read = 0usize;
                for i in 0..lattice.rows * lattice.cols {
                    let p = lattice.node(i);
                    if !region.contains(&p) {
                        continue;
                    }
                    let Some(t) = field.distance_mm(p) else {
                        panic!("every node of the disc is reached: {p:?}");
                    };
                    let euclid = (p.x - seed.x).hypot(p.y - seed.y) - R;
                    let over = match solver {
                        FieldSolver::FirstOrder => cell,
                        FieldSolver::SourceTracking => 1e-9,
                    };
                    assert!(
                        t >= euclid - 1e-9 && t - euclid <= over,
                        "{solver:?}: T {t:.4} vs Euclidean {euclid:.4} at {p:?} (cell {cell})"
                    );
                    read += 1;
                }
                // Every node at least one cell inside the rim is read: the
                // lattice holds about one node per cell² of the disc shrunk
                // to the radius `disc_radius - cell`.
                let interior = std::f64::consts::PI * (disc_radius - cell).powi(2) / cell.powi(2);
                assert!(
                    read as f64 >= interior,
                    "the test read {read} nodes; it must read the disc (>= {interior:.0})"
                );
            }
        }
    }

    /// Behind an island the front goes round: T at a point behind the island
    /// is more than its Euclidean distance by more than a cell, and not less
    /// than the exact geodesic (two tangents and the arc between). Source
    /// tracking reads it within one cell diagonal over the exact geodesic
    /// (its bend points are lattice nodes; prototype: 0.55 cell at most over
    /// the region, 0.29 at the test point) and splits the front into the two
    /// sides of the island.
    #[test]
    fn the_field_goes_round_an_island() {
        let (s, tol) = op_defaults();
        let cell = planner_cell_size(R, tol);
        let island = 6.0;
        // The part grown by R round the region of a 60 x 40 rectangle with
        // a round island of radius `island`: its tool-centre region is that.
        let mut hole = circle(P2::new(0.0, 0.0), island - R, cell);
        hole.reverse();
        let part = Polygon2::with_holes(
            vec![
                P2::new(-30.0 - R, -20.0 - R),
                P2::new(30.0 + R, -20.0 - R),
                P2::new(30.0 + R, 20.0 + R),
                P2::new(-30.0 - R, 20.0 + R),
            ],
            vec![hole],
        );
        let seed = P2::new(-22.0, 0.0);
        let behind = P2::new(12.0, 0.0);
        // Exact geodesic round a circle of radius `island` between two
        // points on its axis at distances `a` and `b`: two tangents and the
        // arc between their touch points.
        let (a, b) = (seed.x.abs(), behind.x.abs());
        let tangents = (a * a - island * island).sqrt() + (b * b - island * island).sqrt();
        let arc = island * (PI - (island / a).acos() - (island / b).acos());
        let exact = tangents + arc - R;
        let euclid = (behind.x - seed.x).hypot(behind.y - seed.y) - R;
        for solver in [FieldSolver::FirstOrder, FieldSolver::SourceTracking] {
            let field =
                GeodesicField::over_part(&part, R, cell, s, seed, R, solver).expect("seeded");
            assert!(field.converged());
            let t = field
                .distance_mm(behind)
                .expect("the point behind is reached");
            assert!(
                t > euclid + cell,
                "{solver:?}: T {t:.3} must exceed the Euclidean {euclid:.3} by more than a cell"
            );
            assert!(
                t >= exact - cell,
                "{solver:?}: T {t:.3} must not under-read the exact geodesic {exact:.3}"
            );
            if solver == FieldSolver::SourceTracking {
                assert!(
                    t >= exact - 1e-9 && t <= exact + SQRT_2 * cell,
                    "source tracking: T {t:.3} vs exact {exact:.3}, bound one cell diagonal"
                );
                assert!(
                    field.branch_count() >= 3,
                    "the seed side and the two sides of the island: {}",
                    field.branch_count()
                );
            }
        }
    }

    /// G1-3: on the demo star (mirror-symmetric about x = 50) the field
    /// along the five arm axes, at equal distance from the seed, equals the
    /// Euclidean distance, so the five arms read alike. The star is
    /// star-shaped about its centre, so source tracking is exact there
    /// (bound: rounding, 1e-9 mm). The first-order solve spreads 0 to 0.84
    /// of a cell over the five axes at 37 mm (prototype 2026-10-23); the
    /// test states that spread and holds it under one cell.
    #[test]
    fn the_five_arm_axes_of_the_star_read_alike() {
        let (s, tol) = op_defaults();
        let cell = planner_cell_size(R, tol);
        let part = demo_star();
        let seed = P2::new(50.0, 50.5);
        let tips = [
            (50.0, 5.0),
            (95.0, 35.0),
            (79.0, 91.0),
            (21.0, 91.0),
            (5.0, 35.0),
        ];
        for solver in [FieldSolver::FirstOrder, FieldSolver::SourceTracking] {
            let field =
                GeodesicField::over_part(&part, R, cell, s, seed, R, solver).expect("seeded");
            assert!(field.converged());
            let bound = match solver {
                FieldSolver::FirstOrder => cell,
                FieldSolver::SourceTracking => 1e-9,
            };
            // Equal distances out from the seed, one stepover apart, from the
            // seed disc to where an arm axis leaves the region (the shortest
            // arm's tool-centre tip is 45.5 - R / sin 20.1° = 36.8 mm out).
            // The prototype (2026-10-23) read every axis to 33 mm; nearer the
            // tips the axis nodes lie within a cell of a wall, where the
            // first-order error reaches 1.74 cells.
            let read_limit = 33.0;
            let mut rho = R + s;
            let mut read_to = 0.0_f64;
            while rho <= read_limit {
                let mut errors = Vec::new();
                for &(tx, ty) in &tips {
                    let len = (tx - seed.x).hypot(ty - seed.y);
                    let p = P2::new(
                        seed.x + rho * (tx - seed.x) / len,
                        seed.y + rho * (ty - seed.y) / len,
                    );
                    // The node the field holds nearest `p`, and its exact
                    // Euclidean distance.
                    let Some(node) = field.nearest_node(p).map(|i| field_node(&field, i)) else {
                        continue;
                    };
                    let Some(t) = field.distance_mm(node) else {
                        continue;
                    };
                    errors.push(t - ((node.x - seed.x).hypot(node.y - seed.y) - R));
                }
                if errors.len() < tips.len() {
                    break;
                }
                let lo = errors.iter().copied().fold(f64::INFINITY, f64::min);
                let hi = errors.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                assert!(
                    hi - lo <= bound && lo >= -1e-9,
                    "{solver:?}: the arm axes spread {:.4} cell at {rho} mm (bound {:.4} cell)",
                    (hi - lo) / cell,
                    bound / cell
                );
                read_to = rho;
                rho += s;
            }
            assert!(
                read_to >= 25.0,
                "{solver:?}: the five axes must be read out to 25 mm at least, read to {read_to}"
            );
        }
    }

    /// The lattice node `i` of `field`.
    fn field_node(field: &GeodesicField, i: usize) -> P2 {
        let f = &field.field;
        P2::new(
            f.origin_x + (i % f.cols) as f64 * f.cell,
            f.origin_y + (i / f.cols) as f64 * f.cell,
        )
    }

    /// The 2D Adaptive params of the unit tests: the op defaults, the 6 mm
    /// end mill, the default helix, the 2D pass-load rule.
    fn test_params() -> AdaptiveParams {
        let (s, tol) = op_defaults();
        let helix = crate::compute::config::HELIX_RADIUS_OVER_D * 2.0 * R;
        AdaptiveParams {
            tool_radius: R,
            stepover: s,
            cut_depth: -1.0,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            safe_z: 5.0,
            tolerance: tol,
            slot_clearing: false,
            min_cutting_radius: 0.0,
            standing_material: None,
            cleanup_strategy: CleanupStrategy::ContourParallelHybrid,
            engagement_measure: super::super::EngagementMeasure::DiskArea,
            path_strategy: PathStrategy2d::Agent,
            trochoid_cap_mult: 1.2,
            keep_down_links: KeepDownLinks::WithinPassLoad,
            entry_helix_radius: helix,
        }
    }

    /// The 60 x 40 mm pocket with a 6 mm round island of the unit tests.
    fn island_pocket() -> Polygon2 {
        let (_, tol) = op_defaults();
        let cell = planner_cell_size(R, tol);
        let mut hole = circle(P2::new(25.0, 20.0), 6.0, cell);
        hole.reverse();
        Polygon2::with_holes(
            vec![
                P2::new(0.0, 0.0),
                P2::new(60.0, 0.0),
                P2::new(60.0, 40.0),
                P2::new(0.0, 40.0),
            ],
            vec![hole],
        )
    }

    /// The outline of `fixtures/demo_star.svg` (the G1 star), as drawn: it is
    /// mirror-symmetric about x = 50 and NOT a regular star (its tips lie
    /// 45.5, 47.6, 49.8, 49.8 and 47.6 mm from (50, 50.5)).
    fn demo_star() -> Polygon2 {
        let pts = [
            (50.0, 5.0),
            (61.0, 35.0),
            (95.0, 35.0),
            (68.0, 57.0),
            (79.0, 91.0),
            (50.0, 70.0),
            (21.0, 91.0),
            (32.0, 57.0),
            (5.0, 35.0),
            (39.0, 35.0),
        ];
        let mut ring: Vec<P2> = pts.iter().map(|&(x, y)| P2::new(x, y)).collect();
        // Counter-clockwise exterior, the polygon convention.
        let twice_area: f64 = ring
            .iter()
            .zip(ring.iter().cycle().skip(1))
            .map(|(a, b)| a.x * b.y - b.x * a.y)
            .sum();
        if twice_area < 0.0 {
            ring.reverse();
        }
        Polygon2::new(ring)
    }

    /// Every step candidate (a) emits, after the shared cleanup, holds the
    /// pass load: the readout replays the plan and asks
    /// `step_within_pass_load` of each step.
    #[test]
    fn every_emitted_step_holds_the_pass_load() {
        let params = test_params();
        let run = run_2d(
            &island_pocket(),
            &params,
            Candidate2d::Geodesic(GeodesicOptions::C2),
        );
        assert!(
            run.fields.iter().any(|f| f.applied && f.chains > 0),
            "the field must apply and cut wraps: {:?}",
            run.fields
        );
        let load = &run.summary.pass_load;
        assert!(load.steps > 100, "the readout must read the plan: {load:?}");
        assert_eq!(
            load.steps_over, 0,
            "every step holds the pass load: {load:?}"
        );
        assert!(load.max_reading <= load.ceiling, "{load:?}");
    }

    /// G1-1: every wrap vertex, and every segment between two vertices of a
    /// wrap, lies inside the tool-centre region (`ToolCentreRegion`, the
    /// planner's exact test). The contour can cross a concave stretch of the
    /// region edge inside one cell; `inside_pieces` cuts the wrap there. The
    /// test holds no tolerance: the planner holds none either.
    #[test]
    fn every_wrap_lies_inside_the_tool_centre_region() {
        let params = test_params();
        for (name, outline) in [
            ("island pocket", island_pocket()),
            ("demo star", demo_star()),
        ] {
            let run = run_2d(
                &outline,
                &params,
                Candidate2d::Geodesic(GeodesicOptions::C2),
            );
            let region = ToolCentreRegion::new(&outline, params.tool_radius);
            assert!(!run.wraps.is_empty(), "{name}: candidate (a) cuts wraps");
            for wrap in &run.wraps {
                for p in wrap {
                    assert!(region.contains(p), "{name}: wrap vertex {p:?} is outside");
                }
                for pair in wrap.windows(2) {
                    let &[a, b] = pair else {
                        continue;
                    };
                    assert!(
                        region.contains_segment(a, b),
                        "{name}: wrap segment {a:?} -> {b:?} leaves the region"
                    );
                }
            }
        }
    }
}
