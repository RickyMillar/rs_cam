//! R-11 — every simulation sample carries its move's **source** role.
//!
//! `SIMULATION_ISSUE_CHANNEL_CENSUS.md` §6.5 item 3 / §8.2 R-11: before this
//! field the only per-sample role handle was the collapsed boolean
//! `in_transit_span`. Any probe that wanted to group samples by what the
//! generator meant them to be had to re-join through `move_index` against the
//! annotated toolpath, and no MCP route reached it at all.
//!
//! These sentries pin the two halves of the contract:
//!
//! 1. the dexel emitter carries the generator's own `MoveIntent` onto every
//!    sample it emits, on all four emission paths (rapid, retract-feed,
//!    cutting linear, cutting arc);
//! 2. `None` means **not carried** and is distinct from
//!    `Some(MoveIntent::Unknown)`, which means "the generator did not tag it".
//!    Legacy traces deserialise to the former, never the latter.
//!
//! Ruling: Checkpoint D Q1 (2026-08-04), sequenced first because the arcfit
//! population bar (Q3) is expressed in terms of this field.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::dexel_stock::{StockCutDirection, TriDexelStock};
use rs_cam_core::geo::{BoundingBox3, P3};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::simulation_cut::SimulationCutSample;
use rs_cam_core::tool::FlatEndmill;
use rs_cam_core::toolpath::{MoveIntent, Toolpath};

fn build_stock() -> TriDexelStock {
    let bbox = BoundingBox3 {
        min: P3::new(-5.0, -5.0, 0.0),
        max: P3::new(15.0, 15.0, 10.0),
    };
    TriDexelStock::from_bounds(&bbox, 0.25)
}

#[test]
fn every_emitted_sample_carries_its_moves_source_intent() {
    // One move per emission path in the dexel emitter:
    //   rapid            → `sample_segment_runtime`
    //   entry plunge     → `capture_cutting_segment` (Plunge kinematics)
    //   clearing cut     → `capture_cutting_segment` (Linear kinematics)
    //   finishing cut    → `capture_cutting_segment` (Linear kinematics)
    //   retract feed     → `sample_segment_runtime` (reclassified non-cutting)
    let mut tp = Toolpath::new();
    tp.rapid_to_with_intent(P3::new(0.0, 0.0, 12.0), MoveIntent::Linking);
    tp.feed_to_with_intent(P3::new(0.0, 0.0, 5.0), 200.0, MoveIntent::EntryPlunge);
    tp.feed_to_with_intent(P3::new(6.0, 0.0, 5.0), 1000.0, MoveIntent::ClearingCut);
    tp.feed_to_with_intent(P3::new(10.0, 0.0, 5.0), 1000.0, MoveIntent::FinishingCut);
    tp.feed_to_with_intent(P3::new(10.0, 0.0, 12.0), 300.0, MoveIntent::Retract);
    // The opening rapid is move 0 and the emitter walks `1..moves.len()`, so
    // it emits nothing; this trailing rapid is what covers that path.
    tp.rapid_to_with_intent(P3::new(-2.0, -2.0, 12.0), MoveIntent::Linking);

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

    assert!(!samples.is_empty(), "fixture must emit samples");

    for sample in &samples {
        let expected = tp.moves[sample.move_index].intent;
        assert_eq!(
            sample.source_intent,
            Some(expected),
            "sample {} (move {}) should carry the generator's intent",
            sample.sample_index,
            sample.move_index
        );
    }

    // All five roles must actually be represented — otherwise the loop above
    // passes vacuously on whichever paths the fixture failed to exercise.
    for intent in [
        MoveIntent::Linking,
        MoveIntent::EntryPlunge,
        MoveIntent::ClearingCut,
        MoveIntent::FinishingCut,
        MoveIntent::Retract,
    ] {
        assert!(
            samples.iter().any(|s| s.source_intent == Some(intent)),
            "no sample carried {intent:?}; the emission path for it is uncovered"
        );
    }
}

#[test]
fn absent_source_intent_deserialises_as_not_carried_not_unknown() {
    // A trace captured before R-11 has no `source_intent` key at all. It must
    // read back as `None` ("not carried"), never as `Some(Unknown)` ("the
    // generator declined to tag it") — the two are different facts and the
    // repo's None-means-not-measured contract forbids coercing one to the
    // other.
    let fresh = SimulationCutSample::test_fixture();
    let mut json = serde_json::to_value(&fresh).expect("sample serialises");
    json.as_object_mut()
        .expect("sample is a JSON object")
        .remove("source_intent");

    let legacy: SimulationCutSample =
        serde_json::from_value(json).expect("legacy sample without the field must deserialise");
    assert_eq!(legacy.source_intent, None);

    // And a tagged sample round-trips its role.
    let tagged = SimulationCutSample {
        source_intent: Some(MoveIntent::FinishingCut),
        ..SimulationCutSample::test_fixture()
    };
    let round: SimulationCutSample =
        serde_json::from_str(&serde_json::to_string(&tagged).expect("serialises"))
            .expect("round-trips");
    assert_eq!(round.source_intent, Some(MoveIntent::FinishingCut));
    assert_ne!(round.source_intent, Some(MoveIntent::Unknown));
}
