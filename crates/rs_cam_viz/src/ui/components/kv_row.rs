//! `KeyValueRow` (§4.6) and `DataTable` (§4.7).
//!
//! # The row is a FIXED line box
//!
//! `ROW_DENSE`, 22 points, set as a height rather than derived from
//! `item_spacing` plus a text ascent — because a derived height drifts with
//! the font and with any caption inside the row, and eight rows must occupy
//! exactly 176 points.
//!
//! The operator ruled on this after looking at the drawn specimen: at 26
//! points an inspector reads as a settings dialog, at 22 it reads as a
//! control panel.
//!
//! # Four slots
//!
//! `label · value · unit · trailing`. The label column takes **one width per
//! panel**, computed once from the longest label and held for every row,
//! which is what removes the two-indent defect (`AUDIT.md` D-15).
//!
//! # It does not call `end_row`
//!
//! Ruling R18. `ValueRow`, `PrecedenceField` and `CompareRow` all call
//! `ui.end_row()` themselves and so only work inside a `Grid`.
//! `KeyValueRow` is a standalone line and works in a plain vertical layout.

use crate::ui::{
    components::{motion, text},
    tokens,
};

/// A read-only label and value pair.
pub struct KeyValueRow {
    label: String,
    value: Value,
    unit: Option<String>,
    trailing: Option<String>,
    label_width: Option<f32>,
    editable_look: bool,
}

/// What sits in the value slot.
pub enum Value {
    /// A measured value. Renders in `Numeric` (§3.4).
    Measured(String),
    /// The product did not measure it. Renders as the em dash in `UNKNOWN`,
    /// never as `0` and never blank (§4.12).
    NotMeasured,
    /// Free text that is not a measurement, such as a name or a mode.
    Text(String),
}

impl KeyValueRow {
    pub fn new(label: impl Into<String>, value: Value) -> Self {
        Self {
            label: label.into(),
            value,
            unit: None,
            trailing: None,
            label_width: None,
            editable_look: false,
        }
    }

    /// A measured value with its unit.
    #[must_use]
    pub fn measured(label: impl Into<String>, value: impl Into<String>, unit: &str) -> Self {
        Self::new(label, Value::Measured(value.into())).unit(unit)
    }

    /// The product abstained. Never render this as a zero.
    #[must_use]
    pub fn not_measured(label: impl Into<String>) -> Self {
        Self::new(label, Value::NotMeasured)
    }

    /// The unit suffix. `Caption` in `TEXT_FAINT`, one `SPACE_1` after the
    /// number and never inside the `Numeric` run (§3.4).
    #[must_use]
    pub fn unit(mut self, unit: impl Into<String>) -> Self {
        self.unit = Some(unit.into());
        self
    }

    /// The fourth slot: a chip, a provenance stamp, a note.
    #[must_use]
    pub fn trailing(mut self, trailing: impl Into<String>) -> Self {
        self.trailing = Some(trailing.into());
        self
    }

    /// The panel's label column width. Compute it ONCE per panel from the
    /// longest label and pass the same value to every row.
    #[must_use]
    pub fn label_width(mut self, w: f32) -> Self {
        self.label_width = Some(w);
        self
    }

    /// Draw the value in a sunken well, the way an editable row does.
    ///
    /// §4.6: a read-only row and an editable row differ by the value slot,
    /// so the difference is visible AT THE VALUE, which is where the eye
    /// already is (`AUDIT.md` D-13).
    #[must_use]
    pub fn editable_look(mut self, editable: bool) -> Self {
        self.editable_look = editable;
        self
    }
}

impl egui::Widget for KeyValueRow {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        ui.scope(|ui| {
            ui.set_min_height(tokens::ROW_DENSE);
            ui.horizontal(|ui| {
                // Slot 1: the label, in its per-panel column.
                let label = egui::Label::new(
                    egui::RichText::new(&self.label)
                        .text_style(egui::TextStyle::Body)
                        .color(tokens::TEXT_MUTED),
                )
                .truncate();
                match self.label_width {
                    Some(w) => {
                        ui.allocate_ui(egui::vec2(w, tokens::ROW_DENSE), |ui| {
                            ui.add(label);
                        });
                    }
                    None => {
                        ui.add(label);
                    }
                }

                // Slot 2: the value.
                let draw_value = |ui: &mut egui::Ui| match &self.value {
                    Value::Measured(v) => {
                        ui.label(text::numeric(v.clone()));
                    }
                    Value::NotMeasured => {
                        ui.add(super::notice::NotMeasured::new());
                    }
                    Value::Text(v) => {
                        ui.label(
                            egui::RichText::new(v)
                                .text_style(egui::TextStyle::Body)
                                .color(tokens::TEXT_STRONG),
                        );
                    }
                };

                if self.editable_look {
                    egui::Frame::default()
                        .fill(tokens::INPUT_WELL)
                        .stroke(egui::Stroke::new(1.0, tokens::BORDER))
                        .corner_radius(tokens::RADIUS_SM)
                        .inner_margin(egui::Margin::symmetric(tokens::SPACE_2 as i8, 0))
                        .show(ui, |ui| {
                            // Ruling R7: a minimum width so a column of
                            // values with different digit counts keeps
                            // one right edge.
                            ui.set_min_width(tokens::WELL_MIN_WIDTH);
                            // §4.6: 18 points inside the 22-point row, so a
                            // column of wells keeps 2 points of air above and
                            // below and reads as a stack, not a list of boxes.
                            ui.set_min_height(tokens::WELL_HEIGHT);
                            ui.set_max_height(tokens::WELL_HEIGHT);
                            draw_value(ui);
                        });
                } else {
                    draw_value(ui);
                }

                // Slot 3: the unit.
                if let Some(unit) = &self.unit {
                    ui.add_space(tokens::SPACE_1);
                    ui.label(text::unit(unit.clone()));
                }

                // Slot 4: the trailing slot. It WRAPS rather than
                // clipping when the panel is narrow, which closes
                // `AUDIT.md` D-16 for value rows.
                if let Some(trailing) = &self.trailing {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add(
                            egui::Label::new(text::caption(trailing.clone()))
                                .wrap_mode(egui::TextWrapMode::Wrap),
                        );
                    });
                }
            });
        })
        .response
    }
}

/// A thin styling wrapper over `egui::Grid`.
///
/// The crate has 89 `Grid::new` sites, no `egui_extras` and no
/// `TableBuilder`, so this adds no dependency. It exists to give every table
/// one header treatment, one rule, one zebra and one row height.
pub struct DataTable {
    id_salt: String,
    headers: Vec<String>,
    striped: bool,
}

impl DataTable {
    pub fn new(id_salt: impl Into<String>, headers: Vec<String>) -> Self {
        Self {
            id_salt: id_salt.into(),
            headers,
            striped: true,
        }
    }

    #[must_use]
    pub fn striped(mut self, striped: bool) -> Self {
        self.striped = striped;
        self
    }

    /// Draw the table. The body closure draws rows and calls `ui.end_row()`
    /// for each, which is the `egui::Grid` contract (ruling R18: this type
    /// owns row termination for its own HEADER only).
    pub fn show<R>(
        self,
        ui: &mut egui::Ui,
        body: impl FnOnce(&mut egui::Ui) -> R,
    ) -> egui::InnerResponse<R> {
        let columns = self.headers.len().max(1);
        egui::Grid::new(self.id_salt)
            .num_columns(columns)
            .spacing([tokens::SPACE_3, tokens::SPACE_2])
            .min_row_height(tokens::ROW_DENSE)
            .striped(self.striped)
            .show(ui, |ui| {
                for header in &self.headers {
                    ui.label(text::subhead(header.clone()));
                }
                ui.end_row();
                body(ui)
            })
    }
}

/// Tint a whole table row on hover, 120 ms, decelerating (§4.7).
///
/// The tint covers the ROW, never a cell: a cell-level hover reads as a
/// selection the table does not support.
pub fn row_hover_tint(ui: &egui::Ui, rect: egui::Rect, id: egui::Id, hovered: bool) {
    let t = motion::fast(ui.ctx(), id, hovered);
    if t <= 0.0 {
        return;
    }
    let fill = motion::mix(
        egui::Color32::TRANSPARENT,
        tokens::hover_lift(tokens::SURFACE_RAISED),
        t,
    );
    ui.painter()
        .rect_filled(rect, egui::CornerRadius::from(tokens::RADIUS_SM), fill);
}
