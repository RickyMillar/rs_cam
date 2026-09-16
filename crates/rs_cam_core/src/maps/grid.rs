//! The grid a full-board walk lays over the mesh bbox, and the row walk that
//! drives it.
//!
//! [`crate::maps::tier_map`] and [`crate::maps::reach_map`] had a copy each of both. The
//! grid is the same row-major convention in both maps, and the walk is the
//! same dual-configuration row loop with one cancel poll per row — the
//! granularity each module doc promises, which cannot drift between the two
//! passes or the two build configurations while there is one site.
//!
//! The constructors stay local, because they are different rules: `tier_map`
//! pads by the ladder's finest envelope, and `reach_map` coarsens the cell
//! until the grid fits its cell budget.

use std::sync::atomic::{AtomicU64, Ordering};

#[cfg(feature = "parallel")]
use rayon::prelude::*;

#[cfg(not(feature = "parallel"))]
use crate::interrupt::check_cancel;
use crate::interrupt::{CancelCheck, Cancelled};

/// The grid a walk lays over the mesh bbox, row-major from `origin`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct GridSpec {
    pub(crate) nx: usize,
    pub(crate) ny: usize,
    pub(crate) origin_x: f64,
    pub(crate) origin_y: f64,
    pub(crate) cell_mm: f64,
}

impl GridSpec {
    pub(crate) fn cell_count(&self) -> usize {
        self.nx * self.ny
    }

    pub(crate) fn x_of(&self, col: usize) -> f64 {
        self.origin_x + col as f64 * self.cell_mm
    }

    pub(crate) fn y_of(&self, row: usize) -> f64 {
        self.origin_y + row as f64 * self.cell_mm
    }
}

/// Run `row_fn` over every grid row and concatenate the results, polling
/// `cancel` **once per row** and folding each row's drop count into `drops`.
///
/// `drops` is the tier map's `DROP_CALLS` instrument: task T3 reads it to
/// prove that a cache hit does no drop-cutter work, so the fold stays on the
/// same site as the row it counts. A reach-map row has no drop count of its
/// own to report and passes `None`.
///
/// # Errors
///
/// [`Cancelled`] when `cancel` fires. A caller with a richer error type
/// converts through its own `From<Cancelled>`.
pub(crate) fn walk_rows<T: Send>(
    grid: &GridSpec,
    cancel: &(dyn CancelCheck + Sync),
    drops: Option<&AtomicU64>,
    row_fn: impl Fn(usize) -> (Vec<T>, u64) + Sync,
) -> Result<Vec<T>, Cancelled> {
    #[cfg(feature = "parallel")]
    {
        use std::sync::atomic::AtomicBool;
        let cancelled = AtomicBool::new(false);
        let collected: Vec<T> = (0..grid.ny)
            .into_par_iter()
            .flat_map(|row| {
                // One poll per row — the granularity `rest_field`'s walk
                // lacks entirely.
                if cancelled.load(Ordering::Relaxed) || cancel.cancelled() {
                    cancelled.store(true, Ordering::Relaxed);
                    return Vec::new();
                }
                let (cells, row_drops) = row_fn(row);
                if let Some(sink) = drops {
                    sink.fetch_add(row_drops, Ordering::Relaxed);
                }
                cells
            })
            .collect();
        if cancelled.load(Ordering::Relaxed) {
            return Err(Cancelled);
        }
        Ok(collected)
    }
    #[cfg(not(feature = "parallel"))]
    {
        let mut collected: Vec<T> = Vec::with_capacity(grid.cell_count());
        for row in 0..grid.ny {
            check_cancel(cancel)?;
            let (cells, row_drops) = row_fn(row);
            if let Some(sink) = drops {
                sink.fetch_add(row_drops, Ordering::Relaxed);
            }
            collected.extend(cells);
        }
        Ok(collected)
    }
}
