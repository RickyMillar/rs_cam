//! W1 (perf sweep round 2, 2026-11-06): a point left of or below the
//! `SpatialIndex` grid must not read a whole row, column or mesh.
//!
//! The radius-0 query wraps on a negative cell index (`x1 as usize`) and
//! reads a whole row or column band, or the whole mesh at a low corner.
//! The radius-0 callers keep a triangle only when
//! `Triangle::contains_point_xy` accepts the point. This file pins that the
//! result of each caller stays the same, bit for bit:
//!
//! 1. `SpatialIndex::query` itself, radius 0 and radius > 0, against
//!    `reference_query`, a copy of the pre-W1 query body over the public
//!    API.
//! 2. `point_is_over_mesh_xy`, `maps::reach_map::surface_z_at` and
//!    `point_drop_cutter`, against the same predicate over
//!    `reference_query`.
//! 3. `SpatialIndex::with_point_candidates`, the exact-candidates door, under
//!    each caller predicate: any hit, max plane Z, the min/max model-Z fold
//!    of the simulation and the GUI drawing, and the first hit. The private
//!    `ModelZQuery` keeps its own P4 tests in `compute/simulate.rs`.
//! 4. Fast: a point far off the low sides gets an empty candidate list,
//!    where the reference reads a band of the grid.
//!
//! The points cover each low side, each low corner, the acceptance band
//! just outside the low edges (`contains_point_xy` accepts barycentric
//! values down to `-1e-8`), the cell edges and far values. The meshes are a
//! wavy height field at an offset origin, the same field with a long safe
//! triangle at the low edge, and the same field with an ill-conditioned
//! sliver that accepts a point 3.4 mm left of its own bounding box.

#![expect(
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_core::geo::{P3, Triangle};
use rs_cam_core::maps::reach_map::surface_z_at;
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::surface::dropcutter::{point_drop_cutter, point_is_over_mesh_xy};
use rs_cam_core::tool::{
    BallEndmill, CLPoint, FlatEndmill, MillingCutter, drop_cutter_can_contact,
};

/// The body of `SpatialIndex::query` before W1, over the public API. The
/// grid origin is `mesh.bbox.min` by the definition in `SpatialIndex::build`.
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
    let mut seen = vec![false; index.total_triangles()];
    if x0 > x1 || y0 > y1 {
        return result;
    }
    for cy_idx in y0..=y1 {
        for cx_idx in x0..=x1 {
            for &tri_idx in index.cell_slice(cy_idx * cell_count_x + cx_idx) {
                if !seen[tri_idx] {
                    seen[tri_idx] = true;
                    result.push(tri_idx);
                }
            }
        }
    }
    result
}

/// `point_is_over_mesh_xy` before W1.
fn over(mesh: &TriangleMesh, tris: &[usize], x: f64, y: f64) -> bool {
    tris.iter().any(|&i| mesh.faces[i].contains_point_xy(x, y))
}

/// The private `plane_z_at` of `maps/reach_map.rs`, verbatim.
fn plane_z_at(tri: &Triangle, x: f64, y: f64) -> Option<f64> {
    let n = tri.normal;
    if n.z.abs() < 1e-12 {
        return None;
    }
    let v0 = tri.v.first()?;
    let z = v0.z - (n.x * (x - v0.x) + n.y * (y - v0.y)) / n.z;
    z.is_finite().then_some(z)
}

/// `surface_z_at` before W1, over a candidate list.
fn surface_z(mesh: &TriangleMesh, tris: &[usize], x: f64, y: f64) -> Option<u64> {
    let mut best: Option<f64> = None;
    for &i in tris {
        let tri = &mesh.faces[i];
        if !tri.contains_point_xy(x, y) {
            continue;
        }
        let Some(z) = plane_z_at(tri, x, y) else {
            continue;
        };
        best = Some(best.map_or(z, |b: f64| b.max(z)));
    }
    best.map(f64::to_bits)
}

/// The model-Z fold of `compute/simulate.rs` (`fold_model_z_range`) and of
/// the GUI drawing: min and max of `z_at_xy` over the accepting triangles.
fn z_range(mesh: &TriangleMesh, tris: &[usize], x: f64, y: f64) -> Option<(u64, u64)> {
    let mut min_z: Option<f64> = None;
    let mut max_z: Option<f64> = None;
    for &i in tris {
        let face = &mesh.faces[i];
        if face.contains_point_xy(x, y)
            && let Some(z) = face.z_at_xy(x, y)
        {
            min_z = Some(min_z.map_or(z, |m: f64| m.min(z)));
            max_z = Some(max_z.map_or(z, |m: f64| m.max(z)));
        }
    }
    min_z.zip(max_z).map(|(a, b)| (a.to_bits(), b.to_bits()))
}

/// The first accepting triangle, the shape of the "first hit" callers.
fn first_hit(mesh: &TriangleMesh, tris: &[usize], x: f64, y: f64) -> Option<usize> {
    tris.iter()
        .copied()
        .find(|&i| mesh.faces[i].contains_point_xy(x, y))
}

/// The `point_drop_cutter` loop before W1, over `reference_query`.
fn reference_drop(
    x: f64,
    y: f64,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
) -> CLPoint {
    let mut cl = CLPoint::new(x, y);
    let r = cutter.radius();
    for i in reference_query(mesh, index, x, y, r) {
        let tri = &mesh.faces[i];
        if drop_cutter_can_contact(&cl, tri, r) {
            cutter.drop_cutter(&mut cl, tri);
        }
    }
    cl
}

/// A wavy height field over `[x0, x1] x [y0, y1]`, as vertices and faces.
fn field_parts(
    (x0, x1): (f64, f64),
    (y0, y1): (f64, f64),
    nx: usize,
    ny: usize,
) -> (Vec<P3>, Vec<[u32; 3]>) {
    let mut v = Vec::new();
    for j in 0..=ny {
        for i in 0..=nx {
            let x = x0 + (x1 - x0) * i as f64 / nx as f64;
            let y = y0 + (y1 - y0) * j as f64 / ny as f64;
            v.push(P3::new(x, y, 3.0 + (x * 0.7).sin() * (y * 0.3).cos()));
        }
    }
    let at = |i: usize, j: usize| (j * (nx + 1) + i) as u32;
    let mut t = Vec::new();
    for j in 0..ny {
        for i in 0..nx {
            t.push([at(i, j), at(i + 1, j), at(i + 1, j + 1)]);
            t.push([at(i, j), at(i + 1, j + 1), at(i, j + 1)]);
        }
    }
    (v, t)
}

fn with_triangle(
    (mut v, mut t): (Vec<P3>, Vec<[u32; 3]>),
    corners: [P3; 3],
) -> (Vec<P3>, Vec<[u32; 3]>) {
    let base = v.len() as u32;
    v.extend(corners);
    t.push([base, base + 1, base + 2]);
    (v, t)
}

/// The ill sliver. Its vertices are near-collinear (`|D| < 1e-9 W^2`), so
/// the computed barycentric values are rounding noise. Found by a random
/// search over float doubles: `contains_point_xy` accepts [`SLIVER_POINT`],
/// which is 3.39 mm left of the sliver's min X and below its min Y.
const SLIVER: [(f64, f64); 3] = [
    (0.590_660_442_595_780_2, 0.378_407_641_407_748_3),
    (192.838_743_350_394_05, 25.898_516_419_502_183),
    (61.407_613_879_456_15, 8.451_597_468_769_219),
];
const SLIVER_POINT: (f64, f64) = (-2.795_573_184_509_477_6, -0.071_100_354_743_312_72);

fn sliver_corners() -> [P3; 3] {
    SLIVER.map(|(x, y)| P3::new(x, y, 1.0))
}

/// The meshes under test, with a cell size for `SpatialIndex::build`
/// (`None`: `build_auto`).
fn cases() -> Vec<(&'static str, TriangleMesh, Option<f64>)> {
    let field = || field_parts((6.0, 32.0), (5.0, 22.0), 27, 18);
    let (v, t) = field();
    let plain = TriangleMesh::from_raw(v, t);
    // A long, well-conditioned triangle along the low-left edge: W is about
    // 2000, so its `1e-5 * W` margin is about 0.02 mm.
    let (v, t) = with_triangle(
        field(),
        [
            P3::new(5.5, 4.0, 2.0),
            P3::new(1005.5, 4.5, 2.0),
            P3::new(5.75, 9.0, 2.5),
        ],
    );
    let long = TriangleMesh::from_raw(v, t);
    // The sliver mesh: a field near the sliver, plus the sliver.
    let (v, t) = with_triangle(
        field_parts((0.6, 190.0), (0.4, 30.0), 40, 12),
        sliver_corners(),
    );
    let sliver = TriangleMesh::from_raw(v, t);
    vec![
        ("plain/auto", plain.clone(), None),
        ("plain/1.7", plain, Some(1.7)),
        ("long/auto", long.clone(), None),
        ("long/3.0", long, Some(3.0)),
        ("sliver/auto", sliver.clone(), None),
        ("sliver/5.0", sliver, Some(5.0)),
    ]
}

fn build(mesh: &TriangleMesh, cell: Option<f64>) -> SpatialIndex {
    cell.map_or_else(
        || SpatialIndex::build_auto(mesh),
        |c| SpatialIndex::build(mesh, c),
    )
}

/// One axis: a lattice across the grid and past both edges, each cell edge,
/// the low and high edges with their float neighbours and the acceptance
/// band below the low edge, far values and the non-finite values.
fn axis(min: f64, max: f64, cell: f64, cells: usize) -> Vec<f64> {
    let grid_max = min + cell * cells as f64;
    let mut v = Vec::new();
    let (lo, hi) = (min - 3.0 * cell, grid_max + 2.0 * cell);
    for k in 0..=50 {
        v.push(lo + (hi - lo) * f64::from(k) / 50.0);
    }
    for k in 0..=cells {
        v.push(min + cell * k as f64);
    }
    for edge in [min, max, grid_max] {
        v.extend([edge, edge.next_down(), edge.next_up()]);
    }
    for d in [1e-12, 1e-9, 1e-7, 1e-6, 1e-5, 1e-4, 2e-2, 0.5 * cell] {
        v.push(min - d);
    }
    v.extend([-1e6, -1e3, 0.0, 1e3, 1e6]);
    v.extend([f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1e300, 1e300]);
    v
}

fn probes(mesh: &TriangleMesh, index: &SpatialIndex) -> Vec<(f64, f64)> {
    let cs = index.cell_size();
    let xs = axis(mesh.bbox.min.x, mesh.bbox.max.x, cs, index.cell_count_x());
    let ys = axis(mesh.bbox.min.y, mesh.bbox.max.y, cs, index.cell_count_y());
    let mut out = Vec::with_capacity(xs.len() * ys.len() + 1);
    for &x in &xs {
        for &y in &ys {
            out.push((x, y));
        }
    }
    out.push(SLIVER_POINT);
    out
}

#[test]
fn the_sliver_accepts_a_point_far_left_of_its_box() {
    // The premise of the sliver case: a cell bound alone cannot keep this
    // triangle, so the exact door must test the ill triangles one by one.
    let [a, b, c] = sliver_corners();
    let tri = Triangle::new(a, b, c);
    let (x, y) = SLIVER_POINT;
    assert!(tri.contains_point_xy(x, y), "the sliver refuses the point");
    assert!(tri.bbox.min.x - x > 3.0, "the point is not far left");
    let (_, mesh, cell) = cases().swap_remove(4);
    let index = build(&mesh, cell);
    let tris = reference_query(&mesh, &index, x, y, 0.0);
    assert!(over(&mesh, &tris, x, y), "the reference misses the sliver");
}

#[test]
fn the_query_is_the_reference_vector() {
    for (name, mesh, cell) in cases() {
        let index = build(&mesh, cell);
        let extent = (mesh.bbox.max.x - mesh.bbox.min.x).max(mesh.bbox.max.y - mesh.bbox.min.y);
        for (x, y) in probes(&mesh, &index) {
            for r in [0.0, 0.1, index.cell_size(), 2.0 * extent] {
                assert_eq!(
                    index.query(x, y, r),
                    reference_query(&mesh, &index, x, y, r),
                    "{name}: query at ({x:e}, {y:e}) r={r}"
                );
            }
        }
    }
}

#[test]
fn the_radius_zero_callers_give_the_reference_result() {
    for (name, mesh, cell) in cases() {
        let index = build(&mesh, cell);
        let mut hits = 0usize;
        let pts = probes(&mesh, &index);
        for &(x, y) in &pts {
            let want = reference_query(&mesh, &index, x, y, 0.0);
            let want_over = over(&mesh, &want, x, y);
            assert_eq!(
                point_is_over_mesh_xy(x, y, &mesh, &index),
                want_over,
                "{name}: point_is_over_mesh_xy at ({x:e}, {y:e})"
            );
            assert_eq!(
                surface_z_at(x, y, &mesh, &index).map(f64::to_bits),
                surface_z(&mesh, &want, x, y),
                "{name}: surface_z_at at ({x:e}, {y:e})"
            );
            hits += usize::from(want_over);
        }
        assert!(
            hits > pts.len() / 50 && hits < pts.len(),
            "{name}: {hits} hits of {}",
            pts.len()
        );
    }
}

#[test]
fn the_drop_cutter_gives_the_reference_cl_point() {
    let ball = BallEndmill::new(6.0, 20.0);
    let flat = FlatEndmill::new(3.0, 20.0);
    let cutters: [&dyn MillingCutter; 2] = [&ball, &flat];
    for (name, mesh, cell) in cases() {
        let index = build(&mesh, cell);
        for (x, y) in probes(&mesh, &index).into_iter().step_by(3) {
            for cutter in cutters {
                let got = point_drop_cutter(x, y, &mesh, &index, cutter);
                let want = reference_drop(x, y, &mesh, &index, cutter);
                assert_eq!(
                    (got.z.to_bits(), got.contacted),
                    (want.z.to_bits(), want.contacted),
                    "{name}: drop at ({x:e}, {y:e})"
                );
            }
        }
    }
}

#[test]
fn the_exact_door_gives_the_reference_result_under_each_predicate() {
    for (name, mesh, cell) in cases() {
        let index = build(&mesh, cell);
        for (x, y) in probes(&mesh, &index) {
            let want = reference_query(&mesh, &index, x, y, 0.0);
            index.with_point_candidates(&mesh, x, y, |got| {
                let at = format!("{name}: ({x:e}, {y:e})");
                assert_eq!(over(&mesh, got, x, y), over(&mesh, &want, x, y), "{at}");
                assert_eq!(
                    surface_z(&mesh, got, x, y),
                    surface_z(&mesh, &want, x, y),
                    "{at}"
                );
                assert_eq!(
                    z_range(&mesh, got, x, y),
                    z_range(&mesh, &want, x, y),
                    "{at}"
                );
                assert_eq!(
                    first_hit(&mesh, got, x, y),
                    first_hit(&mesh, &want, x, y),
                    "{at}"
                );
                // The accepting triangles, in order: the same list.
                let keep = |tris: &[usize]| -> Vec<usize> {
                    tris.iter()
                        .copied()
                        .filter(|&i| mesh.faces[i].contains_point_xy(x, y))
                        .collect()
                };
                assert_eq!(keep(got), keep(&want), "{at}");
            });
        }
    }
}

#[test]
fn a_point_far_off_the_low_sides_reads_no_band() {
    let mut skipped = 0usize;
    for (name, mesh, cell) in cases() {
        let index = build(&mesh, cell);
        let (ox, oy) = (mesh.bbox.min.x, mesh.bbox.min.y);
        let cs = index.cell_size();
        let (mx, my) = (
            0.5 * (mesh.bbox.min.x + mesh.bbox.max.x),
            0.5 * (mesh.bbox.min.y + mesh.bbox.max.y),
        );
        // Left of the grid, below it, and off the low corner: one cell and
        // far away.
        for (x, y) in [
            (ox - cs, my),
            (mx, oy - cs),
            (ox - cs, oy - cs),
            (ox - 1e3, my),
            (mx, oy - 1e3),
            (ox - 1e6, oy - 1e6),
        ] {
            let band = reference_query(&mesh, &index, x, y, 0.0).len();
            let read = index.with_point_candidates(&mesh, x, y, <[usize]>::len);
            // The sliver accepts points millimetres off its box, so on the
            // sliver mesh a point one cell off can still need the band.
            let near_sliver = name.starts_with("sliver") && (ox - x).max(oy - y) < 10.0;
            if near_sliver {
                assert!(read == 0 || read == band, "{name}: ({x}, {y})");
                continue;
            }
            assert!(
                band >= 1,
                "{name}: the reference reads no band at ({x}, {y})"
            );
            assert_eq!(read, 0, "{name}: ({x}, {y}) reads {read} of {band}");
            skipped += band;
        }
    }
    // The low-corner points alone read the whole mesh in the reference.
    assert!(
        skipped > 1000,
        "the reference read only {skipped} candidates"
    );
}
