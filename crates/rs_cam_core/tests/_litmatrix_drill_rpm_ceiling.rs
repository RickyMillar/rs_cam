//! Literature-matrix regression — Drill RPM ceiling survives downstream
//! lifts.
//!
//! Cell: `flat_3mm_drill_oak`. Pre-fix, the Step 1 drill clamp at
//! `feeds/mod.rs:447-450` correctly landed RPM at 14000, and the
//! `SpindleStrategy::MaxSpeed` speedup (Step 2b) in `calculate()` then
//! lifted it.
//!
//! **Docstring corrected 2026-08-04 (W6 audit §4, item R-17).** This
//! used to name a second path, "the vendor-RPM override (Step 2)". That
//! path cannot fire for a drill query: 0 of the 256 bundled LUT rows
//! carry `operation_family: "drill"`, and `passes_must_match` returns
//! false immediately on family mismatch, so a drill lookup is a
//! guaranteed miss and no vendor RPM is ever available to override
//! with. The sentry still guards something real — the MaxSpeed arm is
//! live — but half its stated mechanism was fiction. Assertions
//! unchanged; see `queryable_families_without_rows_are_a_stated_fact`
//! in `feeds/vendor_lut.rs` for the test that now states the emptiness
//! rather than leaving it to be rediscovered.
//!
//! Result: 14000 × `MAX_SPINDLE_SPEEDUP` (1.5) = 21000 RPM, exactly the
//! +50% overshoot the literature matrix flagged as
//! `invariant.drill_rpm_ceiling`. Chip evacuation is an
//! `OperationFamily` invariant — neither chart RPM nor spindle headroom
//! should be allowed to lift a Drill op above the wood-drill band.
//!
//! This sentry asserts that under `SpindleStrategy::MaxSpeed` a Drill op
//! cannot exceed the 14k drill ceiling.

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

const DRILL_RPM_CEIL: f64 = 14_000.0;

#[test]
fn max_speed_cannot_lift_drill_above_ceiling() {
    let lut = embedded_vendor_lut();
    let machine = MachineProfile::shapeoko_vfd();
    let material = Material::SolidWood {
        species: WoodSpecies::WhiteOak,
    };

    let result = calculate(&FeedsInput {
        tool_diameter: 3.0,
        flute_count: 2,
        flute_length: 12.0,
        shank_diameter: None,
        tool_geometry: ToolGeometryHint::Flat,
        material: &material,
        machine: &machine,
        operation: OperationFamily::Drill,
        operation_kind: None,
        pass_role: PassRole::Roughing,
        axial_depth_mm: Some(3.0),
        radial_width_mm: None,
        target_scallop_mm: None,
        vendor_lut: Some(lut),
        setup: SetupContext::default(),
        spindle_strategy: SpindleStrategy::MaxSpeed,
    });

    assert!(
        result.rpm <= DRILL_RPM_CEIL + 1e-6,
        "MaxSpeed Drill op lifted RPM to {} but the drill ceiling is {}",
        result.rpm,
        DRILL_RPM_CEIL,
    );
}

#[test]
fn match_chart_drill_stays_at_ceiling() {
    // Baseline: under MatchChart the drill clamp already worked
    // pre-fix; this sentry locks that in so a future refactor can't
    // regress the easy case while still passing the MaxSpeed test.
    let lut = embedded_vendor_lut();
    let machine = MachineProfile::shapeoko_vfd();
    let material = Material::SolidWood {
        species: WoodSpecies::WhiteOak,
    };

    let result = calculate(&FeedsInput {
        tool_diameter: 3.0,
        flute_count: 2,
        flute_length: 12.0,
        shank_diameter: None,
        tool_geometry: ToolGeometryHint::Flat,
        material: &material,
        machine: &machine,
        operation: OperationFamily::Drill,
        operation_kind: None,
        pass_role: PassRole::Roughing,
        axial_depth_mm: Some(3.0),
        radial_width_mm: None,
        target_scallop_mm: None,
        vendor_lut: Some(lut),
        setup: SetupContext::default(),
        spindle_strategy: SpindleStrategy::MatchChart,
    });

    assert!(
        result.rpm <= DRILL_RPM_CEIL + 1e-6,
        "MatchChart Drill op RPM {} exceeds ceiling {}",
        result.rpm,
        DRILL_RPM_CEIL,
    );
}
