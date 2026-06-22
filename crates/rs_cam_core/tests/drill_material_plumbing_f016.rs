//! F-016 regression — the drill-op view built during session compute must
//! carry the live stock material (not `Material::default()`), so the
//! chip-welding / peck-adequacy / plunge-feed gates evaluate against the
//! actual workpiece.
//!
//! Pre-fix, `session::compute::generate_toolpath` passed
//! `Material::default()` (softwood, janka 600) into
//! `build_drill_op_for_config`, producing a `chip_welding_threshold` of
//! `8.0` regardless of stock material. This test sets the stock to a
//! hardwood species and asserts the resulting `DrillOp.material` is the
//! hardwood we configured.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::ids::ToolpathId;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::config::{BoundaryConfig, DressupConfig, HeightsConfig, StockSource};
use rs_cam_core::compute::operation_configs::{DrillConfig, DrillCycleType};
use rs_cam_core::compute::stock_config::StockConfig;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::debug_trace::ToolpathDebugOptions;
use rs_cam_core::drill_metrics::chip_welding_threshold;
use rs_cam_core::gcode::CoolantMode;
use rs_cam_core::geo::P2;
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{LoadedModel, ProjectSession, ToolpathConfig};

fn unit_square_at(cx: f64, cy: f64) -> Polygon2 {
    Polygon2::new(vec![
        P2::new(cx - 1.0, cy - 1.0),
        P2::new(cx + 1.0, cy - 1.0),
        P2::new(cx + 1.0, cy + 1.0),
        P2::new(cx - 1.0, cy + 1.0),
    ])
}

fn build_drill_session(material: Material) -> ProjectSession {
    let mut session = ProjectSession::new_empty();

    let stock = StockConfig {
        x: 100.0,
        y: 100.0,
        z: 20.0,
        origin_x: 0.0,
        origin_y: 0.0,
        origin_z: -20.0,
        auto_from_model: false,
        material,
        ..StockConfig::default()
    };
    session.set_stock_config(stock);

    let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
    tool.diameter = 4.0;
    let tool_idx = session.add_tool(tool);
    let tool_id = session.tools()[tool_idx].id.0;

    let model = LoadedModel {
        id: 0,
        name: "holes".to_owned(),
        mesh: None,
        polygons: Some(Arc::new(vec![unit_square_at(50.0, 50.0)])),
        drill_targets: std::sync::Arc::new(Vec::new()),
        layers: std::sync::Arc::new(Vec::new()),
        path: PathBuf::from("synthetic://f016_drill_holes.svg"),
        kind: None,
        units: None,
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
    };
    let model_id = session.add_model(model);

    let drill = DrillConfig {
        depth: 15.0,
        cycle: DrillCycleType::Peck,
        peck_depth: 3.0,
        feed_rate: 300.0,
        ..DrillConfig::default()
    };

    let tc = ToolpathConfig {
        id: ToolpathId(0),
        name: "Drill".to_owned(),
        enabled: true,
        operation: OperationConfig::Drill(drill),
        dressups: DressupConfig::for_op(OperationType::Drill),
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
    session.add_toolpath(0, tc).unwrap();

    session
}

fn generate_and_get_drill_material(session: &mut ProjectSession) -> Material {
    let cancel = AtomicBool::new(false);
    let result = session
        .generate_toolpath(0, &cancel)
        .expect("drill toolpath generates");
    let drill_op = result
        .op_data
        .drill_op()
        .expect("drill operation produces a DrillOp view");
    drill_op.material.clone()
}

#[test]
fn drill_op_carries_hardwood_material_from_stock() {
    let mut session = build_drill_session(Material::SolidWood {
        species: WoodSpecies::GenericHardwood,
    });
    let material = generate_and_get_drill_material(&mut session);
    assert_eq!(
        material,
        Material::SolidWood {
            species: WoodSpecies::GenericHardwood
        },
        "drill_op must carry the live stock material (F-016)"
    );
    // Threshold sanity: hardwood (janka 1450) must not return the
    // softwood 8.0 value that the bug surfaced in AS011.
    let threshold = chip_welding_threshold(&material);
    assert!(
        (threshold - 8.0).abs() > 1e-9,
        "hardwood threshold {threshold} must differ from softwood 8.0"
    );
}

#[test]
fn drill_op_carries_softwood_material_from_stock() {
    let mut session = build_drill_session(Material::SolidWood {
        species: WoodSpecies::GenericSoftwood,
    });
    let material = generate_and_get_drill_material(&mut session);
    assert_eq!(
        material,
        Material::SolidWood {
            species: WoodSpecies::GenericSoftwood
        }
    );
    assert!((chip_welding_threshold(&material) - 8.0).abs() < 1e-9);
}

#[test]
fn drill_op_carries_plastic_material_from_stock() {
    let mut session = build_drill_session(Material::Plastic {
        family: rs_cam_core::material::PlasticFamily::Acrylic,
    });
    let material = generate_and_get_drill_material(&mut session);
    assert!(matches!(material, Material::Plastic { .. }));
    assert!((chip_welding_threshold(&material) - 4.0).abs() < 1e-9);
}
