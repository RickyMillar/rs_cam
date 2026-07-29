//! Stage-1 candidate generation, the per-candidate evaluation driver,
//! and Stage-2 refinement.
//!
//! - [`OptimizeCandidate`] — the record produced by every search-stage
//!   evaluation; what the orchestrator hands to outcome dispatch.
//! - [`evaluate_candidate`] — the single per-candidate driver. Apply →
//!   regenerate → sim → gate → produce an `OptimizeCandidate`. Every
//!   Stage-1 / Stage-2 / retarget candidate goes through this.
//! - Stage-1 grid builders — `build_doc_variants`,
//!   `build_stepover_variants`, `build_scallop_height_variants`.
//! - Stage-2 refinement — `select_stage2_candidates`, `refine_stage2`.
//! - Pipeline helpers — `finalize_partial`, `has_doc_knob`.

use std::sync::atomic::AtomicBool;

use serde::{Deserialize, Serialize};

use crate::compute::catalog::{OperationConfig, OperationType};
use crate::feeds::vendor_lookup::MatchedRow;
use crate::session::{ProjectSession, SessionError, SimulationOptions};
use crate::tool_load::verdict::ToolpathLoadVerdict;
use crate::tool_load::{ToolpathLoadContext, evaluate_toolpath};

use super::axes::SearchAxis;
use super::bounds;
use super::context::{
    BaselineRestoreGuard, EvaluationContext, air_cut_fraction_of_total_runtime_from_trace,
    cycle_time_from_trace,
};
use super::delta::{GateDeltas, ParamDelta};
use super::policy::{self, SearchPolicy};
use super::rank::composite_score;
use super::{SearchStage, search_policy, tolerance_bands_from_policy};

/// One candidate's full evaluation record. Populated by the optimizer
/// during Stage 0/1/2; each field is sim-measured (or, for the baseline
/// candidate at index 0, sourced from `baseline_trace`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizeCandidate {
    /// The full operation config that would be applied. Carries
    /// unchanged fields too, so Apply just needs to write this into
    /// `session.toolpath_configs[idx].operation`.
    pub params: OperationConfig,
    /// Diff from the baseline params, for display at Apply time.
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
    /// Per-criterion delta vs the baseline candidate at index 0. `None`
    /// for the baseline itself. Populated by the tier dispatcher in
    /// `build_outcome` so consumers don't have to recompute.
    #[serde(default)]
    pub gate_deltas: Option<GateDeltas>,
    /// Roadmap F.12 — fraction of **total runtime (cutting + rapids)** this
    /// toolpath spent in air in this candidate's sim. Range 0.0..=1.0.
    /// `None` when the trace lacked a per-toolpath summary (failed sim).
    ///
    /// LH-1: this doc used to say "fraction of cutting time" while the code
    /// divided by total runtime — the same two-denominators-one-name defect
    /// the rest of `MEASUREMENT_DOMAINS.md` LH-1 covers. The VALUE is
    /// unchanged (total runtime, matching every threshold in the codebase);
    /// the field and its producer are now named for it. The serialized key
    /// stays `air_cut_pct` so persisted optimizer results keep loading.
    #[serde(default, rename = "air_cut_pct")]
    pub air_cut_fraction_of_total_runtime: Option<f64>,
}

/// True if this op exposes a meaningful `depth_per_pass` knob.
///
/// G2/G3 history: this allowlist used to be split across multiple
/// places. Now a single source of truth so the Stage-1 grid builder
/// and the strategy harness can share a definition. The list is hand-
/// maintained because the catalog's surface API doesn't yet have a
/// "this op has DOC" flag — per-op `OperationParams::depth_per_pass`
/// returns `Some(_)` for the same set, but consulting that on every
/// candidate is awkward when we just need the kind.
///
/// Operation kinds:
/// - G1: Adaptive3d, Pocket, Adaptive, Rest, Face — primary clearing
///   ops with explicit DOC.
/// - G2: Profile, Zigzag — newer 2D clearing ops; both expose DOC via
///   `OperationParams`.
/// - G3: Trace, RampFinish, Waterline — Trace already exposes
///   `depth_per_pass()`; RampFinish wraps `max_stepdown` and Waterline
///   wraps `z_step` as DOC-equivalent axes via the trait.
///
/// `run_stage_1_grid` collapses the stepover and scallop_height
/// dimensions for ops that don't expose those, so adding ops here
/// doesn't fan out duplicate sims.
pub(crate) fn has_doc_knob(op_kind: OperationType) -> bool {
    matches!(
        op_kind,
        OperationType::Adaptive3d
            | OperationType::Pocket
            | OperationType::Adaptive
            | OperationType::Rest
            | OperationType::Face
            | OperationType::Profile
            | OperationType::Zigzag
            | OperationType::Trace
            | OperationType::RampFinish
            | OperationType::Waterline
    )
}

/// Build a partial `OptimizeOutcome` after cancellation. If we have
/// no candidates yet, surface NoSafeImprovement; otherwise dispatch
/// the candidates we managed to evaluate. The user sees a partial
/// rollup rather than nothing — open-and-walk-away cancellation
/// shouldn't lose the work done before they hit Cancel.
pub(crate) fn finalize_partial(
    baseline: OptimizeCandidate,
    candidates: Vec<OptimizeCandidate>,
    machine: &crate::machine::MachineProfile,
) -> super::OptimizeOutcome {
    if candidates.is_empty() {
        let narrative = super::OutcomeNarrative {
            explanation: "cancelled before any candidates were evaluated".to_owned(),
            ..super::OutcomeNarrative::default()
        };
        return super::OptimizeOutcome::no_safe_improvement(
            vec![baseline],
            super::RefuseReason::NoImprovementFound,
            narrative,
        );
    }
    super::build_outcome(baseline, candidates, machine)
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
    let policy = search_policy();
    let bounds = bounds::resolve_doc_bounds(baseline_doc_mm, lut_row, op_type, policy);
    build_geometry_variants_from_bounds(
        &bounds,
        op_type,
        SearchAxis::DepthPerPass,
        &policy.axes.doc,
        policy,
    )
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
    let policy = search_policy();
    let bounds = bounds::resolve_stepover_bounds(baseline_stepover_mm, lut_row, op_type, policy);
    build_geometry_variants_from_bounds(
        &bounds,
        op_type,
        SearchAxis::Stepover,
        &policy.axes.stepover,
        policy,
    )
}

/// Shared composer used by `build_doc_variants` / `build_stepover_variants`.
/// Combines the warm-start grid, outside-preferred probes (when policy
/// permits and the LUT envelope is present), and the operation's factory
/// default anchor. Sorts and dedupes per axis policy.
fn build_geometry_variants_from_bounds(
    bounds: &bounds::AxisBounds,
    op_type: OperationType,
    axis: SearchAxis,
    axis_policy: &policy::AxisPolicy,
    policy: &policy::SearchPolicy,
) -> Vec<f64> {
    let four_variant = matches!(op_type, OperationType::Pocket | OperationType::Adaptive);
    let n_points = if four_variant { 4 } else { 3 };
    let mut variants = bounds.warm_start_grid(n_points, axis_policy.midpoint_weight.value);

    // Outside-preferred probes — extends the search beyond the LUT
    // envelope when policy permits. This is the §1.3 wanaka TP 4 fix:
    // when LUT ae_max caps below operator intent, probes above LUT max
    // give the chipload gate retargeters a wider feasible set.
    variants.extend(bounds.outside_preferred_probes(axis_policy));

    // Factory default anchor — recovers a candidate at the well-tested
    // canonical setup when the user's baseline has drifted far from it.
    // Always added; dedup handles the no-op case where it lands inside
    // the warm-start grid.
    if let Some(default_v) = bounds::factory_default_for_axis(axis, op_type, policy) {
        variants.push(default_v);
    }

    variants.sort_by(f64::total_cmp);
    let dedup_tolerance = axis_policy.dedup_tolerance.value;
    variants.dedup_by(|a, b| (*a - *b).abs() < dedup_tolerance);
    variants
}

/// Build the scallop-height candidate grid for a Stage-1 sweep.
/// Multiplicative envelope only (no LUT clamping) — the LUT's `ae_*_mm`
/// bounds describe radial step in mm, which differs in units and
/// magnitude from `scallop_height` (a 0.1 mm scallop target on a 6 mm
/// ball produces ~1.55 mm radial step). Returns variants sorted
/// ascending; always includes the baseline. Floored at the policy's
/// scallop-height hard floor.
pub(crate) fn build_scallop_height_variants(baseline_scallop_mm: f64) -> Vec<f64> {
    let policy = search_policy();
    let axis_policy = &policy.axes.scallop_height;
    // Scallop height is a 3-point quality axis on Scallop ops. Use
    // `OperationType::Scallop` so the bounds resolver picks the
    // 3-variant multiplier path; Scallop is not in the four-variant
    // clearing set.
    let bounds =
        bounds::resolve_scallop_height_bounds(baseline_scallop_mm, OperationType::Scallop, policy);
    let mut variants = bounds.warm_start_grid(3, axis_policy.midpoint_weight.value);
    variants.sort_by(f64::total_cmp);
    let dedup_tolerance = axis_policy.dedup_tolerance.value;
    variants.dedup_by(|a, b| (*a - *b).abs() < dedup_tolerance);
    variants
}

// ── Candidate evaluation: apply → regen → sim → gate ──────────────────
//
// The single per-candidate driver. Apply the candidate's params via the
// restore guard, regen the toolpath, run a project sim at the
// requested resolution, and produce an `OptimizeCandidate` with
// sim-measured cycle time and gate verdict. Every Stage-1 and Stage-2
// candidate goes through this function — there is no second sim or
// second gate anywhere.

/// Evaluate one candidate end-to-end:
///   1. Apply candidate params via the restore guard.
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
///
/// **Panic isolation (R1, tech-debt review 2026-06-10).** Candidate
/// evaluation regenerates toolpaths at parameters the generators were
/// never exercised at interactively (the search space's hard floors —
/// 0.05 mm DOC / stepover). A panic anywhere in the geometry stack
/// (e.g. `cavalier_contours`' `parallel_offset` asserts on degenerate
/// inputs) must cost one candidate, not the whole optimize run, so the
/// inner evaluation runs under `catch_unwind` and a panic surfaces as
/// `SessionError::OperationFailed`. Grid/retarget loops already skip
/// `Err` candidates; `refine_stage2`'s caller degrades to a partial
/// outcome. State safety: the params the panicking candidate applied
/// are overwritten by the next candidate's apply (or by the guard's
/// `Drop` restoring the baseline), and `generate_toolpath` /
/// `run_simulation` only publish results on success, so a mid-panic
/// leaves the previous (stale-tracked) result in place.
pub(crate) fn evaluate_candidate(
    guard: &mut BaselineRestoreGuard<'_>,
    ctx: &EvaluationContext,
    candidate_op: OperationConfig,
    delta: ParamDelta,
    stage: SearchStage,
    sim_resolution_mm: f64,
    cancel: &AtomicBool,
) -> Result<OptimizeCandidate, SessionError> {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        evaluate_candidate_inner(
            guard,
            ctx,
            candidate_op,
            delta,
            stage,
            sim_resolution_mm,
            cancel,
        )
    }));
    match result {
        Ok(r) => r,
        Err(payload) => {
            let msg = panic_payload_message(payload.as_ref());
            tracing::warn!(
                toolpath_id = ctx.toolpath_id.0,
                "candidate evaluation panicked; skipping candidate: {msg}"
            );
            Err(SessionError::OperationFailed(format!(
                "candidate evaluation panicked: {msg}"
            )))
        }
    }
}

/// Best-effort human-readable message from a `catch_unwind` payload.
/// `panic!("...")` yields `&str`; `panic!("{x}")`-style formatting
/// yields `String`; anything else gets a placeholder.
fn panic_payload_message(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_owned()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "non-string panic payload".to_owned()
    }
}

fn evaluate_candidate_inner(
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
    let toolpath_index = ctx.toolpath_index;

    // Apply.
    guard.session_mut().apply_toolpath_param_snapshot(
        toolpath_index,
        candidate_op.clone(),
        dressups,
        face_selection,
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
        // F-035: the optimizer's candidate-scoring sim runs with
        // predicted-feed plumbing OFF so the optimizer ranks
        // candidates against the same commanded-feed gates the
        // production simulator currently uses. Flipping this on is a
        // follow-up (F-036 territory).
        use_predicted_feed_in_gates: false,
        // F-036b: optimizer's candidate-scoring sim also runs with
        // adaptive feed modulation OFF — the optimizer ranks
        // candidates against the commanded feed in the IR. The
        // optimizer and the feed modulator are two independent
        // bridges over the same engagement summary; running them
        // together would conflate their effects.
        adaptive_feed_modulation: false,
        modulation_strategy: crate::feed_modulation::ModulationStrategy::ConstrainedMax,
        modulation_aggressiveness: 1.0,
    };
    guard.session_mut().run_simulation(&sim_opts, cancel)?;

    // Pull the trace out (Arc<SimulationCutTrace>) and score via the
    // gate. The sim result lives on `session.simulation` after
    // `run_simulation` succeeds.
    let session_ref: &ProjectSession = &*guard.session_mut();
    let sim_result = session_ref
        .simulation_result()
        .ok_or(SessionError::Simulation(
            crate::compute::simulate::SimulationError::Cancelled,
        ))?;
    let trace = sim_result.cut_trace.as_deref();

    // Span lookup for D6/D7: each candidate just regenerated the
    // toolpath at index `toolpath_index`, so the cached compute result's
    // `annotated.spans` are fresh. None when generation hasn't produced
    // a result yet (callers downstream degrade to engagement-only
    // locality labels). F2.2: also None when a transform invalidated
    // the spans (TSP split) — corrupted ancestry must not stamp gate
    // verdicts.
    let spans: Option<&[crate::toolpath_spans::Span]> = session_ref
        .get_result(toolpath_index)
        .filter(|r| r.annotated().spans_valid)
        .map(|r| r.annotated().spans.as_slice());
    let drill_op = session_ref
        .get_result(toolpath_index)
        .and_then(|r| r.drill_op());
    let load_ctx = ToolpathLoadContext {
        toolpath_id: ctx.toolpath_id,
        tool: &ctx.tool,
        material: &ctx.material,
        operation_family: ctx.lut_op_family,
        pass_role: ctx.lut_pass_role,
        operation_feed_rate_mm_min: candidate_op.feed_rate(),
        operation_kind: ctx.operation_kind,
        spans,
        drill_op: drill_op.map(|arc| arc.as_ref()),
    };
    let policy_tolerance = tolerance_bands_from_policy(search_policy());
    let verdict = evaluate_toolpath(
        &load_ctx,
        trace,
        Some(session_ref.machine()),
        &policy_tolerance,
    );
    let cycle_time_s = trace
        .and_then(|t| cycle_time_from_trace(t, ctx.toolpath_id))
        .unwrap_or(f64::INFINITY);
    // Roadmap F.12 — surface air-cut alongside cycle_time, denominator named.
    let air_cut_fraction_of_total_runtime =
        trace.and_then(|t| air_cut_fraction_of_total_runtime_from_trace(t, ctx.toolpath_id));

    Ok(OptimizeCandidate {
        params: candidate_op,
        delta,
        cycle_time_s,
        verdict,
        stage,
        reconciled_cycle_time_s: None,
        reconciled_verdict: None,
        gate_deltas: None,
        air_cut_fraction_of_total_runtime,
    })
}

// ── Stage 2 candidate refinement ──────────────────────────────────────
//
// Stage 2 takes the top-N by Stage-1 composite_score (G16 §11 layer 2b
// — replaces the prior cycle-time-only sort) and re-evaluates them at
// 0.5mm dexel. The reported numbers on the rollup are always Stage-2
// numbers.

/// Sort `stage1_winners` by descending `composite_score` against
/// `baseline` and keep the top `n` (highest-score) entries. The score
/// weights cycle savings against chipload-distance / power-overuse /
/// deflection-overuse penalties — see [`super::rank`] for the formula.
pub(crate) fn select_stage2_candidates(
    mut stage1_winners: Vec<OptimizeCandidate>,
    baseline: &OptimizeCandidate,
    policy: &SearchPolicy,
    n: usize,
) -> Vec<OptimizeCandidate> {
    stage1_winners.sort_by(|a, b| {
        composite_score(b, baseline, policy).total_cmp(&composite_score(a, baseline, policy))
    });
    stage1_winners.truncate(n);
    stage1_winners
}

/// Re-evaluate each Stage-1 winner at Stage-2 resolution. Returns the
/// list of refined candidates in the same order as the input. Each
/// re-evaluation goes through `evaluate_candidate` so the Stage-2
/// numbers come from the same simulator and gate as Stage 1 — there
/// is no second model anywhere.
///
/// R1 isolation: a candidate that fails (or panics) at Stage-2
/// resolution is dropped — matching the Stage-1 grid/retarget loops —
/// rather than aborting the whole refinement. Only cancellation
/// propagates as `Err`, which the orchestrator turns into a partial
/// outcome.
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
        match evaluate_candidate(
            guard,
            ctx,
            c.params,
            c.delta,
            SearchStage::Refined,
            search_policy().stages.refined_resolution_mm.value,
            cancel,
        ) {
            Ok(candidate) => refined.push(candidate),
            Err(SessionError::Simulation(crate::compute::simulate::SimulationError::Cancelled)) => {
                return Err(SessionError::Simulation(
                    crate::compute::simulate::SimulationError::Cancelled,
                ));
            }
            Err(e) => {
                tracing::warn!(
                    toolpath_id = ctx.toolpath_id.0,
                    "stage-2 refinement dropped a candidate: {e:?}"
                );
            }
        }
    }
    Ok(refined)
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::panic_payload_message;

    // R1 isolation seam: the payload shapes catch_unwind hands back for
    // the panic styles the geometry stack actually produces.

    #[test]
    fn panic_payload_message_reads_static_str() {
        let payload = std::panic::catch_unwind(|| {
            panic!("start index should be less than or equal to end index if polyline is open")
        })
        .unwrap_err();
        assert_eq!(
            panic_payload_message(payload.as_ref()),
            "start index should be less than or equal to end index if polyline is open"
        );
    }

    #[test]
    fn panic_payload_message_reads_formatted_string() {
        let payload =
            std::panic::catch_unwind(|| panic!("assert failed at offset {}", 0.05)).unwrap_err();
        assert_eq!(
            panic_payload_message(payload.as_ref()),
            "assert failed at offset 0.05"
        );
    }

    #[test]
    fn panic_payload_message_handles_non_string_payload() {
        let payload = std::panic::catch_unwind(|| std::panic::panic_any(42_u32)).unwrap_err();
        assert_eq!(
            panic_payload_message(payload.as_ref()),
            "non-string panic payload"
        );
    }
}
