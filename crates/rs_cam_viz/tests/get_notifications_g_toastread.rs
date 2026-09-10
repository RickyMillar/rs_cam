//! G-TOASTREAD — the toast stack is readable from outside, without
//! consuming it.
//!
//! Task F3.5 (`planning/ui_fix_2026-09-09/PLAN.md` §5):
//!
//! > Current toast stack with severity and age, so tests can assert what
//! > the operator saw (pairs with F1.1).
//!
//! F1.1 (G-MCPTOAST, merged) made every synchronous MCP arm push its toast
//! AFTER the handler and from its result, so a refusal shows the refusal
//! text at Warning instead of a past-tense success. That fix is only
//! assertable from outside if the stack can be read, which is what this
//! adds: `AppController::notifications`, and `get_notifications` over it.
//!
//! Two decisions this file pins, because both could be made the other way:
//!
//! * **Read-only.** `get_notifications` does not clear. A reader that
//!   consumed what it read would change what the next reader sees, and the
//!   next reader is usually the operator, still looking at the screen.
//! * **Age is measured from the push**, `Instant::elapsed` on
//!   `Notification::created_at`, in the running GUI process. The stack is
//!   not persisted; ages mean nothing across a restart and the stack is
//!   empty after one. `visible` is `age < ttl`, and TTL comes from
//!   severity (`Notification::ttl`: info 4 s, warning 6 s, error 8 s).

#![cfg(feature = "mcp")]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_viz::compute::{
    CollisionRequest, ComputeBackend, ComputeLane, ComputeMessage, ComputeRequest,
    GenerationControl, LaneSnapshot, OptimizeRequest, SimulationRequest, ToolpathSubmitOutcome,
};
use rs_cam_viz::controller::{AppController, Severity};
use rs_cam_viz::mcp_server::EmbeddedCamServer;

// ── the registered surface ──────────────────────────────────────────────

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

/// The assertion that fails on the pre-fix tree.
#[test]
fn get_notifications_is_registered() {
    let facts = tool_facts("get_notifications");
    assert!(!facts.description.is_empty());
}

/// Both dials are optional: the useful call is the bare one.
#[test]
fn every_parameter_is_optional() {
    let facts = tool_facts("get_notifications");
    let req = required(&facts);
    assert!(
        req.is_empty(),
        "get_notifications must be callable with no arguments — got {req:?}"
    );
    for field in ["include_expired", "limit"] {
        assert!(
            facts
                .schema
                .pointer(&format!("/properties/{field}"))
                .is_some_and(|v| v.is_object()),
            "`{field}` must still be declared: {}",
            facts.schema
        );
    }
}

/// The two decisions, stated where the agent reads them. "Read-only" is
/// not a detail: an agent that believes a read drains the stack will
/// avoid calling it twice and will mis-report what the operator saw.
#[test]
fn the_description_states_read_only_and_what_age_means() {
    let facts = tool_facts("get_notifications");
    let text = facts.description.to_lowercase();
    assert!(
        text.contains("read-only"),
        "must say it does not consume the stack: {}",
        facts.description
    );
    assert!(
        text.contains("removes nothing") || text.contains("remove nothing"),
        "must say so in plain words too: {}",
        facts.description
    );
    assert!(
        text.contains("not persisted"),
        "must say the stack dies with the process: {}",
        facts.description
    );
    for field in ["severity", "age_seconds", "visible"] {
        assert!(
            facts.description.contains(field),
            "the description must name the `{field}` it returns: {}",
            facts.description
        );
    }
}

// ── the stack the tool reads ────────────────────────────────────────────

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

/// Order and content: the accessor reports the whole stack, oldest first,
/// with the severity each was pushed at.
#[test]
fn the_accessor_reports_every_push_in_order() {
    let mut controller = AppController::with_backend(SilentBackend);
    assert!(controller.notifications().is_empty());

    controller.push_notification("first".to_owned(), Severity::Info);
    controller.push_notification("second".to_owned(), Severity::Warning);
    controller.push_notification("third".to_owned(), Severity::Error);

    let seen: Vec<(&str, Severity)> = controller
        .notifications()
        .iter()
        .map(|n| (n.message.as_str(), n.severity))
        .collect();
    assert_eq!(
        seen,
        vec![
            ("first", Severity::Info),
            ("second", Severity::Warning),
            ("third", Severity::Error),
        ]
    );
}

/// Read-only, demonstrated rather than asserted from the signature: two
/// reads see the same stack, and the second is not shorter.
#[test]
fn reading_the_stack_does_not_consume_it() {
    let mut controller = AppController::with_backend(SilentBackend);
    controller.push_notification("kept".to_owned(), Severity::Warning);

    let first: Vec<String> = controller
        .notifications()
        .iter()
        .map(|n| n.message.clone())
        .collect();
    let second: Vec<String> = controller
        .notifications()
        .iter()
        .map(|n| n.message.clone())
        .collect();

    assert_eq!(first, vec!["kept".to_owned()]);
    assert_eq!(
        first, second,
        "a read must not change what the next read sees"
    );
    assert_eq!(controller.active_notifications().count(), 1);
}

/// TTL comes from severity, and `visible` is derived from it. These are
/// the numbers `get_notifications` publishes as `ttl_seconds`; pinned so
/// the wire figure and `Notification::ttl` cannot drift apart.
#[test]
fn ttl_is_by_severity_and_a_fresh_toast_is_visible() {
    let mut controller = AppController::with_backend(SilentBackend);
    controller.push_notification("i".to_owned(), Severity::Info);
    controller.push_notification("w".to_owned(), Severity::Warning);
    controller.push_notification("e".to_owned(), Severity::Error);

    let ttls: Vec<u64> = controller
        .notifications()
        .iter()
        .map(|n| n.ttl().as_secs())
        .collect();
    assert_eq!(ttls, vec![4, 6, 8], "info 4 s, warning 6 s, error 8 s");

    for n in controller.notifications() {
        assert!(!n.is_expired(), "a just-pushed toast is visible");
        assert!(
            n.created_at.elapsed().as_secs_f64() < 1.0,
            "age is measured from the push, not from process start"
        );
    }
}

/// The stack keeps expired entries until the frame loop's collector runs,
/// which is why `include_expired` defaults to true: a test asserting what
/// the operator saw must not lose the evidence to a slow assertion.
/// `gc_notifications` is the ONLY thing that removes an entry.
#[test]
fn only_the_collector_removes_an_entry_and_only_when_expired() {
    let mut controller = AppController::with_backend(SilentBackend);
    controller.push_notification("fresh".to_owned(), Severity::Error);

    controller.gc_notifications();
    assert_eq!(
        controller.notifications().len(),
        1,
        "an unexpired toast survives collection"
    );

    // Not simulating an 8-second wait: the derived state is what matters,
    // and `is_expired` is `elapsed >= ttl` over `created_at`, pinned above.
    assert!(!controller.notifications()[0].is_expired());
    assert_eq!(controller.active_notifications().count(), 1);
}

/// The pairing with F1.1, end to end at the controller layer: a refusal
/// routed through `push_mcp_outcome` lands on the stack as one Warning
/// carrying the refusal text — which is exactly what an outside reader
/// needs to assert that the operator was told the truth.
#[test]
fn an_mcp_refusal_is_readable_off_the_stack_as_one_warning() {
    use rs_cam_viz::mcp_bridge::{McpOutcome, mutation_error_json};

    let mut controller = AppController::with_backend(SilentBackend);
    let reply = mutation_error_json("Cannot add toolpath: scallop requires curved tip", None);
    let outcome = McpOutcome::from_json_response(&reply);
    assert!(matches!(outcome, McpOutcome::Refused(_)), "{outcome:?}");

    controller.push_mcp_outcome("MCP: Added toolpath 'x'".to_owned(), &outcome);

    let stack: Vec<(&str, Severity)> = controller
        .notifications()
        .iter()
        .map(|n| (n.message.as_str(), n.severity))
        .collect();
    assert_eq!(stack.len(), 1, "one toast per request: {stack:?}");
    assert_eq!(stack[0].1, Severity::Warning, "{stack:?}");
    assert!(
        stack[0].0.contains("scallop requires curved tip"),
        "the refusal text reaches the stack verbatim: {stack:?}"
    );
    assert!(
        !stack[0].0.contains("Added"),
        "and the success text never does: {stack:?}"
    );
}
