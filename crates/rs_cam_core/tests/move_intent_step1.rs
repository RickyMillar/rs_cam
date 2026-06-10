//! Step 1 — C + I regression-locking tests.
//!
//! Covers the load-bearing behavior introduced by adding `MoveIntent`
//! to the `Move` struct and routing the simulator's retract reclassification
//! + per-toolpath drill detection through it:
//!
//! - In-tree generators emit non-`Unknown` intents (drill produces
//!   `Drilling`, milling generators tag the cut body and bookend
//!   plunge/retract correctly).
//! - The simulator treats `Linear` moves with `MoveIntent::Retract` as
//!   non-cutting (no stamp, `is_cutting = false`), the way it already
//!   treats `Rapid` moves.
//! - The kinematic-heuristic classifier still applies for `Unknown`
//!   moves — the new path is additive, not a replacement.
//!
//! Plan: planning/DEXEL_Z_ONLY_INVESTIGATION.md §6.C / §6.I.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::ids::ToolpathId;
use rs_cam_core::{
    dexel_stock::{StockCutDirection, TriDexelStock},
    drill::{DrillCycle, DrillParams, drill_toolpath},
    geo::{BoundingBox3, P3},
    tool::FlatEndmill,
    toolpath::{MoveIntent, MoveType, Toolpath},
};

fn build_stock() -> TriDexelStock {
    let bbox = BoundingBox3 {
        min: P3::new(-5.0, -5.0, 0.0),
        max: P3::new(15.0, 15.0, 10.0),
    };
    TriDexelStock::from_bounds(&bbox, 0.25)
}

#[test]
fn drill_generator_tags_plunges_with_drilling_intent() {
    let params = DrillParams {
        depth: 5.0,
        top_z: 10.0,
        cycle: DrillCycle::Simple,
        feed_rate: 200.0,
        safe_z: 25.0,
        retract_z: 12.0,
    };
    let tp = drill_toolpath(&[[5.0, 5.0]], &params);

    // The single plunge Linear must be tagged `Drilling`; the bookend rapids
    // are `Linking`/`Retract`.
    let drill_count = tp
        .moves
        .iter()
        .filter(|m| matches!(m.intent, MoveIntent::Drilling))
        .count();
    assert!(
        drill_count >= 1,
        "expected at least one Drilling intent, got 0 in {:?}",
        tp.moves
            .iter()
            .map(|m| (m.move_type, m.intent))
            .collect::<Vec<_>>()
    );

    // Every Linear feed in a Simple cycle is a drill peck.
    for (i, m) in tp.moves.iter().enumerate() {
        if matches!(m.move_type, MoveType::Linear { .. }) {
            assert_eq!(
                m.intent,
                MoveIntent::Drilling,
                "drill simple-cycle Linear at index {i} should be Drilling"
            );
        }
    }
}

#[test]
fn drill_peck_cycle_tags_every_plunge_with_drilling_intent() {
    let params = DrillParams {
        depth: 6.0,
        top_z: 10.0,
        cycle: DrillCycle::Peck(2.0),
        feed_rate: 200.0,
        safe_z: 25.0,
        retract_z: 12.0,
    };
    let tp = drill_toolpath(&[[0.0, 0.0]], &params);

    let drilling_feeds = tp
        .moves
        .iter()
        .filter(|m| {
            matches!(m.move_type, MoveType::Linear { .. })
                && matches!(m.intent, MoveIntent::Drilling)
        })
        .count();
    let total_feeds = tp
        .moves
        .iter()
        .filter(|m| matches!(m.move_type, MoveType::Linear { .. }))
        .count();
    assert!(drilling_feeds >= 1);
    assert_eq!(
        drilling_feeds, total_feeds,
        "every Linear feed in a peck drill cycle should be MoveIntent::Drilling"
    );

    // Inter-peck retracts must be tagged `Retract` (not `Linking`).
    let retract_count = tp
        .moves
        .iter()
        .filter(|m| {
            matches!(m.move_type, MoveType::Rapid) && matches!(m.intent, MoveIntent::Retract)
        })
        .count();
    assert!(
        retract_count >= 1,
        "expected at least one Retract-tagged Rapid in peck cycle"
    );
}

#[test]
fn retract_feed_is_non_cutting_in_simulator() {
    // Build a toolpath where the body cuts material laterally and then is
    // followed by a pure-Z Linear move tagged `Retract`. The retract Linear
    // must produce `is_cutting = false` samples (no stamping).
    let mut tp = Toolpath::new();
    tp.rapid_to_with_intent(P3::new(0.0, 0.0, 12.0), MoveIntent::Linking);
    tp.feed_to_with_intent(P3::new(0.0, 0.0, 5.0), 200.0, MoveIntent::EntryPlunge);
    tp.feed_to_with_intent(P3::new(10.0, 0.0, 5.0), 1000.0, MoveIntent::FinishingCut);
    // Linear retract (NOT rapid) — historically would stamp as `is_cutting = true`.
    tp.feed_to_with_intent(P3::new(10.0, 0.0, 12.0), 300.0, MoveIntent::Retract);

    let mut stock = build_stock();
    let cutter = FlatEndmill::new(2.0, 25.0);

    let never_cancel = || false;
    let samples = stock
        .simulate_toolpath_with_metrics_with_cancel(
            &tp,
            &cutter,
            StockCutDirection::FromTop,
            ToolpathId(0),
            18_000,
            2,
            5000.0,
            0.5,
            None,
            &[],
            &[],
            true,
            &never_cancel,
        )
        .expect("simulation should complete");

    // Every sample at the retract move-index must be non-cutting.
    let retract_move_idx = tp.moves.len() - 1;
    let retract_samples: Vec<_> = samples
        .iter()
        .filter(|s| s.move_index == retract_move_idx)
        .collect();
    assert!(
        !retract_samples.is_empty(),
        "simulator should produce at least one sample for the retract move"
    );
    for s in &retract_samples {
        assert!(
            !s.is_cutting,
            "Linear move with MoveIntent::Retract should yield is_cutting=false, got {s:?}"
        );
        assert_eq!(
            s.engagement.radial_woc_fraction, 0.0,
            "retract sample should have zero radial engagement"
        );
    }
}

#[test]
fn unknown_intent_linear_still_classified_kinematically() {
    // A pure-Z `Linear` with intent `Unknown` (legacy generator path) must
    // still hit the cutting-segment classifier — the kinematic heuristic is
    // the documented fallback. This locks in that `Unknown` did not change
    // semantics for non-migrated generators.
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(0.0, 0.0, 12.0));
    tp.feed_to(P3::new(0.0, 0.0, 5.0), 200.0);

    let mut stock = build_stock();
    let cutter = FlatEndmill::new(2.0, 25.0);

    let never_cancel = || false;
    let samples = stock
        .simulate_toolpath_with_metrics_with_cancel(
            &tp,
            &cutter,
            StockCutDirection::FromTop,
            ToolpathId(0),
            18_000,
            2,
            5000.0,
            0.5,
            None,
            &[],
            &[],
            true,
            &never_cancel,
        )
        .expect("simulation should complete");

    // The plunge is a Linear move with intent Unknown — it should be sampled
    // as a cutting segment (kinematic fallback), producing is_cutting=true
    // samples.
    let plunge_samples: Vec<_> = samples.iter().filter(|s| s.is_cutting).collect();
    assert!(
        !plunge_samples.is_empty(),
        "Linear with Unknown intent should still be classified as cutting by kinematics"
    );
}
