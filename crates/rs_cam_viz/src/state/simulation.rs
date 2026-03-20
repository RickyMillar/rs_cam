/// Simulation playback state.
pub struct SimulationState {
    /// Whether simulation result mesh is displayed (replaces raw STL).
    pub active: bool,
    /// Animation playback state.
    pub playing: bool,
    /// Current move index for animation.
    pub current_move: usize,
    /// Total moves across all simulated toolpaths.
    pub total_moves: usize,
    /// Playback speed multiplier.
    pub speed: f32,
}

impl SimulationState {
    pub fn new() -> Self {
        Self {
            active: false,
            playing: false,
            current_move: 0,
            total_moves: 0,
            speed: 1.0,
        }
    }

    pub fn progress(&self) -> f32 {
        if self.total_moves == 0 {
            0.0
        } else {
            self.current_move as f32 / self.total_moves as f32
        }
    }
}
