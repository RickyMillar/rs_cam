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
