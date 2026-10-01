//! The app settings that the running GUI uses: `AppState::app_settings`.
//!
//! The settings file (`rs_cam_core::settings`) is the record; this module
//! puts its values into the GUI state. Two doors:
//!
//! - [`AppState::apply_startup_settings`], once at start: every section.
//! - [`AppState::apply_live_settings`], at start and after
//!   File ▸ Preferences ▸ Apply: the values that apply at once.
//!
//! What applies when:
//!
//! | Value | When |
//! |---|---|
//! | undo depth, confirm on quit, library folders | at once |
//! | toast durations | the next toast |
//! | overlay defaults | the next workspace switch |
//! | simulation defaults | the next start or project open |
//! | "All toolpaths", colour mode | the next start |
//! | cut-trace file, artifact folder | the next simulation or generation |
//! | window size, log level, present mode | the next start |
//!
//! Nothing here changes a computed number.

use rs_cam_core::settings::{
    AppSettings, SimulationSettings, StockViewDefault, ToolpathColourDefault,
};

use super::AppState;
use super::simulation::{SimulationState, StockVizMode};
use super::viewport::ToolpathColorMode;

/// The GUI colour mode of a settings value.
#[must_use]
pub fn toolpath_colour_mode(value: ToolpathColourDefault) -> ToolpathColorMode {
    match value {
        ToolpathColourDefault::Normal => ToolpathColorMode::Normal,
        ToolpathColourDefault::Engagement => ToolpathColorMode::Engagement,
        ToolpathColourDefault::AdvancePerTooth => ToolpathColorMode::AdvancePerTooth,
    }
}

/// The GUI stock view of a settings value.
#[must_use]
pub fn stock_viz_mode(value: StockViewDefault) -> StockVizMode {
    match value {
        StockViewDefault::Solid => StockVizMode::Solid,
        StockViewDefault::Deviation => StockVizMode::Deviation,
        StockViewDefault::ByHeight => StockVizMode::ByHeight,
    }
}

/// Put the `[simulation]` defaults into `simulation`: the playback speed,
/// the stock view and the stock opacity. The start and every project open
/// (`controller/io.rs`, which makes a new `SimulationState`) call it.
pub fn apply_simulation_defaults(simulation: &mut SimulationState, values: &SimulationSettings) {
    simulation.playback.speed = values.playback_speed;
    simulation.stock_viz_mode = stock_viz_mode(values.stock_view);
    simulation.stock_opacity = values.stock_opacity;
}

impl AppState {
    /// Take `settings` at start: every section.
    ///
    /// With the default settings the state is what `AppState::new` built,
    /// so a GUI with no settings file starts as it did before the file
    /// existed.
    pub fn apply_startup_settings(&mut self, settings: AppSettings) {
        self.app_settings = settings;
        self.apply_live_settings();
        let display = &self.app_settings.display;
        self.viewport.show_all_toolpaths = display.show_all_toolpaths;
        self.viewport.toolpath_color_mode = toolpath_colour_mode(display.toolpath_colour_mode);
        let has_overlay_values = !display.overlays.is_empty();
        apply_simulation_defaults(&mut self.simulation, &self.app_settings.simulation);
        if has_overlay_values {
            // The registry reads the file's overlay values through
            // `effective_default`, so the start workspace shows them.
            let workspace = self.workspace;
            crate::ui::overlays::registry::apply_workspace_defaults(self, workspace);
        }
    }

    /// Apply the values of `self.app_settings` that take effect at once.
    pub fn apply_live_settings(&mut self) {
        self.history.set_limit(self.app_settings.general.undo_depth);
        rs_cam_core::settings::install_paths(&self.app_settings.paths);
    }

    /// Ask before a quit: the project has unsaved changes and
    /// `[general] confirm_unsaved_quit` is on (the default).
    #[must_use]
    pub fn quit_needs_confirmation(&self) -> bool {
        self.gui.dirty && self.app_settings.general.confirm_unsaved_quit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// No settings file: the state is what `AppState::new` built.
    #[test]
    fn the_default_settings_change_nothing_at_start() {
        let fresh = AppState::new();
        let mut started = AppState::new();
        started.apply_startup_settings(AppSettings::default());
        assert_eq!(
            started.viewport.show_all_toolpaths,
            fresh.viewport.show_all_toolpaths
        );
        assert_eq!(
            started.viewport.toolpath_color_mode,
            fresh.viewport.toolpath_color_mode
        );
        assert_eq!(
            started.simulation.playback.speed,
            fresh.simulation.playback.speed
        );
        assert_eq!(
            started.simulation.stock_viz_mode,
            fresh.simulation.stock_viz_mode
        );
        assert_eq!(
            started.simulation.stock_opacity,
            fresh.simulation.stock_opacity
        );
        assert_eq!(started.history.limit(), 100);
        assert_eq!(
            started.overlays.defaults_applied_for,
            fresh.overlays.defaults_applied_for
        );
    }

    #[test]
    fn the_startup_settings_reach_the_state() {
        let mut settings = AppSettings::default();
        settings.general.undo_depth = 7;
        settings.display.show_all_toolpaths = true;
        settings.display.toolpath_colour_mode = ToolpathColourDefault::Engagement;
        settings.simulation.playback_speed = 1234.0;
        settings.simulation.stock_view = StockViewDefault::Deviation;
        settings.simulation.stock_opacity = 0.25;
        let mut state = AppState::new();
        state.apply_startup_settings(settings);
        assert_eq!(state.history.limit(), 7);
        assert!(state.viewport.show_all_toolpaths);
        assert_eq!(
            state.viewport.toolpath_color_mode,
            ToolpathColorMode::Engagement
        );
        assert_eq!(state.simulation.playback.speed, 1234.0);
        assert_eq!(state.simulation.stock_viz_mode, StockVizMode::Deviation);
        assert_eq!(state.simulation.stock_opacity, 0.25);

        // A project open makes a new simulation state; the defaults return.
        state.simulation = SimulationState::new();
        apply_simulation_defaults(&mut state.simulation, &state.app_settings.simulation);
        assert_eq!(state.simulation.playback.speed, 1234.0);
    }

    #[test]
    fn the_quit_guard_reads_the_setting() {
        let mut state = AppState::new();
        assert!(!state.quit_needs_confirmation(), "a clean project quits");
        state.gui.dirty = true;
        assert!(state.quit_needs_confirmation(), "the default asks");
        state.app_settings.general.confirm_unsaved_quit = false;
        assert!(!state.quit_needs_confirmation());
    }
}
