//! The project Optimize run: its lane, its flags, its stage report and its
//! refusal.

use super::*;

/// Cancel arms THIS submit's flag, closes no window, and clears nothing.
///
/// Three claims in one arm, because they are one design decision:
///
/// * the flag that moves is the per-submit `Arc<AtomicBool>`, not a lane.
///   The `Job` lane is FIFO and shared with the MCP surface, so
///   `cancel_lane(ComputeLane::Job)` would also kill an MCP caller's job
///   queued behind this one — which is why none of the three existing
///   cancels fits the progress row;
/// * the run STAYS on the state with `cancel_requested` set. The drain is
///   the one clearing site, and a cancelled Optimize still returns a
///   partial outcome;
/// * the arm closes no window. The brief names `optimize_modal` here; this
///   fixture has none, so the dialog that must survive is the planner's.
///
/// RED at the parent revision: COMPILE-red. The arm names
/// `UiCommand::CancelOptimizeRun` and `AppState::optimize_run`, and
/// neither exists there.
#[test]
fn cancelling_the_run_arms_one_flag_and_clears_nothing() {
    let mut controller = planner_controller();
    controller.handle_internal_event(AppEvent::Ui(UiCommand::OpenMultitoolPlanner(NoArgs)));
    tick_ball_tools(&mut controller);
    controller.handle_internal_event(AppEvent::PreviewMultitoolPlan);

    let request = controller
        .compute
        .job_requests
        .first()
        .expect("a preview was submitted");
    let job_id = request.id;
    let cancel = Arc::clone(&request.cancel);
    assert!(!cancel.load(std::sync::atomic::Ordering::SeqCst));

    controller.handle_internal_event(AppEvent::Ui(UiCommand::CancelOptimizeRun(NoArgs)));

    assert!(
        cancel.load(std::sync::atomic::Ordering::SeqCst),
        "the cancel arms the flag of the submit that is running"
    );
    assert_eq!(
        controller.compute.job_lane.state,
        LaneState::Idle,
        "and it does NOT cancel the shared Job lane, which would kill an \
         MCP caller's job queued behind this one"
    );
    assert_eq!(
        controller.compute.optimize_lane.state,
        LaneState::Idle,
        "nor the Optimize lane, which this run does not ride"
    );
    assert_eq!(
        controller.compute.job_requests.len(),
        1,
        "and it submits nothing"
    );
    let run = controller
        .state
        .optimize_run
        .as_ref()
        .expect("the cancel does not clear the run — the drain does");
    assert!(
        run.cancel_requested,
        "the row reads 'cancelling' off this flag"
    );
    assert!(
        controller
            .state
            .multitool_planner
            .as_ref()
            .is_some_and(|planner| planner.open),
        "the cancel closes no window"
    );

    // The answer still lands, and the drain is the one clearing site.
    let result = crate::compute::JobResult {
        id: job_id,
        answer: Err(crate::compute::ComputeError::Cancelled),
    };
    let message = ComputeMessage::Job(Box::new(result));
    controller.compute.drained.push(message);
    controller.drain_compute_results();

    assert!(
        controller.state.optimize_run.is_none(),
        "the drain clears the run on every outcome, a cancel included"
    );
}

/// A second Optimize request is refused, and the refusal is VISIBLE.
///
/// One run at a time is the policy (§28 ruling 8), and the operator kept
/// it as a refusal. With the placeholder gone the buttons are clickable,
/// so the refusal is reachable and a `tracing::warn!` is not enough: a log
/// line tells the operator nothing about why their click did nothing.
///
/// `open_optimize_modal` refuses BEFORE it calls `ProjectSession::start`,
/// so this arm needs no cut trace either.
///
/// RED at the parent revision: ASSERTION-red — the refusal site reports
/// with `tracing::warn!` and pushes no notification, so the submit-count
/// assertion passes and the toast assertion finds nothing. That red is
/// MASKED in practice: the two arms above it are compile-red in the same
/// test target, so the verifier reads a compile error for the whole
/// `--lib` run.
#[test]
fn a_second_optimize_request_is_refused_with_a_toast() {
    let mut controller = planner_controller();
    controller.handle_internal_event(AppEvent::Ui(UiCommand::OpenMultitoolPlanner(NoArgs)));
    tick_ball_tools(&mut controller);
    controller.handle_internal_event(AppEvent::PreviewMultitoolPlan);
    assert_eq!(controller.compute.job_requests.len(), 1);
    assert_eq!(
        controller.active_notifications().count(),
        0,
        "the control: starting one run says nothing"
    );

    let toolpath_id = controller.state.session.toolpath_configs()[0].id;
    controller.handle_internal_event(AppEvent::OpenOptimizeModal(toolpath_id));

    assert_eq!(
        controller.compute.job_requests.len(),
        1,
        "the second request submits nothing"
    );
    let warnings: Vec<&str> = controller
        .active_notifications()
        .filter(|note| matches!(note.severity, Severity::Warning))
        .map(|note| note.message.as_str())
        .collect();
    assert_eq!(
        warnings.len(),
        1,
        "the refusal must push exactly one Warning toast, got {warnings:?}"
    );
    assert!(
        warnings[0].contains("Tier-map preview"),
        "and the toast must NAME the run that is holding the policy, got \
         {warnings:?}"
    );
}

// ── WP29 — the Optimize run reports its stage and its candidate count ─
//
// Programme: `planning/arch_consolidation_2026-09-09/IMPLEMENTATION_PLAN.md`
// §33 (operator ruling, 2026-09-13). The row and the window showed a
// spinner, a label and the elapsed seconds. They now name the rung of the
// search ladder and the candidate inside it.
//
// The DRAW half is two source scans in
// `crates/rs_cam_viz/tests/optimize_run_is_non_modal_wp24.rs`. These two
// arms are the TEXT half: one pure builder serves the row and the window,
// so a test reads the sentence the operator reads without an egui pass.
//
// **The WP24 limit still stands.** No controller fixture in this crate
// holds a cut trace, and `capture_optimize_toolpath` refuses without one,
// so the `Toolpath` kind of `OptimizeRun` cannot be DRIVEN in-crate. Arm 1
// therefore fabricates the run and writes the progress by hand, and arm 2
// drives the tier-map preview for the `None` branch. A tier-map preview
// runs no optimizer, so its progress is `None` by construction and it can
// never prove a count.
//
// RED at the parent revision: COMPILE-red. Both arms name
// `OptimizeRun::progress_text`, `OptimizeRun::progress` and
// `rs_cam_core::tool_load::optimize::OptimizeProgress`, and none of the
// three exists there.

/// A run with progress names the rung, the candidate and the list.
///
/// Three claims in one arm, because one builder answers all three: the
/// row's sentence, the window's two-row list, and the mark that says
/// which rung is running. The arm reads the run on the first rung, then
/// on the second rung, so each of the three marks appears.
#[test]
fn a_running_optimize_reports_its_stage_and_its_candidate_count() {
    use crate::state::OptimizeStageMark;
    use rs_cam_core::tool_load::optimize::{OptimizeProgress, SearchPhase};

    let state = AppState::new();
    let progress = Arc::new(OptimizeProgress::default());
    progress.begin_phase(SearchPhase::Links, 8);
    progress.begin_candidate(2);

    let run = crate::state::OptimizeRun {
        kind: crate::state::OptimizeRunKind::Toolpath {
            toolpath_id: rs_cam_core::ToolpathId(0),
        },
        job_id: Some(crate::compute::JobRequestId(0)),
        started_at: std::time::Instant::now(),
        cancel_requested: false,
        progress: Some(Arc::clone(&progress)),
    };

    // The first rung runs.
    let text = run.progress_text(&state.session);
    assert!(
        text.contains("stage 1/2"),
        "the row must name the rung and the ladder length, got {text:?}"
    );
    assert!(
        text.contains("candidate 3/8"),
        "the row must name the candidate now running and the count the \
         rung formed, got {text:?}"
    );
    let rows = run.stage_rows();
    assert_eq!(
        rows.len(),
        SearchPhase::ALL.len(),
        "the window lists every rung of the ladder, not only the running \
         one"
    );
    let marks: Vec<OptimizeStageMark> = rows.iter().map(|row| row.mark).collect();
    assert_eq!(
        marks,
        vec![OptimizeStageMark::Current, OptimizeStageMark::Pending],
        "the running rung reads current, and the rung the search has not \
         reached reads pending"
    );
    assert!(
        rows.first()
            .and_then(|row| row.count.as_deref())
            .is_some_and(|count| count.contains("3 / 8")),
        "the running rung's row carries its own count, got {rows:?}"
    );
    assert!(
        rows.last().is_some_and(|row| row.count.is_none()),
        "a rung the search has not announced reports NO count, because \
         its total is not known until it starts: {rows:?}"
    );

    // The second rung runs.
    progress.begin_phase(SearchPhase::DepthWidth, 4);
    progress.begin_candidate(1);
    let text = run.progress_text(&state.session);
    assert!(text.contains("stage 2/2"), "got {text:?}");
    assert!(text.contains("candidate 2/4"), "got {text:?}");
    let marks: Vec<OptimizeStageMark> = run.stage_rows().iter().map(|row| row.mark).collect();
    assert_eq!(
        marks,
        vec![OptimizeStageMark::Done, OptimizeStageMark::Current],
        "the finished rung reads done, and the running rung reads current"
    );
}

/// A run with no progress keeps the WP24 sentence.
///
/// The rollup and the tier-map preview run no candidate ladder, so they
/// carry no progress and the row must not invent a rung for them. This is
/// the branch the planner fixture CAN drive.
#[test]
fn a_run_without_progress_keeps_the_elapsed_only_text() {
    let mut controller = planner_controller();
    controller.handle_internal_event(AppEvent::Ui(UiCommand::OpenMultitoolPlanner(NoArgs)));
    tick_ball_tools(&mut controller);
    controller.handle_internal_event(AppEvent::PreviewMultitoolPlan);

    let run = controller
        .state
        .optimize_run
        .as_ref()
        .expect("the preview is in flight, so the run is on the state");
    assert!(
        run.progress.is_none(),
        "a tier-map preview runs no candidate ladder, so it carries no \
         progress"
    );

    let text = run.progress_text(&controller.state.session);
    assert!(
        !text.contains("stage"),
        "a run with no progress must not claim a rung, got {text:?}"
    );
    assert!(
        text.contains("Tier-map preview"),
        "and it keeps the WP24 label, got {text:?}"
    );
    assert!(
        run.stage_rows().is_empty(),
        "the window lists no rung for a run that walks no ladder"
    );
}

/// R8: the controller refuses a candidate that is not safe, whatever index
/// the event names. Only the button read `candidate_is_safe` before; an
/// event from another door (a script, a stale modal) applied the
/// candidate.
///
/// The candidate below is faster and moves the feed, but its full
/// cut fails. The apply must leave the operation as it was, keep the modal
/// open, and say why.
#[test]
fn an_unsafe_candidate_is_refused_by_the_controller_r8() {
    use rs_cam_core::tool_load::optimize::{
        FullCutCheck, FullCutReadings, FullCutVerdict, OptimizeCandidate, OptimizeOutcome,
        OutcomeNarrative, ParamDelta, SearchStage,
    };
    use rs_cam_core::tool_load::verdict::{
        ChiploadVerdict, DeflectionVerdict, DepthVerdict, PowerVerdict, ToolpathLoadVerdict,
        UnmodeledReason,
    };

    let mut controller = sample_controller();
    let toolpath_id = push_toolpath(&mut controller, "R8");
    let index = index_of(&controller, toolpath_id);
    let before = controller
        .state
        .session
        .get_toolpath_config(index)
        .expect("the toolpath exists")
        .operation
        .clone();

    let na = || UnmodeledReason::NotApplicableForOp("R8 fixture".to_owned());
    let verdict = ToolpathLoadVerdict {
        toolpath_id,
        chipload: ChiploadVerdict::Unmodeled { reason: na() },
        power: PowerVerdict::Unmodeled { reason: na() },
        deflection: DeflectionVerdict::Unmodeled { reason: na() },
        depth: DepthVerdict::Unmodeled { reason: na() },
        drill_gates: None,
        modulation_summary: None,
        feed_explanation: None,
        kinematic_utilization: None,
        material_split: None,
    };
    let readings = FullCutReadings {
        removed_volume_mm3: Some(1000.0),
        rapid_collisions: Some(0),
        overcut_peak_mm: Some(0.0),
    };
    let candidate = |full_cut: FullCutCheck, params: OperationConfig| OptimizeCandidate {
        params,
        dressups: Box::default(),
        delta: ParamDelta::default(),
        cycle_time_s: 100.0,
        verdict: verdict.clone(),
        full_cut,
        stage: SearchStage::Measured,
        reconciled_cycle_time_s: None,
        reconciled_verdict: None,
        air_cut_fraction_of_total_runtime: None,
    };
    let mut faster = before.clone();
    faster.set_feed_rate(before.feed_rate() + 500.0);
    assert!((faster.feed_rate() - before.feed_rate() - 500.0).abs() < 1e-9);
    let failing = FullCutCheck {
        verdict: FullCutVerdict::Fails {
            reason: "the removed volume is short".to_owned(),
        },
        ..FullCutCheck::reference(readings)
    };
    let mut baseline = candidate(FullCutCheck::reference(readings), before.clone());
    baseline.stage = SearchStage::Baseline;
    baseline.cycle_time_s = 200.0;
    let outcome = OptimizeOutcome::ranked(
        vec![baseline, candidate(failing, faster)],
        None,
        OutcomeNarrative::default(),
    );
    controller.state.optimize_modal = Some(crate::state::OptimizeModalState {
        toolpath_id,
        status: crate::state::OptimizeRunStatus::Ready(outcome),
    });

    controller.handle_internal_event(AppEvent::ApplyOptimizeCandidate {
        toolpath_id,
        candidate_index: 1,
    });

    let after = &controller
        .state
        .session
        .get_toolpath_config(index)
        .expect("the toolpath exists")
        .operation;
    assert_eq!(
        format!("{after:?}"),
        format!("{before:?}"),
        "an unsafe candidate must not be applied"
    );
    assert!(
        controller.state.optimize_modal.is_some(),
        "the refusal keeps the modal open"
    );
    let note = controller
        .notifications()
        .iter()
        .find(|n| {
            n.message
                .starts_with("The candidate is not safe to apply: ")
        })
        .expect("the refusal is a notification");
    assert!(
        note.message.contains("the removed volume is short"),
        "{}",
        note.message
    );
}
