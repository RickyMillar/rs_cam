//! B19 — a `Fresh` operation reads only the board.
//!
//! # The defect this file pins
//!
//! The generate step captured the simulated `prior_stocks` snapshot of
//! every operation whose snapshot was current, whatever its stock source.
//! The snapshot reached the dressup air-cut filter, and the filter runs
//! whenever it has a snapshot. So a `Fresh` operation generated AFTER a
//! simulation dropped the moves over stock that an earlier operation had
//! removed, and the same operation generated BEFORE a simulation kept
//! them. The output of a `Fresh` operation was a function of the session
//! history, not of the project file.
//!
//! Operator-level ruling (2026-10-15): a `Fresh` operation reads only the
//! board. Its generation is a pure function of the project file.
//!
//! # The fixture
//!
//! One Ø6 mm end mill, a 10 mm board below world Z0, and two `Fresh`
//! pockets on the same 30 mm square to the same depth. After a simulation,
//! the snapshot of B holds the stock that A left, so every move of B is in
//! air. That is the most air a path can cross, so the filter has the most
//! moves to drop.
//!
//! # The claims
//!
//! 1. Non-vacuity: after the simulation the session holds a CURRENT
//!    snapshot under the id of B. Without it the defect cannot occur.
//! 2. B generated after the simulation emits the same moves as B generated
//!    before it: the same move count and the same move fingerprint.
//! 3. B's result records no stock snapshot (`ToolpathStats::stock_snapshot`
//!    is `None`). The stamp is the record of the snapshot the generation
//!    context held, so `None` means the context held no prior stock.
//!
//! Red proof: on HEAD 2ea3166b (before the fix) claim 2 fails (259 moves
//! before the simulation, 119 after). The unit test
//! `session::compute::tests::a_fresh_op_captures_no_prior_stock_b19` pins
//! the capture itself.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::sync::atomic::AtomicBool;

use common::fingerprint::move_fingerprint;
use common::session::{generate, polygon_model, square_polygon, toolpath_config};
use common::tools::endmill_tool_config;
use rs_cam_core::compute::StockConfig;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::StockSource;
use rs_cam_core::compute::operation_configs::PocketConfig;
use rs_cam_core::session::{
    Command, ProjectSession, ProjectSessionBuilder, SetSimulationResolutionArgs, SimulationOptions,
    SimulationResolution,
};

/// Half-extent (mm) of the square both pockets clear.
const HALF: f64 = 15.0;

/// Stock margin (mm) around the square.
const MARGIN: f64 = 2.0;

/// Stock height (mm). The board hangs below `z = 0`.
const STOCK_Z: f64 = 10.0;

/// Cutter diameter (mm).
const TOOL_D: f64 = 6.0;

/// Simulation cell (mm).
const CELL_MM: f64 = 0.5;

/// Depth (mm) of both pockets.
const DEPTH: f64 = 4.0;

fn stock() -> StockConfig {
    StockConfig {
        x: 2.0 * (HALF + MARGIN),
        y: 2.0 * (HALF + MARGIN),
        z: STOCK_Z,
        origin_x: -HALF - MARGIN,
        origin_y: -HALF - MARGIN,
        origin_z: -STOCK_Z,
        auto_from_model: false,
        ..StockConfig::default()
    }
}

fn pocket() -> OperationConfig {
    OperationConfig::Pocket(PocketConfig {
        stepover: 2.0,
        floor: rs_cam_core::compute::floor::Floor::top(DEPTH),
        depth_per_pass: 1.0,
        ..PocketConfig::default()
    })
}

/// Two `Fresh` pockets, A then B, on the same square in one setup.
fn two_fresh_pockets() -> ProjectSession {
    let mut builder = ProjectSessionBuilder::new().stock(stock());
    let tool_idx = builder.add_tool(endmill_tool_config(TOOL_D));
    let tool_id = builder.tools()[tool_idx].id.0;
    let model = builder.add_model(polygon_model(vec![square_polygon(HALF)], "square"));
    for name in ["A", "B"] {
        let mut cfg = toolpath_config(name, pocket(), tool_id, model);
        cfg.stock_source = StockSource::Fresh;
        let _ = builder.add_toolpath(0, cfg).expect("add a pocket");
    }
    builder.build()
}

fn simulate(session: &mut ProjectSession) {
    let _ = session
        .apply(Command::SetSimulationResolution(
            SetSimulationResolutionArgs {
                resolution: SimulationResolution::Fixed(CELL_MM),
            },
        ))
        .expect("a positive cell");
    let cancel = AtomicBool::new(false);
    session
        .run_simulation(
            &SimulationOptions {
                resolution: rs_cam_core::session::SimulationResolution::Fixed(CELL_MM),
                adaptive_feed_modulation: false,
                ..SimulationOptions::default()
            },
            &cancel,
        )
        .expect("the simulation runs");
}

/// The move count and the move fingerprint of the result at `index`.
fn capture(session: &ProjectSession, index: usize) -> (usize, u64) {
    let result = session.get_result(index).expect("the op holds a result");
    move_fingerprint(result.toolpath())
}

#[test]
fn a_fresh_op_generates_the_same_moves_before_and_after_a_simulation_b19() {
    let mut session = two_fresh_pockets();
    generate(&mut session, 0);
    generate(&mut session, 1);
    let before = capture(&session, 1);
    assert!(
        before.0 > 2,
        "B must emit real motion; got {} moves",
        before.0
    );

    simulate(&mut session);

    // Claim 1: the snapshot under B's id exists and is current, so the
    // pre-fix capture reads it.
    let b_id = session.toolpath_configs()[1].id;
    assert!(
        session
            .simulation_result()
            .is_some_and(|sim| sim.prior_stocks.contains_key(&b_id)),
        "fixture: the simulation must record a prior-stock snapshot for B"
    );
    assert!(
        session.snapshot_is_current(b_id).is_ok(),
        "fixture: B's snapshot must be current after the simulation"
    );

    generate(&mut session, 1);
    let after = capture(&session, 1);

    // Claim 2.
    assert_eq!(
        after, before,
        "B19: a Fresh op generated after a simulation emitted different moves \
         (count, fingerprint) than before it: {after:?} against {before:?}. \
         A Fresh op must read only the board."
    );

    // Claim 3.
    let stamp = session
        .get_result(1)
        .expect("B holds a result")
        .stats
        .stock_snapshot;
    assert_eq!(
        stamp, None,
        "B19: a Fresh op's generation recorded a stock snapshot, so its \
         generation context held the simulated prior stock"
    );
}
