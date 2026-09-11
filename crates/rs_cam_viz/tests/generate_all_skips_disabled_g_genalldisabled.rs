//! G-GENALLDISABLED — Generate All submits only ENABLED operations.
//!
//! `handle_generate_all` has two arms. The ladder arm submits
//! `generate_all_scope(..).enabled`. The arm beside it — taken whenever the
//! project holds no enabled `FromRemainingStock` op, which is most projects —
//! iterated `toolpath_configs()` and submitted every one of them, enabled or
//! not. So whether the enable toggle was honoured depended on whether the
//! project happened to contain a rest chain.
//!
//! Disabling an op promises exclusion "from generation, simulation and
//! output" (`toolpath_row_controls.rs` hover). Core's own `generate_all` skips
//! disabled ops and `io::export::emitted_toolpaths` filters them out, so this
//! was the one surface that generated them anyway: compute-lane time spent on
//! work nothing downstream would read, and a disabled card left reading `OK`.
//!
//! Source: `planning/ui_review_2026-09-09/results/W03/support/r05_source_track.md`
//! §4 item 2 and §8.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::sync::{Arc, Mutex};

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::session::{ProjectSessionBuilder, ToolpathConfig};
use rs_cam_viz::compute::{
    CollisionRequest, ComputeBackend, ComputeLane, ComputeMessage, ComputeRequest,
    GenerationControl, LaneSnapshot, OptimizeRequest, SimulationRequest, ToolpathSubmitOutcome,
};
use rs_cam_viz::controller::AppController;
use rs_cam_viz::state::toolpath::{StockSource, ToolpathId};
use rs_cam_viz::ui::AppEvent;

/// A backend that accepts everything and records which toolpath ids reached
/// the lane. What Generate All SUBMITS is the whole question here, so the
/// submission list is the measurement.
#[derive(Clone, Default)]
struct RecordingBackend {
    submitted: Arc<Mutex<Vec<ToolpathId>>>,
}

impl ComputeBackend for RecordingBackend {
    fn submit_toolpath(&mut self, request: ComputeRequest) -> ToolpathSubmitOutcome {
        self.submitted
            .lock()
            .expect("submission log")
            .push(request.toolpath_id);
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

fn toolpath(id: usize, enabled: bool) -> ToolpathConfig {
    ToolpathConfig {
        id: rs_cam_core::ToolpathId(id),
        name: format!("Op {id}"),
        enabled,
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
    }
}

/// Three ops, the middle one disabled, and no rest chain anywhere — the arm
/// that used to ignore `enabled`.
fn controller_with_a_disabled_op() -> (AppController<RecordingBackend>, Arc<Mutex<Vec<ToolpathId>>>)
{
    let backend = RecordingBackend::default();
    let log = Arc::clone(&backend.submitted);
    let mut controller = AppController::with_backend(backend);
    controller.state.session = ProjectSessionBuilder::new()
        .tool(ToolConfig::new_default(ToolId(1), ToolType::EndMill))
        .build();
    for (id, enabled) in [(0, true), (1, false), (2, true)] {
        controller
            .state
            .session
            .add_toolpath(0, toolpath(id, enabled))
            .expect("add op");
    }
    (controller, log)
}

/// THE sentry. The disabled op must not reach the compute lane. Pre-fix the
/// log reads `[0, 1, 2]`.
#[test]
fn generate_all_without_a_chain_submits_only_enabled_ops() {
    let (mut controller, log) = controller_with_a_disabled_op();

    controller.handle_internal_event(AppEvent::GenerateAll);
    // `handle_generate_all` submits this arm directly, but drain any events
    // it queued so the assertion covers both routes to the lane.
    for event in controller.drain_events() {
        controller.handle_internal_event(event);
    }

    let submitted = log.lock().expect("submission log").clone();
    assert!(
        !submitted.contains(&ToolpathId(1)),
        "a disabled operation must not be generated: {submitted:?}"
    );
    assert_eq!(
        submitted,
        vec![ToolpathId(0), ToolpathId(2)],
        "Generate All submits exactly the enabled ops, in plan order"
    );
}

/// A project with nothing enabled says so rather than firing a Generate All
/// that submits nothing. The ladder arm already did this; the other arm was
/// silent.
#[test]
fn generate_all_with_nothing_enabled_tells_the_operator() {
    let backend = RecordingBackend::default();
    let log = Arc::clone(&backend.submitted);
    let mut controller = AppController::with_backend(backend);
    controller.state.session = ProjectSessionBuilder::new()
        .tool(ToolConfig::new_default(ToolId(1), ToolType::EndMill))
        .build();
    controller
        .state
        .session
        .add_toolpath(0, toolpath(0, false))
        .expect("add op");

    controller.handle_internal_event(AppEvent::GenerateAll);
    for event in controller.drain_events() {
        controller.handle_internal_event(event);
    }

    assert!(
        log.lock().expect("submission log").is_empty(),
        "nothing enabled means nothing submitted"
    );
    assert!(
        controller
            .active_notifications()
            .any(|n| n.message.contains("No enabled toolpaths")),
        "the operator must be told why Generate All did nothing"
    );
}
