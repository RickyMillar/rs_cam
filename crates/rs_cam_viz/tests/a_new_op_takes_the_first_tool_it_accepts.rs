//! A new operation takes the FIRST tool of the project that it accepts,
//! in tool-list order, not simply the first tool.
//!
//! # The claim
//!
//! - A project with tools `[V-bit, end mill]`: the GUI add of a Bore
//!   (`AppEvent::AddToolpath`) creates one Bore bound to the end mill.
//!   The V-bit is skipped without a toast.
//! - R2: a project with only a V-bit: the add makes the Bore with NO
//!   tool, and one Warning toast names the state ("No tool fits: needs a
//!   flat end mill or a bull nose smaller than Ø10.00 ...") and the fix. Generation refuses with
//!   `SessionError::NoTool`.
//!
//! The operator saw the defect on 2026-10-22: Add toolpath → Bore did
//! nothing, because the first tool was a V-bit.
//!
//! Red proof: put back the old rule (`tools().first()`) in
//! `AppController::handle_add_toolpath`. The first test fails.
//!
//! ```text
//! cargo test -p rs_cam_viz -q --test a_new_op_takes_the_first_tool_it_accepts
//! ```

#![expect(
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::path::PathBuf;
use std::sync::Arc;

use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::geo::P2;
use rs_cam_core::selection::{Item, ItemSet, Shape, SourceRef};
use rs_cam_core::session::{LoadedModel, ProjectSessionBuilder};
use rs_cam_viz::compute::{
    CollisionRequest, ComputeBackend, ComputeLane, ComputeMessage, ComputeRequest,
    GenerationControl, LaneSnapshot, SimulationRequest, ToolpathSubmitOutcome,
};
use rs_cam_viz::controller::{AppController, Severity};
use rs_cam_viz::ui::AppEvent;

/// A backend that runs nothing: the test reads the session only.
struct SilentBackend;

impl ComputeBackend for SilentBackend {
    fn submit_toolpath(&mut self, _request: ComputeRequest) -> ToolpathSubmitOutcome {
        ToolpathSubmitOutcome::Queued
    }
    fn submit_simulation(&mut self, _request: SimulationRequest) {}
    fn submit_collision(&mut self, _request: CollisionRequest) {}
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

/// A 2D drawing with one 10 mm circle, so a Bore has geometry.
fn drawing() -> LoadedModel {
    let circle = Item::new(
        SourceRef::ToolpathModel,
        None,
        Shape::circle(P2::new(0.0, 0.0), 10.0),
    )
    .unwrap_or_else(|e| panic!("a circle: {e:?}"));
    LoadedModel {
        id: rs_cam_core::ids::ModelId::from_raw(1),
        path: PathBuf::from("first_fitting_tool.svg"),
        name: "First fitting tool".to_owned(),
        kind: Some(rs_cam_core::compute::stock_config::ModelKind::Svg),
        mesh: None,
        items: Arc::new(ItemSet::new(vec![circle])),
        enriched_mesh: None,
        units: Some(rs_cam_core::compute::stock_config::ModelUnits::Millimeters),
        winding_report: None,
        load_error: None,
        import_options: Default::default(),
        import_report: None,
        drawing: Default::default(),
    }
}

/// The operator's V-bit: Ø6, 90°.
fn v_bit(id: ToolId) -> ToolConfig {
    let mut tool = ToolConfig::new_default(id, ToolType::VBit);
    tool.name = "V-Bittest".to_owned();
    tool.diameter = 6.0;
    tool
}

fn end_mill(id: ToolId) -> ToolConfig {
    let mut tool = ToolConfig::new_default(id, ToolType::EndMill);
    tool.diameter = 6.0;
    tool
}

fn controller(tools: Vec<ToolConfig>) -> AppController<SilentBackend> {
    let mut builder = ProjectSessionBuilder::new().model(drawing());
    for tool in tools {
        builder = builder.tool(tool);
    }
    let mut controller = AppController::with_backend(SilentBackend);
    controller.state.replace_session(builder.build());
    controller
}

fn toasts(controller: &AppController<SilentBackend>) -> Vec<(String, Severity)> {
    controller
        .notifications()
        .iter()
        .map(|n| (n.message.clone(), n.severity))
        .collect()
}

#[test]
fn a_new_op_takes_the_first_tool_it_accepts() {
    let mut controller = controller(vec![
        v_bit(ToolId::from_raw(0)),
        end_mill(ToolId::from_raw(1)),
    ]);
    let end_mill_id = controller.state.session().tools()[1].id;

    controller.handle_internal_event(AppEvent::AddToolpath(OperationType::Bore));

    let configs = controller.state.session().toolpath_configs();
    assert_eq!(
        configs.len(),
        1,
        "the end mill fits a Bore, so one Bore: {:?}",
        toasts(&controller)
    );
    assert_eq!(configs[0].operation.op_type(), OperationType::Bore);
    assert_eq!(
        configs[0].tool_id,
        Some(end_mill_id),
        "the Bore takes the end mill, not the V-bit before it"
    );
    let warnings: Vec<_> = toasts(&controller)
        .into_iter()
        .filter(|(_, s)| *s != Severity::Info)
        .collect();
    assert!(
        warnings.iter().all(|(m, _)| !m.contains("V-bit")),
        "the skipped V-bit gives no toast: {warnings:?}"
    );
}

/// R2 (Fusion, operator ruling 2026-10-23): an add that no project tool
/// fits still MAKES the op, with no tool. One Warning toast names the state
/// and the fix, and generation refuses with the same fix.
///
/// Red proof: make `draft_toolpath` refuse when `FirstFitting` finds no
/// tool (the old rule). The op count is 0 and the test fails.
#[test]
fn an_add_that_no_tool_fits_makes_the_op_with_no_tool_r2() {
    let mut controller = controller(vec![v_bit(ToolId::from_raw(0))]);

    controller.handle_internal_event(AppEvent::AddToolpath(OperationType::Bore));

    let configs = controller.state.session().toolpath_configs();
    assert_eq!(
        configs.len(),
        1,
        "the op is made: {:?}",
        toasts(&controller)
    );
    assert_eq!(configs[0].operation.op_type(), OperationType::Bore);
    assert_eq!(configs[0].tool_id, None, "no tool fits, so the op has none");
    // Q9: a Bore state names the size limit of its held circle (Ø10).
    let state = "No tool fits: needs a flat end mill or a bull nose \
                 smaller than Ø10.00 and at least Ø5.00";
    assert_eq!(
        controller.state.session().no_tool_state(0).as_deref(),
        Some(state)
    );
    let shown = toasts(&controller);
    assert_eq!(shown.len(), 1, "exactly one toast: {shown:?}");
    assert_eq!(shown[0].1, Severity::Warning, "{shown:?}");
    assert_eq!(
        shown[0].0,
        format!(
            "Added with no tool: {state}. {}.",
            rs_cam_core::session::NO_TOOL_FIX
        ),
        "the toast names the state and the fix"
    );
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let refusal = controller
        .state
        .session()
        .resolve_generation_inputs(0, &cancel)
        .err()
        .unwrap_or_else(|| panic!("an op with no tool must refuse generation"));
    assert!(
        matches!(refusal, rs_cam_core::session::SessionError::NoTool { .. }),
        "{refusal:?}"
    );
    assert_eq!(
        refusal.to_string(),
        format!(
            "Toolpath {}: {state}. {}",
            configs[0].id,
            rs_cam_core::session::NO_TOOL_FIX
        )
    );
}
