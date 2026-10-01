//! S1 of `planning/by_area_merge_tree_2026-09-25/PLAN.md` §8.
//!
//! On a flat plane below the stock top, the pocket tree has one root and no
//! valley: one pocket is no split (`adaptive3d/area_plan.rs`). By Area then
//! has one job, the rest, with Global's levels and Global's waterline
//! cleanup after each level. So By Area emits moves byte-identical to
//! Global.
//!
//! Before WP2 the flood-fill detector also found one region here, but it ran
//! the waterline cleanup once at the bottom Z (F9), not after each level, so
//! the moves differed from Global.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::adaptive3d::{
    Adaptive3dDepth, Adaptive3dGeometry, Adaptive3dLinking, Adaptive3dParams,
    Adaptive3dRuntimeEvent, AreaRegionKind, ClearingStrategy3d, EntryStyle3d, RegionOrdering,
    adaptive_3d_toolpath_output_with_cancel,
};
use rs_cam_core::mesh::{SpatialIndex, make_test_flat};
use rs_cam_core::tool::FlatEndmill;

const TOOL_RADIUS: f64 = 3.0;

fn params(strategy: ClearingStrategy3d, ordering: RegionOrdering) -> Adaptive3dParams {
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
            // Three levels (Z 6, 2, 0.5) over the plane at Z 0.
            depth_per_pass: 4.0,
            stock_to_leave: 0.5,
            stock_top_z: 10.0,
            z_floor: None,
            detect_flat_areas: false,
        },
        linking: Adaptive3dLinking {
            region_ordering: ordering,
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
fn by_area_on_a_flat_plane_emits_the_global_moves() {
    let mesh = make_test_flat(40.0);
    let index = SpatialIndex::build(&mesh, 10.0);
    let cutter = FlatEndmill::new(TOOL_RADIUS * 2.0, 25.0);

    for strategy in [
        ClearingStrategy3d::ContourParallel,
        ClearingStrategy3d::Adaptive,
        ClearingStrategy3d::ContourSpiral,
        ClearingStrategy3d::AgentSearch,
    ] {
        let run = |ordering| {
            adaptive_3d_toolpath_output_with_cancel(
                &mesh,
                &index,
                &cutter,
                &params(strategy, ordering),
                &|| false,
                None,
            )
            .unwrap()
        };
        let global = run(RegionOrdering::Global);
        let by_area = run(RegionOrdering::ByArea);

        // One job: the rest, no valley.
        let map = by_area
            .area_regions
            .as_ref()
            .expect("By Area records its jobs");
        assert_eq!(map.regions.len(), 1, "{strategy:?}: one job");
        assert_eq!(map.regions[0].kind, AreaRegionKind::Rest, "{strategy:?}");
        assert!(map.regions[0].saddle_z.is_none());
        let waterlines = |out: &rs_cam_core::adaptive3d::Adaptive3dOutput| {
            out.annotations
                .iter()
                .filter(|a| matches!(a.event, Adaptive3dRuntimeEvent::WaterlineCleanup))
                .count()
        };
        assert_eq!(
            waterlines(&by_area),
            waterlines(&global),
            "{strategy:?}: one waterline cleanup per level, as Global"
        );

        // Non-vacuity: the plane is cut on three levels.
        assert!(
            global.toolpath.moves.len() > 50,
            "{strategy:?}: Global has {} moves",
            global.toolpath.moves.len()
        );
        assert_eq!(map.regions[0].level_count, 3, "{strategy:?}");

        assert_eq!(
            global.toolpath.moves.len(),
            by_area.toolpath.moves.len(),
            "{strategy:?}: the move counts differ"
        );
        for (i, (g, a)) in global
            .toolpath
            .moves
            .iter()
            .zip(&by_area.toolpath.moves)
            .enumerate()
        {
            assert_eq!(
                format!("{g:?}"),
                format!("{a:?}"),
                "{strategy:?}: move {i} differs"
            );
        }
    }
}
