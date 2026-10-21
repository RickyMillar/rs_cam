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
//! `FacesNotReady` (geometry selection P4.7) is the other exception.
//! Generation builds the flat faces of an STL before it resolves, so it
//! does not refuse; the status read only peeks and reports "computing
//! faces" with no refusal. Its arm runs `view` over a resolution with a
//! cold face set, and proves that generation then cuts.
//!
//! [`arm_of`] matches every cause with no wildcard, so a new cause does not
//! compile here until it has an arm; [`every_cause_has_an_arm`] holds the
//! list of arms equal to the list of causes.
//!
//! Geometry selection P3.5 adds two arms of the floor "each picked face":
//! `no_face_for_floor` (a drawing loop has no plane Z) and
//! `floor_above_top` (every face floor above the stock top; P3.7 skips a
//! face at the top when another face is cut, with a note). The status runs
//! the per-area floor rule of the prologue (`compute::floor::region_floors`)
//! with the same top, so both give one refusal.
//!
//! Two arms run on a `FaceUp::Bottom` setup (`kind_not_cut_on_a_flipped_setup`,
//! `face_faces_down`), so the frame door is proven
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

use common::session::{pick_faces, polygon_model, single_op_session_with, stock_under};
use common::tools::endmill_tool_config;
use rs_cam_core::compute::StockConfig;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::floor::Floor;
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
    SetSetupFaceArgs, SetToolpathFloorArgs,
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
        RefusalCause::FaceFacesDown => "face_faces_down",
        RefusalCause::NoFaceForFloor => "no_face_for_floor",
        RefusalCause::FloorAboveTop { .. } => "floor_above_top",
        RefusalCause::MissingSource(_) => "missing_source",
        RefusalCause::UnknownLayer(_) => "unknown_layer",
        RefusalCause::NoSuchRule { .. } => "no_such_rule",
        RefusalCause::FacesNotReady { .. } => "faces_not_ready",
        RefusalCause::HoleDoesNotFit { .. } => "hole_does_not_fit",
        RefusalCause::CannotExtend { .. } => "cannot_extend",
    }
}

/// The arms this file holds: one test function each.
const ARMS: &[&str] = &[
    "empty",
    "lost_pick",
    "ambiguous_pick",
    "kind_not_cut",
    "face_not_flat",
    "face_faces_down",
    "no_face_for_floor",
    "floor_above_top",
    "missing_source",
    "unknown_layer",
    "no_such_rule",
    "faces_not_ready",
    "hole_does_not_fit",
    "cannot_extend",
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
        .apply(Command::EditSelection(EditSelectionArgs {
            index: 0,
            target: rs_cam_core::session::SelectionTarget::Op,
            edit,
        }))
        .map(|_| ())
}

/// Generation and the status of toolpath 0 refuse with one value. Returns
/// it.
#[track_caller]
fn refusal_of(session: &mut ProjectSession) -> SelectionRefusal {
    let tc = session.toolpath_configs()[0].clone();
    let status = session
        .selection_status(&tc, rs_cam_core::session::SelectionTarget::Op)
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
        RefusalCause::FaceFacesDown,
        RefusalCause::NoFaceForFloor,
        RefusalCause::FloorAboveTop {
            floor_z: 0.0,
            top_z: 0.0,
        },
        RefusalCause::MissingSource(SourceRef::Stock),
        RefusalCause::UnknownLayer(String::new()),
        RefusalCause::NoSuchRule { index: 0, rules: 0 },
        RefusalCause::FacesNotReady {
            model: rs_cam_core::ids::ModelId(0),
        },
        RefusalCause::HoleDoesNotFit {
            fit: rs_cam_core::selection::HoleFit::Larger,
            size: rs_cam_core::selection::hole_fit::DrillSize::strict(0.0),
            rule: rs_cam_core::selection::HoleFitRule::ToolSize,
        },
        RefusalCause::CannotExtend {
            why: rs_cam_core::selection::CannotExtend::NoArea,
        },
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

/// An open path on layer `EXT` through the points `pts`.
fn open_item(pts: &[[f64; 2]]) -> Item {
    Item::new(
        SourceRef::ToolpathModel,
        Some("EXT".into()),
        Shape::Path(Polygon2::open_path(
            pts.iter().map(|&[x, y]| P2::new(x, y)).collect(),
        )),
    )
    .unwrap()
}

/// Geometry selection P6.4 (ruling O-2): a Pocket with Extend to stock
/// refuses each open path that cannot extend, by its name, under the
/// default rule too (L-10). One row per reachable `why` of `P6_DESIGN.md`
/// 3.4. The stock is X and Y -42 to 42 (`stock_under`: 2 mm past
/// `STOCK_HALF`); the Pocket grows it by the tool radius (O-2 (B)), so its
/// edge for the extension is at 43.5. `NoStock`
/// is internal: the session always resolves a stock box.
#[test]
fn cannot_extend() {
    use rs_cam_core::compute::operation_configs::OpenPaths;
    use rs_cam_core::selection::CannotExtend;
    let edge = STOCK_HALF + 2.0 + TOOL_DIAMETER / 2.0;
    // (row, open paths, items, the path the refusal names, words)
    let crossing_loop = Item::new(
        SourceRef::ToolpathModel,
        Some("ISLAND".into()),
        Shape::Loop(Polygon2::new(vec![
            P2::new(-4.0, 11.0),
            P2::new(4.0, 11.0),
            P2::new(4.0, 19.0),
            P2::new(-4.0, 19.0),
        ])),
    )
    .unwrap();
    let crossing_key = crossing_loop.key().clone();
    let rows: Vec<(&str, OpenPaths, Vec<Item>, CannotExtend, &str)> = vec![
        (
            "end_outside_stock",
            OpenPaths::ExtendLeft,
            vec![open_item(&[[-10.0, 15.0], [50.0, 15.0]])],
            CannotExtend::EndOutsideStock,
            "starts at X -10.000 Y 15.000",
        ),
        (
            "crosses_stock_edge",
            OpenPaths::ExtendLeft,
            vec![open_item(&[[-10.0, 15.0], [0.0, 50.0], [10.0, 15.0]])],
            CannotExtend::CrossesStockEdge,
            "starts at X -10.000 Y 15.000",
        ),
        (
            "self_crossing",
            OpenPaths::ExtendLeft,
            vec![open_item(&[
                [-10.0, 0.0],
                [10.0, 10.0],
                [10.0, 0.0],
                [-10.0, 10.0],
            ])],
            CannotExtend::SelfCrossing,
            "starts at X -10.000 Y 0.000",
        ),
        (
            "rays_cross",
            OpenPaths::ExtendLeft,
            vec![open_item(&[
                [-5.0, 0.0],
                [-15.0, 0.0],
                [-15.0, 10.0],
                [15.0, 10.0],
                [15.0, 0.0],
                [5.0, 0.0],
            ])],
            CannotExtend::RaysCross,
            "starts at X -5.000 Y 0.000",
        ),
        (
            "no_area",
            OpenPaths::ExtendRight,
            vec![open_item(&[[-20.0, -edge], [20.0, -edge]])],
            CannotExtend::NoArea,
            "starts at X -20.000",
        ),
        (
            "crosses_item",
            OpenPaths::ExtendLeft,
            vec![open_item(&[[-10.0, 15.0], [10.0, 15.0]]), crossing_loop],
            CannotExtend::CrossesItem {
                other: Box::new(rs_cam_core::selection::ItemRef::new(crossing_key)),
            },
            "closed loop on layer 'ISLAND'",
        ),
    ];
    for (row, open_paths, items, why, words) in rows {
        let mut s = single_op_session_with(
            stock_under(STOCK_HALF, STOCK_HEIGHT),
            endmill_tool_config(TOOL_DIAMETER),
            drawing_model(items),
            "p2_4 pocket",
            OperationConfig::Pocket(PocketConfig {
                open_paths,
                ..PocketConfig::default()
            }),
            |_| {},
        );
        let r = refusal_of(&mut s);
        let RefusalCause::CannotExtend { why: got } = &r.cause else {
            panic!("{row}: not a CannotExtend refusal: {r}");
        };
        let mut got = got.clone();
        // The item ref of a crossing loop names it by its drawing key; the
        // source in the refusal is the bound model.
        if let (CannotExtend::CrossesItem { other: g }, CannotExtend::CrossesItem { other: w }) =
            (&mut got, &why)
        {
            assert_eq!(g.key.anchor, w.key.anchor, "{row}: the loop is named");
            g.key = w.key.clone();
        }
        assert_eq!(got, why, "{row}: {r}");
        assert_names(
            &r,
            &[
                "open path on layer 'EXT'",
                words,
                "cannot extend to the stock",
            ],
        );
    }
}

/// Geometry selection P6.5 (ruling O-3): a picked circle that is not of the
/// tool size refuses by item. The circle is two tool diameters across, so
/// it is `Larger` (a helix could make it), not `LeavesCore`.
#[test]
fn hole_does_not_fit() {
    use rs_cam_core::compute::operation_configs::DrillConfig;
    use rs_cam_core::selection::HoleFit;
    let circle = Item::new(
        SourceRef::ToolpathModel,
        Some("HOLES".into()),
        Shape::circle(P2::new(5.0, 5.0), 2.0 * TOOL_DIAMETER),
    )
    .unwrap();
    let mut s = single_op_session_with(
        stock_under(STOCK_HALF, STOCK_HEIGHT),
        endmill_tool_config(TOOL_DIAMETER),
        drawing_model(vec![circle.clone()]),
        "p2_4 drill",
        OperationConfig::Drill(DrillConfig {
            selection: Selection::from_picks(vec![pick_of(&circle)]),
            ..DrillConfig::default()
        }),
        |_| {},
    );
    let r = refusal_of(&mut s);
    assert_eq!(
        r.cause,
        RefusalCause::HoleDoesNotFit {
            fit: HoleFit::Larger,
            size: rs_cam_core::selection::hole_fit::DrillSize::strict(TOOL_DIAMETER),
            rule: rs_cam_core::selection::HoleFitRule::ToolSize,
        }
    );
    let text = r.to_string();
    assert!(text.starts_with("Drill: "), "the op is not named: {text}");
    for name in [
        "circle on layer 'HOLES'",
        "centre X 5.000 Y 5.000, diameter 6.000 mm",
        "is larger than the tool diameter 3.000 mm",
    ] {
        assert!(
            text.contains(name),
            "the refusal does not name {name:?}: {text}"
        );
    }
    assert!(
        text.ends_with(&format!("Fix: {}.", r.fix())),
        "the refusal does not end with its fix: {text}"
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

/// P4.1: the boundary selection refuses as the op selection does. A pick
/// of the boundary selection that the drawing no longer holds: generation
/// stops on the boundary, and its text holds the refusal that
/// `selection_status` gives for the boundary target, word for word.
#[test]
fn a_lost_boundary_pick_refuses_as_its_status_says_p4_1() {
    use rs_cam_core::compute::config::{BoundaryConfig, BoundaryContainment, BoundarySource};
    use rs_cam_core::session::SelectionTarget;

    let a = loop_item("CUT", 0.0);
    let b = loop_item("CUT", GAP_X);
    let mut s = pocket_session(
        drawing_model(vec![a.clone(), b]),
        Selection::drawing(),
        |tc| {
            tc.boundary = BoundaryConfig {
                enabled: true,
                source: BoundarySource::Selection {
                    selection: Selection::empty(),
                    holes: false,
                },
                containment: BoundaryContainment::Center,
                offset: 0.0,
            };
        },
    );
    let _ = s
        .apply(Command::EditSelection(EditSelectionArgs {
            index: 0,
            target: SelectionTarget::Boundary,
            edit: SelectionEdit::AddPick(pick_of(&a)),
        }))
        .unwrap();
    let model_id = s.toolpath_configs()[0].model_id;
    let _ = s
        .apply(Command::AdoptModelGeometry(AdoptModelGeometryArgs {
            model_id,
            geometry: Box::new(drawing_model(vec![loop_item("CUT", GAP_X)])),
            units: None,
        }))
        .unwrap();

    let tc = s.toolpath_configs()[0].clone();
    let status = s
        .selection_status(&tc, SelectionTarget::Boundary)
        .expect("the boundary status answers")
        .refusal
        .clone()
        .expect("the boundary status gives a refusal");
    assert_eq!(status.cause, RefusalCause::LostPick);
    let text = status.to_string();
    assert!(
        text.starts_with("Machining boundary: ") && text.contains("closed loop on layer 'CUT'"),
        "the boundary refusal names the boundary and the item: {text}"
    );
    // The op selection is not the one that refuses.
    assert!(
        s.selection_status(&tc, SelectionTarget::Op)
            .unwrap()
            .refusal
            .is_none()
    );
    let cancel = AtomicBool::new(false);
    let Err(err) = s.generate_toolpath(0, &cancel) else {
        panic!("generation did not refuse; the status said: {text}");
    };
    assert!(
        err.to_string().contains(&text),
        "generation and the boundary status give different refusals:\n  generation: {err}\n  \
         status: {text}"
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
        import_options: Default::default(),
        import_report: None,
        drawing: Default::default(),
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

fn face_session() -> ProjectSession {
    single_op_session_with(
        step_stock(),
        endmill_tool_config(TOOL_DIAMETER),
        step_part(),
        "p2_4 pocket",
        OperationConfig::Pocket(PocketConfig::default()),
        |_| {},
    )
}

fn face_pick_session(face: u16) -> ProjectSession {
    let mut s = face_session();
    pick_faces(&mut s, 0, &[face]).unwrap();
    s
}

/// P2.6: a face that is not flat has no Face item, so it has no key. The
/// pick refuses at the click (`face_pick_key`), with the same cause and
/// text that generation gave when the face id was stored.
#[test]
fn face_not_flat() {
    let mut s = face_session();
    let r = pick_faces(&mut s, 0, &[1]).unwrap_err().for_op("Pocket");
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

/// P3.4 (L-9): the up face of the part points down on a Bottom setup. In
/// the closed areas of a Pocket it refuses by name, after the frame door,
/// and the fix names the setup and the face above it.
#[test]
fn face_faces_down() {
    let mut s = face_pick_session(0);
    let _ = s
        .apply(Command::SetSetupFace(SetSetupFaceArgs {
            setup_index: 0,
            face_up: FaceUp::Bottom,
        }))
        .unwrap();
    let r = refusal_of(&mut s);
    assert_eq!(r.cause, RefusalCause::FaceFacesDown);
    assert_names(
        &r,
        &["face (centre X", "this face faces down in this setup"],
    );
}

// ── The floor "each picked face" (geometry selection P3.5) ───────────

/// Set the floor of toolpath 0 through the `SetToolpathFloor` door.
fn set_floor(session: &mut ProjectSession, floor: Floor) {
    let _ = session
        .apply(Command::SetToolpathFloor(SetToolpathFloorArgs {
            index: 0,
            floor,
        }))
        .expect("the door takes the floor");
}

/// P3.5: under "each picked face" a drawing loop has no plane Z, so it has
/// no floor. It refuses by name, with the fix.
#[test]
fn no_face_for_floor() {
    let mut s = pocket_session(
        drawing_model(vec![loop_item("OUTLINE", 0.0)]),
        Selection::drawing(),
        |_| {},
    );
    set_floor(&mut s, Floor::EachFace { depth: 0.0 });
    let r = refusal_of(&mut s);
    assert_eq!(r.cause, RefusalCause::NoFaceForFloor);
    assert_names(
        &r,
        &[
            "closed loop on layer 'OUTLINE'",
            "this closed area is not a face",
            "Pick faces only",
        ],
    );
}

/// P3.5 (L-16): a face floor at or above the Top Z refuses by name. The
/// auto top under "each picked face" is the stock top (L-15), so a
/// negative depth puts the floor above it. P3.7: the op refuses only when
/// EVERY face is at the top; here the one face is
/// (`a_face_at_the_top_is_skipped_with_a_note_p3_7` holds the skip).
#[test]
fn floor_above_top() {
    let mut s = face_pick_session(0);
    let stock = step_stock();
    let stock_top = stock.origin_z + stock.z;
    let depth = FACE_Z - stock_top - 1.0;
    set_floor(&mut s, Floor::EachFace { depth });
    let r = refusal_of(&mut s);
    assert_eq!(
        r.cause,
        RefusalCause::FloorAboveTop {
            floor_z: FACE_Z - depth,
            top_z: stock_top,
        }
    );
    assert_names(&r, &["face (centre X", "is at or above the Top Z"]);
}

// ── P3.7: a face at the top is skipped with a note ──────────────────

/// An up face item: the square at `cx` of [`square`], at `plane_z`.
fn face_item(cx: f64, plane_z: f64) -> Item {
    Item::new(
        SourceRef::ToolpathModel,
        Some("FACES".into()),
        Shape::Face(rs_cam_core::selection::FaceItem {
            rings: square(cx),
            plane_z,
            normal: V3::new(0.0, 0.0, 1.0),
            brep: None,
        }),
    )
    .unwrap()
}

/// Geometry selection P3.7 (lead ruling): under "each picked face" a face
/// whose floor is at or above the top has nothing to cut. Three faces on
/// a stock with its top at Z 0: one at Z 0 (the top), one at Z -3, one at
/// Z -6.
///
/// - The status and generation agree: no refusal; ONE note that names the
///   top face ("at the top"); the generation finding holds the same note.
/// - The item states are `[Skipped, Cut, Cut]`, and `item_floor_z` gives
///   the floor of each cut face.
/// - The two lower faces are cut to their own floors, and no cutting move
///   enters the top face.
/// - With the top face alone, the op still refuses `FloorAboveTop` (the
///   `floor_above_top` arm).
///
/// Red proof: refuse `FloorAboveTop` on the first face at the top in
/// `compute::floor::region_floors` (the P3.5 rule). Generation and the
/// status then refuse, and this test fails.
#[test]
fn a_face_at_the_top_is_skipped_with_a_note_p3_7() {
    use rs_cam_core::selection::NoteCause;
    use rs_cam_core::session::ItemState;
    use rs_cam_core::toolpath::MoveType;

    const TOP: f64 = 0.0;
    let faces = vec![
        face_item(-GAP_X, TOP),
        face_item(0.0, -3.0),
        face_item(GAP_X, -6.0),
    ];
    let mut s = pocket_session(drawing_model(faces.clone()), Selection::empty(), |_| {});
    for item in &faces {
        edit(&mut s, SelectionEdit::AddPick(pick_of(item))).unwrap();
    }
    set_floor(&mut s, Floor::EachFace { depth: 0.0 });

    let tc = s.toolpath_configs()[0].clone();
    let status = s
        .selection_status(&tc, rs_cam_core::session::SelectionTarget::Op)
        .unwrap();
    assert_eq!(status.refusal, None, "one face at the top must not refuse");
    assert_eq!(status.notes.len(), 1, "{:?}", status.notes);
    let note = &status.notes[0];
    assert_eq!(
        note.cause,
        NoteCause::AtTheTop {
            floor_z: TOP,
            top_z: TOP
        }
    );
    assert_eq!(note.first.key.anchor, faces[0].key().anchor);
    let text = note.to_string();
    assert!(text.contains("at the top"), "{text}");
    assert!(text.contains("(centre X -20.000 Y 0.000"), "{text}");
    assert_eq!(
        status.item_states,
        vec![ItemState::Skipped, ItemState::Cut, ItemState::Cut]
    );
    assert_eq!(status.item_floor_z, vec![None, Some(-3.0), Some(-6.0)]);
    // The Heights read agrees: the floor of the op is the deepest face.
    let heights = s.toolpath_heights(&tc).unwrap();
    assert_eq!(heights.floor_z(&tc.heights), Some(Ok(-6.0)));

    let cancel = AtomicBool::new(false);
    let result = s.generate_toolpath(0, &cancel).expect("generation cuts");
    let finding = result
        .stats
        .selection_note
        .as_ref()
        .expect("generation names the face at the top");
    assert_eq!(finding.notes, status.notes, "status = generation");
    let cuts: Vec<P3> = result
        .toolpath()
        .moves
        .iter()
        .filter(|m| m.move_type != MoveType::Rapid)
        .map(|m| m.target)
        .collect();
    let inside = |cx: f64, p: &P3| (p.x - cx).abs() < HALF && p.y.abs() < HALF;
    let lowest = |cx: f64| {
        cuts.iter()
            .filter(|p| inside(cx, p))
            .map(|p| p.z)
            .reduce(f64::min)
    };
    assert_eq!(
        lowest(0.0),
        Some(-3.0),
        "the middle face is cut to its floor"
    );
    assert_eq!(
        lowest(GAP_X),
        Some(-6.0),
        "the low face is cut to its floor"
    );
    assert!(
        cuts.iter().all(|p| !inside(-GAP_X, p) || p.z >= TOP),
        "no cut enters the face at the top"
    );
}

// ── FacesNotReady: an STL whose flat faces are not computed (P4.7) ───

#[test]
fn faces_not_ready() {
    use rs_cam_core::compute::catalog::ItemUse;
    use rs_cam_core::ids::ModelId;
    use rs_cam_core::selection::{MeshFaces, ResolveScope, flat_region_items, resolve, view};

    let mesh = common::meshes::stepped_top();
    let z5 = flat_region_items(&mesh)
        .unwrap()
        .iter()
        .next()
        .unwrap()
        .clone();
    let mut s = single_op_session_with(
        StockConfig {
            x: 20.0,
            y: 10.0,
            z: 8.0,
            auto_from_model: false,
            ..StockConfig::default()
        },
        endmill_tool_config(TOOL_DIAMETER),
        common::session::mesh_model(mesh, "p2_4_stl"),
        "p2_4 pocket",
        OperationConfig::Pocket(PocketConfig {
            selection: Selection::from_picks(vec![pick_of(&z5)]),
            ..PocketConfig::default()
        }),
        |_| {},
    );

    // The status peeks: computing faces, no refusal.
    let tc = s.toolpath_configs()[0].clone();
    let report = s
        .selection_status(&tc, rs_cam_core::session::SelectionTarget::Op)
        .unwrap();
    assert_eq!(report.faces_not_ready, vec![ModelId(0)]);
    assert_eq!(report.refusal, None);

    // `view` over a cold face set refuses by the model.
    let model = &s.models()[0];
    let stock = s.stock_config().bbox();
    let cold = [MeshFaces {
        model: 0,
        items: None,
    }];
    let resolved = resolve(
        tc.operation.selection().unwrap(),
        &ResolveScope {
            toolpath_model: Ok(model),
            models: s.models(),
            stock_bbox: &stock,
            setup: None,
            mesh_faces: &cold,
        },
    );
    let r = view(&resolved, ItemUse::Regions)
        .unwrap_err()
        .for_op("Pocket");
    assert_eq!(r.cause, RefusalCause::FacesNotReady { model: ModelId(0) });
    assert_names(&r, &["model 0", "not computed yet"]);

    // Generation builds the faces and does not refuse.
    let cancel = AtomicBool::new(false);
    assert!(
        s.generate_toolpath(0, &cancel).is_ok(),
        "generation refused on a cold face set"
    );
}
