//! The memory budget: the limit, the stop reason, the job guard, the
//! estimators and the one grid-cap source.
//!
//! `planning/memory_budget_2026-10-01/PLAN.md`, "Architecture" 1 and 2, and
//! findings B1, B2 and B3. The app had no memory limit of its own; the only
//! guard was an external cgroup that killed the whole process.
//!
//! - [`MemoryBudget`] holds the limit. `None` means no limit, and that is a
//!   valid state, not an error.
//! - [`StopReason`] says why a job stopped: the operator, or the budget.
//! - [`guard::BudgetGuard`] is the per-job cancel flag plus a usage probe. It
//!   implements [`crate::interrupt::CancelCheck`].
//! - [`estimate`] says what a simulation will hold before it runs.
//! - [`grid`] is the one source of every dexel grid cap.

pub mod estimate;
pub mod grid;
pub mod guard;

use std::fmt;

pub use estimate::{
    CellFit, SimulationEstimate, SimulationLoad, estimate_simulation_bytes, largest_cell_that_fits,
};
pub use grid::{GridCapRole, grid_cells};
pub use guard::{BudgetGuard, BudgetWatch, ProcessRss, UsageProbe};

/// The default limit as a fraction of the system memory, when the settings
/// file gives no limit.
///
/// RULING PENDING (operator): the plan lists the default fraction as
/// operator decision 2. Until the ruling, the value is `None`, and the
/// default budget has NO limit. Do not type a number here without the
/// ruling.
pub const DEFAULT_SYSTEM_FRACTION: Option<f64> = None;

/// The memory limit that a surface configured.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MemoryLimit {
    /// Nothing configured: use [`DEFAULT_SYSTEM_FRACTION`].
    #[default]
    Default,
    /// The operator asked for no limit.
    Unlimited,
    /// A limit in bytes.
    Bytes(u64),
}

/// The memory budget of the process.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MemoryBudget {
    /// The limit in bytes. `None` means no limit.
    pub limit_bytes: Option<u64>,
}

impl MemoryBudget {
    /// A budget with no limit.
    pub const UNLIMITED: Self = Self { limit_bytes: None };

    /// A budget with this limit in bytes.
    #[must_use]
    pub const fn with_limit(limit_bytes: u64) -> Self {
        Self {
            limit_bytes: Some(limit_bytes),
        }
    }

    /// `fraction` of `total_bytes`. A fraction that is not finite, not
    /// positive or larger than 1, or a zero total, gives no limit.
    #[must_use]
    pub fn from_fraction_of(total_bytes: u64, fraction: f64) -> Self {
        if total_bytes == 0 || !fraction.is_finite() || fraction <= 0.0 || fraction > 1.0 {
            return Self::UNLIMITED;
        }
        let limit = (total_bytes as f64 * fraction).floor();
        if limit < 1.0 {
            return Self::UNLIMITED;
        }
        Self::with_limit(limit as u64)
    }

    /// `fraction` of the system memory. When the system memory is unknown,
    /// the budget has no limit.
    #[must_use]
    pub fn from_system_fraction(fraction: f64) -> Self {
        system_memory_bytes().map_or(Self::UNLIMITED, |total| {
            Self::from_fraction_of(total, fraction)
        })
    }

    /// The budget for a configured limit. [`MemoryLimit::Default`] reads
    /// [`DEFAULT_SYSTEM_FRACTION`]; while that is `None`, it has no limit.
    #[must_use]
    pub fn from_setting(setting: MemoryLimit) -> Self {
        match setting {
            MemoryLimit::Bytes(bytes) => Self::with_limit(bytes),
            MemoryLimit::Unlimited => Self::UNLIMITED,
            MemoryLimit::Default => {
                DEFAULT_SYSTEM_FRACTION.map_or(Self::UNLIMITED, Self::from_system_fraction)
            }
        }
    }

    /// True when the budget has a limit.
    #[must_use]
    pub fn is_limited(&self) -> bool {
        self.limit_bytes.is_some()
    }

    /// True when `need_bytes` is at or under the limit, or there is no
    /// limit.
    #[must_use]
    pub fn fits(&self, need_bytes: u64) -> bool {
        self.limit_bytes.is_none_or(|limit| need_bytes <= limit)
    }

    /// `Ok` when `need_bytes` fits, else the [`StopReason::OverBudget`] that
    /// a refusal names.
    ///
    /// # Errors
    /// [`StopReason::OverBudget`] when `need_bytes` is over the limit.
    pub fn check(&self, need_bytes: u64) -> Result<(), StopReason> {
        match self.limit_bytes {
            Some(limit_bytes) if need_bytes > limit_bytes => Err(StopReason::OverBudget {
                need_bytes,
                limit_bytes,
            }),
            _ => Ok(()),
        }
    }
}

/// The total memory of the machine in bytes, or `None` when the platform
/// does not say.
///
/// `sysinfo` 0.39: `System::new()`, `refresh_memory()`, `total_memory()`,
/// which answers in bytes. The call reads the whole system table, so call it
/// once per process, not per job.
#[must_use]
pub fn system_memory_bytes() -> Option<u64> {
    let mut system = sysinfo::System::new();
    system.refresh_memory();
    let total = system.total_memory();
    (total > 0).then_some(total)
}

/// Why a job stopped before it finished.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
    /// The operator cancelled it.
    User,
    /// The job needs more memory than the budget allows.
    OverBudget {
        /// The bytes the job needs: an estimate before the job, or the
        /// resident size the probe read during it.
        need_bytes: u64,
        /// The limit, in bytes.
        limit_bytes: u64,
    },
}

impl fmt::Display for StopReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::User => f.write_str("cancelled by the operator"),
            Self::OverBudget {
                need_bytes,
                limit_bytes,
            } => write!(
                f,
                "stopped by the memory budget: needs {}, the limit is {}",
                format_bytes(*need_bytes),
                format_bytes(*limit_bytes)
            ),
        }
    }
}

/// The binary units that [`parse_byte_size`] accepts, largest first.
const BYTE_UNITS: [(&str, u64); 5] = [
    ("TiB", 1 << 40),
    ("GiB", 1 << 30),
    ("MiB", 1 << 20),
    ("KiB", 1 << 10),
    ("B", 1),
];

/// Why a byte size did not parse.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ByteSizeError {
    /// The text is empty.
    #[error("the memory size is empty")]
    Empty,
    /// The number part is not a non-negative number.
    #[error("'{0}' is not a memory size; write for example \"12GiB\" or a byte count")]
    BadNumber(String),
    /// The unit is not one of B, KiB, MiB, GiB, TiB.
    #[error("unknown unit '{0}'; use B, KiB, MiB, GiB or TiB (binary units)")]
    UnknownUnit(String),
    /// The size does not fit in 64 bits.
    #[error("the memory size '{0}' is too large")]
    TooLarge(String),
}

/// Parse a byte size: a number and a binary unit, for example `"12GiB"`,
/// `"1.5 GiB"`, `"512MiB"`, or a bare byte count `"4096"`.
///
/// The unit is case-insensitive. Decimal units (`GB`, `MB`) are refused, so
/// a limit never means two different values.
///
/// # Errors
/// A [`ByteSizeError`] that names the problem.
pub fn parse_byte_size(text: &str) -> Result<u64, ByteSizeError> {
    let text = text.trim();
    if text.is_empty() {
        return Err(ByteSizeError::Empty);
    }
    let split = text
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(text.len());
    let (number, unit) = text.split_at(split);
    let unit = unit.trim();
    let scale = if unit.is_empty() {
        1
    } else {
        BYTE_UNITS
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(unit))
            .map(|(_, scale)| *scale)
            .ok_or_else(|| ByteSizeError::UnknownUnit(unit.to_owned()))?
    };
    if number.is_empty() {
        return Err(ByteSizeError::BadNumber(text.to_owned()));
    }
    if let Ok(whole) = number.parse::<u64>() {
        return whole
            .checked_mul(scale)
            .ok_or_else(|| ByteSizeError::TooLarge(text.to_owned()));
    }
    let value: f64 = number
        .parse()
        .map_err(|_bad_float| ByteSizeError::BadNumber(text.to_owned()))?;
    let scaled = (value * scale as f64).floor();
    if !scaled.is_finite() || scaled < 0.0 {
        return Err(ByteSizeError::BadNumber(text.to_owned()));
    }
    if scaled >= u64::MAX as f64 {
        return Err(ByteSizeError::TooLarge(text.to_owned()));
    }
    Ok(scaled as u64)
}

/// A byte count in the largest binary unit that keeps it at 1 or more, with
/// two decimals, for example `"12.00 GiB"`.
#[must_use]
pub fn format_bytes(bytes: u64) -> String {
    for (name, scale) in BYTE_UNITS {
        if scale > 1 && bytes >= scale {
            return format!("{:.2} {name}", bytes as f64 / scale as f64);
        }
    }
    format!("{bytes} B")
}

#[cfg(test)]
mod tests;
