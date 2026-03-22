use super::job::{PostConfig, StockConfig, ToolConfig, ToolId};
use super::toolpath::{DressupConfig, OperationConfig, ToolpathId};
use rs_cam_core::machine::MachineProfile;

/// A snapshot of undoable state.
#[derive(Debug, Clone)]
pub enum UndoAction {
    StockChange {
        old: StockConfig,
        new: StockConfig,
    },
    PostChange {
        old: PostConfig,
        new: PostConfig,
    },
    ToolChange {
        tool_id: ToolId,
        old: ToolConfig,
        new: ToolConfig,
    },
    ToolpathParamChange {
        tp_id: ToolpathId,
        old_op: OperationConfig,
        new_op: OperationConfig,
        old_dressups: DressupConfig,
        new_dressups: DressupConfig,
    },
    MachineChange {
        old: rs_cam_core::machine::MachineProfile,
        new: rs_cam_core::machine::MachineProfile,
    },
}

/// Simple undo/redo stack.
pub struct UndoHistory {
    undo_stack: Vec<UndoAction>,
    redo_stack: Vec<UndoAction>,
    /// Snapshot of stock config before current edit drag.
    pub stock_snapshot: Option<StockConfig>,
    /// Snapshot of tool config before current edit.
    pub tool_snapshot: Option<(ToolId, ToolConfig)>,
    /// Snapshot of post config before current edit.
    pub post_snapshot: Option<PostConfig>,
    /// Snapshot of machine config before current edit.
    pub machine_snapshot: Option<MachineProfile>,
    /// Snapshot of toolpath params before current edit.
    pub toolpath_snapshot: Option<(ToolpathId, OperationConfig, DressupConfig)>,
}

impl UndoHistory {
    pub fn new() -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            stock_snapshot: None,
            tool_snapshot: None,
            post_snapshot: None,
            machine_snapshot: None,
            toolpath_snapshot: None,
        }
    }

    pub fn push(&mut self, action: UndoAction) {
        self.undo_stack.push(action);
        self.redo_stack.clear();
        if self.undo_stack.len() > 100 {
            self.undo_stack.remove(0);
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn undo(&mut self) -> Option<UndoAction> {
        let action = self.undo_stack.pop()?;
        self.redo_stack.push(action.clone());
        Some(action)
    }

    pub fn redo(&mut self) -> Option<UndoAction> {
        let action = self.redo_stack.pop()?;
        self.undo_stack.push(action.clone());
        Some(action)
    }
}

impl Default for UndoHistory {
    fn default() -> Self {
        Self::new()
    }
}
