//! Geometry selection P3.4: a down face gives no top and no floor.
//!
//! # The claim
//!
//! Only an UP face, in the SETUP frame (after the frame door), gives a top
//! (`ResolvedSelection::face_top_z`) or a closed area to an op. A down face
//! has its material above its plane: the tool cannot stand on it, and its
//! plane is not the top of the material (`P3_DESIGN.md` section 6, lead
//! ruling L-9).
//!
//! 1. Top setup: a boundary pick of the up face at Z 0 and the down face at
//!    Z −18 gives the auto top 0. A boundary pick of the down face alone
//!    gives no face top, so the auto top is the stock top (before P3.4 it
//!    was −18).
//! 2. Top setup: a boundary keeps a down face (DESIGN 6 rule 4: an outline,
//!    no Z). The status gives no refusal, and generation cuts.
//! 3. Top setup: a Pocket pick of the down face refuses `FaceFacesDown`.
//!    Generation and `selection_status` give one refusal, which names the
//!    face and the fix. The report counts the face as down.
//! 4. Bottom setup: the same face points up, and the Pocket cuts from its
//!    setup-frame Z. The report counts the face as up.
//! 5. `--features step`: the STEP door gives each flat face the normal that
//!    points out of the solid (`io::brep_face_items`). Two fixtures
//!    tessellate inward; before the fix their top faces read down.
//!
//! # The fixture
//!
//! No STEP file in `fixtures/gui_step/` or `tests/fixtures/step/` holds an
//! up face at Z 0 and a down face at Z −18. The part is built in memory
//! (`build_enriched_mesh` and `io::brep_face_items`), as the P2.3 and P4.1
//! sentries build theirs: a 10 mm square up face at [`UP_FACE_Z`] and the
//! same square as a down face at [`DOWN_FACE_Z`]. The stock reaches
//! [`STOCK_MARGIN`] past the part on every side and in Z.
//!
//! ```text
//! cargo test -p rs_cam_core -q --test a_down_face_gives_no_top_and_no_floor_p3_4
//! cargo test -p rs_cam_core -q --features step --test a_down_face_gives_no_top_and_no_floor_p3_4
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
use rs_cam_core::compute::operation_configs::{DropCutterConfig, PocketConfig};
use rs_cam_core::compute::transform::FaceUp;
use rs_cam_core::geo::{P3, V3};
use rs_cam_core::geometry::enriched_mesh::{
    FaceTessellation, SurfaceParams, SurfaceType, build_enriched_mesh,
};
use rs_cam_core::io::brep_face_items;
use rs_cam_core::selection::{FacesByFacing, RefusalCause, Selection, SelectionRefusal};
use rs_cam_core::session::{
    Command, GenerateToolpathArgs, Job, JobHandle, LoadedModel, ProjectSession,
    ProjectSessionBuilder, SelectionTarget, SessionError, SetSetupFaceArgs,
};
use rs_cam_core::toolpath::MoveType;

/// The model Z of the up face (face 0).
const UP_FACE_Z: f64 = 0.0;
/// The model Z of the down face (face 1).
const DOWN_FACE_Z: f64 = -18.0;
/// The BREP id of the up face.
const UP_FACE: u16 = 0;
/// The BREP id of the down face.
const DOWN_FACE: u16 = 1;
/// The side of the square part (mm).
const PART_SIDE: f64 = 10.0;
/// The stock reaches this far past the part on every side and in Z (mm).
const STOCK_MARGIN: f64 = 5.0;
/// The pocket depth (mm). One pass: `depth_per_pass` equals it.
const POCKET_DEPTH: f64 = 2.0;
/// The tool diameter (mm): it fits the 10 mm face.
const TOOL_DIAMETER: f64 = 3.0;
/// The float noise of a Z read back from the toolpath (mm).
const EPS: f64 = 1e-6;

/// A flat square face at `z`, wound so that its triangles face `up`.
fn square_face(z: f64, up: bool) -> FaceTessellation {
    let v = vec![
        P3::new(0.0, 0.0, z),
        P3::new(PART_SIDE, 0.0, z),
        P3::new(PART_SIDE, PART_SIDE, z),
        P3::new(0.0, PART_SIDE, z),
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

/// The part: face 0 up at [`UP_FACE_Z`], face 1 down at [`DOWN_FACE_Z`].
fn part_model() -> LoadedModel {
    let enriched = build_enriched_mesh(
        vec![
            square_face(UP_FACE_Z, true),
            square_face(DOWN_FACE_Z, false),
        ],
        Vec::new(),
        Vec::new(),
    )
    .unwrap();
    LoadedModel {
        id: 0,
        name: "p3_4_part".to_owned(),
        mesh: Some(Arc::clone(&enriched.mesh)),
        items: Arc::new(brep_face_items(&enriched)),
        path: PathBuf::from("synthetic://p3_4_part.step"),
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

/// The stock: [`STOCK_MARGIN`] past the part on every side and in Z.
fn stock() -> StockConfig {
    StockConfig {
        x: PART_SIDE + 2.0 * STOCK_MARGIN,
        y: PART_SIDE + 2.0 * STOCK_MARGIN,
        z: UP_FACE_Z - DOWN_FACE_Z + 2.0 * STOCK_MARGIN,
        origin_x: -STOCK_MARGIN,
        origin_y: -STOCK_MARGIN,
        origin_z: DOWN_FACE_Z - STOCK_MARGIN,
        auto_from_model: false,
        ..StockConfig::default()
    }
}

/// The stock top in the world frame (= the Top setup frame).
fn world_stock_top() -> f64 {
    let s = stock();
    s.origin_z + s.z
}

/// One op on the part, on a setup with `face_up`.
fn session_with(op: OperationConfig, boundary: BoundaryConfig, face_up: FaceUp) -> ProjectSession {
    let mut builder = ProjectSessionBuilder::new().stock(stock());
    let tool_idx = builder.add_tool(endmill_tool_config(TOOL_DIAMETER));
    let tool_id = builder.tools()[tool_idx].id.0;
    let model_id = builder.add_model(part_model());
    let mut cfg = toolpath_config("p3_4 op", op, tool_id, model_id);
    cfg.boundary = boundary;
    cfg.heights.top_z = HeightMode::Auto;
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

/// A drop cutter whose boundary selection picks `faces` (the face picks of
/// an op with no selection slot, P4.1), on a Top setup.
fn boundary_session(faces: &[u16]) -> ProjectSession {
    let boundary = BoundaryConfig {
        enabled: true,
        source: BoundarySource::Selection {
            selection: Selection::empty(),
            holes: false,
        },
        containment: BoundaryContainment::Center,
        offset: 0.0,
    };
    let op = OperationConfig::DropCutter(DropCutterConfig {
        stepover: 1.0,
        ..DropCutterConfig::default()
    });
    let mut s = session_with(op, boundary, FaceUp::Top);
    pick_faces_in(&mut s, 0, SelectionTarget::Boundary, faces).unwrap();
    s
}

/// A Pocket that picks `faces`, on a setup with `face_up`.
fn pocket_session(faces: &[u16], face_up: FaceUp) -> ProjectSession {
    let op = OperationConfig::Pocket(PocketConfig {
        depth: POCKET_DEPTH,
        depth_per_pass: POCKET_DEPTH,
        ..PocketConfig::default()
    });
    let mut s = session_with(op, BoundaryConfig::default(), face_up);
    pick_faces_in(&mut s, 0, SelectionTarget::Op, faces).unwrap();
    s
}

/// The top Z that generation hands the op: the request snapshot of the
/// generation job (the field the face top writes).
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

/// The lowest Z of the cutting moves.
fn lowest_cut_z(session: &mut ProjectSession) -> f64 {
    let cancel = AtomicBool::new(false);
    let result = session
        .generate_toolpath(0, &cancel)
        .expect("the selection generates");
    let zs: Vec<f64> = result
        .toolpath()
        .moves
        .iter()
        .filter(|m| matches!(m.move_type, MoveType::Linear { .. }))
        .map(|m| m.target.z)
        .collect();
    assert!(!zs.is_empty(), "the op cut nothing");
    zs.into_iter().fold(f64::INFINITY, f64::min)
}

/// The status of `target` of toolpath 0: the facing counts and the refusal.
fn status(session: &ProjectSession, target: SelectionTarget) -> (FacesByFacing, Option<String>) {
    let tc = session.toolpath_configs()[0].clone();
    let report = session
        .selection_status(&tc, target)
        .expect("the status answers");
    (
        report.faces_by_facing,
        report.refusal.as_ref().map(ToString::to_string),
    )
}

/// Generation and the status of the op selection refuse with one value.
#[track_caller]
fn refusal_of(session: &mut ProjectSession) -> SelectionRefusal {
    let tc = session.toolpath_configs()[0].clone();
    let status = session
        .selection_status(&tc, SelectionTarget::Op)
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

#[test]
fn a_down_face_gives_no_auto_top_on_a_top_setup() {
    let mut both = boundary_session(&[UP_FACE, DOWN_FACE]);
    let top = op_top_z(&mut both);
    assert!(
        (top - UP_FACE_Z).abs() < EPS,
        "the auto top is {top}, want the up face Z {UP_FACE_Z}"
    );

    let mut down = boundary_session(&[DOWN_FACE]);
    let top = op_top_z(&mut down);
    assert!(
        (top - world_stock_top()).abs() < EPS,
        "the auto top is {top}, want the stock top {}: the down face at Z {DOWN_FACE_Z} \
         gives no top",
        world_stock_top()
    );
}

#[test]
fn a_boundary_keeps_a_down_face() {
    let mut s = boundary_session(&[DOWN_FACE]);
    let (facing, refusal) = status(&s, SelectionTarget::Boundary);
    assert_eq!(refusal, None, "a boundary takes the outline of a down face");
    assert_eq!(facing, FacesByFacing { up: 0, down: 1 });
    let cancel = AtomicBool::new(false);
    let _ = s
        .generate_toolpath(0, &cancel)
        .expect("a boundary on a down face generates");
}

#[test]
fn a_pocket_pick_of_the_down_face_refuses_on_a_top_setup() {
    // The up face alone cuts from its own Z: `face_top_z` is 0.
    let mut up = pocket_session(&[UP_FACE], FaceUp::Top);
    let low = lowest_cut_z(&mut up);
    let want = UP_FACE_Z - POCKET_DEPTH;
    assert!(
        (low - want).abs() < EPS,
        "the pocket floor is {low}, want {want} (the up face Z minus the depth)"
    );
    assert_eq!(
        status(&up, SelectionTarget::Op).0,
        FacesByFacing { up: 1, down: 0 }
    );

    for faces in [&[DOWN_FACE][..], &[UP_FACE, DOWN_FACE][..]] {
        let mut s = pocket_session(faces, FaceUp::Top);
        let r = refusal_of(&mut s);
        assert_eq!(r.cause, RefusalCause::FaceFacesDown, "{r}");
        let text = r.to_string();
        for phrase in [
            "Pocket: ",
            "face (centre X",
            "this face faces down in this setup",
            "Fix: Pick it in a setup that faces it, or pick the face above it.",
        ] {
            assert!(
                text.contains(phrase),
                "the refusal lacks {phrase:?}: {text}"
            );
        }
        let want_up = usize::from(faces.contains(&UP_FACE));
        assert_eq!(
            status(&s, SelectionTarget::Op).0,
            FacesByFacing {
                up: want_up,
                down: 1
            },
            "faces {faces:?}"
        );
    }
}

#[test]
fn the_same_face_is_up_and_cuts_on_a_bottom_setup() {
    let mut s = pocket_session(&[DOWN_FACE], FaceUp::Bottom);
    let (facing, refusal) = status(&s, SelectionTarget::Op);
    assert_eq!(refusal, None, "the flip turns the face up");
    assert_eq!(facing, FacesByFacing { up: 1, down: 0 });
    // The setup frame is zero-rooted at the stock bottom as the setup sees
    // it, so a world Z maps to `stock top - Z` (the P2.3 sentry).
    let face_top = world_stock_top() - DOWN_FACE_Z;
    let low = lowest_cut_z(&mut s);
    let want = face_top - POCKET_DEPTH;
    assert!(
        (low - want).abs() < EPS,
        "the pocket floor is {low}, want {want}: the face at model Z {DOWN_FACE_Z} is up at \
         setup Z {face_top} on a Bottom setup"
    );
}

/// The STEP door gives each flat face the normal that points out of the
/// solid, whatever the winding of the tessellation. `stepped_block.step`
/// and `l_bracket.step` tessellate with every triangle wound inward; before
/// the fix their top faces read -Z, and a pick of a top face refused
/// `FaceFacesDown` on a Top setup. Each of these parts stands on its lowest
/// face, and every other flat face is the top of a step, so the lowest face
/// points down and every other face points up.
#[cfg(feature = "step")]
#[test]
fn every_step_face_points_out_of_the_solid() {
    use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits};
    use rs_cam_core::selection::Shape;

    for name in [
        "stepped_block.step",
        "l_bracket.step",
        "block_40x40x60.step",
        "plate_100x60x10.step",
    ] {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/gui_step")
            .join(name);
        let model = rs_cam_core::io::load_model_file(
            &path,
            0,
            ModelKind::Step,
            ModelUnits::Millimeters,
            Default::default(),
        )
        .unwrap_or_else(|e| panic!("{name} loads: {e}"));
        let mut faces: Vec<(f64, f64)> = model
            .items
            .iter()
            .filter_map(|i| match i.shape() {
                Shape::Face(face) => Some((face.plane_z, face.normal.z)),
                _ => None,
            })
            .collect();
        faces.sort_by(|a, b| a.0.total_cmp(&b.0));
        assert!(faces.len() >= 2, "{name}: {faces:?}");
        let (bottom, tops) = faces.split_first().unwrap();
        assert!(bottom.1 < -0.99, "{name}: the bottom faces down: {faces:?}");
        assert!(
            tops.iter().all(|(_, nz)| *nz > 0.99),
            "{name}: every step top faces up: {faces:?}"
        );
    }
}
