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

use super::provenance::{ProvKind, ProvenanceBadge};
use super::suggest::{SuggestButton, Suggestion};
use crate::ui::theme;

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
        }
    }

    /// Render a [`ProvenanceBadge`] after the input.
    pub fn prov(mut self, kind: ProvKind, reference: Option<&'a str>) -> Self {
        self.prov = Some((kind, reference));
        self
    }

    /// Render a single-field ⚡ that writes the recommendation on click.
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

    /// Draw the row and call `ui.end_row()` (grid-friendly).
    pub fn show(self, ui: &mut egui::Ui) -> ValueRowOutcome {
        let label_response = ui.label(self.label);
        let mut suggested = false;
        let mut value_response = None;
        ui.horizontal(|ui| {
            let mut resp = ui.add(
                egui::DragValue::new(self.value)
                    .suffix(self.suffix)
                    .speed(self.speed)
                    .range(self.range.clone()),
            );
            if let Some(tip) = self.tooltip {
                resp = resp.on_hover_text(tip);
            }

            if let Some(s) = self.suggest {
                let trimmed = self.label.trim().trim_end_matches(':');
                let near_match = s.recommended > 0.0
                    && (*self.value - s.recommended).abs() / s.recommended < 0.01;
                let src = s.source.label_with_reference(s.reference);
                let hover = if near_match {
                    format!(
                        "{trimmed} already matches LUT recommendation ({:.3}{}). Source: {src}.",
                        s.recommended, self.suffix,
                    )
                } else {
                    format!(
                        "Suggest {trimmed} = {:.3}{} (source: {src}). \
                         Click to overwrite this field only.",
                        s.recommended, self.suffix,
                    )
                };
                if ui
                    .add(
                        SuggestButton::field(s.source)
                            .enabled(!near_match)
                            .hover(hover),
                    )
                    .clicked()
                {
                    // Match apply_feeds_result_to_op's rounding so suggested
                    // values feel like suggestions, not measurements.
                    let step = if self.suffix.contains("mm/min") {
                        1.0
                    } else {
                        0.001
                    };
                    *self.value =
                        rs_cam_core::feeds::suggest::round_suggestion_value(s.recommended, step);
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
