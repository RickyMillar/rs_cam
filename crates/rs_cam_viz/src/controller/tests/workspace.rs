//! Workspace behaviour (Phase 7) and the simulation stamp matrix.

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
    // simulation epoch, as a real Run Simulation does.
    controller.handle_internal_event(AppEvent::RunSimulation);
    inject_sim_results(&mut controller, 1);

    // Should not be stale immediately
    assert!(
        !controller.state.simulation_is_stale(),
        "Fresh simulation should not be stale"
    );

    // W4: the edit is a real one. `GuiState::mark_edited` alone moves the
    // counter and writes no project data, and the core is right to keep the
    // run through it — see
    // `a_counter_bump_alone_does_not_stale_the_run_g_freshnessdisagree`.
    let tp_id = controller.state.session.toolpath_configs()[0].id;
    panel_edit(&mut controller, tp_id, |entry| {
        entry.operation.set_feed_rate(4321.0);
    });

    assert!(
        controller.state.simulation_is_stale(),
        "Simulation should be stale after job edit"
    );
}

#[test]
fn a_cancelled_run_consumes_the_simulation_stamps() {
    let mut controller = sample_controller();
    generate_all_for_test(&mut controller);
    inject_sim_results(&mut controller, 1);
    controller.handle_internal_event(AppEvent::RunSimulation);
    assert!(controller.state.simulation.submitted_edit_counter.is_some());
    assert!(
        controller
            .state
            .simulation
            .submitted_simulation_epoch
            .is_some()
    );

    controller
        .compute
        .drained
        .push(ComputeMessage::Simulation(Err(
            crate::compute::ComputeError::Cancelled,
        )));
    controller.drain_compute_results();

    assert!(controller.state.simulation.submitted_edit_counter.is_none());
    assert!(
        controller
            .state
            .simulation
            .submitted_simulation_epoch
            .is_none()
    );
}

#[test]
fn a_failed_run_consumes_the_simulation_stamps() {
    let mut controller = sample_controller();
    generate_all_for_test(&mut controller);
    inject_sim_results(&mut controller, 1);
    controller.handle_internal_event(AppEvent::RunSimulation);

    controller
        .compute
        .drained
        .push(ComputeMessage::Simulation(Err(
            crate::compute::ComputeError::Message("simulation failure fixture".to_owned()),
        )));
    controller.drain_compute_results();

    assert!(controller.state.simulation.submitted_edit_counter.is_none());
    assert!(
        controller
            .state
            .simulation
            .submitted_simulation_epoch
            .is_none()
    );
}

#[test]
fn reset_clears_the_run_and_the_stamps_without_dirtying_project() {
    let mut controller = sample_controller();
    generate_all_for_test(&mut controller);
    inject_sim_results(&mut controller, 1);
    controller.handle_internal_event(AppEvent::RunSimulation);
    let dirty_before = controller.state.gui.dirty;

    controller.handle_internal_event(AppEvent::Ui(UiCommand::ResetSimulation(NoArgs)));

    assert!(!controller.state.simulation.has_results());
    assert!(controller.state.simulation.last_run.is_none());
    assert!(controller.state.simulation.submitted_edit_counter.is_none());
    assert!(
        controller
            .state
            .simulation
            .submitted_simulation_epoch
            .is_none()
    );
    assert_eq!(controller.state.gui.dirty, dirty_before);
}
