//! MCP `cancel_generation` (WP23): a cancelled generation resolves the
//! waiter it left behind.

use super::*;

// ---------------------------------------------------------------------------
// MCP `cancel_generation`: a cancelled generation must resolve any pending
// MCP `generate_toolpath` waiter instead of leaving it hanging — mirroring
// the fail-hard-at-submit fix immediately above, but for the
// cancel-in-flight path instead of the reject-before-submit path.
//
// WP23 deleted the lane-targeting test that shared this banner. It pinned
// `UiCommand::CancelToolpathGeneration`, a view row that no surface
// constructed: the MCP tool cancels the toolpath lane through
// `GenerationControl` on the server thread, and the GUI cancels every lane
// through `UiCommand::CancelCompute`.
// ---------------------------------------------------------------------------

/// A `Cancelled` outcome draining through `drain_compute_results` must
/// resolve a pending MCP `generate_toolpath` waiter, the same way a
/// submit-time fail-hard already does (see the pair of tests above this
/// section). Without this, cancelling a runaway generate over MCP would
/// stop the compute but still leave the original `generate_toolpath` call
/// hanging forever — trading a hang-on-completion for a hang-on-cancel.
#[cfg(feature = "mcp")]
#[test]
fn cancelled_drain_resolves_pending_mcp_generate_toolpath_waiter() {
    let mut controller = sample_controller();
    let tp_id = ToolpathId(0);

    controller.pending_mcp = Some(crate::mcp_bridge::PendingMcpCompute::new());
    let (tx, mut rx) = tokio::sync::oneshot::channel();
    controller
        .pending_mcp
        .as_mut()
        .expect("pending_mcp was just set")
        .toolpath
        .insert(tp_id, tx);

    controller
        .compute
        .drained
        .push(ComputeMessage::Toolpath(Box::new(
            crate::compute::worker::ComputeResult {
                toolpath_id: tp_id,
                revision: None,
                result: Err(crate::compute::ComputeError::Cancelled),
                debug_trace: None,
                semantic_trace: None,
                debug_trace_path: None,
            },
        )));

    controller.drain_compute_results();

    let response = rx
        .try_recv()
        .expect("a cancelled drain must resolve the pending MCP oneshot, not strand it");
    let payload = response
        .result
        .expect("mcp response should carry an Ok(json) payload describing the cancellation");
    assert!(
        payload.to_lowercase().contains("cancel"),
        "mcp payload for a cancelled generate should say so plainly, got: {payload}"
    );

    let rt = controller
        .state
        .gui
        .toolpath_rt
        .get(&tp_id)
        .expect("toolpath runtime should exist after cancel");
    assert!(
        matches!(rt.status, crate::state::toolpath::ComputeStatus::Pending),
        "cancelled toolpath status should revert to Pending (not Done), got {:?}",
        rt.status
    );

    assert!(
        !controller
            .pending_mcp
            .as_ref()
            .expect("pending_mcp still set")
            .toolpath
            .contains_key(&tp_id),
        "resolved MCP waiter should be removed from the pending map"
    );
}

// ---------------------------------------------------------------------------
// Roadmap F.1 — session.results cache must repopulate when the threaded
// compute backend returns a fresh result. Before the fix, the callback
// only wrote to gui.toolpath_rt and session.results stayed empty after
// the undo snapshot path ran — breaking project_load_report's span
// lookup for the just-applied toolpath. See planning/F1_RCA.md.
// ---------------------------------------------------------------------------

#[test]
fn drain_compute_results_repopulates_session_results() {
    let mut controller = sample_controller();
    // sample_controller pre-seeds rt.result but leaves session.results
    // empty — exactly the post-Apply pre-fix divergence state.
    assert!(
        controller.state.session.get_result(0).is_none(),
        "fixture should start with empty session.results[0]"
    );

    let annotated = Arc::new(rs_cam_core::trace::toolpath_spans::AnnotatedToolpath::new(
        Toolpath::new(),
    ));

    controller
        .compute
        .drained
        .push(ComputeMessage::Toolpath(Box::new(
            crate::compute::worker::ComputeResult {
                toolpath_id: ToolpathId(0),
                revision: None,
                result: Ok(ToolpathResult {
                    annotated: Arc::clone(&annotated),
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

    // The fix: session.results[0] is populated.
    let session_result = controller
        .state
        .session
        .get_result(0)
        .expect("session.results[0] should be populated after drain");
    // Both caches share the same Arc (no duplicated allocation).
    assert!(Arc::ptr_eq(session_result.annotated(), &annotated));
}
