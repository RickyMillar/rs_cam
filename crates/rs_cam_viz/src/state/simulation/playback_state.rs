//! Playback state: what the simulation is showing right now.
//!
//! The methods build a `SimulationState`, report whether it holds a result,
//! read the boundaries and the checkpoints, advance the playhead and map a
//! global move index to a local one. They are an `impl SimulationState`
//! fragment, so every name keeps its path.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use rs_cam_core::dexel_stock::TriDexelStock;

use super::{
    CutMetricCache, HolderCheckScope, IssueListCache, SetupBoundary, SimCheckpoint,
    SimulationChecks, SimulationDebugState, SimulationPlayback, SimulationState,
    SimulationTriageCache, SpanAggregateCache, SpanScope, StockVizMode, ToolpathBoundary,
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
                checked_at_epoch: None,
                checked_scope: HolderCheckScope::default(),
            },
            last_run: None,
            submitted_edit_counter: None,
            submitted_simulation_epoch: None,
            submitted_collision_epoch: None,
            submitted_collision_scope: None,
            resolution_draft: None,
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
                triage_cache: SimulationTriageCache::default(),
                cut_metric_cache: CutMetricCache::default(),
                issue_cache: IssueListCache::default(),
                semantic_indexes: HashMap::new(),
            },
            cut_metric_over_time: Vec::new(),
            open_trace: None,
            trace_window: None,
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

    /// True when a collision check HAS run and the project has moved under
    /// it since it was submitted (F2.12, G-HOLDERSTALE).
    ///
    /// A project with no check returns `false` here. That is not "clear": it
    /// is "there is nothing to be stale", and
    /// [`crate::ui::readiness::holder_clearance_check`] separates the two by
    /// reading [`SimulationChecks::checked_at_epoch`] first.
    ///
    /// The epoch is project-wide, so an edit that could not have changed the
    /// holder verdict still stales it. That coarseness is F2.10 §3's,
    /// accepted for the same reason: the cost is one re-check, and the cost
    /// of the other error is telling an operator a cut is clear on evidence
    /// that belongs to a different machine setup.
    ///
    /// W4: the stamp is `ProjectSession::simulation_epoch`, not the GUI edit
    /// counter. The check reads the holder assembly, the workholding
    /// obstacles, the emitted motion and the setup frame, and every core
    /// door that writes one of those calls `drop_simulation`, which is the
    /// one site that moves the epoch. The counter moved for edits that write
    /// no project data (an export wizard field, a machine saved to the
    /// library) and withdrew a verdict none of them could have changed.
    pub fn collision_check_is_stale(&self, current_epoch: u64) -> bool {
        self.checks
            .checked_at_epoch
            .is_some_and(|checked_at| current_epoch > checked_at)
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

    /// The boundary under the playback CURSOR.
    ///
    /// The cursor is a count of played moves, `0..=total_moves`, not a move
    /// index. See [`Self::cursor_to_local_toolpath_move`] for the rule.
    pub fn current_boundary(&self) -> Option<&ToolpathBoundary> {
        self.cursor_to_local_toolpath_move(self.playback.current_move)
            .and_then(|(boundary_index, _, _)| self.boundaries().get(boundary_index))
    }

    /// The toolpath in focus for graph filtering and viewport-marker
    /// filtering. Tracks the playback position — focus follows whatever TP
    /// is currently playing. Selection in the left panel jumps playback to
    /// the chosen TP, which then becomes the focus via this getter; there's
    /// no separate sticky pin.
    pub fn focused_toolpath(&self) -> Option<ToolpathId> {
        self.current_boundary().map(|b| b.id)
    }

    /// G-RAPIDFRAME: the boundary, toolpath and local move of the run-global
    /// MOVE INDEX `global_move`.
    ///
    /// This is the one attribution of a move number to a toolpath
    /// ([`rs_cam_core::compute::simulate::locate_global_move`]), the same
    /// function core diagnostics and the triage use. A boundary is the
    /// half-open range `[start_move, end_move)`: the move at `end_move` is
    /// the first move of the NEXT toolpath. Use this for every collision,
    /// issue and marker move.
    pub fn locate_move(
        &self,
        global_move: usize,
    ) -> Option<rs_cam_core::compute::simulate::MoveLocation> {
        rs_cam_core::compute::simulate::locate_global_move(
            self.boundaries().iter().map(ToolpathBoundary::range),
            global_move,
        )
    }

    /// The boundary, toolpath and local move of the playback CURSOR.
    ///
    /// The cursor counts played moves, so it runs to `total_moves`. Below
    /// that value, the cursor at `c` shows move `c` next, and the mapping is
    /// [`Self::locate_move`]. At `total_moves` no move is next: the cursor
    /// stays on the last boundary, at local move `end_move - start_move`.
    /// Do not use this for a move index; use [`Self::locate_move`].
    pub fn cursor_to_local_toolpath_move(
        &self,
        cursor: usize,
    ) -> Option<(usize, ToolpathId, usize)> {
        if let Some(loc) = self.locate_move(cursor) {
            return Some((loc.boundary_index, loc.toolpath_id, loc.local_move));
        }
        let last_index = self.boundaries().len().checked_sub(1)?;
        let last = self.boundaries().get(last_index)?;
        (cursor == last.end_move).then(|| {
            (
                last_index,
                last.id,
                last.end_move.saturating_sub(last.start_move),
            )
        })
    }

    pub fn current_local_toolpath_move(&self) -> Option<(usize, ToolpathId, usize)> {
        self.cursor_to_local_toolpath_move(self.playback.current_move)
    }

    /// G-RAPIDFRAME: every holder/shank collision of the last dedicated
    /// check, with its toolpath and both move frames.
    ///
    /// The check walks ONE toolpath, so `CollisionEvent::move_index` is
    /// that toolpath's own move index. The toolpath comes from the check's
    /// scope ([`HolderCheckScope::toolpath_id`]), and the run-global move
    /// from [`rs_cam_core::compute::simulate::global_move_of_local`].
    /// `global_move` is `None` when the simulated run holds no such move,
    /// for example when the checked toolpath is disabled. Every consumer
    /// reads this list; none treats the raw index as a run move.
    pub fn located_holder_collisions(&self) -> Vec<LocatedHolderCollision<'_>> {
        let Some(report) = self.checks.collision_report.as_ref() else {
            return Vec::new();
        };
        let toolpath_id = self.checks.checked_scope.toolpath_id;
        report
            .collisions
            .iter()
            .map(|event| LocatedHolderCollision {
                event,
                toolpath_id,
                local_move: event.move_index,
                global_move: toolpath_id.and_then(|id| {
                    rs_cam_core::compute::simulate::global_move_of_local(
                        self.boundaries().iter().map(ToolpathBoundary::range),
                        id,
                        event.move_index,
                    )
                }),
            })
            .collect()
    }

    /// Per-toolpath holder/shank collision counts from the last
    /// dedicated collision check, attributed by the check's own toolpath.
    /// Empty when no check has run. This is the holder evidence the
    /// core's `diagnostics_with_evidence` consumes — derived from the
    /// stored report (O(collisions)), never recomputed, so it is safe
    /// to call at frame rate (the 2026-06-11 setup-tab lag was the
    /// diagnostics path re-running the full collision sweep per frame).
    ///
    /// G-RAPIDFRAME: the count does not need the timeline. It used to look
    /// up the toolpath's LOCAL move index in the run-global boundaries,
    /// which gave the hits to whichever toolpath held that run move.
    pub(crate) fn holder_collision_counts_by_tp(
        &self,
    ) -> Vec<(
        ToolpathId,
        rs_cam_core::stock::collision::HolderCollisionCheck,
    )> {
        use rs_cam_core::stock::collision::HolderCollisionCheck;
        let count = self
            .checks
            .collision_report
            .as_ref()
            .map_or(0, |report| report.collisions.len());
        // Only a toolpath the report found HITS on appears here. A toolpath
        // the GUI never checked is absent, and core reads absence as "not
        // measured" — it must not be listed as a measured zero (CMP-14).
        match self.checks.checked_scope.toolpath_id {
            Some(id) if count > 0 => vec![(id, HolderCollisionCheck::Measured(count))],
            _ => Vec::new(),
        }
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

/// One holder/shank collision in both move frames. See
/// [`SimulationState::located_holder_collisions`].
#[derive(Debug, Clone, Copy)]
pub struct LocatedHolderCollision<'a> {
    pub event: &'a rs_cam_core::stock::collision::CollisionEvent,
    /// The checked toolpath. `None` when the report carries no scope.
    pub toolpath_id: Option<ToolpathId>,
    /// The toolpath's own move index (`CollisionEvent::move_index`).
    pub local_move: usize,
    /// The run-global move index. `None` when the run holds no such move.
    pub global_move: Option<usize>,
}
