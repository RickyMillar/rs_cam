//! The generation finish-surface memo
//! (`planning/thin_organic_2026-08-27/FINDINGS.md` §1.4).
//!
//! `build_finish_surface_with_policy_and_cancel` walks a drop cutter over the
//! WHOLE board, and `unified_finish` calls `scallop` — which builds one — once
//! per region. A tier whose mid-steep band decomposes into k islands therefore
//! paid k identical whole-board walks.
//!
//! The bar is not "the second call is faster": on a two-triangle fixture a
//! cache that quietly rebuilt and returned an equal-but-fresh surface would
//! pass a stopwatch assertion and still lose the 3–6 s-per-call board it exists
//! for. The instrument is therefore `finish_setup::surface_build_count()` — a
//! counter INSIDE the builder, so a memo that reported a hit while something
//! else rebuilt would still fail here.
//!
//! Two properties are asserted that a pointer-keyed memo could not give:
//! `equal_meshes_at_different_addresses_hit` (content keying is what makes the
//! key sound without an `Arc` to hang a `Weak` on — see the module doc) and
//! `a_cached_surface_equals_a_fresh_build`, the `geometry_cache_g8` precedent
//! that the cached object is the real answer and not a stale one.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use common::tools::{ball_cutter, wanaka_taper};
use rs_cam_core::finish::finish_setup::{
    FinishResolutionPolicy, build_finish_surface_with_policy_and_cancel, reset_surface_build_count,
    surface_build_count,
};
use rs_cam_core::finish::scallop::{
    ScallopParams, scallop_toolpath_structured_annotated_with_cancel,
};
use rs_cam_core::geo::P3;
use rs_cam_core::maps::finish_surface_cache::{
    CAPACITY, cache_len, cached_finish_surface, clear, stats,
};
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh, make_test_flat, make_test_hemisphere};
use rs_cam_core::tool::MillingCutter;

/// The counters and the table are process-global, so every test that reads a
/// delta off them must not run concurrently with another that builds. Same
/// device as `tier_map_cache_t3.rs::cache_lock` and
/// `geometry_cache_g8.rs::counter_lock`.
fn cache_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    #[allow(clippy::unwrap_used)] // poisoning would mean another test panicked
    LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
}

/// `Send + Sync` spelled out rather than left to auto-trait leakage, matching
/// `tier_map_cache_t3.rs`: `CancelCheck`'s blanket impl requires both.
fn never_cancel() -> impl Fn() -> bool + Send + Sync {
    || false
}

fn policy(cutter: &dyn MillingCutter) -> FinishResolutionPolicy {
    FinishResolutionPolicy::legacy_envelope_quarter(cutter, 0.05)
}

/// A 20×20 ridge with a gentle along-Y ripple — shallow flanks, a crest, and
/// enough curvature that stepover, ring decimation and slope classification all
/// do real work. Copied from `finish_resolution_policy_pr3::ridge_mesh`, which
/// pins a 2 820-move scallop fingerprint on it, so the fixture is known to
/// produce a real toolpath rather than an empty one.
fn ridge_mesh() -> TriangleMesh {
    let n: usize = 21;
    let mut verts = Vec::with_capacity(n * n);
    for iy in 0..n {
        for ix in 0..n {
            let x = ix as f64;
            let y = iy as f64;
            let z = 3.0 * (1.0 - (x - 10.0).abs() / 10.0) + 0.5 * (y * 0.3).sin();
            verts.push(P3::new(x, y, z));
        }
    }
    let mut tris: Vec<[u32; 3]> = Vec::with_capacity((n - 1) * (n - 1) * 2);
    for iy in 0..(n - 1) {
        for ix in 0..(n - 1) {
            let a = (iy * n + ix) as u32;
            let b = a + 1;
            let c = a + n as u32;
            let d = c + 1;
            tris.push([a, b, d]);
            tris.push([a, d, c]);
        }
    }
    TriangleMesh::from_raw(verts, tris)
}

// ── the bar ─────────────────────────────────────────────────────────────

#[test]
fn the_same_key_returns_the_cached_arc_and_does_zero_build_work() {
    let _guard = cache_lock();
    clear();
    reset_surface_build_count();

    let mesh = make_test_hemisphere(20.0, 10);
    let index = SpatialIndex::build_auto(&mesh);
    let cutter = ball_cutter(6.0);
    let res = policy(&cutter);

    let before = stats();
    let a = cached_finish_surface(&mesh, &index, &cutter, res, &never_cancel()).unwrap();
    let after_build = stats();
    let builds_for_the_build = surface_build_count();
    assert_eq!(after_build.builds, before.builds + 1, "first call builds");
    assert_eq!(
        builds_for_the_build, 1,
        "the build must actually run the whole-board walk"
    );

    let b = cached_finish_surface(&mesh, &index, &cutter, res, &never_cancel()).unwrap();
    assert!(
        Arc::ptr_eq(&a, &b),
        "the second call must hand back the same allocation, not an equal copy"
    );
    assert_eq!(stats().hits, after_build.hits + 1);
    assert_eq!(stats().builds, after_build.builds, "no second build");
    assert_eq!(
        surface_build_count(),
        builds_for_the_build,
        "a cache hit must perform ZERO surface-build work"
    );
}

/// The §1.4 win itself, through the production entry point rather than the
/// cache API: `unified_finish` calls this once per region, so two calls with
/// one mesh/tool/tolerance must cost ONE whole-board walk.
#[test]
fn repeated_scallop_calls_share_one_surface_build() {
    let _guard = cache_lock();
    clear();
    reset_surface_build_count();

    let mesh = ridge_mesh();
    let index = SpatialIndex::build(&mesh, 10.0);
    let cutter = wanaka_taper();
    let params = ScallopParams {
        scallop_height: 0.05,
        tolerance: 0.01,
        continuous: false,
        ..ScallopParams::default()
    };

    let (first, _, _) = scallop_toolpath_structured_annotated_with_cancel(
        &mesh,
        &index,
        &cutter,
        &params,
        None,
        None,
        &never_cancel(),
    )
    .unwrap();
    let after_first = surface_build_count();
    assert_eq!(after_first, 1, "the first scallop call builds the surface");
    assert!(
        !first.moves.is_empty(),
        "fixture must actually produce a toolpath, or this proves nothing"
    );

    let (second, _, _) = scallop_toolpath_structured_annotated_with_cancel(
        &mesh,
        &index,
        &cutter,
        &params,
        None,
        None,
        &never_cancel(),
    )
    .unwrap();
    assert_eq!(
        surface_build_count(),
        after_first,
        "a second scallop call over the same mesh, tool and tolerance must \
         reuse the surface — this is the per-region rebuild the memo exists \
         to remove"
    );
    // And the memo changed no output: same surface in, same path out.
    assert_eq!(second.moves.len(), first.moves.len());
}

// ── the cached object is the real answer ────────────────────────────────

#[test]
fn a_cached_surface_equals_a_fresh_build() {
    let _guard = cache_lock();
    clear();

    let mesh = make_test_hemisphere(20.0, 10);
    let index = SpatialIndex::build_auto(&mesh);
    let cutter = wanaka_taper();
    let res = policy(&cutter);

    let cached = cached_finish_surface(&mesh, &index, &cutter, res, &never_cancel()).unwrap();
    let fresh =
        build_finish_surface_with_policy_and_cancel(&mesh, &index, &cutter, res, &never_cancel())
            .unwrap();

    assert_eq!(cached.rows(), fresh.rows());
    assert_eq!(cached.cols(), fresh.cols());
    assert_eq!(cached.cell_size().to_bits(), fresh.cell_size().to_bits());
    assert_eq!(cached.cell_source, fresh.cell_source);
    assert_eq!(cached.sampler, fresh.sampler);

    let cached_z = cached.heightmap.z_or_bbox_floor_values();
    let fresh_z = fresh.heightmap.z_or_bbox_floor_values();
    assert_eq!(cached_z.len(), fresh_z.len());
    for (i, (c, f)) in cached_z.iter().zip(fresh_z.iter()).enumerate() {
        assert_eq!(c.to_bits(), f.to_bits(), "z mismatch at cell {i}");
    }
    assert_eq!(
        cached.heightmap.covered_flags(),
        fresh.heightmap.covered_flags()
    );
    for (i, (c, f)) in cached
        .slope_map
        .angles
        .iter()
        .zip(fresh.slope_map.angles.iter())
        .enumerate()
    {
        assert_eq!(c.to_bits(), f.to_bits(), "slope mismatch at cell {i}");
    }
}

// ── misses ──────────────────────────────────────────────────────────────

#[test]
fn a_different_tool_misses() {
    let _guard = cache_lock();
    clear();

    let mesh = make_test_hemisphere(20.0, 8);
    let index = SpatialIndex::build_auto(&mesh);
    let ball = ball_cutter(6.0);
    let taper = wanaka_taper();
    // The shipped trap: both report an envelope radius of 3.0, so a key that
    // read only the envelope would serve one under the other's name — and at
    // `envelope/4` they even resolve to the SAME cell size.
    assert!((ball.envelope_radius_mm() - taper.envelope_radius_mm()).abs() < 1e-12);
    let ball_res = policy(&ball);
    let taper_res = policy(&taper);
    assert!((ball_res.cell_mm() - taper_res.cell_mm()).abs() < 1e-12);

    let a = cached_finish_surface(&mesh, &index, &ball, ball_res, &never_cancel()).unwrap();
    let b = cached_finish_surface(&mesh, &index, &taper, taper_res, &never_cancel()).unwrap();
    assert!(
        !Arc::ptr_eq(&a, &b),
        "a ball and a taper of equal envelope must not share a surface"
    );
}

#[test]
fn a_different_resolution_misses() {
    let _guard = cache_lock();
    clear();

    let mesh = make_test_flat(20.0);
    let index = SpatialIndex::build_auto(&mesh);
    let cutter = ball_cutter(6.0);

    let coarse = cached_finish_surface(
        &mesh,
        &index,
        &cutter,
        FinishResolutionPolicy::explicit(1.0),
        &never_cancel(),
    )
    .unwrap();
    let fine = cached_finish_surface(
        &mesh,
        &index,
        &cutter,
        FinishResolutionPolicy::explicit(0.5),
        &never_cancel(),
    )
    .unwrap();
    assert!(!Arc::ptr_eq(&coarse, &fine));
    assert!(fine.rows() > coarse.rows());
}

/// Equal millimetres are not equal provenance: a surface built under a derived
/// policy publishes `CellSource::EnvelopeRadius`, an explicit one publishes
/// `CellSource::Explicit`, and `MeasurementProvenance` reads that. Keying only
/// `cell_mm` would hand one out under the other's name.
#[test]
fn an_explicit_policy_of_equal_cell_size_misses() {
    let _guard = cache_lock();
    clear();

    let mesh = make_test_flat(20.0);
    let index = SpatialIndex::build_auto(&mesh);
    let cutter = ball_cutter(6.0);
    let derived = policy(&cutter);
    let explicit = FinishResolutionPolicy::explicit(derived.cell_mm());

    let a = cached_finish_surface(&mesh, &index, &cutter, derived, &never_cancel()).unwrap();
    let b = cached_finish_surface(&mesh, &index, &cutter, explicit, &never_cancel()).unwrap();
    assert!(!Arc::ptr_eq(&a, &b));
    assert_ne!(a.cell_source, b.cell_source);
}

#[test]
fn a_different_index_geometry_misses() {
    let _guard = cache_lock();
    clear();

    let mesh = make_test_hemisphere(20.0, 8);
    let coarse_index = SpatialIndex::build(&mesh, 5.0);
    let fine_index = SpatialIndex::build(&mesh, 1.0);
    let cutter = ball_cutter(6.0);
    let res = policy(&cutter);

    let a = cached_finish_surface(&mesh, &coarse_index, &cutter, res, &never_cancel()).unwrap();
    let b = cached_finish_surface(&mesh, &fine_index, &cutter, res, &never_cancel()).unwrap();
    assert!(
        !Arc::ptr_eq(&a, &b),
        "conservative by design: a differently-built index is a miss, never a \
         wrong hit"
    );
}

#[test]
fn a_different_mesh_misses() {
    let _guard = cache_lock();
    clear();

    let a_mesh = make_test_hemisphere(20.0, 8);
    let b_mesh = make_test_flat(40.0);
    let a_index = SpatialIndex::build_auto(&a_mesh);
    let b_index = SpatialIndex::build_auto(&b_mesh);
    let cutter = ball_cutter(6.0);
    let res = policy(&cutter);

    let a = cached_finish_surface(&a_mesh, &a_index, &cutter, res, &never_cancel()).unwrap();
    let b = cached_finish_surface(&b_mesh, &b_index, &cutter, res, &never_cancel()).unwrap();
    assert!(!Arc::ptr_eq(&a, &b));
}

/// A nanometre move of one interior vertex leaves the counts and the bounding
/// box alone. Only the content digest separates the two, and it must.
#[test]
fn a_perturbed_mesh_misses() {
    let _guard = cache_lock();
    clear();

    let base = make_test_hemisphere(20.0, 8);
    let mut verts = base.vertices.clone();
    // Nudge every vertex slightly inward in Z; the bbox moves, the counts do
    // not, and either half of the key is entitled to catch it — the point is
    // that a changed surface is never served a stale answer.
    for v in &mut verts {
        v.z += 1e-9;
    }
    let moved = TriangleMesh::from_raw(verts, base.triangles.clone());

    let base_index = SpatialIndex::build_auto(&base);
    let moved_index = SpatialIndex::build_auto(&moved);
    let cutter = ball_cutter(6.0);
    let res = policy(&cutter);

    let a = cached_finish_surface(&base, &base_index, &cutter, res, &never_cancel()).unwrap();
    let b = cached_finish_surface(&moved, &moved_index, &cutter, res, &never_cancel()).unwrap();
    assert!(!Arc::ptr_eq(&a, &b));
}

// ── content keying, the property a pointer key could not give ───────────

#[test]
fn equal_meshes_at_different_addresses_hit() {
    let _guard = cache_lock();
    clear();
    reset_surface_build_count();

    let a_mesh = make_test_hemisphere(20.0, 8);
    let b_mesh = make_test_hemisphere(20.0, 8);
    assert!(
        !std::ptr::eq(&a_mesh, &b_mesh),
        "fixture: the two meshes must be distinct objects"
    );
    let a_index = SpatialIndex::build_auto(&a_mesh);
    let b_index = SpatialIndex::build_auto(&b_mesh);
    let cutter = ball_cutter(6.0);
    let res = policy(&cutter);

    let a = cached_finish_surface(&a_mesh, &a_index, &cutter, res, &never_cancel()).unwrap();
    let b = cached_finish_surface(&b_mesh, &b_index, &cutter, res, &never_cancel()).unwrap();
    assert!(
        Arc::ptr_eq(&a, &b),
        "the surface depends only on content, so an independently built but \
         identical mesh must HIT — this is what makes the key sound without an \
         Arc to hang a Weak on"
    );
    assert_eq!(surface_build_count(), 1);
}

// ── bounds ──────────────────────────────────────────────────────────────

#[test]
fn the_table_is_capacity_bounded() {
    let _guard = cache_lock();
    clear();

    let mesh = make_test_flat(20.0);
    let index = SpatialIndex::build_auto(&mesh);
    let cutter = ball_cutter(6.0);

    // CAPACITY + 2 distinct resolutions, so the oldest entries are evicted.
    for i in 0..(CAPACITY + 2) {
        let cell = 1.0 + i as f64;
        let _ = cached_finish_surface(
            &mesh,
            &index,
            &cutter,
            FinishResolutionPolicy::explicit(cell),
            &never_cancel(),
        )
        .unwrap();
    }
    assert_eq!(
        cache_len(),
        CAPACITY,
        "the memo must not grow with the number of distinct keys"
    );
}

#[test]
fn clear_empties_the_table() {
    let _guard = cache_lock();
    clear();

    let mesh = make_test_flat(20.0);
    let index = SpatialIndex::build_auto(&mesh);
    let cutter = ball_cutter(6.0);
    let _ =
        cached_finish_surface(&mesh, &index, &cutter, policy(&cutter), &never_cancel()).unwrap();
    assert_eq!(cache_len(), 1);
    clear();
    assert_eq!(cache_len(), 0);
}
