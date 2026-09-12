//! WP14b — `optimize_toolpath` is a `Job` row over a CLONED session.
//!
//! Programme:
//! `planning/arch_consolidation_2026-09-09/IMPLEMENTATION_PLAN.md` §24
//! ruling 2 and §28 items 4 to 8.
//!
//! # The finding this file pins
//!
//! The optimizer mutates the session it scores against. Each candidate
//! writes the toolpath's parameters, regenerates it and re-simulates the
//! project, and a `BaselineRestoreGuard` puts the parameters back on
//! drop. The GUI therefore handed its whole session to the Optimize lane
//! with `std::mem::replace` and held `ProjectSession::new_empty()` until
//! the lane gave it back. Three call sites did that, and every GUI panel
//! drew against the placeholder for the length of the run.
//!
//! The guard restores the PARAMETERS and nothing else. Two session slots
//! stay dropped after the call, because
//! `apply_toolpath_param_snapshot_narrow` ends with `drop_result(index)`
//! and `session.simulation = None`:
//!
//! - the toolpath's cached result, and the revision that names it;
//! - the project simulation.
//!
//! That residue is the measurable half of "the optimizer writes the live
//! session". A comparison of `toolpath_configs()` before and after
//! PASSES on the pre-fix code and measures nothing, so this file does not
//! make one on its own.
//!
//! WP14b makes the row a `Job`. Step (i) clones the session into the
//! handle and takes the baseline trace off the session beside it; step
//! (ii) runs the candidate search on the clone; there is no step (iii).
//! The live session keeps its result, its revision and its simulation.
//!
//! # Red-first
//!
//! Every arm here is COMPILE-RED at the parent revision. None of
//! `CommandId::OptimizeToolpath`, `Job::OptimizeToolpath`,
//! `OptimizeToolpathArgs`, `OptimizeToolpathHandle`,
//! `JobHandle::OptimizeToolpath`, `execute_optimize_toolpath` or
//! `ProjectSession: Clone` exists there. Red is a failed build of this
//! test binary, one error per missing name — the shape WP14a's sentry
//! red had.
//!
//! # The arms
//!
//! | Arm | What it measures |
//! |---|---|
//! | (a) | the registry row declares `Job` and the wire name `optimize_toolpath` |
//! | (b) | step (ii) coerces to a `fn` pointer and runs after the live session is dropped |
//! | (c) | the job's outcome equals the inline call's on one fixture |
//! | (d) | the live session keeps its result, its revision and its simulation |
//! | (e) | the session clone's wall clock on the P0 fixture, reported through an assertion |
//!
//! # The fixtures
//!
//! **Fixture R — the refusal fixture.** `Material::Custom` refuses at
//! step 3 of `optimize_toolpath_inner`, after the evaluation context is
//! built and before any candidate is generated, so it costs no
//! simulation. It carries a hand-built `SimulationResult` whose only
//! populated field is the cut trace, adopted through
//! `Command::AdoptSimulation`, because step (i) reads the trace off the
//! SESSION (§28 ruling 5). Arms (a), (b) and (c) ride it.
//!
//! **Fixture G — the guard fixture.** Arm (d) needs the guard to run, and
//! only a fixture that passes every pre-flight reaches it. This one is a
//! 40 mm square pocket in a 44 x 44 x 6 mm board with one depth pass and
//! a Ø6 mm end mill. It is generated and simulated once, through the
//! production doors. Arms (d) and (e) ride it.
//!
//! # NOT MEASURED
//!
//! - Arm (e) records a duration. The bound in its assertion is NOT a
//!   threshold and nothing is tuned against it. §24 ruling 2 asks the
//!   verifier for the number, and an assertion message is how a test in
//!   this tree reports one: `clippy::print_stdout` and
//!   `clippy::print_stderr` are denied in tests too.
//! - The GUI lane, the MCP arm and the three deleted `mem::replace`
//!   sites. A core test cannot see them. The viz half is
//!   `crates/rs_cam_viz/tests/optimize_runs_on_the_job_lane_wp14b.rs`.
//! - The wall clock of Fixture G's own candidate search. This file was
//!   written by a lane that runs no cargo.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use common::make_endmill_6mm;
use common::session::{
    generate, polygon_model, single_op_session_with, square_polygon, stock_under,
};
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::PocketConfig;
use rs_cam_core::compute::simulate::SimulationResult;
use rs_cam_core::feed_modulation::ModulationStrategy;
use rs_cam_core::session::{
    AdoptSimulationArgs, Command, CommandId, CommandKind, Job, JobHandle, OptimizeToolpathArgs,
    OptimizeToolpathHandle, ProjectSession, SimulationOptions, execute_optimize_toolpath,
};
use rs_cam_core::simulation_cut::SimulationCutTrace;
use rs_cam_core::stock_mesh::StockMesh;
use rs_cam_core::tool_load::optimize::{OptimizeOutcome, OutcomeKind, optimize_toolpath};

// ── constants ────────────────────────────────────────────────────────

/// Half-extent of the square model, in mm.
const HALF_MM: f64 = 20.0;

/// Board thickness, in mm. The board hangs below `z = 0`, the frame a 2D
/// operation cuts in.
const STOCK_Z_MM: f64 = 6.0;

/// The dexel cell the guard fixture simulates at, in mm. Coarse on
/// purpose: the arm measures which session slots move, not a cut.
const SIM_CELL_MM: f64 = 1.0;

/// A generous non-gating bound on the session clone, for arm (e).
///
/// NOT a threshold. §24 ruling 2 names one second as the point at which
/// the handle would take the simulation by `Arc` instead of copying the
/// display mesh, and this arm reports the measured duration in its own
/// message whether it trips or not.
const CLONE_REPORT_BOUND: Duration = Duration::from_secs(1);

// ── fixtures ─────────────────────────────────────────────────────────

/// The pocket both fixtures machine.
fn pocket_op() -> OperationConfig {
    OperationConfig::Pocket(PocketConfig {
        stepover: 2.0,
        depth: 3.0,
        depth_per_pass: 3.0,
        ..PocketConfig::default()
    })
}

/// The candidate search's own simulation options, at a coarse cell.
fn sim_options() -> SimulationOptions {
    SimulationOptions {
        resolution: SIM_CELL_MM,
        skip_ids: Vec::new(),
        metrics_enabled: true,
        auto_resolution: false,
        use_predicted_feed_in_gates: false,
        adaptive_feed_modulation: false,
        modulation_strategy: ModulationStrategy::ConstrainedMax,
        modulation_aggressiveness: 1.0,
    }
}

/// One stock, one Ø6 mm end mill, one square model, one pocket.
fn base_session() -> ProjectSession {
    single_op_session_with(
        stock_under(HALF_MM, STOCK_Z_MM),
        make_endmill_6mm(),
        polygon_model(vec![square_polygon(HALF_MM)], "square"),
        "Pocket",
        pocket_op(),
        |_| {},
    )
}

/// A `SimulationResult` that carries a cut trace and nothing else.
///
/// `Command::AdoptSimulation` validates no field, so the cheapest way to
/// put a trace on the session is to build the record around it.
fn result_carrying(trace: SimulationCutTrace) -> SimulationResult {
    SimulationResult {
        mesh: StockMesh::empty(),
        total_moves: 0,
        deviations: None,
        column_deviations: None,
        boundaries: Vec::new(),
        checkpoints: Vec::new(),
        rapid_collisions: Vec::new(),
        rapid_collision_move_indices: Vec::new(),
        cut_trace: Some(Arc::new(trace)),
        resolution_clamped: false,
        column_grid_cell_mm: SIM_CELL_MM,
        prior_stocks: std::collections::HashMap::new(),
    }
}

/// Fixture R — a session the optimizer refuses at step 3.
///
/// The stock material is `Custom`, so the search never generates a
/// candidate. The session still carries a trace, because step (i) reads
/// one off the session and refuses without it.
fn refusal_session() -> ProjectSession {
    let mut session = base_session();
    let mut stock = session.stock_config().clone();
    stock.material = rs_cam_core::material::Material::Custom {
        name: "wp14b refusal lever".to_owned(),
        feed_scale_factor: 1.0,
        kc: 10.0,
    };
    let _ = session.set_stock_config(stock);
    let adopted = session.apply(Command::AdoptSimulation(AdoptSimulationArgs {
        result: Box::new(result_carrying(SimulationCutTrace::test_fixture())),
    }));
    let _ = adopted.expect("the adopt door stores a simulation result");
    session
}

/// Fixture G — a generated and simulated pocket the optimizer searches.
///
/// Arm (d) measures the residue the `BaselineRestoreGuard` leaves, and
/// only a run that REACHES the guard leaves any. That needs a real
/// material, a cached result and a trace with samples for the toolpath,
/// so this fixture pays for one generation and one simulation.
fn guard_session() -> ProjectSession {
    let mut session = base_session();
    generate(&mut session, 0);
    let cancel = AtomicBool::new(false);
    let _ = session
        .run_simulation(&sim_options(), &cancel)
        .expect("the fixture pocket simulates");
    session
}

/// The baseline trace the inline door takes by reference.
///
/// The `Arc` is cloned out of the session first: `optimize_toolpath`
/// borrows the session mutably, so a trace borrowed FROM the session
/// cannot live across the call.
fn trace_of(session: &ProjectSession) -> Arc<SimulationCutTrace> {
    session
        .simulation_result()
        .and_then(|result| result.cut_trace.clone())
        .expect("the fixture carries a baseline trace")
}

/// Start the job and unwrap its handle.
fn start_job(session: &mut ProjectSession, cancel: &AtomicBool) -> Box<OptimizeToolpathHandle> {
    let started = session.start(
        Job::OptimizeToolpath(OptimizeToolpathArgs { index: 0 }),
        cancel,
    );
    let JobHandle::OptimizeToolpath(handle) =
        started.expect("step (i) clones the session and captures the trace")
    else {
        panic!("the optimizer row answers its own handle variant");
    };
    handle
}

/// The three session slots the guard's drop leaves behind.
fn residue(session: &ProjectSession) -> (bool, u64, bool) {
    (
        session.get_result(0).is_some(),
        session.toolpath_revision(0),
        session.simulation_result().is_some(),
    )
}

// ── (a) the registry column ──────────────────────────────────────────

/// The optimizer is a `Job` row, and its wire name is the MCP tool name.
#[test]
fn the_optimizer_row_declares_the_job_kind() {
    assert_eq!(
        CommandId::OptimizeToolpath.kind(),
        CommandKind::Job,
        "the optimizer is a job: it captures a cloned session, runs off \
         the frame loop, and adopts nothing"
    );
    assert_eq!(
        CommandId::OptimizeToolpath.wire_name(),
        "optimize_toolpath",
        "the registry's wire name IS the MCP tool name, and the pinned \
         wire snapshot must not move"
    );
}

// ── (b) step (ii) holds no session ───────────────────────────────────

/// `execute_optimize_toolpath` coerces to a plain function pointer, and
/// runs after the session it was captured from is gone.
///
/// A `fn` pointer captures nothing, so a step (ii) that held a session
/// borrow does not coerce to the pointer type at all. `drop(session)` is
/// the second half of the claim: a handle that borrowed the caller's
/// session would not compile past that line.
#[test]
fn execute_optimize_toolpath_holds_no_caller_session() {
    let step_two: fn(&mut OptimizeToolpathHandle, &AtomicBool) -> OptimizeOutcome =
        execute_optimize_toolpath;

    let mut session = refusal_session();
    let cancel = AtomicBool::new(false);
    let mut handle = start_job(&mut session, &cancel);
    drop(session);

    let outcome = step_two(&mut handle, &cancel);
    assert_eq!(
        outcome.kind,
        OutcomeKind::Skipped,
        "Fixture R's Custom material refuses at step 3, so the search \
         skips; another kind means the arm measured a different fixture"
    );
    assert!(
        outcome.machine_snapshot.is_some(),
        "the outcome is stamped with the machine the run consumed, which \
         step (ii) reads off the handle's own clone"
    );
    assert!(
        outcome.assumptions.is_some(),
        "the outcome is stamped with the simulation assumptions, which \
         step (ii) reads off the handle's own clone and trace"
    );
}

// ── (c) the two doors are one computation ────────────────────────────

/// The `Job` door and the inline call answer the same outcome.
///
/// Two identical sessions, the same fixture. The oracle is
/// `optimize_toolpath`, the door that predates this package. Fixture R
/// refuses before any candidate is generated, so the candidate list is
/// empty by construction and this arm asserts nothing about it; the two
/// provenance stamps are the non-vacuity guard.
#[test]
fn the_job_door_answers_as_the_inline_call_does() {
    let cancel = AtomicBool::new(false);

    let mut oracle_session = refusal_session();
    let oracle_trace = trace_of(&oracle_session);
    let oracle = optimize_toolpath(&mut oracle_session, &oracle_trace, 0, &cancel);
    assert!(
        oracle.machine_snapshot.is_some() && oracle.assumptions.is_some(),
        "the oracle must carry both provenance stamps, or the comparison \
         below reads two Nones and asserts nothing"
    );

    let mut job_session = refusal_session();
    let mut handle = start_job(&mut job_session, &cancel);
    let job = execute_optimize_toolpath(&mut handle, &cancel);

    assert_eq!(
        job.kind, oracle.kind,
        "the Job door and the inline call must report one outcome kind"
    );
    assert_eq!(
        format!("{:?}", job.reason),
        format!("{:?}", oracle.reason),
        "the Job door and the inline call must report one refusal reason"
    );
    assert_eq!(
        format!("{:?}", job.machine_snapshot),
        format!("{:?}", oracle.machine_snapshot),
        "the machine stamp comes off the handle's clone, so it must read \
         as the live session's does"
    );
    assert_eq!(
        format!("{:?}", job.assumptions),
        format!("{:?}", oracle.assumptions),
        "the assumption stamp comes off the handle's clone and its trace, \
         so it must read as the live session's does"
    );
}

// ── (d) the live session keeps its result and its simulation ─────────

/// The inline call clears the live session; the job does not.
///
/// One test, three steps, so the measurement and the claim cannot drift
/// apart:
///
/// 1. both sessions carry a result, a revision and a simulation;
/// 2. the inline call DROPS the result and the simulation — this is the
///    evidence that makes step 3 non-vacuous;
/// 3. the job leaves all three where they were.
#[test]
fn the_job_leaves_the_live_session_alone() {
    let cancel = AtomicBool::new(false);

    let mut inline = guard_session();
    let mut job = guard_session();

    let before = residue(&inline);
    assert_eq!(
        before,
        residue(&job),
        "the two fixtures must start in one state, or step 2 and step 3 \
         measure different sessions"
    );
    assert!(
        before.0 && before.2,
        "the fixture must carry a cached result and a simulation before \
         anything runs; it carries {before:?}"
    );

    // Step 2 — the inline door, which is what WP14b replaces.
    let inline_trace = trace_of(&inline);
    let _ = optimize_toolpath(&mut inline, &inline_trace, 0, &cancel);
    let after_inline = residue(&inline);
    assert!(
        !after_inline.0 && !after_inline.2,
        "the inline call must clear the cached result and the simulation \
         — that residue is what this arm measures. It reads \
         {after_inline:?}. A fixture that never reaches \
         BaselineRestoreGuard leaves no residue, so this arm would then \
         measure NOTHING rather than pass: check that the pocket \
         generates, that the trace carries samples for it, and that the \
         pre-flight classifier does not refuse."
    );
    assert_ne!(
        after_inline.1, before.1,
        "drop_result bumps the toolpath's generation-input revision, so \
         the inline call moves it too"
    );

    // Step 3 — the job door, over its own clone.
    let configs_before = format!("{:?}", job.toolpath_configs());
    let mut handle = start_job(&mut job, &cancel);
    let _ = execute_optimize_toolpath(&mut handle, &cancel);
    drop(handle);
    assert_eq!(
        residue(&job),
        before,
        "the job runs on the handle's own clone, so the live session \
         keeps its cached result, its revision and its simulation"
    );
    assert_eq!(
        format!("{:?}", job.toolpath_configs()),
        configs_before,
        "the job writes no parameter on the live session either"
    );
}

// ── (e) what the session clone costs ─────────────────────────────────

/// Record the wall clock of one `ProjectSession::clone` on the P0
/// fixture.
///
/// **The bound is NOT a threshold.** §24 ruling 2 asks for the number and
/// names one second as the point at which the handle would take the
/// simulation by `Arc` and drop the display mesh instead of copying it.
/// Nothing in the product is tuned against this value, and no gate reads
/// it. The duration reaches the log through the assertion message, which
/// is the only reporting channel a test in this tree has:
/// `clippy::print_stdout` and `clippy::print_stderr` are denied in tests.
///
/// The measurement is VACUOUS on an empty session — there is nothing to
/// copy — so the arm asserts both heavy slots are populated first.
#[test]
fn the_session_clone_cost_is_recorded() {
    let session = guard_session();
    assert!(
        session.get_result(0).is_some(),
        "the clone measurement is vacuous without a cached result"
    );
    assert!(
        session.simulation_result().is_some(),
        "the clone measurement is vacuous without a simulation: the \
         display mesh and the deviation vectors are the copied weight"
    );

    let started = Instant::now();
    let copy = session.clone();
    let elapsed = started.elapsed();

    // The copy is read, not merely dropped: a clone that answered an
    // empty session would make the duration above meaningless.
    assert!(
        copy.get_result(0).is_some() && copy.simulation_result().is_some(),
        "the copy must carry both heavy slots, or the duration measured \
         a session with nothing in it"
    );
    drop(copy);

    // And the ORIGINAL is intact, which is the property the whole row
    // rests on.
    assert!(
        session.get_result(0).is_some() && session.simulation_result().is_some(),
        "a clone must leave the original populated"
    );

    assert!(
        elapsed < CLONE_REPORT_BOUND,
        "the session clone took {elapsed:?} on the P0 fixture. This \
         number is a RECORD, not a bar: §24 ruling 2 says that above one \
         second the handle takes the simulation by Arc and drops the \
         display mesh."
    );
}
