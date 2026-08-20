//! Whole-toolpath row-band dispatch — `DELTA_sim_w2.md` §3f, the remaining
//! lever of `PERF_REVIEW.md` **S3**.
//!
//! Wave 2 landed the row-band decomposition and dispatched it **per stamp**.
//! That saturated at a measured **2.10× on four threads** and got worse past
//! eight, for a structural reason: a stamp is tens of microseconds of work, so
//! the join tree is a large fraction of it, and nothing pins a band to a worker
//! — the same rows migrate between cores on every subsegment.
//!
//! This module amortises the dispatch over many stamps instead. One
//! `par_bands` call per **batch**; each band replays every stamp in the batch
//! that touches its rows, clipped to those rows; the per-(band, stamp) partials
//! are reduced serially afterwards and patched back onto the already-emitted
//! sample stream.
//!
//! # The three phases, and why the sample stream cannot move
//!
//! 1. **Enumerate (serial).** The move loop in `simulation.rs` is *unchanged*
//!    and is the only place a `SimulationCutSample` is ever pushed. It emits
//!    every sample in its final order, with its final `sample_index`,
//!    `cumulative_time_s` and `segment_time_s`, and records a [`StampJob`]
//!    carrying the slot the metrics belong in. **There is one enumerator**,
//!    shared with the per-stamp path — the dispatch mode selects what happens
//!    to the geometry, never what happens to the stream. Ordering, numbering
//!    and timings are therefore identical by construction rather than by
//!    parallel maintenance of two loops.
//! 2. **Replay (parallel).** One `par_bands` over the batch's own union row
//!    span, zipped with the per-band job buckets. A cell belongs to exactly one
//!    band and a band replays its jobs in job order, so every cell still sees
//!    its stamps in the original sequence — which is what `ray_blend_above`'s
//!    non-commutativity at `f < 1` requires. Restricting the span to the batch
//!    rather than the grid is a scheduling fix, not a numerical one, and it is
//!    load-bearing: see the note at the dispatch site.
//! 3. **Reduce and patch (serial).** For each job, its bands are merged in
//!    ascending band order — the same order, over the same band set, that the
//!    per-stamp path merges in. The reassociation of the volume sums is
//!    therefore not merely deterministic: it is **bit-identical to the
//!    per-stamp path**, and the four published numbers come out of the same
//!    `StampPartial::finish`.
//!
//! # What the batch boundary costs, and what it does not
//!
//! The S2 air-skip mip cannot be maintained inside the parallel phase: a
//! rebuild reads `conservative_top` across the whole grid, including rows other
//! workers are mutating. So it is refreshed at batch boundaries only, and
//! inside a batch every band asks the *same* immutable mip about each stamp's
//! *global* bounding box — which is what makes the whole-stamp early-out exact
//! here for the same reason it is exact per stamp (`DELTA_sim_w2.md` §3b): all
//! bands reach one verdict, so both volume sums stay at `0.0` or neither does.
//!
//! A staler mip fires the whole-stamp early-out **less** often, never wrongly
//! — `conservative_top` is monotone decreasing, so an older mip is still an
//! upper bound. And a stamp that is *not* skipped but whose cells are all inert
//! contributes the identical addend to `pre_volume` and `post_volume` in the
//! identical order, so its removed volume is exactly `0.0` either way. Mip
//! freshness is a **speed** dial and nothing else. That is why the batch is
//! sized against the mip's own refresh budget rather than as large as memory
//! allows.

use rayon::prelude::*;

use super::band::{self, BAND_ROWS};
use super::stamping::{StampPartial, stamp_segment_with_metrics};
use super::tile_mip::TileMaxTop;
use crate::dexel::DexelGrid;
use crate::interrupt::{CancelCheck, Cancelled, check_cancel};
use crate::radial_profile::RadialProfileLUT;

/// Which dispatch shape the metric simulator uses for its stamp kernel.
///
/// Purely a **schedule**. Both shapes run the same kernel over the same band
/// decomposition and merge the same partials in the same order, so the grid,
/// the sample stream and all four published metrics are bit-identical across
/// them — which is what `whole_path_dispatch_matches_per_stamp_bit_for_bit`
/// pins, and what makes [`StampDispatch::Auto`] safe to change.
/// **The default is not `#[derive(Default)]`.** It consults
/// `RS_CAM_STAMP_DISPATCH` once per process so a whole test binary, a bench, or
/// the CLI can be forced onto one shape without threading a flag through every
/// construction site — which is what the S1 decision package's
/// "run the full suite with the new mode on" measurement needs. Unset (the
/// shipped case) is [`StampDispatch::Auto`], unchanged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StampDispatch {
    /// Pick per simulation from the grid shape and the pool size.
    Auto,
    /// Wave 2's shape: one `par_bands` call per stamp. Kept because it is the
    /// A/B partner the wave-4 numbers are quoted against, and because it is
    /// what a grid with fewer than [`MIN_BANDS_FOR_WHOLE_PATH`] bands falls
    /// back to.
    PerStamp,
    /// Wave 4's shape: one `par_bands` call per batch of stamps.
    WholeToolpath,
    /// S1's shape: one pass per *chunk of consecutive subsegments*, metrics
    /// binned by closest-approach parameter (`super::swept`).
    ///
    /// **Not bit-identical to the other two, by design** — it is the only
    /// value of this enum that changes what the simulator measures. `Auto`
    /// never selects it; it must be asked for. See `swept.rs`'s module docs
    /// for what moves and why.
    Swept,
    /// S1's shape with the **metric-changing half switched off**: chunks are
    /// grown only where growing them is bit-identical.
    ///
    /// A one-bin swept chunk reproduces the shipped kernel exactly (proved by
    /// `swept_with_one_bin_matches_per_stamp_bit_for_bit`), and an
    /// exactly-vertical chunk of any length does too (the hoist is loop
    /// inversion, not approximation). So this mode kills S1's `by_z` redundancy
    /// — the 250-stamps-per-plunge arm — while leaving every published number
    /// where the shipped kernel put it. It is the part of S1 that needs no
    /// re-baseline and no decision.
    SweptPlungeOnly,
}

/// Process-wide dispatch override, parsed once. `None` means "not set".
fn dispatch_override() -> Option<StampDispatch> {
    static OVERRIDE: std::sync::OnceLock<Option<StampDispatch>> = std::sync::OnceLock::new();
    *OVERRIDE.get_or_init(|| {
        let raw = std::env::var("RS_CAM_STAMP_DISPATCH").ok()?;
        match raw.trim().to_ascii_lowercase().as_str() {
            "auto" => Some(StampDispatch::Auto),
            "per_stamp" | "per-stamp" | "perstamp" => Some(StampDispatch::PerStamp),
            "whole" | "whole_path" | "whole_toolpath" => Some(StampDispatch::WholeToolpath),
            "swept" => Some(StampDispatch::Swept),
            "swept_plunge" | "swept_plunge_only" => Some(StampDispatch::SweptPlungeOnly),
            _ => None,
        }
    })
}

impl Default for StampDispatch {
    fn default() -> Self {
        dispatch_override().unwrap_or(Self::Auto)
    }
}

/// What the whole-toolpath dispatcher actually did on the last metric
/// simulation — batches closed, and how big they got.
///
/// **No production reader.** It exists so the wave-4 sentries can prove the
/// dispatch is not vacuous: a fixture that resolves to one band, or to one
/// batch, exercises neither of the two things §3f claims (amortised join cost,
/// band-to-core affinity) while passing every bit-identity check for free.
/// `batches == 0` means the run used per-stamp dispatch.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StampDispatchStats {
    /// Batches closed — each one is exactly one `par_bands` dispatch.
    pub batches: u64,
    /// Bands the grid was split into.
    pub bands: usize,
    /// Most stamps any one batch carried.
    pub max_jobs_in_a_batch: usize,
    /// Most `(band, stamp)` partials any one batch held at once — the figure
    /// `MAX_PARTIALS_PER_BATCH` bounds.
    pub max_partials_in_a_batch: usize,
}

/// Fewest bands a grid must have before whole-toolpath dispatch is worth its
/// bookkeeping. Below this there is nothing to split and the per-stamp path —
/// which declines to dispatch small stamps at all — is the better answer.
const MIN_BANDS_FOR_WHOLE_PATH: usize = 4;

/// Hard cap on jobs per batch. Bounds the job/reduced arrays independently of
/// how few bands each stamp happens to touch.
const MAX_JOBS_PER_BATCH: usize = 8_192;

/// Hard cap on `(band, stamp)` partials per batch — the memory bound that
/// matters, and the reason a batch exists at all.
///
/// `DELTA_sim_w2.md` §3f sized the unchunked case at 64 bands × 70 k
/// subsegments × 96 B ≈ **430 MB**. Batching replaces "the whole toolpath" with
/// a fixed ceiling: at `size_of::<StampPartial>()` ≈ 80 B plus a 4 B job index,
/// 262 144 partials is **≈ 22 MB**, and the job side adds
/// 8 192 × (72 B job + 80 B reduced) ≈ **1.2 MB**. Total working set per batch
/// is therefore bounded at **≈ 23 MB regardless of toolpath length, grid size
/// or cutter diameter** — `partial_memory_bound_holds_at_the_documented_size`
/// checks the arithmetic against the real `size_of`.
const MAX_PARTIALS_PER_BATCH: usize = 262_144;

/// Fewest jobs a batch takes before the visit budget may close it. Without a
/// floor, a grid small enough that one stamp covers it would dispatch per
/// stamp — which is the thing this module exists to stop doing.
const MIN_JOBS_PER_BATCH: usize = 64;

/// How much stamping a batch absorbs before it is closed, in whole grid-passes
/// of estimated cell-visits.
///
/// **This is the mip-freshness dial, not a memory one.** `TileMaxTop` allows
/// itself one rebuild per grid-pass of stamped cell-visits
/// (`REFRESH_VISIT_MULTIPLIER = 1`), and a rebuild can only happen between
/// batches, so a batch of `N` grid-passes leaves the mip up to `N` passes
/// stale and the whole-stamp early-out correspondingly less effective. Set
/// against the plunge fixture, where the early-out is worth the most: the
/// retract half of every plunge-and-retract cycle is pure air over ground the
/// descent just cleared, and `DELTA_sim_w2.md` §1 measured that as more than
/// half the stamp budget on that workload.
const BATCH_VISIT_BUDGET_PASSES: u64 = 4;

/// One subsegment's geometry, deferred until its batch runs.
///
/// `sample_slot` is the index of the already-emitted `SimulationCutSample` this
/// stamp's metrics belong to. Nothing else about the sample is carried: phase 3
/// reads the rest off the sample itself, so there is no second copy of the
/// stream to drift.
#[derive(Clone, Copy, Debug)]
pub(super) struct StampJob {
    pub(super) start: (f64, f64, f64),
    pub(super) end: (f64, f64, f64),
    pub(super) mid_u: f64,
    pub(super) mid_v: f64,
    /// A `usize` rather than a `u32` on purpose: the batch caps bound the
    /// *queue*, not the sample stream, so a `u32` slot would be a silent
    /// wrap on a long enough toolpath and there is no correct fallback —
    /// stamping one job out of sequence would break per-cell mutation order.
    pub(super) sample_slot: usize,
}

impl StampJob {
    /// The driver-side degenerate test, verbatim from
    /// `estimate_and_stamp_cutting_subsegment`: a pure-vertical stamp's
    /// `degenerate`/`descent` are properties of the *segment*, not of any cell,
    /// so they must come from the driver rather than from a band that may never
    /// run.
    fn planar_len_sq(&self) -> f64 {
        let du = self.end.0 - self.start.0;
        let dv = self.end.1 - self.start.1;
        du * du + dv * dv
    }
}

/// The batching driver. One per simulation; reused across batches so the
/// bucket and partial arrays are allocated once.
pub(super) struct BandDispatch {
    jobs: Vec<StampJob>,
    /// Job indices per band, in job order.
    buckets: Vec<Vec<u32>>,
    /// Per-band partials, positionally aligned with `buckets`.
    outs: Vec<Vec<StampPartial>>,
    /// Per-job reduced partial.
    reduced: Vec<StampPartial>,
    /// Bands the arrays above are sized for.
    bands: usize,
    /// Union of the queued jobs' row spans — the only rows the batch can
    /// touch, and therefore the only bands worth dispatching. `None` when
    /// nothing in the queue reaches the grid.
    active_rows: Option<(usize, usize)>,
    pending_partials: usize,
    pending_visits: u64,
    visit_budget: u64,
    /// Diagnostics. No production reader; the non-vacuity sentries and the
    /// delta doc read them.
    stats: StampDispatchStats,
}

impl BandDispatch {
    /// `None` when the grid has too few bands to be worth splitting, in which
    /// case the per-stamp path (which declines to dispatch at all below its own
    /// bbox cutoff) is strictly better.
    pub(super) fn for_grid(grid: &DexelGrid, mode: StampDispatch) -> Option<Self> {
        let bands = grid.rows.div_ceil(BAND_ROWS);
        let wanted = match mode {
            StampDispatch::PerStamp => false,
            // Swept dispatch runs its own driver; this one must stay out of
            // the way rather than queue a second, duplicate set of stamps.
            StampDispatch::Swept | StampDispatch::SweptPlungeOnly => false,
            StampDispatch::WholeToolpath => true,
            // No thread-count condition, and that is a measurement rather
            // than an omission: at a ONE-thread pool whole-path dispatch was
            // 1.07×/1.08×/1.10× faster than per-stamp on the three wave-4
            // arms, in both A/B sessions. Per-stamp still pays a rayon bridge
            // per stamp above its own bbox cutoff, and pays a fresh band
            // iterator per stamp below it; batching amortises both whether or
            // not there is anyone to hand the work to.
            StampDispatch::Auto => bands >= MIN_BANDS_FOR_WHOLE_PATH,
        };
        if !wanted || bands == 0 || grid.cols == 0 {
            return None;
        }
        let cells = (grid.rows as u64).saturating_mul(grid.cols as u64).max(1);
        Some(Self {
            jobs: Vec::new(),
            buckets: vec![Vec::new(); bands],
            outs: vec![Vec::new(); bands],
            reduced: Vec::new(),
            bands,
            active_rows: None,
            pending_partials: 0,
            pending_visits: 0,
            visit_budget: cells.saturating_mul(BATCH_VISIT_BUDGET_PASSES),
            stats: StampDispatchStats {
                bands,
                ..StampDispatchStats::default()
            },
        })
    }

    /// Queue one subsegment, and account for what it will cost.
    ///
    /// Only the *counters* are computed here — the buckets themselves are
    /// built in [`Self::run_batch`], against the grid that is about to be
    /// stamped. Bucketing at push time would leave the band indices aliased to
    /// whatever grid shape happened to be current when the job was queued, and
    /// `zip` would then silently truncate rather than fail. It costs one extra
    /// pass over the queue per batch, against a stamp kernel that is orders of
    /// magnitude more expensive.
    pub(super) fn push(&mut self, grid: &DexelGrid, radius: f64, job: StampJob) {
        self.jobs.push(job);
        if let Some((row_lo, row_hi)) = band::stamp_row_span(grid, radius, job.start, job.end) {
            self.pending_partials += grid.band_span(row_lo, row_hi).1;
        }
        self.pending_visits = self
            .pending_visits
            .saturating_add(band::stamp_bbox_cells(grid, radius, job.start, job.end) as u64);
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
            let Some((row_lo, row_hi)) = band::stamp_row_span(grid, radius, job.start, job.end)
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

    /// Is the queued batch full enough to run?
    pub(super) fn batch_is_due(&self) -> bool {
        self.jobs.len() >= MAX_JOBS_PER_BATCH
            || self.pending_partials >= MAX_PARTIALS_PER_BATCH
            || (self.jobs.len() >= MIN_JOBS_PER_BATCH && self.pending_visits >= self.visit_budget)
    }

    pub(super) fn is_empty(&self) -> bool {
        self.jobs.is_empty()
    }

    pub(super) fn stats(&self) -> StampDispatchStats {
        self.stats
    }

    /// Run the queued batch: refresh the mip, replay every band in parallel,
    /// reduce in band order, and hand each job's four published metrics to
    /// `patch` **in job order**.
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
    /// What that bounds is not "once per 70 k subsegments". A batch closes at
    /// [`BATCH_VISIT_BUDGET_PASSES`] grid-passes of estimated stamped
    /// cell-visits, or [`MAX_JOBS_PER_BATCH`] stamps, or
    /// [`MAX_PARTIALS_PER_BATCH`] partials — whichever comes first — so the
    /// worst-case latency scales with the grid, not with the toolpath. On the
    /// shipped 0.4 mm wanaka grid that is tens of milliseconds; on a 4 M-cell
    /// grid, the largest this simulator is asked for, it is ~16 M cell-visits,
    /// order half a second. Small fixtures close on the job cap instead, which
    /// is tighter still.
    ///
    /// A `Sync` cancel token is the honest fix and would let the poll move
    /// inside the band loop; it is a crate-wide signature change and is not
    /// this wave's.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn run_batch(
        &mut self,
        grid: &mut DexelGrid,
        lut: &RadialProfileLUT,
        radius: f64,
        from_high: bool,
        capture_arc_engagement: bool,
        air_mip: &mut Option<TileMaxTop>,
        cancel: &dyn CancelCheck,
        mut patch: impl FnMut(usize, (f64, f64, Option<f64>, f64)),
    ) -> Result<(), Cancelled> {
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
        // Disjoint field borrows, spelled out because the closure needs
        // `jobs` immutably while `outs` is borrowed mutably.
        let Self {
            jobs,
            buckets,
            outs,
            reduced,
            active_rows,
            ..
        } = self;
        let active_rows = *active_rows;
        let mip = air_mip.as_ref();
        // Dispatch ONLY the bands the batch can reach, not every band in the
        // grid. This is wave 2 §3e's fan-out fix one level up, and it is
        // worth more here than it was there: a batch's stamps are consecutive
        // subsegments, so its active bands are a **contiguous** run — and
        // rayon splits an indexed iterator by index range, which on a
        // 40-band grid with a 16-band footprint left most of the pool holding
        // empty bands while a few workers did all the stamping. Measured
        // consequence before the fix: whole-path dispatch was a *loss* at two
        // threads on the arms whose footprint sits in one half of the grid.
        //
        // Numerically free, as it was in wave 2: a band outside the range
        // would have had an empty bucket and contributed nothing.
        // `active_rows` is `None` only when no queued job reaches the grid at
        // all, in which case every partial is `empty()` and the reduce below
        // is already correct.
        let dispatch_rows = active_rows.and_then(|(row_lo, row_hi)| {
            let (skip, take) = grid.band_span(row_lo, row_hi);
            // The slice is in range by construction — `build_buckets` sized
            // `buckets` from this same grid and `band_span` bounds `skip+take`
            // by that size. Widening to the whole grid rather than bailing on
            // the `None` keeps a broken invariant SLOW instead of WRONG: a
            // truncating `zip` would silently drop bands.
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
                    out.clear();
                    out.reserve(bucket.len());
                    for &j in bucket.iter() {
                        let Some(job) = jobs.get(j as usize) else {
                            continue;
                        };
                        out.push(stamp_segment_with_metrics(
                            &mut band, lut, radius, job.start, job.end, job.mid_u, job.mid_v,
                            from_high, mip,
                        ));
                    }
                });
        }

        // ── Phase 3a: reduce, in ascending band order per job ──
        //
        // Which is the SAME order the per-stamp path merges in: it walks
        // `band_span(row_lo, row_hi)` ascending, and a job's buckets were
        // filled from exactly that range. So the volume sums are reassociated
        // identically, not merely deterministically.
        reduced.clear();
        reduced.resize(jobs.len(), StampPartial::empty());
        for (bucket, out) in buckets.iter().zip(outs.iter()) {
            for (k, &j) in bucket.iter().enumerate() {
                let (Some(slot), Some(partial)) = (reduced.get_mut(j as usize), out.get(k)) else {
                    continue;
                };
                slot.merge(partial);
            }
        }

        // ── Phase 3b: driver-side fixes, mip bookkeeping, patch ──
        for (j, job) in jobs.iter().enumerate() {
            let Some(r) = reduced.get_mut(j) else {
                continue;
            };
            if job.planar_len_sq() < 1e-20 {
                r.degenerate = true;
                r.descent = (job.start.2 - job.end.2).abs();
            }
            if let Some(m) = air_mip.as_mut() {
                m.absorb(r);
            }
            patch(job.sample_slot, r.finish(radius, capture_arc_engagement));
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

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;

    /// The memory bound in [`MAX_PARTIALS_PER_BATCH`]'s docs is arithmetic over
    /// two `size_of`s, and a `size_of` is exactly the kind of number that
    /// changes under someone adding a field.
    ///
    /// Both the *inputs* and the *total* are pinned, deliberately. A test that
    /// only checked a generous ceiling would let the working set grow by 40 %
    /// while the delta doc's "≈ 23 MB, independent of toolpath length, grid
    /// size and cutter diameter" quietly stopped being true — which is the
    /// failure mode `CLAUDE.md` calls instrument integrity: a changed
    /// instrument makes its own docstring a lie you then cite.
    #[test]
    fn partial_memory_bound_holds_at_the_documented_size() {
        let partial = std::mem::size_of::<StampPartial>();
        let job = std::mem::size_of::<StampJob>();
        let index = std::mem::size_of::<u32>();
        assert!(
            partial <= 80,
            "`StampPartial` is {partial} B; the batch bound is documented at 80 B \
             per partial in `MAX_PARTIALS_PER_BATCH` and in `DELTA_sim_w4.md` §2c"
        );
        assert!(
            job <= 72,
            "`StampJob` is {job} B; the batch bound is documented at 72 B per job"
        );
        let bytes =
            MAX_PARTIALS_PER_BATCH * (partial + index) + MAX_JOBS_PER_BATCH * (job + partial);
        assert!(
            bytes <= 24 * 1024 * 1024,
            "a batch's working set is {} MB (partial {partial} B, job {job} B) — \
             the documented bound is ~23 MB and both it and the caps need \
             revisiting together",
            bytes / (1024 * 1024)
        );
    }

    /// `Auto` must not turn whole-path dispatch on for a grid that cannot feed
    /// the pool — the bookkeeping would be pure overhead.
    #[test]
    fn auto_declines_a_grid_with_too_few_bands() {
        use crate::geo::{BoundingBox3, P3};
        let tiny = DexelGrid::z_grid_from_bounds(
            &BoundingBox3 {
                min: P3::new(0.0, 0.0, 0.0),
                max: P3::new(4.0, 2.0, 4.0),
            },
            1.0,
        );
        assert!(tiny.rows.div_ceil(BAND_ROWS) < MIN_BANDS_FOR_WHOLE_PATH);
        assert!(BandDispatch::for_grid(&tiny, StampDispatch::Auto).is_none());
        // …but an explicit request is still honoured, so the A/B harness can
        // force the shape it wants to measure.
        assert!(BandDispatch::for_grid(&tiny, StampDispatch::WholeToolpath).is_some());
        assert!(BandDispatch::for_grid(&tiny, StampDispatch::PerStamp).is_none());
    }
}
