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
    /// Shared project session from core — keeps GUI and core in sync.
    /// Populated when a project is loaded; `None` for new untitled projects.
    pub session: Option<rs_cam_core::session::ProjectSession>,
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
            session: None,
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

    /// Sync the session from the current `JobState`, pushing stock, tools,
    /// post, machine, and all setup/toolpath configs into the session.
    ///
    /// This is a pragmatic bridge: instead of intercepting every individual
    /// GUI mutation, we bulk-sync at key checkpoints (before compute, before
    /// simulation).  Phase 4f (removing `JobState`) will eliminate the need
    /// for this.
    pub fn sync_session_from_job(&mut self) {
        let Some(ref mut session) = self.session else {
            return;
        };

        // Stock (same type, just clone)
        session.set_stock_config(self.state.job.stock.clone());

        // Post — map from viz's PostConfig (enum-based) to session's
        // ProjectPostConfig (string-based).
        let post_format_key = match self.state.job.post.format {
            crate::state::job::PostFormat::Grbl => "grbl",
            crate::state::job::PostFormat::LinuxCnc => "linuxcnc",
            crate::state::job::PostFormat::Mach3 => "mach3",
        };
        session.set_post_config(rs_cam_core::session::ProjectPostConfig {
            format: post_format_key.to_owned(),
            spindle_speed: self.state.job.post.spindle_speed,
            safe_z: self.state.job.post.safe_z,
            high_feedrate_mode: self.state.job.post.high_feedrate_mode,
            high_feedrate: self.state.job.post.high_feedrate,
        });

        // Machine (same type, just clone)
        session.set_machine(self.state.job.machine.clone());

        // Tools (same type, just clone the whole vec)
        session.replace_tools(self.state.job.tools.clone());

        // Setups + toolpaths — build session SetupData + ToolpathConfig vecs
        // from the current JobState.
        let mut session_setups = Vec::new();
        let mut session_tp_configs = Vec::new();

        for setup in &self.state.job.setups {
            let mut tp_indices = Vec::new();
            for tp in &setup.toolpaths {
                let tp_index = session_tp_configs.len();
                tp_indices.push(tp_index);
                session_tp_configs.push(rs_cam_core::session::ToolpathConfig {
                    id: tp.id.0,
                    name: tp.name.clone(),
                    enabled: tp.enabled,
                    operation: tp.operation.clone(),
                    dressups: tp.dressups.clone(),
                    heights: tp.heights.clone(),
                    tool_id: tp.tool_id.0,
                    model_id: tp.model_id.0,
                    pre_gcode: if tp.pre_gcode.is_empty() {
                        None
                    } else {
                        Some(tp.pre_gcode.clone())
                    },
                    post_gcode: if tp.post_gcode.is_empty() {
                        None
                    } else {
                        Some(tp.post_gcode.clone())
                    },
                    boundary: tp.boundary.clone(),
                    boundary_inherit: tp.boundary_inherit,
                    stock_source: tp.stock_source,
                    coolant: tp.coolant,
                    face_selection: tp.face_selection.clone(),
                    feeds_auto: tp.feeds_auto.clone(),
                    debug_options: tp.debug_options,
                });
            }

            session_setups.push(rs_cam_core::session::SetupData {
                id: setup.id.0,
                name: setup.name.clone(),
                face_up: setup.face_up,
                z_rotation: setup.z_rotation,
                fixtures: setup
                    .fixtures
                    .iter()
                    .map(|f| rs_cam_core::session::Fixture {
                        id: f.id,
                        name: f.name.clone(),
                        kind: match f.kind {
                            crate::state::job::FixtureKind::Clamp => {
                                rs_cam_core::session::FixtureKind::Clamp
                            }
                            crate::state::job::FixtureKind::Vise => {
                                rs_cam_core::session::FixtureKind::Vise
                            }
                            crate::state::job::FixtureKind::VacuumPod => {
                                rs_cam_core::session::FixtureKind::VacuumPod
                            }
                            crate::state::job::FixtureKind::Custom => {
                                rs_cam_core::session::FixtureKind::Custom
                            }
                        },
                        enabled: f.enabled,
                        origin_x: f.origin_x,
                        origin_y: f.origin_y,
                        origin_z: f.origin_z,
                        size_x: f.size_x,
                        size_y: f.size_y,
                        size_z: f.size_z,
                        clearance: f.clearance,
                    })
                    .collect(),
                keep_out_zones: setup
                    .keep_out_zones
                    .iter()
                    .map(|k| rs_cam_core::session::KeepOutZone {
                        id: k.id,
                        name: k.name.clone(),
                        enabled: k.enabled,
                        origin_x: k.origin_x,
                        origin_y: k.origin_y,
                        size_x: k.size_x,
                        size_y: k.size_y,
                    })
                    .collect(),
                toolpath_indices: tp_indices,
            });
        }

        session.replace_setups_and_toolpaths(session_setups, session_tp_configs);
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

        let count = stale_ids.len();
        for id in stale_ids {
            if let Some(toolpath) = self.state.job.find_toolpath_mut(id) {
                toolpath.stale_since = None;
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
