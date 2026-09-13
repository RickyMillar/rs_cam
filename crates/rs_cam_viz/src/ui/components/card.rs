//! `SectionHeader` (§4.1) and `Card` (§4.2).
//!
//! # SectionHeader
//!
//! The audit found the same header — `.small().strong()` in a grey — inlined
//! 41 times, beside a `UiExt::named_section` helper that only 10 call sites
//! actually use. `named_section` now calls into [`SectionHeader`], so those
//! 10 gain the treatment with no edit. **The other 41 are hand work**, and
//! §3.3 already schedules them.
//!
//! # Card
//!
//! `theme::card_frame` keeps its signature and calls into [`Card`], so its 5
//! call sites move without being edited.

use crate::ui::{
    components::{motion, text},
    tokens,
};

/// A section header: an optional chevron, the title, an optional trailing
/// slot, and a hairline beneath.
pub struct SectionHeader {
    title: String,
    trailing: Option<String>,
    rule: bool,
}

impl SectionHeader {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            trailing: None,
            rule: true,
        }
    }

    /// A count or a status word at the right end of the header.
    #[must_use]
    pub fn trailing(mut self, trailing: impl Into<String>) -> Self {
        self.trailing = Some(trailing.into());
        self
    }

    /// Draw the hairline beneath. On by default.
    #[must_use]
    pub fn rule(mut self, rule: bool) -> Self {
        self.rule = rule;
        self
    }

    /// Draw the header. `SPACE_4` above and `SPACE_2` below (§2.1).
    ///
    /// Deliberately NOT `SPACE_5` / `SPACE_3`: the operator ruled that an
    /// inspector at the looser rhythm "looks a bit too spaced apart" and
    /// should "retain some density".
    pub fn show(self, ui: &mut egui::Ui) {
        ui.add_space(tokens::SPACE_4);
        ui.horizontal(|ui| {
            ui.label(text::subhead(&self.title));
            if let Some(trailing) = &self.trailing {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(text::caption(trailing));
                });
            }
        });
        if self.rule {
            // Ruling R5: the semantic token, never the ramp position.
            let y = ui.cursor().top();
            let x = ui.max_rect().x_range();
            ui.painter()
                .hline(x, y, egui::Stroke::new(1.0, tokens::HAIRLINE));
        }
        ui.add_space(tokens::SPACE_2);
    }
}

/// A raised, rounded surface holding a group of related things.
pub struct Card {
    selected: bool,
    hovered: bool,
}

impl Default for Card {
    fn default() -> Self {
        Self::new()
    }
}

impl Card {
    #[must_use]
    pub fn new() -> Self {
        Self {
            selected: false,
            hovered: false,
        }
    }

    #[must_use]
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Tell the card it is hovered. The caller owns the hit test, because a
    /// card's hover region is usually its whole row, not the frame.
    #[must_use]
    pub fn hovered(mut self, hovered: bool) -> Self {
        self.hovered = hovered;
        self
    }

    /// The frame this card paints.
    ///
    /// Ruling R14: when a card is both selected and hovered, SELECTION owns
    /// the fill — selection is state and hover is transient — and hover adds
    /// the accent stroke. That introduces no fourth carrier of the accent,
    /// because the card is already selected.
    pub fn frame(&self) -> egui::Frame {
        let fill = if self.selected {
            tokens::ACCENT_QUIET
        } else if self.hovered {
            tokens::hover_lift(tokens::SURFACE_RAISED)
        } else {
            tokens::SURFACE_RAISED
        };

        let stroke = if self.selected && self.hovered {
            egui::Stroke::new(1.0, tokens::ACCENT)
        } else if self.selected {
            egui::Stroke::new(1.0, tokens::ACCENT_QUIET)
        } else {
            egui::Stroke::new(1.0, tokens::HAIRLINE)
        };

        egui::Frame::default()
            .fill(fill)
            .stroke(stroke)
            .inner_margin(tokens::SPACE_3)
            .corner_radius(tokens::RADIUS_MD)
    }

    /// Draw the card and its body.
    pub fn show<R>(
        self,
        ui: &mut egui::Ui,
        body: impl FnOnce(&mut egui::Ui) -> R,
    ) -> egui::InnerResponse<R> {
        self.frame().show(ui, body)
    }
}

/// A full-viewport scrim, painted behind a modal window (§2.3).
///
/// The pattern existed once, at `ui/optimize_project.rs:572-574`. It is the
/// rule now: a modal that does not dim what it covers reads as a panel that
/// happens to float.
pub fn scrim(ctx: &egui::Context, id: egui::Id, visible: bool) {
    let t = motion::slow(ctx, id, visible);
    if t <= 0.0 {
        return;
    }
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Background,
        id.with("scrim"),
    ));
    let mut colour = tokens::SCRIM;
    colour = egui::Color32::from_rgba_premultiplied(
        colour.r(),
        colour.g(),
        colour.b(),
        (f32::from(colour.a()) * t).round().clamp(0.0, 255.0) as u8,
    );
    painter.rect_filled(ctx.viewport_rect(), egui::CornerRadius::ZERO, colour);
}
