//! G-MCPTOAST sentries — an MCP toast reports the handler's OUTCOME, not
//! the request (UX-R03-003, `planning/ui_review_2026-09-09/results/R03/REPORT.md`).
//!
//! Live reproduction, 2026-09-09: an agent asked `add_toolpath` for a
//! Scallop on a flat end mill. The Suggest door refused
//! (`feeds::validate_tool_for_operation`), the reply carried `ok: false`,
//! `list_toolpaths` showed no such op — and the GUI still displayed
//! "MCP: Added toolpath 'Detail finish (flat tool attempt)'". The
//! dispatch in `app/mcp.rs` pushed the past-tense toast BEFORE it called
//! the handler, and no arm corrected it on failure. The R08 source track
//! (`results/W03/support/r08_source_track.md` §8) counted sixteen such
//! pre-announced toasts, nine of them in the past tense.
//!
//! Two layers are pinned here:
//!
//! - The dispatch SOURCE: every synchronous mutation arm pushes its toast
//!   through `push_mcp_outcome` after the handler call, and the only bare
//!   `push_notification` calls left are the three progress toasts for
//!   asynchronous compute ("Generating…", "Running simulation…"), which
//!   are truthful in the progressive tense. `RsCamApp` needs an
//!   `eframe::CreationContext`, so the arm itself cannot be driven from a
//!   test; the source check is what stands in for it.
//! - The outcome-to-toast contract, driven with the REAL payload shapes the
//!   handlers emit (`mutation_error_json`, `MutationResult`, a bare
//!   `{"error": …}`, and the controller's own `open_job_from_path` error)
//!   against a real `AppController`.

#![cfg(feature = "mcp")]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

/// The dispatch and the describe step, read as text.
const MCP_SRC: &str = include_str!("../src/app/mcp.rs");
const COMMANDS_SRC: &str = include_str!("../src/app/mcp/commands.rs");

/// The toasts the command rows push, as literals.
///
/// WP4 moved these out of per-row dispatch arms and into
/// `RsCamApp::core_toast_for` (`app/mcp/commands.rs`), which RETURNS the
/// text rather than pushing it. The pair table this arm used to carry —
/// (toast literal, `self.mcp_x(` handler call) — cannot survive that
/// move: the handlers it named are gone, and the literal no longer sits
/// in the same function as the mutation.
///
/// The rule it enforced survives in three halves, asserted below. The
/// text is read off the REQUEST, so a request the CONVERSION refuses
/// still reports its refusal. Nothing in `commands.rs` pushes a toast of
/// its own. The one `Core` arm pushes it through `push_mcp_outcome`,
/// after `apply`, classified against the reply.
const REQUEST_TOASTS: &[&str] = &[
    "\"MCP: Adding setup\"",
    "\"MCP: Set setup {setup_index} face to '{face_up}'\"",
    "\"MCP: Set setup {setup_index} Z rotation to '{z_rotation}'\"",
    "\"MCP: Moving toolpath {toolpath_index} to setup {target_setup_index}\"",
    "\"MCP: Importing '{name}'\"",
    "\"MCP: Saved project\"",
    "\"MCP: Set {param} = {value_str} on '{tp_name}'\"",
    "\"MCP: Set {param} = {value_str} on '{tool_name}'\"",
    "\"MCP: Bound tool {tool_id} to '{tp_name}'\"",
    "\"MCP: Bound model {model_id} to '{tp_name}'\"",
    "\"MCP: Added toolpath '{display_name}'\"",
    "\"MCP: Removed toolpath {index}\"",
    "\"MCP: Added tool '{name}'\"",
    "\"MCP: Imported tool from library '{catalog}' #{index}\"",
];

/// The one arm that still pairs a toast with a handler call: the project
/// load, which WP4 left hand-written.
const OUTCOME_TOASTS: &[(&str, &str)] = &[("\"MCP: Loaded '{name}'\"", "self.mcp_load_project(")];

/// The asynchronous compute toasts. They announce work that a lane picks
/// up later, so the progressive tense is the truth and they stay as bare
/// `push_notification` calls.
const PROGRESS_TOASTS: &[&str] = &[
    "\"MCP: Generating toolpath '{tp_name}'...\"",
    "\"MCP: Generating all toolpaths...\"",
    "\"MCP: Running simulation...\"",
];

/// One dispatch arm, generously bounded. The arms run 10–45 lines; this
/// keeps a match from leaking into the next arm's handler call.
const ARM_SPAN_CHARS: usize = 2_500;

/// Every command-row toast is a value `core_toast_for` returns, and
/// nothing in `commands.rs` pushes a toast of its own.
///
/// Pre-fix (2026-09-09), all thirteen literals sat inside a bare
/// `push_notification(` call issued BEFORE the handler ran.
#[test]
fn every_command_row_toast_is_returned_not_pushed() {
    let mut missing = Vec::new();
    for literal in REQUEST_TOASTS {
        if !COMMANDS_SRC.contains(literal) {
            missing.push((*literal).to_owned());
        }
    }
    assert!(
        missing.is_empty(),
        "these MCP toast literals no longer appear in app/mcp/commands.rs:\n  {}",
        missing.join("\n  ")
    );
    for pushed in ["push_notification(", "push_mcp_outcome("] {
        assert!(
            !COMMANDS_SRC.contains(pushed),
            "app/mcp/commands.rs calls {pushed}: it must RETURN its toast \
             so the one dispatch arm pushes it from the mutation's \
             outcome, never announce one itself"
        );
    }
    // The text comes from the REQUEST. A toast built after the mutation
    // is silent on every conversion refusal — an unknown face, an
    // unsupported file extension, the Suggest door declining a Scallop
    // on a flat end mill — which is UX-R03-003 in the other direction.
    let at = COMMANDS_SRC
        .find("fn core_toast_for(&self, request: &CoreRequest)")
        .expect("core_toast_for reads the toast off the request");
    for literal in REQUEST_TOASTS {
        let found = COMMANDS_SRC
            .find(literal)
            .unwrap_or_else(|| panic!("{literal} is not in app/mcp/commands.rs"));
        assert!(
            found > at,
            "{literal} sits outside core_toast_for; a toast built after the \
             mutation cannot report a refusal the conversion made"
        );
    }
}

/// The one `Core` arm pushes the describe step's toast AFTER the
/// mutation, through `push_mcp_outcome`.
#[test]
fn the_command_arm_pushes_its_toast_after_the_mutation() {
    let at = MCP_SRC
        .find("McpRequestKind::Core(request) => {")
        .expect("app/mcp.rs holds one McpRequestKind::Core arm");
    let arm = &MCP_SRC[at..MCP_SRC.len().min(at + ARM_SPAN_CHARS)];
    let apply_at = arm
        .find(".session.apply(command)")
        .expect("the Core arm applies the command");
    let push_at = arm
        .find("push_mcp_outcome(")
        .expect("the Core arm pushes the toast through push_mcp_outcome");
    assert!(
        apply_at < push_at,
        "the toast must be pushed after the mutation, not before it"
    );
    assert!(
        arm.find("self.core_toast_for(&request)")
            .is_some_and(|read_at| read_at < apply_at),
        "the Core arm must read the toast text off the request BEFORE the \
         conversion, or a refused conversion pushes nothing"
    );
    let before_push = &arm[..push_at];
    let last_push = before_push.rfind("push_");
    assert!(
        last_push.is_none_or(|offset| before_push[offset..].starts_with("push_mcp_outcome(")),
        "the Core arm pushes a toast by some other door before the outcome door"
    );
}

/// The remaining hand-written arm keeps the pre-WP4 rule: its handler
/// call sits before its toast literal, and the toast goes through
/// `push_mcp_outcome`.
#[test]
fn every_synchronous_toast_is_pushed_from_the_handler_outcome() {
    let mut pre_announced = Vec::new();
    for (literal, handler_call) in OUTCOME_TOASTS {
        let toast_at = MCP_SRC
            .find(literal)
            .unwrap_or_else(|| panic!("toast literal {literal} no longer appears in app/mcp.rs"));
        let handler_at = MCP_SRC.find(handler_call).unwrap_or_else(|| {
            panic!("dispatch call {handler_call} no longer appears in app/mcp.rs")
        });
        let arm_start = toast_at.saturating_sub(ARM_SPAN_CHARS);
        let arm = &MCP_SRC[arm_start..toast_at];
        let call_before_toast = handler_at < toast_at && toast_at - handler_at < ARM_SPAN_CHARS;
        let last_push = arm.rfind("push_");
        let via_outcome = last_push.is_some_and(|at| arm[at..].starts_with("push_mcp_outcome("));
        if !call_before_toast || !via_outcome {
            pre_announced.push(format!(
                "{literal}: handler-before-toast={call_before_toast}, via push_mcp_outcome={via_outcome}"
            ));
        }
    }
    assert!(
        pre_announced.is_empty(),
        "these MCP toasts are pushed before, or independently of, their handler's result:\n  {}",
        pre_announced.join("\n  ")
    );
}

/// The only bare `push_notification(` calls left in the dispatch are the
/// three progress toasts for asynchronous compute.
#[test]
fn only_the_progress_toasts_bypass_the_outcome_door() {
    let bare = MCP_SRC.matches("push_notification(").count();
    assert_eq!(
        bare,
        PROGRESS_TOASTS.len(),
        "app/mcp.rs has {bare} bare push_notification( calls; only the {} progress toasts \
         ({PROGRESS_TOASTS:?}) may announce before their work completes",
        PROGRESS_TOASTS.len()
    );
    for literal in PROGRESS_TOASTS {
        assert!(
            MCP_SRC.contains(literal),
            "progress toast {literal} no longer appears in app/mcp.rs"
        );
    }
}

// ── the outcome-to-toast contract, on real payloads ─────────────────────────

use std::path::Path;

use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::session::ProjectSessionBuilder;
use rs_cam_viz::compute::{
    CollisionRequest, ComputeBackend, ComputeLane, ComputeMessage, ComputeRequest,
    GenerationControl, LaneSnapshot, OptimizeRequest, SimulationRequest, ToolpathSubmitOutcome,
};
use rs_cam_viz::controller::{AppController, Severity};
use rs_cam_viz::mcp_bridge::{McpOutcome, MutationResult, mutation_error_json};

/// A compute backend that accepts every submission and returns nothing.
/// Nothing here waits on a lane.
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

/// A controller holding one tool of `tool_type`, as the live reproduction
/// did (tool 0, Ø6.35 2-flute flat end mill).
fn controller_with_tool(tool_type: ToolType) -> AppController<SilentBackend> {
    let mut controller = AppController::with_backend(SilentBackend);
    controller.state.session = ProjectSessionBuilder::new()
        .tool(ToolConfig::new_default(ToolId(1), tool_type))
        .build();
    controller
}

/// The Suggest door `mcp_add_toolpath` runs at creation, with the same
/// inputs the handler builds — its `Err` is the refusal the handler wraps
/// as `Cannot add toolpath: …` and returns through `mutation_error_json`.
fn suggest_for(
    controller: &AppController<SilentBackend>,
    op_type: OperationType,
) -> Result<(), String> {
    let session = &controller.state.session;
    let tool = &session.tools()[0];
    let stock_ctx = rs_cam_core::feeds::suggest::StockContext::from_stock_bbox(
        session.stock_bbox(),
        session.stock_config().padding,
    );
    rs_cam_core::feeds::suggest::suggest_params(rs_cam_core::feeds::suggest::SuggestParamsInput {
        op_type,
        tool,
        machine: session.machine(),
        material: &session.stock_config().material,
        workholding: session.stock_config().workholding_rigidity,
        lut: rs_cam_core::feeds::embedded_vendor_lut(),
        stock_ctx: &stock_ctx,
        spindle_strategy: rs_cam_core::feeds::SpindleStrategy::default(),
        context: rs_cam_core::feeds::suggest::SuggestContext::default(),
    })
    .map(|_| ())
    .map_err(|e| e.to_string())
}

fn toasts(controller: &AppController<SilentBackend>) -> Vec<(String, Severity)> {
    controller
        .active_notifications()
        .map(|n| (n.message.clone(), n.severity))
        .collect()
}

/// (a) The live reproduction. Scallop on a flat end mill is refused by the
/// Suggest door; the screen must show the refusal, and never "Added".
#[test]
fn a_refused_add_toolpath_shows_the_refusal_and_never_added() {
    let mut controller = controller_with_tool(ToolType::EndMill);
    let refusal = suggest_for(&controller, OperationType::Scallop)
        .expect_err("Scallop on a flat end mill must be refused by the Suggest door");
    // Feeds matrix R1 (2026-09-23): the refusal names the operation and the
    // kinds its registry row allows, in words.
    assert!(
        refusal.contains("Scallop Finish needs a ball nose or tapered ball nose tool"),
        "unexpected refusal text: {refusal}"
    );

    // The handler's reply, byte-for-byte the shape `mcp_add_toolpath` returns.
    let reply = mutation_error_json(&format!("Cannot add toolpath: {refusal}"), None);
    let outcome = McpOutcome::from_json_response(&reply);
    assert!(matches!(outcome, McpOutcome::Refused(_)), "{outcome:?}");

    controller.push_mcp_outcome(
        "MCP: Added toolpath 'Detail finish (flat tool attempt)'".to_owned(),
        &outcome,
    );

    let shown = toasts(&controller);
    assert!(
        shown.iter().all(|(m, _)| !m.contains("Added")),
        "a refused add_toolpath must not announce 'Added': {shown:?}"
    );
    let warnings: Vec<_> = shown
        .iter()
        .filter(|(_, s)| *s == Severity::Warning)
        .collect();
    assert_eq!(warnings.len(), 1, "exactly one Warning toast: {shown:?}");
    assert!(
        warnings[0].0.contains("Cannot add toolpath")
            && warnings[0]
                .0
                .contains("needs a ball nose or tapered ball nose tool"),
        "the Warning must carry the handler's refusal text: {}",
        warnings[0].0
    );
    assert_eq!(shown.len(), 1, "no second toast of any kind: {shown:?}");
}

/// (b) The same request on a ball nose passes the door, and the success
/// reply produces the unchanged "Added" toast at Info.
#[test]
fn a_successful_add_toolpath_shows_added_at_info() {
    let mut controller = controller_with_tool(ToolType::BallNose);
    suggest_for(&controller, OperationType::Scallop)
        .expect("Scallop on a ball nose passes the Suggest door");

    // `mcp_mutation_result`'s document, as the handler serialises it.
    let reply = serde_json::to_string(&MutationResult {
        ok: true,
        summary: "Added toolpath 0 (Scallop).".to_owned(),
        applied: serde_json::json!({ "index": 0, "operation": "Scallop" }),
        stale_toolpaths: vec![0],
        warnings: Vec::new(),
        gui_banners: Vec::new(),
        diagnostic_delta: Vec::new(),
    })
    .unwrap();
    let outcome = McpOutcome::from_json_response(&reply);
    assert_eq!(outcome, McpOutcome::Succeeded);

    controller.push_mcp_outcome("MCP: Added toolpath 'Detail finish'".to_owned(), &outcome);
    assert_eq!(
        toasts(&controller),
        vec![(
            "MCP: Added toolpath 'Detail finish'".to_owned(),
            Severity::Info
        )]
    );
}

/// (c) A `load_project` on a path that does not exist fails in the
/// controller; the toast must not say "Loaded".
#[test]
fn a_failed_load_project_never_says_loaded() {
    let mut controller = controller_with_tool(ToolType::EndMill);
    let path = Path::new("/nonexistent/g_mcptoast/does_not_exist.toml");
    let loaded = controller.open_job_from_path(path);
    assert!(
        loaded.is_err(),
        "a nonexistent project path must fail to load"
    );

    let outcome = McpOutcome::from_result(&loaded);
    controller.push_mcp_outcome("MCP: Loaded 'does_not_exist'".to_owned(), &outcome);

    let shown = toasts(&controller);
    assert!(
        shown.iter().all(|(m, _)| !m.contains("Loaded")),
        "a failed load must not announce 'Loaded': {shown:?}"
    );
    assert_eq!(shown.len(), 1, "{shown:?}");
    assert_eq!(shown[0].1, Severity::Warning, "{shown:?}");
    assert!(shown[0].0.starts_with("MCP: "), "{shown:?}");
}

/// The import and library handlers refuse with a bare `{"error": …}`
/// document rather than `mutation_error_json`; that shape is a refusal too,
/// and its text reaches the toast.
#[test]
fn a_bare_error_reply_is_a_refusal() {
    let reply = serde_json::json!({
        "error": "Unsupported file format '.obj'. Use .stl, .dxf, .svg, .step, or .stp"
    })
    .to_string();
    let outcome = McpOutcome::from_json_response(&reply);
    let (message, severity) = outcome.notification("MCP: Importing 'part.obj'".to_owned());
    assert_eq!(severity, Severity::Warning);
    assert!(
        message.contains("Unsupported file format '.obj'"),
        "{message}"
    );
    assert!(!message.contains("Importing"), "{message}");
}

/// A success reply that carries advisory warnings (`ok: true`, non-empty
/// `warnings`) is still a success: the gate advisories belong to the reply,
/// not to the toast. Only `ok: false` or `error` refuses.
#[test]
fn advisory_warnings_on_a_success_reply_do_not_refuse() {
    let reply = serde_json::to_string(&MutationResult {
        ok: true,
        summary: "Set stepover_mm = 3.0".to_owned(),
        applied: serde_json::json!({}),
        stale_toolpaths: vec![1],
        warnings: vec![rs_cam_viz::mcp_bridge::MutationWarning {
            level: "caution".to_owned(),
            field: None,
            message: "chipload above band".to_owned(),
            recommendation: None,
        }],
        gui_banners: Vec::new(),
        diagnostic_delta: Vec::new(),
    })
    .unwrap();
    assert_eq!(
        McpOutcome::from_json_response(&reply),
        McpOutcome::Succeeded
    );
}
