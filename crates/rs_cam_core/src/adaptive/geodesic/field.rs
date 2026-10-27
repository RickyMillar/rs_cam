//! The geodesic field `T` on the planner lattice: the source-tracked solve
//! ([`track_sources`]), its front branches ([`Branches`]), the local-level
//! spacing of its levels ([`GeodesicField::use_level_spacing`]), and the
//! level sets as chained polylines ([`chain_whole`]). The module doc of
//! `adaptive::geodesic` states the rules.

use std::collections::HashMap;

use crate::finish::scallop_isofield::{IsoScallopField, iso_segments_where};
use crate::geo::{P2, point_to_segment_distance};
use crate::geometry::marching_squares::{CHAIN_EPS, CHAIN_QUANTIZE_SCALE};
use crate::interrupt::{CancelCheck, Cancelled, check_cancel};

use crate::adaptive::path::pass_step_len;
use crate::adaptive::search::ToolCentreRegion;

use super::wraps::{is_closed, law_spacing};
use crate::adaptive::MaterialGrid;

// ── The field ──────────────────────────────────────────────────────────

/// The lattice the field is solved on: node `(row, col)` stands at
/// `(origin_x + col · cell, origin_y + row · cell)`, the convention of
/// `MaterialGrid`.
#[derive(Debug, Clone, Copy)]
pub(super) struct Lattice {
    pub(super) rows: usize,
    pub(super) cols: usize,
    pub(super) origin_x: f64,
    pub(super) origin_y: f64,
    pub(super) cell: f64,
}

impl Lattice {
    pub(super) fn of_grid(grid: &MaterialGrid) -> Self {
        Self {
            rows: grid.rows,
            cols: grid.cols,
            origin_x: grid.origin_x,
            origin_y: grid.origin_y,
            cell: grid.cell_size,
        }
    }

    pub(super) fn node(&self, index: usize) -> P2 {
        let (row, col) = (index / self.cols, index % self.cols);
        P2::new(
            self.origin_x + col as f64 * self.cell,
            self.origin_y + row as f64 * self.cell,
        )
    }
}

/// The straight path of a node to the seed disc starts at `at`, where the
/// distance is `t_mm` (the seed centre, `-R`; or a bend node). `branch`
/// names the side of each wall the path went round ([`Branches`]).
#[derive(Debug, Clone, Copy)]
pub(super) struct Pivot {
    pub(super) at: P2,
    pub(super) t_mm: f64,
    pub(super) branch: u32,
}

/// The branches of the front (G1-2). Branch 0 is every node the seed centre
/// sees. A path that bends round a wall ring on one side starts a new
/// branch, keyed by (parent branch, ring, left turn); further bends round
/// the same ring on the same side stay in it. Two neighbouring nodes in
/// different branches lie on the two sides of a meeting line (a shock: the
/// fronts that went round the two sides of an island meet there).
#[derive(Debug, Default)]
pub(super) struct Branches {
    pub(super) keys: Vec<(u32, usize, bool)>,
}

impl Branches {
    pub(super) fn key(&self, id: u32) -> Option<(u32, usize, bool)> {
        let index = (id as usize).checked_sub(1)?;
        self.keys.get(index).copied()
    }

    pub(super) fn bend(&mut self, parent: u32, ring: usize, left: bool) -> u32 {
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
pub(super) struct Walls<'a> {
    pub(super) region: &'a ToolCentreRegion,
}

impl Walls<'_> {
    pub(super) fn visible(&self, a: P2, b: P2) -> bool {
        self.region.contains_segment(a, b)
    }

    /// The index of the ring (exterior or hole, over every piece) nearest
    /// `p`.
    pub(super) fn ring_near(&self, p: P2) -> usize {
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
pub(super) const NEIGHBOURS: [(isize, isize); 8] = [
    (-1, -1),
    (-1, 0),
    (-1, 1),
    (0, -1),
    (0, 1),
    (1, -1),
    (1, 0),
    (1, 1),
];

/// The source-tracked solve of the field on `lattice`. `t_mm` holds the seeded
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
/// path starts from; the seed for a node never reached), true when a
/// round changed nothing, and the branch table.
pub(super) fn track_sources(
    lattice: &Lattice,
    inside: &[bool],
    fixed: &[bool],
    t_mm: &mut [f64],
    seed: P2,
    seed_radius: f64,
    walls: &Walls<'_>,
) -> (Vec<u32>, Vec<u32>, Vec<P2>, bool, Branches) {
    let pivots: Vec<Option<Pivot>> = fixed
        .iter()
        .map(|&f| {
            f.then_some(Pivot {
                at: seed,
                t_mm: -seed_radius,
                branch: 0,
            })
        })
        .collect();
    sweep_sources(
        lattice,
        inside,
        fixed,
        t_mm,
        pivots,
        Branches::default(),
        seed,
        walls,
    )
}

/// The sweep of [`track_sources`] from the pivots `pivots` (one per fixed
/// node) and the branch table `branches`; `unreached` is the pivot of a
/// node the sweep never reaches.
#[expect(
    clippy::too_many_arguments,
    reason = "each parameter is a separate input of one solve"
)]
pub(super) fn sweep_sources(
    lattice: &Lattice,
    inside: &[bool],
    fixed: &[bool],
    t_mm: &mut [f64],
    mut pivots: Vec<Option<Pivot>>,
    mut branches: Branches,
    unreached: P2,
    walls: &Walls<'_>,
) -> (Vec<u32>, Vec<u32>, Vec<P2>, bool, Branches) {
    let (rows, cols) = (lattice.rows, lattice.cols);
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
    let at = pivots
        .iter()
        .map(|p| p.map_or(unreached, |p| p.at))
        .collect();
    // The parent of each branch: 0 for the root and its children.
    let parent = std::iter::once(0)
        .chain(branches.keys.iter().map(|&(parent, _, _)| parent))
        .collect();
    (branch, parent, at, converged, branches)
}

/// The geodesic distance from an entry disc over a region whose holes and
/// outside are unreachable, in stepovers.
pub(super) struct GeodesicField {
    /// The solved field. `passes` holds `T / stepover`, negative inside the
    /// seed disc and `INFINITY` at an unreachable node; `ring_count` is the
    /// number of wraps, `⌊max T / stepover⌋`.
    pub(super) field: IsoScallopField,
    pub(super) inside: Vec<bool>,
    /// The front branch of each node ([`Branches`]).
    pub(super) branch: Vec<u32>,
    /// The parent of each front branch (index = branch id; the root's is 0).
    pub(super) branch_parent: Vec<u32>,
    pub(super) stepover: f64,
    /// The value of `T` (stepovers) of each level, `levels[0] = 0` (the
    /// starter rim), up to the largest `T` of the field: `0, 1, 2, ...`
    /// from the solve, then the local-level spacing
    /// ([`Self::use_level_spacing`]): closer levels while the front is
    /// tightly curved.
    pub(super) levels: Vec<f64>,
    /// The largest finite `T` (stepovers).
    pub(super) max_t: f64,
}

impl GeodesicField {
    pub(super) fn on_lattice(
        lattice: Lattice,
        inside: Vec<bool>,
        seed: P2,
        seed_radius: f64,
        stepover: f64,
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
        let (branch, branch_parent, _, _, _) = track_sources(
            &lattice,
            &inside,
            &fixed,
            &mut t_mm,
            seed,
            seed_radius,
            walls,
        );
        let passes: Vec<f64> = t_mm.iter().map(|t| t / stepover).collect();
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
            levels,
            max_t: max_pass,
        })
    }

    /// The local-level spacing (research C16): space the levels of `T` by the law at
    /// each level's own curvature. Level `k` is ONE value `T_k`, so the
    /// levels stay parallel level sets of the one field. From `T_{k-1}`, the step `ΔT` (stepovers) is the
    /// largest for which every point `p` of the trial level set
    /// `T = T_{k-1} + ΔT` lies within `min(s, s'(ρ(p)))` of the cut
    /// ([`Self::level_gap`]), with `ρ(p)` the level set's own signed radius
    /// at `p` over one pass step ([`Self::front_radius`]).
    ///
    /// - The gap: `T` is a distance, so a point of the trial level lies
    ///   `ΔT·s` along its shortest path from the level before, and its
    ///   straight distance to that level is at most `ΔT·s`. The test
    ///   `ΔT·s ≤ min_p min(s, s'(ρ(p)))` is therefore the rule read on the
    ///   path length, never an under-read of the gap.
    /// - `s'(ρ)` is read at the trial (outer) level, the root form of
    ///   the law (`ρ_k = -(R - cD) + √((R - cD)² + (ρ_{k-1} + R)² - R²)`):
    ///   on a disc about the seed `ρ = R + T`.
    /// - The bisection: `ΔT = 1` when it holds; else halve `[0, 1]` until
    ///   the bracket is at most the op `tolerance` (mm), the precision the
    ///   plan is held to (the rule of [`Self::radius_at`]), and take the
    ///   lower end, the side that holds the rule. The test is not monotone
    ///   in `ΔT` in general (`ρ` changes with the level), so the bisection
    ///   finds one value at which it holds within the tolerance of one at
    ///   which it fails.
    /// - The walk ends when the next level passes the largest `T`, as
    ///   field ends; or, as a guard, when no step above the
    ///   bisection's first bracket holds (a radius under about one pass
    ///   step's half, which the three-point reading cannot return).
    pub(super) fn use_level_spacing(
        &mut self,
        tool_radius: f64,
        ceiling_width: f64,
        tolerance: f64,
        cancel: &dyn CancelCheck,
    ) -> Result<(), Cancelled> {
        let s = self.stepover;
        let span = pass_step_len(self.field.cell);
        let fits =
            |t: f64, dt: f64| dt * s <= self.level_gap(t + dt, tool_radius, ceiling_width, span);
        let mut levels = vec![0.0];
        let mut t = 0.0_f64;
        loop {
            check_cancel(cancel)?;
            let mut dt = 1.0_f64;
            if !fits(t, dt) {
                let (mut lo, mut hi) = (0.0_f64, 1.0_f64);
                while (hi - lo) * s > tolerance {
                    let mid = 0.5 * (lo + hi);
                    if fits(t, mid) {
                        lo = mid;
                    } else {
                        hi = mid;
                    }
                }
                dt = lo;
            }
            if dt <= 0.0 || t + dt > self.max_t {
                break;
            }
            t += dt;
            levels.push(t);
        }
        self.levels = levels;
        Ok(())
    }

    /// C35 ([`held_core_spiral`]): the turn values of `T` (stepovers) of a
    /// core spiral from the rim (`T = 0`) to level `k`, by the walk of
    /// [`Self::use_level_spacing`] at the width `ceiling_width`. The walk
    /// ends on level `k` itself: a step that would pass it stops on it.
    /// `None` when a step holds nothing above the bisection's first
    /// bracket (the guard of [`Self::use_level_spacing`]).
    pub(super) fn spiral_turn_levels(
        &self,
        k: usize,
        tool_radius: f64,
        ceiling_width: f64,
        tolerance: f64,
        cancel: &dyn CancelCheck,
    ) -> Result<Option<Vec<f64>>, Cancelled> {
        let s = self.stepover;
        let end = self.level_value(k);
        let span = pass_step_len(self.field.cell);
        let fits =
            |t: f64, dt: f64| dt * s <= self.level_gap(t + dt, tool_radius, ceiling_width, span);
        let mut levels = vec![0.0];
        let mut t = 0.0_f64;
        while t < end {
            check_cancel(cancel)?;
            let mut dt = (end - t).min(1.0);
            if !fits(t, dt) {
                let (mut lo, mut hi) = (0.0_f64, dt);
                while (hi - lo) * s > tolerance {
                    let mid = 0.5 * (lo + hi);
                    if fits(t, mid) {
                        lo = mid;
                    } else {
                        hi = mid;
                    }
                }
                dt = lo;
            }
            if dt <= 0.0 {
                return Ok(None);
            }
            t = if t + dt >= end { end } else { t + dt };
            levels.push(t);
        }
        Ok(Some(levels))
    }

    /// C16: the least law spacing (mm) over the level set `T = t`: at each
    /// vertex of each piece, `min(s, s'(ρ))` ([`law_spacing`]) with `ρ` the
    /// piece's signed radius there over `span` of arc length each side
    /// ([`Self::front_radius`]). A vertex with less than `span` of the
    /// piece on one side (an arc end at a wall or at a meeting line) gives
    /// no reading; its neighbours `span` in read the piece. `INFINITY` when
    /// no vertex reads (no level set, or every piece shorter than `2·span`).
    pub(super) fn level_gap(&self, t: f64, tool_radius: f64, ceiling_width: f64, span: f64) -> f64 {
        let cell = self.field.cell;
        let mut least = f64::INFINITY;
        for mut pts in self.chains_at(t) {
            let closed = is_closed(&pts, cell);
            if closed && let Some(&first) = pts.first() {
                pts.push(first);
            }
            let mut cum = Vec::with_capacity(pts.len());
            let mut run = 0.0_f64;
            let mut prev: Option<P2> = None;
            for &p in &pts {
                if let Some(q) = prev {
                    run += (p.x - q.x).hypot(p.y - q.y);
                }
                cum.push(run);
                prev = Some(p);
            }
            if run < 2.0 * span {
                continue;
            }
            let at = |u: f64| {
                let u = if closed { u.rem_euclid(run) } else { u };
                point_at_length(&pts, &cum, u)
            };
            for (&p, &u) in pts.iter().zip(&cum) {
                if !closed && (u < span || u > run - span) {
                    continue;
                }
                let (Some(a), Some(b)) = (at(u - span), at(u + span)) else {
                    continue;
                };
                let rho = self.front_radius(a, p, b);
                let gap = if rho.is_finite() {
                    law_spacing(tool_radius, self.stepover, ceiling_width, rho)
                } else {
                    self.stepover
                };
                least = least.min(gap);
            }
        }
        least
    }

    /// C16: the signed radius (mm) of the circle through `a`, `p` and `b`
    /// (three points of one level set): its radius when its centre lies on
    /// the cleared side of `p` (lower `T`, a convex front); `INFINITY` when
    /// the three are collinear or the centre lies on the stock side (a
    /// concave or straight piece takes the stepover, C15 "The level rule").
    /// The side is read from `T` one cell from `p` toward the centre and
    /// one cell away from it; where either sample is off the reachable
    /// cells, the piece is taken as convex (the smaller spacing).
    pub(super) fn front_radius(&self, a: P2, p: P2, b: P2) -> f64 {
        let d = 2.0 * (a.x * (p.y - b.y) + p.x * (b.y - a.y) + b.x * (a.y - p.y));
        let (sa, sp, sb) = (
            a.x * a.x + a.y * a.y,
            p.x * p.x + p.y * p.y,
            b.x * b.x + b.y * b.y,
        );
        let ux = (sa * (p.y - b.y) + sp * (b.y - a.y) + sb * (a.y - p.y)) / d;
        let uy = (sa * (b.x - p.x) + sp * (a.x - b.x) + sb * (p.x - a.x)) / d;
        let rho = (ux - p.x).hypot(uy - p.y);
        if !rho.is_finite() || rho <= 0.0 {
            return f64::INFINITY;
        }
        let h = self.field.cell;
        let (nx, ny) = ((ux - p.x) / rho, (uy - p.y) / rho);
        match (
            self.sample(P2::new(p.x + h * nx, p.y + h * ny)),
            self.sample(P2::new(p.x - h * nx, p.y - h * ny)),
        ) {
            (Some(toward), Some(away)) if toward >= away => f64::INFINITY,
            _ => rho,
        }
    }

    /// The value of `T` (stepovers) of level `k`; past the last level, the
    /// last value plus one stepover per level.
    pub(super) fn level_value(&self, k: usize) -> f64 {
        match self.levels.get(k) {
            Some(&v) => v,
            None => {
                let last = self.levels.last().copied().unwrap_or(0.0);
                last + (k + 1 - self.levels.len().max(1)) as f64
            }
        }
    }

    /// The number of wraps: the levels above the rim (`⌊max T / s⌋` under
    /// plain spacing).
    #[must_use]
    pub(super) fn level_count(&self) -> usize {
        self.levels.len().saturating_sub(1)
    }

    /// The lattice cell (mm).
    #[must_use]
    pub(super) fn cell(&self) -> f64 {
        self.field.cell
    }

    pub(super) fn reachable(&self, row: usize, col: usize) -> bool {
        self.inside.get(row * self.field.cols + col).copied() == Some(true)
    }

    /// True when the four corners of the cell at `(row, col)` are reachable.
    pub(super) fn cell_reachable(&self, row: usize, col: usize) -> bool {
        self.reachable(row, col)
            && self.reachable(row, col + 1)
            && self.reachable(row + 1, col)
            && self.reachable(row + 1, col + 1)
    }

    /// True when the corners of the cell at `(row, col)` lie in more than one
    /// front branch: the cell holds a meeting line.
    pub(super) fn cell_on_shock(&self, row: usize, col: usize) -> bool {
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
    pub(super) fn related(&self, a: u32, b: u32) -> bool {
        self.is_ancestor(a, b) || self.is_ancestor(b, a)
    }

    /// True when `a` is `b` or lies on the parent chain of `b`.
    pub(super) fn is_ancestor(&self, a: u32, b: u32) -> bool {
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
    /// The cells on a meeting line are left out too, so a wrap ends at the meeting line instead of turning through
    /// its kink.
    pub(super) fn level_chains(&self, level: usize) -> Vec<Vec<P2>> {
        self.chains_at(self.level_value(level))
    }

    /// [`Self::level_chains`] at the value `t` (stepovers), chained whole
    /// ([`chain_whole`]).
    pub(super) fn chains_at(&self, t: f64) -> Vec<Vec<P2>> {
        let kept =
            |row: usize, col: usize| self.cell_reachable(row, col) && !self.cell_on_shock(row, col);
        chain_whole(&iso_segments_where(&self.field, t, &kept))
    }

    /// `T` (stepovers) at `p`, bilinear over the cell that holds `p`, the
    /// interpolation the level sets use. `None` unless the four corners of
    /// the cell are reachable and solved.
    pub(super) fn sample(&self, p: P2) -> Option<f64> {
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
    pub(super) fn ray_step(&self) -> f64 {
        self.field.cell / 2.0
    }

    /// The length of the longest ray: the lattice diagonal.
    pub(super) fn ray_length(&self) -> f64 {
        let f = &self.field;
        (f.rows as f64).hypot(f.cols as f64) * f.cell
    }

    /// `T` along the ray from `seed` at angle `phi`, from radius `r0` to
    /// the lattice edge, at [`Self::ray_step`].
    pub(super) fn ray(&self, seed: P2, phi: f64, r0: f64) -> Vec<Option<f64>> {
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
    pub(super) fn ray_crosses_once(ray: &[Option<f64>], level: f64) -> bool {
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
    pub(super) fn star_rays(&self) -> usize {
        ((std::f64::consts::TAU * self.ray_length() / self.field.cell).ceil() as usize).max(3)
    }

    /// The core zone (C4): the largest `k` such that each level set
    /// `T = 1, ..., k` is star-shaped about `seed` and closed round it:
    /// each of [`Self::star_rays`] rays from the seed disc rim (radius
    /// `r0`) crosses it exactly once, before the ray leaves the reachable
    /// cells. A level set behind an island fails: the rays toward the
    /// island leave the region before they cross it.
    pub(super) fn star_levels(&self, seed: P2, r0: f64) -> usize {
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

    /// The radius on the ray from `seed` at angle `phi` where `T` first
    /// reaches `t` (stepovers), walked out from `r0` at [`Self::ray_step`]
    /// and bisected to `tolerance`. `r0` when `T(r0) ≥ t`. `None` when the
    /// ray leaves the reachable cells first.
    pub(super) fn radius_at(
        &self,
        seed: P2,
        phi: f64,
        r0: f64,
        t: f64,
        tolerance: f64,
    ) -> Option<f64> {
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
}

/// True when `a` and `b` are the same chaining point.
pub(super) fn coincide(a: P2, b: P2) -> bool {
    (a.x - b.x).hypot(a.y - b.y) <= CHAIN_EPS
}

/// The whole chaining (research C16): chain the contour segments of one level
/// (`iso_segments_where`) into polylines. Each chain grows at its tail and
/// then at its head, and a chain of one segment is kept; a loop closes
/// without its closing point (the `chain_segments` form). Endpoints match
/// within `CHAIN_EPS`, looked up on the quantized grid of `chain_segments`
/// (`CHAIN_QUANTIZE_SCALE`, the 3 x 3 neighbour keys).
///
/// The reason (C16 diagnosis): `chain_segments` grows each chain at its
/// tail only and drops a chain of fewer than three points. An open arc
/// whose cells the row-major scan meets against the arc's direction (a
/// level set falling to the right in world XY, one cell per row where it
/// is steep) is cut into pieces of two segments, and the one segment
/// between two pieces is a chain of two points: dropped. A fragment join
/// cannot bridge the hole it leaves. On the star the arm whose arcs fall
/// to the right (the arm to (5, 35)) came out in pieces of about 1 mm.
pub(super) fn chain_whole(segments: &[(P2, P2)]) -> Vec<Vec<P2>> {
    let key = |p: P2| {
        (
            (p.x * CHAIN_QUANTIZE_SCALE).round() as i64,
            (p.y * CHAIN_QUANTIZE_SCALE).round() as i64,
        )
    };
    let mut index: HashMap<(i64, i64), Vec<usize>> = HashMap::with_capacity(2 * segments.len());
    for (i, &(a, b)) in segments.iter().enumerate() {
        index.entry(key(a)).or_default().push(i);
        index.entry(key(b)).or_default().push(i);
    }
    // The unused segment with an end at `p`, and its other end.
    let next = |p: P2, used: &[bool]| -> Option<(usize, P2)> {
        let (kx, ky) = key(p);
        for dx in -1..=1 {
            for dy in -1..=1 {
                for &i in index.get(&(kx + dx, ky + dy)).into_iter().flatten() {
                    if used.get(i).copied() != Some(false) {
                        continue;
                    }
                    let Some(&(a, b)) = segments.get(i) else {
                        continue;
                    };
                    if coincide(a, p) {
                        return Some((i, b));
                    }
                    if coincide(b, p) {
                        return Some((i, a));
                    }
                }
            }
        }
        None
    };
    let mut used = vec![false; segments.len()];
    let mut out = Vec::new();
    for (start, &(a, b)) in segments.iter().enumerate() {
        if used.get(start).copied() != Some(false) {
            continue;
        }
        if let Some(u) = used.get_mut(start) {
            *u = true;
        }
        let mut chain = vec![a, b];
        let mut closed = false;
        while let Some(&tail) = chain.last() {
            let Some((i, q)) = next(tail, &used) else {
                break;
            };
            if let Some(u) = used.get_mut(i) {
                *u = true;
            }
            if coincide(q, a) {
                closed = true;
                break;
            }
            chain.push(q);
        }
        if !closed {
            let mut head: Vec<P2> = Vec::new();
            let mut end = a;
            while let Some((i, q)) = next(end, &used) {
                if let Some(u) = used.get_mut(i) {
                    *u = true;
                }
                head.push(q);
                end = q;
            }
            if !head.is_empty() {
                head.reverse();
                head.extend(chain);
                chain = head;
            }
        }
        out.push(chain);
    }
    out
}

/// The point at arc length `u` along `pts`, whose cumulative lengths are
/// `cum`; `None` off the polyline.
pub(super) fn point_at_length(pts: &[P2], cum: &[f64], u: f64) -> Option<P2> {
    let total = *cum.last()?;
    if !(0.0..=total).contains(&u) {
        return None;
    }
    let k = cum.partition_point(|&c| c < u).max(1);
    let (a, b, ca, cb) = (pts.get(k - 1)?, pts.get(k)?, cum.get(k - 1)?, cum.get(k)?);
    let t = if cb > ca { (u - ca) / (cb - ca) } else { 0.0 };
    Some(P2::new(a.x + t * (b.x - a.x), a.y + t * (b.y - a.y)))
}
