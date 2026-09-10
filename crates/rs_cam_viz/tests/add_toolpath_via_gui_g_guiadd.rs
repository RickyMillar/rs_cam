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
//!    only as a toast; the binding is `tools().first()` even when a
//!    compatible tool sits second; and a permitted add creates one
//!    toolpath and pushes no warning.
//!
//! `RsCamApp` needs an `eframe::CreationContext`, so the dispatch arm
//! cannot be constructed in a test — the same limit F1.1 recorded. What
//! is driven here is the controller the arm calls into, through the same
//! public entry point.

#![cfg(feature = "mcp")]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_viz::compute::{
    CollisionRequest, ComputeBackend, ComputeLane, ComputeMessage, ComputeRequest,
    GenerationControl, LaneSnapshot, OptimizeRequest, SimulationRequest, ToolpathSubmitOutcome,
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

/// The flat-first terrain seed of R0.3 §5.2: tools `[Ø6 end mill, Ø3 ball
/// nose]` in that order. The order is the whole point — `handle_add_toolpath`
/// binds `tools().first()` and never looks for a compatible one.
fn flat_first_seed() -> AppController<SilentBackend> {
    let mut controller = AppController::with_backend(SilentBackend);
    let tools = controller.state.session.tools_mut();
    tools.push(ToolConfig::new_default(ToolId(0), ToolType::EndMill));
    tools.push(ToolConfig::new_default(ToolId(1), ToolType::BallNose));
    controller
}

fn toasts(controller: &AppController<SilentBackend>) -> Vec<(String, Severity)> {
    controller
        .notifications()
        .iter()
        .map(|n| (n.message.clone(), n.severity))
        .collect()
}

/// R0.3 §2.2 row A1, the standing example. Scallop with a flat first tool
/// is refused by the add-time Suggest door: nothing is created, and the
/// ONLY report is a Warning toast. That last clause is why F3.1 and F3.5
/// are one task — without the stack there is nothing for the tool to
/// return.
#[test]
fn a_refused_add_creates_nothing_and_reports_itself_only_as_a_toast() {
    let mut controller = flat_first_seed();
    assert!(controller.notifications().is_empty());

    controller.handle_internal_event(AppEvent::AddToolpath(OperationType::Scallop));

    assert!(
        controller.state.session.toolpath_configs().is_empty(),
        "the Suggest door refused, so NOTHING may be created: {:?}",
        controller
            .state
            .session
            .toolpath_configs()
            .iter()
            .map(|tc| tc.name.clone())
            .collect::<Vec<_>>()
    );
    let shown = toasts(&controller);
    assert_eq!(shown.len(), 1, "exactly one toast: {shown:?}");
    assert_eq!(shown[0].1, Severity::Warning, "{shown:?}");
    assert!(
        shown[0].0.starts_with("Cannot add toolpath: "),
        "the refusal text is the only record of what happened: {shown:?}"
    );
}

/// The binding rule R0.3 §4.3 proposes to CHANGE, pinned as it stands
/// today so F4.2 can show the change. A compatible ball nose is present
/// and second; the GUI binds the end mill anyway and refuses. This is the
/// live parity case §5's closing line asks for.
#[test]
fn the_gui_binds_the_first_tool_even_when_a_compatible_one_is_second() {
    let mut controller = flat_first_seed();
    let kinds: Vec<ToolType> = controller
        .state
        .session
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

    assert!(
        controller.state.session.toolpath_configs().is_empty(),
        "R0.3 §4.3 step 2 (\"first tool that accepts_tool\") is NOT what \
         the code does today; if this now creates a toolpath the binding \
         rule has landed and F4.2's contract test owns this case"
    );
}

/// The other arm: an operation the first tool satisfies creates exactly one
/// toolpath, bound to that tool, with no warning. Face is stock-based, so
/// this reaches the add without needing an imported model (R0.3 §2.2 row
/// A4 guards only non-stock ops).
#[test]
fn a_permitted_add_creates_one_toolpath_bound_to_the_first_tool() {
    let mut controller = flat_first_seed();
    let first_tool_id = controller.state.session.tools()[0].id.0;

    controller.handle_internal_event(AppEvent::AddToolpath(OperationType::Face));

    let configs = controller.state.session.toolpath_configs();
    assert_eq!(configs.len(), 1, "exactly one toolpath");
    assert_eq!(
        configs[0].tool_id, first_tool_id,
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

/// R0.3 §2.2 row A3. With no tools at all there is nothing to bind, and
/// the refusal names that rather than a shape mismatch.
#[test]
fn an_add_with_no_tools_refuses_with_the_no_tools_text() {
    let mut controller = AppController::with_backend(SilentBackend);
    assert!(controller.state.session.tools().is_empty());

    controller.handle_internal_event(AppEvent::AddToolpath(OperationType::Face));

    assert!(controller.state.session.toolpath_configs().is_empty());
    let shown = toasts(&controller);
    assert_eq!(shown.len(), 1, "{shown:?}");
    assert_eq!(shown[0].0, "Cannot add toolpath: no tools defined");
    assert_eq!(shown[0].1, Severity::Warning);
}

/// Two adds, two toasts, in order — the property `add_toolpath_via_gui`
/// relies on when it reads the tail of the stack to find what THIS call
/// pushed. If the stack were cleared or reordered, the reply would
/// attribute an older toast to a newer request.
#[test]
fn the_stack_only_grows_so_the_tail_belongs_to_the_latest_add() {
    let mut controller = flat_first_seed();

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
            .starts_with("Cannot add toolpath: "),
        "the earlier refusal is still the oldest entry"
    );
}
