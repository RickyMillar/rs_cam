//! `OneLine` — a sentence that takes one row of the panel.
//!
//! IC1 (inspector clean-up, 2026-10-13). The inspector printed findings,
//! provenance and stage records as wrapped paragraphs of two to five rows.
//! A line of this kind never wraps. When the text is wider than the row, the
//! face truncates with an ellipsis and the hover shows the full text, then
//! the detail. The painted galley keeps the full text, so a sentry that reads
//! the face still reads the whole record.
//!
//! Put the numbers first in the face text: the truncation keeps the start.

use crate::ui::tokens;

/// One row of text with an optional detail on hover.
#[must_use]
pub struct OneLine {
    text: String,
    color: egui::Color32,
    detail: Option<String>,
}

impl OneLine {
    /// A caption-size line in the muted colour.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            color: tokens::TEXT_MUTED,
            detail: None,
        }
    }

    /// The text colour: a role colour for a finding, muted for a record.
    pub fn color(mut self, color: egui::Color32) -> Self {
        self.color = color;
        self
    }

    /// The explanation the hover adds below the full text.
    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// The detail when there is one.
    pub fn detail_opt(mut self, detail: Option<impl Into<String>>) -> Self {
        self.detail = detail.map(Into::into);
        self
    }
}

impl egui::Widget for OneLine {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let label = egui::Label::new(
            egui::RichText::new(&self.text)
                .size(tokens::SIZE_CAPTION)
                .color(self.color),
        )
        .truncate();
        match self.detail {
            // The hover holds the full face text and the detail, so the
            // built-in elided tooltip would only repeat its first part.
            Some(detail) => ui
                .add(label.show_tooltip_when_elided(false))
                .on_hover_text(format!("{}\n{detail}", self.text)),
            None => ui.add(label),
        }
    }
}
