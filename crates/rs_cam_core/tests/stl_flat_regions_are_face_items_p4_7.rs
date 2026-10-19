//! Geometry selection P4.7: the flat regions of an STL are `Face` items.
//!
//! # The claim
//!
//! 1. The stepped mesh of the basis fork (`common::meshes::stepped_top`)
//!    gives two faces: the Z 5 face with the boss as its one hole (area
//!    184 mm²) and the Z 8 boss face (16 mm²). The wall gives no face.
//! 2. Two loads of one STL file give equal keys, in the same order.
//! 3. A default selection does not build the flat regions: neither the
//!    `selection_status` read nor a generation moves the `geom_cache`
//!    build counter.
//! 4. Generation with a cold cache cuts the picked face. Before it, the
//!    status read (which only peeks) says "computing faces"
//!    (`faces_not_ready`) and gives no refusal; generation builds the faces
//!    itself, once, and cuts inside the face round the boss. After it, the
//!    status counts the face.
//! 5. The viewport rule (`face_pick_key_at`): a hit on a flat region names
//!    its face; a hit on the wall refuses `FaceNotFlat`; a cold cache
//!    refuses `FacesNotReady`.
//! 6. The `build_model_faces` job fills the cache off the session, and
//!    `list_model_items` lists the faces of an STL only for `kind: face`.
//!
//! The cache is one process-global table, so every test here takes
//! [`cache_lock`].
//!
//! Red proofs: claim 1 with the Z grouping removed (one union across Z);
//! claim 4 with generation reading the cache through `FaceAccess::Peek`.
//!
//! ```text
//! cargo test -p rs_cam_core -q --features test-support --test stl_flat_regions_are_face_items_p4_7
//! ```

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::{Mutex, MutexGuard, OnceLock};

use common::meshes::stepped_top;
use common::session::{mesh_model, single_op_session_with};
use common::tools::endmill_tool_config;
use rs_cam_core::compute::StockConfig;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::PocketConfig;
use rs_cam_core::geo::P3;
use rs_cam_core::ids::ModelId;
use rs_cam_core::maps::geom_cache;
use rs_cam_core::mesh::TriangleMesh;
use rs_cam_core::selection::{
    ItemKind, MatchKey, RefusalCause, SelectionEdit, Shape, SourceRef, flat_region_items,
};
use rs_cam_core::session::{
    BuildModelFacesArgs, Command, EditSelectionArgs, Job, JobHandle, LoadedModel, ProjectSession,
    SelectionTarget, execute_build_model_faces,
};
use rs_cam_core::toolpath::MoveType;

/// The tool diameter (mm): it fits the Z 5 face on each side of the boss.
const TOOL_DIAMETER: f64 = 3.0;
/// The pocket depth (mm).
const POCKET_DEPTH: f64 = 1.0;

fn cache_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The plane Z, the hole count and the area of each face item.
fn faces_of(set: &rs_cam_core::selection::ItemSet) -> Vec<(f64, usize, f64)> {
    set.iter()
        .map(|item| match item.shape() {
            Shape::Face(f) => (f.plane_z, f.rings.holes.len(), f.rings.area()),
            other => panic!("a flat region is a face, not {:?}", other.kind()),
        })
        .collect()
}

/// A Pocket over the stepped mesh as an STL model (no BREP faces), with
/// the default selection.
fn stl_pocket() -> ProjectSession {
    let stock = StockConfig {
        x: 20.0,
        y: 10.0,
        z: 8.0,
        origin_x: 0.0,
        origin_y: 0.0,
        origin_z: 0.0,
        auto_from_model: false,
        ..StockConfig::default()
    };
    single_op_session_with(
        stock,
        endmill_tool_config(TOOL_DIAMETER),
        mesh_model(stepped_top(), "p4_7_stepped"),
        "p4_7 pocket",
        OperationConfig::Pocket(PocketConfig {
            floor: rs_cam_core::compute::floor::Floor::top(POCKET_DEPTH),
            depth_per_pass: POCKET_DEPTH,
            ..PocketConfig::default()
        }),
        |_| {},
    )
}

/// The key of the Z 5 face, from the pure source (it touches no cache),
/// as a pick of the toolpath model stores it.
fn z5_face_key() -> MatchKey {
    let set = flat_region_items(&stepped_top()).unwrap();
    let item = set.iter().next().expect("the Z 5 face is first");
    MatchKey {
        source: SourceRef::ToolpathModel,
        ..item.key().clone()
    }
}

fn add_pick(session: &mut ProjectSession, key: MatchKey) {
    let _ = session
        .apply(Command::EditSelection(EditSelectionArgs {
            index: 0,
            target: SelectionTarget::Op,
            edit: SelectionEdit::AddPick(key),
        }))
        .expect("the pick edit applies");
}

#[test]
fn the_stepped_mesh_gives_two_faces() {
    let _guard = cache_lock();
    let set = flat_region_items(&stepped_top()).unwrap();
    let faces = faces_of(&set);
    assert_eq!(faces.len(), 2, "two flat regions, no wall face: {faces:?}");
    let (z, holes, area) = faces[0];
    assert!((z - 5.0).abs() < 1e-9, "the first face is at Z 5: {z}");
    assert_eq!(holes, 1, "the boss is a hole of the Z 5 face");
    assert!((area - 184.0).abs() < 1e-6, "the Z 5 face area: {area}");
    let (z, holes, area) = faces[1];
    assert!((z - 8.0).abs() < 1e-9, "the second face is at Z 8: {z}");
    assert_eq!(holes, 0);
    assert!((area - 16.0).abs() < 1e-6, "the boss face area: {area}");
    let key = set.iter().next().unwrap().key();
    assert_eq!(key.kind, ItemKind::Face);
    assert_eq!(key.brep, None, "a mesh face has no BREP id");
    assert!((key.measure - 184.0_f64.sqrt()).abs() < 1e-9);
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

#[test]
fn two_loads_of_one_stl_give_equal_keys() {
    let _guard = cache_lock();
    let dir = std::env::temp_dir().join(format!("rs_cam_p4_7_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("stepped.stl");
    write_stl(&stepped_top(), &path);
    let load = || LoadedModel::from_file(0, "stepped", &path, None, None, &dir).unwrap();
    let (a, b) = (load(), load());
    let _ = std::fs::remove_dir_all(&dir);
    assert!(a.faces_are_flat_regions() && b.faces_are_flat_regions());
    let keys = |m: &LoadedModel| -> Vec<MatchKey> {
        m.face_items_built()
            .iter()
            .map(|i| i.key().clone())
            .collect()
    };
    let (ka, kb) = (keys(&a), keys(&b));
    assert_eq!(ka.len(), 2, "the STL door gives the two faces");
    assert_eq!(ka, kb, "two loads of one STL give different keys");
    assert!(
        ka.iter().all(|k| k.source == SourceRef::Model(ModelId(0))),
        "the faces are bound to the model"
    );
}

#[test]
fn a_default_selection_does_not_build_the_flat_regions() {
    let _guard = cache_lock();
    let mut session = stl_pocket();
    geom_cache::reset_stats();
    let tc = session.toolpath_configs()[0].clone();
    let report = session.selection_status(&tc, SelectionTarget::Op).unwrap();
    assert!(report.faces_not_ready.is_empty());
    // The default selection of a Pocket names the drawing items; an STL
    // has none, so generation refuses. It must not build the faces first.
    let cancel = AtomicBool::new(false);
    assert!(session.generate_toolpath(0, &cancel).is_err());
    let stats = geom_cache::stats();
    assert_eq!(
        stats.flat_region_builds, 0,
        "a default selection built the flat regions: {stats:?}"
    );
}

#[test]
fn generation_with_a_cold_cache_cuts_the_face() {
    let _guard = cache_lock();
    let mut session = stl_pocket();
    add_pick(&mut session, z5_face_key());
    geom_cache::reset_stats();

    let tc = session.toolpath_configs()[0].clone();
    let report = session.selection_status(&tc, SelectionTarget::Op).unwrap();
    assert_eq!(
        report.faces_not_ready,
        vec![ModelId(0)],
        "the status read on a cold cache says computing faces"
    );
    assert_eq!(report.refusal, None, "computing faces is not a refusal");
    assert!(report.lost.is_empty(), "a pick on cold faces is not lost");
    assert_eq!(
        geom_cache::stats().flat_region_builds,
        0,
        "the status built"
    );

    let cancel = AtomicBool::new(false);
    let result = session
        .generate_toolpath(0, &cancel)
        .expect("generation builds the faces and cuts");
    // The moves below the face plane are the cuts in the material: the
    // auto top follows the face (Z 5), so the floor is Z 5 - depth.
    let cuts: Vec<P3> = result
        .toolpath()
        .moves
        .iter()
        .filter(|m| !matches!(m.move_type, MoveType::Rapid))
        .map(|m| m.target)
        .filter(|p| p.z < 5.0 - 1e-6)
        .collect();
    assert!(!cuts.is_empty(), "the face pick cut nothing");
    let floor = cuts.iter().map(|p| p.z).fold(f64::INFINITY, f64::min);
    assert!(
        (floor - (5.0 - POCKET_DEPTH)).abs() < 1e-6,
        "the cut floor is not the face Z less the depth: {floor}"
    );
    let r = TOOL_DIAMETER / 2.0;
    for p in &cuts {
        assert!(
            (0.0..=20.0).contains(&p.x) && (0.0..=10.0).contains(&p.y),
            "a cut leaves the Z 5 face: {p:?}"
        );
        let in_boss = p.x > 8.0 - r + 1e-6
            && p.x < 12.0 + r - 1e-6
            && p.y > 3.0 - r + 1e-6
            && p.y < 7.0 + r - 1e-6;
        assert!(!in_boss, "a cut enters the boss: {p:?}");
    }
    assert_eq!(
        geom_cache::stats().flat_region_builds,
        1,
        "generation builds the flat regions once"
    );

    let tc = session.toolpath_configs()[0].clone();
    let report = session.selection_status(&tc, SelectionTarget::Op).unwrap();
    assert!(report.faces_not_ready.is_empty(), "the faces landed");
    assert_eq!(report.counts.get(&ItemKind::Face), Some(&1));
    assert_eq!(report.refusal, None);
}

#[test]
fn a_hit_names_the_flat_region_under_it() {
    let _guard = cache_lock();
    let mut session = stl_pocket();
    let tc = session.toolpath_configs()[0].clone();
    let cold = session
        .face_pick_key_at(&tc, 0, P3::new(2.0, 2.0, 5.0))
        .unwrap_err();
    assert_eq!(
        cold.cause,
        RefusalCause::FacesNotReady { model: ModelId(0) }
    );

    let JobHandle::BuildModelFaces(handle) = session
        .start(
            Job::BuildModelFaces(BuildModelFacesArgs { model_id: 0 }),
            &AtomicBool::new(false),
        )
        .unwrap()
    else {
        panic!("the build_model_faces row answers its own handle");
    };
    let built = execute_build_model_faces(&handle, &AtomicBool::new(false)).unwrap();
    assert_eq!(built.faces, 2);

    let floor = session
        .face_pick_key_at(&tc, 0, P3::new(2.0, 2.0, 5.0))
        .unwrap();
    assert_eq!(floor, z5_face_key(), "the hit names the Z 5 face");
    let boss = session
        .face_pick_key_at(&tc, 0, P3::new(10.0, 5.0, 8.0))
        .unwrap();
    assert!((boss.measure - 4.0).abs() < 1e-9, "the hit names the boss");
    let wall = session
        .face_pick_key_at(&tc, 0, P3::new(10.0, 3.0, 6.5))
        .unwrap_err();
    assert_eq!(wall.cause, RefusalCause::FaceNotFlat { face: None });
}

#[test]
fn list_model_items_lists_stl_faces_for_kind_face_only() {
    let _guard = cache_lock();
    let session = stl_pocket();
    assert!(
        session.list_model_items(0, None, None).unwrap().is_empty(),
        "an STL lists no item without kind face"
    );
    let faces = session
        .list_model_items(0, None, Some(ItemKind::Face))
        .unwrap();
    assert_eq!(faces.len(), 2);
    assert!(
        faces
            .iter()
            .all(|f| f.key.source == SourceRef::Model(ModelId(0)))
    );
}
