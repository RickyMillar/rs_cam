//! `Button` — `DESIGN_SPEC.md` §4.5.
//!
//! Four variants. The audit found **129 of 132 buttons identical**, so the
//! product could not say which action on a screen was the one to take.
//!
//! | Variant | Fill | Text | Use |
//! |---|---|---|---|
//! | [`Variant::Primary`] | `ACCENT` | `INK_05` | the one action a screen is FOR. At most one per screen. |
//! | [`Variant::Default`] | `SURFACE_RAISED` | `TEXT_BODY` | an ordinary action |
//! | [`Variant::Quiet`] | transparent | `TEXT_MUTED` | a tertiary action, a row affordance |
//! | [`Variant::Danger`] | `TINT_DANGER` | `DANGER` | deletes or refuses something |
//!
//! # Contrast, computed
//!
//! Ruling R8. `INK_05` on `ACCENT` is **6.22**, and on `ACCENT_PRESSED`
//! **4.59**. Both clear the 4.5 floor. The obvious alternative was tested and
//! rejected: `INK_95` on `ACCENT` reads **2.32**, so a white-on-blue primary
//! button would have failed §9 outright.

use crate::ui::{components::motion, tokens};

/// Which of the four §4.5 variants a button is.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Variant {
    /// The one action a screen is FOR. At most one per screen.
    Primary,
    /// An ordinary action.
    #[default]
    Default,
    /// A tertiary action or a row affordance.
    Quiet,
    /// Deletes or refuses something.
    Danger,
}

impl Variant {
    /// The resting fill.
    #[must_use]
    pub fn fill(self) -> egui::Color32 {
        match self {
            Self::Primary => tokens::ACCENT,
            Self::Default => tokens::SURFACE_RAISED,
            Self::Quiet => egui::Color32::TRANSPARENT,
            Self::Danger => tokens::TINT_DANGER,
        }
    }

    /// The pressed fill.
    #[must_use]
    pub fn pressed_fill(self) -> egui::Color32 {
        match self {
            Self::Primary => tokens::ACCENT_PRESSED,
            Self::Default => tokens::SURFACE_OVERLAY,
            Self::Quiet => tokens::SURFACE_RAISED,
            Self::Danger => tokens::DANGER,
        }
    }

    /// The text colour.
    #[must_use]
    pub fn text(self) -> egui::Color32 {
        match self {
            // Dark ink on the accent. See the contrast note above.
            Self::Primary => tokens::INK_05,
            Self::Default => tokens::TEXT_BODY,
            Self::Quiet => tokens::TEXT_MUTED,
            Self::Danger => tokens::DANGER,
        }
    }

    /// The border. `Quiet` has none until it is hovered.
    #[must_use]
    pub fn stroke(self) -> egui::Stroke {
        match self {
            Self::Primary => egui::Stroke::NONE,
            Self::Default | Self::Danger => egui::Stroke::new(1.0, tokens::BORDER),
            Self::Quiet => egui::Stroke::NONE,
        }
    }
}

/// A button in one of the four §4.5 variants.
pub struct Button {
    text: String,
    variant: Variant,
    enabled: bool,
    min_width: Option<f32>,
}

impl Button {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            variant: Variant::Default,
            enabled: true,
            min_width: None,
        }
    }

    /// The one action a screen is FOR. At most one per screen.
    #[must_use]
    pub fn primary(text: impl Into<String>) -> Self {
        Self::new(text).variant(Variant::Primary)
    }

    /// A tertiary action or a row affordance.
    #[must_use]
    pub fn quiet(text: impl Into<String>) -> Self {
        Self::new(text).variant(Variant::Quiet)
    }

    /// Deletes or refuses something.
    #[must_use]
    pub fn danger(text: impl Into<String>) -> Self {
        Self::new(text).variant(Variant::Danger)
    }

    #[must_use]
    pub fn variant(mut self, variant: Variant) -> Self {
        self.variant = variant;
        self
    }

    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    #[must_use]
    pub fn min_width(mut self, w: f32) -> Self {
        self.min_width = Some(w);
        self
    }
}

impl egui::Widget for Button {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let id = ui.next_auto_id();

        // Every button is at least ROW_ACTION tall. `Style::interact_size.y`
        // carries this globally since UP1, but a button that sets its own
        // min_size must not fall below it.
        let min = egui::vec2(self.min_width.unwrap_or(0.0), tokens::ROW_ACTION);

        let widget = egui::Button::new(
            egui::RichText::new(&self.text)
                .text_style(egui::TextStyle::Button)
                .color(self.variant.text()),
        )
        .min_size(min)
        .corner_radius(tokens::RADIUS_SM)
        .stroke(self.variant.stroke())
        .fill(self.variant.fill());

        let response = ui.add_enabled(self.enabled, widget);

        // Hover and press repaint the fill through the token ladder rather
        // than through egui's own widget visuals, so a variant keeps its
        // identity in every state.
        if self.enabled && (response.hovered() || response.is_pointer_button_down_on()) {
            let t = motion::fast(ui.ctx(), id, true);
            let target = if response.is_pointer_button_down_on() {
                self.variant.pressed_fill()
            } else {
                tokens::hover_lift(self.variant.fill())
            };
            let fill = motion::mix(self.variant.fill(), target, t);
            ui.painter().rect_filled(
                response.rect,
                egui::CornerRadius::from(tokens::RADIUS_SM),
                fill,
            );
            // Repaint the label over the new ground.
            ui.painter().text(
                response.rect.center(),
                egui::Align2::CENTER_CENTER,
                &self.text,
                ui.style()
                    .text_styles
                    .get(&egui::TextStyle::Button)
                    .cloned()
                    .unwrap_or_default(),
                self.variant.text(),
            );
        }

        response
    }
}
