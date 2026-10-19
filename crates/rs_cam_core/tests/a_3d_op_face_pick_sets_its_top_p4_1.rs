//! Geometry selection P4.1: a 3D op takes a face pick through its
//! boundary selection, cuts only inside the face, and its auto top follows
//! the face in the SETUP frame.
//!
//! # The claim
//!
//! A 3D op holds no op selection (DESIGN 6: it uses a selection only
//! through its boundary). Ruling O-1 (A): a face pick of a 3D op goes into
//! the selection of a `BoundarySource::Selection` boundary
//! (`SelectionTarget::Boundary`), and the auto top follows the picked
//! faces, as it did before P2.6 removed `face_selection`:
//!
//! 1. On an identity setup, a pick of the up face at model Z
//!    [`TOP_FACE_Z`] gives the op the top [`TOP_FACE_Z`] (the request
//!    snapshot of the generation: the heights the op receives), and every
//!    cutting move lies inside the face.
//! 2. Non-vacuity: with the boundary off, the top is the stock top and the
//!    op cuts outside the face.
//! 3. On a `FaceUp::Bottom` setup, a pick of the down face at model Z
//!    [`BOTTOM_FACE_Z`] gives the top that the flip gives that face:
//!    `stock top - BOTTOM_FACE_Z`, not the model Z and not the setup-frame
//!    stock top.
//! 4. A pinned top stays: the face does not move it.
//! 5. A boundary whose source is not a selection refuses the edit, names
//!    the fix, and keeps its source.
//! 6. The boundary report (`selection_status` with the boundary target)
//!    holds the face, the op report holds nothing, and the memo keeps the
//!    two apart.
//!
//! # The fixture
//!
//! A part built in memory (`build_enriched_mesh`): face 0, a 10 mm square
//! up face at Z 10; face 1, the same square as a down face at Z 0; face 2, a
//! wide up face at Z 2 under the whole stock. The drop cutter rides face 0
//! inside the square and face 2 outside it, so a cut outside the square
//! exists when the boundary is off. The stock reaches 5 mm past the part,
//! so the face top, the stock top and the model-frame top are different
//! numbers in both frames.
//!
//! Red proof: the auto top read only the op selection (the P2.6 state):
//! claims 1 and 3 fail.
//!
//! ```text
//! cargo test -p rs_cam_core -q --test a_3d_op_face_pick_sets_its_top_p4_1
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

use common::session::{pick_faces_in, toolpath_config};
use common::tools::endmill_tool_config;
use rs_cam_core::compute::StockConfig;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::{
    BoundaryConfig, BoundaryContainment, BoundarySource, HeightMode,
};
use rs_cam_core::compute::operation_configs::DropCutterConfig;
use rs_cam_core::compute::transform::FaceUp;
use rs_cam_core::geo::{P2, P3, V3};
use rs_cam_core::geometry::enriched_mesh::{
    FaceGroupId, FaceTessellation, SurfaceParams, SurfaceType, build_enriched_mesh,
};
use rs_cam_core::io::brep_face_items;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::selection::{ItemKind, Selection, SelectionEdit};
use rs_cam_core::session::{
    Command, EditSelectionArgs, GenerateToolpathArgs, Job, JobHandle, LoadedModel, ProjectSession,
    ProjectSessionBuilder, SelectionTarget, SessionError, SetSetupFaceArgs,
};
use rs_cam_core::toolpath::MoveType;

/// The model Z of the up face (the top of the part).
const TOP_FACE_Z: f64 = 10.0;
/// The model Z of the down face (the bottom of the part).
const BOTTOM_FACE_Z: f64 = 0.0;
/// The model Z of the wide floor face under the stock.
const FLOOR_FACE_Z: f64 = 2.0;
/// The side of the square part (mm).
const PART_SIDE: f64 = 10.0;
/// The stock reaches this far past the part on every side and in Z (mm).
const STOCK_MARGIN: f64 = 5.0;
/// The tool diameter (mm): it fits the 10 mm face.
const TOOL_DIAMETER: f64 = 3.0;
/// The pinned top of claim 4 (mm): not a face Z and not the stock top.
const PINNED_TOP_Z: f64 = 7.0;
/// The float noise of a Z or an XY read back from the toolpath (mm).
const EPS: f64 = 1e-6;
/// Samples along each cutting segment, ends included.
const SEGMENT_SAMPLES: usize = 5;

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
        name: "p4_1_part".to_owned(),
        mesh: Some(Arc::clone(&enriched.mesh)),
        items: Arc::new(brep_face_items(&enriched)),
        path: PathBuf::from("synthetic://p4_1_part.step"),
        kind: None,
        units: None,
        enriched_mesh: Some(Arc::new(enriched)),
        winding_report: None,
        load_error: None,
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

/// A boundary of `source`, on, with `Center` containment.
fn boundary(source: BoundarySource) -> BoundaryConfig {
    BoundaryConfig {
        enabled: true,
        source,
        containment: BoundaryContainment::Center,
        offset: 0.0,
    }
}

/// An empty selection boundary: the boundary a face pick goes into.
fn empty_selection_boundary() -> BoundaryConfig {
    boundary(BoundarySource::Selection {
        selection: Selection::empty(),
        holes: false,
    })
}

/// One drop cutter on the part with `boundary` and `top`, on a setup with
/// `face_up`. The face picks go in after the flip, as an operator picks
/// on the flipped part.
fn session(boundary: BoundaryConfig, top: HeightMode, face_up: FaceUp) -> ProjectSession {
    let mut builder = ProjectSessionBuilder::new().stock(stock());
    let tool_idx = builder.add_tool(endmill_tool_config(TOOL_DIAMETER));
    let tool_id = builder.tools()[tool_idx].id.0;
    let model_id = builder.add_model(part_model());
    let op = OperationConfig::DropCutter(DropCutterConfig {
        stepover: 1.0,
        ..DropCutterConfig::default()
    });
    let mut cfg = toolpath_config("3D face pick", op, tool_id, model_id);
    cfg.boundary = boundary;
    cfg.heights.top_z = top;
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

/// The top Z that generation hands the op: the request snapshot of the
/// generation job (step (i)), the field the face override writes.
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

/// XY samples along every cutting move of a fresh generation.
fn cutting_samples(session: &mut ProjectSession) -> Vec<P2> {
    let cancel = AtomicBool::new(false);
    let result = session
        .generate_toolpath(0, &cancel)
        .expect("the 3D op generates");
    let moves = &result.toolpath().moves;
    let mut samples = Vec::new();
    for pair in moves.windows(2) {
        let (from, to) = (&pair[0], &pair[1]);
        if let MoveType::Linear { .. } = to.move_type {
            for k in 0..SEGMENT_SAMPLES {
                let t = k as f64 / (SEGMENT_SAMPLES - 1) as f64;
                samples.push(P2::new(
                    from.target.x + t * (to.target.x - from.target.x),
                    from.target.y + t * (to.target.y - from.target.y),
                ));
            }
        }
    }
    samples
}

fn face_square() -> Polygon2 {
    Polygon2::new(vec![
        P2::new(0.0, 0.0),
        P2::new(PART_SIDE, 0.0),
        P2::new(PART_SIDE, PART_SIDE),
        P2::new(0.0, PART_SIDE),
    ])
}

/// The stock top in the setup frame of a `FaceUp::Bottom` setup: the setup
/// frame is zero-rooted at the stock bottom, so it is the stock height.
fn flipped_stock_top() -> f64 {
    stock().z
}

#[test]
fn a_face_pick_on_an_identity_setup_sets_the_top_and_confines_the_cut_p4_1() {
    let mut s = session(empty_selection_boundary(), HeightMode::Auto, FaceUp::Top);
    pick_faces_in(&mut s, 0, SelectionTarget::Boundary, &[0]).unwrap();

    let top = op_top_z(&mut s);
    assert!(
        (top - TOP_FACE_Z).abs() < EPS,
        "the auto top is {top}, want the picked face Z {TOP_FACE_Z}"
    );

    let face = face_square();
    let samples = cutting_samples(&mut s);
    assert!(
        samples.len() > 20,
        "a confined op that emits almost no cutting motion proves nothing: {} samples",
        samples.len()
    );
    let outside: Vec<&P2> = samples
        .iter()
        .filter(|p| !face.contains_point_eps(p, EPS))
        .collect();
    assert!(
        outside.is_empty(),
        "{} of {} cutting samples lie outside the picked face. First: {:?}",
        outside.len(),
        samples.len(),
        outside.first()
    );
}

#[test]
fn without_the_boundary_the_top_is_the_stock_and_the_cut_leaves_the_face_p4_1() {
    let mut off = empty_selection_boundary();
    off.enabled = false;
    let mut s = session(off, HeightMode::Auto, FaceUp::Top);
    pick_faces_in(&mut s, 0, SelectionTarget::Boundary, &[0]).unwrap();

    let stock = stock();
    let top = op_top_z(&mut s);
    assert!(
        (top - TOP_FACE_Z).abs() > 1.0,
        "with the boundary off the face must not set the top; got {top}"
    );
    assert!(
        (top - (stock.origin_z + stock.z)).abs() < EPS,
        "with the boundary off the top is the stock top; got {top}"
    );
    let face = face_square();
    let samples = cutting_samples(&mut s);
    assert!(
        samples.iter().any(|p| !face.contains_point_eps(p, EPS)),
        "the unbounded op must cut outside the face, or the confinement claim is vacuous"
    );
}

#[test]
fn a_face_pick_on_a_flipped_setup_sets_the_top_in_the_setup_frame_p4_1() {
    let mut s = session(empty_selection_boundary(), HeightMode::Auto, FaceUp::Bottom);
    pick_faces_in(&mut s, 0, SelectionTarget::Boundary, &[1]).unwrap();

    // A Bottom flip maps a world Z to `stock top - Z` in the zero-rooted
    // setup frame.
    let stock = stock();
    let world_stock_top = stock.origin_z + stock.z;
    let want = world_stock_top - BOTTOM_FACE_Z;
    let top = op_top_z(&mut s);
    assert!(
        (top - want).abs() < EPS,
        "the auto top is {top}, want {want}: the down face at model Z {BOTTOM_FACE_Z} is at \
         setup Z {want} on a Bottom setup (setup stock top {})",
        flipped_stock_top()
    );
}

#[test]
fn a_pinned_top_stays_p4_1() {
    let mut s = session(
        empty_selection_boundary(),
        HeightMode::Manual(PINNED_TOP_Z),
        FaceUp::Top,
    );
    pick_faces_in(&mut s, 0, SelectionTarget::Boundary, &[0]).unwrap();
    let top = op_top_z(&mut s);
    assert!(
        (top - PINNED_TOP_Z).abs() < EPS,
        "a pinned top is the operator's own answer; got {top}, want {PINNED_TOP_Z}"
    );
}

#[test]
fn a_boundary_that_is_not_a_selection_refuses_the_pick_p4_1() {
    let mut s = session(
        boundary(BoundarySource::Stock),
        HeightMode::Auto,
        FaceUp::Top,
    );
    let tc = s.toolpath_configs()[0].clone();
    let key = s.face_pick_key(&tc, tc.model_id, FaceGroupId(0)).unwrap();
    let err = s
        .apply(Command::EditSelection(EditSelectionArgs {
            index: 0,
            target: SelectionTarget::Boundary,
            edit: SelectionEdit::AddPick(key.clone()),
        }))
        .expect_err("a Stock boundary holds no selection");
    let text = err.to_string();
    assert!(
        matches!(err, SessionError::InvalidParam(_))
            && text.contains("3D face pick")
            && text.contains("set the boundary source to Selection"),
        "the refusal names the toolpath and the fix: {text}"
    );
    assert!(
        matches!(
            s.toolpath_configs()[0].boundary.source,
            BoundarySource::Stock
        ),
        "the command never changes the boundary source"
    );
    // The click route refuses the same way, before any edit (L-8).
    let pick = s.pick_edit(&tc, SelectionTarget::Boundary, key);
    assert!(
        pick.is_err_and(|e| e
            .to_string()
            .contains("set the boundary source to Selection")),
        "the click route names the fix"
    );
    // The op target of a 3D op holds no selection either.
    let op_err = s
        .apply(Command::EditSelection(EditSelectionArgs {
            index: 0,
            target: SelectionTarget::Op,
            edit: SelectionEdit::Reset,
        }))
        .expect_err("a 3D op holds no op selection");
    assert!(
        op_err.to_string().contains("holds no selection"),
        "{op_err}"
    );
}

#[test]
fn the_boundary_report_holds_the_face_and_the_memo_keeps_the_targets_apart_p4_1() {
    let mut s = session(empty_selection_boundary(), HeightMode::Auto, FaceUp::Top);
    pick_faces_in(&mut s, 0, SelectionTarget::Boundary, &[0]).unwrap();
    let tc = s.toolpath_configs()[0].clone();
    assert_eq!(
        SelectionTarget::for_face_pick(&tc),
        SelectionTarget::Boundary,
        "a face click on a 3D op edits its boundary"
    );

    let builds = s.selection_status_builds();
    let boundary = s.selection_status(&tc, SelectionTarget::Boundary).unwrap();
    let op = s.selection_status(&tc, SelectionTarget::Op).unwrap();
    assert_eq!(
        s.selection_status_builds() - builds,
        2,
        "one build per target"
    );
    assert_eq!(
        boundary.counts.get(&ItemKind::Face),
        Some(&1),
        "{boundary:?}"
    );
    assert!(boundary.refusal.is_none(), "{boundary:?}");
    assert_eq!(boundary.selected.len(), 1);
    assert_eq!(boundary.selected[0].brep, Some(0));
    assert!(op.selected.is_empty(), "a 3D op selects nothing of its own");

    // Each target answers from its own memo entry.
    let again = s.selection_status(&tc, SelectionTarget::Boundary).unwrap();
    let _ = s.selection_status(&tc, SelectionTarget::Op).unwrap();
    assert_eq!(s.selection_status_builds() - builds, 2, "no rebuild");
    assert!(Arc::ptr_eq(&boundary, &again));

    // A second click on the same face removes the pick: `pick_edit` reads
    // the boundary report.
    let key = s.face_pick_key(&tc, tc.model_id, FaceGroupId(0)).unwrap();
    let edit = s
        .pick_edit(&tc, SelectionTarget::Boundary, key.clone())
        .unwrap();
    assert_eq!(edit, SelectionEdit::RemovePick(key));
}
