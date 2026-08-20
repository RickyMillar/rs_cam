//! S5 — prefix memoization for the `generate_all` fixpoint ladder
//! (`PERF_REVIEW.md` S5).
//!
//! # The finding
//!
//! `generate_all(fixpoint: true)` runs a simulation between generate rounds so
//! that `StockSource::FromRemainingStock` operations can see the stock the ops
//! ahead of them left. Every one of those simulations starts from a fresh
//! dexel grid and replays the **whole** project, so a chain of `k` rest ops
//! costs `O(k²)` op-simulations even though toolpaths `1..j-1` are
//! byte-identical between rounds.
//!
//! On the reference wanaka200 workload (`BASELINES.md` 0C) the ladder is 3
//! rounds / 2 simulations at 0.4 mm, and a standalone `run_simulation` at that
//! resolution is 84 s — so the second round re-does most of a minute and a half
//! of stamping, marching cubes and checkpoint cloning that it already has the
//! answer to.
//!
//! # What is memoized
//!
//! One snapshot of every loop-carried accumulator in
//! [`crate::compute::simulate::run_simulation_with_phase`], taken immediately
//! after the request's **last** toolpath entry has carved and *before* that
//! group's end-of-group work (column deviations, composite mesh, tail phantom
//! snapshot). The next simulation whose group/entry sequence starts with the
//! recorded one restores that state verbatim and continues from the first new
//! entry.
//!
//! Resume is **bit-identical by construction, not by argument**: nothing is
//! recomputed or approximated. The restored accumulators are the exact values
//! the previous run produced, and the per-entry code path that runs afterwards
//! is unchanged. The only thing the cache decides is *whether* the earlier
//! entries run again.
//!
//! The one place that needed explicit care is
//! [`crate::compute::simulate::SimGroupEntry::phantom_prior_stock`], which
//! moves between rounds and therefore must **not** be restored — see
//! "Phantom prior stock" below.
//!
//! # Why the key is sound
//!
//! The key is the closure of every input the memoized prefix depends on:
//!
//! | Input | How it is keyed |
//! |---|---|
//! | stock bbox, `stock_top_z`, resolution | `f64::to_bits`, in the global scalar |
//! | metric options (`enabled`, `capture_arc_engagement`) | global scalar |
//! | `spindle_rpm`, `rapid_feed_mm_min` | global scalar |
//! | reference model mesh | `Weak<TriangleMesh>` identity (it drives per-group column deviations) |
//! | group order + count | position in the key vector |
//! | per-group `local_stock_bbox`, `local_to_global`, `direction` | group scalar, floats by bits |
//! | per-entry toolpath geometry, spans, move intents | `Weak<AnnotatedToolpath>` identity |
//! | per-entry semantic trace | `Weak<ToolpathSemanticTrace>` identity |
//! | per-entry analytic drill op | `Weak<DrillOp>` identity |
//! | per-entry cutter shape + assembly | [`hash_tool`] — parametric *and* probed |
//! | per-entry id, name, tool summary, flute count, rpm override, flags, op-config hash | entry scalar |
//! | `phantom_prior_stock` | deliberately **excluded**; re-derived (below) |
//! | `kinematics` | deliberately **excluded**; consumed only after the loop, by `apply_kinematics_cycle_time` |
//! | `TriDexelStock::stamp_dispatch` | deliberately **excluded**; a process constant — see below |
//!
//! ## The stamp dispatch is excluded, and that is a precondition, not a proof
//!
//! Since SIM w5b the dispatch shape **changes what a carve produces**:
//! `StampDispatch::Swept` (what `Auto` now resolves to) reports a different
//! `removed_volume_est_mm3`, air-cut fraction and axial DOC than
//! `WholeToolpath` does, and leaves a slightly different grid. A prefix carved
//! under one shape and resumed under another would be exactly the key-closure
//! defect this module exists to avoid.
//!
//! It cannot happen, for two reasons that are worth stating separately because
//! either one changing re-opens the question:
//!
//! 1. **The mode is a process constant.** `TriDexelStock::from_bounds` — the
//!    only constructor, and the one every simulation path goes through —
//!    stamps `StampDispatch::default()`, which resolves an `RS_CAM_STAMP_DISPATCH`
//!    read held in a `OnceLock`. No production code assigns `stamp_dispatch`;
//!    only benches and the S1/S3 sentries do, and they build their own stocks.
//!    So within one process every carve used the same kernel.
//! 2. **The cache is in-process and single-slot.** There is no on-disk form and
//!    no cross-process form, so a snapshot cannot reach a run with a different
//!    environment. `take_match` also removes on lookup, hit or miss.
//!
//! Note that `Clone` **preserves** `stamp_dispatch`, so a restored snapshot
//! carries the mode it was carved under rather than re-deriving it. Under (1)
//! that is a no-op. If a `SimulationRequest`-level or per-toolpath dispatch
//! setting is ever added, (1) fails and this row must move into the key —
//! `tests/sim_prefix_memo_s5.rs::prefix_key_may_omit_stamp_dispatch_only_while_it_is_a_process_constant`
//! pins the precondition so that lands as a red test rather than as a wrong
//! resumed metric.
//!
//! **Pointer keys are `Weak`, never bare pointers.** A bare `Arc::as_ptr` key
//! is ABA-unsound — an `Arc` can be dropped and a fresh allocation can land at
//! the same address (`geom_cache.rs` module doc; `DELTA_gen_w4.md` G8
//! correction, where address reuse was *observed* on this machine). A live
//! `Weak` keeps the allocation reserved, so no other `Arc` can be handed that
//! address while the entry exists, and lookup upgrades and compares with
//! `Arc::ptr_eq`.
//!
//! Identity implies content for all three pointer-keyed types for the same
//! reason it does in `geom_cache`: none has interior mutability, and an edit
//! would need `&mut T`, which an `Arc` does not hand out. A regenerated
//! toolpath produces a *new* `Arc<AnnotatedToolpath>`, which misses.
//!
//! ## The tool is the one input with no stable identity
//!
//! `SimToolpathEntry::tool` is a `ToolDefinition` **by value**, rebuilt from
//! the session's `Tool` on every request, so there is no `Arc` to pin. Its
//! `cutter: Box<dyn MillingCutter>` is not `Serialize` and the trait carries no
//! `Any` bound (the same wall `DELTA_gen_w4.md` hit when it declined full
//! devirtualization), so the key is built from the trait's own observable
//! surface instead — see [`hash_tool`] for exactly what, and
//! `tests/sim_prefix_memo_s5.rs::tool_key_separates_every_shipped_shape` for
//! the net that keeps it honest.
//!
//! Note that the *existing* provenance `tool_hashes`
//! (`simulate.rs::build_simulation_provenance`) would **not** have been a sound
//! key: it hashes diameter, length, shank, holder, stickout and flute count and
//! carries **no cutter shape at all**, so a Ø6 flat and a Ø6 ball of the same
//! length hash equal. The review's "provenance hashes already exist" is right
//! that they exist and wrong that they close the input set.
//!
//! # Phantom prior stock
//!
//! `phantom_prior_stock` records the one position at which a *not yet
//! generated* `FromRemainingStock` op gets a `prior_stocks` snapshot. It moves
//! down the group on every fixpoint round — which is precisely the thing that
//! changes between the runs this cache is built to connect — so putting it in
//! the key would make the cache never hit.
//!
//! Instead the snapshot stores `prior_stocks` **restricted to the ids of the
//! entries actually replayed**, and the phantom entry is re-derived from the
//! *current* request after restore. Two cases:
//!
//! * `pk < replayed` — the phantom shares its `Arc` with
//!   `prior_stocks[toolpaths[pk].id]`, exactly as the live path does
//!   (`simulate.rs`, F.4 `Arc::clone(&pre_carve_stock)`), so it is recovered
//!   from the restored map.
//! * `pk >= replayed` — the position has not been reached yet and the normal
//!   loop inserts it.
//!
//! The one case that cannot be recovered is `pk == toolpaths.len()` on a group
//! *strictly before* the resume group: that snapshot is the group's fully
//! carved stock, which the run drops when it moves to the next group. The cache
//! **refuses** rather than approximating it. It cannot arise in the shape this
//! optimisation targets (a single-setup project resumes inside its only group),
//! and a missed hit is a lost optimisation, not a wrong answer.
//!
//! # What bounds the memory
//!
//! * **At most one snapshot.** There is no table and no eviction ordering to
//!   get wrong.
//! * **Lookup takes.** `take_match` removes the snapshot whether it hits or
//!   misses, so a stale one is freed at the next simulation rather than held.
//!   A caller that wants the memo to persist across rounds re-stores at the end
//!   of the run; an ordinary user simulation does not, so the cache is empty
//!   again afterwards.
//! * **Checkpoints are `Arc`-shared with the result** they came from
//!   (`SimulationResult::checkpoints` is `Vec<Arc<SimCheckpointMesh>>`). Those
//!   are the heavy items — a marching-cubes mesh plus a full grid clone, per
//!   toolpath — and the snapshot adds a refcount, not a copy.
//! * **A hard size ceiling.** [`SimPrefixCache::max_bytes`] (default
//!   [`DEFAULT_MAX_BYTES`]) is checked against an estimate of what the snapshot
//!   actually owns; over it, the snapshot is dropped and `size_refusals` ticks.
//! * **Explicit `clear()`** for the owner to drop it at a known point (the GUI
//!   calls it when the fixpoint ladder settles).
//!
//! The residual copies are the prefix's `cut_samples` (the dominant one), the
//! group and global dexel grids, and the composite mesh. See
//! `DELTA_sim_w3.md` for measured figures.
//!
//! # The instrument
//!
//! [`SimPrefixCache::stats`] counts lookups, hits, reused entries, stored
//! snapshots and both refusal reasons. **A memoization that never fires is a
//! silent no-op** — the S2 lesson from `DELTA_sim_w2.md` §2e, where a stale mip
//! was sound, fired zero times and left the whole suite green. The sentries in
//! `tests/sim_prefix_memo_s5.rs` assert `hits > 0` and `entries_reused > 0`
//! wherever a hit is expected, so an inert cache fails red.

use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Weak};

use crate::collision::RapidCollision;
use crate::compute::simulate::{
    ColumnDeviation, SimBoundary, SimCheckpointMesh, SimGroupEntry, SimToolpathEntry,
    SimulationRequest,
};
use crate::dexel_stock::TriDexelStock;
use crate::drill_op::DrillOp;
use crate::ids::ToolpathId;
use crate::mesh::TriangleMesh;
use crate::semantic_trace::ToolpathSemanticTrace;
use crate::simulation_cut::SimulationCutSample;
use crate::stock_mesh::StockMesh;
use crate::tool::{EngagementMode, MillingCutter, ToolDefinition};
use crate::toolpath_spans::AnnotatedToolpath;

/// Default ceiling on what one snapshot may own, in bytes.
///
/// Sized so a wanaka-scale project (600 k cut samples, ~250 k-cell grids)
/// fits with headroom while a pathological one is refused rather than
/// doubling the process's footprint. `Arc`-shared checkpoints are counted at
/// their refcount cost, not their full size, because that is what the
/// snapshot actually adds.
pub const DEFAULT_MAX_BYTES: usize = 1_500_000_000;

/// How many points the tool key probes the radial profile at.
const PROFILE_PROBES: usize = 64;

// ── Stats ───────────────────────────────────────────────────────────────

/// Cumulative counters for one [`SimPrefixCache`].
///
/// `hits` and `entries_reused` are the non-vacuity instrument: a memo that
/// never hits is indistinguishable from no memo at all except in the numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SimPrefixStats {
    /// Simulations that consulted the cache.
    pub lookups: u64,
    /// Lookups that found a usable prefix.
    pub hits: u64,
    /// Total toolpath entries skipped across all hits.
    pub entries_reused: u64,
    /// Snapshots accepted into the cache.
    pub snapshots_stored: u64,
    /// Snapshots dropped because they exceeded [`SimPrefixCache::max_bytes`].
    pub size_refusals: u64,
    /// Lookups refused because a group before the resume point carried a
    /// tail-position phantom snapshot (see the module doc).
    pub phantom_refusals: u64,
    /// Estimated bytes owned by the currently held snapshot.
    pub held_bytes: usize,
}

// ── Keys ────────────────────────────────────────────────────────────────

/// Identity key for one simulated toolpath entry.
struct EntryKey {
    scalar: u64,
    annotated: Weak<AnnotatedToolpath>,
    semantic_trace: Option<Weak<ToolpathSemanticTrace>>,
    drill_op: Option<Weak<DrillOp>>,
}

fn weak_matches<T>(stored: Option<&Weak<T>>, live: Option<&Arc<T>>) -> bool {
    match (stored, live) {
        (None, None) => true,
        (Some(w), Some(a)) => w.upgrade().is_some_and(|up| Arc::ptr_eq(&up, a)),
        _ => false,
    }
}

impl EntryKey {
    fn new(entry: &SimToolpathEntry) -> Self {
        Self {
            scalar: hash_entry_scalar(entry),
            annotated: Arc::downgrade(&entry.annotated),
            semantic_trace: entry.semantic_trace.as_ref().map(Arc::downgrade),
            drill_op: entry.drill_op.as_ref().map(Arc::downgrade),
        }
    }

    fn matches(&self, entry: &SimToolpathEntry) -> bool {
        self.scalar == hash_entry_scalar(entry)
            && weak_matches(Some(&self.annotated), Some(&entry.annotated))
            && weak_matches(self.semantic_trace.as_ref(), entry.semantic_trace.as_ref())
            && weak_matches(self.drill_op.as_ref(), entry.drill_op.as_ref())
    }
}

/// Identity key for one simulated setup group.
struct GroupKey {
    scalar: u64,
    entries: Vec<EntryKey>,
    /// `true` for a group the snapshot replayed in FULL — its end-of-group
    /// work (column deviations, composite-mesh append) is baked into the
    /// restored state, so it must match on the exact entry count, not merely
    /// on a prefix.
    ///
    /// Getting this wrong is not cosmetic. With a `>=` test on a complete
    /// group, a project that *gained* a toolpath in an earlier setup would
    /// resume past it: the group's composited mesh and deviations came from a
    /// stock the new op never carved, and the op would simply never be
    /// simulated. The degenerate form is worse still — an empty group keys as
    /// zero entries and would then match a group that has since grown any
    /// number of them.
    exact: bool,
}

impl GroupKey {
    /// Key covering `entry_count` leading entries of `group`. `exact` marks a
    /// fully replayed group (see the field doc).
    fn new(group: &SimGroupEntry, entry_count: usize, exact: bool) -> Self {
        Self {
            scalar: hash_group_scalar(group),
            entries: group
                .toolpaths
                .iter()
                .take(entry_count)
                .map(EntryKey::new)
                .collect(),
            exact,
        }
    }

    /// True when `group`'s leading entries reproduce this key.
    fn is_prefix_of(&self, group: &SimGroupEntry) -> bool {
        let count_ok = if self.exact {
            group.toolpaths.len() == self.entries.len()
        } else {
            group.toolpaths.len() >= self.entries.len()
        };
        self.scalar == hash_group_scalar(group)
            && count_ok
            && self
                .entries
                .iter()
                .zip(group.toolpaths.iter())
                .all(|(key, entry)| key.matches(entry))
    }
}

fn hash_f64_bits(hasher: &mut DefaultHasher, value: f64) {
    value.to_bits().hash(hasher);
}

/// Hash the observable surface of a cutter assembly.
///
/// There are two independent halves and they are deliberately redundant.
///
/// 1. **Parametric.** `geometry_hint()` is a complete parameterisation of every
///    shipped shape (Flat / Ball / Bull{corner_radius} / VBit{angle, tip} /
///    TaperedBall{tip_radius, taper_angle}), and the scalar accessors pin the
///    scales the simulator reads directly (`radius` for the stamp footprint,
///    `length` for flute-length normalisation, `helix_deg` and
///    `corner_radius_mm` for the chip model).
/// 2. **Probed.** `height_at_radius` sampled at [`PROFILE_PROBES`] radii is the
///    profile `RadialProfileLUT::from_cutter` is built from — i.e. the exact
///    function the stamp kernel consumes — and `engagement_radius_mm` /
///    `chip_geometry` probes cover the depth-dependent width and the chip model
///    that per-sample metrics read.
///
/// A new cutter shape whose behaviour is not a function of the hint plus these
/// probes would need this extended. That is the residual risk and it is stated
/// rather than hidden: the probe half exists so such a shape has to differ at
/// *none* of 64 profile points, 9 engagement depths and 6 chip-model
/// evaluations to collide.
fn hash_tool(hasher: &mut DefaultHasher, tool: &ToolDefinition) {
    for value in [
        tool.diameter(),
        tool.radius(),
        tool.envelope_radius_mm(),
        tool.cusp_radius(),
        tool.length(),
        tool.helix_deg(),
        tool.corner_radius_mm(),
        tool.shank_diameter,
        tool.shank_length,
        tool.holder_diameter,
        tool.stickout,
    ] {
        hash_f64_bits(hasher, value);
    }
    tool.flute_count.hash(hasher);
    format!("{:?}", tool.tool_material).hash(hasher);
    format!("{:?}", tool.geometry_hint()).hash(hasher);

    let radius = tool.radius();
    let length = tool.length();
    for i in 0..=PROFILE_PROBES {
        let r = radius * (i as f64) / (PROFILE_PROBES as f64);
        match tool.height_at_radius(r) {
            Some(h) => {
                1_u8.hash(hasher);
                hash_f64_bits(hasher, h);
            }
            None => 0_u8.hash(hasher),
        }
    }
    for i in 0..=8 {
        let depth = length * (i as f64) / 8.0;
        hash_f64_bits(hasher, tool.engagement_radius_mm(depth));
    }
    for (doc, arc, fpt) in [
        (0.5_f64, 0.3_f64, 0.05_f64),
        (1.0, 1.0, 0.1),
        (2.0, 2.0, 0.2),
        (4.0, 3.0, 0.05),
        (0.1, 0.05, 0.01),
        (8.0, std::f64::consts::PI, 0.3),
    ] {
        match tool.chip_geometry(doc, arc, fpt, tool.flute_count.max(1), EngagementMode::Slot) {
            Ok(g) => {
                1_u8.hash(hasher);
                hash_f64_bits(hasher, g.max_chip_thickness_mm);
                hash_f64_bits(hasher, g.mean_chip_thickness_mm);
                hash_f64_bits(hasher, g.edge_engagement_length_mm);
                hash_f64_bits(hasher, g.instantaneous_flutes_in_cut);
            }
            Err(e) => {
                0_u8.hash(hasher);
                format!("{e:?}").hash(hasher);
            }
        }
    }
}

fn hash_entry_scalar(entry: &SimToolpathEntry) -> u64 {
    let mut hasher = DefaultHasher::new();
    entry.id.hash(&mut hasher);
    entry.name.hash(&mut hasher);
    entry.tool_summary.hash(&mut hasher);
    entry.flute_count.hash(&mut hasher);
    entry.spindle_rpm.hash(&mut hasher);
    entry.metrics_not_applicable.hash(&mut hasher);
    entry.operation_config_hash.hash(&mut hasher);
    hash_tool(&mut hasher, &entry.tool);
    hasher.finish()
}

fn hash_group_scalar(group: &SimGroupEntry) -> u64 {
    let mut hasher = DefaultHasher::new();
    format!("{:?}", group.direction).hash(&mut hasher);
    match group.local_stock_bbox.as_ref() {
        Some(bbox) => {
            1_u8.hash(&mut hasher);
            for value in [
                bbox.min.x, bbox.min.y, bbox.min.z, bbox.max.x, bbox.max.y, bbox.max.z,
            ] {
                hash_f64_bits(&mut hasher, value);
            }
        }
        None => 0_u8.hash(&mut hasher),
    }
    match group.local_to_global.as_ref() {
        Some(info) => {
            1_u8.hash(&mut hasher);
            info.face_up.hash(&mut hasher);
            info.z_rotation.hash(&mut hasher);
            for value in [
                info.stock_x,
                info.stock_y,
                info.stock_z,
                info.stock_origin_x,
                info.stock_origin_y,
                info.stock_origin_z,
            ] {
                hash_f64_bits(&mut hasher, value);
            }
        }
        None => 0_u8.hash(&mut hasher),
    }
    hasher.finish()
}

fn hash_global_scalar(request: &SimulationRequest) -> u64 {
    let mut hasher = DefaultHasher::new();
    for value in [
        request.stock_bbox.min.x,
        request.stock_bbox.min.y,
        request.stock_bbox.min.z,
        request.stock_bbox.max.x,
        request.stock_bbox.max.y,
        request.stock_bbox.max.z,
        request.stock_top_z,
        request.resolution,
        request.rapid_feed_mm_min,
    ] {
        hash_f64_bits(&mut hasher, value);
    }
    request.spindle_rpm.hash(&mut hasher);
    request.metric_options.enabled.hash(&mut hasher);
    request
        .metric_options
        .capture_arc_engagement
        .hash(&mut hasher);
    request.groups.len().hash(&mut hasher);
    hasher.finish()
}

// ── Snapshot state ──────────────────────────────────────────────────────

/// Every loop-carried accumulator of
/// [`crate::compute::simulate::run_simulation_with_phase`], captured at a
/// toolpath boundary.
///
/// This struct is the whole correctness argument for S5: if a value survives
/// across toolpath iterations in that function, it is a field here. Adding a
/// new accumulator there without adding it here would silently produce a
/// resumed run that differs from a full replay — which is what
/// `tests/sim_prefix_memo_s5.rs::resumed_run_is_bit_identical_to_a_full_replay`
/// exists to catch, since it fingerprints the entire `SimulationResult`.
#[derive(Clone)]
pub(crate) struct PrefixState {
    pub(crate) total_moves: usize,
    pub(crate) boundary_index: usize,
    pub(crate) boundaries: Vec<SimBoundary>,
    pub(crate) checkpoints: Vec<Arc<SimCheckpointMesh>>,
    pub(crate) cut_samples: Vec<SimulationCutSample>,
    pub(crate) drill_samples_all: Vec<crate::drill_metrics::DrillSample>,
    pub(crate) drill_summaries_all: Vec<crate::drill_metrics::DrillToolpathSummary>,
    pub(crate) composite_mesh: StockMesh,
    pub(crate) global_stock: TriDexelStock,
    pub(crate) column_deviations: Option<Vec<ColumnDeviation>>,
    pub(crate) rapid_collisions: Vec<RapidCollision>,
    pub(crate) rapid_collision_move_indices: Vec<usize>,
    /// Restricted to the ids of the entries actually replayed — phantom
    /// entries are re-derived from the live request, never restored.
    pub(crate) prior_stocks: HashMap<ToolpathId, Arc<TriDexelStock>>,
    pub(crate) global_drill_ops: Vec<DrillOp>,
    /// State of the group the resume point sits inside. `None` only on the
    /// cold-start value, where the group loop allocates its own.
    pub(crate) group_stock: Option<TriDexelStock>,
    pub(crate) group_drill_ops: Vec<Arc<DrillOp>>,
}

impl PrefixState {
    /// Rough owned-byte estimate, used only to enforce the size ceiling.
    ///
    /// Counts what the snapshot *adds*: `Arc`-shared checkpoints and prior
    /// stocks are counted at pointer cost because the result they came from
    /// already owns them.
    fn estimated_bytes(&self) -> usize {
        let grid = |stock: &TriDexelStock| stock.z_grid.rays.len() * 32;
        self.cut_samples.len() * std::mem::size_of::<SimulationCutSample>()
            + self.cut_samples.len() * 16
            + self.drill_samples_all.len()
                * std::mem::size_of::<crate::drill_metrics::DrillSample>()
            + self.composite_mesh.vertices.len() * 4
            + self.composite_mesh.indices.len() * 4
            + grid(&self.global_stock)
            + self.group_stock.as_ref().map_or(0, grid)
            + self.checkpoints.len() * std::mem::size_of::<usize>()
            + self.prior_stocks.len() * std::mem::size_of::<usize>()
            + self.rapid_collision_move_indices.len() * std::mem::size_of::<usize>()
            + self
                .column_deviations
                .as_ref()
                .map_or(0, |d| d.len() * std::mem::size_of::<ColumnDeviation>())
    }
}

/// One cached prefix: the keys that identify it plus the state it carries.
pub(crate) struct SimPrefixSnapshot {
    global_key: u64,
    model_mesh: Option<Weak<TriangleMesh>>,
    /// Groups `0..resume_group` keyed in full; group `resume_group` keyed over
    /// its first `resume_entry` entries.
    groups: Vec<GroupKey>,
    resume_group: usize,
    resume_entry: usize,
    state: PrefixState,
}

/// A restored prefix, ready to continue from.
pub(crate) struct ResumedPrefix {
    pub(crate) resume_group: usize,
    pub(crate) resume_entry: usize,
    pub(crate) state: PrefixState,
}

// ── Cache ───────────────────────────────────────────────────────────────

/// A single-slot memo of one simulation prefix.
///
/// See the module doc for the key closure, the phantom rule and the memory
/// bound. Not `Sync`-shared internally: the owner (the GUI's analysis lane)
/// serialises simulations, so a plain `&mut` is the whole synchronisation
/// story.
pub struct SimPrefixCache {
    snapshot: Option<SimPrefixSnapshot>,
    stats: SimPrefixStats,
    /// Ceiling on one snapshot's estimated owned bytes.
    pub max_bytes: usize,
}

impl Default for SimPrefixCache {
    fn default() -> Self {
        Self::new()
    }
}

impl SimPrefixCache {
    #[must_use]
    pub fn new() -> Self {
        Self {
            snapshot: None,
            stats: SimPrefixStats::default(),
            max_bytes: DEFAULT_MAX_BYTES,
        }
    }

    /// Drop the held snapshot. Counters are cumulative and are not reset.
    pub fn clear(&mut self) {
        self.snapshot = None;
        self.stats.held_bytes = 0;
    }

    /// True while a snapshot is held.
    #[must_use]
    pub fn is_populated(&self) -> bool {
        self.snapshot.is_some()
    }

    #[must_use]
    pub fn stats(&self) -> SimPrefixStats {
        self.stats
    }

    /// Take the held snapshot, returning it only when it is a usable prefix of
    /// `request`.
    ///
    /// The snapshot is removed either way: a miss means it is stale, and
    /// holding a stale snapshot only costs memory.
    pub(crate) fn take_match(&mut self, request: &SimulationRequest) -> Option<ResumedPrefix> {
        self.stats.lookups += 1;
        let snapshot = self.snapshot.take()?;
        self.stats.held_bytes = 0;

        if snapshot.global_key != hash_global_scalar(request) {
            return None;
        }
        if !weak_matches(snapshot.model_mesh.as_ref(), request.model_mesh.as_ref()) {
            return None;
        }
        if request.groups.len() < snapshot.groups.len() {
            return None;
        }
        for (key, group) in snapshot.groups.iter().zip(request.groups.iter()) {
            if !key.is_prefix_of(group) {
                return None;
            }
        }
        // Every group before the resume group is replayed in full, so its
        // group-end tail phantom snapshot (`phantom_k == toolpaths.len()`) is
        // the fully carved group stock — which the run drops when it moves on.
        // Refuse rather than approximate; see the module doc.
        for group in request.groups.iter().take(snapshot.resume_group) {
            if let Some((phantom_k, _)) = group.phantom_prior_stock
                && phantom_k >= group.toolpaths.len()
            {
                self.stats.phantom_refusals += 1;
                return None;
            }
        }

        self.stats.hits += 1;
        let reused: usize = snapshot
            .groups
            .iter()
            .map(|group| group.entries.len())
            .sum();
        self.stats.entries_reused += reused as u64;
        tracing::debug!(
            target: "rs_cam_core::sim_prefix",
            resume_group = snapshot.resume_group,
            resume_entry = snapshot.resume_entry,
            entries_reused = reused,
            "sim prefix cache hit"
        );
        Some(ResumedPrefix {
            resume_group: snapshot.resume_group,
            resume_entry: snapshot.resume_entry,
            state: snapshot.state,
        })
    }

    /// Offer a freshly captured prefix. Rejected (and dropped) when it exceeds
    /// [`Self::max_bytes`].
    pub(crate) fn store(
        &mut self,
        request: &SimulationRequest,
        resume_group: usize,
        resume_entry: usize,
        state: PrefixState,
    ) {
        let est_bytes = state.estimated_bytes();
        if est_bytes > self.max_bytes {
            self.stats.size_refusals += 1;
            tracing::debug!(
                target: "rs_cam_core::sim_prefix",
                est_bytes,
                max_bytes = self.max_bytes,
                "sim prefix snapshot refused: over size ceiling"
            );
            return;
        }
        let groups = request
            .groups
            .iter()
            .enumerate()
            .take(resume_group + 1)
            .map(|(gi, group)| {
                let exact = gi < resume_group;
                let count = if exact {
                    group.toolpaths.len()
                } else {
                    resume_entry
                };
                GroupKey::new(group, count, exact)
            })
            .collect();
        self.stats.snapshots_stored += 1;
        self.stats.held_bytes = est_bytes;
        self.snapshot = Some(SimPrefixSnapshot {
            global_key: hash_global_scalar(request),
            model_mesh: request.model_mesh.as_ref().map(Arc::downgrade),
            groups,
            resume_group,
            resume_entry,
            state,
        });
    }
}

/// How a caller wants one simulation to interact with the prefix memo.
pub struct SimMemo<'a> {
    /// The cache to consult.
    pub cache: &'a mut SimPrefixCache,
    /// Whether to leave a fresh snapshot behind. `false` still *uses* a held
    /// snapshot — and, because lookup takes, frees it.
    pub store: bool,
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_bytes_estimate_ignores_arc_shared_checkpoints() {
        // The estimate must count what the snapshot ADDS. A checkpoint is a
        // marching-cubes mesh plus a full grid clone; if it were counted at
        // full size the ceiling would refuse every real project even though
        // the snapshot only adds a refcount.
        let bbox = crate::geo::BoundingBox3 {
            min: crate::geo::P3::new(0.0, 0.0, 0.0),
            max: crate::geo::P3::new(10.0, 10.0, 5.0),
        };
        let stock = TriDexelStock::from_bounds(&bbox, 1.0);
        let state = PrefixState {
            total_moves: 0,
            boundary_index: 0,
            boundaries: Vec::new(),
            checkpoints: Vec::new(),
            cut_samples: Vec::new(),
            drill_samples_all: Vec::new(),
            drill_summaries_all: Vec::new(),
            composite_mesh: StockMesh::empty(),
            global_stock: stock.clone(),
            column_deviations: None,
            rapid_collisions: Vec::new(),
            rapid_collision_move_indices: Vec::new(),
            prior_stocks: HashMap::new(),
            global_drill_ops: Vec::new(),
            group_stock: Some(stock.clone()),
            group_drill_ops: Vec::new(),
        };
        let bare = state.estimated_bytes();
        let mut with_cp = state;
        with_cp.checkpoints = vec![Arc::new(SimCheckpointMesh {
            boundary_index: 0,
            mesh: StockMesh::empty(),
            stock,
        })];
        // One pointer, not one grid.
        assert!(with_cp.estimated_bytes() - bare < 64);
    }
}
