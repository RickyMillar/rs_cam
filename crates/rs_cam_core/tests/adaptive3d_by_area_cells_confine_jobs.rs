//! S2 of `planning/by_area_merge_tree_2026-09-25/PLAN.md` (WP1 form).
//!
//! By Area confines a job to the cells that it owns, not to the bounding
//! box of those cells (PLAN §3.2, RESULTS.md finding 3).
//!
//! ## The fixture
//!
//! `common::meshes::l_and_basin_plate`: an L-shaped pocket A and a pocket B
//! in the inside corner of the L. The XY box of A contains all of B. The
//! BFS detector finds two regions: region 1 is A, region 2 is B.
//!
//! ## The assertions
//!
//! For each clearing strategy:
//!
//! - Every clearing cut of a region has its tool centre on a cell of that
//!   region, or on a neighbour of such a cell. A marching-squares ring
//!   point sits on the edge between a material cell and an air cell, so
//!   the test also accepts the 8 neighbours.
//! - No clearing cut of region 1 has its centre on a cell of region 2.
//!   Under bbox confinement region 1 cut pocket B, and this failed.
//! - Region 2 cuts pocket B itself.
//!
//! The waterline cleanup after the last region has no mask (F9), so the
//! test reads only the moves between a `RegionStart` and the cleanup.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

mod common;
use common::meshes::{L_AND_BASIN_TOP_Z, l_and_basin_plate};

use rs_cam_core::adaptive3d::{
    Adaptive3dDepth, Adaptive3dGeometry, Adaptive3dLinking, Adaptive3dParams,
    Adaptive3dRuntimeEvent, AreaRegionMap, ClearingStrategy3d, EntryStyle3d, RegionOrdering,
    adaptive_3d_toolpath_output_with_cancel,
};
use rs_cam_core::mesh::SpatialIndex;
use rs_cam_core::tool::FlatEndmill;
use rs_cam_core::toolpath::MoveIntent;

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
        },
        depth: Adaptive3dDepth {
            depth_per_pass: 4.0,
            stock_to_leave: 0.5,
            stock_top_z: L_AND_BASIN_TOP_Z,
            z_floor: None,
            detect_flat_areas: false,
        },
        linking: Adaptive3dLinking {
            region_ordering: RegionOrdering::ByArea,
            min_region_cut_length_mm: 0.0,
            max_stay_down_distance_mm: Some(0.0),
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

/// The cell `(row, col)` under the world point `(x, y)`, or `None` off the
/// grid.
fn cell_of(map: &AreaRegionMap, x: f64, y: f64) -> Option<(usize, usize)> {
    let col = ((x - map.origin_x) / map.cell_mm).round();
    let row = ((y - map.origin_y) / map.cell_mm).round();
    if col < 0.0 || row < 0.0 || col as usize >= map.cols || row as usize >= map.rows {
        return None;
    }
    Some((row as usize, col as usize))
}

/// True when the cell or one of its 8 neighbours has label `order`.
fn near_label(map: &AreaRegionMap, row: usize, col: usize, order: u16) -> bool {
    (-1i64..=1).any(|dr| {
        (-1i64..=1).any(|dc| {
            let (r, c) = (row as i64 + dr, col as i64 + dc);
            r >= 0 && c >= 0 && map.label_at(r as usize, c as usize) == order
        })
    })
}

#[test]
fn by_area_jobs_cut_only_their_own_cells() {
    let mesh = l_and_basin_plate();
    let index = SpatialIndex::build(&mesh, 10.0);
    let cutter = FlatEndmill::new(TOOL_RADIUS * 2.0, 25.0);

    for strategy in [
        ClearingStrategy3d::ContourParallel,
        ClearingStrategy3d::Adaptive,
        ClearingStrategy3d::ContourSpiral,
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
        let map = out
            .area_regions
            .as_ref()
            .expect("By Area records its regions");
        assert_eq!(
            map.regions.len(),
            2,
            "{strategy:?}: two pockets are two regions"
        );

        // The job of each move: the region of the last RegionStart, and
        // none after the waterline cleanup.
        let n = out.toolpath.moves.len();
        let mut job_of = vec![0u16; n];
        let mut events: Vec<_> = out.annotations.iter().collect();
        events.sort_by_key(|a| a.move_index);
        let mut current = 0u16;
        let mut next = 0usize;
        for (i, slot) in job_of.iter_mut().enumerate() {
            while next < events.len() && events[next].move_index <= i {
                match events[next].event {
                    Adaptive3dRuntimeEvent::RegionStart { region_index, .. } => {
                        current = u16::try_from(region_index).unwrap();
                    }
                    Adaptive3dRuntimeEvent::WaterlineCleanup => current = 0,
                    _ => {}
                }
                next += 1;
            }
            *slot = current;
        }

        let mut stray = Vec::new();
        let mut region1_on_region2 = 0usize;
        let mut region2_cuts = 0usize;
        for (i, m) in out.toolpath.moves.iter().enumerate() {
            if m.intent != MoveIntent::ClearingCut || !m.move_type.is_cutting() {
                continue;
            }
            let job = job_of[i];
            if job == 0 {
                continue;
            }
            let Some((row, col)) = cell_of(map, m.target.x, m.target.y) else {
                stray.push((i, job, m.target.x, m.target.y));
                continue;
            };
            if !near_label(map, row, col, job) {
                stray.push((i, job, m.target.x, m.target.y));
            }
            if job == 1 && map.label_at(row, col) == 2 {
                region1_on_region2 += 1;
            }
            if job == 2 {
                region2_cuts += 1;
            }
        }
        assert_eq!(
            region1_on_region2, 0,
            "{strategy:?}: region 1 cut {region1_on_region2} points on the cells of region 2"
        );
        assert!(
            stray.is_empty(),
            "{strategy:?}: {} clearing cuts are off the cells of their job, first {:?}",
            stray.len(),
            stray.first()
        );
        assert!(
            region2_cuts > 10,
            "{strategy:?}: region 2 must cut pocket B itself, got {region2_cuts} cuts"
        );
    }
}
