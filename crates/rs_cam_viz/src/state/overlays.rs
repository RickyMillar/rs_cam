//! State for the viewport dock and the All viewport options catalogue.
//!
//! The dock and the catalogue hold no copy of any overlay flag. They read
//! every flag from [`crate::state::AppState`] each frame and cache nothing,
//! because the planner and the workspace switch write those flags behind
//! their backs (audit §5, rule 2). What lives here is the shape of the two
//! surfaces plus the per-workspace default bookkeeping.

use super::Workspace;

/// One section of the viewport dock (viewport redesign, MOCKUPS §1).
///
/// The dock owns four sections in a fixed order. Each section opens one
/// upward popover. This type lives in `state` so that `state` does not
/// depend on `ui`; the section of each registry row is
/// `crate::ui::overlays::registry::dock_section`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DockSection {
    /// Camera presets, projection, reset and the orientation gizmo.
    View,
    /// Geometry, stock, fixtures, grid, boundaries and layers.
    Scene,
    /// Draw scope, cutting moves, rapids, spans and move colour.
    Paths,
    /// Model and stock analysis, collisions, deflection and traces.
    Inspect,
}

impl DockSection {
    /// Every section, in dock order. The Tab order follows this order.
    pub const ALL: [Self; 4] = [Self::View, Self::Scene, Self::Paths, Self::Inspect];

    /// The section button text, without a suffix.
    pub fn label(self) -> &'static str {
        match self {
            Self::View => "View",
            Self::Scene => "Scene",
            Self::Paths => "Paths",
            Self::Inspect => "Inspect",
        }
    }
}

/// Which rows the catalogue lists (MOCKUPS §10).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CatalogueFilter {
    /// Every row, in every state.
    #[default]
    All,
    /// Rows whose value differs from the workspace default.
    Changed,
    /// Rows whose precondition is not `Ready`.
    CannotDraw,
}

/// The state of the dock and the catalogue.
pub struct OverlayPanelState {
    /// `true` while the All viewport options catalogue is on screen. The
    /// dock's catalogue button and the `O` shortcut both toggle it.
    pub open: bool,
    /// The dock section whose popover is open. An `Option`, so the dock
    /// can never hold two open popovers at once.
    pub open_section: Option<DockSection>,
    /// The catalogue search text. It is a draft with content, so it lives
    /// here and not in egui temporary memory (`state/CLAUDE.md`).
    pub catalogue_query: String,
    /// The catalogue filter choice.
    pub catalogue_filter: CatalogueFilter,
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
            open_section: None,
            catalogue_query: String::new(),
            catalogue_filter: CatalogueFilter::All,
            defaults_applied_for: None,
            displaced: Vec::new(),
        }
    }

    /// A click on a section button. It opens that section's popover, or
    /// closes it when it is already open. A click on a second section
    /// closes the first and opens the second.
    pub fn toggle_section(&mut self, section: DockSection) {
        self.open_section = if self.open_section == Some(section) {
            None
        } else {
            Some(section)
        };
    }

    /// Close the open popover, if one is open.
    pub fn close_section(&mut self) {
        self.open_section = None;
    }

    /// Escape closes the open popover first, then the catalogue. Returns
    /// `true` when it closed something, so the caller consumes the key.
    pub fn close_one_for_escape(&mut self) -> bool {
        if self.open_section.is_some() {
            self.open_section = None;
            true
        } else if self.open {
            self.open = false;
            true
        } else {
            false
        }
    }
}

impl Default for OverlayPanelState {
    fn default() -> Self {
        Self::new()
    }
}
