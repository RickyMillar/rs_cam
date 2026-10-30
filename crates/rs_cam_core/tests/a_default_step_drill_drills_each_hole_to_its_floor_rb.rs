//! R-B (operator ruling 2026-10-23) — a default Drill on a STEP model
//! drills each hole from its rim to its own floor.
//!
//! The fixture `tests/fixtures/step/block_with_blind_hole.step` (block
//! 40 x 30 x 20, one Ø6 blind hole at (15, 15), rim Z 20, floor Z 12) sits
//! in stock with its top at Z 25: the rim is 5 mm under the fresh stock.
//!
//! 1. The STEP door gives the rim circle its depth (blind, 8 mm) and marks
//!    the floor circle; the two pair by one rule (`ItemSet::rim_of`).
//! 2. A Drill added through the command row on its default selection takes
//!    the rim and no ring, and its floor becomes "Each hole floor"
//!    (`each_hole_floor_due`, with its own note).
//! 3. It drills at (15, 15) down to Z 12, the floor, and starts its R-plane
//!    above the stock top (Z 25): no rapid enters the stock. The heights
//!    read Top Z 25 ("auto: hole rims") and the floor Z 12. The model gouge
//!    guard finds no gouge.
//! 4. A click on the floor circle picks the rim (`hole_rim_key`, decision
//!    4); a pick of the floor circle by key refuses `HoleFloorNotRim`.
//! 5. B §3.4: a batch of picks (a box over the hole, MCP `add_picks`) that
//!    holds the rim circle and the floor circle stores ONE rim pick: the
//!    door folds each key through `hole_rim_key` before the dedup.
//!
//! Red proofs (run by hand, 2026-10-23): claim 2 goes red when
//! `rule::served_by_default` gives the Drill the Q3 Loop rule; claim 3 goes
//! red when `drill_hole_columns` caps the column at the stock box top and
//! measures the depth from it (the old F19 rule: the bottom is then Z 17).
//!
//! ```text
//! cargo test -p rs_cam_core -q --features step --test a_default_step_drill_drills_each_hole_to_its_floor_rb
//! ```

#![cfg(feature = "step")]
#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

mod common;

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use common::session::toolpath_config;
use common::tools::endmill_tool_config;
use rs_cam_core::compute::StockConfig;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::floor::Floor;
use rs_cam_core::compute::operation_configs::DrillConfig;
use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits};
use rs_cam_core::ids::ModelId;
use rs_cam_core::selection::{
    HoleDepth, Item, ItemKind, MatchKey, ModelHole, RefusalCause, SelectionEdit, SourceRef,
};
use rs_cam_core::session::{
    AddToolpathArgs, Command, EditSelectionArgs, Effects, ItemState, LoadedModel, ProjectSession,
    ProjectSessionBuilder, SelectionTarget, SessionError, TopSource,
};
use rs_cam_core::toolpath::{MoveIntent, MoveType};

const HOLE: &str = "tests/fixtures/step/block_with_blind_hole.step";
const HOLE_XY: [f64; 2] = [15.0, 15.0];
const RIM_Z: f64 = 20.0;
const FLOOR_Z: f64 = 12.0;
/// The stock top: 5 mm over the rim.
const STOCK_TOP: f64 = 25.0;

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-3
}

fn load() -> LoadedModel {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(HOLE);
    rs_cam_core::io::load_model_file(
        &path,
        ModelId::from_raw(0),
        ModelKind::Step,
        ModelUnits::Millimeters,
        Default::default(),
    )
    .unwrap()
}

/// The circle of the hole at `z`, and its index in the item set.
fn circle_at(model: &LoadedModel, z: f64) -> (usize, Item) {
    model
        .items
        .iter()
        .enumerate()
        .find(|(_, i)| i.kind() == ItemKind::Circle && i.z().is_some_and(|iz| near(iz, z)))
        .map(|(at, i)| (at, i.clone()))
        .unwrap()
}

/// The stock of the block, X -5..45, Y -5..35, Z 0..`STOCK_TOP`, a Ø6 end
/// mill and the model, with no toolpath.
fn empty_session() -> ProjectSession {
    let mut builder = ProjectSessionBuilder::new().stock(StockConfig {
        x: 50.0,
        y: 40.0,
        z: Some(STOCK_TOP),
        origin_x: -5.0,
        origin_y: -5.0,
        origin_z: 0.0,
        auto_from_model: false,
        ..StockConfig::default()
    });
    let _ = builder.add_tool(endmill_tool_config(6.0));
    let _ = builder.add_model(load());
    builder.build()
}

/// A default Drill added through the command row (every add door).
fn add_default_drill(session: &mut ProjectSession) -> Effects {
    let tool_id = session.tools()[0].id;
    let model_id = session.models()[0].id;
    let mut config = toolpath_config(
        "rb drill",
        OperationConfig::Drill(DrillConfig::default()),
        tool_id,
        model_id,
    );
    // S11: the fixture fills the unset values, but this op keeps the
    // default floor (`Floor::Unset`) that the rule reads.
    assert!(config.operation.set_floor(Floor::Unset));
    session
        .apply(Command::AddToolpath(AddToolpathArgs {
            setup_index: 0,
            config: Box::new(config),
        }))
        .unwrap()
}

/// Claim 1.
#[test]
fn a_step_hole_holds_its_depth_rb() {
    let model = load();
    let (rim_at, rim) = circle_at(&model, RIM_Z);
    let (floor_at, floor) = circle_at(&model, FLOOR_Z);
    assert_eq!(
        rim.model_hole(),
        Some(ModelHole::Rim(HoleDepth::Blind { depth_mm: 8.0 })),
        "the rim holds the hole depth"
    );
    assert_eq!(floor.model_hole(), Some(ModelHole::Floor));
    assert_eq!(model.items.rim_of(floor_at), Some(rim_at));
    assert_eq!(model.items.hole_floor_of(rim_at), Some(floor_at));
}

/// Claims 2 and 3.
#[test]
fn a_default_step_drill_drills_each_hole_to_its_floor_rb() {
    let mut session = empty_session();
    let effects = add_default_drill(&mut session);
    assert_eq!(effects.floor_set, Some(Floor::EachHole { depth: 0.0 }));
    assert_eq!(
        effects.floor_set_note(),
        Some(Effects::EACH_HOLE_FLOOR_NOTE)
    );

    let tc = session.toolpath_configs()[0].clone();
    let report = session.selection_status(&tc, SelectionTarget::Op).unwrap();
    for (item, state) in session.models()[0].items.iter().zip(&report.item_states) {
        let rim = item.kind() == ItemKind::Circle && item.z().is_some_and(|z| near(z, RIM_Z));
        assert_eq!(
            *state == ItemState::Cut,
            rim,
            "only the rim is cut: {:?} at Z {:?} is {state:?}",
            item.kind(),
            item.z()
        );
    }

    // F19: the heights read the rule that the motion runs.
    let heights = session.toolpath_heights(&tc).unwrap();
    assert!(
        near(heights.heights.top_z, STOCK_TOP),
        "{:?}",
        heights.heights
    );
    assert_eq!(heights.top_from, TopSource::AutoHoleRims);
    assert_eq!(heights.top_from.label(), "auto: hole rims");
    let floor_z = heights.floor_z(&tc.heights).unwrap().unwrap();
    assert!(near(floor_z, FLOOR_Z), "the floor line: {floor_z}");

    let result = session
        .generate_toolpath(0, &AtomicBool::new(false))
        .unwrap();
    let tp = &result.op_data.annotated().toolpath;
    let drilling: Vec<_> = tp
        .moves
        .iter()
        .filter(|m| m.intent == MoveIntent::Drilling)
        .collect();
    assert!(!drilling.is_empty(), "the Drill drills");
    for m in &drilling {
        assert!(near(m.target.x, HOLE_XY[0]) && near(m.target.y, HOLE_XY[1]));
    }
    let lowest = drilling
        .iter()
        .map(|m| m.target.z)
        .fold(f64::INFINITY, f64::min);
    assert!(near(lowest, FLOOR_Z), "to the hole floor: {lowest}");
    // The approach rapid (the last rapid before the first drilling feed)
    // stops at the R-plane, over the stock top: it does not enter the
    // stock. (A peck re-entry rapid inside the drilled hole is lower.)
    let first_feed = tp
        .moves
        .iter()
        .position(|m| m.intent == MoveIntent::Drilling)
        .unwrap();
    let r_plane = tp.moves[..first_feed]
        .iter()
        .rev()
        .find(|m| matches!(m.move_type, MoveType::Rapid))
        .map(|m| m.target.z)
        .unwrap();
    assert!(
        r_plane > STOCK_TOP,
        "the approach rapid enters the stock: Z {r_plane}"
    );
    let gouge = session.result_at(0).unwrap().stats.model_gouge;
    assert!(
        gouge.as_ref().is_none_or(|g| !g.gouges()),
        "no gouge: {gouge:?}"
    );
}

/// Claim 4. W3h: a new op holds no item, so the click adds; on the
/// drawing rule (which holds the rim) the click would remove the rim.
#[test]
fn a_click_on_the_floor_circle_picks_the_rim_rb() {
    let mut session = empty_session();
    let _ = add_default_drill(&mut session);
    let _ = session
        .apply(Command::EditSelection(EditSelectionArgs {
            index: 0,
            target: SelectionTarget::Op,
            edit: SelectionEdit::Replace(Box::new(rs_cam_core::selection::Selection::empty())),
        }))
        .unwrap();
    let model = load();
    let (_, rim) = circle_at(&model, RIM_Z);
    let (_, floor) = circle_at(&model, FLOOR_Z);
    let key_of = |item: &Item| MatchKey {
        source: SourceRef::ToolpathModel,
        ..item.key().clone()
    };
    let tc = session.toolpath_configs()[0].clone();
    let edit = session
        .pick_edit(&tc, SelectionTarget::Op, key_of(&floor))
        .unwrap();
    assert_eq!(
        edit,
        SelectionEdit::AddPick(key_of(&rim)),
        "a click on the floor circle picks the rim"
    );

    // A pick by key of the floor circle refuses, as generation does.
    let _ = session
        .apply(Command::EditSelection(EditSelectionArgs {
            index: 0,
            target: SelectionTarget::Op,
            edit: SelectionEdit::AddPick(key_of(&floor)),
        }))
        .unwrap();
    let tc = session.toolpath_configs()[0].clone();
    let refusal = session
        .selection_status(&tc, SelectionTarget::Op)
        .unwrap()
        .refusal
        .clone()
        .expect("the floor circle refuses");
    assert_eq!(refusal.cause, RefusalCause::HoleFloorNotRim, "{refusal}");
    let Err(SessionError::Selection(generated)) =
        session.generate_toolpath(0, &AtomicBool::new(false))
    else {
        panic!("generation must refuse the floor circle");
    };
    assert_eq!(*generated, refusal, "status = generation");
}

/// Claim 5.
#[test]
fn a_box_over_a_hole_stores_one_rim_pick_rb() {
    let mut session = empty_session();
    let _ = add_default_drill(&mut session);
    let model = load();
    let (_, rim) = circle_at(&model, RIM_Z);
    let (_, floor) = circle_at(&model, FLOOR_Z);
    let key_of = |item: &Item| MatchKey {
        source: SourceRef::ToolpathModel,
        ..item.key().clone()
    };
    let _ = session
        .apply(Command::EditSelection(EditSelectionArgs {
            index: 0,
            target: SelectionTarget::Op,
            edit: SelectionEdit::AddPicks(vec![key_of(&rim), key_of(&floor)]),
        }))
        .unwrap();
    let tc = session.toolpath_configs()[0].clone();
    let picks = tc.operation.selection().unwrap().picks().to_vec();
    assert_eq!(picks.len(), 1, "one rim pick: {picks:?}");
    assert!(
        picks[0].names_same_geometry(rim.key()),
        "the rim: {picks:?}"
    );
    let report = session.selection_status(&tc, SelectionTarget::Op).unwrap();
    assert!(report.refusal.is_none(), "the rim pick resolves");
}
