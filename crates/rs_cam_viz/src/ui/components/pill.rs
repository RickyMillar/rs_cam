//! `CountPill` — the count/verdict chip, one renderer.
//!
//! Component layer (IA cleanup, Wave 1). The audit found count/verdict pills
//! rendered ad-hoc (`info_pill` in `sim_timeline.rs`, bare grid labels in the
//! inspector) with two rollups that could print different math (P4-001/002).
//! W0.4 already unified the *producer* (`ToolLoadReport::summary()`); this
//! unifies the *renderer*, so the verdict HUD and every other count surface
//! speak one grammar.
//!
//! Visual grammar (from `FINAL_DESIGN.md`):
//! - `family` — `Verdict` (a pass/fail bucket, `[ … ]`) vs `Observation`
//!   (a neutral tally, `{ … }`), so an observation tally can never read as a
//!   load verdict (INS-003).
//! - `role` — `ReadOnly` (flat label) vs `Actionable` (`( … → )`, a button
//!   that returns a clickable [`egui::Response`]).
//! - `denom` — renders the `/T` proof that two counts share one universe.
//!
//! Colour stays explicit: the verdict HUD legitimately carries five tones
//! (within=green, exceeds=red, unmodeled=amber, collisions=green/red,
//! traces=blue); folding them into the family would lose signal. Callers pass
//! the canonical `theme` colour.

use egui::Color32;

use crate::ui::theme;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PillFamily {
    /// A pass/fail bucket — bracketed `[ … ]`.
    Verdict,
    /// A neutral tally — braced `{ … }`.
    Observation,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PillRole {
    /// Flat label.
    ReadOnly,
    /// `( … → )` button; the returned response's `.clicked()` is the jump.
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
}

impl<'a> CountPill<'a> {
    /// A pass/fail bucket pill (`[ label N ]`).
    pub fn verdict(label: &'a str, count: usize) -> Self {
        Self::new(label, count, PillFamily::Verdict)
    }

    /// A neutral tally pill (`{ label N }`).
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
        }
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

    /// Render as a clickable `( … → )` button.
    pub fn actionable(mut self) -> Self {
        self.role = PillRole::Actionable;
        self
    }

    pub fn hover(mut self, text: &'a str) -> Self {
        self.hover = text;
        self
    }
}

impl egui::Widget for CountPill<'_> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let (open, close) = match (self.family, self.role) {
            (_, PillRole::Actionable) => ("(", ")"),
            (PillFamily::Verdict, PillRole::ReadOnly) => ("[", "]"),
            (PillFamily::Observation, PillRole::ReadOnly) => ("{", "}"),
        };
        let count_text = match self.denom {
            Some(d) => format!("{}/{d}", self.count),
            None => self.count.to_string(),
        };
        let arrow = if matches!(self.role, PillRole::Actionable) {
            " \u{2192}"
        } else {
            ""
        };
        let text = format!("{open} {} {count_text}{arrow} {close}", self.label);
        let rich = egui::RichText::new(text).small().color(self.color);
        let resp = match self.role {
            PillRole::ReadOnly => ui.label(rich),
            PillRole::Actionable => ui.add(egui::Button::new(rich).small()),
        };
        if self.hover.is_empty() {
            resp
        } else {
            resp.on_hover_text(self.hover)
        }
    }
}
