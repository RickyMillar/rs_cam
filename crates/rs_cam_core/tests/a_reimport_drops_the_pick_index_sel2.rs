//! SEL-2: the pick index of a model is keyed by the identity of its mesh,
//! so a re-import drops it with no explicit reset.
//!
//! # The claim (design C §3, §5 row 4, first half)
//!
//! 1. An STL model loads from a file. `cached_bvh` on its `pick_mesh`
//!    builds once; then `pick_index_ready` is true, and `ray_pick` equals
//!    `MeshBvh::cast_ray` and `ray_pick_triangle` on every ray.
//! 2. A second `cached_bvh` on the same mesh is a hit, and gives the same
//!    `Arc`.
//! 3. `AdoptModelGeometry` with inch units gives the model a NEW mesh `Arc`,
//!    25.4 times larger. Then `pick_index_ready` is false, and `ray_pick`
//!    still answers by brute force: it equals `ray_pick_triangle` on the
//!    scaled mesh. The old mesh stays alive in this test, so its entry
//!    stays in the table; only the identity check keeps it out.
//!
//! The second half of row 4 (the `build_model_pick_index` job fills the
//! cache and counts one build) lands with the Job row in commit C3.
//!
//! The pick table is one process-global table, so each test here takes
//! [`cache_lock`] and reads counter deltas.
//!
//! Red proof (run once, red, reverted): `peek_bvh` with no identity check
//! (it gave the first entry) made claim 3 fail.
//!
//! ```text
//! cargo test -p rs_cam_core -q --features test-support --test a_reimport_drops_the_pick_index_sel2
//! ```

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use rs_cam_core::compute::stock_config::ModelUnits;
use rs_cam_core::geo::{P3, V3};
use rs_cam_core::ids::ModelId;
use rs_cam_core::maps::geom_cache;
use rs_cam_core::mesh::{TriangleMesh, ray_pick_triangle};
use rs_cam_core::session::{
    AddModelArgs, AdoptModelGeometryArgs, Command, LoadedModel, ProjectSession,
};

fn cache_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// A small height field: `n × n` cells, two triangles each, heights from a
/// fixed formula.
fn bumps(n: usize) -> TriangleMesh {
    let mut vertices = Vec::new();
    for j in 0..=n {
        for i in 0..=n {
            let (x, y) = (i as f64, j as f64);
            vertices.push(P3::new(x, y, (0.6 * x).sin() + (0.4 * y).cos()));
        }
    }
    let w = (n + 1) as u32;
    let mut triangles = Vec::new();
    for j in 0..n as u32 {
        for i in 0..n as u32 {
            let a = j * w + i;
            triangles.push([a, a + 1, a + w]);
            triangles.push([a + 1, a + w + 1, a + w]);
        }
    }
    TriangleMesh::from_raw(vertices, triangles)
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
    let mut file = std::fs::File::create(path).expect("the STL file opens");
    stl_io::write_stl(&mut file, triangles.iter()).expect("the STL writes");
}

/// Rays over the mesh box: straight down on a grid, and from a tilted eye.
fn rays(mesh: &TriangleMesh) -> Vec<(P3, V3)> {
    let b = &mesh.bbox;
    let span = (b.max.x - b.min.x).max(b.max.y - b.min.y);
    let at = |fx: f64, fy: f64, z: f64| {
        P3::new(
            b.min.x + fx * (b.max.x - b.min.x),
            b.min.y + fy * (b.max.y - b.min.y),
            z,
        )
    };
    let eye = at(1.3, -0.4, b.max.z + span);
    let mut out = Vec::new();
    for i in 0..7 {
        for j in 0..7 {
            let (fx, fy) = ((i as f64 + 0.5) / 7.0, (j as f64 + 0.5) / 7.0);
            out.push((at(fx, fy, b.max.z + 1.0), V3::new(0.0, 0.0, -1.0)));
            let target = at(fx * 1.2 - 0.1, fy * 1.2 - 0.1, b.min.z);
            out.push((eye, (target - eye).normalize()));
        }
    }
    out
}

/// The model of `session`: the one model it holds.
fn model(session: &ProjectSession) -> &LoadedModel {
    let models = session.models();
    assert_eq!(models.len(), 1);
    &models[0]
}

#[test]
fn a_reimport_drops_the_pick_index_sel2() {
    let _guard = cache_lock();
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("a_reimport_drops_the_pick_index_sel2");
    std::fs::create_dir_all(&dir).expect("the temp folder");
    let path = dir.join("bumps.stl");
    write_stl(&bumps(12), &path);
    let load = |units: Option<ModelUnits>| {
        LoadedModel::from_file(ModelId::from_raw(0), "bumps", &path, None, units, &dir)
            .expect("the STL loads")
    };

    let mut session = ProjectSession::new_empty();
    let _ = session
        .apply(Command::AddModel(AddModelArgs {
            model: Box::new(load(None)),
        }))
        .expect("the model adds");
    let model_id = model(&session).id;
    let old_mesh = Arc::clone(model(&session).pick_mesh().expect("an STL has a mesh"));
    assert!(
        model(&session).faces_are_flat_regions(),
        "the model is an STL"
    );

    // Claim 1: one build; the pick runs on the BVH and keeps the answer.
    let (builds0, hits0) = geom_cache::bvh_stats();
    let bvh = geom_cache::cached_bvh(&old_mesh).expect("the BVH builds");
    assert_eq!(geom_cache::bvh_stats(), (builds0 + 1, hits0), "one build");
    assert!(model(&session).pick_index_ready());
    let mut hits = 0;
    for (o, d) in rays(&old_mesh) {
        let pick = model(&session).ray_pick(&o, &d);
        assert_eq!(pick, bvh.cast_ray(&old_mesh, &o, &d));
        assert_eq!(pick, ray_pick_triangle(&old_mesh, &o, &d));
        hits += usize::from(pick.is_some());
    }
    assert!(hits > 0, "no ray hit the model: the check is vacuous");

    // Claim 2: a second call on the same mesh is a hit, the same `Arc`.
    let again = geom_cache::cached_bvh(&old_mesh).expect("the BVH is cached");
    assert!(Arc::ptr_eq(&bvh, &again));
    assert_eq!(geom_cache::bvh_stats(), (builds0 + 1, hits0 + 1), "one hit");

    // Claim 3: a rescale gives a new mesh; the index drops, the pick stays.
    let _ = session
        .apply(Command::AdoptModelGeometry(AdoptModelGeometryArgs {
            model_id,
            geometry: Box::new(load(Some(ModelUnits::Inches))),
            units: Some(ModelUnits::Inches),
        }))
        .expect("the rescale adopts");
    let new_mesh = Arc::clone(model(&session).pick_mesh().expect("the mesh stays"));
    assert!(
        !Arc::ptr_eq(&old_mesh, &new_mesh),
        "a re-import gives a new Arc"
    );
    let scale =
        (new_mesh.bbox.max.x - new_mesh.bbox.min.x) / (old_mesh.bbox.max.x - old_mesh.bbox.min.x);
    assert!((scale - 25.4).abs() < 1e-6, "the inch rescale: {scale}");
    assert!(geom_cache::bvh_cache_len() >= 1, "the old entry is alive");
    assert!(
        !model(&session).pick_index_ready(),
        "the new mesh has no pick index"
    );
    let mut hits = 0;
    for (o, d) in rays(&new_mesh) {
        let pick = model(&session).ray_pick(&o, &d);
        assert_eq!(pick, ray_pick_triangle(&new_mesh, &o, &d));
        hits += usize::from(pick.is_some());
    }
    assert!(
        hits > 0,
        "no ray hit the scaled model: the check is vacuous"
    );
    assert_eq!(
        geom_cache::bvh_stats(),
        (builds0 + 1, hits0 + 1),
        "a pick never builds"
    );
    drop(old_mesh);
    let _ = std::fs::remove_dir_all(&dir);
}
