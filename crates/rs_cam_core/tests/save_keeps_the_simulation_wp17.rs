//! WP17 (tech-debt review H1) — a save keeps the simulation, so a
//! `FromRemainingStock` operation still starts after it.
//!
//! Programme: `planning/arch_consolidation_2026-09-09/IMPLEMENTATION_PLAN.md`
//! §22, WP17.
//!
//! # The defect this file pins
//!
//! `ProjectSession::start` reads the rest snapshot from
//! `session.simulation` (WP11b). It REFUSES a `FromRemainingStock`
//! operation when that field is absent.
//!
//! `ProjectSession::set_post_config` wrote `session.simulation = None`
//! on every call. Both GUI save doors — `viz/controller/io.rs` and
//! `viz/app/mcp/commands.rs` — called that setter before every save, and
//! they called it with the block the session already held. A save
//! therefore dropped the simulation, and every rest operation was
//! refused until the operator simulated again. Nothing re-adopted.
//!
//! The rule this file measures: a post-processor edit does not drop the
//! simulation unless the field that moved reaches emitted motion. The
//! post flavour and the spindle strategy do not. The export door reads
//! both at emit time.
//!
//! # What each arm measures
//!
//! - a session with a simulation starts the rest operation (the
//!   control — without it the file cannot tell a repair from a gate
//!   that was never shut);
//! - `SetPostConfig` with the block the session already holds keeps the
//!   simulation, reports `simulation_cleared == false`, and the rest
//!   operation still starts;
//! - `SetPostConfig` with a changed spindle strategy does the same, and
//!   the session holds the new strategy;
//! - `SaveProject` writes the file and keeps the simulation.
//!
//! Red pre-fix: the post-config arm clears the simulation, so the second
//! arm fails at `simulation_cleared` and `start` reports the rest
//! refusal.
//!
//! # NOT MEASURED
//!
//! The two GUI save doors. `crates/rs_cam_viz/src/controller/tests.rs`
//! drives those:
//! `a_save_keeps_the_simulation_so_a_rest_op_still_starts_wp17` and
//! `the_mcp_save_conversion_writes_no_session_state_wp17`.
//!
//! The fields this file does not exercise — `safe_z`, `spindle_speed`,
//! `high_feedrate_mode` and `high_feedrate`. The arm keeps clearing the
//! simulation for those four, and no arm here measures that.
//!
//! # The fixture
//!
//! The fixture of `adopt_simulation_stores_prior_stocks.rs`: one 40 mm
//! square polygon model, one Ø6 mm end mill, a 6 mm board hanging below
//! world Z0 (2D operations cut at negative Z), and two pockets. Index 0
//! cuts fresh stock. Index 1 takes the remaining stock of index 0.

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
use rs_cam_core::feeds::SpindleStrategy;
use rs_cam_core::geo::{BoundingBox3, P3};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::session::{
    AddToolpathArgs, AdoptSimulationArgs, Command, GenerateToolpathArgs, Job, ProjectSession,
    SaveProjectArgs, SetPostConfigArgs,
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
/// the grid here, so the value only has to be a legal cell.
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
fn simulation_with_prior_stock(for_toolpath: ToolpathId) -> SimulationResult {
    let bbox = BoundingBox3 {
        min: P3::new(0.0, 0.0, 0.0),
        max: P3::new(SNAPSHOT_EXTENT_MM, SNAPSHOT_EXTENT_MM, SNAPSHOT_EXTENT_MM),
    };
    let mut prior_stocks = HashMap::new();
    prior_stocks.insert(
        for_toolpath,
        Arc::new(TriDexelStock::from_bounds(&bbox, CELL_MM)),
    );
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
        prior_stock_sources: std::collections::HashMap::new(),
    }
}

/// A session that holds the rest chain AND an adopted simulation.
fn session_with_simulation() -> ProjectSession {
    let mut session = rest_chain_session();
    let rest_id = session.toolpath_configs()[1].id;
    let mut simulation = simulation_with_prior_stock(rest_id);
    make_snapshot_current(&mut session, &mut simulation, rest_id);
    let _ = session
        .apply(Command::AdoptSimulation(AdoptSimulationArgs {
            result: Box::new(simulation),
            epoch: session.simulation_epoch(),
        }))
        .expect("the adopt stores the simulation");
    session
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

/// Write the post block through the command row.
fn set_post(
    session: &mut ProjectSession,
    post: rs_cam_core::gcode::PostConfig,
) -> rs_cam_core::session::Effects {
    session
        .apply(Command::SetPostConfig(SetPostConfigArgs {
            post: Box::new(post),
        }))
        .expect("the post-config row refuses nothing")
}

/// A project path of this test's own, under the system temp directory.
fn temp_project_path(name: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("the clock is after the epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("rs_cam_wp17_{name}_{nanos}.toml"))
}

// ── A post-config edit keeps the simulation ─────────────────────────

/// A post-config write keeps the simulation, and the rest operation
/// still starts.
#[test]
fn a_post_config_edit_keeps_the_simulation_wp17() {
    let mut session = session_with_simulation();

    // Control. The gate is open before the writes below, so a refusal
    // after one of them is the write's own doing.
    assert!(
        start_refusal(&mut session).is_none(),
        "the control must start; the adopted simulation carries the \
         prior stock the rest operation wants"
    );

    // (a) The block the session already holds. This is what both save
    // doors sent, and it dropped the simulation.
    let same = session.post_config().clone();
    let effects = set_post(&mut session, same);
    assert!(
        !effects.simulation_cleared,
        "a post-config write that changes nothing clears nothing"
    );
    assert!(
        session.simulation_result().is_some(),
        "the session keeps the simulation across an unchanged post write"
    );
    assert!(
        effects.stale.is_empty(),
        "a post-config write moves no generation-input revision; got {:?}",
        effects.stale
    );
    let refusal = start_refusal(&mut session);
    assert!(
        refusal.is_none(),
        "the rest operation still starts after an unchanged post write; \
         got: {refusal:?}"
    );

    // (b) A changed spindle strategy. The strategy reaches the feeds
    // suggest path and the G-code, and it moves no geometry and no
    // simulated stock.
    let mut changed = session.post_config().clone();
    changed.spindle_strategy = match changed.spindle_strategy {
        SpindleStrategy::MatchChart => SpindleStrategy::MaxSpeed,
        SpindleStrategy::MaxSpeed => SpindleStrategy::MatchChart,
    };
    let wanted = changed.spindle_strategy;
    let effects = set_post(&mut session, changed);
    assert_eq!(
        session.post_config().spindle_strategy,
        wanted,
        "the row writes the field it was given"
    );
    assert!(
        !effects.simulation_cleared,
        "a spindle-strategy change clears no simulation: the strategy \
         reaches the suggest path and the G-code, never the stock"
    );
    assert!(
        session.simulation_result().is_some(),
        "the session keeps the simulation across a strategy change"
    );
    let refusal = start_refusal(&mut session);
    assert!(
        refusal.is_none(),
        "the rest operation still starts after a strategy change; \
         got: {refusal:?}"
    );
}

// ── A save keeps the simulation ─────────────────────────────────────

/// The core save door writes the file and keeps the simulation.
#[test]
fn a_save_keeps_the_simulation_wp17() {
    let mut session = session_with_simulation();
    let path = temp_project_path("core_save");

    let effects = session
        .apply(Command::SaveProject(SaveProjectArgs { path: path.clone() }))
        .expect("the save writes the fixture file");
    assert!(
        !effects.simulation_cleared,
        "a save changes no session state, so it clears no simulation"
    );
    assert!(
        session.simulation_result().is_some(),
        "the session keeps the simulation across a save"
    );
    let refusal = start_refusal(&mut session);
    assert!(
        refusal.is_none(),
        "the rest operation still starts after a save; got: {refusal:?}"
    );

    let _ = std::fs::remove_file(&path);
}

/// G-RESTRES: a snapshot counts only when it is CURRENT — every row carved
/// before the rest row holds a result, and the record matches the project.
/// The fixture is cold, so row 0 takes an empty result first, and the
/// snapshot takes the record the rule itself derives.
fn make_snapshot_current(
    session: &mut ProjectSession,
    simulation: &mut SimulationResult,
    rest_id: ToolpathId,
) {
    if session.get_result(0).is_none() {
        let revision = session.toolpath_revision(0);
        let _ = session
            .apply(Command::AdoptResult(
                rs_cam_core::session::AdoptResultArgs {
                    index: 0,
                    revision,
                    result: Box::new(rs_cam_core::session::ToolpathComputeResult {
                        op_data: rs_cam_core::ops::drill_op::OpData::Toolpath(Arc::new(
                            rs_cam_core::trace::toolpath_spans::AnnotatedToolpath::new(
                                rs_cam_core::toolpath::Toolpath::new(),
                            ),
                        )),
                        stats: Default::default(),
                        debug_trace: None,
                        semantic_trace: None,
                    }),
                },
            ))
            .expect("the revision is current");
    }
    if let Some(source) = session.expected_source(rest_id) {
        let _ = simulation.prior_stock_sources.insert(rest_id, source);
    }
}
