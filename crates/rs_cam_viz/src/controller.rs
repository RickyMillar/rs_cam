mod events;
mod io;
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests;

use crate::compute::{ComputeBackend, ComputeLane, LaneSnapshot, ThreadedComputeBackend};
use crate::state::AppState;
use crate::state::simulation::SimulationState;
use crate::ui::AppEvent;

pub struct AppController<B: ComputeBackend = ThreadedComputeBackend> {
    pub state: AppState,
    events: Vec<AppEvent>,
    compute: B,
    pending_upload: bool,
    collision_positions: Vec<[f32; 3]>,
    load_warnings: Vec<String>,
    show_load_warnings: bool,
    status_message: Option<(String, std::time::Instant)>,
}

impl AppController<ThreadedComputeBackend> {
    pub fn new() -> Self {
        Self::with_backend(ThreadedComputeBackend::new())
    }
}

impl Default for AppController<ThreadedComputeBackend> {
    fn default() -> Self {
        Self::new()
    }
}

impl<B: ComputeBackend> AppController<B> {
    pub fn with_backend(compute: B) -> Self {
        Self {
            state: AppState::new(),
            events: Vec::new(),
            compute,
            pending_upload: false,
            collision_positions: Vec::new(),
            load_warnings: Vec::new(),
            show_load_warnings: false,
            status_message: None,
        }
    }

    pub fn state(&self) -> &AppState {
        &self.state
    }

    pub fn state_mut(&mut self) -> &mut AppState {
        &mut self.state
    }

    pub fn state_ref_and_events_mut(&mut self) -> (&AppState, &mut Vec<AppEvent>) {
        (&self.state, &mut self.events)
    }

    pub fn state_and_events_mut(&mut self) -> (&mut AppState, &mut Vec<AppEvent>) {
        (&mut self.state, &mut self.events)
    }

    pub fn simulation_viewport_and_events_mut(
        &mut self,
    ) -> (
        &mut SimulationState,
        &mut crate::state::viewport::ViewportState,
        &mut Vec<AppEvent>,
    ) {
        (
            &mut self.state.simulation,
            &mut self.state.viewport,
            &mut self.events,
        )
    }

    pub fn events_mut(&mut self) -> &mut Vec<AppEvent> {
        &mut self.events
    }

    pub fn drain_events(&mut self) -> Vec<AppEvent> {
        self.events.drain(..).collect()
    }

    pub fn take_pending_upload(&mut self) -> bool {
        std::mem::take(&mut self.pending_upload)
    }

    pub fn set_pending_upload(&mut self) {
        self.pending_upload = true;
    }

    pub fn collision_positions(&self) -> &[[f32; 3]] {
        &self.collision_positions
    }

    pub fn lane_snapshot(&self, lane: ComputeLane) -> LaneSnapshot {
        self.compute.lane_snapshot(lane)
    }

    pub fn lane_snapshots(&self) -> [LaneSnapshot; 2] {
        self.compute.lane_snapshots()
    }

    pub fn load_warnings(&self) -> &[String] {
        &self.load_warnings
    }

    pub fn show_load_warnings(&self) -> bool {
        self.show_load_warnings
    }

    pub fn set_show_load_warnings(&mut self, show: bool) {
        self.show_load_warnings = show;
    }

    /// Set a temporary status message (auto-expires after 5 seconds).
    pub fn set_status(&mut self, message: String) {
        self.status_message = Some((message, std::time::Instant::now()));
    }

    /// Get the current status message, or None if expired.
    pub fn status_message(&self) -> Option<&str> {
        if let Some((msg, when)) = &self.status_message
            && when.elapsed().as_secs() < 5
        {
            return Some(msg.as_str());
        }
        None
    }

    pub fn process_auto_regen(&mut self) {
        let now = std::time::Instant::now();
        let stale_ids: Vec<_> = self
            .state
            .job
            .all_toolpaths()
            .filter(|toolpath| toolpath.auto_regen && !toolpath.locked)
            .filter_map(|toolpath| {
                toolpath
                    .stale_since
                    .filter(|stale_since| now.duration_since(*stale_since).as_millis() > 500)
                    .map(|_| toolpath.id)
            })
            .collect();

        for id in stale_ids {
            if let Some(toolpath) = self.state.job.find_toolpath_mut(id) {
                toolpath.stale_since = None;
            }
            self.submit_toolpath_compute(id);
        }
    }
}
