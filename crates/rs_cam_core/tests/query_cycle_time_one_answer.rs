//! WP9 sentry — the cycle-time `Query` answers what the readiness
//! function answered.
//!
//! Programme: `planning/arch_consolidation_2026-09-09/IMPLEMENTATION_PLAN.md`
//! §4 WP9, §13 "WP9 rulings".
//!
//! `readiness::toolpath_cycle_time` (`crates/rs_cam_viz/src/ui/readiness.rs:485`,
//! pre-fix) decided a toolpath's cycle time from a cut trace, a cutting
//! distance, and a nominal feed. WP9 moves that decision into core and
//! wraps it behind `ProjectSession::query(Query::ToolpathCycleTime(..))`.
//!
//! This file copies the pre-fix decision verbatim as a frozen oracle,
//! `oracle_toolpath_cycle_time`, and asserts the `Query` answers the same
//! `CycleTime` on the same inputs, for every `CycleTimeBasis` this fixture
//! can construct: `MachineModel`, `SimulatedNoAccel`, `CuttingOnly`, and
//! the no-estimate case. The fixture follows
//! `mutation_paths_invalidate_alike_p0.rs` for the session (one tool, one
//! model, one computed toolpath) and
//! `crates/rs_cam_viz/tests/cycle_time_basis_g_timeest.rs` for the traces
//! (`SimulationCutTrace::test_fixture()` plus struct-update syntax).
//!
//! The `Query` carries its own trace rather than reading
//! `session.simulation_result()`: the GUI simulates off the frame loop and
//! keeps its cut trace in its own state (`state.simulation.results`,
//! `crates/rs_cam_viz/src/controller/events/simulation.rs:100-141`), so
//! `session.simulation_result()` reads `None` on a real project even after
//! a simulation. A query that read only the session's own slot would
//! answer `CuttingOnly` or nothing on every real project — a behaviour
//! change this file exists to prevent.
//!
//! Red pre-fix: this file does not compile. `Query`, `QueryAnswer`,
//! `ToolpathCycleTimeArgs`, `ToolpathCycleTimeAnswer` and
//! `ProjectSession::query` do not exist before WP9.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::BTreeMap;
use std::sync::Arc;

use rs_cam_core::ToolpathId;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::{
    BoundaryConfig, DressupConfig, HeightsConfig, RestAnalysisConfig, StockSource,
};
use rs_cam_core::compute::operation_configs::PocketConfig;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::gcode::CoolantMode;
use rs_cam_core::machine::kinematics::CycleTimeBreakdown;
use rs_cam_core::session::{
    AdoptResultArgs, Command, CommandId, CommandKind, CycleTime, CycleTimeBasis, LoadedModel,
    ProjectSession, ProjectSessionBuilder, Query, QueryAnswer, ToolpathConfig,
    ToolpathCycleTimeAnswer, ToolpathCycleTimeArgs,
};
use rs_cam_core::stock::simulation_cut::{
    SimulationCutTrace, SimulationToolpathCutSummary, ToolpathKinematicRuntime,
};
use rs_cam_core::trace::debug_trace::ToolpathDebugOptions;

/// The pre-fix decision, copied verbatim from
/// `crates/rs_cam_viz/src/ui/readiness.rs:485-527`, as a frozen oracle.
/// This is the answer WP9 must reproduce; it is not the code under test.
fn oracle_toolpath_cycle_time(
    trace: Option<&SimulationCutTrace>,
    id: ToolpathId,
    cutting_distance_mm: f64,
    nominal_feed_mm_min: f64,
) -> CycleTime {
    if let Some(rt) = trace.and_then(|t| t.toolpath_runtimes.iter().find(|r| r.toolpath_id == id)) {
        return CycleTime::of(rt.breakdown.total_s, CycleTimeBasis::MachineModel);
    }
    let summary = trace.and_then(|t| t.toolpath_summaries.iter().find(|s| s.toolpath_id == id));
    if let Some(summary) = summary {
        let basis = if summary.runtime_by_intent.is_some() {
            CycleTimeBasis::MachineModel
        } else {
            CycleTimeBasis::SimulatedNoAccel
        };
        return CycleTime::of(summary.total_runtime_s, basis);
    }
    if nominal_feed_mm_min > 0.0 {
        return CycleTime::of(
            (cutting_distance_mm / nominal_feed_mm_min) * 60.0,
            CycleTimeBasis::CuttingOnly,
        );
    }
    CycleTime::NONE
}

// ── traces ───────────────────────────────────────────────────────
// (the idiom from `cycle_time_basis_g_timeest.rs`)

/// A per-toolpath trace summary claiming `total_runtime_s` seconds.
/// `runtime_by_intent` is `Some` only when the F-034 integrator walked it.
fn summary(id: ToolpathId, total_runtime_s: f64, integrated: bool) -> SimulationToolpathCutSummary {
    SimulationToolpathCutSummary {
        toolpath_id: id,
        sample_count: 100,
        total_runtime_s,
        cutting_runtime_s: total_runtime_s,
        rapid_runtime_s: 0.0,
        air_cut_time_s: 0.0,
        low_engagement_time_s: 0.0,
        average_engagement: 0.3,
        peak_chipload_mm_per_tooth: 0.05,
        peak_axial_doc_mm: 2.0,
        peak_plunge_descent_mm: 0.0,
        total_removed_volume_est_mm3: 500.0,
        average_mrr_mm3_s: 20.0,
        metrics_not_applicable: false,
        per_kinematics: BTreeMap::new(),
        runtime_by_intent: integrated.then(|| CycleTimeBreakdown {
            total_s: total_runtime_s,
            cutting_s: total_runtime_s,
            ..CycleTimeBreakdown::default()
        }),
    }
}

fn trace_with_summary(s: SimulationToolpathCutSummary) -> SimulationCutTrace {
    SimulationCutTrace {
        toolpath_summaries: vec![s],
        ..SimulationCutTrace::test_fixture()
    }
}

/// A trace shaped like a project with drill toolpaths: the integrator
/// walked `toolpath_runtimes` but this toolpath has no engagement
/// summary.
fn trace_with_runtime(id: ToolpathId, total_s: f64) -> SimulationCutTrace {
    SimulationCutTrace {
        toolpath_runtimes: vec![ToolpathKinematicRuntime {
            toolpath_id: id,
            breakdown: CycleTimeBreakdown {
                total_s,
                cutting_s: total_s,
                ..CycleTimeBreakdown::default()
            },
        }],
        ..SimulationCutTrace::test_fixture()
    }
}

// ── fixture ──────────────────────────────────────────────────────
// (the idiom from `mutation_paths_invalidate_alike_p0.rs`)

fn tc(
    name: &str,
    op: OperationConfig,
    stock_source: StockSource,
    tool_id: usize,
    model_id: usize,
) -> ToolpathConfig {
    ToolpathConfig {
        id: ToolpathId(0),
        name: name.to_owned(),
        enabled: true,
        operation: op,
        dressups: DressupConfig::default(),
        heights: HeightsConfig::default(),
        tool_id,
        model_id,
        pre_gcode: None,
        post_gcode: None,
        boundary: BoundaryConfig::default(),
        boundary_inherit: true,
        stock_source,
        coolant: CoolantMode::Off,
        face_selection: None,
        debug_options: ToolpathDebugOptions::default(),
        feeds_provenance: rs_cam_core::feeds::FeedsProvenance::default(),
        rest_analysis: RestAnalysisConfig::default(),
        planner_origin: None,
    }
}

fn empty_model(name: &str) -> LoadedModel {
    LoadedModel {
        id: 0,
        name: name.to_owned(),
        mesh: None,
        polygons: None,
        drill_targets: std::sync::Arc::new(Vec::new()),
        layers: std::sync::Arc::new(Vec::new()),
        path: std::path::PathBuf::from(name),
        kind: None,
        units: None,
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
    }
}

fn fake_result() -> rs_cam_core::session::ToolpathComputeResult {
    rs_cam_core::session::ToolpathComputeResult {
        op_data: rs_cam_core::ops::drill_op::OpData::Toolpath(std::sync::Arc::new(
            rs_cam_core::trace::toolpath_spans::AnnotatedToolpath::new(
                rs_cam_core::toolpath::Toolpath::new(),
            ),
        )),
        stats: rs_cam_core::compute::toolpath_stats::ToolpathStats::default(),
        debug_trace: None,
        semantic_trace: None,
    }
}

fn adopt(s: &mut ProjectSession, index: usize) {
    let revision = s.toolpath_revision(index);
    let _ = s
        .apply(Command::AdoptResult(AdoptResultArgs {
            index,
            revision,
            result: Box::new(fake_result()),
        }))
        .expect("the fixture adopts at the current revision");
}

/// One setup, one tool, one model, one computed Pocket toolpath at index
/// 0.
fn fixture() -> ProjectSession {
    let mut builder = ProjectSessionBuilder::new();
    builder.add_tool(ToolConfig::new_default(ToolId(0), ToolType::EndMill));
    builder.add_model(empty_model("part.svg"));
    let tool = builder.tools()[0].id.0;
    let model = builder.models()[0].id;
    let toolpath = tc(
        "pocket",
        OperationConfig::Pocket(PocketConfig::default()),
        StockSource::Fresh,
        tool,
        model,
    );
    let _ = builder.add_toolpath(0, toolpath).unwrap();
    let mut s = builder.build();
    adopt(&mut s, 0);
    assert!(s.get_result(0).is_some(), "index 0 needs a cached result");
    s
}

/// Ask the core `Query` and the frozen oracle the same question, and
/// assert they agree.
fn assert_query_matches_oracle(
    s: &ProjectSession,
    index: usize,
    id: ToolpathId,
    trace: Option<Arc<SimulationCutTrace>>,
    cutting_distance_mm: Option<f64>,
    nominal_feed_mm_min: Option<f64>,
) {
    let expected = oracle_toolpath_cycle_time(
        trace.as_deref(),
        id,
        cutting_distance_mm.unwrap_or(0.0),
        nominal_feed_mm_min.unwrap_or(0.0),
    );
    let answer = s
        .query(Query::ToolpathCycleTime(ToolpathCycleTimeArgs {
            index,
            trace,
            cutting_distance_mm,
            nominal_feed_mm_min,
        }))
        .expect("index 0 names a toolpath");
    // WP13 added a second `Query` row, so this pattern is refutable now.
    let QueryAnswer::ToolpathCycleTime(ToolpathCycleTimeAnswer { cycle_time }) = answer else {
        panic!("the toolpath_cycle_time read answers its own variant");
    };
    assert_eq!(
        cycle_time, expected,
        "the Query and the frozen oracle must answer the same CycleTime"
    );
}

/// A trace the integrator walked (`toolpath_runtimes`): `MachineModel`.
#[test]
fn machine_model_basis_matches_the_oracle() {
    let s = fixture();
    let id = s.toolpath_configs()[0].id;
    let trace = Arc::new(trace_with_runtime(id, 46.8));
    assert_query_matches_oracle(&s, 0, id, Some(trace), Some(1000.0), Some(500.0));
}

/// A trace with an engagement summary but no integrator run:
/// `SimulatedNoAccel`.
#[test]
fn simulated_no_accel_basis_matches_the_oracle() {
    let s = fixture();
    let id = s.toolpath_configs()[0].id;
    let trace = Arc::new(trace_with_summary(summary(id, 1500.0, false)));
    assert_query_matches_oracle(&s, 0, id, Some(trace), Some(1000.0), Some(500.0));
}

/// No trace, a real distance and feed: `CuttingOnly`.
#[test]
fn cutting_only_basis_matches_the_oracle() {
    let s = fixture();
    let id = s.toolpath_configs()[0].id;
    assert_query_matches_oracle(&s, 0, id, None, Some(1000.0), Some(500.0));
}

/// No trace and no feed: the oracle and the Query both answer `NONE`.
#[test]
fn no_evidence_matches_the_oracle_none() {
    let s = fixture();
    let id = s.toolpath_configs()[0].id;
    assert_query_matches_oracle(&s, 0, id, None, None, None);
}

/// A refused `Query` names a toolpath that does not exist.
#[test]
fn a_query_for_a_missing_toolpath_refuses() {
    let s = fixture();
    let outcome = s.query(Query::ToolpathCycleTime(ToolpathCycleTimeArgs {
        index: 99,
        trace: None,
        cutting_distance_mm: Some(1000.0),
        nominal_feed_mm_min: Some(500.0),
    }));
    assert!(outcome.is_err(), "index 99 names no toolpath");
}

/// `CommandId::ALL` carries the new row, and it declares the `Query`
/// kind.
#[test]
fn the_registry_carries_the_cycle_time_row_as_a_query() {
    assert!(
        CommandId::ALL.contains(&CommandId::ToolpathCycleTime),
        "the registry must declare a ToolpathCycleTime row"
    );
    assert_eq!(
        CommandId::ToolpathCycleTime.kind(),
        CommandKind::Query,
        "a synchronous read is a Query, not a Command"
    );
}
