//! SEL-2: when a ray hits two or more triangles at a bit-equal `t`, the
//! BVH pick and the brute-force pick both return the lowest index.
//!
//! The claims (design C §5):
//!
//! 1. Two coplanar triangles share an edge. A ray straight down on the edge
//!    gives one `t` for both, and both picks return index 0. With the
//!    triangle order swapped, both picks return index 0 again (now the
//!    other triangle).
//! 2. A fan of six triangles shares one vertex. A ray on the vertex gives
//!    one `t` for all six, and both picks return index 0.
//! 3. A ray `0.5 · RAY_BARY_EPS` outside a triangle in `u` is a hit for
//!    the triangle test (its tolerance is `RAY_BARY_EPS`). Brute force hits
//!    it, so the BVH must hit it: the box pad holds the point.
//!
//! The coordinates are small integers, so each `det` and each `t` is exact
//! and the ties are bit-equal; each test asserts the tie before it uses it.
//!
//! Red proof (run once, red, reverted): `pad` set to 0 makes claim 3 fail.

#![expect(
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_core::geo::{P3, RAY_BARY_EPS, V3};
use rs_cam_core::mesh::bvh::{DEFAULT_LEAF_SIZE, MeshBvh};
use rs_cam_core::mesh::{TriangleMesh, ray_pick_triangle};

const DOWN: V3 = V3::new(0.0, 0.0, -1.0);

/// Every BVH of `mesh` that the tests compare: one triangle per leaf, and
/// the default leaf size.
fn bvhs(mesh: &TriangleMesh) -> Vec<MeshBvh> {
    [1, DEFAULT_LEAF_SIZE]
        .iter()
        .filter_map(|&leaf| MeshBvh::build_with_leaf(mesh, leaf).ok())
        .collect()
}

/// The pick of brute force and of every BVH; each must equal `expected`.
fn assert_picks(mesh: &TriangleMesh, o: &P3, d: &V3, expected: usize) {
    let brute = ray_pick_triangle(mesh, o, d);
    assert_eq!(brute.map(|h| h.0), Some(expected), "brute force on {o:?}");
    let built = bvhs(mesh);
    assert_eq!(built.len(), 2, "both builds succeed");
    for bvh in &built {
        let fast = bvh.cast_ray(mesh, o, d);
        assert_eq!(
            fast.map(|(i, t)| (i, t.to_bits())),
            brute.map(|(i, t)| (i, t.to_bits())),
            "BVH (nodes {}) against brute force on {o:?}",
            bvh.node_count()
        );
    }
}

/// The `t` bits of each face that the ray hits, in face order.
fn hit_bits(mesh: &TriangleMesh, o: &P3, d: &V3) -> Vec<Option<u64>> {
    mesh.faces
        .iter()
        .map(|f| f.ray_intersect(o, d).map(f64::to_bits))
        .collect()
}

#[test]
fn a_ray_on_a_shared_edge_takes_the_lower_index_sel2() {
    // The square [0, 2]² at z = 0, cut on the diagonal (0,0)-(2,2).
    let vertices = vec![
        P3::new(0.0, 0.0, 0.0),
        P3::new(2.0, 0.0, 0.0),
        P3::new(2.0, 2.0, 0.0),
        P3::new(0.0, 2.0, 0.0),
    ];
    let o = P3::new(1.0, 1.0, 8.0);
    for triangles in [vec![[0, 1, 2], [0, 2, 3]], vec![[0, 2, 3], [0, 1, 2]]] {
        let mesh = TriangleMesh::from_raw(vertices.clone(), triangles);
        let hits = hit_bits(&mesh, &o, &DOWN);
        assert_eq!(hits.len(), 2);
        assert!(hits[0].is_some(), "the ray hits both triangles");
        assert_eq!(hits[0], hits[1], "the two t values are bit-equal");
        assert_picks(&mesh, &o, &DOWN, 0);
    }
}

#[test]
fn a_ray_on_a_shared_vertex_takes_the_lower_index_sel2() {
    // Six triangles around the origin. Each consecutive pair of rim
    // points has the cross product 4, so each `det` is 4.
    let rim = [
        (2.0, 0.0),
        (1.0, 2.0),
        (-1.0, 2.0),
        (-2.0, 0.0),
        (-1.0, -2.0),
        (1.0, -2.0),
    ];
    let mut vertices = vec![P3::new(0.0, 0.0, 0.0)];
    vertices.extend(rim.iter().map(|&(x, y)| P3::new(x, y, 0.0)));
    let triangles: Vec<[u32; 3]> = (0..6).map(|j| [0, 1 + j, 1 + (j + 1) % 6]).collect();
    let mesh = TriangleMesh::from_raw(vertices, triangles);
    let o = P3::new(0.0, 0.0, 8.0);
    let hits = hit_bits(&mesh, &o, &DOWN);
    assert_eq!(hits.len(), 6);
    assert!(hits[0].is_some(), "the ray hits the fan");
    assert!(
        hits.iter().all(|h| *h == hits[0]),
        "all six t values are bit-equal: {hits:?}"
    );
    assert_picks(&mesh, &o, &DOWN, 0);
}

#[test]
fn a_ray_just_outside_in_u_still_hits_sel2() {
    // Triangle 0: v0 = (0,0), e1 = (1,0), e2 = (0,1), so u = x and v = y.
    // Triangle 1 sits away from the ray; it widens the mesh box to x < 0,
    // so the box gate passes the ray and only a leaf box can reject it.
    let vertices = vec![
        P3::new(0.0, 0.0, 0.0),
        P3::new(1.0, 0.0, 0.0),
        P3::new(0.0, 1.0, 0.0),
        P3::new(-2.0, 5.0, 0.0),
        P3::new(-1.0, 5.0, 0.0),
        P3::new(-2.0, 6.0, 0.0),
    ];
    let mesh = TriangleMesh::from_raw(vertices, vec![[0, 1, 2], [3, 4, 5]]);
    let o = P3::new(-0.5 * RAY_BARY_EPS, 0.25, 4.0);
    assert!(
        o.x < mesh.faces[0].bbox.min.x,
        "the ray is outside the triangle box"
    );
    assert!(
        mesh.bbox.ray_intersect(&o, &DOWN).is_some(),
        "the gate passes the ray"
    );
    assert_picks(&mesh, &o, &DOWN, 0);

    // A tilted ray to the same point: no zero direction component.
    let d = V3::new(0.0, 0.5, -1.0);
    let o2 = P3::new(o.x, 0.25, 0.0) - d * 4.0;
    assert_picks(&mesh, &o2, &d, 0);
}
