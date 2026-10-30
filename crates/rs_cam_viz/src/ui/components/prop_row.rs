//! `PropRow` — the one field row of the inspector (W1, P9, design rule 4).
//!
//! One row shape:
//!
//! - the label in a column of `LABEL_COL_WIDTH`, in `TEXT_MUTED`. A long
//!   label truncates and the hover gives the full text;
//! - the control fills the rest of the row at `ROW_ACTION` height, in one
//!   well that the row draws;
//! - the unit is a suffix INSIDE the well, at its right edge, in
//!   `TEXT_FAINT`. The control does not draw the unit;
//! - an optional trailing quiet action ("Pick", "Reset", "Show");
//! - a field state: `Role::Caution` or `Role::Danger` draws the well
//!   outline in the role colour. Other roles draw the normal outline;
//! - a hover text: the full label, the explanation and, when a state is
//!   set, the problem detail.
//!
//! A read-only row ([`PropRow::show_read_only`]) has the same geometry with
//! no well; its state is a line under the value in the role colour.
//!
//! W3g: a numeric row ([`PropRow::numeric`]) has a well of
//! `NUMBER_WELL_WIDTH` at the start of the control column, not one across
//! the column. Its unit slot has the fixed `UNIT_SLOT_WIDTH`, so the
//! numbers of a column end at one x whatever the unit. A checkbox row
//! ([`PropRow::checkbox`]) draws the box alone at the start of the control
//! column, with no well.
//!
//! The row is a standalone line. It does not call `end_row`, so it does not
//! need an `egui::Grid`: the label column is a fixed allocation.
//!
//! # The compatibility path (W1 to W3)
//!
//! `ValueRow`, `OptionalValueRow` and `KeyValueRow` are the old field rows.
//! They stay, with no change to what they draw, until W3 moves their call
//! sites onto `PropRow`; W6 then deletes them. They use the same
//! `LABEL_COL_WIDTH`; their grid label cell ([`label_cell`]) is in this file.

use crate::ui::{
    components::{Button, Role},
    tokens,
};

/// The label cell of a field row: `LABEL_COL_WIDTH` wide and `height`
/// high, the text at its left edge.
///
/// The label never wraps and never breaks mid-word. A parameter label is
/// read as a name, not as prose, so when the column is tight it truncates
/// with an ellipsis and the hover carries the rest.
///
/// IC1: `add_sized` centred the label in the cell, so a `ValueRow` label sat
/// to the right of a plain `ui.label` in the same column and the column read
/// ragged. The cell lays the label out from its left edge.
pub(in crate::ui::components) fn label_cell_sized(
    ui: &mut egui::Ui,
    label: &str,
    height: f32,
) -> egui::Response {
    ui.allocate_ui_with_layout(
        egui::vec2(tokens::LABEL_COL_WIDTH, height),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.set_min_width(tokens::LABEL_COL_WIDTH);
            ui.add(egui::Label::new(label).truncate())
                .on_hover_text(label)
        },
    )
    .inner
}

/// The label cell of a grid row at `ROW_DENSE` (the compatibility rows).
pub fn label_cell(ui: &mut egui::Ui, label: &str) -> egui::Response {
    label_cell_sized(ui, label, tokens::ROW_DENSE)
}

/// W6: a value as text in a row control: the body size in
/// `TEXT_STRONG`, truncated. The read-only row and a count inside a well
/// (the Hand tabs row) paint a value with it.
pub fn well_value(ui: &mut egui::Ui, value: &str) -> egui::Response {
    ui.add(
        egui::Label::new(
            egui::RichText::new(value)
                .size(tokens::SIZE_BODY)
                .color(tokens::TEXT_STRONG),
        )
        .truncate(),
    )
}

/// The hover text of a field row. Pure, so the words are unit-testable.
///
/// The full label comes first, because the label can truncate. Then the
/// explanation. Then the problem detail, only when the row has a state.
#[must_use]
pub fn hover_text(
    label: &str,
    explanation: Option<&str>,
    state: Option<Role>,
    detail: Option<&str>,
) -> String {
    let mut out = label.trim().trim_end_matches(':').to_owned();
    if let Some(explanation) = explanation.filter(|e| !e.is_empty()) {
        out.push_str("\n\n");
        out.push_str(explanation);
    }
    if field_state(state).is_some()
        && let Some(detail) = detail.filter(|d| !d.is_empty())
    {
        out.push_str("\n\n");
        out.push_str(detail);
    }
    out
}

/// The state a field row draws: `Caution` and `Danger` only.
#[must_use]
pub fn field_state(state: Option<Role>) -> Option<Role> {
    state.filter(|role| matches!(role, Role::Caution | Role::Danger))
}

/// The outline colour of the well for a state.
#[must_use]
pub fn outline_color(state: Option<Role>) -> egui::Color32 {
    field_state(state).map_or(tokens::BORDER, Role::text)
}

/// The rectangles of one drawn row, for a caller and for the tests.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RowRects {
    /// The whole row.
    pub row: egui::Rect,
    /// The label column.
    pub label: egui::Rect,
    /// The well (the read-only row: the value area).
    pub well: egui::Rect,
    /// The area the control draws in.
    pub control: egui::Rect,
    /// The unit suffix, when the row has a unit.
    pub unit: Option<egui::Rect>,
    /// The trailing action, when the row has one.
    pub action: Option<egui::Rect>,
}

/// What [`PropRow::show`] returns.
pub struct PropRowResponse<R> {
    /// What the control closure returned.
    pub inner: R,
    /// The trailing action was clicked this frame.
    pub action_clicked: bool,
    /// The response of the whole row, with the hover text on it.
    pub response: egui::Response,
    /// The rectangles of the row.
    pub rects: RowRects,
    /// The hover text of the row. A [`PropRow::show`] caller puts it on its
    /// own control, because egui gives the hover to the top widget only.
    pub hover: String,
}

/// One field row. See the module doc.
#[must_use]
pub struct PropRow<'a> {
    label: &'a str,
    unit: Option<&'a str>,
    state: Option<Role>,
    explanation: Option<&'a str>,
    detail: Option<&'a str>,
    action: Option<&'a str>,
    action_hover: Option<&'a str>,
    numeric: bool,
    unit_slot: bool,
}

/// What the row draws around its control.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Frame {
    /// The editable well.
    Well,
    /// No well; a state is a line under the value (the read-only row).
    ReadOnly,
    /// No well and no line; the control draws its own state (a checkbox).
    Bare,
}

impl<'a> PropRow<'a> {
    pub fn new(label: &'a str) -> Self {
        Self {
            label,
            unit: None,
            state: None,
            explanation: None,
            detail: None,
            action: None,
            action_hover: None,
            numeric: false,
            unit_slot: true,
        }
    }

    /// W3g: a numeric row. The well is `NUMBER_WELL_WIDTH` wide at the start
    /// of the control column, and it keeps a unit slot of `UNIT_SLOT_WIDTH`
    /// at its right end, with or without a unit. A number that the control
    /// right-aligns then ends at the same x in every numeric row.
    pub fn numeric(mut self) -> Self {
        self.numeric = true;
        self
    }

    /// A numeric row whose control takes the unit slot too: an unset value
    /// that shows a rule text ("8 x D") has no unit, and the text needs
    /// the room.
    pub fn without_unit_slot(mut self) -> Self {
        self.unit = None;
        self.unit_slot = false;
        self
    }

    /// The hover of the trailing action.
    pub fn action_hover(mut self, hover: &'a str) -> Self {
        self.action_hover = Some(hover);
        self
    }

    /// The unit, drawn as a suffix inside the well.
    pub fn unit(mut self, unit: &'a str) -> Self {
        self.unit = Some(unit).filter(|u| !u.is_empty());
        self
    }

    /// The field state. `Caution` and `Danger` draw the outline in the role
    /// colour; any other role, or `None`, draws the normal outline.
    pub fn state(mut self, state: Option<Role>) -> Self {
        self.state = state;
        self
    }

    /// The explanation of the field, on the hover.
    pub fn hover(mut self, explanation: Option<&'a str>) -> Self {
        self.explanation = explanation;
        self
    }

    /// The problem detail, on the hover when the row has a state.
    pub fn detail(mut self, detail: Option<&'a str>) -> Self {
        self.detail = detail;
        self
    }

    /// A trailing quiet action, such as "Pick", "Reset" or "Show".
    pub fn action(mut self, label: &'a str) -> Self {
        self.action = Some(label);
        self
    }

    /// Draw an editable row. `control` draws in the control area; the row
    /// takes the visuals of the child widget off, so the well is the one
    /// frame. Use [`Self::show_widget`] for a widget that fills the area.
    pub fn show<R>(
        self,
        ui: &mut egui::Ui,
        control: impl FnOnce(&mut egui::Ui) -> R,
    ) -> PropRowResponse<R> {
        self.draw(ui, Frame::Well, |ui| {
            let visuals = ui.visuals_mut();
            for w in [
                &mut visuals.widgets.inactive,
                &mut visuals.widgets.hovered,
                &mut visuals.widgets.active,
                &mut visuals.widgets.open,
            ] {
                w.bg_fill = egui::Color32::TRANSPARENT;
                w.weak_bg_fill = egui::Color32::TRANSPARENT;
                w.bg_stroke = egui::Stroke::NONE;
            }
            visuals.text_edit_bg_color = Some(egui::Color32::TRANSPARENT);
            control(ui)
        })
    }

    /// Draw an editable row whose control is one widget that fills the
    /// control area.
    pub fn show_widget(
        self,
        ui: &mut egui::Ui,
        widget: impl egui::Widget,
    ) -> PropRowResponse<egui::Response> {
        let mut out = self.show(ui, |ui| ui.add_sized(ui.available_size(), widget));
        out.inner = out.inner.on_hover_text(&out.hover);
        out
    }

    /// Draw a read-only row: the same geometry, no well. A state draws a
    /// line under the value in the role colour. The value truncates; the
    /// hover of the value gives the full value, then the row hover.
    pub fn show_read_only(
        self,
        ui: &mut egui::Ui,
        value: impl Into<String>,
    ) -> PropRowResponse<egui::Response> {
        let value = value.into();
        let mut out = self.draw(ui, Frame::ReadOnly, |ui| well_value(ui, &value));
        let hover = format!("{value}\n\n{}", out.hover);
        out.inner = out.inner.on_hover_text(hover);
        out
    }

    /// W6: a note under a row (a hint, G-THROUGHCUT): one [`OneLine`] at
    /// the control column, so it lines up with the wells above it.
    ///
    /// [`OneLine`]: super::OneLine
    pub fn note(ui: &mut egui::Ui, note: super::OneLine) -> egui::Response {
        ui.horizontal(|ui| {
            ui.add_space(tokens::LABEL_COL_WIDTH + tokens::SPACE_3);
            ui.add(note)
        })
        .inner
    }

    /// W3g: draw an on/off row. The box sits alone at the start of the
    /// control column, with no well; `text` (empty for none) follows the
    /// box. A field state draws the outline of the box in the role colour.
    /// The box has the row hover.
    pub fn checkbox(
        self,
        ui: &mut egui::Ui,
        on: &mut bool,
        text: &str,
    ) -> PropRowResponse<egui::Response> {
        let state = field_state(self.state);
        let mut out = self.draw(ui, Frame::Bare, |ui| {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
            if text.is_empty() {
                ui.add(egui::Checkbox::without_text(on))
            } else {
                ui.checkbox(on, text)
            }
        });
        if let Some(role) = state {
            let side = ui.spacing().icon_width;
            let box_rect = egui::Rect::from_center_size(
                egui::pos2(
                    out.inner.rect.left() + side / 2.0,
                    out.inner.rect.center().y,
                ),
                egui::vec2(side, side),
            )
            .expand(1.0);
            ui.painter().rect_stroke(
                box_rect,
                egui::CornerRadius::from(tokens::RADIUS_SM),
                egui::Stroke::new(1.0, role.text()),
                egui::StrokeKind::Outside,
            );
        }
        out.inner = out.inner.on_hover_text(&out.hover);
        out
    }

    fn draw<R>(
        self,
        ui: &mut egui::Ui,
        frame: Frame,
        control: impl FnOnce(&mut egui::Ui) -> R,
    ) -> PropRowResponse<R> {
        let width = ui.available_width().max(tokens::LABEL_COL_WIDTH);
        let (row, response) =
            ui.allocate_exact_size(egui::vec2(width, tokens::ROW_ACTION), egui::Sense::hover());
        let rects = layout(
            ui,
            row,
            self.unit,
            self.action,
            self.numeric.then_some(self.unit_slot),
        );
        // egui gives the hover to the top widget only, so the hover goes on
        // the row, on the label and (in `show_widget`, `show_read_only`) on
        // the control.
        let hover = hover_text(self.label, self.explanation, self.state, self.detail);

        // The label column.
        let mut label_ui = child(ui, rects.label);
        let _ = label_ui
            .add(
                egui::Label::new(
                    egui::RichText::new(self.label)
                        .size(tokens::SIZE_BODY)
                        .color(tokens::TEXT_MUTED),
                )
                .truncate(),
            )
            .on_hover_text(&hover);

        // The well.
        if frame == Frame::Well {
            ui.painter().rect(
                rects.well,
                egui::CornerRadius::from(tokens::RADIUS_SM),
                tokens::INPUT_WELL,
                egui::Stroke::new(1.0, outline_color(self.state)),
                egui::StrokeKind::Inside,
            );
        } else if frame == Frame::ReadOnly
            && let Some(role) = field_state(self.state)
        {
            // A read-only row has no box; its state is a line under the
            // value, in the role colour.
            ui.painter().line_segment(
                [rects.well.left_bottom(), rects.well.right_bottom()],
                egui::Stroke::new(1.0, role.text()),
            );
        }

        // The unit, inside the well at its right edge.
        if let (Some(unit), Some(rect)) = (self.unit, rects.unit) {
            ui.painter().text(
                rect.left_center(),
                egui::Align2::LEFT_CENTER,
                unit,
                egui::FontId::proportional(tokens::SIZE_CAPTION),
                tokens::TEXT_FAINT,
            );
        }

        // The control.
        let mut control_ui = child(ui, rects.control);
        let inner = control(&mut control_ui);

        // The trailing action.
        let mut action_clicked = false;
        if let (Some(action), Some(rect)) = (self.action, rects.action) {
            let mut action_ui = child(ui, rect);
            let mut clicked = action_ui.add(Button::quiet(action));
            if let Some(hover) = self.action_hover {
                clicked = clicked.on_hover_text(hover);
            }
            action_clicked = clicked.clicked();
        }

        let response = response.on_hover_text(&hover);
        PropRowResponse {
            inner,
            action_clicked,
            response,
            rects,
            hover,
        }
    }
}

/// A child `Ui` in `rect`, left to right, centred on the row. A child does
/// not move the cursor of the parent; the row allocated the space once.
fn child(ui: &mut egui::Ui, rect: egui::Rect) -> egui::Ui {
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    child.set_clip_rect(rect.intersect(ui.clip_rect()));
    child
}

/// The rectangles of a row in `row`. Pure geometry from the tokens.
///
/// A numeric row (W3g) bounds the well to `NUMBER_WELL_WIDTH` and keeps the
/// unit slot of `UNIT_SLOT_WIDTH` whether it has a unit or not.
/// `numeric` is `None` for a row that is not numeric, else whether the row
/// keeps the unit slot.
fn layout(
    ui: &egui::Ui,
    row: egui::Rect,
    unit: Option<&str>,
    action: Option<&str>,
    numeric: Option<bool>,
) -> RowRects {
    let slot = numeric == Some(true);
    let numeric = numeric.is_some();
    let gap = tokens::SPACE_3;
    let label = egui::Rect::from_min_size(
        row.min,
        egui::vec2(tokens::LABEL_COL_WIDTH, tokens::ROW_ACTION),
    );
    let mut right = row.right();
    let action_rect = action.map(|text| {
        let font = ui
            .style()
            .text_styles
            .get(&egui::TextStyle::Button)
            .cloned()
            .unwrap_or_default();
        let w = text_width(ui, text, font) + 2.0 * ui.spacing().button_padding.x;
        let rect = egui::Rect::from_min_max(egui::pos2(right - w, row.top()), row.right_bottom());
        right = rect.left() - gap;
        rect
    });
    let well_left = label.right() + gap;
    let well_right = if numeric {
        right.min(well_left + tokens::NUMBER_WELL_WIDTH)
    } else {
        right
    };
    let well = egui::Rect::from_min_max(
        egui::pos2(well_left, row.top()),
        egui::pos2(well_right.max(well_left), row.bottom()),
    );
    let pad = tokens::SPACE_3;
    let mut control_right = well.right() - pad;
    // A rail too narrow for the whole numeric well (the 240-point rail)
    // gives the value the room first: the unit takes its own width, as in
    // a row that is not numeric.
    let slot = slot && well.width() + 0.5 >= tokens::NUMBER_WELL_WIDTH;
    let numeric = numeric && (slot || unit.is_none());
    // W3g: the fixed unit slot of a numeric row. The unit paints at the
    // left edge of the slot; the value ends at one x in every numeric row.
    let numeric_slot = slot.then(|| {
        let slot_w = tokens::UNIT_SLOT_WIDTH.min((control_right - well.left() - pad).max(0.0));
        let rect = egui::Rect::from_min_max(
            egui::pos2(control_right - slot_w, well.top()),
            egui::pos2(control_right, well.bottom()),
        );
        control_right = rect.left() - tokens::SPACE_2;
        rect
    });
    let unit_rect = if numeric {
        unit.and(numeric_slot)
    } else {
        unit.map(|u| {
            let w = text_width(ui, u, egui::FontId::proportional(tokens::SIZE_CAPTION));
            let rect = egui::Rect::from_min_max(
                egui::pos2(control_right - w, well.top()),
                egui::pos2(control_right, well.bottom()),
            );
            control_right = rect.left() - tokens::SPACE_2;
            rect
        })
    };
    let control_left = well.left() + pad;
    let control = egui::Rect::from_min_max(
        egui::pos2(control_left, well.top()),
        egui::pos2(control_right.max(control_left), well.bottom()),
    );
    RowRects {
        row,
        label,
        well,
        control,
        unit: unit_rect,
        action: action_rect,
    }
}

fn text_width(ui: &egui::Ui, text: &str, font: egui::FontId) -> f32 {
    ui.painter()
        .layout_no_wrap(text.to_owned(), font, tokens::TEXT_FAINT)
        .size()
        .x
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]
mod tests {
    use super::*;

    const LONG_LABEL: &str = "A label that is much too long for the label column";

    fn ctx() -> egui::Context {
        let ctx = egui::Context::default();
        tokens::apply(&ctx);
        tokens::apply_fonts(&ctx);
        let mut warmup = ctx.run_ui(egui::RawInput::default(), |_ui| {});
        warmup.textures_delta.clear();
        ctx
    }

    /// Draw `rows` at `width`; returns each row's rects and the painted
    /// text runs with their rectangles.
    fn draw(
        width: f32,
        mut rows: impl FnMut(&mut egui::Ui) -> Vec<RowRects>,
    ) -> (Vec<RowRects>, Vec<(String, egui::Rect, bool)>) {
        let ctx = ctx();
        let mut rects = Vec::new();
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            ui.set_max_width(width);
            rects = rows(ui);
        });
        let texts = out
            .shapes
            .iter()
            .filter_map(|c| match &c.shape {
                egui::epaint::Shape::Text(t) => Some((
                    t.galley.job.text.clone(),
                    c.shape.visual_bounding_rect(),
                    t.galley.elided,
                )),
                _ => None,
            })
            .collect();
        out.textures_delta.clear();
        (rects, texts)
    }

    fn all_kinds(ui: &mut egui::Ui) -> Vec<RowRects> {
        let mut v = 2.5_f64;
        let mut choice = 0_u8;
        vec![
            PropRow::new("Stepover")
                .unit("mm")
                .show_widget(ui, egui::DragValue::new(&mut v))
                .rects,
            PropRow::new("Cuts")
                .action("Pick")
                .show_read_only(ui, "3 loops, 2 islands")
                .rects,
            PropRow::new(LONG_LABEL)
                .state(Some(Role::Danger))
                .show(ui, |ui| {
                    egui::ComboBox::from_id_salt("c")
                        .selected_text("Centre")
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut choice, 0, "Centre");
                        });
                })
                .rects,
            PropRow::new("Offset")
                .unit("mm")
                .action("Reset")
                .show_widget(ui, egui::DragValue::new(&mut v))
                .rects,
        ]
    }

    /// Rule 4: every kind of row is ONE row of `ROW_ACTION`, and the rows
    /// stack with no overlap.
    #[test]
    fn every_row_is_one_row_action_high_w1() {
        let (rows, _) = draw(340.0, all_kinds);
        assert_eq!(rows.len(), 4);
        for r in &rows {
            assert!((r.row.height() - tokens::ROW_ACTION).abs() < 0.01, "{r:?}");
            assert!((r.well.height() - tokens::ROW_ACTION).abs() < 0.01);
        }
        for pair in rows.windows(2) {
            let [a, b] = pair else { continue };
            assert!(b.row.top() >= a.row.bottom() - 0.01, "{a:?} / {b:?}");
        }
    }

    /// Rule 4: one label width. The label column is `LABEL_COL_WIDTH` and
    /// every well starts at the same x.
    #[test]
    fn the_label_column_is_the_token_width_w1() {
        let (rows, _) = draw(340.0, all_kinds);
        let left = rows.first().unwrap().well.left();
        for r in &rows {
            assert!((r.label.width() - tokens::LABEL_COL_WIDTH).abs() < 0.01);
            assert!((r.well.left() - left).abs() < 0.01, "one well column");
            assert!(r.well.right() <= r.row.right() + 0.01);
        }
    }

    /// Rule 4: the unit is inside the well, right of the control, and it
    /// paints in the well at the right edge. The action is outside it.
    #[test]
    fn the_unit_is_an_inner_suffix_w1() {
        let (rows, texts) = draw(340.0, all_kinds);
        for r in rows.iter().filter(|r| r.unit.is_some()) {
            let unit = r.unit.unwrap();
            assert!(r.well.contains_rect(unit), "unit {unit:?} not in {r:?}");
            assert!(unit.left() >= r.control.right(), "unit after the control");
            if let Some(action) = r.action {
                assert!(action.left() >= r.well.right(), "action after the well");
            }
        }
        let mm: Vec<_> = texts.iter().filter(|(t, _, _)| t == "mm").collect();
        assert_eq!(mm.len(), 2, "both units painted: {texts:?}");
        for (_, rect, _) in mm {
            assert!(
                rows.iter().any(|r| r.well.expand(1.0).contains_rect(*rect)),
                "a unit painted outside its well: {rect:?}"
            );
        }
    }

    /// A long label truncates inside its column; the hover has the full
    /// text.
    #[test]
    fn a_long_label_truncates_and_the_hover_has_it_all_w1() {
        let (rows, texts) = draw(340.0, all_kinds);
        let long = rows.get(2).unwrap();
        let (painted, rect, elided) = texts
            .iter()
            .find(|(t, _, _)| t.starts_with("A label"))
            .unwrap();
        assert!(*elided, "not truncated: {painted}");
        assert!(rect.right() <= long.label.right() + 0.5, "{rect:?}");
        let hover = hover_text(LONG_LABEL, Some("Why"), None, None);
        assert!(hover.starts_with(LONG_LABEL), "{hover}");
        assert!(hover.contains("Why"));
    }

    /// The problem detail is on the hover only when the row has a state,
    /// and only `Caution` and `Danger` are states.
    #[test]
    fn the_detail_is_on_the_hover_with_a_state_w1() {
        let quiet = hover_text("Feed:", Some("The feed."), None, Some("Too fast."));
        assert_eq!(quiet, "Feed\n\nThe feed.");
        let info = hover_text("Feed", None, Some(Role::Info), Some("Too fast."));
        assert_eq!(info, "Feed");
        let caution = hover_text("Feed", None, Some(Role::Caution), Some("Too fast."));
        assert_eq!(caution, "Feed\n\nToo fast.");
        assert_eq!(outline_color(Some(Role::Danger)), tokens::DANGER);
        assert_eq!(outline_color(Some(Role::Caution)), tokens::CAUTION);
        assert_eq!(outline_color(Some(Role::Ok)), tokens::BORDER);
        assert_eq!(outline_color(None), tokens::BORDER);
    }

    /// W3g: a numeric well does not stretch across the control column. It
    /// is `NUMBER_WELL_WIDTH` wide, it starts at the control column, and
    /// the unit slot sits at its right end with or without a unit.
    #[test]
    fn a_numeric_well_is_bounded_w3g() {
        let (rows, _) = draw(340.0, |ui| {
            let mut v = 2.5_f64;
            vec![
                PropRow::new("Feed")
                    .numeric()
                    .unit("mm/min")
                    .show_widget(ui, egui::DragValue::new(&mut v))
                    .rects,
                PropRow::new("Count")
                    .numeric()
                    .show_widget(ui, egui::DragValue::new(&mut v))
                    .rects,
                PropRow::new("Pattern").show_read_only(ui, "Contour").rects,
            ]
        });
        let [feed, count, wide] = rows.as_slice() else {
            panic!("three rows")
        };
        assert!(
            (feed.well.width() - tokens::NUMBER_WELL_WIDTH).abs() < 0.01,
            "{feed:?}"
        );
        assert!(
            (feed.well.left() - wide.well.left()).abs() < 0.01,
            "one well column"
        );
        assert!(wide.well.width() > feed.well.width() + 20.0, "{wide:?}");
        // The control ends at the same x with and without a unit.
        assert!((feed.control.right() - count.control.right()).abs() < 0.01);
        let unit = feed.unit.unwrap();
        assert!(
            (unit.width() - tokens::UNIT_SLOT_WIDTH).abs() < 0.01,
            "{unit:?}"
        );
    }

    /// W3g: a checkbox row draws no well: no rectangle of the well fill in
    /// the row, and the box sits at the start of the control column.
    #[test]
    fn a_checkbox_row_has_no_well_w3g() {
        let ctx = ctx();
        let mut rects = None;
        let mut box_left = 0.0;
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            ui.set_max_width(340.0);
            let mut on = true;
            let row = PropRow::new("Climb").checkbox(ui, &mut on, "");
            box_left = row.inner.rect.left();
            rects = Some(row.rects);
        });
        let rects = rects.unwrap();
        let wells = out
            .shapes
            .iter()
            .filter(|c| matches!(&c.shape, egui::epaint::Shape::Rect(r) if r.fill == tokens::INPUT_WELL && r.rect.width() > 40.0))
            .count();
        out.textures_delta.clear();
        assert_eq!(wells, 0, "a checkbox row paints no well");
        assert!(
            (box_left - rects.control.left()).abs() < 1.0,
            "{box_left} vs {rects:?}"
        );
    }

    /// At the 240-point rail the row still fits: nothing paints past the
    /// row, and the control keeps a positive width.
    #[test]
    fn the_row_fits_the_narrow_rail_w1() {
        let (rows, texts) = draw(240.0, all_kinds);
        for r in &rows {
            assert!(r.row.right() <= 240.0 + 8.5, "{r:?}");
            assert!(r.control.width() > 0.0, "{r:?}");
        }
        for (t, rect, _) in &texts {
            assert!(
                rows.iter().any(|r| r.row.expand(0.5).contains_rect(*rect)),
                "{t:?} painted outside its row: {rect:?}"
            );
        }
    }
}
