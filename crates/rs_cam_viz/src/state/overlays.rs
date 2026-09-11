//! State for the viewport Overlays panel (P6).
//!
//! The panel itself holds no copy of any overlay flag — it reads every flag
//! from [`crate::state::AppState`] each frame and caches nothing, because the
//! planner and the workspace switch write those flags behind its back
//! (audit §5, rule 2). What lives here is the panel's own shape plus the
//! per-workspace default bookkeeping.

use super::Workspace;

/// Which Overlays group a collapsing header belongs to. Mirrors
/// [`crate::ui::overlays::registry::OverlayGroup`] and exists separately only
/// so `state` does not depend on `ui`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GroupOpenState {
    pub geometry: bool,
    pub toolpath: bool,
    pub regions: bool,
    pub analysis: bool,
}

impl Default for GroupOpenState {
    fn default() -> Self {
        Self {
            geometry: true,
            toolpath: true,
            regions: true,
            analysis: true,
        }
    }
}

/// The Overlays panel's own state.
pub struct OverlayPanelState {
    /// `true` while the panel is on screen. The `Overlays (n)` button and the
    /// `O` shortcut both toggle it.
    pub open: bool,
    /// `true` while the panel is docked as a column inside the viewport
    /// instead of floating over it. Toggled by the pin icon and `Shift+O`.
    pub pinned: bool,
    /// Per-group expanded state, remembered across opens.
    pub groups: GroupOpenState,
    /// The workspace whose defaults are currently applied, and the values
    /// they displaced.
    ///
    /// This generalises the ad-hoc three-flag save/restore that
    /// `UiCommand::SwitchWorkspace` carried before P6 (audit §3.4 asked for
    /// exactly that: generalise the existing mechanism, do not add a second
    /// one beside it). Entering a workspace restores whatever the previous
    /// workspace displaced, then saves and overwrites only the flags that
    /// workspace names a default for.
    pub defaults_applied_for: Option<Workspace>,
    /// `(registry id, value before the default was applied)`.
    pub displaced: Vec<(&'static str, bool)>,
}

impl OverlayPanelState {
    pub fn new() -> Self {
        Self {
            open: false,
            pinned: false,
            groups: GroupOpenState::default(),
            defaults_applied_for: None,
            displaced: Vec::new(),
        }
    }
}

impl Default for OverlayPanelState {
    fn default() -> Self {
        Self::new()
    }
}
