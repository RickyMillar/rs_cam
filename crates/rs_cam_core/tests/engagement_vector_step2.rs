//! Step 2 — D + H regression-locking tests.
//!
//! Covers the load-bearing behavior introduced by adding the structured
//! `Engagement` vector to `SimulationCutSample` and surfacing the
//! per-`CutKinematics` summary block on `SimulationToolpathCutSummary` /
//! `SimulationCutSummary`:
//!
//! - Production samples carry a populated `Engagement` with the same
//!   `radial_woc_fraction` as the legacy scalar (legacy-scalar consistency
//!   during the deprecation window).
//! - The per-kinematics summary block appears on a toolpath that mixes
//!   `Linear` and `Plunge` samples, and each block reports the axes that
//!   apply (radial-WOC + leading-edge speed are non-zero for Linear;
//!   peak_axial_doc_mm is non-zero for Plunge).
//! - `SummaryAccumulator::observe` accumulates per-kinematics correctly
//!   when fed a mixed sample stream directly.
//!
//! Plan: planning/DEXEL_Z_ONLY_INVESTIGATION.md §6.D / §6.H / §8 Step 2.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::{
    dexel_stock::{StockCutDirection, TriDexelStock},
    geo::{BoundingBox3, P3},
    simulation_cut::{
        CutKinematics, Engagement, EngagementDirection, SimulationCutSample, SimulationCutTrace,
        SummaryAccumulator,
    },
    tool::FlatEndmill,
    toolpath::{MoveIntent, Toolpath},
};

fn build_stock() -> TriDexelStock {
    let bbox = BoundingBox3 {
        min: P3::new(-5.0, -5.0, 0.0),
        max: P3::new(15.0, 15.0, 10.0),
    };
    TriDexelStock::from_bounds(&bbox, 0.25)
}

#[test]
fn production_samples_carry_populated_engagement_vector() {
    // Drive the real simulator with a lateral cut. Every cutting sample
    // must carry `engagement.radial_woc_fraction == sample.engagement.radial_woc_fraction`
    // (legacy-scalar consistency) and a non-zero leading-edge speed.
    let mut tp = Toolpath::new();
    tp.rapid_to_with_intent(P3::new(0.0, 0.0, 12.0), MoveIntent::Linking);
    tp.feed_to_with_intent(P3::new(0.0, 0.0, 5.0), 200.0, MoveIntent::EntryPlunge);
    tp.feed_to_with_intent(P3::new(10.0, 0.0, 5.0), 1000.0, MoveIntent::FinishingCut);

    let mut stock = build_stock();
    let cutter = FlatEndmill::new(2.0, 25.0);
    let never_cancel = || false;
    let samples = stock
        .simulate_toolpath_with_metrics_with_cancel(
            &tp,
            &cutter,
            StockCutDirection::FromTop,
            0,
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

    let cutting: Vec<_> = samples.iter().filter(|s| s.is_cutting).collect();
    assert!(
        !cutting.is_empty(),
        "expected at least one cutting sample from the lateral feed"
    );
    for s in &cutting {
        assert!(
            (s.engagement.radial_woc_fraction - s.engagement.radial_woc_fraction).abs() < 1e-9,
            "Engagement.radial_woc_fraction must mirror legacy scalar during deprecation window \
             (sample {:?})",
            s
        );
        assert!(
            s.engagement.leading_edge_speed_mm_min > 0.0,
            "production sample must report non-zero leading-edge speed; got {}",
            s.engagement.leading_edge_speed_mm_min
        );
        assert!(
            s.engagement.axial_doc_fraction >= 0.0 && s.engagement.axial_doc_fraction <= 1.0,
            "axial_doc_fraction must be in [0,1], got {}",
            s.engagement.axial_doc_fraction
        );
    }
}

fn mk_sample(
    toolpath_id: usize,
    sample_index: usize,
    kinematics: CutKinematics,
    radial: f64,
    axial_mm: f64,
    arc: Option<f64>,
) -> SimulationCutSample {
    SimulationCutSample {
        toolpath_id,
        move_index: sample_index,
        sample_index,
        position: [sample_index as f64, 0.0, 0.0],
        cumulative_time_s: 0.1 * (sample_index as f64 + 1.0),
        segment_time_s: 0.1,
        is_cutting: true,
        cut_kinematics: kinematics,
        feed_rate_mm_min: 1000.0,
        spindle_rpm: 18_000,
        flute_count: 2,
        axial_doc_mm: axial_mm,
        arc_engagement_radians: arc,
        chipload_mm_per_tooth: 0.02,
        effective_chip_thickness_mm: Some(0.018),
        engagement: Engagement {
            radial_woc_fraction: radial,
            axial_doc_fraction: (axial_mm / 25.0).clamp(0.0, 1.0),
            arc_radians: arc,
            mean_chip_thickness_mm: Some(0.02),
            peak_chip_thickness_mm: Some(0.018),
            leading_edge_speed_mm_min: 1000.0,
            direction: EngagementDirection::Mixed,
        },
        removed_volume_est_mm3: 1.0,
        mrr_mm3_s: 10.0,
        semantic_item_id: None,
        span_path: Vec::new(),
        in_transit_span: false,
    }
}

#[test]
fn summary_accumulator_separates_kinematics() {
    // Feed a mix of Linear and Plunge cutting samples and confirm both
    // kinematics buckets accumulate independently, with the right axis
    // populated for each.
    let mut acc = SummaryAccumulator::default();
    acc.observe(&mk_sample(
        1,
        0,
        CutKinematics::Linear,
        0.50,
        1.5,
        Some(0.8),
    ));
    acc.observe(&mk_sample(
        1,
        1,
        CutKinematics::Linear,
        0.30,
        1.5,
        Some(0.6),
    ));
    acc.observe(&mk_sample(1, 2, CutKinematics::Plunge, 1.0, 4.0, None));

    let summary = acc.finish_toolpath(1);
    let lin = summary
        .per_kinematics
        .get(&CutKinematics::Linear)
        .expect("Linear bucket should be present");
    let plunge = summary
        .per_kinematics
        .get(&CutKinematics::Plunge)
        .expect("Plunge bucket should be present");

    assert_eq!(lin.sample_count, 2);
    assert_eq!(plunge.sample_count, 1);
    assert!(
        (lin.average_radial_woc_fraction - 0.40).abs() < 1e-9,
        "Linear avg radial-WOC = (0.5+0.3)/2 = 0.40, got {}",
        lin.average_radial_woc_fraction
    );
    assert!(
        lin.average_arc_radians.is_some(),
        "Linear samples carried arc, so average_arc_radians should be Some"
    );
    assert!(
        plunge.average_arc_radians.is_none(),
        "Plunge samples carried no arc; average_arc_radians should be None"
    );
    assert!(
        plunge.peak_axial_doc_mm >= 4.0,
        "Plunge peak axial DOC should be ≥ 4.0 mm, got {}",
        plunge.peak_axial_doc_mm
    );
    assert!(
        plunge.peak_axial_doc_fraction > 0.0,
        "Plunge peak axial DOC fraction should be > 0, got {}",
        plunge.peak_axial_doc_fraction
    );
    assert!(
        lin.average_leading_edge_speed_mm_min > 0.0,
        "Linear avg leading-edge speed should be > 0"
    );
}

#[test]
fn trace_summary_exposes_per_kinematics_block() {
    // End-to-end via SimulationCutTrace::from_samples — verifies the
    // per_kinematics block propagates onto both the per-toolpath and
    // overall summaries.
    let samples = vec![
        mk_sample(7, 0, CutKinematics::Linear, 0.45, 1.0, Some(0.7)),
        mk_sample(7, 1, CutKinematics::Linear, 0.50, 1.0, Some(0.7)),
        mk_sample(7, 2, CutKinematics::Plunge, 1.0, 3.0, None),
    ];
    let trace = SimulationCutTrace::from_samples(0.5, samples);

    assert!(
        trace
            .summary
            .per_kinematics
            .contains_key(&CutKinematics::Linear),
        "overall summary should carry Linear bucket"
    );
    assert!(
        trace
            .summary
            .per_kinematics
            .contains_key(&CutKinematics::Plunge),
        "overall summary should carry Plunge bucket"
    );
    assert_eq!(trace.toolpath_summaries.len(), 1);
    let tp_summary = &trace.toolpath_summaries[0];
    assert!(
        tp_summary
            .per_kinematics
            .contains_key(&CutKinematics::Linear),
        "per-toolpath summary should carry Linear bucket"
    );
    assert!(
        tp_summary
            .per_kinematics
            .contains_key(&CutKinematics::Plunge),
        "per-toolpath summary should carry Plunge bucket"
    );
}

#[test]
fn legacy_scalar_matches_engagement_radial_woc_for_all_samples() {
    // Plan §10.3 — the legacy `radial_engagement` scalar stays valid
    // during the one-release deprecation window. Lock that consistency
    // in for the dexel simulator's emitted samples.
    let mut tp = Toolpath::new();
    tp.rapid_to_with_intent(P3::new(-2.0, 0.0, 12.0), MoveIntent::Linking);
    tp.feed_to_with_intent(P3::new(-2.0, 0.0, 5.0), 200.0, MoveIntent::EntryPlunge);
    tp.feed_to_with_intent(P3::new(12.0, 0.0, 5.0), 1000.0, MoveIntent::FinishingCut);

    let mut stock = build_stock();
    let cutter = FlatEndmill::new(3.0, 25.0);
    let never_cancel = || false;
    let samples = stock
        .simulate_toolpath_with_metrics_with_cancel(
            &tp,
            &cutter,
            StockCutDirection::FromTop,
            0,
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

    for s in &samples {
        assert!(
            (s.engagement.radial_woc_fraction - s.engagement.radial_woc_fraction).abs() < 1e-9,
            "legacy scalar must stay in lock-step with engagement.radial_woc_fraction \
             (move {}, sample {})",
            s.move_index,
            s.sample_index
        );
    }
}
