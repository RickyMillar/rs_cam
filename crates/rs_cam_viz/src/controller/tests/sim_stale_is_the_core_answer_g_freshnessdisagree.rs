//! G-FRESHNESSDISAGREE (W4) — one function answers "where does the
//! simulation stand", and the core owns every project input in it.
//!
//! The ledger row reports two surfaces disagreeing about the same run: the
//! inspector read live while the Optimize window read stale. The structural
//! cause is that thirteen draw sites each built the answer themselves out of
//! `SimulationState::is_stale(gui.edit_counter)`, a project-wide counter that
//! is not the project's truth about a simulation.
//!
//! The counter is wrong in both directions:
//!
//! - It moves for edits the core keeps the simulation through. An export
//!   wizard field and a machine saved to the library both bump it, and
//!   neither changes project data.
//! - It stands still for edits the core clears the simulation on. Nothing
//!   bumps it on `Command::AdoptResult`.
//!
//! [`crate::state::freshness::simulation_freshness`] asks the core instead,
//! and asks the GUI only about the capture options, which are runtime-only
//! and which the core cannot see.
//!
//! # What this file asserts
//!
//! The STATE, one arm per mutation class, not the identity
//! `is_stale == simulation_result().is_none()`. Those two are not equal on a
//! GUI path: the three mirroring doors wipe `last_run` as well as the
//! results, so a mirrored GUI edit leaves NOTHING to draw and reads
//! [`SimFreshness::NoRun`]. A door that does not mirror leaves the view
//! drawing an old trace and reads [`SimFreshness::EditedSince`]. Both are
//! stale readings of the project; only one has a picture behind it.
//!
//! # Non-vacuity
//!
//! Each table must be non-empty, and each arm asserts the state BEFORE the
//! edit as well as after it. A walk that starts stale proves nothing about
//! what the edit did.

use super::*;

use crate::state::freshness::SimFreshness;

// ── fixtures ───────────────────────────────────────────────────────────────

/// A project with one generated toolpath and one landed simulation, which
/// the core accepted. The state an operator is in when they read collision
/// counts off the Simulation workspace.
fn controller_with_a_landed_run() -> AppController<ScriptedBackend> {
    let mut controller = sample_controller();
    generate_all_for_test(&mut controller);
    controller.handle_internal_event(AppEvent::RunSimulation);
    inject_sim_results(&mut controller, 1);
    controller
}

/// One operator edit, and the door it goes through.
struct EditClass {
    /// What the operator did.
    name: &'static str,
    /// Which simulation input it moves.
    input: &'static str,
    /// The route.
    apply: fn(&mut AppController<ScriptedBackend>),
}

// ── the doors that mirror ──────────────────────────────────────────────────

fn edit_a_toolpath_parameter(controller: &mut AppController<ScriptedBackend>) {
    let tp_id = controller.state.session.toolpath_configs()[0].id;
    panel_edit(controller, tp_id, |entry| {
        entry.operation.set_feed_rate(4321.0);
    });
    // The panel raises `PanelSideEffects::invalidate_simulation`; the frame
    // loop discharges it. The controller harness runs no frame loop, so the
    // arm discharges it by hand rather than measure a half-applied edit.
    controller.discharge_panel_side_effects();
}

fn edit_the_tool(controller: &mut AppController<ScriptedBackend>) {
    let mut draft = controller
        .state
        .session
        .tools()
        .first()
        .expect("the fixture project carries one tool")
        .clone();
    draft.stickout += 15.0;
    crate::ui::properties::commit_tool_draft(&mut controller.state, ToolId(1), draft);
    controller.discharge_panel_side_effects();
}

fn edit_the_stock(controller: &mut AppController<ScriptedBackend>) {
    let mut stock = controller.state.session.stock_config().clone();
    stock.z += 5.0;
    crate::ui::properties::apply_stock_draft(&mut controller.state, stock);
    controller.discharge_panel_side_effects();
}

fn add_a_fixture(controller: &mut AppController<ScriptedBackend>) {
    controller.handle_internal_event(AppEvent::AddFixture(SetupId(0)));
}

fn add_a_keep_out_zone(controller: &mut AppController<ScriptedBackend>) {
    controller.handle_internal_event(AppEvent::AddKeepOut(SetupId(0)));
}

fn toggle_the_operation_off(controller: &mut AppController<ScriptedBackend>) {
    controller.handle_internal_event(AppEvent::ToggleToolpathEnabled(ToolpathId(0)));
}

fn reorder_the_operations(controller: &mut AppController<ScriptedBackend>) {
    // The fixture carries one operation, so the arm adds the second it
    // needs. The add itself clears the simulation, which is why the
    // assertion runs over the state this function LEAVES and not over the
    // reorder alone.
    controller.handle_internal_event(AppEvent::AddToolpath(
        crate::state::toolpath::OperationType::Pocket,
    ));
    let ids: Vec<ToolpathId> = controller
        .state
        .session
        .toolpath_configs()
        .iter()
        .map(|tc| tc.id)
        .collect();
    let last = *ids.last().expect("the add row created an operation");
    controller.handle_internal_event(AppEvent::MoveToolpathUp(last));
}

fn mirroring_doors() -> Vec<EditClass> {
    vec![
        EditClass {
            name: "a toolpath parameter",
            input: "the motion the run cut",
            apply: edit_a_toolpath_parameter,
        },
        EditClass {
            name: "the tool",
            input: "the cutter geometry and the holder assembly",
            apply: edit_the_tool,
        },
        EditClass {
            name: "the stock",
            input: "the material the run removed",
            apply: edit_the_stock,
        },
        EditClass {
            name: "a fixture added",
            input: "the obstacle set",
            apply: add_a_fixture,
        },
        EditClass {
            name: "a keep-out zone added",
            input: "the obstacle set",
            apply: add_a_keep_out_zone,
        },
        EditClass {
            name: "an operation switched off",
            input: "which motion the job contains",
            apply: toggle_the_operation_off,
        },
        EditClass {
            name: "the operations reordered",
            input: "the order the material comes off in",
            apply: reorder_the_operations,
        },
    ]
}

// ── the doors that do not mirror ───────────────────────────────────────────
//
// A direct `session.apply` is what the MCP surface does today
// (`app/mcp.rs:361`, G-MCPSIMMIRROR / WP28). The core clears the field and
// the view keeps its trace, so the reading is `EditedSince`. W4 does not
// close that door; it pins what the door gives.

fn set_the_setup_face(controller: &mut AppController<ScriptedBackend>) {
    let _ = controller
        .state
        .session
        .apply(Command::SetSetupFace(SetSetupFaceArgs {
            setup_index: 0,
            face_up: rs_cam_core::compute::transform::FaceUp::Bottom,
        }));
}

fn set_the_setup_rotation(controller: &mut AppController<ScriptedBackend>) {
    let _ = controller
        .state
        .session
        .apply(Command::SetSetupRotation(SetSetupRotationArgs {
            setup_index: 0,
            z_rotation: rs_cam_core::compute::transform::ZRotation::Deg90,
        }));
}

fn set_the_machine(controller: &mut AppController<ScriptedBackend>) {
    let machine = controller.state.session.machine().clone();
    let _ =
        controller
            .state
            .session
            .apply(Command::SetMachine(rs_cam_core::session::SetMachineArgs {
                machine: Box::new(machine),
            }));
}

fn unmirrored_doors() -> Vec<EditClass> {
    vec![
        EditClass {
            name: "the setup face",
            input: "the frame every operation is generated in",
            apply: set_the_setup_face,
        },
        EditClass {
            name: "the setup rotation",
            input: "the frame every operation is generated in",
            apply: set_the_setup_rotation,
        },
        EditClass {
            name: "the machine profile",
            input: "the rapid rate and the travel limits the run modelled",
            apply: set_the_machine,
        },
    ]
}

// ── the contract ───────────────────────────────────────────────────────────

/// A mirrored GUI edit leaves NOTHING on screen, so the state is `NoRun`.
///
/// This is the arm that says why the sentry cannot assert
/// `is_stale == simulation_result().is_none()`. The core answer is "no
/// simulation" for every class here, and the view answer is "nothing to
/// draw" — one reading, not two.
#[test]
fn a_mirrored_edit_leaves_no_run_g_freshnessdisagree() {
    let classes = mirroring_doors();
    assert!(
        !classes.is_empty(),
        "an empty walk passes and proves nothing"
    );

    for class in &classes {
        let mut controller = controller_with_a_landed_run();
        assert_eq!(
            controller.state.simulation_freshness(),
            SimFreshness::Current,
            "before the '{}' edit: a landed run the core accepted is current \
             evidence, or this arm measures nothing",
            class.name
        );

        (class.apply)(&mut controller);

        assert!(
            controller.state.session.simulation_result().is_none(),
            "the '{}' edit moves {} and the core kept the simulation — the \
             class is NOT COVERED by the core's own invalidation",
            class.name,
            class.input
        );
        assert_eq!(
            controller.state.simulation_freshness(),
            SimFreshness::NoRun,
            "after the '{}' edit: the door mirrors the drop into the view, so \
             there is no trace left to qualify",
            class.name
        );
    }
}

/// A door that does not mirror leaves the view drawing an old trace, so the
/// state is `EditedSince` and never `Current`.
#[test]
fn an_unmirrored_edit_leaves_edited_since_g_freshnessdisagree() {
    let classes = unmirrored_doors();
    assert!(
        !classes.is_empty(),
        "an empty walk passes and proves nothing"
    );

    for class in &classes {
        let mut controller = controller_with_a_landed_run();
        assert_eq!(
            controller.state.simulation_freshness(),
            SimFreshness::Current,
            "before the '{}' edit: the run must start current",
            class.name
        );

        (class.apply)(&mut controller);

        assert!(
            controller.state.session.simulation_result().is_none(),
            "the '{}' edit moves {} and the core kept the simulation",
            class.name,
            class.input
        );
        assert!(
            controller.state.simulation.has_results(),
            "the '{}' edit went through no mirroring door, so the view still \
             holds the trace — that is what makes this arm EditedSince and \
             not NoRun",
            class.name
        );
        assert_eq!(
            controller.state.simulation_freshness(),
            SimFreshness::EditedSince,
            "after the '{}' edit: the operator is looking at a picture of \
             material the project no longer cuts",
            class.name
        );
        assert!(
            controller.state.simulation_is_stale(),
            "'{}': every EditedSince reading is stale evidence",
            class.name
        );
    }
}

/// A capture-option change keeps the core's simulation and still needs a
/// re-run. This arm proves the GUI half of the answer is still needed.
#[test]
fn a_capture_option_change_reads_capture_options_changed_g_freshnessdisagree() {
    let mut controller = controller_with_a_landed_run();
    assert_eq!(
        controller.state.simulation_freshness(),
        SimFreshness::Current
    );

    let enabled = controller.state.simulation.metric_options.enabled;
    controller
        .state
        .simulation
        .set_metric_capture_enabled(!enabled);

    assert!(
        controller.state.session.simulation_result().is_some(),
        "capture options are runtime-only: the core cannot see them, and it \
         must not drop a run over one"
    );
    assert_eq!(
        controller.state.simulation_freshness(),
        SimFreshness::CaptureOptionsChanged,
        "the geometry answer stands and the metric answer does not, so the \
         state names which"
    );
    assert!(controller.state.simulation_is_stale());
}

/// D7, G-LATESIM. An edit lands while the run works; the result arrives
/// behind it. The core refuses the adopt, the view keeps the trace, and no
/// surface may call it current.
#[test]
fn a_late_run_reads_edited_since_and_never_current_g_freshnessdisagree() {
    let mut controller = sample_controller();
    generate_all_for_test(&mut controller);
    let tp_id = controller.state.session.toolpath_configs()[0].id;

    controller.handle_internal_event(AppEvent::RunSimulation);
    assert_eq!(
        controller.state.simulation_freshness(),
        SimFreshness::Running,
        "the submit stamps the epoch, and the stamp is what says a run is in \
         flight"
    );

    // The operator changes a parameter while it runs. `panel_edit` alone,
    // deliberately: it leaves the submit stamp alone, which is what makes
    // the result late rather than cancelled.
    panel_edit(&mut controller, tp_id, |entry| {
        entry.operation.set_feed_rate(4321.0);
    });

    inject_sim_results(&mut controller, 1);

    assert!(
        controller.state.simulation.has_results(),
        "STORED, not discarded: a simulation is minutes of work"
    );
    assert!(
        controller.state.session.simulation_result().is_none(),
        "the core refused the adopt: the epoch the submit stamped has moved"
    );
    assert_eq!(
        controller.state.simulation_freshness(),
        SimFreshness::EditedSince,
        "stored, and marked not-current"
    );
}

/// The states with no result behind them, and the one with a result the
/// core stands behind. Without these three the predicate could answer
/// stale to everything and pass every arm above.
#[test]
fn the_states_with_no_evidence_are_not_stale_g_freshnessdisagree() {
    let controller = sample_controller();
    assert_eq!(
        controller.state.simulation_freshness(),
        SimFreshness::NoRun,
        "a project that has never been simulated has no evidence to qualify"
    );
    assert!(!controller.state.simulation_is_stale());

    let mut controller = sample_controller();
    generate_all_for_test(&mut controller);
    controller.handle_internal_event(AppEvent::RunSimulation);
    assert_eq!(
        controller.state.simulation_freshness(),
        SimFreshness::Running
    );
    assert!(
        !controller.state.simulation_is_stale(),
        "a run in flight is not stale evidence: it is not evidence yet"
    );

    inject_sim_results(&mut controller, 1);
    assert_eq!(
        controller.state.simulation_freshness(),
        SimFreshness::Current
    );
    assert!(!controller.state.simulation_is_stale());
}

/// Class (iii): the edit counter moves and the core keeps the simulation.
///
/// `GuiState::mark_edited` is what the export wizard rows and the
/// "save machine to library" button call. Neither writes project data, and
/// the old `is_stale` read both as a stale simulation. The core answer says
/// the run still stands. G-DIRTYONLOAD owns the dirty flag half of this; the
/// simulation half is closed here.
#[test]
fn a_counter_bump_alone_does_not_stale_the_run_g_freshnessdisagree() {
    let mut controller = controller_with_a_landed_run();
    controller.state.gui.mark_edited();

    assert!(
        controller.state.session.simulation_result().is_some(),
        "nothing in the project moved, so the core keeps the run"
    );
    assert_eq!(
        controller.state.simulation_freshness(),
        SimFreshness::Current,
        "a GUI-only bump is not evidence that the simulation answers a \
         configuration the operator has left"
    );
}
