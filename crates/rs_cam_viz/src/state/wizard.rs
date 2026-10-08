//! Resumable settings for the export wizard.
//!
//! **This is GUI state, not project data** (plan §19 ruling 5). The
//! record lived on `ProjectSession` until WP6b. Nothing saved it and the
//! project loader reset it on every load, so ten fields of
//! session-lifetime view memory sat inside the core session behind a
//! `wizard_mut` hatch. The record now lives on [`GuiState`], which is the
//! argument every export door already takes.
//!
//! [`GuiState`]: crate::state::runtime::GuiState

use std::path::PathBuf;

use rs_cam_core::gcode::{ToolChangeMode, Units, WcsCode};

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

/// One grblHAL controller option of the export dialog (G11). Each one is
/// a field of the project's post block (`PostConfig`), so the GUI, MCP and
/// the CLI read one value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControllerOption {
    /// `PostConfig::controller_waits_for_spindle`.
    WaitsForSpindle,
    /// `PostConfig::mist_output`.
    MistOutput,
    /// `PostConfig::native_drill_cycles`.
    NativeDrillCycles,
}

impl ControllerOption {
    /// Write `value` into the matching field of `post`. `None` returns the
    /// field to "from the machine profile".
    pub fn write(self, post: &mut rs_cam_core::gcode::PostConfig, value: Option<bool>) {
        match self {
            Self::WaitsForSpindle => post.controller_waits_for_spindle = value,
            Self::MistOutput => post.mist_output = value,
            Self::NativeDrillCycles => post.native_drill_cycles = value,
        }
    }
}

/// Resumable wizard settings, held on
/// [`GuiState`](crate::state::runtime::GuiState).
///
/// All `Option`-valued overrides default to `None`, meaning "use the post's
/// default for this field". Concrete values come from `PostDefinition`
/// when an override is absent.
#[derive(Debug, Clone)]
pub struct WizardState {
    /// The layout the operator chose. Read it through
    /// [`WizardState::effective_layout`], which applies the post default
    /// while the operator has not chosen.
    pub output_layout: OutputLayout,
    /// True once the operator picks a layout in the wizard. Until then a
    /// post with `prefer_one_file_per_setup` (grblHAL) exports a project
    /// with two or more setups as one file per setup (G3).
    pub layout_chosen: bool,
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
    /// Tool-change handling override for this export. `None` = use the
    /// post's `tool_change` template; `Pause`/`M6` swap the template;
    /// `Suppress` strips tool-change blocks (keeping per-tool RPM).
    pub tool_change_override: Option<ToolChangeMode>,
    pub allow_validator_errors: bool,
    pub last_save_dir: Option<PathBuf>,
    /// Highest 0-indexed step the user has visited. The wizard opens at
    /// this step on resume so they can pick up where they left off.
    pub last_step_visited: u8,
}

impl WizardState {
    /// The layout this export uses (G3). The operator's choice wins. With
    /// no choice yet, a post that prefers one file per setup (grblHAL)
    /// splits a project with two or more setups that hold an enabled
    /// toolpath. Otherwise the stored layout applies.
    pub fn effective_layout(
        &self,
        post: &rs_cam_core::gcode::PostDefinition,
        setups_with_ops: usize,
    ) -> OutputLayout {
        if !self.layout_chosen && post.prefer_one_file_per_setup && setups_with_ops > 1 {
            OutputLayout::PerSetup
        } else {
            self.output_layout
        }
    }
}

impl Default for WizardState {
    fn default() -> Self {
        Self {
            output_layout: OutputLayout::default(),
            layout_chosen: false,
            filename_template: "{job}.nc".to_owned(),
            wcs_override: None,
            units_override: None,
            safe_z_override: None,
            spindle_warmup_secs: 0,
            dry_run: false,
            tool_change_override: None,
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
        assert!(!s.layout_chosen);
        assert_eq!(s.filename_template, "{job}.nc");
        assert!(s.wcs_override.is_none());
        assert!(s.units_override.is_none());
        assert!(s.safe_z_override.is_none());
        assert_eq!(s.spindle_warmup_secs, 0);
        assert!(!s.dry_run);
        assert!(s.tool_change_override.is_none());
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
        assert_eq!(
            labels
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len(),
            3
        );
    }

    /// G3: grblHAL splits a two-setup project by default; the GRBL post
    /// does not; an operator choice wins on both.
    #[test]
    fn grblhal_defaults_to_one_file_per_setup_g3() {
        let hal = rs_cam_core::gcode::post::grblhal();
        let grbl = rs_cam_core::gcode::post::grbl();
        let mut s = WizardState::default();
        assert_eq!(s.effective_layout(hal, 2), OutputLayout::PerSetup);
        assert_eq!(s.effective_layout(hal, 1), OutputLayout::SingleFile);
        assert_eq!(s.effective_layout(grbl, 2), OutputLayout::SingleFile);
        s.layout_chosen = true;
        assert_eq!(s.effective_layout(hal, 2), OutputLayout::SingleFile);
    }
}
