//! **G-OPT6 — the baseline and every candidate are one measure.**
//!
//! Pre-registered in `planning/optimize_review_2026-10-12/PLAN.md` §2
//! step 4 (review finding F7). The 2026-05 search simulated its
//! candidates at 1.0 and 0.5 mm with the feed modulator off, while the
//! baseline on screen ran at the project resolution with the modulator
//! on: two operating points in one ranking.
//!
//! The rebuild simulates every candidate with the options the GUI's
//! simulation submit builds (`tool_load::optimize::candidate_sim_options`).
//! The proof is a candidate at the BASELINE parameters:
//!
//! 1. Its cycle time is within 1 % of the on-screen trace's
//!    `total_runtime_s` for that toolpath.
//! 2. Its full-cut check holds against the baseline.
//! 3. The outcome stamp's operating point equals the options of the
//!    on-screen simulation, which this file writes out as the GUI does
//!    (`rs_cam_viz/src/controller/events/simulation.rs` and
//!    `controller/events/compute.rs`), not through the optimizer's own
//!    builder.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::sync::atomic::AtomicBool;

use common::make_endmill_6mm;
use common::session::{
    generate, polygon_model, single_op_session_with, square_polygon, stock_under,
};
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::PocketConfig;
use rs_cam_core::dressup::feed_modulation::ModulationStrategy;
use rs_cam_core::session::{
    Command, ProjectSession, SetSimulationResolutionArgs, SimulationOptions, SimulationResolution,
};
use rs_cam_core::tool_load::optimize::{
    OperatingPointStamp, SimAssumptionStamp, measure_candidate,
};

/// Half-extent of the square model, in mm.
const HALF_MM: f64 = 20.0;
/// Board thickness, in mm.
const STOCK_Z_MM: f64 = 6.0;
/// The project's stored cell, in mm. Coarse, so the fixture is fast.
const CELL_MM: f64 = 0.5;
/// The tolerance of the cycle-time claim: 1 % (PLAN.md §2, package R1).
const CYCLE_TOLERANCE: f64 = 0.01;

fn pocket_op() -> OperationConfig {
    OperationConfig::Pocket(PocketConfig {
        stepover: 2.0,
        depth: 3.0,
        depth_per_pass: 1.5,
        ..PocketConfig::default()
    })
}

/// The options the GUI simulation runs with, written out as the GUI
/// writes them.
fn gui_options(session: &ProjectSession) -> SimulationOptions {
    SimulationOptions {
        resolution: session.simulation_resolution_mm(),
        skip_ids: Vec::new(),
        auto_resolution: false,
        use_predicted_feed_in_gates: false,
        adaptive_feed_modulation: true,
        modulation_strategy: ModulationStrategy::ConstrainedMax,
        modulation_feed_scale: 1.0,
    }
}

#[test]
fn a_candidate_at_the_baseline_parameters_measures_the_baseline() {
    let mut session = single_op_session_with(
        stock_under(HALF_MM, STOCK_Z_MM),
        make_endmill_6mm(),
        polygon_model(vec![square_polygon(HALF_MM)], "square"),
        "Pocket",
        pocket_op(),
        |_| {},
    );
    let _ = session
        .apply(Command::SetSimulationResolution(
            SetSimulationResolutionArgs {
                resolution: SimulationResolution::Fixed(CELL_MM),
            },
        ))
        .expect("the resolution row refuses nothing");
    generate(&mut session, 0);
    let cancel = AtomicBool::new(false);
    let opts = gui_options(&session);
    let trace = session
        .run_simulation(&opts, &cancel)
        .expect("the fixture pocket simulates")
        .cut_trace
        .clone()
        .expect("the simulation keeps its cut trace");
    let id = session.toolpath_configs()[0].id;
    let on_screen_s = trace
        .toolpath_summaries
        .iter()
        .find(|s| s.toolpath_id == id)
        .expect("the trace holds the toolpath")
        .total_runtime_s;
    assert!(on_screen_s > 0.0);

    // 3 — the stamp names the operating point of the on-screen run.
    let stamp = SimAssumptionStamp::of(&session, 0);
    assert_eq!(
        stamp.operating_point,
        OperatingPointStamp::from_options(&opts),
        "the optimizer must simulate at the operating point the GUI uses"
    );

    // 1 and 2 — a candidate at the baseline parameters.
    let tc = session.toolpath_configs()[0].clone();
    let (baseline, candidate) = measure_candidate(
        &mut session,
        &trace,
        0,
        tc.operation.clone(),
        tc.dressups.clone(),
        &cancel,
    )
    .expect("the baseline parameters simulate");
    assert!((baseline.cycle_time_s - on_screen_s).abs() < 1e-12);
    let ratio = (candidate.cycle_time_s - on_screen_s).abs() / on_screen_s;
    assert!(
        ratio <= CYCLE_TOLERANCE,
        "a candidate at the baseline parameters takes {:.3} s against {:.3} s on screen \
         ({:.2} % apart; the claim is 1 %)",
        candidate.cycle_time_s,
        on_screen_s,
        ratio * 100.0
    );
    assert!(
        candidate.full_cut.verdict.holds(),
        "the same parameters cut the same part: {:?}",
        candidate.full_cut
    );
    assert!(!candidate.delta.has_changes(), "{:?}", candidate.delta);

    // The guard restored the parameters.
    assert_eq!(
        format!("{:?}", session.toolpath_configs()[0].operation),
        format!("{:?}", tc.operation)
    );
}
