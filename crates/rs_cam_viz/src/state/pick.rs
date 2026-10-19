//! The view state of the selection section (geometry selection P4.3): the
//! pick mode and the `+ Rule` draft.
//!
//! P4.3 stores only the on/off state and the target of the pick mode. P4.5
//! adds the kind filter and the viewport filter bar.
//!
//! Lead ruling L-3: one `Pick` button is on at a time. The type holds one
//! `Option<PickMode>`, so a second mode cannot exist. With no mode on, a
//! viewport click edits the default target (`SelectionTarget::Op`, or the
//! boundary of an op that holds no selection).

use std::collections::BTreeSet;

use rs_cam_core::selection::{DiameterRange, Facing, ItemKind, SourceRef, ZRange};
use rs_cam_core::session::SelectionTarget;

use super::toolpath::ToolpathId;

/// The pick mode of one selection of one toolpath.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PickMode {
    pub toolpath_id: ToolpathId,
    pub target: SelectionTarget,
}

/// The `+ Rule` popover draft. `Some` on [`SelectionUi::rule_draft`] means
/// that the popover is open. A draft with content lives in `state/`, not
/// in egui memory (UI-09).
#[derive(Debug, Clone, PartialEq)]
pub struct RuleDraft {
    pub toolpath_id: ToolpathId,
    pub target: SelectionTarget,
    /// The items the rule reads: the toolpath model, another model or the
    /// stock.
    pub source: SourceRef,
    /// The layers the rule admits. Empty = every layer.
    pub layers: BTreeSet<String>,
    /// The kinds the rule admits.
    pub kinds: BTreeSet<ItemKind>,
    /// The circle diameter range, when the rule limits circles.
    pub diameter: Option<DiameterRange>,
    /// The face facing, when the rule admits faces.
    pub facing: Facing,
    /// The face plane Z range, when the rule limits it.
    pub z: Option<ZRange>,
}

/// The selection section state of the inspector.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SelectionUi {
    /// The pick mode that is on, if any.
    pub pick: Option<PickMode>,
    /// The `+ Rule` draft, while the popover is open.
    pub rule_draft: Option<RuleDraft>,
}

impl SelectionUi {
    /// True when the `Pick` button of `target` on `toolpath_id` is on.
    pub fn pick_is_on(&self, toolpath_id: ToolpathId, target: SelectionTarget) -> bool {
        self.pick
            == Some(PickMode {
                toolpath_id,
                target,
            })
    }

    /// The target that a viewport click on `toolpath_id` edits, when a
    /// pick mode of that toolpath is on.
    pub fn pick_target(&self, toolpath_id: ToolpathId) -> Option<SelectionTarget> {
        self.pick
            .filter(|mode| mode.toolpath_id == toolpath_id)
            .map(|mode| mode.target)
    }

    /// The open draft of `target` on `toolpath_id`, if any.
    pub fn draft_for(
        &mut self,
        toolpath_id: ToolpathId,
        target: SelectionTarget,
    ) -> Option<&mut RuleDraft> {
        self.rule_draft
            .as_mut()
            .filter(|d| d.toolpath_id == toolpath_id && d.target == target)
    }

    /// Clear the state of every toolpath but `toolpath_id`. The inspector
    /// calls it each frame, so a change of the selected toolpath ends the
    /// pick mode and closes the popover.
    pub fn keep_only(&mut self, toolpath_id: Option<ToolpathId>) {
        if self
            .pick
            .is_some_and(|m| Some(m.toolpath_id) != toolpath_id)
        {
            self.pick = None;
        }
        if self
            .rule_draft
            .as_ref()
            .is_some_and(|d| Some(d.toolpath_id) != toolpath_id)
        {
            self.rule_draft = None;
        }
    }
}
