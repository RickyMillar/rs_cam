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

use rs_cam_core::session::{MultitoolPlanOutcome, MultitoolPlanSpec};

use crate::compute::ComputeBackend;
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
}
