//! Bounded memo for [`crate::maps::reach_map::compute_reach_map`] — P5.
//!
//! # Why this exists
//!
//! The reach map is a viewport overlay: it is rebuilt every time the operator
//! selects a finishing toolpath, and re-selecting the same one must cost
//! nothing. A cold build is one full-grid drop-cutter walk — seconds, not
//! milliseconds — so without a memo the overlay would re-walk the board on
//! every click.
//!
//! # Why the key is sound
//!
//! Verbatim [`crate::maps::tier_map_cache`] discipline, for the same reasons argued
//! at length there:
//!
//! * **Mesh identity** is a [`Weak<TriangleMesh>`] upgraded and compared with
//!   [`Arc::ptr_eq`], never a bare pointer — a bare pointer has an ABA hazard.
//!   Production meshes reach this through
//!   [`crate::maps::geom_cache::cached_transform`], which is itself memoised, so
//!   the same model in the same setup yields the same `Arc` and therefore
//!   hits. A re-imported or re-transformed model is a new `Arc` and misses,
//!   which is the invalidation: **there is no explicit invalidation code, and
//!   there must not be.**
//! * **Tool geometry** is keyed by [`crate::maps::tool_shape_key::ToolShapeKey`] —
//!   the shape-defining accessors plus the
//!   [`crate::feeds::ToolGeometryHint`] discriminant and its dials. A Ø6 ball
//!   and the shipped Ø1-tip/Ø6-shank taper both report `radius() == 3.0`, and
//!   a key that read only the envelope would serve one's map for the other.
//!   Editing a tool's radius therefore changes the key and misses.
//! * **Every float** in the key is compared through [`f64::to_bits`], so
//!   `-0.0` and `0.0` are distinct and `NaN` is an exact bit pattern rather
//!   than a value that never equals itself.
//!
//! The tool and model **ids** ARE in the key, and have been since the file's
//! first commit. They are display payload stamped onto the answer
//! ([`crate::maps::reach_map::ReachMap::with_ids`]), so two different ids over the
//! same mesh and the same shape describe the same reach — but a hit serves
//! the stamped answer, so leaving the ids out would hand one tool's map back
//! carrying the other tool's id. The extra miss is the safe side, and
//! `tests/reach_map_p5.rs` pins the stamped ids.
//!
//! # What bounds it
//!
//! [`CAPACITY`] entries, evicted oldest-first, with dead-mesh entries swept
//! on every insert. Four rather than the tier map's two: the working set is
//! the selected toolpath's map, the one the operator just looked at, and the
//! one an MCP `reach_map` call with a tolerance override asks for — an agent
//! probing tolerances must not evict what the viewport is drawing.

use std::sync::{Arc, Mutex, OnceLock};

use crate::interrupt::{CancelCheck, Cancelled};
use crate::maps::memo::MeshMemo;
use crate::maps::reach_map::{ReachMap, ReachMapRequest, compute_reach_map};
use crate::maps::tool_shape_key::ToolShapeKey;
use crate::mesh::TriangleMesh;

/// Maximum number of distinct (mesh, tool, params) reach maps held at once.
pub const CAPACITY: usize = 4;

/// Everything a reach map depends on except the mesh, which is keyed by
/// identity on the entry itself.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ReachMapKey {
    tool: ToolShapeKey,
    cell_mm: u64,
    tolerance_mm: u64,
    margin_mm: u64,
    tool_id: usize,
    model_id: usize,
}

impl ReachMapKey {
    fn new(request: &ReachMapRequest) -> Self {
        Self {
            tool: ToolShapeKey::new(request.cutter.as_ref()),
            cell_mm: request.params.cell_mm.to_bits(),
            tolerance_mm: request.params.tolerance_mm.to_bits(),
            margin_mm: request.params.margin_mm.to_bits(),
            tool_id: request.tool_id,
            model_id: request.model_id,
        }
    }
}

/// The table itself is [`crate::maps::memo::MeshMemo`]: `Weak` mesh identity,
/// dead-mesh sweep and oldest-first eviction at [`CAPACITY`].
type Table = MeshMemo<ReachMapKey, Arc<ReachMap>, CAPACITY>;

fn table() -> &'static Mutex<Table> {
    static TABLE: OnceLock<Mutex<Table>> = OnceLock::new();
    TABLE.get_or_init(|| Mutex::new(Table::new()))
}

/// Cumulative counters for the memo, since process start.
///
/// **Test door** (FLD-04). Only [`stats`] produces it, and only
/// `crates/rs_cam_core/tests/reach_map_p5.rs` reads it. No product surface
/// shows these counters, so both sit behind `test-support`.
#[cfg(feature = "test-support")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ReachMapCacheStats {
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
pub fn stats() -> ReachMapCacheStats {
    let (builds, hits) = COUNTERS.read();
    ReachMapCacheStats { builds, hits }
}

/// Zero the counters. The cached values themselves are untouched, so this
/// cannot change any result.
///
/// **Test door** (FLD-04/05 tail). The harnesses under
/// `crates/rs_cam_core/tests` are the only callers, so it sits behind
/// `test-support`.
#[cfg(feature = "test-support")]
pub fn reset_stats() {
    COUNTERS.reset();
}

/// Number of live entries — the capacity bound's instrument.
///
/// **Test door** (FLD-04/05 tail). The harnesses under
/// `crates/rs_cam_core/tests` are the only callers, so it sits behind
/// `test-support`.
#[cfg(feature = "test-support")]
#[must_use]
pub fn cache_len() -> usize {
    table().lock().map_or(0, |t| t.entry_count())
}

/// Drop every entry. Not needed for correctness — a stale entry is
/// unreachable once its mesh is gone — but lets a test start from a known
/// state.
///
/// **Test door** (FLD-04/05 tail). The harnesses under
/// `crates/rs_cam_core/tests` are the only callers, so it sits behind
/// `test-support`.
#[cfg(feature = "test-support")]
pub fn clear() {
    if let Ok(mut t) = table().lock() {
        t.clear();
    }
}

/// [`compute_reach_map`] over `mesh`, computed at most once per
/// (mesh identity, tool shape, params).
///
/// The lock is held only for the lookup and the insert: the walk runs
/// **outside** it, so a slow build does not serialise the GUI behind it. Two
/// threads racing the same miss both build and the second insert wins; the
/// maps are equal by construction.
///
/// # Errors
///
/// [`Cancelled`] if `cancel` fires during a build. A cache hit never inspects
/// the cancel token, because there is no work to cancel.
pub fn cached_reach_map(
    request: &ReachMapRequest,
    cancel: &(dyn CancelCheck + Sync),
) -> Result<Arc<ReachMap>, Cancelled> {
    let mesh = &request.mesh;
    let key = ReachMapKey::new(request);
    if let Some(hit) = get(mesh, &key) {
        COUNTERS.record_hit();
        return Ok(hit);
    }
    let built = Arc::new(
        compute_reach_map(
            mesh.as_ref(),
            request.index.as_ref(),
            request.cutter.as_ref(),
            &request.params,
            cancel,
        )?
        .with_ids(request.tool_id, request.model_id),
    );
    COUNTERS.record_build();
    tracing::debug!(
        target: "rs_cam_core::maps::reach_map_cache",
        cells = built.cells.len(),
        cell_mm = built.grid.cell_mm,
        tolerance_mm = built.tolerance_mm,
        "reach map build"
    );
    put(mesh, key, Arc::clone(&built));
    Ok(built)
}

fn get(mesh: &Arc<TriangleMesh>, key: &ReachMapKey) -> Option<Arc<ReachMap>> {
    table().lock().ok()?.get(mesh, key)
}

/// Insert or replace the map for this (mesh identity, key). The memo sweeps
/// entries whose mesh is gone, so a closed model's map is released at the
/// next selection rather than held for the life of the process.
fn put(mesh: &Arc<TriangleMesh>, key: ReachMapKey, map: Arc<ReachMap>) {
    if let Ok(mut table) = table().lock() {
        table.put(mesh, key, map);
    }
}
