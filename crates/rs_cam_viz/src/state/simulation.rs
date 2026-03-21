use super::job::SetupId;
use super::toolpath::ToolpathId;
use rs_cam_core::collision::{CollisionReport, RapidCollision};
use rs_cam_core::simulation::HeightmapMesh;

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
}

/// Per-setup boundary in the simulation: marks where a setup begins.
#[derive(Debug, Clone)]
pub struct SetupBoundary {
    pub setup_id: SetupId,
    pub setup_name: String,
    pub start_move: usize,
}

/// Checkpoint: a snapshot of the heightmap at a toolpath boundary.
pub struct SimCheckpoint {
    pub boundary_index: usize,
    pub mesh: HeightmapMesh,
    /// The heightmap grid at this checkpoint (for resuming incremental sim).
    pub heightmap: Option<rs_cam_core::simulation::Heightmap>,
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
    /// Live heightmap for incremental playback simulation.
    pub live_heightmap: Option<rs_cam_core::simulation::Heightmap>,
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
                live_heightmap: None,
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

impl Default for SimulationPlayback {
    fn default() -> Self {
        Self {
            playing: false,
            current_move: 0,
            speed: 500.0,
            tool_position: None,
            tool_radius: 0.0,
            tool_type_label: String::new(),
            live_heightmap: None,
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
