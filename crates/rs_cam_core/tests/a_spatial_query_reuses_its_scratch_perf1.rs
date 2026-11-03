//! P1 (perf sweep 2026-11-03): `SpatialIndex::query` reuses a thread-local
//! dedup scratch instead of a whole-mesh bitset per call.
//!
//! The change must not change any query result. This file pins that:
//!
//! 1. `query`, `query_into` and `with_query` return the same vector, index
//!    for index, as `reference_query`. `reference_query` is a copy of the
//!    `query` body before P1, written over the public API. The points cover
//!    the inside of the grid, the exact max edge, just outside each side and
//!    far off the grid on both sides. The radii are 0, 0.1, the cell size
//!    and a radius larger than the mesh.
//! 2. `point_drop_cutter` gives bit-identical `CLPoint`s to the old loop
//!    over `reference_query`, and `point_is_over_mesh_xy` gives the same
//!    answer as the old predicate.
//! 3. A `with_query` call inside the closure of another `with_query` call
//!    gets a correct result and does not panic. The outer slice does not
//!    change.
//! 4. The thread-local scratch stays clean when one thread queries meshes
//!    of different size in turn.
//!
//! The grid origin is `mesh.bbox.min` by the definition in
//! `SpatialIndex::build`, so the test reads it from the mesh and needs no
//! new accessor.

#![expect(
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_core::geo::P3;
use rs_cam_core::mesh::{QueryScratch, SpatialIndex, TriangleMesh, make_test_hemisphere};
use rs_cam_core::surface::dropcutter::{point_drop_cutter, point_is_over_mesh_xy};
use rs_cam_core::tool::{
    BallEndmill, CLPoint, FlatEndmill, MillingCutter, drop_cutter_can_contact,
};

/// The body of `SpatialIndex::query` before P1, over the public API.
fn reference_query(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cx: f64,
    cy: f64,
    radius: f64,
) -> Vec<usize> {
    let origin_x = mesh.bbox.min.x;
    let origin_y = mesh.bbox.min.y;
    let cell_size = index.cell_size();
    let cell_count_x = index.cell_count_x();
    let cell_count_y = index.cell_count_y();

    let x0 = ((cx - radius - origin_x) / cell_size).floor() as isize;
    let x1 = ((cx + radius - origin_x) / cell_size).floor() as isize;
    let y0 = ((cy - radius - origin_y) / cell_size).floor() as isize;
    let y1 = ((cy + radius - origin_y) / cell_size).floor() as isize;

    let x0 = x0.max(0) as usize;
    let x1 = (x1 as usize).min(cell_count_x.saturating_sub(1));
    let y0 = y0.max(0) as usize;
    let y1 = (y1 as usize).min(cell_count_y.saturating_sub(1));

    let mut result = Vec::new();
    let n_words = index.total_triangles().div_ceil(64);
    let mut seen = vec![0u64; n_words];

    for cy_idx in y0..=y1 {
        for cx_idx in x0..=x1 {
            let cell_idx = cy_idx * cell_count_x + cx_idx;
            for &tri_idx in index.cell_slice(cell_idx) {
                let word = tri_idx / 64;
                let bit = 1u64 << (tri_idx % 64);
                if seen[word] & bit == 0 {
                    seen[word] |= bit;
                    result.push(tri_idx);
                }
            }
        }
    }

    result
}

/// The `point_drop_cutter` loop before P1, over `reference_query`.
fn reference_drop<C: MillingCutter + ?Sized>(
    x: f64,
    y: f64,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &C,
) -> CLPoint {
    let mut cl = CLPoint::new(x, y);
    let envelope_radius = cutter.radius();
    for idx in reference_query(mesh, index, x, y, envelope_radius) {
        let tri = &mesh.faces[idx];
        if !drop_cutter_can_contact(&cl, tri, envelope_radius) {
            continue;
        }
        cutter.drop_cutter(&mut cl, tri);
    }
    cl
}

/// The `point_is_over_mesh_xy` predicate before P1.
fn reference_over(x: f64, y: f64, mesh: &TriangleMesh, index: &SpatialIndex) -> bool {
    reference_query(mesh, index, x, y, 0.0)
        .iter()
        .any(|&idx| mesh.faces[idx].contains_point_xy(x, y))
}

/// A non-square strip, 40 mm by 9 mm, with a wavy surface. Its origin is
/// not at zero, so the origin arithmetic is tested too.
fn strip_mesh() -> TriangleMesh {
    let (nx, ny) = (41usize, 10usize);
    let (x0, y0) = (12.5, -3.25);
    let mut verts = Vec::with_capacity(nx * ny);
    for j in 0..ny {
        for i in 0..nx {
            let x = x0 + i as f64;
            let y = y0 + j as f64;
            let z = 2.0 + (0.7 * x).sin() * (0.9 * y).cos();
            verts.push(P3::new(x, y, z));
        }
    }
    let mut tris = Vec::new();
    for j in 0..ny - 1 {
        for i in 0..nx - 1 {
            let a = (j * nx + i) as u32;
            let b = a + 1;
            let c = a + nx as u32;
            let d = c + 1;
            tris.push([a, b, d]);
            tris.push([a, d, c]);
        }
    }
    TriangleMesh::from_raw(verts, tris)
}

/// The meshes and indexes under test: two cell sizes on each mesh.
fn cases() -> Vec<(&'static str, TriangleMesh, f64)> {
    vec![
        ("hemisphere/auto", make_test_hemisphere(20.0, 24), -1.0),
        ("hemisphere/3.0", make_test_hemisphere(20.0, 24), 3.0),
        ("strip/auto", strip_mesh(), -1.0),
        ("strip/1.3", strip_mesh(), 1.3),
    ]
}

fn build(mesh: &TriangleMesh, cell: f64) -> SpatialIndex {
    if cell > 0.0 {
        SpatialIndex::build(mesh, cell)
    } else {
        SpatialIndex::build_auto(mesh)
    }
}

/// One axis of the probe set: a dense lattice across the grid and past it,
/// the exact min and max edges and the grid max edge, values just outside
/// each side, and far values on both sides.
fn axis_values(min: f64, max: f64, cell: f64, cells: usize) -> Vec<f64> {
    let grid_max = min + cell * cells as f64;
    let mut v = Vec::new();
    let lo = min - 2.5 * cell;
    let hi = grid_max + 2.5 * cell;
    let steps = 60usize;
    for k in 0..=steps {
        v.push(lo + (hi - lo) * k as f64 / steps as f64);
    }
    for edge in [min, max, grid_max] {
        v.push(edge);
        v.push(edge - 1e-9);
        v.push(edge + 1e-9);
        v.push(edge - 0.5 * cell);
        v.push(edge + 0.5 * cell);
    }
    v.extend([-1.0e6, 1.0e6, -1.0e3, 1.0e3, 0.0]);
    v
}

fn probes(mesh: &TriangleMesh, index: &SpatialIndex) -> Vec<(f64, f64)> {
    let cell = index.cell_size();
    let xs = axis_values(mesh.bbox.min.x, mesh.bbox.max.x, cell, index.cell_count_x());
    let ys = axis_values(mesh.bbox.min.y, mesh.bbox.max.y, cell, index.cell_count_y());
    let mut out = Vec::with_capacity(xs.len() * ys.len());
    for &x in &xs {
        for &y in &ys {
            out.push((x, y));
        }
    }
    out
}

fn radii(mesh: &TriangleMesh, index: &SpatialIndex) -> [f64; 4] {
    let extent = (mesh.bbox.max.x - mesh.bbox.min.x).max(mesh.bbox.max.y - mesh.bbox.min.y);
    [0.0, 0.1, index.cell_size(), 2.0 * extent]
}

#[test]
fn every_query_path_returns_the_reference_vector() {
    for (name, mesh, cell) in cases() {
        let index = build(&mesh, cell);
        let mut scratch = QueryScratch::new();
        let mut out = Vec::new();
        let mut checked = 0usize;
        let mut nonempty = 0usize;
        for (x, y) in probes(&mesh, &index) {
            for r in radii(&mesh, &index) {
                let want = reference_query(&mesh, &index, x, y, r);
                assert_eq!(
                    index.query(x, y, r),
                    want,
                    "{name}: query at ({x}, {y}) r={r}"
                );
                index.query_into(x, y, r, &mut scratch, &mut out);
                assert_eq!(out, want, "{name}: query_into at ({x}, {y}) r={r}");
                index.with_query(x, y, r, |got| {
                    assert_eq!(
                        got,
                        want.as_slice(),
                        "{name}: with_query at ({x}, {y}) r={r}"
                    );
                });
                checked += 1;
                nonempty += usize::from(!want.is_empty());
            }
        }
        // The probe set must reach both the empty and the full case.
        assert!(
            nonempty > 0 && nonempty < checked,
            "{name}: {nonempty} of {checked}"
        );
    }
}

#[test]
fn point_drop_cutter_is_bit_identical_to_the_reference_loop() {
    let ball = BallEndmill::new(6.0, 20.0);
    let flat = FlatEndmill::new(3.0, 20.0);
    let cutters: [&dyn MillingCutter; 2] = [&ball, &flat];
    for (name, mesh, cell) in cases() {
        let index = build(&mesh, cell);
        for (x, y) in probes(&mesh, &index) {
            for cutter in cutters {
                let got = point_drop_cutter(x, y, &mesh, &index, cutter);
                let want = reference_drop(x, y, &mesh, &index, cutter);
                assert_eq!(got.x.to_bits(), want.x.to_bits(), "{name}: x at ({x}, {y})");
                assert_eq!(got.y.to_bits(), want.y.to_bits(), "{name}: y at ({x}, {y})");
                assert_eq!(got.z.to_bits(), want.z.to_bits(), "{name}: z at ({x}, {y})");
                assert_eq!(
                    got.contacted, want.contacted,
                    "{name}: contacted at ({x}, {y})"
                );
            }
            assert_eq!(
                point_is_over_mesh_xy(x, y, &mesh, &index),
                reference_over(x, y, &mesh, &index),
                "{name}: over-mesh at ({x}, {y})"
            );
        }
    }
}

#[test]
fn a_nested_with_query_gets_its_own_buffer_and_does_not_panic() {
    let mesh = make_test_hemisphere(20.0, 24);
    let index = SpatialIndex::build(&mesh, 3.0);
    let r = index.cell_size();
    // Two points far apart, so the outer and inner results differ.
    let (ax, ay) = (-12.0, -9.0);
    let (bx, by) = (11.0, 13.0);
    let want_a = reference_query(&mesh, &index, ax, ay, r);
    let want_b = reference_query(&mesh, &index, bx, by, r);
    let want_c = reference_query(&mesh, &index, 0.0, 0.0, 2.0 * r);
    assert!(!want_a.is_empty() && !want_b.is_empty() && want_a != want_b);

    let depth = index.with_query(ax, ay, r, |outer| {
        assert_eq!(outer, want_a.as_slice());
        let inner_depth = index.with_query(bx, by, r, |inner| {
            assert_eq!(inner, want_b.as_slice());
            // `query` inside two levels of `with_query`.
            assert_eq!(index.query(0.0, 0.0, 2.0 * r), want_c);
            index.with_query(0.0, 0.0, 2.0 * r, |deepest| {
                assert_eq!(deepest, want_c.as_slice());
                3
            })
        });
        // The inner calls do not change the outer slice.
        assert_eq!(outer, want_a.as_slice());
        inner_depth
    });
    assert_eq!(depth, 3);

    // After the nest, the thread-local buffers still give correct results.
    for (x, y, rr, want) in [
        (ax, ay, r, &want_a),
        (bx, by, r, &want_b),
        (0.0, 0.0, 2.0 * r, &want_c),
    ] {
        index.with_query(x, y, rr, |got| assert_eq!(got, want.as_slice()));
        assert_eq!(&index.query(x, y, rr), want);
    }
}

#[test]
fn the_scratch_stays_clean_across_meshes_of_different_size() {
    // One thread queries a large mesh, then a small one, then the large one
    // again. A bit left set by one query would drop a triangle from the next.
    let big = make_test_hemisphere(20.0, 48);
    let small = strip_mesh();
    let big_index = SpatialIndex::build(&big, 2.0);
    let small_index = SpatialIndex::build(&small, 1.3);
    for _ in 0..3 {
        for (mesh, index) in [(&big, &big_index), (&small, &small_index)] {
            for (x, y) in probes(mesh, index).into_iter().step_by(7) {
                for r in radii(mesh, index) {
                    let want = reference_query(mesh, index, x, y, r);
                    assert_eq!(index.query(x, y, r), want);
                    index.with_query(x, y, r, |got| assert_eq!(got, want.as_slice()));
                }
            }
        }
    }
}
