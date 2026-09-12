//! G-RESNOTICE — the fixpoint ladder says when it takes over the operator's
//! simulation resolution.
//!
//! Between generate rounds `settle_generate_all_round` writes
//! `SimulationState::resolution` and clears `auto_resolution`, and the new
//! values STAY after the run: every later simulation, collision count and
//! engagement figure is then measured at whatever cell the ladder chose. It
//! did that silently.
//!
//! The GUI arm can only ever write back a value the operator pinned
//! themselves — `pinned_simulation_resolution` refuses `auto` — so it changes
//! nothing and says nothing. An MCP `generate_all` carries its own
//! `simulation_resolution_mm`, so an agent's argument can overrule both dials
//! in a GUI the operator is watching. That is the case this names.
//!
//! This is a truthfulness fix only: **when** and **whether** the ladder
//! rewrites the setting is unchanged, and no threshold moved.
//!
//! These tests live beside the code because the two entry points they drive,
//! `start_generate_all` and `drain_compute_results`, are `pub(crate)`.
//!
//! Source: `planning/ui_review_2026-09-09/results/W03/support/r05_source_track.md`
//! §5 ("it **writes the operator's simulation settings**").

use std::sync::Arc;

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::session::{ProjectSessionBuilder, ToolpathConfig};

use crate::compute::{
    CollisionRequest, ComputeBackend, ComputeLane, ComputeMessage, ComputeRequest,
    GenerationControl, LaneSnapshot, OptimizeRequest, SimulationRequest, ToolpathSubmitOutcome,
};
use crate::controller::AppController;
use crate::controller::generate_all::{GenerateAllSink, plan_fixpoint, resolution_override_notice};
use crate::state::toolpath::{StockSource, ToolpathId};
use crate::ui::AppEvent;

// ── the message ────────────────────────────────────────────────────────────

/// The operator has to be able to put the old cell size back, so the notice
/// carries both numbers and names the checkbox it cleared.
#[test]
fn the_notice_names_the_old_value_and_the_new_one() {
    let notice = resolution_override_notice(0.300, true, 0.150)
        .expect("auto → pinned at a different cell is a change worth saying");
    assert!(notice.contains("0.150"), "the new cell size: {notice}");
    assert!(notice.contains("0.300"), "the old cell size: {notice}");
    assert!(
        notice.contains("auto from tool size"),
        "the old setting was auto, and that is part of what changed: {notice}"
    );
    assert!(
        notice.contains("Auto from tool size"),
        "name the control the operator has to touch to undo it: {notice}"
    );
}

/// Clearing `auto` is a change even at the same number: auto re-derives the
/// cell per simulation, so the next run would have measured somewhere else.
#[test]
fn clearing_auto_is_a_change_even_at_the_same_number() {
    assert!(
        resolution_override_notice(0.150, true, 0.150).is_some(),
        "auto at 0.150 and pinned at 0.150 are different settings"
    );
}

/// The GUI arm writes back exactly what the operator pinned. Saying "we
/// changed it" there would be the same class of untruth this fix removes.
#[test]
fn writing_back_the_pinned_value_says_nothing() {
    assert!(resolution_override_notice(0.150, false, 0.150).is_none());
}

// ── the ladder ─────────────────────────────────────────────────────────────

/// Accepts every submission and hands back whatever has been queued on it.
#[derive(Default)]
struct ScriptedLane {
    drained: Vec<ComputeMessage>,
}

impl ComputeBackend for ScriptedLane {
    fn submit_toolpath(&mut self, _request: ComputeRequest) -> ToolpathSubmitOutcome {
        ToolpathSubmitOutcome::Queued
    }
    fn submit_simulation(&mut self, _request: SimulationRequest) {}
    fn submit_collision(&mut self, _request: CollisionRequest) {}
    fn submit_optimize(&mut self, _request: OptimizeRequest) {}
    fn cancel_lane(&mut self, _lane: ComputeLane) {}
    fn drain_results(&mut self) -> Vec<ComputeMessage> {
        std::mem::take(&mut self.drained)
    }
    fn lane_snapshot(&self, lane: ComputeLane) -> LaneSnapshot {
        LaneSnapshot::idle(lane)
    }
    fn generation_control(&self) -> GenerationControl {
        GenerationControl::detached()
    }
}

fn toolpath(id: usize, stock_source: StockSource) -> ToolpathConfig {
    ToolpathConfig {
        id: rs_cam_core::ToolpathId(id),
        name: format!("Op {id}"),
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
        stock_source,
        coolant: Default::default(),
        face_selection: None,
        debug_options: Default::default(),
        feeds_provenance: Default::default(),
        planner_origin: None,
    }
}

/// Op 0 cuts fresh stock; op 1 takes the stock op 0 leaves, and no simulation
/// has run, so op 1 blocks on submit — exactly the shape that makes the
/// ladder advance a round.
fn controller_with_a_chain() -> AppController<ScriptedLane> {
    let mut controller = AppController::with_backend(ScriptedLane::default());
    let tool = ToolConfig::new_default(ToolId(1), ToolType::EndMill);
    let mut builder = ProjectSessionBuilder::new().tool(tool);
    builder
        .add_toolpath(0, toolpath(0, StockSource::Fresh))
        .expect("fresh op");
    builder
        .add_toolpath(0, toolpath(1, StockSource::FromRemainingStock))
        .expect("rest op");
    controller.state.session = builder.build();
    controller
}

/// Finish op 0 the way the lane would, so the round can settle.
fn land_a_result_for(controller: &mut AppController<ScriptedLane>, tp_id: ToolpathId) {
    let annotated = Arc::new(rs_cam_core::toolpath_spans::AnnotatedToolpath::new(
        rs_cam_core::toolpath::Toolpath::new(),
    ));
    controller
        .compute
        .drained
        .push(ComputeMessage::Toolpath(Box::new(
            crate::compute::worker::ComputeResult {
                toolpath_id: tp_id,
                revision: None,
                result: Ok(crate::state::toolpath::ToolpathResult {
                    annotated,
                    stats: Default::default(),
                    debug_trace: None,
                    semantic_trace: None,
                    debug_trace_path: None,
                    drill_op: None,
                }),
                debug_trace: None,
                semantic_trace: None,
                debug_trace_path: None,
            },
        )));
    controller.drain_compute_results();
}

/// Drive one ladder whose resolution differs from the operator's dials, the
/// way an MCP `generate_all` does, and return the controller after the round
/// has settled into its simulation.
fn run_one_round_with_a_ladder_resolution(
    current_mm: f64,
    current_auto: bool,
    ladder_mm: f64,
) -> AppController<ScriptedLane> {
    let mut controller = controller_with_a_chain();
    controller.state.simulation.resolution = current_mm;
    controller.state.simulation.auto_resolution = current_auto;

    let plan = plan_fixpoint(true, &[1], Some(ladder_mm)).expect("a cell size was supplied");
    controller.start_generate_all(
        vec![ToolpathId(0), ToolpathId(1)],
        plan,
        GenerateAllSink::Gui,
    );

    // The ladder queues a `GenerateToolpath` per op; op 1 blocks on submit.
    for event in controller.drain_events() {
        if matches!(event, AppEvent::GenerateToolpath(id) if id == ToolpathId(1)) {
            controller.handle_internal_event(event);
        }
    }
    land_a_result_for(&mut controller, ToolpathId(0));
    controller
}

/// THE sentry. The ladder still rewrites both dials — and now the operator is
/// told, with the value that was replaced. Pre-fix the write happened and no
/// notification carried the word "resolution" at all.
#[test]
fn an_overruled_resolution_reaches_the_operator() {
    let controller = run_one_round_with_a_ladder_resolution(0.300, true, 0.150);

    assert!(
        (controller.state.simulation.resolution - 0.150).abs() < 1e-9,
        "the ladder's write is unchanged by this fix"
    );
    assert!(
        !controller.state.simulation.auto_resolution,
        "the ladder's write is unchanged by this fix"
    );

    let notice = controller
        .active_notifications()
        .find(|n| {
            n.message
                .contains("Generate All set the simulation resolution")
        })
        .map(|n| n.message.clone())
        .expect("the operator's dial changed under them and must be told");
    assert!(notice.contains("0.150"), "{notice}");
    assert!(notice.contains("0.300"), "{notice}");
}

/// The GUI's own Generate All can only write back what the operator pinned,
/// so the same round stays silent. A notice on every ladder would be noise
/// that trains the operator to ignore the one that matters.
#[test]
fn a_ladder_that_changes_nothing_stays_silent() {
    let controller = run_one_round_with_a_ladder_resolution(0.150, false, 0.150);

    assert!(
        controller.active_notifications().all(|n| !n
            .message
            .contains("Generate All set the simulation resolution")),
        "nothing changed, so there is nothing to report"
    );
}
