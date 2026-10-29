//! The pick filter bar (geometry selection P4.5).
//!
//! While a `Pick` button of the Selection section is on, this bar floats at
//! the top centre of the viewport, over the 3D view, and takes no width of
//! its own (lead ruling L-4, the overlays invariant). It sets the kinds a
//! click picks first and the circle diameter range, and it ends the pick
//! mode. P4 (reference audit 2026-10-23): the chips are a view filter. A
//! click on an item of a kind whose chip is off, but that the op can cut,
//! turns the chip on and picks the item. P3: a shift-click adds one item.
//! E5: a right-click on an item opens the rule menu ([`PickMenu`]: its
//! layer, then the "Rule…" offers of the item); Ctrl+A selects every item of
//! the chip kinds ([`select_all_edit`]).
//!
//! [`click_event`] is THE map from a pick-mode click to its event.
//!
//! A chip for a kind that the selection cannot cut is disabled. Its hover
//! gives the reason in the core text (`selection::kind_not_cut_reason`),
//! so the GUI cannot make a `KindNotCut` pick. The bar edits
//! `state::pick::PickMode` only; it sends no edit.

use std::collections::BTreeSet;

use rs_cam_core::compute::catalog::ItemUse;
use rs_cam_core::selection::{ItemKind, kind_not_cut_reason, kind_noun};
use rs_cam_core::session::SelectionTarget;

use crate::state::AppState;
use crate::state::pick::{
    DEFAULT_CIRCLE_RANGE, PICK_KINDS, PickFilter, held_kinds, target_item_use,
};
use crate::ui::components::{Button, UiExt as _, ValueRow};
use crate::ui::tokens;

/// The egui id of the bar `Area`.
const PICK_BAR_AREA_ID: &str = "pick_filter_bar";

/// The help line of the bar.
pub const PICK_HELP: &str = "Click: add or remove the item. Shift-click: add the item. \
     Ctrl-click: remove the item. Right-click: rules from the item. \
     Ctrl+A: select every item of the chip kinds. Drag: box (left to right encloses, \
     right to left crosses).";

/// B6: the notice of a box on a boundary whose source is not a selection.
pub const BOX_NEEDS_SELECTION_BOUNDARY: &str = "Set the boundary source to Selection first.";

/// Paint the selection box from `anchor` to `to` (screen points, B6,
/// design B §3.2): `tokens::BOX_FILL`, an `ACCENT` 1 pt edge, solid for
/// enclose (left to right), dashed for crossing (right to left). It paints
/// in egui; nothing uploads during the drag.
pub fn paint_box(painter: &egui::Painter, anchor: egui::Pos2, to: egui::Pos2) {
    use crate::state::viewport_input::{BoxMode, DragState};
    let rect = egui::Rect::from_two_pos(anchor, to);
    let stroke = egui::Stroke::new(1.0, tokens::ACCENT);
    painter.rect_filled(rect, 0.0, tokens::BOX_FILL);
    match DragState::box_mode(anchor, to) {
        BoxMode::Enclose => {
            painter.rect_stroke(rect, 0.0, stroke, egui::StrokeKind::Inside);
        }
        BoxMode::Crossing => {
            let corners = [
                rect.left_top(),
                rect.right_top(),
                rect.right_bottom(),
                rect.left_bottom(),
                rect.left_top(),
            ];
            painter.extend(egui::Shape::dashed_line(&corners, stroke, 4.0, 3.0));
        }
    }
}

/// The first row of the rule menu: the layer rule of the item (the
/// right-click act before E5).
pub const LAYER_ROW: &str = "Add its layer";

/// The rule menu of a right-click on an item in pick mode (E5): the layer
/// rule first, then one row per "Rule…" offer of the item (core
/// `list_rule_offers`, the GT-3 offers). It holds data only; a chosen row
/// gives its event ([`PickMenu::layer_event`], [`PickMenu::offer_event`]).
#[derive(Debug, Clone, PartialEq)]
pub struct PickMenu {
    pub toolpath_id: crate::state::toolpath::ToolpathId,
    pub target: SelectionTarget,
    pub key: rs_cam_core::selection::MatchKey,
    /// The kind whose chip the layer row turns on (P4), if any.
    pub chip_off: Option<ItemKind>,
    /// The kinds of the layer rule (the chips that are on, with the
    /// chip-off kind).
    pub kinds: Vec<ItemKind>,
    pub circle: Option<rs_cam_core::selection::DiameterRange>,
    /// One row per offer: "{label} ({count})" and the edit (`None` when a
    /// rule of the selection already holds every match).
    pub offers: Vec<(String, Option<rs_cam_core::selection::SelectionEdit>)>,
}

impl PickMenu {
    /// The menu of the item `key` in `mode`. The offers come from the core
    /// query row `list_rule_offers` (the MCP tool reads the same row).
    pub fn new(
        session: &rs_cam_core::session::ProjectSession,
        mode: &crate::state::pick::PickMode,
        item_use: ItemUse,
        held: &BTreeSet<ItemKind>,
        key: rs_cam_core::selection::MatchKey,
        chip_off: Option<ItemKind>,
    ) -> Self {
        use rs_cam_core::session::{Query, QueryAnswer, RuleOffersArgs};
        let mut filter = mode.filter.clone();
        if let Some(kind) = chip_off {
            let _ = filter.kinds.insert(kind);
        }
        let offers = session
            .toolpath_configs()
            .iter()
            .position(|tc| tc.id == mode.toolpath_id)
            .and_then(|index| {
                session
                    .query(Query::RuleOffers(RuleOffersArgs {
                        index,
                        target: mode.target,
                        key: key.clone(),
                    }))
                    .ok()
            })
            .map(|answer| match answer {
                QueryAnswer::RuleOffers(offers) => offers
                    .into_iter()
                    .map(|o| (format!("{} ({})", o.label, o.count), o.edit))
                    .collect(),
                _ => Vec::new(),
            })
            .unwrap_or_default();
        Self {
            toolpath_id: mode.toolpath_id,
            target: mode.target,
            key,
            chip_off,
            kinds: filter.effective_kinds(item_use, held),
            circle: filter.circle,
            offers,
        }
    }

    /// The event of the layer row: `PickLayer`, as the right-click before
    /// E5 sent.
    pub fn layer_event(&self) -> crate::ui::AppEvent {
        crate::ui::AppEvent::PickLayer {
            toolpath_id: self.toolpath_id,
            target: self.target,
            key: self.key.clone(),
            kinds: self.kinds.clone(),
            circle: self.circle,
        }
    }

    /// The event of offer row `row`: one `EditToolpathSelection` (one
    /// command, one undo step), or `None` for a held offer.
    pub fn offer_event(&self, row: usize) -> Option<crate::ui::AppEvent> {
        let edit = self.offers.get(row)?.1.clone()?;
        Some(crate::ui::AppEvent::EditToolpathSelection {
            toolpath_id: self.toolpath_id,
            target: self.target,
            edit,
        })
    }
}

/// What the rule menu did this frame.
#[derive(Debug)]
pub enum PickMenuOutcome {
    /// The menu is open; nothing is chosen.
    Open,
    /// The menu closed with no choice.
    Closed,
    /// The layer row (the caller turns the chip-off kind on first).
    Layer,
    /// An offer row was chosen.
    Offer(usize),
}

/// Show `menu` as the context menu of the viewport `response`. egui opens
/// it on the right-click of the frame that built the menu.
pub fn show_pick_menu(response: &egui::Response, menu: &PickMenu) -> PickMenuOutcome {
    let mut chose = None;
    let shown = response.context_menu(|ui| {
        if ui.button(LAYER_ROW).clicked() {
            chose = Some(PickMenuOutcome::Layer);
            ui.close();
        }
        if !menu.offers.is_empty() {
            ui.separator();
        }
        for (row, (label, edit)) in menu.offers.iter().enumerate() {
            let clicked = ui
                .add_enabled(edit.is_some(), egui::Button::new(label.as_str()))
                .on_disabled_hover_text("A rule of the selection already holds every item")
                .clicked();
            if clicked {
                chose = Some(PickMenuOutcome::Offer(row));
                ui.close();
            }
        }
    });
    match (chose, shown) {
        (Some(outcome), _) => outcome,
        (None, Some(_)) => PickMenuOutcome::Open,
        (None, None) if response.secondary_clicked() => PickMenuOutcome::Open,
        (None, None) => PickMenuOutcome::Closed,
    }
}

/// The edit of Ctrl+A in the pick mode that is on (E5, SEL-13): the rules
/// of the chip kinds that the selection serves and the model holds, through
/// core `kind_rule_edit`; under the default the edit replaces the default
/// (operator ruling 2026-10-29). `Ok(None)`: no pick mode, or no edit.
/// `Err`: no chip is on, or the core refusal text.
pub fn select_all_edit(
    state: &AppState,
) -> Result<
    Option<(
        crate::state::toolpath::ToolpathId,
        SelectionTarget,
        rs_cam_core::selection::SelectionEdit,
    )>,
    String,
> {
    let Some(crate::state::pick::ViewportMode::Pick(mode)) = state.gui.selection_ui.mode.as_ref()
    else {
        return Ok(None);
    };
    let session = state.session();
    let Some((_, tc)) = session.find_toolpath_config_by_id(mode.toolpath_id) else {
        return Ok(None);
    };
    let held = session
        .models()
        .iter()
        .find(|m| m.id == tc.model_id)
        .map(held_kinds)
        .unwrap_or_default();
    let kinds = mode
        .filter
        .effective_kinds(target_item_use(tc, mode.target), &held);
    if kinds.is_empty() {
        return Err("No chip is on, so Ctrl+A names no item".to_owned());
    }
    session
        .kind_rule_edit(tc, mode.target, &kinds, mode.filter.circle)
        .map(|edit| edit.map(|edit| (mode.toolpath_id, mode.target, edit)))
        .map_err(|e| e.to_string())
}

/// The button of a pick-mode click (P3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickClick {
    /// A primary click with no modifier: add or remove the item.
    Plain,
    /// A primary click with Shift: add the item (CAD convention).
    Shift,
    /// A primary click with Ctrl (Cmd on macOS): remove the item (B4).
    Ctrl,
    /// A secondary click: add the layer of the item as a rule.
    Secondary,
}

impl PickClick {
    /// The click of a primary press with `modifiers`: THE translation of
    /// the modifiers of a pick click. Ctrl wins over Shift.
    pub fn primary(modifiers: egui::Modifiers) -> Self {
        if modifiers.command {
            Self::Ctrl
        } else if modifiers.shift {
            Self::Shift
        } else {
            Self::Plain
        }
    }
}

/// THE event of a pick-mode click on the item `key` (P3, P4). When
/// `chip_off` names a kind (`PickModeHit::chip_off`), the click turns that
/// chip of `mode` on first, so the item it picks shows. Then:
///
/// - [`PickClick::Plain`]: `TogglePick`; on an item whose chip was off,
///   `AddPick` (the click picks the item and never removes it).
/// - [`PickClick::Shift`]: `AddPick`, one item.
/// - [`PickClick::Ctrl`]: `RemovePick`, one item; on an item whose chip
///   was off, no event (that item is not selected).
/// - [`PickClick::Secondary`]: `PickLayer` with the kinds of the chips
///   that are on and the circle range.
pub fn click_event(
    mode: &mut crate::state::pick::PickMode,
    item_use: ItemUse,
    held: &BTreeSet<ItemKind>,
    key: rs_cam_core::selection::MatchKey,
    chip_off: Option<ItemKind>,
    click: PickClick,
) -> Option<crate::ui::AppEvent> {
    use crate::ui::AppEvent;
    if let Some(kind) = chip_off {
        let _ = mode.filter.kinds.insert(kind);
    }
    let (toolpath_id, target) = (mode.toolpath_id, mode.target);
    match click {
        PickClick::Plain if chip_off.is_none() => Some(AppEvent::TogglePick {
            toolpath_id,
            target,
            key,
        }),
        PickClick::Plain | PickClick::Shift => Some(AppEvent::AddPick {
            toolpath_id,
            target,
            key,
        }),
        PickClick::Ctrl if chip_off.is_none() => Some(AppEvent::RemovePick {
            toolpath_id,
            target,
            key,
        }),
        PickClick::Ctrl => None,
        PickClick::Secondary => Some(AppEvent::PickLayer {
            toolpath_id,
            target,
            key,
            kinds: mode.filter.effective_kinds(item_use, held),
            circle: mode.filter.circle,
        }),
    }
}

/// One kind chip of the bar.
#[derive(Debug, Clone, PartialEq)]
pub struct KindChip {
    pub kind: ItemKind,
    pub label: &'static str,
    /// True when the chip is enabled and on.
    pub on: bool,
    /// Why the chip is disabled, or `None` when it is enabled. A kind the
    /// selection cannot cut gives the core reason
    /// ([`kind_not_cut_reason`]); a kind the model does not hold says so.
    pub disabled_reason: Option<String>,
}

/// The chips of the bar, in bar order, for `filter` on a selection of
/// `item_use` over a model that holds `held`.
pub fn kind_chips(
    filter: &PickFilter,
    item_use: ItemUse,
    held: &BTreeSet<ItemKind>,
) -> Vec<KindChip> {
    PICK_KINDS
        .into_iter()
        .map(|kind| {
            let disabled_reason = if !kind.serves(item_use) {
                Some(kind_not_cut_reason(kind, item_use))
            } else if !held.contains(&kind) {
                Some(format!("The model holds no {}.", kind_noun(kind, 2)))
            } else {
                None
            };
            KindChip {
                kind,
                label: chip_label(kind),
                on: disabled_reason.is_none() && filter.kinds.contains(&kind),
                disabled_reason,
            }
        })
        .collect()
}

/// The text of the chip of `kind`.
fn chip_label(kind: ItemKind) -> &'static str {
    match kind {
        ItemKind::Loop => "Closed contours",
        ItemKind::Path => "Open contours",
        ItemKind::Circle => "Circles",
        ItemKind::Face => "Faces",
        ItemKind::Point => "Points",
    }
}

/// Draw the bar when a pick mode of the selected toolpath is on. The bar
/// edits the filter of the mode; `Done` ends the mode.
pub fn draw(ctx: &egui::Context, state: &mut AppState, viewport_rect: egui::Rect) {
    let crate::state::selection::Selection::Toolpath(tp_id) = *state.focus() else {
        return;
    };
    if state.workspace != crate::state::Workspace::Toolpaths {
        return;
    }
    if state.gui.selection_ui.tab_edit(tp_id).is_some() {
        draw_tab_bar(ctx, state, tp_id, viewport_rect);
        return;
    }
    let Some(mode) = state.gui.selection_ui.pick_mode(tp_id) else {
        return;
    };
    let target = mode.target;
    let Some((_, tc)) = state.session().find_toolpath_config_by_id(tp_id) else {
        return;
    };
    let item_use = target_item_use(tc, target);
    let held = state
        .session()
        .models()
        .iter()
        .find(|m| m.id == tc.model_id)
        .map(held_kinds)
        .unwrap_or_default();
    // P4.7: the flat faces of an STL are not computed yet (the job fills
    // the cache). A face click then says so; the bar says it first.
    let faces_pending = state
        .session()
        .models()
        .iter()
        .find(|m| m.id == tc.model_id)
        .is_some_and(|m| !m.faces_ready());
    let title = format!(
        "Pick for {} \u{25B8} {}",
        tc.name,
        match target {
            SelectionTarget::Op => "Selection",
            SelectionTarget::Boundary => "Machining boundary",
        }
    );
    let mut filter = mode.filter.clone();
    let mut done = false;

    egui::Area::new(egui::Id::new(PICK_BAR_AREA_ID))
        .order(egui::Order::Middle)
        .pivot(egui::Align2::CENTER_TOP)
        .fixed_pos(egui::pos2(
            viewport_rect.center().x,
            viewport_rect.min.y + tokens::SPACE_3,
        ))
        .constrain_to(viewport_rect)
        .show(ctx, |ui| {
            egui::Frame::default()
                .fill(tokens::SURFACE_OVERLAY)
                .stroke(egui::Stroke::new(1.0, tokens::HAIRLINE))
                .corner_radius(tokens::RADIUS_MD)
                .shadow(tokens::SHADOW_OVERLAY)
                .inner_margin(egui::Margin::same(tokens::SPACE_3 as i8))
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = tokens::SPACE_2;
                    ui.label(egui::RichText::new(title).color(tokens::TEXT_STRONG));
                    ui.horizontal_wrapped(|ui| {
                        for chip in kind_chips(&filter, item_use, &held) {
                            draw_chip(ui, &chip, &mut filter);
                        }
                    });
                    let on = filter.effective_kinds(item_use, &held);
                    draw_circle_range(ui, &on, &mut filter);
                    if faces_pending && on.contains(&ItemKind::Face) {
                        ui.label(egui::RichText::new(COMPUTING_FACES).color(tokens::TEXT_MUTED));
                    }
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(PICK_HELP).color(tokens::TEXT_MUTED));
                        if ui
                            .add(Button::new("Done"))
                            .on_hover_text("End the pick mode (Esc).")
                            .clicked()
                        {
                            done = true;
                        }
                    });
                });
        });

    if done {
        state.gui.selection_ui.set_pick(None);
    } else if let Some(mode) = state.gui.selection_ui.pick_mut()
        && mode.filter != filter
    {
        mode.filter = filter;
    }
}

/// The line that the pick bar and the Selection section show while the
/// flat faces of an STL are not computed (geometry selection P4.7).
pub(crate) const COMPUTING_FACES: &str = "Computing faces\u{2026}";

/// The help line of the tab editor bar (geometry selection P4.6).
const TAB_HELP: &str =
    "Click a loop: add a tab. Drag a tab: move it. Right-click a tab: delete it.";

/// The bar of the tab editor of a Profile (geometry selection P4.6), in
/// the place of the pick bar (L-4): the one-line summary of the tabs, the
/// help line and `Done`. A refused layout shows the refusal in `DANGER`.
fn draw_tab_bar(
    ctx: &egui::Context,
    state: &mut AppState,
    tp_id: crate::state::toolpath::ToolpathId,
    viewport_rect: egui::Rect,
) {
    let Some(mode) = state.gui.selection_ui.tab_edit(tp_id) else {
        return;
    };
    let name = state
        .session()
        .find_toolpath_config_by_id(tp_id)
        .map(|(_, tc)| tc.name.clone())
        .unwrap_or_default();
    let line = match mode.layout.as_ref().map(|(_, r)| r) {
        Some(Ok(answer)) => Ok(crate::state::tab_edit::tab_summary(answer)),
        Some(Err(refusal)) => Err(refusal.clone()),
        None => Ok(String::new()),
    };
    let mut done = false;
    egui::Area::new(egui::Id::new(PICK_BAR_AREA_ID))
        .order(egui::Order::Middle)
        .pivot(egui::Align2::CENTER_TOP)
        .fixed_pos(egui::pos2(
            viewport_rect.center().x,
            viewport_rect.min.y + tokens::SPACE_3,
        ))
        .constrain_to(viewport_rect)
        .show(ctx, |ui| {
            egui::Frame::default()
                .fill(tokens::SURFACE_OVERLAY)
                .stroke(egui::Stroke::new(1.0, tokens::HAIRLINE))
                .corner_radius(tokens::RADIUS_MD)
                .shadow(tokens::SHADOW_OVERLAY)
                .inner_margin(egui::Margin::same(tokens::SPACE_3 as i8))
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = tokens::SPACE_2;
                    ui.label(
                        egui::RichText::new(format!("Edit tabs of {name}"))
                            .color(tokens::TEXT_STRONG),
                    );
                    ui.horizontal(|ui| {
                        match &line {
                            Ok(summary) => {
                                ui.label(summary.as_str());
                            }
                            Err(refusal) => {
                                ui.label(
                                    egui::RichText::new(refusal.as_str()).color(tokens::DANGER),
                                );
                            }
                        }
                        if ui
                            .add(Button::new("Done"))
                            .on_hover_text("End the tab editor (Esc).")
                            .clicked()
                        {
                            done = true;
                        }
                    });
                    ui.label(egui::RichText::new(TAB_HELP).color(tokens::TEXT_MUTED));
                });
        });
    if done {
        state.gui.selection_ui.set_tab_edit(tp_id, false);
    }
}

/// One chip: `ACCENT` when on (the primary button), the default fill when
/// off. A disabled chip names its reason on hover.
fn draw_chip(ui: &mut egui::Ui, chip: &KindChip, filter: &mut PickFilter) {
    let button = if chip.on {
        Button::primary(chip.label)
    } else {
        Button::new(chip.label)
    };
    let enabled = chip.disabled_reason.is_none();
    let response = ui.add(button.enabled(enabled));
    let response = match &chip.disabled_reason {
        Some(reason) => response.on_disabled_hover_text(reason.as_str()),
        None => response.on_hover_text(format!(
            "On: a click picks {} first. Off: a click on one turns this chip on.",
            kind_noun(chip.kind, 2)
        )),
    };
    if enabled && response.clicked() {
        if chip.on {
            filter.kinds.remove(&chip.kind);
        } else {
            filter.kinds.insert(chip.kind);
        }
    }
}

/// The circle diameter range, while the circle chip is on.
fn draw_circle_range(ui: &mut egui::Ui, on: &[ItemKind], filter: &mut PickFilter) {
    if !on.contains(&ItemKind::Circle) {
        return;
    }
    let mut limit = filter.circle.is_some();
    if ui
        .checkbox(&mut limit, "Limit the circle diameter")
        .on_hover_text(
            "A click picks only the circles in this range, and a right-click \
             adds a rule for them.",
        )
        .changed()
    {
        filter.circle = limit.then_some(DEFAULT_CIRCLE_RANGE);
    }
    if let Some(range) = filter.circle.as_mut() {
        ui.param_grid("pick_bar_circle", |ui| {
            ValueRow::new(
                "Circle \u{00D8}:",
                &mut range.nominal_mm,
                " mm",
                0.05,
                0.0..=10_000.0,
            )
            .show(ui);
            ValueRow::new(
                "Tolerance \u{00B1}:",
                &mut range.tolerance_mm,
                " mm",
                0.01,
                0.0..=100.0,
            )
            .show(ui);
        });
    }
}

/// The one-line tooltip of the item `key` under the pointer in pick mode
/// (live look 2026-10-29): the kind and the measure of the item, then
/// [`HOVER_ACTS`]. When the selection `target` of `tc` does not resolve,
/// the line is the refusal (`ProjectSession::click_edit`, the function the
/// click sends through). The bar keeps the long help ([`PICK_HELP`]).
pub fn hover_text(
    session: &rs_cam_core::session::ProjectSession,
    tc: &rs_cam_core::session::ToolpathConfig,
    target: SelectionTarget,
    key: &rs_cam_core::selection::MatchKey,
) -> String {
    if let Err(refusal) = session.click_edit(tc, target, key.clone()) {
        return refusal.to_string();
    }
    let m = key.measure;
    let item = match key.kind {
        ItemKind::Loop => format!("Closed contour, {m:.1} mm across"),
        ItemKind::Path => format!("Open contour, {m:.1} mm long"),
        ItemKind::Circle => format!("Circle, \u{2300} {m:.1} mm"),
        ItemKind::Face => format!("Face, {m:.1} mm across"),
        ItemKind::Point => "Point".to_owned(),
    };
    format!("{item} \u{00B7} {HOVER_ACTS}")
}

/// The acts of the pick-mode tooltip, after the item.
pub const HOVER_ACTS: &str = "click picks \u{00B7} Ctrl removes \u{00B7} right-click rules";
