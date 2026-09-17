//! Pass emission: entry-ramp planning, stock-aware link lifts, and the
//! `emit_*` functions that turn ordered `PencilPath`s into a toolpath.
//!
//! Split out of `finish/pencil.rs` (P4). The public entry points
//! (`pencil_toolpath*`) stay in the parent.

use crate::finish::surface_link::build_surface_link;
use crate::geo::P3;
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::tool::MillingCutter;
use crate::toolpath::Toolpath;

use super::chain_paths::contact_runs;
use super::{PencilParams, PencilPath, PencilRuntimeAnnotation, PencilRuntimeEvent};

// ── Entry ramp (G-ENTRYLOAD, 2026-08-23) ────────────────────────────────
//
// A pencil pass that cannot link to its predecessor used to enter every run
// the same way: rapid to the run's first point at `safe_z`, then ONE straight
// fed descent to that point's finished Z. On a `FromRemainingStock` pencil
// that descent is a vertical carve through whatever the upstream op left
// standing over the crease — on the wanaka200 measurement, up to 3.72 mm of
// white oak taken by an R0.5 tapered-ball TIP, at the 150 mm/min flute-tip
// plunge cap, and 2,412 s of the op's 3,332 s total.
//
// It was also invisible. A pure-vertical descent samples as
// `CutKinematics::Plunge`, whose removal the simulator writes to
// `plunge_descent_mm` and NOT to `axial_engagement_mm` — so the
// crosses-standing caution (which reads the axial axis) never saw it, and the
// chipload/deflection/power gates drop entry spans wholesale via
// `tool_load::locality::is_steady_state_for_gate`. Nothing graded it. The
// second half of that hole is closed by `sim_triage::entry_load_finding`.
//
// The fix here is geometric: descend ALONG the valley being entered instead
// of into it, in bite-budgeted zig-zag laps, so no single lap can remove more
// than the budget below.

/// Fraction of the cutter's TIP (cusp) radius one entry lap may remove.
///
/// The budget is tool-scaled rather than pass-scaled because the generator
/// cannot know the pass's median bite — that only exists after a simulation.
/// The two are tied at the other end: `sim_triage::ENTRY_LOAD_MEDIAN_MULTIPLE`
/// (k = 2x the pass's own median body bite) grades what the entry ACTUALLY
/// removed, so a budget that is too generous for a given pass is reported
/// rather than hidden. On the wanaka200 pencil (R0.5 tip, 0.20 mm median
/// bite) this yields 0.25 mm = 1.25x the median, inside k.
pub(crate) const ENTRY_RAMP_BITE_TIP_FRACTION: f64 = 0.5;
/// Floor under the derived per-lap budget — a hair-thin tip must not be able
/// to generate an unbounded lap count.
pub const ENTRY_RAMP_MIN_BITE_MM: f64 = 0.10;
/// Ceiling over the derived per-lap budget. A big ball entering a shallow
/// crease still ramps rather than punching a full-diameter hole.
pub(crate) const ENTRY_RAMP_MAX_BITE_MM: f64 = 0.50;
/// Maximum ramp angle from horizontal, degrees.
///
/// This is deliberately NOT the plunge-rate cap. `stale.tapered_ball_plunge`
/// exists because a tapered ball END-cutting at its tip has zero surface
/// speed on the axis; a bounded-angle ramp is a peripheral cut, and the
/// literature/CAM convention for ball-family ramp entry is an angle limit
/// (a plunge is the 90 degree case). Vertical moves in the entry — only the
/// air descent down to the stock ceiling survives as one — still use
/// `params.plunge_rate`.
///
/// 2026-09-06: raised 8 -> 12 degrees. On a realistic (post-finish) pencil the
/// entry ramps are ~90% of the pass; a steeper cap shortens each lap
/// (`window = budget / (2·tan θ)`) without touching the per-lap bite budget, so
/// the lap COUNT and the anti-gouge guarantee (G-ENTRYLOAD) are unchanged and
/// only the wasted travel drops. 12 degrees is the operator-set cap for the
/// R1.0 tapered ball in white oak — conservative for a small ball tip.
pub(crate) const ENTRY_RAMP_MAX_ANGLE_DEG: f64 = 12.0;
/// Shortest window (mm of path) worth ramping over. Below this the run is
/// treated as too short to enter along and the legacy descent is kept.
pub(crate) const ENTRY_RAMP_MIN_WINDOW_MM: f64 = 0.5;
/// Hard cap on laps. When the stock standing over a crease is so deep that
/// the budget would need more laps than this, the emission stays bounded and
/// the per-lap step grows past the budget — which the post-simulation
/// `project.entry_load` finding then reports. A silently unbounded ramp and a
/// silent plunge are the same failure.
pub(crate) const ENTRY_RAMP_MAX_LAPS: usize = 64;

/// The tool radius that actually nestles into a crease: the corner radius for
/// flat/bullnose cutters, the tip sphere (`cusp_radius_mm`) otherwise. For a
/// tapered ball `radius()` is the SHANK, which is why this is not it.
pub fn tip_contact_radius(cutter: &dyn MillingCutter) -> f64 {
    let cr = cutter.corner_radius_mm();
    if cr > 1e-6 {
        cr
    } else {
        cutter.cusp_radius_mm()
    }
}

/// Per-lap depth budget (mm) for a stepped entry ramp on a cutter with this
/// tip radius. See `ENTRY_RAMP_BITE_TIP_FRACTION`.
pub fn entry_bite_budget_mm(tip_radius_mm: f64) -> f64 {
    (tip_radius_mm * ENTRY_RAMP_BITE_TIP_FRACTION)
        .clamp(ENTRY_RAMP_MIN_BITE_MM, ENTRY_RAMP_MAX_BITE_MM)
}

/// Path length (mm) one entry lap runs over, for a cutter with this tip
/// radius.
///
/// Two consecutive laps run in opposite directions, so their vertical gap is
/// widest at the turn — `2 x step`. The window is therefore sized so that
/// `window_len x tan(angle) <= budget / 2`.
///
/// One construction site: [`plan_entry_ramp`] truncates a caller's run to this
/// length, and a caller that has to SYNTHESISE its run (the dressup ramp, which
/// holds a direction rather than a polyline) reads the same number here.
pub fn entry_ramp_window_mm(tip_radius_mm: f64) -> f64 {
    let tan_ramp = ENTRY_RAMP_MAX_ANGLE_DEG.to_radians().tan();
    (entry_bite_budget_mm(tip_radius_mm) / (2.0 * tan_ramp)).max(ENTRY_RAMP_MIN_WINDOW_MM)
}

/// A planned entry manoeuvre for one run: an air-only vertical descent to the
/// input stock's ceiling, then bite-budgeted zig-zag laps along the run's own
/// first few millimetres.
pub(crate) struct EntryRampPlan {
    /// Z the vertical fed descent stops at — the conservative stock ceiling
    /// over the ramp window, so everything below it is cut by the laps and
    /// everything above it is air.
    pub(crate) air_descent_z: f64,
    /// Lap points in emission order. Every one is clamped to its own point's
    /// finished Z, so no lap can ever cut below the surface.
    pub(crate) points: Vec<P3>,
    /// Index into the run of the window's LAST point. The body pass resumes
    /// at `window_end + 1`; the final lap leaves every window point cut at
    /// its finished Z, so coverage is unchanged.
    ///
    /// Meaningless under `end_at_start` (see [`plan_entry_ramp`]): there the
    /// laps end back on the run's FIRST point, the body pass is not shortened,
    /// and the caller resumes where it already was.
    pub(crate) window_end: usize,
    /// The per-lap Z step actually used (>= the budget only in the
    /// [`ENTRY_RAMP_MAX_LAPS`] clamp case).
    pub(crate) step_mm: f64,
}

/// Plan a bite-budgeted entry ramp along the first few millimetres of `run`.
///
/// Returns `None` — keeping the legacy single descent — when there is no
/// input stock reading, the run is too short to ramp along, or the stock over
/// the window already sits within one bite budget of the finished surface (in
/// which case the descent arrives in near-zero engagement and a ramp would
/// only cost time).
///
/// # Shape
///
/// `laps` descending zig-zag laps, each a straight ramp from the previous
/// level to the next across the whole window, then ONE flat lap at the floor.
/// The flat lap is not optional: a zig-zag whose last lap ramps down leaves a
/// wedge (up to one step) over the start of that lap, and coverage is sacred
/// — the campaign that took pencil coverage from 0.137 to 0.80 is not being
/// paid back a wedge per entry. A second flat lap is appended when parity
/// needs it, so the manoeuvre always ends at the FAR end of the window and
/// the body pass carries on forward from there.
///
/// # Why the worst bite is `2 x step`, not `step`
///
/// Two consecutive laps run in opposite directions, so their vertical gap is
/// widest at the turn: `2 x step` at one end, zero at the other. The window
/// is therefore sized so that `window_len x tan(angle) <= budget / 2`.
///
/// # `end_at_start` — the shared-caller dial (G-ISOCLIPENTRY, 2026-09-09)
///
/// `false` is pencil's own shape: the laps end at the window's FAR end and
/// the caller resumes the body pass at `window_end + 1`, so the window is
/// machined once. `true` ends the laps back on `run[0]` at its finished Z,
/// which is exactly where the plunge this manoeuvre replaces would have left
/// the tool — the shape a caller needs when it only INSERTS the manoeuvre and
/// cannot shorten the body pass that follows.
/// [`crate::dressup::optimize_entry_descents`] is that caller: it rewrites a
/// move list under a provenance contract in which every input move produces at
/// least one output move, so it may not drop the window from the body pass.
/// The window is then cut twice, the second time at zero engagement.
///
/// Both shapes share this one construction site deliberately: the bite budget,
/// the angle cap, the lap ladder and the surface clamp are the physical model
/// of "enter standing material without a full-diameter bite", and one defect
/// class (G-ENTRYLOAD / G-ISOCLIPENTRY) grades both.
pub(crate) fn plan_entry_ramp(
    run: &[P3],
    stock: &crate::dexel_stock::TriDexelStock,
    contact_radius: f64,
    safe_z: f64,
    end_at_start: bool,
) -> Option<EntryRampPlan> {
    if run.len() < 2 {
        return None;
    }
    let tip = contact_radius.max(1e-6);
    let budget = entry_bite_budget_mm(tip);
    let tan_ramp = ENTRY_RAMP_MAX_ANGLE_DEG.to_radians().tan();
    let window_target = entry_ramp_window_mm(tip);

    // The window: the prefix of the run out to `window_target` of XY travel,
    // with its cumulative arclength (both truncated at the same point, so
    // `arc` and the window slice index alike).
    let mut arc: Vec<f64> = Vec::with_capacity(run.len());
    let mut travelled = 0.0_f64;
    let mut prev: Option<P3> = None;
    for p in run {
        if let Some(q) = prev {
            travelled += ((p.x - q.x).powi(2) + (p.y - q.y).powi(2)).sqrt();
        }
        arc.push(travelled);
        prev = Some(*p);
        if travelled >= window_target {
            break;
        }
    }
    let window_end = arc.len().checked_sub(1)?;
    if window_end < 1 {
        return None;
    }
    let window = run.get(..=window_end)?;
    let window_len = *arc.last()?;
    if window_len <= 1e-6 {
        return None;
    }

    // The ceiling: how high the INPUT stock can stand anywhere under the tip
    // over the window. `max_conservative_top_z_in_disc` may only ever err
    // high, so the vertical descent below it is air by construction. The disc
    // is the TIP's, not the envelope's: an envelope-radius disc on a tapered
    // ball would read the valley RIM several millimetres away and ramp
    // through air that is not in the tool's way.
    let mut ceiling = f64::NEG_INFINITY;
    for p in window {
        if let Some(top) = stock.max_conservative_top_z_in_disc(p.x, p.y, contact_radius) {
            ceiling = ceiling.max(top);
        }
    }
    if !ceiling.is_finite() {
        return None;
    }
    let ceiling = ceiling.min(safe_z);
    let floor = window.iter().map(|p| p.z).fold(f64::INFINITY, f64::min);
    if !floor.is_finite() {
        return None;
    }
    let depth = ceiling - floor;
    if depth <= budget {
        // Already inside the budget: the plain descent arrives in near-zero
        // engagement and is the cheaper motion.
        return None;
    }

    let per_lap = (window_len * tan_ramp).min(budget * 0.5).max(1e-6);
    let laps_f = (depth / per_lap)
        .ceil()
        .clamp(1.0, ENTRY_RAMP_MAX_LAPS as f64);
    let laps = laps_f as usize;
    let step = depth / laps as f64;

    let mut levels: Vec<(f64, f64)> = (0..laps)
        .map(|i| (ceiling - step * i as f64, ceiling - step * (i + 1) as f64))
        .collect();
    levels.push((floor, floor));
    // Parity. Each level is one traversal of the window, alternating
    // direction, so an ODD count ends at the window's FAR end and an EVEN
    // count ends back on the run's FIRST point. Pencil owns the window and
    // resumes the body pass past it, so it wants odd. A caller that only
    // INSERTS the manoeuvre in front of an untouched body pass (G-ISOCLIPENTRY,
    // `dressup::optimize_entry_descents`) must hand the tool back where the
    // plunge would have left it, so it wants even.
    let want_even = end_at_start;
    if levels.len().is_multiple_of(2) != want_even {
        levels.push((floor, floor));
    }

    let mut points: Vec<P3> = Vec::with_capacity(levels.len() * window.len());
    for (lap, &(from_z, to_z)) in levels.iter().enumerate() {
        let forward = lap % 2 == 0;
        for k in 1..window.len() {
            let idx = if forward { k } else { window.len() - 1 - k };
            let p = window.get(idx)?;
            let a = *arc.get(idx)?;
            let frac = if forward {
                a / window_len
            } else {
                1.0 - a / window_len
            };
            let z = from_z + (to_z - from_z) * frac;
            points.push(P3::new(p.x, p.y, z.max(p.z)));
        }
    }

    Some(EntryRampPlan {
        air_descent_z: ceiling,
        points,
        window_end,
        step_mm: step,
    })
}

/// Descend into `run`'s first point from `start_z` — a height the caller has
/// already established is clear of standing material — and return the index
/// the body pass resumes at.
///
/// Shared by both junction shapes that arrive from ABOVE the run: the retract
/// (`start_z == params.safe_z`) and the stock-aware lifted link
/// (`start_z ==` the link's own clearance height, see [`plan_link_lift`]).
/// Factoring it out is what stops a lifted link from re-opening G-ENTRYLOAD at
/// the junction it just made safe: the last thing a lifted link does is
/// descend into the crease, and that descent has to be the same bite-budgeted
/// ramp a retract entry gets, not a plunge.
///
/// The `start_z` guard is why this takes the height rather than assuming
/// `safe_z`: the air-only descent is emitted only when the ramp's ceiling is
/// genuinely below where the tool already is. A lifted link can arrive BELOW
/// that ceiling (its clearance is read over the arrival point, the ramp's over
/// the whole window), in which case the descent is skipped and the first lap
/// starts from where the tool stands — still in air, because the lap Z ladder
/// starts at that same ceiling.
fn emit_entry_descent(
    tp: &mut Toolpath,
    run: &[P3],
    first: P3,
    start_z: f64,
    entry_stock: Option<&crate::dexel_stock::TriDexelStock>,
    contact_radius: f64,
    params: &PencilParams,
) -> usize {
    use crate::toolpath::MoveIntent;

    match entry_stock
        .and_then(|stock| plan_entry_ramp(run, stock, contact_radius, params.safe_z, false))
    {
        Some(plan) => {
            // Air only — the descent stops at the conservative stock ceiling,
            // so the plunge-rate cap is paid on nothing but clearance (and
            // `dressup::optimize_entry_descents` turns most of even that into
            // a rapid).
            if plan.air_descent_z < start_z - 1e-9 {
                tp.feed_to_with_intent(
                    P3::new(first.x, first.y, plan.air_descent_z),
                    params.plunge_rate,
                    MoveIntent::EntryPlunge,
                );
            }
            for p in &plan.points {
                tp.feed_to_with_intent(*p, params.feed_rate, MoveIntent::EntryRamp);
            }
            tracing::debug!(
                window_end = plan.window_end,
                step_mm = plan.step_mm,
                ramp_points = plan.points.len(),
                "pencil entry ramped along the crease"
            );
            plan.window_end + 1
        }
        None => {
            tp.feed_to_with_intent(first, params.plunge_rate, MoveIntent::EntryPlunge);
            1
        }
    }
}

// ── Stock-aware surface links (G-LINKLOAD, 2026-08-23) ──────────────────
//
// [`build_surface_link`] follows the drop-cutter surface of the MESH. That is
// the whole truth only when the mesh IS the material — true for a finishing
// pass with everything above it already cleared, false for a
// `FromRemainingStock` pencil, where material stands wherever the upstream op
// could not reach. A link that rides the mesh there is a CUTTING feed through
// whatever is standing above it: on wanaka200 the operator watched the pencil
// "cut through some of the mountains in what looks like travel moves", up to
// ~3 mm of standing material dragged through by an R0.5 tip mid-link.
//
// The entry half of the same defect is G-ENTRYLOAD (see the section above);
// this is the transit half, and it reuses that section's bite budget as its
// trigger so there is one number for "how much a pencil manoeuvre outside its
// body cut may remove".

/// What the emit loop decided to do at one junction between consecutive runs.
enum PencilJunction {
    /// Legacy stay-down surface link: feed along the mesh straight into the
    /// next run's first point. Nothing along it stands more than a bite budget
    /// above the link's own path, so riding the surface removes only what the
    /// target shape says should go.
    Surface(Vec<P3>),
    /// Stock-aware link: the same XY route, every sample lifted clear of the
    /// material standing under it, ENDING ABOVE the next run's first point.
    /// The descent from there is an entry ([`emit_entry_descent`]), not a
    /// link, so the manoeuvre that used to shave a ridge mid-transit becomes
    /// an air hop plus a bite-budgeted entry.
    Lifted(Vec<P3>),
}

impl PencilJunction {
    fn points(&self) -> &[P3] {
        match self {
            Self::Surface(pts) | Self::Lifted(pts) => pts,
        }
    }
}

/// Outcome of asking whether a candidate surface link has to clear standing
/// material.
pub(super) enum LinkLift {
    /// Nothing along the link stands more than one entry bite budget above
    /// the link's own path: keep the legacy stay-down link, byte for byte.
    NotNeeded,
    /// Lifted transit, ending above the next run's first point.
    Lifted(Vec<P3>),
    /// Clearing the standing material reaches `safe_z`. A fed link at retract
    /// height is strictly worse than the rapid it would replace, so the
    /// retract is kept — the same call
    /// [`crate::finish::surface_link::relink_fragments`] makes via
    /// `RelinkReport::ceiling_above_safe_z`.
    Refused,
}

/// Lift a candidate surface link clear of whatever stands above it.
///
/// # Why this is gated rather than unconditional
///
/// [`crate::finish::surface_link::relink_fragments`] lifts every sample of every link
/// it keeps, because its caller (project-curve engraving on raw or partly
/// roughed stock) knows the mesh is never the material. A pencil pass has the
/// opposite prior: it usually runs after a finish pass that already cut most
/// of the surface to shape, and an unconditional lift would spend
/// [`crate::toolpath::PLUNGE_CLEARANCE_MM`] of climb and descent per junction
/// to clear nothing. So the lift engages only where material actually stands
/// in the tool's WAY (see the measurement below), by more than
/// [`entry_bite_budget_mm`] — the same per-manoeuvre bite allowance
/// G-ENTRYLOAD grades entries against. Below it the link shaves finish-pass
/// cusps, which is what a surface link has always done and what the finish
/// op's own gates already grade.
///
/// # The measurement
///
/// All three dexel reads are `max_conservative_top_z_in_disc`, which may only
/// ever err high, so anything derived from them is safe against a finer
/// verification grid. The discs are the TIP's, exactly as
/// [`plan_entry_ramp`] reads its ceiling — not the envelope's, which on a
/// tapered ball reads the valley rim several millimetres away.
///
/// Two of the reads answer the TRIGGER, and it takes both, because one disc
/// cannot say where in itself the material stands:
///
/// * **under the tip** — a zero-radius (i.e. sliver-safe single-column) read:
///   material in the column the tip passes through, standing above the tip.
///   This is precisely the bite a 1-D frontier measures and precisely what the
///   operator saw.
/// * **under the flank** — the tip disc, compared against the highest point of
///   the tool's OWN envelope over that disc (`contact_rise`, i.e.
///   `height_at_radius(contact_radius)` above the tip). Without the envelope
///   term this test fires on every link that rides a crease: the valley wall
///   half a tip-radius away is legitimately higher than the tip, and the tool
///   is legitimately touching it. That is a pencil pass doing its job, not a
///   link ploughing a ridge, and lifting for it would buy every junction in a
///   groove a hop that clears nothing.
///
/// The third read is the CLEARANCE itself
/// ([`crate::finish::surface_link::LinkCeiling::clear_z`]), over the tip disc, once
/// the trigger has fired. That one is PROFILE-AWARE: it lifts only as far as
/// the material the cutter can actually reach at each lateral offset requires,
/// so a ridge standing under the far edge of the disc — where the tool's own
/// flank has already risen well clear of it — no longer raises the
/// transit. The two TRIGGER reads above deliberately stay on the cruder
/// flat-cylinder `material_top`: relaxing the lift HEIGHT is provably safe,
/// relaxing the decision to lift at all is a different question.
///
/// The route sampled is the previous run's exit, the drop-cuttered interior
/// samples (which already carry `stock_to_leave`), and the next run's entry.
/// Both endpoints are cut positions, so their own commanded Z *is* the surface
/// there and no second drop-cutter pass is needed. Lifting the endpoints too
/// is what makes the transit's ends vertical: the tool leaves the cut straight
/// up and arrives straight above the next one.
#[allow(clippy::too_many_arguments)]
pub(super) fn plan_link_lift(
    from: P3,
    to: P3,
    link_pts: &[P3],
    stock: &crate::dexel_stock::TriDexelStock,
    cutter: &dyn MillingCutter,
    contact_radius: f64,
    contact_rise: f64,
    params: &PencilParams,
) -> LinkLift {
    let ceiling = crate::finish::surface_link::LinkCeiling {
        stock: Some(stock),
        tool_radius: contact_radius,
        // Off-grid the dexel has no answer; the analytic fresh-stock top is
        // the fallback `dressup::optimize_entry_descents` takes, and it errs
        // high, which is the safe direction for a clearance.
        fallback_top_z: stock.stock_bbox.max.z,
    };
    // Same stock, same fallback, one column wide — `max_conservative_top_z_in_disc`
    // dilates by half a cell, so radius 0 is "the cells the tip is over".
    let tip_column = crate::finish::surface_link::LinkCeiling {
        tool_radius: 0.0,
        ..ceiling
    };
    let budget = entry_bite_budget_mm(contact_radius.max(1e-6));

    let mut route: Vec<P3> = Vec::with_capacity(link_pts.len() + 2);
    route.push(from);
    route.extend_from_slice(link_pts);
    route.push(to);

    // TRIGGER: both reads stay on the FLAT-CYLINDER `material_top`, unchanged.
    // The profile-aware ceiling reads at or below this, so wiring it in here
    // would make the trigger fire LESS often — the wrong direction for a
    // gouge guard. Only the LIFT HEIGHT below is profile-aware.
    let stands_in_the_way = |p: &P3| {
        tip_column.material_top(p.x, p.y) > p.z + budget
            || ceiling.material_top(p.x, p.y) > p.z + contact_rise + budget
    };
    if !route.iter().any(stands_in_the_way) {
        return LinkLift::NotNeeded;
    }

    let mut lifted = Vec::with_capacity(route.len());
    for p in &route {
        // Profile-aware: on a tapered ball the shank stands many millimetres
        // above the tip at the edge of the disc, so material out there cannot
        // touch the cutter and must not raise the transit.
        let clear = ceiling.clear_z(cutter, p.x, p.y, p.z);
        if clear >= params.safe_z - 1e-6 {
            return LinkLift::Refused;
        }
        lifted.push(P3::new(p.x, p.y, clear));
    }
    LinkLift::Lifted(lifted)
}

/// Emit the ordered `PencilPath`s as toolpath moves. Consecutive passes whose
/// endpoints are within `hookup_distance` are joined by a gouge-safe
/// surface-following feed instead of a retract-rapid-replunge — on dense
/// organic relief the chains fragment heavily, so per-fragment retracts
/// dominated the rapid distance. The retract is deferred: it fires only when
/// the next pass (or run — see [`contact_runs`]) is too far, or its link loses
/// surface contact, and once at the very end.
///
/// P1 quantitative linker (unified-finishing-pass W4a): `hookup_distance` is
/// now only the CANDIDATE cap — a gap has to be within it (and gouge-safe via
/// [`build_surface_link`]) to be considered at all — but which link actually
/// gets emitted is decided by integrated time
/// ([`crate::machine::kinematics::surface_link_time`] vs.
/// [`crate::machine::kinematics::retract_link_time`]), never by raw
/// distance/feed, whenever `params.link_kinematics` is `Some`. The P0
/// unified-finishing probe (`planning/unified_finishing_pass_plan.md`) found
/// the naive distance/feed estimate misjudges wall-clock by up to 10× on
/// segmented paths (3D Finish 6: 10.1× naive) — junction/accel physics, not
/// commanded feed, dominates once segments get short, so a distance-only
/// hookup heuristic picks the wrong link on exactly the paths where it
/// matters most. `params.link_kinematics = None` keeps the legacy behaviour
/// (surface link whenever `build_surface_link` succeeds within
/// `hookup_distance`).
///
/// Each `PencilPath` is itself split at non-contact (NaN-Z) points via
/// [`contact_runs`] before emission, so an off-mesh gap in the middle of a
/// pass produces two independent runs — each with its own rapid/plunge or
/// surface link — rather than a single cutting move bridging the gap.
///
/// Entries and links have no stock reading on this form, so entries keep the
/// legacy single fed descent and links keep riding the mesh surface. Every
/// production caller — the pencil generator and [`crate::finish::unified_finish`]'s
/// pencil-claims pipeline — holds the input stock and calls
/// [`emit_paths_with_entry_stock`] directly (G-ENTRYLOAD, G-LINKLOAD), so this
/// wrapper survives only as the tests' stock-less spelling.
#[cfg(test)]
pub(crate) fn emit_paths(
    all_paths: &[PencilPath],
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &PencilParams,
) -> (Toolpath, Vec<PencilRuntimeAnnotation>) {
    emit_paths_with_entry_stock(all_paths, mesh, index, cutter, params, None)
}

/// [`emit_paths`] with the input stock this pass is cutting into.
///
/// `entry_stock` is read by the two manoeuvres that move through material
/// without being body cuts, and by nothing else:
///
/// * the ENTRY into a run that could not be linked to
///   ([`plan_entry_ramp`], G-ENTRYLOAD), and
/// * the LINK between two runs ([`plan_link_lift`], G-LINKLOAD) — lifted
///   clear of standing material where any stands above its own path, and
///   abandoned for a retract when clearing it would reach `safe_z`.
///
/// `None` reproduces the pre-G-ENTRYLOAD/pre-G-LINKLOAD emission exactly —
/// neither manoeuvre can engage without a stock reading — which is what makes
/// the A/Bs in `tests/pencil_entry_ramp_g_entryload.rs` and
/// `tests/pencil_surface_link_g_linkload.rs` controlled ones.
pub(crate) fn emit_paths_with_entry_stock(
    all_paths: &[PencilPath],
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &PencilParams,
    entry_stock: Option<&crate::dexel_stock::TriDexelStock>,
) -> (Toolpath, Vec<PencilRuntimeAnnotation>) {
    let (tp, anns, _report) =
        emit_paths_with_entry_stock_reported(all_paths, mesh, index, cutter, params, entry_stock);
    (tp, anns)
}

/// Why each pencil junction did or did not link (G-LINKSTAGE instrument).
///
/// The pencil emitter is the last surface op on its own hand-rolled linker,
/// and it published NOTHING about its refusals — so a pass that spends 84 %
/// of its wall clock on entry motion (2 307 s of entry against 50 s of
/// cutting, `planning/linking_2026-09-09/SPEC.md` §8) could not say which of
/// the four gates produced it. One regen now names the binding constraint,
/// the way the scallop's `"Scallop intra-pass relink"` line already does.
///
/// [`Self::linked_at_depth`] is the ACCEPTANCE measure, apart from
/// [`Self::linked_via_hop`] and not summed with it: only an at-depth link
/// removes [`emit_entry_descent`], and on this pass an entry costs 7.2 s
/// against 0.16 s of cutting per fragment. A hop that removes a retract and
/// leaves the ramp standing scores zero here, correctly.
/// G-LINKVISIBLE (2026-09-09): published on
/// [`crate::compute::config::ToolpathStats::pencil_link`], in its OWN slot
/// rather than mapped onto [`crate::finish::unified_finish::RelinkTotals`]. The two
/// counters a mapping would have to drop — [`Self::hop_too_far`] and this
/// pass's own at-depth/hop split — are exactly the ones that name the
/// pencil's binding constraint, and a measurement squeezed into another
/// measurement's shape reads clean and means something else.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PencilLinkReport {
    /// Transitions between two emitted runs — every junction that had a link
    /// decision to make. The first run of the pass is not one.
    pub junctions: usize,
    /// TIER (a): joined at cutting depth. The tool never left the material,
    /// so the next run needs no entry at all.
    pub linked_at_depth: usize,
    /// TIER (b): joined by a lift clear of standing material and a descent.
    /// Removes the retract; the entry survives as a (shallower) descent.
    pub linked_via_hop: usize,
    /// Gap beyond [`PencilParams::hookup_distance`].
    pub too_far: usize,
    /// Gap inside the at-depth cap but beyond
    /// [`PencilParams::link_hop_distance_mm`], after the candidate turned out
    /// to need a lift. Structurally `0` while that dial is `None`.
    pub hop_too_far: usize,
    /// The surface-following candidate lost contact with the mesh.
    pub off_surface: usize,
    /// Clearing the standing material reached `safe_z`, so the retract was
    /// kept ([`LinkLift::Refused`]).
    pub ceiling_refused: usize,
    /// The candidate was safe but the F-034 integrator priced it above the
    /// retract it would replace.
    pub slower_than_retract: usize,
}

/// [`emit_paths_with_entry_stock`] with the link report a sentry can read.
pub(crate) fn emit_paths_with_entry_stock_reported(
    all_paths: &[PencilPath],
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &PencilParams,
    entry_stock: Option<&crate::dexel_stock::TriDexelStock>,
) -> (Toolpath, Vec<PencilRuntimeAnnotation>, PencilLinkReport) {
    use crate::toolpath::MoveIntent;

    // The HOP cap. `None` means "the same cap as `hookup_distance`", which is
    // the byte-identical shipped value — see
    // `PencilParams::link_hop_distance_mm` for why the two tiers want
    // different numbers.
    let hop_cap = params
        .link_hop_distance_mm
        .unwrap_or(params.hookup_distance);
    let mut report = PencilLinkReport::default();

    let contact_radius = tip_contact_radius(cutter);
    // How high the tool's own envelope stands at the edge of that disc, above
    // its tip. G-LINKLOAD's flank test needs it so a link riding a crease is
    // not mistaken for one ploughing a ridge — see `plan_link_lift`.
    let contact_rise = cutter.height_at_radius(contact_radius).unwrap_or(0.0);
    let mut tp = Toolpath::new();
    let mut annotations = Vec::new();
    let mut prev_end: Option<P3> = None;

    for path in all_paths {
        if path.points.len() < 2 {
            continue;
        }

        for run in contact_runs(&path.points) {
            if run.len() < 2 {
                continue;
            }

            let move_index = tp.moves.len();
            let first = *run.first().unwrap_or(&P3::origin());
            // Where the body pass starts. A ramped entry has already cut the
            // window at its finished Z, so the body resumes past it.
            let mut body_start = 1usize;

            // Try to link from the previous run's end without retracting.
            // `hookup_distance` is only the candidate CAP (gap must be within
            // it, and the link must be gouge-safe via `build_surface_link`);
            // the candidate is then lifted clear of standing material
            // (G-LINKLOAD), and when the caller supplied `link_kinematics` it
            // is additionally costed against a retract-link candidate with the
            // F-034 integrator, and only kept when it's actually cheaper —
            // see the doc comment above.
            let link = prev_end.and_then(|end| {
                report.junctions += 1;
                let gap = ((first.x - end.x).powi(2) + (first.y - end.y).powi(2)).sqrt();
                if gap <= 1e-6 {
                    return None;
                }
                if gap > params.hookup_distance {
                    report.too_far += 1;
                    return None;
                }
                let Some(link_pts) = build_surface_link(
                    end,
                    first,
                    mesh,
                    index,
                    cutter,
                    params.stock_to_leave,
                    params.sampling,
                ) else {
                    report.off_surface += 1;
                    return None;
                };
                // G-LINKLOAD: the candidate rides the MESH, which is only the
                // material when nothing stands above it. Lift it clear where
                // something does, and keep the retract when clearing costs the
                // whole retract anyway. `entry_stock: None` cannot reach this
                // at all — no stock reading, no lift, no change.
                let candidate = match entry_stock {
                    None => PencilJunction::Surface(link_pts),
                    Some(stock) => {
                        // Bound rather than matched inline: the arms move
                        // `link_pts`, and a scrutinee temporary would keep its
                        // borrow alive across them.
                        let lift = plan_link_lift(
                            end,
                            first,
                            &link_pts,
                            stock,
                            cutter,
                            contact_radius,
                            contact_rise,
                            params,
                        );
                        match lift {
                            LinkLift::NotNeeded => PencilJunction::Surface(link_pts),
                            LinkLift::Lifted(pts) => {
                                // TIER (b) has its own, shorter, reach. A hop
                                // does not remove the entry, and a long one
                                // crosses more finished terrain to buy less —
                                // the measured 4.2× regression the split cap
                                // exists to prevent.
                                if gap > hop_cap {
                                    report.hop_too_far += 1;
                                    return None;
                                }
                                PencilJunction::Lifted(pts)
                            }
                            LinkLift::Refused => {
                                report.ceiling_refused += 1;
                                return None;
                            }
                        }
                    }
                };
                match &params.link_kinematics {
                    Some(lk) => {
                        // Costs the geometry that will actually be emitted —
                        // the lift is applied BEFORE this, so a link that only
                        // pays for itself while riding the surface loses here
                        // once it has to climb. The descent is modelled as the
                        // straight line into `first` that a legacy link ends
                        // with; the emitted ramp is cheaper than that, so the
                        // comparison errs against the link.
                        let mut costed_path = candidate.points().to_vec();
                        costed_path.push(first);
                        let surface_t = crate::machine::kinematics::surface_link_time(
                            end,
                            &costed_path,
                            params.feed_rate,
                            &lk.kinematics,
                            lk.max_feed_mm_min,
                            lk.rapid_feed_mm_min,
                        );
                        // The retract candidate's descent isn't split by a
                        // rapid-down-to-clearance: the cost model can't
                        // verify the input stock's ceiling from here (that
                        // check lives in the post-generation
                        // `optimize_entry_descents` pass), so it must not
                        // assume a descent it can't guarantee is safe.
                        let retract_t = crate::machine::kinematics::retract_link_time(
                            end,
                            first,
                            params.safe_z,
                            None,
                            params.plunge_rate,
                            &lk.kinematics,
                            lk.max_feed_mm_min,
                            lk.rapid_feed_mm_min,
                        );
                        if surface_t <= retract_t {
                            Some(candidate)
                        } else {
                            report.slower_than_retract += 1;
                            None
                        }
                    }
                    None => Some(candidate),
                }
            });

            match link {
                Some(PencilJunction::Surface(link_pts)) => {
                    report.linked_at_depth += 1;
                    // Surface-following link (no retract / no re-plunge), then the body.
                    for lp in &link_pts {
                        tp.feed_to_with_intent(*lp, params.feed_rate, MoveIntent::Linking);
                    }
                    tp.feed_to_with_intent(first, params.feed_rate, MoveIntent::Linking);
                }
                Some(PencilJunction::Lifted(link_pts)) => {
                    report.linked_via_hop += 1;
                    // Stock-aware link: the transit is entirely above the
                    // standing material, so it ends ABOVE `first` rather than
                    // on it. The way down is an entry, not a link — see
                    // `emit_entry_descent`.
                    for lp in &link_pts {
                        tp.feed_to_with_intent(*lp, params.feed_rate, MoveIntent::Linking);
                    }
                    let start_z = link_pts.last().map_or(params.safe_z, |p| p.z);
                    body_start = emit_entry_descent(
                        &mut tp,
                        run,
                        first,
                        start_z,
                        entry_stock,
                        contact_radius,
                        params,
                    );
                }
                None => {
                    // Too far (or unsafe) to link: retract the previous run, then a
                    // fresh rapid-over + entry.
                    if let Some(end) = prev_end {
                        tp.rapid_to_with_intent(
                            P3::new(end.x, end.y, params.safe_z),
                            MoveIntent::Retract,
                        );
                    }
                    tp.rapid_to_with_intent(
                        P3::new(first.x, first.y, params.safe_z),
                        MoveIntent::Linking,
                    );
                    body_start = emit_entry_descent(
                        &mut tp,
                        run,
                        first,
                        params.safe_z,
                        entry_stock,
                        contact_radius,
                        params,
                    );
                }
            }

            // Feed the body (skipping whatever the entry already cut — at
            // minimum the first point, which we are already standing on).
            for p in run.iter().skip(body_start) {
                tp.feed_to_with_intent(*p, params.feed_rate, MoveIntent::FinishingCut);
            }
            prev_end = run.last().copied();

            annotations.push(PencilRuntimeAnnotation {
                move_index,
                event: PencilRuntimeEvent::OffsetPass {
                    chain_index: path.chain_index,
                    chain_total: path.chain_total,
                    offset_index: path.offset_index,
                    offset_total: path.offset_total,
                    offset_mm: path.offset_mm,
                    is_centerline: path.is_centerline,
                },
            });
        }
    }

    // Final retract once everything is emitted.
    if let Some(end) = prev_end {
        tp.rapid_to_with_intent(P3::new(end.x, end.y, params.safe_z), MoveIntent::Retract);
    }

    // G-LINKSTAGE instrument. Same shape as the scallop's
    // `"Scallop intra-pass relink"` line, so one regen names the binding
    // constraint on either op with the same reading.
    tracing::info!(
        junctions = report.junctions,
        // The acceptance measure. `linked_at_depth` is the only counter that
        // removes an entry, and entry is ~84 % of this pass.
        linked_at_depth = report.linked_at_depth,
        linked_via_hop = report.linked_via_hop,
        too_far = report.too_far,
        hop_too_far = report.hop_too_far,
        off_surface = report.off_surface,
        ceiling_refused = report.ceiling_refused,
        slower_than_retract = report.slower_than_retract,
        hookup_mm = params.hookup_distance,
        hop_cap_mm = hop_cap,
        "Pencil link stage"
    );

    (tp, annotations, report)
}
