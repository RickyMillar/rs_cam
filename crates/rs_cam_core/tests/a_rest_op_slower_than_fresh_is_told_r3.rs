//! Operator ruling 2026-10-15, item 3 — the operator is told, on request,
//! when a rest operation takes longer than the same operation on fresh
//! stock.
//!
//! # The rule
//!
//! Rest can legitimately take longer than fresh: small islands need their
//! own entries and rapids. So rest ≤ fresh is not a bar. A normal
//! generation makes no fresh twin; it keeps only the rest op's planned
//! cycle time (`ToolpathStats::planned_cycle_time_s`). The request
//! `ProjectSession::compare_rest_with_fresh` (MCP `compare_rest_with_fresh`,
//! the GUI Stock tab) generates the twin (the same operation with
//! `StockSource::Fresh`) and `Command::AdoptRestFreshComparison` records
//! both times in one measure (the simulator's integrator at the commanded
//! feeds). While the comparison is current, the triage reports
//! `project.rest_slower_than_fresh` (Info) with both times when rest is
//! slower. An edit of the op makes it stale.
//!
//! # The fixtures
//!
//! `common::op_fixtures` (the G-OPGOLDEN rest arms): a default Adaptive3d
//! roughs the spherical cap, the simulation carves it, then the case op
//! generates with `FromRemainingStock` and the session simulates with the
//! default options (feed modulation on, as the GUI runs it).
//!
//! - Pencil: the rest pencil enters each island and runs longer than the
//!   fresh pencil. No finding before the request; one after it, naming
//!   both times; none after an edit of the op.
//! - Adaptive3d: the rest rough runs far shorter than the fresh rough. The
//!   triage stays silent after the request.
//!
//! The twin is checked against an independent session: the same case
//! with `StockSource::Fresh` generates a toolpath whose cycle time is the
//! twin's.

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

mod common;

use std::sync::atomic::AtomicBool;

use common::op_fixtures::{CaseSpec, Fixture, Frame, Prior, SIM_CELL_MM, case_session, run_prior};
use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::config::StockSource;
use rs_cam_core::compute::toolpath_stats::FreshTwinTime;
use rs_cam_core::diagnostics::{Severity, ids};
use rs_cam_core::machine::kinematics::compute_cycle_time;
use rs_cam_core::session::{
    Command, ProjectEvidence, ProjectSession, SetToolpathParamArgs, SimulationOptions,
};

/// The case op of `op` on `source`, after the prior op is generated and
/// simulated. Returns the session and the case op's index.
fn case(op: OperationType, source: StockSource) -> (ProjectSession, usize) {
    let (mut session, index) = case_session(CaseSpec {
        name: "case",
        op: OperationConfig::new_default(op),
        prior: Prior::RestPrior,
        stock_source: source,
        frame: Frame::TopUp,
        fixture: Fixture::Standard,
        tool: None,
    });
    run_prior(&mut session);
    (session, index)
}

/// The findings of the triage on the session's simulation.
fn findings(session: &ProjectSession) -> Vec<(Severity, String)> {
    let sim = session.simulation_result().expect("a simulation");
    let evidence = ProjectEvidence::from_simulation_with_holder_collisions(sim, Vec::new());
    session
        .simulation_triage(&evidence)
        .actions
        .iter()
        .filter(|f| f.diagnostic.id.as_str() == ids::PROJECT_REST_SLOWER_THAN_FRESH)
        .map(|f| (f.diagnostic.severity, f.diagnostic.message.clone()))
        .collect()
}

/// The rest arm of `op`, generated and simulated: the session, the case
/// op's index, and the cycle time of an independent fresh generation.
fn rest_arm(op: OperationType) -> (ProjectSession, usize, f64) {
    let cancel = AtomicBool::new(false);

    let (mut fresh, fresh_index) = case(op, StockSource::Fresh);
    let link = fresh.machine().link_kinematics();
    let fresh_tp = fresh
        .generate_toolpath(fresh_index, &cancel)
        .expect("the fresh op generates");
    let independent_fresh_s = compute_cycle_time(
        fresh_tp.toolpath(),
        &link.kinematics,
        link.max_feed_mm_min,
        link.rapid_feed_mm_min,
    );

    let (mut session, index) = case(op, StockSource::FromRemainingStock);
    let _ = session
        .generate_toolpath(index, &cancel)
        .expect("the rest op generates");
    let _ = session
        .run_simulation(
            &SimulationOptions {
                resolution: rs_cam_core::session::SimulationResolution::Fixed(SIM_CELL_MM),
                ..SimulationOptions::default()
            },
            &cancel,
        )
        .expect("the session simulates");
    (session, index, independent_fresh_s)
}

/// Run the request and record its answer, the two surfaces' path.
fn compare(session: &mut ProjectSession, index: usize) -> FreshTwinTime {
    let cancel = AtomicBool::new(false);
    let args = session
        .compare_rest_with_fresh(index, &cancel)
        .expect("the request answers");
    let times = args.times;
    let _ = session
        .apply(Command::AdoptRestFreshComparison(args))
        .expect("the comparison is recorded");
    times
}

#[test]
fn a_rest_op_slower_than_its_fresh_twin_is_reported_with_both_times_on_request() {
    let (mut session, index, independent_fresh_s) = rest_arm(OperationType::Pencil);
    assert!(
        findings(&session).is_empty(),
        "no comparison was asked for, so the triage says nothing"
    );
    let times = compare(&mut session, index);
    assert!(
        (times.fresh_s - independent_fresh_s).abs() < 1e-9,
        "the twin is the same op on fresh stock: {} s against {independent_fresh_s} s",
        times.fresh_s
    );
    assert!(
        times.rest_s > times.fresh_s,
        "fixture check: the rest pencil runs longer than the fresh one: {times:?}"
    );
    let found = findings(&session);
    assert_eq!(found.len(), 1, "one finding for the one rest op: {found:?}");
    let (severity, message) = &found[0];
    assert_eq!(*severity, Severity::Info, "information, not a defect");
    for t in [times.rest_s, times.fresh_s] {
        let text = format!("{t:.1} s");
        assert!(
            message.contains(&text),
            "the finding names both times ({text}): {message}"
        );
    }

    // An edit of the op makes the comparison stale, with the result.
    let feed = session.toolpath_configs()[index].operation.feed_rate();
    let _ = session
        .apply(Command::SetToolpathParam(SetToolpathParamArgs {
            index,
            param: "feed_rate".to_owned(),
            value: serde_json::json!(feed * 0.9),
        }))
        .expect("the feed edit");
    assert!(
        session.current_rest_fresh_comparisons().is_empty(),
        "an edit of the op makes its comparison stale"
    );
}

#[test]
fn a_rest_op_faster_than_its_fresh_twin_is_silent() {
    let (mut session, index, independent_fresh_s) = rest_arm(OperationType::Adaptive3d);
    let times = compare(&mut session, index);
    assert!(
        (times.fresh_s - independent_fresh_s).abs() < 1e-9,
        "the twin is the same op on fresh stock: {} s against {independent_fresh_s} s",
        times.fresh_s
    );
    assert!(
        times.rest_s < times.fresh_s,
        "fixture check: the rest rough runs shorter than the fresh one: {times:?}"
    );
    assert!(
        findings(&session).is_empty(),
        "no finding when rest is faster"
    );
}
