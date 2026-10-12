//! Tests for the paths of `optimize_toolpath` that run no simulation:
//! the skip reasons, the cancel check and the Phase 1 outcome. The
//! end-to-end tests with a real simulation are in
//! `tests/optimize_smoke.rs`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use super::*;
use crate::compute::catalog::OperationConfig;
use crate::compute::config::DressupConfig;
use crate::compute::operation_configs::{AlignmentPinDrillConfig, DrillConfig, PocketConfig};
use crate::compute::tool_config::{ToolConfig, ToolId, ToolType};
use crate::session::ToolpathConfig;
use crate::stock::simulation_cut::SimulationCutTrace;

fn make_tool() -> ToolConfig {
    ToolConfig::new_default(ToolId(0), ToolType::EndMill)
}

fn empty_trace() -> SimulationCutTrace {
    SimulationCutTrace {
        sample_step_mm: 1.0,
        ..SimulationCutTrace::test_fixture()
    }
}

fn make_tc(operation: OperationConfig, tool_id: usize) -> ToolpathConfig {
    ToolpathConfig {
        id: ToolpathId(0),
        name: "test".to_owned(),
        enabled: true,
        operation,
        dressups: DressupConfig::default(),
        heights: crate::compute::config::HeightsConfig::default(),
        tool_id,
        model_id: 0,
        pre_gcode: None,
        post_gcode: None,
        boundary: crate::compute::config::BoundaryConfig::default(),
        boundary_inherit: true,
        stock_source: crate::compute::config::StockSource::Fresh,
        coolant: crate::gcode::CoolantMode::Off,
        face_selection: None,
        debug_options: crate::trace::debug_trace::ToolpathDebugOptions::default(),
        feeds_provenance: crate::feeds::FeedsProvenance::default(),
        rest_analysis: crate::compute::config::RestAnalysisConfig::default(),
        planner_origin: None,
    }
}

fn session_with_op(operation: OperationConfig) -> ProjectSession {
    let mut s = ProjectSession::new_empty();
    let _ = s.add_tool(make_tool());
    let _ = s
        .add_toolpath(0, make_tc(operation, s.tools()[0].id.0))
        .unwrap();
    s
}

/// F4.3 — every outcome (including refusals/skips) carries the
/// machine snapshot the run consumed, with the cutting ceiling
/// distinct from the travel rate.
#[test]
fn outcome_carries_machine_snapshot() {
    let mut session = session_with_op(OperationConfig::Drill(DrillConfig::default()));
    let trace = empty_trace();
    let cancel = AtomicBool::new(false);
    let outcome = optimize_toolpath(&mut session, &trace, 0, &cancel);
    let snap = outcome
        .machine_snapshot
        .as_ref()
        .expect("snapshot on every outcome, even Skipped");
    assert_eq!(snap.name, session.machine().name);
    assert!((snap.max_feed_mm_min - session.machine().max_feed_mm_min).abs() < 1e-9);
    assert!(
        (snap.cutting_feed_ceiling_mm_min - session.machine().cutting_feed_ceiling_mm_min()).abs()
            < 1e-9
    );
    assert!(snap.cutting_feed_ceiling_mm_min <= snap.max_feed_mm_min);
}

#[test]
fn drill_op_yields_skipped() {
    let mut session = session_with_op(OperationConfig::Drill(DrillConfig::default()));
    let trace = empty_trace();
    let cancel = AtomicBool::new(false);
    let outcome = optimize_toolpath(&mut session, &trace, 0, &cancel);
    assert_eq!(outcome.kind, OutcomeKind::Skipped, "got {outcome:?}");
    assert_eq!(
        outcome.reason,
        Some(RefuseReason::SteadyStateSamplesNotPresent)
    );
}

#[test]
fn alignment_pin_drill_yields_skipped() {
    let mut session = session_with_op(OperationConfig::AlignmentPinDrill(
        AlignmentPinDrillConfig::default(),
    ));
    let trace = empty_trace();
    let cancel = AtomicBool::new(false);
    let outcome = optimize_toolpath(&mut session, &trace, 0, &cancel);
    assert_eq!(outcome.kind, OutcomeKind::Skipped);
    assert_eq!(
        outcome.reason,
        Some(RefuseReason::SteadyStateSamplesNotPresent)
    );
}

#[test]
fn custom_material_yields_skipped() {
    use crate::compute::stock_config::StockConfig;
    let mut session = session_with_op(OperationConfig::Pocket(PocketConfig::default()));
    let mut stock = session.stock_config().clone();
    // The Custom-material check fires on the variant tag, not on
    // the feed-scale scalar — any Custom value triggers the
    // MaterialUnvalidated skip. A custom material also has no force
    // line (ruling B6 removed its typed `kc` field).
    stock.material = crate::material::Material::test_fixture_custom("test");
    let _ = session.set_stock_config(stock);
    let _ = StockConfig::default(); // silence unused-import false positive

    let trace = empty_trace();
    let cancel = AtomicBool::new(false);
    let outcome = optimize_toolpath(&mut session, &trace, 0, &cancel);
    assert_eq!(outcome.kind, OutcomeKind::Skipped);
    assert_eq!(outcome.reason, Some(RefuseReason::MaterialUnvalidated));
}

#[test]
fn invalid_toolpath_index_yields_skipped() {
    let mut session = session_with_op(OperationConfig::Pocket(PocketConfig::default()));
    let trace = empty_trace();
    let cancel = AtomicBool::new(false);
    let outcome = optimize_toolpath(&mut session, &trace, 99, &cancel);
    assert!(outcome.kind == OutcomeKind::Skipped);
}

#[test]
fn empty_trace_yields_skipped() {
    // Pocket op (supported), but trace has no samples for the
    // toolpath — Skipped per "no steady-state samples".
    let mut session = session_with_op(OperationConfig::Pocket(PocketConfig::default()));
    let trace = empty_trace();
    let cancel = AtomicBool::new(false);
    let outcome = optimize_toolpath(&mut session, &trace, 0, &cancel);
    assert_eq!(outcome.kind, OutcomeKind::Skipped);
    assert_eq!(
        outcome.reason,
        Some(RefuseReason::SteadyStateSamplesNotPresent)
    );
}

/// A trace with a cycle-time summary for toolpath 0, so the run passes
/// the "no samples" skip and reaches the baseline candidate.
fn trace_with_summary() -> SimulationCutTrace {
    use crate::stock::simulation_cut::SimulationToolpathCutSummary;
    let mut trace = empty_trace();
    trace.toolpath_summaries.push(SimulationToolpathCutSummary {
        toolpath_id: ToolpathId(0),
        sample_count: 100,
        total_runtime_s: 60.0,
        cutting_runtime_s: 50.0,
        rapid_runtime_s: 10.0,
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
    });
    trace
}

#[test]
fn cancel_before_run_yields_no_safe_improvement() {
    // The trace has a summary for this toolpath, so the skip paths do
    // not fire; the cancel flag is set before the guard is built.
    let trace = trace_with_summary();
    let mut session = session_with_op(OperationConfig::Pocket(PocketConfig::default()));
    let cancel = AtomicBool::new(true); // cancelled up-front
    let outcome = optimize_toolpath(&mut session, &trace, 0, &cancel);
    // Cancel-up-front should not produce a Ranked outcome.
    assert!(
        outcome.kind != OutcomeKind::Ranked,
        "cancelled run should not produce Ranked, got {outcome:?}"
    );
}

/// Phase 1 of `planning/optimize_review_2026-10-12/PLAN.md`: the search
/// has no levers. The run scores the baseline from the trace, evaluates
/// no candidate, and returns the exact "no search" narrative that the
/// GUI matches.
#[test]
fn a_run_with_no_levers_returns_the_baseline_and_the_no_search_text() {
    let trace = trace_with_summary();
    let mut session = session_with_op(OperationConfig::Pocket(PocketConfig::default()));
    let feed_before = session.toolpath_configs()[0].operation.feed_rate();
    let cancel = AtomicBool::new(false);
    let progress = std::sync::Arc::new(OptimizeProgress::default());
    let outcome = optimize_toolpath_observed(&mut session, &trace, 0, &cancel, &progress);

    assert_eq!(outcome.kind, OutcomeKind::NoSafeImprovement, "{outcome:?}");
    assert_eq!(outcome.reason, Some(RefuseReason::NoImprovementFound));
    assert_eq!(outcome.candidates.len(), 1, "only the baseline");
    let baseline = &outcome.candidates[0];
    assert_eq!(baseline.stage, SearchStage::Baseline);
    assert!(!baseline.delta.has_changes());
    assert!((baseline.cycle_time_s - 60.0).abs() < 1e-12);
    assert_eq!(outcome.narrative.headline, "No candidate was searched.");
    assert_eq!(
        outcome.narrative.explanation,
        "The search has no levers yet; the post-sim tuner is being rebuilt."
    );
    assert_eq!(outcome.recommended_index, None);

    // Every rung announced a total of zero and ticked none.
    assert_eq!(progress.high_water_phase(), Some(SearchPhase::Refine));
    for phase in SearchPhase::ALL {
        assert_eq!(progress.rung(phase), (0, 0), "{phase:?}");
    }
    assert!(progress.snapshot().phase.is_none());

    // The guard restored the parameters.
    assert!((session.toolpath_configs()[0].operation.feed_rate() - feed_before).abs() < 1e-12);
}
