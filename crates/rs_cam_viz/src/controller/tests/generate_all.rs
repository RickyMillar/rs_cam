//! `generate_all` as a fixpoint: the rest chain, submit-time fail-hard, the
//! lane model, and what a supersede is not.

use super::*;

/// Same waiter-resolution family, different precondition:
/// `FromRemainingStock` (rest machining) with no prior simulated stock.
///
/// A/M11 reclassified this from `Error` to `AwaitingPriorStock` — it is a
/// sequencing state, not a failure — but the MCP waiter must still be
/// resolved immediately, which is what this test was written for.
#[cfg(feature = "mcp")]
#[test]
fn submit_toolpath_compute_missing_prior_stock_resolves_mcp_waiter() {
    let mut controller = sample_controller();
    let tp_id = ToolpathId(0);

    // No prior simulation has run, so `self.state.simulation` has no
    // boundaries/checkpoints — `FromRemainingStock` must fail hard.
    let index = index_of(&controller, tp_id);
    let _ = controller
        .state
        .session
        .apply(Command::SetStockSource(SetStockSourceArgs {
            index,
            source: crate::state::toolpath::StockSource::FromRemainingStock,
        }))
        .expect("the index comes from the session");

    controller.pending_mcp = Some(crate::mcp_bridge::PendingMcpCompute::new());
    let (tx, mut rx) = tokio::sync::oneshot::channel();
    controller
        .pending_mcp
        .as_mut()
        .expect("pending_mcp was just set")
        .toolpath
        .insert(tp_id, tx);

    controller.submit_toolpath_compute(tp_id);

    let response = rx
        .try_recv()
        .expect("submit-time fail-hard must resolve the pending MCP oneshot immediately");
    let payload = response
        .result
        .expect("mcp response should carry an Ok(json) payload describing the error");
    assert!(
        payload.contains("waiting on simulated stock") || payload.contains("remaining stock"),
        "mcp payload should describe the missing-prior-stock block, got: {payload}"
    );

    let rt = controller
        .state
        .gui
        .toolpath_rt
        .get(&tp_id)
        .expect("runtime should exist after the block");
    assert!(
        matches!(
            &rt.status,
            crate::state::toolpath::ComputeStatus::AwaitingPriorStock(_)
        ),
        "A/M11: a missing upstream snapshot is a sequencing state, not an Error —          conflating them is what made 'cannot yet' indistinguishable from          'cannot ever'. Got {:?}",
        rt.status
    );
}

/// A/M11 sentry — the message shape. The pre-A/M11 text ("run a simulation of
/// the preceding operations first, then regenerate") was true and useless: it
/// named no operation, so the operator could not tell a one-round wait from a
/// four-round one. Both message variants must name the blocking op AND its
/// index, and the not-yet-generated variant must warn that the cycle repeats.
#[test]
fn blocked_rest_op_names_its_blocking_upstream_operation() {
    let mut controller = sample_controller();
    // Build a two-op setup: index 0 is the blocker, index 1 is the rest op.
    push_toolpath(&mut controller, "Rest Finish");
    let blocker_id = controller.state.session.toolpath_configs()[0].id;
    let blocker_name = controller.state.session.toolpath_configs()[0].name.clone();
    let rest_id = controller.state.session.toolpath_configs()[1].id;
    let rest_index = index_of(&controller, rest_id);
    let _ = controller
        .state
        .session
        .apply(Command::SetStockSource(SetStockSourceArgs {
            index: rest_index,
            source: crate::state::toolpath::StockSource::FromRemainingStock,
        }))
        .expect("the index comes from the session");

    // Blocker not generated: the wait is at least two rounds.
    controller.submit_toolpath_compute(rest_id);
    let block = controller
        .state
        .gui
        .toolpath_rt
        .get(&rest_id)
        .and_then(|rt| rt.status.blocked_on().cloned())
        .expect("rest op with no prior stock must record AwaitingPriorStock");
    assert_eq!(block.blocking_toolpath_id, Some(blocker_id));
    assert_eq!(block.blocking_toolpath_index, Some(0));
    assert!(
        block.message.contains(&blocker_name),
        "the message must NAME the blocking operation, got: {}",
        block.message
    );
    assert!(
        block.message.contains("index 0"),
        "the message must give the blocker's index, got: {}",
        block.message
    );
    assert!(
        block.message.contains("may need repeating"),
        "when the blocker has not generated, the message must say the cycle may          repeat — that is the number the operator cannot otherwise know. Got: {}",
        block.message
    );

    // Blocker generated: exactly one simulation is enough, and the message
    // must say so rather than repeating the vague ladder warning.
    controller
        .state
        .gui
        .toolpath_rt_or_default(blocker_id)
        .status = crate::state::toolpath::ComputeStatus::Done;
    controller.submit_toolpath_compute(rest_id);
    let block = controller
        .state
        .gui
        .toolpath_rt
        .get(&rest_id)
        .and_then(|rt| rt.status.blocked_on().cloned())
        .expect("still blocked — no simulation has run");
    assert!(
        block.message.contains("ONE simulation"),
        "with the blocker generated the wait is a single round and the message must          say so, got: {}",
        block.message
    );
}

/// A/M11 defect 2 — a disabled op must report `Disabled`, never the error it
/// was carrying when it was switched off. Two ops read `3D Finish 6` and
/// `Rivers (back) (copy)` as broken in the live run when they were merely off.
#[test]
fn a_disabled_op_reports_disabled_not_its_last_error() {
    use crate::state::toolpath::ComputeStatus;
    let stale = ComputeStatus::Error("uses remaining stock but none is available".to_owned());

    let enabled = ComputeStatus::effective(true, &stale);
    assert_eq!(enabled.label(), "Error");
    assert!(enabled.error_text().is_some());

    let disabled = ComputeStatus::effective(false, &stale);
    assert_eq!(disabled.label(), "Disabled");
    assert!(
        disabled.error_text().is_none(),
        "a disabled op must contribute nothing to any error list"
    );
    assert!(disabled.detail().is_none());
    assert!(
        !disabled.needs_generation(),
        "a disabled op is not waiting to be generated"
    );
}

/// A blocked op is not an error and must not appear in `runtime_errors`; a
/// disabled one must appear in neither list. This is the channel an agent
/// triages, so the separation has to hold at the JSON boundary, not just in
/// the enum.
#[cfg(feature = "mcp")]
#[test]
fn diagnostics_separate_blocked_from_failed_and_exclude_disabled() {
    let mut controller = sample_controller();
    push_toolpath(&mut controller, "Rest Finish");
    push_toolpath(&mut controller, "Broken");
    push_toolpath(&mut controller, "Switched Off");
    let ids: Vec<_> = controller
        .state
        .session
        .toolpath_configs()
        .iter()
        .map(|tc| tc.id)
        .collect();

    controller.state.gui.toolpath_rt_or_default(ids[1]).status =
        crate::state::toolpath::ComputeStatus::AwaitingPriorStock(
            rs_cam_core::compute::AwaitingPriorStock {
                blocking_toolpath_id: Some(ids[0]),
                blocking_toolpath_index: Some(0),
                message: "waiting on simulated stock after 'Rough' (index 0)".to_owned(),
            },
        );
    controller.state.gui.toolpath_rt_or_default(ids[2]).status =
        crate::state::toolpath::ComputeStatus::Error("no 3D mesh".to_owned());
    controller.state.gui.toolpath_rt_or_default(ids[3]).status =
        crate::state::toolpath::ComputeStatus::Error("stale text from when it was on".to_owned());
    let off_index = index_of(&controller, ids[3]);
    let _ = controller
        .state
        .session
        .apply(Command::SetToolpathEnabled(SetToolpathEnabledArgs {
            index: off_index,
            enabled: false,
        }))
        .expect("the index comes from the session");

    let diag = controller.build_mcp_diagnostics();
    let errors = diag["runtime_errors"].as_array().expect("runtime_errors");
    let blocked = diag["awaiting_prior_stock"]
        .as_array()
        .expect("awaiting_prior_stock");

    assert_eq!(
        errors.len(),
        1,
        "only the genuinely-failing op belongs in runtime_errors, got: {errors:?}"
    );
    assert_eq!(errors[0]["error"], "no 3D mesh");
    assert_eq!(blocked.len(), 1, "got: {blocked:?}");
    assert_eq!(blocked[0]["blocking_toolpath_index"], 0);

    let rows = diag["per_toolpath"].as_array().expect("per_toolpath");
    let off = rows
        .iter()
        .find(|r| r["toolpath_id"] == serde_json::json!(ids[3]))
        .expect("disabled op should still be listed");
    assert_eq!(off["status"], "Disabled");
    assert!(
        off["error"].is_null(),
        "a disabled op must not present a live-looking error, got: {off}"
    );
}

/// THE A/M11 acceptance gate. A 3-deep rest chain reaches fully generated
/// from cold in ONE `generate_all`, and the call reports how many internal
/// rounds it took.
///
/// Before this, the same project needed three manual sim -> generate rounds
/// and nothing told the operator that `k` was three.
#[cfg(feature = "mcp")]
#[test]
fn generate_all_drives_a_three_deep_rest_chain_to_fixpoint_in_one_call() {
    let mut controller = rest_chain_controller(3);
    let (tx, mut rx) = tokio::sync::oneshot::channel();
    controller.mcp_start_generate_all(true, Some(1.0), tx, None);

    let reply = pump_until_resolved(&mut controller, &mut rx);

    assert_eq!(reply["ok"], true, "reply: {reply}");
    assert_eq!(
        reply["generated"], 4,
        "every op in the chain must end up generated, reply: {reply}"
    );
    assert_eq!(reply["failed"], 0);
    assert!(
        reply["awaiting_prior_stock"]
            .as_array()
            .expect("array")
            .is_empty(),
        "nothing may still be blocked at the fixpoint, reply: {reply}"
    );
    // One round per link plus the initial pass — and the caller is TOLD.
    assert_eq!(reply["rounds"], 4, "reply: {reply}");
    assert_eq!(reply["simulations"], 3, "reply: {reply}");
    assert_eq!(controller.compute.simulations, 3);

    for tc in controller.state.session.toolpath_configs() {
        let rt = controller.state.gui.toolpath_rt.get(&tc.id);
        assert!(
            rt.is_some_and(|rt| matches!(rt.status, crate::state::toolpath::ComputeStatus::Done)),
            "'{}' should be Done at the fixpoint, got {:?}",
            tc.name,
            rt.map(|rt| rt.status.label())
        );
    }
}

/// The loop must stop on a genuinely-failing op instead of spinning: a hard
/// failure is never retried, so condition (a) — "something is blocked purely
/// on sequencing" — goes false and the round count stays bounded.
#[cfg(feature = "mcp")]
#[test]
fn the_fixpoint_loop_terminates_on_a_genuinely_failing_op() {
    let mut controller = rest_chain_controller(3);
    // Poison the middle link. Everything downstream can then never see stock.
    let poisoned = controller.state.session.toolpath_configs()[1].id;
    controller.compute.poison = Some(poisoned);

    let (tx, mut rx) = tokio::sync::oneshot::channel();
    controller.mcp_start_generate_all(true, Some(1.0), tx, None);
    let reply = pump_until_resolved(&mut controller, &mut rx);

    assert_eq!(reply["ok"], false, "a hard failure must not report ok");
    assert_eq!(reply["failed"], 1, "reply: {reply}");
    assert!(
        reply["errors"][0]["message"]
            .as_str()
            .expect("error message")
            .contains("synthetic hard failure"),
        "the final error must be clear about what failed, reply: {reply}"
    );
    let rounds = reply["rounds"].as_u64().expect("rounds");
    assert!(
        (1..=4).contains(&rounds),
        "rounds must stay inside the hard bound (rest ops + 1 = 4), got {rounds}"
    );
    // The ops downstream of the failure are reported as still waiting, each
    // naming what it waits for — not as failures of their own.
    let blocked = reply["awaiting_prior_stock"].as_array().expect("array");
    assert!(
        !blocked.is_empty(),
        "downstream ops are blocked, not broken, reply: {reply}"
    );
}

/// N12 item 10 — the GUI's own simulation reaches the SESSION.
///
/// `ProjectSession::start` reads the rest snapshot from
/// `session.simulation.prior_stocks`. The GUI simulates on its own lane
/// and adopted the answer into viz state alone, so the session held no
/// simulation in the GUI process and `start` refused every
/// `FromRemainingStock` operation there. Two simulation states.
///
/// The arm drives the real doors, in the order the operator does:
///
/// 1. the submit door generates the first link;
/// 2. the GUI's own `run_simulation_with_all` publishes the snapshot;
/// 3. the drain adopts it, into viz state AND into the session.
///
/// It then asserts the two hold ONE snapshot, and that `start` on the
/// rest operation succeeds.
///
/// Pre-fix this fails at the `Arc::ptr_eq` assertion: the session holds
/// no simulation at all, and `start` reports the rest refusal.
///
/// Programme: `planning/arch_consolidation_2026-09-09/IMPLEMENTATION_PLAN.md`
/// §22 addendum. The core half is
/// `crates/rs_cam_core/tests/adopt_simulation_stores_prior_stocks.rs`.
#[cfg(feature = "mcp")]
#[test]
fn a_gui_simulation_reaches_the_session_so_start_sees_the_prior_stock_n12_item10() {
    let mut controller = rest_chain_controller(1);
    let ids: Vec<ToolpathId> = controller
        .state
        .session
        .toolpath_configs()
        .iter()
        .map(|tc| tc.id)
        .collect();
    assert_eq!(
        ids.len(),
        2,
        "the fixture holds one fresh-stock op and one rest op"
    );

    controller.submit_toolpath_compute(ids[0]);
    controller.drain_compute_results();
    assert!(
        controller
            .state
            .gui
            .toolpath_rt
            .get(&ids[0])
            .is_some_and(|rt| rt.result.is_some()),
        "the first link must generate, or the simulation below carves \
         nothing and this arm measures nothing"
    );

    assert!(
        controller.run_simulation_with_all(),
        "the simulation must submit, or this arm measures nothing"
    );
    controller.drain_compute_results();

    let viz_stock = controller
        .state
        .simulation
        .prior_stock_for(ids[1])
        .map(Arc::clone)
        .expect("the GUI adopts the snapshot into viz state");
    let session_stock = controller
        .state
        .session
        .simulation_result()
        .and_then(|simulation| simulation.prior_stocks.get(&ids[1]))
        .map(Arc::clone);
    assert!(
        session_stock
            .as_ref()
            .is_some_and(|stock| Arc::ptr_eq(stock, &viz_stock)),
        "the session and the viewport must hold ONE snapshot; the session \
         holds {}",
        if session_stock.is_some() {
            "another"
        } else {
            "none"
        }
    );

    let cancel = std::sync::atomic::AtomicBool::new(false);
    let refusal = controller
        .state
        .session
        .start(
            rs_cam_core::session::Job::GenerateToolpath(
                rs_cam_core::session::GenerateToolpathArgs { index: 1 },
            ),
            &cancel,
        )
        .err()
        .map(|error| error.to_string());
    assert!(
        refusal.is_none(),
        "start must see the prior stock the GUI simulated; got: {refusal:?}"
    );
}

/// WP17 (tech-debt review H1) — a save keeps the simulation.
///
/// `save_job_to_path` called `ProjectSession::set_post_config` on every
/// save, with the post block the session already held, and that setter
/// wrote `simulation = None`. `ProjectSession::start` reads the rest
/// snapshot from that field (WP11b), so every `FromRemainingStock`
/// operation was refused after a save, and nothing re-adopted.
///
/// The arm drives the real doors, in the order the operator does:
///
/// 1. the submit door generates the first link;
/// 2. the GUI's own simulation publishes the snapshot;
/// 3. the drain adopts it into the session;
/// 4. the Save menu's own function writes the file.
///
/// It then asserts the session still holds the simulation, and that
/// `start` on the rest operation succeeds.
///
/// Pre-fix this fails at the first assertion after the save: the session
/// holds no simulation, and `start` reports the rest refusal.
///
/// The core half is
/// `crates/rs_cam_core/tests/save_keeps_the_simulation_wp17.rs`.
#[cfg(feature = "mcp")]
#[test]
fn a_save_keeps_the_simulation_so_a_rest_op_still_starts_wp17() {
    let mut controller = rest_chain_controller(1);
    let ids: Vec<ToolpathId> = controller
        .state
        .session
        .toolpath_configs()
        .iter()
        .map(|tc| tc.id)
        .collect();
    assert_eq!(
        ids.len(),
        2,
        "the fixture holds one fresh-stock op and one rest op"
    );

    controller.submit_toolpath_compute(ids[0]);
    controller.drain_compute_results();
    assert!(
        controller.run_simulation_with_all(),
        "the simulation must submit, or this arm measures nothing"
    );
    controller.drain_compute_results();
    assert!(
        controller.state.session.simulation_result().is_some(),
        "the control: the session holds a simulation BEFORE the save"
    );

    let path = temp_path("wp17_save_keeps_simulation", "toml");
    controller
        .save_job_to_path(&path)
        .expect("the save writes the fixture file");

    assert!(
        controller.state.session.simulation_result().is_some(),
        "a save keeps the simulation; the rest operations depend on it"
    );
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let refusal = controller
        .state
        .session
        .start(
            rs_cam_core::session::Job::GenerateToolpath(
                rs_cam_core::session::GenerateToolpathArgs { index: 1 },
            ),
            &cancel,
        )
        .err()
        .map(|error| error.to_string());
    assert!(
        refusal.is_none(),
        "the rest operation still starts after a save; got: {refusal:?}"
    );

    let _ = std::fs::remove_file(&path);
}

/// WP17 — the MCP `save_project` conversion writes no session state.
///
/// `app/mcp/commands.rs` states the contract at the top of the file: the
/// conversion step READS the session and turns a wire request into a
/// command. The `save_project` arm mutated instead — it called
/// `set_post_config` — and that write is what dropped the simulation.
///
/// The arm reads the source of the conversion rather than the running
/// app, because `RsCamApp` holds a window and a lane and no test builds
/// one. It measures the discriminator alone: `state_mut()` inside the
/// arm. The repaired arm still READS the session on both sides of the
/// comparison it makes.
///
/// Pre-fix the arm contains `state_mut()`.
#[cfg(feature = "mcp")]
#[test]
fn the_mcp_save_conversion_writes_no_session_state_wp17() {
    const COMMANDS_SRC: &str = include_str!("../../app/mcp/commands.rs");
    let at = COMMANDS_SRC
        .find("CoreRequest::SaveProject(p) => {")
        .expect("core_command_for holds a save_project arm");
    let tail = &COMMANDS_SRC[at..];
    let arm = match tail.find("\n            CoreRequest::") {
        Some(end) => &tail[..end],
        None => tail,
    };
    assert!(
        !arm.contains("state_mut()"),
        "the conversion step reads; it must not write. The save_project \
         arm reads:\n{arm}"
    );
}

/// A/M10's rule, enforced: the loop never picks a simulation resolution for
/// you. Omitting it on a project with rest ops is refused, with instructions.
#[cfg(feature = "mcp")]
#[test]
fn generate_all_refuses_to_guess_a_simulation_resolution() {
    let mut controller = rest_chain_controller(3);
    let (tx, mut rx) = tokio::sync::oneshot::channel();
    controller.mcp_start_generate_all(true, None, tx, None);

    let resp = rx
        .try_recv()
        .expect("the refusal is immediate — nothing is submitted");
    let payload = resp.result.expect("Ok(json)");
    let reply: serde_json::Value = serde_json::from_str(&payload).expect("json");
    assert_eq!(reply["ok"], false);
    let err = reply["error"].as_str().expect("error text");
    assert!(err.contains("simulation_resolution_mm"), "got: {err}");
    assert!(
        err.contains("fixpoint: false"),
        "the refusal must say how to opt out, got: {err}"
    );
    assert!(
        err.contains("NOT guessed"),
        "the refusal must say why, got: {err}"
    );
    assert!(
        controller.drain_events().is_empty(),
        "nothing was submitted"
    );
}

/// `fixpoint: false` is the pre-A/M11 single pass: no simulation, no
/// resolution needed, blocked ops reported rather than retried.
#[cfg(feature = "mcp")]
#[test]
fn fixpoint_false_keeps_the_old_single_pass_behaviour() {
    let mut controller = rest_chain_controller(3);
    let (tx, mut rx) = tokio::sync::oneshot::channel();
    controller.mcp_start_generate_all(false, None, tx, None);
    let reply = pump_until_resolved(&mut controller, &mut rx);

    assert_eq!(reply["rounds"], 1);
    assert_eq!(reply["simulations"], 0);
    assert_eq!(controller.compute.simulations, 0);
    assert_eq!(
        reply["generated"], 1,
        "only the fresh-stock op, reply: {reply}"
    );
    assert_eq!(
        reply["awaiting_prior_stock"]
            .as_array()
            .expect("array")
            .len(),
        3,
        "reply: {reply}"
    );
}

/// A disabled rest op is skipped entirely: not generated, not blocked, not an
/// error — and it does not extend the ladder.
#[cfg(feature = "mcp")]
#[test]
fn a_disabled_rest_op_is_not_generated_blocked_or_failed() {
    let mut controller = rest_chain_controller(3);
    let off = controller.state.session.toolpath_configs()[3].id;
    let off_index = index_of(&controller, off);
    let _ = controller
        .state
        .session
        .apply(Command::SetToolpathEnabled(SetToolpathEnabledArgs {
            index: off_index,
            enabled: false,
        }))
        .expect("the index comes from the session");

    let (tx, mut rx) = tokio::sync::oneshot::channel();
    controller.mcp_start_generate_all(true, Some(1.0), tx, None);
    let reply = pump_until_resolved(&mut controller, &mut rx);

    assert_eq!(reply["generated"], 3, "reply: {reply}");
    assert_eq!(reply["failed"], 0);
    let mentions_off = format!("{reply}").contains(&format!("\"toolpath_id\":{}", off.0));
    assert!(
        !mentions_off,
        "a disabled op must appear in neither the error nor the blocked list, reply: {reply}"
    );
    // And it reports Disabled rather than whatever it last recorded.
    let raw = controller
        .state
        .gui
        .toolpath_rt
        .get(&off)
        .map_or(&crate::state::toolpath::ComputeStatus::Pending, |rt| {
            &rt.status
        });
    assert_eq!(
        crate::state::toolpath::ComputeStatus::effective(false, raw).label(),
        "Disabled"
    );
}

/// THE G-REGEN-RACE gate. `generate_all` issued while the GUI's own
/// auto-regen sweep still has that toolpath in flight must report the work
/// it actually got — not a failure for the job it replaced itself.
#[cfg(feature = "mcp")]
#[test]
fn generate_all_does_not_report_its_own_supersede_as_a_failure() {
    let (mut controller, tp_id) = freshly_loaded_controller();

    // 1. The GUI's own sweep fires first (this is guaranteed after a
    //    load_project: the debounce is 500 ms and every toolpath is stale).
    controller.process_auto_regen();
    assert_eq!(
        controller.compute.active,
        Some(tp_id),
        "the auto-regen sweep should have put this toolpath on the lane"
    );

    // 2. The agent calls generate_all while that job is still running.
    let (tx, mut rx) = tokio::sync::oneshot::channel();
    controller.mcp_start_generate_all(false, None, tx, None);

    let reply = pump_lane_model(&mut controller, &mut rx);

    assert_eq!(reply["ok"], true, "reply: {reply}");
    assert_eq!(
        reply["generated"], 1,
        "the replacement generate_all queued DID produce a toolpath; the reply \
         must count it. Pre-fix this read 0. reply: {reply}"
    );
    assert_eq!(reply["failed"], 0, "reply: {reply}");
    let errors = reply["errors"].as_array().expect("errors array");
    assert!(
        errors.is_empty(),
        "generate_all must not report a failure for the job it superseded \
         itself — pre-fix this carried \"Scallop: generation cancelled\". \
         reply: {reply}"
    );
    assert!(
        controller.superseded_toolpaths.is_empty(),
        "the supersede ledger must be empty once the cancellation it was \
         expecting has drained"
    );
    let rt = controller
        .state
        .gui
        .toolpath_rt
        .get(&tp_id)
        .expect("runtime exists");
    assert!(
        matches!(rt.status, crate::state::toolpath::ComputeStatus::Done),
        "the toolpath really did generate, got {:?}",
        rt.status.label()
    );
}

/// The other half of the same contract, and the reason the fix is a
/// supersede ledger rather than "ignore Cancelled": a cancellation with no
/// supersede behind it — `cancel_generation`, the GUI's cancel button — is
/// still terminal and still reported.
#[cfg(feature = "mcp")]
#[test]
fn a_genuine_cancel_is_still_reported_by_generate_all() {
    let (mut controller, tp_id) = freshly_loaded_controller();
    controller
        .state
        .gui
        .toolpath_rt_or_default(tp_id)
        .stale_since = None;

    let (tx, mut rx) = tokio::sync::oneshot::channel();
    controller.mcp_start_generate_all(false, None, tx, None);

    // Let generate_all's own submit reach the lane, then cancel it the way
    // the escape hatch does — nobody resubmitted, so nothing was superseded.
    let events = controller.drain_events();
    for event in events {
        controller.handle_internal_event(event);
    }
    assert_eq!(controller.compute.active, Some(tp_id));
    assert!(
        controller.superseded_toolpaths.is_empty(),
        "a submit onto an idle lane supersedes nothing"
    );
    controller.compute.cancel_lane(ComputeLane::Toolpath);

    let reply = pump_lane_model(&mut controller, &mut rx);

    assert_eq!(reply["generated"], 0, "reply: {reply}");
    assert_eq!(reply["failed"], 1, "reply: {reply}");
    let errors = reply["errors"].as_array().expect("errors array");
    assert_eq!(errors.len(), 1, "reply: {reply}");
    assert!(
        errors[0]["message"]
            .as_str()
            .unwrap_or_default()
            .contains("cancelled"),
        "a real cancellation must still say so: {reply}"
    );
}

/// Interactive auto-regen keeps working, and the supersede path improves
/// it: a param edit while the previous generate is still running no longer
/// bounces the toolpath's status through `Pending` (which the operations
/// tree renders as "not generated") on its way to the new result.
#[cfg(feature = "mcp")]
#[test]
fn a_param_edit_mid_generate_regenerates_without_a_pending_flicker() {
    let (mut controller, tp_id) = freshly_loaded_controller();

    // First edit: the sweep puts it on the lane.
    controller.process_auto_regen();
    assert_eq!(controller.compute.active, Some(tp_id));

    // Second edit lands while that job runs — the GUI marks it stale again
    // and the next sweep resubmits it.
    controller
        .state
        .gui
        .toolpath_rt_or_default(tp_id)
        .stale_since = Some(std::time::Instant::now() - std::time::Duration::from_millis(600));
    controller.process_auto_regen();
    assert!(
        controller.superseded_toolpaths.contains(&tp_id),
        "resubmitting the running toolpath supersedes it"
    );

    // The abandoned job reports; the toolpath is still computing.
    controller.compute.finish_active();
    controller.drain_compute_results();
    let status = controller
        .state
        .gui
        .toolpath_rt
        .get(&tp_id)
        .map(|rt| rt.status.label());
    assert_eq!(
        status,
        Some("Computing"),
        "a superseded job is not an outcome — the replacement is already \
         queued, so the toolpath is still computing"
    );

    // The replacement lands and the edit is honoured.
    controller.compute.finish_active();
    controller.drain_compute_results();
    let rt = controller
        .state
        .gui
        .toolpath_rt
        .get(&tp_id)
        .expect("runtime exists");
    assert!(
        matches!(rt.status, crate::state::toolpath::ComputeStatus::Done),
        "auto-regen must still deliver a fresh result, got {:?}",
        rt.status.label()
    );
    assert!(
        rt.stale_since.is_none(),
        "the submit clears the staleness it satisfies"
    );
}

/// The manual-regeneration arm of the same race — F2.4, G-LATERESULT.
///
/// The test above covers the AUTO arm, and that arm was already safe by a
/// route that has nothing to do with freshness: a second edit resubmits, the
/// resubmit supersedes, and G-REGEN-RACE drops the abandoned result before it
/// reaches the store. **Nothing supersedes on a 3D manual-regen operation.**
/// The operator presses G, edits a parameter while the long generation runs,
/// and the result that lands was computed from the parameter set they just
/// left. It was written into `session.results`, which is what
/// `FreshnessState` reads for `Current`, so every surface F2.2 wired reported
/// the edit as answered and the export gate F2.3 built would have let it
/// through.
///
/// The row's requirement is "stored but marked stale, never shown as
/// current", and all three halves are asserted here: the geometry survives on
/// `rt.result` (a discarded result costs a long generation and explains
/// nothing), the core slot stays empty, and the state reads `EditedSince`.
#[cfg(feature = "mcp")]
#[test]
fn a_param_edit_mid_generate_manual_arm_keeps_the_edit_g_lateresult() {
    let (mut controller, tp_id) = freshly_loaded_controller();
    // The arm under test: 3D ops default to manual regeneration, so no sweep
    // will resubmit and nothing will supersede.
    controller
        .state
        .gui
        .toolpath_rt_or_default(tp_id)
        .auto_regen = false;
    controller
        .state
        .gui
        .toolpath_rt_or_default(tp_id)
        .stale_since = None;

    // The operator presses G. One job, on the lane.
    controller.handle_internal_event(AppEvent::GenerateToolpath(tp_id));
    assert_eq!(
        controller.compute.active,
        Some(tp_id),
        "the manual generate must reach the lane"
    );
    // WP11b: the revision rides the HANDLE, not a GUI runtime field. The
    // lane holding this toolpath is the evidence that a job carrying it is
    // in flight.

    // While it runs, they change a parameter through the panel's own
    // write-back — the same door every inspector edit goes through.
    panel_edit(&mut controller, tp_id, |entry| {
        entry.operation.set_feed_rate(1234.0);
    });
    assert!(
        controller.state.session.get_result(0).is_none(),
        "the edit drops the core result (F2.1); this test is about what \
         happens when the in-flight job then lands"
    );
    assert_eq!(
        state_of(&controller, 0),
        FreshnessState::Regenerating,
        "while the lane holds it the honest state is Regenerating — the lane's \
         status precedes the cache check in `freshness`, and the operator is \
         told work is in progress rather than that it is stale"
    );

    // The job finishes and reports. Nothing superseded it.
    controller.compute.finish_active();
    controller.drain_compute_results();

    assert!(
        !controller.superseded_toolpaths.contains(&tp_id),
        "no resubmit happened, so this is not the G-REGEN-RACE path"
    );
    let rt = &controller.state.gui.toolpath_rt[&tp_id];
    assert!(
        matches!(rt.status, crate::state::toolpath::ComputeStatus::Done),
        "the generation finished and the status says so, got {:?}",
        rt.status.label()
    );
    assert!(
        rt.result.is_some(),
        "STORED, not discarded: the geometry stays drawable so the operator \
         does not lose a long 3D generation and get told nothing"
    );
    assert!(
        controller.state.session.get_result(0).is_none(),
        "but NOT into the core cache — that is what `Current` is read from, \
         and this result answers the parameter set the operator just left"
    );
    assert_eq!(
        state_of(&controller, 0),
        FreshnessState::EditedSince,
        "so the card, the header, the chips and the export gate all still say \
         the edit is unanswered"
    );
}

/// The other side of the same guard: with no edit in flight, a manual
/// generate's result IS accepted. A guard that rejected everything would pass
/// the test above and break the application.
#[cfg(feature = "mcp")]
#[test]
fn a_manual_generate_with_no_edit_is_accepted_g_lateresult() {
    let (mut controller, tp_id) = freshly_loaded_controller();
    controller
        .state
        .gui
        .toolpath_rt_or_default(tp_id)
        .auto_regen = false;
    controller
        .state
        .gui
        .toolpath_rt_or_default(tp_id)
        .stale_since = None;

    controller.handle_internal_event(AppEvent::GenerateToolpath(tp_id));
    controller.compute.finish_active();
    controller.drain_compute_results();

    assert!(
        controller.state.session.get_result(0).is_some(),
        "an unedited generation must land in the core cache"
    );
    assert_eq!(state_of(&controller, 0), FreshnessState::Current);
}
