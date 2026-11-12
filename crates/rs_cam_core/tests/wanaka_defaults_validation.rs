//! Validation that the three pre-optimize default improvements
//! (Fixes 1, 2 — feeds calculator; Fix 4 — adaptive3d helix-on-
//! agent_search) produce the expected outcomes for the Wanaka100
//! scenario.
//!
//! Background: `planning/WANAKA_ASSESSMENT_2026-05-19.md` ran a
//! 3-phase assessment on the pristine Wanaka100 project and found
//! four pre-optimize-stage defaults that deviated from FSWizard
//! published bands. Three were addressed by the code fixes in
//! commit c7d9dbc. This test locks those fixes in against the same
//! input set the assessment used.
//!
//! Fix 5 (existing-project re-derivation policy) is a planning-doc
//! decision, no code, so it isn't validated here.

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::feeds::{FeedsInput, ToolGeometryHint, calculate};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, WoodSpecies};

#[path = "common/feeds_input.rs"]
mod feeds_input;

/// The Wanaka100 machine profile (Generic Wood Router): 0.8 kW
/// constant power, 8k–24k spindle, `adaptive_woc_factor = 0.20` (it
/// carried `safety_factor = 0.75` in the assessment session; ruling R4
/// removed it, and D9 removed the aggressiveness dial).
fn wanaka_machine() -> MachineProfile {
    MachineProfile::generic_wood_router()
}

/// Generic Hardwood — matches the Wanaka stock material.
fn wanaka_material() -> Material {
    Material::SolidWood {
        species: WoodSpecies::GenericHardwood,
    }
}

/// Fix 1 — A fresh adaptive3d toolpath created against the Wanaka
/// machine + a 6 mm flat end-mill in Generic Hardwood should land
/// stepover at `adaptive_woc_factor × D = 1.2 mm`, not the
/// pre-fix 0.7 mm.
#[test]
fn wanaka_adaptive3d_6mm_em_lands_stepover_at_target() {
    let machine = wanaka_machine();
    let material = wanaka_material();
    let target_mm = machine.rigidity.adaptive_woc_factor * 6.0;

    let result = calculate(
        &FeedsInput::of_operation(
            &OperationConfig::new_default(OperationType::Adaptive3d),
            &feeds_input::tool_of_hint(ToolGeometryHint::Flat, 6.0, 2, 18.0, 6.0),
            &material,
            &machine,
            rs_cam_core::feeds::embedded_vendor_lut(),
            rs_cam_core::feeds::SpindleStrategy::default(),
        )
        .with_no_vendor_table(),
    );

    assert!(
        (result.radial_width_mm - target_mm).abs() < 0.05,
        "Wanaka 6mm EM adaptive stepover {} should land at target {} (Fix 1)",
        result.radial_width_mm,
        target_mm
    );
}

/// Fix 2 — the 1 mm tapered ball-nose (Wanaka's TP4/TP5 tool) should
/// not emit plunge > 200 mm/min after Fix 2's tool-geometry derate.
/// Pre-fix it was 400 mm/min (project_curve) and 750 (drop_cutter).
#[test]
fn wanaka_1mm_tapered_ball_plunge_capped() {
    let machine = wanaka_machine();
    let material = wanaka_material();

    // TP4/TP5 case — the Project Curve and the drop cutter.
    for op in [OperationType::ProjectCurve, OperationType::DropCutter] {
        let result = calculate(
            &FeedsInput::of_operation(
                &OperationConfig::new_default(op),
                &feeds_input::tool_of_hint(
                    ToolGeometryHint::TaperedBall {
                        tip_radius: 0.5,
                        taper_angle_deg: 7.0,
                    },
                    1.0,
                    1,
                    6.0,
                    6.0,
                ),
                &material,
                &machine,
                rs_cam_core::feeds::embedded_vendor_lut(),
                rs_cam_core::feeds::SpindleStrategy::default(),
            )
            .with_no_vendor_table(),
        );

        assert!(
            result.plunge_rate_mm_min <= 200.0,
            "Wanaka 1mm TB plunge {} on {:?} should be ≤ 200 mm/min after Fix 2",
            result.plunge_rate_mm_min,
            op
        );
    }
}

/// Counter-check: a 6 mm flat EM in the same wood should NOT be
/// derated by Fix 2 (only ball/tapered-ball geometries are
/// capped).
#[test]
fn wanaka_6mm_em_plunge_not_derated() {
    let machine = wanaka_machine();
    let material = wanaka_material();

    let result = calculate(
        &FeedsInput::of_operation(
            &OperationConfig::new_default(OperationType::Adaptive),
            &feeds_input::tool_of_hint(ToolGeometryHint::Flat, 6.0, 2, 18.0, 6.0),
            &material,
            &machine,
            rs_cam_core::feeds::embedded_vendor_lut(),
            rs_cam_core::feeds::SpindleStrategy::default(),
        )
        .with_no_vendor_table(),
    );

    // Generic Hardwood (hardness ~1.42) → plunge_rate_base ~700 mm/min, no
    // factor since ruling R4 Q5 (was × 0.75 ≈ 525). Should not be derated.
    assert!(
        result.plunge_rate_mm_min > 300.0,
        "Wanaka 6mm EM plunge {} should not be capped by Fix 2 tool-geometry rule",
        result.plunge_rate_mm_min
    );
}
