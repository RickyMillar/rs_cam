//! The cross-field invariant pass: `enforce_invariants` and the clamps and
//! back-offs it runs — plunge against feed, stepover against diameter and
//! predicted runtime, DPP against rigidity, cutting length and predicted
//! deflection — with the back-off thresholds they read.
//!
//! Split out of `feeds/suggest.rs` by P4; every item is unchanged apart from
//! its visibility and the `use` lines.

use crate::compute::catalog::OperationConfig;
use crate::compute::tool_config::ToolConfig;
use crate::feeds::{OperationFamily as FeedsOperationFamily, PassRole};
use crate::machine::MachineProfile;
use crate::material::Material;

use super::adaptive_entry::{
    check_plunge_entry_stability, pick_adaptive3d_clearing_strategy, pick_adaptive3d_entry_style,
    rescale_feed_to_final_geometry,
};
use super::axial_envelope::{pick_axial_envelope, recompute_chipload_bounds_for_dpp};
use super::{SuggestContext, SuggestWarning};

/// Deflection ceiling (µm) for the v1.1 DPP back-off. When the closed-form
/// predictor in [`crate::feeds::predict::predict_peak_deflection_um`]
/// projects a tip deflection above this value, [`enforce_invariants`]
/// iteratively reduces DPP by 20% per step (max 5 steps, floor
/// [`DEFLECTION_BACKOFF_DPP_FLOOR_MM`]) until the prediction clears.
///
/// Until 2026-08-13 it had a second consumer: the retired pass 8 feed-up
/// recalibration refused a solve whose post-write deflection landed within
/// a 10 µm headroom of this gate. That verify was a documented no-op on the
/// current feed-independent predictor and went with the pass (Checkpoint
/// J-1); the headroom constant was deleted alongside it.
///
/// 200 µm matches the post-sim `tool_load::deflection` critical
/// threshold directly. The predictor over-shoots post-sim by ~36% on
/// the Wanaka Back Rough motivating case (487 µm predicted vs 358 µm
/// observed), so this is already conservative — backing off against
/// 80% of 200 µm would trigger too aggressively.
///
/// Drift caveat: the +36% safe-side bias is what keeps the back-off
/// conservative. If
/// [`crate::feeds::predict::predict_peak_deflection_um`]'s underlying
/// constants (especially
/// [`crate::feeds::predict::ENDMILL_CORE_FRACTION`] = 0.7) get tuned
/// closer to the post-sim integrator in
/// [`crate::tool_load::deflection`] / `ToolDefinition::tip_deflection_mm`,
/// this threshold needs re-evaluating — a predictor with smaller
/// systematic over-shoot would push the operating point closer to the
/// real 200 µm boundary.
pub(super) const DEFLECTION_BACKOFF_TARGET_UM: f64 = 200.0;

/// v1.1 combined-Suggest step 2: minimum DPP the deflection back-off
/// loop is allowed to write (mm). Prevents pathological cases where
/// the predictor over-shoots so badly that iteration would zero out
/// DPP. If the loop bails on this floor, the warning still fires
/// showing the user the predicted µm at the floor — that's the "unmet
/// gate" signal until v2 adds a structured unmet-condition surface.
pub(super) const DEFLECTION_BACKOFF_DPP_FLOOR_MM: f64 = 0.5;

/// v1.1 combined-Suggest step 2: per-iteration DPP reduction factor.
/// 0.8 → ~20% drop per step; with the 5-iteration cap a starting DPP
/// of 9 mm hits 9 × 0.8⁵ ≈ 2.95 mm before bailing, which is sufficient
/// headroom for every motivating case (Wanaka Back Rough settles
/// inside 3 iterations).
const DEFLECTION_BACKOFF_FACTOR: f64 = 0.8;

/// v1.1 combined-Suggest step 2: maximum back-off iterations.
const DEFLECTION_BACKOFF_MAX_ITERATIONS: u8 = 5;

/// v1.2 combined-Suggest: runtime-sanity target for the stepover
/// back-off loop. When the closed-form move-count predictor in
/// [`crate::feeds::predict::predict_move_count`] projects more samples
/// than this, [`enforce_invariants`] iteratively raises stepover by 50%
/// per step (max [`STEPOVER_BACKOFF_MAX_ITERATIONS`] iterations, ceiling
/// `tool.diameter × `[`STEPOVER_BACKOFF_DIAMETER_FRACTION`]).
///
/// 500 000 is the practical "this will hang generation" threshold on
/// the Wanaka motivating case. On a 140×150 mm stock the Wanaka 3D
/// Finish 0.03 mm stepover predicts ~4.6 M moves; clearing 500 k
/// requires raising stepover to roughly 0.3 mm — still tight for a
/// fine-finish path, but workable.
pub(super) const STEPOVER_BACKOFF_TARGET_MOVES: u64 = 500_000;

/// v1.2 combined-Suggest: ceiling fraction of tool diameter the
/// stepover back-off is allowed to write. 0.5 (50% of D) is the
/// generally-accepted "you are no longer doing the operation the user
/// asked for" threshold — beyond 50% stepover a finish pass becomes a
/// roughing pass. The diameter floor prevents the back-off from raising
/// stepover into roughing territory in pursuit of runtime sanity.
pub(super) const STEPOVER_BACKOFF_DIAMETER_FRACTION: f64 = 0.5;

/// v1.2 combined-Suggest: per-iteration stepover multiplier. 1.5 ×
/// (vs deflection's 0.8 ×) — stepover is being *raised*, not lowered,
/// so the factor lives on the high side of 1.0. With 5 iterations the
/// loop can lift stepover by up to 1.5⁵ ≈ 7.6× before bailing on the
/// diameter-fraction ceiling.
const STEPOVER_BACKOFF_FACTOR: f64 = 1.5;

/// v1.2 combined-Suggest: maximum back-off iterations.
pub(super) const STEPOVER_BACKOFF_MAX_ITERATIONS: u8 = 5;

// RETIRED 2026-08-13 (Checkpoint J-1): `FEED_RAISE_DEFLECTION_RECAL_HEADROOM_UM
// = 10.0` — the refusal headroom under `DEFLECTION_BACKOFF_TARGET_UM` that the
// retired pass 8 feed-up verify measured against. Its only consumer went with
// the pass; the v1.1 DPP back-off uses the bare 200 µm target.

pub(super) fn enforce_invariants(
    operation: &mut OperationConfig,
    tool: &ToolConfig,
    machine: &MachineProfile,
    material: &Material,
    pass_role: PassRole,
    context: SuggestContext<'_>,
) -> Vec<SuggestWarning> {
    // NOTE: order matters and must mirror the historical monolithic
    // implementation exactly. In particular the stepover runtime
    // back-off runs *before* the DPP-block, and the plunge-entry
    // stability warning fires *after* the deflection back-off has
    // mutated DPP — both intentional.
    //
    // Pass 0 (Phase 3 — `planning/cutter_axial_constraints_2026-06-06.md`
    // §5.3): unified axial-DOC envelope runs FIRST so downstream passes
    // (rigidity clamp / deflection back-off / chipload recalibration)
    // operate on a value consistent with the cutter geometry envelope.
    // When the pass mutates DPP, `chipload_bounds` is stale — the
    // doc-derating ratio just changed — so we re-derive it from the
    // matched LUT row before the chipload-recalibration pass downstream
    // consumes it.
    //
    // The geometry as it stands BEFORE any pass runs — i.e. what
    // `apply_feeds_subset` wrote off the calculator's result. Pass 9 compares
    // the final values against these to decide whether any pass actually
    // moved the cut, and only re-derives the feed when one did. Captured here
    // rather than reconstructed from the calculator's result because that
    // result is unrounded and these are not; comparing like with like is what
    // keeps an untouched operation's feed byte-identical.
    let entry_stepover = operation.stepover();
    let entry_dpp = operation.depth_per_pass();
    let mut working_context = context;
    let (axial_envelope_warnings, dpp_mutated) =
        pick_axial_envelope(operation, tool, material, working_context);
    let mut warnings = Vec::new();
    warnings.extend(axial_envelope_warnings);
    if dpp_mutated && let Some(new_dpp) = operation.depth_per_pass() {
        let fresh_bounds = recompute_chipload_bounds_for_dpp(
            working_context.matched_lut_row,
            working_context.effective_diameter_mm,
            operation,
            new_dpp,
        );
        working_context.chipload_bounds = fresh_bounds;
    }
    let context = working_context;
    warnings.extend(clamp_plunge_to_feed(operation));
    warnings.extend(clamp_stepover_to_diameter(operation, tool));
    warnings.extend(backoff_stepover_for_runtime(operation, tool, context));
    warnings.extend(clamp_dpp_to_rigidity(operation, tool, machine, pass_role));
    warnings.extend(clamp_dpp_to_cutting_length(operation, tool));
    warnings.extend(backoff_dpp_for_deflection(
        operation, tool, material, machine, pass_role,
    ));
    // v3.3b: strategy-aware entry-style rewrite must run BEFORE the
    // plunge-entry-stability warning — when scope = StrategyAndFeeds the
    // rewrite changes Plunge → Ramp and the downstream warning then
    // finds nothing to flag. Under scope = FeedsWithGates the rewrite
    // short-circuits and the warning still fires for the operator.
    warnings.extend(pick_adaptive3d_entry_style(
        operation, tool, pass_role, context,
    ));
    // v3.3c: clearing-strategy recommendation is warn-only (auto-rewrite
    // deferred to v4 pending classifier calibration) — reads, never writes.
    warnings.extend(pick_adaptive3d_clearing_strategy(
        operation, pass_role, context,
    ));
    warnings.extend(check_plunge_entry_stability(operation, tool, pass_role));
    // Pass 8 — the arc-fit chipload feed recalibration — was RETIRED here on
    // 2026-08-13 (Checkpoint J-1, BINDING). Suggest now emits the un-lifted
    // feed the calculator produced. See the retirement note on
    // `feeds::predict` and `planning/review_2026-08-08/ARC_FIT_RATIO_EVIDENCE.md`.
    //
    // Pass 9 runs LAST, and must: every pass above may still move the stepover
    // or the DPP, and this one exists to reconcile the feed with wherever they
    // finally land.
    warnings.extend(rescale_feed_to_final_geometry(
        operation,
        tool,
        machine,
        entry_stepover,
        entry_dpp,
        context,
    ));
    warnings
}

/// Pass 1: clamp plunge_rate to feed_rate when plunge exceeds feed.
pub(super) fn clamp_plunge_to_feed(operation: &mut OperationConfig) -> Vec<SuggestWarning> {
    let mut warnings = Vec::new();
    let feed_rate = operation.feed_rate();
    let plunge_rate = operation.plunge_rate();
    if feed_rate.is_finite() && plunge_rate.is_finite() && plunge_rate > feed_rate {
        operation.set_plunge_rate(feed_rate);
        warnings.push(SuggestWarning::PlungeClampedToFeed {
            requested: plunge_rate,
            capped: feed_rate,
        });
    }
    warnings
}

/// Pass 2: clamp stepover to tool diameter when stepover exceeds diameter.
fn clamp_stepover_to_diameter(
    operation: &mut OperationConfig,
    tool: &ToolConfig,
) -> Vec<SuggestWarning> {
    let mut warnings = Vec::new();
    if let Some(stepover) = operation.stepover()
        && stepover.is_finite()
        && tool.diameter.is_finite()
        && tool.diameter > 0.0
        && stepover > tool.diameter
    {
        operation.set_stepover(tool.diameter);
        warnings.push(SuggestWarning::StepoverClampedToToolDiameter {
            requested: stepover,
            capped: tool.diameter,
        });
    }
    warnings
}

/// Pass 3: v1.2 combined-Suggest runtime-sanity stepover floor.
///
/// The LUT + scallop-height path can write a stepover so small
/// (≤0.1 mm on a small-tip ball, e.g. 0.03 mm on the 2 mm-tip
/// tapered ball in Wanaka 3D Finish 6) that the resulting raster
/// toolpath would emit millions of moves on a normal stock envelope
/// and effectively block `generate_all`. The closed-form move-count
/// predictor consumes the operation's stepover + model bbox + tool
/// diameter and projects an upper-bound move count; when that
/// exceeds [`STEPOVER_BACKOFF_TARGET_MOVES`] we iteratively raise
/// stepover by 50 % per step (max 5 iterations, ceiling
/// tool.diameter × 0.5) until the predicted move count clears the
/// target or the loop bottoms out on the diameter-fraction ceiling.
///
/// Model-bbox is read off `context.model_bbox`. When it's None the
/// predictor returns 0 (no constraint signal) and the loop never
/// executes. Feature-driven ops (V-carve, drill, project_curve,
/// trace, profile) also return 0 — same short-circuit.
fn backoff_stepover_for_runtime(
    operation: &mut OperationConfig,
    tool: &ToolConfig,
    context: SuggestContext<'_>,
) -> Vec<SuggestWarning> {
    let mut warnings = Vec::new();
    if let Some(stepover_pre) = operation.stepover()
        && stepover_pre.is_finite()
        && stepover_pre > 0.0
        && tool.diameter.is_finite()
        && tool.diameter > 0.0
    {
        let initial_moves =
            crate::feeds::predict::predict_move_count(operation, context.model_bbox, tool);
        if initial_moves > STEPOVER_BACKOFF_TARGET_MOVES {
            let ceiling = tool.diameter * STEPOVER_BACKOFF_DIAMETER_FRACTION;
            let mut iterations: u8 = 0;
            let mut current_so = stepover_pre;
            let mut current_moves = initial_moves;

            while current_moves > STEPOVER_BACKOFF_TARGET_MOVES
                && iterations < STEPOVER_BACKOFF_MAX_ITERATIONS
            {
                let raised = (current_so * STEPOVER_BACKOFF_FACTOR).min(ceiling);
                // Already at (or above) the ceiling — bail. Without
                // this we'd spin the iteration counter without writing
                // any new value.
                if raised <= current_so {
                    break;
                }
                current_so = raised;
                operation.set_stepover(current_so);
                current_moves =
                    crate::feeds::predict::predict_move_count(operation, context.model_bbox, tool);
                iterations = iterations.saturating_add(1);
            }

            if iterations > 0 {
                tracing::debug!(
                    requested_mm = stepover_pre,
                    raised_mm = current_so,
                    predicted_moves_at_requested = initial_moves,
                    predicted_moves_at_raised = current_moves,
                    iterations,
                    "Suggest runtime-sanity back-off raised stepover"
                );
                warnings.push(SuggestWarning::StepoverRaisedForRuntime {
                    requested_mm: stepover_pre,
                    raised_mm: current_so,
                    predicted_moves_at_requested: initial_moves,
                    predicted_moves_at_raised: current_moves,
                    iterations,
                });
            }
        }
    }
    warnings
}

/// Pass 4: clamp DPP to the spindle-rigidity ceiling on roughing passes.
///
/// Adaptive ops are a "deep, narrow" engagement strategy — low
/// radial WOC lets the cutter take a high axial DOC the conventional
/// roughing factor doesn't allow. Pre-2026-06-02 the clamp used
/// `doc_roughing_factor` uniformly, stripping the upstream adaptive
/// ap (computed via `adaptive_doc_factor` at feeds/mod.rs:913) back
/// down to the conventional ceiling — defeating the adaptive
/// paradigm. The clamp now branches on feeds family.
///
/// Audit finding: BUG 1 — "Adaptive DOC clamped by conventional-
/// roughing factor" (workflow `w39ma2j1y`).
fn clamp_dpp_to_rigidity(
    operation: &mut OperationConfig,
    tool: &ToolConfig,
    machine: &MachineProfile,
    pass_role: PassRole,
) -> Vec<SuggestWarning> {
    let mut warnings = Vec::new();
    if let Some(current) = operation.depth_per_pass()
        && matches!(pass_role, PassRole::Roughing)
    {
        let is_adaptive_family =
            operation.op_type().spec().feeds_family == FeedsOperationFamily::Adaptive;
        let factor = if is_adaptive_family {
            machine.rigidity.adaptive_doc_factor
        } else {
            machine.rigidity.doc_roughing_factor
        };
        let cap = factor * tool.diameter;
        if current.is_finite() && cap.is_finite() && cap > 0.0 && current > cap {
            operation.set_depth_per_pass(cap);
            warnings.push(SuggestWarning::RoughingDepthClampedToRigidity {
                requested: current,
                capped: cap,
            });
        }
    }
    warnings
}

/// Pass 5: clamp DPP to the tool's cutting length.
///
/// Reads DPP fresh after the rigidity-clamp pass; runs for all pass
/// roles (not roughing-only).
fn clamp_dpp_to_cutting_length(
    operation: &mut OperationConfig,
    tool: &ToolConfig,
) -> Vec<SuggestWarning> {
    let mut warnings = Vec::new();
    if let Some(current) = operation.depth_per_pass() {
        let cap = tool.cutting_length;
        if current.is_finite() && cap.is_finite() && cap > 0.0 && current > cap {
            operation.set_depth_per_pass(cap);
            warnings.push(SuggestWarning::DepthClampedToCuttingLength {
                requested: current,
                capped: cap,
            });
        }
    }
    warnings
}

/// Pass 6: v1.1 combined-Suggest deflection-aware DPP back-off.
///
/// The rigidity clamp and cutting-length clamp guarantee structural /
/// geometric safety; this clamp adds a *physical* safety check by
/// consulting the closed-form deflection predictor at the current
/// operating point, then iteratively backing DPP off by 20% per step
/// until the predicted tip deflection clears the 200 µm critical
/// threshold (or the loop bottoms out on the 0.5 mm DPP floor).
///
/// Roughing-only (same gate as the rigidity clamp): finishing passes
/// already run shallow DPPs by design and the deflection envelope
/// rarely threatens them.
///
/// The predictor returns 0 µm for refusal cases (V-bit, drill, un-
/// validated material, zero feed) — the loop's `> target` condition
/// short-circuits trivially and no back-off occurs.
fn backoff_dpp_for_deflection(
    operation: &mut OperationConfig,
    tool: &ToolConfig,
    material: &Material,
    machine: &MachineProfile,
    pass_role: PassRole,
) -> Vec<SuggestWarning> {
    let mut warnings = Vec::new();
    if let Some(current) = operation.depth_per_pass()
        && matches!(pass_role, PassRole::Roughing)
        && current.is_finite()
        && current > DEFLECTION_BACKOFF_DPP_FLOOR_MM
    {
        let pre_backoff_dpp = current;
        let initial_prediction =
            crate::feeds::predict::predict_peak_deflection_um(operation, tool, material, machine);
        let predicted_initial_um = initial_prediction.predicted_um;
        let mut predicted_um = predicted_initial_um;
        let mut iterations: u8 = 0;
        let mut dpp = current;

        while predicted_um > DEFLECTION_BACKOFF_TARGET_UM
            && iterations < DEFLECTION_BACKOFF_MAX_ITERATIONS
            && dpp > DEFLECTION_BACKOFF_DPP_FLOOR_MM
        {
            dpp = (dpp * DEFLECTION_BACKOFF_FACTOR).max(DEFLECTION_BACKOFF_DPP_FLOOR_MM);
            operation.set_depth_per_pass(dpp);
            let next = crate::feeds::predict::predict_peak_deflection_um(
                operation, tool, material, machine,
            );
            predicted_um = next.predicted_um;
            iterations = iterations.saturating_add(1);
        }

        if iterations > 0 {
            tracing::debug!(
                requested_mm = pre_backoff_dpp,
                capped_mm = dpp,
                predicted_um_at_requested = predicted_initial_um,
                predicted_um_at_capped = predicted_um,
                iterations,
                "Suggest deflection back-off reduced DPP"
            );
            warnings.push(SuggestWarning::DppCappedByDeflection {
                requested_mm: pre_backoff_dpp,
                capped_mm: dpp,
                predicted_um_at_requested: predicted_initial_um,
                predicted_um_at_capped: predicted_um,
                iterations,
            });
        }
    }
    warnings
}
