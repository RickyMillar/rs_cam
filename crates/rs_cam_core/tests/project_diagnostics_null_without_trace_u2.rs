//! U2 (memory programme 2026-10-01): core `ProjectDiagnostics` publishes a
//! trace-less run as NOT MEASURED.
//!
//! `diagnostics_with_evidence` used to write `(0.0, 0.0, 0.0, 0.0)` for the
//! runtime, both air-cut readings and the engagement when the evidence had
//! no cut trace. The CLI `summary.json` and its printout read those values
//! as a measured, clean run ("Time: 0s"). The rule on every diagnostics
//! surface is that `null` means NOT MEASURED and 0.0 means measured and
//! clean. Each figure is now `None`, and `cut_metrics_not_measured` names
//! the reason in the words MCP `get_diagnostics` uses.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::ids::ToolpathId;
use rs_cam_core::session::{ProjectDiagnostics, ProjectEvidence, ProjectSessionBuilder};
use rs_cam_core::stock::simulation_cut::SimulationCutTrace;

fn trace_figures(diag: &ProjectDiagnostics) -> [(&'static str, Option<f64>); 4] {
    [
        ("total_runtime_s", diag.total_runtime_s),
        (
            "air_cut_pct_of_total_runtime",
            diag.air_cut_pct_of_total_runtime,
        ),
        (
            "air_cut_pct_of_cutting_time",
            diag.air_cut_pct_of_cutting_time,
        ),
        ("average_engagement", diag.average_engagement),
    ]
}

#[test]
fn no_simulation_publishes_null_and_names_the_reason() {
    let session = ProjectSessionBuilder::new().build();
    let diag = session.diagnostics();

    for (key, value) in trace_figures(&diag) {
        assert_eq!(value, None, "`{key}` must be NOT MEASURED with no run");
    }
    let reason = diag
        .cut_metrics_not_measured
        .expect("a NOT MEASURED figure names its reason");
    assert!(reason.contains("no simulation"), "{reason}");

    let json = serde_json::to_value(&diag).expect("the diagnostics serialise");
    for (key, _) in trace_figures(&diag) {
        assert!(
            json[key].is_null(),
            "`{key}` must be an explicit null on the wire, not {}",
            json[key]
        );
    }
}

/// The GUI case: a run with "Capture cutting metrics" off. The evidence
/// carries the run's toolpath boundaries and no cut trace.
#[test]
fn a_trace_less_simulation_publishes_null_and_names_the_capture_control_cause() {
    let session = ProjectSessionBuilder::new().build();
    let evidence = ProjectEvidence {
        boundaries: vec![(ToolpathId(1), 0, 12)],
        ..ProjectEvidence::default()
    };
    let diag = session.diagnostics_with_evidence(&evidence);

    for (key, value) in trace_figures(&diag) {
        assert_eq!(
            value, None,
            "`{key}` must be NOT MEASURED when the run kept no cut trace"
        );
    }
    let reason = diag
        .cut_metrics_not_measured
        .expect("a NOT MEASURED figure names its reason");
    assert!(
        reason.contains("without cutting metrics"),
        "a run that exists must not read as `no simulation`: {reason}"
    );
    assert!(
        !reason.contains("no simulation"),
        "a run that exists must not read as `no simulation`: {reason}"
    );
}

/// Non-vacuity: with a trace, the same call measures every figure and gives
/// no reason.
#[test]
fn a_traced_simulation_measures_every_figure() {
    let session = ProjectSessionBuilder::new().build();
    let mut trace = SimulationCutTrace::test_fixture();
    trace.summary.total_runtime_s = 100.0;
    let evidence = ProjectEvidence {
        boundaries: vec![(ToolpathId(1), 0, 12)],
        cut_trace: Some(&trace),
        ..ProjectEvidence::default()
    };
    let diag = session.diagnostics_with_evidence(&evidence);

    for (key, value) in trace_figures(&diag) {
        assert!(value.is_some(), "`{key}` must be measured with a trace");
    }
    assert_eq!(diag.total_runtime_s, Some(100.0));
    assert_eq!(diag.cut_metrics_not_measured, None);
}
