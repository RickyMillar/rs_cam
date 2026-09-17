//! Workspace behaviour (Phase 7) and the metric-capture staleness matrix.

use super::*;

// ---------------------------------------------------------------------------
// Workspace behavior tests (Phase 7)
// ---------------------------------------------------------------------------

#[test]
fn workspace_defaults_to_toolpaths() {
    let controller = AppController::with_backend(ScriptedBackend::new());
    assert_eq!(
        controller.state.workspace,
        crate::state::Workspace::Toolpaths
    );
}

#[test]
fn workspace_switch_preserves_simulation_results() {
    let mut controller = sample_controller();

    // Inject simulation results
    inject_sim_results(&mut controller, 1);

    assert!(controller.state.simulation.has_results());

    // Switch to Toolpaths
    controller.state.workspace = crate::state::Workspace::Toolpaths;
    assert!(
        controller.state.simulation.has_results(),
        "Simulation results should persist when leaving Simulation workspace"
    );

    // Switch to Setup
    controller.state.workspace = crate::state::Workspace::Setup;
    assert!(
        controller.state.simulation.has_results(),
        "Simulation results should persist in Setup workspace"
    );
}

#[test]
fn reset_simulation_clears_results_and_checks() {
    let mut controller = sample_controller();
    inject_sim_results(&mut controller, 1);

    assert!(controller.state.simulation.has_results());

    controller.handle_internal_event(crate::ui::AppEvent::Ui(UiCommand::ResetSimulation(NoArgs)));

    assert!(
        !controller.state.simulation.has_results(),
        "Reset should clear results"
    );
    assert!(
        controller
            .state
            .simulation
            .checks
            .rapid_collisions
            .is_empty(),
        "Reset should clear rapid collisions"
    );
    assert_eq!(
        controller.state.simulation.checks.holder_collision_count, 0,
        "Reset should clear holder collision count"
    );
    assert!(
        controller.state.simulation.last_run.is_none(),
        "Reset should clear run metadata"
    );
}

#[test]
fn simulation_staleness_tracks_edits() {
    let mut controller = sample_controller();
    generate_all_for_test(&mut controller);
    // UR3 (f97327c3): an UNSTAMPED result reads stale, never current. A
    // fresh-looking run must come through the submit that stamps the
    // capture revision, as a real Run Simulation does.
    controller.handle_internal_event(AppEvent::RunSimulation);
    inject_sim_results(&mut controller, 1);

    // Should not be stale immediately
    assert!(
        !controller
            .state
            .simulation
            .is_stale(controller.state.gui.edit_counter),
        "Fresh simulation should not be stale"
    );

    // Mark an edit
    controller.state.gui.mark_edited();

    assert!(
        controller
            .state
            .simulation
            .is_stale(controller.state.gui.edit_counter),
        "Simulation should be stale after job edit"
    );
}

#[test]
fn metric_capture_toggle_before_first_result_tracks_in_flight_mismatch_without_dirtying_project() {
    let mut controller = sample_controller();
    generate_all_for_test(&mut controller);
    let dirty_before = controller.state.gui.dirty;
    let revision_before = controller.state.simulation.metric_options_revision;

    controller.state.simulation.set_metric_capture_enabled(true);
    assert_eq!(
        controller.state.simulation.metric_options_revision,
        revision_before + 1
    );
    assert!(
        !controller.state.simulation.metric_options_are_stale(),
        "a pre-first-run capture choice has no evidence to stale"
    );

    controller.handle_internal_event(AppEvent::RunSimulation);
    assert_eq!(
        controller
            .state
            .simulation
            .submitted_metric_options_revision,
        Some(revision_before + 1),
        "the run must stamp the capture revision it answers"
    );
    controller
        .state
        .simulation
        .set_metric_capture_enabled(false);
    assert!(
        !controller.state.simulation.metric_options_are_stale(),
        "there is still no accepted evidence before the first result lands"
    );

    inject_sim_results(&mut controller, 1);

    assert!(
        controller.state.simulation.metric_options_are_stale(),
        "a first result answering the older in-flight capture revision is stale"
    );
    assert_eq!(
        controller.state.gui.dirty, dirty_before,
        "runtime capture choices must not dirty the machining project"
    );
}

#[test]
fn metric_capture_toggle_after_results_marks_existing_evidence_stale_without_dirtying_project() {
    let mut controller = sample_controller();
    inject_sim_results(&mut controller, 1);
    let dirty_before = controller.state.gui.dirty;
    let revision_before = controller.state.simulation.metric_options_revision;

    controller.state.simulation.set_metric_capture_enabled(true);

    assert_eq!(
        controller.state.simulation.metric_options_revision,
        revision_before + 1
    );
    assert!(controller.state.simulation.metric_options_are_stale());
    assert_eq!(controller.state.gui.dirty, dirty_before);
}

#[test]
fn metric_capture_matching_success_clears_stale_marker() {
    let mut controller = sample_controller();
    generate_all_for_test(&mut controller);
    inject_sim_results(&mut controller, 1);
    controller.state.simulation.set_metric_capture_enabled(true);
    assert!(controller.state.simulation.metric_options_are_stale());

    controller.handle_internal_event(AppEvent::RunSimulation);
    inject_sim_results(&mut controller, 1);

    assert!(
        !controller.state.simulation.metric_options_are_stale(),
        "a successful result for the submitted capture revision is current"
    );
}

#[test]
fn metric_capture_unstamped_success_preserves_stale_marker() {
    let mut controller = sample_controller();
    inject_sim_results(&mut controller, 1);
    controller.state.simulation.set_metric_capture_enabled(true);
    assert!(
        controller
            .state
            .simulation
            .submitted_metric_options_revision
            .is_none()
    );

    inject_sim_results(&mut controller, 1);

    assert!(
        controller.state.simulation.metric_options_are_stale(),
        "an unstamped result cannot clear a marker whose capture revision it cannot prove"
    );
}

#[test]
fn metric_capture_cancel_preserves_marker_and_consumes_simulation_stamps() {
    let mut controller = sample_controller();
    generate_all_for_test(&mut controller);
    inject_sim_results(&mut controller, 1);
    controller.state.simulation.set_metric_capture_enabled(true);
    controller.handle_internal_event(AppEvent::RunSimulation);
    assert!(controller.state.simulation.submitted_edit_counter.is_some());
    assert!(
        controller
            .state
            .simulation
            .submitted_metric_options_revision
            .is_some()
    );

    controller
        .compute
        .drained
        .push(ComputeMessage::Simulation(Err(
            crate::compute::ComputeError::Cancelled,
        )));
    controller.drain_compute_results();

    assert!(controller.state.simulation.metric_options_are_stale());
    assert!(controller.state.simulation.submitted_edit_counter.is_none());
    assert!(
        controller
            .state
            .simulation
            .submitted_metric_options_revision
            .is_none()
    );
}

#[test]
fn metric_capture_error_preserves_marker_and_consumes_simulation_stamps() {
    let mut controller = sample_controller();
    generate_all_for_test(&mut controller);
    inject_sim_results(&mut controller, 1);
    controller.state.simulation.set_metric_capture_enabled(true);
    controller.handle_internal_event(AppEvent::RunSimulation);

    controller
        .compute
        .drained
        .push(ComputeMessage::Simulation(Err(
            crate::compute::ComputeError::Message("metric capture fixture".to_owned()),
        )));
    controller.drain_compute_results();

    assert!(controller.state.simulation.metric_options_are_stale());
    assert!(controller.state.simulation.submitted_edit_counter.is_none());
    assert!(
        controller
            .state
            .simulation
            .submitted_metric_options_revision
            .is_none()
    );
}

#[test]
fn metric_capture_reset_clears_evidence_and_stamps_without_dirtying_project() {
    let mut controller = sample_controller();
    generate_all_for_test(&mut controller);
    inject_sim_results(&mut controller, 1);
    controller.state.simulation.set_metric_capture_enabled(true);
    controller.handle_internal_event(AppEvent::RunSimulation);
    let dirty_before = controller.state.gui.dirty;

    controller.handle_internal_event(AppEvent::Ui(UiCommand::ResetSimulation(NoArgs)));

    assert!(!controller.state.simulation.has_results());
    assert!(!controller.state.simulation.metric_options_are_stale());
    assert!(controller.state.simulation.submitted_edit_counter.is_none());
    assert!(
        controller
            .state
            .simulation
            .submitted_metric_options_revision
            .is_none()
    );
    assert_eq!(controller.state.gui.dirty, dirty_before);
}
