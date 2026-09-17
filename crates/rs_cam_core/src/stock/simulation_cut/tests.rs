//! Unit tests for the simulation-cut trace and summary model.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use super::*;
use crate::toolpath::Toolpath;
use crate::trace::semantic_trace::{
    ToolpathSemanticKind, ToolpathSemanticRecorder, ToolpathSemanticTrace,
};
use std::time::{SystemTime, UNIX_EPOCH};

/// C2: an unmeasured axial-DOC fraction must not be averaged in as a
/// zero. The mean is taken over the samples that carried one — the same
/// contract `average_arc_radians` has had since Step 2 — and a class where
/// nothing was measured reports `None`, not `0.0`.
#[test]
fn unmeasured_axial_doc_fraction_is_none_not_a_zero_in_the_mean() {
    let sample = |axial: Option<f64>, dt: f64, idx: usize| SimulationCutSample {
        toolpath_id: ToolpathId(1),
        move_index: idx,
        sample_index: idx,
        position: [idx as f64, 0.0, -1.0],
        cumulative_time_s: dt * (idx as f64 + 1.0),
        segment_time_s: dt,
        is_cutting: true,
        cut_kinematics: CutKinematics::Linear,
        feed_rate_mm_min: 600.0,
        engagement: Engagement {
            radial_woc_fraction: 0.5,
            axial_doc_fraction: axial,
            ..Default::default()
        },
        ..SimulationCutSample::test_fixture()
    };

    // Nothing measured anywhere → None, and the radial axis is unaffected.
    let none_trace =
        SimulationCutTrace::from_samples(0.5, vec![sample(None, 0.4, 0), sample(None, 0.6, 1)]);
    let lin = none_trace
        .summary
        .per_kinematics
        .get(&CutKinematics::Linear)
        .expect("linear samples were observed");
    assert_eq!(lin.average_axial_doc_fraction, None);
    assert_eq!(lin.peak_axial_doc_fraction, None);
    assert!((lin.average_radial_woc_fraction - 0.5).abs() < 1e-9);

    // One measured 0.8 over 0.4 s, one unmeasured over 0.6 s: the mean is
    // 0.8 (over the observed runtime), NOT 0.32 (over all cutting time),
    // which is what the pre-C2 zero-sentinel produced.
    let mixed = SimulationCutTrace::from_samples(
        0.5,
        vec![sample(Some(0.8), 0.4, 0), sample(None, 0.6, 1)],
    );
    let lin = mixed
        .summary
        .per_kinematics
        .get(&CutKinematics::Linear)
        .expect("linear samples were observed");
    assert!(
        lin.average_axial_doc_fraction
            .is_some_and(|a| (a - 0.8).abs() < 1e-9),
        "mean must be over MEASURED runtime, got {:?}",
        lin.average_axial_doc_fraction
    );
    assert!(
        lin.peak_axial_doc_fraction
            .is_some_and(|p| (p - 0.8).abs() < 1e-9)
    );

    // A measured zero is still a measurement.
    let zero = SimulationCutTrace::from_samples(0.5, vec![sample(Some(0.0), 0.4, 0)]);
    let lin = zero
        .summary
        .per_kinematics
        .get(&CutKinematics::Linear)
        .expect("linear samples were observed");
    assert_eq!(lin.average_axial_doc_fraction, Some(0.0));
    assert_eq!(lin.peak_axial_doc_fraction, Some(0.0));
}

#[test]
fn trace_from_samples_accumulates_summary_and_issues() {
    let trace = SimulationCutTrace::from_samples(
        0.5,
        vec![
            SimulationCutSample {
                toolpath_id: ToolpathId(1),
                move_index: 4,
                position: [1.0, 2.0, -1.0],
                cumulative_time_s: 0.2,
                segment_time_s: 0.2,
                is_cutting: true,
                cut_kinematics: crate::stock::simulation_cut::CutKinematics::Linear,
                feed_rate_mm_min: 600.0,
                spindle_rpm: 18_000,
                flute_count: 2,
                axial_doc_mm: 1.5,
                axial_engagement_mm: 1.5,
                chipload_mm_per_tooth: 0.0166,
                effective_chip_thickness_mm: Some(0.0166),
                engagement: Engagement::with_radial_woc(0.01),
                removed_volume_est_mm3: 2.0,
                mrr_mm3_s: 10.0,
                semantic_item_id: Some(9),
                ..SimulationCutSample::test_fixture()
            },
            SimulationCutSample {
                toolpath_id: ToolpathId(1),
                move_index: 5,
                sample_index: 1,
                position: [2.0, 2.0, -1.0],
                cumulative_time_s: 0.5,
                segment_time_s: 0.3,
                is_cutting: true,
                cut_kinematics: crate::stock::simulation_cut::CutKinematics::Linear,
                feed_rate_mm_min: 600.0,
                spindle_rpm: 18_000,
                flute_count: 2,
                axial_doc_mm: 2.0,
                axial_engagement_mm: 2.0,
                chipload_mm_per_tooth: 0.0166,
                effective_chip_thickness_mm: Some(0.0166),
                engagement: Engagement::with_radial_woc(0.08),
                removed_volume_est_mm3: 3.0,
                mrr_mm3_s: 10.0,
                semantic_item_id: Some(9),
                ..SimulationCutSample::test_fixture()
            },
            SimulationCutSample {
                toolpath_id: ToolpathId(1),
                move_index: 6,
                sample_index: 2,
                position: [3.0, 2.0, 5.0],
                cumulative_time_s: 0.6,
                segment_time_s: 0.1,
                cut_kinematics: crate::stock::simulation_cut::CutKinematics::Linear,
                feed_rate_mm_min: 5000.0,
                spindle_rpm: 18_000,
                flute_count: 2,
                ..SimulationCutSample::test_fixture()
            },
        ],
    );

    assert_eq!(trace.summary.sample_count, 3);
    assert_eq!(trace.summary.issue_count, 2);
    assert_eq!(trace.toolpath_summaries.len(), 1);
    assert_eq!(trace.hotspots.len(), 2);
    assert!((trace.summary.total_runtime_s - 0.6).abs() < 1e-9);
    assert!((trace.summary.air_cut_time_s - 0.2).abs() < 1e-9);
    assert!((trace.summary.low_engagement_time_s - 0.3).abs() < 1e-9);
    assert!((trace.summary.total_removed_volume_est_mm3 - 5.0).abs() < 1e-9);
}

#[test]
fn trace_from_samples_with_semantics_emits_per_item_summaries() {
    let recorder = ToolpathSemanticRecorder::new("Metrics", "Metrics");
    let root = recorder.root_context();
    let op = root.start_item(ToolpathSemanticKind::Operation, "Metrics");
    let mut tp = Toolpath::new();
    tp.rapid_to(crate::geo::P3::new(0.0, 0.0, 5.0));
    tp.feed_to(crate::geo::P3::new(0.0, 0.0, -1.0), 300.0);
    tp.feed_to(crate::geo::P3::new(10.0, 0.0, -1.0), 300.0);
    tp.rapid_to(crate::geo::P3::new(10.0, 0.0, 5.0));
    let pass = op
        .context()
        .start_item(ToolpathSemanticKind::Pass, "Pass 1");
    pass.bind_to_toolpath(&tp, 0, tp.moves.len());
    let trace = recorder.finish();

    let cut_trace = SimulationCutTrace::from_samples_with_semantics(
        0.5,
        vec![
            SimulationCutSample {
                toolpath_id: ToolpathId(1),
                move_index: 1,
                position: [0.0, 0.0, -1.0],
                cumulative_time_s: 0.2,
                segment_time_s: 0.2,
                is_cutting: true,
                cut_kinematics: crate::stock::simulation_cut::CutKinematics::Linear,
                feed_rate_mm_min: 300.0,
                spindle_rpm: 18_000,
                flute_count: 2,
                axial_doc_mm: 1.0,
                axial_engagement_mm: 1.0,
                chipload_mm_per_tooth: 0.0083,
                effective_chip_thickness_mm: Some(0.0083),
                engagement: Engagement::with_radial_woc(0.08),
                removed_volume_est_mm3: 0.2,
                mrr_mm3_s: 1.0,
                semantic_item_id: Some(2),
                ..SimulationCutSample::test_fixture()
            },
            SimulationCutSample {
                toolpath_id: ToolpathId(1),
                move_index: 2,
                sample_index: 1,
                position: [5.0, 0.0, -1.0],
                cumulative_time_s: 0.4,
                segment_time_s: 0.2,
                is_cutting: true,
                cut_kinematics: crate::stock::simulation_cut::CutKinematics::Linear,
                feed_rate_mm_min: 300.0,
                spindle_rpm: 18_000,
                flute_count: 2,
                axial_doc_mm: 1.2,
                axial_engagement_mm: 1.2,
                chipload_mm_per_tooth: 0.0083,
                effective_chip_thickness_mm: Some(0.0083),
                engagement: Engagement::with_radial_woc(0.15),
                removed_volume_est_mm3: 0.4,
                mrr_mm3_s: 2.0,
                semantic_item_id: Some(2),
                ..SimulationCutSample::test_fixture()
            },
        ],
        [(ToolpathId(1), &trace)],
    );

    let summary = cut_trace
        .semantic_summaries
        .iter()
        .find(|summary| summary.semantic_item_id == 2)
        .expect("semantic summary");
    assert_eq!(summary.label, "Pass 1");
    assert_eq!(summary.kind, ToolpathSemanticKind::Pass);
    assert_eq!(summary.sample_count, 2);
    assert!(summary.peak_engagement >= summary.average_engagement);
    assert!(summary.peak_mrr_mm3_s >= summary.average_mrr_mm3_s);
}

#[test]
fn issues_and_hotspots_inherit_span_path_from_first_sample() {
    let span_path = vec![
        crate::trace::toolpath_spans::SpanId(0),
        crate::trace::toolpath_spans::SpanId(1),
    ];
    let trace = SimulationCutTrace::from_samples(
        0.5,
        vec![
            SimulationCutSample {
                toolpath_id: ToolpathId(1),
                move_index: 4,
                position: [1.0, 2.0, -1.0],
                cumulative_time_s: 0.2,
                segment_time_s: 0.2,
                is_cutting: true,
                cut_kinematics: CutKinematics::Linear,
                feed_rate_mm_min: 600.0,
                spindle_rpm: 18_000,
                flute_count: 2,
                axial_doc_mm: 1.5,
                axial_engagement_mm: 1.5,
                chipload_mm_per_tooth: 0.0166,
                effective_chip_thickness_mm: Some(0.0166),
                engagement: Engagement::with_radial_woc(0.005),
                removed_volume_est_mm3: 0.1,
                mrr_mm3_s: 0.5,
                semantic_item_id: Some(7),
                span_path: span_path.clone(),
                ..SimulationCutSample::test_fixture()
            },
            SimulationCutSample {
                toolpath_id: ToolpathId(1),
                move_index: 4,
                sample_index: 1,
                position: [1.5, 2.0, -1.0],
                cumulative_time_s: 0.4,
                segment_time_s: 0.2,
                is_cutting: true,
                cut_kinematics: CutKinematics::Linear,
                feed_rate_mm_min: 600.0,
                spindle_rpm: 18_000,
                flute_count: 2,
                axial_doc_mm: 1.5,
                axial_engagement_mm: 1.5,
                chipload_mm_per_tooth: 0.0166,
                effective_chip_thickness_mm: Some(0.0166),
                engagement: Engagement::with_radial_woc(0.005),
                removed_volume_est_mm3: 0.1,
                mrr_mm3_s: 0.5,
                semantic_item_id: Some(7),
                span_path: span_path.clone(),
                ..SimulationCutSample::test_fixture()
            },
        ],
    );
    assert_eq!(trace.issues.len(), 1, "one coalesced air-cut issue");
    assert_eq!(trace.issues[0].span_path, span_path);
    assert_eq!(trace.hotspots.len(), 1);
    assert_eq!(trace.hotspots[0].span_path, span_path);
}

#[test]
fn simulation_cut_artifact_writer_creates_json() {
    let artifact = SimulationCutArtifact::new(
        0.25,
        0.25,
        [0.0, 0.0, 0.0],
        [10.0, 10.0, 10.0],
        vec![ToolpathId(1), ToolpathId(2)],
        serde_json::json!({"resolution": 0.25}),
        SimulationCutTrace::from_samples(0.25, Vec::new()),
    );
    let dir = std::env::temp_dir().join(format!(
        "rs_cam_sim_cut_artifact_{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock before epoch")
            .as_nanos()
    ));
    let path = write_simulation_cut_artifact(&dir, "Adaptive 3D", &artifact)
        .expect("write sim cut artifact");
    let text = std::fs::read_to_string(&path).expect("read sim cut artifact");
    assert!(text.contains("\"included_toolpath_ids\""));
    // Two writes in the same millisecond must not share a path: ten viz
    // worker tests write-then-delete through one shared directory, and a
    // shared path hands one file two owners — the second owner's cleanup
    // deletes the first owner's artifact out from under its own
    // existence assert (observed 2026-08-27, schedule-dependent).
    let second = write_simulation_cut_artifact(&dir, "Adaptive 3D", &artifact)
        .expect("write second sim cut artifact");
    assert_ne!(path, second, "artifact paths must be unique per write");
    std::fs::remove_file(path).ok();
    std::fs::remove_file(second).ok();
    std::fs::remove_dir(dir).ok();
}

// --- Task E-simcut: Empty samples produce valid defaults ---

#[test]
fn trace_from_empty_samples() {
    let trace = SimulationCutTrace::from_samples(1.0, Vec::new());

    assert_eq!(trace.summary.sample_count, 0);
    assert_eq!(trace.summary.toolpath_count, 0);
    assert_eq!(trace.summary.issue_count, 0);
    assert_eq!(trace.summary.hotspot_count, 0);
    assert!((trace.summary.total_runtime_s).abs() < 1e-9);
    assert!((trace.summary.cutting_runtime_s).abs() < 1e-9);
    assert!((trace.summary.rapid_runtime_s).abs() < 1e-9);
    assert!((trace.summary.average_engagement).abs() < 1e-9);
    assert!((trace.summary.total_removed_volume_est_mm3).abs() < 1e-9);
    assert!(trace.toolpath_summaries.is_empty());
    assert!(trace.issues.is_empty());
    assert!(trace.hotspots.is_empty());
    assert!(trace.samples.is_empty());
}

// --- Rapid-only toolpath (no cutting) ---

#[test]
fn trace_rapid_only_has_zero_cutting_time() {
    let samples = vec![
        SimulationCutSample {
            position: [0.0, 0.0, 10.0],
            cumulative_time_s: 0.1,
            segment_time_s: 0.1,
            feed_rate_mm_min: 5000.0,
            spindle_rpm: 18_000,
            flute_count: 2,
            ..SimulationCutSample::test_fixture()
        },
        SimulationCutSample {
            move_index: 1,
            sample_index: 1,
            position: [50.0, 0.0, 10.0],
            cumulative_time_s: 0.3,
            segment_time_s: 0.2,
            feed_rate_mm_min: 5000.0,
            spindle_rpm: 18_000,
            flute_count: 2,
            ..SimulationCutSample::test_fixture()
        },
    ];

    let trace = SimulationCutTrace::from_samples(1.0, samples);

    assert_eq!(trace.summary.sample_count, 2);
    assert_eq!(trace.summary.toolpath_count, 1);
    assert!((trace.summary.total_runtime_s - 0.3).abs() < 1e-9);
    assert!((trace.summary.cutting_runtime_s).abs() < 1e-9);
    assert!((trace.summary.rapid_runtime_s - 0.3).abs() < 1e-9);
    assert_eq!(trace.summary.issue_count, 0);
    assert!((trace.summary.average_engagement).abs() < 1e-9);
    assert!((trace.summary.total_removed_volume_est_mm3).abs() < 1e-9);
}

// --- Engagement metrics classification ---

#[test]
fn trace_classifies_air_cut_and_low_engagement() {
    let samples = vec![
        // Air cut: is_cutting=true, engagement < 0.02
        SimulationCutSample {
            position: [0.0, 0.0, -1.0],
            cumulative_time_s: 0.1,
            segment_time_s: 0.1,
            is_cutting: true,
            cut_kinematics: crate::stock::simulation_cut::CutKinematics::Linear,
            feed_rate_mm_min: 600.0,
            spindle_rpm: 18_000,
            flute_count: 2,
            axial_doc_mm: 1.0,
            axial_engagement_mm: 1.0,
            chipload_mm_per_tooth: 0.01,
            effective_chip_thickness_mm: Some(0.01),
            engagement: Engagement::with_radial_woc(0.01),
            removed_volume_est_mm3: 0.1,
            mrr_mm3_s: 1.0,
            ..SimulationCutSample::test_fixture()
        },
        // Low engagement: is_cutting=true, 0.02 <= engagement < 0.10
        SimulationCutSample {
            move_index: 1,
            sample_index: 1,
            position: [1.0, 0.0, -1.0],
            cumulative_time_s: 0.2,
            segment_time_s: 0.1,
            is_cutting: true,
            cut_kinematics: crate::stock::simulation_cut::CutKinematics::Linear,
            feed_rate_mm_min: 600.0,
            spindle_rpm: 18_000,
            flute_count: 2,
            axial_doc_mm: 1.0,
            axial_engagement_mm: 1.0,
            chipload_mm_per_tooth: 0.01,
            effective_chip_thickness_mm: Some(0.01),
            engagement: Engagement::with_radial_woc(0.05),
            removed_volume_est_mm3: 0.3,
            mrr_mm3_s: 3.0,
            ..SimulationCutSample::test_fixture()
        },
        // Good engagement: is_cutting=true, engagement >= 0.10
        SimulationCutSample {
            move_index: 2,
            sample_index: 2,
            position: [2.0, 0.0, -1.0],
            cumulative_time_s: 0.3,
            segment_time_s: 0.1,
            is_cutting: true,
            cut_kinematics: crate::stock::simulation_cut::CutKinematics::Linear,
            feed_rate_mm_min: 600.0,
            spindle_rpm: 18_000,
            flute_count: 2,
            axial_doc_mm: 2.0,
            axial_engagement_mm: 2.0,
            chipload_mm_per_tooth: 0.02,
            effective_chip_thickness_mm: Some(0.02),
            engagement: Engagement::with_radial_woc(0.50),
            removed_volume_est_mm3: 1.0,
            mrr_mm3_s: 10.0,
            ..SimulationCutSample::test_fixture()
        },
    ];

    let trace = SimulationCutTrace::from_samples(0.5, samples);

    // Two issues: one air cut and one low engagement
    assert_eq!(trace.summary.issue_count, 2);
    assert_eq!(trace.issues.len(), 2);

    let air_cuts: Vec<_> = trace
        .issues
        .iter()
        .filter(|i| i.kind == SimulationCutIssueKind::AirCut)
        .collect();
    let low_engs: Vec<_> = trace
        .issues
        .iter()
        .filter(|i| i.kind == SimulationCutIssueKind::LowEngagement)
        .collect();
    assert_eq!(air_cuts.len(), 1, "Should have exactly 1 air cut issue");
    assert_eq!(
        low_engs.len(),
        1,
        "Should have exactly 1 low engagement issue"
    );

    // Air cut time = 0.1s, low engagement time = 0.1s
    assert!((trace.summary.air_cut_time_s - 0.1).abs() < 1e-9);
    assert!((trace.summary.low_engagement_time_s - 0.1).abs() < 1e-9);
}

#[test]
fn old_trace_json_deserializes_without_new_fields() {
    let json = r#"{
        "schema_version": 1,
        "sample_step_mm": 1.0,
        "summary": {
            "sample_count": 0,
            "toolpath_count": 0,
            "issue_count": 0,
            "hotspot_count": 0,
            "total_runtime_s": 0.0,
            "cutting_runtime_s": 0.0,
            "rapid_runtime_s": 0.0,
            "air_cut_time_s": 0.0,
            "low_engagement_time_s": 0.0,
            "average_engagement": 0.0,
            "peak_chipload_mm_per_tooth": 0.0,
            "peak_axial_doc_mm": 0.0,
            "total_removed_volume_est_mm3": 0.0,
            "average_mrr_mm3_s": 0.0
        },
        "toolpath_summaries": [],
        "semantic_summaries": [],
        "hotspots": [],
        "issues": [],
        "samples": [{
            "toolpath_id": 0,
            "move_index": 0,
            "sample_index": 0,
            "position": [0.0, 0.0, 0.0],
            "cumulative_time_s": 0.0,
            "segment_time_s": 0.0,
            "is_cutting": false,
            "feed_rate_mm_min": 0.0,
            "spindle_rpm": 0,
            "flute_count": 0,
            "axial_doc_mm": 0.0,
            "radial_engagement": 0.0,
            "chipload_mm_per_tooth": 0.0,
            "removed_volume_est_mm3": 0.0,
            "mrr_mm3_s": 0.0,
            "semantic_item_id": null
        }]
    }"#;
    let trace: SimulationCutTrace = serde_json::from_str(json).expect("old trace deserializes");
    assert!(trace.provenance.is_none());
    assert_eq!(trace.samples[0].arc_engagement_radians, None);
    assert_eq!(trace.samples[0].cut_kinematics, CutKinematics::Rapid);
}

#[test]
fn trace_coalesces_contiguous_air_cut_samples_into_one_issue() {
    // Ten consecutive air-cut samples, then one good sample, then
    // five more consecutive air-cut samples. Should produce exactly
    // TWO issue segments (not fifteen). See F-15 in the April review.
    let mut samples = Vec::new();
    for i in 0..10 {
        samples.push(SimulationCutSample {
            move_index: i,
            sample_index: i,
            position: [i as f64, 0.0, -1.0],
            cumulative_time_s: 0.1 * (i as f64 + 1.0),
            segment_time_s: 0.1,
            is_cutting: true,
            cut_kinematics: crate::stock::simulation_cut::CutKinematics::Linear,
            feed_rate_mm_min: 600.0,
            spindle_rpm: 18_000,
            flute_count: 2,
            axial_doc_mm: 1.0,
            axial_engagement_mm: 1.0,
            chipload_mm_per_tooth: 0.01,
            effective_chip_thickness_mm: Some(0.01),
            // all below 0.02
            engagement: Engagement::with_radial_woc(0.005 + i as f64 * 0.001),
            ..SimulationCutSample::test_fixture()
        });
    }
    // One good sample breaks the segment
    samples.push(SimulationCutSample {
        move_index: 10,
        sample_index: 10,
        position: [10.0, 0.0, -1.0],
        cumulative_time_s: 1.1,
        segment_time_s: 0.1,
        is_cutting: true,
        cut_kinematics: crate::stock::simulation_cut::CutKinematics::Linear,
        feed_rate_mm_min: 600.0,
        spindle_rpm: 18_000,
        flute_count: 2,
        axial_doc_mm: 2.0,
        axial_engagement_mm: 2.0,
        chipload_mm_per_tooth: 0.02,
        effective_chip_thickness_mm: Some(0.02),
        engagement: Engagement::with_radial_woc(0.50),
        removed_volume_est_mm3: 1.0,
        mrr_mm3_s: 10.0,
        ..SimulationCutSample::test_fixture()
    });
    for i in 0..5 {
        samples.push(SimulationCutSample {
            move_index: 11 + i,
            sample_index: 11 + i,
            position: [11.0 + i as f64, 0.0, -1.0],
            cumulative_time_s: 1.2 + 0.1 * i as f64,
            segment_time_s: 0.1,
            is_cutting: true,
            cut_kinematics: crate::stock::simulation_cut::CutKinematics::Linear,
            feed_rate_mm_min: 600.0,
            spindle_rpm: 18_000,
            flute_count: 2,
            axial_doc_mm: 1.0,
            axial_engagement_mm: 1.0,
            chipload_mm_per_tooth: 0.01,
            effective_chip_thickness_mm: Some(0.01),
            engagement: Engagement::with_radial_woc(0.01),
            ..SimulationCutSample::test_fixture()
        });
    }

    let trace = SimulationCutTrace::from_samples(0.5, samples);

    // Two segments, not 15 issues-per-sample.
    assert_eq!(
        trace.issues.len(),
        2,
        "expected 2 coalesced segments, got {}",
        trace.issues.len()
    );
    assert_eq!(trace.summary.issue_count, 2);

    let first = &trace.issues[0];
    assert_eq!(first.kind, SimulationCutIssueKind::AirCut);
    assert_eq!(first.sample_count, 10);
    assert_eq!(first.move_index, 0);
    assert_eq!(first.end_move_index, 9);
    // Duration from t=0.1 to t=1.0
    assert!((first.duration_s - 0.9).abs() < 1e-9);
    // Minimum engagement should be the first sample's 0.005
    assert!((first.min_radial_engagement - 0.005).abs() < 1e-9);

    let second = &trace.issues[1];
    assert_eq!(second.kind, SimulationCutIssueKind::AirCut);
    assert_eq!(second.sample_count, 5);
    assert_eq!(second.move_index, 11);
    assert_eq!(second.end_move_index, 15);

    // Sample-level counters in the summary are still per-sample
    // (not per-segment), so air_cut_time_s accumulates all 15
    // air-cut sample times: 10 × 0.1 + 5 × 0.1 = 1.5s.
    assert!((trace.summary.air_cut_time_s - 1.5).abs() < 1e-9);
}

#[test]
fn trace_flushes_open_segment_at_end_of_stream() {
    // If the sample stream ends mid-segment (last samples are still
    // air-cut), the open segment must be flushed, not lost.
    let samples = vec![
        SimulationCutSample {
            position: [0.0, 0.0, -1.0],
            cumulative_time_s: 0.1,
            segment_time_s: 0.1,
            is_cutting: true,
            cut_kinematics: crate::stock::simulation_cut::CutKinematics::Linear,
            feed_rate_mm_min: 600.0,
            spindle_rpm: 18_000,
            flute_count: 2,
            axial_doc_mm: 1.0,
            axial_engagement_mm: 1.0,
            chipload_mm_per_tooth: 0.01,
            effective_chip_thickness_mm: Some(0.01),
            engagement: Engagement::with_radial_woc(0.01),
            ..SimulationCutSample::test_fixture()
        },
        SimulationCutSample {
            move_index: 1,
            sample_index: 1,
            position: [1.0, 0.0, -1.0],
            cumulative_time_s: 0.2,
            segment_time_s: 0.1,
            is_cutting: true,
            cut_kinematics: crate::stock::simulation_cut::CutKinematics::Linear,
            feed_rate_mm_min: 600.0,
            spindle_rpm: 18_000,
            flute_count: 2,
            axial_doc_mm: 1.0,
            axial_engagement_mm: 1.0,
            chipload_mm_per_tooth: 0.01,
            effective_chip_thickness_mm: Some(0.01),
            engagement: Engagement::with_radial_woc(0.01),
            ..SimulationCutSample::test_fixture()
        },
    ];

    let trace = SimulationCutTrace::from_samples(0.5, samples);
    assert_eq!(trace.issues.len(), 1);
    let seg = &trace.issues[0];
    assert_eq!(seg.kind, SimulationCutIssueKind::AirCut);
    assert_eq!(seg.sample_count, 2);
    assert_eq!(seg.end_move_index, 1);
}

#[test]
fn trace_tracks_open_segments_per_toolpath() {
    // Interleaved samples from two toolpaths: each toolpath's
    // air-cut run should produce its own segment, not bleed into
    // the other's.
    let samples = vec![
        SimulationCutSample {
            position: [0.0, 0.0, -1.0],
            cumulative_time_s: 0.1,
            segment_time_s: 0.1,
            is_cutting: true,
            cut_kinematics: crate::stock::simulation_cut::CutKinematics::Linear,
            feed_rate_mm_min: 600.0,
            spindle_rpm: 18_000,
            flute_count: 2,
            axial_doc_mm: 1.0,
            axial_engagement_mm: 1.0,
            chipload_mm_per_tooth: 0.01,
            effective_chip_thickness_mm: Some(0.01),
            engagement: Engagement::with_radial_woc(0.01),
            ..SimulationCutSample::test_fixture()
        },
        SimulationCutSample {
            toolpath_id: ToolpathId(1),
            position: [10.0, 0.0, -1.0],
            cumulative_time_s: 0.15,
            segment_time_s: 0.1,
            is_cutting: true,
            cut_kinematics: crate::stock::simulation_cut::CutKinematics::Linear,
            feed_rate_mm_min: 600.0,
            spindle_rpm: 18_000,
            flute_count: 2,
            axial_doc_mm: 1.0,
            axial_engagement_mm: 1.0,
            chipload_mm_per_tooth: 0.01,
            effective_chip_thickness_mm: Some(0.01),
            engagement: Engagement::with_radial_woc(0.01),
            ..SimulationCutSample::test_fixture()
        },
        SimulationCutSample {
            move_index: 1,
            sample_index: 1,
            position: [1.0, 0.0, -1.0],
            cumulative_time_s: 0.2,
            segment_time_s: 0.1,
            is_cutting: true,
            cut_kinematics: crate::stock::simulation_cut::CutKinematics::Linear,
            feed_rate_mm_min: 600.0,
            spindle_rpm: 18_000,
            flute_count: 2,
            axial_doc_mm: 1.0,
            axial_engagement_mm: 1.0,
            chipload_mm_per_tooth: 0.01,
            effective_chip_thickness_mm: Some(0.01),
            engagement: Engagement::with_radial_woc(0.01),
            ..SimulationCutSample::test_fixture()
        },
    ];

    let trace = SimulationCutTrace::from_samples(0.5, samples);
    // Two segments — one per toolpath — not three per-sample issues
    // and not one bleeding across toolpath boundaries.
    assert_eq!(trace.issues.len(), 2);
    let tp0_segs: Vec<_> = trace
        .issues
        .iter()
        .filter(|i| i.toolpath_id == ToolpathId(0))
        .collect();
    let tp1_segs: Vec<_> = trace
        .issues
        .iter()
        .filter(|i| i.toolpath_id == ToolpathId(1))
        .collect();
    assert_eq!(tp0_segs.len(), 1);
    assert_eq!(tp1_segs.len(), 1);
    assert_eq!(tp0_segs[0].sample_count, 2);
    assert_eq!(tp1_segs[0].sample_count, 1);
}

// --- Peak metrics tracking ---

#[test]
fn trace_tracks_peak_chipload_and_axial_doc() {
    let samples = vec![
        SimulationCutSample {
            position: [0.0, 0.0, -1.0],
            cumulative_time_s: 0.1,
            segment_time_s: 0.1,
            is_cutting: true,
            cut_kinematics: crate::stock::simulation_cut::CutKinematics::Linear,
            feed_rate_mm_min: 600.0,
            spindle_rpm: 18_000,
            flute_count: 2,
            axial_doc_mm: 1.5,
            axial_engagement_mm: 1.5,
            chipload_mm_per_tooth: 0.05,
            effective_chip_thickness_mm: Some(0.05),
            engagement: Engagement::with_radial_woc(0.30),
            removed_volume_est_mm3: 2.0,
            mrr_mm3_s: 20.0,
            ..SimulationCutSample::test_fixture()
        },
        SimulationCutSample {
            move_index: 1,
            sample_index: 1,
            position: [1.0, 0.0, -2.0],
            cumulative_time_s: 0.2,
            segment_time_s: 0.1,
            is_cutting: true,
            cut_kinematics: crate::stock::simulation_cut::CutKinematics::Linear,
            feed_rate_mm_min: 600.0,
            spindle_rpm: 18_000,
            flute_count: 2,
            axial_doc_mm: 3.0,
            axial_engagement_mm: 3.0,
            chipload_mm_per_tooth: 0.08,
            effective_chip_thickness_mm: Some(0.08),
            engagement: Engagement::with_radial_woc(0.60),
            removed_volume_est_mm3: 5.0,
            mrr_mm3_s: 50.0,
            ..SimulationCutSample::test_fixture()
        },
    ];

    let trace = SimulationCutTrace::from_samples(0.5, samples);

    assert!(
        (trace.summary.peak_chipload_mm_per_tooth - 0.08).abs() < 1e-9,
        "Peak chipload should be 0.08, got {}",
        trace.summary.peak_chipload_mm_per_tooth
    );
    assert!(
        (trace.summary.peak_axial_doc_mm - 3.0).abs() < 1e-9,
        "Peak axial DOC should be 3.0, got {}",
        trace.summary.peak_axial_doc_mm
    );
    assert!(
        (trace.summary.total_removed_volume_est_mm3 - 7.0).abs() < 1e-9,
        "Total removed volume should be 7.0, got {}",
        trace.summary.total_removed_volume_est_mm3
    );
}

// --- Multiple toolpaths produce separate summaries ---

#[test]
fn trace_multiple_toolpaths_produce_separate_summaries() {
    let samples = vec![
        SimulationCutSample {
            position: [0.0, 0.0, -1.0],
            cumulative_time_s: 0.1,
            segment_time_s: 0.1,
            is_cutting: true,
            cut_kinematics: crate::stock::simulation_cut::CutKinematics::Linear,
            feed_rate_mm_min: 600.0,
            spindle_rpm: 18_000,
            flute_count: 2,
            axial_doc_mm: 1.0,
            axial_engagement_mm: 1.0,
            chipload_mm_per_tooth: 0.01,
            effective_chip_thickness_mm: Some(0.01),
            engagement: Engagement::with_radial_woc(0.40),
            removed_volume_est_mm3: 1.0,
            mrr_mm3_s: 10.0,
            ..SimulationCutSample::test_fixture()
        },
        SimulationCutSample {
            toolpath_id: ToolpathId(1),
            sample_index: 1,
            position: [10.0, 0.0, -2.0],
            cumulative_time_s: 0.3,
            segment_time_s: 0.2,
            is_cutting: true,
            cut_kinematics: crate::stock::simulation_cut::CutKinematics::Linear,
            feed_rate_mm_min: 300.0,
            spindle_rpm: 18_000,
            flute_count: 2,
            axial_doc_mm: 2.0,
            axial_engagement_mm: 2.0,
            chipload_mm_per_tooth: 0.02,
            effective_chip_thickness_mm: Some(0.02),
            engagement: Engagement::with_radial_woc(0.50),
            removed_volume_est_mm3: 3.0,
            mrr_mm3_s: 15.0,
            ..SimulationCutSample::test_fixture()
        },
    ];

    let trace = SimulationCutTrace::from_samples(1.0, samples);

    assert_eq!(trace.summary.sample_count, 2);
    assert_eq!(trace.summary.toolpath_count, 2);
    assert_eq!(trace.toolpath_summaries.len(), 2);

    let tp0 = trace
        .toolpath_summaries
        .iter()
        .find(|s| s.toolpath_id == ToolpathId(0))
        .expect("should have toolpath 0");
    let tp1 = trace
        .toolpath_summaries
        .iter()
        .find(|s| s.toolpath_id == ToolpathId(1))
        .expect("should have toolpath 1");

    assert_eq!(tp0.sample_count, 1);
    assert_eq!(tp1.sample_count, 1);
    assert!((tp0.total_runtime_s - 0.1).abs() < 1e-9);
    assert!((tp1.total_runtime_s - 0.2).abs() < 1e-9);
    assert!(
        (tp0.total_removed_volume_est_mm3 - 1.0).abs() < 1e-9,
        "TP0 removed volume should be 1.0, got {}",
        tp0.total_removed_volume_est_mm3
    );
    assert!(
        (tp1.total_removed_volume_est_mm3 - 3.0).abs() < 1e-9,
        "TP1 removed volume should be 3.0, got {}",
        tp1.total_removed_volume_est_mm3
    );
}

// --- Average engagement is time-weighted ---

#[test]
fn trace_average_engagement_is_time_weighted() {
    let samples = vec![
        // 0.1s at engagement=0.20
        SimulationCutSample {
            position: [0.0, 0.0, -1.0],
            cumulative_time_s: 0.1,
            segment_time_s: 0.1,
            is_cutting: true,
            cut_kinematics: crate::stock::simulation_cut::CutKinematics::Linear,
            feed_rate_mm_min: 600.0,
            spindle_rpm: 18_000,
            flute_count: 2,
            axial_doc_mm: 1.0,
            axial_engagement_mm: 1.0,
            chipload_mm_per_tooth: 0.01,
            effective_chip_thickness_mm: Some(0.01),
            engagement: Engagement::with_radial_woc(0.20),
            removed_volume_est_mm3: 0.5,
            mrr_mm3_s: 5.0,
            ..SimulationCutSample::test_fixture()
        },
        // 0.3s at engagement=0.80
        SimulationCutSample {
            move_index: 1,
            sample_index: 1,
            position: [1.0, 0.0, -1.0],
            cumulative_time_s: 0.4,
            segment_time_s: 0.3,
            is_cutting: true,
            cut_kinematics: crate::stock::simulation_cut::CutKinematics::Linear,
            feed_rate_mm_min: 600.0,
            spindle_rpm: 18_000,
            flute_count: 2,
            axial_doc_mm: 2.0,
            axial_engagement_mm: 2.0,
            chipload_mm_per_tooth: 0.02,
            effective_chip_thickness_mm: Some(0.02),
            engagement: Engagement::with_radial_woc(0.80),
            removed_volume_est_mm3: 2.0,
            mrr_mm3_s: 6.67,
            ..SimulationCutSample::test_fixture()
        },
    ];

    let trace = SimulationCutTrace::from_samples(0.5, samples);

    // Time-weighted average: (0.20*0.1 + 0.80*0.3) / (0.1+0.3)
    // = (0.02 + 0.24) / 0.4 = 0.26 / 0.4 = 0.65
    let expected_avg = (0.20 * 0.1 + 0.80 * 0.3) / (0.1 + 0.3);
    assert!(
        (trace.summary.average_engagement - expected_avg).abs() < 1e-6,
        "Average engagement should be {}, got {}",
        expected_avg,
        trace.summary.average_engagement
    );
}

// --- Hotspots are created per (toolpath_id, semantic_item_id) ---

#[test]
fn trace_hotspots_grouped_by_toolpath_and_semantic_id() {
    let samples = vec![
        SimulationCutSample {
            position: [0.0, 0.0, -1.0],
            cumulative_time_s: 0.1,
            segment_time_s: 0.1,
            is_cutting: true,
            cut_kinematics: crate::stock::simulation_cut::CutKinematics::Linear,
            feed_rate_mm_min: 600.0,
            spindle_rpm: 18_000,
            flute_count: 2,
            axial_doc_mm: 1.0,
            axial_engagement_mm: 1.0,
            chipload_mm_per_tooth: 0.01,
            effective_chip_thickness_mm: Some(0.01),
            engagement: Engagement::with_radial_woc(0.50),
            removed_volume_est_mm3: 1.0,
            mrr_mm3_s: 10.0,
            semantic_item_id: Some(1),
            ..SimulationCutSample::test_fixture()
        },
        SimulationCutSample {
            move_index: 1,
            sample_index: 1,
            position: [1.0, 0.0, -1.0],
            cumulative_time_s: 0.2,
            segment_time_s: 0.1,
            is_cutting: true,
            cut_kinematics: crate::stock::simulation_cut::CutKinematics::Linear,
            feed_rate_mm_min: 600.0,
            spindle_rpm: 18_000,
            flute_count: 2,
            axial_doc_mm: 1.0,
            axial_engagement_mm: 1.0,
            chipload_mm_per_tooth: 0.01,
            effective_chip_thickness_mm: Some(0.01),
            engagement: Engagement::with_radial_woc(0.50),
            removed_volume_est_mm3: 1.0,
            mrr_mm3_s: 10.0,
            semantic_item_id: Some(2),
            ..SimulationCutSample::test_fixture()
        },
        SimulationCutSample {
            move_index: 2,
            sample_index: 2,
            position: [2.0, 0.0, -1.0],
            cumulative_time_s: 0.3,
            segment_time_s: 0.1,
            is_cutting: true,
            cut_kinematics: crate::stock::simulation_cut::CutKinematics::Linear,
            feed_rate_mm_min: 600.0,
            spindle_rpm: 18_000,
            flute_count: 2,
            axial_doc_mm: 1.0,
            axial_engagement_mm: 1.0,
            chipload_mm_per_tooth: 0.01,
            effective_chip_thickness_mm: Some(0.01),
            engagement: Engagement::with_radial_woc(0.50),
            removed_volume_est_mm3: 1.0,
            mrr_mm3_s: 10.0,
            semantic_item_id: Some(1),
            ..SimulationCutSample::test_fixture()
        },
    ];

    let trace = SimulationCutTrace::from_samples(0.5, samples);

    // Hotspots are keyed by (toolpath_id, semantic_item_id).
    // We have 2 unique keys: (0, Some(1)) and (0, Some(2))
    assert_eq!(
        trace.hotspots.len(),
        2,
        "Should have 2 hotspots for 2 distinct semantic_item_ids, got {}",
        trace.hotspots.len()
    );

    // The hotspot for semantic_item_id=1 should have 2 samples
    let hotspot_1 = trace
        .hotspots
        .iter()
        .find(|h| h.semantic_item_id == Some(1))
        .expect("should have hotspot for semantic_item_id=1");
    assert_eq!(
        hotspot_1.sample_index_start, 0,
        "Hotspot 1 sample start should be 0"
    );
    assert_eq!(
        hotspot_1.sample_index_end, 2,
        "Hotspot 1 sample end should be 2"
    );
}

// --- Sanitize filename component ---

#[test]
fn sanitize_filename_handles_special_chars() {
    use crate::export::artifact_io::sanitize_filename_component;
    const FALLBACK: &str = "simulation_cut_trace";
    assert_eq!(
        sanitize_filename_component("Adaptive 3D", FALLBACK),
        "adaptive_3d"
    );
    assert_eq!(
        sanitize_filename_component("my/file:name.ext", FALLBACK),
        "my_file_name_ext"
    );
    assert_eq!(
        sanitize_filename_component("---test---", FALLBACK),
        "---test---"
    );
    assert_eq!(sanitize_filename_component("", FALLBACK), FALLBACK);
    assert_eq!(sanitize_filename_component("___", FALLBACK), FALLBACK);
}

// ── P3: transit-span gating of peak-axial-DOC ───────────────────────

/// Build a minimal cutting sample for the peak-DOC accumulator tests.
fn make_sample(
    sample_index: usize,
    axial_doc_mm: f64,
    in_transit_span: bool,
) -> SimulationCutSample {
    SimulationCutSample {
        move_index: sample_index,
        sample_index,
        cumulative_time_s: sample_index as f64 * 0.01,
        segment_time_s: 0.01,
        is_cutting: true,
        cut_kinematics: CutKinematics::Linear,
        feed_rate_mm_min: 1000.0,
        spindle_rpm: 18_000,
        flute_count: 2,
        axial_doc_mm,
        axial_engagement_mm: axial_doc_mm,
        chipload_mm_per_tooth: 0.03,
        effective_chip_thickness_mm: Some(0.03),
        engagement: Engagement::with_radial_woc(0.3),
        removed_volume_est_mm3: 1.0,
        mrr_mm3_s: 50.0,
        in_transit_span,
        ..SimulationCutSample::test_fixture()
    }
}

#[test]
fn peak_doc_includes_cutting_samples() {
    // Wanaka commanded DOC is 3 mm. A clean cutting sample at 3 mm
    // axial DOC should appear in the peak.
    let samples = vec![make_sample(0, 3.0, false)];
    let trace = SimulationCutTrace::from_samples(0.5, samples);
    assert!((trace.summary.peak_axial_doc_mm - 3.0).abs() < 1e-9);
}

#[test]
fn peak_doc_suppresses_transit_lift_bridge_artifact() {
    // Wanaka TP3 pattern: a transit sample reads 18.59 mm because the
    // cutter is bridging cleared air over uncleared neighbouring stock.
    // It must NOT contaminate peak axial DOC.
    let samples = vec![
        make_sample(0, 3.0, false),
        make_sample(1, 18.59, true), // lift-bridge artifact
        make_sample(2, 3.0, false),
    ];
    let trace = SimulationCutTrace::from_samples(0.5, samples);
    assert!(
        (trace.summary.peak_axial_doc_mm - 3.0).abs() < 1e-9,
        "transit-span sample at 18.59 mm should be excluded from peak DOC; got {}",
        trace.summary.peak_axial_doc_mm
    );
}

#[test]
fn peak_doc_excludes_purely_transit_streams() {
    // If every sample is in a transit span (extreme case), peak DOC is 0.
    let samples = vec![
        make_sample(0, 18.0, true),
        make_sample(1, 12.0, true),
        make_sample(2, 7.0, true),
    ];
    let trace = SimulationCutTrace::from_samples(0.5, samples);
    assert_eq!(trace.summary.peak_axial_doc_mm, 0.0);
}

#[test]
fn peak_chipload_also_skips_transit_samples() {
    // The same lift-bridge artifact inflates peak_chipload — the gate
    // applies to both extreme-value metrics.
    let mut transit = make_sample(0, 18.0, true);
    transit.chipload_mm_per_tooth = 5.0; // wildly high
    let mut cutting = make_sample(1, 3.0, false);
    cutting.chipload_mm_per_tooth = 0.05;
    let trace = SimulationCutTrace::from_samples(0.5, vec![transit, cutting]);
    assert!(
        trace.summary.peak_chipload_mm_per_tooth < 0.1,
        "transit-span sample must be excluded from peak chipload"
    );
}

// ── P4: suppress issues + mark `metrics_not_applicable` for drills ──

fn make_drill_sample(toolpath_id: usize, sample_index: usize) -> SimulationCutSample {
    // Drill kinematics: is_cutting=true, radial_engagement=0 (dexel can't
    // see Z-only moves).
    let mut s = make_sample(sample_index, 1.0, false);
    s.toolpath_id = ToolpathId(toolpath_id);
    s.engagement.radial_woc_fraction = 0.0;
    s
}

#[test]
fn drill_samples_dont_emit_air_cut_issues() {
    // 100 samples of a drill TP; without the gate they'd produce 1
    // coalesced air-cut issue. With the gate, zero.
    let samples: Vec<_> = (0..100).map(|i| make_drill_sample(7, i)).collect();
    let drill_ids: std::collections::BTreeSet<ToolpathId> =
        std::iter::once(ToolpathId(7)).collect();
    let trace = SimulationCutTrace::from_samples_with_context(
        0.5,
        samples,
        std::iter::empty::<(ToolpathId, &'static ToolpathSemanticTrace)>(),
        &drill_ids,
    );
    assert_eq!(
        trace.issues.len(),
        0,
        "drill TP should emit zero air-cut issues"
    );
}

#[test]
fn drill_toolpath_summary_is_marked_not_applicable() {
    let samples = vec![make_drill_sample(7, 0), make_drill_sample(7, 1)];
    let drill_ids: std::collections::BTreeSet<ToolpathId> =
        std::iter::once(ToolpathId(7)).collect();
    let trace = SimulationCutTrace::from_samples_with_context(
        0.5,
        samples,
        std::iter::empty::<(ToolpathId, &'static ToolpathSemanticTrace)>(),
        &drill_ids,
    );
    let tp = trace
        .toolpath_summaries
        .iter()
        .find(|s| s.toolpath_id == ToolpathId(7))
        .expect("toolpath 7 should have a summary");
    assert!(
        tp.metrics_not_applicable,
        "drill TP summary should be marked metrics_not_applicable"
    );
}

#[test]
fn non_drill_toolpath_still_emits_issues() {
    // 100 air-cut samples on a non-drill toolpath → still emit 1 coalesced issue.
    let mut samples = Vec::new();
    for i in 0..100 {
        let mut s = make_sample(i, 1.0, false);
        s.toolpath_id = ToolpathId(1);
        s.engagement.radial_woc_fraction = 0.0;
        samples.push(s);
    }
    let drill_ids = std::collections::BTreeSet::new();
    let trace = SimulationCutTrace::from_samples_with_context(
        0.5,
        samples,
        std::iter::empty::<(ToolpathId, &'static ToolpathSemanticTrace)>(),
        &drill_ids,
    );
    assert_eq!(
        trace.issues.len(),
        1,
        "non-drill TP with low engagement should emit 1 coalesced air-cut issue"
    );
    // Summary should NOT be marked metrics_not_applicable.
    let tp = trace
        .toolpath_summaries
        .iter()
        .find(|s| s.toolpath_id == ToolpathId(1))
        .unwrap();
    assert!(!tp.metrics_not_applicable);
}

#[test]
fn mixed_drill_and_non_drill_isolates_suppression() {
    // 1 drill (ids=7) + 1 non-drill (id=1) toolpath in one trace.
    // Only TP1's air-cut samples should produce issues.
    let mut samples: Vec<_> = (0..50).map(|i| make_drill_sample(7, i)).collect();
    for i in 50..100 {
        let mut s = make_sample(i, 1.0, false);
        s.toolpath_id = ToolpathId(1);
        s.engagement.radial_woc_fraction = 0.0;
        samples.push(s);
    }
    let drill_ids: std::collections::BTreeSet<ToolpathId> =
        std::iter::once(ToolpathId(7)).collect();
    let trace = SimulationCutTrace::from_samples_with_context(
        0.5,
        samples,
        std::iter::empty::<(ToolpathId, &'static ToolpathSemanticTrace)>(),
        &drill_ids,
    );
    assert_eq!(trace.issues.len(), 1, "only non-drill TP emits an issue");
    let drill_summary = trace
        .toolpath_summaries
        .iter()
        .find(|s| s.toolpath_id == ToolpathId(7))
        .unwrap();
    let mill_summary = trace
        .toolpath_summaries
        .iter()
        .find(|s| s.toolpath_id == ToolpathId(1))
        .unwrap();
    assert!(drill_summary.metrics_not_applicable);
    assert!(!mill_summary.metrics_not_applicable);
}

#[test]
fn per_toolpath_peak_doc_respects_transit_flag() {
    let samples = vec![
        make_sample(0, 3.0, false),
        make_sample(1, 5.5, true), // lift-bridge artifact (Wanaka TP6 pattern)
    ];
    let trace = SimulationCutTrace::from_samples(0.5, samples);
    let tp = trace
        .toolpath_summaries
        .iter()
        .find(|s| s.toolpath_id == ToolpathId(0))
        .expect("toolpath 0 should have a summary");
    assert!(
        (tp.peak_axial_doc_mm - 3.0).abs() < 1e-9,
        "per-TP peak DOC must respect transit flag; got {}",
        tp.peak_axial_doc_mm
    );
}
