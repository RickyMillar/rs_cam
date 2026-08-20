//! Scaling harness for PERF_REVIEW **G5** (six hand-rolled O(n²) greedy
//! nearest-neighbour orderers) and **G6** (`surface_link` rescanning
//! provenance once per fragment).
//!
//! Both findings are about an *exponent*, not an absolute, so this harness
//! measures the same work at two sizes and reports the empirical exponent
//! `log(t2/t1) / log(n2/n1)`. A quadratic reads ≈ 2.0; the fix should read
//! near 1.0–1.3 (the residual being the linear emission work that dominates
//! once the quadratic is gone).
//!
//! It is written to be run **both sides** of the change:
//!
//! ```text
//! git stash push -- crates/rs_cam_core/src/tsp.rs \
//!     crates/rs_cam_core/src/surface_link.rs crates/rs_cam_core/src/lib.rs
//! cargo test --release -p rs_cam_core --test nn_order_scaling_g5_g6 -- --nocapture
//! git stash pop
//! cargo test --release -p rs_cam_core --test nn_order_scaling_g5_g6 -- --nocapture
//! ```
//!
//! The assertions are deliberately loose (they only have to separate a
//! quadratic from a near-linear on a contended machine); the *reported*
//! numbers are the deliverable, and they live in
//! `planning/perf_review_2026-08-19/DELTA_gen_w3b.md`.
//!
//! `nn_order_relink_provenance_is_unchanged` is not a timing test: it pins
//! G6's inversion by asserting the whole `MoveRemap` — every input move's
//! output range, junction rapids included — against a fingerprint of the
//! transform's own output. That is the channel G6 rewrote.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]

// Just the canonical fingerprint, not the whole `common` fixture library:
// this harness needs one function and pulling `mod common;` would compile ten
// unrelated submodules into it.
#[path = "common/fingerprint.rs"]
mod fingerprint;

use std::time::Instant;

use rs_cam_core::geo::P3;
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::surface_link::{RelinkParams, relink_fragments};
use rs_cam_core::tool::BallEndmill;
use rs_cam_core::toolpath::{MoveIntent, Toolpath};
use rs_cam_core::toolpath_spans::AnnotatedToolpath;
use rs_cam_core::transform_provenance::ReconcileSet;

// ── fixtures ────────────────────────────────────────────────────────────

/// The `gen_rapid_order` bench's own scatter — `n` disjoint cutting segments
/// separated by retract + rapid, deliberately not in raster order.
fn scattered_segments(n: usize) -> Toolpath {
    let mut tp = Toolpath::new();
    for i in 0..n {
        let t = i as f64;
        let x = 200.0 * (t * 0.9137).sin().abs();
        let y = 200.0 * (t * 0.4271).cos().abs();
        tp.rapid_to(P3::new(x, y, 5.0));
        tp.feed_to(P3::new(x, y, -1.0), 400.0);
        tp.feed_to(P3::new(x + 1.5, y + 0.8, -1.0), 1200.0);
        tp.rapid_to(P3::new(x + 1.5, y + 0.8, 5.0));
    }
    tp
}

/// A flat plate at z = 0 — enough mesh for `relink_fragments` to have an
/// index and a cutter, without the drop-cutter cost dominating the ordering
/// cost this harness is about.
fn flat_plate(size: f64, n: usize) -> TriangleMesh {
    let mut verts = Vec::new();
    let s = size / n as f64;
    for j in 0..=n {
        for i in 0..=n {
            verts.push(P3::new(i as f64 * s, j as f64 * s, 0.0));
        }
    }
    let idx = |i: usize, j: usize| (j * (n + 1) + i) as u32;
    let mut tris = Vec::new();
    for j in 0..n {
        for i in 0..n {
            tris.push([idx(i, j), idx(i + 1, j), idx(i + 1, j + 1)]);
            tris.push([idx(i, j), idx(i + 1, j + 1), idx(i, j + 1)]);
        }
    }
    TriangleMesh::from_raw(verts, tris)
}

/// `f` short cut runs at scattered XY, each separated by the
/// retract → traverse → plunge triple `relink_fragments` rewrites. Every
/// fragment carries 5 non-rapid moves and 2 junction rapids, so `n_in ≈ 7f`
/// — which is what makes the pre-G6 `fragments × n_in` scan quadratic.
fn scattered_fragments(f: usize, safe_z: f64) -> Toolpath {
    let mut tp = Toolpath::new();
    for i in 0..f {
        let t = i as f64;
        let x = 90.0 * (t * 0.9137).sin().abs() + 2.0;
        let y = 90.0 * (t * 0.4271).cos().abs() + 2.0;
        if i > 0 {
            tp.rapid_to_with_intent(P3::new(x, y, safe_z), MoveIntent::Retract);
        }
        tp.rapid_to_with_intent(P3::new(x, y, safe_z), MoveIntent::Linking);
        tp.feed_to_with_intent(P3::new(x, y, -0.5), 100.0, MoveIntent::EntryPlunge);
        for k in 1..=4 {
            let d = k as f64 * 0.4;
            tp.feed_to_with_intent(
                P3::new(x + d, y + d * 0.3, -0.5),
                500.0,
                MoveIntent::FinishingCut,
            );
        }
    }
    tp
}

fn relink_params(safe_z: f64, reorder: bool) -> RelinkParams<'static> {
    RelinkParams {
        hookup_distance: 2.0,
        stock_to_leave: 0.0,
        sampling: 0.5,
        feed_rate: 500.0,
        plunge_rate: 100.0,
        safe_z,
        link_kinematics: None,
        reorder,
        boundary: None,
    }
}

fn exponent(t1: f64, t2: f64, n1: f64, n2: f64) -> f64 {
    (t2 / t1).ln() / (n2 / n1).ln()
}

// ── G5: the tsp rapid-order seed ────────────────────────────────────────

#[test]
fn g5_tsp_rapid_order_scaling() {
    let mut times = Vec::new();
    let sizes = [5_000_usize, 20_000];
    for &n in &sizes {
        let tp = scattered_segments(n);
        // One warm-up (allocator + page faults), then the measured run.
        let _ = rs_cam_core::tsp::optimize_rapid_order(AnnotatedToolpath::new(tp.clone()), 5.0);
        let t0 = Instant::now();
        let out = rs_cam_core::tsp::optimize_rapid_order(AnnotatedToolpath::new(tp.clone()), 5.0);
        let dt = t0.elapsed().as_secs_f64();
        assert!(
            out.toolpath.moves.len() > n,
            "fixture must actually reorder"
        );
        println!("G5 tsp  n={n:>6}  {:>9.3} ms", dt * 1e3);
        times.push(dt);
    }
    let e = exponent(times[0], times[1], sizes[0] as f64, sizes[1] as f64);
    println!("G5 tsp  exponent = {e:.3}  (quadratic = 2.0)");
    assert!(
        e < 1.7,
        "rapid-order seed still looks quadratic: exponent {e:.3} over {sizes:?}"
    );
}

// ── G5 + G6: surface_link ───────────────────────────────────────────────

#[test]
fn g5_g6_surface_link_scaling() {
    let mesh = flat_plate(100.0, 40);
    let index = SpatialIndex::build(&mesh, 5.0);
    let tool = BallEndmill::new(2.0, 25.0);
    let safe_z = 10.0;
    let params = relink_params(safe_z, true);

    let mut times = Vec::new();
    let sizes = [1_500_usize, 6_000];
    for &f in &sizes {
        let tp = scattered_fragments(f, safe_z);
        let n_in = tp.moves.len();
        let run = || {
            let (t, r) = relink_fragments(
                AnnotatedToolpath::new(tp.clone()),
                &mesh,
                &index,
                &tool,
                &params,
            );
            (t.reconcile(&mut ReconcileSet::empty()).into_inner(), r)
        };
        let _ = run();
        let t0 = Instant::now();
        let (_, report) = run();
        let dt = t0.elapsed().as_secs_f64();
        assert_eq!(report.fragments, f, "fixture must produce one frag per run");
        println!(
            "G5/G6 relink  fragments={f:>5} n_in={n_in:>6}  {:>9.3} ms",
            dt * 1e3
        );
        times.push(dt);
    }
    let e = exponent(times[0], times[1], sizes[0] as f64, sizes[1] as f64);
    println!("G5/G6 relink  exponent = {e:.3}  (quadratic = 2.0)");
    assert!(
        e < 1.7,
        "relink still looks quadratic in fragments: exponent {e:.3} over {sizes:?}"
    );
}

// ── G6: the provenance channel is unchanged ─────────────────────────────

/// Byte-level identity over the whole transform: every emitted move AND
/// every input move's provenance range, through the repo's one canonical
/// FNV-1a-over-`Debug` fingerprint (`tests/common/fingerprint.rs`) rather
/// than a re-rolled hash. `Debug` on `f64` round-trips every bit, so a
/// last-ULP divergence cannot hide and `-0.0` cannot pass for `0.0`.
///
/// G6 rewrote how the junction-rapid provenance ranges are *found*, not what
/// they are, and G5 rewrote how the visiting order is *computed*, not what it
/// is. Neither may move this pair — and unlike a move count, a pinned hash
/// cannot miss a reassignment that keeps the same shape (which is exactly the
/// failure mode wave 1's G3 shipped and was caught by).
#[test]
fn g5_g6_relink_output_and_provenance_fingerprint() {
    let mesh = flat_plate(100.0, 40);
    let index = SpatialIndex::build(&mesh, 5.0);
    let tool = BallEndmill::new(2.0, 25.0);
    let safe_z = 10.0;

    let mut got: Vec<(bool, usize, u64)> = Vec::new();
    for (reorder, _, _) in RELINK_GOLDEN {
        let params = relink_params(safe_z, *reorder);
        let tp = scattered_fragments(120, safe_z);
        let (transformed, report) =
            relink_fragments(AnnotatedToolpath::new(tp), &mesh, &index, &tool, &params);
        assert_eq!(report.fragments, 120);
        let (annotated, prov) = transformed
            .reconcile(&mut ReconcileSet::empty())
            .into_parts();
        let moves = annotated.toolpath.moves.len();
        let hash = fingerprint::fnv1a_debug(&(&annotated.toolpath.moves, &prov));
        println!("G6 relink reorder={reorder} moves={moves} fnv={hash:016x}");
        got.push((*reorder, moves, hash));
    }
    // Report BOTH arms before asserting — a harness that dies on the first
    // arm hides the second one's value, which is exactly what you need when
    // capturing the pre-change side of the A/B.
    assert_eq!(
        got.as_slice(),
        RELINK_GOLDEN,
        "relink output or provenance moved"
    );
}

/// Captured from the **pre-G5/G6** tree — scoped `git stash push` of
/// `tsp.rs` / `surface_link.rs` / `lib.rs`, run, pop — so these constants are
/// the OLD code's answer, not this change's own output blessed after the
/// fact. See `planning/perf_review_2026-08-19/DELTA_gen_w3b.md`.
const RELINK_GOLDEN: &[(bool, usize, u64)] = &[
    (false, 840, 0xfd01_9db2_3752_f9ca),
    (true, 837, 0x8363_14d3_c7a9_1e97),
];
