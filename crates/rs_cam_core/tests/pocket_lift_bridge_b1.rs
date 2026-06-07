//! PR-2A: Pocket lift-bridge B1 regression test.
//!
//! Reproduces the "rapid collisions on default-LUT skeleton pocket" pain
//! reported in `planning/UX_DIALIN_REVIEW_2026-05-20.md` Phase B (28
//! collisions on the `ux_2d_pocket` fixture exercised via MCP).
//!
//! Builds the equivalent of the ux_2d_pocket fixture programmatically:
//! - 100×100×12 mm hardwood stock (origin -10,-10,-12 → top at z=0).
//! - 70×50 mm outer rectangle with a Ø20 mm circular island (mirrors
//!   the `fixtures/demo_pocket.svg` shape with one polygon-with-hole).
//! - 6 mm flat end mill.
//! - PocketConfig with depth=12, depth_per_pass=4.2 (the LUT-suggested
//!   value the review observed on a 6 mm EM in hardwood).
//! - Pocket dressups via `DressupConfig::for_op(OperationType::Pocket)`,
//!   which is what `add_toolpath` actually wires up (Ramp entry + link
//!   moves + arc fitting + TSP rapid-order optimization).
//!
//! Pre-fix the ramp dressup descended from `safe_z` straight to
//! `cut_depth + 2 mm` via a rapid, punching through uncut stock at every
//! new contour entry (42 collisions observed in this scaled-up repro).
//!
//! The fix plumbs `stock_top` from the simulator's bbox down through
//! `apply_dressups` → `apply_entry` → `emit_ramp` / `emit_helix`; the
//! initial descent is now split into a rapid (down to `stock_top +
//! clearance`, still in air) followed by a plunge-feed to the ramp /
//! helix start.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

mod common;
use common::make_endmill_6mm;

use std::f64::consts::TAU;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::config::{BoundaryConfig, DressupConfig, HeightsConfig, StockSource};
use rs_cam_core::compute::operation_configs::{PocketConfig, PocketPattern};
use rs_cam_core::compute::stock_config::StockConfig;
use rs_cam_core::debug_trace::ToolpathDebugOptions;
use rs_cam_core::gcode::CoolantMode;
use rs_cam_core::geo::P2;
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{LoadedModel, ProjectSession, SimulationOptions, ToolpathConfig};

fn rounded_rect_with_island() -> Polygon2 {
    let exterior = vec![
        P2::new(5.0, 5.0),
        P2::new(75.0, 5.0),
        P2::new(75.0, 55.0),
        P2::new(5.0, 55.0),
    ];

    let mut hole = Vec::with_capacity(64);
    let cx = 40.0;
    let cy = 30.0;
    let r = 10.0;
    let n = 64;
    for i in 0..n {
        let t = (i as f64) * TAU / (n as f64);
        // CW for holes (negative area)
        hole.push(P2::new(cx + r * (-t).cos(), cy + r * (-t).sin()));
    }

    Polygon2::with_holes(exterior, vec![hole])
}

fn build_pocket_session() -> ProjectSession {
    let mut session = ProjectSession::new_empty();

    let stock = StockConfig {
        x: 100.0,
        y: 100.0,
        z: 12.0,
        origin_x: -10.0,
        origin_y: -10.0,
        origin_z: -12.0,
        auto_from_model: false,
        material: Material::SolidWood {
            species: WoodSpecies::GenericHardwood,
        },
        ..StockConfig::default()
    };
    session.set_stock_config(stock);

    let tool_idx = session.add_tool(make_endmill_6mm());
    let tool_id = session.tools()[tool_idx].id.0;

    let polygon = rounded_rect_with_island();
    let model = LoadedModel {
        id: 0,
        name: "demo_pocket".to_owned(),
        mesh: None,
        polygons: Some(Arc::new(vec![polygon])),
        path: PathBuf::from("synthetic://demo_pocket.svg"),
        kind: None,
        units: None,
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
    };
    let model_id = session.add_model(model);

    let pocket = PocketConfig {
        stepover: 2.0,
        depth: 12.0,
        depth_per_pass: 4.2,
        feed_rate: 770.0,
        plunge_rate: 385.0,
        climb: true,
        pattern: PocketPattern::Contour,
        angle: 0.0,
        finishing_passes: 0,
        spindle_rpm: Some(18_000),
    };

    let tc = ToolpathConfig {
        id: 0,
        name: "Pocket".to_owned(),
        enabled: true,
        operation: OperationConfig::Pocket(pocket),
        // Matches the MCP / GUI `add_toolpath` defaults — Roughing role
        // dressups (Ramp entry + link moves + arc fit + TSP). Without the
        // Ramp entry the ramp-descent bug doesn't fire.
        dressups: DressupConfig::for_op(OperationType::Pocket),
        heights: HeightsConfig::default(),
        tool_id,
        model_id,
        pre_gcode: None,
        post_gcode: None,
        boundary: BoundaryConfig::default(),
        boundary_inherit: true,
        stock_source: StockSource::default(),
        coolant: CoolantMode::Off,
        face_selection: None,
        debug_options: ToolpathDebugOptions::default(),
    };
    session.add_toolpath(0, tc).expect("add pocket toolpath");

    session
}

#[test]
fn pocket_default_skeleton_emits_no_rapid_collisions() {
    let mut session = build_pocket_session();
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(0, &cancel)
        .expect("generate pocket toolpath");

    let opts = SimulationOptions {
        resolution: 1.0,
        skip_ids: Vec::new(),
        metrics_enabled: true,
        auto_resolution: false,
        use_predicted_feed_in_gates: false,
        adaptive_feed_modulation: false,
        modulation_strategy: rs_cam_core::feed_modulation::ModulationStrategy::ConstrainedMax,
        modulation_aggressiveness: 1.0,
    };
    session
        .run_simulation(&opts, &cancel)
        .expect("simulation completes");

    let sim = session.simulation_result().expect("simulation result");
    let count = sim.rapid_collisions.len();

    if count > 0 {
        eprintln!("B1 pocket repro: {count} rapid collisions");
        for c in sim.rapid_collisions.iter().take(8) {
            eprintln!(
                "  collision @ move {} : start ({:.2},{:.2},{:.3}) -> end ({:.2},{:.2},{:.3})",
                c.move_index, c.start.x, c.start.y, c.start.z, c.end.x, c.end.y, c.end.z,
            );
        }
    }

    assert_eq!(
        count, 0,
        "pocket on default 12 mm hardwood stock with safe_z=10 should not generate \
         rapid-through-stock collisions; got {count}",
    );
}
