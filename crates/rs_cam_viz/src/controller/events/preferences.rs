//! File ▸ Preferences: open the window, and apply its draft.
//!
//! Operator request and rulings 2026-10-02: every app setting is in one
//! window; the memory budget defaults to half of the system RAM; the
//! cut-trace file is off by default.
//!
//! Apply does three things, in this order:
//!
//! 1. It writes the draft to the settings file through the ONE core writer
//!    (`rs_cam_core::settings::save_to`). The writer keeps every key that it
//!    does not own. The next start of the GUI and every CLI run read it.
//! 2. It applies the live values: the memory budget of the running compute
//!    backend ([`ComputeBackend::set_memory_budget`]; a job that runs keeps
//!    its old guard), the undo depth, the library folders, and the
//!    `AppState::app_settings` that the toasts, the quit guard, the overlay
//!    defaults and the next job read.
//! 3. It closes the window and posts an Info toast. The toast says when a
//!    value takes effect only after a restart.
//!
//! When the draft is not valid or the write fails, nothing changes, and
//! the window stays open with the error. So the file and the running app
//! never disagree after an Apply.

use crate::compute::ComputeBackend;
use crate::controller::Severity;
use crate::ui::preferences::{
    AutomationStatus, FolderFacts, PreferencesState, budget_text, needs_restart,
};

use super::super::AppController;

impl<B: ComputeBackend> AppController<B> {
    /// Take the settings file's values at start (`RsCamApp::new`).
    pub(crate) fn apply_startup_settings(&mut self, settings: rs_cam_core::settings::AppSettings) {
        self.state.apply_startup_settings(settings);
    }

    /// Open File ▸ Preferences. Reads the settings file, the backend budget,
    /// the system memory, the folder defaults and the present-mode record
    /// ONCE here, never per frame.
    pub(crate) fn open_preferences(&mut self) {
        self.state.close_modals_for_exclusivity();
        let loaded = crate::io::app_settings::load();
        let system_bytes = rs_cam_core::budget::system_memory_bytes();
        let mut prefs = PreferencesState::new(&loaded, self.compute.memory_budget(), system_bytes);
        let (present_requested, present_source, present_negotiated) =
            crate::present_mode::status_text();
        prefs.automation = AutomationStatus {
            mcp_active: self.mcp_is_active(),
            present_requested,
            present_source,
            present_negotiated,
        };
        prefs.folders = FolderFacts::of_process();
        self.state.preferences = Some(prefs);
    }

    /// The app runs with `--mcp`: the controller holds the MCP compute map.
    fn mcp_is_active(&self) -> bool {
        #[cfg(feature = "mcp")]
        let active = self.pending_mcp.is_some();
        #[cfg(not(feature = "mcp"))]
        let active = false;
        active
    }

    /// File ▸ Preferences ▸ Apply. See the module doc for the order.
    pub(crate) fn apply_preferences(&mut self) {
        let Some(prefs) = self.state.preferences.as_mut() else {
            return;
        };
        let settings = match prefs.chosen_settings() {
            Ok(settings) => settings,
            Err(problem) => {
                prefs.apply_error = Some(problem);
                return;
            }
        };
        let Some(path) = prefs.settings_path.clone() else {
            prefs.apply_error =
                Some("No settings path: set RS_CAM_SETTINGS, XDG_CONFIG_HOME or HOME.".to_owned());
            return;
        };
        if let Err(problem) = crate::io::app_settings::save_to(&path, &settings) {
            prefs.apply_error = Some(problem);
            return;
        }
        let budget = prefs.budget_for(settings.memory_limit);
        let restart = needs_restart(&self.state.app_settings, &settings);
        self.compute.set_memory_budget(budget);
        self.state.app_settings = settings;
        self.state.apply_live_settings();
        self.state.preferences = None;
        tracing::info!(
            limit = ?budget.limit_bytes,
            path = %path.display(),
            "preferences applied"
        );
        let mut message = format!(
            "Preferences saved to {}. Memory budget: {}; the next job uses it.",
            path.display(),
            budget_text(budget)
        );
        if restart {
            message.push_str(
                " Restart rs_cam to apply the window size, the log level or the present mode.",
            );
        }
        self.push_notification(message, Severity::Info);
    }
}

#[cfg(test)]
mod tests {
    // SAFETY: test module; a failed unwrap or expect is a failed test.
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use rs_cam_core::budget::{MemoryBudget, MemoryLimit};
    use rs_cam_core::settings::{AppSettings, LoadedSettings};

    use crate::compute::ThreadedComputeBackend;
    use crate::controller::AppController;
    use crate::ui::AppEvent;
    use crate::ui::preferences::{MemoryChoice, PreferencesCategory, PreferencesState};

    const GIB: u64 = 1 << 30;

    /// A scratch settings file in a folder of its own. The test never
    /// writes the operator's real settings file: the window state names
    /// the path, and the test gives it this one.
    fn scratch_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("rs_cam_prefs_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn open_on(controller: &mut AppController<ThreadedComputeBackend>, path: &std::path::Path) {
        let loaded = rs_cam_core::settings::load_from(path);
        let loaded = LoadedSettings {
            path: Some(path.to_path_buf()),
            ..loaded
        };
        controller.state.preferences = Some(PreferencesState::new(
            &loaded,
            controller.compute.memory_budget(),
            Some(64 * GIB),
        ));
    }

    fn prefs(controller: &mut AppController<ThreadedComputeBackend>) -> &mut PreferencesState {
        controller
            .state
            .preferences
            .as_mut()
            .expect("the window is open")
    }

    /// Apply writes the file AND changes the budget of the running backend
    /// for the next job, closes the window, and posts an Info toast.
    #[test]
    fn apply_writes_the_file_and_changes_the_backend_budget() {
        let dir = scratch_dir("apply");
        let path = dir.join("settings.toml");
        let mut controller = AppController::with_backend(ThreadedComputeBackend::with_budget(
            MemoryBudget::with_limit(32 * GIB),
        ));
        open_on(&mut controller, &path);
        prefs(&mut controller).memory_choice = MemoryChoice::Custom;
        prefs(&mut controller).custom_text = "12GiB".to_owned();

        controller.handle_internal_event(AppEvent::ApplyPreferences);

        assert_eq!(
            controller.compute.memory_budget(),
            MemoryBudget::with_limit(12 * GIB),
            "the next job uses the new budget"
        );
        assert!(controller.state.preferences.is_none(), "Apply closes");
        let loaded = rs_cam_core::settings::load_from(&path);
        assert_eq!(loaded.settings.memory_limit, MemoryLimit::Bytes(12 * GIB));
        assert!(
            controller
                .notifications()
                .iter()
                .any(|n| n.message.contains("Preferences saved")),
            "an Info toast"
        );

        // Default: half of the window's system total.
        open_on(&mut controller, &path);
        prefs(&mut controller).memory_choice = MemoryChoice::Default;
        controller.handle_internal_event(AppEvent::ApplyPreferences);
        assert_eq!(
            controller.compute.memory_budget(),
            MemoryBudget::with_limit(32 * GIB)
        );
        let loaded = rs_cam_core::settings::load_from(&path);
        assert_eq!(loaded.settings.memory_limit, MemoryLimit::Default);

        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Apply puts the draft into the running app: the live values change
    /// at once, and the file holds every key. A category switch before
    /// Apply loses nothing.
    #[test]
    fn apply_applies_the_live_values_after_a_category_switch() {
        let dir = scratch_dir("live");
        let path = dir.join("settings.toml");
        let mut controller = AppController::with_backend(ThreadedComputeBackend::new());
        open_on(&mut controller, &path);
        {
            let p = prefs(&mut controller);
            p.category = PreferencesCategory::General;
            p.texts.undo_depth = "7".to_owned();
            p.texts.toast_info = "2".to_owned();
            p.draft.general.confirm_unsaved_quit = false;
            p.category = PreferencesCategory::Diagnostics;
            p.draft.diagnostics.save_cut_trace = true;
            p.category = PreferencesCategory::Files;
            p.texts.screenshots = "/shots".to_owned();
            p.category = PreferencesCategory::Automation;
        }

        controller.handle_internal_event(AppEvent::ApplyPreferences);

        assert!(controller.state.preferences.is_none(), "Apply closes");
        let live = &controller.state.app_settings;
        assert_eq!(live.general.undo_depth, 7);
        assert_eq!(
            controller.state.history.limit(),
            7,
            "the undo depth is live"
        );
        assert!(!live.general.confirm_unsaved_quit);
        assert!(live.diagnostics.save_cut_trace);
        assert_eq!(
            live.paths.screenshots.as_deref(),
            Some(std::path::Path::new("/shots"))
        );
        let file = rs_cam_core::settings::load_from(&path).settings;
        assert_eq!(&file, live, "the file and the running app agree");

        // The next toast takes the new duration.
        controller.push_notification("next".to_owned(), crate::controller::Severity::Info);
        let last = controller.notifications().last().expect("a toast");
        assert_eq!(last.ttl().as_secs_f64(), 2.0);

        // Back to the defaults, so the process-wide library folders that
        // Apply installed do not reach another test.
        open_on(&mut controller, &path);
        prefs(&mut controller).draft = AppSettings::default();
        prefs(&mut controller).texts =
            PreferencesState::new(&LoadedSettings::default(), MemoryBudget::UNLIMITED, None).texts;
        prefs(&mut controller).memory_choice = MemoryChoice::Default;
        controller.handle_internal_event(AppEvent::ApplyPreferences);
        assert_eq!(controller.state.app_settings, AppSettings::default());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// A write that fails, or a draft that is not valid, changes nothing
    /// and keeps the window open with the error.
    #[test]
    fn a_failed_write_changes_nothing() {
        let dir = scratch_dir("bad_file");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.toml");
        std::fs::write(&path, "not toml at all [").unwrap();
        let before = MemoryBudget::with_limit(32 * GIB);
        let mut controller =
            AppController::with_backend(ThreadedComputeBackend::with_budget(before));
        open_on(&mut controller, &path);
        prefs(&mut controller).memory_choice = MemoryChoice::Unlimited;
        prefs(&mut controller).texts.undo_depth = "9".to_owned();

        controller.handle_internal_event(AppEvent::ApplyPreferences);

        assert_eq!(controller.compute.memory_budget(), before);
        assert_eq!(controller.state.history.limit(), 100);
        let p = controller.state.preferences.as_ref().expect("stays open");
        assert!(p.apply_error.is_some());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "not toml at all [");

        // A bad draft: nothing is written, the window stays open.
        std::fs::remove_file(&path).unwrap();
        open_on(&mut controller, &path);
        prefs(&mut controller).texts.undo_depth = "none".to_owned();
        controller.handle_internal_event(AppEvent::ApplyPreferences);
        let p = controller.state.preferences.as_ref().expect("stays open");
        assert!(
            p.apply_error
                .as_deref()
                .is_some_and(|e| e.contains("Undo steps"))
        );
        assert!(!path.exists(), "a bad draft writes no file");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Cancel (the window closes with no event) writes nothing and keeps
    /// the running values.
    #[test]
    fn cancel_discards_the_draft() {
        let dir = scratch_dir("cancel");
        let path = dir.join("settings.toml");
        let mut controller = AppController::with_backend(ThreadedComputeBackend::new());
        open_on(&mut controller, &path);
        prefs(&mut controller).texts.undo_depth = "3".to_owned();
        // Cancel, Escape and the cross all set the window state to `None`.
        controller.state.preferences = None;
        assert_eq!(controller.state.history.limit(), 100);
        assert_eq!(controller.state.app_settings, AppSettings::default());
        assert!(!path.exists());
        // A reopen starts from the file, not from the discarded draft.
        open_on(&mut controller, &path);
        assert_eq!(prefs(&mut controller).texts.undo_depth, "100");
    }
}
