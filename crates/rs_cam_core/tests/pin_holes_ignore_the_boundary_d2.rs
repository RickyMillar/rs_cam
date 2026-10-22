//! D2 (lead ruling 2026-10-16): the pin holes ignore the machining
//! boundary.
//!
//! # The ruling
//!
//! The alignment pin holes are absolute setup-map positions through the
//! whole stock. The other setups key on them, so a boundary must not move
//! or drop a hole. The Pin Drill row states `TerritoryXy::SetupMap`: no
//! territory clip applies, before or after generation. The obstacle step
//! still applies (K1).
//!
//! # What this test measures
//!
//! A boundary that shrinks the stock by 30 mm leaves both pins outside it.
//!
//! 1. Control: a Drill on points at the two pin positions and at the
//!    stock centre, with the same boundary, drills only the centre. So the
//!    boundary clip is on and it removes a hole at a pin position.
//! 2. The Pin Drill with the same boundary drills both pins.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;
use common::make_endmill_6mm;
use common::session::{polygon_model_with_points, toolpath_config};

use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::alignment_pins::AlignmentPin;
use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::config::{
    BoundaryConfig, BoundaryContainment, BoundarySource, DressupEntryStyle,
};
use rs_cam_core::compute::operation_configs::{
    AlignmentPinDrillConfig, DrillConfig, DrillCycleType,
};
use rs_cam_core::compute::stock_config::StockConfig;
use rs_cam_core::geo::P2;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{AddToolpathArgs, Command, ProjectSession, ProjectSessionBuilder};

/// Stock-relative; the stock origin is (0, 0), so also world.
const LEFT: AlignmentPin = AlignmentPin::new(10.0, 50.0, 6.0);
const RIGHT: AlignmentPin = AlignmentPin::new(90.0, 50.0, 6.0);
const CENTRE: [f64; 2] = [50.0, 50.0];

/// The stock less 30 mm on each side: both pins are outside it.
fn shrunk_boundary() -> BoundaryConfig {
    BoundaryConfig {
        enabled: true,
        source: BoundarySource::Stock,
        containment: BoundaryContainment::Center,
        offset: -30.0,
    }
}

/// A plate over the stock. The Drill family needs a polygon model; every
/// op here names its targets, so no centroid is drilled.
fn plate_polygon() -> Polygon2 {
    Polygon2::new(vec![
        P2::new(0.0, 0.0),
        P2::new(100.0, 0.0),
        P2::new(100.0, 100.0),
        P2::new(0.0, 100.0),
    ])
}

/// Toolpath 0: a Drill on the points at both pins and the centre (the
/// drawing selection). Toolpath 1: the Pin Drill (no extra hole). Both carry the shrunk boundary.
fn session() -> ProjectSession {
    let stock = StockConfig {
        x: 100.0,
        y: 100.0,
        z: 20.0,
        origin_x: 0.0,
        origin_y: 0.0,
        origin_z: -20.0,
        auto_from_model: false,
        alignment_pins: vec![LEFT, RIGHT],
        ..StockConfig::default()
    };
    let mut builder = ProjectSessionBuilder::new().stock(stock);
    let tool_idx = builder.add_tool(make_endmill_6mm());
    let tool_id = builder.tools()[tool_idx].id.raw();
    let model_id = builder.add_model(polygon_model_with_points(
        vec![plate_polygon()],
        &[
            [LEFT.at.x(), LEFT.at.y()],
            [RIGHT.at.x(), RIGHT.at.y()],
            CENTRE,
        ],
        "plate",
    ));
    let mut session = builder.build();
    let picks = OperationConfig::Drill(DrillConfig {
        cycle: DrillCycleType::Simple,
        ..DrillConfig::default()
    });
    let pins = OperationConfig::AlignmentPinDrill(AlignmentPinDrillConfig {
        diameter: 6.0,
        cycle: DrillCycleType::Simple,
        ..AlignmentPinDrillConfig::default()
    });
    for (name, op) in [("Drill", picks), ("Pin Drill", pins)] {
        let mut tc = toolpath_config(
            name,
            op,
            rs_cam_core::ids::ToolId::from_raw(tool_id),
            model_id,
        );
        tc.boundary = shrunk_boundary();
        tc.boundary_inherit = false;
        // Only the hole positions are under test.
        tc.dressups.entry_style = DressupEntryStyle::None;
        tc.dressups.optimize_rapid_order = false;
        let _ = session
            .apply(Command::AddToolpath(AddToolpathArgs {
                setup_index: 0,
                config: Box::new(tc),
            }))
            .expect("add drill toolpath");
    }
    session
}

/// The XY positions (um) where toolpath `index` feeds down.
fn drilled_xy(session: &mut ProjectSession, index: usize) -> Vec<(i64, i64)> {
    let cancel = AtomicBool::new(false);
    let result = session.generate_toolpath(index, &cancel).expect("generate");
    let mut xys: Vec<(i64, i64)> = result
        .op_data
        .annotated()
        .toolpath
        .moves
        .iter()
        .filter(|m| !matches!(m.move_type, rs_cam_core::toolpath::MoveType::Rapid))
        .map(|m| {
            (
                (m.target.x * 1000.0).round() as i64,
                (m.target.y * 1000.0).round() as i64,
            )
        })
        .collect();
    xys.sort_unstable();
    xys.dedup();
    xys
}

fn um(x: f64, y: f64) -> (i64, i64) {
    ((x * 1000.0).round() as i64, (y * 1000.0).round() as i64)
}

#[test]
fn the_boundary_drops_a_drill_hole_at_a_pin_position() {
    let mut s = session();
    assert_eq!(
        s.toolpath_configs()[0].operation.op_type(),
        OperationType::Drill
    );
    let drilled = drilled_xy(&mut s, 0);
    assert!(drilled.contains(&um(CENTRE[0], CENTRE[1])), "{drilled:?}");
    for pin in [LEFT, RIGHT] {
        assert!(
            !drilled.contains(&um(pin.at.x(), pin.at.y())),
            "control: the boundary must drop the pick at ({}, {}): {drilled:?}",
            pin.at.x(),
            pin.at.y()
        );
    }
}

#[test]
fn the_pin_drill_keeps_every_hole_outside_the_boundary() {
    let mut s = session();
    assert_eq!(
        s.toolpath_configs()[1].operation.op_type(),
        OperationType::AlignmentPinDrill
    );
    let drilled = drilled_xy(&mut s, 1);
    for pin in [LEFT, RIGHT] {
        assert!(
            drilled.contains(&um(pin.at.x(), pin.at.y())),
            "the pin hole at ({}, {}) must survive the boundary: {drilled:?}",
            pin.at.x(),
            pin.at.y()
        );
    }
}
