use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use super::job::{JobState, SetupId};
use super::toolpath::ToolpathId;
use rs_cam_core::collision::{CollisionReport, RapidCollision};
use rs_cam_core::debug_trace::{ToolpathDebugAnnotation, ToolpathDebugBounds2};
use rs_cam_core::dexel_stock::{StockCutDirection, TriDexelStock};
use rs_cam_core::geo::{BoundingBox3, P3, V3};
use rs_cam_core::semantic_trace::{
    ToolpathSemanticItem, ToolpathSemanticKind, ToolpathSemanticTrace,
};
use rs_cam_core::simulation_cut::{
    SimulationCutHotspot, SimulationCutIssue, SimulationCutIssueKind, SimulationCutSample,
    SimulationCutTrace, SimulationMetricOptions, SimulationSemanticCutSummary,
};
use rs_cam_core::stock_mesh::StockMesh;
use rs_cam_core::toolpath::{MoveType, Toolpath};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SimulationDebugTab {
    #[default]
    Semantic,
    Generation,
    Cutting,
    Trace,
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
    pub drawer_open: bool,
    pub active_tab: SimulationDebugTab,
    pub expanded_toolpaths: HashSet<ToolpathId>,
    pub focused_hotspot: Option<(ToolpathId, usize)>,
    pub pinned_semantic_item: Option<(ToolpathId, u64)>,
    pub focused_issue_index: Option<usize>,
    pub highlight_active_item: bool,
    pub pending_inspect_toolpath: Option<ToolpathId>,
    pub(crate) semantic_indexes: HashMap<ToolpathId, SimulationSemanticIndex>,
    runtime_profiles: HashMap<ToolpathId, SimulationRuntimeProfile>,
}

/// How the simulation stock mesh is colored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StockVizMode {
    /// Default wood-tone gradient.
    Solid,
    /// Green/yellow/red/blue deviation from model surface.
    Deviation,
    /// Colored by which operation removed material.
    ByOperation,
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
pub struct SimCheckpoint {
    pub boundary_index: usize,
    pub mesh: StockMesh,
    /// The tri-dexel stock at this checkpoint (for resuming incremental sim).
    pub stock: Option<TriDexelStock>,
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
    /// Pre-transformed toolpath data for incremental playback.
    /// Each entry: (toolpath in global stock frame, tool_config, direction).
    pub playback_data: Vec<(
        std::sync::Arc<rs_cam_core::toolpath::Toolpath>,
        super::job::ToolConfig,
        StockCutDirection,
    )>,
    /// Global stock bounding box used for this simulation (for fresh-stock reset).
    pub stock_bbox: rs_cam_core::geo::BoundingBox3,
    /// Simulation-time cutting metrics captured during the run.
    pub cut_trace: Option<Arc<SimulationCutTrace>>,
    /// Artifact path for the simulation cutting metrics trace.
    pub cut_trace_path: Option<PathBuf>,
}

/// Transport / playback state — independent of whether results exist.
pub struct SimulationPlayback {
    /// Animation playback state.
    pub playing: bool,
    /// Current move index for timeline scrubbing.
    pub current_move: usize,
    /// Playback speed (moves per second).
    pub speed: f32,
    /// Tool position during playback (X, Y, Z).
    pub tool_position: Option<[f64; 3]>,
    /// Tool radius for the current operation during playback.
    pub tool_radius: f64,
    /// Tool type label for current operation during playback.
    pub tool_type_label: String,
    /// Live tri-dexel stock for incremental playback simulation.
    pub live_stock: Option<TriDexelStock>,
    /// Move index the live heightmap has been simulated up to.
    pub live_sim_move: usize,
    /// Current display mesh (may differ from final mesh during scrubbing).
    pub display_mesh: Option<StockMesh>,
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
    /// Stock visualization mode.
    pub stock_viz_mode: StockVizMode,
    /// Stock opacity (0.0 = transparent, 1.0 = solid).
    pub stock_opacity: f32,
    /// Saved viewport state from editor mode (restored on exit).
    pub saved_viewport: SavedViewportState,
    /// Runtime-only debugger state and semantic lookup cache.
    pub debug: SimulationDebugState,
}

impl SimulationState {
    pub fn new() -> Self {
        Self {
            results: None,
            playback: SimulationPlayback {
                playing: false,
                current_move: 0,
                speed: 500.0,
                tool_position: None,
                tool_radius: 0.0,
                tool_type_label: String::new(),
                live_stock: None,
                live_sim_move: 0,
                display_mesh: None,
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
            stock_viz_mode: StockVizMode::Solid,
            stock_opacity: 1.0,
            saved_viewport: SavedViewportState {
                show_cutting: true,
                show_rapids: true,
                show_stock: true,
            },
            debug: SimulationDebugState {
                enabled: false,
                drawer_open: false,
                active_tab: SimulationDebugTab::Semantic,
                expanded_toolpaths: HashSet::new(),
                focused_hotspot: None,
                pinned_semantic_item: None,
                focused_issue_index: None,
                highlight_active_item: true,
                pending_inspect_toolpath: None,
                semantic_indexes: HashMap::new(),
                runtime_profiles: HashMap::new(),
            },
        }
    }

    // --- Convenience accessors ---

    /// Whether simulation results exist.
    pub fn has_results(&self) -> bool {
        self.results.is_some()
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
    pub fn advance(&mut self, dt: f32) -> bool {
        let total = self.total_moves();
        if !self.playback.playing || self.playback.current_move >= total {
            return false;
        }
        let advance = (self.playback.speed * dt) as usize;
        self.playback.current_move = (self.playback.current_move + advance.max(1)).min(total);
        if self.playback.current_move >= total {
            self.playback.playing = false;
        }
        true
    }

    /// Find which toolpath boundary contains the current move.
    pub fn current_boundary(&self) -> Option<&ToolpathBoundary> {
        let current = self.playback.current_move;
        self.boundaries()
            .iter()
            .find(|b| current >= b.start_move && current <= b.end_move)
    }

    pub fn current_boundary_index(&self) -> Option<usize> {
        let current = self.playback.current_move;
        self.boundaries()
            .iter()
            .position(|b| current >= b.start_move && current <= b.end_move)
    }

    #[allow(clippy::indexing_slicing)] // boundary_index from position() is always in bounds
    pub fn move_to_local_toolpath_move(
        &self,
        move_idx: usize,
    ) -> Option<(usize, ToolpathId, usize)> {
        let boundary_index = self.boundaries().iter().position(|boundary| {
            move_idx >= boundary.start_move && move_idx <= boundary.end_move
        })?;
        let boundary = &self.boundaries()[boundary_index];
        let local_move = move_idx.saturating_sub(boundary.start_move);
        Some((boundary_index, boundary.id, local_move))
    }

    pub fn current_local_toolpath_move(&self) -> Option<(usize, ToolpathId, usize)> {
        self.move_to_local_toolpath_move(self.playback.current_move)
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
        job: &JobState,
        toolpath_id: ToolpathId,
    ) -> ToolpathTraceAvailability {
        let Some(toolpath) = job.find_toolpath(toolpath_id) else {
            return ToolpathTraceAvailability::None;
        };

        let has_perf = toolpath.debug_trace.is_some();
        let has_semantic = toolpath.semantic_trace.is_some();
        let has_partial_only = toolpath.result.is_none() && (has_perf || has_semantic);

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

    pub fn sync_debug_state(&mut self, job: &JobState) {
        let boundaries = self.boundaries().to_vec();
        self.debug.sync_semantic_indexes(job, &boundaries);
        self.debug.sync_runtime_profiles(job, &boundaries);
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
        job: &JobState,
        toolpath_id: ToolpathId,
        item_id: u64,
    ) -> Option<SimulationRuntimeMetrics> {
        self.sync_debug_state(job);
        let toolpath = job.find_toolpath(toolpath_id)?;
        let trace = toolpath.semantic_trace.as_ref()?;
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
            .find(|summary| summary.toolpath_id == toolpath_id.0)
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
                summary.toolpath_id == toolpath_id.0 && summary.semantic_item_id == item_id
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
            .filter(|summary| summary.toolpath_id == toolpath_id.0)
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
            .filter(|hotspot| hotspot.toolpath_id == toolpath_id.0)
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
            .filter(|sample| sample.toolpath_id == toolpath_id.0 && sample.move_index <= local_move)
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
        job: &JobState,
        toolpath_id: ToolpathId,
        limit: usize,
    ) -> Vec<SimulationRuntimeHotspot> {
        self.sync_debug_state(job);
        let Some(toolpath) = job.find_toolpath(toolpath_id) else {
            return Vec::new();
        };
        let Some(trace) = toolpath.semantic_trace.as_ref() else {
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
    pub fn playback_semantic_item(&mut self, job: &JobState) -> Option<ActiveSemanticItem> {
        let (boundary_index, toolpath_id, local_move) = self.current_local_toolpath_move()?;
        self.sync_debug_state(job);
        let toolpath = job.find_toolpath(toolpath_id)?;
        let trace = toolpath.semantic_trace.as_ref()?;
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
        job: &JobState,
        toolpath_id: ToolpathId,
        item_id: u64,
    ) -> Option<ActiveSemanticItem> {
        self.sync_debug_state(job);
        let toolpath = job.find_toolpath(toolpath_id)?;
        let trace = toolpath.semantic_trace.as_ref()?;
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

    pub fn active_semantic_item(&mut self, job: &JobState) -> Option<ActiveSemanticItem> {
        self.sync_debug_state(job);
        if let Some((toolpath_id, item_id)) = self.debug.pinned_semantic_item
            && let Some(active) = self.semantic_item_by_id(job, toolpath_id, item_id)
        {
            return Some(active);
        }
        self.playback_semantic_item(job)
    }

    pub fn pin_semantic_item(&mut self, toolpath_id: ToolpathId, item_id: u64) {
        self.debug.pinned_semantic_item = Some((toolpath_id, item_id));
    }

    pub fn clear_pinned_semantic_item(&mut self) {
        self.debug.pinned_semantic_item = None;
    }

    pub fn active_debug_span(
        &mut self,
        job: &JobState,
    ) -> Option<(ToolpathId, rs_cam_core::debug_trace::ToolpathDebugSpan)> {
        let active = self.active_semantic_item(job)?;
        let toolpath = job.find_toolpath(active.toolpath_id)?;
        let trace = toolpath.debug_trace.as_ref()?;
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
        job: &JobState,
        toolpath_id: ToolpathId,
        item_id: u64,
        prefer_end: bool,
    ) -> Option<SimulationTraceTarget> {
        let active = self.semantic_item_by_id(job, toolpath_id, item_id)?;
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
        job: &JobState,
        toolpath_id: ToolpathId,
        span_id: u64,
        prefer_end: bool,
    ) -> Option<SimulationTraceTarget> {
        let toolpath = job.find_toolpath(toolpath_id)?;
        let debug_trace = toolpath.debug_trace.as_ref()?;
        let span = debug_trace.spans.iter().find(|span| span.id == span_id)?;
        if let (Some(move_start), Some(move_end)) = (span.move_start, span.move_end) {
            return Some(SimulationTraceTarget {
                toolpath_id,
                move_index: self.global_move_for_local(
                    toolpath_id,
                    if prefer_end { move_end } else { move_start },
                )?,
                semantic_item_id: toolpath.semantic_trace.as_ref().and_then(|trace| {
                    trace
                        .items
                        .iter()
                        .find(|item| item.debug_span_id == Some(span_id))
                        .map(|item| item.id)
                }),
                debug_span_id: Some(span_id),
            });
        }

        let semantic_item_id = toolpath.semantic_trace.as_ref().and_then(|trace| {
            trace
                .items
                .iter()
                .find(|item| item.debug_span_id == Some(span_id))
                .map(|item| item.id)
        })?;
        self.trace_target_for_item(job, toolpath_id, semantic_item_id, prefer_end)
    }

    pub fn trace_target_for_hotspot(
        &mut self,
        job: &JobState,
        toolpath_id: ToolpathId,
        hotspot_index: usize,
    ) -> Option<SimulationTraceTarget> {
        let toolpath = job.find_toolpath(toolpath_id)?;
        let debug_trace = toolpath.debug_trace.as_ref()?;
        let hotspot = debug_trace.hotspots.get(hotspot_index)?;
        if let Some(item_id) = hotspot.semantic_item_id {
            return self.trace_target_for_item(job, toolpath_id, item_id, false);
        }
        if let (Some(move_start), Some(_)) = (hotspot.move_start, hotspot.move_end) {
            return Some(SimulationTraceTarget {
                toolpath_id,
                move_index: self.global_move_for_local(toolpath_id, move_start)?,
                semantic_item_id: None,
                debug_span_id: hotspot.representative_span_id,
            });
        }
        hotspot
            .representative_span_id
            .and_then(|span_id| self.trace_target_for_span(job, toolpath_id, span_id, false))
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
        _job: &JobState,
        issue: &SimulationCutIssue,
    ) -> Option<SimulationTraceTarget> {
        let toolpath_id = ToolpathId(issue.toolpath_id);
        Some(SimulationTraceTarget {
            toolpath_id,
            move_index: self.global_move_for_local(toolpath_id, issue.move_index)?,
            semantic_item_id: issue.semantic_item_id,
            debug_span_id: None,
        })
    }

    pub fn current_debug_annotation_with_index(
        &self,
        job: &JobState,
    ) -> Option<(ToolpathId, usize, ToolpathDebugAnnotation)> {
        let (_, toolpath_id, local_move) = self.current_local_toolpath_move()?;
        let toolpath = job.find_toolpath(toolpath_id)?;
        let trace = toolpath.debug_trace.as_ref()?;
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
        job: &JobState,
    ) -> Option<(ToolpathId, ToolpathDebugAnnotation)> {
        self.current_debug_annotation_with_index(job)
            .map(|(toolpath_id, _, annotation)| (toolpath_id, annotation))
    }

    pub fn current_item_bbox(
        &mut self,
        job: &JobState,
    ) -> Option<(ToolpathId, ToolpathDebugBounds2, f64, f64)> {
        let active = self.active_semantic_item(job)?;
        let bbox = self.semantic_item_bbox_in_simulation(job, active.toolpath_id, &active.item)?;
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
        job: &JobState,
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
        let setup = job
            .setup_of_toolpath(toolpath_id)
            .and_then(|setup_id| job.setups.iter().find(|setup| setup.id == setup_id));
        Some(BoundingBox3::from_points(local_corners.into_iter().map(
            |corner| {
                setup.map_or(corner, |setup| {
                    setup.inverse_transform_point(corner, &job.stock)
                })
            },
        )))
    }

    pub fn issues(&mut self, job: &JobState) -> Vec<SimulationIssue> {
        self.sync_debug_state(job);
        let mut issues = Vec::new();

        for boundary in self.boundaries().to_vec() {
            let Some(toolpath) = job.find_toolpath(boundary.id) else {
                continue;
            };
            if let Some(trace) = toolpath.debug_trace.as_ref() {
                for (annotation_index, annotation) in trace.annotations.iter().enumerate() {
                    issues.push(SimulationIssue {
                        kind: SimulationIssueKind::Annotation,
                        toolpath_id: Some(boundary.id),
                        move_index: boundary.start_move + annotation.move_index,
                        label: annotation.label.clone(),
                        semantic_item_id: toolpath.semantic_trace.as_ref().and_then(
                            |semantic_trace| {
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
                            },
                        ),
                        debug_span_id: None,
                        hotspot_index: None,
                        annotation_index: Some(annotation_index),
                    });
                }

                for (hotspot_index, hotspot) in trace.hotspots.iter().enumerate() {
                    let Some(target) =
                        self.trace_target_for_hotspot(job, boundary.id, hotspot_index)
                    else {
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
                let toolpath_id = ToolpathId(issue.toolpath_id);
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

        issues.sort_by(|left, right| {
            left.move_index
                .cmp(&right.move_index)
                .then_with(|| issue_kind_rank(left.kind).cmp(&issue_kind_rank(right.kind)))
                .then_with(|| left.label.cmp(&right.label))
        });
        issues
    }

    pub fn current_issue(&mut self, job: &JobState) -> Option<SimulationIssue> {
        let issues = self.issues(job);
        let index = self.debug.focused_issue_index?;
        issues.get(index).cloned()
    }

    pub fn focus_issue_delta(
        &mut self,
        job: &JobState,
        delta: isize,
    ) -> Option<SimulationTraceTarget> {
        let issues = self.issues(job);
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
                return self.trace_target_for_hotspot(job, toolpath_id, hotspot_index);
            }
            if let Some(annotation_index) = issue.annotation_index
                && let Some(toolpath) = job.find_toolpath(toolpath_id)
                && let Some(trace) = toolpath.debug_trace.as_ref()
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
                return self.trace_target_for_item(job, toolpath_id, item_id, false);
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
        job: &JobState,
        origin: &P3,
        dir: &V3,
    ) -> Option<SimulationTraceTarget> {
        self.sync_debug_state(job);
        let mut best_hit: Option<(f64, usize, usize, ToolpathId, u64)> = None;

        for boundary in self.boundaries().to_vec() {
            let Some(toolpath) = job.find_toolpath(boundary.id) else {
                continue;
            };
            let Some(trace) = toolpath.semantic_trace.as_ref() else {
                continue;
            };
            let Some(index) = self.debug.semantic_indexes.get(&boundary.id) else {
                continue;
            };

            for (item_index, item) in trace.items.iter().enumerate() {
                if item.move_start.is_none() || item.move_end.is_none() {
                    continue;
                }
                let Some(bbox) = self.semantic_item_bbox_in_simulation(job, boundary.id, item)
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
        self.trace_target_for_item(job, toolpath_id, item_id, false)
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

    fn sync_semantic_indexes(&mut self, job: &JobState, boundaries: &[ToolpathBoundary]) {
        let boundary_ids: HashSet<_> = boundaries.iter().map(|boundary| boundary.id).collect();
        self.semantic_indexes
            .retain(|toolpath_id, _| boundary_ids.contains(toolpath_id));
        self.expanded_toolpaths
            .retain(|toolpath_id| boundary_ids.contains(toolpath_id));

        for toolpath_id in boundary_ids {
            let Some(trace) = job
                .find_toolpath(toolpath_id)
                .and_then(|toolpath| toolpath.semantic_trace.as_ref())
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

    fn sync_runtime_profiles(&mut self, job: &JobState, boundaries: &[ToolpathBoundary]) {
        let boundary_ids: HashSet<_> = boundaries.iter().map(|boundary| boundary.id).collect();
        self.runtime_profiles
            .retain(|toolpath_id, _| boundary_ids.contains(toolpath_id));

        for toolpath_id in boundary_ids {
            let Some(toolpath) = job.find_toolpath(toolpath_id) else {
                self.runtime_profiles.remove(&toolpath_id);
                continue;
            };
            let Some(result) = toolpath.result.as_ref() else {
                self.runtime_profiles.remove(&toolpath_id);
                continue;
            };
            let Some(trace) = toolpath.semantic_trace.as_ref() else {
                self.runtime_profiles.remove(&toolpath_id);
                continue;
            };

            let rapid_feed_mm_min = job.machine.max_feed_mm_min.max(1.0);
            let needs_rebuild = self
                .runtime_profiles
                .get(&toolpath_id)
                .is_none_or(|profile| {
                    profile.move_count != result.toolpath.moves.len()
                        || profile.trace_item_count != trace.items.len()
                        || (profile.rapid_feed_mm_min - rapid_feed_mm_min).abs() > 1e-6
                });
            if needs_rebuild {
                self.runtime_profiles.insert(
                    toolpath_id,
                    SimulationRuntimeProfile::build(
                        result.toolpath.as_ref(),
                        trace,
                        rapid_feed_mm_min,
                    ),
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
            speed: 500.0,
            tool_position: None,
            tool_radius: 0.0,
            tool_type_label: String::new(),
            live_stock: None,
            live_sim_move: 0,
            display_mesh: None,
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

fn issue_kind_rank(kind: SimulationIssueKind) -> u8 {
    match kind {
        SimulationIssueKind::Hotspot => 0,
        SimulationIssueKind::Annotation => 1,
        SimulationIssueKind::AirCut => 2,
        SimulationIssueKind::LowEngagement => 3,
        SimulationIssueKind::RapidCollision => 4,
        SimulationIssueKind::HolderCollision => 5,
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
    use crate::state::job::{JobState, ModelId, ToolId};
    use crate::state::toolpath::{OperationType, ToolpathEntry};
    use rs_cam_core::debug_trace::ToolpathDebugRecorder;
    use rs_cam_core::dexel_stock::StockCutDirection;
    use rs_cam_core::semantic_trace::{
        ToolpathSemanticKind, ToolpathSemanticRecorder, enrich_traces,
    };
    use rs_cam_core::toolpath::Toolpath;
    use std::sync::Arc;

    fn job_with_traces() -> JobState {
        let mut job = JobState::new();
        let toolpath_id = ToolpathId(1);
        let mut entry = ToolpathEntry::for_operation(
            toolpath_id,
            "Adaptive".to_owned(),
            ToolId(1),
            ModelId(1),
            OperationType::Adaptive,
        );

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
        entry.semantic_trace = Some(Arc::new(semantic_trace));
        entry.debug_trace = Some(Arc::new(debug_trace));
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
        entry.result = Some(crate::state::toolpath::ToolpathResult {
            toolpath: Arc::new(toolpath),
            stats: Default::default(),
            debug_trace: entry.debug_trace.clone(),
            semantic_trace: entry.semantic_trace.clone(),
            debug_trace_path: None,
        });

        job.push_toolpath(entry);
        job
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
        });
        sim
    }

    fn attach_cut_trace(sim: &mut SimulationState) {
        let trace = rs_cam_core::simulation_cut::SimulationCutTrace::from_samples(
            0.5,
            vec![
                rs_cam_core::simulation_cut::SimulationCutSample {
                    toolpath_id: 1,
                    move_index: 1,
                    sample_index: 0,
                    position: [0.0, 0.0, -1.0],
                    cumulative_time_s: 0.2,
                    segment_time_s: 0.2,
                    is_cutting: true,
                    feed_rate_mm_min: 300.0,
                    spindle_rpm: 18_000,
                    flute_count: 2,
                    axial_doc_mm: 1.0,
                    radial_engagement: 0.01,
                    chipload_mm_per_tooth: 0.0083,
                    removed_volume_est_mm3: 0.1,
                    mrr_mm3_s: 0.5,
                    semantic_item_id: Some(2),
                },
                rs_cam_core::simulation_cut::SimulationCutSample {
                    toolpath_id: 1,
                    move_index: 7,
                    sample_index: 1,
                    position: [8.0, 8.0, -1.0],
                    cumulative_time_s: 0.6,
                    segment_time_s: 0.4,
                    is_cutting: true,
                    feed_rate_mm_min: 1000.0,
                    spindle_rpm: 18_000,
                    flute_count: 2,
                    axial_doc_mm: 0.4,
                    radial_engagement: 0.08,
                    chipload_mm_per_tooth: 0.0277,
                    removed_volume_est_mm3: 2.0,
                    mrr_mm3_s: 5.0,
                    semantic_item_id: Some(3),
                },
            ],
        );
        if let Some(results) = sim.results.as_mut() {
            results.cut_trace = Some(Arc::new(trace));
        }
    }

    #[test]
    fn active_semantic_item_prefers_deepest_matching_item() {
        let job = job_with_traces();
        let mut sim = simulation_for_toolpath();
        sim.playback.current_move = 1;

        let active = sim
            .active_semantic_item(&job)
            .expect("active semantic item");
        assert_eq!(active.item.label, "Helix entry");

        sim.playback.current_move = 7;
        let active = sim
            .active_semantic_item(&job)
            .expect("active semantic item");
        assert_eq!(active.item.label, "Cleanup");
    }

    #[test]
    fn current_debug_annotation_uses_local_toolpath_move() {
        let job = job_with_traces();
        let mut sim = simulation_for_toolpath();
        sim.playback.current_move = 7;

        let annotation = sim
            .current_debug_annotation(&job)
            .expect("annotation for current move");
        assert_eq!(annotation.0, ToolpathId(1));
        assert_eq!(annotation.1.label, "Cleanup");
    }

    #[test]
    fn pinned_semantic_item_overrides_playback_resolution() {
        let job = job_with_traces();
        let mut sim = simulation_for_toolpath();
        sim.playback.current_move = 7;
        sim.pin_semantic_item(ToolpathId(1), 2);

        let active = sim
            .active_semantic_item(&job)
            .expect("pinned semantic item");
        assert_eq!(active.item.label, "Helix entry");

        sim.clear_pinned_semantic_item();
        let active = sim
            .active_semantic_item(&job)
            .expect("playback semantic item");
        assert_eq!(active.item.label, "Cleanup");
    }

    #[test]
    fn hotspot_target_resolves_move_and_semantic_item() {
        let job = job_with_traces();
        let mut sim = simulation_for_toolpath();

        let target = sim
            .trace_target_for_hotspot(&job, ToolpathId(1), 0)
            .expect("hotspot target");
        assert_eq!(target.toolpath_id, ToolpathId(1));
        assert_eq!(target.move_index, 0);
        assert!(target.semantic_item_id.is_some());
        assert!(target.debug_span_id.is_some());
    }

    #[test]
    fn issue_navigation_prioritizes_hotspots_then_annotations() {
        let job = job_with_traces();
        let mut sim = simulation_for_toolpath();

        let first = sim.focus_issue_delta(&job, 1).expect("first issue target");
        assert_eq!(first.move_index, 0);
        assert_eq!(
            sim.current_issue(&job).expect("focused issue").kind,
            SimulationIssueKind::Hotspot
        );

        let second = sim.focus_issue_delta(&job, 1).expect("second issue target");
        assert_eq!(second.move_index, 1);
        assert_eq!(
            sim.current_issue(&job).expect("focused issue").kind,
            SimulationIssueKind::Annotation
        );
    }

    #[test]
    fn semantic_pick_prefers_deeper_item_then_smaller_move_span() {
        let job = job_with_traces();
        let mut sim = simulation_for_toolpath();

        let target = sim
            .pick_semantic_item_with_ray(&job, &P3::new(2.0, 2.0, 10.0), &V3::new(0.0, 0.0, -1.0))
            .expect("semantic pick target");
        assert_eq!(target.toolpath_id, ToolpathId(1));
        assert_eq!(target.semantic_item_id, Some(2));
        assert_eq!(target.move_index, 0);
    }

    #[test]
    fn runtime_hotspots_rank_leaf_semantics_and_metrics_are_available() {
        let job = job_with_traces();
        let mut sim = simulation_for_toolpath();

        let hotspots = sim.runtime_hotspots(&job, ToolpathId(1), 5);
        assert!(!hotspots.is_empty(), "expected runtime hotspots");
        assert!(hotspots[0].total_seconds > 0.0);

        let metrics = sim
            .semantic_runtime_metrics(&job, ToolpathId(1), 2)
            .expect("runtime metrics for entry item");
        assert!(metrics.total_seconds > 0.0);
        assert!(metrics.cutting_seconds > 0.0);
    }

    #[test]
    fn cut_trace_surfaces_current_sample_and_cutting_issues() {
        let job = job_with_traces();
        let mut sim = simulation_for_toolpath();
        attach_cut_trace(&mut sim);
        sim.playback.current_move = 7;

        let sample = sim.current_cut_sample().expect("current cut sample");
        assert_eq!(sample.toolpath_id, ToolpathId(1));
        assert_eq!(sample.sample.move_index, 7);

        let issues = sim.issues(&job);
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
}
