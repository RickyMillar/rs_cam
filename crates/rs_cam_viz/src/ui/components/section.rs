//! Sections & param grids — the `UiExt` trait, plus `SummaryCard`.
//!
//! Component layer (IA cleanup, Wave 1). The audit found the same section
//! header (`RichText::new(..).small().strong().color(..)`) inlined 105+ times
//! and the same 2-column param grid (`Grid::new(..).num_columns(2)
//! .spacing([8.0, 4.0])`) repeated everywhere. These helpers standardize both
//! so every surface speaks one layout grammar (P3-*, INS-001, TIM-008).
//!
//! `SummaryCard` is the "dig deeper" primitive: a glanceable header with an
//! optional badge over a body hidden behind a disclosure — the summary-first
//! pattern the redesign applies to every dense panel.

use crate::ui::theme;

/// Layout helpers on `egui::Ui`. The canonical homes for the section-header
/// and param-grid idioms the audit found copy-pasted across the GUI.
pub trait UiExt {
    /// Always-visible labelled group: a small/strong heading, then the body.
    fn named_section(&mut self, title: &str, add: impl FnOnce(&mut egui::Ui));

    /// Collapsible "▸ title" drawer (the "Advanced" disclosure idiom).
    fn disclosure(
        &mut self,
        id_salt: &str,
        title: &str,
        default_open: bool,
        add: impl FnOnce(&mut egui::Ui),
    ) -> egui::CollapsingResponse<()>;

    /// 2-column param grid with the canonical `[8.0, 4.0]` spacing.
    fn param_grid(&mut self, id_salt: &str, add: impl FnOnce(&mut egui::Ui));
}

impl UiExt for egui::Ui {
    fn named_section(&mut self, title: &str, add: impl FnOnce(&mut egui::Ui)) {
        self.label(
            egui::RichText::new(title)
                .small()
                .strong()
                .color(theme::TEXT_HEADING),
        );
        add(self);
    }

    fn disclosure(
        &mut self,
        id_salt: &str,
        title: &str,
        default_open: bool,
        add: impl FnOnce(&mut egui::Ui),
    ) -> egui::CollapsingResponse<()> {
        egui::CollapsingHeader::new(title)
            .id_salt(id_salt)
            .default_open(default_open)
            .show(self, add)
    }

    fn param_grid(&mut self, id_salt: &str, add: impl FnOnce(&mut egui::Ui)) {
        egui::Grid::new(id_salt)
            .num_columns(2)
            .spacing([8.0, 4.0])
            .show(self, add);
    }
}

/// Glanceable header + body-behind-disclosure. The "dig deeper" primitive.
pub struct SummaryCard {
    headline: egui::WidgetText,
    default_open: bool,
}

impl SummaryCard {
    pub fn new(headline: impl Into<egui::WidgetText>) -> Self {
        Self {
            headline: headline.into(),
            default_open: false,
        }
    }

    pub fn default_open(mut self, open: bool) -> Self {
        self.default_open = open;
        self
    }

    pub fn show(self, ui: &mut egui::Ui, body: impl FnOnce(&mut egui::Ui)) {
        egui::CollapsingHeader::new(self.headline)
            .default_open(self.default_open)
            .show(ui, body);
    }
}
