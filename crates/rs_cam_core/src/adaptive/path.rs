//! Main adaptive clearing path generation: orchestrator + utilities.
//!
//! Consumes MaterialGrid, compute_engagement, search_direction_with_metrics,
//! and find_entry_point from the sibling submodules and produces a sequence
//! of `AdaptiveSegment` items that `segments_to_toolpath` converts into a
//! final Toolpath with rapids, plunges, feeds, and runtime annotations.

use super::material_grid::polygon_bbox;
use super::search::{
    NoStep, PassLoad, ToolCentreRegion, find_entry_point, find_entry_via_distance_transform,
    path_bounds, search_direction_gradient, search_direction_with_metrics, step_within_pass_load,
};
use super::{
    AdaptiveParams, AdaptiveRuntimeAnnotation, AdaptiveRuntimeEvent, CleanupStrategy,
    KeepDownLinks, MaterialGrid, average_angles, blend_corners_to_moves,
};
use crate::geo::P2;
use crate::interrupt::{CancelCheck, Cancelled, check_cancel};
use crate::ops::adaptive_shared::BlendedMove;
use crate::polygon::{Polygon2, offset_polygon};
use crate::toolpath::Toolpath;
use crate::trace::debug_trace::{HotspotRecord, ToolpathDebugBounds2, ToolpathDebugContext};

use std::time::Instant;

// ── Link vs retract ────────────────────────────────────────────────────

/// Check if the straight line from `from` to `to` is safe to traverse at
/// cut depth. The entire path must be within the machinable region, and
/// at most 20% of the path may cross uncut material (thin strips are OK —
/// the tool handles light engagement during a link move).
pub(super) fn is_clear_path(
    grid: &MaterialGrid,
    mask: &[bool],
    from: P2,
    to: P2,
    _tool_radius: f64,
) -> bool {
    let dx = to.x - from.x;
    let dy = to.y - from.y;
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1e-10 {
        return true;
    }

    let n_steps = (len / (grid.cell_size * 2.0)).ceil() as usize;
    let mut material_hits = 0;
    let mut total = 0;

    for i in 0..=n_steps {
        let t = i as f64 / n_steps.max(1) as f64;
        let x = from.x + t * dx;
        let y = from.y + t * dy;
        total += 1;

        // Hard fail: outside machinable region
        if !grid.is_machinable(mask, x, y) {
            return false;
        }
        if grid.is_material(x, y) {
            material_hits += 1;
        }
    }

    // Safe if less than 20% of the path crosses material
    total > 0 && (material_hits as f64 / total as f64) <= 0.2
}

/// How one planning call holds its steps and keep-down links to the pass
/// load (G-ADAPTLINKLOAD, G-ADAPTPASSLOAD).
#[derive(Clone)]
pub(crate) struct LinkLoad {
    rule: KeepDownLinks,
    tool_radius: f64,
    /// The pass step length, `cell_size × 3`.
    step_len: f64,
    /// The measure, band and ceiling every step is held to.
    pass: PassLoad,
    /// The radius of the hole a re-entry (`Rapid`) cuts: `R` plus the
    /// entry dressup's helix radius (G-ADAPTPASSLOAD).
    entry_radius: f64,
    /// Where a re-entry may plunge when the entry is a helix: the lattice
    /// points at least `R + helix radius` inside the part (the machinable
    /// region inset by the helix radius), so the helix circle does not cut
    /// a wall. `None` when a re-entry is a straight plunge (any machinable
    /// point) or under the historical rule.
    entry_mask: Option<Vec<bool>>,
    /// Replay walks each kept `Cut` again under the pass load (every 2D
    /// strategy, `ContourSpiral` included).
    rewalk_replayed_cuts: bool,
    /// The machinable region under the 2D rule: a candidate cutter centre,
    /// and the straight move to it, is read against it exactly, not on the
    /// lattice. Its pieces are also where a helix circle must stay (the
    /// region the entry dressup's containment reads).
    region: Option<ToolCentreRegion>,
}

impl LinkLoad {
    pub(crate) fn new(params: &AdaptiveParams, grid: &MaterialGrid, polygon: &Polygon2) -> Self {
        let holds = params.keep_down_links == KeepDownLinks::WithinPassLoad;
        let helix = params.entry_helix_radius.max(0.0);
        let region = holds.then(|| ToolCentreRegion::new(polygon, params.tool_radius));
        // Where the full helix fits inside the part: where an agent pass
        // prefers to enter (its departure needs the full hole). Every other
        // legal point is an entry with a helix shrunk to fit (round 3).
        let entry_region: Vec<Polygon2> = if holds && helix > 0.0 {
            offset_polygon(polygon, params.tool_radius + helix)
        } else {
            Vec::new()
        };
        let entry_mask = (!entry_region.is_empty()).then(|| {
            let mut mask = vec![false; grid.rows * grid.cols];
            for poly in &entry_region {
                let m = MaterialGrid::build_machinable_mask(
                    poly,
                    grid.origin_x,
                    grid.origin_y,
                    grid.rows,
                    grid.cols,
                    grid.cell_size,
                );
                for (a, b) in mask.iter_mut().zip(m) {
                    *a |= b;
                }
            }
            mask
        });
        Self {
            rule: params.keep_down_links,
            tool_radius: params.tool_radius,
            step_len: grid.cell_size * 3.0,
            pass: pass_load_for(params, grid.cell_size),
            entry_radius: params.tool_radius + helix,
            entry_mask,
            rewalk_replayed_cuts: holds,
            region,
        }
    }

    /// True when the cutter centre may stand at `p`: exactly inside the
    /// machinable region under the 2D rule, else the lattice read.
    fn legal(&self, grid: &MaterialGrid, mask: &[bool], p: P2) -> bool {
        match &self.region {
            Some(region) => region.contains(&p),
            None => grid.is_machinable(mask, p.x, p.y),
        }
    }

    /// True when the cutter may feed straight from `from` to `to`: the
    /// whole segment inside the region under the 2D rule (a chord between
    /// two legal ends can cut an island), else the lattice read of `to`.
    fn legal_step(&self, grid: &MaterialGrid, mask: &[bool], from: P2, to: P2) -> bool {
        match &self.region {
            Some(region) => region.contains_segment(from, to),
            None => grid.is_machinable(mask, to.x, to.y),
        }
    }

    /// Stamp the hole a re-entry at `p` cuts. The entry dressup helixes only
    /// where stock stands within R of the entry (`rapid_to_entry_top`: a
    /// column that reads air is fed straight down), so the helix hole
    /// (R + helix radius) is stamped only where the disc holds stock; else
    /// the plunge's own disc.
    fn stamp_entry(&self, grid: &mut MaterialGrid, p: P2) {
        let helix = super::search::compute_engagement(grid, p.x, p.y, self.tool_radius) > 0.0;
        let r = if helix {
            self.entry_hole(p)
        } else {
            self.tool_radius
        };
        grid.clear_circle(p.x, p.y, r);
    }

    /// The radius of the hole a helix entry at `p` cuts: R plus the helix
    /// radius the dressup emits there, the default shrunk to the largest
    /// whose circle stays inside the machinable region
    /// (`dressup::contained_helix_radius`, the same function and region the
    /// dressup reads). Where no helix fits the dressup ramps along the cut
    /// it enters, which the planner stamps as that cut: the hole is R.
    fn entry_hole(&self, p: P2) -> f64 {
        if self.entry_radius <= self.tool_radius {
            return self.tool_radius;
        }
        let fit = crate::dressup::contained_helix_radius(
            self.region.as_ref().map_or(&[], |r| r.pieces()),
            p.x,
            p.y,
            self.entry_radius - self.tool_radius,
        );
        if fit > crate::dressup::HELIX_MIN_FIT_MM {
            self.tool_radius + fit
        } else {
            self.tool_radius
        }
    }

    /// True when a re-entry is a straight plunge (no helix hole): under the
    /// swept-width model no step out of a hole the cutter's own size holds
    /// the pass load (it reads at least half the diameter), so the first
    /// steps out of such an entry are exempt ([`PassLoad::departing`]).
    fn plunge_departure_exempt(&self) -> bool {
        self.holds_pass_load() && self.entry_radius <= self.tool_radius
    }

    /// True when an entry at `p` is one the planner models exactly: a
    /// straight plunge (no helix configured), or a helix with room to fit
    /// (`entry_hole` > R). A contained helix with no room becomes a ramp
    /// folded along the cut it enters, which the planner does not model,
    /// so the planner does not enter there (G-ADAPTPASSLOAD round 3).
    fn entry_modelled(&self, p: P2) -> bool {
        self.entry_radius <= self.tool_radius || self.entry_hole(p) > self.tool_radius
    }

    /// The legal lattice point nearest `(x, y)` within R whose entry the
    /// planner models ([`Self::entry_modelled`]).
    #[allow(clippy::indexing_slicing)] // bounded lattice scan
    fn helix_entry_near(&self, grid: &MaterialGrid, mask: &[bool], x: f64, y: f64) -> Option<P2> {
        let r = self.tool_radius;
        let cell = grid.cell_size;
        let n = (r / cell).ceil() as i64;
        let mut best: Option<(f64, P2)> = None;
        for j in -n..=n {
            for i in -n..=n {
                let p = P2::new(x + i as f64 * cell, y + j as f64 * cell);
                let d = (p.x - x).hypot(p.y - y);
                if d > r || best.is_some_and(|(b, _)| d >= b) {
                    continue;
                }
                if self.legal(grid, mask, p) && self.entry_modelled(p) {
                    best = Some((d, p));
                }
            }
        }
        best.map(|(_, p)| p)
    }

    /// True when a re-entry (retract, rapid, entry) may plunge at `p`: its
    /// helix, if any, stays inside the part.
    fn entry_ok(&self, grid: &MaterialGrid, machinable_mask: &[bool], p: P2) -> bool {
        // Every legal point with room for a (shrunk) helix is an entry.
        self.legal(grid, machinable_mask, p) && self.entry_modelled(p)
    }

    /// True for the 2D rule: every cutting step and keep-down link holds
    /// the pass load (`step_within_pass_load`).
    fn holds_pass_load(&self) -> bool {
        self.rule == KeepDownLinks::WithinPassLoad
    }
}

/// The load a planning call holds its steps to. 2D Adaptive
/// ([`KeepDownLinks::WithinPassLoad`]) measures swept width against the
/// commanded radial fraction whatever `engagement_measure` says
/// (G-ADAPTPASSLOAD, operator decision 2026-09-26); the Adaptive3d slices
/// keep their saved measure and the historical search.
pub(crate) fn pass_load_for(params: &AdaptiveParams, cell_size: f64) -> PassLoad {
    match params.keep_down_links {
        KeepDownLinks::WithinPassLoad => {
            PassLoad::swept_width(params.stepover, params.tool_radius, cell_size)
        }
        KeepDownLinks::RetractedByCaller => PassLoad::historical(
            params.stepover,
            params.tool_radius,
            params.engagement_measure,
        ),
    }
}

/// Operator ruling 2026-09-26: a keep-down link feeds through material only
/// as a legitimate cutting move with a defined load. Walk the straight link
/// from `from` to `to` in steps no longer than a pass step, holding each
/// step to [`step_within_pass_load`] on the grid as cut up to the previous
/// step, and clearing the cutter disc as a pass does. Admit the link when
/// the path stays machinable (the `is_clear_path` sampling) and every step
/// holds the load; the grid then keeps the stamp, so planner stock is the
/// emitted geometry. Otherwise restore the grid and return false: the
/// caller retracts and re-enters. A link through cleared cells reads 0.
fn feed_link_within_pass_load(
    grid: &mut MaterialGrid,
    mask: &[bool],
    from: P2,
    to: P2,
    load: &LinkLoad,
) -> bool {
    feed_link_checked(grid, mask, from, to, load, false)
}

/// [`feed_link_within_pass_load`] to a point the caller knows the cutter
/// may stand on (a region-boundary contour point, a planner pass point): the
/// line is not read within two cells of it, where the lattice read of a
/// point on the region boundary falls outside the region.
fn feed_link_to_legal(
    grid: &mut MaterialGrid,
    mask: &[bool],
    from: P2,
    to: P2,
    load: &LinkLoad,
) -> bool {
    feed_link_checked(grid, mask, from, to, load, true)
}

fn feed_link_checked(
    grid: &mut MaterialGrid,
    mask: &[bool],
    from: P2,
    to: P2,
    load: &LinkLoad,
    to_is_legal: bool,
) -> bool {
    let dx = to.x - from.x;
    let dy = to.y - from.y;
    let len = dx.hypot(dy);
    if len < 1e-10 {
        return true;
    }
    // Under the 2D rule the straight link must not cross the region's
    // boundary anywhere (a chord between two legal samples can cut an
    // island's inset arc).
    if load.region.as_ref().is_some_and(|r| r.crossed_by(from, to)) {
        return false;
    }
    // The line between the ends is read at all four lattice points around
    // each sample (a wall-hugging link must not cut the wall); the ends
    // themselves are where the cutter stands before and after, read as a
    // pass reads its own positions.
    let n_mask = (len / (grid.cell_size * 2.0)).ceil() as usize;
    for i in 0..=n_mask {
        let t = i as f64 / n_mask.max(1) as f64;
        let (x, y) = (from.x + t * dx, from.y + t * dy);
        if to_is_legal && (to.x - x).hypot(to.y - y) < 2.0 * grid.cell_size {
            continue;
        }
        let inside = if i == 0 || i == n_mask {
            grid.is_machinable(mask, x, y)
        } else {
            load.legal(grid, mask, P2::new(x, y))
        };
        if !inside {
            return false;
        }
    }
    let angle = dy.atan2(dx);
    let n_steps = ((len / load.step_len).ceil() as usize).max(1);
    let mut log = Vec::new();
    grid.clear_circle_logged(from.x, from.y, load.tool_radius, &mut log);
    let mut at = from;
    for k in 1..=n_steps {
        let t = k as f64 / n_steps as f64;
        let (x, y) = (from.x + t * dx, from.y + t * dy);
        if !step_within_pass_load(grid, x, y, load.tool_radius, angle, &load.pass) {
            grid.restore_cleared(&log);
            return false;
        }
        let next = P2::new(x, y);
        grid.clear_segment_logged(at, next, load.tool_radius, &mut log);
        at = next;
    }
    true
}

/// `path` with every segment longer than `max_step` split evenly, so each
/// consecutive pair is at most one pass step apart.
fn subdivide(path: &[P2], max_step: f64) -> Vec<P2> {
    let mut out = Vec::with_capacity(path.len());
    let Some(&first) = path.first() else {
        return out;
    };
    out.push(first);
    for w in path.windows(2) {
        let &[a, b] = w else {
            continue;
        };
        let len = (b.x - a.x).hypot(b.y - a.y);
        let n = ((len / max_step).ceil() as usize).max(1);
        for k in 1..=n {
            let t = k as f64 / n as f64;
            out.push(P2::new(a.x + t * (b.x - a.x), a.y + t * (b.y - a.y)));
        }
    }
    out
}

/// G-ADAPTPASSLOAD: emit the cutter-centre polyline `path` as cutting spans
/// that each hold the pass load, splitting it where a step would not.
///
/// The path is walked in steps of at most a pass step. While the cutter is
/// down, a step that holds [`step_within_pass_load`] on the grid as cut so
/// far is stamped and joins the span; a step that does not ends the span
/// and the cutter is lifted. While lifted, a step `a -> b` that holds the
/// load WITHOUT the disc at `a` cleared (a conservative reading: clearing
/// more only shrinks the extent) re-starts a span at `a`, reached from
/// `here` by a keep-down link that holds the load or else by a retract and
/// re-entry. If the cutter already stands at the first point it starts
/// there. Only emitted motion is stamped, so planner stock is the emitted
/// geometry. Returns the number of `Cut` spans emitted; `here` follows the
/// cutter.
fn emit_capped_walk(
    path: &[P2],
    grid: &mut MaterialGrid,
    mask: &[bool],
    load: &LinkLoad,
    here: &mut Option<P2>,
    out: &mut Vec<AdaptiveSegment>,
) -> usize {
    emit_capped_walk_with(path, grid, mask, load, here, out, false, None)
}

/// Air steps in a row after which a `cut_only` walk ends its span.
const CUT_ONLY_MAX_AIR_STEPS: usize = 4;

/// [`emit_capped_walk`]; with `cut_only` a span starts only on a step that
/// cuts stock and ends after [`CUT_ONLY_MAX_AIR_STEPS`] steps in air, cut
/// back to its last step in stock (the final wall pass walks the whole
/// region boundary for the slivers left along it).
// SAFETY: not memory safety; the walker's state (grid, cursor, output) and
// its two mode flags are one call, and a struct for them would be built
// once per call site.
#[allow(clippy::too_many_arguments)]
fn emit_capped_walk_with(
    path: &[P2],
    grid: &mut MaterialGrid,
    mask: &[bool],
    load: &LinkLoad,
    here: &mut Option<P2>,
    out: &mut Vec<AdaptiveSegment>,
    cut_only: bool,
    // The straight-plunge point the walk departs from, when the planner
    // exempted the steps inside its hole (`PassLoad::departing`); replay
    // holds those steps to the same rule generation did.
    departure: Option<P2>,
) -> usize {
    let r = load.tool_radius;
    let pts = subdivide(path, load.step_len);
    let mut spans = 0usize;
    let mut span: Vec<P2> = Vec::new();
    // The span length up to its last step in stock, and the air run since.
    let mut kept_len = 0usize;
    let mut air_run = 0usize;
    let mut was_departing = false;
    let flush = |span: &mut Vec<P2>, out: &mut Vec<AdaptiveSegment>, spans: &mut usize| {
        if span.len() >= 2 {
            out.push(AdaptiveSegment::Cut(std::mem::take(span)));
            *spans += 1;
        } else {
            span.clear();
        }
    };
    if let (Some(h), Some(&p0)) = (*here, pts.first())
        && (p0.x - h.x).hypot(p0.y - h.y) < 1e-6
    {
        grid.clear_circle(p0.x, p0.y, r);
        span.push(p0);
        kept_len = 1;
    }
    for w in pts.windows(2) {
        let &[a, b] = w else {
            continue;
        };
        let angle = (b.y - a.y).atan2(b.x - a.x);
        let departing = departure.is_some_and(|d| (a.x - d.x).hypot(a.y - d.y) < r);
        let step_pass = if departing {
            load.pass.departing()
        } else {
            load.pass
        };
        let reading = super::search::measure_step(grid, b.x, b.y, r, angle, &step_pass);
        if was_departing && !departing && span.len() >= 2 {
            // The exempt departure stays its own cut (as generation made it).
            let last = span.last().copied();
            out.push(AdaptiveSegment::Cut(std::mem::take(&mut span)));
            spans += 1;
            span.extend(last);
            kept_len = span.len();
        }
        was_departing = departing;
        let holds = reading <= step_pass.ceiling;
        let cuts = reading >= load.pass.presence;
        if !span.is_empty() {
            if holds {
                grid.clear_segment(a, b, r);
                span.push(b);
                if cuts {
                    kept_len = span.len();
                    air_run = 0;
                } else {
                    air_run += 1;
                }
                if cut_only && air_run > CUT_ONLY_MAX_AIR_STEPS {
                    // The air steps stamped nothing: drop them.
                    span.truncate(kept_len);
                    *here = span.last().copied();
                    flush(&mut span, out, &mut spans);
                    air_run = 0;
                }
            } else {
                if cut_only {
                    span.truncate(kept_len);
                }
                *here = span.last().copied();
                flush(&mut span, out, &mut spans);
                air_run = 0;
            }
            continue;
        }
        if !holds || (cut_only && !cuts) {
            continue;
        }
        let reenters = match *here {
            None => true,
            Some(h) if (a.x - h.x).hypot(a.y - h.y) < 1e-6 => false,
            Some(h) => !feed_link_to_legal(grid, mask, h, a, load),
        };
        // A walk point is a legal cutter position: with a straight plunge
        // any of them is an entry; a helix must also fit inside the part.
        if reenters && load.entry_mask.is_some() && !load.entry_ok(grid, mask, a) {
            // No link holds the load and a helix here would cut a wall:
            // stay lifted and look further along.
            continue;
        }
        if reenters {
            out.push(AdaptiveSegment::Rapid(a));
            load.stamp_entry(grid, a);
        } else if here.is_some_and(|h| (a.x - h.x).hypot(a.y - h.y) >= 1e-6) {
            out.push(AdaptiveSegment::Link(a));
        }
        grid.clear_circle(a.x, a.y, r);
        grid.clear_circle(b.x, b.y, r);
        *here = Some(a);
        span.push(a);
        span.push(b);
        kept_len = span.len();
        air_run = 0;
    }
    if cut_only {
        span.truncate(kept_len);
    }
    if let Some(&end) = span.last() {
        *here = Some(end);
    }
    flush(&mut span, out, &mut spans);
    spans
}

/// Replay kept segments on `grid` in emitted order, the way the cleanup
/// strategies rebuild the planner stock after the short-cut filter. Under
/// [`KeepDownLinks::WithinPassLoad`] every `Link` is decided again on the
/// grid of that moment (the filter can join two runs with a link the main
/// loop never checked) and an admitted link is stamped; a refused one
/// becomes a `Rapid`. Every `Cut` is walked again under the pass load
/// ([`emit_capped_walk`]): a cut the filter left standing on material a
/// dropped cut had cleared is split where it would overload. Slot-clearing
/// lines are the one declared exception (full-width slots the operator
/// opted into) and are replayed as they are. Returns the replayed segments
/// and the last cutter position of the last `Cut`.
fn replay_kept_segments(
    segments: Vec<AdaptiveSegment>,
    grid: &mut MaterialGrid,
    mask: &[bool],
    load: &LinkLoad,
) -> (Vec<AdaptiveSegment>, Option<P2>) {
    let mut out = Vec::with_capacity(segments.len());
    let mut last_cut: Option<P2> = None;
    let mut here: Option<P2> = None;
    let mut in_slot_lines = false;
    let mut plunged_at: Option<P2> = None;
    for seg in segments {
        match seg {
            AdaptiveSegment::Cut(ref path) if load.rewalk_replayed_cuts && !in_slot_lines => {
                let departure = plunged_at.take();
                if emit_capped_walk_with(
                    path, grid, mask, load, &mut here, &mut out, false, departure,
                ) > 0
                {
                    last_cut = here;
                }
            }
            AdaptiveSegment::Cut(ref path) => {
                for p in path {
                    grid.clear_circle(p.x, p.y, load.tool_radius);
                    last_cut = Some(*p);
                    here = Some(*p);
                }
                out.push(seg);
            }
            AdaptiveSegment::Link(p) if load.holds_pass_load() => {
                let feeds = here.is_some_and(|from| feed_link_to_legal(grid, mask, from, p, load));
                if feeds {
                    out.push(AdaptiveSegment::Link(p));
                    plunged_at = None;
                } else {
                    // The refused link becomes a re-entry: its hole is cut.
                    out.push(AdaptiveSegment::Rapid(p));
                    load.stamp_entry(grid, p);
                    plunged_at = load.plunge_departure_exempt().then_some(p);
                }
                here = Some(p);
            }
            AdaptiveSegment::Rapid(p) if load.holds_pass_load() => {
                // A rapid re-enters at `p`: the entry cuts its hole there.
                load.stamp_entry(grid, p);
                plunged_at = load.plunge_departure_exempt().then_some(p);
                here = Some(p);
                out.push(seg);
            }
            AdaptiveSegment::Link(p) | AdaptiveSegment::Rapid(p) => {
                here = Some(p);
                out.push(seg);
            }
            AdaptiveSegment::Marker(ref event) => {
                in_slot_lines = matches!(event, AdaptiveRuntimeEvent::SlotClearing { .. });
                out.push(seg);
            }
        }
    }
    (out, last_cut)
}

// ── Main adaptive path generation ──────────────────────────────────────

/// 2D agent passes in a row that may remove nothing before the pass loop
/// stops (G-ADAPTPASSLOAD, which removed the forced clear that used to end
/// such a stall).
const MAX_STALLED_PASSES: usize = 3;

/// A segment of the adaptive path: cutting, rapid reposition, or link (tool-down reposition).
#[derive(Clone)]
pub(crate) enum AdaptiveSegment {
    /// Cutting moves: a sequence of 2D points.
    Cut(Vec<P2>),
    /// Rapid reposition to a new entry point (retract → rapid → plunge).
    Rapid(P2),
    /// Link move: reposition at cut depth without retracting (cleared path).
    Link(P2),
    /// Structured runtime marker at the current point in the toolpath.
    Marker(AdaptiveRuntimeEvent),
}

/// Generate the 2D adaptive clearing path segments.
#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn adaptive_segments(
    polygon: &Polygon2,
    tool_radius: f64,
    stepover: f64,
    tolerance: f64,
    slot_clearing: bool,
    cancel: &dyn CancelCheck,
) -> Result<Vec<AdaptiveSegment>, Cancelled> {
    let params = AdaptiveParams {
        tool_radius,
        stepover,
        tolerance,
        slot_clearing,
        cut_depth: 0.0,
        feed_rate: 0.0,
        plunge_rate: 0.0,
        safe_z: 0.0,
        min_cutting_radius: 0.0,
        initial_stock: None,
        cleanup_strategy: crate::adaptive::CleanupStrategy::Legacy,
        engagement_measure: crate::adaptive::EngagementMeasure::DiskArea,
        path_strategy: crate::adaptive::PathStrategy2d::Agent,
        trochoid_cap_mult: 1.2,
        keep_down_links: crate::adaptive::KeepDownLinks::WithinPassLoad,
        entry_helix_radius: 0.0,
    };
    adaptive_segments_with_debug(polygon, &params, cancel, None, None)
}

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// Generate 2D adaptive segments and optionally record detailed debug spans.
pub(crate) fn adaptive_segments_with_debug(
    polygon: &Polygon2,
    params: &AdaptiveParams,
    cancel: &dyn CancelCheck,
    debug: Option<&ToolpathDebugContext>,
    // Stage 4 — optional sink for the contour-spiral's per-point predicted
    // leading-arc engagement (α/2π), collected 1:1 with the emitted Cut
    // path. `None` for every caller except the adaptive3d slice assembly,
    // which feeds it into the planner-engagement sampler.
    engagement_sink: Option<&mut Vec<(P2, f64)>>,
) -> Result<Vec<AdaptiveSegment>, Cancelled> {
    let tool_radius = params.tool_radius;
    let stepover = params.stepover;
    let tolerance = params.tolerance;
    let slot_clearing = params.slot_clearing;
    let cut_depth = params.cut_depth;
    // Inset polygon by tool radius to get the machinable region
    let machinable_vec = offset_polygon(polygon, tool_radius);
    if machinable_vec.is_empty() {
        return Ok(Vec::new());
    }
    let machinable = &machinable_vec[0];

    // Build material grid from the original polygon (not inset)
    let cell_size = (tool_radius / 6.0).max(tolerance);
    let mut grid = MaterialGrid::from_polygon(polygon, cell_size);
    if params.keep_down_links == KeepDownLinks::WithinPassLoad {
        grid.keep_fringe(polygon);
    }

    // If prior stock state is available, mark cells already cleared by
    // earlier operations so the adaptive algorithm does not re-cut them.
    if let Some(ref stock) = params.initial_stock {
        grid.apply_initial_stock(stock, cut_depth);
    }

    // Cache the machinable region as a boolean mask for fast lookups
    let machinable_mask = MaterialGrid::build_machinable_mask(
        machinable,
        grid.origin_x,
        grid.origin_y,
        grid.rows,
        grid.cols,
        grid.cell_size,
    );

    // Precompute boundary distance field for wall-tangent bias
    let boundary_distances = grid.compute_boundary_distances();

    // ── Narrow-region gate ───────────────────────────────────────────
    // For thin ring-shaped pockets (donut-topology with a hole hugging
    // the bounds, narrow strips), the engagement-target spiral has no
    // room to swing its ~21-candidate angle search and degenerates
    // into a sawtooth wiggle. When the largest inscribed disk inside
    // the machinable region is ≤ 2 × stepover (i.e. the cutter
    // diameter + a stepover doesn't fit on either side), skip the
    // spiral entirely and emit concentric contour-parallel offset
    // loops. See doc on `CleanupStrategy::ContourParallelNarrow`.
    if matches!(
        params.cleanup_strategy,
        CleanupStrategy::ContourParallelNarrow | CleanupStrategy::ContourParallelHybrid
    ) && is_narrow_machinable(machinable, tool_radius, stepover)
    {
        let narrow_load = LinkLoad::new(params, &grid, polygon);
        return contour_parallel_segments(
            machinable,
            &mut grid,
            &machinable_mask,
            stepover,
            cell_size,
            &narrow_load,
            cancel,
            None,
        );
    }

    let step_len = cell_size * 3.0;
    let link_load = LinkLoad::new(params, &grid, polygon);
    let pass_load = link_load.pass;
    // Where an agent pass may enter: the machinable region, or under a
    // helix entry the region inset by the helix radius (its entry mask), so
    // the helix does not cut a wall (G-ADAPTPASSLOAD).
    let entry_region: Option<(Polygon2, &[bool])> = link_load.entry_mask.as_deref().and_then(|m| {
        offset_polygon(polygon, tool_radius + params.entry_helix_radius)
            .into_iter()
            .max_by(|a, b| a.area().total_cmp(&b.area()))
            .map(|poly| (poly, m))
    });
    let (entry_poly, entry_mask) = match &entry_region {
        Some((poly, m)) => (poly, *m),
        None => (machinable, machinable_mask.as_slice()),
    };
    let holds_pass_load = link_load.holds_pass_load();
    // Passes in a row that removed nothing (G-ADAPTPASSLOAD: the 2D planner
    // no longer force-clears a stalled spot, so it stops re-entering it).
    let mut stalled_passes = 0usize;
    // Stock cells the agent's frontier hop found no load-held way to reach.
    let mut hop_skip = vec![false; if holds_pass_load { grid.cells.len() } else { 0 }];
    let mut segments = Vec::new();
    let mut last_pos: Option<P2> = None;
    let mut pass_endpoints = super::search::EndpointGrid::new(tool_radius * 3.0);

    // ── Slot clearing (Fusion-style first pass) ───────────────────────
    // Generate sparse zigzag lines at wide spacing to open pockets across
    // all regions of the polygon. Uses tool_diameter spacing so each line
    // creates a slot the adaptive spiral can expand from.
    if slot_clearing {
        let slot_scope = debug.map(|ctx| ctx.start_span("slot_clearing", "Slot clearing"));
        let (x_min, y_min, x_max, y_max) = polygon_bbox(&polygon.exterior);
        let w = x_max - x_min;
        let h = y_max - y_min;
        // Slot along the longest axis
        let slot_angle = if w >= h { 0.0 } else { 90.0 };
        // Target ~3 seeding lines across the pocket's narrow axis.
        // This opens pockets in all regions without doing the adaptive's job.
        let narrow_span = if w >= h { h } else { w };
        let slot_spacing = (narrow_span / 3.0).max(tool_radius * 4.0);
        let slot_lines =
            crate::ops::zigzag::zigzag_lines(polygon, tool_radius, slot_spacing, slot_angle);

        for (line_idx, line) in slot_lines.iter().enumerate() {
            check_cancel(cancel)?;
            segments.push(AdaptiveSegment::Marker(
                AdaptiveRuntimeEvent::SlotClearing {
                    line_index: line_idx + 1,
                    line_total: slot_lines.len(),
                },
            ));
            segments.push(AdaptiveSegment::Rapid(line[0]));

            // Walk along the line and clear material in the grid
            let dx = line[1].x - line[0].x;
            let dy = line[1].y - line[0].y;
            let len = (dx * dx + dy * dy).sqrt();
            let n_steps = (len / (cell_size * 1.5)).ceil() as usize;
            for j in 0..=n_steps {
                let t = j as f64 / n_steps.max(1) as f64;
                let x = line[0].x + t * dx;
                let y = line[0].y + t * dy;
                grid.clear_circle(x, y, tool_radius);
            }

            segments.push(AdaptiveSegment::Cut(vec![line[0], line[1]]));
            last_pos = Some(line[1]);
        }
        if let Some(scope) = slot_scope.as_ref() {
            scope.set_counter("line_count", slot_lines.len() as f64);
        }
    }

    // ── Helical starter pocket ────────────────────────────────────────
    // Pre-clear a 2 × tool_radius disc at the medial-axis maximum so
    // the engagement-target spiral has full swing room from move 1
    // and skips the bootstrap convergence wiggle. Gated on a non-
    // Legacy cleanup strategy — adaptive3d and other downstream
    // consumers that rely on the original boundary-entry spiral
    // sweep (for full-coverage guarantees on concave shapes) keep
    // their current behaviour via the Legacy path. If no DT-max
    // cell qualifies (≥ 2 × tool_radius clearance), proceed without
    // a starter pocket — the spiral will bootstrap as before.
    let helical_entry_pos: Option<P2> =
        if matches!(params.cleanup_strategy, CleanupStrategy::Legacy) {
            None
        } else if let Some((helix_segments, helix_end)) = emit_helical_starter_pocket(
            &mut grid,
            &machinable_mask,
            &boundary_distances,
            tool_radius,
            holds_pass_load.then_some((&pass_load, stepover, &link_load)),
        ) {
            segments.extend(helix_segments);
            last_pos = Some(helix_end);
            Some(helix_end)
        } else {
            None
        };

    // ── Contour-spiral passes (Stage 1, constructive) ─────────────────
    // Replaces only the agent loop below; the narrow gate, starter
    // pocket, residue cleanup and emission stay shared. Requires the
    // starter pocket (the first wrap rides flush with its cleared rim);
    // when it isn't available — or the spiral degenerates — fall through
    // to the agent loop.
    if matches!(
        params.path_strategy,
        crate::adaptive::PathStrategy2d::ContourSpiral
    ) && let Some(starter_end) = helical_entry_pos
    {
        let spiral_scope = debug.map(|ctx| ctx.start_span("contour_spiral", "Contour spiral"));
        let applied = super::spiral::spiral_passes(
            &mut grid,
            &machinable_mask,
            tool_radius,
            stepover,
            starter_end,
            params.trochoid_cap_mult,
            holds_pass_load.then_some(&pass_load),
            &mut segments,
            &mut last_pos,
            engagement_sink,
            cancel,
        )?;
        if let Some(scope) = spiral_scope.as_ref() {
            scope.set_counter("applied", if applied { 1.0 } else { 0.0 });
        }
        if applied {
            return Ok(segments);
        }
    }

    // ── Adaptive passes ───────────────────────────────────────────────
    let max_passes = 500; // safety limit
    let mut pass_count = 0;

    while grid.material_fraction() > 0.01 && pass_count < max_passes {
        check_cancel(cancel)?;
        if stalled_passes >= MAX_STALLED_PASSES {
            break;
        }
        pass_count += 1;

        // For non-Legacy strategies with a successful helical entry:
        // only the helical-entry-driven pass 1 produces useful spiral
        // arcs. Subsequent passes would enter at the machinable
        // boundary and run engagement-target search through whatever
        // remains — which, by construction, is now narrow strips
        // where the search degenerates into wiggle. The cleanup phase
        // (boundary cleanup + contour-parallel sweep + cell-walking
        // mop) handles those strips with clean boundary walks
        // instead.
        //
        // When helical entry was skipped (region too small to fit a
        // 2 × tool_radius starter pocket — common in adaptive3d's
        // per-region calls), the spiral runs Legacy-style across
        // multiple passes: we don't have the helical bootstrap to
        // make a single pass cover the region, so capping it would
        // leave material uncleared.
        //
        // G-ADAPTPASSLOAD: under the 2D pass load a pass also ends where
        // every step would overload, which is not a narrow strip; later
        // passes re-enter at the entries the helix fits (the stall limit
        // stops them when they no longer cut).
        if pass_count > 1
            && helical_entry_pos.is_some()
            && !matches!(params.cleanup_strategy, CleanupStrategy::Legacy)
            && !holds_pass_load
        {
            break;
        }

        let pass_started = Instant::now();
        let material_before = grid.material_fraction();
        let material_count_before = grid.material_count;
        let pass_scope =
            debug.map(|ctx| ctx.start_span("adaptive_pass", format!("Pass {pass_count}")));
        if let Some(scope) = pass_scope.as_ref() {
            scope.set_z_level(cut_depth);
            scope.set_counter("material_fraction_before", material_before);
        }
        let pass_ctx = pass_scope.as_ref().map(|scope| scope.context());

        // Find entry point.
        //
        // For pass 1, if a helical starter pocket was emitted upstream
        // the spiral picks up from the helix end directly — that
        // already puts the cutter inside a 2 × tool_radius cleared
        // disc, so the engagement-target search has its full swing
        // band on move 1 and skips the bootstrap wiggle. Falls
        // through to the boundary walk otherwise.
        let entry_scope = pass_ctx
            .as_ref()
            .map(|ctx| ctx.start_span("entry_search", format!("Entry {pass_count}")));
        let mut entered_by_hop = false;
        let entry = if pass_count == 1 {
            if let Some(p) = helical_entry_pos {
                Some(p)
            } else {
                find_entry_point(
                    &grid,
                    entry_mask,
                    entry_poly,
                    tool_radius,
                    last_pos,
                    &pass_endpoints,
                )
            }
        } else if let Some(hop) = last_pos.filter(|_| holds_pass_load).and_then(|from| {
            frontier_hop(&mut grid, &machinable_mask, from, &link_load, &mut hop_skip)
        }) {
            // G-ADAPTPASSLOAD: a later pass starts beside the nearest stock
            // by a load-held keep-down link (already walked and stamped),
            // not by a retract and a helix at the region boundary.
            entered_by_hop = true;
            Some(hop.0)
        } else {
            find_entry_point(
                &grid,
                entry_mask,
                entry_poly,
                tool_radius,
                last_pos,
                &pass_endpoints,
            )
        };
        let Some(entry) = entry else {
            if let Some(scope) = pass_scope.as_ref() {
                scope.set_exit_reason("no entry");
                scope.set_counter("pass_index", pass_count as f64);
            }
            break;
        };
        if let Some(scope) = entry_scope.as_ref() {
            scope.set_xy_bbox(ToolpathDebugBounds2 {
                min_x: entry.x,
                max_x: entry.x,
                min_y: entry.y,
                max_y: entry.y,
            });
        }

        // Link or retract to entry point
        let max_link_dist = tool_radius * 6.0; // ~3 tool diameters
        segments.push(AdaptiveSegment::Marker(AdaptiveRuntimeEvent::PassEntry {
            pass_index: pass_count,
            entry_x: entry.x,
            entry_y: entry.y,
        }));
        if let Some(last) = last_pos {
            let dx = entry.x - last.x;
            let dy = entry.y - last.y;
            let dist = (dx * dx + dy * dy).sqrt();
            let keep_down = entered_by_hop
                || dist < max_link_dist
                    && match params.keep_down_links {
                        KeepDownLinks::WithinPassLoad => {
                            feed_link_to_legal(&mut grid, &machinable_mask, last, entry, &link_load)
                        }
                        KeepDownLinks::RetractedByCaller => {
                            is_clear_path(&grid, &machinable_mask, last, entry, tool_radius)
                        }
                    };
            if keep_down {
                segments.push(AdaptiveSegment::Link(entry));
            } else {
                segments.push(AdaptiveSegment::Rapid(entry));
            }
        } else {
            segments.push(AdaptiveSegment::Rapid(entry));
        }

        // Walk the adaptive path from this entry
        let mut path = vec![entry];
        let mut cx = entry.x;
        let mut cy = entry.y;

        // Initial direction: toward nearest material
        let mut prev_angle = if let Some(pos) = last_pos {
            (entry.y - pos.y).atan2(entry.x - pos.x)
        } else if let Some((mx, my)) = grid.find_nearest_material(cx, cy) {
            (my - cy).atan2(mx - cx)
        } else {
            0.0
        };

        // Clear material at entry position: the link's landing, or the hole
        // a re-entry cuts (with the entry dressup's helix under the 2D
        // rule).
        let entered_by_rapid = matches!(
            segments
                .iter()
                .rev()
                .find(|s| !matches!(s, AdaptiveSegment::Marker(_))),
            Some(AdaptiveSegment::Rapid(_))
        );
        if holds_pass_load && entered_by_rapid {
            link_load.stamp_entry(&mut grid, P2::new(cx, cy));
        }
        grid.clear_circle(cx, cy, tool_radius);

        // Direction smoothing buffer (gyro) — average last N directions
        // for smooth curves instead of jagged steps. Inspired by Freesteel.
        const SMOOTH_BUF_LEN: usize = 3;
        let mut angle_buf: Vec<f64> = Vec::with_capacity(SMOOTH_BUF_LEN);

        let max_steps = 5000;
        let mut idle_count = 0;
        let mut search_evaluations = 0u32;
        // Trace-only counters (G-ADAPTPASSLOAD): gradient steps taken, the
        // gradient steps the pass load turned over to the capped search,
        // steps taken outside the band, and a pass ended by the cap.
        let mut gradient_steps = 0u32;
        let mut gradient_over_load = 0u32;
        let mut fallback_steps = 0u32;
        let mut departure_steps = 0u32;
        let mut was_departing = false;
        let mut frontier_hops = 0u32;
        let mut last_hop: Option<(usize, usize)> = None;
        let mut refused = false;
        // CONVERGENCE DETECTOR — disabled, kept here for context.
        //
        // Two variants tried: engagement-based (exit when engagement <
        // 0.4–0.7× target for N steps) and angle-oscillation (exit
        // when |Δangle| > π/2…2.5 for N steps). Both failed because
        // the visible sawtooth-at-spiral-end is *small-amplitude
        // continuous noise* (~60° per-step changes that integrate to
        // a wiggle), not a sharp signal. Engagement stays near target
        // through the wiggle; per-step angle changes don't exceed
        // what a tight wrap legitimately needs. Set the threshold
        // strict enough to catch the wiggle → fragments healthy
        // spirals; set it loose enough to spare healthy spirals →
        // wiggle survives.
        //
        // Right fix is structural: path-smoothing post-process OR
        // algorithm-swap to offset-loops when boundary distance gets
        // tight. Filed as I2-followup.
        for _step_idx in 0..max_steps {
            check_cancel(cancel)?;
            let before = grid.material_count;

            // Smoothed direction: average recent angles for prev_angle hint
            let smoothed_angle = if angle_buf.len() >= 2 {
                average_angles(&angle_buf)
            } else {
                prev_angle
            };

            // Search for next direction. When the cutter is in a
            // region too thin for the engagement-target search to
            // swing (dt_here < tool_radius + stepover), switch to
            // gradient-following: pick the direction perpendicular
            // to ∇boundary_distance, riding the strip's centerline.
            // Single clean pass instead of wiggle.
            //
            // Gated additionally on helical entry having succeeded —
            // without the bootstrap, the cutter starts at the
            // boundary with dt_here ≈ tool_radius < threshold, and
            // we'd switch to gradient mode from step 1 with no
            // momentum. Better to let engagement-target run.
            // G-ADAPTPASSLOAD: out of a straight-plunge entry the steps are
            // exempt until the cutter has left its plunge hole.
            let departing = entered_by_rapid
                && link_load.plunge_departure_exempt()
                && (cx - entry.x).hypot(cy - entry.y) < tool_radius;
            let step_load = if departing {
                departure_steps += 1;
                pass_load.departing()
            } else {
                pass_load
            };
            // The exempt departure is its own cut, so no emitted move mixes
            // exempt and held steps (the simplifier merges within a cut).
            if was_departing && !departing && path.len() >= 2 {
                let here = P2::new(cx, cy);
                segments.push(AdaptiveSegment::Cut(std::mem::replace(
                    &mut path,
                    vec![here],
                )));
            }
            was_departing = departing;
            let dt_here = grid.boundary_distance_at(&boundary_distances, cx, cy);
            let use_gradient = helical_entry_pos.is_some()
                && !matches!(params.cleanup_strategy, CleanupStrategy::Legacy)
                && dt_here < tool_radius + stepover;
            let search_result_opt = if use_gradient {
                match search_direction_gradient(
                    &grid,
                    &machinable_mask,
                    &boundary_distances,
                    cx,
                    cy,
                    step_len,
                    smoothed_angle,
                ) {
                    // G-ADAPTPASSLOAD: the gradient picks a heading without
                    // reading material; under the 2D cap its step is held to
                    // the pass load, else the capped search takes over.
                    // Under the 2D load the gradient step must also cut: a
                    // centreline walk over cut stock is not a pass step (it
                    // left the agent idling along walls it had cleared).
                    Some(r)
                        if holds_pass_load
                            && (!link_load.legal_step(
                                &grid,
                                &machinable_mask,
                                P2::new(cx, cy),
                                P2::new(
                                    cx + step_len * r.angle.cos(),
                                    cy + step_len * r.angle.sin(),
                                ),
                            ) || !{
                                let w = super::search::measure_engagement(
                                    &grid,
                                    cx + step_len * r.angle.cos(),
                                    cy + step_len * r.angle.sin(),
                                    tool_radius,
                                    r.angle,
                                    step_load.measure,
                                );
                                w >= step_load.presence
                                    && super::search::step_within_pass_load(
                                        &grid,
                                        cx + step_len * r.angle.cos(),
                                        cy + step_len * r.angle.sin(),
                                        tool_radius,
                                        r.angle,
                                        &step_load,
                                    )
                            }) =>
                    {
                        gradient_over_load += 1;
                        search_direction_with_metrics(
                            &grid,
                            &machinable_mask,
                            cx,
                            cy,
                            tool_radius,
                            step_len,
                            &step_load,
                            smoothed_angle,
                            &boundary_distances,
                            link_load.region.as_ref(),
                        )
                    }
                    Some(r) => {
                        gradient_steps += 1;
                        Ok(r)
                    }
                    // The gradient step would leave the machinable region.
                    // Under the 2D cap the capped search may still find a
                    // step within the load; historically the pass ended.
                    None if holds_pass_load => search_direction_with_metrics(
                        &grid,
                        &machinable_mask,
                        cx,
                        cy,
                        tool_radius,
                        step_len,
                        &step_load,
                        smoothed_angle,
                        &boundary_distances,
                        link_load.region.as_ref(),
                    ),
                    None => Err(NoStep::NoMaterial),
                }
            } else {
                search_direction_with_metrics(
                    &grid,
                    &machinable_mask,
                    cx,
                    cy,
                    tool_radius,
                    step_len,
                    &step_load,
                    smoothed_angle,
                    &boundary_distances,
                    link_load.region.as_ref(),
                )
            };
            let search_result = match search_result_opt {
                Ok(r) => r,
                Err(_)
                    if holds_pass_load
                        && let Some((hop, hop_target)) = frontier_hop(
                            &mut grid,
                            &machinable_mask,
                            P2::new(cx, cy),
                            &link_load,
                            &mut hop_skip,
                        ) =>
                {
                    // G-ADAPTPASSLOAD: the pass has run out of stock within a
                    // step that holds the load. Carry on from the nearest
                    // stock instead of ending: a keep-down link that holds the
                    // load (it is stamped) to a stand-off beside it.
                    frontier_hops += 1;
                    // A hop after which the pass removed nothing gives its
                    // target up (it stays stock for the cleanup).
                    if let Some((cell, count)) = last_hop
                        && grid.material_count >= count
                        && let Some(flag) = hop_skip.get_mut(cell)
                    {
                        *flag = true;
                    }
                    last_hop = grid
                        .cell_index(hop_target.0, hop_target.1)
                        .map(|c| (c, grid.material_count));
                    let done = std::mem::replace(&mut path, vec![hop]);
                    if done.len() >= 2 {
                        segments.push(AdaptiveSegment::Cut(done));
                    }
                    segments.push(AdaptiveSegment::Link(hop));
                    prev_angle = (hop_target.1 - hop.y).atan2(hop_target.0 - hop.x);
                    cx = hop.x;
                    cy = hop.y;
                    angle_buf.clear();
                    idle_count = 0;
                    continue;
                }
                Err(why) => {
                    refused = why == NoStep::OverPassLoad;
                    break;
                }
            };
            if !search_result.in_band {
                fallback_steps += 1;
            }
            search_evaluations += search_result.evaluations;
            let angle = search_result.angle;

            // Move in that direction
            let from = P2::new(cx, cy);
            cx += step_len * angle.cos();
            cy += step_len * angle.sin();
            path.push(P2::new(cx, cy));

            // Clear the material the step sweeps
            grid.clear_segment(from, P2::new(cx, cy), tool_radius);

            // Update direction smoothing buffer
            if angle_buf.len() >= SMOOTH_BUF_LEN {
                angle_buf.remove(0);
            }
            angle_buf.push(angle);

            // Idle detection: if no material was cleared for many steps, we're
            // going in circles over already-cleared area.
            if grid.material_count == before {
                idle_count += 1;
                if idle_count > 15 {
                    break;
                }
            } else {
                idle_count = 0;
            }

            prev_angle = angle;
        }

        let was_idle = idle_count > 15;
        let exit_reason = if was_idle {
            "idle"
        } else if refused {
            "over pass load"
        } else {
            "no direction"
        };

        let path_len = path.len();
        let path_debug_bounds = path_bounds(&path);

        if path_len >= 2 {
            // SAFETY: path.len() >= 2 checked on line above
            #[allow(clippy::expect_used)]
            let endpoint = *path.last().expect("path is non-empty after loop");
            last_pos = Some(endpoint);
            pass_endpoints.insert(endpoint);
            segments.push(AdaptiveSegment::Cut(path));
        } else {
            last_pos = Some(entry);
            pass_endpoints.insert(entry);
        }

        // If the pass ended due to idle detection, the remaining material
        // nearby is too small or inaccessible. Force-clear a wider area
        // around the last position to prevent revisiting the same spot.
        // G-ADAPTPASSLOAD: the 2D planner does not force-clear. The wipe
        // marked 2R of stock cleared with no motion, so later passes and
        // links planned through material the sim still holds. The residue
        // stays on the grid for the cleanup strategies; a stalled spot is
        // left by the endpoint exclusion and the stall limit below.
        if holds_pass_load {
            // A pass that cut no step (its entry alone may have removed
            // stock) or removed nothing is stalled.
            if path_len < 2 || grid.material_count == material_count_before {
                stalled_passes += 1;
            } else {
                stalled_passes = 0;
            }
        }
        if was_idle && !holds_pass_load {
            let forced_clear_scope = pass_ctx
                .as_ref()
                .map(|ctx| ctx.start_span("forced_clear", format!("Forced clear {pass_count}")));
            grid.clear_circle(cx, cy, tool_radius * 2.0);
            segments.push(AdaptiveSegment::Marker(AdaptiveRuntimeEvent::ForcedClear {
                pass_index: pass_count,
                center_x: cx,
                center_y: cy,
                radius: tool_radius * 2.0,
            }));
            if let Some(scope) = forced_clear_scope.as_ref() {
                scope.set_xy_bbox(ToolpathDebugBounds2 {
                    min_x: cx - tool_radius * 2.0,
                    max_x: cx + tool_radius * 2.0,
                    min_y: cy - tool_radius * 2.0,
                    max_y: cy + tool_radius * 2.0,
                });
                scope.set_z_level(cut_depth);
            }
        }

        if let Some(scope) = pass_scope.as_ref() {
            scope.set_counter("pass_index", pass_count as f64);
            scope.set_counter("step_count", path_len as f64);
            scope.set_counter("idle_count", idle_count as f64);
            scope.set_counter("search_evaluations", search_evaluations as f64);
            scope.set_counter("gradient_steps", f64::from(gradient_steps));
            scope.set_counter("gradient_over_pass_load", f64::from(gradient_over_load));
            scope.set_counter("fallback_steps", f64::from(fallback_steps));
            scope.set_counter("plunge_departure_steps", f64::from(departure_steps));
            scope.set_counter("frontier_hops", f64::from(frontier_hops));
            scope.set_counter("refused_over_pass_load", if refused { 1.0 } else { 0.0 });
            scope.set_counter("material_fraction_after", grid.material_fraction());
            scope.set_exit_reason(exit_reason);
            if let Some(bounds) = path_debug_bounds {
                scope.set_xy_bbox(bounds);
                let (center_x, center_y) = bounds.center();
                if let Some(ctx) = pass_ctx.as_ref() {
                    ctx.record_hotspot(&HotspotRecord {
                        kind: "adaptive_pass".into(),
                        center_x,
                        center_y,
                        z_level: Some(cut_depth),
                        bucket_size_xy: tool_radius * 2.0,
                        bucket_size_z: Some(tolerance.max(step_len)),
                        elapsed_us: pass_started.elapsed().as_micros() as u64,
                        pass_count: 1,
                        step_count: path_len as u64,
                        low_yield_exit_count: 0,
                    });
                }
            }
            scope.set_z_level(cut_depth);
        }
        segments.push(AdaptiveSegment::Marker(AdaptiveRuntimeEvent::PassSummary {
            pass_index: pass_count,
            step_count: path_len,
            idle_count,
            search_evaluations: search_evaluations as usize,
            exit_reason: exit_reason.to_owned(),
        }));
    }

    // ── Boundary cleanup pass ─────────────────────────────────────────
    // Trace ALL machinable boundaries (exterior + hole contours) to sweep
    // any thin strip of material left along the walls. This is the
    // tool-center contour that puts the tool edge right on each wall.
    //
    // ContourParallelHybrid mode SKIPS this pass — its post-process
    // contour-parallel sweep walks the same boundaries and continues
    // inward at stepover intervals, which subsumes this single-loop
    // sweep and exposes residue-region differences cleanly.
    let mut contours: Vec<&Vec<P2>> = Vec::new();
    if !matches!(
        params.cleanup_strategy,
        CleanupStrategy::ContourParallelHybrid
    ) {
        if machinable.exterior.len() >= 3 {
            contours.push(&machinable.exterior);
        }
        for hole in &machinable.holes {
            if hole.len() >= 3 {
                contours.push(hole);
            }
        }
    }

    let cleanup_scope = debug.map(|ctx| ctx.start_span("boundary_cleanup", "Boundary cleanup"));
    for (contour_idx, boundary) in contours.iter().enumerate() {
        check_cancel(cancel)?;
        let marker_at = segments.len();
        segments.push(AdaptiveSegment::Marker(
            AdaptiveRuntimeEvent::BoundaryCleanup {
                contour_index: contour_idx + 1,
                contour_total: contours.len(),
            },
        ));
        if holds_pass_load {
            // G-ADAPTPASSLOAD: the wall loop is cut where each step holds
            // the pass load, entered by a load-held link or a retract.
            let path = contour_walk_points(boundary, cell_size);
            if emit_capped_walk(
                &path,
                &mut grid,
                &machinable_mask,
                &link_load,
                &mut last_pos,
                &mut segments,
            ) == 0
            {
                segments.truncate(marker_at);
            }
            continue;
        }
        segments.push(AdaptiveSegment::Rapid(boundary[0]));

        let mut cleanup_path = vec![boundary[0]];
        // Walk the contour, clearing material and interpolating between
        // vertices so no cells are missed on long edges.
        for i in 0..boundary.len() {
            let a = boundary[i];
            let b = boundary[(i + 1) % boundary.len()];
            let dx = b.x - a.x;
            let dy = b.y - a.y;
            let len = (dx * dx + dy * dy).sqrt();
            let n_steps = (len / (cell_size * 1.5)).ceil() as usize;
            for j in 1..=n_steps {
                let t = j as f64 / n_steps.max(1) as f64;
                let x = a.x + t * dx;
                let y = a.y + t * dy;
                grid.clear_circle(x, y, tool_radius);
                cleanup_path.push(P2::new(x, y));
            }
        }
        // Close the loop back to the start
        grid.clear_circle(boundary[0].x, boundary[0].y, tool_radius);
        cleanup_path.push(boundary[0]);
        segments.push(AdaptiveSegment::Cut(cleanup_path));
    }
    if let Some(scope) = cleanup_scope.as_ref() {
        scope.set_counter("contour_count", contours.len() as f64);
        scope.set_z_level(cut_depth);
    }

    Ok(segments)
}

// ── ResidueMop cleanup strategy ────────────────────────────────────────
//
// Post-processes a segment stream produced by `adaptive_segments_with_debug`:
//
//   1. Drop short Cut groups (< MIN_KEEP_STEPS), along with their preceding
//      Rapid/Link approach. The main spiral, any subsequent long adaptive
//      sweeps, and the boundary_cleanup pass survive verbatim.
//   2. Lookahead-filter orphan Rapid/Link runs left by step 1 (a Marker
//      can sit between consecutive transitions, defeating naive
//      consecutive-collapse).
//   3. Replay the kept segments on a MaterialGrid to determine remaining
//      residue.
//   4. Walk the residue with `mop_residue_into_segments`, emitting one
//      Cut per residue patch with Rapid/Link transitions between.
//
// Cf. `CleanupStrategy::ResidueMop` doc on `AdaptiveParams`.
const MIN_KEEP_STEPS: usize = 40;

/// Threshold for keeping an otherwise-short Cut: if at least this
/// fraction of its sampled path points lies over material that would
/// not be cleared by the long Cuts alone, the short Cut is kept. The
/// principle is that *overlap is cheap, travel is expensive* — dropping
/// a short Cut and re-cleaning the same material via a mop patch
/// costs an extra Rapid (retract + travel + plunge), which is far
/// more cycle-time than the small overlap of keeping the short Cut.
const SHORT_CUT_UNIQUE_FRACTION: f64 = 0.10;

/// Apply the smart short-Cut drop rule on `segments`: returns a
/// filtered list where Cuts with `len() >= MIN_KEEP_STEPS` are always
/// kept, and shorter Cuts are kept only if they clear material that
/// the long Cuts wouldn't (i.e. `unique_fraction >= SHORT_CUT_UNIQUE_FRACTION`).
/// Preceding Rapid/Link approaches are dropped along with the Cuts
/// they introduce (consistent with the original blunt filter).
#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
fn filter_short_cuts_by_redundancy(
    segments: &[AdaptiveSegment],
    polygon: &Polygon2,
    params: &AdaptiveParams,
    cell_size: f64,
) -> Vec<AdaptiveSegment> {
    let tool_radius = params.tool_radius;

    // Build a grid populated with the cells cleared by the LONG Cuts only.
    let mut long_grid = MaterialGrid::from_polygon(polygon, cell_size);
    if let Some(stock) = &params.initial_stock {
        long_grid.apply_initial_stock(stock, params.cut_depth);
    }
    for seg in segments {
        if let AdaptiveSegment::Cut(path) = seg
            && path.len() >= MIN_KEEP_STEPS
        {
            for p in path {
                long_grid.clear_circle(p.x, p.y, tool_radius);
            }
        }
    }

    // Walk short Cuts in order, accumulating their clearing into the
    // grid so subsequent short Cuts see each other's contributions.
    let mut keep_index: Vec<bool> = Vec::with_capacity(segments.len());
    for seg in segments {
        if let AdaptiveSegment::Cut(path) = seg {
            if path.len() >= MIN_KEEP_STEPS {
                keep_index.push(true);
                continue;
            }
            if path.is_empty() {
                keep_index.push(false);
                continue;
            }
            // Sample-fraction of points over still-material cells.
            let unique = path
                .iter()
                .filter(|p| long_grid.is_material(p.x, p.y))
                .count();
            let frac = unique as f64 / path.len() as f64;
            if frac >= SHORT_CUT_UNIQUE_FRACTION {
                keep_index.push(true);
                for p in path {
                    long_grid.clear_circle(p.x, p.y, tool_radius);
                }
            } else {
                keep_index.push(false);
            }
        } else {
            keep_index.push(true); // R/L/Marker handled below by orphan trim
        }
    }

    // Walk segments, dropping non-kept Cuts and their preceding
    // contiguous Rapid/Link runs.
    let mut filtered: Vec<AdaptiveSegment> = Vec::with_capacity(segments.len());
    for (i, seg) in segments.iter().enumerate() {
        match seg {
            AdaptiveSegment::Cut(_) => {
                if keep_index[i] {
                    filtered.push(seg.clone());
                } else {
                    while let Some(last) = filtered.last() {
                        if matches!(last, AdaptiveSegment::Rapid(_) | AdaptiveSegment::Link(_)) {
                            filtered.pop();
                        } else {
                            break;
                        }
                    }
                }
            }
            AdaptiveSegment::Rapid(_) | AdaptiveSegment::Link(_) => filtered.push(seg.clone()),
            AdaptiveSegment::Marker(_) => filtered.push(seg.clone()),
        }
    }
    while let Some(last) = filtered.last() {
        if matches!(last, AdaptiveSegment::Rapid(_) | AdaptiveSegment::Link(_)) {
            filtered.pop();
        } else {
            break;
        }
    }
    filtered
}

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
pub(crate) fn apply_residue_mop_cleanup(
    polygon: &Polygon2,
    params: &AdaptiveParams,
    segments: &[AdaptiveSegment],
) -> Vec<AdaptiveSegment> {
    let tool_radius = params.tool_radius;
    let cell_size = (tool_radius / 6.0).max(params.tolerance);
    let step_len = cell_size * 3.0;

    // Step 1 — smart short-Cut drop. Short Cuts are kept when they
    // clear material the long Cuts wouldn't (overlap is cheap; travel
    // is expensive — see `SHORT_CUT_UNIQUE_FRACTION`).
    // G-ADAPTPASSLOAD: under the 2D pass load every kept cut is replayed
    // on the stock the cuts before it left, so dropping a short cut that
    // removed stock would load the cuts after it; keep them all.
    let filtered = if params.keep_down_links == KeepDownLinks::WithinPassLoad {
        segments.to_vec()
    } else {
        filter_short_cuts_by_redundancy(segments, polygon, params, cell_size)
    };

    // Step 2 — lookahead-filter orphan R/L (Markers can sit between).
    let mut out: Vec<AdaptiveSegment> = Vec::with_capacity(filtered.len());
    for (i, seg) in filtered.iter().enumerate() {
        // G-ADAPTPASSLOAD: under the 2D pass load a link or entry is motion
        // the planner stamped (a load-held link may cut); it stays.
        if params.keep_down_links != KeepDownLinks::WithinPassLoad
            && matches!(seg, AdaptiveSegment::Rapid(_) | AdaptiveSegment::Link(_))
        {
            let mut has_cut_next = false;
            for next in &filtered[i + 1..] {
                match next {
                    AdaptiveSegment::Cut(_) => {
                        has_cut_next = true;
                        break;
                    }
                    AdaptiveSegment::Rapid(_) | AdaptiveSegment::Link(_) => break,
                    AdaptiveSegment::Marker(_) => continue,
                }
            }
            if !has_cut_next {
                continue;
            }
        }
        out.push(seg.clone());
    }

    // Step 3 — replay grid state and last cutter position, deciding each
    // kept link again on the replayed grid.
    let mut grid = MaterialGrid::from_polygon(polygon, cell_size);
    if params.keep_down_links == KeepDownLinks::WithinPassLoad {
        grid.keep_fringe(polygon);
    }
    if let Some(stock) = &params.initial_stock {
        grid.apply_initial_stock(stock, params.cut_depth);
    }
    let machinable_vec = crate::polygon::offset_polygon(polygon, tool_radius);
    if machinable_vec.is_empty() {
        return out;
    }
    let machinable = &machinable_vec[0];
    let machinable_mask = MaterialGrid::build_machinable_mask(
        machinable,
        grid.origin_x,
        grid.origin_y,
        grid.rows,
        grid.cols,
        grid.cell_size,
    );
    let link_load = LinkLoad::new(params, &grid, polygon);
    let (mut out, last_pos) = replay_kept_segments(out, &mut grid, &machinable_mask, &link_load);

    // Step 4 — mop remaining residue.
    if link_load.holds_pass_load() {
        // G-ADAPTPASSLOAD: the wall slivers first, by a pass along the
        // region boundary, then the mop; a mop first re-entered for each
        // sliver along a wall.
        finish_walls_within_pass_load(
            machinable,
            &mut grid,
            &machinable_mask,
            &link_load,
            &mut out,
        );
        return out;
    }
    let mop_segments = mop_residue_into_segments(
        &mut grid,
        &machinable_mask,
        tool_radius,
        step_len,
        last_pos,
        &link_load,
    );
    out.extend(mop_segments);
    out
}

// ── Hybrid cleanup strategy ────────────────────────────────────────────
//
// Same filter/replay as `apply_residue_mop_cleanup`, but the residue
// phase walks `machinable` inward at stepover offsets (via
// `contour_parallel_segments`, which skips contours that don't pass
// through material). After the contour-parallel sweep, any tiny patches
// the contour walks didn't reach get cleaned up by the cell-walking
// `mop_residue_into_segments` fallback.
//
// On shapes with a wide core and narrow extensions (tadpole, key), the
// spiral handles the core and the contour-parallel sweep handles the
// extensions with clean concentric loops. See `CleanupStrategy::ContourParallelHybrid`.
pub(crate) fn apply_contour_parallel_residue_cleanup(
    polygon: &Polygon2,
    params: &AdaptiveParams,
    segments: &[AdaptiveSegment],
) -> Vec<AdaptiveSegment> {
    let tool_radius = params.tool_radius;
    let cell_size = (tool_radius / 6.0).max(params.tolerance);
    let step_len = cell_size * 3.0;
    let stepover = params.stepover;

    // Step 1 — smart short-Cut drop. Short Cuts are kept when they
    // clear material the long Cuts wouldn't (overlap is cheap; travel
    // is expensive — see `SHORT_CUT_UNIQUE_FRACTION`).
    // G-ADAPTPASSLOAD: under the 2D pass load every kept cut is replayed
    // on the stock the cuts before it left, so dropping a short cut that
    // removed stock would load the cuts after it; keep them all.
    let filtered = if params.keep_down_links == KeepDownLinks::WithinPassLoad {
        segments.to_vec()
    } else {
        filter_short_cuts_by_redundancy(segments, polygon, params, cell_size)
    };

    // Step 2 — lookahead-filter orphan R/L (Markers can sit between).
    let mut out: Vec<AdaptiveSegment> = Vec::with_capacity(filtered.len());
    for (i, seg) in filtered.iter().enumerate() {
        // G-ADAPTPASSLOAD: under the 2D pass load a link or entry is motion
        // the planner stamped (a load-held link may cut); it stays.
        if params.keep_down_links != KeepDownLinks::WithinPassLoad
            && matches!(seg, AdaptiveSegment::Rapid(_) | AdaptiveSegment::Link(_))
        {
            let mut has_cut_next = false;
            #[allow(clippy::indexing_slicing)] // i < filtered.len()
            for next in &filtered[i + 1..] {
                match next {
                    AdaptiveSegment::Cut(_) => {
                        has_cut_next = true;
                        break;
                    }
                    AdaptiveSegment::Rapid(_) | AdaptiveSegment::Link(_) => break,
                    AdaptiveSegment::Marker(_) => continue,
                }
            }
            if !has_cut_next {
                continue;
            }
        }
        out.push(seg.clone());
    }

    // Step 3 — replay grid state and last cutter position, deciding each
    // kept link again on the replayed grid.
    let mut grid = MaterialGrid::from_polygon(polygon, cell_size);
    if params.keep_down_links == KeepDownLinks::WithinPassLoad {
        grid.keep_fringe(polygon);
    }
    if let Some(stock) = &params.initial_stock {
        grid.apply_initial_stock(stock, params.cut_depth);
    }
    let machinable_vec = crate::polygon::offset_polygon(polygon, tool_radius);
    if machinable_vec.is_empty() {
        return out;
    }
    #[allow(clippy::indexing_slicing)] // machinable_vec non-empty checked above
    let machinable = &machinable_vec[0];
    let machinable_mask = MaterialGrid::build_machinable_mask(
        machinable,
        grid.origin_x,
        grid.origin_y,
        grid.rows,
        grid.cols,
        grid.cell_size,
    );
    let link_load = LinkLoad::new(params, &grid, polygon);
    let (mut out, mut last_pos) =
        replay_kept_segments(out, &mut grid, &machinable_mask, &link_load);

    // Step 4 — contour-parallel sweep of residue (filtered by material
    // presence; sweeps only the offsets that actually cross residue).
    let never_cancel: &dyn CancelCheck = &|| false;
    if let Ok(contour_segments) = contour_parallel_segments(
        machinable,
        &mut grid,
        &machinable_mask,
        stepover,
        cell_size,
        &link_load,
        never_cancel,
        last_pos,
    ) {
        if let Some(last_cut) = contour_segments.iter().rev().find_map(|s| match s {
            AdaptiveSegment::Cut(p) => p.last().copied(),
            _ => None,
        }) {
            last_pos = Some(last_cut);
        }
        out.extend(contour_segments);
    }

    // Step 5 — tiny-patch fallback. Anything the contour walks missed
    // (sub-stepover slivers, far-off-axis residue) gets cleaned by the
    // cell-walking mop.
    if link_load.holds_pass_load() {
        // G-ADAPTPASSLOAD: the wall slivers first, by a pass along the
        // region boundary, then the mop; a mop first re-entered for each
        // sliver along a wall.
        finish_walls_within_pass_load(
            machinable,
            &mut grid,
            &machinable_mask,
            &link_load,
            &mut out,
        );
        return out;
    }
    let mop_segments = mop_residue_into_segments(
        &mut grid,
        &machinable_mask,
        tool_radius,
        step_len,
        last_pos,
        &link_load,
    );
    out.extend(mop_segments);
    out
}

/// Where the cutter stands after `segments`.
fn last_position(segments: &[AdaptiveSegment]) -> Option<P2> {
    segments.iter().rev().find_map(|s| match s {
        AdaptiveSegment::Cut(path) => path.last().copied(),
        AdaptiveSegment::Link(p) | AdaptiveSegment::Rapid(p) => Some(*p),
        AdaptiveSegment::Marker(_) => None,
    })
}

/// G-ADAPTPASSLOAD: the last cleanup under the 2D pass load. The mop
/// stands only on machinable lattice points, so it cannot reach the last
/// sliver along a wall (within a cell of R from the region boundary); the
/// contour loop on the region boundary that would take it may have been
/// refused while the band was thicker than the pass load. Walk the region
/// boundary again, cutting only where it meets stock and within the load,
/// then mop what that exposes. A no-op under the historical rule.
fn finish_walls_within_pass_load(
    machinable: &Polygon2,
    grid: &mut MaterialGrid,
    mask: &[bool],
    load: &LinkLoad,
    out: &mut Vec<AdaptiveSegment>,
) {
    if !load.holds_pass_load() {
        return;
    }
    // The walls of the region the planner reads (every piece), so the wall
    // pass walks exactly the boundary its moves are checked against.
    let pieces: &[Polygon2] = match &load.region {
        Some(region) => region.pieces(),
        None => std::slice::from_ref(machinable),
    };
    let contours: Vec<&Vec<P2>> = pieces
        .iter()
        .flat_map(|piece| std::iter::once(&piece.exterior).chain(piece.holes.iter()))
        .filter(|ring| ring.len() >= 3)
        .collect();
    // The mop thins a band the wall pass refused; the next wall pass then
    // takes the rest. Repeat while a round still removes stock.
    for _ in 0..WALL_FINISH_ROUNDS {
        let before = grid.material_count;
        finish_walls_once(&contours, grid, mask, load, out);
        if grid.material_count >= before {
            break;
        }
    }
}

/// Rounds of wall pass and mop in [`finish_walls_within_pass_load`].
const WALL_FINISH_ROUNDS: usize = 3;

fn finish_walls_once(
    contours: &[&Vec<P2>],
    grid: &mut MaterialGrid,
    mask: &[bool],
    load: &LinkLoad,
    out: &mut Vec<AdaptiveSegment>,
) {
    let mut here = last_position(out);
    let total = contours.len();
    for (i, contour) in contours.iter().enumerate() {
        let marker_at = out.len();
        out.push(AdaptiveSegment::Marker(
            AdaptiveRuntimeEvent::BoundaryCleanup {
                contour_index: i + 1,
                contour_total: total,
            },
        ));
        let walk = match here {
            Some(p) => rotate_contour_to_nearest(contour, p),
            None => (*contour).clone(),
        };
        let path = contour_walk_points(&walk, grid.cell_size);
        if emit_capped_walk_with(&path, grid, mask, load, &mut here, out, true, None) == 0 {
            out.truncate(marker_at);
        }
    }
    let mop = mop_residue_into_segments(grid, mask, load.tool_radius, load.step_len, here, load);
    out.extend(mop);
}

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// Walks the MaterialGrid directly, emitting one Cut per reachable
/// residue patch. See doc on `apply_residue_mop_cleanup`.
pub(crate) fn mop_residue_into_segments(
    grid: &mut MaterialGrid,
    machinable_mask: &[bool],
    tool_radius: f64,
    step_len: f64,
    start_pos: Option<P2>,
    link_load: &LinkLoad,
) -> Vec<AdaptiveSegment> {
    const MAX_PATCHES: usize = 400;
    const MAX_PATCHES_WITHIN_PASS_LOAD: usize = 20_000;
    const MAX_STEPS_PER_PATCH: usize = 600;
    // Lowered from 0.005 → 0.001 so the mop chases thin islands left
    // between the spiral's outermost reach and the boundary band.
    const RESIDUE_DONE_FRACTION: f64 = 0.001;
    let max_link_dist = tool_radius * 6.0;

    let mut segments: Vec<AdaptiveSegment> = Vec::new();
    let mut last_pos = start_pos;
    let mut patch_index = 0usize;
    // G-ADAPTPASSLOAD: residue cells the mop cannot start on (no link to
    // them holds the load and no helix entry reaches them inside the part).
    // They stay stock on the grid, so every load reading still sees them.
    let per_cell = if link_load.holds_pass_load() {
        grid.cells.len()
    } else {
        0
    };
    let mut targets = MopTargets {
        skip: vec![false; per_cell],
        plunge_only: vec![false; per_cell],
    };
    // Under the 2D pass load every patch removes stock or marks its cell
    // (see `mop_patch_start`), so the walk ends on its own; the cap is a
    // backstop sized for the smaller patches a capped walk makes.
    let max_patches = if link_load.holds_pass_load() {
        MAX_PATCHES_WITHIN_PASS_LOAD
    } else {
        MAX_PATCHES
    };

    for _ in 0..max_patches {
        if grid.material_fraction() < RESIDUE_DONE_FRACTION {
            break;
        }
        let search_from = last_pos.unwrap_or_else(|| P2::new(0.0, 0.0));
        let material_before_patch = grid.material_count;
        let mut target_cell = None;
        let (start, approach) = if link_load.holds_pass_load() {
            match mop_patch_start(
                grid,
                machinable_mask,
                search_from,
                last_pos,
                link_load,
                &mut targets,
            ) {
                Some((start, approach, cell)) => {
                    target_cell = Some(cell);
                    (start, approach)
                }
                None => break,
            }
        } else {
            let Some((mx, my)) = grid.find_nearest_material(search_from.x, search_from.y) else {
                break;
            };
            if !grid.is_machinable(machinable_mask, mx, my) {
                grid.clear_circle(mx, my, tool_radius);
                continue;
            }
            let start = P2::new(mx, my);
            let approach = match last_pos {
                None => Some(AdaptiveSegment::Rapid(start)),
                Some(prev) => {
                    let dx = start.x - prev.x;
                    let dy = start.y - prev.y;
                    let dist = (dx * dx + dy * dy).sqrt();
                    if dist < 1e-6 {
                        None
                    } else if is_clear_path(grid, machinable_mask, prev, start, tool_radius) {
                        // Path is over already-cleared cells — tool-down
                        // traverse is safe regardless of distance. Saves
                        // the retract + plunge cycle of a Rapid.
                        Some(AdaptiveSegment::Link(start))
                    } else if dist > max_link_dist {
                        Some(AdaptiveSegment::Rapid(start))
                    } else {
                        Some(AdaptiveSegment::Link(start))
                    }
                }
            };
            (start, approach)
        };
        patch_index += 1;
        let marker_at = segments.len();
        if link_load.holds_pass_load() {
            segments.push(AdaptiveSegment::Marker(AdaptiveRuntimeEvent::ResidueMop {
                patch_index,
            }));
        }
        if let Some(seg) = approach {
            segments.push(seg);
        }

        let mut path: Vec<P2> = vec![start];
        let mut cur = start;
        // True once this patch has emitted a hop link, so its approach
        // is no longer the last segment.
        let mut hopped = false;
        // The last capped walk heading (G-ADAPTPASSLOAD), for a smooth walk.
        let mut prev_heading: Option<f64> = None;
        let started_by_plunge = matches!(segments.last(), Some(AdaptiveSegment::Rapid(_)));
        let mut mop_departing = false;
        if link_load.holds_pass_load() && started_by_plunge {
            link_load.stamp_entry(grid, cur);
        }
        grid.clear_circle(cur.x, cur.y, tool_radius);

        for _ in 0..MAX_STEPS_PER_PATCH {
            let Some((mx, my)) = grid.find_nearest_material(cur.x, cur.y) else {
                break;
            };
            let dx = mx - cur.x;
            let dy = my - cur.y;
            let dist = (dx * dx + dy * dy).sqrt();
            // A chain hop (G-ADAPTLINKLOAD, operator ruling 2026-09-26):
            // the next material is more than one pass step beyond the
            // cutter, so the walk first crosses cleared cells. That
            // traverse is a reposition, not a cut: it ends at the last
            // pass-step position short of the material (the historical
            // chain's own positions), is admitted only under the keep-down
            // link rule, and is emitted as a `Link` so traces and the load
            // checks see it as one. The bite that follows is the walk's
            // ordinary step. A refused hop ends the chain; the next patch
            // retracts or links under the same rule.
            if link_load.rule == KeepDownLinks::WithinPassLoad
                && dist > tool_radius + step_len
                && dist <= max_link_dist
            {
                let k = ((dist - tool_radius - step_len) / step_len).ceil();
                let target = P2::new(
                    cur.x + k * step_len * dx / dist,
                    cur.y + k * step_len * dy / dist,
                );
                if !feed_link_within_pass_load(grid, machinable_mask, cur, target, link_load) {
                    break;
                }
                if path.len() >= 2 {
                    segments.push(AdaptiveSegment::Cut(std::mem::replace(
                        &mut path,
                        vec![target],
                    )));
                } else {
                    path = vec![target];
                }
                segments.push(AdaptiveSegment::Link(target));
                hopped = true;
                cur = target;
                continue;
            }
            // G-ADAPTPASSLOAD: the walk step toward the residue is a
            // capped direction search. It takes the heading nearest the
            // residue whose step cuts material within the pass load; when
            // every heading that cuts is over the load, the walk retreats
            // (the patch ends) and the next patch re-enters.
            if link_load.holds_pass_load() {
                let toward = dy.atan2(dx);
                // Out of a straight-plunge start the first steps are exempt
                // until the cutter leaves its plunge hole.
                let departing = started_by_plunge
                    && link_load.plunge_departure_exempt()
                    && (cur.x - start.x).hypot(cur.y - start.y) < tool_radius;
                let step_load = if departing {
                    link_load.pass.departing()
                } else {
                    link_load.pass
                };
                if mop_departing && !departing && path.len() >= 2 {
                    segments.push(AdaptiveSegment::Cut(std::mem::replace(
                        &mut path,
                        vec![cur],
                    )));
                }
                mop_departing = departing;
                let Some((next, heading)) = mop_capped_step(
                    grid,
                    machinable_mask,
                    cur,
                    toward,
                    prev_heading.unwrap_or(toward),
                    link_load,
                    &step_load,
                ) else {
                    break;
                };
                prev_heading = Some(heading);
                grid.clear_segment(cur, next, tool_radius);
                cur = next;
                path.push(cur);
                continue;
            }
            // Chain across short cleared gaps: stay tool-down and
            // walk to the next material as part of the same Cut,
            // rather than ending this Cut and starting a new one
            // with its own approach. Each Rapid costs a retract +
            // plunge cycle, which dominates over the cheap overlap
            // of walking through cleared cells. (Adaptive3d slices,
            // `RetractedByCaller`, keep this historical chain.)
            if dist > max_link_dist {
                break;
            }
            // Refuse the chain hop if it would cross outside the
            // machinable region (e.g. across a thin neck where
            // straight-line travel would clip a wall).
            if dist > tool_radius * 2.0
                && !is_clear_path(grid, machinable_mask, cur, P2::new(mx, my), tool_radius)
            {
                break;
            }
            if dist < 1e-9 {
                grid.clear_circle(cur.x, cur.y, tool_radius);
                break;
            }
            let step = step_len.min(dist).max(1e-9);
            let nx = cur.x + step * dx / dist;
            let ny = cur.y + step * dy / dist;
            if !grid.is_machinable(machinable_mask, nx, ny) {
                let nx2 = cur.x + (step * 0.5) * dx / dist;
                let ny2 = cur.y + (step * 0.5) * dy / dist;
                if grid.is_machinable(machinable_mask, nx2, ny2) {
                    cur = P2::new(nx2, ny2);
                } else {
                    grid.clear_circle(mx, my, tool_radius);
                    break;
                }
            } else {
                cur = P2::new(nx, ny);
            }
            path.push(cur);
            grid.clear_circle(cur.x, cur.y, tool_radius);
        }

        if path.len() >= 2 {
            segments.push(AdaptiveSegment::Cut(path));
            last_pos = Some(cur);
        } else if hopped {
            last_pos = Some(cur);
        } else if link_load.rule == KeepDownLinks::WithinPassLoad
            && matches!(
                segments.last(),
                Some(AdaptiveSegment::Link(_) | AdaptiveSegment::Rapid(_))
            )
        {
            // The approach is stamped on the grid (a link's walk, or the
            // plunge a rapid re-enters with): keep it, so the planner stock
            // stays the emitted path. G-ADAPTPASSLOAD: dropping a plunge
            // whose first step the load refused left a cutter disc marked
            // cut that no move cut, and a later step ran into it.
            last_pos = Some(cur);
        } else if link_load.rule == KeepDownLinks::WithinPassLoad {
            // Nothing cut and nothing stamped: drop the approach and the
            // patch marker.
            segments.truncate(marker_at);
        } else {
            segments.pop();
        }
        // A patch that removed nothing re-enters its cell by an entry next
        // time: the entry hole takes the cell, so the walk always advances.
        // A second start that removes nothing (the cutter cannot stand
        // where it touches the cell) gives the cell up: it stays stock.
        if grid.material_count == material_before_patch
            && let Some(i) = target_cell
        {
            if targets.plunge_only.get(i).copied().unwrap_or(false) {
                if let Some(flag) = targets.skip.get_mut(i) {
                    *flag = true;
                }
            } else if let Some(flag) = targets.plunge_only.get_mut(i) {
                *flag = true;
            }
        }
    }
    segments
}

/// The mop's per-cell start decisions under the 2D pass load.
struct MopTargets {
    /// Cells no start reaches: they stay stock, and the mop stops aiming at
    /// them.
    skip: Vec<bool>,
    /// Cells a keep-down start made no progress on: the next start there is
    /// an entry.
    plunge_only: Vec<bool>,
}

/// Stock cells the frontier hop tries before the pass gives up.
const FRONTIER_HOP_TRIES: usize = 32;

/// Directions around a stock cell the frontier hop tries for a stand-off:
/// the finest set whose neighbouring stand-offs (one pass step plus one
/// cell from the cell, at `R + cell`) lie at most a pass step apart:
/// `2 pi (R + cell) / step`, rounded up.
fn standoff_directions(load: &LinkLoad, cell: f64) -> usize {
    ((std::f64::consts::TAU * (load.tool_radius + cell)) / load.step_len).ceil() as usize
}

/// G-ADAPTPASSLOAD: where an agent pass that has no step within the load
/// carries on. The target is the stock cell nearest `from` not in `skip`;
/// the stand-off is a machinable point one cell outside the cutter's reach
/// of it, reached from `from` by a keep-down link that holds the pass load
/// (it runs over cut stock, and is stamped). Stand-offs are tried nearest
/// `from` first. A cell with none goes into `skip`; after
/// [`FRONTIER_HOP_TRIES`] cells the pass ends. Returns the stand-off and
/// the target cell's position.
fn frontier_hop(
    grid: &mut MaterialGrid,
    mask: &[bool],
    from: P2,
    load: &LinkLoad,
    skip: &mut [bool],
) -> Option<(P2, (f64, f64))> {
    let cell = grid.cell_size;
    let reach = load.tool_radius + cell;
    let n = standoff_directions(load, cell);
    for _ in 0..FRONTIER_HOP_TRIES {
        let (mx, my) = grid.find_nearest_material_where(from.x, from.y, |i| {
            !skip.get(i).copied().unwrap_or(true)
        })?;
        let mut stands: Vec<P2> = (0..n)
            .map(|k| {
                let a = std::f64::consts::TAU * k as f64 / n as f64;
                P2::new(mx + reach * a.cos(), my + reach * a.sin())
            })
            .filter(|p| load.legal(grid, mask, *p))
            .collect();
        stands.sort_by(|a, b| {
            let da = (a.x - from.x).hypot(a.y - from.y);
            let db = (b.x - from.x).hypot(b.y - from.y);
            da.total_cmp(&db)
        });
        for p in stands {
            if (p.x - from.x).hypot(p.y - from.y) < cell {
                continue;
            }
            if feed_link_within_pass_load(grid, mask, from, p, load) {
                return Some((p, (mx, my)));
            }
        }
        *skip.get_mut(grid.cell_index(mx, my)?)? = true;
    }
    None
}

/// Where the next mop patch starts under the 2D pass load
/// (G-ADAPTPASSLOAD), and how the cutter gets there. The target is the
/// residue cell nearest `from` that is not in `skip`. The cutter stands
/// where it can touch that cell (the cell, or the machinable lattice point
/// nearest it within R), reached by a keep-down link that holds the pass
/// load; else it re-enters by a retract and an entry at the cell, or at the
/// nearest point whose entry hole (R plus the helix) reaches the cell and
/// whose helix stays inside the part. A cell with neither is added to
/// `skip` (it stays stock) and the next is tried. `None` when no residue
/// cell is left to try.
fn mop_patch_start(
    grid: &mut MaterialGrid,
    mask: &[bool],
    from: P2,
    last_pos: Option<P2>,
    load: &LinkLoad,
    targets: &mut MopTargets,
) -> Option<(P2, Option<AdaptiveSegment>, usize)> {
    loop {
        let skip = &targets.skip;
        let (mx, my) = grid.find_nearest_material_where(from.x, from.y, |i| {
            !skip.get(i).copied().unwrap_or(true)
        })?;
        let cell = grid.cell_index(mx, my)?;
        let plunge_only = targets.plunge_only.get(cell).copied().unwrap_or(false);
        let stand = if grid.is_machinable(mask, mx, my) {
            Some(P2::new(mx, my))
        } else {
            grid.nearest_machinable_within(mask, mx, my, load.tool_radius)
        };
        if let (Some(prev), Some(s)) = (last_pos, stand)
            && !plunge_only
        {
            if (s.x - prev.x).hypot(s.y - prev.y) < 1e-6 {
                return Some((s, None, cell));
            }
            // First choice: a stand-off one cell outside the cutter's reach
            // of the cell, on the side the cutter comes from, reached by a
            // link through cleared stock; the walk then bites from there.
            let (ux, uy) = (prev.x - mx, prev.y - my);
            let d = ux.hypot(uy);
            if d > load.tool_radius + grid.cell_size {
                let k = (load.tool_radius + grid.cell_size) / d;
                let off = P2::new(mx + ux * k, my + uy * k);
                if load.legal(grid, mask, off)
                    && feed_link_within_pass_load(grid, mask, prev, off, load)
                {
                    return Some((off, Some(AdaptiveSegment::Link(off)), cell));
                }
            }
            if feed_link_within_pass_load(grid, mask, prev, s, load) {
                return Some((s, Some(AdaptiveSegment::Link(s)), cell));
            }
        }
        // An entry needs room for a helix (a contained helix of radius 0 is
        // a ramp, whose geometry the planner does not model): the nearest
        // point touching the cell whose helix fits.
        if let Some(p) = load.helix_entry_near(grid, mask, mx, my) {
            return Some((p, Some(AdaptiveSegment::Rapid(p)), cell));
        }
        *targets.skip.get_mut(cell)? = true;
    }
}

/// Angular resolution of the mop's capped direction search.
const MOP_HEADING_STEP_RAD: f64 = std::f64::consts::PI / 36.0;

/// The mop's capped walk step (G-ADAPTPASSLOAD): a direction search from
/// `cur` over headings every [`MOP_HEADING_STEP_RAD`]. A heading qualifies
/// when its pass step stays machinable, cuts material and holds
/// [`step_within_pass_load`]; among those the step reading nearest the
/// band target wins, with small penalties for turning away from the last
/// heading (a smooth walk, which the segment merge and arc fit downstream
/// do not cut corners on) and from `toward` (the nearest residue). `None`
/// when no heading qualifies: the walk retreats.
fn mop_capped_step(
    grid: &MaterialGrid,
    mask: &[bool],
    cur: P2,
    toward: f64,
    prev: f64,
    load: &LinkLoad,
    pass: &PassLoad,
) -> Option<(P2, f64)> {
    let n = (std::f64::consts::TAU / MOP_HEADING_STEP_RAD).round() as i32;
    // A full pass step first; a half step where every full one overloads
    // (a shorter step takes a thinner bite off a wall it meets head-on).
    for len in [load.step_len, load.step_len * 0.5] {
        if let Some(found) = mop_capped_step_of(grid, mask, cur, toward, prev, load, pass, len, n) {
            return Some(found);
        }
    }
    None
}

// SAFETY: a private helper; the arguments are the search state.
#[allow(clippy::too_many_arguments)]
fn mop_capped_step_of(
    grid: &MaterialGrid,
    mask: &[bool],
    cur: P2,
    toward: f64,
    prev: f64,
    load: &LinkLoad,
    pass: &PassLoad,
    len: f64,
    n: i32,
) -> Option<(P2, f64)> {
    let mut best: Option<(f64, P2, f64)> = None;
    for k in 0..n {
        let angle = toward + f64::from(k) * MOP_HEADING_STEP_RAD;
        let next = P2::new(cur.x + len * angle.cos(), cur.y + len * angle.sin());
        if !load.legal_step(grid, mask, cur, next) {
            continue;
        }
        let w = super::search::measure_engagement(
            grid,
            next.x,
            next.y,
            load.tool_radius,
            angle,
            pass.measure,
        );
        if w < pass.presence
            || !super::search::step_within_pass_load(
                grid,
                next.x,
                next.y,
                load.tool_radius,
                angle,
                pass,
            )
        {
            continue;
        }
        // The agent's own scoring: band error plus its heading-change
        // weight (`search::HEADING_CHANGE_WEIGHT`); no mop-specific weight.
        let score = (w - pass.target).abs()
            + super::search::HEADING_CHANGE_WEIGHT * super::angle_diff(angle, prev).abs()
                / std::f64::consts::PI;
        if best.is_none_or(|(b, _, _)| score < b) {
            best = Some((score, next, angle));
        }
    }
    best.map(|(_, p, a)| (p, a))
}

// ── Narrow-region contour-parallel strategy ────────────────────────────
//
// When the largest inscribed disk inside the machinable mask is small
// relative to the stepover (≤ 3 × stepover by default), the engagement-
// target spiral has no room to settle and produces a per-step sawtooth.
// For those regions, the planner emits concentric inward offsets of the
// machinable polygon and walks each contour as one continuous Cut. The
// final residue mop still runs on the output to catch any leftover
// strip between concentric loops. See `CleanupStrategy::ContourParallelNarrow`.

/// True when the machinable region is too narrow for the engagement-
/// target spiral to settle: the largest inscribed disk fits within
/// 2 × stepover. Implemented by checking that an inward offset of
/// 2 × stepover collapses the machinable region to empty.
///
/// We use a Euclidean offset (via `offset_polygon`) rather than the
/// Manhattan-grid distance transform on `boundary_distances` because
/// the Manhattan metric over-estimates depth at corners (e.g. a donut
/// ring corner reads ~12 mm Manhattan but ~8 mm Euclidean), which
/// matters for the gate threshold.
/// Emit a circular starter pocket centred on the largest inscribed
/// disk inside `machinable`, returning the cutter end position so the
/// engagement-target spiral can continue from there with full swing
/// room from move 1.
///
/// The cutter walks a circle of radius `tool_radius` around the
/// medial-axis maximum, clearing a disc of radius `2 × tool_radius`.
/// In 3D production this would be a helical plunge; for the 2D
/// planner it's a single circular pass after a Rapid + Z-plunge.
///
/// Returns `None` when no DT-maximum cell with ≥ `2 × tool_radius`
/// clearance exists (uniformly-narrow regions — the narrow gate
/// would already have handled those).
///
/// Reference: Autodesk patent US7831332 (Fusion HSM transition
/// portion), Bieterman & Sandström (Boeing, ~2003), Ren & Bi (2014).
#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
fn emit_helical_starter_pocket(
    grid: &mut MaterialGrid,
    machinable_mask: &[bool],
    boundary_distances: &[f64],
    tool_radius: f64,
    pass_load: Option<(&PassLoad, f64, &LinkLoad)>,
) -> Option<(Vec<AdaptiveSegment>, P2)> {
    let medial =
        find_entry_via_distance_transform(grid, machinable_mask, boundary_distances, tool_radius)?;
    // Need the medial-axis disk to fit the helix (radius `tool_radius`)
    // plus the cutter (radius `tool_radius`) plus a small safety margin.
    let dt_at = grid.boundary_distance_at(boundary_distances, medial.x, medial.y);
    let required = 2.0 * tool_radius;
    if dt_at < required {
        return None;
    }

    if let Some((load, stepover, link_load)) = pass_load {
        let entry_radius = link_load.entry_hole(medial);
        return emit_spiral_starter_pocket(grid, medial, tool_radius, stepover, load, entry_radius);
    }

    let helix_r = tool_radius;
    let n_steps = 64;
    let mut segments: Vec<AdaptiveSegment> = Vec::new();

    // Clear the medial cell itself (the Z-plunge centre).
    grid.clear_circle(medial.x, medial.y, tool_radius);

    let start = P2::new(medial.x + helix_r, medial.y);
    segments.push(AdaptiveSegment::Rapid(start));
    grid.clear_circle(start.x, start.y, tool_radius);

    let mut path: Vec<P2> = vec![start];
    for i in 1..=n_steps {
        let theta = (i as f64 / n_steps as f64) * std::f64::consts::TAU;
        let x = medial.x + helix_r * theta.cos();
        let y = medial.y + helix_r * theta.sin();
        grid.clear_circle(x, y, tool_radius);
        path.push(P2::new(x, y));
    }
    let end = *path.last()?;
    segments.push(AdaptiveSegment::Cut(path));
    Some((segments, end))
}

/// Angular steps per turn of the starter pocket (the historical helix's).
const STARTER_STEPS_PER_TURN: usize = 64;

/// The cutter-centre path of the 2D starter pocket: from the plunge at
/// `medial`, an Archimedean spiral `r = pitch · θ / 2π` out to `r = R`, then
/// one full lap at `R`. It clears the same `2R` disc the historical circle
/// did, but each lap bites at most `pitch` sideways instead of a full slot.
fn starter_spiral_points(medial: P2, tool_radius: f64, pitch: f64) -> Vec<P2> {
    let dtheta = std::f64::consts::TAU / STARTER_STEPS_PER_TURN as f64;
    let theta_end = std::f64::consts::TAU * tool_radius / pitch;
    let at = |r: f64, theta: f64| P2::new(medial.x + r * theta.cos(), medial.y + r * theta.sin());
    let mut pts = vec![medial];
    let mut i = 1usize;
    loop {
        let theta = i as f64 * dtheta;
        if theta >= theta_end {
            break;
        }
        pts.push(at(pitch * theta / std::f64::consts::TAU, theta));
        i += 1;
    }
    for k in 0..=STARTER_STEPS_PER_TURN {
        pts.push(at(tool_radius, theta_end + k as f64 * dtheta));
    }
    pts
}

/// G-ADAPTPASSLOAD: the 2D starter pocket. The historical circle of radius
/// R around the plunge ran a full slot (the sim read 0.83). Plunge at the
/// medial point (the disc the grid clears there), then spiral out
/// ([`starter_spiral_points`]) at pitch `min(stepover, R)`, halved up to
/// three times until every step holds [`step_within_pass_load`]. `None`
/// when no pitch does; the agent then enters from the boundary.
fn emit_spiral_starter_pocket(
    grid: &mut MaterialGrid,
    medial: P2,
    tool_radius: f64,
    stepover: f64,
    load: &PassLoad,
    entry_radius: f64,
) -> Option<(Vec<AdaptiveSegment>, P2)> {
    let mut pitch = stepover.min(tool_radius);
    for _ in 0..4 {
        let pts = starter_spiral_points(medial, tool_radius, pitch);
        let mut log = Vec::new();
        grid.clear_circle_logged(medial.x, medial.y, entry_radius.max(tool_radius), &mut log);
        let mut holds = true;
        for w in pts.windows(2) {
            let &[a, b] = w else {
                continue;
            };
            let angle = (b.y - a.y).atan2(b.x - a.x);
            // A straight plunge (no helix hole): the steps inside the
            // plunge hole are exempt (`PassLoad::departing`).
            let step_load = if entry_radius <= tool_radius
                && (b.x - medial.x).hypot(b.y - medial.y) < tool_radius
            {
                load.departing()
            } else {
                *load
            };
            if !step_within_pass_load(grid, b.x, b.y, tool_radius, angle, &step_load) {
                holds = false;
                break;
            }
            grid.clear_segment_logged(a, b, tool_radius, &mut log);
        }
        if holds {
            let end = *pts.last()?;
            let segments = vec![
                AdaptiveSegment::Marker(AdaptiveRuntimeEvent::StarterPocket {
                    center_x: medial.x,
                    center_y: medial.y,
                }),
                AdaptiveSegment::Rapid(medial),
                AdaptiveSegment::Cut(pts),
            ];
            return Some((segments, end));
        }
        grid.restore_cleared(&log);
        pitch *= 0.5;
    }
    None
}

fn is_narrow_machinable(machinable: &Polygon2, tool_radius: f64, stepover: f64) -> bool {
    // Probe radius: tool_radius + stepover. If the machinable region
    // inset by this much collapses to nothing or only tiny fragments,
    // the engagement-target spiral has no room to settle. A "tiny
    // fragment" is one with area < 2 × (cutter footprint). Donut-
    // topology rings break into corner residues well below this
    // threshold; convex pockets (square / rect / circle / L) produce
    // a single large fragment that exceeds it.
    let probe = tool_radius + stepover;
    let result = offset_polygon(machinable, probe);
    if result.is_empty() {
        return true;
    }
    let cutter_area = std::f64::consts::PI * tool_radius * tool_radius;
    let min_viable_area = 2.0 * cutter_area;
    let max_fragment_area = result.iter().map(|p| p.area()).fold(0.0_f64, f64::max);
    max_fragment_area < min_viable_area
}

/// Emit concentric inward-offset loops of `machinable` at stride `stepover`,
/// clearing the grid along each contour. Contours that pass through no
/// remaining material are skipped (no emission) — so a fresh-grid call
/// emits every loop, but a post-spiral call emits only the loops that
/// would actually remove residue. Adjacent emitted loops are connected
/// by Link when within 6R via a clear path, else by Rapid.
#[allow(clippy::too_many_arguments)]
fn contour_parallel_segments(
    machinable: &Polygon2,
    grid: &mut MaterialGrid,
    machinable_mask: &[bool],
    stepover: f64,
    cell_size: f64,
    link_load: &LinkLoad,
    cancel: &dyn CancelCheck,
    start_pos: Option<P2>,
) -> Result<Vec<AdaptiveSegment>, Cancelled> {
    const MAX_LOOPS: usize = 120;
    const RESIDUE_DONE_FRACTION: f64 = 0.001;
    // Overlap factor < 1.0 makes consecutive offset loops overlap so
    // thin rings between the spiral's outermost reach and the boundary
    // band don't survive as uncleared islands. 0.85 → 15% overlap.
    const OFFSET_OVERLAP: f64 = 0.85;
    let tool_radius = link_load.tool_radius;
    let max_link_dist = tool_radius * 6.0;
    let mut segments: Vec<AdaptiveSegment> = Vec::new();
    let mut last_pos: Option<P2> = start_pos;
    for k in 0..MAX_LOOPS {
        check_cancel(cancel)?;
        if grid.material_fraction() < RESIDUE_DONE_FRACTION {
            break;
        }
        let dist = stepover * OFFSET_OVERLAP * (k as f64);
        let polys: Vec<Polygon2> = if dist <= 1e-9 {
            vec![machinable.clone()]
        } else {
            offset_polygon(machinable, dist)
        };
        if polys.is_empty() {
            break;
        }

        let material_before = grid.material_count;
        for poly in &polys {
            let mut contours: Vec<&Vec<P2>> = Vec::new();
            if poly.exterior.len() >= 3 {
                contours.push(&poly.exterior);
            }
            for hole in &poly.holes {
                if hole.len() >= 3 {
                    contours.push(hole);
                }
            }

            for contour in contours {
                if !contour_passes_material(contour, grid) {
                    continue;
                }
                // Begin walking each closed loop at the vertex nearest the
                // cutter's previous exit, so the inter-loop hop is as short
                // as possible. Same cells cleared (identical closed path,
                // different start vertex) — this only shrinks the link/rapid
                // travel between concentric offset loops.
                let walk_owned: Vec<P2>;
                let walk: &[P2] = match last_pos {
                    Some(prev) => {
                        walk_owned = rotate_contour_to_nearest(contour, prev);
                        &walk_owned
                    }
                    None => contour,
                };
                if link_load.holds_pass_load() {
                    // G-ADAPTPASSLOAD: the loop is cut only where each step
                    // holds the pass load; an over-load span is left for
                    // the mop, and the loop is re-entered past it by a
                    // link that holds the load or a retract.
                    let marker_at = segments.len();
                    segments.push(AdaptiveSegment::Marker(
                        AdaptiveRuntimeEvent::ResidueContour { offset_index: k },
                    ));
                    let path = contour_walk_points(walk, cell_size);
                    if emit_capped_walk(
                        &path,
                        grid,
                        machinable_mask,
                        link_load,
                        &mut last_pos,
                        &mut segments,
                    ) == 0
                    {
                        segments.truncate(marker_at);
                    }
                    continue;
                }
                // The historical rule (Adaptive3d slices) reads the corridor
                // after the loop's own clearing.
                let path = walk_contour_clearing(walk, cell_size, grid, tool_radius);
                if path.len() < 2 {
                    continue;
                }
                #[allow(clippy::indexing_slicing)] // path.len() >= 2 checked above
                let entry = path[0];
                #[allow(clippy::expect_used)]
                let end = *path.last().expect("path non-empty");

                match last_pos {
                    None => segments.push(AdaptiveSegment::Rapid(entry)),
                    Some(prev) => {
                        let dx = entry.x - prev.x;
                        let dy = entry.y - prev.y;
                        let d = (dx * dx + dy * dy).sqrt();
                        if d < 1e-6 {
                            // already at entry — no approach needed
                        } else if is_clear_path(grid, machinable_mask, prev, entry, tool_radius) {
                            // Cleared-cell traverse — Link at any
                            // distance, saving the retract + plunge
                            // cycle of a Rapid.
                            segments.push(AdaptiveSegment::Link(entry));
                        } else if d < max_link_dist {
                            segments.push(AdaptiveSegment::Link(entry));
                        } else {
                            segments.push(AdaptiveSegment::Rapid(entry));
                        }
                    }
                }
                segments.push(AdaptiveSegment::Cut(path));
                last_pos = Some(end);
            }
        }

        // If a full pass at this offset couldn't reduce material, stop
        // (further offsets would just spin). Under the 2D pass load an
        // outer offset can refuse all of its over-load residue while the
        // inner offsets, nearer the cleared core, still cut: keep going
        // until the offsets run out.
        if !link_load.holds_pass_load() && grid.material_count >= material_before {
            break;
        }
    }

    Ok(segments)
}

/// True when a meaningful fraction of the contour lies over uncleared
/// (material) cells. Samples ~128 points; emits if ≥ 25% are material.
///
/// A simple "any material" filter over-emits: the spiral leaves thin
/// wiggle-slivers between passes, so every inward offset contour
/// crosses *some* material at a sliver and walks the whole loop just
/// to clear a 5%-coverage band. The fraction threshold ignores these
/// tracer-sliver intersections and emits only when the contour covers
/// substantial residue — typically the outermost band where the
/// spiral didn't reach, or a residue ring in a narrow strip.
fn contour_passes_material(contour: &[P2], grid: &MaterialGrid) -> bool {
    const COVERAGE_THRESHOLD: f64 = 0.25;
    if contour.len() < 3 {
        return false;
    }
    let n_samples = 128.min(contour.len());
    let stride = (contour.len() / n_samples).max(1);
    let mut hits = 0usize;
    let mut total = 0usize;
    let mut i = 0;
    while i < contour.len() {
        #[allow(clippy::indexing_slicing)] // i < contour.len() bounded above
        let p = contour[i];
        if grid.is_material(p.x, p.y) {
            hits += 1;
        }
        total += 1;
        i += stride;
    }
    total > 0 && (hits as f64 / total as f64) >= COVERAGE_THRESHOLD
}

/// Rotate a closed contour's vertex order so the vertex nearest to
/// `target` becomes index 0. The loop covers the same cells regardless of
/// where it starts, so this is a pure travel optimisation: walking begins
/// at the point closest to the cutter's previous position, minimising the
/// link/rapid hop between consecutive concentric offset loops.
///
/// A trailing duplicate-of-first closing vertex (some offset outputs carry
/// one) is dropped first so it can't land mid-loop as a zero-length edge.
fn rotate_contour_to_nearest(contour: &[P2], target: P2) -> Vec<P2> {
    let mut verts = contour;
    if verts.len() >= 2 {
        #[allow(clippy::indexing_slicing)] // len >= 2 checked
        let (first, last) = (verts[0], verts[verts.len() - 1]);
        let dx = first.x - last.x;
        let dy = first.y - last.y;
        if dx * dx + dy * dy < 1e-18 {
            #[allow(clippy::indexing_slicing)] // len >= 2 checked
            let trimmed = &contour[..contour.len() - 1];
            verts = trimmed;
        }
    }
    if verts.len() < 2 {
        return verts.to_vec();
    }
    let mut best_idx = 0usize;
    let mut best_d2 = f64::INFINITY;
    for (i, p) in verts.iter().enumerate() {
        let dx = p.x - target.x;
        let dy = p.y - target.y;
        let d2 = dx * dx + dy * dy;
        if d2 < best_d2 {
            best_d2 = d2;
            best_idx = i;
        }
    }
    #[allow(clippy::indexing_slicing)] // best_idx in 0..verts.len()
    let mut rotated = verts[best_idx..].to_vec();
    #[allow(clippy::indexing_slicing)] // best_idx in 0..verts.len()
    rotated.extend_from_slice(&verts[..best_idx]);
    rotated
}

/// Walk a closed contour in world coords, subdividing each edge to
/// `cell_size * 1.5` and clearing the grid at every step. Returns the
/// emitted Cut path (start point repeated at the end to close the loop).
fn walk_contour_clearing(
    contour: &[P2],
    cell_size: f64,
    grid: &mut MaterialGrid,
    tool_radius: f64,
) -> Vec<P2> {
    let path = contour_walk_points(contour, cell_size);
    for p in &path {
        grid.clear_circle(p.x, p.y, tool_radius);
    }
    path
}

/// The points [`walk_contour_clearing`] visits, without touching a grid.
fn contour_walk_points(contour: &[P2], cell_size: f64) -> Vec<P2> {
    let mut path = Vec::new();
    if contour.len() < 2 {
        return path;
    }
    #[allow(clippy::indexing_slicing)] // contour.len() >= 2 checked above
    let start = contour[0];
    path.push(start);

    let n = contour.len();
    for i in 0..n {
        #[allow(clippy::indexing_slicing)] // i, (i+1)%n bounded by contour len
        let a = contour[i];
        #[allow(clippy::indexing_slicing)]
        let b = contour[(i + 1) % n];
        let dx = b.x - a.x;
        let dy = b.y - a.y;
        let len = (dx * dx + dy * dy).sqrt();
        let n_steps = (len / (cell_size * 1.5)).ceil() as usize;
        let steps = n_steps.max(1);
        for j in 1..=steps {
            let t = j as f64 / steps as f64;
            let x = a.x + t * dx;
            let y = a.y + t * dy;
            path.push(P2::new(x, y));
        }
    }
    path
}

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// Simplify a path using the Douglas-Peucker algorithm.
pub(crate) fn simplify_path(points: &[P2], tolerance: f64) -> Vec<P2> {
    if points.len() <= 2 {
        return points.to_vec();
    }

    // Find the point farthest from the line between first and last
    let first = points[0];
    let last = points[points.len() - 1];
    let dx = last.x - first.x;
    let dy = last.y - first.y;
    let line_len = (dx * dx + dy * dy).sqrt();

    let mut max_dist = 0.0;
    let mut max_idx = 0;

    if line_len > 1e-10 {
        for (i, pt) in points.iter().enumerate().take(points.len() - 1).skip(1) {
            let d = ((pt.x - first.x) * dy - (pt.y - first.y) * dx).abs() / line_len;
            if d > max_dist {
                max_dist = d;
                max_idx = i;
            }
        }
    } else {
        // Degenerate case: all points are close together
        for (i, pt) in points.iter().enumerate().take(points.len() - 1).skip(1) {
            let ddx = pt.x - first.x;
            let ddy = pt.y - first.y;
            let d = (ddx * ddx + ddy * ddy).sqrt();
            if d > max_dist {
                max_dist = d;
                max_idx = i;
            }
        }
    }

    if max_dist > tolerance {
        let mut left = simplify_path(&points[..=max_idx], tolerance);
        let right = simplify_path(&points[max_idx..], tolerance);
        left.pop(); // Remove duplicate junction point
        left.extend(right);
        left
    } else {
        vec![first, last]
    }
}

/// Douglas-Peucker like [`simplify_path`], but a point's deviation is its
/// distance to the chord SEGMENT, not to the chord's infinite line
/// (G-ADAPTPASSLOAD). A walk that doubles back (a mop spike, a reversal)
/// puts its tip on the chord's line beyond an end point: the line distance
/// reads 0 and drops the tip, so the planner has stamped stock the emitted
/// path never cuts, and a later step walks into it at full width.
///
/// Under the 2D rule a chord is kept only when it stays inside the tool-
/// centre `region` too: a chord within tolerance of a walk that hugs an
/// island's inset arc still cuts the island (measured 0.081 mm on the
/// six-island pocket, round 3).
pub(crate) fn simplify_path_keeping_reversals(
    points: &[P2],
    tolerance: f64,
    region: Option<&ToolCentreRegion>,
) -> Vec<P2> {
    if points.len() <= 2 {
        return points.to_vec();
    }
    let (Some(&first), Some(&last)) = (points.first(), points.last()) else {
        return points.to_vec();
    };
    let (dx, dy) = (last.x - first.x, last.y - first.y);
    let len_sq = dx * dx + dy * dy;
    let mut max_dist = 0.0;
    let mut max_idx = 0;
    for (i, pt) in points.iter().enumerate().take(points.len() - 1).skip(1) {
        let t = if len_sq > 1e-20 {
            (((pt.x - first.x) * dx + (pt.y - first.y) * dy) / len_sq).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let d = (pt.x - (first.x + t * dx)).hypot(pt.y - (first.y + t * dy));
        if d > max_dist {
            max_dist = d;
            max_idx = i;
        }
    }
    let leaves_region = max_dist <= tolerance
        && region.is_some_and(|r| {
            r.crossed_by(first, last)
                || !r.contains(&P2::new(0.5 * (first.x + last.x), 0.5 * (first.y + last.y)))
        });
    if leaves_region && max_idx == 0 {
        // Every inner point is on the chord: split at the middle one.
        max_idx = points.len() / 2;
    }
    if max_dist > tolerance || leaves_region {
        // SAFETY: 0 < max_idx < points.len() - 1, both slices are in bounds.
        #[allow(clippy::indexing_slicing)]
        let mut left = simplify_path_keeping_reversals(&points[..=max_idx], tolerance, region);
        // SAFETY: 0 < max_idx < points.len() - 1, both slices are in bounds.
        #[allow(clippy::indexing_slicing)]
        let right = simplify_path_keeping_reversals(&points[max_idx..], tolerance, region);
        left.pop();
        left.extend(right);
        left
    } else {
        vec![first, last]
    }
}

/// Phase 3: default contour-spiral corner-blend radius as a fraction of the
/// tool radius when the user leaves `min_cutting_radius` at 0. Kept well below
/// 1.0 so the rounded centerline stays inside the tool's own corner fillet.
const SPIRAL_DEFAULT_MIN_CUTTING_RADIUS_FACTOR: f64 = 0.3;

pub(super) fn segments_to_toolpath(
    segments: &[AdaptiveSegment],
    params: &AdaptiveParams,
    region: Option<&ToolCentreRegion>,
) -> (Toolpath, Vec<AdaptiveRuntimeAnnotation>) {
    let mut tp = Toolpath::new();
    let mut annotations = Vec::new();

    // Phase 3 (accel-friendly): the contour spiral's tight inner-wrap reversals
    // otherwise force the controller to a near-stop (junction velocity √(A·R)→0
    // as the corner radius → 0). When the user hasn't pinned a corner radius,
    // default the spiral to rounding centerline corners at 0.3× the tool radius
    // — well inside the tool's own fillet, so it removes no extra material — and
    // emit them as native G2/G3 (`blend_corners_to_moves`). Other strategies and
    // an explicit user value are untouched.
    let effective_min_cutting_radius = if params.path_strategy
        == crate::adaptive::PathStrategy2d::ContourSpiral
        && params.min_cutting_radius <= 0.0
    {
        SPIRAL_DEFAULT_MIN_CUTTING_RADIUS_FACTOR * params.tool_radius
    } else {
        params.min_cutting_radius
    };

    for segment in segments {
        match segment {
            AdaptiveSegment::Marker(event) => {
                annotations.push(AdaptiveRuntimeAnnotation {
                    move_index: tp.moves.len(),
                    event: event.clone(),
                });
            }
            AdaptiveSegment::Rapid(entry) => {
                // Retract straight up before any XY travel: a rapid that
                // left cut depth on a diagonal dragged the cutter through
                // the walls and islands standing beside it.
                tp.final_retract(params.safe_z);
                tp.rapid_to_with_intent(
                    crate::geo::P3::new(entry.x, entry.y, params.safe_z),
                    crate::toolpath::MoveIntent::Linking,
                );
                tp.feed_to_with_intent(
                    crate::geo::P3::new(entry.x, entry.y, params.cut_depth),
                    params.plunge_rate,
                    crate::toolpath::MoveIntent::EntryPlunge,
                );
            }
            AdaptiveSegment::Link(entry) => {
                tp.feed_to_with_intent(
                    crate::geo::P3::new(entry.x, entry.y, params.cut_depth),
                    params.feed_rate,
                    crate::toolpath::MoveIntent::Linking,
                );
            }
            AdaptiveSegment::Cut(path) => {
                let simplified = if params.keep_down_links == KeepDownLinks::WithinPassLoad {
                    // Under the 2D rule the emitted cut is the walked cut: the
                    // planner stamped every step, so a chord within the
                    // operator's tolerance would leave a sliver up to that
                    // tolerance thick that the planner holds cut, and a
                    // later step would cross it at its full chord (read
                    // 0.65 of D in the simulation on the six-island pocket
                    // with straight-plunge entries, round 3). Only points
                    // on the walked line are dropped: the tolerance is the
                    // shortest move the toolpath emits.
                    simplify_path_keeping_reversals(
                        path,
                        crate::toolpath::MIN_EMITTED_SEGMENT_MM,
                        region,
                    )
                } else {
                    simplify_path(path, params.tolerance)
                };
                if effective_min_cutting_radius > 0.0 {
                    let moves = blend_corners_to_moves(&simplified, effective_min_cutting_radius);
                    for m in moves.iter().skip(1) {
                        match m {
                            BlendedMove::Linear(p) => {
                                tp.feed_to_with_intent(
                                    crate::geo::P3::new(p.x, p.y, params.cut_depth),
                                    params.feed_rate,
                                    crate::toolpath::MoveIntent::ClearingCut,
                                );
                            }
                            BlendedMove::Arc {
                                end,
                                center,
                                clockwise,
                            } => {
                                // SAFETY: Cut always follows Plunge/Link, so tp.moves
                                // is non-empty; unwrap_or is a defensive fallback.
                                let prev =
                                    tp.moves.last().map(|mv| mv.target).unwrap_or(
                                        crate::geo::P3::new(end.x, end.y, params.cut_depth),
                                    );
                                let i = center.x - prev.x;
                                let j = center.y - prev.y;
                                let target = crate::geo::P3::new(end.x, end.y, params.cut_depth);
                                if *clockwise {
                                    tp.arc_cw_to_with_intent(
                                        target,
                                        i,
                                        j,
                                        params.feed_rate,
                                        crate::toolpath::MoveIntent::ClearingCut,
                                    );
                                } else {
                                    tp.arc_ccw_to_with_intent(
                                        target,
                                        i,
                                        j,
                                        params.feed_rate,
                                        crate::toolpath::MoveIntent::ClearingCut,
                                    );
                                }
                            }
                        }
                    }
                } else {
                    for p in simplified.iter().skip(1) {
                        tp.feed_to_with_intent(
                            crate::geo::P3::new(p.x, p.y, params.cut_depth),
                            params.feed_rate,
                            crate::toolpath::MoveIntent::ClearingCut,
                        );
                    }
                }
            }
        }
    }

    if let Some(last) = tp.moves.last() {
        tp.rapid_to_with_intent(
            crate::geo::P3::new(last.target.x, last.target.y, params.safe_z),
            crate::toolpath::MoveIntent::Retract,
        );
    }

    (tp, annotations)
}

impl crate::compute::spans::RuntimeLabel for AdaptiveRuntimeAnnotation {
    fn move_index(&self) -> usize {
        self.move_index
    }

    fn label(&self) -> String {
        self.event.label()
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod link_load_tests {
    //! G-ADAPTLINKLOAD: a keep-down link is a pass-load cut or nothing.
    use super::{AdaptiveSegment, LinkLoad, feed_link_within_pass_load, mop_residue_into_segments};
    use crate::adaptive::search::compute_engagement;
    use crate::adaptive::{KeepDownLinks, MaterialGrid};
    use crate::geo::P2;
    use crate::polygon::{Polygon2, offset_polygon};

    const R: f64 = 3.0;

    fn scene() -> (MaterialGrid, Vec<bool>, LinkLoad) {
        let square = Polygon2::rectangle(-30.0, -30.0, 30.0, 30.0);
        let grid = MaterialGrid::from_polygon(&square, R / 6.0);
        let machinable = offset_polygon(&square, R);
        let mask = MaterialGrid::build_machinable_mask(
            &machinable[0],
            grid.origin_x,
            grid.origin_y,
            grid.rows,
            grid.cols,
            grid.cell_size,
        );
        let load = LinkLoad {
            rule: KeepDownLinks::WithinPassLoad,
            tool_radius: R,
            step_len: grid.cell_size * 3.0,
            pass: crate::adaptive::search::PassLoad::swept_width(2.0, R, R / 6.0),
            entry_radius: R,
            entry_mask: None,
            rewalk_replayed_cuts: true,
            region: None,
        };
        (grid, mask, load)
    }

    /// A link through standing stock is a slot: refused, and the grid is
    /// left exactly as it was.
    #[test]
    fn a_slot_through_stock_is_refused_and_leaves_the_grid() {
        let (mut grid, mask, load) = scene();
        grid.clear_circle(-10.0, 0.0, R);
        let before = grid.cells.clone();
        let count = grid.material_count;
        assert!(!feed_link_within_pass_load(
            &mut grid,
            &mask,
            P2::new(-10.0, 0.0),
            P2::new(10.0, 0.0),
            &load
        ));
        assert_eq!(grid.cells, before);
        assert_eq!(grid.material_count, count);
    }

    /// A link along a cleared corridor reads no engagement and is admitted.
    #[test]
    fn a_link_through_a_cleared_corridor_is_admitted() {
        let (mut grid, mask, load) = scene();
        for i in -40..=40 {
            grid.clear_circle(f64::from(i) * 0.5, 0.0, R);
        }
        assert!(feed_link_within_pass_load(
            &mut grid,
            &mask,
            P2::new(-15.0, 0.0),
            P2::new(15.0, 0.0),
            &load
        ));
    }

    /// A link that skims a wall below the pass stepover is a legitimate
    /// cut: admitted, and stamped so the next pass sees it cut.
    #[test]
    fn a_link_below_the_pass_stepover_is_admitted_and_stamped() {
        let (mut grid, mask, load) = scene();
        // A corridor cleared to |y| <= R; the link runs 1.5 mm above its
        // axis, so it takes 1.5 mm (< the 2 mm stepover) of the wall.
        for i in -40..=40 {
            grid.clear_circle(f64::from(i) * 0.5, 0.0, R);
        }
        let (from, to) = (P2::new(-15.0, 1.5), P2::new(15.0, 1.5));
        assert!(grid.is_material(0.0, 4.0));
        assert!(feed_link_within_pass_load(
            &mut grid, &mask, from, to, &load
        ));
        assert!(
            !grid.is_material(0.0, 4.0),
            "the admitted link must be stamped on the grid"
        );
    }

    /// A mop chain hop is a `Link`, never part of a `Cut` (G-ADAPTLINKLOAD,
    /// operator ruling 2026-09-26). Two 2 mm residue spots stand 10 mm
    /// apart on a cleared floor; the mop walks the first and must reach the
    /// second. Replaying the emitted segments on the starting grid, every
    /// `Cut` step lands on material (it is a cut), and every `Link` ends
    /// with the cutter disc on cleared cells only (it is a reposition, not
    /// a cut). The historical chain walked the 10 mm gap inside the `Cut`.
    #[test]
    fn a_mop_hop_is_a_link_not_a_cut() {
        let (mut grid, mask, load) = scene();
        let mut all = Vec::new();
        for i in -15..=15 {
            for j in -15..=15 {
                grid.clear_circle_logged(f64::from(i) * 2.0, f64::from(j) * 2.0, R, &mut all);
            }
        }
        let spot = |x0: f64, i: usize| {
            let col = i % grid.cols;
            let row = i / grid.cols;
            let x = grid.origin_x + col as f64 * grid.cell_size;
            let y = grid.origin_y + row as f64 * grid.cell_size;
            (x0..=x0 + 2.0).contains(&x) && (0.0..=2.0).contains(&y)
        };
        let residue: Vec<(usize, u8, u16)> = all
            .iter()
            .copied()
            .filter(|&(i, _, _)| spot(-12.0, i) || spot(0.0, i))
            .collect();
        grid.restore_cleared(&residue);
        let start = grid.clone();

        let segments = mop_residue_into_segments(&mut grid, &mask, R, load.step_len, None, &load);

        let mut replay = start;
        let mut links = 0;
        for seg in &segments {
            match seg {
                AdaptiveSegment::Cut(path) => {
                    for (k, p) in path.iter().enumerate() {
                        if k > 0 {
                            assert!(
                                compute_engagement(&replay, p.x, p.y, R) > 0.0,
                                "a Cut step to ({:.2}, {:.2}) lands on no material: a hop \
                                 inside a Cut",
                                p.x,
                                p.y
                            );
                        }
                        replay.clear_circle(p.x, p.y, R);
                    }
                }
                AdaptiveSegment::Link(p) => {
                    links += 1;
                    assert_eq!(
                        compute_engagement(&replay, p.x, p.y, R),
                        0.0,
                        "a hop Link must end short of the material"
                    );
                }
                AdaptiveSegment::Rapid(p) => replay.clear_circle(p.x, p.y, R),
                AdaptiveSegment::Marker(_) => {}
            }
        }
        assert!(links >= 1, "the 10 mm gap must be crossed by a Link");
        assert!(
            !replay.is_material(-11.0, 1.0) && !replay.is_material(1.0, 1.0),
            "both spots are mopped"
        );
    }
}
