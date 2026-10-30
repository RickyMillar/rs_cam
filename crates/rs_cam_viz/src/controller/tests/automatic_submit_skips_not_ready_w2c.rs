//! W2c: an automatic submit skips a toolpath that is Not ready.
//!
//! Seen on screen: a Pocket with no feeds got a tool from the library, the
//! 500 ms sweep submitted it, and a red toast repeated the issue row
//! ("Operation failed: ... no feeds data"). The rule is core
//! `skips_automatic_generate`, read through
//! `AppController::automatic_submit_skip`:
//!
//! - the sweep (`process_auto_regen`) and every plan step that is not the
//!   plan's target skip a disabled or not-ready op, with no toast and no
//!   recorded refusal;
//! - an explicit generate (MCP `generate_toolpath`) still refuses with its
//!   reason, because the agent needs the text.

use super::*;

/// Append a Scallop with no feeds to setup 0. The tool, the model and the
/// floor are set, so the feeds are the input that refuses generation.
fn push_toolpath_with_no_feeds<B: ComputeBackend>(
    controller: &mut AppController<B>,
    name: &str,
) -> ToolpathId {
    use rs_cam_core::compute::catalog::OperationType;

    let id = push_toolpath(controller, name);
    let index = index_of(controller, id);
    let mut config = controller.state.session().toolpath_configs()[index].clone();
    let fixture = rs_cam_core::compute::fixture_values::fixture_op(OperationType::Scallop);
    let mut op = rs_cam_core::compute::OperationConfig::new_default(OperationType::Scallop);
    if let Some(floor) = fixture.floor() {
        let _ = op.set_floor(floor);
    }
    config.operation = op;
    let _ = controller
        .state
        .apply(Command::ReplaceToolpathConfig(
            rs_cam_core::session::ReplaceToolpathConfigArgs {
                index,
                config: Box::new(config),
            },
        ))
        .expect("the index comes from the session");
    id
}

fn run_state<B: ComputeBackend>(
    controller: &AppController<B>,
    id: ToolpathId,
) -> rs_cam_core::session::RunState {
    crate::state::freshness::run_state_of(controller.state.session(), &controller.state.gui, id)
        .expect("the toolpath is in the project")
}

#[test]
fn the_sweep_skips_a_not_ready_op_w2c() {
    let mut controller = sample_controller();
    let id = push_toolpath_with_no_feeds(&mut controller, "Pocket 1");
    assert!(
        matches!(
            run_state(&controller, id),
            rs_cam_core::session::RunState::NotReady { .. }
        ),
        "the fixture op is not ready: {:?}",
        run_state(&controller, id)
    );
    let rt = controller.state.gui.toolpath_rt_or_default(id);
    rt.auto_regen = true;
    rt.locked = false;
    rt.stale_since = Some(std::time::Instant::now() - std::time::Duration::from_secs(5));
    let toasts_before = controller.notifications().len();

    controller.process_auto_regen();

    assert!(
        !controller.compute.submitted.contains(&id),
        "the sweep must not submit a not-ready op"
    );
    assert_eq!(
        controller.notifications().len(),
        toasts_before,
        "the sweep says nothing about a not-ready op: {:?}",
        controller
            .notifications()
            .iter()
            .map(|n| n.message.clone())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        controller.state.session().current_run_refusal(id),
        None,
        "the sweep records no refusal"
    );
    assert!(
        matches!(
            run_state(&controller, id),
            rs_cam_core::session::RunState::NotReady { .. }
        ),
        "the op still reads Not ready, not Failed"
    );
    assert!(
        controller
            .state
            .gui
            .toolpath_rt
            .get(&id)
            .is_some_and(|rt| rt.stale_since.is_none()),
        "the sweep takes the stamp, so it does not ask again on every pump"
    );
}

#[test]
fn generate_all_skips_a_not_ready_op_w2c() {
    let mut controller = AppController::with_backend(RestChainBackend::new());
    sample_project_into(&mut controller);
    let id = push_toolpath_with_no_feeds(&mut controller, "Pocket 1");
    let toasts_before = controller.notifications().len();

    controller.handle_internal_event(crate::ui::AppEvent::GenerateAll);
    pump_until_idle(&mut controller);

    assert!(!controller.plan_is_busy(), "the plan finished");
    assert_eq!(
        controller.state.session().current_run_refusal(id),
        None,
        "the plan records no refusal for a not-ready op"
    );
    let toasts: Vec<_> = controller.notifications()[toasts_before..]
        .iter()
        .map(|n| (n.message.clone(), n.severity))
        .collect();
    assert!(
        toasts
            .iter()
            .all(|(message, _)| !message.contains("Pocket 1") && !message.contains("feeds")),
        "no toast repeats the issue row: {toasts:?}"
    );
    assert!(
        toasts
            .iter()
            .any(|(message, _)| message.contains("1 not ready")),
        "the headline counts the skip: {toasts:?}"
    );
    assert!(
        matches!(
            run_state(&controller, id),
            rs_cam_core::session::RunState::NotReady { .. }
        ),
        "the op still reads Not ready"
    );
}

/// An explicit MCP `generate_toolpath` on a not-ready op still refuses with
/// its reason: the agent needs the text.
#[cfg(feature = "mcp")]
#[test]
fn an_explicit_mcp_generate_still_refuses_a_not_ready_op_w2c() {
    let mut controller = sample_controller();
    let id = push_toolpath_with_no_feeds(&mut controller, "Pocket 1");

    controller.pending_mcp = Some(crate::mcp_bridge::PendingMcpCompute::new());
    let (tx, mut rx) = tokio::sync::oneshot::channel();
    controller
        .pending_mcp
        .as_mut()
        .expect("pending_mcp was just set")
        .toolpath
        .insert(id, tx);

    controller.handle_generate_toolpath_mcp(id);

    let response = rx
        .try_recv()
        .expect("the refusal resolves the MCP waiter at once");
    let text = format!("{:?}", response.result);
    assert!(
        text.contains("feed"),
        "the reply names the refused input: {text}"
    );
    assert!(
        controller.state.session().current_run_refusal(id).is_some(),
        "an explicit generate records its refusal"
    );
}
