//! G1 — the push-cutter fiber query is bounded to the fiber's own band.
//!
//! `PERF_REVIEW.md` G1: `pushcutter.rs` sized its spatial-index query by the
//! fiber's *length* (`query_r = fiber.length()/2 + r`, a square around the
//! fiber midpoint). Waterline fibers span the whole mesh bbox, so that square
//! covered the entire index and returned every triangle in the mesh — the
//! index pruned nothing at all. The fix queries the fiber's XY bounding box
//! inflated by the cutter's lateral reach: for an X-fiber at row `y`, the band
//! `y ± reach` instead of a square of side `fiber_length + 2·reach`.
//!
//! **This is a pruning change and nothing else.** A bounded query that drops a
//! triangle which *could* have contacted is a gouge, not a slow path, so this
//! file is the equivalence evidence: it re-implements the OLD unbounded query
//! verbatim and asserts the new one produces **bit-identical** fiber intervals
//! and bit-identical woven contours across every shipped cutter shape, several
//! mesh classes, and Z levels including ones that graze the mesh extremes.
//!
//! Wave 1's G3 is the reason the bar is bit-identity rather than "looks the
//! same": a reject that read as provably sound (`bbox.max.z <= cl.z`) turned
//! out to be systematically wrong for every triangle sharing a flat-tip
//! contact vertex, and was caught only by a fingerprint sentry that showed an
//! identical move count and a different hash. So the boundary cases are
//! deliberately hostile here: fibers laid exactly on index cell edges,
//! triangles whose bboxes end exactly on a cell boundary, a cutter whose
//! envelope is an exact multiple of the cell size, and Z planes at the exact
//! mesh min/max.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    // The `#[ignore]`d lever-2 headroom harness REPORTS a measurement — the
    // number is its entire output, and it is read off the terminal by whoever
    // runs it explicitly. Same reason `classification.rs` and
    // `pocket_lift_bridge_b1.rs` carry the print allows.
    clippy::print_stdout
)]

mod common;

use common::fingerprint::fnv1a_debug;

use rs_cam_core::contour_extract::weave_contours;
use rs_cam_core::fiber::Fiber;
use rs_cam_core::geo::P3;
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh, make_test_hemisphere};
use rs_cam_core::pushcutter::{
    batch_push_cutter, fiber_lateral_reach_mm, push_cutter_fiber, push_cutter_triangle,
};
use rs_cam_core::tool::{
    BallEndmill, BullNoseEndmill, FlatEndmill, MillingCutter, TaperedBallEndmill, VBitEndmill,
};

// ── the pre-G1 query, reproduced verbatim ────────────────────────────────

/// `push_cutter_fiber` exactly as it stood before G1 (commit-era
/// `pushcutter.rs:31-54`): a square query centred on the fiber midpoint whose
/// half-side is `fiber.length()/2 + cutter.radius()`, then the same per-triangle
/// Z reject and the same `push_cutter_triangle` call.
///
/// Kept here rather than behind a feature flag in the library so the shipped
/// crate carries one query path, not two.
fn reference_push_cutter_fiber(
    fiber: &mut Fiber,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
) {
    let r = cutter.radius();
    let length = cutter.length();

    let cx = (fiber.p1.x + fiber.p2.x) / 2.0;
    let cy = (fiber.p1.y + fiber.p2.y) / 2.0;
    let half_len = fiber.length() / 2.0;
    let query_r = half_len + r;
    let z_min = fiber.z();
    let z_max = fiber.z() + length;

    let candidates = index.query(cx, cy, query_r);

    for &tri_idx in &candidates {
        let tri = &mesh.faces[tri_idx];
        let tri_z_min = tri.v[0].z.min(tri.v[1].z).min(tri.v[2].z);
        let tri_z_max = tri.v[0].z.max(tri.v[1].z).max(tri.v[2].z);
        if tri_z_min > z_max || tri_z_max < z_min {
            continue;
        }
        push_cutter_triangle(fiber, tri, cutter);
    }
}

// ── bit-level comparison ─────────────────────────────────────────────────

/// Intervals as raw bit patterns, so a last-ULP divergence cannot hide behind
/// a tolerance and `-0.0 == 0.0` cannot mask one.
fn interval_bits(fiber: &Fiber) -> Vec<(u64, u64)> {
    fiber
        .intervals()
        .iter()
        .map(|iv| (iv.lower.to_bits(), iv.upper.to_bits()))
        .collect()
}

fn contour_bits(contours: &[Vec<P3>]) -> Vec<Vec<(u64, u64, u64)>> {
    contours
        .iter()
        .map(|c| {
            c.iter()
                .map(|p| (p.x.to_bits(), p.y.to_bits(), p.z.to_bits()))
                .collect()
        })
        .collect()
}

/// The waterline fiber layout, reproduced from `waterline_contours_with_cancel`
/// so the equivalence check runs on the exact grid the shipped generator uses.
fn waterline_fibers(
    mesh: &TriangleMesh,
    cutter: &dyn MillingCutter,
    z: f64,
    sampling: f64,
) -> (Vec<Fiber>, Vec<Fiber>) {
    let bbox = &mesh.bbox;
    let r = cutter.radius();
    let x_min = bbox.min.x - r;
    let x_max = bbox.max.x + r;
    let y_min = bbox.min.y - r;
    let y_max = bbox.max.y + r;

    let ny = ((y_max - y_min) / sampling).ceil() as usize + 1;
    let x_fibers: Vec<Fiber> = (0..ny)
        .map(|i| Fiber::new_x(y_min + i as f64 * sampling, z, x_min, x_max))
        .collect();

    let nx = ((x_max - x_min) / sampling).ceil() as usize + 1;
    let y_fibers: Vec<Fiber> = (0..nx)
        .map(|i| Fiber::new_y(x_min + i as f64 * sampling, z, y_min, y_max))
        .collect();

    (x_fibers, y_fibers)
}

/// Assert the bounded query reproduces the unbounded one, fiber for fiber and
/// contour for contour, on one (mesh, cutter, z) triple.
///
/// Returns `(candidate_ratio_numerator, denominator)` — the total triangle
/// tests each path performed — so callers can also assert the change actually
/// pruned something rather than silently degenerating to the old query.
fn assert_level_identical(
    label: &str,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    z: f64,
    sampling: f64,
) {
    let (mut ref_x, mut ref_y) = waterline_fibers(mesh, cutter, z, sampling);
    let (mut new_x, mut new_y) = waterline_fibers(mesh, cutter, z, sampling);

    for f in ref_x.iter_mut().chain(ref_y.iter_mut()) {
        reference_push_cutter_fiber(f, mesh, index, cutter);
    }
    batch_push_cutter(&mut new_x, mesh, index, cutter);
    batch_push_cutter(&mut new_y, mesh, index, cutter);

    for (i, (a, b)) in ref_x.iter().zip(new_x.iter()).enumerate() {
        assert_eq!(
            interval_bits(a),
            interval_bits(b),
            "{label}: x-fiber {i} (y={}, z={z}) intervals differ",
            a.p1.y
        );
    }
    for (i, (a, b)) in ref_y.iter().zip(new_y.iter()).enumerate() {
        assert_eq!(
            interval_bits(a),
            interval_bits(b),
            "{label}: y-fiber {i} (x={}, z={z}) intervals differ",
            a.p1.x
        );
    }

    // The contours are what the generator actually emits — compare those too,
    // because marching squares reads interval *boundaries* and a divergence
    // too small to show in one fiber can still move a woven vertex.
    let ref_contours = weave_contours(&ref_x, &ref_y, z);
    let new_contours = weave_contours(&new_x, &new_y, z);
    assert_eq!(
        contour_bits(&ref_contours),
        contour_bits(&new_contours),
        "{label}: woven contours differ at z={z}"
    );
    assert_eq!(
        fnv1a_debug(&ref_contours),
        fnv1a_debug(&new_contours),
        "{label}: contour fingerprint differs at z={z}"
    );
}

/// A Z ladder that deliberately includes the degenerate ends: exactly the mesh
/// top, exactly the mesh bottom, a hair above and below each, and a spread of
/// interior planes.
fn hostile_z_levels(mesh: &TriangleMesh) -> Vec<f64> {
    let lo = mesh.bbox.min.z;
    let hi = mesh.bbox.max.z;
    let span = (hi - lo).max(1e-6);
    let mut levels = vec![
        hi,
        hi + 1e-9,
        hi - 1e-9,
        lo,
        lo + 1e-9,
        lo - 1e-9,
        lo - span,
        hi + span,
    ];
    for i in 1..8 {
        levels.push(lo + span * (i as f64) / 8.0);
    }
    levels
}

// ── fixtures ─────────────────────────────────────────────────────────────

/// A rolling height field — the shape class the `gen_waterline` bench uses.
fn rolling_field(half: f64, n: usize) -> TriangleMesh {
    let step = 2.0 * half / (n - 1) as f64;
    let mut vertices = Vec::with_capacity(n * n);
    for iy in 0..n {
        let y = -half + iy as f64 * step;
        for ix in 0..n {
            let x = -half + ix as f64 * step;
            let z = 1.6 * (x * 0.9).sin() * (y * 0.7).cos() + 0.9 * (x * 2.3 + y * 1.7).sin()
                - 0.35 * (x * x + y * y).sqrt();
            vertices.push(P3::new(x, y, z));
        }
    }
    let mut triangles = Vec::with_capacity(2 * (n - 1) * (n - 1));
    for iy in 0..n - 1 {
        for ix in 0..n - 1 {
            let a = (iy * n + ix) as u32;
            let b = a + 1;
            let c = a + n as u32;
            let d = c + 1;
            triangles.push([a, c, b]);
            triangles.push([b, c, d]);
        }
    }
    TriangleMesh::from_raw(vertices, triangles)
}

/// A grid-aligned stepped plate: every vertex lands on an exact multiple of
/// `step`, so triangle bboxes end precisely on index cell boundaries when the
/// index cell size divides `step`. This is the fixture for the cell-boundary
/// tie case.
fn grid_aligned_steps(half: f64, step: f64) -> TriangleMesh {
    let n = ((2.0 * half) / step).round() as usize + 1;
    let mut vertices = Vec::with_capacity(n * n);
    for iy in 0..n {
        let y = -half + iy as f64 * step;
        for ix in 0..n {
            let x = -half + ix as f64 * step;
            // Piecewise-constant terraces at exact millimetre heights, so many
            // triangles are exactly horizontal and share exact vertex Z values.
            let z = ((ix / 3) as f64).min(4.0) - ((iy / 4) as f64).min(3.0);
            vertices.push(P3::new(x, y, z));
        }
    }
    let mut triangles = Vec::with_capacity(2 * (n - 1) * (n - 1));
    for iy in 0..n - 1 {
        for ix in 0..n - 1 {
            let a = (iy * n + ix) as u32;
            let b = a + 1;
            let c = a + n as u32;
            let d = c + 1;
            triangles.push([a, c, b]);
            triangles.push([b, c, d]);
        }
    }
    TriangleMesh::from_raw(vertices, triangles)
}

// ── the tests ────────────────────────────────────────────────────────────

#[test]
fn band_query_matches_unbounded_on_hemisphere_every_shape() {
    let mesh = make_test_hemisphere(20.0, 24);
    let index = SpatialIndex::build_auto(&mesh);

    let flat = FlatEndmill::new(6.0, 25.0);
    let ball = BallEndmill::new(6.0, 25.0);
    let bull = BullNoseEndmill::new(8.0, 1.5, 25.0);
    let vbit = VBitEndmill::new(12.0, 90.0, 25.0);
    let taper = TaperedBallEndmill::new(2.0, 5.0, 6.0, 25.0);

    let shapes: [(&str, &dyn MillingCutter); 5] = [
        ("flat6", &flat),
        ("ball6", &ball),
        ("bull8r1.5", &bull),
        ("vbit12x90", &vbit),
        ("taperball", &taper),
    ];

    for (name, cutter) in shapes {
        for z in hostile_z_levels(&mesh) {
            assert_level_identical(name, &mesh, &index, cutter, z, 2.0);
        }
    }
}

#[test]
fn band_query_matches_unbounded_on_rolling_terrain() {
    let mesh = rolling_field(20.0, 61);
    let index = SpatialIndex::build_auto(&mesh);
    let ball = BallEndmill::new(6.0, 25.0);

    for z in hostile_z_levels(&mesh) {
        assert_level_identical("rolling61_ball6", &mesh, &index, &ball, z, 2.0);
    }
}

/// Cell-boundary ties: an index whose cell size divides the mesh's vertex
/// pitch, a cutter whose envelope radius is an exact multiple of that cell
/// size, and a fiber grid whose sampling puts fibers exactly on cell edges.
/// Every quantity in `floor((y ± reach - origin) / cell_size)` is therefore an
/// exact integer boundary case.
#[test]
fn band_query_matches_unbounded_on_exact_cell_boundaries() {
    let mesh = grid_aligned_steps(12.0, 1.0);
    // origin_y = bbox.min.y = -12.0, cell 0.5 → every fiber at a 0.5 multiple
    // sits exactly on a cell edge, and reach = 3.0 + slack is 6 cells.
    let index = SpatialIndex::build(&mesh, 0.5);
    let flat = FlatEndmill::new(6.0, 25.0);
    let ball = BallEndmill::new(6.0, 25.0);

    for z in hostile_z_levels(&mesh) {
        assert_level_identical("gridsteps_flat6", &mesh, &index, &flat, z, 0.5);
        assert_level_identical("gridsteps_ball6", &mesh, &index, &ball, z, 0.5);
    }
}

/// A fiber grid offset by exactly half a cell, so no fiber ever lands on a
/// boundary and every band edge falls mid-cell — the complement of the case
/// above.
#[test]
fn band_query_matches_unbounded_on_half_cell_offset_fibers() {
    let mesh = grid_aligned_steps(12.0, 1.0);
    let index = SpatialIndex::build(&mesh, 0.5);
    let ball = BallEndmill::new(5.0, 25.0);
    let bbox = &mesh.bbox;
    let r = ball.radius();

    for z in [bbox.max.z, 0.5, -0.25, bbox.min.z] {
        let mut ref_fibers: Vec<Fiber> = (0..60)
            .map(|i| {
                let y = bbox.min.y + 0.25 + i as f64 * 0.4;
                Fiber::new_x(y, z, bbox.min.x - r, bbox.max.x + r)
            })
            .collect();
        let mut new_fibers = ref_fibers.clone();

        for f in ref_fibers.iter_mut() {
            reference_push_cutter_fiber(f, &mesh, &index, &ball);
        }
        for f in new_fibers.iter_mut() {
            push_cutter_fiber(f, &mesh, &index, &ball);
        }
        for (i, (a, b)) in ref_fibers.iter().zip(new_fibers.iter()).enumerate() {
            assert_eq!(
                interval_bits(a),
                interval_bits(b),
                "half-cell-offset fiber {i} at z={z} differs"
            );
        }
    }
}

/// A mesh far larger than the cutter in one axis only: catches a band that
/// accidentally kept the fiber-length term on the perpendicular axis.
#[test]
fn band_query_matches_unbounded_on_long_thin_mesh() {
    let mut vertices = Vec::new();
    let mut triangles = Vec::new();
    let n = 201;
    for i in 0..n {
        let x = -100.0 + i as f64 * 1.0;
        let z = 2.0 * (x * 0.31).sin();
        vertices.push(P3::new(x, -4.0, z));
        vertices.push(P3::new(x, 4.0, z + 0.7 * (x * 0.13).cos()));
    }
    for i in 0..n - 1 {
        let a = (2 * i) as u32;
        let b = a + 1;
        let c = a + 2;
        let d = a + 3;
        triangles.push([a, c, b]);
        triangles.push([b, c, d]);
    }
    let mesh = TriangleMesh::from_raw(vertices, triangles);
    let index = SpatialIndex::build_auto(&mesh);
    let ball = BallEndmill::new(6.0, 25.0);

    for z in hostile_z_levels(&mesh) {
        assert_level_identical("longthin_ball6", &mesh, &index, &ball, z, 1.0);
    }
}

/// The change must actually prune. Without this, a band that silently
/// degenerated back to the full grid would still pass every equivalence test
/// above — the vacuity failure `gate_population_vacuity_xvac` exists for,
/// applied to a perf fix.
#[test]
fn band_query_actually_prunes() {
    let mesh = rolling_field(20.0, 61);
    let index = SpatialIndex::build_auto(&mesh);
    let ball = BallEndmill::new(6.0, 25.0);
    let bbox = &mesh.bbox;
    let z = (bbox.min.z + bbox.max.z) / 2.0;
    let reach = fiber_lateral_reach_mm(&ball);

    let fiber = Fiber::new_x(0.0, z, bbox.min.x - 3.0, bbox.max.x + 3.0);

    let old = index.query(
        (fiber.p1.x + fiber.p2.x) / 2.0,
        (fiber.p1.y + fiber.p2.y) / 2.0,
        fiber.length() / 2.0 + ball.radius(),
    );
    let mut scratch = rs_cam_core::mesh::QueryScratch::new();
    let mut new = Vec::new();
    rs_cam_core::pushcutter::fiber_query_candidates(
        &fiber, &index, &ball, &mut scratch, &mut new,
    );

    assert_eq!(
        old.len(),
        mesh.faces.len(),
        "pre-G1 baseline: the unbounded query is expected to return the WHOLE mesh"
    );
    assert!(
        new.len() * 4 < old.len(),
        "band query returned {} of {} triangles — expected a large reduction \
         (reach {reach:.4} mm on a {:.1} mm mesh)",
        new.len(),
        old.len(),
        bbox.max.y - bbox.min.y
    );
    // And the retained set must be a superset of everything that can contact:
    // every triangle the reference path would have tested and that lies within
    // `reach` of the fiber row must survive.
    let kept: std::collections::HashSet<usize> = new.iter().copied().collect();
    for (i, tri) in mesh.faces.iter().enumerate() {
        let ymin = tri.v[0].y.min(tri.v[1].y).min(tri.v[2].y);
        let ymax = tri.v[0].y.max(tri.v[1].y).max(tri.v[2].y);
        if ymin <= fiber.p1.y + ball.radius() && ymax >= fiber.p1.y - ball.radius() {
            assert!(
                kept.contains(&i),
                "triangle {i} (y {ymin:.4}..{ymax:.4}) is within the cutter radius of the \
                 fiber row but was pruned"
            );
        }
    }
}

/// Not a regression test — the measurement that decides whether G1's SECOND
/// lever ("the fiber grid's XY is identical at every Z — compute per-row
/// candidates once and reuse them across levels") is worth its restructure.
///
/// Reuse can only remove the **query** half of a level's cost; the contact
/// math is Z-dependent and has to run at every level regardless. So the
/// ceiling on lever 2 is `query_share × (1 − 1/levels)`, and this prints
/// `query_share` on the `gen_waterline` bench fixture.
///
/// Run explicitly, in release (a debug timing of float-heavy contact math is
/// not representative of anything):
///
/// ```text
/// cargo test -p rs_cam_core --release --test pushcutter_band_query_g1 \
///     -- --ignored --nocapture measure_query_vs_contact_split
/// ```
#[test]
#[ignore = "measurement, not an assertion — see doc comment"]
fn measure_query_vs_contact_split() {
    use std::time::Instant;

    let mesh = rolling_field(20.0, 61);
    let index = SpatialIndex::build_auto(&mesh);
    let ball = BallEndmill::new(6.0, 25.0);
    let bbox = &mesh.bbox;
    let levels: Vec<f64> = (0..20)
        .map(|i| bbox.min.z + (bbox.max.z - bbox.min.z) * (i as f64 + 0.5) / 20.0)
        .collect();

    let mut scratch = rs_cam_core::mesh::QueryScratch::new();
    let mut candidates = Vec::new();

    // Query only.
    let t0 = Instant::now();
    let mut n_candidates = 0usize;
    for &z in &levels {
        let (xs, ys) = waterline_fibers(&mesh, &ball, z, 2.0);
        for f in xs.iter().chain(ys.iter()) {
            rs_cam_core::pushcutter::fiber_query_candidates(
                f,
                &index,
                &ball,
                &mut scratch,
                &mut candidates,
            );
            n_candidates += candidates.len();
        }
    }
    let query_only = t0.elapsed();

    // Query + contact math.
    let t1 = Instant::now();
    let mut n_intervals = 0usize;
    for &z in &levels {
        let (mut xs, mut ys) = waterline_fibers(&mesh, &ball, z, 2.0);
        batch_push_cutter(&mut xs, &mesh, &index, &ball);
        batch_push_cutter(&mut ys, &mesh, &index, &ball);
        n_intervals += xs.iter().chain(ys.iter()).map(|f| f.intervals().len()).sum::<usize>();
    }
    let full = t1.elapsed();

    let share = query_only.as_secs_f64() / full.as_secs_f64();
    println!(
        "G1 lever-2 headroom: query {:?}, full {:?}, query_share {:.1}% \
         (candidates {n_candidates}, intervals {n_intervals}); \
         ceiling on reuse across {} levels = {:.1}%",
        query_only,
        full,
        share * 100.0,
        levels.len(),
        share * (1.0 - 1.0 / levels.len() as f64) * 100.0
    );
    assert!(n_candidates > 0 && n_intervals > 0, "measurement was vacuous");
}

/// `query_rect_into` must remain a drop-in for `query_into` / `query` on a
/// square window — the three paths share one implementation and this pins that
/// they cannot drift.
#[test]
fn rect_query_matches_square_query() {
    let mesh = rolling_field(15.0, 41);
    let index = SpatialIndex::build_auto(&mesh);
    let mut scratch = rs_cam_core::mesh::QueryScratch::new();
    let mut rect_out = Vec::new();
    let mut sq_out = Vec::new();

    for cx in [-20.0, -7.5, 0.0, 3.25, 15.0, 40.0] {
        for cy in [-20.0, -7.5, 0.0, 3.25, 15.0, 40.0] {
            for r in [0.0, 0.4, 3.0, 12.0] {
                index.query_into(cx, cy, r, &mut scratch, &mut sq_out);
                index.query_rect_into(cx - r, cx + r, cy - r, cy + r, &mut scratch, &mut rect_out);
                assert_eq!(sq_out, rect_out, "rect vs square at ({cx},{cy}) r={r}");
                assert_eq!(
                    index.query(cx, cy, r),
                    rect_out,
                    "rect vs allocating query at ({cx},{cy}) r={r}"
                );
            }
        }
    }
}
