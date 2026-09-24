//! S3 of `planning/by_area_merge_tree_2026-09-25/PLAN.md` (WP1 form).
//!
//! By Area changes only the order of the cuts. The final planner stock of
//! By Area equals the final planner stock of Global within 0.05 mm per
//! cell.
//!
//! ## The fixture
//!
//! `common::meshes::l_and_basin_plate`: pocket A (an L, floor Z 0) and
//! pocket B (floor Z 3) in the inside corner of the L. Stock top Z 10,
//! Depth/Pass 4, stock-to-leave 0.5. So the floor of B plus the leave
//! (Z 3.5) is between the levels Z 6 and Z 2.
//!
//! ## The two defects this test holds (PLAN §3.4)
//!
//! - Level filter: the levels of region B must continue down to the first
//!   level at or below Z 3.5. Before the fix, B stopped at Z 6.
//! - Material gate: at that last level every cell of B has its floor above
//!   the level. Before the fix, the gate counted no cell and skipped the
//!   level.
//!
//! Either defect leaves pocket B at Z 6, and Global cuts it to Z 3.5.
//!
//! ## The waterline confound (PLAN §7.4, F9)
//!
//! Global runs the waterline cleanup after every level. By Area runs it
//! once, at the bottom Z. The cleanup is not masked, so the two orders do
//! not give the same stock, and the difference is not a mask defect.
//! Measured on this fixture (2026-09-25):
//!
//! - With the Global cleanup moved to the last level (a local experiment,
//!   not in the product), all four strategies give the same stock on every
//!   owned cell: 0 cells differ by more than 0.05 mm.
//! - With the product Global, Contour Parallel still gives 0 cells. The
//!   three strategies that run the 2D adaptive sub-pass leave a rim strip
//!   inside the tool radius of the wall, and the Global cleanup at Z 6 cuts
//!   it. Pocket B has no wall at the bottom Z, so the By Area cleanup never
//!   reaches it: 361 to 683 cells differ, by up to 4 mm.
//! - The rim cells beside a wall (label 0) differ by up to 4.4 mm for the
//!   same reason. The test does not compare them.
//!
//! ## The assertions
//!
//! - Every strategy: the levels of region B reach Z 3.5.
//! - Contour Parallel: every owned cell (label not 0) is within 0.05 mm of
//!   Global.
//! - Every strategy: region B has at least 95 % as many cells at its floor
//!   (Z 3.5) as Global has. Either defect of §3.4 gives 0 cells.

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
    Adaptive3dDepth, Adaptive3dGeometry, Adaptive3dLinking, Adaptive3dParams, ClearingStrategy3d,
    EntryStyle3d, RegionOrdering, adaptive_3d_toolpath_output_with_cancel,
    debug_adaptive_3d_segments_for_f029_probe,
};
use rs_cam_core::mesh::SpatialIndex;
use rs_cam_core::tool::FlatEndmill;

const TOOL_RADIUS: f64 = 3.0;
const STOCK_TO_LEAVE: f64 = 0.5;
/// The floor Z of pocket B.
const POCKET_B_FLOOR: f64 = 3.0;
/// The largest per-cell difference of the final stock top (PLAN §8, S3).
const TOLERANCE_MM: f64 = 0.05;

fn params(strategy: ClearingStrategy3d, ordering: RegionOrdering) -> Adaptive3dParams {
    Adaptive3dParams {
        entry_clearance_mm: 0.5,
        ramp_feed_rate: None,
        geometry: Adaptive3dGeometry {
            tool_radius: TOOL_RADIUS,
            envelope_radius: TOOL_RADIUS,
            stepover: 1.5,
            tolerance: 0.5,
            min_cutting_radius: 0.0,
            boundary: None,
            world_stock_xy_bbox: None,
        },
        depth: Adaptive3dDepth {
            depth_per_pass: 4.0,
            stock_to_leave: STOCK_TO_LEAVE,
            stock_top_z: L_AND_BASIN_TOP_Z,
            z_floor: None,
            detect_flat_areas: false,
        },
        linking: Adaptive3dLinking {
            region_ordering: ordering,
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

#[test]
fn by_area_final_stock_equals_global() {
    let mesh = l_and_basin_plate();
    let index = SpatialIndex::build(&mesh, 10.0);
    let cutter = FlatEndmill::new(TOOL_RADIUS * 2.0, 25.0);

    for strategy in [
        ClearingStrategy3d::ContourParallel,
        ClearingStrategy3d::Adaptive,
        ClearingStrategy3d::ContourSpiral,
        ClearingStrategy3d::AgentSearch,
    ] {
        let by_area = params(strategy, RegionOrdering::ByArea);

        // The level filter: region B (region 2) continues to a level at or
        // below its floor plus the leave.
        let out = adaptive_3d_toolpath_output_with_cancel(
            &mesh,
            &index,
            &cutter,
            &by_area,
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
        let b = &map.regions[1];
        let [_, b_bottom] = b.level_z_range.expect("region B has levels");
        assert!(
            b_bottom <= POCKET_B_FLOOR + STOCK_TO_LEAVE + 0.01,
            "{strategy:?}: the last level of region B is Z {b_bottom:.3}; it must be at or \
             below Z {:.3}",
            POCKET_B_FLOOR + STOCK_TO_LEAVE
        );

        let (tops_area, rows, cols, ..) =
            debug_adaptive_3d_segments_for_f029_probe(&mesh, &index, &cutter, &by_area, &|| false)
                .unwrap();
        let global = params(strategy, RegionOrdering::Global);
        let (tops_global, rows_g, cols_g, ..) =
            debug_adaptive_3d_segments_for_f029_probe(&mesh, &index, &cutter, &global, &|| false)
                .unwrap();
        assert_eq!((rows, cols), (rows_g, cols_g));
        assert_eq!(
            (rows, cols),
            (map.rows, map.cols),
            "the map is on the planner grid"
        );

        // Region B reaches its floor on as many cells as Global.
        let floor = (POCKET_B_FLOOR + STOCK_TO_LEAVE + TOLERANCE_MM) as f32;
        let b_cells = |tops: &[f32]| {
            tops.iter()
                .zip(&map.labels)
                .filter(|&(&t, &l)| l == 2 && t <= floor)
                .count()
        };
        let (b_area, b_global) = (b_cells(&tops_area), b_cells(&tops_global));
        assert!(
            b_global > 500 && b_area * 100 >= b_global * 95,
            "{strategy:?}: region B is at its floor on {b_area} cells in By Area and on \
             {b_global} cells in Global"
        );
        if strategy != ClearingStrategy3d::ContourParallel {
            continue;
        }

        let mut worst = (0.0f64, 0usize);
        let mut over = 0usize;
        let mut compared = 0usize;
        for (i, (a, g)) in tops_area.iter().zip(&tops_global).enumerate() {
            if map.labels[i] == 0 {
                continue;
            }
            compared += 1;
            // A cell with no material in both runs reads -inf in both.
            if !a.is_finite() && !g.is_finite() {
                continue;
            }
            let d = f64::from((a - g).abs());
            if !d.is_finite() || d > TOLERANCE_MM {
                over += 1;
            }
            if !d.is_finite() || d > worst.0 {
                worst = (if d.is_finite() { d } else { f64::INFINITY }, i);
            }
        }
        assert!(compared > 500, "{strategy:?}: only {compared} owned cells");
        let (d, i) = worst;
        assert_eq!(
            over,
            0,
            "{strategy:?}: {over} cells differ by more than {TOLERANCE_MM} mm; the worst is \
             cell (row {}, col {}) by {d:.3} mm (By Area {:.3}, Global {:.3})",
            i / cols,
            i % cols,
            tops_area[i],
            tops_global[i]
        );
    }
}
