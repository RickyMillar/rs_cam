//! One selection block of the Geometry tab (geometry selection P4.3, GT-2,
//! GT-3): the summary, the actions, the filter box, the selected list and
//! the notes of one selection of a toolpath.
//!
//! The Geometry tab (`geometry_tab.rs`) draws it for the op selection and,
//! under "Boundary", for the boundary selection (operator ruling GT-Q7).
//!
//! The list shows the SELECTED items only (operator ruling GT-Q6, L-R1):
//! the default row while the selection holds its default rule (L-R2), one
//! group per rule, and one group per picked kind. A group of more than
//! [`GROUP_CAP`] rows starts closed; an open group shows [`GROUP_CAP`]
//! rows, then "Show N more". The rows draw through
//! `egui::ScrollArea::show_rows`, so only the visible rows are laid out.
//!
//! Lead ruling L-1: the block sends `AppEvent::EditToolpathSelection`,
//! `AppEvent::UseAsFloor` and `AppEvent::ClearFloorFace` only. It never
//! writes `entry.operation` or `entry.boundary`. The controller applies
//! each act through one `Command`, so one act is one undo step.
//!
//! The counts, the rows, the notes, the lost picks and the refusal come
//! from the `selection_status` report (the `SelectionStatus` query), which
//! MCP `selection_status` also gives. The "Rule…" offers come from core
//! `ProjectSession::rule_offers` (L-R4: the facts of one picked item).

use std::collections::BTreeSet;

use rs_cam_core::compute::catalog::ItemUse;
use rs_cam_core::ids::ModelId;
use rs_cam_core::selection::{
    Facing, HoleFit, HoleFitRule, ItemKind, KindFilter, MatchKey, MatchedBy, RefusalCause, Rule,
    Selection, SelectionEdit, SourceRef, kind_noun,
};
use rs_cam_core::session::{ItemState, RuleOffer, SelectionReport, SelectionTarget};

use super::SelectionRead;
use crate::state::pick::{OffersFor, PickMode, SelectionUi};
use crate::state::toolpath::ToolpathId;
use crate::ui::AppEvent;
use crate::ui::components::{Button, DataRow, DataTable, Details, Role};
use crate::ui::tokens;

/// The most rows that a group shows before "Show N more", and the count
/// above which a group starts closed (L-R1).
pub(crate) const GROUP_CAP: usize = 8;

/// The hover of the `Pick` button.
const PICK_HOVER: &str = "Click items in the viewport to add them to this selection or to \
    remove them. One Pick is on at a time.";

/// The fix line of a selection that has nothing to cut while the toolpath
/// model holds faces that the reader can cut (geometry selection P4.D).
pub(crate) const PICK_A_FACE_FIX: &str = "Fix: pick a face of the model.";

/// The hover of [`PICK_A_FACE_FIX`].
const PICK_A_FACE_HOVER: &str = "The selection holds nothing that this operation cuts, and this \
    model holds faces. Click Pick below, then click a flat face in the viewport.";

/// True when the selection refuses with nothing to cut (`Empty`) and a
/// face pick is a fix: the toolpath model holds faces, and the reader cuts
/// faces (P4.D).
pub(crate) fn wants_a_face_pick(
    inputs: &SelectionSectionInputs<'_>,
    report: &SelectionReport,
) -> bool {
    matches!(
        report.refusal.as_ref().map(|r| &r.cause),
        Some(RefusalCause::Empty { .. })
    ) && inputs.model_kinds.contains(&ItemKind::Face)
        && ItemKind::Face.serves(inputs.item_use)
}

/// Everything the section reads. The section owns no project state.
pub(crate) struct SelectionSectionInputs<'a> {
    pub toolpath_id: ToolpathId,
    /// The model of the toolpath: a stored selection names it as
    /// `SourceRef::ToolpathModel`.
    pub toolpath_model: ModelId,
    pub target: SelectionTarget,
    /// The stored selection of `target`.
    pub selection: &'a Selection,
    /// The selection that `Reset` restores: the op default, or for the
    /// boundary the value of the "Selection" source button.
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
    /// The op label, for the default row ("nothing that Drill cuts").
    pub op_label: &'static str,
    /// True when a picked face or model ring offers "Use as floor": the op
    /// target of an op that holds a floor (L-R5).
    pub floor_op: bool,
    /// For each note of the report, the keys that "Show me" frames
    /// (`selection_highlight::note_item_keys`): the skipped items of its
    /// kind and layer, or the one face of an at-the-top note.
    pub note_items: &'a [Vec<MatchKey>],
    /// The item kinds that the toolpath model holds
    /// (`state::pick::held_kinds`): the "pick a face" fix reads it (P4.D).
    pub model_kinds: &'a BTreeSet<ItemKind>,
    /// The diameter of the tool of the toolpath (`ToolConfig::diameter`),
    /// for the hole size rows (P6.7): "drilled at" a diameter and the Bore
    /// helix radius. `None` with no tool, and for the boundary.
    pub tool_diameter: Option<f64>,
}

impl SelectionSectionInputs<'_> {
    /// One edit of this selection: the only write of the section (L-1).
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
    let report = match inputs.report {
        Some(Ok(report)) => Some(report.as_ref()),
        Some(Err(error)) => {
            super::tab_badges::wrapped_small_label(ui, error.clone(), tokens::DANGER);
            None
        }
        None => None,
    };

    // ── 1. The summary ─────────────────────────────────────────────
    let mut table = DataTable::new(inputs.salt("selection_counts"));
    table = table.row(cuts_row(report));
    let held = report.map_or(0, |r| r.rows.len());
    table = table.row(
        DataRow::value("Selected", format!("{held} {}", items_word(held)))
            .hover("The items that this selection names: the rule matches and the picks."),
    );
    // The hole sizes (P6.7): one row per size, after the counts.
    let sizes = report.map_or_else(Vec::new, |report| hole_sizes(inputs, report));
    for size in &sizes {
        table = table.row(size.row(inputs.tool_diameter));
    }
    // A hole size row answers a click: Show me frames the holes of that
    // size. The first two rows are the Cuts and Selected rows.
    if let Some(clicked) = table.show(ui).clicked
        && let Some(size) = clicked.checked_sub(2).and_then(|at| sizes.get(at))
    {
        show_me(inputs, events, size.keys.clone());
    }

    // ── 2. The actions ─────────────────────────────────────────────
    ui.horizontal(|ui| {
        let on = ui_state.pick_is_on(inputs.toolpath_id, inputs.target);
        let pick_button = if on {
            Button::primary("Pick")
        } else {
            Button::new("Pick")
        };
        if ui.add(pick_button).on_hover_text(PICK_HOVER).clicked() {
            // L-3: one Pick at a time. A click on this button replaces the
            // mode of any other block.
            ui_state.set_pick(if on {
                None
            } else {
                Some(PickMode::new(
                    inputs.toolpath_id,
                    inputs.target,
                    inputs.item_use,
                ))
            });
        }
        if ui
            .add(Button::new("Reset").enabled(!inputs.is_default()))
            .on_hover_text(match inputs.target {
                SelectionTarget::Op => "Go back to the default selection of the operation.",
                SelectionTarget::Boundary => {
                    "Go back to the selection that the Selection source writes."
                }
            })
            .clicked()
        {
            inputs.send(events, SelectionEdit::Reset);
        }
    });

    // ── 3. The filter box and 4. the selected list ─────────────────
    if let Some(report) = report {
        if report.rows.len() > GROUP_CAP {
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
        let filter = ui_state.list_filter.trim().to_lowercase();
        let groups = list_groups(inputs, report);
        for group in &groups {
            draw_group(ui, inputs, group, &filter, ui_state, events);
        }
    }

    // ── 6. The notes ───────────────────────────────────────────────
    draw_add_bore(ui, inputs, &sizes, events);
    if let Some(report) = report {
        draw_extend_summary(ui, inputs, report, events);
        draw_findings(ui, inputs, report, events);
        // P4.D: a new 2D op on a STEP model can start with nothing to cut.
        // The fix is a face pick; the one Pick is in the action row.
        if wants_a_face_pick(inputs, report) {
            line_with_action(
                ui,
                "",
                PICK_A_FACE_FIX,
                tokens::TEXT_BODY,
                PICK_A_FACE_HOVER,
                None,
            );
        }
        draw_details(ui, inputs, report);
    }
}

/// "item" or "items".
fn items_word(count: usize) -> &'static str {
    if count == 1 { "item" } else { "items" }
}

/// What a row of the selected list offers (L-R4, L-R5, GT-Q8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct RowActs {
    /// "Rule…": the offers from the facts of this picked item.
    pub rule: bool,
    /// "Use as floor": a picked face or model ring of a floor op.
    pub floor: bool,
    /// "×": drop the item. On a rule member, core turns the rule into
    /// picks less this item (GT-Q8, `rule_drop_edit`).
    pub remove: bool,
}

/// One row of the selected list.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ListRow {
    pub key: MatchKey,
    /// The text of the row: the item and its facts.
    pub label: String,
    /// The report state of the item.
    pub state: ItemState,
    pub acts: RowActs,
}

/// What a group of the list stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GroupKind {
    /// The default row (L-R2): the default rule while the selection holds
    /// it. Its rows have no acts.
    Default,
    /// The rule at this index; "×" on the group removes it.
    Rule(usize),
    /// The picks of one kind.
    Picked(ItemKind),
}

/// One group of the selected list: a header row and its item rows.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ListGroup {
    pub kind: GroupKind,
    /// The header text, with no count.
    pub title: String,
    pub rows: Vec<ListRow>,
}

impl ListGroup {
    /// The rows that the filter text keeps (lower case; empty keeps all).
    pub(crate) fn kept<'a>(&'a self, filter: &str) -> Vec<&'a ListRow> {
        self.rows
            .iter()
            .filter(|row| filter.is_empty() || row.label.to_lowercase().contains(filter))
            .collect()
    }

    /// The count text of the header: "24", or "6 of 24" under a filter.
    pub(crate) fn count_text(&self, filter: &str) -> String {
        let kept = self.kept(filter).len();
        if kept == self.rows.len() {
            kept.to_string()
        } else {
            format!("{kept} of {}", self.rows.len())
        }
    }

    /// True when the group starts closed: the default row (L-R2), and a
    /// group of more than [`GROUP_CAP`] rows (L-R1).
    pub(crate) fn starts_closed(&self) -> bool {
        self.kind == GroupKind::Default || self.rows.len() > GROUP_CAP
    }
}

/// The text of one item row: the kind, the size, the place and the layer.
pub(crate) fn row_label(key: &MatchKey, plane_z: Option<f64>) -> String {
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

/// The groups of the selected list (L-R1, L-R2): the default row while the
/// selection holds its default rule, else one group per rule (in rule
/// order), then one group per picked kind. Each row of the report is in
/// one group only, so the rows of the groups ARE the report rows.
pub(crate) fn list_groups(
    inputs: &SelectionSectionInputs<'_>,
    report: &SelectionReport,
) -> Vec<ListGroup> {
    let default = inputs.is_default();
    let row_of = |row: &rs_cam_core::session::SelectedRow, acts: RowActs| ListRow {
        key: row.key.clone(),
        label: row_label(&row.key, row.plane_z),
        state: report.row_state(row),
        acts,
    };
    if default {
        let rows: Vec<ListRow> = report
            .rows
            .iter()
            .map(|row| row_of(row, RowActs::default()))
            .collect();
        let kinds: BTreeSet<ItemKind> = rows.iter().map(|r| r.key.kind).collect();
        let title = if rows.is_empty() {
            format!("Default: nothing that {} cuts", inputs.op_label)
        } else {
            let names: Vec<&str> = kinds.iter().map(|k| kind_noun(*k, 1)).collect();
            format!("Default: every {}", names.join(", "))
        };
        return vec![ListGroup {
            kind: GroupKind::Default,
            title,
            rows,
        }];
    }
    let mut groups = Vec::new();
    for (index, rule) in inputs.selection.rules().iter().enumerate() {
        let rows = report
            .rows
            .iter()
            .filter(|row| {
                matches!(row.by, MatchedBy::Named { rule } | MatchedBy::Default { rule } if rule == index)
            })
            .map(|row| {
                row_of(
                    row,
                    RowActs {
                        remove: true,
                        ..RowActs::default()
                    },
                )
            })
            .collect();
        groups.push(ListGroup {
            kind: GroupKind::Rule(index),
            title: rule_text(rule, inputs.models),
            rows,
        });
    }
    let mut picked: Vec<(ItemKind, Vec<ListRow>)> = Vec::new();
    for row in report.rows.iter().filter(|row| row.by == MatchedBy::Pick) {
        let kind = row.key.kind;
        let floor = inputs.floor_op
            && match kind {
                ItemKind::Face => true,
                ItemKind::Loop => row.key.z.is_some(),
                ItemKind::Path | ItemKind::Point | ItemKind::Circle => false,
            };
        let list_row = row_of(
            row,
            RowActs {
                rule: true,
                floor,
                remove: true,
            },
        );
        match picked.iter_mut().find(|(k, _)| *k == kind) {
            Some((_, rows)) => rows.push(list_row),
            None => picked.push((kind, vec![list_row])),
        }
    }
    for (kind, rows) in picked {
        groups.push(ListGroup {
            kind: GroupKind::Picked(kind),
            title: format!("Picked {}", kind_noun(kind, 2)),
            rows,
        });
    }
    groups
}

/// The open state of a group, kept in egui memory (a view toggle, not
/// content): `(open, show_all)`.
fn group_view(ui: &egui::Ui, id: egui::Id, group: &ListGroup) -> (bool, bool) {
    ui.data(|d| d.get_temp::<(bool, bool)>(id))
        .unwrap_or((!group.starts_closed(), false))
}

/// Draw one group: the header row and, when open, its rows.
fn draw_group(
    ui: &mut egui::Ui,
    inputs: &SelectionSectionInputs<'_>,
    group: &ListGroup,
    filter: &str,
    ui_state: &mut SelectionUi,
    events: &mut Vec<AppEvent>,
) {
    let id = ui.make_persistent_id(inputs.salt(&format!("group {:?}", group.kind)));
    let (mut open, mut show_all) = group_view(ui, id, group);
    let arrow = if open { "\u{25BE}" } else { "\u{25B8}" };
    let mut actions: Vec<(&str, &str)> = vec![(arrow, "Open or close this group.")];
    if let GroupKind::Rule(_) = group.kind {
        actions.push(("\u{2715}", "Remove this rule."));
    }
    let header = format!("{} ({})", group.title, group.count_text(filter));
    let color = if group.kind == GroupKind::Default {
        tokens::TEXT_MUTED
    } else {
        tokens::TEXT_BODY
    };
    match line_with_actions(ui, "", &header, color, &header, &actions) {
        Some(0) => open = !open,
        Some(_) => {
            if let GroupKind::Rule(index) = group.kind {
                inputs.send(events, SelectionEdit::RemoveRule(index));
            }
        }
        None => {}
    }
    if open {
        let kept = group.kept(filter);
        let shown = if show_all {
            kept.len()
        } else {
            kept.len().min(GROUP_CAP)
        };
        let row_height = tokens::ROW_DENSE + ui.spacing().item_spacing.y;
        egui::ScrollArea::vertical()
            .id_salt(inputs.salt(&format!("rows {:?}", group.kind)))
            .max_height(row_height * GROUP_CAP as f32 * 1.5)
            .auto_shrink([false, true])
            .show_rows(ui, row_height, shown, |ui, range| {
                for row in kept.iter().skip(range.start).take(range.len()) {
                    draw_row(ui, inputs, row, ui_state, events);
                }
            });
        if !show_all && kept.len() > GROUP_CAP {
            let more = kept.len() - GROUP_CAP;
            if ui
                .add(Button::quiet(format!("Show {more} more")))
                .on_hover_text("Show every row of this group.")
                .clicked()
            {
                show_all = true;
            }
        }
    }
    ui.data_mut(|d| d.insert_temp(id, (open, show_all)));
}

/// The acts of a row, in order, with their hover.
fn row_actions(acts: RowActs) -> Vec<(RowAct, &'static str, &'static str)> {
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
            "The op cuts down to the plane of this face. The face leaves the picks.",
        ));
    }
    if acts.remove {
        out.push((
            RowAct::Remove,
            "\u{2715}",
            "Remove this item. On a rule member, the rule turns into picks of its other items.",
        ));
    }
    out
}

/// One act of a row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RowAct {
    Rule,
    Floor,
    Remove,
}

/// Draw one item row, its acts, and its open "Rule…" offers.
fn draw_row(
    ui: &mut egui::Ui,
    inputs: &SelectionSectionInputs<'_>,
    row: &ListRow,
    ui_state: &mut SelectionUi,
    events: &mut Vec<AppEvent>,
) {
    let acts = row_actions(row.acts);
    let labels: Vec<(&str, &str)> = acts.iter().map(|(_, l, h)| (*l, *h)).collect();
    let text = format!("{} \u{00B7} {}", row.label, state_word(row.state));
    let color = match row.state {
        ItemState::Cut | ItemState::Island => tokens::TEXT_BODY,
        ItemState::Skipped | ItemState::Excluded | ItemState::NotSelected => tokens::TEXT_MUTED,
    };
    let (clicked, hovered) = line_with_actions_hover(ui, "", &text, color, &text, &labels);
    if hovered {
        // GT-2: the viewport hover buffer draws the row item.
        ui_state.row_hover = Some(row.key.clone());
    }
    match clicked
        .and_then(|index| acts.get(index))
        .map(|(act, _, _)| *act)
    {
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
        Some(RowAct::Remove) => inputs.send(events, SelectionEdit::RemovePick(row.key.clone())),
        None => {}
    }
    let open = ui_state
        .offers_for(inputs.toolpath_id, inputs.target)
        .is_some_and(|o| o.key == row.key);
    if open && draw_offers(ui, inputs, events) {
        ui_state.offers_for = None;
    }
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
        line_with_action(
            ui,
            "",
            "No rule fits this item.",
            tokens::TEXT_MUTED,
            "This item has no fact that a rule can read.",
            None,
        );
        return false;
    }
    let mut added = false;
    for offer in inputs.offers {
        let text = format!("  {} ({})", offer.label, offer.count);
        match &offer.edit {
            Some(edit) => {
                if line_with_action(
                    ui,
                    "",
                    &text,
                    tokens::TEXT_BODY,
                    "The rule replaces the picks that it holds and every rule whose items it \
                     holds.",
                    Some(("Add", "Add this rule.")),
                ) {
                    inputs.send(events, edit.clone());
                    added = true;
                }
            }
            None => {
                line_with_action(
                    ui,
                    "",
                    &text,
                    tokens::TEXT_MUTED,
                    "A rule of this selection holds these items already.",
                    None,
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
    let mut parts: Vec<String> = report
        .counts
        .iter()
        .filter(|(_, n)| **n > 0)
        .map(|(kind, n)| {
            let down = report.faces_by_facing.down;
            if *kind == ItemKind::Face && down > 0 {
                format!("{n} {} ({down} down)", kind_noun(*kind, *n))
            } else {
                format!("{n} {}", kind_noun(*kind, *n))
            }
        })
        .collect();
    if report.islands > 0 {
        let n = report.islands;
        parts.push(format!("{n} island{}", if n == 1 { "" } else { "s" }));
    }
    (!parts.is_empty()).then(|| parts.join(" \u{00B7} "))
}

/// The "Cuts" row: the counts and one status chip.
fn cuts_row(report: Option<&SelectionReport>) -> DataRow {
    let Some(report) = report else {
        return DataRow::not_measured("Cuts");
    };
    // P4.7: the flat faces of an STL are not computed yet. No count and no
    // refusal; the job fills the cache, and Generate computes them too.
    if !report.faces_not_ready.is_empty() {
        return DataRow::value("Cuts", crate::ui::pick_bar::COMPUTING_FACES)
            .hover(
                "The flat faces of the model are not computed yet. A background job \
                 computes them; Generate also computes them.",
            )
            .status(Role::Info, "Wait");
    }
    let text = counts_text(report).unwrap_or_else(|| "nothing".to_owned());
    let findings = report.notes.len() + report.lost.len();
    let row = DataRow::value("Cuts", text.clone()).hover(text);
    if report.refusal.is_some() {
        row.status(Role::Danger, "Refused")
    } else if findings > 0 {
        row.status(Role::Caution, findings.to_string())
    } else {
        row.status(Role::Ok, "OK")
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
        let hover = format!(
            "{word}. The fit compares the hole with {tool}, within the size tolerance of \
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
                (Role::Caution, "larger than the tool".to_owned())
            }
            (HoleFitRule::ToolSize | HoleFitRule::None, HoleFit::LeavesCore) => {
                (Role::Caution, "leaves a core".to_owned())
            }
            (_, HoleFit::Smaller) => (Role::Caution, "smaller than the tool".to_owned()),
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

/// One line with a truncated label in `color` and, at the right edge, one
/// quiet button. Returns true when the button was clicked.
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
    line_with_actions_hover(ui, name, text, color, hover, actions).0
}

/// [`line_with_actions`], and true when the pointer is over the line.
fn line_with_actions_hover(
    ui: &mut egui::Ui,
    name: &str,
    text: &str,
    color: egui::Color32,
    hover: &str,
    actions: &[(&str, &str)],
) -> (Option<usize>, bool) {
    let mut clicked = None;
    let line = ui.horizontal(|ui| {
        // Each button is at least as wide as its label and padding.
        let action_width: f32 = actions
            .iter()
            .map(|(label, _)| action_width(ui, label))
            .sum();
        let line_width = (ui.available_width() - action_width).max(0.0);
        ui.allocate_ui_with_layout(
            egui::vec2(line_width, tokens::ROW_DENSE),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.set_width(line_width);
                let face = if name.is_empty() {
                    text.to_owned()
                } else {
                    format!("{name}  {text}")
                };
                ui.add(
                    egui::Label::new(egui::RichText::new(face).color(color))
                        .truncate()
                        .show_tooltip_when_elided(false),
                )
                .on_hover_text(hover);
            },
        );
        for (index, (label, action_hover)) in actions.iter().enumerate() {
            if ui
                .add(Button::quiet(*label))
                .on_hover_text(*action_hover)
                .clicked()
            {
                clicked = Some(index);
            }
        }
    });
    let hovered = ui.rect_contains_pointer(line.response.rect);
    (clicked, hovered)
}

/// The width a quiet button of `label` takes in a line, with its spacing:
/// at least `SPACE_7`, as before P4.4.
fn action_width(ui: &egui::Ui, label: &str) -> f32 {
    let font = egui::TextStyle::Button.resolve(ui.style());
    let text = ui
        .painter()
        .layout_no_wrap(label.to_owned(), font, tokens::TEXT_BODY)
        .size()
        .x;
    let padding = ui.spacing().button_padding.x;
    (text + 2.0 * padding).max(tokens::SPACE_7) + ui.spacing().item_spacing.x
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
    // of the note (`note_item_keys`; IC2: the note stays one row).
    let mut notes = DataTable::new(inputs.salt("selection_notes"));
    for (index, note) in report.notes.iter().enumerate() {
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
        let has_items = inputs.note_items.get(index).is_some_and(|k| !k.is_empty());
        notes = notes.row(if has_items {
            row.hover(format!("{note} {SHOW_ME_NOTE_HOVER}"))
                .clickable()
        } else {
            row.hover(note.to_string())
        });
    }
    if let Some(index) = notes.show(ui).clicked
        && let Some(keys) = inputs.note_items.get(index)
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
    if let Some(refusal) = &report.refusal {
        super::tab_badges::wrapped_small_label(ui, refusal.to_string(), tokens::DANGER);
    }
}

/// The derivation: what the reader leaves out, and how many items the
/// selection holds. One closed `Details` (IC2).
fn draw_details(ui: &mut egui::Ui, inputs: &SelectionSectionInputs<'_>, report: &SelectionReport) {
    let mut rows: Vec<DataRow> = report
        .skipped_by_kind
        .iter()
        .filter(|(_, n)| **n > 0)
        .map(|(kind, n)| {
            DataRow::value("Not cut", format!("{n} {}", kind_noun(*kind, *n)))
                .hover("Items the rules match that this operation does not cut.")
        })
        .collect();
    rows.push(
        DataRow::value("Items held", report.selected.len().to_string())
            .hover("The items the selection holds, before the operation reads them."),
    );
    Details::new(inputs.salt("selection_details"))
        .count(rows.len())
        .show(ui, |ui| {
            DataTable::new(inputs.salt("selection_details_rows"))
                .rows(rows)
                .show(ui);
        });
}
