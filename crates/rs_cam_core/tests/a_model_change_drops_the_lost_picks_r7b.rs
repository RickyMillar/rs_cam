//! Operator ruling R7b (2026-10-30): a model change drops the picks that
//! name no item on the new model.
//!
//! # The live case
//!
//! `set_toolpath_model` changed `model_id` and kept the picks of the
//! boundary selection, so a STEP face boundary refused on the mesh with a
//! lost pick (`planning/viewport_parity_2026-10-28/FOLLOW_UPS.md` §12
//! item 3).
//!
//! # The claims
//!
//! (a) A drop cutter on `stepped_block.step` with a boundary of one STEP
//!     face (Center, offset 0). `SetToolpathModel` to an STL of the same
//!     part (its flat faces computed, so the resolution can judge each
//!     pick) drops the face pick: a STL face has no BREP id, so the STEP
//!     key names no item. The boundary lost every pick, so it is back to
//!     the boundary a fresh op gets (`draft_boundary`: the model
//!     silhouette grown by the tool diameter), the containment kept.
//! (b) `Effects::picks_dropped` counts the pick and names the reset;
//!     `Effects::picks_dropped_note` gives the one text.
//! (c) The op then generates with no refusal.
//! (d) A change back to the model that the picks name drops nothing.
//!
//! Red proof: with `drop_lost_picks` returning `None` at once, (a) keeps
//! the pick and (c) refuses with the lost pick.
//!
//! ```text
//! cargo test -p rs_cam_core -q --features step,test-support --test a_model_change_drops_the_lost_picks_r7b
//! ```

#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

mod common;

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::StockConfig;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::{
    BoundaryConfig, BoundaryContainment, BoundarySource, HeightMode,
};
use rs_cam_core::compute::operation_configs::DropCutterConfig;
use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits};
use rs_cam_core::ids::ModelId;
use rs_cam_core::mesh::TriangleMesh;
use rs_cam_core::selection::{Selection, Shape};
use rs_cam_core::session::{
    BuildModelFacesArgs, Command, DroppedPicks, Job, JobHandle, LoadedModel, ProjectSession,
    ProjectSessionBuilder, SelectionTarget, SetToolpathModelArgs, execute_build_model_faces,
};

const STEPPED: &str = "tests/fixtures/step/stepped_block.step";
const TOOL_DIAMETER: f64 = 3.0;

fn load(path: &Path, id: usize, kind: ModelKind) -> LoadedModel {
    rs_cam_core::io::load_model_file(
        path,
        ModelId::from_raw(id),
        kind,
        ModelUnits::Millimeters,
        Default::default(),
    )
    .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// Write `mesh` as a binary STL.
fn write_stl(mesh: &TriangleMesh, path: &Path) {
    let vertex = |i: u32| {
        let p = mesh.vertices[i as usize];
        stl_io::Vector([p.x as f32, p.y as f32, p.z as f32])
    };
    let triangles: Vec<stl_io::Triangle> = mesh
        .triangles
        .iter()
        .map(|t| stl_io::Triangle {
            normal: stl_io::Vector([0.0, 0.0, 0.0]),
            vertices: [vertex(t[0]), vertex(t[1]), vertex(t[2])],
        })
        .collect();
    let mut file = std::fs::File::create(path).unwrap();
    stl_io::write_stl(&mut file, triangles.iter()).unwrap();
}

/// Model 0: the STEP part. Model 1: an STL of its mesh. One drop cutter
/// on model 0 with a boundary of the up face at Z 30.
fn session() -> ProjectSession {
    let step = load(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(STEPPED),
        0,
        ModelKind::Step,
    );
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    let stl_path = dir.join(format!("r7b_stepped_{}.stl", std::process::id()));
    write_stl(step.mesh.as_ref().unwrap(), &stl_path);
    let stl = load(&stl_path, 1, ModelKind::Stl);

    let mut builder = ProjectSessionBuilder::new().stock(StockConfig {
        x: 70.0,
        y: 50.0,
        z: Some(30.0),
        origin_x: -5.0,
        origin_y: -5.0,
        origin_z: 0.0,
        auto_from_model: false,
        ..StockConfig::default()
    });
    let tool = builder.add_tool(common::tools::endmill_tool_config(TOOL_DIAMETER));
    let tool_id = builder.tools()[tool].id;
    let step_id = builder.add_model(step);
    let _ = builder.add_model(stl);
    let op = OperationConfig::DropCutter(DropCutterConfig {
        stepover: 1.0,
        ..DropCutterConfig::default()
    });
    let mut cfg = common::session::toolpath_config("face finish", op, tool_id, step_id);
    cfg.boundary = BoundaryConfig {
        enabled: true,
        source: BoundarySource::Selection {
            selection: Selection::empty(),
            holes: false,
        },
        containment: BoundaryContainment::Center,
        offset: 0.0,
    };
    cfg.heights.top_z = HeightMode::Auto;
    let _ = builder.add_toolpath(0, cfg).unwrap();
    let mut s = builder.build();
    let face = s.models()[0]
        .items
        .iter()
        .find_map(|item| match item.shape() {
            Shape::Face(f) if f.normal.z > 0.5 && (f.plane_z - 30.0).abs() < 1e-6 => {
                f.brep.map(|id| id.0)
            }
            _ => None,
        })
        .expect("the up face at Z 30");
    common::session::pick_faces_in(&mut s, 0, SelectionTarget::Boundary, &[face]).unwrap();
    s
}

/// Compute the flat faces of the STL (model 1), as the GUI does off the
/// frame, so a `Peek` resolution can judge a face pick on it.
fn build_stl_faces(s: &mut ProjectSession) {
    let JobHandle::BuildModelFaces(handle) = s
        .start(
            Job::BuildModelFaces(BuildModelFacesArgs {
                model_id: ModelId::from_raw(1),
            }),
            &AtomicBool::new(false),
        )
        .unwrap()
    else {
        panic!("the build_model_faces row answers its own handle");
    };
    let _ = execute_build_model_faces(&handle, &AtomicBool::new(false)).unwrap();
}

fn set_model(s: &mut ProjectSession, id: usize) -> rs_cam_core::session::Effects {
    s.apply(Command::SetToolpathModel(SetToolpathModelArgs {
        index: 0,
        model_id: ModelId::from_raw(id),
    }))
    .unwrap()
}

#[test]
fn a_model_change_drops_the_lost_face_pick_r7b() {
    let mut s = session();
    // Non-vacuity: the face boundary generates on the STEP model.
    let _ = s
        .generate_toolpath(0, &AtomicBool::new(false))
        .expect("the face boundary generates on the STEP");
    build_stl_faces(&mut s);

    // (d) a change to the model already bound drops nothing.
    let same = set_model(&mut s, 0);
    assert_eq!(same.picks_dropped, None);

    // (a), (b)
    let effects = set_model(&mut s, 1);
    assert_eq!(
        effects.picks_dropped,
        Some(DroppedPicks {
            count: 1,
            reset: vec![SelectionTarget::Boundary],
        })
    );
    assert_eq!(
        effects.picks_dropped_note().as_deref(),
        Some(
            "The model change dropped 1 pick that names no item on the new model. \
             The boundary is back to its default."
        )
    );
    let tc = &s.toolpath_configs()[0];
    assert_eq!(tc.model_id, ModelId::from_raw(1));
    assert_eq!(
        tc.boundary,
        BoundaryConfig::for_3d_op(TOOL_DIAMETER),
        "the boundary a fresh 3D op gets on a mesh"
    );

    // (c)
    let _ = s
        .generate_toolpath(0, &AtomicBool::new(false))
        .expect("the op generates on the STL with no refusal");
}

/// (d) on a selection with a pick that the new model names: the face pick
/// of a STEP model stays when the change goes to another load of the same
/// STEP file (the BREP ids and geometry are equal).
#[test]
fn a_pick_that_names_an_item_stays_r7b() {
    let mut s = session();
    let step = load(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(STEPPED),
        2,
        ModelKind::Step,
    );
    let _ = s
        .apply(Command::AddModel(rs_cam_core::session::AddModelArgs {
            model: Box::new(step),
        }))
        .expect("the add");
    let id = s.models().last().unwrap().id.raw();
    let effects = set_model(&mut s, id);
    assert_eq!(effects.picks_dropped, None);
    let tc = &s.toolpath_configs()[0];
    let BoundarySource::Selection { selection, .. } = &tc.boundary.source else {
        panic!("the boundary stays a selection: {:?}", tc.boundary.source);
    };
    assert_eq!(selection.picks().len(), 1);
}
