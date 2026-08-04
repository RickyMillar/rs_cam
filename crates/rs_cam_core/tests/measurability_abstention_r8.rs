//! R-8 + Checkpoint D Q2 — a gate reading an unmeasurable metric ABSTAINS.
//!
//! Red-first fixture, from `SIMULATION_ISSUE_CHANNEL_CENSUS.md` §5.1: a pass
//! that removes less than the dexel's fixed 0.05 mm fresh-material floor per
//! stamp removes real material, reports its removed *height* correctly, and
//! reports radial engagement of **exactly zero**. Because air cut is defined
//! as `radial_woc_fraction < 0.02`, every cutting sample of such a pass is
//! classified air cut — the census measured 95.9% of total runtime — and that
//! precise-looking percentage trips every shipped band while nothing anywhere
//! says the number is not a measurement.
//!
//! Before this wave the story ended there. These tests pin all three legs:
//!
//! 1. the fixture really does behave that way (`the_floor_fixture_...`) —
//!    if it ever stops, the rest of this file is testing nothing;
//! 2. the measurability detector calls it `NotMeasurable` and names the
//!    floor, while leaving collision detection and material removal live;
//! 3. the control arm — the same geometry cut deep — stays `Measurable`, so
//!    the detector is not simply abstaining on everything.
//!
//! Ruled 2026-08-04 (Checkpoint D Q2): the 0.05 mm floor is documented, not
//! tuned; a `NotMeasurable` metric stops feeding its gate; collision
//! detection always stays live.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::dexel_stock::{StockCutDirection, TriDexelStock};
use rs_cam_core::geo::{BoundingBox3, P3};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::sim_measurability::{
    Measurability, MeasurabilityReason, MeasurabilityReport, SimMetric,
};
use rs_cam_core::simulation_cut::{AirCutRatios, CutKinematics, SimulationCutTrace};
use rs_cam_core::tool::FlatEndmill;
use rs_cam_core::toolpath::{MoveIntent, Toolpath};

/// Stock top sits at Z = 10.
fn build_stock() -> TriDexelStock {
    let bbox = BoundingBox3 {
        min: P3::new(-5.0, -5.0, 0.0),
        max: P3::new(25.0, 15.0, 10.0),
    };
    TriDexelStock::from_bounds(&bbox, 0.25)
}

/// One lateral pass across virgin stock at `depth` below the stock top.
fn trace_for_depth(depth_mm: f64) -> SimulationCutTrace {
    let z = 10.0 - depth_mm;
    let mut tp = Toolpath::new();
    tp.rapid_to_with_intent(P3::new(0.0, 5.0, 12.0), MoveIntent::Linking);
    tp.feed_to_with_intent(P3::new(0.0, 5.0, z), 200.0, MoveIntent::EntryPlunge);
    tp.feed_to_with_intent(P3::new(20.0, 5.0, z), 1000.0, MoveIntent::FinishingCut);
    tp.feed_to_with_intent(P3::new(20.0, 5.0, 12.0), 300.0, MoveIntent::Retract);

    let mut stock = build_stock();
    let cutter = FlatEndmill::new(6.0, 25.0);
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
    SimulationCutTrace::from_samples(0.5, samples)
}

#[test]
fn the_floor_fixture_removes_material_and_still_reads_zero_engagement() {
    // Leg 1: the condition the ruling is about is real and reproduces here.
    let trace = trace_for_depth(0.02);
    let tp = &trace.toolpath_summaries[0];

    assert!(
        tp.total_removed_volume_est_mm3 > 0.0,
        "the shallow pass must actually remove material; got {}",
        tp.total_removed_volume_est_mm3
    );
    // Every LATERAL cutting sample reads exactly zero — the census's finding.
    // (The single entry-plunge sample is Z-only kinematics boring a
    // full-diameter hole and correctly reads 1.0; it is not part of the
    // claim, and its presence is what keeps this assertion honest.)
    let lateral: Vec<_> = trace
        .samples
        .iter()
        .filter(|s| s.is_cutting && s.cut_kinematics == CutKinematics::Linear)
        .collect();
    assert!(
        lateral.len() >= 30,
        "fixture must produce a real lateral pass; got {} samples",
        lateral.len()
    );
    assert!(
        lateral
            .iter()
            .all(|s| s.engagement.radial_woc_fraction == 0.0),
        "engagement must read EXACTLY zero under the floor; {} of {} lateral samples did not",
        lateral
            .iter()
            .filter(|s| s.engagement.radial_woc_fraction != 0.0)
            .count(),
        lateral.len()
    );
    assert!(
        tp.average_engagement < 0.01,
        "the toolpath average must collapse to ~0; got {}",
        tp.average_engagement
    );
    // And the resulting percentage clears the widest shipped band (40%, the
    // 2.5D clearing/contour band) by a mile — this is the misleading number.
    assert!(
        tp.air_cut_pct_of_total_runtime() > 40.0,
        "expected the misleading high air-cut reading; got {:.1}%",
        tp.air_cut_pct_of_total_runtime()
    );
}

#[test]
fn the_engagement_gates_abstain_below_the_fresh_material_floor() {
    // Leg 2: the detector calls it, names the floor, and says so in prose.
    let trace = trace_for_depth(0.02);
    let report = MeasurabilityReport::from_trace(&trace, Some(0.25));

    for metric in SimMetric::ENGAGEMENT_DERIVED {
        let m = report.for_metric(ToolpathId(0), metric);
        assert!(
            m.abstains(),
            "{metric:?} must abstain under the floor; got {m:?}"
        );
        match m.reason() {
            Some(MeasurabilityReason::BelowFreshMaterialFloor { floor_mm, .. }) => {
                assert!((floor_mm - 0.05).abs() < 1e-12, "floor should be 0.05 mm");
            }
            other => panic!("expected the fresh-material floor reason, got {other:?}"),
        }
    }

    let stated = report
        .for_metric(ToolpathId(0), SimMetric::AirCut)
        .reason()
        .expect("abstention carries a reason")
        .describe();
    assert!(
        stated.contains("0.05") && stated.contains("floor"),
        "the abstention must STATE its reason, not just assert it; got: {stated}"
    );
    assert!(
        stated.contains("does NOT fix"),
        "the reason must say a finer cell will not help — the floor is \
         independent of cell size; got: {stated}"
    );
}

#[test]
fn collision_and_removal_metrics_stay_live_while_engagement_abstains() {
    // Leg 2b, the explicit half of the ruling: collision detection always
    // stays live. It never touches the engagement path.
    let trace = trace_for_depth(0.02);
    let report = MeasurabilityReport::from_trace(&trace, Some(0.25));

    for metric in [
        SimMetric::RapidCollision,
        SimMetric::HolderCollision,
        SimMetric::MaterialRemoval,
        SimMetric::AxialEngagement,
    ] {
        assert_eq!(
            report.for_metric(ToolpathId(0), metric),
            Measurability::Measurable,
            "{metric:?} must remain live"
        );
    }
}

#[test]
fn the_deep_control_arm_stays_measurable() {
    // Leg 3: the detector is not just abstaining on everything. Same
    // geometry, 2 mm deep — the census's control arm.
    let trace = trace_for_depth(2.0);
    let tp = &trace.toolpath_summaries[0];
    assert!(
        tp.average_engagement > 0.0,
        "the deep arm must measure real engagement; got {}",
        tp.average_engagement
    );

    let report = MeasurabilityReport::from_trace(&trace, Some(0.25));
    assert!(
        report.all_measurable(),
        "nothing should abstain on a normal cut; got {:?}",
        report
            .entries
            .iter()
            .filter(|e| e.measurability != Measurability::Measurable)
            .collect::<Vec<_>>()
    );
}
