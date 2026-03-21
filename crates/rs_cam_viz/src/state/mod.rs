pub mod history;
pub mod job;
pub mod selection;
pub mod simulation;
pub mod toolpath;
pub mod viewport;

use history::UndoHistory;
use job::JobState;
use selection::Selection;
use simulation::SimulationState;
use viewport::ViewportState;

/// Which top-level workspace the user is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    /// Toolpath authoring — parameters, feeds, strategies.
    Editor,
    /// Verification — material removal, collisions, cycle time, safety.
    Simulation,
}

/// Top-level application state. Single source of truth.
pub struct AppState {
    pub mode: AppMode,
    pub job: JobState,
    pub selection: Selection,
    pub viewport: ViewportState,
    pub simulation: SimulationState,
    pub history: UndoHistory,
    /// Show pre-flight checklist modal before export.
    pub show_preflight: bool,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            mode: AppMode::Editor,
            job: JobState::new(),
            selection: Selection::None,
            viewport: ViewportState::new(),
            simulation: SimulationState::new(),
            history: UndoHistory::new(),
            show_preflight: false,
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
