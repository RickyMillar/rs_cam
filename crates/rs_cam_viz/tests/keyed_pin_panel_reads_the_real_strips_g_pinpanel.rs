//! G-PINPANEL: the pin wizard's "Auto-place" (the stock panel's "Auto-place
//! keyed pins" before round 2) reads the real
//! clear strips, not the stock padding.
//!
//! # What the finding was
//!
//! The panel planned the pins with `model_x_range = (padding, x - padding)`.
//! The core placer's own doc (`place_keyed_pins`, step 2) says padding
//! describes how the stock would be auto-sized, not how big it is. The live
//! stock was 380 mm wide at origin x -15 around a model at x 0..350, with a
//! padding of 5. The real strips are 15 mm each. The panel said "the clear
//! strip on -X is 5.0 mm" and greyed the button, while the "Two-sided
//! setup" route placed the pins.
//!
//! # What this test measures
//!
//! `ui::properties::pins::keyed_pin_plan` is the one planner both doors
//! call, with `KeyedPinInputs::from_session`. The arms:
//!
//! 1. the live 380 / -15 / 0..350 x 0..480 stock places the axis pin at
//!    x 372.5 (the centre of the 15 mm +X strip) on the mirror line
//!    y = 255, although the padding is 5 (P1, 2026-10-12: three pins,
//!    because two equal pins never key). P6 (2026-10-12) moved the pair:
//!    the 2D placer also reads the 15 mm Y rims, and the pair now sits at
//!    (8.5, 11) and (8.5, 499), in the -X strip near its corners. Its walls
//!    are 4.6 mm to the part and 5.5 mm to the edge; the axis pin has 4.5
//!    mm each side. The X-only placer had the pair at x 7.5 and y 7 / 503;
//! 2. a 359 mm stock at origin x -5 around the same model has 5 mm and
//!    4 mm X strips, so no point of the flip axis is clear, and the planner
//!    refuses with the 10 mm need;
//! 3. `KeyedPinInputs::from_session` reads the default part models and the
//!    tool of the drill operation, never `tools[0]` (P6);
//! 4. the mini-shaped project (P6): a 65 x 65 stock, a `stock_outline.dxf`
//!    model 0..65 and a 41 x 41 part 12..53, a 3 mm end mill as
//!    `tools[0]` and a 6 mm drill on a drill operation. The stock outline
//!    does not block the placement, and the plan is a keyed 3-pin pattern
//!    of 6 mm pins in the 12 mm rim with every wall at least
//!    `PIN_WALL_MM`.
//!
//! # NOT MEASURED
//!
//! The greyed button and the refusal label, which need an egui frame.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::indexing_slicing
)]

use rs_cam_core::compute::alignment_pins::{
    PIN_WALL_MM, PinObstacle, PinObstacleKind, PinPlacementError, pin_walls, validate_pins_for_flip,
};
use rs_cam_core::compute::stock_config::StockConfig;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::compute::transform::{FaceUp, ZRotation};
use rs_cam_viz::ui::properties::pins::{KeyedPinInputs, keyed_pin_plan};

const PIN_DIAMETER_MM: f64 = 6.0;
const STOCK_D_MM: f64 = 510.0;

/// The live model footprint, world frame: x 0..350, y 0..480.
fn model() -> PinObstacle {
    PinObstacle::rectangle(PinObstacleKind::Part, "model", (0.0, 0.0), (350.0, 480.0))
}

/// A hand-sized stock with padding 5, which does not describe it.
fn stock(width: f64, origin_x: f64) -> StockConfig {
    StockConfig {
        x: width,
        y: STOCK_D_MM,
        z: 26.0,
        origin_x,
        origin_y: -15.0,
        padding: 5.0,
        auto_from_model: false,
        ..StockConfig::default()
    }
}

fn inputs() -> KeyedPinInputs {
    KeyedPinInputs {
        obstacles: vec![model()],
        pin_tool_diameter: Some(PIN_DIAMETER_MM),
    }
}

/// Every pin has every wall at least `PIN_WALL_MM` (stock-local obstacles).
fn assert_walls(
    pins: &[rs_cam_core::compute::alignment_pins::AlignmentPin],
    stock: &StockConfig,
    local: &[PinObstacle],
) {
    for pin in pins {
        let walls = pin_walls(pin, stock.x, stock.y, local);
        assert!(walls.min_mm() >= PIN_WALL_MM - 1e-9, "{pin:?}: {walls:?}");
    }
}

#[test]
fn the_live_stock_places_the_pins_in_its_15mm_strips() {
    let pins = keyed_pin_plan(
        &stock(380.0, -15.0),
        FaceUp::Bottom,
        ZRotation::Deg0,
        &inputs(),
    )
    .unwrap_or_else(|e| panic!("the real strips are 15 mm; got the refusal {e}"));
    assert!(
        (pins[0].x - 372.5).abs() < 1e-9,
        "axis pin at x {}",
        pins[0].x
    );
    assert!(
        (pins[0].y - STOCK_D_MM * 0.5).abs() < 1e-9,
        "axis pin at y {}",
        pins[0].y
    );
    for pin in &pins[1..] {
        assert!((pin.x - 8.5).abs() < 1e-6, "pair pin at x {}", pin.x);
    }
    assert!(
        (pins[1].y - 11.0).abs() < 1e-6,
        "pair pin at y {}",
        pins[1].y
    );
    assert!(
        (pins[1].y + pins[2].y - STOCK_D_MM).abs() < 1e-9 && pins[1].y < pins[2].y,
        "not a mirror pair: {pins:?}"
    );
    for pin in &pins {
        assert!((pin.diameter - PIN_DIAMETER_MM).abs() < 1e-9);
    }
    let local = [model().translated(15.0, 15.0)];
    assert_walls(&pins, &stock(380.0, -15.0), &local);
}

#[test]
fn a_5mm_and_4mm_strip_stock_is_refused_on_the_axis() {
    match keyed_pin_plan(
        &stock(359.0, -5.0),
        FaceUp::Bottom,
        ZRotation::Deg0,
        &inputs(),
    ) {
        Err(PinPlacementError::NoRoomOnAxis { axis, need_mm, .. }) => {
            assert_eq!(axis, "y = 255.0 mm");
            assert!((need_mm - 10.0).abs() < 1e-9, "need {need_mm}");
        }
        other => panic!("expected the no-room-on-axis refusal, got {other:?}"),
    }
}

fn polygon_model(
    id: usize,
    name: &str,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
) -> rs_cam_core::session::LoadedModel {
    use rs_cam_core::geo::P2;
    use rs_cam_core::polygon::Polygon2;
    use std::sync::Arc;

    let poly = Polygon2::new(vec![
        P2::new(x0, y0),
        P2::new(x1, y0),
        P2::new(x1, y1),
        P2::new(x0, y1),
    ]);
    rs_cam_core::session::LoadedModel {
        id,
        name: name.to_owned(),
        mesh: None,
        polygons: Some(Arc::new(vec![poly])),
        drill_targets: Arc::new(Vec::new()),
        layers: Arc::new(Vec::new()),
        path: std::path::PathBuf::from(name),
        kind: None,
        units: None,
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
    }
}

fn tool(id: usize, tool_type: ToolType, diameter: f64) -> ToolConfig {
    let mut t = ToolConfig::new_default(ToolId(id), tool_type);
    t.diameter = diameter;
    t
}

/// A drill operation with the tool `tool_id`.
fn drill_op(tool_id: usize, model_id: usize) -> rs_cam_core::session::ToolpathConfig {
    use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
    use rs_cam_core::compute::config::{
        BoundaryConfig, DressupConfig, HeightsConfig, RestAnalysisConfig, StockSource,
    };
    use rs_cam_core::compute::operation_configs::DrillConfig;

    rs_cam_core::session::ToolpathConfig {
        id: rs_cam_core::ids::ToolpathId(0),
        name: "Dowel holes, 6 mm drill".to_owned(),
        enabled: true,
        operation: OperationConfig::Drill(DrillConfig::default()),
        dressups: DressupConfig::for_op(OperationType::Drill),
        heights: HeightsConfig::default(),
        tool_id,
        model_id,
        pre_gcode: None,
        post_gcode: None,
        boundary: BoundaryConfig::default(),
        boundary_inherit: true,
        stock_source: StockSource::default(),
        coolant: rs_cam_core::gcode::CoolantMode::Off,
        face_selection: None,
        debug_options: rs_cam_core::trace::debug_trace::ToolpathDebugOptions::default(),
        feeds_provenance: rs_cam_core::feeds::FeedsProvenance::default(),
        rest_analysis: RestAnalysisConfig::default(),
        planner_origin: None,
    }
}

#[test]
fn the_inputs_come_from_the_part_models_and_the_drill() {
    use rs_cam_core::session::ProjectSessionBuilder;

    let session = ProjectSessionBuilder::new()
        .stock(StockConfig {
            x: 400.0,
            y: 200.0,
            z: 20.0,
            origin_x: -25.0,
            origin_y: -50.0,
            auto_from_model: false,
            ..StockConfig::default()
        })
        .model(polygon_model(1, "left", 0.0, 0.0, 100.0, 100.0))
        .model(polygon_model(2, "right", 250.0, 0.0, 350.0, 100.0))
        .tool(tool(1, ToolType::EndMill, 3.0))
        .build();

    let got = KeyedPinInputs::from_session(&session);
    let names: Vec<&str> = got.obstacles.iter().map(|o| o.name.as_str()).collect();
    assert_eq!(names, ["left", "right"], "both models are part");
    // No operation drills the holes, so no tool: `tools[0]` is a 3 mm end
    // mill, and it is not the pin drill.
    assert_eq!(got.pin_tool_diameter, None);

    let with_drill = ProjectSessionBuilder::new()
        .model(polygon_model(1, "left", 0.0, 0.0, 100.0, 100.0))
        .tool(tool(1, ToolType::EndMill, 3.0))
        .tool(tool(4, ToolType::VBit, 6.0))
        .toolpath(drill_op(4, 1))
        .build();
    let got = KeyedPinInputs::from_session(&with_drill);
    let d = got
        .pin_tool_diameter
        .expect("the drill op names the 6 mm drill");
    assert!((d - 6.0).abs() < 1e-12, "{d}");
}

/// The mini test piece (`planning/rivmap_block_job/testpiece_mini.toml`),
/// in shape: a stock outline that fills the stock is not part, and the 6 mm
/// drill sizes the pins.
#[test]
fn the_mini_project_gets_a_keyed_pattern_in_the_rim() {
    use rs_cam_core::session::ProjectSessionBuilder;

    let stock = StockConfig {
        x: 65.0,
        y: 65.0,
        z: 24.5,
        origin_z: -24.5,
        padding: 0.0,
        auto_from_model: false,
        ..StockConfig::default()
    };
    let session = ProjectSessionBuilder::new()
        .stock(stock.clone())
        .model(polygon_model(0, "stock_outline.dxf", 0.0, 0.0, 65.0, 65.0))
        .model(polygon_model(1, "back_cavity.dxf", 12.0, 12.0, 53.0, 53.0))
        .tool(tool(0, ToolType::EndMill, 3.0))
        .tool(tool(3, ToolType::VBit, 6.0))
        .toolpath(drill_op(3, 1))
        .build();

    let inputs = KeyedPinInputs::from_session(&session);
    assert_eq!(inputs.obstacles.len(), 1, "{:?}", inputs.obstacles);
    let pins = keyed_pin_plan(&stock, FaceUp::Bottom, ZRotation::Deg0, &inputs)
        .unwrap_or_else(|e| panic!("the stock outline must not block the pins; got {e}"));
    assert_eq!(pins.len(), 3);
    for pin in &pins {
        assert!(
            (pin.diameter - 6.0).abs() < 1e-12,
            "not the 6 mm drill: {pins:?}"
        );
    }
    assert_walls(&pins, &stock, &inputs.obstacles);
    let report = validate_pins_for_flip(&pins, FaceUp::Bottom, ZRotation::Deg0, 65.0, 65.0);
    assert!(report.seats(), "{report:?}");
    assert!(report.keyed(), "{report:?}");
}
