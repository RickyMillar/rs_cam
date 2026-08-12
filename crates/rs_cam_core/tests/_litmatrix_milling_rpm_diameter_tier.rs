//! Literature-matrix regression — milling RPM ceiling under
//! `SpindleStrategy::MaxSpeed` is diameter-tiered.
//!
//! Cells covered (round-5 Phase 3 / Pattern P1):
//!   - `flat_12mm_adaptive2d_oak_power`: pre-fix 22 800 RPM vs band
//!     [10 000, 14 000] (+62.9%)
//!   - `flat_10mm_pocket_maple`:          pre-fix 22 800 vs band
//!     [12 000, 16 000] (+42.5%)
//!   - `flat_6mm_pocket_softwood`:        pre-fix 22 000 vs band
//!     [14 000, 20 000] (+10.0%)
//!   - `flat_3mm_pocket_softwood/maple/ipe_extreme`: pre-fix 22 800
//!     vs band-max 22 000 (+3.6%).
//!
//! Root cause: `SpindleStrategy::MaxSpeed` walked RPM up to
//! `machine_max × SPINDLE_CEILING_HEADROOM = 24 000 × 0.95 = 22 800`
//! whenever the vendor LUT row had no `rpm_max` published. The cap
//! ignored tool diameter — the same 22.8 k ceiling applied to a 1.5 mm
//! micro endmill and a 12 mm shouldermill. For large tools (≥ 10 mm)
//! the chipload envelope in hardwood is much narrower (Onsrud series
//! 70/85: 12 mm 2F hardwood = 12-14 krpm; 10 mm = 12-16 krpm).
//!
//! The fix mirrors the drill diameter-tier pattern (round-4 commit
//! c9818dd / `_litmatrix_drill_rpm_diameter_tier.rs`):
//!   - D ≤ 3 mm:  22 000 RPM ceiling
//!   - D ≤ 6 mm:  20 000 RPM ceiling
//!   - D ≤ 8 mm:  18 000 RPM ceiling
//!   - D ≤ 10 mm: 16 000 RPM ceiling
//!   - D > 10 mm: 14 000 RPM ceiling
//!
//! Vendor `rpm_max` still wins when published — the tier ceiling only
//! gates the LUT-says-nothing path.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::feeds::{
    FeedsInput, OperationFamily, PassRole, SetupContext, SpindleStrategy, ToolGeometryHint,
    calculate, embedded_vendor_lut,
};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, WoodSpecies};

fn pocket_input(d: f64, flutes: u32, species: WoodSpecies) -> FeedsInput<'static> {
    // SAFETY: `embedded_vendor_lut()` returns a `'static` reference.
    let lut = embedded_vendor_lut();
    let machine = Box::leak(Box::new(MachineProfile::shapeoko_vfd()));
    let material = Box::leak(Box::new(Material::SolidWood { species }));
    FeedsInput {
        tool_diameter: d,
        flute_count: flutes,
        flute_length: d * 4.0,
        shank_diameter: None,
        tool_geometry: ToolGeometryHint::Flat,
        material,
        machine,
        operation: OperationFamily::Pocket,
        operation_kind: None,
        pass_role: PassRole::Roughing,
        axial_depth_mm: None,
        radial_width_mm: None,
        target_scallop_mm: None,
        vendor_lut: Some(lut),
        setup: SetupContext::default(),
        spindle_strategy: SpindleStrategy::MaxSpeed,
    }
}

fn adaptive_input(d: f64, flutes: u32, species: WoodSpecies) -> FeedsInput<'static> {
    let lut = embedded_vendor_lut();
    let machine = Box::leak(Box::new(MachineProfile::shapeoko_vfd()));
    let material = Box::leak(Box::new(Material::SolidWood { species }));
    FeedsInput {
        tool_diameter: d,
        flute_count: flutes,
        flute_length: d * 4.0,
        shank_diameter: None,
        tool_geometry: ToolGeometryHint::Flat,
        material,
        machine,
        operation: OperationFamily::Adaptive,
        operation_kind: None,
        pass_role: PassRole::Roughing,
        axial_depth_mm: None,
        radial_width_mm: None,
        target_scallop_mm: None,
        vendor_lut: Some(lut),
        setup: SetupContext::default(),
        spindle_strategy: SpindleStrategy::MaxSpeed,
    }
}

#[test]
fn big_mill_12mm_adaptive_under_14k_ceiling() {
    let input = adaptive_input(12.0, 4, WoodSpecies::WhiteOak);
    let result = calculate(&input);
    assert!(
        result.rpm <= 14_000.0 + 1e-6,
        "12 mm Flat Adaptive (MaxSpeed) RPM {} exceeds big-mill tier ceiling 14000",
        result.rpm,
    );
}

#[test]
fn mid_mill_10mm_pocket_under_16k_ceiling() {
    let input = pocket_input(10.0, 3, WoodSpecies::HardMaple);
    let result = calculate(&input);
    assert!(
        result.rpm <= 16_000.0 + 1e-6,
        "10 mm Flat Pocket (MaxSpeed) RPM {} exceeds 10mm-mill tier ceiling 16000",
        result.rpm,
    );
}

#[test]
fn mid_mill_8mm_pocket_under_18k_ceiling() {
    let input = pocket_input(8.0, 2, WoodSpecies::HardMaple);
    let result = calculate(&input);
    assert!(
        result.rpm <= 18_000.0 + 1e-6,
        "8 mm Flat Pocket (MaxSpeed) RPM {} exceeds 8mm-mill tier ceiling 18000",
        result.rpm,
    );
}

#[test]
fn small_mill_6mm_pocket_under_20k_ceiling() {
    let input = pocket_input(6.0, 2, WoodSpecies::GenericSoftwood);
    let result = calculate(&input);
    assert!(
        result.rpm <= 20_000.0 + 1e-6,
        "6 mm Flat Pocket (MaxSpeed) RPM {} exceeds 6mm-mill tier ceiling 20000",
        result.rpm,
    );
}

#[test]
fn tiny_mill_3mm_pocket_under_22k_ceiling() {
    let input = pocket_input(3.0, 2, WoodSpecies::GenericSoftwood);
    let result = calculate(&input);
    assert!(
        result.rpm <= 22_000.0 + 1e-6,
        "3 mm Flat Pocket (MaxSpeed) RPM {} exceeds 3mm-mill tier ceiling 22000",
        result.rpm,
    );
}

#[test]
fn small_mill_1p5mm_pocket_still_allowed_to_climb() {
    // Regression: a sub-3mm tool should still tolerate near-spindle
    // RPM (22000); the tier ceiling must not narrow the micro-tool
    // envelope below what the round-2 clamp tests expect.
    let input = pocket_input(1.5, 2, WoodSpecies::GenericSoftwood);
    let result = calculate(&input);
    assert!(
        result.rpm <= 22_000.0 + 1e-6,
        "1.5 mm Flat Pocket (MaxSpeed) RPM {} exceeds 3mm-tier ceiling 22000",
        result.rpm,
    );
    // And it should actually reach the tier (within the machine clamp
    // and the existing MAX_SPINDLE_SPEEDUP cap) — i.e. the speedup
    // didn't get nerfed to 0.
    assert!(
        result.rpm >= 15_000.0,
        "1.5 mm Flat Pocket (MaxSpeed) RPM {} unexpectedly low \
         — speedup may have been over-clamped",
        result.rpm,
    );
}
