//! Feeds contract P18 (review D15, lead ruling option 1): a ConstrainedMax
//! move with no engagement takes the kinematic reach cap, as every engaged
//! arm does.
//!
//! Source: `planning/feeds_review_2026-11-06/RECHECK_bd72997c9.md` row D15.
//! On `P_thin_005` (a 0.3 mm width on a 0.5 mm simulation cell) the
//! simulation sees no engagement on real cuts, and the modulator left 13
//! moves at F9999 that a 200 mm/s^2 machine cannot reach. Before P18 the
//! no-engagement arm was `min(commanded, cutting ceiling)`.
//!
//! The reach is the machine's: the probe runs every move at the cutting
//! ceiling (a move with no engagement has no chip, so the band ceiling of
//! the engaged probe does not apply to it). The test computes the reach
//! with the public integrator, `predicted_feeds_for_toolpath`, on the same
//! probe.

#![expect(
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_core::dressup::feed_modulation::{
    BindingConstraint, ChiploadBand, ModulationContext, ModulationStrategy, PerMoveEngagement,
    RadialThinning, adaptive_feed_modulate,
};
use rs_cam_core::geo::P3;
use rs_cam_core::machine::kinematics::{MachineKinematics, predicted_feeds_for_toolpath};
use rs_cam_core::machine::{CuttingCeilingMmMin, MachineProfile};
use rs_cam_core::toolpath::{MoveIntent, Toolpath};

/// The commanded feed of cell `P_thin_005` (Suggest's lifted feed).
const COMMANDED_MM_MIN: f64 = 9999.0;

fn ceiling() -> CuttingCeilingMmMin {
    // 10000 mm/min: above the commanded feed, so the ceiling does not bind
    // and only the reach can lower the feed (the F039 fixture pattern).
    MachineProfile {
        max_feed_mm_min: 10_000.0,
        max_cutting_feed_mm_min: Some(10_000.0),
        ..MachineProfile::default()
    }
    .cutting_ceiling()
}

/// Short fed moves with right-angle corners: the machine cannot reach
/// 9999 mm/min on a 4 mm leg.
fn zigzag() -> Toolpath {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(0.0, 0.0, -2.0));
    for i in 0..8 {
        let x = f64::from(i + 1) * 4.0;
        let y = if i % 2 == 0 { 4.0 } else { 0.0 };
        tp.feed_to_with_intent(
            P3::new(x - 4.0, y, -2.0),
            COMMANDED_MM_MIN,
            MoveIntent::ClearingCut,
        );
        tp.feed_to_with_intent(
            P3::new(x, y, -2.0),
            COMMANDED_MM_MIN,
            MoveIntent::ClearingCut,
        );
    }
    tp
}

#[test]
fn a_move_with_no_engagement_is_capped_at_its_reach_p18() {
    let k = MachineKinematics::shapeoko_xxl_stock();
    let cut_ceiling = ceiling();
    let ctx = ModulationContext {
        spindle_rpm: 18_000.0,
        flute_count: 2,
        cutting_ceiling: cut_ceiling,
        rapid_feed_mm_min: 10_000.0,
        chipload_band: Some(ChiploadBand::new(0.02, 0.08).expect("valid band")),
        radial_thinning: RadialThinning::Applied,
        kinematics: &k,
        strategy: ModulationStrategy::ConstrainedMax,
        mode: rs_cam_core::dressup::feed_modulation::ModulationMode::Full,
        feed_scale: 1.0,
        deflection_inputs: None,
        power_inputs: None,
        force_budget_inputs: None,
        sample_engagements: &[],
        nominal_axial_doc_mm: 2.0,
        plunge_rate_mm_min: 300.0,
    };
    let mut tp = zigzag();
    // Every move has no engagement (the simulation cell cannot see the cut).
    let engagements = vec![PerMoveEngagement::default(); tp.moves.len()];

    // The reach at the cutting ceiling: every move commanded at it.
    let mut probe = tp.clone();
    for m in probe.moves.iter_mut() {
        m.move_type = m.move_type.with_feed_rate(cut_ceiling.mm_min());
    }
    let reach = predicted_feeds_for_toolpath(&probe, &k, cut_ceiling.mm_min(), 10_000.0);

    let outcome = adaptive_feed_modulate(&mut tp, &engagements, &ctx).expect("modulates");
    let mut capped = 0;
    for (i, m) in tp.moves.iter().enumerate() {
        let Some(emitted) = m.move_type.feed_rate() else {
            continue;
        };
        let want = reach
            .get(&i)
            .copied()
            .map_or(COMMANDED_MM_MIN, |r| COMMANDED_MM_MIN.min(r.max(1e-3)));
        assert!(
            (emitted - want).abs() < 1e-9,
            "move {i}: emitted {emitted}, reach cap {want}"
        );
        assert!(emitted <= COMMANDED_MM_MIN, "move {i}: the feed rose");
        if want < COMMANDED_MM_MIN {
            capped += 1;
            assert_eq!(
                outcome.per_move.get(&i).map(|(_, b)| *b),
                Some(BindingConstraint::KinematicReach),
                "move {i}"
            );
        }
    }
    // Non-vacuity: the reach binds on the short legs.
    assert!(capped >= 4, "the reach binds on {capped} moves");
}

/// The bandless arm (lead ruling, P18): the path of a V-bit Project Curve,
/// where no vendor row gives a chipload band. The moves are engaged, but
/// the arm has no chip target, so it takes the same rule: the commanded
/// feed under the ceiling and under the reach at the ceiling.
#[test]
fn a_bandless_move_is_capped_at_its_reach_p18() {
    let k = MachineKinematics::shapeoko_xxl_stock();
    let cut_ceiling = ceiling();
    let ctx = ModulationContext {
        spindle_rpm: 18_000.0,
        flute_count: 2,
        cutting_ceiling: cut_ceiling,
        rapid_feed_mm_min: 10_000.0,
        chipload_band: None,
        radial_thinning: RadialThinning::Applied,
        kinematics: &k,
        strategy: ModulationStrategy::ConstrainedMax,
        mode: rs_cam_core::dressup::feed_modulation::ModulationMode::Full,
        feed_scale: 1.0,
        deflection_inputs: None,
        power_inputs: None,
        force_budget_inputs: None,
        sample_engagements: &[],
        nominal_axial_doc_mm: 2.0,
        plunge_rate_mm_min: 300.0,
    };
    let mut tp = zigzag();
    // A V-bit trace engages its cone along the curve.
    let engaged = PerMoveEngagement {
        radial_woc_fraction: 1.0,
        axial_doc_fraction: 1.0,
        axial_doc_mm: 2.0,
        thinning_lift: None,
    };
    let engagements: Vec<PerMoveEngagement> = tp
        .moves
        .iter()
        .map(|m| {
            if m.move_type.feed_rate().is_some() {
                engaged
            } else {
                PerMoveEngagement::default()
            }
        })
        .collect();
    let mut probe = tp.clone();
    for m in probe.moves.iter_mut() {
        m.move_type = m.move_type.with_feed_rate(cut_ceiling.mm_min());
    }
    let reach = predicted_feeds_for_toolpath(&probe, &k, cut_ceiling.mm_min(), 10_000.0);

    let outcome = adaptive_feed_modulate(&mut tp, &engagements, &ctx).expect("modulates");
    let mut capped = 0;
    for (i, m) in tp.moves.iter().enumerate() {
        let Some(emitted) = m.move_type.feed_rate() else {
            continue;
        };
        let want = reach
            .get(&i)
            .copied()
            .map_or(COMMANDED_MM_MIN, |r| COMMANDED_MM_MIN.min(r.max(1e-3)));
        assert!(
            (emitted - want).abs() < 1e-9,
            "move {i}: emitted {emitted}, reach cap {want}"
        );
        if want < COMMANDED_MM_MIN {
            capped += 1;
            assert_eq!(
                outcome.per_move.get(&i).map(|(_, b)| *b),
                Some(BindingConstraint::KinematicReach),
                "move {i}"
            );
        }
    }
    assert!(capped >= 4, "the reach binds on {capped} moves");
}

/// Lead rule (P18 4a): the change moves no ENGAGED move. A mixed path of
/// engaged and no-engagement moves runs twice: once with the no-engagement
/// moves commanded at 9999 mm/min (the reach cap binds), once at 1 mm/min
/// (no cap binds). The engaged arms read the probe, which commands every
/// move at one feed, so the commanded feed of a no-engagement move cannot
/// reach them: every engaged move emits the same feed, bit for bit.
#[test]
fn the_cap_moves_no_engaged_move_p18() {
    let k = MachineKinematics::shapeoko_xxl_stock();
    let ctx = ModulationContext {
        spindle_rpm: 18_000.0,
        flute_count: 2,
        cutting_ceiling: ceiling(),
        rapid_feed_mm_min: 10_000.0,
        chipload_band: Some(ChiploadBand::new(0.02, 0.08).expect("valid band")),
        radial_thinning: RadialThinning::Applied,
        kinematics: &k,
        strategy: ModulationStrategy::ConstrainedMax,
        mode: rs_cam_core::dressup::feed_modulation::ModulationMode::Full,
        feed_scale: 1.0,
        deflection_inputs: None,
        power_inputs: None,
        force_budget_inputs: None,
        sample_engagements: &[],
        nominal_axial_doc_mm: 2.0,
        plunge_rate_mm_min: 300.0,
    };
    let engaged = PerMoveEngagement {
        radial_woc_fraction: 0.5,
        axial_doc_fraction: 1.0,
        axial_doc_mm: 2.0,
        thinning_lift: None,
    };
    // Every third fed move has no engagement.
    let base = zigzag();
    let engagements: Vec<PerMoveEngagement> = base
        .moves
        .iter()
        .enumerate()
        .map(|(i, m)| {
            if m.move_type.feed_rate().is_some() && i % 3 != 0 {
                engaged
            } else {
                PerMoveEngagement::default()
            }
        })
        .collect();
    let run = |no_engagement_feed: f64| {
        let mut tp = base.clone();
        for (m, e) in tp.moves.iter_mut().zip(&engagements) {
            if m.move_type.feed_rate().is_some() && e.radial_woc_fraction <= 1e-6 {
                m.move_type = m.move_type.with_feed_rate(no_engagement_feed);
            }
        }
        let _ = adaptive_feed_modulate(&mut tp, &engagements, &ctx).expect("modulates");
        tp
    };
    let capped = run(COMMANDED_MM_MIN);
    let free = run(1.0);
    let mut engaged_moves = 0;
    let mut no_engagement_differs = 0;
    for (i, ((a, b), e)) in capped
        .moves
        .iter()
        .zip(&free.moves)
        .zip(&engagements)
        .enumerate()
    {
        let (Some(fa), Some(fb)) = (a.move_type.feed_rate(), b.move_type.feed_rate()) else {
            continue;
        };
        if e.radial_woc_fraction > 1e-6 {
            engaged_moves += 1;
            assert_eq!(fa.to_bits(), fb.to_bits(), "engaged move {i}: {fa} vs {fb}");
        } else if (fa - fb).abs() > 1e-9 {
            no_engagement_differs += 1;
        }
    }
    // Non-vacuity: both kinds of move exist, and the no-engagement moves
    // did differ between the two runs.
    assert!(engaged_moves >= 8, "{engaged_moves} engaged moves");
    assert!(no_engagement_differs >= 2, "{no_engagement_differs} differ");
}
