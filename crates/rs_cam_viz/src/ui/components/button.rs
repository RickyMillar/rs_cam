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
//! W2c: a disabled button of every variant reads as disabled. It has no
//! accent or tint fill: `SURFACE_RAISED` (`Quiet` stays transparent), a
//! `BORDER` stroke (`Quiet` has none) and `TEXT_FAINT` text. egui does not
//! dim an explicit fill, so the variant gives the disabled colours itself.
//! [`Button::disabled_reason`] shows the reason on hover.
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
    pub(crate) fn pressed_fill(self) -> egui::Color32 {
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

    /// The fill while the button is disabled. No variant keeps its accent
    /// or tint fill, so a disabled button does not read as an action.
    #[must_use]
    pub fn disabled_fill(self) -> egui::Color32 {
        match self {
            Self::Primary | Self::Default | Self::Danger => tokens::SURFACE_RAISED,
            Self::Quiet => egui::Color32::TRANSPARENT,
        }
    }

    /// The text colour while the button is disabled.
    #[must_use]
    pub fn disabled_text(self) -> egui::Color32 {
        tokens::TEXT_FAINT
    }

    /// The border while the button is disabled.
    #[must_use]
    pub fn disabled_stroke(self) -> egui::Stroke {
        match self {
            Self::Primary | Self::Default | Self::Danger => egui::Stroke::new(1.0, tokens::BORDER),
            Self::Quiet => egui::Stroke::NONE,
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
    progress: Option<f32>,
    disabled_reason: Option<String>,
}

impl Button {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            variant: Variant::Default,
            enabled: true,
            min_width: None,
            progress: None,
            disabled_reason: None,
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

    /// The reason the button is disabled. The hover shows it while the
    /// button is disabled; an enabled button ignores it.
    #[must_use]
    pub fn disabled_reason(mut self, reason: impl Into<String>) -> Self {
        self.disabled_reason = Some(reason.into());
        self
    }

    #[must_use]
    pub fn min_width(mut self, w: f32) -> Self {
        self.min_width = Some(w);
        self
    }

    /// How far a job this button started has got, as a fraction of its own
    /// width, painted as a bar along the bottom edge. `None` paints nothing.
    ///
    /// A progress BAR and not a second label. The button paints its label
    /// exactly ONCE (`component_contracts_up2`), so the progress WORDS are
    /// the button's own `text`, which the caller composes. That also keeps
    /// the one centred label centred while a job runs.
    #[must_use]
    pub fn progress(mut self, fraction: f32) -> Self {
        self.progress = Some(fraction.clamp(0.0, 1.0));
        self
    }
}

impl egui::Widget for Button {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let id = ui.next_auto_id();

        // Every button is at least ROW_ACTION tall. `Style::interact_size.y`
        // carries this globally since UP1, but a button that sets its own
        // min_size must not fall below it.
        let font_id = ui
            .style()
            .text_styles
            .get(&egui::TextStyle::Button)
            .cloned()
            .unwrap_or_default();
        let text_size = ui
            .painter()
            .layout_no_wrap(self.text.clone(), font_id.clone(), self.variant.text())
            .size();
        let padding = ui.spacing().button_padding;
        let min = egui::vec2(
            self.min_width
                .unwrap_or(0.0)
                .max(text_size.x + 2.0 * padding.x),
            tokens::ROW_ACTION.max(text_size.y + 2.0 * padding.y),
        );

        // Keep egui responsible for the button's frame, disabled state, and
        // WidgetInfo label. The transparent native label reserves its space
        // without competing with the one centered label painted below.
        // W2c: egui does not dim an explicit fill, so a disabled button
        // takes the variant's disabled colours.
        let (fill, stroke) = if self.enabled {
            (self.variant.fill(), self.variant.stroke())
        } else {
            (self.variant.disabled_fill(), self.variant.disabled_stroke())
        };
        let widget =
            egui::Button::new(egui::RichText::new(&self.text).color(egui::Color32::TRANSPARENT))
                .min_size(min)
                .corner_radius(tokens::RADIUS_SM)
                .stroke(stroke)
                .fill(fill);

        let mut response = ui.add_enabled(self.enabled, widget);
        if !self.enabled
            && let Some(reason) = &self.disabled_reason
        {
            response = response.on_disabled_hover_text(reason);
        }

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
        }

        // AFTER the hover block, which covers the whole rect and would
        // erase a bar painted before it.
        if let Some(fraction) = self.progress {
            // A bar that jumps is a wrong instrument (§5).
            // `animate_value_with_time` is LINEAR in egui 0.34, which is
            // right here: progress is a measurement, not an arrival.
            let shown = ui.ctx().animate_value_with_time(
                id.with("progress"),
                fraction,
                tokens::MOTION_BASE,
            );
            let bar = egui::Rect::from_min_size(
                response.rect.left_bottom() - egui::vec2(0.0, tokens::SPACE_1),
                egui::vec2(response.rect.width() * shown, tokens::SPACE_1),
            );
            // INK_05, not ACCENT: the Primary fill IS ACCENT, so an ACCENT
            // bar would be invisible on the one variant that needs this.
            // INK_05 on ACCENT reads 6.22, so it needs no new token.
            ui.painter()
                .rect_filled(bar, egui::CornerRadius::ZERO, tokens::INK_05);
        }

        let text_color = if self.enabled {
            self.variant.text()
        } else {
            self.variant.disabled_text()
        };
        ui.painter().text(
            response.rect.center(),
            egui::Align2::CENTER_CENTER,
            &self.text,
            font_id,
            text_color,
        );

        response
    }
}

#[cfg(test)]
mod tests {
    use super::Variant;
    use crate::ui::tokens;

    const ALL: [Variant; 4] = [
        Variant::Primary,
        Variant::Default,
        Variant::Quiet,
        Variant::Danger,
    ];

    /// W2c: a disabled primary button looked enabled, because its blue fill
    /// stayed. The disabled fill is not the enabled fill and not the accent.
    #[test]
    fn a_disabled_primary_has_no_accent_fill_w2c() {
        let disabled = Variant::Primary.disabled_fill();
        assert_ne!(disabled, Variant::Primary.fill());
        assert_ne!(disabled, tokens::ACCENT);
        assert_ne!(disabled, tokens::ACCENT_PRESSED);
    }

    /// W2c: no variant keeps an accent or tint fill when it is disabled, and
    /// every variant gives its disabled text in `TEXT_FAINT`.
    #[test]
    fn every_disabled_variant_reads_as_disabled_w2c() {
        for variant in ALL {
            let fill = variant.disabled_fill();
            assert!(
                fill == tokens::SURFACE_RAISED || fill == egui::Color32::TRANSPARENT,
                "{variant:?} disabled fill {fill:?} is not a neutral fill"
            );
            assert_eq!(variant.disabled_text(), tokens::TEXT_FAINT, "{variant:?}");
            assert_ne!(variant.disabled_text(), variant.text(), "{variant:?}");
        }
    }
}
