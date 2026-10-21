//! W3.0 / W3.1 / W3.2 — the Layered framework owns the Z ladder of a 2.5D op.
//!
//! # Why this exists
//!
//! `compute/execute/layered.rs` holds `run_layered`. It owns the ladder, the
//! rest clip per level, the prepare cache per region, the region order and
//! the depth-run spans of every Layered op. Pocket is the first op on it
//! (W3.1); Zigzag, Trace and Profile follow (W3.2)
//! (`planning/op_architecture_2026-10-14/PLAN.md`, W3).
//!
//! # The fixture
//!
//! One Ø6 mm end mill, a 10 mm board below world Z0, and ONE polygon model
//! that holds two disjoint 16 mm squares. Each op cuts 3 mm at 1 mm per
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
//! 4. Region-first Zigzag, Trace (left compensation) and Profile (with
//!    tabs, with and without a spring pass) give the toolpath of the
//!    hand-written adapters. The pinned fingerprints come from commit
//!    86692c56, before W3.2.
//! 5. Zigzag, Trace and Profile take the region order: level-first never
//!    climbs back, cuts both regions at each level, holds the same cuts and
//!    the same retract count, and has fewer depth passes.
//! 6. Profile tabs land only in a pass at the final Z, in both orders. A
//!    spring pass repeats the final Z and carries the tabs too.
//! 7. Trace keeps its own semantic annotation (one region, chains).
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
use rs_cam_core::compute::operation_configs::{
    PocketConfig, PocketPattern, ProfileConfig, RegionOrdering, TraceCompensation, TraceConfig,
    ZigzagConfig,
};
use rs_cam_core::dexel_stock::{FRESH_MATERIAL_THRESHOLD_MM, TriDexelStock};
use rs_cam_core::geo::P2;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{
    Command, ProjectSession, SetSimulationResolutionArgs, SetToolpathParamArgs, SimulationOptions,
    SimulationResolution,
};
use rs_cam_core::toolpath::{Move, MoveIntent, MoveType, Toolpath};
use rs_cam_core::trace::semantic_trace::ToolpathSemanticKind;
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
        floor: rs_cam_core::compute::floor::Floor::top(DEPTH),
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
                resolution: rs_cam_core::session::SimulationResolution::Fixed(CELL_MM),
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
/// shares (MCP `set_toolpath_param`, the CLI): for Pocket, Zigzag, Trace
/// and Profile, a write of `global` reaches the config, and a token that is
/// not an order is refused.
#[test]
fn the_region_ordering_dial_is_written_through_the_command_door() {
    let ops = [
        pocket(PocketPattern::Contour),
        zigzag_op(RegionOrdering::ByArea),
        trace_op(RegionOrdering::ByArea),
        profile_op(0, RegionOrdering::ByArea),
    ];
    for op in ops {
        let name = op.label();
        let mut s = session(op);
        let ordering =
            |s: &ProjectSession| match &s.get_toolpath_config(0).expect("a config").operation {
                OperationConfig::Pocket(cfg) => cfg.region_ordering,
                OperationConfig::Zigzag(cfg) => cfg.region_ordering,
                OperationConfig::Trace(cfg) => cfg.region_ordering,
                OperationConfig::Profile(cfg) => cfg.region_ordering,
                other => panic!("not a Layered op: {other:?}"),
            };
        assert_eq!(ordering(&s), RegionOrdering::ByArea, "{name}: the default");
        let _ = s
            .apply(Command::SetToolpathParam(SetToolpathParamArgs {
                index: 0,
                param: "region_ordering".to_owned(),
                value: serde_json::json!("global"),
            }))
            .expect("global is a region order");
        assert_eq!(ordering(&s), RegionOrdering::Global, "{name}");
        let refused = s.apply(Command::SetToolpathParam(SetToolpathParamArgs {
            index: 0,
            param: "region_ordering".to_owned(),
            value: serde_json::json!("by-area"),
        }));
        assert!(refused.is_err(), "{name}: an unknown token must be refused");
        assert_eq!(
            ordering(&s),
            RegionOrdering::Global,
            "{name}: a refusal writes nothing"
        );
    }
}

// ── W3.2: Zigzag, Trace and Profile on the runner ─────────────────────

/// The move fingerprints `(count, hash)` of the two-region Zigzag, Trace and
/// Profile, generated on commit 86692c56 (before W3.2) by this file.
const BEFORE_ZIGZAG_OP: (usize, u64) = (312, 13_353_448_158_642_052_239);
/// Re-pinned for P6.0 (2026-10-21), the corner gouge fix: the outward
/// offset of each square now goes round each corner on points ON the arc,
/// not on the chord. The chord cut 0.29 r into the corner. Before:
/// `(96, 4_273_299_077_702_301_738)`.
/// P6.0c: the Trace Left lead now turns away from the part. Before: `(96, 11_387_551_302_651_154_206)`.
const BEFORE_TRACE: (usize, u64) = (96, 6_617_181_410_166_771_539);
/// The two Profile pins were re-pinned for geometry selection P5
/// (2026-10-19): tab placement changed by design. The 16 mm sides are
/// shorter than a tab needs (6 mm plus two 9 mm corner clearances), so each
/// square takes the even spacing by length, as before. But the pass now
/// starts in the largest gap between tabs (`dressup::tabs::tabbed_ring`),
/// so the first tab is whole: before, it sat on the plunge point, half
/// size, with a slanted lift.
///
/// Re-pinned again for P5b (2026-10-19), by design: `tab_width` is now
/// the bridge, so each lift is longer by the tool diameter
/// (`dressup::tabs::tab_lift_length`). The longer lifts move the split
/// points on the ring (move counts 102 -> 106, 150 -> 154). The
/// tab top (`TAB_H`) stands above the final level only, so the P5b level
/// rule adds no lift here.
///
/// Re-pinned for P6.0 (2026-10-21), the corner gouge fix: the Profile
/// Outside ring now goes round each corner on points ON the arc, not on
/// the chord (0.29 r into the corner). Before: `(106, 10_647_655_285_267_498_227)`
/// and `(154, 6_534_920_059_359_883_601)`.
///
/// Re-pinned for P6.0b (2026-10-21): the segment merge on a finished edge
/// is capped at 0.010 mm, so the arc points on each corner stay (move
/// counts 110 -> 124, 150 -> 174). With the cap removed, both pins read
/// their old values. Before: `(110, 5_137_917_641_995_499_451)` and
/// `(150, 13_892_067_383_589_783_972)`.
const BEFORE_PROFILE_TABS: (usize, u64) = (124, 1_011_377_598_541_499_624);
const BEFORE_PROFILE_TABS_SPRING: (usize, u64) = (174, 4_487_676_720_641_426_579);

/// Tab count of the Profile cases.
const TABS: usize = 2;

/// Tab height (mm) of the Profile cases. The tab top stands at
/// `-DEPTH + TAB_H`, between the two deepest ladder levels.
const TAB_H: f64 = 0.5;

fn zigzag_op(region_ordering: RegionOrdering) -> OperationConfig {
    OperationConfig::Zigzag(ZigzagConfig {
        stepover: 2.0,
        floor: rs_cam_core::compute::floor::Floor::top(DEPTH),
        depth_per_pass: DPP,
        region_ordering,
        ..ZigzagConfig::default()
    })
}

/// O-1 (P6.2): `Right` is the outward offset that `Left` was before the
/// ruling, so the pinned toolpath of before W3.2 holds unchanged.
fn trace_op(region_ordering: RegionOrdering) -> OperationConfig {
    OperationConfig::Trace(TraceConfig {
        depth: DEPTH,
        depth_per_pass: DPP,
        compensation: TraceCompensation::Right,
        region_ordering,
        ..TraceConfig::default()
    })
}

fn profile_op(finishing_passes: usize, region_ordering: RegionOrdering) -> OperationConfig {
    OperationConfig::Profile(ProfileConfig {
        floor: rs_cam_core::compute::floor::Floor::top(DEPTH),
        depth_per_pass: DPP,
        tab_count: TABS,
        tab_height: TAB_H,
        finishing_passes,
        region_ordering,
        ..ProfileConfig::default()
    })
}

/// Claim 4: region-first Zigzag, Trace and Profile give the toolpath of
/// the hand-written adapters before W3.2. The Profile cases have tabs; the
/// second also has one spring pass, which repeats the final Z.
#[test]
fn region_first_keeps_zigzag_trace_and_profile_of_before_w3_2() {
    let cases = [
        (
            "zigzag",
            zigzag_op(RegionOrdering::ByArea),
            BEFORE_ZIGZAG_OP,
        ),
        ("trace", trace_op(RegionOrdering::ByArea), BEFORE_TRACE),
        (
            "profile tabs",
            profile_op(0, RegionOrdering::ByArea),
            BEFORE_PROFILE_TABS,
        ),
        (
            "profile tabs spring",
            profile_op(1, RegionOrdering::ByArea),
            BEFORE_PROFILE_TABS_SPRING,
        ),
    ];
    let mut changed = Vec::new();
    for (name, op, before) in cases {
        let now = move_fingerprint(&generated(op));
        eprintln!("{name}: {now:?}");
        if now != before {
            changed.push(name);
        }
    }
    assert!(changed.is_empty(), "changed: {changed:?}");
}

/// What one order of a W3.2 op gave: the moves and the depth-pass count.
fn generated_with_passes(op: OperationConfig) -> (Vec<Move>, usize) {
    let mut s = session(op);
    generate(&mut s, 0);
    let result = s.get_result(0).expect("a result");
    (
        result.toolpath().moves.clone(),
        result
            .annotated()
            .spans_of_kind(SpanKind::DepthPass)
            .count(),
    )
}

/// The ladder level (index) and the region (0 = -X, 1 = +X) of each fed
/// cut on a ladder level, in toolpath order. A spring pass shares the
/// index of the final level.
fn ladder_visits(moves: &[Move]) -> Vec<(usize, usize)> {
    moves
        .iter()
        .filter(|m| {
            matches!(m.move_type, MoveType::Linear { .. })
                && matches!(m.intent, MoveIntent::ClearingCut | MoveIntent::FinishingCut)
        })
        .filter_map(|m| {
            let level = (1..=LEVELS).find(|&k| (m.target.z + k as f64 * DPP).abs() < 1e-6)?;
            Some((level - 1, usize::from(m.target.x > 0.0)))
        })
        .collect()
}

/// The target and intent of each fed cut (`ClearingCut`, `FinishingCut`),
/// as text, sorted: the multiset of cuts, blind to the order. The entry
/// dressup and the feed stage read the order, so the entry moves and the
/// feeds are left out.
fn sorted_cuts(moves: &[Move]) -> Vec<String> {
    let mut all: Vec<String> = moves
        .iter()
        .filter(|m| {
            matches!(m.move_type, MoveType::Linear { .. })
                && matches!(m.intent, MoveIntent::ClearingCut | MoveIntent::FinishingCut)
        })
        .map(|m| {
            format!(
                "{:?} {:.6} {:.6} {:.6}",
                m.intent, m.target.x, m.target.y, m.target.z
            )
        })
        .collect();
    all.sort();
    all
}

/// Claim 5: Zigzag, Trace and Profile on the runner take the region order.
/// Level-first never climbs back to a level it left and cuts both regions
/// at each level; region-first climbs back for the second region. Both
/// orders hold the same cuts and the same retract count, and level-first
/// has fewer depth passes.
#[test]
fn zigzag_trace_and_profile_take_the_region_order() {
    type Build = fn(RegionOrdering) -> OperationConfig;
    let cases: [(&str, Build); 3] = [
        ("zigzag", zigzag_op),
        ("trace", trace_op),
        ("profile", |o| profile_op(0, o)),
    ];
    for (name, build) in cases {
        let (by_area, by_area_passes) = generated_with_passes(build(RegionOrdering::ByArea));
        let (global, global_passes) = generated_with_passes(build(RegionOrdering::Global));
        eprintln!(
            "{name}: moves {} / {}, depth passes {by_area_passes} / {global_passes}, \
             retracts {} / {}",
            by_area.len(),
            global.len(),
            retracts(&by_area),
            retracts(&global),
        );

        let visits = ladder_visits(&by_area);
        assert!(
            visits.windows(2).any(|w| w[1].0 < w[0].0),
            "{name}: region-first must climb back to the top for the second region"
        );
        let visits = ladder_visits(&global);
        assert!(
            visits.windows(2).all(|w| w[1].0 >= w[0].0),
            "{name}: level-first must never climb back to a level it left"
        );
        for level in 0..LEVELS {
            for region in 0..2 {
                assert!(
                    visits.contains(&(level, region)),
                    "{name}: level {level} must cut region {region}"
                );
            }
        }

        assert!(retracts(&by_area) > 0, "{name}: the fixture must retract");
        assert_eq!(
            retracts(&by_area),
            retracts(&global),
            "{name}: the retract count must not change"
        );
        assert_eq!(
            sorted_cuts(&by_area),
            sorted_cuts(&global),
            "{name}: both orders must hold the same cuts"
        );
        assert!(
            global_passes < by_area_passes,
            "{name}: level-first must have fewer depth passes ({global_passes} vs {by_area_passes})"
        );
    }
}

/// The fed cuts that stand on a tab top (`-DEPTH + TAB_H`), with their
/// index in the toolpath. An entry ramp can pass through that Z, so only a
/// `FinishingCut` counts.
fn tab_top_moves(moves: &[Move]) -> Vec<usize> {
    moves
        .iter()
        .enumerate()
        .filter(|(_, m)| {
            matches!(m.move_type, MoveType::Linear { .. })
                && m.intent == MoveIntent::FinishingCut
                && (m.target.z - (-DEPTH + TAB_H)).abs() < 1e-6
        })
        .map(|(i, _)| i)
        .collect()
}

/// The ladder levels of the pass that holds move `i`. A pass runs from the
/// move after one retract to the next retract.
fn levels_of_pass(moves: &[Move], i: usize) -> Vec<usize> {
    let start = moves[..i]
        .iter()
        .rposition(|m| m.intent == MoveIntent::Retract)
        .map_or(0, |r| r + 1);
    let end = moves[i..]
        .iter()
        .position(|m| m.intent == MoveIntent::Retract)
        .map_or(moves.len(), |r| i + r);
    let mut levels: Vec<usize> = ladder_visits(&moves[start..end])
        .into_iter()
        .map(|(level, _)| level)
        .collect();
    levels.dedup();
    levels
}

/// Claim 6: Profile tabs land only on the levels at the final Z, in both
/// orders. Each tab-top cut lies in a pass whose ladder cuts are all at
/// the final Z. A spring pass repeats the final Z, so it carries the tabs
/// too: one spring pass doubles the tab-top cuts (a spring pass without
/// tabs would cut the tabs away).
#[test]
fn profile_tabs_land_only_on_the_final_z_in_both_orders() {
    let final_level = LEVELS - 1;
    let mut counts = Vec::new();
    for finishing_passes in [0, 1] {
        for ordering in [RegionOrdering::ByArea, RegionOrdering::Global] {
            let moves = generated(profile_op(finishing_passes, ordering)).moves;
            let tabs = tab_top_moves(&moves);
            assert!(
                !tabs.is_empty(),
                "{ordering:?}, spring {finishing_passes}: the fixture must make tabs"
            );
            for &i in &tabs {
                assert_eq!(
                    levels_of_pass(&moves, i),
                    vec![final_level],
                    "{ordering:?}, spring {finishing_passes}: the tab cut {i} must lie \
                     in a pass at the final Z"
                );
            }
            eprintln!(
                "{ordering:?}, spring {finishing_passes}: {} tab-top cuts",
                tabs.len()
            );
            counts.push(tabs.len());
        }
    }
    // [ByArea, Global] without a spring pass, then with one.
    assert_eq!(counts[0], counts[1], "both orders make the same tabs");
    assert_eq!(counts[2], counts[3], "both orders make the same tabs");
    assert_eq!(
        counts[2],
        2 * counts[0],
        "the spring pass must carry the tabs of the final level"
    );
}

/// Claim 7: Trace keeps its own semantic annotation on the runner (the
/// `LayeredOp::annotate` hook): ONE region over the two outlines, and the
/// depth runs as chains inside it. The default annotation gives a region
/// per run and no chain. `narrate_regions_closed_c8` pins the same rule on
/// one outline.
#[test]
fn trace_keeps_its_one_region_and_its_chains() {
    let mut s = session(trace_op(RegionOrdering::ByArea));
    generate(&mut s, 0);
    let trace = s
        .get_result(0)
        .expect("a result")
        .semantic_trace
        .clone()
        .expect("the session attaches a semantic trace");
    let count = |kind: ToolpathSemanticKind| trace.items.iter().filter(|i| i.kind == kind).count();
    assert_eq!(count(ToolpathSemanticKind::Region), 1, "one trace region");
    assert!(
        count(ToolpathSemanticKind::Chain) > 0,
        "the depth runs must be chains"
    );
}
