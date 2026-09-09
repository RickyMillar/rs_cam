#![deny(clippy::indexing_slicing)]

mod events;
pub mod generate_all;
mod io;
#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::field_reassign_with_default
)]
mod results_parity_tests;
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

/// How long a reach-map request may sit in `Computing` over an idle, empty
/// Reach lane before the scheduler treats it as lost and asks again (P5).
///
/// Repo-authored, and chosen only to be far longer than the pump interval and
/// far shorter than an operator's patience. A cold reach walk is one to two
/// seconds on a board-sized terrain, but a walk that is still running holds
/// the lane in `Running`, so this timer never races it.
const REACH_STALL_GRACE: std::time::Duration = std::time::Duration::from_secs(3);

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
    /// G-REGEN-RACE: toolpaths whose in-flight generation this controller
    /// has just replaced, and whose `ComputeError::Cancelled` is therefore
    /// still on the wire as bookkeeping rather than as a result.
    ///
    /// A set, not a counter, on purpose: two supersedes of the same
    /// toolpath before the first `Cancelled` drains produce **one**
    /// `Cancelled` (the second submit only re-`retain`s the queued
    /// replacement), so a counter would leak an entry and swallow the next
    /// genuine cancellation. `drain_compute_results` removes the id on
    /// *any* result for that toolpath, so a submit that superseded a job
    /// which had already finished (the one narrow TOCTOU the lane can
    /// produce) self-heals on the very next drain instead of persisting.
    superseded_toolpaths: std::collections::HashSet<crate::state::toolpath::ToolpathId>,
    /// Phase O — the in-flight `generate_all`, from whichever surface started
    /// it. Unconditional on purpose: the GUI's Generate All runs the same
    /// fixpoint ladder the MCP tool does, and `pending_mcp` is `None` outside
    /// `--mcp`. Only [`generate_all::GenerateAllSink`] differs.
    generate_all: Option<generate_all::PendingGenerateAll>,
    /// P5 — the reach-map checkbox as this controller last saw it.
    ///
    /// V13: `upload_gpu_data` runs only when `pending_upload` is set, so a
    /// dial that changes what the viewport draws must mark one. Nothing else
    /// watches this checkbox, and the operator can flip it on a frame where
    /// no other input lands.
    reach_overlay_shown: bool,
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
            superseded_toolpaths: std::collections::HashSet::new(),
            generate_all: None,
            reach_overlay_shown: true,
            #[cfg(feature = "mcp")]
            pending_mcp: None,
        }
    }

    /// Completions this controller still owes a **future frame**.
    ///
    /// G-LV.1: the compute drain, `generate_all`'s fixpoint round handoff and
    /// the MCP screenshot pump all live in `RsCamApp::update`, so a non-zero
    /// count is a standing reason to keep repainting. The ladder term is
    /// counted here rather than on `PendingMcpCompute` because a GUI-started
    /// ladder needs those frames just as much and has no MCP slot at all.
    #[must_use]
    pub fn awaiting_deferred_completions(&self) -> u64 {
        let ladder = u64::from(self.generate_all.is_some());
        #[cfg(feature = "mcp")]
        let ladder = ladder
            + self
                .pending_mcp
                .as_ref()
                .map_or(0, crate::mcp_bridge::PendingMcpCompute::awaiting_gui);
        ladder
    }

    /// Whether a `generate_all` ladder is among them.
    #[must_use]
    pub fn awaiting_generate_all(&self) -> bool {
        self.generate_all.is_some()
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

    pub fn lane_snapshots(&self) -> [LaneSnapshot; 4] {
        self.compute.lane_snapshots()
    }

    /// A/M12: a `Send + Sync` handle onto the toolpath lane, for the embedded
    /// MCP server thread. Lets `cancel_generation` / `generation_status` be
    /// answered without the GUI frame loop.
    pub fn generation_control(&self) -> crate::compute::GenerationControl {
        self.compute.generation_control()
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

    /// Push the toast for a synchronous MCP request from the handler's
    /// OUTCOME (G-MCPTOAST, UX-R03-003): `success_message` at Info when the
    /// handler succeeded, the handler's refusal text at Warning when it did
    /// not. Call this AFTER the handler, never before it — the past tense
    /// in the success text is only true once the handler has returned.
    #[cfg(feature = "mcp")]
    pub fn push_mcp_outcome(
        &mut self,
        success_message: String,
        outcome: &crate::mcp_bridge::McpOutcome,
    ) {
        let (message, severity) = outcome.notification(success_message);
        self.push_notification(message, severity);
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
                    .map(|_| *id)
            })
            .collect();

        let count = stale_ids.len();
        for id in stale_ids {
            if let Some(rt) = self.state.gui.toolpath_rt.get_mut(&id) {
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

    /// Keep the reach-map overlay pointed at the current selection (P5).
    ///
    /// Runs on every pump beside [`Self::process_auto_regen`], and submits
    /// only when the scheduling key moves. The key is the selected toolpath
    /// plus the session edit counter, so a tool or parameter edit resolves a
    /// fresh request while a plain repaint resolves nothing.
    ///
    /// `ProjectSession::reach_map_spec` does no drop-cutter work — it reads
    /// two memoised geometry caches — so resolving one here is safe on the UI
    /// thread. It answers `None` when the operation is not one a reach map
    /// speaks about, or has no mesh, or has no tool; that clears the overlay
    /// rather than leaving the previous toolpath's answer on screen.
    ///
    /// No debounce: the memo makes a repeat key free, and the key cannot move
    /// on its own.
    pub fn process_reach_overlay(&mut self) {
        use crate::compute::LaneState;
        use crate::state::runtime::ReachStatus;
        use crate::state::selection::Selection;

        // The checkbox changes what the viewport draws, so a flip owes the
        // next pass an upload even though no buffer moved.
        if self.reach_overlay_shown != self.state.viewport.show_reach_map {
            self.reach_overlay_shown = self.state.viewport.show_reach_map;
            self.pending_upload = true;
        }

        let edit_counter = self.state.gui.edit_counter;
        let Selection::Toolpath(id) = self.state.selection else {
            if self.state.gui.reach_overlay.toolpath.is_some() {
                self.state.gui.reach_overlay.clear();
                self.pending_upload = true;
            }
            return;
        };

        // Recover a walk that was cancelled with nothing queued behind it —
        // `cancel_all`, or a shutdown that did not happen. The drain drops
        // every `Cancelled` (see `handle_reach_map_result` for why it must),
        // so `Computing` over an idle, empty lane is the one state that would
        // otherwise never resolve. Clearing the key makes the check below
        // submit again.
        //
        // The grace period is load-bearing, not politeness: a backend with no
        // Reach lane — every scripted test double — reports an idle lane the
        // instant after a submit, and an unbounded recovery would then
        // resubmit on every pump for ever.
        //
        // The reverse race is harmless: a result already sent but not yet
        // drained arrives after the resubmit, is accepted because the toolpath
        // still matches, and the resubmitted walk then hits the reach memo and
        // returns the same `Arc`, so no buffer is rebuilt.
        let stalled = matches!(self.state.gui.reach_overlay.status, ReachStatus::Computing)
            && self
                .state
                .gui
                .reach_overlay
                .requested_at
                .is_some_and(|at| at.elapsed() >= REACH_STALL_GRACE);
        if stalled {
            let lane = self.compute.lane_snapshot(ComputeLane::Reach);
            if matches!(lane.state, LaneState::Idle) && lane.queue_depth == 0 {
                self.state.gui.reach_overlay.clear();
            }
        }

        // The key already holds the answer, or the request for it. A resubmit
        // would cancel the walk that is about to answer, because the lane's
        // rule is latest-wins.
        let overlay = &self.state.gui.reach_overlay;
        if overlay.toolpath == Some(id) && overlay.edit_counter == edit_counter {
            return;
        }

        let spec = self
            .state
            .session
            .find_toolpath_config_by_id(id)
            .and_then(|(index, _)| self.state.session.reach_map_spec(index, None));

        let overlay = &mut self.state.gui.reach_overlay;
        overlay.toolpath = Some(id);
        overlay.edit_counter = edit_counter;
        overlay.colors = None;
        match spec {
            Some(spec) => {
                overlay.status = ReachStatus::Computing;
                overlay.requested_at = Some(Instant::now());
                self.compute.submit_reach_map(crate::compute::ReachRequest {
                    toolpath_id: id,
                    spec,
                });
            }
            None => {
                overlay.status = ReachStatus::Idle;
                overlay.requested_at = None;
            }
        }
        self.pending_upload = true;
    }
}
