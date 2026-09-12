//! Phase O — the multi-tool finishing planner's reconciler.
//!
//! The core planner emits (and replaces) session toolpaths; everything the
//! GUI keeps *beside* the session — per-op runtime, selection, isolation, the
//! GPU upload flag — has to be brought back into step afterwards. That is this
//! module, and it is the ONE path: the MCP tool calls it today and Phase U's
//! planner dialog calls the same method, so the two surfaces cannot drift.
//!
//! Reconciler precedent: `events::model::sync_alignment_pin_drill`, extended
//! to many ops at once.
//!
//! # Phase U — the dialog's four verbs
//!
//! Open / Preview / Apply / Close also live here, because they are the same
//! seam: the dialog edits a [`MultitoolPlanSpec`], the preview computes what
//! that spec would claim, and Apply hands it to [`AppController::apply_multitool_plan`]
//! above.
//!
//! WP14b (§28 ruling 7): the preview submits the `preview_tier_map` `Job`
//! row on the Job lane. Step (i) captures the ladder and the geometry on
//! the frame loop, so the walk holds no session and the view keeps its
//! own. It used to `mem::replace` the session into an Optimize-lane
//! request and hold an empty placeholder behind `is_optimizing`. Cancel
//! is now this submit's own flag rather than the lane's, because the Job
//! lane is FIFO and shared with the MCP surface.

use rs_cam_core::compute::cutter::build_cutter;
use rs_cam_core::session::{MultitoolPlanOutcome, MultitoolPlanSpec};
use rs_cam_core::tool::MillingCutter;

use crate::compute::ComputeBackend;
use crate::state::multitool_planner::{
    MultitoolPlannerState, MultitoolPreviewStatus, PlannerToolRow,
};
use crate::state::selection::Selection;

use super::super::AppController;

impl<B: ComputeBackend> AppController<B> {
    /// Run the multi-tool finishing planner and reconcile GUI runtime state
    /// with what it emitted.
    ///
    /// Planning is cheap — no tier map is built here; each emitted op resolves
    /// its own tier boundary lazily at generation — so this is safe on the
    /// frame loop. The emitted chain is NOT generated: the ops land stale and
    /// the operator (or `generate_all`'s fixpoint ladder) generates them.
    ///
    /// # Errors
    /// The core planner's own refusal, rendered. Nothing is mutated when it
    /// refuses — the reconciliation below only runs on success.
    pub fn apply_multitool_plan(
        &mut self,
        spec: &MultitoolPlanSpec,
    ) -> Result<MultitoolPlanOutcome, String> {
        let outcome = self
            .state
            .session
            .plan_multitool_finishing(spec)
            .map_err(|e| e.to_string())?;

        // Ops the planner superseded: the same teardown `handle_remove_toolpath`
        // does, minus the session removal the planner already performed.
        for id in &outcome.replaced {
            self.state.gui.toolpath_rt.remove(id);
            // A consumer whose `DerivedRestRegions` boundary pointed at a
            // replaced op just lost its source; force it to regenerate so it
            // fails loudly instead of clipping against orphaned regions.
            self.mark_derived_rest_dependents_stale(*id);
            if self.state.selection == Selection::Toolpath(*id) {
                self.state.selection = Selection::None;
            }
            if self.state.viewport.isolate_toolpath == Some(*id) {
                self.state.viewport.isolate_toolpath = None;
            }
        }

        // Emitted ops: the same runtime entry `handle_add_toolpath` creates.
        // `insert`, not `entry`, so a recycled id cannot inherit a previous
        // op's cached result.
        for id in &outcome.toolpath_ids {
            let auto_regen = self
                .state
                .session
                .find_toolpath_config_by_id(*id)
                .is_none_or(|(_, tc)| tc.operation.default_auto_regen());
            self.state
                .gui
                .toolpath_rt
                .insert(*id, crate::state::runtime::ToolpathRuntime::new(auto_regen));
        }

        self.pending_upload = true;
        self.state.gui.mark_edited();
        Ok(outcome)
    }

    // ── Phase U: the planner dialog ─────────────────────────────────────

    /// Open the planner, snapshotting the drawer.
    ///
    /// Re-opening a dialog that was vetoed restores it whole — ladder, dials
    /// and any held preview — rather than building a fresh one, which is what
    /// makes "reject re-opens the ladder dialog" (§3.1) a cheap loop. Only the
    /// tool ROWS are refreshed, because a tool may have been re-dialled or
    /// removed in between; ticks survive by id.
    pub(crate) fn open_multitool_planner(&mut self) {
        let rows = self.planner_tool_rows();
        if rows.len() < 2 {
            self.push_notification(
                "Multi-tool finishing needs at least two tools in the drawer.".to_owned(),
                crate::controller::Severity::Warning,
            );
            return;
        }
        let Some((model_id, model_name)) = self.planner_model() else {
            self.push_notification(
                "Multi-tool finishing needs a 3D model — the tier map is a drop-cutter \
                 residual over a surface."
                    .to_owned(),
                crate::controller::Severity::Warning,
            );
            return;
        };
        let setup_index = self.planner_setup_index();

        self.state.close_modals_for_exclusivity();
        if self.state.multitool_planner.is_none() {
            self.state.multitool_planner = Some(MultitoolPlannerState::new(
                rows,
                setup_index,
                model_id,
                model_name,
            ));
            return;
        }
        let mut restore_overlay = false;
        if let Some(existing) = self.state.multitool_planner.as_mut() {
            let merged = merge_tool_ticks(&existing.tools, rows);
            existing.open = true;
            existing.apply_error = None;
            existing.tools = merged;
            existing.setup_index = setup_index;
            existing.model_id = model_id;
            existing.model_name = model_name;
            restore_overlay = existing.ready_preview().is_some();
        }
        // A held preview comes back with its overlay, so a re-opened veto
        // shows the operator the same territory they rejected.
        if restore_overlay {
            self.state.viewport.show_tier_preview = true;
            self.pending_upload = true;
        }
    }

    /// Submit a tier-map preview on the `Job` lane (WP14b).
    ///
    /// `ProjectSession::start` captures the ladder and the geometry here,
    /// on the frame loop, so the walk holds no session and the view keeps
    /// its own. A refusal — a ladder tool the project no longer carries,
    /// or no mesh — therefore appears at SUBMIT time and lands on the
    /// dialog's own `Failed` status, where it used to arrive through the
    /// lane.
    pub(crate) fn request_multitool_preview(&mut self) {
        if self.state.is_optimizing {
            tracing::warn!("Ignored PreviewMultitoolPlan — an Optimize run is already busy");
            return;
        }
        let Some(planner) = self.state.multitool_planner.as_mut() else {
            return;
        };
        if planner.blocking_reason().is_some() {
            return;
        }
        let spec = planner.to_spec();
        let key = planner.map_key();
        planner.requested_key = Some(key);
        planner.status = MultitoolPreviewStatus::Loading;
        planner.dirty_since = None;
        planner.apply_error = None;

        // §22 ruling 3: a FRESH flag per submit. The veto arms it.
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let started = self.state.session.start(
            rs_cam_core::session::Job::PreviewTierMap(rs_cam_core::session::PreviewTierMapArgs {
                spec: Box::new(spec),
            }),
            &cancel,
        );
        let handle = match started {
            Ok(handle) => handle,
            Err(error) => {
                if let Some(planner) = self.state.multitool_planner.as_mut() {
                    planner.requested_key = None;
                    planner.status = MultitoolPreviewStatus::Failed(error.to_string());
                }
                return;
            }
        };
        self.state.is_optimizing = true;
        self.submit_gui_job(
            handle,
            cancel,
            crate::controller::GuiJobTarget::MultitoolPreview,
        );
    }

    /// Emit the previewed ladder.
    ///
    /// Routes through [`Self::apply_multitool_plan`] — never
    /// `session.plan_multitool_finishing` directly — because that method owns
    /// the GUI-side bookkeeping (runtime entries for the emitted ops, teardown
    /// for the replaced ones, selection and isolation).
    ///
    /// On success the dialog closes but the overlay **stays up**: it is now a
    /// picture of what was planned, and dropping it at the moment the ops
    /// appear is the moment it is most useful.
    pub(crate) fn apply_multitool_planner(&mut self) {
        let Some(planner) = self.state.multitool_planner.as_ref() else {
            return;
        };
        if planner.ready_preview().is_none() {
            return;
        }
        let spec = planner.to_spec();
        match self.apply_multitool_plan(&spec) {
            Ok(outcome) => {
                let names: Vec<String> = outcome
                    .toolpath_ids
                    .iter()
                    .filter_map(|id| {
                        self.state
                            .session
                            .find_toolpath_config_by_id(*id)
                            .map(|(_, tc)| tc.name.clone())
                    })
                    .collect();
                let replaced = outcome.replaced.len();
                if let Some(planner) = self.state.multitool_planner.as_mut() {
                    planner.open = false;
                    planner.apply_error = None;
                }
                let suffix = if replaced == 0 {
                    String::new()
                } else {
                    format!(" (replaced {replaced} prior planner op(s))")
                };
                self.push_notification(
                    format!("Planned {}{suffix}", names.join(", ")),
                    crate::controller::Severity::Info,
                );
            }
            Err(e) => {
                if let Some(planner) = self.state.multitool_planner.as_mut() {
                    planner.apply_error = Some(e);
                }
            }
        }
    }

    /// The veto. Closes the dialog, drops the overlay, cancels an in-flight
    /// walk — and touches no toolpath.
    pub(crate) fn close_multitool_planner(&mut self) {
        let loading = self
            .state
            .multitool_planner
            .as_ref()
            .is_some_and(MultitoolPlannerState::is_loading);
        if loading && self.state.is_optimizing {
            // The walk polls this submit's OWN flag once per grid row.
            // Not the lane's: the Job lane is FIFO and shared, so a lane
            // cancel would also kill an MCP caller's job queued behind
            // this one. The answer still lands, and the drain is where
            // `is_optimizing` flips back to false.
            self.cancel_gui_optimize_jobs();
        }
        if let Some(planner) = self.state.multitool_planner.as_mut() {
            planner.open = false;
            planner.dirty_since = None;
        }
        self.state.viewport.show_tier_preview = false;
        self.pending_upload = true;
    }

    /// One row per drawer tool, sorted coarse → fine on **cusp** radius —
    /// the same ordering the core ladder uses, so the dialog reads in the
    /// order the chain will run. Nothing is pre-ticked: which tools take part
    /// is the operator's call (§3.2).
    fn planner_tool_rows(&self) -> Vec<PlannerToolRow> {
        let mut rows: Vec<PlannerToolRow> = self
            .state
            .session
            .tools()
            .iter()
            .map(|tool| PlannerToolRow {
                tool_id: tool.id.0,
                name: tool.name.clone(),
                cusp_radius_mm: build_cutter(tool).cusp_radius_mm(),
                selected: false,
                strategy: rs_cam_core::session::TierStrategy::UnifiedFinish,
            })
            .collect();
        rows.sort_by(|a, b| {
            b.cusp_radius_mm
                .total_cmp(&a.cusp_radius_mm)
                .then_with(|| a.tool_id.cmp(&b.tool_id))
        });
        rows
    }

    /// The model the chain targets: the selected toolpath's, when that model
    /// carries a 3D mesh, else the first mesh model in the project.
    ///
    /// Selection first so the model and the setup below come from the SAME
    /// toolpath — a plan whose setup came from the selection and whose model
    /// came from the project's first entry is a plan for a pairing the
    /// operator never chose.
    fn planner_model(&self) -> Option<(usize, String)> {
        let mesh_model = |id: usize| {
            self.state
                .session
                .models()
                .iter()
                .find(|m| m.id == id && m.mesh.is_some())
                .map(|m| (m.id, m.name.clone()))
        };
        if let Selection::Toolpath(tp_id) = self.state.selection
            && let Some((_, tc)) = self.state.session.find_toolpath_config_by_id(tp_id)
            && let Some(found) = mesh_model(tc.model_id)
        {
            return Some(found);
        }
        self.state
            .session
            .models()
            .iter()
            .find(|m| m.mesh.is_some())
            .map(|m| (m.id, m.name.clone()))
    }

    /// The setup the chain is emitted into: the one owning the current
    /// selection when there is one, else the first.
    fn planner_setup_index(&self) -> usize {
        let Selection::Toolpath(id) = self.state.selection else {
            return 0;
        };
        let Some((index, _)) = self.state.session.find_toolpath_config_by_id(id) else {
            return 0;
        };
        self.state
            .session
            .list_setups()
            .iter()
            .position(|s| s.toolpath_indices.contains(&index))
            .unwrap_or(0)
    }
}

/// Refresh the tool list against the drawer while keeping the operator's
/// ticks. Ticks follow the tool **id**, so a tool that was re-dialled stays
/// ticked with its new radius and one that was removed simply disappears.
fn merge_tool_ticks(
    previous: &[PlannerToolRow],
    mut fresh: Vec<PlannerToolRow>,
) -> Vec<PlannerToolRow> {
    for row in &mut fresh {
        row.selected = previous
            .iter()
            .find(|old| old.tool_id == row.tool_id)
            .is_some_and(|old| old.selected);
    }
    fresh
}
