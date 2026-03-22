use std::collections::{HashMap, HashSet};

use super::job::{JobState, SetupId};
use super::toolpath::ToolpathId;
use rs_cam_core::collision::{CollisionReport, RapidCollision};
use rs_cam_core::debug_trace::{ToolpathDebugAnnotation, ToolpathDebugBounds2};
use rs_cam_core::dexel_stock::{StockCutDirection, TriDexelStock};
use rs_cam_core::semantic_trace::{ToolpathSemanticItem, ToolpathSemanticTrace};
use rs_cam_core::simulation::HeightmapMesh;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimulationDebugTab {
    Semantic,
    Performance,
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

#[derive(Default)]
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
    pub drawer_open: bool,
    pub active_tab: SimulationDebugTab,
    pub expanded_toolpaths: HashSet<ToolpathId>,
    pub focused_hotspot_index: Option<usize>,
    pub highlight_active_item: bool,
    pub pending_inspect_toolpath: Option<ToolpathId>,
    pub(crate) semantic_indexes: HashMap<ToolpathId, SimulationSemanticIndex>,
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
    pub mesh: HeightmapMesh,
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
    pub mesh: HeightmapMesh,
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
    pub display_mesh: Option<HeightmapMesh>,
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
            stock_viz_mode: StockVizMode::Solid,
            stock_opacity: 1.0,
            saved_viewport: SavedViewportState {
                show_cutting: true,
                show_rapids: true,
                show_stock: true,
            },
            debug: SimulationDebugState {
                enabled: false,
                drawer_open: true,
                active_tab: SimulationDebugTab::Semantic,
                expanded_toolpaths: HashSet::new(),
                focused_hotspot_index: None,
                highlight_active_item: true,
                pending_inspect_toolpath: None,
                semantic_indexes: HashMap::new(),
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
    }

    pub fn active_semantic_item(&mut self, job: &JobState) -> Option<ActiveSemanticItem> {
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

    pub fn current_debug_annotation(
        &self,
        job: &JobState,
    ) -> Option<(ToolpathId, ToolpathDebugAnnotation)> {
        let (_, toolpath_id, local_move) = self.current_local_toolpath_move()?;
        let toolpath = job.find_toolpath(toolpath_id)?;
        let trace = toolpath.debug_trace.as_ref()?;
        trace
            .annotations
            .iter()
            .rev()
            .find(|annotation| annotation.move_index <= local_move)
            .cloned()
            .map(|annotation| (toolpath_id, annotation))
    }

    pub fn current_item_bbox(
        &mut self,
        job: &JobState,
    ) -> Option<(ToolpathId, ToolpathDebugBounds2, f64, f64)> {
        let active = self.active_semantic_item(job)?;
        Some((
            active.toolpath_id,
            active.item.xy_bbox?,
            active.item.z_min?,
            active.item.z_max?,
        ))
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

impl Default for SimulationDebugTab {
    fn default() -> Self {
        Self::Semantic
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
}

impl SimulationSemanticIndex {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::job::{JobState, ModelId, ToolId};
    use crate::state::toolpath::{OperationType, ToolpathEntry};
    use rs_cam_core::debug_trace::ToolpathDebugRecorder;
    use rs_cam_core::dexel_stock::StockCutDirection;
    use rs_cam_core::semantic_trace::{ToolpathSemanticKind, ToolpathSemanticRecorder};
    use rs_cam_core::toolpath::Toolpath;
    use std::sync::Arc;

    fn job_with_traces() -> JobState {
        let mut job = JobState::new();
        let toolpath_id = ToolpathId(1);
        let mut entry = ToolpathEntry::for_operation(
            toolpath_id,
            "Adaptive".to_string(),
            ToolId(1),
            ModelId(1),
            OperationType::Adaptive,
        );

        let semantic = ToolpathSemanticRecorder::new("Adaptive", "Adaptive");
        let root = semantic.root_context();
        let pass = root.start_item(ToolpathSemanticKind::Pass, "Pass 1");
        pass.set_move_range(0, 8);
        let entry_item = pass
            .context()
            .start_item(ToolpathSemanticKind::Entry, "Helix entry");
        entry_item.set_move_range(0, 2);
        entry_item.finish();
        let cleanup = pass
            .context()
            .start_item(ToolpathSemanticKind::Cleanup, "Cleanup");
        cleanup.set_move_range(6, 8);
        cleanup.finish();
        pass.finish();
        entry.semantic_trace = Some(Arc::new(semantic.finish()));

        let debug = ToolpathDebugRecorder::new("Adaptive", "Adaptive");
        let debug_ctx = debug.root_context();
        debug_ctx.add_annotation(1, "Entry");
        debug_ctx.add_annotation(7, "Cleanup");
        entry.debug_trace = Some(Arc::new(debug.finish()));
        entry.result = Some(crate::state::toolpath::ToolpathResult {
            toolpath: Arc::new(Toolpath::new()),
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
            mesh: HeightmapMesh {
                vertices: Vec::new(),
                indices: Vec::new(),
                colors: Vec::new(),
            },
            total_moves: 9,
            boundaries: vec![ToolpathBoundary {
                id: ToolpathId(1),
                name: "Adaptive".to_string(),
                tool_name: "6mm End Mill".to_string(),
                start_move: 0,
                end_move: 8,
            }],
            setup_boundaries: vec![SetupBoundary {
                setup_id: SetupId(1),
                setup_name: "Setup 1".to_string(),
                start_move: 0,
            }],
            checkpoints: Vec::new(),
            selected_toolpaths: None,
            direction: StockCutDirection::FromTop,
        });
        sim
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
}
