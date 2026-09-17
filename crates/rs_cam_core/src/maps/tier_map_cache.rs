//! Bounded memo for [`crate::maps::tier_map::compute_tier_map`] — task T3 of the
//! multi-tool plan (`planning/multitool_2026-08-23/ORCHESTRATION_PLAN.md`
//! Phase T).
//!
//! # Why this exists
//!
//! A per-tool full-grid drop-cutter map over the wanaka board costs **≈ 8 s at
//! 0.6 mm and ≈ 31 s at 0.3 mm**, per tool
//! (`planning/multitool_2026-08-23/T1_FINDINGS.md` §1.3). A planner that shows
//! an operator a tier-map preview, takes a veto, and then emits the op chain
//! would pay that twice or more for an identical answer. Nothing in the tree
//! caches a tool-parameterised surface today: `geom_cache` memoises the
//! spatial index, the silhouette and the setup-transformed mesh, and none of
//! its keys carry a tool.
//!
//! # What is cached, and why it is the whole map
//!
//! The unit is the finished [`TierMap`], not the per-tool drop grids it was
//! derived from. That is the **smaller** of the two options by a factor of
//! roughly `n`:
//!
//! | unit | bytes/cell | wanaka @ 0.3 mm (473 k cells) |
//! |---|---|---|
//! | `TierMap` (`u8` label + `f32` reference drop) | **5** | **2.4 MB** |
//! | one `f32` drop plane per ladder tool (n = 3) | 12 | 5.7 MB |
//! | five cached `FinishSurface`s (49 B/cell, 0.15 mm) | 49 | 463 MB |
//!
//! It is also the unit consumers ask for: Phase I feeds
//! [`TierMap::tier_mask`] into the morphology, and the preview renders the
//! labels. Retaining the intermediate planes would cost more and serve
//! nobody. The one thing lost is the ability to answer a *different*
//! tolerance from a cached walk — that is a deliberate trade, and a tolerance
//! change is a key miss.
//!
//! # Why the key is sound
//!
//! Verbatim [`crate::maps::geom_cache`] discipline, for the same reasons argued at
//! length there:
//!
//! * **Mesh identity** is a [`Weak<TriangleMesh>`] upgraded and compared with
//!   [`Arc::ptr_eq`], never a bare pointer — a bare pointer has an ABA hazard
//!   (drop a mesh, allocate another at the same address, and the memo answers
//!   with the previous mesh's map), and a live `Weak` makes address reuse
//!   impossible rather than merely unlikely. Identity implies content because
//!   `TriangleMesh` has no interior mutability and the workspace hands out no
//!   `&mut TriangleMesh` behind an `Arc`.
//! * **Every float** in the key — cell size, tolerance, margin, and each
//!   tool's shape dials — is compared through [`f64::to_bits`], so `-0.0` and
//!   `0.0` are distinct and `NaN` is an exact bit pattern rather than a value
//!   that never equals itself.
//! * **Tool geometry** is keyed by `tool_shape_key::ToolShapeKey`, which is
//!   that module's subject: the shape-defining accessors the drop
//!   cutter itself reads (diameter, length, corner radius, flat-tip diameter,
//!   cusp and envelope radii) *plus the cutter's own
//!   [`crate::feeds::ToolGeometryHint`] discriminant and its dials*. The hint
//!   is what separates shapes that agree on every scalar: a Ø6 ball and the
//!   shipped Ø1-tip/Ø6-shank taper both report `radius() == 3.0`, and a key
//!   that read only the envelope would collide on them — which is exactly the
//!   radius-semantics defect class this repo has been paying down. That key
//!   lived here until [`crate::maps::finish_surface_cache`] needed the same answer;
//!   it was moved rather than copied.
//! * **[`ResidualTreatment`]** is in the key, so a slope-compensated map (T2)
//!   can never be served out of a raw map's entry.
//!
//! The spatial index is deliberately **not** keyed: `build_auto`'s cell size
//! is a pure function of the mesh, so an index for a given mesh is unique up
//! to the mesh identity already in the key.
//!
//! # What bounds it
//!
//! [`CAPACITY`] entries, evicted oldest-first, and entries whose mesh has been
//! dropped are swept on every insert. Two is deliberate: the working set is
//! "the ladder the operator is looking at, and the one they just compared it
//! against". A third distinct ladder is not free to retain — at 0.3 mm each
//! entry is single-digit megabytes, but at 0.15 mm on a bigger board it is
//! not, and this board already OOMs simulation at 0.1 mm cells.
//!
//! # The instrument
//!
//! [`stats`] counts builds and hits. The bar T3 is written against is not
//! "the second call is faster" but "the second call does **no** drop-cutter
//! work", which is read off [`crate::maps::tier_map::drop_call_count`] — see
//! `tests/tier_map_cache_t3.rs`.

use std::sync::{Arc, Mutex, OnceLock};

use crate::interrupt::CancelCheck;
use crate::maps::memo::MeshMemo;
use crate::maps::tier_map::{
    ResidualTreatment, TierLadder, TierMap, TierMapError, TierMapParams, compute_tier_map,
};
use crate::maps::tool_shape_key::ToolShapeKey;
use crate::mesh::{SpatialIndex, TriangleMesh};

/// Maximum number of distinct (mesh, ladder, params) tier maps held at once.
///
/// See the module doc: the working set is the ladder under inspection plus
/// the one it is being compared against. A project that cycles past this
/// still plans correctly — it rebuilds.
pub const CAPACITY: usize = 2;

/// Everything a tier map depends on except the mesh, which is keyed by
/// identity on the entry itself.
#[derive(Debug, Clone, PartialEq, Eq)]
struct TierMapKey {
    tools: Vec<ToolShapeKey>,
    cell_mm: u64,
    tolerance_mm: u64,
    margin_mm: u64,
    treatment: ResidualTreatment,
}

impl TierMapKey {
    fn new(ladder: &TierLadder<'_>, params: &TierMapParams) -> Self {
        let tools = ladder
            .tools()
            .iter()
            .map(|tool| ToolShapeKey::new(*tool))
            .collect();
        Self {
            tools,
            cell_mm: params.cell_mm.to_bits(),
            tolerance_mm: params.tolerance_mm.to_bits(),
            margin_mm: params.margin_mm.to_bits(),
            treatment: params.treatment,
        }
    }
}

/// The table itself is [`crate::maps::memo::MeshMemo`]: `Weak` mesh identity,
/// dead-mesh sweep and oldest-first eviction at [`CAPACITY`].
type Table = MeshMemo<TierMapKey, Arc<TierMap>, CAPACITY>;

fn table() -> &'static Mutex<Table> {
    static TABLE: OnceLock<Mutex<Table>> = OnceLock::new();
    TABLE.get_or_init(|| Mutex::new(Table::new()))
}

/// Cumulative counters for the memo, since process start.
///
/// **Test door** (FLD-04). Only [`stats`] produces it, and only
/// `crates/rs_cam_core/tests/tier_map_cache_t3.rs` reads it. No product
/// surface shows these counters, so both sit behind `test-support`.
#[cfg(feature = "test-support")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TierMapCacheStats {
    pub builds: u64,
    pub hits: u64,
}

/// This cache's own counters. The mechanism is shared
/// ([`crate::maps::memo::CacheCounters`]); the static is per cache, by the same
/// rule as the capacity and the key.
static COUNTERS: crate::maps::memo::CacheCounters = crate::maps::memo::CacheCounters::new();

/// Read the counters.
///
/// **Test door** (FLD-04). No product path reads it.
#[cfg(feature = "test-support")]
#[must_use]
pub fn stats() -> TierMapCacheStats {
    let (builds, hits) = COUNTERS.read();
    TierMapCacheStats { builds, hits }
}

/// Zero the counters. The cached values themselves are untouched, so this
/// cannot change any result.
pub fn reset_stats() {
    COUNTERS.reset();
}

/// Number of live entries. Test hook for the capacity bound.
#[must_use]
pub fn cache_len() -> usize {
    table().lock().map_or(0, |t| t.entry_count())
}

/// Drop every entry. Not needed for correctness — a stale entry is
/// unreachable once its mesh is gone — but lets a test start from a known
/// state and lets an embedder release the retained maps eagerly.
pub fn clear() {
    if let Ok(mut t) = table().lock() {
        t.clear();
    }
}

/// [`compute_tier_map`] over `mesh`, computed at most once per
/// (mesh identity, ladder, params).
///
/// The lock is held only for the lookup and the insert: the walk itself runs
/// **outside** it, because a full-board tier map is not something to
/// serialise other threads behind. Two threads racing the same miss both
/// build and the second insert wins; the maps are equal by construction, so
/// the race costs one redundant walk and nothing else.
///
/// # Errors
///
/// [`TierMapError::Cancelled`] if `cancel` fires during a build. A cache hit
/// never inspects the cancel token, because there is no work to cancel.
pub fn cached_tier_map(
    mesh: &Arc<TriangleMesh>,
    index: &SpatialIndex,
    ladder: &TierLadder<'_>,
    params: &TierMapParams,
    cancel: &(dyn CancelCheck + Sync),
) -> Result<Arc<TierMap>, TierMapError> {
    let key = TierMapKey::new(ladder, params);
    if let Some(hit) = get(mesh, &key) {
        COUNTERS.record_hit();
        return Ok(hit);
    }
    let built = Arc::new(compute_tier_map(mesh, index, ladder, params, cancel)?);
    COUNTERS.record_build();
    tracing::debug!(
        target: "rs_cam_core::maps::tier_map_cache",
        tiers = built.tier_count,
        cells = built.labels.len(),
        cell_mm = built.grid.cell_mm,
        "tier map build"
    );
    put(mesh, key, Arc::clone(&built));
    Ok(built)
}

/// The cached map for this (mesh identity, ladder, params), or `None`.
/// **Never builds one**, never inspects a cancel token, and never counts as a
/// hit or a miss in [`stats`].
///
/// For a caller whose contract forbids the walk — `plan_multitool_finishing`
/// is cheap by design — but that can say something useful when the operator
/// has already previewed. `None` there means "not measured", never "clean".
#[must_use]
pub fn peek_tier_map(
    mesh: &Arc<TriangleMesh>,
    ladder: &TierLadder<'_>,
    params: &TierMapParams,
) -> Option<Arc<TierMap>> {
    get(mesh, &TierMapKey::new(ladder, params))
}

fn get(mesh: &Arc<TriangleMesh>, key: &TierMapKey) -> Option<Arc<TierMap>> {
    table().lock().ok()?.get(mesh, key)
}

/// Insert or replace the map for this (mesh identity, key). The memo sweeps
/// entries whose mesh is gone, so a closed model's map is released at the
/// next plan rather than held for the life of the process.
fn put(mesh: &Arc<TriangleMesh>, key: TierMapKey, map: Arc<TierMap>) {
    if let Ok(mut table) = table().lock() {
        table.put(mesh, key, map);
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::TierMapKey;
    use crate::maps::tier_map::{TierLadder, TierMapParams};
    use crate::tool::{BallEndmill, MillingCutter};

    // The ball-vs-taper shape-key test moved to `tool_shape_key::tests` with
    // the struct it exercises; it was never about the tier map.

    #[test]
    fn negative_zero_margin_is_a_distinct_key() {
        let fine = BallEndmill::new(1.0, 25.0);
        let tools: [&dyn MillingCutter; 1] = [&fine];
        let ladder = TierLadder::new(&tools).unwrap();
        let pos = TierMapParams {
            margin_mm: 0.0,
            ..TierMapParams::default()
        };
        let neg = TierMapParams {
            margin_mm: -0.0,
            ..TierMapParams::default()
        };
        // Conservative by design: the two produce the same map (the walk
        // clamps the margin at zero), so this is an extra miss, never a
        // wrong hit — the direction a memo is allowed to be wrong in.
        assert_ne!(
            TierMapKey::new(&ladder, &pos),
            TierMapKey::new(&ladder, &neg),
            "bit comparison must separate -0.0 from 0.0"
        );
    }

    #[test]
    fn ladder_order_is_part_of_the_key() {
        let a = BallEndmill::new(6.0, 25.0);
        let b = BallEndmill::new(2.0, 25.0);
        let c = BallEndmill::new(1.0, 25.0);
        let long: [&dyn MillingCutter; 3] = [&a, &b, &c];
        let short: [&dyn MillingCutter; 2] = [&a, &c];
        let params = TierMapParams::default();
        let long_key = TierMapKey::new(&TierLadder::new(&long).unwrap(), &params);
        let short_key = TierMapKey::new(&TierLadder::new(&short).unwrap(), &params);
        assert_ne!(long_key.tools.len(), short_key.tools.len());
        assert_ne!(long_key, short_key);
    }
}
