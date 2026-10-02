//! Roadmap F.1 — the drain repopulates `session.results`, and it refuses a
//! completion whose revision moved.

use super::*;

/// WP3 — the drain hands the lane's revision stamp to
/// `Command::AdoptResult`, and core refuses a completion whose toolpath
/// moved while the job ran.
///
/// The refusal costs the operator nothing visible: `rt.result` still
/// carries the geometry, so the viewport draws it. Only the CORE slot
/// stays empty, which is what derives `EditedSince` and puts STALE on
/// every surface F2.2 wired. This is the outcome the deleted viz gate
/// produced, now produced by the one door.
#[test]
fn drain_refuses_a_completion_whose_revision_moved() {
    let mut controller = sample_controller();
    let submitted = controller.state.session.toolpath_revision(0);

    // The edit an operator makes while the lane runs.
    let _ = controller
        .state
        .session
        .apply(Command::InvalidateToolpathInputs(
            InvalidateToolpathInputsArgs { index: 0 },
        ))
        .expect("toolpath 0 exists");
    assert_ne!(
        controller.state.session.toolpath_revision(0),
        submitted,
        "the fixture must move the revision, or the test proves nothing"
    );

    push_toolpath_completion(&mut controller, Some(submitted));
    controller.drain_compute_results();

    assert!(
        controller.state.session.get_result(0).is_none(),
        "a completion that answers a superseded revision must not reach \
         the core cache"
    );
    assert!(
        controller.state.gui.toolpath_rt[&ToolpathId(0)]
            .result
            .is_some(),
        "the viz copy is kept, so the viewport still draws the geometry"
    );
}

/// The sibling arm: a completion that answers the CURRENT revision is
/// adopted. This is what fails if the stamp reads a session-global
/// counter instead of the toolpath's own revision.
#[test]
fn drain_adopts_a_completion_whose_revision_is_current() {
    let mut controller = sample_controller();
    // Move the revision FIRST, so the test cannot pass on a stamp that
    // only ever matches the initial value.
    let _ = controller
        .state
        .session
        .apply(Command::InvalidateToolpathInputs(
            InvalidateToolpathInputsArgs { index: 0 },
        ))
        .expect("toolpath 0 exists");
    let submitted = controller.state.session.toolpath_revision(0);

    push_toolpath_completion(&mut controller, Some(submitted));
    controller.drain_compute_results();

    assert!(
        controller.state.session.get_result(0).is_some(),
        "a completion that answers the current revision is adopted"
    );
}

#[test]
fn drain_compute_results_clears_pending_apply_resim_on_success() {
    // Roadmap F.2 — auto-verify after Apply. The drain handler must
    // clear `pending_apply_resim` and trigger the project sim when
    // the regen for the just-applied candidate lands. We assert the
    // pending flag is cleared; the actual sim submission is a side
    // effect on the backend (covered by integration of F.2's behavior
    // in the GUI).
    let mut controller = sample_controller();
    controller.state.pending_apply_resim = Some(0);
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
                    annotated,
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

    assert!(
        controller.state.pending_apply_resim.is_none(),
        "pending_apply_resim should be cleared when the matching regen lands"
    );
}

#[test]
fn drain_compute_results_keeps_pending_apply_resim_for_other_toolpath() {
    // If a regen lands for a different TP than the one awaiting
    // verification, the pending flag stays set — we only kick the
    // post-apply sim when the matching TP's regen finishes.
    let mut controller = sample_controller();
    controller.state.pending_apply_resim = Some(42);
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
                    annotated,
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

    assert_eq!(
        controller.state.pending_apply_resim,
        Some(42),
        "pending_apply_resim should remain set when an unrelated regen lands"
    );
}

#[test]
fn drain_compute_results_skips_session_write_on_error() {
    let mut controller = sample_controller();
    controller
        .compute
        .drained
        .push(ComputeMessage::Toolpath(Box::new(
            crate::compute::worker::ComputeResult {
                toolpath_id: ToolpathId(0),
                revision: None,
                result: Err(crate::compute::ComputeError::Message("boom".into())),
                debug_trace: None,
                semantic_trace: None,
                debug_trace_path: None,
            },
        )));

    controller.drain_compute_results();

    assert!(
        controller.state.session.get_result(0).is_none(),
        "session.results must not be written on compute error"
    );
}

/// G-CACHEKEYS sentry: the reach overlay keys on the inputs the map reads.
///
/// The overlay used to key on `GuiState::edit_counter`. An MCP core command
/// applies through `ProjectSession::apply` and never calls `mark_edited`, so
/// an MCP `set_tool_param` or `set_toolpath_param` left the previous map on
/// screen. These tests apply the edits the way the MCP path does, and assert
/// that the counter did not move (the non-vacuity anchor), that the sweep
/// submits exactly one new walk, and that an unrelated edit submits none.
mod reach_overlay_key_g_cachekeys {
    use super::*;

    use rs_cam_core::session::{SetToolParamArgs, SetToolpathParamArgs};

    use rs_cam_core::tool::MillingCutter;

    use crate::state::runtime::ReachStatus;

    /// Apply `command` the way the MCP core path does: straight to the session,
    /// with no `mark_edited` and no controller event.
    fn apply_like_mcp(controller: &mut AppController<ScriptedBackend>, command: Command) {
        let _ = controller
            .state
            .session
            .apply(command)
            .expect("the MCP-style edit applies");
    }

    fn reach_submits(controller: &AppController<ScriptedBackend>) -> usize {
        controller.compute.reach_requests.len()
    }

    /// A sample controller whose overlay has resolved its first walk.
    fn settled_controller() -> AppController<ScriptedBackend> {
        let mut controller = sample_controller();
        controller.process_reach_overlay();
        assert_eq!(
            reach_submits(&controller),
            1,
            "the selected scallop toolpath resolves one reach walk"
        );
        controller.process_reach_overlay();
        assert_eq!(
            reach_submits(&controller),
            1,
            "a pump with no edit resolves no second walk"
        );
        controller
    }

    #[test]
    fn an_mcp_tool_edit_resubmits_the_reach_walk_once_g_cachekeys() {
        let mut controller = settled_controller();
        let counter = controller.state.gui.edit_counter;

        apply_like_mcp(
            &mut controller,
            Command::SetToolParam(SetToolParamArgs {
                index: 0,
                param: "diameter".to_owned(),
                value: serde_json::json!(8.0),
            }),
        );
        assert_eq!(
            controller.state.gui.edit_counter, counter,
            "anchor: the MCP-style edit does not move edit_counter"
        );

        controller.process_reach_overlay();
        assert_eq!(
            reach_submits(&controller),
            2,
            "the tool edit resolves exactly one new walk"
        );
        controller.process_reach_overlay();
        assert_eq!(
            reach_submits(&controller),
            2,
            "the new key holds: a second pump submits nothing"
        );
        let first = &controller.compute.reach_requests[0].spec;
        let second = &controller.compute.reach_requests[1].spec;
        assert!(
            (second.cutter.diameter() - 8.0).abs() < 1e-9
                && (first.cutter.diameter() - 8.0).abs() > 1e-3,
            "the new walk carries the edited tool"
        );
    }

    #[test]
    fn an_mcp_toolpath_param_edit_resubmits_the_reach_walk_once_g_cachekeys() {
        let mut controller = settled_controller();
        let counter = controller.state.gui.edit_counter;

        apply_like_mcp(
            &mut controller,
            Command::SetToolpathParam(SetToolpathParamArgs {
                index: 0,
                param: "scallop_height".to_owned(),
                value: serde_json::json!(0.031),
            }),
        );
        assert_eq!(
            controller.state.gui.edit_counter, counter,
            "anchor: the MCP-style edit does not move edit_counter"
        );

        controller.process_reach_overlay();
        assert_eq!(
            reach_submits(&controller),
            2,
            "the tolerance edit resolves exactly one new walk"
        );
        controller.process_reach_overlay();
        assert_eq!(reach_submits(&controller), 2);
        assert!(
            (controller.compute.reach_requests[1]
                .spec
                .params
                .tolerance_mm
                - 0.031)
                .abs()
                < 1e-12,
            "the new walk carries the edited tolerance"
        );
    }

    #[test]
    fn an_unrelated_edit_does_not_resubmit_the_reach_walk_g_cachekeys() {
        let mut controller = settled_controller();
        // The old key moved here and blinked the overlay to "computing".
        controller.state.gui.mark_edited();
        controller.state.gui.mark_edited();
        controller.process_reach_overlay();
        assert_eq!(
            reach_submits(&controller),
            1,
            "an edit that moves no input of the map submits no walk"
        );
    }

    #[test]
    fn a_result_for_the_previous_key_is_dropped_g_cachekeys() {
        let mut controller = settled_controller();
        let id = controller
            .state
            .gui
            .reach_overlay
            .toolpath
            .expect("a toolpath is selected");
        let old_key = rs_cam_core::maps::reach_map_cache::ReachRequestKey::of(
            &controller.compute.reach_requests[0].spec,
        );

        apply_like_mcp(
            &mut controller,
            Command::SetToolParam(SetToolParamArgs {
                index: 0,
                param: "diameter".to_owned(),
                value: serde_json::json!(8.0),
            }),
        );
        controller.process_reach_overlay();
        assert_eq!(reach_submits(&controller), 2);

        // The walk for the old key was already sent when the edit landed.
        controller
            .compute
            .drained
            .push(ComputeMessage::Reach(Box::new(
                crate::compute::ReachResult {
                    toolpath_id: id,
                    key: old_key,
                    result: Err(crate::compute::ComputeError::Message("old walk".to_owned())),
                    colors: Arc::new(Vec::new()),
                },
            )));
        controller.drain_compute_results();
        assert!(
            matches!(
                controller.state.gui.reach_overlay.status,
                ReachStatus::Computing
            ),
            "a result for the old key must not land on the overlay"
        );

        // Non-vacuity: the same result under the live key does land.
        let live_key = rs_cam_core::maps::reach_map_cache::ReachRequestKey::of(
            &controller.compute.reach_requests[1].spec,
        );
        controller
            .compute
            .drained
            .push(ComputeMessage::Reach(Box::new(
                crate::compute::ReachResult {
                    toolpath_id: id,
                    key: live_key,
                    result: Err(crate::compute::ComputeError::Message("new walk".to_owned())),
                    colors: Arc::new(Vec::new()),
                },
            )));
        controller.drain_compute_results();
        assert!(
            matches!(
                controller.state.gui.reach_overlay.status,
                ReachStatus::Failed(_)
            ),
            "a result for the live key lands"
        );
    }
}
