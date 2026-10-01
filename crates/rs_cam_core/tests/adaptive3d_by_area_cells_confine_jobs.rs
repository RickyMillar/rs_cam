//! S2 of `planning/by_area_merge_tree_2026-09-25/PLAN.md` §8 (WP2 form).
//!
//! By Area confines a job to the cells that it owns, not to the bounding
//! box of those cells (PLAN §3.2, RESULTS.md finding 3).
//!
//! ## The fixtures
//!
//! - `common::meshes::l_and_basin_plate`: an L-shaped pocket A and a pocket
//!   B in the inside corner of the L. The XY box of A contains all of B. The
//!   plate is at the stock top, so the pocket tree has two roots with no
//!   children: two valleys and no rest. Valley 1 is A (the deeper floor
//!   goes first when the tool has no position yet), valley 2 is B.
//! - `common::meshes::three_valley_plate`: three pockets under a plate at
//!   Z 6, stock top Z 10. The ridge is below the top, so the tree has a
//!   real rest (the plate) and three valleys. The rest runs first.
//!
//! ## The assertions
//!
//! For each clearing strategy:
//!
//! - Every clearing cut of a valley job has its tool centre on a cell of
//!   that valley, or on a neighbour of such a cell. A marching-squares ring
//!   point sits on the edge between a material cell and an air cell, so
//!   the test also accepts the 8 neighbours.
//! - No clearing cut of valley A has its centre on a cell of valley B.
//!   Under bbox confinement region 1 cut pocket B, and this failed.
//! - Each valley job cuts its own pocket.
//!
//! The waterline cleanup after the last job has no mask (F9), so the test
//! reads only the moves between a `RegionStart` and the cleanup.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

mod common;
use common::meshes::{
    L_AND_BASIN_TOP_Z, THREE_VALLEY_TOP_Z, l_and_basin_plate, three_valley_plate,
};

use rs_cam_core::adaptive3d::{
    Adaptive3dDepth, Adaptive3dGeometry, Adaptive3dLinking, Adaptive3dOutput, Adaptive3dParams,
    Adaptive3dRuntimeEvent, AreaRegionKind, AreaRegionMap, ClearingStrategy3d, EntryStyle3d,
    RegionOrdering, adaptive_3d_toolpath_output_with_cancel,
};
use rs_cam_core::mesh::SpatialIndex;
use rs_cam_core::tool::FlatEndmill;
use rs_cam_core::toolpath::MoveIntent;

const TOOL_RADIUS: f64 = 3.0;

fn params(strategy: ClearingStrategy3d, stock_top_z: f64) -> Adaptive3dParams {
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
            depth_per_pass: 4.0,
            stock_to_leave: 0.5,
            stock_top_z,
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
            r >= 0 && c >= 0 && map.region_order_at(r as usize, c as usize) == order
        })
    })
}

/// The job (region order) of each move: the region of the last
/// `RegionStart`, and 0 after the waterline cleanup.
fn job_of_moves(out: &Adaptive3dOutput) -> Vec<u16> {
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
    job_of
}

/// The valley cuts of each job: `(cuts, stray)` per region order, where a
/// stray cut has its centre off the cells of its valley. Rest jobs are not
/// read. Also returns the cuts of a valley job on the cells of another
/// valley.
fn valley_cuts(out: &Adaptive3dOutput, map: &AreaRegionMap) -> (Vec<usize>, Vec<String>, usize) {
    let job_of = job_of_moves(out);
    let mut cuts = vec![0usize; map.regions.len() + 1];
    let mut stray = Vec::new();
    let mut on_other_valley = 0usize;
    for (i, m) in out.toolpath.moves.iter().enumerate() {
        if m.intent != MoveIntent::ClearingCut || !m.move_type.is_cutting() {
            continue;
        }
        let job = job_of[i];
        let Some(region) = map.regions.iter().find(|r| r.order == job) else {
            continue;
        };
        if region.kind != AreaRegionKind::Valley {
            continue;
        }
        cuts[usize::from(job)] += 1;
        let Some((row, col)) = cell_of(map, m.target.x, m.target.y) else {
            stray.push(format!("move {i} job {job} off the grid"));
            continue;
        };
        if !near_label(map, row, col, job) {
            stray.push(format!(
                "move {i} job {job} at ({:.2}, {:.2})",
                m.target.x, m.target.y
            ));
        }
        let label = map.region_order_at(row, col);
        if label != job
            && map
                .regions
                .iter()
                .any(|r| r.order == label && r.kind == AreaRegionKind::Valley)
        {
            on_other_valley += 1;
        }
    }
    (cuts, stray, on_other_valley)
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
            &params(strategy, L_AND_BASIN_TOP_Z),
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
        assert!(
            map.regions.iter().all(|r| r.kind == AreaRegionKind::Valley),
            "{strategy:?}: the plate is at the top, so there is no rest"
        );
        // Valley 1 is pocket A (floor Z 0), valley 2 is pocket B (floor Z 3).
        assert!(
            map.regions[0].surface_z_range[0] < 1.0,
            "{:?}",
            map.regions[0]
        );
        assert!(
            map.regions[1].surface_z_range[0] > 2.0,
            "{:?}",
            map.regions[1]
        );

        let (cuts, stray, on_other_valley) = valley_cuts(&out, map);
        assert_eq!(
            on_other_valley, 0,
            "{strategy:?}: a valley job cut {on_other_valley} points on the cells of another valley"
        );
        assert!(
            stray.is_empty(),
            "{strategy:?}: {} clearing cuts are off the cells of their job, first {:?}",
            stray.len(),
            stray.first()
        );
        assert!(
            cuts[2] > 10,
            "{strategy:?}: region 2 must cut pocket B itself, got {} cuts",
            cuts[2]
        );
    }
}

/// The ridge is below the top: a rest job and three valley jobs. Every
/// valley job cuts only its own cells.
#[test]
fn by_area_valley_jobs_cut_only_their_own_cells_beside_a_rest() {
    let mesh = three_valley_plate();
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
            &params(strategy, THREE_VALLEY_TOP_Z),
            &|| false,
            None,
        )
        .unwrap();
        let map = out
            .area_regions
            .as_ref()
            .expect("By Area records its regions");
        let kinds: Vec<AreaRegionKind> = map.regions.iter().map(|r| r.kind).collect();
        assert_eq!(
            kinds,
            vec![
                AreaRegionKind::Rest,
                AreaRegionKind::Valley,
                AreaRegionKind::Valley,
                AreaRegionKind::Valley
            ],
            "{strategy:?}: the rest, then three valleys"
        );

        let (cuts, stray, on_other_valley) = valley_cuts(&out, map);
        assert_eq!(
            on_other_valley, 0,
            "{strategy:?}: a valley job cut {on_other_valley} points on the cells of another valley"
        );
        assert!(
            stray.is_empty(),
            "{strategy:?}: {} clearing cuts are off the cells of their job, first {:?}",
            stray.len(),
            stray.first()
        );
        for (order, &n) in cuts.iter().enumerate().skip(2) {
            assert!(
                n > 10,
                "{strategy:?}: valley job {order} must cut its pocket, got {n} cuts"
            );
        }
        assert_eq!(cuts.len(), 5, "{strategy:?}: jobs 1..=4");
    }
}
