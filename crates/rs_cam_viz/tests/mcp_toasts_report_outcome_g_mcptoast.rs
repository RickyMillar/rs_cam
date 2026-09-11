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

/// The dispatch under test, read as text.
const MCP_SRC: &str = include_str!("../src/app/mcp.rs");

/// The twelve synchronous arms the review named, as (toast literal, the
/// handler call that must come BEFORE it in the same arm).
const OUTCOME_TOASTS: &[(&str, &str)] = &[
    ("\"MCP: Adding setup\"", "self.mcp_add_setup("),
    (
        "\"MCP: Set setup {setup_index} face to",
        "self.mcp_set_setup_face(",
    ),
    (
        "\"MCP: Set setup {setup_index} Z rotation to",
        "self.mcp_set_setup_rotation(",
    ),
    (
        "\"MCP: Moving toolpath {toolpath_index} to setup",
        "self.mcp_move_toolpath_to_setup(",
    ),
    ("\"MCP: Importing '{name}'\"", "self.mcp_import_model("),
    ("\"MCP: Loaded '{name}'\"", "self.mcp_load_project("),
    ("\"MCP: Saved project\"", "self.mcp_save_project("),
    (
        "\"MCP: Set {param} = {value_str} on '{tp_name}'\"",
        "self.mcp_set_toolpath_param(",
    ),
    (
        "\"MCP: Set {param} = {value_str} on '{tool_name}'\"",
        "self.mcp_set_tool_param(",
    ),
    (
        "\"MCP: Added toolpath '{display_name}'\"",
        "self.mcp_add_toolpath(",
    ),
    (
        "\"MCP: Removed toolpath {index}\"",
        "self.mcp_remove_toolpath(",
    ),
    ("\"MCP: Added tool '{}'\"", "self.mcp_add_tool("),
    (
        "\"MCP: Imported tool from library '{catalog}' #{index}\"",
        "self.mcp_add_tool_from_library(",
    ),
];

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

/// Every synchronous mutation toast is pushed from the handler's result,
/// AFTER the handler ran. Pre-fix, all thirteen literals sat inside a bare
/// `push_notification(` call issued before the handler.
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
    assert!(
        refusal.contains("scallop requires curved tip"),
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
            && warnings[0].0.contains("scallop requires curved tip"),
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
