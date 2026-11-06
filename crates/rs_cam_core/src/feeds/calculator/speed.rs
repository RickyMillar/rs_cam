//! The spindle-speed steps of the calculator: the engaged diameter, Step 1,
//! the vendor RPM of Step 2, and Steps 2b, 2b' and 2c.

use super::{Chip, Speed};
use crate::feeds::sourced::{self, Basis};
use crate::feeds::{
    FALLBACK_RPM, FeedsInput, MAX_SPINDLE_SPEEDUP, MAX_SPINDLE_SPEEDUP_ROW, OperationFamily,
    SPINDLE_CEILING_HEADROOM, SpindleScaleReason, SpindleStrategy, ToolGeometryHint,
    drill_rpm_tier_rows, milling_rpm_ceiling_for_diameter, milling_rpm_ceiling_row, moved_basis,
};

/// The engaged diameter at the commanded depth, with the basis of the
/// depth that sets it.
pub(super) struct EngagedDiameter {
    /// The LUT-semantics engaged diameter (see the naming warning in
    /// [`engaged_diameter`]).
    pub(super) diameter: f64,
    /// The basis of the depth: the operator's depth, or the tool diameter.
    depth_basis: Basis,
}

/// The engaged diameter at the operation's commanded DOC.
pub(super) fn engaged_diameter(input: &FeedsInput) -> EngagedDiameter {
    let d = input.tool_diameter;
    // Engaged diameter at the operation's commanded DOC. For VBit and
    // TaperedBall geometries the *cutting* circle is the cone-shoulder
    // diameter at this DOC — not the published tip. Flat / Ball / Bull
    // tools engage at nominal D regardless of DOC, so `effective_d == d`
    // there.
    //
    // Before ruling A1 (2026-09-24) this mirrored
    // `vendor_normalize::lookup_diameter_for_input`. It does not now for a
    // tapered ball: the LUT key is the tip (`geometry::lut_key_diameter_mm`),
    // and this binding stays the engaged cone diameter for the SFM, the
    // RPM and the formula chipload. Since ruling B4 (2026-09-25) a V-bit
    // is keyed at its nominal diameter too, and this binding stays its
    // engaged width for the SFM, the RPM and the formula chipload
    // (`feeds::tests::test_vbit_rpm_uses_engaged_diameter` pins it). A
    // V-bit row that prints an RPM (Amana AMS-159 and Spektra engraving,
    // 18 000) overrides this RPM as every vendor RPM does.
    // Pre-2026-06-02 the
    // formula path used nominal D, producing wrong-low RPM for V-bits
    // (e.g. a 5.5 mm-tip 20° V-bit at DOC=0.5 saw SFM derived from
    // 5.5 mm instead of the ~0.18 mm engaged tip) — audit finding
    // "nominal-D leakage through formula path".
    //
    // NAMING WARNING (census F-3, T1.4). This binding is the
    // **LUT-semantics** engaged diameter — before A1 it was "which vendor
    // row applies"; since A1 that is the tip for a tapered ball. On a
    // tapered ball the chipload band's depth de-rate still uses the same
    // kind of diameter, taken again at the shipped depth after the power
    // ladder. On a V-bit it no longer does: B4 step 2 moves the band
    // de-rate to the nominal diameter, so it diverges from this binding,
    // which stays the engaged width.
    // Step 5 computes the **chip-thinning** diameter
    // (`feeds::effective_diameter`, "what actually touches material",
    // `Feed::effective_d`), and it is *that* one which is published as
    // `FeedsResult::effective_diameter_mm`. Two different questions, two
    // fields. Collapsing them is census T2.4.
    let axial_doc_for_eff_d = input.axial_depth_mm.unwrap_or(d).max(0.0);
    // P5 (K2): the depth that sets the SFM diameter is the operator's when
    // the input states one, else the tool diameter (a tool fact).
    // P16 (item 2b): on a surface offset (Project Curve) a tool whose
    // engaged diameter grows with the depth (ball, tapered ball, V-bit)
    // takes the rpm at the full diameter; the basis says why.
    let grows_with_depth = match input.tool_geometry {
        ToolGeometryHint::Ball
        | ToolGeometryHint::TaperedBall { .. }
        | ToolGeometryHint::VBit { .. } => true,
        ToolGeometryHint::Flat | ToolGeometryHint::Bull { .. } => false,
    };
    let depth_basis = if input.axial_depth_mm.is_some() {
        Basis::stated()
    } else if input.depth_after_simulation() && grows_with_depth {
        Basis::derived(RPM_AT_FULL_DIAMETER_LAW, vec![Basis::tool_diameter()])
    } else {
        Basis::tool_diameter()
    };
    EngagedDiameter {
        diameter: input.tool_geometry.engaged_diameter_at_doc(
            axial_doc_for_eff_d,
            d,
            input.shank_diameter.unwrap_or(d),
        ),
        depth_basis,
    }
}

/// P16 (item 2b): the law of the SFM diameter of a cone tool on a surface
/// offset. The card shows it in the speed basis.
pub(crate) const RPM_AT_FULL_DIAMETER_LAW: &str =
    "rpm at the full diameter: the depth is known after the simulation";

fn machine_rpm() -> Basis {
    Basis::Machine(sourced::MachineField::SpindleRpm)
}

/// Step 1: the RPM from the surface speed at the engaged diameter, the
/// machine range, and the drill RPM tier.
pub(super) fn step1_rpm(input: &FeedsInput, engaged: &EngagedDiameter) -> Speed {
    let machine = input.machine;
    let material = input.material;
    let d = input.tool_diameter;
    let effective_d = engaged.diameter;
    // --- Step 1: RPM ---
    let (ideal_rpm, sfm_basis) = if effective_d > 0.0 {
        let speed = material.base_cutting_speed_constant();
        (
            (speed.value * 1000.0) / (std::f64::consts::PI * effective_d),
            Basis::derived(
                "SFM at engaged diameter",
                vec![Basis::Rule(speed), engaged.depth_basis.clone()],
            ),
        )
    } else {
        (FALLBACK_RPM.value, Basis::Rule(FALLBACK_RPM))
    };
    let mut rpm = machine.clamp_rpm(ideal_rpm);
    let mut rpm_basis = moved_basis(
        sfm_basis,
        ideal_rpm,
        rpm,
        "machine RPM range",
        machine_rpm(),
    );

    // Drill ops want a lower RPM band than milling regardless of D:
    // chip evacuation, not surface speed, is the limiting factor. The
    // wood-drill band tightens as diameter grows — small drills (≤6 mm)
    // tolerate 8-14k, mid drills (≤10 mm) cap around 10k, and big
    // drills (>10 mm) cap around 6-8k. **Citation corrected
    // 2026-08-04 (W6 audit §6.1/§6.2): these tiers are REPO-AUTHORED.**
    // The "Onsrud wood-drilling bulletin" cited here was not located;
    // the retrieved Onsrud drill chart publishes exactly one wood-drill
    // RPM, the 4,500 gang-drill footnote, and no band. The FPL Wood
    // Handbook has no drilling chapter (Ch.19 is *Specialty
    // Treatments*). The tiers remain defensible as hobby-router
    // spindle practice and are held, not moved. The milling SFM formula above
    // would push small-D drills past 16k where chipload starves and
    // the cut rubs/burns. Pre-2026-06-02 drill ops routed through
    // `Pocket` family and inherited milling RPM (audit finding: "Drill
    // ops route through OperationFamily::Pocket with no chipload
    // reconciliation"). Pre-2026-06-03 the clamp was diameter-
    // independent at 8-14k, letting 12 mm hardwood drills emit 12k RPM
    // — well above the 3-8k band (literature-matrix cell
    // flat_12mm_drill_oak_big: ceiling 8000, emitted 12000, +50%).
    if input.operation == OperationFamily::Drill {
        let (floor_row, ceil_row) = drill_rpm_tier_rows(d);
        let before = rpm;
        rpm = rpm.clamp(floor_row.value, ceil_row.value);
        rpm = machine.clamp_rpm(rpm);
        let tier = if before > rpm { ceil_row } else { floor_row };
        rpm_basis = moved_basis(rpm_basis, before, rpm, "drill RPM tier", Basis::Rule(tier));
    }
    Speed {
        rpm,
        basis: rpm_basis,
        spindle_scale: 1.0_f64,
        spindle_scale_reason: SpindleScaleReason::Unchanged,
    }
}

/// Step 2 (the RPM half): the vendor RPM replaces the Step 1 RPM.
pub(super) fn step2_vendor_rpm(input: &FeedsInput, chip: &Chip, speed: &mut Speed) {
    // Override RPM if vendor provided one within machine range. This
    // is the chart-RPM operating point — preserved verbatim under
    // `SpindleStrategy::MatchChart`.
    if let Some(v_rpm) = chip.vendor_rpm {
        speed.rpm = input.machine.clamp_rpm(v_rpm);
        let printed = Basis::Printed(sourced::RowRef(
            chip.vendor_source.clone().unwrap_or_default(),
        ));
        speed.basis = moved_basis(
            printed,
            v_rpm,
            speed.rpm,
            "machine RPM range",
            machine_rpm(),
        );
    }
}

/// Step 2b: the spindle speed-up along the constant-chipload line
/// (`SpindleStrategy::MaxSpeed`).
pub(super) fn step2b_max_speed(input: &FeedsInput, chip: &Chip, speed: &mut Speed) {
    let machine = input.machine;
    let d = input.tool_diameter;
    // --- Step 2b: Spindle-speedup along the constant-chipload line ---
    //
    // Under `SpindleStrategy::MaxSpeed` push RPM up toward the
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
    if matches!(input.spindle_strategy, SpindleStrategy::MaxSpeed) && speed.rpm > 0.0 {
        let (_, machine_max_rpm) = machine.rpm_range();
        let machine_ceiling = machine_max_rpm * SPINDLE_CEILING_HEADROOM;
        // Diameter-tier literature ceiling for milling. The MaxSpeed
        // speedup pre-2026-06-03 used machine_ceiling unconditionally,
        // pushing a 12 mm hardwood adaptive2d cut to 22.8 k RPM —
        // ~63% above the Onsrud/Amana 14 k literature ceiling
        // (literature-matrix cell flat_12mm_adaptive2d_oak_power).
        // Drill ops already had a diameter tier from round-4
        // (`drill_rpm_envelope_for_diameter`, commit 9c4f3b9); this is
        // the milling-side analogue.
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

/// Step 2b': the diameter-tier ceiling for milling under `MaxSpeed`.
pub(super) fn step2b_milling_tier(input: &FeedsInput, speed: &mut Speed) {
    // --- Step 2b': Diameter-tier ceiling for milling under MaxSpeed ---
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
    // Apply the tier ceiling as a hard cap under MaxSpeed only —
    // MatchChart explicitly opts into the vendor row's nominal RPM and
    // shouldn't be silently re-clamped (the smoke baselines + chart-
    // fidelity tests depend on that opt-in). The Drill family has its
    // own dedicated Step 2c clamp below.
    if matches!(input.spindle_strategy, SpindleStrategy::MaxSpeed)
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

/// Step 2c: the drill RPM tier again, after the vendor RPM and Step 2b.
pub(super) fn step2c_drill_tier(input: &FeedsInput, speed: &mut Speed) {
    // --- Step 2c: Re-apply the Drill RPM clamp ---
    //
    // Both the vendor-RPM override (Step 2) and the MaxSpeed speedup
    // (Step 2b) can lift RPM above the drill ceiling. Chip evacuation
    // is the binding constraint for drill ops regardless of spindle
    // headroom or chart RPM — pushing past 14k starves chipload below
    // the rubbing floor (literature-matrix cell flat_3mm_drill_oak:
    // MaxSpeed lifted clamped 14000 → 14000 × MAX_SPINDLE_SPEEDUP =
    // 21000, +50% over the 8-14k wood-drill band).
    //
    // We also roll `spindle_scale` back proportionally so the modal
    // reports the actual speedup the engine kept, not the requested
    // one it then undid.
    if input.operation == OperationFamily::Drill {
        let (floor_row, ceil_row) = drill_rpm_tier_rows(input.tool_diameter);
        let pre_clamp = speed.rpm;
        speed.rpm = speed.rpm.clamp(floor_row.value, ceil_row.value);
        speed.rpm = input.machine.clamp_rpm(speed.rpm);
        if pre_clamp > 0.0 && speed.rpm < pre_clamp {
            speed.spindle_scale *= speed.rpm / pre_clamp;
        }
        let tier = if pre_clamp > speed.rpm {
            ceil_row
        } else {
            floor_row
        };
        speed.basis = moved_basis(
            speed.basis.clone(),
            pre_clamp,
            speed.rpm,
            "drill RPM tier",
            Basis::Rule(tier),
        );
    }
}
