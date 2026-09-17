//! Once-per-(model, setup) memo for the three geometry derivations that
//! generation used to redo per toolpath (`PERF_REVIEW.md` G8).
//!
//! # What is memoised
//!
//! | Function | Derivation | Cost on the reference 661 k-triangle terrain |
//! |---|---|---|
//! | [`cached_auto_index`] | [`SpatialIndex::build_auto`] | one full grid build |
//! | [`cached_silhouette`] | [`crate::geometry::boundary::model_silhouette`] at the default cell size | one full-mesh rasterisation + marching squares |
//! | [`cached_transform`] | [`SetupTransformInfo::apply_to_mesh`] | a ~111 MB deep copy, ~95 MB of it the re-derived `faces` array |
//!
//! Before this module, `ProjectSession::resolve_generation_inputs` and the
//! GUI worker's `generate_via_core` each ran the first of these inside
//! per-toolpath resolution, so an 8-operation `generate_all` built the index
//! eight times — multiplied again by every fixpoint round.
//!
//! # Why the key is sound
//!
//! The review proposes keying on `Arc::as_ptr`. **A bare pointer is not a
//! sound key**: an `Arc` can be dropped and a fresh allocation can land at the
//! same address, so a later lookup would be answered with the previous mesh's
//! index — the ABA problem, and a live one in a GUI session that loads,
//! unloads and reloads models. A stale spatial index does not fail loudly; it
//! silently mis-answers every downstream query.
//!
//! Each entry therefore holds a [`Weak<TriangleMesh>`] rather than a raw
//! pointer, and that closes the hole exactly:
//!
//! 1. A live `Weak` keeps the `Arc` *allocation* from being freed even after
//!    the last strong reference drops (only the `T` inside it is dropped), so
//!    while an entry exists **no other `Arc` can be handed that address**.
//!    Address collision between a cached entry and a different live mesh is
//!    therefore impossible, not merely unlikely.
//! 2. Lookup upgrades the `Weak` and compares with [`Arc::ptr_eq`]. A hit
//!    proves the cached entry refers to the very object being queried and that
//!    that object is still alive.
//!
//! This is strictly stronger than pairing the pointer with an element count
//! (`DELTA_viz_w2.md`'s mitigation), which narrows the collision window rather
//! than closing it. It costs one atomic increment per lookup and retains only
//! a control block — not the 111 MB of mesh — for a dropped model.
//!
//! # Why identity implies content
//!
//! Keying on object identity is only equivalent to keying on content if a
//! `TriangleMesh` cannot change behind a shared `Arc`. It cannot: the type has
//! no interior mutability, and the workspace contains **no** `Arc::get_mut` or
//! `Arc::make_mut` on an `Arc<TriangleMesh>` (the four `make_mut` sites in the
//! tree are on grids, polygon sets and cut traces). A mutation would need
//! `&mut TriangleMesh`, which an `Arc` does not hand out. Any edit that
//! introduces one must invalidate here — hence
//! `tests/geometry_cache_g8.rs::cached_index_equals_a_fresh_build`, which
//! compares a cached index cell-for-cell against a fresh build.
//!
//! # Invalidation
//!
//! There is no explicit invalidation, and none is needed, because every input
//! to every memoised derivation is in the key:
//!
//! - **Mesh content** — by identity, per the argument above. A re-import, a
//!   `fix_winding`, a units change or any other edit produces a *new*
//!   `TriangleMesh` and therefore a new `Arc`, which misses.
//! - **Cell size** — [`cached_auto_index`] memoises `build_auto` only, whose
//!   cell size is a pure function of the mesh (extent + triangle count). There
//!   is nothing left to vary. Explicit-`cell_size` builds are **not** cached;
//!   callers that pass their own resolution keep calling
//!   [`SpatialIndex::build`] directly. Likewise [`cached_silhouette`] covers
//!   only the `None` (default 0.5 mm) resolution, which is what both
//!   generation call sites request.
//! - **Setup transform** — [`cached_transform`] keys on the source mesh's
//!   identity *and* on every field of [`SetupTransformInfo`], floats compared
//!   by [`f64::to_bits`] so `-0.0` and `NaN` behave as exact bit patterns
//!   rather than by `PartialEq`.
//!
//! # What bounds it
//!
//! [`CAPACITY`] entries, one per distinct source mesh, evicted oldest-first;
//! entries whose mesh has been dropped are swept on every insert, so a closed
//! model's index is released at the next generation rather than held for the
//! life of the process. Each entry holds at most one index, one silhouette and
//! one transformed mesh, so the ceiling is a constant multiple of
//! `CAPACITY` — it cannot grow with the number of toolpaths, generations or
//! fixpoint rounds, which is the leak the naive version of this would have.
//!
//! The transformed-mesh slot is deliberately **one deep**: a project that
//! alternates two non-identity setups over one model rebuilds on each switch
//! rather than retaining two ~111 MB copies. Setups are generated
//! setup-major in practice, so the alternation is rare and the memory bound is
//! worth more than the hit rate.
//!
//! # The instrument
//!
//! [`stats`] returns cumulative build/hit counts per derivation, and every
//! *build* logs one `tracing::debug!` line under target `rs_cam_core::maps::geom_cache`.
//! An 8-operation `generate_all` over one model should print exactly one
//! `geom cache build kind=index` line; a regression that reintroduces
//! per-toolpath rebuilding shows up as eight.

use std::sync::{Arc, Mutex, OnceLock};

use crate::compute::transform::SetupTransformInfo;
use crate::maps::memo::MeshMemo;
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::polygon::Polygon2;

/// Maximum number of distinct source meshes held at once.
///
/// A project with more models than this still generates correctly — it just
/// rebuilds when it cycles past the capacity. Four covers every shipped
/// fixture (single terrain, plus the project-curve surface-model case) with
/// headroom.
pub const CAPACITY: usize = 4;

/// Bit-exact key for a setup transform. `SetupTransformInfo` carries f64
/// dimensions; comparing them with `==` would make `-0.0 == 0.0` and
/// `NaN != NaN`, neither of which is the identity relation a memo needs.
#[derive(Clone, Copy, PartialEq, Eq)]
struct TransformKey {
    face_up: crate::compute::transform::FaceUp,
    z_rotation: crate::compute::transform::ZRotation,
    dims: [u64; 6],
}

impl TransformKey {
    fn new(info: &SetupTransformInfo) -> Self {
        Self {
            face_up: info.face_up,
            z_rotation: info.z_rotation,
            dims: [
                info.stock_x.to_bits(),
                info.stock_y.to_bits(),
                info.stock_z.to_bits(),
                info.stock_origin_x.to_bits(),
                info.stock_origin_y.to_bits(),
                info.stock_origin_z.to_bits(),
            ],
        }
    }
}

/// The three memoised products of one mesh. They are built independently, so
/// an entry can hold any subset of them.
#[derive(Default)]
struct Entry {
    index: Option<Arc<SpatialIndex>>,
    silhouette: Option<Arc<Vec<Polygon2>>>,
    transformed: Option<(TransformKey, Arc<TriangleMesh>)>,
}

/// The table itself is [`crate::maps::memo::MeshMemo`]: `Weak` mesh identity (the
/// module doc above argues why that is not a raw pointer), dead-mesh sweep
/// and oldest-first eviction at [`CAPACITY`]. The key is the mesh alone.
type Table = MeshMemo<(), Entry, CAPACITY>;

fn table() -> &'static Mutex<Table> {
    static TABLE: OnceLock<Mutex<Table>> = OnceLock::new();
    TABLE.get_or_init(|| Mutex::new(Table::new()))
}

/// Cumulative counters for the memo, since process start.
///
/// **Test door** (FLD-04). Only [`stats`] produces it, and only the harness
/// `crates/rs_cam_core/tests/geometry_cache_g8.rs` reads it. No product
/// surface shows these counters, so both sit behind `test-support`.
#[cfg(feature = "test-support")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GeomCacheStats {
    pub index_builds: u64,
    pub index_hits: u64,
    pub silhouette_builds: u64,
    pub silhouette_hits: u64,
    pub transform_builds: u64,
    pub transform_hits: u64,
}

/// This cache's own counters, one pair per memoised product. The mechanism
/// is shared ([`crate::maps::memo::CacheCounters`]); the statics are per cache and,
/// here, per product.
static INDEX: crate::maps::memo::CacheCounters = crate::maps::memo::CacheCounters::new();
static SILHOUETTE: crate::maps::memo::CacheCounters = crate::maps::memo::CacheCounters::new();
static TRANSFORM: crate::maps::memo::CacheCounters = crate::maps::memo::CacheCounters::new();

/// Read the counters. This is the measurement instrument for G8: the point of
/// the change is that these `*_builds` stay at one per model across a whole
/// `generate_all` instead of rising with the toolpath count.
///
/// **Test door** (FLD-04). No product path reads it.
#[cfg(feature = "test-support")]
#[must_use]
pub fn stats() -> GeomCacheStats {
    let (index_builds, index_hits) = INDEX.read();
    let (silhouette_builds, silhouette_hits) = SILHOUETTE.read();
    let (transform_builds, transform_hits) = TRANSFORM.read();
    GeomCacheStats {
        index_builds,
        index_hits,
        silhouette_builds,
        silhouette_hits,
        transform_builds,
        transform_hits,
    }
}

/// Zero the counters. For harnesses that want a per-run delta; the cached
/// values themselves are untouched, so this cannot change any result.
pub fn reset_stats() {
    for counters in [&INDEX, &SILHOUETTE, &TRANSFORM] {
        counters.reset();
    }
}

/// Number of live entries. Test hook for the capacity bound.
#[must_use]
pub fn cache_len() -> usize {
    table().lock().map_or(0, |t| t.entry_count())
}

/// Drop every entry. Not needed for correctness — a stale entry is
/// unreachable once its mesh is gone — but lets a test start from a known
/// state and lets an embedder release the retained buffers eagerly.
pub fn clear() {
    if let Ok(mut t) = table().lock() {
        t.clear();
    }
}

/// Run `read` against the entry for `mesh`, if there is a live one.
///
/// The lock is held only for the lookup: builders run **outside** it, because
/// a full index build over a 661 k-triangle mesh is not something to serialise
/// other threads behind. Two threads racing the same miss both build, and the
/// second insert wins; the results are equal by construction, so the race
/// costs one redundant build and nothing else.
fn get<T>(mesh: &Arc<TriangleMesh>, read: impl FnOnce(&Entry) -> Option<T>) -> Option<T> {
    table().lock().ok()?.read(mesh, &(), read)
}

/// Insert or update the entry for `mesh` via `write`, evicting as needed.
///
/// The memo sweeps entries whose mesh is gone. That is what keeps a closed
/// model's index (and its transformed copy) from being retained for the life
/// of the process.
fn put(mesh: &Arc<TriangleMesh>, write: impl FnOnce(&mut Entry)) {
    if let Ok(mut table) = table().lock() {
        table.write(mesh, (), Entry::default, write);
    }
}

/// [`SpatialIndex::build_auto`] over `mesh`, built at most once per mesh.
///
/// Returns exactly what a fresh `build_auto` would return — asserted, not
/// assumed, by `tests/geometry_cache_g8.rs`.
#[must_use]
pub fn cached_auto_index(mesh: &Arc<TriangleMesh>) -> Arc<SpatialIndex> {
    if let Some(hit) = get(mesh, |entry| entry.index.clone()) {
        INDEX.record_hit();
        return hit;
    }
    let built = Arc::new(SpatialIndex::build_auto(mesh));
    INDEX.record_build();
    tracing::debug!(
        target: "rs_cam_core::maps::geom_cache",
        kind = "index",
        triangles = mesh.faces.len(),
        cells = built.cell_count(),
        cell_size = built.cell_size(),
        "geom cache build"
    );
    let stored = Arc::clone(&built);
    put(mesh, move |entry| entry.index = Some(stored));
    built
}

/// A shared spatial index that is not built until something asks for it.
///
/// WP11b moved the GUI's generation submit onto the frame loop
/// (`ProjectSession::start`), and the index build over the reference
/// 661 k-triangle terrain is the most expensive step generation makes. A
/// bare [`cached_auto_index`] call inside `start` would therefore pay that
/// build on the frame loop, on the first Generate click for a fresh mesh.
///
/// [`ResolvedGenInputs`](crate::session::ResolvedGenInputs) carries one of
/// these instead. `start` constructs it and builds nothing; the executor,
/// which runs on the worker thread, calls [`force`](Self::force).
///
/// The BUILD is still memoised by [`cached_auto_index`], so the cell here
/// only stops one handle forcing twice. A second handle over the same mesh
/// hits the per-mesh memo and shares the same `Arc<SpatialIndex>`.
///
/// One production caller forces on the frame loop by necessity: a
/// `PlannedTierRegions` boundary walks the tier map inside `start`, and the
/// tier map reads the index.
pub struct LazyIndex {
    mesh: Arc<TriangleMesh>,
    cell: OnceLock<Arc<SpatialIndex>>,
}

impl LazyIndex {
    /// The index over this mesh, built on the first call.
    ///
    /// Two threads racing the first call both run
    /// [`OnceLock::get_or_init`], which serialises them; the loser drops
    /// its answer. Both answers come from the per-mesh memo, so they are
    /// the same object in practice.
    #[must_use]
    pub fn force(&self) -> &Arc<SpatialIndex> {
        self.cell.get_or_init(|| cached_auto_index(&self.mesh))
    }
}

/// A [`LazyIndex`] over `mesh`. Builds nothing.
#[must_use]
pub fn lazy_auto_index(mesh: &Arc<TriangleMesh>) -> Arc<LazyIndex> {
    Arc::new(LazyIndex {
        mesh: Arc::clone(mesh),
        cell: OnceLock::new(),
    })
}

/// [`crate::geometry::boundary::model_silhouette`] at the default resolution, computed
/// at most once per mesh.
#[must_use]
pub fn cached_silhouette(mesh: &Arc<TriangleMesh>) -> Arc<Vec<Polygon2>> {
    if let Some(hit) = get(mesh, |entry| entry.silhouette.clone()) {
        SILHOUETTE.record_hit();
        return hit;
    }
    let built = Arc::new(crate::geometry::boundary::model_silhouette(mesh, None));
    SILHOUETTE.record_build();
    tracing::debug!(
        target: "rs_cam_core::maps::geom_cache",
        kind = "silhouette",
        triangles = mesh.faces.len(),
        polygons = built.len(),
        "geom cache build"
    );
    let stored = Arc::clone(&built);
    put(mesh, move |entry| entry.silhouette = Some(stored));
    built
}

/// [`SetupTransformInfo::apply_to_mesh`] over `mesh`, computed at most once
/// per (mesh, transform).
///
/// Returning a shared `Arc` is what makes [`cached_auto_index`] hit on a
/// non-identity setup: before this, every toolpath produced a *fresh*
/// transformed mesh, so an identity-keyed index cache would have missed every
/// time however it was keyed.
#[must_use]
pub fn cached_transform(mesh: &Arc<TriangleMesh>, info: &SetupTransformInfo) -> Arc<TriangleMesh> {
    let key = TransformKey::new(info);
    if let Some(hit) = get(mesh, |entry| {
        entry
            .transformed
            .as_ref()
            .filter(|(k, _)| *k == key)
            .map(|(_, m)| Arc::clone(m))
    }) {
        TRANSFORM.record_hit();
        return hit;
    }
    let built = Arc::new(info.apply_to_mesh(mesh));
    TRANSFORM.record_build();
    tracing::debug!(
        target: "rs_cam_core::maps::geom_cache",
        kind = "transform",
        triangles = mesh.faces.len(),
        "geom cache build"
    );
    let stored = Arc::clone(&built);
    put(mesh, move |entry| entry.transformed = Some((key, stored)));
    built
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::mesh::{make_test_flat, make_test_hemisphere};

    /// The cache is one process-global table, and libtest runs this module's
    /// tests in PARALLEL — `a_dropped_mesh_releases_its_entry` clears it and
    /// counts entries while its siblings insert. The race was latent until
    /// 2026-08-27, when new unrelated lib tests shifted the schedule enough
    /// to interleave them. Same serialization device as
    /// `tier_map_cache::tests` / `tests/geometry_cache_g8.rs`.
    fn cache_test_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        LOCK.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    #[test]
    fn second_lookup_reuses_the_same_allocation() {
        let _guard = cache_test_lock();
        let mesh = Arc::new(make_test_hemisphere(20.0, 10));
        let a = cached_auto_index(&mesh);
        let b = cached_auto_index(&mesh);
        assert!(Arc::ptr_eq(&a, &b));
    }

    #[test]
    fn distinct_meshes_get_distinct_indexes() {
        let _guard = cache_test_lock();
        let a = Arc::new(make_test_hemisphere(20.0, 10));
        let b = Arc::new(make_test_flat(40.0));
        let ia = cached_auto_index(&a);
        let ib = cached_auto_index(&b);
        assert!(!Arc::ptr_eq(&ia, &ib));
        assert_eq!(ia.total_triangles(), a.faces.len());
        assert_eq!(ib.total_triangles(), b.faces.len());
    }

    #[test]
    fn a_dropped_mesh_releases_its_entry() {
        let _guard = cache_test_lock();
        clear();
        {
            let m = Arc::new(make_test_hemisphere(20.0, 8));
            let _ = cached_auto_index(&m);
            assert_eq!(cache_len(), 1);
        }
        // The sweep runs on insert, so provoke one with a second mesh.
        let other = Arc::new(make_test_flat(10.0));
        let _ = cached_auto_index(&other);
        assert_eq!(
            cache_len(),
            1,
            "the dropped mesh's entry should have been swept, not accumulated"
        );
    }

    #[test]
    fn transform_key_distinguishes_rotations() {
        use crate::compute::transform::{FaceUp, ZRotation};
        let mesh = Arc::new(make_test_flat(20.0));
        let base = SetupTransformInfo {
            face_up: FaceUp::Top,
            z_rotation: ZRotation::Deg0,
            stock_x: 100.0,
            stock_y: 80.0,
            stock_z: 20.0,
            stock_origin_x: 0.0,
            stock_origin_y: 0.0,
            stock_origin_z: 0.0,
        };
        let rotated = SetupTransformInfo {
            z_rotation: ZRotation::Deg90,
            ..base
        };
        let a = cached_transform(&mesh, &base);
        let b = cached_transform(&mesh, &rotated);
        assert!(!Arc::ptr_eq(&a, &b), "a rotation change must miss");
        // And re-asking for the rotated one hits.
        assert!(Arc::ptr_eq(&b, &cached_transform(&mesh, &rotated)));
    }

    #[test]
    fn transform_key_distinguishes_negative_zero_origin() {
        use crate::compute::transform::{FaceUp, ZRotation};
        let pos = SetupTransformInfo {
            face_up: FaceUp::Top,
            z_rotation: ZRotation::Deg0,
            stock_x: 10.0,
            stock_y: 10.0,
            stock_z: 10.0,
            stock_origin_x: 0.0,
            stock_origin_y: 0.0,
            stock_origin_z: 0.0,
        };
        let neg = SetupTransformInfo {
            stock_origin_x: -0.0,
            ..pos
        };
        assert_ne!(
            TransformKey::new(&pos).dims,
            TransformKey::new(&neg).dims,
            "bit comparison must separate -0.0 from 0.0"
        );
    }
}
