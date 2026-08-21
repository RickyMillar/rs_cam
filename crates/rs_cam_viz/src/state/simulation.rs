use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::ops::Range;
use std::path::PathBuf;
use std::sync::{Arc, Weak};

use super::job::SetupId;
use super::runtime::GuiState;
use super::toolpath::ToolpathId;
use rs_cam_core::collision::{CollisionReport, RapidCollision};
use rs_cam_core::debug_trace::{ToolpathDebugAnnotation, ToolpathDebugBounds2};
use rs_cam_core::dexel_stock::{StockCutDirection, TriDexelStock};
use rs_cam_core::geo::{BoundingBox3, P3, V3};
use rs_cam_core::semantic_trace::{
    ToolpathSemanticItem, ToolpathSemanticKind, ToolpathSemanticTrace,
};
use rs_cam_core::session::ProjectSession;
use rs_cam_core::simulation_cut::{
    SimulationCutHotspot, SimulationCutIssue, SimulationCutIssueKind, SimulationCutSample,
    SimulationCutTrace, SimulationMetricOptions, SimulationSemanticCutSummary,
};
use rs_cam_core::stock_mesh::StockMesh;
use rs_cam_core::tool_load::ToolLoadReport;
use rs_cam_core::toolpath::{MoveType, Toolpath};
use rs_cam_core::toolpath_spans::SpanId;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SimulationAnalyticsTab {
    #[default]
    RunStatus,
    Safety,
    CutQuality,
    DebugTrace,
}

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

#[derive(Clone, Default)]
struct SimulationRuntimeProfile {
    move_count: usize,
    trace_item_count: usize,
    rapid_feed_mm_min: f64,
    cumulative_total_seconds: Vec<f64>,
    cumulative_cutting_seconds: Vec<f64>,
    cumulative_rapid_seconds: Vec<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SimulationRuntimeMetrics {
    pub total_seconds: f64,
    pub cutting_seconds: f64,
    pub rapid_seconds: f64,
    pub move_count: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SimulationRuntimeHotspot {
    pub toolpath_id: ToolpathId,
    pub item_id: u64,
    pub label: String,
    pub kind: ToolpathSemanticKind,
    pub move_start: usize,
    pub move_end: usize,
    pub total_seconds: f64,
    pub cutting_seconds: f64,
    pub rapid_seconds: f64,
    pub debug_span_id: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct ActiveCutSample {
    pub toolpath_id: ToolpathId,
    pub boundary_index: usize,
    pub local_move: usize,
    pub sample: SimulationCutSample,
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
    runtime_profiles: HashMap<ToolpathId, SimulationRuntimeProfile>,
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
/// The doctrine and its full argument live in `rs_cam_core::geom_cache` (module
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

/// Cached [`rs_cam_core::sim_triage::SimulationTriage`] for the inspector.
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
    triage: rs_cam_core::sim_triage::SimulationTriage,
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
        predicted_feeds: &rs_cam_core::machine_kinematics::PredictedFeedMap,
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

/// Per-toolpath boundary in the simulation: toolpath ID and cumulative move count at its end.
#[derive(Debug, Clone)]
pub struct ToolpathBoundary {
    pub id: ToolpathId,
    pub name: String,
    pub tool_name: String,
    pub start_move: usize,
    pub end_move: usize,
    /// Cut direction for this toolpath's setup.
    pub direction: StockCutDirection,
}

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
    #[must_use]
    pub fn stock(&self) -> &TriDexelStock {
        &self.core.stock
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
    pub min_safe_stickout: Option<f64>,
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
    /// Generation counter — incremented when sim results arrive.
    pub sim_generation: u64,
    /// Edit counter at the time of the last simulation run.
    pub last_sim_edit_counter: u64,
}

/// Saved viewport state for workspace transitions.
pub struct SavedViewportState {
    pub show_cutting: bool,
    pub show_rapids: bool,
    pub show_stock: bool,
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
    /// Heightmap cell size in mm (smaller = finer detail, more memory/time).
    pub resolution: f64,
    /// When true, resolution is auto-calculated from the smallest tool.
    pub auto_resolution: bool,
    /// Runtime-only capture options for simulation cutting metrics.
    pub metric_options: SimulationMetricOptions,
    /// Active right-panel simulation analytics section.
    pub analytics_tab: SimulationAnalyticsTab,
    /// Stock visualization mode.
    pub stock_viz_mode: StockVizMode,
    /// Stock opacity (0.0 = transparent, 1.0 = solid).
    pub stock_opacity: f32,
    /// Saved viewport state from editor mode (restored on exit).
    pub saved_viewport: SavedViewportState,
    /// Runtime-only debugger state and semantic lookup cache.
    pub debug: SimulationDebugState,
    /// Global move index (as f64 for sub-move pointer precision) under the
    /// cursor in the bottom signal spine. Set when the user hovers any signal
    /// track; consumed by every other track so they all show a vertical
    /// crosshair at the same X. One frame of lag is intentional: tracks read
    /// this on the same frame they may overwrite it.
    pub hovered_x: Option<f64>,
}

impl SimulationState {
    pub fn new() -> Self {
        Self {
            results: None,
            playback: SimulationPlayback {
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
                live_sim_move: 0,
                display_mesh: None,
                display_mesh_move: None,
                last_mesh_upload_at: None,
                tool_gpu_move: None,
                display_mesh_preview: false,
                scrub_drag_active: false,
                display_deviations: None,
            },
            checks: SimulationChecks {
                rapid_collisions: Vec::new(),
                rapid_collision_move_indices: Vec::new(),
                collision_report: None,
                holder_collision_count: 0,
                min_safe_stickout: None,
            },
            last_run: None,
            resolution: 0.25,
            auto_resolution: true,
            metric_options: SimulationMetricOptions::default(),
            analytics_tab: SimulationAnalyticsTab::default(),
            stock_viz_mode: StockVizMode::Solid,
            stock_opacity: 1.0,
            saved_viewport: SavedViewportState {
                show_cutting: true,
                show_rapids: true,
                show_stock: true,
            },
            debug: SimulationDebugState {
                enabled: false,
                expanded_toolpaths: HashSet::new(),
                focused_hotspot: None,
                pinned_semantic_item: None,
                focused_issue_index: None,
                highlight_active_item: true,
                pending_inspect_toolpath: None,
                span_scope: SpanScope::default(),
                span_aggregates: SpanAggregateCache::default(),
                load_report_cache: ToolLoadReportCache::default(),
                chipload_envelope_cache: ChiploadEnvelopeCache::default(),
                triage_cache: SimulationTriageCache::default(),
                issue_cache: IssueListCache::default(),
                semantic_indexes: HashMap::new(),
                runtime_profiles: HashMap::new(),
            },
            hovered_x: None,
        }
    }

    // --- Convenience accessors ---

    /// Whether simulation results exist.
    pub fn has_results(&self) -> bool {
        self.results.is_some()
    }

    /// Project tool-load report cached by simulation trace pointer and GUI edit counter.
    ///
    /// The report evaluates chipload/power/deflection for every enabled toolpath and
    /// scans the cut trace for the sample-based criteria. Both the bottom timeline and
    /// right inspector need it every frame, so compute it once per trace/edit version
    /// and return a cheap clone for borrow-friendly UI code.
    pub fn cached_load_report(
        &mut self,
        session: &ProjectSession,
        edit_counter: u64,
    ) -> ToolLoadReport {
        let live = self
            .results
            .as_ref()
            .and_then(|results| results.cut_trace.as_ref());
        if weak_matches(self.debug.load_report_cache.trace.as_ref(), live)
            && self.debug.load_report_cache.edit_counter == edit_counter
            && let Some(report) = &self.debug.load_report_cache.report
        {
            return report.clone();
        }
        let stored_trace = live.map(Arc::downgrade);

        let start = std::time::Instant::now();
        let sim_trace = self.results.as_ref().and_then(|r| r.cut_trace.as_deref());
        let report = rs_cam_core::gcode::project_load_report(session, sim_trace);
        let elapsed = start.elapsed();
        if elapsed > std::time::Duration::from_millis(8) {
            tracing::debug!(
                elapsed_ms = elapsed.as_secs_f64() * 1000.0,
                "slow simulation tool-load report build"
            );
        }
        self.debug.load_report_cache.trace = stored_trace;
        self.debug.load_report_cache.edit_counter = edit_counter;
        self.debug.load_report_cache.report = Some(report.clone());
        report
    }

    pub fn cached_chipload_envelopes(
        &mut self,
        session: &ProjectSession,
        edit_counter: u64,
    ) -> HashMap<rs_cam_core::ToolpathId, Range<f64>> {
        let live = self
            .results
            .as_ref()
            .and_then(|results| results.cut_trace.as_ref());
        if weak_matches(self.debug.chipload_envelope_cache.trace.as_ref(), live)
            && self.debug.chipload_envelope_cache.edit_counter == edit_counter
            && let Some(envelopes) = &self.debug.chipload_envelope_cache.envelopes
        {
            return envelopes.clone();
        }
        let stored_trace = live.map(Arc::downgrade);

        let start = std::time::Instant::now();
        let sim_trace = self.results.as_ref().and_then(|r| r.cut_trace.as_deref());
        let envelopes = rs_cam_core::tool_load::chipload_envelopes_for_session(session, sim_trace);
        let elapsed = start.elapsed();
        if elapsed > std::time::Duration::from_millis(8) {
            tracing::debug!(
                envelope_count = envelopes.len(),
                elapsed_ms = elapsed.as_secs_f64() * 1000.0,
                "slow chipload envelope build"
            );
        }
        self.debug.chipload_envelope_cache.trace = stored_trace;
        self.debug.chipload_envelope_cache.edit_counter = edit_counter;
        self.debug.chipload_envelope_cache.envelopes = Some(envelopes.clone());
        envelopes
    }

    /// Simulation triage cached by simulation trace identity, GUI edit
    /// counter and evidence fingerprint — the trace and edit rule is the same
    /// as [`Self::cached_load_report`] and [`Self::cached_chipload_envelopes`];
    /// the fingerprint is this cache's alone, because it is the only one of
    /// the three whose build reads state outside the trace
    /// ([`Self::project_evidence`], and see [`Self::evidence_fingerprint`]).
    ///
    /// Building it is `O(samples × toolpaths)` with a per-toolpath sort (full
    /// `ProjectDiagnostics` + `MeasurabilityReport` + `SimulationTriage::build`,
    /// plus a `build_cutter` per toolpath), and the inspector's measurability
    /// strip asks for it on every frame the Diagnostics header is open.
    /// Returned by reference: the triage carries several `Vec<Finding>`, so
    /// even a cache-hit clone would be per-frame allocation.
    pub fn cached_simulation_triage(
        &mut self,
        session: &ProjectSession,
        edit_counter: u64,
    ) -> &rs_cam_core::sim_triage::SimulationTriage {
        let evidence_fp = self.evidence_fingerprint();
        let fresh = {
            let live = self
                .results
                .as_ref()
                .and_then(|results| results.cut_trace.as_ref());
            self.debug
                .triage_cache
                .matches(live, edit_counter, evidence_fp)
        };
        if !fresh {
            let stored_trace = self
                .results
                .as_ref()
                .and_then(|results| results.cut_trace.as_ref())
                .map(Arc::downgrade);
            let start = std::time::Instant::now();
            // Scoped so the immutable `project_evidence` borrow of `self`
            // ends before the cache write below.
            let triage = {
                let evidence = self.project_evidence();
                session.simulation_triage(&evidence)
            };
            let elapsed = start.elapsed();
            if elapsed > std::time::Duration::from_millis(8) {
                tracing::debug!(
                    elapsed_ms = elapsed.as_secs_f64() * 1000.0,
                    "slow simulation triage build"
                );
            }
            self.debug.triage_cache.built = true;
            self.debug.triage_cache.trace = stored_trace;
            self.debug.triage_cache.edit_counter = edit_counter;
            self.debug.triage_cache.evidence_fp = evidence_fp;
            self.debug.triage_cache.triage = triage;
        }
        &self.debug.triage_cache.triage
    }

    /// Total moves from results (0 if no results).
    pub fn total_moves(&self) -> usize {
        self.results.as_ref().map_or(0, |r| r.total_moves)
    }

    /// Toolpath boundaries (empty slice if no results).
    pub fn boundaries(&self) -> &[ToolpathBoundary] {
        self.results
            .as_ref()
            .map_or(&[], |r| r.boundaries.as_slice())
    }

    /// Setup boundaries (empty slice if no results).
    pub fn setup_boundaries(&self) -> &[SetupBoundary] {
        self.results
            .as_ref()
            .map_or(&[], |r| r.setup_boundaries.as_slice())
    }

    /// Checkpoints (empty slice if no results).
    pub fn checkpoints(&self) -> &[SimCheckpoint] {
        self.results
            .as_ref()
            .map_or(&[], |r| r.checkpoints.as_slice())
    }

    /// Simulated stock snapshot from *before* `toolpath_id` carved, if the
    /// last simulation run produced one — either the toolpath's own
    /// pre-carve snapshot (it was generated and included in the run) or a
    /// phantom snapshot (F.4: it's the first pending `FromRemainingStock`
    /// toolpath in its group). `None` when no simulation has run yet, or
    /// when this toolpath sits behind a still-pending predecessor in its
    /// group (the ladder rule — see
    /// `rs_cam_core::compute::simulate::SimGroupEntry::phantom_prior_stock`).
    pub fn prior_stock_for(&self, toolpath_id: ToolpathId) -> Option<&Arc<TriDexelStock>> {
        self.results
            .as_ref()
            .and_then(|r| r.prior_stocks.get(&toolpath_id))
    }

    /// Selected toolpaths (None = all enabled).
    pub fn selected_toolpaths(&self) -> Option<&Vec<ToolpathId>> {
        self.results
            .as_ref()
            .and_then(|r| r.selected_toolpaths.as_ref())
    }

    /// Returns true if simulation results are stale (params changed since last sim).
    pub fn is_stale(&self, current_edit_counter: u64) -> bool {
        self.last_run
            .as_ref()
            .is_some_and(|meta| current_edit_counter > meta.last_sim_edit_counter)
    }

    pub fn progress(&self) -> f32 {
        let total = self.total_moves();
        if total == 0 {
            0.0
        } else {
            self.playback.current_move as f32 / total as f32
        }
    }

    /// Advance playback by dt seconds. Returns true if still playing.
    ///
    /// Carries the fractional part of `speed * dt` across frames in
    /// `partial_move` so the requested moves-per-second is honoured even
    /// when it's less than the frame rate. The old `.max(1)` floor here
    /// meant any speed below ~60 mv/s (at 60 fps) silently ran at the
    /// frame rate — making short toolpaths (e.g. an 18-move drill cycle)
    /// finish in a single frame regardless of the speed slider.
    pub fn advance(&mut self, dt: f32) -> bool {
        let total = self.total_moves();
        if !self.playback.playing || self.playback.current_move >= total {
            return false;
        }
        self.playback.partial_move += self.playback.speed * dt;
        // SAFETY: f32 < 4e9 fits usize on every target we care about;
        // partial_move is clamped to [0, speed * dt] so this can't grow
        // without bound.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let step = self.playback.partial_move.floor() as usize;
        if step > 0 {
            self.playback.partial_move -= step as f32;
            self.playback.current_move = (self.playback.current_move + step).min(total);
        }
        if self.playback.current_move >= total {
            self.playback.playing = false;
            self.playback.partial_move = 0.0;
        }
        true
    }

    /// Find which toolpath boundary contains the current move. Boundaries
    /// touch at a point (one's `end_move` equals the next's `start_move`),
    /// so we scan in reverse and pick the **later** boundary on a tie. That
    /// way, jumping to a boundary's `start_move` lands focus on that
    /// boundary rather than the one that just ended.
    pub fn current_boundary(&self) -> Option<&ToolpathBoundary> {
        let current = self.playback.current_move;
        self.boundaries()
            .iter()
            .rev()
            .find(|b| current >= b.start_move && current <= b.end_move)
    }

    /// The toolpath in focus for graph filtering and viewport-marker
    /// filtering. Tracks the playback position — focus follows whatever TP
    /// is currently playing. Selection in the left panel jumps playback to
    /// the chosen TP, which then becomes the focus via this getter; there's
    /// no separate sticky pin.
    pub fn focused_toolpath(&self) -> Option<ToolpathId> {
        self.current_boundary().map(|b| b.id)
    }

    pub fn current_boundary_index(&self) -> Option<usize> {
        // Same tie-breaking as `current_boundary`: prefer the later boundary
        // when `current_move` lands on a boundary point.
        let current = self.playback.current_move;
        let count = self.boundaries().len();
        self.boundaries()
            .iter()
            .rev()
            .position(|b| current >= b.start_move && current <= b.end_move)
            .map(|rev_idx| count - 1 - rev_idx)
    }

    #[allow(clippy::indexing_slicing)] // boundary_index from position() is always in bounds
    pub fn move_to_local_toolpath_move(
        &self,
        move_idx: usize,
    ) -> Option<(usize, ToolpathId, usize)> {
        // Same tie-breaking as `current_boundary`: at a boundary point
        // (where one ends and the next begins) prefer the *later* boundary.
        let count = self.boundaries().len();
        let boundary_index = self
            .boundaries()
            .iter()
            .rev()
            .position(|boundary| move_idx >= boundary.start_move && move_idx <= boundary.end_move)
            .map(|rev_idx| count - 1 - rev_idx)?;
        let boundary = &self.boundaries()[boundary_index];
        let local_move = move_idx.saturating_sub(boundary.start_move);
        Some((boundary_index, boundary.id, local_move))
    }

    pub fn current_local_toolpath_move(&self) -> Option<(usize, ToolpathId, usize)> {
        self.move_to_local_toolpath_move(self.playback.current_move)
    }

    /// Per-toolpath holder/shank collision counts from the last
    /// dedicated collision check, attributed via simulation boundaries.
    /// Empty when no check has run. This is the holder evidence the
    /// core's `diagnostics_with_evidence` consumes — derived from the
    /// stored report (O(collisions)), never recomputed, so it is safe
    /// to call at frame rate (the 2026-06-11 setup-tab lag was the
    /// diagnostics path re-running the full collision sweep per frame).
    pub fn holder_collision_counts_by_tp(&self) -> Vec<(ToolpathId, usize)> {
        let mut counts: Vec<(ToolpathId, usize)> = Vec::new();
        if let Some(report) = self.checks.collision_report.as_ref() {
            for collision in &report.collisions {
                if let Some((_, id, _)) = self.move_to_local_toolpath_move(collision.move_idx) {
                    match counts.iter_mut().find(|(cid, _)| *cid == id) {
                        Some((_, count)) => *count += 1,
                        None => counts.push((id, 1)),
                    }
                }
            }
        }
        counts
    }

    pub fn boundary_for_toolpath_id(&self, toolpath_id: ToolpathId) -> Option<&ToolpathBoundary> {
        self.boundaries()
            .iter()
            .find(|boundary| boundary.id == toolpath_id)
    }

    pub fn global_move_for_local(
        &self,
        toolpath_id: ToolpathId,
        local_move: usize,
    ) -> Option<usize> {
        let boundary = self.boundary_for_toolpath_id(toolpath_id)?;
        Some(boundary.start_move + local_move)
    }

    pub fn trace_availability_for_toolpath(
        gui: &GuiState,
        toolpath_id: ToolpathId,
    ) -> ToolpathTraceAvailability {
        let Some(rt) = gui.toolpath_rt.get(&toolpath_id) else {
            return ToolpathTraceAvailability::None;
        };

        let has_perf = rt.debug_trace.is_some();
        let has_semantic = rt.semantic_trace.is_some();
        let has_partial_only = rt.result.is_none() && (has_perf || has_semantic);

        if has_partial_only {
            ToolpathTraceAvailability::Partial
        } else if has_perf && has_semantic {
            ToolpathTraceAvailability::PerformanceAndSemantic
        } else if has_perf {
            ToolpathTraceAvailability::Performance
        } else if has_semantic {
            ToolpathTraceAvailability::Semantic
        } else {
            ToolpathTraceAvailability::None
        }
    }

    pub fn sync_debug_state(&mut self, gui: &GuiState, max_feed_mm_min: f64) {
        let boundaries = self.boundaries().to_vec();
        self.debug.sync_semantic_indexes(gui, &boundaries);
        self.debug
            .sync_runtime_profiles(gui, &boundaries, max_feed_mm_min);
        let boundary_ids: HashSet<_> = boundaries.iter().map(|boundary| boundary.id).collect();
        if self
            .debug
            .focused_hotspot
            .is_some_and(|(toolpath_id, _)| !boundary_ids.contains(&toolpath_id))
        {
            self.debug.focused_hotspot = None;
        }
        if self
            .debug
            .pinned_semantic_item
            .is_some_and(|(toolpath_id, _)| !boundary_ids.contains(&toolpath_id))
        {
            self.debug.pinned_semantic_item = None;
        }
    }

    pub fn semantic_runtime_metrics(
        &mut self,
        gui: &GuiState,
        max_feed_mm_min: f64,
        toolpath_id: ToolpathId,
        item_id: u64,
    ) -> Option<SimulationRuntimeMetrics> {
        self.sync_debug_state(gui, max_feed_mm_min);
        let rt = gui.toolpath_rt.get(&toolpath_id)?;
        let trace = rt.semantic_trace.as_ref()?;
        let index = self.debug.semantic_indexes.get(&toolpath_id)?;
        let item_index = index.item_index_by_id.get(&item_id).copied()?;
        let item = trace.items.get(item_index)?;
        let (move_start, move_end) = (item.move_start?, item.move_end?);
        let profile = self.debug.runtime_profiles.get(&toolpath_id)?;
        profile.metrics_for_range(move_start, move_end + 1)
    }

    pub fn toolpath_cut_summary(
        &self,
        toolpath_id: ToolpathId,
    ) -> Option<&rs_cam_core::simulation_cut::SimulationToolpathCutSummary> {
        self.results
            .as_ref()?
            .cut_trace
            .as_ref()?
            .toolpath_summaries
            .iter()
            .find(|summary| summary.toolpath_id == toolpath_id)
    }

    pub fn semantic_cut_summary(
        &self,
        toolpath_id: ToolpathId,
        item_id: u64,
    ) -> Option<&SimulationSemanticCutSummary> {
        self.results
            .as_ref()?
            .cut_trace
            .as_ref()?
            .semantic_summaries
            .iter()
            .find(|summary| {
                summary.toolpath_id == toolpath_id && summary.semantic_item_id == item_id
            })
    }

    pub fn cut_worst_items(
        &self,
        toolpath_id: ToolpathId,
        limit: usize,
    ) -> Vec<SimulationSemanticCutSummary> {
        let Some(trace) = self
            .results
            .as_ref()
            .and_then(|results| results.cut_trace.as_ref())
        else {
            return Vec::new();
        };
        let mut items: Vec<_> = trace
            .semantic_summaries
            .iter()
            .filter(|summary| summary.toolpath_id == toolpath_id)
            .cloned()
            .collect();
        items.sort_by(|left, right| {
            right
                .wasted_runtime_s
                .total_cmp(&left.wasted_runtime_s)
                .then_with(|| left.average_mrr_mm3_s.total_cmp(&right.average_mrr_mm3_s))
                .then_with(|| right.total_runtime_s.total_cmp(&left.total_runtime_s))
                .then_with(|| left.move_start.cmp(&right.move_start))
        });
        items.truncate(limit);
        items
    }

    /// Resolve the hotspot referenced by `debug.focused_hotspot`, if any.
    /// Returns `None` if no hotspot is focused or the index is stale (e.g.
    /// after a re-run produced a different `trace.hotspots`).
    pub fn focused_hotspot_data(&self) -> Option<&SimulationCutHotspot> {
        let (_, hotspot_index) = self.debug.focused_hotspot?;
        self.results
            .as_ref()
            .and_then(|r| r.cut_trace.as_ref())
            .and_then(|trace| trace.hotspots.get(hotspot_index))
    }

    pub fn cut_hotspots(&self, toolpath_id: ToolpathId, limit: usize) -> Vec<SimulationCutHotspot> {
        let Some(trace) = self
            .results
            .as_ref()
            .and_then(|results| results.cut_trace.as_ref())
        else {
            return Vec::new();
        };
        trace
            .hotspots
            .iter()
            .filter(|hotspot| hotspot.toolpath_id == toolpath_id)
            .take(limit)
            .cloned()
            .collect()
    }

    pub fn current_cut_sample(&self) -> Option<ActiveCutSample> {
        let (boundary_index, toolpath_id, local_move) = self.current_local_toolpath_move()?;
        let trace = self.results.as_ref()?.cut_trace.as_ref()?;
        let sample = trace
            .samples
            .iter()
            .filter(|sample| sample.toolpath_id == toolpath_id && sample.move_index <= local_move)
            .max_by(|left, right| {
                left.move_index
                    .cmp(&right.move_index)
                    .then_with(|| left.sample_index.cmp(&right.sample_index))
            })
            .cloned()?;
        Some(ActiveCutSample {
            toolpath_id,
            boundary_index,
            local_move,
            sample,
        })
    }

    #[allow(clippy::indexing_slicing)] // child_index from parent's child list, bounded by trace.items
    pub fn runtime_hotspots(
        &mut self,
        gui: &GuiState,
        max_feed_mm_min: f64,
        toolpath_id: ToolpathId,
        limit: usize,
    ) -> Vec<SimulationRuntimeHotspot> {
        self.sync_debug_state(gui, max_feed_mm_min);
        let Some(rt) = gui.toolpath_rt.get(&toolpath_id) else {
            return Vec::new();
        };
        let Some(trace) = rt.semantic_trace.as_ref() else {
            return Vec::new();
        };
        let Some(index) = self.debug.semantic_indexes.get(&toolpath_id) else {
            return Vec::new();
        };
        let Some(profile) = self.debug.runtime_profiles.get(&toolpath_id) else {
            return Vec::new();
        };

        let mut hotspots: Vec<_> = trace
            .items
            .iter()
            .filter_map(|item| {
                let (move_start, move_end) = (item.move_start?, item.move_end?);
                let has_move_linked_child = index
                    .child_indices_by_parent
                    .get(&Some(item.id))
                    .is_some_and(|children| {
                        children.iter().any(|child_index| {
                            let child = &trace.items[*child_index];
                            child.move_start.is_some() && child.move_end.is_some()
                        })
                    });
                if has_move_linked_child {
                    return None;
                }
                let metrics = profile.metrics_for_range(move_start, move_end + 1)?;
                Some(SimulationRuntimeHotspot {
                    toolpath_id,
                    item_id: item.id,
                    label: item.label.clone(),
                    kind: item.kind.clone(),
                    move_start,
                    move_end,
                    total_seconds: metrics.total_seconds,
                    cutting_seconds: metrics.cutting_seconds,
                    rapid_seconds: metrics.rapid_seconds,
                    debug_span_id: item.debug_span_id,
                })
            })
            .collect();

        hotspots.sort_by(|left, right| {
            right
                .total_seconds
                .total_cmp(&left.total_seconds)
                .then_with(|| left.move_start.cmp(&right.move_start))
                .then_with(|| left.label.cmp(&right.label))
        });
        hotspots.truncate(limit);
        hotspots
    }

    #[allow(clippy::indexing_slicing)] // active_index from active_item_index() bounded by trace.items
    pub fn playback_semantic_item(
        &mut self,
        gui: &GuiState,
        max_feed_mm_min: f64,
    ) -> Option<ActiveSemanticItem> {
        let (boundary_index, toolpath_id, local_move) = self.current_local_toolpath_move()?;
        self.sync_debug_state(gui, max_feed_mm_min);
        let rt = gui.toolpath_rt.get(&toolpath_id)?;
        let trace = rt.semantic_trace.as_ref()?;
        let index = self.debug.semantic_indexes.get(&toolpath_id)?;
        let active_index = index.active_item_index(trace, local_move)?;
        Some(ActiveSemanticItem {
            toolpath_id,
            boundary_index,
            local_move,
            item: trace.items[active_index].clone(),
            ancestry: index.ancestry(trace, active_index),
        })
    }

    pub fn semantic_item_by_id(
        &mut self,
        gui: &GuiState,
        max_feed_mm_min: f64,
        toolpath_id: ToolpathId,
        item_id: u64,
    ) -> Option<ActiveSemanticItem> {
        self.sync_debug_state(gui, max_feed_mm_min);
        let rt = gui.toolpath_rt.get(&toolpath_id)?;
        let trace = rt.semantic_trace.as_ref()?;
        let index = self.debug.semantic_indexes.get(&toolpath_id)?;
        let item_index = index.item_index_by_id.get(&item_id).copied()?;
        let boundary_index = self
            .boundaries()
            .iter()
            .position(|boundary| boundary.id == toolpath_id)?;
        let item = trace.items.get(item_index)?.clone();
        let local_move = item.move_start.unwrap_or_default();
        Some(ActiveSemanticItem {
            toolpath_id,
            boundary_index,
            local_move,
            item,
            ancestry: index.ancestry(trace, item_index),
        })
    }

    pub fn active_semantic_item(
        &mut self,
        gui: &GuiState,
        max_feed_mm_min: f64,
    ) -> Option<ActiveSemanticItem> {
        self.sync_debug_state(gui, max_feed_mm_min);
        if let Some((toolpath_id, item_id)) = self.debug.pinned_semantic_item
            && let Some(active) =
                self.semantic_item_by_id(gui, max_feed_mm_min, toolpath_id, item_id)
        {
            return Some(active);
        }
        self.playback_semantic_item(gui, max_feed_mm_min)
    }

    pub fn pin_semantic_item(&mut self, toolpath_id: ToolpathId, item_id: u64) {
        self.debug.pinned_semantic_item = Some((toolpath_id, item_id));
    }

    pub fn clear_pinned_semantic_item(&mut self) {
        self.debug.pinned_semantic_item = None;
    }

    pub fn active_debug_span(
        &mut self,
        gui: &GuiState,
        max_feed_mm_min: f64,
    ) -> Option<(ToolpathId, rs_cam_core::debug_trace::ToolpathDebugSpan)> {
        let active = self.active_semantic_item(gui, max_feed_mm_min)?;
        let rt = gui.toolpath_rt.get(&active.toolpath_id)?;
        let trace = rt.debug_trace.as_ref()?;
        let span_id = active
            .ancestry
            .iter()
            .rev()
            .find_map(|item| item.debug_span_id)?;
        trace
            .spans
            .iter()
            .find(|span| span.id == span_id)
            .cloned()
            .map(|span| (active.toolpath_id, span))
    }

    pub fn trace_target_for_item(
        &mut self,
        gui: &GuiState,
        max_feed_mm_min: f64,
        toolpath_id: ToolpathId,
        item_id: u64,
        prefer_end: bool,
    ) -> Option<SimulationTraceTarget> {
        let active = self.semantic_item_by_id(gui, max_feed_mm_min, toolpath_id, item_id)?;
        let local_move = if prefer_end {
            active.item.move_end.or(active.item.move_start)?
        } else {
            active.item.move_start.or(active.item.move_end)?
        };
        Some(SimulationTraceTarget {
            toolpath_id,
            move_index: self.global_move_for_local(toolpath_id, local_move)?,
            semantic_item_id: Some(item_id),
            debug_span_id: active
                .ancestry
                .iter()
                .rev()
                .find_map(|item| item.debug_span_id),
        })
    }

    pub fn trace_target_for_span(
        &mut self,
        gui: &GuiState,
        max_feed_mm_min: f64,
        toolpath_id: ToolpathId,
        span_id: u64,
        prefer_end: bool,
    ) -> Option<SimulationTraceTarget> {
        let rt = gui.toolpath_rt.get(&toolpath_id)?;
        let debug_trace = rt.debug_trace.as_ref()?;
        let span = debug_trace.spans.iter().find(|span| span.id == span_id)?;
        if let (Some(move_start), Some(move_end)) = (span.move_start, span.move_end) {
            return Some(SimulationTraceTarget {
                toolpath_id,
                move_index: self.global_move_for_local(
                    toolpath_id,
                    if prefer_end { move_end } else { move_start },
                )?,
                semantic_item_id: rt.semantic_trace.as_ref().and_then(|trace| {
                    trace
                        .items
                        .iter()
                        .find(|item| item.debug_span_id == Some(span_id))
                        .map(|item| item.id)
                }),
                debug_span_id: Some(span_id),
            });
        }

        let semantic_item_id = rt.semantic_trace.as_ref().and_then(|trace| {
            trace
                .items
                .iter()
                .find(|item| item.debug_span_id == Some(span_id))
                .map(|item| item.id)
        })?;
        self.trace_target_for_item(
            gui,
            max_feed_mm_min,
            toolpath_id,
            semantic_item_id,
            prefer_end,
        )
    }

    /// Resolve a [`SpanId`] from the [`AnnotatedToolpath`] of `toolpath_id`
    /// into a sim trace target. Mirrors [`Self::trace_target_for_span`] (which
    /// operates over debug-trace spans) but reads spans persisted on the
    /// generated toolpath itself.
    ///
    /// `prefer_end`: when true, anchor on the span's last move; otherwise
    /// the first. Boundary spans (zero-width) anchor on `start_move`.
    pub fn trace_target_for_annotated_span(
        &mut self,
        gui: &GuiState,
        toolpath_id: ToolpathId,
        span_id: SpanId,
        prefer_end: bool,
    ) -> Option<SimulationTraceTarget> {
        let rt = gui.toolpath_rt.get(&toolpath_id)?;
        let result = rt.result.as_ref()?;
        if !result.spans_valid() {
            return None;
        }
        let span = result.spans().get(span_id.0 as usize)?;
        let local_move = if span.is_boundary() {
            span.start_move
        } else if prefer_end {
            span.end_move.saturating_sub(1)
        } else {
            span.start_move
        };
        Some(SimulationTraceTarget {
            toolpath_id,
            move_index: self.global_move_for_local(toolpath_id, local_move)?,
            semantic_item_id: None,
            debug_span_id: None,
        })
    }

    pub fn trace_target_for_hotspot(
        &mut self,
        gui: &GuiState,
        max_feed_mm_min: f64,
        toolpath_id: ToolpathId,
        hotspot_index: usize,
    ) -> Option<SimulationTraceTarget> {
        let rt = gui.toolpath_rt.get(&toolpath_id)?;
        let debug_trace = rt.debug_trace.as_ref()?;
        let hotspot = debug_trace.hotspots.get(hotspot_index)?.clone();
        if let Some(item_id) = hotspot.semantic_item_id {
            return self.trace_target_for_item(gui, max_feed_mm_min, toolpath_id, item_id, false);
        }
        if let (Some(move_start), Some(_)) = (hotspot.move_start, hotspot.move_end) {
            return Some(SimulationTraceTarget {
                toolpath_id,
                move_index: self.global_move_for_local(toolpath_id, move_start)?,
                semantic_item_id: None,
                debug_span_id: hotspot.representative_span_id,
            });
        }
        hotspot.representative_span_id.and_then(|span_id| {
            self.trace_target_for_span(gui, max_feed_mm_min, toolpath_id, span_id, false)
        })
    }

    pub fn trace_target_for_annotation(
        &self,
        toolpath_id: ToolpathId,
        annotation: &ToolpathDebugAnnotation,
    ) -> Option<SimulationTraceTarget> {
        Some(SimulationTraceTarget {
            toolpath_id,
            move_index: self.global_move_for_local(toolpath_id, annotation.move_index)?,
            semantic_item_id: None,
            debug_span_id: None,
        })
    }

    pub fn trace_target_for_cut_issue(
        &mut self,
        issue: &SimulationCutIssue,
    ) -> Option<SimulationTraceTarget> {
        let toolpath_id = issue.toolpath_id;
        Some(SimulationTraceTarget {
            toolpath_id,
            move_index: self.global_move_for_local(toolpath_id, issue.move_index)?,
            semantic_item_id: issue.semantic_item_id,
            debug_span_id: None,
        })
    }

    pub fn current_debug_annotation_with_index(
        &self,
        gui: &GuiState,
    ) -> Option<(ToolpathId, usize, ToolpathDebugAnnotation)> {
        let (_, toolpath_id, local_move) = self.current_local_toolpath_move()?;
        let rt = gui.toolpath_rt.get(&toolpath_id)?;
        let trace = rt.debug_trace.as_ref()?;
        trace
            .annotations
            .iter()
            .enumerate()
            .rev()
            .find(|(_, annotation)| annotation.move_index <= local_move)
            .map(|(index, annotation)| (toolpath_id, index, annotation.clone()))
    }

    pub fn current_debug_annotation(
        &self,
        gui: &GuiState,
    ) -> Option<(ToolpathId, ToolpathDebugAnnotation)> {
        self.current_debug_annotation_with_index(gui)
            .map(|(toolpath_id, _, annotation)| (toolpath_id, annotation))
    }

    pub fn current_item_bbox(
        &mut self,
        gui: &GuiState,
        max_feed_mm_min: f64,
        session: &ProjectSession,
    ) -> Option<(ToolpathId, ToolpathDebugBounds2, f64, f64)> {
        let active = self.active_semantic_item(gui, max_feed_mm_min)?;
        let bbox =
            self.semantic_item_bbox_in_simulation(session, active.toolpath_id, &active.item)?;
        Some((
            active.toolpath_id,
            ToolpathDebugBounds2 {
                min_x: bbox.min.x,
                max_x: bbox.max.x,
                min_y: bbox.min.y,
                max_y: bbox.max.y,
            },
            bbox.min.z,
            bbox.max.z,
        ))
    }

    pub fn semantic_item_bbox_in_simulation(
        &self,
        session: &ProjectSession,
        toolpath_id: ToolpathId,
        item: &ToolpathSemanticItem,
    ) -> Option<BoundingBox3> {
        let xy = item.xy_bbox?;
        let z_min = item.z_min?;
        let z_max = item.z_max?;
        let local_corners = [
            P3::new(xy.min_x, xy.min_y, z_min),
            P3::new(xy.max_x, xy.min_y, z_min),
            P3::new(xy.max_x, xy.max_y, z_min),
            P3::new(xy.min_x, xy.max_y, z_min),
            P3::new(xy.min_x, xy.min_y, z_max),
            P3::new(xy.max_x, xy.min_y, z_max),
            P3::new(xy.max_x, xy.max_y, z_max),
            P3::new(xy.min_x, xy.max_y, z_max),
        ];
        let setup = session
            .setup_of_toolpath_id(toolpath_id)
            .and_then(|idx| session.list_setups().get(idx));
        Some(BoundingBox3::from_points(local_corners.into_iter().map(
            |corner| {
                setup.map_or(corner, |s| {
                    session.inverse_transform_point_from_setup(corner, s.face_up, s.z_rotation)
                })
            },
        )))
    }

    /// Build the core [`ProjectEvidence`] borrow view from viz state.
    ///
    /// One builder, so the GUI panel, the MCP handlers and anything else on
    /// the viz side hand core the same evidence. It lives here rather than in
    /// `app::mcp` because that module is behind the `mcp` feature and the GUI
    /// needs this with or without it.
    pub fn project_evidence(&self) -> rs_cam_core::session::ProjectEvidence<'_> {
        let boundaries = self
            .results
            .as_ref()
            .map(|r| {
                r.boundaries
                    .iter()
                    .map(|b| (b.id, b.start_move, b.end_move))
                    .collect()
            })
            .unwrap_or_default();
        rs_cam_core::session::ProjectEvidence {
            boundaries,
            rapid_collisions: &self.checks.rapid_collisions,
            rapid_collision_move_indices: &self.checks.rapid_collision_move_indices,
            cut_trace: self.results.as_ref().and_then(|r| r.cut_trace.as_deref()),
            holder_collisions: self.holder_collision_counts_by_tp(),
            // The cell the GUI last simulated at. Read only to enrich a
            // measurability abstention's reason with the number the operator
            // would have to change; it never decides a verdict.
            resolution_mm: Some(self.resolution),
        }
    }

    /// Fingerprint over every [`Self::project_evidence`] input that is **not**
    /// the cut trace, for [`Self::cached_simulation_triage`]'s key.
    ///
    /// The trace is keyed by weak-pinned identity; these are the other four
    /// evidence fields, none of which move the trace pointer and none of which
    /// bump the GUI edit counter:
    ///
    /// - `boundaries` — the move ranges the triage attributes findings through;
    /// - `rapid_collisions` and `rapid_collision_move_indices` — the
    ///   through-stock rapids the triage reports as `safety` findings;
    /// - the holder-collision report's move indices, which is the whole of
    ///   what [`Self::holder_collision_counts_by_tp`] reads. That report is
    ///   written by a *separate async job* (`controller::events::compute`),
    ///   long after the trace lands and with no counter bump: without this
    ///   fingerprint, a holder report arriving while the Diagnostics header is
    ///   open is never reflected in the cached triage;
    /// - `resolution_mm`, which enriches a measurability abstention's reason —
    ///   the one field today's sole consumer (`ui::sim_diagnostics`'s
    ///   `NOT MEASURED` strip) actually renders.
    ///
    /// Mirrors the `collision_fingerprint` the neighbouring `issue_cache_key`
    /// has always folded in. `O(boundaries + collisions)` per frame, the same
    /// order that key already pays.
    fn evidence_fingerprint(&self) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        for boundary in self.boundaries() {
            boundary.id.0.hash(&mut hasher);
            boundary.start_move.hash(&mut hasher);
            boundary.end_move.hash(&mut hasher);
        }
        self.checks.rapid_collisions.len().hash(&mut hasher);
        for collision in &self.checks.rapid_collisions {
            collision.move_index.hash(&mut hasher);
            for coord in [
                collision.start.x,
                collision.start.y,
                collision.start.z,
                collision.end.x,
                collision.end.y,
                collision.end.z,
            ] {
                coord.to_bits().hash(&mut hasher);
            }
        }
        self.checks
            .rapid_collision_move_indices
            .len()
            .hash(&mut hasher);
        for &move_index in &self.checks.rapid_collision_move_indices {
            move_index.hash(&mut hasher);
        }
        match self.checks.collision_report.as_ref() {
            Some(report) => {
                report.collisions.len().hash(&mut hasher);
                for collision in &report.collisions {
                    collision.move_idx.hash(&mut hasher);
                }
            }
            // Distinguish "no report yet" from "report with no collisions":
            // the async holder job replacing the former with the latter is
            // exactly the transition this fingerprint exists to catch.
            None => u64::MAX.hash(&mut hasher),
        }
        self.resolution.to_bits().hash(&mut hasher);
        hasher.finish()
    }

    /// Sorted issue list for the current simulation.
    ///
    /// Returns a shared handle, not a copy: the list runs to tens of
    /// thousands of entries (each with a `String` label) and three or four
    /// panels ask for it every frame, so a cache hit must cost an `Arc`
    /// bump rather than a deep clone. `Arc<[_]>` derefs to `&[_]`, so read
    /// sites (`iter`, `get`, indexing, `&issues` into a `&[_]` parameter)
    /// are unchanged.
    pub fn issues(&mut self, gui: &GuiState, max_feed_mm_min: f64) -> Arc<[SimulationIssue]> {
        self.ensure_issue_cache(gui, max_feed_mm_min);
        Arc::clone(&self.debug.issue_cache.issues)
    }

    /// Number of `Hotspot` issues, folded in when the issue cache is built.
    /// The timeline's pill needs only this count and used to clone the whole
    /// list to get it.
    pub fn issue_hotspot_count(&mut self, gui: &GuiState, max_feed_mm_min: f64) -> usize {
        self.ensure_issue_cache(gui, max_feed_mm_min);
        self.debug.issue_cache.hotspot_count
    }

    fn ensure_issue_cache(&mut self, gui: &GuiState, max_feed_mm_min: f64) {
        self.sync_debug_state(gui, max_feed_mm_min);
        let cache_key = self.issue_cache_key(gui, max_feed_mm_min);
        if self.debug.issue_cache.key == Some(cache_key) {
            return;
        }

        let start = std::time::Instant::now();
        let mut issues = Vec::new();

        for boundary in self.boundaries().to_vec() {
            let Some(rt) = gui.toolpath_rt.get(&boundary.id) else {
                continue;
            };
            if let Some(trace) = rt.debug_trace.as_ref() {
                for (annotation_index, annotation) in trace.annotations.iter().enumerate() {
                    issues.push(SimulationIssue {
                        kind: SimulationIssueKind::Annotation,
                        toolpath_id: Some(boundary.id),
                        move_index: boundary.start_move + annotation.move_index,
                        label: annotation.label.clone(),
                        semantic_item_id: rt.semantic_trace.as_ref().and_then(|semantic_trace| {
                            semantic_trace
                                .items
                                .iter()
                                .find(|item| {
                                    item.move_start
                                        .is_some_and(|start| start <= annotation.move_index)
                                        && item
                                            .move_end
                                            .is_some_and(|end| annotation.move_index <= end)
                                })
                                .map(|item| item.id)
                        }),
                        debug_span_id: None,
                        hotspot_index: None,
                        annotation_index: Some(annotation_index),
                    });
                }

                for (hotspot_index, hotspot) in trace.hotspots.iter().enumerate() {
                    let Some(target) = self.trace_target_for_hotspot(
                        gui,
                        max_feed_mm_min,
                        boundary.id,
                        hotspot_index,
                    ) else {
                        continue;
                    };
                    issues.push(SimulationIssue {
                        kind: SimulationIssueKind::Hotspot,
                        toolpath_id: Some(boundary.id),
                        move_index: target.move_index,
                        label: format!("{} hotspot {}", hotspot.kind, hotspot_index + 1),
                        semantic_item_id: target.semantic_item_id,
                        debug_span_id: hotspot.representative_span_id.or(target.debug_span_id),
                        hotspot_index: Some(hotspot_index),
                        annotation_index: None,
                    });
                }
            }
        }

        if let Some(trace) = self
            .results
            .as_ref()
            .and_then(|results| results.cut_trace.as_ref())
        {
            for issue in &trace.issues {
                let toolpath_id = issue.toolpath_id;
                let Some(global_move) = self.global_move_for_local(toolpath_id, issue.move_index)
                else {
                    continue;
                };
                issues.push(SimulationIssue {
                    kind: match issue.kind {
                        SimulationCutIssueKind::AirCut => SimulationIssueKind::AirCut,
                        SimulationCutIssueKind::LowEngagement => SimulationIssueKind::LowEngagement,
                    },
                    toolpath_id: Some(toolpath_id),
                    move_index: global_move,
                    label: issue.label.clone(),
                    semantic_item_id: issue.semantic_item_id,
                    debug_span_id: None,
                    hotspot_index: None,
                    annotation_index: None,
                });
            }
        }

        for &move_index in &self.checks.rapid_collision_move_indices {
            let toolpath_id = self
                .move_to_local_toolpath_move(move_index)
                .map(|(_, id, _)| id);
            issues.push(SimulationIssue {
                kind: SimulationIssueKind::RapidCollision,
                toolpath_id,
                move_index,
                label: "Rapid collision".to_owned(),
                semantic_item_id: None,
                debug_span_id: None,
                hotspot_index: None,
                annotation_index: None,
            });
        }

        if let Some(report) = self.checks.collision_report.as_ref() {
            for collision in &report.collisions {
                let toolpath_id = self
                    .move_to_local_toolpath_move(collision.move_idx)
                    .map(|(_, id, _)| id);
                issues.push(SimulationIssue {
                    kind: SimulationIssueKind::HolderCollision,
                    toolpath_id,
                    move_index: collision.move_idx,
                    label: format!("{} collision", collision.segment),
                    semantic_item_id: None,
                    debug_span_id: None,
                    hotspot_index: None,
                    annotation_index: None,
                });
            }
        }

        // D1 (census §3.5), ruled at Checkpoint D D-6. Severity was a
        // TIEBREAK under `move_index`, so an operator stepping the list with
        // `focus_issue_delta` reached collisions in path order — i.e. at
        // random relative to how much they matter — and a second,
        // contradictory rank in `sim_op_list.rs` put collisions first. One
        // rank now, severity-major, with `move_index` as the LAST key:
        // "what should I look at" is answered before "where is it".
        issues.sort_by(|left, right| {
            issue_kind_rank(left.kind)
                .cmp(&issue_kind_rank(right.kind))
                .then_with(|| left.move_index.cmp(&right.move_index))
                .then_with(|| left.label.cmp(&right.label))
        });
        let elapsed = start.elapsed();
        if elapsed > std::time::Duration::from_millis(8) {
            tracing::debug!(
                issue_count = issues.len(),
                elapsed_ms = elapsed.as_secs_f64() * 1000.0,
                "slow simulation issue list build"
            );
        }
        let hotspot_count = issues
            .iter()
            .filter(|issue| issue.kind == SimulationIssueKind::Hotspot)
            .count();
        self.debug.issue_cache.key = Some(cache_key);
        self.debug.issue_cache.issues = Arc::from(issues);
        self.debug.issue_cache.hotspot_count = hotspot_count;
    }

    fn issue_cache_key(&self, gui: &GuiState, max_feed_mm_min: f64) -> IssueListCacheKey {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        for boundary in self.boundaries() {
            boundary.id.0.hash(&mut hasher);
            if let Some(rt) = gui.toolpath_rt.get(&boundary.id) {
                if let Some(trace) = rt.debug_trace.as_ref() {
                    (Arc::as_ptr(trace) as usize).hash(&mut hasher);
                    trace.annotations.len().hash(&mut hasher);
                    trace.hotspots.len().hash(&mut hasher);
                }
                if let Some(trace) = rt.semantic_trace.as_ref() {
                    (Arc::as_ptr(trace) as usize).hash(&mut hasher);
                    trace.items.len().hash(&mut hasher);
                }
            }
        }
        let debug_trace_fingerprint = hasher.finish();

        let mut collision_hasher = std::collections::hash_map::DefaultHasher::new();
        for &move_index in &self.checks.rapid_collision_move_indices {
            move_index.hash(&mut collision_hasher);
        }
        if let Some(report) = self.checks.collision_report.as_ref() {
            for collision in &report.collisions {
                collision.move_idx.hash(&mut collision_hasher);
            }
        }
        let collision_fingerprint = collision_hasher.finish();

        IssueListCacheKey {
            cut_trace_ptr: self
                .results
                .as_ref()
                .and_then(|results| results.cut_trace.as_ref())
                .map(|trace| Arc::as_ptr(trace) as usize),
            gui_edit_counter: gui.edit_counter,
            debug_trace_fingerprint,
            max_feed_bits: max_feed_mm_min.to_bits(),
            collision_fingerprint,
        }
    }

    pub fn current_issue(
        &mut self,
        gui: &GuiState,
        max_feed_mm_min: f64,
    ) -> Option<SimulationIssue> {
        let issues = self.issues(gui, max_feed_mm_min);
        let index = self.debug.focused_issue_index?;
        issues.get(index).cloned()
    }

    pub fn focus_issue_delta(
        &mut self,
        gui: &GuiState,
        max_feed_mm_min: f64,
        delta: isize,
    ) -> Option<SimulationTraceTarget> {
        let issues = self.issues(gui, max_feed_mm_min);
        if issues.is_empty() {
            self.debug.focused_issue_index = None;
            self.debug.focused_hotspot = None;
            return None;
        }

        let len = issues.len() as isize;
        let current = self.debug.focused_issue_index.map(|index| index as isize);
        let next = match current {
            Some(index) => (index + delta).rem_euclid(len),
            None if delta < 0 => len - 1,
            None => 0,
        } as usize;
        self.debug.focused_issue_index = Some(next);
        let issue = issues.get(next)?.clone();
        self.debug.focused_hotspot = issue.toolpath_id.zip(issue.hotspot_index);

        if let Some(toolpath_id) = issue.toolpath_id {
            if let Some(hotspot_index) = issue.hotspot_index {
                if let Some(item_id) = issue.semantic_item_id {
                    self.pin_semantic_item(toolpath_id, item_id);
                }
                return self.trace_target_for_hotspot(
                    gui,
                    max_feed_mm_min,
                    toolpath_id,
                    hotspot_index,
                );
            }
            if let Some(annotation_index) = issue.annotation_index
                && let Some(rt) = gui.toolpath_rt.get(&toolpath_id)
                && let Some(trace) = rt.debug_trace.as_ref()
                && let Some(annotation) = trace.annotations.get(annotation_index)
            {
                if let Some(item_id) = issue.semantic_item_id {
                    self.pin_semantic_item(toolpath_id, item_id);
                }
                return self.trace_target_for_annotation(toolpath_id, annotation);
            }
            if matches!(
                issue.kind,
                SimulationIssueKind::AirCut | SimulationIssueKind::LowEngagement
            ) {
                if let Some(item_id) = issue.semantic_item_id {
                    self.pin_semantic_item(toolpath_id, item_id);
                }
                return Some(SimulationTraceTarget {
                    toolpath_id,
                    move_index: issue.move_index,
                    semantic_item_id: issue.semantic_item_id,
                    debug_span_id: issue.debug_span_id,
                });
            }
            if let Some(item_id) = issue.semantic_item_id {
                self.pin_semantic_item(toolpath_id, item_id);
                return self.trace_target_for_item(
                    gui,
                    max_feed_mm_min,
                    toolpath_id,
                    item_id,
                    false,
                );
            }
        }

        let toolpath_id = issue.toolpath_id.or_else(|| {
            self.move_to_local_toolpath_move(issue.move_index)
                .map(|(_, id, _)| id)
        })?;

        Some(SimulationTraceTarget {
            toolpath_id,
            move_index: issue.move_index,
            semantic_item_id: issue.semantic_item_id,
            debug_span_id: issue.debug_span_id,
        })
    }

    #[allow(clippy::indexing_slicing)] // item_index from enumerate(), bounded by trace.items
    pub fn pick_semantic_item_with_ray(
        &mut self,
        gui: &GuiState,
        max_feed_mm_min: f64,
        session: &ProjectSession,
        origin: &P3,
        dir: &V3,
    ) -> Option<SimulationTraceTarget> {
        self.sync_debug_state(gui, max_feed_mm_min);
        let mut best_hit: Option<(f64, usize, usize, ToolpathId, u64)> = None;

        for boundary in self.boundaries().to_vec() {
            let Some(rt) = gui.toolpath_rt.get(&boundary.id) else {
                continue;
            };
            let Some(trace) = rt.semantic_trace.as_ref() else {
                continue;
            };
            let Some(index) = self.debug.semantic_indexes.get(&boundary.id) else {
                continue;
            };

            for (item_index, item) in trace.items.iter().enumerate() {
                if item.move_start.is_none() || item.move_end.is_none() {
                    continue;
                }
                let Some(bbox) = self.semantic_item_bbox_in_simulation(session, boundary.id, item)
                else {
                    continue;
                };
                let Some(t) = bbox.ray_intersect(origin, dir) else {
                    continue;
                };
                let depth = index.depths[item_index];
                let move_span = item
                    .move_end
                    .unwrap_or(usize::MAX)
                    .saturating_sub(item.move_start.unwrap_or(0));
                let candidate = (t, depth, move_span, boundary.id, item.id);
                let replace = match best_hit {
                    None => true,
                    Some(current) => {
                        candidate.0 < current.0 - 1e-6
                            || ((candidate.0 - current.0).abs() <= 1e-6
                                && (candidate.1 > current.1
                                    || (candidate.1 == current.1 && candidate.2 < current.2)))
                    }
                };
                if replace {
                    best_hit = Some(candidate);
                }
            }
        }

        let (_, _, _, toolpath_id, item_id) = best_hit?;
        self.trace_target_for_item(gui, max_feed_mm_min, toolpath_id, item_id, false)
    }

    /// Progress within the current toolpath (0.0..1.0).
    pub fn current_toolpath_progress(&self) -> (usize, usize) {
        if let Some(b) = self.current_boundary() {
            let within = self.playback.current_move.saturating_sub(b.start_move);
            let total = b.end_move - b.start_move;
            (within, total)
        } else {
            (0, 0)
        }
    }

    /// Find the nearest checkpoint at or before the given move index.
    pub fn checkpoint_for_move(&self, move_idx: usize) -> Option<usize> {
        let boundaries = self.boundaries();
        let boundary_idx = boundaries.iter().position(|b| move_idx <= b.end_move)?;
        if boundary_idx == 0 {
            return None; // before the first toolpath, use initial stock
        }
        self.checkpoints()
            .iter()
            .position(|c| c.boundary_index == boundary_idx - 1)
    }
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

    fn sync_runtime_profiles(
        &mut self,
        gui: &GuiState,
        boundaries: &[ToolpathBoundary],
        max_feed_mm_min: f64,
    ) {
        let boundary_ids: HashSet<_> = boundaries.iter().map(|boundary| boundary.id).collect();
        self.runtime_profiles
            .retain(|toolpath_id, _| boundary_ids.contains(toolpath_id));

        for toolpath_id in boundary_ids {
            let Some(rt) = gui.toolpath_rt.get(&toolpath_id) else {
                self.runtime_profiles.remove(&toolpath_id);
                continue;
            };
            let Some(result) = rt.result.as_ref() else {
                self.runtime_profiles.remove(&toolpath_id);
                continue;
            };
            let Some(trace) = rt.semantic_trace.as_ref() else {
                self.runtime_profiles.remove(&toolpath_id);
                continue;
            };

            let rapid_feed_mm_min = max_feed_mm_min.max(1.0);
            let needs_rebuild = self
                .runtime_profiles
                .get(&toolpath_id)
                .is_none_or(|profile| {
                    profile.move_count != result.toolpath().moves.len()
                        || profile.trace_item_count != trace.items.len()
                        || (profile.rapid_feed_mm_min - rapid_feed_mm_min).abs() > 1e-6
                });
            if needs_rebuild {
                self.runtime_profiles.insert(
                    toolpath_id,
                    SimulationRuntimeProfile::build(result.toolpath(), trace, rapid_feed_mm_min),
                );
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

impl SimulationRuntimeProfile {
    fn build(toolpath: &Toolpath, trace: &ToolpathSemanticTrace, rapid_feed_mm_min: f64) -> Self {
        let mut cumulative_total_seconds = Vec::with_capacity(toolpath.moves.len() + 1);
        let mut cumulative_cutting_seconds = Vec::with_capacity(toolpath.moves.len() + 1);
        let mut cumulative_rapid_seconds = Vec::with_capacity(toolpath.moves.len() + 1);
        cumulative_total_seconds.push(0.0);
        cumulative_cutting_seconds.push(0.0);
        cumulative_rapid_seconds.push(0.0);

        for move_index in 0..toolpath.moves.len() {
            let metrics = estimate_move_runtime_seconds(toolpath, move_index, rapid_feed_mm_min);
            cumulative_total_seconds
                .push(cumulative_total_seconds.last().copied().unwrap_or_default() + metrics.0);
            cumulative_cutting_seconds.push(
                cumulative_cutting_seconds
                    .last()
                    .copied()
                    .unwrap_or_default()
                    + metrics.1,
            );
            cumulative_rapid_seconds
                .push(cumulative_rapid_seconds.last().copied().unwrap_or_default() + metrics.2);
        }

        Self {
            move_count: toolpath.moves.len(),
            trace_item_count: trace.items.len(),
            rapid_feed_mm_min,
            cumulative_total_seconds,
            cumulative_cutting_seconds,
            cumulative_rapid_seconds,
        }
    }

    #[allow(clippy::indexing_slicing)] // bounds checked: move_end_exclusive <= cumulative.len()-1
    fn metrics_for_range(
        &self,
        move_start: usize,
        move_end_exclusive: usize,
    ) -> Option<SimulationRuntimeMetrics> {
        if move_start >= move_end_exclusive
            || move_end_exclusive > self.cumulative_total_seconds.len() - 1
        {
            return None;
        }
        Some(SimulationRuntimeMetrics {
            total_seconds: self.cumulative_total_seconds[move_end_exclusive]
                - self.cumulative_total_seconds[move_start],
            cutting_seconds: self.cumulative_cutting_seconds[move_end_exclusive]
                - self.cumulative_cutting_seconds[move_start],
            rapid_seconds: self.cumulative_rapid_seconds[move_end_exclusive]
                - self.cumulative_rapid_seconds[move_start],
            move_count: move_end_exclusive - move_start,
        })
    }
}

#[allow(clippy::indexing_slicing)] // move_index bounded by caller's loop over toolpath.moves
fn estimate_move_runtime_seconds(
    toolpath: &Toolpath,
    move_index: usize,
    rapid_feed_mm_min: f64,
) -> (f64, f64, f64) {
    if move_index == 0 {
        return (0.0, 0.0, 0.0);
    }

    let current = &toolpath.moves[move_index];
    let previous = &toolpath.moves[move_index - 1];
    let length_mm = move_length_mm(previous.target, current);
    if length_mm <= 1e-9 {
        return (0.0, 0.0, 0.0);
    }

    match current.move_type {
        MoveType::Rapid => {
            let seconds = (length_mm / rapid_feed_mm_min.max(1.0)) * 60.0;
            (seconds, 0.0, seconds)
        }
        MoveType::Linear { feed_rate }
        | MoveType::ArcCW { feed_rate, .. }
        | MoveType::ArcCCW { feed_rate, .. } => {
            let seconds = (length_mm / feed_rate.max(1.0)) * 60.0;
            (seconds, seconds, 0.0)
        }
    }
}

fn move_length_mm(start: P3, mv: &rs_cam_core::toolpath::Move) -> f64 {
    match mv.move_type {
        MoveType::Rapid | MoveType::Linear { .. } => (mv.target - start).norm(),
        MoveType::ArcCW { i, j, .. } => arc_move_length(start, mv.target, i, j, true),
        MoveType::ArcCCW { i, j, .. } => arc_move_length(start, mv.target, i, j, false),
    }
}

fn arc_move_length(start: P3, end: P3, i: f64, j: f64, clockwise: bool) -> f64 {
    let center_x = start.x + i;
    let center_y = start.y + j;
    let start_angle = (start.y - center_y).atan2(start.x - center_x);
    let end_angle = (end.y - center_y).atan2(end.x - center_x);
    let radius = (i * i + j * j).sqrt();
    if radius <= 1e-9 {
        return (end - start).norm();
    }

    let mut sweep = end_angle - start_angle;
    if clockwise {
        if sweep >= 0.0 {
            sweep -= std::f64::consts::TAU;
        }
    } else if sweep <= 0.0 {
        sweep += std::f64::consts::TAU;
    }

    let arc_xy = radius * sweep.abs();
    let dz = end.z - start.z;
    (arc_xy * arc_xy + dz * dz).sqrt()
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

impl Default for SavedViewportState {
    fn default() -> Self {
        Self {
            show_cutting: true,
            show_rapids: true,
            show_stock: true,
        }
    }
}

/// Display rank, **worst first**. Lower sorts earlier.
///
/// D1 (census §3.5), ruled D-6. This used to run the other way — collisions
/// last, behind hotspots, annotations and per-sample air-cut noise — while
/// `sim_op_list.rs` ranked collisions first. Two contradictory orderings
/// over one list is how a rapid-through-stock ends up below an air-cut run
/// in the panel the operator scans before pressing go.
///
/// The order mirrors the census §3.1 classes: safety, then action-required,
/// then bounded advisories, then the diagnostic-sample tallies that are
/// emission noise by construction.
fn issue_kind_rank(kind: SimulationIssueKind) -> u8 {
    match kind {
        // Class A — physical damage if run.
        SimulationIssueKind::RapidCollision => 0,
        SimulationIssueKind::HolderCollision => 1,
        // Class C — bounded advisories.
        SimulationIssueKind::Hotspot => 2,
        SimulationIssueKind::Annotation => 3,
        // Class D/E — per-run tallies, not defect counts.
        SimulationIssueKind::LowEngagement => 4,
        SimulationIssueKind::AirCut => 5,
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
    use super::*;
    use crate::state::runtime::ToolpathRuntime;
    use rs_cam_core::debug_trace::ToolpathDebugRecorder;
    use rs_cam_core::dexel_stock::StockCutDirection;
    use rs_cam_core::semantic_trace::{
        ToolpathSemanticKind, ToolpathSemanticRecorder, enrich_traces,
    };
    use rs_cam_core::toolpath::Toolpath;
    use std::sync::Arc;

    const TEST_MAX_FEED: f64 = 3000.0;

    fn gui_with_traces() -> GuiState {
        let mut gui = GuiState::new();
        let toolpath_id = rs_cam_core::ToolpathId(1);

        let semantic = ToolpathSemanticRecorder::new("Adaptive", "Adaptive");
        let root = semantic.root_context();
        let pass = root.start_item(ToolpathSemanticKind::Pass, "Pass 1");
        pass.set_move_range(0, 8);
        pass.set_xy_bbox(ToolpathDebugBounds2 {
            min_x: 0.0,
            max_x: 10.0,
            min_y: 0.0,
            max_y: 10.0,
        });
        pass.set_z_range(-1.0, -1.0);
        let entry_item = pass
            .context()
            .start_item(ToolpathSemanticKind::Entry, "Helix entry");
        entry_item.set_move_range(0, 2);
        entry_item.set_xy_bbox(ToolpathDebugBounds2 {
            min_x: 0.0,
            max_x: 4.0,
            min_y: 0.0,
            max_y: 4.0,
        });
        entry_item.set_z_range(-1.0, -1.0);
        entry_item.finish();
        let cleanup = pass
            .context()
            .start_item(ToolpathSemanticKind::Cleanup, "Cleanup");
        cleanup.set_move_range(6, 8);
        cleanup.set_xy_bbox(ToolpathDebugBounds2 {
            min_x: 6.0,
            max_x: 10.0,
            min_y: 6.0,
            max_y: 10.0,
        });
        cleanup.set_z_range(-1.0, -1.0);
        cleanup.finish();
        pass.finish();
        let mut semantic_trace = semantic.finish();

        let debug = ToolpathDebugRecorder::new("Adaptive", "Adaptive");
        let debug_ctx = debug.root_context();
        let pass_span = debug_ctx.start_span("adaptive_pass", "Pass 1");
        pass_span.set_move_range(0, 8);
        pass_span.set_xy_bbox(ToolpathDebugBounds2 {
            min_x: 0.0,
            max_x: 10.0,
            min_y: 0.0,
            max_y: 10.0,
        });
        pass_span.set_z_level(-1.0);
        let pass_span_id = pass_span.id();
        pass_span.finish();
        debug_ctx.add_annotation(1, "Entry");
        debug_ctx.add_annotation(7, "Cleanup");
        debug_ctx.record_hotspot(&rs_cam_core::debug_trace::HotspotRecord {
            kind: "adaptive_pass".into(),
            center_x: 5.0,
            center_y: 5.0,
            z_level: Some(-1.0),
            bucket_size_xy: 10.0,
            bucket_size_z: Some(1.0),
            elapsed_us: 1_000,
            pass_count: 1,
            step_count: 8,
            low_yield_exit_count: 0,
        });
        if let Some(pass_item) = semantic_trace
            .items
            .iter_mut()
            .find(|item| item.label == "Pass 1")
        {
            pass_item.debug_span_id = Some(pass_span_id);
        }
        let mut debug_trace = debug.finish();
        enrich_traces(&mut debug_trace, &mut semantic_trace);
        let semantic_trace = Arc::new(semantic_trace);
        let debug_trace = Arc::new(debug_trace);
        let mut toolpath = Toolpath::new();
        toolpath.rapid_to(P3::new(0.0, 0.0, 5.0));
        toolpath.feed_to(P3::new(0.0, 0.0, -1.0), 300.0);
        toolpath.feed_to(P3::new(2.0, 0.0, -1.0), 1000.0);
        toolpath.feed_to(P3::new(4.0, 0.0, -1.0), 1000.0);
        toolpath.rapid_to(P3::new(4.0, 0.0, 5.0));
        toolpath.rapid_to(P3::new(6.0, 6.0, 5.0));
        toolpath.feed_to(P3::new(6.0, 6.0, -1.0), 300.0);
        toolpath.feed_to(P3::new(8.0, 8.0, -1.0), 1000.0);
        toolpath.rapid_to(P3::new(8.0, 8.0, 5.0));
        let mut rt = ToolpathRuntime::new(true);
        rt.semantic_trace = Some(Arc::clone(&semantic_trace));
        rt.debug_trace = Some(Arc::clone(&debug_trace));
        rt.result = Some(crate::state::toolpath::ToolpathResult {
            annotated: Arc::new(rs_cam_core::toolpath_spans::AnnotatedToolpath::new(
                toolpath,
            )),
            stats: Default::default(),
            debug_trace: Some(debug_trace),
            semantic_trace: Some(semantic_trace),
            debug_trace_path: None,
            drill_op: None,
        });
        gui.toolpath_rt.insert(toolpath_id, rt);
        gui
    }

    fn simulation_for_toolpath() -> SimulationState {
        let mut sim = SimulationState::new();
        sim.results = Some(SimulationResults {
            mesh: StockMesh {
                vertices: Vec::new(),
                indices: Vec::new(),
                colors: Vec::new(),
            },
            total_moves: 9,
            boundaries: vec![ToolpathBoundary {
                id: ToolpathId(1),
                name: "Adaptive".to_owned(),
                tool_name: "6mm End Mill".to_owned(),
                start_move: 0,
                end_move: 8,
                direction: StockCutDirection::FromTop,
            }],
            setup_boundaries: vec![SetupBoundary {
                setup_id: SetupId(1),
                setup_name: "Setup 1".to_owned(),
                start_move: 0,
            }],
            checkpoints: Vec::new(),
            selected_toolpaths: None,
            playback_data: Vec::new(),
            stock_bbox: BoundingBox3 {
                min: P3::new(0.0, 0.0, 0.0),
                max: P3::new(10.0, 10.0, 10.0),
            },
            cut_trace: None,
            cut_trace_path: None,
            column_grid_cell_mm: 0.5,
            prior_stocks: HashMap::new(),
        });
        sim
    }

    /// Attach a fresh cut trace, returning the `Arc` that was stored so cache
    /// sentries can compare identities.
    fn attach_cut_trace(sim: &mut SimulationState) -> Arc<SimulationCutTrace> {
        let trace = rs_cam_core::simulation_cut::SimulationCutTrace::from_samples(
            0.5,
            vec![
                rs_cam_core::simulation_cut::SimulationCutSample {
                    toolpath_id: rs_cam_core::ToolpathId(1),
                    move_index: 1,
                    sample_index: 0,
                    position: [0.0, 0.0, -1.0],
                    cumulative_time_s: 0.2,
                    segment_time_s: 0.2,
                    is_cutting: true,
                    cut_kinematics: rs_cam_core::simulation_cut::CutKinematics::Linear,
                    feed_rate_mm_min: 300.0,
                    spindle_rpm: 18_000,
                    flute_count: 2,
                    axial_doc_mm: 1.0,
                    axial_engagement_mm: 1.0,
                    plunge_descent_mm: 0.0,
                    arc_engagement_radians: Some(std::f64::consts::FRAC_PI_2),
                    chipload_mm_per_tooth: 0.0083,
                    effective_chip_thickness_mm: Some(0.0083),
                    engagement: rs_cam_core::simulation_cut::Engagement::with_radial_woc(0.01),
                    removed_volume_est_mm3: 0.1,
                    mrr_mm3_s: 0.5,
                    semantic_item_id: Some(2),
                    span_path: Vec::new(),
                    in_transit_span: false,
                    source_intent: None,
                },
                rs_cam_core::simulation_cut::SimulationCutSample {
                    toolpath_id: rs_cam_core::ToolpathId(1),
                    move_index: 7,
                    sample_index: 1,
                    position: [8.0, 8.0, -1.0],
                    cumulative_time_s: 0.6,
                    segment_time_s: 0.4,
                    is_cutting: true,
                    cut_kinematics: rs_cam_core::simulation_cut::CutKinematics::Linear,
                    feed_rate_mm_min: 1000.0,
                    spindle_rpm: 18_000,
                    flute_count: 2,
                    axial_doc_mm: 0.4,
                    axial_engagement_mm: 0.4,
                    plunge_descent_mm: 0.0,
                    arc_engagement_radians: Some(std::f64::consts::FRAC_PI_2),
                    chipload_mm_per_tooth: 0.0277,
                    effective_chip_thickness_mm: Some(0.0277),
                    engagement: rs_cam_core::simulation_cut::Engagement::with_radial_woc(0.08),
                    removed_volume_est_mm3: 2.0,
                    mrr_mm3_s: 5.0,
                    semantic_item_id: Some(3),
                    span_path: Vec::new(),
                    in_transit_span: false,
                    source_intent: None,
                },
            ],
        );
        let trace = Arc::new(trace);
        if let Some(results) = sim.results.as_mut() {
            results.cut_trace = Some(Arc::clone(&trace));
        }
        trace
    }

    #[test]
    fn the_issue_list_is_ordered_by_severity_not_by_move_index() {
        // D1 (census §3.5), ruled D-6. RED-FIRST SHAPE: the collision sits at
        // a LATER move than the air-cut run, so under the old ordering
        // (`move_index` primary, kind only as a tiebreak) it sorted BELOW the
        // per-run air-cut noise — and an operator stepping the list with
        // `focus_issue_delta` reached it after the noise, if at all.
        let gui = gui_with_traces();
        let mut sim = simulation_for_toolpath();
        attach_cut_trace(&mut sim);
        sim.checks.rapid_collision_move_indices = vec![8];

        let issues = sim.issues(&gui, TEST_MAX_FEED);
        assert!(
            issues.len() >= 2,
            "fixture must produce a collision AND at least one air-cut run"
        );
        assert_eq!(
            issues[0].kind,
            SimulationIssueKind::RapidCollision,
            "the collision must sort first even though it is at the LAST move; \
             got {:?}",
            issues
                .iter()
                .map(|i| (i.kind, i.move_index))
                .collect::<Vec<_>>()
        );
        // And the air-cut tallies sort last, behind everything curated.
        assert_eq!(
            issues
                .last()
                .map(|i| i.kind)
                .expect("non-empty after the length assertion above"),
            SimulationIssueKind::AirCut
        );
    }

    #[test]
    fn active_semantic_item_prefers_deepest_matching_item() {
        let gui = gui_with_traces();
        let mut sim = simulation_for_toolpath();
        sim.playback.current_move = 1;

        let active = sim
            .active_semantic_item(&gui, TEST_MAX_FEED)
            .expect("active semantic item");
        assert_eq!(active.item.label, "Helix entry");

        sim.playback.current_move = 7;
        let active = sim
            .active_semantic_item(&gui, TEST_MAX_FEED)
            .expect("active semantic item");
        assert_eq!(active.item.label, "Cleanup");
    }

    #[test]
    fn current_debug_annotation_uses_local_toolpath_move() {
        let gui = gui_with_traces();
        let mut sim = simulation_for_toolpath();
        sim.playback.current_move = 7;

        let annotation = sim
            .current_debug_annotation(&gui)
            .expect("annotation for current move");
        assert_eq!(annotation.0, ToolpathId(1));
        assert_eq!(annotation.1.label, "Cleanup");
    }

    #[test]
    fn pinned_semantic_item_overrides_playback_resolution() {
        let gui = gui_with_traces();
        let mut sim = simulation_for_toolpath();
        sim.playback.current_move = 7;
        sim.pin_semantic_item(ToolpathId(1), 2);

        let active = sim
            .active_semantic_item(&gui, TEST_MAX_FEED)
            .expect("pinned semantic item");
        assert_eq!(active.item.label, "Helix entry");

        sim.clear_pinned_semantic_item();
        let active = sim
            .active_semantic_item(&gui, TEST_MAX_FEED)
            .expect("playback semantic item");
        assert_eq!(active.item.label, "Cleanup");
    }

    #[test]
    fn hotspot_target_resolves_move_and_semantic_item() {
        let gui = gui_with_traces();
        let mut sim = simulation_for_toolpath();

        let target = sim
            .trace_target_for_hotspot(&gui, TEST_MAX_FEED, ToolpathId(1), 0)
            .expect("hotspot target");
        assert_eq!(target.toolpath_id, ToolpathId(1));
        assert_eq!(target.move_index, 0);
        assert!(target.semantic_item_id.is_some());
        assert!(target.debug_span_id.is_some());
    }

    #[test]
    fn issue_navigation_prioritizes_hotspots_then_annotations() {
        let gui = gui_with_traces();
        let mut sim = simulation_for_toolpath();

        let first = sim
            .focus_issue_delta(&gui, TEST_MAX_FEED, 1)
            .expect("first issue target");
        assert_eq!(first.move_index, 0);
        assert_eq!(
            sim.current_issue(&gui, TEST_MAX_FEED)
                .expect("focused issue")
                .kind,
            SimulationIssueKind::Hotspot
        );

        let second = sim
            .focus_issue_delta(&gui, TEST_MAX_FEED, 1)
            .expect("second issue target");
        assert_eq!(second.move_index, 1);
        assert_eq!(
            sim.current_issue(&gui, TEST_MAX_FEED)
                .expect("focused issue")
                .kind,
            SimulationIssueKind::Annotation
        );
    }

    #[test]
    fn semantic_pick_prefers_deeper_item_then_smaller_move_span() {
        let gui = gui_with_traces();
        let session = rs_cam_core::session::ProjectSession::new_empty();
        let mut sim = simulation_for_toolpath();

        let target = sim
            .pick_semantic_item_with_ray(
                &gui,
                TEST_MAX_FEED,
                &session,
                &P3::new(2.0, 2.0, 10.0),
                &V3::new(0.0, 0.0, -1.0),
            )
            .expect("semantic pick target");
        assert_eq!(target.toolpath_id, ToolpathId(1));
        assert_eq!(target.semantic_item_id, Some(2));
        assert_eq!(target.move_index, 0);
    }

    #[test]
    fn runtime_hotspots_rank_leaf_semantics_and_metrics_are_available() {
        let gui = gui_with_traces();
        let mut sim = simulation_for_toolpath();

        let hotspots = sim.runtime_hotspots(&gui, TEST_MAX_FEED, ToolpathId(1), 5);
        assert!(!hotspots.is_empty(), "expected runtime hotspots");
        assert!(hotspots[0].total_seconds > 0.0);

        let metrics = sim
            .semantic_runtime_metrics(&gui, TEST_MAX_FEED, ToolpathId(1), 2)
            .expect("runtime metrics for entry item");
        assert!(metrics.total_seconds > 0.0);
        assert!(metrics.cutting_seconds > 0.0);
    }

    #[test]
    fn cut_trace_surfaces_current_sample_and_cutting_issues() {
        let gui = gui_with_traces();
        let mut sim = simulation_for_toolpath();
        attach_cut_trace(&mut sim);
        sim.playback.current_move = 7;

        let sample = sim.current_cut_sample().expect("current cut sample");
        assert_eq!(sample.toolpath_id, ToolpathId(1));
        assert_eq!(sample.sample.move_index, 7);

        let issues = sim.issues(&gui, TEST_MAX_FEED);
        assert!(
            issues
                .iter()
                .any(|issue| issue.kind == SimulationIssueKind::AirCut)
        );
        assert!(
            issues
                .iter()
                .any(|issue| issue.kind == SimulationIssueKind::LowEngagement)
        );
    }

    // ── Cache-key soundness (RESEARCH_f2_and_aba.md, Topic B) ─────────────

    /// Sentinel poked into the cached triage. A cache **hit** returns the
    /// same object and carries it out; a **rebuild** replaces `triage`
    /// wholesale and wipes it. This is the hit/miss witness these sentries
    /// use — the triage's own content cannot serve, because two different
    /// traces may legitimately triage identically.
    const POISON: usize = usize::MAX;

    fn poison(sim: &mut SimulationState) {
        sim.debug.triage_cache.triage.counts.samples_total = POISON;
    }

    //
    // The four viz caches below used to key on `Arc::as_ptr(trace) as usize`
    // — three of them paired with the GUI edit counter, `SpanAggregateCache`
    // with nothing at all. Neither component closes the ABA window: the
    // reachable gesture is `invalidate_simulation` (`controller::events::
    // simulation`), which sets `results = None` — freeing the trace with
    // nothing replacing it — and bumps no counter and clears no cache.
    // Every `ArcInner<SimulationCutTrace>` is the same fixed size, so a
    // re-simulate after that free is a same-size-class allocation and
    // same-address reuse is likely rather than unlikely.

    /// The property that makes the address question moot: while a cache holds
    /// a `Weak`, the freed allocation stays reserved, so no replacement can
    /// be handed that address and a stale key cannot false-hit.
    ///
    /// The `assert_ne!` is the load-bearing half — it is exactly the
    /// comparison the old `usize` key performed, and it is guaranteed here
    /// only *because* the `Weak` is still alive.
    #[test]
    fn a_weak_key_pins_the_freed_address_so_it_cannot_false_hit() {
        let mut sim = simulation_for_toolpath();
        let trace = attach_cut_trace(&mut sim);
        let freed_addr = Arc::as_ptr(&trace) as usize;
        let stored: Weak<SimulationCutTrace> = Arc::downgrade(&trace);
        drop(trace);
        sim.results = None; // the `invalidate_simulation` gesture

        assert!(
            stored.upgrade().is_none(),
            "fixture must actually drop the trace"
        );
        let mut replacements: Vec<Arc<SimulationCutTrace>> = Vec::new();
        for _ in 0..64 {
            let mut next = simulation_for_toolpath();
            let replacement = attach_cut_trace(&mut next);
            assert_ne!(
                Arc::as_ptr(&replacement) as usize,
                freed_addr,
                "a live Weak must reserve the freed allocation; the old \
                 `Arc::as_ptr as usize` key had no such guarantee"
            );
            assert!(
                !weak_matches(Some(&stored), Some(&replacement)),
                "a dead Weak must never match a live Arc"
            );
            replacements.push(replacement);
        }
        // …and the arm that must survive: "no trace" is a cached state.
        assert!(weak_matches(None::<&Weak<SimulationCutTrace>>, None));
        assert!(!weak_matches(None, replacements.first()));
    }

    /// The ABA scenario end to end: cache the triage, invalidate the
    /// simulation (no edit-counter bump, no cache clear), re-simulate, and
    /// ask again at the *same* edit counter. The cache must miss.
    ///
    /// Witness of the miss is the stored key itself: `trace` is written only
    /// on a rebuild, so a hit would have left the dead `Weak` in place.
    #[test]
    fn invalidate_then_resimulate_misses_the_triage_cache() {
        let session = rs_cam_core::session::ProjectSession::new_empty();
        let mut sim = simulation_for_toolpath();
        let first = attach_cut_trace(&mut sim);
        let first_addr = Arc::as_ptr(&first) as usize;
        drop(first); // only `sim.results` holds the trace, as in the GUI

        let _ = sim.cached_simulation_triage(&session, 7);
        assert!(sim.debug.triage_cache.built);

        // A second ask at the same version is a hit — the cache still earns
        // its keep after the key change. Witnessed by a sentinel poked into
        // the cached value: a rebuild replaces the whole `triage`.
        poison(&mut sim);
        assert_eq!(
            sim.cached_simulation_triage(&session, 7)
                .counts
                .samples_total,
            POISON,
            "an unchanged project must still hit the cache"
        );

        // `invalidate_simulation`: results dropped, counter untouched.
        sim.results = None;
        let mut refreshed = simulation_for_toolpath();
        let second = attach_cut_trace(&mut refreshed);
        assert_ne!(
            Arc::as_ptr(&second) as usize,
            first_addr,
            "the cache's Weak reserves the freed address"
        );
        sim.results = refreshed.results.take();

        assert_ne!(
            sim.cached_simulation_triage(&session, 7)
                .counts
                .samples_total,
            POISON,
            "the invalidate → re-simulate gesture must MISS the cache"
        );
        let stored_is_second = sim
            .debug
            .triage_cache
            .trace
            .as_ref()
            .and_then(|w| w.upgrade())
            .is_some_and(|up| Arc::ptr_eq(&up, &second));
        assert!(
            stored_is_second,
            "the triage must have been rebuilt against the new trace"
        );
    }

    /// §B.3 — the larger hole, and it is not ABA: the triage is built from
    /// evidence that is not the trace, and the holder-collision report
    /// arrives from a *separate async job* with the same trace, the same edit
    /// counter and no cache invalidation. Each arm below moves one
    /// `project_evidence` input and must invalidate.
    #[test]
    fn evidence_movement_invalidates_the_cached_triage() {
        let session = rs_cam_core::session::ProjectSession::new_empty();
        let mut sim = simulation_for_toolpath();
        attach_cut_trace(&mut sim);

        let _ = sim.cached_simulation_triage(&session, 3);
        let baseline_fp = sim.debug.triage_cache.evidence_fp;
        poison(&mut sim);
        assert_eq!(
            sim.cached_simulation_triage(&session, 3)
                .counts
                .samples_total,
            POISON,
            "an unchanged project must not rebuild"
        );

        // (a) the async holder-collision report lands.
        sim.checks.collision_report = Some(CollisionReport {
            collisions: vec![rs_cam_core::collision::CollisionEvent {
                move_idx: 4,
                position: P3::new(1.0, 1.0, -1.0),
                penetration_depth: 0.8,
                segment: "holder".to_owned(),
                kind: rs_cam_core::collision::CollisionKind::Workpiece,
            }],
            min_safe_stickout: 42.0,
        });
        sim.checks.holder_collision_count = 1;
        assert_ne!(
            sim.cached_simulation_triage(&session, 3)
                .counts
                .samples_total,
            POISON,
            "the holder report must force a rebuild"
        );
        let after_holder = sim.debug.triage_cache.evidence_fp;
        assert_ne!(
            after_holder, baseline_fp,
            "a holder report arriving must invalidate the triage — it feeds \
             `ProjectEvidence::holder_collisions` and moves no other key part"
        );

        // (b) a rapid-through-stock collision list change.
        sim.checks.rapid_collisions = vec![RapidCollision {
            move_index: 5,
            start: P3::new(0.0, 0.0, 5.0),
            end: P3::new(9.0, 9.0, 5.0),
        }];
        sim.checks.rapid_collision_move_indices = vec![5];
        let _ = sim.cached_simulation_triage(&session, 3);
        let after_rapids = sim.debug.triage_cache.evidence_fp;
        assert_ne!(after_rapids, after_holder, "rapid collisions must be keyed");

        // (c) the simulated cell size, which enriches a measurability
        // abstention's reason — the field today's sole consumer renders.
        sim.resolution *= 2.0;
        let _ = sim.cached_simulation_triage(&session, 3);
        assert_ne!(
            sim.debug.triage_cache.evidence_fp, after_rapids,
            "resolution_mm must be keyed"
        );
    }

    /// The sibling caches carry the same key shape, and `SpanAggregateCache`
    /// carried the strictest form of the bug (bare pointer, no counter).
    /// Rebuilding it against a *different* trace after the first was freed
    /// must produce the new trace's aggregates, not the old ones.
    #[test]
    fn span_aggregate_cache_rebuilds_after_its_trace_is_freed() {
        let mut sim = simulation_for_toolpath();
        let first = attach_cut_trace(&mut sim);
        sim.debug.span_aggregates.ensure_built(&first);
        assert_eq!(
            sim.debug
                .span_aggregates
                .cutting_indices_for(ToolpathId(1))
                .len(),
            2,
            "fixture must have cutting samples to make the sentry non-vacuous"
        );
        drop(first);
        sim.results = None;

        // A shorter replacement: a false hit would keep the two-sample
        // grouping of the freed trace.
        let mut refreshed = simulation_for_toolpath();
        let full = attach_cut_trace(&mut refreshed);
        let short = Arc::new(
            rs_cam_core::simulation_cut::SimulationCutTrace::from_samples(
                0.5,
                full.samples.iter().take(1).cloned().collect(),
            ),
        );
        sim.debug.span_aggregates.ensure_built(&short);
        assert_eq!(
            sim.debug
                .span_aggregates
                .cutting_indices_for(ToolpathId(1))
                .len(),
            1,
            "the cache must reflect the trace it was last given"
        );
    }

    /// Load-report and chipload-envelope caches: a hit must still be a hit
    /// (the key change is strictly narrowing, and both are per-frame paths),
    /// and both must miss once their trace is replaced.
    #[test]
    fn load_report_and_envelope_caches_hit_then_miss_on_a_new_trace() {
        let session = rs_cam_core::session::ProjectSession::new_empty();
        let mut sim = simulation_for_toolpath();
        let first = attach_cut_trace(&mut sim);
        drop(first);

        let _ = sim.cached_load_report(&session, 1);
        let _ = sim.cached_chipload_envelopes(&session, 1);
        assert!(sim.debug.load_report_cache.report.is_some());
        assert!(sim.debug.chipload_envelope_cache.envelopes.is_some());
        let live = sim
            .results
            .as_ref()
            .and_then(|r| r.cut_trace.as_ref())
            .cloned();
        assert!(
            weak_matches(sim.debug.load_report_cache.trace.as_ref(), live.as_ref()),
            "the stored key must match the trace it was built from"
        );
        drop(live);

        sim.results = None;
        let mut refreshed = simulation_for_toolpath();
        attach_cut_trace(&mut refreshed);
        sim.results = refreshed.results.take();
        let live = sim
            .results
            .as_ref()
            .and_then(|r| r.cut_trace.as_ref())
            .cloned();
        assert!(
            !weak_matches(sim.debug.load_report_cache.trace.as_ref(), live.as_ref()),
            "a freed trace's key must not answer for its replacement"
        );
        assert!(
            !weak_matches(
                sim.debug.chipload_envelope_cache.trace.as_ref(),
                live.as_ref()
            ),
            "a freed trace's key must not answer for its replacement"
        );

        let _ = sim.cached_load_report(&session, 1);
        assert!(
            weak_matches(sim.debug.load_report_cache.trace.as_ref(), live.as_ref()),
            "the rebuild must re-key against the live trace"
        );
    }
}
