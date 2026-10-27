//! The wraps: the level sets of the field as chains inside the region, the
//! meeting-line trim, the order they are cut in
//! ([`ordered_wraps_beside_core`]), and the core spiral round the seed
//! ([`held_core_spiral`]) with its spacing law.

use std::f64::consts::SQRT_2;

use crate::geo::{P2, point_to_segment_distance};
use crate::geometry::marching_squares::CHAIN_EPS;
use crate::interrupt::{CancelCheck, Cancelled, check_cancel};
use crate::polygon::Polygon2;

use crate::adaptive::search::ToolCentreRegion;

use super::field::GeodesicField;

// ── The wrap order ─────────────────────────────────────────────────────

/// One wrap piece: a closed loop or an open arc of one level set.
pub(super) struct Chain {
    pub(super) level: usize,
    pub(super) pts: Vec<P2>,
    pub(super) closed: bool,
    pub(super) lo: P2,
    pub(super) hi: P2,
    /// Whether the first and the last point end at a meeting line; false
    /// for a loop.
    pub(super) shock: [bool; 2],
}

impl Chain {
    pub(super) fn new(level: usize, pts: Vec<P2>, closed: bool) -> Self {
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
    pub(super) fn segments(&self) -> impl Iterator<Item = (P2, P2)> + '_ {
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
    pub(super) fn within(&self, other: &Self, reach: f64) -> bool {
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
    pub(super) fn entry_distance(&self, p: P2) -> f64 {
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
    pub(super) fn oriented(&self, cur: P2) -> Vec<P2> {
        if self.closed {
            let seam = crate::adaptive::spiral::nearest_index(&self.pts, cur);
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
pub(super) fn is_closed(pts: &[P2], cell: f64) -> bool {
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
pub(super) fn inside_pieces(
    pts: Vec<P2>,
    closed: bool,
    region: &ToolCentreRegion,
) -> Vec<(Vec<P2>, bool)> {
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

/// One wrap in cutting order.
pub(super) struct Wrap {
    /// The level set, oriented for cutting (a loop closed on its seam).
    pub(super) pts: Vec<P2>,
    pub(super) closed: bool,
}

/// True when `p` (a chain end) lies next to a cell on a meeting line: the
/// 3 x 3 cells round the cell that holds `p`.
pub(super) fn end_at_shock(field: &GeodesicField, p: P2) -> bool {
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
pub(super) fn trim_end(pts: &mut Vec<P2>, mut len: f64) {
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

pub(super) fn polyline_length(pts: &[P2]) -> f64 {
    pts.windows(2)
        .filter_map(|w| match w {
            &[a, b] => Some((b.x - a.x).hypot(b.y - a.y)),
            _ => None,
        })
        .sum()
}

/// The wrap pieces of `field` from level `min_level` up, level by level,
/// every vertex and segment inside `region` ([`inside_pieces`]); each open
/// piece records whether its ends lie at a meeting line.
pub(super) fn collect_chains(
    field: &GeodesicField,
    region: &ToolCentreRegion,
    min_level: usize,
    cancel: &dyn CancelCheck,
) -> Result<Vec<Chain>, Cancelled> {
    let cell = field.cell();
    let mut chains: Vec<Chain> = Vec::new();
    for level in min_level.max(1)..=field.level_count() {
        check_cancel(cancel)?;
        for pts in field.level_chains(level) {
            if pts.len() < 2 {
                continue;
            }
            let closed = is_closed(&pts, cell);
            for (piece, closed) in inside_pieces(pts, closed, region) {
                let mut chain = Chain::new(level, piece, closed);
                if !closed {
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
pub(super) fn parents_of(
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

/// The meeting-line rule on `pts`, the cutting
/// order of `chain`: the branch cut second ends one stepover short of the
/// line. `cut_shock_ends` holds the meeting-line ends already cut, per
/// level; the ends of `chain` at a meeting line are added. Call it before
/// any other change to `pts` (it compares `pts` with `chain.pts` to know
/// the direction).
pub(super) fn meeting_trim(
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

/// §12.9: the core of a core spiral ([`held_core_spiral`]): its seed and
/// its last level `K`.
#[derive(Debug, Clone, Copy)]
pub(super) struct SpiralCore {
    pub(super) seed: P2,
    pub(super) levels: usize,
}

impl SpiralCore {
    /// True when `chain` is the core's own loop: a closed chain of level
    /// `1..=K` that encloses the seed. The spiral cuts that loop; every
    /// other piece of a level `≤ K` (a loop in a pocket the spiral does not
    /// reach, an open arc) is not the core's.
    pub(super) fn owns(&self, chain: &Chain) -> bool {
        chain.level <= self.levels
            && chain.closed
            && Polygon2::new(chain.pts.clone()).contains_point(&self.seed)
    }
}

/// §12.9: `chains` less the core's own loops ([`SpiralCore::owns`]).
pub(super) fn beside_core(mut chains: Vec<Chain>, core: SpiralCore) -> Vec<Chain> {
    chains.retain(|c| !core.owns(c));
    chains
}

/// The wraps of `field` in cutting order, from `start`. Each step takes a
/// chain whose parents ([`parents_of`]) are all cut: a child of the wrap
/// just cut when one is ready, so a lobe is cut to its end before the
/// cutter leaves it, else any ready chain; of those, the one whose entry is
/// nearest the cutter. Each wrap is oriented from the cutter and trimmed at
/// its meeting-line ends ([`meeting_trim`]).
///
/// Beside a core spiral (§12.9): with `core`, the chains start at level 1
/// and the core's own loops ([`SpiralCore::owns`]) are dropped: the spiral
/// cuts them. A star-shaped core
/// ([`GeodesicField::star_levels`]) can still hold a pocket the plate cuts
/// off along the rays: its loops of a level `≤ K` are not crossed by any
/// ray before the ray leaves the reachable cells, so the spiral never
/// sweeps them, and they stay wraps. Without `core`, the chains start at
/// `min_level`, as before.
#[expect(
    clippy::too_many_arguments,
    reason = "each parameter is a separate input of one planning call"
)]
pub(super) fn ordered_wraps_beside_core(
    field: &GeodesicField,
    region: &ToolCentreRegion,
    start: P2,
    reach: f64,
    stepover: f64,
    min_level: usize,
    core: Option<SpiralCore>,
    cancel: &dyn CancelCheck,
) -> Result<Vec<Wrap>, Cancelled> {
    let mut chains = collect_chains(
        field,
        region,
        if core.is_some() { 1 } else { min_level },
        cancel,
    )?;
    if let Some(core) = core {
        chains = beside_core(chains, core);
    }
    let parents = parents_of(&chains, reach, cancel)?;
    let mut done = vec![false; chains.len()];
    // The shock ends already cut, per level.
    let mut cut_shock_ends: Vec<(usize, P2)> = Vec::new();
    let mut out: Vec<Wrap> = Vec::with_capacity(chains.len());
    let mut cur = start;
    let mut last: Option<usize> = None;
    for _ in 0..chains.len() {
        let is_done = |j: &usize| done.get(*j).copied().unwrap_or(true);
        let ready =
            |i: usize| !is_done(&i) && parents.get(i).is_some_and(|ps| ps.iter().all(is_done));
        // A child of the wrap just cut goes next, so a lobe is cut to its
        // end before the cutter leaves it (the arc links).
        let child_of_last =
            |i: usize| last.is_some_and(|l| parents.get(i).is_some_and(|ps| ps.contains(&l)));
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
        let (Some(flag), Some(chain)) = (done.get_mut(i), chains.get(i)) else {
            break;
        };
        *flag = true;
        let mut pts = chain.oriented(cur);
        meeting_trim(chain, &mut pts, &mut cut_shock_ends, reach, stepover);
        if let Some(&end) = pts.last() {
            cur = end;
        }
        out.push(Wrap {
            pts,
            closed: chain.closed,
        });
        last = Some(i);
    }
    Ok(out)
}

// ── The spacing law and the core spiral ────────────────────────────────

/// `s'(ρ) = ρ + R - √(R² + ρ² + 2ρ(R - c·D))`: the spacing that puts a pass
/// at cutter-centre radius `rho` round a cleared disc on the ceiling width
/// `ceiling_width` (`c·D`); the derivation is in the module doc of
/// `adaptive::geodesic` ("The spacing law").
pub(super) fn spacing_bound(tool_radius: f64, ceiling_width: f64, rho: f64) -> f64 {
    let r = tool_radius;
    rho + r - (r * r + rho * rho + 2.0 * rho * (r - ceiling_width)).sqrt()
}

/// The law's spacing at `rho`: `s'(ρ)` capped at the stepover (C5).
pub(super) fn law_spacing(tool_radius: f64, stepover: f64, ceiling_width: f64, rho: f64) -> f64 {
    spacing_bound(tool_radius, ceiling_width, rho)
        .min(stepover)
        .max(0.0)
}

/// The chord rule of the core spiral: the vertices per turn that keep each
/// chord within `tolerance` of a circle of radius `outer`
/// (`⌈π / acos(1 - tolerance / ρ)⌉`).
pub(super) fn chord_turn_steps(outer: f64, tolerance: f64) -> usize {
    let sag = (1.0 - (tolerance / outer).min(1.0))
        .acos()
        .max(f64::EPSILON);
    (std::f64::consts::PI / sag).ceil().max(3.0) as usize
}

/// The core spiral through the turn values `levels` of `T`
/// (stepovers): turn `n` runs from `levels[n]` to `levels[n + 1]`, linear
/// in the angle, and one closing lap follows on the last value. Each turn
/// has `per_turn` vertices, on the same rays as the turn before it, and
/// each vertex radius is bisected to `radius_tolerance`. `None` when a ray
/// does not reach its value or a chord leaves the region.
#[expect(
    clippy::too_many_arguments,
    reason = "each parameter is a separate input of one planning call"
)]
pub(super) fn trace_core_spiral(
    field: &GeodesicField,
    region: &ToolCentreRegion,
    seed: P2,
    r0: f64,
    phi0: f64,
    levels: &[f64],
    per_turn: usize,
    radius_tolerance: f64,
) -> Option<Vec<P2>> {
    let (&last, k) = (levels.last()?, levels.len() - 1);
    let dphi = std::f64::consts::TAU / per_turn as f64;
    let mut pts: Vec<P2> = Vec::with_capacity(per_turn * (k + 1) + 1);
    for j in 0..=per_turn * (k + 1) {
        let theta = j as f64 * dphi;
        // The unwrapped turn count, and `T` there: turn value `n` at turn
        // `n`, linear in the angle between two values.
        let turns = theta / std::f64::consts::TAU;
        let t = if turns >= k as f64 {
            last
        } else {
            let n = turns.floor();
            let (lo, hi) = (
                levels.get(n as usize).copied()?,
                levels.get(n as usize + 1).copied()?,
            );
            lo + (turns - n) * (hi - lo)
        };
        let phi = phi0 + theta;
        let r = field.radius_at(seed, phi, r0, t, radius_tolerance)?;
        pts.push(P2::new(seed.x + r * phi.cos(), seed.y + r * phi.sin()));
    }
    pts.windows(2)
        .all(|w| match w {
            &[a, b] => region.contains_segment(a, b),
            _ => true,
        })
        .then_some(pts)
}

/// The largest heading tilt (rad) of a step into a band of depth `s`
/// that reads at most `margin` (mm) over the band's own width: the tilted
/// step sweeps `R(1 - cos β) + h sin β` more, `h = √(R² - (R - s)²)`
/// (the research entry law), so
/// `β = atan2(R, h) + asin((margin - R) / √(h² + R²))`. 0 when no tilt holds.
pub(super) fn held_tilt(tool_radius: f64, stepover: f64, margin: f64) -> f64 {
    let r = tool_radius;
    let h = (r * r - (r - stepover).max(0.0).powi(2)).max(0.0).sqrt();
    let rho_h = h.hypot(r);
    (r.atan2(h) + ((margin - r) / rho_h).clamp(-1.0, 1.0).asin()).max(0.0)
}

/// C35, D21b: the core spiral, spaced under the law by what its chords
/// add. Returns the spiral and `K`, the last core level. `r0` is the seed disc's radius, which is the tool
/// radius `R` (the field's rim).
///
/// A spiral spaced on the field levels alone (research C4) holds the
/// law with no margin inside `ρ*` (D21: the replay refused 9 chords at
/// 0.3631 to 0.3931 against 0.3626). The level holds on the curve; a chord
/// does not follow it. Each chord heads `β = π / n` off the tangent at its
/// end (`n` vertices per turn), out into the stock on its second half, and
/// the previous turn's chord ahead of it lies `ρ(1 - cos β)` inside the
/// level. The swept width reads across the heading, so the cap of stock
/// ahead of the cutter reads `R(1 - cos β) + h sin β` wider (the entry law;
/// on exact circles at s 2, R 3 and 22 vertices per turn, 0.44 against
/// 0.3626; the excess falls linearly with `β`).
///
/// - The angle step holds the tilt to the op `tolerance`
///   ([`held_tilt`]), or the chord rule ([`chord_turn_steps`]) when that
///   is finer.
/// - Each vertex radius is bisected to the float resolution: a radius
///   error between two vertices tilts the chord too.
/// - The turns are spaced by the local-level walk
///   ([`GeodesicField::spiral_turn_levels`]) at the ceiling width less the
///   tilt term at the step used, the chord sag `ρ_K(1 - cos β)` at the
///   outer radius, and one `tolerance`: the law is read on the traced
///   contours and the spiral on the bilinear field, and the plan holds
///   positions to the op tolerance. The last turn ends on level `K`, so
///   the wraps past it keep their spacing.
#[expect(
    clippy::too_many_arguments,
    reason = "each parameter is a separate input of one planning call"
)]
pub(super) fn held_core_spiral(
    field: &GeodesicField,
    region: &ToolCentreRegion,
    seed: P2,
    r0: f64,
    phi0: f64,
    stepover: f64,
    ceiling_width: f64,
    tolerance: f64,
    cancel: &dyn CancelCheck,
) -> Result<(Vec<P2>, usize), Cancelled> {
    let h = (r0 * r0 - (r0 - stepover).max(0.0).powi(2)).max(0.0).sqrt();
    let beta = held_tilt(r0, stepover, tolerance);
    if beta <= 0.0 {
        return Ok((Vec::new(), 0));
    }
    let mut k = field.star_levels(seed, r0);
    while k > 0 {
        check_cancel(cancel)?;
        let outer = r0 + field.level_value(k) * stepover;
        let per_turn =
            chord_turn_steps(outer, tolerance).max((std::f64::consts::PI / beta).ceil() as usize);
        let step = std::f64::consts::PI / per_turn as f64;
        let held_width = ceiling_width
            - r0 * (1.0 - step.cos())
            - h * step.sin()
            - outer * (1.0 - step.cos())
            - tolerance;
        let levels = field.spiral_turn_levels(k, r0, held_width, tolerance, cancel)?;
        if let Some(pts) = levels.and_then(|levels| {
            trace_core_spiral(field, region, seed, r0, phi0, &levels, per_turn, 0.0)
        }) {
            return Ok((pts, k));
        }
        k -= 1;
    }
    Ok((Vec::new(), 0))
}
