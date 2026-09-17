//! Tests for `optimize_project` orchestration. We lean on Drill
//! and AlignmentPinDrill ops because `optimize_toolpath` returns
//! `Skipped` for them without running any sim — so the rollup
//! walks several toolpaths without needing actual generation /
//! simulation infrastructure. Bottleneck-detection and progress
//! sequencing are observable from the outcomes alone.
//!
//! Moved out of `tool_load/optimize/mod.rs` by P4; the module body is
//! unchanged.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use super::*;
use crate::compute::catalog::OperationConfig;
use crate::compute::config::{BoundaryConfig, DressupConfig, HeightsConfig, StockSource};
use crate::compute::operation_configs::{AlignmentPinDrillConfig, DrillConfig, PocketConfig};
use crate::compute::tool_config::{ToolConfig, ToolId, ToolType};
use crate::gcode::CoolantMode;
use crate::session::ToolpathConfig;
use crate::stock::simulation_cut::{SimulationCutTrace, SimulationToolpathCutSummary};
use crate::trace::debug_trace::ToolpathDebugOptions;
use std::sync::Mutex;

fn make_tool() -> ToolConfig {
    ToolConfig::new_default(ToolId(0), ToolType::EndMill)
}

fn empty_trace() -> SimulationCutTrace {
    SimulationCutTrace {
        sample_step_mm: 1.0,
        ..SimulationCutTrace::test_fixture()
    }
}

fn summary_for(toolpath_id: ToolpathId, runtime_s: f64) -> SimulationToolpathCutSummary {
    SimulationToolpathCutSummary {
        toolpath_id,
        sample_count: 100,
        total_runtime_s: runtime_s,
        cutting_runtime_s: runtime_s,
        rapid_runtime_s: 0.0,
        air_cut_time_s: 0.0,
        low_engagement_time_s: 0.0,
        average_engagement: 0.5,
        peak_chipload_mm_per_tooth: 0.04,
        peak_axial_doc_mm: 2.0,
        peak_plunge_descent_mm: 0.0,
        total_removed_volume_est_mm3: 100.0,
        average_mrr_mm3_s: 2.0,
        metrics_not_applicable: false,
        per_kinematics: std::collections::BTreeMap::new(),
        runtime_by_intent: None,
    }
}

fn make_tc(
    name: &str,
    operation: OperationConfig,
    tool_id: usize,
    enabled: bool,
) -> ToolpathConfig {
    ToolpathConfig {
        id: ToolpathId(0),
        name: name.to_owned(),
        enabled,
        operation,
        dressups: DressupConfig::default(),
        heights: HeightsConfig::default(),
        tool_id,
        model_id: 0,
        pre_gcode: None,
        post_gcode: None,
        boundary: BoundaryConfig::default(),
        boundary_inherit: true,
        stock_source: StockSource::Fresh,
        coolant: CoolantMode::Off,
        face_selection: None,
        debug_options: ToolpathDebugOptions::default(),
        feeds_provenance: crate::feeds::FeedsProvenance::default(),
        rest_analysis: crate::compute::config::RestAnalysisConfig::default(),
        planner_origin: None,
    }
}

/// Build a session with N drill toolpaths. Each drill triggers
/// `optimize_toolpath`'s `Skipped` early-exit (no sim required),
/// keeping these tests fast.
fn session_with_n_drills(names_and_enabled: &[(&str, bool)]) -> ProjectSession {
    let mut s = ProjectSession::new_empty();
    let _ = s.add_tool(make_tool());
    let tool_id = s.tools()[0].id.0;
    for (name, enabled) in names_and_enabled {
        let _ = s
            .add_toolpath(
                0,
                make_tc(
                    name,
                    OperationConfig::Drill(DrillConfig::default()),
                    tool_id,
                    *enabled,
                ),
            )
            .unwrap();
    }
    s
}

/// Progress reporter that records every (completed, total, label)
/// invocation. Used to verify call sequencing.
struct RecordingProgress {
    calls: Mutex<Vec<(usize, usize, String)>>,
}

impl RecordingProgress {
    fn new() -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
        }
    }

    fn into_calls(self) -> Vec<(usize, usize, String)> {
        self.calls.into_inner().unwrap()
    }
}

impl ProgressReporter for RecordingProgress {
    fn report(&self, completed: usize, total: usize, label: &str) {
        self.calls
            .lock()
            .unwrap()
            .push((completed, total, label.to_owned()));
    }
}

#[test]
fn empty_project_returns_empty_report() {
    let mut session = ProjectSession::new_empty();
    let trace = empty_trace();
    let cancel = AtomicBool::new(false);
    let report = optimize_project(&mut session, &trace, &NoProgress, &cancel);
    assert_eq!(report.baseline_cycle_time_s, 0.0);
    assert_eq!(report.bottleneck_index, None);
    assert!(report.per_toolpath.is_empty());
}

#[test]
fn disabled_toolpaths_excluded_from_walk() {
    // 3 toolpaths, only the middle one is enabled. Progress should
    // see total=1 and the per_toolpath should have one entry.
    let mut session = session_with_n_drills(&[("a", false), ("b", true), ("c", false)]);
    let trace = empty_trace();
    let cancel = AtomicBool::new(false);
    let progress = RecordingProgress::new();
    let report = optimize_project(&mut session, &trace, &progress, &cancel);
    assert_eq!(report.per_toolpath.len(), 1);
    assert_eq!(report.per_toolpath[0].0, 1, "the enabled TP is at index 1");

    let calls = progress.into_calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0, 0); // completed before this one
    assert_eq!(calls[0].1, 1); // total enabled
    assert!(
        calls[0].2.contains("b"),
        "label should name the toolpath: {}",
        calls[0].2
    );
}

#[test]
fn drill_toolpaths_all_yield_skipped() {
    let mut session = session_with_n_drills(&[("d1", true), ("d2", true), ("d3", true)]);
    let trace = empty_trace();
    let cancel = AtomicBool::new(false);
    let report = optimize_project(&mut session, &trace, &NoProgress, &cancel);
    assert_eq!(report.per_toolpath.len(), 3);
    for (_, outcome) in &report.per_toolpath {
        assert!(outcome.kind == OutcomeKind::Skipped);
    }
}

#[test]
fn bottleneck_picks_dominant_toolpath() {
    // 3 toolpaths with cycles [10s, 60s, 30s]. Total = 100s.
    // TP 1 at 60% trips the 30% threshold; TP 2 at 30% also trips
    // (≥). Bottleneck is the largest, so TP 1 wins.
    let mut session = session_with_n_drills(&[("a", true), ("b", true), ("c", true)]);
    // Pull the assigned ids out of the session (add_toolpath
    // increments next_toolpath_id, so they're stable: 0, 1, 2).
    let ids: Vec<ToolpathId> = session.toolpath_configs().iter().map(|tc| tc.id).collect();
    let mut trace = empty_trace();
    trace.toolpath_summaries.push(summary_for(ids[0], 10.0));
    trace.toolpath_summaries.push(summary_for(ids[1], 60.0));
    trace.toolpath_summaries.push(summary_for(ids[2], 30.0));

    let cancel = AtomicBool::new(false);
    let report = optimize_project(&mut session, &trace, &NoProgress, &cancel);
    assert_eq!(report.baseline_cycle_time_s, 100.0);
    // TP at session index 1 (id=1) has the largest cycle.
    assert_eq!(report.bottleneck_index, Some(1));
}

#[test]
fn no_bottleneck_when_runtime_evenly_split() {
    // 4 toolpaths at 25% each — none exceeds 30%, so bottleneck
    // is None. The rollup view will show "no single bottleneck".
    let mut session = session_with_n_drills(&[("a", true), ("b", true), ("c", true), ("d", true)]);
    let ids: Vec<ToolpathId> = session.toolpath_configs().iter().map(|tc| tc.id).collect();
    let mut trace = empty_trace();
    for &id in &ids {
        trace.toolpath_summaries.push(summary_for(id, 10.0));
    }
    let cancel = AtomicBool::new(false);
    let report = optimize_project(&mut session, &trace, &NoProgress, &cancel);
    assert_eq!(report.baseline_cycle_time_s, 40.0);
    assert_eq!(report.bottleneck_index, None);
}

#[test]
fn no_bottleneck_when_total_runtime_zero() {
    // Trace has no per-toolpath cycle data — the bottleneck calc
    // would divide by zero, so we short-circuit to None.
    let mut session = session_with_n_drills(&[("a", true), ("b", true)]);
    let trace = empty_trace();
    let cancel = AtomicBool::new(false);
    let report = optimize_project(&mut session, &trace, &NoProgress, &cancel);
    assert_eq!(report.baseline_cycle_time_s, 0.0);
    assert_eq!(report.bottleneck_index, None);
    assert_eq!(report.per_toolpath.len(), 2);
}

#[test]
fn cancel_before_any_walk_yields_partial_report() {
    // Cancel set before the loop runs. per_toolpath empty;
    // baseline metrics still populated from the trace.
    let mut session = session_with_n_drills(&[("a", true), ("b", true)]);
    let ids: Vec<ToolpathId> = session.toolpath_configs().iter().map(|tc| tc.id).collect();
    let mut trace = empty_trace();
    trace.toolpath_summaries.push(summary_for(ids[0], 50.0));
    trace.toolpath_summaries.push(summary_for(ids[1], 50.0));
    let cancel = AtomicBool::new(true);
    let report = optimize_project(&mut session, &trace, &NoProgress, &cancel);
    assert!(report.per_toolpath.is_empty());
    assert_eq!(report.baseline_cycle_time_s, 100.0);
    // Both TPs are at 50% > 30% threshold; bottleneck still
    // computes from the baseline trace, not the per_toolpath
    // result. Tie-break by largest cycle — both equal, so the
    // first one wins (max_by stable).
    assert!(report.bottleneck_index.is_some());
}

#[test]
fn progress_reports_in_order_for_each_enabled_toolpath() {
    let mut session = session_with_n_drills(&[("a", true), ("b", true), ("c", true)]);
    let trace = empty_trace();
    let cancel = AtomicBool::new(false);
    let progress = RecordingProgress::new();
    let _report = optimize_project(&mut session, &trace, &progress, &cancel);
    let calls = progress.into_calls();
    assert_eq!(calls.len(), 3);
    // Completed counts increase 0, 1, 2; total stays at 3.
    for (i, (completed, total, _)) in calls.iter().enumerate() {
        assert_eq!(*completed, i);
        assert_eq!(*total, 3);
    }
}

#[test]
fn bottleneck_threshold_constant_is_thirty_percent() {
    // Lock the constant in place so a future tweak to the
    // threshold gets a deliberate test update.
    assert!((search_policy().bottleneck_fraction.value - 0.30).abs() < 1e-9);
}

#[test]
fn no_progress_implements_trait() {
    // Smoke test that NoProgress satisfies ProgressReporter and
    // can be passed where a `&dyn ProgressReporter` is expected.
    let p: &dyn ProgressReporter = &NoProgress;
    p.report(0, 1, "ignored");
}

#[test]
fn pocket_op_with_empty_trace_yields_skipped_in_walk() {
    // Pocket trips a different skip path inside optimize_toolpath
    // (no steady-state samples for the toolpath_id in the trace).
    // Verify the walk surfaces it as a Skipped row in per_toolpath.
    let mut session = ProjectSession::new_empty();
    let _ = session.add_tool(make_tool());
    let tool_id = session.tools()[0].id.0;
    let _ = session
        .add_toolpath(
            0,
            make_tc(
                "p",
                OperationConfig::Pocket(PocketConfig::default()),
                tool_id,
                true,
            ),
        )
        .unwrap();
    // Add an alignment-pin drill to mix in the second skip path.
    let _ = session
        .add_toolpath(
            0,
            make_tc(
                "ap",
                OperationConfig::AlignmentPinDrill(AlignmentPinDrillConfig::default()),
                tool_id,
                true,
            ),
        )
        .unwrap();
    let trace = empty_trace();
    let cancel = AtomicBool::new(false);
    let report = optimize_project(&mut session, &trace, &NoProgress, &cancel);
    assert_eq!(report.per_toolpath.len(), 2);
    assert_eq!(report.per_toolpath[0].1.kind, OutcomeKind::Skipped);
    assert_eq!(report.per_toolpath[1].1.kind, OutcomeKind::Skipped);
}
