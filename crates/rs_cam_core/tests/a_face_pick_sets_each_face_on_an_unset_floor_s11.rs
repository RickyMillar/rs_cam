//! S11 (operator ruling 2026-10-23): a face pick still sets the floor
//! "Each picked face".
//!
//! S11 removed the Pocket floor at the add (min(half the stock, 5 mm)), so
//! a new Pocket holds `Floor::Unset`. THE rule `each_face_floor_due` fires
//! only while `floor_is_default`; an unset floor is the registry default,
//! so it counts as the default.
//!
//! # The claims
//!
//! 1. A Pocket drafted on a STEP model (THE draft door) and added through
//!    `AddToolpath` holds nothing (W3h: a new op starts with nothing
//!    selected), so its floor stays `Floor::Unset` and the add names no
//!    floor in `floor_set`.
//! 2. With the floor at `Floor::Unset`, `floor_is_default` is true, and a
//!    face pick below the stock top sets "Each picked face".
//! 3. A floor the operator typed (`Top Z` 4) is not a default, and a pick
//!    keeps it.
//!
//! Red proof (run by hand, 2026-10-23): `floor_is_default` gives `false`
//! for `Floor::Unset` (claims 1 and 2 go red).
//!
//! ```text
//! cargo test -p rs_cam_core -q --features step --test a_face_pick_sets_each_face_on_an_unset_floor_s11
//! ```

#![cfg(feature = "step")]
#![expect(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::path::PathBuf;

use rs_cam_core::compute::StockConfig;
use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::compute::floor::Floor;
use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::feeds::suggest::StockContext;
use rs_cam_core::ids::ModelId;
use rs_cam_core::selection::{ItemKind, SelectionEdit};
use rs_cam_core::session::{
    AddToolpathArgs, Command, EditSelectionArgs, Effects, ProjectSession, ProjectSessionBuilder,
    SelectionTarget, SetToolpathFloorArgs,
};

const STEPPED: &str = "tests/fixtures/step/stepped_block.step";
/// The stock top of the stepped block fixture (mm): 2 mm over the Z 30
/// step.
const STOCK_TOP: f64 = 32.0;

fn stepped_session() -> ProjectSession {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(STEPPED);
    let model = rs_cam_core::io::load_model_file(
        &path,
        ModelId::from_raw(0),
        ModelKind::Step,
        ModelUnits::Millimeters,
        Default::default(),
    )
    .unwrap_or_else(|e| panic!("{STEPPED}: {e}"));
    let mut builder = ProjectSessionBuilder::new().stock(StockConfig {
        x: 70.0,
        y: 50.0,
        z: Some(STOCK_TOP),
        origin_x: -5.0,
        origin_y: -5.0,
        origin_z: 0.0,
        auto_from_model: false,
        ..StockConfig::default()
    });
    let _ = builder.add_tool(
        ToolConfig::builder(ToolId::from_raw(0), ToolType::EndMill)
            .diameter(6.0)
            .build(),
    );
    let _ = builder.add_model(model);
    builder.build()
}

/// Draft a Pocket through THE door and add it.
fn add_drafted_pocket(s: &mut ProjectSession) -> Effects {
    let draft = s
        .draft_toolpath(
            rs_cam_core::session::DraftRequest::new(OperationType::Pocket, 0, "Pocket".to_owned())
                .tool(s.tools()[0].id)
                .model(s.models()[0].id),
        )
        .expect("the end mill fits a Pocket");
    assert_eq!(
        draft.config.operation.floor(),
        Some(Floor::Unset),
        "non-vacuity: the draft has no floor"
    );
    s.apply(Command::AddToolpath(AddToolpathArgs {
        setup_index: 0,
        config: Box::new(draft.config),
    }))
    .expect("the add")
}

fn set_floor(s: &mut ProjectSession, floor: Floor) {
    let _ = s
        .apply(Command::SetToolpathFloor(SetToolpathFloorArgs {
            index: 0,
            floor,
        }))
        .expect("the floor door");
}

fn floor_is_default(s: &ProjectSession) -> bool {
    s.toolpath_configs()[0]
        .operation
        .floor_is_default(&StockContext::from_stock_bbox(
            s.stock_bbox(),
            s.stock_config().padding,
        ))
}

/// A face pick of the up face at Z 10 (below the stock top).
fn pick_the_low_face(s: &mut ProjectSession) -> Effects {
    let item = s.models()[0]
        .items
        .iter()
        .find(|i| {
            i.kind() == ItemKind::Face && i.plane_z().is_some_and(|z| (z - 10.0).abs() < 1e-6)
        })
        .expect("the stepped block has an up face at Z 10")
        .clone();
    let model_id = s.models()[0].id;
    s.apply(Command::EditSelection(EditSelectionArgs {
        index: 0,
        target: SelectionTarget::Op,
        edit: SelectionEdit::AddPick(item.key().clone().for_toolpath_model(model_id)),
    }))
    .expect("the op holds a selection")
}

/// Claims 1 and 2.
#[test]
fn a_face_pick_still_sets_each_face_on_an_unset_floor_s11() {
    let mut s = stepped_session();
    let effects = add_drafted_pocket(&mut s);
    assert_eq!(
        effects.floor_set, None,
        "W3h: the add holds nothing, so no floor rule fires"
    );
    assert_eq!(
        s.toolpath_configs()[0].operation.floor(),
        Some(Floor::Unset)
    );
    assert!(floor_is_default(&s), "an unset floor counts as the default");
    let effects = pick_the_low_face(&mut s);
    assert_eq!(effects.floor_set, Some(Floor::EachFace { depth: 0.0 }));
    assert_eq!(
        s.toolpath_configs()[0].operation.floor(),
        Some(Floor::EachFace { depth: 0.0 })
    );
}

/// Claim 3.
#[test]
fn a_typed_floor_is_kept_by_a_face_pick_s11() {
    let mut s = stepped_session();
    let _ = add_drafted_pocket(&mut s);
    set_floor(&mut s, Floor::top(4.0));
    assert!(!floor_is_default(&s), "a typed floor is the operator's");
    let effects = pick_the_low_face(&mut s);
    assert_eq!(effects.floor_set, None);
    assert_eq!(
        s.toolpath_configs()[0].operation.floor(),
        Some(Floor::top(4.0))
    );
}
