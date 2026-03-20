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
}

impl SimulationState {
    pub fn new() -> Self {
        Self {
            active: false,
            playing: false,
            current_move: 0,
            total_moves: 0,
            speed: 500.0,
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
}
