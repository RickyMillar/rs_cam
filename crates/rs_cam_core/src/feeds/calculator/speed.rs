//! The spindle-speed steps of the calculator: the engaged diameter, Step 1
//! and the vendor RPM of Step 2. The constant-chip RPM is Step 5c in
//! `spindle.rs` (feeds contract P21c: the one RPM-choice door).

use super::{Chip, Speed};
use crate::feeds::sourced::{self, Basis};
use crate::feeds::{FALLBACK_RPM, FeedsInput, SpindleScaleReason, ToolGeometryHint, moved_basis};

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
            input.cut_body.max_cut_diameter_mm,
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

/// Step 1: the RPM from the surface speed at the engaged diameter and the
/// machine range.
pub(super) fn step1_rpm(input: &FeedsInput, engaged: &EngagedDiameter) -> Speed {
    let machine = input.machine;
    let material = input.material;
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
    let rpm = machine.clamp_rpm(ideal_rpm);
    let rpm_basis = moved_basis(
        sfm_basis,
        ideal_rpm,
        rpm,
        "machine RPM range",
        machine_rpm(),
    );

    // P20a (2026-11-08): no drill RPM tier. The tier had no source, and
    // the chart that the G6 drill claim reads prints 18 000 RPM for the
    // same tool. A drill takes the RPM of the vendor row (Step 2), clamped
    // only to the machine spindle range.
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
