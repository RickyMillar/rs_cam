//! Outcome types and the tier dispatcher (`build_outcome`).
//!
//! - [`OptimizeOutcome`] — what `optimize_toolpath` returns: a unified
//!   struct carrying a [`OutcomeKind`] tag plus always-present
//!   `candidates` / `narrative` / `recommended_index` / `reason` fields
//!   (Roadmap F.7). MCP agents and the GUI modal read the same shape
//!   for every tier, branching only on `kind`.
//! - [`ProjectOptimizeReport`] — project-level rollup over every
//!   enabled toolpath.
//! - [`build_outcome`] — tier dispatcher. Sorts the Stage-2 candidates,
//!   populates each candidate's `gate_deltas`, and folds the list into
//!   the appropriate [`OutcomeKind`].

use serde::{Deserialize, Serialize};

use crate::tool_load::RefuseReason;

use super::candidate::OptimizeCandidate;
use super::delta::{
    candidate_is_marginally_safe, candidate_is_safe, candidate_is_strictly_safe,
    classify_candidate_vs_baseline,
};
use super::narrative::{
    OutcomeNarrative, build_marginal_safe_narrative, build_no_safe_narrative,
    build_ranked_narrative, build_tradeoff_narrative,
};
use super::rank::composite_score;
use super::search_policy;

/// Tier of an [`OptimizeOutcome`]. Replaces the prior variant-tag on a
/// tagged enum so the outcome's data shape stays uniform across tiers
/// — only the kind tag varies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutcomeKind {
    /// At least one candidate is strictly safe AND faster than baseline.
    /// `recommended_index` points at the auto-Apply target.
    Ranked,
    /// At least one candidate is faster AND every gate is `Within`, but
    /// at least one `Within` reading was admitted only by the layer-1
    /// tolerance band (G16 §11.4) — would be `Exceeds` under the strict
    /// LUT bound. The user must explicitly confirm before applying
    /// ("verify on a scrap"); the optimizer won't auto-recommend.
    /// `recommended_index` points at the verify-on-scrap target.
    MarginalSafe,
    /// At least one candidate is faster AND improves a failing baseline
    /// gate, but also worsens a non-failing one. The user has to
    /// explicitly accept the regression — the optimizer can't
    /// auto-recommend a trade-off. `recommended_index` is None;
    /// `narrative.improved_gates` / `worsened_gates` describe the swap.
    TradeOff,
    /// Every non-baseline candidate either failed the gate or was
    /// slower than baseline (or pre-flight refused before any sim
    /// fired). `reason` carries the refuse classification;
    /// `narrative.limiting_gates` carries what the closest candidate
    /// hit; `narrative.suggestions` lists machine-feasible operator
    /// levers.
    NoSafeImprovement,
    /// The optimizer can't model this toolpath at all — drill cycles,
    /// project_curve with no steady-state samples, custom materials.
    /// `reason` is set; `candidates` is empty.
    Skipped,
}

/// Outcome of `optimize_toolpath` for one toolpath. Roadmap F.7
/// collapsed the prior variant-shaped enum into one struct with a
/// [`OutcomeKind`] tag, so every consumer can read `candidates`,
/// `narrative`, `recommended_index`, and `reason` uniformly without
/// per-tier pattern matching. Per-tier semantics are encoded by which
/// fields the constructors populate:
///
/// | kind | candidates | narrative | recommended_index | reason |
/// |---|---|---|---|---|
/// | Ranked | baseline + sorted | headline + envelope | Some(N) for first strict-safe | None |
/// | MarginalSafe | baseline + sorted | + limiting_gates (band) | Some(N) for first band-safe | None |
/// | TradeOff | baseline + sorted | + improved/worsened gates | None | None |
/// | NoSafeImprovement | baseline + attempted | + limiting_gates + suggestions | None | Some(_) |
/// | Skipped | empty | empty | None | Some(_) |
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizeOutcome {
    pub kind: OutcomeKind,
    /// Index 0 is always the baseline (for non-Skipped outcomes);
    /// subsequent entries are the search candidates already sorted by
    /// composite score with `gate_deltas` populated. Empty for
    /// `Skipped`.
    pub candidates: Vec<OptimizeCandidate>,
    /// Boxed to keep the outcome's size symmetric and small for use
    /// inside `Vec<OptimizeOutcome>` collections (the narrative carries
    /// strings + Vecs that would otherwise dominate the struct size).
    pub narrative: Box<OutcomeNarrative>,
    /// Auto-Apply / verify-on-scrap target index into `candidates`.
    /// `Some(N)` only when `kind` is `Ranked` (first strictly-safe
    /// faster candidate) or `MarginalSafe` (first marginally-safe
    /// faster candidate). `None` for `TradeOff`, `NoSafeImprovement`,
    /// `Skipped`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recommended_index: Option<usize>,
    /// Refusal classification. `Some(_)` only when `kind` is
    /// `NoSafeImprovement` or `Skipped`; `None` for the other tiers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<RefuseReason>,
}

impl OptimizeOutcome {
    /// Construct a `Ranked` outcome. Caller is responsible for sorting
    /// `candidates` (baseline at index 0) and computing
    /// `recommended_index` via [`ProjectOptimizeReport::first_safe_index`].
    pub fn ranked(
        candidates: Vec<OptimizeCandidate>,
        recommended_index: Option<usize>,
        narrative: OutcomeNarrative,
    ) -> Self {
        Self {
            kind: OutcomeKind::Ranked,
            candidates,
            narrative: Box::new(narrative),
            recommended_index,
            reason: None,
        }
    }

    /// Construct a `MarginalSafe` outcome.
    pub fn marginal_safe(
        candidates: Vec<OptimizeCandidate>,
        recommended_index: Option<usize>,
        narrative: OutcomeNarrative,
    ) -> Self {
        Self {
            kind: OutcomeKind::MarginalSafe,
            candidates,
            narrative: Box::new(narrative),
            recommended_index,
            reason: None,
        }
    }

    /// Construct a `TradeOff` outcome. `recommended_index` is always
    /// `None` — trade-offs require explicit user acceptance via the
    /// modal.
    pub fn trade_off(candidates: Vec<OptimizeCandidate>, narrative: OutcomeNarrative) -> Self {
        Self {
            kind: OutcomeKind::TradeOff,
            candidates,
            narrative: Box::new(narrative),
            recommended_index: None,
            reason: None,
        }
    }

    /// Construct a `NoSafeImprovement` outcome. `candidates` includes
    /// the baseline at index 0 plus every candidate the optimizer
    /// managed to evaluate; the prior `attempted` field is folded in.
    pub fn no_safe_improvement(
        candidates: Vec<OptimizeCandidate>,
        reason: RefuseReason,
        narrative: OutcomeNarrative,
    ) -> Self {
        Self {
            kind: OutcomeKind::NoSafeImprovement,
            candidates,
            narrative: Box::new(narrative),
            recommended_index: None,
            reason: Some(reason),
        }
    }

    /// Construct a `Skipped` outcome — the optimizer refused before
    /// evaluating any candidate.
    pub fn skipped(reason: RefuseReason) -> Self {
        Self {
            kind: OutcomeKind::Skipped,
            candidates: Vec::new(),
            narrative: Box::default(),
            recommended_index: None,
            reason: Some(reason),
        }
    }

    /// Recommended candidate from a `Ranked` outcome: the first
    /// non-baseline strictly-safe candidate faster than baseline by
    /// more than the policy recommendation cycle delta. Returns `None`
    /// for any other `kind`.
    ///
    /// Why faster-than-baseline matters: a `Ranked` outcome may surface
    /// candidates the user can override to (per the modal's table),
    /// but the *recommendation* — the ⭐ row in the modal — should be
    /// a candidate that actually wins on cycle time. An equally-fast
    /// or slower safe candidate is information, not a recommendation.
    pub fn first_safe(&self) -> Option<&OptimizeCandidate> {
        if self.kind != OutcomeKind::Ranked {
            return None;
        }
        let baseline = self.candidates.first()?;
        let min_cycle_delta_s = search_policy().ranking.recommendation_cycle_delta_s.value;
        self.candidates.iter().skip(1).find(|c| {
            candidate_is_strictly_safe(c)
                && c.cycle_time_s + min_cycle_delta_s < baseline.cycle_time_s
        })
    }

    /// Recommended candidate from a `MarginalSafe` outcome: the first
    /// non-baseline marginally-safe candidate (every gate `Within`, at
    /// least one reading band-admitted) faster than baseline by more
    /// than the policy recommendation cycle delta. Returns `None` for
    /// any other `kind`.
    ///
    /// Distinct from [`first_safe`](Self::first_safe): the modal must
    /// surface this as a "verify on a scrap" recommendation, not an
    /// auto-Apply target.
    pub fn first_marginal_safe(&self) -> Option<&OptimizeCandidate> {
        if self.kind != OutcomeKind::MarginalSafe {
            return None;
        }
        let baseline = self.candidates.first()?;
        let min_cycle_delta_s = search_policy().ranking.recommendation_cycle_delta_s.value;
        self.candidates.iter().skip(1).find(|c| {
            candidate_is_marginally_safe(c)
                && c.cycle_time_s + min_cycle_delta_s < baseline.cycle_time_s
        })
    }
}

/// Project-level rollup over every enabled toolpath. Surfaced by U3's
/// Optimize-project view.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectOptimizeReport {
    /// Baseline project cycle time (seconds), as measured by the sim
    /// already on screen.
    pub baseline_cycle_time_s: f64,
    /// Toolpath index that dominates runtime (the "Bottleneck:"
    /// callout). `None` if no toolpath crosses the threshold (currently
    /// 30% of total runtime — calibrated against wanaka in U3).
    pub bottleneck_index: Option<usize>,
    /// Per-toolpath outcome paired with the toolpath index it relates
    /// to.
    pub per_toolpath: Vec<(usize, OptimizeOutcome)>,
}

impl ProjectOptimizeReport {
    /// Index of the first non-baseline candidate that's safe (no
    /// `Exceeds` verdict on any criterion) and faster than baseline
    /// by more than the policy recommendation cycle delta. Returns `None`
    /// if no such candidate exists. This is the index version of
    /// [`OptimizeOutcome::first_safe`] so callers can mutate the
    /// candidate in place (e.g. populate reconciled values during
    /// U4 reconciliation).
    pub fn first_safe_index(candidates: &[OptimizeCandidate]) -> Option<usize> {
        let baseline = candidates.first()?;
        let min_cycle_delta_s = search_policy().ranking.recommendation_cycle_delta_s.value;
        candidates
            .iter()
            .enumerate()
            .skip(1)
            .find(|(_, c)| {
                candidate_is_strictly_safe(c)
                    && c.cycle_time_s + min_cycle_delta_s < baseline.cycle_time_s
            })
            .map(|(i, _)| i)
    }

    /// Index version of [`OptimizeOutcome::first_marginal_safe`] — the
    /// first non-baseline candidate that's marginally safe and faster
    /// than baseline by more than the policy recommendation cycle delta.
    /// Caller is responsible for matching the right outcome variant
    /// before reading; this works on a raw candidate slice.
    pub fn first_marginal_safe_index(candidates: &[OptimizeCandidate]) -> Option<usize> {
        let baseline = candidates.first()?;
        let min_cycle_delta_s = search_policy().ranking.recommendation_cycle_delta_s.value;
        candidates
            .iter()
            .enumerate()
            .skip(1)
            .find(|(_, c)| {
                candidate_is_marginally_safe(c)
                    && c.cycle_time_s + min_cycle_delta_s < baseline.cycle_time_s
            })
            .map(|(i, _)| i)
    }

    /// Estimated project cycle time if the operator applied exactly the
    /// rows flagged `true` in `selected` (parallel to `per_toolpath`;
    /// missing trailing entries are treated as unselected).
    ///
    /// Computed as the true project baseline **minus** the realized
    /// per-row savings, rather than re-summing per-row baselines. Only a
    /// selected `Ranked` row with a recommended faster candidate
    /// (`first_safe`) contributes a saving; every other row keeps its
    /// real cost in the total — including `Skipped` rows, which carry no
    /// candidate cycle at all and so can't be re-summed honestly. The
    /// pre-W0.2 header re-summed per-row baselines and folded refused /
    /// skipped rows to `0.0`, understating the optimized time and
    /// inflating the headline savings on every such row (OPT-001).
    pub fn optimized_cycle_time_s(&self, selected: &[bool]) -> f64 {
        let total_saving: f64 = self
            .per_toolpath
            .iter()
            .zip(selected.iter().chain(std::iter::repeat(&false)))
            .filter_map(|((_, outcome), &sel)| {
                if !sel {
                    return None;
                }
                // `first_safe` only resolves for `Ranked` outcomes faster
                // than baseline, so non-applicable rows contribute nothing.
                let baseline = outcome.candidates.first()?.cycle_time_s;
                let improved = outcome.first_safe()?.cycle_time_s;
                Some((baseline - improved).max(0.0))
            })
            .sum();
        (self.baseline_cycle_time_s - total_saving).max(0.0)
    }
}

/// Build an `OptimizeOutcome` from a baseline candidate and the
/// Stage-2 refined candidates. Tier dispatcher per the redesign plan
/// and G16 §11.4 Layer 3:
///
///   - Empty candidates → `NoSafeImprovement` (no improvement found).
///   - At least one candidate is faster AND strictly safe (every gate
///     `Within` AND every reading inside the strict LUT bound) AND has
///     no regression → `Ranked`. Auto-recommendation surface.
///   - Else, at least one candidate is faster AND marginally safe
///     (every gate `Within` but at least one reading admitted only by
///     the layer-1 tolerance band) AND has no regression →
///     `MarginalSafe`. Verify-on-a-scrap recommendation.
///   - Else, at least one candidate is faster AND improves a failing
///     gate while worsening a non-failing one → `TradeOff`.
///   - Otherwise → `NoSafeImprovement`.
///
/// Every non-baseline candidate carries populated `gate_deltas` after
/// this function runs, so consumers don't have to recompute.
pub(crate) fn build_outcome(
    baseline: OptimizeCandidate,
    candidates: Vec<OptimizeCandidate>,
    machine: &crate::machine::MachineProfile,
) -> OptimizeOutcome {
    if candidates.is_empty() {
        let attempted = vec![baseline];
        let narrative = attempted
            .first()
            .map(|b| build_no_safe_narrative(b, &attempted, machine))
            .map(|mut n| {
                n.explanation = format!(
                    "{}: no candidates were produced — operation has no geometry knobs and feed/RPM are at machine limits",
                    RefuseReason::NoImprovementFound.explanation_for_optimize()
                );
                n
            })
            .unwrap_or_default();
        return OptimizeOutcome::no_safe_improvement(
            attempted,
            RefuseReason::NoImprovementFound,
            narrative,
        );
    }

    // Populate per-candidate gate deltas vs baseline. Done up-front so
    // every downstream branch sees the same data on the candidates,
    // not just the surviving tier.
    let baseline_verdict = baseline.verdict.clone();
    let mut sorted = candidates;
    for c in sorted.iter_mut() {
        c.gate_deltas = Some(classify_candidate_vs_baseline(
            &baseline_verdict,
            &c.verdict,
        ));
    }
    // G16 §11 layer 2b — sort by composite_score descending (highest
    // score = best). Replaces the prior cycle-time-only sort so that
    // band-admitted candidates near the chipload edge don't outrank a
    // mid-bracket sibling at comparable cycle time.
    let policy = search_policy();
    sorted.sort_by(|a, b| {
        composite_score(b, &baseline, policy).total_cmp(&composite_score(a, &baseline, policy))
    });

    let baseline_cycle = baseline.cycle_time_s;
    let min_cycle_delta_s = search_policy().ranking.recommendation_cycle_delta_s.value;
    let is_faster = |c: &OptimizeCandidate| c.cycle_time_s + min_cycle_delta_s < baseline_cycle;

    let no_regression =
        |c: &OptimizeCandidate| c.gate_deltas.map(|d| d.no_regression()).unwrap_or(false);

    // Pure improvement (Ranked tier): faster AND strictly safe AND no
    // regression. Strictly-safe = no Exceeds AND no band-admitted Within
    // — see `delta::candidate_is_strictly_safe`.
    let any_pure_improvement = sorted
        .iter()
        .any(|c| candidate_is_strictly_safe(c) && no_regression(c) && is_faster(c));

    // Marginally-safe improvement (G16 §11.4 Layer 3): faster AND every
    // gate Within AND no regression, but at least one Within reading
    // was admitted only by the layer-1 tolerance band. The user must
    // verify on a scrap — the optimizer surfaces but doesn't auto-Apply.
    let any_marginal_improvement = !any_pure_improvement
        && sorted
            .iter()
            .any(|c| candidate_is_marginally_safe(c) && no_regression(c) && is_faster(c));

    // Trade-off tier: faster AND improves something AND worsens
    // something. Pure / marginal improvements take priority — only
    // land in TradeOff if neither Ranked nor MarginalSafe applies.
    let any_tradeoff = !any_pure_improvement
        && !any_marginal_improvement
        && sorted.iter().any(|c| {
            c.gate_deltas
                .map(|d| d.any_improved() && d.any_worsened())
                .unwrap_or(false)
                && is_faster(c)
        });

    if any_pure_improvement {
        let mut ranked = Vec::with_capacity(sorted.len() + 1);
        ranked.push(baseline);
        ranked.extend(sorted);
        let recommended_index = ProjectOptimizeReport::first_safe_index(&ranked);
        let narrative = build_ranked_narrative(&ranked);
        return OptimizeOutcome::ranked(ranked, recommended_index, narrative);
    }

    if any_marginal_improvement {
        let mut marginal = Vec::with_capacity(sorted.len() + 1);
        marginal.push(baseline);
        marginal.extend(sorted);
        let recommended_index = ProjectOptimizeReport::first_marginal_safe_index(&marginal);
        let mut narrative = marginal
            .first()
            .map(|b| build_marginal_safe_narrative(b, &marginal))
            .unwrap_or_default();
        narrative.explanation = "Best candidate is admitted only by the layer-1 tolerance band — \
                 verify on a scrap before applying. The strict LUT bound was \
                 exceeded by less than the configured breakage / burn tolerance."
            .to_owned();
        return OptimizeOutcome::marginal_safe(marginal, recommended_index, narrative);
    }

    if any_tradeoff {
        let mut tradeoffs = Vec::with_capacity(sorted.len() + 1);
        tradeoffs.push(baseline);
        tradeoffs.extend(sorted);
        let narrative = tradeoffs
            .first()
            .map(|b| build_tradeoff_narrative(b, &tradeoffs))
            .unwrap_or_default();
        return OptimizeOutcome::trade_off(tradeoffs, narrative);
    }

    // Fall through: NoSafeImprovement, with the same attempted-list
    // shape as before.
    let all_unsafe = sorted.iter().all(|c| !candidate_is_safe(c));
    let explanation = if all_unsafe {
        format!(
            "{}: every candidate hit a gate limit (chipload, power, or deflection)",
            RefuseReason::NoImprovementFound.explanation_for_optimize()
        )
    } else {
        format!(
            "{}: no candidate beat the baseline cycle time by more than {:.1}s",
            RefuseReason::NoImprovementFound.explanation_for_optimize(),
            min_cycle_delta_s
        )
    };
    let mut attempted = Vec::with_capacity(sorted.len() + 1);
    attempted.push(baseline);
    attempted.extend(sorted);
    let mut narrative = attempted
        .first()
        .map(|b| build_no_safe_narrative(b, &attempted, machine))
        .unwrap_or_default();
    narrative.explanation = explanation;
    OptimizeOutcome::no_safe_improvement(attempted, RefuseReason::NoImprovementFound, narrative)
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

    fn report_with(baseline_s: f64, outcomes: Vec<OptimizeOutcome>) -> ProjectOptimizeReport {
        ProjectOptimizeReport {
            baseline_cycle_time_s: baseline_s,
            bottleneck_index: None,
            per_toolpath: outcomes.into_iter().enumerate().collect(),
        }
    }

    #[test]
    fn skipped_and_refused_rows_do_not_deflate_optimized_total() {
        // Two rows the optimizer can't improve: a Skipped drill (no
        // candidates at all) and a NoSafeImprovement. Neither has a
        // recommended faster candidate, so the optimized estimate must
        // equal the baseline. The pre-W0.2 header folded these to zero,
        // understating optimized time and inflating the savings %.
        let report = report_with(
            120.0,
            vec![
                OptimizeOutcome::skipped(RefuseReason::NoImprovementFound),
                OptimizeOutcome::no_safe_improvement(
                    Vec::new(),
                    RefuseReason::NoImprovementFound,
                    OutcomeNarrative::default(),
                ),
            ],
        );
        // Even with both rows checked, nothing is applicable → no saving.
        assert_eq!(report.optimized_cycle_time_s(&[true, true]), 120.0);
    }

    #[test]
    fn empty_selection_returns_exact_baseline() {
        let report = report_with(
            80.0,
            vec![OptimizeOutcome::skipped(RefuseReason::NoImprovementFound)],
        );
        // No selection slice at all — missing entries treated unselected.
        assert_eq!(report.optimized_cycle_time_s(&[]), 80.0);
    }
}
