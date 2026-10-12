//! Undo: every arm marks the project edited, and what an undo stales.

use super::*;

/// THE rule, over every arm. A per-arm table lives in the report; this is
/// the executable half of it.
///
/// Pre-fix NONE of the five arms marked the project edited, so `dirty` stayed
/// false — an undone project was not offered for saving — and `edit_counter`
/// never moved, so the GUI went on presenting a simulation computed from the
/// configuration the undo had just discarded, with no staleness anywhere.
#[test]
fn every_undo_arm_marks_the_project_edited_g_undofresh() {
    use crate::state::history::UndoAction;

    /// One undo arm: a label and a builder for the action the operator's
    /// edit would have pushed.
    type ArmBuilder = Box<dyn Fn(&AppController<ScriptedBackend>) -> UndoAction>;

    let arms: Vec<(&str, ArmBuilder)> = vec![
        (
            "StockChange",
            Box::new(
                |c: &AppController<ScriptedBackend>| UndoAction::StockChange {
                    old: c.state.session.stock_config().clone(),
                    new: c.state.session.stock_config().clone(),
                },
            ),
        ),
        (
            "PostChange",
            Box::new(
                |c: &AppController<ScriptedBackend>| UndoAction::PostChange {
                    old: c.state.gui.post.clone(),
                    new: c.state.gui.post.clone(),
                },
            ),
        ),
        (
            "ToolChange",
            Box::new(
                |c: &AppController<ScriptedBackend>| UndoAction::ToolChange {
                    tool_id: c.state.session.tools()[0].id,
                    old: c.state.session.tools()[0].clone(),
                    new: c.state.session.tools()[0].clone(),
                },
            ),
        ),
        (
            "ToolpathParamChange",
            Box::new(|c: &AppController<ScriptedBackend>| {
                let tc = &c.state.session.toolpath_configs()[0];
                UndoAction::ToolpathParamChange {
                    tp_id: tc.id,
                    old_op: tc.operation.clone(),
                    new_op: tc.operation.clone(),
                    old_dressups: tc.dressups.clone(),
                    new_dressups: tc.dressups.clone(),
                    old_face_selection: None,
                    new_face_selection: None,
                    old_feeds_provenance: tc.feeds_provenance.clone(),
                    new_feeds_provenance: tc.feeds_provenance.clone(),
                }
            }),
        ),
        (
            "MachineChange",
            Box::new(
                |c: &AppController<ScriptedBackend>| UndoAction::MachineChange {
                    old: c.state.session.machine().clone(),
                    new: c.state.session.machine().clone(),
                },
            ),
        ),
    ];

    for (name, build) in arms {
        // Undo.
        let (mut controller, _) = controller_ready_for_undo();
        let action = build(&controller);
        controller.state.history.push(action.clone());
        controller.state.gui.dirty = false;
        let before = controller.state.gui.edit_counter;

        controller.handle_internal_event(AppEvent::Undo);

        assert!(
            controller.state.gui.dirty,
            "{name}: undo must leave the project unsaved, or the close \
             interception lets the operator walk away from it"
        );
        assert!(
            controller.state.gui.edit_counter > before,
            "{name}: undo must move the edit counter, or the simulation goes \
             on calling itself fresh"
        );
        // One property, whatever the treatment. Every arm leaves the
        // project where the same edit made by hand would. WP19 (plan §28)
        // made that one rule: a session that drops its simulation leaves
        // no viewport showing one, for a parameter edit as for a stock or
        // machine change. Either way the operator must not be shown the
        // old run as current evidence, and that is what is asserted:
        // cleared or stale, never present-and-fresh.
        let sim = &controller.state.simulation;
        assert!(
            !sim.has_results() || controller.state.simulation_is_stale(),
            "{name}: the simulation was computed from the configuration this \
             undo just discarded, and is still being presented as fresh"
        );

        // Redo — the row does not name it; it carries the same defect and
        // takes the same fix, so it is asserted the same way.
        let (mut controller, _) = controller_ready_for_undo();
        controller.state.history.push(action);
        controller.handle_internal_event(AppEvent::Undo);
        controller.state.gui.dirty = false;
        let before = controller.state.gui.edit_counter;

        controller.handle_internal_event(AppEvent::Redo);

        assert!(
            controller.state.gui.dirty,
            "{name}: redo is the same edit in the other direction"
        );
        assert!(
            controller.state.gui.edit_counter > before,
            "{name}: redo must move the edit counter too"
        );
    }
}

/// The row's first clause. A tool undo must invalidate the operations that
/// tool machines, exactly as `commit_tool_draft` does — the undo arms used to
/// write `tools_mut()` directly and clear only the simulation, so every
/// dependent kept a CORE result generated with the other tool's geometry and
/// read `Current` on every surface.
#[test]
fn a_tool_undo_stales_the_operations_that_tool_machines_g_undofresh() {
    use crate::state::history::UndoAction;

    let (mut controller, tp_id) = controller_ready_for_undo();
    let tool_id = controller.state.session.tools()[0].id;
    let original = controller.state.session.tools()[0].clone();
    let mut widened = original.clone();
    widened.diameter += 3.0;

    // The hand edit, through the door the panel uses.
    crate::ui::properties::commit_tool_draft(&mut controller.state, tool_id, widened);
    assert_eq!(
        state_of(&controller, 0),
        FreshnessState::EditedSince,
        "the hand edit stales its users (F2.1) — that is the behaviour undo has to match"
    );

    // Regenerate so the undo has something to invalidate.
    generate_all_for_test(&mut controller);
    assert_eq!(state_of(&controller, 0), FreshnessState::Current);
    controller.state.gui.dirty = false;

    controller.handle_internal_event(AppEvent::Undo);

    assert_eq!(
        controller.state.session.tools()[0].diameter,
        original.diameter,
        "the undo restores the tool"
    );
    assert_eq!(
        state_of(&controller, 0),
        FreshnessState::EditedSince,
        "and every operation it machines is no longer current — the stored \
         result was generated with the OTHER tool's geometry"
    );
    assert!(
        controller.state.session.get_result(0).is_none(),
        "which means the core result is gone, not merely flagged"
    );
    assert!(
        controller.state.gui.toolpath_rt[&tp_id]
            .stale_since
            .is_some(),
        "and a regeneration is requested, as commit_tool_draft does — the \
         operator did not ask these operations to be invalidated"
    );
    // Sanity: the undo really did go through the shared door rather than a
    // second one that happens to agree today.
    assert!(
        matches!(
            controller.state.history.undo(),
            None | Some(UndoAction::ToolChange { .. })
        ),
        "the history holds what we think it holds"
    );
}

/// The decision of §3, pinned: an undo that restores the exact configuration
/// the retained geometry was generated from still reads `EditedSince`.
///
/// This is the one assertion in F2.5 that could reasonably have gone the
/// other way, so it is asserted explicitly rather than left as a consequence.
/// Reversing it must be a decision someone makes, not a side effect.
#[test]
fn an_undone_param_edit_does_not_resurrect_the_old_result() {
    let (mut controller, tp_id) = controller_ready_for_undo();
    let before = controller.state.session.toolpath_configs()[0]
        .operation
        .clone();

    // A parameter edit with an undo entry behind it, as the panel pushes one.
    controller
        .state
        .history
        .push(crate::state::history::UndoAction::ToolpathParamChange {
            tp_id,
            old_op: before.clone(),
            new_op: {
                let mut op = before.clone();
                op.set_feed_rate(4321.0);
                op
            },
            old_dressups: Default::default(),
            new_dressups: Default::default(),
            old_face_selection: None,
            new_face_selection: None,
            old_feeds_provenance: Default::default(),
            new_feeds_provenance: Default::default(),
        });
    panel_edit(&mut controller, tp_id, |entry| {
        entry.operation.set_feed_rate(4321.0);
    });
    assert_eq!(state_of(&controller, 0), FreshnessState::EditedSince);

    controller.handle_internal_event(AppEvent::Undo);

    assert_eq!(
        controller.state.session.toolpath_configs()[0]
            .operation
            .feed_rate(),
        before.feed_rate(),
        "the undo restores the configuration the retained geometry answers"
    );
    assert_eq!(
        state_of(&controller, 0),
        FreshnessState::EditedSince,
        "and it STILL reads EditedSince. Nothing records which configuration \
         `rt.result` answers; the snapshot restores three of the nine fields \
         that decide the geometry; and `toolpath_revision` only moves forward. \
         Being wrong this way costs a regeneration — the other way exports a \
         program that does not match the project. See reports/F2.5.md §3."
    );
}

/// WP8 / N14 — an undo stales the whole chain, and it restores the feeds
/// provenance the edit stamped.
///
/// Two defects in one test, both of the same shape: the undo door knew
/// about ONE toolpath and ONE set of fields.
///
/// - The core call dropped the edited row's result alone, and the GUI
///   stamped `stale_since` on that row alone. A downstream
///   `FromRemainingStock` operation kept a result generated against
///   stock that had moved, read `Current` on every surface, and the
///   auto-regen sweep never queued it.
/// - The undo entry carried no `feeds_provenance`. An apply path that
///   wrote the provenance in a second call right after the snapshot let
///   an undo put the earlier numbers back and leave the later stamp
///   standing on them.
#[test]
fn an_undo_stales_the_downstream_row_and_restores_the_provenance_n14() {
    let (mut controller, tp_id) = controller_ready_for_undo();
    let rest_id = push_toolpath(&mut controller, "Rest");
    if let Some((idx, _)) = controller.state.session.find_toolpath_config_by_id(rest_id) {
        let _ = controller
            .state
            .session
            .apply(Command::SetStockSource(SetStockSourceArgs {
                index: idx,
                source: rs_cam_core::compute::config::StockSource::FromRemainingStock,
            }))
            .expect("index is in range");
    }

    let before_op = controller.state.session.toolpath_configs()[0]
        .operation
        .clone();
    let before_provenance = controller.state.session.toolpath_configs()[0]
        .feeds_provenance
        .clone();
    let after_provenance = rs_cam_core::feeds::FeedsProvenance {
        feed_rate: Some(rs_cam_core::feeds::ValueProvenance::manual()),
        ..Default::default()
    };
    let mut after_op = before_op.clone();
    after_op.set_feed_rate(4321.0);
    assert_ne!(
        before_provenance, after_provenance,
        "the two stamps must differ, or the restore assertion is vacuous"
    );

    // The edit, with the entry the panel pushes for it.
    panel_edit(&mut controller, tp_id, |entry| {
        entry.operation.set_feed_rate(4321.0);
    });
    let _ = controller
        .state
        .session
        .apply(Command::SetFeedsProvenance(SetFeedsProvenanceArgs {
            index: 0,
            feeds_provenance: Box::new(after_provenance.clone()),
        }))
        .expect("index 0 exists");
    controller
        .state
        .history
        .push(crate::state::history::UndoAction::ToolpathParamChange {
            tp_id,
            old_op: before_op.clone(),
            new_op: after_op,
            old_dressups: Default::default(),
            new_dressups: Default::default(),
            old_face_selection: None,
            new_face_selection: None,
            old_feeds_provenance: before_provenance.clone(),
            new_feeds_provenance: after_provenance.clone(),
        });

    // Regenerate, so the undo has something to invalidate on BOTH rows.
    generate_all_for_test(&mut controller);
    assert!(
        controller.state.session.get_result(1).is_some()
            && controller.state.gui.toolpath_rt[&rest_id]
                .stale_since
                .is_none(),
        "the downstream row starts fresh, or the assertions below are \
         vacuous"
    );

    controller.handle_internal_event(AppEvent::Undo);

    assert_eq!(
        controller.state.session.toolpath_configs()[0]
            .operation
            .feed_rate(),
        before_op.feed_rate(),
        "the undo restores the operation"
    );
    assert_eq!(
        controller.state.session.toolpath_configs()[0].feeds_provenance,
        before_provenance,
        "and the provenance the edit stamped"
    );
    assert!(
        controller.state.session.get_result(1).is_none(),
        "the downstream row reads the stock index 0 leaves, so its \
         result goes with the undo"
    );
    assert!(
        controller.state.gui.toolpath_rt[&rest_id]
            .stale_since
            .is_some(),
        "and the GUI stamps every index in Effects::stale, not the \
         edited one alone. The auto-regen sweep reads stale_since."
    );

    controller.handle_internal_event(AppEvent::Redo);

    assert_eq!(
        controller.state.session.toolpath_configs()[0].feeds_provenance,
        after_provenance,
        "redo is the same edit in the other direction, so it carries the \
         provenance too"
    );
}

/// A machine-kinematics undo leaves the toolpaths current. F2.1's operator
/// ruling (R0.1 §7 Q3) is that kinematics change timing and modulation, not
/// geometry — so this arm must NOT acquire staleness it does not deserve
/// just because the other arms did.
#[test]
fn a_machine_undo_leaves_the_toolpaths_current_g_undofresh() {
    let (mut controller, _) = controller_ready_for_undo();
    let machine = controller.state.session.machine().clone();
    controller
        .state
        .history
        .push(crate::state::history::UndoAction::MachineChange {
            old: machine.clone(),
            new: machine,
        });

    controller.handle_internal_event(AppEvent::Undo);

    assert_eq!(
        state_of(&controller, 0),
        FreshnessState::Current,
        "kinematics do not move geometry; only the simulation goes"
    );
    assert!(
        controller.state.gui.dirty,
        "but the project is still edited"
    );
}

// ── F2.10 / G-LATESIM — the simulation arm of the late-arrival race ──────
//
// F2.4 closed the toolpath arm: a generation result that answers a
// superseded configuration is stored but not made current. This is the same
// defect on the simulation, and it matters at least as much — an operator
// makes machining decisions off collision counts, engagement and air-cut, so
// a run that answers a discarded configuration, presented as current
// evidence, is the export defect on the surface people trust to tell them it
// is safe to cut.
//
// The run used to be recorded against the live `edit_counter` in the DRAIN,
// which folded every edit made while the simulation ran into the record of
// when it was run, and the staleness read said `false`.
//
// W4 moved the record to `ProjectSession::simulation_epoch`, stamped at
// SUBMIT. The epoch is project-wide and that is deliberate — see
// `reports/F2.10.md` §3: the in-flight window now behaves like the window
// either side of it.

/// THE race. An edit lands while the simulation runs; the result that
/// arrives answers the configuration the operator has already left.
///
/// Pre-fix the drain stamped the live counter, so `is_stale` read `false` and
/// every surface — the Simulation tab chip, `readiness::simulation_check`,
/// the Readiness panel's `FreshnessGate` — presented the run as current.
#[test]
fn an_edit_during_a_simulation_leaves_the_result_stale_g_latesim() {
    let mut controller = sample_controller();
    generate_all_for_test(&mut controller);
    let tp_id = controller.state.session.toolpath_configs()[0].id;

    // Submit the run. The stamp is taken here, not when the result lands.
    controller.handle_internal_event(AppEvent::RunSimulation);
    let submitted_at = controller.state.simulation.submitted_edit_counter;
    assert!(
        submitted_at.is_some(),
        "the submit must record the configuration this run answers"
    );

    // The operator changes a parameter while it runs.
    panel_edit(&mut controller, tp_id, |entry| {
        entry.operation.set_feed_rate(4321.0);
    });
    let after_edit = controller.state.gui.edit_counter;
    assert!(
        Some(after_edit) != submitted_at,
        "the edit must move the counter, or this test proves nothing"
    );

    // The result lands.
    inject_sim_results(&mut controller, 1);

    assert!(
        controller.state.simulation.has_results(),
        "STORED, not discarded: a simulation is minutes of work, and throwing \
         it away leaves the operator unable to tell a cancelled run from one \
         that never happened"
    );
    assert!(
        controller.state.simulation_is_stale(),
        "but NOT current — this run measured the configuration the operator \
         has already left, and it is the surface they read collision counts \
         off"
    );
    assert!(
        controller.state.session.simulation_result().is_none(),
        "and the CORE refused the adopt: the epoch this run was submitted \
         with has moved, so the session holds no simulation at all"
    );
}

/// The other side: with no edit in flight, the result is current. A guard
/// that staled every simulation would pass the test above and make the
/// Simulation workspace useless.
#[test]
fn a_simulation_with_no_edit_in_flight_is_current_g_latesim() {
    let mut controller = sample_controller();
    generate_all_for_test(&mut controller);

    controller.handle_internal_event(AppEvent::RunSimulation);
    inject_sim_results(&mut controller, 1);

    assert!(controller.state.simulation.has_results());
    assert!(
        !controller.state.simulation_is_stale(),
        "an unedited run is current evidence"
    );
}

/// The stamp is consumed by the run it belongs to. A second result arriving
/// with no submit behind it falls back to the live counter — the pre-F2.10
/// behaviour, which is not a claim this guard can make either way.
#[test]
fn the_submit_stamp_belongs_to_one_run_g_latesim() {
    let mut controller = sample_controller();
    generate_all_for_test(&mut controller);

    controller.handle_internal_event(AppEvent::RunSimulation);
    assert!(controller.state.simulation.submitted_edit_counter.is_some());
    inject_sim_results(&mut controller, 1);
    assert!(
        controller.state.simulation.submitted_edit_counter.is_none(),
        "the stamp is taken by the result it describes, so it cannot be \
         re-used by a later one that had no submit of its own"
    );
}

// ── P4: a pin edit and its undo stale only what reads the pins ───────────
//
// The stock panel's pin edits and their undo take
// `Command::SetAlignmentPins`, which stales only what reads the pins
// (operator ruling 2026-10-12). Before P4 the panel applied the whole
// stock through `SetStockConfig`, and every pin edit dropped every result.

/// A controller whose stock is fixed (no `auto_from_model` refit) and
/// whose toolpaths all carry a result.
fn generated_with_fixed_stock() -> AppController<ScriptedBackend> {
    let mut controller = sample_controller();
    let mut draft = controller.state.session.stock_config().clone();
    draft.auto_from_model = false;
    crate::ui::properties::apply_stock_draft(&mut controller.state, draft);
    generate_all_for_test(&mut controller);
    let count = controller.state.session.toolpath_configs().len();
    assert!(count > 0, "non-vacuity: the fixture has toolpaths");
    for index in 0..count {
        assert!(
            controller.state.session.get_result(index).is_some(),
            "the fixture generated toolpath {index}"
        );
    }
    controller
}

fn all_results_present(controller: &AppController<ScriptedBackend>) -> bool {
    (0..controller.state.session.toolpath_configs().len())
        .all(|index| controller.state.session.get_result(index).is_some())
}

#[test]
fn a_panel_pin_edit_and_its_undo_leave_the_other_results_current() {
    let mut controller = generated_with_fixed_stock();
    let old = controller.state.session.stock_config().clone();

    // The panel's draft moves the pins and nothing else.
    let mut draft = old.clone();
    draft.alignment_pins = vec![
        crate::state::job::AlignmentPin::new(10.0, 20.0, 6.0),
        crate::state::job::AlignmentPin::new(30.0, 20.0, 8.0),
    ];
    let command = crate::ui::properties::stock_edit_command(&old, draft.clone());
    assert!(
        matches!(command, Some(Command::SetAlignmentPins(_))),
        "a pin-only draft is a SetAlignmentPins, got {command:?}"
    );
    crate::ui::properties::apply_stock_draft(&mut controller.state, draft.clone());
    assert_eq!(
        controller.state.session.stock_config().alignment_pins,
        draft.alignment_pins
    );
    assert!(
        all_results_present(&controller),
        "no pin-drill operation exists, so a pin edit stales nothing"
    );
    assert!(controller.state.panel_side_effects.pin_drill_sync);

    // One undo step puts the old set back, through the same rule.
    controller
        .state
        .history
        .push(crate::state::history::UndoAction::StockChange {
            old: old.clone(),
            new: controller.state.session.stock_config().clone(),
        });
    controller.undo();
    assert_eq!(
        controller.state.session.stock_config().alignment_pins,
        old.alignment_pins
    );
    assert!(
        all_results_present(&controller),
        "the undo of a pin edit took the narrow rule too"
    );

    // Control: any other stock edit is still SetStockConfig.
    let mut wider = old.clone();
    wider.x += 10.0;
    assert!(matches!(
        crate::ui::properties::stock_edit_command(&old, wider),
        Some(Command::SetStockConfig(_))
    ));
    assert!(crate::ui::properties::stock_edit_command(&old, old.clone()).is_none());
}

// ── P5 / P8: a pin edit takes one door ────────────────────────────────
//
// P5 and P8 (C7, C9): a pin edit takes one door.
//
// Before P5 the GUI ran the pin-drill synchronisation after a pin edit
// and MCP did not. An MCP `add_alignment_pin` then left no "Pin Drill"
// toolpath, and a GUI add left one. The door is
// `AppController::finish_pin_edit`. The MCP describe step calls it, and
// the stock panel reaches the same synchronisation through
// `pin_drill_sync`.
//
// `RsCamApp` needs an `eframe::CreationContext`, so no test drives the
// MCP dispatch. This module drives the MCP route at its two steps: the
// dispatch's `session.apply(command)`, then the door that the describe
// step calls. A source scan pins that the describe step calls that door.
//
// The fixture puts a `Bottom` setup at index 0 and a `Top` setup at
// index 1, so the C7 rule (the first `Top` setup at 0 deg, not index 0) is not
// vacuous.
mod pin_edit_one_door_p5 {
    use super::super::*;

    use rs_cam_core::compute::transform::{FaceUp, ZRotation};
    use rs_cam_core::session::{AddAlignmentPinArgs, SetAlignmentPinsArgs};

    use crate::controller::PinDrillSync;
    use crate::controller::events::pins::{
        PIN_DRILL_NEEDS_TOP_SETUP, PinEdit, pin_drill_needs_tool,
    };
    use crate::state::job::AlignmentPin;

    /// Add a 6 mm drill and a drill operation with it to `setup_index`, so
    /// the project names the tool that drills the 6 mm pin holes (P6, R1:
    /// the door reads `pin_drill_tool_for`, never `tools[0]`).
    fn add_drill_op(controller: &mut AppController<ScriptedBackend>, setup_index: usize) {
        let mut drill =
            rs_cam_core::compute::tool_config::ToolConfig::new_default(ToolId(0), ToolType::VBit);
        drill.diameter = 6.0;
        let created = controller
            .state
            .session
            .apply(Command::AddTool(rs_cam_core::session::AddToolArgs {
                tool: Box::new(drill),
            }))
            .expect("a tool can be added")
            .created
            .expect("the add reports the index");
        let drill_id = controller.state.session.tools()[created].id.0;
        let session = &controller.state.session;
        let mut config = session
            .toolpath_configs()
            .first()
            .cloned()
            .expect("the fixture has a first toolpath");
        let next = session
            .toolpath_configs()
            .iter()
            .map(|tc| tc.id.0)
            .max()
            .map_or(0, |m| m + 1);
        config.id = ToolpathId(next);
        config.name = "Dowel holes".to_owned();
        config.tool_id = drill_id;
        config.operation =
            OperationConfig::Drill(rs_cam_core::compute::operation_configs::DrillConfig::default());
        let _ = controller
            .state
            .session
            .apply(Command::AddToolpath(
                rs_cam_core::session::AddToolpathArgs {
                    setup_index,
                    config: Box::new(config),
                },
            ))
            .expect("the setup accepts a toolpath");
    }

    /// A controller with a Bottom setup at index 0 and a Top setup at index 1.
    fn flipped_first_controller() -> AppController<ScriptedBackend> {
        let mut controller = sample_controller();
        let mut draft = controller.state.session.stock_config().clone();
        draft.auto_from_model = false;
        crate::ui::properties::apply_stock_draft(&mut controller.state, draft);
        let _ = controller
            .state
            .session
            .apply(Command::SetSetupFace(SetSetupFaceArgs {
                setup_index: 0,
                face_up: FaceUp::Bottom,
            }))
            .expect("setup 0 exists");
        let _ = controller
            .state
            .session
            .apply(Command::AddSetup(AddSetupArgs {
                name: Some("Top".to_owned()),
                face_up: FaceUp::Top,
            }))
            .expect("a setup can be added");
        add_drill_op(&mut controller, 0);
        controller
    }

    /// What a toolpath list says, by row: the operation, the setup index and
    /// the pin-drill holes.
    fn toolpath_list(
        controller: &AppController<ScriptedBackend>,
    ) -> Vec<(String, Option<usize>, Vec<[f64; 2]>)> {
        let session = &controller.state.session;
        session
            .toolpath_configs()
            .iter()
            .enumerate()
            .map(|(index, tc)| {
                let setup = session
                    .list_setups()
                    .iter()
                    .position(|s| s.toolpath_indices.contains(&index));
                let holes = match &tc.operation {
                    OperationConfig::AlignmentPinDrill(c) => c.holes.clone(),
                    _ => Vec::new(),
                };
                (format!("{:?}", tc.operation.op_type()), setup, holes)
            })
            .collect()
    }

    /// The GUI route: the stock panel's draft, then the frame loop's discharge.
    fn gui_add_pin(controller: &mut AppController<ScriptedBackend>, pin: AlignmentPin) {
        let mut draft = controller.state.session.stock_config().clone();
        draft.alignment_pins.push(pin);
        crate::ui::properties::apply_stock_draft(&mut controller.state, draft);
        controller.discharge_panel_side_effects();
    }

    /// The MCP route: the dispatch applies the row, the describe step runs the
    /// door.
    fn mcp_add_pin(controller: &mut AppController<ScriptedBackend>, pin: &AlignmentPin) -> PinEdit {
        let effects = controller
            .state
            .session
            .apply(Command::AddAlignmentPin(AddAlignmentPinArgs {
                x: pin.x,
                y: pin.y,
                diameter: pin.diameter,
            }))
            .expect("an add is never refused");
        controller.finish_pin_edit(&effects)
    }

    #[test]
    fn an_mcp_add_and_a_gui_add_leave_the_same_toolpath_list_p5() {
        let pin = AlignmentPin::new(10.0, 20.0, 6.0);

        let mut gui = flipped_first_controller();
        let mut mcp = flipped_first_controller();
        let before = toolpath_list(&gui);
        assert_eq!(before, toolpath_list(&mcp), "the two fixtures start equal");

        gui_add_pin(&mut gui, pin.clone());
        let edit = mcp_add_pin(&mut mcp, &pin);

        let after = toolpath_list(&gui);
        assert_eq!(
            after.len(),
            before.len() + 1,
            "non-vacuity: the GUI add created the pin-drill toolpath"
        );
        assert_eq!(
            toolpath_list(&mcp),
            after,
            "an MCP add and a GUI add must leave the same toolpath list"
        );
        assert!(
            matches!(
                edit.outcome_of(6.0),
                Some(PinDrillSync::Created { setup_index: 1, .. })
            ),
            "the MCP door created the pin drill in setup 1, got {:?}",
            edit.pin_drill
        );
        let (label, setup, holes) = after.last().expect("the pin drill").clone();
        assert_eq!(label, "AlignmentPinDrill");
        assert_eq!(
            setup,
            Some(1),
            "C7: the pin drill goes in the first Top setup, not in setup index 0"
        );
        assert_eq!(holes, vec![[pin.x, pin.y]]);

        // A second edit moves the holes the same way on both routes.
        let moved = AlignmentPin::new(30.0, 20.0, 6.0);
        gui_add_pin(&mut gui, moved.clone());
        let edit = mcp_add_pin(&mut mcp, &moved);
        assert_eq!(toolpath_list(&mcp), toolpath_list(&gui));
        let drill_index = toolpath_list(&mcp).len() - 1;
        assert!(
            matches!(edit.outcome_of(6.0), Some(PinDrillSync::Updated { .. })),
            "{:?}",
            edit.pin_drill
        );
        assert!(
            edit.stale.contains(&drill_index),
            "the reply's stale set names the pin drill: {:?}",
            edit.stale
        );
        assert!(
            !edit.stale.contains(&0),
            "the pin edit leaves the other toolpath current: {:?}",
            edit.stale
        );

        // Clearing the set removes the pin drill on both routes.
        let mut draft = gui.state.session.stock_config().clone();
        draft.alignment_pins.clear();
        crate::ui::properties::apply_stock_draft(&mut gui.state, draft);
        gui.discharge_panel_side_effects();
        let edit = mcp
            .apply_pin_edit(Command::SetAlignmentPins(SetAlignmentPinsArgs {
                pins: Vec::new(),
            }))
            .expect("a set is never refused");
        assert!(
            matches!(edit.outcome_of(6.0), Some(PinDrillSync::Removed { .. })),
            "{:?}",
            edit.pin_drill
        );
        assert_eq!(toolpath_list(&mcp), toolpath_list(&gui));
        assert_eq!(toolpath_list(&mcp), before);
    }

    #[test]
    fn with_no_top_setup_the_door_refuses_and_says_why_p8() {
        let mut controller = sample_controller();
        let _ = controller
            .state
            .session
            .apply(Command::SetSetupFace(SetSetupFaceArgs {
                setup_index: 0,
                face_up: FaceUp::Bottom,
            }))
            .expect("setup 0 exists");
        let count = controller.state.session.toolpath_configs().len();
        let edit = mcp_add_pin(&mut controller, &AlignmentPin::new(10.0, 20.0, 6.0));
        assert_eq!(
            edit.outcome_of(6.0),
            Some(&PinDrillSync::Refused(PIN_DRILL_NEEDS_TOP_SETUP.to_owned()))
        );
        assert_eq!(
            controller.state.session.toolpath_configs().len(),
            count,
            "a refusal adds no toolpath"
        );
    }

    /// The MCP describe step takes the door for every pin row.
    #[test]
    fn the_mcp_pin_rows_reply_through_the_door_p5() {
        const COMMANDS_SRC: &str = include_str!("../../app/mcp/commands.rs");
        // Non-vacuity: the helper exists and calls the door.
        let helper_at = COMMANDS_SRC
            .find("    fn describe_pin_edit(")
            .expect("the describe helper of a pin edit");
        let helper = &COMMANDS_SRC[helper_at..];
        let helper = &helper[..helper[1..]
            .find("\n    fn ")
            .map_or(helper.len(), |n| n + 1)];
        assert!(
            helper.contains("self.controller.finish_pin_edit(effects)"),
            "describe_pin_edit must call the pin-edit door"
        );
        for row in [
            "CommandId::AddAlignmentPin => {",
            "CommandId::RemoveAlignmentPin => {",
            "CommandId::SetAlignmentPins => {",
        ] {
            let at = COMMANDS_SRC
                .find(row)
                .unwrap_or_else(|| panic!("the describe step has an arm `{row}`"));
            let arm = &COMMANDS_SRC[at..];
            let arm = &arm[..arm[1..]
                .find("\n            CommandId::")
                .map_or(arm.len(), |n| n + 1)];
            assert!(
                arm.contains("self.describe_pin_edit("),
                "the arm `{row}` must reply through describe_pin_edit:\n{arm}"
            );
        }
    }

    #[test]
    fn a_top_setup_turned_180_does_not_take_the_pin_drill() {
        // The only Top setup is turned 180 deg. The generator does not turn
        // the pin holes with the setup, so the door refuses (lead ruling
        // 2026-10-12) and names why.
        let mut controller = sample_controller();
        add_drill_op(&mut controller, 0);
        let _ = controller
            .state
            .session
            .apply(Command::SetSetupRotation(SetSetupRotationArgs {
                setup_index: 0,
                z_rotation: ZRotation::Deg180,
            }))
            .expect("setup 0 exists");
        assert_eq!(
            controller.state.session.list_setups()[0].face_up,
            FaceUp::Top,
            "non-vacuity: the only setup is a Top setup"
        );
        let count = controller.state.session.toolpath_configs().len();
        let edit = mcp_add_pin(&mut controller, &AlignmentPin::new(10.0, 20.0, 6.0));
        assert_eq!(
            edit.outcome_of(6.0),
            Some(&PinDrillSync::Refused(PIN_DRILL_NEEDS_TOP_SETUP.to_owned()))
        );
        assert!(
            PIN_DRILL_NEEDS_TOP_SETUP.contains("'Top' at 0 deg"),
            "the refusal names why: {PIN_DRILL_NEEDS_TOP_SETUP}"
        );
        assert_eq!(
            controller.state.session.toolpath_configs().len(),
            count,
            "a refusal adds no toolpath"
        );

        // Control: a Top setup at 0 deg added after it takes the pin drill.
        let _ = controller
            .state
            .session
            .apply(Command::AddSetup(AddSetupArgs {
                name: Some("Top 0".to_owned()),
                face_up: FaceUp::Top,
            }))
            .expect("a setup can be added");
        let edit = mcp_add_pin(&mut controller, &AlignmentPin::new(30.0, 20.0, 6.0));
        assert!(
            matches!(
                edit.outcome_of(6.0),
                Some(PinDrillSync::Created { setup_index: 1, .. })
            ),
            "{:?}",
            edit.pin_drill
        );
    }

    /// P6: no operation drills holes, so no tool drills the pins. The door
    /// refuses and does not take `tools[0]` (the fixture's only tool is an
    /// end mill).
    #[test]
    fn no_drilling_operation_refuses_the_pin_drill_p6() {
        let mut controller = sample_controller();
        assert_eq!(controller.state.session.pin_drill_tool_for(6.0), None);
        assert!(
            !controller.state.session.tools().is_empty(),
            "non-vacuity: a tool exists that the old door took"
        );
        let count = controller.state.session.toolpath_configs().len();
        let edit = mcp_add_pin(&mut controller, &AlignmentPin::new(10.0, 20.0, 6.0));
        assert_eq!(
            edit.outcome_of(6.0),
            Some(&PinDrillSync::Refused(pin_drill_needs_tool(6.0)))
        );
        assert_eq!(controller.state.session.toolpath_configs().len(), count);
    }

    /// The pin-drill operations of the project: (diameter, tool diameter,
    /// holes), in toolpath order.
    fn pin_drills(controller: &AppController<ScriptedBackend>) -> Vec<(f64, f64, Vec<[f64; 2]>)> {
        let session = &controller.state.session;
        session
            .toolpath_configs()
            .iter()
            .filter_map(|tc| match &tc.operation {
                OperationConfig::AlignmentPinDrill(c) => {
                    let tool = session
                        .tools()
                        .iter()
                        .find(|t| t.id.0 == tc.tool_id)
                        .map_or(0.0, |t| t.diameter);
                    Some((c.diameter, tool, c.holes.clone()))
                }
                _ => None,
            })
            .collect()
    }

    fn set_pins(
        controller: &mut AppController<ScriptedBackend>,
        pins: Vec<AlignmentPin>,
    ) -> PinEdit {
        controller
            .apply_pin_edit(Command::SetAlignmentPins(SetAlignmentPinsArgs { pins }))
            .expect("a set is never refused")
    }

    /// R1: two pin sizes give two pin-drill operations. Each drills the
    /// holes of its own size with a tool of its own size. Before R1 the
    /// one operation drilled the 8 mm hole with the 6 mm drill.
    #[test]
    fn two_pin_sizes_give_two_pin_drills_r1() {
        let mut controller = flipped_first_controller();
        let small = AlignmentPin::new(10.0, 20.0, 6.0);
        let large = AlignmentPin::new(40.0, 20.0, 8.0);

        // No 8 mm tool: the 6 mm operation is made, the 8 mm one refused.
        let edit = set_pins(&mut controller, vec![small.clone(), large]);
        assert!(
            matches!(edit.outcome_of(6.0), Some(PinDrillSync::Created { .. })),
            "{edit:?}"
        );
        assert_eq!(
            edit.outcome_of(8.0),
            Some(&PinDrillSync::Refused(pin_drill_needs_tool(8.0)))
        );
        assert_eq!(
            pin_drills(&controller),
            vec![(6.0, 6.0, vec![[10.0, 20.0]])]
        );

        // The wizard's "Add a 8.00 mm drill", then Apply: one tool, and the
        // 8 mm operation with it.
        let tools_before = controller.state.session.tools().len();
        let pins_now = controller
            .state
            .session
            .stock_config()
            .alignment_pins
            .clone();
        let _ = controller
            .apply_pin_wizard(pins_now, &[8.0], &[])
            .expect("the wizard's pins are taken");
        assert_eq!(controller.state.session.tools().len(), tools_before + 1);
        let tool = controller
            .state
            .session
            .tools()
            .last()
            .expect("the new tool");
        assert!((tool.diameter - 8.0).abs() < 1e-12, "{tool:?}");
        assert_eq!(
            pin_drills(&controller),
            vec![
                (6.0, 6.0, vec![[10.0, 20.0]]),
                (8.0, 8.0, vec![[40.0, 20.0]]),
            ],
            "one operation per size, each with its own holes and a tool of its size"
        );

        // A pin move updates only the holes of its size.
        let moved = AlignmentPin::new(42.0, 20.0, 8.0);
        let edit = set_pins(&mut controller, vec![small.clone(), moved]);
        assert_eq!(edit.outcome_of(6.0), Some(&PinDrillSync::Unchanged));
        assert!(matches!(
            edit.outcome_of(8.0),
            Some(PinDrillSync::Updated { .. })
        ));

        // The 8 mm pin goes: its operation goes, the 6 mm one stays.
        let edit = set_pins(&mut controller, vec![small]);
        assert!(matches!(
            edit.outcome_of(8.0),
            Some(PinDrillSync::Removed { .. })
        ));
        assert_eq!(
            pin_drills(&controller),
            vec![(6.0, 6.0, vec![[10.0, 20.0]])]
        );
    }

    /// R2, round 2: the wizard's per-size tool chooser queues a tool; Apply
    /// rebinds the operation of that size through the door.
    #[test]
    fn the_tool_chooser_binds_one_size_r2() {
        let mut controller = flipped_first_controller();
        let _ = set_pins(&mut controller, vec![AlignmentPin::new(10.0, 20.0, 6.0)]);
        let mut second = rs_cam_core::compute::tool_config::ToolConfig::new_default(
            ToolId(0),
            ToolType::EndMill,
        );
        second.diameter = 6.0;
        let index = controller
            .state
            .session
            .apply(Command::AddTool(rs_cam_core::session::AddToolArgs {
                tool: Box::new(second),
            }))
            .expect("a tool can be added")
            .created
            .expect("an index");
        let second_id = controller.state.session.tools()[index].id.0;
        assert_eq!(controller.state.session.pin_tool_candidates(6.0).len(), 2);
        let op = controller
            .state
            .session
            .pin_drill_op_for(6.0)
            .expect("the op");
        assert_ne!(
            controller.state.session.toolpath_configs()[op].tool_id,
            second_id,
            "non-vacuity: the door chose the first tool"
        );
        let pins = controller
            .state
            .session
            .stock_config()
            .alignment_pins
            .clone();
        let _ = controller
            .apply_pin_wizard(pins, &[], &[(6.0, second_id)])
            .expect("the wizard's pins are taken");
        let op = controller
            .state
            .session
            .pin_drill_op_for(6.0)
            .expect("the op");
        assert_eq!(
            controller.state.session.toolpath_configs()[op].tool_id,
            second_id
        );
    }

    /// R4: the wizard's Apply adds the queued drill, writes the pins in one
    /// step (one undo entry) and leaves one operation per size.
    #[test]
    fn the_wizard_apply_adds_the_drill_and_writes_once_r4() {
        let mut controller = flipped_first_controller();
        let undo_before = controller.state.history.undo_len();
        let pins = vec![
            AlignmentPin::new(10.0, 20.0, 6.0),
            AlignmentPin::new(40.0, 20.0, 8.0),
        ];
        let edit = controller
            .apply_pin_wizard(pins.clone(), &[8.0], &[])
            .expect("the wizard's pins are taken");
        assert!(
            edit.pin_drill
                .iter()
                .all(|s| !matches!(s.outcome, PinDrillSync::Refused(_))),
            "{edit:?}"
        );
        assert_eq!(controller.state.session.stock_config().alignment_pins, pins);
        assert_eq!(
            pin_drills(&controller),
            vec![
                (6.0, 6.0, vec![[10.0, 20.0]]),
                (8.0, 8.0, vec![[40.0, 20.0]]),
            ]
        );
        assert_eq!(
            controller.state.history.undo_len(),
            undo_before + 1,
            "the pin edit is one undo step"
        );
    }

    /// R4: the wizard works on its own draft. A wizard that edits its
    /// pins and queues a drill, then closes without Apply, changes nothing.
    #[test]
    fn the_wizard_cancel_changes_nothing_r4() {
        let mut controller = flipped_first_controller();
        let _ = set_pins(&mut controller, vec![AlignmentPin::new(10.0, 20.0, 6.0)]);
        let stock_before = controller.state.session.stock_config().clone();
        let tools_before = controller.state.session.tools().len();
        let toolpaths_before = toolpath_list(&controller);

        let mut wizard = crate::ui::pin_wizard::PinWizardState::open(&controller.state.session);
        let map = controller.state.session.pin_set_report().map;
        wizard.pins = crate::ui::properties::pins::edit::move_along_axis(&wizard.pins, &map, 3.0);
        wizard.pins.push(AlignmentPin::new(40.0, 20.0, 8.0));
        wizard.add_drills.push(8.0);
        controller.state.panels.pin_wizard = Some(wizard);
        // Cancel: the draw drops the wizard; nothing reaches the session.
        controller.state.panels.pin_wizard = None;

        assert_eq!(*controller.state.session.stock_config(), stock_before);
        assert_eq!(controller.state.session.tools().len(), tools_before);
        assert_eq!(toolpath_list(&controller), toolpaths_before);
    }

    /// Round 2: the wizard's part-model choice goes through the door as one
    /// `SetPinPartModels` row and one undo step; the default rule comes back
    /// the same way.
    #[test]
    fn the_wizard_part_model_choice_is_one_door_row() {
        let mut controller = flipped_first_controller();
        let undo_before = controller.state.history.undo_len();
        assert_eq!(
            controller.state.session.stock_config().pin_part_models,
            None
        );
        controller.set_pin_part_models(Some(Vec::new()));
        assert_eq!(
            controller.state.session.stock_config().pin_part_models,
            Some(Vec::new())
        );
        assert_eq!(controller.state.history.undo_len(), undo_before + 1);
        controller.set_pin_part_models(None);
        assert_eq!(
            controller.state.session.stock_config().pin_part_models,
            None
        );
        assert_eq!(controller.state.history.undo_len(), undo_before + 2);
    }

    /// A size with no drill refuses on every GUI pin edit, but it toasts
    /// once: the stock panel pin block and the wizard state the refusal.
    #[test]
    fn a_standing_refusal_toasts_once() {
        let mut controller = flipped_first_controller();
        let toasts = |c: &AppController<ScriptedBackend>| {
            c.notifications()
                .iter()
                .filter(|n| n.message == pin_drill_needs_tool(7.0))
                .count()
        };
        for x in [40.0, 41.0, 42.0] {
            let mut draft = controller.state.session.stock_config().clone();
            draft.alignment_pins = vec![
                AlignmentPin::new(10.0, 20.0, 6.0),
                AlignmentPin::new(x, 20.0, 7.0),
            ];
            crate::ui::properties::apply_stock_draft(&mut controller.state, draft);
            controller.discharge_panel_side_effects();
        }
        assert_eq!(toasts(&controller), 1, "one toast for a standing refusal");
    }

    /// R5a: a pin edit that keys the set drops the load-time keying
    /// warning at once, and the modal closes when no warning is left. A
    /// warning that is not a pin warning stays.
    #[test]
    fn a_keying_pin_edit_drops_the_stale_load_warning_r5a() {
        let mut controller = flipped_first_controller();
        let unkeyed = vec![
            AlignmentPin::new(10.0, 20.0, 6.0),
            AlignmentPin::new(30.0, 20.0, 6.0),
        ];
        let _ = set_pins(&mut controller, unkeyed);
        let session = &controller.state.session;
        let live: Vec<String> = session
            .stock_config()
            .pin_findings_for_setups(
                session
                    .list_setups()
                    .iter()
                    .map(|s| (s.face_up, s.z_rotation)),
            )
            .into_iter()
            .map(|f| f.text)
            .collect();
        assert!(!live.is_empty(), "non-vacuity: the two pins give a finding");
        // The state a load leaves: the pin warnings and one other warning.
        controller.load_pin_warnings = live.clone();
        controller.load_warnings = live;
        controller.show_load_warnings = true;

        // The keyed 3-pin set of the placer clears the pin warnings. The
        // warning that is not a pin warning stays, and so does the modal.
        controller
            .load_warnings
            .push("Model 'x.stl' failed to load".to_owned());
        // One pin on the flip axis and a mirror pair off it: the first
        // pattern of a small grid that keys on the fixture's stock.
        let map = controller.state.session.pin_set_report().map;
        let length = crate::ui::properties::pins::edit::axis_length(&map);
        let keyed = [0.1, 0.2]
            .into_iter()
            .flat_map(|a| [0.7, 0.9].into_iter().map(move |b| (a, b)))
            .flat_map(|(a, b)| [0.2, 0.3, 0.4].into_iter().map(move |c| (a, b, c)))
            .filter_map(|(a, b, c)| {
                crate::ui::properties::pins::edit::axis_pair_pins(
                    &map,
                    rs_cam_core::compute::alignment_pins::AxisPairPattern {
                        axis_along: a * length,
                        pair_along: b * length,
                        pair_across: c * length,
                        diameter: 6.0,
                    },
                )
            })
            .find(|pins| {
                controller
                    .state
                    .session
                    .pin_set_report_for(pins)
                    .flip
                    .keyed()
            })
            .expect("a keyed pattern exists on the fixture");
        let _ = set_pins(&mut controller, keyed);
        assert!(
            controller.state.session.pin_set_report().flip.keyed(),
            "non-vacuity: the new set keys"
        );
        let pin_left = controller
            .load_warnings
            .iter()
            .filter(|w| controller.load_pin_warnings.contains(w))
            .count();
        assert_eq!(
            pin_left,
            controller.load_pin_warnings.len(),
            "the list and the pin list agree"
        );
        assert_eq!(
            controller.load_warnings,
            vec!["Model 'x.stl' failed to load".to_owned()],
            "only the warning that is not a pin warning stays"
        );
        assert!(
            controller.show_load_warnings,
            "one warning stays, so the modal stays"
        );

        // With no warning left, the modal closes.
        controller.load_warnings.clear();
        controller.load_pin_warnings = vec!["stale".to_owned()];
        controller.load_warnings.push("stale".to_owned());
        controller.show_load_warnings = true;
        let pins = controller
            .state
            .session
            .stock_config()
            .alignment_pins
            .clone();
        let _ = set_pins(&mut controller, pins);
        assert!(controller.load_warnings.is_empty());
        assert!(!controller.show_load_warnings, "the modal closes");
    }

    /// The mini test piece (`planning/rivmap_block_job/testpiece_mini.toml`)
    /// still loads with ONE pin-drill operation of 6 mm and the same three
    /// holes. A pin edit that keeps the pins leaves it unchanged.
    #[test]
    fn the_mini_project_keeps_one_pin_drill_r1() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../planning/rivmap_block_job/testpiece_mini.toml");
        let mut controller = AppController::with_backend(ScriptedBackend::new());
        controller
            .open_job_from_path(&path)
            .expect("the mini project loads");
        let expected = vec![[59.0, 32.5], [7.511, 9.0], [7.511, 56.0]];
        assert_eq!(
            pin_drills(&controller),
            vec![(6.0, 6.0, expected.clone())],
            "one 6 mm pin drill with the three holes"
        );
        let pins = controller
            .state
            .session
            .stock_config()
            .alignment_pins
            .clone();
        let edit = set_pins(&mut controller, pins);
        assert!(
            edit.pin_drill
                .iter()
                .all(|s| s.outcome == PinDrillSync::Unchanged),
            "{edit:?}"
        );
        assert_eq!(pin_drills(&controller), vec![(6.0, 6.0, expected)]);
    }
}
