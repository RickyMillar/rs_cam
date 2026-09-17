//! Sections & param grids — the `UiExt` trait, plus `SummaryCard`.
//!
//! Component layer (IA cleanup, Wave 1). The audit found the same section
//! header inlined many times, and the same 2-column param grid repeated
//! everywhere with its own spacing. These helpers standardize both so every
//! surface speaks one layout grammar (P3-*, INS-001, TIM-008).
//!
//! Measured 2026-09-14: the crate carried **nine different grid spacings**.
//! Nine rhythms in one product is what "cluttered" looks like from the
//! operator's side, so all of them are now the token pair `SPACE_3` by
//! `SPACE_2`, with `ROW_DENSE` as the row height.
//!
//! `SummaryCard` is the "dig deeper" primitive: a glanceable header with an
//! optional badge over a body hidden behind a disclosure — the summary-first
//! pattern the redesign applies to every dense panel.

use crate::ui::components::card::SectionHeader;

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
    ///
    /// UI-02: the id salt is any `Hash` value, not a `&str`, because two of
    /// the 69 converted call sites name their grid with a tuple
    /// (`("selected_metrics_grid", sid)`) or with `ui.next_auto_id()`.
    /// A narrower parameter would have left those two as raw grids for a
    /// reason that is about the id, not about the layout.
    fn param_grid(&mut self, id_salt: impl egui::AsIdSalt, add: impl FnOnce(&mut egui::Ui));
}

impl UiExt for egui::Ui {
    fn named_section(&mut self, title: &str, add: impl FnOnce(&mut egui::Ui)) {
        // UP2: one implementation, in `components::card::SectionHeader`.
        // Every call site gains the treatment without being edited.
        SectionHeader::new(title).show(self);
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

    fn param_grid(&mut self, id_salt: impl egui::AsIdSalt, add: impl FnOnce(&mut egui::Ui)) {
        egui::Grid::new(id_salt)
            .num_columns(2)
            .spacing([crate::ui::tokens::SPACE_3, crate::ui::tokens::SPACE_2])
            .min_row_height(crate::ui::tokens::ROW_DENSE)
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
