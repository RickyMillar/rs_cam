// The P4 split moved three method groups of `impl SimulationState` into
// children. An inherent impl in a child still produces
// `SimulationState::method(..)`, so no name needs a re-export.
mod issue_triage;
mod playback_state;
mod semantic_trace;

use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::path::PathBuf;
use std::sync::{Arc, Weak};

use super::job::SetupId;
use super::runtime::GuiState;
use super::toolpath::ToolpathId;
use rs_cam_core::dexel_stock::TriDexelStock;
use rs_cam_core::stock::collision::{CollisionReport, RapidCollision};
use rs_cam_core::stock::simulation_cut::{
    SimulationCutSample, SimulationCutTrace, SimulationMetricOptions,
};
use rs_cam_core::stock::stock_mesh::StockMesh;
use rs_cam_core::tool_load::ToolLoadReport;
use rs_cam_core::trace::semantic_trace::{ToolpathSemanticItem, ToolpathSemanticTrace};
use rs_cam_core::trace::toolpath_spans::SpanId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolpathTraceAvailability {
    None,
    Semantic,
    Performance,
    PerformanceAndSemantic,
    Partial,
}

#[derive(Debug, Clone)]
pub struct ActiveSemanticItem {
    pub toolpath_id: ToolpathId,
    pub boundary_index: usize,
    pub local_move: usize,
    pub item: ToolpathSemanticItem,
    pub ancestry: Vec<ToolpathSemanticItem>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SimulationTraceTarget {
    pub toolpath_id: ToolpathId,
    pub move_index: usize,
    pub semantic_item_id: Option<u64>,
    pub debug_span_id: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimulationIssueKind {
    Hotspot,
    Annotation,
    AirCut,
    LowEngagement,
    RapidCollision,
    HolderCollision,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulationIssue {
    pub kind: SimulationIssueKind,
    pub toolpath_id: Option<ToolpathId>,
    pub move_index: usize,
    pub label: String,
    pub semantic_item_id: Option<u64>,
    pub debug_span_id: Option<u64>,
    pub hotspot_index: Option<usize>,
    pub annotation_index: Option<usize>,
}

#[derive(Clone, Default)]
pub(crate) struct SimulationSemanticIndex {
    trace_item_count: usize,
    pub(crate) move_item_indices: Vec<usize>,
    pub(crate) item_index_by_id: HashMap<u64, usize>,
    pub(crate) child_indices_by_parent: HashMap<Option<u64>, Vec<usize>>,
    pub(crate) depths: Vec<usize>,
}

#[derive(Default)]
pub struct SimulationDebugState {
    pub enabled: bool,
    pub expanded_toolpaths: HashSet<ToolpathId>,
    pub focused_hotspot: Option<(ToolpathId, usize)>,
    pub pinned_semantic_item: Option<(ToolpathId, u64)>,
    pub focused_issue_index: Option<usize>,
    pub highlight_active_item: bool,
    pub pending_inspect_toolpath: Option<ToolpathId>,
    /// Span-scope filter for the inspector pane. When set, scopes the
    /// project-overview metrics, breakdown tree, and findings list to a
    /// single span (and its descendants) on a single toolpath. Mirrors the
    /// `inspect_spans` MCP filter semantics so agent and human read the
    /// same numbers.
    pub span_scope: SpanScope,
    /// Per-(toolpath, span) sample aggregates, computed once per
    /// `Arc<SimulationCutTrace>` and reused on every subsequent inspector
    /// frame. Without this cache the Selected section rescans the full
    /// sample list every frame — see `SpanAggregateCache::ensure_built`.
    pub(crate) span_aggregates: SpanAggregateCache,
    /// Cached project tool-load verdicts keyed by sim trace + edit counter.
    /// The bottom timeline and right inspector both need this every frame;
    /// building it scans large cut traces once per criterion/toolpath.
    pub(crate) load_report_cache: ToolLoadReportCache,
    /// Cached chipload envelope LUT matches keyed by sim trace + edit counter.
    /// The lookup needs per-toolpath peak axial DOC and otherwise scans the
    /// full sample trace for every toolpath if rebuilt per frame.
    pub(crate) chipload_envelope_cache: ChiploadEnvelopeCache,
    /// Cached simulation triage keyed by sim trace + edit counter, matching
    /// the `load_report_cache` / `chipload_envelope_cache` staleness rule.
    /// Building it walks the full cut trace per toolpath twice (diagnostics
    /// then triage) with a per-toolpath height sort, so the inspector's
    /// measurability strip must not rebuild it every frame.
    pub(crate) triage_cache: SimulationTriageCache,
    /// Cached sorted issue list keyed by sim/debug trace fingerprints.
    /// Avoids rebuilding + sorting the same air-cut/hotspot/collision list
    /// in multiple panels during smooth playback.
    issue_cache: IssueListCache,
    pub(crate) semantic_indexes: HashMap<ToolpathId, SimulationSemanticIndex>,
}

/// Liveness-checked pointer identity for a cache key.
///
/// **Pointer keys are `Weak`, never bare pointers.** A bare `Arc::as_ptr`
/// key is ABA-unsound: the `Arc` can be dropped and a fresh allocation can
/// land at the same address, so a later lookup is answered with the previous
/// object's derived data. A live `Weak` keeps the *allocation* reserved even
/// after the last strong reference drops (only the `T` inside it is dropped),
/// so while a cache entry exists no other `Arc` can be handed that address —
/// the collision is impossible, not merely unlikely. Lookup upgrades and
/// compares with [`Arc::ptr_eq`], which additionally proves the cached entry's
/// subject is still alive.
///
/// The doctrine and its full argument live in `rs_cam_core::maps::geom_cache` (module
/// doc) and `rs_cam_core::compute::sim_prefix` (`weak_matches`, copied here
/// because the viz caches key on viz-side state). Pairing a bare pointer with
/// an element count or an edit counter — what these four caches used to do —
/// narrows the collision window rather than closing it; for the cut trace it
/// narrows it barely at all, because every `ArcInner<SimulationCutTrace>` is
/// the same fixed size (the sample `Vec`s hang off it), so every trace in the
/// process shares one malloc size class.
///
/// The `(None, None) => true` arm is required: "no trace" is a legitimate
/// cached state, and dropping the arm would rebuild every frame a project has
/// no simulation.
fn weak_matches<T>(stored: Option<&Weak<T>>, live: Option<&Arc<T>>) -> bool {
    match (stored, live) {
        (None, None) => true,
        (Some(w), Some(a)) => w.upgrade().is_some_and(|up| Arc::ptr_eq(&up, a)),
        _ => false,
    }
}

#[derive(Default)]
pub(crate) struct ToolLoadReportCache {
    /// Weak-pinned identity of the trace this report was built from. See
    /// [`weak_matches`].
    trace: Option<Weak<SimulationCutTrace>>,
    edit_counter: u64,
    report: Option<ToolLoadReport>,
}

#[derive(Default)]
pub(crate) struct ChiploadEnvelopeCache {
    /// Weak-pinned identity of the trace these envelopes were built from.
    /// See [`weak_matches`].
    trace: Option<Weak<SimulationCutTrace>>,
    edit_counter: u64,
    envelopes: Option<HashMap<rs_cam_core::ToolpathId, Range<f64>>>,
}

/// Cached [`rs_cam_core::stock::sim_triage::SimulationTriage`] for the inspector.
///
/// `built` distinguishes "never built" from "built for a project with no cut
/// trace", because `trace == None` is itself a legitimate cached state.
///
/// `evidence_fp` closes a second, larger staleness hole that is not ABA:
/// [`SimulationState::project_evidence`] feeds the triage from collision and
/// resolution state that is *not* the trace — and the holder-collision report
/// in particular is written by a separate async job that bumps no counter and
/// replaces no trace. The sibling `issue_cache_key` has folded a
/// `collision_fingerprint` in since it was written; this cache had not, so a
/// holder report arriving after the triage was cached left the panel showing
/// safety findings built without it.
#[derive(Default)]
pub(crate) struct SimulationTriageCache {
    built: bool,
    /// Weak-pinned identity of the trace this triage was built from. See
    /// [`weak_matches`].
    trace: Option<Weak<SimulationCutTrace>>,
    edit_counter: u64,
    /// Fingerprint over the non-trace [`rs_cam_core::session::ProjectEvidence`]
    /// inputs — see [`SimulationState::evidence_fingerprint`].
    evidence_fp: u64,
    triage: rs_cam_core::stock::sim_triage::SimulationTriage,
}

impl SimulationTriageCache {
    /// True when this entry answers for exactly this trace, edit version and
    /// evidence fingerprint.
    fn matches(
        &self,
        live: Option<&Arc<SimulationCutTrace>>,
        edit_counter: u64,
        evidence_fp: u64,
    ) -> bool {
        self.built
            && weak_matches(self.trace.as_ref(), live)
            && self.edit_counter == edit_counter
            && self.evidence_fp == evidence_fp
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct IssueListCacheKey {
    cut_trace_ptr: Option<usize>,
    gui_edit_counter: u64,
    debug_trace_fingerprint: u64,
    max_feed_bits: u64,
    collision_fingerprint: u64,
}

/// Cached issue list. Held behind an `Arc<[_]>` rather than a `Vec` because
/// the list runs to tens of thousands of entries (each with a `String`) and
/// three or four panels ask for it every frame — a cache hit must not deep
/// copy it. `hotspot_count` is folded in at build time so the timeline's
/// hotspot pill needs no scan at all.
struct IssueListCache {
    key: Option<IssueListCacheKey>,
    issues: Arc<[SimulationIssue]>,
    hotspot_count: usize,
}

impl Default for IssueListCache {
    fn default() -> Self {
        Self {
            key: None,
            issues: Arc::from(Vec::new()),
            hotspot_count: 0,
        }
    }
}

/// Per-(toolpath, span) aggregate of cut samples whose `span_path` contains
/// the span. Mirrors what the per-span breakdown displays: counts, mean and
/// peak engagement / chipload, peak axial DOC, and MRR.
#[derive(Default, Clone, Copy)]
pub struct SpanAggregate {
    pub n_samples: usize,
    pub n_cutting: usize,
    pub sum_eng: f64,
    pub peak_eng: f64,
    /// Running sum / peak of the **achieved advance per tooth**
    /// (`effective_feed / (rpm · flutes)`), over the `n_advance` samples
    /// that could state one.
    pub sum_advance: f64,
    pub peak_advance: f64,
    pub n_advance: usize,
    /// Running sum / peak of the **arc-mean chip thickness**, over the
    /// `n_chip` samples that resolved a chip model.
    ///
    /// Until 2026-08-08 there was a single `sum_chip` fed by
    /// `effective_chip_thickness_mm.unwrap_or(chipload_mm_per_tooth)` and
    /// printed under one label with unit `mm`. On a trace where some
    /// samples resolve a chip model and some do not, that average was a
    /// **mean of two different physical quantities** (A-1 census row V4).
    /// The blend is gone: each quantity now has its own accumulator, its
    /// own denominator, and its own row.
    pub sum_chip: f64,
    pub peak_chip: f64,
    pub n_chip: usize,
    pub peak_doc: f64,
    pub sum_mrr: f64,
    pub peak_mrr: f64,
}

impl SpanAggregate {
    pub fn ingest(
        &mut self,
        sample: &SimulationCutSample,
        predicted_feeds: &rs_cam_core::machine::kinematics::PredictedFeedMap,
    ) {
        self.n_samples += 1;
        if !sample.is_cutting {
            return;
        }
        self.n_cutting += 1;
        self.sum_eng += sample.engagement.radial_woc_fraction;
        if sample.engagement.radial_woc_fraction > self.peak_eng {
            self.peak_eng = sample.engagement.radial_woc_fraction;
        }
        if let Some(advance) =
            rs_cam_core::tool_load::display::achieved_advance_per_tooth(sample, predicted_feeds)
        {
            self.n_advance += 1;
            self.sum_advance += advance.mm();
            if advance.mm() > self.peak_advance {
                self.peak_advance = advance.mm();
            }
        }
        if let Some(chip) = rs_cam_core::tool_load::display::arc_mean_chip_thickness(sample) {
            self.n_chip += 1;
            self.sum_chip += chip.mm();
            if chip.mm() > self.peak_chip {
                self.peak_chip = chip.mm();
            }
        }
        if sample.axial_engagement_mm > self.peak_doc {
            self.peak_doc = sample.axial_engagement_mm;
        }
        self.sum_mrr += sample.mrr_mm3_s;
        if sample.mrr_mm3_s > self.peak_mrr {
            self.peak_mrr = sample.mrr_mm3_s;
        }
    }

    pub fn avg_engagement(&self) -> f64 {
        if self.n_cutting == 0 {
            0.0
        } else {
            self.sum_eng / self.n_cutting as f64
        }
    }

    /// Mean **achieved advance per tooth**. Denominator is the number of
    /// samples that produced one, not the cutting-sample count — a mean
    /// must divide by its own population.
    pub fn avg_advance_per_tooth(&self) -> f64 {
        if self.n_advance == 0 {
            0.0
        } else {
            self.sum_advance / self.n_advance as f64
        }
    }

    /// Mean **arc-mean chip thickness**, over the samples that resolved a
    /// chip model.
    pub fn avg_chip_thickness(&self) -> f64 {
        if self.n_chip == 0 {
            0.0
        } else {
            self.sum_chip / self.n_chip as f64
        }
    }

    pub fn avg_mrr(&self) -> f64 {
        if self.n_cutting == 0 {
            0.0
        } else {
            self.sum_mrr / self.n_cutting as f64
        }
    }
}

/// One-shot cache of derived per-trace data, keyed by the
/// `Arc<SimulationCutTrace>` pointer so it invalidates automatically when
/// a new sim trace lands. Currently caches:
///
/// - per-`(toolpath, span)` `SpanAggregate` for the inspector's Selected
///   section (replaces a per-frame full sample scan)
/// - per-toolpath cutting-sample indices for the signal-spine grouping
///   (replaces a per-frame `O(samples × toolpaths)` linear bucket find)
///
/// The full sample list is scanned exactly once per trace; subsequent
/// lookups are O(1).
#[derive(Default)]
pub struct SpanAggregateCache {
    /// Weak-pinned identity of the trace this cache reflects. `None` = no
    /// cache yet. We compare via object identity rather than content because
    /// the trace is large and immutable behind an `Arc`; the identity is a
    /// [`Weak`] and not a raw pointer for the reason [`weak_matches`] states
    /// — this cache used to hold a bare `Option<usize>` with no edit counter
    /// beside it, the strictest form of the hazard in the tree.
    cached_trace: Option<Weak<SimulationCutTrace>>,
    aggregates: HashMap<(ToolpathId, u32), SpanAggregate>,
    /// Indices into `trace.samples` of cutting samples, partitioned by
    /// toolpath. Used by `draw_signal_spine` to avoid re-grouping
    /// hundreds of thousands of samples every frame.
    cutting_indices: HashMap<ToolpathId, Vec<usize>>,
}

impl SpanAggregateCache {
    /// Rebuild caches from `trace` if this is a new (or first) trace.
    /// Cheap when the trace pointer matches the cached one.
    pub fn ensure_built(&mut self, trace: &Arc<SimulationCutTrace>) {
        if weak_matches(self.cached_trace.as_ref(), Some(trace)) {
            return;
        }
        self.aggregates.clear();
        self.cutting_indices.clear();
        for (idx, sample) in trace.samples.iter().enumerate() {
            let key_tp = sample.toolpath_id;
            for sid in &sample.span_path {
                self.aggregates
                    .entry((key_tp, sid.0))
                    .or_default()
                    .ingest(sample, &trace.predicted_feeds);
            }
            if sample.is_cutting {
                self.cutting_indices.entry(key_tp).or_default().push(idx);
            }
        }
        self.cached_trace = Some(Arc::downgrade(trace));
    }

    pub fn get(&self, toolpath_id: ToolpathId, span_id: u32) -> Option<&SpanAggregate> {
        self.aggregates.get(&(toolpath_id, span_id))
    }

    /// Indices of cutting samples for `toolpath_id`. Empty slice when the
    /// toolpath has no cutting samples in the cached trace.
    pub fn cutting_indices_for(&self, toolpath_id: ToolpathId) -> &[usize] {
        self.cutting_indices
            .get(&toolpath_id)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    pub fn invalidate(&mut self) {
        self.cached_trace = None;
        self.aggregates.clear();
        self.cutting_indices.clear();
    }
}

/// Inspector-pane span scope. `toolpath_id` selects the toolpath; `span_id`
/// optionally narrows to one span within that toolpath (the deepest-selected
/// chip in the filter row). A sample/issue/hotspot is "in scope" when its
/// `toolpath_id` matches and `span_id` (if set) appears in its `span_path`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SpanScope {
    pub toolpath_id: Option<ToolpathId>,
    pub span_id: Option<u32>,
}

impl SpanScope {
    pub fn is_active(&self) -> bool {
        self.toolpath_id.is_some() || self.span_id.is_some()
    }

    pub fn clear(&mut self) {
        self.toolpath_id = None;
        self.span_id = None;
    }

    /// True if a sample/issue/hotspot with this `toolpath_id` and `span_path`
    /// is in scope. Empty/unset filter matches everything.
    pub fn matches(&self, toolpath_id: rs_cam_core::ToolpathId, span_path: &[SpanId]) -> bool {
        if let Some(tp) = self.toolpath_id
            && tp != toolpath_id
        {
            return false;
        }
        if let Some(sid) = self.span_id
            && !span_path.iter().any(|s| s.0 == sid)
        {
            return false;
        }
        true
    }
}

/// How the simulation stock mesh is colored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StockVizMode {
    /// Default wood-tone gradient.
    Solid,
    /// Green/yellow/red/blue deviation from model surface.
    Deviation,
    /// Height gradient (low=blue, high=red).
    ByHeight,
}

/// Per-toolpath boundary in the simulation: toolpath ID and cumulative
/// move count at its end.
///
/// The core type, under the name the viz surfaces use. Viz held a
/// field-identical copy until 2026-09-16.
pub use rs_cam_core::compute::simulate::SimBoundary as ToolpathBoundary;

/// Per-setup boundary in the simulation: marks where a setup begins.
#[derive(Debug, Clone)]
pub struct SetupBoundary {
    pub setup_id: SetupId,
    pub setup_name: String,
    pub start_move: usize,
}

/// Checkpoint: a snapshot of the stock at a toolpath boundary.
///
/// **Shared with the core simulation result, not owned** (S5,
/// `rs_cam_core::compute::sim_prefix`). A checkpoint carries a marching-cubes
/// mesh plus a full dexel-grid clone, so it is the heaviest per-toolpath
/// artifact a simulation produces; deep-copying it here would put a second
/// copy of every checkpoint in GUI state while the fixpoint prefix memo holds
/// the first. Read through [`Self::mesh`] / [`Self::stock`].
pub struct SimCheckpoint {
    pub boundary_index: usize,
    /// The core checkpoint: composited display mesh + the tri-dexel stock at
    /// this boundary (the latter for resuming incremental sim).
    pub core: Arc<rs_cam_core::compute::simulate::SimCheckpointMesh>,
}

impl SimCheckpoint {
    /// Composited stock mesh at this boundary, in the global stock frame.
    #[must_use]
    pub fn mesh(&self) -> &StockMesh {
        &self.core.mesh
    }

    /// Tri-dexel stock at this boundary, for resuming incremental simulation.
    ///
    /// Read [`Self::stock_local_to_global`] alongside it — the frame is not
    /// the same for every setup.
    #[must_use]
    pub fn stock(&self) -> &TriDexelStock {
        &self.core.stock
    }

    /// Frame of [`Self::stock`]: `None` for the zero-rooted global playback
    /// frame, `Some(info)` for a setup-local one (lateral setups). See
    /// `rs_cam_core::compute::simulate::SimCheckpointMesh::stock_local_to_global`.
    #[must_use]
    pub fn stock_local_to_global(&self) -> Option<&rs_cam_core::compute::SetupTransformInfo> {
        self.core.stock_local_to_global.as_ref()
    }
}

// ---------------------------------------------------------------------------
// Sub-structs
// ---------------------------------------------------------------------------

/// Cached outputs from a simulation run. Treated as an artifact that persists
/// across workspace switches until the user explicitly resets.
pub struct SimulationResults {
    /// The fully-simulated stock mesh (at end of all toolpaths).
    pub mesh: StockMesh,
    /// Total move count across all simulated toolpaths.
    pub total_moves: usize,
    /// Per-toolpath boundaries for progress tracking and checkpoint lookup.
    pub boundaries: Vec<ToolpathBoundary>,
    /// Per-setup boundaries for setup transition markers.
    pub setup_boundaries: Vec<SetupBoundary>,
    /// Checkpoints at each toolpath boundary for rewind.
    pub checkpoints: Vec<SimCheckpoint>,
    /// Which toolpaths were included (None = all enabled).
    pub selected_toolpaths: Option<Vec<ToolpathId>>,
    /// Pre-transformed toolpath data for incremental playback. See
    /// [`crate::compute::worker::PlaybackToolpath`] for entry semantics.
    pub playback_data: Vec<crate::compute::worker::PlaybackToolpath>,
    /// Global stock bounding box used for this simulation (for fresh-stock reset).
    pub stock_bbox: rs_cam_core::geo::BoundingBox3,
    /// Simulation-time cutting metrics captured during the run.
    pub cut_trace: Option<Arc<SimulationCutTrace>>,
    /// Artifact path for the simulation cutting metrics trace.
    pub cut_trace_path: Option<PathBuf>,
    /// The dexel COLUMN grid cell this simulation used (mm). See
    /// `crate::compute::worker::SimulationResult::column_grid_cell_mm` — it
    /// is a property of THIS trace, not of `SimulationState::resolution`,
    /// which is the dial the next run will use.
    pub column_grid_cell_mm: f64,
    /// Per-toolpath snapshots of the material stock *before* that toolpath
    /// carves, keyed by toolpath id (F.4). Mirrors core's
    /// `rs_cam_core::compute::simulate::SimulationResult::prior_stocks` —
    /// includes real per-toolpath snapshots AND any phantom snapshot for
    /// the first pending `FromRemainingStock` toolpath in each group. The
    /// submit-time rest-machining gate in
    /// `controller::events::compute::submit_toolpath_compute` reads this
    /// map directly via [`SimulationState::prior_stock_for`] instead of
    /// re-deriving a "previous checkpoint" from `boundaries()` position
    /// arithmetic (which could never see a toolpath that had no boundary
    /// of its own, i.e. one that had never been generated).
    pub prior_stocks: HashMap<ToolpathId, Arc<TriDexelStock>>,
}

/// Transport / playback state — independent of whether results exist.
pub struct SimulationPlayback {
    /// Animation playback state.
    pub playing: bool,
    /// Current move index for timeline scrubbing.
    pub current_move: usize,
    /// Sub-integer move accumulator. Carries the fractional part of
    /// `speed * dt` across frames so slow playback rates (< 60 mv/s at
    /// 60 fps) actually run at the requested rate instead of being
    /// floored to "advance at least one move per frame". Reset to 0
    /// on scrub / play-pause toggles.
    pub partial_move: f32,
    /// Playback speed (moves per second).
    pub speed: f32,
    /// Tool position during playback (X, Y, Z).
    pub tool_position: Option<[f64; 3]>,
    /// Tool radius for the current operation during playback.
    pub tool_radius: f64,
    /// Tool type label for current operation during playback.
    pub tool_type_label: String,
    /// Total stickout for the current tool during playback.
    pub tool_stickout: f64,
    /// Cutting length for the current tool during playback.
    pub tool_cutting_length: f64,
    /// Predicted tip deflection for the current cutting sample, in mm.
    /// `None` on rapids, when metrics are unavailable, or when the current
    /// sample cannot be modeled.
    pub tool_deflection_mm: Option<f64>,
    /// Live tri-dexel stock for incremental playback simulation.
    pub live_stock: Option<TriDexelStock>,
    /// Setup group [`Self::live_stock`] belongs to, or `None` when it has not
    /// been claimed by one yet.
    ///
    /// The live stock is not frame-agnostic: a lateral setup replays into a
    /// SETUP-LOCAL stock (G-LATERALSCRUB) while every other setup replays into
    /// the shared zero-rooted global one. Crossing a group boundary can
    /// therefore mean the stock in hand is in the wrong frame entirely, which
    /// a move-index comparison alone would never notice.
    pub live_stock_group: Option<usize>,
    /// Move index the live heightmap has been simulated up to.
    pub live_sim_move: usize,
    /// Current display mesh (may differ from final mesh during scrubbing).
    pub display_mesh: Option<StockMesh>,
    /// Move index represented by `display_mesh` / uploaded GPU stock mesh.
    pub display_mesh_move: Option<usize>,
    /// Wall-clock time of the last live stock remesh/upload. Used to throttle
    /// playback rendering without slowing the timeline/tool marker.
    pub last_mesh_upload_at: Option<std::time::Instant>,
    /// Move index represented by the uploaded tool wireframe. The geometry is
    /// tiny, but recreating its GPU buffer every frame is still avoidable.
    pub tool_gpu_move: Option<usize>,
    /// True when `display_mesh` is the fast top-surface playback preview
    /// rather than the fully closed simulation mesh. When playback pauses,
    /// the next live-sim update replaces it with a full mesh for inspection.
    pub display_mesh_preview: bool,
    /// True for the current frame while the user is dragging a simulation
    /// scrubber/plot. Live stock replay is intentionally deferred until the
    /// drag releases so the UI tracks the pointer immediately.
    pub scrub_drag_active: bool,
    /// Per-vertex deviations from model surface (for deviation coloring).
    pub display_deviations: Option<Vec<f32>>,
}

/// The population one holder-clearance verdict covers (F2.13, G-HOLDERSCOPE).
///
/// `AppController::request_collision_check` examines ONE toolpath — the first
/// that has a result, a cutter and a mesh — while the row it feeds is titled
/// "Holder clearance" for the whole job. A verdict that does not carry its own
/// population cannot say so, and a clear reading off one operation then
/// presents as a clean job-wide verdict.
///
/// `Default` is the EMPTY population: `examined` 0, and [`Self::covers_the_job`]
/// is `false`. A gate handed an empty population must not pass.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct HolderCheckScope {
    /// ENABLED toolpaths the verdict examined — one, or zero, on every
    /// shipped path today.
    ///
    /// Zero is reachable: the selection takes the first toolpath that has a
    /// result and a mesh, with no `enabled` test, and a switched-off
    /// operation keeps its GUI result. A verdict about motion the job does
    /// not contain examines none of the job.
    pub examined: usize,
    /// Enabled toolpaths in the job when the check was submitted — the
    /// population the row is labelled for.
    ///
    /// Deliberately NOT "toolpaths a check could examine". An operation with
    /// no mesh is one this checker cannot reach, and counting it out of the
    /// denominator would let the row read Clear for a job most of which was
    /// never asked about — the same overstatement through a different door.
    pub population: usize,
    /// 1-based position, in the operation list, of the one toolpath examined.
    /// `None` when the verdict covers none, or more than one.
    pub position: Option<usize>,
}

impl HolderCheckScope {
    /// True only when the verdict covers every enabled operation.
    ///
    /// An empty population is false, not true. Three gates on this repo once
    /// returned `Within` over `sample_range 0..0`.
    pub fn covers_the_job(self) -> bool {
        self.examined > 0 && self.population > 0 && self.examined == self.population
    }

    /// Enabled operations this verdict never examined.
    pub fn unexamined(self) -> usize {
        self.population.saturating_sub(self.examined)
    }
}

/// Verification / check outputs.
#[derive(Default)]
pub struct SimulationChecks {
    /// Rapid-through-stock collisions from last simulation.
    pub rapid_collisions: Vec<RapidCollision>,
    /// Move indices with rapid collisions (for timeline markers).
    pub rapid_collision_move_indices: Vec<usize>,
    /// Full collision report from last dedicated collision check.
    pub collision_report: Option<CollisionReport>,
    /// Number of holder collisions from last dedicated collision check.
    pub holder_collision_count: usize,
    /// Min safe stickout from last collision check.
    ///
    /// Written only when the check FOUND collisions; a clear check leaves it
    /// `None`. It therefore says nothing about whether a check has run, and
    /// [`crate::ui::readiness::holder_clearance_check`] no longer reads it as
    /// if it did — see [`Self::checked_at_epoch`].
    pub min_safe_stickout: Option<f64>,
    /// `ProjectSession::simulation_epoch` as it stood when the collision
    /// check that produced the verdict above was SUBMITTED (F2.12,
    /// G-HOLDERSTALE).
    ///
    /// `None` means no check has run. Any other value is compared against the
    /// live epoch by [`SimulationState::collision_check_is_stale`], which is
    /// how the holder-clearance row says its evidence is old. The check reads
    /// the holder assembly, the workholding obstacles, the emitted motion and
    /// the setup frame. Every core door that writes one of those reaches
    /// `ProjectSession::drop_simulation`, so the one epoch follows them all.
    ///
    /// W4: this was the GUI edit counter. The counter moved for edits that
    /// write no project data, so an export wizard field withdrew a verdict it
    /// could not have changed; the epoch moves only where the core drops the
    /// simulation.
    ///
    /// Staleness withdraws a CLEARANCE claim; it never withdraws a measured
    /// STRIKE. See [`crate::ui::readiness::holder_clearance_check`].
    pub checked_at_epoch: Option<u64>,
    /// The population [`Self::holder_collision_count`] was measured over
    /// (F2.13, G-HOLDERSCOPE), stamped at SUBMIT like the counter above.
    ///
    /// `HolderCheckScope::default()` (`examined: 0`) means no check has run.
    /// [`crate::ui::readiness::holder_clearance_check`] reads
    /// [`HolderCheckScope::covers_the_job`] before it lets the row read
    /// `Pass`, so a verdict about one operation of four is never published as
    /// the job's.
    pub checked_scope: HolderCheckScope,
}

impl SimulationChecks {
    /// Total safety-relevant collisions: holder-clearance + rapid-through-stock.
    ///
    /// The single source of truth for every collision tally in the UI
    /// (status bar, workspace badges, timeline, diagnostics). Before W0.3
    /// the status bar counted holder-only while the workspace bar counted
    /// holder+rapid, so the same project could read "0 collisions" at the
    /// bottom and "2!" on the Simulation tab (SHE-002).
    pub fn total_collision_count(&self) -> usize {
        self.holder_collision_count + self.rapid_collisions.len()
    }
}

/// Metadata about the last simulation run for staleness tracking.
pub struct SimulationRunMeta {
    /// The metric-options revision this accepted result PROVABLY answers.
    ///
    /// `None` means the drain could not prove one: the submit stamp was
    /// consumed by a cancel or an error, and a late result arrived behind
    /// it. An unprovable revision reads STALE, never current — the rule
    /// [`SimulationChecks::checked_at_epoch`] already applies to a
    /// holder verdict, for the same reason: recording preferences are not
    /// a safety claim, but a "current" reading on unprovable evidence is
    /// still a wrong reading.
    pub accepted_metric_options_revision: Option<u64>,
}

// ---------------------------------------------------------------------------
// Top-level simulation state
// ---------------------------------------------------------------------------

/// Simulation state: results artifact + playback transport + verification checks.
pub struct SimulationState {
    /// Cached simulation results (None = no results yet).
    pub results: Option<SimulationResults>,
    /// Transport / playback state.
    pub playback: SimulationPlayback,
    /// Verification outputs (collisions, etc.).
    pub checks: SimulationChecks,
    /// Staleness metadata from the last simulation run.
    pub last_run: Option<SimulationRunMeta>,
    /// [`crate::state::runtime::GuiState::edit_counter`] as it stood when the
    /// in-flight simulation was SUBMITTED (F2.10, G-LATESIM).
    ///
    /// The drain used to stamp `last_sim_edit_counter` from the live counter
    /// on ARRIVAL, which folded any edit made while the simulation ran into
    /// the record of when it was run — so a result answering a discarded
    /// configuration was recorded as current evidence and
    /// [`SimulationState::is_stale`] said `false`. Same defect shape as
    /// F2.4's late toolpath result, on the surface an operator reads
    /// collision counts and engagement off.
    ///
    /// `None` means no submit has been seen through this controller, in which
    /// case the drain falls back to the live counter — the old behaviour, and
    /// not a claim this guard can make.
    ///
    /// The analysis lane runs one job at a time and a second submit cancels
    /// the first (`ThreadedComputeBackend::submit_analysis` clears the queue
    /// and sets the cancel flag), so at most one result can arrive per stamp.
    pub submitted_edit_counter: Option<u64>,
    /// `ProjectSession::simulation_epoch` as it stood when the in-flight
    /// simulation was submitted (D7, W0c).
    ///
    /// The core refuses an adopt whose epoch has moved, so this stamp is
    /// what lets a run reach the session at all. `None` means no submit
    /// has been seen through this controller, and then the drain adopts
    /// NOTHING: there is no fallback, because an unstamped result is a
    /// claim this guard cannot make. The view keeps the result and reads
    /// it as not current, which is what F2.10 already asks for.
    pub submitted_simulation_epoch: Option<u64>,
    /// `ProjectSession::simulation_epoch` as it stood when the in-flight
    /// COLLISION check was submitted (F2.12, G-HOLDERSTALE; W4 moved it off
    /// the GUI edit counter).
    ///
    /// The collision lane is the analysis lane, so `submit_analysis` clears
    /// the queue and cancels any in-flight job before queuing a new one. At
    /// most one collision result can therefore arrive per stamp, which is
    /// what makes a single `Option<u64>` sound rather than a map keyed by
    /// run. The drain `take()`s it, so a result with no submit of its own
    /// leaves [`SimulationChecks::checked_at_epoch`] at `None` and the row
    /// reads "Not checked". An unstamped result cannot say when it was
    /// measured, and a holder verdict is a safety claim.
    ///
    /// The simulation carries the same stamp for the same reason
    /// ([`Self::submitted_simulation_epoch`]), and neither one falls back to
    /// a live read.
    pub submitted_collision_epoch: Option<u64>,
    /// The population of the in-flight collision check, stamped at SUBMIT
    /// beside [`Self::submitted_collision_epoch`] (F2.13,
    /// G-HOLDERSCOPE).
    ///
    /// Stamped at submit rather than read live at render, for the reason the
    /// epoch is: it describes what the check COVERED, and the operation list
    /// can move under it while the lane works. An operation added afterwards
    /// moves the epoch, so the row withdraws the claim through staleness
    /// instead of quietly re-scoping a verdict that never saw it.
    pub submitted_collision_scope: Option<HolderCheckScope>,
    /// Heightmap cell size in mm (smaller = finer detail, more memory/time).
    pub resolution: f64,
    /// When true, resolution is auto-calculated from the smallest tool.
    pub auto_resolution: bool,
    /// Runtime-only capture options for simulation cutting metrics.
    pub metric_options: SimulationMetricOptions,
    /// Revision of runtime-only metric options.
    pub metric_options_revision: u64,
    /// Metric-options revision captured when the in-flight run was submitted.
    pub submitted_metric_options_revision: Option<u64>,
    /// Stock visualization mode.
    pub stock_viz_mode: StockVizMode,
    /// Stock opacity (0.0 = transparent, 1.0 = solid).
    pub stock_opacity: f32,
    /// Runtime-only debugger state and semantic lookup cache.
    pub debug: SimulationDebugState,
    /// Global move index (as f64 for sub-move pointer precision) under the
    /// cursor in the bottom signal spine. Set when the user hovers any signal
    /// track; consumed by every other track so they all show a vertical
    /// crosshair at the same X. One frame of lag is intentional: tracks read
    /// this on the same frame they may overwrite it.
    pub hovered_x: Option<f64>,
}

impl Default for SimulationState {
    fn default() -> Self {
        Self::new()
    }
}

impl SimulationDebugState {
    pub fn is_toolpath_expanded(&self, toolpath_id: ToolpathId) -> bool {
        self.expanded_toolpaths.contains(&toolpath_id)
    }

    pub fn set_toolpath_expanded(&mut self, toolpath_id: ToolpathId, expanded: bool) {
        if expanded {
            self.expanded_toolpaths.insert(toolpath_id);
        } else {
            self.expanded_toolpaths.remove(&toolpath_id);
        }
    }

    pub fn toggle_toolpath_expanded(&mut self, toolpath_id: ToolpathId) {
        let expanded = !self.is_toolpath_expanded(toolpath_id);
        self.set_toolpath_expanded(toolpath_id, expanded);
    }

    fn sync_semantic_indexes(&mut self, gui: &GuiState, boundaries: &[ToolpathBoundary]) {
        let boundary_ids: HashSet<_> = boundaries.iter().map(|boundary| boundary.id).collect();
        self.semantic_indexes
            .retain(|toolpath_id, _| boundary_ids.contains(toolpath_id));
        self.expanded_toolpaths
            .retain(|toolpath_id| boundary_ids.contains(toolpath_id));

        for toolpath_id in boundary_ids {
            let Some(trace) = gui
                .toolpath_rt
                .get(&toolpath_id)
                .and_then(|rt| rt.semantic_trace.as_ref())
            else {
                self.semantic_indexes.remove(&toolpath_id);
                continue;
            };

            let needs_rebuild = self
                .semantic_indexes
                .get(&toolpath_id)
                .is_none_or(|index| index.trace_item_count != trace.items.len());
            if needs_rebuild {
                self.semantic_indexes
                    .insert(toolpath_id, SimulationSemanticIndex::build(trace));
            }
        }
    }
}

impl SimulationSemanticIndex {
    #[allow(clippy::indexing_slicing)] // item_index from enumerate(), bounded by trace.items
    fn build(trace: &ToolpathSemanticTrace) -> Self {
        let mut item_index_by_id = HashMap::with_capacity(trace.items.len());
        let mut child_indices_by_parent: HashMap<Option<u64>, Vec<usize>> = HashMap::new();
        let mut move_item_indices = Vec::new();

        for (item_index, item) in trace.items.iter().enumerate() {
            item_index_by_id.insert(item.id, item_index);
            child_indices_by_parent
                .entry(item.parent_id)
                .or_default()
                .push(item_index);
            if item.move_start.is_some() && item.move_end.is_some() {
                move_item_indices.push(item_index);
            }
        }

        let mut depths = vec![0; trace.items.len()];
        for (item_index, item) in trace.items.iter().enumerate() {
            let mut depth = 0usize;
            let mut current_parent = item.parent_id;
            while let Some(parent_id) = current_parent {
                depth += 1;
                current_parent = item_index_by_id
                    .get(&parent_id)
                    .and_then(|parent_index| trace.items.get(*parent_index))
                    .and_then(|parent| parent.parent_id);
            }
            depths[item_index] = depth;
        }

        Self {
            trace_item_count: trace.items.len(),
            move_item_indices,
            item_index_by_id,
            child_indices_by_parent,
            depths,
        }
    }

    #[allow(clippy::indexing_slicing)] // item indices from move_item_indices, bounded by trace.items
    fn active_item_index(&self, trace: &ToolpathSemanticTrace, local_move: usize) -> Option<usize> {
        self.move_item_indices
            .iter()
            .copied()
            .filter(|item_index| {
                let item = &trace.items[*item_index];
                item.move_start.is_some_and(|start| start <= local_move)
                    && item.move_end.is_some_and(|end| local_move <= end)
            })
            .max_by(|left, right| {
                let left_item = &trace.items[*left];
                let right_item = &trace.items[*right];
                let left_span =
                    left_item.move_end.unwrap_or(usize::MAX) - left_item.move_start.unwrap_or(0);
                let right_span =
                    right_item.move_end.unwrap_or(usize::MAX) - right_item.move_start.unwrap_or(0);
                self.depths[*left]
                    .cmp(&self.depths[*right])
                    .then_with(|| right_span.cmp(&left_span))
                    .then_with(|| left_item.id.cmp(&right_item.id))
            })
    }

    #[allow(clippy::indexing_slicing)] // index from item_index_by_id, bounded by trace.items
    fn ancestry(
        &self,
        trace: &ToolpathSemanticTrace,
        item_index: usize,
    ) -> Vec<ToolpathSemanticItem> {
        let mut ancestry = Vec::new();
        let mut cursor = Some(item_index);
        while let Some(index) = cursor {
            let item = trace.items[index].clone();
            cursor = item
                .parent_id
                .and_then(|parent_id| self.item_index_by_id.get(&parent_id).copied());
            ancestry.push(item);
        }
        ancestry.reverse();
        ancestry
    }
}

impl Default for SimulationPlayback {
    fn default() -> Self {
        Self {
            playing: false,
            current_move: 0,
            partial_move: 0.0,
            speed: 500.0,
            tool_position: None,
            tool_radius: 0.0,
            tool_type_label: String::new(),
            tool_stickout: 0.0,
            tool_cutting_length: 0.0,
            tool_deflection_mm: None,
            live_stock: None,
            live_stock_group: None,
            live_sim_move: 0,
            display_mesh: None,
            display_mesh_move: None,
            last_mesh_upload_at: None,
            tool_gpu_move: None,
            display_mesh_preview: false,
            scrub_drag_active: false,
            display_deviations: None,
        }
    }
}

#[cfg(test)]
mod tests;
