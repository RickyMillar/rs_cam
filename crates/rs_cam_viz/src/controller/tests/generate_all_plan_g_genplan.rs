//! G-GENPLAN — Generate All is a plan over the dependency edges, not a
//! round-based fixpoint.
//!
//! W1 of `planning/gen_sim_rest_ux_2026-09-18/`. The operator's report was
//! "if I generate all, it errors that some need rest". The old ladder refused
//! to start on the panel's default resolution setting, toasted once per
//! blocked operation when it did start, and advanced by pushing an `AppEvent`
//! that only a painted frame drained.
//!
//! What this file pins, over a two-setup rest chain:
//!
//! - the exact step sequence the walk produces, and one prefix simulation per
//!   rest operation;
//! - that the run makes every enabled operation current with ZERO warning
//!   toasts, on the panel's own auto resolution;
//! - that `drain_events()` stays empty across the whole plan, so no step
//!   waits for a frame (the 137 s dispatch stall);
//! - that a blocked submit inside a plan says nothing, because the row
//!   already reads WAIT;
//! - that one Generate on the last rest operation runs its ancestors;
//! - that the 500 ms auto-regeneration sweep is inert while a plan runs;
//! - that an operator edit mid-plan cancels it.

use super::*;

use crate::controller::generate_all::{Activity, PlanStep};
use crate::state::toolpath::ComputeStatus;

/// Setup 1 = `Rough (Fresh)`, `Rest A`, `Rest B`; setup 2 = one plain op.
/// Auto resolution on, no pinned value, every operation cold.
fn two_setup_chain() -> (AppController<RestChainBackend>, Vec<ToolpathId>) {
    let mut controller = AppController::with_backend(RestChainBackend::new());
    sample_project_into(&mut controller);
    push_toolpath(&mut controller, "Rest A");
    push_toolpath(&mut controller, "Rest B");
    for index in 1..=2 {
        let _ = controller
            .state
            .session
            .apply(Command::SetStockSource(SetStockSourceArgs {
                index,
                source: crate::state::toolpath::StockSource::FromRemainingStock,
            }))
            .expect("the index comes from the session");
    }

    let second = controller
        .state
        .session
        .apply(Command::AddSetup(AddSetupArgs {
            name: Some("Back".to_owned()),
            face_up: rs_cam_core::compute::transform::FaceUp::default(),
        }))
        .expect("the session accepts a second setup")
        .created
        .expect("the AddSetup row reports the new setup index");
    let plain = ToolpathId(9);
    let mut config = controller
        .state
        .session
        .toolpath_configs()
        .first()
        .cloned()
        .expect("the fixture has a first toolpath");
    config.id = plain;
    config.name = "Plain".to_owned();
    config.stock_source = crate::state::toolpath::StockSource::Fresh;
    let _ = controller
        .state
        .session
        .apply(Command::AddToolpath(AddToolpathArgs {
            setup_index: second,
            config: Box::new(config),
        }))
        .expect("the second setup accepts a toolpath");

    let ids: Vec<ToolpathId> = controller
        .state
        .session
        .toolpath_configs()
        .iter()
        .map(|tc| tc.id)
        .collect();
    // The fixture op at index 0 starts with a cached result; clear every one
    // so the run really is "from cold", core cache included.
    for (index, id) in ids.iter().enumerate() {
        let rt = controller.state.gui.toolpath_rt_or_default(*id);
        rt.result = None;
        rt.status = ComputeStatus::Pending;
        let _ = controller.state.session.apply(Command::ForgetResult(
            rs_cam_core::session::ForgetResultArgs { index },
        ));
    }
    controller.state.simulation.auto_resolution = true;
    (controller, ids)
}

/// The step list the plan holds, as a comparable shape.
fn steps(controller: &AppController<RestChainBackend>) -> Vec<PlanStep> {
    controller
        .plan
        .as_ref()
        .map(|plan| plan.steps.clone())
        .unwrap_or_default()
}

#[test]
fn generate_all_walks_one_plan_over_the_rest_chain() {
    let (mut controller, ids) = two_setup_chain();
    let (rough, rest_a, rest_b, plain) = (ids[0], ids[1], ids[2], ids[3]);

    controller.handle_internal_event(crate::ui::AppEvent::GenerateAll);

    // 1. The step list, in order.
    let setup_1 = rs_cam_core::ids::SetupId(controller.state.session.list_setups()[0].id);
    assert_eq!(
        steps(&controller),
        vec![
            PlanStep::Generate(rough),
            PlanStep::SimulatePrefix {
                setup: setup_1,
                upto: rest_a
            },
            PlanStep::Generate(rest_a),
            PlanStep::SimulatePrefix {
                setup: setup_1,
                upto: rest_b
            },
            PlanStep::Generate(rest_b),
            PlanStep::Generate(plain),
            PlanStep::SimulateAll,
        ],
        "the walk is setup order, then `toolpath_indices`, with one prefix \
         simulation before each rest operation that has no snapshot"
    );

    // 6. The plan advances by direct call, never through the event queue.
    // `handle_events` runs only inside `draw_frame`, so a step that waited for
    // an `AppEvent` would wait for a painted frame — the 137 s stall.
    assert!(
        controller.drain_events().is_empty(),
        "arming the plan must push no AppEvent"
    );
    for _ in 0..200 {
        if !controller.awaiting_generate_all() {
            break;
        }
        controller.drain_compute_results();
        assert!(
            controller.drain_events().is_empty(),
            "a plan step must not hand off through the event queue"
        );
    }
    assert!(
        !controller.awaiting_generate_all(),
        "the plan must terminate: no step is retried and the cursor only \
         moves forward"
    );

    // 2. Exactly one prefix simulation per rest operation, plus one closing
    // full simulation.
    assert_eq!(controller.compute.simulations, 3);
    // Each prefix simulation unlocks exactly the operation the next step
    // generates. The scan latches at the FIRST enabled config with no result,
    // so one simulation cannot unlock two.
    let phantoms: Vec<Option<ToolpathId>> = controller
        .compute
        .sim_requests
        .iter()
        .filter_map(|groups| groups.first().map(|(_, phantom)| phantom.map(|(_, id)| id)))
        .collect();
    assert_eq!(
        phantoms,
        vec![Some(rest_a), Some(rest_b), None],
        "one unlock per setup per simulation, and the closing one unlocks \
         nothing: {:?}",
        controller.compute.sim_requests
    );

    // 3. Zero warnings. Read the whole stack, not `active_notifications`.
    let warnings: Vec<&str> = controller
        .notifications()
        .iter()
        .filter(|n| n.severity == crate::controller::Severity::Warning)
        .map(|n| n.message.as_str())
        .collect();
    assert!(
        warnings.is_empty(),
        "the panel's auto resolution is the operator's standing choice, and \
         a blocked step is a sequencing state: {warnings:?}"
    );

    // 4. Every enabled operation is current.
    for id in &ids {
        let (index, _) = controller
            .state
            .session
            .find_toolpath_config_by_id(*id)
            .expect("the fixture keeps every toolpath");
        assert!(
            controller.state.session.get_result(index).is_some(),
            "toolpath {} holds no core result",
            id.0
        );
        assert!(
            matches!(
                controller
                    .state
                    .gui
                    .toolpath_rt
                    .get(id)
                    .map(|rt| &rt.status),
                Some(ComputeStatus::Done)
            ),
            "toolpath {} did not reach Done",
            id.0
        );
    }

    // 5. Nothing is left armed.
    assert!(controller.generation_plan_progress().is_none());
}

/// The progress a button reads, mid-plan.
#[test]
fn the_plan_reports_its_position_while_it_runs() {
    let (mut controller, ids) = two_setup_chain();
    controller.handle_internal_event(crate::ui::AppEvent::GenerateAll);

    let progress = controller
        .generation_plan_progress()
        .expect("a running plan reports a position");
    assert_eq!(progress.of, 7);
    assert!(progress.cancellable);
    // The first step is in flight, and its completion is already queued on
    // the fake, so the cursor still names step 1.
    assert_eq!(progress.step, 1);
    assert_eq!(
        progress.activity,
        Activity::Generating(ids[0], "Scallop".to_owned())
    );
    assert!(!progress.is_simulating());
}

/// A/M11 kept, W1 tightened: a blocked submit is a SEQUENCING state. The row
/// reads WAIT and nothing is said, inside a plan or outside one.
#[test]
fn a_blocked_submit_pushes_no_toast() {
    let (mut controller, ids) = two_setup_chain();
    let rest_a = ids[1];

    // No plan, no snapshot: the submit blocks.
    controller.submit_toolpath_compute(rest_a);

    assert!(
        matches!(
            controller
                .state
                .gui
                .toolpath_rt
                .get(&rest_a)
                .map(|rt| &rt.status),
            Some(ComputeStatus::AwaitingPriorStock(_))
        ),
        "the row is the surface: it must read WAIT"
    );
    assert!(
        controller.notifications().is_empty(),
        "blocked is a sequencing state, not an error, so it never toasts: {:?}",
        controller
            .notifications()
            .iter()
            .map(|n| n.message.as_str())
            .collect::<Vec<_>>()
    );
}

/// R6 — one Generate makes the named operation current, which means running
/// its ancestors first.
#[test]
fn a_single_generate_on_the_last_rest_op_runs_the_ancestors() {
    let (mut controller, ids) = two_setup_chain();
    let (rough, rest_a, rest_b, plain) = (ids[0], ids[1], ids[2], ids[3]);

    controller.handle_internal_event(crate::ui::AppEvent::GenerateToolpath(rest_b));

    let armed = steps(&controller);
    assert!(
        armed.contains(&PlanStep::Generate(rough)),
        "the ancestors enter the plan: {armed:?}"
    );
    assert!(
        armed.contains(&PlanStep::Generate(rest_a)),
        "every enabled same-setup predecessor is a Stock source: {armed:?}"
    );
    assert_eq!(
        armed
            .iter()
            .filter(|step| matches!(step, PlanStep::SimulatePrefix { .. }))
            .count(),
        2,
        "both rest operations need their own snapshot: {armed:?}"
    );
    assert!(
        !armed.contains(&PlanStep::Generate(plain)),
        "the other setup's operation is not an ancestor: {armed:?}"
    );

    pump_until_idle(&mut controller);

    for id in [rough, rest_a, rest_b] {
        assert!(
            matches!(
                controller
                    .state
                    .gui
                    .toolpath_rt
                    .get(&id)
                    .map(|rt| &rt.status),
                Some(ComputeStatus::Done)
            ),
            "toolpath {} did not reach Done",
            id.0
        );
    }
}

/// The sweep and the cursor cannot both submit one operation. A Regions
/// source that lands stales its consumers while they are still steps, so this
/// is not a corner case; it happens inside every plan.
#[test]
fn the_auto_regen_sweep_is_inert_while_a_plan_runs() {
    let (mut controller, ids) = two_setup_chain();
    let rest_b = ids[2];

    controller.handle_internal_event(crate::ui::AppEvent::GenerateAll);
    // Stamp a stale row that is old enough for the sweep to take.
    if let Some(rt) = controller.state.gui.toolpath_rt.get_mut(&rest_b) {
        rt.auto_regen = true;
        rt.locked = false;
        rt.stale_since = Some(std::time::Instant::now() - std::time::Duration::from_secs(5));
    }

    controller.process_auto_regen();

    assert!(
        controller
            .state
            .gui
            .toolpath_rt
            .get(&rest_b)
            .and_then(|rt| rt.stale_since)
            .is_some(),
        "the sweep must leave the stamp alone so a genuine leftover is swept \
         on the tick after the plan"
    );
    assert!(
        controller.notifications().is_empty(),
        "an inert sweep says nothing"
    );
}

/// An operator edit drops results the plan has produced. Cancel; the sweep
/// re-plans. Splicing steps into a live cursor is not worth the failure mode.
#[test]
fn an_edit_mid_plan_cancels_it() {
    let (mut controller, _ids) = two_setup_chain();
    controller.handle_internal_event(crate::ui::AppEvent::GenerateAll);
    assert!(controller.awaiting_generate_all());

    controller.state.gui.mark_edited();
    pump_until_idle(&mut controller);

    assert!(
        !controller.awaiting_generate_all(),
        "the plan must not walk on over results the edit dropped"
    );
    let stopped = controller
        .notifications()
        .iter()
        .any(|n| n.message.contains("an edit cancelled the plan"));
    assert!(
        stopped,
        "the summary must say why it stopped: {:?}",
        controller
            .notifications()
            .iter()
            .map(|n| n.message.as_str())
            .collect::<Vec<_>>()
    );
}

/// R1 — a PINNED cell size coarser than the rest needs is the one case the
/// plan asks about. Nothing is submitted until the operator answers.
#[test]
fn a_coarse_pinned_resolution_asks_before_it_generates() {
    let (mut controller, _ids) = two_setup_chain();
    controller.state.simulation.auto_resolution = false;
    controller.state.simulation.resolution = 2.0;

    controller.handle_internal_event(crate::ui::AppEvent::GenerateAll);

    let confirm = controller
        .pending_plan_confirm()
        .cloned()
        .expect("a coarse pinned resolution must ask");
    assert!(
        !controller.awaiting_generate_all(),
        "nothing is submitted while the question stands"
    );
    assert!(confirm.required_mm < 2.0);
    assert!(
        confirm.message.contains("Rest A") && confirm.message.contains("Rest B"),
        "the question names the operations that ask for the finer cell: {}",
        confirm.message
    );
    assert!(
        confirm.accept_label.contains("Use"),
        "the accept button states the number it writes: {}",
        confirm.accept_label
    );

    controller.accept_plan_resolution();

    assert!(controller.pending_plan_confirm().is_none());
    assert!(
        controller.awaiting_generate_all(),
        "accepting starts the plan"
    );
    assert!(
        (controller.state.simulation.resolution - confirm.required_mm).abs() < f64::EPSILON,
        "this is the ONE place a plan writes the panel"
    );
    assert!(
        !controller.state.simulation.auto_resolution,
        "the operator pinned a value; the answer is a different pinned value"
    );
}

/// The other end of it: cancelling submits nothing and writes nothing.
#[test]
fn cancelling_the_resolution_question_starts_no_plan() {
    let (mut controller, _ids) = two_setup_chain();
    controller.state.simulation.auto_resolution = false;
    controller.state.simulation.resolution = 2.0;

    controller.handle_internal_event(crate::ui::AppEvent::GenerateAll);
    controller.cancel_plan_resolution();

    assert!(controller.pending_plan_confirm().is_none());
    assert!(!controller.awaiting_generate_all());
    assert!((controller.state.simulation.resolution - 2.0).abs() < f64::EPSILON);
}

/// A pinned value FINER than the rest needs is the operator's own choice and
/// is taken without a question.
#[test]
fn a_fine_pinned_resolution_asks_nothing() {
    let (mut controller, _ids) = two_setup_chain();
    controller.state.simulation.auto_resolution = false;
    controller.state.simulation.resolution = 0.05;

    controller.handle_internal_event(crate::ui::AppEvent::GenerateAll);

    assert!(controller.pending_plan_confirm().is_none());
    assert!(controller.awaiting_generate_all());
    pump_until_idle(&mut controller);
    assert!(
        (controller.state.simulation.resolution - 0.05).abs() < f64::EPSILON,
        "a plan that asks nothing writes nothing"
    );
}
