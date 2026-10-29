//! The context rail: one chip with an eye for each special-case overlay
//! that can draw now (R6, operator ruling Q5, 2026-10-30).
//!
//! The operator: "when special cases like this are available they should
//! come up as more prominent options with an eye visibility toggle next to
//! them. maybe next to the legend."
//!
//! - A context row is a registry row with `context: true`. The rail lists
//!   the context rows whose precondition holds NOW
//!   (`registry::context_rows_ready`). When the precondition stops holding,
//!   the chip goes and the flag keeps its value.
//! - A click toggles the row through `registry::toggle_overlay`, the setter
//!   that `More` and the catalogue also reach through `set_overlay`. So a
//!   rail toggle and a `More` toggle give one state. A radio member (the
//!   stock Deviation colour) goes off to the plain choice of its surface; a
//!   model colour goes off to the plain model.
//! - The rail stands on the legend: its bottom-right corner is above the
//!   legend rect of this frame, or at the legend anchor when no legend
//!   draws. It so keeps the legend's clearance: it never covers the dock or
//!   the view cube corner.
//! - The rail stores nothing. It starts no compute.

use crate::state::AppState;
use crate::ui::setup_panel::{MODEL_EYE_HIDDEN, MODEL_EYE_SHOWN};
use crate::ui::tokens;

use super::legend_rail::{GIZMO_CLEARANCE, LegendPlacement};
use super::registry::{self, OverlayRow};

/// The egui id of the context rail area.
pub const CONTEXT_AREA_ID: &str = "viewport_context_rail";

/// The chips of the rail now, in list order.
#[must_use]
pub fn chips(state: &AppState) -> Vec<&'static OverlayRow> {
    registry::context_rows_ready(state)
}

/// The text of the chip of `row`: the eye glyph, then the row label.
#[must_use]
pub fn chip_text(state: &AppState, row: &OverlayRow) -> String {
    let eye = if (row.get)(state) {
        MODEL_EYE_SHOWN
    } else {
        MODEL_EYE_HIDDEN
    };
    format!("{eye} {}", row.label)
}

/// A click on the chip of `row`: the one toggle door.
pub fn toggle(state: &mut AppState, row: &OverlayRow) {
    registry::toggle_overlay(state, row);
}

/// Where the rail goes: its bottom-right corner, its widest width and its
/// tallest height. `None` when no row of chips fits under the view cube
/// corner.
#[must_use]
pub fn placement(
    viewport: egui::Rect,
    dock: egui::Rect,
    legend: Option<egui::Rect>,
) -> Option<(egui::Pos2, f32, f32)> {
    let place = LegendPlacement::new(viewport, dock);
    let anchor = match legend {
        Some(rect) => egui::pos2(place.anchor.x, rect.top() - tokens::SPACE_2),
        None => place.anchor,
    };
    let room = anchor.y - viewport.top() - GIZMO_CLEARANCE;
    (room >= tokens::ROW_DENSE).then_some((anchor, place.max_width, room))
}

/// Draw the rail when at least one context row can draw now. Returns the
/// rail rect when it drew.
pub fn draw(
    ctx: &egui::Context,
    state: &mut AppState,
    viewport: egui::Rect,
    dock: egui::Rect,
    legend: Option<egui::Rect>,
) -> Option<egui::Rect> {
    let rows = chips(state);
    if rows.is_empty() {
        return None;
    }
    let (anchor, max_width, max_height) = placement(viewport, dock, legend)?;
    let mut clicked: Option<&'static OverlayRow> = None;
    let shown: &AppState = state;
    let area = egui::Area::new(egui::Id::new(CONTEXT_AREA_ID))
        .order(egui::Order::Middle)
        .pivot(egui::Align2::RIGHT_BOTTOM)
        .fixed_pos(anchor)
        .constrain_to(viewport)
        .show(ctx, |ui| {
            ui.set_max_width(max_width);
            ui.set_max_height(max_height);
            egui::Frame::default()
                .fill(tokens::SURFACE_OVERLAY)
                .stroke(egui::Stroke::new(1.0, tokens::HAIRLINE))
                .corner_radius(tokens::RADIUS_MD)
                .inner_margin(egui::Margin::same(tokens::SPACE_2 as i8))
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = tokens::SPACE_1;
                    ui.with_layout(egui::Layout::top_down(egui::Align::Max), |ui| {
                        for row in &rows {
                            let on = (row.get)(shown);
                            let response = ui
                                .add(egui::Button::selectable(
                                    on,
                                    egui::RichText::new(chip_text(shown, row))
                                        .size(tokens::SIZE_CAPTION),
                                ))
                                .on_hover_text(row.hover);
                            if response.clicked() {
                                clicked = Some(row);
                            }
                        }
                    });
                });
        });
    if let Some(row) = clicked {
        toggle(state, row);
    }
    Some(area.response.rect)
}
