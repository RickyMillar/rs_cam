//! Geometry selection P2.4: every selection refusal names the item and the
//! fix, and the `selection_status` query gives the same refusal as
//! generation.
//!
//! # The claim
//!
//! For each [`RefusalCause`] there is one arm here. Each arm builds a
//! project that refuses with that cause, and then:
//!
//! 1. runs the REAL session generation path
//!    (`ProjectSession::generate_toolpath`), which must give
//!    `SessionError::Selection` with that cause;
//! 2. asserts that the sentence names the item (its kind, layer or anchor,
//!    the face id or the model id) and ends with the fix of the cause;
//! 3. asserts that `ProjectSession::selection_status` gives the SAME
//!    refusal, value for value (DESIGN 5.4: the status runs the same
//!    resolution and the same `view`).
//!
//! `NoSuchRule` is an edit refusal: no stored selection can hold it, so
//! generation cannot give it. Its arm goes through the `EditSelection`
//! command, which gives `SessionError::Selection` too.
//!
//! [`arm_of`] matches every cause with no wildcard, so a new cause does not
//! compile here until it has an arm; [`every_cause_has_an_arm`] holds the
//! list of arms equal to the list of causes.
//!
//! One arm runs on a `FaceUp::Bottom` setup, so the frame door is proven
//! not to change the verdict of the status.
//!
//! ```text
//! cargo test -p rs_cam_core -q --test selection_refusals_name_the_item_p2_4
//! ```

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use common::session::{polygon_model, single_op_session_with, stock_under};
use common::tools::endmill_tool_config;
use rs_cam_core::compute::StockConfig;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::PocketConfig;
use rs_cam_core::compute::transform::FaceUp;
use rs_cam_core::geo::{P2, P3, V3};
use rs_cam_core::geometry::enriched_mesh::{
    FaceGroupId, FaceTessellation, SurfaceParams, SurfaceType, build_enriched_mesh,
};
use rs_cam_core::io::brep_face_items;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::selection::{
    Filter, Item, ItemSet, MatchKey, RefusalCause, Rule, Selection, SelectionEdit,
    SelectionRefusal, Shape, SourceRef,
};
use rs_cam_core::session::{
    AdoptModelGeometryArgs, Command, EditSelectionArgs, LoadedModel, ProjectSession, SessionError,
    SetSetupFaceArgs,
};

/// The half side of each square loop (mm).
const HALF: f64 = 6.0;
/// The X of the centre of the second square (mm).
const GAP_X: f64 = 20.0;
/// The stock half extent and height (mm).
const STOCK_HALF: f64 = 40.0;
const STOCK_HEIGHT: f64 = 10.0;
/// The tool diameter (mm).
const TOOL_DIAMETER: f64 = 3.0;
/// A model id that no model has.
const NO_SUCH_MODEL: usize = 99;
/// The side of a face of the STEP part (mm).
const FACE_SIDE: f64 = 10.0;
/// The Z of the flat face of the STEP part (mm).
const FACE_Z: f64 = 10.0;

/// The arm name of each cause. No wildcard: a new cause stops this file
/// from compiling until it has an arm.
fn arm_of(cause: &RefusalCause) -> &'static str {
    match cause {
        RefusalCause::Empty { .. } => "empty",
        RefusalCause::LostPick => "lost_pick",
        RefusalCause::AmbiguousPick { .. } => "ambiguous_pick",
        RefusalCause::KindNotCut { .. } => "kind_not_cut",
        RefusalCause::FaceNotFlat { .. } => "face_not_flat",
        RefusalCause::MissingSource(_) => "missing_source",
        RefusalCause::UnknownLayer(_) => "unknown_layer",
        RefusalCause::NoSuchRule { .. } => "no_such_rule",
    }
}

/// The arms this file holds: one test function each.
const ARMS: &[&str] = &[
    "empty",
    "lost_pick",
    "ambiguous_pick",
    "kind_not_cut",
    "face_not_flat",
    "missing_source",
    "unknown_layer",
    "no_such_rule",
];

fn square(cx: f64) -> Polygon2 {
    Polygon2::new(vec![
        P2::new(cx - HALF, -HALF),
        P2::new(cx + HALF, -HALF),
        P2::new(cx + HALF, HALF),
        P2::new(cx - HALF, HALF),
    ])
}

fn loop_item(layer: &str, cx: f64) -> Item {
    Item::new(
        SourceRef::ToolpathModel,
        Some(layer.into()),
        Shape::Loop(square(cx)),
    )
    .unwrap()
}

/// An open path on `layer`, from X -10 to X 10 at Y 15.
fn path_item(layer: &str) -> Item {
    let mut path = Polygon2::new(vec![P2::new(-10.0, 15.0), P2::new(10.0, 15.0)]);
    path.closed = false;
    Item::new(
        SourceRef::ToolpathModel,
        Some(layer.into()),
        Shape::Path(path),
    )
    .unwrap()
}

fn drawing_model(items: Vec<Item>) -> LoadedModel {
    let mut model = polygon_model(Vec::new(), "p2_4_drawing");
    model.items = Arc::new(ItemSet::new(items));
    model
}

/// One Pocket over `model` with `selection`; `tweak` adjusts the toolpath.
fn pocket_session(
    model: LoadedModel,
    selection: Selection,
    tweak: impl FnOnce(&mut rs_cam_core::session::ToolpathConfig),
) -> ProjectSession {
    single_op_session_with(
        stock_under(STOCK_HALF, STOCK_HEIGHT),
        endmill_tool_config(TOOL_DIAMETER),
        model,
        "p2_4 pocket",
        OperationConfig::Pocket(PocketConfig {
            selection,
            ..PocketConfig::default()
        }),
        tweak,
    )
}

/// The pick key of `item` on the toolpath model.
fn pick_of(item: &Item) -> MatchKey {
    MatchKey {
        source: SourceRef::ToolpathModel,
        ..item.key().clone()
    }
}

fn edit(session: &mut ProjectSession, edit: SelectionEdit) -> Result<(), SessionError> {
    session
        .apply(Command::EditSelection(EditSelectionArgs { index: 0, edit }))
        .map(|_| ())
}

/// Generation and the status of toolpath 0 refuse with one value. Returns
/// it.
#[track_caller]
fn refusal_of(session: &mut ProjectSession) -> SelectionRefusal {
    let tc = session.toolpath_configs()[0].clone();
    let status = session
        .selection_status(&tc)
        .expect("the status answers")
        .refusal
        .clone()
        .expect("the status gives a refusal");
    let cancel = AtomicBool::new(false);
    let Err(err) = session.generate_toolpath(0, &cancel) else {
        panic!("generation did not refuse; the status said: {status}");
    };
    let SessionError::Selection(generated) = err else {
        panic!("generation gave an error that is not a selection refusal: {err}");
    };
    assert_eq!(
        generated, status,
        "generation and selection_status give different refusals"
    );
    generated
}

/// The sentence names the op, every phrase in `names`, and the fix.
#[track_caller]
fn assert_names(refusal: &SelectionRefusal, names: &[&str]) {
    let text = refusal.to_string();
    assert!(text.starts_with("Pocket: "), "the op is not named: {text}");
    for name in names {
        assert!(
            text.contains(name),
            "the refusal does not name {name:?}: {text}"
        );
    }
    assert!(
        text.ends_with(&format!("Fix: {}.", refusal.fix())),
        "the refusal does not end with its fix: {text}"
    );
}

#[test]
fn every_cause_has_an_arm() {
    let samples = [
        RefusalCause::Empty { skipped: vec![] },
        RefusalCause::LostPick,
        RefusalCause::AmbiguousPick { matches: 2 },
        RefusalCause::KindNotCut {
            kind: rs_cam_core::selection::ItemKind::Path,
            item_use: rs_cam_core::compute::catalog::ItemUse::Regions,
        },
        RefusalCause::FaceNotFlat { face: None },
        RefusalCause::MissingSource(SourceRef::Stock),
        RefusalCause::UnknownLayer(String::new()),
        RefusalCause::NoSuchRule { index: 0, rules: 0 },
    ];
    let causes: BTreeSet<&str> = samples.iter().map(arm_of).collect();
    let arms: BTreeSet<&str> = ARMS.iter().copied().collect();
    assert_eq!(causes, arms, "a cause has no arm here, or an arm no cause");
}

#[test]
fn empty() {
    let mut s = pocket_session(
        drawing_model(vec![path_item("TEXT")]),
        Selection::drawing(),
        |_| {},
    );
    let r = refusal_of(&mut s);
    assert!(matches!(&r.cause, RefusalCause::Empty { skipped } if !skipped.is_empty()));
    assert_names(
        &r,
        &["open path on layer 'TEXT'", "starts at X -10.000 Y 15.000"],
    );
}

#[test]
fn lost_pick() {
    let a = loop_item("CUT", 0.0);
    let b = loop_item("CUT", GAP_X);
    let mut s = pocket_session(
        drawing_model(vec![a.clone(), b]),
        Selection::drawing(),
        |_| {},
    );
    edit(&mut s, SelectionEdit::AddPick(pick_of(&a))).unwrap();
    // The drawing changes: the picked loop is gone after a re-import.
    let model_id = s.toolpath_configs()[0].model_id;
    let _ = s
        .apply(Command::AdoptModelGeometry(AdoptModelGeometryArgs {
            model_id,
            geometry: Box::new(drawing_model(vec![loop_item("CUT", GAP_X)])),
            units: None,
        }))
        .unwrap();
    let r = refusal_of(&mut s);
    assert_eq!(r.cause, RefusalCause::LostPick);
    assert_names(
        &r,
        &["closed loop on layer 'CUT'", "centre X 0.000 Y 0.000"],
    );
}

#[test]
fn ambiguous_pick() {
    // A duplicate entity: two equal loops.
    let a = loop_item("CUT", 0.0);
    let mut s = pocket_session(
        drawing_model(vec![a.clone(), a.clone()]),
        Selection::drawing(),
        |_| {},
    );
    edit(&mut s, SelectionEdit::AddPick(pick_of(&a))).unwrap();
    let r = refusal_of(&mut s);
    assert_eq!(r.cause, RefusalCause::AmbiguousPick { matches: 2 });
    assert_names(
        &r,
        &["closed loop on layer 'CUT'", "2 items of the drawing"],
    );
}

#[test]
fn kind_not_cut() {
    let path = path_item("TEXT");
    let mut s = pocket_session(
        drawing_model(vec![loop_item("CUT", 0.0), path.clone()]),
        Selection::drawing(),
        |_| {},
    );
    edit(&mut s, SelectionEdit::AddPick(pick_of(&path))).unwrap();
    let r = refusal_of(&mut s);
    assert!(matches!(r.cause, RefusalCause::KindNotCut { .. }), "{r}");
    assert_names(&r, &["open path on layer 'TEXT'", "cuts closed areas only"]);
}

/// The same refusal on a flipped setup: the status and generation agree
/// after the frame door, and the item is named as the drawing holds it.
#[test]
fn kind_not_cut_on_a_flipped_setup() {
    let path = path_item("TEXT");
    let mut s = pocket_session(
        drawing_model(vec![loop_item("CUT", 0.0), path.clone()]),
        Selection::drawing(),
        |_| {},
    );
    let _ = s
        .apply(Command::SetSetupFace(SetSetupFaceArgs {
            setup_index: 0,
            face_up: FaceUp::Bottom,
        }))
        .unwrap();
    edit(&mut s, SelectionEdit::AddPick(pick_of(&path))).unwrap();
    let r = refusal_of(&mut s);
    assert!(matches!(r.cause, RefusalCause::KindNotCut { .. }), "{r}");
    assert_names(
        &r,
        &["open path on layer 'TEXT'", "starts at X -10.000 Y 15.000"],
    );
}

#[test]
fn missing_source() {
    let mut s = pocket_session(
        drawing_model(vec![loop_item("CUT", 0.0)]),
        Selection::drawing(),
        |tc| tc.model_id = NO_SUCH_MODEL,
    );
    let r = refusal_of(&mut s);
    assert_eq!(
        r.cause,
        RefusalCause::MissingSource(SourceRef::Model(rs_cam_core::ids::ModelId(NO_SUCH_MODEL)))
    );
    assert_names(&r, &[&format!("model {NO_SUCH_MODEL}"), "does not exist"]);
}

#[test]
fn unknown_layer() {
    let rule = Selection::from_rules(vec![Rule {
        source: SourceRef::ToolpathModel,
        filter: Filter {
            layers: vec!["NOPE".to_owned()],
            ..Filter::default()
        },
    }]);
    let mut s = pocket_session(drawing_model(vec![loop_item("CUT", 0.0)]), rule, |_| {});
    let r = refusal_of(&mut s);
    assert_eq!(r.cause, RefusalCause::UnknownLayer("NOPE".to_owned()));
    assert_names(&r, &["layer 'NOPE'"]);
}

#[test]
fn no_such_rule() {
    let mut s = pocket_session(
        drawing_model(vec![loop_item("CUT", 0.0)]),
        Selection::drawing(),
        |_| {},
    );
    let Err(SessionError::Selection(r)) = edit(&mut s, SelectionEdit::RemoveRule(5)) else {
        panic!("an edit of a rule that does not exist did not refuse as a selection refusal");
    };
    assert_eq!(r.cause, RefusalCause::NoSuchRule { index: 5, rules: 1 });
    assert_names(&r, &["rule 5"]);
}

// ── FaceNotFlat: a STEP part ─────────────────────────────────────────

/// Face 0: a flat up face at [`FACE_Z`]. Face 1: a vertical plane face at
/// X = [`FACE_SIDE`], which has no Face item (`io::brep_face_items` makes
/// one for a flat horizontal face only).
fn step_part() -> LoadedModel {
    let flat = {
        let v = vec![
            P3::new(0.0, 0.0, FACE_Z),
            P3::new(FACE_SIDE, 0.0, FACE_Z),
            P3::new(FACE_SIDE, FACE_SIDE, FACE_Z),
            P3::new(0.0, FACE_SIDE, FACE_Z),
        ];
        FaceTessellation {
            boundary_loops: vec![v.clone()],
            vertices: v,
            triangles: vec![[0, 1, 2], [0, 2, 3]],
            surface_type: SurfaceType::Plane,
            surface_params: SurfaceParams::Plane {
                normal: V3::new(0.0, 0.0, 1.0),
                d: FACE_Z,
            },
        }
    };
    let wall = {
        let v = vec![
            P3::new(FACE_SIDE, 0.0, 0.0),
            P3::new(FACE_SIDE, FACE_SIDE, 0.0),
            P3::new(FACE_SIDE, FACE_SIDE, FACE_Z),
            P3::new(FACE_SIDE, 0.0, FACE_Z),
        ];
        FaceTessellation {
            boundary_loops: vec![v.clone()],
            vertices: v,
            triangles: vec![[0, 1, 2], [0, 2, 3]],
            surface_type: SurfaceType::Plane,
            surface_params: SurfaceParams::Plane {
                normal: V3::new(1.0, 0.0, 0.0),
                d: FACE_SIDE,
            },
        }
    };
    let enriched = build_enriched_mesh(vec![flat, wall], Vec::new(), Vec::new()).unwrap();
    LoadedModel {
        id: 0,
        name: "p2_4_part".to_owned(),
        mesh: Some(Arc::clone(&enriched.mesh)),
        items: Arc::new(brep_face_items(&enriched)),
        path: PathBuf::from("synthetic://p2_4_part.step"),
        kind: None,
        units: None,
        enriched_mesh: Some(Arc::new(enriched)),
        winding_report: None,
        load_error: None,
    }
}

fn step_stock() -> StockConfig {
    StockConfig {
        x: FACE_SIDE + 10.0,
        y: FACE_SIDE + 10.0,
        z: FACE_Z + 10.0,
        origin_x: -5.0,
        origin_y: -5.0,
        origin_z: -5.0,
        auto_from_model: false,
        ..StockConfig::default()
    }
}

fn face_pick_session(face: u16) -> ProjectSession {
    single_op_session_with(
        step_stock(),
        endmill_tool_config(TOOL_DIAMETER),
        step_part(),
        "p2_4 pocket",
        OperationConfig::Pocket(PocketConfig::default()),
        |tc| tc.face_selection = Some(vec![FaceGroupId(face)]),
    )
}

#[test]
fn face_not_flat() {
    let mut s = face_pick_session(1);
    let r = refusal_of(&mut s);
    assert_eq!(
        r.cause,
        RefusalCause::FaceNotFlat {
            face: Some(FaceGroupId(1))
        }
    );
    assert_names(&r, &["face 1", "not flat and horizontal"]);
}

/// A flat face on a lateral setup stands on its side in the setup frame:
/// it refuses by name, after the frame door.
#[test]
fn face_not_flat_on_a_lateral_setup() {
    let mut s = face_pick_session(0);
    let _ = s
        .apply(Command::SetSetupFace(SetSetupFaceArgs {
            setup_index: 0,
            face_up: FaceUp::Front,
        }))
        .unwrap();
    let r = refusal_of(&mut s);
    assert_eq!(r.cause, RefusalCause::FaceNotFlat { face: None });
    assert_names(
        &r,
        &[
            "face (centre X",
            "not flat and horizontal in the setup frame",
        ],
    );
}
