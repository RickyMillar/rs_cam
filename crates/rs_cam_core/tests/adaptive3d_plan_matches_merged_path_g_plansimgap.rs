//! G-PLANSIMGAP — the 3D Rough planner stamps the path that ships.
//!
//! The adaptive3d planner keeps its own dexel stock. It reads each entry's
//! rapid floor and helix start from that stock, and it stamps each cut into
//! it before it plans the next. The invariant (`stamp_emitted_segment`) is
//! that the planner stamps the path it emits. The segment merge dressup
//! (`dressup::condition::merge_linear_runs`, default tolerance 0.3 mm) ran
//! AFTER the planner and moved the cut points up to 0.3 mm. On a slope a
//! lateral move of a cut is a large vertical move of the material it
//! leaves. On rivmap100 (2026-09-25) the simulated stock stood up to
//! 2.34 mm above the planner stock in 4016 cells (> 0.25 mm), and two entry
//! rapids went into material (min clearance -2.19 mm, where the planner
//! intends 0.5 mm). With the merge off, every in-stock entry was exactly
//! 0.500 mm clear. Evidence:
//! `planning/entry_stock_awareness_2026-09-24/plansimgap/`.
//!
//! The test replays two paths on a fresh dexel stock: the path the planner
//! emits (and so stamps), and that path after the production dressup
//! pipeline of a 3D Rough with the segment merge ON at its default
//! tolerance (the operator's setting; do not turn it off to pass). The
//! shipped path must not leave material that the planner believes it
//! removed. The planner gets the merge tolerance as the session gives it
//! (`ExecutionContext::segment_merge_tolerance`).
//!
//! Red before the fix (2026-09-26, the planner had no merge tolerance and
//! the dressup merged after it): 1382 cells over 0.25 mm, max 2.812 mm.
//!
//! The fix (G-PLANSIMGAP): the planner applies the merge. It simplifies
//! each cut at the larger of the operation tolerance and the merge
//! tolerance before it stamps it, and the dressup does not merge a 3D Rough
//! again. The second test holds that the merge still acts: the operator's
//! setting still cuts the number of clearing-cut moves.
//!
//! The cost, measured on this fixture (2026-09-26): clearing-cut moves
//! 2141 with no merge, 1265 with the old merge after the planner, 1275 with
//! the planner merge. The helix entries grow from 73110 moves (21065 mm) to
//! 77487 moves (22296 mm): the planner now sees the material the merged
//! cuts leave, so a helix starts over the real top, not under it. The fed
//! length is 29093 mm against 27708 mm with the old merge (+5.0 %).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

mod common;

use rs_cam_core::adaptive3d::{
    Adaptive3dDepth, Adaptive3dGeometry, Adaptive3dLinking, Adaptive3dParams, ClearingStrategy3d,
    EntryStyle3d, RegionOrdering, adaptive_3d_toolpath,
};
use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::compute::config::{DressupConfig, SegmentMergeParams};
use rs_cam_core::compute::execute::{DressupContext, apply_dressups};
use rs_cam_core::dexel_stock::{StockCutDirection, TriDexelStock};
use rs_cam_core::geo::{BoundingBox3, P3};
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::stock::radial_profile::{LUT_SAMPLES, RadialProfileLUT};
use rs_cam_core::tool::{FlatEndmill, MillingCutter};
use rs_cam_core::toolpath::{MoveIntent, MoveType, Toolpath};
use rs_cam_core::trace::toolpath_spans::AnnotatedToolpath;
use rs_cam_core::trace::transform_provenance::ReconcileSet;

/// Terrain half-width.
const HALF: f64 = 20.0;
const STOCK_TOP_Z: f64 = 16.0;
const SAFE_Z: f64 = 21.0;
/// The replay grid.
const CELL: f64 = 0.25;
/// Two replays of one path agree to the dexel read; a cell where the
/// shipped path leaves more than this over the planner's path is material
/// the planner believes it removed.
const GAP_TOL_MM: f64 = 0.25;

/// Rolling terrain under 16 mm of stock: hills with steep flanks, where a
/// lateral move of a cut is a large vertical move of the material it leaves.
fn terrain_mesh() -> TriangleMesh {
    common::meshes::height_field(HALF, 0.5, |x, y| {
        2.0 + 11.0 * ((x / 5.0).sin() * (y / 5.0).sin()).max(0.0)
    })
}

/// The 3D Rough dressups with the segment merge ON at its default
/// tolerance. Arc fitting is off, so the replay can stamp every fed move as
/// a line (arc fitting has no part in the gap: the `arc_off` arm of the
/// evidence has the same gap as the base).
fn rough_dressups() -> DressupConfig {
    let mut cfg = DressupConfig::for_op(OperationType::Adaptive3d);
    cfg.arc_fitting = None;
    cfg.segment_merge = Some(SegmentMergeParams::default());
    cfg
}

/// The planner parameters. `merge` is what the session passes from the
/// toolpath's dressups: the segment merge tolerance when the merge is on.
fn params(merge: Option<f64>) -> Adaptive3dParams {
    Adaptive3dParams {
        entry_clearance_mm: 0.5,
        ramp_feed_rate: None,
        geometry: Adaptive3dGeometry {
            tool_radius: 3.0,
            envelope_radius: 3.0,
            stepover: 2.4,
            tolerance: 0.1,
            segment_merge_tolerance: merge,
            min_cutting_radius: 0.0,
            boundary: None,
            world_stock_xy_bbox: None,
        },
        depth: Adaptive3dDepth {
            depth_per_pass: 4.0,
            stock_to_leave: 0.5,
            stock_top_z: STOCK_TOP_Z,
            z_floor: None,
            detect_flat_areas: false,
        },
        linking: Adaptive3dLinking {
            region_ordering: RegionOrdering::Global,
            min_region_cut_length_mm: 0.0,
            max_stay_down_distance_mm: None,
            stay_down_clearance_mm: 0.5,
        },
        trochoid_cap_mult: 1.6,
        engagement_measure: rs_cam_core::adaptive::EngagementMeasure::DiskArea,
        feed_rate: 2400.0,
        plunge_rate: 500.0,
        safe_z: SAFE_Z,
        entry_style: EntryStyle3d::Helix {
            radius: 1.8,
            pitch: 1.0,
        },
        initial_stock: None,
        clearing_strategy: ClearingStrategy3d::ContourParallel,
        z_blend: false,
    }
}

/// The production dressup pipeline of a 3D Rough on the planner's path.
fn ship(planned: &Toolpath, cfg: &DressupConfig) -> Toolpath {
    apply_dressups(
        AnnotatedToolpath::new(planned.clone()),
        DressupContext {
            cfg,
            nominal_feed_rate: 2400.0,
            plunge_rate_mm_min: Some(500.0),
            ramp_feed_rate_mm_min: None,
            tool_diameter: 6.0,
            safe_z: SAFE_Z,
            stock_top: STOCK_TOP_Z,
            prior_stock: None,
            feed_opt_stock: None,
            cutter: None,
            entry_surface: None,
            transform_capabilities: OperationType::Adaptive3d.transform_capabilities(),
            debug_ctx: None,
            semantic_ctx: None,
        },
        &mut ReconcileSet::empty(),
    )
    .toolpath
}

/// Replay `tp` on a prism stock and return the top of every cell.
fn replay_tops(tp: &Toolpath, tool: &dyn MillingCutter) -> Vec<f64> {
    let h = HALF + 2.0;
    let mut stock = TriDexelStock::from_bounds(
        &BoundingBox3 {
            min: P3::new(-h, -h, 0.0),
            max: P3::new(h, h, STOCK_TOP_Z),
        },
        CELL,
    );
    let lut = RadialProfileLUT::from_cutter(tool, LUT_SAMPLES);
    for i in 1..tp.moves.len() {
        assert!(
            matches!(
                tp.moves[i].move_type,
                MoveType::Rapid | MoveType::Linear { .. }
            ),
            "move {i} is an arc; the replay stamps lines only"
        );
        stock.stamp_linear_segment(
            &lut,
            tool.radius(),
            tp.moves[i - 1].target,
            tp.moves[i].target,
            StockCutDirection::FromTop,
        );
    }
    let g = &stock.z_grid;
    let mut tops = Vec::with_capacity(g.rows * g.cols);
    for r in 0..g.rows {
        for c in 0..g.cols {
            tops.push(g.top_z_at(r, c).map_or(0.0, f64::from));
        }
    }
    tops
}

fn fed_moves(tp: &Toolpath) -> usize {
    tp.moves
        .iter()
        .filter(|m| !matches!(m.move_type, MoveType::Rapid))
        .count()
}

/// The fed clearing cuts: the moves the segment merge acts on. (It keeps
/// every point of a helix or ramp entry.)
fn clearing_cuts(tp: &Toolpath) -> usize {
    tp.moves
        .iter()
        .filter(|m| !matches!(m.move_type, MoveType::Rapid) && m.intent == MoveIntent::ClearingCut)
        .count()
}

/// The rule: the shipped 3D Rough leaves no material that the planner
/// stamped as removed.
#[test]
fn the_shipped_rough_leaves_no_material_the_planner_removed() {
    let mesh = terrain_mesh();
    let index = SpatialIndex::build(&mesh, 8.0);
    let tool = FlatEndmill::new(6.0, 25.0);
    let cfg = rough_dressups();
    let merge = cfg.segment_merge.map(|m| m.tolerance);
    let planned = adaptive_3d_toolpath(&mesh, &index, &tool, &params(merge));
    let shipped = ship(&planned, &cfg);

    let plan_tops = replay_tops(&planned, &tool);
    let ship_tops = replay_tops(&shipped, &tool);
    let gaps: Vec<f64> = plan_tops
        .iter()
        .zip(&ship_tops)
        .map(|(p, s)| s - p)
        .filter(|d| *d > GAP_TOL_MM)
        .collect();
    let max_gap = gaps.iter().copied().fold(0.0, f64::max);
    eprintln!(
        "planned {} moves ({} fed), shipped {} moves ({} fed); cells over {GAP_TOL_MM} mm: {}, max {max_gap:.3} mm",
        planned.moves.len(),
        fed_moves(&planned),
        shipped.moves.len(),
        fed_moves(&shipped),
        gaps.len()
    );
    assert!(
        gaps.is_empty(),
        "the shipped path leaves material the planner stamped as removed: {} cells over {GAP_TOL_MM} mm, max {max_gap:.3} mm",
        gaps.len()
    );
}

/// The operator's segment merge still acts on a 3D Rough: the planner that
/// applies it emits fewer clearing-cut moves than the planner without it
/// (2026-09-26: 1275 against 2141), and the session passes the tolerance
/// through to the planner.
#[test]
fn the_segment_merge_still_cuts_the_move_count() {
    let mesh = terrain_mesh();
    let index = SpatialIndex::build(&mesh, 8.0);
    let tool = FlatEndmill::new(6.0, 25.0);
    let merge = SegmentMergeParams::default().tolerance;
    let merged = clearing_cuts(&adaptive_3d_toolpath(
        &mesh,
        &index,
        &tool,
        &params(Some(merge)),
    ));
    let unmerged = clearing_cuts(&adaptive_3d_toolpath(&mesh, &index, &tool, &params(None)));
    eprintln!("planner clearing cuts: merge {merge} mm {merged}, no merge {unmerged}");
    assert!(
        merged < unmerged,
        "the merge at {merge} mm no longer cuts the clearing moves: {merged} vs {unmerged}"
    );

    // End to end: the session reads the toolpath's dressups and passes the
    // tolerance to the planner.
    let session_fed = |segment_merge: Option<SegmentMergeParams>| {
        use rs_cam_core::compute::catalog::OperationConfig;
        use rs_cam_core::compute::operation_configs::{
            Adaptive3dConfig, Adaptive3dEntryStyle, ClearingStrategy, RegionOrdering as CfgOrdering,
        };
        let cfg = Adaptive3dConfig {
            stepover: 2.4,
            depth_per_pass: 4.0,
            stock_to_leave_axial: 0.5,
            entry_style: Adaptive3dEntryStyle::Helix,
            region_ordering: CfgOrdering::Global,
            clearing_strategy: ClearingStrategy::ContourParallel,
            ..Adaptive3dConfig::default()
        };
        let mut session = common::session::single_op_session_with(
            common::session::stock_over(HALF, STOCK_TOP_Z),
            common::make_endmill_6mm(),
            common::session::mesh_model(terrain_mesh(), "terrain"),
            "3D Rough",
            OperationConfig::Adaptive3d(cfg),
            |tc| {
                tc.dressups.arc_fitting = None;
                tc.dressups.segment_merge = segment_merge;
            },
        );
        common::session::generate(&mut session, 0);
        clearing_cuts(session.get_result(0).expect("result").toolpath())
    };
    let on = session_fed(Some(SegmentMergeParams::default()));
    let off = session_fed(None);
    eprintln!("session clearing cuts: merge on {on}, merge off {off}");
    assert!(
        on < off,
        "the session's merge no longer cuts the clearing moves: {on} vs {off}"
    );
}
