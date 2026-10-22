//! P8 (C6) — the dowel holes have one source.
//!
//! The mini test piece (`planning/rivmap_block_job/testpiece_mini.toml`)
//! drills its dowel holes with a plain drill operation on
//! `dowel_through.dxf`: two 6 mm circles at (6, 32.5) and (59, 32.5) on a
//! 65 x 65 stock at origin 0. The stock pins are a second copy of the same
//! two holes. This file builds that shape inline (the two circle targets,
//! the pins, the drill operation in a Top setup) and checks the two
//! project verdicts:
//!
//! - with a pin-drill operation added, the machine drills each hole twice:
//!   `pin_holes_drilled_twice`;
//! - with the pins moved and the drawing not moved, the drawn dowel holes
//!   and the pins disagree: `dowel_holes_disagree`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::ToolpathId;
use rs_cam_core::compute::alignment_pins::{AlignmentPin, translate};
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::{
    BoundaryConfig, DressupConfig, HeightsConfig, RestAnalysisConfig, StockSource,
};
use rs_cam_core::compute::operation_configs::{AlignmentPinDrillConfig, DrillConfig};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::diagnostics::ids;
use rs_cam_core::gcode::CoolantMode;
use rs_cam_core::geo::P2;
use rs_cam_core::selection::{Item, ItemSet, Shape, SourceRef};
use rs_cam_core::session::{
    Command, LoadedModel, ProjectSession, ProjectSessionBuilder, SetAlignmentPinsArgs,
    ToolpathConfig, VerdictKind,
};
use rs_cam_core::trace::debug_trace::ToolpathDebugOptions;

/// The two circles of `dowel_through.dxf`, world frame, mm.
const DOWEL_CIRCLES: [(f64, f64); 2] = [(6.0, 32.5), (59.0, 32.5)];
const DOWEL_D: f64 = 6.0;

fn tc(name: &str, op: OperationConfig) -> ToolpathConfig {
    ToolpathConfig {
        shoe: Default::default(),
        id: ToolpathId::from_raw(0),
        name: name.to_owned(),
        enabled: true,
        operation: op,
        dressups: DressupConfig::default(),
        heights: HeightsConfig::default(),
        tool_id: rs_cam_core::ids::ToolId::from_raw(0),
        model_id: rs_cam_core::ids::ModelId::from_raw(0),
        pre_gcode: None,
        post_gcode: None,
        boundary: BoundaryConfig::default(),
        boundary_inherit: true,
        stock_source: StockSource::Fresh,
        coolant: CoolantMode::Off,
        debug_options: ToolpathDebugOptions::default(),
        feeds_provenance: rs_cam_core::feeds::FeedsProvenance::default(),
        rest_analysis: RestAnalysisConfig::default(),
        planner_origin: None,
    }
}

fn dowel_drawing() -> LoadedModel {
    LoadedModel {
        id: rs_cam_core::ids::ModelId::from_raw(0),
        name: "dowel_through.dxf".to_owned(),
        mesh: None,
        // P2.1: each CIRCLE entity is one item, ring and hole in one.
        items: std::sync::Arc::new(ItemSet::new(
            DOWEL_CIRCLES
                .iter()
                .map(|&(x, y)| {
                    Item::new(
                        SourceRef::ToolpathModel,
                        Some("0".into()),
                        Shape::circle(P2::new(x, y), DOWEL_D),
                    )
                    .unwrap()
                })
                .collect(),
        )),
        path: std::path::PathBuf::from("dowel_through.dxf"),
        kind: None,
        units: None,
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
        import_options: Default::default(),
        import_report: None,
        drawing: Default::default(),
    }
}

fn mini_pins() -> Vec<AlignmentPin> {
    DOWEL_CIRCLES
        .iter()
        .map(|&(x, y)| AlignmentPin::new(x, y, DOWEL_D))
        .collect()
}

/// The mini shape: a 65 x 65 x 24.5 stock at origin 0 with the two pins,
/// the dowel drawing, and the plain drill operation on it in setup 0 (a
/// Top setup). `with_pin_drill` adds the pin-drill operation too.
fn mini(with_pin_drill: bool) -> ProjectSession {
    let mut builder = ProjectSessionBuilder::new();
    // A 6 mm drill, as the op name says: P6.5 (O-3) drills a circle of the
    // tool size only.
    builder.add_tool(ToolConfig {
        diameter: DOWEL_D,
        ..ToolConfig::new_default(ToolId::from_raw(0), ToolType::EndMill)
    });
    builder.add_model(dowel_drawing());
    let tool = builder.tools()[0].id.raw();
    let model = builder.models()[0].id;
    let mut ops = vec![tc(
        "Dowel holes, 6 mm drill, peck",
        OperationConfig::Drill(DrillConfig::default()),
    )];
    if with_pin_drill {
        ops.push(tc(
            "Pin Drill",
            OperationConfig::AlignmentPinDrill(AlignmentPinDrillConfig {
                ..AlignmentPinDrillConfig::default()
            }),
        ));
    }
    for mut op in ops {
        op.tool_id = rs_cam_core::ids::ToolId::from_raw(tool);
        op.model_id = model;
        let _ = builder.add_toolpath(0, op).unwrap();
    }
    let stock = rs_cam_core::compute::stock_config::StockConfig {
        x: 65.0,
        y: 65.0,
        z: 24.5,
        origin_x: 0.0,
        origin_y: 0.0,
        origin_z: 0.0,
        auto_from_model: false,
        alignment_pins: mini_pins(),
        ..Default::default()
    };
    builder.stock(stock).build()
}

fn verdicts(s: &ProjectSession, kind: VerdictKind) -> Vec<String> {
    s.diagnostics()
        .verdicts
        .into_iter()
        .filter(|v| v.kind == kind)
        .map(|v| v.headline)
        .collect()
}

#[test]
fn the_mini_shape_without_a_pin_drill_is_one_source() {
    let s = mini(false);
    assert!(verdicts(&s, VerdictKind::PinHolesDrilledTwice).is_empty());
    assert!(
        verdicts(&s, VerdictKind::DowelHolesDisagree).is_empty(),
        "the drawn holes sit on the pins"
    );
}

#[test]
fn a_pin_drill_beside_the_drawn_dowel_holes_drills_them_twice() {
    let s = mini(true);
    let twice = verdicts(&s, VerdictKind::PinHolesDrilledTwice);
    assert_eq!(twice.len(), 1, "{twice:?}");
    assert!(
        twice[0].contains("drilled twice") && twice[0].contains("2 hole(s)"),
        "{}",
        twice[0]
    );
    // The diagnostic id reaches every surface.
    let found: Vec<String> =
        rs_cam_core::diagnostics::adapters::from_project_diagnostics::diagnostics_from_project(
            &s.diagnostics(),
        )
        .into_iter()
        .map(|d| d.id.as_str().to_owned())
        .collect();
    assert!(
        found
            .iter()
            .any(|id| id == ids::PROJECT_PIN_HOLES_DRILLED_TWICE),
        "{found:?}"
    );
}

#[test]
fn pins_moved_and_the_drawing_not_moved_disagree() {
    let mut s = mini(false);
    let _ = s
        .apply(Command::SetAlignmentPins(SetAlignmentPinsArgs {
            pins: translate(&mini_pins(), 0.0, 10.0),
        }))
        .unwrap();
    let disagree = verdicts(&s, VerdictKind::DowelHolesDisagree);
    assert_eq!(disagree.len(), 1, "{disagree:?}");
    assert!(
        disagree[0].contains("disagree") && disagree[0].contains("10.0 mm"),
        "{}",
        disagree[0]
    );
    assert!(
        verdicts(&s, VerdictKind::PinHolesDrilledTwice).is_empty(),
        "no pin drill exists, so nothing is drilled twice"
    );
}

#[test]
fn a_disabled_drill_is_not_a_source() {
    let mut s = mini(true);
    let _ = s
        .apply(Command::SetToolpathEnabled(
            rs_cam_core::session::SetToolpathEnabledArgs {
                index: 0,
                enabled: false,
            },
        ))
        .unwrap();
    assert!(verdicts(&s, VerdictKind::PinHolesDrilledTwice).is_empty());
}
