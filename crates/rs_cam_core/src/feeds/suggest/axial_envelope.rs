//! The axial-DOC envelope: the cutter-axial-constraints policy, the
//! per-operation envelope resolver, the envelope picker that reconciles a
//! commanded DPP with the vendor band, and the chipload-bound recompute that
//! follows a DPP rewrite.
//!
//! Split out of `feeds/suggest.rs` by P4; every item is unchanged apart from
//! its visibility and the `use` lines.

use crate::compute::catalog::OperationConfig;
use crate::compute::tool_config::ToolConfig;
use crate::feeds::{OperationFamily as FeedsOperationFamily, ToolGeometryHint};
use crate::material::Material;

use super::{SuggestContext, SuggestWarning};

/// Apply policy C of `planning/cutter_axial_constraints_2026-06-06.md`
/// §5.4 to a single axial value against the cutter-axial-constraints
/// envelope.
///
/// - Value > `safe_max` → clamp down + emit `AxialDocClampedByEnvelope`.
/// - Value < chipload-burn floor → leave value + emit `AxialDocBelowBurnFloor`
///   (deliberate conservative pin respected).
/// - SafeBandEmpty → emit `AxialEnvelopeSafeBandEmpty` and clamp to
///   `safe_max` (the upper bound; lower bound is infeasible so further
///   conservatism is pointless).
/// - In-band → leave the value silently.
///
/// Returns the post-policy value plus the list of warnings to surface.
fn apply_axial_envelope(
    commanded_mm: f64,
    envelope: &crate::feeds::cutter_constraints::CutterAxialConstraints,
    op_kind: &'static str,
    param_name: &'static str,
) -> (f64, Vec<SuggestWarning>) {
    use crate::feeds::cutter_constraints::AxialBindingConstraint;
    let safe_max = envelope.safe_max_doc_mm();
    let mut warnings = Vec::new();
    let mut new_val = commanded_mm;

    let binding_str = axial_binding_str(envelope.binding_constraint);

    if matches!(
        envelope.binding_constraint,
        AxialBindingConstraint::SafeBandEmpty
    ) {
        let chipload_floor = envelope.min_doc_chipload_floor_mm.unwrap_or(0.0);
        // SafeBandEmpty implies one of the upper bounds (Deflection / VendorAp /
        // Scallop) is tighter than the floor; surface whichever is.
        let safe_max_now = envelope.safe_max_doc_mm();
        let inner_binding = if envelope
            .max_doc_vendor_mm
            .is_some_and(|v| (v - safe_max_now).abs() < 1.0e-9)
        {
            "vendor_ap"
        } else if envelope
            .max_doc_scallop_mm
            .is_some_and(|v| (v - safe_max_now).abs() < 1.0e-9)
        {
            "scallop"
        } else {
            "deflection"
        };
        warnings.push(SuggestWarning::AxialEnvelopeSafeBandEmpty {
            op_kind,
            max_safe_doc_mm: safe_max,
            chipload_floor_doc_mm: chipload_floor,
            binding_upper: inner_binding,
        });
        if commanded_mm > safe_max && safe_max > 0.0 {
            new_val = safe_max;
            // No additional clamp warning — `AxialEnvelopeSafeBandEmpty`
            // already names the situation.
        }
        return (new_val, warnings);
    }

    if commanded_mm > safe_max && safe_max > 0.0 {
        new_val = safe_max;
        warnings.push(SuggestWarning::AxialDocClampedByEnvelope {
            op_kind,
            param_name,
            commanded_mm,
            clamped_mm: safe_max,
            binding: binding_str,
        });
        return (new_val, warnings);
    }
    if let Some(floor) = envelope.min_doc_chipload_floor_mm
        && commanded_mm < floor
        && floor > 0.0
    {
        warnings.push(SuggestWarning::AxialDocBelowBurnFloor {
            op_kind,
            param_name,
            commanded_mm,
            floor_mm: floor,
        });
    }
    (new_val, warnings)
}

/// Stable string token for an [`AxialBindingConstraint`] — used in
/// `SuggestWarning` payloads (and from there the rationale tree), so
/// the mapping is pinned in one place.
fn axial_binding_str(
    binding: crate::feeds::cutter_constraints::AxialBindingConstraint,
) -> &'static str {
    use crate::feeds::cutter_constraints::AxialBindingConstraint;
    match binding {
        AxialBindingConstraint::Deflection => "deflection",
        AxialBindingConstraint::VendorAp => "vendor_ap",
        AxialBindingConstraint::Scallop => "scallop",
        AxialBindingConstraint::SafeBandEmpty => "safe_band_empty",
    }
}

/// Construct the axial-DOC constraint envelope for an operation at its
/// current operating point (feed / RPM / stepover as configured), or
/// `None` for ops outside the envelope routing.
///
/// This is the env-construction half of [`pick_axial_envelope`] —
/// extracted so [`crate::feeds::profile::CutterOpProfile`] surfaces the
/// SAME envelope Suggest's pass 0 enforces, with per-op radial-WOC /
/// finish-target / deflection-limit derivation in exactly one place.
///
/// Routing (mirrors planning §5):
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

    let tool_def = build_cutter(tool);
    let radial = operation.stepover();
    let feed = operation.feed_rate();
    let rpm = operation.spindle_rpm().unwrap_or(0) as f64;
    let flute_count = tool.flute_count.max(1) as f64;
    let feed_per_tooth_mm = if rpm > 0.0 {
        feed / (rpm * flute_count)
    } else {
        0.0
    };

    match operation {
        OperationConfig::Adaptive3d(_) => {
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
        OperationConfig::VCarve(cfg) => {
            // V-bit radial WOC at the commanded depth is the engaged
            // width along the surface — `vbit_width_at_depth`. We use
            // half the engaged width as the lateral WOC for the force
            // model (triangular groove averages ½ width).
            let geometry = tool_def.to_geometry_hint();
            let radial_woc = match geometry {
                ToolGeometryHint::VBit {
                    included_angle,
                    tip_diameter,
                } => {
                    let w = crate::feeds::geometry::vbit_width_at_depth(
                        included_angle,
                        tip_diameter,
                        cfg.max_depth,
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
        OperationConfig::ProjectCurve(_) => {
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
        OperationConfig::Scallop(_)
        | OperationConfig::UnifiedFinish(_)
        | OperationConfig::DropCutter(_)
        | OperationConfig::Waterline(_)
        | OperationConfig::SteepShallow(_)
        | OperationConfig::SpiralFinish(_)
        | OperationConfig::RadialFinish(_)
        | OperationConfig::HorizontalFinish(_) => {
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
        // Non-axial-envelope ops (2D pocket, contour, drill, etc.) —
        // the envelope adds nothing the existing passes don't already
        // cover (rigidity / cutting-length / deflection backoff).
        _ => None,
    }
}

/// Pass 0 of [`enforce_invariants`] — unified axial-DOC envelope per
/// `planning/cutter_axial_constraints_2026-06-06.md` §5.
///
/// Envelope construction (per-op radial WOC / target / limit routing)
/// lives in [`axial_envelope_for_operation`]; this pass applies the
/// policy to the operation:
/// - `Adaptive3d` — picks `depth_per_pass` via policy C against the
///   envelope. Mutates DPP when the commanded value is above
///   `safe_max_doc_mm`. Each coarse step of the step ladder above
///   `safe_max_doc_mm` moves down to it and gets its own
///   `AxialDocClampedByEnvelope` (`param_name` `"coarse step depth"`,
///   `commanded_mm` = the step). Then the ladder is made valid again: a
///   coarse step that is not above `depth_per_pass` goes, and so does a
///   second copy of one step. Each step that goes gets a
///   `CoarseStepRemoved` note.
/// - `VCarve` — clamps `cfg.max_depth` via policy C. The V-bit
///   engaged width at the candidate depth is the radial WOC.
/// - `ProjectCurve` — warning-only feasibility check on `cfg.depth`.
/// - Finish-3D (Scallop / UnifiedFinish / DropCutter / Waterline /
///   SteepShallow / SpiralFinish / RadialFinish / HorizontalFinish) —
///   emits `FinishEnvelopeAdvisory`; automatic `stock_to_leave` mutation is
///   deferred until in-process stock at gen time lands (planning
///   §5.1.1).
///
/// Returns `(warnings, dpp_mutated)` so `enforce_invariants` can run
/// the chipload-bounds re-derivation step in §5.3 only when needed.
pub(super) fn pick_axial_envelope(
    operation: &mut OperationConfig,
    tool: &ToolConfig,
    material: &Material,
    context: SuggestContext<'_>,
) -> (Vec<SuggestWarning>, bool) {
    let mut warnings = Vec::new();
    let mut dpp_mutated = false;

    let Some(env) =
        axial_envelope_for_operation(operation, tool, material, context.matched_lut_row)
    else {
        return (warnings, dpp_mutated);
    };

    match operation {
        OperationConfig::Adaptive3d(cfg) => {
            let commanded = cfg.depth_per_pass;
            let (new_dpp, ws) =
                apply_axial_envelope(commanded, &env, "adaptive3d", "depth_per_pass");
            warnings.extend(ws);
            if (new_dpp - commanded).abs() > 1.0e-6 {
                cfg.depth_per_pass = new_dpp;
                dpp_mutated = true;
            }
            // The step ladder (D7): every step must be at or below the cap,
            // not only the base step. Each coarse step above the cap moves
            // down to the cap, and gets its own clamp record. The record
            // names the step by its commanded value. The warnings that
            // `apply_axial_envelope` gives once per envelope (the empty
            // safe band, the burn floor) are not given again per step: a
            // coarse step is deeper than the base step, so the burn floor
            // cannot apply to it when it does not apply to the base step.
            // With an empty safe band the steps clamp with no clamp record,
            // as the base step does: `AxialEnvelopeSafeBandEmpty` already
            // names the cap, and a clamp record never carries that binding.
            let safe_max = env.safe_max_doc_mm();
            if safe_max > 0.0 {
                let band_empty = matches!(
                    env.binding_constraint,
                    crate::feeds::cutter_constraints::AxialBindingConstraint::SafeBandEmpty
                );
                let binding = axial_binding_str(env.binding_constraint);
                let capped = super::ladder::cap_coarse_steps_of(cfg, safe_max);
                for &(from, to) in &capped.moved {
                    if !band_empty {
                        warnings.push(SuggestWarning::AxialDocClampedByEnvelope {
                            op_kind: "adaptive3d",
                            param_name: "coarse step depth",
                            commanded_mm: from,
                            clamped_mm: to,
                            binding,
                        });
                    }
                    dpp_mutated = true;
                }
                // A capped step that is no longer above the next step goes,
                // with its note (operator ruling 1, 2026-09-24). The note is
                // filed also with an empty safe band: a removed step always
                // has a note.
                warnings.extend(super::ladder::removal_notes(
                    &capped.removed,
                    "axial envelope",
                ));
            }
            // Keep the ladder valid even when no step moved here. With a
            // ladder the apply funnel keeps the operator's base step (ruling
            // 1), so this removes a step only when the operator's own ladder
            // breaks the rule. The adapter refuses such a ladder; the note
            // says why the step went.
            let invalid = super::ladder::normalize_ladder_of(cfg);
            warnings.extend(super::ladder::removal_notes(&invalid, "ladder rule"));
        }
        OperationConfig::VCarve(cfg) => {
            let commanded = cfg.max_depth;
            let (new_depth, ws) = apply_axial_envelope(commanded, &env, "vcarve", "max_depth");
            warnings.extend(ws);
            if (new_depth - commanded).abs() > 1.0e-6 {
                cfg.max_depth = new_depth;
                // VCarve `max_depth` is not the chipload-DPP — re-derivation
                // only matters for ops where DPP feeds back into the chip
                // geometry calculator. Leave `dpp_mutated` false here.
            }
        }
        OperationConfig::ProjectCurve(cfg) => {
            let safe_max = env.safe_max_doc_mm();
            if cfg.depth > safe_max && safe_max > 0.0 {
                warnings.push(SuggestWarning::ProjectCurveDepthInfeasible {
                    commanded_mm: cfg.depth,
                    max_safe_mm: safe_max,
                    binding: axial_binding_str(env.binding_constraint),
                });
            }
        }
        // Finish-3D family — warning-only per planning §5.1.1.
        OperationConfig::Scallop(_)
        | OperationConfig::UnifiedFinish(_)
        | OperationConfig::DropCutter(_)
        | OperationConfig::Waterline(_)
        | OperationConfig::SteepShallow(_)
        | OperationConfig::SpiralFinish(_)
        | OperationConfig::RadialFinish(_)
        | OperationConfig::HorizontalFinish(_) => {
            let op_kind = match operation {
                OperationConfig::Scallop(_) => "scallop",
                OperationConfig::UnifiedFinish(_) => "unified_finish",
                OperationConfig::DropCutter(_) => "drop_cutter",
                OperationConfig::Waterline(_) => "waterline",
                OperationConfig::SteepShallow(_) => "steep_shallow",
                OperationConfig::SpiralFinish(_) => "spiral_finish",
                OperationConfig::RadialFinish(_) => "radial_finish",
                OperationConfig::HorizontalFinish(_) => "horizontal_finish",
                _ => "finish_3d",
            };
            let binding_str = axial_binding_str(env.binding_constraint);
            // Only emit the advisory when the envelope actually carries a
            // meaningful bound (safe max under 5×D is the sniff test —
            // anything looser is "no binding" noise).
            if env.safe_max_doc_mm() < tool.diameter * 5.0 {
                warnings.push(SuggestWarning::FinishEnvelopeAdvisory {
                    op_kind,
                    max_safe_doc_mm: env.safe_max_doc_mm(),
                    binding: binding_str,
                });
            }
            if matches!(env.safe_band_is_empty(), Some(true)) {
                warnings.push(SuggestWarning::AxialEnvelopeSafeBandEmpty {
                    op_kind,
                    max_safe_doc_mm: env.safe_max_doc_mm(),
                    chipload_floor_doc_mm: env.min_doc_chipload_floor_mm.unwrap_or(0.0),
                    binding_upper: binding_str,
                });
            }
        }
        // Unreachable in practice: `axial_envelope_for_operation`
        // returns `None` for every other variant, so we never get here
        // with an envelope. Kept as an explicit no-op rather than
        // `unreachable!` so routing changes fail soft.
        _ => {}
    }

    (warnings, dpp_mutated)
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
