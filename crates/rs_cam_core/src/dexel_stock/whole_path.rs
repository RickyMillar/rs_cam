//! Whole-toolpath row-band dispatch — `DELTA_sim_w2.md` §3f, the remaining
//! lever of `PERF_REVIEW.md` **S3**.
//!
//! STK-09: the driver itself is `band_batch.rs`, shared with `playback.rs`.
//! This file holds what is metric-specific: the [`StampDispatch`] dial, the
//! caps, the [`StampJob`] shape and the serial tail that patches each sample.
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

use super::band::BAND_ROWS;
use super::band_batch::{BandBatch, BandJob, BandPartial, BatchCaps};
use super::stamping::{StampPartial, stamp_segment_with_metrics};
use super::tile_mip::TileMaxTop;
use crate::interrupt::{CancelCheck, Cancelled};
use crate::stock::dexel::DexelGrid;
use crate::stock::radial_profile::RadialProfileLUT;
use crate::tool::MillingCutter;

/// Which dispatch shape the metric simulator uses for its stamp kernel.
///
/// **Three of the five shapes are purely a schedule; one is not.**
/// [`StampDispatch::PerStamp`], [`StampDispatch::WholeToolpath`] and
/// [`StampDispatch::SweptPlungeOnly`] run the same kernel over the same band
/// decomposition and merge the same partials in the same order, so the grid,
/// the sample stream and all four published metrics are bit-identical across
/// them — which is what `whole_path_dispatch_matches_per_stamp_bit_for_bit`
/// and `pure_vertical_chunks_are_bit_identical_to_per_stamp` pin.
/// [`StampDispatch::Swept`] changes what the simulator *measures*, on purpose,
/// and **is what [`StampDispatch::Auto`] now resolves to** (SIM w5b, landed
/// 2026-08-21; `planning/perf_review_2026-08-19/DELTA_sim_w5b_landing.md`).
///
/// **The default is not `#[derive(Default)]`.** It consults
/// `RS_CAM_STAMP_DISPATCH` once per process so a whole test binary, a bench, or
/// the CLI can be forced onto one shape without threading a flag through every
/// construction site. That override is the A/B instrument the whole S1 decision
/// rests on and it stays. Unset (the shipped case) is [`StampDispatch::Auto`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StampDispatch {
    /// Pick per simulation. **Resolves to [`StampDispatch::Swept`]** — see
    /// [`StampDispatch::resolved`], which is the single place that mapping
    /// lives.
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
    /// **Not bit-identical to the other three, by design** — it is the only
    /// value of this enum that changes what the simulator measures. It is
    /// what `Auto` resolves to as of SIM w5b; the other three remain
    /// selectable because the A/B, the bit-identity sentries and the
    /// determinism sentries are written against them. See `swept.rs`'s module
    /// docs for what moves and why, and `DELTA_sim_w5_s1_DECISION.md` §2 for
    /// the field-by-field classification of what moved when it was made the
    /// default.
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

impl StampDispatch {
    /// Turn `Auto` into the concrete shape it selects. **The only place that
    /// mapping exists** — every consumer resolves first, so there is no second
    /// site that can disagree about what the default means.
    ///
    /// `Auto` → [`StampDispatch::Swept`] since SIM w5b. Every other value is
    /// returned unchanged, which is what keeps `RS_CAM_STAMP_DISPATCH=whole_path`
    /// a usable A/B arm against the shipped default.
    #[must_use]
    pub fn resolved(self) -> Self {
        match self {
            Self::Auto => Self::Swept,
            other => other,
        }
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
    /// `CAPS.max_partials` bounds.
    pub max_partials_in_a_batch: usize,
}

/// Fewest bands a grid must have before whole-toolpath dispatch is worth its
/// bookkeeping. Below this there is nothing to split and the per-stamp path —
/// which declines to dispatch small stamps at all — is the better answer.
const MIN_BANDS_FOR_WHOLE_PATH: usize = 4;

/// The metric route's batch caps.
///
/// `max_partials` is the memory bound that matters, and the reason a batch
/// exists at all. `DELTA_sim_w2.md` §3f sized the unchunked case at 64 bands ×
/// 70 k subsegments × 96 B ≈ **430 MB**. Batching replaces "the whole toolpath"
/// with a fixed ceiling: at `size_of::<StampPartial>()` ≈ 80 B plus a 4 B job
/// index, 262 144 partials is **≈ 22 MB**, and the job side adds
/// 8 192 × (72 B job + 80 B reduced) ≈ **1.2 MB**. Total working set per batch
/// is therefore bounded at **≈ 23 MB regardless of toolpath length, grid size
/// or cutter diameter** — `partial_memory_bound_holds_at_the_documented_size`
/// checks the arithmetic against the real `size_of`.
///
/// `min_jobs` is 64 here and 16 in `playback.rs`, and the difference is a UNIT
/// change rather than a tuning preference: a metric job is one subsegment, a
/// playback job is a whole move.
///
/// `visit_budget_passes` is the mip-freshness dial, not a memory one.
/// `TileMaxTop` allows itself one rebuild per grid-pass of stamped cell-visits,
/// and a rebuild can only happen between batches, so a batch of `N` grid-passes
/// leaves the mip up to `N` passes stale. Set against the plunge fixture, where
/// the early-out is worth the most: the retract half of every
/// plunge-and-retract cycle is pure air over ground the descent just cleared,
/// and `DELTA_sim_w2.md` §1 measured that as more than half the stamp budget on
/// that workload.
const CAPS: BatchCaps = BatchCaps {
    max_jobs: 8_192,
    max_partials: 262_144,
    min_jobs: 64,
    visit_budget_passes: 4,
};

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

impl BandJob for StampJob {
    fn start(&self) -> (f64, f64, f64) {
        self.start
    }
    fn end(&self) -> (f64, f64, f64) {
        self.end
    }
}

impl BandPartial for StampPartial {
    fn empty() -> Self {
        StampPartial::empty()
    }
    fn merge(&mut self, other: &Self) {
        StampPartial::merge(self, other);
    }
}

/// The metric route's batching driver. The machinery is `band_batch.rs`; this
/// file supplies the caps, the job shape and the serial tail.
pub(super) type BandDispatch = BandBatch<StampJob, StampPartial>;

impl BandDispatch {
    /// `None` when the grid has too few bands to be worth splitting, in which
    /// case the per-stamp path (which declines to dispatch at all below its own
    /// bbox cutoff) is strictly better.
    pub(super) fn for_grid(grid: &DexelGrid, mode: StampDispatch) -> Option<Self> {
        // Resolve first so `Auto` cannot mean one thing here and another in
        // `SweptDispatch::for_grid`. Since w5b `Auto` resolves to `Swept`, so
        // the `Auto` arm below is reachable only if that mapping changes back.
        let mode = mode.resolved();
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
        Some(BandBatch::new(grid, CAPS))
    }

    /// The route's own view of what the driver did.
    pub(super) fn stats(&self) -> StampDispatchStats {
        let s = self.batch_stats();
        StampDispatchStats {
            batches: s.batches,
            bands: s.bands,
            max_jobs_in_a_batch: s.max_jobs_in_a_batch,
            max_partials_in_a_batch: s.max_partials_in_a_batch,
        }
    }

    /// Run the queued batch and hand each job's four published metrics to
    /// `patch`, **in job order**.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn run_batch(
        &mut self,
        grid: &mut DexelGrid,
        lut: &RadialProfileLUT,
        // The stamp bbox and the engagement denominator are two different
        // questions; `radius` below still answers the first (envelope), and
        // this answers the second at each partial's own axial DOC. See
        // `StampPartial::finish` (U3 / Phase M3).
        cutter: &dyn MillingCutter,
        radius: f64,
        from_high: bool,
        capture_arc_engagement: bool,
        air_mip: &mut Option<TileMaxTop>,
        cancel: &dyn CancelCheck,
        mut patch: impl FnMut(usize, (f64, f64, Option<f64>, f64)),
    ) -> Result<(), Cancelled> {
        self.run(
            grid,
            radius,
            air_mip,
            cancel,
            |band, job, mip| {
                stamp_segment_with_metrics(
                    band, lut, radius, job.start, job.end, job.mid_u, job.mid_v, from_high, mip,
                )
            },
            |job, r, air_mip| {
                if job.planar_len_sq() < 1e-20 {
                    r.degenerate = true;
                    r.descent = (job.start.2 - job.end.2).abs();
                }
                if let Some(m) = air_mip.as_mut() {
                    m.absorb(r);
                }
                patch(job.sample_slot, r.finish(cutter, capture_arc_engagement));
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
             per partial in `CAPS` and in `DELTA_sim_w4.md` §2c"
        );
        assert!(
            job <= 72,
            "`StampJob` is {job} B; the batch bound is documented at 72 B per job"
        );
        let bytes = CAPS.max_partials * (partial + index) + CAPS.max_jobs * (job + partial);
        assert!(
            bytes <= 24 * 1024 * 1024,
            "a batch's working set is {} MB (partial {partial} B, job {job} B) — \
             the documented bound is ~23 MB and both it and the caps need \
             revisiting together",
            bytes / (1024 * 1024)
        );
    }

    /// `Auto` must not turn whole-path dispatch on. Since w5b that holds for a
    /// second reason on top of the band count — `Auto` resolves to `Swept`,
    /// which runs its own driver — and the assertion is kept because the
    /// explicit arms below it are what the A/B harness depends on.
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
