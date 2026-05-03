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
use crate::feeds::vendor_lookup::MatchedRow;
use crate::feeds::vendor_lut::{LutOperationFamily, LutPassRole};
use crate::feeds::{OperationFamily, PassRole};
use crate::machine::{MachineProfile, PowerModel};
use crate::session::{ProjectSession, SessionError, SimulationOptions};
use crate::tool::MillingCutter;
use crate::simulation_cut::SimulationCutTrace;

use super::suggest::RefuseReason;
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
    if matches!(
        ctx.material,
        crate::material::Material::Custom { .. }
    ) {
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
    //    and Stage 1's DOC-grid endpoint clamping.
    let matched_lut_row = find_matched_lut_row(&ctx.tool, &ctx.material, &ctx);

    // Cancel check before any sims.
    if cancel.load(Ordering::SeqCst) {
        return OptimizeOutcome::NoSafeImprovement {
            reason: RefuseReason::NoImprovementFound,
            explanation: "cancelled before any candidates were generated".to_owned(),
        };
    }

    // 6. From here on the session is mutated per-candidate. The
    //    BaselineRestoreGuard restores `(operation, dressups,
    //    face_selection, feeds_auto)` on drop, regardless of how
    //    we exit (early return, Err, panic).
    let mut guard = match BaselineRestoreGuard::new(session, toolpath_index) {
        Ok(g) => g,
        Err(_) => {
            return OptimizeOutcome::Skipped {
                reason: RefuseReason::SimulationRequired,
            };
        }
    };

    let baseline_rpm = baseline_rpm_from_trace(
        baseline_trace,
        ctx.toolpath_id,
        baseline_op.spindle_rpm(),
        &machine,
    );

    // 7. Stage 0: closed-form analytical RPM/feed scaling. Skipped
    //    when the baseline already fails the chipload gate
    //    (proportional scaling preserves chipload — can't fix Exceeds).
    let baseline_chipload_exceeds =
        matches!(baseline_verdict.chipload, Verdict::Exceeds { .. });
    let mut all_candidates: Vec<OptimizeCandidate> = Vec::new();
    if !baseline_chipload_exceeds {
        let stage0_inputs = Stage0Inputs {
            rpm_baseline: baseline_rpm,
            feed_baseline_mm_min: baseline_op.feed_rate(),
            peak_power_baseline_kw: baseline_peak_power_kw(&baseline_verdict),
            machine: &machine,
            lut_row: matched_lut_row.as_ref(),
        };
        let k = solve_headroom_scale(&stage0_inputs);
        if k > 1.0 + 1e-6 {
            let scaled_op = apply_scale_to_op(&baseline_op, baseline_rpm, k, &machine);
            let delta = delta_against_baseline(&baseline_op, &scaled_op);
            if let Ok(candidate) = evaluate_candidate(
                &mut guard,
                &ctx,
                scaled_op,
                delta,
                SearchStage::Coarse,
                STAGE1_RESOLUTION_MM,
                cancel,
            ) {
                all_candidates.push(candidate);
            }
        }
    }

    if cancel.load(Ordering::SeqCst) {
        drop(guard);
        return finalize_partial(baseline_candidate, all_candidates);
    }

    // 8. Stage 1: DOC variant grid for ops with DOC knobs. Anchored at
    //    the headroom-point op when available, baseline op otherwise.
    if has_doc_knob(ctx.operation_kind) {
        let anchor_op = all_candidates
            .first()
            .map(|c| c.params.clone())
            .unwrap_or_else(|| baseline_op.clone());
        let anchor_doc = anchor_op
            .depth_per_pass()
            .unwrap_or_else(|| baseline_op.depth_per_pass().unwrap_or(1.5));
        let doc_variants = build_doc_variants(
            anchor_doc,
            matched_lut_row.as_ref(),
            ctx.operation_kind,
        );
        for &doc in &doc_variants {
            if cancel.load(Ordering::SeqCst) {
                break;
            }
            // Skip if this DOC matches the anchor — no point re-simming
            // the same params.
            if (doc - anchor_doc).abs() < DOC_DEDUP_TOLERANCE_MM {
                continue;
            }
            let candidate_op = apply_doc_to_op(&anchor_op, doc);
            let delta = delta_against_baseline(&baseline_op, &candidate_op);
            if let Ok(candidate) = evaluate_candidate(
                &mut guard,
                &ctx,
                candidate_op,
                delta,
                SearchStage::Coarse,
                STAGE1_RESOLUTION_MM,
                cancel,
            ) {
                all_candidates.push(candidate);
            }
        }
    }

    if cancel.load(Ordering::SeqCst) {
        drop(guard);
        return finalize_partial(baseline_candidate, all_candidates);
    }

    // 9. Stage 2: top-3 by cycle time, re-eval at full resolution.
    let stage2_seeds = select_stage2_candidates(all_candidates, STAGE2_SURVIVOR_COUNT);
    let stage2_candidates = match refine_stage2(&mut guard, &ctx, stage2_seeds, cancel) {
        Ok(c) => c,
        Err(_) => {
            drop(guard);
            return OptimizeOutcome::NoSafeImprovement {
                reason: RefuseReason::NoImprovementFound,
                explanation:
                    "candidate evaluation failed at full resolution — partial result returned"
                        .to_owned(),
            };
        }
    };

    // 10. Drop the guard explicitly so the baseline is restored before
    //     building the outcome (which references the candidates,
    //     not the session). The outcome is returned to the caller; the
    //     caller's view of `session` is now back at the baseline.
    drop(guard);
    build_outcome(baseline_candidate, stage2_candidates)
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
    if !samples_rpm.is_empty() {
        return samples_rpm[samples_rpm.len() / 2];
    }
    op_rpm
        .map(f64::from)
        .unwrap_or_else(|| machine.rpm_range().0)
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
        diameter_mm: tool.diameter(),
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
    fn capture(
        session: &ProjectSession,
        toolpath_index: usize,
    ) -> Result<Self, SessionError> {
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

/// Build the DOC candidate grid for a Stage-1 sweep. Always includes
/// the baseline (`baseline_doc_mm`) as a control candidate; the lo and
/// hi endpoints come from the LUT row's calibrated bounds (clamped by
/// the multiplier envelope) or fall back to the multiplier envelope
/// alone.
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
pub fn feeds_auto_for_candidate(
    baseline: &FeedsAutoMode,
    delta: &ParamDelta,
) -> FeedsAutoMode {
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
    pub(crate) fn from_session(
        session: &ProjectSession,
        toolpath_index: usize,
    ) -> Option<Self> {
        let tc = session.get_toolpath_config(toolpath_index)?;
        let tool_cfg = session.get_tool(crate::compute::tool_config::ToolId(tc.tool_id))?;
        let tool = crate::compute::cutter::build_cutter(tool_cfg);
        let spec = tc.operation.spec();
        Some(Self {
            toolpath_index,
            toolpath_id: tc.id,
            operation_kind: tc.operation.op_type(),
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
    let sim_result = session_ref.simulation_result().ok_or_else(|| {
        SessionError::Simulation(crate::compute::simulate::SimulationError::Cancelled)
    })?;
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
        };
    }

    let baseline_cycle = baseline.cycle_time_s;
    let any_safe_and_faster = candidates.iter().any(|c| {
        candidate_is_safe(c)
            && c.cycle_time_s + RECOMMENDATION_CYCLE_DELTA_S < baseline_cycle
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
        return OptimizeOutcome::NoSafeImprovement {
            reason: RefuseReason::NoImprovementFound,
            explanation,
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

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod stage0_tests {
    use super::*;
    use crate::machine::{ChipLoadFormula, MachineProfile, PowerModel, RigidityProfile, SpindleConfig};

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
            10_000.0, // generous feed cap
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
        assert!(
            (session.toolpath_configs()[0].operation.feed_rate() - baseline_feed).abs() < 1e-9
        );
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
            assert!(!guard.session_mut().toolpath_configs()[0].feeds_auto.feed_rate);
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
        s.add_toolpath(0, make_tc(operation, s.tools()[0].id.0)).unwrap();
        s
    }

    #[test]
    fn drill_op_yields_skipped() {
        let mut session = session_with_op(OperationConfig::Drill(DrillConfig::default()));
        let trace = empty_trace();
        let cancel = AtomicBool::new(false);
        let outcome = optimize_toolpath(&mut session, &trace, 0, &cancel);
        assert!(matches!(
            outcome,
            OptimizeOutcome::Skipped {
                reason: RefuseReason::SteadyStateSamplesNotPresent
            }
        ), "got {outcome:?}");
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
        let outcome = OptimizeOutcome::Ranked(vec![baseline, faster_safe.clone()]);
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
            OptimizeOutcome::NoSafeImprovement { reason, explanation } => {
                assert!(matches!(reason, RefuseReason::NoImprovementFound));
                assert!(
                    explanation.contains("no candidates"),
                    "explanation should say no candidates were produced: {explanation}"
                );
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
            OptimizeOutcome::NoSafeImprovement { explanation, .. } => {
                assert!(
                    explanation.contains("no candidate beat the baseline"),
                    "explanation should mention slower-than-baseline: {explanation}"
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
            OptimizeOutcome::NoSafeImprovement { explanation, .. } => {
                assert!(
                    explanation.contains("gate limit"),
                    "explanation should mention gate limit: {explanation}"
                );
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
}
