//! Current-vs-recommended comparison primitives, lifted out of `feeds_modal`.
//!
//! Component layer (IA cleanup, Wave 1). The audit found the compare row, the
//! delta tag, the power bar and the MRR row all implemented *private* inside
//! `feeds_modal.rs`, so the optimizer rollup (which wants the same
//! current-vs-recommended language) couldn't reuse them. This module is their
//! shared home: the Feeds Details drawer (W3.1) and the optimizer rollup (W3.5)
//! both draw from it.
//!
//! `power_bar` / `mrr_row` take raw numbers rather than a `FeedsExplain`, so
//! the module stays decoupled from feeds-modal-specific types — any surface
//! with a power fraction or an MRR can render the same widget.

use egui::Color32;

use crate::ui::theme;

/// Format an optional value with a unit and a precision hint (`—` when absent
/// or effectively zero). Shared by the compare grid and the project feeds table.
pub fn format_optional(v: Option<f64>, unit: &str, precision: f64) -> String {
    match v {
        Some(x) if x.is_finite() && x.abs() > 1e-12 => {
            if precision >= 1.0 {
                format!("{x:.0}{unit}")
            } else if precision >= 0.01 {
                format!("{x:.2}{unit}")
            } else {
                format!("{x:.4}{unit}")
            }
        }
        _ => "—".to_owned(),
    }
}

/// The `≈` / `↑ N×` / `↓ N×` ratio tag between a current and recommended value.
pub fn delta_tag(current: Option<f64>, recommended: Option<f64>) -> egui::RichText {
    let (Some(c), Some(r)) = (current, recommended) else {
        return egui::RichText::new("—").small().color(theme::TEXT_DIM);
    };
    if c.abs() < 1e-9 {
        return egui::RichText::new("—").small().color(theme::TEXT_DIM);
    }
    let ratio = r / c;
    let (text, color) = if (ratio - 1.0).abs() < 0.05 {
        ("\u{2248}".to_owned(), theme::TEXT_DIM)
    } else if ratio > 1.0 {
        (format!("\u{2191} {ratio:.2}\u{00D7}"), theme::SUCCESS)
    } else {
        (format!("\u{2193} {ratio:.2}\u{00D7}"), theme::WARNING_MILD)
    };
    egui::RichText::new(text).small().color(color)
}

/// A power gauge: `Power: [bar] X / Y kW (Z %)`, colour-ramped by load.
pub fn power_bar(ui: &mut egui::Ui, power_kw: f64, available_kw: f64) {
    let avail = available_kw.max(0.0001);
    let frac = (power_kw / avail).clamp(0.0, 1.0);
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Power:").small().color(theme::TEXT_DIM));
        let bar = egui::ProgressBar::new(frac as f32)
            .fill(power_color(frac))
            .desired_width(160.0);
        ui.add(bar);
        ui.label(
            egui::RichText::new(format!(
                "{power_kw:.2} / {avail:.2} kW ({:.0} %)",
                frac * 100.0
            ))
            .small(),
        );
    });
}

fn power_color(frac: f64) -> Color32 {
    if frac > 0.9 {
        theme::ERROR
    } else if frac > 0.7 {
        theme::WARNING_MILD
    } else {
        theme::SUCCESS
    }
}

/// A `MRR: N mm³/min` readout.
pub fn mrr_row(ui: &mut egui::Ui, mrr_mm3_min: f64) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("MRR:").small().color(theme::TEXT_DIM));
        ui.label(egui::RichText::new(format!("{mrr_mm3_min:.0} mm\u{00B3}/min")).small());
    });
}

/// One `label | current | recommended | Δ | ` row in a 5-column compare grid.
///
/// **Read-only** since Checkpoint I-1 (A-4, 2026-08-12). The trailing cell used
/// to hold a per-field `Apply` button that pushed `AppEvent::ApplyFeedsField`,
/// which wrote the raw preview value into the operation with neither the tool ×
/// operation validation nor `enforce_invariants` — on the shipped default
/// fixture that put **4.445 mm** of DOC where the funnel puts **1.27 mm**. The
/// buttons were deleted rather than rerouted; the trailing cell is now blank
/// and the row is a display primitive with no event surface at all, which is
/// why `show` no longer takes an event sink.
pub struct CompareRow<'a> {
    label: &'a str,
    current: Option<f64>,
    recommended: Option<f64>,
    unit: &'a str,
    precision: f64,
}

impl<'a> CompareRow<'a> {
    pub fn new(
        label: &'a str,
        current: Option<f64>,
        recommended: Option<f64>,
        unit: &'a str,
        precision: f64,
    ) -> Self {
        Self {
            label,
            current,
            recommended,
            unit,
            precision,
        }
    }

    pub fn show(self, ui: &mut egui::Ui) {
        ui.label(
            egui::RichText::new(self.label)
                .small()
                .color(theme::TEXT_DIM),
        );
        ui.label(format_optional(self.current, self.unit, self.precision));
        ui.label(format_optional(self.recommended, self.unit, self.precision));
        ui.label(delta_tag(self.current, self.recommended));
        ui.label("");
        ui.end_row();
    }
}
