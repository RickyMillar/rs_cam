//! G-RESTLEVELS — a rest Pocket cuts only where the remaining stock stands.
//!
//! # The defect this file pins
//!
//! A 2.5D Pocket with `StockSource::FromRemainingStock` planned every Z
//! level from `top_z` down and ignored the machined stock. On the operator's
//! project, op Z23 re-cut 112 levels that op Z22 had already cleared, and
//! 93.5 % of its cut time was air. The operator's expectation: "if I select
//! rest I would expect the air cutting to be removed and only mill the left
//! over".
//!
//! The fix (`compute/execute/standing_material.rs`) tests each level against
//! the stock. A level with no material in the reach of the tool is skipped.
//! A level with material in part of the outline cuts only that part.
//!
//! # The fixture
//!
//! One Ø6 mm end mill, a 10 mm board below world Z0, and two pockets on a
//! 30 mm square. Pocket A cuts fresh stock to 5 mm. Pocket B follows it.
//! The stock covers A and a copy of B shifted 10 mm in +X, with 2 mm of
//! margin. Each arm generates A, simulates, generates B, and simulates
//! again.
//!
//! # The claims
//!
//! 1. Non-vacuity: B as Fresh cuts every level of its ladder.
//! 2. B as rest on A's outline cuts nothing at or above A's floor.
//! 3. B as rest, shifted: above A's floor every cut lies near the material
//!    A left in B, and the strip outside A is cut at every level.
//! 4. B as rest to A's own depth: an empty result, and no refusal.
//! 5. No undercut: the final stock of a rest B equals the final stock of a
//!    fresh B, cell for cell, over the cells B's tool covers completely, on
//!    both outlines.
//! 6. A rest pocket that the clip makes empty does not block the rest pocket
//!    after it: one empty, two empties in a row, and across a setup.
//!
//! # The other ops (W3.4, `Capabilities::rest_use`)
//!
//! 7. Zigzag (`LevelClip`): on A's outline it cuts only below A's floor and
//!    leaves the stock a fresh Zigzag leaves.
//! 8. Profile (`LevelSkip`): on A's outline it skips each level with no
//!    material in the reach of its path. On the shifted outline material
//!    stands in its reach at every level, so it cuts the fresh contour at
//!    every level: a rest Profile never clips its outline into a new wall.
//! 9. Face (`LevelClip` on the stock rectangle): after a Face to 3 mm, a
//!    rest Face to 5 mm skips the levels at or above the remaining top and
//!    cuts the two levels below it as a fresh Face does.
//! 10. Drill (`StockColumn`, D8): a hole in A starts at A's floor. Its
//!     R-plane, where its pecks start, keeps the clearance a fresh hole has
//!     above the stock top. A hole outside A keeps its fresh R-plane.
//! 11. Chamfer (`LevelSkip`, W3.5): after a Face below the chamfer level,
//!     a rest Chamfer skips its one level and gives an empty result. After
//!     a Face above the chamfer level, it cuts the fresh rings.
//! 12. 2D Adaptive (`LevelMask`): on A's outline it skips the levels A
//!     cleared and cuts only below A's floor. Shifted, above A's floor it
//!     cuts and enters only near the material A left (B10). It
//!     leaves the stock a fresh Adaptive leaves, with no material above its
//!     floor in the cells its tool covers.
//! 13. B8, Face on a whole top: a rest Face cuts the rows of a fresh Face.
//! 14. B8, Face on part of the top: each rest row lies on a fresh row and
//!     inside that row's X span.
//! 15. B11, Drill: a peck through air is dropped, every move keeps a known
//!     intent, and the holes read back as native canned cycles.
//! 16. B9, Zigzag op and Pocket zigzag on part of the outline: each rest
//!     row lies on a fresh row, in that row's direction and span.
//!
//! Each rule has a red proof: with the rule off, the claim fails. The red
//! runs are recorded in the W3.4 report.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
// The measured numbers ARE the record.
#![allow(clippy::print_stderr)]

mod common;

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use common::session::{generate, polygon_model, square_polygon, toolpath_config};
use common::tools::endmill_tool_config;
use rs_cam_core::compute::StockConfig;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::StockSource;
use rs_cam_core::compute::operation_configs::{
    AdaptiveConfig, ChamferConfig, DrillConfig, FaceConfig, PocketConfig, PocketPattern,
    ProfileConfig, ZigzagConfig,
};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::compute::toolpath_stats::RestLevelsFinding;
use rs_cam_core::compute::transform::FaceUp;
use rs_cam_core::dexel_stock::{FRESH_MATERIAL_THRESHOLD_MM, TriDexelStock};
use rs_cam_core::geo::P2;
use rs_cam_core::io::dxf_input::{DrillTarget, DrillTargetKind};
use rs_cam_core::ops::drill_op::DrillHole;
use rs_cam_core::ops::profile::ProfileSide;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{
    Command, ProjectSession, ProjectSessionBuilder, SetSimulationResolutionArgs, SimulationOptions,
    SimulationResolution,
};
use rs_cam_core::toolpath::{Move, MoveIntent, MoveType};

/// Half-extent (mm) of the square both pockets clear.
const HALF: f64 = 15.0;

/// The +X shift (mm) of B in the partial-overlap arm.
const SHIFT: f64 = 10.0;

/// Stock margin (mm) around A ∪ shifted B.
const MARGIN: f64 = 2.0;

/// Stock height (mm). The board hangs below `z = 0`.
const STOCK_Z: f64 = 10.0;

/// Cutter diameter (mm).
const TOOL_D: f64 = 6.0;

/// Cutter radius (mm).
const TOOL_R: f64 = TOOL_D / 2.0;

/// Simulation cell (mm).
const CELL_MM: f64 = 0.5;

/// Depth (mm) of A. B's rest levels must stay below `-A_DEPTH`.
const A_DEPTH: f64 = 5.0;

/// Depth (mm) of B: two levels below A's floor.
const B_DEPTH: f64 = 7.0;

/// Depth per pass (mm) of both pockets: one level per millimetre.
const DPP: f64 = 1.0;

fn stock() -> StockConfig {
    StockConfig {
        x: 2.0 * HALF + SHIFT + 2.0 * MARGIN,
        y: 2.0 * HALF + 2.0 * MARGIN,
        z: STOCK_Z,
        origin_x: -HALF - MARGIN,
        origin_y: -HALF - MARGIN,
        origin_z: -STOCK_Z,
        auto_from_model: false,
        ..StockConfig::default()
    }
}

fn pocket(depth: f64) -> OperationConfig {
    pocket_with(depth, PocketPattern::Contour)
}

fn pocket_with(depth: f64, pattern: PocketPattern) -> OperationConfig {
    OperationConfig::Pocket(PocketConfig {
        stepover: 2.0,
        depth,
        depth_per_pass: DPP,
        pattern,
        ..PocketConfig::default()
    })
}

/// A's square shifted `dx` in +X, wound CCW.
fn shifted_square(dx: f64) -> Polygon2 {
    Polygon2::new(vec![
        P2::new(-HALF + dx, -HALF),
        P2::new(HALF + dx, -HALF),
        P2::new(HALF + dx, HALF),
        P2::new(-HALF + dx, HALF),
    ])
}

/// What one arm left behind.
struct Arm {
    moves: Vec<Move>,
    final_stock: Arc<TriDexelStock>,
    finding: Option<RestLevelsFinding>,
    /// The holes of the drill-op view (D8), empty for any other op.
    drill_holes: Vec<DrillHole>,
}

/// Generate A, simulate, generate B, simulate again.
fn run(b_shift: f64, b_depth: f64, b_source: StockSource) -> Arm {
    run_with(b_shift, b_depth, b_source, PocketPattern::Contour)
}

/// [`run`] with the pocket pattern of B.
fn run_with(b_shift: f64, b_depth: f64, b_source: StockSource, pattern: PocketPattern) -> Arm {
    run_ops(
        pocket(A_DEPTH),
        pocket_with(b_depth, pattern),
        b_shift,
        b_source,
        &[],
    )
}

/// Generate the op `a` on A's outline, simulate, generate the op `b` on
/// A's outline shifted `b_shift` in +X (with the drill targets
/// `targets`), and simulate again. Both ops use the Ø[`TOOL_D`] end mill.
fn run_ops(
    a: OperationConfig,
    b: OperationConfig,
    b_shift: f64,
    b_source: StockSource,
    targets: &[[f64; 2]],
) -> Arm {
    run_ops_with_tool(a, b, None, b_shift, b_source, targets)
}

/// [`run_ops`] with `b_tool` for the op `b` (`None`: the end mill of A).
fn run_ops_with_tool(
    a: OperationConfig,
    b: OperationConfig,
    b_tool: Option<ToolConfig>,
    b_shift: f64,
    b_source: StockSource,
    targets: &[[f64; 2]],
) -> Arm {
    let mut builder = ProjectSessionBuilder::new().stock(stock());
    let tool_idx = builder.add_tool(endmill_tool_config(TOOL_D));
    let tool_id = builder.tools()[tool_idx].id.0;
    let b_tool_id = match b_tool {
        Some(tool) => {
            let idx = builder.add_tool(tool);
            builder.tools()[idx].id.0
        }
        None => tool_id,
    };
    let model_a = builder.add_model(polygon_model(vec![square_polygon(HALF)], "a"));
    let mut b_model = polygon_model(vec![shifted_square(b_shift)], "b");
    b_model.drill_targets = Arc::new(
        targets
            .iter()
            .map(|&[x, y]| DrillTarget {
                x,
                y,
                layer: "holes".to_owned(),
                kind: DrillTargetKind::Point,
            })
            .collect(),
    );
    let model_b = builder.add_model(b_model);
    let _ = builder
        .add_toolpath(0, toolpath_config("A", a, tool_id, model_a))
        .expect("add A");
    let mut b = toolpath_config("B", b, b_tool_id, model_b);
    b.stock_source = b_source;
    let _ = builder.add_toolpath(0, b).expect("add B");
    let mut session = builder.build();

    generate(&mut session, 0);
    let _ = session
        .apply(Command::SetSimulationResolution(
            SetSimulationResolutionArgs {
                resolution: SimulationResolution::Fixed(CELL_MM),
            },
        ))
        .expect("a positive cell");
    simulate(&mut session);
    generate(&mut session, 1);
    simulate(&mut session);

    let result = session.get_result(1).expect("B holds a result");
    let final_stock = session
        .simulation_result()
        .and_then(|sim| sim.checkpoints.last().map(|c| Arc::clone(&c.mesh_stock)))
        .expect("the second simulation carved B");
    Arm {
        moves: result.toolpath().moves.clone(),
        final_stock,
        finding: result.stats.rest_levels,
        drill_holes: result
            .op_data
            .drill_op()
            .map(|d| d.holes.clone())
            .unwrap_or_default(),
    }
}

fn simulate(session: &mut ProjectSession) {
    let cancel = AtomicBool::new(false);
    let _ = session
        .run_simulation(
            &SimulationOptions {
                resolution: CELL_MM,
                auto_resolution: false,
                adaptive_feed_modulation: false,
                ..SimulationOptions::default()
            },
            &cancel,
        )
        .expect("simulation");
}

/// The cutting moves: feed moves that the planner marks as a cut. Entry
/// and link moves are a dressup's business and are not judged here.
fn cutting_moves(moves: &[Move]) -> Vec<&Move> {
    moves
        .iter()
        .filter(|m| {
            m.move_type != MoveType::Rapid
                && matches!(m.intent, MoveIntent::ClearingCut | MoveIntent::FinishingCut)
        })
        .collect()
}

/// The ladder of B: `-DPP, -2·DPP, …, -depth`.
fn ladder(depth: f64) -> Vec<f64> {
    let n = (depth / DPP).round() as usize;
    (1..=n).map(|i| -(i as f64) * DPP).collect()
}

fn cuts_at(moves: &[&Move], z: f64) -> Vec<P2> {
    moves
        .iter()
        .filter(|m| (m.target.z - z).abs() < 1e-6)
        .map(|m| P2::new(m.target.x, m.target.y))
        .collect()
}

/// 1. Non-vacuity: a fresh B cuts every level, so a rest B that cuts fewer
///    levels is a change the clip made.
#[test]
fn a_fresh_pocket_cuts_every_level() {
    let arm = run(0.0, B_DEPTH, StockSource::Fresh);
    let cuts = cutting_moves(&arm.moves);
    for z in ladder(B_DEPTH) {
        assert!(
            !cuts_at(&cuts, z).is_empty(),
            "a fresh pocket must cut level {z}"
        );
    }
    assert_eq!(
        arm.finding, None,
        "a fresh pocket reads no stock, so it tests no level"
    );
}

/// 2. Same outline: every cut of the rest B lies below A's floor.
#[test]
fn a_rest_pocket_on_the_same_outline_cuts_only_below_the_prior_floor() {
    let arm = run(0.0, B_DEPTH, StockSource::FromRemainingStock);
    let cuts = cutting_moves(&arm.moves);
    let floor = -A_DEPTH - FRESH_MATERIAL_THRESHOLD_MM;
    let highest = cuts
        .iter()
        .map(|m| m.target.z)
        .fold(f64::NEG_INFINITY, f64::max);
    eprintln!(
        "same outline: {} cutting moves, highest z {highest:.3}, finding {:?}",
        cuts.len(),
        arm.finding
    );
    assert!(!cuts.is_empty(), "the rest pocket must cut below A's floor");
    for m in &cuts {
        assert!(
            m.target.z < floor,
            "a cut at z = {} lies at or above A's floor {floor}",
            m.target.z
        );
    }
    let finding = arm.finding.expect("a rest pocket records its level clip");
    assert_eq!(finding.planned_levels, ladder(B_DEPTH).len());
    assert_eq!(
        finding.skipped_levels,
        ladder(A_DEPTH).len(),
        "every level A already cut must be skipped"
    );
    assert_eq!(finding.fallback, None);
}

/// 3. Shifted outline: above A's floor every cut lies near the material A
///    left inside B, and the strip outside A is cut at every level.
#[test]
fn a_shifted_rest_pocket_cuts_only_the_strip_above_the_prior_floor() {
    let arm = run(SHIFT, B_DEPTH, StockSource::FromRemainingStock);
    let cuts = cutting_moves(&arm.moves);
    eprintln!(
        "shifted: {} cutting moves, finding {:?}",
        cuts.len(),
        arm.finding
    );

    // The material A left inside B: B minus A's footprint, the square with
    // corners rounded to the tool radius. Sampled on a quarter cell.
    let step = CELL_MM / 4.0;
    let inner = HALF - TOOL_R;
    let mut left = Vec::new();
    let nx = ((2.0 * HALF) / step).round() as usize;
    for i in 0..=nx {
        for j in 0..=nx {
            let q = P2::new(-HALF + SHIFT + i as f64 * step, -HALF + j as f64 * step);
            let dx = (q.x.abs() - inner).max(0.0);
            let dy = (q.y.abs() - inner).max(0.0);
            if dx.hypot(dy) > TOOL_R {
                left.push(q);
            }
        }
    }
    let bound = TOOL_R + 2.0 * CELL_MM;
    let mut worst: f64 = 0.0;
    for z in ladder(A_DEPTH) {
        for p in cuts_at(&cuts, z) {
            let d = left
                .iter()
                .map(|q| (q.x - p.x).hypot(q.y - p.y))
                .fold(f64::INFINITY, f64::min);
            worst = worst.max(d);
            assert!(
                d <= bound,
                "a cut at ({:.2}, {:.2}, {z}) lies {d:.2} mm from the material A left; \
                 the bound is {bound} mm",
                p.x,
                p.y
            );
        }
    }
    eprintln!("shifted: worst distance above A's floor {worst:.3} mm (bound {bound})");

    // The strip x ∈ [HALF, HALF + SHIFT] is outside A. The tool centre fits
    // there from x = HALF + R on, so a cut that far in is a strip cut.
    for z in ladder(B_DEPTH) {
        assert!(
            cuts_at(&cuts, z).iter().any(|p| p.x >= HALF + TOOL_R),
            "the strip outside A must be cut at level {z}"
        );
    }
    let finding = arm.finding.expect("a rest pocket records its level clip");
    assert_eq!(
        finding.skipped_levels, 0,
        "the strip holds material at every level"
    );
    assert!(
        finding.clipped_levels >= ladder(A_DEPTH).len(),
        "every level above A's floor must be clipped; got {finding:?}"
    );
}

/// 4. Rest B to A's own depth has nothing to cut: an empty result, which
///    the empty-generation gate accepts for a rest operation.
#[test]
fn a_rest_pocket_with_nothing_left_is_an_empty_result() {
    let arm = run(0.0, A_DEPTH, StockSource::FromRemainingStock);
    let cuts = cutting_moves(&arm.moves);
    assert!(
        cuts.is_empty(),
        "nothing stands above A's floor; got {} cutting moves",
        cuts.len()
    );
    let finding = arm.finding.expect("a rest pocket records its level clip");
    assert_eq!(finding.skipped_levels, finding.planned_levels);
}

/// 5. No undercut: rest B leaves the same stock as fresh B.
#[test]
fn a_rest_pocket_leaves_the_same_stock_as_a_fresh_pocket() {
    for shift in [0.0, SHIFT] {
        let fresh = run(shift, B_DEPTH, StockSource::Fresh);
        let rest = run(shift, B_DEPTH, StockSource::FromRemainingStock);
        let (a, b) = (&fresh.final_stock.z_grid, &rest.final_stock.z_grid);
        assert_eq!((a.rows, a.cols), (b.rows, b.cols), "one grid shape");
        // Judge only the cells B's tool covers completely: centre inside B's
        // footprint (the square with corners rounded to R) by one cell. The
        // simulator blends a cell a tool covers in part, and it blends it
        // again on each pass over the same path. A fresh B re-runs A's
        // paths, so the part-covered cells at the footprint edge read lower
        // after it with no material removed. That is a simulator artefact,
        // not an undercut.
        let inner = HALF - TOOL_R;
        let mut worst: f32 = 0.0;
        let mut worst_at = (0, 0);
        let mut judged = 0_usize;
        let mut worst_outside: f32 = 0.0;
        for r in 0..a.rows {
            for c in 0..a.cols {
                let (x, y) = a.cell_to_world(r, c);
                let dx = ((x - shift).abs() - inner).max(0.0);
                let dy = (y.abs() - inner).max(0.0);
                let covered = dx.hypot(dy) <= TOOL_R - CELL_MM;
                let ta = a.top_z_at(r, c).unwrap_or(f32::NEG_INFINITY);
                let tb = b.top_z_at(r, c).unwrap_or(f32::NEG_INFINITY);
                let d = (ta - tb).abs();
                if !covered {
                    worst_outside = worst_outside.max(d);
                    continue;
                }
                judged += 1;
                if d > worst {
                    worst = d;
                    worst_at = (r, c);
                }
            }
        }
        eprintln!(
            "shift {shift}: worst top difference {worst:.4} mm at {worst_at:?} over {judged} \
             covered cells; {worst_outside:.4} mm outside them (fresh moves {}, rest moves {})",
            fresh.moves.len(),
            rest.moves.len()
        );
        assert!(
            judged > 0,
            "shift {shift}: the judged set must not be empty"
        );
        assert!(
            f64::from(worst) <= FRESH_MATERIAL_THRESHOLD_MM,
            "shift {shift}: the rest pocket left {worst} mm more or less than the fresh \
             pocket at cell {worst_at:?}"
        );
        assert!(
            rest.moves.len() < fresh.moves.len(),
            "shift {shift}: the rest pocket must skip work the fresh pocket does"
        );
    }
}

/// 2z, 5z. The Zigzag pattern of Pocket takes the same clip: on A's outline
///    it cuts only below A's floor, and it leaves the stock a fresh Zigzag
///    pocket leaves on both outlines.
#[test]
fn a_rest_zigzag_pocket_cuts_only_below_the_prior_floor_and_leaves_the_same_stock() {
    let arm = run_with(
        0.0,
        B_DEPTH,
        StockSource::FromRemainingStock,
        PocketPattern::Zigzag,
    );
    let cuts = cutting_moves(&arm.moves);
    let floor = -A_DEPTH - FRESH_MATERIAL_THRESHOLD_MM;
    assert!(!cuts.is_empty(), "the rest zigzag must cut below A's floor");
    assert!(
        cuts.iter().all(|m| m.target.z < floor),
        "a rest zigzag cut lies at or above A's floor"
    );
    for shift in [0.0, SHIFT] {
        let fresh = run_with(shift, B_DEPTH, StockSource::Fresh, PocketPattern::Zigzag);
        let rest = run_with(
            shift,
            B_DEPTH,
            StockSource::FromRemainingStock,
            PocketPattern::Zigzag,
        );
        let worst = worst_covered_difference(&fresh.final_stock, &rest.final_stock, shift);
        eprintln!("zigzag shift {shift}: worst top difference {worst:.4} mm");
        assert!(
            f64::from(worst) <= FRESH_MATERIAL_THRESHOLD_MM,
            "zigzag shift {shift}: the rest pocket left {worst} mm more or less than the \
             fresh pocket"
        );
        assert!(rest.moves.len() < fresh.moves.len());
    }
}

/// The largest top difference over the cells B's tool covers completely
/// (see claim 5 for why the part-covered cells are not judged).
fn worst_covered_difference(fresh: &TriDexelStock, rest: &TriDexelStock, shift: f64) -> f32 {
    let (a, b) = (&fresh.z_grid, &rest.z_grid);
    assert_eq!((a.rows, a.cols), (b.rows, b.cols), "one grid shape");
    let inner = HALF - TOOL_R;
    let mut worst: f32 = 0.0;
    let mut judged = 0_usize;
    for r in 0..a.rows {
        for c in 0..a.cols {
            let (x, y) = a.cell_to_world(r, c);
            let dx = ((x - shift).abs() - inner).max(0.0);
            let dy = (y.abs() - inner).max(0.0);
            if dx.hypot(dy) > TOOL_R - CELL_MM {
                continue;
            }
            judged += 1;
            let ta = a.top_z_at(r, c).unwrap_or(f32::NEG_INFINITY);
            let tb = b.top_z_at(r, c).unwrap_or(f32::NEG_INFINITY);
            worst = worst.max((ta - tb).abs());
        }
    }
    assert!(judged > 0, "the judged set must not be empty");
    worst
}

/// 6. The chain wedge (fixed). A rest pocket that the clip makes empty must
///    not block the rest pocket after it.
///
/// Before the fix, `PhantomPriorStockScan` treated "generated empty" like
/// "not generated": the empty rest op took the group's one phantom
/// snapshot, and the rest op after it was refused for want of a snapshot,
/// at every simulation. Now both simulation builders carve an empty result
/// with no change, so it keeps its own snapshot and passes the stock on.
#[test]
fn an_empty_rest_pocket_does_not_block_the_rest_pocket_after_it() {
    chain_cuts_only_below_the_floor(&[(0, A_DEPTH), (0, B_DEPTH)], false);
}

/// 6b. Two empty rest pockets in a row before the rest pocket C.
#[test]
fn two_empty_rest_pockets_do_not_block_the_rest_pocket_after_them() {
    chain_cuts_only_below_the_floor(&[(0, A_DEPTH), (0, A_DEPTH), (0, B_DEPTH)], false);
}

/// 6c. The chain crosses a setup: the empty rest pocket is last in setup 1
///     and the rest pocket C is first in setup 2 (S0 carries the stock).
#[test]
fn an_empty_rest_pocket_last_in_a_setup_does_not_block_the_next_setup() {
    chain_cuts_only_below_the_floor(&[(0, A_DEPTH), (1, B_DEPTH)], true);
}

/// Fresh A in setup 0, then the rest pockets `rests` as `(setup, depth)`.
/// Every rest pocket but the last is at A's depth and must be empty; the
/// last is deeper and must cut, and only below A's floor. Each rest pocket
/// is generated after a simulation of the ones before it, as the plan does.
fn chain_cuts_only_below_the_floor(rests: &[(usize, f64)], second_setup: bool) {
    let mut builder = ProjectSessionBuilder::new().stock(stock());
    let tool_idx = builder.add_tool(endmill_tool_config(TOOL_D));
    let tool_id = builder.tools()[tool_idx].id.0;
    let model = builder.add_model(polygon_model(vec![square_polygon(HALF)], "a"));
    if second_setup {
        let _ = builder.add_setup("Setup 2".to_owned(), FaceUp::Top);
    }
    let _ = builder
        .add_toolpath(0, toolpath_config("A", pocket(A_DEPTH), tool_id, model))
        .expect("add A");
    let mut indices = Vec::new();
    for (k, &(setup, depth)) in rests.iter().enumerate() {
        let mut rest = toolpath_config(&format!("Rest {k}"), pocket(depth), tool_id, model);
        rest.stock_source = StockSource::FromRemainingStock;
        indices.push(
            builder
                .add_toolpath(setup, rest)
                .expect("add a rest pocket"),
        );
    }
    let mut session = builder.build();
    let _ = session
        .apply(Command::SetSimulationResolution(
            SetSimulationResolutionArgs {
                resolution: SimulationResolution::Fixed(CELL_MM),
            },
        ))
        .expect("a positive cell");
    let cancel = AtomicBool::new(false);
    generate(&mut session, 0);
    let last = indices.len() - 1;
    for (k, &index) in indices.iter().enumerate() {
        simulate(&mut session);
        let outcome = session.generate_toolpath(index, &cancel);
        assert!(
            outcome.is_ok(),
            "rest pocket {k} must generate after the empty rest pockets before it; got: {:?}",
            outcome.err().map(|e| e.to_string())
        );
        let result = session.get_result(index).expect("a rest result");
        let cuts = cutting_moves(&result.toolpath().moves);
        if k < last {
            assert!(
                cuts.is_empty(),
                "precondition: rest pocket {k} has nothing to cut"
            );
        } else {
            assert!(
                !cuts.is_empty(),
                "the last rest pocket has material below A's floor"
            );
            assert!(
                cuts.iter()
                    .all(|m| m.target.z < -A_DEPTH - FRESH_MATERIAL_THRESHOLD_MM),
                "the last rest pocket cuts only below A's floor"
            );
        }
    }
    // The chain is stable: a second round regenerates every rest pocket.
    for &index in &indices {
        simulate(&mut session);
        assert!(
            session.generate_toolpath(index, &cancel).is_ok(),
            "a second round must regenerate rest pocket index {index}"
        );
    }
}

// ── W3.4: the rest rule of each other op ────────────────────────────────

/// The cutting moves of `moves` at level `z`, as XY points with the bits of
/// each coordinate, so two arms compare exactly.
fn cut_bits_at(moves: &[Move], z: f64) -> Vec<(u64, u64)> {
    cutting_moves(moves)
        .iter()
        .filter(|m| (m.target.z - z).abs() < 1e-6)
        .map(|m| (m.target.x.to_bits(), m.target.y.to_bits()))
        .collect()
}

fn zigzag_op(depth: f64) -> OperationConfig {
    OperationConfig::Zigzag(ZigzagConfig {
        stepover: 2.0,
        depth,
        depth_per_pass: DPP,
        ..ZigzagConfig::default()
    })
}

/// 7. Zigzag (`LevelClip`): on A's outline it cuts only below A's floor,
///    and on both outlines it leaves the stock a fresh Zigzag leaves.
#[test]
fn a_rest_zigzag_cuts_only_below_the_prior_floor() {
    let fresh = run_ops(
        pocket(A_DEPTH),
        zigzag_op(B_DEPTH),
        0.0,
        StockSource::Fresh,
        &[],
    );
    let floor = -A_DEPTH - FRESH_MATERIAL_THRESHOLD_MM;
    assert!(
        cutting_moves(&fresh.moves)
            .iter()
            .any(|m| m.target.z > floor),
        "non-vacuity: a fresh Zigzag cuts above A's floor"
    );
    let rest = run_ops(
        pocket(A_DEPTH),
        zigzag_op(B_DEPTH),
        0.0,
        StockSource::FromRemainingStock,
        &[],
    );
    let cuts = cutting_moves(&rest.moves);
    eprintln!(
        "zigzag op: {} rest cutting moves, finding {:?}",
        cuts.len(),
        rest.finding
    );
    assert!(!cuts.is_empty(), "the rest Zigzag must cut below A's floor");
    assert!(
        cuts.iter().all(|m| m.target.z < floor),
        "a rest Zigzag cut lies at or above A's floor"
    );
    let finding = rest.finding.expect("a rest Zigzag records its level clip");
    assert_eq!(finding.skipped_levels, ladder(A_DEPTH).len());
    for shift in [0.0, SHIFT] {
        let fresh = run_ops(
            pocket(A_DEPTH),
            zigzag_op(B_DEPTH),
            shift,
            StockSource::Fresh,
            &[],
        );
        let rest = run_ops(
            pocket(A_DEPTH),
            zigzag_op(B_DEPTH),
            shift,
            StockSource::FromRemainingStock,
            &[],
        );
        let worst = worst_covered_difference(&fresh.final_stock, &rest.final_stock, shift);
        eprintln!("zigzag op shift {shift}: worst top difference {worst:.4} mm");
        assert!(
            f64::from(worst) <= FRESH_MATERIAL_THRESHOLD_MM,
            "zigzag op shift {shift}: the rest Zigzag left {worst} mm more or less than the \
             fresh Zigzag"
        );
        assert!(rest.moves.len() < fresh.moves.len());
    }
}

fn inside_profile(depth: f64) -> OperationConfig {
    OperationConfig::Profile(ProfileConfig {
        side: ProfileSide::Inside,
        depth,
        depth_per_pass: DPP,
        ..ProfileConfig::default()
    })
}

/// 8. Profile (`LevelSkip`): on A's outline the rest Profile skips every
///    level A cut and cuts the fresh contour at each level below. On the
///    shifted outline the strip holds material at every level, so the rest
///    Profile cuts the fresh contour at every level: it never clips the
///    outline into a new wall.
#[test]
fn a_rest_profile_skips_the_cleared_levels_and_never_cuts_a_new_wall() {
    for (shift, skipped) in [(0.0, ladder(A_DEPTH).len()), (SHIFT, 0)] {
        let fresh = run_ops(
            pocket(A_DEPTH),
            inside_profile(B_DEPTH),
            shift,
            StockSource::Fresh,
            &[],
        );
        let rest = run_ops(
            pocket(A_DEPTH),
            inside_profile(B_DEPTH),
            shift,
            StockSource::FromRemainingStock,
            &[],
        );
        let finding = rest.finding.expect("a rest Profile records its level skip");
        eprintln!("profile shift {shift}: finding {finding:?}");
        assert_eq!(
            finding.skipped_levels, skipped,
            "shift {shift}: the skipped levels"
        );
        assert_eq!(finding.clipped_levels, 0, "a Profile never clips a level");
        let mut cut_levels = 0;
        for z in ladder(B_DEPTH) {
            let f = cut_bits_at(&fresh.moves, z);
            let r = cut_bits_at(&rest.moves, z);
            assert!(!f.is_empty(), "non-vacuity: a fresh Profile cuts level {z}");
            if r.is_empty() {
                assert!(
                    z > -A_DEPTH - FRESH_MATERIAL_THRESHOLD_MM,
                    "shift {shift}: the rest Profile skipped level {z}, below A's floor"
                );
                continue;
            }
            cut_levels += 1;
            assert_eq!(
                r, f,
                "shift {shift}: at level {z} the rest Profile must cut the fresh contour, \
                 not a clipped one"
            );
        }
        assert_eq!(cut_levels + skipped, ladder(B_DEPTH).len());
    }
}

/// Depth (mm) of the prior Face, and of the rest Face after it.
const FACE_A_DEPTH: f64 = 3.0;
const FACE_B_DEPTH: f64 = 5.0;

fn face(depth: f64) -> OperationConfig {
    OperationConfig::Face(FaceConfig {
        depth,
        depth_per_pass: DPP,
        ..FaceConfig::default()
    })
}

/// 9. Face (`LevelClip` on the stock rectangle): after a Face to 3 mm, a
///    rest Face to 5 mm skips the levels at and above the remaining top and
///    cuts each level below it exactly as a fresh Face does.
#[test]
fn a_rest_face_skips_the_levels_above_the_remaining_top() {
    let fresh = run_ops(
        face(FACE_A_DEPTH),
        face(FACE_B_DEPTH),
        0.0,
        StockSource::Fresh,
        &[],
    );
    let rest = run_ops(
        face(FACE_A_DEPTH),
        face(FACE_B_DEPTH),
        0.0,
        StockSource::FromRemainingStock,
        &[],
    );
    let finding = rest.finding.expect("a rest Face records its level clip");
    eprintln!(
        "face: fresh {} moves, rest {} moves, finding {finding:?}",
        fresh.moves.len(),
        rest.moves.len()
    );
    assert_eq!(finding.planned_levels, ladder(FACE_B_DEPTH).len());
    assert_eq!(
        finding.skipped_levels,
        ladder(FACE_A_DEPTH).len(),
        "every level the prior Face cut must be skipped"
    );
    assert_eq!(finding.fallback, None);
    for z in ladder(FACE_B_DEPTH) {
        let f = cut_bits_at(&fresh.moves, z);
        let r = cut_bits_at(&rest.moves, z);
        assert!(!f.is_empty(), "non-vacuity: a fresh Face cuts level {z}");
        if z > -FACE_A_DEPTH - FRESH_MATERIAL_THRESHOLD_MM {
            assert!(
                r.is_empty(),
                "the rest Face cut level {z}, above the remaining top"
            );
        } else {
            assert_eq!(
                r, f,
                "below the remaining top the rest Face cuts as a fresh Face"
            );
        }
    }
}

/// The drill targets of claim 10: one in A, one in the strip outside A,
/// more than the tool radius plus a cell from A's wall.
const HOLE_IN_A: [f64; 2] = [0.0, 0.0];
const HOLE_OUTSIDE_A: [f64; 2] = [HALF + SHIFT / 2.0 + TOOL_R / 2.0, 0.0];

fn drill(depth: f64) -> OperationConfig {
    OperationConfig::Drill(DrillConfig {
        depth,
        ..DrillConfig::default()
    })
}

/// The moves of one hole: every move at the hole's XY.
fn hole_moves(moves: &[Move], xy: [f64; 2]) -> Vec<&Move> {
    moves
        .iter()
        .filter(|m| (m.target.x - xy[0]).abs() < 1e-9 && (m.target.y - xy[1]).abs() < 1e-9)
        .collect()
}

/// The R-plane of one hole: the Z of the second rapid at the hole (the
/// first is the rapid at the safe Z).
fn r_plane(moves: &[&Move]) -> f64 {
    moves
        .iter()
        .filter(|m| m.move_type == MoveType::Rapid)
        .nth(1)
        .expect("a hole rapids to its R-plane")
        .target
        .z
}

/// 10. Drill (`StockColumn`, D8): the hole in A starts at A's floor. Its
///     R-plane, where its pecks start, sits the clearance a fresh hole has
///     above the stock top over A's floor. The hole outside A keeps its
///     fresh cycle. The drill-op view names the same tops.
#[test]
fn a_rest_drill_starts_each_hole_at_the_remaining_stock_top() {
    let targets = [HOLE_IN_A, HOLE_OUTSIDE_A];
    let fresh = run_ops(
        pocket(A_DEPTH),
        drill(B_DEPTH),
        0.0,
        StockSource::Fresh,
        &targets,
    );
    let rest = run_ops(
        pocket(A_DEPTH),
        drill(B_DEPTH),
        0.0,
        StockSource::FromRemainingStock,
        &targets,
    );
    // The stock top is world Z0 (the board hangs below it).
    let fresh_r = r_plane(&hole_moves(&fresh.moves, HOLE_IN_A));
    let clearance = fresh_r;
    let rest_in = hole_moves(&rest.moves, HOLE_IN_A);
    let rest_r = r_plane(&rest_in);
    eprintln!("drill: fresh R {fresh_r:.3}, rest R in A {rest_r:.3}");
    assert!(
        (rest_r - (-A_DEPTH + clearance)).abs() < 1e-6,
        "the hole in A must keep its clearance above A's floor: R {rest_r}, floor {}",
        -A_DEPTH
    );
    // The pecks start at the R-plane, so no drilling feed ends in the air
    // above A's floor. (The dressups also turn a feed through air into
    // rapids, so this is read together with the R-plane above.)
    for m in rest_in.iter().filter(|m| m.intent == MoveIntent::Drilling) {
        assert!(
            m.target.z < -A_DEPTH,
            "a drilling feed of the hole in A ends at {}, above A's floor",
            m.target.z
        );
    }
    let drilled = |moves: &[&Move]| {
        moves
            .iter()
            .filter(|m| m.intent == MoveIntent::Drilling)
            .count()
    };
    assert!(
        drilled(&rest_in) < drilled(&hole_moves(&fresh.moves, HOLE_IN_A)),
        "the hole in A pecks less air than the fresh hole"
    );
    // The bottom is the design bottom in both arms.
    let bottom = |moves: &[&Move]| {
        moves
            .iter()
            .map(|m| m.target.z)
            .fold(f64::INFINITY, f64::min)
    };
    assert!((bottom(&rest_in) - (-B_DEPTH)).abs() < 1e-9);
    // The hole outside A keeps its fresh R-plane and bottom. (The rest arm
    // also runs the dressup that turns a feed through air into rapids, so
    // the two cycles are not compared move for move.)
    let outside = hole_moves(&rest.moves, HOLE_OUTSIDE_A);
    assert_eq!(
        r_plane(&outside).to_bits(),
        r_plane(&hole_moves(&fresh.moves, HOLE_OUTSIDE_A)).to_bits(),
        "the hole outside A keeps its fresh R-plane"
    );
    assert!((bottom(&outside) - (-B_DEPTH)).abs() < 1e-9);
    // The drill-op view names the same columns.
    let top_of = |holes: &[DrillHole], xy: [f64; 2]| {
        holes
            .iter()
            .find(|h| h.xy == xy)
            .map(|h| (h.top_z, h.retract_z))
            .expect("the view holds the hole")
    };
    let (top, r) = top_of(&rest.drill_holes, HOLE_IN_A);
    assert!((top - (-A_DEPTH)).abs() < 1e-3, "view top {top}");
    assert!(
        (r - rest_r).abs() < 1e-9,
        "view R-plane {r} vs emitted {rest_r}"
    );
    assert_eq!(top_of(&rest.drill_holes, HOLE_OUTSIDE_A).0, 0.0);
}

/// 15. B11, Drill: the air-cut filter drops a peck that runs only through
///     air above the remaining stock. Every move of the rest Drill keeps a
///     known intent, and the toolpath still reads back as native peck
///     cycles (G83) on a post that runs them.
#[test]
fn a_rest_drill_reads_back_as_native_canned_cycles() {
    use rs_cam_core::gcode::post::grblhal;
    use rs_cam_core::gcode::{
        CoolantMode, GcodePhase, PhaseDrill, ToolLoadExportPolicy, WizardOverlay,
        export_gcode_phases_with_overlay_checked,
    };
    let op = drill(B_DEPTH);
    let OperationConfig::Drill(cfg) = &op else {
        unreachable!("drill() builds a Drill")
    };
    let cycle = cfg.cycle.to_core(cfg);
    let fresh = run_ops(
        pocket(A_DEPTH),
        op.clone(),
        0.0,
        StockSource::Fresh,
        &[HOLE_IN_A, HOLE_OUTSIDE_A],
    );
    let rest = run_ops(
        pocket(A_DEPTH),
        op,
        0.0,
        StockSource::FromRemainingStock,
        &[HOLE_IN_A, HOLE_OUTSIDE_A],
    );
    // Non-vacuity: the rest arm is shorter, so the filter dropped a peck.
    assert!(rest.moves.len() < fresh.moves.len());
    let unknown = rest
        .moves
        .iter()
        .filter(|m| m.intent == MoveIntent::Unknown)
        .count();
    assert_eq!(unknown, 0, "the rest Drill emits moves with intent Unknown");
    let toolpath = rs_cam_core::toolpath::Toolpath { moves: rest.moves };
    let phase = GcodePhase {
        toolpath: &toolpath,
        spindle_rpm: 12_000,
        label: "rest drill",
        pre_gcode: None,
        post_gcode: None,
        tool: None,
        coolant: CoolantMode::Off,
        controller_compensation: None,
        drill: Some(PhaseDrill { cycle }),
        feeds: None,
    };
    let overlay = WizardOverlay {
        native_drill_cycles: true,
        ..WizardOverlay::default()
    };
    let gcode = export_gcode_phases_with_overlay_checked(
        &[phase],
        grblhal(),
        &rs_cam_core::tool_load::ToolLoadReport {
            per_toolpath: vec![],
        },
        ToolLoadExportPolicy::default(),
        &overlay,
    )
    .expect("the rest drill exports");
    assert!(
        !gcode.contains("native drill cycles not used"),
        "the rest drill must read back as plain drill holes:\n{gcode}"
    );
    assert!(
        gcode.lines().filter(|l| l.contains("G83")).count() >= 1,
        "the rest drill must write a native peck cycle:\n{gcode}"
    );
}

// ── W3.5: Chamfer ───────────────────────────────────────────────────────

/// Width (mm) and tip offset (mm) of the chamfer of claim 11. With the 90°
/// V-bit the tip runs at `-(width + tip) / tan 45° = -2.1` mm.
const CHAMFER_WIDTH: f64 = 2.0;
const CHAMFER_TIP: f64 = 0.1;

/// The tip Z (mm) of the chamfer of claim 11: `-(w + t) / tan(a)`.
fn chamfer_level() -> f64 {
    -(CHAMFER_WIDTH + CHAMFER_TIP) / (VBIT_ANGLE_DEG / 2.0).to_radians().tan()
}

/// Included angle (degrees) of the V-bit of claim 11.
const VBIT_ANGLE_DEG: f64 = 90.0;

/// The depth (mm) of a prior Face that leaves its top above the chamfer
/// level. [`FACE_A_DEPTH`] leaves its top below it.
const FACE_ABOVE_CHAMFER_DEPTH: f64 = 1.0;

fn vbit() -> ToolConfig {
    ToolConfig {
        diameter: 12.7,
        included_angle: VBIT_ANGLE_DEG,
        ..ToolConfig::new_default(ToolId(0), ToolType::VBit)
    }
}

fn chamfer() -> OperationConfig {
    OperationConfig::Chamfer(ChamferConfig {
        side: ProfileSide::Outside,
        chamfer_width: CHAMFER_WIDTH,
        tip_offset: CHAMFER_TIP,
        ..ChamferConfig::default()
    })
}

/// Face to `face_depth`, then a Chamfer on A's outline from `source`.
fn chamfer_after_face(face_depth: f64, source: StockSource) -> Arm {
    run_ops_with_tool(face(face_depth), chamfer(), Some(vbit()), 0.0, source, &[])
}

/// 11. Chamfer (`LevelSkip`, W3.5). A Face to 3 mm removes every edge the
///     chamfer at -2.1 mm would bevel: the rest Chamfer skips its one level
///     and gives an empty result, not a refusal. A Face to 1 mm leaves the
///     edge standing above the level: the rest Chamfer cuts the rings a
///     fresh Chamfer cuts, bit for bit.
#[test]
fn a_rest_chamfer_skips_an_edge_a_prior_op_removed() {
    let z = chamfer_level();
    assert!(
        -FACE_A_DEPTH < z - FRESH_MATERIAL_THRESHOLD_MM
            && z + FRESH_MATERIAL_THRESHOLD_MM < -FACE_ABOVE_CHAMFER_DEPTH,
        "the two Faces must lie on the two sides of the chamfer level {z}"
    );
    for (face_depth, skipped) in [(FACE_A_DEPTH, 1), (FACE_ABOVE_CHAMFER_DEPTH, 0)] {
        let fresh = chamfer_after_face(face_depth, StockSource::Fresh);
        let rest = chamfer_after_face(face_depth, StockSource::FromRemainingStock);
        let f = cut_bits_at(&fresh.moves, z);
        let r = cut_bits_at(&rest.moves, z);
        eprintln!(
            "chamfer after a {face_depth} mm Face: fresh {} cuts, rest {} cuts, \
             finding {:?}",
            f.len(),
            r.len(),
            rest.finding
        );
        assert!(!f.is_empty(), "non-vacuity: a fresh Chamfer cuts level {z}");
        if skipped == 1 {
            assert!(
                rest.moves.is_empty(),
                "face {face_depth}: the rest Chamfer moved over a removed edge"
            );
        } else {
            assert_eq!(
                r, f,
                "face {face_depth}: the rest Chamfer must cut the fresh rings"
            );
        }
        let finding = rest.finding.expect("a rest Chamfer records its level skip");
        assert_eq!(finding.planned_levels, 1, "a Chamfer has one level");
        assert_eq!(finding.skipped_levels, skipped, "face {face_depth}: skips");
        assert_eq!(finding.clipped_levels, 0, "a Chamfer never clips a level");
        assert_eq!(finding.fallback, None);
    }
}

/// A 2D Adaptive to `depth`, one level per millimetre.
fn adaptive(depth: f64) -> OperationConfig {
    OperationConfig::Adaptive(AdaptiveConfig {
        stepover: 2.0,
        depth,
        depth_per_pass: DPP,
        ..AdaptiveConfig::default()
    })
}

/// The distance from `p` to the nearest point of the material A left
/// inside B shifted `shift` (B minus A's footprint, sampled on a quarter
/// cell). `f64::INFINITY` when A left none.
fn distance_to_material_left(p: P2, shift: f64) -> f64 {
    let step = CELL_MM / 4.0;
    let inner = HALF - TOOL_R;
    let n = ((2.0 * HALF) / step).round() as usize;
    let mut best = f64::INFINITY;
    for i in 0..=n {
        for j in 0..=n {
            let q = P2::new(-HALF + shift + i as f64 * step, -HALF + j as f64 * step);
            let dx = (q.x.abs() - inner).max(0.0);
            let dy = (q.y.abs() - inner).max(0.0);
            if dx.hypot(dy) > TOOL_R {
                best = best.min((q.x - p.x).hypot(q.y - p.y));
            }
        }
    }
    best
}

/// The highest top (mm) over the cells whose centre lies at least one cell
/// inside the footprint of B's tool, and the count of those cells. The
/// cells along the wall are left out: the simulator blends a cell the tool
/// covers in part, and a fresh Adaptive leaves the same blend there (0.31
/// mm at a cell from the wall on this fixture, cell for cell the same).
fn highest_covered_top(stock: &TriDexelStock, shift: f64) -> (f32, usize) {
    let g = &stock.z_grid;
    let inner = HALF - TOOL_R;
    let mut top = f32::NEG_INFINITY;
    let mut judged = 0;

    for r in 0..g.rows {
        for c in 0..g.cols {
            let (x, y) = g.cell_to_world(r, c);
            let dx = ((x - shift).abs() - inner).max(0.0);
            let dy = (y.abs() - inner).max(0.0);
            if dx.hypot(dy) > TOOL_R - 2.0 * CELL_MM {
                continue;
            }
            judged += 1;
            top = top.max(g.top_z_at(r, c).unwrap_or(f32::NEG_INFINITY));
        }
    }
    (top, judged)
}

/// The Z tolerance (mm) of a level: a planner Z carries float noise.
const LEVEL_Z_EPS_MM: f64 = 1e-6;

/// 12. 2D Adaptive (`LevelMask`, item 1 of the W3.4 follow-up): after
///     Pocket A, a rest Adaptive on A's outline skips every level A cleared
///     and cuts only below A's floor. Shifted, above A's floor it cuts only
///     near the material A left. After the rest Adaptive no material stands
///     above its floor (plus the simulator's threshold) in the cells its
///     tool covers completely.
#[test]
fn a_rest_adaptive_skips_the_cleared_levels_and_cuts_only_where_material_stands() {
    let floor = -A_DEPTH - FRESH_MATERIAL_THRESHOLD_MM;
    for shift in [0.0, SHIFT] {
        let rest = run_ops(
            pocket(A_DEPTH),
            adaptive(B_DEPTH),
            shift,
            StockSource::FromRemainingStock,
            &[],
        );
        let cuts = cutting_moves(&rest.moves);
        let finding = rest
            .finding
            .expect("a rest Adaptive records its level read");
        eprintln!(
            "adaptive shift {shift}: {} moves, {} cutting moves, finding {finding:?}",
            rest.moves.len(),
            cuts.len()
        );
        assert_eq!(finding.planned_levels, ladder(B_DEPTH).len());
        assert_eq!(finding.fallback, None);
        assert!(
            !cuts.is_empty(),
            "shift {shift}: the rest Adaptive must cut"
        );
        if shift == 0.0 {
            assert_eq!(
                finding.skipped_levels,
                ladder(A_DEPTH).len(),
                "every level A already cut must be skipped"
            );
            for m in &cuts {
                assert!(
                    m.target.z < floor,
                    "a cut at z = {} lies at or above A's floor {floor}",
                    m.target.z
                );
            }
        } else {
            assert_eq!(
                finding.skipped_levels, 0,
                "the strip holds material at every level"
            );
            // Every cut and every entry move above A's floor lies near the
            // material A left (B10: the entry starter stands on material,
            // not at the medial point of the piece, which lies in A's
            // cleared area here). The level at A's own floor is not
            // judged: there the simulator's blend of the cells along A's
            // wall stands above the threshold, so the planner reads (and
            // cleans) a sliver along that wall.
            let bound = TOOL_R + 2.0 * CELL_MM;
            let mut worst: f64 = 0.0;
            let mut judge = |kind: &str, p: P2, z: f64| {
                let d = distance_to_material_left(p, shift);
                worst = worst.max(d);
                assert!(
                    d <= bound,
                    "a {kind} at ({:.2}, {:.2}, {z:.3}) lies {d:.2} mm from the material A \
                     left; the bound is {bound} mm",
                    p.x,
                    p.y
                );
            };
            for z in ladder(A_DEPTH - DPP) {
                let level = cuts_at(&cuts, z);
                assert!(!level.is_empty(), "the strip holds material at level {z}");
                for p in level {
                    judge("cut", p, z);
                }
            }
            // The fed entry moves (plunge, helix, ramp) inside the stock
            // and above the level at A's floor (not judged, as above).
            let mut entries = 0;
            for m in &rest.moves {
                if m.move_type != MoveType::Rapid
                    && matches!(
                        m.intent,
                        MoveIntent::EntryPlunge | MoveIntent::EntryHelix | MoveIntent::EntryRamp
                    )
                    && m.target.z <= 0.0
                    && m.target.z > -A_DEPTH + LEVEL_Z_EPS_MM
                {
                    entries += 1;
                    judge("entry move", P2::new(m.target.x, m.target.y), m.target.z);
                }
            }
            eprintln!("adaptive shifted: {entries} entry moves judged above A's floor");
            assert!(
                entries > 0,
                "non-vacuity: the rest Adaptive enters above A's floor"
            );
            eprintln!("adaptive shifted: worst distance above A's floor {worst:.3} mm");
        }
        let fresh = run_ops(
            pocket(A_DEPTH),
            adaptive(B_DEPTH),
            shift,
            StockSource::Fresh,
            &[],
        );
        let worst = worst_covered_difference(&fresh.final_stock, &rest.final_stock, shift);
        eprintln!("adaptive shift {shift}: worst top difference to a fresh Adaptive {worst:.4} mm");
        assert!(
            f64::from(worst) <= FRESH_MATERIAL_THRESHOLD_MM,
            "shift {shift}: the rest Adaptive left {worst} mm more or less than a fresh one"
        );
        assert!(
            rest.moves.len() < fresh.moves.len(),
            "shift {shift}: the rest Adaptive must skip work the fresh one does"
        );
        let (top, judged) = highest_covered_top(&rest.final_stock, shift);
        eprintln!("adaptive shift {shift}: highest covered top {top:.4} over {judged} cells");
        assert!(
            judged > 0,
            "shift {shift}: the judged set must not be empty"
        );
        assert!(
            f64::from(top) <= -B_DEPTH + FRESH_MATERIAL_THRESHOLD_MM,
            "shift {shift}: material stands at {top} mm, above the floor {}",
            -B_DEPTH
        );
    }
}

/// The facing offset (mm) of the B8 cases: more than `2R + cell`, so a rest
/// clip region ends inside the facing rectangle.
const FACE_B8_OFFSET: f64 = 10.0;

/// A Face to `depth` with the B8 offset and a stepover that is no divisor
/// of the gap between the clip region and the rectangle.
fn face_b8(depth: f64) -> OperationConfig {
    OperationConfig::Face(FaceConfig {
        depth,
        depth_per_pass: DPP,
        stepover: 2.3,
        stock_offset: FACE_B8_OFFSET,
        ..FaceConfig::default()
    })
}

/// The rows cut at level `z`: each cutting move as a segment from the
/// target of the move before it.
fn cut_rows(moves: &[Move], z: f64) -> Vec<[P2; 2]> {
    moves
        .windows(2)
        .filter(|w| {
            let m = &w[1];
            m.move_type != MoveType::Rapid
                && matches!(m.intent, MoveIntent::ClearingCut | MoveIntent::FinishingCut)
                && (m.target.z - z).abs() < 1e-6
        })
        .map(|w| {
            [
                P2::new(w[0].target.x, w[0].target.y),
                P2::new(w[1].target.x, w[1].target.y),
            ]
        })
        .collect()
}

/// The rows a Face cuts at level `z` ([`cut_rows`]). Only rows in the
/// reach of the stock count (`|y|` at most the stock half-width plus R):
/// the air-cut filter drops a rest row that runs only in air, and a fresh
/// Face does not know the stock.
fn face_rows(moves: &[Move], z: f64) -> Vec<[P2; 2]> {
    let reach = HALF + MARGIN + TOOL_R;
    cut_rows(moves, z)
        .into_iter()
        .filter(|[_, b]| b.y.abs() <= reach)
        .collect()
}

/// 13. B8, whole top: after a Face to 1 mm the material covers the whole
///     top at each lower level, so a rest Face cuts each of those levels
///     on the rows of a fresh Face, bit for bit.
#[test]
fn a_rest_face_on_a_whole_top_cuts_as_a_fresh_face() {
    let fresh = run_ops(face_b8(1.0), face_b8(3.0), 0.0, StockSource::Fresh, &[]);
    let rest = run_ops(
        face_b8(1.0),
        face_b8(3.0),
        0.0,
        StockSource::FromRemainingStock,
        &[],
    );
    let finding = rest.finding.expect("a rest Face records its level clip");
    assert_eq!(finding.skipped_levels, 1, "the level the first Face cut");
    assert_eq!(finding.clipped_levels, 0, "the whole top stands below it");
    let bits = |rows: Vec<[P2; 2]>| -> Vec<[u64; 4]> {
        rows.iter()
            .map(|[a, b]| [a.x.to_bits(), a.y.to_bits(), b.x.to_bits(), b.y.to_bits()])
            .collect()
    };
    for z in [-2.0, -3.0] {
        let f = bits(face_rows(&fresh.moves, z));
        assert!(!f.is_empty(), "non-vacuity: a fresh Face cuts level {z}");
        assert_eq!(bits(face_rows(&rest.moves, z)), f, "level {z}");
    }
}

/// 14. B8, part of the top: after Pocket A a rest Face cuts on the rows of
///     the fresh Face only. Each rest row lies on a fresh row and inside
///     that row's X span, so no rest row reaches past the fresh rows.
#[test]
fn a_rest_face_never_reaches_past_the_fresh_rows() {
    let fresh = run_ops(pocket(A_DEPTH), face_b8(2.0), 0.0, StockSource::Fresh, &[]);
    let rest = run_ops(
        pocket(A_DEPTH),
        face_b8(2.0),
        0.0,
        StockSource::FromRemainingStock,
        &[],
    );
    let finding = rest.finding.expect("a rest Face records its level clip");
    assert_eq!(
        finding.clipped_levels, 2,
        "both levels are clipped: {finding:?}"
    );
    for z in [-1.0, -2.0] {
        let rows = face_rows(&fresh.moves, z);
        assert!(!rows.is_empty(), "non-vacuity: a fresh Face cuts level {z}");
        let rest_rows = face_rows(&rest.moves, z);
        assert!(!rest_rows.is_empty(), "the rest Face cuts level {z}");
        for p in rest_rows.iter().flatten() {
            let span = rows
                .iter()
                .flatten()
                .filter(|q| (q.y - p.y).abs() < 1e-9)
                .fold(None, |acc: Option<(f64, f64)>, q| {
                    Some(acc.map_or((q.x, q.x), |(lo, hi)| (lo.min(q.x), hi.max(q.x))))
                });
            let Some((lo, hi)) = span else {
                panic!(
                    "level {z}: a rest row at y = {:.4} lies on no fresh row",
                    p.y
                );
            };
            assert!(
                p.x >= lo - 1e-9 && p.x <= hi + 1e-9,
                "level {z}: a rest row reaches x = {:.4} past the fresh row \
                 [{lo:.4}, {hi:.4}] at y = {:.4}",
                p.x,
                p.y
            );
        }
    }
}

/// The scan angle (degrees) of the B9 cases. The rows run along -Y, so
/// the first row of a region lies at its lowest X. The rest clip of the
/// shifted outline starts inside the outline on that side (the material A
/// left is the strip at the +X end), so the clip moves the region's first
/// row.
const ZIGZAG_B9_ANGLE: f64 = 270.0;

/// The stepover (mm) of the B9 cases: no divisor of the gap between the
/// clip region and the outline.
const ZIGZAG_B9_STEPOVER: f64 = 2.3;

/// 16. B9, Zigzag op and Pocket zigzag: on the shifted outline the rest
///     clip region is the strip A left, so it starts inside the outline.
///     Each rest row must lie on a fresh row (the same X, bit for bit),
///     cut in the direction of that fresh row, and stay inside its span.
#[test]
fn a_rest_zigzag_cuts_on_the_fresh_rows() {
    let ops = [
        (
            "zigzag op",
            OperationConfig::Zigzag(ZigzagConfig {
                stepover: ZIGZAG_B9_STEPOVER,
                angle: ZIGZAG_B9_ANGLE,
                depth: B_DEPTH,
                depth_per_pass: DPP,
                ..ZigzagConfig::default()
            }),
        ),
        (
            "pocket zigzag",
            OperationConfig::Pocket(PocketConfig {
                stepover: ZIGZAG_B9_STEPOVER,
                angle: ZIGZAG_B9_ANGLE,
                depth: B_DEPTH,
                depth_per_pass: DPP,
                pattern: PocketPattern::Zigzag,
                ..PocketConfig::default()
            }),
        ),
    ];
    for (name, op) in ops {
        let fresh = run_ops(pocket(A_DEPTH), op.clone(), SHIFT, StockSource::Fresh, &[]);
        let rest = run_ops(
            pocket(A_DEPTH),
            op,
            SHIFT,
            StockSource::FromRemainingStock,
            &[],
        );
        let finding = rest.finding.expect("a rest zigzag records its level clip");
        assert!(
            finding.clipped_levels > 0,
            "{name}: precondition, the strip clips a level: {finding:?}"
        );
        let mut judged = 0;
        for z in ladder(A_DEPTH - DPP) {
            let rows = cut_rows(&fresh.moves, z);
            assert!(!rows.is_empty(), "{name}: non-vacuity, a fresh row at {z}");
            for [a, b] in cut_rows(&rest.moves, z) {
                let Some([fa, fb]) = rows.iter().find(|[fa, _]| fa.x.to_bits() == a.x.to_bits())
                else {
                    panic!(
                        "{name}: level {z}: a rest row at x = {:.4} lies on no fresh row",
                        a.x
                    );
                };
                assert_eq!(
                    (b.y - a.y).signum(),
                    (fb.y - fa.y).signum(),
                    "{name}: level {z}: the rest row at x = {:.4} cuts against its fresh row",
                    a.x
                );
                let (lo, hi) = (fa.y.min(fb.y), fa.y.max(fb.y));
                for p in [a, b] {
                    assert!(
                        p.y >= lo - 1e-9 && p.y <= hi + 1e-9,
                        "{name}: level {z}: the rest row at x = {:.4} reaches y = {:.4} \
                         past its fresh row [{lo:.4}, {hi:.4}]",
                        a.x,
                        p.y
                    );
                }
                judged += 1;
            }
        }
        eprintln!("{name}: {judged} rest rows judged above A's floor");
        assert!(
            judged > 0,
            "{name}: the rest zigzag cuts rows above A's floor"
        );
    }
}
