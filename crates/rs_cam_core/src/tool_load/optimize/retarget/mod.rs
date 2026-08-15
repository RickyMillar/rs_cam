//! Sample-driven retargeters — translate per-gate sim verdicts into
//! axis patches. One implementation per gate.
//!
//! Step 5 scaffold (G16). Concrete implementations live in
//! `chipload.rs`, `power.rs`, `deflection.rs` — each picked up by a
//! parallel agent. The shared trait + `RetargetSolution` are defined
//! here so all three can land independently.

use serde::{Deserialize, Serialize};

use crate::tool_load::RefuseReason;
use crate::tool_load::verdict::{ChipBoundsSource, ChipSide};

use super::axes::{AxisContext, AxisView, SearchAxis};
use super::patches::AxisPatch;
use super::space::SearchSpace;

pub mod chipload;
pub mod deflection;
pub mod power;

/// One retargeter per load-driving gate. The verdict type is gate-
/// specific at the trait level; Step 7 swaps the existing flat `Verdict`
/// for typed verdicts (`ChiploadVerdict`, `PowerVerdict`,
/// `DeflectionVerdict`) — that change is local to each retargeter file.
pub trait Retargeter {
    type Verdict;

    /// Axes this retargeter is allowed to drive. Declaring this in the
    /// trait is part of the contract — a retargeter that names only
    /// `[FeedRate]` commits to NOT touching RPM, which keeps the linear
    /// chipload-feed math correct.
    fn driving_axes(&self) -> &'static [SearchAxis];

    /// Compute a target patch list for the given verdict.
    ///
    /// Three answers, and the difference between the last two is the
    /// whole of Q-NARROW (c):
    ///
    /// - [`RetargetOutcome::Solved`] — a patch list to evaluate.
    /// - [`RetargetOutcome::NotApplicable`] — this isn't an `Exceeds`
    ///   arm of the gate this retargeter handles, or an input the math
    ///   cannot be run on at all (each retargeter is typed to exactly
    ///   one gate's verdict via the associated `Verdict` type).
    /// - [`RetargetOutcome::Refused`] — the retarget was **declined on
    ///   the evidence**, with a typed reason carrying the numbers that
    ///   produced it. Not the same as `NotApplicable`: something was
    ///   asked and answered "no, and here is why".
    fn target(
        &self,
        verdict: &Self::Verdict,
        space: &SearchSpace,
        view: &AxisView<'_>,
        ctx: &AxisContext<'_>,
    ) -> RetargetOutcome;
}

/// What a [`Retargeter`] answered. See [`Retargeter::target`] for the
/// contract between the three arms.
#[derive(Debug, Clone)]
pub enum RetargetOutcome {
    /// Nothing to do — not this retargeter's arm, or an unmodellable
    /// input. Carries no reason **by design**: there is no claim here
    /// to explain.
    NotApplicable,
    /// A patch list to apply and evaluate.
    Solved(RetargetSolution),
    /// A typed refusal — the retargeter could compute a target and has
    /// decided it must not be emitted.
    Refused(RetargetRefusal),
}

impl RetargetOutcome {
    /// The solution, if there is one. `Refused` collapses to `None`
    /// here — callers that care about the difference must match.
    #[must_use]
    pub fn solution(self) -> Option<RetargetSolution> {
        match self {
            Self::Solved(s) => Some(s),
            Self::NotApplicable | Self::Refused(_) => None,
        }
    }

    /// The refusal, if this is one.
    #[must_use]
    pub fn refusal(&self) -> Option<&RetargetRefusal> {
        match self {
            Self::Refused(r) => Some(r),
            Self::NotApplicable | Self::Solved(_) => None,
        }
    }
}

/// Typed reason a retargeter declined to emit a candidate.
///
/// One variant today. The enum exists so a second refusal class cannot
/// be added as prose in a rationale string — the pattern the optimizer
/// already uses for [`RefuseReason`] and
/// [`super::narrative::DeflectionSetupDetail`]: a machine-readable
/// record next to the operator sentence, never instead of it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetargetRefusal {
    /// **Q-NARROW (c), 2026-08-14.** The vendor band the gate judges by
    /// is narrower than the retarget's own headroom policy, so the
    /// headroom target falls outside the band and no feed can satisfy
    /// both. See [`NarrowChipBandRefusal`].
    ChiploadBandNarrowerThanHeadroom(NarrowChipBandRefusal),
}

impl RetargetRefusal {
    /// The optimizer-level classification this refusal maps to, for
    /// [`super::OptimizeOutcome::reason`].
    #[must_use]
    pub fn reason(&self) -> RefuseReason {
        match self {
            Self::ChiploadBandNarrowerThanHeadroom(_) => {
                RefuseReason::ChiploadBandNarrowerThanHeadroom
            }
        }
    }

    /// One-line operator sentence carrying the numbers behind the
    /// refusal. The structured record is still on the outcome — this is
    /// the prose half, not a substitute for it.
    #[must_use]
    pub fn explanation(&self) -> String {
        match self {
            Self::ChiploadBandNarrowerThanHeadroom(d) => d.explanation(),
        }
    }
}

/// The numbers behind a [`RetargetRefusal::ChiploadBandNarrowerThanHeadroom`].
///
/// # What is being refused, and why it is not a clamp
///
/// The chipload retargeter aims at the band the gate judged by
/// (Checkpoint P (1a)), pulled off the boundary by a repo-authored
/// headroom factor: `min × low_headroom` on the burn side,
/// `max / high_headroom` on the breakage side. On a band narrower than
/// that factor — measured at **86 of 235** two-sided shipped LUT rows,
/// 48 of them single-point rows where `max == min`
/// (`planning/review_2026-08-08/artifacts/a8i/narrow_band_census.txt`)
/// — the target lands outside the band, and
/// [`crate::tool_load::verdict::ChipBounds::contains`], the gate's own
/// predicate, says so before a single sim runs.
///
/// A-8i measured this and reported it (P-(1b)); Q-NARROW ruled that the
/// retargeter must **refuse** rather than emit the doomed candidate —
/// and explicitly rejected inventing an in-band target (option (b)),
/// because a clamped target is a number the vendor row does not
/// support. The refusal is therefore the honest answer *and* the census
/// instrument for the follow-up research row on the headroom policy
/// itself (Q-NARROW (d)).
///
/// # Predicate: the side's own target, not a band-ratio test
///
/// The trigger is `!bounds.contains(target)` for the side being
/// retargeted, evaluated through the [`crate::tool_load::boundary`]
/// epsilon helpers. Under the shipped policy (`low_headroom ==
/// high_headroom == 1.20`) that is exactly the ruling's two-sided
/// condition — `min × h > max` **and** `max / h < min` are the same
/// statement about the band ratio. They can only come apart if the two
/// dials are set differently, and then the correct answer is still to
/// refuse the side whose target is provably outside the bar it would be
/// judged by. Both targets are recorded either way, so a consumer can
/// see whether the other side was reachable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NarrowChipBandRefusal {
    /// Which `Exceeds` side was being retargeted when the refusal fired.
    pub side: ChipSide,
    /// Band floor the gate judged by (DOC-derated). `None` on a
    /// half-band row — unmodelled, never zero.
    pub band_min_mm_per_tooth: Option<f64>,
    /// Band ceiling the gate judged by (DOC-derated).
    pub band_max_mm_per_tooth: f64,
    /// Where that band came from, carried so a refusal can be traced to
    /// its row rather than re-derived.
    pub band_source: ChipBoundsSource,
    /// Burn-side headroom multiplier applied to the floor.
    pub low_headroom: f64,
    /// Breakage-side headroom divisor applied to the ceiling.
    pub high_headroom: f64,
    /// `band_min × low_headroom`. `None` when the row publishes no floor.
    pub low_target_mm_per_tooth: Option<f64>,
    /// `band_max / high_headroom`.
    pub high_target_mm_per_tooth: f64,
    /// The observed peak that tripped the gate, mm/tooth — the number
    /// the refused multiplier would have been computed from.
    pub observed_mm_per_tooth: f64,
}

impl NarrowChipBandRefusal {
    /// The headroom target for [`Self::side`] — the one that was found
    /// unreachable.
    #[must_use]
    pub fn refused_target_mm_per_tooth(&self) -> Option<f64> {
        match self.side {
            ChipSide::Low => self.low_target_mm_per_tooth,
            ChipSide::High => Some(self.high_target_mm_per_tooth),
        }
    }

    /// Operator sentence. Names the band, the unreachable target, and
    /// the dial responsible — so the reader can tell this from "we
    /// searched and found nothing", which is a different claim.
    #[must_use]
    pub fn explanation(&self) -> String {
        let min = self
            .band_min_mm_per_tooth
            .map_or_else(|| "none".to_owned(), |m| format!("{m:.4}"));
        let target = self
            .refused_target_mm_per_tooth()
            .map_or_else(|| "none".to_owned(), |t| format!("{t:.4}"));
        let (dial, headroom) = match self.side {
            ChipSide::Low => ("low", self.low_headroom),
            ChipSide::High => ("high", self.high_headroom),
        };
        format!(
            "chipload retarget refused: the band the gate judges by \
             [{min}, {max:.4}] mm/tooth is narrower than the {headroom:.2}× \
             {dial}-side headroom, so its target {target} mm/tooth falls \
             outside that band — no feed can both clear the headroom and \
             stay inside the band, and the optimizer will not invent one",
            max = self.band_max_mm_per_tooth,
        )
    }
}

/// Result of a successful retarget. `patches` carry the change(s) to
/// apply; `rationale` is a human-readable explanation surfaced in MCP /
/// GUI output. Multi-patch when the retargeter has coupled levers
/// (chipload retarget produces a feed patch and a coupled plunge-
/// tracking patch).
#[derive(Debug, Clone)]
pub struct RetargetSolution {
    pub patches: Vec<AxisPatch>,
    pub rationale: String,
}
