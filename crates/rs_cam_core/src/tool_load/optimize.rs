//! Tool-load optimize module — phase U1 of the optimizer plan.
//!
//! Search across feed/RPM/geometry params for a single toolpath and
//! rank candidates by measured cycle time. The simulator gate
//! (`tool_load::evaluate_toolpath`) is the single source of truth —
//! every number on every candidate came from a sim of that candidate's
//! params. There is no second chipload model.
//!
//! Three search stages, ordered cheapest-first:
//!
//! - **Stage 0** — closed-form analytical scaling of `(rpm, feed)` at
//!   constant chipload until a machine, tool, or LUT-row limit binds.
//!   No sim required. Headline "scale up to limits" win.
//! - **Stage 1** — for the 5 geometry ops (Adaptive3d, Pocket, Adaptive,
//!   Rest, Face), vary DOC anchored at the headroom point. 1mm dexel.
//! - **Stage 2** — top 3 by Stage-1 cycle time, re-simmed at default
//!   resolution (0.5mm). The reported cycle time and verdict on each
//!   candidate are always Stage-2 numbers.
//!
//! See `planning/OPTIMIZER_UX_PLAN.md` — particularly Resolutions 1-9
//! and Engineering Defaults 1-10.

use std::sync::atomic::AtomicBool;

use serde::{Deserialize, Serialize};

use crate::compute::catalog::OperationConfig;
use crate::session::ProjectSession;
use crate::simulation_cut::SimulationCutTrace;

use super::suggest::RefuseReason;
use super::verdict::{ToolpathLoadVerdict, Verdict};

/// Which search stage produced a candidate. The reported `cycle_time_s`
/// and `verdict` on every candidate the optimizer surfaces come from
/// Stage 2 (or directly from the baseline trace for the index-0
/// baseline candidate). Stage 0/1 candidates are intermediate and never
/// surface untouched.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchStage {
    /// The user's existing params, scored against `baseline_trace`.
    Baseline,
    /// Closed-form analytical RPM/feed headroom point. No sim run.
    Stage0Headroom,
    /// 1mm-dexel coarse sim, used for ranking geometry candidates.
    Coarse,
    /// Default-resolution sim, used for the survivors that get reported.
    Refined,
}

/// Human-readable diff between a candidate and the baseline. Each field
/// carries `Some(new_value)` only if the candidate is changing it.
/// Used by the modal to render "feed 1899→2100" style summaries and by
/// the Apply path to know which `feeds_auto.*` flags need clearing.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ParamDelta {
    /// New feed in mm/min. `None` if the candidate matches baseline feed.
    pub feed_mm_min: Option<f64>,
    /// New spindle RPM. `None` if the candidate is not changing RPM.
    pub spindle_rpm: Option<u32>,
    /// New radial stepover in mm.
    pub stepover_mm: Option<f64>,
    /// New depth-per-pass in mm.
    pub depth_per_pass_mm: Option<f64>,
}

impl ParamDelta {
    /// True if any field is `Some(_)` — i.e. the candidate is non-trivial.
    pub fn has_changes(&self) -> bool {
        self.feed_mm_min.is_some()
            || self.spindle_rpm.is_some()
            || self.stepover_mm.is_some()
            || self.depth_per_pass_mm.is_some()
    }
}

/// One candidate's full evaluation record. Populated by the optimizer
/// during Stage 0/1/2; each field is sim-measured (or, for the baseline
/// candidate at index 0, sourced from `baseline_trace`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizeCandidate {
    /// The full operation config that would be applied. Carries
    /// unchanged fields too, so Apply just needs to write this into
    /// `session.toolpath_configs[idx].operation`.
    pub params: OperationConfig,
    /// Diff from the baseline params, for display and feeds_auto flag
    /// management at Apply time.
    pub delta: ParamDelta,
    /// Measured cycle time from this candidate's sim (seconds).
    pub cycle_time_s: f64,
    /// Verdict from the gate over this candidate's sim.
    pub verdict: ToolpathLoadVerdict,
    /// Which stage produced this candidate.
    pub stage: SearchStage,
    /// Project-level reconciliation result (U4). After the user Applies
    /// and the project re-sims end-to-end, downstream stock-state
    /// changes can shift this candidate's verdict — that shifted value
    /// lands here. `None` until U4 fires.
    pub reconciled_cycle_time_s: Option<f64>,
    /// Reconciled verdict from the post-Apply project sim (U4).
    pub reconciled_verdict: Option<ToolpathLoadVerdict>,
}

/// Outcome of `optimize_toolpath` for one toolpath.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "detail", rename_all = "snake_case")]
pub enum OptimizeOutcome {
    /// At least one candidate was generated. Index 0 is always the
    /// baseline (current params). The recommendation is whichever
    /// candidate has `.first_safe()` returns — the first non-baseline
    /// candidate whose verdict is not `Exceeds` on any criterion.
    Ranked(Vec<OptimizeCandidate>),
    /// Every non-baseline candidate either failed the gate (Exceeds on
    /// some criterion) or was slower than baseline. The rollup row
    /// surfaces this with the binding-limit narrative.
    NoSafeImprovement {
        reason: RefuseReason,
        /// Narrative composed at outcome time via
        /// `RefuseReason::explanation_for_optimize` (Engineering
        /// Default 4). Free-form English for the modal.
        explanation: String,
    },
    /// The optimizer can't model this toolpath at all — drill cycles,
    /// project_curve with no steady-state samples, custom materials.
    /// The gate refuses, so the optimizer refuses.
    Skipped { reason: RefuseReason },
}

impl OptimizeOutcome {
    /// Recommended candidate: the first non-baseline candidate that
    /// passes the gate (no `Exceeds` verdict on any criterion). Returns
    /// `None` for `Skipped` / `NoSafeImprovement` outcomes, or for
    /// `Ranked` outcomes where every non-baseline candidate Exceeds.
    pub fn first_safe(&self) -> Option<&OptimizeCandidate> {
        let OptimizeOutcome::Ranked(candidates) = self else {
            return None;
        };
        candidates.iter().skip(1).find(|c| candidate_is_safe(c))
    }
}

/// True if every criterion is non-`Exceeds`. `Within` and `Unmodeled`
/// both pass; `Unmodeled` is the gate's honest "I don't know" and
/// shouldn't block a recommendation by itself.
pub(crate) fn candidate_is_safe(candidate: &OptimizeCandidate) -> bool {
    !matches!(candidate.verdict.chipload, Verdict::Exceeds { .. })
        && !matches!(candidate.verdict.power, Verdict::Exceeds { .. })
        && !matches!(candidate.verdict.deflection, Verdict::Exceeds { .. })
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

/// Optimize one toolpath and return the search outcome.
///
/// **Mutation note.** `session` is mutated transiently — each candidate
/// evaluation writes that candidate's params via
/// `apply_toolpath_param_snapshot`, regenerates the toolpath, and runs
/// a fresh project sim. An RAII baseline-restore guard re-applies the
/// original params on every exit path (Ok, NoSafeImprovement, Skipped,
/// cancelled, panicked candidate), so callers observe the session as
/// unchanged after this returns. Apply remains a separate user-initiated
/// mutation; the optimizer never persists candidate state.
///
/// **Cancellation.** `cancel` is polled between candidates and between
/// search stages. Mid-sim cancellation works through the simulator's
/// existing `&AtomicBool` plumbing, so cooperative cancellation lands
/// at the next sim sample boundary (sub-second).
///
/// `baseline_trace` should be the project sim already on screen — the
/// same trace the gate / diagnostics panel are reading. The baseline
/// candidate at index 0 of `Ranked` outcomes is scored against this
/// trace directly (no re-sim of baseline).
pub fn optimize_toolpath(
    _session: &mut ProjectSession,
    _baseline_trace: &SimulationCutTrace,
    _toolpath_index: usize,
    _cancel: &AtomicBool,
) -> OptimizeOutcome {
    // U1 skeleton — Stages 0/1/2 land in follow-up commits.
    OptimizeOutcome::Skipped {
        reason: RefuseReason::SimulationRequired,
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

    #[test]
    fn param_delta_has_changes_detects_any_field() {
        assert!(!ParamDelta::default().has_changes());
        assert!(
            ParamDelta {
                feed_mm_min: Some(2100.0),
                ..Default::default()
            }
            .has_changes()
        );
        assert!(
            ParamDelta {
                stepover_mm: Some(0.8),
                ..Default::default()
            }
            .has_changes()
        );
    }

    #[test]
    fn first_safe_skips_index_zero_baseline() {
        // Skipped/NoSafeImprovement outcomes never recommend.
        let skipped = OptimizeOutcome::Skipped {
            reason: RefuseReason::SimulationRequired,
        };
        assert!(skipped.first_safe().is_none());

        let nsi = OptimizeOutcome::NoSafeImprovement {
            reason: RefuseReason::NoFeasibleRow,
            explanation: "test".to_owned(),
        };
        assert!(nsi.first_safe().is_none());
    }
}
