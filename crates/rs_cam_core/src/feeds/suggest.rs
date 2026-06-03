//! Canonical feed/speed suggestion plumbing.
//!
//! This module owns the bridge from an [`OperationConfig`] + project context into
//! the feeds calculator, and the write-back path from a [`FeedsResult`] into an
//! operation. GUI, MCP, and diagnostics call through here so recommendations and
//! pre-sim warning baselines stay in lock-step.

use crate::compute::catalog::{OperationConfig, OperationType};
use crate::compute::cutter::build_cutter;
use crate::compute::tool_config::ToolConfig;
use crate::feeds::{
    FeedsError, FeedsInput, FeedsResult, OperationFamily as FeedsOperationFamily, PassRole,
    SetupContext, VendorLut, WorkholdingRigidity,
};
use crate::machine::MachineProfile;
use crate::material::Material;

/// Stock-derived values used by stock-aware operation defaults.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StockContext {
    /// Top of the stock in part-Z coordinates.
    pub stock_top_z: f64,
    /// Bottom of the stock in part-Z coordinates.
    pub stock_bottom_z: f64,
    /// Stock thickness in mm.
    pub stock_z: f64,
    /// Stock-top minus model-top margin in mm.
    pub stock_padding: f64,
}

impl StockContext {
    /// Build a stock context from a stock bbox and padding value.
    pub fn from_stock_bbox(bbox: crate::geo::BoundingBox3, padding: f64) -> Self {
        let stock_z = (bbox.max.z - bbox.min.z).max(0.0);
        Self {
            stock_top_z: bbox.max.z,
            stock_bottom_z: bbox.min.z,
            stock_z,
            stock_padding: padding,
        }
    }
}

/// Warnings emitted while applying suggestions to an operation.
#[derive(Debug, Clone, PartialEq)]
pub enum SuggestWarning {
    PlungeClampedToFeed { requested: f64, capped: f64 },
    StepoverClampedToToolDiameter { requested: f64, capped: f64 },
    RoughingDepthClampedToRigidity { requested: f64, capped: f64 },
    DepthClampedToCuttingLength { requested: f64, capped: f64 },
}

/// Canonical suggestion result: a fully-populated operation plus the calculator
/// output that produced it.
#[derive(Debug, Clone)]
pub struct SuggestedParams {
    pub operation: OperationConfig,
    pub feeds_result: FeedsResult,
    pub warnings: Vec<SuggestWarning>,
}

/// Input for [`suggest_params`].
#[derive(Clone, Copy)]
pub struct SuggestParamsInput<'a> {
    pub op_type: OperationType,
    pub tool: &'a ToolConfig,
    pub machine: &'a MachineProfile,
    pub material: &'a Material,
    pub workholding: WorkholdingRigidity,
    pub lut: &'a VendorLut,
    pub stock_ctx: &'a StockContext,
    /// Spindle-RPM policy. See [`crate::feeds::SpindleStrategy`].
    /// Defaults to `MatchChart` if the caller doesn't care about the
    /// distinction. `MaxSpeed` walks the constant-chipload line up to
    /// the spindle ceiling, capped by vendor.rpm_max when published.
    pub spindle_strategy: crate::feeds::SpindleStrategy,
}

/// Input for [`suggest_for_operation`].
#[derive(Clone, Copy)]
pub struct SuggestForOperationInput<'a> {
    pub operation: &'a OperationConfig,
    pub tool: &'a ToolConfig,
    pub machine: &'a MachineProfile,
    pub material: &'a Material,
    pub workholding: WorkholdingRigidity,
    pub lut: &'a VendorLut,
    /// Spindle-RPM policy. See [`crate::feeds::SpindleStrategy`].
    pub spindle_strategy: crate::feeds::SpindleStrategy,
}

/// Construct a default operation for `op_type`, apply stock-aware defaults, run
/// the feeds calculator, write recommendations into the operation, and enforce
/// cross-field invariants before returning.
///
/// Returns `Err(FeedsError)` when the tool × operation combination is
/// physically unrunnable (e.g. flat endmill assigned to a Scallop
/// op). Callers that want the legacy "always produce something"
/// behaviour should `.unwrap_or_else(|_| _)` or fall back to a default,
/// but the GUI / CLI / MCP Suggest buttons should surface the refusal
/// to the user instead of writing a meaningless recipe into the op.
pub fn suggest_params(input: SuggestParamsInput<'_>) -> Result<SuggestedParams, FeedsError> {
    let mut operation = OperationConfig::new_default(input.op_type);
    apply_stock_defaults(&mut operation, input.stock_ctx);
    suggest_for_operation(SuggestForOperationInput {
        operation: &operation,
        tool: input.tool,
        machine: input.machine,
        material: input.material,
        workholding: input.workholding,
        lut: input.lut,
        spindle_strategy: input.spindle_strategy,
    })
}

/// Run the canonical suggestion path for an existing operation. Operation fields
/// that act as feed-calculator hints (for example scallop height) are read from
/// `operation`; suggested feed/plunge/stepover/depth are written into a clone.
///
/// Returns `Err(FeedsError)` for physically-unrunnable
/// tool × operation combinations — see [`suggest_params`] for the
/// rationale.
pub fn suggest_for_operation(
    input: SuggestForOperationInput<'_>,
) -> Result<SuggestedParams, FeedsError> {
    let feeds_result = feeds_result_for_operation(
        input.operation,
        input.tool,
        input.material,
        input.machine,
        input.workholding,
        input.lut,
        input.spindle_strategy,
    )?;
    let mut operation = input.operation.clone();
    let warnings = apply_feeds_result_to_op(
        &mut operation,
        &feeds_result,
        input.tool,
        input.machine,
        input.operation.feeds_style().1,
    );
    apply_drill_defaults(&mut operation, input.tool, input.material);
    Ok(SuggestedParams {
        operation,
        feeds_result,
        warnings,
    })
}

/// Build a [`FeedsInput`] for the given operation. Shared by both
/// [`feeds_result_for_operation`] and [`feeds_explain_for_operation`]
/// so the recommendation and the explanation are always derived from
/// identical inputs.
fn feeds_input_for_operation<'a>(
    operation: &OperationConfig,
    tool: &'a ToolConfig,
    material: &'a Material,
    machine: &'a MachineProfile,
    workholding: WorkholdingRigidity,
    lut: &'a VendorLut,
    spindle_strategy: crate::feeds::SpindleStrategy,
) -> FeedsInput<'a> {
    let (family, role) = operation.feeds_style();
    let (axial_hint, radial_hint, scallop_hint) = operation_feeds_hints(operation);
    let tool_def = build_cutter(tool);
    FeedsInput {
        tool_diameter: tool.diameter,
        flute_count: tool.flute_count,
        flute_length: tool.cutting_length,
        shank_diameter: Some(tool.shank_diameter),
        tool_geometry: tool_def.to_geometry_hint(),
        material,
        machine,
        operation: family,
        pass_role: role,
        axial_depth_mm: axial_hint,
        radial_width_mm: radial_hint,
        target_scallop_mm: scallop_hint,
        vendor_lut: Some(lut),
        setup: SetupContext {
            tool_overhang_mm: Some(tool.stickout),
            workholding_rigidity: workholding,
        },
        spindle_strategy,
    }
}

/// Run the feeds calculator for an operation and project context without
/// mutating the operation.
///
/// Returns `Err(FeedsError)` when the tool × operation pairing is
/// physically unrunnable — see [`crate::feeds::validate_tool_for_operation`]
/// for the predicate. Existing callers that just want the numbers can
/// `.unwrap_or_else(|_| FeedsResult::default())`; the GUI Suggest path
/// (properties/feeds modal) should propagate the refusal so the user
/// sees why the recipe was withheld.
pub fn feeds_result_for_operation(
    operation: &OperationConfig,
    tool: &ToolConfig,
    material: &Material,
    machine: &MachineProfile,
    workholding: WorkholdingRigidity,
    lut: &VendorLut,
    spindle_strategy: crate::feeds::SpindleStrategy,
) -> Result<FeedsResult, FeedsError> {
    let input = feeds_input_for_operation(
        operation,
        tool,
        material,
        machine,
        workholding,
        lut,
        spindle_strategy,
    );
    crate::feeds::validate_tool_for_operation(&input)?;
    Ok(crate::feeds::calculate(&input))
}

/// Same inputs as [`feeds_result_for_operation`] but returns the full
/// [`FeedsExplain`] payload — recommended values plus matched LUT row,
/// sibling rows, and machine envelope. Used by the redesigned Feeds &
/// Speeds modal.
pub fn feeds_explain_for_operation(
    operation: &OperationConfig,
    tool: &ToolConfig,
    material: &Material,
    machine: &MachineProfile,
    workholding: WorkholdingRigidity,
    lut: &VendorLut,
    spindle_strategy: crate::feeds::SpindleStrategy,
) -> crate::feeds::FeedsExplain {
    let input = feeds_input_for_operation(
        operation,
        tool,
        material,
        machine,
        workholding,
        lut,
        spindle_strategy,
    );
    crate::feeds::explain_feeds(&input)
}

/// Write a [`FeedsResult`] into an [`OperationConfig`] and enforce the canonical
/// suggestion invariants. Values are rounded for UI-friendly display before
/// clamping, matching the historical Suggest-button behaviour.
pub fn apply_feeds_result_to_op(
    operation: &mut OperationConfig,
    result: &FeedsResult,
    tool: &ToolConfig,
    machine: &MachineProfile,
    pass_role: PassRole,
) -> Vec<SuggestWarning> {
    operation.set_feed_rate(round_suggestion_value(result.feed_rate_mm_min, 1.0));
    operation.set_plunge_rate(round_suggestion_value(result.plunge_rate_mm_min, 1.0));
    operation.set_stepover(round_suggestion_value(result.radial_width_mm, 0.001));
    operation.set_depth_per_pass(round_suggestion_value(result.axial_depth_mm, 0.001));
    enforce_invariants(operation, tool, machine, pass_role)
}

/// Apply drill-cycle defaults that depend on tool diameter + material —
/// specifically `peck_depth`, which gets overwritten with
/// `material.drill_default_peck_depth_mm(tool.diameter)`.
///
/// Pre-2026-06-02 `DrillConfig::default()` hardcoded `peck_depth = 3.0`
/// regardless of cutter diameter or material; for a 3 mm bit that's a
/// full-diameter peck (unsafe), for a 12 mm bit that's 0.25×D
/// (trivially shallow). The Suggest path now overwrites the value
/// every time it runs, so the operating point auto-scales with tool
/// diameter and material per-peck threshold (audit finding "peck_depth
/// is a hardcoded constant with no diameter/material scaling",
/// workflow `w39ma2j1y`, fix #6).
///
/// Non-drill ops are untouched.
pub fn apply_drill_defaults(
    operation: &mut OperationConfig,
    tool: &ToolConfig,
    material: &Material,
) {
    let d = tool.diameter;
    if !d.is_finite() || d <= 0.0 {
        return;
    }
    let peck = material.drill_default_peck_depth_mm(d);
    match operation {
        OperationConfig::Drill(cfg) => {
            // Clamp peck so the cycle still pecks at least once
            // (peck must be strictly less than hole depth). With the
            // Janka-banded per-peck max introduced 2026-06-03, a
            // softwood Suggest default of 3×D can exceed shallow
            // drill_depth values (e.g. 12 mm bit + 25 mm hole gives
            // 36 mm peck before clamping). Without this guard the
            // matrix `drill_no_peck_cycle` anti-pattern fires.
            cfg.peck_depth = clamp_peck_to_depth(peck, cfg.depth);
        }
        OperationConfig::AlignmentPinDrill(cfg) => cfg.peck_depth = peck,
        _ => {}
    }
}

/// Ensure the Suggest-default peck depth still produces a real peck
/// cycle. If `peck >= depth` the operator gets a single-shot drill
/// with no chip evacuation; clamp to a fixed fraction of the hole
/// depth so the cycle always pecks at least twice. Chosen factor:
/// 0.75 — keeps the peck count low (2 pecks for typical holes) while
/// staying strictly below `drill_depth`.
fn clamp_peck_to_depth(peck: f64, drill_depth: f64) -> f64 {
    if !drill_depth.is_finite() || drill_depth <= 0.0 {
        return peck;
    }
    let ceiling = drill_depth * 0.75;
    peck.min(ceiling)
}

/// Apply stock-aware overrides to defaults that require stock context.
pub fn apply_stock_defaults(operation: &mut OperationConfig, ctx: &StockContext) {
    match operation {
        OperationConfig::DropCutter(cfg) => {
            cfg.min_z = ctx.stock_bottom_z;
        }
        OperationConfig::Face(cfg) => {
            cfg.depth = ctx.stock_padding.max(1.0);
        }
        OperationConfig::Profile(cfg) => {
            cfg.depth = ctx.stock_z;
        }
        OperationConfig::Drill(cfg) => {
            cfg.depth = ctx.stock_z;
        }
        OperationConfig::Pocket(cfg) => {
            cfg.depth = (ctx.stock_z * 0.5).min(5.0);
        }
        OperationConfig::Adaptive(cfg) => {
            cfg.depth = ctx.stock_z * 0.5;
        }
        _ => {}
    }
}

/// Extract operation-specific hints for the feeds calculator.
/// Returns `(axial_depth_hint, radial_width_hint, scallop_hint)`.
pub fn operation_feeds_hints(
    operation: &OperationConfig,
) -> (Option<f64>, Option<f64>, Option<f64>) {
    match operation {
        OperationConfig::Scallop(cfg) => (None, None, Some(cfg.scallop_height)),
        // DropCutter (the "3D Finish" parallel-raster op) optionally
        // derives stepover from a scallop target the same way Scallop
        // does. `None` keeps the legacy `ae_factor × diameter` stepover.
        OperationConfig::DropCutter(cfg) => (None, None, cfg.scallop_height),
        OperationConfig::Waterline(cfg) => (Some(cfg.z_step), None, None),
        OperationConfig::SteepShallow(cfg) => (Some(cfg.z_step), None, None),
        OperationConfig::VCarve(cfg) => (Some(cfg.max_depth), None, None),
        OperationConfig::RampFinish(cfg) => (Some(cfg.max_stepdown), None, None),
        _ => (None, None, None),
    }
}

fn enforce_invariants(
    operation: &mut OperationConfig,
    tool: &ToolConfig,
    machine: &MachineProfile,
    pass_role: PassRole,
) -> Vec<SuggestWarning> {
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

    if let Some(dpp) = operation.depth_per_pass() {
        let mut current = dpp;
        if matches!(pass_role, PassRole::Roughing) {
            // Adaptive ops are a "deep, narrow" engagement strategy —
            // low radial WOC lets the cutter take a high axial DOC the
            // conventional roughing factor doesn't allow. Pre-2026-06-02
            // the clamp used `doc_roughing_factor` uniformly, stripping
            // the upstream adaptive ap (computed via
            // `adaptive_doc_factor` at feeds/mod.rs:913) back down to
            // the conventional ceiling — defeating the adaptive
            // paradigm. The clamp now branches on feeds family.
            //
            // Audit finding: BUG 1 — "Adaptive DOC clamped by
            // conventional-roughing factor" (workflow `w39ma2j1y`).
            let is_adaptive_family = operation.op_type().spec().feeds_family
                == FeedsOperationFamily::Adaptive;
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
                current = cap;
            }
        }
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

/// Round a suggestion value to the nearest multiple of `step`.
///
/// Used by the Suggest path to produce UI-friendly numbers before
/// clamping (e.g. round chipload to 0.005 mm, RPM to 100). When
/// `step <= 0.0` the value is returned unchanged.
pub fn round_suggestion_value(value: f64, step: f64) -> f64 {
    if step <= 0.0 {
        return value;
    }
    (value / step).round() * step
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::compute::operation_configs::{DropCutterConfig, PocketConfig};
    use crate::compute::tool_config::{ToolId, ToolType};
    use crate::feeds::{ChiploadSource, EMBEDDED_LUT};

    /// Ball-nose tool of the given diameter (tip radius = diameter / 2).
    fn ball_tool(diameter: f64) -> ToolConfig {
        let mut tool = ToolConfig::new_default(ToolId(0), ToolType::BallNose);
        tool.diameter = diameter;
        tool.cutting_length = 25.0;
        tool
    }

    /// DropCutter (3D Finish) operation with an optional scallop target.
    fn dropcutter_op(scallop_height: Option<f64>) -> OperationConfig {
        OperationConfig::DropCutter(DropCutterConfig {
            scallop_height,
            ..DropCutterConfig::default()
        })
    }

    /// Run the canonical feeds path for a DropCutter op + tool and return
    /// the suggested radial stepover (mm).
    fn dropcutter_stepover(op: &OperationConfig, tool: &ToolConfig) -> f64 {
        feeds_result_for_operation(
            op,
            tool,
            &Material::default(),
            &MachineProfile::default(),
            WorkholdingRigidity::Medium,
            &EMBEDDED_LUT,
            crate::feeds::SpindleStrategy::default(),
        )
        .expect("dropcutter stepover should not be refused for ball tool")
        .radial_width_mm
    }

    /// S1: a scallop target on DropCutter overrides the `ae_factor`
    /// formula stepover with the chord-height geometry stepover. A 10 μm
    /// scallop on a 1 mm ball (tip r = 0.5 mm) gives
    /// `2·√(2·0.5·0.010 − 0.010²) ≈ 0.199 mm` — practical wood finish —
    /// instead of the sub-micron formula value.
    #[test]
    fn scallop_height_some_overrides_default_ae_factor() {
        let stepover = dropcutter_stepover(&dropcutter_op(Some(0.010)), &ball_tool(1.0));
        let expected = 2.0 * (2.0 * 0.5 * 0.010 - 0.010_f64.powi(2)).sqrt();
        assert!(
            (stepover - expected).abs() < 1e-6,
            "scallop stepover {stepover} should match chord-height {expected}"
        );
        assert!(
            (0.198..0.200).contains(&stepover),
            "10 μm scallop on a 1 mm ball should give ~0.199 mm, got {stepover}"
        );
    }

    /// S1: with no scallop target the legacy formula-based stepover is
    /// preserved — the scallop override must not engage, so the result
    /// stays the tiny `ae_factor`-derived value (well under the scallop
    /// path's 0.199 mm). Guards the F-037 smoke baseline.
    #[test]
    fn scallop_height_none_preserves_legacy_ae() {
        let stepover = dropcutter_stepover(&dropcutter_op(None), &ball_tool(1.0));
        assert!(
            stepover > 0.0 && stepover < 0.05,
            "legacy DropCutter stepover on a 1 mm ball should stay small \
             (formula-based), got {stepover}"
        );
    }

    /// S1: a tapered ball cuts the same cusp curve as a true ball of the
    /// same tip radius — the scallop stepover is determined by the
    /// spherical tip only. A tapered ball with tip radius 0.5 mm
    /// (diameter 1.0 mm) must produce the same stepover as a 1 mm true
    /// ball for the same scallop target.
    #[test]
    fn scallop_height_uses_tip_radius_for_tapered_ball() {
        let mut tapered = ToolConfig::new_default(ToolId(0), ToolType::TaperedBallNose);
        tapered.diameter = 1.0; // tip diameter → tip radius 0.5 mm
        tapered.cutting_length = 25.0;
        let tapered_step = dropcutter_stepover(&dropcutter_op(Some(0.010)), &tapered);
        let ball_step = dropcutter_stepover(&dropcutter_op(Some(0.010)), &ball_tool(1.0));
        assert!(
            (tapered_step - ball_step).abs() < 1e-9,
            "tapered ball (tip r=0.5) stepover {tapered_step} should equal \
             1 mm true ball stepover {ball_step}"
        );
    }

    fn stock_ctx() -> StockContext {
        StockContext {
            stock_top_z: 10.0,
            stock_bottom_z: 0.0,
            stock_z: 10.0,
            stock_padding: 1.0,
        }
    }

    fn tool(diameter: f64) -> ToolConfig {
        let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
        tool.diameter = diameter;
        tool.cutting_length = 25.0;
        tool
    }

    #[test]
    fn vendor_lut_row_present_path_is_used() {
        let result = suggest_params(SuggestParamsInput {
            op_type: OperationType::Pocket,
            tool: &tool(6.35),
            machine: &MachineProfile::default(),
            material: &Material::default(),
            workholding: WorkholdingRigidity::Medium,
            lut: &EMBEDDED_LUT,
            stock_ctx: &stock_ctx(),
            spindle_strategy: crate::feeds::SpindleStrategy::default(),
        })
        .expect("pocket + flat is not a refused combination");
        assert!(matches!(
            result.feeds_result.chipload_source,
            ChiploadSource::VendorLut { .. }
        ));
    }

    #[test]
    fn fallback_formula_path_is_used_without_matching_lut_row() {
        // 200 mm exceeds 10x the largest embedded flat-end pocket row
        // (12.7 mm compression spiral), so no row passes the diameter
        // sanity floor and the empirical fallback must take over.
        let result = suggest_params(SuggestParamsInput {
            op_type: OperationType::Pocket,
            tool: &tool(200.0),
            machine: &MachineProfile::default(),
            material: &Material::default(),
            workholding: WorkholdingRigidity::Medium,
            lut: &EMBEDDED_LUT,
            stock_ctx: &stock_ctx(),
            spindle_strategy: crate::feeds::SpindleStrategy::default(),
        })
        .expect("pocket + flat is not a refused combination");
        assert_eq!(
            result.feeds_result.chipload_source,
            ChiploadSource::FormulaFallback
        );
    }

    #[test]
    fn invariants_clamp_and_warn() {
        let mut op = OperationConfig::Pocket(PocketConfig {
            feed_rate: 100.0,
            plunge_rate: 250.0,
            stepover: 12.0,
            depth_per_pass: 20.0,
            ..PocketConfig::default()
        });
        let mut tool = tool(6.0);
        tool.cutting_length = 1.0;
        let mut machine = MachineProfile::default();
        machine.rigidity.doc_roughing_factor = 0.25;

        let warnings = enforce_invariants(&mut op, &tool, &machine, PassRole::Roughing);

        assert_eq!(op.plunge_rate(), 100.0);
        assert_eq!(op.stepover(), Some(6.0));
        assert_eq!(op.depth_per_pass(), Some(1.0));
        assert!(
            warnings
                .iter()
                .any(|w| matches!(w, SuggestWarning::PlungeClampedToFeed { .. }))
        );
        assert!(
            warnings
                .iter()
                .any(|w| matches!(w, SuggestWarning::StepoverClampedToToolDiameter { .. }))
        );
        assert!(
            warnings
                .iter()
                .any(|w| matches!(w, SuggestWarning::RoughingDepthClampedToRigidity { .. }))
        );
        assert!(
            warnings
                .iter()
                .any(|w| matches!(w, SuggestWarning::DepthClampedToCuttingLength { .. }))
        );
    }

    /// Fix #6 (2026-06-02 audit): `peck_depth` is overwritten by
    /// `material.drill_default_peck_depth_mm(D)` when the Suggest
    /// path runs.
    ///
    /// **Updated 2026-06-03 (P5 lit-matrix fix):** softwood per-peck
    /// max is now Janka-banded at 6.0×D (was a flat 2.0 for every
    /// wood). Softwood Suggest default is therefore 0.5 × 6.0 × D =
    /// 3.0×D — inside the matrix band 3–8×D — instead of the old
    /// 1.0×D that matched dense hardwood. `apply_drill_defaults`
    /// additionally clamps the result to `0.75 × drill_depth` so
    /// even shallow holes still peck at least twice. For the test's
    /// default `DrillConfig` (`depth = 10 mm`) the clamp ceiling is
    /// 7.5 mm.
    #[test]
    fn drill_peck_depth_scales_with_diameter_and_material() {
        use crate::compute::operation_configs::{DrillConfig, DrillCycleType};
        use crate::material::Material;

        let material = Material::SolidWood {
            species: crate::material::WoodSpecies::GenericSoftwood,
        };

        // 3 mm bit — pre-2026-06-03 result was 3.0 mm (1.0×D, matched
        // hardwood). Janka-banded softwood now defaults to 0.5×6.0×D
        // = 9.0 mm, then clamps to 0.75 × default depth (10 mm) = 7.5 mm.
        let mut op_3mm = OperationConfig::Drill(DrillConfig {
            cycle: DrillCycleType::Peck,
            peck_depth: 3.0, // pre-fix hardcode value
            ..DrillConfig::default()
        });
        let mut tool_3mm = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
        tool_3mm.diameter = 3.0;

        apply_drill_defaults(&mut op_3mm, &tool_3mm, &material);

        let peck_3mm = match &op_3mm {
            OperationConfig::Drill(cfg) => cfg.peck_depth,
            _ => panic!("expected Drill"),
        };
        assert!(
            (peck_3mm - 7.5).abs() < 1e-6,
            "3 mm bit softwood peck depth should be min(0.5×6.0×3.0, 0.75×10.0) = 7.5 mm \
             (Janka-banded softwood clamped to 0.75×depth), got {peck_3mm}"
        );

        // 12 mm bit — softwood Janka band gives 0.5×6.0×12 = 36.0 mm,
        // clamped to 0.75 × 10 mm = 7.5 mm by the default depth.
        let mut op_12mm = OperationConfig::Drill(DrillConfig {
            cycle: DrillCycleType::Peck,
            peck_depth: 3.0, // pre-fix hardcode value
            ..DrillConfig::default()
        });
        let mut tool_12mm = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
        tool_12mm.diameter = 12.0;

        apply_drill_defaults(&mut op_12mm, &tool_12mm, &material);

        let peck_12mm = match &op_12mm {
            OperationConfig::Drill(cfg) => cfg.peck_depth,
            _ => panic!("expected Drill"),
        };
        assert!(
            peck_12mm > 6.0,
            "12 mm bit softwood peck depth should scale up beyond the old 3.0 mm hardcode, got {peck_12mm}"
        );

        // Non-drill op: untouched.
        let mut op_pocket = OperationConfig::Pocket(PocketConfig::default());
        let pocket_before = format!("{op_pocket:?}");
        apply_drill_defaults(&mut op_pocket, &tool_3mm, &material);
        assert_eq!(
            format!("{op_pocket:?}"),
            pocket_before,
            "apply_drill_defaults must be a no-op for non-drill ops"
        );
    }

    /// P5 (2026-06-03 literature-matrix triage): softwood Suggest
    /// per-peck must exceed dense-hardwood Suggest per-peck. Before
    /// the Janka-banded `drill_per_peck_max_dtd` patch, every wood
    /// species returned a flat 2.0 max → 1.0×D default, so softwood
    /// pecks collapsed to the same value as ipe (Janka 3510). That
    /// triggered the matrix anti-pattern
    /// `softwood_drill_matches_hardwood_peck` (`peck_over_d < 3.0`)
    /// on `flat_3mm_drill_softwood`.
    ///
    /// Sources for the 3–8×D softwood band: Onsrud Drill Chart, FPL
    /// Wood Handbook §3.7, Vectric drill defaults, Amana Spektra.
    /// Hardwood ceiling of 5×D is the matrix invariant for white oak
    /// in `flat_3mm_drill_oak`.
    #[test]
    fn drill_peck_depth_softwood_exceeds_hardwood() {
        use crate::compute::operation_configs::{DrillConfig, DrillCycleType};
        use crate::material::{Material, WoodSpecies};

        let pine = Material::SolidWood {
            species: WoodSpecies::RadiataPine, // Janka 710 → just above 700 cutoff;
                                               // use GenericSoftwood for true softwood
        };
        let generic_softwood = Material::SolidWood {
            species: WoodSpecies::GenericSoftwood, // Janka 600
        };
        let white_oak = Material::SolidWood {
            species: WoodSpecies::WhiteOak, // Janka 1360 (medium hardwood)
        };
        let ipe = Material::SolidWood {
            species: WoodSpecies::Ipe, // Janka 3510 (dense hardwood / unknown bucket)
        };

        let diameter = 6.0_f64;
        let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
        tool.diameter = diameter;

        // Use a deep hole so the per-material Suggest defaults stay
        // below the 0.75 × drill_depth clamp; otherwise all materials
        // saturate at the same ceiling and the ordering can't be
        // tested. 50 mm hole → 37.5 mm clamp ceiling, well above any
        // 6 mm-diameter Suggest result.
        let drill_depth = 50.0_f64;

        let peck_for = |material: &Material| -> f64 {
            let mut op = OperationConfig::Drill(DrillConfig {
                cycle: DrillCycleType::Peck,
                peck_depth: 3.0,
                depth: drill_depth,
                ..DrillConfig::default()
            });
            apply_drill_defaults(&mut op, &tool, material);
            match &op {
                OperationConfig::Drill(cfg) => cfg.peck_depth,
                _ => panic!("expected Drill"),
            }
        };

        let pine_peck = peck_for(&pine);
        let generic_softwood_peck = peck_for(&generic_softwood);
        let oak_peck = peck_for(&white_oak);
        let ipe_peck = peck_for(&ipe);

        let pine_over_d = pine_peck / diameter;
        let softwood_over_d = generic_softwood_peck / diameter;
        let oak_over_d = oak_peck / diameter;
        let ipe_over_d = ipe_peck / diameter;

        // Softwood must clear the matrix floor (3×D) for
        // `softwood_drill_matches_hardwood_peck` (peck_over_d < 3.0).
        assert!(
            softwood_over_d >= 3.0,
            "generic softwood peck must be ≥ 3×D (matrix band floor for `flat_3mm_drill_softwood`), \
             got {softwood_over_d:.3}×D"
        );
        // RadiataPine is right at the 700 Janka softwood cutoff (710);
        // it falls into the medium-hardwood band by design — assert it
        // at least beats the dense bucket.
        assert!(
            pine_over_d >= 2.5,
            "radiata pine peck should be ≥ 2.5×D (sits at softwood/medium boundary), \
             got {pine_over_d:.3}×D"
        );
        // Softwood < 8×D ceiling.
        assert!(
            softwood_over_d <= 8.0,
            "softwood peck must stay ≤ 8×D (Onsrud/FPL upper band), got {softwood_over_d:.3}×D"
        );
        // Hardwood ceiling (matrix `peck_over_d_hardwood` invariant: 5×D).
        assert!(
            oak_over_d <= 5.0,
            "white oak peck must stay ≤ 5×D (matrix hardwood ceiling), got {oak_over_d:.3}×D"
        );
        assert!(
            ipe_over_d <= 5.0,
            "ipe peck must stay ≤ 5×D (matrix hardwood ceiling), got {ipe_over_d:.3}×D"
        );

        // Ordering: softwood > oak > ipe (matches density / Janka).
        assert!(
            softwood_over_d > oak_over_d,
            "softwood ({softwood_over_d:.3}×D) must exceed white oak ({oak_over_d:.3}×D)"
        );
        assert!(
            oak_over_d > ipe_over_d,
            "white oak ({oak_over_d:.3}×D) must exceed ipe ({ipe_over_d:.3}×D)"
        );
    }

    /// Bug 1 (2026-06-02 audit, workflow `w39ma2j1y`): adaptive ops
    /// use `adaptive_doc_factor` (deep+narrow), not the
    /// conventional `doc_roughing_factor`. For a 6 mm bit on a wood
    /// router with `doc_roughing_factor=0.20` and
    /// `adaptive_doc_factor=1.50`, an Adaptive3d op with DOC=6 mm
    /// must not be clamped. Pre-fix the clamp stripped it to 1.2 mm,
    /// erasing the upstream `adaptive_doc_factor` floor and turning
    /// adaptive into "shallow conventional".
    #[test]
    fn adaptive_op_uses_adaptive_doc_factor_not_doc_roughing_factor() {
        use crate::compute::operation_configs::Adaptive3dConfig;
        let mut op = OperationConfig::Adaptive3d(Adaptive3dConfig {
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            stepover: 0.88,
            depth_per_pass: 6.0,
            ..Adaptive3dConfig::default()
        });
        let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
        tool.diameter = 6.0;
        tool.cutting_length = 25.0;
        let mut machine = MachineProfile::default();
        machine.rigidity.doc_roughing_factor = 0.20; // conventional
        machine.rigidity.adaptive_doc_factor = 1.50; // adaptive can go deep

        let warnings = enforce_invariants(&mut op, &tool, &machine, PassRole::Roughing);

        // DOC=6 mm is under adaptive_doc_factor*D=9 mm and under
        // cutting_length=25 mm — should NOT clamp.
        assert_eq!(
            op.depth_per_pass(),
            Some(6.0),
            "Adaptive3d DOC=6 mm on 6 mm bit must not clamp (adaptive_doc_factor=1.5)"
        );
        assert!(
            !warnings
                .iter()
                .any(|w| matches!(w, SuggestWarning::RoughingDepthClampedToRigidity { .. })),
            "Adaptive3d DOC=6 mm must not trigger RoughingDepthClampedToRigidity \
             (was Bug 1 — adaptive ops clamped by conventional factor)"
        );
    }

    /// Bug 1 counter-test: conventional Pocket op DOES still clamp to
    /// `doc_roughing_factor` — the fix is selective on feeds family,
    /// not a blanket relaxation.
    #[test]
    fn conventional_op_still_clamps_by_doc_roughing_factor() {
        let mut op = OperationConfig::Pocket(PocketConfig {
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            stepover: 3.0,
            depth_per_pass: 6.0,
            ..PocketConfig::default()
        });
        let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
        tool.diameter = 6.0;
        tool.cutting_length = 25.0;
        let mut machine = MachineProfile::default();
        machine.rigidity.doc_roughing_factor = 0.20;
        machine.rigidity.adaptive_doc_factor = 1.50;

        let warnings = enforce_invariants(&mut op, &tool, &machine, PassRole::Roughing);

        // Pocket on 6 mm bit: doc_roughing_factor*D = 1.2 mm cap.
        let dpp = op.depth_per_pass().expect("dpp set after clamp");
        assert!(
            (dpp - 1.2).abs() < 1e-6,
            "Pocket DOC must still clamp to doc_roughing_factor*D (1.2), got {dpp}"
        );
        assert!(
            warnings
                .iter()
                .any(|w| matches!(w, SuggestWarning::RoughingDepthClampedToRigidity { .. })),
            "Pocket DOC over conventional ceiling must still warn"
        );
    }
}
