//! G-ADAPT3DPIECES — 3D Rough plans every piece of a split slice (W1.2b).
//!
//! # The defect this file pins (D2 in Adaptive3d)
//!
//! The AgentSearch and ContourSpiral strategies of 3D Rough clear each
//! region of a Z level with the shared 2D adaptive planner
//! (`adaptive3d/clearing.rs::clear_one_region`). The planner insets the
//! polygon it gets by the tool radius and plans only the first piece of
//! that inset (`machinable_vec[0]` in `adaptive/path.rs`). When the inset
//! split, every other piece got only the perimeter sweep, and its core
//! stood at full depth. The ContourParallel and Adaptive strategies do not
//! use the planner, so they did not show the defect.
//!
//! The fix is in the 3D adapter, with the split the 2D adapter uses
//! (`adaptive::inset_piece_outlines`): `planner_outlines` gives one
//! outline for each smoothed polygon and each piece of its inset, and
//! `clear_one_region` plans each in turn. The planner is unchanged.
//!
//! # Why the neck is between D and 2D wide
//!
//! The region polygon is in tool-centre space: `cell_has_material` reads
//! the drop-cutter height map, so a cell within R of a wall is not
//! material. The region is the cavity inset by R, and the planner insets
//! it by R again. The split thus needs a neck wider than D (else the
//! region itself is two regions, and each one is planned) and narrower
//! than 2D. Measured 2026-10-14: a 5 mm neck gives two regions and no
//! defect; this 10 mm neck shows it.
//!
//! # The fixture
//!
//! A block (top Z 10) with a cavity 6 mm deep: two 30 mm square lobes
//! joined by a neck 10 mm long and `NECK_W` wide. Ø6 mm end mill. 3D
//! Rough with its default settings except the strategy; the axial leave
//! is the default 0.5 mm. The 3D adapter has no cleanup dial (it always
//! uses `ContourParallelHybrid`, `clearing.rs`), so the default cleanup
//! is the exposed one.
//!
//! # The claims
//!
//! 1. The fixture is what it says: the cavity insets by R to one piece
//!    (one region) and by 2R to two pieces (the planner's split).
//! 2. AgentSearch and ContourSpiral each clear both lobes: no judged cell
//!    keeps material above the floor plus the leave plus
//!    `FRESH_MATERIAL_THRESHOLD_MM`. A judged cell is in a lobe and at
//!    least `R + CELL_MM` from the lobe walls. The band nearer the walls
//!    is not judged: it stands in both lobes, also with the fix, because
//!    of the double inset above (a separate finding).
//! 3. Each arm has no rapid collision: the move from one lobe to the
//!    other does not rapid through stock.
//!
//! Red before the fix (2026-10-14), both strategies: the +X lobe kept
//! 555 of 2209 judged cells at 5.500 mm above the target (the full depth
//! less the leave); the -X lobe kept 0. Green after: 0 and 0.
//!
//! ```text
//! cargo test -p rs_cam_core -q --test adaptive3d_plans_every_slice_piece_g_adapt3dpieces
//! ```

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
// The measured numbers ARE the record.
#![allow(clippy::print_stderr)]

mod common;

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use common::meshes::height_field_grid;
use common::session::{generate, mesh_model, single_op_session};
use common::tools::endmill_tool_config;
use rs_cam_core::compute::StockConfig;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::{Adaptive3dConfig, ClearingStrategy};
use rs_cam_core::dexel_stock::{FRESH_MATERIAL_THRESHOLD_MM, TriDexelStock};
use rs_cam_core::geo::P2;
use rs_cam_core::mesh::TriangleMesh;
use rs_cam_core::polygon::{Polygon2, offset_polygon};
use rs_cam_core::session::{
    Command, ProjectSession, SetSimulationResolutionArgs, SimulationOptions, SimulationResolution,
};

/// Side (mm) of each square lobe.
const LOBE: f64 = 30.0;

/// Length (mm) of the neck between the lobes.
const NECK_LEN: f64 = 10.0;

/// Width (mm) of the neck: between D and 2D (see the module doc).
const NECK_W: f64 = 10.0;

/// Cutter diameter (mm) and radius (mm).
const TOOL_D: f64 = 6.0;
const TOOL_R: f64 = TOOL_D / 2.0;

/// Block top (mm), which is also the stock top, and the cavity depth (mm).
const TOP: f64 = 10.0;
const DEPTH: f64 = 6.0;

/// The axial leave (mm): the `Adaptive3dConfig` default, stated.
const LEAVE: f64 = 0.5;

/// Stock margin (mm) around the cavity. The mesh reaches `2 * MARGIN`
/// past the cavity, so every stock cell lies over the block top.
const MARGIN: f64 = 2.0;

/// Mesh sample step (mm). The samples sit half a step off the integer
/// grid, so no sample lies on a cavity edge.
const MESH_STEP: f64 = 0.5;

/// Simulation cell (mm).
const CELL_MM: f64 = 0.5;

/// Half extents (mm) of the cavity.
const HALF_X: f64 = LOBE + NECK_LEN / 2.0;
const HALF_Y: f64 = LOBE / 2.0;

/// The distance (mm) from the lobe walls inside which a cell is not
/// judged: the band of the double inset, plus one cell that the
/// simulator blends.
const JUDGE_INSET: f64 = TOOL_R + CELL_MM;

fn in_cavity(x: f64, y: f64) -> bool {
    let lobe = y.abs() <= HALF_Y && x.abs() >= NECK_LEN / 2.0 && x.abs() <= HALF_X;
    let neck = x.abs() < NECK_LEN / 2.0 && y.abs() <= NECK_W / 2.0;
    lobe || neck
}

/// The cavity outline, wound CCW, centred on the origin.
fn cavity() -> Polygon2 {
    let (n, h, w) = (NECK_LEN / 2.0, HALF_Y, NECK_W / 2.0);
    Polygon2::new(vec![
        P2::new(-HALF_X, -h),
        P2::new(-n, -h),
        P2::new(-n, -w),
        P2::new(n, -w),
        P2::new(n, -h),
        P2::new(HALF_X, -h),
        P2::new(HALF_X, h),
        P2::new(n, h),
        P2::new(n, w),
        P2::new(-n, w),
        P2::new(-n, h),
        P2::new(-HALF_X, h),
    ])
}

fn mesh() -> TriangleMesh {
    let reach_x = HALF_X + 2.0 * MARGIN;
    let reach_y = HALF_Y + 2.0 * MARGIN;
    let x0 = -reach_x - MESH_STEP / 2.0;
    let y0 = -reach_y - MESH_STEP / 2.0;
    let nx = ((2.0 * reach_x + MESH_STEP) / MESH_STEP).round() as usize + 1;
    let ny = ((2.0 * reach_y + MESH_STEP) / MESH_STEP).round() as usize + 1;
    height_field_grid(x0, MESH_STEP, nx, y0, MESH_STEP, ny, |x, y| {
        if in_cavity(x, y) { TOP - DEPTH } else { TOP }
    })
}

fn stock() -> StockConfig {
    StockConfig {
        x: 2.0 * (HALF_X + MARGIN),
        y: 2.0 * (HALF_Y + MARGIN),
        z: TOP,
        origin_x: -HALF_X - MARGIN,
        origin_y: -HALF_Y - MARGIN,
        origin_z: 0.0,
        auto_from_model: false,
        ..StockConfig::default()
    }
}

/// The final stock and the count of rapid collisions.
fn simulate(session: &mut ProjectSession) -> (Arc<TriDexelStock>, usize) {
    let _ = session
        .apply(Command::SetSimulationResolution(
            SetSimulationResolutionArgs {
                resolution: SimulationResolution::Fixed(CELL_MM),
            },
        ))
        .expect("a positive cell");
    let cancel = AtomicBool::new(false);
    let _ = session
        .run_simulation(
            &SimulationOptions {
                resolution: rs_cam_core::session::SimulationResolution::Fixed(CELL_MM),
                adaptive_feed_modulation: false,
                ..SimulationOptions::default()
            },
            &cancel,
        )
        .expect("simulation");
    let sim = session.simulation_result().expect("a simulation result");
    let stock = sim
        .checkpoints
        .last()
        .map(|c| Arc::clone(&c.mesh_stock))
        .expect("the simulation carved the op");
    (stock, sim.rapid_collisions.len())
}

/// 1. The cavity insets by R to one piece and by 2R to two pieces.
#[test]
fn the_cavity_is_one_region_whose_planner_inset_splits() {
    assert_eq!(
        offset_polygon(&cavity(), TOOL_R).len(),
        1,
        "the tool-centre region must be one region (neck wider than D)"
    );
    assert_eq!(
        offset_polygon(&cavity(), 2.0 * TOOL_R).len(),
        2,
        "the planner's inset of the region must split (neck narrower than 2D)"
    );
}

/// 2. AgentSearch clears both lobes.
#[test]
fn agent_search_clears_both_lobes() {
    assert_lobes_cleared(ClearingStrategy::AgentSearch);
}

/// 2. ContourSpiral clears both lobes.
#[test]
fn contour_spiral_clears_both_lobes() {
    assert_lobes_cleared(ClearingStrategy::ContourSpiral);
}

fn assert_lobes_cleared(strategy: ClearingStrategy) {
    let op = OperationConfig::Adaptive3d(Adaptive3dConfig {
        stock_to_leave_axial: LEAVE,
        clearing_strategy: strategy,
        ..Adaptive3dConfig::default()
    });
    let mut session = single_op_session(
        stock(),
        endmill_tool_config(TOOL_D),
        mesh_model(mesh(), "dumbbell_cavity"),
        "3D Rough",
        op,
    );
    generate(&mut session, 0);
    let (stock, rapid_collisions) = simulate(&mut session);
    assert_eq!(
        rapid_collisions, 0,
        "{strategy:?}: a rapid passes through stock (the move between the lobes)"
    );
    let grid = &stock.z_grid;
    let target = TOP - DEPTH + LEAVE;
    let mut tally = Vec::new();
    for side in [-1.0, 1.0] {
        let (mut judged, mut standing) = (0_usize, 0_usize);
        let mut worst = f64::NEG_INFINITY;
        for r in 0..grid.rows {
            for c in 0..grid.cols {
                let (x, y) = grid.cell_to_world(r, c);
                let xs = x * side;
                if xs < NECK_LEN / 2.0 + JUDGE_INSET
                    || xs > HALF_X - JUDGE_INSET
                    || y.abs() > HALF_Y - JUDGE_INSET
                {
                    continue;
                }
                judged += 1;
                let top = f64::from(grid.top_z_at(r, c).unwrap_or(f32::NEG_INFINITY));
                let above = top - target;
                worst = worst.max(above);
                if above > FRESH_MATERIAL_THRESHOLD_MM {
                    standing += 1;
                }
            }
        }
        eprintln!(
            "{strategy:?} lobe x {side:+}: {standing} of {judged} judged cells stand above \
             the target; worst {worst:.3} mm"
        );
        tally.push((side, judged, standing, worst));
    }
    for (side, judged, standing, worst) in tally {
        assert!(judged > 0, "{strategy:?} lobe x {side:+}: no judged cell");
        assert_eq!(
            standing, 0,
            "{strategy:?} lobe x {side:+}: material stands above the floor plus the leave in \
             {standing} of {judged} judged cells (worst {worst:.3} mm) — G-ADAPT3DPIECES"
        );
    }
}
