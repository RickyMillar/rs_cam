//! `DataTable` and `Details` — the one renderer for tabular data.
//!
//! Inspector clean-up round 2 (2026-10-13). The operator: "there is very
//! little consistency in how we display data like this. recommendation and
//! this show completely different ui. its all just tabular data."
//!
//! # The rule
//!
//! Tabular data uses `DataTable`. Derivation and provenance go in `Details`.
//!
//! A table is a list of rows in fixed columns:
//!
//! `name · value · status · source`
//!
//! - The name column has one width for the whole table, so the values align.
//! - Every row is one line of `ROW_DENSE` height. No cell wraps; a long cell
//!   truncates and the row hover gives the full text.
//! - A row with a hover shows the detail mark after its name. The hover
//!   covers the whole row.
//! - The status slot is the role vocabulary of `chip::Role`: a glyph, a word
//!   and the role tint.
//! - The source slot is a provenance badge (`ProvKind`) or the name of a
//!   setting.
//!
//! Four row kinds share the columns:
//!
//! - a value (`DataRow::value`, `DataRow::measured`, `DataRow::not_measured`);
//!   a value that is not measured reads "N/A", with no chip after it, and its
//!   reason is on the row hover (round 3);
//! - a change, current → recommended and the ratio (`DataRow::change`);
//! - a bar from 0 to a limit (`DataRow::bar`);
//! - a record, one full-width line for a sentence that core writes
//!   (`DataRow::record`) or for a status line (`DataRow::note`).
//!
//! `Details` is the one disclosure for derivation. It is closed by default,
//! and every section uses the same label.

use crate::ui::components::{ProvKind, ProvenanceBadge};
use crate::ui::components::{Role, kv_row::row_hover_tint};
use crate::ui::tokens;

/// The text a not-measured value cell shows (inspector clean-up round 3).
///
/// Operator, 2026-10-13: "instead of that not measurable can you just
/// replace the -- with 'N/A' and remove the extra tag on the right".
pub const NOT_MEASURED_TEXT: &str = "N/A";

/// The share of the row width that the name column takes, by default.
const NAME_SHARE: f32 = 0.42;

/// The largest share of the space after the name that the status and the
/// source slots can take together.
const TRAILING_SHARE: f32 = 0.55;

/// The label of every `Details` disclosure.
pub const DETAILS_LABEL: &str = "Details";

/// What the value column of a row holds.
enum Cell {
    /// A value, with an optional unit and an optional colour.
    Value {
        text: String,
        unit: Option<String>,
        tone: egui::Color32,
    },
    /// The product did not measure it: "N/A", never a zero.
    NotMeasured,
    /// The current value, an arrow, the recommended value and the ratio tag.
    Change {
        current: String,
        recommended: String,
        recommended_tone: egui::Color32,
        delta: egui::RichText,
    },
    /// A bar from 0 to a limit, with its text on the bar.
    Bar {
        fraction: f32,
        text: String,
        fill: egui::Color32,
    },
    /// One line across the whole row: the name holds the text.
    Record { tone: egui::Color32 },
}

/// Where a value comes from: the fourth column.
pub enum Source {
    /// A provenance badge: the glyph, with the label on hover.
    Prov(ProvKind, Option<String>),
    /// The setting that a limit traces back to, as a short dim word.
    Setting(String),
}

/// One row of a [`DataTable`].
#[must_use]
pub struct DataRow {
    name: String,
    cell: Cell,
    status: Option<(Role, String)>,
    source: Option<Source>,
    hover: Option<String>,
    clickable: bool,
}

impl DataRow {
    fn new(name: impl Into<String>, cell: Cell) -> Self {
        Self {
            name: name.into(),
            cell,
            status: None,
            source: None,
            hover: None,
            clickable: false,
        }
    }

    /// A value in the strong text colour. Put the unit in the text or in
    /// [`Self::unit`].
    pub fn value(name: impl Into<String>, text: impl Into<String>) -> Self {
        Self::new(
            name,
            Cell::Value {
                text: text.into(),
                unit: None,
                tone: tokens::TEXT_STRONG,
            },
        )
    }

    /// A measured value and its unit.
    pub fn measured(name: impl Into<String>, value: impl Into<String>, unit: &str) -> Self {
        Self::value(name, value).unit(unit)
    }

    /// The product abstained. The value column shows "N/A". The row has no
    /// status or source slot: a status word set on it moves to the first
    /// line of the row hover.
    pub fn not_measured(name: impl Into<String>) -> Self {
        Self::new(name, Cell::NotMeasured)
    }

    /// Current → recommended, and the ratio tag of the two.
    pub fn change(
        name: impl Into<String>,
        current: impl Into<String>,
        recommended: impl Into<String>,
        delta: egui::RichText,
    ) -> Self {
        Self::new(
            name,
            Cell::Change {
                current: current.into(),
                recommended: recommended.into(),
                recommended_tone: tokens::TEXT_STRONG,
                delta,
            },
        )
    }

    /// A bar from 0 to a limit. `fraction` is clamped to the bar; `text` is
    /// not, so a value past its limit reads past its limit.
    pub fn bar(
        name: impl Into<String>,
        fraction: f32,
        text: impl Into<String>,
        fill: egui::Color32,
    ) -> Self {
        Self::new(
            name,
            Cell::Bar {
                fraction: fraction.clamp(0.0, 1.0),
                text: text.into(),
                fill,
            },
        )
    }

    /// One full-width line: a sentence that core writes, kept in one piece.
    /// The row hover gives the full text and the detail.
    pub fn record(text: impl Into<String>) -> Self {
        Self::new(
            text,
            Cell::Record {
                tone: tokens::TEXT_MUTED,
            },
        )
    }

    /// One full-width status line in the colour of its role. Use one for
    /// each problem, and only for a problem.
    pub fn note(role: Role, text: impl Into<String>) -> Self {
        Self::new(text, Cell::Record { tone: role.text() })
    }

    /// The unit after the value, in the faint unit colour.
    pub fn unit(mut self, unit: impl Into<String>) -> Self {
        if let Cell::Value { unit: slot, .. } = &mut self.cell {
            let unit = unit.into();
            *slot = (!unit.is_empty()).then_some(unit);
        }
        self
    }

    /// The colour of the value, or of a record line.
    pub fn tone(mut self, color: egui::Color32) -> Self {
        match &mut self.cell {
            Cell::Value { tone, .. } | Cell::Record { tone } => *tone = color,
            Cell::Change {
                recommended_tone, ..
            } => *recommended_tone = color,
            Cell::NotMeasured | Cell::Bar { .. } => {}
        }
        self
    }

    /// The status slot: a role and a short lower-case word or phrase.
    ///
    /// A not-measured row has no status slot. Its word becomes the first
    /// line of the row hover, so the reason stays one hover away.
    pub fn status(mut self, role: Role, word: impl Into<String>) -> Self {
        let word = word.into();
        if matches!(self.cell, Cell::NotMeasured) {
            self.hover = Some(match self.hover.take() {
                Some(hover) => format!("{}\n{hover}", capitalised(&word)),
                None => capitalised(&word),
            });
            return self;
        }
        self.status = Some((role, word));
        self
    }

    /// The source slot. A not-measured row has none.
    pub fn source(mut self, source: Source) -> Self {
        if !matches!(self.cell, Cell::NotMeasured) {
            self.source = Some(source);
        }
        self
    }

    /// The row hover: the explanation and the provenance. On a not-measured
    /// row it follows the status word, when one is set.
    pub fn hover(mut self, hover: impl Into<String>) -> Self {
        let hover = hover.into();
        self.hover = Some(match (self.hover.take(), &self.cell) {
            (Some(status), Cell::NotMeasured) => format!("{status}\n{hover}"),
            _ => hover,
        });
        self
    }

    /// The row hover, when there is one.
    pub fn hover_opt(mut self, hover: Option<impl Into<String>>) -> Self {
        match hover {
            Some(hover) => self.hover(hover),
            None => {
                if !matches!(self.cell, Cell::NotMeasured) {
                    self.hover = None;
                }
                self
            }
        }
    }

    /// Append a line to the row hover: the full status, which the status
    /// slot may truncate.
    pub fn hover_suffix(mut self, status: &str) -> Self {
        let line = format!("Status: {status}.");
        self.hover = Some(match self.hover.take() {
            Some(hover) => format!("{hover}\n\n{line}"),
            None => line,
        });
        self
    }

    /// The row answers a click; [`DataTableResponse::clicked`] names it.
    pub fn clickable(mut self) -> Self {
        self.clickable = true;
        self
    }

    /// The text the name cell paints: the name, then the detail mark when
    /// the row has a hover.
    fn name_text(&self) -> String {
        match &self.hover {
            None => self.name.clone(),
            Some(_) => format!("{} {}", self.name, tokens::GLYPH_DETAIL),
        }
    }
}

/// What a table reports after it draws.
#[derive(Debug, Default, Clone, Copy)]
pub struct DataTableResponse {
    /// The index of the clickable row that was clicked.
    pub clicked: Option<usize>,
    /// The index of the row under the pointer.
    pub hovered: Option<usize>,
}

/// The one table of name and value rows.
#[must_use]
pub struct DataTable {
    id: egui::Id,
    name_share: f32,
    rows: Vec<DataRow>,
}

impl DataTable {
    /// An empty table. `id_salt` must be unique in its panel.
    pub fn new(id_salt: impl std::hash::Hash + std::fmt::Debug) -> Self {
        Self {
            id: egui::Id::new(id_salt),
            name_share: NAME_SHARE,
            rows: Vec::new(),
        }
    }

    /// The share of the row width the name column takes. A table of change
    /// rows needs a narrow name column; a table of values a wider one.
    pub fn name_share(mut self, share: f32) -> Self {
        self.name_share = share.clamp(0.15, 0.7);
        self
    }

    /// Add one row.
    pub fn row(mut self, row: DataRow) -> Self {
        self.rows.push(row);
        self
    }

    /// Add rows.
    pub fn rows(mut self, rows: impl IntoIterator<Item = DataRow>) -> Self {
        self.rows.extend(rows);
        self
    }

    /// Add one row when there is one.
    pub fn row_opt(mut self, row: Option<DataRow>) -> Self {
        self.rows.extend(row);
        self
    }

    /// The number of rows.
    #[must_use]
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// True when the table has no row.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Draw the rows. An empty table draws nothing.
    pub fn show(self, ui: &mut egui::Ui) -> DataTableResponse {
        let mut out = DataTableResponse::default();
        let width = ui.available_width();
        let name_width = (width * self.name_share).floor();
        let saved_spacing = ui.spacing().item_spacing.y;
        ui.spacing_mut().item_spacing.y = 0.0;
        for (index, row) in self.rows.into_iter().enumerate() {
            let response = show_row(ui, row, self.id.with(index), width, name_width);
            if response.clicked {
                out.clicked = Some(index);
            }
            if response.hovered {
                out.hovered = Some(index);
            }
        }
        ui.spacing_mut().item_spacing.y = saved_spacing;
        out
    }
}

/// What one row reports.
struct RowResponse {
    clicked: bool,
    hovered: bool,
}

/// A caption-size run in `color`.
fn caption(text: impl Into<String>, color: egui::Color32) -> egui::RichText {
    egui::RichText::new(text.into())
        .size(tokens::SIZE_CAPTION)
        .color(color)
}

/// The painted width of `text` on one line.
fn text_width(ui: &egui::Ui, text: egui::RichText) -> f32 {
    egui::WidgetText::from(text)
        .into_galley(
            ui,
            Some(egui::TextWrapMode::Extend),
            f32::INFINITY,
            egui::TextStyle::Body,
        )
        .size()
        .x
}

/// One cell of `width` that truncates its label. When the row has a hover,
/// the label shows no tooltip of its own, so the row shows one hover.
fn cell(ui: &mut egui::Ui, width: f32, text: egui::RichText, row_hover: bool) {
    let width = width.max(0.0);
    ui.allocate_ui_with_layout(
        egui::vec2(width, tokens::ROW_DENSE),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.set_width(width);
            ui.add(
                egui::Label::new(text)
                    .truncate()
                    .show_tooltip_when_elided(!row_hover),
            );
        },
    );
}

/// `text` with its first letter in upper case: a hover line starts with a
/// capital.
fn capitalised(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// The status slot: the role glyph and the word on the role tint.
fn status_text(role: Role, word: &str) -> String {
    match role.glyph() {
        Some(glyph) => format!("{glyph} {word}"),
        None => word.to_owned(),
    }
}

/// The painted width of the status and the source slots.
fn trailing_width(ui: &egui::Ui, row: &DataRow) -> f32 {
    let mut width = 0.0;
    if let Some((role, word)) = &row.status {
        width += text_width(ui, caption(status_text(*role, word), role.text()))
            + 2.0 * tokens::SPACE_2
            + tokens::SPACE_1;
    }
    match &row.source {
        Some(Source::Prov(kind, _)) => {
            width += text_width(ui, caption(kind.glyph(), kind.color())) + tokens::SPACE_1;
        }
        Some(Source::Setting(setting)) => {
            width += text_width(ui, caption(setting.clone(), tokens::TEXT_FAINT)) + tokens::SPACE_1;
        }
        None => {}
    }
    width
}

fn show_row(
    ui: &mut egui::Ui,
    row: DataRow,
    id: egui::Id,
    width: f32,
    name_width: f32,
) -> RowResponse {
    let sense = if row.clickable {
        egui::Sense::click()
    } else {
        egui::Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, tokens::ROW_DENSE), sense);
    row_hover_tint(ui, rect, id, response.hovered());
    let has_hover = row.hover.is_some();
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .id_salt(id)
            .max_rect(rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    child.spacing_mut().item_spacing.x = tokens::SPACE_1;
    paint_row(&mut child, &row, width, name_width, has_hover);

    let response = if row.clickable {
        response.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        response
    };
    let hovered = response.hovered();
    let clicked = response.clicked();
    if let Some(hover) = row.hover {
        // A record truncates its face, so its hover starts with the full
        // text, then the detail.
        let hover = match row.cell {
            Cell::Record { .. } => format!("{}\n{hover}", row.name),
            _ => hover,
        };
        response.on_hover_text(hover);
    }
    RowResponse { clicked, hovered }
}

fn paint_row(ui: &mut egui::Ui, row: &DataRow, width: f32, name_width: f32, has_hover: bool) {
    let name = row.name_text();
    if let Cell::Record { tone } = &row.cell {
        let trailing = trailing_width(ui, row);
        cell(ui, width - trailing, caption(name, *tone), has_hover);
        paint_trailing(ui, row);
        return;
    }
    cell(ui, name_width, caption(name, tokens::TEXT_MUTED), has_hover);
    let rest = (width - name_width - tokens::SPACE_1).max(0.0);
    let trailing = trailing_width(ui, row).min(rest * TRAILING_SHARE);
    let value_width = (rest - trailing).max(0.0);
    match &row.cell {
        Cell::Value { text, unit, tone } => {
            let mut job = egui::text::LayoutJob::default();
            caption(text.clone(), *tone).append_to(
                &mut job,
                ui.style(),
                egui::FontSelection::Default,
                egui::Align::Center,
            );
            if let Some(unit) = unit {
                caption(format!(" {unit}"), tokens::TEXT_FAINT).append_to(
                    &mut job,
                    ui.style(),
                    egui::FontSelection::Default,
                    egui::Align::Center,
                );
            }
            let width = value_width.max(0.0);
            ui.allocate_ui_with_layout(
                egui::vec2(width, tokens::ROW_DENSE),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.set_width(width);
                    ui.add(
                        egui::Label::new(job)
                            .truncate()
                            .show_tooltip_when_elided(!has_hover),
                    );
                },
            );
        }
        Cell::NotMeasured => {
            cell(
                ui,
                value_width,
                caption(NOT_MEASURED_TEXT, tokens::UNKNOWN),
                has_hover,
            );
        }
        Cell::Change {
            current,
            recommended,
            recommended_tone,
            delta,
        } => {
            // The current and the recommended value take the same width, so
            // the arrows of a table line up.
            let arrow = tokens::SPACE_4;
            let value = ((value_width - arrow) * 0.38).floor();
            let delta_width = (value_width - arrow - 2.0 * value - 3.0 * tokens::SPACE_1).max(0.0);
            cell(
                ui,
                value,
                caption(current.clone(), tokens::TEXT_MUTED),
                has_hover,
            );
            cell(
                ui,
                arrow,
                caption("\u{2192}", tokens::TEXT_MUTED),
                has_hover,
            );
            cell(
                ui,
                value,
                caption(recommended.clone(), *recommended_tone),
                has_hover,
            );
            cell(ui, delta_width, delta.clone(), has_hover);
        }
        Cell::Bar {
            fraction,
            text,
            fill,
        } => {
            ui.add(
                egui::ProgressBar::new(*fraction)
                    .fill(*fill)
                    .desired_width(value_width.max(0.0))
                    .desired_height(tokens::ROW_DENSE - 2.0 * tokens::SPACE_1)
                    .text(caption(text.clone(), tokens::TEXT_STRONG)),
            );
        }
        Cell::Record { .. } => {}
    }
    paint_trailing(ui, row);
}

/// The status slot, then the source slot, at the right end of the row.
fn paint_trailing(ui: &mut egui::Ui, row: &DataRow) {
    if row.status.is_none() && row.source.is_none() {
        return;
    }
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        ui.spacing_mut().item_spacing.x = tokens::SPACE_1;
        // Right to left: the source is the last column.
        match &row.source {
            Some(Source::Prov(kind, reference)) => {
                ui.add(
                    ProvenanceBadge::new(*kind)
                        .reference_opt(reference.as_deref())
                        .compact(),
                );
            }
            Some(Source::Setting(setting)) => {
                ui.add(
                    egui::Label::new(caption(setting.clone(), tokens::TEXT_FAINT))
                        .truncate()
                        .show_tooltip_when_elided(false),
                );
            }
            None => {}
        }
        if let Some((role, word)) = &row.status {
            egui::Frame::default()
                .fill(role.tint())
                .corner_radius(tokens::RADIUS_SM)
                .inner_margin(egui::Margin::symmetric(tokens::SPACE_2 as i8, 0))
                .show(ui, |ui| {
                    ui.add(
                        egui::Label::new(caption(status_text(*role, word), role.text()))
                            .truncate()
                            .show_tooltip_when_elided(false),
                    );
                });
        }
    });
}

/// The one disclosure for derivation and provenance: closed by default,
/// the same label in every section.
#[must_use]
pub struct Details {
    id_salt: egui::Id,
    count: Option<usize>,
}

impl Details {
    /// `id_salt` must be unique in its panel.
    pub fn new(id_salt: impl std::hash::Hash + std::fmt::Debug) -> Self {
        Self {
            id_salt: egui::Id::new(id_salt),
            count: None,
        }
    }

    /// The number of rows inside, shown after the label.
    pub fn count(mut self, count: usize) -> Self {
        self.count = Some(count);
        self
    }

    /// Draw the disclosure; `add` draws the body when it is open.
    pub fn show<R>(
        self,
        ui: &mut egui::Ui,
        add: impl FnOnce(&mut egui::Ui) -> R,
    ) -> egui::CollapsingResponse<R> {
        let label = match self.count {
            Some(n) => format!("{DETAILS_LABEL} \u{00B7} {n}"),
            None => DETAILS_LABEL.to_owned(),
        };
        egui::CollapsingHeader::new(caption(label, tokens::TEXT_MUTED))
            .id_salt(self.id_salt)
            .default_open(false)
            .show(ui, add)
    }
}
