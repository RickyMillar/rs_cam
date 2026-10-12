//! The candidate record, the parameter diff, and the per-candidate
//! evaluation driver.
//!
//! - [`OptimizeCandidate`] — the record of one evaluation; the
//!   orchestrator gives the list to `rank_candidates`.
//! - [`ParamDelta`] and [`delta_against_baseline`] — which parameters a
//!   candidate moves away from the baseline.
//! - [`evaluate_candidate`] — the one per-candidate driver: apply,
//!   regenerate, simulate, gate, read the full cut, and record.
//! - [`candidate_sim_options`] — the one construction site for the
//!   candidate simulation options.

use std::sync::atomic::AtomicBool;

use serde::{Deserialize, Serialize};

use crate::compute::catalog::OperationConfig;
use crate::compute::config::DressupConfig;
use crate::session::{ProjectSession, SessionError, SimulationOptions};
use crate::tool_load::verdict::ToolpathLoadVerdict;
use crate::tool_load::{ToleranceBands, ToolpathLoadContext, evaluate_toolpath};
use crate::util::panic_message::panic_payload_message;

use super::SearchStage;
use super::context::{
    BaselineRestoreGuard, EvaluationContext, air_cut_fraction_of_total_runtime_from_trace,
    cycle_time_from_trace,
};
use super::full_cut::{FullCutCheck, read_full_cut, realised_cell_mm};

/// One candidate's full evaluation record. A simulation of the
/// candidate measures each field. For the baseline candidate at index 0
/// the fields come from `baseline_trace`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizeCandidate {
    /// The full operation config that would be applied. Carries
    /// unchanged fields too, so Apply just needs to write this into
    /// `session.toolpath_configs[idx].operation`.
    pub params: OperationConfig,
    /// The dressups this candidate ran with. The link lever changes
    /// them, so Apply must write them beside [`Self::params`].
    #[serde(default)]
    pub dressups: Box<DressupConfig>,
    /// Diff from the baseline params, for display at Apply time.
    pub delta: ParamDelta,
    /// Measured cycle time from this candidate's sim (seconds).
    pub cycle_time_s: f64,
    /// Verdict from the gate over this candidate's sim.
    pub verdict: ToolpathLoadVerdict,
    /// The full-cut readings of this candidate and their verdict against
    /// the baseline (PLAN.md §3). The baseline carries its own readings
    /// with the verdict `Holds`.
    #[serde(default)]
    pub full_cut: FullCutCheck,
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
    /// The new state of the link-moves dressup. `Some(true)` when the
    /// candidate turns the links on. `None` if the candidate keeps the
    /// baseline state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link_moves: Option<bool>,
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
            || self.link_moves.is_some()
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

/// Compute a [`ParamDelta`] from the baseline and the candidate. A feed
/// change smaller than [`FEED_DELTA_DISPLAY_TOLERANCE_MM_MIN`] is not
/// shown.
pub(crate) fn delta_against_baseline(
    baseline: &OperationConfig,
    baseline_dressups: &DressupConfig,
    candidate: &OperationConfig,
    candidate_dressups: &DressupConfig,
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
    let links_before = baseline_dressups.link_moves.is_some();
    let links_after = candidate_dressups.link_moves.is_some();
    if links_before != links_after {
        delta.link_moves = Some(links_after);
    }
    delta
}

/// The **one** construction site for the simulation options of an
/// optimizer candidate.
///
/// The options are the ones the GUI's simulation submit builds
/// (operator ruling 2026-10-12, PLAN.md §2 step 4, "one measure"):
///
/// - `resolution`: [`ProjectSession::simulation_resolution_mm`], the one
///   stored project resolution (G-RESTRES). The GUI submit reads the same
///   value (`rs_cam_viz/src/controller/events/simulation.rs`,
///   `submit_simulation_for_groups`).
/// - `auto_resolution: false`: the stored value already holds the `Auto`
///   rule.
/// - `adaptive_feed_modulation: true`, `ModulationStrategy::ConstrainedMax`,
///   `modulation_feed_scale: 1.0`: the GUI post-pass
///   (`rs_cam_viz/src/controller/events/compute.rs`, F-039).
/// - `use_predicted_feed_in_gates: false`: the GUI `KinematicsContext`
///   value.
/// - no `skip_ids`.
///
/// The baseline trace on screen and every candidate are therefore one
/// measure. `one_measure_for_baseline_and_candidate_g_opt6` is the proof.
#[must_use]
pub fn candidate_sim_options(session: &ProjectSession) -> SimulationOptions {
    SimulationOptions {
        resolution: session.simulation_resolution_mm(),
        skip_ids: Vec::new(),
        auto_resolution: false,
        use_predicted_feed_in_gates: false,
        adaptive_feed_modulation: true,
        modulation_strategy: crate::dressup::feed_modulation::ModulationStrategy::ConstrainedMax,
        modulation_feed_scale: 1.0,
    }
}

// ── Candidate evaluation: apply → regen → sim → gate → full cut ───────
//
// The one per-candidate driver. It applies the candidate's parameters
// through the restore guard, regenerates the toolpath, runs a project
// simulation with the one set of options, and records the measured
// cycle time, the gate verdict and the full-cut check. Every candidate
// goes through this function; there is no second simulation and no
// second gate.

/// Evaluate one candidate end-to-end:
///   1. Apply the candidate's operation and dressups through the guard.
///   2. Regenerate the toolpath.
///   3. Run a project sim with [`candidate_sim_options`].
///   4. Score via the gate (`tool_load::evaluate_toolpath`).
///   5. Read the cycle time and the full-cut readings, and judge them
///      against `baseline_full_cut`.
///
/// Errors propagate as `SessionError`. The orchestrator records the
/// cell and goes on with the next one. Cancellation is honoured by the
/// generator and the simulator and surfaces here as an `Err`.
///
/// **Panic isolation (R1, tech-debt review 2026-06-10).** A candidate
/// can regenerate a toolpath at parameters that no operator tried. A
/// panic in the geometry stack must cost one candidate, not the whole
/// run. The inner evaluation therefore runs under `catch_unwind`, and a
/// panic becomes `SessionError::OperationFailed`. The next candidate's
/// apply (or the guard's `Drop`) writes over the parameters the
/// panicking candidate applied, and `generate_toolpath` /
/// `run_simulation` publish results only on success.
pub(crate) fn evaluate_candidate(
    guard: &mut BaselineRestoreGuard<'_>,
    ctx: &EvaluationContext,
    candidate_op: OperationConfig,
    candidate_dressups: DressupConfig,
    baseline_full_cut: &FullCutCheck,
    cancel: &AtomicBool,
) -> Result<OptimizeCandidate, SessionError> {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        evaluate_candidate_inner(
            guard,
            ctx,
            candidate_op,
            candidate_dressups,
            baseline_full_cut,
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
    candidate_dressups: DressupConfig,
    baseline_full_cut: &FullCutCheck,
    cancel: &AtomicBool,
) -> Result<OptimizeCandidate, SessionError> {
    let baseline = guard.baseline();
    let delta = delta_against_baseline(
        &baseline.operation,
        &baseline.dressups,
        &candidate_op,
        &candidate_dressups,
    );
    let face_selection = baseline.face_selection.clone();
    let toolpath_index = ctx.toolpath_index;

    // Apply.
    // The NARROW path, deliberately. A wide invalidation would drop
    // every downstream `FromRemainingStock` result once per candidate,
    // and the simulation below depends on those results standing.
    let _ = guard.session_mut().apply_toolpath_param_snapshot_narrow(
        toolpath_index,
        candidate_op.clone(),
        candidate_dressups.clone(),
        face_selection,
    )?;

    // Regen — writes session.results[idx], leaves other entries intact.
    guard
        .session_mut()
        .generate_toolpath(toolpath_index, cancel)?;

    // Sim — a full project sim with the one set of options. Other
    // toolpaths' cached results still apply because
    // `generate_toolpath` only touched index `toolpath_index`.
    let sim_opts = candidate_sim_options(guard.session_mut());
    let total_depth_mm = candidate_op.total_depth();
    guard.session_mut().run_simulation(&sim_opts, cancel)?;

    let session_ref: &ProjectSession = &*guard.session_mut();
    let sim_result = session_ref
        .simulation_result()
        .ok_or(SessionError::Simulation(
            crate::compute::simulate::SimulationError::Cancelled,
        ))?;
    let Some(trace) = sim_result.cut_trace.as_deref() else {
        return Err(SessionError::SimulationRequired(
            "the candidate simulation kept no cut trace".to_owned(),
        ));
    };

    // Span lookup for D6/D7: the toolpath was just regenerated, so the
    // cached `annotated.spans` are fresh. F2.2: `None` when a transform
    // invalidated the spans (TSP split).
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
        Some(trace),
        Some(session_ref.machine()),
        &ToleranceBands::default(),
    );
    let cycle_time_s = cycle_time_from_trace(trace, ctx.toolpath_id).unwrap_or(f64::INFINITY);
    // Roadmap F.12 — surface air-cut alongside cycle_time, denominator named.
    let air_cut_fraction_of_total_runtime =
        air_cut_fraction_of_total_runtime_from_trace(trace, ctx.toolpath_id);
    let readings = read_full_cut(session_ref, trace, ctx.toolpath_id);
    // R4: the REALISED cell of this simulation
    // (`SimulationResult::column_grid_cell_mm`), not the requested
    // `sim_opts.resolution`; `effective_cell_size` can enlarge it. The
    // reading's bar and the triage finding read the same field, so the
    // finding and its reader use one tolerance.
    let full_cut = FullCutCheck::judge(
        baseline_full_cut,
        readings,
        realised_cell_mm(session_ref),
        total_depth_mm,
    );

    Ok(OptimizeCandidate {
        params: candidate_op,
        dressups: Box::new(candidate_dressups),
        delta,
        cycle_time_s,
        verdict,
        full_cut,
        stage: SearchStage::Measured,
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
    use crate::compute::config::DressupConfig;
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
        let dressups = DressupConfig {
            link_moves: None,
            ..DressupConfig::default()
        };
        assert!(!delta_against_baseline(&baseline, &dressups, &same_feed, &dressups).has_changes());

        let delta = delta_against_baseline(&baseline, &dressups, &deeper, &dressups);
        assert_eq!(delta.depth_per_pass_mm, Some(deeper_dpp));
        assert_eq!(delta.feed_mm_min, None);
        assert_eq!(delta.link_moves, None);

        let linked = DressupConfig::default();
        assert!(linked.link_moves.is_some());
        let delta = delta_against_baseline(&baseline, &dressups, &baseline, &linked);
        assert_eq!(delta.link_moves, Some(true));
        assert!(delta.has_changes());
    }
}
