//! The pick filter bar (geometry selection P4.5).
//!
//! While a `Pick` button of the Selection section is on, this bar floats at
//! the top centre of the viewport, over the 3D view, and takes no width of
//! its own (lead ruling L-4, the overlays invariant). It sets the kinds a
//! click picks and the circle diameter range, and it ends the pick mode.
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
const PICK_HELP: &str = "Click: add or remove the item. Shift-click: add its layer.";

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
        ItemKind::Loop => "Loops",
        ItemKind::Path => "Open",
        ItemKind::Circle => "Circles",
        ItemKind::Face => "Faces",
        ItemKind::Point => "Points",
    }
}

/// Draw the bar when a pick mode of the selected toolpath is on. The bar
/// edits the filter of the mode; `Done` ends the mode.
pub fn draw(ctx: &egui::Context, state: &mut AppState, viewport_rect: egui::Rect) {
    let crate::state::selection::Selection::Toolpath(tp_id) = state.selection else {
        return;
    };
    if state.workspace != crate::state::Workspace::Toolpaths {
        return;
    }
    let Some(mode) = state.gui.selection_ui.pick_mode(tp_id) else {
        return;
    };
    let target = mode.target;
    let Some((_, tc)) = state.session.find_toolpath_config_by_id(tp_id) else {
        return;
    };
    let item_use = target_item_use(tc, target);
    let held = state
        .session
        .models()
        .iter()
        .find(|m| m.id == tc.model_id)
        .map(|m| held_kinds(m.items.iter()))
        .unwrap_or_default();
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
        state.gui.selection_ui.pick = None;
    } else if let Some(mode) = state.gui.selection_ui.pick.as_mut()
        && mode.filter != filter
    {
        mode.filter = filter;
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
            "Pick {} with a click; a shift-click names them in the layer rule.",
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
            "A click picks only the circles in this range, and a shift-click \
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

/// The tooltip of the item `key` under the pointer in pick mode, for the
/// selection `target` of `tc`: the kind, the layer and the size of the item
/// (`ItemRef`, the text of a refusal), and what a click does. The click
/// result comes from `ProjectSession::pick_edit`, the function the click
/// itself sends through, so the tooltip and the click agree.
pub fn hover_text(
    session: &rs_cam_core::session::ProjectSession,
    tc: &rs_cam_core::session::ToolpathConfig,
    target: SelectionTarget,
    key: &rs_cam_core::selection::MatchKey,
) -> String {
    use rs_cam_core::selection::{ItemRef, SelectionEdit};
    let click = match session.pick_edit(tc, target, key.clone()) {
        Ok(SelectionEdit::RemovePick(_)) => "click to remove",
        Ok(_) => "click to add",
        Err(_) => "the selection does not resolve",
    };
    let shift = if key.layer.is_some() {
        "Shift-click: add its layer."
    } else {
        "Shift-click: the item is on no layer."
    };
    format!("{} \u{2014} {click}.\n{shift}", ItemRef::new(key.clone()))
}
