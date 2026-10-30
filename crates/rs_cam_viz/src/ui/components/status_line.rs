//! `StatusLine` — the status line of an inspector header (W2b, W6).
//!
//! One line, from the right edge: the one button, the notes counter
//! ([`NotesCounter`]), then a faint meta text (the move count). The state
//! word takes the width that is left and truncates. A word marked
//! [`StatusLine::keep_whole`] (the F20 safety signal "Done · BLOCKED")
//! never truncates: it takes its full width first, and the meta text drops.
//! The line never grows the panel (FR-2 F22).

use crate::ui::components::{Button, NotesCounter, Notice};
use crate::ui::tokens;

/// The status line. See the module doc.
#[must_use]
pub struct StatusLine<'a> {
    word: &'a str,
    color: egui::Color32,
    hover: String,
    keep_whole: bool,
    meta: Option<(String, String)>,
    button: Option<Button>,
    notes: &'a [Notice],
}

impl<'a> StatusLine<'a> {
    /// A line with the state `word` in `color`; `hover` explains the word.
    pub fn new(word: &'a str, color: egui::Color32, hover: impl Into<String>) -> Self {
        Self {
            word,
            color,
            hover: hover.into(),
            keep_whole: false,
            meta: None,
            button: None,
            notes: &[],
        }
    }

    /// The word never truncates; the meta text drops first.
    pub fn keep_whole(mut self, keep: bool) -> Self {
        self.keep_whole = keep;
        self
    }

    /// The faint meta text and its hover. The meta text can drop out of
    /// the line, so the caller puts what it says in the word hover too.
    pub fn meta(mut self, meta: Option<(String, String)>) -> Self {
        self.meta = meta;
        self
    }

    /// The one button at the right edge.
    pub fn button(mut self, button: Option<Button>) -> Self {
        self.button = button;
        self
    }

    /// The notes of the counter (none draws no counter).
    pub fn notes(mut self, notes: &'a [Notice]) -> Self {
        self.notes = notes;
        self
    }

    /// Draw the line. True when the button was clicked.
    pub fn show(self, ui: &mut egui::Ui) -> bool {
        let Self {
            word,
            color,
            hover,
            keep_whole,
            meta,
            button,
            notes,
        } = self;
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let clicked = button.is_some_and(|button| ui.add(button).clicked());
                let _ = NotesCounter::new(notes).show(ui);
                if let Some((text, text_hover)) = meta {
                    let word_room = if keep_whole {
                        let font = egui::TextStyle::Body.resolve(ui.style());
                        ui.painter()
                            .layout_no_wrap(word.to_owned(), font, color)
                            .size()
                            .x
                    } else {
                        0.0
                    };
                    if small_width(ui, &text) + word_room <= ui.available_width() {
                        let _ = ui
                            .label(egui::RichText::new(text).small().color(tokens::TEXT_FAINT))
                            .on_hover_text(text_hover);
                    }
                }
                let rest = ui.available_width().max(0.0);
                ui.allocate_ui_with_layout(
                    egui::vec2(rest, tokens::ROW_DENSE),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        ui.set_max_width(rest);
                        let text = egui::RichText::new(word).color(color);
                        let label = if keep_whole {
                            egui::Label::new(text).extend()
                        } else {
                            egui::Label::new(text)
                                .truncate()
                                .show_tooltip_when_elided(false)
                        };
                        let _ = ui.add(label).on_hover_text(hover);
                    },
                );
                clicked
            })
            .inner
        })
        .inner
    }
}

/// The width of one small label of `text`.
fn small_width(ui: &egui::Ui, text: &str) -> f32 {
    let font = egui::TextStyle::Small.resolve(ui.style());
    ui.painter()
        .layout_no_wrap(text.to_owned(), font, tokens::TEXT_FAINT)
        .size()
        .x
        + ui.spacing().item_spacing.x
}
