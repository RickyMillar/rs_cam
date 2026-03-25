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
pub enum Workspace {
    /// Setup definition: stock orientation, datum, workholding, fixtures.
    Setup,
    /// Toolpath authoring — parameters, feeds, strategies.
    Toolpaths,
    /// Verification — material removal, collisions, cycle time, safety.
    Simulation,
}

/// Top-level application state. Single source of truth.
pub struct AppState {
    pub workspace: Workspace,
    pub job: JobState,
    pub selection: Selection,
    pub viewport: ViewportState,
    pub simulation: SimulationState,
    pub history: UndoHistory,
    /// Show pre-flight checklist modal before export.
    pub show_preflight: bool,
    /// Show keyboard shortcuts reference window.
    pub show_shortcuts: bool,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            workspace: Workspace::Toolpaths,
            job: JobState::new(),
            selection: Selection::None,
            viewport: ViewportState::new(),
            simulation: SimulationState::new(),
            history: UndoHistory::new(),
            show_preflight: false,
            show_shortcuts: false,
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
