//! G-SETUPONLY — the setup header's "Regenerate setup only" is a NARROW plan.
//!
//! Operator ruling, 2026-09-19: "the setup should have `…` with 'Regenerate
//! setup only', and by default it should regen what's required."
//!
//! The DEFAULT is the dependency walk: Generate All and a single Generate
//! both resolve what a scope depends on. This route is the other one, and
//! the whole point of it is what it does NOT do. So the claims are about
//! absence as much as presence:
//!
//! - only the named setup's operations get a Generate step;
//! - an earlier setup's stale operation is NOT regenerated, however stale;
//! - the Simulate steps still cover the earlier setups, because a rest
//!   operation reads a snapshot of the whole stock above it;
//! - it takes the same busy door as Generate All, so it cannot race a plan.

use super::*;

use crate::controller::generate_all::PlanStep;
use crate::state::toolpath::ComputeStatus;

/// Setup 1 = `Rough (Fresh)`, `Rest A`, `Rest B`; setup 2 = `Plain` and
/// `Rest C`, which machines what setup 2 leaves.
///
/// The second setup holds a rest operation ON PURPOSE: its prefix simulation
/// is what proves the narrow scope still simulates the setups above it.
fn two_setups_each_with_rest() -> (AppController<RestChainBackend>, Vec<ToolpathId>, usize) {
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

    let template = controller
        .state
        .session
        .toolpath_configs()
        .first()
        .cloned()
        .expect("the fixture has a first toolpath");
    for (id, name, source) in [
        (
            ToolpathId(9),
            "Plain",
            crate::state::toolpath::StockSource::Fresh,
        ),
        (
            ToolpathId(10),
            "Rest C",
            crate::state::toolpath::StockSource::FromRemainingStock,
        ),
    ] {
        let mut config = template.clone();
        config.id = id;
        config.name = name.to_owned();
        config.stock_source = source;
        let _ = controller
            .state
            .session
            .apply(Command::AddToolpath(AddToolpathArgs {
                setup_index: second,
                config: Box::new(config),
            }))
            .expect("the second setup accepts a toolpath");
    }

    let ids: Vec<ToolpathId> = controller
        .state
        .session
        .toolpath_configs()
        .iter()
        .map(|tc| tc.id)
        .collect();
    for (index, id) in ids.iter().enumerate() {
        let rt = controller.state.gui.toolpath_rt_or_default(*id);
        rt.result = None;
        rt.status = ComputeStatus::Pending;
        let _ = controller.state.session.apply(Command::ForgetResult(
            rs_cam_core::session::ForgetResultArgs { index },
        ));
    }
    let _ = controller
        .state
        .session
        .apply(Command::SetSimulationResolution(
            rs_cam_core::session::SetSimulationResolutionArgs {
                resolution: rs_cam_core::session::SimulationResolution::Auto,
            },
        ))
        .expect("auto is always accepted");
    (controller, ids, second)
}

fn steps(controller: &AppController<RestChainBackend>) -> Vec<PlanStep> {
    controller
        .plan
        .as_ref()
        .map(|plan| plan.steps.clone())
        .unwrap_or_default()
}

fn setup_id(controller: &AppController<RestChainBackend>, position: usize) -> SetupId {
    SetupId(
        controller
            .state
            .session
            .list_setups()
            .get(position)
            .expect("the fixture holds that setup")
            .id,
    )
}

/// The narrow plan generates ONE setup and simulates the ones above it.
#[test]
fn a_setup_only_plan_generates_that_setup_g_setuponly() {
    let (mut controller, ids, _second) = two_setups_each_with_rest();
    let (rough, rest_a, rest_b) = (ids[0], ids[1], ids[2]);
    let (plain, rest_c) = (ids[3], ids[4]);
    let back = setup_id(&controller, 1);

    controller.handle_internal_event(crate::ui::AppEvent::GenerateSetupOnly(back));

    let steps = steps(&controller);
    assert_eq!(
        steps,
        vec![
            PlanStep::Generate(plain),
            PlanStep::SimulatePrefix {
                setup: back,
                upto: rest_c,
            },
            PlanStep::Generate(rest_c),
            PlanStep::SimulateAll,
        ],
        "the narrow scope generates setup 2 and nothing else, and its rest \
         operation still gets a prefix simulation"
    );

    // The claim that matters is the ABSENCE. Setup 1 is cold, and every one
    // of its operations stays out of the plan.
    for (name, id) in [("Rough", rough), ("Rest A", rest_a), ("Rest B", rest_b)] {
        assert!(
            !steps.contains(&PlanStep::Generate(id)),
            "{name} is in an EARLIER setup and it is cold, and the narrow \
             route still must not regenerate it: {steps:?}"
        );
    }

    // The prefix simulation names the setup it stops at, and the driver
    // covers every setup up to that one's position. A request narrowed to
    // setup 2 alone would take the snapshot before setup 1's cuts.
    let prefix = steps
        .iter()
        .find_map(|step| match step {
            PlanStep::SimulatePrefix { setup, .. } => Some(*setup),
            _ => None,
        })
        .expect("the plan simulates before its rest operation");
    assert_eq!(
        prefix, back,
        "the Simulate step carries the setup the plan stops AT, which is \
         position 1, so it covers setups 0..=1"
    );
    assert_ne!(
        prefix,
        setup_id(&controller, 0),
        "non-vacuity: the two setups have different ids"
    );
}

/// A stale operation in an earlier setup is left alone.
///
/// The same claim as above, driven from the other side: setup 1 is made
/// CURRENT and then edited, so nothing about it is ambiguous. The default
/// route would rebuild it; this one must not.
#[test]
fn a_stale_earlier_setup_is_not_rebuilt_g_setuponly() {
    let (mut controller, ids, _second) = two_setups_each_with_rest();
    let rough = ids[0];
    let back = setup_id(&controller, 1);

    // Setup 1's first operation is generated, then edited.
    let rt = controller.state.gui.toolpath_rt_or_default(rough);
    rt.status = ComputeStatus::Done;
    controller.state.gui.mark_edited();

    controller.handle_internal_event(crate::ui::AppEvent::GenerateSetupOnly(back));

    assert!(
        !steps(&controller).contains(&PlanStep::Generate(rough)),
        "the narrow route regenerates the named setup, and only it"
    );
}

/// It takes the same busy door as Generate All.
#[test]
fn a_setup_only_plan_is_refused_while_one_runs_g_setuponly() {
    let (mut controller, _ids, _second) = two_setups_each_with_rest();
    let back = setup_id(&controller, 1);

    controller.handle_internal_event(crate::ui::AppEvent::GenerateAll);
    assert!(controller.plan_is_busy(), "the first plan is armed");
    let before = steps(&controller);

    controller.handle_internal_event(crate::ui::AppEvent::GenerateSetupOnly(back));
    assert_eq!(
        steps(&controller),
        before,
        "a second plan would race the first one's cursor, so the narrow \
         route is refused and the running plan is untouched"
    );
}
