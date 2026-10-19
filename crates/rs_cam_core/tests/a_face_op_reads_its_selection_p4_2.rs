//! Geometry selection P4.2: the Face op reads its selection (decision D5).
//!
//! # The claim
//!
//! The Face op holds a selection. Its default is `Selection::stock()`, the
//! stock outline of the setup:
//!
//! 1. The default faces the facing rectangle: the stock XY box grown by
//!    `stock_offset` on each side, with the tool centre one radius inside
//!    it (the output before P4.2; G-OPGOLDEN holds it byte for byte).
//! 2. A viewport face click (`pick_edit`) on a Face op under the default
//!    replaces the stock rule with that one pick (ruling L-6). An explicit
//!    `AddPick` (MCP) adds the pick and keeps the stock rule.
//! 3. The picked face moves the AUTO top to the face Z in the SETUP frame:
//!    the face Z on a Top setup, and `stock top - face Z` on a
//!    `FaceUp::Bottom` setup.
//! 4. A Face op on an STL (a model with no item) and a Face op whose model
//!    id names no model both generate with the default: the stock rule
//!    reads no model, so no `MissingSource` refusal comes.
//! 5. A picked region is faced inside its outline (ruling O-3): every
//!    cutting move lies one tool radius inside the face, and the tool
//!    reaches that inset. Only the stock region grows by `stock_offset`.
//! 6. The stock ring nests with the drawing rings (lead review of P4.2): a
//!    Pocket whose selection is the stock rule plus a drawing loop inside
//!    the stock has one region with one hole, so it clears round the loop
//!    and does not cut inside it.
//!
//! # The fixture
//!
//! The part of the P4.1 sentry, built in memory: face 0, a 10 mm square up
//! face at Z 10; face 1, the same square as a down face at Z 0; face 2, a
//! wide up face at Z 2. The stock reaches 5 mm past the part, so the face
//! top and the stock top are different numbers in both frames.
//!
//! Red proofs (each one run, each one went red, each one reverted):
//!
//! - Claim 1: the stock region grown by two `stock_offset`
//!   (`generate_face`): the X range moved.
//! - Claim 2: `pick_edit` without the L-6 rule (always `AddPick`): the
//!   stock rule stayed.
//! - Claim 3: the auto top override skipped (`face_top_z` read as `None`
//!   in `resolve_generation_inputs`): the top stayed at the stock top.
//! - Claim 4: the Face default as `Selection::drawing()`: the STL arm
//!   refused `Empty`.
//! - Claim 5: every region grown by `stock_offset`: cutting moves left the
//!   face.
//! - Claim 6: the stock ring kept out of the nesting (`regions_of`, each
//!   stock item its own region first): the loop was cut.
//!
//! ```text
//! cargo test -p rs_cam_core -q --test a_face_op_reads_its_selection_p4_2
//! ```

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use common::op_fixtures::no_dressups;
use common::session::{polygon_model, toolpath_config};
use common::tools::endmill_tool_config;
use rs_cam_core::compute::StockConfig;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::{FaceConfig, PocketConfig};
use rs_cam_core::compute::transform::FaceUp;
use rs_cam_core::geo::{P2, P3, V3};
use rs_cam_core::geometry::enriched_mesh::{
    FaceGroupId, FaceTessellation, SurfaceParams, SurfaceType, build_enriched_mesh,
};
use rs_cam_core::io::brep_face_items;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::selection::{MatchKey, Selection, SelectionEdit};
use rs_cam_core::session::{
    Command, EditSelectionArgs, GenerateToolpathArgs, Job, JobHandle, LoadedModel, ProjectSession,
    ProjectSessionBuilder, SelectionTarget, SetSetupFaceArgs,
};
use rs_cam_core::toolpath::{MoveType, Toolpath};

/// The model Z of the up face (the top of the part).
const TOP_FACE_Z: f64 = 10.0;
/// The model Z of the down face (the bottom of the part).
const BOTTOM_FACE_Z: f64 = 0.0;
/// The model Z of the wide floor face.
const FLOOR_FACE_Z: f64 = 2.0;
/// The side of the square part (mm).
const PART_SIDE: f64 = 10.0;
/// The stock reaches this far past the part on every side and in Z (mm).
const STOCK_MARGIN: f64 = 5.0;
/// The tool diameter (mm): it fits the 10 mm face.
const TOOL_DIAMETER: f64 = 3.0;
/// The tool radius (mm).
const TOOL_RADIUS: f64 = TOOL_DIAMETER / 2.0;
/// The stepover of every Face op here (mm): two rows on the face.
const STEPOVER: f64 = 2.0;
/// The `stock_offset` of every Face op here (mm).
const STOCK_OFFSET: f64 = 4.0;
/// The float noise of an XY or a Z read back from the toolpath (mm).
const EPS: f64 = 1e-6;
/// The model id of claim 4 that names no model.
const NO_MODEL_ID: usize = 77;

/// A flat square face from `lo` to `hi` in X and Y at `z`, wound so that
/// its triangles face `up`.
fn square_face(lo: f64, hi: f64, z: f64, up: bool) -> FaceTessellation {
    let v = vec![
        P3::new(lo, lo, z),
        P3::new(hi, lo, z),
        P3::new(hi, hi, z),
        P3::new(lo, hi, z),
    ];
    let triangles = if up {
        vec![[0, 1, 2], [0, 2, 3]]
    } else {
        vec![[0, 2, 1], [0, 3, 2]]
    };
    FaceTessellation {
        boundary_loops: vec![v.clone()],
        vertices: v,
        triangles,
        surface_type: SurfaceType::Plane,
        surface_params: SurfaceParams::Plane {
            normal: V3::new(0.0, 0.0, if up { 1.0 } else { -1.0 }),
            d: z,
        },
    }
}

/// The part: face 0 up at [`TOP_FACE_Z`], face 1 down at
/// [`BOTTOM_FACE_Z`], face 2 the wide floor at [`FLOOR_FACE_Z`].
fn part_model() -> LoadedModel {
    let enriched = build_enriched_mesh(
        vec![
            square_face(0.0, PART_SIDE, TOP_FACE_Z, true),
            square_face(0.0, PART_SIDE, BOTTOM_FACE_Z, false),
            square_face(-STOCK_MARGIN, PART_SIDE + STOCK_MARGIN, FLOOR_FACE_Z, true),
        ],
        Vec::new(),
        Vec::new(),
    )
    .unwrap();
    LoadedModel {
        id: 0,
        name: "p4_2_part".to_owned(),
        mesh: Some(Arc::clone(&enriched.mesh)),
        items: Arc::new(brep_face_items(&enriched)),
        path: PathBuf::from("synthetic://p4_2_part.step"),
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

/// An STL: a mesh and no item.
fn stl_model() -> LoadedModel {
    LoadedModel {
        id: 0,
        name: "p4_2.stl".to_owned(),
        mesh: Some(Arc::new(rs_cam_core::mesh::make_test_flat(40.0))),
        items: Arc::default(),
        path: PathBuf::from("synthetic://p4_2.stl"),
        kind: None,
        units: None,
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
        import_options: Default::default(),
        import_report: None,
        drawing: Default::default(),
    }
}

/// The stock: [`STOCK_MARGIN`] past the part on every side and in Z.
fn stock() -> StockConfig {
    StockConfig {
        x: PART_SIDE + 2.0 * STOCK_MARGIN,
        y: PART_SIDE + 2.0 * STOCK_MARGIN,
        z: TOP_FACE_Z - BOTTOM_FACE_Z + 2.0 * STOCK_MARGIN,
        origin_x: -STOCK_MARGIN,
        origin_y: -STOCK_MARGIN,
        origin_z: BOTTOM_FACE_Z - STOCK_MARGIN,
        auto_from_model: false,
        ..StockConfig::default()
    }
}

/// The default Face op with the stepover and the stock offset of this
/// file.
fn face_op() -> OperationConfig {
    OperationConfig::Face(FaceConfig {
        stepover: STEPOVER,
        stock_offset: STOCK_OFFSET,
        ..FaceConfig::default()
    })
}

/// One default Face op with no dressups on `model` (or on the model id
/// [`NO_MODEL_ID`] when `model` is `None`), on a setup with `face_up`.
fn session(model: Option<LoadedModel>, face_up: FaceUp) -> ProjectSession {
    let mut builder = ProjectSessionBuilder::new().stock(stock());
    let tool_idx = builder.add_tool(endmill_tool_config(TOOL_DIAMETER));
    let tool_id = builder.tools()[tool_idx].id.0;
    let model_id = model.map_or(NO_MODEL_ID, |m| builder.add_model(m));
    let mut cfg = toolpath_config("Face", face_op(), tool_id, model_id);
    cfg.dressups = no_dressups();
    let _ = builder.add_toolpath(0, cfg).unwrap();
    let mut session = builder.build();
    let _ = session
        .apply(Command::SetSetupFace(SetSetupFaceArgs {
            setup_index: 0,
            face_up,
        }))
        .unwrap();
    session
}

/// The key of BREP face `face` of the toolpath model.
fn face_key(session: &ProjectSession, face: u16) -> MatchKey {
    let tc = &session.toolpath_configs()[0];
    session
        .face_pick_key(tc, tc.model_id, FaceGroupId(face))
        .expect("a flat face has a Face item")
}

/// The viewport face click: the edit `pick_edit` builds, through the
/// command door.
fn click_face(session: &mut ProjectSession, face: u16) -> SelectionEdit {
    let key = face_key(session, face);
    let tc = session.toolpath_configs()[0].clone();
    let edit = session
        .pick_edit(&tc, SelectionTarget::Op, key)
        .expect("the Face op holds a selection");
    let _ = session
        .apply(Command::EditSelection(EditSelectionArgs {
            index: 0,
            target: SelectionTarget::Op,
            edit: edit.clone(),
        }))
        .expect("the click edit applies");
    edit
}

/// The stored op selection of the Face op.
fn stored(session: &ProjectSession) -> Selection {
    session.toolpath_configs()[0]
        .operation
        .selection()
        .expect("the Face op holds a selection")
        .clone()
}

/// The top Z that generation hands the op: the request snapshot of the
/// generation job, the field the face override writes.
fn op_top_z(session: &mut ProjectSession) -> f64 {
    let cancel = AtomicBool::new(false);
    let JobHandle::GenerateToolpath(handle) = session
        .start(
            Job::GenerateToolpath(GenerateToolpathArgs { index: 0 }),
            &cancel,
        )
        .expect("step (i) captures the generation inputs")
    else {
        panic!("the generate_toolpath row answers its own handle variant");
    };
    handle.request_snapshot()["heights"]["top_z"]
        .as_f64()
        .expect("the snapshot holds the top")
}

fn generate(session: &mut ProjectSession) -> Toolpath {
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(0, &cancel)
        .unwrap_or_else(|e| panic!("the Face op generates: {e}"))
        .toolpath()
        .clone()
}

/// The XY end points of every cutting move.
fn cutting_points(tp: &Toolpath) -> Vec<P2> {
    tp.moves
        .iter()
        .filter(|m| matches!(m.move_type, MoveType::Linear { .. }))
        .map(|m| P2::new(m.target.x, m.target.y))
        .collect()
}

/// The XY box `[x0, y0, x1, y1]` of `points`.
fn xy_box(points: &[P2]) -> [f64; 4] {
    points.iter().fold(
        [
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        ],
        |[x0, y0, x1, y1], p| [x0.min(p.x), y0.min(p.y), x1.max(p.x), y1.max(p.y)],
    )
}

/// Claim 1: the default faces the facing rectangle.
#[test]
fn the_default_faces_the_stock_rectangle_p4_2() {
    let mut s = session(Some(part_model()), FaceUp::Top);
    assert!(stored(&s).is_stock(), "the default is the stock rule");
    let points = cutting_points(&generate(&mut s));
    assert!(
        points.len() > 4,
        "non-vacuity: {} cutting moves",
        points.len()
    );
    let stock = stock();
    let [x0, y0, x1, _] = xy_box(&points);
    let want_x0 = stock.origin_x - STOCK_OFFSET + TOOL_RADIUS;
    let want_x1 = stock.origin_x + stock.x + STOCK_OFFSET - TOOL_RADIUS;
    let want_y0 = stock.origin_y - STOCK_OFFSET + TOOL_RADIUS;
    assert!(
        (x0 - want_x0).abs() < EPS && (x1 - want_x1).abs() < EPS,
        "the rows run from X {x0} to {x1}, want {want_x0} to {want_x1}: the stock box grown \
         by the stock offset, less one tool radius"
    );
    assert!(
        (y0 - want_y0).abs() < EPS,
        "the first row is at Y {y0}, want {want_y0}"
    );
}

/// Claim 2: a click under the default replaces the stock rule; an
/// explicit `AddPick` adds.
#[test]
fn a_click_replaces_the_stock_rule_and_an_add_pick_adds_p4_2() {
    let mut s = session(Some(part_model()), FaceUp::Top);
    let key = face_key(&s, 0);
    let edit = click_face(&mut s, 0);
    assert!(
        matches!(edit, SelectionEdit::Replace(_)),
        "a click under the stock default replaces it: {edit:?}"
    );
    assert_eq!(
        stored(&s),
        Selection::from_picks(vec![key.clone()]),
        "one click gives one region: the pick, no stock rule"
    );
    // A second click on the same face removes the pick (the toggle).
    let edit = click_face(&mut s, 0);
    assert!(
        matches!(edit, SelectionEdit::RemovePick(_)),
        "a second click removes the pick: {edit:?}"
    );

    // An explicit `AddPick` on the stock default keeps the stock rule.
    let mut s = session(Some(part_model()), FaceUp::Top);
    let _ = s
        .apply(Command::EditSelection(EditSelectionArgs {
            index: 0,
            target: SelectionTarget::Op,
            edit: SelectionEdit::AddPick(key.clone()),
        }))
        .unwrap();
    let held = stored(&s);
    assert_eq!(
        held.rules(),
        Selection::stock().rules(),
        "the stock rule stays"
    );
    assert_eq!(held.picks(), &[key], "the pick is added");
}

/// Claim 3: the picked face sets the auto top in the setup frame.
#[test]
fn a_face_pick_sets_the_auto_top_in_the_setup_frame_p4_2() {
    let stock = stock();
    let world_stock_top = stock.origin_z + stock.z;

    let mut s = session(Some(part_model()), FaceUp::Top);
    let before = op_top_z(&mut s);
    assert!(
        (before - world_stock_top).abs() < EPS,
        "non-vacuity: the default top is the stock top {world_stock_top}, got {before}"
    );
    let _ = click_face(&mut s, 0);
    let top = op_top_z(&mut s);
    assert!(
        (top - TOP_FACE_Z).abs() < EPS,
        "Top setup: the auto top is {top}, want the face Z {TOP_FACE_Z}"
    );

    // A Bottom flip maps a world Z to `stock top - Z` in the zero-rooted
    // setup frame.
    let mut s = session(Some(part_model()), FaceUp::Bottom);
    let _ = click_face(&mut s, 1);
    let want = world_stock_top - BOTTOM_FACE_Z;
    let top = op_top_z(&mut s);
    assert!(
        (top - want).abs() < EPS,
        "Bottom setup: the auto top is {top}, want {want} (the down face at model Z \
         {BOTTOM_FACE_Z} in the setup frame)"
    );
}

/// Claim 4: a Face op on an STL and a Face op with no model generate.
#[test]
fn a_face_op_with_no_drawing_generates_p4_2() {
    for (what, model) in [("an STL", Some(stl_model())), ("no model", None)] {
        let mut s = session(model, FaceUp::Top);
        let tp = generate(&mut s);
        assert!(
            !cutting_points(&tp).is_empty(),
            "{what}: the default Face op faces the stock"
        );
    }
}

/// Claim 5: a picked region is faced inside its outline.
#[test]
fn a_picked_region_is_faced_inside_its_outline_p4_2() {
    let mut s = session(Some(part_model()), FaceUp::Top);
    let _ = click_face(&mut s, 0);
    let points = cutting_points(&generate(&mut s));
    assert!(
        points.len() > 2,
        "non-vacuity: {} cutting moves",
        points.len()
    );
    let [x0, y0, x1, y1] = xy_box(&points);
    let (lo, hi) = (TOOL_RADIUS, PART_SIDE - TOOL_RADIUS);
    assert!(
        x0 > lo - EPS && y0 > lo - EPS && x1 < hi + EPS && y1 < hi + EPS,
        "the tool left the face: the cut box is [{x0}, {y0}, {x1}, {y1}], the face less one \
         tool radius is [{lo}, {hi}]"
    );
    assert!(
        (x0 - lo).abs() < EPS && (x1 - hi).abs() < EPS && (y0 - lo).abs() < EPS,
        "the rows reach the face less one tool radius: the cut box is [{x0}, {y0}, {x1}, {y1}]"
    );
}

/// Claim 6: a Pocket with the stock rule and a drawing loop clears round
/// the loop: the loop is a hole of the stock region.
#[test]
fn a_pocket_on_the_stock_clears_round_a_drawing_loop_p4_2() {
    // The loop: a 6 mm square in the middle of the stock.
    let (lo, hi) = (2.0, 8.0);
    let mut builder = ProjectSessionBuilder::new().stock(stock());
    let tool_idx = builder.add_tool(endmill_tool_config(TOOL_DIAMETER));
    let tool_id = builder.tools()[tool_idx].id.0;
    let model_id = builder.add_model(polygon_model(
        vec![Polygon2::rectangle(lo, lo, hi, hi)],
        "p4_2_island",
    ));
    let op = OperationConfig::Pocket(PocketConfig {
        stepover: 1.0,
        depth: 1.0,
        depth_per_pass: 1.0,
        ..PocketConfig::default()
    });
    let mut cfg = toolpath_config("Pocket", op, tool_id, model_id);
    cfg.dressups = no_dressups();
    let _ = builder.add_toolpath(0, cfg).unwrap();
    let mut s = builder.build();
    let stock_rule = Selection::stock().rules()[0].clone();
    let _ = s
        .apply(Command::EditSelection(EditSelectionArgs {
            index: 0,
            target: SelectionTarget::Op,
            edit: SelectionEdit::AddRule(stock_rule),
        }))
        .unwrap();

    let tc = s.toolpath_configs()[0].clone();
    let report = s.selection_status(&tc, SelectionTarget::Op).unwrap();
    assert!(report.refusal.is_none(), "{:?}", report.refusal);
    assert_eq!(
        report.islands, 1,
        "the loop is one hole of the stock region"
    );

    let points = cutting_points(&generate(&mut s));
    assert!(
        points.len() > 4,
        "non-vacuity: {} cutting moves",
        points.len()
    );
    let inside: Vec<&P2> = points
        .iter()
        .filter(|p| p.x > lo + EPS && p.x < hi - EPS && p.y > lo + EPS && p.y < hi - EPS)
        .collect();
    assert!(
        inside.is_empty(),
        "{} cutting moves end inside the loop; first {:?}",
        inside.len(),
        inside.first()
    );
    let stock = stock();
    let [x0, _, x1, _] = xy_box(&points);
    assert!(
        x0 < stock.origin_x + 2.0 && x1 > stock.origin_x + stock.x - 2.0,
        "the pocket clears the stock round the loop: X {x0} to {x1}"
    );
}
