//! Step 6 of the calculator: the power check and its engagement ladder.
//!
//! The ladder has five rungs, one function each: the RPM ([`rung1_rpm`]),
//! the depth and the width ([`rungs2_3_geometry`]), the feed
//! ([`rung4_feed`]) and the report ([`rung5_report`]).

use super::feed::power_curve;
use super::{CalcState, Chip, Cut, Feed, Speed};
use crate::feeds::sourced::Basis;
use crate::feeds::{
    FeedsInput, FeedsWarning, POWER_LADDER_AE_FLOOR_MM, POWER_LADDER_AP_FLOOR_MM,
    SpindleScaleReason, largest_fitting, power_model_terms, worst_case_power,
};
use crate::material::force_line::ForceLine;

/// 1 % of utilisation is the threshold for "this lever does something".
/// The VFD case lands at exactly 0 % by construction; a router moves tens
/// of percent.
const TRAVERSE_HELPS_EPS: f64 = 0.01;

/// What the ladder did, for the `PowerLadderReducedCut` report.
struct LadderMoves {
    rpm_before: f64,
    moved_rpm: bool,
    ap_from: Option<f64>,
    ae_from: Option<f64>,
    feed_factor: Option<f64>,
}

/// The fixed inputs of the ladder.
struct Ladder<'i, 'a> {
    input: &'i FeedsInput<'a>,
    line: ForceLine,
    effective_d: crate::feeds::engagement::EngagedDiameter,
}

impl Ladder<'_, '_> {
    /// Required kW at a candidate operating point.
    /// K7 change 3: every rung reads the worst case of the set.
    fn required_at(&self, rpm_c: f64, ap_c: f64, ae_c: f64, feed_c: f64) -> f64 {
        worst_case_power(
            self.input,
            self.line,
            ap_c,
            ae_c,
            self.effective_d,
            rpm_c,
            feed_c,
        )
        .1
    }

    fn budget_at(&self, rpm_c: f64) -> f64 {
        self.input.machine.power_at_rpm(rpm_c).max(0.0)
    }
}

/// Step 6: the power check.
pub(super) fn step6_power(s: &mut CalcState) {
    let input = s.input;
    let machine = input.machine;
    let rpm = s.speed.rpm;
    let (ap, ae) = (s.cut.ap, s.cut.ae);
    let effective_d = s.feed.effective_d;
    let raw_feed = s.feed.raw_feed;
    // --- Step 6: Power check ---
    // Materials without a primary-source Kc skip the power-vs-machine
    // ramp here; downstream tool_load::power refuses with
    // `MaterialUnvalidated` so the user sees the gap explicitly rather
    // than getting a silently-fabricated feed.
    //
    // Power prediction routes through the canonical
    // `tool_load::power::PowerTerms` so the Suggest path and the Sim
    // verdict can't diverge. P20d: the engagement's section
    // (`feeds::section::EngagedSection`: the rectangle, or the cone of a
    // V-bit) states both the shear area and the edge term, the same
    // section the Sim verdict reads; `power_model_terms` assembles both
    // halves from one place.
    //
    // ── The power ceiling (ruling R4 Q2, 2026-09-24) ──
    //
    // The ceiling is the rated spindle curve `power_at_rpm(rpm)`, with no
    // fraction. Until R4 it was `power_at_rpm × safety_factor`, and Step 9
    // multiplied the feed by the same factor, so this step had to state its
    // clamp on a "commanded" axis (F-2, R1). With the factor gone the raw
    // feed IS the commanded feed, and there is one axis. No load margin
    // scales the ceiling (D9, 2026-11-04, removed the machine dial).
    //
    // Measured before R4 (tests/power_ceiling_parity_f2.rs): the branch
    // fires on Ø12 slots in Jarrah and Ipe, where the edge term is largest.
    let available_power = machine.power_at_rpm(rpm);
    // The gate's ceiling — what every published power number below is
    // quoted against. Since ruling R4 Q2 it is the rated curve itself.
    let gate_available_power = available_power;
    // Power-ladder bookkeeping. An RPM move is booked on `spindle_scale`, a
    // geometry move changes `ap` / `ae` themselves, and only the last-resort
    // feed rung touches `power_factor` — the CHIPLOAD axis — because only
    // that rung changes the advance per tooth.
    let mut moves = LadderMoves {
        rpm_before: rpm,
        moved_rpm: false,
        ap_from: None,
        ae_from: None,
        feed_factor: None,
    };

    // P16: no power figure when the depth is known after the simulation.
    // P20d: a drill cycle's axial power is advisory (the force line is not
    // measured on an end cut), so the ladder does not act on it.
    if let Some(line) = input.power_force_line()
        && available_power > 0.0
        && !input.is_axial_plunge()
    {
        let ladder = Ladder {
            input,
            line,
            effective_d,
        };
        // The power the recipe draws at the feed it ships: the worst case
        // of the pre-sim set (K7 change 3).
        let (_, required_at_commanded, _) =
            worst_case_power(input, line, ap, ae, effective_d, rpm, raw_feed);
        if required_at_commanded > gate_available_power {
            // `feed_for_kw` returns `None` when the feed-free edge term ALONE is
            // over the budget — the power-side twin of
            // `DeflectionCapRefusal::EdgeForceOverBudget`. No feed
            // rescues that cut: thinning the chip leaves the ploughing
            // power exactly where it was, while driving the chipload
            // toward the rubbing floor and raising the energy spent per
            // mm³. So the feed is left alone and the warning carries the
            // conflict — the same clamp-and-warn convention as the
            // rubbing floor at Step 9b, which also refuses to serve a
            // recipe it cannot honour. The real fix is less DOC, less
            // stepover or a lower RPM, none of which a feed derate can
            // reach for. Serving a zero feed instead would be a
            // fabricated recipe, not an answer.
            // ── The engagement ladder (2026-09-16) ────────────────────
            //
            // Pre-ladder this scaled the feed and nothing else. That is
            // the wrong dial for this constraint, twice over.
            //
            // First, the edge term carries no feed at all, so thinning
            // the chip sheds only part of the load while pushing the
            // chipload toward the rubbing floor. On the motivating case
            // — Ø12 4-flute bull, white oak, Shapeoko VFD at 9 000 rpm —
            // the feed derate came out at 0.058, a 17x cut, and the
            // recipe was STILL over budget afterwards and rubbing-
            // adjacent. Two warnings fired and neither constraint was
            // honoured. That is literature-matrix cell
            // `bull_12mm_pocket_oak`.
            //
            // Second, what actually sheds spindle load depends on the
            // spindle. On a constant-torque VFD below its rated speed
            // the available power falls with the RPM at exactly the rate
            // the required power falls, so walking the constant-chipload
            // line changes NOTHING: measured 120 % utilisation at 9 000,
            // 6 000, 4 500 and 3 000 rpm alike. On a constant-power
            // router the same walk takes 95 % to 32 %.
            //
            // So the ladder asks what this cut can give up, in order,
            // and stops at the first rung that works:
            //
            //   1. RPM, holding the chipload — where it helps.
            //   2. Axial depth — where the ENGINE chose it.
            //   3. Radial width — where the ENGINE chose it.
            //   4. Nothing. Leave the cut alone and say so.
            //
            // Rungs 2 and 3 do not move the feed or the RPM, so the
            // advance per tooth is untouched: the cut gets smaller, not
            // slower and thinner.
            //
            // Rung 1 branches on MEASURED behaviour, not on the
            // `PowerModel` variant. The physical question is "does a
            // lower RPM reduce utilisation here", and the numbers answer
            // it directly — which also covers the flat region above a
            // VFD's rated speed without a special case, and keeps
            // working if a new `PowerModel` is added.
            //
            // See planning/load_model_2026-09-16/{DERATE_SPEC.md,
            // IMPLEMENTATION_PLAN.md} and derate_levers.py.
            rung1_rpm(
                &ladder,
                &s.chip,
                &s.cut,
                &mut s.speed,
                &mut s.feed,
                &mut moves,
                required_at_commanded,
                gate_available_power,
            );
            rungs2_3_geometry(&ladder, &s.speed, &mut s.cut, &mut s.feed, &mut moves);
            let (still_required, still_available) =
                rung4_feed(&ladder, &s.chip, &s.speed, &s.cut, &mut s.feed, &mut moves);
            rung5_report(
                s,
                &moves,
                required_at_commanded,
                gate_available_power,
                still_required,
                still_available,
            );
        }
    }
}

/// Rung 1: walk the constant-chipload line, where a lower RPM helps and
/// can close the gap by itself.
#[expect(
    clippy::too_many_arguments,
    reason = "the rung reads the four records of the calculator and two figures of the check"
)]
fn rung1_rpm(
    ladder: &Ladder,
    chip: &Chip,
    cut: &Cut,
    speed: &mut Speed,
    feed: &mut Feed,
    moves: &mut LadderMoves,
    required_at_commanded: f64,
    gate_available_power: f64,
) {
    let machine = ladder.input.machine;
    let (ap, ae) = (cut.ap, cut.ae);
    let rpm = speed.rpm;
    // ── Rung 1: walk the constant-chipload line ───────────────
    //
    // At a fixed chipload the feed is proportional to the RPM, so
    // a candidate RPM implies its own feed.
    // The traverse has a FLOOR, and it is not the machine's.
    //
    // A published RPM band is a statement about the cut, not about
    // the machine: below its minimum the surface speed is too low
    // for the material and the tool rubs or builds up an edge —
    // acutely so in aluminium. Step 5c honours the same
    // row's `rpm_max` on the way up; this is the same rule facing
    // the other way.
    //
    // The literature matrix found this: the first version of the
    // ladder bounded only by `machine.rpm_range()` and walked
    // `flat_6mm_pocket_al6061_lut` from 20 000 rpm to 8 000, which
    // is 55 % under the row's published minimum. Shedding spindle
    // load by cutting aluminium at a wood RPM is not a trade the
    // engine gets to make silently.
    let machine_floor = machine.rpm_range().0;
    let rpm_floor = match chip.vendor_rpm_min {
        Some(v) if v.is_finite() && v > 0.0 => machine_floor.max(v),
        _ => machine_floor,
    };
    let chipload_held = if rpm > 0.0 { feed.raw_feed / rpm } else { 0.0 };
    let feed_at = |rpm_c: f64| chipload_held * rpm_c;

    let fits = |rpm_c: f64, ap_c: f64, ae_c: f64| -> bool {
        ladder.required_at(rpm_c, ap_c, ae_c, feed_at(rpm_c)) <= ladder.budget_at(rpm_c)
    };

    // Does slowing down help AT ALL? Ask the slowest speed this
    // spindle can run. On a constant-torque VFD the answer is no,
    // and the ladder must not waste the operator's RPM on it.
    let slowest = machine.next_rpm_at_or_below(rpm_floor);
    let util_now = required_at_commanded / gate_available_power.max(1e-12);
    let util_slowest = {
        let b = ladder.budget_at(slowest);
        if b > 0.0 {
            ladder.required_at(slowest, ap, ae, feed_at(slowest)) / b
        } else {
            f64::INFINITY
        }
    };
    let traverse_helps = util_slowest < util_now - TRAVERSE_HELPS_EPS;

    // Use the traverse ONLY when it can close the gap by itself.
    //
    // The first version bisected regardless, so when the traverse
    // could not close the gap the bisection converged on the floor
    // and the ladder spent the machine's entire speed range for a
    // partial gain — and then shrank the cut anyway. The literature
    // matrix showed the result plainly: two unrelated cells both
    // pinned at exactly 8 000 rpm, the floor, for different reasons.
    //
    // A partial traverse is not obviously better than no traverse:
    // both cost time, and a slower spindle also costs surface
    // finish and tool life. So the rule is all-or-nothing, and when
    // the RPM cannot do the job the geometry does it at the RPM the
    // chart chose.
    let traverse_can_close_it = fits(rpm_floor, ap, ae);
    if traverse_helps && traverse_can_close_it && rpm > rpm_floor {
        // Highest RPM that fits, by bisection on the continuous
        // range, then rounded DOWN to a speed the spindle can
        // actually run. Rounding down matters: `clamp_rpm` snaps
        // to the NEAREST speed and would round back up, raising
        // the load this rung exists to shed.
        let mut lo = rpm_floor;
        let mut hi = rpm;
        for _ in 0..40 {
            let mid = 0.5 * (lo + hi);
            if fits(mid, ap, ae) {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        let candidate = machine.next_rpm_at_or_below(lo);
        if candidate < rpm && candidate > 0.0 {
            // Book it on the SPEED axis. The chipload did not
            // change, so `combined_factor` must not see this.
            speed.spindle_scale *= candidate / rpm;
            speed.spindle_scale_reason = SpindleScaleReason::PowerLimit;
            feed.raw_feed = feed_at(candidate);
            speed.rpm = candidate;
            feed.feed = feed.raw_feed;
            moves.moved_rpm = true;
            speed.basis =
                Basis::derived("power ladder RPM", vec![speed.basis.clone(), power_curve()]);
        }
    }
}

/// Rungs 2 and 3: make the cut smaller, where the engine chose the depth
/// or the width.
fn rungs2_3_geometry(
    ladder: &Ladder,
    speed: &Speed,
    cut: &mut Cut,
    feed: &mut Feed,
    moves: &mut LadderMoves,
) {
    let input = ladder.input;
    let rpm = speed.rpm;
    // ── Rungs 2 and 3: make the cut smaller ───────────────────
    //
    // The feed and the RPM are settled now. Power rises
    // monotonically with both the axial depth and the radial
    // width, so the largest value that fits is a bisection.
    //
    // A knob the USER pinned is not the engine's to move. Pinning
    // a depth is a statement about the part, and quietly cutting
    // it shallower would change what was asked for rather than
    // how fast it is cut.
    let raw_feed = feed.raw_feed;
    let ae = cut.ae;
    if ladder.required_at(rpm, cut.ap, ae, raw_feed) > ladder.budget_at(rpm)
        && input.axial_depth_mm.is_none()
        && let Some(fitted) = largest_fitting(cut.ap, POWER_LADDER_AP_FLOOR_MM, |c| {
            ladder.required_at(rpm, c, ae, raw_feed) <= ladder.budget_at(rpm)
        })
        && fitted < cut.ap
    {
        moves.ap_from = Some(cut.ap);
        cut.ap = fitted;
        cut.ap_basis = Basis::derived(
            "power ladder depth",
            vec![cut.ap_basis.clone(), power_curve()],
        );
    }
    // P11b (L1): a narrower width lifts the feed by the chip-thinning
    // law, so the peak chip stays the band chip at every candidate
    // width. Power still rises with the width at that feed
    // (`(1 - cos psi) / sin psi = tan(psi / 2)` rises with psi), so the
    // bisection stays monotone.
    let thinning = feed.thinning;
    let effective_d = feed.effective_d;
    let radial_lift = feed.radial_lift;
    // P23a: the lift at a candidate width reads the full-feed width of that
    // width (the pass ceiling is a function of the stepover).
    let lift_at = |c: f64| {
        thinning.lift(
            input.power_cases.full_feed_width(c, input.tool_diameter),
            effective_d.mm(),
        )
    };
    let feed_at_width = |c: f64| raw_feed * lift_at(c) / radial_lift;
    let ap = cut.ap;
    if ladder.required_at(rpm, ap, cut.ae, raw_feed) > ladder.budget_at(rpm)
        && input.radial_width_mm.is_none()
        && let Some(fitted) = largest_fitting(cut.ae, POWER_LADDER_AE_FLOOR_MM, |c| {
            ladder.required_at(rpm, ap, c, feed_at_width(c)) <= ladder.budget_at(rpm)
        })
        && fitted < cut.ae
    {
        moves.ae_from = Some(cut.ae);
        feed.raw_feed = feed_at_width(fitted);
        feed.feed = feed.raw_feed;
        feed.radial_lift = lift_at(fitted);
        cut.ae = fitted;
        cut.ae_basis = Basis::derived(
            "power ladder width",
            vec![cut.ae_basis.clone(), power_curve()],
        );
    }
}

/// Rung 4: the feed, last. Returns the required and the available power
/// after the rung.
///
/// P16 (review D8, lead ruling 2026-11-06): the rubbing floor
/// ([`Chip::rubbing_floor`], the Step 9b floor) is the lower bound of the
/// rung. "No feed fits" has two causes: the edge term alone is over the
/// budget (`feed_for_kw` gives `None`), or the feed that fits the power is
/// below the floor feed. Both ship ONE answer: the floor feed, never above
/// the feed before the rung (R4: the floor never lifts a feed).
/// `PowerLimited` and `PowerLadderReducedCut` name the conflict.
fn rung4_feed(
    ladder: &Ladder,
    chip: &Chip,
    speed: &Speed,
    cut: &Cut,
    feed: &mut Feed,
    moves: &mut LadderMoves,
) -> (f64, f64) {
    let rpm = speed.rpm;
    let (ap, ae) = (cut.ap, cut.ae);
    // ── Rung 4: the feed, LAST ────────────────────────────────
    //
    // The feed is a poor lever for this constraint, not a forbidden
    // one. The edge term carries no feed, so a feed cut sheds only
    // part of the load and drives the chipload toward rubbing — which
    // is why it must not be FIRST. But shipping a recipe the spindle
    // cannot turn is worse than shipping a thinner chip, and the
    // gate's contract is that a power-limited recommendation lands ON
    // the ceiling, never above it (`power_ceiling_parity_f2`).
    //
    // By the time the ladder reaches here the RPM and the geometry
    // have already taken the load down, so the feed cut needed is far
    // smaller than the pre-ladder one — the motivating case went from
    // a 17x feed cut to none at all. This rung exists to close the
    // last sliver, not to carry the constraint.
    //
    // It is booked on the CHIPLOAD axis, because unlike every rung
    // above it this one DOES change the advance per tooth.
    let mut still_required = ladder.required_at(rpm, ap, ae, feed.raw_feed);
    let still_available = ladder.budget_at(rpm);
    if still_required > still_available {
        // The terms of the worst case: the widest case is the
        // largest at every feed, so its inversion is the cap.
        let (_, _, worst_ae) = worst_case_power(
            ladder.input,
            ladder.line,
            ap,
            ae,
            ladder.effective_d,
            rpm,
            feed.raw_feed,
        );
        let terms_now = power_model_terms(
            ladder.input,
            ladder.line,
            ap,
            worst_ae,
            ladder.effective_d,
            rpm,
        );
        // P16 (D8): the floor feed, the rubbing floor at this RPM.
        // `None` means the feed-free edge term ALONE is over budget; a cap
        // under the floor feed means the fitting feed rubs. In both cases no
        // feed fits, and the rung ships the floor feed: the least power
        // that does not rub (the modulator's `PowerMax` pin has the same
        // shape). It never raises the feed.
        let (floor_fpt, _) = chip.rubbing_floor();
        let floor_feed = floor_fpt * rpm * f64::from(ladder.input.flute_count);
        let target = match terms_now.feed_for_kw(still_available) {
            Some(cap) if cap >= floor_feed => cap,
            Some(_) | None => floor_feed,
        };
        if feed.raw_feed > 0.0 && target.is_finite() && target > 0.0 {
            let f = (target / feed.raw_feed).clamp(0.0, 1.0);
            if f < 1.0 {
                feed.power_factor = f;
                feed.raw_feed = target;
                moves.feed_factor = Some(f);
                feed.feed_power_cut = true;
                still_required = ladder.required_at(rpm, ap, ae, feed.raw_feed);
            }
        }
        feed.feed = feed.raw_feed;
    }
    (still_required, still_available)
}

/// Rung 5: the report.
fn rung5_report(
    s: &mut CalcState,
    moves: &LadderMoves,
    required_at_commanded: f64,
    gate_available_power: f64,
    still_required: f64,
    still_available: f64,
) {
    // ── Rung 5: report ────────────────────────────────────────
    //
    // `PowerLimited` means "the power constraint bound". It carries
    // the BEFORE pair, which is what it carried pre-ladder and what
    // makes it self-explaining: `required > available` always holds,
    // so the warning states the conflict that caused the change
    // rather than the state after it. `power_ceiling_parity_f2`
    // asserts exactly that.
    //
    // What the engine DID about it, and whether the cut fits now,
    // is `PowerLadderReducedCut`'s job. Splitting them this way
    // keeps each warning true on its own terms: one names the
    // constraint, the other names the response.
    s.feed.power_limited = true;
    s.warnings.push(FeedsWarning::PowerLimited {
        required_kw: required_at_commanded,
        available_kw: gate_available_power,
    });
    if moves.moved_rpm
        || moves.ap_from.is_some()
        || moves.ae_from.is_some()
        || moves.feed_factor.is_some()
    {
        s.warnings.push(FeedsWarning::PowerLadderReducedCut {
            rpm_from: moves.moved_rpm.then_some(moves.rpm_before),
            rpm_to: moves.moved_rpm.then_some(s.speed.rpm),
            axial_from: moves.ap_from,
            axial_to: moves.ap_from.map(|_| s.cut.ap),
            radial_from: moves.ae_from,
            radial_to: moves.ae_from.map(|_| s.cut.ae),
            feed_factor: moves.feed_factor,
            required_kw_before: required_at_commanded,
            required_kw_after: still_required,
            available_kw: still_available,
        });
    }
}
