//! Literature-matrix regression — Drill RPM ceiling is diameter-tiered.
//!
//! Cell: `flat_12mm_drill_oak_big`. Pre-fix, the drill RPM clamp in
//! `feeds/mod.rs` was diameter-independent at 8-14k RPM, letting a
//! 12 mm hardwood drill emit 12000 RPM — well above the 3-8k band
//! that Onsrud, Vectric, FPL Wood Handbook Ch.19, and Sandvik all
//! cite for big drills. Chip evacuation per revolution scales with
//! D²; big drills need fewer revolutions per second to clear chips.
//!
//! The fix replaced the two const-pair clamps with a single
//! `drill_rpm_envelope_for_diameter(d_mm) -> (floor, ceil)` helper:
//!   - D ≤ 6 mm:  (8000, 14000)
//!   - D ≤ 10 mm: (6000, 10000)
//!   - D > 10 mm: (4000, 8000)
//!
//! These tests assert the engine respects each tier under both
//! `SpindleStrategy::MatchChart` (the baseline path) and
//! `SpindleStrategy::MaxSpeed` (the speedup path) — the same two
//! entry points the round-2 ceiling test covered.

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

fn drill_input(d: f64, strategy: SpindleStrategy) -> FeedsInput<'static> {
    // SAFETY: `embedded_vendor_lut()` returns a `'static` reference.
    let lut = embedded_vendor_lut();
    let machine = Box::leak(Box::new(MachineProfile::shapeoko_vfd()));
    let material = Box::leak(Box::new(Material::SolidWood {
        species: WoodSpecies::WhiteOak,
    }));
    FeedsInput {
        tool_diameter: d,
        flute_count: 2,
        flute_length: d * 4.0,
        shank_diameter: None,
        tool_geometry: ToolGeometryHint::Flat,
        material,
        machine,
        operation: OperationFamily::Drill,
        pass_role: PassRole::Roughing,
        axial_depth_mm: Some(d),
        radial_width_mm: None,
        target_scallop_mm: None,
        vendor_lut: Some(lut),
        setup: SetupContext::default(),
        spindle_strategy: strategy,
    }
}

#[test]
fn big_drill_12mm_matchchart_under_8k_ceiling() {
    // The failing literature-matrix cell: 12 mm drill in white oak.
    // Pre-fix emitted 12000 RPM; post-fix must land at ≤ 8000.
    let input = drill_input(12.0, SpindleStrategy::MatchChart);
    let result = calculate(&input);
    assert!(
        result.rpm <= 8_000.0 + 1e-6,
        "12 mm Drill (MatchChart) RPM {} exceeds big-drill ceiling 8000",
        result.rpm,
    );
    assert!(
        result.rpm >= 4_000.0 - 1e-6,
        "12 mm Drill (MatchChart) RPM {} below big-drill floor 4000",
        result.rpm,
    );
}

#[test]
fn big_drill_12mm_maxspeed_under_8k_ceiling() {
    // The MaxSpeed path must also respect the big-drill tier — the
    // Step 2c re-clamp uses the same diameter-aware envelope.
    let input = drill_input(12.0, SpindleStrategy::MaxSpeed);
    let result = calculate(&input);
    assert!(
        result.rpm <= 8_000.0 + 1e-6,
        "12 mm Drill (MaxSpeed) RPM {} exceeds big-drill ceiling 8000",
        result.rpm,
    );
}

#[test]
fn mid_drill_8mm_under_10k_ceiling() {
    let input = drill_input(8.0, SpindleStrategy::MaxSpeed);
    let result = calculate(&input);
    assert!(
        result.rpm <= 10_000.0 + 1e-6,
        "8 mm Drill (MaxSpeed) RPM {} exceeds mid-drill ceiling 10000",
        result.rpm,
    );
    assert!(
        result.rpm >= 6_000.0 - 1e-6,
        "8 mm Drill (MaxSpeed) RPM {} below mid-drill floor 6000",
        result.rpm,
    );
}

#[test]
fn small_drill_3mm_retains_14k_ceiling() {
    // Regression guard for the round-2 small-drill cell: small drills
    // must still tolerate the 8-14k band; the new tiering must not
    // narrow the small-drill envelope.
    let input = drill_input(3.0, SpindleStrategy::MaxSpeed);
    let result = calculate(&input);
    assert!(
        result.rpm <= 14_000.0 + 1e-6,
        "3 mm Drill (MaxSpeed) RPM {} exceeds small-drill ceiling 14000",
        result.rpm,
    );
    assert!(
        result.rpm >= 8_000.0 - 1e-6,
        "3 mm Drill (MaxSpeed) RPM {} below small-drill floor 8000",
        result.rpm,
    );
}
