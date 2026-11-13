//! Steps 3 to 4c of the calculator: the depth and the width of the cut.

use super::{Cut, DepthClamps};
use crate::feeds::sourced::Basis;
use crate::feeds::{
    FeedsInput, FeedsWarning, MIN_AE_MM, MIN_AE_MM_ROW, MIN_AP_MM, MIN_AP_MM_ROW, ToolGeometryHint,
    default_engagement, default_profile_of, flute_guard_of, geometry, moved_basis,
};

/// Step 3: the depth and the width from the operation defaults, Step 3b
/// (the scallop stepover of a ball or a tapered ball), and the operator's
/// values.
pub(super) fn step3_engagement(input: &FeedsInput, warnings: &mut Vec<FeedsWarning>) -> Cut {
    let d = input.tool_diameter;
    // --- Step 3: DOC/WOC from operation defaults ---
    let profile = default_profile_of(input);
    let (mut ap, mut ae, mut ap_basis, mut ae_basis) =
        default_engagement(d, &profile, input, input.machine);

    // --- Step 3b: Scallop-driven stepover for ball/tapered ball ---
    if let Some(target_scallop) = input.target_scallop_mm {
        // A bull nose forms its scallop with its corner radius (EVIDENCE
        // 2-7): the registry refuses Bull on the Scallop family, so this
        // arm serves the Parallel family's scallop hint.
        let ball_r = match input.tool_geometry {
            ToolGeometryHint::Ball => d / 2.0,
            ToolGeometryHint::TaperedBall { tip_radius, .. } => tip_radius,
            ToolGeometryHint::Bull { corner_radius } => corner_radius,
            ToolGeometryHint::Flat | ToolGeometryHint::VBit { .. } => 0.0,
        };
        if ball_r > 0.0 {
            if let Some(stepover) = geometry::scallop_stepover(ball_r, target_scallop) {
                ae = stepover;
                let radius_fact = match input.tool_geometry {
                    ToolGeometryHint::Bull { .. } => Basis::tool_fact(
                        crate::feeds::sourced::ToolFactName::CornerRadius,
                        crate::compute::tool_config::FactKind::Stated,
                    ),
                    ToolGeometryHint::Ball
                    | ToolGeometryHint::TaperedBall { .. }
                    | ToolGeometryHint::Flat
                    | ToolGeometryHint::VBit { .. } => Basis::tool_diameter(),
                };
                ae_basis = Basis::derived("scallop stepover", vec![Basis::stated(), radius_fact]);
            } else {
                warnings.push(FeedsWarning::ScallopInvalid {
                    target: target_scallop,
                    max_possible: ball_r,
                });
            }
        }
    }

    // Apply user overrides
    if let Some(user_ap) = input.axial_depth_mm {
        ap = user_ap;
        ap_basis = Basis::stated();
    }
    if let Some(user_ae) = input.radial_width_mm {
        ae = user_ae;
        ae_basis = Basis::stated();
    }
    // P16 (review D7): a surface-offset op has no depth before the
    // simulation, so no family default depth applies. The chip-thinning
    // diameter reads the tool diameter, the same rule as the SFM diameter
    // of Step 1 on an unstated depth. No power figure reads this value
    // (`FeedsInput::power_force_line`).
    if input.depth_after_simulation() {
        ap = d;
        ap_basis = Basis::derived(DEPTH_AFTER_SIMULATION_LAW, vec![Basis::tool_diameter()]);
        warnings.push(FeedsWarning::DepthJudgedAfterSimulation);
    }
    Cut {
        ap,
        ae,
        ap_basis,
        ae_basis,
    }
}

/// P16: the law of the depth of a surface-offset op (Project Curve).
pub(crate) const DEPTH_AFTER_SIMULATION_LAW: &str =
    "depth known after the simulation; the chip-thinning diameter reads the tool diameter";

/// Step 4: the flute guard.
pub(super) fn step4_flute_guard(
    input: &FeedsInput,
    clamps: DepthClamps,
    cut: &mut Cut,
    warnings: &mut Vec<FeedsWarning>,
) {
    // --- Step 4: Flute guard ---
    //
    // Feeds contract P11b (the P11a lead item): a STATED depth ships, so the
    // feed and the power are sized at that depth and the warning names the
    // guard (ruling Q6: warn, never refuse). Only a depth that the
    // calculator chose (the family default) is lowered to the guard.
    // P16: a depth known after the simulation gives the guard nothing to
    // judge; the post-sim gates judge it.
    if input.depth_after_simulation() {
        return;
    }
    let depth_is_stated = input.axial_depth_mm.is_some();
    let (flute_guard, flute_guard_basis) = flute_guard_of(
        input.flute_length,
        input.tool_facts.cutting_length,
        input.tool_diameter,
    );
    if cut.ap > flute_guard && clamps == DepthClamps::Apply {
        let requested = cut.ap;
        if !depth_is_stated {
            cut.ap = flute_guard;
            cut.ap_basis =
                Basis::derived("flute guard", vec![cut.ap_basis.clone(), flute_guard_basis]);
        }
        warnings.push(FeedsWarning::DocExceedsFlute {
            requested_mm: requested,
            guard_mm: flute_guard,
            shipped_mm: cut.ap,
        });
    }
}

/// The engagement floor (`MIN_AP_MM`, `MIN_AE_MM`) and the width cap at
/// the tool diameter.
pub(super) fn engagement_floor(input: &FeedsInput, cut: &mut Cut) {
    // Ensure minimum engagement
    let ap_before = cut.ap;
    cut.ap = cut.ap.max(MIN_AP_MM);
    cut.ap_basis = moved_basis(
        cut.ap_basis.clone(),
        ap_before,
        cut.ap,
        "engagement floor",
        Basis::Rule(MIN_AP_MM_ROW),
    );
    let ae_before = cut.ae;
    cut.ae = cut.ae.max(MIN_AE_MM);
    cut.ae_basis = moved_basis(
        cut.ae_basis.clone(),
        ae_before,
        cut.ae,
        "engagement floor",
        Basis::Rule(MIN_AE_MM_ROW),
    );
    // Cap ae to tool diameter
    let ae_before = cut.ae;
    cut.ae = cut.ae.min(input.tool_diameter);
    cut.ae_basis = moved_basis(
        cut.ae_basis.clone(),
        ae_before,
        cut.ae,
        "tool diameter",
        Basis::tool_diameter(),
    );
}

/// Step 4c: the shank check.
pub(super) fn step4c_shank(input: &FeedsInput, warnings: &mut Vec<FeedsWarning>) {
    // --- Step 4c: Shank check ---
    if let Some(shank) = input.shank_diameter
        && shank > input.machine.max_shank_mm
    {
        warnings.push(FeedsWarning::ShankTooLarge {
            shank_mm: shank,
            max_mm: input.machine.max_shank_mm,
        });
    }
}
