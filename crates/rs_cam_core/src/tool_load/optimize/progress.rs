//! What the candidate search publishes about its own progress (WP29).
//!
//! Programme:
//! `planning/arch_consolidation_2026-09-09/IMPLEMENTATION_PLAN.md` §33
//! (operator ruling, 2026-09-13). The operator watched a four-minute
//! Optimize run and read a spinner and a second count. The row and the
//! Optimize window now name the rung of the ladder and the candidate
//! inside it.
//!
//! # Why the counts are PER RUNG
//!
//! The search walks a fixed ladder of two rungs, and each rung forms
//! its own candidate list in one call before it walks it. A rung's total
//! is therefore honest the moment that rung starts, and a WHOLE-RUN
//! total is not: the depth and width cells are known only after the
//! link rung decides which dressups they carry. One pair per rung
//! records what each rung did, and the pairs stand after the run
//! returns.
//!
//! **There is no time-left figure, and this type carries no clock.** The
//! whole-run total is not known up front, and a deep cell simulates
//! fewer levels than a shallow one, so one mean second per candidate is
//! not a forecast.
//!
//! # Why atomics and not a `Mutex`
//!
//! The GUI frame loop reads this while the worker thread writes it. A
//! contended or poisoned mutex on the frame loop is a new failure mode
//! for no gain. Six atomics carry every number the row, the window and
//! the sentry read. The ordering is `SeqCst`, which matches the cancel
//! flag the same loops poll.
//!
//! # Why there is no `end_phase`
//!
//! A rung that walks every candidate ends with `reached == total` on its
//! own, because the tick sits at the TOP of the loop and publishes the
//! index the loop enters. A post-loop call that wrote `reached = total`
//! would report a cancelled rung as complete, and it would also hide a
//! tick placed in the wrong place — which is the defect
//! `crates/rs_cam_core/tests/optimize_reports_progress_wp29.rs` measures.
//!
//! This type is runtime-only. It needs no `Serialize`: nothing persists
//! a progress reading, and no MCP reply carries one.

use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering};

/// Which rung of the fixed two-rung ladder the search is on.
///
/// **This is not [`super::SearchStage`].** That enum describes the
/// source of a CANDIDATE; this one describes where the RUN is. The two
/// vocabularies answer different questions.
///
/// The rungs are the levers of the post-sim tuner
/// (`planning/optimize_review_2026-10-12/PLAN.md` §2): first the link
/// lever, then the depth and width lever.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchPhase {
    /// Lever L1: the link-moves dressup on the baseline operation.
    Links,
    /// Lever L2: the depth-per-pass and stepover cells.
    DepthWidth,
}

impl SearchPhase {
    /// The ladder, in the order the search walks it.
    ///
    /// One list, so the rung count appears once. The row reads the length
    /// for its "stage 1/2" denominator and the window walks the list.
    pub const ALL: [SearchPhase; 2] = [SearchPhase::Links, SearchPhase::DepthWidth];

    /// This rung's position on the ladder, counted from one.
    #[must_use]
    pub fn index_from_one(self) -> usize {
        match self {
            SearchPhase::Links => 1,
            SearchPhase::DepthWidth => 2,
        }
    }

    /// Name the rung for the operator.
    ///
    /// Core owns the vocabulary, so the row, the window and a log line
    /// read one name per rung.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            SearchPhase::Links => "links",
            SearchPhase::DepthWidth => "depth and width",
        }
    }

    /// The code this rung stores in the `phase` atomic. Zero means "no
    /// rung runs", so every rung code is positive.
    fn code(self) -> u8 {
        match self {
            SearchPhase::Links => 1,
            SearchPhase::DepthWidth => 2,
        }
    }

    /// Read a rung back from its stored code. `None` for zero.
    fn from_code(code: u8) -> Option<Self> {
        match code {
            1 => Some(SearchPhase::Links),
            2 => Some(SearchPhase::DepthWidth),
            _ => None,
        }
    }
}

/// What the search published about itself. Lock-free, and plain data.
///
/// The search writes it from the worker thread. The GUI frame loop reads
/// it through [`OptimizeProgress::snapshot`]. Core holds no GUI type
/// here: these are atomics and a `Copy` enum.
///
/// Every counter is named rather than indexed, because
/// `clippy::indexing_slicing` is denied workspace-wide and a rung index
/// into an array would need an allow.
#[derive(Debug, Default)]
pub struct OptimizeProgress {
    /// The rung now running, as a [`SearchPhase`] code. Zero means none.
    phase: AtomicU8,
    /// The highest rung code the run ever reached. It never falls.
    high_water: AtomicU8,
    /// Candidates the link rung formed.
    links_total: AtomicUsize,
    /// Candidates the link rung entered.
    links_reached: AtomicUsize,
    /// Cells the depth and width rung formed.
    depth_width_total: AtomicUsize,
    /// Cells the depth and width rung entered.
    depth_width_reached: AtomicUsize,
}

/// One read of an [`OptimizeProgress`], taken on the frame loop.
///
/// The reader takes the whole shape in one call, so the row cannot mix
/// one rung's index with another rung's count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OptimizeProgressSnapshot {
    /// The rung now running. `None` before the first announce and after
    /// [`OptimizeProgress::finish`].
    pub phase: Option<SearchPhase>,
    /// The running rung's position, counted from one. Zero when no rung
    /// runs.
    pub phase_index: usize,
    /// How many rungs the ladder has.
    pub phase_count: usize,
    /// The highest rung the run reached, counted from one. Zero when the
    /// run announced no rung at all.
    pub high_water_index: usize,
    /// The candidate the running rung entered, counted from one. Zero
    /// when the rung has entered none.
    pub candidate: usize,
    /// The candidates the running rung formed.
    pub candidate_total: usize,
}

impl OptimizeProgress {
    /// Announce a rung and the candidate count it formed.
    ///
    /// A rung with no candidates is announced with a total of zero, so
    /// the window still names it.
    pub fn begin_phase(&self, phase: SearchPhase, candidate_total: usize) {
        let (reached, total) = self.slot(phase);
        total.store(candidate_total, Ordering::SeqCst);
        reached.store(0, Ordering::SeqCst);
        self.phase.store(phase.code(), Ordering::SeqCst);
        let _ = self.high_water.fetch_max(phase.code(), Ordering::SeqCst);
    }

    /// Record the candidate the running rung is entering.
    ///
    /// `index_from_zero` is the loop's own `.enumerate()` index. The
    /// stored number is one higher, so the row reads "the candidate now
    /// running". A call before any [`Self::begin_phase`] writes nothing.
    pub fn begin_candidate(&self, index_from_zero: usize) {
        let Some(phase) = SearchPhase::from_code(self.phase.load(Ordering::SeqCst)) else {
            return;
        };
        let (reached, _) = self.slot(phase);
        reached.store(index_from_zero.saturating_add(1), Ordering::SeqCst);
    }

    /// Clear the running rung. The per-rung counters stand, so a settled
    /// run stays readable.
    pub fn finish(&self) {
        self.phase.store(0, Ordering::SeqCst);
    }

    /// What one rung did: `(candidates entered, candidates formed)`.
    #[must_use]
    pub fn rung(&self, phase: SearchPhase) -> (usize, usize) {
        let (reached, total) = self.slot(phase);
        let entered = reached.load(Ordering::SeqCst);
        let formed = total.load(Ordering::SeqCst);
        (entered, formed)
    }

    /// The highest rung the run reached. `None` when it announced none.
    #[must_use]
    pub fn high_water_phase(&self) -> Option<SearchPhase> {
        SearchPhase::from_code(self.high_water.load(Ordering::SeqCst))
    }

    /// Read the whole shape in one call.
    #[must_use]
    pub fn snapshot(&self) -> OptimizeProgressSnapshot {
        let phase = SearchPhase::from_code(self.phase.load(Ordering::SeqCst));
        let (candidate, candidate_total) = match phase {
            Some(p) => self.rung(p),
            None => (0, 0),
        };
        OptimizeProgressSnapshot {
            phase,
            phase_index: phase.map_or(0, SearchPhase::index_from_one),
            phase_count: SearchPhase::ALL.len(),
            high_water_index: self
                .high_water_phase()
                .map_or(0, SearchPhase::index_from_one),
            candidate,
            candidate_total,
        }
    }

    /// One rung's `(reached, total)` pair. A `match`, never an index.
    fn slot(&self, phase: SearchPhase) -> (&AtomicUsize, &AtomicUsize) {
        match phase {
            SearchPhase::Links => (&self.links_reached, &self.links_total),
            SearchPhase::DepthWidth => (&self.depth_width_reached, &self.depth_width_total),
        }
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

    /// A fresh reading claims no rung and counts nothing.
    #[test]
    fn a_fresh_progress_claims_nothing() {
        let progress = OptimizeProgress::default();
        let snapshot = progress.snapshot();
        assert!(snapshot.phase.is_none(), "no rung runs yet");
        assert_eq!(snapshot.phase_index, 0);
        assert_eq!(snapshot.high_water_index, 0);
        assert_eq!(snapshot.candidate, 0);
        assert_eq!(snapshot.candidate_total, 0);
        assert_eq!(snapshot.phase_count, SearchPhase::ALL.len());
        assert!(progress.high_water_phase().is_none());
    }

    /// The mark rises with the ladder and never falls, and each rung
    /// keeps its own pair.
    #[test]
    fn each_rung_keeps_its_own_pair_and_the_mark_never_falls() {
        let progress = OptimizeProgress::default();
        progress.begin_phase(SearchPhase::Links, 1);
        progress.begin_candidate(0);
        progress.begin_phase(SearchPhase::DepthWidth, 8);
        progress.begin_candidate(2);

        let snapshot = progress.snapshot();
        assert_eq!(snapshot.phase, Some(SearchPhase::DepthWidth));
        assert_eq!(snapshot.phase_index, 2);
        assert_eq!(snapshot.candidate, 3, "the tick publishes index + 1");
        assert_eq!(snapshot.candidate_total, 8);
        assert_eq!(
            progress.rung(SearchPhase::Links),
            (1, 1),
            "the earlier rung's pair stands"
        );
        assert_eq!(progress.high_water_phase(), Some(SearchPhase::DepthWidth));

        // A re-announce of an EARLIER rung must not lower the mark.
        progress.begin_phase(SearchPhase::Links, 3);
        assert_eq!(
            progress.high_water_phase(),
            Some(SearchPhase::DepthWidth),
            "the high-water mark never falls"
        );
    }

    /// `finish` clears the rung and leaves every count readable.
    #[test]
    fn finish_clears_the_rung_and_keeps_the_counts() {
        let progress = OptimizeProgress::default();
        progress.begin_phase(SearchPhase::DepthWidth, 2);
        progress.begin_candidate(1);
        progress.finish();

        let snapshot = progress.snapshot();
        assert!(snapshot.phase.is_none(), "a settled run claims no rung");
        assert_eq!(snapshot.high_water_index, 2, "but it says how far it got");
        assert_eq!(progress.rung(SearchPhase::DepthWidth), (2, 2));
    }

    /// A tick before any announce writes nothing, rather than guessing a
    /// rung.
    #[test]
    fn a_tick_without_a_rung_writes_nothing() {
        let progress = OptimizeProgress::default();
        progress.begin_candidate(5);
        for phase in SearchPhase::ALL {
            assert_eq!(progress.rung(phase), (0, 0));
        }
    }
}
