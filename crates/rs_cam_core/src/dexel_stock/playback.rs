//! Row-band dispatch for the **non-metric playback replay** — SIM w6, the
//! piece `DELTA_sim_w4.md` §6 left on the table.
//!
//! Wave 4 banded the *metric* stamp kernel and closed by naming this: the
//! playback replay (`simulate_toolpath_with_lut_cancel` →
//! `stamping::stamp_segment_on_grid`) was still 100 % serial, and
//! `compute/simulate.rs` runs it against `global_stock` for **every** toolpath
//! in the project — i.e. it is a second full replay of the whole job, on top of
//! the metric one.
//!
//! # The same three phases, and one of them disappears
//!
//! 1. **Enumerate (serial).** The move loop in `simulation.rs`, unchanged: same
//!    arc linearisation, same per-move cancel poll. It records a
//!    [`PlaybackJob`] instead of stamping immediately.
//! 2. **Replay (parallel).** One `par_bands` per **batch**, over the batch's own
//!    union row span, zipped with per-band job buckets. Each band replays its
//!    own jobs in job order through [`stamping::stamp_segment_on_band`].
//! 3. **Reduce (serial).** There is nothing to reduce into a result. The only
//!    thing that comes back out of the bands is the S2 mip's own bookkeeping —
//!    cells visited, stamp skipped — which decides refresh cadence and
//!    diagnostics and reaches no published number.
//!
//! # Why the bit-identity claim is stronger here than in `whole_path.rs`
//!
//! `whole_path.rs` has to argue that its reduce merges a job's bands in the
//! same ascending order the per-stamp path did, because `pre_volume` and
//! `post_volume` are sums that banding would otherwise reassociate. This kernel
//! has **no accumulators at all**. It writes to cells; cells are partitioned by
//! the bands; a band replays its jobs in the original sequence. So there is no
//! reassociation available to get wrong, and the identity is structural rather
//! than order-dependent.
//!
//! That matters beyond tidiness: `global_stock` is what
//! `StockSource::FromRemainingStock` toolpath generation reads, so a single
//! last-bit difference in this grid changes generated G-code downstream. The
//! contract for this wave was bit-identity with no reassociation latitude, and
//! `playback_band_dispatch_s6` is where it is checked.
//!
//! # What is deliberately NOT changed
//!
//! Every move the serial path stamps, this path stamps — including
//! `MoveIntent::Retract` linear feeds, which the playback loop has always
//! treated as ordinary `MoveType::Linear` cuts. Skipping them
//! (`DELTA_sim_w5b_landing.md` §9, W5B-F5) would change `global_stock` and
//! therefore change generated geometry. That is a product decision, not a
//! scheduling one, and it is out of this wave's scope.

use std::sync::atomic::{AtomicU64, Ordering};

use rayon::prelude::*;

use super::band::{self, BAND_ROWS};
use super::stamping::{CoverageFastPath, PlaybackPartial, stamp_segment_on_band};
use super::tile_mip::TileMaxTop;
use crate::dexel::DexelGrid;
use crate::interrupt::{CancelCheck, Cancelled, check_cancel};
use crate::radial_profile::RadialProfileLUT;

/// Which dispatch shape the **playback** (non-metric) replay uses.
///
/// **Both shapes are purely a schedule.** Unlike [`super::StampDispatch`],
/// whose `Swept` arm changes what the simulator measures, every value here
/// produces a bit-identical grid — see the module docs for why that is
/// structural. So this dial is safe to flip from a bench harness, a sentry or
/// the environment, and a "which one is faster" answer can never also be a
/// "which one is right" answer.
///
/// **The default is not `#[derive(Default)]`.** It consults
/// `RS_CAM_PLAYBACK_DISPATCH` once per process — `serial`, `banded` or `auto` —
/// so a whole test binary, a bench or the CLI can be pinned to one shape
/// without threading a flag through every construction site. That override is
/// the A/B instrument, and the serial arm stays as the reference
/// implementation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaybackDispatch {
    /// Pick per replay. Resolves to [`PlaybackDispatch::Banded`] — see
    /// [`PlaybackDispatch::resolved`], the single place that mapping lives —
    /// subject to the grid having at least [`MIN_BANDS_FOR_BANDED`] bands.
    Auto,
    /// The pre-w6 shape: one `stamp_segment_on_grid` per move, on the calling
    /// thread. Kept as the A/B partner and as the reference the bit-identity
    /// sentries compare against.
    Serial,
    /// One `par_bands` call per batch of queued stamps.
    Banded,
}

/// Process-wide playback dispatch override, parsed once. `None` means "not set".
fn dispatch_override() -> Option<PlaybackDispatch> {
    static OVERRIDE: std::sync::OnceLock<Option<PlaybackDispatch>> = std::sync::OnceLock::new();
    *OVERRIDE.get_or_init(|| {
        let raw = std::env::var("RS_CAM_PLAYBACK_DISPATCH").ok()?;
        match raw.trim().to_ascii_lowercase().as_str() {
            "auto" => Some(PlaybackDispatch::Auto),
            "serial" | "per_stamp" | "per-stamp" => Some(PlaybackDispatch::Serial),
            "banded" | "band" | "whole" | "whole_path" => Some(PlaybackDispatch::Banded),
            _ => None,
        }
    })
}

impl Default for PlaybackDispatch {
    fn default() -> Self {
        dispatch_override().unwrap_or(Self::Auto)
    }
}

impl PlaybackDispatch {
    /// Turn `Auto` into the concrete shape it selects. **The only place that
    /// mapping exists**, so no second site can disagree about what the default
    /// means.
    #[must_use]
    pub fn resolved(self) -> Self {
        match self {
            Self::Auto => Self::Banded,
            other => other,
        }
    }
}

/// What the playback dispatcher actually did on the last non-metric replay.
///
/// **No production reader.** It exists so the w6 sentries can prove the
/// dispatch is not vacuous — a run that resolves to one band, or to one batch,
/// or that never enters the parallel region at all, passes every bit-identity
/// check for free while exercising nothing. `DELTA_sim_w2.md` §2e and
/// `tile_mip`'s own `REFRESH_VISIT_MULTIPLIER` note both record that exact
/// failure mode on this codebase, so the counters exist and are asserted.
///
/// `batches == 0` means the replay ran serially.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlaybackDispatchStats {
    /// Batches closed — each one is exactly one `par_bands` dispatch.
    pub batches: u64,
    /// Bands the grid was split into.
    pub bands: usize,
    /// Most stamps any one batch carried.
    pub max_jobs_in_a_batch: usize,
    /// Most `(band, stamp)` partials any one batch held at once.
    pub max_partials_in_a_batch: usize,
    /// **Band closure bodies actually executed**, counted from inside the
    /// parallel region.
    ///
    /// The other four counters are all recorded on the serial side and would
    /// keep counting if the `par_bands` call itself were reduced to nothing —
    /// an empty dispatch range, a truncating `zip`, a slice that came back
    /// `None`. This one cannot: it is incremented by the closure that does the
    /// stamping. A stale gate that fires zero times is silently sound, which is
    /// this programme's most-repeated trap.
    pub band_tasks_run: u64,
}

/// Fewest bands a grid must have before banded playback is worth its
/// bookkeeping.
const MIN_BANDS_FOR_BANDED: usize = 4;

/// Hard cap on jobs per batch.
const MAX_JOBS_PER_BATCH: usize = 8_192;

/// Hard cap on `(band, stamp)` partials per batch — the memory bound.
///
/// [`PlaybackPartial`] is 16 B against [`super::stamping::StampPartial`]'s
/// 80 B, so the same partial count costs a fifth of what the metric batch does:
/// `262 144 × (16 + 4) ≈ 5.2 MB`, plus `8 192 × (48 B job + 16 B reduced)
/// ≈ 0.5 MB`. Bounded at **≈ 6 MB regardless of toolpath length, grid size or
/// cutter diameter**, and `playback_batch_memory_bound_holds_at_the_documented_size`
/// checks the arithmetic against the real `size_of`s rather than against this
/// comment.
const MAX_PARTIALS_PER_BATCH: usize = 262_144;

/// Fewest jobs a batch takes before the visit budget may close it. Without a
/// floor, a grid small enough that one stamp covers it would dispatch per
/// stamp — the thing this module exists to stop doing.
///
/// **16, not `whole_path.rs`'s 64, and the difference is a unit change rather
/// than a tuning preference.** A metric job is one *subsegment* — a
/// `sample_step_mm` slice of a move, and a plunge is cut into 250 of them by
/// the `by_z` rule. A playback job is a whole *move*. On the shipped raster
/// fixture one playback move is ~6.6 k cell-visits against a metric
/// subsegment's few hundred, so a 64-job floor would hold a batch open for
/// ~20 grid-passes of stamping and leave the S2 mip five times staler than
/// [`BATCH_VISIT_BUDGET_PASSES`] says it may be — the budget would never get to
/// close a batch at all, on the workload where the whole-stamp early-out is
/// worth the most.
const MIN_JOBS_PER_BATCH: usize = 16;

/// How much stamping a batch absorbs before it is closed, in whole grid-passes
/// of estimated cell-visits.
///
/// **The mip-freshness dial, not a memory one** — the same argument as
/// `whole_path.rs`, and it transfers unchanged because `TileMaxTop` charges in
/// *cell-visits* rather than in wall-clock. A rebuild reads `conservative_top`
/// across the whole grid, which is a data race against the bands' own writes,
/// so it can only happen at a batch boundary; a batch of `N` grid-passes leaves
/// the mip up to `N` passes stale and the whole-stamp early-out correspondingly
/// less effective.
///
/// Staleness is a **speed** cost and never a correctness one:
/// `conservative_top` is monotone decreasing, so an older mip is still an upper
/// bound and fires the early-out less often. And in this kernel a stamp that is
/// not skipped but whose cells are all inert writes nothing at all — there are
/// no volume accumulators to keep in step, so the "identical addend in
/// identical order" argument `whole_path.rs` needs does not even arise.
const BATCH_VISIT_BUDGET_PASSES: u64 = 4;

/// One playback stamp's geometry, deferred until its batch runs.
///
/// Already decomposed into the grid's `(u, v, depth)` axes by the driver, so
/// the parallel phase never touches a `StockCutDirection`.
#[derive(Clone, Copy, Debug)]
pub(super) struct PlaybackJob {
    pub(super) start: (f64, f64, f64),
    pub(super) end: (f64, f64, f64),
}

/// The batching driver. One per replay; reused across batches so the bucket and
/// partial arrays are allocated once.
pub(super) struct PlaybackBandDispatch {
    jobs: Vec<PlaybackJob>,
    /// Job indices per band, in job order.
    buckets: Vec<Vec<u32>>,
    /// Per-band partials, positionally aligned with `buckets`.
    outs: Vec<Vec<PlaybackPartial>>,
    /// Per-job reduced partial.
    reduced: Vec<PlaybackPartial>,
    /// Bands the arrays above are sized for.
    bands: usize,
    /// Union of the queued jobs' row spans — the only rows the batch can touch,
    /// and therefore the only bands worth dispatching.
    active_rows: Option<(usize, usize)>,
    pending_partials: usize,
    pending_visits: u64,
    visit_budget: u64,
    /// Incremented from inside the parallel region. See
    /// [`PlaybackDispatchStats::band_tasks_run`].
    band_tasks_run: AtomicU64,
    stats: PlaybackDispatchStats,
}

impl PlaybackBandDispatch {
    /// `None` when the replay should run serially — either because it was asked
    /// to, or because the grid has too few bands to be worth splitting.
    pub(super) fn for_grid(grid: &DexelGrid, mode: PlaybackDispatch) -> Option<Self> {
        let mode = mode.resolved();
        let bands = grid.rows.div_ceil(BAND_ROWS);
        let wanted = match mode {
            PlaybackDispatch::Serial => false,
            PlaybackDispatch::Banded => true,
            // Unreachable while `resolved()` maps `Auto` to `Banded`; kept so a
            // future change to that mapping lands here rather than silently.
            PlaybackDispatch::Auto => bands >= MIN_BANDS_FOR_BANDED,
        };
        // The band-count floor applies to an explicit `Banded` request too: a
        // one-band grid has nothing to split, and the queueing would be pure
        // overhead over a path that is already bit-identical.
        if !wanted || bands < MIN_BANDS_FOR_BANDED || grid.cols == 0 {
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
            band_tasks_run: AtomicU64::new(0),
            stats: PlaybackDispatchStats {
                bands,
                ..PlaybackDispatchStats::default()
            },
        })
    }

    /// Queue one stamp, and account for what it will cost.
    ///
    /// Only the *counters* are computed here; the buckets are built in
    /// [`Self::run_batch`] against the grid that is about to be stamped, for the
    /// reason `whole_path.rs` states — bucketing at push time aliases the band
    /// indices to whatever grid shape was current when the job was queued, and
    /// `zip` would then truncate rather than fail.
    pub(super) fn push(&mut self, grid: &DexelGrid, radius: f64, job: PlaybackJob) {
        self.jobs.push(job);
        if let Some((row_lo, row_hi)) = band::stamp_row_span(grid, radius, job.start, job.end) {
            self.pending_partials += grid.band_span(row_lo, row_hi).1;
        }
        self.pending_visits = self
            .pending_visits
            .saturating_add(band::stamp_bbox_cells(grid, radius, job.start, job.end) as u64);
    }

    /// Fill `buckets` from the queued jobs against `grid`'s current shape. Job
    /// indices land in ascending order inside each bucket, which is what makes
    /// a band replay its share of the batch in the original stamp sequence.
    fn build_buckets(&mut self, grid: &DexelGrid, radius: f64) {
        let bands = grid.rows.div_ceil(BAND_ROWS);
        if bands != self.bands {
            self.buckets = vec![Vec::new(); bands];
            self.outs = vec![Vec::new(); bands];
            self.bands = bands;
            self.stats.bands = bands;
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

    pub(super) fn stats(&self) -> PlaybackDispatchStats {
        PlaybackDispatchStats {
            band_tasks_run: self.band_tasks_run.load(Ordering::Relaxed),
            ..self.stats
        }
    }

    /// Run the queued batch: refresh the mip, replay every band in parallel,
    /// then fold each job's mip bookkeeping back in **in job order**.
    ///
    /// # Cancellation granularity, stated
    ///
    /// The token is polled **once per batch**, serially, at the top of this
    /// function; the enumerating loop in `simulation.rs` still polls it once per
    /// move and once per linearised arc window, unchanged. It is deliberately
    /// NOT polled inside the parallel phase: `CancelCheck` carries no `Sync`
    /// bound, so reaching it from a rayon worker would mean widening a signature
    /// the whole crate depends on — the same tail item `DELTA_sim_w4.md` §2f
    /// recorded, and this wave does not take it either.
    ///
    /// What that bounds is the *grid*, not the toolpath: a batch closes at
    /// [`BATCH_VISIT_BUDGET_PASSES`] grid-passes of estimated stamped
    /// cell-visits, [`MAX_JOBS_PER_BATCH`] stamps, or
    /// [`MAX_PARTIALS_PER_BATCH`] partials, whichever comes first. The playback
    /// stamp is roughly a 25th of the metric stamp's per-cell cost
    /// (`perf_suite`'s own measurement), so the same budget in cell-visits is a
    /// **shorter** wall-clock latency here than the one wave 4 accepted.
    pub(super) fn run_batch(
        &mut self,
        grid: &mut DexelGrid,
        lut: &RadialProfileLUT,
        radius: f64,
        from_high: bool,
        air_mip: &mut Option<TileMaxTop>,
        cancel: &dyn CancelCheck,
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
        // Computed ONCE per batch and shared by every band. Its two inputs are
        // constant across a replay, and it is several `sqrt`s plus bounded ULP
        // walks — recomputing it per (band, stamp) is what made the first w6
        // A/B read 0.42× at one thread on a coarse-cell fixture. Bit-identical:
        // same pure function, same two arguments.
        let fast = CoverageFastPath::new(lut.radius_sq(), grid.cell_size);
        // Dispatch ONLY the bands the batch can reach. This is the fan-out
        // defect wave 2 §3e and wave 4 §3d each found once — rayon splits an
        // indexed parallel iterator by index RANGE, and a batch's stamps are
        // consecutive moves, so its active bands are a contiguous run. Handing
        // `par_bands` the whole grid gives one worker every active band and the
        // others a pile of empties, and it presents as "parallelism does not
        // help here" rather than as a bug. Numerically free: an excluded band
        // has an empty bucket and would have written nothing.
        let dispatch_rows = active_rows.and_then(|(row_lo, row_hi)| {
            let (skip, take) = grid.band_span(row_lo, row_hi);
            // In range by construction — `build_buckets` sized `buckets` from
            // this same grid. Widening to the whole grid rather than bailing
            // keeps a broken invariant SLOW instead of WRONG: a truncating
            // `zip` would silently drop bands, and a dropped band is a stamp
            // that never happened.
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
                        out.push(stamp_segment_on_band(
                            &mut band, lut, radius, job.start, job.end, from_high, mip, fast,
                        ));
                    }
                });
        }

        // ── Phase 3: fold the mip bookkeeping back, in job order ──
        //
        // Nothing that reaches a result passes through here. Both channels are
        // order-independent (a sum of disjoint counts and a boolean `and`), so
        // this loop exists for the refresh cadence and the diagnostics only.
        reduced.clear();
        reduced.resize(jobs.len(), PlaybackPartial::empty());
        for (bucket, out) in buckets.iter().zip(outs.iter()) {
            for (k, &j) in bucket.iter().enumerate() {
                let (Some(slot), Some(partial)) = (reduced.get_mut(j as usize), out.get(k)) else {
                    continue;
                };
                slot.merge(partial);
            }
        }
        if let Some(m) = air_mip.as_mut() {
            for r in reduced.iter() {
                m.absorb_playback(r);
            }
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
    /// changes under someone adding a field. Both the inputs and the total are
    /// pinned — a test that only checked a generous ceiling would let the
    /// working set grow while the doc's "≈ 6 MB, independent of toolpath length,
    /// grid size and cutter diameter" quietly stopped being true.
    #[test]
    fn playback_batch_memory_bound_holds_at_the_documented_size() {
        let partial = std::mem::size_of::<PlaybackPartial>();
        let job = std::mem::size_of::<PlaybackJob>();
        let index = std::mem::size_of::<u32>();
        assert!(
            partial <= 16,
            "`PlaybackPartial` is {partial} B; the batch bound is documented at \
             16 B per partial — and the whole point of a playback-specific \
             partial is that it is a fifth of `StampPartial`"
        );
        assert!(
            job <= 48,
            "`PlaybackJob` is {job} B; the batch bound is documented at 48 B"
        );
        let bytes =
            MAX_PARTIALS_PER_BATCH * (partial + index) + MAX_JOBS_PER_BATCH * (job + partial);
        assert!(
            bytes <= 7 * 1024 * 1024,
            "a batch's working set is {} MB (partial {partial} B, job {job} B) — \
             the documented bound is ~6 MB and both it and the caps need \
             revisiting together",
            bytes / (1024 * 1024)
        );
    }

    /// The band-count floor, and that `Serial` really means serial.
    #[test]
    fn a_grid_with_too_few_bands_declines_banded_playback() {
        use crate::geo::{BoundingBox3, P3};
        let tiny = DexelGrid::z_grid_from_bounds(
            &BoundingBox3 {
                min: P3::new(0.0, 0.0, 0.0),
                max: P3::new(4.0, 2.0, 4.0),
            },
            1.0,
        );
        assert!(tiny.rows.div_ceil(BAND_ROWS) < MIN_BANDS_FOR_BANDED);
        assert!(PlaybackBandDispatch::for_grid(&tiny, PlaybackDispatch::Auto).is_none());
        // …and an explicit request does NOT override the floor here, unlike
        // `StampDispatch::WholeToolpath`. There is nothing to split on a
        // one-band grid, and both paths are bit-identical anyway, so the A/B
        // harness loses nothing by being handed the serial path.
        assert!(PlaybackBandDispatch::for_grid(&tiny, PlaybackDispatch::Banded).is_none());
        assert!(PlaybackBandDispatch::for_grid(&tiny, PlaybackDispatch::Serial).is_none());

        let big = DexelGrid::z_grid_from_bounds(
            &BoundingBox3 {
                min: P3::new(0.0, 0.0, 0.0),
                max: P3::new(40.0, 30.0, 12.0),
            },
            0.5,
        );
        assert!(PlaybackBandDispatch::for_grid(&big, PlaybackDispatch::Auto).is_some());
        assert!(PlaybackBandDispatch::for_grid(&big, PlaybackDispatch::Banded).is_some());
        assert!(PlaybackBandDispatch::for_grid(&big, PlaybackDispatch::Serial).is_none());
    }

    /// `Auto` resolves in exactly one place, and this is the assertion that a
    /// second place cannot appear without a red test.
    #[test]
    fn auto_resolves_to_banded() {
        assert_eq!(PlaybackDispatch::Auto.resolved(), PlaybackDispatch::Banded);
        assert_eq!(
            PlaybackDispatch::Serial.resolved(),
            PlaybackDispatch::Serial
        );
        assert_eq!(
            PlaybackDispatch::Banded.resolved(),
            PlaybackDispatch::Banded
        );
    }
}
