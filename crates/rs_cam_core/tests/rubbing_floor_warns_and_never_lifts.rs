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
//! holds trivially.
//!
//! Ruling R4 Q9 (2026-09-24) moved the threshold to
//! `min(0.025, band min)`: a published band minimum outranks the unsourced
//! constant. A band with no minimum keeps `min(0.025, band max)`; no band
//! keeps the constant. `feeds::rubbing_floor` names the bound that set it.
//!
//! What this sentry pins:
//!
//! 1. The rule on three bands, by hand: 0.0036–0.0072 gives 0.0036
//!    (band minimum); 0.034–0.059 gives 0.025 (constant); 0.0–0.012 gives
//!    0.012 (band maximum, no minimum).
//! 2. On the B3 sub-floor fixture (band wholly below 0.025) the floor is
//!    the band minimum. The recipe ships inside the band: the fixture band
//!    is 0.00378–0.00756, so the midpoint is 1.5 × the minimum, and the
//!    shipped advance is the midpoint × the feed factors (× 0.75 before R4
//!    WP3), at least 1.125 × the minimum. No warning fires, and the
//!    shipped advance equals `derates.effective_chip_load_mm()` (no lift).
//! 3. The same fixture on a machine with a 50 mm/min cutting ceiling: the
//!    ceiling pushes the advance under the band minimum
//!    (50 / (2 × 8000 rpm) = 0.0031 at the lowest RPM of the generic router,
//!    and less at any higher RPM), so the warning fires with the source
//!    `BandMinimum`, and the feed is not raised.
//! 4. On the Ø6.35 oak ball control no warning fires.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]

use rs_cam_core::feeds::vendor_lut::{
    EvidenceGrade, LutOperationFamily, LutPassRole, ObservationKind, VendorLut,
};
use rs_cam_core::feeds::{
    ChiploadBounds, FeedsInput, FeedsResult, FeedsWarning, OperationFamily, PassRole,
    RUBBING_FLOOR_MM_TOOTH, RubbingFloorSource, SetupContext, SpindleStrategy, ToolGeometryHint,
    calculate, effective_rubbing_floor, embedded_vendor_lut, rubbing_floor,
};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, WoodSpecies};

/// A one-row LUT whose derated band sits wholly below the 0.025 mm/tooth
/// floor. Feeds matrix R5 (2026-09-23) replaced the reduced Amana rows the
/// live B3 cell used to match with the printed Onsrud 77-100 rows
/// (0.0762-0.127 mm/tooth at 1/8 in), so no embedded wood row scales below
/// the floor any more. The subordination rule this file pins is unchanged,
/// so its fixture is now explicit: the printed Onsrud row, cloned,
/// re-labelled derived/c, with the ledger's B3 band (0.00378-0.00756) as
/// its bounds. Since A3 step 3 the LUT files the printed row once, under
/// pocket/roughing. The fixture files its clone under scallop/finish. Thus
/// the row is printed in the queried family, and no family rule applies.
fn sub_floor_lut() -> VendorLut {
    let mut row = embedded_vendor_lut()
        .observations
        .iter()
        .find(|o| o.observation_id == "onsrud-hardwood-77-100-1_8-pocket")
        .expect("the printed Onsrud 77-100 pocket row exists")
        .clone();
    row.observation_id = "synthetic-b3-sub-floor-scallop".to_owned();
    row.operation_family = LutOperationFamily::Scallop;
    row.pass_role = LutPassRole::Finish;
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
    b3_scallop_on(&MachineProfile::generic_wood_router())
}

/// The B3 operation on a given machine.
fn b3_scallop_on(machine: &MachineProfile) -> FeedsResult {
    let lut = &sub_floor_lut();
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
        machine,
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
fn the_floor_is_the_band_minimum_then_the_band_maximum_then_the_constant() {
    let sub = ChiploadBounds {
        min_mm_per_tooth: 0.0036,
        max_mm_per_tooth: 0.0072,
    };
    assert_eq!(
        rubbing_floor(Some(sub)),
        (0.0036, RubbingFloorSource::BandMinimum)
    );
    let roomy = ChiploadBounds {
        min_mm_per_tooth: 0.034,
        max_mm_per_tooth: 0.059,
    };
    assert_eq!(
        rubbing_floor(Some(roomy)),
        (RUBBING_FLOOR_MM_TOOTH, RubbingFloorSource::RepoConstant)
    );
    let max_only = ChiploadBounds {
        min_mm_per_tooth: 0.0,
        max_mm_per_tooth: 0.012,
    };
    assert_eq!(
        rubbing_floor(Some(max_only)),
        (0.012, RubbingFloorSource::BandMaximum)
    );
    assert_eq!(
        rubbing_floor(None),
        (RUBBING_FLOOR_MM_TOOTH, RubbingFloorSource::RepoConstant)
    );
    assert_eq!(effective_rubbing_floor(Some(sub)), 0.0036);
}

#[test]
fn a_recipe_inside_a_sub_floor_band_ships_its_computed_feed_and_does_not_warn() {
    let result = b3_scallop();
    let fpt = commanded_fpt(&result, 2.0);
    let bounds = band(&result);
    let computed = result.derates.effective_chip_load_mm();

    println!(
        "B3 scallop: rpm={:.0} feed={:.3} fpt={:.6} computed={computed:.6} \
         band={:.6}..{:.6} warnings={:?}",
        result.rpm,
        result.feed_rate_mm_min,
        fpt,
        bounds.min_mm_per_tooth,
        bounds.max_mm_per_tooth,
        result.warnings,
    );

    assert!(
        bounds.max_mm_per_tooth < RUBBING_FLOOR_MM_TOOTH,
        "precondition: the fixture band {:.6} must sit wholly below the 0.025 floor",
        bounds.max_mm_per_tooth
    );
    assert_eq!(
        rubbing_floor(Some(bounds)),
        (bounds.min_mm_per_tooth, RubbingFloorSource::BandMinimum),
        "a published band minimum below 0.025 is the floor (ruling R4 Q9)"
    );
    assert!(
        (fpt - computed).abs() <= computed.abs() * 1e-9,
        "the shipped advance {fpt:.9} mm/tooth is not the computed advance \
         {computed:.9} (target x combined derates). A floor lift moved the feed; \
         ruling R4 WP2a removed it."
    );
    assert!(
        fpt >= bounds.min_mm_per_tooth && fpt <= bounds.max_mm_per_tooth * (1.0 + 1e-9),
        "the shipped advance {fpt:.6} is outside the derated band {:.6}..{:.6}",
        bounds.min_mm_per_tooth,
        bounds.max_mm_per_tooth
    );
    assert!(
        !result
            .warnings
            .iter()
            .any(|w| matches!(w, FeedsWarning::ChiploadBelowRubbingFloor { .. })),
        "a recipe inside the published band must not warn. warnings: {:?}",
        result.warnings
    );
}

#[test]
fn a_ceiling_that_pushes_the_advance_under_the_band_minimum_warns_and_does_not_lift() {
    let mut machine = MachineProfile::generic_wood_router();
    machine.max_cutting_feed_mm_min = Some(50.0);
    let result = b3_scallop_on(&machine);
    let fpt = commanded_fpt(&result, 2.0);
    let bounds = band(&result);

    let warning = result.warnings.iter().find_map(|w| match w {
        FeedsWarning::ChiploadBelowRubbingFloor {
            commanded,
            floor,
            source,
            band_min,
            band_max,
        } => Some((*commanded, *floor, *source, *band_min, *band_max)),
        _ => None,
    });

    println!(
        "B3 scallop, 50 mm/min ceiling: rpm={:.0} feed={:.3} fpt={:.6} \
         band={:.6}..{:.6} warning={warning:?}",
        result.rpm, result.feed_rate_mm_min, fpt, bounds.min_mm_per_tooth, bounds.max_mm_per_tooth,
    );

    let (commanded, floor, source, band_min, band_max) = warning.unwrap_or_else(|| {
        panic!(
            "the 50 mm/min ceiling puts the advance under the band minimum, so the \
             warning must fire. warnings: {:?}",
            result.warnings
        )
    });

    assert!(
        result.feed_rate_mm_min <= 50.0 + 1e-9,
        "the feed {:.3} is above the 50 mm/min ceiling; the floor must not lift it",
        result.feed_rate_mm_min
    );
    assert!(
        (commanded - fpt).abs() <= fpt.abs() * 1e-9,
        "the warning reports {commanded:.9} mm/tooth but the recipe ships {fpt:.9}"
    );
    assert!(
        commanded < floor,
        "the warning fired at {commanded:.6}, which is not below its floor {floor:.6}"
    );
    assert_eq!(source, RubbingFloorSource::BandMinimum);
    assert_eq!(floor, bounds.min_mm_per_tooth);
    assert_eq!(band_min, Some(bounds.min_mm_per_tooth));
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
    // the 0.025 floor, so `min(0.025, band_min)` is the constant and
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
