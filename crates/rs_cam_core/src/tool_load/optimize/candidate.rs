//! The candidate record, the parameter diff, and the per-candidate
//! evaluation driver.
//!
//! - [`OptimizeCandidate`] — the record of one evaluation; the
//!   orchestrator gives the list to `build_outcome`.
//! - [`ParamDelta`] and [`delta_against_baseline`] — which parameters a
//!   candidate moves away from the baseline.
//! - [`evaluate_candidate`] — the one per-candidate driver: apply,
//!   regenerate, simulate, gate, and record.
//! - [`candidate_sim_options`] — the one construction site for the
//!   candidate simulation options.

use std::sync::atomic::AtomicBool;

use serde::{Deserialize, Serialize};

use crate::compute::catalog::OperationConfig;
use crate::session::{ProjectSession, SessionError, SimulationOptions};
use crate::tool_load::verdict::ToolpathLoadVerdict;
use crate::tool_load::{ToleranceBands, ToolpathLoadContext, evaluate_toolpath};
use crate::util::panic_message::panic_payload_message;

use super::SearchStage;
use super::context::{
    BaselineRestoreGuard, EvaluationContext, air_cut_fraction_of_total_runtime_from_trace,
    cycle_time_from_trace,
};

/// One candidate's full evaluation record. A simulation of the
/// candidate measures each field. For the baseline candidate at index 0
/// the fields come from `baseline_trace`.
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
    /// Which source produced this candidate.
    pub stage: SearchStage,
    /// Project-level reconciliation result (U4). After the user Applies
    /// and the project re-sims end-to-end, downstream stock-state
    /// changes can shift this candidate's verdict — that shifted value
    /// lands here. `None` until U4 fires.
    pub reconciled_cycle_time_s: Option<f64>,
    /// Reconciled verdict from the post-Apply project sim (U4).
    pub reconciled_verdict: Option<ToolpathLoadVerdict>,
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

/// The diff between a candidate and the baseline, for the GUI row text.
/// Each field is `Some(new_value)` only when the candidate changes that
/// parameter. The modal renders "feed 1899→2100" from it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ParamDelta {
    /// New feed in mm/min. `None` if the candidate keeps the baseline feed.
    pub feed_mm_min: Option<f64>,
    /// New spindle RPM. `None` if the candidate keeps the baseline RPM.
    pub spindle_rpm: Option<u32>,
    /// New radial stepover in mm.
    pub stepover_mm: Option<f64>,
    /// New depth per pass in mm.
    pub depth_per_pass_mm: Option<f64>,
    /// New scallop ridge height in mm. This is a different parameter
    /// from `stepover_mm`: Scallop derives its stepover from
    /// `(scallop_height, ball_radius)`. G2 (2026-05-08).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scallop_height_mm: Option<f64>,
}

impl ParamDelta {
    /// True if any field is `Some(_)`, i.e. the candidate changes a
    /// parameter.
    pub fn has_changes(&self) -> bool {
        self.feed_mm_min.is_some()
            || self.spindle_rpm.is_some()
            || self.stepover_mm.is_some()
            || self.depth_per_pass_mm.is_some()
            || self.scallop_height_mm.is_some()
    }
}

/// The smallest feed change, in mm/min, that [`delta_against_baseline`]
/// shows as a parameter change.
///
/// A display precision, not a cutting limit. The value is the one the
/// deleted search policy printed
/// (`git show 009bee30:crates/rs_cam_core/src/tool_load/optimize/policy.rs`,
/// `delta_display_tolerance_mm_min`, source "GUI/display precision").
const FEED_DELTA_DISPLAY_TOLERANCE_MM_MIN: f64 = 0.5;

/// Compute a [`ParamDelta`] from two `OperationConfig`s. A feed change
/// smaller than [`FEED_DELTA_DISPLAY_TOLERANCE_MM_MIN`] is not shown.
// SAFETY: Phase 2 (planning/optimize_review_2026-10-12/PLAN.md §2) calls
// this for each candidate; Phase 1 evaluates no candidate, so no
// production caller exists yet.
#[allow(dead_code)]
pub(crate) fn delta_against_baseline(
    baseline: &OperationConfig,
    candidate: &OperationConfig,
) -> ParamDelta {
    let mut delta = ParamDelta::default();
    if (baseline.feed_rate() - candidate.feed_rate()).abs() > FEED_DELTA_DISPLAY_TOLERANCE_MM_MIN {
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
    if baseline.scallop_height() != candidate.scallop_height()
        && let Some(s) = candidate.scallop_height()
    {
        delta.scallop_height_mm = Some(s);
    }
    delta
}

/// The **one** construction site for the optimizer's candidate-scoring
/// [`SimulationOptions`].
///
/// A-8 (2026-08-13) hoisted this out of `evaluate_candidate_inner` so
/// that [`super::outcome::CandidateSimAssumptions`] — the stamp the
/// optimizer publishes on every outcome — reads the *same* value the sim
/// ran, rather than a second literal that can drift from it. §0 rule 5:
/// a stamp built from a copy of the options is a docstring waiting to
/// become a lie. Nothing here changed behaviour; the literal moved.
///
/// # The two pinned flags, and the state of their justifications
///
/// * `use_predicted_feed_in_gates: false` (F-035) — the optimizer ranks
///   candidates against commanded-feed gates. This **still agrees** with
///   `SimulationOptions::default()` (`session/mod.rs`), so the original
///   "same as the production simulator" justification holds. Consequence
///   worth stating: with this off, the machine's kinematics model cannot
///   reach any candidate verdict, which is why the stamp records the
///   kinematics *source* rather than claiming a kinematics-aware score.
///
/// * `adaptive_feed_modulation: false` (F-036b) — the optimizer scores
///   the commanded feed in the IR, deliberately keeping the optimizer
///   and the feed modulator from conflating two corrections over the
///   same engagement summary. **Its stated premise is now stale.**
///   Checkpoint J (2026-08-13) flipped the library default to `true` and
///   Checkpoint K (g1) flipped the CLI flag to match, so this pin no
///   longer tracks "what the production simulator does" — it is now the
///   only shipped non-fixture site that pins `false` (`cli smoke` is the
///   other, and that is a fingerprint harness by design). An optimizer
///   verdict is therefore taken at a **different operating point** than
///   the GUI/CLI verdict for the same project.
///
///   A-8 did **not** flip it: that is number-moving on every optimizer
///   outcome and belongs to an operator ruling, not to the wave that
///   discovered it. What A-8 shipped instead is the disclosure — the
///   pin is now *visible* on the outcome instead of being an invisible
///   literal three call levels down.
///
/// # Baseline asymmetry (not fixed here)
///
/// Candidate 0 — the baseline — is **not** simulated by this function.
/// It is scored against the caller's `baseline_trace`, whose options are
/// whatever the caller ran. See [`super::outcome::BaselineTraceAssumptions`].
pub(crate) fn candidate_sim_options(sim_resolution_mm: f64) -> SimulationOptions {
    SimulationOptions {
        resolution: sim_resolution_mm,
        skip_ids: Vec::new(),
        auto_resolution: false,
        use_predicted_feed_in_gates: false,
        adaptive_feed_modulation: false,
        modulation_strategy: crate::dressup::feed_modulation::ModulationStrategy::ConstrainedMax,
        modulation_feed_scale: 1.0,
    }
}

// ── Candidate evaluation: apply → regen → sim → gate ──────────────────
//
// The one per-candidate driver. It applies the candidate's parameters
// through the restore guard, regenerates the toolpath, runs a project
// simulation at the requested resolution, and records the measured
// cycle time and gate verdict. Every candidate goes through this
// function; there is no second simulation and no second gate.

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
/// **Panic isolation (R1, tech-debt review 2026-06-10).** A candidate
/// can regenerate a toolpath at parameters that no operator tried. A
/// panic in the geometry stack (for example `cavalier_contours`'
/// `parallel_offset` asserts on a degenerate input) must cost one
/// candidate, not the whole run. The inner evaluation therefore runs
/// under `catch_unwind`, and a panic becomes
/// `SessionError::OperationFailed`. State safety: the next candidate's
/// apply (or the guard's `Drop`) writes over the parameters the
/// panicking candidate applied, and `generate_toolpath` /
/// `run_simulation` publish results only on success.
// SAFETY: Phase 2 (planning/optimize_review_2026-10-12/PLAN.md §2) calls
// this for each lever cell; Phase 1 evaluates no candidate, so no
// production caller exists yet.
#[allow(dead_code)]
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
    // The NARROW path, deliberately. A wide invalidation would drop
    // every downstream `FromRemainingStock` result once per candidate,
    // and the simulation below depends on those results standing.
    let _ = guard.session_mut().apply_toolpath_param_snapshot_narrow(
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
    let sim_opts = candidate_sim_options(sim_resolution_mm);
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
    let spans: Option<&[crate::trace::toolpath_spans::Span]> = session_ref
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
    // The strict bands of the tool-load card: the optimizer judges a
    // candidate by the same measure as the card.
    let verdict = evaluate_toolpath(
        &load_ctx,
        trace,
        Some(session_ref.machine()),
        &ToleranceBands::default(),
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
        air_cut_fraction_of_total_runtime,
    })
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use crate::util::panic_message::panic_payload_message;

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

    // ── ParamDelta and delta_against_baseline ────────────────────────

    use super::{ParamDelta, delta_against_baseline};
    use crate::compute::catalog::OperationConfig;
    use crate::compute::operation_configs::PocketConfig;

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

    /// A feed change below the display tolerance is not a change; a
    /// depth change is.
    #[test]
    fn delta_against_baseline_names_only_the_moved_parameters() {
        let base = PocketConfig::default();
        let deeper_dpp = base.depth_per_pass + 1.0;
        let same_feed = OperationConfig::Pocket(PocketConfig {
            feed_rate: base.feed_rate + 0.1,
            ..PocketConfig::default()
        });
        let deeper = OperationConfig::Pocket(PocketConfig {
            depth_per_pass: deeper_dpp,
            ..PocketConfig::default()
        });
        let baseline = OperationConfig::Pocket(base);
        assert!(!delta_against_baseline(&baseline, &same_feed).has_changes());

        let delta = delta_against_baseline(&baseline, &deeper);
        assert_eq!(delta.depth_per_pass_mm, Some(deeper_dpp));
        assert_eq!(delta.feed_mm_min, None);
    }
}
