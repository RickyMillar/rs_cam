//! The view state of the selection section (geometry selection P4.3): the
//! pick mode, and the Geometry tab state (GT-2, GT-3): the open "Rule…"
//! menu, the hovered row and the list filter.
//!
//! P4.3 stored the on/off state and the target of the pick mode. P4.5 adds
//! the kind filter that the viewport filter bar (`ui/pick_bar.rs`) edits.
//!
//! Lead ruling L-3: one `Pick` button is on at a time. The type holds one
//! `Option<ViewportMode>`, so a second mode cannot exist. With no mode on, a
//! viewport click edits the default target (`SelectionTarget::Op`, or the
//! boundary of an op that holds no selection).
//!
//! P4.6 adds the second viewport mode, the hand-tab editor of a Profile
//! ([`TabEditMode`]). It is a variant of the same `Option`, so a pick mode
//! and the tab editor cannot both be on.

use std::collections::BTreeSet;

use rs_cam_core::compute::catalog::ItemUse;
use rs_cam_core::selection::{DiameterRange, Item, ItemKind, MatchKey, Shape};
use rs_cam_core::session::{BOUNDARY_ITEM_USE, SelectionTarget, ToolpathConfig};

use super::toolpath::ToolpathId;

/// The circle diameter range that a new circle limit starts at (the pick
/// bar): the 5 mm shelf-pin hole of the P4.5
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
/// click picks first and a right-click names in its layer rule, and the
/// circle diameter range. P4 (reference audit 2026-10-23): the chips are a
/// view filter. A click on an item that the op can cut, of a kind whose
/// chip is off, turns that chip on and picks the item
/// ([`Self::cut_kinds`]).
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

    /// The kinds the click tries first and the right-click layer rule
    /// uses: the chips that are on, that `item_use` serves and that `held`
    /// (the kinds of the model) holds, in bar order. A disabled chip never
    /// counts, so the GUI cannot make a `KindNotCut` pick.
    pub fn effective_kinds(&self, item_use: ItemUse, held: &BTreeSet<ItemKind>) -> Vec<ItemKind> {
        PICK_KINDS
            .into_iter()
            .filter(|k| self.kinds.contains(k) && k.serves(item_use) && held.contains(k))
            .collect()
    }

    /// The kinds that the op can cut on this model, whatever the chip
    /// state (P4): the kinds that `item_use` serves and that `held` holds,
    /// in bar order. A click that hits no item of an on chip tries these.
    pub fn cut_kinds(item_use: ItemUse, held: &BTreeSet<ItemKind>) -> Vec<ItemKind> {
        PICK_KINDS
            .into_iter()
            .filter(|k| k.serves(item_use) && held.contains(k))
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

/// The kinds that `model` holds: the kinds of its items, and `Face` for a
/// mesh whose faces are its flat regions (an STL, geometry selection P4.7).
/// The flat regions need no build to count: the click builds nothing and
/// says "computing faces" until the job fills the cache.
pub fn held_kinds(model: &rs_cam_core::session::LoadedModel) -> BTreeSet<ItemKind> {
    let mut held: BTreeSet<ItemKind> = model.items.iter().map(Item::kind).collect();
    if model.faces_are_flat_regions() {
        held.insert(ItemKind::Face);
    }
    held
}

/// The "Rule…" offers of one row of the Geometry tab (geometry selection
/// GT-3, L-R4). `Some` on [`SelectionUi::offers_for`] means that the menu
/// of that row is open; the inspector reads the offers of core
/// (`ProjectSession::rule_offers`) only while it is open.
#[derive(Debug, Clone, PartialEq)]
pub struct OffersFor {
    pub toolpath_id: ToolpathId,
    pub target: SelectionTarget,
    /// The key of the picked item that the offers read.
    pub key: MatchKey,
}

/// A drag of one tab in the tab editor (geometry selection P4.6). It is GUI
/// state only: the release sends ONE `SetToolpathParam`.
#[derive(Debug, Clone, PartialEq)]
pub struct TabDrag {
    /// The index of the dragged tab in `TabLayoutAnswer::marks`.
    pub mark: usize,
    /// The point on the loop under the pointer (drawing frame), snapped on
    /// each frame. `None` until the pointer first moves onto the loop.
    pub to: Option<[f64; 2]>,
}

/// The hand-tab editor of one Profile (geometry selection P4.6).
#[derive(Debug, Clone, PartialEq)]
pub struct TabEditMode {
    pub toolpath_id: ToolpathId,
    /// The drag in progress, if any.
    pub drag: Option<TabDrag>,
    /// The tab layout the editor shows, and the input revision of the
    /// toolpath it was read at (`ProjectSession::toolpath_revision_at`). The
    /// frame reads the query again only when the revision moves
    /// ([`super::tab_edit::refresh_layout`]).
    pub layout: Option<(u64, Result<rs_cam_core::session::TabLayoutAnswer, String>)>,
}

impl TabEditMode {
    /// A new editor of `toolpath_id`, with no layout read yet.
    pub fn new(toolpath_id: ToolpathId) -> Self {
        Self {
            toolpath_id,
            drag: None,
            layout: None,
        }
    }

    /// The layout, when the query gave one.
    pub fn answer(&self) -> Option<&rs_cam_core::session::TabLayoutAnswer> {
        self.layout.as_ref().and_then(|(_, r)| r.as_ref().ok())
    }
}

/// The one viewport mode that is on (lead ruling L-3).
#[derive(Debug, Clone, PartialEq)]
pub enum ViewportMode {
    /// A `Pick` button of the Selection section (P4.3, P4.5).
    Pick(PickMode),
    /// The hand-tab editor of a Profile (P4.6).
    Tabs(TabEditMode),
}

impl ViewportMode {
    fn toolpath_id(&self) -> ToolpathId {
        match self {
            Self::Pick(mode) => mode.toolpath_id,
            Self::Tabs(mode) => mode.toolpath_id,
        }
    }
}

/// The selection section state of the inspector.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SelectionUi {
    /// The viewport mode that is on, if any.
    pub mode: Option<ViewportMode>,
    /// The "Rule…" menu of one row of the Geometry tab, while it is open.
    pub offers_for: Option<OffersFor>,
    /// The key of the selected-list row under the pointer (GT-2): the
    /// viewport hover buffer draws it, as it draws a pick hover.
    pub row_hover: Option<MatchKey>,
    /// The text of the filter box of the Geometry tab (GT-2): it filters
    /// the rows of the list; it does not search the model.
    pub list_filter: String,
    /// The pointer gesture in the viewport (B2). A change or the end of the
    /// mode sets it to `Idle`, so a gesture never outlives its mode.
    pub drag: super::viewport_input::DragState,
    /// E-Q6 "select other": the candidates under the pointer and the one
    /// in force, while Tab cycles them. A change of the mode or a pointer
    /// move ends it ([`keep_cycle`]).
    pub cycle: Option<PickCycle>,
    /// E-Q6: a Tab asked for the next candidate
    /// (`UiCommand::CyclePickCandidate`); the viewport frame takes it, as
    /// only the frame knows the pointer and the camera ([`step_cycle`]).
    pub cycle_request: bool,
}

/// How far the pointer may move, in points, before the "select other"
/// cycle ends and the hover picks again. A design choice, not a
/// measurement: the hand shake of a pointer at rest, well under the
/// 15 px pick threshold.
pub const CYCLE_STAY_PX: f32 = 3.0;

/// E-Q6 "select other": the candidates under one pointer position, in
/// order (the drawn items nearest first, then the faces along the ray,
/// nearest first), and the index of the one in force.
#[derive(Debug, Clone, PartialEq)]
pub struct PickCycle {
    /// The pointer position the list was made at (viewport points).
    pub at: [f32; 2],
    pub candidates: Vec<MatchKey>,
    pub index: usize,
}

impl PickCycle {
    /// The candidate in force.
    pub fn current(&self) -> Option<&MatchKey> {
        self.candidates.get(self.index)
    }

    /// The 1-based position and the count, for the one-line note.
    pub fn position(&self) -> (usize, usize) {
        (self.index + 1, self.candidates.len())
    }

    /// True when `pointer` is within [`CYCLE_STAY_PX`] of the list point.
    pub fn holds(&self, pointer: [f32; 2]) -> bool {
        let dx = pointer[0] - self.at[0];
        let dy = pointer[1] - self.at[1];
        dx.hypot(dy) <= CYCLE_STAY_PX
    }
}

/// THE cycle step of a Tab at `pointer` (pure). A cycle at the same point
/// moves to the next candidate and wraps. Else `candidates` makes the list
/// at the pointer and the cycle starts on the SECOND candidate: the first
/// is the one the hover already shows. No candidate gives `None`.
pub fn step_cycle(
    cycle: Option<PickCycle>,
    pointer: [f32; 2],
    candidates: impl FnOnce() -> Vec<MatchKey>,
) -> Option<PickCycle> {
    match cycle {
        Some(cycle) if cycle.holds(pointer) && !cycle.candidates.is_empty() => {
            let index = (cycle.index + 1) % cycle.candidates.len();
            Some(PickCycle { index, ..cycle })
        }
        _ => {
            let candidates = candidates();
            if candidates.is_empty() {
                return None;
            }
            let index = 1 % candidates.len();
            Some(PickCycle {
                at: pointer,
                candidates,
                index,
            })
        }
    }
}

/// The cycle of the next frame (pure): it ends when the pointer left the
/// list point or is not over the viewport.
pub fn keep_cycle(cycle: Option<PickCycle>, pointer: Option<[f32; 2]>) -> Option<PickCycle> {
    cycle.filter(|c| pointer.is_some_and(|p| c.holds(p)))
}

impl SelectionUi {
    /// True when the `Pick` button of `target` on `toolpath_id` is on.
    pub fn pick_is_on(&self, toolpath_id: ToolpathId, target: SelectionTarget) -> bool {
        self.pick_mode(toolpath_id)
            .is_some_and(|m| m.target == target)
    }

    /// B6: a first Escape during a box cancels the box and keeps the mode
    /// (`DragState::Cancelled`: the rest of the drag sends nothing). True
    /// when it cancelled a box; false otherwise (the caller then ends the
    /// mode).
    pub fn cancel_box(&mut self) -> bool {
        use super::viewport_input::DragState;
        if matches!(self.drag, DragState::BoxSelect { .. }) {
            self.drag = DragState::Cancelled;
            true
        } else {
            false
        }
    }

    /// End the viewport mode that is on (the pick mode or the tab editor;
    /// F4, Escape). The gesture resets with it (L-3).
    pub fn end_mode(&mut self) {
        self.set_mode(None);
    }

    /// Turn the pick mode of `target` on `toolpath_id` on (with `mode`) or
    /// off. Any other mode ends (L-3).
    pub fn set_pick(&mut self, mode: Option<PickMode>) {
        self.set_mode(mode.map(ViewportMode::Pick));
    }

    /// Write the mode. A change of the mode ends the gesture (B2).
    fn set_mode(&mut self, mode: Option<ViewportMode>) {
        if self.mode != mode {
            self.drag = super::viewport_input::DragState::Idle;
            self.cycle = None;
            self.cycle_request = false;
        }
        self.mode = mode;
    }

    /// The pick mode that is on, of any toolpath.
    pub fn pick_mut(&mut self) -> Option<&mut PickMode> {
        match self.mode.as_mut() {
            Some(ViewportMode::Pick(mode)) => Some(mode),
            Some(ViewportMode::Tabs(_)) | None => None,
        }
    }

    /// The tab editor of `toolpath_id`, if it is on.
    pub fn tab_edit(&self, toolpath_id: ToolpathId) -> Option<&TabEditMode> {
        match self.mode.as_ref() {
            Some(ViewportMode::Tabs(mode)) if mode.toolpath_id == toolpath_id => Some(mode),
            Some(ViewportMode::Tabs(_) | ViewportMode::Pick(_)) | None => None,
        }
    }

    /// The tab editor of `toolpath_id`, if it is on, to edit.
    pub fn tab_edit_mut(&mut self, toolpath_id: ToolpathId) -> Option<&mut TabEditMode> {
        match self.mode.as_mut() {
            Some(ViewportMode::Tabs(mode)) if mode.toolpath_id == toolpath_id => Some(mode),
            Some(ViewportMode::Tabs(_) | ViewportMode::Pick(_)) | None => None,
        }
    }

    /// Turn the tab editor of `toolpath_id` on or off. On ends any other
    /// mode (L-3).
    pub fn set_tab_edit(&mut self, toolpath_id: ToolpathId, on: bool) {
        if on {
            self.set_mode(Some(ViewportMode::Tabs(TabEditMode::new(toolpath_id))));
        } else if self.tab_edit(toolpath_id).is_some() {
            self.set_mode(None);
        }
    }

    /// The target that a viewport click on `toolpath_id` edits, when a
    /// pick mode of that toolpath is on.
    pub fn pick_target(&self, toolpath_id: ToolpathId) -> Option<SelectionTarget> {
        self.pick_mode(toolpath_id).map(|mode| mode.target)
    }

    /// The pick mode of `toolpath_id`, if one is on.
    pub fn pick_mode(&self, toolpath_id: ToolpathId) -> Option<&PickMode> {
        match self.mode.as_ref() {
            Some(ViewportMode::Pick(mode)) if mode.toolpath_id == toolpath_id => Some(mode),
            Some(ViewportMode::Pick(_) | ViewportMode::Tabs(_)) | None => None,
        }
    }

    /// The open "Rule…" menu of `target` on `toolpath_id`, if any.
    pub fn offers_for(
        &self,
        toolpath_id: ToolpathId,
        target: SelectionTarget,
    ) -> Option<&OffersFor> {
        self.offers_for
            .as_ref()
            .filter(|o| o.toolpath_id == toolpath_id && o.target == target)
    }

    /// Clear the state of every toolpath but `toolpath_id`. The inspector
    /// calls it each frame, so a change of the selected toolpath ends the
    /// pick mode or the tab editor and closes the popover.
    pub fn keep_only(&mut self, toolpath_id: Option<ToolpathId>) {
        if self
            .mode
            .as_ref()
            .is_some_and(|m| Some(m.toolpath_id()) != toolpath_id)
        {
            self.set_mode(None);
        }
        if self
            .offers_for
            .as_ref()
            .is_some_and(|o| Some(o.toolpath_id) != toolpath_id)
        {
            self.offers_for = None;
        }
    }
}
