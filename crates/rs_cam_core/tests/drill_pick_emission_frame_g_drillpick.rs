//! G-DRILLPICK-FRAME — picked drill targets are stored in the world frame
//! and must be transformed into the frame the toolpath emits in.
//!
//! ## The defect
//!
//! `selected_holes` are raw model/DXF coordinates: the viz picker maps
//! `LoadedModel::drill_targets` straight through. The model's *polygons*,
//! though, are setup-transformed before generation
//! (`session/compute.rs`), and the toolpath emits in the setup frame —
//! world for an identity setup, zero-rooted setup-local otherwise. Both
//! drill families then consumed `selected_holes` verbatim, so on a
//! non-identity setup every pick was off by the whole setup transform.
//!
//! Scope is wider than the pin drill: it hits the ordinary `Drill`
//! family, which is the common case for DXF hole drilling.
//!
//! ## The decision this sentry pins
//!
//! Picks stay **stored in world/model coordinates** and are converted at
//! generation. No storage change, no migration, and no saved project
//! changes meaning. The identity arms below are the other half of that
//! contract: world already IS the emission frame there, so the conversion
//! must be a no-op and must not be applied twice.
//!
//! ## What this measures pre-fix
//!
//! Fixture: 240x250x25 stock at origin (-20, -25, -25); one pick at world
//! (30, 40); one stock alignment pin at stock-relative (2.5, 2.5); the
//! same two ops built once per setup — setup 0 identity, setup 1
//! `face_up = Bottom`.
//!
//! | arm | expected | pre-fix |
//! |---|---|---|
//! | Drill pick, identity | (30, 40) | (30, 40) — already correct |
//! | Drill pick, `Bottom` | **(50, 185)** | (30, 40) |
//! | pin-drill pin, identity | (-17.5, -22.5) | (-17.5, -22.5) |
//! | pin-drill pin, `Bottom` | (2.5, 2.5) | (2.5, 2.5) |
//! | pin-drill pick, identity | (30, 40) | (30, 40) |
//! | pin-drill pick, `Bottom` | **(50, 185)** | (30, 40) |
//!
//! The two pin rows are not incidental. The pin-drill op mixes two
//! differently-framed hole sources in one operation: `cfg.holes` are
//! STOCK-RELATIVE and take a translation by the emission-frame stock
//! origin (G-PINDRILL-FRAME, fixed 2026-08-21), while `selected_holes` are
//! WORLD and take the setup transform. Applying either correction to the
//! other's holes is the failure this pins — and so is applying the setup
//! transform twice, which would put the flipped pick at (70, 40) rather
//! than (50, 185).

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
use rs_cam_core::compute::transform::FaceUp;
use rs_cam_core::geo::P2;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::ProjectSession;

const STOCK_X: f64 = 240.0;
const STOCK_Y: f64 = 250.0;
const STOCK_Z: f64 = 25.0;
const ORIGIN_X: f64 = -20.0;
const ORIGIN_Y: f64 = -25.0;

/// A target picked off the model, in world/model coordinates — the frame
/// the picker hands over and the frame the project stores.
const PICK: [f64; 2] = [30.0, 40.0];
/// The same physical point in setup-local coordinates under `Bottom`:
/// `x - origin_x = 50`, `stock_y - (y - origin_y) = 250 - 65 = 185`.
const PICK_FLIPPED: [f64; 2] = [50.0, 185.0];

/// Stock-relative, like everything in `StockConfig::alignment_pins`.
const PIN: [f64; 2] = [2.5, 2.5];
/// The pin in the identity setup's emission frame (world): stock-relative
/// zero sits at the stock's min corner.
const PIN_WORLD: [f64; 2] = [PIN[0] + ORIGIN_X, PIN[1] + ORIGIN_Y];

fn stock() -> StockConfig {
    StockConfig {
        x: STOCK_X,
        y: STOCK_Y,
        z: STOCK_Z,
        origin_x: ORIGIN_X,
        origin_y: ORIGIN_Y,
        origin_z: -STOCK_Z,
        auto_from_model: false,
        alignment_pins: vec![AlignmentPin::new(PIN[0], PIN[1], 6.0)],
        ..StockConfig::default()
    }
}

/// A plate over the stock footprint in WORLD coordinates. Every op here
/// selects its targets explicitly, so no centroid is ever drilled — the
/// model exists to satisfy the `Drill` family's polygon requirement, and
/// to make it visible that the picks do NOT ride the polygon pipeline.
fn plate_polygon() -> Polygon2 {
    Polygon2::new(vec![
        P2::new(ORIGIN_X, ORIGIN_Y),
        P2::new(ORIGIN_X + STOCK_X, ORIGIN_Y),
        P2::new(ORIGIN_X + STOCK_X, ORIGIN_Y + STOCK_Y),
        P2::new(ORIGIN_X, ORIGIN_Y + STOCK_Y),
    ])
}

fn hole_drill_op() -> OperationConfig {
    OperationConfig::Drill(DrillConfig {
        depth: 10.0,
        cycle: DrillCycleType::Peck,
        peck_depth: 3.0,
        selected_holes: Some(vec![PICK]),
        ..DrillConfig::default()
    })
}

fn pin_drill_op() -> OperationConfig {
    OperationConfig::AlignmentPinDrill(AlignmentPinDrillConfig {
        holes: vec![PIN],
        spoilboard_penetration: 2.0,
        cycle: DrillCycleType::Peck,
        peck_depth: 3.0,
        selected_holes: Some(vec![PICK]),
        ..AlignmentPinDrillConfig::default()
    })
}

/// Indices into the session built by [`two_setup_session`].
const IDENTITY_HOLE: usize = 0;
const IDENTITY_PIN: usize = 1;
const FLIPPED_HOLE: usize = 2;
const FLIPPED_PIN: usize = 3;

/// One stock, two setups, the same two drill ops in each. The ONLY thing
/// that differs between an identity arm and its flipped twin is the setup
/// transform, so any coordinate difference is attributable to it.
fn two_setup_session() -> ProjectSession {
    let mut session = ProjectSession::new_empty();
    session.set_stock_config(stock());
    let tool_idx = session.add_tool(make_endmill_6mm());
    let tool_id = session.tools()[tool_idx].id.0;
    let model_id = session.add_model(polygon_model(vec![plate_polygon()], "plate"));

    // ISOLATE THE VARIABLE: this sentry is about WHERE the holes are, not
    // what shape they are, so every dressup that can add or move a
    // coordinate is pinned off. Entry styling above all —
    // `for_op(AlignmentPinDrill)` ships `Ramp`, which is its own defect on
    // these same ops (G-WANAKA-DRILL-RAMP) and would smear each hole into
    // a 19 mm oval.
    let add = |session: &mut ProjectSession, setup: usize, name: &str, op: OperationConfig| {
        let mut tc = toolpath_config(name, op, tool_id, model_id);
        tc.dressups.entry_style = DressupEntryStyle::None;
        tc.dressups.optimize_rapid_order = false;
        tc.dressups.link_moves = false;
        tc.dressups.arc_fitting = false;
        tc.dressups.segment_merge = false;
        session.add_toolpath(setup, tc).expect("add drill toolpath");
    };

    // Setup 0 is the identity setup `new_empty` already created.
    add(&mut session, 0, "HoleTop", hole_drill_op());
    add(&mut session, 0, "PinTop", pin_drill_op());

    let flipped = session.add_setup("Flip".to_owned(), FaceUp::Bottom);
    add(&mut session, flipped, "HoleBottom", hole_drill_op());
    add(&mut session, flipped, "PinBottom", pin_drill_op());

    session
}

/// The distinct XY columns one toolpath visits, to micron resolution and
/// in sorted order — a drill cycle only ever parks and descends, so this
/// is exactly the set of holes it machines.
fn drilled_columns(session: &mut ProjectSession, index: usize) -> Vec<(i64, i64)> {
    let cancel = AtomicBool::new(false);
    let result = session
        .generate_toolpath(index, &cancel)
        .expect("drill generation must succeed");
    let tp = &result.op_data.annotated().toolpath;
    assert!(
        !tp.moves.is_empty(),
        "fixture is vacuous: toolpath {index} emitted no moves"
    );
    let mut cols: Vec<(i64, i64)> = tp
        .moves
        .iter()
        .map(|m| {
            (
                (m.target.x * 1000.0).round() as i64,
                (m.target.y * 1000.0).round() as i64,
            )
        })
        .collect();
    cols.sort_unstable();
    cols.dedup();
    cols
}

fn expect_columns(expected: &[[f64; 2]]) -> Vec<(i64, i64)> {
    let mut cols: Vec<(i64, i64)> = expected
        .iter()
        .map(|xy| {
            (
                (xy[0] * 1000.0).round() as i64,
                (xy[1] * 1000.0).round() as i64,
            )
        })
        .collect();
    cols.sort_unstable();
    cols.dedup();
    cols
}

#[test]
fn an_identity_setup_drills_a_pick_where_it_was_picked() {
    let mut session = two_setup_session();
    assert_eq!(
        drilled_columns(&mut session, IDENTITY_HOLE),
        expect_columns(&[PICK]),
        "an identity setup emits in the world frame, so a world-frame pick \
         must pass through untouched. A non-no-op here means the transform \
         is being applied where there is none."
    );
}

#[test]
fn a_flipped_setup_drills_a_pick_at_its_setup_local_position() {
    let mut session = two_setup_session();
    assert_eq!(
        drilled_columns(&mut session, FLIPPED_HOLE),
        expect_columns(&[PICK_FLIPPED]),
        "a `Bottom` setup emits in the zero-rooted setup-local frame, so a \
         pick stored at world {PICK:?} belongs at {PICK_FLIPPED:?}. \
         Pre-fix it was drilled at {PICK:?} — off by the whole setup \
         transform. Drilled at (70, 40) would mean the transform ran twice."
    );
}

#[test]
fn the_pin_drills_two_frames_of_holes_in_one_op_on_an_identity_setup() {
    let mut session = two_setup_session();
    assert_eq!(
        drilled_columns(&mut session, IDENTITY_PIN),
        expect_columns(&[PIN_WORLD, PICK]),
        "the pin is stock-relative and lands at the stock's min corner \
         plus {PIN:?}; the pick is world and passes through. Neither \
         correction may be applied to the other's holes."
    );
}

#[test]
fn the_pin_drills_two_frames_of_holes_in_one_op_on_a_flipped_setup() {
    let mut session = two_setup_session();
    assert_eq!(
        drilled_columns(&mut session, FLIPPED_PIN),
        expect_columns(&[PIN, PICK_FLIPPED]),
        "on a non-identity setup the emission-frame stock origin is (0, 0), \
         so the stock-relative pin stays at {PIN:?} — while the world-frame \
         pick moves to {PICK_FLIPPED:?}. Pre-fix the pick stayed at \
         {PICK:?}."
    );
}
