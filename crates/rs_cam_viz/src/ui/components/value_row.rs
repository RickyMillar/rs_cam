//! `ValueRow` — the labelled numeric input row of a param grid.
//!
//! W1 (2026-10-30): a COMPATIBILITY row. `PropRow` (`prop_row.rs`) is the
//! one field row; W3 moves the call sites of `ValueRow` and
//! `OptionalValueRow` onto it, and W6 deletes this file.
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

use super::number::{near_match, suggest_hover};
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

/// The label column is shared with [`super::prop_row::PropRow`]; this
/// re-export keeps the callers of the compatibility rows unchanged.
pub use super::prop_row::label_cell;

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
    keep_existing: bool,
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
            keep_existing: false,
        }
    }

    /// Do not clamp a stored value that sits outside the range; clamp only
    /// what the operator drags or types. For a range that is a drag
    /// convenience and not a domain (the tool editor's
    /// `ToolDimension::ui_range`): a stored 200 mm tool stays 200 mm.
    pub fn keep_existing(mut self) -> Self {
        self.keep_existing = true;
        self
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
        let label_response = label_cell(ui, self.label);
        let mut suggested = false;
        let mut value_response = None;
        ui.horizontal(|ui| {
            // A FIXED value width, so a column of parameter rows keeps one
            // right edge whatever the digit count. Without it every row sized
            // its own DragValue and the column read as ragged — `AUDIT.md`
            // D-15, the two-indent defect, seen from the value side.
            let mut drag = egui::DragValue::new(self.value)
                .speed(self.speed)
                .range(self.range.clone())
                .clamp_existing_to_range(!self.keep_existing);
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

/// S11 (operator ruling 2026-10-23): one numeric row whose value can be
/// not set (`None`).
///
/// - A set value draws as a [`ValueRow`]. With [`Self::clearable`] a "×"
///   after the input sets the value back to `None`.
/// - A value that is not set draws an empty field with a hint: "set a
///   value" for a value that no rule gives, or the rule text (for example
///   "tool radius") for a value that a rule derives. A typed number sets the
///   value, clamped to the range.
pub struct OptionalValueRow<'a> {
    label: &'a str,
    value: &'a mut Option<f64>,
    suffix: &'a str,
    speed: f64,
    range: std::ops::RangeInclusive<f64>,
    hint: &'a str,
    tooltip: Option<&'a str>,
    clearable: bool,
    suggest: Option<Suggestion<'a>>,
}

impl<'a> OptionalValueRow<'a> {
    /// A row with the hint "set a value" (`not_set::SET_A_VALUE_HINT`).
    pub fn new(
        label: &'a str,
        value: &'a mut Option<f64>,
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
            hint: rs_cam_core::compute::not_set::SET_A_VALUE_HINT,
            tooltip: None,
            clearable: false,
            suggest: None,
        }
    }

    /// The hint of the empty field: the rule that gives the value.
    pub fn hint(mut self, hint: &'a str) -> Self {
        self.hint = hint;
        self
    }

    /// Explicit tooltip.
    pub fn tooltip(mut self, tip: Option<&'a str>) -> Self {
        self.tooltip = tip;
        self
    }

    /// Show a "×" that clears a set value (back to the rule).
    pub fn clearable(mut self) -> Self {
        self.clearable = true;
        self
    }

    /// The single-field ⚡ of a set value (see [`ValueRow::suggest`]).
    pub fn suggest(mut self, s: Option<Suggestion<'a>>) -> Self {
        self.suggest = s;
        self
    }

    /// Draw the row and call `ui.end_row()`. `true` when the value changed.
    pub fn show(self, ui: &mut egui::Ui) -> bool {
        if let Some(current) = *self.value {
            let mut v = current;
            if !self.clearable {
                let mut row =
                    ValueRow::new(self.label, &mut v, self.suffix, self.speed, self.range)
                        .tooltip(self.tooltip);
                if let Some(s) = self.suggest {
                    row = row.suggest(s);
                }
                let edited = row.show(ui).edited;
                *self.value = Some(v);
                return edited;
            }
            // A clearable value: the input and the "×" share the value cell.
            let _ = label_cell(ui, self.label);
            let mut changed = false;
            let mut cleared = false;
            ui.horizontal(|ui| {
                let mut resp = ui.add_sized(
                    [
                        crate::ui::tokens::WELL_MIN_WIDTH,
                        crate::ui::tokens::WELL_HEIGHT,
                    ],
                    egui::DragValue::new(&mut v)
                        .speed(self.speed)
                        .range(self.range.clone())
                        .suffix(self.suffix),
                );
                if let Some(tip) = self.tooltip {
                    resp = resp.on_hover_text(tip);
                }
                changed |= resp.changed();
                if ui
                    .small_button("\u{00D7}")
                    .on_hover_text(format!("Clear: {}", self.hint))
                    .clicked()
                {
                    cleared = true;
                }
            });
            ui.end_row();
            *self.value = if cleared { None } else { Some(v) };
            return changed || cleared;
        }
        // No value: a drag field that shows the hint. The field works on a
        // local NaN that is never stored; egui keeps the typed text of an
        // edit in its own widget state, so the panel keeps no draft. A
        // typed or dragged number sets the value.
        let _ = label_cell(ui, self.label);
        let hint = self.hint.to_owned();
        let mut local = f64::NAN;
        let mut changed = false;
        ui.horizontal(|ui| {
            let drag = egui::DragValue::new(&mut local)
                .speed(self.speed)
                .range(self.range.clone())
                .clamp_existing_to_range(false)
                .custom_formatter(move |v, _| {
                    if v.is_nan() {
                        hint.clone()
                    } else {
                        format!("{v}")
                    }
                })
                .custom_parser(|text| text.trim().parse::<f64>().ok());
            let resp = ui.add_sized(
                [
                    crate::ui::tokens::WELL_MIN_WIDTH,
                    crate::ui::tokens::WELL_HEIGHT,
                ],
                drag,
            );
            if let Some(tip) = self.tooltip {
                let _ = resp.on_hover_text(tip);
            }
            if local.is_finite() {
                *self.value = Some(local.clamp(*self.range.start(), *self.range.end()));
                changed = true;
            }
        });
        ui.end_row();
        changed
    }
}
