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
//! - Solver: the one Eikonal solver of the crate,
//!   `finish::scallop_isofield::fast_sweep` (Godunov upwind fast sweeping).
//!   The speed is the op stepover at every node, so `T` comes out in
//!   stepovers and the integer levels are one stepover apart.
//! - Seed: the entry disc. The starter pocket (`path.rs`,
//!   `emit_helical_starter_pocket`) plunges at the EDT-max entry
//!   (`search.rs`, `find_entry_via_distance_transform`) and ends with one lap
//!   of the cutter centre at radius `R` (`starter_spiral_points`). That lap is
//!   the cleared boundary the front starts from, so the seed is the disc of
//!   radius `R` about the plunge point. Each node within one cell of the disc
//!   holds its exact signed distance `(|x - c| - R) / s` and is fixed, the way
//!   `seed_boundary` pins the region edge in the iso-scallop field.
//! - Grid: the planner's material-grid lattice, cell `max(R / 6, tolerance)`
//!   (`path.rs`, `adaptive_segments_with_debug`:
//!   `let cell_size = (tool_radius / 6.0).max(tolerance);`). A node is
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
//! # The join and the order
//!
//! - A closed wrap is lapped the way `spiral.rs` laps its wraps: rotated to
//!   start at the point nearest the cutter (`spiral::nearest_index`), cut
//!   round, and closed on its seam.
//! - An open wrap is cut from the end nearer the cutter to the other end. In
//!   a lobe the arcs therefore alternate direction (climb, then
//!   conventional).
//! - A seam step no longer than the reach (below) is a cutting step inside
//!   the `Cut`, as in `spiral.rs`. A longer join is a keep-down `Link`; the
//!   replay admits it under the pass load or turns it into a re-entry.
//! - The order rule (grow outward from the cleared side): a wrap at level `k`
//!   is cut only after every wrap at level `k - 1` within the reach of it.
//!   Among the wraps that may be cut, the one nearest the cutter is next.
//!   So every wrap has the previous level, one stepover away, on its cleared
//!   side, and the cutter fills one lobe to its end before it moves to the
//!   next lobe (a level-by-level order would jump between lobes at each
//!   level).
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
//! - The readout steps are `3 · cell` long, the pass step of the planner
//!   (`path.rs`, `LinkLoad::new`: `step_len: grid.cell_size * 3.0`).
//!
//! # Known risks (R3, Source 3)
//!
//! - Rarefaction at the source: the front fans out of the entry disc. The
//!   exact seeding within one cell of the disc holds it.
//! - Rarefaction at an island's convex corner: the front fans round the
//!   corner, and the first-order update over-reads the distance there. The
//!   island unit test reads 2.5 cells over the exact geodesic behind a round
//!   island (prototype, 2026-10-23): the wraps bunch behind an island, which
//!   is safe for the load and not exact for the spacing.
//! - Shocks: where two fronts meet (behind an island, at a lobe junction) the
//!   level set has a kink. A wrap that crosses the meeting line turns
//!   sharply; the replay splits a step there that is over the ceiling.
//! - The sweep cap: `fast_sweep` stops after 24 rounds (`MAX_ROUNDS`, an
//!   inherited value of the iso-scallop module). A region where the front
//!   must turn many times (a spiral corridor, many islands) may need more.
//!   [`FieldReport::converged`] says when the cap stopped the sweep.
//! - The field error grows with the distance from the seed: 0.62 to 0.72 of
//!   a cell at 20 mm, 0.89 to 1.03 cells at 45 to 60 mm (prototype). The
//!   error is always an over-read.
//! - The seam step of a closed wrap is radial, so the replay may refuse it
//!   (the contour spiral has the same seam).

use std::f64::consts::SQRT_2;

use crate::finish::scallop_isofield::{IsoScallopField, fast_sweep, iso_contour_where};
use crate::geo::{P2, point_to_segment_distance};
use crate::geometry::marching_squares::CHAIN_EPS;
use crate::interrupt::{CancelCheck, Cancelled, check_cancel};
use crate::polygon::{Polygon2, offset_polygon};
use crate::toolpath::{MoveIntent, Toolpath};

use super::path::{
    AdaptiveSegment, LinkLoad, apply_contour_parallel_residue_cleanup, apply_residue_mop_cleanup,
    emit_helical_starter_pocket, pass_load_for, segments_to_toolpath, subdivide,
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

/// The geodesic distance from an entry disc over a region whose holes and
/// outside are unreachable, in stepovers.
pub struct GeodesicField {
    /// The solved field. `passes` holds `T / stepover`, negative inside the
    /// seed disc and `INFINITY` at an unreachable node; `ring_count` is the
    /// number of wraps, `⌊max T / stepover⌋`.
    field: IsoScallopField,
    inside: Vec<bool>,
    stepover: f64,
    converged: bool,
}

impl GeodesicField {
    /// Solve the field over `region` (the tool-centre region: the cutter
    /// centre may stand inside it) from the disc of `seed_radius` about
    /// `seed`, on a lattice of `cell` over the region's bounding box. `None`
    /// when no reachable node lies within one cell of the disc.
    #[must_use]
    pub fn over_region(
        region: &Polygon2,
        cell: f64,
        stepover: f64,
        seed: P2,
        seed_radius: f64,
    ) -> Option<Self> {
        let lattice = Lattice::over(region, cell);
        let inside = (0..lattice.rows * lattice.cols)
            .map(|i| region.contains_point(&lattice.node(i)))
            .collect();
        Self::on_lattice(lattice, inside, seed, seed_radius, stepover)
    }

    fn on_lattice(
        lattice: Lattice,
        inside: Vec<bool>,
        seed: P2,
        seed_radius: f64,
        stepover: f64,
    ) -> Option<Self> {
        let n = lattice.rows * lattice.cols;
        let mut passes = vec![f64::INFINITY; n];
        let mut fixed = vec![false; n];
        let speed: Vec<f64> = inside
            .iter()
            .map(|&reach| if reach { stepover } else { f64::NAN })
            .collect();
        // The seed: every reachable node within one cell of the disc holds
        // its exact signed distance from the disc rim and is fixed.
        let mut seeded = false;
        for (i, ((t, f), &reach)) in passes
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
                *t = (d - seed_radius) / stepover;
                *f = true;
                seeded = true;
            }
        }
        if !seeded {
            return None;
        }
        let converged = fast_sweep(
            &mut passes,
            &inside,
            &fixed,
            &speed,
            lattice.rows,
            lattice.cols,
            lattice.cell,
        );
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
            stepover,
            converged,
        })
    }

    /// The geodesic distance (mm) at the lattice node nearest `p`, `None`
    /// at an unreachable or unsolved node.
    #[must_use]
    pub fn distance_mm(&self, p: P2) -> Option<f64> {
        let f = &self.field;
        let col = ((p.x - f.origin_x) / f.cell).round();
        let row = ((p.y - f.origin_y) / f.cell).round();
        if col < 0.0 || row < 0.0 {
            return None;
        }
        let (row, col) = (row as usize, col as usize);
        if row >= f.rows || col >= f.cols {
            return None;
        }
        let i = row * f.cols + col;
        let t = *f.passes.get(i)?;
        (self.inside.get(i).copied() == Some(true) && t.is_finite()).then_some(t * self.stepover)
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

    fn reachable(&self, row: usize, col: usize) -> bool {
        self.inside.get(row * self.field.cols + col).copied() == Some(true)
    }

    /// The level set `T = level` stepovers, over the cells whose four
    /// corners are reachable, with the fragments of each open arc joined.
    fn level_chains(&self, level: usize) -> Vec<Vec<P2>> {
        let kept = |row: usize, col: usize| {
            self.reachable(row, col)
                && self.reachable(row, col + 1)
                && self.reachable(row + 1, col)
                && self.reachable(row + 1, col + 1)
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
}

impl Chain {
    fn new(level: usize, pts: Vec<P2>, cell: f64) -> Self {
        let closed = match (pts.first(), pts.last()) {
            (Some(&a), Some(&b)) => (a.x - b.x).hypot(a.y - b.y) <= SQRT_2 * cell + CHAIN_EPS,
            _ => false,
        };
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

/// The wraps of `field` in cutting order from `start` (module doc, "The join
/// and the order"), each oriented for cutting.
fn ordered_wraps(
    field: &GeodesicField,
    start: P2,
    reach: f64,
    cancel: &dyn CancelCheck,
) -> Result<Vec<Vec<P2>>, Cancelled> {
    let cell = field.cell();
    let mut chains: Vec<Chain> = Vec::new();
    for level in 1..=field.level_count() {
        check_cancel(cancel)?;
        chains.extend(
            field
                .level_chains(level)
                .into_iter()
                .filter(|pts| pts.len() >= 2)
                .map(|pts| Chain::new(level, pts, cell)),
        );
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
    let mut done = vec![false; chains.len()];
    let mut out = Vec::with_capacity(chains.len());
    let mut cur = start;
    for _ in 0..chains.len() {
        let is_done = |j: &usize| done.get(*j).copied().unwrap_or(true);
        let next = chains
            .iter()
            .zip(&parents)
            .enumerate()
            .filter(|(i, (_, ps))| !is_done(i) && ps.iter().all(is_done))
            .map(|(i, (c, _))| (i, c.entry_distance(cur)))
            .min_by(|a, b| a.1.total_cmp(&b.1));
        // The lowest level left always qualifies (its parents are all cut),
        // so this ends only when every chain is cut.
        let Some((i, _)) = next else {
            break;
        };
        let (Some(flag), Some(chain)) = (done.get_mut(i), chains.get(i)) else {
            break;
        };
        *flag = true;
        let path = chain.oriented(cur);
        if let Some(&end) = path.last() {
            cur = end;
        }
        out.push(path);
    }
    Ok(out)
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

/// The planner cell: `max(R / 6, tolerance)` (`path.rs`).
fn planner_cell(params: &AdaptiveParams) -> f64 {
    (params.tool_radius / 6.0).max(params.tolerance)
}

/// What the producer of candidate (a) gives for one piece.
struct Produced {
    /// The starter pocket, then the wraps, before the cleanup.
    segments: Vec<AdaptiveSegment>,
    report: FieldReport,
    /// The wraps in cutting order, oriented.
    wraps: Vec<Vec<P2>>,
}

/// Candidate (a) on one piece outline: the starter pocket, then the wraps
/// of the geodesic field as `Cut`s and `Link`s, before the cleanup. `None`
/// when the field does not apply (see [`FieldReport::applied`]).
fn geodesic_segments(
    polygon: &Polygon2,
    params: &AdaptiveParams,
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
    let lattice = Lattice::of_grid(&setup.grid);
    let inside: Vec<bool> = setup
        .mask
        .iter()
        .enumerate()
        .map(|(i, &m)| m && region.contains(&lattice.node(i)))
        .collect();
    let Some(field) = GeodesicField::on_lattice(lattice, inside, seed, r, s) else {
        return Ok(None);
    };

    let reach = s + (1.0 + SQRT_2) * cell;
    let wraps = ordered_wraps(&field, starter_end, reach, cancel)?;
    let report = FieldReport {
        applied: true,
        seed: Some(seed),
        seed_radius: r,
        levels: field.level_count(),
        chains: wraps.len(),
        converged: field.converged(),
    };

    if let Some(first) = wraps.iter().find_map(|w| w.first()) {
        segments.push(AdaptiveSegment::Marker(AdaptiveRuntimeEvent::PassEntry {
            pass_index: 1,
            entry_x: first.x,
            entry_y: first.y,
        }));
    }
    let mut cur = starter_end;
    let mut run: Vec<P2> = vec![cur];
    for path in &wraps {
        let Some(&start) = path.first() else {
            continue;
        };
        let gap = (start.x - cur.x).hypot(start.y - cur.y);
        if gap <= reach {
            // The seam step, a cutting step as in `spiral.rs`.
            if gap > CHAIN_EPS {
                run.push(start);
            }
            run.extend(path.iter().skip(1).copied());
        } else {
            if run.len() >= 2 {
                segments.push(AdaptiveSegment::Cut(std::mem::take(&mut run)));
            }
            segments.push(AdaptiveSegment::Link(start));
            run = path.clone();
        }
        if let Some(&end) = run.last() {
            cur = end;
        }
    }
    if run.len() >= 2 {
        segments.push(AdaptiveSegment::Cut(run));
    }
    Ok(Some(Produced {
        segments,
        report,
        wraps,
    }))
}

// ── The pass-load readout ──────────────────────────────────────────────

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
#[derive(Debug, Clone, Copy, Default)]
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
}

impl PassLoadReadout {
    fn merge(&mut self, other: &Self) {
        self.ceiling = self.ceiling.max(other.ceiling);
        self.max_reading = self.max_reading.max(other.max_reading);
        self.steps += other.steps;
        self.steps_over += other.steps_over;
        self.exempt_steps += other.exempt_steps;
    }
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
    let step_len = cell * 3.0;
    let mut here: Option<P2> = None;
    let mut plunged_at: Option<P2> = None;
    let mut in_slot_lines = false;
    let read = |grid: &MaterialGrid, a: P2, b: P2, departing: bool, out: &mut PassLoadReadout| {
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
        }
    };
    for seg in segments {
        match seg {
            AdaptiveSegment::Marker(event) => {
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
                    let mut at = from;
                    for k in 1..=n {
                        let t = k as f64 / n as f64;
                        let next =
                            P2::new(from.x + t * (p.x - from.x), from.y + t * (p.y - from.y));
                        read(&setup.grid, at, next, false, &mut out);
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
                for w in pts.windows(2) {
                    let &[a, b] = w else {
                        continue;
                    };
                    if !in_slot_lines {
                        let departing = departure.is_some_and(|d| (a.x - d.x).hypot(a.y - d.y) < r);
                        read(&setup.grid, a, b, departing, &mut out);
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

// ── The run (harness door) ─────────────────────────────────────────────

/// Which 2D clearing a research run plans.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Candidate2d {
    /// A shipped strategy, through the product pipeline.
    Shipped(PathStrategy2d),
    /// Candidate (a), the geodesic wavefront.
    Geodesic,
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
        Candidate2d::Geodesic => PathStrategy2d::Agent,
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
    };
    for piece in &pieces {
        let raw = match candidate {
            Candidate2d::Shipped(_) => {
                super::adaptive_segments_with_debug(piece, &params, never, None, None).ok()
            }
            Candidate2d::Geodesic => match geodesic_segments(piece, &params, never) {
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
            },
        };
        let Some(raw) = raw else {
            continue;
        };
        let segments = cleaned(piece, &params, raw);
        let region = (params.keep_down_links == KeepDownLinks::WithinPassLoad)
            .then(|| ToolCentreRegion::new(piece, params.tool_radius));
        let (tp, _) = segments_to_toolpath(&segments, &params, region.as_ref());
        add_counts(&mut run.summary, &segments);
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
    /// seed disc. The first-order solve over-reads it by less than one cell
    /// within 20 mm of the seed (0.72 cell, prototype 2026-10-23) and never
    /// under-reads it.
    #[test]
    fn the_field_on_a_disc_is_the_euclidean_distance_within_one_cell() {
        let (s, tol) = op_defaults();
        let cell = (R / 6.0).max(tol);
        let disc_radius = 20.0;
        let region = Polygon2::new(circle(P2::new(0.0, 0.0), disc_radius, cell));
        for seed in [P2::new(0.0, 0.0), P2::new(7.3, -4.1)] {
            let field = GeodesicField::over_region(&region, cell, s, seed, R)
                .unwrap_or_else(|| panic!("the seed {seed:?} lies in the disc"));
            assert!(field.converged(), "a disc converges inside the cap");
            let lattice = Lattice::over(&region, cell);
            let mut read = 0usize;
            for i in 0..lattice.rows * lattice.cols {
                let p = lattice.node(i);
                if !region.contains_point(&p) {
                    continue;
                }
                let Some(t) = field.distance_mm(p) else {
                    panic!("every node of the disc is reached: {p:?}");
                };
                let euclid = (p.x - seed.x).hypot(p.y - seed.y) - R;
                assert!(
                    t >= euclid - 1e-9 && t - euclid <= cell,
                    "T {t:.4} vs Euclidean {euclid:.4} at {p:?} (cell {cell})"
                );
                read += 1;
            }
            // Every node at least one cell inside the rim is read: the
            // lattice holds about one node per cell² of the polygon shrunk
            // to the radius `disc_radius - cell`.
            let interior =
                region.area() * ((disc_radius - cell) / disc_radius).powi(2) / cell.powi(2);
            assert!(
                read as f64 >= interior,
                "the test read {read} nodes; it must read the disc (>= {interior:.0})"
            );
        }
    }

    /// Behind an island the front goes round: T at a point behind the island
    /// is more than its Euclidean distance by more than a cell, and not less
    /// than the exact geodesic (two tangents and the arc between) by more
    /// than a cell.
    #[test]
    fn the_field_goes_round_an_island() {
        let (s, tol) = op_defaults();
        let cell = (R / 6.0).max(tol);
        let island = 6.0;
        let mut hole = circle(P2::new(0.0, 0.0), island, cell);
        hole.reverse();
        let region = Polygon2::with_holes(
            vec![
                P2::new(-30.0, -20.0),
                P2::new(30.0, -20.0),
                P2::new(30.0, 20.0),
                P2::new(-30.0, 20.0),
            ],
            vec![hole],
        );
        let seed = P2::new(-22.0, 0.0);
        let behind = P2::new(12.0, 0.0);
        let field = GeodesicField::over_region(&region, cell, s, seed, R).expect("seeded");
        assert!(field.converged());
        let t = field
            .distance_mm(behind)
            .expect("the point behind is reached");
        let euclid = (behind.x - seed.x).hypot(behind.y - seed.y) - R;
        // Exact geodesic round a circle of radius `island` between two
        // points on its axis at distances `a` and `b`: two tangents and the
        // arc between their touch points.
        let (a, b) = (seed.x.abs(), behind.x.abs());
        let tangents = (a * a - island * island).sqrt() + (b * b - island * island).sqrt();
        let arc = island * (PI - (island / a).acos() - (island / b).acos());
        let exact = tangents + arc - R;
        assert!(
            t > euclid + cell,
            "T {t:.3} must exceed the Euclidean {euclid:.3} by more than a cell"
        );
        assert!(
            t >= exact - cell,
            "T {t:.3} must not under-read the exact geodesic {exact:.3}"
        );
    }

    /// Every step candidate (a) emits, after the shared cleanup, holds the
    /// pass load: the readout replays the plan and asks
    /// `step_within_pass_load` of each step.
    #[test]
    fn every_emitted_step_holds_the_pass_load() {
        let (s, tol) = op_defaults();
        let cell = (R / 6.0).max(tol);
        let mut hole = circle(P2::new(25.0, 20.0), 6.0, cell);
        hole.reverse();
        let outline = Polygon2::with_holes(
            vec![
                P2::new(0.0, 0.0),
                P2::new(60.0, 0.0),
                P2::new(60.0, 40.0),
                P2::new(0.0, 40.0),
            ],
            vec![hole],
        );
        let helix = crate::compute::config::HELIX_RADIUS_OVER_D * 2.0 * R;
        let params = AdaptiveParams {
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
        };
        let run = run_2d(&outline, &params, Candidate2d::Geodesic);
        assert!(
            run.fields.iter().any(|f| f.applied && f.chains > 0),
            "the field must apply and cut wraps: {:?}",
            run.fields
        );
        let load = run.summary.pass_load;
        assert!(load.steps > 100, "the readout must read the plan: {load:?}");
        assert_eq!(
            load.steps_over, 0,
            "every step holds the pass load: {load:?}"
        );
        assert!(load.max_reading <= load.ceiling, "{load:?}");
    }
}
