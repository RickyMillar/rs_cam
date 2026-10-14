//! W3.0 / W3.1 — the Layered framework owns the Z ladder of a 2.5D op.
//!
//! # Why this exists
//!
//! `compute/execute/layered.rs` holds `run_layered`. It owns the ladder, the
//! rest clip per level, the prepare cache per region, the region order and
//! the depth-run spans of every Layered op. Pocket is the first op on it
//! (`planning/op_architecture_2026-10-14/PLAN.md`, W3).
//!
//! # The fixture
//!
//! One Ø6 mm end mill, a 10 mm board below world Z0, and ONE polygon model
//! that holds two disjoint 16 mm squares. The pocket cuts 3 mm at 1 mm per
//! pass, so each region has three levels.
//!
//! # The claims
//!
//! 1. Region-first (the default) gives the toolpath that the hand-written
//!    Pocket adapter gave before W3.1. The pinned fingerprints come from
//!    commit 68226ec5, for both patterns.
//! 2. Level-first (`RegionOrdering::Global`) visits every region at each
//!    level before it descends. It has one depth pass per level, where
//!    region-first has one per region and level. It has the same number of
//!    retracts. It cuts the same material: the final stock of the two
//!    orders is equal on every cell more than one cell from the edge of
//!    the tool footprint.
//!
//! The third claim (a cancel set mid-ladder stops at the next level) needs a
//! hook inside the ladder, so it is a unit test beside the framework:
//! `--lib compute::execute::layered`.

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

use common::fingerprint::move_fingerprint;
use common::session::{generate, polygon_model, single_op_session};
use common::tools::endmill_tool_config;
use rs_cam_core::compute::StockConfig;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::{PocketConfig, PocketPattern, RegionOrdering};
use rs_cam_core::dexel_stock::{FRESH_MATERIAL_THRESHOLD_MM, TriDexelStock};
use rs_cam_core::geo::P2;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{
    Command, ProjectSession, SetSimulationResolutionArgs, SetToolpathParamArgs, SimulationOptions,
    SimulationResolution,
};
use rs_cam_core::toolpath::{Move, MoveIntent, MoveType, Toolpath};
use rs_cam_core::trace::toolpath_spans::SpanKind;

/// Half-extent (mm) of each square.
const HALF: f64 = 8.0;

/// X of the centre (mm) of each square. The gap between the squares is
/// `2 * CENTRE_X - 2 * HALF` = 14 mm, more than the tool diameter.
const CENTRE_X: f64 = 15.0;

/// Stock margin (mm) around the two squares.
const MARGIN: f64 = 2.0;

/// Stock height (mm). The board hangs below `z = 0`.
const STOCK_Z: f64 = 10.0;

/// Cutter diameter (mm).
const TOOL_D: f64 = 6.0;

/// Cutter radius (mm).
const TOOL_R: f64 = TOOL_D / 2.0;

/// Pocket depth (mm) and depth per pass (mm): three levels.
const DEPTH: f64 = 3.0;
const DPP: f64 = 1.0;

/// Level count: `DEPTH / DPP`.
const LEVELS: usize = 3;

/// Simulation cell (mm).
const CELL_MM: f64 = 0.5;

/// The move fingerprints `(count, hash)` of the region-first two-region
/// Pocket, generated on commit 68226ec5 (before W3.1) by this file.
const BEFORE_CONTOUR: (usize, u64) = (232, 18_123_423_408_609_281_905);
const BEFORE_ZIGZAG: (usize, u64) = (312, 13_353_448_158_642_052_239);

fn square_at(cx: f64) -> Polygon2 {
    Polygon2::new(vec![
        P2::new(cx - HALF, -HALF),
        P2::new(cx + HALF, -HALF),
        P2::new(cx + HALF, HALF),
        P2::new(cx - HALF, HALF),
    ])
}

fn stock() -> StockConfig {
    StockConfig {
        x: 2.0 * (CENTRE_X + HALF + MARGIN),
        y: 2.0 * (HALF + MARGIN),
        z: STOCK_Z,
        origin_x: -CENTRE_X - HALF - MARGIN,
        origin_y: -HALF - MARGIN,
        origin_z: -STOCK_Z,
        auto_from_model: false,
        ..StockConfig::default()
    }
}

fn pocket(pattern: PocketPattern) -> OperationConfig {
    ordered_pocket(pattern, RegionOrdering::ByArea)
}

fn ordered_pocket(pattern: PocketPattern, region_ordering: RegionOrdering) -> OperationConfig {
    OperationConfig::Pocket(PocketConfig {
        stepover: 2.0,
        depth: DEPTH,
        depth_per_pass: DPP,
        pattern,
        region_ordering,
        ..PocketConfig::default()
    })
}

fn session(op: OperationConfig) -> ProjectSession {
    let model = polygon_model(vec![square_at(-CENTRE_X), square_at(CENTRE_X)], "two");
    single_op_session(stock(), endmill_tool_config(TOOL_D), model, "pocket", op)
}

fn generated(op: OperationConfig) -> Toolpath {
    let mut s = session(op);
    generate(&mut s, 0);
    s.get_result(0).expect("a result").toolpath().clone()
}

/// Claim 1: region-first is the Pocket of commit 68226ec5.
#[test]
fn region_first_keeps_the_pocket_of_before_w3() {
    let contour = move_fingerprint(&generated(pocket(PocketPattern::Contour)));
    let zigzag = move_fingerprint(&generated(pocket(PocketPattern::Zigzag)));
    eprintln!("contour {contour:?}  zigzag {zigzag:?}");
    assert_eq!(contour, BEFORE_CONTOUR, "the Contour pocket changed");
    assert_eq!(zigzag, BEFORE_ZIGZAG, "the Zigzag pocket changed");
}

/// What one order left behind.
struct Arm {
    moves: Vec<Move>,
    depth_passes: usize,
    final_stock: Arc<TriDexelStock>,
}

/// Generate the two-region pocket in `ordering`, then simulate it.
fn run(pattern: PocketPattern, ordering: RegionOrdering) -> Arm {
    let mut s = session(ordered_pocket(pattern, ordering));
    generate(&mut s, 0);
    let _ = s
        .apply(Command::SetSimulationResolution(
            SetSimulationResolutionArgs {
                resolution: SimulationResolution::Fixed(CELL_MM),
            },
        ))
        .expect("a positive cell");
    let cancel = AtomicBool::new(false);
    let _ = s
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
    let result = s.get_result(0).expect("a result");
    let final_stock = s
        .simulation_result()
        .and_then(|sim| sim.checkpoints.last().map(|c| Arc::clone(&c.mesh_stock)))
        .expect("the simulation carved the pocket");
    Arm {
        moves: result.toolpath().moves.clone(),
        depth_passes: result
            .annotated()
            .spans_of_kind(SpanKind::DepthPass)
            .count(),
        final_stock,
    }
}

/// The ladder level (index) and the region (0 = -X, 1 = +X) of each
/// cutting move that lies on a ladder level, in toolpath order. An entry
/// move between levels is not on a level and is left out.
fn level_visits(moves: &[Move]) -> Vec<(usize, usize)> {
    moves
        .iter()
        .filter(|m| m.move_type != MoveType::Rapid && m.intent == MoveIntent::ClearingCut)
        .filter_map(|m| {
            let level = (1..=LEVELS).find(|&k| (m.target.z + k as f64 * DPP).abs() < 1e-6)?;
            Some((level - 1, usize::from(m.target.x > 0.0)))
        })
        .collect()
}

fn retracts(moves: &[Move]) -> usize {
    moves
        .iter()
        .filter(|m| m.intent == MoveIntent::Retract)
        .count()
}

/// The largest top difference (mm) between the two stocks, over every cell
/// whose centre lies more than one cell from the edge of the tool footprint
/// (a square with corners rounded to R). The simulator blends a cell that a
/// tool covers in part, and the blend depends on the direction of the pass
/// over it. The rapid-order stage may reverse a scan line in one order and
/// not in the other, so a cell in that one-cell band is not judged (the
/// method of G-RESTLEVELS claim 5). Also returns the count of judged cells.
fn worst_difference(a: &TriDexelStock, b: &TriDexelStock) -> (f32, usize) {
    let (a, b) = (&a.z_grid, &b.z_grid);
    assert_eq!((a.rows, a.cols), (b.rows, b.cols), "one grid shape");
    let inner = HALF - TOOL_R;
    let mut worst: f32 = 0.0;
    let mut judged = 0_usize;
    for r in 0..a.rows {
        for c in 0..a.cols {
            let (x, y) = a.cell_to_world(r, c);
            let dx = ((x.abs() - CENTRE_X).abs() - inner).max(0.0);
            let dy = (y.abs() - inner).max(0.0);
            let edge = (dx.hypot(dy) - TOOL_R).abs();
            if edge <= CELL_MM {
                continue;
            }
            judged += 1;
            let ta = a.top_z_at(r, c).unwrap_or(f32::NEG_INFINITY);
            let tb = b.top_z_at(r, c).unwrap_or(f32::NEG_INFINITY);
            worst = worst.max((ta - tb).abs());
        }
    }
    (worst, judged)
}

/// Claim 2: level-first visits every region at each level before it
/// descends, has one depth pass per level and the same retract count, and
/// cuts the same material as region-first.
#[test]
fn level_first_cuts_each_level_everywhere_before_it_descends() {
    for pattern in [PocketPattern::Contour, PocketPattern::Zigzag] {
        let region_first = run(pattern, RegionOrdering::ByArea);
        let level_first = run(pattern, RegionOrdering::Global);

        // Region-first: region 0 to full depth, then region 1. Non-vacuity:
        // the level climbs back to 0 when region 1 starts.
        let visits = level_visits(&region_first.moves);
        assert!(
            visits.windows(2).any(|w| w[1].0 < w[0].0),
            "{pattern:?}: region-first must climb back to the top for the second region"
        );

        // Level-first: the level never climbs, and each level cuts both
        // regions.
        let visits = level_visits(&level_first.moves);
        assert!(
            visits.windows(2).all(|w| w[1].0 >= w[0].0),
            "{pattern:?}: level-first must never climb back to a level it left"
        );
        for level in 0..LEVELS {
            for region in 0..2 {
                assert!(
                    visits.contains(&(level, region)),
                    "{pattern:?}: level {level} must cut region {region}"
                );
            }
        }

        // One depth pass per (region, level) against one per level.
        assert_eq!(region_first.depth_passes, 2 * LEVELS, "{pattern:?}");
        assert_eq!(level_first.depth_passes, LEVELS, "{pattern:?}");

        // Each (region, level) piece ends with its own retract in both
        // orders, so the count is equal.
        let (rf, lf) = (retracts(&region_first.moves), retracts(&level_first.moves));
        assert!(rf > 0, "{pattern:?}: the fixture must retract");
        assert_eq!(rf, lf, "{pattern:?}: the retract count must not change");

        // Same material: the regions are 14 mm apart, more than the tool,
        // so the order of the regions cannot change a cell. Each cell more
        // than one cell from the footprint edge is judged.
        let (worst, judged) = worst_difference(&region_first.final_stock, &level_first.final_stock);
        assert!(judged > 0, "{pattern:?}: the judged set must not be empty");
        eprintln!(
            "{pattern:?}: moves {} / {}, depth passes {} / {}, retracts {rf} / {lf}, \
             worst top difference {worst:.6} mm over {judged} cells",
            region_first.moves.len(),
            level_first.moves.len(),
            region_first.depth_passes,
            level_first.depth_passes,
        );
        assert!(
            f64::from(worst) <= FRESH_MATERIAL_THRESHOLD_MM,
            "{pattern:?}: level-first left {worst} mm more or less than region-first"
        );
    }
}

/// The `region_ordering` dial is live on the command door every surface
/// shares (MCP `set_toolpath_param`, the CLI job): a write of `global`
/// reaches the Pocket config, and a token that is not an order is refused.
#[test]
fn the_region_ordering_dial_is_written_through_the_command_door() {
    let mut s = session(pocket(PocketPattern::Contour));
    let ordering = |s: &ProjectSession| match &s.get_toolpath_config(0).expect("a config").operation
    {
        OperationConfig::Pocket(cfg) => cfg.region_ordering,
        other => panic!("not a pocket: {other:?}"),
    };
    assert_eq!(ordering(&s), RegionOrdering::ByArea, "the default");
    let _ = s
        .apply(Command::SetToolpathParam(SetToolpathParamArgs {
            index: 0,
            param: "region_ordering".to_owned(),
            value: serde_json::json!("global"),
        }))
        .expect("global is a region order");
    assert_eq!(ordering(&s), RegionOrdering::Global);
    let refused = s.apply(Command::SetToolpathParam(SetToolpathParamArgs {
        index: 0,
        param: "region_ordering".to_owned(),
        value: serde_json::json!("by-area"),
    }));
    assert!(refused.is_err(), "an unknown token must be refused");
    assert_eq!(
        ordering(&s),
        RegionOrdering::Global,
        "a refusal writes nothing"
    );
}
