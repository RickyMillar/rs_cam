//! G-CHAMFERINSIDE — a Chamfer bevels an inside edge into the material.
//!
//! # The ruling (W1.3b, operator 2026-10-14)
//!
//! Chamfer also cuts inside edges: the wall of a pocket and the edge of a
//! hole. `ChamferConfig::side` says where the material is. `Outside`: the
//! outline is a part, and the material is inside it. `Inside`: the outline
//! is a cavity, and the material is outside it. A hole ring of an `Outside`
//! outline is a hole in the part, so its edge is chamfered too. The bevel
//! goes into the material at every edge (`ops/chamfer.rs`, module doc).
//!
//! # The bevel rule (the same as G-CHAMFERSIDE)
//!
//! The tool centre runs `t` into the air side of the edge, at the depth
//! `(w + t) / tan(a)`. The bevel takes `w` from the top face and
//! `w / tan(a)` down the wall, where `a` is the half angle of the V-bit.
//!
//! # The fixtures
//!
//! The stock is a 40 mm square board, top at world Z0. A 3 mm end mill first
//! pockets the cavity `POCKET_DEPTH` deep, so the edge is a real wall. Then
//! the V-bit chamfers it. The session simulates both ops at `CELL_MM`.
//!
//! - (a) Pocket: a 20 mm square cavity, chamfered with `side = Inside`.
//! - (b) Hole: the part is the board outline with a round hole of radius
//!   `HOLE_R` as a hole ring. Chamfer runs with `side = Outside` on the part.
//!   The pocket op clears the hole from a separate circle outline.
//!
//! # The claims (for each fixture, with a 90° and a 60° bit)
//!
//! Each judged cell has a distance `d` into the material from the edge. A
//! line fit of the stock top `z` against `d`, over the cells inside the
//! bevel, gives the bevel:
//!
//! 1. its width on the top face (where the line meets Z0) equals `w` within
//!    one cell;
//! 2. its depth on the wall (the line at `d = 0`) equals `w / tan(a)` within
//!    one cell;
//! 3. its angle from the top face equals `90° - a` within `ANGLE_TOL_DEG`;
//! 4. the top face from `w` plus one cell to `BEYOND_MM` into the material
//!    stays whole.
//!
//! # Red proof (run once, 2026-10-14)
//!
//! - (a) with the side forced to `Outside`: the tip ran `t` into the
//!   material. Width 3.000 mm (`w + 2t`) for both bits; wall depth 3.000 mm
//!   (90°) and 5.196 mm (60°); the top face beyond the bevel cut 0.900 mm
//!   and 1.559 mm.
//! - (b) on the code before W1.3b (the exterior of the first offset result
//!   only): the hole edge had no bevel, 10 cells (90°) and 38 cells (60°).

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

use common::session::{generate, polygon_model, square_polygon, toolpath_config};
use rs_cam_core::compute::StockConfig;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::{ChamferConfig, PocketConfig, ProfileSide};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::dexel_stock::{FRESH_MATERIAL_THRESHOLD_MM, TriDexelStock};
use rs_cam_core::geo::P2;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{
    Command, ProjectSession, ProjectSessionBuilder, SetSimulationResolutionArgs, SimulationOptions,
    SimulationResolution,
};

/// Half side (mm) of the square board.
const BOARD_HALF: f64 = 20.0;

/// Board thickness (mm).
const STOCK_Z: f64 = 10.0;

/// Half side (mm) of the square pocket in fixture (a).
const POCKET_HALF: f64 = 10.0;

/// Radius (mm) of the round hole in fixture (b).
const HOLE_R: f64 = 8.0;

/// Segments of the hole circle. The chord error is `R (1 - cos(pi / n))`,
/// 0.08 µm at 720 segments.
const HOLE_SEGMENTS: usize = 720;

/// Depth (mm) of the cavity. It is deeper than the 60° bevel (4.33 mm).
const POCKET_DEPTH: f64 = 6.0;

/// Diameter (mm) of the end mill that cuts the cavity.
const END_MILL_D: f64 = 3.0;

/// Chamfer width (mm) and tip offset (mm).
const WIDTH: f64 = 2.0;
const TIP_OFFSET: f64 = 0.5;

/// Simulation cell (mm).
const CELL_MM: f64 = 0.1;

/// Cells nearer than this (mm) to a pocket corner, along the edge, are not
/// judged: the end mill rounds the cavity corners.
const CORNER_CLEAR: f64 = 3.0 * WIDTH;

/// The top face from `WIDTH + CELL_MM` to this distance (mm) into the
/// material must stay whole. It stays clear of the board's outer edge.
const BEYOND_MM: f64 = 6.0;

/// The tolerance (degrees) on the fitted bevel angle.
const ANGLE_TOL_DEG: f64 = 1.0;

#[derive(Clone, Copy, Debug)]
enum Fixture {
    /// (a) A square cavity chamfered with `side = Inside`.
    Pocket,
    /// (b) A part with a round hole chamfered with `side = Outside`.
    Hole,
}

impl Fixture {
    /// The side the operator picks for this edge.
    const fn side(self) -> ProfileSide {
        match self {
            Self::Pocket => ProfileSide::Inside,
            Self::Hole => ProfileSide::Outside,
        }
    }

    /// The distance (mm) into the material from the judged edge, or `None`
    /// for a cell that is not judged.
    fn depth_into_material(self, x: f64, y: f64) -> Option<f64> {
        match self {
            Self::Pocket => {
                let (near, along) = if x.abs() >= y.abs() {
                    (x.abs(), y.abs())
                } else {
                    (y.abs(), x.abs())
                };
                (along <= POCKET_HALF - CORNER_CLEAR).then_some(near - POCKET_HALF)
            }
            Self::Hole => Some(x.hypot(y) - HOLE_R),
        }
    }
}

/// The board is the part outline, so its sides are outside edges.
fn stock() -> StockConfig {
    StockConfig {
        x: 2.0 * BOARD_HALF,
        y: 2.0 * BOARD_HALF,
        z: STOCK_Z,
        origin_x: -BOARD_HALF,
        origin_y: -BOARD_HALF,
        origin_z: -STOCK_Z,
        auto_from_model: false,
        ..StockConfig::default()
    }
}

/// A circle of radius `r` about the origin, wound CCW.
fn circle(r: f64) -> Vec<P2> {
    (0..HOLE_SEGMENTS)
        .map(|i| {
            let a = std::f64::consts::TAU * i as f64 / HOLE_SEGMENTS as f64;
            P2::new(r * a.cos(), r * a.sin())
        })
        .collect()
}

fn vbit(included_angle_deg: f64) -> ToolConfig {
    ToolConfig {
        diameter: 12.7,
        included_angle: included_angle_deg,
        ..ToolConfig::new_default(ToolId(0), ToolType::VBit)
    }
}

fn end_mill() -> ToolConfig {
    ToolConfig {
        diameter: END_MILL_D,
        ..ToolConfig::new_default(ToolId(0), ToolType::EndMill)
    }
}

fn pocket_op() -> OperationConfig {
    OperationConfig::Pocket(PocketConfig {
        stepover: 1.0,
        depth: POCKET_DEPTH,
        depth_per_pass: 2.0,
        ..PocketConfig::default()
    })
}

fn chamfer_op(side: ProfileSide) -> OperationConfig {
    OperationConfig::Chamfer(ChamferConfig {
        side,
        chamfer_width: WIDTH,
        tip_offset: TIP_OFFSET,
        ..ChamferConfig::default()
    })
}

/// Two ops: the pocket that opens the cavity, then the chamfer.
fn session(fixture: Fixture, side: ProfileSide, included_angle_deg: f64) -> ProjectSession {
    let (cavity, part) = match fixture {
        Fixture::Pocket => {
            let cavity = square_polygon(POCKET_HALF);
            (cavity.clone(), cavity)
        }
        Fixture::Hole => {
            let mut hole = circle(HOLE_R);
            hole.reverse();
            let part = Polygon2::with_holes(square_polygon(BOARD_HALF).exterior, vec![hole]);
            (Polygon2::new(circle(HOLE_R)), part)
        }
    };
    let mut builder = ProjectSessionBuilder::new().stock(stock());
    let mill_idx = builder.add_tool(end_mill());
    let mill_id = builder.tools()[mill_idx].id.0;
    let vbit_idx = builder.add_tool(vbit(included_angle_deg));
    let vbit_id = builder.tools()[vbit_idx].id.0;
    let cavity_model = builder.add_model(polygon_model(vec![cavity], "cavity"));
    let part_model = builder.add_model(polygon_model(vec![part], "part"));
    let _ = builder
        .add_toolpath(
            0,
            toolpath_config("pocket", pocket_op(), mill_id, cavity_model),
        )
        .expect("add the pocket");
    let _ = builder
        .add_toolpath(
            0,
            toolpath_config("chamfer", chamfer_op(side), vbit_id, part_model),
        )
        .expect("add the chamfer");
    builder.build()
}

fn simulate(session: &mut ProjectSession) -> Arc<TriDexelStock> {
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
                resolution: CELL_MM,
                auto_resolution: false,
                adaptive_feed_modulation: false,
                ..SimulationOptions::default()
            },
            &cancel,
        )
        .expect("simulation");
    session
        .simulation_result()
        .and_then(|sim| sim.checkpoints.last().map(|c| Arc::clone(&c.mesh_stock)))
        .expect("the simulation carved the ops")
}

/// The bevel that one bit left on the material side of the judged edge.
struct Bevel {
    width: f64,
    depth: f64,
    angle_deg: f64,
    fitted_cells: usize,
    /// The deepest cut (mm below Z0) from `WIDTH + CELL_MM` to `BEYOND_MM`
    /// into the material.
    worst_beyond_cut: f64,
    /// The stock top (mm) at the centre of the cavity: the pocket floor.
    cavity_floor: f64,
}

fn measure(fixture: Fixture, side: ProfileSide, included_angle_deg: f64) -> Bevel {
    let mut session = session(fixture, side, included_angle_deg);
    generate(&mut session, 0);
    generate(&mut session, 1);
    let stock = simulate(&mut session);
    let grid = &stock.z_grid;

    let half_angle = (included_angle_deg / 2.0).to_radians();
    let expected_depth = WIDTH / half_angle.tan();

    // (d, z) of each cell inside the bevel, away from both of its ends.
    let mut samples: Vec<(f64, f64)> = Vec::new();
    let mut worst_beyond_cut: f64 = 0.0;
    for r in 0..grid.rows {
        for c in 0..grid.cols {
            let (x, y) = grid.cell_to_world(r, c);
            let Some(d) = fixture.depth_into_material(x, y) else {
                continue;
            };
            if !(0.0..=BEYOND_MM).contains(&d) {
                continue;
            }
            let Some(top) = grid.top_z_at(r, c) else {
                continue;
            };
            let z = f64::from(top);
            if d > WIDTH + CELL_MM {
                worst_beyond_cut = worst_beyond_cut.max((-z).max(0.0));
                continue;
            }
            if z < -0.15 * expected_depth && z > -0.85 * expected_depth {
                samples.push((d, z));
            }
        }
    }

    let (cr, cc) = grid
        .world_to_cell(0.0, 0.0)
        .expect("the cavity centre is on the grid");
    let cavity_floor = grid.top_z_at(cr, cc).map_or(f64::NAN, f64::from);

    // Least squares z = intercept + slope * d.
    let n = samples.len() as f64;
    let (sd, sz) = samples
        .iter()
        .fold((0.0, 0.0), |(a, b), &(d, z)| (a + d, b + z));
    let (md, mz) = (sd / n, sz / n);
    let (sdd, sdz) = samples.iter().fold((0.0, 0.0), |(a, b), &(d, z)| {
        (a + (d - md) * (d - md), b + (d - md) * (z - mz))
    });
    let slope = sdz / sdd;
    let intercept = mz - slope * md;
    Bevel {
        width: -intercept / slope,
        depth: -intercept,
        angle_deg: slope.atan().to_degrees(),
        fitted_cells: samples.len(),
        worst_beyond_cut,
        cavity_floor,
    }
}

fn assert_bevel_into_the_material(fixture: Fixture, included_angle_deg: f64) {
    let half_angle = (included_angle_deg / 2.0).to_radians();
    let expected_depth = WIDTH / half_angle.tan();
    let expected_angle = 90.0 - included_angle_deg / 2.0;
    let b = measure(fixture, fixture.side(), included_angle_deg);
    assert!(
        (b.cavity_floor + POCKET_DEPTH).abs() <= CELL_MM,
        "{fixture:?}: the pocket op did not open the cavity (floor {:.3} mm)",
        b.cavity_floor
    );
    eprintln!(
        "{fixture:?}, {included_angle_deg}° bit: {} bevel cells; width {:.3} mm (asked \
         {WIDTH}), wall depth {:.3} mm (expected {expected_depth:.3}), angle {:.2}° \
         (expected {expected_angle}), deepest cut beyond the bevel {:.3} mm",
        b.fitted_cells, b.width, b.depth, b.angle_deg, b.worst_beyond_cut
    );
    assert!(
        b.fitted_cells > 100,
        "{fixture:?}, {included_angle_deg}° bit: no bevel on the material side ({} cells)",
        b.fitted_cells
    );
    assert!(
        (b.width - WIDTH).abs() <= CELL_MM,
        "{fixture:?}, {included_angle_deg}° bit: bevel width on the top face {:.3} mm, \
         asked {WIDTH}",
        b.width
    );
    assert!(
        (b.depth - expected_depth).abs() <= CELL_MM,
        "{fixture:?}, {included_angle_deg}° bit: bevel depth on the wall {:.3} mm, \
         expected {expected_depth:.3}",
        b.depth
    );
    assert!(
        (b.angle_deg - expected_angle).abs() <= ANGLE_TOL_DEG,
        "{fixture:?}, {included_angle_deg}° bit: bevel angle {:.2}°, expected \
         {expected_angle}°",
        b.angle_deg
    );
    assert!(
        b.worst_beyond_cut <= FRESH_MATERIAL_THRESHOLD_MM,
        "{fixture:?}, {included_angle_deg}° bit: the top face beyond the bevel was cut \
         {:.3} mm",
        b.worst_beyond_cut
    );
}

#[test]
fn a_90_degree_bit_bevels_a_pocket_wall_at_45_degrees() {
    assert_bevel_into_the_material(Fixture::Pocket, 90.0);
}

#[test]
fn a_60_degree_bit_bevels_a_pocket_wall_at_60_degrees() {
    assert_bevel_into_the_material(Fixture::Pocket, 60.0);
}

#[test]
fn a_90_degree_bit_bevels_the_hole_in_a_part_at_45_degrees() {
    assert_bevel_into_the_material(Fixture::Hole, 90.0);
}

#[test]
fn a_60_degree_bit_bevels_the_hole_in_a_part_at_60_degrees() {
    assert_bevel_into_the_material(Fixture::Hole, 60.0);
}
