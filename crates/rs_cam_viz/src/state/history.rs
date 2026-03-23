use super::job::{PostConfig, StockConfig, ToolConfig, ToolId};
use super::toolpath::{DressupConfig, OperationConfig, ToolpathId};
use rs_cam_core::enriched_mesh::FaceGroupId;
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
        old_face_selection: Option<Vec<FaceGroupId>>,
        new_face_selection: Option<Vec<FaceGroupId>>,
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
    pub toolpath_snapshot: Option<(
        ToolpathId,
        OperationConfig,
        DressupConfig,
        Option<Vec<FaceGroupId>>,
    )>,
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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    /// Helper: create a simple stock-change action with distinguishable stock sizes.
    fn stock_action(old_x: f64, new_x: f64) -> UndoAction {
        UndoAction::StockChange {
            old: StockConfig {
                x: old_x,
                ..StockConfig::default()
            },
            new: StockConfig {
                x: new_x,
                ..StockConfig::default()
            },
        }
    }

    fn extract_stock_old_x(action: &UndoAction) -> f64 {
        match action {
            UndoAction::StockChange { old, .. } => old.x,
            _ => panic!("Expected StockChange"),
        }
    }

    fn extract_stock_new_x(action: &UndoAction) -> f64 {
        match action {
            UndoAction::StockChange { new, .. } => new.x,
            _ => panic!("Expected StockChange"),
        }
    }

    #[test]
    fn push_and_undo_returns_correct_action() {
        let mut history = UndoHistory::new();
        history.push(stock_action(100.0, 200.0));

        assert!(history.can_undo());
        let undone = history.undo().expect("should have an action to undo");
        assert_eq!(extract_stock_old_x(&undone), 100.0);
        assert_eq!(extract_stock_new_x(&undone), 200.0);
        assert!(!history.can_undo());
    }

    #[test]
    fn redo_after_undo_returns_undone_state() {
        let mut history = UndoHistory::new();
        history.push(stock_action(100.0, 200.0));

        let undone = history.undo().expect("undo");
        assert!(history.can_redo());
        let redone = history.redo().expect("redo");

        assert_eq!(extract_stock_old_x(&redone), extract_stock_old_x(&undone));
        assert_eq!(extract_stock_new_x(&redone), extract_stock_new_x(&undone));
        assert!(!history.can_redo());
        assert!(history.can_undo());
    }

    #[test]
    fn new_push_after_undo_clears_redo_stack() {
        let mut history = UndoHistory::new();
        history.push(stock_action(100.0, 200.0));
        history.push(stock_action(200.0, 300.0));

        // Undo once
        history.undo().expect("undo");
        assert!(history.can_redo());

        // Push a new action — redo stack should be cleared
        history.push(stock_action(200.0, 400.0));
        assert!(
            !history.can_redo(),
            "Redo stack should be cleared after new push"
        );

        // Undo should get the newly pushed action
        let undone = history.undo().expect("undo newly pushed");
        assert_eq!(extract_stock_new_x(&undone), 400.0);
    }

    #[test]
    fn stack_overflow_drops_oldest() {
        let mut history = UndoHistory::new();

        // Push 105 actions (overflow threshold is 100)
        for i in 0..105 {
            history.push(stock_action(i as f64, (i + 1) as f64));
        }

        // Should have at most 100 entries
        let mut count = 0;
        while history.undo().is_some() {
            count += 1;
        }
        assert_eq!(count, 100, "Stack should cap at 100 entries");
    }

    #[test]
    fn empty_undo_redo_returns_none() {
        let mut history = UndoHistory::new();

        assert!(!history.can_undo());
        assert!(!history.can_redo());
        assert!(history.undo().is_none());
        assert!(history.redo().is_none());
    }

    #[test]
    fn multiple_undo_redo_sequence() {
        let mut history = UndoHistory::new();
        history.push(stock_action(10.0, 20.0));
        history.push(stock_action(20.0, 30.0));
        history.push(stock_action(30.0, 40.0));

        // Undo all three
        let u3 = history.undo().expect("undo 3rd");
        assert_eq!(extract_stock_new_x(&u3), 40.0);
        let u2 = history.undo().expect("undo 2nd");
        assert_eq!(extract_stock_new_x(&u2), 30.0);
        let u1 = history.undo().expect("undo 1st");
        assert_eq!(extract_stock_new_x(&u1), 20.0);
        assert!(history.undo().is_none());

        // Redo all three
        let r1 = history.redo().expect("redo 1st");
        assert_eq!(extract_stock_new_x(&r1), 20.0);
        let r2 = history.redo().expect("redo 2nd");
        assert_eq!(extract_stock_new_x(&r2), 30.0);
        let r3 = history.redo().expect("redo 3rd");
        assert_eq!(extract_stock_new_x(&r3), 40.0);
        assert!(history.redo().is_none());
    }
}
