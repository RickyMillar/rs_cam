//! PR-2A: Adaptive3d lift-bridge B1 regression test (companion to
//! `pocket_lift_bridge_b1.rs` and `vcarve_lift_bridge_b1.rs`).
//!
//! adaptive3d emits its own entry rapids via `path.rs` (4 emit call sites
//! patched in PR-2A) instead of going through `apply_entry`. Those sites
//! now receive `stock_top` (or a dexel-sampled descent floor) so the
//! initial descent rapid stops at clear-air rather than driving into
//! uncut stock.
//!
//! This test exercises that path: a synthetic hemisphere mesh stood up
//! inside a block of hardwood, then roughed with adaptive3d + Helix
//! entry. Pre-fix, the helix descent rapid punched through the unbeated
//! shoulder of the stock at every new region; post-fix it should report
//! zero rapid collisions.

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
use rs_cam_core::compute::operation_configs::{
    Adaptive3dConfig, Adaptive3dEntryStyle, ClearingStrategy, RegionOrdering,
};
use rs_cam_core::compute::stock_config::StockConfig;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::debug_trace::ToolpathDebugOptions;
use rs_cam_core::gcode::CoolantMode;
use rs_cam_core::geo::P3;
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::mesh::{TriangleMesh, make_test_hemisphere};
use rs_cam_core::session::{LoadedModel, ProjectSession, SimulationOptions, ToolpathConfig};

/// Translate a mesh so its bounding box minimum sits at the given point.
fn translate_mesh(mut mesh: TriangleMesh, dx: f64, dy: f64, dz: f64) -> TriangleMesh {
    for v in &mut mesh.vertices {
        *v = P3::new(v.x + dx, v.y + dy, v.z + dz);
    }
    mesh
}

fn make_endmill_6mm() -> ToolConfig {
    let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
    tool.diameter = 6.0;
    tool.cutting_length = 25.0;
    tool.shank_diameter = 6.35;
    tool.shank_length = 20.0;
    tool.stickout = 45.0;
    tool.flute_count = 2;
    tool.name = "End Mill 6mm".to_owned();
    tool
}

fn build_adaptive3d_session() -> ProjectSession {
    let mut session = ProjectSession::new_empty();

    // Hemisphere radius 15 → mesh bbox (−15..15, −15..15, 0..15).
    // Place stock so the hemisphere is centered at (25,25,0) with the
    // stock top at z=25 (10 mm of stock above the hemisphere top).
    let raw = make_test_hemisphere(15.0, 12);
    let mesh = translate_mesh(raw, 25.0, 25.0, 0.0);

    let stock = StockConfig {
        x: 50.0,
        y: 50.0,
        z: 25.0,
        origin_x: 0.0,
        origin_y: 0.0,
        origin_z: 0.0,
        auto_from_model: false,
        material: Material::SolidWood {
            species: WoodSpecies::GenericHardwood,
        },
        ..StockConfig::default()
    };
    session.set_stock_config(stock);

    let tool_idx = session.add_tool(make_endmill_6mm());
    let tool_id = session.tools()[tool_idx].id.0;

    let model = LoadedModel {
        id: 0,
        name: "hemisphere".to_owned(),
        mesh: Some(Arc::new(mesh)),
        polygons: None,
        path: PathBuf::from("synthetic://hemisphere.stl"),
        kind: None,
        units: None,
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
    };
    let model_id = session.add_model(model);

    // Helix entry — that's the variant adaptive3d's `emit` path now
    // protects against descent-through-stock (4 sites in
    // `adaptive3d/path.rs`). Default depth_per_pass=3 with stock_to_leave
    // ≈ 0.5 gives ~8 Z-levels to bridge between, plenty of opportunity
    // for the bug to fire if the fix regresses.
    let cfg = Adaptive3dConfig {
        entry_style: Adaptive3dEntryStyle::Helix,
        helix_radius_factor: 0.4,
        helix_pitch: 1.0,
        region_ordering: RegionOrdering::Global,
        clearing_strategy: ClearingStrategy::ContourParallel,
        ..Adaptive3dConfig::default()
    };

    let tc = ToolpathConfig {
        id: 0,
        name: "Adaptive3d".to_owned(),
        enabled: true,
        operation: OperationConfig::Adaptive3d(cfg),
        dressups: DressupConfig::for_op(OperationType::Adaptive3d),
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
    session
        .add_toolpath(0, tc)
        .expect("add adaptive3d toolpath");

    session
}

#[test]
fn adaptive3d_default_skeleton_emits_no_rapid_collisions() {
    let mut session = build_adaptive3d_session();
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(0, &cancel)
        .expect("generate adaptive3d toolpath");

    let result = session
        .get_result(0)
        .expect("adaptive3d produced a toolpath");
    assert!(
        result.toolpath().moves.len() > 200,
        "adaptive3d fixture should generate substantive toolpath (got {} moves)",
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
        eprintln!("B1 adaptive3d repro: {count} rapid collisions");
        for c in sim.rapid_collisions.iter().take(8) {
            eprintln!(
                "  collision @ move {} : start ({:.2},{:.2},{:.3}) -> end ({:.2},{:.2},{:.3})",
                c.move_index, c.start.x, c.start.y, c.start.z, c.end.x, c.end.y, c.end.z,
            );
        }
    }

    assert_eq!(
        count, 0,
        "adaptive3d on a hemisphere over 25 mm hardwood stock should not \
         generate rapid-through-stock collisions; got {count}",
    );
}
