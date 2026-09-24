//! G-STALECARDS — the cut-metric cards read "simulation trace is stale"
//! right after a fresh simulation.
//!
//! Operator report, 2026-09-24: Face (fresh stock) + 3D Rough
//! (`from_remaining_stock`), "Capture cutting metrics" on, Run Simulation.
//! The drawer plots show data, but every card reads
//! `UnmodeledReason::StaleSimulation` and a re-run does not clear it.
//!
//! The tests drive the real threaded backend through the GUI door: open
//! the project, Generate All (the plan), turn on metric capture, Run
//! Simulation, then read the cards the way `ui/sim_diagnostics.rs` reads
//! them (`SimulationState::cached_cut_metrics`).
//!
//! Cause (stage-1 diagnosis). An edit drops the core result of the edited
//! operation (`ReplaceToolpathConfig`), but the GUI keeps its runtime copy
//! for the stale display. A 3D operation has no auto-regeneration, so the
//! copy stays. `build_simulation_groups` simulates the GUI copy
//! (`gui.toolpath_rt[..].result`), the core adopts the run, and the GUI
//! freshness (`state::freshness::simulation_freshness`) reads `Current`.
//! `rs_cam_core::gcode::sim_trace_is_fresh` finds no core result for the
//! operation and reads the trace as stale, so every gate of EVERY
//! operation is rewritten to `StaleSimulation`. A re-run repeats the same
//! steps, so "re-run simulation" can never clear the cards.
//!
//! The plain flow (no edit) is sound: `a_plain_generate_and_simulate_...`
//! is the control and passes.

use super::*;

use rs_cam_core::tool_load::UnmodeledReason;
use rs_cam_core::tool_load::distribution::{DistributionOutcome, NotMeasuredReason};

fn stale_cards_fixture() -> std::path::PathBuf {
    fixture_path("stale_cards").join("face_and_rest_rough.toml")
}

/// One pump, in the order `app.rs` runs it off the frame.
fn pump_once(controller: &mut AppController) {
    controller.drain_compute_results();
    controller.process_auto_regen();
    for event in controller.drain_events() {
        controller.handle_internal_event(event);
    }
}

fn lanes_idle(controller: &AppController) -> bool {
    controller
        .lane_snapshots()
        .iter()
        .all(|snapshot| snapshot.state == LaneState::Idle)
}

/// Pump until no plan runs and every lane is idle, twice in a row.
fn pump_until_settled(controller: &mut AppController, what: &str) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(600);
    let mut quiet = 0;
    loop {
        pump_once(controller);
        if !controller.plan_is_busy() && lanes_idle(controller) {
            quiet += 1;
            if quiet >= 3 {
                return;
            }
        } else {
            quiet = 0;
        }
        assert!(std::time::Instant::now() < deadline, "{what}: timed out");
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
}

/// A copy of `rs_cam_core::compute::simulate::hash_toolpath` (it is
/// `pub(crate)`), for the evidence print only.
fn hash_toolpath(toolpath: &Toolpath) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    toolpath.moves.len().hash(&mut hasher);
    for motion in &toolpath.moves {
        motion.target.x.to_bits().hash(&mut hasher);
        motion.target.y.to_bits().hash(&mut hasher);
        motion.target.z.to_bits().hash(&mut hasher);
        match motion.move_type {
            rs_cam_core::toolpath::MoveType::Rapid => 0_u8.hash(&mut hasher),
            rs_cam_core::toolpath::MoveType::Linear { feed_rate } => {
                1_u8.hash(&mut hasher);
                feed_rate.to_bits().hash(&mut hasher);
            }
            rs_cam_core::toolpath::MoveType::ArcCW { i, j, feed_rate } => {
                2_u8.hash(&mut hasher);
                i.to_bits().hash(&mut hasher);
                j.to_bits().hash(&mut hasher);
                feed_rate.to_bits().hash(&mut hasher);
            }
            rs_cam_core::toolpath::MoveType::ArcCCW { i, j, feed_rate } => {
                3_u8.hash(&mut hasher);
                i.to_bits().hash(&mut hasher);
                j.to_bits().hash(&mut hasher);
                feed_rate.to_bits().hash(&mut hasher);
            }
        }
    }
    hasher.finish()
}

/// Print both sides of every freshness comparison `sim_trace_is_fresh`
/// makes, so a failure names the value that moved.
fn freshness_evidence(controller: &AppController) -> String {
    let session = &controller.state.session;
    let mut out = String::new();
    let viz_trace = controller
        .state
        .simulation
        .results
        .as_ref()
        .and_then(|results| results.cut_trace.as_ref());
    let core_trace = session
        .simulation_result()
        .and_then(|simulation| simulation.cut_trace.as_ref());
    out.push_str(&format!(
        "viz trace: {}, core trace: {}, same Arc: {}\n",
        viz_trace.is_some(),
        core_trace.is_some(),
        match (viz_trace, core_trace) {
            (Some(a), Some(b)) => Arc::ptr_eq(a, b).to_string(),
            _ => "n/a".to_owned(),
        }
    ));
    let Some(trace) = viz_trace else {
        return out;
    };
    let Some(provenance) = trace.provenance.as_ref() else {
        out.push_str("viz trace carries NO provenance\n");
        return out;
    };
    for (idx, tc) in session.toolpath_configs().iter().enumerate() {
        let session_hash = session
            .get_result(idx)
            .map(|result| hash_toolpath(&result.annotated().toolpath));
        let gui_hash = controller
            .state
            .gui
            .toolpath_rt
            .get(&tc.id)
            .and_then(|rt| rt.result.as_ref())
            .map(|result| hash_toolpath(&result.annotated.toolpath));
        let cfg_now = rs_cam_core::compute::simulate::hash_operation_config(&tc.operation);
        out.push_str(&format!(
            "tp {} '{}' enabled={} provenance.toolpath={:?} session.result={:?} \
             gui.rt.result={:?} provenance.config={:?} config.now={}\n",
            tc.id.0,
            tc.name,
            tc.enabled,
            provenance.toolpath_hashes.get(&tc.id),
            session_hash,
            gui_hash,
            provenance.operation_config_hashes.get(&tc.id),
            cfg_now,
        ));
    }
    out.push_str(&format!(
        "sim_trace_is_fresh(session, viz trace) = {}\n",
        rs_cam_core::gcode::sim_trace_is_fresh(session, trace)
    ));
    out
}

/// The cards of `toolpath_id` that read `StaleSimulation`. Asserts that
/// the set is not empty, so the check is not vacuous.
fn stale_cards(controller: &mut AppController, toolpath_id: ToolpathId) -> Vec<String> {
    let edit_counter = controller.state.gui.edit_counter;
    let AppState {
        session,
        simulation,
        ..
    } = &mut controller.state;
    let set = simulation.cached_cut_metrics(session, edit_counter, toolpath_id);
    assert!(
        !set.cards.is_empty(),
        "no cards: the check would be vacuous"
    );
    set.cards
        .iter()
        .filter(|card| {
            matches!(
                card.outcome,
                DistributionOutcome::NotMeasured(NotMeasuredReason::Unmodeled(
                    UnmodeledReason::StaleSimulation
                ))
            )
        })
        .map(|card| format!("{:?}", card.metric))
        .collect()
}

/// Open the fixture, turn on metric capture at 0.5 mm, run Generate All
/// to the end. Returns the controller and the id of the rest rough.
fn generated_fixture() -> (AppController, ToolpathId) {
    let mut controller = AppController::new();
    controller
        .open_job_from_path(&stale_cards_fixture())
        .expect("the fixture opens");
    controller.state.simulation.auto_resolution = false;
    controller.state.simulation.resolution = 0.5;
    controller.state.simulation.set_metric_capture_enabled(true);
    controller.handle_generate_all();
    if controller.pending_plan_confirm.is_some() {
        controller.accept_plan_resolution();
    }
    pump_until_settled(&mut controller, "generate all");
    let rough = controller
        .state
        .session
        .toolpath_configs()
        .iter()
        .find(|tc| tc.stock_source == rs_cam_core::compute::config::StockSource::FromRemainingStock)
        .map(|tc| tc.id)
        .expect("the fixture holds a rest rough");
    (controller, rough)
}

/// The control: with no edit, a fresh simulation gives measured cards.
#[test]
#[ignore = "slow real-lane control for G-STALECARDS"]
fn a_plain_generate_and_simulate_gives_measured_cards_g_stalecards() {
    let (mut controller, rough) = generated_fixture();
    for run in 1..=2 {
        controller.handle_internal_event(AppEvent::RunSimulation);
        pump_until_settled(&mut controller, "run simulation");
        let evidence = freshness_evidence(&controller);
        assert!(
            !controller.state.simulation_is_stale(),
            "run {run}: the GUI freshness reads stale\n{evidence}"
        );
        let stale = stale_cards(&mut controller, rough);
        assert!(
            stale.is_empty(),
            "run {run}: stale cards {stale:?}\n{evidence}"
        );
    }
}

/// G-STALECARDS: edit the rough (a feed change, as the Feeds tab makes it)
/// and run the simulation without a regenerate. The GUI calls the result
/// `Current`, and the cards call the same trace stale. The two answers must
/// agree. Stage 2 decides which side moves.
#[test]
#[ignore = "reproduces G-STALECARDS; fixed in stage 2"]
fn the_cards_and_the_gui_agree_on_a_simulation_that_just_landed_g_stalecards() {
    let (mut controller, rough) = generated_fixture();
    panel_edit(&mut controller, rough, |entry| {
        let feed = entry.operation.feed_rate();
        entry.operation.set_feed_rate(feed + 100.0);
    });
    pump_until_settled(&mut controller, "after the edit");
    let index = index_of(&controller, rough);
    let runtime = controller.state.gui.toolpath_rt.get(&rough).expect("row");
    assert!(
        !runtime.auto_regen
            && runtime.result.is_some()
            && controller.state.session.get_result(index).is_none(),
        "precondition: the core dropped the rough and the GUI kept its copy"
    );

    for run in 1..=2 {
        controller.handle_internal_event(AppEvent::RunSimulation);
        pump_until_settled(&mut controller, "run simulation");
        let evidence = freshness_evidence(&controller);
        let gui_current = !controller.state.simulation_is_stale();
        let stale = stale_cards(&mut controller, rough);
        assert!(
            !(gui_current && !stale.is_empty()),
            "G-STALECARDS run {run}: the GUI reads the simulation that just \
             landed as {:?}, and the cut-metric cards read it as stale \
             ({stale:?}). \"Re-run simulation\" cannot clear this.\n{evidence}",
            controller.state.simulation_freshness(),
        );
    }
}
