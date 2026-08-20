//! Coarse max-of-`conservative_top` mip over a [`DexelGrid`] — the tile
//! early-out of `PERF_REVIEW.md` **S2**.
//!
//! # What it answers
//!
//! One `f32` per 16×16 cells: an **upper bound** on
//! [`DexelGrid::conservative_top`] anywhere in that tile. ~4 KB per million
//! cells, so it sits in L2 next to a grid that does not.
//!
//! A stamp asks one question of it: *"can this stamp remove anything at all
//! inside its own bounding box?"* If the bound over the covered tiles is at or
//! below the stamp's **lowest** tip position, the answer is no — and the whole
//! stamp is skipped without visiting a single cell.
//!
//! # Why `conservative_top` and not the ray tops
//!
//! Two channels have to be shown inert before a stamp can be skipped, not one:
//! the rays, and the sliver-safe `conservative_top` bound the rapid-collision
//! check reads. `conservative_top` is maintained as a **pointwise
//! over-estimate of the ray top** (see its docs on [`DexelGrid`]), so a single
//! mip over it dominates both:
//!
//! * `ray_top <= conservative_top <= tile_bound <= tip_lo <= tip + h(d)`, so
//!   `ray_blend_above` is a no-op — it only touches segments whose `exit`
//!   exceeds the surface;
//! * `lower_conservative_top(idx, tip_hi + h(far))` is a monotone *min*, and
//!   `tip_hi + h(far) >= tip_lo >= conservative_top[idx]`, so it is a no-op too.
//!
//! Both steps need `h >= 0`, which is checked on the actual table rather than
//! assumed — [`crate::radial_profile::RadialProfileLUT::profile_is_nonneg_total`].
//!
//! # Staleness is safe by construction
//!
//! `conservative_top` is monotone **decreasing** — `lower_conservative_top` is
//! the only writer and it takes a min. So a mip built at any earlier moment is
//! still an upper bound of the present state: it can only make the early-out
//! fire *less* often, never wrongly. That is what lets the refresh be
//! amortised instead of maintained per cell.
//!
//! The refresh is charged against stamped cell-visits: a rebuild costs one
//! pass over the grid, and it is only allowed once per
//! [`REFRESH_VISIT_MULTIPLIER`] grid-passes' worth of stamping, which bounds
//! the maintenance overhead at `1 / REFRESH_VISIT_MULTIPLIER` of the work it
//! is trying to remove.

use crate::dexel::DexelGrid;

/// log2 of the tile edge in cells. 16×16 = 256 cells per `f32`.
const TILE_LOG2: usize = 4;
const TILE: usize = 1 << TILE_LOG2;

/// How much stamping has to happen before a rebuild is allowed, measured in
/// grid-passes' worth of stamped cell-visits.
///
/// The amortisation is better than it first looks, and getting this wrong in
/// the *cautious* direction is what makes the whole-stamp early-out dead code.
/// A rebuild touches one `f32` per cell; a stamp visit runs a segment
/// projection, a coverage test, a LUT probe, three ray walks and a
/// read-modify-write. The per-cell cost ratio is order 1:40, so charging a
/// rebuild against **one** grid-pass of stamping bounds the maintenance
/// overhead at a few percent while keeping the mip fresh to within one
/// grid-pass of stamping.
///
/// It was first written as 32, on the theory that a rebuild is expensive.
/// Measured consequence: on both the `sim_kernel_lateral` fixture (161 k
/// cells, ~4.9 k cells per stamp, 960 stamps) and the exactness sentry, the
/// budget was never exhausted, the mip never left its build-time value of
/// "stock top everywhere", and the whole-stamp early-out fired **zero** times
/// in the entire suite. A stale mip is sound, which is exactly why that failure
/// mode is silent — `the_air_skip_is_bit_exact_and_not_vacuous` catches it
/// only because it asserts the skip is not vacuous.
const REFRESH_VISIT_MULTIPLIER: i64 = 1;

/// Coarse per-tile upper bound on `DexelGrid::conservative_top`.
pub(super) struct TileMaxTop {
    tile_max: Vec<f32>,
    tile_cols: usize,
    rows: usize,
    cols: usize,
    /// Remaining stamped-cell-visit budget before a rebuild is due.
    budget: i64,
    /// Cost of one rebuild, in cell-visits, times [`REFRESH_VISIT_MULTIPLIER`].
    budget_reset: i64,
    /// How much the early-out actually removed. Not read by production — it
    /// exists so the exactness sentry can prove it is not vacuous (a skip that
    /// never fires is trivially exact and worth nothing) and so the delta doc
    /// can quote a measured hit rate rather than a predicted one.
    stats: SkipStats,
}

/// Counters for what the S2 early-out removed. Serial-side only: every update
/// happens where the caller already holds `&mut TileMaxTop`.
#[derive(Clone, Copy, Default, Debug)]
pub(super) struct SkipStats {
    /// Stamps that returned without visiting a cell.
    pub(super) stamps_skipped: u64,
    /// Stamps that ran the cell loop.
    pub(super) stamps_run: u64,
    /// Cells the per-cell early-out short-circuited.
    pub(super) cells_skipped: u64,
    /// Cells the cell loop reached at all (i.e. inside a run stamp's bbox).
    pub(super) cells_in_bbox: u64,
}

impl TileMaxTop {
    /// Build a mip over `grid`'s current `conservative_top`.
    pub(super) fn build(grid: &DexelGrid) -> Self {
        let tile_cols = grid.cols.div_ceil(TILE);
        let tile_rows = grid.rows.div_ceil(TILE);
        let cells = (grid.rows * grid.cols) as i64;
        let mut mip = Self {
            tile_max: vec![f32::NEG_INFINITY; tile_rows * tile_cols],
            tile_cols,
            rows: grid.rows,
            cols: grid.cols,
            budget: 0,
            budget_reset: cells.saturating_mul(REFRESH_VISIT_MULTIPLIER).max(1),
            stats: SkipStats::default(),
        };
        mip.rebuild(grid);
        mip
    }

    /// Record a whole-stamp skip (no cell visited).
    #[inline]
    pub(super) fn note_stamp_skipped(&mut self) {
        self.stats.stamps_skipped += 1;
    }

    /// Record a stamp that ran its cell loop: `in_bbox` cells reached, of
    /// which `skipped` took the per-cell early-out.
    #[inline]
    pub(super) fn note_stamp_run(&mut self, in_bbox: u64, skipped: u64) {
        self.stats.stamps_run += 1;
        self.stats.cells_in_bbox += in_bbox;
        self.stats.cells_skipped += skipped;
    }

    #[cfg(test)]
    pub(super) fn stats(&self) -> SkipStats {
        self.stats
    }

    /// Charge stamped cell-visits against the refresh budget.
    #[inline]
    pub(super) fn charge(&mut self, cell_visits: u64) {
        self.budget -= cell_visits as i64;
    }

    /// Rebuild if the budget has run out. Call before [`Self::max_over`] so
    /// the answer is as tight as the amortisation policy allows.
    pub(super) fn refresh_if_due(&mut self, grid: &DexelGrid) {
        // A grid swapped underneath the mip (a different `StockCutDirection`
        // reaching a side grid) invalidates the shape, not just the values.
        if grid.rows != self.rows || grid.cols != self.cols {
            *self = Self::build(grid);
            return;
        }
        if self.budget <= 0 {
            self.rebuild(grid);
        }
    }

    /// Fold one stamp's band-reduced diagnostics in (S3 path).
    #[inline]
    pub(super) fn absorb(&mut self, partial: &super::stamping::StampPartial) {
        self.charge(partial.bbox_cells);
        if partial.stamp_skipped {
            self.stats.stamps_skipped += 1;
        } else {
            self.stats.stamps_run += 1;
            self.stats.cells_in_bbox += partial.bbox_cells;
            self.stats.cells_skipped += partial.cells_skipped;
        }
    }

    /// Fold one playback stamp's band-reduced diagnostics in (SIM w6 path).
    ///
    /// The twin of [`Self::absorb`] for the non-metric kernel, which has no
    /// per-cell early-out counter to report — the serial playback kernel passes
    /// a literal `0` for `skipped` to [`Self::note_stamp_run`], and this keeps
    /// that exactly.
    #[inline]
    pub(super) fn absorb_playback(&mut self, partial: &super::stamping::PlaybackPartial) {
        self.charge(partial.bbox_cells);
        if partial.stamp_skipped {
            self.stats.stamps_skipped += 1;
        } else {
            self.stats.stamps_run += 1;
            self.stats.cells_in_bbox += partial.bbox_cells;
        }
    }

    #[allow(clippy::indexing_slicing)] // bounded by rows/cols, checked above
    fn rebuild(&mut self, grid: &DexelGrid) {
        for slot in self.tile_max.iter_mut() {
            *slot = f32::NEG_INFINITY;
        }
        let ct = &grid.conservative_top;
        for row in 0..self.rows {
            let tile_row_base = (row >> TILE_LOG2) * self.tile_cols;
            let cell_base = row * self.cols;
            for col in 0..self.cols {
                let v = ct[cell_base + col];
                let t = tile_row_base + (col >> TILE_LOG2);
                if v > self.tile_max[t] {
                    self.tile_max[t] = v;
                }
            }
        }
        self.budget = self.budget_reset;
    }

    /// Upper bound on `conservative_top` over the inclusive cell rectangle.
    ///
    /// The bound is taken over whole tiles, so it covers a superset of the
    /// rectangle — which is the safe direction: a looser bound skips less.
    #[allow(clippy::indexing_slicing)] // tile indices derived from clamped cells
    pub(super) fn max_over(
        &self,
        row_lo: usize,
        row_hi: usize,
        col_lo: usize,
        col_hi: usize,
    ) -> f32 {
        if row_lo > row_hi || col_lo > col_hi || self.rows == 0 || self.cols == 0 {
            return f32::NEG_INFINITY;
        }
        let tr_lo = row_lo >> TILE_LOG2;
        let tr_hi = (row_hi.min(self.rows - 1)) >> TILE_LOG2;
        let tc_lo = col_lo >> TILE_LOG2;
        let tc_hi = (col_hi.min(self.cols - 1)) >> TILE_LOG2;
        let mut best = f32::NEG_INFINITY;
        for tr in tr_lo..=tr_hi {
            let base = tr * self.tile_cols;
            for tc in tc_lo..=tc_hi {
                let v = self.tile_max[base + tc];
                if v > best {
                    best = v;
                }
            }
        }
        best
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;
    use crate::geo::{BoundingBox3, P3};

    fn grid(rows_mm: f64, cols_mm: f64, cs: f64) -> DexelGrid {
        DexelGrid::z_grid_from_bounds(
            &BoundingBox3 {
                min: P3::new(0.0, 0.0, 0.0),
                max: P3::new(cols_mm, rows_mm, 10.0),
            },
            cs,
        )
    }

    /// The whole soundness argument is "the mip is an upper bound of every
    /// cell it covers". Checked exhaustively against a brute-force max over
    /// every rectangle in a small grid, on a deliberately ragged shape whose
    /// last tile row and column are partial.
    #[test]
    fn tile_bound_dominates_every_cell_it_covers() {
        let mut g = grid(9.0, 11.0, 0.25); // 37 rows × 45 cols — ragged tiles
        // Give the field some structure so a bug cannot hide behind a
        // constant.
        for row in 0..g.rows {
            for col in 0..g.cols {
                let idx = row * g.cols + col;
                let v = ((row * 7 + col * 13) % 23) as f32 * 0.25;
                g.conservative_top[idx] = v;
            }
        }
        let mip = TileMaxTop::build(&g);
        for row_lo in (0..g.rows).step_by(3) {
            for row_hi in (row_lo..g.rows).step_by(5) {
                for col_lo in (0..g.cols).step_by(3) {
                    for col_hi in (col_lo..g.cols).step_by(5) {
                        let bound = mip.max_over(row_lo, row_hi, col_lo, col_hi);
                        let mut truth = f32::NEG_INFINITY;
                        for row in row_lo..=row_hi {
                            for col in col_lo..=col_hi {
                                truth = truth.max(g.conservative_top[row * g.cols + col]);
                            }
                        }
                        assert!(
                            bound >= truth,
                            "mip bound {bound} < true max {truth} over \
                             rows {row_lo}..={row_hi} cols {col_lo}..={col_hi}"
                        );
                    }
                }
            }
        }
    }

    /// Staleness must only ever LOOSEN the bound. Lower cells without
    /// refreshing and the bound has to stay valid — that is what lets the
    /// rebuild be amortised.
    #[test]
    fn lowering_cells_without_a_refresh_keeps_the_bound_valid() {
        let mut g = grid(8.0, 8.0, 0.25);
        let mip = TileMaxTop::build(&g);
        let before = mip.max_over(0, g.rows - 1, 0, g.cols - 1);
        for idx in 0..g.rays.len() {
            g.lower_conservative_top(idx, -5.0);
        }
        // Not refreshed: still the old (higher) value, still an upper bound.
        let after = mip.max_over(0, g.rows - 1, 0, g.cols - 1);
        assert_eq!(after.to_bits(), before.to_bits());
        assert!(after >= -5.0);
    }

    /// A refresh must actually tighten, or the early-out never fires inside a
    /// long toolpath.
    #[test]
    fn a_refresh_tightens_the_bound() {
        let mut g = grid(8.0, 8.0, 0.25);
        let mut mip = TileMaxTop::build(&g);
        for idx in 0..g.rays.len() {
            g.lower_conservative_top(idx, -5.0);
        }
        // Charge more than the whole budget so the refresh is due.
        mip.charge(u64::MAX / 4);
        mip.refresh_if_due(&g);
        let after = mip.max_over(0, g.rows - 1, 0, g.cols - 1);
        assert!((after - -5.0).abs() < 1e-6, "after refresh: {after}");
    }

    /// The refresh cadence has to be amortised, not per stamp: a single
    /// stamp-sized batch of visits must NOT trigger a rebuild, and a full
    /// grid-pass worth of them MUST.
    ///
    /// Both halves matter and the second one is the one that was wrong: too
    /// lazy a cadence leaves the mip pinned at its build-time value and the
    /// whole-stamp early-out never fires, silently, because a stale mip is
    /// still sound.
    #[test]
    fn the_refresh_cadence_is_amortised_but_not_asleep() {
        let mut g = grid(8.0, 8.0, 0.25);
        let cells = g.rows * g.cols;
        // Build FIRST, so both mips start from the untouched stock top and
        // the only thing that can tighten them is a refresh.
        let mut lazy = TileMaxTop::build(&g);
        let mut due = TileMaxTop::build(&g);
        for idx in 0..g.rays.len() {
            g.lower_conservative_top(idx, -5.0);
        }

        lazy.charge((cells / 8) as u64);
        lazy.refresh_if_due(&g);
        assert!(
            lazy.max_over(0, g.rows - 1, 0, g.cols - 1) > 0.0,
            "rebuilt after an eighth of a grid pass — not amortised"
        );

        due.charge(cells as u64);
        due.refresh_if_due(&g);
        assert!(
            (due.max_over(0, g.rows - 1, 0, g.cols - 1) - -5.0).abs() < 1e-6,
            "one full grid-pass of stamped visits did not refresh the mip"
        );
    }
}
