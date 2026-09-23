//! Phase 3 sentry, BANDLESS arm — the geometric plunge guard must hold
//! with NO vendor chipload band (`planning/wanaka200_feeds_check_2026-09-19/`
//! `IMPLEMENTATION_PLAN.md` work item A).
//!
//! The defect: `apply_adaptive_feed_modulation` skipped every toolpath
//! without a LUT chipload band, so the modulator — and the geometric
//! plunge guard living inside it — never ran for bandless ops. A 20°
//! V-bit ProjectCurve (no V-bit contour rows in the LUT) emitted a
//! 1165 mm/min vertical descent against its operation's own
//! 400 mm/min plunge rate (2.9x), measured by simulation at move 3700.
//!
//! The fix widens `ModulationContext::chipload_band` to `Option`: a
//! bandless run does NO chipload targeting — only the machine
//! cutting-feed ceiling and the geometric plunge guard bind.
//!
//! These sentries pin the three things that can drift:
//!
//! 1. a bandless untagged vertical descent IS capped, and says
//!    `PlungeRate` — the incident, in one move, without a band;
//! 2. a bandless move above the machine ceiling is clamped to the
//!    ceiling and says `MachineMaxFeed` — the bandless arm's only
//!    other cap;
//! 3. the BANDED arm on the same fixtures is unchanged (regression
//!    guard: `Some(band)` behaviour stays byte-identical).

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
/// Three times the plunge rate — the shape of the wanaka200 incident.
const COMMANDED: f64 = 3.0 * PLUNGE_RATE;
/// Machine cutting-feed ceiling for the bandless arm's only other cap.
const MAX_FEED: f64 = 10_000.0;

fn machine() -> MachineKinematics {
    MachineKinematics::shapeoko_xxl_stock()
}

/// The wanaka200 incident's context, bandless by construction.
fn bandless_ctx<'a>(k: &'a MachineKinematics, plunge_rate_mm_min: f64) -> ModulationContext<'a> {
    ModulationContext {
        spindle_rpm: 18_000.0,
        flute_count: 2,
        max_feed_mm_min: MAX_FEED,
        rapid_feed_mm_min: MAX_FEED,
        chipload_band: None,
        kinematics: k,
        strategy: ModulationStrategy::ConstrainedMax,
        aggressiveness: 1.0,
        deflection_inputs: None,
        power_inputs: None,
        nominal_axial_doc_mm: 0.0,
        plunge_rate_mm_min,
    }
}

/// Same context, BANDED — the regression arm.
fn banded_ctx<'a>(k: &'a MachineKinematics, plunge_rate_mm_min: f64) -> ModulationContext<'a> {
    let mut ctx = bandless_ctx(k, plunge_rate_mm_min);
    ctx.chipload_band = Some(ChiploadBand::new(0.02, 0.08).expect("valid band"));
    ctx
}

/// Full engagement, so the banded constrained-max solver lifts the move
/// rather than short-circuiting on air.
fn engaged() -> PerMoveEngagement {
    PerMoveEngagement {
        radial_woc_fraction: 1.0,
        axial_doc_fraction: 1.0,
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

/// Pure-vertical descent of `length` mm.
fn vertical(length: f64) -> P3 {
    P3::new(0.0, 0.0, -length)
}

fn modulate_with(
    tp: &mut Toolpath,
    ctx: &ModulationContext<'_>,
) -> (f64, Option<BindingConstraint>) {
    let engagements: Vec<PerMoveEngagement> = tp
        .moves
        .iter()
        .map(|m| {
            if m.move_type.feed_rate().is_some() {
                engaged()
            } else {
                PerMoveEngagement::default()
            }
        })
        .collect();
    let outcome = adaptive_feed_modulate(tp, &engagements, ctx).unwrap();
    let feed = tp.moves[1].move_type.feed_rate().unwrap();
    (feed, outcome.per_move.get(&1).map(|(_, b)| *b))
}

// ---------------------------------------------------------------
// 1 — the incident, bandless: an untagged vertical descent is
//     capped at the plunge rate and says `PlungeRate`.
// ---------------------------------------------------------------

#[test]
fn bandless_untagged_vertical_descent_is_capped_at_plunge_rate() {
    let k = machine();
    let mut tp = one_move(vertical(2.0), MoveIntent::ClearingCut);
    let (feed, binding) = modulate_with(&mut tp, &bandless_ctx(&k, PLUNGE_RATE));
    assert_eq!(
        feed, PLUNGE_RATE,
        "bandless descent must sit at the plunge rate"
    );
    assert_eq!(binding, Some(BindingConstraint::PlungeRate));
}

#[test]
fn bandless_guard_disabled_by_infinity_leaves_commanded_feed() {
    // Phase 3's documented disable dial: a non-positive or non-finite
    // plunge rate means no guard. Commanded 1500 < MAX_FEED, so the
    // bandless arm leaves it untouched.
    let k = machine();
    let mut tp = one_move(vertical(2.0), MoveIntent::ClearingCut);
    let (feed, binding) = modulate_with(&mut tp, &bandless_ctx(&k, f64::INFINITY));
    assert_eq!(feed, COMMANDED);
    assert_eq!(binding, Some(BindingConstraint::MachineMaxFeed));
}

// ---------------------------------------------------------------
// 2 — the bandless arm's only other cap: the machine ceiling.
// ---------------------------------------------------------------

#[test]
fn bandless_lateral_move_above_machine_ceiling_is_clamped() {
    let k = machine();
    // A lateral move commanded PAST the machine ceiling, with the guard
    // disabled, so only the ceiling can act.
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(0.0, 0.0, 0.0));
    tp.feed_to_with_intent(
        P3::new(50.0, 0.0, 0.0),
        MAX_FEED * 2.0,
        MoveIntent::ClearingCut,
    );
    let ctx = {
        let mut c = bandless_ctx(&k, f64::INFINITY);
        c.max_feed_mm_min = MAX_FEED;
        c
    };
    let (feed, binding) = modulate_with(&mut tp, &ctx);
    assert_eq!(
        feed, MAX_FEED,
        "bandless feed must clamp at the machine ceiling"
    );
    assert_eq!(binding, Some(BindingConstraint::MachineMaxFeed));
}

// ---------------------------------------------------------------
// 3 — regression: the BANDED arm on the same fixture still lifts an
//     engaged move to the band maximum, and still caps the descent.
// ---------------------------------------------------------------

#[test]
fn banded_arm_on_the_same_fixture_is_unchanged() {
    let k = machine();
    // Engaged lateral move: banded ConstrainedMax reaches the band
    // ceiling 0.08 mm/tooth × 18000 rpm × 2 flutes = 2880 mm/min.
    let mut tp_lateral = Toolpath::new();
    tp_lateral.rapid_to(P3::new(0.0, 0.0, 0.0));
    tp_lateral.feed_to_with_intent(P3::new(50.0, 0.0, 0.0), COMMANDED, MoveIntent::ClearingCut);
    let (feed, _) = modulate_with(&mut tp_lateral, &banded_ctx(&k, f64::INFINITY));
    assert!(
        (feed - 2880.0).abs() < 1e-6,
        "banded arm must still lift to the band ceiling 2880, got {feed}"
    );

    // Engaged vertical descent: the guard still caps at the plunge
    // rate THROUGH the banded lift.
    let mut tp_descent = one_move(vertical(2.0), MoveIntent::ClearingCut);
    let (feed, binding) = modulate_with(&mut tp_descent, &banded_ctx(&k, PLUNGE_RATE));
    assert_eq!(feed, PLUNGE_RATE);
    assert_eq!(binding, Some(BindingConstraint::PlungeRate));
}
