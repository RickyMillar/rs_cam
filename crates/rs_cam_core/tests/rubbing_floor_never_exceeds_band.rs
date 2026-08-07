//! **The rubbing floor may not clamp a feed above the band it protects.**
//!
//! `planning/review_2026-08-04/FEEDS_CENSUS.md` C-12 / T3.3: Suggest's
//! Step-9b rubbing-floor clamp (`feeds::calculate`) raises the commanded
//! feed-per-tooth to a **global** constant, `RUBBING_FLOOR_MM_TOOTH =
//! 0.025`, while every other chipload bound in the crate is a *scaled
//! vendor band*. On small tools the two policies contradict: on the live
//! B3 row (Ø1 tapered-ball, 2 flutes, scallop finish, hard maple) the
//! matched row's band is 0.00378–0.00756 mm/tooth undated and
//! 0.003605–0.007211 after DOC derating, so the floor sits **3.31× above
//! the band maximum**. Clamping *up* to it commands a chipload the
//! post-sim chipload gate's own envelope calls breakage-side —
//! i.e. Suggest walks past the gate's ceiling in the name of protecting
//! against undershooting it.
//!
//! The floor itself is not in doubt. Rubbing/burnishing below a minimum
//! chip thickness is a real wood-routing failure mode (see the constant's
//! own doc comment) and the clamp survives. What cannot survive is a
//! floor that *exceeds the ceiling of the very band it is derived to keep
//! the recipe inside*. This sentry pins the subordination:
//!
//! ```text
//! effective_floor = min(RUBBING_FLOOR_MM_TOOTH, derated_band_max)
//! ```
//!
//! and nothing else. When no derated band exists (RPM-only vendor rows,
//! formula fallback) the global constant still applies unchanged — that
//! path is pinned by `_litmatrix_rpm_only_lut_chipload`.
//!
//! Both tests below are **red on parent** in the sense that matters: the
//! first fails outright, the second is the control that must stay green
//! across the fix.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]

use rs_cam_core::feeds::{
    ChiploadBounds, FeedsInput, FeedsResult, FeedsWarning, OperationFamily, PassRole, SetupContext,
    SpindleStrategy, ToolGeometryHint, calculate, embedded_vendor_lut,
};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, WoodSpecies};

/// The B3 reference operation, verbatim from
/// `tests/feed_explanation_snapshot_b3.rs`: a Ø1 mm tapered ball nose,
/// 2 flutes, 5.26° half-angle, scallop finish in hard maple (Janka 1450 —
/// the exact hardness the matched row publishes, so the hardness scale is
/// 1.00 and only the diameter scale moves).
fn b3_scallop() -> FeedsResult {
    let lut = embedded_vendor_lut();
    let machine = MachineProfile::generic_wood_router();
    let material = Material::SolidWood {
        species: WoodSpecies::HardMaple,
    };

    calculate(&FeedsInput {
        tool_diameter: 1.0,
        flute_count: 2,
        flute_length: 20.0,
        shank_diameter: Some(6.0),
        tool_geometry: ToolGeometryHint::TaperedBall {
            tip_radius: 0.5,
            taper_angle_deg: 5.26,
        },
        material: &material,
        machine: &machine,
        operation: OperationFamily::Scallop,
        pass_role: PassRole::Finish,
        axial_depth_mm: Some(0.35),
        radial_width_mm: None,
        target_scallop_mm: Some(0.01),
        vendor_lut: Some(lut),
        setup: SetupContext::default(),
        spindle_strategy: SpindleStrategy::MatchChart,
    })
}

fn commanded_fpt(result: &FeedsResult, flutes: f64) -> f64 {
    result.feed_rate_mm_min / (result.rpm * flutes)
}

fn band(result: &FeedsResult) -> ChiploadBounds {
    result
        .chipload_bounds
        .expect("the B3 row publishes a chipload band, so Suggest derates one")
}

#[test]
fn the_floor_never_clamps_a_feed_above_the_matched_band_maximum() {
    let result = b3_scallop();
    let fpt = commanded_fpt(&result, 2.0);
    let bounds = band(&result);

    let clamped = result
        .warnings
        .iter()
        .any(|w| matches!(w, FeedsWarning::ChiploadClampedToFloor { .. }));

    println!(
        "B3 scallop: rpm={:.0} feed={:.3} fpt={:.6} band={:.6}..{:.6} clamped={clamped}",
        result.rpm, result.feed_rate_mm_min, fpt, bounds.min_mm_per_tooth, bounds.max_mm_per_tooth,
    );

    assert!(
        clamped,
        "fixture precondition: the B3 row must derate below the rubbing floor so \
         the clamp fires at all. warnings: {:?}",
        result.warnings
    );

    assert!(
        fpt <= bounds.max_mm_per_tooth * (1.0 + 1e-9),
        "the rubbing-floor clamp raised feed-per-tooth to {fpt:.6} mm, which is \
         {:.2}× the matched row's derated band maximum {:.6} mm. Suggest is \
         commanding a chipload the post-sim gate's own envelope calls \
         breakage-side. (FEEDS_CENSUS C-12 / T3.3.)",
        fpt / bounds.max_mm_per_tooth,
        bounds.max_mm_per_tooth,
    );
}

#[test]
fn a_tool_whose_band_sits_above_the_floor_is_unchanged() {
    // CONTROL. Ø6 flat 2-flute pocket rough in white oak: the matched
    // band (~0.034–0.059 mm/tooth) is entirely *above* the 0.025 floor,
    // so `min(floor, band_max)` is the floor itself and nothing about
    // this recipe may move. If this test ever changes value, the fix has
    // leaked past the small-tool case it was scoped to.
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

    let bounds = band(&result);
    println!(
        "Ø6 oak pocket control: rpm={:.0} feed={:.3} fpt={:.6} band={:.6}..{:.6}",
        result.rpm,
        result.feed_rate_mm_min,
        commanded_fpt(&result, 2.0),
        bounds.min_mm_per_tooth,
        bounds.max_mm_per_tooth,
    );

    assert!(
        bounds.max_mm_per_tooth > 0.025,
        "control precondition: this row's band max {:.6} must sit above the \
         0.025 global floor, otherwise it is not a control",
        bounds.max_mm_per_tooth
    );
    assert!(
        !result
            .warnings
            .iter()
            .any(|w| matches!(w, FeedsWarning::ChiploadClampedToFloor { .. })),
        "control: the Ø6 oak pocket recipe must not clamp at all. warnings: {:?}",
        result.warnings
    );
}
