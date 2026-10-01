//! The memory budget of one CLI run (plan B3 and "Architecture" 6,
//! `planning/memory_budget_2026-10-01/PLAN.md`).
//!
//! `--memory-limit <SIZE>` sets the budget for the whole process. With no
//! flag the budget is `MemoryLimit::Default`, which reads
//! `rs_cam_core::budget::DEFAULT_SYSTEM_FRACTION`. That value is RULING
//! PENDING (`None`), so with no flag the run has no limit, as before.
//!
//! The CLI does not read `~/.config/rs_cam/settings.toml` yet: the settings
//! loader lives in `rs_cam_viz::io::app_settings`, and this crate depends on
//! core only. When the loader moves to core, the flag overrides the file.

use std::sync::OnceLock;

use rs_cam_core::budget::{
    BudgetGuard, MemoryBudget, MemoryLimit, StopReason, format_bytes, parse_byte_size,
};

/// The text of `--memory-limit` that asks for no limit. The same word as
/// `[memory] limit = "unlimited"` in the settings file.
const UNLIMITED: &str = "unlimited";

/// The budget of this process, set once by `main`.
static BUDGET: OnceLock<MemoryBudget> = OnceLock::new();

/// Parse the value of `--memory-limit`: a binary size (`"12GiB"`), a byte
/// count (`"4096"`), or `"unlimited"`.
///
/// # Errors
/// A sentence that names the problem, for clap to show.
pub(crate) fn parse_memory_limit(text: &str) -> Result<MemoryLimit, String> {
    if text.trim().eq_ignore_ascii_case(UNLIMITED) {
        return Ok(MemoryLimit::Unlimited);
    }
    parse_byte_size(text)
        .map(MemoryLimit::Bytes)
        .map_err(|error| error.to_string())
}

/// Set the budget of this process. A second call has no effect.
pub(crate) fn set(limit: Option<MemoryLimit>) {
    let budget = MemoryBudget::from_setting(limit.unwrap_or_default());
    if BUDGET.set(budget).is_err() {
        tracing::debug!("memory budget already set; the first value stays");
    }
}

/// The budget of this process. No limit when `main` set none.
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

/// The sentence for an over-budget stop: the two values and the two
/// remedies.
pub(crate) fn over_budget_message(need_bytes: u64, limit_bytes: u64) -> String {
    format!(
        "stopped: the job needs about {}, but the memory budget is {}. \
         Pass a larger --memory-limit or use a coarser --resolution.",
        format_bytes(need_bytes),
        format_bytes(limit_bytes),
    )
}
