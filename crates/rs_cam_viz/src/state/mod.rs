pub mod job;
pub mod selection;
pub mod viewport;

use job::JobState;
use selection::Selection;
use viewport::ViewportState;

/// Top-level application state. Single source of truth.
pub struct AppState {
    pub job: JobState,
    pub selection: Selection,
    pub viewport: ViewportState,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            job: JobState::new(),
            selection: Selection::None,
            viewport: ViewportState::new(),
        }
    }
}
