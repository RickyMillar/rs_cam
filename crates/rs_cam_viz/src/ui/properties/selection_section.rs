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
    FaceFilter, Facing, Filter, HoleFit, HoleFitRule, ItemKind, KindFilter, MatchKey, RefusalCause,
    Rule, Selection, SelectionEdit, SourceRef, ZRange, kind_noun,
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

/// The fix line of a selection that has nothing to cut while the toolpath
/// model holds faces that the reader can cut (geometry selection P4.D).
pub(crate) const PICK_A_FACE_FIX: &str = "Fix: pick a face of the model.";

/// The hover of [`PICK_A_FACE_FIX`].
const PICK_A_FACE_HOVER: &str = "The default rule takes the items of a drawing, and this model \
    holds faces only. Click Pick below, then click a flat face in the viewport.";

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
    /// The sources of a `+ Rule` draft (the `ListItems` query). Empty while
    /// no draft of this toolpath is open.
    pub sources: &'a [RuleSource],
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
    // ── The hole sizes (P6.7): one row per size, after the counts ──
    let sizes = report.map_or_else(Vec::new, |report| hole_sizes(inputs, report));
    for size in &sizes {
        table = table.row(size.row(inputs.tool_diameter));
    }
    // The rows after the Cuts row answer a click: Show me frames the
    // holes of that size.
    if let Some(clicked) = table.show(ui).clicked
        && let Some(size) = clicked.checked_sub(1).and_then(|at| sizes.get(at))
    {
        show_me(inputs, events, size.keys.clone());
    }
    // ── The open paths of a Pocket Extend (P6.7) ───────────────────
    if let Some(report) = report {
        draw_extend_summary(ui, inputs, report, events);
    }
    // ── The rules ──────────────────────────────────────────────────
    draw_rules(ui, inputs, events);
    // ── The picks ──────────────────────────────────────────────────
    draw_picks(ui, inputs, events);
    // ── The notes and the lost picks ───────────────────────────────
    if let Some(report) = report {
        draw_findings(ui, inputs, report, events);
        // P4.D: a new 2D op on a STEP model starts with the default rule,
        // which takes no face (D2), so it has nothing to cut. The fix is a
        // face pick. The line names the fix and has no button: the one
        // Pick is in the action row below, which also shows when Pick is
        // on and turns it off.
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
        show_me(inputs, events, vec![key]);
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
