//! The axial-DOC envelope: the per-operation envelope resolver and the
//! chipload-bound recompute at a depth that is not the calculator's. Feeds
//! contract P5 deleted the policy and the picker (pass 0).
//!
//! Split out of `feeds/suggest.rs` by P4; every item is unchanged apart from
//! its visibility and the `use` lines.

use crate::compute::catalog::{AxialEnvelope, OperationConfig};
use crate::compute::tool_config::ToolConfig;
use crate::feeds::{OperationFamily as FeedsOperationFamily, ToolGeometryHint};
use crate::material::Material;

/// Construct the axial-DOC constraint envelope for an operation at its
/// current operating point (feed / RPM / stepover as configured), or
/// `None` for ops outside the envelope routing.
///
/// [`crate::feeds::profile::CutterOpProfile`] surfaces this envelope.
/// Suggest's pass 0 (`pick_axial_envelope`) applied it to the op until feeds
/// contract P5 deleted the pass; the safe envelope (`feeds::envelope`) holds
/// its limits now.
///
/// Routing (mirrors planning §5). W2.4: the registry row states each op's
/// class (`Capabilities::axial_envelope`):
/// - `Adaptive3d` — rough limit, radial WOC = stepover (default 0.4·D).
/// - `VCarve` — rough limit, radial WOC = ½ engaged width at
///   `max_depth` ([`crate::feeds::geometry::vbit_width_at_depth`]).
/// - `ProjectCurve` — rough limit, radial WOC = 0.2·D.
/// - Finish-3D (Scallop / UnifiedFinish / DropCutter / Waterline /
///   SteepShallow / SpiralFinish / RadialFinish / HorizontalFinish) —
///   finish limit + default scallop target, radial WOC = stepover
///   (default 0.15·D).
/// - Everything else — `None` (2D pocket/contour/drill etc.; the
///   envelope adds nothing the other invariant passes don't cover).
pub(crate) fn axial_envelope_for_operation(
    operation: &OperationConfig,
    tool: &ToolConfig,
    material: &Material,
    matched_lut_row: Option<&crate::feeds::vendor_lookup::LookupResult>,
) -> Option<crate::feeds::cutter_constraints::CutterAxialConstraints> {
    use crate::compute::cutter::build_cutter;
    use crate::feeds::cutter_constraints::{
        DEFAULT_FINISH_DEFLECTION_LIMIT_UM, DEFAULT_ROUGH_DEFLECTION_LIMIT_UM,
        DEFAULT_SCALLOP_TARGET_UM, cutter_axial_constraints,
    };

    // F029: a tool outside its domain has no envelope.
    let tool_def = build_cutter(tool).ok()?;
    let radial = operation.stepover();
    let rpm = operation.spindle_rpm().unwrap_or(0) as f64;
    let flute_count = tool.flute_count_planning().max(1) as f64;
    // S11: an unset feed gives no chip (0, as no RPM does).
    let feed_per_tooth_mm = match operation.feed_rate() {
        Some(feed) if rpm > 0.0 => feed / (rpm * flute_count),
        Some(_) | None => 0.0,
    };

    // W2.4: the registry row states the class; the WOC heuristics below
    // stay here.
    match operation.capabilities().axial_envelope {
        AxialEnvelope::NotApplied => None,
        AxialEnvelope::Roughing3d => {
            let radial_woc = radial.unwrap_or(tool.diameter * 0.4).max(1.0e-3);
            Some(cutter_axial_constraints(
                &tool_def,
                material,
                radial_woc,
                feed_per_tooth_mm,
                matched_lut_row,
                None,
                Some(DEFAULT_ROUGH_DEFLECTION_LIMIT_UM),
            ))
        }
        AxialEnvelope::VBitGroove => {
            // V-bit radial WOC at the commanded depth is the engaged
            // width along the surface — `vbit_width_at_depth`. We use
            // half the engaged width as the lateral WOC for the force
            // model (triangular groove averages ½ width). The commanded
            // depth is the op's imposed axial depth (VCarve: `max_depth`).
            let geometry = tool_def.to_geometry_hint();
            let depth = super::operation_feeds_hints(operation, tool.diameter).0;
            let radial_woc = match (geometry, depth) {
                (
                    ToolGeometryHint::VBit {
                        included_angle,
                        tip_diameter,
                    },
                    Some(depth),
                ) => {
                    let w = crate::feeds::geometry::vbit_width_at_depth(
                        included_angle,
                        tip_diameter,
                        depth,
                    )
                    .unwrap_or(tool.diameter * 0.2);
                    (w * 0.5).max(1.0e-3)
                }
                _ => (tool.diameter * 0.2).max(1.0e-3),
            };
            Some(cutter_axial_constraints(
                &tool_def,
                material,
                radial_woc,
                feed_per_tooth_mm,
                matched_lut_row,
                None,
                Some(DEFAULT_ROUGH_DEFLECTION_LIMIT_UM),
            ))
        }
        AxialEnvelope::CurveRough => {
            let radial_woc = (tool.diameter * 0.2).max(1.0e-3);
            Some(cutter_axial_constraints(
                &tool_def,
                material,
                radial_woc,
                feed_per_tooth_mm,
                matched_lut_row,
                None,
                Some(DEFAULT_ROUGH_DEFLECTION_LIMIT_UM),
            ))
        }
        // Finish-3D family — finish deflection limit + scallop target.
        AxialEnvelope::Finish3d => {
            let radial_woc = radial.unwrap_or(tool.diameter * 0.15).max(1.0e-3);
            Some(cutter_axial_constraints(
                &tool_def,
                material,
                radial_woc,
                feed_per_tooth_mm,
                matched_lut_row,
                Some(DEFAULT_SCALLOP_TARGET_UM),
                Some(DEFAULT_FINISH_DEFLECTION_LIMIT_UM),
            ))
        }
    }
}

/// Re-derive `chipload_bounds` after the axial-envelope pass mutated
/// DPP. Uses the same shared helper as `feeds::calculate`'s in-place
/// derivation (`geometry::derate_chipload_bounds`, S.8 — see
/// `planning/finishing_stack_review_2026-07.md`) so the post-mutation
/// chipload-recalibration pass sees a bounds value consistent with the
/// new DPP / effective-D ratio. No-op when the matched LUT row is
/// absent or carries no chipload band (the original derivation would
/// also be `None`).
pub(super) fn recompute_chipload_bounds_for_dpp(
    matched_row: Option<&crate::feeds::vendor_lookup::LookupResult>,
    effective_diameter_mm: f64,
    operation: &OperationConfig,
    new_dpp_mm: f64,
) -> Option<crate::feeds::ChiploadBounds> {
    let row = matched_row?;
    // Drill ops are excluded from doc-derating per `feeds::calculate`
    // (same path), so leave bounds at the raw LUT values for them —
    // forcing `doc_ratio` to `0.0` bypasses derating since
    // `doc_derating_scale` maps any ratio `<= 1.0` to a scale of
    // `1.0`.
    let op_family = operation.feeds_style().0;
    let is_drill = matches!(op_family, FeedsOperationFamily::Drill);
    let doc_ratio = if is_drill || effective_diameter_mm <= 0.0 {
        0.0
    } else {
        new_dpp_mm / effective_diameter_mm
    };
    let (min, max) = crate::feeds::geometry::derate_chipload_bounds(
        row.chip_load_min_mm,
        row.chip_load_max_mm,
        doc_ratio,
        crate::feeds::geometry::ChiploadBoundPolicy::RequireBoth,
    )?
    .into_pair()?;
    Some(crate::feeds::ChiploadBounds {
        min_mm_per_tooth: min,
        max_mm_per_tooth: max,
    })
}

/// The printed point of the matched row at a new DPP (A2, point mode).
///
/// This is the point twin of [`recompute_chipload_bounds_for_dpp`]: the
/// same row, the same DOC ratio and the same de-rate. It reads the row
/// (`SuggestContext::matched_lut_row`), so no `SuggestContext` field is
/// added. `None` when the row is absent or prints a band or no chipload.
pub(super) fn chip_point_for_dpp(
    matched_row: Option<&crate::feeds::vendor_lookup::LookupResult>,
    effective_diameter_mm: f64,
    operation: &OperationConfig,
    new_dpp_mm: f64,
) -> Option<f64> {
    let point = matched_row?.printed_chipload().point_mm()?;
    // Drill ops take no DOC de-rate, as in the band twin above.
    let op_family = operation.feeds_style().0;
    let is_drill = matches!(op_family, FeedsOperationFamily::Drill);
    let doc_ratio = if is_drill || effective_diameter_mm <= 0.0 {
        0.0
    } else {
        new_dpp_mm / effective_diameter_mm
    };
    crate::feeds::geometry::derate_chipload_bounds(
        None,
        Some(point),
        doc_ratio,
        crate::feeds::geometry::ChiploadBoundPolicy::AllowHalfBand,
    )
    .map(|b| b.max_mm_per_tooth)
}
