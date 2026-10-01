//! U3, wave 3 group K (memory programme 2026-10-01): the PER-TOOLPATH
//! Optimize names the real cause when a simulation exists but kept no cut
//! trace.
//!
//! Wave 2 fixed the project-level Optimize (`optimize_names_the_missing_
//! trace_u3.rs`). The per-toolpath modal still opened a card that read "no
//! simulation has been run yet", directly after a run that kept no cut
//! trace. That run now gives the toast the project-level Optimize gives.
//! The true no-simulation case keeps its card.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::session::{ProjectSessionBuilder, ToolpathConfig};
use rs_cam_core::tool_load::RefuseReason;
use rs_cam_core::tool_load::optimize::OutcomeKind;
use rs_cam_viz::compute::{
    CollisionRequest, ComputeBackend, ComputeLane, ComputeMessage, ComputeRequest,
    GenerationControl, LaneSnapshot, OptimizeRequest, SimulationRequest, ToolpathSubmitOutcome,
};
use rs_cam_viz::controller::AppController;
use rs_cam_viz::state::OptimizeRunStatus;
use rs_cam_viz::state::simulation::SimulationResults;
use rs_cam_viz::state::toolpath::StockSource;
use rs_cam_viz::ui::AppEvent;
use rs_cam_viz::ui::menu_bar::OPTIMIZE_NEEDS_CUT_TRACE;

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

/// One enabled Face operation, id 0, so the handler finds a toolpath and
/// reaches `ProjectSession::start`. The session holds no simulation, so the
/// start refuses with `SimulationRequired`.
fn controller_with_one_op() -> AppController<SilentBackend> {
    let mut controller = AppController::with_backend(SilentBackend);
    let tool = ToolConfig::new_default(ToolId(1), ToolType::EndMill);
    let op = ToolpathConfig {
        id: rs_cam_core::ToolpathId(0),
        name: "Face".to_owned(),
        enabled: true,
        operation: OperationConfig::new_default(OperationType::Face),
        dressups: Default::default(),
        heights: Default::default(),
        tool_id: 1,
        model_id: 0,
        pre_gcode: None,
        post_gcode: None,
        boundary: Default::default(),
        boundary_inherit: true,
        rest_analysis: Default::default(),
        stock_source: StockSource::Fresh,
        coolant: Default::default(),
        face_selection: None,
        debug_options: Default::default(),
        feeds_provenance: Default::default(),
        planner_origin: None,
    };
    let mut builder = ProjectSessionBuilder::new().tool(tool);
    let _ = builder.add_toolpath(0, op).expect("add op");
    controller.state.session = builder.build();
    controller
}

/// A simulation result with no cut trace in the view state.
fn trace_less_results() -> SimulationResults {
    SimulationResults {
        mesh: std::sync::Arc::new(rs_cam_core::stock::stock_mesh::StockMesh {
            vertices: Vec::new(),
            indices: Vec::new(),
            colors: Vec::new(),
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
    let mut controller = controller_with_one_op();
    controller.state.simulation.results = Some(trace_less_results());

    controller.handle_internal_event(AppEvent::OpenOptimizeModal(rs_cam_core::ToolpathId(0)));

    assert!(
        controller.state.optimize_modal.is_none(),
        "the 'no simulation has been run yet' card must not open after a run"
    );
    let texts = notification_texts(&controller);
    assert_eq!(texts, vec![OPTIMIZE_NEEDS_CUT_TRACE.to_owned()]);
    let text = &texts[0];
    assert!(!text.to_lowercase().contains("simulation first"), "{text}");
    assert!(!text.contains("Capture cutting metrics"), "{text}");
    assert!(
        text.to_lowercase().contains("re-run the simulation"),
        "{text}"
    );
}

#[test]
fn no_simulation_keeps_the_simulation_required_card() {
    let mut controller = controller_with_one_op();
    assert!(controller.state.simulation.results.is_none());

    controller.handle_internal_event(AppEvent::OpenOptimizeModal(rs_cam_core::ToolpathId(0)));

    let modal = controller
        .state
        .optimize_modal
        .as_ref()
        .expect("the true no-simulation case keeps its card");
    let OptimizeRunStatus::Ready(outcome) = &modal.status else {
        panic!("the card is a settled Skipped outcome");
    };
    assert!(matches!(outcome.kind, OutcomeKind::Skipped));
    assert_eq!(outcome.reason, Some(RefuseReason::SimulationRequired));
    assert!(notification_texts(&controller).is_empty());
}
