//! The numeric controls of a field row (W3f): THE home of the raw
//! `egui::DragValue` of the inspector.
//!
//! A `PropRow` draws the label, the well, the unit and the state. The
//! control inside the well is one of these:
//!
//! - [`number_field`] — a value of any numeric type;
//! - [`optional_number_field`] — a value that can be not set (S11). An
//!   unset value shows the empty text of the spec; a typed number sets
//!   the value, clamped to the range;
//! - [`PropRow::number`] and [`PropRow::optional_number`] — a row whose
//!   control is only that field.
//!
//! A composite row (a height value and its reference dropdown) calls the
//! field with a width and an inner unit ([`NumberSpec::unit`]), so the
//! unit sits beside its value, before the next control.
//!
//! The Suggest helpers of a numeric row ([`near_match`], [`suggest_hover`])
//! are here too, so that `value_row.rs` can go (W6).

use super::suggest::Suggestion;
use super::{PropRow, PropRowResponse};
use crate::ui::tokens;

/// How a numeric field drags, clamps and prints.
#[derive(Clone, Debug)]
pub struct NumberSpec<'a> {
    speed: f64,
    range: Option<std::ops::RangeInclusive<f64>>,
    clamp_existing: bool,
    decimals: Option<usize>,
    empty: &'a str,
    unit: Option<&'a str>,
}

impl<'a> NumberSpec<'a> {
    /// A spec with the drag speed `speed`, no range, and the empty text
    /// "—".
    #[must_use]
    pub fn new(speed: f64) -> Self {
        Self {
            speed,
            range: None,
            clamp_existing: true,
            decimals: None,
            empty: tokens::GLYPH_UNKNOWN,
            unit: None,
        }
    }

    /// The range of the drag and of a typed value.
    #[must_use]
    pub fn range(mut self, range: std::ops::RangeInclusive<f64>) -> Self {
        self.range = Some(range);
        self
    }

    /// Keep a stored value outside the range as it is. The range then
    /// applies to a drag and to a typed value only.
    #[must_use]
    pub fn keep_out_of_range(mut self) -> Self {
        self.clamp_existing = false;
        self
    }

    /// Print the value with `decimals` fixed decimals.
    #[must_use]
    pub fn decimals(mut self, decimals: usize) -> Self {
        self.decimals = Some(decimals);
        self
    }

    /// The text of an unset value ([`optional_number_field`]).
    #[must_use]
    pub fn empty(mut self, empty: &'a str) -> Self {
        self.empty = empty;
        self
    }

    /// A unit that the field draws after its value, at the width of the
    /// unit. Use it in a composite row, where the next control follows the
    /// unit. A row with one field puts the unit on the `PropRow`
    /// ([`PropRow::unit`]).
    #[must_use]
    pub fn unit(mut self, unit: &'a str) -> Self {
        self.unit = Some(unit).filter(|u| !u.is_empty());
        self
    }

    fn drag<'v, N: egui::emath::Numeric>(&self, value: &'v mut N) -> egui::DragValue<'v> {
        let mut drag = egui::DragValue::new(value).speed(self.speed);
        if let Some(range) = &self.range {
            drag = drag.range(range.clone());
        }
        if !self.clamp_existing {
            drag = drag.clamp_existing_to_range(false);
        }
        if let Some(decimals) = self.decimals {
            drag = drag.fixed_decimals(decimals);
        }
        drag
    }

    fn clamp(&self, value: f64) -> f64 {
        self.range
            .as_ref()
            .map_or(value, |r| value.clamp(*r.start(), *r.end()))
    }
}

/// Draw `widget` in `size`, and the unit of `spec` after it. The unit adds
/// its own width. When the area is too narrow for both, the value gives up
/// width, so the unit never paints past the area.
fn sized_with_unit(
    ui: &mut egui::Ui,
    size: egui::Vec2,
    spec: &NumberSpec<'_>,
    widget: impl egui::Widget,
) -> egui::Response {
    let Some(unit) = spec.unit else {
        return add_right_aligned(ui, size, widget);
    };
    let font = egui::FontId::proportional(tokens::SIZE_CAPTION);
    let unit_w = ui
        .painter()
        .layout_no_wrap(unit.to_owned(), font.clone(), tokens::TEXT_FAINT)
        .size()
        .x;
    let gap = tokens::SPACE_2;
    // The value gives up width first, so the unit stays inside the area.
    let value_w = size.x.min((ui.available_width() - unit_w - gap).max(0.0));
    let response = add_right_aligned(ui, egui::vec2(value_w, size.y), widget);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(unit_w + gap, size.y), egui::Sense::hover());
    ui.painter().text(
        egui::pos2(rect.right(), rect.center().y),
        egui::Align2::RIGHT_CENTER,
        unit,
        font,
        tokens::TEXT_FAINT,
    );
    response
}

/// W3g: draw `widget` at the right edge of an area of `size`, at the width
/// of its text. A number then ends at the right edge of its area, so the
/// numbers of a column of rows end at one x (the digits of the body font
/// are tabular). The area is allocated in full.
fn add_right_aligned(
    ui: &mut egui::Ui,
    size: egui::Vec2,
    widget: impl egui::Widget,
) -> egui::Response {
    let out = ui.allocate_ui_with_layout(
        size,
        egui::Layout::right_to_left(egui::Align::Center),
        |ui| {
            ui.set_min_size(size);
            // The widget takes the width of its text, not the minimum
            // interact width, so a short value does not centre in it.
            ui.spacing_mut().interact_size.x = 0.0;
            ui.add(widget)
        },
    );
    let response = out.inner;
    // A click in the empty part of the area starts the edit of the value,
    // as a click on the value does.
    let rest = egui::Rect::from_min_max(
        out.response.rect.min,
        egui::pos2(response.rect.left(), out.response.rect.max.y),
    );
    if rest.width() > 0.0
        && ui
            .interact(rest, response.id.with("rest"), egui::Sense::click())
            .clicked()
    {
        ui.memory_mut(|m| m.request_focus(response.id));
    }
    response
}

/// The numeric field of a row: a `DragValue` of `size`. The response says
/// whether the value changed.
pub fn number_field<N: egui::emath::Numeric>(
    ui: &mut egui::Ui,
    value: &mut N,
    spec: &NumberSpec<'_>,
    size: egui::Vec2,
) -> egui::Response {
    let drag = spec.drag(value);
    sized_with_unit(ui, size, spec, drag)
}

/// The numeric field of a value that can be not set (S11).
///
/// A set value draws as [`number_field`]. An unset value draws the empty
/// text of `spec` ("—" when the text does not fit the field; the hover then
/// gives the text); the field works on a local NaN that is never stored, and
/// egui keeps the typed text in its own widget state, so the panel keeps no
/// draft (ui09). A typed number sets the value, clamped to the range.
/// The bool is true when the value changed.
pub fn optional_number_field(
    ui: &mut egui::Ui,
    value: &mut Option<f64>,
    spec: &NumberSpec<'_>,
    size: egui::Vec2,
) -> (egui::Response, bool) {
    if let Some(current) = *value {
        let mut v = current;
        let response = number_field(ui, &mut v, spec, size);
        let changed = response.changed();
        *value = Some(v);
        return (response, changed);
    }
    // An empty text that does not fit the field shows "—"; the hover then
    // gives the full text.
    let room = size.x.min(ui.available_width()) - 2.0 * ui.spacing().button_padding.x;
    let text_w = ui
        .painter()
        .layout_no_wrap(
            spec.empty.to_owned(),
            egui::TextStyle::Button.resolve(ui.style()),
            tokens::TEXT_FAINT,
        )
        .size()
        .x;
    let fits = text_w <= room;
    let empty = if fits {
        spec.empty.to_owned()
    } else {
        tokens::GLYPH_UNKNOWN.to_owned()
    };
    let decimals = spec.decimals;
    let mut local = f64::NAN;
    let drag = spec
        .drag(&mut local)
        .clamp_existing_to_range(false)
        .custom_formatter(move |v, _| {
            if v.is_nan() {
                empty.clone()
            } else if let Some(d) = decimals {
                format!("{v:.d$}")
            } else {
                format!("{v}")
            }
        })
        .custom_parser(|text| text.trim().parse::<f64>().ok());
    let mut response = sized_with_unit(ui, size, spec, drag);
    if !fits {
        response = response.on_hover_text(spec.empty);
    }
    let changed = local.is_finite();
    if changed {
        *value = Some(spec.clamp(local));
    }
    (response, changed)
}

impl PropRow<'_> {
    /// Draw a row whose control is one numeric field that fills the
    /// control area. The field has the row hover.
    pub fn number<N: egui::emath::Numeric>(
        self,
        ui: &mut egui::Ui,
        value: &mut N,
        spec: &NumberSpec<'_>,
    ) -> PropRowResponse<egui::Response> {
        let mut out = self
            .numeric()
            .show(ui, |ui| number_field(ui, value, spec, ui.available_size()));
        out.inner = out.inner.on_hover_text(&out.hover);
        out
    }

    /// Draw a row whose control is one field of a value that can be not set
    /// ([`optional_number_field`]). The bool is true when the value changed.
    pub fn optional_number(
        self,
        ui: &mut egui::Ui,
        value: &mut Option<f64>,
        spec: &NumberSpec<'_>,
    ) -> PropRowResponse<(egui::Response, bool)> {
        let mut out = self.numeric().show(ui, |ui| {
            optional_number_field(ui, value, spec, ui.available_size())
        });
        out.inner.0 = out.inner.0.on_hover_text(&out.hover);
        out
    }
}

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

#[cfg(test)]
#[expect(
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]
mod tests {
    use super::*;
    use crate::ui::components::ProvKind;

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

    fn ctx() -> egui::Context {
        let ctx = egui::Context::default();
        tokens::apply(&ctx);
        tokens::apply_fonts(&ctx);
        let mut warmup = ctx.run_ui(egui::RawInput::default(), |_ui| {});
        warmup.textures_delta.clear();
        ctx
    }

    /// The painted text runs of one frame, with their rectangles.
    #[expect(
        clippy::wildcard_enum_match_arm,
        reason = "test: foreign enum egui Shape; the arm skips every shape that this test does not read"
    )]
    fn painted(out: &egui::FullOutput) -> Vec<(String, egui::Rect)> {
        out.shapes
            .iter()
            .filter_map(|c| match &c.shape {
                egui::epaint::Shape::Text(t) => {
                    Some((t.galley.job.text.clone(), c.shape.visual_bounding_rect()))
                }
                _ => None,
            })
            .collect()
    }

    /// W3g (operator, 2026-10-30: "the units shift them off centre"): two
    /// numeric wells with the units "mm" and "mm/min" put the right edge of
    /// the number at the same x, and a short and a long number end at one
    /// x too.
    #[test]
    fn numbers_end_at_one_x_whatever_the_unit_w3g() {
        let ctx = ctx();
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            ui.set_max_width(340.0);
            let mut a = 1.5_f64;
            let mut b = 3999.0_f64;
            let spec = NumberSpec::new(1.0).decimals(1);
            let _ = PropRow::new("Depth/pass")
                .unit("mm")
                .number(ui, &mut a, &spec);
            let _ = PropRow::new("Feed")
                .unit("mm/min")
                .number(ui, &mut b, &spec);
        });
        let texts = painted(&out);
        out.textures_delta.clear();
        let right = |needle: &str| {
            texts
                .iter()
                .find(|(t, _)| t == needle)
                .map(|(_, r)| r.right())
                .unwrap_or_else(|| panic!("{needle} not painted: {texts:?}"))
        };
        let (short, long) = (right("1.5"), right("3999.0"));
        assert!(
            (short - long).abs() < 1.0,
            "the numbers end at {short} and {long}"
        );
        let (mm, mm_min) = (
            texts.iter().find(|(t, _)| t == "mm").map(|(_, r)| r.left()),
            texts
                .iter()
                .find(|(t, _)| t == "mm/min")
                .map(|(_, r)| r.left()),
        );
        assert!(
            mm.zip(mm_min).is_some_and(|(a, b)| (a - b).abs() < 1.0),
            "{texts:?}"
        );
    }

    /// W3g: the number tokens hold the widest text that they carry. The
    /// widths are measured with the inspector fonts, not typed.
    #[test]
    fn the_number_tokens_hold_the_widest_text_w3g() {
        let ctx = ctx();
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            let width = |text: &str, size: f32| {
                ui.painter()
                    .layout_no_wrap(
                        text.to_owned(),
                        egui::FontId::proportional(size),
                        tokens::TEXT_FAINT,
                    )
                    .size()
                    .x
            };
            let value = width("-8888.88", tokens::SIZE_BODY) + 2.0 * ui.spacing().button_padding.x;
            assert!(
                value <= tokens::NUMBER_VALUE_WIDTH,
                "the widest value is {value} points, the token {}",
                tokens::NUMBER_VALUE_WIDTH
            );
            for unit in ["mm", "mm/min", "RPM", "deg", "s", "1/mm", "\u{00D7} D", "%"] {
                let w = width(unit, tokens::SIZE_CAPTION);
                assert!(
                    w <= tokens::UNIT_SLOT_WIDTH,
                    "the unit {unit:?} is {w} points, the slot {}",
                    tokens::UNIT_SLOT_WIDTH
                );
            }
        });
        out.textures_delta.clear();
    }
}
