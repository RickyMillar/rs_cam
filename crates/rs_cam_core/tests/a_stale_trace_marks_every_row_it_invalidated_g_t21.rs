//! **T-21 — a stale trace must mark every row it invalidated.**
//!
//! `gcode::project_load_report` throws away a trace whose provenance no
//! longer matches the project, runs the gates without it, and then
//! rewrites each `Unmodeled(SimulationRequired)` into
//! `Unmodeled(StaleSimulation)`. The two reasons are different operator
//! actions: "run the simulation" against "re-run it, the one you have
//! no longer describes this job".
//!
//! The rewrite named three verdicts — chipload, power, deflection. S3
//! added the depth-of-cut gate beside them and no rewrite arm, so ONE
//! toolpath reported three rows stale and one row never simulated, for
//! the one trace and the one cause. The operator reads a self-
//! contradicting report, and a surface that keys on the reason paints
//! the depth row as never-simulated evidence.
//!
//! What this file pins:
//!
//! - every milling row a stale trace invalidated carries
//!   `StaleSimulation`, and the depth row carries the same reason as the
//!   deflection row beside it;
//! - non-vacuity: the same fixture with NO trace carries
//!   `SimulationRequired` on those same rows, so the rewrite is what
//!   moves the reason and the arm above is not reading a constant.
//!
//! The drill gates take no rewrite and need none.
//! `drill_gates::DrillGateOutcome` has two arms, `Within` and
//! `Exceeds`, and no `Unmodeled` at all: those gates read the drill
//! op's own geometry and feeds, never a simulation trace, so no trace
//! can invalidate them.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::PocketConfig;
use rs_cam_core::gcode::{project_load_report, sim_trace_is_fresh};
use rs_cam_core::session::ProjectSession;
use rs_cam_core::stock::simulation_cut::SimulationCutTrace;
use rs_cam_core::tool_load::UnmodeledReason;
use rs_cam_core::tool_load::verdict::{LoadState, ToolLoadReport, ToolpathLoadVerdict};

mod common;
use common::session::{polygon_model, single_op_session, square_polygon, stock_under};
use common::tools::endmill_tool_config;

/// Half-extent (mm) of the square the pocket clears.
const HALF: f64 = 20.0;

/// Stock height (mm). `stock_under` hangs the board below `z = 0`.
const STOCK_Z: f64 = 6.0;

/// Cutter diameter (mm).
const TOOL_D: f64 = 6.0;

fn pocket_op() -> OperationConfig {
    OperationConfig::Pocket(PocketConfig {
        stepover: 2.0,
        depth: 3.0,
        depth_per_pass: 3.0,
        ..PocketConfig::default()
    })
}

/// One stock, one tool, one model, one enabled milling toolpath. The
/// gates need the CONFIG, not a generated result: a report is built per
/// enabled toolpath config.
fn milling_session() -> ProjectSession {
    single_op_session(
        stock_under(HALF, STOCK_Z),
        endmill_tool_config(TOOL_D),
        polygon_model(vec![square_polygon(HALF)], "square"),
        "Pocket",
        pocket_op(),
    )
}

/// **A trace this project cannot honour.** It carries no provenance
/// block, so it says nothing about what it was simulated against, and
/// `sim_trace_is_fresh` reads it as stale (L7, 2026-09-16). That is the
/// condition the rewrite exists for, reached without simulating.
fn stale_trace() -> SimulationCutTrace {
    let trace = SimulationCutTrace::test_fixture();
    assert!(
        trace.provenance.is_none(),
        "fixture guard: this trace must carry no provenance"
    );
    trace
}

fn only_verdict(report: &ToolLoadReport) -> &ToolpathLoadVerdict {
    assert_eq!(
        report.per_toolpath.len(),
        1,
        "the fixture holds exactly one enabled toolpath"
    );
    &report.per_toolpath[0]
}

/// The rows a trace can invalidate, as the reasons they carry. A
/// `None` means the row reached a verdict and states no reason.
fn reasons(v: &ToolpathLoadVerdict) -> Vec<(&'static str, Option<UnmodeledReason>)> {
    vec![
        ("chipload", v.chipload.unmodeled_reason().cloned()),
        ("power", v.power.unmodeled_reason().cloned()),
        ("deflection", v.deflection.unmodeled_reason().cloned()),
        ("depth", v.depth.unmodeled_reason().cloned()),
    ]
}

// ── the claim ────────────────────────────────────────────────────────

/// One trace, one cause, one reason on every row it invalidated.
#[test]
fn a_stale_trace_marks_the_depth_row_like_the_rows_beside_it() {
    let session = milling_session();
    let trace = stale_trace();
    // Fixture guard: the rewrite only runs on the stale path.
    assert!(
        !sim_trace_is_fresh(&session, &trace),
        "fixture guard: this trace must read as stale"
    );

    let report = project_load_report(&session, Some(&trace));
    let v = only_verdict(&report);
    for (name, reason) in reasons(v) {
        assert_eq!(
            reason,
            Some(UnmodeledReason::StaleSimulation),
            "the {name} row must read as stale evidence, not as missing evidence"
        );
    }
    assert_eq!(
        v.depth.unmodeled_reason(),
        v.deflection.unmodeled_reason(),
        "two rows, one trace, one cause: they cannot disagree about why"
    );
}

/// **Non-vacuity.** Take the trace away and the same rows read
/// `SimulationRequired`. The rewrite is what moves the reason, so the
/// arm above is not reading a constant the gates set themselves.
fn assert_all_rows_read_not_simulated(report: &ToolLoadReport) {
    let v = only_verdict(report);
    for (name, reason) in reasons(v) {
        assert_eq!(
            reason,
            Some(UnmodeledReason::SimulationRequired),
            "with no trace at all, the {name} row must ask for a simulation"
        );
    }
}

#[test]
fn with_no_trace_the_same_rows_ask_for_a_simulation() {
    let session = milling_session();
    let report = project_load_report(&session, None);
    assert_all_rows_read_not_simulated(&report);
    // And the rows are genuinely unmodelled, so the table above is a
    // table of reasons and not of absences.
    let v = only_verdict(&report);
    assert!(
        v.criteria().iter().all(|s| s.state == LoadState::Unmodeled),
        "fixture guard: no gate can measure without a trace"
    );
}
