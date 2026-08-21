//! Narration hygiene from `SIMULATION_ISSUE_CHANNEL_CENSUS.md` §3.5.
//!
//! - **D7** (ruled D-5): the air-cut ⚠ marker followed the CUTTING-time
//!   percentage while every shipped threshold in the workspace — the GUI's
//!   banner, the CLI's 40%, every band in `air_cut_high_threshold_pct` — reads
//!   the TOTAL-runtime one. The line prints both numbers, so a retract-heavy
//!   op could carry a warning marker that no gate agreed with. The marker
//!   moves; the reported numbers do not.
//! - **W5B-F4 P8**: D7 moved the marker onto the right *denominator* but left
//!   it comparing against a flat 50 that no gate used, so the ⚠ and the gate
//!   could still disagree in both directions on the same toolpath. The marker
//!   now reads the narrated op's OWN band from
//!   `OperationType::air_cut_high_threshold_pct`, falling back to 50 only when
//!   there is no band to read.
//! - **D8 / R-5**: `UnifiedFinish` was missing from the finish-op hint arm.
//! - **R-13**: ops with no commanded axial step were told there was no
//!   denominator and then handed a ratio to form anyway.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::compute::tool_config::ToolMaterial;
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::narrate::{ToolpathNarrationContext, narrate_toolpath_with_context};
use rs_cam_core::sim_measurability::{MeasurabilityReport, SimMetric};
use rs_cam_core::simulation_cut::{Engagement, SimulationCutSample, SimulationCutTrace};
use rs_cam_core::tool::{FlatEndmill, ToolDefinition};
use rs_cam_core::toolpath::{MoveIntent, Toolpath};
use rs_cam_core::toolpath_spans::AnnotatedToolpath;

/// A trace whose air cut is a majority of CUTTING time but a minority of
/// TOTAL runtime — the exact shape the two denominators disagree on.
///
/// 30 cutting samples, 20 of them air (66.7% of cutting time), plus 70
/// rapid samples. Air is then 20/100 = 20% of total runtime: under the 50%
/// warning bar on the total-runtime measure, over it on the cutting one.
fn split_denominator_trace() -> SimulationCutTrace {
    let mut samples = Vec::new();
    let mut push = |idx: usize, cutting: bool, radial: f64| {
        samples.push(SimulationCutSample {
            toolpath_id: ToolpathId(0),
            move_index: 1,
            sample_index: idx,
            segment_time_s: 1.0,
            cumulative_time_s: idx as f64,
            is_cutting: cutting,
            engagement: Engagement::with_radial_woc(radial),
            removed_volume_est_mm3: if radial > 0.02 { 1.0 } else { 0.0 },
            ..SimulationCutSample::test_fixture()
        });
    };
    let mut i = 0;
    for _ in 0..20 {
        push(i, true, 0.0);
        i += 1;
    }
    for _ in 0..10 {
        push(i, true, 0.5);
        i += 1;
    }
    for _ in 0..70 {
        push(i, false, 0.0);
        i += 1;
    }
    SimulationCutTrace::from_samples(0.5, samples)
}

fn annotated() -> AnnotatedToolpath {
    let mut tp = Toolpath::new();
    tp.rapid_to_with_intent(
        rs_cam_core::geo::P3::new(0.0, 0.0, 5.0),
        MoveIntent::Linking,
    );
    tp.feed_to_with_intent(
        rs_cam_core::geo::P3::new(10.0, 0.0, 5.0),
        1000.0,
        MoveIntent::FinishingCut,
    );
    AnnotatedToolpath::new(tp)
}

fn tool() -> ToolDefinition {
    ToolDefinition::new(
        Box::new(FlatEndmill::new(6.0, 25.0)),
        6.0,
        30.0,
        25.0,
        30.0,
        2,
        ToolMaterial::Carbide,
    )
}

fn narrate(context: &ToolpathNarrationContext<'_>) -> String {
    let trace = split_denominator_trace();
    narrate_toolpath_with_context(&annotated(), None, Some(&trace), None, &tool(), context)
}

/// The air-cut line, isolated.
fn air_cut_line(text: &str) -> String {
    text.lines()
        .find(|l| l.contains("air cut") || l.contains("air-cut"))
        .unwrap_or_default()
        .to_owned()
}

#[test]
fn the_air_cut_marker_follows_the_total_runtime_denominator() {
    let context = ToolpathNarrationContext {
        toolpath_id: Some(ToolpathId(0)),
        operation_kind: Some(OperationType::Pocket),
        ..Default::default()
    };
    let line = air_cut_line(&narrate(&context));
    assert!(!line.is_empty(), "narration must emit an air-cut line");

    // 66.7% of cutting time, 20% of total runtime. Under the old rule the
    // cutting reading (> 50) set a ⚠ that no shipped gate would have raised.
    assert!(
        !line.contains('⚠'),
        "the marker must follow the total-runtime reading (20%), not the \
         cutting-time one (66.7%); got: {line}"
    );
    // Both numbers still get published — only the marker moved.
    assert!(
        line.contains("20") && line.contains("67"),
        "the line must still report BOTH denominators; got: {line}"
    );
}

#[test]
fn a_genuinely_high_total_runtime_reading_still_warns() {
    // Guard against fixing the false positive by disabling the marker.
    let mut samples = Vec::new();
    for i in 0..100 {
        samples.push(SimulationCutSample {
            toolpath_id: ToolpathId(0),
            move_index: 1,
            sample_index: i,
            segment_time_s: 1.0,
            cumulative_time_s: i as f64,
            is_cutting: true,
            engagement: Engagement::with_radial_woc(if i < 90 { 0.0 } else { 0.5 }),
            removed_volume_est_mm3: if i < 90 { 0.0 } else { 1.0 },
            ..SimulationCutSample::test_fixture()
        });
    }
    let trace = SimulationCutTrace::from_samples(0.5, samples);
    let context = ToolpathNarrationContext {
        toolpath_id: Some(ToolpathId(0)),
        operation_kind: Some(OperationType::Pocket),
        ..Default::default()
    };
    let text =
        narrate_toolpath_with_context(&annotated(), None, Some(&trace), None, &tool(), &context);
    assert!(
        air_cut_line(&text).contains('⚠'),
        "90% air of TOTAL runtime must still warn; got: {}",
        air_cut_line(&text)
    );
}

#[test]
fn narration_withholds_the_air_cut_percentage_when_it_is_not_a_measurement() {
    // Checkpoint D Q2, at the surface most likely to be quoted back as
    // evidence. RED-FIRST: before this, narration printed "air cut: 66.7% of
    // cutting time / 20.0% of total runtime" for a pass whose engagement
    // channel measured nothing — a precise-looking figure the gates had
    // already declined to act on.
    let mut samples = Vec::new();
    for i in 0..60 {
        samples.push(SimulationCutSample {
            toolpath_id: ToolpathId(0),
            move_index: 1,
            sample_index: i,
            segment_time_s: 1.0,
            cumulative_time_s: i as f64,
            is_cutting: true,
            // Removes material, reads exactly zero: the census's floor case.
            engagement: Engagement::with_radial_woc(0.0),
            removed_volume_est_mm3: 0.4,
            axial_engagement_mm: 0.02,
            ..SimulationCutSample::test_fixture()
        });
    }
    let trace = SimulationCutTrace::from_samples(0.5, samples);
    let report = MeasurabilityReport::from_trace(&trace, Some(0.25));
    assert!(
        report.abstains(ToolpathId(0), SimMetric::AirCut),
        "fixture must actually be unmeasurable"
    );

    let context = ToolpathNarrationContext {
        toolpath_id: Some(ToolpathId(0)),
        operation_kind: Some(OperationType::Scallop),
        measurability: Some(&report),
        ..Default::default()
    };
    let text =
        narrate_toolpath_with_context(&annotated(), None, Some(&trace), None, &tool(), &context);
    let line = air_cut_line(&text);
    assert!(
        line.contains("NOT MEASURED"),
        "narration must withhold the number, not print it; got: {line}"
    );
    assert!(
        line.contains("0.05") || line.contains("floor"),
        "and it must say WHY; got: {line}"
    );
    assert!(
        line.contains("Collision detection"),
        "and what remains valid; got: {line}"
    );
    // The AIR-CUT percentage specifically must be gone. (The reason text
    // legitimately quotes what fraction of samples read blind — that is a
    // statement about the instrument, not a reading of the metric.)
    assert!(
        !line.contains("of total runtime") && !line.contains("of cutting time"),
        "no air-cut percentage may be published for an unmeasurable metric; got: {line}"
    );
}

#[test]
fn narration_still_publishes_the_percentage_when_it_is_measured() {
    // The control: without an abstention the line is unchanged.
    let context = ToolpathNarrationContext {
        toolpath_id: Some(ToolpathId(0)),
        operation_kind: Some(OperationType::Pocket),
        measurability: None,
        ..Default::default()
    };
    let line = air_cut_line(&narrate(&context));
    assert!(line.contains('%'), "got: {line}");
    assert!(!line.contains("NOT MEASURED"), "got: {line}");
}

/// A trace whose air cut is exactly `air_samples` % of TOTAL runtime.
///
/// Every sample is cutting, so total runtime == cutting runtime and the two
/// denominators agree — which is what isolates the *marker's bar* as the
/// only variable.
fn total_runtime_air_pct_trace(air_samples: usize) -> SimulationCutTrace {
    let mut samples = Vec::new();
    for i in 0..100 {
        samples.push(SimulationCutSample {
            toolpath_id: ToolpathId(0),
            move_index: 1,
            sample_index: i,
            segment_time_s: 1.0,
            cumulative_time_s: i as f64,
            is_cutting: true,
            engagement: Engagement::with_radial_woc(if i < air_samples { 0.0 } else { 0.5 }),
            removed_volume_est_mm3: if i < air_samples { 0.0 } else { 1.0 },
            ..SimulationCutSample::test_fixture()
        });
    }
    SimulationCutTrace::from_samples(0.5, samples)
}

fn air_cut_line_for(op: Option<OperationType>, air_samples: usize) -> String {
    let trace = total_runtime_air_pct_trace(air_samples);
    let context = ToolpathNarrationContext {
        toolpath_id: Some(ToolpathId(0)),
        operation_kind: op,
        ..Default::default()
    };
    let text =
        narrate_toolpath_with_context(&annotated(), None, Some(&trace), None, &tool(), &context);
    air_cut_line(&text)
}

/// **W5B-F4 P8**: the ⚠ marker reads the narrated op's OWN air-cut band, not
/// a flat 50.
///
/// RED-FIRST shape: 44 % of total runtime is *over* the 2.5D clearing band
/// (40) and *under* the 3D finish band (45). One trace, two op kinds,
/// opposite markers — which the flat 50 could not produce, because it called
/// both of them ℹ and disagreed with the gate that fires on the pocket.
#[test]
fn the_air_cut_marker_reads_the_ops_own_band() {
    let pocket = air_cut_line_for(Some(OperationType::Pocket), 44);
    assert!(
        pocket.contains('⚠'),
        "44% air on a Pocket is over its 40 band and the gate fires; the \
         marker must agree. got: {pocket}"
    );

    let scallop = air_cut_line_for(Some(OperationType::Scallop), 44);
    assert!(
        !scallop.contains('⚠'),
        "44% air on a Scallop is under its 45 band and no gate fires; the \
         marker must not warn. got: {scallop}"
    );
}

/// The other direction, and the reason P8 had to land AFTER the band moved
/// 30 → 45: a defect-free 3D finish pass sits in the 34–43 cluster.
#[test]
fn a_defect_free_finish_reading_does_not_warn() {
    let line = air_cut_line_for(Some(OperationType::DropCutter), 35);
    assert!(
        !line.contains('⚠'),
        "35% is inside the measured defect-free finish cluster (34.3–42.5) \
         and under the 45 band; got: {line}"
    );
}

/// No op kind means no band to read — the fallback must still warn on a
/// genuinely high reading rather than silently disabling the marker.
#[test]
fn a_caller_with_no_operation_kind_falls_back_to_the_flat_bar() {
    let under = air_cut_line_for(None, 44);
    assert!(
        !under.contains('⚠'),
        "44% is under the 50 fallback; got: {under}"
    );
    let over = air_cut_line_for(None, 90);
    assert!(
        over.contains('⚠'),
        "90% must warn even with no band to read; got: {over}"
    );
}

#[test]
fn unified_finish_gets_the_finish_op_air_cut_hint() {
    let context = ToolpathNarrationContext {
        toolpath_id: Some(ToolpathId(0)),
        operation_kind: Some(OperationType::UnifiedFinish),
        ..Default::default()
    };
    let line = air_cut_line(&narrate(&context));
    assert!(
        line.contains("surface terrain"),
        "UnifiedFinish is in the 3D-finish air-cut band in catalog.rs and must \
         get the same hint as its siblings; got: {line}"
    );
}

#[test]
fn an_op_with_no_commanded_step_is_not_invited_to_form_a_ratio() {
    // R-13. `ProjectCurve`'s `depth` is a surface OFFSET, not an axial step —
    // dividing a removed height by it is what produced the "20.4x commanded"
    // reading in census §6.4.
    let context = ToolpathNarrationContext {
        toolpath_id: Some(ToolpathId(0)),
        operation_kind: Some(OperationType::ProjectCurve),
        depth_per_pass_mm: None,
        ..Default::default()
    };
    let text = narrate(&context);
    let doc_line = text
        .lines()
        .find(|l| l.contains("peak axial DOC"))
        .unwrap_or_default();
    assert!(!doc_line.is_empty(), "expected a peak-axial-DOC line");
    assert!(
        doc_line.contains("no commanded DOC"),
        "the line must say there is no denominator; got: {doc_line}"
    );
    assert!(
        !doc_line.contains("exact multiple of the step"),
        "having said there is no step, the line must not then invite a \
         multiple of it; got: {doc_line}"
    );
    assert!(
        doc_line.contains("no multiple to read it against"),
        "the replacement advice must name the absence explicitly; got: {doc_line}"
    );
}

#[test]
fn an_op_with_a_commanded_step_keeps_the_multiple_advice() {
    let context = ToolpathNarrationContext {
        toolpath_id: Some(ToolpathId(0)),
        operation_kind: Some(OperationType::Pocket),
        depth_per_pass_mm: Some(2.0),
        ..Default::default()
    };
    let text = narrate(&context);
    let doc_line = text
        .lines()
        .find(|l| l.contains("peak axial DOC"))
        .unwrap_or_default();
    assert!(
        doc_line.contains("exact multiple of the step"),
        "an op that DOES command a step keeps the multiple reading; got: {doc_line}"
    );
}
