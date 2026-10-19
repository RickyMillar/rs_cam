//! The view state of the selection section (geometry selection P4.3): the
//! pick mode and the `+ Rule` draft.
//!
//! P4.3 stored the on/off state and the target of the pick mode. P4.5 adds
//! the kind filter that the viewport filter bar (`ui/pick_bar.rs`) edits.
//!
//! Lead ruling L-3: one `Pick` button is on at a time. The type holds one
//! `Option<PickMode>`, so a second mode cannot exist. With no mode on, a
//! viewport click edits the default target (`SelectionTarget::Op`, or the
//! boundary of an op that holds no selection).

use std::collections::BTreeSet;

use rs_cam_core::compute::catalog::ItemUse;
use rs_cam_core::selection::{DiameterRange, Facing, Item, ItemKind, Shape, SourceRef, ZRange};
use rs_cam_core::session::{BOUNDARY_ITEM_USE, SelectionTarget, ToolpathConfig};

use super::toolpath::ToolpathId;

/// The circle diameter range that a new circle limit starts at (the
/// `+ Rule` card and the pick bar): the 5 mm shelf-pin hole of the P4.5
/// operator look, plus or minus 0.1 mm. The user edits both values.
pub const DEFAULT_CIRCLE_RANGE: DiameterRange = DiameterRange {
    nominal_mm: 5.0,
    tolerance_mm: 0.1,
};

/// The kinds of the pick bar chips, in bar order (the P4.5 mockup).
pub const PICK_KINDS: [ItemKind; 5] = [
    ItemKind::Loop,
    ItemKind::Path,
    ItemKind::Circle,
    ItemKind::Face,
    ItemKind::Point,
];

/// The kind filter of a pick mode (geometry selection P4.5): the kinds a
/// click picks and a shift-click names in its layer rule, and the circle
/// diameter range.
#[derive(Debug, Clone, PartialEq)]
pub struct PickFilter {
    /// The kinds whose chip is on. A kind that the reader does not serve or
    /// that the model does not hold is never effective
    /// ([`Self::effective_kinds`]).
    pub kinds: BTreeSet<ItemKind>,
    /// With `Some`, a circle is picked only in this diameter range, and the
    /// layer rule of a shift-click limits its circles to it.
    pub circle: Option<DiameterRange>,
}

impl PickFilter {
    /// Every kind that a reader of `item_use` serves, and no circle limit:
    /// the filter of a new pick mode.
    pub fn served(item_use: ItemUse) -> Self {
        Self {
            kinds: PICK_KINDS
                .into_iter()
                .filter(|k| k.serves(item_use))
                .collect(),
            circle: None,
        }
    }

    /// The kinds the click and the shift-click use: the chips that are on,
    /// that `item_use` serves and that `held` (the kinds of the model)
    /// holds, in bar order. A disabled chip never counts, so the GUI cannot
    /// make a `KindNotCut` pick.
    pub fn effective_kinds(&self, item_use: ItemUse, held: &BTreeSet<ItemKind>) -> Vec<ItemKind> {
        PICK_KINDS
            .into_iter()
            .filter(|k| self.kinds.contains(k) && k.serves(item_use) && held.contains(k))
            .collect()
    }

    /// True when a click may pick `item` under this filter, with `kinds`
    /// from [`Self::effective_kinds`]: its kind is on, and a circle is in
    /// the diameter range when the range is set.
    pub fn admits(&self, kinds: &[ItemKind], item: &Item) -> bool {
        if !kinds.contains(&item.kind()) {
            return false;
        }
        match (item.shape(), self.circle) {
            (Shape::Circle { diameter, .. }, Some(range)) => range.contains(*diameter),
            _ => true,
        }
    }
}

/// The pick mode of one selection of one toolpath.
#[derive(Debug, Clone, PartialEq)]
pub struct PickMode {
    pub toolpath_id: ToolpathId,
    pub target: SelectionTarget,
    /// The kind filter of the viewport filter bar (P4.5).
    pub filter: PickFilter,
}

impl PickMode {
    /// A new pick mode for the selection `target` of `toolpath_id`, read by
    /// `item_use`: every served kind on, no circle limit.
    pub fn new(toolpath_id: ToolpathId, target: SelectionTarget, item_use: ItemUse) -> Self {
        Self {
            toolpath_id,
            target,
            filter: PickFilter::served(item_use),
        }
    }
}

/// How the selection `target` of `tc` uses its items: the op's `ItemUse`
/// for the op selection, [`BOUNDARY_ITEM_USE`] for the boundary.
pub fn target_item_use(tc: &ToolpathConfig, target: SelectionTarget) -> ItemUse {
    match target {
        SelectionTarget::Op => tc.operation.capabilities().item_use,
        SelectionTarget::Boundary => BOUNDARY_ITEM_USE,
    }
}

/// The kinds that the items `items` hold.
pub fn held_kinds<'a>(items: impl IntoIterator<Item = &'a Item>) -> BTreeSet<ItemKind> {
    items.into_iter().map(Item::kind).collect()
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
            .as_ref()
            .is_some_and(|m| m.toolpath_id == toolpath_id && m.target == target)
    }

    /// The target that a viewport click on `toolpath_id` edits, when a
    /// pick mode of that toolpath is on.
    pub fn pick_target(&self, toolpath_id: ToolpathId) -> Option<SelectionTarget> {
        self.pick_mode(toolpath_id).map(|mode| mode.target)
    }

    /// The pick mode of `toolpath_id`, if one is on.
    pub fn pick_mode(&self, toolpath_id: ToolpathId) -> Option<&PickMode> {
        self.pick
            .as_ref()
            .filter(|mode| mode.toolpath_id == toolpath_id)
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
            .as_ref()
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
