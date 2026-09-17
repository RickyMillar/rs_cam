//! One batch dispatcher, shared by the metric route and the playback route.
//!
//! STK-09: `whole_path.rs` and `playback.rs` held two copies of this driver —
//! eleven methods with the same names in the same order, ten fields with the
//! same names, and four of five constants with the same values. The folder
//! invariant says the metric route and the playback route must agree on the
//! stamped volume. One driver makes that structural instead of reviewed.
//!
//! # The three phases
//!
//! 1. **Enumerate (serial).** The move loop in `simulation.rs` records a job
//!    and moves on. It is the only place a `SimulationCutSample` is pushed, so
//!    the sample stream cannot drift between the two routes.
//! 2. **Replay (parallel).** One `par_bands` call per batch, over the batch's
//!    own union row span, zipped with per-band job buckets. A cell belongs to
//!    exactly one band and a band replays its jobs in job order, so every cell
//!    still sees its stamps in the original sequence — which is what
//!    `ray_blend_above`'s non-commutativity at `f < 1` requires.
//! 3. **Reduce and finish (serial).** Each job's bands merge in ascending band
//!    order — the same order, over the same band set, that the per-stamp path
//!    merges in. The reassociation of the `f64` volume sums is therefore not
//!    merely deterministic: it is bit-identical to the per-stamp path. Then the
//!    caller's `finish` closure runs once per job, in job order.
//!
//! # Dispatch only the bands the batch can reach
//!
//! Rayon splits an indexed parallel iterator by index RANGE, and a batch's
//! stamps are consecutive, so its active bands are a contiguous run. Handing
//! `par_bands` the whole grid gives one worker every active band and the others
//! a pile of empties. Both waves found this defect once (`DELTA_sim_w2.md` §3e,
//! `DELTA_sim_w4.md` §3d), and it presents as "parallelism does not help here"
//! rather than as a bug. The restriction is numerically free: an excluded band
//! has an empty bucket and would have written nothing.
//!
//! # What the batch boundary costs, and what it does not
//!
//! The S2 air-skip mip cannot be maintained inside the parallel phase: a
//! rebuild reads `conservative_top` across the whole grid, including rows other
//! workers are mutating. So it is refreshed at batch boundaries only, and
//! inside a batch every band asks the same immutable mip about each stamp's
//! global bounding box. A staler mip fires the whole-stamp early-out LESS
//! often, never wrongly — `conservative_top` is monotone decreasing, so an
//! older mip is still an upper bound. Mip freshness is a speed dial and nothing
//! else. That is why a batch is sized against the mip's own refresh budget
//! rather than as large as memory allows.

use std::sync::atomic::{AtomicU64, Ordering};

use rayon::prelude::*;

use super::band::{self, BAND_ROWS, GridBand};
use super::tile_mip::TileMaxTop;
use crate::interrupt::{CancelCheck, Cancelled, check_cancel};
use crate::stock::dexel::DexelGrid;

/// A queued stamp. The driver buckets a job by its endpoints and never reads
/// anything else about it.
pub(super) trait BandJob: Copy + Send + Sync {
    fn start(&self) -> (f64, f64, f64);
    fn end(&self) -> (f64, f64, f64);
}

/// What one band reports back about one job.
///
/// `merge` runs in ascending band order, once per band the job touched.
pub(super) trait BandPartial: Copy + Send + Sync {
    fn empty() -> Self;
    fn merge(&mut self, other: &Self);
}

/// The batch size limits. **The only numbers the two routes disagree about.**
#[derive(Clone, Copy, Debug)]
pub(super) struct BatchCaps {
    /// Hard cap on jobs per batch. Bounds the job and reduced arrays
    /// independently of how few bands each stamp happens to touch.
    pub(super) max_jobs: usize,
    /// Hard cap on `(band, stamp)` partials per batch — the memory bound that
    /// matters, and the reason a batch exists at all.
    pub(super) max_partials: usize,
    /// Fewest jobs a batch takes before the visit budget may close it. Without
    /// a floor, a grid small enough that one stamp covers it would dispatch per
    /// stamp — the thing this driver exists to stop doing.
    pub(super) min_jobs: usize,
    /// How much stamping a batch absorbs before it closes, in whole grid-passes
    /// of estimated cell-visits. The mip-freshness dial, not a memory one.
    pub(super) visit_budget_passes: u64,
}

/// What the dispatcher actually did. **No production reader.** It exists so the
/// route sentries can prove the dispatch is not vacuous: a fixture that
/// resolves to one band, or to one batch, exercises nothing while passing every
/// bit-identity check for free.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct BandBatchStats {
    /// Batches closed — each one is exactly one `par_bands` dispatch.
    pub(super) batches: u64,
    /// Bands the grid is split into.
    pub(super) bands: usize,
    /// Most stamps any one batch carried.
    pub(super) max_jobs_in_a_batch: usize,
    /// Most `(band, stamp)` partials any one batch held at once.
    pub(super) max_partials_in_a_batch: usize,
    /// **Band closure bodies actually executed**, counted from inside the
    /// parallel region.
    ///
    /// The other four counters are recorded on the serial side and would keep
    /// counting if the `par_bands` call itself were reduced to nothing — an
    /// empty dispatch range, a truncating `zip`, a slice that came back `None`.
    /// This one cannot: the closure that does the stamping increments it. A
    /// stale gate that fires zero times is silently sound, which is this
    /// programme's most-repeated trap.
    pub(super) band_tasks_run: u64,
}

/// The batching driver. One per run; reused across batches so the bucket and
/// partial arrays are allocated once.
pub(super) struct BandBatch<J, P> {
    jobs: Vec<J>,
    /// Job indices per band, in job order.
    buckets: Vec<Vec<u32>>,
    /// Per-band partials, positionally aligned with `buckets`.
    outs: Vec<Vec<P>>,
    /// Per-job reduced partial.
    reduced: Vec<P>,
    /// Bands the arrays above are sized for.
    bands: usize,
    /// Union of the queued jobs' row spans — the only rows the batch can touch,
    /// and therefore the only bands worth dispatching. `None` when nothing in
    /// the queue reaches the grid.
    active_rows: Option<(usize, usize)>,
    pending_partials: usize,
    pending_visits: u64,
    visit_budget: u64,
    caps: BatchCaps,
    band_tasks_run: AtomicU64,
    stats: BandBatchStats,
}

impl<J: BandJob, P: BandPartial> BandBatch<J, P> {
    /// Size the arrays for `grid` and the visit budget for its cell count.
    /// The caller decides whether banded dispatch is wanted at all.
    pub(super) fn new(grid: &DexelGrid, caps: BatchCaps) -> Self {
        let bands = grid.rows.div_ceil(BAND_ROWS);
        let cells = (grid.rows as u64).saturating_mul(grid.cols as u64).max(1);
        Self {
            jobs: Vec::new(),
            buckets: vec![Vec::new(); bands],
            outs: vec![Vec::new(); bands],
            reduced: Vec::new(),
            bands,
            active_rows: None,
            pending_partials: 0,
            pending_visits: 0,
            visit_budget: cells.saturating_mul(caps.visit_budget_passes),
            caps,
            band_tasks_run: AtomicU64::new(0),
            stats: BandBatchStats {
                bands,
                ..BandBatchStats::default()
            },
        }
    }

    /// Queue one stamp, and account for what it will cost.
    ///
    /// Only the *counters* are computed here — the buckets themselves are built
    /// in [`Self::run`], against the grid that is about to be stamped.
    /// Bucketing at push time would leave the band indices aliased to whatever
    /// grid shape happened to be current when the job was queued, and `zip`
    /// would then silently truncate rather than fail. It costs one extra pass
    /// over the queue per batch, against a stamp kernel that is orders of
    /// magnitude more expensive.
    pub(super) fn push(&mut self, grid: &DexelGrid, radius: f64, job: J) {
        let (start, end) = (job.start(), job.end());
        self.jobs.push(job);
        if let Some((row_lo, row_hi)) = band::stamp_row_span(grid, radius, start, end) {
            self.pending_partials += grid.band_span(row_lo, row_hi).1;
        }
        self.pending_visits = self
            .pending_visits
            .saturating_add(band::stamp_bbox_cells(grid, radius, start, end) as u64);
    }

    /// Is the queued batch full enough to run?
    pub(super) fn batch_is_due(&self) -> bool {
        self.jobs.len() >= self.caps.max_jobs
            || self.pending_partials >= self.caps.max_partials
            || (self.jobs.len() >= self.caps.min_jobs && self.pending_visits >= self.visit_budget)
    }

    pub(super) fn is_empty(&self) -> bool {
        self.jobs.is_empty()
    }

    pub(super) fn batch_stats(&self) -> BandBatchStats {
        BandBatchStats {
            bands: self.bands,
            band_tasks_run: self.band_tasks_run.load(Ordering::Relaxed),
            ..self.stats
        }
    }

    /// Fill `buckets` from the queued jobs against `grid`'s current shape.
    ///
    /// Job indices land in ascending order inside each bucket, which is what
    /// makes a band replay its share of the batch in the original stamp
    /// sequence.
    fn build_buckets(&mut self, grid: &DexelGrid, radius: f64) {
        let bands = grid.rows.div_ceil(BAND_ROWS);
        if bands != self.bands {
            self.buckets = vec![Vec::new(); bands];
            self.outs = vec![Vec::new(); bands];
            self.bands = bands;
        } else {
            for bucket in self.buckets.iter_mut() {
                bucket.clear();
            }
        }
        self.pending_partials = 0;
        self.active_rows = None;
        for (idx, job) in self.jobs.iter().enumerate() {
            let Some((row_lo, row_hi)) = band::stamp_row_span(grid, radius, job.start(), job.end())
            else {
                continue;
            };
            self.active_rows = Some(match self.active_rows {
                None => (row_lo, row_hi),
                Some((lo, hi)) => (lo.min(row_lo), hi.max(row_hi)),
            });
            let (skip, take) = grid.band_span(row_lo, row_hi);
            for bucket in self.buckets.iter_mut().skip(skip).take(take) {
                bucket.push(idx as u32);
            }
            self.pending_partials += take;
        }
    }

    /// Run the queued batch: refresh the mip, replay every band in parallel,
    /// reduce in band order, then call `finish` once per job in job order.
    ///
    /// `stamp` is the route's kernel. It receives one band, one job and the
    /// batch's immutable mip, and returns that band's partial.
    /// `finish` is the route's serial tail: the metric route fixes the
    /// degenerate flags, absorbs the mip bookkeeping and patches the sample;
    /// the playback route only absorbs.
    ///
    /// # Cancellation granularity, stated
    ///
    /// The token is polled **once per batch**, serially, at the top of this
    /// function — and the enumerating loop in `simulation.rs` still polls it
    /// once per subsegment, unchanged. It is deliberately NOT polled inside the
    /// parallel phase: `CancelCheck` carries no `Sync` bound, so reaching it
    /// from a rayon worker would mean widening a signature the whole crate
    /// depends on.
    ///
    /// What that bounds is the GRID, not the toolpath: a batch closes at
    /// [`BatchCaps::visit_budget_passes`] grid-passes of estimated stamped
    /// cell-visits, [`BatchCaps::max_jobs`] stamps, or
    /// [`BatchCaps::max_partials`] partials, whichever comes first. On the
    /// shipped 0.4 mm wanaka grid that is tens of milliseconds; on a 4 M-cell
    /// grid, the largest this simulator is asked for, it is ~16 M cell-visits,
    /// order half a second. Small fixtures close on the job cap, which is
    /// tighter still.
    ///
    /// A `Sync` cancel token is the honest fix and would let the poll move
    /// inside the band loop; it is a crate-wide signature change.
    pub(super) fn run<F, G>(
        &mut self,
        grid: &mut DexelGrid,
        radius: f64,
        air_mip: &mut Option<TileMaxTop>,
        cancel: &dyn CancelCheck,
        stamp: F,
        mut finish: G,
    ) -> Result<(), Cancelled>
    where
        F: Fn(&mut GridBand<'_>, &J, Option<&TileMaxTop>) -> P + Sync,
        G: FnMut(&J, &mut P, &mut Option<TileMaxTop>),
    {
        if self.jobs.is_empty() {
            return Ok(());
        }
        check_cancel(cancel)?;

        if let Some(m) = air_mip.as_mut() {
            m.refresh_if_due(grid);
        }

        self.build_buckets(grid, radius);
        self.stats.batches += 1;
        self.stats.max_jobs_in_a_batch = self.stats.max_jobs_in_a_batch.max(self.jobs.len());
        self.stats.max_partials_in_a_batch = self
            .stats
            .max_partials_in_a_batch
            .max(self.pending_partials);

        // ── Phase 2: one dispatch, every band, every stamp in the batch ──
        //
        // Disjoint field borrows, spelled out because the closure needs `jobs`
        // immutably while `outs` is borrowed mutably.
        let Self {
            jobs,
            buckets,
            outs,
            reduced,
            active_rows,
            band_tasks_run,
            ..
        } = self;
        let active_rows = *active_rows;
        let mip = air_mip.as_ref();
        let dispatch_rows = active_rows.and_then(|(row_lo, row_hi)| {
            let (skip, take) = grid.band_span(row_lo, row_hi);
            // The slice is in range by construction — `build_buckets` sized
            // `buckets` from this same grid and `band_span` bounds `skip+take`
            // by that size. Widening to the whole grid rather than bailing on
            // the `None` keeps a broken invariant SLOW instead of WRONG: a
            // truncating `zip` would silently drop bands, and a dropped band is
            // a stamp that never happened.
            if skip + take <= buckets.len() && skip + take <= outs.len() && take > 0 {
                Some((row_lo, row_hi, skip, take))
            } else if grid.rows > 0 {
                Some((0, grid.rows - 1, 0, buckets.len().min(outs.len())))
            } else {
                None
            }
        });
        if let Some((row_lo, row_hi, skip, take)) = dispatch_rows {
            let (Some(buckets), Some(outs)) = (
                buckets.get(skip..skip + take),
                outs.get_mut(skip..skip + take),
            ) else {
                return Ok(());
            };
            grid.par_bands(row_lo, row_hi)
                .zip(buckets.par_iter())
                .zip(outs.par_iter_mut())
                .for_each(|((mut band, bucket), out)| {
                    band_tasks_run.fetch_add(1, Ordering::Relaxed);
                    out.clear();
                    out.reserve(bucket.len());
                    for &j in bucket.iter() {
                        let Some(job) = jobs.get(j as usize) else {
                            continue;
                        };
                        out.push(stamp(&mut band, job, mip));
                    }
                });
        }

        // ── Phase 3a: reduce, in ascending band order per job ──
        //
        // Which is the SAME order the per-stamp path merges in: it walks
        // `band_span(row_lo, row_hi)` ascending, and a job's buckets were filled
        // from exactly that range. So the volume sums are reassociated
        // identically, not merely deterministically.
        reduced.clear();
        reduced.resize(jobs.len(), P::empty());
        for (bucket, out) in buckets.iter().zip(outs.iter()) {
            for (k, &j) in bucket.iter().enumerate() {
                let (Some(slot), Some(partial)) = (reduced.get_mut(j as usize), out.get(k)) else {
                    continue;
                };
                slot.merge(partial);
            }
        }

        // ── Phase 3b: the route's own serial tail, in job order ──
        for (j, job) in jobs.iter().enumerate() {
            let Some(r) = reduced.get_mut(j) else {
                continue;
            };
            finish(job, r, air_mip);
        }

        self.clear_batch();
        Ok(())
    }

    fn clear_batch(&mut self) {
        self.jobs.clear();
        self.pending_partials = 0;
        self.pending_visits = 0;
    }
}
