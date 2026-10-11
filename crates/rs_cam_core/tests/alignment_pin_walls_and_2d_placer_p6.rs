//! P6 (2026-10-12): the pin walls in 2D, the placer inputs, and the 2D
//! placer.
//!
//! # What the findings were (review C4, C5)
//!
//! - The validator checked a pin only against the stock edge, with no wall.
//!   A hand-placed pin could go through the part or a keep-out zone with
//!   no warning.
//! - The placer read one X range: the union bbox of ALL models. The mini
//!   test piece loads `stock_outline.dxf` as a model, which fills the
//!   stock, so both clear strips were 0 mm and the placer refused.
//! - The pin tool fell back to `tools[0]`, the 3 mm end mill of the mini
//!   piece. Auto-place would have replaced the 6 mm pins with 3 mm pins.
//! - The placer refused every setup but `Bottom` at 0 deg.
//!
//! # What this test measures
//!
//! A mini-shaped project: a 65 x 65 stock, a stock-outline model 0..65, a
//! 41 x 41 part 12..53 (a 12 mm rim), a 3 mm end mill as `tools[0]` and a
//! 6 mm drill used by a drill operation.
//!
//! 1. The stock outline is not part by default; the 41 x 41 model is.
//! 2. The pin tool is the 6 mm drill, not `tools[0]`.
//! 3. The placer finds a keyed 3-pin pattern in the rim with every wall at
//!    least `PIN_WALL_MM`, at 0 deg and at 180 deg.
//! 4. A hand-placed pin in the part and one in a keep-out zone each give a
//!    wall finding with the pin number and the wall value.
//! 5. A pin diameter that differs from the drill gives a finding.
//!
//! # NOT MEASURED
//!
//! The live mini file itself (its models are outside the repository). It
//! also loads `dowel_through.dxf`, two 6 mm circles on the flip axis, and
//! the default rule calls that model part; see the P6 report.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::PathBuf;
use std::sync::Arc;

use rs_cam_core::compute::alignment_pins::{
    AlignmentPin, PIN_WALL_MM, PinFindingKind, PinObstacle, pin_walls, validate_pins_for_flip,
};
use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::config::{BoundaryConfig, DressupConfig, HeightsConfig, StockSource};
use rs_cam_core::compute::operation_configs::DrillConfig;
use rs_cam_core::compute::stock_config::StockConfig;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::compute::transform::{FaceUp, ZRotation};
use rs_cam_core::gcode::CoolantMode;
use rs_cam_core::geo::P2;
use rs_cam_core::ids::{KeepOutId, ModelId, ToolpathId};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{
    AddKeepOutArgs, Command, KeepOutZone, LoadedModel, ProjectSession, ProjectSessionBuilder,
    SetAlignmentPinsArgs, ToolpathConfig,
};
use rs_cam_core::trace::debug_trace::ToolpathDebugOptions;

const W: f64 = 65.0;
const D: f64 = 65.0;
const PIN_D: f64 = 6.0;
const OUTLINE: usize = 0;
const CAVITY: usize = 1;

fn rectangle_model(id: usize, name: &str, x0: f64, y0: f64, x1: f64, y1: f64) -> LoadedModel {
    let poly = Polygon2::new(vec![
        P2::new(x0, y0),
        P2::new(x1, y0),
        P2::new(x1, y1),
        P2::new(x0, y1),
    ]);
    LoadedModel {
        id,
        name: name.to_owned(),
        mesh: None,
        polygons: Some(Arc::new(vec![poly])),
        drill_targets: Arc::new(Vec::new()),
        layers: Arc::new(Vec::new()),
        path: PathBuf::from(name),
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

fn drill_op(tool_id: usize, model_id: usize) -> ToolpathConfig {
    ToolpathConfig {
        id: ToolpathId(0),
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
        coolant: CoolantMode::Off,
        face_selection: None,
        debug_options: ToolpathDebugOptions::default(),
        feeds_provenance: rs_cam_core::feeds::FeedsProvenance::default(),
        rest_analysis: rs_cam_core::compute::config::RestAnalysisConfig::default(),
        planner_origin: None,
    }
}

fn mini_stock(w: f64, d: f64) -> StockConfig {
    StockConfig {
        x: w,
        y: d,
        z: 24.5,
        origin_z: -24.5,
        padding: 0.0,
        auto_from_model: false,
        ..StockConfig::default()
    }
}

/// The mini-shaped project. `with_drill` adds the drill operation that
/// names the 6 mm drill.
fn mini_session(with_drill: bool) -> ProjectSession {
    let builder = ProjectSessionBuilder::new()
        .stock(mini_stock(W, D))
        .model(rectangle_model(
            OUTLINE,
            "stock_outline.dxf",
            0.0,
            0.0,
            W,
            D,
        ))
        .model(rectangle_model(
            CAVITY,
            "back_cavity.dxf",
            12.0,
            12.0,
            53.0,
            53.0,
        ))
        .tool(tool(0, ToolType::EndMill, 3.0))
        .tool(tool(3, ToolType::VBit, 6.0));
    let builder = if with_drill {
        builder.toolpath(drill_op(3, CAVITY))
    } else {
        builder
    };
    builder.build()
}

fn obstacles(session: &ProjectSession) -> Vec<PinObstacle> {
    session.pin_obstacles(&session.default_pin_part_models())
}

/// Every wall of every pin is at least `PIN_WALL_MM`, and the pins keep a
/// wall between them.
fn assert_walls(pins: &[AlignmentPin], w: f64, d: f64, obstacles: &[PinObstacle]) {
    for (i, pin) in pins.iter().enumerate() {
        let walls = pin_walls(pin, w, d, obstacles);
        assert!(
            walls.min_mm() >= PIN_WALL_MM - 1e-9,
            "pin {} at ({:.3}, {:.3}): {walls:?}",
            i + 1,
            pin.x,
            pin.y
        );
        for other in pins.iter().skip(i + 1) {
            let gap = (pin.x - other.x).hypot(pin.y - other.y) - PIN_D;
            assert!(gap >= PIN_WALL_MM - 1e-9, "{pin:?} {other:?}: {gap}");
        }
    }
}

#[test]
fn a_stock_outline_model_is_not_part_by_default() {
    let session = mini_session(true);
    assert_eq!(session.default_pin_part_models(), vec![ModelId(CAVITY)]);
    let obstacles = obstacles(&session);
    assert_eq!(obstacles.len(), 1, "{obstacles:?}");
    assert_eq!(obstacles[0].name, "back_cavity.dxf");
}

#[test]
fn the_pin_tool_is_the_drill_not_tools_0() {
    let session = mini_session(true);
    assert!((session.tools()[0].diameter - 3.0).abs() < 1e-12);
    let drill = session
        .pin_drill_tool_diameter()
        .expect("the drill op names a tool");
    assert!((drill - PIN_D).abs() < 1e-12, "{drill}");
    // No operation drills the holes: no tool, never `tools[0]`.
    assert_eq!(mini_session(false).pin_drill_tool_diameter(), None);
}

/// The rim is 12 mm. A 6 mm pin with a 2 mm wall each side needs 10 mm,
/// so each pin centre has a band 2 mm wide, 5..7 mm from the stock edge.
/// The axis pin sits on `y = 32.5` in the X rims; the pair has the X rims
/// and the Y rims. An equilateral triangle would need a pair offset of
/// about 30.6 mm, more than the 27.5 mm the stock allows, so a keyed
/// pattern exists.
#[test]
fn the_mini_rim_holds_a_keyed_three_pin_pattern() {
    let session = mini_session(true);
    let obstacles = obstacles(&session);
    let stock = session.stock_config();
    let pins = stock
        .plan_keyed_pins(
            FaceUp::Bottom,
            ZRotation::Deg0,
            &obstacles,
            session.pin_drill_tool_diameter().unwrap(),
        )
        .unwrap_or_else(|e| panic!("the rim holds the pins; got the refusal {e}"));
    assert!(
        pins.iter().all(|p| (p.diameter - PIN_D).abs() < 1e-12),
        "{pins:?}"
    );
    assert!((pins[0].y - D * 0.5).abs() < 1e-9, "{pins:?}");
    assert_walls(&pins, W, D, &obstacles);
    let report = validate_pins_for_flip(&pins, FaceUp::Bottom, ZRotation::Deg0, W, D);
    assert!(report.seats(), "{report:?}");
    assert!(report.keyed(), "{report:?}");
    let mut placed = stock.clone();
    placed.alignment_pins = pins.to_vec();
    assert!(placed.pin_wall_findings(&obstacles).is_empty());
}

/// A `Bottom` setup turned 180 deg mirrors X about `x = W/2`. The same
/// search places the axis pin on that line and the pair across it, on the
/// square mini stock and on a stock that is not square.
#[test]
fn bottom_180_placement_seats_and_keys() {
    let session = mini_session(true);
    let obstacles = obstacles(&session);
    let pins = session
        .stock_config()
        .plan_keyed_pins(FaceUp::Bottom, ZRotation::Deg180, &obstacles, PIN_D)
        .unwrap();
    assert!((pins[0].x - W * 0.5).abs() < 1e-9, "{pins:?}");
    assert!((pins[1].x + pins[2].x - W).abs() < 1e-9, "{pins:?}");
    assert!((pins[1].y - pins[2].y).abs() < 1e-9, "{pins:?}");
    assert_walls(&pins, W, D, &obstacles);
    let report = validate_pins_for_flip(&pins, FaceUp::Bottom, ZRotation::Deg180, W, D);
    assert!(report.seats() && report.keyed(), "{report:?}");

    // 140 x 150, a part with a 20 mm rim.
    let stock = mini_stock(140.0, 150.0);
    let part = [PinObstacle::rectangle(
        rs_cam_core::compute::alignment_pins::PinObstacleKind::Part,
        "part",
        (20.0, 20.0),
        (120.0, 130.0),
    )];
    let pins = stock
        .plan_keyed_pins(FaceUp::Bottom, ZRotation::Deg180, &part, PIN_D)
        .unwrap();
    assert!((pins[0].x - 70.0).abs() < 1e-9, "{pins:?}");
    assert_walls(&pins, 140.0, 150.0, &part);
    let report = validate_pins_for_flip(&pins, FaceUp::Bottom, ZRotation::Deg180, 140.0, 150.0);
    assert!(report.seats() && report.keyed(), "{report:?}");
}

/// The wall check of a stored set. Pin 1 is inside the part, pin 2 is
/// inside a keep-out zone, pin 3 is in the rim with 3 mm walls.
#[test]
fn a_pin_in_the_part_or_a_keep_out_is_a_wall_finding() {
    let mut session = mini_session(true);
    let _ = session
        .apply(Command::AddKeepOut(AddKeepOutArgs {
            setup_index: 0,
            zone: Box::new(KeepOutZone {
                id: KeepOutId(0),
                name: "clamp pad".to_owned(),
                enabled: true,
                origin_x: 0.0,
                origin_y: 0.0,
                size_x: 10.0,
                size_y: 10.0,
            }),
        }))
        .unwrap();
    let _ = session
        .apply(Command::SetAlignmentPins(SetAlignmentPinsArgs {
            pins: vec![
                AlignmentPin::new(32.5, 32.5, PIN_D),
                AlignmentPin::new(6.0, 6.0, PIN_D),
                AlignmentPin::new(59.0, 32.5, PIN_D),
            ],
        }))
        .unwrap();

    let findings = session.pin_placement_findings();
    let walls: Vec<_> = findings
        .iter()
        .filter(|f| f.kind == PinFindingKind::ThinWall)
        .collect();
    // Pin 1: 20.5 mm inside the part edge, minus the 3 mm radius.
    assert!(
        walls.iter().any(|f| f
            .text
            .contains("pin 1 cuts into the part 'back_cavity.dxf'")
            && f.text.contains("-23.5 mm")),
        "{findings:?}"
    );
    // Pin 2: 4 mm inside the zone edge, minus the 3 mm radius.
    assert!(
        walls.iter().any(|f| f
            .text
            .contains("pin 2 cuts into the keep-out zone 'clamp pad'")
            && f.text.contains("-7.0 mm")),
        "{findings:?}"
    );
    assert!(
        !walls.iter().any(|f| f.text.contains("pin 3")),
        "{findings:?}"
    );

    // The placer keeps the pins out of the zone too.
    let obstacles = obstacles(&session);
    let pins = session
        .stock_config()
        .plan_keyed_pins(FaceUp::Bottom, ZRotation::Deg0, &obstacles, PIN_D)
        .unwrap();
    assert_walls(&pins, W, D, &obstacles);
}

#[test]
fn a_pin_that_differs_from_the_drill_is_a_finding() {
    let mut session = mini_session(true);
    let _ = session
        .apply(Command::SetAlignmentPins(SetAlignmentPinsArgs {
            pins: vec![AlignmentPin::new(59.0, 32.5, 3.0)],
        }))
        .unwrap();
    let findings = session.pin_placement_findings();
    assert!(
        findings
            .iter()
            .any(|f| f.kind == PinFindingKind::PinToolMismatch
                && f.text.contains("3.00 mm")
                && f.text.contains("6.00 mm")),
        "{findings:?}"
    );
}
