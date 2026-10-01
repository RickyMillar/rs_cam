//! File ▸ Preferences: open the window, and apply its memory limit.
//!
//! Operator ruling 2026-10-02: the memory budget is configurable in
//! File ▸ Preferences and defaults to half of the system RAM.
//!
//! Apply does two things, in this order:
//!
//! 1. It writes `[memory] limit` to the settings file through the ONE core
//!    writer (`rs_cam_core::budget::settings::save_limit_to`). The next
//!    start of the GUI and every CLI run without `--memory-limit` read it.
//! 2. It gives the new budget to the running compute backend
//!    ([`ComputeBackend::set_memory_budget`]). A job that runs keeps its old
//!    guard; the next job uses the new budget.
//!
//! When the write fails, the backend does not change, and the window stays
//! open with the error. So the file and the running budget never disagree
//! after an Apply.

use rs_cam_core::budget::MemoryLimit;

use crate::compute::ComputeBackend;
use crate::controller::Severity;
use crate::ui::preferences::{PreferencesState, budget_text};

use super::super::AppController;

impl<B: ComputeBackend> AppController<B> {
    /// Open File ▸ Preferences. Reads the settings file, the backend budget
    /// and the system memory ONCE here, never per frame.
    pub(crate) fn open_preferences(&mut self) {
        self.state.close_modals_for_exclusivity();
        let loaded = crate::io::app_settings::load();
        let system_bytes = rs_cam_core::budget::system_memory_bytes();
        self.state.preferences = Some(PreferencesState::new(
            &loaded,
            self.compute.memory_budget(),
            system_bytes,
        ));
    }

    /// File ▸ Preferences ▸ Apply. See the module doc for the order.
    pub(crate) fn apply_preferences(&mut self, limit: MemoryLimit) {
        let Some(prefs) = self.state.preferences.as_mut() else {
            return;
        };
        let Some(path) = prefs.settings_path.clone() else {
            prefs.apply_error =
                Some("No settings path: set RS_CAM_SETTINGS, XDG_CONFIG_HOME or HOME.".to_owned());
            return;
        };
        if let Err(problem) = crate::io::app_settings::save_limit_to(&path, limit) {
            prefs.apply_error = Some(problem);
            return;
        }
        let budget = prefs.budget_for(limit);
        self.compute.set_memory_budget(budget);
        self.state.preferences = None;
        tracing::info!(
            limit = ?budget.limit_bytes,
            path = %path.display(),
            "memory budget changed in Preferences"
        );
        self.push_notification(
            format!(
                "Memory budget: {}. Saved to {}. The next job uses it.",
                budget_text(budget),
                path.display()
            ),
            Severity::Info,
        );
    }
}

#[cfg(test)]
mod tests {
    // SAFETY: test module; a failed unwrap or expect is a failed test.
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use rs_cam_core::budget::settings::{AppSettings, LoadedSettings};
    use rs_cam_core::budget::{MemoryBudget, MemoryLimit};

    use crate::compute::{ComputeBackend, ThreadedComputeBackend};
    use crate::controller::AppController;
    use crate::ui::AppEvent;
    use crate::ui::preferences::PreferencesState;

    const GIB: u64 = 1 << 30;

    /// A scratch settings file in a directory of its own. The test never
    /// writes the operator's real settings file: the window state names
    /// the path, and the test gives it this one.
    fn scratch_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("rs_cam_prefs_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn open_on(controller: &mut AppController<ThreadedComputeBackend>, path: &std::path::Path) {
        let loaded = LoadedSettings {
            settings: AppSettings::default(),
            path: Some(path.to_path_buf()),
            warning: None,
        };
        controller.state.preferences = Some(PreferencesState::new(
            &loaded,
            controller.compute.memory_budget(),
            Some(64 * GIB),
        ));
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

        controller.handle_internal_event(AppEvent::ApplyPreferences(MemoryLimit::Bytes(12 * GIB)));

        assert_eq!(
            controller.compute.memory_budget(),
            MemoryBudget::with_limit(12 * GIB),
            "the next job uses the new budget"
        );
        assert!(controller.state.preferences.is_none(), "Apply closes");
        let loaded = rs_cam_core::budget::settings::load_from(&path);
        assert_eq!(loaded.settings.memory_limit, MemoryLimit::Bytes(12 * GIB));

        // Default: half of the window's system total.
        open_on(&mut controller, &path);
        controller.handle_internal_event(AppEvent::ApplyPreferences(MemoryLimit::Default));
        assert_eq!(
            controller.compute.memory_budget(),
            MemoryBudget::with_limit(32 * GIB)
        );
        let loaded = rs_cam_core::budget::settings::load_from(&path);
        assert_eq!(loaded.settings.memory_limit, MemoryLimit::Default);

        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// A write that fails leaves the backend budget as it was and keeps the
    /// window open with the error.
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

        controller.handle_internal_event(AppEvent::ApplyPreferences(MemoryLimit::Unlimited));

        assert_eq!(controller.compute.memory_budget(), before);
        let prefs = controller.state.preferences.as_ref().expect("stays open");
        assert!(prefs.apply_error.is_some());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "not toml at all [");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
