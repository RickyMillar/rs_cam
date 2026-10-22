//! Geometry selection UX-8 (operator ruling Q4, 2026-10-22): the floor
//! reference "A picked face".
//!
//! # The claim
//!
//! A Pocket picks two faces of a drawing: face A at Z -3 and face B at
//! Z -6 (the stock top is Z 0). The `SetToolpathFloor` door takes
//! `Floor::Face { key: B, depth: 1 }`. Then:
//!
//! 1. B leaves the picks (L-R5); A stays. The stored key names the
//!    toolpath model, as a pick does.
//! 2. The floor resolves to the plane of B less the depth (Z -7) on the
//!    read door (`toolpath_heights`), and the deepest fed move of the
//!    generated toolpath is at that Z.
//! 3. The floor survives a save and a load (`from = "face"`).
//! 4. A key of an item that has no plane (an open path) refuses at the
//!    door, and nothing is written.
//! 5. After a re-import without B, the read door names
//!    `FloorRefusal::FaceLost` and generation refuses with the typed
//!    selection cause `RefusalCause::FloorFaceLost` (the P2.4 arm
//!    `floor_face_lost` holds the sentence and the status parity).
//!
//! Red proof: make `Floor::resolve` give `FloorRefusal::FaceLost` for
//! every `Floor::Face`. Claim 2 fails.
//!
//! ```text
//! cargo test -p rs_cam_core -q --test a_picked_face_sets_the_floor_ux8
//! ```

#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

mod common;

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use common::session::{polygon_model, single_op_session_with, stock_under};
use common::tools::endmill_tool_config;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::floor::{Floor, FloorRefusal};
use rs_cam_core::compute::operation_configs::PocketConfig;
use rs_cam_core::geo::{P2, V3};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::selection::{
    FaceItem, Item, ItemSet, MatchKey, RefusalCause, Selection, Shape, SourceRef,
};
use rs_cam_core::session::{
    AdoptModelGeometryArgs, Command, LoadedModel, ProjectSession, SessionError,
    SetToolpathFloorArgs,
};
use rs_cam_core::toolpath::MoveType;

/// The half side of each square face (mm).
const HALF: f64 = 8.0;
/// The X of the centre of face B (mm).
const B_X: f64 = 30.0;
/// The plane Z of face A and of face B (mm).
const A_Z: f64 = -3.0;
const B_Z: f64 = -6.0;
/// The depth below face B (mm).
const DEPTH: f64 = 1.0;
/// The stock half extent and height (mm); the stock top is Z 0.
const STOCK_HALF: f64 = 60.0;
const STOCK_HEIGHT: f64 = 12.0;
/// The tool diameter (mm).
const TOOL_DIAMETER: f64 = 4.0;
/// The tolerance of a Z (mm).
const EPS: f64 = 1e-6;

fn square(cx: f64) -> Polygon2 {
    Polygon2::new(vec![
        P2::new(cx - HALF, -HALF),
        P2::new(cx + HALF, -HALF),
        P2::new(cx + HALF, HALF),
        P2::new(cx - HALF, HALF),
    ])
}

fn face(cx: f64, plane_z: f64) -> Item {
    Item::new(
        SourceRef::ToolpathModel,
        Some("FACES".into()),
        Shape::Face(FaceItem {
            rings: square(cx),
            plane_z,
            normal: V3::new(0.0, 0.0, 1.0),
            brep: None,
        }),
    )
    .unwrap()
}

fn path() -> Item {
    let mut p = Polygon2::new(vec![P2::new(-10.0, 20.0), P2::new(10.0, 20.0)]);
    p.closed = false;
    Item::new(
        SourceRef::ToolpathModel,
        Some("FACES".into()),
        Shape::Path(p),
    )
    .unwrap()
}

fn drawing(items: Vec<Item>) -> LoadedModel {
    let mut model = polygon_model(Vec::new(), "ux8_drawing");
    model.items = Arc::new(ItemSet::new(items));
    model
}

fn pick_of(item: &Item) -> MatchKey {
    MatchKey {
        source: SourceRef::ToolpathModel,
        ..item.key().clone()
    }
}

/// A Pocket that picks faces A and B, with the floor "A picked face" on B.
fn session() -> (ProjectSession, Item, Item) {
    let a = face(0.0, A_Z);
    let b = face(B_X, B_Z);
    let mut s = single_op_session_with(
        stock_under(STOCK_HALF, STOCK_HEIGHT),
        endmill_tool_config(TOOL_DIAMETER),
        drawing(vec![a.clone(), b.clone(), path()]),
        "ux8 pocket",
        OperationConfig::Pocket(PocketConfig {
            selection: Selection::from_picks(vec![pick_of(&a), pick_of(&b)]),
            ..PocketConfig::default()
        }),
        |_| {},
    );
    let mut key = b.key().clone();
    // The GUI sends the key of the drawn item, which names the model by
    // id; the door stores the toolpath form.
    key.source = SourceRef::Model(s.toolpath_configs()[0].model_id);
    let _ = s
        .apply(Command::SetToolpathFloor(SetToolpathFloorArgs {
            index: 0,
            floor: Floor::Face { key, depth: DEPTH },
        }))
        .expect("the door takes the face floor");
    (s, a, b)
}

#[test]
fn a_picked_face_sets_the_floor_ux8() {
    let (mut s, a, b) = session();
    let tc = s.toolpath_configs()[0].clone();

    // 1. B leaves the picks; A stays; the key is in the toolpath form.
    let selection = tc.operation.selection().unwrap();
    assert_eq!(selection.picks(), &[pick_of(&a)], "only face A is cut");
    assert_eq!(
        tc.operation.floor(),
        Some(Floor::Face {
            key: pick_of(&b),
            depth: DEPTH
        })
    );

    // 2. The read door and the motion give the plane of B less the depth.
    let want = B_Z - DEPTH;
    let answer = s.toolpath_heights(&tc).unwrap();
    assert_eq!(answer.floor_refusal(&tc.heights), None);
    let floor_z = answer.floor_z(&tc.heights).unwrap().unwrap();
    assert!(
        (floor_z - want).abs() < EPS,
        "floor Z {floor_z}, want {want}"
    );
    let cancel = AtomicBool::new(false);
    let result = s.generate_toolpath(0, &cancel).expect("the pocket cuts");
    let deepest = result
        .toolpath()
        .moves
        .iter()
        .filter(|m| matches!(m.move_type, MoveType::Linear { .. }))
        .map(|m| m.target.z)
        .fold(f64::INFINITY, f64::min);
    assert!(
        (deepest - want).abs() < EPS,
        "the deepest fed move is at Z {deepest}, want {want}"
    );

    // 3. A save and a load keep the floor.
    let dir = std::env::temp_dir().join(format!("rs_cam_ux8_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file: PathBuf = dir.join("project.toml");
    let mut no_model = rs_cam_core::session::ProjectSessionBuilder::new();
    let tool = no_model.add_tool(endmill_tool_config(TOOL_DIAMETER));
    let tool_id = no_model.tools()[tool].id;
    let mut saved_tc = tc.clone();
    saved_tc.tool_id = tool_id;
    let _ = no_model.add_toolpath(0, saved_tc).unwrap();
    no_model.build().save(&file).unwrap();
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(text.contains("from = \"face\""), "{text}");
    let loaded = ProjectSession::load(&file).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(
        loaded.toolpath_configs()[0].operation.floor(),
        tc.operation.floor(),
        "the face floor changed across a save and a load"
    );
}

#[test]
fn a_key_with_no_plane_refuses_at_the_door_ux8() {
    let (mut s, _, _) = session();
    let before = s.toolpath_configs()[0].operation.floor();
    let err = s
        .apply(Command::SetToolpathFloor(SetToolpathFloorArgs {
            index: 0,
            floor: Floor::Face {
                key: pick_of(&path()),
                depth: 0.0,
            },
        }))
        .expect_err("an open path is not a floor face");
    assert!(
        matches!(err, SessionError::OutOfDomain(_)),
        "a typed domain refusal, got {err:?}"
    );
    assert_eq!(s.toolpath_configs()[0].operation.floor(), before);
}

#[test]
fn a_lost_floor_face_refuses_generation_ux8() {
    let (mut s, a, _) = session();
    let model_id = s.toolpath_configs()[0].model_id;
    let _ = s
        .apply(Command::AdoptModelGeometry(AdoptModelGeometryArgs {
            model_id,
            geometry: Box::new(drawing(vec![a])),
            units: None,
        }))
        .unwrap();
    let tc = s.toolpath_configs()[0].clone();
    let answer = s.toolpath_heights(&tc).unwrap();
    assert_eq!(
        answer.floor_refusal(&tc.heights),
        Some(FloorRefusal::FaceLost)
    );
    let cancel = AtomicBool::new(false);
    let err = s.generate_toolpath(0, &cancel).map(|_| ()).unwrap_err();
    let SessionError::Selection(refusal) = err else {
        panic!("a typed selection refusal, got {err}");
    };
    assert_eq!(refusal.cause, RefusalCause::FloorFaceLost);
}
