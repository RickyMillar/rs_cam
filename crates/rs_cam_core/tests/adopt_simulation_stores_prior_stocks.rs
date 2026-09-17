//! WP11b follow-up, N12 item 10 — an adopted simulation reaches
//! [`ProjectSession::start`].
//!
//! Programme: `planning/arch_consolidation_2026-09-09/IMPLEMENTATION_PLAN.md`
//! §22 addendum, "N12 item 10 and the `AdoptSimulation` ruling".
//!
//! # The defect this file pins
//!
//! `ProjectSession::start` reads the rest snapshot from
//! `session.simulation.prior_stocks`. The GUI simulates on its own lane
//! and adopted the answer into VIZ state alone, so `session.simulation`
//! stayed `None` in the GUI process and `start` refused every
//! `FromRemainingStock` operation there. Two simulation states.
//!
//! The ruling is one simulation state, adopted through a command. This
//! file measures the core half of it:
//!
//! - a session with no simulation refuses the rest operation, and the
//!   refusal names the missing snapshot;
//! - `apply(Command::AdoptSimulation(..))` stores the simulation, stales
//!   nothing and clears nothing;
//! - the stored snapshot is the SAME `Arc` the caller handed over — the
//!   arm stores the simulation, it does not rebuild one;
//! - `start` on the rest operation then succeeds.
//!
//! Red pre-fix: the registry declares no `AdoptSimulation` row, so this
//! file does not compile.
//!
//! # NOT MEASURED
//!
//! The GUI half. `crates/rs_cam_viz/src/controller/tests.rs::
//! a_gui_simulation_reaches_the_session_so_start_sees_the_prior_stock_n12_item10`
//! drives the GUI's own lane and the drain that adopts.
//!
//! # The fixture
//!
//! One 40 mm square polygon model, one Ø6 mm end mill, a 6 mm board
//! hanging below world Z0 (2D operations cut at negative Z), and two
//! pockets: index 0 cuts fresh stock, index 1 takes the remaining stock
//! of the one before it. The same recipe
//! `job_three_steps_equal_generate_toolpath.rs` builds.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::StockSource;
use rs_cam_core::compute::operation_configs::PocketConfig;
use rs_cam_core::compute::simulate::SimulationResult;
use rs_cam_core::dexel_stock::TriDexelStock;
use rs_cam_core::geo::{BoundingBox3, P3};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::session::{
    AddToolpathArgs, AdoptSimulationArgs, Command, CommandId, CommandKind, GenerateToolpathArgs,
    Job, ProjectSession, Reach,
};
use rs_cam_core::stock::stock_mesh::StockMesh;

mod common;
use common::session::{
    polygon_model, single_op_session, square_polygon, stock_under, toolpath_config,
};
use common::tools::endmill_tool_config;

/// Half-extent (mm) of the square the pockets clear.
const HALF: f64 = 20.0;

/// Stock height (mm). `stock_under` hangs the board below `z = 0`.
const STOCK_Z: f64 = 6.0;

/// Cutter diameter (mm).
const TOOL_D: f64 = 6.0;

/// Dexel cell (mm) of the snapshot the fixture hands over. Nothing reads
/// the grid here — the arm stores the map — so the value only has to be
/// a legal cell.
const CELL_MM: f64 = 2.0;

/// Extent (mm) of the snapshot's bounding box, on every axis.
const SNAPSHOT_EXTENT_MM: f64 = 10.0;

// ── Fixture ─────────────────────────────────────────────────────────

fn pocket_op() -> OperationConfig {
    OperationConfig::Pocket(PocketConfig {
        stepover: 2.0,
        depth: 3.0,
        depth_per_pass: 3.0,
        ..PocketConfig::default()
    })
}

/// One stock, one tool, one model, two ungenerated pockets. Index 1
/// takes the remaining stock of index 0.
fn rest_chain_session() -> ProjectSession {
    let mut session = single_op_session(
        stock_under(HALF, STOCK_Z),
        endmill_tool_config(TOOL_D),
        polygon_model(vec![square_polygon(HALF)], "square"),
        "Pocket",
        pocket_op(),
    );
    let tool_id = session.tools()[0].id.0;
    let model_id = session.models()[0].id;
    let mut rest = toolpath_config("Rest", pocket_op(), tool_id, model_id);
    rest.stock_source = StockSource::FromRemainingStock;
    let _ = session
        .apply(Command::AddToolpath(AddToolpathArgs {
            setup_index: 0,
            config: Box::new(rest),
        }))
        .expect("the fresh session holds setup 0");
    session
}

/// A simulation that carries one prior-stock snapshot, keyed by
/// `for_toolpath`, and nothing else.
///
/// Every display field is empty. `start` reads `prior_stocks` alone, and
/// an empty mesh here is the fixture's own answer, not a measurement.
fn simulation_with_prior_stock(
    for_toolpath: ToolpathId,
    snapshot: &Arc<TriDexelStock>,
) -> SimulationResult {
    let mut prior_stocks = HashMap::new();
    prior_stocks.insert(for_toolpath, Arc::clone(snapshot));
    SimulationResult {
        mesh: StockMesh {
            vertices: Vec::new(),
            indices: Vec::new(),
            colors: Vec::new(),
        },
        total_moves: 0,
        deviations: None,
        column_deviations: None,
        boundaries: Vec::new(),
        checkpoints: Vec::new(),
        rapid_collisions: Vec::new(),
        rapid_collision_move_indices: Vec::new(),
        cut_trace: None,
        resolution_clamped: false,
        column_grid_cell_mm: CELL_MM,
        prior_stocks,
    }
}

/// The snapshot the fixture hands over.
fn snapshot() -> Arc<TriDexelStock> {
    let bbox = BoundingBox3 {
        min: P3::new(0.0, 0.0, 0.0),
        max: P3::new(SNAPSHOT_EXTENT_MM, SNAPSHOT_EXTENT_MM, SNAPSHOT_EXTENT_MM),
    };
    Arc::new(TriDexelStock::from_bounds(&bbox, CELL_MM))
}

/// Start the rest operation and report the refusal text, if any.
fn start_refusal(session: &mut ProjectSession) -> Option<String> {
    let cancel = AtomicBool::new(false);
    session
        .start(
            Job::GenerateToolpath(GenerateToolpathArgs { index: 1 }),
            &cancel,
        )
        .err()
        .map(|error| error.to_string())
}

// ── The adopt reaches `start` ───────────────────────────────────────

/// An adopted simulation lets `start` see the prior stock.
#[test]
fn an_adopted_simulation_lets_start_see_the_prior_stock() {
    let mut session = rest_chain_session();
    let rest_id = session.toolpath_configs()[1].id;

    // Control. Without a simulation the rest operation is refused, and
    // the refusal names the snapshot it wants. Without this arm the test
    // cannot tell "the adopt worked" from "the gate was never shut".
    let refusal = start_refusal(&mut session);
    let refusal = refusal.expect("a rest operation with no simulation is refused");
    assert!(
        refusal.contains("remaining stock"),
        "the control must fail on the REST gate, not on something else; \
         got: {refusal}"
    );

    let snapshot = snapshot();
    let effects = session
        .apply(Command::AdoptSimulation(AdoptSimulationArgs {
            result: Box::new(simulation_with_prior_stock(rest_id, &snapshot)),
            epoch: session.simulation_epoch(),
        }))
        .expect("the adopt stores the simulation");
    assert!(
        effects.stale.is_empty(),
        "an adopted simulation moves no generation-input revision, so it \
         stales nothing; got {:?}",
        effects.stale
    );
    assert!(
        !effects.simulation_cleared,
        "the adopt REPLACES the simulation, so nothing was cleared"
    );

    let stored = session
        .simulation_result()
        .and_then(|simulation| simulation.prior_stocks.get(&rest_id))
        .map(Arc::clone)
        .expect("the session holds the adopted snapshot");
    assert!(
        Arc::ptr_eq(&stored, &snapshot),
        "the arm must STORE the snapshot it was handed, not rebuild one"
    );

    let refusal = start_refusal(&mut session);
    assert!(
        refusal.is_none(),
        "start must see the adopted prior stock; got: {refusal:?}"
    );
}

// ── The registry column ─────────────────────────────────────────────

/// The `adopt_simulation` row is a `Command` the GUI reaches alone.
#[test]
fn the_adopt_simulation_row_is_a_gui_only_command() {
    assert_eq!(
        CommandId::AdoptSimulation.kind(),
        CommandKind::Command,
        "the adopt is one synchronous mutation, not a job"
    );
    assert_eq!(CommandId::AdoptSimulation.wire_name(), "adopt_simulation");
    let surfaces = CommandId::AdoptSimulation.surfaces();
    assert!(
        matches!(surfaces.gui, Reach::Reached),
        "the GUI drain adopts its own simulation through this row"
    );
    assert!(
        matches!(surfaces.mcp, Reach::Skip(_)),
        "the MCP simulate tool runs the GUI lane, which adopts"
    );
    assert!(
        matches!(surfaces.cli, Reach::Skip(_)),
        "the CLI simulates through run_simulation, which stores the \
         result itself"
    );
}
