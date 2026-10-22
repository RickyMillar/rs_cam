//! Geometry selection P3.4 follow-up: a STEP shell faces out at import.
//!
//! # The claim
//!
//! The STEP door (`io::step_input::load_step`) gives outward triangles for
//! every closed shell. Two fixtures (`fixtures/gui_step/stepped_block.step`
//! and `l_bracket.step`) hold a closed shell whose triangles all face
//! inward. Before the fix, the door kept that winding: the signed volume
//! was negative, and every reader of the mesh (the 3D ops, the flat faces,
//! the render normals) saw the top faces as down. P3.4 only turned the
//! normals of the face items round (`io::outward_sign`, now deleted).
//!
//! For each STEP fixture in the repository:
//!
//! 1. The signed volume of each shell is positive. A shell here is one
//!    connected set of BREP faces (`EnrichedMesh::adjacency`); each fixture
//!    holds one solid.
//! 2. Each flat face points out of the solid. Each part stands on its
//!    lowest flat face and every other flat face is the top of a step, so
//!    the lowest flat face points down and each other flat face points up.
//!    Every triangle of an up face has a +Z normal, and every triangle of
//!    the down face has a -Z normal.
//! 3. Two readers agree on the sign of each flat face: the face items of
//!    the door (`io::brep_face_items`) and the flat regions of the mesh
//!    (`selection::flat_region_items`), which reads only the triangles.
//!
//! ```text
//! cargo test -p rs_cam_core -q --features step --test a_step_shell_faces_out_at_import_p3_4
//! ```

#![cfg(feature = "step")]
#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::path::PathBuf;

use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits};
use rs_cam_core::geometry::enriched_mesh::EnrichedMesh;
use rs_cam_core::selection::{Shape, flat_region_items};

/// Every STEP fixture in the repository, relative to this crate.
const FIXTURES: &[&str] = &[
    "../../fixtures/gui_step/stepped_block.step",
    "../../fixtures/gui_step/l_bracket.step",
    "../../fixtures/gui_step/block_40x40x60.step",
    "../../fixtures/gui_step/plate_100x60x10.step",
    "tests/fixtures/step/occt-cube.step",
    "tests/fixtures/step/occt-cylinder.step",
    "tests/fixtures/step/occt-cone.step",
    "tests/fixtures/step/occt-sphere.step",
];

/// The two fixtures that the source file triangulates inward.
const INWARD_FIXTURES: &[&str] = &["stepped_block.step", "l_bracket.step"];

/// The smallest `|normal.z|` of a flat-face triangle.
const FLAT_NZ: f64 = 0.99;

/// The Z tolerance (mm) to match a face item to a flat region.
const Z_EPS: f64 = 1e-6;

fn path_of(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel)
}

/// The root of `i` in the union-find array `parent`.
fn root(parent: &mut [usize], mut i: usize) -> usize {
    while parent[i] != i {
        parent[i] = parent[parent[i]];
        i = parent[i];
    }
    i
}

/// The face groups of each shell: the connected sets of
/// `enriched.adjacency`.
fn shells(enriched: &EnrichedMesh) -> Vec<Vec<usize>> {
    let n = enriched.face_groups.len();
    let mut parent: Vec<usize> = (0..n).collect();
    for &(a, b) in &enriched.adjacency {
        let ra = root(&mut parent, usize::from(a.0));
        let rb = root(&mut parent, usize::from(b.0));
        parent[ra] = rb;
    }
    let mut groups: std::collections::BTreeMap<usize, Vec<usize>> = Default::default();
    for i in 0..n {
        let r = root(&mut parent, i);
        groups.entry(r).or_default().push(i);
    }
    groups.into_values().collect()
}

/// The signed volume (mm^3) of the triangles of `faces`, about the centre
/// of the mesh bounding box.
fn signed_volume(enriched: &EnrichedMesh, faces: &[usize]) -> f64 {
    let mesh = &enriched.mesh;
    let c = mesh.bbox.center();
    let mut volume = 0.0;
    for &f in faces {
        for tri in &mesh.triangles[enriched.face_groups[f].triangle_range.clone()] {
            let a = mesh.vertices[tri[0] as usize] - c;
            let b = mesh.vertices[tri[1] as usize] - c;
            let d = mesh.vertices[tri[2] as usize] - c;
            volume += a.dot(&b.cross(&d)) / 6.0;
        }
    }
    volume
}

#[test]
fn a_step_shell_faces_out_at_import() {
    let mut inward_seen = 0;
    let mut fixtures_with_an_up_face = 0;
    for rel in FIXTURES {
        let path = path_of(rel);
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let model = rs_cam_core::io::load_model_file(
            &path,
            rs_cam_core::ids::ModelId::from_raw(0),
            ModelKind::Step,
            ModelUnits::Millimeters,
            Default::default(),
        )
        .unwrap_or_else(|e| panic!("{name} loads: {e}"));
        let enriched = model
            .enriched_mesh
            .clone()
            .expect("a STEP model is enriched");
        let mesh = &enriched.mesh;

        // 1. Each shell faces out.
        let shells = shells(&enriched);
        assert!(!shells.is_empty(), "{name}: no shell");
        for shell in &shells {
            let volume = signed_volume(&enriched, shell);
            assert!(
                volume > 0.0,
                "{name}: the shell of faces {shell:?} has the signed volume {volume:.1} mm^3; \
                 an outward shell has a positive volume"
            );
        }
        if INWARD_FIXTURES.contains(&name.as_str()) {
            inward_seen += 1;
        }

        // 2. The lowest flat face points down; each other flat face points up.
        let mut flat: Vec<(f64, usize)> = enriched
            .face_groups
            .iter()
            .enumerate()
            .filter(|(_, g)| g.boundary_loops_2d.is_some())
            .map(|(i, g)| (g.bbox.max.z, i))
            .collect();
        flat.sort_by(|a, b| a.0.total_cmp(&b.0));
        if let Some((bottom, tops)) = flat.split_first() {
            let sign_of = |group: usize| -> Vec<f64> {
                mesh.faces[enriched.face_groups[group].triangle_range.clone()]
                    .iter()
                    .map(|t| t.normal.z)
                    .collect()
            };
            let down = sign_of(bottom.1);
            assert!(
                down.iter().all(|nz| *nz < -FLAT_NZ),
                "{name}: the bottom face at Z {} has a triangle that does not point down: {down:?}",
                bottom.0
            );
            for &(z, group) in tops {
                let up = sign_of(group);
                assert!(
                    up.iter().all(|nz| *nz > FLAT_NZ),
                    "{name}: the up face at Z {z} has a triangle that does not point up: {up:?}"
                );
            }
            if !tops.is_empty() {
                fixtures_with_an_up_face += 1;
            }
        }

        // 3. The face items and the flat regions agree on each sign.
        let items: Vec<(f64, f64)> = model
            .items
            .iter()
            .filter_map(|i| match i.shape() {
                Shape::Face(f) => Some((f.plane_z, f.normal.z)),
                _ => None,
            })
            .collect();
        let regions: Vec<(f64, f64)> = flat_region_items(mesh)
            .unwrap()
            .iter()
            .filter_map(|i| match i.shape() {
                Shape::Face(f) => Some((f.plane_z, f.normal.z)),
                _ => None,
            })
            .collect();
        assert_eq!(items.len(), flat.len(), "{name}: one item per flat face");
        for (z, nz) in &items {
            let matching: Vec<f64> = regions
                .iter()
                .filter(|(rz, _)| (rz - z).abs() < Z_EPS)
                .map(|(_, rnz)| *rnz)
                .collect();
            assert!(!matching.is_empty(), "{name}: no flat region at Z {z}");
            assert!(
                matching.iter().all(|rnz| rnz.signum() == nz.signum()),
                "{name}: at Z {z} the face item has normal Z {nz} and the flat regions \
                 have {matching:?}"
            );
        }
    }
    // Not vacuous: both inward fixtures were read, and seven fixtures (all
    // but the sphere) hold an up face.
    assert_eq!(inward_seen, INWARD_FIXTURES.len());
    assert_eq!(fixtures_with_an_up_face, FIXTURES.len() - 1);
}
