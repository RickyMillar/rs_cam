//! C4b (the F9 fix; operator ruling D69, 2026-10-28): with "Contour each
//! level" on, a By Area 3D Rough cuts the walls of each region at each of
//! its levels, masked so that no cut meets another region's material above
//! the level.
//!
//! The fixture is one block (top 10) with two pockets, one with a hole and a ridge at the
//! stock top between them, so the planner detects two regions. Pocket A
//! (x > 0) goes down to 1. Pocket B (x < 0) has a shallow floor at 7 with a
//! deep hole in it down to 1. By Area roughs pocket A to the bottom before
//! pocket B. The wall cleanup traces every mesh contour at a Z: the hole's
//! wall contour first exists at Z 4, where pocket B still stands at the top
//! (10). An UNMASKED region-level run (F9) at pocket A's level 4 would cut
//! that wall through 6 mm of stock, two Depth/Pass.
//!
//! 1. Option off: the walls are cut once, at the bottom (one cleanup span).
//! 2. Option on: one cleanup per region-level plus the bottom run, and the
//!    op cuts more moves (non-vacuity); the simulated axial engagement of
//!    the op stays within one Depth/Pass plus one simulation cell (the mask
//!    keeps every cut off pocket B's unroughed stock). The planted red (the
//!    mask accepts every point) cuts the hole's wall through full stock.
//! 3. Two option-on runs in one process give the same moves.

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::print_stderr,
    reason = "test code: an expect is how a test reports a failure; the test prints its measurements for the reader"
)]

mod common;

use common::meshes::height_field;
use common::session::{mesh_model, stock_over, toolpath_config};
use common::tools::endmill_tool_config;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::{Adaptive3dConfig, ClearingStrategy, RegionOrdering};
use rs_cam_core::ids::ToolId;
use rs_cam_core::session::{
    ProjectSession, ProjectSessionBuilder, SimulationOptions, SimulationResolution,
};
use std::sync::atomic::AtomicBool;

const TOP_Z: f64 = 10.0;
/// Pocket A's floor and the hole's floor.
const DEEP_Z: f64 = 1.0;
/// Pocket B's shallow floor.
const SHALLOW_Z: f64 = 7.0;
const MESH_HALF: f64 = 30.0;
const MESH_STEP: f64 = 0.5;
/// Each pocket: 20 × 30 mm, centred 13 mm either side of the ridge.
const POCKET_HALF_X: f64 = 10.0;
const POCKET_HALF_Y: f64 = 15.0;
const POCKET_CENTRE_X: f64 = 13.0;
/// The hole in pocket B: 8 × 12 mm at its centre.
const HOLE_HALF_X: f64 = 4.0;
const HOLE_HALF_Y: f64 = 6.0;
const TOOL_DIAMETER: f64 = 6.0;
const DEPTH_PER_PASS: f64 = 3.0;
const SIM_CELL: f64 = 0.5;

fn session(contour: bool) -> ProjectSession {
    let mesh = height_field(MESH_HALF, MESH_STEP, |x, y| {
        let in_pocket =
            (x.abs() - POCKET_CENTRE_X).abs() <= POCKET_HALF_X && y.abs() <= POCKET_HALF_Y;
        let in_hole = (x + POCKET_CENTRE_X).abs() <= HOLE_HALF_X && y.abs() <= HOLE_HALF_Y;
        if !in_pocket {
            TOP_Z
        } else if x > 0.0 || in_hole {
            DEEP_Z
        } else {
            SHALLOW_Z
        }
    });
    let mut stock = stock_over(MESH_HALF, TOP_Z);
    rs_cam_core::compute::fixture_values::fill_unset_stock(&mut stock);
    let mut builder = ProjectSessionBuilder::new().stock(stock);
    let tool_idx = builder.add_tool(endmill_tool_config(TOOL_DIAMETER));
    let tool = ToolId::from_raw(builder.tools()[tool_idx].id.raw());
    let model_id = builder.add_model(mesh_model(mesh, "two pockets, one with a hole"));
    let mut op = toolpath_config(
        "rough",
        OperationConfig::Adaptive3d(Adaptive3dConfig {
            stepover: Some(2.0),
            depth_per_pass: Some(DEPTH_PER_PASS),
            clearing_strategy: ClearingStrategy::ContourParallel,
            region_ordering: RegionOrdering::ByArea,
            contour_each_level: contour,
            ..Adaptive3dConfig::default()
        }),
        tool,
        model_id,
    );
    op.debug_options.enabled = true;
    builder.add_toolpath(0, op).expect("add the rough");
    builder.build()
}

struct Run {
    regions: f64,
    cleanups: usize,
    moves: String,
    move_count: usize,
    max_axial: f64,
}

fn run(contour: bool) -> Run {
    let mut s = session(contour);
    let cancel = AtomicBool::new(false);
    s.generate_toolpath(0, &cancel).expect("generate the rough");
    let result = s.result_at(0).expect("the result");
    let spans = &result.debug_trace.as_ref().expect("the debug trace").spans;
    let regions = spans
        .iter()
        .find(|span| span.kind == "region_detect")
        .and_then(|span| span.counters.get("regions").copied())
        .expect("the region span states the count");
    let cleanups = spans
        .iter()
        .filter(|span| span.kind == "waterline_cleanup")
        .count();
    let moves = format!("{:?}", result.toolpath().moves);
    let move_count = result.toolpath().moves.len();
    s.run_simulation(
        &SimulationOptions {
            resolution: SimulationResolution::Fixed(SIM_CELL),
            ..SimulationOptions::default()
        },
        &cancel,
    )
    .expect("simulate");
    let id = s.toolpath_configs()[0].id;
    let max_axial = s
        .simulation_result()
        .and_then(|r| r.cut_trace.as_deref())
        .expect("the cut trace")
        .samples
        .iter()
        .filter(|x| x.toolpath_id == id && x.removed_volume_est_mm3 > 0.0)
        .map(|x| x.axial_engagement_mm)
        .fold(0.0, f64::max);
    eprintln!(
        "contour each level {contour}: {regions} regions, {cleanups} wall cleanups, \
         {move_count} moves, max axial {max_axial:.3} (DPP {DEPTH_PER_PASS})"
    );
    Run {
        regions,
        cleanups,
        moves,
        move_count,
        max_axial,
    }
}

#[test]
fn by_area_walls_cut_each_region_level_c4b() {
    let off = run(false);
    assert!(
        (off.regions - 2.0).abs() < 1e-9,
        "the fixture must hold two regions, or it tests nothing: {}",
        off.regions
    );
    assert_eq!(
        off.cleanups, 1,
        "option off: the walls are cut once, at the bottom"
    );

    let on = run(true);
    assert!(
        on.cleanups > off.cleanups,
        "option on: no region-level wall cleanup ran ({} cleanups)",
        on.cleanups
    );
    assert!(
        on.move_count > off.move_count,
        "option on: the region-level cleanups cut nothing ({} moves against {})",
        on.move_count,
        off.move_count
    );
    assert!(
        on.max_axial <= DEPTH_PER_PASS + SIM_CELL + 1e-6,
        "option on: a wall cut reaches {:.3} mm axially, over one Depth/Pass plus one sim \
         cell: a region-level cleanup cut stock that is not roughed yet (F9)",
        on.max_axial
    );
}

#[test]
fn by_area_contour_each_level_is_deterministic_c4b() {
    assert_eq!(run(true).moves, run(true).moves, "two runs differ");
}
