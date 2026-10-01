//! G-STALECARDS — the cut-metric cards read "simulation trace is stale"
//! right after a fresh simulation.
//!
//! Operator report, 2026-09-24: Face (fresh stock) + 3D Rough
//! (`from_remaining_stock`), cutting metrics captured, Run Simulation.
//! The drawer plots show data, but every card reads
//! `UnmodeledReason::StaleSimulation` and a re-run does not clear it.
//!
//! The tests drive the real threaded backend through the GUI door: open
//! the project, Generate All (the plan), Run Simulation, then read the
//! cards the way `ui/sim_diagnostics.rs` reads them
//! (`SimulationState::cached_cut_metrics`).
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

/// The wall-clock budget of ONE test, shared by all its pumps.
///
/// Measured 2026-10-02: the three tests of this file (the ignored control
/// included) finish in 3.5 s to 3.8 s together on an idle machine, and in
/// 15 s to 34 s beside 24 busy processes and a cargo build. 120 s keeps a
/// margin of more than 3x over the worst loaded run. A wait that stays busy for longer is a hang, and the test must
/// fail with the lane state, not hold the cargo lane for minutes.
const TEST_BUDGET: std::time::Duration = std::time::Duration::from_secs(120);

/// The time that the controller must stay settled before a pump returns.
///
/// A lane writes `Idle` BEFORE it sends its result, so an idle lane can
/// still have one result in the channel. The pump drains on each pass, and
/// this hold gives a late send time to arrive.
const SETTLE_HOLD: std::time::Duration = std::time::Duration::from_millis(150);

/// The deadline that all pumps of one test share.
struct SettleBudget {
    deadline: std::time::Instant,
}

impl SettleBudget {
    fn start() -> Self {
        Self {
            deadline: std::time::Instant::now() + TEST_BUDGET,
        }
    }
}

fn lanes_idle(controller: &AppController) -> bool {
    controller
        .lane_snapshots()
        .iter()
        .all(|snapshot| snapshot.state == LaneState::Idle && snapshot.queue_depth == 0)
}

/// The toolpaths whose runtime row still reads `Computing`.
fn computing_toolpaths(controller: &AppController) -> Vec<ToolpathId> {
    controller
        .state
        .gui
        .toolpath_rt
        .iter()
        .filter(|(_, rt)| matches!(rt.status, crate::state::runtime::ComputeStatus::Computing))
        .map(|(id, _)| *id)
        .collect()
}

/// True when no work is in flight on the controller side or on a lane.
///
/// - No plan runs and no plan waits for the resolution question.
/// - Every lane is idle with an empty queue.
/// - No toolpath row waits for a result (`Computing`).
/// - No simulation waits for a result. Every adopt arm takes the submit
///   stamp, so a stamp that stays set means that a result has not landed.
fn settled(controller: &AppController) -> bool {
    !controller.plan_is_busy()
        && lanes_idle(controller)
        && computing_toolpaths(controller).is_empty()
        && controller
            .state
            .simulation
            .submitted_simulation_epoch
            .is_none()
}

/// The state of each lane and of the controller, for a timeout message.
fn settle_evidence(controller: &AppController) -> String {
    let mut out = String::new();
    for snapshot in controller.lane_snapshots() {
        out.push_str(&format!(
            "lane {:?}: state={:?} queue={} job={:?} phase={:?} elapsed={:?}\n",
            snapshot.lane,
            snapshot.state,
            snapshot.queue_depth,
            snapshot.current_job,
            snapshot.current_phase,
            snapshot.elapsed(),
        ));
    }
    out.push_str(&format!(
        "plan: busy={} pending_confirm={} cursor={:?} steps={:?} in_flight={:?}\n",
        controller.plan_is_busy(),
        controller.pending_plan_confirm.is_some(),
        controller.plan.as_ref().map(|plan| plan.cursor),
        controller.plan.as_ref().map(|plan| plan.steps.len()),
        controller.plan.as_ref().map(|plan| plan.in_flight),
    ));
    out.push_str(&format!(
        "toolpaths computing: {:?}\nsimulation submit stamp: {:?}\n",
        computing_toolpaths(controller),
        controller.state.simulation.submitted_simulation_epoch,
    ));
    out
}

/// Pump until the controller is settled (see [`settled`]) for
/// [`SETTLE_HOLD`] and for at least three pumps.
///
/// The pump panics when `budget` runs out. Before the panic it cancels every
/// lane: the backend `Drop` joins each lane thread and does not cancel the
/// job that runs, so without the cancel the unwind waits for that job.
fn pump_until_settled(controller: &mut AppController, budget: &SettleBudget, what: &str) {
    let mut quiet_pumps = 0;
    let mut quiet_since: Option<std::time::Instant> = None;
    loop {
        pump_once(controller);
        if settled(controller) {
            quiet_pumps += 1;
            let since = *quiet_since.get_or_insert_with(std::time::Instant::now);
            if quiet_pumps >= 3 && since.elapsed() >= SETTLE_HOLD {
                return;
            }
        } else {
            quiet_pumps = 0;
            quiet_since = None;
        }
        if std::time::Instant::now() >= budget.deadline {
            let evidence = settle_evidence(controller);
            controller.compute.cancel_all();
            panic!(
                "{what}: the controller did not settle within the {TEST_BUDGET:?} test \
                 budget\n{evidence}"
            );
        }
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
    let AppState {
        session,
        simulation,
        ..
    } = &mut controller.state;
    let set = simulation.cached_cut_metrics(session, toolpath_id);
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

/// Open the fixture at 0.5 mm and run Generate All to the end. Returns the
/// controller, the id of the rest rough and the budget of the test.
fn generated_fixture() -> (AppController, ToolpathId, SettleBudget) {
    let budget = SettleBudget::start();
    // The fixture turns on the debug trace of each toolpath. A unit-test
    // build resolves the default artifact folder under the temp folder
    // (`ArtifactPolicy::from_settings`), so those files never reach the
    // operator's cache folder.
    let mut controller = AppController::new();
    controller
        .open_job_from_path(&stale_cards_fixture())
        .expect("the fixture opens");
    // G-RESTRES: the ONE stored project resolution.
    let _ = controller
        .set_simulation_resolution(rs_cam_core::session::SimulationResolution::Fixed(0.5))
        .expect("a positive cell");
    controller.handle_generate_all();
    if controller.pending_plan_confirm.is_some() {
        controller.accept_plan_resolution();
    }
    pump_until_settled(&mut controller, &budget, "generate all");
    let rough = controller
        .state
        .session
        .toolpath_configs()
        .iter()
        .find(|tc| tc.stock_source == rs_cam_core::compute::config::StockSource::FromRemainingStock)
        .map(|tc| tc.id)
        .expect("the fixture holds a rest rough");
    (controller, rough, budget)
}

/// Operator ruling 2026-10-02 ("always capture"): a simulation that the GUI
/// door builds keeps its cut trace with the arc engagement, in the view and
/// in the core. No capture control exists, and no state can switch it off.
#[test]
fn a_gui_simulation_always_keeps_the_cut_trace_w4l() {
    let (mut controller, _rough, budget) = generated_fixture();
    controller.handle_internal_event(AppEvent::RunSimulation);
    pump_until_settled(&mut controller, &budget, "run simulation");

    let results = controller
        .state
        .simulation
        .results
        .as_ref()
        .expect("the run landed in the view");
    let trace = results
        .cut_trace
        .as_ref()
        .expect("every GUI run keeps the cut trace");
    assert!(
        trace
            .provenance
            .as_ref()
            .is_some_and(|provenance| provenance.captured_arc_engagement),
        "every GUI run captures the arc engagement"
    );
    assert!(
        !trace.samples.is_empty(),
        "non-vacuity: the fixture cuts, so the trace holds samples"
    );
    assert!(
        controller
            .state
            .session
            .simulation_result()
            .is_some_and(|sim| sim.cut_trace.is_some()),
        "the core adopts the same run with its trace"
    );
}

/// The control: with no edit, a fresh simulation gives measured cards.
#[test]
#[ignore = "slow real-lane control for G-STALECARDS"]
fn a_plain_generate_and_simulate_gives_measured_cards_g_stalecards() {
    let (mut controller, rough, budget) = generated_fixture();
    for run in 1..=2 {
        controller.handle_internal_event(AppEvent::RunSimulation);
        pump_until_settled(&mut controller, &budget, "run simulation");
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
fn the_cards_and_the_gui_agree_on_a_simulation_that_just_landed_g_stalecards() {
    let (mut controller, rough, budget) = generated_fixture();
    panel_edit(&mut controller, rough, |entry| {
        let feed = entry.operation.feed_rate();
        entry.operation.set_feed_rate(feed + 100.0);
    });
    pump_until_settled(&mut controller, &budget, "after the edit");
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
        pump_until_settled(&mut controller, &budget, "run simulation");
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
        // Stage 2 (2): the GUI names the operation to regenerate, not
        // "re-run simulation".
        assert_eq!(
            controller.state.simulation_freshness(),
            crate::state::freshness::SimFreshness::Ungenerated(rough),
            "run {run}: the freshness must name the rough\n{evidence}"
        );
        // Stage 2 (3): the stale rewrite is per operation. The Face did
        // not change, so its cards keep their measured verdicts.
        let face = controller
            .state
            .session
            .toolpath_configs()
            .iter()
            .find(|tc| tc.id != rough && tc.enabled)
            .map(|tc| tc.id)
            .expect("the fixture holds a face");
        let face_stale = stale_cards(&mut controller, face);
        assert!(
            face_stale.is_empty(),
            "run {run}: the unchanged Face reads stale ({face_stale:?})\n{evidence}"
        );
        // The core load report: the Face row keeps its own verdict; only
        // the rough's row is rewritten to StaleSimulation.
        let trace = controller
            .state
            .session
            .simulation_result()
            .and_then(|sim| sim.cut_trace.clone());
        let report =
            rs_cam_core::gcode::project_load_report(&controller.state.session, trace.as_deref());
        let stale_rows: Vec<ToolpathId> = report
            .per_toolpath
            .iter()
            .filter(|verdict| {
                matches!(
                    verdict.chipload,
                    rs_cam_core::tool_load::verdict::ChiploadVerdict::Unmodeled {
                        reason: UnmodeledReason::StaleSimulation,
                        ..
                    }
                )
            })
            .map(|verdict| verdict.toolpath_id)
            .collect();
        assert_eq!(
            stale_rows,
            vec![rough],
            "run {run}: only the rough's row reads stale\n{evidence}"
        );
    }
}
