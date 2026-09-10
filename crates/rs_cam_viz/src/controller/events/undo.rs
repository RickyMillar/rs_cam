use crate::compute::ComputeBackend;
use crate::state::history::UndoAction;
use crate::state::job::{ToolConfig, ToolId};

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
                    self.apply_tool_snapshot(tool_id, old);
                }
                UndoAction::ToolpathParamChange {
                    tp_id,
                    old_op,
                    old_dressups,
                    old_face_selection,
                    ..
                } => {
                    self.apply_toolpath_snapshot(tp_id, old_op, old_dressups, old_face_selection);
                }
                UndoAction::MachineChange { old, .. } => {
                    self.state.session.set_machine(old);
                    self.invalidate_simulation();
                }
            }
            // F2.5 (G-UNDOFRESH) — EVERY arm, at the one place all of them
            // pass through.
            //
            // `mark_edited` does two things nothing else here did: it sets
            // `dirty`, so an undone project is offered for saving and the
            // close interception warns; and it bumps `edit_counter`, which is
            // what `SimulationState::is_stale` compares against. Without it an
            // undo left the GUI showing a simulation computed from the
            // configuration the undo had just discarded, and calling it fresh.
            //
            // Placed after the match rather than in each arm so a sixth
            // `UndoAction` cannot be added without it — the defect this
            // closes is that five arms each had to remember, and none did.
            self.state.gui.mark_edited();
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
                    self.apply_tool_snapshot(tool_id, new);
                }
                UndoAction::ToolpathParamChange {
                    tp_id,
                    new_op,
                    new_dressups,
                    new_face_selection,
                    ..
                } => {
                    self.apply_toolpath_snapshot(tp_id, new_op, new_dressups, new_face_selection);
                }
                UndoAction::MachineChange { new, .. } => {
                    self.state.session.set_machine(new);
                    self.invalidate_simulation();
                }
            }
            // Redo carries the identical defect and takes the identical fix.
            // It is the same edit, applied in the other direction.
            self.state.gui.mark_edited();
        }
    }

    /// Install a tool snapshot the way a hand tool edit installs one.
    ///
    /// F2.5 (G-UNDOFRESH). Both undo arms used to write `tools_mut()`
    /// directly and then clear the simulation, which left every operation
    /// this tool machines holding a CORE result generated with the other
    /// tool's geometry — reading `Current` on every surface F2.2 wired, and
    /// exportable through the gate F2.3 built. `invalidate_tool` is the door
    /// `commit_tool_draft` goes through, and it knows about the second
    /// dependency door too (a planned-tier ladder that names the tool).
    ///
    /// The regeneration request mirrors `commit_tool_draft` for the same
    /// reason it exists there: the affected operations did not ask to be
    /// invalidated, so the sweep should pick them up rather than the operator
    /// hunting for them.
    fn apply_tool_snapshot(&mut self, tool_id: ToolId, tool: ToolConfig) {
        if let Some(slot) = self
            .state
            .session
            .tools_mut()
            .iter_mut()
            .find(|t| t.id == tool_id)
        {
            *slot = tool;
        }
        let affected = self.state.session.invalidate_tool(tool_id.0);
        let now = std::time::Instant::now();
        for index in affected {
            if let Some(tc) = self.state.session.get_toolpath_config(index) {
                let id = tc.id;
                self.state.gui.toolpath_rt_or_default(id).stale_since = Some(now);
            }
        }
        self.invalidate_simulation();
    }

    /// Install an operation/dressup/face snapshot.
    ///
    /// `apply_toolpath_param_snapshot` drops the core result and bumps the
    /// revision, so the operation reads `EditedSince` afterwards — including
    /// when the undo has restored the exact configuration the retained
    /// geometry was generated from. **That is deliberate; see `F2.5.md` §3.**
    /// In short: nothing records which configuration the retained
    /// `ToolpathRuntime::result` actually answers, the snapshot restores three
    /// fields out of the nine that decide the geometry, and being wrong the
    /// conservative way costs a regeneration while being wrong the other way
    /// exports a program that does not match the project.
    fn apply_toolpath_snapshot(
        &mut self,
        tp_id: crate::state::toolpath::ToolpathId,
        operation: crate::state::toolpath::OperationConfig,
        dressups: crate::state::toolpath::DressupConfig,
        face_selection: Option<Vec<rs_cam_core::enriched_mesh::FaceGroupId>>,
    ) {
        if let Some((idx, _)) = self.state.session.find_toolpath_config_by_id(tp_id) {
            let _ = self.state.session.apply_toolpath_param_snapshot(
                idx,
                operation,
                dressups,
                face_selection,
            );
            if let Some(rt) = self.state.gui.toolpath_rt.get_mut(&tp_id) {
                rt.stale_since = Some(std::time::Instant::now());
            }
        }
        // No `invalidate_simulation` here, deliberately: a hand parameter
        // edit does not clear the simulation either, it stales it — and the
        // rule this task implements is that an undo leaves the project where
        // the same edit made by hand would. The `mark_edited` at the end of
        // the caller is what does the staling.
    }
}
