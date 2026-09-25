//! FM5 (viz door) — a Suggest refusal for lack of evidence does not stop the
//! operation from being created.
//!
//! Feeds matrix ruling R1 (2026-09-23) refuses a recipe where no published
//! figure backs the formula (`FeedsError::Unbacked`). The ruling is about the
//! recommendation, not about the operation: the GUI add door adds the
//! operation with the registry and stock defaults and no recipe, and shows the
//! refusal as one Warning toast. A tool the operation's registry row refuses
//! (`FeedsError::WrongToolForOperation`) still stops the add, as before.
//!
//! Fixture: a flat end mill, the generic wood router, the default stock
//! material. `AlignmentPinDrill` is stock-based (no model needed); the tool
//! is set to 15.875 mm so it sits outside the G6 drill claim (3.0-12.7 mm,
//! ruling B5, widened 2026-09-25), and the drill cell refuses. `Scallop`
//! with the same tool is the tool-rule control.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::session::ProjectSessionBuilder;
use rs_cam_viz::compute::{
    CollisionRequest, ComputeBackend, ComputeLane, ComputeMessage, ComputeRequest,
    GenerationControl, LaneSnapshot, OptimizeRequest, SimulationRequest, ToolpathSubmitOutcome,
};
use rs_cam_viz::controller::{AppController, Severity};
use rs_cam_viz::ui::AppEvent;

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

fn controller_with_end_mill() -> AppController<SilentBackend> {
    let mut controller = AppController::with_backend(SilentBackend);
    // 15.875 mm sits outside the G6 drill claim's 3.0-12.7 mm range, so
    // AlignmentPinDrill still refuses (the default 6.35 mm end mill no
    // longer does, since the range widened 2026-09-25).
    let mut tool = ToolConfig::new_default(ToolId(1), ToolType::EndMill);
    tool.diameter = 15.875;
    controller.state.session = ProjectSessionBuilder::new().tool(tool).build();
    controller
}

fn toasts(controller: &AppController<SilentBackend>) -> Vec<(String, Severity)> {
    controller
        .active_notifications()
        .map(|n| (n.message.clone(), n.severity))
        .collect()
}

/// An `Unbacked` refusal adds the operation without a recipe and says so once.
#[test]
fn an_unbacked_cell_still_adds_its_operation_fm5() {
    let mut controller = controller_with_end_mill();
    assert_eq!(controller.state.session.toolpath_count(), 0);

    controller.handle_internal_event(AppEvent::AddToolpath(OperationType::AlignmentPinDrill));

    assert_eq!(
        controller.state.session.toolpath_count(),
        1,
        "the operation must exist although Suggest refused a recipe"
    );
    let added = &controller.state.session.toolpath_configs()[0];
    assert_eq!(added.operation.op_type(), OperationType::AlignmentPinDrill);
    assert!(
        added.feeds_provenance.spindle_rpm.is_none() && added.feeds_provenance.feed_rate.is_none(),
        "no recipe was written, so no provenance may claim one"
    );

    let shown = toasts(&controller);
    let warnings: Vec<_> = shown
        .iter()
        .filter(|(_, s)| *s == Severity::Warning)
        .collect();
    assert_eq!(warnings.len(), 1, "exactly one Warning toast: {shown:?}");
    let label = OperationType::AlignmentPinDrill.spec().label;
    assert!(
        warnings[0].0.contains("Added without a feeds recipe")
            && warnings[0].0.contains(label)
            && warnings[0].0.contains("flat end mill")
            && !warnings[0].0.contains('{'),
        "the toast carries the refusal, names the operation and leaks no Debug output: {}",
        warnings[0].0
    );
}

/// Control: the registry's tool rule still stops the add.
#[test]
fn a_tool_rule_refusal_still_stops_the_add_fm5() {
    let mut controller = controller_with_end_mill();

    controller.handle_internal_event(AppEvent::AddToolpath(OperationType::Scallop));

    assert_eq!(
        controller.state.session.toolpath_count(),
        0,
        "a flat end mill cannot run Scallop; the add must stop"
    );
    let shown = toasts(&controller);
    assert_eq!(shown.len(), 1, "one Warning toast: {shown:?}");
    assert!(
        shown[0].0.contains("Cannot add toolpath")
            && shown[0]
                .0
                .contains("Scallop Finish needs a ball nose or tapered ball nose tool"),
        "unexpected toast: {}",
        shown[0].0
    );
}

/// The MCP add door carries the same refusal in its reply. The handler
/// runs inside the app, so this arm reads the source: the `Unbacked` arm is
/// the non-vacuity anchor, and the reply must read the text it stores.
#[test]
fn the_mcp_add_reply_carries_the_refusal_fm5() {
    const COMMANDS_SRC: &str = include_str!("../src/app/mcp/commands.rs");
    let unbacked_arm = COMMANDS_SRC
        .find("Err(e @ rs_cam_core::feeds::FeedsError::Unbacked { .. }) =>")
        .expect("anchor: the MCP add door has an Unbacked arm");
    let stores = COMMANDS_SRC[unbacked_arm..]
        .find("\"feeds_refusal\": feeds_refusal,")
        .expect("the Unbacked arm's text is stored on the row");
    assert!(
        COMMANDS_SRC[unbacked_arm + stores..].contains("without a feeds recipe: {feeds_refusal}"),
        "the add_toolpath reply must print the stored refusal"
    );
}
