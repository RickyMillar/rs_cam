//! The levers of the post-sim tuner (PLAN.md §2 of
//! `planning/optimize_review_2026-10-12/PLAN.md`).
//!
//! - Lever L1, links: [`link_dressups`].
//! - Lever L2, depth and width: [`depth_width_plan`] forms the cells,
//!   and [`depth_width_cell`] writes one cell through the feeds
//!   calculator.
//! - [`unmodeled_note`]: the statement on the outcome when the stock has
//!   no force line.
//!
//! **The division of labour (operator ruling 2026-10-12).** The feeds
//! screen (Suggest) owns the calculation before a simulation and keeps
//! its rigidity clamp. This module never calls that clamp and carries
//! no private feed, chipload, power or deflection formula. The depth
//! bound comes from `feeds::cutter_constraints`, the feed, plunge, ramp
//! and RPM from the feeds calculator, and the depth staircase from
//! `ops::depth`.

use crate::compute::catalog::OperationConfig;
use crate::compute::config::{DressupConfig, LinkDressupParams};
use crate::compute::tool_config::ToolConfig;
use crate::feeds::{FeedsInput, FeedsResult, SpindleStrategy};
use crate::machine::MachineProfile;
use crate::material::Material;

/// The largest number of depth-per-pass values the search takes for one
/// stepover (PLAN.md §2 step 3).
pub(crate) const MAX_DOC_VALUES: usize = 4;

/// Two depths closer than this are one depth, in mm (PLAN.md §2 step 3).
const DOC_DEDUP_MM: f64 = 1e-6;

/// The step the Suggest funnel rounds a depth and a stepover to, in mm.
///
/// Source: `feeds/suggest/apply.rs`, `apply_feeds_subset`, which writes
/// `round_suggestion_value(result.axial_depth_mm, 0.001)` and the same
/// for the stepover. A cell whose depth the calculator moved by more than
/// this step is refused.
pub(crate) const GEOMETRY_ROUNDING_STEP_MM: f64 = 0.001;

/// Lever L1: the dressups of the link candidate.
///
/// The candidate is the baseline dressups with
/// `link_moves = Some(LinkDressupParams::default())`
/// (`compute/config.rs`). `None` when the baseline already links (the
/// lever has nothing to change), or when the operation's registry row
/// strips the links (`DressupPolicy::strip_all_reason`, the rule
/// `DressupConfig::normalize_for_op` applies).
#[must_use]
pub fn link_dressups(
    baseline: &DressupConfig,
    op_type: crate::compute::catalog::OperationType,
) -> Option<DressupConfig> {
    if baseline.link_moves.is_some()
        || op_type
            .registry_entry()
            .dressup_policy
            .strip_all_reason
            .is_some()
    {
        return None;
    }
    Some(DressupConfig {
        link_moves: Some(LinkDressupParams::default()),
        ..baseline.clone()
    })
}

/// The project inputs the feeds calculator reads for a cell.
#[derive(Clone, Copy)]
pub struct CellInputs<'a> {
    /// The tool of the toolpath.
    pub tool: &'a ToolConfig,
    /// The stock material.
    pub material: &'a Material,
    /// The machine profile.
    pub machine: &'a MachineProfile,
    /// `PostConfig::spindle_strategy`, the value the session's Suggest
    /// door passes (`session/compute/export.rs`,
    /// `feeds_result_for_toolpath`).
    pub spindle_strategy: SpindleStrategy,
    /// The dressups the cell runs with. The ramp feed reads the entry.
    pub dressups: &'a DressupConfig,
}

/// One cell of lever L2.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DepthWidthCell {
    /// The depth per pass, in mm. A value on the realised staircase.
    pub depth_per_pass_mm: f64,
    /// The stepover, in mm.
    pub stepover_mm: f64,
}

/// The cells of lever L2 and the depth bound of each stepover.
#[derive(Debug, Clone, Default)]
pub struct DepthWidthPlan {
    /// The cells, deepest first within each stepover.
    pub cells: Vec<DepthWidthCell>,
    /// `(stepover_mm, depth bound in mm)` for each stepover the plan
    /// took.
    pub bounds: Vec<(f64, f64)>,
}

/// The depth-per-pass values for one stepover (PLAN.md §2 step 3).
///
/// The reachable depths for a total depth `T` are `T / n`
/// (`ops::depth::realised_step_down`). The values are: the deepest
/// reachable depth at or below `bound_mm`, the baseline's own realised
/// step, and up to two values with an evenly spaced `n` between them.
/// No value is shallower than the baseline. At most
/// [`MAX_DOC_VALUES`] values; two values within 1e-6 mm are one.
#[must_use]
pub fn doc_values(total_depth_mm: f64, baseline_step_mm: f64, bound_mm: f64) -> Vec<f64> {
    use crate::ops::depth::realised_step_down;
    let Some(base_step) = realised_step_down(total_depth_mm, baseline_step_mm) else {
        return Vec::new();
    };
    let n_of = |step: f64| (total_depth_mm / step).round().max(1.0);
    let n_base = n_of(base_step);
    let Some(bound_step) = realised_step_down(total_depth_mm, bound_mm) else {
        return vec![base_step];
    };
    let n_bound = n_of(bound_step);
    let mut counts: Vec<f64> = vec![n_bound.min(n_base)];
    if n_bound < n_base {
        let span = n_base - n_bound;
        for k in 1..=2 {
            let n = (n_bound + span * f64::from(k) / 3.0).round();
            if n > n_bound && n < n_base {
                counts.push(n);
            }
        }
        counts.push(n_base);
    }
    let mut values: Vec<f64> = Vec::with_capacity(counts.len());
    for n in counts {
        let step = total_depth_mm / n;
        if step + DOC_DEDUP_MM < base_step {
            continue;
        }
        if values.iter().all(|v| (v - step).abs() > DOC_DEDUP_MM) {
            values.push(step);
        }
    }
    values.sort_by(|a, b| b.total_cmp(a));
    values.truncate(MAX_DOC_VALUES);
    values
}

/// The feeds calculator at a pinned depth and stepover.
///
/// The input is the one the production door builds
/// ([`crate::feeds::suggest::feeds_input_for_operation`]); only the two
/// geometry hints are pinned. A pocket publishes no geometry hint, so the
/// production door would size the feed at the calculator's own depth,
/// not at the cell's. With a pinned depth the calculator honours it
/// (`feeds/mod.rs`, "Apply user overrides") and its power ladder does not
/// move it.
///
/// # Errors
///
/// The calculator's refusal of the tool and operation pair.
pub fn feeds_at_cell(
    operation: &OperationConfig,
    inputs: &CellInputs<'_>,
    depth_per_pass_mm: Option<f64>,
    stepover_mm: Option<f64>,
) -> Result<FeedsResult, crate::feeds::FeedsError> {
    let mut input: FeedsInput<'_> = crate::feeds::suggest::feeds_input_for_operation(
        operation,
        inputs.tool,
        inputs.material,
        inputs.machine,
        crate::feeds::embedded_vendor_lut(),
        inputs.spindle_strategy,
    );
    if depth_per_pass_mm.is_some() {
        input.axial_depth_mm = depth_per_pass_mm;
    }
    if stepover_mm.is_some() {
        input.radial_width_mm = stepover_mm;
    }
    crate::feeds::validate_tool_for_operation(&input)?;
    Ok(crate::feeds::calculate(&input))
}

/// Form the cells of lever L2 for an operation.
///
/// Returns an empty plan when the operation has no depth per pass or no
/// total depth.
///
/// - **Stepover values**: the baseline stepover and the calculator's
///   `radial_width_mm` for the operation (the Suggest door's own value,
///   [`crate::feeds::suggest::feeds_result_for_operation`]), rounded to
///   [`GEOMETRY_ROUNDING_STEP_MM`]. Two values within that step are one.
/// - **Depth bound for each stepover**:
///   `cutter_axial_constraints(tool, material, woc, fz, lut_row, None,
///   Some(limit_um)).safe_max_doc_mm()`. `woc` is the stepover, `fz` the
///   calculator's `chip_load_mm` at the baseline depth and that stepover,
///   `lut_row` the calculator's matched vendor row, and `limit_um` the
///   deflection gate's own bound (`tool_load::deflection::EXCEEDS_BOUND_MM`).
///   With no force line the function returns its upper bracket for
///   deflection, so the vendor `ap` rule binds.
/// - **Depth values**: [`doc_values`].
///
/// # Errors
///
/// The calculator's refusal of the baseline operation.
pub fn depth_width_plan(
    operation: &OperationConfig,
    inputs: &CellInputs<'_>,
) -> Result<DepthWidthPlan, crate::feeds::FeedsError> {
    let (Some(baseline_step), Some(total_depth), Some(baseline_stepover)) = (
        operation.depth_per_pass(),
        operation.total_depth(),
        operation.stepover(),
    ) else {
        return Ok(DepthWidthPlan::default());
    };
    let suggest = crate::feeds::suggest::feeds_result_for_operation(
        operation,
        inputs.tool,
        inputs.material,
        inputs.machine,
        crate::feeds::embedded_vendor_lut(),
        inputs.spindle_strategy,
    )?;
    let calculator_stepover = crate::feeds::suggest::round_suggestion_value(
        suggest.radial_width_mm,
        GEOMETRY_ROUNDING_STEP_MM,
    );
    let mut stepovers = vec![baseline_stepover];
    if calculator_stepover.is_finite()
        && calculator_stepover > 0.0
        && (calculator_stepover - baseline_stepover).abs() > GEOMETRY_ROUNDING_STEP_MM / 2.0
    {
        stepovers.push(calculator_stepover);
    }

    let tool_def = crate::compute::cutter::build_cutter(inputs.tool);
    let limit_um = crate::tool_load::deflection::EXCEEDS_BOUND_MM * 1000.0;
    let mut plan = DepthWidthPlan::default();
    for stepover in stepovers {
        let at_base = feeds_at_cell(operation, inputs, Some(baseline_step), Some(stepover))?;
        let bound = crate::feeds::cutter_constraints::cutter_axial_constraints(
            &tool_def,
            inputs.material,
            stepover,
            at_base.chip_load_mm,
            at_base.matched_lut_row.as_ref(),
            None,
            Some(limit_um),
        )
        .safe_max_doc_mm();
        plan.bounds.push((stepover, bound));
        for depth_per_pass_mm in doc_values(total_depth, baseline_step, bound) {
            plan.cells.push(DepthWidthCell {
                depth_per_pass_mm,
                stepover_mm: stepover,
            });
        }
    }
    Ok(plan)
}

/// Write one cell of lever L2 onto a clone of the operation.
///
/// 1. Set the depth per pass and the stepover with the operation's own
///    setters.
/// 2. Run the feeds calculator at that depth and stepover
///    ([`feeds_at_cell`]).
/// 3. Refuse the cell when the calculator moved the depth by more than
///    [`GEOMETRY_ROUNDING_STEP_MM`] (the flute guard or the slotting cap
///    did it), or when the calculator refuses the pair.
/// 4. Write the calculator's feed, plunge, RPM and ramp feed with the
///    operation's setters ([`write_calculator_speeds`]). The geometry
///    stays.
///
/// # Errors
///
/// The refusal text for the narrative.
pub fn depth_width_cell(
    operation: &OperationConfig,
    cell: DepthWidthCell,
    inputs: &CellInputs<'_>,
) -> Result<OperationConfig, String> {
    let mut candidate = operation.clone();
    if !candidate.set_depth_per_pass(cell.depth_per_pass_mm) {
        return Err(format!(
            "cell refused: the operation holds no depth per pass for {:.3} mm",
            cell.depth_per_pass_mm
        ));
    }
    if !candidate.set_stepover(cell.stepover_mm) {
        return Err(format!(
            "cell refused: the operation holds no stepover for {:.3} mm",
            cell.stepover_mm
        ));
    }
    let result = feeds_at_cell(
        &candidate,
        inputs,
        Some(cell.depth_per_pass_mm),
        Some(cell.stepover_mm),
    )
    .map_err(|e| {
        format!(
            "cell refused at depth {:.3} mm, stepover {:.3} mm: {e}",
            cell.depth_per_pass_mm, cell.stepover_mm
        )
    })?;
    if (result.axial_depth_mm - cell.depth_per_pass_mm).abs() > GEOMETRY_ROUNDING_STEP_MM {
        return Err(format!(
            "cell refused: the calculator moved the depth from {:.3} mm to {:.3} mm \
             (stepover {:.3} mm)",
            cell.depth_per_pass_mm, result.axial_depth_mm, cell.stepover_mm
        ));
    }
    if !(result.feed_rate_mm_min.is_finite() && result.feed_rate_mm_min > 0.0) {
        return Err(format!(
            "cell refused: the calculator gave no feed at depth {:.3} mm, stepover {:.3} mm",
            cell.depth_per_pass_mm, cell.stepover_mm
        ));
    }
    write_calculator_speeds(&mut candidate, &result, inputs);
    Ok(candidate)
}

/// Write the calculator's speeds for a cell onto the operation.
///
/// Each value comes from its owner:
///
/// - feed: `FeedsResult::feed_rate_mm_min`, rounded DOWN to 1 mm/min
///   ([`crate::feeds::suggest::round_suggestion_value_down`], T-9: a
///   ceiling binds the feed from above);
/// - plunge: the G10 rule at that feed ([`crate::feeds::plunge::resolve_plunge`]),
///   else `FeedsResult::plunge_rate_mm_min` rounded down;
/// - RPM: `FeedsResult::rpm`, so the RPM honours `spindle_strategy`;
/// - ramp feed: the G6 rule ([`crate::feeds::ramp::resolve_ramp_feed`])
///   at the entry the dressups emit ([`crate::feeds::ramp::entry_geometry`]).
///
/// **Why not the Suggest funnel.** `feeds::suggest::apply_speeds_to_op`
/// runs `enforce_invariants` on a scratch copy before it copies the
/// speeds back. That pass holds the rigidity clamp
/// (`clamp_dpp_to_rigidity`, the 0.25 x D rule) and pass 9
/// (`rescale_feed_to_final_geometry`), which re-derives the feed at the
/// clamped depth. On a 2.5 x D cell of a 3 mm tool the funnel shipped
/// 3877.8 mm/min against the calculator's 2423 mm/min for that depth
/// (measured 2026-10-12 by
/// `a_feed_follows_the_depth_through_the_calculator_g_opt5`, red before
/// this function). The optimizer never calls the rigidity clamp
/// (operator ruling 2026-10-12), so it writes the calculator's values.
pub fn write_calculator_speeds(
    operation: &mut OperationConfig,
    result: &FeedsResult,
    inputs: &CellInputs<'_>,
) {
    use crate::feeds::suggest::round_suggestion_value_down;
    let feed = round_suggestion_value_down(result.feed_rate_mm_min, 1.0);
    operation.set_feed_rate(feed);
    let plunge = crate::feeds::plunge::resolve_plunge(&result.plunge, feed).map_or_else(
        || round_suggestion_value_down(result.plunge_rate_mm_min, 1.0),
        |(value, _)| value,
    );
    operation.set_plunge_rate(plunge);
    if result.rpm.is_finite() && result.rpm > 0.0 {
        operation.set_spindle_rpm(Some(result.rpm.round() as u32));
    }
    let entry = crate::feeds::ramp::entry_geometry(operation, Some(inputs.dressups), inputs.tool);
    let ramp = crate::feeds::ramp::resolve_ramp_feed(
        &result.ramp,
        entry,
        operation.feed_rate(),
        operation.spindle_rpm(),
        inputs.tool.flute_count,
        operation.plunge_rate(),
    );
    let _ = operation.as_params_mut().set_ramp_feed_rate(ramp.value());
}

/// The statement on the outcome when the stock material has no force
/// line (PLAN.md §2 step 1). `None` when a force line exists.
///
/// The text: "Power and deflection are not modelled: <reason>; stock
/// source `<source_id>`". The source part is present for a
/// `Material::SolidWoodByJanka` stock only.
#[must_use]
pub fn unmodeled_note(material: &Material) -> Option<String> {
    let refusal = material.force_line().err()?;
    let reason = refusal.headline();
    Some(match material {
        Material::SolidWoodByJanka { source_id, .. } => {
            format!("Power and deflection are not modelled: {reason}; stock source `{source_id}`")
        }
        _ => format!("Power and deflection are not modelled: {reason}"),
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
    use super::*;

    /// The Z22 channel of the mini project: 22 mm at 22 / 37 mm per pass,
    /// and the vendor `ap` rule of 1 x D on a 3 mm tool.
    #[test]
    fn the_z22_staircase_takes_four_values_from_the_bound_to_the_baseline() {
        let values = doc_values(22.0, 22.0 / 37.0, 3.0);
        assert_eq!(values.len(), 4, "{values:?}");
        // n = 8, 18, 27, 37.
        let expected = [22.0 / 8.0, 22.0 / 18.0, 22.0 / 27.0, 22.0 / 37.0];
        for (got, want) in values.iter().zip(expected) {
            assert!((got - want).abs() < 1e-9, "{values:?}");
        }
    }

    #[test]
    fn a_bound_at_or_below_the_baseline_gives_the_baseline_only() {
        let values = doc_values(22.0, 2.0, 1.0);
        assert_eq!(values.len(), 1, "{values:?}");
        assert!((values[0] - 22.0 / 11.0).abs() < 1e-9);
    }

    #[test]
    fn a_short_staircase_dedups() {
        // T = 3, baseline 1.5 (n = 2), bound 3 (n = 1): two values only.
        let values = doc_values(3.0, 1.5, 3.0);
        assert_eq!(values.len(), 2, "{values:?}");
        assert!((values[0] - 3.0).abs() < 1e-9);
        assert!((values[1] - 1.5).abs() < 1e-9);
    }

    #[test]
    fn the_link_lever_skips_a_linked_baseline_and_a_stripping_operation() {
        use crate::compute::catalog::OperationType;
        let plain = DressupConfig {
            link_moves: None,
            ..DressupConfig::default()
        };
        let linked = link_dressups(&plain, OperationType::Pocket)
            .expect("a pocket with no links has a link candidate");
        assert!(linked.link_moves.is_some());
        assert!(link_dressups(&linked, OperationType::Pocket).is_none());
        assert!(
            link_dressups(&plain, OperationType::DropCutter).is_none(),
            "the DropCutter registry row strips the links"
        );
    }

    #[test]
    fn a_janka_stock_with_no_density_names_its_source() {
        let oak = Material::SolidWoodByJanka {
            janka_lbf: 1290.0,
            label: "Oak (feeds_oak.toml janka_lbf)".to_owned(),
            source_id: "rivmap feeds_oak.toml".to_owned(),
        };
        let note = unmodeled_note(&oak).expect("the rivmap oak has no force line");
        assert!(
            note.starts_with("Power and deflection are not modelled: "),
            "{note}"
        );
        assert!(
            note.ends_with("; stock source `rivmap feeds_oak.toml`"),
            "{note}"
        );
    }
}
