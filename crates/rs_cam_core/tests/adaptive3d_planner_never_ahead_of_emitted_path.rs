//! G-PHANTOMSTAMP (2026-09-25) — the adaptive3d planner is never ahead of
//! the path it emits.
//!
//! The planner keeps its own dexel stock and reads every entry's rapid floor
//! from it. The F-038 coalescing pass deleted an entry that the planner had
//! already stamped: the entry column stayed in the planner stock, so a later
//! entry read a floor the emitted path never cut and rapided into material
//! (Wanaka "3D Rough 6", 1.33 mm into stock at every simulation cell;
//! `planning/entry_stock_awareness_2026-09-24/PHANTOMSTAMP_PLAN.md`).
//!
//! The fix stamps an entry only when a cut or a link commits it; an entry
//! that the next entry replaces is never stamped and never moves the
//! planner's position.
//!
//! The fixture: a 60×60 plate at Z 0 with a centred 20×20 boss 12 mm tall
//! and 45° chamfered walls (slope 1, over `PLUNGE_SLOPE_LIMIT` 0.3, so wall
//! steps split a lifted path into back-to-back entries), stock top 15; Ø6
//! flat, AgentSearch, Global, plunge, Depth/Pass 2.6, stepover 2.2, leave
//! 0.5, `min_region_cut_length_mm` 15, no boundary.
//!
//! - Precondition: the planner replaced (coalesced) at least one entry, so
//!   the test is not vacuous.
//! - (1) Replaying the emitted toolpath on a fresh stock at the planner's
//!   0.5 mm cell, no cell inside the mesh footprint of the planner's final
//!   stock stands more than 0.25 mm below the replay. The plan is also
//!   stopped after each of its first four levels (`z_floor` at the level):
//!   a phantom entry column shows between levels, because the next level
//!   cuts deeper over it and hides it from the whole plan's final stock.
//! - (2) The generated operation has no rapid collision at 0.25 and 0.125 mm.
//!
//! Red before the fix (2026-09-26, the pre-fix `clearing.rs` / `path.rs`
//! put back in the working tree): (1) failed, the plan stopped at Z 9.8
//! stood 1.219 mm below the replay at (-16.50, -25.50) and the whole plan
//! 0.252 mm at (20.00, -20.50); 94 entries coalesced. (2) was green before
//! the fix on this fixture; the rapid strike itself is pinned on Wanaka by
//! `adaptive3d_wanaka_rough6_no_phantom_rapid_g_phantomstamp`. After the
//! fix: 0 cells at every stop, 87 entries replaced before any stamp.
//!
//! ```text
//! cargo test -p rs_cam_core -q --test adaptive3d_planner_never_ahead_of_emitted_path
//! ```

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

mod common;

use std::sync::atomic::AtomicBool;

use rs_cam_core::adaptive3d::{
    Adaptive3dDepth, Adaptive3dGeometry, Adaptive3dLinking, Adaptive3dParams, ClearingStrategy3d,
    EntryStyle3d, RegionOrdering, adaptive_3d_toolpath_output_with_cancel,
    debug_adaptive_3d_segments_for_f029_probe,
};
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::{
    Adaptive3dConfig, Adaptive3dEntryStyle, ClearingStrategy, RegionOrdering as CfgOrdering,
};
use rs_cam_core::dexel_stock::{StockCutDirection, TriDexelStock};
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::session::SimulationOptions;
use rs_cam_core::stock::radial_profile::{LUT_SAMPLES, RadialProfileLUT};
use rs_cam_core::tool::{FlatEndmill, MillingCutter};
use rs_cam_core::trace::debug_trace::ToolpathDebugRecorder;

const HALF: f64 = 30.0;
const BOSS_HALF: f64 = 10.0;
const BOSS_HEIGHT: f64 = 12.0;
const STOCK_TOP_Z: f64 = 15.0;
const SAFE_Z: f64 = 20.0;
const DPP: f64 = 2.6;
const STEPOVER: f64 = 2.2;
const LEAVE: f64 = 0.5;
const MIN_REGION_CUT_MM: f64 = 15.0;
/// The planner may stand below the replay by the dexel read, not more.
const AHEAD_TOL_MM: f64 = 0.25;

/// A 20×20 top at Z 12; 45° walls down to the plate at Z 0.
fn boss_mesh() -> TriangleMesh {
    common::meshes::height_field(HALF, 0.5, |x, y| {
        let d = x.abs().max(y.abs());
        (BOSS_HEIGHT - (d - BOSS_HALF).max(0.0)).clamp(0.0, BOSS_HEIGHT)
    })
}

fn params(z_floor: Option<f64>) -> Adaptive3dParams {
    Adaptive3dParams {
        entry_clearance_mm: 0.5,
        ramp_feed_rate: None,
        geometry: Adaptive3dGeometry {
            tool_radius: 3.0,
            envelope_radius: 3.0,
            stepover: STEPOVER,
            tolerance: 0.1,
            segment_merge_tolerance: None,
            min_cutting_radius: 0.0,
            boundary: None,
            world_stock_xy_bbox: None,
        },
        depth: Adaptive3dDepth {
            depth_per_pass: DPP,
            stock_to_leave: LEAVE,
            stock_top_z: STOCK_TOP_Z,
            z_floor,
            detect_flat_areas: false,
        },
        linking: Adaptive3dLinking {
            region_ordering: RegionOrdering::Global,
            min_region_cut_length_mm: MIN_REGION_CUT_MM,
            max_stay_down_distance_mm: None,
            stay_down_clearance_mm: 0.5,
        },
        trochoid_cap_mult: 1.6,
        engagement_measure: rs_cam_core::adaptive::EngagementMeasure::DiskArea,
        feed_rate: 1500.0,
        plunge_rate: 500.0,
        safe_z: SAFE_Z,
        entry_style: EntryStyle3d::Plunge,
        initial_stock: None,
        clearing_strategy: ClearingStrategy3d::AgentSearch,
        z_blend: false,
    }
}

/// The planner's own run: the emitted toolpath and the entries it
/// coalesced (the `coalesced_entries_f038` counter, summed over levels).
fn planned(z_floor: Option<f64>) -> (rs_cam_core::toolpath::Toolpath, f64) {
    let mesh = boss_mesh();
    let index = SpatialIndex::build(&mesh, 8.0);
    let tool = FlatEndmill::new(6.0, 25.0);
    let recorder = ToolpathDebugRecorder::new("phantom stamp", "Adaptive3d");
    let root = recorder.root_context();
    let out = adaptive_3d_toolpath_output_with_cancel(
        &mesh,
        &index,
        &tool,
        &params(z_floor),
        &|| false,
        Some(&root),
    )
    .expect("planner");
    drop(root);
    let trace = recorder.finish();
    let coalesced: f64 = trace
        .spans
        .iter()
        .filter_map(|s| s.counters.get("coalesced_entries_f038"))
        .sum();
    (out.toolpath, coalesced)
}

/// The Z levels of the plan (`path.rs::step_levels` from the stock top), so
/// a `z_floor` at one of them stops the plan there with the same levels
/// above it.
fn level_zs() -> Vec<f64> {
    let mut out = Vec::new();
    let mut z = STOCK_TOP_Z - DPP;
    while z > LEAVE {
        out.push(z);
        z -= DPP;
    }
    out
}

/// Cells (inside the mesh footprint) where the planner's final stock stands
/// more than `AHEAD_TOL_MM` below a replay of the emitted toolpath, for the
/// plan stopped at `z_floor`; and the entries it coalesced.
fn planner_ahead_cells(z_floor: Option<f64>) -> (usize, f64, (f64, f64), f64) {
    let (tp, coalesced) = planned(z_floor);
    let mesh = boss_mesh();
    let index = SpatialIndex::build(&mesh, 8.0);
    let tool = FlatEndmill::new(6.0, 25.0);
    let (plan_tops, rows, cols, cell, ou, ov, zmin, zmax) =
        debug_adaptive_3d_segments_for_f029_probe(&mesh, &index, &tool, &params(z_floor), &|| {
            false
        })
        .expect("planner probe");

    // The replay grid is the planner's grid: same origin, cell and extent
    // (`path.rs::adaptive_3d_segments`: the mesh box grown by the radius).
    let b = mesh.bbox;
    let r = tool.radius();
    let mut replay = TriDexelStock::from_stock(
        b.min.x - r,
        b.min.y - r,
        b.max.x + r,
        b.max.y + r,
        zmin,
        zmax,
        cell,
    );
    assert_eq!(
        (replay.z_grid.rows, replay.z_grid.cols),
        (rows, cols),
        "the replay grid must be the planner grid"
    );
    assert!((replay.z_grid.origin_u - ou).abs() < 1e-9);
    assert!((replay.z_grid.origin_v - ov).abs() < 1e-9);
    let lut = RadialProfileLUT::from_cutter(&tool, LUT_SAMPLES);
    for w in tp.moves.windows(2) {
        replay.stamp_linear_segment(
            &lut,
            tool.radius(),
            w[0].target,
            w[1].target,
            StockCutDirection::FromTop,
        );
    }

    let mut ahead = 0usize;
    let mut worst = (0.0_f64, (0.0, 0.0));
    for r in 0..rows {
        for c in 0..cols {
            // Outside the mesh footprint the planner clears the border cells
            // by fiat (the F-027 border clear, `path.rs`), not by stamping.
            let (x, y) = (ou + c as f64 * cell, ov + r as f64 * cell);
            if x.abs() > HALF || y.abs() > HALF {
                continue;
            }
            let p = f64::from(plan_tops[r * cols + c]);
            let p = if p.is_finite() { p } else { zmin };
            let s = replay.z_grid.top_z_at(r, c).map_or(zmin, f64::from);
            if s - p > AHEAD_TOL_MM {
                ahead += 1;
                if s - p > worst.0 {
                    worst = (s - p, (x, y));
                }
            }
        }
    }
    (ahead, worst.0, worst.1, coalesced)
}

/// The planner stock after each level (the plan stopped there by `z_floor`)
/// and after the whole plan is never below the emitted path. A phantom
/// entry column shows between levels: the next level cuts deeper over it.
#[test]
fn the_planner_stock_is_never_below_the_emitted_path() {
    let mut floors: Vec<Option<f64>> = level_zs().into_iter().take(4).map(Some).collect();
    floors.push(None);
    let mut total_coalesced = 0.0;
    let mut failures = Vec::new();
    for z_floor in floors {
        let (ahead, worst, at, coalesced) = planner_ahead_cells(z_floor);
        total_coalesced += coalesced;
        eprintln!(
            "z_floor {z_floor:?}: coalesced entries {coalesced}; cells where the planner is \
             ahead of the emitted path by > {AHEAD_TOL_MM} mm: {ahead}, worst {worst:.3} mm at \
             ({:.2}, {:.2})",
            at.0, at.1
        );
        if ahead > 0 {
            failures.push(format!(
                "z_floor {z_floor:?}: {ahead} cells, worst {worst:.3} mm at ({:.2}, {:.2})",
                at.0, at.1
            ));
        }
    }
    assert!(
        total_coalesced > 0.0,
        "non-vacuity: the fixture must make the planner coalesce an entry"
    );
    assert!(
        failures.is_empty(),
        "the planner stock stands below the emitted path (G-PHANTOMSTAMP): {failures:?}"
    );
}

#[test]
fn the_rough_rapids_into_no_stock() {
    let cfg = Adaptive3dConfig {
        stepover: STEPOVER,
        depth_per_pass: DPP,
        stock_to_leave_axial: LEAVE,
        entry_style: Adaptive3dEntryStyle::Plunge,
        region_ordering: CfgOrdering::Global,
        clearing_strategy: ClearingStrategy::AgentSearch,
        min_region_cut_length_mm: MIN_REGION_CUT_MM,
        ..Adaptive3dConfig::default()
    };
    let mut session = common::session::single_op_session(
        common::session::stock_over(HALF, STOCK_TOP_Z),
        common::make_endmill_6mm(),
        common::session::mesh_model(boss_mesh(), "boss"),
        "3D Rough",
        OperationConfig::Adaptive3d(cfg),
    );
    common::session::generate(&mut session, 0);
    let cancel = AtomicBool::new(false);
    for cell_mm in [0.25, 0.125] {
        let opts = SimulationOptions {
            resolution: cell_mm,
            adaptive_feed_modulation: false,
            ..Default::default()
        };
        session.run_simulation(&opts, &cancel).expect("simulation");
        let sim = session.simulation_result().expect("sim result");
        for c in &sim.rapid_collisions {
            eprintln!(
                "cell {cell_mm}: rapid_collision move={} start={:?} end={:?}",
                c.move_index, c.start, c.end
            );
        }
        assert_eq!(
            sim.rapid_collisions.len(),
            0,
            "cell {cell_mm} mm: the rough rapids into stock (G-PHANTOMSTAMP)"
        );
    }
}
