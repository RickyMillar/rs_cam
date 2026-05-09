//! Outcome types and the tier dispatcher (`build_outcome`).
//!
//! - [`OptimizeOutcome`] — what `optimize_toolpath` returns: Ranked,
//!   TradeOff, NoSafeImprovement, or Skipped.
//! - [`ProjectOptimizeReport`] — project-level rollup over every
//!   enabled toolpath.
//! - [`build_outcome`] — tier dispatcher. Sorts the Stage-2 candidates,
//!   populates each candidate's `gate_deltas`, and folds the list into
//!   one of the four `OptimizeOutcome` variants.

use serde::{Deserialize, Serialize};

use crate::tool_load::RefuseReason;

use super::candidate::OptimizeCandidate;
use super::delta::{candidate_is_safe, classify_candidate_vs_baseline};
use super::rank::composite_score;
use super::search_policy;

/// Outcome of `optimize_toolpath` for one toolpath.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "detail", rename_all = "snake_case")]
pub enum OptimizeOutcome {
    /// At least one candidate was generated. Index 0 is always the
    /// baseline (current params). The recommendation is whichever
    /// candidate has `.first_safe()` returns — the first non-baseline
    /// candidate whose verdict is not `Exceeds` on any criterion.
    Ranked(Vec<OptimizeCandidate>),
    /// At least one candidate is faster than baseline AND improves a
    /// failing baseline gate, but also worsens a non-failing one.
    /// Distinct from `Ranked` because the user has to explicitly
    /// accept the regression — the optimizer can't auto-recommend a
    /// trade-off.
    ///
    /// Index 0 is the baseline; subsequent entries are sorted ascending
    /// by cycle time and carry populated `gate_deltas`.
    TradeOff(Vec<OptimizeCandidate>),
    /// Every non-baseline candidate either failed the gate (Exceeds on
    /// some criterion) or was slower than baseline. The rollup row
    /// surfaces this with the binding-limit narrative. The
    /// `attempted` list lets the user see what the optimizer tried
    /// — without it, "no improvement found" is opaque.
    NoSafeImprovement {
        reason: RefuseReason,
        /// Narrative composed at outcome time via
        /// `RefuseReason::explanation_for_optimize` (Engineering
        /// Default 4). Free-form English for the modal.
        explanation: String,
        /// Every candidate that made it to Stage 2 evaluation,
        /// including the baseline at index 0. Stage-1 candidates that
        /// didn't survive into Stage 2 are not included (they're
        /// intermediate). Empty when the search bailed before any
        /// candidate could be evaluated (e.g. cancel-up-front).
        attempted: Vec<OptimizeCandidate>,
    },
    /// The optimizer can't model this toolpath at all — drill cycles,
    /// project_curve with no steady-state samples, custom materials.
    /// The gate refuses, so the optimizer refuses.
    Skipped { reason: RefuseReason },
}

impl OptimizeOutcome {
    /// Recommended candidate: the first non-baseline candidate that
    /// (a) passes the gate (no `Exceeds` verdict on any criterion)
    /// AND (b) is faster than baseline by more than
    /// the policy recommendation cycle delta. Returns `None` for `Skipped` /
    /// `NoSafeImprovement` outcomes, and for `Ranked` outcomes where
    /// no candidate clears both bars.
    ///
    /// Why faster-than-baseline matters: `Ranked` may surface
    /// candidates the user can override to (per the modal's table),
    /// but the *recommendation* — the ⭐ row in the modal — should be
    /// a candidate that actually wins on cycle time. An equally-fast
    /// or slower safe candidate is information, not a recommendation.
    pub fn first_safe(&self) -> Option<&OptimizeCandidate> {
        let OptimizeOutcome::Ranked(candidates) = self else {
            return None;
        };
        let baseline = candidates.first()?;
        let min_cycle_delta_s = search_policy().ranking.recommendation_cycle_delta_s.value;
        candidates.iter().skip(1).find(|c| {
            candidate_is_safe(c) && c.cycle_time_s + min_cycle_delta_s < baseline.cycle_time_s
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
                candidate_is_safe(c) && c.cycle_time_s + min_cycle_delta_s < baseline.cycle_time_s
            })
            .map(|(i, _)| i)
    }
}

/// Build an `OptimizeOutcome` from a baseline candidate and the
/// Stage-2 refined candidates. Tier dispatcher per the redesign plan:
///
///   - Empty candidates → `NoSafeImprovement` (no improvement found).
///   - At least one candidate is faster AND has no regression on any
///     gate (every delta Improved/Same/Unmodeled) → `Ranked`. Today's
///     auto-recommendation surface; UI can ⭐ the first safe entry.
///   - At least one candidate is faster AND improves a failing gate
///     while worsening a non-failing one → `TradeOff`. New tier — the
///     user has to explicitly accept the regression.
///   - Otherwise → `NoSafeImprovement`.
///
/// Every non-baseline candidate carries populated `gate_deltas` after
/// this function runs, so consumers don't have to recompute.
pub(crate) fn build_outcome(
    baseline: OptimizeCandidate,
    candidates: Vec<OptimizeCandidate>,
) -> OptimizeOutcome {
    if candidates.is_empty() {
        return OptimizeOutcome::NoSafeImprovement {
            reason: RefuseReason::NoImprovementFound,
            explanation: format!(
                "{}: no candidates were produced — operation has no geometry knobs and feed/RPM are at machine limits",
                RefuseReason::NoImprovementFound.explanation_for_optimize()
            ),
            attempted: vec![baseline],
        };
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

    // Pure improvement (Ranked tier): faster AND no regression.
    let any_pure_improvement = sorted
        .iter()
        .any(|c| c.gate_deltas.map(|d| d.no_regression()).unwrap_or(false) && is_faster(c));

    // Trade-off tier: faster AND improves something AND worsens
    // something. Pure improvements take priority — only land in
    // TradeOff if there's no Ranked candidate.
    let any_tradeoff = !any_pure_improvement
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
        return OptimizeOutcome::Ranked(ranked);
    }

    if any_tradeoff {
        let mut tradeoffs = Vec::with_capacity(sorted.len() + 1);
        tradeoffs.push(baseline);
        tradeoffs.extend(sorted);
        return OptimizeOutcome::TradeOff(tradeoffs);
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
    OptimizeOutcome::NoSafeImprovement {
        reason: RefuseReason::NoImprovementFound,
        explanation,
        attempted,
    }
}
