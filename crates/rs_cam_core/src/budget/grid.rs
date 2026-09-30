//! The one grid-cap source (B2 of `planning/memory_budget_2026-10-01/PLAN.md`).
//!
//! Before this module, four constants capped a dexel grid and they did not
//! agree: 16 M in `stock/dexel.rs` (the cap that clamps every grid), 8 M in
//! `session/rest_stock.rs` and again in `session/compute.rs` (the `Auto`
//! resolution rule), and 4 M in `dressup/mod.rs` (the entry replay prism).
//!
//! The three values protect three different things, so they stay three
//! caps. They now come from ONE ceiling, [`GRID_CELL_CEILING`], through
//! [`GridCapRole::cell_cap`], and every cap uses ONE formula,
//! [`cell_for_cap`]. A change to the ceiling moves all three together.
//!
//! - [`GridCapRole::Ceiling`] clamps every grid that
//!   [`crate::stock::dexel::DexelGrid::from_bounds`] builds. It is the last
//!   net against a cell size that makes a grid too large to allocate.
//! - [`GridCapRole::AutoResolution`] is the target of the `Auto` resolution
//!   rule. A full simulation keeps many grids and meshes of that size (see
//!   [`super::estimate`]), so the rule picks a grid smaller than the
//!   ceiling.
//! - [`GridCapRole::EntryReplay`] caps the transient prism that
//!   `dressup::OwnStockReplay` builds beside the session's own grids. Its
//!   reads are conservative, so a coarse cell costs air time, never a
//!   collision.
//!
//! The divisors reproduce the legacy values exactly (16 M / 2 = 8 M and
//! 16 M / 4 = 4 M), so every current caller gets bit-identical cell sizes.
//! A configured [`MemoryBudget`] does NOT change these caps. The budget acts
//! through the preflight refusal, because the plan rules that the
//! simulation grid is never coarsened silently.

use super::MemoryBudget;

/// The smallest cell size, in mm, a dexel grid accepts. A smaller or
/// non-positive request is raised to it, which avoids a division by zero.
pub const MIN_CELL_MM: f64 = 1e-6;

/// The most columns one dexel grid may hold. This is the value that
/// `stock/dexel.rs` held as `DexelGrid::MAX_GRID_CELLS` before B2.
pub const GRID_CELL_CEILING: usize = 16_000_000;

/// The `Auto` resolution cap is this fraction of the ceiling: 1 / 2 gives
/// the legacy 8 M of `session/rest_stock.rs:63`.
const AUTO_RESOLUTION_DIVISOR: usize = 2;

/// The entry replay cap is this fraction of the ceiling: 1 / 4 gives the
/// legacy 4 M of `dressup/mod.rs` (`OwnStockReplay::MAX_CELLS`).
const ENTRY_REPLAY_DIVISOR: usize = 4;

/// What a grid cap protects. See the module documentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridCapRole {
    /// The clamp that every dexel grid obeys.
    Ceiling,
    /// The target of the project `Auto` resolution rule.
    AutoResolution,
    /// The transient prism of the entry-descent replay.
    EntryReplay,
}

impl GridCapRole {
    /// The most columns a grid in this role may hold.
    #[must_use]
    pub const fn cell_cap(self) -> usize {
        match self {
            Self::Ceiling => GRID_CELL_CEILING,
            Self::AutoResolution => GRID_CELL_CEILING / AUTO_RESOLUTION_DIVISOR,
            Self::EntryReplay => GRID_CELL_CEILING / ENTRY_REPLAY_DIVISOR,
        }
    }

    /// The cap of this role, made no larger than the grid a budget allows.
    ///
    /// `bytes_per_cell` is the cost of one column of the job, for example
    /// [`super::estimate::SimulationLoad::bytes_per_cell`]. An unlimited
    /// budget, or a zero cost, returns [`Self::cell_cap`] unchanged. The
    /// preflight uses this value; the grid constructors do not.
    #[must_use]
    pub fn cell_cap_under(self, budget: &MemoryBudget, bytes_per_cell: u64) -> usize {
        let cap = self.cell_cap();
        match budget.limit_bytes {
            Some(limit) if bytes_per_cell > 0 => {
                let fit = limit / bytes_per_cell;
                usize::try_from(fit).map_or(cap, |fit| fit.min(cap))
            }
            _ => cap,
        }
    }
}

/// The cell size, in mm, at which a `extent_u` x `extent_v` area holds
/// `cap_cells` square cells: `sqrt(area / cap_cells)`.
///
/// This is the one formula the three caps use. It ignores the `+ 1` edge
/// column of [`grid_cells`], so a grid at this cell can hold slightly more
/// than `cap_cells` columns. That is the legacy behaviour of all three
/// callers, and it stays.
#[must_use]
pub fn cell_for_cap(extent_u: f64, extent_v: f64, cap_cells: usize) -> f64 {
    ((extent_u * extent_v) / cap_cells as f64).sqrt()
}

/// The number of columns a dexel grid holds over a `footprint_w` x
/// `footprint_d` footprint at `cell_mm`, before any cap.
///
/// Same rounding as `DexelGrid::from_bounds` (`stock/dexel.rs`):
/// `cols = ceil(w / cell) + 1` and `rows = ceil(d / cell) + 1`, with the
/// cell raised to [`MIN_CELL_MM`] first. The product saturates instead of
/// wrapping.
#[must_use]
pub fn grid_cells(footprint_w: f64, footprint_d: f64, cell_mm: f64) -> usize {
    let cell = floor_cell(cell_mm);
    let cols = ((footprint_w / cell).ceil() as usize).saturating_add(1);
    let rows = ((footprint_d / cell).ceil() as usize).saturating_add(1);
    rows.saturating_mul(cols)
}

/// The cell raised to [`MIN_CELL_MM`].
///
/// A NaN request stays NaN, as in the legacy `DexelGrid::clamp_cell_size`;
/// [`grid_cells`] then counts one column.
#[must_use]
pub fn floor_cell(cell_mm: f64) -> f64 {
    if cell_mm < MIN_CELL_MM {
        MIN_CELL_MM
    } else {
        cell_mm
    }
}

/// The coarser cell the ceiling forces, or `None` when the request fits.
///
/// This is the arithmetic of `DexelGrid::would_exceed_grid` and
/// `DexelGrid::clamp_cell_size`. Both now call it.
#[must_use]
pub fn ceiling_clamp(cell_mm: f64, extent_u: f64, extent_v: f64) -> Option<f64> {
    let cap = GridCapRole::Ceiling.cell_cap();
    if grid_cells(extent_u, extent_v, cell_mm) > cap {
        Some(cell_for_cap(extent_u, extent_v, cap))
    } else {
        None
    }
}

/// The cell a grid over this extent ACTUALLY uses: the request after the
/// minimum floor and the ceiling clamp.
#[must_use]
pub fn effective_cell(cell_mm: f64, extent_u: f64, extent_v: f64) -> f64 {
    // `f64::max`, not `floor_cell`: the legacy `effective_cell_size` maps a
    // NaN request to the minimum, and this keeps that answer.
    ceiling_clamp(cell_mm, extent_u, extent_v).unwrap_or_else(|| cell_mm.max(MIN_CELL_MM))
}

/// The columns a grid over this extent ACTUALLY allocates, after the floor
/// and the ceiling clamp. Use this, not [`grid_cells`], to estimate memory.
#[must_use]
pub fn effective_grid_cells(footprint_w: f64, footprint_d: f64, cell_mm: f64) -> usize {
    grid_cells(
        footprint_w,
        footprint_d,
        effective_cell(cell_mm, footprint_w, footprint_d),
    )
}
