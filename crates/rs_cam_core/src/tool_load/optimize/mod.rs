//! The post-simulation toolpath tuner.
//!
//! The optimizer reads the simulation that is on screen, scores the
//! baseline with the load gates (`tool_load::evaluate_toolpath`), and
//! ranks candidates by their measured cycle time. Every number on a
//! candidate comes from a simulation of that candidate's parameters.
//! The feeds screen (Suggest) owns the calculation before a simulation.
//!
//! **Phase 1 of `planning/optimize_review_2026-10-12/PLAN.md`.** The
//! search has no levers. The run scores the baseline, evaluates no
//! candidate and returns `NoSafeImprovement`. Phase 2 adds the levers
//! (PLAN.md §2) through [`candidate::evaluate_candidate`].

use crate::ids::ToolpathId;
use std::sync::atomic::AtomicBool;

use serde::{Deserialize, Serialize};

use crate::session::ProjectSession;
use crate::stock::simulation_cut::SimulationCutTrace;

use super::RefuseReason;
use super::{ToleranceBands, ToolpathLoadContext, evaluate_toolpath};

mod candidate;
mod context;
mod narrative;
mod outcome;
pub mod progress;

pub use candidate::{OptimizeCandidate, ParamDelta};
pub use narrative::{
    AxisExtent, GateKind, LimitingGate, OutcomeNarrative, SearchEnvelopeReached,
    limiting_gates_for_verdict,
};
pub(crate) use outcome::build_outcome;
pub use outcome::{
    BaselineTraceAssumptions, CandidateSimAssumptions, KinematicsSource, LutQueryStamp,
    MachineSnapshot, OptimizeOutcome, OutcomeKind, ProjectOptimizeReport, SimAssumptionStamp,
};
pub use progress::{OptimizeProgress, OptimizeProgressSnapshot, SearchPhase};

use context::{
    BaselineRestoreGuard, EvaluationContext, air_cut_fraction_of_total_runtime_from_trace,
    cycle_time_from_trace,
};

/// The headline of the Phase 1 outcome. The GUI matches this text.
pub(crate) const NO_SEARCH_HEADLINE: &str = "No candidate was searched.";

/// The explanation of the Phase 1 outcome. The GUI matches this text.
pub(crate) const NO_SEARCH_EXPLANATION: &str =
    "The search has no levers yet; the post-sim tuner is being rebuilt.";

/// The fraction of the project baseline cycle time at which the rollup
/// names a toolpath the bottleneck.
///
/// This is a display choice for the rollup callout, not a cutting
/// limit. The value is the one the deleted search policy printed
/// (`git show 009bee30:crates/rs_cam_core/src/tool_load/optimize/policy.rs`,
/// `bottleneck_fraction`, a tuning choice: "Wanaka TP6 at 61% should
/// trip; small project-curve ops near 5% should not").
pub(crate) const BOTTLENECK_FRACTION: f64 = 0.30;

/// Which source produced a candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchStage {
    /// The operator's own parameters, scored against the on-screen
    /// `baseline_trace`. No new simulation runs for it.
    Baseline,
    /// A candidate the optimizer simulated with
    /// [`candidate::candidate_sim_options`] and scored with the gates.
    Measured,
}

/// Optimize one toolpath and return the search outcome.
///
/// **Mutation note.** The search mutates `session` for a short time.
/// Each candidate evaluation writes the candidate's parameters through
/// `apply_toolpath_param_snapshot_narrow`, regenerates the toolpath and
/// runs a project simulation. An RAII baseline-restore guard writes the
/// original parameters back on every exit path. Apply is a separate
/// operator action; the optimizer never keeps a candidate's state.
///
/// **The guard restores the PARAMETERS and only the parameters.**
/// `apply_toolpath_param_snapshot_narrow` ends with `drop_result(index)`
/// and `session.simulation = None`. After the call the toolpath's cached
/// result is gone, its generation-input revision has moved, and the
/// project simulation slot is empty. Measured by
/// `crates/rs_cam_core/tests/optimize_toolpath_is_a_job_wp14b.rs`.
///
/// Since WP14b no production caller gives this function a LIVE session.
/// The per-toolpath route takes the `optimize_toolpath` `Job` row, whose
/// handle owns a clone; the project rollup passes `session.clone()` to
/// its lane. The residue therefore lands on a copy.
///
/// **Cancellation.** The search reads `cancel` before it changes the
/// session. The simulator reads the same `&AtomicBool`.
///
/// `baseline_trace` is the project simulation on screen, the same trace
/// that the gate and the diagnostics panel read. The search scores
/// candidate 0 (the baseline) against this trace directly.
pub fn optimize_toolpath(
    session: &mut ProjectSession,
    baseline_trace: &SimulationCutTrace,
    toolpath_index: usize,
    cancel: &AtomicBool,
) -> OptimizeOutcome {
    let progress = std::sync::Arc::new(OptimizeProgress::default());
    optimize_toolpath_observed(session, baseline_trace, toolpath_index, cancel, &progress)
}

/// Optimize one toolpath and publish the search progress while it runs
/// (WP29).
///
/// Same search as [`optimize_toolpath`], same outcome, same mutation
/// note. The one difference is `progress`: the search announces each
/// rung of its ladder on it and ticks once for each candidate. The
/// caller reads it from another thread through
/// [`OptimizeProgress::snapshot`].
///
/// [`optimize_toolpath`] delegates here with a silent sink, so the
/// machine and assumption stamps stay in ONE place.
///
/// [`OptimizeProgress::finish`] runs after the inner call returns, so
/// the progress clears on every exit path. The per-rung counters stay,
/// so a settled run stays readable.
pub fn optimize_toolpath_observed(
    session: &mut ProjectSession,
    baseline_trace: &SimulationCutTrace,
    toolpath_index: usize,
    cancel: &AtomicBool,
    progress: &std::sync::Arc<OptimizeProgress>,
) -> OptimizeOutcome {
    // F4.3 — stamp the machine caps this run used on every outcome
    // (refusals and skips included).
    let snapshot = outcome::MachineSnapshot::of(session.machine());
    // A-8 (F-OPT) — stamp the simulation assumptions on every outcome.
    // The stamp is read BEFORE the search, so a `Skipped` outcome also
    // names its operating point, and so the search cannot change the
    // session the stamp reads.
    let assumptions = outcome::SimAssumptionStamp::of(session, toolpath_index, baseline_trace);
    let mut outcome =
        optimize_toolpath_inner(session, baseline_trace, toolpath_index, cancel, progress);
    // WP29 — one clear site for every exit path of the inner search.
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

    // 1. Build the evaluation context. Skip if the toolpath or its tool
    //    is missing.
    let Some(ctx) = EvaluationContext::from_session(session, toolpath_index)
        .map(|ctx| ctx.with_progress(std::sync::Arc::clone(progress)))
    else {
        return OptimizeOutcome::skipped(RefuseReason::SimulationRequired);
    };

    // 2. Skip the drill cycles. A drill cycle is a plunge; it has no
    //    steady-state chipload to compare.
    if ctx.operation_kind.is_drill_kinematics() {
        return OptimizeOutcome::skipped(RefuseReason::SteadyStateSamplesNotPresent);
    }

    // 3. Skip a `Custom` material. It has no measured force line.
    if matches!(ctx.material, crate::material::Material::Custom { .. }) {
        return OptimizeOutcome::skipped(RefuseReason::MaterialUnvalidated);
    }

    // 4. Score the baseline candidate from the on-screen trace, with the
    //    same gate and the same strict bands as the tool-load card, so
    //    the index-0 row matches what the operator already sees.
    let baseline_op = match session.get_toolpath_config(toolpath_index) {
        Some(tc) => tc.operation.clone(),
        None => {
            return OptimizeOutcome::skipped(RefuseReason::SimulationRequired);
        }
    };
    // D6/D7: the spans of this toolpath from the cached compute result.
    // `None` when no generation ran; the locality classifier then uses
    // engagement labels only. F2.2: also `None` when a transform made
    // the spans invalid (TSP split).
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
    let baseline_verdict = evaluate_toolpath(
        &baseline_load_ctx,
        Some(baseline_trace),
        Some(&machine),
        &ToleranceBands::default(),
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
        params: baseline_op,
        delta: ParamDelta::default(),
        cycle_time_s: baseline_cycle_s,
        verdict: baseline_verdict,
        stage: SearchStage::Baseline,
        reconciled_cycle_time_s: None,
        reconciled_verdict: None,
        air_cut_fraction_of_total_runtime: baseline_air_cut,
    };

    // 5. Read the cancel flag before the session changes.
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

    // 6. From here the search can change the session. The guard
    //    restores `(operation, dressups, face_selection)` when it drops,
    //    on every exit path (return, error, panic).
    let Ok(guard) = BaselineRestoreGuard::new(session, toolpath_index) else {
        return OptimizeOutcome::skipped(RefuseReason::SimulationRequired);
    };

    // 7. Phase 1: the search has no levers. Each rung announces a total
    //    of zero, so the progress row names every rung and ticks none.
    for phase in SearchPhase::ALL {
        ctx.progress.begin_phase(phase, 0);
    }
    let candidates: Vec<OptimizeCandidate> = Vec::new();

    // 8. Drop the guard before the outcome is built, so the session has
    //    the baseline parameters again when the caller reads it.
    drop(guard);
    build_outcome(baseline_candidate, candidates)
}

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
/// toolpath whose baseline cycle is at least [`BOTTLENECK_FRACTION`] of
/// the total project runtime. The largest cycle wins a tie, so only one
/// row gets the callout.
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
    //    cycle crosses `BOTTLENECK_FRACTION` of total project time. Walk
    //    the `enabled` list (not `per_toolpath`, which may be partial
    //    under cancel) so the bottleneck is stable regardless of how far
    //    the optimization run got.
    let bottleneck_index = if baseline_cycle_time_s > 0.0 {
        enabled
            .iter()
            .zip(baseline_cycles.iter())
            .filter(|(_, (_, cycle))| *cycle / baseline_cycle_time_s >= BOTTLENECK_FRACTION)
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
