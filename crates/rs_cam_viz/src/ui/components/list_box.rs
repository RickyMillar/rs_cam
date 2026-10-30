//! `ListBox` — a list of items under the field rows (W3g).
//!
//! The operator, 2026-10-30: the selected cuts list "looks like it's part
//! of the controls, not a list of selected parts". A list box gives the
//! list its own surface:
//!
//! - an inset container across the panel width: the fill `LIST_FILL`, a
//!   1-point `HAIRLINE` border and the radius `RADIUS_SM`;
//! - a list header ([`list_header`]): the title at the left, the count at
//!   the right, then its quiet actions;
//! - list rows ([`list_row`]): one line each, a hover highlight and a faint
//!   separator under each row.
//!
//! A field row (`PropRow`) stays outside the box.

use crate::ui::components::Button;
use crate::ui::tokens;

/// The inset container of a list. See the module doc.
#[must_use]
pub struct ListBox {
    id_salt: egui::Id,
}

impl ListBox {
    pub fn new(id_salt: impl std::hash::Hash + std::fmt::Debug) -> Self {
        Self {
            id_salt: egui::Id::new(id_salt),
        }
    }

    /// Draw the box across the available width; `body` draws the header
    /// and the rows in it.
    pub fn show<R>(
        self,
        ui: &mut egui::Ui,
        body: impl FnOnce(&mut egui::Ui) -> R,
    ) -> egui::InnerResponse<R> {
        ui.push_id(self.id_salt, |ui| {
            egui::Frame::NONE
                .fill(tokens::LIST_FILL)
                .stroke(egui::Stroke::new(1.0, tokens::HAIRLINE))
                .corner_radius(tokens::RADIUS_SM)
                .inner_margin(egui::Margin::symmetric(
                    tokens::SPACE_2 as i8,
                    tokens::SPACE_1 as i8,
                ))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.spacing_mut().item_spacing.y = 0.0;
                    body(ui)
                })
                .inner
        })
    }
}

/// What a list line reports: the index of the clicked action, and whether
/// the pointer is over the line.
pub struct ListLine {
    pub clicked: Option<usize>,
    pub hovered: bool,
}

/// The header of a list: `title` at the left in `TEXT_STRONG`, `count` at
/// the right in `TEXT_FAINT`, then the quiet `actions` (label, hover).
pub fn list_header(
    ui: &mut egui::Ui,
    title: &str,
    count: &str,
    hover: &str,
    actions: &[(&str, &str)],
) -> ListLine {
    line(
        ui,
        egui::RichText::new(title)
            .size(tokens::SIZE_CAPTION)
            .color(tokens::TEXT_STRONG),
        Some(count),
        hover,
        actions,
        false,
    )
}

/// One row of a list: `text` in `color`, the quiet `actions` (label,
/// hover) at the right edge, the hover highlight and a faint separator
/// under the row. The text truncates; `hover` gives the full text.
pub fn list_row(
    ui: &mut egui::Ui,
    text: &str,
    color: egui::Color32,
    hover: &str,
    actions: &[(&str, &str)],
) -> ListLine {
    line(
        ui,
        egui::RichText::new(text).color(color),
        None,
        hover,
        actions,
        true,
    )
}

/// One line of the box. FR-2 F22: the action buttons take their width
/// first, at the right edge; the text takes what is left and truncates.
fn line(
    ui: &mut egui::Ui,
    text: egui::RichText,
    count: Option<&str>,
    hover: &str,
    actions: &[(&str, &str)],
    separator: bool,
) -> ListLine {
    // The highlight paints under the line, so its slot comes first.
    let highlight = ui.painter().add(egui::Shape::Noop);
    let mut clicked = None;
    let line = ui.horizontal(|ui| {
        ui.set_min_height(tokens::ROW_DENSE);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            for (index, (label, action_hover)) in actions.iter().enumerate().rev() {
                if ui
                    .add(Button::quiet(*label))
                    .on_hover_text(*action_hover)
                    .clicked()
                {
                    clicked = Some(index);
                }
            }
            if let Some(count) = count {
                ui.label(
                    egui::RichText::new(count)
                        .size(tokens::SIZE_CAPTION)
                        .color(tokens::TEXT_FAINT),
                );
            }
            let rest = ui.available_width().max(0.0);
            ui.allocate_ui_with_layout(
                egui::vec2(rest, tokens::ROW_DENSE),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.set_max_width(rest);
                    ui.add(
                        egui::Label::new(text)
                            .truncate()
                            .show_tooltip_when_elided(false),
                    )
                    .on_hover_text(hover);
                },
            );
        });
    });
    let rect = line.response.rect;
    let hovered = ui.rect_contains_pointer(rect);
    if hovered && separator {
        ui.painter().set(
            highlight,
            egui::Shape::rect_filled(
                rect,
                egui::CornerRadius::from(tokens::RADIUS_SM),
                tokens::SURFACE_RAISED,
            ),
        );
    }
    if separator {
        ui.painter().line_segment(
            [rect.left_bottom(), rect.right_bottom()],
            egui::Stroke::new(1.0, tokens::HAIRLINE.gamma_multiply(0.5)),
        );
    }
    ListLine { clicked, hovered }
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    reason = "test code: a panic is how a test reports a failure"
)]
mod tests {
    use super::*;

    /// W3g: the box paints its own inset surface (the list fill and the
    /// border) across the width, and a row paints a separator under it.
    #[test]
    fn a_list_box_has_its_own_surface_w3g() {
        let ctx = egui::Context::default();
        tokens::apply(&ctx);
        tokens::apply_fonts(&ctx);
        let mut warmup = ctx.run_ui(egui::RawInput::default(), |_ui| {});
        warmup.textures_delta.clear();
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            ui.set_max_width(300.0);
            let _ = ListBox::new("t").show(ui, |ui| {
                let _ = list_header(ui, "Picked faces", "2", "Picked faces (2)", &[]);
                let _ = list_row(
                    ui,
                    "Face Z20.00",
                    tokens::TEXT_BODY,
                    "Face",
                    &[("x", "Remove")],
                );
                let _ = list_row(ui, "Face Z12.00", tokens::TEXT_BODY, "Face", &[]);
            });
        });
        let boxes: Vec<egui::Rect> = out
            .shapes
            .iter()
            .filter_map(|c| match &c.shape {
                egui::epaint::Shape::Rect(r)
                    if r.fill == tokens::LIST_FILL && r.stroke.color == tokens::HAIRLINE =>
                {
                    Some(r.rect)
                }
                _ => None,
            })
            .collect();
        let separators = out
            .shapes
            .iter()
            .filter(|c| matches!(&c.shape, egui::epaint::Shape::LineSegment { .. }))
            .count();
        out.textures_delta.clear();
        assert_eq!(boxes.len(), 1, "one inset surface");
        assert!(boxes.first().unwrap().width() > 250.0, "{boxes:?}");
        assert_eq!(separators, 2, "one separator per row");
    }
}
