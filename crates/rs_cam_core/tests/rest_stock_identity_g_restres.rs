//! G-RESTRES and G-RESTSTALE — a rest result records the stock it read, and
//! a change to that stock drops it.
//!
//! Plan: `planning/rest_stock_identity_2026-09-24/PLAN.md` §3 and §5.
//!
//! # The defects
//!
//! - G-RESTRES (measured on rivmap100, 2026-09-24): the same rest
//!   parameters gave 7925 moves after a 0.2 mm simulation and 5373 after a
//!   0.5 mm one. The snapshot key named only the consumer, so nothing could
//!   tell the two snapshots apart, and a simulation at another cell staled
//!   nothing.
//! - G-RESTSTALE (M-A): a predecessor whose output changed with no input
//!   edit left the consumer's snapshot and result standing.
//! - G-RESTSTALE (M-B): a DISABLED consumer has no Stock edge, so an edit
//!   above it did not drop it, and re-enable kept the stale result.
//!
//! # Red before the fix
//!
//! Each test below was run with the fix switched off —
//! `ProjectSession::snapshot_is_current` answering on key presence and
//! `drop_out_of_date_rest_results` a no-op, the two rules the code had
//! before — and failed. The file records the arm that failed on each.
//!
//! # The fixture
//!
//! A 40 mm square polygon, a Ø6 mm end mill, a 6 mm board below Z0, and a
//! chain of pockets: index 0 cuts fresh stock, every later one takes the
//! remaining stock of the ones above it. Cheap: a 2D pocket at a 1 mm cell.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::StockSource;
use rs_cam_core::compute::operation_configs::PocketConfig;
use rs_cam_core::session::generation_plan::{Scope, Step, plan};
use rs_cam_core::session::{
    AddToolpathArgs, AdoptResultArgs, Command, GenerateToolpathArgs, Job, ProjectSession,
    SetSimulationResolutionArgs, SetToolpathEnabledArgs, SetToolpathParamArgs, SimulationOptions,
    SimulationResolution, SnapshotMiss, ToolpathComputeResult,
};
use rs_cam_core::trace::toolpath_spans::AnnotatedToolpath;

mod common;
use common::session::{
    polygon_model, single_op_session, square_polygon, stock_under, toolpath_config,
};
use common::tools::endmill_tool_config;

const HALF: f64 = 20.0;
const STOCK_Z: f64 = 6.0;
const TOOL_D: f64 = 6.0;
/// The project cell the fixture stores.
const CELL_MM: f64 = 1.0;
/// Another cell, for "a simulation at another resolution".
const OTHER_CELL_MM: f64 = 2.0;

fn pocket_op() -> OperationConfig {
    OperationConfig::Pocket(PocketConfig {
        stepover: 2.0,
        depth: 3.0,
        depth_per_pass: 3.0,
        ..PocketConfig::default()
    })
}

/// Index 0 fresh, then `rest_count` rest pockets. The stored cell is
/// `Fixed(CELL_MM)`.
fn chain(rest_count: usize) -> ProjectSession {
    let mut session = single_op_session(
        stock_under(HALF, STOCK_Z),
        endmill_tool_config(TOOL_D),
        polygon_model(vec![square_polygon(HALF)], "square"),
        "Pocket",
        pocket_op(),
    );
    let tool_id = session.tools()[0].id.0;
    let model_id = session.models()[0].id;
    for k in 0..rest_count {
        let mut rest = toolpath_config(&format!("Rest {k}"), pocket_op(), tool_id, model_id);
        rest.stock_source = StockSource::FromRemainingStock;
        let _ = session
            .apply(Command::AddToolpath(AddToolpathArgs {
                setup_index: 0,
                config: Box::new(rest),
            }))
            .expect("setup 0 exists");
    }
    let _ = session
        .apply(Command::SetSimulationResolution(
            SetSimulationResolutionArgs {
                resolution: SimulationResolution::Fixed(CELL_MM),
            },
        ))
        .expect("a positive cell");
    session
}

fn simulate_at(session: &mut ProjectSession, cell_mm: f64) {
    let cancel = AtomicBool::new(false);
    let opts = SimulationOptions {
        resolution: cell_mm,
        // Metrics observe the carve and never change it
        // (`metric_and_plain_carve_agree_g_restres.rs`).
        metrics_enabled: false,
        adaptive_feed_modulation: false,
        ..SimulationOptions::default()
    };
    let _ = session.run_simulation(&opts, &cancel).expect("simulate");
}

fn simulate(session: &mut ProjectSession) {
    let cell = session.simulation_resolution_mm();
    simulate_at(session, cell);
}

fn generate(session: &mut ProjectSession, index: usize) {
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(index, &cancel)
        .unwrap_or_else(|e| panic!("generate {index}: {e}"));
}

/// Walk the chain the way the CLI does: generate, simulate, generate.
fn make_current(session: &mut ProjectSession) {
    for step in plan(session, Scope::Project) {
        match step {
            Step::Simulate { .. } => simulate(session),
            Step::Generate { index, .. } => generate(session, index),
        }
    }
}

fn simulate_steps(session: &ProjectSession) -> usize {
    plan(session, Scope::Project)
        .iter()
        .filter(|s| matches!(s, Step::Simulate { .. }))
        .count()
}

fn start_refusal(session: &mut ProjectSession, index: usize) -> Option<String> {
    let cancel = AtomicBool::new(false);
    session
        .start(
            Job::GenerateToolpath(GenerateToolpathArgs { index }),
            &cancel,
        )
        .err()
        .map(|e| e.to_string())
}

/// Adopt a DIFFERENT output for `index` with no input edit: its first
/// half of moves. The shape M-A needs: the output moved, the revision did
/// not.
fn adopt_other_output(session: &mut ProjectSession, index: usize) {
    let held = session.get_result(index).expect("a result to vary");
    let mut toolpath = held.toolpath().clone();
    let keep = toolpath.moves.len() / 2;
    toolpath.moves.truncate(keep.max(2));
    let other = ToolpathComputeResult {
        op_data: rs_cam_core::ops::drill_op::OpData::Toolpath(Arc::new(AnnotatedToolpath::new(
            toolpath,
        ))),
        stats: held.stats.clone(),
        debug_trace: None,
        semantic_trace: None,
    };
    let revision = session.toolpath_revision(index);
    let _ = session
        .apply(Command::AdoptResult(AdoptResultArgs {
            index,
            revision,
            result: Box::new(other),
        }))
        .expect("the revision is current");
}

// ── G-RESTRES ───────────────────────────────────────────────────────

/// The result records the cell it read. A simulation at another cell is
/// not a current snapshot, and changing the stored cell drops the result.
///
/// Red with the fix off: `snapshot_is_current` answered `Ok` after the
/// 2 mm simulation (key present), and the 0.5 mm `Fixed` left the result.
#[test]
fn a_rest_result_records_its_cell_and_drops_when_the_cell_changes() {
    let mut session = chain(1);
    make_current(&mut session);
    let rest_id = session.toolpath_configs()[1].id;

    let source = session
        .get_result(1)
        .expect("the rest op generated")
        .stats
        .source_stock
        .clone()
        .expect("a rest result records its source stock");
    assert_eq!(source.cell_mm.to_bits(), CELL_MM.to_bits());
    assert_eq!(
        source.after.len(),
        1,
        "the rest pocket read the fresh pocket's carve"
    );

    // A simulation at another cell replaces the snapshot. It is not the
    // project's, so the plan simulates again and `start` refuses it.
    simulate_at(&mut session, OTHER_CELL_MM);
    assert!(matches!(
        session.snapshot_is_current(rest_id),
        Err(SnapshotMiss::OtherCell { .. })
    ));
    assert_eq!(simulate_steps(&session), 1);

    // The result itself read the stored cell, so it stays current.
    assert!(session.get_result(1).is_some());

    // `start` clears the slot it regenerates, then refuses the snapshot.
    let refusal = start_refusal(&mut session, 1).expect("a snapshot at another cell is refused");
    assert!(
        refusal.contains("mm cells"),
        "the refusal names the cell: {refusal}"
    );
    make_current(&mut session);
    assert!(session.get_result(1).is_some());

    // A new stored cell drops the simulation and the result.
    let effects = session
        .apply(Command::SetSimulationResolution(
            SetSimulationResolutionArgs {
                resolution: SimulationResolution::Fixed(0.5),
            },
        ))
        .expect("a positive cell");
    assert!(
        effects.stale.contains(&1),
        "Effects names the dropped rest row"
    );
    assert!(
        !effects.stale.contains(&0),
        "the fresh pocket read no stock"
    );
    assert!(session.get_result(1).is_none());
    assert!(session.simulation_result().is_none());
}

/// Nothing that leaves the stock the same stales anything.
#[test]
fn a_repeat_simulation_or_an_equal_regenerate_stales_nothing() {
    let mut session = chain(1);
    make_current(&mut session);

    simulate(&mut session);
    assert!(session.get_result(1).is_some());
    assert_eq!(
        simulate_steps(&session),
        0,
        "a covering simulation plans none"
    );

    // Regenerate the source: same inputs, same moves, a new `Arc`.
    generate(&mut session, 0);
    assert!(
        session.get_result(1).is_some(),
        "an equal regenerate keeps the rest result"
    );
    assert_eq!(simulate_steps(&session), 0);
    assert!(start_refusal(&mut session, 1).is_none());
}

/// `Auto` is one project-wide number: the rest tool's rule, whatever plan
/// or request asks.
#[test]
fn auto_is_worked_out_over_the_whole_project() {
    let mut session = chain(1);
    let _ = session
        .apply(Command::SetSimulationResolution(
            SetSimulationResolutionArgs {
                resolution: SimulationResolution::Auto,
            },
        ))
        .unwrap();
    // Ø6 rest tool: TIP radius 3 / 5 = 0.6; the tool rule clamps to 0.5.
    let auto = session.simulation_resolution_mm();
    assert!((auto - 0.5).abs() < 1e-12, "auto = {auto}");
    assert_eq!(session.rest_resolution_required_mm(), Some(0.6));
}

// ── G-RESTSTALE, M-A ────────────────────────────────────────────────

/// A predecessor adopts a different output with no input edit. The
/// consumer's result drops, its snapshot is not current, and the plan
/// simulates again.
///
/// Red with the fix off: the rest result stayed, and the plan planned no
/// Simulate step because the key was present.
#[test]
fn a_predecessor_output_change_drops_the_rest_result() {
    let mut session = chain(1);
    make_current(&mut session);
    let rest_id = session.toolpath_configs()[1].id;

    adopt_other_output(&mut session, 0);

    assert!(
        session.get_result(1).is_none(),
        "M-A: the rest result must drop"
    );
    assert!(matches!(
        session.snapshot_is_current(rest_id),
        Err(SnapshotMiss::SourceMoved { .. })
    ));
    assert_eq!(simulate_steps(&session), 1);
    assert!(start_refusal(&mut session, 1).is_some());
}

/// The tail of a chain drops when the middle moves.
#[test]
fn a_middle_output_change_drops_the_tail() {
    let mut session = chain(2);
    make_current(&mut session);
    assert!(session.get_result(2).is_some());

    adopt_other_output(&mut session, 1);

    assert!(
        session.get_result(2).is_none(),
        "the tail read the old middle"
    );
}

// ── G-RESTSTALE, M-B ────────────────────────────────────────────────

/// A disabled consumer keeps no stale result through an edit above it.
///
/// Red with the fix off: re-enable showed the rest result generated
/// against the OLD pocket.
#[test]
fn a_disabled_consumer_does_not_keep_a_stale_result() {
    let mut session = chain(1);
    make_current(&mut session);
    let moves_before = session.get_result(1).unwrap().toolpath().moves.len();
    assert!(moves_before > 0);

    let _ = session
        .apply(Command::SetToolpathEnabled(SetToolpathEnabledArgs {
            index: 1,
            enabled: false,
        }))
        .unwrap();
    let _ = session
        .apply(Command::SetToolpathParam(SetToolpathParamArgs {
            index: 0,
            param: "stepover".to_owned(),
            value: serde_json::json!(3.0),
        }))
        .unwrap();
    generate(&mut session, 0);
    let _ = session
        .apply(Command::SetToolpathEnabled(SetToolpathEnabledArgs {
            index: 1,
            enabled: true,
        }))
        .unwrap();

    assert!(
        session.get_result(1).is_none(),
        "M-B: the re-enabled rest op must not read Current on the old stock"
    );
}

// ── The project file ────────────────────────────────────────────────

/// `Fixed` round-trips through the file; a file with no key loads `Auto`.
#[test]
fn the_stored_resolution_round_trips_and_a_missing_key_is_auto() {
    let dir = std::env::temp_dir().join(format!("rs_cam_restres_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    let session = chain(1);
    let path = dir.join("fixed.toml");
    session.save(&path).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("resolution_mm = 1.0"), "saved:\n{text}");
    let loaded = ProjectSession::load(&path).unwrap();
    assert_eq!(
        loaded.simulation_resolution(),
        SimulationResolution::Fixed(CELL_MM)
    );

    let mut auto = chain(1);
    let _ = auto
        .apply(Command::SetSimulationResolution(
            SetSimulationResolutionArgs {
                resolution: SimulationResolution::Auto,
            },
        ))
        .unwrap();
    let path = dir.join("auto.toml");
    auto.save(&path).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(
        !text.contains("resolution_mm"),
        "Auto writes no key:\n{text}"
    );
    let loaded = ProjectSession::load(&path).unwrap();
    assert_eq!(loaded.simulation_resolution(), SimulationResolution::Auto);

    let _ = std::fs::remove_dir_all(&dir);
}
