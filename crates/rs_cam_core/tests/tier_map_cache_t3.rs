//! T3 — the per-(mesh, ladder, params) tier-map memo
//! (`planning/multitool_2026-08-23/ORCHESTRATION_PLAN.md` Phase T, task T3).
//!
//! The bar the plan sets is not "the second call is faster" — it is **the
//! second call does no drop-cutter work at all**. A cache that quietly rebuilt
//! and returned an equal-but-fresh map would pass a timing assertion on a
//! two-triangle fixture and fail the 8 s-per-tool board it exists for, so the
//! instrument here is `tier_map::drop_call_count()`, not a stopwatch.
//!
//! The key discipline is `geom_cache`'s: mesh identity through a `Weak` +
//! `Arc::ptr_eq` (never a bare pointer — the ABA hazard that module documents),
//! every float in the key compared by `to_bits`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use common::tools::{ball_cutter, wanaka_taper};
use rs_cam_core::maps::tier_map::{
    ResidualTreatment, TierLadder, TierMapParams, drop_call_count, reset_drop_call_count,
};
use rs_cam_core::maps::tier_map_cache::{CAPACITY, cache_len, cached_tier_map, clear, stats};
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh, make_test_flat, make_test_hemisphere};
use rs_cam_core::tool::MillingCutter;

/// The counters and the table are process-global, so every test that reads a
/// delta off them must not run concurrently with another that builds. Same
/// device as `geometry_cache_g8.rs::counter_lock`.
fn cache_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    #[allow(clippy::unwrap_used)] // poisoning would mean another test panicked
    LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
}

fn params(cell_mm: f64) -> TierMapParams {
    TierMapParams {
        cell_mm,
        tolerance_mm: 0.03,
        margin_mm: 0.5,
        treatment: ResidualTreatment::Raw,
    }
}

fn never_cancel() -> impl Fn() -> bool + Send + Sync {
    || false
}

#[test]
fn the_same_key_returns_the_cached_arc_and_does_zero_drop_work() {
    let _guard = cache_lock();
    clear();
    reset_drop_call_count();

    let mesh = Arc::new(make_test_flat(20.0));
    let index = SpatialIndex::build_auto(&mesh);
    let coarse = ball_cutter(3.0);
    let fine = ball_cutter(0.5);
    let tools: [&dyn MillingCutter; 2] = [&coarse, &fine];
    let ladder = TierLadder::new(&tools).unwrap();

    let before = stats();
    let a = cached_tier_map(&mesh, &index, &ladder, &params(0.5), &never_cancel()).unwrap();
    let after_build = stats();
    let drops_for_the_build = drop_call_count();
    assert_eq!(after_build.builds, before.builds + 1, "first call builds");
    assert!(
        drops_for_the_build > 0,
        "the build must actually run the drop cutter"
    );

    let b = cached_tier_map(&mesh, &index, &ladder, &params(0.5), &never_cancel()).unwrap();
    assert!(
        Arc::ptr_eq(&a, &b),
        "the second call must hand back the same allocation, not an equal copy"
    );
    assert_eq!(stats().hits, after_build.hits + 1);
    assert_eq!(stats().builds, after_build.builds, "no second build");
    assert_eq!(
        drop_call_count(),
        drops_for_the_build,
        "a cache hit must perform ZERO drop-cutter work"
    );
}

#[test]
fn a_different_tool_misses() {
    let _guard = cache_lock();
    clear();

    let mesh = Arc::new(make_test_flat(20.0));
    let index = SpatialIndex::build_auto(&mesh);
    let coarse = ball_cutter(3.0);
    let fine = ball_cutter(0.5);
    let finer = ball_cutter(0.4);

    let a = {
        let tools: [&dyn MillingCutter; 2] = [&coarse, &fine];
        let ladder = TierLadder::new(&tools).unwrap();
        cached_tier_map(&mesh, &index, &ladder, &params(0.5), &never_cancel()).unwrap()
    };
    let b = {
        let tools: [&dyn MillingCutter; 2] = [&coarse, &finer];
        let ladder = TierLadder::new(&tools).unwrap();
        cached_tier_map(&mesh, &index, &ladder, &params(0.5), &never_cancel()).unwrap()
    };
    assert!(!Arc::ptr_eq(&a, &b), "a Ø0.5 -> Ø0.4 swap must miss");

    // Two tools of the SAME cusp radius must still miss. The shipped
    // Ø1-tip / 7° / Ø6-shank taper and a Ø1 ball both resolve a 0.5 mm
    // feature, so a key that read only the feature scale would collide on
    // them — while their drop-cutter surfaces differ everywhere the 7° cone
    // touches. The mirror case (equal ENVELOPE radius, different shape) is
    // pinned by `tier_map_cache`'s own unit test.
    let taper = wanaka_taper();
    let equal_cusp_ball = ball_cutter(1.0);
    assert_eq!(
        taper.cusp_radius_mm(),
        equal_cusp_ball.cusp_radius_mm(),
        "fixture precondition: the two fine tools resolve the same feature"
    );
    let c = {
        let tools: [&dyn MillingCutter; 2] = [&coarse, &taper];
        let ladder = TierLadder::new(&tools).unwrap();
        cached_tier_map(&mesh, &index, &ladder, &params(0.5), &never_cancel()).unwrap()
    };
    let e = {
        let tools: [&dyn MillingCutter; 2] = [&coarse, &equal_cusp_ball];
        let ladder = TierLadder::new(&tools).unwrap();
        cached_tier_map(&mesh, &index, &ladder, &params(0.5), &never_cancel()).unwrap()
    };
    assert!(
        !Arc::ptr_eq(&c, &e),
        "same cusp radius, different shape: must miss"
    );
    assert!(!Arc::ptr_eq(&a, &c) && !Arc::ptr_eq(&b, &c));

    // And a shorter ladder over a prefix of the same tools is a different key.
    let d = {
        let tools: [&dyn MillingCutter; 1] = [&coarse];
        let ladder = TierLadder::new(&tools).unwrap();
        cached_tier_map(&mesh, &index, &ladder, &params(0.5), &never_cancel()).unwrap()
    };
    assert!(!Arc::ptr_eq(&a, &d), "a shorter ladder must miss");
}

#[test]
fn a_different_cell_size_or_tolerance_misses() {
    let _guard = cache_lock();
    clear();

    let mesh = Arc::new(make_test_flat(20.0));
    let index = SpatialIndex::build_auto(&mesh);
    let coarse = ball_cutter(3.0);
    let fine = ball_cutter(0.5);
    let tools: [&dyn MillingCutter; 2] = [&coarse, &fine];
    let ladder = TierLadder::new(&tools).unwrap();

    let a = cached_tier_map(&mesh, &index, &ladder, &params(0.5), &never_cancel()).unwrap();
    let b = cached_tier_map(&mesh, &index, &ladder, &params(0.6), &never_cancel()).unwrap();
    assert!(!Arc::ptr_eq(&a, &b), "a cell-size change must miss");
    assert_eq!(a.cell_mm, 0.5);
    assert_eq!(b.cell_mm, 0.6);

    let mut looser = params(0.5);
    looser.tolerance_mm = 0.05;
    let c = cached_tier_map(&mesh, &index, &ladder, &looser, &never_cancel()).unwrap();
    assert!(!Arc::ptr_eq(&a, &c), "a tolerance change must miss");
    assert!(cache_len() <= CAPACITY, "the table stays bounded");
}

#[test]
fn a_different_mesh_misses() {
    let _guard = cache_lock();
    clear();

    let flat = Arc::new(make_test_flat(20.0));
    let dome = Arc::new(make_test_hemisphere(8.0, 12));
    let coarse = ball_cutter(3.0);
    let fine = ball_cutter(0.5);
    let tools: [&dyn MillingCutter; 2] = [&coarse, &fine];
    let ladder = TierLadder::new(&tools).unwrap();

    let ia = SpatialIndex::build_auto(&flat);
    let ib = SpatialIndex::build_auto(&dome);
    let a = cached_tier_map(&flat, &ia, &ladder, &params(0.5), &never_cancel()).unwrap();
    let b = cached_tier_map(&dome, &ib, &ladder, &params(0.5), &never_cancel()).unwrap();
    assert!(!Arc::ptr_eq(&a, &b));

    // Re-imported identical geometry is a NEW Arc and must miss: identity is
    // the key, and a fresh allocation is a different object.
    let flat_again: Arc<TriangleMesh> = Arc::new(make_test_flat(20.0));
    let c = cached_tier_map(&flat_again, &ia, &ladder, &params(0.5), &never_cancel()).unwrap();
    assert!(!Arc::ptr_eq(&a, &c));
}

#[test]
fn capacity_bounds_the_table_and_evicts_the_oldest() {
    let _guard = cache_lock();
    clear();

    let mesh = Arc::new(make_test_flat(20.0));
    let index = SpatialIndex::build_auto(&mesh);
    let coarse = ball_cutter(3.0);
    let fine = ball_cutter(0.5);
    let tools: [&dyn MillingCutter; 2] = [&coarse, &fine];
    let ladder = TierLadder::new(&tools).unwrap();

    // One distinct key per cell size, one more than the table can hold.
    let mut first = None;
    for i in 0..=CAPACITY {
        let cell = 0.5 + i as f64 * 0.1;
        let map = cached_tier_map(&mesh, &index, &ladder, &params(cell), &never_cancel()).unwrap();
        if i == 0 {
            first = Some(map);
        }
        assert!(
            cache_len() <= CAPACITY,
            "table grew past CAPACITY at insert {i}: {}",
            cache_len()
        );
    }

    let first = first.expect("the first map was retained by the test");
    let refetched = cached_tier_map(&mesh, &index, &ladder, &params(0.5), &never_cancel()).unwrap();
    assert!(
        !Arc::ptr_eq(&first, &refetched),
        "the oldest key must have been evicted, so re-asking rebuilds"
    );
}

#[test]
fn a_dropped_mesh_releases_its_entry() {
    let _guard = cache_lock();
    clear();

    let coarse = ball_cutter(3.0);
    let fine = ball_cutter(0.5);
    let tools: [&dyn MillingCutter; 2] = [&coarse, &fine];
    let ladder = TierLadder::new(&tools).unwrap();

    {
        let mesh = Arc::new(make_test_flat(20.0));
        let index = SpatialIndex::build_auto(&mesh);
        let _ = cached_tier_map(&mesh, &index, &ladder, &params(0.5), &never_cancel()).unwrap();
        assert_eq!(cache_len(), 1);
    }
    // The sweep runs on insert, so provoke one with a second mesh.
    let other = Arc::new(make_test_flat(10.0));
    let index = SpatialIndex::build_auto(&other);
    let _ = cached_tier_map(&other, &index, &ladder, &params(0.5), &never_cancel()).unwrap();
    assert_eq!(
        cache_len(),
        1,
        "the dropped mesh's entry should have been swept, not accumulated"
    );
}
