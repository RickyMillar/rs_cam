use crate::compute::ComputeBackend;
use crate::state::history::UndoAction;

use super::super::AppController;

impl<B: ComputeBackend> AppController<B> {
    pub(crate) fn undo(&mut self) {
        if let Some(action) = self.state.history.undo() {
            match action {
                UndoAction::StockChange { old, .. } => {
                    self.state.session.set_stock_config(old);
                    self.invalidate_simulation();
                }
                UndoAction::PostChange { old, .. } => {
                    self.state.gui.post = old;
                    let session_post =
                        crate::state::runtime::GuiState::post_to_session(&self.state.gui.post);
                    self.state.session.set_post_config(session_post);
                }
                UndoAction::ToolChange { tool_id, old, .. } => {
                    if let Some(tool) = self
                        .state
                        .session
                        .tools_mut()
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
                    if let Some((idx, _)) =
                        self.state.session.find_toolpath_config_by_id(tp_id.0)
                    {
                        if let Some(tc) =
                            self.state.session.toolpath_configs_mut().get_mut(idx)
                        {
                            tc.operation = old_op;
                            tc.dressups = old_dressups;
                            tc.face_selection = old_face_selection;
                        }
                        if let Some(rt) = self.state.gui.toolpath_rt.get_mut(&tp_id.0) {
                            rt.stale_since = Some(std::time::Instant::now());
                        }
                    }
                }
                UndoAction::MachineChange { old, .. } => {
                    self.state.session.set_machine(old);
                    self.invalidate_simulation();
                }
            }
        }
    }

    pub(crate) fn redo(&mut self) {
        if let Some(action) = self.state.history.redo() {
            match action {
                UndoAction::StockChange { new, .. } => {
                    self.state.session.set_stock_config(new);
                    self.invalidate_simulation();
                }
                UndoAction::PostChange { new, .. } => {
                    self.state.gui.post = new;
                    let session_post =
                        crate::state::runtime::GuiState::post_to_session(&self.state.gui.post);
                    self.state.session.set_post_config(session_post);
                }
                UndoAction::ToolChange { tool_id, new, .. } => {
                    if let Some(tool) = self
                        .state
                        .session
                        .tools_mut()
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
                    if let Some((idx, _)) =
                        self.state.session.find_toolpath_config_by_id(tp_id.0)
                    {
                        if let Some(tc) =
                            self.state.session.toolpath_configs_mut().get_mut(idx)
                        {
                            tc.operation = new_op;
                            tc.dressups = new_dressups;
                            tc.face_selection = new_face_selection;
                        }
                        if let Some(rt) = self.state.gui.toolpath_rt.get_mut(&tp_id.0) {
                            rt.stale_since = Some(std::time::Instant::now());
                        }
                    }
                }
                UndoAction::MachineChange { new, .. } => {
                    self.state.session.set_machine(new);
                    self.invalidate_simulation();
                }
            }
        }
    }
}
