//! Playback state: what the simulation is showing right now.
//!
//! The methods build a `SimulationState`, report whether it holds a result,
//! read the boundaries and the checkpoints, advance the playhead and map a
//! global move index to a local one. They are an `impl SimulationState`
//! fragment, so every name keeps its path.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use rs_cam_core::dexel_stock::TriDexelStock;
use rs_cam_core::stock::simulation_cut::SimulationMetricOptions;

use super::{
    ChiploadEnvelopeCache, HolderCheckScope, IssueListCache, SetupBoundary, SimCheckpoint,
    SimulationChecks, SimulationDebugState, SimulationPlayback, SimulationState,
    SimulationTriageCache, SpanAggregateCache, SpanScope, StockVizMode, ToolLoadReportCache,
    ToolpathBoundary,
};
use crate::state::toolpath::ToolpathId;

impl SimulationState {
    pub fn new() -> Self {
        Self {
            results: None,
            playback: SimulationPlayback {
                playing: false,
                current_move: 0,
                partial_move: 0.0,
                speed: 500.0,
                tool_position: None,
                tool_radius: 0.0,
                tool_type_label: String::new(),
                tool_stickout: 0.0,
                tool_cutting_length: 0.0,
                tool_deflection_mm: None,
                live_stock: None,
                live_stock_group: None,
                live_sim_move: 0,
                display_mesh: None,
                display_mesh_move: None,
                last_mesh_upload_at: None,
                tool_gpu_move: None,
                display_mesh_preview: false,
                scrub_drag_active: false,
                display_deviations: None,
            },
            checks: SimulationChecks {
                rapid_collisions: Vec::new(),
                rapid_collision_move_indices: Vec::new(),
                collision_report: None,
                holder_collision_count: 0,
                min_safe_stickout: None,
                checked_at_edit_counter: None,
                checked_scope: HolderCheckScope::default(),
            },
            last_run: None,
            submitted_edit_counter: None,
            submitted_collision_edit_counter: None,
            submitted_collision_scope: None,
            resolution: 0.25,
            auto_resolution: true,
            metric_options: SimulationMetricOptions::default(),
            metric_options_revision: 0,
            submitted_metric_options_revision: None,
            stock_viz_mode: StockVizMode::Solid,
            stock_opacity: 1.0,
            debug: SimulationDebugState {
                enabled: false,
                expanded_toolpaths: HashSet::new(),
                focused_hotspot: None,
                pinned_semantic_item: None,
                focused_issue_index: None,
                highlight_active_item: true,
                pending_inspect_toolpath: None,
                span_scope: SpanScope::default(),
                span_aggregates: SpanAggregateCache::default(),
                load_report_cache: ToolLoadReportCache::default(),
                chipload_envelope_cache: ChiploadEnvelopeCache::default(),
                triage_cache: SimulationTriageCache::default(),
                issue_cache: IssueListCache::default(),
                semantic_indexes: HashMap::new(),
            },
            hovered_x: None,
        }
    }

    // --- Convenience accessors ---

    /// Whether simulation results exist.
    pub fn has_results(&self) -> bool {
        self.results.is_some()
    }

    /// Total moves from results (0 if no results).
    pub fn total_moves(&self) -> usize {
        self.results.as_ref().map_or(0, |r| r.total_moves)
    }

    /// Toolpath boundaries (empty slice if no results).
    pub fn boundaries(&self) -> &[ToolpathBoundary] {
        self.results
            .as_ref()
            .map_or(&[], |r| r.boundaries.as_slice())
    }

    /// Setup boundaries (empty slice if no results).
    pub fn setup_boundaries(&self) -> &[SetupBoundary] {
        self.results
            .as_ref()
            .map_or(&[], |r| r.setup_boundaries.as_slice())
    }

    /// Checkpoints (empty slice if no results).
    pub fn checkpoints(&self) -> &[SimCheckpoint] {
        self.results
            .as_ref()
            .map_or(&[], |r| r.checkpoints.as_slice())
    }

    /// Simulated stock snapshot from *before* `toolpath_id` carved, if the
    /// last simulation run produced one — either the toolpath's own
    /// pre-carve snapshot (it was generated and included in the run) or a
    /// phantom snapshot (F.4: it's the first pending `FromRemainingStock`
    /// toolpath in its group). `None` when no simulation has run yet, or
    /// when this toolpath sits behind a still-pending predecessor in its
    /// group (the ladder rule — see
    /// `rs_cam_core::compute::simulate::SimGroupEntry::phantom_prior_stock`).
    pub fn prior_stock_for(&self, toolpath_id: ToolpathId) -> Option<&Arc<TriDexelStock>> {
        self.results
            .as_ref()
            .and_then(|r| r.prior_stocks.get(&toolpath_id))
    }

    /// Selected toolpaths (None = all enabled).
    pub fn selected_toolpaths(&self) -> Option<&Vec<ToolpathId>> {
        self.results
            .as_ref()
            .and_then(|r| r.selected_toolpaths.as_ref())
    }

    /// Set metric capture without dirtying the project. The revision bump
    /// alone carries the staleness: [`Self::metric_options_are_stale`]
    /// derives it by comparing the accepted run's stamped revision against
    /// the revision set now, so no separate bool can drift out of step
    /// with the evidence it describes.
    pub fn set_metric_capture_enabled(&mut self, enabled: bool) {
        if self.metric_options.enabled != enabled {
            self.metric_options.enabled = enabled;
            self.metric_options_revision += 1;
        }
        if enabled {
            self.metric_options.capture_arc_engagement = true;
        }
    }

    /// True when the accepted result did not answer the capture revision
    /// that is set now. A project with no accepted run returns `false`:
    /// there is nothing to be stale, which is not the same as "clean".
    pub fn metric_options_are_stale(&self) -> bool {
        self.last_run.as_ref().is_some_and(|meta| {
            meta.accepted_metric_options_revision != Some(self.metric_options_revision)
        })
    }

    /// Returns true if simulation results are stale (params or recording
    /// options changed since the last sim).
    pub fn is_stale(&self, current_edit_counter: u64) -> bool {
        self.last_run.as_ref().is_some_and(|meta| {
            current_edit_counter > meta.last_sim_edit_counter || self.metric_options_are_stale()
        })
    }

    /// True when a collision check HAS run and the project has been edited
    /// since it was submitted (F2.12, G-HOLDERSTALE).
    ///
    /// A project with no check returns `false` here. That is not "clear": it
    /// is "there is nothing to be stale", and
    /// [`crate::ui::readiness::holder_clearance_check`] separates the two by
    /// reading [`SimulationChecks::checked_at_edit_counter`] first.
    ///
    /// The counter is project-wide, so an edit that could not have changed
    /// the holder verdict still stales it. That coarseness is F2.10 §3's,
    /// accepted for the same reason: the cost is one re-check, and the cost
    /// of the other error is telling an operator a cut is clear on evidence
    /// that belongs to a different machine setup.
    pub fn collision_check_is_stale(&self, current_edit_counter: u64) -> bool {
        self.checks
            .checked_at_edit_counter
            .is_some_and(|checked_at| current_edit_counter > checked_at)
    }

    pub fn progress(&self) -> f32 {
        let total = self.total_moves();
        if total == 0 {
            0.0
        } else {
            self.playback.current_move as f32 / total as f32
        }
    }

    /// Advance playback by dt seconds. Returns true if still playing.
    ///
    /// Carries the fractional part of `speed * dt` across frames in
    /// `partial_move` so the requested moves-per-second is honoured even
    /// when it's less than the frame rate. The old `.max(1)` floor here
    /// meant any speed below ~60 mv/s (at 60 fps) silently ran at the
    /// frame rate — making short toolpaths (e.g. an 18-move drill cycle)
    /// finish in a single frame regardless of the speed slider.
    pub fn advance(&mut self, dt: f32) -> bool {
        let total = self.total_moves();
        if !self.playback.playing || self.playback.current_move >= total {
            return false;
        }
        self.playback.partial_move += self.playback.speed * dt;
        // SAFETY: f32 < 4e9 fits usize on every target we care about;
        // partial_move is clamped to [0, speed * dt] so this can't grow
        // without bound.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let step = self.playback.partial_move.floor() as usize;
        if step > 0 {
            self.playback.partial_move -= step as f32;
            self.playback.current_move = (self.playback.current_move + step).min(total);
        }
        if self.playback.current_move >= total {
            self.playback.playing = false;
            self.playback.partial_move = 0.0;
        }
        true
    }

    /// Find which toolpath boundary contains the current move. Boundaries
    /// touch at a point (one's `end_move` equals the next's `start_move`),
    /// so we scan in reverse and pick the **later** boundary on a tie. That
    /// way, jumping to a boundary's `start_move` lands focus on that
    /// boundary rather than the one that just ended.
    pub fn current_boundary(&self) -> Option<&ToolpathBoundary> {
        let current = self.playback.current_move;
        self.boundaries()
            .iter()
            .rev()
            .find(|b| current >= b.start_move && current <= b.end_move)
    }

    /// The toolpath in focus for graph filtering and viewport-marker
    /// filtering. Tracks the playback position — focus follows whatever TP
    /// is currently playing. Selection in the left panel jumps playback to
    /// the chosen TP, which then becomes the focus via this getter; there's
    /// no separate sticky pin.
    pub fn focused_toolpath(&self) -> Option<ToolpathId> {
        self.current_boundary().map(|b| b.id)
    }

    #[allow(clippy::indexing_slicing)] // boundary_index from position() is always in bounds
    pub fn move_to_local_toolpath_move(
        &self,
        move_idx: usize,
    ) -> Option<(usize, ToolpathId, usize)> {
        // Same tie-breaking as `current_boundary`: at a boundary point
        // (where one ends and the next begins) prefer the *later* boundary.
        let count = self.boundaries().len();
        let boundary_index = self
            .boundaries()
            .iter()
            .rev()
            .position(|boundary| move_idx >= boundary.start_move && move_idx <= boundary.end_move)
            .map(|rev_idx| count - 1 - rev_idx)?;
        let boundary = &self.boundaries()[boundary_index];
        let local_move = move_idx.saturating_sub(boundary.start_move);
        Some((boundary_index, boundary.id, local_move))
    }

    pub fn current_local_toolpath_move(&self) -> Option<(usize, ToolpathId, usize)> {
        self.move_to_local_toolpath_move(self.playback.current_move)
    }

    /// Per-toolpath holder/shank collision counts from the last
    /// dedicated collision check, attributed via simulation boundaries.
    /// Empty when no check has run. This is the holder evidence the
    /// core's `diagnostics_with_evidence` consumes — derived from the
    /// stored report (O(collisions)), never recomputed, so it is safe
    /// to call at frame rate (the 2026-06-11 setup-tab lag was the
    /// diagnostics path re-running the full collision sweep per frame).
    pub(crate) fn holder_collision_counts_by_tp(
        &self,
    ) -> Vec<(
        ToolpathId,
        rs_cam_core::stock::collision::HolderCollisionCheck,
    )> {
        use rs_cam_core::stock::collision::HolderCollisionCheck;
        let mut counts: Vec<(ToolpathId, usize)> = Vec::new();
        if let Some(report) = self.checks.collision_report.as_ref() {
            for collision in &report.collisions {
                if let Some((_, id, _)) = self.move_to_local_toolpath_move(collision.move_index) {
                    match counts.iter_mut().find(|(cid, _)| *cid == id) {
                        Some((_, count)) => *count += 1,
                        None => counts.push((id, 1)),
                    }
                }
            }
        }
        // Only toolpaths the report found HITS on appear here. A toolpath
        // the GUI never checked is absent, and core reads absence as "not
        // measured" — it must not be listed as a measured zero (CMP-14).
        counts
            .into_iter()
            .map(|(id, count)| (id, HolderCollisionCheck::Measured(count)))
            .collect()
    }

    pub(crate) fn boundary_for_toolpath_id(
        &self,
        toolpath_id: ToolpathId,
    ) -> Option<&ToolpathBoundary> {
        self.boundaries()
            .iter()
            .find(|boundary| boundary.id == toolpath_id)
    }

    pub fn global_move_for_local(
        &self,
        toolpath_id: ToolpathId,
        local_move: usize,
    ) -> Option<usize> {
        let boundary = self.boundary_for_toolpath_id(toolpath_id)?;
        Some(boundary.start_move + local_move)
    }

    /// Progress within the current toolpath (0.0..1.0).
    pub fn current_toolpath_progress(&self) -> (usize, usize) {
        if let Some(b) = self.current_boundary() {
            let within = self.playback.current_move.saturating_sub(b.start_move);
            let total = b.end_move - b.start_move;
            (within, total)
        } else {
            (0, 0)
        }
    }

    /// Find the nearest checkpoint at or before the given move index.
    pub fn checkpoint_for_move(&self, move_idx: usize) -> Option<usize> {
        let boundaries = self.boundaries();
        let boundary_idx = boundaries.iter().position(|b| move_idx <= b.end_move)?;
        if boundary_idx == 0 {
            return None; // before the first toolpath, use initial stock
        }
        self.checkpoints()
            .iter()
            .position(|c| c.boundary_index == boundary_idx - 1)
    }
}
