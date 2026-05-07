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

use crate::compute::catalog::{OperationConfig, OperationType};
use crate::compute::config::{DressupConfig, FeedsAutoMode};
use crate::enriched_mesh::FaceGroupId;
use crate::feeds::geometry::radial_chip_thinning_factor;
use crate::feeds::vendor_lookup::MatchedRow;
use crate::feeds::vendor_lut::{LutOperationFamily, LutPassRole};
use crate::feeds::{OperationFamily, PassRole};
use crate::machine::{MachineProfile, PowerModel};
use crate::session::{ProjectSession, SessionError, SimulationOptions};
use crate::simulation_cut::SimulationCutTrace;
use crate::tool::MillingCutter;

use super::RefuseReason;
use super::verdict::{ToolpathLoadVerdict, Verdict};
use super::{ToolpathLoadContext, evaluate_toolpath};

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
    /// `RECOMMENDATION_CYCLE_DELTA_S`. Returns `None` for `Skipped` /
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
        candidates.iter().skip(1).find(|c| {
            candidate_is_safe(c)
                && c.cycle_time_s + RECOMMENDATION_CYCLE_DELTA_S < baseline.cycle_time_s
        })
    }
}

/// Minimum cycle-time improvement (in seconds) for a candidate to
/// count as a recommendation. Below this delta, the candidate is
/// indistinguishable from baseline given simulator and gate-noise
/// uncertainty — surfacing it as ⭐ would be misleading. 0.5 s is
/// well below any user-perceptible cycle time and well above
/// floating-point noise on small sub-second toolpaths.
pub(crate) const RECOMMENDATION_CYCLE_DELTA_S: f64 = 0.5;

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

impl ProjectOptimizeReport {
    /// Index of the first non-baseline candidate that's safe (no
    /// `Exceeds` verdict on any criterion) and faster than baseline
    /// by more than `RECOMMENDATION_CYCLE_DELTA_S`. Returns `None`
    /// if no such candidate exists. This is the index version of
    /// [`OptimizeOutcome::first_safe`] so callers can mutate the
    /// candidate in place (e.g. populate reconciled values during
    /// U4 reconciliation).
    pub fn first_safe_index(candidates: &[OptimizeCandidate]) -> Option<usize> {
        let baseline = candidates.first()?;
        candidates
            .iter()
            .enumerate()
            .skip(1)
            .find(|(_, c)| {
                candidate_is_safe(c)
                    && c.cycle_time_s + RECOMMENDATION_CYCLE_DELTA_S < baseline.cycle_time_s
            })
            .map(|(i, _)| i)
    }
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
    let baseline_verdict =
        evaluate_toolpath(&baseline_load_ctx, Some(baseline_trace), Some(&machine));
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
    if let Some(refusal) = preflight_classify(
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
    let stage_f_candidate = match baseline_verdict.chipload {
        Verdict::Within { .. } => run_stage_0(
            &mut guard,
            &ctx,
            &baseline_op,
            baseline_rpm,
            &baseline_verdict,
            matched_lut_row.as_ref(),
            &machine,
            cancel,
        ),
        Verdict::Exceeds { .. } => run_stage_f_retarget(
            &mut guard,
            &ctx,
            &baseline_op,
            baseline_rpm,
            matched_lut_row.as_ref(),
            &machine,
            cancel,
        ),
        Verdict::Unmodeled { .. } => None,
    };
    if let Some(c) = stage_f_candidate {
        all_candidates.push(c);
    }

    if cancel.load(Ordering::SeqCst) {
        drop(guard);
        return finalize_partial(baseline_candidate, all_candidates);
    }

    // 8. Stage 1: joint DOC × stepover variant grid for ops with both
    //    knobs.
    let stage_1_candidates = run_stage_1_grid(
        &mut guard,
        &ctx,
        &baseline_op,
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
    let stage2_seeds = select_stage2_candidates(all_candidates, STAGE2_SURVIVOR_COUNT);
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
/// Pure helper — extracted from `optimize_toolpath` for reviewability.
/// The optimizer's pre-flight policy lives in the caller; this just
/// runs the stage.
#[allow(clippy::too_many_arguments)]
fn run_stage_0(
    guard: &mut BaselineRestoreGuard<'_>,
    ctx: &EvaluationContext,
    baseline_op: &OperationConfig,
    baseline_rpm: f64,
    baseline_verdict: &ToolpathLoadVerdict,
    matched_lut_row: Option<&MatchedRow>,
    machine: &MachineProfile,
    cancel: &AtomicBool,
) -> Option<OptimizeCandidate> {
    let baseline_chipload_exceeds = matches!(baseline_verdict.chipload, Verdict::Exceeds { .. });
    if baseline_chipload_exceeds {
        return None;
    }
    let stage0_inputs = Stage0Inputs {
        rpm_baseline: baseline_rpm,
        feed_baseline_mm_min: baseline_op.feed_rate(),
        peak_power_baseline_kw: baseline_peak_power_kw(baseline_verdict),
        machine,
        lut_row: matched_lut_row,
    };
    let k = solve_headroom_scale(&stage0_inputs);
    if k <= 1.0 + 1e-6 {
        return None;
    }
    let scaled_op = apply_scale_to_op(baseline_op, baseline_rpm, k, machine);
    let delta = delta_against_baseline(baseline_op, &scaled_op);
    evaluate_candidate(
        guard,
        ctx,
        scaled_op,
        delta,
        SearchStage::Coarse,
        STAGE1_RESOLUTION_MM,
        cancel,
    )
    .ok()
}

/// Run Stage F's re-target mode for a baseline that trips the
/// chipload gate single-sidedly (Burn or Breakage but not bipolar —
/// the pre-flight already refuses bipolar). Computes a target feed /
/// RPM / plunge from the LUT row's chipload envelope and tries that
/// candidate at coarse resolution. Returns `None` when the LUT row
/// is missing data, the RPM bracket has no machine overlap, the
/// retarget is below the noise floor, or the candidate sim fails.
///
/// Sibling to `run_stage_0` — both produce at most one Stage F
/// candidate. The orchestrator picks which to run based on baseline
/// chipload verdict.
#[allow(clippy::too_many_arguments)]
fn run_stage_f_retarget(
    guard: &mut BaselineRestoreGuard<'_>,
    ctx: &EvaluationContext,
    baseline_op: &OperationConfig,
    baseline_rpm: f64,
    matched_lut_row: Option<&MatchedRow>,
    machine: &MachineProfile,
    cancel: &AtomicBool,
) -> Option<OptimizeCandidate> {
    let row = matched_lut_row?;

    // Engaged diameter at commanded DOC — same convention as the
    // chipload gate uses sample-by-sample.
    let commanded_doc = baseline_op.depth_per_pass().unwrap_or(0.0);
    let engaged_diameter = if commanded_doc > 0.0 {
        ctx.tool.lookup_diameter_at(commanded_doc)
    } else {
        ctx.tool.diameter()
    };

    // Commanded radial engagement = stepover. Ops without a stepover
    // pass 0; RCTF returns 1.0 in that case (treated as full slot).
    let commanded_ae = baseline_op.stepover().unwrap_or(0.0).max(0.0);

    // Material plunge base for the safety cap.
    let plunge_base = ctx.material.plunge_rate_base();

    let solution = solve_chipload_retarget(
        baseline_op,
        baseline_rpm,
        ctx.tool.flute_count,
        engaged_diameter,
        commanded_ae,
        plunge_base,
        row,
        machine,
    )?;

    let candidate_op = apply_retarget_to_op(baseline_op, &solution, machine);
    let delta = delta_against_baseline(baseline_op, &candidate_op);
    evaluate_candidate(
        guard,
        ctx,
        candidate_op,
        delta,
        SearchStage::Coarse,
        STAGE1_RESOLUTION_MM,
        cancel,
    )
    .ok()
}

/// Run Stage 1 (joint DOC × stepover variant grid) for one toolpath.
/// Returns the per-grid-cell candidates that successfully evaluated;
/// the anchor cell (matching the Stage 0 candidate or baseline) is
/// skipped to avoid duplication.
///
/// `stage0_anchor` is the optional Stage 0 candidate — when present,
/// the grid is anchored on its params; otherwise it falls back to
/// `baseline_op`. Returns an empty vector for ops that don't have a
/// DOC knob.
fn run_stage_1_grid(
    guard: &mut BaselineRestoreGuard<'_>,
    ctx: &EvaluationContext,
    baseline_op: &OperationConfig,
    stage0_anchor: Option<&OptimizeCandidate>,
    matched_lut_row: Option<&MatchedRow>,
    cancel: &AtomicBool,
) -> Vec<OptimizeCandidate> {
    use std::sync::atomic::Ordering;

    if !has_doc_knob(ctx.operation_kind) {
        return Vec::new();
    }

    let anchor_op = stage0_anchor
        .map(|c| c.params.clone())
        .unwrap_or_else(|| baseline_op.clone());
    let anchor_doc = anchor_op
        .depth_per_pass()
        .unwrap_or_else(|| baseline_op.depth_per_pass().unwrap_or(1.5));
    let anchor_stepover = anchor_op
        .stepover()
        .unwrap_or_else(|| baseline_op.stepover().unwrap_or(1.0));
    let doc_variants = build_doc_variants(anchor_doc, matched_lut_row, ctx.operation_kind);
    let stepover_variants =
        build_stepover_variants(anchor_stepover, matched_lut_row, ctx.operation_kind);

    let mut out: Vec<OptimizeCandidate> = Vec::new();
    'outer: for &doc in &doc_variants {
        for &stepover in &stepover_variants {
            if cancel.load(Ordering::SeqCst) {
                break 'outer;
            }
            // Skip the anchor combo — it's already represented by the
            // headroom-point candidate (or the baseline).
            if (doc - anchor_doc).abs() < DOC_DEDUP_TOLERANCE_MM
                && (stepover - anchor_stepover).abs() < STEPOVER_DEDUP_TOLERANCE_MM
            {
                continue;
            }
            let candidate_op = apply_stepover_to_op(&apply_doc_to_op(&anchor_op, doc), stepover);
            let delta = delta_against_baseline(baseline_op, &candidate_op);
            if let Ok(candidate) = evaluate_candidate(
                guard,
                ctx,
                candidate_op,
                delta,
                SearchStage::Coarse,
                STAGE1_RESOLUTION_MM,
                cancel,
            ) {
                out.push(candidate);
            }
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

/// Outcome of pre-flight: either a refusal that should short-circuit
/// the optimizer, or `None` to proceed to stages.
struct PreflightRefusal {
    reason: RefuseReason,
    explanation: String,
}

/// Classify the baseline before running any stages. Returns `Some`
/// when the optimizer should refuse early without burning sims.
fn preflight_classify(
    ctx: &EvaluationContext,
    baseline_trace: &SimulationCutTrace,
    operation_feed_rate_mm_min: f64,
    baseline_verdict: &ToolpathLoadVerdict,
    matched_lut_row: Option<&MatchedRow>,
) -> Option<PreflightRefusal> {
    // 1. Deflection — pure setup geometry, unreachable from search space.
    if let Verdict::Exceeds { peak: ld_ratio, .. } = baseline_verdict.deflection {
        return Some(PreflightRefusal {
            reason: RefuseReason::DeflectionSetupLocked,
            explanation: deflection_setup_prescription(&ctx.tool, ld_ratio),
        });
    }

    // 2. Bipolar chipload — needs both LUT bounds to be defined,
    //    otherwise we can't classify against an undefined floor or
    //    ceiling. Many vendor rows publish only `chip_load_max_mm`,
    //    so this gate is opt-in by row coverage.
    if let Some(row) = matched_lut_row
        && let (Some(cl_min), Some(cl_max)) = (row.chip_load_min_mm, row.chip_load_max_mm)
    {
        let steady = super::chipload::steady_state_samples_for_toolpath(
            baseline_trace,
            ctx.toolpath_id,
            operation_feed_rate_mm_min,
        );
        if super::chipload::is_bipolar_engagement(&steady.samples, cl_min, cl_max) {
            return Some(PreflightRefusal {
                reason: RefuseReason::BipolarEngagement,
                explanation: bipolar_prescription(ctx.operation_kind, ctx.op_family),
            });
        }
    }

    None
}

/// Build the user-facing prescription string for a deflection-setup-
/// locked refusal. Reports the actual L/D ratio and the stickout that
/// would bring it back to the safe target (L/D=4).
fn deflection_setup_prescription(tool: &crate::tool::ToolDefinition, ld_ratio: f64) -> String {
    let target_stickout_mm = (tool.diameter() * 4.0).max(0.0);
    format!(
        "tool L/D ratio is {ld_ratio:.1} (above 6.0 limit) — feed/RPM/DOC/stepover \
         can't fix this; shorten stickout to ~{target_stickout_mm:.0} mm (target L/D=4) \
         or use a stiffer tool"
    )
}

/// Build the user-facing prescription string for a bipolar-engagement
/// refusal. The lever depends on whether the operation has a
/// depth-per-pass knob the user can adjust to reduce engagement
/// variance: 2.5D ops with DOC/stepover can usually fix it; 3D
/// finishing ops typically can't and need a setup change.
fn bipolar_prescription(op_kind: OperationType, op_family: OperationFamily) -> String {
    let lever = if has_doc_knob(op_kind) {
        "lower stepover or raise depth-per-pass to reduce engagement variance across the toolpath"
    } else {
        match op_family {
            OperationFamily::Contour | OperationFamily::Trace => {
                "engagement variance is driven by the part geometry — break the operation into \
                 multiple passes at fixed engagement, or use a smaller cutter"
            }
            OperationFamily::Parallel | OperationFamily::Scallop => {
                "this is a 3D finishing op — reduce stepover for tighter passes, or shorten \
                 the cutter to lower setup deflection"
            }
            OperationFamily::Face => {
                "engagement variance on a face op usually means the stock or stepover is \
                 misaligned with the cutter footprint — adjust stepover or face the stock first"
            }
            // Adaptive / Pocket / Adaptive3d are all has_doc_knob — they hit the branch above.
            OperationFamily::Adaptive | OperationFamily::Pocket => {
                "lower stepover or raise depth-per-pass to reduce engagement variance"
            }
        }
    };
    format!(
        "steady-state chipload samples straddle the LUT chipload range \
         (some below the burn floor, some above the breakage ceiling) — \
         no single feed/RPM clears both extremes. {lever}."
    )
}

/// Operation kinds with a `depth_per_pass` knob that Stage 1 sweeps.
/// Per Engineering Default 9 + the 5 ops the plan explicitly calls
/// out (Adaptive3d, Pocket, Adaptive, Rest, Face).
fn has_doc_knob(op_kind: OperationType) -> bool {
    matches!(
        op_kind,
        OperationType::Adaptive3d
            | OperationType::Pocket
            | OperationType::Adaptive
            | OperationType::Rest
            | OperationType::Face
    )
}

/// Extract peak power from the gate's verdict. `None` for `Unmodeled`.
fn baseline_peak_power_kw(verdict: &ToolpathLoadVerdict) -> Option<f64> {
    match verdict.power {
        Verdict::Within { peak, .. } => Some(peak),
        Verdict::Exceeds { peak, .. } => Some(peak),
        Verdict::Unmodeled { .. } => None,
    }
}

/// Determine baseline RPM from the trace's actual samples for this
/// toolpath. Falls back to the operation's `spindle_rpm` field, then
/// to the machine's minimum RPM. The trace's median is preferred over
/// the commanded RPM because feed-override settings or spindle
/// dynamics may have run the toolpath at a slightly different speed
/// than the commanded value.
fn baseline_rpm_from_trace(
    trace: &SimulationCutTrace,
    toolpath_id: usize,
    op_rpm: Option<u32>,
    machine: &MachineProfile,
) -> f64 {
    let mut samples_rpm: Vec<f64> = trace
        .samples
        .iter()
        .filter(|s| s.toolpath_id == toolpath_id && s.is_cutting)
        .map(|s| f64::from(s.spindle_rpm))
        .collect();
    samples_rpm.sort_by(f64::total_cmp);
    if let Some(rpm) = samples_rpm.get(samples_rpm.len() / 2).copied() {
        return rpm;
    }
    op_rpm
        .map(f64::from)
        .unwrap_or_else(|| machine.rpm_range().0)
}

/// Pick the diameter to feed into the LUT lookup for a given
/// commanded DOC. For tools whose engaged diameter varies with axial
/// engagement (tapered ball nose, V-bit), `lookup_diameter_at(doc)`
/// returns the actual engaged diameter; for cylindrical tools (end
/// mill, bull nose, drill, plain ball nose) it equals the nominal
/// diameter. When the operation has no commanded DOC (drilling per
/// peck, V-carve, scallop, ...) or the value is non-positive, fall
/// back to the nominal diameter — matching the LUT row to the shank
/// is still better than rejecting the lookup.
fn diameter_for_lut_lookup(
    tool: &crate::tool::ToolDefinition,
    commanded_doc_mm: Option<f64>,
) -> f64 {
    match commanded_doc_mm {
        Some(doc) if doc.is_finite() && doc > 0.0 => tool.lookup_diameter_at(doc),
        _ => tool.diameter(),
    }
}

/// Look up the best-matching LUT row for the toolpath's tool /
/// material / operation combination. Mirrors `suggest::evaluate`'s
/// LUT plumbing so the optimizer reads from the same calibration data
/// the gate does. Returns `None` for ProjectCurve+VBit/BullNose etc.
/// where `routed_lookup_family` has no target, or for `Custom`
/// material.
fn find_matched_lut_row(
    tool: &crate::tool::ToolDefinition,
    material: &crate::material::Material,
    ctx: &EvaluationContext,
    commanded_doc_mm: Option<f64>,
) -> Option<MatchedRow> {
    let tool_family = super::chipload::tool_family_for(tool.to_geometry_hint());
    let (lut_op_family, lut_pass_role) = super::chipload::routed_lookup_family(
        ctx.operation_kind,
        tool_family,
        ctx.lut_op_family,
        ctx.lut_pass_role,
    )?;
    if matches!(material, crate::material::Material::Custom { .. }) {
        return None;
    }
    let (material_family, hardness_kind, hardness_value) =
        crate::feeds::vendor_normalize::material_to_lut(material);
    let criteria = crate::feeds::vendor_lookup::LookupCriteria {
        tool_family,
        tool_subfamily: None,
        diameter_mm: diameter_for_lut_lookup(tool, commanded_doc_mm),
        flute_count: tool.flute_count,
        material_family,
        hardness_kind: Some(hardness_kind),
        hardness_value: Some(hardness_value),
        operation_family: lut_op_family,
        pass_role: lut_pass_role,
    };
    let lut = super::chipload::embedded_lut();
    crate::feeds::vendor_lookup::enumerate_matching_rows(lut, &criteria)
        .into_iter()
        .next()
}

/// Build a partial `OptimizeOutcome` after cancellation. If we have
/// no candidates yet, surface NoSafeImprovement; otherwise dispatch
/// the candidates we managed to evaluate. The user sees a partial
/// rollup rather than nothing — open-and-walk-away cancellation
/// shouldn't lose the work done before they hit Cancel.
fn finalize_partial(
    baseline: OptimizeCandidate,
    candidates: Vec<OptimizeCandidate>,
) -> OptimizeOutcome {
    if candidates.is_empty() {
        return OptimizeOutcome::NoSafeImprovement {
            reason: RefuseReason::NoImprovementFound,
            explanation: "cancelled before any candidates were evaluated".to_owned(),
            attempted: vec![baseline],
        };
    }
    build_outcome(baseline, candidates)
}

// ── Baseline-restore guard (Engineering Default 10) ───────────────────
//
// Each candidate evaluation in the optimizer mutates
// `session.toolpath_configs[idx].operation` (via
// `apply_toolpath_param_snapshot`), regenerates, and runs a fresh sim.
// Without explicit cleanup, the session is left holding the last
// candidate's params when the optimizer returns — a silent state leak
// of the same flavour as the `feeds_auto` LUT-overwrite issue.
//
// `BaselineRestoreGuard` snapshots `(operation, dressups,
// face_selection, feeds_auto)` at construction and re-applies them in
// `Drop`, regardless of how the optimizer exits (Ok, refusal, cancel,
// or panic in `execute_operation`). Apply remains a separate
// user-initiated mutation; the optimizer's internal candidate writes
// never persist past `optimize_toolpath` returning.

/// Snapshot of the four toolpath fields the optimizer mutates per
/// candidate. Captured up-front and re-applied via
/// `apply_toolpath_param_snapshot` on drop.
#[derive(Debug, Clone)]
pub(crate) struct ToolpathParamsSnapshot {
    pub operation: OperationConfig,
    pub dressups: DressupConfig,
    pub face_selection: Option<Vec<FaceGroupId>>,
    pub feeds_auto: FeedsAutoMode,
}

impl ToolpathParamsSnapshot {
    fn capture(session: &ProjectSession, toolpath_index: usize) -> Result<Self, SessionError> {
        let tc = session
            .get_toolpath_config(toolpath_index)
            .ok_or(SessionError::ToolpathNotFound(toolpath_index))?;
        Ok(Self {
            operation: tc.operation.clone(),
            dressups: tc.dressups.clone(),
            face_selection: tc.face_selection.clone(),
            feeds_auto: tc.feeds_auto.clone(),
        })
    }
}

/// RAII guard that restores the toolpath's params to a captured
/// baseline on drop. Construct with [`Self::new`], use
/// [`Self::session_mut`] for any mid-search session mutations, and let
/// the value go out of scope to trigger restoration.
///
/// The guard holds the only `&mut ProjectSession` in flight while it
/// lives; access the session via `session_mut()` so the borrow chain
/// stays through the guard. Drop calls
/// `apply_toolpath_param_snapshot` and ignores its `Result` —
/// restoration failure can only happen if the toolpath was removed
/// mid-search, which would already be a programming error.
pub(crate) struct BaselineRestoreGuard<'a> {
    session: &'a mut ProjectSession,
    toolpath_index: usize,
    snapshot: ToolpathParamsSnapshot,
}

impl<'a> BaselineRestoreGuard<'a> {
    /// Capture the current params and wrap the session. Returns
    /// `ToolpathNotFound` if `toolpath_index` is out of range.
    pub(crate) fn new(
        session: &'a mut ProjectSession,
        toolpath_index: usize,
    ) -> Result<Self, SessionError> {
        let snapshot = ToolpathParamsSnapshot::capture(session, toolpath_index)?;
        Ok(Self {
            session,
            toolpath_index,
            snapshot,
        })
    }

    /// Mutable session reference for in-search mutations. The borrow
    /// stays through the guard so restoration on drop sees a valid
    /// session.
    pub(crate) fn session_mut(&mut self) -> &mut ProjectSession {
        self.session
    }

    /// Read-only snapshot of the captured baseline. Useful for
    /// computing `ParamDelta` against the original params.
    pub(crate) fn baseline(&self) -> &ToolpathParamsSnapshot {
        &self.snapshot
    }
}

impl Drop for BaselineRestoreGuard<'_> {
    fn drop(&mut self) {
        let _ = self.session.apply_toolpath_param_snapshot(
            self.toolpath_index,
            self.snapshot.operation.clone(),
            self.snapshot.dressups.clone(),
            self.snapshot.face_selection.clone(),
            self.snapshot.feeds_auto.clone(),
        );
    }
}

// ── Stage 0: analytical RPM/feed scaling ──────────────────────────────
//
// The closed-form: `k = min(four_ratios)` where the four ratios come
// from machine-RPM, machine-feed, machine-power, and LUT-row-RPM caps.
// Chipload is invariant under proportional (RPM, feed) scaling, so this
// is *only* an optimisation when the baseline is `Within` — scaling
// preserves chipload and can't fix an `Exceeds` baseline. Caller
// responsibility: skip Stage 0 and route directly to Stage 1 when the
// baseline carries an `Exceeds` chipload verdict.

/// Inputs to Stage 0's analytical scaling. Bundled so the function
/// stays terse and the call sites in the orchestrator are explicit
/// about where each value comes from.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Stage0Inputs<'a> {
    /// Baseline spindle RPM the optimizer scales from. Prefer the
    /// trace's actual median RPM over the operation config's
    /// `spindle_rpm` field — the trace is what actually ran.
    pub rpm_baseline: f64,
    /// Baseline commanded feed in mm/min (from the operation config).
    pub feed_baseline_mm_min: f64,
    /// Peak power in kW from the baseline trace, as measured by the
    /// power gate (`power::evaluate` returns this in `Verdict::Within
    /// { peak }` or `Exceeds { peak }`). `None` if the gate refused
    /// (Unmodeled) — Stage 0 cannot scale safely without a power
    /// reference.
    pub peak_power_baseline_kw: Option<f64>,
    pub machine: &'a MachineProfile,
    /// Matched LUT row carrying the calibrated RPM bracket. `None`
    /// drops the LUT-RPM constraint from the scale solve (the optimizer
    /// will rely on machine bounds alone).
    pub lut_row: Option<&'a MatchedRow>,
}

/// Solve the maximum scale factor `k` that keeps all four limits
/// satisfied at constant chipload. Returns `1.0` if no headroom is
/// available (every cap is already binding at baseline).
///
/// Note: this is a pure arithmetic helper. The orchestrator must
/// independently decide whether to run Stage 0 at all (skip it for
/// `Exceeds`-baseline TPs per Engineering Default 6).
pub(crate) fn solve_headroom_scale(inputs: &Stage0Inputs<'_>) -> f64 {
    let rpm_baseline = inputs.rpm_baseline.max(1.0);
    let feed_baseline = inputs.feed_baseline_mm_min.max(1.0);

    // 1. Machine RPM cap.
    let (_min_rpm, max_rpm) = inputs.machine.rpm_range();
    let k_rpm_machine = max_rpm / rpm_baseline;

    // 2. Machine feed cap.
    let k_feed = inputs.machine.max_feed_mm_min / feed_baseline;

    // 3. Power cap. `machine_max_power_kw × safety / peak_baseline`
    //    works for both `ConstantPower` (rhs is constant) and
    //    `VfdConstantTorque` (below rated_rpm both sides scale with k
    //    so the inequality is invariant; above rated_rpm the rhs caps
    //    at rated_power so the bound is the same scalar).
    let k_power = match inputs.peak_power_baseline_kw {
        Some(peak) if peak > 0.0 => {
            machine_max_power_kw(inputs.machine) * inputs.machine.safety_factor / peak
        }
        _ => f64::INFINITY,
    };

    // 4. LUT-row RPM cap. Use rpm_max if present; fall back to
    //    rpm_nominal × 1.2 (Engineering Default 5's ±20% bracket); fall
    //    back further to the machine ceiling (no LUT constraint).
    let k_lut = match inputs.lut_row {
        Some(row) => {
            let lut_rpm_max = row
                .rpm_max
                .or_else(|| row.rpm_nominal.map(|n| n * 1.2))
                .unwrap_or(max_rpm);
            lut_rpm_max / rpm_baseline
        }
        None => f64::INFINITY,
    };

    let k_unclamped = k_rpm_machine.min(k_feed).min(k_power).min(k_lut);
    // Floor at 1.0 — never propose scaling DOWN from baseline in
    // Stage 0. (Down-scaling is Stage 1's job for Exceeds baselines.)
    k_unclamped.max(1.0)
}

/// Maximum power the spindle is capable of delivering at any RPM.
/// `ConstantPower` reports its flat figure; `VfdConstantTorque` reports
/// rated power (the cap above rated RPM).
fn machine_max_power_kw(machine: &MachineProfile) -> f64 {
    match machine.power {
        PowerModel::ConstantPower { power_kw } => power_kw,
        PowerModel::VfdConstantTorque { rated_power_kw, .. } => rated_power_kw,
    }
}

/// Build the headroom-point `OperationConfig` by applying scale `k` to
/// the baseline op's feed and RPM. All other fields are left unchanged
/// — the search only touches feed/RPM in Stage 0. The new RPM is
/// rounded and clamped to the machine's discrete or variable range.
pub(crate) fn apply_scale_to_op(
    baseline_op: &OperationConfig,
    rpm_baseline: f64,
    k: f64,
    machine: &MachineProfile,
) -> OperationConfig {
    let mut scaled = baseline_op.clone();
    scaled.set_feed_rate(baseline_op.feed_rate() * k);
    let scaled_rpm = machine.clamp_rpm(rpm_baseline * k).round() as u32;
    scaled.set_spindle_rpm(Some(scaled_rpm));
    scaled
}

// ── Stage F re-target (chipload-Exceeds baselines) ────────────────────
//
// Stage 0 (headroom scale-up) is the Stage F mode for Within
// baselines. The re-target mode runs when the baseline trips the
// chipload gate single-sidedly (Burn or Breakage but not bipolar — the
// pre-flight in #1 already refuses bipolar). It computes a target
// chipload from the LUT row, compensates for radial chip thinning
// (RCTF) at the commanded engagement, and re-derives feed and RPM
// inside the machine + LUT envelope.

/// Solution returned by `solve_chipload_retarget`. Absolute target
/// values, not a multiplicative scale.
#[derive(Debug, Clone, Copy)]
struct RetargetSolution {
    target_feed_mm_min: f64,
    target_rpm: f64,
    /// Updated plunge rate, when the feed change is meaningful enough
    /// (>10%) to warrant tracking. `None` means "leave plunge alone."
    target_plunge_mm_min: Option<f64>,
}

/// Pick the target effective chipload from the LUT row's published
/// bounds. Returns `None` when the row carries neither bound — there
/// is nothing to retarget against.
fn lut_chipload_target(row: &MatchedRow) -> Option<f64> {
    match (row.chip_load_min_mm, row.chip_load_max_mm) {
        (Some(lo), Some(hi)) if lo > 0.0 && hi >= lo => Some((lo + hi) * 0.5),
        (None, Some(hi)) if hi > 0.0 => Some(hi * 0.85),
        (Some(lo), None) if lo > 0.0 => Some(lo * 1.15),
        _ => None,
    }
}

/// Pick the target RPM for retarget. Prefers the LUT row's
/// `rpm_nominal` when present, else the baseline RPM, then clamps
/// against the machine envelope and the LUT row's `[rpm_min, rpm_max]`
/// bracket if defined. Returns `None` when the LUT bracket has no
/// overlap with the machine range — that case is `RpmBracketEmpty`
/// and the orchestrator should refuse.
fn solve_target_rpm(baseline_rpm: f64, row: &MatchedRow, machine: &MachineProfile) -> Option<f64> {
    let (machine_min, machine_max) = machine.rpm_range();

    // 1. LUT bracket: prefer rpm_nominal; fall back to mid(min,max).
    //    The Engineering Default 5 ±20% bracket isn't applied here —
    //    retarget is selecting an exact target, not a cap.
    let lut_target = row.rpm_nominal.or({
        match (row.rpm_min, row.rpm_max) {
            (Some(lo), Some(hi)) => Some((lo + hi) * 0.5),
            (Some(v), None) | (None, Some(v)) => Some(v),
            (None, None) => None,
        }
    });

    let target = lut_target.unwrap_or(baseline_rpm);

    // 2. Clamp by machine + LUT bracket. If the LUT row's bracket
    //    has no overlap with the machine range, that's RpmBracketEmpty.
    let lut_lo = row.rpm_min.unwrap_or(machine_min);
    let lut_hi = row.rpm_max.unwrap_or(machine_max);
    let effective_min = lut_lo.max(machine_min);
    let effective_max = lut_hi.min(machine_max);
    if effective_min > effective_max {
        return None;
    }
    Some(target.clamp(effective_min, effective_max))
}

/// Compute a retarget solution for a chipload-Exceeds baseline.
/// Targets the LUT row's nominal chipload, compensated by RCTF for
/// the commanded radial engagement, with RPM clamped by both the
/// machine envelope and the LUT row's RPM bracket.
///
/// Returns `None` when:
///   - the LUT row has no usable chipload bounds (NoFeasibleRow case)
///   - the LUT row's RPM bracket has no overlap with the machine
///     range (RpmBracketEmpty)
///   - the resulting feed would be non-finite or non-positive
///   - the change from baseline feed is below 1% (no meaningful
///     retarget — Stage 1 grid will pick up the rest)
#[allow(clippy::too_many_arguments)]
fn solve_chipload_retarget(
    baseline_op: &OperationConfig,
    baseline_rpm: f64,
    flute_count: u32,
    engaged_diameter_mm: f64,
    commanded_ae_mm: f64,
    plunge_base_mm_min: f64,
    lut_row: &MatchedRow,
    machine: &MachineProfile,
) -> Option<RetargetSolution> {
    // 1. Target effective (post-thinning) chipload from the LUT row.
    let target_chipload_eff = lut_chipload_target(lut_row)?;

    // 2. Compensate for radial chip thinning. We aim for the feed
    //    that produces the LUT-target effective chipload AT the
    //    commanded engagement, so the nominal chipload (= feed /
    //    (rpm × flutes)) must be raised by RCTF.
    let rctf = radial_chip_thinning_factor(commanded_ae_mm, engaged_diameter_mm);
    let target_chipload_nominal = target_chipload_eff * rctf;

    // 3. Pick a target RPM inside the machine + LUT envelope.
    let target_rpm = solve_target_rpm(baseline_rpm, lut_row, machine)?;

    // 4. Compute target feed = chipload × rpm × flutes.
    let target_feed_raw = target_chipload_nominal * target_rpm * f64::from(flute_count);
    if !target_feed_raw.is_finite() || target_feed_raw <= 0.0 {
        return None;
    }
    // Clamp by machine feed envelope. Below the machine min isn't
    // valid; if the unclamped target is above the machine max we
    // accept the clamped value — this is still a meaningful retarget
    // even when the machine ceiling is the binding cap.
    let target_feed = target_feed_raw.min(machine.max_feed_mm_min).max(1.0);

    // 5. Skip noise-level retargets — Stage 1 grid will explore from
    //    baseline if the change is too small to matter.
    let baseline_feed = baseline_op.feed_rate().max(1.0);
    if ((target_feed - baseline_feed) / baseline_feed).abs() < 0.01 {
        return None;
    }

    // 6. Plunge tracking. Plunge is derived from feed in the F&S
    //    calculator; when the optimizer moves feed by >10%, plunge
    //    has to come along or the next regen will rebuild a transient
    //    that's no longer steady-state. Cap by `material plunge_base
    //    × machine.safety_factor`.
    let feed_ratio = target_feed / baseline_feed;
    let target_plunge_mm_min = if (feed_ratio - 1.0).abs() > 0.10 {
        let baseline_plunge = baseline_op.plunge_rate();
        let scaled = baseline_plunge * feed_ratio;
        let cap = (plunge_base_mm_min * machine.safety_factor).max(0.0);
        Some(scaled.clamp(0.0, cap.max(scaled.min(plunge_base_mm_min))))
    } else {
        None
    };

    Some(RetargetSolution {
        target_feed_mm_min: target_feed,
        target_rpm,
        target_plunge_mm_min,
    })
}

/// Build the retarget `OperationConfig` from a `RetargetSolution`.
/// Mirrors `apply_scale_to_op` but uses absolute targets and
/// optionally writes a new plunge rate.
fn apply_retarget_to_op(
    baseline_op: &OperationConfig,
    solution: &RetargetSolution,
    machine: &MachineProfile,
) -> OperationConfig {
    let mut out = baseline_op.clone();
    out.set_feed_rate(solution.target_feed_mm_min);
    let rpm = machine.clamp_rpm(solution.target_rpm).round() as u32;
    out.set_spindle_rpm(Some(rpm));
    if let Some(plunge) = solution.target_plunge_mm_min {
        out.set_plunge_rate(plunge);
    }
    out
}

// ── Stage 1: DOC candidate generation (Engineering Default 9) ─────────
//
// Five geometry-bearing ops carry a `depth_per_pass`; the optimizer
// sweeps DOC at the headroom point (baseline `depth_per_pass`) and
// scores each variant via the gate over a fresh sim.
//
// Grid construction prefers the matched LUT row's calibrated bounds
// (`ap_min_mm`, `ap_max_mm`) where available, clamping a multiplier
// envelope (0.7×–1.4× of baseline) inside them. When the LUT row
// doesn't carry bounds, the multiplier envelope is the grid directly.
//
// 3-variant ops (Adaptive3d, Rest, Face): `[lo, base, hi]`.
// 4-variant ops (Pocket, Adaptive): `[lo, base, mid(base, hi), hi]`.

/// Hard floor on any DOC value the optimizer proposes. Real router
/// toolpaths can pass values smaller than this for finishing operations,
/// but Stage 1's job is the rougher 5 ops where ~50µm is a reasonable
/// minimum. ED 9 calls this out as "use 0.05 mm as a hard floor".
const DOC_HARD_FLOOR_MM: f64 = 0.05;

/// Equality threshold for deduping near-identical DOC values produced
/// by the LUT-anchored grid (e.g. when `ap_min`/`ap_max` happen to
/// land microns from the multiplier endpoints). 5 µm is well below
/// any simulator-distinguishable resolution; deliberately tight so
/// the spec'd `[0.7×, 1.0×, 1.3×]` grid survives intact even when
/// floating-point arithmetic puts adjacent values 0.0500000001 mm
/// apart.
const DOC_DEDUP_TOLERANCE_MM: f64 = 0.005;

/// Hard floor on stepover. Same reasoning as `DOC_HARD_FLOOR_MM` —
/// the optimizer is calibrated for the 5 roughing/clearing ops where
/// sub-50µm stepover is finishing territory and outside the LUT
/// envelope.
const STEPOVER_HARD_FLOOR_MM: f64 = 0.05;

/// Dedup tolerance for stepover variants. Same rationale as
/// `DOC_DEDUP_TOLERANCE_MM`.
const STEPOVER_DEDUP_TOLERANCE_MM: f64 = 0.005;

/// Build the DOC candidate grid for a Stage-1 sweep. Always includes
/// the baseline (`baseline_doc_mm`) as a control candidate; the lo and
/// hi endpoints come from the LUT row's calibrated bounds (clamped by
/// the multiplier envelope) or fall back to the multiplier envelope
/// alone.
///
/// Always also includes the operation's own factory default DOC as an
/// "anchor" candidate so a wildly-out-of-envelope baseline (e.g. user
/// set 0.5mm DOC on an op whose default is 3.0mm) can still walk back
/// to the well-known-safe value the variant search would otherwise miss.
///
/// Returns variants sorted ascending. May return fewer than the
/// nominal 3 or 4 entries when adjacent values dedupe (e.g. an LUT
/// row whose `ap_max` lies inside `1.4 × baseline` ends up with the
/// hi endpoint very close to baseline). Always contains at least the
/// baseline value.
pub(crate) fn build_doc_variants(
    baseline_doc_mm: f64,
    lut_row: Option<&MatchedRow>,
    op_type: OperationType,
) -> Vec<f64> {
    let baseline = baseline_doc_mm.max(DOC_HARD_FLOOR_MM);
    let four_variant = matches!(op_type, OperationType::Pocket | OperationType::Adaptive);

    // Multiplier envelope. ED 9: 0.7× to 1.3× for 3-variant; 0.7× to
    // 1.4× for 4-variant (so the midpoint between base and hi lands at
    // 1.2× — a useful intermediate step).
    let mult_lo = 0.7 * baseline;
    let mult_hi = if four_variant {
        1.4 * baseline
    } else {
        1.3 * baseline
    };

    // Clamp inside LUT-row calibrated bounds if available. The
    // `max(ap_min, mult_lo)` choice mirrors ED 9 directly — never go
    // below the calibrated floor *or* below 0.7× baseline.
    let (lo, hi) = match lut_row {
        Some(row) => {
            let ap_min = row.ap_min_mm.unwrap_or(mult_lo);
            let ap_max = row.ap_max_mm.unwrap_or(mult_hi);
            (ap_min.max(mult_lo), ap_max.min(mult_hi))
        }
        None => (mult_lo, mult_hi),
    };

    // Floor every value at the hard minimum, and ensure the high
    // endpoint isn't accidentally smaller than baseline (degenerate
    // case where the LUT row is very tight).
    let lo = lo.max(DOC_HARD_FLOOR_MM);
    let hi = hi.max(baseline).max(DOC_HARD_FLOOR_MM);

    let mut variants = vec![lo, baseline];
    if four_variant {
        variants.push((baseline + hi) * 0.5);
    }
    variants.push(hi);

    // Anchor: the operation's factory default. Brings the optimizer
    // back into the well-tested envelope when the user's baseline drifted
    // far from it (ex: TP 1 wanaka — adaptive3d default 3mm, baseline
    // 3mm; benign here). Always added; dedup handles the no-op case.
    if let Some(default_doc) = OperationConfig::new_default(op_type).depth_per_pass()
        && default_doc.is_finite()
        && default_doc > DOC_HARD_FLOOR_MM
    {
        variants.push(default_doc);
    }

    variants.sort_by(f64::total_cmp);
    variants.dedup_by(|a, b| (*a - *b).abs() < DOC_DEDUP_TOLERANCE_MM);
    variants
}

/// Apply a candidate DOC value to a baseline op, leaving every other
/// field unchanged. The op must be one of the 5 families that exposes
/// `depth_per_pass` via the `OperationParams` trait.
pub(crate) fn apply_doc_to_op(baseline_op: &OperationConfig, doc_mm: f64) -> OperationConfig {
    let mut variant = baseline_op.clone();
    variant.set_depth_per_pass(doc_mm);
    variant
}

/// Build the stepover candidate grid for a Stage-1 sweep. Same shape
/// as `build_doc_variants` but anchored to the LUT row's
/// `ae_min_mm`/`ae_max_mm` (radial engagement) bounds instead of
/// `ap_*`. Always includes the baseline (`baseline_stepover_mm`).
///
/// 4-variant ops (Pocket, Adaptive) get `[lo, baseline, mid, hi]` so
/// the search has more granularity on the dominant clearing
/// strategies; 3-variant ops (Adaptive3d, Rest, Face) get
/// `[lo, baseline, hi]`. Returns variants sorted ascending. May
/// return fewer than the nominal entries when adjacent values dedupe.
pub(crate) fn build_stepover_variants(
    baseline_stepover_mm: f64,
    lut_row: Option<&MatchedRow>,
    op_type: OperationType,
) -> Vec<f64> {
    let baseline = baseline_stepover_mm.max(STEPOVER_HARD_FLOOR_MM);
    let four_variant = matches!(op_type, OperationType::Pocket | OperationType::Adaptive);

    let mult_lo = 0.7 * baseline;
    let mult_hi = if four_variant {
        1.4 * baseline
    } else {
        1.3 * baseline
    };

    let (lo, hi) = match lut_row {
        Some(row) => {
            let ae_min = row.ae_min_mm.unwrap_or(mult_lo);
            let ae_max = row.ae_max_mm.unwrap_or(mult_hi);
            (ae_min.max(mult_lo), ae_max.min(mult_hi))
        }
        None => (mult_lo, mult_hi),
    };

    let lo = lo.max(STEPOVER_HARD_FLOOR_MM);
    let hi = hi.max(baseline).max(STEPOVER_HARD_FLOOR_MM);

    let mut variants = vec![lo, baseline];
    if four_variant {
        variants.push((baseline + hi) * 0.5);
    }
    variants.push(hi);

    // Anchor: the operation's factory default stepover. Without this,
    // a user-baseline that's drifted far below (or above) the safe
    // envelope is unrecoverable by the local search — the multipliers
    // stay near baseline. Wanaka exhibited this: adaptive3d default
    // stepover is 2.0mm, but TP 1's baseline of 0.84mm bounded the
    // search to [0.59, 1.18], well below the 2.0 the chipload gate
    // needed. Always added; dedup handles the no-op case.
    if let Some(default_so) = OperationConfig::new_default(op_type).stepover()
        && default_so.is_finite()
        && default_so > STEPOVER_HARD_FLOOR_MM
    {
        variants.push(default_so);
    }

    variants.sort_by(f64::total_cmp);
    variants.dedup_by(|a, b| (*a - *b).abs() < STEPOVER_DEDUP_TOLERANCE_MM);
    variants
}

/// Apply a candidate stepover value to a baseline op. The op must be
/// one of the 5 families that exposes `stepover` via the
/// `OperationParams` trait.
pub(crate) fn apply_stepover_to_op(
    baseline_op: &OperationConfig,
    stepover_mm: f64,
) -> OperationConfig {
    let mut variant = baseline_op.clone();
    variant.set_stepover(stepover_mm);
    variant
}

// ── Candidate evaluation: apply → regen → sim → gate ──────────────────
//
// The single per-candidate driver. Apply the candidate's params via the
// restore guard, regen the toolpath, run a project sim at the
// requested resolution, and produce an `OptimizeCandidate` with
// sim-measured cycle time and gate verdict. Every Stage-1 and Stage-2
// candidate goes through this function — there is no second sim or
// second gate anywhere.

/// Build the param-delta describing how a candidate differs from the
/// baseline op. Used both for display in the modal and to drive the
/// `feeds_auto.*` flag clearing on apply.
pub(crate) fn delta_against_baseline(
    baseline: &OperationConfig,
    candidate: &OperationConfig,
) -> ParamDelta {
    let mut delta = ParamDelta::default();
    if (baseline.feed_rate() - candidate.feed_rate()).abs() > 0.5 {
        delta.feed_mm_min = Some(candidate.feed_rate());
    }
    if baseline.spindle_rpm() != candidate.spindle_rpm() {
        delta.spindle_rpm = candidate.spindle_rpm();
    }
    if baseline.stepover() != candidate.stepover()
        && let Some(s) = candidate.stepover()
    {
        delta.stepover_mm = Some(s);
    }
    if baseline.depth_per_pass() != candidate.depth_per_pass()
        && let Some(d) = candidate.depth_per_pass()
    {
        delta.depth_per_pass_mm = Some(d);
    }
    delta
}

/// Compose a `feeds_auto` for the candidate apply: clone the baseline
/// flags, then flip `false` for any field the candidate is changing.
/// During eval the candidate's params are ephemeral (the restore guard
/// puts the baseline back on drop), but flipping these flags here
/// prevents the GUI's LUT auto-write from clobbering the candidate
/// mid-sim if the user happens to be on the Feeds tab. After Apply,
/// the same flags persist, which is what we want — the user's chosen
/// candidate should not be silently overwritten.
///
/// Public because the GUI's Apply handler in viz needs the same
/// translation when committing the user's selection.
pub fn feeds_auto_for_candidate(baseline: &FeedsAutoMode, delta: &ParamDelta) -> FeedsAutoMode {
    let mut out = baseline.clone();
    if delta.feed_mm_min.is_some() {
        out.feed_rate = false;
    }
    if delta.spindle_rpm.is_some() {
        out.spindle_speed = false;
    }
    if delta.stepover_mm.is_some() {
        out.stepover = false;
    }
    if delta.depth_per_pass_mm.is_some() {
        out.depth_per_pass = false;
    }
    out
}

/// Map the operation's `feeds_family` (used by the F&S calculator) to
/// the `LutOperationFamily` (used by the chipload/power gate's LUT
/// lookup). Same mapping the suggest module applied — kept as a
/// shared helper so optimize and suggest can't drift apart.
fn lut_op_family_from(family: OperationFamily) -> LutOperationFamily {
    match family {
        OperationFamily::Adaptive => LutOperationFamily::Adaptive,
        OperationFamily::Pocket => LutOperationFamily::Pocket,
        OperationFamily::Contour => LutOperationFamily::Contour,
        OperationFamily::Parallel => LutOperationFamily::Parallel,
        OperationFamily::Scallop => LutOperationFamily::Scallop,
        OperationFamily::Trace => LutOperationFamily::Trace,
        OperationFamily::Face => LutOperationFamily::Face,
    }
}

/// Map the operation's `feeds_pass_role` to the LUT's pass-role enum.
fn lut_pass_role_from(role: PassRole) -> LutPassRole {
    match role {
        PassRole::Roughing => LutPassRole::Roughing,
        PassRole::SemiFinish => LutPassRole::SemiFinish,
        PassRole::Finish => LutPassRole::Finish,
    }
}

/// Look up this toolpath's per-toolpath cycle time from the trace's
/// per-toolpath summary. Returns `None` if the toolpath isn't
/// represented in the trace (no samples for that id — happens when
/// the toolpath was disabled or skipped).
fn cycle_time_from_trace(trace: &SimulationCutTrace, toolpath_id: usize) -> Option<f64> {
    trace
        .toolpath_summaries
        .iter()
        .find(|s| s.toolpath_id == toolpath_id)
        .map(|s| s.total_runtime_s)
}

/// Resources the candidate evaluation needs that are constant across
/// all candidates for a given toolpath. Built once at the top of
/// `optimize_toolpath`, passed to each `evaluate_candidate` call.
///
/// All fields are owned — the context outlives any mutable borrow of
/// the session held by the restore guard.
pub(crate) struct EvaluationContext {
    /// The toolpath's index in `session.toolpath_configs`.
    pub toolpath_index: usize,
    /// The toolpath's stable id (matches `SimulationCutSample::toolpath_id`).
    pub toolpath_id: usize,
    /// Operation kind — used for the gate's LUT routing and for
    /// skipping ops the optimizer can't model.
    pub operation_kind: OperationType,
    /// The op's `feeds_family` (pre-LUT-routing). Used by the pre-flight
    /// prescription helpers to phrase op-aware refusals. The
    /// `lut_op_family` field below is what the LUT lookup uses; this
    /// one is for human-readable surface only.
    pub op_family: OperationFamily,
    /// LUT family from the op's `feeds_family`.
    pub lut_op_family: LutOperationFamily,
    /// LUT pass role from the op's `feeds_pass_role`.
    pub lut_pass_role: LutPassRole,
    /// Built tool definition (`build_cutter` over the tool config).
    pub tool: crate::tool::ToolDefinition,
    /// Owned clone of the session's stock material.
    pub material: crate::material::Material,
}

impl EvaluationContext {
    /// Build the evaluation context from the session for the given
    /// toolpath index. Returns `None` if the toolpath or its tool is
    /// missing — caller should `Skipped` in that case.
    pub(crate) fn from_session(session: &ProjectSession, toolpath_index: usize) -> Option<Self> {
        let tc = session.get_toolpath_config(toolpath_index)?;
        let tool_cfg = session.get_tool(crate::compute::tool_config::ToolId(tc.tool_id))?;
        let tool = crate::compute::cutter::build_cutter(tool_cfg);
        let spec = tc.operation.spec();
        Some(Self {
            toolpath_index,
            toolpath_id: tc.id,
            operation_kind: tc.operation.op_type(),
            op_family: spec.feeds_family,
            lut_op_family: lut_op_family_from(spec.feeds_family),
            lut_pass_role: lut_pass_role_from(spec.feeds_pass_role),
            tool,
            material: session.stock_config().material.clone(),
        })
    }
}

/// Evaluate one candidate end-to-end:
///   1. Apply candidate params via the restore guard (transactional
///      mutation that also clears the relevant `feeds_auto.*` flags).
///   2. Regenerate the toolpath at the new params.
///   3. Run a fresh project sim at `sim_resolution_mm` dexel.
///   4. Score via the gate (`tool_load::evaluate_toolpath`).
///   5. Read per-toolpath cycle time from the trace.
///   6. Build the `OptimizeCandidate`.
///
/// Errors propagate as `SessionError` — the orchestrator decides
/// whether to surface a `Skipped` outcome or skip just this candidate.
/// Cancellation (`cancel.load(Ordering::SeqCst) == true`) is honoured
/// by the underlying generate/sim functions and surfaces here as
/// `SessionError::Simulation(SimulationError::Cancelled)`.
pub(crate) fn evaluate_candidate(
    guard: &mut BaselineRestoreGuard<'_>,
    ctx: &EvaluationContext,
    candidate_op: OperationConfig,
    delta: ParamDelta,
    stage: SearchStage,
    sim_resolution_mm: f64,
    cancel: &AtomicBool,
) -> Result<OptimizeCandidate, SessionError> {
    // Clone the unchanged-by-optimize fields from the captured
    // baseline so the apply mutation is one transactional write.
    let baseline = guard.baseline();
    let dressups = baseline.dressups.clone();
    let face_selection = baseline.face_selection.clone();
    let feeds_auto = feeds_auto_for_candidate(&baseline.feeds_auto, &delta);
    let toolpath_index = ctx.toolpath_index;

    // Apply.
    guard.session_mut().apply_toolpath_param_snapshot(
        toolpath_index,
        candidate_op.clone(),
        dressups,
        face_selection,
        feeds_auto,
    )?;

    // Regen — writes session.results[idx], leaves other entries intact.
    guard
        .session_mut()
        .generate_toolpath(toolpath_index, cancel)?;

    // Sim — full project sim at the requested resolution. Other
    // toolpaths' cached results from baseline still apply because
    // generate_toolpath only touched index `toolpath_index`.
    let sim_opts = SimulationOptions {
        resolution: sim_resolution_mm,
        skip_ids: Vec::new(),
        metrics_enabled: true,
        auto_resolution: false,
    };
    guard.session_mut().run_simulation(&sim_opts, cancel)?;

    // Pull the trace out (Arc<SimulationCutTrace>) and score via the
    // gate. The sim result lives on `session.simulation` after
    // `run_simulation` succeeds.
    let session_ref = &*guard.session_mut();
    let sim_result = session_ref
        .simulation_result()
        .ok_or(SessionError::Simulation(
            crate::compute::simulate::SimulationError::Cancelled,
        ))?;
    let trace = sim_result.cut_trace.as_deref();

    let load_ctx = ToolpathLoadContext {
        toolpath_id: ctx.toolpath_id,
        tool: &ctx.tool,
        material: &ctx.material,
        operation_family: ctx.lut_op_family,
        pass_role: ctx.lut_pass_role,
        operation_feed_rate_mm_min: candidate_op.feed_rate(),
        operation_kind: ctx.operation_kind,
    };
    let verdict = evaluate_toolpath(&load_ctx, trace, Some(session_ref.machine()));
    let cycle_time_s = trace
        .and_then(|t| cycle_time_from_trace(t, ctx.toolpath_id))
        .unwrap_or(f64::INFINITY);

    Ok(OptimizeCandidate {
        params: candidate_op,
        delta,
        cycle_time_s,
        verdict,
        stage,
        reconciled_cycle_time_s: None,
        reconciled_verdict: None,
    })
}

// ── Stage 2 + outcome dispatch ────────────────────────────────────────
//
// Stage 2 takes the top-3 by Stage-1 cycle time (per ED 2 — ranking
// ignores verdict) and re-evaluates them at 0.5mm dexel. The reported
// numbers on the rollup are always Stage-2 numbers. Outcome dispatch
// then folds Stage 2 + the baseline candidate into one of the three
// `OptimizeOutcome` variants per ED 8.

/// How many Stage-1 candidates survive into Stage 2 by default. Per
/// ED 2: top 3 by cycle time, ignoring verdict.
pub(crate) const STAGE2_SURVIVOR_COUNT: usize = 3;

/// Default Stage-1 dexel resolution per ED 1: 1.0mm. Calibrated
/// 2026-05-03 on wanaka to keep verdict-kinds stable vs. 0.5mm.
pub(crate) const STAGE1_RESOLUTION_MM: f64 = 1.0;

/// Default Stage-2 / Refined dexel resolution. Matches the GUI's
/// default sim resolution; the rollup quotes Stage-2 numbers verbatim.
pub(crate) const STAGE2_RESOLUTION_MM: f64 = 0.5;

/// Sort `stage1_winners` by ascending cycle time and keep the top
/// `n` (lowest-cycle-time) entries. Per ED 2 we pick by cycle time
/// alone — Stage 2 re-applies the gate verdict at full resolution.
pub(crate) fn select_stage2_candidates(
    mut stage1_winners: Vec<OptimizeCandidate>,
    n: usize,
) -> Vec<OptimizeCandidate> {
    stage1_winners.sort_by(|a, b| a.cycle_time_s.total_cmp(&b.cycle_time_s));
    stage1_winners.truncate(n);
    stage1_winners
}

/// Re-evaluate each Stage-1 winner at Stage-2 resolution. Returns the
/// list of refined candidates in the same order as the input. Each
/// re-evaluation goes through `evaluate_candidate` so the Stage-2
/// numbers come from the same simulator and gate as Stage 1 — there
/// is no second model anywhere.
pub(crate) fn refine_stage2(
    guard: &mut BaselineRestoreGuard<'_>,
    ctx: &EvaluationContext,
    stage1_winners: Vec<OptimizeCandidate>,
    cancel: &AtomicBool,
) -> Result<Vec<OptimizeCandidate>, SessionError> {
    use std::sync::atomic::Ordering;
    let mut refined = Vec::with_capacity(stage1_winners.len());
    for c in stage1_winners {
        if cancel.load(Ordering::SeqCst) {
            break;
        }
        let candidate = evaluate_candidate(
            guard,
            ctx,
            c.params,
            c.delta,
            SearchStage::Refined,
            STAGE2_RESOLUTION_MM,
            cancel,
        )?;
        refined.push(candidate);
    }
    Ok(refined)
}

/// Build an `OptimizeOutcome` from a baseline candidate and the
/// Stage-2 refined candidates. Per ED 8:
///   - Empty `candidates` (no candidates produced at all) →
///     `NoSafeImprovement` with the "no improvement found" narrative.
///   - All candidates `Exceeds` or all slower than baseline →
///     `NoSafeImprovement`.
///   - At least one safe-and-faster candidate → `Ranked`, with
///     baseline at index 0 and the rest sorted ascending by cycle
///     time.
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

    let baseline_cycle = baseline.cycle_time_s;
    let any_safe_and_faster = candidates.iter().any(|c| {
        candidate_is_safe(c) && c.cycle_time_s + RECOMMENDATION_CYCLE_DELTA_S < baseline_cycle
    });

    if !any_safe_and_faster {
        let all_unsafe = candidates.iter().all(|c| !candidate_is_safe(c));
        let explanation = if all_unsafe {
            format!(
                "{}: every candidate hit a gate limit (chipload, power, or deflection)",
                RefuseReason::NoImprovementFound.explanation_for_optimize()
            )
        } else {
            format!(
                "{}: no candidate beat the baseline cycle time by more than {:.1}s",
                RefuseReason::NoImprovementFound.explanation_for_optimize(),
                RECOMMENDATION_CYCLE_DELTA_S
            )
        };
        // Build the attempted list the same way Ranked does — baseline
        // at index 0, then candidates sorted by ascending cycle time.
        // The user can see what was tried and how each fell short.
        let mut sorted = candidates;
        sorted.sort_by(|a, b| a.cycle_time_s.total_cmp(&b.cycle_time_s));
        let mut attempted = Vec::with_capacity(sorted.len() + 1);
        attempted.push(baseline);
        attempted.extend(sorted);
        return OptimizeOutcome::NoSafeImprovement {
            reason: RefuseReason::NoImprovementFound,
            explanation,
            attempted,
        };
    }

    // Build the ranked list: baseline at index 0, then candidates
    // sorted by ascending cycle time. The recommendation is whichever
    // first_safe() returns.
    let mut sorted = candidates;
    sorted.sort_by(|a, b| a.cycle_time_s.total_cmp(&b.cycle_time_s));
    let mut ranked = Vec::with_capacity(sorted.len() + 1);
    ranked.push(baseline);
    ranked.extend(sorted);
    OptimizeOutcome::Ranked(ranked)
}

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

/// Threshold above which a toolpath is flagged as the project's
/// bottleneck. A toolpath whose baseline cycle time is at least this
/// fraction of the project's total runtime gets the "Bottleneck:"
/// callout in the U3 rollup. 30% is calibrated against wanaka — TP 6
/// (the 3D finish at 61% of project time) trips it; smaller setups
/// like the project_curves at ~5% don't. If multiple toolpaths trip
/// it, the largest wins (only one bottleneck callout per rollup).
pub const BOTTLENECK_FRACTION: f64 = 0.30;

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
/// toolpath whose baseline cycle exceeds [`BOTTLENECK_FRACTION`] of the
/// project total, breaking ties by the largest cycle (so only one row
/// gets the callout).
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
    //    cycle is ≥ BOTTLENECK_FRACTION of the project total. Walk the
    //    `enabled` list (not `per_toolpath`, which may be partial under
    //    cancel) so the bottleneck is stable regardless of how far the
    //    optimization run got.
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
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod stage0_tests {
    use super::*;
    use crate::compute::operation_configs::PocketConfig;
    use crate::machine::{
        ChipLoadFormula, MachineProfile, PowerModel, RigidityProfile, SpindleConfig,
    };

    fn synthetic_machine(
        max_rpm: f64,
        max_feed: f64,
        power: PowerModel,
        safety: f64,
    ) -> MachineProfile {
        MachineProfile {
            name: "stage0 test".to_owned(),
            spindle: SpindleConfig::Variable {
                min_rpm: 6000.0,
                max_rpm,
            },
            power,
            chip_load: ChipLoadFormula::default(),
            max_feed_mm_min: max_feed,
            max_shank_mm: 7.0,
            rigidity: RigidityProfile::default(),
            safety_factor: safety,
        }
    }

    #[test]
    fn k_is_one_when_all_caps_already_binding() {
        // rpm_baseline at machine max; feed_baseline at machine max;
        // power_baseline at machine cap × safety. No headroom.
        let machine = synthetic_machine(
            18_000.0,
            5_000.0,
            PowerModel::ConstantPower { power_kw: 1.5 },
            0.8,
        );
        let inputs = Stage0Inputs {
            rpm_baseline: 18_000.0,
            feed_baseline_mm_min: 5_000.0,
            peak_power_baseline_kw: Some(1.2), // 1.5 × 0.8 = 1.2
            machine: &machine,
            lut_row: None,
        };
        let k = solve_headroom_scale(&inputs);
        assert!((k - 1.0).abs() < 1e-6, "expected k = 1.0, got {k}");
    }

    #[test]
    fn k_machine_rpm_binds_when_feed_and_power_have_room() {
        let machine = synthetic_machine(
            24_000.0,
            10_000.0,                                    // generous feed cap
            PowerModel::ConstantPower { power_kw: 5.0 }, // generous power
            0.8,
        );
        let inputs = Stage0Inputs {
            rpm_baseline: 12_000.0,
            feed_baseline_mm_min: 1_500.0,
            peak_power_baseline_kw: Some(0.5),
            machine: &machine,
            lut_row: None,
        };
        // k_rpm = 24000/12000 = 2.0
        // k_feed = 10000/1500 ≈ 6.67
        // k_power = 5.0 × 0.8 / 0.5 = 8.0
        // k = 2.0 (rpm binds)
        let k = solve_headroom_scale(&inputs);
        assert!((k - 2.0).abs() < 1e-6, "expected k = 2.0, got {k}");
    }

    #[test]
    fn k_feed_binds_when_rpm_and_power_have_room() {
        let machine = synthetic_machine(
            30_000.0, // generous rpm
            1_800.0,
            PowerModel::ConstantPower { power_kw: 5.0 },
            0.8,
        );
        let inputs = Stage0Inputs {
            rpm_baseline: 12_000.0,
            feed_baseline_mm_min: 1_500.0,
            peak_power_baseline_kw: Some(0.5),
            machine: &machine,
            lut_row: None,
        };
        // k_rpm = 30000/12000 = 2.5
        // k_feed = 1800/1500 = 1.2
        // k_power = 5.0 × 0.8 / 0.5 = 8.0
        // k = 1.2
        let k = solve_headroom_scale(&inputs);
        assert!((k - 1.2).abs() < 1e-6, "expected k = 1.2, got {k}");
    }

    #[test]
    fn k_power_binds_on_constant_power_machine() {
        let machine = synthetic_machine(
            30_000.0,
            10_000.0,
            PowerModel::ConstantPower { power_kw: 0.6 }, // tight
            0.8,
        );
        let inputs = Stage0Inputs {
            rpm_baseline: 12_000.0,
            feed_baseline_mm_min: 1_500.0,
            peak_power_baseline_kw: Some(0.4),
            machine: &machine,
            lut_row: None,
        };
        // k_power = 0.6 × 0.8 / 0.4 = 1.2
        let k = solve_headroom_scale(&inputs);
        assert!((k - 1.2).abs() < 1e-6, "expected k = 1.2, got {k}");
    }

    #[test]
    fn k_power_uses_rated_for_vfd() {
        // VFD: rated_power is the cap that binds k_power, not the
        // power-at-baseline-rpm value.
        let machine = synthetic_machine(
            24_000.0,
            10_000.0,
            PowerModel::VfdConstantTorque {
                rated_power_kw: 1.0,
                rated_rpm: 18_000.0,
            },
            0.8,
        );
        let inputs = Stage0Inputs {
            rpm_baseline: 12_000.0,
            feed_baseline_mm_min: 1_500.0,
            peak_power_baseline_kw: Some(0.4), // baseline well under rated.
            machine: &machine,
            lut_row: None,
        };
        // k_power = 1.0 × 0.8 / 0.4 = 2.0
        // k_rpm = 24000/12000 = 2.0 (also 2.0)
        // k_feed plenty
        let k = solve_headroom_scale(&inputs);
        assert!((k - 2.0).abs() < 1e-6, "expected k = 2.0, got {k}");
    }

    #[test]
    fn lut_row_rpm_can_bind() {
        let machine = synthetic_machine(
            24_000.0,
            10_000.0,
            PowerModel::ConstantPower { power_kw: 5.0 },
            0.8,
        );
        // LUT row carries an explicit rpm_max.
        let lut_row = MatchedRow {
            chip_load_mm: 0.04,
            chip_load_min_mm: Some(0.025),
            chip_load_max_mm: Some(0.055),
            rpm_nominal: Some(15_000.0),
            rpm_min: Some(14_000.0),
            rpm_max: Some(16_000.0),
            ap_min_mm: None,
            ap_max_mm: None,
            ae_min_mm: None,
            ae_max_mm: None,
            observation_id: "test-row".to_owned(),
            source_vendor: "Test".to_owned(),
            score: 100,
            diameter_match_score: 200,
        };
        let inputs = Stage0Inputs {
            rpm_baseline: 12_000.0,
            feed_baseline_mm_min: 1_500.0,
            peak_power_baseline_kw: Some(0.4),
            machine: &machine,
            lut_row: Some(&lut_row),
        };
        // k_lut = 16000/12000 ≈ 1.333
        // Other caps higher. k = 1.333.
        let k = solve_headroom_scale(&inputs);
        assert!(
            (k - 16_000.0 / 12_000.0).abs() < 1e-6,
            "expected k ≈ 1.333, got {k}"
        );
    }

    #[test]
    fn lut_falls_back_to_rpm_nominal_times_1_2() {
        let machine = synthetic_machine(
            24_000.0,
            10_000.0,
            PowerModel::ConstantPower { power_kw: 5.0 },
            0.8,
        );
        // No rpm_max, no rpm_min, only rpm_nominal — Engineering
        // Default 5: ±20% bracket from nominal.
        let lut_row = MatchedRow {
            chip_load_mm: 0.04,
            chip_load_min_mm: Some(0.025),
            chip_load_max_mm: Some(0.055),
            rpm_nominal: Some(15_000.0),
            rpm_min: None,
            rpm_max: None,
            ap_min_mm: None,
            ap_max_mm: None,
            ae_min_mm: None,
            ae_max_mm: None,
            observation_id: "test-row".to_owned(),
            source_vendor: "Test".to_owned(),
            score: 100,
            diameter_match_score: 200,
        };
        let inputs = Stage0Inputs {
            rpm_baseline: 12_000.0,
            feed_baseline_mm_min: 1_500.0,
            peak_power_baseline_kw: Some(0.4),
            machine: &machine,
            lut_row: Some(&lut_row),
        };
        // k_lut = (15000 × 1.2) / 12000 = 1.5
        let k = solve_headroom_scale(&inputs);
        assert!((k - 1.5).abs() < 1e-6, "expected k = 1.5, got {k}");
    }

    #[test]
    fn power_unmodeled_drops_constraint() {
        let machine = synthetic_machine(
            24_000.0,
            10_000.0,
            PowerModel::ConstantPower { power_kw: 5.0 },
            0.8,
        );
        let inputs = Stage0Inputs {
            rpm_baseline: 12_000.0,
            feed_baseline_mm_min: 1_500.0,
            peak_power_baseline_kw: None, // gate said Unmodeled
            machine: &machine,
            lut_row: None,
        };
        // Without power data, k constrained only by rpm and feed.
        // k_rpm = 2.0; k_feed ≈ 6.67. k = 2.0.
        let k = solve_headroom_scale(&inputs);
        assert!((k - 2.0).abs() < 1e-6, "expected k = 2.0, got {k}");
    }

    #[test]
    fn k_floors_at_one_never_proposes_down_scaling() {
        // Force every limit to be below baseline; Stage 0 should still
        // return k = 1.0 (downscaling is Stage 1's job for Exceeds).
        let machine = synthetic_machine(
            8_000.0, // baseline 12000 already over machine max
            500.0,   // baseline feed 1500 already over machine cap
            PowerModel::ConstantPower { power_kw: 0.1 },
            0.8,
        );
        let inputs = Stage0Inputs {
            rpm_baseline: 12_000.0,
            feed_baseline_mm_min: 1_500.0,
            peak_power_baseline_kw: Some(1.0),
            machine: &machine,
            lut_row: None,
        };
        let k = solve_headroom_scale(&inputs);
        assert!((k - 1.0).abs() < 1e-9, "expected k floored to 1.0, got {k}");
    }

    #[test]
    fn apply_scale_writes_feed_rpm_and_clamps() {
        use crate::compute::catalog::OperationConfig;
        use crate::compute::operation_configs::PocketConfig;
        let machine = synthetic_machine(
            24_000.0,
            10_000.0,
            PowerModel::ConstantPower { power_kw: 5.0 },
            0.8,
        );
        let baseline = OperationConfig::Pocket(PocketConfig {
            feed_rate: 1500.0,
            spindle_rpm: Some(12_000),
            ..PocketConfig::default()
        });
        let scaled = apply_scale_to_op(&baseline, 12_000.0, 1.5, &machine);
        assert!(
            (scaled.feed_rate() - 2_250.0).abs() < 1e-6,
            "feed should scale 1500 × 1.5 = 2250, got {}",
            scaled.feed_rate()
        );
        assert_eq!(scaled.spindle_rpm(), Some(18_000));
    }

    #[test]
    fn apply_scale_clamps_rpm_to_machine_max() {
        use crate::compute::catalog::OperationConfig;
        use crate::compute::operation_configs::PocketConfig;
        // Machine max 18000; baseline 12000; k = 2.0 → 24000 → clamped to 18000.
        let machine = synthetic_machine(
            18_000.0,
            10_000.0,
            PowerModel::ConstantPower { power_kw: 5.0 },
            0.8,
        );
        let baseline = OperationConfig::Pocket(PocketConfig {
            feed_rate: 1500.0,
            spindle_rpm: Some(12_000),
            ..PocketConfig::default()
        });
        let scaled = apply_scale_to_op(&baseline, 12_000.0, 2.0, &machine);
        assert_eq!(scaled.spindle_rpm(), Some(18_000));
    }

    // ── Stage F re-target (Commit #2) ────────────────────────────────

    fn empty_row() -> MatchedRow {
        MatchedRow {
            chip_load_mm: 0.05,
            chip_load_min_mm: None,
            chip_load_max_mm: None,
            rpm_nominal: None,
            rpm_min: None,
            rpm_max: None,
            ap_min_mm: None,
            ap_max_mm: None,
            ae_min_mm: None,
            ae_max_mm: None,
            observation_id: "test".to_owned(),
            source_vendor: "synthetic".to_owned(),
            score: 0,
            diameter_match_score: 0,
        }
    }

    #[test]
    fn lut_chipload_target_uses_midpoint_when_both_bounds_set() {
        let mut row = empty_row();
        row.chip_load_min_mm = Some(0.04);
        row.chip_load_max_mm = Some(0.10);
        let target = lut_chipload_target(&row).unwrap();
        assert!((target - 0.07).abs() < 1e-9, "got {target}");
    }

    #[test]
    fn lut_chipload_target_uses_offset_when_only_one_bound_set() {
        // Only max published → target slightly below max (15% margin)
        // so we don't sit on the breakage ceiling.
        let mut row = empty_row();
        row.chip_load_max_mm = Some(0.10);
        let target = lut_chipload_target(&row).unwrap();
        assert!((target - 0.085).abs() < 1e-6, "got {target}");

        // Only min published → target slightly above min (15% margin)
        // so we don't sit on the burn floor.
        let mut row = empty_row();
        row.chip_load_min_mm = Some(0.04);
        let target = lut_chipload_target(&row).unwrap();
        assert!((target - 0.046).abs() < 1e-6, "got {target}");
    }

    #[test]
    fn lut_chipload_target_returns_none_when_no_bounds() {
        let row = empty_row();
        assert!(lut_chipload_target(&row).is_none());
    }

    #[test]
    fn solve_target_rpm_prefers_lut_nominal_inside_machine_range() {
        let machine = synthetic_machine(
            24_000.0,
            10_000.0,
            PowerModel::ConstantPower { power_kw: 5.0 },
            0.8,
        );
        let mut row = empty_row();
        row.rpm_nominal = Some(15_000.0);
        row.rpm_min = Some(12_000.0);
        row.rpm_max = Some(20_000.0);
        let rpm = solve_target_rpm(8_000.0, &row, &machine).unwrap();
        assert!((rpm - 15_000.0).abs() < 1e-6, "got {rpm}");
    }

    #[test]
    fn solve_target_rpm_returns_none_when_brackets_disjoint() {
        // Machine maxes at 18k; LUT row needs ≥ 25k. No overlap.
        let machine = synthetic_machine(
            18_000.0,
            10_000.0,
            PowerModel::ConstantPower { power_kw: 5.0 },
            0.8,
        );
        let mut row = empty_row();
        row.rpm_min = Some(25_000.0);
        row.rpm_max = Some(30_000.0);
        assert!(solve_target_rpm(12_000.0, &row, &machine).is_none());
    }

    #[test]
    fn retarget_raises_feed_for_burn_baseline() {
        // Baseline feed 600 mm/min, RPM 12k, 2 flutes → chipload =
        // 600 / (12000 × 2) = 0.025 mm/tooth, BELOW the row's
        // [0.04, 0.10] range. Retarget should pick midpoint 0.07
        // and raise feed accordingly.
        let machine = synthetic_machine(
            24_000.0,
            10_000.0,
            PowerModel::ConstantPower { power_kw: 5.0 },
            0.8,
        );
        let mut row = empty_row();
        row.chip_load_min_mm = Some(0.04);
        row.chip_load_max_mm = Some(0.10);
        row.rpm_nominal = Some(12_000.0);
        let baseline = OperationConfig::Pocket(PocketConfig {
            feed_rate: 600.0,
            spindle_rpm: Some(12_000),
            ..PocketConfig::default()
        });
        // Full slot (ae=0 → RCTF=1.0 default). 6.35 mm endmill, 2 flutes.
        let solution = solve_chipload_retarget(
            &baseline, 12_000.0, 2,    // flutes
            6.35, // engaged diameter
            0.0,  // commanded ae (full slot fallback)
            1500.0, &row, &machine,
        )
        .unwrap();
        // Target nominal chipload = 0.07 × RCTF(0.0, 6.35) = 0.07.
        // Feed = 0.07 × 12000 × 2 = 1680. Capped by machine max 10000.
        assert!(solution.target_feed_mm_min > baseline.feed_rate());
        assert!(
            (solution.target_feed_mm_min - 1680.0).abs() < 1.0,
            "got {solution:?}"
        );
        // RPM stays at LUT nominal.
        assert!((solution.target_rpm - 12_000.0).abs() < 1.0);
    }

    #[test]
    fn retarget_lowers_feed_for_breakage_baseline() {
        // Baseline feed 4000 mm/min → chipload 0.167, well above max.
        // Retarget should pull feed down toward midpoint 0.07.
        let machine = synthetic_machine(
            24_000.0,
            10_000.0,
            PowerModel::ConstantPower { power_kw: 5.0 },
            0.8,
        );
        let mut row = empty_row();
        row.chip_load_min_mm = Some(0.04);
        row.chip_load_max_mm = Some(0.10);
        row.rpm_nominal = Some(12_000.0);
        let baseline = OperationConfig::Pocket(PocketConfig {
            feed_rate: 4000.0,
            spindle_rpm: Some(12_000),
            ..PocketConfig::default()
        });
        let solution =
            solve_chipload_retarget(&baseline, 12_000.0, 2, 6.35, 0.0, 1500.0, &row, &machine)
                .unwrap();
        assert!(solution.target_feed_mm_min < baseline.feed_rate());
        // Feed = 0.07 × 12000 × 2 = 1680.
        assert!((solution.target_feed_mm_min - 1680.0).abs() < 1.0);
    }

    #[test]
    fn retarget_compensates_for_partial_engagement_via_rctf() {
        // 20% engagement on a 6 mm tool: ae/d = 0.2, RCTF ~ 1.25.
        // Target nominal chipload = 0.05 × 1.25 = 0.0625, so feed
        // should land 25% higher than the no-RCTF case.
        let machine = synthetic_machine(
            24_000.0,
            10_000.0,
            PowerModel::ConstantPower { power_kw: 5.0 },
            0.8,
        );
        let mut row = empty_row();
        row.chip_load_min_mm = Some(0.04);
        row.chip_load_max_mm = Some(0.06);
        row.rpm_nominal = Some(12_000.0);
        let baseline = OperationConfig::Pocket(PocketConfig {
            feed_rate: 600.0,
            spindle_rpm: Some(12_000),
            ..PocketConfig::default()
        });
        let no_rctf = solve_chipload_retarget(
            &baseline, 12_000.0, 2, 6.0, 0.0, // ae=0 → RCTF=1
            1500.0, &row, &machine,
        )
        .unwrap();
        let with_rctf = solve_chipload_retarget(
            &baseline, 12_000.0, 2, 6.0, 1.2, // 20% engagement → RCTF ≈ 1.25
            1500.0, &row, &machine,
        )
        .unwrap();
        let ratio = with_rctf.target_feed_mm_min / no_rctf.target_feed_mm_min;
        assert!(
            ratio > 1.20 && ratio < 1.30,
            "RCTF should bump feed ~25% — got ratio {ratio}"
        );
    }

    #[test]
    fn retarget_plunge_tracks_feed_when_delta_exceeds_ten_percent() {
        // Big retarget (>10% feed change) → plunge updates.
        let machine = synthetic_machine(
            24_000.0,
            10_000.0,
            PowerModel::ConstantPower { power_kw: 5.0 },
            0.8,
        );
        let mut row = empty_row();
        row.chip_load_min_mm = Some(0.04);
        row.chip_load_max_mm = Some(0.10);
        row.rpm_nominal = Some(12_000.0);
        let baseline = OperationConfig::Pocket(PocketConfig {
            feed_rate: 600.0,
            spindle_rpm: Some(12_000),
            plunge_rate: 200.0,
            ..PocketConfig::default()
        });
        let solution =
            solve_chipload_retarget(&baseline, 12_000.0, 2, 6.35, 0.0, 1500.0, &row, &machine)
                .unwrap();
        assert!(
            solution.target_plunge_mm_min.is_some(),
            "feed change ~2.8× should trip the plunge tracker"
        );
    }

    #[test]
    fn retarget_skips_noise_floor_changes() {
        // Baseline feed already inside the LUT envelope at midpoint.
        // Resulting "retarget" delta is well below 1% — solver should
        // refuse rather than emit a no-op candidate.
        let machine = synthetic_machine(
            24_000.0,
            10_000.0,
            PowerModel::ConstantPower { power_kw: 5.0 },
            0.8,
        );
        let mut row = empty_row();
        row.chip_load_min_mm = Some(0.04);
        row.chip_load_max_mm = Some(0.10);
        row.rpm_nominal = Some(12_000.0);
        // Baseline feed = midpoint × rpm × flutes = 0.07 × 12000 × 2 = 1680.
        let baseline = OperationConfig::Pocket(PocketConfig {
            feed_rate: 1680.0,
            spindle_rpm: Some(12_000),
            ..PocketConfig::default()
        });
        let solution =
            solve_chipload_retarget(&baseline, 12_000.0, 2, 6.35, 0.0, 1500.0, &row, &machine);
        assert!(solution.is_none(), "noise-level retarget should refuse");
    }

    #[test]
    fn retarget_returns_none_when_no_lut_chipload_bounds() {
        let machine = synthetic_machine(
            24_000.0,
            10_000.0,
            PowerModel::ConstantPower { power_kw: 5.0 },
            0.8,
        );
        let row = empty_row(); // no chipload bounds
        let baseline = OperationConfig::Pocket(PocketConfig {
            feed_rate: 600.0,
            spindle_rpm: Some(12_000),
            ..PocketConfig::default()
        });
        assert!(
            solve_chipload_retarget(&baseline, 12_000.0, 2, 6.35, 0.0, 1500.0, &row, &machine)
                .is_none()
        );
    }

    #[test]
    fn apply_retarget_writes_feed_rpm_and_optional_plunge() {
        let machine = synthetic_machine(
            24_000.0,
            10_000.0,
            PowerModel::ConstantPower { power_kw: 5.0 },
            0.8,
        );
        let baseline = OperationConfig::Pocket(PocketConfig {
            feed_rate: 600.0,
            spindle_rpm: Some(12_000),
            plunge_rate: 200.0,
            ..PocketConfig::default()
        });
        let solution = RetargetSolution {
            target_feed_mm_min: 1680.0,
            target_rpm: 12_500.0,
            target_plunge_mm_min: Some(560.0),
        };
        let out = apply_retarget_to_op(&baseline, &solution, &machine);
        assert!((out.feed_rate() - 1680.0).abs() < 1e-6);
        assert_eq!(out.spindle_rpm(), Some(12_500));
        assert!((out.plunge_rate() - 560.0).abs() < 1e-6);
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod restore_guard_tests {
    use super::*;
    use crate::compute::catalog::OperationConfig;
    use crate::compute::operation_configs::PocketConfig;
    use crate::compute::tool_config::{ToolConfig, ToolId, ToolType};
    use crate::session::ToolpathConfig;

    fn make_tool() -> ToolConfig {
        ToolConfig::new_default(ToolId(0), ToolType::EndMill)
    }

    fn make_tc(tool_id: usize) -> ToolpathConfig {
        ToolpathConfig {
            id: 0,
            name: "test".to_owned(),
            enabled: true,
            operation: OperationConfig::Pocket(PocketConfig {
                feed_rate: 1500.0,
                stepover: 2.0,
                depth_per_pass: 1.5,
                spindle_rpm: Some(18_000),
                ..PocketConfig::default()
            }),
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

    fn session_with_one_pocket() -> ProjectSession {
        let mut s = ProjectSession::new_empty();
        s.add_tool(make_tool());
        s.add_toolpath(0, make_tc(s.tools()[0].id.0)).unwrap();
        s
    }

    #[test]
    fn drop_with_no_changes_is_a_noop() {
        let mut session = session_with_one_pocket();
        let baseline_feed = session.toolpath_configs()[0].operation.feed_rate();
        {
            let _guard = BaselineRestoreGuard::new(&mut session, 0).unwrap();
            // No mutations.
        }
        // Session unchanged.
        assert!((session.toolpath_configs()[0].operation.feed_rate() - baseline_feed).abs() < 1e-9);
    }

    #[test]
    fn drop_restores_after_mutation() {
        let mut session = session_with_one_pocket();
        {
            let mut guard = BaselineRestoreGuard::new(&mut session, 0).unwrap();
            // Mutate via the guard.
            let mut new_op = guard.baseline().operation.clone();
            new_op.set_feed_rate(2999.0);
            let new_op_clone = new_op.clone();
            let dressups = guard.baseline().dressups.clone();
            let face_sel = guard.baseline().face_selection.clone();
            // Use a feeds_auto with a flipped flag to verify restoration
            // covers the feeds_auto override too.
            let mut tweaked = guard.baseline().feeds_auto.clone();
            tweaked.feed_rate = false;
            guard
                .session_mut()
                .apply_toolpath_param_snapshot(0, new_op_clone, dressups, face_sel, tweaked)
                .unwrap();
            // While the guard lives, the session reflects the candidate.
            assert!(
                (guard.session_mut().toolpath_configs()[0]
                    .operation
                    .feed_rate()
                    - 2999.0)
                    .abs()
                    < 1e-6
            );
            assert!(
                !guard.session_mut().toolpath_configs()[0]
                    .feeds_auto
                    .feed_rate
            );
            // Mutation also bridges into the snapshot — but only for
            // copies; the captured snapshot is immutable.
            assert!(
                (guard.baseline().operation.feed_rate() - 1500.0).abs() < 1e-6,
                "snapshot should remain at baseline 1500 mm/min"
            );
            // Verify mutating new_op outside the guard didn't touch the
            // snapshot — defends against accidental shared mutation.
            new_op.set_feed_rate(0.0);
            let _ = new_op;
            assert!(
                (guard.baseline().operation.feed_rate() - 1500.0).abs() < 1e-6,
                "snapshot must be detached from the candidate's params"
            );
        }
        // After drop, session restored to baseline.
        let tc = &session.toolpath_configs()[0];
        assert!((tc.operation.feed_rate() - 1500.0).abs() < 1e-6);
        assert!(tc.feeds_auto.feed_rate, "feeds_auto.feed_rate restored");
    }

    #[test]
    fn drop_restores_on_panic() {
        let mut session = session_with_one_pocket();

        // Trigger a panic inside a guard's scope and confirm Drop still
        // restored the baseline. AssertUnwindSafe is needed because
        // &mut ProjectSession is not UnwindSafe by default — that's
        // fine, the contract here is "panic-safe", not "safe to
        // continue using session after a panic in the body". We only
        // inspect immutable state on the way out.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut guard = BaselineRestoreGuard::new(&mut session, 0).unwrap();
            // Mutate to a non-baseline state.
            let dressups = guard.baseline().dressups.clone();
            let face_sel = guard.baseline().face_selection.clone();
            let feeds_auto = guard.baseline().feeds_auto.clone();
            let mut new_op = guard.baseline().operation.clone();
            new_op.set_feed_rate(9999.0);
            guard
                .session_mut()
                .apply_toolpath_param_snapshot(0, new_op, dressups, face_sel, feeds_auto)
                .unwrap();
            // Confirm we're in the mutated state.
            assert!(
                (guard.session_mut().toolpath_configs()[0]
                    .operation
                    .feed_rate()
                    - 9999.0)
                    .abs()
                    < 1e-6
            );
            // Now panic. Drop fires unwinding through this frame.
            panic!("simulated panic during candidate eval");
        }));

        assert!(result.is_err(), "the panic must propagate as expected");
        // After the panic + drop, session is back to baseline.
        let tc = &session.toolpath_configs()[0];
        assert!(
            (tc.operation.feed_rate() - 1500.0).abs() < 1e-6,
            "baseline feed must be restored after a panic; got {}",
            tc.operation.feed_rate()
        );
    }

    #[test]
    fn new_returns_not_found_for_invalid_index() {
        let mut session = session_with_one_pocket();
        let result = BaselineRestoreGuard::new(&mut session, 99);
        assert!(matches!(result, Err(SessionError::ToolpathNotFound(99))));
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

    use super::*;
    use crate::compute::catalog::OperationConfig;
    use crate::compute::operation_configs::{AlignmentPinDrillConfig, DrillConfig, PocketConfig};
    use crate::compute::tool_config::{ToolConfig, ToolId, ToolType};
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
    fn trace_with_summary() -> SimulationCutTrace {
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
        trace
    }

    #[test]
    fn deflection_exceeds_yields_setup_locked_refusal() {
        // Default endmill has stickout 45mm, diameter 6.35mm →
        // L/D ≈ 7.09 — above the 6.0 threshold. Pocket op with a
        // populated trace summary walks past every prior skip path
        // and lands in pre-flight, where the deflection Exceeds
        // verdict triggers `DeflectionSetupLocked`.
        let mut session = session_with_op(OperationConfig::Pocket(PocketConfig::default()));
        let trace = trace_with_summary();
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
                    explanation.contains("L/D"),
                    "explanation should mention L/D, got: {explanation}"
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
        // 6.35mm tool → target stickout for L/D=4 is 25mm. Verify
        // the prescription number lands in the explanation string.
        let tool = crate::tool::ToolDefinition::new(
            Box::new(crate::tool::FlatEndmill::new(6.35, 20.0)),
            6.35,
            20.0,
            40.0,
            45.0, // L/D = 7.087
            2,
            crate::compute::tool_config::ToolMaterial::Carbide,
        );
        let s = deflection_setup_prescription(&tool, 7.087);
        assert!(s.contains("7.1"), "ratio not in '{s}'");
        assert!(s.contains("25"), "target stickout 25mm not in '{s}'");
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
    use crate::compute::config::{BoundaryConfig, DressupConfig, HeightsConfig, StockSource};
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
        assert!((BOTTLENECK_FRACTION - 0.30).abs() < 1e-9);
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
        }
    }

    fn within_verdict() -> ToolpathLoadVerdict {
        use super::super::verdict::Confidence;
        ToolpathLoadVerdict {
            toolpath_id: 0,
            chipload: Verdict::Within {
                peak: 0.04,
                confidence: Confidence::Validated,
            },
            power: Verdict::Within {
                peak: 0.5,
                confidence: Confidence::Validated,
            },
            deflection: Verdict::Within {
                peak: 5.0,
                confidence: Confidence::Validated,
            },
        }
    }

    fn exceeds_chipload_verdict() -> ToolpathLoadVerdict {
        use super::super::verdict::{Confidence, ExceedsReason};
        ToolpathLoadVerdict {
            toolpath_id: 0,
            chipload: Verdict::Exceeds {
                peak: 0.08,
                sample_range: 0..1,
                reason: ExceedsReason::ChiploadBreakageRisk,
                confidence: Confidence::Validated,
            },
            power: Verdict::Within {
                peak: 0.5,
                confidence: Confidence::Validated,
            },
            deflection: Verdict::Within {
                peak: 5.0,
                confidence: Confidence::Validated,
            },
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
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod stage1_grid_tests {
    use super::*;
    use crate::compute::operation_configs::PocketConfig;

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
    fn lut_row_clamps_lo_to_ap_min() {
        // Baseline 3.0mm, LUT ap_min = 2.5mm (above 0.7×3.0 = 2.1).
        // Lo should be ap_min = 2.5, not 2.1.
        let row = synthetic_lut_row(Some(2.5), Some(5.0));
        let variants = build_doc_variants(3.0, Some(&row), OperationType::Adaptive3d);
        assert!((variants[0] - 2.5).abs() < 1e-6, "got {variants:?}");
    }

    #[test]
    fn lut_row_clamps_hi_to_ap_max() {
        // Baseline 3.0mm, LUT ap_max = 3.5mm (below 1.3×3.0 = 3.9).
        // Hi should be ap_max = 3.5, not 3.9.
        let row = synthetic_lut_row(Some(1.0), Some(3.5));
        let variants = build_doc_variants(3.0, Some(&row), OperationType::Adaptive3d);
        let last = *variants.last().unwrap();
        assert!((last - 3.5).abs() < 1e-6, "got {variants:?}");
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
    fn deduplicates_when_lut_bounds_collapse_envelope() {
        // LUT row tight to within microns of baseline → lo, base, hi
        // all within the dedupe tolerance (5 µm) → collapse.
        let row = synthetic_lut_row(Some(2.9999), Some(3.0001));
        let variants = build_doc_variants(3.0, Some(&row), OperationType::Adaptive3d);
        assert!(
            variants.len() <= 2,
            "expected dedupe to collapse near-identical values, got {variants:?}"
        );
    }

    #[test]
    fn floors_at_hard_minimum_for_tiny_baseline() {
        // Baseline 0.01mm — below the hard floor. Floor brings it up.
        let variants = build_doc_variants(0.01, None, OperationType::Pocket);
        // Every variant respects the floor (0.05mm).
        for v in &variants {
            assert!(*v >= DOC_HARD_FLOOR_MM - 1e-9, "got {variants:?}");
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

    #[test]
    fn apply_doc_writes_only_depth_per_pass() {
        let baseline = OperationConfig::Pocket(PocketConfig {
            feed_rate: 1500.0,
            stepover: 2.0,
            depth_per_pass: 1.5,
            spindle_rpm: Some(18_000),
            ..PocketConfig::default()
        });
        let candidate = apply_doc_to_op(&baseline, 2.5);
        // depth_per_pass changed.
        assert_eq!(candidate.depth_per_pass(), Some(2.5));
        // Other fields unchanged.
        assert!((candidate.feed_rate() - 1500.0).abs() < 1e-9);
        assert_eq!(candidate.stepover(), Some(2.0));
        assert_eq!(candidate.spindle_rpm(), Some(18_000));
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
    fn stepover_lut_row_clamps_to_ae_bounds() {
        // Baseline 1.0mm, LUT ae_min 0.8mm, ae_max 1.2mm. Multiplier
        // would give [0.7, 1.0, 1.3] — LUT clamps to [0.8, 1.0, 1.2].
        let row = synthetic_lut_row_with_ae(Some(0.8), Some(1.2));
        let variants = build_stepover_variants(1.0, Some(&row), OperationType::Adaptive3d);
        assert!((variants[0] - 0.8).abs() < 1e-6, "got {variants:?}");
        assert!((variants[2] - 1.2).abs() < 1e-6, "got {variants:?}");
    }

    #[test]
    fn stepover_floors_at_hard_minimum() {
        let variants = build_stepover_variants(0.01, None, OperationType::Pocket);
        for v in &variants {
            assert!(*v >= STEPOVER_HARD_FLOOR_MM - 1e-9, "got {variants:?}");
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

    #[test]
    fn apply_stepover_writes_only_stepover() {
        let baseline = OperationConfig::Pocket(PocketConfig {
            feed_rate: 1500.0,
            stepover: 2.0,
            depth_per_pass: 1.5,
            spindle_rpm: Some(18_000),
            ..PocketConfig::default()
        });
        let candidate = apply_stepover_to_op(&baseline, 2.5);
        assert_eq!(candidate.stepover(), Some(2.5));
        // Other fields unchanged.
        assert!((candidate.feed_rate() - 1500.0).abs() < 1e-9);
        assert_eq!(candidate.depth_per_pass(), Some(1.5));
        assert_eq!(candidate.spindle_rpm(), Some(18_000));
    }

    #[test]
    fn joint_apply_doc_then_stepover_preserves_both() {
        // The Stage 1 sweep applies DOC first, then stepover. Verify
        // the second apply doesn't clobber the first.
        let baseline = OperationConfig::Pocket(PocketConfig {
            feed_rate: 1500.0,
            stepover: 2.0,
            depth_per_pass: 1.5,
            ..PocketConfig::default()
        });
        let with_doc = apply_doc_to_op(&baseline, 2.5);
        let with_both = apply_stepover_to_op(&with_doc, 2.5);
        assert_eq!(with_both.depth_per_pass(), Some(2.5));
        assert_eq!(with_both.stepover(), Some(2.5));
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
    use super::*;
    use crate::compute::operation_configs::PocketConfig;

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
