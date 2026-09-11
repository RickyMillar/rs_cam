//! Phase O item 4 — GUI Generate All runs the SAME rest-stock fixpoint ladder
//! the MCP `generate_all` tool runs.
//!
//! A/M11 landed the ladder behind `#[cfg(feature = "mcp")]`, so the GUI stayed
//! single-pass and told the operator to loop by hand. The multi-tool planner
//! emits a k-op coarse-to-fine chain from one action, which makes "loop it by
//! hand k times" untenable.
//!
//! What is asserted here is the shared decision, not the loop: the plan a
//! project produces (`plan_fixpoint`), and the GUI entry's two ends of it —
//! it arms the ladder when the operator has pinned a resolution, and it
//! REFUSES rather than guessing when they have not. A/M10's rule is that the
//! cell size is never chosen silently, because collision counts and
//! engagement both move with it.

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
use rs_cam_viz::controller::generate_all::plan_fixpoint;
use rs_cam_viz::state::toolpath::StockSource;
use rs_cam_viz::ui::AppEvent;

// ── the pure decision ──────────────────────────────────────────────────────

/// `k` enabled rest ops plus a pinned resolution = a ladder bounded by `k + 1`
/// rounds. A stock chain cannot be longer than the number of links in it.
#[test]
fn rest_ops_with_a_resolution_produce_a_bounded_looping_plan() {
    for k in 1..=4 {
        let rest_ops: Vec<usize> = (1..=k).collect();
        let plan = plan_fixpoint(true, &rest_ops, Some(0.15))
            .unwrap_or_else(|_| panic!("{k} rest ops + a resolution must plan a ladder"));
        assert!(plan.enabled, "k={k}");
        assert_eq!(plan.resolution_mm, Some(0.15), "k={k}");
        assert_eq!(plan.max_rounds, k + 1, "k={k}");
        assert_eq!(plan.round, 1, "k={k}");
        assert_eq!(plan.simulations, 0, "k={k}");
        assert!(!plan.awaiting_simulation, "k={k}");
    }
}

/// Nothing depends on simulated stock, so no simulation runs and no resolution
/// is needed — the pre-A/M11 single pass, unchanged.
#[test]
fn no_rest_ops_is_a_single_pass_and_needs_no_resolution() {
    let plan = plan_fixpoint(true, &[], None).expect("no rest ops needs no resolution");
    assert!(!plan.enabled);
    assert_eq!(plan.max_rounds, 1);
    assert!(plan.resolution_mm.is_none());
}

/// An explicit opt-out is honoured even with a chain present.
#[test]
fn fixpoint_off_is_a_single_pass_even_with_a_chain() {
    let plan = plan_fixpoint(false, &[1, 2, 3], None).expect("opting out never refuses");
    assert!(!plan.enabled);
    assert_eq!(plan.max_rounds, 1);
}

/// A/M10 — the refusal, and what it carries so each surface can render its own
/// remedy. Zero and NaN are refusals too, not "close enough to a cell size".
#[test]
fn a_chain_without_a_resolution_refuses_and_names_the_blocking_ops() {
    for supplied in [None, Some(0.0), Some(-0.1), Some(f64::NAN)] {
        let refusal = plan_fixpoint(true, &[2, 5], supplied)
            .err()
            .unwrap_or_else(|| panic!("{supplied:?} must not be accepted as a cell size"));
        assert_eq!(refusal.rest_op_indices, vec![2, 5], "{supplied:?}");
        assert!(!refusal.supplied_clause().is_empty(), "{supplied:?}");
    }
    assert!(
        plan_fixpoint(true, &[2], None)
            .err()
            .unwrap()
            .supplied_clause()
            .contains("not supplied"),
        "an absent resolution must read differently from an unusable one"
    );
}

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
    controller.state.session = ProjectSessionBuilder::new()
        .tool(ToolConfig::new_default(ToolId(1), ToolType::BallNose))
        .build();
    controller
        .state
        .session
        .add_toolpath(0, toolpath(0, StockSource::Fresh))
        .expect("first op");
    for i in 1..=rest_ops {
        controller
            .state
            .session
            .add_toolpath(0, toolpath(i, StockSource::FromRemainingStock))
            .expect("rest op");
    }
    controller
}

/// THE Phase O gate. With a chain present and a resolution pinned, the GUI's
/// Generate All arms the ladder — the same tracker the MCP path arms — instead
/// of firing a single pass and leaving the rest ops blocked.
#[test]
fn gui_generate_all_arms_the_ladder_when_a_resolution_is_pinned() {
    let mut controller = controller_with_chain(2);
    controller.state.simulation.auto_resolution = false;
    controller.state.simulation.resolution = 0.15;

    controller.handle_internal_event(AppEvent::GenerateAll);

    assert!(
        controller.awaiting_generate_all(),
        "a chain + a pinned resolution must start the fixpoint ladder"
    );
    assert!(
        controller.awaiting_deferred_completions() > 0,
        "the ladder owes a future frame, so the repaint driver must see it"
    );
    assert!(
        controller
            .active_notifications()
            .all(|n| !n.message.contains("resolution")),
        "nothing to refuse here"
    );
}

/// The other end of A/M10. `auto_resolution` is NOT a pinned cell size — it is
/// re-derived per simulation from whatever has generated so far, so it can
/// move between rounds of one ladder. The GUI refuses and says which control
/// to touch; it does not start a run whose verdicts nobody asked for.
#[test]
fn gui_generate_all_refuses_rather_than_guessing_a_resolution() {
    let mut controller = controller_with_chain(2);
    controller.state.simulation.auto_resolution = true;

    controller.handle_internal_event(AppEvent::GenerateAll);

    assert!(
        !controller.awaiting_generate_all(),
        "a refusal must not leave a ladder armed"
    );
    let refusal = controller
        .active_notifications()
        .find(|n| n.message.contains("resolution"))
        .map(|n| n.message.clone())
        .expect("the refusal has to reach the operator, not just the log");
    assert!(
        refusal.contains("Auto from tool size"),
        "the refusal must name the control that fixes it: {refusal}"
    );
    assert!(
        refusal.contains('2'),
        "the refusal must say how many ops force the ladder: {refusal}"
    );
}

/// A project with no rest ops keeps the pre-Phase-O behaviour exactly: every
/// config submitted once, no ladder state, nothing said to the operator.
#[test]
fn gui_generate_all_without_a_chain_stays_a_single_pass() {
    let mut controller = controller_with_chain(0);

    controller.handle_internal_event(AppEvent::GenerateAll);

    assert!(
        !controller.awaiting_generate_all(),
        "no rest op means no ladder"
    );
    assert_eq!(controller.awaiting_deferred_completions(), 0);
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
