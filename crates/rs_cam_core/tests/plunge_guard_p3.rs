//! Phase 3 sentries — the modulator's GEOMETRIC plunge guard
//! (`planning/machine_kinematics_confidence_2026-09-07.md`).
//!
//! The defect: [`rs_cam_core::dressup::feed_modulation::adaptive_feed_modulate`]
//! skipped plunges by INTENT tag only. The adaptive3d rough emits its
//! vertical step-down and re-entry descents as plain cutting moves, so
//! they fell through the filter and every strategy lifted them to the
//! lateral chipload-band maximum — 1807 mm/min against a 512 mm/min
//! plunge rate on the wanaka front rough.
//!
//! The fix is a cap the move's own GEOMETRY decides, taken from the
//! shared classifier
//! [`rs_cam_core::machine::kinematic_utilization::classify_move`], so the guard
//! and the Phase 2 instrument can never disagree about what a plunge is.
//!
//! These sentries pin the four things that can drift:
//!
//! 1. an untagged vertical descent IS capped, and says `PlungeRate` —
//!    including one the simulator read as UNENGAGED, which the modulator
//!    never used to alter;
//! 2. a TAGGED plunge still owns its feed — geometry was not folded into
//!    the intent skip;
//! 3. the 15-degree class boundary, bracketed at 14 and 16 degrees
//!    THROUGH THE MODULATOR, not through the classifier alone;
//! 4. the guard is a no-op on every other class, and on a plunge that is
//!    already slow enough.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::dressup::feed_modulation::{
    BindingConstraint, ChiploadBand, ModulationContext, ModulationStrategy, PerMoveEngagement,
    adaptive_feed_modulate,
};
use rs_cam_core::geo::P3;
use rs_cam_core::machine::kinematics::MachineKinematics;
use rs_cam_core::toolpath::{MoveIntent, Toolpath};

/// The operation plunge rate every fixture here is graded against.
const PLUNGE_RATE: f64 = 500.0;
/// Three times the plunge rate — the shape of the incident.
const COMMANDED: f64 = 3.0 * PLUNGE_RATE;
/// `0.08 mm/tooth x 18000 rpm x 2 flutes` — the lateral band maximum the
/// modulator lifts an engaged move to.
const BAND_MAX_FEED: f64 = 2880.0;

fn machine() -> MachineKinematics {
    MachineKinematics::shapeoko_xxl_stock()
}

/// A context whose only interesting dial is `plunge_rate_mm_min`.
///
/// `f64::INFINITY` disables the guard without touching the intent skip,
/// which is how the "with and without" arms below stay honest.
fn ctx<'a>(k: &'a MachineKinematics, plunge_rate_mm_min: f64) -> ModulationContext<'a> {
    ModulationContext {
        spindle_rpm: 18_000.0,
        flute_count: 2,
        max_feed_mm_min: 10_000.0,
        rapid_feed_mm_min: 10_000.0,
        chipload_band: Some(ChiploadBand::new(0.02, 0.08).expect("valid band")),
        kinematics: k,
        strategy: ModulationStrategy::ConstrainedMax,
        feed_scale: 1.0,
        deflection_inputs: None,
        power_inputs: None,
        nominal_axial_doc_mm: 0.0,
        plunge_rate_mm_min,
    }
}

/// Full engagement, so the constrained-max solver lifts the move rather
/// than short-circuiting on air.
fn engaged() -> PerMoveEngagement {
    PerMoveEngagement {
        radial_woc_fraction: 1.0,
        axial_doc_fraction: 1.0,
        // T-11: this fixture disables both caps (`deflection_inputs` and
        // `power_inputs` are `None`), so the depth is never read. `0.0`
        // is what the pre-T-11 expression returned here, kept so this
        // plunge-guard test cannot shift on a depth it does not test.
        axial_doc_mm: 0.0,
    }
}

/// One fed move from the origin to `to`, seeded by a rapid so the fed
/// move is index 1 and the guard has a predecessor to read.
fn one_move(to: P3, intent: MoveIntent) -> Toolpath {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(0.0, 0.0, 0.0));
    tp.feed_to_with_intent(to, COMMANDED, intent);
    tp
}

/// End point of a descent `deg` degrees from VERTICAL, `length` mm long.
fn descent(deg: f64, length: f64) -> P3 {
    let a = deg.to_radians();
    P3::new(length * a.sin(), 0.0, -length * a.cos())
}

fn modulate(tp: &mut Toolpath, plunge_rate: f64) -> (f64, Option<BindingConstraint>) {
    modulate_at(tp, plunge_rate, engaged())
}

/// [`modulate`] with the engagement summary chosen by the caller, so a
/// fixture can drive the solver's no-engagement branch on purpose.
fn modulate_at(
    tp: &mut Toolpath,
    plunge_rate: f64,
    engagement: PerMoveEngagement,
) -> (f64, Option<BindingConstraint>) {
    let k = machine();
    let c = ctx(&k, plunge_rate);
    let engagements: Vec<PerMoveEngagement> = tp
        .moves
        .iter()
        .map(|m| {
            if m.move_type.feed_rate().is_some() {
                engagement
            } else {
                PerMoveEngagement::default()
            }
        })
        .collect();
    let outcome = adaptive_feed_modulate(tp, &engagements, &c).unwrap();
    let feed = tp.moves[1].move_type.feed_rate().unwrap();
    (feed, outcome.per_move.get(&1).map(|(_, b)| *b))
}

// ---------------------------------------------------------------
// 1 — an UNTAGGED vertical descent is capped at the plunge rate.
// ---------------------------------------------------------------

/// The incident, in one move. A pure-vertical fed descent tagged
/// `ClearingCut` — the tag the adaptive3d rough actually emits — carries
/// three times the plunge rate and is fully engaged, so every candidate
/// limit in the constrained-max solver sits above it. Pre-Phase-3 the
/// modulator lifted it to the lateral band maximum. It must now land
/// exactly on the plunge rate, and the binding must SAY so.
#[test]
fn untagged_vertical_descent_is_capped_at_the_plunge_rate() {
    let mut tp = one_move(P3::new(0.0, 0.0, -20.0), MoveIntent::ClearingCut);
    let (feed, binding) = modulate(&mut tp, PLUNGE_RATE);
    assert!(
        (feed - PLUNGE_RATE).abs() < 1e-9,
        "a vertical descent must land on the plunge rate {PLUNGE_RATE}, got {feed}"
    );
    assert_eq!(
        binding,
        Some(BindingConstraint::PlungeRate),
        "the binding must name the cap that actually held the feed"
    );

    // The control: with the guard disabled the SAME move is lifted to the
    // lateral band maximum. Without this arm the assertion above could
    // pass on a move nothing wanted to lift.
    let mut tp = one_move(P3::new(0.0, 0.0, -20.0), MoveIntent::ClearingCut);
    let (lifted, lifted_binding) = modulate(&mut tp, f64::INFINITY);
    assert!(
        (lifted - BAND_MAX_FEED).abs() < 1.0,
        "pre-guard this move is lifted to the band maximum {BAND_MAX_FEED}, got {lifted}"
    );
    assert_eq!(lifted_binding, Some(BindingConstraint::ChiploadMax));
    assert!(
        lifted > PLUNGE_RATE * 3.0,
        "the control arm must be well above the plunge rate, or the sentry is vacuous"
    );
}

/// A ZERO-ENGAGEMENT vertical descent is capped too — and this is a move
/// the modulator never used to alter at all.
///
/// The `ConstrainedMax` solver short-circuits an unengaged move: it
/// returns `(commanded, MachineMaxFeed)` and rewrites nothing. The guard
/// sits AFTER that branch, so an untagged descent the simulator read as
/// air is now lowered to the plunge rate. That is a deliberate widening
/// of the modulator's reach, not a side effect, and it is physically
/// right: engagement is measured where the tool HAS BEEN, and a vertical
/// descent through air ends in material. The dexel reading is also the
/// weakest evidence in the pipeline here — a sub-`FRESH_MATERIAL_THRESHOLD_MM`
/// pass reads a hard zero while removing material (CLAUDE.md), so
/// "engagement 0" is not a licence to descend at the lateral feed.
///
/// Not covered by the guard: a plunge carrying a plunge INTENT. The
/// sentry below owns that half.
#[test]
fn zero_engagement_vertical_descent_is_capped_too() {
    let mut tp = one_move(P3::new(0.0, 0.0, -20.0), MoveIntent::ClearingCut);
    let (feed, binding) = modulate_at(&mut tp, PLUNGE_RATE, PerMoveEngagement::default());
    assert!(
        (feed - PLUNGE_RATE).abs() < 1e-9,
        "an unengaged vertical descent must still land on the plunge rate \
         {PLUNGE_RATE}, got {feed}"
    );
    assert_eq!(
        binding,
        Some(BindingConstraint::PlungeRate),
        "the binding must name the plunge cap, not the solver's no-engagement default"
    );

    // The control: without the guard this move is UNTOUCHED at its
    // commanded feed and reports the no-engagement default. That is the
    // behaviour this sentry records as deliberately changed.
    let mut tp = one_move(P3::new(0.0, 0.0, -20.0), MoveIntent::ClearingCut);
    let (untouched, untouched_binding) =
        modulate_at(&mut tp, f64::INFINITY, PerMoveEngagement::default());
    assert!(
        (untouched - COMMANDED).abs() < 1e-9,
        "pre-guard an unengaged move keeps its commanded feed {COMMANDED}, got {untouched}"
    );
    assert_eq!(untouched_binding, Some(BindingConstraint::MachineMaxFeed));
}

// ---------------------------------------------------------------
// 2 — a TAGGED plunge is still the intent skip's, not the guard's.
// ---------------------------------------------------------------

/// `should_skip_modulation` stays intent-based. A move tagged
/// `EntryPlunge` keeps its commanded feed EXACTLY, and the modulator
/// records no per-move entry for it at all — the guard did not fold
/// geometry into the skip, and an operator-tuned entry feed above the
/// plunge rate is still the operator's.
#[test]
fn tagged_entry_plunge_keeps_its_commanded_feed() {
    let mut tp = one_move(P3::new(0.0, 0.0, -20.0), MoveIntent::EntryPlunge);
    let (feed, binding) = modulate(&mut tp, PLUNGE_RATE);
    assert!(
        (feed - COMMANDED).abs() < 1e-9,
        "a tagged EntryPlunge keeps its commanded feed {COMMANDED}, got {feed}"
    );
    assert_eq!(
        binding, None,
        "a skipped move must be ABSENT from the per-move map, not recorded as capped"
    );
}

// ---------------------------------------------------------------
// 3 — the 15-degree class boundary, through the modulator.
// ---------------------------------------------------------------

/// Angles are measured from VERTICAL. 14 degrees is inside the plunge
/// cone and is capped; 16 degrees is a ramp, whose flutes cut laterally,
/// and the chipload band governs it.
///
/// The bracket runs through `adaptive_feed_modulate`, not through
/// `classify_move` alone: the Phase 2 sentries already pin the
/// classifier, and what can drift is whether the MODULATOR consults it.
#[test]
fn plunge_cone_brackets_at_fourteen_and_sixteen_degrees_through_the_modulator() {
    let mut inside = one_move(descent(14.0, 20.0), MoveIntent::ClearingCut);
    let (feed_14, binding_14) = modulate(&mut inside, PLUNGE_RATE);
    assert!(
        (feed_14 - PLUNGE_RATE).abs() < 1e-9,
        "14 degrees from vertical is plunge-class and must be capped; got {feed_14}"
    );
    assert_eq!(binding_14, Some(BindingConstraint::PlungeRate));

    let mut outside = one_move(descent(16.0, 20.0), MoveIntent::ClearingCut);
    let (feed_16, binding_16) = modulate(&mut outside, PLUNGE_RATE);
    assert!(
        feed_16 > PLUNGE_RATE,
        "16 degrees from vertical is a ramp and must NOT be capped; got {feed_16}"
    );
    assert_ne!(
        binding_16,
        Some(BindingConstraint::PlungeRate),
        "a ramp must keep the strategy's own binding"
    );
    // Non-vacuous: the ramp really was lifted well past the plunge rate,
    // so "not capped" is a measurement and not an accident of the fixture.
    assert!(
        (feed_16 - BAND_MAX_FEED).abs() < 1.0,
        "the 16-degree arm must reach the band maximum {BAND_MAX_FEED}, got {feed_16}"
    );
}

// ---------------------------------------------------------------
// 4 — the guard is a no-op on every other class.
// ---------------------------------------------------------------

/// A 20-degree-from-vertical ramp and a lateral move at the band
/// maximum: byte-identical feeds with the guard armed and disarmed.
///
/// Compared on `to_bits`, not on a tolerance. The Phase 3 A/B bar is
/// "every `Lateral` / `Ramp` move's feed is byte-identical across the
/// arms", and a tolerance here would not be that claim.
#[test]
fn ramp_and_lateral_feeds_are_byte_identical_with_and_without_the_guard() {
    let build = || {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 0.0));
        // A 20-degree-from-vertical ramp — outside the plunge cone.
        tp.feed_to_with_intent(descent(20.0, 25.0), COMMANDED, MoveIntent::ClearingCut);
        // A lateral cut at constant Z.
        let start = descent(20.0, 25.0);
        tp.feed_to_with_intent(
            P3::new(start.x + 40.0, 0.0, start.z),
            COMMANDED,
            MoveIntent::ClearingCut,
        );
        tp
    };
    let mut armed = build();
    let mut disarmed = build();
    let _ = modulate(&mut armed, PLUNGE_RATE);
    let _ = modulate(&mut disarmed, f64::INFINITY);
    for i in 1..=2 {
        let a = armed.moves[i].move_type.feed_rate().unwrap();
        let d = disarmed.moves[i].move_type.feed_rate().unwrap();
        assert_eq!(
            a.to_bits(),
            d.to_bits(),
            "move {i}: the guard changed a non-plunge feed ({a} vs {d})"
        );
        assert!(
            a > PLUNGE_RATE,
            "move {i} must sit above the plunge rate, or this sentry proves nothing: {a}"
        );
    }
}

/// A plunge-class move the strategy already holds at or below the plunge
/// rate is UNTOUCHED, and it keeps the strategy's own binding. The guard
/// is a floor under the lift, never a rewrite of the diagnostic.
#[test]
fn plunge_already_slow_enough_keeps_the_strategy_binding() {
    // Plunge rate well above the band maximum, so the solver's own answer
    // is the smaller number.
    let generous = BAND_MAX_FEED * 2.0;
    let mut guarded = one_move(P3::new(0.0, 0.0, -20.0), MoveIntent::ClearingCut);
    let (feed, binding) = modulate(&mut guarded, generous);
    let mut control = one_move(P3::new(0.0, 0.0, -20.0), MoveIntent::ClearingCut);
    let (control_feed, control_binding) = modulate(&mut control, f64::INFINITY);
    assert_eq!(
        feed.to_bits(),
        control_feed.to_bits(),
        "an already-slow-enough plunge must be byte-identical to the unguarded run"
    );
    assert_eq!(
        binding, control_binding,
        "the binding must stay the strategy's, not become PlungeRate"
    );
    assert_ne!(binding, Some(BindingConstraint::PlungeRate));
}

/// The seed move has no predecessor, so it has no vector and cannot be
/// classified. It is left alone — the same rule
/// `kinematic_utilization::analyse_toolpath` applies when it skips move
/// 0. Inferring a delta from the origin would classify a move the
/// machine may never make.
#[test]
fn seed_move_has_no_predecessor_and_is_not_capped() {
    let mut tp = Toolpath::new();
    tp.feed_to_with_intent(P3::new(0.0, 0.0, -20.0), COMMANDED, MoveIntent::ClearingCut);
    let k = machine();
    let c = ctx(&k, PLUNGE_RATE);
    let outcome = adaptive_feed_modulate(&mut tp, &[engaged()], &c).unwrap();
    let feed = tp.moves[0].move_type.feed_rate().unwrap();
    assert!(
        feed > PLUNGE_RATE,
        "the seed move carries no vector and must not be capped; got {feed}"
    );
    assert_ne!(
        outcome.per_move.get(&0).map(|(_, b)| *b),
        Some(BindingConstraint::PlungeRate)
    );
}
