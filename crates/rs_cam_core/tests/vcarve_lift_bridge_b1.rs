//! PR-2A: V-carve lift-bridge B1 regression test (companion to
//! `pocket_lift_bridge_b1.rs`).
//!
//! V-carve shares the `apply_dressups` → `apply_entry` → `emit_ramp` /
//! `emit_helix` path with Pocket, so the pocket fix should cover it too.
//! This test asserts that empirically: a V-bit on a star-shaped polygon
//! over hardwood stock generates zero rapid-through-stock collisions.
//!
//! Repro mirrors the Phase B finding in
//! `planning/UX_DIALIN_REVIEW_2026-05-20.md`: 12.7 mm V-bit, 60° included
//! angle, hardwood, default Finish-role dressups (which provide a Ramp
//! entry and lead-in/out arcs).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::config::{BoundaryConfig, DressupConfig, HeightsConfig, StockSource};
use rs_cam_core::compute::operation_configs::VCarveConfig;
use rs_cam_core::compute::stock_config::StockConfig;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::debug_trace::ToolpathDebugOptions;
use rs_cam_core::gcode::CoolantMode;
use rs_cam_core::geo::P2;
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{LoadedModel, ProjectSession, SimulationOptions, ToolpathConfig};

/// 5-pointed star polygon (10 vertices, alternating outer/inner radius).
/// Matches the geometry of `fixtures/demo_star.svg` in spirit: a single
/// concave outline with no holes. Centered at (40, 40), outer radius 35.
fn five_point_star() -> Polygon2 {
    use std::f64::consts::PI;
    let cx = 40.0;
    let cy = 40.0;
    let r_outer = 35.0;
    let r_inner = 14.0;
    let mut pts = Vec::with_capacity(10);
    for i in 0..10 {
        // start at top (angle = -PI/2) and alternate
        let theta = -PI / 2.0 + (i as f64) * PI / 5.0;
        let r = if i % 2 == 0 { r_outer } else { r_inner };
        pts.push(P2::new(cx + r * theta.cos(), cy + r * theta.sin()));
    }
    Polygon2::new(pts)
}

fn make_vbit_12_7mm_60deg() -> ToolConfig {
    let mut tool = ToolConfig::new_default(ToolId(0), ToolType::VBit);
    tool.diameter = 12.7;
    tool.cutting_length = 11.0;
    tool.included_angle = 60.0;
    tool.shank_diameter = 6.35;
    tool.shank_length = 20.0;
    tool.stickout = 30.0;
    tool.flute_count = 2;
    tool.name = "V-bit 60deg".to_owned();
    tool
}

fn build_vcarve_session() -> ProjectSession {
    let mut session = ProjectSession::new_empty();

    // Matches `test_data/ux_2d_star.toml` stock — 120×120×12 hardwood,
    // origin -10,-10,-12 so the stock top sits at z=0.
    let stock = StockConfig {
        x: 120.0,
        y: 120.0,
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

    let tool_idx = session.add_tool(make_vbit_12_7mm_60deg());
    let tool_id = session.tools()[tool_idx].id.0;

    let polygon = five_point_star();
    let model = LoadedModel {
        id: 0,
        name: "demo_star".to_owned(),
        mesh: None,
        polygons: Some(Arc::new(vec![polygon])),
        path: PathBuf::from("synthetic://demo_star.svg"),
        kind: None,
        units: None,
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
    };
    let model_id = session.add_model(model);

    let vcarve = VCarveConfig {
        max_depth: 3.0,
        stepover: 0.5,
        feed_rate: 800.0,
        plunge_rate: 400.0,
        tolerance: 0.05,
        spindle_rpm: Some(18_000),
    };

    let tc = ToolpathConfig {
        id: 0,
        name: "VCarve".to_owned(),
        enabled: true,
        operation: OperationConfig::VCarve(vcarve),
        // Finish role → Ramp entry + lead_in_out + arc_fitting +
        // optimize_rapid_order. Without the Ramp entry the lift-bridge
        // bug doesn't fire.
        dressups: DressupConfig::for_op(OperationType::VCarve),
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
        feeds_provenance: rs_cam_core::feeds::FeedsProvenance::default(),
    };
    session.add_toolpath(0, tc).expect("add v_carve toolpath");

    session
}

#[test]
fn vcarve_default_skeleton_emits_no_rapid_collisions() {
    let mut session = build_vcarve_session();
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(0, &cancel)
        .expect("generate v_carve toolpath");

    // Sanity: the TP should have actual cut moves, otherwise "no
    // collisions" is a vacuous pass.
    let result = session.get_result(0).expect("v_carve produced a toolpath");
    assert!(
        result.toolpath().moves.len() > 50,
        "v_carve fixture should generate substantive toolpath (got {} moves)",
        result.toolpath().moves.len()
    );

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
        eprintln!("B1 v_carve repro: {count} rapid collisions");
        for c in sim.rapid_collisions.iter().take(8) {
            eprintln!(
                "  collision @ move {} : start ({:.2},{:.2},{:.3}) -> end ({:.2},{:.2},{:.3})",
                c.move_index, c.start.x, c.start.y, c.start.z, c.end.x, c.end.y, c.end.z,
            );
        }
    }

    assert_eq!(
        count, 0,
        "v_carve on default 12 mm hardwood stock with a 12.7 mm V-bit should not \
         generate rapid-through-stock collisions; got {count}",
    );
}
