//! Pre-sim `CutterOpProfile` — one aggregate over the already-shipped
//! feeds surface (Phase 4 of `planning/architectural_refactor_2026-06-06_v2.md`).
//!
//! [`CutterOpProfile::for_combo`] runs the SAME canonical helpers every
//! production Suggest surface uses —
//! [`crate::feeds::suggest::feeds_explain_for_operation`] +
//! [`crate::feeds::suggest::suggest_for_operation`] — and bundles their
//! outputs with the safe envelope ([`crate::feeds::envelope::SafeEnvelope`],
//! CONTRACT K2b; it folds in the Phase-0 axial constraint envelope) into one
//! preflight view of a cutter × operation × material × machine combination.
//!
//! ## Boundaries (plan §3.2)
//!
//! - **Owns:** tool/op compatibility ([`FeedsError`] feasibility — NOT
//!   a new refusal type), feeds result + explain
//!   payload, pre-sim axial envelope, Suggest warnings.
//! - **Must not own:** the post-sim `ToolpathLoadVerdict` — simulation
//!   gate evaluation stays separate.
//! - The profile **reads** chipload bounds off the feeds result; it
//!   does not re-derive DOC derating
//!   ([`crate::feeds::geometry::doc_derating_scale`] stays canonical).

use crate::compute::catalog::{OperationConfig, OperationSpec, OperationType};
use crate::compute::cutter::build_cutter;
use crate::compute::tool_config::ToolConfig;
use crate::feeds::envelope::{EnvelopeInput, SafeEnvelope};
use crate::feeds::suggest::{
    SuggestContext, SuggestForOperationInput, SuggestWarning, feeds_explain_for_operation,
    suggest_for_operation,
};
use crate::feeds::{CutterKind, FeedsError, FeedsExplain, FeedsResult, VendorLut};
use crate::machine::MachineProfile;
use crate::material::Material;
use crate::tool::ToolDefinition;

// RETIRED 2026-09-18 (T-4): `Predictions` and the `CutterOpProfile::
// predictions` field. The struct held a `DeflectionPrediction` and a
// move-count estimate, was written on every profile build, and had no
// reader — production or test. `rg -n "\.predictions\b"` across
// `crates/` returned the declaration and nothing else; the two consumers
// of `CutterOpProfile` (`rs_cam_cli/src/project.rs` and
// `rs_cam_viz/src/app/mcp/generation.rs`) read `feasibility`,
// `suggested_operation`, `feeds` and `warnings` only. T-4 turned the
// deflection predictor's signature into a `Result`, and migrating a
// field nothing reads costs more than deleting it. `predict_move_count`
// keeps its live caller in `feeds::suggest::invariants`.

/// Input for [`CutterOpProfile::for_combo`]. Field-for-field the same
/// vocabulary as [`SuggestForOperationInput`]; the second lifetime
/// keeps the context's stock/bbox borrows (typically locals at the
/// call site) independent from the profile's own borrows.
#[derive(Clone, Copy)]
pub struct CutterOpProfileInput<'a, 'ctx> {
    pub operation: &'a OperationConfig,
    pub tool: &'a ToolConfig,
    pub machine: &'a MachineProfile,
    pub material: &'a Material,
    /// The vendor table. `'static`: the envelope keeps it for a pick.
    pub lut: &'static VendorLut,
    /// Spindle-RPM policy. See [`crate::feeds::SpindleStrategy`].
    pub spindle_strategy: crate::feeds::SpindleStrategy,
    /// Project-level context (model bbox, stock, policy). Consumed
    /// during construction only — the profile does not retain it.
    pub context: SuggestContext<'ctx>,
}

/// One preflight view of a cutter × operation combination — plan §3.2.
///
/// Everything here is **pre-sim**: feasibility, the canonical Suggest
/// recommendation, the explain payload and the axial constraint
/// envelope. The post-sim verdict
/// (`tool_load::ToolpathLoadVerdict`) is deliberately NOT part of the
/// profile.
///
/// No `Debug` derive: [`ToolDefinition`] wraps a `Box<dyn MillingCutter>`.
pub struct CutterOpProfile<'a> {
    pub tool_cfg: &'a ToolConfig,
    /// Canonical cutter built from `tool_cfg` (same [`build_cutter`]
    /// path the calculator uses).
    pub tool_def: ToolDefinition,
    /// The INPUT operation (pre-Suggest values). The recommended
    /// operating point lives in [`Self::suggested_operation`].
    pub operation: &'a OperationConfig,
    pub material: &'a Material,
    pub machine: &'a MachineProfile,

    /// Phase-3 cutter-shape classifier, derived from the tool
    /// definition's geometry hint.
    pub cutter_kind: CutterKind,
    pub op_type: OperationType,
    pub spec: OperationSpec,

    /// Tool × operation compatibility — `Err` carries the same
    /// [`FeedsError`] `suggest_for_operation` refuses with.
    pub feasibility: Result<(), FeedsError>,
    /// Calculator output at the recommended operating point. `None`
    /// when `feasibility` is `Err`.
    pub feeds: Option<FeedsResult>,
    /// Full explain payload (recommendation + matched LUT row +
    /// sibling rows + machine envelope). Always populated — the
    /// explain path renders a "would-have-produced" preview even for
    /// refused combinations.
    pub explain: FeedsExplain,
    /// The operation with Suggest's speeds written in (the output of
    /// `suggest_for_operation`; K2b: the speeds for the geometry the op
    /// keeps, no geometry). `None` when refused.
    pub suggested_operation: Option<OperationConfig>,
    /// CONTRACT K2b (feeds contract P5): the safe envelope of the op: the
    /// limits, the speeds map, the proposal and the operator's placement.
    /// It folds in the Phase-0 axial envelope (`ConstraintEnvelopes`).
    pub envelope: Result<SafeEnvelope, FeedsError>,
    /// Suggest's invariant-pass warnings — identical to what the GUI
    /// Suggest button / `--apply-suggest` would surface. Empty when
    /// refused.
    pub warnings: Vec<SuggestWarning>,
}

impl<'a> CutterOpProfile<'a> {
    /// Build the profile for one cutter × operation combination.
    ///
    /// Internally runs [`feeds_explain_for_operation`] +
    /// [`suggest_for_operation`] with the caller's exact context (no
    /// auto-populated policy/scope — a profile that filled those in
    /// differently would silently change live Suggest output), then
    /// evaluates the predictors and the axial envelope at the
    /// *recommended* operating point when Suggest succeeds, falling
    /// back to the input operating point on refusal.
    ///
    /// Refuses a tool outside its domain (F029): the profile holds a built
    /// cutter.
    pub fn for_combo(
        input: CutterOpProfileInput<'a, '_>,
    ) -> Result<Self, crate::tool::ToolGeometryError> {
        let op_type = input.operation.op_type();
        let spec = op_type.spec();
        let tool_def = build_cutter(input.tool)?;
        let cutter_kind = tool_def.to_geometry_hint().cutter_kind();

        let explain = feeds_explain_for_operation(
            input.operation,
            input.tool,
            input.material,
            input.machine,
            input.lut,
            input.spindle_strategy,
        );

        let suggest_outcome = suggest_for_operation(SuggestForOperationInput {
            operation: input.operation,
            tool: input.tool,
            machine: input.machine,
            material: input.material,
            lut: input.lut,
            spindle_strategy: input.spindle_strategy,
            context: input.context,
        });

        let (feasibility, feeds, suggested_operation, warnings) = match suggest_outcome {
            Ok(suggested) => (
                Ok(()),
                Some(suggested.feeds_result),
                Some(suggested.operation),
                suggested.warnings,
            ),
            Err(e) => (Err(e), None, None, Vec::new()),
        };

        let envelope = SafeEnvelope::for_cut(EnvelopeInput {
            operation: input.operation,
            tool: input.tool,
            machine: input.machine,
            material: input.material,
            lut: input.lut,
            spindle_strategy: input.spindle_strategy,
            context: input.context,
        });

        Ok(Self {
            tool_cfg: input.tool,
            tool_def,
            operation: input.operation,
            material: input.material,
            machine: input.machine,
            cutter_kind,
            op_type,
            spec,
            feasibility,
            feeds,
            explain,
            suggested_operation,
            envelope,
            warnings,
        })
    }

    /// The cut Suggest ships: the operation with Suggest's speeds, else
    /// the input operation (a refused combination).
    #[must_use]
    pub fn shipped_operation(&self) -> &OperationConfig {
        self.suggested_operation.as_ref().unwrap_or(self.operation)
    }

    /// Spindle power at the shipped cut (feeds contract P14, review M9):
    /// [`crate::feeds::power_at_shipped_point`] with the calculator point as
    /// the fallback. `None` when Suggest refused (no feeds result).
    #[must_use]
    pub fn shipped_power(
        &self,
    ) -> Option<Result<crate::feeds::PowerFigure, crate::feeds::PowerUnmodeled>> {
        self.feeds.as_ref().map(|feeds| {
            crate::feeds::power_at_shipped_point(
                self.shipped_operation(),
                self.tool_cfg,
                self.material,
                self.machine,
                feeds,
            )
        })
    }

    /// The force line at the mean chip of the shipped cut (P14, M9). `None`
    /// when Suggest refused, the material has no force line, or the cut
    /// resolves no point.
    #[must_use]
    pub fn shipped_force_point(&self) -> Option<crate::material::force_line::ForceAtPoint> {
        self.feeds.as_ref().and_then(|feeds| {
            crate::feeds::operating_point::force_at_shipped_point(
                self.shipped_operation(),
                self.tool_cfg,
                self.material,
                feeds,
            )
            .ok()
        })
    }
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]
mod tests {
    use super::*;
    use crate::compute::catalog::OperationType;
    use crate::compute::tool_config::{ToolId, ToolType};
    use crate::feeds::embedded_vendor_lut;
    use crate::machine::MachineProfile;
    use crate::material::Material;

    fn endmill_6mm() -> ToolConfig {
        let mut tool = ToolConfig::test_default(ToolId::from_raw(0), ToolType::EndMill);
        tool.diameter = 6.35;
        tool
    }

    fn profile_input<'a>(
        operation: &'a OperationConfig,
        tool: &'a ToolConfig,
        machine: &'a MachineProfile,
        material: &'a Material,
    ) -> CutterOpProfileInput<'a, 'static> {
        CutterOpProfileInput {
            operation,
            tool,
            machine,
            material,
            lut: embedded_vendor_lut(),
            spindle_strategy: crate::feeds::SpindleStrategy::default(),
            context: SuggestContext::default(),
        }
    }

    /// The profile's warnings / feeds / suggested operation must be
    /// byte-identical to calling `suggest_for_operation` directly with
    /// the same input — for_combo aggregates, it must never diverge.
    #[test]
    fn profile_matches_direct_suggest_call() {
        let operation = OperationConfig::new_default(OperationType::Adaptive3d);
        let tool = endmill_6mm();
        let machine = MachineProfile::default();
        let material = Material::default();

        let profile =
            CutterOpProfile::for_combo(profile_input(&operation, &tool, &machine, &material))
                .unwrap();

        let direct = suggest_for_operation(SuggestForOperationInput {
            operation: &operation,
            tool: &tool,
            machine: &machine,
            material: &material,
            lut: embedded_vendor_lut(),
            spindle_strategy: crate::feeds::SpindleStrategy::default(),
            context: SuggestContext::default(),
        })
        .expect("endmill × adaptive3d must be feasible");

        assert!(profile.feasibility.is_ok());
        assert_eq!(
            format!("{:?}", profile.warnings),
            format!("{:?}", direct.warnings),
            "profile warnings diverged from direct suggest_for_operation"
        );
        assert_eq!(
            format!("{:?}", profile.suggested_operation.as_ref().unwrap()),
            format!("{:?}", direct.operation),
            "profile suggested operation diverged from direct call"
        );
        assert_eq!(
            profile.feeds.as_ref().unwrap().feed_rate_mm_min,
            direct.feeds_result.feed_rate_mm_min,
        );
        assert_eq!(profile.op_type, OperationType::Adaptive3d);
        assert_eq!(profile.cutter_kind, CutterKind::Flat);
    }

    /// Refused combination (flat endmill × Scallop): feasibility
    /// carries the FeedsError, feeds/suggested are None, warnings
    /// empty — but explain is still populated (the modal renders a
    /// would-have-produced preview for refusals today).
    #[test]
    fn refused_combo_carries_feeds_error_and_keeps_explain() {
        let operation = OperationConfig::new_default(OperationType::Scallop);
        let tool = endmill_6mm();
        let machine = MachineProfile::default();
        let material = Material::default();

        let profile =
            CutterOpProfile::for_combo(profile_input(&operation, &tool, &machine, &material))
                .unwrap();

        assert!(
            matches!(
                profile.feasibility,
                Err(FeedsError::WrongToolForOperation { .. })
            ),
            "flat endmill × scallop must be refused"
        );
        assert!(profile.feeds.is_none());
        assert!(profile.suggested_operation.is_none());
        assert!(profile.warnings.is_empty());
        // Explain path never refuses.
        assert!(profile.explain.recommended.rpm > 0.0);
    }

    /// K2b: the profile carries the safe envelope; a refused pairing
    /// carries the refusal (D6: a missing input is not a finding).
    #[test]
    fn the_profile_carries_the_safe_envelope() {
        let tool = endmill_6mm();
        let machine = MachineProfile::default();
        let material = Material::default();

        let pocket = crate::compute::fixture_values::fixture_op(OperationType::Pocket);
        let profile =
            CutterOpProfile::for_combo(profile_input(&pocket, &tool, &machine, &material)).unwrap();
        let env = profile.envelope.as_ref().expect("a pocket is backed");
        assert!(
            env.limit(crate::feeds::envelope::LimitKind::FluteGuard)
                .is_some()
        );

        let scallop = OperationConfig::new_default(OperationType::Scallop);
        let profile =
            CutterOpProfile::for_combo(profile_input(&scallop, &tool, &machine, &material))
                .unwrap();
        assert!(
            profile.envelope.is_err(),
            "flat endmill × scallop is refused"
        );
    }
}
