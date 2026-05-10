//! Resumable settings for the export wizard.
//!
//! Stored on `ProjectSession` so the GUI, CLI, and MCP harness can all read
//! and update the same state. Currently in-memory only; persistence to the
//! project TOML can be added later if it proves useful.

use std::path::PathBuf;

use crate::gcode::{Units, WcsCode};

/// How emitted g-code is split across files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputLayout {
    /// One file containing every enabled toolpath.
    #[default]
    SingleFile,
    /// One file per setup, with `M0` pauses between setups in the
    /// combined preview.
    PerSetup,
    /// One file per toolpath.
    PerToolpath,
}

impl OutputLayout {
    pub fn label(self) -> &'static str {
        match self {
            Self::SingleFile => "Single file",
            Self::PerSetup => "One file per setup",
            Self::PerToolpath => "One file per toolpath",
        }
    }
}

/// Resumable wizard settings persisted on `ProjectSession`.
///
/// All `Option`-valued overrides default to `None`, meaning "use the post's
/// default for this field". Concrete values come from `PostDefinition`
/// when an override is absent.
#[derive(Debug, Clone)]
pub struct WizardState {
    pub output_layout: OutputLayout,
    pub filename_template: String,
    pub wcs_override: Option<WcsCode>,
    pub units_override: Option<Units>,
    pub safe_z_override: Option<f64>,
    pub spindle_warmup_secs: u32,
    /// Dry-run mode — when true, every cutting move's Z is clamped to
    /// the effective safe-Z so the spindle stays in air for the whole
    /// program. Operators use this to verify XY paths and feed rates
    /// without touching material. Resolved into the emit path via
    /// `WizardOverlay::dry_run_safe_z`.
    pub dry_run: bool,
    pub allow_validator_errors: bool,
    pub last_save_dir: Option<PathBuf>,
    /// Highest 0-indexed step the user has visited. The wizard opens at
    /// this step on resume so they can pick up where they left off.
    pub last_step_visited: u8,
}

impl Default for WizardState {
    fn default() -> Self {
        Self {
            output_layout: OutputLayout::default(),
            filename_template: "{job}.nc".to_owned(),
            wcs_override: None,
            units_override: None,
            safe_z_override: None,
            spindle_warmup_secs: 0,
            dry_run: false,
            allow_validator_errors: false,
            last_save_dir: None,
            last_step_visited: 0,
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_neutral() {
        let s = WizardState::default();
        assert_eq!(s.output_layout, OutputLayout::SingleFile);
        assert_eq!(s.filename_template, "{job}.nc");
        assert!(s.wcs_override.is_none());
        assert!(s.units_override.is_none());
        assert!(s.safe_z_override.is_none());
        assert_eq!(s.spindle_warmup_secs, 0);
        assert!(!s.dry_run);
        assert!(!s.allow_validator_errors);
        assert!(s.last_save_dir.is_none());
        assert_eq!(s.last_step_visited, 0);
    }

    #[test]
    fn output_layout_labels_distinct() {
        let labels = [
            OutputLayout::SingleFile.label(),
            OutputLayout::PerSetup.label(),
            OutputLayout::PerToolpath.label(),
        ];
        assert_eq!(labels.iter().collect::<std::collections::HashSet<_>>().len(), 3);
    }
}
