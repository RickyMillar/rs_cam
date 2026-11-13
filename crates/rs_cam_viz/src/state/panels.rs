//! Panel drafts that must survive a frame, and the one cached parse.
//!
//! # What the finding was (UI-09)
//!
//! Forty `insert_temp` / `get_temp` calls across ten files held panel state
//! in egui's temporary memory, beside a `state/` module that
//! `state/CLAUDE.md` names as the home of GUI state and that already holds
//! `history.tool_draft`, `history.stock_draft` and `history.post_snapshot`
//! for exactly this job. `machine_panel.rs` alone held sixteen.
//!
//! The GRBL importer was the sharp end. `draw_grbl_import` ran
//! `MachineKinematics::from_grbl_settings(&buf)` unconditionally inside the
//! draw, so the whole pasted `$$` dump was re-parsed on EVERY frame the
//! disclosure was open, and the buffer was cloned into `ui.data` on every
//! keystroke.
//!
//! # What lives here
//!
//! The drafts with content: text the operator typed, and the status line a
//! panel prints after an action. State that is a view toggle, a drag index
//! or a modal's own shape stays in egui memory; `panel_drafts_leave_egui_memory_ui09`
//! holds the list and the reason for each.

use rs_cam_core::machine::kinematics::{GrblImport, MachineKinematics};

use super::job::SetupId;

/// The pasted GRBL `$$` dump, its status line, and the parse of it.
///
/// The parse is cached on the exact text it was made from, so a frame that
/// changes nothing re-uses it. A string compare replaces a full parse of
/// the dump.
#[derive(Debug, Default, Clone)]
pub struct GrblImportDraft {
    /// The pasted text. The `TextEdit` writes it in place.
    pub buffer: String,
    /// What the panel printed after the last action.
    pub status: String,
    /// The parse, and the exact text it was made from.
    cached: Option<(String, GrblImport)>,
    /// How many times the parser has actually run.
    ///
    /// This is the instrument `panel_drafts_leave_egui_memory_ui09` reads:
    /// a frame that does not change the buffer must not raise it. It is
    /// state, not a measurement of the machine, so it carries no units.
    pub parses: u64,
}

impl GrblImportDraft {
    /// The parse of the current buffer, or `None` when the buffer is blank.
    ///
    /// Parses only when the buffer differs from the text the cached parse
    /// was made from.
    pub fn parsed(&mut self) -> Option<&GrblImport> {
        if self.buffer.trim().is_empty() {
            self.cached = None;
            return None;
        }
        let stale = match &self.cached {
            Some((text, _)) => text != &self.buffer,
            None => true,
        };
        if stale {
            let parsed = MachineKinematics::from_grbl_settings(&self.buffer);
            self.parses += 1;
            self.cached = Some((self.buffer.clone(), parsed));
        }
        self.cached.as_ref().map(|(_, parsed)| parsed)
    }

    /// Drop the text and its parse. The status line is the caller's.
    pub fn clear(&mut self) {
        self.buffer.clear();
        self.cached = None;
    }
}

/// A pending destructive action requested from either setup surface.
#[derive(Debug, Clone)]
pub struct SetupRemovalConfirmation {
    pub setup_id: SetupId,
    pub setup_name: String,
}

/// The setup panel's stock-change editor (S4): the draft the operator edits
/// and the refusal of the last apply.
///
/// A new change is not in the session until it passes the validation, so
/// the draft lives here and not in a selection.
#[derive(Debug, Clone)]
pub struct StockChangeEditor {
    /// The setup the draft belongs to.
    pub setup_id: SetupId,
    /// `true` for a new change (Apply adds it), `false` for an edit of a
    /// change the setup holds (Apply replaces it).
    pub is_new: bool,
    /// The record the operator edits.
    pub draft: rs_cam_core::compute::stock_change::StockChange,
    /// The refusal of the last apply, in the core's words. `None` before
    /// an apply, and after the operator edits the draft again.
    pub refusal: Option<String>,
}

/// The obstacle editor (K2): the draft the operator edits on the
/// `Selection::Obstacle` page, and the refusal of the last apply.
///
/// A kind switch to an outline or a model is not valid until the operator
/// picks a model, so the draft lives here and is applied only on Apply.
#[derive(Debug, Clone)]
pub struct ObstacleEditor {
    /// The setup that holds the obstacle.
    pub setup_id: SetupId,
    /// The obstacle the draft edits.
    pub obstacle_id: rs_cam_core::ids::ObstacleId,
    /// The record the operator edits.
    pub draft: rs_cam_core::compute::obstacle::Obstacle,
    /// The refusal of the last apply, in the core's words. `None` before
    /// an apply, and after the operator edits the draft again.
    pub refusal: Option<String>,
}

/// H2: the machine head editor's draft. The draft is the whole head; Apply
/// writes it through `Command::SetMachineHead`.
#[derive(Debug, Clone, PartialEq)]
pub struct HeadEditor {
    /// The head the operator edits.
    pub draft: rs_cam_core::machine::head::MachineHead,
    /// The refusal of the last apply, in the core's words. `None` before
    /// an apply, and after the operator edits the draft again.
    pub refusal: Option<String>,
}

/// K2: the obstacle `obstacle_id` of the setup is gone. Close an editor
/// open on it. When `focus` is that obstacle, return its setup as the new
/// focus (the S4 rule); the caller writes it through the focus door. The
/// controller door, the undo and the MCP remove all call it.
#[must_use]
pub fn forget_obstacle(
    panels: &mut PanelDrafts,
    focus: &super::selection::Selection,
    setup_id: SetupId,
    obstacle_id: rs_cam_core::ids::ObstacleId,
) -> Option<super::selection::Selection> {
    if panels
        .obstacle_editor
        .as_ref()
        .is_some_and(|e| e.setup_id == setup_id && e.obstacle_id == obstacle_id)
    {
        panels.obstacle_editor = None;
    }
    (*focus == super::selection::Selection::Obstacle(setup_id, obstacle_id))
        .then_some(super::selection::Selection::Setup(setup_id))
}

/// Every panel draft that outlives one frame.
#[derive(Debug, Default, Clone)]
pub struct PanelDrafts {
    /// The Machine panel's "Save as" name and the line under the row.
    pub machine_library_name: String,
    pub machine_library_status: String,
    /// The Machine panel's GRBL `$$` importer.
    pub grbl: GrblImportDraft,
    /// The Tool panel's "Save to library" catalog name and status line.
    pub tool_catalog_name: String,
    pub tool_catalog_status: String,
    /// The setup deletion confirmation, shared by the inspector and setup rail.
    pub pending_setup_removal: Option<SetupRemovalConfirmation>,
    /// The setup panel's stock-change editor, when it is open.
    pub stock_change_editor: Option<StockChangeEditor>,
    /// The obstacle editor (K2), when an obstacle is selected.
    pub obstacle_editor: Option<ObstacleEditor>,
    /// The machine head editor (H2), while a draft differs from the stored
    /// head or holds a refusal.
    pub head_editor: Option<HeadEditor>,
    /// The head part whose rows show in the Head section (H2). A view
    /// choice, but typed here beside the draft it indexes.
    pub head_part_open: Option<(crate::ui::properties::head::PartList, usize)>,
    /// The pin the operator clicked in the viewport (index into the stock
    /// pins). The viewport draws its circle twice, and the wizard marks it
    /// on the plan view. A pin count below the index clears it.
    pub selected_pin: Option<usize>,
    /// The pin wizard (R4), when it is open. It holds its own draft pins,
    /// so Cancel changes nothing.
    pub pin_wizard: Option<crate::ui::pin_wizard::PinWizardState>,
    /// The import dialog of a drawing (P4.8), when it is open. Cancel adds
    /// nothing; Import adds the model of the preview.
    pub import_dialog: Option<ImportDialog>,
    /// The tolerance of the model panel's "Join all within" (P4.9), mm.
    /// `None` until the operator sets it: the panel shows
    /// [`DEFAULT_JOIN_ALL_MM`].
    pub join_all_mm: Option<f64>,
    /// The viewport popover of an open path (P4.9), while it is open.
    pub drawing_popover: Option<DrawingPopover>,
    /// E5: the rule menu of a right-click on an item in pick mode, while
    /// it is open.
    pub pick_menu: Option<crate::ui::pick_bar::PickMenu>,
    /// The value in flight of each committed number field
    /// (`ui::import_modal::committed_number`: the import dialog, the model
    /// panel's scale, join tolerance and "Join all within"), by widget id,
    /// while the field is dragged or has focus (ui09).
    pub number_drafts: NumberDrafts,
    /// The last heights edit of the selected toolpath that broke the plane
    /// order (F033; K6: written and warned, not refused). The Heights tab
    /// draws it as the state of the row it edited.
    pub heights_refusal: HeightsRefusal,
    /// G-C (D-2): the `⟲` badge of a containment the `Rest Regions` pick
    /// set to `Inside`, scoped to its toolpath.
    pub containment_autocorrect: ContainmentAutoCorrect,
    /// K2b (feeds contract P5b): the candidate ring of the envelope plot of
    /// each toolpath, a point that nothing has written yet. The Passes tab
    /// writes it on a click or a drag; Esc and an apply drop it.
    pub envelope_candidates: EnvelopeCandidates,
    /// P29-10 10f: the "Limits" drawer of the Suggest card on the Passes
    /// tab is open. Closed by default; MCP `set_ui_view card_limits` and a
    /// click on the drawer header write it.
    pub card_limits_open: bool,
}

/// K2b (feeds contract P5b, live link): the candidate point of the safe
/// envelope plot, per toolpath. A candidate writes nothing: the session
/// changes only on the ⚡ apply (`AppEvent::ApplyFeedsPick`). The
/// nomogram draws the speeds of the candidate while it exists.
///
/// It is GUI state here, not egui memory (UI-09), so a memo refresh or a
/// redraw does not reset it.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct EnvelopeCandidates {
    points:
        std::collections::BTreeMap<rs_cam_core::ToolpathId, rs_cam_core::feeds::envelope::CutPoint>,
}

impl EnvelopeCandidates {
    /// The candidate of toolpath `id`, if one exists.
    #[must_use]
    pub fn get(
        &self,
        id: rs_cam_core::ToolpathId,
    ) -> Option<rs_cam_core::feeds::envelope::CutPoint> {
        self.points.get(&id).copied()
    }

    /// Move the candidate of toolpath `id` to `point`.
    pub fn set(
        &mut self,
        id: rs_cam_core::ToolpathId,
        point: rs_cam_core::feeds::envelope::CutPoint,
    ) {
        let _ = self.points.insert(id, point);
    }

    /// Drop the candidate of toolpath `id` (Esc, or an apply). True when
    /// one existed.
    pub fn clear(&mut self, id: rs_cam_core::ToolpathId) -> bool {
        self.points.remove(&id).is_some()
    }
}

/// The last heights edit that broke the plane order (F033), which the
/// Heights tab draws as the state of the row it edited for
/// [`HeightsRefusal::SHOW_S`] seconds. K6: the edit is written; this is its
/// warning.
///
/// The order gate (`ui::properties::admit_heights_edit`) writes it, and an
/// edit in order clears it. Before UI-09 it was held in egui memory under
/// one global id. The slot holds the toolpath it belongs to, so the tab of
/// another toolpath does not show it.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct HeightsRefusal {
    toolpath: Option<rs_cam_core::ToolpathId>,
    /// The text and the egui time (seconds) of the refusal.
    line: Option<(String, f64)>,
    /// W3d: the field key of the row the refused edit changed, or `None`
    /// when no one row changed. The row draws the refusal as its state.
    field: Option<&'static str>,
}

impl HeightsRefusal {
    /// How long the Heights tab shows a refused edit (seconds).
    pub const SHOW_S: f64 = 4.0;

    /// Give the slot to toolpath `id`. A refusal of another toolpath is
    /// dropped.
    pub fn scope(&mut self, id: rs_cam_core::ToolpathId) {
        if self.toolpath != Some(id) {
            self.toolpath = Some(id);
            self.line = None;
            self.field = None;
        }
    }

    /// Keep `text` as the refusal of an edit of the row `field` at egui
    /// time `now` (seconds).
    pub fn refuse(&mut self, field: Option<&'static str>, text: String, now: f64) {
        self.line = Some((text, now));
        self.field = field;
    }

    /// Drop the refusal: an edit was accepted.
    pub fn clear(&mut self) {
        self.line = None;
        self.field = None;
    }

    /// The field key of the refused edit at egui time `now`, while the
    /// refusal is recent.
    #[must_use]
    pub fn field(&self, now: f64) -> Option<&'static str> {
        self.text(now).and(self.field)
    }

    /// The refusal text at egui time `now`, while it is recent.
    #[must_use]
    pub fn text(&self, now: f64) -> Option<&str> {
        let (text, at) = self.line.as_ref()?;
        (now - at <= Self::SHOW_S).then_some(text.as_str())
    }
}

/// G-C (D-2, review §2 e): the `Rest Regions` pick wrote `Inside` to the
/// containment of a toolpath whose tool is larger than its source's. The
/// Containment combo shows the `ProvKind::AutoCorrect` badge `⟲` with this
/// hover until the first containment edit or a change of toolpath. GUI
/// state, not a project field, and not the per-frame draft (which the
/// session rebuilds each frame).
#[derive(Debug, Default, Clone, PartialEq)]
pub struct ContainmentAutoCorrect {
    toolpath: Option<rs_cam_core::ToolpathId>,
    hover: Option<String>,
}

impl ContainmentAutoCorrect {
    /// Give the slot to toolpath `id`. The badge of another toolpath is
    /// dropped.
    pub fn scope(&mut self, id: rs_cam_core::ToolpathId) {
        if self.toolpath != Some(id) {
            self.toolpath = Some(id);
            self.hover = None;
        }
    }

    /// The pick set `Inside`; `hover` says why.
    pub fn set(&mut self, hover: String) {
        self.hover = Some(hover);
    }

    /// The operator edited the containment: the badge goes.
    pub fn clear(&mut self) {
        self.hover = None;
    }

    /// The badge hover while the badge stands.
    #[must_use]
    pub fn hover(&self) -> Option<&str> {
        self.hover.as_deref()
    }
}

/// The values in flight of the committed number fields, by widget id.
pub type NumberDrafts = std::collections::HashMap<egui::Id, f64>;

/// The first value of the model panel's "Join all within" field, mm: the
/// value of the P4.9 panel sketch (`P4_DESIGN.md`, "P4.9").
pub const DEFAULT_JOIN_ALL_MM: f64 = 0.5;

/// The viewport popover of an open path of a drawing (geometry selection
/// P4.9): a click on an open path of the selected model opens it at the
/// click, with the two close acts of the model panel.
#[derive(Debug, Clone, PartialEq)]
pub struct DrawingPopover {
    pub model_id: super::job::ModelId,
    /// The key of the open path.
    pub key: rs_cam_core::selection::MatchKey,
    /// The screen point of the click.
    pub at: [f32; 2],
}

/// The import dialog of a drawing, SVG or DXF (geometry selection P4.8).
///
/// The dialog reads the file through the one import door
/// (`rs_cam_core::io::load_model_file`) at its units and options, on open
/// and on each committed change. Import adds the model of that read, so
/// the dialog shows exactly what Import adds.
#[derive(Debug, Clone)]
pub struct ImportDialog {
    pub path: std::path::PathBuf,
    /// `Svg` or `Dxf`.
    pub kind: rs_cam_core::compute::stock_config::ModelKind,
    /// The declared units (the scale). The default is core's
    /// `svg_default_units` for an SVG, mm for a DXF.
    pub units: rs_cam_core::compute::stock_config::ModelUnits,
    pub options: rs_cam_core::io::ImportOptions,
    /// The size the SVG file declares, or why it could not be read. `None`
    /// for a DXF.
    pub declared: Option<Result<rs_cam_core::io::svg_input::SvgDeclaredSize, String>>,
    /// The import at `units` and `options`, or the refusal of the door.
    pub preview: Result<rs_cam_core::session::LoadedModel, String>,
    /// S11: true when the stock has no thickness at the open: a drawing
    /// has none, so the dialog asks for it.
    pub asks_stock_z: bool,
    /// S11: the stock thickness (mm) the operator states in the dialog.
    /// `None`: the stock stays "not set", and every op refuses until it is
    /// set.
    pub stock_z: Option<f64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const DUMP: &str = "$11=0.020\n$120=500.000\n$121=500.000\n$122=250.000\n";

    /// The defect UI-09 names: the dump was parsed on every frame the
    /// disclosure was open.
    #[test]
    fn a_frame_that_changes_nothing_does_not_parse_again_ui09() {
        let mut draft = GrblImportDraft {
            buffer: DUMP.to_owned(),
            ..GrblImportDraft::default()
        };
        assert!(draft.parsed().is_some(), "the dump parses");
        assert_eq!(draft.parses, 1);

        for _ in 0..60 {
            let _ = draft.parsed();
        }
        assert_eq!(
            draft.parses, 1,
            "sixty frames of an unchanged buffer must parse once"
        );
    }

    #[test]
    fn a_changed_buffer_parses_again_ui09() {
        let mut draft = GrblImportDraft {
            buffer: DUMP.to_owned(),
            ..GrblImportDraft::default()
        };
        let first = draft.parsed().cloned();
        draft.buffer.push_str("$110=8000.000\n");
        let second = draft.parsed().cloned();
        assert_eq!(draft.parses, 2, "a changed buffer parses again");
        assert_ne!(first, second, "and the new text reaches the answer");
    }

    #[test]
    fn a_blank_buffer_parses_nothing_and_drops_the_cache_ui09() {
        let mut draft = GrblImportDraft {
            buffer: DUMP.to_owned(),
            ..GrblImportDraft::default()
        };
        assert!(draft.parsed().is_some());
        draft.clear();
        assert!(draft.parsed().is_none(), "a blank buffer has no parse");
        assert_eq!(draft.parses, 1, "and clearing does not parse");
    }
}
