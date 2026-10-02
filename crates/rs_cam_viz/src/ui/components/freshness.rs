//! `Freshness` / `FreshnessGate` — the single staleness cue.
//!
//! Component layer (IA cleanup, Wave 1). W0.5 added one shared
//! `theme::stale_banner` and wired it at the three fresh-on-stale readouts
//! (INS-005, OPT-003, TIM-009). This promotes that into the component layer:
//! [`FreshnessGate::banner`] is the single banner renderer (the old
//! `theme::stale_banner` body, now superseded), and [`FreshnessGate::show`]
//! wraps a body so a stale number is dimmed *and* banner-flagged — staleness
//! becomes a wrapper that can't be forgotten rather than a call discipline.

use crate::ui::theme;

/// Whether the sim that produced a number is stale.
#[derive(Clone, Copy)]
pub struct Freshness {
    pub stale: bool,
}

impl Freshness {
    pub fn new(stale: bool) -> Self {
        Self { stale }
    }

    pub fn fresh() -> Self {
        Self { stale: false }
    }
}

pub struct FreshnessGate {
    /// Why the simulation is stale (`SimFreshness::stale_reason`), or
    /// `None` when it is not stale.
    stale_reason: Option<String>,
}

impl FreshnessGate {
    pub fn new(stale_reason: Option<String>) -> Self {
        Self { stale_reason }
    }

    /// The single "results stale" banner — consulted at every concrete-metric
    /// readout so a stale number is never styled as fresh. Supersedes the
    /// `theme::stale_banner` free function.
    ///
    /// `reason` is `SimFreshness::stale_reason`. The banner used to read
    /// "params changed" for every cause, also for a machine import.
    pub fn banner(ui: &mut egui::Ui, reason: &str) {
        ui.label(
            egui::RichText::new(banner_text(reason))
                .small()
                .color(theme::WARNING),
        );
    }

    /// Render `body`, prefixing the stale banner and dimming the body when
    /// stale. Use this to wrap concrete-metric readouts.
    pub fn show(self, ui: &mut egui::Ui, body: impl FnOnce(&mut egui::Ui)) {
        if let Some(reason) = self.stale_reason.as_deref() {
            Self::banner(ui, reason);
            ui.scope(|ui| {
                ui.set_opacity(0.5);
                body(ui);
            });
        } else {
            body(ui);
        }
    }
}

/// The text of [`FreshnessGate::banner`] for one stale reason.
pub fn banner_text(reason: &str) -> String {
    format!("\u{26A0} Results stale ({reason}) \u{2014} run the simulation again")
}
