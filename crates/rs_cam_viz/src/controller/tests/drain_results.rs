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
