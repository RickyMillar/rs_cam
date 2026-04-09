#![deny(clippy::indexing_slicing)]

mod events;
mod io;
#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests;
#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::collapsible_if,
    clippy::clone_on_ref_ptr,
    clippy::field_reassign_with_default,
    dead_code
)]
mod workflow_tests;

use std::time::Instant;

use crate::compute::{ComputeBackend, ComputeLane, LaneSnapshot, ThreadedComputeBackend};
use crate::error::VizError;
use crate::state::AppState;
use crate::state::simulation::SimulationState;
use crate::ui::AppEvent;

/// Severity level for user-facing notifications.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Info,
    Warning,
    Error,
}

/// A user-facing notification displayed as a toast overlay.
pub struct Notification {
    pub message: String,
    pub severity: Severity,
    pub created_at: Instant,
}

impl Notification {
    /// Auto-dismiss duration based on severity.
    pub fn ttl(&self) -> std::time::Duration {
        match self.severity {
            Severity::Info => std::time::Duration::from_secs(4),
            Severity::Warning => std::time::Duration::from_secs(6),
            Severity::Error => std::time::Duration::from_secs(8),
        }
    }

    pub fn is_expired(&self) -> bool {
        self.created_at.elapsed() >= self.ttl()
    }
}

pub struct AppController<B: ComputeBackend = ThreadedComputeBackend> {
    pub state: AppState,
    events: Vec<AppEvent>,
    compute: B,
    pending_upload: bool,
    collision_positions: Vec<[f32; 3]>,
    load_warnings: Vec<String>,
    show_load_warnings: bool,
    status_message: Option<(String, Instant)>,
    notifications: Vec<Notification>,
    /// Pending MCP compute operations awaiting async results.
    /// `Some` when MCP mode is enabled, `None` otherwise.
    #[cfg(feature = "mcp")]
    pub pending_mcp: Option<crate::mcp_bridge::PendingMcpCompute>,
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
            notifications: Vec::new(),
            #[cfg(feature = "mcp")]
            pending_mcp: None,
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

    /// Push a notification from a VizError (logs via tracing AND shows toast).
    pub fn push_error(&mut self, error: &VizError) {
        tracing::error!("{error}");
        self.notifications.push(Notification {
            message: error.user_message(),
            severity: Severity::Error,
            created_at: Instant::now(),
        });
    }

    /// Push a notification with a string message and severity.
    pub fn push_notification(&mut self, message: String, severity: Severity) {
        self.notifications.push(Notification {
            message,
            severity,
            created_at: Instant::now(),
        });
    }

    /// Get active (non-expired) notifications.
    pub fn active_notifications(&self) -> impl Iterator<Item = &Notification> {
        self.notifications.iter().filter(|n| !n.is_expired())
    }

    /// Remove expired notifications.
    pub fn gc_notifications(&mut self) {
        self.notifications.retain(|n| !n.is_expired());
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
        use crate::state::toolpath::ToolpathId;

        let now = std::time::Instant::now();
        // Collect stale toolpath IDs from the GUI runtime overlay.
        let stale_ids: Vec<ToolpathId> = self
            .state
            .gui
            .toolpath_rt
            .iter()
            .filter(|(_, rt)| rt.auto_regen && !rt.locked)
            .filter_map(|(id, rt)| {
                rt.stale_since
                    .filter(|stale_since| now.duration_since(*stale_since).as_millis() > 500)
                    .map(|_| ToolpathId(*id))
            })
            .collect();

        let count = stale_ids.len();
        for id in stale_ids {
            if let Some(rt) = self.state.gui.toolpath_rt.get_mut(&id.0) {
                rt.stale_since = None;
            }
            self.submit_toolpath_compute(id);
        }
        if count > 0 {
            self.push_notification(
                format!("Auto-regenerating {} toolpath(s)", count),
                Severity::Info,
            );
        }
    }
}
