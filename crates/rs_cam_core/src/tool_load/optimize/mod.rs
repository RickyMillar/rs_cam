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

use std::sync::LazyLock;
use std::sync::atomic::AtomicBool;

use serde::{Deserialize, Serialize};

use crate::compute::catalog::{OperationConfig, OperationType};
use crate::feeds::vendor_lookup::MatchedRow;
use crate::machine::MachineProfile;
use crate::session::ProjectSession;
use crate::simulation_cut::SimulationCutTrace;

use super::RefuseReason;
use super::verdict::{ChiploadVerdict, ToolpathLoadVerdict};
use super::{ToleranceBands, ToolpathLoadContext, evaluate_toolpath};

pub mod axes;
pub mod bounds;
mod candidate;
mod context;
mod delta;
mod outcome;
pub mod patches;
mod policy;
mod preflight;
mod refusal;
pub mod retarget;
pub mod space;
pub mod strategy;

pub use candidate::{OptimizeCandidate, feeds_auto_for_candidate};
pub(crate) use candidate::{
    evaluate_candidate, finalize_partial, refine_stage2, select_stage2_candidates,
};
pub(crate) use delta::delta_against_baseline;
pub use delta::{GateDelta, GateDeltas, ParamDelta};
pub(crate) use outcome::build_outcome;
pub use outcome::{OptimizeOutcome, ProjectOptimizeReport};

use context::{
    BaselineRestoreGuard, EvaluationContext, baseline_rpm_from_trace, cycle_time_from_trace,
    find_matched_lut_row,
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
        // `deflection_breach_tolerance` exists on the policy for symmetry
        // but the deflection gate isn't wired through `ToleranceBands` in
        // this commit — see the doc on `tool_load::ToleranceBands`.
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
    session: &mut ProjectSession,
    baseline_trace: &SimulationCutTrace,
    toolpath_index: usize,
    cancel: &AtomicBool,
) -> OptimizeOutcome {
    use std::sync::atomic::Ordering;

    // 1. Build the evaluation context. Skip cleanly if the toolpath or
    //    its tool is missing.
    let Some(ctx) = EvaluationContext::from_session(session, toolpath_index) else {
        return OptimizeOutcome::Skipped {
            reason: RefuseReason::SimulationRequired,
        };
    };

    // 2. Skip op kinds the gate can't model. Drill cycles are pure
    //    plunge — no chipload measurement to compare against.
    if matches!(
        ctx.operation_kind,
        OperationType::Drill | OperationType::AlignmentPinDrill
    ) {
        return OptimizeOutcome::Skipped {
            reason: RefuseReason::SteadyStateSamplesNotPresent,
        };
    }

    // 3. Skip Custom material — Kc is unvalidated, power model
    //    unreliable.
    if matches!(ctx.material, crate::material::Material::Custom { .. }) {
        return OptimizeOutcome::Skipped {
            reason: RefuseReason::MaterialUnvalidated,
        };
    }

    // 4. Build the baseline candidate from the existing trace. Score
    //    via the same gate that diagnostics already used so the
    //    rollup's index-0 row matches what the user already sees.
    let baseline_op = match session.get_toolpath_config(toolpath_index) {
        Some(tc) => tc.operation.clone(),
        None => {
            return OptimizeOutcome::Skipped {
                reason: RefuseReason::SimulationRequired,
            };
        }
    };
    let baseline_load_ctx = ToolpathLoadContext {
        toolpath_id: ctx.toolpath_id,
        tool: &ctx.tool,
        material: &ctx.material,
        operation_family: ctx.lut_op_family,
        pass_role: ctx.lut_pass_role,
        operation_feed_rate_mm_min: baseline_op.feed_rate(),
        operation_kind: ctx.operation_kind,
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
            return OptimizeOutcome::Skipped {
                reason: RefuseReason::SteadyStateSamplesNotPresent,
            };
        }
    };
    let baseline_candidate = OptimizeCandidate {
        params: baseline_op.clone(),
        delta: ParamDelta::default(),
        cycle_time_s: baseline_cycle_s,
        verdict: baseline_verdict.clone(),
        stage: SearchStage::Baseline,
        reconciled_cycle_time_s: None,
        reconciled_verdict: None,
        gate_deltas: None,
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
    //     optimizer's search space (deflection L/D, bipolar chipload),
    //     refuse immediately with an op-aware prescription rather
    //     than running stages that can't move the failing gate.
    if let Some(refusal) = preflight::preflight_classify(
        &ctx,
        baseline_trace,
        baseline_op.feed_rate(),
        &baseline_verdict,
        matched_lut_row.as_ref(),
    ) {
        return OptimizeOutcome::NoSafeImprovement {
            reason: refusal.reason,
            explanation: refusal.explanation,
            attempted: vec![baseline_candidate],
        };
    }

    // Cancel check before any sims.
    if cancel.load(Ordering::SeqCst) {
        return OptimizeOutcome::NoSafeImprovement {
            reason: RefuseReason::NoImprovementFound,
            explanation: "cancelled before any candidates were generated".to_owned(),
            attempted: vec![baseline_candidate],
        };
    }

    // 6. From here on the session is mutated per-candidate. The
    //    BaselineRestoreGuard restores `(operation, dressups,
    //    face_selection, feeds_auto)` on drop, regardless of how
    //    we exit (early return, Err, panic).
    let Ok(mut guard) = BaselineRestoreGuard::new(session, toolpath_index) else {
        return OptimizeOutcome::Skipped {
            reason: RefuseReason::SimulationRequired,
        };
    };

    let baseline_rpm = baseline_rpm_from_trace(
        baseline_trace,
        ctx.toolpath_id,
        baseline_op.spindle_rpm(),
        &machine,
    );

    // 7. Stage F: closed-form feed/RPM solve. Two modes:
    //    - Within baselines → headroom scale-up (Stage 0).
    //    - Single-side Exceeds (Burn or Breakage, not bipolar — pre-
    //      flight refused bipolar already) → re-target via the LUT
    //      row's chipload envelope, RCTF-compensated.
    //    Either mode produces at most one candidate.
    let mut all_candidates: Vec<OptimizeCandidate> = Vec::new();
    match baseline_verdict.chipload {
        ChiploadVerdict::Within { .. } => {
            if let Some(c) = run_headroom_strategy(
                &mut guard,
                &ctx,
                &baseline_op,
                baseline_rpm,
                &baseline_verdict,
                matched_lut_row.as_ref(),
                &machine,
                cancel,
            ) {
                all_candidates.push(c);
            }
        }
        ChiploadVerdict::Exceeds { .. } => {
            all_candidates.extend(run_retarget_strategy(
                &mut guard,
                &ctx,
                &baseline_op,
                baseline_rpm,
                &baseline_verdict,
                matched_lut_row.as_ref(),
                &machine,
                cancel,
            ));
        }
        ChiploadVerdict::Unmodeled { .. } => {}
    }

    if cancel.load(Ordering::SeqCst) {
        drop(guard);
        return finalize_partial(baseline_candidate, all_candidates);
    }

    // 8. Axis-grid strategy: joint DOC × stepover × scallop_height
    //    variant grid, anchored on the headroom candidate's params
    //    (when stage F fired) or baseline (when it didn't). Replaces
    //    the legacy `run_stage_1_grid` (G16 Step 6c).
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
        return finalize_partial(baseline_candidate, all_candidates);
    }

    // 9. Stage 2: top-3 by cycle time, re-eval at full resolution.
    let stage2_survivor_count = search_policy().stages.refined_survivor_count.value;
    let stage2_seeds = select_stage2_candidates(all_candidates, stage2_survivor_count);
    let Ok(stage2_candidates) = refine_stage2(&mut guard, &ctx, stage2_seeds, cancel) else {
        drop(guard);
        return OptimizeOutcome::NoSafeImprovement {
            reason: RefuseReason::NoImprovementFound,
            explanation: "candidate evaluation failed at full resolution — partial result returned"
                .to_owned(),
            attempted: vec![baseline_candidate],
        };
    };

    // 10. Drop the guard explicitly so the baseline is restored before
    //     building the outcome (which references the candidates,
    //     not the session). The outcome is returned to the caller; the
    //     caller's view of `session` is now back at the baseline.
    drop(guard);
    build_outcome(baseline_candidate, stage2_candidates)
}

/// Run Stage 0 (analytical RPM/feed headroom scale-up) for one
/// toolpath. Returns the headroom candidate, or `None` if the baseline
/// already trips the chipload gate (proportional scaling preserves
/// chipload, so it can't fix `Exceeds`), if the closed-form solver
/// finds no headroom (`k ≤ 1`), or if the candidate sim fails.
///
/// Run the headroom-scale strategy against the baseline and return
/// the (at most one) candidate it produces. Replaces the legacy
/// `run_stage_0` (G16 Step 6) — the strategy emits `CandidatePatch`es;
/// this wrapper applies them to the baseline op via `apply_patches_to_op`
/// and runs the per-candidate sim.
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
/// sim. Replaces the legacy `run_stage_f_retarget` (G16 Step 6b).
///
/// **Behaviour change.** The legacy chipload retarget produced a
/// `commanded × RCTF` solve that lowered feed on `BurnRisk`. The new
/// chipload retargeter is sample-driven (`target_chipload /
/// observed_peak`), so `BurnRisk` raises feed. Wanaka TP 4 (feed=3150,
/// peak=0.0253, LUT [0.038, 0.07], 1.20× headroom) now produces a
/// feed-up candidate that clamps at 5000 mm/min — the previous Stage F
/// produced a feed-down one.
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
) -> Vec<OptimizeCandidate> {
    use crate::compute::catalog::OptimizationSurface;
    use std::sync::atomic::Ordering;
    use strategy::OptimizationStrategy;
    use strategy::retarget::PerGateRetargetStrategy;

    let view = match baseline_op.optimization_surface() {
        OptimizationSurface::Optimizable(v) => v,
        OptimizationSurface::NotOptimizable { .. } => return Vec::new(),
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

    let chipload = matched_lut_row.and_then(|row| {
        if row.chip_load_min_mm.is_none() && row.chip_load_max_mm.is_none() {
            return None;
        }
        Some(retarget::chipload::ChiploadFeedRetargeter {
            lut_chipload_min: row.chip_load_min_mm.unwrap_or(f64::NAN),
            lut_chipload_max: row.chip_load_max_mm.unwrap_or(f64::NAN),
            low_headroom: policy.retarget.chipload_low_headroom.value,
            high_headroom: policy.retarget.chipload_high_headroom.value,
            plunge_tracking_threshold: policy.feed.plunge_tracking_threshold_fraction.value,
        })
    });

    let available_kw = machine.power_at_rpm(baseline_rpm) * machine.safety_factor;
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
    let cps = strat.candidates(&view, baseline_verdict);

    let mut out: Vec<OptimizeCandidate> = Vec::new();
    for cp in cps {
        if cancel.load(Ordering::SeqCst) {
            break;
        }
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
            out.push(candidate);
        }
    }
    out
}

/// Run the [`AxisGridStrategy`] against the baseline (or the headroom
/// candidate, when stage F fired) and evaluate every emitted cell.
/// Replaces the legacy `run_stage_1_grid` (G16 Step 6c) — same anchor
/// + dedup + variant logic, now lifted into the strategy module.
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

    let mut out: Vec<OptimizeCandidate> = Vec::new();
    for cp in cps {
        if cancel.load(Ordering::SeqCst) {
            break;
        }
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
//   - Deflection `Exceeds` → search space (feed/RPM/DOC/stepover)
//     can't reach a Within answer because L/D depends only on the
//     tool config. Refuse with `DeflectionSetupLocked`.
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
// would need each worker to clone the whole session, which we
// deliberately avoid (`ToolpathConfig` is not `Clone`, and a Cloneable
// session would require a wide-touch refactor we don't want to do for
// this).
//
// **Stock-state hygiene between toolpaths.** `optimize_toolpath`'s
// `BaselineRestoreGuard` restores the toolpath's params on drop via
// `apply_toolpath_param_snapshot`, which invalidates that toolpath's
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
    let enabled: Vec<(usize, usize, String)> = session
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
    let baseline_cycles: Vec<(usize, f64)> = enabled
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
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod orchestration_skip_tests {
    //! Tests for `optimize_toolpath`'s early-skip paths — the cases
    //! that don't require running a real sim. End-to-end tests with
    //! actual sims are deferred to integration tests in
    //! `tests/optimize_smoke.rs` (slow path).

    use super::refusal::{bipolar_prescription, deflection_setup_prescription};
    use super::*;
    use crate::compute::catalog::OperationConfig;
    use crate::compute::config::{DressupConfig, FeedsAutoMode};
    use crate::compute::operation_configs::{AlignmentPinDrillConfig, DrillConfig, PocketConfig};
    use crate::compute::tool_config::{ToolConfig, ToolId, ToolType};
    use crate::feeds::OperationFamily;
    use crate::session::ToolpathConfig;
    use crate::simulation_cut::{SimulationCutSummary, SimulationCutTrace};

    fn make_tool() -> ToolConfig {
        ToolConfig::new_default(ToolId(0), ToolType::EndMill)
    }

    fn empty_trace() -> SimulationCutTrace {
        SimulationCutTrace {
            schema_version: 1,
            sample_step_mm: 1.0,
            summary: SimulationCutSummary::default(),
            samples: Vec::new(),
            toolpath_summaries: Vec::new(),
            semantic_summaries: Vec::new(),
            hotspots: Vec::new(),
            issues: Vec::new(),
            provenance: None,
        }
    }

    fn make_tc(operation: OperationConfig, tool_id: usize) -> ToolpathConfig {
        ToolpathConfig {
            id: 0,
            name: "test".to_owned(),
            enabled: true,
            operation,
            dressups: DressupConfig::default(),
            heights: crate::compute::config::HeightsConfig::default(),
            tool_id,
            model_id: 0,
            pre_gcode: None,
            post_gcode: None,
            boundary: crate::compute::config::BoundaryConfig::default(),
            boundary_inherit: true,
            stock_source: crate::compute::config::StockSource::Fresh,
            coolant: crate::gcode::CoolantMode::Off,
            face_selection: None,
            feeds_auto: FeedsAutoMode::default(),
            debug_options: crate::debug_trace::ToolpathDebugOptions::default(),
        }
    }

    fn session_with_op(operation: OperationConfig) -> ProjectSession {
        let mut s = ProjectSession::new_empty();
        s.add_tool(make_tool());
        s.add_toolpath(0, make_tc(operation, s.tools()[0].id.0))
            .unwrap();
        s
    }

    #[test]
    fn drill_op_yields_skipped() {
        let mut session = session_with_op(OperationConfig::Drill(DrillConfig::default()));
        let trace = empty_trace();
        let cancel = AtomicBool::new(false);
        let outcome = optimize_toolpath(&mut session, &trace, 0, &cancel);
        assert!(
            matches!(
                outcome,
                OptimizeOutcome::Skipped {
                    reason: RefuseReason::SteadyStateSamplesNotPresent
                }
            ),
            "got {outcome:?}"
        );
    }

    #[test]
    fn alignment_pin_drill_yields_skipped() {
        let mut session = session_with_op(OperationConfig::AlignmentPinDrill(
            AlignmentPinDrillConfig::default(),
        ));
        let trace = empty_trace();
        let cancel = AtomicBool::new(false);
        let outcome = optimize_toolpath(&mut session, &trace, 0, &cancel);
        assert!(matches!(
            outcome,
            OptimizeOutcome::Skipped {
                reason: RefuseReason::SteadyStateSamplesNotPresent
            }
        ));
    }

    #[test]
    fn custom_material_yields_skipped() {
        use crate::compute::stock_config::StockConfig;
        let mut session = session_with_op(OperationConfig::Pocket(PocketConfig::default()));
        let mut stock = session.stock_config().clone();
        stock.material = crate::material::Material::Custom {
            name: "test".to_owned(),
            hardness_index: 1.0,
            kc: 30.0,
        };
        session.set_stock_config(stock);
        let _ = StockConfig::default(); // silence unused-import false positive

        let trace = empty_trace();
        let cancel = AtomicBool::new(false);
        let outcome = optimize_toolpath(&mut session, &trace, 0, &cancel);
        assert!(matches!(
            outcome,
            OptimizeOutcome::Skipped {
                reason: RefuseReason::MaterialUnvalidated
            }
        ));
    }

    #[test]
    fn invalid_toolpath_index_yields_skipped() {
        let mut session = session_with_op(OperationConfig::Pocket(PocketConfig::default()));
        let trace = empty_trace();
        let cancel = AtomicBool::new(false);
        let outcome = optimize_toolpath(&mut session, &trace, 99, &cancel);
        assert!(matches!(outcome, OptimizeOutcome::Skipped { .. }));
    }

    #[test]
    fn empty_trace_yields_skipped() {
        // Pocket op (supported), but trace has no samples for the
        // toolpath — Skipped per "no steady-state samples".
        let mut session = session_with_op(OperationConfig::Pocket(PocketConfig::default()));
        let trace = empty_trace();
        let cancel = AtomicBool::new(false);
        let outcome = optimize_toolpath(&mut session, &trace, 0, &cancel);
        assert!(matches!(
            outcome,
            OptimizeOutcome::Skipped {
                reason: RefuseReason::SteadyStateSamplesNotPresent
            }
        ));
    }

    #[test]
    fn cancel_before_run_yields_no_safe_improvement() {
        // Trace has a baseline samples for this toolpath_id, so the
        // early skip paths don't trigger; cancel flag is set before
        // any candidates are evaluated.
        use crate::simulation_cut::SimulationToolpathCutSummary;
        let mut trace = empty_trace();
        trace.toolpath_summaries.push(SimulationToolpathCutSummary {
            toolpath_id: 0,
            sample_count: 100,
            total_runtime_s: 60.0,
            cutting_runtime_s: 50.0,
            rapid_runtime_s: 10.0,
            air_cut_time_s: 0.0,
            low_engagement_time_s: 0.0,
            average_engagement: 0.5,
            peak_chipload_mm_per_tooth: 0.04,
            peak_axial_doc_mm: 2.0,
            total_removed_volume_est_mm3: 100.0,
            average_mrr_mm3_s: 2.0,
        });

        let mut session = session_with_op(OperationConfig::Pocket(PocketConfig::default()));
        let cancel = AtomicBool::new(true); // cancelled up-front
        let outcome = optimize_toolpath(&mut session, &trace, 0, &cancel);
        // Cancel-up-front should not produce a Ranked outcome.
        assert!(
            !matches!(outcome, OptimizeOutcome::Ranked(_)),
            "cancelled run should not produce Ranked, got {outcome:?}"
        );
    }

    // ── Pre-flight refusals (Commit #1) ──────────────────────────────

    /// Build a populated trace whose `toolpath_summaries[0]` reports a
    /// non-zero cycle time, so `optimize_toolpath` walks past the
    /// "no cycle time" skip path and reaches the pre-flight classifier.
    /// Adds cutting samples for `toolpath_id = 0` so the force-aware
    /// deflection gate has data to evaluate.
    fn trace_with_summary_and_high_force_samples() -> SimulationCutTrace {
        use crate::simulation_cut::{
            CutKinematics, SimulationCutSample, SimulationToolpathCutSummary,
        };
        let mut trace = empty_trace();
        trace.toolpath_summaries.push(SimulationToolpathCutSummary {
            toolpath_id: 0,
            sample_count: 4,
            total_runtime_s: 60.0,
            cutting_runtime_s: 50.0,
            rapid_runtime_s: 10.0,
            air_cut_time_s: 0.0,
            low_engagement_time_s: 0.0,
            average_engagement: 0.8,
            peak_chipload_mm_per_tooth: 0.06,
            peak_axial_doc_mm: 6.0,
            total_removed_volume_est_mm3: 100.0,
            average_mrr_mm3_s: 2.0,
        });
        // Slot at 6 mm DOC on a 6.35 mm cutter at full π arc — force
        // peaks at Kc × 6 × 6.35. With softwood Kc=6 and the default
        // 45 mm stickout carbide tool, this produces δ around 230 µm,
        // tripping the 200 µm Exceeds threshold.
        for i in 0..4 {
            trace.samples.push(SimulationCutSample {
                toolpath_id: 0,
                move_index: i,
                sample_index: i,
                position: [i as f64, 0.0, -6.0],
                cumulative_time_s: 0.1 * i as f64,
                segment_time_s: 0.1,
                is_cutting: true,
                cut_kinematics: CutKinematics::Linear,
                feed_rate_mm_min: 1500.0,
                spindle_rpm: 18_000,
                flute_count: 2,
                axial_doc_mm: 6.0,
                radial_engagement: 1.0,
                arc_engagement_radians: Some(std::f64::consts::PI),
                chipload_mm_per_tooth: 0.04,
                effective_chip_thickness_mm: Some(0.04),
                removed_volume_est_mm3: 1.0,
                mrr_mm3_s: 10.0,
                semantic_item_id: None,
                span_path: Vec::new(),
            });
        }
        trace
    }

    #[test]
    fn deflection_exceeds_yields_setup_locked_refusal() {
        // Default endmill has stickout 45 mm, diameter 6.35 mm. With a
        // 6 mm slot in HardMaple (Kc=15) the force-aware deflection
        // gate predicts a peak δ around 350 µm, comfortably above the
        // 200 µm Exceeds threshold and triggering the pre-flight
        // `DeflectionSetupLocked` refusal.
        let mut session = session_with_op(OperationConfig::Pocket(PocketConfig::default()));
        let mut stock = session.stock_config().clone();
        stock.material = crate::material::Material::SolidWood {
            species: crate::material::WoodSpecies::HardMaple,
        };
        session.set_stock_config(stock);
        let trace = trace_with_summary_and_high_force_samples();
        let cancel = AtomicBool::new(false);
        let outcome = optimize_toolpath(&mut session, &trace, 0, &cancel);
        match outcome {
            OptimizeOutcome::NoSafeImprovement {
                reason,
                explanation,
                attempted,
            } => {
                assert_eq!(reason, RefuseReason::DeflectionSetupLocked);
                assert!(
                    explanation.contains("µm"),
                    "explanation should report deflection in µm, got: {explanation}"
                );
                assert!(
                    explanation.contains("stickout"),
                    "explanation should point at the stickout lever, got: {explanation}"
                );
                assert_eq!(
                    attempted.len(),
                    1,
                    "deflection refusal should not burn any candidate sims — only the baseline \
                     candidate should be attempted"
                );
            }
            other => panic!("expected DeflectionSetupLocked NoSafeImprovement, got {other:?}"),
        }
    }

    #[test]
    fn deflection_setup_locked_explanation_carries_target_stickout() {
        // For peak δ=215 µm and current stickout=45 mm, the cube-root
        // scaling target stickout for δ=50 µm is
        //   target_L = 45 × (50/215)^(1/3) ≈ 27.8 mm  →  rounds to "28".
        let tool = crate::tool::ToolDefinition::new(
            Box::new(crate::tool::FlatEndmill::new(6.35, 20.0)),
            6.35,
            20.0,
            40.0,
            45.0,
            2,
            crate::compute::tool_config::ToolMaterial::Carbide,
        );
        let peak_delta_mm = 0.215;
        let s = deflection_setup_prescription(&tool, peak_delta_mm);
        assert!(s.contains("215"), "peak µm not in '{s}'");
        assert!(s.contains("28"), "target stickout ~28mm not in '{s}'");
        assert!(s.contains("stickout"), "stickout lever not in '{s}'");
    }

    #[test]
    fn bipolar_prescription_for_doc_knob_op_points_at_doc_or_stepover() {
        // Adaptive3d / Pocket / Adaptive / Rest / Face all have a DOC
        // knob the user can adjust to reduce engagement variance.
        let s = bipolar_prescription(OperationType::Adaptive3d, OperationFamily::Adaptive);
        assert!(
            s.contains("stepover") || s.contains("depth-per-pass"),
            "DOC-knob op should suggest stepover or DOC, got: {s}"
        );
    }

    #[test]
    fn bipolar_prescription_for_3d_finishing_op_points_at_setup() {
        // Parallel and Scallop have no DOC knob — engagement variance
        // is driven by geometry. The prescription should point at a
        // setup-level lever, not a DOC tweak.
        let s = bipolar_prescription(OperationType::Scallop, OperationFamily::Parallel);
        assert!(
            !s.contains("depth-per-pass"),
            "non-DOC-knob op should NOT suggest DOC tweak, got: {s}"
        );
        assert!(
            s.contains("stepover") || s.contains("cutter"),
            "3D finishing prescription should point at stepover or tool, got: {s}"
        );
    }

    #[test]
    fn bipolar_prescription_for_contour_points_at_geometry() {
        // Contour and Trace are profile-following ops — variance comes
        // from the part geometry, not stepover.
        let s = bipolar_prescription(OperationType::Profile, OperationFamily::Contour);
        assert!(
            s.contains("part geometry")
                || s.contains("multiple passes")
                || s.contains("smaller cutter"),
            "contour prescription should point at geometry-driven levers, got: {s}"
        );
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod project_rollup_tests {
    //! Tests for `optimize_project` orchestration. We lean on Drill
    //! and AlignmentPinDrill ops because `optimize_toolpath` returns
    //! `Skipped` for them without running any sim — so the rollup
    //! walks several toolpaths without needing actual generation /
    //! simulation infrastructure. Bottleneck-detection and progress
    //! sequencing are observable from the outcomes alone.

    use super::*;
    use crate::compute::catalog::OperationConfig;
    use crate::compute::config::{
        BoundaryConfig, DressupConfig, FeedsAutoMode, HeightsConfig, StockSource,
    };
    use crate::compute::operation_configs::{AlignmentPinDrillConfig, DrillConfig, PocketConfig};
    use crate::compute::tool_config::{ToolConfig, ToolId, ToolType};
    use crate::debug_trace::ToolpathDebugOptions;
    use crate::gcode::CoolantMode;
    use crate::session::ToolpathConfig;
    use crate::simulation_cut::{
        SimulationCutSummary, SimulationCutTrace, SimulationToolpathCutSummary,
    };
    use std::sync::Mutex;

    fn make_tool() -> ToolConfig {
        ToolConfig::new_default(ToolId(0), ToolType::EndMill)
    }

    fn empty_trace() -> SimulationCutTrace {
        SimulationCutTrace {
            schema_version: 1,
            sample_step_mm: 1.0,
            summary: SimulationCutSummary::default(),
            samples: Vec::new(),
            toolpath_summaries: Vec::new(),
            semantic_summaries: Vec::new(),
            hotspots: Vec::new(),
            issues: Vec::new(),
            provenance: None,
        }
    }

    fn summary_for(toolpath_id: usize, runtime_s: f64) -> SimulationToolpathCutSummary {
        SimulationToolpathCutSummary {
            toolpath_id,
            sample_count: 100,
            total_runtime_s: runtime_s,
            cutting_runtime_s: runtime_s,
            rapid_runtime_s: 0.0,
            air_cut_time_s: 0.0,
            low_engagement_time_s: 0.0,
            average_engagement: 0.5,
            peak_chipload_mm_per_tooth: 0.04,
            peak_axial_doc_mm: 2.0,
            total_removed_volume_est_mm3: 100.0,
            average_mrr_mm3_s: 2.0,
        }
    }

    fn make_tc(
        name: &str,
        operation: OperationConfig,
        tool_id: usize,
        enabled: bool,
    ) -> ToolpathConfig {
        ToolpathConfig {
            id: 0,
            name: name.to_owned(),
            enabled,
            operation,
            dressups: DressupConfig::default(),
            heights: HeightsConfig::default(),
            tool_id,
            model_id: 0,
            pre_gcode: None,
            post_gcode: None,
            boundary: BoundaryConfig::default(),
            boundary_inherit: true,
            stock_source: StockSource::Fresh,
            coolant: CoolantMode::Off,
            face_selection: None,
            feeds_auto: FeedsAutoMode::default(),
            debug_options: ToolpathDebugOptions::default(),
        }
    }

    /// Build a session with N drill toolpaths. Each drill triggers
    /// `optimize_toolpath`'s `Skipped` early-exit (no sim required),
    /// keeping these tests fast.
    fn session_with_n_drills(names_and_enabled: &[(&str, bool)]) -> ProjectSession {
        let mut s = ProjectSession::new_empty();
        s.add_tool(make_tool());
        let tool_id = s.tools()[0].id.0;
        for (name, enabled) in names_and_enabled {
            s.add_toolpath(
                0,
                make_tc(
                    name,
                    OperationConfig::Drill(DrillConfig::default()),
                    tool_id,
                    *enabled,
                ),
            )
            .unwrap();
        }
        s
    }

    /// Progress reporter that records every (completed, total, label)
    /// invocation. Used to verify call sequencing.
    struct RecordingProgress {
        calls: Mutex<Vec<(usize, usize, String)>>,
    }

    impl RecordingProgress {
        fn new() -> Self {
            Self {
                calls: Mutex::new(Vec::new()),
            }
        }

        fn into_calls(self) -> Vec<(usize, usize, String)> {
            self.calls.into_inner().unwrap()
        }
    }

    impl ProgressReporter for RecordingProgress {
        fn report(&self, completed: usize, total: usize, label: &str) {
            self.calls
                .lock()
                .unwrap()
                .push((completed, total, label.to_owned()));
        }
    }

    #[test]
    fn empty_project_returns_empty_report() {
        let mut session = ProjectSession::new_empty();
        let trace = empty_trace();
        let cancel = AtomicBool::new(false);
        let report = optimize_project(&mut session, &trace, &NoProgress, &cancel);
        assert_eq!(report.baseline_cycle_time_s, 0.0);
        assert_eq!(report.bottleneck_index, None);
        assert!(report.per_toolpath.is_empty());
    }

    #[test]
    fn disabled_toolpaths_excluded_from_walk() {
        // 3 toolpaths, only the middle one is enabled. Progress should
        // see total=1 and the per_toolpath should have one entry.
        let mut session = session_with_n_drills(&[("a", false), ("b", true), ("c", false)]);
        let trace = empty_trace();
        let cancel = AtomicBool::new(false);
        let progress = RecordingProgress::new();
        let report = optimize_project(&mut session, &trace, &progress, &cancel);
        assert_eq!(report.per_toolpath.len(), 1);
        assert_eq!(report.per_toolpath[0].0, 1, "the enabled TP is at index 1");

        let calls = progress.into_calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, 0); // completed before this one
        assert_eq!(calls[0].1, 1); // total enabled
        assert!(
            calls[0].2.contains("b"),
            "label should name the toolpath: {}",
            calls[0].2
        );
    }

    #[test]
    fn drill_toolpaths_all_yield_skipped() {
        let mut session = session_with_n_drills(&[("d1", true), ("d2", true), ("d3", true)]);
        let trace = empty_trace();
        let cancel = AtomicBool::new(false);
        let report = optimize_project(&mut session, &trace, &NoProgress, &cancel);
        assert_eq!(report.per_toolpath.len(), 3);
        for (_, outcome) in &report.per_toolpath {
            assert!(matches!(outcome, OptimizeOutcome::Skipped { .. }));
        }
    }

    #[test]
    fn bottleneck_picks_dominant_toolpath() {
        // 3 toolpaths with cycles [10s, 60s, 30s]. Total = 100s.
        // TP 1 at 60% trips the 30% threshold; TP 2 at 30% also trips
        // (≥). Bottleneck is the largest, so TP 1 wins.
        let mut session = session_with_n_drills(&[("a", true), ("b", true), ("c", true)]);
        // Pull the assigned ids out of the session (add_toolpath
        // increments next_toolpath_id, so they're stable: 0, 1, 2).
        let ids: Vec<usize> = session.toolpath_configs().iter().map(|tc| tc.id).collect();
        let mut trace = empty_trace();
        trace.toolpath_summaries.push(summary_for(ids[0], 10.0));
        trace.toolpath_summaries.push(summary_for(ids[1], 60.0));
        trace.toolpath_summaries.push(summary_for(ids[2], 30.0));

        let cancel = AtomicBool::new(false);
        let report = optimize_project(&mut session, &trace, &NoProgress, &cancel);
        assert_eq!(report.baseline_cycle_time_s, 100.0);
        // TP at session index 1 (id=1) has the largest cycle.
        assert_eq!(report.bottleneck_index, Some(1));
    }

    #[test]
    fn no_bottleneck_when_runtime_evenly_split() {
        // 4 toolpaths at 25% each — none exceeds 30%, so bottleneck
        // is None. The rollup view will show "no single bottleneck".
        let mut session =
            session_with_n_drills(&[("a", true), ("b", true), ("c", true), ("d", true)]);
        let ids: Vec<usize> = session.toolpath_configs().iter().map(|tc| tc.id).collect();
        let mut trace = empty_trace();
        for &id in &ids {
            trace.toolpath_summaries.push(summary_for(id, 10.0));
        }
        let cancel = AtomicBool::new(false);
        let report = optimize_project(&mut session, &trace, &NoProgress, &cancel);
        assert_eq!(report.baseline_cycle_time_s, 40.0);
        assert_eq!(report.bottleneck_index, None);
    }

    #[test]
    fn no_bottleneck_when_total_runtime_zero() {
        // Trace has no per-toolpath cycle data — the bottleneck calc
        // would divide by zero, so we short-circuit to None.
        let mut session = session_with_n_drills(&[("a", true), ("b", true)]);
        let trace = empty_trace();
        let cancel = AtomicBool::new(false);
        let report = optimize_project(&mut session, &trace, &NoProgress, &cancel);
        assert_eq!(report.baseline_cycle_time_s, 0.0);
        assert_eq!(report.bottleneck_index, None);
        assert_eq!(report.per_toolpath.len(), 2);
    }

    #[test]
    fn cancel_before_any_walk_yields_partial_report() {
        // Cancel set before the loop runs. per_toolpath empty;
        // baseline metrics still populated from the trace.
        let mut session = session_with_n_drills(&[("a", true), ("b", true)]);
        let ids: Vec<usize> = session.toolpath_configs().iter().map(|tc| tc.id).collect();
        let mut trace = empty_trace();
        trace.toolpath_summaries.push(summary_for(ids[0], 50.0));
        trace.toolpath_summaries.push(summary_for(ids[1], 50.0));
        let cancel = AtomicBool::new(true);
        let report = optimize_project(&mut session, &trace, &NoProgress, &cancel);
        assert!(report.per_toolpath.is_empty());
        assert_eq!(report.baseline_cycle_time_s, 100.0);
        // Both TPs are at 50% > 30% threshold; bottleneck still
        // computes from the baseline trace, not the per_toolpath
        // result. Tie-break by largest cycle — both equal, so the
        // first one wins (max_by stable).
        assert!(report.bottleneck_index.is_some());
    }

    #[test]
    fn progress_reports_in_order_for_each_enabled_toolpath() {
        let mut session = session_with_n_drills(&[("a", true), ("b", true), ("c", true)]);
        let trace = empty_trace();
        let cancel = AtomicBool::new(false);
        let progress = RecordingProgress::new();
        let _report = optimize_project(&mut session, &trace, &progress, &cancel);
        let calls = progress.into_calls();
        assert_eq!(calls.len(), 3);
        // Completed counts increase 0, 1, 2; total stays at 3.
        for (i, (completed, total, _)) in calls.iter().enumerate() {
            assert_eq!(*completed, i);
            assert_eq!(*total, 3);
        }
    }

    #[test]
    fn bottleneck_threshold_constant_is_thirty_percent() {
        // Lock the constant in place so a future tweak to the
        // threshold gets a deliberate test update.
        assert!((search_policy().bottleneck_fraction.value - 0.30).abs() < 1e-9);
    }

    #[test]
    fn no_progress_implements_trait() {
        // Smoke test that NoProgress satisfies ProgressReporter and
        // can be passed where a `&dyn ProgressReporter` is expected.
        let p: &dyn ProgressReporter = &NoProgress;
        p.report(0, 1, "ignored");
    }

    #[test]
    fn pocket_op_with_empty_trace_yields_skipped_in_walk() {
        // Pocket trips a different skip path inside optimize_toolpath
        // (no steady-state samples for the toolpath_id in the trace).
        // Verify the walk surfaces it as a Skipped row in per_toolpath.
        let mut session = ProjectSession::new_empty();
        session.add_tool(make_tool());
        let tool_id = session.tools()[0].id.0;
        session
            .add_toolpath(
                0,
                make_tc(
                    "p",
                    OperationConfig::Pocket(PocketConfig::default()),
                    tool_id,
                    true,
                ),
            )
            .unwrap();
        // Add an alignment-pin drill to mix in the second skip path.
        session
            .add_toolpath(
                0,
                make_tc(
                    "ap",
                    OperationConfig::AlignmentPinDrill(AlignmentPinDrillConfig::default()),
                    tool_id,
                    true,
                ),
            )
            .unwrap();
        let trace = empty_trace();
        let cancel = AtomicBool::new(false);
        let report = optimize_project(&mut session, &trace, &NoProgress, &cancel);
        assert_eq!(report.per_toolpath.len(), 2);
        assert!(matches!(
            report.per_toolpath[0].1,
            OptimizeOutcome::Skipped { .. }
        ));
        assert!(matches!(
            report.per_toolpath[1].1,
            OptimizeOutcome::Skipped { .. }
        ));
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
    use super::delta::{classify_one_gate_chipload, classify_one_gate_power};
    use super::*;
    use crate::tool_load::verdict::{DeflectionVerdict, PowerVerdict};

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
            attempted: Vec::new(),
        };
        assert!(nsi.first_safe().is_none());
    }

    fn synthetic_candidate(
        feed: f64,
        cycle_time: f64,
        verdict: ToolpathLoadVerdict,
    ) -> OptimizeCandidate {
        use crate::compute::operation_configs::PocketConfig;
        OptimizeCandidate {
            params: OperationConfig::Pocket(PocketConfig {
                feed_rate: feed,
                ..PocketConfig::default()
            }),
            delta: ParamDelta {
                feed_mm_min: Some(feed),
                ..Default::default()
            },
            cycle_time_s: cycle_time,
            verdict,
            stage: SearchStage::Refined,
            reconciled_cycle_time_s: None,
            reconciled_verdict: None,
            gate_deltas: None,
        }
    }

    fn within_power_verdict() -> PowerVerdict {
        use super::super::verdict::{Confidence, SampleEvidence};
        PowerVerdict::Within {
            peak_kw: 0.5,
            available_kw: 0.71,
            evidence: SampleEvidence::empty(),
            confidence: Confidence::Validated,
        }
    }

    fn within_deflection_verdict(peak_mm: f64) -> DeflectionVerdict {
        use super::super::verdict::{Confidence, DeflectionBounds, SampleEvidence};
        DeflectionVerdict::Within {
            peak_mm,
            bounds: DeflectionBounds {
                validated_within_mm: 0.050,
                exceeds_mm: 0.200,
            },
            evidence: SampleEvidence::empty(),
            confidence: Confidence::Validated,
        }
    }

    fn within_chipload_verdict(peak: f64) -> ChiploadVerdict {
        use super::super::verdict::{
            ChipBounds, ChipBoundsSource, ChiploadMetric, ChiploadStatistic, Confidence,
            SampleEvidence,
        };
        ChiploadVerdict::Within {
            approach_to_min: None,
            approach_to_max: ChiploadMetric {
                observed_mm_per_tooth: peak,
                statistic: ChiploadStatistic::PeakInRange,
                evidence: SampleEvidence::empty(),
                bounds: ChipBounds {
                    min_mm_per_tooth: Some(0.038),
                    max_mm_per_tooth: 0.07,
                    source: ChipBoundsSource::VendorLut,
                },
            },
            confidence: Confidence::Validated,
        }
    }

    fn exceeds_chipload_verdict_high(peak: f64) -> ChiploadVerdict {
        use super::super::verdict::{
            ChipBounds, ChipBoundsSource, ChipSide, ChiploadMetric, ChiploadStatistic, Confidence,
            SampleEvidence,
        };
        ChiploadVerdict::Exceeds {
            side: ChipSide::High,
            triggering: ChiploadMetric {
                observed_mm_per_tooth: peak,
                statistic: ChiploadStatistic::PeakHigh,
                evidence: SampleEvidence::at(0),
                bounds: ChipBounds {
                    min_mm_per_tooth: Some(0.038),
                    max_mm_per_tooth: 0.07,
                    source: ChipBoundsSource::VendorLut,
                },
            },
            confidence: Confidence::Validated,
        }
    }

    fn within_verdict() -> ToolpathLoadVerdict {
        ToolpathLoadVerdict {
            toolpath_id: 0,
            chipload: within_chipload_verdict(0.04),
            power: within_power_verdict(),
            deflection: within_deflection_verdict(0.030),
        }
    }

    fn exceeds_chipload_verdict() -> ToolpathLoadVerdict {
        ToolpathLoadVerdict {
            toolpath_id: 0,
            chipload: exceeds_chipload_verdict_high(0.08),
            power: within_power_verdict(),
            deflection: within_deflection_verdict(0.030),
        }
    }

    #[test]
    fn first_safe_finds_faster_safe_candidate() {
        let baseline = synthetic_candidate(1500.0, 100.0, within_verdict());
        let faster_safe = synthetic_candidate(2100.0, 70.0, within_verdict());
        let outcome = OptimizeOutcome::Ranked(vec![baseline, faster_safe]);
        let recommended = outcome.first_safe().expect("should recommend");
        assert!((recommended.cycle_time_s - 70.0).abs() < 1e-9);
    }

    #[test]
    fn first_safe_skips_unsafe_candidates() {
        let baseline = synthetic_candidate(1500.0, 100.0, within_verdict());
        let faster_unsafe = synthetic_candidate(2500.0, 60.0, exceeds_chipload_verdict());
        let faster_safe = synthetic_candidate(2100.0, 70.0, within_verdict());
        // Unsafe is faster but is not safe; recommendation is the safe one.
        let outcome = OptimizeOutcome::Ranked(vec![baseline, faster_unsafe, faster_safe]);
        let recommended = outcome.first_safe().expect("should recommend");
        assert!((recommended.cycle_time_s - 70.0).abs() < 1e-9);
    }

    #[test]
    fn first_safe_returns_none_when_only_slower_candidates() {
        let baseline = synthetic_candidate(1500.0, 100.0, within_verdict());
        let slower_safe = synthetic_candidate(1200.0, 110.0, within_verdict());
        let outcome = OptimizeOutcome::Ranked(vec![baseline, slower_safe]);
        assert!(outcome.first_safe().is_none());
    }

    #[test]
    fn first_safe_returns_none_when_all_candidates_unsafe() {
        let baseline = synthetic_candidate(1500.0, 100.0, within_verdict());
        let faster_unsafe = synthetic_candidate(2500.0, 60.0, exceeds_chipload_verdict());
        let outcome = OptimizeOutcome::Ranked(vec![baseline, faster_unsafe]);
        assert!(outcome.first_safe().is_none());
    }

    #[test]
    fn first_safe_index_returns_position_of_recommended() {
        let baseline = synthetic_candidate(1500.0, 100.0, within_verdict());
        let faster_unsafe = synthetic_candidate(2500.0, 60.0, exceeds_chipload_verdict());
        let faster_safe = synthetic_candidate(2100.0, 70.0, within_verdict());
        let candidates = vec![baseline, faster_unsafe, faster_safe];
        // Index 1 is unsafe, so the recommendation is index 2.
        assert_eq!(
            ProjectOptimizeReport::first_safe_index(&candidates),
            Some(2)
        );
    }

    #[test]
    fn first_safe_index_returns_none_when_no_recommendation() {
        let baseline = synthetic_candidate(1500.0, 100.0, within_verdict());
        let slower_safe = synthetic_candidate(1200.0, 110.0, within_verdict());
        let candidates = vec![baseline, slower_safe];
        assert_eq!(ProjectOptimizeReport::first_safe_index(&candidates), None);
    }

    #[test]
    fn first_safe_index_returns_none_when_only_baseline() {
        let baseline = synthetic_candidate(1500.0, 100.0, within_verdict());
        let candidates = vec![baseline];
        assert_eq!(ProjectOptimizeReport::first_safe_index(&candidates), None);
    }

    #[test]
    fn select_stage2_keeps_top_n_by_cycle_time() {
        let candidates = vec![
            synthetic_candidate(2000.0, 80.0, within_verdict()),
            synthetic_candidate(2100.0, 70.0, within_verdict()),
            synthetic_candidate(1900.0, 90.0, within_verdict()),
            synthetic_candidate(2200.0, 60.0, exceeds_chipload_verdict()),
            synthetic_candidate(1800.0, 100.0, within_verdict()),
        ];
        let top3 = select_stage2_candidates(candidates, 3);
        assert_eq!(top3.len(), 3);
        // Sorted ascending: 60, 70, 80.
        assert!((top3[0].cycle_time_s - 60.0).abs() < 1e-9);
        assert!((top3[1].cycle_time_s - 70.0).abs() < 1e-9);
        assert!((top3[2].cycle_time_s - 80.0).abs() < 1e-9);
    }

    #[test]
    fn select_stage2_does_not_panic_when_fewer_candidates_than_n() {
        let candidates = vec![synthetic_candidate(2000.0, 80.0, within_verdict())];
        let result = select_stage2_candidates(candidates, 3);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn build_outcome_empty_candidates_yields_no_safe_improvement() {
        let baseline = synthetic_candidate(1500.0, 100.0, within_verdict());
        let outcome = build_outcome(baseline, Vec::new());
        match outcome {
            OptimizeOutcome::NoSafeImprovement {
                reason,
                explanation,
                attempted,
            } => {
                assert!(matches!(reason, RefuseReason::NoImprovementFound));
                assert!(
                    explanation.contains("no candidates"),
                    "explanation should say no candidates were produced: {explanation}"
                );
                // Even with no candidates, the baseline is preserved
                // so the modal/rollup can show "this is what you had".
                assert_eq!(attempted.len(), 1);
            }
            other => panic!("expected NoSafeImprovement, got {other:?}"),
        }
    }

    #[test]
    fn build_outcome_all_slower_yields_no_safe_improvement() {
        let baseline = synthetic_candidate(1500.0, 100.0, within_verdict());
        let candidates = vec![
            synthetic_candidate(1300.0, 105.0, within_verdict()),
            synthetic_candidate(1200.0, 110.0, within_verdict()),
        ];
        let outcome = build_outcome(baseline, candidates);
        match outcome {
            OptimizeOutcome::NoSafeImprovement {
                explanation,
                attempted,
                ..
            } => {
                assert!(
                    explanation.contains("no candidate beat the baseline"),
                    "explanation should mention slower-than-baseline: {explanation}"
                );
                // Baseline at index 0 + the two attempted candidates.
                assert_eq!(
                    attempted.len(),
                    3,
                    "attempted must include baseline + 2 candidates"
                );
            }
            other => panic!("expected NoSafeImprovement, got {other:?}"),
        }
    }

    #[test]
    fn build_outcome_all_unsafe_yields_no_safe_improvement() {
        let baseline = synthetic_candidate(1500.0, 100.0, within_verdict());
        let candidates = vec![
            synthetic_candidate(2500.0, 60.0, exceeds_chipload_verdict()),
            synthetic_candidate(2300.0, 65.0, exceeds_chipload_verdict()),
        ];
        let outcome = build_outcome(baseline, candidates);
        match outcome {
            OptimizeOutcome::NoSafeImprovement {
                explanation,
                attempted,
                ..
            } => {
                assert!(
                    explanation.contains("gate limit"),
                    "explanation should mention gate limit: {explanation}"
                );
                assert_eq!(attempted.len(), 3);
                // Sorted by ascending cycle time means index 1 has
                // the lower cycle (60s), index 2 the higher (65s).
                assert!(attempted[1].cycle_time_s <= attempted[2].cycle_time_s);
            }
            other => panic!("expected NoSafeImprovement, got {other:?}"),
        }
    }

    #[test]
    fn build_outcome_with_safe_faster_candidate_yields_ranked() {
        let baseline = synthetic_candidate(1500.0, 100.0, within_verdict());
        let faster_safe = synthetic_candidate(2100.0, 70.0, within_verdict());
        let faster_unsafe = synthetic_candidate(2500.0, 60.0, exceeds_chipload_verdict());
        let candidates = vec![faster_unsafe, faster_safe];
        let outcome = build_outcome(baseline, candidates);
        let OptimizeOutcome::Ranked(ranked) = outcome else {
            panic!("expected Ranked");
        };
        // Baseline at index 0.
        assert!((ranked[0].cycle_time_s - 100.0).abs() < 1e-9);
        // Sorted ascending after baseline: 60.0 (unsafe), 70.0 (safe).
        assert!((ranked[1].cycle_time_s - 60.0).abs() < 1e-9);
        assert!((ranked[2].cycle_time_s - 70.0).abs() < 1e-9);
    }

    #[test]
    fn refuse_reason_explanations_cover_every_variant() {
        // Smoke test: every variant gives a non-empty explanation. If
        // someone adds a variant without an explanation arm, this fails.
        let variants = [
            RefuseReason::SimulationRequired,
            RefuseReason::ArcEngagementNotCaptured,
            RefuseReason::MaterialUnvalidated,
            RefuseReason::NoVendorData,
            RefuseReason::SteadyStateSamplesNotPresent,
            RefuseReason::BipolarEngagement,
            RefuseReason::DeflectionSetupLocked,
            RefuseReason::NoFeasibleRow,
            RefuseReason::RpmBracketEmpty,
            RefuseReason::DiameterExtrapolationTooPoor,
            RefuseReason::NoImprovementFound,
        ];
        for v in variants {
            let s = v.explanation_for_optimize();
            assert!(!s.is_empty(), "variant {v:?} returned empty explanation");
        }
    }

    // ── Per-candidate gate deltas + tier dispatcher (Commit #3) ──────

    fn exceeds_power_verdict() -> ToolpathLoadVerdict {
        use super::super::verdict::{Confidence, SampleEvidence};
        let mut v = within_verdict();
        v.power = PowerVerdict::Exceeds {
            peak_kw: 2.5,
            available_kw: 0.71,
            evidence: SampleEvidence::at(0),
            confidence: Confidence::Validated,
        };
        v
    }

    fn unmodeled_chipload_verdict() -> ToolpathLoadVerdict {
        use super::super::verdict::UnmodeledReason;
        let mut v = within_verdict();
        v.chipload = ChiploadVerdict::Unmodeled {
            reason: UnmodeledReason::NoVendorData,
        };
        v
    }

    #[test]
    fn classify_one_gate_within_to_within_is_same() {
        let b = within_verdict();
        let mut c = within_verdict();
        if let PowerVerdict::Within { peak_kw, .. } = &mut c.power {
            *peak_kw = 0.55;
        }
        assert_eq!(classify_one_gate_power(&b.power, &c.power), GateDelta::Same);
    }

    #[test]
    fn classify_one_gate_exceeds_to_within_is_improved() {
        let b = exceeds_chipload_verdict();
        let c = within_verdict();
        assert_eq!(
            classify_one_gate_chipload(&b.chipload, &c.chipload),
            GateDelta::Improved
        );
    }

    #[test]
    fn classify_one_gate_within_to_exceeds_is_worsened() {
        let b = within_verdict();
        let c = exceeds_chipload_verdict();
        assert_eq!(
            classify_one_gate_chipload(&b.chipload, &c.chipload),
            GateDelta::Worsened
        );
    }

    #[test]
    fn classify_one_gate_exceeds_to_smaller_exceeds_is_improved() {
        let b_v = exceeds_chipload_verdict_high(0.10);
        let c_v = exceeds_chipload_verdict_high(0.08); // strictly smaller, > 5% threshold
        assert_eq!(classify_one_gate_chipload(&b_v, &c_v), GateDelta::Improved);
    }

    #[test]
    fn classify_one_gate_exceeds_to_larger_exceeds_is_worsened() {
        let b_v = exceeds_chipload_verdict_high(0.08);
        let c_v = exceeds_chipload_verdict_high(0.10);
        assert_eq!(classify_one_gate_chipload(&b_v, &c_v), GateDelta::Worsened);
    }

    #[test]
    fn classify_one_gate_unmodeled_either_side_is_unmodeled() {
        let b = unmodeled_chipload_verdict();
        let c = within_verdict();
        assert_eq!(
            classify_one_gate_chipload(&b.chipload, &c.chipload),
            GateDelta::Unmodeled
        );
        assert_eq!(
            classify_one_gate_chipload(&c.chipload, &b.chipload),
            GateDelta::Unmodeled
        );
    }

    #[test]
    fn gate_deltas_helpers() {
        let pure = GateDeltas {
            chipload: GateDelta::Improved,
            power: GateDelta::Same,
            deflection: GateDelta::Same,
        };
        assert!(pure.no_regression());
        assert!(pure.any_improved());
        assert!(!pure.any_worsened());

        let tradeoff = GateDeltas {
            chipload: GateDelta::Improved,
            power: GateDelta::Worsened,
            deflection: GateDelta::Same,
        };
        assert!(!tradeoff.no_regression());
        assert!(tradeoff.any_improved());
        assert!(tradeoff.any_worsened());
    }

    #[test]
    fn build_outcome_pure_improvement_yields_ranked_with_deltas() {
        // Baseline has chipload Exceeds. Candidate fixes it (Improved
        // on chipload, Same on others) and is faster. Should land in
        // Ranked, with gate_deltas populated on the candidate.
        let baseline = synthetic_candidate(1500.0, 100.0, exceeds_chipload_verdict());
        let pure = synthetic_candidate(2100.0, 70.0, within_verdict());
        let outcome = build_outcome(baseline, vec![pure]);
        let OptimizeOutcome::Ranked(ranked) = outcome else {
            panic!("expected Ranked, got {outcome:?}");
        };
        assert!(ranked[0].gate_deltas.is_none(), "baseline has no deltas");
        let deltas = ranked[1].gate_deltas.expect("candidate has deltas");
        assert_eq!(deltas.chipload, GateDelta::Improved);
        assert_eq!(deltas.power, GateDelta::Same);
        assert_eq!(deltas.deflection, GateDelta::Same);
    }

    #[test]
    fn build_outcome_tradeoff_yields_tradeoff_variant() {
        // Baseline trips chipload. Candidate fixes chipload (Improved)
        // but pushes power into Exceeds (Worsened). Faster overall.
        // Pure-improvement check fails; trade-off check fires.
        let baseline = synthetic_candidate(1500.0, 100.0, exceeds_chipload_verdict());
        let mut tradeoff_verdict = within_verdict();
        // Fix chipload but trip power.
        tradeoff_verdict.chipload = within_verdict().chipload;
        tradeoff_verdict.power = exceeds_power_verdict().power;
        let candidate = synthetic_candidate(2200.0, 80.0, tradeoff_verdict);
        let outcome = build_outcome(baseline, vec![candidate]);
        let OptimizeOutcome::TradeOff(tradeoffs) = outcome else {
            panic!("expected TradeOff, got {outcome:?}");
        };
        assert_eq!(tradeoffs.len(), 2, "baseline + 1 trade-off");
        let deltas = tradeoffs[1].gate_deltas.expect("populated");
        assert_eq!(deltas.chipload, GateDelta::Improved);
        assert_eq!(deltas.power, GateDelta::Worsened);
    }

    #[test]
    fn build_outcome_prefers_pure_improvement_over_tradeoff() {
        // Two candidates: one is a trade-off, one is pure improvement.
        // Pure should win — Ranked outcome.
        let baseline = synthetic_candidate(1500.0, 100.0, exceeds_chipload_verdict());
        let pure = synthetic_candidate(2100.0, 75.0, within_verdict());
        let mut tradeoff_verdict = within_verdict();
        tradeoff_verdict.power = exceeds_power_verdict().power;
        let tradeoff_cand = synthetic_candidate(2200.0, 70.0, tradeoff_verdict);
        let outcome = build_outcome(baseline, vec![tradeoff_cand, pure]);
        assert!(
            matches!(outcome, OptimizeOutcome::Ranked(_)),
            "pure improvement must win, got {outcome:?}"
        );
    }

    #[test]
    fn first_safe_returns_none_for_tradeoff_outcome() {
        // first_safe only auto-recommends from Ranked outcomes.
        // TradeOff candidates need explicit user acceptance.
        let baseline = synthetic_candidate(1500.0, 100.0, exceeds_chipload_verdict());
        let mut tradeoff_verdict = within_verdict();
        tradeoff_verdict.power = exceeds_power_verdict().power;
        let candidate = synthetic_candidate(2200.0, 70.0, tradeoff_verdict);
        let outcome = build_outcome(baseline, vec![candidate]);
        assert!(matches!(outcome, OptimizeOutcome::TradeOff(_)));
        assert!(
            outcome.first_safe().is_none(),
            "TradeOff outcomes should not auto-recommend"
        );
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod stage1_grid_tests {
    use super::candidate::{
        build_doc_variants, build_scallop_height_variants, build_stepover_variants, has_doc_knob,
    };
    use super::*;
    use crate::compute::catalog::OperationParams;
    use crate::compute::operation_configs::{
        PencilConfig, ProfileConfig, RampFinishConfig, ScallopConfig, TraceConfig, WaterlineConfig,
        ZigzagConfig,
    };

    #[test]
    fn has_doc_knob_includes_g3_ops() {
        // G3 (2026-05-08) — Trace was always exposed via the trait but
        // missing from `has_doc_knob`; RampFinish and Waterline now
        // wrap their semantically-equivalent Z-step knobs.
        assert!(has_doc_knob(OperationType::Trace));
        assert!(has_doc_knob(OperationType::RampFinish));
        assert!(has_doc_knob(OperationType::Waterline));
        // RadialFinish stays out — its `angular_step` is degrees,
        // structurally different from the existing mm-based axes.
        assert!(!has_doc_knob(OperationType::RadialFinish));
    }

    #[test]
    fn ramp_finish_depth_per_pass_wraps_max_stepdown() {
        let mut r = RampFinishConfig::default();
        let initial = r
            .depth_per_pass()
            .expect("RampFinish should expose depth_per_pass after G3");
        assert!((initial - r.max_stepdown).abs() < 1e-9);
        // Set-through writes max_stepdown so existing planner code
        // that consumes max_stepdown picks up the new value.
        r.set_depth_per_pass(0.8);
        assert!((r.max_stepdown - 0.8).abs() < 1e-9);
        assert!((r.depth_per_pass().expect("present") - 0.8).abs() < 1e-9);
    }

    #[test]
    fn waterline_depth_per_pass_wraps_z_step() {
        let mut w = WaterlineConfig::default();
        let initial = w
            .depth_per_pass()
            .expect("Waterline should expose depth_per_pass after G3");
        assert!((initial - w.z_step).abs() < 1e-9);
        w.set_depth_per_pass(2.0);
        assert!((w.z_step - 2.0).abs() < 1e-9);
    }

    #[test]
    fn pencil_stepover_only_when_multipass() {
        // Default `num_offset_passes = 1` → no stepover knob exposed.
        let single = PencilConfig {
            num_offset_passes: 1,
            ..PencilConfig::default()
        };
        assert!(single.stepover().is_none());
        // Multipass → exposes offset_stepover.
        let mut multi = PencilConfig {
            num_offset_passes: 3,
            ..PencilConfig::default()
        };
        let s = multi.stepover().expect("stepover exposed when multipass");
        assert!((s - multi.offset_stepover).abs() < 1e-9);
        multi.set_stepover(0.75);
        assert!((multi.offset_stepover - 0.75).abs() < 1e-9);
    }

    #[test]
    fn trace_config_exposes_depth_per_pass() {
        // Trace's accessor already existed pre-G3 — this test pins it
        // alongside the `has_doc_knob` membership change so a future
        // refactor can't silently drop Trace from Stage 1 again.
        let t = TraceConfig::default();
        assert!(t.depth_per_pass().is_some());
        assert!(
            t.stepover().is_none(),
            "Trace is a path-follow, no stepover"
        );
    }

    #[test]
    fn scallop_config_exposes_scallop_height_not_stepover() {
        // G2 (2026-05-08): ScallopConfig has no `stepover` field —
        // its spacing knob is `scallop_height` (default 0.1 mm).
        // Stage 1 sweeps this as a third axis distinct from
        // `stepover` because units differ (a 0.1 mm scallop on a
        // 6 mm ball produces ~1.55 mm radial step).
        let s = ScallopConfig::default();
        assert!(
            s.scallop_height().is_some(),
            "Scallop should expose scallop_height"
        );
        assert!(s.stepover().is_none(), "Scallop should NOT expose stepover");
        assert!(
            s.depth_per_pass().is_none(),
            "Scallop should NOT expose depth_per_pass — it's surface-following"
        );
        assert!(
            (s.scallop_height().expect("scallop_height present") - 0.1).abs() < 1e-9,
            "expected default 0.1 mm scallop height"
        );
    }

    #[test]
    fn set_scallop_height_writes_through() {
        let mut s = ScallopConfig::default();
        s.set_scallop_height(0.05);
        assert!((s.scallop_height().expect("scallop_height present") - 0.05).abs() < 1e-9);
    }

    #[test]
    fn build_scallop_height_variants_three_around_baseline() {
        // 0.10 mm baseline → [0.07, 0.10, 0.13].
        let variants = build_scallop_height_variants(0.10);
        assert_eq!(variants.len(), 3, "got {variants:?}");
        assert!((variants[0] - 0.07).abs() < 1e-6);
        assert!((variants[1] - 0.10).abs() < 1e-6);
        assert!((variants[2] - 0.13).abs() < 1e-6);
    }

    #[test]
    fn build_scallop_height_variants_floored_at_minimum() {
        // 0.005 mm baseline (below the 0.01 floor) → all variants
        // collapse to the floor.
        let variants = build_scallop_height_variants(0.005);
        for v in &variants {
            assert!(
                *v >= search_policy().axes.scallop_height.hard_floor.value - 1e-9,
                "got {variants:?}"
            );
        }
    }

    #[test]
    fn has_doc_knob_includes_profile_and_zigzag() {
        // G1 (2026-05-08): Profile and Zigzag both expose
        // `depth_per_pass()` via the trait but were excluded from the
        // original 5-op list, so Stage 1 silently skipped them.
        assert!(has_doc_knob(OperationType::Profile));
        assert!(has_doc_knob(OperationType::Zigzag));
        // Sanity: the original 5 stay in.
        assert!(has_doc_knob(OperationType::Pocket));
        assert!(has_doc_knob(OperationType::Adaptive));
        assert!(has_doc_knob(OperationType::Adaptive3d));
        assert!(has_doc_knob(OperationType::Rest));
        assert!(has_doc_knob(OperationType::Face));
        // Sanity: ops that genuinely lack a DOC knob stay out.
        assert!(!has_doc_knob(OperationType::Drill));
        assert!(!has_doc_knob(OperationType::DropCutter));
        assert!(!has_doc_knob(OperationType::Scallop));
    }

    #[test]
    fn profile_config_exposes_doc_but_not_stepover() {
        // Used by `run_stage_1_grid` to collapse the stepover dimension
        // for ops without a stepover knob — Profile is a contour follow.
        let p = ProfileConfig::default();
        assert!(p.depth_per_pass().is_some(), "Profile should expose DOC");
        assert!(p.stepover().is_none(), "Profile should NOT expose stepover");
    }

    #[test]
    fn zigzag_config_exposes_both_doc_and_stepover() {
        // Zigzag is a full DOC × stepover grid op.
        let z = ZigzagConfig::default();
        assert!(z.depth_per_pass().is_some(), "Zigzag should expose DOC");
        assert!(z.stepover().is_some(), "Zigzag should expose stepover");
    }

    fn synthetic_lut_row(ap_min: Option<f64>, ap_max: Option<f64>) -> MatchedRow {
        MatchedRow {
            chip_load_mm: 0.04,
            chip_load_min_mm: Some(0.025),
            chip_load_max_mm: Some(0.055),
            rpm_nominal: Some(15_000.0),
            rpm_min: Some(14_000.0),
            rpm_max: Some(16_000.0),
            ap_min_mm: ap_min,
            ap_max_mm: ap_max,
            ae_min_mm: None,
            ae_max_mm: None,
            observation_id: "synthetic".to_owned(),
            source_vendor: "Test".to_owned(),
            score: 100,
            diameter_match_score: 200,
            row_diameter_mm: 6.0,
            chipload_diameter_scale: 1.0,
            chipload_hardness_scale: 1.0,
            is_extrapolated: false,
        }
    }

    #[test]
    fn three_variant_op_with_no_lut_row() {
        // Adaptive3d with baseline 3.0mm and no LUT bounds.
        // Expected: [0.7×3.0, 3.0, 1.3×3.0] = [2.1, 3.0, 3.9]
        let variants = build_doc_variants(3.0, None, OperationType::Adaptive3d);
        assert_eq!(variants.len(), 3, "got {variants:?}");
        assert!((variants[0] - 2.1).abs() < 1e-6);
        assert!((variants[1] - 3.0).abs() < 1e-6);
        assert!((variants[2] - 3.9).abs() < 1e-6);
    }

    #[test]
    fn four_variant_op_with_no_lut_row() {
        // Pocket with baseline 1.5mm and no LUT bounds.
        // Expected: [0.7×1.5, 1.5, mid(1.5, 2.1), 2.1] = [1.05, 1.5, 1.8, 2.1]
        let variants = build_doc_variants(1.5, None, OperationType::Pocket);
        assert_eq!(variants.len(), 4, "got {variants:?}");
        assert!((variants[0] - 1.05).abs() < 1e-6);
        assert!((variants[1] - 1.5).abs() < 1e-6);
        assert!((variants[2] - 1.8).abs() < 1e-6);
        assert!((variants[3] - 2.1).abs() < 1e-6);
    }

    #[test]
    fn lut_row_does_not_clamp_warm_start_when_baseline_inside_preferred() {
        // Baseline 3.0mm, LUT ap_min = 2.5mm (above 0.7×3.0 = 2.1).
        // Old behaviour: lo clamped to ap_min = 2.5.
        // G16 Step 4: LUT becomes preferred, not hard. Warm-start uses
        // baseline × mults so lo = 2.1; an outside-preferred probe at
        // ap_min × 0.85 = 2.125 sits between baseline-mult-lo and ap_min.
        let row = synthetic_lut_row(Some(2.5), Some(5.0));
        let variants = build_doc_variants(3.0, Some(&row), OperationType::Adaptive3d);
        // The smallest variant should be the multiplier-anchored 2.1, not the LUT lo.
        assert!(
            variants[0] < 2.5,
            "smallest variant should sit below LUT ap_min (warm-start unclamped): got {variants:?}"
        );
        // Baseline survives.
        assert!(variants.iter().any(|v| (v - 3.0).abs() < 1e-6));
    }

    #[test]
    fn lut_row_does_not_clamp_warm_start_hi_when_baseline_inside_preferred() {
        // Baseline 3.0mm, LUT ap_max = 3.5mm (below 1.3×3.0 = 3.9).
        // Old behaviour: hi clamped to ap_max = 3.5.
        // G16 Step 4: warm-start hi = 3.9, plus an outside-preferred
        // probe at ap_max × 1.15 = 4.025 — search now exceeds the LUT
        // upper bound.
        let row = synthetic_lut_row(Some(1.0), Some(3.5));
        let variants = build_doc_variants(3.0, Some(&row), OperationType::Adaptive3d);
        let last = *variants.last().unwrap();
        assert!(
            last > 3.5,
            "search should now extend above LUT ap_max via outside-preferred probe: got {variants:?}"
        );
    }

    #[test]
    fn always_includes_baseline() {
        // Even when LUT bounds shrink the envelope to almost nothing,
        // baseline always survives.
        let row = synthetic_lut_row(Some(2.95), Some(3.05));
        let variants = build_doc_variants(3.0, Some(&row), OperationType::Adaptive3d);
        assert!(
            variants.iter().any(|v| (v - 3.0).abs() < 1e-6),
            "baseline must be present: got {variants:?}"
        );
    }

    #[test]
    fn tight_lut_envelope_no_longer_collapses_search() {
        // LUT row tight to within microns of baseline. Old behaviour:
        // intersection with mult envelope collapsed to ~baseline.
        // G16 Step 4: warm-start uses pure multipliers so the search
        // still spans [0.7×3.0, 3.0, 1.3×3.0]; the tight LUT bounds
        // generate near-baseline outside-preferred probes that simply
        // dedupe against the warm-start grid.
        let row = synthetic_lut_row(Some(2.9999), Some(3.0001));
        let variants = build_doc_variants(3.0, Some(&row), OperationType::Adaptive3d);
        // We retain a useful spread, not a collapsed pair.
        assert!(
            variants.len() >= 3,
            "warm-start should not collapse against tight LUT: got {variants:?}"
        );
        // Baseline still present, low and high endpoints span around it.
        assert!(variants.first().unwrap() < &3.0);
        assert!(variants.last().unwrap() > &3.0);
    }

    #[test]
    fn floors_at_hard_minimum_for_tiny_baseline() {
        // Baseline 0.01mm — below the hard floor. Floor brings it up.
        let variants = build_doc_variants(0.01, None, OperationType::Pocket);
        // Every variant respects the floor (0.05mm).
        for v in &variants {
            assert!(
                *v >= search_policy().axes.doc.hard_floor.value - 1e-9,
                "got {variants:?}"
            );
        }
    }

    #[test]
    fn lut_row_with_ap_min_above_baseline_does_not_crash() {
        // Degenerate case: LUT row's ap_min > baseline. The user's
        // baseline is below the calibrated range. Grid should sort and
        // dedupe sanely without crashing.
        let row = synthetic_lut_row(Some(5.0), Some(8.0));
        let variants = build_doc_variants(2.0, Some(&row), OperationType::Adaptive3d);
        // Variants are sorted ascending.
        for w in variants.windows(2) {
            assert!(
                w[0] <= w[1] + 1e-9,
                "variants must be sorted: got {variants:?}"
            );
        }
        // Baseline survives.
        assert!(variants.iter().any(|v| (v - 2.0).abs() < 1e-6));
    }

    fn synthetic_lut_row_with_ae(ae_min: Option<f64>, ae_max: Option<f64>) -> MatchedRow {
        let mut row = synthetic_lut_row(None, None);
        row.ae_min_mm = ae_min;
        row.ae_max_mm = ae_max;
        row
    }

    #[test]
    fn stepover_three_variant_op_with_no_lut_row() {
        // Adaptive3d with baseline 0.8mm and no LUT bounds.
        // Multiplier envelope: [0.7×0.8, 0.8, 1.3×0.8] = [0.56, 0.8, 1.04].
        // Factory default anchor for Adaptive3d stepover is 2.0mm — this
        // is far enough from the baseline range that it survives the
        // dedup, giving 4 sorted variants.
        let variants = build_stepover_variants(0.8, None, OperationType::Adaptive3d);
        assert_eq!(variants.len(), 4, "got {variants:?}");
        assert!((variants[0] - 0.56).abs() < 1e-6);
        assert!((variants[1] - 0.8).abs() < 1e-6);
        assert!((variants[2] - 1.04).abs() < 1e-6);
        assert!((variants[3] - 2.0).abs() < 1e-6);
    }

    #[test]
    fn stepover_factory_default_anchored_when_baseline_low() {
        // Regression test for the wanaka case: a user-set stepover well
        // below the operation's factory default should still produce a
        // candidate at the default value, so the optimizer can walk the
        // toolpath back up into the well-known safe envelope.
        let variants = build_stepover_variants(0.84, None, OperationType::Adaptive3d);
        assert!(
            variants.iter().any(|v| (v - 2.0).abs() < 1e-6),
            "expected the Adaptive3d default stepover (2.0) to be \
             included as an anchor candidate, got {variants:?}"
        );
    }

    #[test]
    fn doc_factory_default_anchored_when_baseline_low() {
        // Same regression for DOC: a tiny user baseline should still
        // produce the factory default DOC as a candidate.
        let variants = build_doc_variants(0.5, None, OperationType::Adaptive3d);
        assert!(
            variants.iter().any(|v| (v - 3.0).abs() < 1e-6),
            "expected the Adaptive3d default DOC (3.0) to be included \
             as an anchor candidate, got {variants:?}"
        );
    }

    #[test]
    fn stepover_four_variant_op_with_no_lut_row() {
        // Pocket with baseline 2.0mm and no LUT bounds.
        // Expected: [0.7×2.0, 2.0, mid(2.0, 2.8), 2.8] = [1.4, 2.0, 2.4, 2.8]
        let variants = build_stepover_variants(2.0, None, OperationType::Pocket);
        assert_eq!(variants.len(), 4, "got {variants:?}");
        assert!((variants[0] - 1.4).abs() < 1e-6);
        assert!((variants[1] - 2.0).abs() < 1e-6);
        assert!((variants[2] - 2.4).abs() < 1e-6);
        assert!((variants[3] - 2.8).abs() < 1e-6);
    }

    #[test]
    fn stepover_warm_start_unclamped_by_lut_with_outside_preferred_probes() {
        // Baseline 1.0mm, LUT ae_min 0.8mm, ae_max 1.2mm.
        // Old behaviour: warm-start clamped to LUT [0.8, 1.2].
        // G16 Step 4: warm-start uses pure mults [0.7, 1.0, 1.3]; LUT
        // is preferred and produces outside-preferred probes at 0.68
        // (ae_min × 0.85) and 1.38 (ae_max × 1.15).
        let row = synthetic_lut_row_with_ae(Some(0.8), Some(1.2));
        let variants = build_stepover_variants(1.0, Some(&row), OperationType::Adaptive3d);
        // Warm-start lo = 0.7 must be present (not LUT's 0.8).
        assert!(
            variants.iter().any(|v| (v - 0.7).abs() < 1e-6),
            "warm-start lo (0.7) should be unclamped: got {variants:?}"
        );
        // Warm-start hi = 1.3 must be present (not LUT's 1.2).
        assert!(
            variants.iter().any(|v| (v - 1.3).abs() < 1e-6),
            "warm-start hi (1.3) should be unclamped: got {variants:?}"
        );
        // Outside-preferred probe above LUT: 1.2 × 1.15 = 1.38.
        assert!(
            variants.iter().any(|v| (v - 1.38).abs() < 1e-6),
            "expected outside-preferred probe above LUT: got {variants:?}"
        );
    }

    #[test]
    fn wanaka_tp4_stepover_search_extends_beyond_lut_max() {
        // Regression for the wanaka TP 4 bug (G16 §1.3): baseline 0.84mm
        // Adaptive3d stepover, LUT ae_max = 0.95mm capped the search at
        // [0.59, 0.84, 0.95, 2.0]. After Step 4, search must extend
        // beyond LUT ae_max via outside-preferred probes and unclamped
        // warm-start hi.
        let row = synthetic_lut_row_with_ae(Some(0.40), Some(0.95));
        let variants = build_stepover_variants(0.84, Some(&row), OperationType::Adaptive3d);
        // At least one variant must sit strictly above LUT ae_max.
        assert!(
            variants.iter().any(|v| *v > 0.95 + 1e-6),
            "wanaka TP 4 stepover search should now extend above LUT ae_max=0.95, got {variants:?}"
        );
        // Baseline still present.
        assert!(variants.iter().any(|v| (v - 0.84).abs() < 1e-6));
        // Factory-default anchor (Adaptive3d default 2.0mm) still present.
        assert!(variants.iter().any(|v| (v - 2.0).abs() < 1e-6));
    }

    #[test]
    fn stepover_floors_at_hard_minimum() {
        let variants = build_stepover_variants(0.01, None, OperationType::Pocket);
        for v in &variants {
            assert!(
                *v >= search_policy().axes.stepover.hard_floor.value - 1e-9,
                "got {variants:?}"
            );
        }
    }

    #[test]
    fn stepover_always_includes_baseline() {
        // Tight LUT bounds — baseline must still survive the dedup.
        let row = synthetic_lut_row_with_ae(Some(0.99), Some(1.01));
        let variants = build_stepover_variants(1.0, Some(&row), OperationType::Adaptive3d);
        assert!(
            variants.iter().any(|v| (v - 1.0).abs() < 1e-6),
            "baseline must be present: got {variants:?}"
        );
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod candidate_eval_tests {
    use super::context::{diameter_for_lut_lookup, lut_op_family_from, lut_pass_role_from};
    use super::*;
    use crate::compute::config::FeedsAutoMode;
    use crate::compute::operation_configs::PocketConfig;
    use crate::feeds::vendor_lut::{LutOperationFamily, LutPassRole};
    use crate::feeds::{OperationFamily, PassRole};
    use crate::tool::MillingCutter;

    fn baseline_op() -> OperationConfig {
        OperationConfig::Pocket(PocketConfig {
            feed_rate: 1500.0,
            stepover: 2.0,
            depth_per_pass: 1.5,
            spindle_rpm: Some(18_000),
            ..PocketConfig::default()
        })
    }

    #[test]
    fn delta_against_baseline_detects_only_changed_fields() {
        let base = baseline_op();
        let mut candidate = base.clone();
        candidate.set_feed_rate(2100.0);
        let delta = delta_against_baseline(&base, &candidate);
        assert_eq!(delta.feed_mm_min, Some(2100.0));
        assert!(delta.spindle_rpm.is_none());
        assert!(delta.stepover_mm.is_none());
        assert!(delta.depth_per_pass_mm.is_none());
    }

    #[test]
    fn delta_against_baseline_picks_up_doc_change() {
        let base = baseline_op();
        let mut candidate = base.clone();
        candidate.set_depth_per_pass(2.5);
        candidate.set_stepover(2.4);
        let delta = delta_against_baseline(&base, &candidate);
        assert_eq!(delta.depth_per_pass_mm, Some(2.5));
        assert_eq!(delta.stepover_mm, Some(2.4));
        // Feed and rpm unchanged.
        assert!(delta.feed_mm_min.is_none());
        assert!(delta.spindle_rpm.is_none());
    }

    #[test]
    fn delta_against_baseline_ignores_subhalf_feed_drift() {
        // Tiny floating-point drift (under 0.5 mm/min) should NOT be
        // reported as a change. The 0.5-mm/min floor matches typical
        // CNC controller granularity.
        let base = baseline_op();
        let mut candidate = base.clone();
        candidate.set_feed_rate(1500.000001);
        let delta = delta_against_baseline(&base, &candidate);
        assert!(delta.feed_mm_min.is_none(), "got {delta:?}");
    }

    #[test]
    fn feeds_auto_for_candidate_flips_only_changed_fields() {
        let baseline = FeedsAutoMode::default(); // all true
        let delta = ParamDelta {
            feed_mm_min: Some(2100.0),
            spindle_rpm: None,
            stepover_mm: None,
            depth_per_pass_mm: Some(2.5),
            scallop_height_mm: None,
        };
        let result = feeds_auto_for_candidate(&baseline, &delta);
        assert!(!result.feed_rate, "feed_rate should be flipped to false");
        assert!(!result.depth_per_pass, "depth_per_pass should be flipped");
        // Untouched flags remain true.
        assert!(result.plunge_rate);
        assert!(result.stepover);
        assert!(result.spindle_speed);
    }

    #[test]
    fn feeds_auto_for_candidate_preserves_existing_user_overrides() {
        // User had already manually overridden stepover (false) before
        // Optimize ran. The optimizer is changing feed but not
        // stepover. The stepover override must survive.
        let baseline = FeedsAutoMode {
            feed_rate: true,
            plunge_rate: true,
            stepover: false, // user override
            depth_per_pass: true,
            spindle_speed: true,
        };
        let delta = ParamDelta {
            feed_mm_min: Some(2100.0),
            ..Default::default()
        };
        let result = feeds_auto_for_candidate(&baseline, &delta);
        assert!(!result.feed_rate, "optimize flipped feed_rate");
        assert!(!result.stepover, "user's stepover override preserved");
    }

    #[test]
    fn lut_op_family_mapping_covers_all_variants() {
        // Cover every variant so the match arms can't drift apart from
        // the suggest module's mapping.
        assert_eq!(
            lut_op_family_from(OperationFamily::Adaptive),
            LutOperationFamily::Adaptive
        );
        assert_eq!(
            lut_op_family_from(OperationFamily::Pocket),
            LutOperationFamily::Pocket
        );
        assert_eq!(
            lut_op_family_from(OperationFamily::Contour),
            LutOperationFamily::Contour
        );
        assert_eq!(
            lut_op_family_from(OperationFamily::Parallel),
            LutOperationFamily::Parallel
        );
        assert_eq!(
            lut_op_family_from(OperationFamily::Scallop),
            LutOperationFamily::Scallop
        );
        assert_eq!(
            lut_op_family_from(OperationFamily::Trace),
            LutOperationFamily::Trace
        );
        assert_eq!(
            lut_op_family_from(OperationFamily::Face),
            LutOperationFamily::Face
        );
    }

    #[test]
    fn lut_pass_role_mapping_covers_all_variants() {
        assert_eq!(
            lut_pass_role_from(PassRole::Roughing),
            LutPassRole::Roughing
        );
        assert_eq!(
            lut_pass_role_from(PassRole::SemiFinish),
            LutPassRole::SemiFinish
        );
        assert_eq!(lut_pass_role_from(PassRole::Finish), LutPassRole::Finish);
    }

    /// Build a `ToolDefinition` wrapping any `MillingCutter` for testing.
    /// Stickout / shank / holder values are arbitrary — the LUT-lookup
    /// helper only reads the cutter's diameter / engaged-diameter.
    fn wrap_cutter(cutter: Box<dyn crate::tool::MillingCutter>) -> crate::tool::ToolDefinition {
        crate::tool::ToolDefinition::new(
            cutter,
            6.35, // shank
            20.0, // shank length
            40.0, // holder diameter
            60.0, // stickout
            2,    // flutes
            crate::compute::tool_config::ToolMaterial::Carbide,
        )
    }

    #[test]
    fn diameter_for_lut_lookup_tapered_ball_uses_engaged_at_doc() {
        // Tapered ball nose: ball_dia 2 mm, half-angle 10°, shaft 8 mm,
        // cutting length 30 mm. At a shallow DOC of 0.1 mm we're in
        // the ball region — engaged diameter is much less than the
        // 8 mm shaft.
        let cutter = Box::new(crate::tool::TaperedBallEndmill::new(2.0, 10.0, 8.0, 30.0));
        let tool = wrap_cutter(cutter);

        // Sanity: nominal "diameter" is the shaft (max) — the bug we're
        // fixing is the optimizer matching LUT rows against this value.
        assert!((tool.diameter() - 8.0).abs() < 1e-9);

        let engaged = diameter_for_lut_lookup(&tool, Some(0.1));
        assert!(
            engaged < 1.0,
            "expected engaged diameter <1 mm at DOC=0.1 mm on a 2 mm \
             ball-tip taper, got {engaged}"
        );
        assert!(
            engaged > 0.0,
            "engaged diameter should be positive at DOC>0"
        );
    }

    #[test]
    fn diameter_for_lut_lookup_endmill_returns_nominal_diameter() {
        // Cylindrical endmill: lookup_diameter_at returns nominal at
        // any DOC. The helper must preserve that — non-tapered tools
        // are unaffected by this change.
        let cutter = Box::new(crate::tool::FlatEndmill::new(6.0, 25.0));
        let tool = wrap_cutter(cutter);
        assert!((diameter_for_lut_lookup(&tool, Some(3.0)) - 6.0).abs() < 1e-9);
        assert!((diameter_for_lut_lookup(&tool, Some(0.05)) - 6.0).abs() < 1e-9);
    }

    #[test]
    fn diameter_for_lut_lookup_falls_back_to_nominal_when_doc_missing() {
        // Drilling, V-carve, scallop don't carry depth_per_pass; the
        // helper should still produce a usable diameter rather than
        // refusing the lookup.
        let cutter = Box::new(crate::tool::TaperedBallEndmill::new(2.0, 10.0, 8.0, 30.0));
        let tool = wrap_cutter(cutter);

        assert!((diameter_for_lut_lookup(&tool, None) - 8.0).abs() < 1e-9);
        // Defensive: zero / negative DOC also falls back.
        assert!((diameter_for_lut_lookup(&tool, Some(0.0)) - 8.0).abs() < 1e-9);
        assert!((diameter_for_lut_lookup(&tool, Some(-1.0)) - 8.0).abs() < 1e-9);
        // NaN / infinite likewise.
        assert!((diameter_for_lut_lookup(&tool, Some(f64::NAN)) - 8.0).abs() < 1e-9);
        assert!((diameter_for_lut_lookup(&tool, Some(f64::INFINITY)) - 8.0).abs() < 1e-9);
    }
}
