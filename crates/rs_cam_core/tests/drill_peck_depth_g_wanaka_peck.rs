//! G-WANAKA-PECK — a peck may not be deeper than the hole it is pecking.
//!
//! ## The defect
//!
//! Auto-applied feeds set `peck_depth = 15.0` on both of wanaka200's drill
//! ops (the schema default is 3.0) while the holes op had `depth = 12.0`.
//! A peck bigger than the hole is not a peck: the cycle takes one
//! oversized bite and the chip evacuation the operator picked the cycle
//! for never happens. In white oak with a 6 mm end mill that is a welded
//! flute.
//!
//! The peck-adequacy gate reported `Within` throughout. It divides by the
//! ENVELOPE radius (R-12) and is not evidence about anything here, which
//! is why this sentry measures the **emitted motion** instead.
//!
//! ## What this measures pre-fix
//!
//! Each arm reads the fed descents straight off the toolpath — the
//! `MoveIntent::Drilling` moves and the Z each one starts from. The cycle
//! is rooted at the R-plane, not the hole top (Fanuc G83; see
//! `drill::fed_descents`), and `effective_safe_z` floors the R-plane at
//! `stock_top + 5`, so with the stock top at Z0 every descent below starts
//! from Z+5.
//!
//! | arm | depth | peck | descents pre-fix | first descent pre-fix |
//! |---|---|---|---|---|
//! | wanaka holes op | 12.0 | 15.0 | 2 (15.0, 2.5) | **15.0 mm** |
//! | shallow hole | 4.0 | 15.0 | **1** (9.0) | **9.0 mm** |
//! | pin drill (25 + 2 spoilboard) | 27.0 | 30.0 | 2 (30.0, 2.5) | **30.0 mm** |
//! | control: valid peck | 12.0 | 3.0 | 6 | 3.0 mm |
//!
//! After the clamp the same arms read first descents of 12.0, 4.0 and
//! 27.0 — each exactly its own hole depth — and the shallow arm goes from
//! one bite to three. The control arm is byte-identical either way: the
//! clamp must not touch a configuration that was already valid.
//!
//! The first descent is the interesting one because it is the only one
//! that is exactly one peck: every later descent re-enters through
//! `PECK_REENTRY_CLEARANCE_MM` and so measures `peck + 0.5`.

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
use rs_cam_core::geo::P2;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{LoadedModel, ProjectSession};
use rs_cam_core::toolpath::MoveIntent;

const STOCK_X: f64 = 240.0;
const STOCK_Y: f64 = 250.0;
const STOCK_Z: f64 = 25.0;
const SPOILBOARD: f64 = 2.0;
const PIN: [f64; 2] = [2.5, 2.5];
const HOLE: [f64; 2] = [100.0, 50.0];

fn stock() -> StockConfig {
    StockConfig {
        x: STOCK_X,
        y: STOCK_Y,
        z: STOCK_Z,
        origin_x: 0.0,
        origin_y: 0.0,
        origin_z: -STOCK_Z,
        auto_from_model: false,
        alignment_pins: vec![AlignmentPin::new(PIN[0], PIN[1], 6.0)],
        ..StockConfig::default()
    }
}

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

/// Every fed descent the emitted cycle performs, as the distance each
/// `Drilling` move travels down from wherever the tool was parked.
///
/// Entry dressups are pinned off in the fixtures, so nothing is inserted
/// between the park and the descent and `moves[i - 1]` is the park.
fn fed_descent_lengths(session: &mut ProjectSession, index: usize) -> Vec<f64> {
    let cancel = AtomicBool::new(false);
    let result = session
        .generate_toolpath(index, &cancel)
        .expect("drill generation must succeed");
    let tp = &result.op_data.annotated().toolpath;
    let lengths: Vec<f64> = tp
        .moves
        .iter()
        .enumerate()
        .filter(|(i, m)| *i > 0 && matches!(m.intent, MoveIntent::Drilling))
        .map(|(i, m)| tp.moves[i - 1].target.z - m.target.z)
        .collect();
    assert!(
        !lengths.is_empty(),
        "fixture is vacuous: the cycle emitted no fed descent"
    );
    lengths
}

fn drill_session(op: OperationConfig) -> ProjectSession {
    let mut session = ProjectSession::new_empty();
    session.set_stock_config(stock());
    let tool_idx = session.add_tool(make_endmill_6mm());
    let tool_id = session.tools()[tool_idx].id.0;
    let model_id = session.add_model(plate_model());
    let mut tc = toolpath_config("Drill", op, tool_id, model_id);
    // ISOLATE THE VARIABLE: this sentry measures the peck schedule, and it
    // reads each descent's length off the move that parked the tool. Every
    // dressup that can insert, merge or reorder moves is therefore pinned
    // off — entry styling above all, which is a separate defect on these
    // same ops (G-WANAKA-DRILL-RAMP) and would put a ramp between the park
    // and the descent.
    tc.dressups.entry_style = DressupEntryStyle::None;
    tc.dressups.optimize_rapid_order = false;
    tc.dressups.link_moves = false;
    tc.dressups.arc_fitting = false;
    tc.dressups.segment_merge = false;
    session.add_toolpath(0, tc).expect("add drill toolpath");
    session
}

fn hole_op(depth: f64, peck_depth: f64) -> OperationConfig {
    OperationConfig::Drill(DrillConfig {
        depth,
        cycle: DrillCycleType::Peck,
        peck_depth,
        selected_holes: Some(vec![HOLE]),
        ..DrillConfig::default()
    })
}

#[test]
fn the_wanaka_holes_op_pecks_no_deeper_than_the_hole() {
    const DEPTH: f64 = 12.0;
    let mut session = drill_session(hole_op(DEPTH, 15.0));
    let descents = fed_descent_lengths(&mut session, 0);
    assert!(
        descents[0] <= DEPTH + 1e-9,
        "peck_depth 15.0 on a {DEPTH} mm hole: the first fed descent \
         travelled {:.3} mm (pre-fix 15.000). A peck deeper than the hole \
         is not a peck, and in hardwood that is a chip-evacuation \
         failure. Full schedule: {descents:?}",
        descents[0]
    );
    assert!(
        descents.len() >= 2,
        "a Peck cycle must take more than one bite; got {descents:?}"
    );
}

#[test]
fn a_hole_shallower_than_the_peck_still_pecks() {
    // The starkest form: pre-fix this emitted ONE fed descent of 9.0 mm —
    // the cycle silently became a single full-depth plunge, which is what
    // the ledger entry describes.
    const DEPTH: f64 = 4.0;
    let mut session = drill_session(hole_op(DEPTH, 15.0));
    let descents = fed_descent_lengths(&mut session, 0);
    assert!(
        descents.len() >= 2,
        "peck_depth 15.0 on a {DEPTH} mm hole degenerated to a single \
         full-depth descent (pre-fix: exactly 1 descent of 9.000 mm). \
         Schedule: {descents:?}"
    );
    assert!(
        descents[0] <= DEPTH + 1e-9,
        "first fed descent {:.3} mm exceeds the {DEPTH} mm hole",
        descents[0]
    );
}

#[test]
fn the_pin_drill_pecks_no_deeper_than_the_pin_hole() {
    // The pin-drill's depth is `stock height + spoilboard_penetration`,
    // which the config cannot compute on its own — the clamp has to be fed
    // that depth by the generator, so it gets its own arm.
    let depth = STOCK_Z + SPOILBOARD;
    let op = OperationConfig::AlignmentPinDrill(AlignmentPinDrillConfig {
        holes: vec![PIN],
        spoilboard_penetration: SPOILBOARD,
        cycle: DrillCycleType::Peck,
        peck_depth: 30.0,
        ..AlignmentPinDrillConfig::default()
    });
    let mut session = drill_session(op);
    let descents = fed_descent_lengths(&mut session, 0);
    assert!(
        descents[0] <= depth + 1e-9,
        "peck_depth 30.0 on a {depth} mm pin hole: first fed descent \
         {:.3} mm (pre-fix 30.000). Schedule: {descents:?}",
        descents[0]
    );
    assert!(
        descents.len() >= 2,
        "a Peck cycle must take more than one bite; got {descents:?}"
    );
}

#[test]
fn a_valid_peck_depth_is_left_exactly_alone() {
    // The clamp is a cap, not a rewrite: a configuration that already
    // means what it says must emit the schedule it always did.
    let mut session = drill_session(hole_op(12.0, 3.0));
    let descents = fed_descent_lengths(&mut session, 0);
    assert_eq!(
        descents.len(),
        6,
        "a 3 mm peck on a 12 mm hole from an R-plane at +5 takes 6 fed \
         descents; got {descents:?}"
    );
    assert!(
        (descents[0] - 3.0).abs() < 1e-9,
        "the first fed descent is exactly one peck: expected 3.000, got \
         {:.3}. The clamp must not touch a valid peck depth.",
        descents[0]
    );
}
