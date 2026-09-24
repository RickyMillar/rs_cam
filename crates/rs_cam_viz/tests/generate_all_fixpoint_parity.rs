//! GUI Generate All and the MCP `generate_all` tool walk the SAME plan.
//!
//! A/M11 landed the rest-stock ladder behind `#[cfg(feature = "mcp")]`, so
//! the GUI stayed single-pass and told the operator to loop by hand. The
//! multi-tool planner emits a k-op coarse-to-fine chain from one action,
//! which makes "loop it by hand k times" untenable.
//!
//! What is asserted here is the GUI entry, not the walk. G-RESTRES
//! (2026-09-24) retired the MCP `require_resolution` decision: every surface
//! reads the ONE stored project resolution now. W1 inverted one of those ends. A/M10's rule — the cell size is
//! never chosen SILENTLY — is an MCP rule: an agent's cell size arrives as an
//! argument or not at all. In the GUI the Simulation panel's setting IS the
//! operator's standing choice, because Run Simulation uses it as it stands,
//! so reading it is not a default (R1). The GUI now arms a plan on auto, and
//! asks only when the panel's PINNED value is coarser than the rest needs.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::session::{ProjectSessionBuilder, ToolpathConfig};
use rs_cam_viz::compute::{
    CollisionRequest, ComputeBackend, ComputeLane, ComputeMessage, ComputeRequest,
    GenerationControl, LaneSnapshot, OptimizeRequest, SimulationRequest, ToolpathSubmitOutcome,
};
use rs_cam_viz::controller::AppController;
use rs_cam_viz::state::toolpath::StockSource;
use rs_cam_viz::ui::AppEvent;

// ── the GUI entry ──────────────────────────────────────────────────────────

/// A compute backend that accepts every submission and returns nothing. These
/// tests assert what `handle_generate_all` arms, not what the lane does with
/// it, so a permanently idle lane is a faithful stand-in.
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

fn toolpath(id: usize, stock_source: StockSource) -> ToolpathConfig {
    ToolpathConfig {
        id: rs_cam_core::ToolpathId(id),
        name: format!("Finish {id}"),
        enabled: true,
        operation: OperationConfig::new_default(OperationType::UnifiedFinish),
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

/// A project whose second op takes the remaining stock of the first.
fn controller_with_chain(rest_ops: usize) -> AppController<SilentBackend> {
    let mut controller = AppController::with_backend(SilentBackend);
    let tool = ToolConfig::new_default(ToolId(1), ToolType::BallNose);
    let mut builder = ProjectSessionBuilder::new().tool(tool);
    let _ = builder
        .add_toolpath(0, toolpath(0, StockSource::Fresh))
        .expect("first op");
    for i in 1..=rest_ops {
        let _ = builder
            .add_toolpath(0, toolpath(i, StockSource::FromRemainingStock))
            .expect("rest op");
    }
    controller.state.session = builder.build();
    controller
}

/// The one line every finished plan leaves behind.
fn summary(controller: &AppController<SilentBackend>) -> String {
    controller
        .notifications()
        .iter()
        .map(|n| n.message.clone())
        .find(|m| m.starts_with("Generated "))
        .expect("every plan reports one summary")
}

/// THE Phase O gate. With a chain present, the GUI's Generate All walks a
/// plan — the same plan the MCP path walks — instead of firing a single pass
/// and leaving the rest ops blocked. Six steps: three generations, one prefix
/// simulation per rest op, and the closing full simulation.
///
/// `SilentBackend` has no model, so every submit refuses at validation and
/// the plan walks to its end inside the call. That is what makes the STEP
/// COUNT observable here; what the lane does with a submit is the controller
/// sentry's subject, not this file's.
#[test]
fn gui_generate_all_walks_a_plan_when_a_resolution_is_pinned() {
    let mut controller = controller_with_chain(2);
    set_resolution(
        &mut controller,
        rs_cam_core::session::SimulationResolution::Fixed(0.02),
    );

    controller.handle_internal_event(AppEvent::GenerateAll);

    let summary = summary(&controller);
    assert!(
        summary.contains("in 6 steps"),
        "a chain of two rest ops plans three generations, two prefix \
         simulations and one closing simulation: {summary}"
    );
    assert!(
        controller.pending_plan_confirm().is_none(),
        "a cell size finer than the rest needs asks nothing"
    );
    assert!(
        controller
            .active_notifications()
            .all(|n| !n.message.contains("resolution")),
        "nothing to refuse here"
    );
}

/// R1, and the test this INVERTS. The old rule refused to start on "Auto from
/// tool size", which is the panel's default, so the operator's first Generate
/// All was an error message. The panel's setting is their standing choice; the
/// plan reads it, narrows the REQUEST to what the rest needs, and says
/// nothing.
#[test]
fn gui_generate_all_reads_the_panel_resolution_r1() {
    let mut controller = controller_with_chain(2);
    set_resolution(
        &mut controller,
        rs_cam_core::session::SimulationResolution::Auto,
    );

    controller.handle_internal_event(AppEvent::GenerateAll);

    assert!(
        summary(&controller).contains("in 6 steps"),
        "auto resolution must walk a plan, not refuse one"
    );
    assert!(
        controller.pending_plan_confirm().is_none(),
        "auto asks nothing: the plan narrows the request, not the panel"
    );
    assert!(
        controller
            .active_notifications()
            .all(|n| !n.message.contains("resolution")),
        "the operator is asked for nothing"
    );
    assert_eq!(
        controller.state.session.simulation_resolution(),
        rs_cam_core::session::SimulationResolution::Auto,
        "the plan does not write the stored value unless the operator says so"
    );
}

/// Write the ONE stored simulation resolution (G-RESTRES) through its
/// command, the door the Simulation panel takes.
fn set_resolution<B: ComputeBackend>(
    controller: &mut AppController<B>,
    resolution: rs_cam_core::session::SimulationResolution,
) {
    let _ = controller
        .state
        .session
        .apply(rs_cam_core::session::Command::SetSimulationResolution(
            rs_cam_core::session::SetSimulationResolutionArgs { resolution },
        ))
        .expect("a positive cell size");
}

/// The other inversion. A project with no rest ops used to take a bare submit
/// loop with no plan state and no summary; it now walks the same plan, so one
/// summary exists.
#[test]
fn gui_generate_all_without_a_chain_still_walks_a_plan() {
    let mut controller = controller_with_chain(0);

    controller.handle_internal_event(AppEvent::GenerateAll);

    let summary = summary(&controller);
    assert!(
        summary.contains("in 2 steps"),
        "one generation and the closing simulation: {summary}"
    );
    assert!(
        controller
            .active_notifications()
            .all(|n| !n.message.contains("resolution")),
        "a project with no chain must never be asked for a simulation resolution"
    );
}

// ── the planner trigger's registration ─────────────────────────────────────

/// O-B2 — the tool an agent actually sees. Its schema is what tells the model
/// which dials exist and which are required, so a silent registration or
/// schema regression is a real defect on this surface.
#[cfg(feature = "mcp")]
#[test]
fn plan_multitool_finishing_is_registered_with_its_dials() {
    let router = rs_cam_viz::mcp_server::EmbeddedCamServer::into_tool_router();
    let tool = router
        .list_all()
        .into_iter()
        .find(|t| t.name == "plan_multitool_finishing")
        .expect("the embedded server must register `plan_multitool_finishing`");

    let schema = serde_json::Value::Object((*tool.input_schema).clone());
    for field in [
        "setup_index",
        "model_id",
        "tool_ids",
        "cell_mm",
        "tolerance_mm",
        "margin_mm",
        "cusp_height_mm",
        "coarseness",
        "overlap_mm",
        "max_regions_per_tier",
    ] {
        assert!(
            schema.pointer(&format!("/properties/{field}")).is_some(),
            "missing `{field}` in {schema}"
        );
    }
    assert!(
        schema.pointer("/properties/treatment").is_none(),
        "residual treatment is the B1 decision, not a dial: {schema}"
    );

    let required: Vec<String> = schema
        .pointer("/required")
        .and_then(|r| r.as_array())
        .map(|r| {
            r.iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    assert!(required.contains(&"setup_index".to_owned()), "{required:?}");
    assert!(required.contains(&"tool_ids".to_owned()), "{required:?}");
    for dial in ["cell_mm", "tolerance_mm", "coarseness", "overlap_mm"] {
        assert!(
            !required.contains(&dial.to_owned()),
            "`{dial}` must be optional so unset can mean the campaign default"
        );
    }

    let description = tool.description.map(|d| d.into_owned()).unwrap_or_default();
    assert!(
        description.contains("TIP-sphere radius"),
        "which radius orders the ladder is not inferable — on a tapered ball the \
         envelope radius would call the finest tool the coarsest: {description}"
    );
    assert!(
        description.contains("0.15"),
        "the planning-resolution trap (never plan at 0.15 mm) must be stated: {description}"
    );
    assert!(
        description.contains("preview_tier_map"),
        "the planner must point at its look-before-emit twin: {description}"
    );
}

/// U-B2 — the preview twin. It must offer the SAME dials as the planner (an
/// agent that previews at one coarseness and plans at another is looking at a
/// picture of a different job), plus `svg_path`, and its description must say
/// the two things that are not inferable from the schema: that it modifies
/// nothing, and that unlike the planner it is NOT cheap.
#[cfg(feature = "mcp")]
#[test]
fn preview_tier_map_is_registered_with_the_planners_dials() {
    let router = rs_cam_viz::mcp_server::EmbeddedCamServer::into_tool_router();
    let tools = router.list_all();
    let preview = tools
        .iter()
        .find(|t| t.name == "preview_tier_map")
        .expect("the embedded server must register `preview_tier_map`");
    let plan = tools
        .iter()
        .find(|t| t.name == "plan_multitool_finishing")
        .expect("the embedded server must register `plan_multitool_finishing`");

    fn property_names(schema: &serde_json::Value) -> Vec<String> {
        let mut names: Vec<String> = schema
            .pointer("/properties")
            .and_then(|p| p.as_object())
            .map(|o| o.keys().cloned().collect())
            .unwrap_or_default();
        names.sort();
        names
    }

    let preview_schema = serde_json::Value::Object((*preview.input_schema).clone());
    let plan_schema = serde_json::Value::Object((*plan.input_schema).clone());
    let mut preview_fields = property_names(&preview_schema);
    assert!(
        preview_fields.contains(&"svg_path".to_owned()),
        "{preview_fields:?}"
    );
    preview_fields.retain(|n| n != "svg_path");
    assert_eq!(
        preview_fields,
        property_names(&plan_schema),
        "preview and plan must take the same dials"
    );

    let description = preview
        .description
        .clone()
        .map(|d| d.into_owned())
        .unwrap_or_default();
    assert!(
        description.contains("without emitting"),
        "the preview's whole contract is that it changes nothing: {description}"
    );
    assert!(
        description.contains("0.15"),
        "the resolution trap applies here too — this one actually walks the grid: \
         {description}"
    );
}
