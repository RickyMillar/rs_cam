//! **The rubbing floor warns. It never lifts a feed.**
//!
//! Ruling R4 WP2a (2026-09-23,
//! `planning/feeds_matrix_2026-09-23/R4_AGGRESSIVENESS_SPEC.md` §3.6): the
//! operator ruled "keep the warning, drop the clamp". Until then Step 9b of
//! `feeds::calculate` (and Suggest pass 9) raised a commanded advance per
//! tooth below the floor up to it. The floor constant,
//! `RUBBING_FLOOR_MM_TOOTH = 0.025`, is a repo rule with no source, and the
//! lift contradicted vendor bands that sit below it (EVIDENCE 4.1-26).
//!
//! This file was `rubbing_floor_never_exceeds_band.rs`. That sentry pinned
//! `effective_floor = min(RUBBING_FLOOR_MM_TOOTH, derated_band_max)` so the
//! lift could not push a feed past the band. With no lift, the band rule
//! holds trivially; the warning threshold is still that `min` (Q9 asks
//! whether it becomes the band minimum).
//!
//! What this sentry pins:
//!
//! 1. On the B3 sub-floor fixture the warning fires, the commanded advance
//!    equals `derates.effective_chip_load_mm()` (the computed feed, no
//!    lift) to 1e-9, and it is at or below the band maximum.
//! 2. On the Ø6.35 oak ball control no warning fires.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]

use rs_cam_core::feeds::vendor_lut::{EvidenceGrade, ObservationKind, VendorLut};
use rs_cam_core::feeds::{
    ChiploadBounds, FeedsInput, FeedsResult, FeedsWarning, OperationFamily, PassRole, SetupContext,
    SpindleStrategy, ToolGeometryHint, calculate, embedded_vendor_lut,
};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, WoodSpecies};

/// A one-row LUT whose derated band sits wholly below the 0.025 mm/tooth
/// floor. Feeds matrix R5 (2026-09-23) replaced the reduced Amana rows the
/// live B3 cell used to match with the printed Onsrud 77-100 rows
/// (0.0762-0.127 mm/tooth at 1/8 in), so no embedded wood row scales below
/// the floor any more. The subordination rule this file pins is unchanged,
/// so its fixture is now explicit: the printed Onsrud scallop row, cloned,
/// re-labelled derived/c, with the ledger's B3 band (0.00378-0.00756) as
/// its bounds.
fn sub_floor_lut() -> VendorLut {
    let mut row = embedded_vendor_lut()
        .observations
        .iter()
        .find(|o| o.observation_id == "onsrud-hardwood-77-100-1_8-scallop")
        .expect("the printed Onsrud 77-100 scallop row exists")
        .clone();
    row.observation_id = "synthetic-b3-sub-floor-scallop".to_owned();
    row.diameter_mm = Some(1.0);
    row.flute_count = 2;
    row.chipload_min_mm_tooth = Some(0.003_78);
    row.chipload_max_mm_tooth = Some(0.007_56);
    row.row_kind = ObservationKind::Derived;
    row.evidence_grade = EvidenceGrade::C;
    row.notes = Some(
        "test fixture: the ledger B3 band on a cloned printed row; not a vendor figure".to_owned(),
    );
    VendorLut {
        observations: vec![row],
    }
}

/// The B3 reference operation, verbatim from
/// `tests/feed_explanation_snapshot_b3.rs`: a Ø1 mm tapered ball nose,
/// 2 flutes, 5.26° half-angle, scallop finish in hard maple, against the
/// sub-floor fixture LUT above.
fn b3_scallop() -> FeedsResult {
    let lut = &sub_floor_lut();
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
        operation_kind: None,
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
fn a_sub_floor_recipe_ships_its_computed_feed_and_warns() {
    let result = b3_scallop();
    let fpt = commanded_fpt(&result, 2.0);
    let bounds = band(&result);
    let computed = result.derates.effective_chip_load_mm();

    let warning = result.warnings.iter().find_map(|w| match w {
        FeedsWarning::ChiploadBelowRubbingFloor {
            commanded,
            floor,
            band_max,
        } => Some((*commanded, *floor, *band_max)),
        _ => None,
    });

    println!(
        "B3 scallop: rpm={:.0} feed={:.3} fpt={:.6} computed={computed:.6} \
         band={:.6}..{:.6} warning={warning:?}",
        result.rpm, result.feed_rate_mm_min, fpt, bounds.min_mm_per_tooth, bounds.max_mm_per_tooth,
    );

    let (commanded, floor, band_max) = warning.unwrap_or_else(|| {
        panic!(
            "the B3 fixture derates below the rubbing floor, so the warning must \
             fire. warnings: {:?}",
            result.warnings
        )
    });

    assert!(
        (fpt - computed).abs() <= computed.abs() * 1e-9,
        "the shipped advance {fpt:.9} mm/tooth is not the computed advance \
         {computed:.9} (target x combined derates). A floor lift moved the feed; \
         ruling R4 WP2a removed it."
    );
    assert!(
        (commanded - fpt).abs() <= fpt.abs() * 1e-9,
        "the warning reports {commanded:.9} mm/tooth but the recipe ships {fpt:.9}"
    );
    assert!(
        commanded < floor,
        "the warning fired at {commanded:.6}, which is not below its floor {floor:.6}"
    );
    assert!(
        fpt <= bounds.max_mm_per_tooth * (1.0 + 1e-9),
        "the shipped advance {fpt:.6} is above the derated band maximum {:.6}",
        bounds.max_mm_per_tooth
    );
    assert_eq!(
        band_max,
        Some(bounds.max_mm_per_tooth),
        "the warning carries the band maximum that the floor read"
    );
}

#[test]
fn a_tool_whose_band_sits_above_the_floor_does_not_warn() {
    // CONTROL. Ø6.35 ball 2-flute pocket rough in white oak: the matched
    // printed Amana ball-nose v7 row (`amana-ball-hardwood-pocket-6350-2f-v7`,
    // 0.127-0.1778 mm/tooth before the hardness scale) is entirely *above*
    // the 0.025 floor, so `min(floor, band_max)` is the floor itself and
    // nothing about this recipe may move. Feeds matrix R5 (2026-09-23): the
    // flat 6 mm pocket cell resolves to a single-value Spektra row with no
    // band, so the control moved to a printed row that publishes one.
    let lut = embedded_vendor_lut();
    let machine = MachineProfile::generic_wood_router();
    let material = Material::SolidWood {
        species: WoodSpecies::WhiteOak,
    };

    let result = calculate(&FeedsInput {
        tool_diameter: 6.35,
        flute_count: 2,
        flute_length: 22.0,
        shank_diameter: None,
        tool_geometry: ToolGeometryHint::Ball,
        material: &material,
        machine: &machine,
        operation: OperationFamily::Pocket,
        operation_kind: None,
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
        "Ø6.35 oak ball pocket control: rpm={:.0} feed={:.3} fpt={:.6} band={:.6}..{:.6}",
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
            .any(|w| matches!(w, FeedsWarning::ChiploadBelowRubbingFloor { .. })),
        "control: the Ø6 oak pocket recipe must not warn at all. warnings: {:?}",
        result.warnings
    );
}
