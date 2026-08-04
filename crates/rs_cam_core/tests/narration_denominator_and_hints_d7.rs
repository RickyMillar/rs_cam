//! Narration hygiene from `SIMULATION_ISSUE_CHANNEL_CENSUS.md` §3.5.
//!
//! - **D7** (ruled D-5): the air-cut ⚠ marker followed the CUTTING-time
//!   percentage while every shipped threshold in the workspace — the GUI's
//!   20%, the CLI's 40%, every band in `air_cut_high_threshold_pct` — reads
//!   the TOTAL-runtime one. The line prints both numbers, so a retract-heavy
//!   op could carry a warning marker that no gate agreed with. The marker
//!   moves; the reported numbers do not.
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
