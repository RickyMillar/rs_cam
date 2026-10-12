//! `ChoiceRow` — the one labelled two-way (or n-way) choice row.
//!
//! The kit had no renderer for a small closed choice, so every such control
//! in the app was a raw widget: a bare `ui.checkbox` for the generic stock
//! source, a pair of `ui.selectable_label` calls for pencil's rest
//! reference. Two raw controls wrote ONE field, and they disagreed on
//! screen.
//!
//! `ChoiceRow` is the renderer for that element. It is a leaf widget, so a
//! caller writes `ui.add(ChoiceRow::new(..))`. It owns no state, reads only
//! `ui/tokens.rs`, and reports a real move through `Response::changed`.
//!
//! The options strip never SPLITS. A label column plus two long option words
//! is wider than the 240 point rail, and the first drawing wrapped inside
//! the strip: one option stayed on the label's line and the other dropped
//! below it, so a closed pair of two read as two unrelated controls
//! (operator, 2026-09-18). The row MEASURES instead. The options sit beside
//! the label when they fit, and otherwise the label takes its own line and
//! the whole set sits under it. A row that cannot wrap at all pushes the
//! inspector off its own left edge (UP4, `AUDIT.md` D-16), so the fallback
//! is a stack, never an overflow.

use crate::ui::tokens;

/// A label, a closed set of options, and the option in force.
///
/// `T` needs `PartialEq + Copy` only. The row compares the current value
/// against each option and writes the option the operator clicks.
pub struct ChoiceRow<'a, T: PartialEq + Copy> {
    label: &'a str,
    value: &'a mut T,
    options: &'a [(T, &'a str)],
    hover: Option<&'a str>,
}

impl<'a, T: PartialEq + Copy> ChoiceRow<'a, T> {
    pub fn new(label: &'a str, value: &'a mut T, options: &'a [(T, &'a str)]) -> Self {
        Self {
            label,
            value,
            options,
            hover: None,
        }
    }

    /// One hover for the whole row. It names every option, because the
    /// difference between the options is what the operator needs.
    #[must_use]
    pub fn hover(mut self, hover: &'a str) -> Self {
        self.hover = Some(hover);
        self
    }

    /// The width of every option on one line, with the gaps, in points.
    ///
    /// Measured, not guessed. The widths come from the same layout the
    /// buttons then use, so the answer cannot disagree with what is drawn.
    fn options_width(&self, ui: &egui::Ui) -> f32 {
        let font = ui
            .style()
            .text_styles
            .get(&egui::TextStyle::Button)
            .cloned()
            .unwrap_or_default();
        let padding = ui.spacing().button_padding.x * 2.0;
        let gap = ui.spacing().item_spacing.x;
        self.options
            .iter()
            .map(|(_, text)| {
                ui.painter()
                    .layout_no_wrap((*text).to_owned(), font.clone(), egui::Color32::PLACEHOLDER)
                    .size()
                    .x
                    + padding
                    + gap
            })
            .sum()
    }

    /// Is there room for the label column AND every option on one line?
    fn options_fit_beside_the_label(&self, ui: &egui::Ui) -> bool {
        let gap = ui.spacing().item_spacing.x;
        tokens::LABEL_COL_WIDTH + gap + self.options_width(ui) <= ui.available_width()
    }

    /// Is there room for every option on one line of its own?
    ///
    /// R5b (2026-10-12): the stacked fallback put the options in one
    /// `horizontal` strip. Three long options ("3 pins: axis + mirror
    /// pair", "2 pins, mixed diameter", "Custom") are wider than the 240
    /// point rail, and the strip ran past the panel edge. A set that does
    /// not fit on one line goes one option per line.
    fn options_fit_on_a_line(&self, ui: &egui::Ui) -> bool {
        self.options_width(ui) <= ui.available_width()
    }
}

impl<T: PartialEq + Copy> egui::Widget for ChoiceRow<'_, T> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let mut changed = false;
        let inline = self.options_fit_beside_the_label(ui);
        let one_line = inline || self.options_fit_on_a_line(ui);
        let value = self.value;
        let options = self.options;
        let mut draw_options = |ui: &mut egui::Ui| {
            for (option, text) in options {
                let selected = *value == *option;
                if ui.add(egui::Button::selectable(selected, *text)).clicked() && !selected {
                    *value = *option;
                    changed = true;
                }
            }
        };
        let inner = ui.vertical(|ui| {
            if inline {
                ui.horizontal(|ui| {
                    // A fixed, truncating label column, the same cell
                    // `ValueRow` uses, so this row lines up with the
                    // parameter rows above it and cannot widen the panel.
                    super::value_row::label_cell(ui, self.label);
                    draw_options(ui);
                });
            } else {
                // The label takes its own line, at the panel width, and the
                // closed set stays together under it.
                ui.add(egui::Label::new(self.label).truncate());
                if one_line {
                    ui.horizontal(&mut draw_options);
                } else {
                    ui.vertical(&mut draw_options);
                }
            }
        });
        let mut response = inner.response;
        if let Some(hover) = self.hover {
            response = response.on_hover_text(hover);
        }
        if changed {
            response.mark_changed();
        }
        response
    }
}
