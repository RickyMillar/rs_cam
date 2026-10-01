//! S4 of `planning/by_area_merge_tree_2026-09-25/PLAN.md` §8.
//!
//! The By Area job order (PLAN §4): the rest first, then the valleys
//! nearest next, and each job cuts all its levels before the next job.
//!
//! ## The fixture
//!
//! `common::meshes::three_valley_plate`: valleys A (west, floor Z 0),
//! B (centre, floor Z 2) and C (east, floor Z 1) under a plate at Z 6, stock
//! top Z 10. Deepest first is A, C, B. No nearest-next order gives A, C, B:
//! from A the nearest valley is B, and from C it is B too. So the test
//! fails when the planner orders the valleys deepest first (the old
//! `PocketTree::cut_order`).
//!
//! ## The assertions
//!
//! For each clearing strategy:
//!
//! - The `RegionStart` sequence is the rest, then the three valleys.
//! - Each valley is the remaining valley whose anchor is nearest to the
//!   tool at the end of the previous job (the last move before its
//!   `RegionStart`). A tie goes to the lower floor.
//! - The levels of each job are contiguous: every `RegionZLevel` between
//!   two `RegionStart` markers names the job of the first, and its Z values
//!   go down.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;
use common::meshes::{THREE_VALLEY_TOP_Z, three_valley_plate};

use rs_cam_core::adaptive3d::{
    Adaptive3dDepth, Adaptive3dGeometry, Adaptive3dLinking, Adaptive3dParams,
    Adaptive3dRuntimeEvent, AreaRegionKind, ClearingStrategy3d, EntryStyle3d, RegionOrdering,
    adaptive_3d_toolpath_output_with_cancel,
};
use rs_cam_core::mesh::SpatialIndex;
use rs_cam_core::tool::FlatEndmill;

const TOOL_RADIUS: f64 = 3.0;

fn params(strategy: ClearingStrategy3d) -> Adaptive3dParams {
    Adaptive3dParams {
        entry_clearance_mm: 0.5,
        ramp_feed_rate: None,
        geometry: Adaptive3dGeometry {
            tool_radius: TOOL_RADIUS,
            envelope_radius: TOOL_RADIUS,
            stepover: 1.5,
            tolerance: 0.5,
            segment_merge_tolerance: None,
            min_cutting_radius: 0.0,
            boundary: None,
            world_stock_xy_bbox: None,
            centre_boundary: Vec::new(),
        },
        depth: Adaptive3dDepth {
            depth_per_pass: 3.0,
            stock_to_leave: 0.5,
            stock_top_z: THREE_VALLEY_TOP_Z,
            z_floor: None,
            detect_flat_areas: false,
        },
        linking: Adaptive3dLinking {
            region_ordering: RegionOrdering::ByArea,
            min_region_cut_length_mm: 0.0,
            max_stay_down_distance_mm: None,
            stay_down_clearance_mm: 0.5,
        },
        feed_rate: 1000.0,
        plunge_rate: 500.0,
        safe_z: 15.0,
        entry_style: EntryStyle3d::Plunge,
        initial_stock: None,
        clearing_strategy: strategy,
        trochoid_cap_mult: 1.6,
        engagement_measure: rs_cam_core::adaptive::EngagementMeasure::DiskArea,
        z_blend: false,
    }
}

#[test]
fn by_area_runs_the_rest_then_the_valleys_nearest_next() {
    let mesh = three_valley_plate();
    let index = SpatialIndex::build(&mesh, 10.0);
    let cutter = FlatEndmill::new(TOOL_RADIUS * 2.0, 25.0);

    for strategy in [
        ClearingStrategy3d::ContourParallel,
        ClearingStrategy3d::Adaptive,
        ClearingStrategy3d::ContourSpiral,
        ClearingStrategy3d::AgentSearch,
    ] {
        let out = adaptive_3d_toolpath_output_with_cancel(
            &mesh,
            &index,
            &cutter,
            &params(strategy),
            &|| false,
            None,
        )
        .unwrap();
        let map = out.area_regions.as_ref().expect("By Area records its jobs");
        assert_eq!(
            map.regions.len(),
            4,
            "{strategy:?}: the rest and three valleys"
        );

        let mut events: Vec<_> = out.annotations.iter().collect();
        events.sort_by_key(|a| a.move_index);

        // The RegionStart sequence and where the tool stands at each one.
        let starts: Vec<(usize, usize)> = events
            .iter()
            .filter_map(|a| match a.event {
                Adaptive3dRuntimeEvent::RegionStart { region_index, .. } => {
                    Some((region_index, a.move_index))
                }
                _ => None,
            })
            .collect();
        let orders: Vec<usize> = starts.iter().map(|s| s.0).collect();
        assert_eq!(orders, vec![1, 2, 3, 4], "{strategy:?}: one start per job");
        assert_eq!(
            map.regions[0].kind,
            AreaRegionKind::Rest,
            "{strategy:?}: the rest runs first"
        );

        // Nearest next: each valley is the nearest remaining valley to the
        // tool at the end of the previous job.
        let mut left: Vec<usize> = (1..4).collect();
        for &(order, move_index) in starts.iter().skip(1) {
            assert!(move_index > 0, "{strategy:?}: the rest cut nothing");
            let tool = out.toolpath.moves[move_index - 1].target;
            let dist = |k: usize| {
                let a = map.regions[k].anchor_xy;
                (a[0] - tool.x).powi(2) + (a[1] - tool.y).powi(2)
            };
            let expected = *left
                .iter()
                .min_by(|&&a, &&b| {
                    dist(a).total_cmp(&dist(b)).then(
                        map.regions[a].surface_z_range[0]
                            .total_cmp(&map.regions[b].surface_z_range[0]),
                    )
                })
                .unwrap();
            assert_eq!(
                order - 1,
                expected,
                "{strategy:?}: from the tool at ({:.2}, {:.2}) the nearest valley is order {}, \
                 not {order}",
                tool.x,
                tool.y,
                expected + 1
            );
            left.retain(|&k| k != expected);
        }

        // Non-vacuity: the order is not deepest first (A, C, B).
        let floors: Vec<f64> = map.regions[1..]
            .iter()
            .map(|r| r.surface_z_range[0])
            .collect();
        let mut deepest = floors.clone();
        deepest.sort_by(f64::total_cmp);
        assert_ne!(
            floors, deepest,
            "{strategy:?}: the valleys went deepest first"
        );

        // The levels of each job are contiguous and go down.
        let mut current = 0usize;
        let mut last_z = f64::INFINITY;
        let mut seen_levels = [0usize; 5];
        for a in &events {
            match a.event {
                Adaptive3dRuntimeEvent::RegionStart { region_index, .. } => {
                    current = region_index;
                    last_z = f64::INFINITY;
                }
                Adaptive3dRuntimeEvent::RegionZLevel {
                    region_index,
                    z_level,
                    ..
                } => {
                    assert_eq!(
                        region_index, current,
                        "{strategy:?}: a level of job {region_index} inside job {current}"
                    );
                    assert!(
                        z_level < last_z,
                        "{strategy:?}: job {current} level Z {z_level} after Z {last_z}"
                    );
                    last_z = z_level;
                    seen_levels[region_index] += 1;
                }
                Adaptive3dRuntimeEvent::GlobalZLevel { .. } => {
                    panic!("{strategy:?}: a Global level in a By Area run")
                }
                _ => {}
            }
        }
        for (k, region) in map.regions.iter().enumerate() {
            assert!(
                seen_levels[k + 1] > 0,
                "{strategy:?}: job {} has no level marker",
                k + 1
            );
            assert!(region.level_count > 0);
        }
    }
}
