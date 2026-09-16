//! Bounded memo for the GENERATION finish surface —
//! [`crate::finish_setup::build_finish_surface_with_policy_and_cancel`].
//!
//! # Why this exists
//!
//! `build_finish_surface_with_policy_and_cancel` builds a **mesh-global**
//! heightmap plus slope map: one drop-cutter query per grid cell over the whole
//! board, whatever the caller is about to machine. `scallop.rs` calls it once
//! per call, and `unified_finish` calls scallop **once per region**
//! (`unified_finish.rs`'s Step 4 loop, whose own comment has flagged this since
//! P2.d: *"scallop and waterline rebuild their internal surfaces per call — O(1)
//! regions per band on wanaka today, so this duplication is accepted and
//! flagged as a P2.e datapoint if conditioned region counts ever grow"*).
//!
//! Tier islands are that growth. The multi-tool planner caps a tier at 24
//! islands (`crate::tier_islands::DEFAULT_MAX_REGIONS_PER_TIER`), so a tier
//! whose mid-steep band decomposes into k regions pays k identical whole-board
//! surface builds. `planning/thin_organic_2026-08-27/FINDINGS.md` §1.4 names
//! this the prerequisite for the contour-for-thin-regions work, because that
//! change routes *more* regions through the same generator: without this memo a
//! generation-time regression would be indistinguishable from a toolpath
//! regression in the A/B.
//!
//! Nothing in the tree cached this before. [`crate::geom_cache`] memoises the
//! spatial index, the silhouette and the setup-transformed mesh, and none of
//! its keys carry a tool or a resolution; [`crate::tier_map_cache`] carries
//! both but memoises a tier map, not a surface.
//!
//! # Why this is a module and not a `geom_cache` entry
//!
//! `geom_cache`'s entry API is keyed `&Arc<TriangleMesh>` throughout — `get`,
//! `put` and `Entry::new` all need an `Arc` to downgrade. **No call site on
//! this path has one.** `ExecutionContext::mesh` is `Option<&'a TriangleMesh>`
//! (`compute/execute.rs`), `unified_finish` receives and forwards a bare
//! `&TriangleMesh`, and `scallop`'s signature is `&TriangleMesh` — the `Arc`
//! stops at `ResolvedGenInputs` in `session/compute.rs`, several frames above.
//! Threading one down would have to pass through `unified_finish`, and the
//! per-region loop there is precisely the hot path this memo exists for.
//!
//! So the identity question had to be answered a different way; see below. That
//! answer does not fit `geom_cache`'s entry shape, and `tier_map_cache` is the
//! in-repo precedent for a memo whose key is richer than `geom_cache`'s living
//! beside it rather than inside it.
//!
//! # Why the key is sound
//!
//! **Mesh identity is by CONTENT, not by address.** A bare pointer is not a
//! sound key — `geom_cache`'s module doc argues the ABA hazard at length, and it
//! is live in a GUI session that loads, unloads and reloads models: drop a
//! mesh, allocate another of the same size class, and the allocator will very
//! plausibly hand back the same address. `geom_cache` closes that with a
//! [`std::sync::Weak`], which needs an `Arc` this path does not have. Content
//! keying closes it too, and closes it *harder*: identity-implies-content is
//! the property `geom_cache` has to argue for separately, whereas here it is
//! the key itself.
//!
//! The content key is the three element counts, the bounding box as bit
//! patterns, and a [`std::collections::hash_map::DefaultHasher`] digest of
//! `vertices` and `triangles`. `faces` is deliberately **not** hashed, and that
//! is not a shortcut: every constructor in `mesh.rs` derives `faces` from
//! `(vertices, triangles)` by the same `Triangle::new` map — four sites, checked
//! — every out-of-module producer goes through `TriangleMesh::from_raw` or an
//! STL loader, and the workspace mutates `faces` nowhere. Hashing it would cost
//! **95 MB** of digest on the 661 k-triangle reference mesh against 16 MB for
//! the two arrays that determine it. `faces.len()` *is* in the key, so a
//! hand-built literal with an inconsistent `faces` (the fields are `pub`) can
//! still not match on counts alone — that case is the stated residual risk, and
//! it has no producer today.
//!
//! Cost, since a memo that costs more than it saves is a defect: the digest is
//! ~16 MB on the reference mesh, single-digit milliseconds, against the 3–6 s
//! whole-board build it replaces. It is paid on hits as well as misses. If it
//! ever shows up in a profile the documented next step is a strided sample —
//! the "probed" half of `compute::sim_prefix::hash_tool`'s discipline — not a
//! pointer.
//!
//! **The spatial index is in the key**, by its own geometry (cell size, cell
//! counts, triangle count). A correct index is a pure accelerator, so two
//! indexes over one mesh *should* yield identical surfaces — but "should" is
//! the failure class this repo names, and the check is O(1). Keying it makes a
//! differently-built index a **miss**, never a wrong hit. Production always
//! passes `build_auto`'s index, whose geometry is a pure function of the mesh,
//! so this costs no hit rate there.
//!
//! **Every float in the key** — bbox bounds, index cell size, the tool's shape
//! dials, the resolved cell size — is compared through [`f64::to_bits`], so
//! `-0.0` and `0.0` are distinct and `NaN` is an exact bit pattern rather than
//! a value that never equals itself.
//!
//! **The whole resolution policy is in the key**, not just its millimetres.
//! [`FinishResolutionPolicy`] carries a mode and a tolerance-floor flag as well
//! as `cell_mm`, and a `FinishSurface` publishes all three (`resolution`,
//! `cell_source`). `explicit(0.75)` and a `legacy_envelope_quarter` that
//! resolves to 0.75 produce grids of identical geometry that are **not** the
//! same object: their provenance differs, and
//! `crate::measurement::MeasurementProvenance` reads it. Keying only the
//! millimetres would serve one under the other's name.
//!
//! **The sampler is not in the key** because this builder has none: it always
//! produces [`crate::finish_setup::SurfaceSampler::CutterOffset`]. The
//! CLASSIFICATION builder, which does take a sampler, is deliberately not
//! memoised here — see "What is not cached".
//!
//! # What is not cached
//!
//! The classification surface
//! (`build_classification_surface_with_sampler_and_cancel`). It is built **once
//! per op**, outside `unified_finish`'s region loop, so it is not the repeated
//! build this memo exists for; and its only production caller lives in
//! `unified_finish.rs`, which would have to change to consume a shared object.
//! Adding it later means adding a `sampler` field to the private `SurfaceKey`
//! and a second entry point — the key discipline here already anticipates it.
//!
//! # What bounds it
//!
//! [`CAPACITY`] entries, evicted oldest-first. A `FinishSurface` costs **49
//! bytes per cell** — heightmap 9 (`f64` Z + `bool` covered), slope map 40
//! (`V3` normal + `f64` angle + `f64` curvature) — the same figure
//! `tier_map_cache`'s module doc quotes. That is 3.7 MB for the shipped wanaka
//! tapered tools (envelope 3.0 mm → `envelope/4` = 0.75 mm cells over a 200 mm
//! board), but **127 MB** for a Ø1 ball (envelope 0.5 mm → 0.125 mm cells,
//! 2.6 M cells). Two is chosen for that worst case, not the typical one: the
//! working set within one op is exactly **one** key — `unified_finish` calls
//! scallop with the same tool, tolerance and policy for every region — and the
//! second slot exists so a two-tier ladder does not evict mid-op. A third is
//! not free to retain on a board that already OOMs simulation at 0.1 mm cells.
//!
//! # The instrument
//!
//! [`stats`] counts builds and hits. The bar is not "the second call is faster"
//! but "the second call does **no** surface-build work", which is read off
//! [`crate::finish_setup::surface_build_count`] — a counter inside the builder
//! itself, so a cache that reported a hit while something else rebuilt would
//! still fail. See `tests/finish_surface_cache.rs`.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex, OnceLock};

use crate::finish_setup::{
    FinishResolutionMode, FinishResolutionPolicy, FinishSurface,
    build_finish_surface_with_policy_and_cancel,
};
use crate::interrupt::{CancelCheck, Cancelled};
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::tool::MillingCutter;
use crate::tool_shape_key::ToolShapeKey;

/// Maximum number of distinct surfaces held at once. See the module doc: the
/// working set inside one operation is one, the second slot spans a tier
/// boundary, and a third is not free to retain at fine cell sizes.
pub const CAPACITY: usize = 2;

/// Bit-exact CONTENT identity of a mesh. See the module doc for why this is
/// content and not a `Weak` + address, and for why `faces` is not digested.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MeshContentKey {
    vertices: usize,
    triangles: usize,
    /// Not digested, but counted — see the module doc's residual-risk note.
    faces: usize,
    bbox: [u64; 6],
    digest: u64,
}

impl MeshContentKey {
    fn new(mesh: &TriangleMesh) -> Self {
        let mut hasher = DefaultHasher::new();
        // Lengths go into the digest as well as the struct so that a boundary
        // between the two arrays cannot be shifted without changing it.
        mesh.vertices.len().hash(&mut hasher);
        for v in &mesh.vertices {
            v.x.to_bits().hash(&mut hasher);
            v.y.to_bits().hash(&mut hasher);
            v.z.to_bits().hash(&mut hasher);
        }
        mesh.triangles.len().hash(&mut hasher);
        for tri in &mesh.triangles {
            tri.hash(&mut hasher);
        }
        let bbox = &mesh.bbox;
        Self {
            vertices: mesh.vertices.len(),
            triangles: mesh.triangles.len(),
            faces: mesh.faces.len(),
            bbox: [
                bbox.min.x.to_bits(),
                bbox.min.y.to_bits(),
                bbox.min.z.to_bits(),
                bbox.max.x.to_bits(),
                bbox.max.y.to_bits(),
                bbox.max.z.to_bits(),
            ],
            digest: hasher.finish(),
        }
    }
}

/// Bit-exact identity of a spatial index's own geometry. Conservative by
/// design: a differently-built index is a MISS, never a wrong hit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct IndexKey {
    cell_size: u64,
    cell_count_x: usize,
    cell_count_y: usize,
    total_triangles: usize,
}

impl IndexKey {
    fn new(index: &SpatialIndex) -> Self {
        Self {
            cell_size: index.cell_size().to_bits(),
            cell_count_x: index.cell_count_x(),
            cell_count_y: index.cell_count_y(),
            total_triangles: index.total_triangles(),
        }
    }
}

/// Bit-exact identity of a resolved resolution policy — all three of its
/// components, not just the millimetres. See the module doc.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ResolutionKey {
    mode: FinishResolutionMode,
    cell_mm: u64,
    tolerance_floor_applied: bool,
}

impl ResolutionKey {
    fn new(resolution: FinishResolutionPolicy) -> Self {
        Self {
            mode: resolution.mode(),
            cell_mm: resolution.cell_mm().to_bits(),
            tolerance_floor_applied: resolution.tolerance_floor_applied(),
        }
    }
}

/// Everything a generation finish surface depends on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SurfaceKey {
    mesh: MeshContentKey,
    index: IndexKey,
    tool: ToolShapeKey,
    resolution: ResolutionKey,
}

struct Entry {
    key: SurfaceKey,
    surface: Arc<FinishSurface>,
}

fn table() -> &'static Mutex<Vec<Entry>> {
    static TABLE: OnceLock<Mutex<Vec<Entry>>> = OnceLock::new();
    TABLE.get_or_init(|| Mutex::new(Vec::new()))
}

/// Cumulative counters for the memo, since process start.
///
/// Stays `pub`: `finish_surface_cache::stats` returns it, so a crate-private
/// form raises `private_interfaces` (S29, 2026-09-16).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FinishSurfaceCacheStats {
    pub builds: u64,
    pub hits: u64,
}

/// This cache's own counters. The mechanism is shared
/// ([`crate::memo::CacheCounters`]); the static is per cache, by the same
/// rule as the capacity and the key.
static COUNTERS: crate::memo::CacheCounters = crate::memo::CacheCounters::new();

/// Read the counters.
#[must_use]
pub fn stats() -> FinishSurfaceCacheStats {
    let (builds, hits) = COUNTERS.read();
    FinishSurfaceCacheStats { builds, hits }
}

/// Zero the counters. The cached surfaces themselves are untouched, so this
/// cannot change any result.
pub fn reset_stats() {
    COUNTERS.reset();
}

/// Number of live entries. Test hook for the capacity bound.
#[must_use]
pub fn cache_len() -> usize {
    table().lock().map_or(0, |t| t.len())
}

/// Drop every entry. Not needed for correctness; lets a test start from a known
/// state and lets an embedder release the retained grids eagerly.
pub fn clear() {
    if let Ok(mut t) = table().lock() {
        t.clear();
    }
}

/// [`build_finish_surface_with_policy_and_cancel`] over `mesh`, built at most
/// once per (mesh content, index geometry, cutter shape, resolution policy).
///
/// Returns the **same** `Arc` on a hit — never a recomputed-equal surface — so
/// a consumer that borrows out of it is reading the identical grid the previous
/// caller read.
///
/// The lock is held only for the lookup and the insert: the build itself runs
/// **outside** it, because a whole-board drop-cutter walk is not something to
/// serialise other threads behind. Two threads racing the same miss both build
/// and the second insert wins; the surfaces are equal by construction, so the
/// race costs one redundant build and nothing else.
///
/// # Errors
///
/// [`Cancelled`] if `cancel` fires during a build. A cache hit never inspects
/// the cancel token, because there is no work to cancel.
pub fn cached_finish_surface(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    resolution: FinishResolutionPolicy,
    cancel: &dyn CancelCheck,
) -> Result<Arc<FinishSurface>, Cancelled> {
    let key = SurfaceKey {
        mesh: MeshContentKey::new(mesh),
        index: IndexKey::new(index),
        tool: ToolShapeKey::new(cutter),
        resolution: ResolutionKey::new(resolution),
    };
    if let Some(hit) = get(&key) {
        COUNTERS.record_hit();
        return Ok(hit);
    }
    let built = Arc::new(build_finish_surface_with_policy_and_cancel(
        mesh, index, cutter, resolution, cancel,
    )?);
    COUNTERS.record_build();
    tracing::debug!(
        target: "rs_cam_core::finish_surface_cache",
        rows = built.rows(),
        cols = built.cols(),
        cell_mm = built.cell_size(),
        mode = resolution.mode().label(),
        "finish surface build"
    );
    put(key, Arc::clone(&built));
    Ok(built)
}

fn get(key: &SurfaceKey) -> Option<Arc<FinishSurface>> {
    let table = table().lock().ok()?;
    table
        .iter()
        .find(|entry| entry.key == *key)
        .map(|entry| Arc::clone(&entry.surface))
}

fn put(key: SurfaceKey, surface: Arc<FinishSurface>) {
    let Ok(mut table) = table().lock() else {
        return;
    };
    if let Some(entry) = table.iter_mut().find(|entry| entry.key == key) {
        entry.surface = surface;
        return;
    }
    while table.len() >= CAPACITY {
        table.remove(0);
    }
    table.push(Entry { key, surface });
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::{IndexKey, MeshContentKey, ResolutionKey};
    use crate::finish_setup::FinishResolutionPolicy;
    use crate::geo::P3;
    use crate::mesh::{SpatialIndex, TriangleMesh, make_test_flat, make_test_hemisphere};
    use crate::tool::BallEndmill;

    /// A 3×3 vertex grid over `[-10, 10]²` at `z = 0`, triangulated into 8
    /// faces. Vertex 4 is the CENTRE, strictly inside the bounding box on every
    /// axis — so it can be nudged in X without the bbox moving, which is what
    /// isolates the digest from the rest of the key.
    fn centre_vertex_plate() -> (Vec<P3>, Vec<[u32; 3]>) {
        let mut verts = Vec::new();
        for j in 0..3 {
            for i in 0..3 {
                verts.push(P3::new(
                    -10.0 + 10.0 * f64::from(i),
                    -10.0 + 10.0 * f64::from(j),
                    0.0,
                ));
            }
        }
        let mut tris = Vec::new();
        for j in 0..2_u32 {
            for i in 0..2_u32 {
                let a = j * 3 + i;
                tris.push([a, a + 1, a + 4]);
                tris.push([a, a + 4, a + 3]);
            }
        }
        (verts, tris)
    }

    // These exercise the KEY only — no table access — so they need no
    // serialization guard. The table-touching assertions live in
    // `tests/finish_surface_cache.rs`, which takes one.

    #[test]
    fn the_same_mesh_keys_equal() {
        let mesh = make_test_hemisphere(20.0, 8);
        assert_eq!(MeshContentKey::new(&mesh), MeshContentKey::new(&mesh));
    }

    #[test]
    fn two_equal_meshes_key_equal_even_at_different_addresses() {
        // Content keying, not identity keying: an independently built but
        // identical mesh MUST hit, because the surface depends only on
        // content. This is the property that makes the key sound without an
        // `Arc` to hang a `Weak` on.
        let a = make_test_hemisphere(20.0, 8);
        let b = make_test_hemisphere(20.0, 8);
        assert!(!std::ptr::eq(&a, &b));
        assert_eq!(MeshContentKey::new(&a), MeshContentKey::new(&b));
    }

    #[test]
    fn different_meshes_key_differently() {
        let a = make_test_hemisphere(20.0, 8);
        let b = make_test_flat(40.0);
        assert_ne!(MeshContentKey::new(&a), MeshContentKey::new(&b));
    }

    #[test]
    fn a_moved_interior_vertex_changes_the_digest() {
        // The centre vertex moves 1 nm in X: counts unchanged, bounding box
        // unchanged (asserted, not assumed), surface changed. Only the digest
        // can catch this, which is the whole reason it is in the key.
        let (verts, tris) = centre_vertex_plate();
        let base = TriangleMesh::from_raw(verts.clone(), tris.clone());
        let mut moved_verts = verts;
        moved_verts[4].x += 1e-9;
        let moved = TriangleMesh::from_raw(moved_verts, tris);

        let base_key = MeshContentKey::new(&base);
        let moved_key = MeshContentKey::new(&moved);
        assert_eq!(base_key.vertices, moved_key.vertices);
        assert_eq!(base_key.triangles, moved_key.triangles);
        assert_eq!(base_key.faces, moved_key.faces);
        assert_eq!(
            base_key.bbox, moved_key.bbox,
            "fixture: the nudged vertex must be strictly interior, or this \
             test is not exercising the digest"
        );
        assert_ne!(
            base_key.digest, moved_key.digest,
            "a nanometre vertex move must not be served a stale surface"
        );
        assert_ne!(base_key, moved_key);
    }

    #[test]
    fn a_reordered_triangle_changes_the_digest() {
        // Same vertices, same counts, same bbox — a winding flip on one face,
        // which flips that face's normal and so really is a different surface.
        let (verts, tris) = centre_vertex_plate();
        let base = TriangleMesh::from_raw(verts.clone(), tris.clone());
        let mut flipped_tris = tris;
        let first = flipped_tris[0];
        flipped_tris[0] = [first[0], first[2], first[1]];
        let flipped = TriangleMesh::from_raw(verts, flipped_tris);

        let base_key = MeshContentKey::new(&base);
        let flipped_key = MeshContentKey::new(&flipped);
        assert_eq!(base_key.bbox, flipped_key.bbox);
        assert_eq!(base_key.triangles, flipped_key.triangles);
        assert_ne!(base_key.digest, flipped_key.digest);
    }

    #[test]
    fn index_geometry_is_part_of_the_key() {
        let mesh = make_test_flat(20.0);
        let coarse = SpatialIndex::build(&mesh, 5.0);
        let fine = SpatialIndex::build(&mesh, 1.0);
        assert_ne!(IndexKey::new(&coarse), IndexKey::new(&fine));
        assert_eq!(IndexKey::new(&coarse), IndexKey::new(&coarse));
    }

    #[test]
    fn an_explicit_policy_never_collides_with_a_derived_one_of_equal_cell() {
        // A Ø6 ball's `envelope/4` is 0.75 mm. An explicit 0.75 mm policy
        // produces a grid of identical geometry but different provenance
        // (`CellSource::Explicit` vs `CellSource::EnvelopeRadius`), so the two
        // surfaces are NOT interchangeable.
        let ball = BallEndmill::new(6.0, 25.0);
        let derived = FinishResolutionPolicy::legacy_envelope_quarter(&ball, 0.05);
        let explicit = FinishResolutionPolicy::explicit(derived.cell_mm());
        assert!((derived.cell_mm() - explicit.cell_mm()).abs() < 1e-12);
        assert_ne!(
            ResolutionKey::new(derived),
            ResolutionKey::new(explicit),
            "equal millimetres are not equal provenance"
        );
    }

    #[test]
    fn a_tolerance_floored_policy_is_a_distinct_key() {
        // Same mode, same resolved cell, but the floor bound in one arm —
        // which is what `cell_source()` reports, so it must key apart.
        let fine = BallEndmill::new(0.2, 25.0);
        let floored = FinishResolutionPolicy::legacy_envelope_quarter(&fine, 0.5);
        assert!(floored.tolerance_floor_applied());
        let unfloored = FinishResolutionPolicy::explicit(floored.cell_mm());
        assert!(!unfloored.tolerance_floor_applied());
        assert_ne!(ResolutionKey::new(floored), ResolutionKey::new(unfloored));
    }
}
