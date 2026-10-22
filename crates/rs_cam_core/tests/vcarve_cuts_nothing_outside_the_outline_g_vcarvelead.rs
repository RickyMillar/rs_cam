//! G-VCARVELEAD — VCarve cuts nothing outside the design outline (W1.4b).
//!
//! # The defect
//!
//! W1.4 (G-INLAYSPLIT) found that VCarve kept the Finish-role lead-in. A
//! lead arc at the start of a run takes the V-bit outside the design:
//! 238 cells, up to 0.75 mm outside, 0.060 mm deep. Inlay had the same
//! defect, and its registry row now strips leads and entries.
//!
//! # Why VCarve needs no lead
//!
//! VCarve cuts scan lines across the design. The depth at each point is
//! `distance to the outline / tan(half angle)`, so a run starts and ends
//! on the outline at almost zero depth. The scan lines are inset by
//! `min(tolerance, 0.05)` mm, so the first cut point of a run is
//! `inset / tan(half angle)` deep. There the cone of the V-bit has the
//! radius `inset`, which is the distance to the outline. The plunge
//! straight down to that point is therefore contained. Claim (b) measures
//! this on the emitted moves.
//!
//! # Red proof
//!
//! With the old row (`DressupPolicy::ANY_DRESSUP`, the Finish-role lead-in
//! on) on this fixture, 2026-10-14: claim (a) failed with **2 227 cells cut
//! outside the outline + one cell, up to 2.147 mm out, 0.113 mm deep**, and
//! claim (b) failed on the first lead arc. With the stripped row: 0 cells,
//! and the cone reaches 0.000 mm past the outline.
//!
//! # The claims
//!
//! - (a) The simulated stock holds no cut cell outside the design plus one
//!   cell.
//! - (b) Every emitted cutting point lies in the design, and the V-bit cone
//!   at it (radius = depth x tan(half angle)) does not reach past the
//!   outline. The first cut point of each run is at most
//!   `inset / tan(half angle)` deep.

#![expect(
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]
#![expect(clippy::print_stderr, reason = "The measured numbers ARE the record.")]

mod common;

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use common::session::{generate, polygon_model, single_op_session};
use rs_cam_core::compute::StockConfig;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::VCarveConfig;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::dexel_stock::TriDexelStock;
use rs_cam_core::geo::P2;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{
    Command, ProjectSession, SetSimulationResolutionArgs, SimulationOptions, SimulationResolution,
};
use rs_cam_core::toolpath::MoveType;

/// Simulation cell (mm). The defect cut up to 0.75 mm outside the outline,
/// and the margin of claim (a) is one cell, so a 0.1 mm cell resolves an
/// overcut from 0.1 mm out. The board is 40 x 40 mm: 400 x 400 cells.
const CELL_MM: f64 = 0.1;

/// A cell counts as cut when its top is this far below the stock top (mm).
/// An uncut cell reads the stock top exactly; the defect cut 0.060 mm deep.
const CUT_MM: f64 = 0.01;

/// Slack of claim (b), in mm. The emitted depth is the distance-field depth
/// at the same point, so the cone meets the outline to rounding.
const EMIT_SLACK_MM: f64 = 1e-6;

/// The L: a 30 x 15 bar along the bottom and a 15 x 15 upright on the left.
/// It is not symmetric about either axis.
fn l_design() -> Polygon2 {
    Polygon2::new(vec![
        P2::new(-15.0, -15.0),
        P2::new(15.0, -15.0),
        P2::new(15.0, 0.0),
        P2::new(0.0, 0.0),
        P2::new(0.0, 15.0),
        P2::new(-15.0, 15.0),
    ])
}

fn board() -> StockConfig {
    StockConfig {
        x: 40.0,
        y: 40.0,
        z: 12.0,
        origin_x: -20.0,
        origin_y: -20.0,
        origin_z: -12.0,
        auto_from_model: false,
        ..StockConfig::default()
    }
}

/// A 90 deg V-bit: half angle 45 deg.
fn vbit() -> ToolConfig {
    ToolConfig {
        diameter: 12.7,
        included_angle: 90.0,
        ..ToolConfig::new_default(ToolId::from_raw(0), ToolType::VBit)
    }
}

/// The default config with a coarser scan (1 mm lines, 0.1 mm samples), so
/// the 0.1 mm simulation stays fast. Every run still starts on the outline,
/// where the lead-in was.
fn vcarve_config() -> VCarveConfig {
    VCarveConfig {
        stepover: 1.0,
        tolerance: 0.1,
        ..VCarveConfig::default()
    }
}

fn vcarve_session() -> ProjectSession {
    let mut session = single_op_session(
        board(),
        vbit(),
        polygon_model(vec![l_design()], "l"),
        "vcarve",
        OperationConfig::VCarve(vcarve_config()),
    );
    generate(&mut session, 0);
    session
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

// ── (a) the simulated stock ──────────────────────────────────────────────

/// The red proof numbers are in the module doc.
#[test]
fn a_vcarve_cuts_no_cell_outside_the_outline() {
    let mut session = vcarve_session();
    let stock = simulate(&mut session);
    let design = l_design();
    let grid = &stock.z_grid;

    let (mut inside_cut, mut outside_cut) = (0usize, 0usize);
    let (mut worst_depth, mut worst_out) = (0.0_f64, 0.0_f64);
    for r in 0..grid.rows {
        for c in 0..grid.cols {
            let (x, y) = grid.cell_to_world(r, c);
            let Some(top) = grid.top_z_at(r, c) else {
                continue;
            };
            let depth = -f64::from(top);
            if depth < CUT_MM {
                continue;
            }
            let p = P2::new(x, y);
            if design.contains_point(&p) {
                inside_cut += 1;
                continue;
            }
            let out = design.distance_to_boundary(&p);
            if out <= CELL_MM {
                continue;
            }
            outside_cut += 1;
            worst_depth = worst_depth.max(depth);
            worst_out = worst_out.max(out);
        }
    }
    eprintln!(
        "(a) cut cells inside = {inside_cut}; outside the outline + one cell = \
         {outside_cut}, up to {worst_out:.3} mm out, {worst_depth:.3} mm deep"
    );
    // Non-vacuity: the V-carve cut the design.
    assert!(
        inside_cut > 5_000,
        "vacuous: only {inside_cut} cells cut inside the design"
    );
    assert_eq!(
        outside_cut, 0,
        "VCarve cut {outside_cut} cells outside the outline, up to {worst_out:.3} mm \
         out and {worst_depth:.3} mm deep"
    );
}

// ── (b) the emitted moves ────────────────────────────────────────────────

#[test]
fn b_every_cut_point_keeps_the_cone_inside_the_outline() {
    let session = vcarve_session();
    let design = l_design();
    let cfg = vcarve_config();
    let tan_half = 45.0_f64.to_radians().tan();
    // The stock top of this board is Z = 0.
    let top_z = 0.0;
    // `ops::vcarve`: the scan lines are inset by min(tolerance, 0.05) mm.
    let inset = cfg.tolerance.min(0.05);

    let toolpath = session
        .result_at(0)
        .expect("the op generated")
        .toolpath()
        .clone();
    let (mut cut_points, mut runs) = (0usize, 0usize);
    let mut worst_reach = f64::NEG_INFINITY;
    let mut worst_first_depth = 0.0_f64;
    let mut after_rapid = true;
    for mv in &toolpath.moves {
        match mv.move_type {
            MoveType::Rapid => after_rapid = true,
            MoveType::Linear { .. } => {
                let depth = top_z - mv.target.z;
                if depth <= 0.0 {
                    continue;
                }
                let p = P2::new(mv.target.x, mv.target.y);
                assert!(
                    design.contains_point(&p),
                    "a cut point at ({:.3}, {:.3}) is outside the design",
                    p.x,
                    p.y
                );
                // How far the cone reaches past the outline (negative when
                // it stays inside).
                let reach = depth * tan_half - design.distance_to_boundary(&p);
                worst_reach = worst_reach.max(reach);
                cut_points += 1;
                if after_rapid {
                    runs += 1;
                    worst_first_depth = worst_first_depth.max(depth);
                    after_rapid = false;
                }
            }
            MoveType::ArcCW { .. } | MoveType::ArcCCW { .. } => {
                panic!("VCarve emits no arc; a lead arc is the defect")
            }
        }
    }
    eprintln!(
        "(b) {cut_points} cut points in {runs} runs; the cone reaches {worst_reach:.6} mm \
         past the outline at worst; the deepest run start is {worst_first_depth:.4} mm"
    );
    assert!(runs > 10, "vacuous: only {runs} runs");
    assert!(
        worst_reach <= EMIT_SLACK_MM,
        "the cone reaches {worst_reach:.6} mm past the outline"
    );
    assert!(
        worst_first_depth <= inset / tan_half + EMIT_SLACK_MM,
        "a run starts {worst_first_depth:.4} mm deep; a V-carve run starts at \
         inset / tan(half angle) = {:.4} mm",
        inset / tan_half
    );
}
