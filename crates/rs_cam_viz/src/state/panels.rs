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
/// open on it, and move a selection of it to its setup (the S4 rule). The
/// controller door, the undo and the MCP remove all call it.
pub fn forget_obstacle(
    panels: &mut PanelDrafts,
    selection: &mut super::selection::Selection,
    setup_id: SetupId,
    obstacle_id: rs_cam_core::ids::ObstacleId,
) {
    if panels
        .obstacle_editor
        .as_ref()
        .is_some_and(|e| e.setup_id == setup_id && e.obstacle_id == obstacle_id)
    {
        panels.obstacle_editor = None;
    }
    if *selection == super::selection::Selection::Obstacle(setup_id, obstacle_id) {
        *selection = super::selection::Selection::Setup(setup_id);
    }
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
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
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
