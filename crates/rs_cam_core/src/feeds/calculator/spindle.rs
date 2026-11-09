//! Feeds contract P21a: the one RPM-choice door. Steps 2b and 2b' of the
//! calculator, and the named limits of the RPM they choose
//! ([`crate::feeds::RpmChoice`]).

use super::{Chip, Speed, StepLog};
use crate::feeds::sourced::Basis;
use crate::feeds::{
    FeedsInput, MAX_SPINDLE_SPEEDUP, MAX_SPINDLE_SPEEDUP_ROW, OperationFamily, RpmChoice, RpmLimit,
    RpmLimitValue, SPINDLE_CEILING_HEADROOM, SpindleScaleReason, SpindleStrategy, ToolGeometryHint,
    milling_rpm_ceiling_for_diameter, milling_rpm_ceiling_row, moved_basis,
};

/// Steps 2b and 2b': the RPM that [`SpindleStrategy::ConstantChip`] chooses,
/// with its named limits. `None` under [`SpindleStrategy::MatchChart`],
/// where both steps change nothing.
///
/// The two steps run in their old order with their old arithmetic, and
/// each writes its own step record, so no shipped value moves (P21a is
/// byte-identical). The limits are read beside them: the minimum of the
/// limits is the RPM of the steps before the machine clamp
/// (`MachineProfile::clamp_rpm`).
pub(super) fn rpm_choice(
    input: &FeedsInput,
    chip: &Chip,
    speed: &mut Speed,
    log: &mut StepLog,
) -> Option<RpmChoice> {
    let limits = named_limits(input, chip, speed.rpm);
    step2b_max_speed(input, chip, speed);
    log.speed("step2b_max_speed", speed);
    step2b_milling_tier(input, speed);
    log.speed("step2b_milling_tier", speed);
    let limits = limits?;
    let binding = limits
        .iter()
        .min_by(|a, b| a.rpm.total_cmp(&b.rpm))
        .map_or(RpmLimit::LegacySpeedup, |l| l.limit);
    Some(RpmChoice {
        rpm: speed.rpm,
        binding,
        limits,
    })
}

/// The named limits of Steps 2b and 2b' at the chart RPM `chart_rpm` (the
/// RPM after Step 2), each with the highest RPM it admits. `None` under
/// [`SpindleStrategy::MatchChart`].
///
/// Step 2b only lifts the RPM: it climbs to `min(ceiling, chart x 1.5)`
/// when the ceiling is above the chart RPM, and else it keeps the chart
/// RPM. So a cap of Step 2b admits the larger of the cap and the chart
/// RPM. Step 2b' lowers the RPM to the tier on a milling op with a
/// nominal-diameter tool, so there the tier admits the tier itself.
fn named_limits(input: &FeedsInput, chip: &Chip, chart_rpm: f64) -> Option<Vec<RpmLimitValue>> {
    match input.spindle_strategy {
        SpindleStrategy::MatchChart => return None,
        SpindleStrategy::ConstantChip { .. } => {}
    }
    let (_, machine_max_rpm) = input.machine.rpm_range();
    let mut limits = vec![
        RpmLimitValue {
            limit: RpmLimit::LegacySpeedup,
            rpm: chart_rpm * MAX_SPINDLE_SPEEDUP,
        },
        RpmLimitValue {
            limit: RpmLimit::LegacyHeadroom,
            rpm: (machine_max_rpm * SPINDLE_CEILING_HEADROOM).max(chart_rpm),
        },
    ];
    if let Some(vm) = chip.vendor_rpm_max
        && vm.is_finite()
        && vm > 0.0
    {
        limits.push(RpmLimitValue {
            limit: RpmLimit::VendorRpmMax,
            rpm: vm.max(chart_rpm),
        });
    }
    let geom_uses_nominal_d = !matches!(
        input.tool_geometry,
        ToolGeometryHint::VBit { .. } | ToolGeometryHint::TaperedBall { .. }
    );
    if geom_uses_nominal_d {
        let tier = milling_rpm_ceiling_for_diameter(input.tool_diameter);
        let rpm = if input.operation == OperationFamily::Drill {
            tier.max(chart_rpm)
        } else {
            tier
        };
        limits.push(RpmLimitValue {
            limit: RpmLimit::LegacyTier,
            rpm,
        });
    }
    Some(limits)
}

/// Step 2b: the spindle speed-up along the constant-chipload line
/// (`SpindleStrategy::ConstantChip`).
fn step2b_max_speed(input: &FeedsInput, chip: &Chip, speed: &mut Speed) {
    let machine = input.machine;
    let d = input.tool_diameter;
    // --- Step 2b: Spindle-speedup along the constant-chipload line ---
    //
    // Under `SpindleStrategy::ConstantChip` push RPM up toward the
    // spindle ceiling (clamped by vendor.rpm_max when published,
    // capped by MAX_SPINDLE_SPEEDUP and a small safety headroom).
    // Feed scales proportionally below (the feed formula already
    // multiplies by rpm), so chipload is preserved. Power and feed-
    // cap derates apply on top — if the higher operating point
    // exceeds spindle power the existing `power_limit` claws feed
    // back, and the modal renders the resulting binding constraint.
    //
    // `spindle_scale` is captured into `FeedsDerates` for the modal.
    // Default (MatchChart) leaves it at 1.0; smoke baselines unchanged.
    if matches!(input.spindle_strategy, SpindleStrategy::ConstantChip { .. }) && speed.rpm > 0.0 {
        let (_, machine_max_rpm) = machine.rpm_range();
        let machine_ceiling = machine_max_rpm * SPINDLE_CEILING_HEADROOM;
        // Diameter-tier literature ceiling for milling. The MaxSpeed
        // speedup pre-2026-06-03 used machine_ceiling unconditionally,
        // pushing a 12 mm hardwood adaptive2d cut to 22.8 k RPM —
        // ~63% above the Onsrud/Amana 14 k literature ceiling
        // (literature-matrix cell flat_12mm_adaptive2d_oak_power).
        //
        // The tier ceiling always applies for nominal-D == cutting-D
        // geometries (Flat / Ball / Bull) — vendor `rpm_max` on the
        // matched LUT row is treated as a per-row safety ceiling
        // (often inherited from a small-diameter anchor row and not
        // re-scaled by diameter), so it isn't a reliable per-diameter
        // bound. We `.min()` all three: vendor rpm_max (when present),
        // the machine safety headroom, and the diameter-tier
        // literature ceiling. The lowest defensible cap wins.
        //
        // V-bit and tapered-ball geometries cut on the **engaged-D**
        // circle, not nominal D — the tier table is calibrated on the
        // cutting-D ≈ nominal-D assumption, so clamping nominal-D
        // would force a 6.35 mm V-bit down to the 18 k tier even when
        // engaged-D is ~1 mm and the engaged-D SFM math wants near-
        // ceiling RPM (literature-matrix cell
        // vbit_60deg_vcarve_oak_micro_depth).
        let geom_uses_nominal_d = !matches!(
            input.tool_geometry,
            ToolGeometryHint::VBit { .. } | ToolGeometryHint::TaperedBall { .. }
        );
        let diameter_tier_ceiling = if geom_uses_nominal_d {
            milling_rpm_ceiling_for_diameter(d)
        } else {
            f64::INFINITY
        };
        let ceiling = match chip.vendor_rpm_max {
            Some(vm) if vm.is_finite() && vm > 0.0 => {
                vm.min(machine_ceiling).min(diameter_tier_ceiling)
            }
            _ => machine_ceiling.min(diameter_tier_ceiling),
        };
        if ceiling > speed.rpm {
            let raw_speedup = ceiling / speed.rpm;
            speed.spindle_scale = raw_speedup.min(MAX_SPINDLE_SPEEDUP);
            speed.spindle_scale_reason = SpindleScaleReason::MaxSpeedPolicy;
            let before = speed.rpm;
            speed.rpm = machine.clamp_rpm(speed.rpm * speed.spindle_scale);
            speed.basis = moved_basis(
                speed.basis.clone(),
                before,
                speed.rpm,
                "max speed policy",
                Basis::Rule(MAX_SPINDLE_SPEEDUP_ROW),
            );
        }
    }
}

/// Step 2b': the diameter-tier ceiling for milling under `ConstantChip`.
fn step2b_milling_tier(input: &FeedsInput, speed: &mut Speed) {
    // --- Step 2b': Diameter-tier ceiling for milling under ConstantChip ---
    //
    // The vendor LUT can publish `rpm_nominal` above the diameter-tier
    // literature ceiling (e.g. a 12 mm hardwood adaptive2d row anchored
    // on a 6 mm chart with vendor_rpm=18 000). Step 2's vendor-override
    // path (`rpm = machine.clamp_rpm(v_rpm)`) then carries that value
    // forward, and the Step 2b speedup branch is a no-op because the
    // tier ceiling is already below `rpm`. Without this final clamp the
    // engine emits the vendor-anchor RPM verbatim — overshooting the
    // literature band whenever the vendor row outpaces the per-diameter
    // wood-cutting envelope.
    //
    // Apply the tier ceiling as a hard cap under ConstantChip only —
    // MatchChart explicitly opts into the vendor row's nominal RPM and
    // shouldn't be silently re-clamped (the smoke baselines + chart-
    // fidelity tests depend on that opt-in). This step does not clamp the
    // Drill family. P20a removed the drill RPM tier (Step 2c), and this
    // step kept its Drill exclusion, so a drill under ConstantChip reads only
    // the Step 2b ceiling.
    if matches!(input.spindle_strategy, SpindleStrategy::ConstantChip { .. })
        && input.operation != OperationFamily::Drill
        && !matches!(
            input.tool_geometry,
            ToolGeometryHint::VBit { .. } | ToolGeometryHint::TaperedBall { .. }
        )
    {
        // Skip V-bit / tapered ball — see Step 2b note: tier ceiling
        // is calibrated against nominal-D, but these geometries cut on
        // engaged-D, so the nominal-D tier is a false constraint here.
        let tier_row = milling_rpm_ceiling_row(input.tool_diameter);
        let tier_ceiling = tier_row.value;
        if speed.rpm > tier_ceiling {
            let pre_clamp = speed.rpm;
            speed.rpm = tier_ceiling;
            speed.rpm = input.machine.clamp_rpm(speed.rpm);
            if pre_clamp > 0.0 && speed.rpm < pre_clamp {
                speed.spindle_scale *= speed.rpm / pre_clamp;
            }
            speed.basis = moved_basis(
                speed.basis.clone(),
                pre_clamp,
                speed.rpm,
                "milling RPM tier",
                Basis::Rule(tier_row),
            );
        }
    }
}
