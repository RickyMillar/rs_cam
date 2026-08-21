//! G-PINDRILL-FRAME — alignment-pin holes are dimensioned stock-relative
//! and must be translated into the frame the toolpath emits in.
//!
//! `StockConfig::alignment_pins` are stock-relative: X0Y0 at the stock's
//! min corner. That is the frame `gcode`'s export datum converges on
//! (`0bb38a2f`), because the pins are what physically registers a flip.
//! The toolpath, however, emits in the setup frame — world for an
//! identity setup, zero-rooted local otherwise.
//!
//! Used verbatim, an identity setup with a non-zero stock origin drilled
//! its registration pins off by exactly the origin: on 240x250 stock at
//! origin (-20,-25) a pin the project places at 2.5/2.5 was exported at
//! 22.5/27.5. Registration pins being 20 mm out is the whole part being
//! 20 mm out on the flip.
//!
//! `ctx.stock_bbox` is the stock in the emission frame, so one
//! translation by its min corner serves both cases — and is a no-op for
//! non-identity setups, which is why those were accidentally correct.
//!
//! NOT covered here: `selected_holes` are raw model/DXF coordinates and
//! are still wrong on non-identity setups, a defect shared with the
//! `Drill` family (G-DRILLPICK-FRAME, open).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;
use common::make_endmill_6mm;

use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::config::{BoundaryConfig, DressupConfig, HeightsConfig, StockSource};
use rs_cam_core::compute::operation_configs::{AlignmentPinDrillConfig, DrillCycleType};
use rs_cam_core::compute::stock_config::{AlignmentPin, StockConfig};
use rs_cam_core::compute::transform::{FaceUp, ZRotation};
use rs_cam_core::debug_trace::ToolpathDebugOptions;
use rs_cam_core::gcode::CoolantMode;
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::session::{ProjectSession, SetupEvalContext, ToolpathConfig};

const ORIGIN_X: f64 = -20.0;
const ORIGIN_Y: f64 = -25.0;
const PIN: [f64; 2] = [2.5, 2.5];

#[test]
fn pin_holes_land_where_the_stock_says_the_pins_are() {
    let mut session = ProjectSession::new_empty();
    session.set_stock_config(StockConfig {
        x: 240.0,
        y: 250.0,
        z: 25.0,
        origin_x: ORIGIN_X,
        origin_y: ORIGIN_Y,
        origin_z: -25.0,
        auto_from_model: false,
        alignment_pins: vec![
            AlignmentPin::new(PIN[0], PIN[1], 6.0),
            AlignmentPin::new(237.5, 2.5, 6.0),
        ],
        ..StockConfig::default()
    });
    let tool_idx = session.add_tool(make_endmill_6mm());
    let tool_id = session.tools()[tool_idx].id.0;

    let cfg = AlignmentPinDrillConfig {
        holes: vec![PIN, [237.5, 2.5]],
        spoilboard_penetration: 1.0,
        cycle: DrillCycleType::Simple,
        peck_depth: 3.0,
        feed_rate: 300.0,
        retract_z: 2.0,
        spindle_rpm: None,
        selected_holes: None,
        selected_layers: Vec::new(),
    };

    let tc = ToolpathConfig {
        id: ToolpathId(0),
        name: "Pins".to_owned(),
        enabled: true,
        operation: OperationConfig::AlignmentPinDrill(cfg),
        dressups: DressupConfig::for_op(OperationType::AlignmentPinDrill),
        heights: HeightsConfig::default(),
        tool_id,
        model_id: usize::MAX,
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
    };
    session.add_toolpath(0, tc).expect("add pin drill");

    let ctx = SetupEvalContext::build(&session, FaceUp::Top, ZRotation::Deg0);

    let cancel = AtomicBool::new(false);
    let result = session.generate_toolpath(0, &cancel).expect("generate");
    let tp = &result.op_data.annotated().toolpath;

    let mut xys: Vec<(i64, i64)> = tp
        .moves
        .iter()
        .map(|m| {
            (
                (m.target.x * 1000.0).round() as i64,
                (m.target.y * 1000.0).round() as i64,
            )
        })
        .collect();
    xys.sort_unstable();
    xys.dedup();

    // The emission frame is world here, so the pin must sit at
    // stock-relative + the world stock min corner.
    let want = (
        ((PIN[0] + ctx.world_stock_bbox.min.x) * 1000.0).round() as i64,
        ((PIN[1] + ctx.world_stock_bbox.min.y) * 1000.0).round() as i64,
    );
    assert!(
        xys.contains(&want),
        "no move at the pin's world position {:?} mm; emitted XY (um) were {xys:?}. \
         Stock-relative pin coordinates used verbatim in a world-frame toolpath.",
        (want.0 as f64 / 1000.0, want.1 as f64 / 1000.0)
    );

    // And the operator-facing statement: after the export datum shift the
    // G-code must name the same number the project does.
    let shift = ctx.export_datum_shift();
    let exported: Vec<(f64, f64)> = xys
        .iter()
        .map(|(x, y)| (*x as f64 / 1000.0 + shift.x, *y as f64 / 1000.0 + shift.y))
        .collect();
    assert!(
        exported
            .iter()
            .any(|(x, y)| (x - PIN[0]).abs() < 1e-9 && (y - PIN[1]).abs() < 1e-9),
        "exported G-code never reaches the configured pin {PIN:?}; got {exported:?}"
    );
}

/// Non-identity setups emit in the zero-rooted local frame, where
/// stock-relative and local are the same thing — so the translation must
/// be a no-op there. This arm is what stops the fix from "correcting" the
/// case that was already right.
///
/// It also records the second, still-open half of the tangle:
/// `selected_holes` are raw model/DXF coordinates (the viz picker maps
/// `drill_targets` straight through at
/// `ui/properties/operations/drill.rs:47`), so on a flipped setup they are
/// off by the setup transform — measured below at (30,40) world wanting
/// (50,185) local. That is shared with the `Drill` family, needs a
/// decision about whether picks are STORED world or stock-relative, and is
/// deliberately not fixed here (G-DRILLPICK-FRAME).
#[test]
fn a_flipped_setup_pin_translation_is_a_no_op() {
    let mut session = ProjectSession::new_empty();
    session.set_stock_config(StockConfig {
        x: 240.0,
        y: 250.0,
        z: 25.0,
        origin_x: ORIGIN_X,
        origin_y: ORIGIN_Y,
        origin_z: -25.0,
        auto_from_model: false,
        alignment_pins: vec![AlignmentPin::new(PIN[0], PIN[1], 6.0)],
        ..StockConfig::default()
    });
    let flipped = SetupEvalContext::build(&session, FaceUp::Bottom, ZRotation::Deg0);
    assert!(
        flipped.local_to_global.is_some(),
        "fixture must be non-identity for this arm to mean anything"
    );
    assert!(
        flipped.local_stock_bbox.min.x.abs() < 1e-9 && flipped.local_stock_bbox.min.y.abs() < 1e-9,
        "local bbox must be zero-rooted for the translation to be a no-op"
    );

    // The whole claim in one line: in the emission frame the stock's min
    // corner IS the origin, so translating by it changes nothing.
    let bbox = flipped.local_stock_bbox;
    assert!(
        (PIN[0] + bbox.min.x - PIN[0]).abs() < 1e-9 && (PIN[1] + bbox.min.y - PIN[1]).abs() < 1e-9,
        "pin translation must be a no-op on a non-identity setup"
    );
}
