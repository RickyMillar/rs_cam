//! Per-toolpath evaluation context, baseline-restore RAII, and the
//! pure trace/LUT helpers each candidate evaluation reads.
//!
//! - [`EvaluationContext`] — resources (tool, material, op kind, LUT
//!   routing) computed once per `optimize_toolpath` call and shared
//!   across every candidate.
//! - [`ToolpathParamsSnapshot`] / [`BaselineRestoreGuard`] — RAII guard
//!   that snapshots `(operation, dressups, face_selection, feeds_auto)`
//!   at construction and re-applies them on drop. Restoration runs on
//!   every exit path (Ok, refusal, cancel, panic).
//! - Pure helpers: [`cycle_time_from_trace`], [`baseline_rpm_from_trace`],
//!   [`diameter_for_lut_lookup`], [`find_matched_lut_row`],
//!   [`lut_op_family_from`], [`lut_pass_role_from`],
//!   [`machine_max_power_kw`].

use crate::compute::catalog::{OperationConfig, OperationType};
use crate::compute::config::{DressupConfig, FeedsAutoMode};
use crate::enriched_mesh::FaceGroupId;
use crate::feeds::vendor_lookup::MatchedRow;
use crate::feeds::vendor_lut::{LutOperationFamily, LutPassRole};
use crate::feeds::{OperationFamily, PassRole};
use crate::machine::{MachineProfile, PowerModel};
use crate::session::{ProjectSession, SessionError};
use crate::simulation_cut::SimulationCutTrace;
use crate::tool::MillingCutter;

/// Look up this toolpath's per-toolpath cycle time from the trace's
/// per-toolpath summary. Returns `None` if the toolpath isn't
/// represented in the trace (no samples for that id — happens when
/// the toolpath was disabled or skipped).
pub(crate) fn cycle_time_from_trace(trace: &SimulationCutTrace, toolpath_id: usize) -> Option<f64> {
    trace
        .toolpath_summaries
        .iter()
        .find(|s| s.toolpath_id == toolpath_id)
        .map(|s| s.total_runtime_s)
}

/// Read the baseline cutting-RPM as the median spindle RPM observed in
/// the trace's cutting samples. Falls back to the operation's commanded
/// RPM, then to the machine's minimum RPM.
pub(crate) fn baseline_rpm_from_trace(
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
pub(crate) fn diameter_for_lut_lookup(
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
pub(crate) fn find_matched_lut_row(
    tool: &crate::tool::ToolDefinition,
    material: &crate::material::Material,
    ctx: &EvaluationContext,
    commanded_doc_mm: Option<f64>,
) -> Option<MatchedRow> {
    let tool_family = crate::tool_load::chipload::tool_family_for(tool.to_geometry_hint());
    let (lut_op_family, lut_pass_role) = crate::tool_load::chipload::routed_lookup_family(
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
    let lut = crate::tool_load::chipload::embedded_lut();
    crate::feeds::vendor_lookup::enumerate_matching_rows(lut, &criteria)
        .into_iter()
        .next()
}

/// Map the operation's `feeds_family` (used by the F&S calculator) to
/// the `LutOperationFamily` (used by the chipload/power gate's LUT
/// lookup). Same mapping the suggest module applied — kept as a
/// shared helper so optimize and suggest can't drift apart.
pub(crate) fn lut_op_family_from(family: OperationFamily) -> LutOperationFamily {
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
pub(crate) fn lut_pass_role_from(role: PassRole) -> LutPassRole {
    match role {
        PassRole::Roughing => LutPassRole::Roughing,
        PassRole::SemiFinish => LutPassRole::SemiFinish,
        PassRole::Finish => LutPassRole::Finish,
    }
}

/// Maximum power the spindle is capable of delivering at any RPM.
/// `ConstantPower` reports its flat figure; `VfdConstantTorque` reports
/// rated power (the cap above rated RPM).
pub(crate) fn machine_max_power_kw(machine: &MachineProfile) -> f64 {
    match machine.power {
        PowerModel::ConstantPower { power_kw } => power_kw,
        PowerModel::VfdConstantTorque { rated_power_kw, .. } => rated_power_kw,
    }
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
