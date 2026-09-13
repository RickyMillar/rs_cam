//! `CountPill` — the count/verdict chip, one renderer.
//!
//! Component layer (IA cleanup, Wave 1). The audit found count/verdict pills
//! rendered ad-hoc (`info_pill` in `sim_timeline.rs`, bare grid labels in the
//! inspector) with two rollups that could print different math (P4-001/002).
//! W0.4 already unified the *producer* (`ToolLoadReport::summary()`); this
//! unifies the *renderer*, so the verdict HUD and every other count surface
//! speak one grammar.
//!
//! Visual grammar (from `FINAL_DESIGN.md`, restyled in the 2026-06-11
//! density pass — V4 retired the `[ … ]`/`{ … }` punctuation-as-UI):
//! - `family` — `Verdict` (a pass/fail bucket: tinted fill + stroke) vs
//!   `Observation` (a neutral tally: tinted fill, no stroke), so an
//!   observation tally can never read as a load verdict (INS-003).
//! - `role` — `ReadOnly` (flat pill) vs `Actionable` (`… →` button that
//!   returns a clickable [`egui::Response`]).
//! - `denom` — renders the `/T` proof that two counts share one universe.
//! - `hide_when_zero` — zero-count chips self-hide instead of shipping
//!   wallpaper ("exceeding 0/7" says nothing "within 7/7" doesn't).
//!
//! Colour stays explicit: the verdict HUD legitimately carries five tones
//! (within=green, exceeds=red, unmodeled=amber, collisions=green/red,
//! traces=blue); folding them into the family would lose signal. Callers pass
//! the canonical `theme` colour.

use egui::Color32;

use crate::ui::{components::chip::Role, theme, tokens};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PillFamily {
    /// A pass/fail bucket — tinted fill + stroke.
    Verdict,
    /// A neutral tally — tinted fill only.
    Observation,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PillRole {
    /// Flat pill.
    ReadOnly,
    /// `… →` button; the returned response's `.clicked()` is the jump.
    Actionable,
}

pub struct CountPill<'a> {
    label: &'a str,
    count: usize,
    denom: Option<usize>,
    color: Color32,
    family: PillFamily,
    role: PillRole,
    hover: &'a str,
    hide_when_zero: bool,
    semantic: Option<Role>,
}

impl<'a> CountPill<'a> {
    /// A pass/fail bucket pill (tinted fill + stroke).
    pub fn verdict(label: &'a str, count: usize) -> Self {
        Self::new(label, count, PillFamily::Verdict)
    }

    /// A neutral tally pill (tinted fill only).
    pub fn observation(label: &'a str, count: usize) -> Self {
        Self::new(label, count, PillFamily::Observation)
    }

    fn new(label: &'a str, count: usize, family: PillFamily) -> Self {
        Self {
            label,
            count,
            denom: None,
            color: theme::TEXT_STRONG,
            family,
            role: PillRole::ReadOnly,
            hover: "",
            hide_when_zero: false,
            semantic: None,
        }
    }

    /// Bind the pill to a semantic role (ruling R15).
    ///
    /// With a role, the pill takes the role's `TINT_*` ground and text, and a
    /// `Verdict` additionally takes the role's glyph — so a verdict carries
    /// three channels and an `Observation` cannot borrow a verdict colour,
    /// which is §2.6 principle 1 applied to the pill.
    ///
    /// Without one the pill keeps its legacy caller-supplied colour. UP3
    /// migrates the call sites; UP2 must not touch a production panel.
    #[must_use]
    pub fn semantic(mut self, role: Role) -> Self {
        self.semantic = Some(role);
        self
    }

    /// Render the `/T` denominator (proof both counts share one universe).
    pub fn denom(mut self, total: usize) -> Self {
        self.denom = Some(total);
        self
    }

    pub fn color(mut self, color: Color32) -> Self {
        self.color = color;
        self
    }

    /// Render as a clickable `… →` button.
    pub fn actionable(mut self) -> Self {
        self.role = PillRole::Actionable;
        self
    }

    pub fn hover(mut self, text: &'a str) -> Self {
        self.hover = text;
        self
    }

    /// Self-hide at zero — render nothing instead of a zero-count chip.
    pub fn hide_when_zero(mut self) -> Self {
        self.hide_when_zero = true;
        self
    }
}

impl egui::Widget for CountPill<'_> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        if self.hide_when_zero && self.count == 0 {
            return ui.allocate_response(egui::Vec2::ZERO, egui::Sense::hover());
        }
        let count_text = match self.denom {
            Some(d) => format!("{}/{d}", self.count),
            None => self.count.to_string(),
        };
        let arrow = if matches!(self.role, PillRole::Actionable) {
            " \u{2192}"
        } else {
            ""
        };
        // Ruling R15: with a role, a Verdict leads with the role's glyph.
        let glyph = match (self.semantic, self.family) {
            (Some(role), PillFamily::Verdict) => role
                .glyph()
                .map(|g| format!("{g}\u{2009}"))
                .unwrap_or_default(),
            _ => String::new(),
        };
        let text = format!("{glyph}{} {count_text}{arrow}", self.label);

        // Ruling R15: an Observation never borrows a verdict colour.
        let fg = match (self.semantic, self.family) {
            (Some(role), PillFamily::Verdict) => role.text(),
            (Some(_), PillFamily::Observation) => tokens::TEXT_MUTED,
            (None, _) => self.color,
        };
        let rich = egui::RichText::new(text).small().color(fg);
        let resp = match self.role {
            PillRole::ReadOnly => {
                // Styled pill: tinted fill, verdicts additionally stroked.
                // Ruling R2 / R16: the stroke is the BORDER token, never a
                // colour synthesised with `linear_multiply`. A per-call blend
                // is a computed value the literal sentry cannot see.
                let stroke = match self.family {
                    PillFamily::Verdict => egui::Stroke::new(1.0_f32, tokens::BORDER),
                    PillFamily::Observation => egui::Stroke::NONE,
                };
                let fill = match (self.semantic, self.family) {
                    (Some(role), PillFamily::Verdict) => role.tint(),
                    (Some(_), PillFamily::Observation) => tokens::SURFACE_RAISED,
                    // Legacy path, until UP3 migrates the call sites.
                    (None, _) => self.color.linear_multiply(0.10),
                };
                egui::Frame::default()
                    .fill(fill)
                    .stroke(stroke)
                    // Ruling R16: on the grid. `corner_radius(6)` was neither
                    // RADIUS_SM nor RADIUS_MD, and `Margin::symmetric(5, 1)`
                    // was off the 4-point scale entirely.
                    .inner_margin(egui::Margin::symmetric(
                        tokens::SPACE_2 as i8,
                        tokens::SPACE_1 as i8,
                    ))
                    .corner_radius(tokens::RADIUS_SM)
                    .show(ui, |ui| {
                        ui.set_min_width(tokens::CHIP_MIN_WIDTH);
                        ui.label(rich)
                    })
                    .response
            }
            PillRole::Actionable => ui.add(egui::Button::new(rich).small()),
        };
        if self.hover.is_empty() {
            resp
        } else {
            resp.on_hover_text(self.hover)
        }
    }
}
