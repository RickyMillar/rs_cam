//! WP19 — an edit clears the view simulation, and a stamped row reads the
//! catalogue's auto-regen.

use super::*;

/// b2 — a panel edit that drops the session's simulation owes the view.
///
/// `Command::SetStockConfig` reaches `drop_all_results`, which clears
/// the session's simulation. A draw site holds an `AppState` and
/// nothing else, so it cannot call `invalidate_simulation` itself: it
/// raises a `PanelSideEffects` flag and the frame loop discharges it,
/// the mechanism WP6 built.
///
/// Red before the fix: `state.simulation.results` is still `Some`. The
/// operator reads collision counts off a simulation of a stock that no
/// longer exists.
#[cfg(feature = "mcp")]
#[test]
fn a_panel_stock_edit_clears_the_view_simulation_wp19() {
    let mut controller = controller_holding_a_simulation();

    let mut draft = controller.state.session.stock_config().clone();
    // The funnel re-fits an `auto_from_model` draft around the model
    // before it compares, so a typed dimension alone would arrive back
    // at the stored record. The operator clears the checkbox to type a
    // dimension; the draft does the same
    // (`freshness_stock_edit_stales_every_toolpath`).
    draft.auto_from_model = false;
    draft.x = 321.0;
    crate::ui::properties::apply_stock_draft(&mut controller.state, draft);

    assert!(
        controller.state.session.simulation_result().is_none(),
        "the control: the stock row drops every result, and the \
         session's simulation with them"
    );
    controller.discharge_panel_side_effects();
    assert!(
        controller.state.simulation.results.is_none(),
        "the session dropped its simulation and the viewport still \
         draws one. The panel owes the frame loop that work, and the \
         frame loop runs it."
    );
}

/// c1 — a stamp on a never-drawn toolpath creates the catalog's row.
///
/// `stamp_stale` writes through `toolpath_rt_or_default`, which CREATES
/// a missing row. The row then joins `process_auto_regen`, which reads
/// `rt.auto_regen`. Twelve of the twenty-four operations declare
/// `default_auto_regen: false`, so a row hardcoded to `true` queues a
/// 3D finishing pass the operator never asked for.
///
/// Red before the fix: the `auto_regen` half. The fixture's operation
/// is `Scallop`, which the catalog opts OUT of auto-regeneration, and
/// the created row reads `true`. The `stale_since` half is green — that
/// is today's behaviour, and the arm pins it.
#[test]
fn a_stamped_row_reads_the_catalog_auto_regen_h3() {
    let mut controller = sample_controller();
    push_toolpath(&mut controller, "Second");
    let (id, expected) = {
        let tc = &controller.state.session.toolpath_configs()[1];
        (tc.id, tc.operation.default_auto_regen())
    };
    assert!(
        !expected,
        "the fixture must use an operation the catalog opts OUT of, or \
         this arm cannot tell a hardcoded `true` from a catalog read"
    );
    // The MCP route reaches a toolpath whose card the operator has
    // never drawn, so no runtime row exists. That is the case H3 names.
    controller.state.gui.toolpath_rt.remove(&id);

    let mut stale = std::collections::BTreeSet::new();
    stale.insert(1_usize);
    crate::state::stale::stamp_stale(&mut controller.state, &stale);

    let rt = controller
        .state
        .gui
        .toolpath_rt
        .get(&id)
        .expect("the stamp creates the row a never-drawn toolpath lacks");
    assert!(rt.stale_since.is_some(), "and it stamps what it created");
    assert_eq!(
        rt.auto_regen, expected,
        "a created row must read the operation's own `default_auto_regen`"
    );
}

/// b3 — the undo of a stock change stales every toolpath.
///
/// This arm was GREEN when it was written. WP15a routed the undo door
/// through `apply_quietly`, which stamps `Effects::stale`, and H2 named
/// this site before that landed. It stays as a regression pin: the row
/// drops EVERY result (G-FRESHSTATE), so every card must follow.
#[test]
fn an_undone_stock_change_stales_every_toolpath_wp19() {
    use crate::state::history::UndoAction;

    let (mut controller, _) = controller_ready_for_undo();
    push_toolpath(&mut controller, "Second");
    generate_all_for_test(&mut controller);
    let action = UndoAction::StockChange {
        old: controller.state.session.stock_config().clone(),
        new: controller.state.session.stock_config().clone(),
    };
    controller.state.history.push(action);

    controller.handle_internal_event(AppEvent::Undo);

    for index in 0..2 {
        assert_eq!(
            state_of(&controller, index),
            FreshnessState::EditedSince,
            "toolpath {index} after an undone stock change"
        );
        assert!(controller.state.session.get_result(index).is_none());
    }
}

/// b1 — a toolpath edit clears the view's simulation (plan §28).
///
/// The operator ruled ONE rule for every row: a session that drops its
/// simulation leaves no viewport showing one. A toolpath-scoped edit
/// used to STALE the view's instead, under the F2.5 banner, while
/// `ProjectSession::start` was already refusing every
/// `FromRemainingStock` operation because the session held none. A
/// banner does not say that; an empty viewport does.
///
/// RED: this arm lands in the same commit as its fix, so the red is
/// read by running it against the commit BEFORE. There the first
/// assertion passes — `set_toolpath_enabled` writes
/// `session.simulation = None` — and the last two fail, because
/// `apply_quietly` cleared nothing in the view.
#[cfg(feature = "mcp")]
#[test]
fn a_toolpath_edit_clears_the_view_simulation_wp19() {
    let mut controller = controller_holding_a_simulation();
    let id = controller.state.session.toolpath_configs()[0].id;

    controller.handle_internal_event(AppEvent::ToggleToolpathEnabled(id));

    assert!(
        controller.state.session.simulation_result().is_none(),
        "the control: the toggle drops the session's simulation"
    );
    assert!(
        controller.state.simulation.results.is_none(),
        "and the viewport must not go on drawing the run of a job that \
         no longer exists"
    );
    assert!(controller.state.simulation.last_run.is_none());
}

// ── WP24 — the Optimize run is state, not a lockout ──────────────────
//
// Programme: `planning/arch_consolidation_2026-09-09/IMPLEMENTATION_PLAN.md`
// §30 ruling 3 (operator, 2026-09-13). The draw half of this finding is
// `crates/rs_cam_viz/tests/optimize_run_is_non_modal_wp24.rs`; these three
// arms are the state half, and they need `ScriptedBackend` plus the
// `pub(crate)` drain, so they live here.
//
// All three arms drive the run with the TIER-MAP PREVIEW. It is the same
// `OptimizeRun` the per-toolpath Optimize carries, and it needs no cut
// trace: no controller fixture in this crate holds one
// (`cut_trace: None` on every one of them), and
// `capture_optimize_toolpath` refuses without one. So the `Toolpath` kind
// of `OptimizeRun` is NOT exercised in-crate, and this heading says so
// rather than hiding it.

/// The event loop never blocked, and a run no longer blocks the drawing.
///
/// The placeholder replaced the central panel; it never stopped an event
/// reaching the controller. So the BEHAVIOURAL half of this arm already
/// held before WP24, and the arm is a regression guard: it bites a future
/// writer who puts the lockout back into the event loop instead of into
/// the draw.
///
/// RED at the parent revision: COMPILE-red. The arm names
/// `AppState::optimize_run`, which does not exist there — the state is a
/// bare `is_optimizing: bool`.
///
/// The probe is `UiCommand::ToggleSimPlayback`, and NOT
/// `UiCommand::SwitchWorkspace`. The controller's arm for a workspace
/// switch is a no-op (`controller/events/mod.rs:586`); `RsCamApp` applies
/// that command in `app/input.rs`, because the switch also moves the
/// camera and the overlay set. So a controller test cannot read a
/// workspace switch at all, and the probe has to be a command the
/// controller itself applies.
#[test]
fn a_view_command_still_applies_while_an_optimize_run_is_in_flight() {
    let mut controller = planner_controller();
    controller.handle_internal_event(AppEvent::Ui(UiCommand::OpenMultitoolPlanner(NoArgs)));
    tick_ball_tools(&mut controller);
    controller.handle_internal_event(AppEvent::PreviewMultitoolPlan);

    assert!(
        controller.state.optimize_run.is_some(),
        "the preview is in flight, so the run is on the state"
    );
    assert_eq!(controller.compute.job_requests.len(), 1);
    assert!(
        !controller.state.simulation.playback.playing,
        "the probe must flip a value, so read it first"
    );

    controller.handle_internal_event(AppEvent::Ui(UiCommand::ToggleSimPlayback(NoArgs)));

    assert!(
        controller.state.simulation.playback.playing,
        "a view command applies during a run — the GUI stays usable"
    );
    assert!(
        controller.state.optimize_run.is_some(),
        "and the unrelated command leaves the run alone"
    );
}
