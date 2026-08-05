//! Literature-matrix regression — chipload must be clamped to the
//! 0.025 mm/tooth rubbing floor when extreme-Janka materials derate
//! a vendor-LUT row below the chip-formation threshold.
//!
//! Cell: `flat_6mm_pocket_ipe_hardness`. Pre-fix, Ipe (Janka 3510)
//! scaled an oak-anchored (~1290 lbf) vendor LUT row by 1290/3510 ≈
//! 0.367 via `vendor_lookup::hardness_ratio_raw`, producing a
//! 0.0124 mm/tooth chipload — well below the 0.025 mm/tooth chip-
//! formation floor where wood-router cutting becomes ploughing /
//! burning. The engine then served that as a valid recipe with no
//! warning: `LookupResult::chip_load_min_mm` is populated but was
//! never consulted in `feeds::calculate`.
//!
//! The fix adds a Step-2c-style clamp immediately after the LUT /
//! formula chipload is resolved: if `chip_load > 0.0 &&
//! chip_load < RUBBING_FLOOR_MM_TOOTH (0.025)`, clamp to the floor
//! and emit `FeedsWarning::ChiploadClampedToFloor`. Standard
//! machinist convention: clamp + warn, don't weaken the hardness
//! scaling for materials inside the calibrated band.
//!
//! The `> 0.0` guard intentionally preserves the RPM-only-LUT-row
//! sentry (zero-chipload should still surface via the formula
//! fallback path, not be silently bumped to 0.025).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::feeds::{
    FeedsInput, FeedsWarning, OperationFamily, PassRole, SetupContext, SpindleStrategy,
    ToolGeometryHint, calculate, embedded_vendor_lut,
};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, WoodSpecies};

const RUBBING_FLOOR_MM_TOOTH: f64 = 0.025;

fn calc_ipe_6mm() -> rs_cam_core::feeds::FeedsResult {
    let lut = embedded_vendor_lut();
    let machine = MachineProfile::generic_wood_router();
    let material = Material::SolidWood {
        species: WoodSpecies::Ipe,
    };

    calculate(&FeedsInput {
        tool_diameter: 6.0,
        flute_count: 2,
        flute_length: 22.0,
        shank_diameter: None,
        tool_geometry: ToolGeometryHint::Flat,
        material: &material,
        machine: &machine,
        operation: OperationFamily::Pocket,
        pass_role: PassRole::Roughing,
        axial_depth_mm: None,
        radial_width_mm: None,
        target_scallop_mm: None,
        vendor_lut: Some(lut),
        setup: SetupContext::default(),
        spindle_strategy: SpindleStrategy::MatchChart,
    })
}

#[test]
fn ipe_pocket_chipload_never_drops_below_rubbing_floor() {
    let result = calc_ipe_6mm();
    let rpm = result.rpm;
    let flutes = 2.0_f64;

    assert!(rpm > 0.0, "engine produced rpm = {rpm}");

    let chipload = result.feed_rate_mm_min / (rpm * flutes);
    assert!(
        chipload >= RUBBING_FLOOR_MM_TOOTH - 1e-9,
        "Ipe-derated chipload {chipload:.6} fell below rubbing floor \
         {RUBBING_FLOOR_MM_TOOTH} (feed_rate={}, rpm={rpm}, flutes={flutes}). \
         The Step-2c rubbing-floor clamp regressed.",
        result.feed_rate_mm_min,
    );
}

#[test]
fn ipe_pocket_emits_chipload_clamped_warning() {
    let result = calc_ipe_6mm();
    let has_clamp_warning = result
        .warnings
        .iter()
        .any(|w| matches!(w, FeedsWarning::ChiploadClampedToFloor { .. }));
    assert!(
        has_clamp_warning,
        "Expected FeedsWarning::ChiploadClampedToFloor for Ipe pocket cell \
         (pre-clamp chipload ~0.0124 < floor 0.025). Warnings: {:?}",
        result.warnings,
    );
}

#[test]
fn oak_pocket_chipload_above_floor_is_not_clamped() {
    // Anti-regression: a normal-Janka species (oak) must NOT trigger
    // the clamp warning — the LUT chipload sits comfortably above the
    // floor and the engine should pass it through verbatim. This locks
    // in that the clamp only fires on the rubbing-floor edge case.
    let lut = embedded_vendor_lut();
    let machine = MachineProfile::generic_wood_router();
    let material = Material::SolidWood {
        species: WoodSpecies::WhiteOak,
    };

    let result = calculate(&FeedsInput {
        tool_diameter: 6.0,
        flute_count: 2,
        flute_length: 22.0,
        shank_diameter: None,
        tool_geometry: ToolGeometryHint::Flat,
        material: &material,
        machine: &machine,
        operation: OperationFamily::Pocket,
        pass_role: PassRole::Roughing,
        axial_depth_mm: None,
        radial_width_mm: None,
        target_scallop_mm: None,
        vendor_lut: Some(lut),
        setup: SetupContext::default(),
        spindle_strategy: SpindleStrategy::MatchChart,
    });

    let has_clamp_warning = result
        .warnings
        .iter()
        .any(|w| matches!(w, FeedsWarning::ChiploadClampedToFloor { .. }));
    assert!(
        !has_clamp_warning,
        "Oak pocket should not have triggered ChiploadClampedToFloor; \
         warnings: {:?}",
        result.warnings,
    );

    let rpm = result.rpm;
    let chipload = result.feed_rate_mm_min / (rpm * 2.0);
    assert!(
        chipload > RUBBING_FLOOR_MM_TOOTH,
        "Oak chipload {chipload} should sit above the rubbing floor without clamping",
    );
}
