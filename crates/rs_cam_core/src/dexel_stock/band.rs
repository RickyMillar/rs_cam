//! Row-band decomposition of a [`DexelGrid`] — the substrate for
//! `PERF_REVIEW.md` **S3** (parallel stamping).
//!
//! A [`GridBand`] is a contiguous run of grid rows carrying exclusive mutable
//! access to *its own* rays and `conservative_top` entries and to nothing
//! else. The stamp kernels work on a band rather than on a whole grid, so the
//! serial path (one band covering every row) and the parallel path (many
//! bands, one per rayon task) run **the same code**.
//!
//! # Why row bands, and why per-cell order survives
//!
//! `ray_blend_above` with `f < 1` is not commutative: blending `f₁` then `f₂`
//! is not blending `f₂` then `f₁` unless one of them is 1. So a decomposition
//! is only safe if every cell still sees its stamps in the same sequence.
//! Row bands give that for free — a cell belongs to exactly one band, and a
//! band applies stamps in the order it was handed them. No cell is ever
//! touched by two workers, so there is nothing to order between workers.
//!
//! The rays and the sliver-safe channel are split with the **same** chunk
//! size and zipped, so a band's two slices always describe the same rows.
//!
//! # What is NOT preserved, and it is not a detail
//!
//! `stamp_segment_with_metrics` accumulates `pre_volume` and `post_volume` as
//! two running `f64` sums over covered cells in row-major order and
//! *differences them at the end*. Splitting the rows splits those sums, and
//! `(a₁+a₂)+(a₃+a₄) != ((a₁+a₂)+a₃)+a₄` in floating point. So banding
//! **reassociates the removed-volume sum**; it does not reproduce the serial
//! value bit-for-bit. Everything else does: the rays, `conservative_top`,
//! `axial_doc_mm` (a max), `radial_engagement` (from a min and a max), the arc
//! derived from it, and the sample stream's order, indices and timings.
//!
//! The band decomposition is a function of the grid's row count alone — never
//! of `rayon::current_num_threads()` — so the reassociation is *fixed*: one
//! thread and N threads produce bit-identical results, which is what
//! `band_decomposition_is_thread_count_independent` pins.

use rayon::prelude::*;

use crate::dexel::{DexelGrid, DexelRay};

/// A contiguous run of rows of a [`DexelGrid`].
pub(super) struct GridBand<'a> {
    pub(super) rays: &'a mut [DexelRay],
    pub(super) conservative_top: &'a mut [f32],
    /// Global row index of this band's local row 0.
    pub(super) row_offset: usize,
    /// Rows in this band.
    pub(super) rows: usize,
    /// Columns — the same for every band, and the row stride.
    pub(super) cols: usize,
    pub(super) origin_u: f64,
    pub(super) origin_v: f64,
    pub(super) cell_size: f64,
}

impl GridBand<'_> {
    /// Flat index into this band's slices for a **global** (row, col).
    #[inline]
    pub(super) fn local(&self, row: usize, col: usize) -> usize {
        (row - self.row_offset) * self.cols + col
    }

    /// Last global row this band owns. Callers clamp their stamp bounding box
    /// to `[row_offset, last_row]` before indexing.
    #[inline]
    pub(super) fn last_row(&self) -> usize {
        self.row_offset + self.rows - 1
    }

    /// Monotone min on the sliver-safe channel — the band-local twin of
    /// [`DexelGrid::lower_conservative_top`], with the same contract: a caller
    /// that stamps the same ground twice cannot walk the bound back up.
    #[inline]
    #[allow(clippy::indexing_slicing)] // caller derives `local` from clamped bounds
    pub(super) fn lower_conservative_top(&mut self, local: usize, surface: f32) {
        if surface < self.conservative_top[local] {
            self.conservative_top[local] = surface;
        }
    }
}

/// Rows per band. **A constant, deliberately, and not a function of the
/// thread count.**
///
/// The band boundaries decide how the per-stamp volume sums are reassociated
/// (see the module docs). If they moved with `rayon::current_num_threads()`
/// the simulator would return a different `removed_volume_est_mm3` on a
/// 4-core machine than on a 32-core one, and a golden captured on one would
/// be un-reproducible on the other. Fixing the granularity instead means
/// rayon may run the bands on any number of threads — including one — and get
/// the same answer, which is what the determinism sentry checks.
///
/// 8 rows keeps a typical stamp footprint (a Ø6 tool on a 0.1 mm grid spans
/// ~63 rows) spread across ~8 bands, which is enough to feed a desktop core
/// count without cutting the footprint so fine that each task is smaller than
/// a rayon dispatch.
pub(super) const BAND_ROWS: usize = 8;

// Coarsening the rayon split with `with_min_len` was tried and is NOT here.
// Grouping the 16 bands of a `flat12/cs0.1` stamp into 4 fatter tasks made
// every thread count slower — 105 ms at 4 threads against 78.5 ms without it,
// and 172 ms at 24 against 108.6 — so the fan-out is left at one band per
// task. The scaling table in `DELTA_sim_w2.md` is the measurement that
// matters; the ceiling is the per-stamp dispatch itself, not how the bands
// are grouped inside it.

impl DexelGrid {
    /// The band index range that can possibly overlap global rows
    /// `[row_lo, row_hi]`, as `(skip, take)`.
    ///
    /// Restricting the fan-out to the stamp's own rows is not an optimisation
    /// of the *result* — a band outside the stamp returns
    /// `StampPartial::empty()`, which is the identity of `merge` — but it is
    /// the difference between visiting `grid_rows / BAND_ROWS` bands per stamp
    /// and visiting `footprint_rows / BAND_ROWS`. On a 500-row grid with a Ø6
    /// tool at 0.1 mm that is 63 bands versus 9, per subsegment.
    fn band_span(&self, row_lo: usize, row_hi: usize) -> (usize, usize) {
        let total = self.rows.div_ceil(BAND_ROWS);
        if self.rows == 0 || self.cols == 0 || row_lo > row_hi || row_lo >= self.rows {
            return (0, 0);
        }
        let first = row_lo / BAND_ROWS;
        let last = (row_hi.min(self.rows - 1)) / BAND_ROWS;
        (first, (last + 1 - first).min(total.saturating_sub(first)))
    }

    /// The same decomposition as [`Self::par_bands`], driven serially.
    ///
    /// This exists so the parallel/serial choice is **purely a scheduling
    /// decision**. Both paths see the identical band boundaries and merge in
    /// the identical order, so switching between them cannot move a single
    /// bit — which is what makes the small-stamp cutoff safe to tune, and
    /// what the determinism sentry rests on.
    pub(super) fn serial_bands(
        &mut self,
        row_lo: usize,
        row_hi: usize,
    ) -> impl Iterator<Item = GridBand<'_>> {
        let (skip, take) = self.band_span(row_lo, row_hi);
        let cols = self.cols;
        let (origin_u, origin_v, cell_size) = (self.origin_u, self.origin_v, self.cell_size);
        let chunk = BAND_ROWS.saturating_mul(cols).max(1);
        self.rays
            .chunks_mut(chunk)
            .zip(self.conservative_top.chunks_mut(chunk))
            .enumerate()
            .skip(skip)
            .take(take)
            .map(move |(i, (rays, conservative_top))| GridBand {
                rows: if cols == 0 { 0 } else { rays.len() / cols },
                row_offset: i * BAND_ROWS,
                cols,
                origin_u,
                origin_v,
                cell_size,
                rays,
                conservative_top,
            })
    }

    /// Split into row bands of [`BAND_ROWS`] rows for parallel stamping.
    ///
    /// The two side arrays are chunked with the same stride and zipped, so a
    /// band's rays and `conservative_top` always describe the same rows.
    pub(super) fn par_bands(
        &mut self,
        row_lo: usize,
        row_hi: usize,
    ) -> impl IndexedParallelIterator<Item = GridBand<'_>> {
        let (skip, take) = self.band_span(row_lo, row_hi);
        let cols = self.cols;
        let (origin_u, origin_v, cell_size) = (self.origin_u, self.origin_v, self.cell_size);
        let chunk = BAND_ROWS.saturating_mul(cols).max(1);
        self.rays
            .par_chunks_mut(chunk)
            .zip(self.conservative_top.par_chunks_mut(chunk))
            .enumerate()
            .skip(skip)
            .take(take)
            .map(move |(i, (rays, conservative_top))| GridBand {
                rows: if cols == 0 { 0 } else { rays.len() / cols },
                row_offset: i * BAND_ROWS,
                cols,
                origin_u,
                origin_v,
                cell_size,
                rays,
                conservative_top,
            })
    }
}

/// Global row range a stamp can possibly touch — a **superset** of the range
/// the kernel itself computes, which is all the band fan-out needs.
///
/// The kernel's own scan radius is `radius + cs·(3/8)·√2 ≈ radius + 0.53·cs`;
/// this uses `radius + 2·cs`, so it can only be wider. A band the kernel would
/// have found empty contributes `StampPartial::empty()`, the identity of
/// `merge`, so widening costs time and narrowing would cost correctness.
pub(super) fn stamp_row_span(
    grid: &DexelGrid,
    radius: f64,
    start: (f64, f64, f64),
    end: (f64, f64, f64),
) -> (usize, usize) {
    let cs = grid.cell_size;
    if cs <= 0.0 || grid.rows == 0 {
        return (0, 0);
    }
    let scan = radius + 2.0 * cs;
    let v_min = start.1.min(end.1) - scan;
    let v_max = start.1.max(end.1) + scan;
    let lo = ((v_min - grid.origin_v) / cs).floor().max(0.0) as usize;
    let hi = (((v_max - grid.origin_v) / cs).ceil().max(0.0) as usize).min(grid.rows - 1);
    (lo, hi)
}

/// Cells in a stamp's swept bounding box below which the bands are driven
/// serially instead of through rayon.
///
/// Purely a scheduling threshold: [`DexelGrid::serial_bands`] and
/// [`DexelGrid::par_bands`] produce the same bands in the same order, so
/// crossing it cannot change a result.
///
/// **Measured, not guessed, and the number is the finding.** A stamp is a few
/// tens of microseconds of work — the whole `sim_kernel_lateral/flat6/cs0.1`
/// arm is 960 subsegments in ~50 ms, i.e. 52 µs each — and a rayon dispatch
/// plus the cache traffic of handing a row band to whichever worker steals it
/// is not free. Paired A/B at 24 threads, S2-only versus S3:
///
/// | arm | est. bbox cells | S3 / S2 |
/// |---|---:|---:|
/// | `flat6/cs0.1` | ~4.3 k | **0.72×** — a 28 % LOSS |
/// | `flat12/cs0.1` | ~15.7 k | **1.48×** |
///
/// So the crossover is somewhere in between, and this is set above the losing
/// point. The deeper reading is in `DELTA_sim_w2.md`: per-*stamp* dispatch
/// cannot reach the review's predicted 6–12× at any threshold, because the
/// work per dispatch is bounded by one subsegment's footprint. Amortising the
/// dispatch over a whole toolpath — which also gives each band cache affinity
/// instead of letting rows migrate between cores on every subsegment — is a
/// different piece of work.
const PARALLEL_MIN_BBOX_CELLS: f64 = 12_000.0;

/// Should this stamp be dispatched across threads?
///
/// Estimated from the geometry rather than measured, because the real cell
/// count is only known after the bands have run. The estimate is the swept
/// bounding box the kernel will compute, and it is used for **nothing but the
/// serial/parallel choice**.
pub(super) fn stamp_wants_threads(
    grid: &DexelGrid,
    radius: f64,
    start: (f64, f64, f64),
    end: (f64, f64, f64),
) -> bool {
    if grid.rows <= BAND_ROWS {
        return false;
    }
    let cs = grid.cell_size;
    if cs <= 0.0 {
        return false;
    }
    let scan = radius + cs;
    let span_u = (end.0 - start.0).abs() + 2.0 * scan;
    let span_v = (end.1 - start.1).abs() + 2.0 * scan;
    (span_u / cs + 2.0) * (span_v / cs + 2.0) >= PARALLEL_MIN_BBOX_CELLS
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

    fn grid(cs: f64) -> DexelGrid {
        DexelGrid::z_grid_from_bounds(
            &BoundingBox3 {
                min: P3::new(0.0, 0.0, 0.0),
                max: P3::new(11.0, 9.0, 10.0),
            },
            cs,
        )
    }

    /// Every cell must belong to exactly one band, and the band's own
    /// `local()` must map it back to the same slot the whole-grid layout uses.
    /// That equivalence is what lets the parallel and serial paths be the same
    /// code.
    #[test]
    fn bands_partition_every_cell_exactly_once() {
        let mut g = grid(0.25);
        let (rows, cols) = (g.rows, g.cols);
        // Stamp a unique value into every cell through the band view, then
        // check it landed at the flat index the grid itself would use.
        let mut seen = vec![0u32; rows * cols];
        let stamped: Vec<(usize, usize, f32)> = g
            .par_bands(0, rows - 1)
            .flat_map_iter(|band| {
                let mut out = Vec::new();
                for row in band.row_offset..=band.last_row() {
                    for col in 0..band.cols {
                        let local = band.local(row, col);
                        let v = (row * 1000 + col) as f32;
                        band.conservative_top[local] = v;
                        out.push((row, col, v));
                    }
                }
                out
            })
            .collect();
        assert_eq!(
            stamped.len(),
            rows * cols,
            "band coverage is not a partition"
        );
        for (row, col, v) in stamped {
            seen[row * cols + col] += 1;
            assert_eq!(
                g.conservative_top[row * cols + col].to_bits(),
                v.to_bits(),
                "band write landed at the wrong flat index for ({row}, {col})"
            );
        }
        assert!(
            seen.iter().all(|&n| n == 1),
            "some cell was visited by zero or more than one band"
        );
    }

    /// The band granularity must not move with the machine. If it did, the
    /// reassociated volume sum — and therefore every golden that pins it —
    /// would depend on the core count of whoever ran it.
    #[test]
    fn band_granularity_is_a_constant_not_a_thread_count() {
        let mut g = grid(0.5);
        let expected = g.rows.div_ceil(BAND_ROWS);
        let count = g.par_bands(0, g.rows - 1).count();
        assert_eq!(
            count, expected,
            "band count {count} is not rows/BAND_ROWS = {expected}"
        );
    }
}
