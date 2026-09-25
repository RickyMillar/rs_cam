//! Global ordering cuts a floor that is between two levels.
//!
//! ## The defect
//!
//! Every level drapes: it cuts to `max(z, CL + stock_to_leave)`. So a level
//! below a floor still cuts the material between the level above and that
//! floor. The Global level gate once counted only the cells with
//! `CL + stock_to_leave <= z`. At a level below every floor that the tool
//! can reach, it counted no cell and skipped the level. The material above
//! the floor stayed for the finish tool. On rivmap100 (Depth/Pass 8, levels
//! Z 4.0 and Z 0.5) Global skipped Z 0.5 and left about 1200 mm³ in the
//! valleys.
//!
//! The gate now counts the cells of the level grid, the same test that the
//! rings read (PLAN §3.4.1 of `planning/by_area_merge_tree_2026-09-25`).
//!
//! ## The fixture
//!
//! `common::meshes::pitted_basin_plate`: a plate at Z 10 with one basin
//! (floor Z 3). A 2 mm pit in the basin floor puts the bottom of the mesh
//! box at Z 0, so the lowest level is Z 0.5, below the basin floor. Stock
//! top Z 10, Depth/Pass 4, stock-to-leave 0.5: the levels are Z 6, Z 2 and
//! Z 0.5, and the basin floor plus the leave (Z 3.5) is between Z 6 and
//! Z 2. Detect Flat is off, because a flat level at Z 3.5 hides the defect.
//!
//! ## The assertions
//!
//! For each of the four strategies, with Global ordering:
//!
//! - At least 95 % of the basin cells where the tool fits have a final
//!   planner stock top at Z 3.5 or below (within 0.05 mm). The old gate
//!   left all of them at Z 6.
//! - No basin cell is below Z 3.5 by more than 0.05 mm.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;
use common::meshes::{PITTED_BASIN, PITTED_BASIN_TOP_Z, pitted_basin_plate};

use rs_cam_core::adaptive3d::{
    Adaptive3dDepth, Adaptive3dGeometry, Adaptive3dLinking, Adaptive3dParams, ClearingStrategy3d,
    EntryStyle3d, RegionOrdering, debug_adaptive_3d_segments_for_f029_probe,
};
use rs_cam_core::mesh::SpatialIndex;
use rs_cam_core::tool::FlatEndmill;

const TOOL_RADIUS: f64 = 3.0;
const STOCK_TO_LEAVE: f64 = 0.5;
const TOLERANCE_MM: f64 = 0.05;

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
            stock_to_leave: STOCK_TO_LEAVE,
            stock_top_z: PITTED_BASIN_TOP_Z,
            z_floor: None,
            detect_flat_areas: false,
        },
        linking: Adaptive3dLinking {
            region_ordering: RegionOrdering::Global,
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
fn global_levels_drape_to_a_floor_between_two_levels() {
    let mesh = pitted_basin_plate();
    let index = SpatialIndex::build(&mesh, 10.0);
    let cutter = FlatEndmill::new(TOOL_RADIUS * 2.0, 25.0);
    let (x0, y0, x1, y1, basin_floor) = PITTED_BASIN;
    let floor = basin_floor + STOCK_TO_LEAVE;

    for strategy in [
        ClearingStrategy3d::ContourParallel,
        ClearingStrategy3d::Adaptive,
        ClearingStrategy3d::ContourSpiral,
        ClearingStrategy3d::AgentSearch,
    ] {
        let (tops, rows, cols, cell, origin_u, origin_v, ..) =
            debug_adaptive_3d_segments_for_f029_probe(
                &mesh,
                &index,
                &cutter,
                &params(strategy),
                &|| false,
            )
            .unwrap();

        // The basin cells where the whole tool fits, one cell in from that
        // edge. The tool-centre floor of these cells is the basin floor.
        let inset = TOOL_RADIUS + cell;
        let mut basin = 0usize;
        let mut at_floor = 0usize;
        let mut worst_high = f32::NEG_INFINITY;
        let mut worst_low = f32::INFINITY;
        for row in 0..rows {
            for col in 0..cols {
                let x = origin_u + col as f64 * cell;
                let y = origin_v + row as f64 * cell;
                if x < x0 + inset || x > x1 - inset || y < y0 + inset || y > y1 - inset {
                    continue;
                }
                let top = tops[row * cols + col];
                basin += 1;
                if f64::from(top) <= floor + TOLERANCE_MM {
                    at_floor += 1;
                }
                worst_high = worst_high.max(top);
                worst_low = worst_low.min(top);
            }
        }
        assert!(basin > 500, "{strategy:?}: only {basin} basin cells");
        assert!(
            at_floor * 100 >= basin * 95,
            "{strategy:?}: {at_floor} of {basin} basin cells are at Z {floor:.2} or below; \
             the highest basin cell is at Z {worst_high:.3}"
        );
        assert!(
            f64::from(worst_low) >= floor - TOLERANCE_MM,
            "{strategy:?}: a basin cell is at Z {worst_low:.3}, below the floor Z {floor:.2}"
        );
    }
}
