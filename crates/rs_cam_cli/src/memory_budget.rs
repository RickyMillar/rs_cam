//! The memory budget of one CLI run (plan B3, B5 and "Architecture" 6,
//! `planning/memory_budget_2026-10-01/PLAN.md`).
//!
//! The precedence, highest first:
//!
//! 1. `--memory-limit <SIZE>` on the command line.
//! 2. `[memory] limit` in the settings file. The CLI reads the file through
//!    `rs_cam_core::settings`, the loader the GUI uses, so the path
//!    resolves the same way: `RS_CAM_SETTINGS`, then `XDG_CONFIG_HOME`,
//!    then `HOME`, then `APPDATA`, then `USERPROFILE`. The text of the
//!    limit reads through `rs_cam_core::budget::settings`.
//! 3. `MemoryLimit::Default`, which reads
//!    `rs_cam_core::budget::DEFAULT_SYSTEM_FRACTION`: half of the system
//!    RAM (operator ruling 2026-10-02). So with no flag and no file the run
//!    has a limit of half of the system RAM. `--memory-limit unlimited`
//!    removes it.
//!
//! When the flag is given, the CLI does not read the file.

use std::sync::OnceLock;

use rs_cam_core::budget::settings as limit_text;
use rs_cam_core::budget::{BudgetGuard, MemoryBudget, MemoryLimit, StopReason, format_bytes};
use rs_cam_core::settings::{self, LoadedSettings};

/// The budget of this process, set once by `main`.
static BUDGET: OnceLock<MemoryBudget> = OnceLock::new();

/// Parse the value of `--memory-limit`: a binary size (`"12GiB"`), a byte
/// count (`"4096"`), `"unlimited"` or `"default"`. The settings file reads
/// its text value through the same core function.
///
/// # Errors
/// A sentence that names the problem, for clap to show.
pub(crate) fn parse_memory_limit(text: &str) -> Result<MemoryLimit, String> {
    limit_text::parse_limit_text(text).map_err(|error| error.to_string())
}

/// The limit of this run: the flag, else the file that `load` reads, and
/// the file's warning when the file was read and not used.
///
/// `load` runs only when there is no flag, so a bad file never warns about
/// a value the flag replaced.
pub(crate) fn limit_from(
    flag: Option<MemoryLimit>,
    load: impl FnOnce() -> LoadedSettings,
) -> (MemoryLimit, Option<String>) {
    let loaded = flag.is_none().then(load);
    let file = loaded
        .as_ref()
        .map_or(MemoryLimit::Default, |loaded| loaded.settings.memory_limit);
    (
        limit_text::resolve_limit(flag, file),
        loaded.and_then(|loaded| loaded.warning),
    )
}

/// Set the budget of this process from the flag and the settings file. A
/// second call has no effect.
///
/// Returns the settings-file warning, for `main` to log after the log
/// subscriber starts.
pub(crate) fn set(flag: Option<MemoryLimit>) -> Option<String> {
    let (limit, warning) = limit_from(flag, settings::load);
    let budget = MemoryBudget::from_setting(limit);
    if BUDGET.set(budget).is_err() {
        tracing::debug!("memory budget already set; the first value stays");
    }
    warning
}

/// The budget of this process. When `main` set none, the default: half of
/// the system RAM.
pub(crate) fn current() -> MemoryBudget {
    BUDGET
        .get()
        .copied()
        .unwrap_or_else(|| MemoryBudget::from_setting(MemoryLimit::Default))
}

/// The error of a step that the memory budget stopped, or `None`.
///
/// Read it after a step fails, or after it returns: the step itself sees
/// only the cancel flag and reports a plain cancel.
pub(crate) fn over_budget(guard: &BudgetGuard) -> Option<anyhow::Error> {
    match guard.stop_reason() {
        Some(StopReason::OverBudget {
            need_bytes,
            limit_bytes,
        }) => Some(anyhow::anyhow!(over_budget_message(
            need_bytes,
            limit_bytes
        ))),
        Some(StopReason::User) | None => None,
    }
}

/// The sentence for an over-budget stop: the two values and the three
/// remedies.
pub(crate) fn over_budget_message(need_bytes: u64, limit_bytes: u64) -> String {
    format!(
        "stopped: the job needs about {}, but the memory budget is {}. \
         Pass a larger --memory-limit, raise [memory] limit in settings.toml, \
         or use a coarser --resolution.",
        format_bytes(need_bytes),
        format_bytes(limit_bytes),
    )
}
