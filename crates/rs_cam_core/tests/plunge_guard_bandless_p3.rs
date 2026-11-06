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
//!    ceiling and says `MachineMaxFeed`; feeds contract P18 (review D15,
//!    lead ruling) adds the kinematic reach at the cutting ceiling as the
//!    arm's third cap (`KinematicReach`), the rule of the no-engagement
//!    arm;
//! 3. the BANDED arm on the same fixtures is unchanged (regression
//!    guard: `Some(band)` behaviour stays byte-identical).

#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_core::dressup::feed_modulation::{
    BindingConstraint, ChiploadBand, ModulationContext, ModulationStrategy, PerMoveEngagement,
    adaptive_feed_modulate,
};
use rs_cam_core::geo::P3;
use rs_cam_core::machine::kinematics::{MachineKinematics, predicted_feeds_for_toolpath};
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
        cutting_ceiling: cutting_ceiling(MAX_FEED),
        rapid_feed_mm_min: MAX_FEED,
        chipload_band: None,
        radial_thinning: rs_cam_core::dressup::feed_modulation::RadialThinning::Applied,
        kinematics: k,
        strategy: ModulationStrategy::ConstrainedMax,
        feed_scale: 1.0,
        deflection_inputs: None,
        power_inputs: None,
        force_budget_inputs: None,
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
        thinning_lift: None,
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
    // plunge rate means no guard. Commanded 1500 < MAX_FEED.
    // Re-pinned by feeds contract P18 (review D15, lead ruling): the
    // bandless arm also takes the kinematic reach at the cutting ceiling.
    // A 2 mm vertical move from rest to rest cannot reach 1500 mm/min on
    // the Z axis of the stock Shapeoko, so the feed is the reach, computed
    // here by the public integrator on the probe (every move at the
    // ceiling). Before P18 the feed stayed at the commanded 1500.
    let k = machine();
    let mut tp = one_move(vertical(2.0), MoveIntent::ClearingCut);
    let reach = reach_of(&tp, &k);
    assert!(reach < COMMANDED, "non-vacuity: the reach binds ({reach})");
    let (feed, binding) = modulate_with(&mut tp, &bandless_ctx(&k, f64::INFINITY));
    assert_eq!(feed, reach);
    assert_eq!(binding, Some(BindingConstraint::KinematicReach));
}

/// The reach of move 1 at the cutting ceiling: every move commanded at
/// `MAX_FEED`, through the integrator the modulator's probe uses.
fn reach_of(tp: &Toolpath, k: &MachineKinematics) -> f64 {
    let mut probe = tp.clone();
    for m in probe.moves.iter_mut() {
        m.move_type = m.move_type.with_feed_rate(MAX_FEED);
    }
    predicted_feeds_for_toolpath(&probe, k, MAX_FEED, MAX_FEED)
        .get(&1)
        .copied()
        .expect("move 1 is fed")
}

// ---------------------------------------------------------------
// 2 — the bandless arm's only other cap: the machine ceiling.
// ---------------------------------------------------------------

#[test]
fn bandless_lateral_move_above_machine_ceiling_is_clamped() {
    let k = machine();
    // A lateral move commanded PAST the machine ceiling, with the guard
    // disabled, so only the ceiling can act.
    // P18: the move is long enough that the reach at the ceiling is the
    // ceiling (a 50 mm move reached only 6708 mm/min from rest to rest), so
    // the ceiling is the cap that acts, as this case intends.
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(0.0, 0.0, 0.0));
    tp.feed_to_with_intent(
        P3::new(1000.0, 0.0, 0.0),
        MAX_FEED * 2.0,
        MoveIntent::ClearingCut,
    );
    assert!(
        reach_of(&tp, &k) >= MAX_FEED,
        "non-vacuity: the reach does not bind"
    );
    let ctx = {
        let mut c = bandless_ctx(&k, f64::INFINITY);
        c.cutting_ceiling = cutting_ceiling(MAX_FEED);
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

/// F002: a cutting ceiling of `mm_min`, made the one way, by a machine
/// profile whose travel rate and explicit cutting cap are both `mm_min`.
fn cutting_ceiling(mm_min: f64) -> rs_cam_core::machine::CuttingCeilingMmMin {
    rs_cam_core::machine::MachineProfile {
        max_feed_mm_min: mm_min,
        max_cutting_feed_mm_min: Some(mm_min),
        ..rs_cam_core::machine::MachineProfile::default()
    }
    .cutting_ceiling()
}
