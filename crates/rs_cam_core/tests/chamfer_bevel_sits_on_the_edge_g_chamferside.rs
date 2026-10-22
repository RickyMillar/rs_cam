//! G-CHAMFERSIDE — a Chamfer bevel sits on the edge it chamfers.
//!
//! # The suspect (D4)
//!
//! `planning/op_architecture_2026-10-14/AUDIT.md` lists D4: Chamfer offsets
//! the outline outward by `w + t` (chamfer width plus tip offset) and cuts at
//! the depth `(w + t) / tan(a)`. At the stock top the cone radius is then
//! `w + t`, so the cone touches the outline at one line and the bevel lies
//! outside it. The doc said that the chamfer extends `w` mm inward.
//!
//! # What a woodworker expects
//!
//! A chamfer of width `w` on the outside edge of a part is a flat bevel.
//! It starts `w` mm in from the edge on the top face, and it meets the wall
//! `w / tan(a)` mm below the top, where `a` is the half angle of the V-bit.
//! A 90° bit gives a 45° bevel that takes `w` from the top face and `w` down
//! the wall. A 60° bit gives a bevel 60° from the top face that takes `w`
//! from the top face and `w / tan(30°)` down the wall. The tip offset `t`
//! keeps the tip `t` mm outside the wall; it does not move the bevel.
//!
//! # The fixture
//!
//! The stock is the outline: a 30 mm square board, top at world Z0. Every
//! side of the outline is a real outside edge with a wall below it. Chamfer
//! runs once with each bit through the session, and the session simulates it
//! at `CELL_MM`.
//!
//! # The claims (for each bit)
//!
//! Each cell away from the corners has an inward distance `d` from its
//! nearest edge. A line fit of the stock top `z` against `d`, over the cells
//! inside the bevel, gives the bevel:
//!
//! 1. its width on the top face (where the line meets Z0) equals `w` within
//!    one cell;
//! 2. its depth on the wall (the line at `d = 0`) equals `w / tan(a)` within
//!    one cell;
//! 3. its angle from the top face equals `90° - a` within `ANGLE_TOL_DEG`;
//! 4. the top face more than `w` plus one cell from the edge stays whole.
//!
//! Before the fix the bevel did not exist: the cone met the board only at
//! the edge line, and no cell inside the outline was cut.

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

use common::session::{generate, polygon_model, single_op_session, square_polygon};
use rs_cam_core::compute::StockConfig;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::ChamferConfig;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::dexel_stock::{FRESH_MATERIAL_THRESHOLD_MM, TriDexelStock};
use rs_cam_core::session::{
    Command, ProjectSession, SetSimulationResolutionArgs, SimulationOptions, SimulationResolution,
};

/// Half side (mm) of the square board.
const HALF: f64 = 15.0;

/// Board thickness (mm).
const STOCK_Z: f64 = 10.0;

/// Chamfer width (mm) and tip offset (mm).
const WIDTH: f64 = 2.0;
const TIP_OFFSET: f64 = 0.5;

/// Simulation cell (mm).
const CELL_MM: f64 = 0.1;

/// Cells nearer than this (mm) to a corner, along the edge, are not judged:
/// the outward offset rounds the corners.
const CORNER_CLEAR: f64 = 3.0 * WIDTH;

/// The tolerance (degrees) on the fitted bevel angle.
const ANGLE_TOL_DEG: f64 = 1.0;

/// The board is the outline, so its sides are outside edges with walls.
fn stock() -> StockConfig {
    StockConfig {
        x: 2.0 * HALF,
        y: 2.0 * HALF,
        z: STOCK_Z,
        origin_x: -HALF,
        origin_y: -HALF,
        origin_z: -STOCK_Z,
        auto_from_model: false,
        ..StockConfig::default()
    }
}

fn vbit(included_angle_deg: f64) -> ToolConfig {
    ToolConfig {
        diameter: 12.7,
        included_angle: included_angle_deg,
        ..ToolConfig::new_default(ToolId::from_raw(0), ToolType::VBit)
    }
}

fn chamfer() -> OperationConfig {
    OperationConfig::Chamfer(ChamferConfig {
        chamfer_width: WIDTH,
        tip_offset: TIP_OFFSET,
        ..ChamferConfig::default()
    })
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
                resolution: rs_cam_core::session::SimulationResolution::Fixed(CELL_MM),
                adaptive_feed_modulation: false,
                ..SimulationOptions::default()
            },
            &cancel,
        )
        .expect("simulation");
    session
        .simulation_result()
        .and_then(|sim| sim.checkpoints.last().map(|c| Arc::clone(&c.mesh_stock)))
        .expect("the simulation carved the op")
}

/// The bevel that one bit left on the board.
struct Bevel {
    width: f64,
    depth: f64,
    angle_deg: f64,
    fitted_cells: usize,
    /// The deepest cut (mm below Z0) more than `WIDTH + CELL_MM` from an edge.
    worst_inner_cut: f64,
}

fn measure(included_angle_deg: f64) -> Bevel {
    let mut session = single_op_session(
        stock(),
        vbit(included_angle_deg),
        polygon_model(vec![square_polygon(HALF)], "board"),
        "chamfer",
        chamfer(),
    );
    generate(&mut session, 0);
    let stock = simulate(&mut session);
    let grid = &stock.z_grid;

    let half_angle = (included_angle_deg / 2.0).to_radians();
    let expected_depth = WIDTH / half_angle.tan();

    // (d, z) of each cell inside the bevel, away from both of its ends.
    let mut samples: Vec<(f64, f64)> = Vec::new();
    let mut worst_inner_cut: f64 = 0.0;
    for r in 0..grid.rows {
        for c in 0..grid.cols {
            let (x, y) = grid.cell_to_world(r, c);
            let (near, along) = if x.abs() >= y.abs() {
                (x.abs(), y.abs())
            } else {
                (y.abs(), x.abs())
            };
            let d = HALF - near;
            if d < 0.0 || along > HALF - CORNER_CLEAR {
                continue;
            }
            let Some(top) = grid.top_z_at(r, c) else {
                continue;
            };
            let z = f64::from(top);
            if d > WIDTH + CELL_MM {
                worst_inner_cut = worst_inner_cut.max(-z);
                continue;
            }
            if z < -0.15 * expected_depth && z > -0.85 * expected_depth {
                samples.push((d, z));
            }
        }
    }

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
        worst_inner_cut,
    }
}

fn assert_bevel_on_the_edge(included_angle_deg: f64) {
    let half_angle = (included_angle_deg / 2.0).to_radians();
    let expected_depth = WIDTH / half_angle.tan();
    let expected_angle = 90.0 - included_angle_deg / 2.0;
    let b = measure(included_angle_deg);
    eprintln!(
        "{included_angle_deg}° bit: {} bevel cells; width {:.3} mm (asked {WIDTH}), \
         wall depth {:.3} mm (expected {expected_depth:.3}), angle {:.2}° (expected \
         {expected_angle}), deepest cut beyond the bevel {:.3} mm",
        b.fitted_cells, b.width, b.depth, b.angle_deg, b.worst_inner_cut
    );
    assert!(
        b.fitted_cells > 100,
        "{included_angle_deg}° bit: no bevel inside the outline ({} cells)",
        b.fitted_cells
    );
    assert!(
        (b.width - WIDTH).abs() <= CELL_MM,
        "{included_angle_deg}° bit: bevel width on the top face {:.3} mm, asked {WIDTH}",
        b.width
    );
    assert!(
        (b.depth - expected_depth).abs() <= CELL_MM,
        "{included_angle_deg}° bit: bevel depth on the wall {:.3} mm, expected \
         {expected_depth:.3}",
        b.depth
    );
    assert!(
        (b.angle_deg - expected_angle).abs() <= ANGLE_TOL_DEG,
        "{included_angle_deg}° bit: bevel angle {:.2}°, expected {expected_angle}°",
        b.angle_deg
    );
    assert!(
        b.worst_inner_cut <= FRESH_MATERIAL_THRESHOLD_MM,
        "{included_angle_deg}° bit: the top face beyond the bevel was cut {:.3} mm",
        b.worst_inner_cut
    );
}

#[test]
fn a_90_degree_bit_bevels_the_edge_at_45_degrees() {
    assert_bevel_on_the_edge(90.0);
}

#[test]
fn a_60_degree_bit_bevels_the_edge_at_60_degrees() {
    assert_bevel_on_the_edge(60.0);
}
