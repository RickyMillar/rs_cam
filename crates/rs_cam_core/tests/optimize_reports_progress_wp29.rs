//! WP29 — the Optimize run reports its rung and its candidate count.
//!
//! Programme:
//! `planning/arch_consolidation_2026-09-09/IMPLEMENTATION_PLAN.md` §33
//! (operator ruling, 2026-09-13). The operator watched a Back Rough
//! Optimize run for over four minutes and read a spinner and a second
//! count: "I get no real progress anymore.. like. its just a spinner."
//!
//! # The finding this file pins
//!
//! The candidate search published nothing about itself.
//! `optimize_toolpath` took no sink, and the one progress channel that
//! does exist (`ProgressReporter`, the project rollup's) reaches no GUI
//! surface. So the workspace-bar row and the Optimize window could carry
//! a label and the elapsed seconds and nothing else.
//!
//! WP29 gives the search a progress struct. It rides
//! `EvaluationContext`, so no candidate-helper signature moves, and it
//! rides the `OptimizeToolpathHandle`, so
//! `execute_optimize_toolpath`'s signature does not move either — that
//! signature is pinned by
//! `crates/rs_cam_core/tests/optimize_toolpath_is_a_job_wp14b.rs:299`,
//! and this file's green depends on that arm staying green.
//!
//! # Red-first
//!
//! This file is COMPILE-red at the parent revision, and it is
//! compile-red BECAUSE THE TYPE HAS NO DECLARATION, not because the
//! type has no writer. `OptimizeProgress`, `SearchPhase` and
//! `OptimizeToolpathHandle::with_progress` do not exist there, so the
//! integration binary does not build against the pre-fix library. Red is
//! one error per missing name.
//!
//! # One test, one search
//!
//! The fixture pays for one generation, one simulation and one whole
//! candidate search. Three `#[test]` arms would pay for it three times,
//! which is how a targeted suite stops being targeted. So one arm makes
//! four assertions on one run.
//!
//! # The fixture
//!
//! Fixture G of the WP14b suite: a 40 mm square pocket in a
//! 44 x 44 x 6 mm board, one depth pass, one Ø6 mm end mill, generated
//! and simulated once at a 1 mm cell, links off. It REACHES `BaselineRestoreGuard`.
//! `optimize_toolpath_is_a_job_wp14b.rs`'s arm
//! `the_job_leaves_the_live_session_alone` proves that on this same
//! fixture, by measuring the residue the guard's drop leaves. That arm is
//! the non-vacuity proof for this one, and the verifier runs both.
//!
//! # Phase 2 (2026-10-12)
//!
//! `planning/optimize_review_2026-10-12/PLAN.md` §2 gives the search two
//! rungs: `Links` and `DepthWidth`. The fixture turns the links off, so
//! the link rung forms one candidate. The arm pins: every rung is named
//! (the high-water mark reaches the last rung), each rung entered every
//! candidate it formed, and a settled run claims no rung. The fixture
//! simulates with the project cell of 1 mm.
//!
//! # NOT MEASURED
//!
//! - **The silent default.** `OptimizeToolpathHandle::progress` is
//!   private with no accessor, so no test reads it. WP14b arms (c) and
//!   (d) run the handle `capture_optimize_toolpath` built; while they
//!   stay green the default is silent.
//! - **Anything on screen.** The row and the window are viz surfaces.
//!   Their halves are
//!   `crates/rs_cam_viz/src/controller/tests.rs` (the text builder) and
//!   `crates/rs_cam_viz/tests/optimize_run_is_non_modal_wp24.rs` (the two
//!   source scans).
//! - **Wall clock.** This file asserts no duration. WP29 adds no
//!   time-left figure, for the two reasons the progress module's doc
//!   gives.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use common::make_endmill_6mm;
use common::session::{
    generate, polygon_model, single_op_session_with, square_polygon, stock_under,
};
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::PocketConfig;
use rs_cam_core::session::{
    Command, Job, JobHandle, OptimizeToolpathArgs, OptimizeToolpathHandle, ProjectSession,
    SetSimulationResolutionArgs, SimulationResolution, execute_optimize_toolpath,
};
use rs_cam_core::tool_load::optimize::{OptimizeProgress, SearchPhase, candidate_sim_options};

// ── constants ────────────────────────────────────────────────────────

/// Half-extent of the square model, in mm.
const HALF_MM: f64 = 20.0;

/// Board thickness, in mm. The board hangs below `z = 0`, the frame a 2D
/// operation cuts in.
const STOCK_Z_MM: f64 = 6.0;

/// The project's stored dexel cell, in mm. Coarse on purpose: the arm
/// measures what the search announced, not a cut.
const SIM_CELL_MM: f64 = 1.0;

// ── fixture ──────────────────────────────────────────────────────────

/// The pocket the fixture machines.
fn pocket_op() -> OperationConfig {
    OperationConfig::Pocket(PocketConfig {
        stepover: 2.0,
        depth: 3.0,
        depth_per_pass: 3.0,
        ..PocketConfig::default()
    })
}

/// Fixture G — a generated and simulated pocket the optimizer searches.
///
/// The links are off, so the link rung forms one candidate. The depth per
/// pass is the whole depth, so the depth and width rung forms one cell for
/// each stepover.
fn guard_session() -> ProjectSession {
    let mut session = single_op_session_with(
        stock_under(HALF_MM, STOCK_Z_MM),
        make_endmill_6mm(),
        polygon_model(vec![square_polygon(HALF_MM)], "square"),
        "Pocket",
        pocket_op(),
        |tc| tc.dressups.link_moves = None,
    );
    let _ = session
        .apply(Command::SetSimulationResolution(
            SetSimulationResolutionArgs {
                resolution: SimulationResolution::Fixed(SIM_CELL_MM),
            },
        ))
        .expect("the resolution row refuses nothing");
    generate(&mut session, 0);
    let cancel = AtomicBool::new(false);
    let opts = candidate_sim_options(&session);
    let _ = session
        .run_simulation(&opts, &cancel)
        .expect("the fixture pocket simulates");
    session
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

// ── the arm ──────────────────────────────────────────────────────────

/// The search announces every rung, forms its candidates per rung, and
/// enters every candidate it formed.
///
/// **Why a high-water mark.** This arm runs the search to completion on
/// one thread, so it cannot watch a rung go by. The mark is what makes
/// the measurement possible at all, and that is why `OptimizeProgress`
/// keeps one — it is not a convenience.
#[test]
fn the_search_publishes_its_rungs_and_its_candidate_counts() {
    let cancel = AtomicBool::new(false);
    let mut session = guard_session();
    let progress = Arc::new(OptimizeProgress::default());

    let mut handle = start_job(&mut session, &cancel);
    handle.with_progress(Arc::clone(&progress));
    let outcome = execute_optimize_toolpath(&mut handle, &cancel);
    drop(handle);

    // 0 — the run reached the levers, so the rung announcements below
    // are not from an early skip.
    assert!(
        outcome.candidates.len() >= 2,
        "the fixture must pass every skip path and simulate a candidate; \
         it returned {outcome:?}"
    );

    // 1 — the run named every rung, up to the last.
    assert_eq!(
        progress.high_water_phase(),
        Some(SearchPhase::DepthWidth),
        "the search announces each rung of the two-rung ladder, so the \
         high-water mark must name the depth and width rung"
    );

    // 2 — the link rung formed one candidate and entered it; the depth
    // and width rung entered every cell it formed.
    assert_eq!(
        progress.rung(SearchPhase::Links),
        (1, 1),
        "the links are off, so the link rung forms and enters one candidate"
    );
    let (entered, formed) = progress.rung(SearchPhase::DepthWidth);
    assert!(
        formed >= 1 && entered == formed,
        "the depth and width rung must enter every cell it formed; it \
         reports ({entered}, {formed})"
    );

    // 3 — a settled run claims no rung.
    let snapshot = progress.snapshot();
    assert!(
        snapshot.phase.is_none(),
        "a settled run must not leave the row claiming a rung. finish() \
         clears the rung and leaves the per-rung counters, which is why \
         assertions 1 and 2 still read after it; the snapshot reports \
         {snapshot:?}"
    );
}
