use crate::compute::ComputeBackend;
use crate::state::history::UndoAction;

use super::super::AppController;

impl<B: ComputeBackend> AppController<B> {
    pub(crate) fn undo(&mut self) {
        if let Some(action) = self.state.history.undo() {
            match action {
                UndoAction::StockChange { old, .. } => {
                    self.state.job.stock = old;
                    self.invalidate_simulation();
                }
                UndoAction::PostChange { old, .. } => {
                    self.state.job.post = old;
                }
                UndoAction::ToolChange { tool_id, old, .. } => {
                    if let Some(tool) = self
                        .state
                        .job
                        .tools
                        .iter_mut()
                        .find(|tool| tool.id == tool_id)
                    {
                        *tool = old;
                    }
                    self.invalidate_simulation();
                }
                UndoAction::ToolpathParamChange {
                    tp_id,
                    old_op,
                    old_dressups,
                    old_face_selection,
                    ..
                } => {
                    if let Some(toolpath) = self.state.job.find_toolpath_mut(tp_id) {
                        toolpath.operation = old_op;
                        toolpath.dressups = old_dressups;
                        toolpath.face_selection = old_face_selection;
                        toolpath.stale_since = Some(std::time::Instant::now());
                    }
                }
                UndoAction::MachineChange { old, .. } => {
                    self.state.job.machine = old;
                    self.invalidate_simulation();
                }
            }
        }
    }

    pub(crate) fn redo(&mut self) {
        if let Some(action) = self.state.history.redo() {
            match action {
                UndoAction::StockChange { new, .. } => {
                    self.state.job.stock = new;
                    self.invalidate_simulation();
                }
                UndoAction::PostChange { new, .. } => {
                    self.state.job.post = new;
                }
                UndoAction::ToolChange { tool_id, new, .. } => {
                    if let Some(tool) = self
                        .state
                        .job
                        .tools
                        .iter_mut()
                        .find(|tool| tool.id == tool_id)
                    {
                        *tool = new;
                    }
                    self.invalidate_simulation();
                }
                UndoAction::ToolpathParamChange {
                    tp_id,
                    new_op,
                    new_dressups,
                    new_face_selection,
                    ..
                } => {
                    if let Some(toolpath) = self.state.job.find_toolpath_mut(tp_id) {
                        toolpath.operation = new_op;
                        toolpath.dressups = new_dressups;
                        toolpath.face_selection = new_face_selection;
                        toolpath.stale_since = Some(std::time::Instant::now());
                    }
                }
                UndoAction::MachineChange { new, .. } => {
                    self.state.job.machine = new;
                    self.invalidate_simulation();
                }
            }
        }
    }
}
