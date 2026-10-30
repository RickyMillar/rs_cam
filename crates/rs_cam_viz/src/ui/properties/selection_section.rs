//! One selection block of the Geometry tab (geometry selection P4.3, GT-2,
//! GT-3, W3h): the list of one selection of a toolpath, and its notes.
//!
//! The Geometry tab (`geometry_tab.rs`) draws it for the op selection and,
//! under the Boundary row, for the boundary selection (operator ruling
//! GT-Q7).
//!
//! W3h (operator design 2026-10-30): the list IS the selection UI. One
//! `ListBox`, always drawn, never closed:
//!
//! - the header: the title, the counts ("3 loops · 2 islands",
//!   [`row_counts_text`]) at the right, the state mark (a refusal, a
//!   finding of the `selection` field, the notes, the lost picks, a report
//!   that the query cannot read: the role glyph, the texts on hover),
//!   "Pick" only while the operator left the pick mode, and the `⋯` menu
//!   ([`header_menu`]: the rule presets, "Remove rule", "Clear selection",
//!   "Reset to default" for an op whose default holds a rule: Face);
//! - one row per selected item, rule members and picks alike: the label
//!   (it truncates), the `⋯` menu ("Rule…", "Use as floor", "Show me"),
//!   then `×`. A row under the pointer lights its item in the viewport
//!   (`SelectionUi::row_hover`). A long list scrolls inside the box
//!   ([`LIST_CAP`] rows); above the cap a filter field sits under the
//!   header;
//! - an empty list: one grey placeholder row ([`placeholder_text`]: "Select
//!   faces or loops", from the kinds of the pick bar chips).
//!
//! Lead ruling L-1: the block sends `AppEvent::EditToolpathSelection`,
//! `AppEvent::UseAsFloor`, `AppEvent::FrameSelectionItems` and (the Drill
//! fix) `AppEvent::AddBoreFromDrill` only. It never writes
//! `entry.operation` or `entry.boundary`. The controller applies each act
//! through one `Command`, so one act is one undo step.
//!
//! The counts, the rows, the notes, the lost picks and the refusal come
//! from the `selection_status` report (the `SelectionStatus` query), which
//! MCP `selection_status` also gives. The "Rule…" offers come from core
//! `ProjectSession::rule_offers` (L-R4: the facts of one item).

use std::collections::BTreeSet;

use rs_cam_core::compute::catalog::ItemUse;
use rs_cam_core::ids::ModelId;
use rs_cam_core::selection::{
    Facing, Filter, HoleFit, HoleFitRule, ItemKind, KindFilter, MatchKey, MatchedBy, NoteCause,
    RefusalCause, Rule, Selection, SelectionEdit, SelectionNote, SourceRef, kind_noun,
};
use rs_cam_core::session::{
    HoleSummary, ItemState, RuleOffer, SelectionReport, SelectionTarget, short_number,
};

use super::SelectionRead;
use super::placement::Finding;
use crate::state::pick::{OffersFor, PickFilter, PickMode, SelectionUi};
use crate::state::toolpath::ToolpathId;
use crate::ui::AppEvent;
use crate::ui::components::{
    Act, DataRow, DataTable, Details, ListBox, Mark, Role, list_header, list_line,
    list_placeholder, list_row,
};
use crate::ui::tokens;

/// The most rows that the list shows before it scrolls, and the count
/// above which the filter field shows (W3h).
pub(crate) const LIST_CAP: usize = 8;

/// Everything the section reads. The section owns no project state.
pub(crate) struct SelectionSectionInputs<'a> {
    pub toolpath_id: ToolpathId,
    /// The model of the toolpath: a stored selection names it as
    /// `SourceRef::ToolpathModel`.
    pub toolpath_model: ModelId,
    pub target: SelectionTarget,
    /// The stored selection of `target`.
    pub selection: &'a Selection,
    /// The selection that `Reset` restores: the op default (W3h: no item,
    /// except Face: the stock outline), or for the boundary the value of
    /// the "Selection" source button.
    pub op_default: &'a Selection,
    /// The `selection_status` answer of `target`.
    pub report: Option<&'a SelectionRead>,
    /// How the reader uses the items: the op's `ItemUse`, or
    /// `BOUNDARY_ITEM_USE`.
    pub item_use: ItemUse,
    /// The project models, for the rule text.
    pub models: &'a [(crate::state::job::ModelId, String)],
    /// The "Rule…" offers of the open row menu (core `rule_offers`).
    /// Empty while no menu of this selection is open.
    pub offers: &'a [RuleOffer],
    /// The op label, for the note of the kinds that the op never cuts.
    pub op_label: &'static str,
    /// True when a face or a model ring offers "Use as floor": the op
    /// target of an op that holds a floor (L-R5).
    pub floor_op: bool,
    /// For each note of the report, the keys that "Show me" frames
    /// (`selection_highlight::note_item_keys`): the skipped items of its
    /// kind and layer, or the one face of an at-the-top note.
    pub note_items: &'a [Vec<MatchKey>],
    /// The item kinds that the toolpath model holds
    /// (`state::pick::held_kinds`): the placeholder and the rule presets
    /// read it.
    pub model_kinds: &'a BTreeSet<ItemKind>,
    /// The diameter of the tool of the toolpath (`ToolConfig::diameter`),
    /// for the hole size rows (P6.7): "drilled at" a diameter and the Bore
    /// helix radius. `None` with no tool, and for the boundary.
    pub tool_diameter: Option<f64>,
    /// The findings that `placement::place` anchors to the `selection`
    /// field. They set the state mark of the list header. Empty for the
    /// boundary selection.
    pub findings: &'a [Finding<'a>],
}

impl SelectionSectionInputs<'_> {
    /// One edit of this selection: the only selection write of the
    /// section (L-1).
    fn send(&self, events: &mut Vec<AppEvent>, edit: SelectionEdit) {
        events.push(AppEvent::EditToolpathSelection {
            toolpath_id: self.toolpath_id,
            target: self.target,
            edit,
        });
    }

    /// True when the stored selection is the default. The command stores
    /// each source in the toolpath form (P2.7), so both sides are compared
    /// in that form.
    fn is_default(&self) -> bool {
        self.selection
            .clone()
            .for_toolpath_model(self.toolpath_model)
            == self
                .op_default
                .clone()
                .for_toolpath_model(self.toolpath_model)
    }

    /// The egui id salt of one widget of this section.
    fn salt(&self, what: &str) -> (ToolpathId, SelectionTarget, String) {
        (self.toolpath_id, self.target, what.to_owned())
    }
}

/// Draw the block. `ui_state` holds the pick mode, the open "Rule…" menu,
/// the hovered row and the filter text; `events` receives the edits.
pub(crate) fn draw_selection_section(
    ui: &mut egui::Ui,
    inputs: &SelectionSectionInputs<'_>,
    ui_state: &mut SelectionUi,
    events: &mut Vec<AppEvent>,
) {
    // A report that the query cannot read is the Danger mark of the list
    // header, with the error on its hover.
    let (report, read_error) = match inputs.report {
        Some(Ok(report)) => (Some(report.as_ref()), None),
        Some(Err(error)) => (None, Some(error.as_str())),
        None => (None, None),
    };
    let rows = report.map_or_else(Vec::new, |report| list_rows(inputs, report));
    let header = header_state(inputs, report, read_error);
    let on = ui_state.pick_is_on(inputs.toolpath_id, inputs.target);

    let _ = ListBox::new(inputs.salt("selected_list")).show(ui, |ui| {
        // ── The header ─────────────────────────────────────────────
        let mut header_choice: Option<HeaderAct> = None;
        let mut acts: Vec<Act<'_>> = Vec::new();
        // W3g (operator ruling 2026-10-30): the pick mode stays on while
        // the toolpath is selected, and the pick bar has "Done". So the
        // header shows "Pick" only when the operator left the mode.
        if !on {
            acts.push(Act::Button(
                PICK,
                "Turn on the viewport pick for this selection.",
            ));
        }
        let menu = header_menu(inputs);
        let choice = &mut header_choice;
        acts.push(Act::Menu(
            "Add a rule, remove a rule, or clear the selection.",
            Box::new(move |ui: &mut egui::Ui| {
                for (index, (label, hover, enabled)) in menu.iter().enumerate() {
                    if ui
                        .add_enabled(*enabled, egui::Button::new(label.as_str()))
                        .on_hover_text(hover.as_str())
                        .on_disabled_hover_text(hover.as_str())
                        .clicked()
                    {
                        *choice = Some(HeaderAct::Menu(index));
                    }
                }
            }),
        ));
        let mark = header.role.map(|role| Mark {
            role,
            hover: header.detail.as_deref().unwrap_or(""),
        });
        let line = list_header(
            ui,
            list_title(inputs.target),
            &header.count,
            list_hover(inputs.target),
            mark,
            acts,
        );
        if line.clicked == Some(0) && !on {
            header_choice = Some(HeaderAct::Pick);
        }
        match header_choice {
            Some(HeaderAct::Pick) => {
                // L-3: one Pick at a time. A click replaces the mode of
                // any other block.
                ui_state.set_pick(Some(PickMode::new(
                    inputs.toolpath_id,
                    inputs.target,
                    inputs.item_use,
                )));
            }
            Some(HeaderAct::Menu(index)) => {
                if let Some(edit) = header_menu_edit(inputs, index) {
                    inputs.send(events, edit);
                }
            }
            None => {}
        }

        // ── The filter field, above the cap ────────────────────────
        if rows.len() > LIST_CAP {
            ui.add(
                egui::TextEdit::singleline(&mut ui_state.list_filter)
                    .id_salt(inputs.salt("list_filter"))
                    .hint_text("Filter the selected items")
                    .desired_width(f32::INFINITY),
            )
            .on_hover_text(
                "Show only the rows whose text holds this text. It does not search the model.",
            );
        }
        let filter = if rows.len() > LIST_CAP {
            ui_state.list_filter.trim().to_lowercase()
        } else {
            String::new()
        };
        let kept: Vec<&ListRow> = rows
            .iter()
            .filter(|row| filter.is_empty() || row.full.to_lowercase().contains(&filter))
            .collect();

        // ── The rows, or the placeholder ───────────────────────────
        if rows.is_empty() {
            list_placeholder(
                ui,
                &placeholder_text(inputs.item_use, inputs.model_kinds),
                "Nothing is selected. Click items in the viewport (Pick), or add a rule from \
                 the \u{22EF} menu of the list.",
            );
        } else {
            ListBox::rows(
                ui,
                inputs.salt("rows"),
                kept.len(),
                LIST_CAP,
                |ui, index| {
                    if let Some(row) = kept.get(index) {
                        draw_row(ui, inputs, row, ui_state, events);
                    }
                },
            );
        }

        // ── The "Rule…" offers of the open row ─────────────────────
        let open = ui_state
            .offers_for(inputs.toolpath_id, inputs.target)
            .map(|o| o.key.clone());
        if let Some(key) = open
            && rows.iter().any(|row| row.key == key)
            && draw_offers(ui, inputs, events)
        {
            ui_state.offers_for = None;
        }
    });

    // The hole sizes (P6.7): one derived row per size (IC2).
    let sizes = report.map_or_else(Vec::new, |report| hole_sizes(inputs, report));
    if !sizes.is_empty() {
        let mut table = DataTable::new(inputs.salt("selection_counts"));
        for size in &sizes {
            table = table.row(size.row(inputs.tool_diameter));
        }
        // A hole size row answers a click: Show me frames the holes of
        // that size. The table holds the size rows only.
        if let Some(clicked) = table.show(ui).clicked
            && let Some(size) = sizes.get(clicked)
        {
            show_me(inputs, events, size.keys.clone());
        }
    }

    // ── The notes ──────────────────────────────────────────────────
    draw_add_bore(ui, inputs, &sizes, events);
    if let Some(report) = report {
        draw_extend_summary(ui, inputs, report, events);
        draw_findings(ui, inputs, report, events);
        draw_details(ui, inputs, report);
    }
}

/// What the header of the list reported.
enum HeaderAct {
    Pick,
    /// The item of [`header_menu`] at this index.
    Menu(usize),
}

/// The title of the list of a selection: "Selection" for the op,
/// "Boundary selection" for the boundary (it sits under the Boundary row).
pub(crate) fn list_title(target: SelectionTarget) -> &'static str {
    match target {
        SelectionTarget::Op => "Selection",
        SelectionTarget::Boundary => "Boundary selection",
    }
}

/// The explanation of the list header.
fn list_hover(target: SelectionTarget) -> &'static str {
    match target {
        SelectionTarget::Op => {
            "What this operation cuts: one row per item, the counts at the right. Pick turns \
             on the viewport pick; the \u{22EF} menu adds a rule or clears the selection."
        }
        SelectionTarget::Boundary => {
            "The items that bound where the tool may go. Pick turns on the viewport pick; the \
             \u{22EF} menu adds a rule or clears the selection."
        }
    }
}

/// The plural noun of a pick kind in the placeholder.
fn placeholder_noun(kind: ItemKind) -> &'static str {
    match kind {
        ItemKind::Loop => "loops",
        ItemKind::Path => "open paths",
        ItemKind::Circle => "circles",
        ItemKind::Face => "faces",
        ItemKind::Point => "points",
    }
}

/// W3h: the placeholder row of an empty list, from the kinds of the pick
/// bar chips (`PickFilter::cut_kinds`: the kinds that the reader serves
/// and that the model holds; every served kind when the model holds none).
/// A hole reader reads "Select holes"; else "Select faces or loops".
pub(crate) fn placeholder_text(item_use: ItemUse, model_kinds: &BTreeSet<ItemKind>) -> String {
    if matches!(item_use, ItemUse::Holes { .. }) {
        return "Select holes".to_owned();
    }
    let mut kinds = PickFilter::cut_kinds(item_use, model_kinds);
    if kinds.is_empty() {
        kinds = PickFilter::served(item_use).effective_kinds(item_use, &all_kinds());
    }
    // Faces first: the pick of a STEP model is a face pick.
    kinds.sort_by_key(|k| *k != ItemKind::Face);
    let nouns: Vec<&str> = kinds.into_iter().map(placeholder_noun).collect();
    match nouns.as_slice() {
        [] => "Select items".to_owned(),
        [one] => format!("Select {one}"),
        [rest @ .., last] => format!("Select {} or {last}", rest.join(", ")),
    }
}

/// Every item kind, as a set.
fn all_kinds() -> BTreeSet<ItemKind> {
    KINDS.into_iter().collect()
}

/// "item" or "items".
fn items_word(count: usize) -> &'static str {
    if count == 1 { "item" } else { "items" }
}

/// Every item kind, in the order of the "cuts" phrase.
const KINDS: [ItemKind; 5] = [
    ItemKind::Loop,
    ItemKind::Circle,
    ItemKind::Face,
    ItemKind::Path,
    ItemKind::Point,
];

/// The count of the items that the rules hold and that the op never
/// cuts: the skipped items of a kind that does not serve the op.
fn never_cut_count(inputs: &SelectionSectionInputs<'_>, report: &SelectionReport) -> usize {
    report
        .skipped_by_kind
        .iter()
        .filter(|(kind, _)| !kind.serves(inputs.item_use))
        .map(|(_, n)| *n)
        .sum()
}

/// The one line of the items that the op never cuts (FR-2 F32): "Drill
/// cuts circles and points; 6 other items are not cut".
pub(crate) fn never_cut_line(op: &str, item_use: ItemUse, count: usize) -> String {
    let served: Vec<&str> = KINDS
        .iter()
        .filter(|kind| kind.serves(item_use))
        .map(|kind| kind_noun(*kind, 2))
        .collect();
    let cuts = match served.as_slice() {
        [] => "cuts no drawing item".to_owned(),
        [one] => format!("cuts {one}"),
        [rest @ .., last] => format!("cuts {} and {last}", rest.join(", ")),
    };
    let verb = if count == 1 { "is" } else { "are" };
    format!(
        "{op} {cuts}; {count} other {} {verb} not cut",
        items_word(count)
    )
}

/// True when `note` is of a kind that the op never cuts: the one line
/// holds it, and the Skipped items list names its layer.
fn is_never_cut(note: &SelectionNote) -> bool {
    matches!(note.cause, NoteCause::KindNotServed)
}

/// What the `⋯` menu of a row offers besides `×` (L-R4, L-R5, GT-Q8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct RowActs {
    /// "Rule…": the offers from the facts of this item.
    pub rule: bool,
    /// "Use as floor": a face or a model ring of a floor op.
    pub floor: bool,
}

/// Where a row of the list comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RowFrom {
    /// A member of the rule at this index. Its `×` turns the rule into
    /// picks of its other members (GT-Q8, core `rule_drop_edit`).
    Rule(usize),
    /// A pick. Its `×` removes the pick.
    Pick,
}

/// One row of the selected list.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ListRow {
    pub key: MatchKey,
    /// The short text of the row: the kind, the size and the Z (FR-2 F22).
    pub label: String,
    /// The full text of the row: the place and the layer too. The hover
    /// shows it, and the filter field reads it.
    pub full: String,
    /// The report state of the item.
    pub state: ItemState,
    pub from: RowFrom,
    pub acts: RowActs,
}
/// The short text of one item row (FR-2 F22): the kind, the size and the
/// Z, so that the row fits the inspector column with its acts. The place
/// and the layer are in [`row_full_label`], on the hover. A circle reads
/// "Ø6.00 Z20", a face "Face Z20.00".
pub(crate) fn row_label(key: &MatchKey, plane_z: Option<f64>) -> String {
    let z = plane_z.map_or_else(String::new, |z| format!(" Z{}", short_number(z)));
    let m = key.measure;
    match key.kind {
        ItemKind::Circle => format!("\u{00D8}{m:.2}{z}"),
        ItemKind::Face => plane_z.map_or_else(|| format!("Face {m:.1} mm"), face_label),
        ItemKind::Loop => format!("Loop{z} {m:.1} mm"),
        ItemKind::Path => format!("Path{z} {m:.1} mm"),
        ItemKind::Point => format!("Point{z}"),
    }
}

/// R-A: the short text of the part outline row: "Part outline Z0".
pub(crate) fn outline_label(plane_z: Option<f64>) -> String {
    let z = plane_z.map_or_else(String::new, |z| format!(" Z{}", short_number(z)));
    format!("{}{z}", rs_cam_core::selection::PART_OUTLINE_LABEL)
}

/// R-A: the full text of the part outline row: "Part outline at (0.0,
/// 0.0) Z 0.00".
pub(crate) fn outline_full_label(key: &MatchKey, plane_z: Option<f64>) -> String {
    let z = plane_z.map_or_else(String::new, |z| format!(" Z {z:.2}"));
    format!(
        "{} at ({:.1}, {:.1}){z}",
        rs_cam_core::selection::PART_OUTLINE_LABEL,
        key.anchor[0],
        key.anchor[1]
    )
}

/// The short name of a face at plane `z`: "Face Z20.00" (FR-2 F23). The
/// list rows and the floor face slot read it.
pub(crate) fn face_label(z: f64) -> String {
    format!("Face Z{z:.2}")
}

/// The full text of one item row: the kind, the size, the place and the
/// layer.
pub(crate) fn row_full_label(key: &MatchKey, plane_z: Option<f64>) -> String {
    let at = format!("({:.1}, {:.1})", key.anchor[0], key.anchor[1]);
    let z = plane_z.map_or_else(String::new, |z| format!(" Z {z:.2}"));
    let layer = key
        .layer
        .as_deref()
        .map_or_else(String::new, |l| format!(" \u{00B7} {l}"));
    match key.kind {
        ItemKind::Circle => format!("\u{00D8} {:.2} at {at}{z}{layer}", key.measure),
        ItemKind::Face => format!("Face{z} at {at}, {:.1} mm across{layer}", key.measure),
        ItemKind::Loop => format!("Loop{z} at {at}{layer}"),
        ItemKind::Path => format!("Path{z} from {at}, {:.1} mm{layer}", key.measure),
        ItemKind::Point => format!("Point{z} at {at}{layer}"),
    }
}

/// The word of an item state on a row.
pub(crate) fn state_word(state: ItemState) -> &'static str {
    match state {
        ItemState::Cut => "Cut",
        ItemState::Island => "Island",
        ItemState::Skipped => "Not cut",
        ItemState::Excluded => "Removed",
        ItemState::NotSelected => "\u{2014}",
    }
}

/// The rows of the selected list (W3h): the members of each rule, in rule
/// order, then the picks. Each row of the report is one row. FR-2 F32: a
/// rule member of a kind that the op never cuts is not a row; the one
/// "not cut" note holds it. A pick of such a kind stays a row (it refuses,
/// and its `×` is the fix).
pub(crate) fn list_rows(
    inputs: &SelectionSectionInputs<'_>,
    report: &SelectionReport,
) -> Vec<ListRow> {
    // R-E: a hole row reads the one core hole text ("Ø6 hole, 8 deep").
    // R-A: the part outline row reads "Part outline Z0".
    let row_of = |row: &rs_cam_core::session::SelectedRow, from: RowFrom| {
        let floor = inputs.floor_op
            && match row.key.kind {
                ItemKind::Face => true,
                ItemKind::Loop => row.key.z.is_some(),
                ItemKind::Path | ItemKind::Point | ItemKind::Circle => false,
            };
        ListRow {
            key: row.key.clone(),
            label: if row.outline {
                outline_label(row.plane_z)
            } else {
                row.hole
                    .as_ref()
                    .map_or_else(|| row_label(&row.key, row.plane_z), HoleSummary::label)
            },
            full: match (&row.hole, row.outline) {
                (_, true) => outline_full_label(&row.key, row.plane_z),
                (Some(hole), false) => format!(
                    "{} \u{00B7} {}",
                    hole.label(),
                    row_full_label(&row.key, row.plane_z)
                ),
                (None, false) => row_full_label(&row.key, row.plane_z),
            },
            state: report.row_state(row),
            from,
            acts: RowActs { rule: true, floor },
        }
    };
    let mut rows = Vec::new();
    for index in 0..inputs.selection.rules().len() {
        rows.extend(
            report
                .rows
                .iter()
                .filter(|row| {
                    matches!(row.by, MatchedBy::Named { rule } | MatchedBy::Default { rule } if rule == index)
                })
                .filter(|row| row.key.kind.serves(inputs.item_use))
                .map(|row| row_of(row, RowFrom::Rule(index))),
        );
    }
    rows.extend(
        report
            .rows
            .iter()
            .filter(|row| row.by == MatchedBy::Pick)
            .map(|row| row_of(row, RowFrom::Pick)),
    );
    rows
}

/// One act of the `⋯` menu of a row, or its `×`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RowAct {
    Rule,
    Floor,
    ShowMe,
    Remove,
}

/// The items of the `⋯` menu of a row, in order, with their hover.
fn row_menu(acts: RowActs) -> Vec<(RowAct, &'static str, &'static str)> {
    let mut out = Vec::new();
    if acts.rule {
        out.push((
            RowAct::Rule,
            "Rule\u{2026}",
            "Make a rule from the facts of this item: its size, its Z, its layer.",
        ));
    }
    if acts.floor {
        out.push((
            RowAct::Floor,
            "Use as floor",
            "The op cuts down to the plane of this face. The face leaves the selection.",
        ));
    }
    out.push((RowAct::ShowMe, SHOW_ME, "Frame this item in the viewport."));
    out
}

/// The text of the remove act of a row (W3h): `×`. It is not the Danger
/// glyph `✕` of the header mark.
pub(crate) const REMOVE: &str = "\u{00D7}";

/// The hover of the `×` of a row.
fn remove_hover(from: RowFrom) -> &'static str {
    match from {
        RowFrom::Pick => "Remove this pick from the selection.",
        RowFrom::Rule(_) => {
            "Remove this item. The rule that holds it turns into picks of its other items."
        }
    }
}

/// Draw one item row: the label and the state word, the `⋯` menu, then
/// `×`. W3h: a row under the pointer lights its item in the viewport,
/// for every row kind (a pick, a rule member, a face).
fn draw_row(
    ui: &mut egui::Ui,
    inputs: &SelectionSectionInputs<'_>,
    row: &ListRow,
    ui_state: &mut SelectionUi,
    events: &mut Vec<AppEvent>,
) {
    let text = format!("{} \u{00B7} {}", row.label, state_word(row.state));
    let hover = format!("{} \u{00B7} {}", row.full, state_word(row.state));
    let color = match row.state {
        ItemState::Cut | ItemState::Island => tokens::TEXT_BODY,
        ItemState::Skipped | ItemState::Excluded | ItemState::NotSelected => tokens::TEXT_MUTED,
    };
    let menu = row_menu(row.acts);
    let mut chosen: Option<RowAct> = None;
    let choice = &mut chosen;
    let line = list_row(
        ui,
        &text,
        color,
        &hover,
        vec![
            Act::Menu(
                "More acts for this item.",
                Box::new(move |ui: &mut egui::Ui| {
                    for (act, label, act_hover) in &menu {
                        if ui.button(*label).on_hover_text(*act_hover).clicked() {
                            *choice = Some(*act);
                        }
                    }
                }),
            ),
            Act::Button(REMOVE, remove_hover(row.from)),
        ],
    );
    if line.clicked == Some(1) {
        chosen = Some(RowAct::Remove);
    }
    if line.hovered {
        // GT-2: the viewport hover buffer draws the row item.
        ui_state.row_hover = Some(row.key.clone());
    }
    match chosen {
        Some(RowAct::Rule) => {
            let open = ui_state
                .offers_for(inputs.toolpath_id, inputs.target)
                .is_some_and(|o| o.key == row.key);
            ui_state.offers_for = (!open).then(|| OffersFor {
                toolpath_id: inputs.toolpath_id,
                target: inputs.target,
                key: row.key.clone(),
            });
            // The inputs read the offers only while the menu is open, so
            // the next frame must come at once.
            ui.ctx().request_repaint();
        }
        Some(RowAct::Floor) => events.push(AppEvent::UseAsFloor {
            toolpath_id: inputs.toolpath_id,
            key: row.key.clone(),
        }),
        Some(RowAct::ShowMe) => show_me(inputs, events, vec![row.key.clone()]),
        // GT-Q8: on a rule member, core turns the rule into picks of its
        // other members (`rule_drop_edit`).
        Some(RowAct::Remove) => inputs.send(events, SelectionEdit::RemovePick(row.key.clone())),
        None => {}
    }
}

/// The rule of a preset: every item of `kinds` on the toolpath model, or
/// every item that the reader cuts when `kinds` is `None`.
fn preset_rule(kinds: Option<ItemKind>) -> Rule {
    Rule {
        source: SourceRef::ToolpathModel,
        filter: Filter {
            kinds: kinds.map_or(KindFilter::Served, |k| KindFilter::Only(vec![k])),
            ..Filter::default()
        },
    }
}

/// One item of the header `⋯` menu: what it writes.
#[derive(Debug, Clone, PartialEq)]
enum HeaderItem {
    /// "Every closed loop", "Every circle", "Every item it cuts".
    AddRule(Rule),
    /// "Remove rule: …".
    RemoveRule(usize),
    /// "Clear selection".
    Clear,
    /// "Reset to default" (an op whose default holds a rule: Face).
    Reset,
}

/// The items of the header `⋯` menu (W3h), each with its label and
/// whether it does something now:
///
/// - the rule presets: one "Every …" per kind that the reader cuts and
///   the model holds, and "Every item it cuts" when there is more than one
///   such kind (a layer rule comes from the "Rule…" menu of a row);
/// - "Remove rule: …" for each rule of the selection;
/// - "Clear selection";
/// - "Reset to default" while the default of the op holds a rule (Face:
///   the stock outline) and the selection is not that default.
fn header_items(inputs: &SelectionSectionInputs<'_>) -> Vec<(HeaderItem, String, String, bool)> {
    let mut out = Vec::new();
    let kinds = PickFilter::cut_kinds(inputs.item_use, inputs.model_kinds);
    let holds = |rule: &Rule| inputs.selection.rules().contains(rule);
    for kind in &kinds {
        let rule = preset_rule(Some(*kind));
        let enabled = !holds(&rule);
        out.push((
            HeaderItem::AddRule(rule),
            format!("Every {}", kind_noun(*kind, 1)),
            format!(
                "Add the rule: every {} of the model of this toolpath, on every layer.",
                kind_noun(*kind, 1)
            ),
            enabled,
        ));
    }
    if kinds.len() > 1 {
        let rule = preset_rule(None);
        let enabled = !holds(&rule);
        out.push((
            HeaderItem::AddRule(rule),
            "Every item it cuts".to_owned(),
            "Add the rule: every item of the model of this toolpath that this operation can \
             cut, on every layer."
                .to_owned(),
            enabled,
        ));
    }
    for (index, rule) in inputs.selection.rules().iter().enumerate() {
        let text = rule_text(rule, inputs.models);
        out.push((
            HeaderItem::RemoveRule(index),
            format!("Remove rule: {text}"),
            format!("Remove the rule \u{201C}{text}\u{201D} and the items that only it holds."),
            true,
        ));
    }
    out.push((
        HeaderItem::Clear,
        "Clear selection".to_owned(),
        "Remove every rule and every pick: the selection holds nothing.".to_owned(),
        !inputs.selection.is_empty(),
    ));
    if inputs.target == SelectionTarget::Op && !inputs.op_default.is_empty() {
        out.push((
            HeaderItem::Reset,
            "Reset to default".to_owned(),
            "Go back to the default selection of the operation.".to_owned(),
            !inputs.is_default(),
        ));
    }
    out
}

/// The labels, the hovers and the enabled states of the header menu.
fn header_menu(inputs: &SelectionSectionInputs<'_>) -> Vec<(String, String, bool)> {
    header_items(inputs)
        .into_iter()
        .map(|(_, label, hover, enabled)| (label, hover, enabled))
        .collect()
}

/// The edit of the header menu item at `index`.
fn header_menu_edit(inputs: &SelectionSectionInputs<'_>, index: usize) -> Option<SelectionEdit> {
    let (item, _, _, enabled) = header_items(inputs).into_iter().nth(index)?;
    if !enabled {
        return None;
    }
    Some(match item {
        HeaderItem::AddRule(rule) => SelectionEdit::AddRule(rule),
        HeaderItem::RemoveRule(index) => SelectionEdit::RemoveRule(index),
        HeaderItem::Clear => SelectionEdit::Replace(Box::new(Selection::empty())),
        HeaderItem::Reset => SelectionEdit::Reset,
    })
}

/// The labels of the header menu, for the tests.
#[cfg(test)]
pub(crate) fn header_menu_labels(inputs: &SelectionSectionInputs<'_>) -> Vec<String> {
    header_menu(inputs)
        .into_iter()
        .map(|(label, _, _)| label)
        .collect()
}

/// The edit of the header menu item `label`, for the tests.
#[cfg(test)]
pub(crate) fn header_menu_edit_of(
    inputs: &SelectionSectionInputs<'_>,
    label: &str,
) -> Option<SelectionEdit> {
    let index = header_menu(inputs)
        .iter()
        .position(|(l, _, _)| l == label)?;
    header_menu_edit(inputs, index)
}

/// The "Rule…" offers of the open row (L-R4). Each offer shows its match
/// count; "Add" sends its one `Replace`. An offer that a rule already holds
/// has no "Add". True when an offer was added (the menu closes).
fn draw_offers(
    ui: &mut egui::Ui,
    inputs: &SelectionSectionInputs<'_>,
    events: &mut Vec<AppEvent>,
) -> bool {
    if inputs.offers.is_empty() {
        let _ = list_line(
            ui,
            "No rule fits this item.",
            tokens::TEXT_MUTED,
            "This item has no fact that a rule can read.",
            &[],
        );
        return false;
    }
    let mut added = false;
    for offer in inputs.offers {
        let text = format!("  {} ({})", offer.label, offer.count);
        match &offer.edit {
            Some(edit) => {
                if list_line(
                    ui,
                    &text,
                    tokens::TEXT_BODY,
                    "The rule replaces the picks that it holds and every rule whose items it \
                     holds.",
                    &[("Add", "Add this rule.")],
                )
                .clicked
                .is_some()
                {
                    inputs.send(events, edit.clone());
                    added = true;
                }
            }
            None => {
                let _ = list_line(
                    ui,
                    &text,
                    tokens::TEXT_MUTED,
                    "A rule of this selection holds these items already.",
                    &[],
                );
            }
        }
    }
    added
}

/// The count of each kind the reader uses, and the islands: "14 closed
/// loops · 3 islands · 2 circles". P3.4: the faces name their down count,
/// "4 faces (1 down)", in the setup frame. `None` when nothing is cut.
pub(crate) fn counts_text(report: &SelectionReport) -> Option<String> {
    counts_with(report, kind_noun)
}

/// The noun of a kind in the list header (W3g, W3h): "loop" for a closed loop, so
/// "3 loops · 2 islands" fits the 340-point inspector. The other kinds keep
/// core `kind_noun`; [`counts_text`] keeps the full nouns.
fn row_noun(kind: ItemKind, count: usize) -> &'static str {
    match kind {
        ItemKind::Loop if count == 1 => "loop",
        ItemKind::Loop => "loops",
        ItemKind::Path | ItemKind::Point | ItemKind::Circle | ItemKind::Face => {
            kind_noun(kind, count)
        }
    }
}

/// The counts of the list header (W3g, W3h): [`counts_text`] with the short nouns
/// of [`row_noun`]. "3 loops · 2 islands".
pub(crate) fn row_counts_text(report: &SelectionReport) -> Option<String> {
    counts_with(report, row_noun)
}

/// The counts with the noun of `noun` for each kind.
fn counts_with(
    report: &SelectionReport,
    noun: fn(ItemKind, usize) -> &'static str,
) -> Option<String> {
    let mut parts: Vec<String> = report
        .counts
        .iter()
        .filter(|(_, n)| **n > 0)
        .map(|(kind, n)| {
            let down = report.faces_by_facing.down;
            if *kind == ItemKind::Face && down > 0 {
                format!("{n} {} ({down} down)", noun(*kind, *n))
            } else {
                format!("{n} {}", noun(*kind, *n))
            }
        })
        .collect();
    if report.islands > 0 {
        let n = report.islands;
        parts.push(format!("{n} island{}", if n == 1 { "" } else { "s" }));
    }
    (!parts.is_empty()).then(|| parts.join(" \u{00B7} "))
}

/// The quiet action of the summary row that turns Pick on.
/// The quiet act of the list header that turns Pick on.
pub(crate) const PICK: &str = "Pick";

/// What the list header draws: the counts, the state mark and the texts
/// of its hover.
struct HeaderState {
    count: String,
    role: Option<Role>,
    detail: Option<String>,
}

/// The header of a selection list (W3h). The mark is the worst of:
///
/// - a report that the query cannot read (Danger);
/// - the findings of the `selection` field (`placement::place`; W3h: the
///   empty selection of a new op is the finding "Select the geometry to
///   cut");
/// - the refusal of the report (Danger), except the refusal of an empty
///   selection: the placeholder row and the finding state it;
/// - the notes and the lost picks of the report (Caution).
///
/// The texts of each go on the hover of the mark, one per paragraph.
fn header_state(
    inputs: &SelectionSectionInputs<'_>,
    report: Option<&SelectionReport>,
    read_error: Option<&str>,
) -> HeaderState {
    let mut lines: Vec<String> = Vec::new();
    let mut roles: Vec<Role> = Vec::new();
    if let Some(error) = read_error {
        lines.push(error.to_owned());
        roles.push(Role::Danger);
    }
    if matches!(inputs.target, SelectionTarget::Op) {
        super::placement::note_drawn(rs_cam_core::diagnostics::presentation::SELECTION_KEY);
    }
    if let Some(hover) = super::placement::tool_hover(inputs.findings) {
        lines.push(hover);
    }
    roles.extend(super::placement::tool_role(inputs.findings));
    let count = match report {
        None => String::new(),
        // P4.7: the flat faces of an STL are not computed yet. No count
        // and no refusal; the job fills the cache, and Generate computes
        // them too.
        Some(report) if !report.faces_not_ready.is_empty() => {
            crate::ui::pick_bar::COMPUTING_FACES.to_owned()
        }
        Some(report) => {
            if let Some(refusal) = &report.refusal {
                let empty = inputs.selection.is_empty()
                    && matches!(refusal.cause, RefusalCause::Empty { .. });
                let text = refusal.to_string();
                // A `selection` finding states the same refusal; the hover
                // holds it once.
                if !empty && !lines.iter().any(|line| line.contains(&text)) {
                    lines.push(text);
                    roles.push(Role::Danger);
                } else if empty && inputs.findings.is_empty() {
                    // The boundary selection has no finding: the empty
                    // list says it on the mark.
                    lines.push("Nothing is selected.".to_owned());
                    roles.push(Role::Caution);
                }
            }
            // One finding per note row: the items of the kinds that the op
            // never cuts are one row (F32).
            let never_cut = usize::from(never_cut_count(inputs, report) > 0);
            let other_notes = report.notes.iter().filter(|n| !is_never_cut(n)).count();
            let findings = never_cut + other_notes + report.lost.len();
            if findings > 0 {
                lines.push(if findings == 1 {
                    "1 note under the list.".to_owned()
                } else {
                    format!("{findings} notes under the list.")
                });
                roles.push(Role::Caution);
            }
            row_counts_text(report).unwrap_or_default()
        }
    };
    let role = roles.into_iter().min_by_key(|role| role.severity_rank());
    HeaderState {
        count,
        role,
        detail: (!lines.is_empty()).then(|| lines.join("\n\n")),
    }
}

/// One hole size of the selection of a Drill or a Bore (geometry selection
/// P6.7): the held holes of one diameter, or the points, with their fit.
struct HoleSize {
    /// The circle diameter; `None` for the points.
    diameter: Option<f64>,
    /// The fit of the size (`SelectionReport::hole_fits`); `None` when the
    /// report holds no fit (no valid tool).
    fit: Option<HoleFit>,
    /// The fit rule of the op.
    rule: HoleFitRule,
    /// The keys of the holes, for Show me.
    keys: Vec<MatchKey>,
}

impl HoleSize {
    /// The row: the diameter, the count, the fit word and its role.
    fn row(&self, tool_diameter: Option<f64>) -> DataRow {
        let name = self
            .diameter
            .map_or_else(|| "points".to_owned(), |d| format!("\u{00D8} {d:.3}"));
        let (role, word) = self.status(tool_diameter);
        let tool = tool_diameter.map_or_else(
            || "the tool".to_owned(),
            |d| format!("the tool \u{00D8} {d:.3}"),
        );
        // FR-2 F22: the status word is short; the hover gives it in full.
        let long = match word.as_str() {
            "larger" => "larger than the tool",
            "smaller" => "smaller than the tool",
            other => other,
        };
        let hover = format!(
            "{long}. The fit compares the hole with {tool}, within the size tolerance of \
             the op. Click to show these holes in the viewport."
        );
        DataRow::value(name, format!("\u{00D7} {}", self.keys.len()))
            .status(role, word)
            .hover(hover)
            .clickable()
    }

    /// The role and the fit word of the size, for the rule of the op
    /// (design section 7). A size that the op cuts is `Ok`. A size that
    /// it leaves out is `Caution`; a Bore circle that leaves a core is
    /// `Danger`.
    fn status(&self, tool_diameter: Option<f64>) -> (Role, String) {
        let Some(fit) = self.fit else {
            return (Role::Unknown, "no tool to compare".to_owned());
        };
        match (self.rule, fit) {
            (HoleFitRule::ToolSize, HoleFit::NoSize) => (
                Role::Ok,
                tool_diameter.map_or_else(
                    || "drilled at the tool size".to_owned(),
                    |d| format!("drilled at \u{00D8} {d:.3}"),
                ),
            ),
            (HoleFitRule::ToolSize | HoleFitRule::None, HoleFit::ToolSize)
            | (HoleFitRule::None, HoleFit::NoSize) => (Role::Ok, "drilled".to_owned()),
            (HoleFitRule::LargerThanTool, HoleFit::Larger) => (
                Role::Ok,
                match (self.diameter, tool_diameter) {
                    (Some(d), Some(t)) => {
                        format!("helix r {:.3}", rs_cam_core::ops::bore::centre_radius(d, t))
                    }
                    _ => "bored".to_owned(),
                },
            ),
            (HoleFitRule::LargerThanTool, HoleFit::LeavesCore) => {
                (Role::Danger, "leaves a core".to_owned())
            }
            (HoleFitRule::LargerThanTool, HoleFit::NoSize) => {
                (Role::Caution, "no size: not bored".to_owned())
            }
            (HoleFitRule::LargerThanTool, HoleFit::ToolSize) => {
                (Role::Caution, "the tool size: not bored".to_owned())
            }
            (HoleFitRule::ToolSize | HoleFitRule::None, HoleFit::Larger) => {
                (Role::Caution, "larger".to_owned())
            }
            (HoleFitRule::ToolSize | HoleFitRule::None, HoleFit::LeavesCore) => {
                (Role::Caution, "leaves a core".to_owned())
            }
            (_, HoleFit::Smaller) => (Role::Caution, "smaller".to_owned()),
        }
    }
}

/// The hole sizes of the selection (P6.7): the held holes of the report
/// (`hole_items`), grouped by diameter, each with the fit that the report
/// gives (`hole_fits`, the fit step of generation). The circles come first,
/// the smallest first, then the points. Empty for an op with no fit rule
/// (Pin Drill drills every hole) and for the boundary target.
fn hole_sizes(inputs: &SelectionSectionInputs<'_>, report: &SelectionReport) -> Vec<HoleSize> {
    let ItemUse::Holes { fit: rule, .. } = inputs.item_use else {
        return Vec::new();
    };
    if rule == HoleFitRule::None || inputs.target != SelectionTarget::Op {
        return Vec::new();
    }
    let fit_of = |key: &MatchKey| {
        report
            .hole_fits
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, fit)| *fit)
    };
    let mut sizes: Vec<HoleSize> = Vec::new();
    for item in report.hole_items.iter().filter(|i| i.selected) {
        // Two diameters that print the same at three decimals are one row.
        let same = |size: &HoleSize| match (size.diameter, item.diameter) {
            (Some(a), Some(b)) => format!("{a:.3}") == format!("{b:.3}"),
            (None, None) => true,
            (Some(_), None) | (None, Some(_)) => false,
        };
        match sizes.iter_mut().find(|size| same(size)) {
            Some(size) => size.keys.push(item.key.clone()),
            None => sizes.push(HoleSize {
                diameter: item.diameter,
                fit: fit_of(&item.key),
                rule,
                keys: vec![item.key.clone()],
            }),
        }
    }
    sizes.sort_by(|a, b| match (a.diameter, b.diameter) {
        (Some(a), Some(b)) => a.total_cmp(&b),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    });
    sizes
}

/// The label of the action that adds a Bore for the larger holes (P6.7).
pub(crate) const ADD_A_BORE: &str = "Add a Bore for these";

/// The count of the held holes that the Drill skips as larger than the
/// tool (P6.7): the holes of each size whose fit is `Larger` under the
/// `ToolSize` rule. A Bore cuts them. `0` for every other op and fit.
fn holes_to_bore(sizes: &[HoleSize]) -> usize {
    sizes
        .iter()
        .filter(|size| size.rule == HoleFitRule::ToolSize && size.fit == Some(HoleFit::Larger))
        .map(|size| size.keys.len())
        .sum()
}

/// The one-line fix action of the Drill hole rows (P6.7, L-17; the
/// "auto magic is a wizard" ruling allows it because it shows only with
/// the rows it fixes): with a size larger than the tool, one line with
/// [`ADD_A_BORE`]. A click sends one `AddBoreFromDrill`.
fn draw_add_bore(
    ui: &mut egui::Ui,
    inputs: &SelectionSectionInputs<'_>,
    sizes: &[HoleSize],
    events: &mut Vec<AppEvent>,
) {
    let count = holes_to_bore(sizes);
    if count == 0 {
        return;
    }
    let text = format!(
        "{count} {} to bore",
        if count == 1 { "hole" } else { "holes" }
    );
    if line_with_action(
        ui,
        "",
        &text,
        tokens::CAUTION,
        "The Drill leaves out each hole larger than the tool. A Bore cuts these holes with a \
         helix of the same tool.",
        Some((
            ADD_A_BORE,
            "Add a Bore after this Drill with a copy of its selection. The Bore cuts the \
             larger holes; the Drill keeps the holes of the tool size.",
        )),
    ) {
        events.push(AppEvent::AddBoreFromDrill(inputs.toolpath_id));
    }
}

/// The one line of a Pocket with Extend to stock (P6.7, ruling O-2): how
/// many open paths the op extends, and the path that cannot extend, with
/// Show me. Core refuses the whole selection for the first path that
/// cannot extend (`RefusalCause::CannotExtend`), so at most one path is
/// refused. The full refusal and its fix stay in the section refusal.
fn draw_extend_summary(
    ui: &mut egui::Ui,
    inputs: &SelectionSectionInputs<'_>,
    report: &SelectionReport,
    events: &mut Vec<AppEvent>,
) {
    if !matches!(inputs.item_use, ItemUse::RegionsExtendOpen { .. })
        || inputs.target != SelectionTarget::Op
    {
        return;
    }
    let extended = report.extended.len();
    let paths = format!(
        "{extended} path{} extended",
        if extended == 1 { "" } else { "s" }
    );
    let refused = report
        .refusal
        .as_ref()
        .filter(|refusal| matches!(refusal.cause, RefusalCause::CannotExtend { .. }));
    let Some(refusal) = refused else {
        line_with_action(
            ui,
            "Open paths",
            &paths,
            tokens::TEXT_BODY,
            "The area beside each open path, out to the stock edge, is cut. The viewport \
             draws each area in the cut colour.",
            None,
        );
        return;
    };
    // The cause with no op and no item: the face starts with the reason.
    let why =
        rs_cam_core::selection::SelectionRefusal::new(None, refusal.cause.clone()).to_string();
    let text = format!("{paths} \u{00B7} 1 refused: {why}");
    let key = refusal.item.as_ref().map(|item| item.key.clone());
    let action = key
        .as_ref()
        .map(|_| (SHOW_ME, "Frame the path that cannot extend."));
    if line_with_action(
        ui,
        "Open paths",
        &text,
        tokens::CAUTION,
        &refusal.to_string(),
        action,
    ) && let Some(key) = key
    {
        show_me(inputs, events, vec![*key]);
    }
}

/// The text of one rule: "This model · layer CUT · closed loops".
fn rule_text(rule: &Rule, models: &[(crate::state::job::ModelId, String)]) -> String {
    let source = match rule.source {
        SourceRef::ToolpathModel => "This model".to_owned(),
        SourceRef::Model(id) => models
            .iter()
            .find(|(m, _)| m.raw() == id.raw())
            .map_or_else(
                || format!("model id {}", id.raw()),
                |(_, name)| name.clone(),
            ),
        SourceRef::Stock => "Stock".to_owned(),
    };
    let filter = &rule.filter;
    let layers = if filter.layers.is_empty() {
        "every layer".to_owned()
    } else {
        format!("layer {}", filter.layers.join(", "))
    };
    let kinds = match &filter.kinds {
        KindFilter::Served => "every item it can cut".to_owned(),
        KindFilter::Only(kinds) => kinds
            .iter()
            .map(|k| kind_noun(*k, 2))
            .collect::<Vec<_>>()
            .join(", "),
    };
    let mut text = format!("{source} \u{00B7} {layers} \u{00B7} {kinds}");
    if let Some(range) = filter.diameter {
        text.push_str(&format!(
            " \u{00B7} \u{00D8} {:.2} \u{00B1} {:.2} mm",
            range.nominal_mm, range.tolerance_mm
        ));
    }
    if let Some(face) = filter.face {
        let facing = match face.facing {
            Facing::Up => "up",
            Facing::Down => "down",
            Facing::Either => "up or down",
        };
        text.push_str(&format!(" \u{00B7} {facing}"));
        if let Some(z) = face.z {
            text.push_str(&format!(" \u{00B7} Z {:.2} to {:.2}", z.min, z.max));
        }
    }
    text
}

/// The text of the "Show me" button of a lost pick (P4.4).
pub(crate) const SHOW_ME: &str = "Show me";
/// The hover end of a note row that answers a click (P4.4).
const SHOW_ME_NOTE_HOVER: &str = "Click to show these items in the viewport.";

/// One line with a truncated text in `color` and, at the right edge, one
/// quiet button (the kit `list_line`). Returns true when the button was
/// clicked.
fn line_with_action(
    ui: &mut egui::Ui,
    name: &str,
    text: &str,
    color: egui::Color32,
    hover: &str,
    action: Option<(&str, &str)>,
) -> bool {
    let actions: Vec<(&str, &str)> = action.into_iter().collect();
    line_with_actions(ui, name, text, color, hover, &actions).is_some()
}

/// [`line_with_action`] with several quiet buttons, in order. Returns the
/// index of the clicked button.
pub(super) fn line_with_actions(
    ui: &mut egui::Ui,
    name: &str,
    text: &str,
    color: egui::Color32,
    hover: &str,
    actions: &[(&str, &str)],
) -> Option<usize> {
    let face = if name.is_empty() {
        text.to_owned()
    } else {
        format!("{name}  {text}")
    };
    list_line(ui, &face, color, hover, actions).clicked
}

/// P4.4: ask the viewport to frame and flash `keys`.
fn show_me(inputs: &SelectionSectionInputs<'_>, events: &mut Vec<AppEvent>, keys: Vec<MatchKey>) {
    events.push(AppEvent::FrameSelectionItems {
        toolpath_id: inputs.toolpath_id,
        keys,
    });
}

/// The notes, the lost picks and the refusal, each one line; the refusal
/// is the one text that wraps (IC1).
fn draw_findings(
    ui: &mut egui::Ui,
    inputs: &SelectionSectionInputs<'_>,
    report: &SelectionReport,
    events: &mut Vec<AppEvent>,
) {
    // A note row with items answers a click: "Show me" frames the items
    // of the note (`note_item_keys`; IC2: the note stays one row). Each
    // table row holds the keys it frames.
    let mut notes = DataTable::new(inputs.salt("selection_notes"));
    let mut row_keys: Vec<Vec<MatchKey>> = Vec::new();
    // FR-2 F32: the items of a kind that the op never cuts are ONE line;
    // the Skipped items list names each layer.
    let never_cut = never_cut_count(inputs, report);
    if never_cut > 0 {
        let face = never_cut_line(inputs.op_label, inputs.item_use, never_cut);
        let keys: Vec<MatchKey> = report
            .notes
            .iter()
            .enumerate()
            .filter(|(_, note)| is_never_cut(note))
            .filter_map(|(index, _)| inputs.note_items.get(index))
            .flatten()
            .cloned()
            .collect();
        let row = DataRow::note(Role::Caution, face.clone());
        notes = notes.row(if keys.is_empty() {
            row.hover(format!("{face}. The Skipped items list names each layer."))
        } else {
            row.hover(format!(
                "{face}. The Skipped items list names each layer. {SHOW_ME_NOTE_HOVER}"
            ))
            .clickable()
        });
        row_keys.push(keys);
    }
    for (index, note) in report.notes.iter().enumerate() {
        if is_never_cut(note) {
            continue;
        }
        let layer = note
            .layer
            .as_deref()
            .map_or_else(String::new, |l| format!(" on layer {l}"));
        let verb = if note.count == 1 { "is" } else { "are" };
        let face = format!(
            "{} {}{layer} {verb} not cut",
            note.count,
            kind_noun(note.kind, note.count)
        );
        let row = DataRow::note(Role::Caution, face);
        let keys = inputs.note_items.get(index).cloned().unwrap_or_default();
        notes = notes.row(if keys.is_empty() {
            row.hover(note.to_string())
        } else {
            row.hover(format!("{note} {SHOW_ME_NOTE_HOVER}"))
                .clickable()
        });
        row_keys.push(keys);
    }
    if let Some(index) = notes.show(ui).clicked
        && let Some(keys) = row_keys.get(index)
        && !keys.is_empty()
    {
        show_me(inputs, events, keys.clone());
    }
    for lost in &report.lost {
        let face = format!("Lost pick: {}", kind_noun(lost.key.kind, 1));
        let hover = format!(
            "A pick matches no item, or more than one, after a re-import: {lost}. \
             Remove it, or import the drawing again."
        );
        let clicked = line_with_actions(
            ui,
            "",
            &face,
            tokens::DANGER,
            &hover,
            &[
                (
                    SHOW_ME,
                    "Frame the stored position of this pick in the viewport.",
                ),
                ("Remove", "Remove this pick from the selection."),
            ],
        );
        if clicked == Some(0) {
            show_me(inputs, events, vec![(*lost.key).clone()]);
        }
        if clicked == Some(1) {
            let picks = inputs
                .selection
                .picks()
                .iter()
                .filter(|k| !k.matches(&lost.key) && **k != *lost.key)
                .cloned()
                .collect();
            let excludes = inputs
                .selection
                .excludes()
                .iter()
                .filter(|k| !k.matches(&lost.key) && **k != *lost.key)
                .cloned()
                .collect();
            let rules = inputs.selection.rules().to_vec();
            inputs.send(
                events,
                SelectionEdit::Replace(Box::new(Selection::from_parts(rules, picks, excludes))),
            );
        }
    }
}

/// The label of the derivation of a selection block (FR-2 F28): the items
/// that the op does not cut.
pub(crate) const SKIPPED_ITEMS: &str = "Skipped items";

/// The derivation: the items that the op leaves out, by kind, and each
/// layer of the items that it never cuts (FR-2 F32). One closed `Details`
/// named [`SKIPPED_ITEMS`] (IC2, F28), with the count of the skipped
/// items. No `Details` when the op leaves nothing out.
fn draw_details(ui: &mut egui::Ui, inputs: &SelectionSectionInputs<'_>, report: &SelectionReport) {
    let skipped: usize = report.skipped_by_kind.values().sum();
    if skipped == 0 {
        return;
    }
    let mut rows: Vec<DataRow> = report
        .skipped_by_kind
        .iter()
        .filter(|(_, n)| **n > 0)
        .map(|(kind, n)| {
            DataRow::value("Not cut", format!("{n} {}", kind_noun(*kind, *n)))
                .hover("Items the rules match that this operation does not cut.")
        })
        .collect();
    for note in report.notes.iter().filter(|note| is_never_cut(note)) {
        let layer = note
            .layer
            .as_deref()
            .map_or_else(|| "No layer".to_owned(), |l| format!("Layer {l}"));
        rows.push(
            DataRow::value(
                layer,
                format!("{} {}", note.count, kind_noun(note.kind, note.count)),
            )
            .hover(note.to_string()),
        );
    }
    Details::new(inputs.salt("selection_details"))
        .label(SKIPPED_ITEMS)
        .count(skipped)
        .show(ui, |ui| {
            DataTable::new(inputs.salt("selection_details_rows"))
                .rows(rows)
                .show(ui);
        });
}
