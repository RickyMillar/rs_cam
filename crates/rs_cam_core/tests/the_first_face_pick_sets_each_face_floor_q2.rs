//! Operator ruling Q2 (2026-10-22): the first face pick on a floor op whose
//! floor is still the op default sets "Each picked face", depth 0, in the
//! same command, with a one-line note.
//!
//! # The defect
//!
//! Measured on `stepped_block.step`: a Profile with two picked faces and
//! the default floor `Top Z` cut both faces to ONE height. The operator
//! read a face pick as "the floor" ("it seems like it cut both to the same
//! height? not to the face?"). "Each picked face" was there, but only by a
//! hint button.
//!
//! # The claim
//!
//! The rule lives in `edit_toolpath_selection`, so every surface (GUI
//! click, MCP `edit_toolpath_selection`, a CLI job `Replace`) takes it.
//! "Default floor" is exactly `OperationConfig::default_floor`: the floor
//! of `OperationConfig::new_default` of the op type, by value.
//!
//! 1. A face pick on a default Pocket (the MCP route: `add_pick` as it is)
//!    stores the one pick (O-PICK) and the floor `EachFace { depth: 0 }`.
//!    `Effects::floor_set` names the floor; `Effects::FLOOR_SET_NOTE` is
//!    the one-line note.
//! 2. The second face pick keeps the floor and reports no floor change.
//! 3. A Pocket whose floor the operator set (here `Stock top` 2) keeps it.
//! 4. A Profile with side On cannot take "Each picked face" (the operation
//!    check, P6.3): the pick lands and the floor stays.
//! 5. An op with no floor (Trace) reports no floor change.
//! 6. The Pocket of claim 1 generates, and each face cuts to its own
//!    plane: the deepest cutting move is at the face Z.
//! 7. Lead decision (2026-10-22): Q2 applies only to a face below the
//!    stock top (the top of UX-1, tolerance
//!    `BuriedFaceRefusal::STOCK_TOP_EPS_MM`). (a) A pick of the face at the
//!    stock top keeps the default floor, gives no note, and generation does
//!    not refuse. (b) A later pick of a lower face, while the floor is
//!    still the default, sets "Each picked face".
//!
//! The undo arm (one undo restores the rule and the floor) is in the viz
//! workflow test `w4_the_first_face_pick_sets_each_face_floor_o_pick_q2`.
//!
//! # The fixture
//!
//! The stepped block of `a_face_top_never_starts_below_the_stock_top_ux1`,
//! built in memory (`build_enriched_mesh`, `io::brep_face_items`): up
//! faces at Z 30 (X 0..20), Z 20 (X 20..40), Z 10 (X 40..60), Y 0..40.
//! The stock is Z 0..30, so the Z 30 face lies at the stock top.
//!
//! Red proofs (run, red, reverted):
//! - The Q2 block of `edit_toolpath_selection` off: claim 1 failed
//!   (`floor_set` was `None`, the floor stayed the Pocket default `Top Z`
//!   3).
//! - The below-the-top test off (`has_face_below_stock_top` always true):
//!   claim 7 (a) failed (the top face pick set `EachFace`).
//!
//! ```text
//! cargo test -p rs_cam_core -q --test the_first_face_pick_sets_each_face_floor_q2
//! ```

#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

mod common;

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use common::session::toolpath_config;
use common::tools::endmill_tool_config;
use rs_cam_core::compute::StockConfig;
use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::floor::Floor;
use rs_cam_core::compute::operation_configs::ProfileSide;
use rs_cam_core::geo::{P3, V3};
use rs_cam_core::geometry::enriched_mesh::{
    FaceGroupId, FaceTessellation, SurfaceParams, SurfaceType, build_enriched_mesh,
};
use rs_cam_core::io::brep_face_items;
use rs_cam_core::selection::SelectionEdit;
use rs_cam_core::session::{
    Command, EditSelectionArgs, Effects, LoadedModel, ProjectSession, ProjectSessionBuilder,
    SelectionTarget, SetToolpathFloorArgs,
};
use rs_cam_core::toolpath::MoveType;

/// The BREP id of the Z 30 up face (X 0..20): at the stock top.
const FACE_Z30: u16 = 0;
/// The stock top (mm): the top of the part, so the Z 30 face lies at it.
const STOCK_TOP: f64 = 30.0;
/// The BREP id of the Z 20 up face (X 20..40).
const FACE_Z20: u16 = 1;
/// The BREP id of the Z 10 up face (X 40..60).
const FACE_Z10: u16 = 2;
/// The tool diameter (mm).
const TOOL_DIAMETER: f64 = 6.0;
/// The float noise of a Z (mm).
const EPS: f64 = 1e-6;

/// A flat up rectangle X `x0..x1`, Y 0..40 at `z`.
fn rect_face(x0: f64, x1: f64, z: f64) -> FaceTessellation {
    let v = vec![
        P3::new(x0, 0.0, z),
        P3::new(x1, 0.0, z),
        P3::new(x1, 40.0, z),
        P3::new(x0, 40.0, z),
    ];
    FaceTessellation {
        boundary_loops: vec![v.clone()],
        vertices: v,
        triangles: vec![[0, 1, 2], [0, 2, 3]],
        surface_type: SurfaceType::Plane,
        surface_params: SurfaceParams::Plane {
            normal: V3::new(0.0, 0.0, 1.0),
            d: z,
        },
    }
}

fn stepped_block() -> LoadedModel {
    let enriched = build_enriched_mesh(
        vec![
            rect_face(0.0, 20.0, 30.0),
            rect_face(20.0, 40.0, 20.0),
            rect_face(40.0, 60.0, 10.0),
        ],
        Vec::new(),
        Vec::new(),
    )
    .unwrap();
    LoadedModel {
        id: rs_cam_core::ids::ModelId::from_raw(0),
        name: "q2_stepped_block".to_owned(),
        mesh: Some(Arc::clone(&enriched.mesh)),
        items: Arc::new(brep_face_items(&enriched)),
        path: PathBuf::from("synthetic://q2_stepped_block.step"),
        kind: None,
        units: None,
        enriched_mesh: Some(Arc::new(enriched)),
        winding_report: None,
        load_error: None,
        import_options: Default::default(),
        import_report: None,
        drawing: Default::default(),
    }
}

/// One op with its default config on the stepped block, stock Z 0..30.
fn session(op: OperationConfig) -> ProjectSession {
    let stock = StockConfig {
        x: 70.0,
        y: 50.0,
        z: STOCK_TOP,
        origin_x: -5.0,
        origin_y: -5.0,
        origin_z: 0.0,
        auto_from_model: false,
        ..StockConfig::default()
    };
    let mut builder = ProjectSessionBuilder::new().stock(stock);
    let tool_idx = builder.add_tool(endmill_tool_config(TOOL_DIAMETER));
    let tool_id = builder.tools()[tool_idx].id.raw();
    let model_id = builder.add_model(stepped_block());
    let cfg = toolpath_config(
        "q2",
        op,
        rs_cam_core::ids::ToolId::from_raw(tool_id),
        model_id,
    );
    let _ = builder.add_toolpath(0, cfg).unwrap();
    builder.build()
}

/// A face pick sent as MCP sends it (`add_pick` with the key of the face).
fn pick(s: &mut ProjectSession, face: u16) -> Effects {
    let tc = s.toolpath_configs()[0].clone();
    let key = s
        .face_pick_key(&tc, tc.model_id, FaceGroupId(face))
        .expect("a flat face has a Face item");
    s.apply(Command::EditSelection(EditSelectionArgs {
        index: 0,
        target: SelectionTarget::Op,
        edit: SelectionEdit::AddPick(key),
    }))
    .expect("the pick applies")
}

/// The stock of the session as the add doors read it (UX-7b: a default
/// floor is the registry default or the stock default of this stock).
fn stock_ctx(s: &ProjectSession) -> rs_cam_core::feeds::suggest::StockContext {
    rs_cam_core::feeds::suggest::StockContext::from_stock_bbox(
        s.stock_bbox(),
        s.stock_config().padding,
    )
}

fn op(s: &ProjectSession) -> OperationConfig {
    s.toolpath_configs()[0].operation.clone()
}

/// Claims 1, 2 and 6.
#[test]
fn a_face_pick_on_a_default_pocket_sets_each_face() {
    let mut s = session(OperationConfig::new_default(OperationType::Pocket));
    let default_floor = op(&s).default_floor().unwrap();
    assert!(
        matches!(default_floor, Floor::Top { .. }),
        "non-vacuity: the Pocket default is Top Z"
    );
    assert!(op(&s).floor_is_default(&stock_ctx(&s)));
    assert!(op(&s).selection().unwrap().is_drawing());

    // Claim 1.
    let effects = pick(&mut s, FACE_Z20);
    assert_eq!(effects.floor_set, Some(Floor::EachFace { depth: 0.0 }));
    assert_eq!(op(&s).floor(), Some(Floor::EachFace { depth: 0.0 }));
    let picks = op(&s).selection().unwrap().clone();
    assert!(
        picks.rules().is_empty(),
        "O-PICK: no default rule {picks:?}"
    );
    assert_eq!(picks.face_picks().count(), 1);
    assert!(
        Effects::FLOOR_SET_NOTE.contains("Each picked face"),
        "the note names the floor"
    );

    // Claim 6: the face cuts to its own plane (Z 20, depth 0).
    let cancel = AtomicBool::new(false);
    s.generate_toolpath(0, &cancel)
        .expect("the face pick generates");
    let lowest = s
        .result_at(0)
        .unwrap()
        .toolpath()
        .moves
        .iter()
        .filter(|m| !matches!(m.move_type, MoveType::Rapid))
        .map(|m| m.target.z)
        .fold(f64::MAX, f64::min);
    assert!(
        (lowest - 20.0).abs() < EPS,
        "the deepest cutting move is at the face plane, Z {lowest:.3}"
    );

    // Claim 2.
    let effects = pick(&mut s, FACE_Z10);
    assert_eq!(effects.floor_set, None, "only the first face pick moves it");
    assert_eq!(op(&s).floor(), Some(Floor::EachFace { depth: 0.0 }));
    assert_eq!(op(&s).selection().unwrap().face_picks().count(), 2);
}

/// Claim 3: a floor that the operator set stays.
#[test]
fn a_hand_set_floor_stays() {
    let mut s = session(OperationConfig::new_default(OperationType::Pocket));
    let hand = Floor::StockTop { depth: 2.0 };
    let _ = s
        .apply(Command::SetToolpathFloor(SetToolpathFloorArgs {
            index: 0,
            floor: hand.clone(),
        }))
        .unwrap();
    assert!(!op(&s).floor_is_default(&stock_ctx(&s)));
    let effects = pick(&mut s, FACE_Z20);
    assert_eq!(effects.floor_set, None);
    assert_eq!(op(&s).floor(), Some(hand));
    assert_eq!(op(&s).selection().unwrap().face_picks().count(), 1);
}

/// Claims 4 and 5: an op that cannot take the face floor, and an op with
/// no floor.
#[test]
fn an_op_that_cannot_take_the_face_floor_keeps_its_floor() {
    let mut on = OperationConfig::new_default(OperationType::Profile);
    if let OperationConfig::Profile(c) = &mut on {
        c.side = ProfileSide::On;
    }
    let mut s = session(on);
    let before = op(&s).floor();
    assert!(
        op(&s).floor_is_default(&stock_ctx(&s)),
        "non-vacuity: the default floor"
    );
    let effects = pick(&mut s, FACE_Z20);
    assert_eq!(effects.floor_set, None);
    assert_eq!(op(&s).floor(), before);
    assert_eq!(op(&s).selection().unwrap().face_picks().count(), 1);

    let mut trace = session(OperationConfig::new_default(OperationType::Trace));
    let effects = pick(&mut trace, FACE_Z20);
    assert_eq!(effects.floor_set, None);
    assert_eq!(op(&trace).floor(), None);
    assert_eq!(
        op(&trace)
            .selection()
            .cloned()
            .map(|s| s.face_picks().count()),
        Some(1)
    );
}

/// Claim 7: a face at the stock top keeps the default floor; a later
/// pick of a lower face sets the face floor.
#[test]
fn a_face_at_the_stock_top_keeps_the_default_floor() {
    let mut s = session(OperationConfig::new_default(OperationType::Pocket));
    let default_floor = op(&s).floor();

    // (a) The face at the stock top.
    let effects = pick(&mut s, FACE_Z30);
    assert_eq!(effects.floor_set, None, "no Q2 note for a face at the top");
    assert_eq!(op(&s).floor(), default_floor, "the default floor stays");
    assert_eq!(op(&s).selection().unwrap().face_picks().count(), 1);
    let cancel = AtomicBool::new(false);
    s.generate_toolpath(0, &cancel)
        .expect("a pick of the top face does not refuse");

    // (b) A lower face: the floor is still the default, so Q2 applies.
    let effects = pick(&mut s, FACE_Z20);
    assert_eq!(effects.floor_set, Some(Floor::EachFace { depth: 0.0 }));
    assert_eq!(op(&s).floor(), Some(Floor::EachFace { depth: 0.0 }));
    assert_eq!(op(&s).selection().unwrap().face_picks().count(), 2);
}
