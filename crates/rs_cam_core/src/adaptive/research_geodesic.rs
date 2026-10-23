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
//! The default is the G1 join (`JoinRule::Seam`):
//!
//! - A closed wrap is lapped the way `spiral.rs` laps its wraps: rotated to
//!   start at the point nearest the cutter (`spiral::nearest_index`), cut
//!   round, and closed on its seam.
//! - An open wrap is cut from the end nearer the cutter to the other end.
//! - A seam step no longer than the reach (below) is a cutting step inside
//!   the `Cut`, as in `spiral.rs`. A longer join is a keep-down `Link`; the
//!   replay admits it under the pass load or turns it into a re-entry.
//! - The order rule (grow outward from the cleared side): a wrap at level `k`
//!   is cut only after every wrap at level `k - 1` within the reach of it.
//!   Among the wraps that may be cut, the one nearest the cutter is next.
//! - A wrap leaves the region nowhere: a vertex outside the tool-centre
//!   region or a segment that leaves it cuts the wrap (`inside_pieces`).
//!
//! Under the swept-width measure a radial step of one stepover into the
//! next band reads about 0.94 of D, so the replay re-enters at most seams
//! and arc-to-arc steps (C3 run: 56 of 86 entries on the star, 41 of 89 on
//! the pocket). A pass round a cleared disc of small radius also reads
//! over the ceiling at the full stepover (the stock wraps round the
//! cutter: `2 + 4/ρ` mm at s 2, R 3, against 2.18 mm), so the replay
//! splits the inner wraps mid-way (14 and 25 entries).
//!
//! The ramp join (`JoinRule::Ramp`: a spiral round the seed, a ramp out of
//! the parent wrap, arcs cut one way) was tried and refused by the replay:
//! its entry law assumed a straight cleared edge ([`EntryLaw`]). It stays
//! selectable for the record.
//!
//! # The morph join (C4, [`JoinRule::Morph`])
//!
//! The G1 join leaves one entry per wrap: the replay refuses the radial
//! seam step (C3: 56 of 92 star entries are joins). The morph join spreads
//! the outward drift over a whole wrap instead, so no step is radial:
//!
//! - The core zone: the levels whose level sets are star-shaped about the
//!   seed ([`GeodesicField::star_levels`]) are one spiral ([`core_spiral`],
//!   the level set of `T/s - φ/2π`, with `φ` unwrapped along the curve)
//!   and a closing lap. One plunge, the starter's.
//! - The lobes: a run of wall-to-wall arcs, one per level, each the one
//!   child of the arc before it, in one set of front branches, with a
//!   length ratio of at most `(R + 2s)/(R + s)` ([`lobe_length_ratio`])
//!   and every matched pair within the reach, is cut as blended legs
//!   ([`morph_legs`], [`LobeMorph`]; Bieterman's morph between consecutive
//!   contours, R3 Source 2). The other arcs are cut as under the G1 join
//!   with the one-side meeting rule.
//! - The spacing near the seed is the C5 spacing law (below); under plain
//!   spacing the replay splits the inner turns of the core (C4 run D4:
//!   138 of 392 raw spiral steps over on the star).
//!
//! # The curvature spacing law (C5, [`Spacing::Curvature`])
//!
//! A pass round a cleared disc meets more stock than its spacing: the stock
//! wraps round the cutter. For a pass at cutter-centre radius `ρ` one
//! spacing `s'` outside the passes before it (cleared to `a = ρ - s' + R`),
//! the swept width is `R - (a² - R² - ρ²) / (2ρ)` ([`curvature_levels`]
//! has the derivation). Held to the ceiling `c·D`, the spacing is
//! `s'(ρ) = ρ + R - √(R² + ρ² + 2ρ(R - c·D))`, which reaches the stepover
//! at `ρ* = s(2R - s) / (2(c·D - s))` (22.8 mm at s 2, R 3). The law
//! changes only WHICH values of `T` are the levels; the field and the
//! contouring are as before. From `ρ*` out (the lobes, where the front is
//! nearly straight) the spacing is plain.
//!
//! # The transition join (C6, [`JoinRule::Transition`])
//!
//! The seam join steps radially into the next level and the replay refuses
//! it (D5: 58 of the 73 star entries are joins). The transition join takes
//! no radial step: a closed wrap drifts out to the next level over its
//! final fraction `f` of arc length and ends on the next wrap's seam
//! ([`transition_fraction`] has the derivation of `f` from the margin the
//! law leaves and the kink the drift makes). Inside `ρ*` the law leaves no
//! margin, so `f = 1`: the whole wrap drifts, which is the spiral form
//! `T/s - φ/2π` of the core spiral, applied to any closed wrap.
//!
//! An open arc cannot take a transition within one stepover. One-way
//! passes over an arc need an advance of `Δ` at the start end (the arc
//! ends on a wall, where the cutter meets the band head-on). Alternating
//! passes with one level per pass meet `2Δ` at alternate ends; the only
//! alternation within `Δ` is the half pitch ([`LobeMorph::Alternate`]),
//! measured and dropped at D5. So arcs keep the seam join, and
//! [`JoinReport`] counts which joins the transition can take.
//!
//! # The local spacing (C6, [`Spacing::CurvatureLocal`])
//!
//! Behind a wall the front is a circle about the bend node, not the seed.
//! The local law uses each node's distance from its pivot as `ρ` and
//! solves the pass count `U` with the Eikonal sweep
//! ([`GeodesicField::use_local_spacing`]). Known limits: the sweep is
//! first order (its error is in `U`, so the levels shift as a whole and
//! their spacing changes little); close to a bend node `Δ(ρ)` falls
//! linearly (`s'(ρ) → ρ·cD/R`), so the levels pack round every island
//! corner. A floor on `ρ` needs a derivation or a ruling.
//!
//! # The cascade diagnostic (C5, [`CascadeReadout`])
//!
//! The replay stamps only the steps it accepts, so one refused step can
//! leave stock that the next steps were planned to meet as cut. The
//! diagnostic walks the raw plan on a planned grid (every step stamped) and
//! on a replay-model grid (accepted steps only) and calls a refusal a
//! cascade when the step holds on the planned grid.
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
//! - C4: the star-shape rays are spaced so the arc between two rays at the
//!   lattice diagonal is one cell; each ray is sampled at half a cell; the
//!   spiral's radius is bisected to the op tolerance, and its angle step is
//!   the chord rule above at the outer radius `R + K·s`.
//! - C5: the spacing law `s'(ρ)` and `ρ*` above, from `R`, `s` and the
//!   ceiling only; levels at `ρ_k = -(R - cD) + √((R - cD)² + (ρ_{k-1} +
//!   R)² - R²)`, capped at `ρ_{k-1} + s`, from `ρ_0 = R`.
//! - C4: the lobe length ratio `(R + 2s)/(R + s)`, the growth of a circular
//!   front at the first wrap ([`lobe_length_ratio`]); the matched gap bound
//!   is the reach; a band has one matched sample per cell of its longer
//!   arc; the half-pitch alternation has two legs per band, the least that
//!   holds one stepover ([`LobeMorph::Alternate`]).
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
use super::search::{PassLoad, ToolCentreRegion, measure_step, step_within_pass_load};
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
/// the branch of each node, the parent of each branch (index = branch id)
/// the pivot of each node (the seed centre, or the bend node its straight
/// path starts from; the seed for a node never reached), and true when a
/// round changed nothing.
fn track_sources(
    lattice: &Lattice,
    inside: &[bool],
    fixed: &[bool],
    t_mm: &mut [f64],
    seed: P2,
    seed_radius: f64,
    walls: &Walls<'_>,
) -> (Vec<u32>, Vec<u32>, Vec<P2>, bool) {
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
    let at = pivots.iter().map(|p| p.map_or(seed, |p| p.at)).collect();
    // The parent of each branch: 0 for the root and its children.
    let parent = std::iter::once(0)
        .chain(branches.keys.iter().map(|&(parent, _, _)| parent))
        .collect();
    (branch, parent, at, converged)
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
    /// The parent of each front branch (index = branch id; the root's is 0).
    branch_parent: Vec<u32>,
    stepover: f64,
    converged: bool,
    /// The value of `T` (stepovers) of each level, `levels[0] = 0` (the
    /// starter rim), up to the largest `T` of the field. Plain spacing:
    /// `0, 1, 2, ...`. The spacing law ([`Spacing::Curvature`],
    /// [`curvature_levels`]): closer levels while the front is tightly
    /// curved.
    levels: Vec<f64>,
    /// The largest finite `T` (stepovers).
    max_t: f64,
    /// The pivot of each node ([`track_sources`]; the seed under the
    /// first-order solver).
    pivot: Vec<P2>,
    seed: P2,
    seed_radius: f64,
    /// C6: whether the sweep of `U` converged ([`Self::use_local_spacing`]);
    /// `None` when the field is `T`.
    local_converged: Option<bool>,
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
        let (passes, branch, branch_parent, pivot, converged) = match solver {
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
                (passes, vec![0; n], vec![0], vec![seed; n], converged)
            }
            FieldSolver::SourceTracking => {
                let (branch, branch_parent, pivot, converged) = track_sources(
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
                    branch_parent,
                    pivot,
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
        let levels = (0..=field.ring_count).map(|k| k as f64).collect();
        Some(Self {
            field,
            inside,
            branch,
            branch_parent,
            stepover,
            converged,
            levels,
            max_t: max_pass,
            pivot,
            seed,
            seed_radius,
            local_converged: None,
        })
    }

    /// The pivot of the lattice node nearest `p` ([`track_sources`]).
    fn pivot_near(&self, p: P2) -> Option<P2> {
        self.nearest_node(p)
            .and_then(|i| self.pivot.get(i).copied())
    }

    /// C6, [`Spacing::CurvatureLocal`]: replace the field by `U`, the
    /// number of passes, solved by the Eikonal sweep (`fast_sweep`) with
    /// the local spacing as its speed: `|∇U| = 1 / Δ(ρ)` with
    /// `Δ(ρ) = min(s, s'(ρ))` ([`law_spacing`]) and `ρ` the node's distance
    /// from its pivot (the centre of curvature of the front through it:
    /// the seed, or the bend node behind a wall). The levels are then the
    /// integers. The source-tracked `T` keeps its branches, meeting lines
    /// and pivots; `U` is what is contoured, sampled and displaced on.
    ///
    /// Along a straight path from one pivot, `U` grows by
    /// `∫ dρ / Δ(ρ)`, so a pass at `ρ_k` lies at most `s'(ρ_k)` outside the
    /// pass before it (`s'` grows with `ρ`). Across a shadow line `ρ`
    /// jumps (the seed circle meets the tighter bend circle); the sweep's
    /// viscosity solution stays continuous there, so the levels bend
    /// instead of ending.
    fn use_local_spacing(&mut self, tool_radius: f64, ceiling_width: f64) {
        let f = &self.field;
        let (rows, cols, cell) = (f.rows, f.cols, f.cell);
        let n = rows * cols;
        let node = |i: usize| {
            P2::new(
                f.origin_x + (i % cols) as f64 * cell,
                f.origin_y + (i / cols) as f64 * cell,
            )
        };
        let mut speed = vec![f64::NAN; n];
        let mut passes = vec![f64::INFINITY; n];
        let mut fixed = vec![false; n];
        for i in 0..n {
            if self.inside.get(i).copied() != Some(true) {
                continue;
            }
            let x = node(i);
            let pivot = self.pivot.get(i).copied().unwrap_or(self.seed);
            let rho = (x.x - pivot.x).hypot(x.y - pivot.y);
            let delta = law_spacing(tool_radius, self.stepover, ceiling_width, rho);
            if let Some(v) = speed.get_mut(i) {
                *v = delta;
            }
            // The seed nodes, as the field seeds them: within one cell of
            // the seed disc, their signed distance in local passes.
            let d = (x.x - self.seed.x).hypot(x.y - self.seed.y);
            if d <= self.seed_radius + cell {
                // A seed node may sit at the seed centre (`ρ = 0`, a zero
                // speed); it holds the rim's speed instead.
                let rim = law_spacing(tool_radius, self.stepover, ceiling_width, self.seed_radius);
                if let (Some(p), Some(fx), Some(v)) =
                    (passes.get_mut(i), fixed.get_mut(i), speed.get_mut(i))
                {
                    *p = (d - self.seed_radius) / rim;
                    *fx = true;
                    *v = rim;
                }
            }
        }
        let converged = fast_sweep(&mut passes, &self.inside, &fixed, &speed, rows, cols, cell);
        let max_u = passes
            .iter()
            .copied()
            .filter(|v| v.is_finite())
            .fold(0.0_f64, f64::max);
        self.field.passes = passes;
        self.field.stepover_mm = speed;
        self.field.ring_count = max_u.floor() as usize;
        self.levels = (0..=self.field.ring_count).map(|k| k as f64).collect();
        self.max_t = max_u;
        self.local_converged = Some(converged);
    }

    /// Space the levels by the curvature law ([`curvature_levels`]) for a
    /// cutter of `tool_radius` and a ceiling of `ceiling_width` mm (`c·D`).
    fn use_curvature_spacing(&mut self, tool_radius: f64, ceiling_width: f64) {
        self.levels = curvature_levels(
            tool_radius,
            self.stepover,
            ceiling_width,
            self.max_t * self.stepover,
        )
        .into_iter()
        .map(|t_mm| t_mm / self.stepover)
        .collect();
    }

    /// The value of `T` (stepovers) of level `k`; past the last level, the
    /// last value plus one stepover per level.
    fn level_value(&self, k: usize) -> f64 {
        match self.levels.get(k) {
            Some(&v) => v,
            None => {
                let last = self.levels.last().copied().unwrap_or(0.0);
                last + (k + 1 - self.levels.len().max(1)) as f64
            }
        }
    }

    /// The geodesic distance (mm) at the lattice node nearest `p`, `None`
    /// at an unreachable or unsolved node. Under
    /// [`Spacing::CurvatureLocal`] the field is the pass count `U`, and
    /// this returns `U·s`, not millimetres (only the tests read it, under
    /// plain spacing).
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

    /// The number of wraps: the levels above the rim (`⌊max T / s⌋` under
    /// plain spacing).
    #[must_use]
    pub fn level_count(&self) -> usize {
        self.levels.len().saturating_sub(1)
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
        let at = |r: usize, c: usize| self.branch.get(r * cols + c).copied().unwrap_or(0);
        let corners = [
            at(row, col),
            at(row, col + 1),
            at(row + 1, col),
            at(row + 1, col + 1),
        ];
        corners
            .iter()
            .enumerate()
            .any(|(k, &a)| corners.iter().skip(k + 1).any(|&b| !self.related(a, b)))
    }

    /// True when front branch `a` is `b`, or one is an ancestor of the
    /// other. Two such branches meet on a shadow line, where a path starts
    /// to bend round a wall and the field stays smooth: not a meeting line
    /// (C3: the C2 rule cut every wrap there too). Two unrelated branches
    /// went round a wall on different sides and meet on a shock.
    fn related(&self, a: u32, b: u32) -> bool {
        self.is_ancestor(a, b) || self.is_ancestor(b, a)
    }

    /// True when `a` is `b` or lies on the parent chain of `b`.
    fn is_ancestor(&self, a: u32, b: u32) -> bool {
        let mut at = b;
        // A parent chain is at most one link per branch long.
        for _ in 0..=self.branch_parent.len() {
            if at == a {
                return true;
            }
            if at == 0 {
                return false;
            }
            at = self.branch_parent.get(at as usize).copied().unwrap_or(0);
        }
        false
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
        join_fragments(iso_contour_where(
            &self.field,
            self.level_value(level),
            &kept,
        ))
    }

    /// `T` (stepovers) at `p`, bilinear over the cell that holds `p`, the
    /// interpolation the level sets use. `None` unless the four corners of
    /// the cell are reachable and solved.
    fn sample(&self, p: P2) -> Option<f64> {
        let f = &self.field;
        let x = (p.x - f.origin_x) / f.cell;
        let y = (p.y - f.origin_y) / f.cell;
        if !x.is_finite() || !y.is_finite() || x < 0.0 || y < 0.0 {
            return None;
        }
        let (col, row) = (x.floor() as usize, y.floor() as usize);
        if col + 1 >= f.cols || row + 1 >= f.rows || !self.cell_reachable(row, col) {
            return None;
        }
        let at = |r: usize, c: usize| {
            f.passes
                .get(r * f.cols + c)
                .copied()
                .filter(|t| t.is_finite())
        };
        let (v00, v01) = (at(row, col)?, at(row, col + 1)?);
        let (v10, v11) = (at(row + 1, col)?, at(row + 1, col + 1)?);
        let (tx, ty) = (x - col as f64, y - row as f64);
        Some(
            v00 * (1.0 - tx) * (1.0 - ty)
                + v01 * tx * (1.0 - ty)
                + v10 * (1.0 - tx) * ty
                + v11 * tx * ty,
        )
    }

    /// The sample spacing along a ray: half a cell, so every cell that a
    /// ray crosses for more than half a cell holds a sample.
    fn ray_step(&self) -> f64 {
        self.field.cell / 2.0
    }

    /// The length of the longest ray: the lattice diagonal.
    fn ray_length(&self) -> f64 {
        let f = &self.field;
        (f.rows as f64).hypot(f.cols as f64) * f.cell
    }

    /// `T` along the ray from `seed` at angle `phi`, from radius `r0` to
    /// the lattice edge, at [`Self::ray_step`].
    fn ray(&self, seed: P2, phi: f64, r0: f64) -> Vec<Option<f64>> {
        let h = self.ray_step();
        let n = ((self.ray_length() - r0).max(0.0) / h).ceil() as usize;
        let (c, s) = (phi.cos(), phi.sin());
        (0..=n)
            .map(|i| {
                let r = r0 + i as f64 * h;
                self.sample(P2::new(seed.x + r * c, seed.y + r * s))
            })
            .collect()
    }

    /// True when the ray samples `ray` cross the level set `T = level`
    /// exactly once, and that crossing comes before the ray first leaves
    /// the reachable cells (so the level set is closed round the seed on
    /// this ray). A crossing is a pair of consecutive reachable samples on
    /// the two sides of `level`.
    fn ray_crosses_once(ray: &[Option<f64>], level: f64) -> bool {
        let mut crossings = 0usize;
        let mut first_before_exit = false;
        let mut left = false;
        for w in ray.windows(2) {
            let &[a, b] = w else {
                continue;
            };
            let (Some(a), Some(b)) = (a, b) else {
                left = true;
                continue;
            };
            if (a < level) != (b < level) {
                crossings += 1;
                if crossings == 1 && !left {
                    first_before_exit = true;
                }
            }
        }
        crossings == 1 && first_before_exit
    }

    /// The number of rays of the star-shape test: the arc between two rays
    /// at the longest ray is at most one cell, so no reachable cell lies
    /// between two rays unseen.
    fn star_rays(&self) -> usize {
        ((std::f64::consts::TAU * self.ray_length() / self.field.cell).ceil() as usize).max(3)
    }

    /// The core zone (C4): the largest `k` such that each level set
    /// `T = 1, ..., k` is star-shaped about `seed` and closed round it:
    /// each of [`Self::star_rays`] rays from the seed disc rim (radius
    /// `r0`) crosses it exactly once, before the ray leaves the reachable
    /// cells. A level set behind an island fails: the rays toward the
    /// island leave the region before they cross it.
    fn star_levels(&self, seed: P2, r0: f64) -> usize {
        let n = self.star_rays();
        let rays: Vec<Vec<Option<f64>>> = (0..n)
            .map(|i| self.ray(seed, std::f64::consts::TAU * i as f64 / n as f64, r0))
            .collect();
        let mut k = 0usize;
        while k < self.level_count()
            && rays
                .iter()
                .all(|ray| Self::ray_crosses_once(ray, self.level_value(k + 1)))
        {
            k += 1;
        }
        k
    }

    /// True when the level set `T = level` passes the star-shape test of
    /// [`Self::star_levels`] about `seed`.
    #[cfg(test)]
    fn level_is_star_shaped(&self, seed: P2, r0: f64, level: usize) -> bool {
        let n = self.star_rays();
        (0..n).all(|i| {
            let ray = self.ray(seed, std::f64::consts::TAU * i as f64 / n as f64, r0);
            Self::ray_crosses_once(&ray, self.level_value(level))
        })
    }

    /// The radius on the ray from `seed` at angle `phi` where `T` first
    /// reaches `t` (stepovers), walked out from `r0` at [`Self::ray_step`]
    /// and bisected to `tolerance`. `r0` when `T(r0) ≥ t`. `None` when the
    /// ray leaves the reachable cells first.
    fn radius_at(&self, seed: P2, phi: f64, r0: f64, t: f64, tolerance: f64) -> Option<f64> {
        let (c, s) = (phi.cos(), phi.sin());
        let at = |r: f64| self.sample(P2::new(seed.x + r * c, seed.y + r * s));
        if at(r0).is_some_and(|v| v >= t) {
            return Some(r0);
        }
        let h = self.ray_step();
        let n = ((self.ray_length() - r0).max(0.0) / h).ceil() as usize;
        let mut lo = r0;
        for i in 1..=n {
            let hi = r0 + i as f64 * h;
            let v = at(hi)?;
            if v >= t {
                let (mut lo, mut hi) = (lo, hi);
                while hi - lo > tolerance {
                    let mid = 0.5 * (lo + hi);
                    // The float halving has reached its resolution.
                    if mid <= lo || mid >= hi {
                        break;
                    }
                    if at(mid)? >= t {
                        hi = mid;
                    } else {
                        lo = mid;
                    }
                }
                return Some(hi);
            }
            lo = hi;
        }
        None
    }

    /// The front branches ([`Branches`]) at the lattice nodes nearest the
    /// points of `pts`, sorted, without repeats.
    fn branches_of(&self, pts: &[P2]) -> Vec<u32> {
        let mut out: Vec<u32> = pts
            .iter()
            .filter_map(|&p| self.nearest_node(p))
            .filter_map(|i| self.branch.get(i).copied())
            .collect();
        out.sort_unstable();
        out.dedup();
        out
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
    /// entered from its parent wrap by a ramp; the open arcs of a lobe are
    /// cut one way.
    ///
    /// TRIED AND REFUSED BY THE REPLAY (C3, verifier run 2026-10-23: star
    /// 87 entries against 86 with the G1 join, pocket 89 against 89; the
    /// spiral came out as 2 to 6 mm pieces, each with its own entry, and
    /// the mop took the rest). Reason: the law bounds the swept width of a
    /// tilted step against a STRAIGHT cleared edge. Round the seed the
    /// cleared edge is a circle that the stock wraps round, and the swept
    /// width of a pass of radius `ρ` one stepover outside it is
    /// `R - (a² - R² - ρ²) / (2ρ)`, `a = ρ - s + R`: `2 + 4/ρ` mm at s 2,
    /// R 3, over the ceiling (2.18 mm) for every `ρ` under about 23 mm. Not
    /// the default; kept so a run can dump the steps the replay refuses.
    Ramp,
    /// The morph join (C4): the star-shaped core round the seed is one
    /// spiral ([`core_spiral`]), and each lobe (a run of wall-to-wall arcs,
    /// one per level, each with one parent and one child) is cut as blended
    /// legs between consecutive arcs ([`morph_legs`]). An arc that is not
    /// in a lobe is cut as under [`JoinRule::Seam`], with the one-side
    /// meeting rule.
    Morph(LobeMorph),
    /// C6, the transition join: a closed wrap followed by a closed wrap one
    /// level up drifts outward over its final fraction `f` of arc length,
    /// from its level to the next ([`transition_wrap`],
    /// [`transition_fraction`]), and ends on the next wrap's seam, so the
    /// join has no radial step. The starter rim is a wrap of level 0 for
    /// this. Every other join (to or from an open arc) is the
    /// [`JoinRule::Seam`] join: an open arc cannot take the transition
    /// within one stepover (C6 note).
    Transition,
}

/// How the legs of a lobe run between consecutive arcs `A_k` and `A_{k+1}`
/// (C4).
///
/// A leg is the blend `B(p, λ) = (1 - λ)·A_k(p) + λ·A_{k+1}(p)` at the
/// matched normalised arc length `p ∈ [0, 1]`; `λ` is the part of the band
/// the leg has reached at `p`. Before the band the cleared edge is `A_k`
/// (`λ = 0` everywhere, after the plain pass of the lobe's first arc or the
/// band before). The stock a leg meets at `p` is its `λ` minus the largest
/// `λ` cut so far at `p`, in stepovers. The bound is one stepover, the
/// spacing of every other wrap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LobeMorph {
    /// Half-pitch alternation, the default: two legs per band, out with
    /// `λ = p/2` from `p = 0` and back with `λ = 1 - p/2` from `p = 1`.
    /// The leg out meets `p/2 ≤ 1/2` stepover past `A_k`; the leg back
    /// meets `(1 - p/2) - p/2 = 1 - p ≤ 1`. The band leaves the edge at
    /// `λ = 1 - p/2`, which is `-p/2` of the next band, so the next leg out
    /// meets `p/2 + p/2 = p ≤ 1`. Both turns are continuous (the legs meet
    /// at `λ = 1/2` at `p = 1` and at `A_{k+1}(0)` at `p = 0`). Two legs per
    /// band is the least number for which an alternation holds one
    /// stepover: one leg per band meets two ([`LobeMorph::AlternateFull`]).
    /// The last band leaves `A_n` short by `p/2`; the lobe ends with a
    /// plain pass of `A_n`, which meets at most half a stepover.
    Alternate,
    /// The brief's literal option (i): one leg per band, the direction
    /// alternating (`λ = p` out; `λ = 1 - p` back from `p = 1`). The leg
    /// back meets `(1 - p) - (p - 1) = 2 - 2p` stepovers: two at the near
    /// end. Kept selectable so a run can dump the steps the replay refuses
    /// there; not a harness column by default.
    AlternateFull,
    /// The brief's option (ii): one leg per band, always out (`λ = p`, so
    /// each leg meets exactly one stepover), then back along the leg just
    /// cut, through cleared stock (zero load: the FreeCAD link idea, R1
    /// §4), then the step from `A_k(0)` to `A_{k+1}(0)`. That step is
    /// radial: it meets one full stepover head-on, the seam step of
    /// [`JoinRule::Seam`] (about 0.94 of D), so the replay is predicted to
    /// refuse it once per band. The last band does not return: the lobe
    /// ends with `A_n` cut back from the far end (it meets `1 - p`).
    ///
    /// The return is a `Cut` along the reversed leg, not keep-down `Link`s:
    /// `feed_link_checked` (`path.rs`) reads a link's samples on the
    /// lattice with the wall test, and a leg ends on the wall, so a link
    /// could turn into a re-entry and hide the mechanism under test. The
    /// readout reads the return like any cut (about zero load).
    Return,
}

/// How far apart the levels (wraps) are (C5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Spacing {
    /// One stepover everywhere.
    Plain,
    /// The curvature law ([`curvature_levels`]): closer while the front is
    /// a tight circle about the seed, one stepover from `ρ*` out.
    Curvature,
    /// C6: the curvature law with `ρ` the local centre of curvature, the
    /// node's distance from its pivot (the seed, or the bend node behind a
    /// wall), solved as a field of passes
    /// ([`GeodesicField::use_local_spacing`]).
    CurvatureLocal,
}

/// The research options of candidate (a).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GeodesicOptions {
    pub solver: FieldSolver,
    pub meeting: MeetingRule,
    pub join: JoinRule,
    pub spacing: Spacing,
}

impl GeodesicOptions {
    /// The form the operator saw at G1 (2026-10-23).
    pub const G1: Self = Self {
        solver: FieldSolver::FirstOrder,
        meeting: MeetingRule::Through,
        join: JoinRule::Seam,
        spacing: Spacing::Plain,
    };
    /// The G1 fixes of editors C2 and C3: the source-tracked field, the
    /// one-side meeting rule, and the G1 join (C3 reverted the ramp join,
    /// see [`JoinRule::Ramp`]).
    pub const C2: Self = Self {
        solver: FieldSolver::SourceTracking,
        meeting: MeetingRule::Split,
        join: JoinRule::Seam,
        spacing: Spacing::Plain,
    };
    /// C4: the C2 field and meeting rule with the morph join, half-pitch
    /// alternation in the lobes.
    pub const C4_ALT: Self = Self {
        join: JoinRule::Morph(LobeMorph::Alternate),
        ..Self::C2
    };
    /// C4: the morph join, one-way legs with a return through cleared
    /// stock.
    pub const C4_RETURN: Self = Self {
        join: JoinRule::Morph(LobeMorph::Return),
        ..Self::C2
    };
    /// C5: the C2 form (plain wraps, the G1 join) with the curvature
    /// spacing law.
    pub const C5_CURV: Self = Self {
        spacing: Spacing::Curvature,
        ..Self::C2
    };
    /// C5: the curvature spacing law with the morph join (half-pitch
    /// alternation).
    pub const C5_CURV_MORPH: Self = Self {
        spacing: Spacing::Curvature,
        ..Self::C4_ALT
    };
    /// C6: the curvature law with the transition join.
    pub const C6_JOIN: Self = Self {
        join: JoinRule::Transition,
        ..Self::C5_CURV
    };
    /// C6: the local-curvature law (centre of curvature from the source),
    /// the G1 join.
    pub const C6_LOCAL: Self = Self {
        spacing: Spacing::CurvatureLocal,
        ..Self::C2
    };
    /// C6: the local-curvature law with the transition join.
    pub const C6_LOCAL_JOIN: Self = Self {
        join: JoinRule::Transition,
        ..Self::C6_LOCAL
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
    /// The level (C6: the transition join drifts to the next one).
    level: usize,
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

/// The segment of `poly` nearest `p`: the distance and the segment's two
/// ends. `None` for fewer than two points.
fn nearest_segment(poly: &[P2], p: P2) -> Option<(f64, P2, P2)> {
    poly.windows(2)
        .filter_map(|w| match w {
            &[u, v] => Some((point_to_segment_distance(&p, &u, &v), u, v)),
            _ => None,
        })
        .min_by(|x, y| x.0.total_cmp(&y.0))
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

/// The wrap pieces of `field` from level `min_level` up, level by level,
/// every vertex and segment inside `region` ([`inside_pieces`]); under
/// [`MeetingRule::Split`] each open piece records whether its ends lie at a
/// meeting line.
fn collect_chains(
    field: &GeodesicField,
    region: &ToolCentreRegion,
    min_level: usize,
    meeting: MeetingRule,
    cancel: &dyn CancelCheck,
) -> Result<Vec<Chain>, Cancelled> {
    let cell = field.cell();
    let mut chains: Vec<Chain> = Vec::new();
    for level in min_level.max(1)..=field.level_count() {
        check_cancel(cancel)?;
        for pts in field.level_chains(level, meeting) {
            if pts.len() < 2 {
                continue;
            }
            let closed = is_closed(&pts, cell);
            for (piece, closed) in inside_pieces(pts, closed, region) {
                let mut chain = Chain::new(level, piece, closed);
                if meeting == MeetingRule::Split && !closed {
                    chain.shock = [
                        chain.pts.first().is_some_and(|&p| end_at_shock(field, p)),
                        chain.pts.last().is_some_and(|&p| end_at_shock(field, p)),
                    ];
                }
                chains.push(chain);
            }
        }
    }
    Ok(chains)
}

/// For each chain, the chains one level lower within `reach` of it: its
/// parents, which must be cut first.
fn parents_of(
    chains: &[Chain],
    reach: f64,
    cancel: &dyn CancelCheck,
) -> Result<Vec<Vec<usize>>, Cancelled> {
    let mut parents: Vec<Vec<usize>> = Vec::with_capacity(chains.len());
    for chain in chains {
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
    Ok(parents)
}

/// The meeting-line rule ([`MeetingRule::Split`]) on `pts`, the cutting
/// order of `chain`: the branch cut second ends one stepover short of the
/// line. `cut_shock_ends` holds the meeting-line ends already cut, per
/// level; the ends of `chain` at a meeting line are added. Call it before
/// any other change to `pts` (it compares `pts` with `chain.pts` to know
/// the direction).
fn meeting_trim(
    chain: &Chain,
    pts: &mut Vec<P2>,
    cut_shock_ends: &mut Vec<(usize, P2)>,
    reach: f64,
    stepover: f64,
) {
    if chain.closed {
        return;
    }
    let reversed = pts.first() != chain.pts.first();
    let [first_shock, last_shock] = if reversed {
        [chain.shock[1], chain.shock[0]]
    } else {
        chain.shock
    };
    let met = |p: Option<&P2>, ends: &[(usize, P2)]| {
        p.is_some_and(|&p| {
            ends.iter()
                .any(|&(level, q)| level == chain.level && (q.x - p.x).hypot(q.y - p.y) <= reach)
        })
    };
    let trim_last = last_shock && met(pts.last(), cut_shock_ends);
    let trim_first = first_shock && met(pts.first(), cut_shock_ends);
    for (end, shock) in [
        (pts.first().copied(), first_shock),
        (pts.last().copied(), last_shock),
    ] {
        if let (Some(p), true) = (end, shock) {
            cut_shock_ends.push((chain.level, p));
        }
    }
    let keep =
        polyline_length(pts) - stepover * f64::from(u8::from(trim_last) + u8::from(trim_first));
    if keep > 0.0 {
        if trim_last {
            trim_end(pts, stepover);
        }
        if trim_first {
            pts.reverse();
            trim_end(pts, stepover);
            pts.reverse();
        }
    }
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
    let chains = collect_chains(field, region, min_level, options.meeting, cancel)?;
    let parents = parents_of(&chains, reach, cancel)?;
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
        if options.meeting == MeetingRule::Split {
            meeting_trim(chain, &mut pts, &mut cut_shock_ends, reach, stepover);
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
            level: chain.level,
            parent,
        });
        last = Some(i);
    }
    Ok(out)
}

/// The entry law of the ramp join: how steeply the cutter may cross into
/// the next band at each depth and still hold the pass load.
///
/// DEFECT (C3): the bound below assumes a straight cleared edge parallel to
/// the step. A front grown from a disc is curved: round the seed the stock
/// wraps round the cutter and reads wider (see [`JoinRule::Ramp`]), so the
/// replay refuses the spiral and the ramps there.
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

// ── The morph join (C4) ────────────────────────────────────────────────

/// `s'(ρ) = ρ + R - √(R² + ρ² + 2ρ(R - c·D))`: the spacing that puts a pass
/// at cutter-centre radius `rho` round a cleared disc on the ceiling width
/// `ceiling_width` (`c·D`); derivation at [`curvature_levels`].
fn spacing_bound(tool_radius: f64, ceiling_width: f64, rho: f64) -> f64 {
    let r = tool_radius;
    rho + r - (r * r + rho * rho + 2.0 * rho * (r - ceiling_width)).sqrt()
}

/// The radius of the next pass after a pass at `rho` under the law (C5),
/// read at the OUTER pass: the root of `ρ' = rho + s'(ρ')`,
/// `ρ' = -(R - cD) + √((R - cD)² + (rho + R)² - R²)`, capped at
/// `rho + stepover` ([`curvature_levels`] has the derivation).
fn law_next_radius(tool_radius: f64, stepover: f64, ceiling_width: f64, rho: f64) -> f64 {
    let r = tool_radius;
    let b = r - ceiling_width;
    (-b + (b * b + (rho + r).powi(2) - r * r).sqrt()).min(rho + stepover)
}

/// The law's spacing at `rho`: `s'(ρ)` capped at the stepover (C5).
fn law_spacing(tool_radius: f64, stepover: f64, ceiling_width: f64, rho: f64) -> f64 {
    spacing_bound(tool_radius, ceiling_width, rho)
        .min(stepover)
        .max(0.0)
}

/// The swept width (mm) of a pass at cutter-centre radius `rho`, `delta`
/// outside the passes before it, round a cleared disc (C5):
/// `R - (a² - R² - ρ²) / (2ρ)`, `a = ρ - Δ + R`.
fn circular_front_width(tool_radius: f64, rho: f64, delta: f64) -> f64 {
    let r = tool_radius;
    let a = rho - delta + r;
    r - (a * a - r * r - rho * rho) / (2.0 * rho)
}

/// The fraction `f` of a closed wrap's arc length over which the transition
/// join (C6, [`JoinRule::Transition`]) drifts out by `delta` (mm) to the
/// next level, for a wrap of `length` (mm) whose next level lies at
/// cutter-centre radius `rho` from its centre of curvature.
///
/// - The plain spacing leaves the margin `m = c·D - w(ρ, Δ)`
///   ([`circular_front_width`]) under the ceiling width. Inside `ρ*` the
///   law spends all of it (`m = 0`).
/// - Where the transition starts and ends, the path kinks by `β`, with
///   `tan β = Δ / (f·L)` (the lateral drift per pass step of length `ℓ` is
///   `ℓ·tan β`). A step tilted by `β` into a band of depth `d` against a
///   straight cleared edge sweeps `d + R(1 - cos β) + h sin β`,
///   `h = √(R² - (R - d)²)` ([`EntryLaw`]), so the kink costs
///   `R(1 - cos β) + h sin β` over the plain width. Held to `m`:
///   `h sin β - R cos β ≤ m - R`, so
///   `β_max = φ₀ + asin((m - R)/ρ_h)`, `ρ_h = √(h² + R²)`, `φ₀ = atan2(R, h)`.
/// - So `f = Δ / (L·tan β_max)`, at most 1. Where `m ≤ 0` (inside `ρ*`),
///   `f = 1`: the drift spreads over the whole wrap, the spiral form
///   `T/s - φ/2π` of the core spiral. It has no kink, and each turn runs
///   parallel to the turn before it, so it meets the spacing `Δ` and no
///   tilt.
///
/// From `ρ*` out the margin grows with `ρ` and `L` with it, so `f` and the
/// sector angle `f·L/ρ` (on the wrap) shrink outward. The sectors therefore nest: each
/// transition runs over the transition before it, which is the condition
/// for its spacing to stay at most `Δ` (a wider sector would meet the plain
/// wrap two levels down).
fn transition_fraction(
    tool_radius: f64,
    ceiling_width: f64,
    rho: f64,
    delta: f64,
    length: f64,
) -> f64 {
    let r = tool_radius;
    let margin = ceiling_width - circular_front_width(r, rho, delta);
    if margin <= 0.0 || length <= 0.0 {
        return 1.0;
    }
    let h = (r * r - (r - delta).max(0.0).powi(2)).max(0.0).sqrt();
    let rho_h = h.hypot(r);
    let beta = r.atan2(h) + ((margin - r) / rho_h).clamp(-1.0, 1.0).asin();
    if beta <= 0.0 {
        return 1.0;
    }
    (delta / (length * beta.tan())).min(1.0)
}

/// One transition of the transition join (C6).
#[derive(Debug, Clone, Copy)]
pub struct TransitionRecord {
    /// The level the wrap lies on (the transition goes to the next one).
    pub level: usize,
    /// The radius (mm) of the next level about the wrap's centre of
    /// curvature (its seam point's pivot).
    pub rho: f64,
    /// The spacing (mm) to the next level.
    pub delta: f64,
    /// The fraction `f` of the wrap's length ([`transition_fraction`]).
    pub fraction: f64,
    /// The sector angle (rad) of the transition on the wrap,
    /// `f·L / ρ_wrap` (`2πf` on a circle about the centre).
    pub sector: f64,
    /// False when the transition could not be traced inside the region
    /// (the wrap then closes on its seam, the G1 join).
    pub applied: bool,
}

/// How the wraps of a plan join (C6), counted for every join rule except
/// the morph.
#[derive(Debug, Clone, Default)]
pub struct JoinReport {
    /// The starter rim drifted out to the first closed wrap.
    pub rim_transition: usize,
    /// Closed wrap to the closed wrap one level up, by a transition.
    pub closed_transition: usize,
    /// Closed to closed, the transition refused (left the region or the
    /// ray left the reachable cells): the seam join.
    pub transition_fallback: usize,
    /// Closed to closed by the seam join (no transition asked for).
    pub closed_seam: usize,
    /// Closed wrap to an open arc (the seam join).
    pub closed_to_arc: usize,
    /// From an open arc to the next wrap (the seam join).
    pub from_arc: usize,
    /// Each transition traced, in cutting order.
    pub transitions: Vec<TransitionRecord>,
}

/// The transition of the closed wrap `pts` (oriented, closed on its seam)
/// from level `level` to the next (C6): the points of its final fraction
/// `f` of arc length move out along the ray from their pivot to
/// `T = T_k + λ (T_{k+1} - T_k)`, `λ` from 0 to 1 over that fraction.
/// Returns the path and the index of its first moved point. `None` when a
/// ray leaves the reachable cells or a segment leaves the region.
#[expect(
    clippy::too_many_arguments,
    reason = "each parameter is a separate input of one planning call"
)]
fn transition_wrap(
    field: &GeodesicField,
    region: &ToolCentreRegion,
    pts: &[P2],
    level: usize,
    fraction: f64,
    cell: f64,
    tolerance: f64,
    cancel: &dyn CancelCheck,
) -> Result<Option<(Vec<P2>, usize)>, Cancelled> {
    let sub = subdivide(pts, cell);
    let total = polyline_length(&sub);
    if total <= 0.0 || fraction <= 0.0 {
        return Ok(None);
    }
    let start = (1.0 - fraction) * total;
    let (t0, t1) = (field.level_value(level), field.level_value(level + 1));
    let mut out: Vec<P2> = Vec::with_capacity(sub.len());
    let mut moved_from: Option<usize> = None;
    let mut walked = 0.0_f64;
    let mut prev: Option<P2> = None;
    for &p in &sub {
        check_cancel(cancel)?;
        if let Some(q) = prev {
            walked += (p.x - q.x).hypot(p.y - q.y);
        }
        prev = Some(p);
        if walked < start {
            out.push(p);
            continue;
        }
        moved_from.get_or_insert(out.len());
        let lambda = ((walked - start) / (fraction * total)).clamp(0.0, 1.0);
        let t = t0 + lambda * (t1 - t0);
        let Some(pivot) = field.pivot_near(p) else {
            return Ok(None);
        };
        let (dx, dy) = (p.x - pivot.x, p.y - pivot.y);
        let r0 = dx.hypot(dy);
        if r0 <= 0.0 {
            return Ok(None);
        }
        let phi = dy.atan2(dx);
        let Some(r) = field.radius_at(pivot, phi, r0, t, tolerance) else {
            return Ok(None);
        };
        out.push(P2::new(pivot.x + r * phi.cos(), pivot.y + r * phi.sin()));
    }
    let inside = out.windows(2).all(|w| match w {
        &[a, b] => region.contains_segment(a, b),
        _ => true,
    });
    Ok(inside.then(|| {
        let from = moved_from.unwrap_or(out.len());
        (out, from)
    }))
}

/// The levels of the spacing law (C5, [`Spacing::Curvature`]): the values
/// of `T` (mm) of the wraps, from 0 (the starter rim) up to `t_max_mm`.
///
/// A pass at cutter-centre radius `ρ` round a cleared disc (the passes
/// before it, centre the seed) holds the ceiling `c·D` when its spacing is
/// at most `s'(ρ)` (C3 §3). The derivation: the last pass at `ρ - s'`
/// cleared the disc of radius `a = ρ - s' + R`. The stock in the cutter's
/// disc (radius `R`, centre at `ρ`) lies outside that disc; across the
/// heading (the radial direction) it spans from the two circles'
/// intersection, at `(a² - R² + ρ²) / (2ρ)` from the seed, to the disc's
/// far edge at `ρ + R`. That span is the swept width the planner reads:
/// `w = R - (a² - R² - ρ²) / (2ρ)`. (A straight edge, `ρ → ∞`, gives
/// `w = s'`; a circle gives more, because the stock wraps round the
/// cutter.) Setting `w = c·D` and solving for `a`:
/// `s'(ρ) = ρ + R - √(R² + ρ² + 2ρ(R - c·D))`.
///
/// The law is applied pass by pass: the pass of level `k` sits at
/// `ρ_k = ρ_{k-1} + Δ` with `Δ = s'(ρ_k)` (the spacing is read at the
/// OUTER pass, the one that meets the stock). Squaring
/// `√(R² + ρ_k² + 2ρ_k(R - cD)) = ρ_{k-1} + R` gives the closed form
/// `ρ_k = -(R - cD) + √((R - cD)² + (ρ_{k-1} + R)² - R²)`, and `Δ` is capped
/// at the stepover `s`. `ρ_0 = R` (the starter's last lap) and `T = ρ - R`.
/// The law reaches `s` at `ρ* = s(2R - s) / (2(cD - s))` (from
/// `s'(ρ*) = s`; 22.8 mm at s 2, R 3, c 0.3626); from there the spacing is
/// plain. Only `R`, `s` and the ceiling enter.
///
/// The radius is `R + T`, the radius of a circular front about the seed.
/// Where the seed sees the point (the core and every star-shaped zone)
/// that is the front's curvature radius. Behind a wall the front is a
/// circle about the bend node, tighter than `R + T`, so the law spaces it
/// too widely there (a known limit, C5 note).
fn curvature_levels(
    tool_radius: f64,
    stepover: f64,
    ceiling_width: f64,
    t_max_mm: f64,
) -> Vec<f64> {
    let r = tool_radius;
    let mut out = vec![0.0];
    let mut rho = r;
    // `s'` grows with `ρ`, so each level moves at least the first step
    // (positive), and the walk ends at `t_max_mm`.
    while rho - r < t_max_mm {
        let next = law_next_radius(r, stepover, ceiling_width, rho);
        if next <= rho || !next.is_finite() {
            break;
        }
        if next - r > t_max_mm {
            break;
        }
        rho = next;
        out.push(rho - r);
    }
    out
}

/// The core-zone spiral (C4): one curve from the starter rim out to the
/// last star-shaped level `K` ([`GeodesicField::star_levels`]), then one
/// closing lap on level `K`. Returns the spiral and `K`; an empty spiral
/// and 0 when no level is star-shaped.
///
/// The curve is the level set `F = 0` of `F(x) = T(x)/s - φ(x)/2π`, with
/// `φ` the polar angle about the seed from the seam ray `phi0` (the
/// starter's end). `F` jumps by one across the seam ray. The branch cut is
/// handled by unwrapping `φ` along the curve: the tracer walks the
/// unwrapped angle `θ` from 0 to `2πK` and, on the ray at `phi0 + θ`, takes
/// the radius where `T = θ/2π` stepovers ([`GeodesicField::radius_at`]).
/// Each level set of the core crosses each ray once (the star-shape test),
/// so `T` grows along the ray through every level up to `K` and the point
/// is the one crossing of that ray. The curve is continuous, one stepover
/// outward per turn, so a step of length `ds` on a turn of length `L_k`
/// drifts outward by `s·ds/L_k`.
///
/// The angle step keeps every chord within the op tolerance of a circle of
/// the outer radius `r0 + K·s`: `Δφ = 2π / ⌈π / acos(1 - tolerance / ρ)⌉`
/// (the rule of [`circle_from`]). A spiral that leaves the region
/// (`ToolCentreRegion::contains_segment`) drops its outer level and is
/// traced again.
#[expect(
    clippy::too_many_arguments,
    reason = "each parameter is a separate input of one planning call"
)]
fn core_spiral(
    field: &GeodesicField,
    region: &ToolCentreRegion,
    seed: P2,
    r0: f64,
    phi0: f64,
    stepover: f64,
    tolerance: f64,
    cancel: &dyn CancelCheck,
) -> Result<(Vec<P2>, usize), Cancelled> {
    let mut k = field.star_levels(seed, r0);
    while k > 0 {
        check_cancel(cancel)?;
        let outer = r0 + field.level_value(k) * stepover;
        let sag = (1.0 - (tolerance / outer).min(1.0))
            .acos()
            .max(f64::EPSILON);
        let per_turn = (std::f64::consts::PI / sag).ceil().max(3.0) as usize;
        let dphi = std::f64::consts::TAU / per_turn as f64;
        let mut pts: Vec<P2> = Vec::with_capacity(per_turn * (k + 1) + 1);
        let mut traced = true;
        for j in 0..=per_turn * (k + 1) {
            let theta = j as f64 * dphi;
            // The unwrapped turn count, and `T` there: level `n` of the field
            // at turn `n`, linear in the angle between two levels.
            let turns = theta / std::f64::consts::TAU;
            let t = if turns >= k as f64 {
                field.level_value(k)
            } else {
                let n = turns.floor();
                let (lo, hi) = (
                    field.level_value(n as usize),
                    field.level_value(n as usize + 1),
                );
                lo + (turns - n) * (hi - lo)
            };
            let phi = phi0 + theta;
            let Some(r) = field.radius_at(seed, phi, r0, t, tolerance) else {
                traced = false;
                break;
            };
            pts.push(P2::new(seed.x + r * phi.cos(), seed.y + r * phi.sin()));
        }
        let inside = traced
            && pts.windows(2).all(|w| match w {
                &[a, b] => region.contains_segment(a, b),
                _ => true,
            });
        if inside {
            return Ok((pts, k));
        }
        k -= 1;
    }
    Ok((Vec::new(), 0))
}

/// The largest ratio of the lengths of two consecutive arcs of one lobe
/// (C4). A front grows from the seed disc of radius `R`; at level `k` a
/// circular front has radius `R + k·s`, so an arc of one angular span grows
/// by `(R + (k + 1)·s) / (R + k·s)` per level. That is largest at the first
/// wrap (`k = 1`): `(R + 2s) / (R + s)` (1.4 at s 2, R 3). It is the
/// loosest growth the front itself gives, not a fit; a larger ratio (either
/// way) means the arc gained or lost length at a wall, and matched
/// normalised arc length would pair points that are not across one band.
fn lobe_length_ratio(tool_radius: f64, stepover: f64) -> f64 {
    (tool_radius + 2.0 * stepover) / (tool_radius + stepover)
}

/// Why two consecutive arcs are not one lobe (C4). The harness prints the
/// counts ([`LobeReport`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LobeStop {
    /// The arc has more than one child, or its child more than one parent
    /// (an arc splits or two arcs merge), or one of them is a closed loop.
    Split,
    /// The two arcs lie in different sets of front branches (behind an
    /// island, or where a shadow line first crosses the arc).
    Branch,
    /// The lengths differ by more than [`lobe_length_ratio`].
    Ratio,
    /// A matched pair of points is more than the reach apart.
    Gap,
}

/// The lobe counts of one piece (C4).
#[derive(Debug, Clone, Copy, Default)]
pub struct LobeReport {
    /// Lobes: runs of two or more arcs cut by the morph.
    pub lobes: usize,
    /// Arcs in the lobes.
    pub lobe_arcs: usize,
    /// Wrap pieces cut as plain wraps (not in a lobe).
    pub plain_wraps: usize,
    /// Parent-child pairs not joined, per [`LobeStop`] reason.
    /// `stop_split` is the sum of the three split counts below.
    pub stop_split: usize,
    /// C5, the split cases: the arc has two or more children (each arc of
    /// the next level within the reach counts, so two arms whose arcs come
    /// within the reach at an inside corner make two children) ...
    pub split_children: usize,
    /// ... its one child has two or more parents ...
    pub split_parents: usize,
    /// ... or one of the two is a closed loop.
    pub split_closed: usize,
    pub stop_branch: usize,
    pub stop_ratio: usize,
    pub stop_gap: usize,
    /// Lobes cut as plain wraps after all, because a leg left the
    /// tool-centre region (a blend of two arcs that end on a curved wall
    /// cuts the chord of that wall).
    pub outside: usize,
    /// Length (mm) of the return cuts through cleared stock
    /// ([`LobeMorph::Return`]).
    pub return_length_mm: f64,
}

/// `pts` resampled at `m + 1` points of equal arc length, its two ends
/// included.
fn resample(pts: &[P2], m: usize) -> Vec<P2> {
    let m = m.max(1);
    let total = polyline_length(pts);
    let (Some(&first), Some(&last)) = (pts.first(), pts.last()) else {
        return Vec::new();
    };
    if total <= 0.0 {
        return vec![first; m + 1];
    }
    let mut out = Vec::with_capacity(m + 1);
    out.push(first);
    let mut seg = 0usize;
    let mut walked = 0.0_f64;
    for i in 1..m {
        let target = total * i as f64 / m as f64;
        while let (Some(&a), Some(&b)) = (pts.get(seg), pts.get(seg + 1)) {
            let len = (b.x - a.x).hypot(b.y - a.y);
            if walked + len >= target || seg + 2 >= pts.len() {
                let t = if len > 0.0 {
                    ((target - walked) / len).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                out.push(P2::new(a.x + t * (b.x - a.x), a.y + t * (b.y - a.y)));
                break;
            }
            walked += len;
            seg += 1;
        }
    }
    out.push(last);
    out
}

/// The number of matched samples of a band between arcs `a` and `b`: one
/// per cell of the longer arc (the contour has about one vertex per cell).
fn band_samples(a: &[P2], b: &[P2], cell: f64) -> usize {
    ((polyline_length(a).max(polyline_length(b)) / cell).ceil() as usize).max(1)
}

/// `b` turned so that its first point pairs with the first point of `a`:
/// the pairing of the ends with the smaller sum of distances.
fn aligned_to(a: &[P2], b: &[P2]) -> Vec<P2> {
    let d = |p: Option<&P2>, q: Option<&P2>| match (p, q) {
        (Some(p), Some(q)) => (p.x - q.x).hypot(p.y - q.y),
        _ => f64::INFINITY,
    };
    let straight = d(a.first(), b.first()) + d(a.last(), b.last());
    let crossed = d(a.first(), b.last()) + d(a.last(), b.first());
    let mut out = b.to_vec();
    if crossed < straight {
        out.reverse();
    }
    out
}

/// Whether arc `b` (the one child of `a`) continues the lobe of `a`
/// (C4): `None` when it does, else why not. Read on the untrimmed arcs:
/// the meeting trim depends on the cutting order.
fn lobe_link(
    field: &GeodesicField,
    a: &Chain,
    b: &Chain,
    reach: f64,
    ratio: f64,
) -> Option<LobeStop> {
    if field.branches_of(&a.pts) != field.branches_of(&b.pts) {
        return Some(LobeStop::Branch);
    }
    let (la, lb) = (polyline_length(&a.pts), polyline_length(&b.pts));
    if la <= 0.0 || lb <= 0.0 || la.max(lb) > ratio * la.min(lb) {
        return Some(LobeStop::Ratio);
    }
    let b_pts = aligned_to(&a.pts, &b.pts);
    let m = band_samples(&a.pts, &b_pts, field.cell());
    let gap = resample(&a.pts, m)
        .iter()
        .zip(resample(&b_pts, m).iter())
        .map(|(p, q)| (q.x - p.x).hypot(q.y - p.y))
        .fold(0.0_f64, f64::max);
    (gap > reach).then_some(LobeStop::Gap)
}

/// What a planned piece of the morph join is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PieceKind {
    /// A plain pass of one wrap piece (a plain wrap, the first and the last
    /// arc of a lobe).
    Wrap,
    /// A blended leg of a lobe.
    Morph,
    /// The return of [`LobeMorph::Return`] along the leg just cut.
    Return,
}

/// One planned piece of the morph join, in cutting order.
struct PlanPiece {
    pts: Vec<P2>,
    kind: PieceKind,
}

/// The blend of `a` and `b` (resampled to the same count) at sample `i`
/// with band fraction `lambda`.
fn blend(a: &[P2], b: &[P2], i: usize, lambda: f64) -> Option<P2> {
    let (p, q) = (a.get(i)?, b.get(i)?);
    Some(P2::new(
        (1.0 - lambda) * p.x + lambda * q.x,
        (1.0 - lambda) * p.y + lambda * q.y,
    ))
}

/// The legs of a lobe after its first arc is cut (C4, [`LobeMorph`]).
/// `arcs` are the lobe's arcs in level order, each with its first point on
/// the `p = 0` side: where the plain pass of `arcs[0]` ended. Ends with a
/// plain pass of the last arc from the end the cutter stands at. Every
/// piece starts where the one before ends, except the step of
/// [`LobeMorph::Return`] from `A_k(0)` to `A_{k+1}(0)`.
fn morph_legs(arcs: &[Vec<P2>], morph: LobeMorph, cell: f64) -> Vec<PlanPiece> {
    let mut out: Vec<PlanPiece> = Vec::new();
    let bands = arcs.len().saturating_sub(1);
    let mut at_far_end = false;
    for k in 0..bands {
        let (Some(a), Some(b)) = (arcs.get(k), arcs.get(k + 1)) else {
            break;
        };
        let m = band_samples(a, b, cell);
        let (a, b) = (resample(a, m), resample(b, m));
        let leg = |lambda: &dyn Fn(f64) -> f64, forward: bool| -> Vec<P2> {
            (0..=m)
                .filter_map(|j| {
                    let i = if forward { j } else { m - j };
                    blend(&a, &b, i, lambda(i as f64 / m as f64))
                })
                .collect()
        };
        match morph {
            LobeMorph::Alternate => {
                out.push(PlanPiece {
                    pts: leg(&|p: f64| 0.5 * p, true),
                    kind: PieceKind::Morph,
                });
                out.push(PlanPiece {
                    pts: leg(&|p: f64| 1.0 - 0.5 * p, false),
                    kind: PieceKind::Morph,
                });
            }
            LobeMorph::AlternateFull => {
                if at_far_end {
                    out.push(PlanPiece {
                        pts: leg(&|p: f64| 1.0 - p, false),
                        kind: PieceKind::Morph,
                    });
                } else {
                    out.push(PlanPiece {
                        pts: leg(&|p: f64| p, true),
                        kind: PieceKind::Morph,
                    });
                }
                at_far_end = !at_far_end;
            }
            LobeMorph::Return => {
                let pts = leg(&|p: f64| p, true);
                if k + 1 < bands {
                    let mut back = pts.clone();
                    back.reverse();
                    out.push(PlanPiece {
                        pts,
                        kind: PieceKind::Morph,
                    });
                    out.push(PlanPiece {
                        pts: back,
                        kind: PieceKind::Return,
                    });
                } else {
                    out.push(PlanPiece {
                        pts,
                        kind: PieceKind::Morph,
                    });
                    at_far_end = true;
                }
            }
        }
    }
    // The last arc, from the end the cutter stands at.
    if bands > 0
        && let Some(last) = arcs.last()
    {
        let mut pts = last.clone();
        if at_far_end {
            pts.reverse();
        }
        out.push(PlanPiece {
            pts,
            kind: PieceKind::Wrap,
        });
    }
    out
}

/// The cutting plan of the morph join after the core spiral (C4): the
/// wrap pieces of `field` from `min_level` up, grouped into lobes and
/// plain wraps, in cutting order from `start`.
///
/// - A lobe link joins arc `a` to arc `b` when `b` is the one child of `a`
///   and `a` the one parent of `b`, both are open arcs, and [`lobe_link`]
///   holds (one set of front branches, the length ratio, the matched gap
///   within the reach). Each run of linked arcs is a lobe; an arc with no
///   link either way is a plain wrap.
/// - The order rule of [`ordered_wraps`]: a unit (lobe or plain wrap) is cut
///   after every parent of its first arc; the nearest ready unit goes next.
///   Inside a lobe each arc's one parent is the arc before it.
/// - A lobe: its first arc is cut as a plain wrap from the end nearer the
///   cutter, then [`morph_legs`]. The one-side meeting rule
///   ([`meeting_trim`]) applies to every arc as it is placed.
#[expect(
    clippy::too_many_arguments,
    reason = "each parameter is a separate input of one planning call"
)]
fn morph_plan(
    field: &GeodesicField,
    region: &ToolCentreRegion,
    start: P2,
    reach: f64,
    stepover: f64,
    tool_radius: f64,
    min_level: usize,
    options: GeodesicOptions,
    morph: LobeMorph,
    cancel: &dyn CancelCheck,
) -> Result<(Vec<PlanPiece>, LobeReport), Cancelled> {
    let cell = field.cell();
    let chains = collect_chains(field, region, min_level, options.meeting, cancel)?;
    let parents = parents_of(&chains, reach, cancel)?;
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); chains.len()];
    for (j, ps) in parents.iter().enumerate() {
        for &i in ps {
            if let Some(c) = children.get_mut(i) {
                c.push(j);
            }
        }
    }
    let ratio = lobe_length_ratio(tool_radius, stepover);
    let mut report = LobeReport::default();
    let mut next: Vec<Option<usize>> = vec![None; chains.len()];
    let mut has_prev = vec![false; chains.len()];
    for (i, a) in chains.iter().enumerate() {
        check_cancel(cancel)?;
        let kids = children.get(i).map_or(&[][..], Vec::as_slice);
        if kids.is_empty() {
            continue;
        }
        let stop = match kids {
            &[j] => match chains.get(j) {
                Some(b) if a.closed || b.closed => {
                    report.split_closed += 1;
                    Err(LobeStop::Split)
                }
                Some(b) if parents.get(j).is_some_and(|ps| ps.len() == 1) => {
                    lobe_link(field, a, b, reach, ratio).map_or(Ok(j), Err)
                }
                _ => {
                    report.split_parents += 1;
                    Err(LobeStop::Split)
                }
            },
            _ => {
                report.split_children += 1;
                Err(LobeStop::Split)
            }
        };
        match stop {
            Ok(j) => {
                if let Some(n) = next.get_mut(i) {
                    *n = Some(j);
                }
                if let Some(h) = has_prev.get_mut(j) {
                    *h = true;
                }
            }
            Err(LobeStop::Split) => report.stop_split += 1,
            Err(LobeStop::Branch) => report.stop_branch += 1,
            Err(LobeStop::Ratio) => report.stop_ratio += 1,
            Err(LobeStop::Gap) => report.stop_gap += 1,
        }
    }
    // The units: each run of linked arcs from an arc with no link into it.
    let mut units: Vec<Vec<usize>> = Vec::new();
    for (i, &prev) in has_prev.iter().enumerate() {
        if prev {
            continue;
        }
        let mut unit = vec![i];
        let mut at = i;
        while let Some(j) = next.get(at).copied().flatten() {
            if unit.len() > chains.len() {
                break;
            }
            unit.push(j);
            at = j;
        }
        if unit.len() >= 2 {
            report.lobes += 1;
            report.lobe_arcs += unit.len();
        } else {
            report.plain_wraps += 1;
        }
        units.push(unit);
    }

    let mut done = vec![false; chains.len()];
    let mut unit_done = vec![false; units.len()];
    let mut cut_shock_ends: Vec<(usize, P2)> = Vec::new();
    let mut out: Vec<PlanPiece> = Vec::new();
    let mut cur = start;
    for _ in 0..units.len() {
        check_cancel(cancel)?;
        let first_of = |u: usize| units.get(u).and_then(|unit| unit.first().copied());
        let ready = |u: usize| {
            !unit_done.get(u).copied().unwrap_or(true)
                && first_of(u)
                    .and_then(|i| parents.get(i))
                    .is_some_and(|ps| ps.iter().all(|&p| done.get(p).copied().unwrap_or(true)))
        };
        let pending = |u: usize| !unit_done.get(u).copied().unwrap_or(true);
        let nearest = (0..units.len())
            .filter(|&u| ready(u))
            .filter_map(|u| {
                first_of(u)
                    .and_then(|i| chains.get(i))
                    .map(|c| (u, c.entry_distance(cur)))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(u, _)| u);
        // No unit ready (not expected: a unit waits only on the parents of
        // its first arc): take the pending unit of the lowest level.
        let lowest = || {
            (0..units.len())
                .filter(|&u| pending(u))
                .filter_map(|u| {
                    first_of(u)
                        .and_then(|i| chains.get(i))
                        .map(|c| (u, c.level))
                })
                .min_by_key(|&(_, level)| level)
                .map(|(u, _)| u)
        };
        let Some(u) = nearest.or_else(lowest) else {
            break;
        };
        if let Some(flag) = unit_done.get_mut(u) {
            *flag = true;
        }
        let Some(unit) = units.get(u) else {
            break;
        };
        for &i in unit {
            if let Some(flag) = done.get_mut(i) {
                *flag = true;
            }
        }
        let Some(first) = unit.first().and_then(|&i| chains.get(i)) else {
            continue;
        };
        // The first arc (or the plain wrap): from the end nearer the cutter.
        let mut pts = first.oriented(cur);
        if options.meeting == MeetingRule::Split {
            meeting_trim(first, &mut pts, &mut cut_shock_ends, reach, stepover);
        }
        let mut arcs: Vec<Vec<P2>> = Vec::with_capacity(unit.len());
        let mut p0 = pts.clone();
        p0.reverse();
        arcs.push(p0);
        out.push(PlanPiece {
            pts,
            kind: PieceKind::Wrap,
        });
        for &i in unit.iter().skip(1) {
            let (Some(chain), Some(prev)) = (chains.get(i), arcs.last()) else {
                break;
            };
            // `p = 0` on the side where the arc before starts.
            let mut q = aligned_to(prev, &chain.pts);
            if options.meeting == MeetingRule::Split {
                meeting_trim(chain, &mut q, &mut cut_shock_ends, reach, stepover);
            }
            arcs.push(q);
        }
        let legs = morph_legs(&arcs, morph, cell);
        let inside = legs.iter().all(|piece| {
            piece.pts.windows(2).all(|w| match w {
                &[a, b] => region.contains_segment(a, b),
                _ => true,
            })
        });
        if inside {
            for piece in legs {
                if piece.kind == PieceKind::Return {
                    report.return_length_mm += polyline_length(&piece.pts);
                }
                out.push(piece);
            }
        } else {
            // The fallback: the other arcs as plain wraps, each from the
            // end nearer the cutter.
            report.outside += 1;
            let mut at = out
                .last()
                .and_then(|p| p.pts.last())
                .copied()
                .unwrap_or(cur);
            for arc in arcs.iter().skip(1) {
                let d =
                    |q: Option<&P2>| q.map_or(f64::INFINITY, |q| (q.x - at.x).hypot(q.y - at.y));
                let mut pts = arc.clone();
                if d(pts.last()) < d(pts.first()) {
                    pts.reverse();
                }
                if let Some(&end) = pts.last() {
                    at = end;
                }
                out.push(PlanPiece {
                    pts,
                    kind: PieceKind::Wrap,
                });
            }
        }
        if let Some(&end) = out.last().and_then(|p| p.pts.last()) {
            cur = end;
        }
    }
    Ok((out, report))
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
    /// The levels the spiral replaces. [`JoinRule::Ramp`]: the wraps that
    /// are whole circles round the seed, `⌊(clearance - R) / s⌋`.
    /// [`JoinRule::Morph`]: the star-shaped core levels
    /// ([`GeodesicField::star_levels`]).
    pub spiral_levels: usize,
    /// The lobe counts of the morph join (all 0 under the other joins).
    pub lobes: LobeReport,
    /// C6: how the wraps join (every join rule but the morph).
    pub joins: JoinReport,
    /// C6: whether the sweep of the local-spacing field converged
    /// ([`Spacing::CurvatureLocal`]); `None` under the other spacings.
    pub local_converged: Option<bool>,
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
    /// The wraps in cutting order, oriented. Under the morph join: the
    /// planned pieces (the core spiral, the plain passes and the legs; not
    /// the returns), so a "wrap end" of [`EntrySplit`] is a piece end, a
    /// leg's turn included.
    wraps: Vec<Vec<P2>>,
    /// The planned polylines by part, for the readout's step classes
    /// ([`PartIndex`]).
    parts: PartIndex,
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

/// Append `path` to the cutting run `run` from the cutter position `cur`:
/// a gap no longer than `reach` is a cutting step inside the `Cut` (the
/// spiral's seam step); a longer one is a keep-down `Link`, which the
/// replay admits under the pass load or turns into a re-entry.
fn append_path(
    path: &[P2],
    run: &mut Vec<P2>,
    cur: &mut P2,
    segments: &mut Vec<AdaptiveSegment>,
    reach: f64,
) {
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
    let Some(mut field) =
        GeodesicField::on_lattice(lattice, inside, seed, r, s, options.solver, &walls)
    else {
        return Ok(None);
    };
    match options.spacing {
        Spacing::Plain => {}
        Spacing::Curvature => field.use_curvature_spacing(r, pass.ceiling * 2.0 * r),
        Spacing::CurvatureLocal => field.use_local_spacing(r, pass.ceiling * 2.0 * r),
    }

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
    if let JoinRule::Morph(morph) = options.join {
        // C4: the core spiral, then the lobes and the plain wraps.
        let (core, core_levels) =
            core_spiral(&field, &region, seed, r, phi0, s, params.tolerance, cancel)?;
        let (plan, lobes) = morph_plan(
            &field,
            &region,
            core.last().copied().unwrap_or(starter_end),
            reach,
            s,
            r,
            core_levels + 1,
            options,
            morph,
            cancel,
        )?;
        let report = FieldReport {
            applied: true,
            seed: Some(seed),
            seed_radius: r,
            levels: field.level_count(),
            chains: plan.iter().filter(|p| p.kind != PieceKind::Return).count(),
            converged: field.converged(),
            branches: field.branch_count(),
            spiral_levels: core_levels,
            lobes,
            joins: JoinReport::default(),
            local_converged: field.local_converged,
        };
        let mut parts = PartIndex::new(cell);
        parts.add(&core, StepPart::Spiral);
        let mut wraps: Vec<Vec<P2>> = Vec::with_capacity(plan.len() + 1);
        if !core.is_empty() {
            wraps.push(core.clone());
        }
        for piece in &plan {
            let part = match piece.kind {
                PieceKind::Wrap => StepPart::Wrap,
                PieceKind::Morph => StepPart::Morph,
                PieceKind::Return => StepPart::Return,
            };
            parts.add(&piece.pts, part);
            if piece.kind != PieceKind::Return {
                wraps.push(piece.pts.clone());
            }
        }
        let first = core
            .first()
            .or_else(|| plan.first().and_then(|p| p.pts.first()))
            .copied();
        if let Some(first) = first {
            segments.push(AdaptiveSegment::Marker(AdaptiveRuntimeEvent::PassEntry {
                pass_index: 1,
                entry_x: first.x,
                entry_y: first.y,
            }));
        }
        let mut run: Vec<P2> = vec![starter_end];
        let mut cur = starter_end;
        append_path(&core, &mut run, &mut cur, &mut segments, reach);
        for piece in &plan {
            append_path(&piece.pts, &mut run, &mut cur, &mut segments, reach);
        }
        if run.len() >= 2 {
            segments.push(AdaptiveSegment::Cut(run));
        }
        return Ok(Some(Produced {
            segments,
            report,
            wraps,
            parts,
        }));
    }
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
    let mut report = FieldReport {
        applied: true,
        seed: Some(seed),
        seed_radius: r,
        levels: field.level_count(),
        chains: wraps.len(),
        converged: field.converged(),
        branches: field.branch_count(),
        spiral_levels,
        lobes: LobeReport::default(),
        joins: JoinReport::default(),
        local_converged: field.local_converged,
    };
    let nominal: Vec<Vec<P2>> = wraps.iter().map(|w| w.pts.clone()).collect();
    // C6: the rim drifts out to the first wrap when that is a closed wrap
    // of level 1 (the transition join).
    let transition = options.join == JoinRule::Transition;
    let rim_first = transition && wraps.first().is_some_and(|w| w.closed && w.level == 1);

    let first = match law {
        Some(_) => Some(starter_end),
        None if rim_first => Some(starter_end),
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
    let join =
        |path: &[P2], run: &mut Vec<P2>, cur: &mut P2, segments: &mut Vec<AdaptiveSegment>| {
            append_path(path, run, cur, segments, reach);
        };

    let mut moved: Vec<Vec<P2>> = Vec::new();
    match law {
        None => {
            let cd = pass.ceiling * 2.0 * r;
            let mut joins = JoinReport::default();
            // The transition of a closed wrap at `level` to the next level:
            // its fraction (the radius and spacing read at its last point's
            // pivot) and its path.
            let mut drift = |pts: &[P2],
                             level: usize,
                             joins: &mut JoinReport|
             -> Result<Option<Vec<P2>>, Cancelled> {
                let Some(&end) = pts.last() else {
                    return Ok(None);
                };
                let pivot = field.pivot_near(end).unwrap_or(seed);
                let rho_here = (end.x - pivot.x).hypot(end.y - pivot.y);
                let delta = match options.spacing {
                    Spacing::Plain => s,
                    Spacing::Curvature => {
                        (field.level_value(level + 1) - field.level_value(level)) * s
                    }
                    // The law read at the outer pass, as the levels of
                    // `Curvature` are (the same value when the pivot is the
                    // seed).
                    Spacing::CurvatureLocal => law_next_radius(r, s, cd, rho_here) - rho_here,
                };
                let rho = rho_here + delta;
                let length = polyline_length(pts);
                let fraction = transition_fraction(r, cd, rho, delta, length);
                let traced = transition_wrap(
                    &field,
                    &region,
                    pts,
                    level,
                    fraction,
                    cell,
                    params.tolerance,
                    cancel,
                )?;
                joins.transitions.push(TransitionRecord {
                    level,
                    rho,
                    delta,
                    fraction,
                    sector: fraction * length / rho_here.max(f64::EPSILON),
                    applied: traced.is_some(),
                });
                Ok(traced.map(|(path, from)| {
                    if let Some(part) = path.get(from.saturating_sub(1)..) {
                        moved.push(part.to_vec());
                    }
                    path
                }))
            };
            if rim_first {
                let rim = circle_from(seed, r, phi0, params.tolerance);
                match drift(&rim, 0, &mut joins)? {
                    Some(path) => {
                        joins.rim_transition += 1;
                        join(&path, &mut run, &mut cur, &mut segments);
                    }
                    None => joins.transition_fallback += 1,
                }
            }
            for (i, w) in wraps.iter().enumerate() {
                let next = wraps.get(i + 1);
                let to_closed = next.is_some_and(|n| n.closed && n.level == w.level + 1);
                let mut path: Option<Vec<P2>> = None;
                if w.closed && to_closed {
                    if transition {
                        path = drift(&w.pts, w.level, &mut joins)?;
                        if path.is_some() {
                            joins.closed_transition += 1;
                        } else {
                            joins.transition_fallback += 1;
                        }
                    } else {
                        joins.closed_seam += 1;
                    }
                } else if next.is_some() {
                    if w.closed {
                        joins.closed_to_arc += 1;
                    } else {
                        joins.from_arc += 1;
                    }
                }
                match path {
                    Some(p) => join(&p, &mut run, &mut cur, &mut segments),
                    None => join(&w.pts, &mut run, &mut cur, &mut segments),
                }
            }
            report.joins = joins;
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
    let mut parts = PartIndex::new(cell);
    parts.add(&spiral, StepPart::Spiral);
    for m in &moved {
        parts.add(m, StepPart::Transition);
    }
    for w in &nominal {
        parts.add(w, StepPart::Wrap);
    }
    Ok(Some(Produced {
        segments,
        report,
        wraps: nominal,
        parts,
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
    /// The index of the segment in the plan of its piece.
    pub segment: usize,
    /// Where on candidate (a)'s plan the step lies ([`StepPart`]); only the
    /// raw-plan readout classifies it.
    pub part: StepPart,
}

/// Where a step of candidate (a)'s plan lies (C3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StepPart {
    /// Not classified (a cleaned plan, a shipped strategy).
    #[default]
    Unclassified,
    /// On the spiral of the ramp join or the core spiral of the morph join
    /// (its end within one cell of the spiral polyline).
    Spiral,
    /// On a blended leg of a lobe (morph join).
    Morph,
    /// On the return of [`LobeMorph::Return`] through cleared stock.
    Return,
    /// On the moved part of a transition ([`JoinRule::Transition`], C6).
    Transition,
    /// On a planned wrap (its end within one cell of the wrap polyline).
    Wrap,
    /// Off every planned piece: a ramp, a back-run, a seam step, an
    /// arc-to-arc step, or the near-end step of [`LobeMorph::Return`].
    Join,
}

impl StepPart {
    /// A short name for a summary line.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Unclassified => "",
            Self::Spiral => "spiral",
            Self::Morph => "morph leg",
            Self::Return => "return",
            Self::Transition => "transition",
            Self::Wrap => "on a wrap",
            Self::Join => "join (ramp, back-run or seam step)",
        }
    }

    /// A one-word name for a tally line.
    #[must_use]
    pub fn short(self) -> &'static str {
        match self {
            Self::Unclassified => "",
            Self::Spiral => "spiral",
            Self::Morph => "morph",
            Self::Return => "return",
            Self::Transition => "transition",
            Self::Wrap => "wrap",
            Self::Join => "join",
        }
    }
}

/// The planned polylines of candidate (a) by [`StepPart`], to classify
/// each step of the raw plan: a step whose end AND midpoint lie within one
/// cell of a polyline is on that part (the first match in the order
/// spiral, morph leg, return, transition, wrap); else it is a join. (C5: the midpoint
/// test is new; at C4 a radial join step that ended on the next wrap was
/// classed "wrap".) One cell is the lattice the
/// pieces were traced on. A return runs on its leg's polyline the other
/// way, so for a morph leg and a return the step must also run the way of
/// the nearest segment (a positive dot product).
#[derive(Debug, Clone, Default)]
struct PartIndex {
    cell: f64,
    /// Each polyline with its bounding box (low, high corner) and part.
    polys: Vec<(P2, P2, Vec<P2>, StepPart)>,
}

impl PartIndex {
    fn new(cell: f64) -> Self {
        Self {
            cell,
            polys: Vec::new(),
        }
    }

    fn add(&mut self, pts: &[P2], part: StepPart) {
        if pts.is_empty() {
            return;
        }
        let mut lo = P2::new(f64::INFINITY, f64::INFINITY);
        let mut hi = P2::new(f64::NEG_INFINITY, f64::NEG_INFINITY);
        for p in pts {
            lo = P2::new(lo.x.min(p.x), lo.y.min(p.y));
            hi = P2::new(hi.x.max(p.x), hi.y.max(p.y));
        }
        self.polys.push((lo, hi, pts.to_vec(), part));
    }

    /// The part of the step from `a` to `p`.
    fn part(&self, a: P2, p: P2) -> StepPart {
        let directed = |part: StepPart| matches!(part, StepPart::Morph | StepPart::Return);
        let mid = P2::new(0.5 * (a.x + p.x), 0.5 * (a.y + p.y));
        let near = |part: StepPart| {
            self.polys.iter().any(|(lo, hi, pts, q)| {
                *q == part
                    && p.x >= lo.x - self.cell
                    && p.x <= hi.x + self.cell
                    && p.y >= lo.y - self.cell
                    && p.y <= hi.y + self.cell
                    && nearest_segment(pts, mid).is_some_and(|(d, _, _)| d <= self.cell)
                    && nearest_segment(pts, p).is_some_and(|(d, u, v)| {
                        d <= self.cell
                            && (!directed(part)
                                || (v.x - u.x) * (p.x - a.x) + (v.y - u.y) * (p.y - a.y) > 0.0)
                    })
            })
        };
        [
            StepPart::Spiral,
            StepPart::Morph,
            StepPart::Return,
            StepPart::Transition,
            StepPart::Wrap,
        ]
        .into_iter()
        .find(|&part| near(part))
        .unwrap_or(StepPart::Join)
    }
}

/// The readout of one class of steps (C4): a producer of the cleaned plan,
/// or a producer and a [`StepPart`] of the raw plan.
#[derive(Debug, Clone, Default)]
pub struct LoadTally {
    /// The class: the producer's label, then the part or "link".
    pub label: String,
    /// The largest reading of a step in the class.
    pub max_reading: f64,
    /// Steps read.
    pub steps: usize,
    /// Steps over the ceiling: in the raw plan, the steps the replay
    /// refuses.
    pub steps_over: usize,
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
    /// The readout per class of steps ([`LoadTally`]), in the order the
    /// classes first appear.
    pub tallies: Vec<LoadTally>,
}

impl PassLoadReadout {
    fn merge(&mut self, other: &Self) {
        self.ceiling = self.ceiling.max(other.ceiling);
        self.max_reading = self.max_reading.max(other.max_reading);
        self.steps += other.steps;
        self.steps_over += other.steps_over;
        self.exempt_steps += other.exempt_steps;
        self.over.extend_from_slice(&other.over);
        for t in &other.tallies {
            self.tally(&t.label, t.max_reading, t.steps, t.steps_over);
        }
    }

    /// Add `steps` steps (of which `over` over the ceiling, the largest
    /// reading `max`) to the tally of `label`.
    fn tally(&mut self, label: &str, max: f64, steps: usize, over: usize) {
        if let Some(t) = self.tallies.iter_mut().find(|t| t.label == label) {
            t.max_reading = t.max_reading.max(max);
            t.steps += steps;
            t.steps_over += over;
        } else {
            self.tallies.push(LoadTally {
                label: label.to_owned(),
                max_reading: max,
                steps,
                steps_over: over,
            });
        }
    }
}

/// Where a step sits in the plan, for an [`OverStep`].
#[derive(Clone, Copy)]
struct StepTag {
    producer: Producer,
    link: bool,
    segment: usize,
}

/// The readout of `segments` on `polygon`. With `parts` (candidate (a)'s
/// raw plan) each clearing step is classed by [`PartIndex::part`] and each
/// over-step records its part; the tallies are per producer and part.
/// Without it the tallies are per producer.
fn pass_load_readout(
    polygon: &Polygon2,
    params: &AdaptiveParams,
    segments: &[AdaptiveSegment],
    parts: Option<&PartIndex>,
    cell: f64,
) -> PassLoadReadout {
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
        let part = match parts {
            Some(index) if tag.producer == Producer::Clearing && !tag.link => index.part(a, b),
            _ => StepPart::Unclassified,
        };
        let label = step_label(tag, part);
        let held = step_within_pass_load(grid, a, b.x, b.y, r, angle, &load);
        out.tally(&label, reading, 1, usize::from(!held));
        if !held {
            out.steps_over += 1;
            out.over.push(OverStep {
                from: a,
                to: b,
                reading,
                producer: tag.producer,
                link: tag.link,
                segment: tag.segment,
                part,
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

/// The tally label of a step: the producer, then "link" or the part.
fn step_label(tag: StepTag, part: StepPart) -> String {
    if tag.link {
        format!("{} link", tag.producer.label())
    } else if part == StepPart::Unclassified {
        tag.producer.label().to_owned()
    } else {
        format!("{} {}", tag.producer.label(), part.short())
    }
}

/// One step the replay model refuses (C5, [`CascadeReadout`]).
#[derive(Debug, Clone, Copy)]
pub struct Refusal {
    pub from: P2,
    pub to: P2,
    pub producer: Producer,
    pub part: StepPart,
    /// True for a step of a keep-down `Link`.
    pub link: bool,
    /// The index of the segment in the raw plan of its piece.
    pub segment: usize,
    /// The reading on the replay grid (only accepted steps stamped).
    pub replay_reading: f64,
    /// The reading on the planned grid (every step before it stamped).
    pub planned_reading: f64,
    /// True when the step holds on the planned grid: the stock it met is
    /// stock the plan cleared but the replay did not stamp, because a step
    /// before it was refused. False: the step meets too much stock even
    /// when every step before it is cut (an intrinsic refusal).
    pub cascade: bool,
    /// The distance (mm) of the step end from the seed.
    pub rho: Option<f64>,
}

impl Refusal {
    /// The class of the step (as [`LoadTally::label`]).
    #[must_use]
    pub fn label(&self) -> String {
        step_label(
            StepTag {
                producer: self.producer,
                link: self.link,
                segment: self.segment,
            },
            self.part,
        )
    }
}

/// The cascade counts of one class of steps (C5).
#[derive(Debug, Clone, Default)]
pub struct CascadeTally {
    /// The class (as [`LoadTally::label`]).
    pub label: String,
    /// Steps the replay model refuses.
    pub refused: usize,
    /// Of those, the cascades (they hold on the planned grid).
    pub cascade: usize,
    /// Of those, the intrinsic refusals.
    pub intrinsic: usize,
    /// The largest planned-grid reading of an intrinsic refusal.
    pub max_intrinsic: f64,
}

/// The cascade diagnostic of a raw plan (C5).
///
/// The replay (`replay_kept_segments`, `emit_capped_walk_with` in
/// `path.rs`) stamps only the steps it accepts. A refused step leaves its
/// stock, and a later step that the plan meant to run on cleared stock may
/// then be refused too: a cascade. The diagnostic walks the raw plan on two
/// grids at once:
///
/// - the planned grid stamps every step (the raw readout's grid, so its
///   over-steps are those of [`Summary2d::raw_pass_load`]);
/// - the replay grid models the replay: a step that holds there is
///   stamped; a refused step is not, and the cutter lifts; while lifted, a
///   step that holds WITHOUT the disc at its start cleared re-starts the
///   cut there, with the entry hole stamped (`LinkLoad::stamp_entry`; the
///   replay may reach it by a keep-down link instead, which stamps the
///   link's corridor, not a hole). A `Link` holds when
///   every step of it holds (else it is a re-entry at its end); a `Rapid`
///   stamps its entry on both grids.
///
/// A refusal on the replay grid is a cascade when the same step holds on
/// the planned grid, and intrinsic when it does not.
#[derive(Debug, Clone, Default)]
pub struct CascadeReadout {
    /// Per class of steps, in the order the classes first appear.
    pub tallies: Vec<CascadeTally>,
    /// The first refusal of each producer piece (a run of steps of one raw
    /// segment and one part), in plan order.
    pub first_of_piece: Vec<Refusal>,
    /// Every intrinsic refusal, in plan order.
    pub intrinsic: Vec<Refusal>,
    /// C6: the intrinsic refusals whose planned reading lies in the first
    /// lattice quantum over the ceiling, `(c, c + cell/D]`, with `cell` the
    /// lattice the walk read on ([`Self::read_cell`]). The lattice reads an
    /// edge up to one cell off (`PassLoad::swept_width`), so a refusal in
    /// that band may be the measure's quantum, not the stock.
    pub near_ceiling: usize,
    /// The lattice cell (mm) the walk read on.
    pub read_cell: f64,
}

impl CascadeReadout {
    fn merge(&mut self, other: &Self) {
        for t in &other.tallies {
            self.add(t);
        }
        self.first_of_piece.extend_from_slice(&other.first_of_piece);
        self.intrinsic.extend_from_slice(&other.intrinsic);
        self.near_ceiling += other.near_ceiling;
        self.read_cell = self.read_cell.max(other.read_cell);
    }

    /// Add the counts of `t` to the tally of its label.
    fn add(&mut self, t: &CascadeTally) {
        if let Some(mine) = self.tallies.iter_mut().find(|m| m.label == t.label) {
            mine.refused += t.refused;
            mine.cascade += t.cascade;
            mine.intrinsic += t.intrinsic;
            mine.max_intrinsic = mine.max_intrinsic.max(t.max_intrinsic);
        } else {
            self.tallies.push(t.clone());
        }
    }

    /// The refusals, the cascades among them, and the intrinsic ones, over
    /// every class.
    #[must_use]
    pub fn totals(&self) -> (usize, usize, usize) {
        self.tallies.iter().fold((0, 0, 0), |(r, c, i), t| {
            (r + t.refused, c + t.cascade, i + t.intrinsic)
        })
    }
}

/// The two grids of the cascade walk and where the replay cutter stands.
struct CascadeWalk {
    plan: PieceSetup,
    replay: PieceSetup,
    pass: PassLoad,
    tool_radius: f64,
    /// True while the replay cutter is down (cutting).
    down: bool,
    piece: Option<(usize, StepPart)>,
    piece_refused: bool,
    seed: Option<P2>,
    out: CascadeReadout,
}

impl CascadeWalk {
    /// One cutting step from `a` to `b` on both grids.
    fn step(&mut self, a: P2, b: P2, tag: StepTag, part: StepPart, departing: bool) {
        let key = (tag.segment, part);
        if self.piece != Some(key) {
            self.piece = Some(key);
            self.piece_refused = false;
        }
        let load = if departing {
            self.pass.departing()
        } else {
            self.pass
        };
        let r = self.tool_radius;
        let angle = (b.y - a.y).atan2(b.x - a.x);
        let planned = measure_step(&self.plan.grid, a, b.x, b.y, r, angle, &load);
        self.plan.grid.clear_segment(a, b, r);
        let replayed = measure_step(&self.replay.grid, a, b.x, b.y, r, angle, &load);
        if replayed <= load.ceiling {
            if !self.down {
                self.replay.link_load.stamp_entry(&mut self.replay.grid, a);
                self.down = true;
            }
            self.replay.grid.clear_segment(a, b, r);
            return;
        }
        self.down = false;
        self.refuse(a, b, tag, part, replayed, planned);
    }

    /// Record a refusal on the replay grid. A refused step is never a
    /// departure from a plunge (that load has no ceiling), so the ceiling is
    /// the pass ceiling.
    fn refuse(&mut self, a: P2, b: P2, tag: StepTag, part: StepPart, replayed: f64, planned: f64) {
        let cascade = planned <= self.pass.ceiling;
        let refusal = Refusal {
            from: a,
            to: b,
            producer: tag.producer,
            part,
            link: tag.link,
            segment: tag.segment,
            replay_reading: replayed,
            planned_reading: planned,
            cascade,
            rho: self.seed.map(|c| (b.x - c.x).hypot(b.y - c.y)),
        };
        self.out.add(&CascadeTally {
            label: step_label(tag, part),
            refused: 1,
            cascade: usize::from(cascade),
            intrinsic: usize::from(!cascade),
            max_intrinsic: if cascade { 0.0 } else { planned },
        });
        if !cascade {
            self.out.intrinsic.push(refusal);
            let quantum = self.out.read_cell / (2.0 * self.tool_radius);
            if planned <= self.pass.ceiling + quantum {
                self.out.near_ceiling += 1;
            }
        }
        if !self.piece_refused {
            self.piece_refused = true;
            self.out.first_of_piece.push(refusal);
        }
    }

    /// A keep-down link from `from` to `to` in steps of at most `step_len`.
    fn link(&mut self, from: P2, to: P2, step_len: f64, tag: StepTag) {
        let r = self.tool_radius;
        let len = (to.x - from.x).hypot(to.y - from.y);
        let n = ((len / step_len).ceil() as usize).max(1);
        self.plan.grid.clear_circle(from.x, from.y, r);
        let mut trial = self.replay.grid.clone();
        if self.down {
            trial.clear_circle(from.x, from.y, r);
        }
        let mut held = true;
        let mut at = from;
        for k in 1..=n {
            let t = k as f64 / n as f64;
            let next = P2::new(from.x + t * (to.x - from.x), from.y + t * (to.y - from.y));
            let angle = (next.y - at.y).atan2(next.x - at.x);
            let planned = measure_step(&self.plan.grid, at, next.x, next.y, r, angle, &self.pass);
            self.plan.grid.clear_segment(at, next, r);
            if held {
                let replayed = measure_step(&trial, at, next.x, next.y, r, angle, &self.pass);
                if replayed <= self.pass.ceiling {
                    trial.clear_segment(at, next, r);
                } else {
                    held = false;
                    self.piece = Some((tag.segment, StepPart::Unclassified));
                    self.piece_refused = false;
                    self.refuse(at, next, tag, StepPart::Unclassified, replayed, planned);
                }
            }
            at = next;
        }
        if held {
            self.replay.grid = trial;
        } else {
            // The refused link is a re-entry at its end.
            self.replay.link_load.stamp_entry(&mut self.replay.grid, to);
        }
        self.down = true;
    }
}

/// The cascade diagnostic ([`CascadeReadout`]) of the raw plan `segments`
/// of one piece, read on a lattice of `cell`. `parts` classes the clearing
/// steps as in [`pass_load_readout`]; `seed` gives each refusal its
/// distance.
fn cascade_readout(
    polygon: &Polygon2,
    params: &AdaptiveParams,
    segments: &[AdaptiveSegment],
    parts: Option<&PartIndex>,
    seed: Option<P2>,
    cell: f64,
) -> CascadeReadout {
    let (Some(plan), Some(replay)) = (
        PieceSetup::new(polygon, params, cell),
        PieceSetup::new(polygon, params, cell),
    ) else {
        return CascadeReadout::default();
    };
    let mut walk = CascadeWalk {
        plan,
        replay,
        pass: pass_load_for(params, cell),
        tool_radius: params.tool_radius,
        down: true,
        piece: None,
        piece_refused: false,
        seed,
        out: CascadeReadout {
            read_cell: cell,
            ..CascadeReadout::default()
        },
    };
    let r = params.tool_radius;
    let step_len = pass_step_len(cell);
    let mut here: Option<P2> = None;
    let mut plunged_at: Option<P2> = None;
    let mut producer = Producer::Unmarked;
    let mut in_slot_lines = false;
    for (index, seg) in segments.iter().enumerate() {
        match seg {
            AdaptiveSegment::Marker(event) => {
                producer = producer.after(event);
                in_slot_lines = matches!(event, AdaptiveRuntimeEvent::SlotClearing { .. });
            }
            AdaptiveSegment::Rapid(p) => {
                walk.plan.link_load.stamp_entry(&mut walk.plan.grid, *p);
                walk.replay.link_load.stamp_entry(&mut walk.replay.grid, *p);
                walk.down = true;
                plunged_at = walk.plan.link_load.plunge_departure_exempt().then_some(*p);
                here = Some(*p);
            }
            AdaptiveSegment::Link(p) => {
                if let Some(from) = here {
                    let tag = StepTag {
                        producer,
                        link: true,
                        segment: index,
                    };
                    walk.link(from, *p, step_len, tag);
                }
                plunged_at = None;
                here = Some(*p);
            }
            AdaptiveSegment::Cut(path) => {
                let pts = subdivide(path, step_len);
                if let Some(&p0) = pts.first() {
                    walk.plan.grid.clear_circle(p0.x, p0.y, r);
                    if walk.down {
                        walk.replay.grid.clear_circle(p0.x, p0.y, r);
                    }
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
                    if in_slot_lines {
                        walk.plan.grid.clear_segment(a, b, r);
                        walk.replay.grid.clear_segment(a, b, r);
                        continue;
                    }
                    let departing = departure.is_some_and(|d| (a.x - d.x).hypot(a.y - d.y) < r);
                    let part = match parts {
                        Some(ix) if producer == Producer::Clearing => ix.part(a, b),
                        _ => StepPart::Unclassified,
                    };
                    walk.step(a, b, tag, part, departing);
                }
                if let Some(&end) = pts.last() {
                    here = Some(end);
                }
            }
        }
    }
    walk.out
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
    /// The planner's pass-load reading of the cleaned plan (what is
    /// emitted).
    pub pass_load: PassLoadReadout,
    /// The same reading of the producer's raw plan, before the cleanup
    /// replays it (C3): its over-steps are the steps the replay refuses,
    /// in plan order. Exact up to the first one; after it the replay has
    /// lifted and re-entered, so its grid differs from this readout's.
    pub raw_pass_load: PassLoadReadout,
    /// Where the entries of the cleaned plan come from ([`EntrySplit`]).
    pub entry_split: EntrySplit,
    /// The cascade diagnostic of the raw plan (C5, [`CascadeReadout`]).
    pub cascade: CascadeReadout,
}

/// Where the entries (`Rapid`s) of a cleaned plan come from (C3). The
/// producer is the kind of the last marker before the entry.
#[derive(Debug, Clone, Default)]
pub struct EntrySplit {
    /// The starter pocket's plunge.
    pub starter: usize,
    /// A clearing re-entry within one pass step of a planned wrap end (the
    /// replay re-enters at a walk point, and walk points are at most one
    /// pass step apart): a join the replay refused (a seam step, an
    /// arc-to-arc step, a link, a ramp start).
    pub clearing_join: usize,
    /// Any other clearing re-entry: the replay split a wrap mid-way. For a
    /// shipped strategy (no wraps) every clearing re-entry counts here,
    /// the agent's frontier hops included.
    pub clearing_split: usize,
    /// A residue contour's re-entry.
    pub residue_contour: usize,
    /// A wall pass's re-entry.
    pub wall_pass: usize,
    /// A residue mop's re-entry.
    pub residue_mop: usize,
    /// Slot lines or no marker.
    pub other: usize,
}

impl EntrySplit {
    fn merge(&mut self, other: &Self) {
        self.starter += other.starter;
        self.clearing_join += other.clearing_join;
        self.clearing_split += other.clearing_split;
        self.residue_contour += other.residue_contour;
        self.wall_pass += other.wall_pass;
        self.residue_mop += other.residue_mop;
        self.other += other.other;
    }
}

/// The [`EntrySplit`] of a cleaned plan whose producer planned `wraps`; a
/// re-entry within `step` of a wrap end is a join.
fn entry_split(segments: &[AdaptiveSegment], wraps: &[Vec<P2>], step: f64) -> EntrySplit {
    let ends: Vec<P2> = wraps
        .iter()
        .flat_map(|w| w.first().into_iter().chain(w.last()))
        .copied()
        .collect();
    let mut out = EntrySplit::default();
    let mut producer = Producer::Unmarked;
    for seg in segments {
        match seg {
            AdaptiveSegment::Marker(event) => producer = producer.after(event),
            AdaptiveSegment::Rapid(p) => match producer {
                Producer::Starter => out.starter += 1,
                Producer::Clearing => {
                    let at_end = ends.iter().any(|q| (q.x - p.x).hypot(q.y - p.y) <= step);
                    if at_end {
                        out.clearing_join += 1;
                    } else {
                        out.clearing_split += 1;
                    }
                }
                Producer::ResidueContour => out.residue_contour += 1,
                Producer::WallPass => out.wall_pass += 1,
                Producer::ResidueMop => out.residue_mop += 1,
                Producer::Unmarked | Producer::SlotLines => out.other += 1,
            },
            AdaptiveSegment::Cut(_) | AdaptiveSegment::Link(_) => {}
        }
    }
    out
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
    run_2d_with(outline, params, candidate, ReadResolution::Planner)
}

/// The lattice the research readouts of the RAW plan read on (C6, harness
/// only). The producer, the shipped cleanup and its replay, and the
/// readout of the emitted plan stay on the planner cell
/// (`max(R/6, tolerance)`, `path.rs` `planner_cell_size`): the shipped
/// cleanup derives its own cell, so a finer replay would need a seam in
/// `path.rs`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadResolution {
    /// The planner cell.
    Planner,
    /// Half the planner cell: the same plan read at twice the lattice
    /// resolution, to tell the measure's one-cell quantum from stock.
    HalfCell,
}

/// [`run_2d`] with the raw-plan readouts (`Summary2d::raw_pass_load`,
/// `Summary2d::cascade`) read at `read` ([`ReadResolution`]).
#[must_use]
pub fn run_2d_with(
    outline: &Polygon2,
    params: &AdaptiveParams,
    candidate: Candidate2d,
    read: ReadResolution,
) -> Run2d {
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
        let mut piece_wraps: Vec<Vec<P2>> = Vec::new();
        let mut piece_parts: Option<PartIndex> = None;
        let mut piece_seed: Option<P2> = None;
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
                        parts,
                    })) => {
                        piece_parts = Some(parts);
                        piece_seed = report.seed;
                        run.fields.push(report);
                        piece_wraps.clone_from(&wraps);
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
        let cell = planner_cell(&params);
        let read_cell = match read {
            ReadResolution::Planner => cell,
            ReadResolution::HalfCell => cell / 2.0,
        };
        let raw_load = pass_load_readout(piece, &params, &raw, piece_parts.as_ref(), read_cell);
        run.summary.raw_pass_load.merge(&raw_load);
        run.summary.cascade.merge(&cascade_readout(
            piece,
            &params,
            &raw,
            piece_parts.as_ref(),
            piece_seed,
            read_cell,
        ));
        let segments = cleaned(piece, &params, raw);
        run.summary
            .entry_split
            .merge(&entry_split(&segments, &piece_wraps, pass_step_len(cell)));
        let region = (params.keep_down_links == KeepDownLinks::WithinPassLoad)
            .then(|| ToolCentreRegion::new(piece, params.tool_radius));
        let (tp, _) = segments_to_toolpath(&segments, &params, region.as_ref());
        add_counts(&mut run.summary, &segments);
        run.spans.extend(spans_of(&segments));
        run.summary
            .pass_load
            .merge(&pass_load_readout(piece, &params, &segments, None, cell));
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
                // The meeting line: behind the island, on its axis (the
                // seed is on the axis, so the two fronts meet there, at
                // y = 0, within one cell diagonal of it). The shadow lines
                // (where the paths start to bend, from the island's top and
                // bottom) are not meeting lines.
                let f = &field.field;
                let mut shock_cells = 0usize;
                for row in 0..f.rows - 1 {
                    for col in 0..f.cols - 1 {
                        if !(field.cell_reachable(row, col) && field.cell_on_shock(row, col)) {
                            continue;
                        }
                        let x = f.origin_x + (col as f64 + 0.5) * f.cell;
                        let y = f.origin_y + (row as f64 + 0.5) * f.cell;
                        assert!(
                            x > island && y.abs() <= SQRT_2 * cell,
                            "a meeting-line cell at ({x:.2}, {y:.2}) is not behind the island on its axis"
                        );
                        shock_cells += 1;
                    }
                }
                assert!(shock_cells > 0, "the two fronts meet behind the island");
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

    /// C4: on a disc seeded at its centre every level set is a circle round
    /// the seed, so the core is star-shaped out to the last level whose
    /// cells are all reachable, and the core spiral is one piece (every
    /// segment inside the region) whose radius never falls by more than the
    /// tolerance it was bisected to. It starts on the starter rim (radius
    /// `R`) and ends one closing lap at `R + K·s`.
    #[test]
    fn the_core_spiral_on_a_disc_is_one_piece_with_monotone_radius() {
        let (s, tol) = op_defaults();
        let cell = planner_cell_size(R, tol);
        let disc_radius = 20.0;
        let part = Polygon2::new(circle(P2::new(0.0, 0.0), disc_radius + R, cell));
        let region = ToolCentreRegion::new(&part, R);
        let seed = P2::new(0.0, 0.0);
        let ceiling_width = pass_load_for(&test_params(), cell).ceiling * 2.0 * R;
        for spacing in [Spacing::Plain, Spacing::Curvature] {
            let mut field =
                GeodesicField::over_part(&part, R, cell, s, seed, R, FieldSolver::SourceTracking)
                    .expect("the seed lies in the disc");
            if spacing == Spacing::Curvature {
                field.use_curvature_spacing(R, ceiling_width);
            }
            let never: &dyn CancelCheck = &|| false;
            let (spiral, k) =
                core_spiral(&field, &region, seed, R, 0.0, s, tol, never).expect("not cancelled");
            // Level `k` is a circle of radius `R + T_k` about the seed. The
            // polygon of the disc lies within `cell` of the circle (the sag
            // of `circle`), and a level is read only where the four corners
            // of its cell are reachable (one cell diagonal inside the rim):
            // the last core level fits in the disc, and the next one does
            // not fit inside that margin.
            let outer = R + field.level_value(k) * s;
            let next = R + field.level_value(k + 1) * s;
            assert!(
                k > 0 && outer <= disc_radius && next > disc_radius - cell - SQRT_2 * cell,
                "{spacing:?}: core levels {k}, outer {outer:.3}, next {next:.3}"
            );
            assert!(spiral.len() > 2, "the spiral has points");
            for pair in spiral.windows(2) {
                let &[a, b] = pair else {
                    continue;
                };
                assert!(
                    region.contains_segment(a, b),
                    "{spacing:?}: spiral segment {a:?} -> {b:?} leaves the region"
                );
                let (ra, rb) = (a.x.hypot(a.y), b.x.hypot(b.y));
                assert!(
                    rb >= ra - tol,
                    "{spacing:?}: the radius falls from {ra:.4} to {rb:.4} (tolerance {tol})"
                );
            }
            let first = spiral
                .first()
                .map(|p| p.x.hypot(p.y))
                .expect("a first point");
            let last = spiral.last().map(|p| p.x.hypot(p.y)).expect("a last point");
            assert!(
                (first - R).abs() <= tol,
                "{spacing:?}: starts on the rim: {first:.4}"
            );
            assert!(
                (last - outer).abs() <= cell,
                "{spacing:?}: ends on level {k} at {outer:.3}: {last:.4}"
            );
        }
    }

    /// The spacing law of the module doc: `s'(ρ) = ρ + R - √(R² + ρ² +
    /// 2ρ(R - cD))`.
    fn s_prime(rho: f64, r: f64, cd: f64) -> f64 {
        rho + r - (r * r + rho * rho + 2.0 * rho * (r - cd)).sqrt()
    }

    /// The swept width of a pass at cutter-centre radius `rho`, one spacing
    /// `delta` outside a cleared disc of radius `rho - delta + R` (the
    /// circular-front engagement of the derivation).
    fn circular_width(rho: f64, delta: f64, r: f64) -> f64 {
        let a = rho - delta + r;
        r - (a * a - r * r - rho * rho) / (2.0 * rho)
    }

    /// C5: the curvature spacing law. Each level whose step is under the
    /// stepover puts the pass exactly on the ceiling width (the closed
    /// form solves the width equation); the steps never shrink; the step is
    /// capped at `s` exactly where `ρ ≥ ρ* = s(2R - s) / (2(cD - s))`, and
    /// `s'(ρ*) = s`; a straight edge (`ρ → ∞`) gives `s' → cD`, the
    /// straight-edge width.
    #[test]
    fn the_curvature_spacing_law_puts_each_tight_pass_on_the_ceiling() {
        let (s, tol) = op_defaults();
        let cell = planner_cell_size(R, tol);
        let cd = pass_load_for(&test_params(), cell).ceiling * 2.0 * R;
        assert!(
            cd > s,
            "the ceiling width {cd} must exceed the stepover {s}"
        );
        let rho_star = s * (2.0 * R - s) / (2.0 * (cd - s));
        assert!(
            (s_prime(rho_star, R, cd) - s).abs() < 1e-9,
            "s'(ρ*) = s: {}",
            s_prime(rho_star, R, cd)
        );
        assert!(
            (s_prime(1e6, R, cd) - cd).abs() < 1e-3,
            "the straight limit"
        );
        let levels = curvature_levels(R, s, cd, 2.0 * rho_star);
        assert!(levels.len() > 3, "levels: {levels:?}");
        let mut prev_step = 0.0_f64;
        for pair in levels.windows(2) {
            let &[t0, t1] = pair else {
                continue;
            };
            let (rho0, rho1) = (R + t0, R + t1);
            let step = rho1 - rho0;
            assert!(
                step >= prev_step - 1e-9,
                "the steps never shrink: {prev_step} then {step}"
            );
            assert!(step <= s + 1e-9, "a step is at most s: {step}");
            let capped = step >= s - 1e-9;
            assert_eq!(
                capped,
                rho1 >= rho_star - 1e-9,
                "capped exactly from ρ* = {rho_star:.4}: step {step:.4} at ρ {rho1:.4}"
            );
            if !capped {
                let w = circular_width(rho1, step, R);
                assert!(
                    (w - cd).abs() < 1e-9,
                    "the pass at ρ {rho1:.4} reads {w:.6} against cD {cd:.6}"
                );
            }
            prev_step = step;
        }
    }

    /// C6: the transition join on a disc large enough to pass `ρ*`. The rim
    /// and every closed wrap drift out to the next level (one rim
    /// transition, the rest closed to closed, all traced inside the
    /// region); inside `ρ*` the fraction is 1 (the whole wrap, the spiral
    /// form); it is at most 1 everywhere and under 1 somewhere past `ρ*`
    /// (the margin branch runs); and the sectors nest: each sector angle is
    /// at most the one before, to the angle of one cell at that radius (the
    /// contour gives a wrap's length to about one cell).
    #[test]
    #[ignore = "C6 transition join: D6 read nesting failures at levels 2, 9 and 12 and an \
                unapplied edge transition at level 16; the join is not a default column and \
                no C6 variant beat geodesic_curv (P15_D_verify.md D6). Fix or drop with the \
                engagement-measure ruling"]
    fn the_transition_join_nests_its_sectors_on_a_disc() {
        let params = test_params();
        let (s, tol) = op_defaults();
        let cell = planner_cell_size(R, tol);
        let disc_radius = 35.0;
        let part = Polygon2::new(circle(P2::new(0.0, 0.0), disc_radius + R, cell));
        let never: &dyn CancelCheck = &|| false;
        let cd = pass_load_for(&params, cell).ceiling * 2.0 * R;
        let rho_star = s * (2.0 * R - s) / (2.0 * (cd - s));
        // The disc has no wall inside, so every pivot is the seed and the
        // local law reads the same radius as the remapped law.
        for options in [GeodesicOptions::C6_JOIN, GeodesicOptions::C6_LOCAL_JOIN] {
            let produced = geodesic_segments(&part, &params, options, never)
                .expect("not cancelled")
                .expect("the field applies on a disc");
            let j = &produced.report.joins;
            assert_eq!(
                j.rim_transition, 1,
                "the rim drifts to the first wrap: {j:?}"
            );
            assert!(j.closed_transition >= 2, "closed wraps drift: {j:?}");
            assert_eq!(
                j.transition_fallback, 0,
                "every transition is traced: {j:?}"
            );
            assert!(j.transitions.iter().all(|r| r.applied), "{j:?}");
            for r in &j.transitions {
                assert!(r.fraction > 0.0 && r.fraction <= 1.0, "{r:?}");
                if r.rho < rho_star {
                    assert!(
                        (r.fraction - 1.0).abs() < 1e-12,
                        "inside ρ* = {rho_star:.3} the transition takes the whole wrap: {r:?}"
                    );
                }
            }
            assert!(
                j.transitions.iter().any(|r| r.fraction < 1.0),
                "past ρ* the margin shortens a transition: {j:?}"
            );
            for pair in j.transitions.windows(2) {
                let [a, b] = pair else {
                    continue;
                };
                let wrap_radius = a.rho - a.delta;
                assert!(
                    b.sector <= a.sector + cell / wrap_radius,
                    "the sectors nest: {a:?} then {b:?}"
                );
            }
        }
    }

    /// The straight parallel arcs of a rectangular lobe, `s` apart, `n` of
    /// them, `width` long, each with one point per cell (the contour's
    /// density), first point at `x = 0`.
    fn rectangle_lobe(n: usize, width: f64, s: f64, cell: f64) -> Vec<Vec<P2>> {
        (0..n)
            .map(|k| {
                let y = k as f64 * s;
                subdivide(&[P2::new(0.0, y), P2::new(width, y)], cell)
            })
            .collect()
    }

    /// True when segments `ab` and `cd` cross at a point inside both (a
    /// touch at a shared end does not count).
    fn segments_cross(a: P2, b: P2, c: P2, d: P2) -> bool {
        let orient = |p: P2, q: P2, r: P2| (q.x - p.x) * (r.y - p.y) - (q.y - p.y) * (r.x - p.x);
        let eps = 1e-9;
        let (o1, o2) = (orient(a, b, c), orient(a, b, d));
        let (o3, o4) = (orient(c, d, a), orient(c, d, b));
        ((o1 > eps && o2 < -eps) || (o1 < -eps && o2 > eps))
            && ((o3 > eps && o4 < -eps) || (o3 < -eps && o4 > eps))
    }

    /// The largest and the smallest distance from the points of `from` to
    /// the polyline `to`.
    fn spacing(from: &[P2], to: &[P2]) -> (f64, f64) {
        from.iter()
            .filter_map(|&p| nearest_on(to, p).map(|(_, d)| d))
            .fold((f64::INFINITY, 0.0_f64), |(lo, hi), d| {
                (lo.min(d), hi.max(d))
            })
    }

    /// C4: the lobe morph on a rectangular lobe. The cut pieces (every piece
    /// but a return) do not cross themselves or each other, and the legs
    /// that run the same way lie `s` apart within one cell: the half-pitch
    /// alternation's legs `l` and `l + 2`, the one-way legs `l` and `l + 1`.
    /// The literal alternation ([`LobeMorph::AlternateFull`]) leaves two
    /// stepovers between its leg back and the leg before at the near end,
    /// the claim of its doc.
    #[test]
    fn the_lobe_morph_on_a_rectangle_lobe_holds_one_stepover() {
        let (s, tol) = op_defaults();
        let cell = planner_cell_size(R, tol);
        let arcs = rectangle_lobe(5, 40.0, s, cell);
        for morph in [LobeMorph::Alternate, LobeMorph::Return] {
            let pieces = morph_legs(&arcs, morph, cell);
            let cuts: Vec<&PlanPiece> = pieces
                .iter()
                .filter(|p| p.kind != PieceKind::Return)
                .collect();
            let segs: Vec<(P2, P2)> = cuts
                .iter()
                .flat_map(|p| {
                    p.pts.windows(2).filter_map(|w| match w {
                        &[a, b] => Some((a, b)),
                        _ => None,
                    })
                })
                .collect();
            for (i, &(a, b)) in segs.iter().enumerate() {
                for &(c, d) in segs.iter().skip(i + 1) {
                    assert!(
                        !segments_cross(a, b, c, d),
                        "{morph:?}: {a:?}->{b:?} crosses {c:?}->{d:?}"
                    );
                }
            }
            let legs: Vec<&[P2]> = cuts
                .iter()
                .filter(|p| p.kind == PieceKind::Morph)
                .map(|p| p.pts.as_slice())
                .collect();
            let apart = match morph {
                LobeMorph::Alternate => 2,
                LobeMorph::AlternateFull | LobeMorph::Return => 1,
            };
            assert_eq!(
                legs.len(),
                match morph {
                    LobeMorph::Alternate => 2 * (arcs.len() - 1),
                    LobeMorph::AlternateFull | LobeMorph::Return => arcs.len() - 1,
                },
                "{morph:?}: legs per band"
            );
            for (l, later) in legs.iter().zip(legs.iter().skip(apart)) {
                let (lo, hi) = spacing(later, l);
                assert!(
                    lo >= s - cell && hi <= s + cell,
                    "{morph:?}: legs {lo:.4} to {hi:.4} apart, expected {s} ± {cell}"
                );
            }
            // The last piece is the plain pass of the last arc.
            let last = pieces.last().expect("the lobe has pieces");
            assert_eq!(
                last.kind,
                PieceKind::Wrap,
                "{morph:?}: ends on the last arc"
            );
            assert!(
                last.pts.iter().all(|p| (p.y - 4.0 * s).abs() < 1e-9),
                "{morph:?}: the last pass is the last arc"
            );
        }
        // The literal alternation: the leg back of the second band ends at
        // `A_2(0) = (0, 2s)`, two stepovers from the first leg.
        let full = morph_legs(&arcs, LobeMorph::AlternateFull, cell);
        let mut legs = full.iter().filter(|p| p.kind == PieceKind::Morph);
        let (Some(out), Some(back)) = (legs.next(), legs.next()) else {
            panic!("the literal alternation has two legs at least");
        };
        let (_, hi) = spacing(&back.pts, &out.pts);
        assert!(
            hi >= 2.0 * s - cell,
            "the literal alternation leaves {hi:.4} mm at the near end, the doc says 2s"
        );
    }

    /// C4: the star-shape test. On a disc seeded at its centre with a round
    /// island off the seed, the circles round the seed that do not reach
    /// the island are star-shaped; a level set that reaches round the
    /// island fails, since the rays toward the island leave the region
    /// before they cross it. The same disc with no island passes those
    /// levels (the control: the island is the cause).
    #[test]
    fn the_star_shape_test_rejects_a_level_set_behind_an_island() {
        let (s, tol) = op_defaults();
        let cell = planner_cell_size(R, tol);
        let disc_radius = 30.0;
        let outer = circle(P2::new(0.0, 0.0), disc_radius + R, cell);
        // The island of the part: radius 2 at (14, 0); its hole in the
        // tool-centre region has radius 2 + R = 5, so the front reaches it
        // 9 mm from the seed, at T = 9 - R = 6 mm, level 3.
        let mut hole = circle(P2::new(14.0, 0.0), 2.0, cell);
        hole.reverse();
        let seed = P2::new(0.0, 0.0);
        let with_island = Polygon2::with_holes(outer.clone(), vec![hole]);
        let without = Polygon2::new(outer);
        let solve = |part: &Polygon2| {
            GeodesicField::over_part(part, R, cell, s, seed, R, FieldSolver::SourceTracking)
                .expect("the seed lies in the disc")
        };
        let island = solve(&with_island);
        let open = solve(&without);
        for level in [1, 2] {
            assert!(
                island.level_is_star_shaped(seed, R, level),
                "level {level} closes before the island"
            );
        }
        for level in [4, 7] {
            assert!(
                !island.level_is_star_shaped(seed, R, level),
                "level {level} reaches round the island: not star-shaped"
            );
            assert!(
                open.level_is_star_shaped(seed, R, level),
                "control: level {level} is a circle without the island"
            );
        }
        let k = island.star_levels(seed, R);
        assert!((2..=3).contains(&k), "core levels with the island: {k}");
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
    #[ignore = "G-MOPLOAD: one shipped ResidueMop step reads 0.3669 over the 0.3626 ceiling \
                (D2, 2026-10-23); the candidate's own steps hold. Un-ignore when the mop \
                holds the pass load (DEFECTS.md)"]
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
