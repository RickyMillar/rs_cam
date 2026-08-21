//! G-WANAKA-DRILL-RAMP — an entry dressup must never reshape a drill cycle.
//!
//! ## The defect
//!
//! `add_toolpath` auto-applied `entry_style = "ramp"` to both of
//! wanaka200's drill cycles, and `apply_entry` duly rewrote every peck
//! descent as a ramp. The emitted pin-drill motion was:
//!
//! ```text
//! G0 X15.708 Y16.271 Z30.000
//! G1 X15.708 Y16.271 Z28.000 F3000
//! G1 X2.500 Y2.500  Z27.000        <- 19 mm lateral while descending
//! ```
//!
//! A ramped alignment-pin hole is an oval slot, and flip registration is
//! the entire purpose of that op. No gate, warning or advisory flagged it;
//! it was found by reading the G-code.
//!
//! ## What this measures pre-fix
//!
//! The excursion is not incidental, it is the dressup's own arithmetic:
//! `emit_ramp` ramps the last `ENTRY_CLEARANCE = 2.0` mm of descent at
//! `ramp_angle`, out and back, so the turn-around sits
//! `(2.0 / tan(3 deg)) / 2 = 19.081` mm from the hole centre. That is the
//! "19 mm" above, and it is what a live probe of the pin drill reported on
//! 2026-08-21: a distinct X of **21.581 mm** for a pin at **X2.5**.
//!
//! With the fixture below (one pin at 2.5/2.5, one hole at 100/50, stock
//! top at Z0) the three arms measured, before the fix:
//!
//! | arm | entry style | max XY excursion from the hole centre |
//! |---|---|---|
//! | pin drill | `Ramp` 3 deg (the shipped default for the op) | 19.081 mm |
//! | hole drill | `Ramp` 3 deg | 19.081 mm |
//! | hole drill | `Helix` r=2.0 | 2.000 mm |
//!
//! After the fix all three read 0, and the emitted motion is move-for-move
//! what `entry_style = None` produces. The helix arm is here because the
//! defect was never about ramps: both styles route through one plunge
//! detector, so a fix that only named `Ramp` would leave a two-millimetre
//! bore instead of a hole.
//!
//! The pin-drill arm deliberately does NOT set an entry style. It takes
//! `DressupConfig::for_op(AlignmentPinDrill)`, which is what the product
//! itself builds, and that ships `Ramp` — which is also why the engine
//! strips the dressup rather than refusing it.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;
use common::make_endmill_6mm;
use common::session::{polygon_model, toolpath_config};

use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::DressupEntryStyle;
use rs_cam_core::compute::operation_configs::{
    AlignmentPinDrillConfig, DrillConfig, DrillCycleType,
};
use rs_cam_core::compute::stock_config::{AlignmentPin, StockConfig};
use rs_cam_core::dressup::{EntryStyle, apply_entry};
use rs_cam_core::geo::{P2, P3};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{LoadedModel, ProjectSession};
use rs_cam_core::toolpath::{MoveIntent, Toolpath};
use rs_cam_core::toolpath_spans::AnnotatedToolpath;

const STOCK_X: f64 = 240.0;
const STOCK_Y: f64 = 250.0;
const STOCK_Z: f64 = 25.0;

/// Stock-relative, and — with `origin = (0, 0)` — world too, so the numbers
/// in this file are the ones an operator would read off the G-code.
const PIN: [f64; 2] = [2.5, 2.5];
const HOLE: [f64; 2] = [100.0, 50.0];

/// Half the zigzag ramp `emit_ramp` inserts at the shipped 3 deg angle:
/// `(ENTRY_CLEARANCE / tan(3 deg)) / 2`. Recorded so a change to either
/// constant shows up here as a changed expectation rather than as a
/// silently different oval.
const RAMP_EXCURSION_MM: f64 = 19.081;

fn stock() -> StockConfig {
    StockConfig {
        x: STOCK_X,
        y: STOCK_Y,
        z: STOCK_Z,
        origin_x: 0.0,
        origin_y: 0.0,
        // Stock top at world Z0 — the 2D convention, so the emitted Z is
        // depth below the surface and `effective_safe_z` floors the
        // R-plane at +5.
        origin_z: -STOCK_Z,
        auto_from_model: false,
        alignment_pins: vec![AlignmentPin::new(PIN[0], PIN[1], 6.0)],
        ..StockConfig::default()
    }
}

/// A plate covering the stock. Both drill arms select their target
/// explicitly, so this exists only to satisfy the `Drill` family's polygon
/// requirement — its centroid is never drilled.
fn plate_model() -> LoadedModel {
    polygon_model(
        vec![Polygon2::new(vec![
            P2::new(0.0, 0.0),
            P2::new(STOCK_X, 0.0),
            P2::new(STOCK_X, STOCK_Y),
            P2::new(0.0, STOCK_Y),
        ])],
        "plate",
    )
}

fn hole_drill_op() -> OperationConfig {
    OperationConfig::Drill(DrillConfig {
        depth: 12.0,
        cycle: DrillCycleType::Peck,
        peck_depth: 3.0,
        selected_holes: Some(vec![HOLE]),
        ..DrillConfig::default()
    })
}

fn pin_drill_op() -> OperationConfig {
    OperationConfig::AlignmentPinDrill(AlignmentPinDrillConfig {
        holes: vec![PIN],
        spoilboard_penetration: 2.0,
        cycle: DrillCycleType::Peck,
        peck_depth: 3.0,
        ..AlignmentPinDrillConfig::default()
    })
}

/// Generate one drill op with `entry` applied to its dressups and hand back
/// every emitted move target, to micron resolution.
///
/// `None` for `entry` leaves whatever `DressupConfig::for_op` builds for
/// the operation — which for `AlignmentPinDrill` is `Ramp`.
fn drill_moves(op: OperationConfig, entry: Option<DressupEntryStyle>) -> Vec<(i64, i64, i64)> {
    let mut session = ProjectSession::new_empty();
    session.set_stock_config(stock());
    let tool_idx = session.add_tool(make_endmill_6mm());
    let tool_id = session.tools()[tool_idx].id.0;
    let model_id = session.add_model(plate_model());

    let mut tc = toolpath_config("Drill", op, tool_id, model_id);
    if let Some(style) = entry {
        tc.dressups.entry_style = style;
    }
    session.add_toolpath(0, tc).expect("add drill toolpath");

    let cancel = AtomicBool::new(false);
    let result = session
        .generate_toolpath(0, &cancel)
        .expect("drill generation must succeed");
    let moves: Vec<(i64, i64, i64)> = result
        .op_data
        .annotated()
        .toolpath
        .moves
        .iter()
        .map(|m| {
            (
                (m.target.x * 1000.0).round() as i64,
                (m.target.y * 1000.0).round() as i64,
                (m.target.z * 1000.0).round() as i64,
            )
        })
        .collect();
    assert!(
        !moves.is_empty(),
        "fixture is vacuous: the drill emitted no moves"
    );
    moves
}

/// How far, in XY, the emitted motion strays from `centre` (mm).
fn max_xy_excursion(moves: &[(i64, i64, i64)], centre: [f64; 2]) -> f64 {
    moves
        .iter()
        .map(|&(x, y, _)| {
            let dx = x as f64 / 1000.0 - centre[0];
            let dy = y as f64 / 1000.0 - centre[1];
            (dx * dx + dy * dy).sqrt()
        })
        .fold(0.0_f64, f64::max)
}

#[test]
fn the_shipped_pin_drill_default_does_not_ream_an_oval() {
    let moves = drill_moves(pin_drill_op(), None);
    let excursion = max_xy_excursion(&moves, PIN);
    assert!(
        excursion < 1e-9,
        "an alignment-pin hole must be round: the pin at {PIN:?} was \
         machined up to {excursion:.3} mm away from its centre (pre-fix \
         {RAMP_EXCURSION_MM:.3} mm, the zigzag ramp's half-length). A \
         ramped pin hole is a slot, and flip registration is what the op \
         is for."
    );
}

#[test]
fn a_ramp_entry_on_a_hole_drill_is_stripped() {
    let moves = drill_moves(hole_drill_op(), Some(DressupEntryStyle::Ramp));
    let excursion = max_xy_excursion(&moves, HOLE);
    assert!(
        excursion < 1e-9,
        "a drill cycle's descent is the cut, not an approach to one: the \
         hole at {HOLE:?} was machined up to {excursion:.3} mm away from \
         its centre (pre-fix {RAMP_EXCURSION_MM:.3} mm)"
    );
}

#[test]
fn a_helix_entry_on_a_hole_drill_is_stripped_too() {
    let moves = drill_moves(hole_drill_op(), Some(DressupEntryStyle::Helix));
    let excursion = max_xy_excursion(&moves, HOLE);
    assert!(
        excursion < 1e-9,
        "the strip must cover both entry styles — they share one plunge \
         detector. The hole at {HOLE:?} was machined up to \
         {excursion:.3} mm away from its centre (pre-fix 2.000 mm, the \
         default helix radius, i.e. a bore rather than a hole)"
    );
}

#[test]
fn a_stripped_entry_leaves_the_cycle_move_for_move_intact() {
    // Stronger than the excursion bound, and the reason "strip" is the
    // right word: the dressup must not merely stay near the hole centre,
    // it must not touch the cycle at all. Z included — a partially-applied
    // entry would show up as an extra descent split.
    let baseline = drill_moves(hole_drill_op(), Some(DressupEntryStyle::None));
    for style in [DressupEntryStyle::Ramp, DressupEntryStyle::Helix] {
        assert_eq!(
            drill_moves(hole_drill_op(), Some(style)),
            baseline,
            "entry_style {style:?} changed the emitted drill cycle; it must \
             be dropped whole"
        );
    }
}

/// One rapid down to safe Z, one plunge to depth, one lateral cut — the
/// minimal shape `apply_entry`'s plunge detector fires on.
fn one_plunge_toolpath() -> AnnotatedToolpath {
    let mut tp = Toolpath::new();
    tp.rapid_to_with_intent(P3::new(10.0, 10.0, 30.0), MoveIntent::Linking);
    tp.feed_to_with_intent(P3::new(10.0, 10.0, -3.0), 300.0, MoveIntent::EntryPlunge);
    tp.feed_to_with_intent(P3::new(50.0, 10.0, -3.0), 300.0, MoveIntent::ClearingCut);
    AnnotatedToolpath::new(tp)
}

#[test]
fn the_ramp_transform_itself_is_alive() {
    // Non-vacuity for every assertion above: they all read zero if entry
    // ramping were simply broken. `apply_entry` — the transform the drill
    // arms are protected from — still reshapes an ordinary milling plunge
    // into a ramp, so a zero above is a strip and not a dead dressup.
    let out = apply_entry(
        one_plunge_toolpath(),
        EntryStyle::Ramp { max_angle_deg: 3.0 },
        300.0,
        0.0,
    );
    assert!(
        out.toolpath
            .moves
            .iter()
            .any(|m| matches!(m.intent, MoveIntent::EntryRamp)),
        "apply_entry emitted no EntryRamp move for a plain milling plunge \
         — this file's zero-excursion assertions would then prove nothing"
    );
}
