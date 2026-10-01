//! G-BOUNDARYPHANTOM (2026-10-01) — a 3D Rough with an `Inside`
//! containment boundary never rapids into stock along the containment line.
//!
//! ## The strike
//!
//! rivmap350 (the operator's project, not in the repo), "3D Rough 8":
//! Ø6.35 flat, ContourParallel, plunge entry, a `ModelOutline` boundary with
//! `holes = true` (the area inside an edge band), containment `Inside`.
//! Eight rapids ran straight down from the retract height into uncut stock
//! (4 to 11 mm), real at the 0.2, 0.25 and 0.125 mm cells. Many sat on one
//! line, one tool radius inside the band (y = 23.283).
//!
//! ## The cause
//!
//! The planner pre-clears the region outside the boundary and plans inside
//! the region edge. The session clips the generated path to the CONTAINMENT
//! polygon after generation: the region inset by the tool radius. The
//! planner planned and stamped the contour rings whose tool centre lay
//! between the region edge and the containment line (ContourParallel's
//! first ring is `min(r, stepover / 2)` from the edge); the clip deleted
//! them, but their stamps stayed in the planner stock. A later entry near
//! the containment line read that phantom cut as its rapid floor
//! (`clearing.rs::plan_entry`) and rapided into the band material the
//! shipped path had not cut.
//!
//! ## The fix
//!
//! The session hands the planner the clip's containment polygons
//! (`ExecutionContext::boundary_centre` ->
//! `Adaptive3dGeometry::centre_boundary`). The planner clips every segment
//! before it pushes and stamps it (`adaptive3d/centre_clip.rs`), so it
//! stamps only what the clip keeps.
//!
//! ## The fixture
//!
//! A 60 x 60 mm height field of ridges and valleys (z 1 to 11) in a
//! 64 x 64 x 25 stock. An edge-band drawing: the model square and an inner
//! square, nested as one ring as the DXF import nests it. Boundary
//! `ModelOutline { holes: true }`, containment `Inside`. Ø6.35 flat,
//! ContourParallel, Global, plunge, stepover 3, leave 0.5. Two arms: inner
//! half-width 17.3 with Depth/Pass 6, and 16.1 with Depth/Pass 4.
//!
//! ## What is asserted, per arm
//!
//! 1. The op has no rapid collision at the 0.25 and 0.125 mm cells.
//! 2. Every cutting move keeps the tool centre inside the containment
//!    square (inner half-width - 3.175), and some cutting move comes within
//!    1 mm of it (non-vacuity: the path works the band edge).
//!
//! Red before the fix (2026-10-01, `finish_3d.rs` passing an empty
//! `centre_boundary`): arm 17.3 / 6, one rapid at (-14.125, 7.103) from
//! Z 30 to 11.133, ON the containment line x = -14.125, at both cells; arm
//! 16.1 / 4, one rapid at (4.925, 12.006) to 7.978 at both cells. Green
//! after: 0 at both cells in both arms.
//!
//! ```text
//! cargo test -p rs_cam_core -q --test adaptive3d_boundary_clip_leaves_no_phantom_stock_g_boundaryphantom
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

use common::meshes::height_field;
use common::session::{mesh_model, polygon_model, square_polygon, stock_over, toolpath_config};
use common::tools::endmill_tool_config;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::{BoundaryConfig, BoundaryContainment, BoundarySource};
use rs_cam_core::compute::operation_configs::{
    Adaptive3dConfig, Adaptive3dEntryStyle, ClearingStrategy, RegionOrdering,
};
use rs_cam_core::polygon::detect_containment;
use rs_cam_core::session::{ProjectSession, ProjectSessionBuilder, SimulationOptions};
use rs_cam_core::toolpath::MoveType;

const PLATE_HALF: f64 = 30.0;
const STOCK_HEIGHT: f64 = 25.0;
const STEPOVER: f64 = 3.0;
const LEAVE: f64 = 0.5;
const TOOL_D: f64 = 6.35;
/// The clip puts its crossing points on the containment edge.
const EDGE_EPS_MM: f64 = 1e-3;

/// Ridges and valleys between z 1 and 11.
fn terrain(x: f64, y: f64) -> f64 {
    6.0 + 3.0 * (x / 4.3).sin() * (y / 5.1).cos() + 2.0 * ((x + y) / 3.7).sin()
}

/// One arm: the band's inner half-width and the Depth/Pass.
#[derive(Clone, Copy, Debug)]
struct Arm {
    band_inner_half: f64,
    depth_per_pass: f64,
}

/// Both arms rapided into stock before the fix, at both cells.
const ARMS: [Arm; 2] = [
    Arm {
        band_inner_half: 17.3,
        depth_per_pass: 6.0,
    },
    Arm {
        band_inner_half: 16.1,
        depth_per_pass: 4.0,
    },
];

fn session(arm: Arm) -> ProjectSession {
    let mut builder = ProjectSessionBuilder::new().stock(stock_over(PLATE_HALF, STOCK_HEIGHT));
    let tool_idx = builder.add_tool(endmill_tool_config(TOOL_D));
    let tool_id = builder.tools()[tool_idx].id.0;
    let mesh_id = builder.add_model(mesh_model(
        height_field(PLATE_HALF, 1.0, terrain),
        "terrain",
    ));
    let band = detect_containment(vec![
        square_polygon(PLATE_HALF),
        square_polygon(arm.band_inner_half),
    ]);
    assert_eq!(band.len(), 1, "the two squares nest as one ring");
    assert_eq!(band[0].holes.len(), 1, "the inner square is the hole");
    let band_id = builder.add_model(polygon_model(band, "machinable_edge_band"));

    let op = OperationConfig::Adaptive3d(Adaptive3dConfig {
        stepover: STEPOVER,
        depth_per_pass: arm.depth_per_pass,
        stock_to_leave_axial: LEAVE,
        entry_style: Adaptive3dEntryStyle::Plunge,
        region_ordering: RegionOrdering::Global,
        clearing_strategy: ClearingStrategy::ContourParallel,
        ..Adaptive3dConfig::default()
    });
    let mut cfg = toolpath_config("3D Rough", op, tool_id, mesh_id);
    cfg.boundary = BoundaryConfig {
        enabled: true,
        source: BoundarySource::ModelOutline {
            model_id: band_id,
            holes: true,
        },
        containment: BoundaryContainment::Inside,
        offset: 0.0,
    };
    cfg.boundary_inherit = false;
    builder.add_toolpath(0, cfg).expect("add the rough");
    let mut s = builder.build();
    let cancel = AtomicBool::new(false);
    s.generate_toolpath(0, &cancel)
        .expect("the rough generates");
    s
}

fn rapid_collisions(s: &mut ProjectSession, cell_mm: f64) -> usize {
    let cancel = AtomicBool::new(false);
    let opts = SimulationOptions {
        resolution: cell_mm,
        adaptive_feed_modulation: false,
        ..Default::default()
    };
    s.run_simulation(&opts, &cancel).expect("simulation");
    let sim = s.simulation_result().expect("a simulation result");
    for c in &sim.rapid_collisions {
        eprintln!(
            "cell {cell_mm}: rapid collision move {} {:?} -> {:?}",
            c.move_index, c.start, c.end
        );
    }
    sim.rapid_collisions.len()
}

#[test]
fn no_rapid_strikes_along_the_containment_line_g_boundaryphantom() {
    for arm in ARMS {
        check_arm(arm);
    }
}

fn check_arm(arm: Arm) {
    let mut s = session(arm);

    // (2) The path stays inside the containment square and reaches it.
    let limit = arm.band_inner_half - TOOL_D / 2.0;
    let moves = &s.get_result(0).expect("a result").toolpath().moves;
    let mut nearest_gap = f64::INFINITY;
    for m in moves {
        if matches!(m.move_type, MoveType::Rapid) {
            continue;
        }
        let reach = m.target.x.abs().max(m.target.y.abs());
        assert!(
            reach <= limit + EDGE_EPS_MM,
            "{arm:?}: a cutting move ends outside the containment square: {:?} \
             (limit {limit:.3})",
            m.target
        );
        nearest_gap = nearest_gap.min(limit - reach);
    }
    assert!(
        nearest_gap < 1.0,
        "{arm:?}: no cutting move comes within 1 mm of the containment line \
         ({nearest_gap:.3} mm): the fixture no longer exercises the band"
    );

    // (1) No rapid of the rough ends in stock.
    let coarse = rapid_collisions(&mut s, 0.25);
    let fine = rapid_collisions(&mut s, 0.125);
    eprintln!("{arm:?}: rapid collisions: 0.25 mm {coarse}, 0.125 mm {fine}");
    assert_eq!(
        (coarse, fine),
        (0, 0),
        "{arm:?}: the rough rapids into stock (0.25 mm, 0.125 mm) — G-BOUNDARYPHANTOM"
    );
}
