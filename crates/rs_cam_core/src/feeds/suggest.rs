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
    FeedsInput, FeedsResult, PassRole, SetupContext, VendorLut, WorkholdingRigidity,
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
pub fn suggest_params(input: SuggestParamsInput<'_>) -> SuggestedParams {
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
pub fn suggest_for_operation(input: SuggestForOperationInput<'_>) -> SuggestedParams {
    let feeds_result = feeds_result_for_operation(
        input.operation,
        input.tool,
        input.material,
        input.machine,
        input.workholding,
        input.lut,
        input.spindle_strategy,
    );
    let mut operation = input.operation.clone();
    let warnings = apply_feeds_result_to_op(
        &mut operation,
        &feeds_result,
        input.tool,
        input.machine,
        input.operation.feeds_style().1,
    );
    SuggestedParams {
        operation,
        feeds_result,
        warnings,
    }
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
pub fn feeds_result_for_operation(
    operation: &OperationConfig,
    tool: &ToolConfig,
    material: &Material,
    machine: &MachineProfile,
    workholding: WorkholdingRigidity,
    lut: &VendorLut,
    spindle_strategy: crate::feeds::SpindleStrategy,
) -> FeedsResult {
    let input = feeds_input_for_operation(
        operation,
        tool,
        material,
        machine,
        workholding,
        lut,
        spindle_strategy,
    );
    crate::feeds::calculate(&input)
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
            let cap = machine.rigidity.doc_roughing_factor * tool.diameter;
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
    use crate::compute::operation_configs::PocketConfig;
    use crate::compute::tool_config::{ToolId, ToolType};
    use crate::feeds::{ChiploadSource, EMBEDDED_LUT};

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
        });
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
        });
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
}
