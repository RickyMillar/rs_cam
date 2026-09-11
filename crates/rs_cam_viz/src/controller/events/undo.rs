use rs_cam_core::session::{
    Command, ReplaceToolArgs, RestoreToolpathSnapshotArgs, SetMachineArgs, SetPostConfigArgs,
    SetStockConfigArgs,
};

use crate::compute::ComputeBackend;
use crate::state::history::UndoAction;
use crate::state::job::{ToolConfig, ToolId};

use super::super::AppController;

impl<B: ComputeBackend> AppController<B> {
    pub(crate) fn undo(&mut self) {
        if let Some(action) = self.state.history.undo() {
            match action {
                UndoAction::StockChange { old, .. } => {
                    let command = Command::SetStockConfig(SetStockConfigArgs {
                        stock: Box::new(old),
                    });
                    let _ = self.apply_quietly(command);
                    self.invalidate_simulation();
                }
                UndoAction::PostChange { old, .. } => {
                    self.state.gui.post = old;
                    let session_post =
                        crate::state::runtime::GuiState::post_to_session(&self.state.gui.post);
                    let command = Command::SetPostConfig(SetPostConfigArgs {
                        post: Box::new(session_post),
                    });
                    let _ = self.apply_quietly(command);
                }
                UndoAction::ToolChange { tool_id, old, .. } => {
                    self.apply_tool_snapshot(tool_id, old);
                }
                UndoAction::ToolpathParamChange {
                    tp_id,
                    old_op,
                    old_dressups,
                    old_face_selection,
                    old_feeds_provenance,
                    ..
                } => {
                    self.apply_toolpath_snapshot(
                        tp_id,
                        old_op,
                        old_dressups,
                        old_face_selection,
                        old_feeds_provenance,
                    );
                }
                UndoAction::MachineChange { old, .. } => {
                    let command = Command::SetMachine(SetMachineArgs {
                        machine: Box::new(old),
                    });
                    let _ = self.apply_quietly(command);
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
                    let command = Command::SetStockConfig(SetStockConfigArgs {
                        stock: Box::new(new),
                    });
                    let _ = self.apply_quietly(command);
                    self.invalidate_simulation();
                }
                UndoAction::PostChange { new, .. } => {
                    self.state.gui.post = new;
                    let session_post =
                        crate::state::runtime::GuiState::post_to_session(&self.state.gui.post);
                    let command = Command::SetPostConfig(SetPostConfigArgs {
                        post: Box::new(session_post),
                    });
                    let _ = self.apply_quietly(command);
                }
                UndoAction::ToolChange { tool_id, new, .. } => {
                    self.apply_tool_snapshot(tool_id, new);
                }
                UndoAction::ToolpathParamChange {
                    tp_id,
                    new_op,
                    new_dressups,
                    new_face_selection,
                    new_feeds_provenance,
                    ..
                } => {
                    self.apply_toolpath_snapshot(
                        tp_id,
                        new_op,
                        new_dressups,
                        new_face_selection,
                        new_feeds_provenance,
                    );
                }
                UndoAction::MachineChange { new, .. } => {
                    let command = Command::SetMachine(SetMachineArgs {
                        machine: Box::new(new),
                    });
                    let _ = self.apply_quietly(command);
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
        // `Command::ReplaceTool` writes the record and runs
        // `invalidate_tool` in ONE mutation, so the write and the drop
        // cannot drift apart. The row is the one the tool panel's
        // `commit_tool_draft` takes (WP6).
        let command = Command::ReplaceTool(ReplaceToolArgs {
            tool_id: tool_id.0,
            config: Box::new(tool),
        });
        self.apply_controller_command(command, "the tool");
        self.invalidate_simulation();
    }

    /// Install an operation, dressup, face and provenance snapshot.
    ///
    /// `Command::RestoreToolpathSnapshot` drops the core result and bumps
    /// the revision, so the operation reads `EditedSince` afterwards —
    /// including when the undo has restored the exact configuration the
    /// retained geometry was generated from. **That is deliberate; see
    /// `F2.5.md` §3.** In short: nothing records which configuration the
    /// retained `ToolpathRuntime::result` actually answers, the snapshot
    /// restores three fields out of the nine that decide the geometry, and
    /// being wrong the conservative way costs a regeneration while being
    /// wrong the other way exports a program that does not match the
    /// project. The command is unconditional for that reason, and a
    /// signature-gated command cannot take its place.
    ///
    /// WP8 (N14) widened both halves of this. The command drops the
    /// downstream stock chain, not the edited index alone, and the stamp
    /// below follows `Effects::stale` rather than naming one toolpath.
    fn apply_toolpath_snapshot(
        &mut self,
        tp_id: crate::state::toolpath::ToolpathId,
        operation: crate::state::toolpath::OperationConfig,
        dressups: crate::state::toolpath::DressupConfig,
        face_selection: Option<Vec<rs_cam_core::enriched_mesh::FaceGroupId>>,
        feeds_provenance: rs_cam_core::feeds::FeedsProvenance,
    ) {
        if let Some((idx, _)) = self.state.session.find_toolpath_config_by_id(tp_id) {
            let restored = self.state.session.apply(Command::RestoreToolpathSnapshot(
                RestoreToolpathSnapshotArgs {
                    index: idx,
                    operation: Box::new(operation),
                    dressups: Box::new(dressups),
                    face_selection,
                    feeds_provenance: Some(Box::new(feeds_provenance)),
                },
            ));
            if let Ok(effects) = restored {
                crate::state::stale::stamp_stale(&mut self.state, &effects.stale);
            }
        }
        // No `invalidate_simulation` here, deliberately: a hand parameter
        // edit does not clear the simulation either, it stales it — and the
        // rule this task implements is that an undo leaves the project where
        // the same edit made by hand would. The `mark_edited` at the end of
        // the caller is what does the staling.
    }
}
