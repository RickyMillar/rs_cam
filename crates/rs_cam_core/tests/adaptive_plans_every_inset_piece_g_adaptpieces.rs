//! G-ADAPTPIECES — 2D Adaptive plans every piece of the machinable inset.
//!
//! # The defect this file pins (D2)
//!
//! The 2D adaptive planner insets the outline by the tool radius to get the
//! region a cutter centre may visit. When that inset splits into several
//! pieces, the planner kept only the first piece (`machinable_vec[0]` in
//! `adaptive/path.rs`). Every other piece was not planned, and its material
//! stayed (`planning/op_architecture_2026-10-14/AUDIT.md`, D2).
//!
//! The planner still plans one piece by design (it is shared with the
//! Adaptive3d slices, which split the same way: G-ADAPT3DPIECES). The fix is in the 2D adapter
//! (`compute/execute/clearing_2d.rs`): `adaptive::inset_piece_outlines`
//! gives one outline for each piece, and the adapter plans each to full
//! depth in turn.
//!
//! # The fixture
//!
//! A dumbbell: two 20 mm square lobes joined by a neck 5 mm wide. The tool
//! is a Ø6 mm end mill, so its centre cannot pass the neck: the inset by
//! R = 3 mm is two pieces, one in each lobe. The board hangs below world
//! Z0. 2D Adaptive cuts the outline to `DEPTH` on fresh stock, and the
//! session simulates it.
//!
//! # The claims
//!
//! 1. The fixture is what it says: the inset is two pieces.
//! 2. The discriminator: under the `Legacy` cleanup (no residue pass over
//!    every piece) each lobe is cleared. No cell that the tool covers
//!    completely from its own inset piece keeps material above the floor
//!    plus `FRESH_MATERIAL_THRESHOLD_MM`. Red with the split disabled.
//! 3. A guard on the default cleanup, under a ramp and under the default
//!    helix entry: the same claim.
//! 4. Each arm: the move from one lobe to the other crosses the neck above
//!    the stock top, and the neck centre keeps its full height.

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

use common::session::{generate, polygon_model, single_op_session_with};
use common::tools::endmill_tool_config;
use rs_cam_core::adaptive::CleanupStrategy;
use rs_cam_core::compute::StockConfig;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::DressupEntryStyle;
use rs_cam_core::compute::operation_configs::AdaptiveConfig;
use rs_cam_core::dexel_stock::{FRESH_MATERIAL_THRESHOLD_MM, TriDexelStock};
use rs_cam_core::geo::P2;
use rs_cam_core::polygon::{Polygon2, offset_polygon};
use rs_cam_core::session::{
    Command, ProjectSession, SetSimulationResolutionArgs, SimulationOptions, SimulationResolution,
};
use rs_cam_core::toolpath::Move;

/// Side (mm) of each square lobe.
const LOBE: f64 = 20.0;

/// Length (mm) of the neck between the lobes.
const NECK_LEN: f64 = 10.0;

/// Width (mm) of the neck. It is less than the tool diameter, so the
/// inset by the tool radius splits at the neck.
const NECK_W: f64 = 5.0;

/// Cutter diameter (mm) and radius (mm).
const TOOL_D: f64 = 6.0;
const TOOL_R: f64 = TOOL_D / 2.0;

/// Depth (mm) of the clear, and its depth per pass (mm).
const DEPTH: f64 = 3.0;
const DPP: f64 = 1.5;

/// Stock margin (mm) around the outline, and stock height (mm).
const MARGIN: f64 = 2.0;
const STOCK_Z: f64 = 10.0;

/// Simulation cell (mm).
const CELL_MM: f64 = 0.5;

/// The fraction of judged cells the helix arm allows to stand: 5 %. It
/// is above the 2.1 % that `Legacy` leaves on the left piece outline alone
/// and below the 40 % that the defect left.
const HELIX_RESIDUE_FRACTION: f64 = 0.05;

/// The fraction of judged cells the default helix arm allows to stand:
/// 0.1 %, which is one cell of a lobe (1408 and 1421 judged cells).
const DEFAULT_HELIX_SLIVER_FRACTION: f64 = 0.001;

/// Half extent (mm) of the neck centre that keeps its full height. A
/// cutter centre in a lobe stays at least `NECK_LEN / 2` from the origin
/// in X, so with R = 3 mm it reaches no closer than 2 mm.
const NECK_PROBE: f64 = NECK_LEN / 2.0 - TOOL_R - CELL_MM;

/// Half the X extent (mm) of the dumbbell.
const HALF_X: f64 = LOBE + NECK_LEN / 2.0;

/// The dumbbell, wound CCW, centred on the origin.
fn dumbbell() -> Polygon2 {
    let (n, h, w) = (NECK_LEN / 2.0, LOBE / 2.0, NECK_W / 2.0);
    Polygon2::new(vec![
        P2::new(-HALF_X, -h),
        P2::new(-n, -h),
        P2::new(-n, -w),
        P2::new(n, -w),
        P2::new(n, -h),
        P2::new(HALF_X, -h),
        P2::new(HALF_X, h),
        P2::new(n, h),
        P2::new(n, w),
        P2::new(-n, w),
        P2::new(-n, h),
        P2::new(-HALF_X, h),
    ])
}

fn stock() -> StockConfig {
    StockConfig {
        x: 2.0 * (HALF_X + MARGIN),
        y: LOBE + 2.0 * MARGIN,
        z: STOCK_Z,
        origin_x: -HALF_X - MARGIN,
        origin_y: -LOBE / 2.0 - MARGIN,
        origin_z: -STOCK_Z,
        auto_from_model: false,
        ..StockConfig::default()
    }
}

fn adaptive(cleanup: CleanupStrategy) -> OperationConfig {
    OperationConfig::Adaptive(AdaptiveConfig {
        depth: DEPTH,
        depth_per_pass: DPP,
        cleanup_strategy: cleanup,
        ..AdaptiveConfig::default()
    })
}

fn simulate(session: &mut ProjectSession) -> Arc<TriDexelStock> {
    let _ = session
        .apply(Command::SetSimulationResolution(
            SetSimulationResolutionArgs {
                resolution: SimulationResolution::Fixed(CELL_MM),
            },
        ))
        .expect("a positive cell");
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
    session
        .simulation_result()
        .and_then(|sim| sim.checkpoints.last().map(|c| Arc::clone(&c.mesh_stock)))
        .expect("the simulation carved the op")
}

/// 1. The inset by the tool radius is two pieces, one in each lobe.
#[test]
fn the_dumbbell_inset_is_two_pieces() {
    let pieces = offset_polygon(&dumbbell(), TOOL_R);
    assert_eq!(pieces.len(), 2, "the inset must split at the neck");
    let left = pieces
        .iter()
        .filter(|p| p.contains_point(&P2::new(-HALF_X + LOBE / 2.0, 0.0)))
        .count();
    let right = pieces
        .iter()
        .filter(|p| p.contains_point(&P2::new(HALF_X - LOBE / 2.0, 0.0)))
        .count();
    assert_eq!((left, right), (1, 1), "one piece in each lobe");
}

/// 2. The discriminator: the `Legacy` cleanup has no residue pass over
///    every piece, so before the fix one lobe stood whole.
#[test]
fn legacy_cleanup_clears_both_lobes_under_a_straight_plunge() {
    let run = run(CleanupStrategy::Legacy, DressupEntryStyle::None);
    run.assert_lobes_cleared(0.0);
    run.assert_the_neck_is_crossed_above_the_stock();
}

/// 2. The discriminator under the default helix entry. Before the fix
///    the `[0]` lobe stood whole (1408 of 1408 cells) and the other lobe
///    stood in 573 of 1421 cells (40 %). After the fix the left lobe keeps
///    30 wall cells (2.1 %). The planner leaves the same 30 cells when the
///    left piece outline alone is the model (control run, 2026-10-14), so
///    that residue belongs to `Legacy` under a helix on this shape, not to
///    the piece split. This arm allows `HELIX_RESIDUE_FRACTION`.
#[test]
fn legacy_cleanup_clears_both_lobes_under_a_helix_entry() {
    let run = run(CleanupStrategy::Legacy, DressupEntryStyle::Helix);
    run.assert_lobes_cleared(HELIX_RESIDUE_FRACTION);
    run.assert_the_neck_is_crossed_above_the_stock();
}

/// 3. A regression guard on the default cleanup. Before the fix its wall
///    pass and mop over every piece already cleared the second lobe, so
///    this arm does not discriminate the defect.
#[test]
fn default_cleanup_clears_both_lobes_under_a_ramp_entry() {
    let run = run(
        CleanupStrategy::ContourParallelHybrid,
        DressupEntryStyle::Ramp,
    );
    run.assert_lobes_cleared(0.0);
    run.assert_the_neck_is_crossed_above_the_stock();
}

/// 3. The default op: the default cleanup under the default helix entry.
///    One cell stands 0.074 mm at (-5.5, 3.0), beside the neck corner. The
///    same cell stood by the same amount before the fix, so it is not the
///    piece split. This arm allows `DEFAULT_HELIX_SLIVER_FRACTION`.
#[test]
fn default_cleanup_clears_both_lobes_under_a_helix_entry() {
    let run = run(
        CleanupStrategy::ContourParallelHybrid,
        DressupEntryStyle::Helix,
    );
    run.assert_lobes_cleared(DEFAULT_HELIX_SLIVER_FRACTION);
    run.assert_the_neck_is_crossed_above_the_stock();
}

/// What one arm left behind.
struct Run {
    label: String,
    moves: Vec<Move>,
    stock: Arc<TriDexelStock>,
}

fn run(cleanup: CleanupStrategy, style: DressupEntryStyle) -> Run {
    let mut session = single_op_session_with(
        stock(),
        endmill_tool_config(TOOL_D),
        polygon_model(vec![dumbbell()], "dumbbell"),
        "adaptive",
        adaptive(cleanup),
        |cfg| cfg.dressups.entry_style = style,
    );
    generate(&mut session, 0);
    let moves = session
        .get_result(0)
        .expect("the op holds a result")
        .toolpath()
        .moves
        .clone();
    let stock = simulate(&mut session);
    Run {
        label: format!("{cleanup:?} / {style:?}"),
        moves,
        stock,
    }
}

impl Run {
    /// Claim 2: no more than `allowed` of the cells that the tool covers
    /// completely from a piece keep material above the floor plus the
    /// fresh-material threshold.
    fn assert_lobes_cleared(&self, allowed: f64) {
        let grid = &self.stock.z_grid;
        let floor = -DEPTH;
        let mut tally = Vec::new();
        for (i, piece) in offset_polygon(&dumbbell(), TOOL_R).iter().enumerate() {
            // The reach of a piece: the piece grown by R less one cell (the
            // simulator blends a cell the tool covers in part).
            let reach = offset_polygon(piece, -(TOOL_R - CELL_MM));
            let mut judged = 0_usize;
            let mut standing = 0_usize;
            let mut worst: f64 = f64::NEG_INFINITY;
            for r in 0..grid.rows {
                for c in 0..grid.cols {
                    let (x, y) = grid.cell_to_world(r, c);
                    if !reach.iter().any(|q| q.contains_point(&P2::new(x, y))) {
                        continue;
                    }
                    judged += 1;
                    let top = f64::from(grid.top_z_at(r, c).unwrap_or(f32::NEG_INFINITY));
                    let above = top - floor;
                    worst = worst.max(above);
                    if above > FRESH_MATERIAL_THRESHOLD_MM {
                        standing += 1;
                        eprintln!(
                            "  {} piece {i}: ({x:.2}, {y:.2}) stands {above:.3} mm",
                            self.label
                        );
                    }
                }
            }
            let n = piece.exterior.len() as f64;
            let mid_x = piece.exterior.iter().map(|q| q.x).sum::<f64>() / n;
            eprintln!(
                "{} piece {i} (mean vertex x {mid_x:.2}): {standing} of {judged} cells stand \
                 above the floor; worst {worst:.3} mm",
                self.label
            );
            tally.push((i, judged, standing, worst));
        }
        for (i, judged, standing, worst) in tally {
            assert!(
                judged > 0,
                "{} piece {i}: the judged set must not be empty",
                self.label
            );
            assert!(
                standing as f64 <= allowed * judged as f64,
                "{} piece {i}: material stands above the floor in {standing} of {judged} \
                 cells in the reach of the piece (worst {worst:.3} mm)",
                self.label
            );
        }
    }

    /// Claim 4: the move from one lobe to the other crosses the neck above
    /// the stock top, and the neck centre keeps its full height. A
    /// lobe-to-lobe move at depth would fail both.
    fn assert_the_neck_is_crossed_above_the_stock(&self) {
        let mut crossings = 0_usize;
        for pair in self.moves.windows(2) {
            let (a, b) = (pair[0].target, pair[1].target);
            if (a.x < 0.0) != (b.x < 0.0) {
                crossings += 1;
                assert!(
                    a.z >= 0.0 && b.z >= 0.0,
                    "{}: a move crosses the neck below the stock top: {a:?} -> {b:?}",
                    self.label
                );
            }
        }
        assert!(
            crossings > 0,
            "{}: the toolpath must visit both lobes",
            self.label
        );
        let grid = &self.stock.z_grid;
        for r in 0..grid.rows {
            for c in 0..grid.cols {
                let (x, y) = grid.cell_to_world(r, c);
                if x.abs() > NECK_PROBE || y.abs() > NECK_PROBE {
                    continue;
                }
                let top = f64::from(grid.top_z_at(r, c).unwrap_or(f32::NEG_INFINITY));
                assert!(
                    top >= -FRESH_MATERIAL_THRESHOLD_MM,
                    "{}: the neck centre at ({x:.2}, {y:.2}) was cut to {top:.3} mm",
                    self.label
                );
            }
        }
    }
}
