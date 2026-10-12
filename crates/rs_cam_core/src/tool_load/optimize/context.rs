//! Per-toolpath evaluation context, baseline-restore RAII, and the
//! trace readers each candidate evaluation uses.
//!
//! - [`EvaluationContext`] — resources (tool, material, op kind, LUT
//!   routing) computed once per `optimize_toolpath` call and shared
//!   across every candidate.
//! - [`ToolpathParamsSnapshot`] / [`BaselineRestoreGuard`] — RAII guard
//!   that snapshots `(operation, dressups, face_selection)` at
//!   construction and re-applies them on drop. Restoration runs on
//!   every exit path (Ok, refusal, cancel, panic).
//! - Trace readers: [`cycle_time_from_trace`],
//!   [`air_cut_fraction_of_total_runtime_from_trace`].
//! - LUT routing: [`lut_op_family_from`], [`lut_pass_role_from`].

use crate::compute::catalog::{OperationConfig, OperationType};
use crate::compute::config::DressupConfig;
use crate::feeds::vendor_lut::{LutOperationFamily, LutPassRole};
use crate::feeds::{OperationFamily, PassRole};
use crate::geometry::enriched_mesh::FaceGroupId;
use crate::ids::ToolpathId;
use crate::session::{ProjectSession, SessionError};
use crate::stock::simulation_cut::SimulationCutTrace;

/// Look up this toolpath's per-toolpath cycle time from the trace's
/// per-toolpath summary. Returns `None` if the toolpath isn't
/// represented in the trace (no samples for that id — happens when
/// the toolpath was disabled or skipped).
pub(crate) fn cycle_time_from_trace(
    trace: &SimulationCutTrace,
    toolpath_id: ToolpathId,
) -> Option<f64> {
    trace
        .toolpath_summaries
        .iter()
        .find(|s| s.toolpath_id == toolpath_id)
        .map(|s| s.total_runtime_s)
}

/// Roadmap F.12 — per-toolpath air-cut fraction of **total runtime**
/// (cutting + rapids), range 0.0..=1.0. Returns `None` when the toolpath has
/// no summary entry (failed sim) or `total_runtime_s` is non-positive.
///
/// LH-1: the denominator is in the name because the other one
/// ([`crate::stock::simulation_cut::AirCutRatios::air_cut_pct_of_cutting_time`])
/// also ships, reads larger, and used to travel under the same word.
pub(crate) fn air_cut_fraction_of_total_runtime_from_trace(
    trace: &SimulationCutTrace,
    toolpath_id: ToolpathId,
) -> Option<f64> {
    use crate::stock::simulation_cut::AirCutRatios;
    trace
        .toolpath_summaries
        .iter()
        .find(|s| s.toolpath_id == toolpath_id)
        .and_then(|s| {
            if s.total_runtime_s > 0.0 {
                Some(s.air_cut_pct_of_total_runtime() / 100.0)
            } else {
                None
            }
        })
}

/// Map the operation's `feeds_family` (used by the F&S calculator) to
/// the `LutOperationFamily` (used by the chipload/power gate's LUT
/// lookup).
///
/// This held a second copy of the eight-arm match. The canonical one is
/// [`crate::feeds::vendor_normalize::op_family_to_lut`], which the LUT
/// loader's reachability census and the gate's query builder already read,
/// so optimize and suggest now share the mapping instead of keeping two in
/// step by hand. `lut_op_family_mapping_covers_all_variants` stays as the
/// tripwire over the arms.
pub(crate) fn lut_op_family_from(family: OperationFamily) -> LutOperationFamily {
    crate::feeds::vendor_normalize::op_family_to_lut(family)
}

/// Map the operation's `feeds_pass_role` to the LUT's pass-role enum.
pub(crate) fn lut_pass_role_from(role: PassRole) -> LutPassRole {
    match role {
        PassRole::Roughing => LutPassRole::Roughing,
        PassRole::SemiFinish => LutPassRole::SemiFinish,
        PassRole::Finish => LutPassRole::Finish,
    }
}

// ── Baseline-restore guard (Engineering Default 10) ───────────────────
//
// Each candidate evaluation in the optimizer mutates
// `session.toolpath_configs[idx].operation` (via
// `apply_toolpath_param_snapshot_narrow`), regenerates, and runs a fresh
// sim.
// Without explicit cleanup, the session is left holding the last
// candidate's params when the optimizer returns — a silent state leak.
//
// `BaselineRestoreGuard` snapshots `(operation, dressups,
// face_selection)` at construction and re-applies them in `Drop`,
// regardless of how the optimizer exits (Ok, refusal, cancel, or
// panic in `execute_operation`). Apply remains a separate
// user-initiated mutation; the optimizer's internal candidate writes
// never persist past `optimize_toolpath` returning.

/// Snapshot of the toolpath fields the optimizer mutates per
/// candidate. Captured up-front and re-applied via
/// `apply_toolpath_param_snapshot_narrow` on drop.
#[derive(Debug, Clone)]
pub(crate) struct ToolpathParamsSnapshot {
    pub operation: OperationConfig,
    pub dressups: DressupConfig,
    pub face_selection: Option<Vec<FaceGroupId>>,
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
/// `apply_toolpath_param_snapshot_narrow` and ignores its `Result` —
/// restoration failure can only happen if the toolpath was removed
/// mid-search, which would already be a programming error.
///
/// **This restore must never go wide.** A wide restore drops exactly
/// the neighbour results the search preserved, at the moment the caller
/// wants them back, and a `Drop` cannot report the loss.
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
        let _ = self.session.apply_toolpath_param_snapshot_narrow(
            self.toolpath_index,
            self.snapshot.operation.clone(),
            self.snapshot.dressups.clone(),
            self.snapshot.face_selection.clone(),
        );
    }
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
    pub toolpath_id: ToolpathId,
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
    /// Where the search publishes its rung and its candidate count
    /// (WP29).
    ///
    /// The field rides the CONTEXT rather than a parameter, because the
    /// candidate driver already takes `&EvaluationContext`.
    ///
    /// [`Self::from_session`] supplies a silent default, so a caller that
    /// observes nothing writes nothing and changes nothing.
    pub progress: std::sync::Arc<super::progress::OptimizeProgress>,
}

impl EvaluationContext {
    /// Build the evaluation context from the session for the given
    /// toolpath index. Returns `None` if the toolpath or its tool is
    /// missing — caller should `Skipped` in that case.
    ///
    /// The progress sink starts SILENT. Use [`Self::with_progress`] to
    /// attach the one the caller wants to read.
    pub(crate) fn from_session(session: &ProjectSession, toolpath_index: usize) -> Option<Self> {
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
            progress: std::sync::Arc::new(super::progress::OptimizeProgress::default()),
        })
    }

    /// Attach the progress sink the caller reads (WP29).
    pub(crate) fn with_progress(
        mut self,
        progress: std::sync::Arc<super::progress::OptimizeProgress>,
    ) -> Self {
        self.progress = progress;
        self
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
            id: ToolpathId(0),
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
            debug_options: crate::trace::debug_trace::ToolpathDebugOptions::default(),
            feeds_provenance: crate::feeds::FeedsProvenance::default(),
            rest_analysis: crate::compute::config::RestAnalysisConfig::default(),
            planner_origin: None,
        }
    }

    fn session_with_one_pocket() -> ProjectSession {
        let mut s = ProjectSession::new_empty();
        let _ = s.add_tool(make_tool());
        let _ = s.add_toolpath(0, make_tc(s.tools()[0].id.0)).unwrap();
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
            let _ = guard
                .session_mut()
                .apply_toolpath_param_snapshot_narrow(0, new_op_clone, dressups, face_sel)
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
            let mut new_op = guard.baseline().operation.clone();
            new_op.set_feed_rate(9999.0);
            let _ = guard
                .session_mut()
                .apply_toolpath_param_snapshot_narrow(0, new_op, dressups, face_sel)
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

    // ── F.12 — per-toolpath air-cut fraction helper ──────────────────

    fn trace_with_summary(
        toolpath_id: ToolpathId,
        total_runtime_s: f64,
        air_cut_time_s: f64,
    ) -> crate::stock::simulation_cut::SimulationCutTrace {
        crate::stock::simulation_cut::SimulationCutTrace {
            sample_step_mm: 1.0,
            toolpath_summaries: vec![crate::stock::simulation_cut::SimulationToolpathCutSummary {
                toolpath_id,
                sample_count: 1,
                total_runtime_s,
                cutting_runtime_s: total_runtime_s,
                rapid_runtime_s: 0.0,
                air_cut_time_s,
                low_engagement_time_s: 0.0,
                average_engagement: 0.3,
                peak_chipload_mm_per_tooth: 0.05,
                peak_axial_doc_mm: 1.0,
                peak_plunge_descent_mm: 0.0,
                total_removed_volume_est_mm3: 1.0,
                average_mrr_mm3_s: 1.0,
                metrics_not_applicable: false,
                per_kinematics: std::collections::BTreeMap::new(),
                runtime_by_intent: None,
            }],
            ..crate::stock::simulation_cut::SimulationCutTrace::test_fixture()
        }
    }

    #[test]
    fn air_cut_fraction_divides_air_by_total_runtime() {
        let trace = trace_with_summary(ToolpathId(7), 100.0, 42.0);
        let pct = air_cut_fraction_of_total_runtime_from_trace(&trace, ToolpathId(7))
            .expect("summary present");
        assert!((pct - 0.42).abs() < 1e-9, "{pct}");
    }

    #[test]
    fn air_cut_fraction_missing_toolpath_returns_none() {
        let trace = trace_with_summary(ToolpathId(7), 100.0, 42.0);
        assert!(air_cut_fraction_of_total_runtime_from_trace(&trace, ToolpathId(99)).is_none());
    }

    #[test]
    fn air_cut_fraction_zero_runtime_returns_none() {
        let trace = trace_with_summary(ToolpathId(7), 0.0, 0.0);
        assert!(air_cut_fraction_of_total_runtime_from_trace(&trace, ToolpathId(7)).is_none());
    }

    // Ported from the deleted `stage1_grid_tests.rs`: the LUT routing
    // of the evaluation context survives the cull.
    #[test]
    fn lut_op_family_mapping_covers_all_variants() {
        use crate::feeds::OperationFamily;
        for (family, lut) in [
            (OperationFamily::Adaptive, LutOperationFamily::Adaptive),
            (OperationFamily::Pocket, LutOperationFamily::Pocket),
            (OperationFamily::Contour, LutOperationFamily::Contour),
            (OperationFamily::Parallel, LutOperationFamily::Parallel),
            (OperationFamily::Scallop, LutOperationFamily::Scallop),
            (OperationFamily::Trace, LutOperationFamily::Trace),
            (OperationFamily::Face, LutOperationFamily::Face),
        ] {
            assert_eq!(lut_op_family_from(family), lut);
        }
    }

    #[test]
    fn lut_pass_role_mapping_covers_all_variants() {
        use crate::feeds::PassRole;
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
