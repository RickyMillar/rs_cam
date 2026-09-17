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
            !sim.has_results() || sim.is_stale(controller.state.gui.edit_counter),
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
/// - The undo entry carried no `feeds_provenance`. Three optimizer apply
///   paths wrote the provenance in a second call right after the
///   snapshot, so an undo put the pre-optimizer numbers back and left
///   the `Optimizer` stamp standing on them.
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
        feed_rate: Some(rs_cam_core::feeds::ValueProvenance::optimizer()),
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
// `SimulationRunMeta::last_sim_edit_counter` was stamped in the DRAIN from
// the live `edit_counter`, which folded every edit made while the simulation
// ran into the record of when it was run. `is_stale` then said `false`.
//
// The counter is project-wide and that is deliberate — see `reports/F2.10.md`
// §3. In short: this is already `is_stale`'s semantics after a run, so
// stamping at submit adds no new coarseness; it makes the in-flight window
// behave like the window either side of it.

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
        controller.state.simulation.is_stale(after_edit),
        "but NOT current — this run measured the configuration the operator \
         has already left, and it is the surface they read collision counts \
         off"
    );
    assert_eq!(
        controller
            .state
            .simulation
            .last_run
            .as_ref()
            .map(|m| m.last_sim_edit_counter),
        submitted_at,
        "the run is recorded against the counter it was SUBMITTED at, not the \
         one that happened to be live when it landed"
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
        !controller
            .state
            .simulation
            .is_stale(controller.state.gui.edit_counter),
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
