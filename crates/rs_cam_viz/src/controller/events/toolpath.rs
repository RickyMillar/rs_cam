use crate::compute::ComputeBackend;
use crate::state::selection::Selection;
use crate::state::toolpath::{OperationConfig, ToolpathEntry, ToolpathId};
use crate::state::Workspace;
use crate::ui::AppEvent;

use super::super::AppController;

impl<B: ComputeBackend> AppController<B> {
    // ── Toolpath helpers ─────────────────────────────────────────────────

    pub(crate) fn handle_add_toolpath(&mut self, op_type: crate::state::toolpath::OperationType) {
        let target_setup_id = match self.state.selection {
            Selection::Toolpath(tp_id) => self.state.job.setup_of_toolpath(tp_id),
            Selection::Setup(setup_id) => Some(setup_id),
            Selection::Fixture(setup_id, _) => Some(setup_id),
            Selection::KeepOut(setup_id, _) => Some(setup_id),
            _ => None,
        }
        .or_else(|| self.state.job.setups.first().map(|setup| setup.id));

        let Some(tool_id) = self.state.job.tools.first().map(|tool| tool.id) else {
            tracing::warn!("Cannot add toolpath: no tools defined");
            self.push_notification(
                "Cannot add toolpath: no tools defined".into(),
                super::super::Severity::Warning,
            );
            return;
        };
        let operation = OperationConfig::new_default(op_type);
        if !operation.is_stock_based() && self.state.job.models.is_empty() {
            let msg = format!(
                "Cannot add {} toolpath: import geometry first",
                operation.label()
            );
            tracing::warn!("{msg}");
            self.push_notification(msg, super::super::Severity::Warning);
            return;
        }
        let id = self.state.job.next_toolpath_id();
        let model_id = self
            .state
            .job
            .models
            .first()
            .map(|model| model.id)
            .unwrap_or(crate::state::job::ModelId(0));
        let entry = ToolpathEntry::new(
            id,
            format!("{} {}", op_type.label(), id.0 + 1),
            tool_id,
            model_id,
            operation,
        );
        self.state.selection = Selection::Toolpath(id);
        if let Some(setup_id) = target_setup_id {
            self.state.job.push_toolpath_to_setup(setup_id, entry);
        } else {
            self.state.job.push_toolpath(entry);
        }
        self.state.job.mark_edited();
    }

    pub(crate) fn handle_duplicate_toolpath(&mut self, tp_id: ToolpathId) {
        let new_id = self.state.job.next_toolpath_id();
        let target_setup_id = self.state.job.setup_of_toolpath(tp_id);
        if let Some(src) = self.state.job.find_toolpath(tp_id) {
            self.state.selection = Selection::Toolpath(new_id);
            let entry = src.duplicate_as(new_id, format!("{} (copy)", src.name));
            if let Some(setup_id) = target_setup_id {
                self.state.job.push_toolpath_to_setup(setup_id, entry);
            } else {
                self.state.job.push_toolpath(entry);
            }
            self.state.job.mark_edited();
        }
    }

    pub(crate) fn handle_move_toolpath_up(&mut self, tp_id: ToolpathId) {
        if self.state.job.move_toolpath_up(tp_id) {
            self.state.job.mark_edited();
        }
    }

    pub(crate) fn handle_move_toolpath_down(&mut self, tp_id: ToolpathId) {
        if self.state.job.move_toolpath_down(tp_id) {
            self.state.job.mark_edited();
        }
    }

    pub(crate) fn handle_reorder_toolpath(&mut self, tp_id: ToolpathId, target_idx: usize) {
        if self.state.job.reorder_toolpath(tp_id, target_idx) {
            self.state.job.mark_edited();
        }
    }

    pub(crate) fn handle_move_toolpath_to_setup(
        &mut self,
        tp_id: ToolpathId,
        setup_id: crate::state::job::SetupId,
        idx: usize,
    ) {
        if self.state.job.move_toolpath_to_setup(tp_id, setup_id, idx) {
            self.pending_upload = true;
            self.state.job.mark_edited();
        }
    }

    pub(crate) fn handle_remove_toolpath(&mut self, tp_id: ToolpathId) {
        self.state.job.remove_toolpath(tp_id);
        if self.state.selection == Selection::Toolpath(tp_id) {
            self.state.selection = Selection::None;
        }
        if self.state.viewport.isolate_toolpath == Some(tp_id) {
            self.state.viewport.isolate_toolpath = None;
        }
        self.pending_upload = true;
        self.state.job.mark_edited();
    }

    pub(crate) fn handle_generate_all(&mut self) {
        let ids: Vec<_> = self
            .state
            .job
            .all_toolpaths()
            .map(|toolpath| toolpath.id)
            .collect();
        for id in ids {
            self.submit_toolpath_compute(id);
        }
    }

    pub(crate) fn handle_toggle_isolate_toolpath(&mut self) {
        if let Selection::Toolpath(id) = self.state.selection {
            if self.state.viewport.isolate_toolpath == Some(id) {
                self.state.viewport.isolate_toolpath = None;
            } else {
                self.state.viewport.isolate_toolpath = Some(id);
            }
            self.pending_upload = true;
        }
    }

    pub(crate) fn handle_inspect_toolpath_in_simulation(&mut self, tp_id: ToolpathId) {
        self.events
            .push(AppEvent::SwitchWorkspace(Workspace::Simulation));
        if let Some(boundary) = self
            .state
            .simulation
            .boundaries()
            .iter()
            .find(|boundary| boundary.id == tp_id)
        {
            self.events
                .push(AppEvent::SimJumpToMove(boundary.start_move));
        } else if self
            .state
            .job
            .find_toolpath(tp_id)
            .and_then(|toolpath| toolpath.result.as_ref())
            .is_some()
        {
            self.state.simulation.debug.pending_inspect_toolpath = Some(tp_id);
            self.events.push(AppEvent::RunSimulationWith(vec![tp_id]));
        }
    }
}
