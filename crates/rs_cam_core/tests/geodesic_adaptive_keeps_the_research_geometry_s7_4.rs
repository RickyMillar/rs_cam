//! Integration step 7.4 (`planning/adaptive_research_2026-10-23/integration/
//! step7_design.md` §5; lead conditions 1 and 3 of option B): Geodesic
//! Adaptive plans the research arm's geometry, move by move.
//!
//! The research arm is `ClearingStrategy3d::Geodesic` of the research branch
//! (607e52730) with the product preset, C35 plan A
//! (`C35_SPIRAL_PICK.pick_variants().0`), the stock-depth step split and
//! the routed keep-down travel at the cut feed. Both trees run the same
//! fixtures (the island pocket of the research C12 judge at one level and
//! at two, its shelf, and its straight channel) with these rules set
//! explicitly, so no default of either tree enters.
//!
//! The pins are the research branch's numbers: the move count and the
//! FNV-1a hash (`common::fingerprint::fnv1a_debug`) of each move's target,
//! move kind with the feed value left out, and intent. Only the feed may
//! differ: the research arm matched its turn records to moves by position
//! and fed them from its readout; the product writes each turn's cap on its
//! own cut from its own load reading (design part F,
//! `adaptive::geodesic::feed_caps`). The feed differences are in the step
//! 7.4 report. A pin that moves is a port defect: report it, do not
//! re-pin.
//!
//! D65 (`planning/adaptive_research_2026-10-23/phase3/d65/`, lead ruling
//! 2026-10-28): the product leaves the research geometry ON PURPOSE where a
//! step over the pass ceiling that reads at most a full slot is now cut at
//! a capped feed instead of a re-entry. The pins below are the D65
//! product's (the research numbers: 978, 2063, 1056, 0 moves). Every other
//! move-by-move difference is still a defect.

#![expect(
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]

mod common;

use common::fingerprint::fnv1a_debug;
use common::meshes::height_field;
use rs_cam_core::adaptive3d::{
    Adaptive3dDepth, Adaptive3dGeometry, Adaptive3dLinking, Adaptive3dParams, ClearingStrategy3d,
    EntryStyle3d, KeepDownRoute, RegionOrdering, StepSplit,
    adaptive_3d_toolpath_output_with_cancel,
};
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::tool::FlatEndmill;
use rs_cam_core::toolpath::Toolpath;

// The research C12 judge's fixture (`adaptive3d_geodesic_arm_c12.rs`).
const TOOL_RADIUS: f64 = 3.0;
const STEPOVER: f64 = 2.0;
const TOLERANCE: f64 = 0.1;
const STOCK_TO_LEAVE: f64 = 0.5;
const TOP_Z: f64 = 10.0;
const FLOOR_Z: f64 = 4.0;
const MESH_HALF: f64 = 40.0;
const MESH_STEP: f64 = 0.5;
const POCKET_HALF_X: f64 = 34.0;
const POCKET_HALF_Y: f64 = 24.0;
const ISLAND_RADIUS: f64 = 5.0;
/// The shelf of the research width-read test: x 12 to 30, y -12 to 12.
const SHELF_Z: f64 = 6.5;
/// The research channel (§13.3): half length, and the shipped smoothing
/// inset its width is built from.
const CHANNEL_HALF_LENGTH: f64 = 30.0;
const SIMPLIFY_INSET: f64 = 0.3;

fn pocket(shelf: bool) -> TriangleMesh {
    height_field(MESH_HALF, MESH_STEP, |x, y| {
        if x.hypot(y) <= ISLAND_RADIUS {
            TOP_Z
        } else if shelf && (12.0..=30.0).contains(&x) && y.abs() <= 12.0 {
            SHELF_Z
        } else if x.abs() <= POCKET_HALF_X && y.abs() <= POCKET_HALF_Y {
            FLOOR_Z
        } else {
            TOP_Z
        }
    })
}

fn channel() -> TriangleMesh {
    let cell = (TOOL_RADIUS / 6.0).max(TOLERANCE);
    let hw = (2.0 * TOOL_RADIUS + 2.0 * (SIMPLIFY_INSET + cell) + STEPOVER) / 2.0;
    height_field(MESH_HALF, MESH_STEP, |x, y| {
        if x.abs() <= CHANNEL_HALF_LENGTH && y.abs() <= hw {
            FLOOR_Z
        } else {
            TOP_Z
        }
    })
}

/// The research C12 judge's params, the arm's rules set explicitly.
fn params(depth_per_pass: f64) -> Adaptive3dParams {
    Adaptive3dParams {
        entry_clearance_mm: 0.5,
        ramp_feed_rate: None,
        geometry: Adaptive3dGeometry {
            tool_radius: TOOL_RADIUS,
            envelope_radius: TOOL_RADIUS,
            stepover: STEPOVER,
            tolerance: TOLERANCE,
            segment_merge_tolerance: None,
            min_cutting_radius: 0.0,
            boundary: None,
            world_stock_xy_bbox: None,
            centre_boundary: Vec::new(),
            gouge_probe: None,
        },
        depth: Adaptive3dDepth {
            depth_per_pass,
            stock_to_leave: STOCK_TO_LEAVE,
            stock_top_z: TOP_Z,
            z_floor: None,
            detect_flat_areas: false,
            rest_top_z: None,
        },
        linking: Adaptive3dLinking {
            region_ordering: RegionOrdering::Global,
            min_region_cut_length_mm: 0.0,
            max_stay_down_distance_mm: None,
            stay_down_clearance_mm: 0.5,
            step_split: Some(StepSplit::StockDepth),
            keep_down_route: Some(KeepDownRoute::CutFeed),
            level_contour: None,
            rest_region_filter: None,
        },
        feed_rate: 1000.0,
        plunge_rate: 500.0,
        safe_z: 15.0,
        entry_style: EntryStyle3d::Plunge,
        initial_stock: None,
        clearing_strategy: ClearingStrategy3d::GeodesicAdaptive,
        trochoid_cap_mult: 1.6,
        engagement_measure: rs_cam_core::adaptive::EngagementMeasure::DiskArea,
    }
}

fn plan(mesh: &TriangleMesh, depth_per_pass: f64) -> Toolpath {
    let index = SpatialIndex::build(mesh, 10.0);
    let cutter = FlatEndmill::new(2.0 * TOOL_RADIUS, 25.0);
    adaptive_3d_toolpath_output_with_cancel(
        mesh,
        &index,
        &cutter,
        &params(depth_per_pass),
        &|| false,
        None,
    )
    .expect("the plan is not cancelled")
    .toolpath
}

/// The move count and the hash of each move's target, move kind with the
/// feed value left out, and intent.
fn geometry(tp: &Toolpath) -> (usize, u64) {
    let fields: Vec<_> = tp
        .moves
        .iter()
        .map(|m| (m.target, m.move_type.with_feed_rate(0.0), m.intent))
        .collect();
    (tp.moves.len(), fnv1a_debug(&fields))
}

/// `(fixture, mesh, Depth/Pass)`.
fn fixtures() -> Vec<(&'static str, TriangleMesh, f64)> {
    vec![
        ("island pocket, one level", pocket(false), TOP_Z - FLOOR_Z),
        (
            "island pocket, two levels",
            pocket(false),
            (TOP_Z - FLOOR_Z) / 2.0,
        ),
        ("shelf, one level", pocket(true), TOP_Z - FLOOR_Z),
        ("channel, two levels", channel(), (TOP_Z - FLOOR_Z) / 2.0),
    ]
}

/// The geometry of each fixture, in [`fixtures`] order: the research
/// branch's, as D65 changed it (the capped steps; module doc). C3, the D55
/// port (G-AIRPECK): "island pocket, two levels" 1976 → 1941 moves; 16
/// `EntryPlunge` feeds and 19 `Retract` rapids go, every XY move is the
/// same.
const PINS: [(usize, u64); 4] = [
    (957, 5931675112365951456),
    (1941, 14901688868866573348),
    (1008, 6172482053443525791),
    (0, 675868731199239589),
];

#[test]
fn geodesic_adaptive_plans_the_research_geometry() {
    let got: Vec<(&str, (usize, u64))> = fixtures()
        .iter()
        .map(|(name, mesh, dpp)| (*name, geometry(&plan(mesh, *dpp))))
        .collect();
    for ((name, g), pin) in got.iter().zip(PINS) {
        assert_eq!(
            *g, pin,
            "{name}: the geometry left the research arm's; all: {got:?}"
        );
    }
}

/// Instrument for the step 7.4 feed-diff report: each move's feed and feed
/// cap, one line per move, per fixture, into
/// `<CARGO_TARGET_TMPDIR>/geodesic_feeds/<n>.txt`.
#[test]
#[ignore = "instrument: writes the per-move feeds for the step 7.4 feed-diff report"]
fn write_the_per_move_feeds() {
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("geodesic_feeds");
    std::fs::create_dir_all(&dir).expect("create the output dir");
    for (k, (_, mesh, dpp)) in fixtures().iter().enumerate() {
        let tp = plan(mesh, *dpp);
        let mut text = String::new();
        for (i, m) in tp.moves.iter().enumerate() {
            text.push_str(&format!(
                "{i} {:.4} {:.4} {:.4} {:?} {:?}\n",
                m.target.x,
                m.target.y,
                m.target.z,
                m.move_type.feed_rate(),
                m.feed_cap
            ));
        }
        std::fs::write(dir.join(format!("{k}.txt")), text).expect("write the feeds");
    }
}
