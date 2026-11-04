//! G-GUIADD — `add_toolpath_via_gui` exists, and the GUI add path it
//! wraps behaves the way R0.3 says it does.
//!
//! Task F3.1 (`planning/ui_fix_2026-09-09/PLAN.md` §5):
//!
//! > Dispatches `AppEvent::AddToolpath` through `handle_add_toolpath`, so
//! > the GUI's tool/model binding and refusal path run. Returns the
//! > created index or the refusal text and whether a notification was
//! > pushed.
//!
//! §5's closing line asks for "one live parity test that
//! `add_toolpath_via_gui` on the flat-first terrain seed reproduces the
//! R0.3 contract". The MCP server is down, so nothing here is called over
//! the wire. This file pins the two halves that CAN be checked offline:
//!
//! 1. the registration and schema the LLM receives, off the real
//!    `ToolRouter` (as `mcp_authoring_surface.rs` does);
//! 2. the GUI add path itself, driven through the very event the tool
//!    dispatches — `AppEvent::AddToolpath` into
//!    `AppController::handle_internal_event` — on the flat-first terrain
//!    seed, asserting the three R0.3 §2.2 behaviours the tool exists to
//!    expose: the A1 Suggest refusal creates NOTHING and reports itself
//!    only as a toast; the binding is the first tool that the operation
//!    accepts, so a compatible tool second in the list is bound; and a
//!    permitted add creates one toolpath and pushes no warning.
//!
//! `RsCamApp` needs an `eframe::CreationContext`, so the dispatch arm
//! cannot be constructed in a test — the same limit F1.1 recorded. What
//! is driven here is the controller the arm calls into, through the same
//! public entry point.

#![cfg(feature = "mcp")]
#![expect(
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::session::ProjectSessionBuilder;
use rs_cam_viz::compute::{
    CollisionRequest, ComputeBackend, ComputeLane, ComputeMessage, ComputeRequest,
    GenerationControl, LaneSnapshot, SimulationRequest, ToolpathSubmitOutcome,
};
use rs_cam_viz::controller::{AppController, Severity};
use rs_cam_viz::mcp_server::EmbeddedCamServer;
use rs_cam_viz::ui::AppEvent;

// ── half 1: the registered surface ──────────────────────────────────────

struct ToolFacts {
    description: String,
    schema: serde_json::Value,
}

fn tool_facts(name: &str) -> ToolFacts {
    let router = EmbeddedCamServer::into_tool_router();
    let tool = router
        .list_all()
        .into_iter()
        .find(|t| t.name == name)
        .unwrap_or_else(|| panic!("the embedded server does not register `{name}`"));
    ToolFacts {
        description: tool.description.map(|d| d.into_owned()).unwrap_or_default(),
        schema: serde_json::Value::Object((*tool.input_schema).clone()),
    }
}

fn required(facts: &ToolFacts) -> Vec<String> {
    facts
        .schema
        .pointer("/required")
        .and_then(|r| r.as_array())
        .map(|r| {
            r.iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

/// The assertion that fails on the pre-fix tree: the route exists at all.
#[test]
fn add_toolpath_via_gui_is_registered() {
    let facts = tool_facts("add_toolpath_via_gui");
    assert!(
        !facts.description.is_empty(),
        "add_toolpath_via_gui must carry a description — it is the only \
         thing an agent reads before choosing between this and add_toolpath"
    );
}

#[test]
fn operation_type_is_required_and_setup_index_is_optional() {
    let facts = tool_facts("add_toolpath_via_gui");
    let req = required(&facts);
    assert!(
        req.contains(&"operation_type".to_owned()),
        "operation_type must be required — got {req:?}"
    );
    assert!(
        !req.contains(&"setup_index".to_owned()),
        "setup_index must be OPTIONAL: omitting it means \"add into the \
         current selection\", which is what the GUI does — got {req:?}"
    );
    assert!(
        facts
            .schema
            .pointer("/properties/setup_index")
            .is_some_and(|v| v.is_object()),
        "setup_index must still be declared: {}",
        facts.schema
    );
}

/// The reason this tool exists beside `add_toolpath`. An agent that cannot
/// tell them apart will reach for the wrong one, so the description must
/// say which behaviours differ (R0.3 §2.2 rows A1 and the binding table).
#[test]
fn the_description_distinguishes_it_from_add_toolpath() {
    let facts = tool_facts("add_toolpath_via_gui");
    for needle in ["add_toolpath", "first", "Scallop", "refus"] {
        assert!(
            facts.description.contains(needle),
            "add_toolpath_via_gui's description must mention `{needle}` — \
             the GUI path binds the FIRST tool and can refuse outright, and \
             an agent choosing between the two tools needs both facts: {}",
            facts.description
        );
    }
}

/// A refusal is reported only as a toast, so the reply has to name the
/// stack it read, and `get_notifications` has to exist to read it again.
#[test]
fn it_points_at_get_notifications() {
    let facts = tool_facts("add_toolpath_via_gui");
    assert!(
        facts.description.contains("get_notifications"),
        "the refusal is a toast; the description must name the tool that \
         reads them: {}",
        facts.description
    );
    // And that tool must be registered, or the pointer is a dead end.
    tool_facts("get_notifications");
}

// ── half 2: the GUI add path this tool dispatches into ──────────────────

/// Accepts every submission, returns nothing. Nothing here waits on a lane.
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

/// The flat-first terrain seed of R0.3 §5.2: tools `[Ø6 end mill, Ø3 ball
/// nose]` in that order. The order is the whole point — `handle_add_toolpath`
/// binds the first tool that the operation accepts.
fn flat_first_seed() -> AppController<SilentBackend> {
    let mut controller = AppController::with_backend(SilentBackend);
    controller.state.replace_session(
        ProjectSessionBuilder::new()
            .tool(ToolConfig::test_default(
                ToolId::from_raw(0),
                ToolType::EndMill,
            ))
            .tool(ToolConfig::test_default(
                ToolId::from_raw(1),
                ToolType::BallNose,
            ))
            .build(),
    );
    controller
}

/// A project whose only tool is a flat end mill: Scallop accepts no tool.
fn flat_only_seed() -> AppController<SilentBackend> {
    let mut controller = AppController::with_backend(SilentBackend);
    controller.state.replace_session(
        ProjectSessionBuilder::new()
            .tool(ToolConfig::test_default(
                ToolId::from_raw(0),
                ToolType::EndMill,
            ))
            .build(),
    );
    controller
}

fn toasts(controller: &AppController<SilentBackend>) -> Vec<(String, Severity)> {
    controller
        .notifications()
        .iter()
        .map(|n| (n.message.clone(), n.severity))
        .collect()
}

/// R0.3 §2.2 row A1, as R2 (operator ruling 2026-10-23) left it: a Scallop
/// with a flat end mill as the only tool is no longer refused for the tool
/// (the op would be made with no tool). This seed has no model, so the add
/// refuses for the geometry: nothing is created, and the ONLY report is a
/// Warning toast. That last clause is why F3.1 and F3.5 are one task —
/// without the stack there is nothing for the tool to return.
#[test]
fn a_refused_add_creates_nothing_and_reports_itself_only_as_a_toast() {
    let mut controller = flat_only_seed();
    assert!(controller.notifications().is_empty());

    controller.handle_internal_event(AppEvent::AddToolpath(OperationType::Scallop));

    assert!(
        controller.state.session().toolpath_configs().is_empty(),
        "the add refused, so NOTHING may be created: {:?}",
        controller
            .state
            .session()
            .toolpath_configs()
            .iter()
            .map(|tc| tc.name.clone())
            .collect::<Vec<_>>()
    );
    let shown = toasts(&controller);
    assert_eq!(shown.len(), 1, "exactly one toast: {shown:?}");
    assert_eq!(shown[0].1, Severity::Warning, "{shown:?}");
    assert_eq!(
        shown[0].0, "Cannot add Scallop Finish toolpath: import geometry first",
        "the refusal text is the only record of what happened: {shown:?}"
    );
}

/// The binding rule of R0.3 §4.3 step 2, now landed: a compatible ball
/// nose second in the list passes the tool rule, and the end mill before
/// it is skipped without a toast. The seed has no model, so the add then
/// stops at the geometry rule; that refusal proves the tool rule passed.
/// `a_new_op_takes_the_first_tool_it_accepts.rs` covers a completed add.
#[test]
fn the_gui_binds_the_first_compatible_tool_when_it_is_second() {
    let mut controller = flat_first_seed();
    let kinds: Vec<ToolType> = controller
        .state
        .session()
        .tools()
        .iter()
        .map(|t| t.tool_type)
        .collect();
    assert_eq!(
        kinds,
        vec![ToolType::EndMill, ToolType::BallNose],
        "seed order is load-bearing"
    );

    controller.handle_internal_event(AppEvent::AddToolpath(OperationType::Scallop));

    // A "no feeds recipe" notice can come first: it says the ball nose
    // fits. The last toast is the geometry rule, and no toast names a
    // refused tool.
    let shown = toasts(&controller);
    assert!(
        shown
            .last()
            .is_some_and(|(m, _)| m.ends_with("import geometry first")),
        "the ball nose fits, so the geometry rule refuses last: {shown:?}"
    );
    assert!(
        shown.iter().all(|(m, _)| !m.contains("this tool is")),
        "no tool refusal: {shown:?}"
    );
}

/// The other arm: an operation the first tool satisfies creates exactly one
/// toolpath, bound to that tool, with no warning. Face is stock-based, so
/// this reaches the add without needing an imported model (R0.3 §2.2 row
/// A4 guards only non-stock ops).
#[test]
fn a_permitted_add_creates_one_toolpath_bound_to_the_first_tool() {
    let mut controller = flat_first_seed();
    let first_tool_id = controller.state.session().tools()[0].id.raw();

    controller.handle_internal_event(AppEvent::AddToolpath(OperationType::Face));

    let configs = controller.state.session().toolpath_configs();
    assert_eq!(configs.len(), 1, "exactly one toolpath");
    assert_eq!(
        configs[0].tool_id,
        Some(rs_cam_core::ids::ToolId::from_raw(first_tool_id)),
        "bound to tools().first(), not to a chosen tool"
    );
    let warnings: Vec<_> = toasts(&controller)
        .into_iter()
        .filter(|(_, s)| *s != Severity::Info)
        .collect();
    assert!(
        warnings.is_empty(),
        "a permitted add warns about nothing: {warnings:?}"
    );
}

/// R0.3 §2.2 row A3, as R2 (operator ruling 2026-10-23) left it: with no
/// tools at all the op is still made, with no tool. W2b: no toast repeats
/// that STATE; the core no-tool state holds it (the inspector Tool row
/// and MCP `run_state` read it).
#[test]
fn an_add_with_no_tools_makes_the_op_with_no_tool_r2() {
    let mut controller = AppController::with_backend(SilentBackend);
    assert!(controller.state.session().tools().is_empty());

    controller.handle_internal_event(AppEvent::AddToolpath(OperationType::Face));

    let configs = controller.state.session().toolpath_configs();
    assert_eq!(configs.len(), 1, "the op is made");
    assert_eq!(configs[0].tool_id, None);
    let shown = toasts(&controller);
    assert!(shown.is_empty(), "a state takes no toast: {shown:?}");
    assert_eq!(
        controller.state.session().no_tool_state(0).as_deref(),
        Some("No tool fits: needs a tool"),
        "the state is in core"
    );
}

/// Two adds, two toasts, in order — the property `add_toolpath_via_gui`
/// relies on when it reads the tail of the stack to find what THIS call
/// pushed. If the stack were cleared or reordered, the reply would
/// attribute an older toast to a newer request.
#[test]
fn the_stack_only_grows_so_the_tail_belongs_to_the_latest_add() {
    let mut controller = flat_only_seed();

    controller.handle_internal_event(AppEvent::AddToolpath(OperationType::Scallop));
    let after_first = controller.notifications().len();
    assert_eq!(after_first, 1);

    controller.handle_internal_event(AppEvent::AddToolpath(OperationType::Face));
    // Face is permitted: it pushes nothing, and it must not disturb the
    // refusal already on the stack.
    assert_eq!(
        controller.notifications().len(),
        after_first,
        "a silent add must neither clear nor duplicate: {:?}",
        toasts(&controller)
    );
    assert!(
        controller.notifications()[0]
            .message
            .starts_with("Cannot add "),
        "the earlier refusal is still the oldest entry"
    );
}
