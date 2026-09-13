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
//! five assertions on one run.
//!
//! # The fixture
//!
//! Fixture G of the WP14b suite: a 40 mm square pocket in a
//! 44 x 44 x 6 mm board, one depth pass, one Ø6 mm end mill, generated
//! and simulated once at a 1 mm cell. It is the one fixture in this tree
//! that REACHES `BaselineRestoreGuard` and therefore evaluates real
//! candidates. `optimize_toolpath_is_a_job_wp14b.rs`'s arm
//! `the_job_leaves_the_live_session_alone` proves that on this same
//! fixture, by measuring the residue the guard's drop leaves. That arm is
//! the non-vacuity proof for this one, and the verifier runs both.
//!
//! # NOT MEASURED
//!
//! - **Stage F.** `ChiploadVerdict` has three variants, and an
//!   `Unmodeled` chipload with no exceeding load gate runs NEITHER
//!   Stage-F mode. The fixture's verdict is not known to the lane that
//!   wrote this file, so no assertion names the Stage-F rung. The
//!   orchestrator still announces rung 1 with a zero total, so the
//!   window's first row ticks on that path.
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
use rs_cam_core::feed_modulation::ModulationStrategy;
use rs_cam_core::session::{
    Job, JobHandle, OptimizeToolpathArgs, OptimizeToolpathHandle, ProjectSession,
    SimulationOptions, execute_optimize_toolpath,
};
use rs_cam_core::tool_load::optimize::{OptimizeProgress, SearchPhase};

// ── constants ────────────────────────────────────────────────────────

/// Half-extent of the square model, in mm.
const HALF_MM: f64 = 20.0;

/// Board thickness, in mm. The board hangs below `z = 0`, the frame a 2D
/// operation cuts in.
const STOCK_Z_MM: f64 = 6.0;

/// The dexel cell the fixture simulates at, in mm. Coarse on purpose:
/// the arm measures what the search announced, not a cut.
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

/// The baseline simulation's options, at the coarse cell.
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

/// Fixture G — a generated and simulated pocket the optimizer searches.
fn guard_session() -> ProjectSession {
    let mut session = single_op_session_with(
        stock_under(HALF_MM, STOCK_Z_MM),
        make_endmill_6mm(),
        polygon_model(vec![square_polygon(HALF_MM)], "square"),
        "Pocket",
        pocket_op(),
        |_| {},
    );
    generate(&mut session, 0);
    let cancel = AtomicBool::new(false);
    let _ = session
        .run_simulation(&sim_options(), &cancel)
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

/// The search announces every rung it walks, and enters every candidate
/// it announced.
///
/// **Why a high-water mark.** This arm runs the search to completion on
/// one thread, so it cannot watch a rung go by. The mark is what makes
/// the measurement possible at all, and that is why `OptimizeProgress`
/// keeps one — it is not a convenience.
///
/// **Why `reached == total` is the load-bearing half.** The tick sits at
/// the TOP of each candidate loop, not inside `evaluate_candidate`. Both
/// the Stage-F and the Stage-1 loops `continue` when
/// `apply_patches_to_op` fails and never reach the evaluator, so a tick
/// inside it would leave `reached` short of `total` for the rest of the
/// rung and the row would freeze. There is no post-loop call that writes
/// `reached = total`: such a call would report a cancelled rung as
/// complete, and it would hide exactly this defect.
#[test]
fn the_search_publishes_its_rungs_and_its_candidate_counts() {
    let cancel = AtomicBool::new(false);
    let mut session = guard_session();
    let progress = Arc::new(OptimizeProgress::default());

    let mut handle = start_job(&mut session, &cancel);
    handle.with_progress(Arc::clone(&progress));
    let _ = execute_optimize_toolpath(&mut handle, &cancel);
    drop(handle);

    // 1 — the run reached the last rung.
    assert_eq!(
        progress.high_water_phase(),
        Some(SearchPhase::Refine),
        "the search walks a fixed three-rung ladder and this fixture \
         reaches the end of it, so the high-water mark must name the \
         refine rung"
    );

    // 2 — the grid formed candidates, so assertion 3 is not vacuous.
    let grid = progress.rung(SearchPhase::AxisGrid);
    assert!(
        grid.1 >= 1,
        "the axis grid must form at least one candidate, or assertion 3 \
         compares two zeroes and measures nothing; it reports {grid:?}"
    );

    // 3 — the grid entered every candidate it announced.
    assert_eq!(
        grid.0, grid.1,
        "the grid loop must enter every candidate it announced. A tick \
         placed inside evaluate_candidate instead of at the top of the \
         loop leaves this pair unequal, because the loop's \
         apply_patches_to_op arm skips a candidate the evaluator never \
         sees. It reports {grid:?}"
    );

    // 4 — the refine rung did the same.
    let refine = progress.rung(SearchPhase::Refine);
    assert!(
        refine.1 >= 1,
        "the refine rung must form at least one candidate. It takes the \
         Stage-1 survivors, so a zero here means no grid candidate \
         survived its gate on this fixture — check that before blaming \
         the progress struct. It reports {refine:?}"
    );
    assert_eq!(
        refine.0, refine.1,
        "the refine loop must enter every candidate it announced, for \
         the reason assertion 3 gives; it reports {refine:?}"
    );

    // 5 — a settled run claims no rung.
    let snapshot = progress.snapshot();
    assert!(
        snapshot.phase.is_none(),
        "a settled run must not leave the row claiming a rung. finish() \
         clears the rung and leaves the per-rung counters, which is why \
         assertions 1 to 4 still read after it; the snapshot reports \
         {snapshot:?}"
    );
}
