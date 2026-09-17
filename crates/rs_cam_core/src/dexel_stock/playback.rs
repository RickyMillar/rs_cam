//! Row-band dispatch for the **non-metric playback replay** — SIM w6, the
//! piece `DELTA_sim_w4.md` §6 left on the table.
//!
//! STK-09: the driver itself is `band_batch.rs`, shared with `whole_path.rs`.
//! This file holds what is playback-specific: the [`PlaybackDispatch`] dial,
//! the caps, the [`PlaybackJob`] shape and the mip-only serial tail.
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

use super::band::BAND_ROWS;
use super::band_batch::{BandBatch, BandJob, BandPartial, BatchCaps};
use super::stamping::{CoverageFastPath, PlaybackPartial, stamp_segment_on_band};
use super::tile_mip::TileMaxTop;
use crate::interrupt::{CancelCheck, Cancelled};
use crate::stock::dexel::DexelGrid;
use crate::stock::radial_profile::RadialProfileLUT;

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

/// The playback route's batch caps.
///
/// `max_partials` is the memory bound. [`PlaybackPartial`] is 16 B against
/// [`super::stamping::StampPartial`]'s 80 B, so the same partial count costs a
/// fifth of what the metric batch does: `262 144 × (16 + 4) ≈ 5.2 MB`, plus
/// `8 192 × (48 B job + 16 B reduced) ≈ 0.5 MB`. Bounded at **≈ 6 MB
/// regardless of toolpath length, grid size or cutter diameter**, and
/// `playback_batch_memory_bound_holds_at_the_documented_size` checks the
/// arithmetic against the real `size_of`s rather than against this comment.
///
/// `min_jobs` is **16, not `whole_path.rs`'s 64, and the difference is a unit
/// change rather than a tuning preference.** A metric job is one *subsegment* —
/// a `sample_step_mm` slice of a move, and a plunge is cut into 250 of them by
/// the `by_z` rule. A playback job is a whole *move*. On the shipped raster
/// fixture one playback move is ~6.6 k cell-visits against a metric
/// subsegment's few hundred, so a 64-job floor would hold a batch open for ~20
/// grid-passes of stamping and leave the S2 mip five times staler than
/// `visit_budget_passes` says it may be — the budget would never get to close a
/// batch at all, on the workload where the whole-stamp early-out is worth the
/// most.
///
/// `visit_budget_passes` is the mip-freshness dial, and the argument transfers
/// from `whole_path.rs` unchanged because `TileMaxTop` charges in *cell-visits*
/// rather than in wall-clock. Staleness is a **speed** cost and never a
/// correctness one: `conservative_top` is monotone decreasing, so an older mip
/// is still an upper bound and fires the early-out less often. And in this
/// kernel a stamp that is not skipped but whose cells are all inert writes
/// nothing at all — there are no volume accumulators to keep in step, so the
/// "identical addend in identical order" argument `whole_path.rs` needs does
/// not even arise.
const CAPS: BatchCaps = BatchCaps {
    max_jobs: 8_192,
    max_partials: 262_144,
    min_jobs: 16,
    visit_budget_passes: 4,
};

/// One playback stamp's geometry, deferred until its batch runs.
///
/// Already decomposed into the grid's `(u, v, depth)` axes by the driver, so
/// the parallel phase never touches a `StockCutDirection`.
#[derive(Clone, Copy, Debug)]
pub(super) struct PlaybackJob {
    pub(super) start: (f64, f64, f64),
    pub(super) end: (f64, f64, f64),
}

impl BandJob for PlaybackJob {
    fn start(&self) -> (f64, f64, f64) {
        self.start
    }
    fn end(&self) -> (f64, f64, f64) {
        self.end
    }
}

impl BandPartial for PlaybackPartial {
    fn empty() -> Self {
        PlaybackPartial::empty()
    }
    fn merge(&mut self, other: &Self) {
        PlaybackPartial::merge(self, other);
    }
}

/// The playback route's batching driver. The machinery is `band_batch.rs`;
/// this file supplies the caps, the job shape and the mip-only serial tail.
pub(super) type PlaybackBandDispatch = BandBatch<PlaybackJob, PlaybackPartial>;

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
        Some(BandBatch::new(grid, CAPS))
    }

    /// The route's own view of what the driver did.
    pub(super) fn stats(&self) -> PlaybackDispatchStats {
        let s = self.batch_stats();
        PlaybackDispatchStats {
            batches: s.batches,
            bands: s.bands,
            max_jobs_in_a_batch: s.max_jobs_in_a_batch,
            max_partials_in_a_batch: s.max_partials_in_a_batch,
            band_tasks_run: s.band_tasks_run,
        }
    }

    /// Run the queued batch, then fold each job's mip bookkeeping back in **in
    /// job order**. Nothing that reaches a result passes through the tail: both
    /// `PlaybackPartial` channels are order-independent, so it exists for the
    /// refresh cadence and the diagnostics only.
    pub(super) fn run_batch(
        &mut self,
        grid: &mut DexelGrid,
        lut: &RadialProfileLUT,
        radius: f64,
        from_high: bool,
        air_mip: &mut Option<TileMaxTop>,
        cancel: &dyn CancelCheck,
    ) -> Result<(), Cancelled> {
        // Computed ONCE per batch and shared by every band. Its two inputs are
        // constant across a replay, and it is several `sqrt`s plus bounded ULP
        // walks — recomputing it per (band, stamp) is what made the first w6
        // A/B read 0.42× at one thread on a coarse-cell fixture. Bit-identical:
        // same pure function, same two arguments.
        let fast = CoverageFastPath::new(lut.radius_sq(), grid.cell_size);
        self.run(
            grid,
            radius,
            air_mip,
            cancel,
            |band, job, mip| {
                stamp_segment_on_band(band, lut, radius, job.start, job.end, from_high, mip, fast)
            },
            |_job, r, air_mip| {
                if let Some(m) = air_mip.as_mut() {
                    m.absorb_playback(r);
                }
            },
        )
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

    /// The memory bound in [`CAPS`]'s docs is arithmetic over
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
        let bytes = CAPS.max_partials * (partial + index) + CAPS.max_jobs * (job + partial);
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
