//! The cross-field invariant pass: `enforce_invariants` and the clamps and
//! back-offs it runs — plunge against feed, stepover against diameter and
//! predicted runtime, DPP against rigidity, cutting length and predicted
//! deflection — with the back-off thresholds they read.
//!
//! Split out of `feeds/suggest.rs` by P4; every item is unchanged apart from
//! its visibility and the `use` lines.

use crate::compute::catalog::OperationConfig;
use crate::compute::tool_config::ToolConfig;
use crate::feeds::PassRole;
use crate::machine::MachineProfile;
use crate::material::Material;

use super::adaptive_entry::{
    check_plunge_entry_stability, pick_adaptive3d_clearing_strategy, pick_adaptive3d_entry_style,
    recheck_power_after_rescale, rescale_feed_to_final_geometry,
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
/// threshold directly. That is the whole justification, and it stands on
/// its own.
///
/// It used to carry a second one: "the predictor over-shoots post-sim by
/// ~36% (487 µm predicted vs 358 µm observed), so this is already
/// conservative". **That margin no longer exists and the claim is
/// withdrawn.** It was measured on 2026-06-07 (`ba01f7a8`) against the
/// bespoke single-section predictor that `3b0dc487` deleted on
/// 2026-06-17. Since that commit the predictor and the post-sim gate call
/// ONE beam — `ToolDefinition::tip_deflection_mm` — so no beam-level
/// over-shoot is left to lean on.
///
/// A divergence does remain, but it is a different quantity: the
/// predictor forecasts the engagement, and the gate measures it. Nobody
/// has measured that residual. Do not treat this threshold as carrying a
/// hidden safety margin. If a margin is wanted here, state it and source
/// it; do not inherit one from a retired model.
pub(crate) const DEFLECTION_BACKOFF_TARGET_UM: f64 = 200.0;

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
    // The step ladder (operator ruling 1, 2026-09-24): with a ladder the
    // apply funnel keeps the operator's base step and does not write the
    // calculator's depth. The feed was sized at the calculator's depth, so
    // that depth is the entry value that pass 9 compares with. Else a ladder
    // that loses every coarse step to a cap ships the operator's base step
    // with a feed sized at a depth that it does not cut.
    let had_ladder = super::ladder::has_ladder(operation);
    let entry_dpp = if had_ladder {
        context
            .calculator_operating_point
            .map(|c| c.axial_depth_mm)
            .or_else(|| operation.depth_per_pass())
    } else {
        operation.depth_per_pass()
    };
    let mut working_context = context;
    let (axial_envelope_warnings, dpp_mutated) =
        pick_axial_envelope(operation, tool, material, working_context);
    let mut warnings = Vec::new();
    warnings.extend(axial_envelope_warnings);
    // The step ladder (D7): the band is derated at the DEEPEST step, because
    // one feed serves every level and the deepest level has the smallest
    // band. With no ladder the deepest step is `depth_per_pass`. With a
    // ladder the calculator never saw the coarse steps or the operator's
    // base step, so the band is re-derived even when pass 0 moved no step,
    // and also when pass 0 removed the last coarse step.
    if (dpp_mutated || had_ladder)
        && let Some(new_dpp) = operation.deepest_axial_step()
    {
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
    // Pass 6b (ruling R4, 2026-09-24): the machine aggressiveness dial. It
    // reads the base engagement that passes 0 to 6 left, and scales the depth
    // per pass and the stepover by one common factor to hold the load at the
    // dial's fraction. Pass 9 and pass 10 below must see its result.
    let dial_warnings = super::aggressiveness::apply_aggressiveness(
        operation, tool, material, machine, pass_role, context, &warnings,
    );
    warnings.extend(dial_warnings);
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
    // Pass 9 runs second to last, and must: every pass above may still move
    // the stepover or the DPP, and this one exists to reconcile the feed with
    // wherever they finally land.
    let rescale_warnings = rescale_feed_to_final_geometry(
        operation,
        tool,
        machine,
        entry_stepover,
        entry_dpp,
        context,
    );
    // Pass 10 (T-15) re-checks the spindle power ceiling at the geometry the
    // operation ships, and it runs ONLY when pass 9 moved the feed.
    //
    // Pass 9 re-multiplies the feed by the depth-tier factor at the final
    // depth, and that factor RISES as the depth falls, so a clamp that crosses
    // a tier boundary downward makes pass 9 RAISE the feed. Calculator Step 6
    // checked power at the calculator's geometry and nothing re-checked it
    // here.
    //
    // The gate is the warning pass 9 files when it acts, which also carries
    // the pre-rescale feed pass 10 falls back to. When pass 9 does nothing the
    // feed is byte-identical and there is nothing new to check — the contract
    // `suggest_feed_matches_final_geometry` pins.
    let pre_rescale_feed = rescale_warnings.iter().find_map(|w| match w {
        SuggestWarning::FeedRescaledToFinalGeometry {
            requested_mm_per_min,
            ..
        } => Some(*requested_mm_per_min),
        _ => None,
    });
    warnings.extend(rescale_warnings);
    if let Some(pre_rescale_feed) = pre_rescale_feed {
        warnings.extend(recheck_power_after_rescale(
            operation,
            tool,
            machine,
            material,
            pre_rescale_feed,
            context,
        ));
    }
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
    // S3 (2026-09-18): the factor selection moved to
    // `RigidityProfile::depth_cap_mm`, the ONE producer of this cap. The
    // post-simulation depth criterion reads the same helper, so the cap
    // the recipe was lowered to and the cap the criterion row draws
    // against cannot disagree. The branch is unchanged: adaptive family
    // → `adaptive_doc_factor`, otherwise (this pass is roughing-only) →
    // `doc_roughing_factor`. The helper's `None` arm is the drill
    // family and (feeds matrix R2, 2026-09-23) every finishing and
    // semi-finishing pass, which gets no axial ceiling.
    //
    // R2: the diameter is `geometry::depth_cap_diameter_mm` at the depth
    // under test, the function the gate calls at the measured peak. On a
    // tapered ball that is the engaged cone diameter, not the tip.
    // The step ladder (D7): the cap applies to the deepest step. Each step
    // above the cap moves down to it; a step below the cap does not move.
    let Some(current) = operation.deepest_axial_step() else {
        return warnings;
    };
    if !matches!(pass_role, PassRole::Roughing) || !current.is_finite() {
        return warnings;
    }
    let family = operation.op_type().spec().feeds_family;
    let cutter = crate::compute::cutter::build_cutter(tool);
    let cap_at = |depth: f64| -> Option<f64> {
        let diameter = crate::feeds::geometry::depth_cap_diameter_mm(&cutter, depth);
        machine
            .rigidity
            .depth_cap_mm(family, pass_role, diameter)
            .map(|c| c.cap_mm())
            .filter(|cap| cap.is_finite() && *cap > 0.0)
    };
    let Some(cap_at_current) = cap_at(current) else {
        return warnings;
    };
    if current > cap_at_current {
        let capped = deepest_depth_within_cap(cap_at_current, &cap_at);
        let removed = super::ladder::cap_axial_steps(operation, capped);
        warnings.push(SuggestWarning::RoughingDepthClampedToRigidity {
            requested: current,
            capped,
        });
        // A capped coarse step that is no longer above the next step goes,
        // with its note (operator ruling 1, 2026-09-24).
        warnings.extend(super::ladder::removal_notes(&removed, "rigidity cap"));
    }
    warnings
}

/// The deepest depth at or below `first_cap` that is inside the cap
/// measured at that same depth. Feeds matrix R2 (2026-09-23).
///
/// When the cap diameter does not change with depth (flat, ball, bull,
/// V-bit), the answer is `first_cap` itself, returned unchanged. On a
/// tapered ball the cap grows with depth, so the cap at `first_cap` is
/// smaller than `first_cap`. A bisection then finds the depth `d` where
/// `d <= cap_at(d)`. The result is the low end of the bracket, so it is
/// always inside its own cap, and the gate reads it as `Within`.
fn deepest_depth_within_cap(first_cap: f64, cap_at: &dyn Fn(f64) -> Option<f64>) -> f64 {
    const BISECTION_STEPS: usize = 64;
    let inside = |d: f64| cap_at(d).is_some_and(|cap| d <= cap);
    if inside(first_cap) {
        return first_cap;
    }
    let mut lo = 0.0_f64;
    let mut hi = first_cap;
    for _ in 0..BISECTION_STEPS {
        let mid = 0.5 * (lo + hi);
        if inside(mid) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    lo
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
    // The step ladder (D7): a coarse step deeper than the flutes is the
    // deepest step, and it moves down to the flute length with the base step.
    if let Some(current) = operation.deepest_axial_step() {
        let cap = tool.cutting_length;
        if current.is_finite() && cap.is_finite() && cap > 0.0 && current > cap {
            let removed = super::ladder::cap_axial_steps(operation, cap);
            warnings.push(SuggestWarning::DepthClampedToCuttingLength {
                requested: current,
                capped: cap,
            });
            // Operator ruling 1 (2026-09-24): a removed step has a note.
            warnings.extend(super::ladder::removal_notes(&removed, "flute length"));
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
/// The predictor abstains on a drill, an unvalidated or custom material,
/// a zero feed and five other input-shaped cases. Since T-4 that
/// abstention is an `Err` that names itself, and this pass reports it
/// through [`SuggestWarning::DeflectionBackoffUnmodeled`] instead of
/// passing through in silence. A V-bit no longer abstains: its figure is
/// a modelled floor, the back-off runs on it, and
/// [`SuggestWarning::DeflectionBackoffFigureIsAFloor`] says so.
fn backoff_dpp_for_deflection(
    operation: &mut OperationConfig,
    tool: &ToolConfig,
    material: &Material,
    machine: &MachineProfile,
    pass_role: PassRole,
) -> Vec<SuggestWarning> {
    let mut warnings = Vec::new();
    // The step ladder (D7): the predictor reads the deepest step, so the
    // back-off lowers the deepest step. Each step lowers to the new depth
    // only when it is above it (`cap_axial_steps`), so a base step below the
    // deflection limit keeps its value. With no ladder this is the old
    // write to `depth_per_pass`.
    if let Some(current) = operation.deepest_axial_step()
        && matches!(pass_role, PassRole::Roughing)
        && current.is_finite()
        && current > DEFLECTION_BACKOFF_DPP_FLOOR_MM
    {
        let pre_backoff_dpp = current;
        let initial_prediction = match crate::feeds::predict::predict_peak_deflection_um(
            operation, tool, material, machine,
        ) {
            Ok(prediction) => prediction,
            Err(reason) => {
                // T-4: the back-off ACTS on the refusal. It changes no
                // DPP — there is no figure to back off from — but it
                // states the abstention on the channel Suggest already
                // uses for a step it did not take.
                tracing::debug!(
                    dpp_mm = current,
                    reason = ?reason,
                    "Suggest deflection back-off did not run — the predictor abstained"
                );
                warnings.push(SuggestWarning::DeflectionBackoffUnmodeled {
                    dpp_mm: current,
                    reason,
                });
                return warnings;
            }
        };
        // A caveated figure is a floor. The back-off runs on it, because
        // backing a DPP off a floor is conservative, and the operator
        // gets the clause as well, because a floor does not show the cut
        // inside the bound. The cutter shape does not change inside the
        // loop, so one report covers every iteration.
        if let Some(caveat) = initial_prediction.caveat {
            warnings.push(SuggestWarning::DeflectionBackoffFigureIsAFloor {
                dpp_mm: current,
                predicted_um: initial_prediction.predicted_um,
                caveat,
            });
        }
        let predicted_initial_um = initial_prediction.predicted_um;
        let mut predicted_um = predicted_initial_um;
        let mut iterations: u8 = 0;
        // The last DPP the model evaluated. The operation carries this
        // value whenever the loop is not mid-step.
        let mut dpp = current;
        // The step ladder: the loop can remove a coarse step. The note names
        // the step as it was before this pass, so `origin` keeps that value
        // for each step that is still on the ladder (operator ruling 1,
        // 2026-09-24).
        let mut origin = super::ladder::coarse_steps(operation).to_vec();
        let mut removed_steps = Vec::new();

        while predicted_um > DEFLECTION_BACKOFF_TARGET_UM
            && iterations < DEFLECTION_BACKOFF_MAX_ITERATIONS
            && dpp > DEFLECTION_BACKOFF_DPP_FLOOR_MM
        {
            let next_dpp = (dpp * DEFLECTION_BACKOFF_FACTOR).max(DEFLECTION_BACKOFF_DPP_FLOOR_MM);
            let before_step = operation.clone();
            let removed = super::ladder::cap_axial_steps(operation, next_dpp);
            match crate::feeds::predict::predict_peak_deflection_um(
                operation, tool, material, machine,
            ) {
                Ok(next) => {
                    dpp = next_dpp;
                    predicted_um = next.predicted_um;
                    iterations = iterations.saturating_add(1);
                    // Highest index first, so each `remove` leaves the
                    // lower indices of `origin` in place.
                    for step in removed.iter().rev() {
                        let mut step = *step;
                        if step.index < origin.len() {
                            step.step_mm = origin.remove(step.index);
                        }
                        removed_steps.push(step);
                    }
                }
                Err(reason) => {
                    // Not reachable on today's model: the initial call
                    // modelled this (operation, tool, material) and the
                    // loop only lowers the DPP, which can only un-refuse
                    // `DegenerateCantilever`. Handled rather than
                    // assumed. Step back to the last DPP the model
                    // evaluated, so the warning below cannot quote a
                    // deflection at a depth the operation does not run.
                    // The step back also puts back any coarse step that
                    // this step removed, so it gets no note.
                    *operation = before_step;
                    warnings.push(SuggestWarning::DeflectionBackoffUnmodeled {
                        dpp_mm: dpp,
                        reason,
                    });
                    break;
                }
            }
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
        // Operator ruling 1 (2026-09-24): a removed step has a note,
        // coarsest first.
        removed_steps.sort_by(|a, b| b.step_mm.total_cmp(&a.step_mm));
        warnings.extend(super::ladder::removal_notes(
            &removed_steps,
            "deflection back-off",
        ));
    }
    warnings
}
