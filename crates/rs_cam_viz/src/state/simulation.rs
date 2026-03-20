use super::toolpath::ToolpathId;
use rs_cam_core::simulation::HeightmapMesh;

/// Per-toolpath boundary in the simulation: toolpath ID and cumulative move count at its end.
#[derive(Debug, Clone)]
pub struct ToolpathBoundary {
    pub id: ToolpathId,
    pub name: String,
    pub tool_name: String,
    pub start_move: usize,
    pub end_move: usize,
}

/// Checkpoint: a snapshot of the heightmap mesh at a toolpath boundary.
pub struct SimCheckpoint {
    pub boundary_index: usize,
    pub mesh: HeightmapMesh,
}

/// Simulation playback state.
pub struct SimulationState {
    /// Whether simulation result mesh is displayed (replaces raw STL).
    pub active: bool,
    /// Animation playback state.
    pub playing: bool,
    /// Current move index for timeline scrubbing.
    pub current_move: usize,
    /// Total moves across all simulated toolpaths.
    pub total_moves: usize,
    /// Playback speed (moves per second).
    pub speed: f32,
    /// Per-toolpath boundaries for progress tracking and checkpoint lookup.
    pub boundaries: Vec<ToolpathBoundary>,
    /// Checkpoints at each toolpath boundary for rewind.
    pub checkpoints: Vec<SimCheckpoint>,
    /// Which toolpaths are selected for simulation (None = all enabled).
    pub selected_toolpaths: Option<Vec<ToolpathId>>,
    /// Tool position during playback (X, Y, Z).
    pub tool_position: Option<[f64; 3]>,
    /// Tool radius for the current operation during playback.
    pub tool_radius: f64,
    /// Tool type label for current operation during playback.
    pub tool_type_label: String,
}

impl SimulationState {
    pub fn new() -> Self {
        Self {
            active: false,
            playing: false,
            current_move: 0,
            total_moves: 0,
            speed: 500.0,
            boundaries: Vec::new(),
            checkpoints: Vec::new(),
            selected_toolpaths: None,
            tool_position: None,
            tool_radius: 0.0,
            tool_type_label: String::new(),
        }
    }

    pub fn progress(&self) -> f32 {
        if self.total_moves == 0 {
            0.0
        } else {
            self.current_move as f32 / self.total_moves as f32
        }
    }

    /// Advance playback by dt seconds. Returns true if still playing.
    pub fn advance(&mut self, dt: f32) -> bool {
        if !self.playing || self.current_move >= self.total_moves {
            return false;
        }
        let advance = (self.speed * dt) as usize;
        self.current_move = (self.current_move + advance.max(1)).min(self.total_moves);
        if self.current_move >= self.total_moves {
            self.playing = false;
        }
        true
    }

    /// Find which toolpath boundary contains the current move.
    pub fn current_boundary(&self) -> Option<&ToolpathBoundary> {
        self.boundaries.iter().find(|b| self.current_move >= b.start_move && self.current_move <= b.end_move)
    }

    /// Progress within the current toolpath (0.0..1.0).
    pub fn current_toolpath_progress(&self) -> (usize, usize) {
        if let Some(b) = self.current_boundary() {
            let within = self.current_move.saturating_sub(b.start_move);
            let total = b.end_move - b.start_move;
            (within, total)
        } else {
            (0, 0)
        }
    }

    /// Find the nearest checkpoint at or before the given move index.
    pub fn checkpoint_for_move(&self, move_idx: usize) -> Option<usize> {
        // Find the boundary index for this move
        let boundary_idx = self.boundaries.iter().position(|b| move_idx <= b.end_move)?;
        // The checkpoint to use is the one before this boundary (i.e., the state after previous toolpath)
        if boundary_idx == 0 {
            return None; // before the first toolpath, use initial stock
        }
        // Find checkpoint for boundary_idx - 1
        self.checkpoints.iter().position(|c| c.boundary_index == boundary_idx - 1)
    }
}
