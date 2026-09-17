//! WP18 sentry — the feed-optimisation refusals are covered again (H5).
//!
//! Programme: `planning/arch_consolidation_2026-09-09/IMPLEMENTATION_PLAN.md`,
//! tech-debt review row H5.
//!
//! # What this file restores
//!
//! WP11b deleted three tests from
//! `crates/rs_cam_viz/src/compute/worker/tests.rs`:
//! `feed_optimization_uses_real_stock_bounds`,
//! `feed_optimization_rejects_remaining_stock` and
//! `feed_optimization_rejects_mesh_derived_operations`. They measured the
//! GUI worker's own `helpers::feed_optimization_stock`. That helper is gone:
//! the core door builds the feed-optimisation stock now
//! (`session/compute.rs`, the `feed_optimization` block of
//! [`execute_job`](rs_cam_core::session::execute_job)), and it gates on
//! `compute::catalog::feed_optimization_unavailable_reason`. No test in the
//! workspace named that function, so the three subjects lost their cover.
//!
//! # The four arms
//!
//! * **(a) The predicate.** `feed_optimization_unavailable_reason` answers
//!   with the remaining-stock reason, the rest-machining reason and the
//!   mesh-derived reason, and answers `None` for a fresh-stock 2D pocket.
//!   The three strings are asserted verbatim, because the GUI panel and the
//!   log line both show the operator this text.
//! * **(b) The real stock bounds.** The handle's own request snapshot
//!   reports the session's stock bounding box. That bounding box is the
//!   argument the door hands to `TriDexelStock::from_bounds`.
//! * **(c) The pass is live.** On the same fresh pocket the dial ON leaves
//!   a DIFFERENT number of cutting moves at a feed the operation did not
//!   command than the dial OFF leaves. The pair makes arm (d) a measurement
//!   rather than a tautology. The arm asserts no direction; the doc comment
//!   on the arm says why.
//! * **(d) The refusal.** A `StockSource::FromRemainingStock` pocket with the
//!   dial ON generates, cuts, and reports the same feed count as the same
//!   operation with the dial OFF. The refusal makes the dial inert; it does
//!   not fail the generation.
//!
//! Arm (c) compares the two counts and never asserts a zero. The dial OFF
//! count is NOT zero on this fixture: the pocket's plunge descents run at the
//! operation's plunge rate, which the pass never wrote. WP11b's own red
//! output measured that second feed on the door that carried no
//! feed-optimisation stock (`feed_rate_count: gui 5 vs session 2`,
//! `min_feed_rate: gui 1000.0 vs session 500.0`).
//!
//! # Red before the fix
//!
//! There is none, and that is deliberate. This file restores COVERAGE; it
//! fixes no defect. Its red-first evidence is the absence it closes:
//! at the parent commit `rg -c feed_optimization_unavailable_reason
//! crates/*/tests` matched no file. An arm here goes red when a refusal
//! breaks.
//!
//! # NOT MEASURED
//!
//! * The dexel cell size, `(tool.diameter / 4.0).clamp(0.25, 2.0)`, and the
//!   `TriDexelStock` the door builds. `ResolvedGenInputs` keeps its fields
//!   private and the stock is a local, so neither is reachable from an
//!   integration test. That the bounding box arm (b) reads is the SAME field
//!   `from_bounds` receives is established by reading `session/compute.rs`,
//!   not by this test.
//! * The log line the refusal writes. The door reports the reason through
//!   `tracing::warn!`, and this test installs no subscriber. Arm (d)
//!   measures the refusal by its effect on the emitted feeds instead.
//! * A non-identity setup. The fixture is a `FaceUp::Top` setup, where the
//!   emission bounding box is the world stock bounding box
//!   (`session/eval_context.rs`, `heights_stock_bbox`).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::catalog::{
    OperationConfig, OperationType, feed_optimization_unavailable_reason,
};
use rs_cam_core::compute::config::StockSource;
use rs_cam_core::compute::operation_configs::PocketConfig;
use rs_cam_core::compute::simulate::SimulationResult;
use rs_cam_core::dexel_stock::TriDexelStock;
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::session::{
    AddToolpathArgs, AdoptSimulationArgs, Command, GenObserver, GenerateToolpathArgs,
    GenerateToolpathHandle, Job, JobHandle, ProjectSession, ToolpathComputeResult, execute_job,
};
use rs_cam_core::stock::stock_mesh::StockMesh;
use rs_cam_core::toolpath::Toolpath;

mod common;
use common::session::{
    polygon_model, single_op_session_with, square_polygon, stock_under, toolpath_config,
};
use common::tools::endmill_tool_config;

/// The refusal a `StockSource::FromRemainingStock` operation reads.
const REMAINING_STOCK_REASON: &str =
    "Phase 1 feed optimization only supports fresh stock, not remaining-stock workflows.";

/// The refusal a rest operation reads.
const REST_REASON: &str =
    "Rest machining depends on prior tool removal, so feed optimization is disabled for now.";

/// The refusal a mesh-derived operation reads.
const MESH_DERIVED_REASON: &str = "Phase 1 feed optimization only supports operations that start from flat stock, not mesh-derived surfaces.";

/// Half-extent (mm) of the square the pocket clears — a 40 x 40 mm region.
/// The fixture `gen_inputs_one_assembly_n12.rs` builds.
const HALF: f64 = 20.0;

/// Stock height (mm). `stock_under` hangs the board below `z = 0`, the frame
/// a 2D operation cuts in, so the pocket's levels sit inside material. A
/// pocket that cuts air modulates no feed and every arm below is vacuous.
const STOCK_Z: f64 = 6.0;

/// Cutter diameter (mm).
const TOOL_D: f64 = 6.0;

/// Dexel cell (mm) of the remaining-stock snapshot arm (d) adopts.
///
/// Nothing reads the grid: the snapshot carries full material over the whole
/// board, so the operation that follows it cuts what a fresh pocket cuts.
const SNAPSHOT_CELL_MM: f64 = 1.0;

// ── Fixture ─────────────────────────────────────────────────────────

fn pocket_op() -> OperationConfig {
    OperationConfig::Pocket(PocketConfig {
        stepover: 2.0,
        depth: 3.0,
        depth_per_pass: 1.5,
        ..PocketConfig::default()
    })
}

/// One stock, one tool, one model, one ungenerated fresh-stock pocket.
///
/// `feed_optimization` writes the dressup dial. `DressupConfig::for_op`
/// leaves it ON, so the `true` arm is the shipped configuration.
fn pocket_session(feed_optimization: bool) -> ProjectSession {
    single_op_session_with(
        stock_under(HALF, STOCK_Z),
        endmill_tool_config(TOOL_D),
        polygon_model(vec![square_polygon(HALF)], "square"),
        "Pocket",
        pocket_op(),
        |cfg| cfg.dressups.feed_optimization = feed_optimization,
    )
}

/// A simulation that carries one prior-stock snapshot and nothing else.
///
/// `ProjectSession::start` reads `prior_stocks` alone. Every display field
/// is the fixture's own answer, never a measurement. The shape follows
/// `adopt_simulation_stores_prior_stocks.rs`.
fn full_material_simulation(
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
        column_grid_cell_mm: SNAPSHOT_CELL_MM,
        prior_stocks,
    }
}

/// Two pockets. Index 1 takes the remaining stock of index 0, and the
/// session already holds the snapshot index 1 needs.
///
/// The snapshot covers the whole board and removes nothing, so index 1 cuts
/// what a fresh pocket cuts. Without it `start` refuses the operation, and
/// arm (d) would measure the refusal of the rest precondition instead of the
/// refusal of the feed-optimisation pass.
fn rest_chain_session(feed_optimization: bool) -> ProjectSession {
    let mut session = pocket_session(feed_optimization);
    let tool_id = session.tools()[0].id.0;
    let model_id = session.models()[0].id;
    let mut rest = toolpath_config("Rest", pocket_op(), tool_id, model_id);
    rest.stock_source = StockSource::FromRemainingStock;
    rest.dressups.feed_optimization = feed_optimization;
    let _ = session
        .apply(Command::AddToolpath(AddToolpathArgs {
            setup_index: 0,
            config: Box::new(rest),
        }))
        .expect("the fresh session holds setup 0");

    let rest_id = session.toolpath_configs()[1].id;
    let bbox = session.stock_bbox();
    let snapshot = Arc::new(TriDexelStock::from_bounds(&bbox, SNAPSHOT_CELL_MM));
    let _ = session
        .apply(Command::AdoptSimulation(AdoptSimulationArgs {
            result: Box::new(full_material_simulation(rest_id, &snapshot)),
            epoch: session.simulation_epoch(),
        }))
        .expect("the adopt stores the simulation");
    session
}

/// The operation's commanded cutting feed, in mm/min.
fn commanded_feed(session: &ProjectSession, index: usize) -> f64 {
    session.toolpath_configs()[index].operation.feed_rate()
}

/// Step (i): capture the generation inputs.
fn start_job(session: &mut ProjectSession, index: usize) -> GenerateToolpathHandle {
    let cancel = AtomicBool::new(false);
    let JobHandle::GenerateToolpath(handle) = session
        .start(
            Job::GenerateToolpath(GenerateToolpathArgs { index }),
            &cancel,
        )
        .expect("step (i) captures the generation inputs")
    else {
        panic!("the generate_toolpath row answers its own handle variant");
    };
    *handle
}

/// Steps (i) and (ii): capture the inputs, then generate.
fn run_job(session: &mut ProjectSession, index: usize) -> ToolpathComputeResult {
    let handle = start_job(session, index);
    let cancel = AtomicBool::new(false);
    execute_job(&handle, &GenObserver::none(), &cancel).expect("step (ii) generates the toolpath")
}

/// How many cutting moves carry a feed the operation did not command.
///
/// The feed-optimisation pass rewrites the feed rate in place and touches
/// nothing else, so this count is exactly what that pass changed. The
/// quantity is `gen_inputs_one_assembly_n12.rs`'s.
fn modulated_move_count(toolpath: &Toolpath, commanded: f64) -> usize {
    toolpath
        .moves
        .iter()
        .filter(|mv| mv.move_type.is_cutting())
        .filter(|mv| {
            mv.move_type
                .feed_rate()
                .is_some_and(|feed| (feed - commanded).abs() > 1e-9)
        })
        .count()
}

/// How many cutting moves the toolpath holds.
fn cutting_move_count(toolpath: &Toolpath) -> usize {
    toolpath
        .moves
        .iter()
        .filter(|mv| mv.move_type.is_cutting())
        .count()
}

// ── (a) the predicate names each refusal ────────────────────────────

/// Each refusal reads back with its own sentence, and a fresh 2D pocket
/// reads none.
///
/// The three subjects the WP11b deletion left uncovered. The operator sees
/// this text, so the assertion is on the sentence, not on "some reason".
#[test]
fn the_predicate_names_the_three_refusals_and_clears_a_fresh_pocket() {
    let pocket = pocket_op();
    assert_eq!(
        feed_optimization_unavailable_reason(&pocket, StockSource::FromRemainingStock),
        Some(REMAINING_STOCK_REASON),
        "a remaining-stock operation is refused before anything else, \
         whatever the operation is"
    );

    let rest = OperationConfig::new_default(OperationType::Rest);
    assert_eq!(
        feed_optimization_unavailable_reason(&rest, StockSource::Fresh),
        Some(REST_REASON),
        "a rest operation depends on prior tool removal, so it is refused \
         on fresh stock too"
    );

    let drop_cutter = OperationConfig::new_default(OperationType::DropCutter);
    assert!(
        drop_cutter.is_3d(),
        "the mesh-derived arm reads `is_3d`; DropCutter must stay a 3D \
         operation or this arm measures nothing"
    );
    assert_eq!(
        feed_optimization_unavailable_reason(&drop_cutter, StockSource::Fresh),
        Some(MESH_DERIVED_REASON),
        "a mesh-derived operation does not start from flat stock"
    );

    assert_eq!(
        feed_optimization_unavailable_reason(&pocket, StockSource::Fresh),
        None,
        "a fresh-stock 2D pocket is the case the pass supports; a reason \
         here disables the pass everywhere"
    );
}

// ── (b) the door reads the real stock bounds ────────────────────────

/// The captured inputs carry the session's own stock bounding box.
///
/// The deleted `feed_optimization_uses_real_stock_bounds` read the dexel the
/// GUI worker built. The core door builds its dexel from
/// `ResolvedGenInputs::emission_stock_bbox`, which is private, so the
/// measurement here is the same field as the handle's request snapshot
/// publishes. On an identity setup that field is the world stock bounding
/// box.
#[test]
fn the_captured_inputs_carry_the_real_stock_bounds() {
    let mut session = pocket_session(true);
    let bbox = session.stock_bbox();
    let snapshot = start_job(&mut session, 0).request_snapshot();

    for (corner, axis, expected) in [
        ("min", "x", bbox.min.x),
        ("min", "y", bbox.min.y),
        ("min", "z", bbox.min.z),
        ("max", "x", bbox.max.x),
        ("max", "y", bbox.max.y),
        ("max", "z", bbox.max.z),
    ] {
        let got = snapshot["stock_bbox"][corner][axis]
            .as_f64()
            .unwrap_or_else(|| panic!("no stock_bbox.{corner}.{axis}"));
        assert!(
            (got - expected).abs() < 1e-9,
            "the feed-optimisation stock is built from the captured bounding \
             box; stock_bbox.{corner}.{axis} reads {got} and the session's \
             stock reads {expected}"
        );
    }
}

// ── (c) the dial moves the off-commanded feed count ─────────────────

/// The dial ON and the dial OFF report different off-commanded counts.
///
/// Arm (d) reads "the two counts agree" as "the pass did not run". That
/// reading is only sound while the pass moves the count at all. This arm
/// measures that it does.
///
/// The OFF count is not asserted to be zero. The pocket's plunge descents
/// carry the operation's plunge rate, which no pass wrote, so the baseline
/// is a population this arm subtracts rather than a zero it claims.
///
/// The arm asserts NO direction, and the reason is the pass's design.
/// `feedopt::optimize_feed_rates_inner` writes `nominal * factor` onto every
/// cutting move. It reads no move's own feed. A move at partial engagement
/// therefore leaves the pass away from the commanded feed, and it ENTERS
/// this population.
///
/// Since WP22 (G-FEEDOPTPLUNGE) the pass caps a move the shared classifier
/// calls `Plunge` at the operation's own plunge rate. A capped descent sits
/// at the plunge rate, which is not the commanded feed, so it also enters
/// the population. Before WP22 the pass lifted that same descent TO the
/// commanded feed and it left the population instead. The two populations
/// therefore differ in composition as well as in size, which is why this
/// arm grades the counts as unequal and names no direction. The pre-WP22
/// pair of counts is retired; the current pair is not re-stated here.
#[test]
fn the_feed_optimisation_dial_moves_the_off_commanded_feed_count() {
    let mut on = pocket_session(true);
    let commanded = commanded_feed(&on, 0);
    let with_pass = run_job(&mut on, 0);
    let cutting = cutting_move_count(with_pass.toolpath());
    assert!(
        cutting > 0,
        "the fixture generated no cutting motion, so every feed count below \
         is vacuous"
    );
    let modulated_on = modulated_move_count(with_pass.toolpath(), commanded);
    assert!(
        modulated_on > 0,
        "the core door reads a feed-optimisation stock, so at least one of \
         the {cutting} cutting moves must carry a feed the operation did not \
         command ({commanded} mm/min); none did"
    );

    let mut off = pocket_session(false);
    let without_pass = run_job(&mut off, 0);
    let modulated_off = modulated_move_count(without_pass.toolpath(), commanded);
    assert!(
        modulated_on != modulated_off,
        "the dial must move the count it is the only lever on: the dial ON \
         reports {modulated_on} cutting moves away from the commanded \
         {commanded} mm/min and the dial OFF reports {modulated_off}. Equal \
         counts make arm (d) vacuous"
    );
}

// ── (d) the refusal makes the dial inert, not the generation fail ───

/// A remaining-stock operation generates, and the dial changes nothing.
///
/// The door reports the refusal and continues, which is what the GUI worker
/// did. The measurement is a paired A/B over one operation: the dial ON and
/// the dial OFF must report one feed count. Arm (c) shows the same pair
/// disagrees where the pass is allowed to run.
#[test]
fn a_remaining_stock_operation_generates_with_the_dial_inert() {
    let mut on = rest_chain_session(true);
    let commanded = commanded_feed(&on, 1);
    let with_dial = run_job(&mut on, 1);
    let cutting = cutting_move_count(with_dial.toolpath());
    assert!(
        cutting > 0,
        "the remaining-stock operation cut nothing, so the feed counts below \
         are vacuous; the adopted snapshot must hold full material"
    );

    let mut off = rest_chain_session(false);
    let without_dial = run_job(&mut off, 1);

    let modulated_on = modulated_move_count(with_dial.toolpath(), commanded);
    let modulated_off = modulated_move_count(without_dial.toolpath(), commanded);
    assert_eq!(
        modulated_on, modulated_off,
        "feed optimisation is refused on remaining stock, so the dial is \
         inert: the dial ON reports {modulated_on} moves at a feed the \
         operation did not command and the dial OFF reports {modulated_off}"
    );
    assert_eq!(
        cutting,
        cutting_move_count(without_dial.toolpath()),
        "the refusal skips one pass; it changes no geometry"
    );
}
