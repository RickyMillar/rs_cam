//! `SuggestButton` — the apply action, three roles, one type.
//!
//! Component layer (IA cleanup, Wave 1). The audit found the suggest action
//! hand-rolled in several places (`suggest_pill` + bulk "Suggest all" buttons)
//! and, worse, visually confusable with the provenance *badge* — a coloured ⚡
//! could read as "where from" rather than "apply" (finding P4-003 / P7-003).
//!
//! `SuggestButton` is the single apply affordance. The provenance signal is a
//! separate type ([`super::provenance::ProvenanceBadge`]), so "where from" can
//! never be mistaken for "apply" again. A [`SuggestScope::Field`] renders one
//! ⚡ in the source colour (apply this one field); a [`SuggestScope::Recipe`]
//! renders a doubled-glyph labelled button (apply a whole recipe).

use super::provenance::ProvKind;

/// What a suggest action applies.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SuggestScope {
    /// A single field — rendered as one ⚡ chip.
    Field,
    /// A whole recipe — rendered as `⚡⚡ <label>`.
    Recipe,
}

/// A single-field recommendation: the value to write and where it came from.
pub struct Suggestion<'a> {
    pub recommended: f64,
    pub source: ProvKind,
    pub reference: Option<&'a str>,
}

/// The suggest action widget. `enabled = false` greys it (e.g. the current
/// value already matches the recommendation).
pub struct SuggestButton<'a> {
    scope: SuggestScope,
    source: ProvKind,
    label: Option<&'a str>,
    hover: Option<String>,
    enabled: bool,
}

impl<'a> SuggestButton<'a> {
    pub fn field(source: ProvKind) -> Self {
        Self {
            scope: SuggestScope::Field,
            source,
            label: None,
            hover: None,
            enabled: true,
        }
    }

    pub fn recipe(source: ProvKind, label: &'a str) -> Self {
        Self {
            scope: SuggestScope::Recipe,
            source,
            label: Some(label),
            hover: None,
            enabled: true,
        }
    }

    pub fn hover(mut self, text: impl Into<String>) -> Self {
        self.hover = Some(text.into());
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

impl egui::Widget for SuggestButton<'_> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let color = self.source.color();
        let btn = match self.scope {
            SuggestScope::Field => {
                egui::Button::new(egui::RichText::new("\u{26A1}").color(color)).small()
            }
            SuggestScope::Recipe => {
                let label = self.label.unwrap_or("Apply recommended");
                egui::Button::new(
                    egui::RichText::new(format!("\u{26A1}\u{26A1} {label}")).color(color),
                )
                .small()
            }
        };
        let resp = ui.add_enabled(self.enabled, btn);
        match self.hover {
            Some(h) => resp.on_hover_text(h),
            None => resp,
        }
    }
}
