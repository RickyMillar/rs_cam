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

use crate::ids::ToolpathId;
use std::sync::LazyLock;
use std::sync::atomic::AtomicBool;

use serde::{Deserialize, Serialize};

use crate::compute::catalog::OperationConfig;
#[cfg(test)]
use crate::compute::catalog::OperationType;
use crate::feeds::vendor_lookup::MatchedRow;
use crate::machine::MachineProfile;
use crate::session::ProjectSession;
use crate::stock::simulation_cut::SimulationCutTrace;

use super::RefuseReason;
use super::verdict::{ChiploadVerdict, ToolpathLoadVerdict};
use super::{ToleranceBands, ToolpathLoadContext, evaluate_toolpath};

pub mod axes;
pub mod bounds;
mod candidate;
mod context;
mod delta;
mod narrative;
mod outcome;
pub mod patches;
mod policy;
mod preflight;
pub mod progress;
mod rank;
mod refusal;
pub mod retarget;
/// A-8 (F-OPT) — the first fixture that measures a REAL retarget outcome
/// end to end (sim-produced verdict → real retargeter → re-sim). In `src/`
/// because the retarget stage is assembled from `pub(crate)` parts that a
/// `tests/` file cannot reach without reproducing them.
#[cfg(test)]
mod retarget_reconciliation_a8;
pub mod space;
pub mod strategy;

pub use candidate::OptimizeCandidate;
pub(crate) use candidate::{
    evaluate_candidate, finalize_partial, refine_stage2, select_stage2_candidates,
};
pub(crate) use delta::delta_against_baseline;
pub use delta::{GateDelta, GateDeltas, ParamDelta};
pub use narrative::{
    AxisExtent, EntryAdvisory, GateKind, KnobAxis, LimitingGate, OperatorSuggestion,
    OutcomeNarrative, SearchEnvelopeReached, entry_advisories_for_verdict,
    limiting_gates_for_verdict, suggest_levers,
};
pub(crate) use outcome::build_outcome;
pub use outcome::{
    BaselineTraceAssumptions, CandidateSimAssumptions, KinematicsSource, LutQueryStamp,
    MachineSnapshot, OptimizeOutcome, OutcomeKind, ProjectOptimizeReport, SimAssumptionStamp,
};
pub use progress::{OptimizeProgress, OptimizeProgressSnapshot, SearchPhase};

use context::{
    BaselineRestoreGuard, EvaluationContext, air_cut_fraction_of_total_runtime_from_trace,
    baseline_rpm_from_trace, cycle_time_from_trace, find_matched_lut_row,
};
use policy::SearchPolicy;

static DEFAULT_SEARCH_POLICY: LazyLock<SearchPolicy> = LazyLock::new(SearchPolicy::default);

fn search_policy() -> &'static SearchPolicy {
    &DEFAULT_SEARCH_POLICY
}

/// Build the gate-trigger tolerance bands from the active `SearchPolicy`'s
/// ranking section. Centralises the policy → `ToleranceBands` mapping so
/// every optimizer call to `evaluate_toolpath` widens the gates the same
/// way (Layer 1 of `planning/OPTIMIZER_REFACTOR_G16.md` §11).
pub(crate) fn tolerance_bands_from_policy(policy: &SearchPolicy) -> ToleranceBands {
    ToleranceBands {
        breakage: policy.ranking.breakage_tolerance.value,
        burn: policy.ranking.burn_tolerance.value,
        power_breach: policy.ranking.power_breach_tolerance.value,
        deflection_breach: policy.ranking.deflection_breach_tolerance.value,
    }
}

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

/// Optimize one toolpath and return the search outcome.
///
/// **Mutation note.** `session` is mutated transiently — each candidate
/// evaluation writes that candidate's params via
/// `apply_toolpath_param_snapshot_narrow`, regenerates the toolpath, and runs
/// a fresh project sim. An RAII baseline-restore guard re-applies the
/// original params on every exit path (Ok, NoSafeImprovement, Skipped,
/// cancelled, panicked candidate). Apply remains a separate
/// user-initiated mutation; the optimizer never persists candidate state.
///
/// **The guard restores the PARAMETERS and only the parameters.** This
/// note used to end "so callers observe the session as unchanged after
/// this returns", which is false: `apply_toolpath_param_snapshot_narrow`
/// ends with `drop_result(index)` and `session.simulation = None`, so the
/// toolpath's cached result is gone, its generation-input revision has
/// moved, and the project simulation slot is empty. Measured by
/// `crates/rs_cam_core/tests/optimize_toolpath_is_a_job_wp14b.rs`.
///
/// Since WP14b no production caller hands this function a LIVE session.
/// The per-toolpath route takes the `optimize_toolpath` `Job` row, whose
/// handle owns a clone; the project rollup passes `session.clone()` to
/// its lane. The residue therefore lands on a copy, and the session on
/// screen keeps all three.
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
    session: &mut ProjectSession,
    baseline_trace: &SimulationCutTrace,
    toolpath_index: usize,
    cancel: &AtomicBool,
) -> OptimizeOutcome {
    let progress = std::sync::Arc::new(OptimizeProgress::default());
    optimize_toolpath_observed(session, baseline_trace, toolpath_index, cancel, &progress)
}

/// Optimize one toolpath and publish the search's progress as it runs
/// (WP29).
///
/// Same search as [`optimize_toolpath`], same outcome, same mutation
/// note. The one difference is `progress`: the search announces each rung
/// of its three-rung ladder on it, and ticks once per candidate at the top
/// of each candidate loop. The caller reads it from another thread through
/// [`OptimizeProgress::snapshot`].
///
/// [`optimize_toolpath`] delegates here with a silent sink, so the machine
/// and assumption stamps stay in ONE place.
///
/// The progress is cleared on every exit path, because
/// [`OptimizeProgress::finish`] runs after the inner call
/// returns. The per-rung counters stand after that, so a settled run stays
/// readable.
pub fn optimize_toolpath_observed(
    session: &mut ProjectSession,
    baseline_trace: &SimulationCutTrace,
    toolpath_index: usize,
    cancel: &AtomicBool,
    progress: &std::sync::Arc<OptimizeProgress>,
) -> OptimizeOutcome {
    // F4.3 — stamp the machine caps this run consumed on every outcome
    // (including refusals/skips), so narratives stay reconcilable with
    // the profile that actually bounded the search.
    let snapshot = outcome::MachineSnapshot::of(session.machine());
    // A-8 (F-OPT) — stamp the simulation assumptions the run's numbers
    // are taken at, on every outcome including refusals, for the same
    // reason F4.3 stamps the machine. Observed BEFORE the search so a
    // `Skipped` outcome that never simulated still names its operating
    // point, and so the observation cannot be taken from a session the
    // search has transiently mutated.
    let assumptions = outcome::SimAssumptionStamp::of(session, toolpath_index, baseline_trace);
    let mut outcome =
        optimize_toolpath_inner(session, baseline_trace, toolpath_index, cancel, progress);
    // WP29 — one clear site for every exit path of the inner search,
    // including each early return and each refusal.
    progress.finish();
    outcome.machine_snapshot = Some(snapshot);
    outcome.assumptions = Some(assumptions);
    outcome
}

fn optimize_toolpath_inner(
    session: &mut ProjectSession,
    baseline_trace: &SimulationCutTrace,
    toolpath_index: usize,
    cancel: &AtomicBool,
    progress: &std::sync::Arc<OptimizeProgress>,
) -> OptimizeOutcome {
    use std::sync::atomic::Ordering;

    // 1. Build the evaluation context. Skip cleanly if the toolpath or
    //    its tool is missing.
    let Some(ctx) = EvaluationContext::from_session(session, toolpath_index)
        .map(|ctx| ctx.with_progress(std::sync::Arc::clone(progress)))
    else {
        return OptimizeOutcome::skipped(RefuseReason::SimulationRequired);
    };

    // 2. Skip op kinds the gate can't model. Drill cycles are pure
    //    plunge — no chipload measurement to compare against.
    if ctx.operation_kind.is_drill_kinematics() {
        return OptimizeOutcome::skipped(RefuseReason::SteadyStateSamplesNotPresent);
    }

    // 3. Skip Custom material — Kc is unvalidated, power model
    //    unreliable.
    if matches!(ctx.material, crate::material::Material::Custom { .. }) {
        return OptimizeOutcome::skipped(RefuseReason::MaterialUnvalidated);
    }

    // 4. Build the baseline candidate from the existing trace. Score
    //    via the same gate that diagnostics already used so the
    //    rollup's index-0 row matches what the user already sees.
    let baseline_op = match session.get_toolpath_config(toolpath_index) {
        Some(tc) => tc.operation.clone(),
        None => {
            return OptimizeOutcome::skipped(RefuseReason::SimulationRequired);
        }
    };
    // D6/D7: pull spans for the current toolpath out of the cached
    // compute result. None when generation hasn't been run; locality
    // classifier degrades to engagement-only labels in that case.
    // F2.2: also None when a transform invalidated the spans (TSP
    // split) — corrupted ancestry must not stamp gate verdicts.
    let baseline_spans: Option<&[crate::trace::toolpath_spans::Span]> = session
        .get_result(toolpath_index)
        .filter(|r| r.annotated().spans_valid)
        .map(|r| r.annotated().spans.as_slice());
    let baseline_drill_op = session
        .get_result(toolpath_index)
        .and_then(|r| r.drill_op());
    let baseline_load_ctx = ToolpathLoadContext {
        toolpath_id: ctx.toolpath_id,
        tool: &ctx.tool,
        material: &ctx.material,
        operation_family: ctx.lut_op_family,
        pass_role: ctx.lut_pass_role,
        operation_feed_rate_mm_min: baseline_op.feed_rate(),
        operation_kind: ctx.operation_kind,
        spans: baseline_spans,
        drill_op: baseline_drill_op.map(|arc| arc.as_ref()),
    };
    let machine = session.machine().clone();
    let policy_tolerance = tolerance_bands_from_policy(search_policy());
    let baseline_verdict = evaluate_toolpath(
        &baseline_load_ctx,
        Some(baseline_trace),
        Some(&machine),
        &policy_tolerance,
    );
    let baseline_cycle_s = match cycle_time_from_trace(baseline_trace, ctx.toolpath_id) {
        Some(t) if t > 0.0 => t,
        _ => {
            return OptimizeOutcome::skipped(RefuseReason::SteadyStateSamplesNotPresent);
        }
    };
    let baseline_air_cut =
        air_cut_fraction_of_total_runtime_from_trace(baseline_trace, ctx.toolpath_id);
    let baseline_candidate = OptimizeCandidate {
        params: baseline_op.clone(),
        delta: ParamDelta::default(),
        cycle_time_s: baseline_cycle_s,
        verdict: baseline_verdict.clone(),
        stage: SearchStage::Baseline,
        reconciled_cycle_time_s: None,
        reconciled_verdict: None,
        gate_deltas: None,
        air_cut_fraction_of_total_runtime: baseline_air_cut,
    };

    // 5. Look up the matched LUT row. Used by Stage 0's `k_lut` bound
    //    and Stage 1's DOC-grid endpoint clamping. The commanded DOC
    //    drives engaged-diameter selection for tapered tools — at
    //    shallow DOC a tapered ball engages a much smaller diameter
    //    than its shank, and the LUT row that fits the engaged tool is
    //    not the same row that fits the shank.
    let matched_lut_row =
        find_matched_lut_row(&ctx.tool, &ctx.material, &ctx, baseline_op.depth_per_pass());

    // 5b. Pre-flight: classify the baseline against the gates before
    //     burning any sims. If the failing axis isn't in the
    //     optimizer's search space (deflection over the bound even at
    //     the minimum-force corner, bipolar chipload), refuse
    //     immediately with an op-aware prescription rather than
    //     running stages that can't move the failing gate.
    if let Some(refusal) = preflight::preflight_classify(
        &ctx,
        &baseline_op,
        &machine,
        baseline_trace,
        baseline_op.feed_rate(),
        &baseline_verdict,
        matched_lut_row.as_ref(),
    ) {
        let narrative = OutcomeNarrative {
            explanation: refusal.explanation,
            deflection_setup: refusal.deflection_setup,
            ..OutcomeNarrative::default()
        };
        return OptimizeOutcome::no_safe_improvement(
            vec![baseline_candidate],
            refusal.reason,
            narrative,
        );
    }

    // Cancel check before any sims.
    if cancel.load(Ordering::SeqCst) {
        let narrative = OutcomeNarrative {
            explanation: "cancelled before any candidates were generated".to_owned(),
            ..OutcomeNarrative::default()
        };
        return OptimizeOutcome::no_safe_improvement(
            vec![baseline_candidate],
            RefuseReason::NoImprovementFound,
            narrative,
        );
    }

    // 6. From here on the session is mutated per-candidate. The
    //    BaselineRestoreGuard restores `(operation, dressups,
    //    face_selection)` on drop, regardless of how
    //    we exit (early return, Err, panic).
    let Ok(mut guard) = BaselineRestoreGuard::new(session, toolpath_index) else {
        return OptimizeOutcome::skipped(RefuseReason::SimulationRequired);
    };

    let baseline_rpm = baseline_rpm_from_trace(
        baseline_trace,
        ctx.toolpath_id,
        baseline_op.spindle_rpm(),
        &machine,
    );

    // 7. Stage F: closed-form feed/RPM solve. Two modes:
    //    - All load gates Within → headroom scale-up (Stage 0).
    //    - ANY load gate Exceeds (chipload single-side Burn/Breakage —
    //      pre-flight refused bipolar already — power overuse, or
    //      reachable deflection) → per-gate retargeters. F2.3: pre-F2
    //      only chipload-Exceeds dispatched here, which left
    //      `DeflectionDocRetargeter` dead code and starved
    //      `PowerFeedRetargeter` on power-only-Exceeds baselines —
    //      worse, a power/deflection-Exceeds baseline with Within
    //      chipload ran the headroom SCALE-UP instead.
    let mut all_candidates: Vec<OptimizeCandidate> = Vec::new();
    // Q-NARROW (c): a retargeter can now decline with a typed reason
    // instead of emitting a candidate its own gate has already rejected.
    // The refusals travel alongside the candidates and are attached to
    // whatever outcome this run ends up producing — including the cancel
    // paths, so a partial result still says what was declined.
    let mut retarget_refusals: Vec<retarget::RetargetRefusal> = Vec::new();
    // WP29 — the ORCHESTRATOR announces rung 1, not the two Stage-F
    // helpers. An `Unmodeled` chipload with no exceeding load gate runs
    // NEITHER mode, so no helper can announce the rung and the window's
    // first row would never tick. Both helpers also return early on a
    // non-optimizable surface, and this zero-total announce covers that
    // path too. Each helper re-announces its own real total once the
    // candidate list exists.
    ctx.progress.begin_phase(SearchPhase::FeedRpm, 0);
    if any_load_gate_exceeds(&baseline_verdict) {
        let staged = run_retarget_strategy(
            &mut guard,
            &ctx,
            &baseline_op,
            baseline_rpm,
            &baseline_verdict,
            matched_lut_row.as_ref(),
            &machine,
            cancel,
        );
        all_candidates.extend(staged.candidates);
        retarget_refusals.extend(staged.refusals);
    } else if matches!(baseline_verdict.chipload, ChiploadVerdict::Within { .. })
        && let Some(c) = run_headroom_strategy(
            &mut guard,
            &ctx,
            &baseline_op,
            baseline_rpm,
            &baseline_verdict,
            matched_lut_row.as_ref(),
            &machine,
            cancel,
        )
    {
        all_candidates.push(c);
    }

    if cancel.load(Ordering::SeqCst) {
        drop(guard);
        return attach_retarget_refusals(
            finalize_partial(baseline_candidate, all_candidates, &machine),
            &retarget_refusals,
        );
    }

    // 8. Axis-grid strategy: joint DOC × stepover × scallop_height
    //    variant grid, anchored on the headroom candidate's params
    //    (when stage F fired) or baseline (when it didn't).
    let stage_1_candidates = run_grid_strategy(
        &mut guard,
        &ctx,
        &baseline_op,
        &baseline_verdict,
        all_candidates.first(),
        matched_lut_row.as_ref(),
        cancel,
    );
    all_candidates.extend(stage_1_candidates);

    if cancel.load(Ordering::SeqCst) {
        drop(guard);
        return attach_retarget_refusals(
            finalize_partial(baseline_candidate, all_candidates, &machine),
            &retarget_refusals,
        );
    }

    // 9. Stage 2: top-N by composite_score, re-eval at full resolution.
    //    G16 §11 layer 2b — score weights cycle savings against
    //    chipload-distance / power-overuse / deflection-overuse penalties.
    let stage2_survivor_count = search_policy().stages.refined_survivor_count.value;
    let stage2_seeds = select_stage2_candidates(
        all_candidates,
        &baseline_candidate,
        search_policy(),
        stage2_survivor_count,
    );
    let Ok(stage2_candidates) = refine_stage2(&mut guard, &ctx, stage2_seeds, cancel) else {
        drop(guard);
        let narrative = OutcomeNarrative {
            explanation: "candidate evaluation failed at full resolution — partial result returned"
                .to_owned(),
            ..OutcomeNarrative::default()
        };
        return attach_retarget_refusals(
            OptimizeOutcome::no_safe_improvement(
                vec![baseline_candidate],
                RefuseReason::NoImprovementFound,
                narrative,
            ),
            &retarget_refusals,
        );
    };

    // 10. Drop the guard explicitly so the baseline is restored before
    //     building the outcome (which references the candidates,
    //     not the session). The outcome is returned to the caller; the
    //     caller's view of `session` is now back at the baseline.
    drop(guard);
    attach_retarget_refusals(
        build_outcome(baseline_candidate, stage2_candidates, &machine),
        &retarget_refusals,
    )
}

/// **Q-NARROW (c), 2026-08-14.** Fold the typed retarget refusals this
/// run produced into the outcome it ended up with.
///
/// Three separable effects, deliberately not one:
///
/// 1. **The structured record always lands** on
///    [`OutcomeNarrative::chipload_band_refusal`]. A refused retarget is
///    a fact about the run regardless of what the rest of the search
///    found — a grid candidate can still win, and both things are true.
/// 2. **The reason is replaced only on a `NoSafeImprovement` outcome
///    still carrying the generic `NoImprovementFound`.** That reason
///    claims "no candidate was both faster and safe", i.e. that a search
///    ran and came back empty. When the retarget was declined before it
///    ran, the typed reason is the honest one. A reason set by some
///    other classifier (pre-flight `DeflectionSetupLocked`,
///    `BipolarEngagement`) is left alone — it is closer to the cause.
/// 3. **The headline is replaced only when nothing non-baseline was
///    attempted.** `headline_no_safe` says "No candidates were produced
///    — the search space is empty for this op" at zero attempts, which
///    the refusal falsifies: the space was not empty, the target inside
///    it was unreachable. With attempts on the board the existing
///    headline describes them accurately and is kept.
fn attach_retarget_refusals(
    mut outcome: OptimizeOutcome,
    refusals: &[retarget::RetargetRefusal],
) -> OptimizeOutcome {
    let Some(first) = refusals.first() else {
        return outcome;
    };
    let retarget::RetargetRefusal::ChiploadBandNarrowerThanHeadroom(detail) = first;
    outcome.narrative.chipload_band_refusal = Some(detail.clone());

    if outcome.kind == OutcomeKind::NoSafeImprovement
        && matches!(
            outcome.reason,
            None | Some(RefuseReason::NoImprovementFound)
        )
    {
        outcome.reason = Some(first.reason());
        let explanation = first.explanation();
        outcome.narrative.explanation = if outcome.narrative.explanation.is_empty() {
            explanation
        } else {
            format!("{explanation}. {}", outcome.narrative.explanation)
        };
        if outcome.candidates.len() <= 1 {
            outcome.narrative.headline = NARROW_BAND_HEADLINE.to_owned();
        }
    }
    outcome
}

/// Headline for a narrow-band refusal at zero non-baseline attempts.
///
/// It exists to displace `headline_no_safe`'s zero-attempt sentence
/// ("No candidates were produced — the search space is empty for this
/// op"), which the refusal falsifies: the space was not empty, the
/// target inside it was unreachable.
///
/// **G-EXPL-HIDDEN, 2026-08-16** — it used to carry the band values
/// too, because the modal rendered `headline` and never `explanation`,
/// so this was the only line that reached the operator. The modal now
/// renders both, and `RetargetRefusal::explanation` states the band,
/// the refused target and the dial responsible. Repeating the band
/// here would print it twice in one card, so the headline is trimmed
/// back to the claim only it makes — that a refusal happened, not a
/// failed search. The numbers live on `explanation` and on the
/// structured `narrative.chipload_band_refusal`, both of which land on
/// the same outcome.
const NARROW_BAND_HEADLINE: &str = "No candidate proposed — the chipload retarget was refused: \
     the vendor band the gate judges by is narrower than the retarget headroom.";

/// Run Stage 0 (analytical RPM/feed headroom scale-up) for one
/// toolpath. Returns the headroom candidate, or `None` if the baseline
/// already trips the chipload gate (proportional scaling preserves
/// chipload, so it can't fix `Exceeds`), if the closed-form solver
/// finds no headroom (`k ≤ 1`), or if the candidate sim fails.
///
/// Run the headroom-scale strategy against the baseline and return
/// the (at most one) candidate it produces. The strategy emits
/// `CandidatePatch`es; this wrapper applies them to the baseline op via
/// `apply_patches_to_op` and runs the per-candidate sim.
// SAFETY: the strategy needs the guard, the context, the baseline op,
// its RPM, its verdict, the matched LUT row, the machine and the cancel
// flag; no subset builds a candidate.
#[allow(clippy::too_many_arguments)]
fn run_headroom_strategy(
    guard: &mut BaselineRestoreGuard<'_>,
    ctx: &EvaluationContext,
    baseline_op: &OperationConfig,
    baseline_rpm: f64,
    baseline_verdict: &ToolpathLoadVerdict,
    matched_lut_row: Option<&MatchedRow>,
    machine: &MachineProfile,
    cancel: &AtomicBool,
) -> Option<OptimizeCandidate> {
    use crate::compute::catalog::OptimizationSurface;
    use strategy::OptimizationStrategy;
    use strategy::headroom::HeadroomScaleStrategy;

    // The strategy operates on an AxisView; non-optimizable ops are
    // skipped here just like the rest of the search machinery. (The
    // orchestrator already excludes Drill / AlignmentPinDrill earlier.)
    let view = match baseline_op.optimization_surface() {
        OptimizationSurface::Optimizable(v) => v,
        OptimizationSurface::NotOptimizable { .. } => return None,
    };

    let policy = search_policy();
    let strat = HeadroomScaleStrategy {
        machine,
        lut_row: matched_lut_row,
        baseline_rpm,
        policy,
    };

    let mut candidates = strat.candidates(&view, baseline_verdict);
    let cp = candidates.pop()?; // strategy emits at most one candidate.

    // WP29 — the real Stage-F total, over the orchestrator's zero.
    ctx.progress.begin_phase(SearchPhase::FeedRpm, 1);
    ctx.progress.begin_candidate(0);

    let candidate_op = patches::apply_patches_to_op(baseline_op, &cp.patches).ok()?;
    let delta = delta_against_baseline(baseline_op, &candidate_op);
    evaluate_candidate(
        guard,
        ctx,
        candidate_op,
        delta,
        SearchStage::Coarse,
        policy.stages.coarse_resolution_mm.value,
        cancel,
    )
    .ok()
}

/// Run the [`PerGateRetargetStrategy`] against a baseline whose
/// chipload gate is `Exceeds`. For each load-driving gate that's also
/// Exceeds, the strategy emits one [`CandidatePatch`]; this wrapper
/// applies each via `apply_patches_to_op` and runs the per-candidate
/// sim.
///
/// **Behaviour change.** The legacy chipload retarget produced a
/// `commanded × RCTF` solve that lowered feed on `BurnRisk`. The new
/// chipload retargeter is sample-driven (`target_chipload /
/// observed_peak`), so `BurnRisk` raises feed. Wanaka TP 4 (feed=3150,
/// peak=0.0253, LUT [0.038, 0.07], 1.20× headroom) now produces a
/// feed-up candidate that clamps at 5000 mm/min — the previous Stage F
/// produced a feed-down one.
/// F2.3 — Stage F dispatch predicate: the per-gate retarget strategy
/// runs when ANY load gate is `Exceeds`. Pre-F2.3 only
/// chipload-Exceeds dispatched, which left `DeflectionDocRetargeter`
/// dead code, starved `PowerFeedRetargeter` on power-only-Exceeds
/// baselines, and routed those baselines to the headroom SCALE-UP
/// instead.
fn any_load_gate_exceeds(v: &ToolpathLoadVerdict) -> bool {
    use crate::tool_load::verdict::{DeflectionVerdict, PowerVerdict};
    matches!(v.chipload, ChiploadVerdict::Exceeds { .. })
        || matches!(v.power, PowerVerdict::Exceeds { .. })
        || matches!(v.deflection, DeflectionVerdict::Exceeds { .. })
}

// SAFETY: the strategy needs the guard, the context, the baseline op,
// its RPM, its verdict, the matched LUT row, the machine and the cancel
// flag; no subset builds a candidate.
#[allow(clippy::too_many_arguments)]
fn run_retarget_strategy(
    guard: &mut BaselineRestoreGuard<'_>,
    ctx: &EvaluationContext,
    baseline_op: &OperationConfig,
    baseline_rpm: f64,
    baseline_verdict: &ToolpathLoadVerdict,
    matched_lut_row: Option<&MatchedRow>,
    machine: &MachineProfile,
    cancel: &AtomicBool,
) -> RetargetStageOutput {
    use crate::compute::catalog::OptimizationSurface;
    use std::sync::atomic::Ordering;
    use strategy::retarget::PerGateRetargetStrategy;

    let view = match baseline_op.optimization_surface() {
        OptimizationSurface::Optimizable(v) => v,
        OptimizationSurface::NotOptimizable { .. } => return RetargetStageOutput::default(),
    };

    let policy = search_policy();
    let axis_ctx = axes::AxisContext {
        // No project-default-RPM accessor on session today; 18_000 matches
        // the gcode pipeline's hard-coded fallback. Step 8 should plumb a
        // real default through `EvaluationContext`.
        project_default_rpm: 18_000,
        machine,
        tool: &ctx.tool,
        material: &ctx.material,
    };
    let space = space::SearchSpace::build(&view, &axis_ctx, matched_lut_row, policy);

    // Checkpoint P (1a): the row is consulted only to decide whether a
    // chipload retarget is possible at all (a row publishing neither bound
    // cannot produce one). The *band* the retargeter aims at is no longer
    // injected here — it is read off the verdict's `ChiploadMetric::bounds`,
    // which is DOC-derated, so the retargeter and the gate can no longer hold
    // two different numbers. See `retarget::chipload`'s module docs.
    let chipload = matched_lut_row.and_then(|row| {
        if row.chip_load_min_mm.is_none() && row.chip_load_max_mm.is_none() {
            return None;
        }
        Some(retarget::chipload::ChiploadFeedRetargeter {
            low_headroom: policy.retarget.chipload_low_headroom.value,
            high_headroom: policy.retarget.chipload_high_headroom.value,
            plunge_tracking_threshold: policy.feed.plunge_tracking_threshold_fraction.value,
        })
    });

    // The rated spindle curve at the baseline RPM, with no fraction
    // (ruling R4 Q2, 2026-09-24): the same ceiling the power gate reads.
    let available_kw = machine.power_at_rpm(baseline_rpm);
    let power = retarget::power::PowerFeedRetargeter {
        available_kw,
        headroom: policy.retarget.power_headroom.value,
        plunge_tracking_threshold: policy.feed.plunge_tracking_threshold_fraction.value,
    };

    let deflection = retarget::deflection::DeflectionDocRetargeter::with_headroom(
        super::deflection::EXCEEDS_BOUND_MM,
        policy.retarget.deflection_headroom.value,
    );

    let strat = PerGateRetargetStrategy {
        chipload,
        power,
        deflection,
        space: &space,
        ctx: &axis_ctx,
    };
    // Q-NARROW (c): both halves of the strategy's answer come back from
    // ONE call — the refusals are decided by the same arithmetic that
    // would have produced the candidates, so asking twice would put two
    // instruments on one comparison.
    let staged = strat.candidates_and_refusals(&view, baseline_verdict);

    let mut out = RetargetStageOutput {
        candidates: Vec::new(),
        refusals: staged.refusals,
    };
    // WP29 — read the length BEFORE the loop consumes the `Vec`, and
    // announce the real Stage-F total over the orchestrator's zero.
    let stage_f_total = staged.candidates.len();
    ctx.progress
        .begin_phase(SearchPhase::FeedRpm, stage_f_total);
    for (index, cp) in staged.candidates.into_iter().enumerate() {
        if cancel.load(Ordering::SeqCst) {
            break;
        }
        // The tick sits HERE — after the cancel break and before the
        // `continue` below. A tick inside `evaluate_candidate` would miss
        // every candidate this arm skips, and the row would then freeze
        // short of the total for the rest of the rung.
        ctx.progress.begin_candidate(index);
        let Ok(candidate_op) = patches::apply_patches_to_op(baseline_op, &cp.patches) else {
            continue;
        };
        let delta = delta_against_baseline(baseline_op, &candidate_op);
        if let Ok(candidate) = evaluate_candidate(
            guard,
            ctx,
            candidate_op,
            delta,
            SearchStage::Coarse,
            policy.stages.coarse_resolution_mm.value,
            cancel,
        ) {
            out.candidates.push(candidate);
        }
    }
    out
}

/// What the retarget stage produced: evaluated candidates plus the
/// typed refusals the retargeters returned instead of candidates
/// (Q-NARROW (c)). Distinct from
/// [`strategy::retarget::RetargetStrategyOutput`], whose candidates are
/// un-simulated patch lists.
#[derive(Debug, Default)]
struct RetargetStageOutput {
    candidates: Vec<OptimizeCandidate>,
    refusals: Vec<retarget::RetargetRefusal>,
}

/// Run the [`AxisGridStrategy`] against the baseline (or the headroom
/// candidate, when stage F fired) and evaluate every emitted cell. The
/// anchor, dedup and variant logic live in the strategy module.
fn run_grid_strategy(
    guard: &mut BaselineRestoreGuard<'_>,
    ctx: &EvaluationContext,
    baseline_op: &OperationConfig,
    baseline_verdict: &ToolpathLoadVerdict,
    stage0_anchor: Option<&OptimizeCandidate>,
    matched_lut_row: Option<&MatchedRow>,
    cancel: &AtomicBool,
) -> Vec<OptimizeCandidate> {
    use crate::compute::catalog::OptimizationSurface;
    use std::sync::atomic::Ordering;
    use strategy::OptimizationStrategy;
    use strategy::grid::AxisGridStrategy;

    let anchor_op = stage0_anchor
        .map(|c| c.params.clone())
        .unwrap_or_else(|| baseline_op.clone());

    // Strategy needs a baseline view for trait conformance; it's
    // unused inside the grid path (anchor-relative by design) but
    // we still produce one to satisfy the contract.
    let baseline_view = match baseline_op.optimization_surface() {
        OptimizationSurface::Optimizable(v) => v,
        OptimizationSurface::NotOptimizable { .. } => return Vec::new(),
    };

    let policy = search_policy();
    let strat = AxisGridStrategy {
        anchor_op: &anchor_op,
        lut_row: matched_lut_row,
        op_type: ctx.operation_kind,
        policy,
    };
    let cps = strat.candidates(&baseline_view, baseline_verdict);

    // WP29 — the grid is the big rung. Read the length before the loop
    // consumes the `Vec`; the strategy forms the whole cross product in
    // one call, so the total is honest the moment the rung starts.
    let grid_total = cps.len();
    ctx.progress.begin_phase(SearchPhase::AxisGrid, grid_total);

    let mut out: Vec<OptimizeCandidate> = Vec::new();
    for (index, cp) in cps.into_iter().enumerate() {
        if cancel.load(Ordering::SeqCst) {
            break;
        }
        // The tick sits HERE, for the reason `run_retarget_strategy`
        // records: the `continue` below skips a candidate the evaluator
        // never sees.
        ctx.progress.begin_candidate(index);
        let Ok(candidate_op) = patches::apply_patches_to_op(&anchor_op, &cp.patches) else {
            continue;
        };
        let delta = delta_against_baseline(baseline_op, &candidate_op);
        if let Ok(candidate) = evaluate_candidate(
            guard,
            ctx,
            candidate_op,
            delta,
            SearchStage::Coarse,
            policy.stages.coarse_resolution_mm.value,
            cancel,
        ) {
            out.push(candidate);
        }
    }
    out
}

// ── Pre-flight classifier (HIGH-1, HIGH-3, HIGH-5 from the redesign) ──
//
// Before any sims fire, classify the baseline against each gate:
//
//   - Deflection `Exceeds` AND the closed-form model still over the
//     bound at the search space's minimum-force corner (F2.3) →
//     tool/material/stickout-driven failure the levers can't reach.
//     Refuse with `DeflectionSetupLocked`. Reachable corners proceed
//     to the per-gate retarget strategy instead.
//   - Bipolar chipload (steady-state samples straddle both `cl_min`
//     and `cl_max`) → no single feed/RPM scaling fixes both
//     extremes. Refuse with `BipolarEngagement`.
//
// Both refusals carry an op-aware prescription string. The shape is
// "<diagnostic> — <lever>", where the diagnostic explains *what* is
// wrong and the lever points at a knob the user has access to.

// ── Project-level rollup (U3) ─────────────────────────────────────────
//
// `optimize_project` walks every enabled toolpath in order and runs
// `optimize_toolpath` for each, producing a `ProjectOptimizeReport`
// for the U3 rollup view. The walk is sequential — `optimize_toolpath`
// holds `&mut session` and runs full project sims internally; rayon
// would need each worker to clone the whole session.
//
// WP14b (2026-09-13) made `ProjectSession` `Clone`, so that is now
// possible rather than blocked. It is still not done here: a clone per
// worker copies the simulation display mesh per worker, and the walk's
// own stock-state hygiene below reads one session in order. The note
// used to say `ToolpathConfig` is not `Clone` and a cloneable session
// would need a wide-touch refactor; both halves are stale.
//
// **Stock-state hygiene between toolpaths.** `optimize_toolpath`'s
// `BaselineRestoreGuard` restores the toolpath's params on drop via
// `apply_toolpath_param_snapshot_narrow`, which invalidates that toolpath's
// cached `result`. After each call, the project's per-toolpath stock
// state would be incomplete (the just-optimized TP has no result, so
// subsequent project sims would skip it). We re-generate the toolpath
// at baseline params after each `optimize_toolpath` call to keep the
// project's results map populated for the rest of the walk.

/// Progress callback for `optimize_project`. The worker-thread lane
/// implements this to mirror progress into `LaneSnapshot::current_phase`
/// for the modal's progress strip; tests use [`NoProgress`].
///
/// `report` is called once per toolpath at the start of its
/// optimization. `completed` is the count of toolpaths fully done
/// **before** this one; `total` is the count of toolpaths the run will
/// touch (skipped toolpaths counted). `label` is a human-readable hint
/// like `"TP 3 / 7 — Lakes back"`.
pub trait ProgressReporter: Send + Sync {
    fn report(&self, completed: usize, total: usize, label: &str);
}

/// No-op progress reporter for tests and CLI flows that don't need the
/// progress strip.
pub struct NoProgress;

impl ProgressReporter for NoProgress {
    fn report(&self, _completed: usize, _total: usize, _label: &str) {}
}

/// Optimize every enabled toolpath in the project and return a rollup.
///
/// **Sequential.** Each toolpath's `optimize_toolpath` call is run
/// in order — they share `&mut session` and run full project sims
/// internally. After each call, the just-optimized toolpath is
/// re-generated at baseline params so subsequent toolpaths see a
/// fully-populated project stock state.
///
/// **Cancellation.** `cancel` is checked between toolpaths and is
/// also forwarded to each `optimize_toolpath` call (which polls it
/// between candidates). A cancel mid-walk produces a partial report
/// containing every toolpath that completed before the cancel.
///
/// **Baseline cycle time** comes from `baseline_trace.toolpath_summaries`
/// — the same trace the gate read from. The bottleneck index is the
/// toolpath whose baseline cycle exceeds the policy bottleneck fraction
/// of total project runtime, breaking ties by the largest cycle (so only
/// one row gets the callout).
pub fn optimize_project(
    session: &mut ProjectSession,
    baseline_trace: &SimulationCutTrace,
    progress: &dyn ProgressReporter,
    cancel: &AtomicBool,
) -> ProjectOptimizeReport {
    use std::sync::atomic::Ordering;

    // 1. Build the list of (toolpath_index, toolpath_id, name) triples
    //    for every enabled toolpath. We need ids to look up each
    //    toolpath's baseline cycle from the trace, names for the
    //    progress label, and indices for `optimize_toolpath`.
    let enabled: Vec<(usize, ToolpathId, String)> = session
        .toolpath_configs()
        .iter()
        .enumerate()
        .filter(|(_, tc)| tc.enabled)
        .map(|(idx, tc)| (idx, tc.id, tc.name.clone()))
        .collect();
    let total = enabled.len();

    // 2. Compute baseline cycle times per toolpath from the trace.
    //    Pre-compute the project total so we can derive the bottleneck
    //    callout without walking the report after the loop.
    let baseline_cycles: Vec<(ToolpathId, f64)> = enabled
        .iter()
        .map(|(_, id, _)| {
            let cycle = cycle_time_from_trace(baseline_trace, *id).unwrap_or(0.0);
            (*id, cycle)
        })
        .collect();
    let baseline_cycle_time_s: f64 = baseline_cycles.iter().map(|(_, c)| *c).sum();

    // 3. Walk the enabled toolpaths sequentially.
    let mut per_toolpath: Vec<(usize, OptimizeOutcome)> = Vec::with_capacity(total);
    for (completed, (idx, _id, name)) in enabled.iter().enumerate() {
        if cancel.load(Ordering::SeqCst) {
            break;
        }
        progress.report(
            completed,
            total,
            &format!("TP {} / {} — {}", completed + 1, total, name),
        );

        let outcome = optimize_toolpath(session, baseline_trace, *idx, cancel);
        per_toolpath.push((*idx, outcome));

        // The BaselineRestoreGuard inside `optimize_toolpath` cleared
        // this toolpath's cached result on drop. Re-generate at baseline
        // params so the next toolpath's project sim sees correct stock
        // state. Best-effort — a regen failure here doesn't invalidate
        // the per-toolpath outcome we already collected.
        let _ = session.generate_toolpath(*idx, cancel);
    }

    // 4. Pick the bottleneck: the largest-cycle toolpath whose baseline
    //    cycle crosses the policy fraction of total project time. Walk
    //    the `enabled` list (not `per_toolpath`, which may be partial
    //    under cancel) so the bottleneck is stable regardless of how far
    //    the optimization run got.
    let bottleneck_fraction = search_policy().bottleneck_fraction.value;
    let bottleneck_index = if baseline_cycle_time_s > 0.0 {
        enabled
            .iter()
            .zip(baseline_cycles.iter())
            .filter(|(_, (_, cycle))| *cycle / baseline_cycle_time_s >= bottleneck_fraction)
            .max_by(|(_, (_, a)), (_, (_, b))| a.total_cmp(b))
            .map(|((idx, _, _), _)| *idx)
    } else {
        None
    };

    ProjectOptimizeReport {
        baseline_cycle_time_s,
        bottleneck_index,
        per_toolpath,
    }
}

#[cfg(test)]
mod orchestration_skip_tests;

#[cfg(test)]
mod project_rollup_tests;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod stage1_grid_tests;

#[cfg(test)]
mod candidate_eval_tests;
