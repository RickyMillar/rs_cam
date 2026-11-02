//! D44 (`planning/adaptive_research_2026-10-23/P3_C12_derived_rest_regions.md`;
//! integration step 7.5, ported from the research branch 453d52571 onto the
//! product arm): a Geodesic Adaptive 3D Rough states the regions it leaves
//! to a rest pass (`Produced::rest_regions`), so that a following op with a
//! `DerivedRestRegions` boundary plans only there.
//!
//! The planner states them on every Geodesic Adaptive run; the runner keeps them
//! only on demand (`compute/execute/volumetric.rs`), which the session
//! sentry of the job pair checks. Here, the planner alone:
//! 1. a narrow channel the field cannot plan is left: the union covers it;
//! 2. a stepped pocket: the wide pocket at the first level is cut, the
//!    narrow channel below it is left. The union, made of the lower
//!    level's piece, covers the channel's XY also at the first level,
//!    where the arm records no piece (the Corne edge band at Z 17);
//! 3. a pocket the arm plans everywhere leaves nothing: `None`.

#![expect(
    clippy::expect_used,
    clippy::print_stderr,
    reason = "test code: a panic is how a test reports a failure; the test prints its measurements for the reader"
)]

mod common;

use common::meshes::height_field;

use rs_cam_core::adaptive3d::{
    Adaptive3dDepth, Adaptive3dGeometry, Adaptive3dLinking, Adaptive3dParams, ClearingStrategy3d,
    EntryStyle3d, RegionOrdering, adaptive_3d_toolpath_output_with_cancel,
};
use rs_cam_core::geo::P2;
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::tool::FlatEndmill;

const TOOL_RADIUS: f64 = 3.0;
const STEPOVER: f64 = 2.0;
const TOLERANCE: f64 = 0.1;
const DEPTH_PER_PASS: f64 = 3.0;
const STOCK_TO_LEAVE: f64 = 0.5;
const TOP_Z: f64 = 10.0;
const FLOOR_Z: f64 = 4.0;
const MESH_HALF: f64 = 40.0;
const MESH_STEP: f64 = 0.5;
/// The channel: about one stepover of tool-centre width, so the field's
/// seed (a centre clearance of R) does not fit (as the C12 channel).
const CHANNEL_HALF_LENGTH: f64 = 30.0;
const CHANNEL_HALF_WIDTH: f64 = 4.8;
/// The wide shallow pocket of the stepped fixture: its floor lies above the
/// second level, so only the first level cuts it.
const WIDE_HALF_X: f64 = 34.0;
const WIDE_HALF_Y: f64 = 24.0;
const WIDE_FLOOR_Z: f64 = 7.0;
/// The F-038 filter at the op default (`Adaptive3dConfig`); the forecast
/// reads 0 on the channel and the arm leaves it.
const MIN_REGION_CUT_LENGTH_MM: f64 = 15.0;

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
            depth_per_pass: DEPTH_PER_PASS,
            stock_to_leave: STOCK_TO_LEAVE,
            stock_top_z: TOP_Z,
            z_floor: None,
            detect_flat_areas: false,
            rest_top_z: None,
        },
        linking: Adaptive3dLinking {
            region_ordering: RegionOrdering::Global,
            min_region_cut_length_mm: MIN_REGION_CUT_LENGTH_MM,
            max_stay_down_distance_mm: None,
            stay_down_clearance_mm: 0.5,
            step_split: None,
            keep_down_route: None,
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

/// The regions the planner states for `mesh`.
fn left_regions(mesh: &TriangleMesh) -> Option<Vec<Polygon2>> {
    let index = SpatialIndex::build(mesh, 10.0);
    let cutter = FlatEndmill::new(2.0 * TOOL_RADIUS, 25.0);
    let out =
        adaptive_3d_toolpath_output_with_cancel(mesh, &index, &cutter, &params(), &|| false, None)
            .expect("the plan is not cancelled");
    out.produced.rest_regions.map(|r| r.as_ref().clone())
}

fn in_union(regions: &[Polygon2], p: P2) -> bool {
    regions.iter().any(|r| r.contains_point(&p))
}

/// The points of the channel's tool-centre band: its centre line and the
/// lines half a tool radius to each side, every 2 mm along it.
fn channel_points() -> Vec<P2> {
    let mut pts = Vec::new();
    let mut x = -CHANNEL_HALF_LENGTH + TOOL_RADIUS;
    while x <= CHANNEL_HALF_LENGTH - TOOL_RADIUS {
        for y in [-0.5 * TOOL_RADIUS, 0.0, 0.5 * TOOL_RADIUS] {
            pts.push(P2::new(x, y));
        }
        x += 2.0;
    }
    pts
}

fn in_channel(x: f64, y: f64) -> bool {
    x.abs() <= CHANNEL_HALF_LENGTH && y.abs() <= CHANNEL_HALF_WIDTH
}

#[test]
fn a_rough_publishes_the_regions_it_leaves() {
    // 1. The channel alone.
    let channel = height_field(MESH_HALF, MESH_STEP, |x, y| {
        if in_channel(x, y) { FLOOR_Z } else { TOP_Z }
    });
    let regions = left_regions(&channel).expect("the arm leaves the channel and states it");
    let area: f64 = regions.iter().map(|r| r.area().abs()).sum();
    eprintln!("channel: {} regions, {area:.1} mm2", regions.len());
    let missed: Vec<P2> = channel_points()
        .into_iter()
        .filter(|&p| !in_union(&regions, p))
        .collect();
    assert!(
        missed.is_empty(),
        "channel points outside the stated regions: {missed:?}"
    );
    // The union is the channel, not the board: a point well outside it is
    // not in it.
    assert!(
        !in_union(
            &regions,
            P2::new(0.0, CHANNEL_HALF_WIDTH + 2.0 * TOOL_RADIUS)
        ),
        "the stated regions reach far outside the channel"
    );

    // 2. The stepped pocket: the wide pocket (cut at the first level) with
    // the channel below it (left at the second level).
    let stepped = height_field(MESH_HALF, MESH_STEP, |x, y| {
        if in_channel(x, y) {
            FLOOR_Z
        } else if x.abs() <= WIDE_HALF_X && y.abs() <= WIDE_HALF_Y {
            WIDE_FLOOR_Z
        } else {
            TOP_Z
        }
    });
    let regions = left_regions(&stepped).expect("the arm leaves the channel below the wide pocket");
    let area: f64 = regions.iter().map(|r| r.area().abs()).sum();
    eprintln!("stepped: {} regions, {area:.1} mm2", regions.len());
    let missed: Vec<P2> = channel_points()
        .into_iter()
        .filter(|&p| !in_union(&regions, p))
        .collect();
    assert!(
        missed.is_empty(),
        "channel points (also the first level's band) outside the stated regions: {missed:?}"
    );
    // The wide pocket the arm cut is not stated.
    assert!(
        !in_union(
            &regions,
            P2::new(
                0.0,
                0.5 * (WIDE_HALF_Y + CHANNEL_HALF_WIDTH + 2.0 * TOOL_RADIUS)
            )
        ),
        "the stated regions hold the wide pocket the arm cut"
    );

    // 3. A pocket the arm plans everywhere states nothing.
    let pocket = height_field(MESH_HALF, MESH_STEP, |x, y| {
        if x.abs() <= WIDE_HALF_X && y.abs() <= WIDE_HALF_Y {
            FLOOR_Z
        } else {
            TOP_Z
        }
    });
    let regions = left_regions(&pocket);
    eprintln!(
        "pocket: {:?}",
        regions
            .as_ref()
            .map(|r| r.iter().map(|p| p.area().abs()).sum::<f64>())
    );
    assert!(regions.is_none(), "a fully planned pocket states regions");
}
