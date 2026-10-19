//! The Selection section (geometry selection P4.3): one component shows and
//! edits one selection of a toolpath.
//!
//! It serves the op selection of every op that holds one (the Geometry tab)
//! and the boundary selection of every op (under "Machining Boundary", when
//! the boundary source is `Selection`). It replaces the drill hole
//! selector's Select all, Clear, layer combo and refusal, the face picker
//! block, and the "Outline model" combo of the boundary.
//!
//! Lead ruling L-1: the section sends `AppEvent::EditToolpathSelection`
//! only. It never writes `entry.operation` or `entry.boundary`. The
//! controller applies the edit through `Command::EditSelection`, so the
//! inspector undo snapshot covers it.
//!
//! Lead ruling L-2: the section owns the refusal text and its fix. The
//! footer row of `precondition.selection_refused` shows one line that names
//! this section.
//!
//! The counts, the notes, the lost picks and the refusal come from the
//! `selection_status` report (the `SelectionStatus` query), which MCP
//! `selection_status` also gives.

use std::collections::BTreeSet;

use rs_cam_core::compute::catalog::ItemUse;
use rs_cam_core::ids::ModelId;
use rs_cam_core::selection::{
    FaceFilter, Facing, Filter, ItemKind, KindFilter, MatchKey, Rule, Selection, SelectionEdit,
    SourceRef, ZRange, kind_noun,
};
use rs_cam_core::session::{SelectionReport, SelectionTarget};

use super::{RuleSource, SelectionRead};
use crate::state::pick::{DEFAULT_CIRCLE_RANGE, PickMode, RuleDraft, SelectionUi};
use crate::state::toolpath::ToolpathId;
use crate::ui::AppEvent;
use crate::ui::components::{
    Button, Card, ChoiceRow, DataRow, DataTable, Details, Role, SectionHeader, UiExt as _, ValueRow,
};
use crate::ui::tokens;

/// The kinds a `+ Rule` draft offers, in the order of the row, with the
/// label of each check box.
const RULE_KINDS: [(ItemKind, &str); 5] = [
    (ItemKind::Loop, "loops"),
    (ItemKind::Path, "open"),
    (ItemKind::Circle, "circles"),
    (ItemKind::Point, "points"),
    (ItemKind::Face, "faces"),
];

/// The three facings of a face rule.
const FACINGS: &[(Facing, &str)] = &[
    (Facing::Up, "Up"),
    (Facing::Down, "Down"),
    (Facing::Either, "Either"),
];

/// The hover of the `Pick` button.
const PICK_HOVER: &str = "Click items in the viewport to add them to this selection or to \
    remove them. One Pick is on at a time.";

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
    /// The sources of a `+ Rule` draft (the `ListItems` query). Empty while
    /// no draft of this toolpath is open.
    pub sources: &'a [RuleSource],
    /// P4.4: for each note of the report, the keys of the skipped items of
    /// its kind and layer (`selection_highlight::note_item_keys`). "Show
    /// me" frames them.
    pub note_items: &'a [Vec<MatchKey>],
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

/// Draw the section. `ui_state` holds the pick mode and the `+ Rule`
/// draft; `events` receives the edits.
pub(crate) fn draw_selection_section(
    ui: &mut egui::Ui,
    inputs: &SelectionSectionInputs<'_>,
    ui_state: &mut SelectionUi,
    events: &mut Vec<AppEvent>,
) {
    if inputs.target == SelectionTarget::Op {
        SectionHeader::new("Selection")
            .hover(
                "The items this operation cuts: rules, then picks. Pick items in the \
                 viewport, or add a rule by layer and kind.",
            )
            .show(ui);
    }

    let report = match inputs.report {
        Some(Ok(report)) => Some(report.as_ref()),
        Some(Err(error)) => {
            super::tab_badges::wrapped_small_label(ui, error.clone(), tokens::DANGER);
            None
        }
        None => None,
    };

    // ── The counts row ─────────────────────────────────────────────
    let mut table = DataTable::new(inputs.salt("selection_counts"));
    table = table.row(cuts_row(report));
    // ── The rules ──────────────────────────────────────────────────
    table.show(ui);
    draw_rules(ui, inputs, events);
    // ── The picks ──────────────────────────────────────────────────
    draw_picks(ui, inputs, events);
    // ── The notes and the lost picks ───────────────────────────────
    if let Some(report) = report {
        draw_findings(ui, inputs, report, events);
    }

    // ── The actions ────────────────────────────────────────────────
    ui.horizontal(|ui| {
        let draft_open = ui_state
            .draft_for(inputs.toolpath_id, inputs.target)
            .is_some();
        let rule_button = if draft_open {
            Button::primary("+ Rule")
        } else {
            Button::new("+ Rule")
        };
        if ui
            .add(rule_button)
            .on_hover_text("Add a rule: the items of one source by layer and kind.")
            .clicked()
        {
            ui_state.rule_draft = if draft_open {
                None
            } else {
                // The inputs read the item sources only while a draft is
                // open, so the next frame must come at once.
                ui.ctx().request_repaint();
                Some(new_draft(inputs))
            };
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
        let on = ui_state.pick_is_on(inputs.toolpath_id, inputs.target);
        let pick_button = if on {
            Button::primary("Pick")
        } else {
            Button::new("Pick")
        };
        if ui.add(pick_button).on_hover_text(PICK_HOVER).clicked() {
            // L-3: one Pick at a time. A click on this button replaces the
            // mode of any other section.
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
    });

    if let Some(draft) = ui_state.draft_for(inputs.toolpath_id, inputs.target) {
        let mut close = false;
        draw_rule_draft(ui, inputs, draft, events, &mut close);
        if close {
            ui_state.rule_draft = None;
        }
    }

    if let Some(report) = report {
        draw_details(ui, inputs, report);
    }
}

/// The count of each kind the reader uses, and the islands: "14 closed
/// loops · 3 islands · 2 circles". `None` when nothing is cut.
pub(crate) fn counts_text(report: &SelectionReport) -> Option<String> {
    let mut parts: Vec<String> = report
        .counts
        .iter()
        .filter(|(_, n)| **n > 0)
        .map(|(kind, n)| format!("{n} {}", kind_noun(*kind, *n)))
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

/// The text of one rule: "This model · layer CUT · closed loops".
fn rule_text(rule: &Rule, models: &[(crate::state::job::ModelId, String)]) -> String {
    let source = match rule.source {
        SourceRef::ToolpathModel => "This model".to_owned(),
        SourceRef::Model(id) => models
            .iter()
            .find(|(m, _)| m.0 == id.0)
            .map_or_else(|| format!("model id {}", id.0), |(_, name)| name.clone()),
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
fn line_with_actions(
    ui: &mut egui::Ui,
    name: &str,
    text: &str,
    color: egui::Color32,
    hover: &str,
    actions: &[(&str, &str)],
) -> Option<usize> {
    let mut clicked = None;
    ui.horizontal(|ui| {
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
    clicked
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

/// One row per rule, with a remove button. The default selection says so.
fn draw_rules(ui: &mut egui::Ui, inputs: &SelectionSectionInputs<'_>, events: &mut Vec<AppEvent>) {
    let rules = inputs.selection.rules();
    if rules.is_empty() {
        line_with_action(
            ui,
            "Rules",
            "none",
            tokens::TEXT_MUTED,
            "No rule: only the picks are in the selection.",
            None,
        );
        return;
    }
    let default = inputs.is_default();
    for (index, rule) in rules.iter().enumerate() {
        let mut text = rule_text(rule, inputs.models);
        if default {
            text.push_str(" (default)");
        }
        let name = if index == 0 { "Rules" } else { "" };
        let color = if default {
            tokens::TEXT_MUTED
        } else {
            tokens::TEXT_BODY
        };
        if line_with_action(
            ui,
            name,
            &text,
            color,
            &text,
            Some(("\u{2715}", "Remove this rule.")),
        ) {
            inputs.send(events, SelectionEdit::RemoveRule(index));
        }
    }
}

/// The pick counts and their Clear button: Clear keeps the rules only.
fn draw_picks(ui: &mut egui::Ui, inputs: &SelectionSectionInputs<'_>, events: &mut Vec<AppEvent>) {
    let added = inputs.selection.picks().len();
    let removed = inputs.selection.excludes().len();
    if added == 0 && removed == 0 {
        return;
    }
    let text = format!("{added} added \u{00B7} {removed} removed");
    if line_with_action(
        ui,
        "Picks",
        &text,
        tokens::TEXT_BODY,
        "Items added or removed by hand, on top of the rules.",
        Some(("Clear", "Remove every pick. The rules stay.")),
    ) {
        let rules = inputs.selection.rules().to_vec();
        inputs.send(
            events,
            SelectionEdit::Replace(Box::new(Selection::from_rules(rules))),
        );
    }
}

/// The notes, the lost picks and the refusal, each one line; the refusal
/// is the one text that wraps (IC1).
fn draw_findings(
    ui: &mut egui::Ui,
    inputs: &SelectionSectionInputs<'_>,
    report: &SelectionReport,
    events: &mut Vec<AppEvent>,
) {
    // P4.4: a note row with items answers a click: "Show me" frames the
    // skipped items of its kind and layer (IC2: the note stays one row).
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
            show_me(inputs, events, vec![lost.key.clone()]);
        }
        if clicked == Some(1) {
            let picks = inputs
                .selection
                .picks()
                .iter()
                .filter(|k| !k.matches(&lost.key) && **k != lost.key)
                .cloned()
                .collect();
            let excludes = inputs
                .selection
                .excludes()
                .iter()
                .filter(|k| !k.matches(&lost.key) && **k != lost.key)
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

/// A new `+ Rule` draft: the toolpath model, every layer, the kinds the
/// reader serves.
fn new_draft(inputs: &SelectionSectionInputs<'_>) -> RuleDraft {
    RuleDraft {
        toolpath_id: inputs.toolpath_id,
        target: inputs.target,
        source: SourceRef::ToolpathModel,
        layers: BTreeSet::new(),
        kinds: RULE_KINDS
            .iter()
            .map(|(kind, _)| *kind)
            .filter(|kind| kind.serves(inputs.item_use) && *kind != ItemKind::Face)
            .collect(),
        diameter: None,
        facing: Facing::Up,
        z: None,
    }
}

/// The rule a draft gives. A diameter range or a face filter admits no
/// other kind, so the draft offers each only when it is the one kind.
pub(crate) fn draft_rule(draft: &RuleDraft) -> Rule {
    let one_kind = |kind: ItemKind| draft.kinds.len() == 1 && draft.kinds.contains(&kind);
    Rule {
        source: draft.source,
        filter: Filter {
            layers: draft.layers.iter().cloned().collect(),
            kinds: KindFilter::Only(draft.kinds.iter().copied().collect()),
            diameter: draft.diameter.filter(|_| one_kind(ItemKind::Circle)),
            face: one_kind(ItemKind::Face).then_some(FaceFilter {
                facing: draft.facing,
                z: draft.z,
            }),
        },
    }
}

/// The `+ Rule` card. `close` goes true on Cancel and on Add.
fn draw_rule_draft(
    ui: &mut egui::Ui,
    inputs: &SelectionSectionInputs<'_>,
    draft: &mut RuleDraft,
    events: &mut Vec<AppEvent>,
    close: &mut bool,
) {
    Card::new().show(ui, |ui| {
        ui.label(crate::ui::components::text::body_strong("Add a rule"));
        // A toolpath model with no item (a mesh) is not a source: the
        // draft then starts on the first source that holds items.
        if !inputs.sources.iter().any(|s| s.source == draft.source)
            && let Some(first) = inputs.sources.first()
        {
            draft.source = first.source;
            draft.layers.clear();
        }
        let source = inputs.sources.iter().find(|s| s.source == draft.source);
        // Source.
        ui.horizontal(|ui| {
            ui.label("Source");
            let current = source.map_or("(no source)", |s| s.name.as_str());
            egui::ComboBox::from_id_salt(inputs.salt("rule_source"))
                .truncate()
                .selected_text(current)
                .show_ui(ui, |ui| {
                    for candidate in inputs.sources {
                        let selected = candidate.source == draft.source;
                        if ui
                            .selectable_label(selected, candidate.name.as_str())
                            .clicked()
                            && !selected
                        {
                            draft.source = candidate.source;
                            draft.layers.clear();
                        }
                    }
                });
        });
        // Layers.
        let layers = source.map_or(&[][..], |s| s.layers.as_slice());
        if layers.is_empty() {
            ui.label(egui::RichText::new("Layers: none in this source").color(tokens::TEXT_MUTED));
        } else {
            ui.horizontal_wrapped(|ui| {
                ui.label("Layers");
                for layer in layers {
                    let mut on = draft.layers.contains(layer);
                    if ui
                        .checkbox(&mut on, layer.as_str())
                        .on_hover_text("No layer checked: every layer.")
                        .changed()
                    {
                        if on {
                            draft.layers.insert(layer.clone());
                        } else {
                            draft.layers.remove(layer);
                        }
                    }
                }
            });
        }
        // Kinds: only the kinds the reader serves and the source holds.
        ui.horizontal_wrapped(|ui| {
            ui.label("Kinds");
            for (kind, label) in RULE_KINDS {
                let served = kind.serves(inputs.item_use);
                let held = source.is_some_and(|s| s.kinds.contains(&kind));
                let mut on = draft.kinds.contains(&kind);
                let hover = if !served {
                    format!("This selection does not cut {}.", kind_noun(kind, 2))
                } else if !held {
                    format!("This source holds no {}.", kind_noun(kind, 2))
                } else {
                    format!("The rule takes the {}.", kind_noun(kind, 2))
                };
                if ui
                    .add_enabled(served && held, egui::Checkbox::new(&mut on, label))
                    .on_hover_text(hover)
                    .on_disabled_hover_text(hover_disabled(served, kind))
                    .changed()
                {
                    if on {
                        draft.kinds.insert(kind);
                    } else {
                        draft.kinds.remove(&kind);
                    }
                }
            }
        });
        // The circle diameter, when the rule takes circles only.
        if draft.kinds.len() == 1 && draft.kinds.contains(&ItemKind::Circle) {
            let mut limit = draft.diameter.is_some();
            if ui
                .checkbox(&mut limit, "Limit the circle diameter")
                .changed()
            {
                draft.diameter = limit.then_some(DEFAULT_CIRCLE_RANGE);
            }
            if let Some(range) = draft.diameter.as_mut() {
                ui.param_grid(inputs.salt("rule_diameter"), |ui| {
                    ValueRow::new(
                        "Circle \u{00D8}:",
                        &mut range.nominal_mm,
                        " mm",
                        0.05,
                        0.0..=10_000.0,
                    )
                    .show(ui);
                    ValueRow::new(
                        "Tolerance \u{00B1}:",
                        &mut range.tolerance_mm,
                        " mm",
                        0.01,
                        0.0..=100.0,
                    )
                    .show(ui);
                });
            }
        }
        // The face filter, when the rule takes faces only.
        if draft.kinds.len() == 1 && draft.kinds.contains(&ItemKind::Face) {
            ui.add(
                ChoiceRow::new("Faces", &mut draft.facing, FACINGS)
                    .hover("Up: the face points up in the setup. Down: it points down."),
            );
            let mut limit = draft.z.is_some();
            if ui.checkbox(&mut limit, "Limit the face Z").changed() {
                draft.z = limit.then_some(ZRange {
                    min: -10.0,
                    max: 0.0,
                });
            }
            if let Some(z) = draft.z.as_mut() {
                ui.param_grid(inputs.salt("rule_face_z"), |ui| {
                    ValueRow::new("Z from:", &mut z.min, " mm", 0.1, -10_000.0..=10_000.0).show(ui);
                    ValueRow::new("Z to:", &mut z.max, " mm", 0.1, -10_000.0..=10_000.0).show(ui);
                });
            }
        }
        ui.horizontal(|ui| {
            if ui.add(Button::quiet("Cancel")).clicked() {
                *close = true;
            }
            let can_add = !draft.kinds.is_empty() && source.is_some();
            if ui
                .add(Button::primary("Add").enabled(can_add))
                .on_hover_text("Add the rule to this selection.")
                .clicked()
            {
                // Under the default, the first rule replaces it, so one act
                // gives one set of items (the drill's first layer pick).
                // Core owns that rule (P4.5): the viewport shift-click
                // reads the same function.
                if let Some(edit) = SelectionEdit::add_rules(
                    inputs.selection,
                    inputs.op_default,
                    inputs.toolpath_model,
                    vec![draft_rule(draft)],
                ) {
                    inputs.send(events, edit);
                }
                *close = true;
            }
        });
    });
}

/// The hover of a disabled kind check box.
fn hover_disabled(served: bool, kind: ItemKind) -> String {
    if served {
        format!("This source holds no {}.", kind_noun(kind, 2))
    } else {
        format!("This selection does not cut {}.", kind_noun(kind, 2))
    }
}
