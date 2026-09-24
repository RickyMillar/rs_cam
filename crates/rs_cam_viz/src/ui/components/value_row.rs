//! `ValueRow` — the one labelled-numeric-input row.
//!
//! Component layer (IA cleanup, Wave 1). Supersedes the `dv` / `dv_pill`
//! free functions in `properties/mod.rs`. A `ValueRow` is `label : [DragValue]`
//! in a 2-column grid, with three opt-in extras rendered in the value cell:
//! an inline [`SuggestButton`] (the per-field ⚡), a [`ProvenanceBadge`]
//! (where the stored value came from), and a "· changes cut" marker for
//! geometry-affecting fields (DOC/WOC — the W3.1 SPEED/CUT split).
//!
//! Commit-1 callers (`dv`/`dv_pill`) opt into *only* the suggest pill, so the
//! refactor is behaviour-preserving; `prov()` / `changes_geometry()` light up
//! in the Wave-2 surface rewrites that render the honest per-field provenance.
//!
//! G-PILLCLAMP (UX-R03-014, 2026-09-10): the ⚡ pill writes
//! [`Suggestion::recommended`] **verbatim**. It used to re-round the value
//! here, and the value it was handed was the raw calculator output, so on the
//! demo pocket the Depth/Pass pill wrote 4.2 mm where `⚡ Apply cut geometry`
//! wrote the rigidity-capped 1.2 mm. The producer of a [`Suggestion`] is now
//! responsible for handing over the funnel's as-applied value
//! (`rs_cam_core::feeds::suggest::preview_field_applies`), and the near-match
//! test that greys the pill compares the configured value against THAT value,
//! so a pill never stays lit for a click that would change nothing.

use rs_cam_core::compute::tool_config::SizeUnits;
use rs_cam_core::tool::size_units;

use super::provenance::{ProvKind, ProvenanceBadge};
use super::suggest::{SuggestButton, Suggestion};
use crate::ui::theme;

/// Relative band inside which the configured value counts as already matching
/// the suggestion and the pill is greyed.
const NEAR_MATCH_BAND: f64 = 0.01;

/// Does `configured` already match the value the pill would write?
///
/// `recommended` must be the as-applied (funnel) value — see the module doc.
/// A non-positive recommendation never matches, so the pill stays live and
/// the operator can see the number it offers.
pub fn near_match(configured: f64, recommended: f64) -> bool {
    recommended > 0.0 && (configured - recommended).abs() / recommended < NEAR_MATCH_BAND
}

/// The hover text for a suggest pill. Pure so the wording is unit-testable.
pub fn suggest_hover(label: &str, suffix: &str, s: &Suggestion<'_>, matches: bool) -> String {
    let trimmed = label.trim().trim_end_matches(':');
    let src = s.source.label_with_reference(s.reference);
    let clamp_note = match s.calculator {
        Some(raw) if s.clamped && (raw - s.recommended).abs() > 1e-9 => {
            format!(" Calculator {raw:.3}{suffix}, clamped by the apply funnel.")
        }
        _ if !s.clamped => {
            " Calculator value, not clamped: the apply funnel does not write this field.".to_owned()
        }
        _ => String::new(),
    };
    if matches {
        format!(
            "{trimmed} already matches the recommendation as applied ({:.3}{}). Source: {src}.{clamp_note}",
            s.recommended, suffix,
        )
    } else {
        format!(
            "Suggest {trimmed} = {:.3}{} (source: {src}) \u{2014} the same value Apply writes. \
             Click to overwrite this field only.{clamp_note}",
            s.recommended, suffix,
        )
    }
}

/// Outcome of a [`ValueRow::show`]. Carries the egui responses so callers that
/// need them (e.g. UI-automation hooks) don't lose access through the wrapper.
pub struct ValueRowOutcome {
    /// The value changed via the DragValue this frame.
    pub edited: bool,
    /// The inline ⚡ suggest pill was clicked this frame (value already written).
    pub suggested: bool,
    pub value_response: egui::Response,
    pub label_response: egui::Response,
}

pub struct ValueRow<'a> {
    label: &'a str,
    value: &'a mut f64,
    suffix: &'a str,
    speed: f64,
    range: std::ops::RangeInclusive<f64>,
    prov: Option<(ProvKind, Option<&'a str>)>,
    suggest: Option<Suggestion<'a>>,
    tooltip: Option<&'a str>,
    changes_geometry: bool,
    note: Option<&'a str>,
    length_units: Option<SizeUnits>,
}

impl<'a> ValueRow<'a> {
    pub fn new(
        label: &'a str,
        value: &'a mut f64,
        suffix: &'a str,
        speed: f64,
        range: std::ops::RangeInclusive<f64>,
    ) -> Self {
        Self {
            label,
            value,
            suffix,
            speed,
            range,
            prov: None,
            suggest: None,
            tooltip: None,
            changes_geometry: false,
            note: None,
            length_units: None,
        }
    }

    /// Render a [`ProvenanceBadge`] after the input.
    pub fn prov(mut self, kind: ProvKind, reference: Option<&'a str>) -> Self {
        self.prov = Some((kind, reference));
        self
    }

    /// Render a single-field ⚡ that writes `s.recommended` on click.
    ///
    /// Hand this the funnel's as-applied value (see the module doc). The pill
    /// writes it verbatim.
    pub fn suggest(mut self, s: Suggestion<'a>) -> Self {
        self.suggest = Some(s);
        self
    }

    /// Explicit tooltip (overrides nothing; callers pass their own lookup).
    pub fn tooltip(mut self, tip: Option<&'a str>) -> Self {
        self.tooltip = tip;
        self
    }

    /// Append a "· changes cut" marker — for DOC/WOC under Feeds (W3.1).
    pub fn changes_geometry(mut self) -> Self {
        self.changes_geometry = true;
        self
    }

    /// Append read-only text after the input, for a value derived from
    /// this one (the tool editor's `= R1.00` beside a ball diameter).
    pub fn note(mut self, text: Option<&'a str>) -> Self {
        self.note = text;
        self
    }

    /// Show and accept a LENGTH in `units`. The stored value stays in mm;
    /// the field converts at the boundary. In inches it shows the nearest
    /// 1/64" fraction (`1/4"`) or decimal inches, and drops the mm suffix.
    /// In either unit it accepts a typed `1/4`, `1/4"`, `0.25in` or `6mm`
    /// (`rs_cam_core::tool::size_units::parse_length_entry`).
    pub fn length_units(mut self, units: SizeUnits) -> Self {
        self.length_units = Some(units);
        self
    }

    /// Draw the row and call `ui.end_row()` (grid-friendly).
    pub fn show(self, ui: &mut egui::Ui) -> ValueRowOutcome {
        // The label never wraps and never breaks mid-word. A parameter label
        // is read as a name, not as prose, so when the column is tight it
        // truncates with an ellipsis and the hover carries the rest.
        let label_response = ui
            .add_sized(
                [
                    crate::ui::tokens::LABEL_COL_WIDTH,
                    crate::ui::tokens::ROW_DENSE,
                ],
                egui::Label::new(self.label)
                    .truncate()
                    .wrap_mode(egui::TextWrapMode::Truncate)
                    .halign(egui::Align::LEFT),
            )
            .on_hover_text(self.label);
        let mut suggested = false;
        let mut value_response = None;
        ui.horizontal(|ui| {
            // A FIXED value width, so a column of parameter rows keeps one
            // right edge whatever the digit count. Without it every row sized
            // its own DragValue and the column read as ragged — `AUDIT.md`
            // D-15, the two-indent defect, seen from the value side.
            let mut drag = egui::DragValue::new(self.value)
                .speed(self.speed)
                .range(self.range.clone());
            drag = match self.length_units {
                Some(units @ SizeUnits::Imperial) => drag
                    .custom_formatter(move |mm, _| size_units::entry_text(mm, units))
                    .custom_parser(move |text| size_units::parse_length_entry(text, units)),
                Some(units @ SizeUnits::Metric) => drag
                    .suffix(self.suffix)
                    .custom_parser(move |text| size_units::parse_length_entry(text, units)),
                None => drag.suffix(self.suffix),
            };
            let mut resp = ui.add_sized(
                [
                    crate::ui::tokens::WELL_MIN_WIDTH,
                    crate::ui::tokens::WELL_HEIGHT,
                ],
                drag,
            );
            if let Some(tip) = self.tooltip {
                resp = resp.on_hover_text(tip);
            }

            if let Some(s) = self.suggest {
                // Compare against the value the click would WRITE. Pre-fix this
                // compared against the raw calculator value, so on the demo
                // pocket (configured 1.2, raw 4.2, as-applied 1.2) the pill
                // stayed lit for a no-op.
                let matches = near_match(*self.value, s.recommended);
                let hover = suggest_hover(self.label, self.suffix, &s, matches);
                if ui
                    .add(
                        SuggestButton::field(s.source)
                            .enabled(!matches)
                            .hover(hover),
                    )
                    .clicked()
                {
                    // Verbatim: the producer already ran the funnel (which
                    // rounds before it clamps), so re-rounding here would move
                    // the value off what Apply writes — the clamp output is
                    // not on the rounding grid (0.20 × 6.0 = 1.2000000000000002).
                    *self.value = s.recommended;
                    suggested = true;
                }
            }

            if let Some((kind, reference)) = self.prov {
                ui.add(
                    ProvenanceBadge::new(kind)
                        .reference_opt(reference)
                        .compact(),
                );
            }

            if let Some(note) = self.note {
                ui.label(egui::RichText::new(note).color(crate::ui::tokens::TEXT_FAINT));
            }

            if self.changes_geometry {
                ui.label(
                    egui::RichText::new("\u{00B7} changes cut")
                        .small()
                        .color(theme::TEXT_DIM),
                );
            }

            value_response = Some(resp);
        });
        ui.end_row();

        // SAFETY: the closure above always assigns `value_response`.
        let value_response = value_response.unwrap_or_else(|| label_response.clone());
        ValueRowOutcome {
            edited: value_response.changed() || suggested,
            suggested,
            value_response,
            label_response,
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn suggestion(recommended: f64, clamped: bool, calculator: Option<f64>) -> Suggestion<'static> {
        Suggestion {
            recommended,
            source: ProvKind::Formula,
            reference: None,
            clamped,
            calculator,
        }
    }

    /// The demo-pocket numbers (UX-R03-014): configured 1.2, raw 4.2, funnel
    /// 1.2. The pill must be greyed because the click would change nothing.
    #[test]
    fn near_match_reads_the_as_applied_value_not_the_raw_one() {
        let configured = 1.2;
        let as_applied = 1.200_000_000_000_000_2; // 0.20 × 6.0, the clamp output
        let raw = 4.2;
        assert!(near_match(configured, as_applied), "1.2 vs as-applied 1.2");
        assert!(
            !near_match(configured, raw),
            "1.2 vs raw 4.2 is not a match"
        );
    }

    #[test]
    fn near_match_never_fires_on_a_non_positive_recommendation() {
        assert!(!near_match(0.0, 0.0));
        assert!(!near_match(1.0, -1.0));
    }

    #[test]
    fn hover_names_the_clamp_when_the_calculator_value_differs() {
        let s = suggestion(1.2, true, Some(4.2));
        let text = suggest_hover("Depth/Pass:", " mm", &s, false);
        assert!(text.contains("1.200 mm"), "{text}");
        assert!(text.contains("Calculator 4.200 mm, clamped"), "{text}");
        assert!(text.contains("the same value Apply writes"), "{text}");
    }

    #[test]
    fn hover_says_not_clamped_when_the_funnel_does_not_write_the_field() {
        let s = suggestion(4.2, false, Some(4.2));
        let text = suggest_hover("Max Depth:", " mm", &s, false);
        assert!(text.contains("Calculator value, not clamped"), "{text}");
    }

    #[test]
    fn hover_is_quiet_when_calculator_and_funnel_agree() {
        let s = suggestion(2.1, true, Some(2.1));
        let text = suggest_hover("Stepover:", " mm", &s, false);
        assert!(!text.contains("Calculator"), "{text}");
        assert!(!text.contains("not clamped"), "{text}");
    }
}
