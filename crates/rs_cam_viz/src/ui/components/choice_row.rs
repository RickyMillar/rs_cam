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
//! The options strip wraps. A label column plus two long option words is
//! wider than the 240 point Simulation rail, and a row that cannot wrap
//! pushes the whole inspector off its own left edge (UP4, `AUDIT.md` D-16).

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
}

impl<T: PartialEq + Copy> egui::Widget for ChoiceRow<'_, T> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let mut changed = false;
        let inner = ui.horizontal_wrapped(|ui| {
            // A fixed, truncating label column, the same rung `ValueRow`
            // uses, so this row lines up with the parameter rows above it
            // and cannot widen the panel.
            ui.add_sized(
                [tokens::LABEL_COL_WIDTH, tokens::ROW_DENSE],
                egui::Label::new(self.label)
                    .truncate()
                    .wrap_mode(egui::TextWrapMode::Truncate)
                    .halign(egui::Align::LEFT),
            );
            for (option, text) in self.options {
                let selected = *self.value == *option;
                if ui.add(egui::Button::selectable(selected, *text)).clicked() && !selected {
                    *self.value = *option;
                    changed = true;
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
