//! The Linking tab rows, and the parameter-grid helpers every operation
//! editor draws its rows with.
//!
//! `draw_linking_params` and `draw_dressup_params` both draw on the Linking
//! tab (R2: the Dressup tab is deleted), so they share one file.

use super::feeds_speeds::draw_entry_preview_diagram;
use super::operations::{draw_dogbone_diagram, draw_lead_in_out_diagram};
use super::{operations, pills};
use crate::state::toolpath::{
    ArcFitParams, DogboneParams, DressupConfig, DressupEntryStyle, LeadParams, LinkDressupParams,
    ToolpathEntry,
};
use crate::ui::automation;
use crate::ui::components::PropRow;
use crate::ui::components::UiExt as _;
use rs_cam_core::compute::catalog::OperationType;

// --- Parameter row helpers ---
//
// W3b: every helper below draws ONE `PropRow` (the label column, the
// control in one well, the unit inside the well, the field state outline).
// The rows stack; no helper needs an `egui::Grid`. Each helper takes the
// `Fields` of the toolpath as its LAST argument, so the ui04 scan still
// finds the `dv(ui, p(..))` call shape, and the row reads its field state
// from `placement::place`.

/// W3b: the field state of the op parameter rows of one toolpath.
///
/// It holds the findings that `placement::place` gives each field key
/// (`Placement::by_field`), and the G-DEPTHSTOCK caution of the depth row.
/// A row reads its state by its registry parameter name.
#[derive(Clone, Copy, Default)]
pub(super) struct Fields<'a> {
    by_field: Option<&'a super::placement::FieldFindings<'a>>,
    depth_caution: Option<(&'static str, &'a operations::DepthBeyondStock)>,
}

/// The state of one field row: the role of the worst finding, and the
/// hover text of the findings.
#[derive(Default)]
pub(super) struct FieldState {
    pub role: Option<crate::ui::components::Role>,
    pub detail: Option<String>,
}

impl<'a> Fields<'a> {
    /// The field states of one toolpath, from `Placement::by_field`.
    pub(super) fn new(by_field: &'a super::placement::FieldFindings<'a>) -> Self {
        Self {
            by_field: Some(by_field),
            depth_caution: None,
        }
    }

    /// The same states, with the G-DEPTHSTOCK caution on the row of `key`.
    /// The caution is the outline of the depth row, and its text is the
    /// hover. The issue stack prints the finding; the row prints no line.
    #[must_use]
    pub(super) fn with_depth_caution(
        self,
        key: &'static str,
        caution: Option<&'a operations::DepthBeyondStock>,
    ) -> Self {
        Self {
            depth_caution: caution.map(|c| (key, c)),
            ..self
        }
    }

    /// The state of the row of the field `key`.
    pub(super) fn state(self, key: &str) -> FieldState {
        super::placement::note_drawn(key);
        if let Some((caution_key, caution)) = self.depth_caution
            && caution_key == key
        {
            return FieldState {
                role: Some(crate::ui::components::Role::Caution),
                detail: Some(format!(
                    "{}\n{}",
                    caution.message(),
                    depth_caution_hover(caution)
                )),
            };
        }
        let Some(findings) = self.by_field.and_then(|m| m.get(key)) else {
            return FieldState::default();
        };
        FieldState {
            role: super::placement::tool_role(findings),
            detail: super::placement::tool_hover(findings),
        }
    }
}

/// The `PropRow` of one parameter: its label, its registry help on the
/// hover, and its field state.
pub(super) fn param_row<'a>(param: Param, state: &'a FieldState) -> PropRow<'a> {
    PropRow::new(param.label)
        .hover(help_for(param.op, param.name))
        .state(state.role)
        .detail(state.detail.as_deref())
}

/// The `PropRow` of a row that is not a registry parameter (the rest
/// analysis dials, a reference picker): a label and an explanation.
pub(super) fn plain_row<'a>(label: &'a str, explanation: Option<&'a str>) -> PropRow<'a> {
    PropRow::new(label).hover(explanation)
}

/// What a numeric row reports.
struct NumOut {
    /// The response of the value field, with the row hover.
    response: egui::Response,
    /// The response of the whole row.
    row: egui::Response,
    /// The ⚡ of the row wrote its value this frame.
    suggested: bool,
}

/// One numeric row: a `DragValue` in the bounded numeric well (W3g), the
/// unit in the unit slot, and a suggestion as the quiet action of the row:
/// "⚡ 0.83" (W3g, operator 2026-10-30: the cut half of the recommendation
/// sits on the field it changes).
///
/// G-PILLCLAMP: the ⚡ writes `Suggestion::recommended` verbatim. The row
/// shows no action when the value already matches it (`near_match`).
#[expect(
    clippy::too_many_arguments,
    reason = "the row, the value and its unit, speed and range, and the suggestion"
)]
fn num_row(
    ui: &mut egui::Ui,
    row: PropRow<'_>,
    label: &str,
    val: &mut f64,
    suffix: &str,
    speed: f64,
    range: std::ops::RangeInclusive<f64>,
    suggestion: Option<crate::ui::components::suggest::Suggestion<'_>>,
) -> NumOut {
    use crate::ui::components::number::{near_match, suggest_hover};
    use crate::ui::components::{NumberSpec, number_field};
    let spec = NumberSpec::new(speed).range(range);
    let offer = suggestion.filter(|s| !near_match(*val, s.recommended));
    let action = offer.as_ref().map(|s| suggest_action(s.recommended));
    let hover = offer
        .as_ref()
        .map(|s| suggest_hover(label, suffix, s, false));
    let mut row = row.numeric().unit(suffix.trim());
    if let (Some(action), Some(hover)) = (action.as_deref(), hover.as_deref()) {
        row = row.action(action).action_hover(hover);
    }
    let out = row.show(ui, |ui| number_field(ui, val, &spec, ui.available_size()));
    let suggested = out.action_clicked;
    if suggested && let Some(s) = &offer {
        *val = s.recommended;
    }
    let response = out.inner.on_hover_text(&out.hover);
    NumOut {
        response,
        row: out.response,
        suggested,
    }
}

/// The quiet action of a suggestion (W3g): "⚡ 0.83". A value of 100 or
/// more prints no decimals.
pub(super) fn suggest_action(recommended: f64) -> String {
    if recommended.abs() >= 100.0 {
        format!("\u{26A1} {recommended:.0}")
    } else {
        format!("\u{26A1} {recommended:.2}")
    }
}

/// One numeric row whose value can be not set (S11). A set value draws
/// the value field; with `clearable`, a "×" in the well sets it back to
/// `None`. An unset value draws an empty field with `hint`; a typed number
/// sets the value, clamped to the range. `true` when the value changed.
#[expect(
    clippy::too_many_arguments,
    reason = "the row, the value and its unit, speed and range, the hint and the clear"
)]
pub(super) fn opt_row(
    ui: &mut egui::Ui,
    row: PropRow<'_>,
    val: &mut Option<f64>,
    suffix: &str,
    speed: f64,
    range: std::ops::RangeInclusive<f64>,
    hint: &str,
    clearable: bool,
) -> bool {
    use crate::ui::components::{NumberSpec, optional_number_field};
    let mut changed = false;
    let spec = NumberSpec::new(speed).range(range).empty(hint);
    // W3g: an unset value with a rule shows the rule text ("8 x D",
    // "tool R", "machine max"). That text is not a number in the unit of
    // the field, so the row shows no unit until a value is set.
    // The rule text takes the unit slot too, so it fits the well.
    let rule_shown = val.is_none() && clearable;
    let mut row = row.numeric();
    row = if rule_shown {
        row.without_unit_slot()
    } else {
        row.unit(suffix.trim())
    };
    // A set value with a rule has the quiet action "×": it clears the
    // value back to the rule.
    let clear_hover = format!("Clear: {hint}");
    if clearable && val.is_some() {
        row = row.action("\u{00D7}").action_hover(&clear_hover);
    }
    let out = row.show(ui, |ui| {
        let (resp, edited) = optional_number_field(ui, val, &spec, ui.available_size());
        changed |= edited;
        resp
    });
    if out.action_clicked {
        *val = None;
        changed = true;
    }
    let _ = out.inner.on_hover_text(&out.hover);
    changed
}

/// One on/off parameter row: the box alone, no well (W3g).
pub(super) fn check_param(
    ui: &mut egui::Ui,
    param: Param,
    on: &mut bool,
    fields: Fields<'_>,
) -> egui::Response {
    let state = fields.state(param.name);
    check_in_row(ui, param_row(param, &state), on)
}

/// One on/off row of a `PropRow` the caller built ([`plain_row`]).
pub(super) fn check_in_row(ui: &mut egui::Ui, row: PropRow<'_>, on: &mut bool) -> egui::Response {
    row.checkbox(ui, on, "").inner
}

/// One dropdown parameter row. `menu` adds the entries.
pub(super) fn combo_param<R>(
    ui: &mut egui::Ui,
    param: Param,
    fields: Fields<'_>,
    id_salt: impl std::hash::Hash + std::fmt::Debug,
    selected: impl Into<egui::WidgetText>,
    menu: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::Response {
    let state = fields.state(param.name);
    combo_in_row(ui, param_row(param, &state), id_salt, selected, menu)
}

/// One dropdown row of a `PropRow` the caller built ([`plain_row`]).
pub(super) fn combo_in_row<R>(
    ui: &mut egui::Ui,
    row: PropRow<'_>,
    id_salt: impl std::hash::Hash + std::fmt::Debug,
    selected: impl Into<egui::WidgetText>,
    menu: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::Response {
    let out = row.show(ui, |ui| combo_control(ui, id_salt, selected, menu));
    out.inner.on_hover_text(&out.hover)
}

/// The dropdown control of a row: it fills the control area.
pub(super) fn combo_control<R>(
    ui: &mut egui::Ui,
    id_salt: impl std::hash::Hash + std::fmt::Debug,
    selected: impl Into<egui::WidgetText>,
    menu: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::Response {
    egui::ComboBox::from_id_salt(id_salt)
        .width(ui.available_width())
        .truncate()
        .selected_text(selected)
        .show_ui(ui, menu)
        .response
}

/// The `PropRow` of a parameter whose registry entry states no help: the
/// row names its own explanation, and it reads the field state of `key`.
pub(super) fn keyed_row<'a>(
    label: &'a str,
    explanation: &'a str,
    state: &'a FieldState,
) -> PropRow<'a> {
    PropRow::new(label)
        .hover(Some(explanation))
        .state(state.role)
        .detail(state.detail.as_deref())
}

/// One integer row of a `PropRow` the caller built ([`keyed_row`]). The
/// clamp range is [`schema_range`] of `op` and `name`.
pub(super) fn count_in_row(
    ui: &mut egui::Ui,
    row: PropRow<'_>,
    val: &mut usize,
    speed: f64,
    (op, name): (OperationType, &str),
) -> egui::Response {
    let spec = crate::ui::components::NumberSpec::new(speed).range(schema_range(op, name));
    row.number(ui, val, &spec).inner
}

/// One integer parameter row (UI-10: the field edits the `usize` itself,
/// so a partial edit commits the value the operator typed).
pub(super) fn count_param(
    ui: &mut egui::Ui,
    param: Param,
    val: &mut usize,
    speed: f64,
    fields: Fields<'_>,
) -> egui::Response {
    let state = fields.state(param.name);
    count_in_row(
        ui,
        param_row(param, &state),
        val,
        speed,
        (param.op, param.name),
    )
}

/// One numeric row of a `PropRow` the caller built ([`plain_row`]): a
/// value that is not a registry parameter.
pub(super) fn num_in_row(
    ui: &mut egui::Ui,
    row: PropRow<'_>,
    val: &mut f64,
    suffix: &str,
    speed: f64,
    range: std::ops::RangeInclusive<f64>,
) -> egui::Response {
    num_row(ui, row, "", val, suffix, speed, range, None).response
}

/// One parameter row's identity: which operation owns it, the registry name
/// the help text is keyed on, and the label the operator reads.
///
/// UI-04: the three used to be one string. The label WAS the key, so
/// re-wording it dropped the tooltip in silence. Holding them together
/// means a row states its key beside the words it shows.
#[derive(Clone, Copy)]
pub(super) struct Param {
    pub op: OperationType,
    pub name: &'static str,
    pub label: &'static str,
}

/// Name one parameter row. The short form the editors call.
pub(super) const fn p(op: OperationType, name: &'static str, label: &'static str) -> Param {
    Param { op, name, label }
}

/// The clamp range of one parameter row: the range the schema declares for
/// the parameter (W5).
///
/// The same `ParamRange` makes MCP `set_toolpath_param` refuse a value, so
/// the GUI and an agent accept one domain. A parameter with no declared
/// range gets an unbounded row.
pub(super) fn schema_range(op: OperationType, name: &str) -> std::ops::RangeInclusive<f64> {
    rs_cam_core::compute::catalog::OperationConfig::param_range_for_type(op, name)
        .map_or(f64::NEG_INFINITY..=f64::INFINITY, |range| {
            range.inclusive_bounds()
        })
}

/// One numeric parameter row. The clamp range is [`schema_range`].
pub(super) fn dv(
    ui: &mut egui::Ui,
    param: Param,
    val: &mut f64,
    suffix: &str,
    speed: f64,
    fields: Fields<'_>,
) {
    let state = fields.state(param.name);
    let out = num_row(
        ui,
        param_row(param, &state),
        param.label,
        val,
        suffix,
        speed,
        schema_range(param.op, param.name),
        None,
    );
    record_stock_to_leave(ui, param.name, &out);
}

/// S11: one numeric row whose value can be not set. An unset value shows
/// an empty field with `hint`: "set a value" (`not_set::SET_A_VALUE_HINT`)
/// for a value that no rule gives, else the rule text. A rule value
/// (`rule` true) has a "×" that clears it back to the rule.
pub(super) fn dv_opt(
    ui: &mut egui::Ui,
    param: Param,
    val: &mut Option<f64>,
    suffix: &str,
    speed: f64,
    hint: Option<&str>,
    fields: Fields<'_>,
) {
    let state = fields.state(param.name);
    let _ = opt_row(
        ui,
        param_row(param, &state),
        val,
        suffix,
        speed,
        schema_range(param.op, param.name),
        hint.unwrap_or(rs_cam_core::compute::not_set::SET_A_VALUE_HINT),
        hint.is_some(),
    );
}

/// [`dv`] for a row whose range a GUI rule narrows inside [`schema_range`].
/// The row clamps to the part of `narrow` that is inside the schema range.
///
/// One rule uses it: a cross-field rule. The Unified Finish waterline
/// threshold stays above the steep threshold. A single-field floor goes in
/// the schema, not here (operator ruling 2026-10-15).
pub(super) fn dv_narrowed(
    ui: &mut egui::Ui,
    param: Param,
    val: &mut f64,
    suffix: &str,
    speed: f64,
    narrow: std::ops::RangeInclusive<f64>,
    fields: Fields<'_>,
) {
    let schema = schema_range(param.op, param.name);
    let lo = narrow.start().max(*schema.start());
    let hi = narrow.end().min(*schema.end()).max(lo);
    let state = fields.state(param.name);
    let out = num_row(
        ui,
        param_row(param, &state),
        param.label,
        val,
        suffix,
        speed,
        lo..=hi,
        None,
    );
    record_stock_to_leave(ui, param.name, &out);
}

/// IC1: the ONE collapsed "Advanced" section of a tab.
///
/// It holds the parameters an operator rarely changes, so the tab opens on
/// the parameters that decide the cut. Every parameter in it stays
/// settable; only its default visibility changes. The section is collapsed
/// until the operator opens it, and egui keeps that choice per `id_salt`.
/// W3b: the rows are `PropRow`s that stack, so the section has no grid.
pub(super) fn advanced(ui: &mut egui::Ui, id_salt: &str, add: impl FnOnce(&mut egui::Ui)) {
    ui.add_space(crate::ui::tokens::SPACE_2);
    // A test that records the drawn rows opens the section, so its rows
    // draw (`placement::recording`).
    ui.disclosure(id_salt, "Advanced", super::placement::recording(), add);
}

/// The help line for one operation parameter, from the core registry.
///
/// UI-04: the key is the registry parameter NAME, not the visible label. The
/// old `tooltip_for` matched 60 arms against
/// `label.trim().trim_end_matches(':')`, so `"Stepover:"` reached its help
/// only because two spellings agreed by hand, and a label re-word dropped the
/// tooltip in silence. `help_for` cannot miss for a name the registry states.
pub(super) fn help_for(op: OperationType, param: &'static str) -> Option<&'static str> {
    op.registry_entry()
        .params
        .defs
        .iter()
        .find(|d| d.name == param)
        .and_then(|d| d.help)
}

/// The help line for one dressup field, from `DressupConfig::FIELD_DEFS`.
///
/// A dressup dial is not an operation parameter, so it reads the table CMP-17
/// made the one place the dressup vocabulary is written down.
pub(super) fn dressup_help(field: &'static str) -> Option<&'static str> {
    rs_cam_core::compute::config::DressupConfig::FIELD_DEFS
        .iter()
        .find(|d| d.name == field)
        .map(|d| d.description)
}

/// The `PropRow` of one dressup dial: its label, the help of the published
/// dressup field `field` on the hover (ui04), and the field state of
/// `field` in `fields`.
fn dressup_row<'a>(field: &'static str, label: &'a str, state: &'a FieldState) -> PropRow<'a> {
    PropRow::new(label)
        .hover(dressup_help(field))
        .state(state.role)
        .detail(state.detail.as_deref())
}

/// One numeric `PropRow` of a dressup dial, keyed on the published dressup
/// field name. The unit is inside the well.
#[expect(
    clippy::too_many_arguments,
    reason = "the field, the label, the value and its unit, speed and range, and the field states"
)]
pub(super) fn dv_dressup(
    ui: &mut egui::Ui,
    field: &'static str,
    label: &str,
    val: &mut f64,
    suffix: &str,
    speed: f64,
    range: std::ops::RangeInclusive<f64>,
    fields: Fields<'_>,
) {
    let state = fields.state(field);
    let _ = num_in_row(
        ui,
        dressup_row(field, label, &state),
        val,
        suffix,
        speed,
        range,
    );
}

/// S11: [`dv_dressup`] for a dressup value that a rule derives when it is
/// empty. `rule` is the hint of the empty field; a set value has a "×" that
/// clears it back to the rule.
#[expect(
    clippy::too_many_arguments,
    reason = "the arguments of `dv_dressup` and the rule text of the empty field"
)]
pub(super) fn dv_dressup_rule(
    ui: &mut egui::Ui,
    field: &'static str,
    label: &str,
    val: &mut Option<f64>,
    suffix: &str,
    speed: f64,
    range: std::ops::RangeInclusive<f64>,
    rule: &str,
    fields: Fields<'_>,
) {
    let state = fields.state(field);
    let _ = opt_row(
        ui,
        dressup_row(field, label, &state),
        val,
        suffix,
        speed,
        range,
        rule,
        true,
    );
}

/// P29-8 8b: the hint of an empty ramp angle field: the value of the rule
/// for the stock material and its source, "20° rule (Easel)". The card
/// names the material class.
pub(super) fn ramp_rule_hint(material: &rs_cam_core::material::Material) -> String {
    let rule = rs_cam_core::feeds::entry_rule::RampAngle::resolve(
        rs_cam_core::feeds::entry_rule::EntryAngle::Rule,
        material,
    );
    format!("{}° rule ({})", rule.deg, rule.source)
}

/// The hover of the G-DEPTHSTOCK caution.
///
/// F1.18: the last clause used to name the Bottom Z on the Heights tab as a
/// third thing to check. The rule no longer reads that field, and on every
/// operation this caution fires for the pin moves no motion at all (F1.19),
/// so naming it sent the operator to a dial that cannot fix this.
fn depth_caution_hover(caution: &operations::DepthBeyondStock) -> String {
    format!(
        "The cut floor sits {:.2} mm below the bottom of a {:.2} mm board, so the \
         tool cuts into the bed. Generate stays enabled. Check the operation's \
         Depth field, or the stock thickness on the Stock panel.",
        caution.excess_mm, caution.stock_thickness_mm
    )
}

/// G-THROUGHCUT (UX-R03-006): the through-cut line under the Profile Side
/// row.
///
/// Informational, in the panel's own text colour: a through cut is the
/// normal way to cut a part out, and zero tabs is a valid holding choice.
/// The hover names the consequence (the last pass frees the part) and the
/// holding options. Nothing is drawn for a partial-depth profile.
pub(super) fn through_cut_line(ui: &mut egui::Ui, through_cut: Option<&operations::ThroughCut>) {
    let Some(through_cut) = through_cut else {
        return;
    };
    let _ = PropRow::note(
        ui,
        crate::ui::components::OneLine::new(through_cut.message()).detail(format!(
            "The cut bottom reaches the bottom of the {:.2} mm board, so the last pass \
             frees the part. Tabs hold it in the sheet until you cut them; zero tabs \
             is valid when a vacuum table or double-sided tape holds the part.",
            through_cut.stock_thickness_mm
        )),
    );
}

/// The "Stock to Leave" UI-automation hook, shared by `dv`/`dv_pill`.
///
/// UI-04: the test keyed on the visible label, so re-wording the label broke
/// the hook with nothing failing. It now reads the registry parameter name.
/// Adaptive3d spells its field `stock_to_leave_axial` and every other op
/// spells it `stock_to_leave`; both carry the hook, exactly as the label
/// match did.
fn record_stock_to_leave(ui: &mut egui::Ui, param: &'static str, out: &NumOut) {
    if param.starts_with("stock_to_leave") {
        automation::record(
            ui,
            "properties_stock_to_leave",
            &out.response,
            "Stock to Leave",
        );
        automation::record(
            ui,
            "properties_stock_to_leave_label",
            &out.row,
            "Stock to Leave",
        );
    }
}

/// Same as [`dv`] (the clamp range is [`schema_range`]) but with an optional
/// ⚡ inside the field that writes the recommended value on click. The
/// suggestion is the one a [`pills::PillSuggestions`] built for this field;
/// the ⚡ takes the colour of its `ProvKind` source.
///
/// G-PILLCLAMP (2026-09-10): the suggestion is the apply funnel's as-applied
/// value for the field (or a labelled raw fallback when the funnel does not
/// write it), so the pill offers and writes the same number as `⚡ Apply cut
/// geometry`. A click is recorded on the `PillSuggestions` so the caller can
/// stamp the recommendation's provenance on the entry.
pub(super) fn dv_pill(
    ui: &mut egui::Ui,
    param: Param,
    val: &mut f64,
    suffix: &str,
    speed: f64,
    suggestion: Option<pills::PillSuggestion<'_>>,
    fields: Fields<'_>,
) -> bool {
    let (suggestion, click) = match suggestion.map(pills::PillSuggestion::into_parts) {
        Some((s, c)) => (Some(s), Some(c)),
        None => (None, None),
    };
    let state = fields.state(param.name);
    let out = num_row(
        ui,
        param_row(param, &state),
        param.label,
        val,
        suffix,
        speed,
        schema_range(param.op, param.name),
        suggestion,
    );
    record_stock_to_leave(ui, param.name, &out);
    if out.suggested
        && let Some(c) = &click
    {
        c.record();
    }
    out.suggested
}

/// S11: [`dv_pill`] for a value that can be not set. A set value draws with
/// its pill; an unset one draws the empty field of [`dv_opt`].
pub(super) fn dv_pill_opt(
    ui: &mut egui::Ui,
    param: Param,
    val: &mut Option<f64>,
    suffix: &str,
    speed: f64,
    suggestion: Option<pills::PillSuggestion<'_>>,
    fields: Fields<'_>,
) -> bool {
    match val.as_mut() {
        Some(value) => dv_pill(ui, param, value, suffix, speed, suggestion, fields),
        None => {
            dv_opt(ui, param, val, suffix, speed, None, fields);
            false
        }
    }
}

// ── Dressup configuration ────────────────────────────────────────────────

/// One section label of the Linking tab: the small-caps label of the kit.
/// Design rule 7: space groups the sections, so the label has no rule line
/// under it.
fn section(ui: &mut egui::Ui, title: &str) {
    crate::ui::components::SectionHeader::new(title)
        .small_caps()
        .show(ui);
}

/// One on/off row of the dressup field `key` ([`check_in_row`]), with the
/// field state of `key`. `disabled` greys the box and gives the reason on
/// the hover. Returns true when the operator changed the value.
fn check_row(
    ui: &mut egui::Ui,
    key: &str,
    label: &str,
    explanation: &str,
    on: &mut bool,
    disabled: Option<&str>,
    fields: Fields<'_>,
) -> bool {
    let state = fields.state(key);
    let row = keyed_row(label, explanation, &state);
    let resp = match disabled {
        Some(reason) => ui
            .add_enabled_ui(false, |ui| check_in_row(ui, row, on))
            .inner
            .on_disabled_hover_text(format!("{label}\n\n{explanation}\n\n{reason}")),
        None => check_in_row(ui, row, on),
    };
    resp.changed()
}

/// The hover of the Entry style row.
const ENTRY_STYLE_HOVER: &str = "How the tool enters the material. None plunges straight \
     down. Ramp descends at an angle and prevents plunge burns. Helix descends in a \
     spiral, for deep pockets and hard materials.";

/// The Entry style row: a `PropRow` with a dropdown. Its field state is the
/// state of the `entry_style` field in `fields`.
fn entry_style_row(ui: &mut egui::Ui, style: &mut DressupEntryStyle, fields: Fields<'_>) {
    let state = fields.state("entry_style");
    let selected = match *style {
        DressupEntryStyle::None => "None",
        DressupEntryStyle::Ramp => "Ramp",
        DressupEntryStyle::Helix => "Helix",
    };
    let _ = combo_in_row(
        ui,
        keyed_row("Entry style", ENTRY_STYLE_HOVER, &state),
        "dressup_entry",
        selected,
        |ui| {
            ui.selectable_value(style, DressupEntryStyle::None, "None")
                .on_hover_text("Plunge straight down. Fast but can burn in hard materials.");
            ui.selectable_value(style, DressupEntryStyle::Ramp, "Ramp")
                .on_hover_text("Angled descent into material. Prevents plunge burns. Recommended for most operations.");
            ui.selectable_value(style, DressupEntryStyle::Helix, "Helix")
                .on_hover_text("Spiral descent. Best for deep pockets and hard materials. Spreads heat and load evenly.");
        },
    );
}

/// The Linking tab (R2, W3e): the "Entry & exit" and "Links" sections.
/// Path quality is [`draw_dressup_params`]; the ONE Advanced section of the
/// tab is [`draw_linking_advanced`].
///
/// `fields` holds the field state of each row (`placement::place`).
pub(super) fn draw_linking_params(
    ui: &mut egui::Ui,
    entry: &mut ToolpathEntry,
    tool: Option<&rs_cam_core::compute::tool_config::ToolConfig>,
    toolpath_heights: Option<&rs_cam_core::session::ToolpathHeights>,
    material: &rs_cam_core::material::Material,
    fields: Fields<'_>,
) {
    // Some dressups are geometrically incompatible with specific operations
    // (project_curve traces many small rings; drop_cutter 3D finish emits
    // hundreds of raster segments — a lead-in or a link move at each one
    // produces diagonal trenches carving through the stock). Grey those
    // controls out so the UI reflects what compute actually does. Read from
    // the SAME registry policy `DressupConfig::normalize_for_op` applies.
    let row = entry.operation.op_type().registry_entry();
    let op_incompatible_msg: Option<&str> = row.lead_link_policy.strip_reason;
    // F5 (W3e): ONE predicate. K4: an op whose entry contract is not
    // `Dressup` (the strip-all ops, 3D Rough, Drill, Trace, Pin Drill)
    // ignores the dressup entry style. The row, its diagram and its
    // dependent rows then do not draw, so the tab never shows a second
    // Entry style control.
    let entry_applies = row.entry_style_applies();
    // The rapid-order pass reads the op's own capability (Face, 2D Adaptive
    // and 3D Rough refuse it: the planner stock depends on the cut order).
    let reorder_ok = entry
        .operation
        .capabilities()
        .transform
        .allows_rapid_reorder;
    // G10 Q6 and D2: the helix radius resolves against the tool (the rule
    // 0.3 x D, or the operator value, capped at the flat bottom).
    // F029: a tool outside its domain gives no rule radius.
    let cutter = tool.and_then(|t| rs_cam_core::compute::cutter::build_cutter(t).ok());
    let cfg = &mut entry.dressups;

    // ── Entry & exit ──────────────────────────────────────────
    section(ui, "Entry & exit");
    if entry_applies {
        // IC1: the entry diagram summarises the entry move, so it opens the
        // section, above the rows that set it.
        // P3.1: the diagram reads THE heights of the session; with no answer
        // it draws nothing rather than invented planes.
        if let Some(answer) = toolpath_heights {
            let helix_radius_mm = cutter.as_ref().map(|c| cfg.helix_radius_for(c).emitted_mm);
            let ramp_angle_deg =
                rs_cam_core::feeds::entry_rule::RampAngle::resolve(cfg.ramp_angle, material).deg;
            draw_entry_preview_diagram(
                ui,
                cfg,
                ramp_angle_deg,
                helix_radius_mm,
                answer,
                &entry.heights,
            );
            ui.add_space(4.0);
        }
        entry_style_row(ui, &mut cfg.entry_style, fields);
        match cfg.entry_style {
            DressupEntryStyle::Ramp => {
                // P29-8 8b: an empty field is the rule of the stock material
                // (Easel); the hint names its value and source, and the "×"
                // goes back to it. The range holds the 20° rule and the
                // 3D Rough schema range.
                let mut stated = cfg.ramp_angle.stated();
                dv_dressup_rule(
                    ui,
                    "ramp_angle",
                    "Max angle",
                    &mut stated,
                    " deg",
                    0.5,
                    0.5..=45.0,
                    &ramp_rule_hint(material),
                    fields,
                );
                cfg.ramp_angle = rs_cam_core::feeds::entry_rule::EntryAngle::from_stated(stated);
                dv_dressup(
                    ui,
                    "entry_clearance_mm",
                    "Clearance",
                    &mut cfg.entry_clearance_mm,
                    " mm",
                    0.1,
                    0.0..=5.0,
                    fields,
                );
            }
            DressupEntryStyle::Helix => {
                // G10 D2: `None` is the rule 0.3 x D; `Some` is an operator
                // value. Clearing the rule starts the value at the radius
                // the rule gives this tool.
                let resolved = cutter.as_ref().map(|c| cfg.helix_radius_for(c));
                let radius_range = 0.5..=20.0;
                let mut rule = cfg.helix_radius.is_none();
                let rule_label = format!(
                    "Radius {}",
                    rs_cam_core::feeds::entry_rule::helix_rule_text()
                );
                if check_row(
                    ui,
                    "helix_radius",
                    &rule_label,
                    "Repo rule, no source (G10): the helix radius is 0.3 x the tool \
                         diameter. Clear the box to set an operator value. Either way the \
                         engine caps the radius at the flat bottom, so the helix leaves no \
                         core.",
                    &mut rule,
                    None,
                    fields,
                ) {
                    cfg.helix_radius = if rule {
                        None
                    } else {
                        Some(resolved.map_or(*radius_range.start(), |h| h.requested_mm))
                    };
                }
                if let Some(radius) = cfg.helix_radius.as_mut() {
                    dv_dressup(
                        ui,
                        "helix_radius",
                        "Radius",
                        radius,
                        " mm",
                        0.1,
                        radius_range,
                        fields,
                    );
                }
                // The radius the engine emits: a derived value, so a
                // read-only row.
                if let Some(h) = resolved {
                    let value = if h.capped() {
                        format!(
                            "r {:.2} mm (capped from {:.2})",
                            h.emitted_mm, h.requested_mm
                        )
                    } else {
                        format!("r {:.2} mm", h.emitted_mm)
                    };
                    let _ = PropRow::new("Emits")
                        .hover(Some(
                            "The helix radius the engine emits. The no-core rule caps it \
                             at the flat bottom of the tool (geometry).",
                        ))
                        .show_read_only(ui, value);
                }
                dv_dressup(
                    ui,
                    "helix_pitch",
                    "Pitch",
                    &mut cfg.helix_pitch,
                    " mm",
                    0.1,
                    0.2..=10.0,
                    fields,
                );
                dv_dressup(
                    ui,
                    "entry_clearance_mm",
                    "Clearance",
                    &mut cfg.entry_clearance_mm,
                    " mm",
                    0.1,
                    0.0..=5.0,
                    fields,
                );
            }
            DressupEntryStyle::None => {}
        }
    }

    // CUT-13: the parameters live inside the `Option`. The box reads a
    // local bool and puts the module defaults back on a fresh enable.
    let mut lead_on = cfg.lead_in_out.is_some();
    if check_row(
        ui,
        "lead_in_out",
        "Lead-in / out",
        "Add smooth arc transitions at cut start and end. Prevents tool marks at \
             entry/exit points. Best for finishing and profile cuts.",
        &mut lead_on,
        op_incompatible_msg,
        fields,
    ) {
        cfg.lead_in_out = lead_on.then(LeadParams::default);
    }
    if op_incompatible_msg.is_none()
        && let Some(lead) = cfg.lead_in_out.as_mut()
    {
        // IC1: the diagram comes before the row it shows.
        draw_lead_in_out_diagram(ui, lead.radius);
        dv_dressup(
            ui,
            "lead_radius",
            "Radius",
            &mut lead.radius,
            " mm",
            0.1,
            0.5..=20.0,
            fields,
        );
    }

    // ── Links ─────────────────────────────────────────────────
    section(ui, "Links");
    let mut link_on = cfg.link_moves.is_some();
    if check_row(
        ui,
        "link_moves",
        "Link moves",
        "Keep the tool down: replace short retract-rapid-plunge sequences with slow \
             linear feeds. Major time saver for operations with many small regions.",
        &mut link_on,
        op_incompatible_msg,
        fields,
    ) {
        cfg.link_moves = link_on.then(LinkDressupParams::default);
    }
    if op_incompatible_msg.is_none()
        && let Some(link) = cfg.link_moves.as_mut()
    {
        // S11: empty = 8 x the tool diameter.
        dv_dressup_rule(
            ui,
            "link_max_distance",
            "Max distance",
            &mut link.max_distance,
            " mm",
            0.5,
            1.0..=50.0,
            "8 x D",
            fields,
        );
        dv_dressup(
            ui,
            "link_feed_rate",
            "Feed rate",
            &mut link.feed_rate,
            " mm/min",
            10.0,
            50.0..=5000.0,
            fields,
        );
    }
    let _ = check_row(
        ui,
        "optimize_rapid_order",
        "Reorder rapids",
        "Reorder disconnected toolpath segments to minimize total rapid travel distance \
             (TSP heuristic). Only operations whose cut order does not change what later \
             moves meet allow it.",
        &mut cfg.optimize_rapid_order,
        (!reorder_ok).then_some(
            "This operation keeps its planned cut order: its entries and keep-down links \
             are planned against the stock that earlier cuts leave, so a reorder would \
             feed into uncut stock.",
        ),
        fields,
    );
}

/// The ONE Advanced section of the Linking tab (IC1): the feed cap and the
/// feed-rate optimiser.
pub(super) fn draw_linking_advanced(
    ui: &mut egui::Ui,
    entry: &mut ToolpathEntry,
    fields: Fields<'_>,
) {
    let feed_opt_reason = crate::state::toolpath::feed_optimization_unavailable_reason(
        &entry.operation,
        entry.stock_source,
    );
    let cfg = &mut entry.dressups;
    advanced(ui, "linking_adv", |ui| {
        // G-FEEDMAX (C2, 2026-10-28): the max rate caps every feed stage,
        // the feed modulation after a simulation as well as the optimiser,
        // so it is one row whatever the optimisation box says. Its hover is
        // the core field description, which says so.
        // S11: empty = the machine maximum feed.
        dv_dressup_rule(
            ui,
            "feed_max_rate",
            "Max rate",
            &mut cfg.feed_max_rate,
            " mm/min",
            50.0,
            500.0..=20000.0,
            "machine max",
            fields,
        );
        // G-LINKFEEDOPT: viewing a tab commits nothing. A disabled box
        // reads a local copy; `dressup_apply` already skips the pass when
        // `feed_opt_stock` is `None`, so the stored flag is inert.
        let mut shown = false;
        let value = if feed_opt_reason.is_some() {
            &mut shown
        } else {
            &mut cfg.feed_optimization
        };
        let _ = check_row(
            ui,
            "feed_optimization",
            "Feed optimization",
            "Dynamically adjust feed rate based on stock engagement. Higher feed in \
             light cuts, lower in heavy cuts. Only available for fresh-stock 2D \
             operations.",
            value,
            feed_opt_reason,
            fields,
        );
        if cfg.feed_optimization && feed_opt_reason.is_none() {
            dv_dressup(
                ui,
                "feed_ramp_rate",
                "Ramp rate",
                &mut cfg.feed_ramp_rate,
                " mm/min/mm",
                10.0,
                10.0..=2000.0,
                fields,
            );
        }
    });
}

/// The "Path quality" section of the Linking tab (R2; the Dressup tab
/// before it): arc fitting and, where `dogbone_applies`, dogbone overcuts.
pub(super) fn draw_dressup_params(
    ui: &mut egui::Ui,
    cfg: &mut DressupConfig,
    dogbone_applies: bool,
    fields: Fields<'_>,
) {
    section(ui, "Path quality");

    let mut arc_on = cfg.arc_fitting.is_some();
    if check_row(
        ui,
        "arc_fitting",
        "Arc fitting",
        "Convert sequences of linear segments into smooth G2/G3 arcs. Reduces file \
             size, improves surface finish, and produces smoother machine motion. Safe for \
             all operations.",
        &mut arc_on,
        None,
        fields,
    ) {
        cfg.arc_fitting = arc_on.then(ArcFitParams::default);
    }
    if let Some(arc) = cfg.arc_fitting.as_mut() {
        dv_dressup(
            ui,
            "arc_tolerance",
            "Tolerance",
            &mut arc.tolerance,
            " mm",
            0.01,
            0.01..=0.5,
            fields,
        );
    }

    if !dogbone_applies {
        return;
    }
    let mut dogbone_on = cfg.dogbone.is_some();
    if check_row(
        ui,
        "dogbone",
        "Dogbone",
        "Add circular overcuts at inside corners so parts fit together. Essential for \
             joints, inlays, and press-fit assemblies. Not needed for open pockets or 3D \
             surfaces.",
        &mut dogbone_on,
        None,
        fields,
    ) {
        cfg.dogbone = dogbone_on.then(DogboneParams::default);
    }
    if let Some(dogbone) = cfg.dogbone.as_mut() {
        // IC1: the diagram comes before the row it shows.
        draw_dogbone_diagram(ui, dogbone.angle);
        dv_dressup(
            ui,
            "dogbone_angle",
            "Max angle",
            &mut dogbone.angle,
            " deg",
            1.0,
            45.0..=135.0,
            fields,
        );
    }
}
