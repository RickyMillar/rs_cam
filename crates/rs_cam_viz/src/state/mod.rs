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

/// Top-level application state. Single source of truth.
pub struct AppState {
    pub job: JobState,
    pub selection: Selection,
    pub viewport: ViewportState,
    pub simulation: SimulationState,
    pub history: UndoHistory,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            job: JobState::new(),
            selection: Selection::None,
            viewport: ViewportState::new(),
            simulation: SimulationState::new(),
            history: UndoHistory::new(),
        }
    }
}
