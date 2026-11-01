//! D65 (`planning/adaptive_research_2026-10-23/phase3/d65/`): Geodesic
//! Adaptive cuts a step over the pass ceiling that reads at most a full slot
//! at a capped feed, where it used to lift and re-enter.
//!
//! The fixture is the shelf pocket of the C12 judge
//! (`geodesic_adaptive_keeps_the_research_geometry_s7_4`): a pocket with a
//! round island and a raised shelf, one level, the 6 mm flat end mill at
//! stepover 2. On `7ee3d3226` the plan re-entered along the shelf's edge
//! where a step read over the ceiling (the operator's ledge row on Corne).
//!
//! 1. The plan makes fewer entries than the base pin.
//! 2. Every cutting move fed below the op feed carries `Move::feed_cap`, and
//!    the cap is its feed (the cap rule: a lowered feed is a capped feed).

#![expect(
    clippy::expect_used,
    clippy::print_stderr,
    reason = "test code: an expect is how a test reports a failure; the test prints its measurements for the reader"
)]

mod common;

use common::meshes::height_field;
use rs_cam_core::adaptive3d::{
    Adaptive3dDepth, Adaptive3dGeometry, Adaptive3dLinking, Adaptive3dParams, ClearingStrategy3d,
    EntryStyle3d, KeepDownRoute, RegionOrdering, StepSplit,
    adaptive_3d_toolpath_output_with_cancel,
};
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::tool::FlatEndmill;
use rs_cam_core::toolpath::{MoveIntent, Toolpath};

// The C12 judge's fixture, as in `geodesic_adaptive_keeps_the_research_geometry_s7_4`.
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
const SHELF_Z: f64 = 6.5;
const FEED: f64 = 1000.0;

/// The entries of the shelf fixture's plan on `7ee3d3226`, before D65
/// (measured: 46; this tree plans 18).
const BASE_ENTRIES: usize = 46;

fn shelf() -> TriangleMesh {
    height_field(MESH_HALF, MESH_STEP, |x, y| {
        if x.hypot(y) <= ISLAND_RADIUS {
            TOP_Z
        } else if (12.0..=30.0).contains(&x) && y.abs() <= 12.0 {
            SHELF_Z
        } else if x.abs() <= POCKET_HALF_X && y.abs() <= POCKET_HALF_Y {
            FLOOR_Z
        } else {
            TOP_Z
        }
    })
}

fn params() -> Adaptive3dParams {
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
            depth_per_pass: TOP_Z - FLOOR_Z,
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
        },
        feed_rate: FEED,
        plunge_rate: 500.0,
        safe_z: 15.0,
        entry_style: EntryStyle3d::Plunge,
        initial_stock: None,
        clearing_strategy: ClearingStrategy3d::GeodesicAdaptive,
        trochoid_cap_mult: 1.6,
        engagement_measure: rs_cam_core::adaptive::EngagementMeasure::DiskArea,
    }
}

fn plan() -> Toolpath {
    let mesh = shelf();
    let index = SpatialIndex::build(&mesh, 10.0);
    let cutter = FlatEndmill::new(2.0 * TOOL_RADIUS, 25.0);
    adaptive_3d_toolpath_output_with_cancel(&mesh, &index, &cutter, &params(), &|| false, None)
        .expect("the plan is not cancelled")
        .toolpath
}

/// The entries: each run of `EntryPlunge` moves that follows another move.
fn entries(tp: &Toolpath) -> usize {
    tp.moves
        .windows(2)
        .filter(|w| {
            matches!(w, [a, b] if b.intent == MoveIntent::EntryPlunge && a.intent != MoveIntent::EntryPlunge)
        })
        .count()
}

#[test]
fn a_geodesic_step_over_the_ceiling_is_cut_capped() {
    let tp = plan();
    let n = entries(&tp);
    let lowered: Vec<(usize, f64, Option<f64>)> = tp
        .moves
        .iter()
        .enumerate()
        .filter(|(_, m)| m.intent == MoveIntent::ClearingCut)
        .filter_map(|(i, m)| m.move_type.feed_rate().map(|f| (i, f, m.feed_cap)))
        .filter(|(_, f, _)| *f < FEED)
        .collect();
    eprintln!(
        "shelf: {} moves, {n} entries (base {BASE_ENTRIES}), {} cutting moves fed below the op feed",
        tp.moves.len(),
        lowered.len()
    );
    assert!(
        n < BASE_ENTRIES,
        "the plan makes {n} entries, not fewer than the base {BASE_ENTRIES}"
    );
    let uncapped: Vec<_> = lowered
        .iter()
        .filter(|(_, f, cap)| cap.is_none_or(|c| (c - f).abs() > 1e-9))
        .collect();
    assert!(
        uncapped.is_empty(),
        "cutting moves fed below the op feed without their cap: {uncapped:?}"
    );
}
