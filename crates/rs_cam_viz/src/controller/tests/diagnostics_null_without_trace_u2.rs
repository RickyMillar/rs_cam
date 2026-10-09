//! U2 (memory programme 2026-10-01): MCP `get_diagnostics` publishes a
//! trace-less simulation as NOT MEASURED.
//!
//! A GUI run with the old capture checkbox off kept no cut trace, and
//! `get_diagnostics` published
//! `"air_cut_pct_of_cutting_time": 0.0`, `"total_runtime_s": 0.0` and
//! `"samples_total": 0`. Those are the values of a measured, clean run.
//! The `get_diagnostics` contract is that `null` means NOT MEASURED and
//! 0.0 means measured and clean, so each figure must be `null`, and
//! `cut_metrics_not_measured` must name the reason. Every run captures the
//! trace since the operator ruling of 2026-10-02 ("always capture"), but a
//! result without one must still read NOT MEASURED.
//!
//! `build_mcp_diagnostics` exists only under the `mcp` feature, so the
//! whole module is gated on it.
#![cfg(feature = "mcp")]

use super::*;

/// A simulation result with no cut trace in the view state.
fn trace_less_results() -> crate::state::simulation::SimulationResults {
    crate::state::simulation::SimulationResults {
        mesh: std::sync::Arc::new(rs_cam_core::stock::stock_mesh::StockMesh {
            vertices: Vec::new(),
            indices: Vec::new(),
            colors: Vec::new(),
            material_slots: Vec::new(),
        }),
        total_moves: 12,
        boundaries: Vec::new(),
        setup_boundaries: Vec::new(),
        checkpoints: Vec::new(),
        selected_toolpaths: None,
        playback_data: Vec::new(),
        stock_bbox: rs_cam_core::geo::BoundingBox3 {
            min: rs_cam_core::geo::P3::new(0.0, 0.0, 0.0),
            max: rs_cam_core::geo::P3::new(10.0, 10.0, 10.0),
        },
        cut_trace: None,
        cut_trace_path: None,
        column_grid_cell_mm: 0.5,
        prior_stocks: std::collections::HashMap::new(),
    }
}

const TRACE_FIGURES: [&str; 4] = [
    "total_runtime_s",
    "air_cut_pct_of_total_runtime",
    "air_cut_pct_of_cutting_time",
    "average_engagement",
];

#[test]
fn a_trace_less_simulation_publishes_null_not_zero() {
    let mut controller = sample_controller();
    controller.state.simulation.results = Some(trace_less_results());

    let diag = controller.build_mcp_diagnostics();

    for key in TRACE_FIGURES {
        let value = diag.get(key).expect("the key stays on the wire");
        assert!(
            value.is_null(),
            "`{key}` must be null (NOT MEASURED) without a cut trace, not {value}"
        );
    }
    let reason = diag["cut_metrics_not_measured"]
        .as_str()
        .expect("a trace-less run names why the figures are null");
    assert!(reason.contains("no cut trace"), "{reason}");
    assert!(reason.contains("re-run the simulation"), "{reason}");

    let counts = diag["triage"]["counts"]
        .as_object()
        .expect("the triage keeps its counts block");
    assert!(
        counts.contains_key("samples_total"),
        "non-vacuity: the counts block names samples_total: {counts:?}"
    );
    for (key, value) in counts {
        assert!(
            value.is_null(),
            "triage.counts.{key} must be null without a cut trace, not {value}"
        );
    }
}

#[test]
fn no_simulation_publishes_null_and_says_no_run() {
    let controller = sample_controller();
    assert!(controller.state.simulation.results.is_none());

    let diag = controller.build_mcp_diagnostics();

    for key in TRACE_FIGURES {
        assert!(diag[key].is_null(), "`{key}` must be null with no run");
    }
    assert_eq!(
        diag["cut_metrics_not_measured"].as_str(),
        Some("no simulation has run")
    );
}
