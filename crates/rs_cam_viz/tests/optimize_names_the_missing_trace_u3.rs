//! U3 (memory programme 2026-10-01): Optimize names the real cause when a
//! simulation exists but kept no cut trace.
//!
//! A GUI run with the old capture checkbox off kept no cut trace, so
//! "Optimize project…" had no baseline. The menu item was enabled (it read
//! only "a simulation exists"), and the click pushed "Run a simulation
//! first", directly after a run. The cause is the trace. Every run captures
//! the trace since the operator ruling of 2026-10-02 ("always capture"), so
//! the remedy is a re-run, and no text names the deleted checkbox. The true
//! "no simulation" case keeps its old text.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_viz::compute::{
    CollisionRequest, ComputeBackend, ComputeLane, ComputeMessage, ComputeRequest,
    GenerationControl, LaneSnapshot, OptimizeRequest, SimulationRequest, ToolpathSubmitOutcome,
};
use rs_cam_viz::controller::AppController;
use rs_cam_viz::state::simulation::SimulationResults;
use rs_cam_viz::ui::AppEvent;
use rs_cam_viz::ui::menu_bar::{
    OPTIMIZE_NEEDS_CUT_TRACE, OPTIMIZE_NEEDS_SIMULATION, optimize_baseline_missing,
};

struct SilentBackend;

impl ComputeBackend for SilentBackend {
    fn submit_toolpath(&mut self, _request: ComputeRequest) -> ToolpathSubmitOutcome {
        ToolpathSubmitOutcome::Queued
    }
    fn submit_simulation(&mut self, _request: SimulationRequest) {}
    fn submit_collision(&mut self, _request: CollisionRequest) {}
    fn submit_optimize(&mut self, _request: OptimizeRequest) {}
    fn cancel_lane(&mut self, _lane: ComputeLane) {}
    fn drain_results(&mut self) -> Vec<ComputeMessage> {
        Vec::new()
    }
    fn lane_snapshot(&self, lane: ComputeLane) -> LaneSnapshot {
        LaneSnapshot::idle(lane)
    }
    fn generation_control(&self) -> GenerationControl {
        GenerationControl::detached()
    }
}

/// A simulation result with no cut trace in the view state.
fn trace_less_results() -> SimulationResults {
    SimulationResults {
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

fn notification_texts(controller: &AppController<SilentBackend>) -> Vec<String> {
    controller
        .notifications()
        .iter()
        .map(|n| n.message.clone())
        .collect()
}

#[test]
fn a_trace_less_run_names_the_missing_trace_not_run_a_simulation() {
    let mut controller = AppController::with_backend(SilentBackend);
    controller.state.simulation.results = Some(trace_less_results());

    // The menu reads this decision for its enabled state and hover text.
    assert_eq!(
        optimize_baseline_missing(&controller.state),
        Some(OPTIMIZE_NEEDS_CUT_TRACE),
        "a trace-less run disables Optimize and names the missing trace"
    );

    controller.handle_internal_event(AppEvent::OpenOptimizeProject);

    let texts = notification_texts(&controller);
    assert_eq!(texts.len(), 1, "one refusal toast: {texts:?}");
    let text = &texts[0];
    assert!(
        !text.contains("Run a simulation first"),
        "a simulation exists, so this names the wrong cause: {text}"
    );
    assert!(text.contains("no cut trace"), "{text}");
    assert!(!text.contains("Capture cutting metrics"), "{text}");
    assert!(
        text.to_lowercase().contains("re-run the simulation"),
        "{text}"
    );
}

#[test]
fn no_simulation_keeps_run_a_simulation_first() {
    let mut controller = AppController::with_backend(SilentBackend);
    assert!(controller.state.simulation.results.is_none());

    assert_eq!(
        optimize_baseline_missing(&controller.state),
        Some(OPTIMIZE_NEEDS_SIMULATION)
    );

    controller.handle_internal_event(AppEvent::OpenOptimizeProject);

    let texts = notification_texts(&controller);
    assert_eq!(texts.len(), 1, "one refusal toast: {texts:?}");
    assert!(
        texts[0].starts_with("Run a simulation first"),
        "the true no-simulation case keeps its text: {}",
        texts[0]
    );
}
