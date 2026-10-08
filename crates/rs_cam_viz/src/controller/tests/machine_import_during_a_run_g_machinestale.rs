//! G-MACHINESTALE: a machine import lands while a simulation runs.
//!
//! Seen once, live, 2026-10-02 (`planning/memory_budget_2026-10-01/
//! BASELINES.md`, W3): the MCP `import_machine_settings` ran a few seconds
//! after the operator clicked Simulate. After the run the GUI read
//! "Results may be stale (params changed)", and the cycle time read
//! "cutting only: not simulated" beside the run on screen.
//!
//! The interleaving, in the order the controller sees it:
//!
//! 1. The submit door releases the view of the previous run and stamps the
//!    simulation epoch `E`. The run carries the machine as it is NOW: its
//!    `KinematicsContext` and rapid rate are built at submit.
//! 2. The MCP import applies `Command::ImportMachineSettings`. The core drops
//!    its simulation and moves the epoch to `E + 1`. The MCP door does not
//!    mirror the drop into the view, so the run keeps working.
//! 3. The run lands with stamp `E`. The core refuses the adopt (D7), and the
//!    view keeps the run. That stale verdict is CORRECT: the run used the
//!    old kinematics and the old travel rate.
//!
//! What was wrong is what the operator was told:
//!
//! - every stale label said "params changed", also for a machine edit;
//! - the drain dropped the late run's cut trace, so the cycle time read
//!   "not simulated" while the run was on screen;
//! - the re-run read "Ready to simulate", because the session held no run
//!   and so the state was not a release.
//!
//! The sentry drives the interleaving with the scripted backend and the
//! door the MCP arm uses (`session.apply` plus `mark_edited`).

use super::*;

use crate::state::freshness::{SimFreshness, running_label};
use rs_cam_core::machine::kinematics::{CycleTimeBreakdown, MachineKinematics};
use rs_cam_core::session::{
    CycleTimeBasis, ImportMachineSettingsArgs, SimulationDropCause, SimulationDropCauses,
};
use rs_cam_core::stock::simulation_cut::{SimulationCutTrace, ToolpathKinematicRuntime};

/// The seconds the trace claims for the simulated toolpath.
const TRACE_SECONDS: f64 = 123.0;

/// Land one simulation result that carries a cut trace, the way every GUI
/// run does since the "always capture" ruling (2026-10-02). The trace holds
/// an integrator runtime for `ToolpathId(0)`, so the cycle time can read
/// it.
fn inject_a_run_with_a_trace(controller: &mut AppController<ScriptedBackend>) {
    use rs_cam_core::stock::stock_mesh::StockMesh;

    let mesh = Arc::new(StockMesh {
        vertices: vec![0.0; 9],
        indices: vec![0, 1, 2],
        colors: vec![0.5; 9],
    });
    let trace = SimulationCutTrace {
        toolpath_runtimes: vec![ToolpathKinematicRuntime {
            toolpath_id: ToolpathId(0),
            breakdown: CycleTimeBreakdown {
                total_s: TRACE_SECONDS,
                cutting_s: TRACE_SECONDS,
                ..CycleTimeBreakdown::default()
            },
        }],
        ..SimulationCutTrace::test_fixture()
    };
    controller
        .compute
        .drained
        .push(ComputeMessage::Simulation(Ok(Box::new(SimulationResult {
            core: rs_cam_core::compute::simulate::SimulationResult {
                mesh,
                total_moves: 10,
                deviations: None,
                column_deviations: None,
                boundaries: vec![crate::compute::worker::SimBoundary {
                    id: ToolpathId(0),
                    name: "Op 1".to_owned(),
                    tool_name: "EndMill".to_owned(),
                    start_move: 0,
                    end_move: 10,
                    direction: rs_cam_core::dexel_stock::StockCutDirection::FromTop,
                }],
                checkpoints: Vec::new(),
                rapid_collisions: Vec::new(),
                rapid_collision_move_indices: Vec::new(),
                cut_trace: Some(Arc::new(trace)),
                column_grid_cell_mm: 0.5,
                resolution_clamped: false,
                prior_stocks: std::collections::HashMap::new(),
                prior_stock_sources: std::collections::HashMap::new(),
                display_degrade: None,
            },
            playback_data: Vec::new(),
            cut_trace_path: None,
        }))));
    controller.drain_compute_results();
}

/// The MCP `import_machine_settings` arm, without the GRBL parse: the core
/// row, then `mark_edited`. The arm mirrors nothing into the view.
fn import_machine_settings_as_mcp_does(controller: &mut AppController<ScriptedBackend>) {
    let kinematics = MachineKinematics::default();
    let _effects = controller
        .state
        .session
        .apply(Command::ImportMachineSettings(ImportMachineSettingsArgs {
            kinematics: Box::new(kinematics),
            max_feed_mm_min: Some(4321.0),
            controller: None,
        }))
        .expect("the import row applies");
    controller.state.gui.mark_edited();
}

/// A project with a generated toolpath and one landed run that the core
/// holds: the state after `generate_all` and its closing simulation.
fn controller_after_generate_all() -> AppController<ScriptedBackend> {
    let mut controller = sample_controller();
    generate_all_for_test(&mut controller);
    controller.handle_internal_event(AppEvent::RunSimulation);
    inject_a_run_with_a_trace(&mut controller);
    assert_eq!(
        controller.state.simulation_freshness(),
        SimFreshness::Current,
        "the fixture run must start current, or the sentry measures nothing"
    );
    controller
}

fn only_machine() -> SimulationDropCauses {
    let mut causes = SimulationDropCauses::EMPTY;
    causes.insert(SimulationDropCause::Machine);
    causes
}

/// The live interleaving, step by step, with the text of each step.
#[test]
fn a_machine_import_during_a_run_reads_machine_stale_g_machinestale() {
    let mut controller = controller_after_generate_all();

    // 1. The operator clicks Simulate.
    controller.handle_internal_event(AppEvent::RunSimulation);
    assert_eq!(
        controller.state.simulation_freshness(),
        SimFreshness::Released { in_flight: true }
    );

    // 2. The MCP import lands while the run works.
    import_machine_settings_as_mcp_does(&mut controller);
    let freshness = controller.state.simulation_freshness();
    assert!(
        freshness.is_in_flight(),
        "the import does not cancel the run, so a run is still in flight"
    );
    assert_eq!(
        crate::ui::preflight::simulation_detail(&controller.state),
        "Running \u{2014} previous result released",
        "a run in flight reads as running whatever the core holds"
    );

    // 3. The run lands with the stamp from before the import.
    inject_a_run_with_a_trace(&mut controller);
    assert!(
        controller.state.session.simulation_result().is_none(),
        "the core refuses the run: it used the machine from before the import"
    );
    assert!(
        controller.state.simulation.has_results(),
        "the view keeps the run (WP28: a simulation is minutes of work)"
    );
    let freshness = controller.state.simulation_freshness();
    assert_eq!(
        freshness,
        SimFreshness::EditedSince(only_machine()),
        "the stale reading names the machine import, and only it"
    );
    assert!(freshness.is_stale(), "correct staleness must stay");
    assert_eq!(
        controller.state.simulation_stale_reason().as_deref(),
        Some("machine settings changed after this run")
    );
    assert_eq!(
        crate::ui::preflight::simulation_detail(&controller.state),
        "Stale \u{2014} machine settings changed after this run, run again"
    );
    let banner = crate::ui::components::freshness::banner_text(
        &controller
            .state
            .simulation_stale_reason()
            .expect("a stale run has a reason"),
    );
    assert!(
        !banner.contains("params"),
        "the banner must not call a machine edit a parameter change: {banner}"
    );

    // The drain kept the late run's trace, so the cycle time reads the run
    // on screen and not "not simulated".
    assert!(
        controller
            .state
            .simulation
            .results
            .as_ref()
            .is_some_and(|results| results.cut_trace.is_some()),
        "the drain must keep the trace of a run the core refused"
    );
    let cycle = crate::ui::readiness::estimate_total_time(&controller.state);
    assert_eq!(cycle.basis, Some(CycleTimeBasis::MachineModel));
    assert!(
        (cycle.seconds - TRACE_SECONDS).abs() < 1e-9,
        "the cycle time reads the trace on screen: {}",
        cycle.seconds
    );
    assert!(
        !cycle.label().contains("not simulated"),
        "a run is on screen, so the cycle time must not read 'not simulated': {}",
        cycle.label()
    );

    // 4. The operator re-runs. The session holds no run, and the panel
    //    still reads as running.
    controller.handle_internal_event(AppEvent::RunSimulation);
    let freshness = controller.state.simulation_freshness();
    assert_eq!(freshness, SimFreshness::Running);
    assert!(freshness.is_in_flight());
    assert_eq!(
        running_label(&controller.state.simulation),
        "Running \u{2014} previous result released"
    );
    assert_eq!(
        crate::ui::preflight::simulation_detail(&controller.state),
        "Running \u{2014} previous result released"
    );

    // 5. The re-run answers the live epoch, and the core holds it.
    inject_a_run_with_a_trace(&mut controller);
    assert!(controller.state.session.simulation_result().is_some());
    assert_eq!(
        controller.state.simulation_freshness(),
        SimFreshness::Current
    );
}

/// The control: the same import BEFORE the click. The run carries the new
/// machine, so it lands current. Without this arm the sentry could pass on
/// a door that calls every run stale.
#[test]
fn a_machine_import_before_the_run_lands_current_g_machinestale() {
    let mut controller = controller_after_generate_all();
    import_machine_settings_as_mcp_does(&mut controller);
    controller.handle_internal_event(AppEvent::RunSimulation);
    inject_a_run_with_a_trace(&mut controller);
    assert!(controller.state.session.simulation_result().is_some());
    assert_eq!(
        controller.state.simulation_freshness(),
        SimFreshness::Current
    );
    assert_eq!(controller.state.simulation_stale_reason(), None);
}

/// A parameter edit after a run still reads "parameters changed": the
/// cause follows the edit, not the surface.
#[test]
fn a_parameter_edit_during_a_run_names_the_parameters_g_machinestale() {
    let mut controller = controller_after_generate_all();
    let tp_id = controller.state.session.toolpath_configs()[0].id;
    controller.handle_internal_event(AppEvent::RunSimulation);
    panel_edit(&mut controller, tp_id, |entry| {
        entry.operation.set_feed_rate(4321.0);
    });
    inject_a_run_with_a_trace(&mut controller);
    let mut operations = SimulationDropCauses::EMPTY;
    operations.insert(SimulationDropCause::Operations);
    assert_eq!(
        controller.state.simulation_freshness(),
        SimFreshness::EditedSince(operations)
    );
    assert_eq!(
        crate::ui::preflight::simulation_detail(&controller.state),
        "Stale \u{2014} parameters changed after this run, run again"
    );
}
