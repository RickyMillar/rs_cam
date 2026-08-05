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
//!
//! ## RE-PIN 2026-08-06 — the floor is subordinated to the band
//!
//! `FEEDS_CENSUS.md` C-12 / T3.3 established that the global constant
//! and the matched row's derated band cross on hard and small work, and
//! the operator ruled the floor may never exceed the band ceiling it
//! exists to keep the recipe inside (`feeds::effective_rubbing_floor`).
//!
//! **This cell was one of the crossings.** Ipe scales the Ø6
//! hardwood-anchored row's band to 0.013219–0.022721 mm/tooth, so the
//! 0.025 global floor sat **1.10× above the band maximum** — the clamp
//! was lifting the Ipe recipe past the vendor window's own ceiling.
//! Measured through `feeds::calculate` on this fixture:
//!
//! | | before (parent) | after |
//! |---|---:|---:|
//! | derated band (mm/tooth) | 0.013219 – 0.022721 | unchanged |
//! | pre-clamp `requested` | 0.014128 | unchanged |
//! | floor applied | **0.025000** | **0.022721** (band ceiling) |
//! | commanded feed-per-tooth | **0.025000** | **0.022721** |
//! | feed (mm/min @ 15 000 rpm, 2F) | **750.00** | **681.62** |
//! | `ChiploadClampedToFloor` fires | yes | yes |
//! | `band_capped_from` | *(field did not exist)* | `Some(0.025)` |
//!
//! Mechanism: `feeds::calculate` Step 9b now clamps to
//! `min(RUBBING_FLOOR_MM_TOOTH, chipload_bounds.max_mm_per_tooth)`
//! instead of the bare constant. Direction: the Ipe recipe gets
//! **slower**, which is the conservative side on the breakage axis and
//! the *un*conservative side on the burn axis — hence the new
//! `band_capped_from` disclosure, asserted below, which tells the
//! operator the recipe is still under the chip-formation threshold and
//! no feed exists that is not.
//!
//! The cell's assertion is restated accordingly: it no longer pins the
//! literal 0.025 (which this row cannot reach without leaving its band)
//! but pins **the clamp firing and landing exactly on the band
//! ceiling**, which is what the cell was always testing — that the
//! engine does not serve a silently-derated ploughing recipe.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]

use rs_cam_core::feeds::{
    FeedsInput, FeedsWarning, OperationFamily, PassRole, SetupContext, SpindleStrategy,
    ToolGeometryHint, calculate, embedded_vendor_lut,
};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, WoodSpecies};

/// The literature value, re-declared locally on purpose: a
/// literature-matrix cell must pin the published number independently
/// of whatever the crate currently believes it is.
const RUBBING_FLOOR_MM_TOOTH: f64 = 0.025;

/// The Ipe cell's derated band ceiling, measured 2026-08-06 through
/// `feeds::calculate` (row `amana-flat-hardwood-pocket-6000-2f`, raw
/// hardness ratio 1290/3510 = 0.3675 applied at `^1.0`). Pinned as a
/// literal so a change to the scaling law shows up here as a diff and
/// not as a silently-tracking assertion.
const IPE_DERATED_BAND_MAX_MM_TOOTH: f64 = 0.022_720_797_720_797_72;

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

/// Records the cell's live numbers so the re-pin table in this file's
/// docstring can be re-measured rather than trusted.
/// `cargo test -p rs_cam_core --test _litmatrix_rubbing_floor_clamp -- --nocapture`
#[test]
fn record_the_cell() {
    for (label, result) in [("ipe", calc_ipe_6mm()), ("oak", calc_oak_6mm())] {
        let fpt = result.feed_rate_mm_min / (result.rpm * 2.0);
        println!(
            "{label}: rpm={:.0} feed={:.4} fpt={:.6} band={:?} warnings={:?}",
            result.rpm, result.feed_rate_mm_min, fpt, result.chipload_bounds, result.warnings
        );
    }
}

#[test]
fn ipe_pocket_chipload_never_drops_below_the_effective_rubbing_floor() {
    // RE-PINNED 2026-08-06 (was: `>= 0.025`, the bare global constant).
    // The Ipe-derated band tops out at 0.022721, BELOW the global floor,
    // so 0.025 is not reachable without commanding past the vendor
    // window. The clamp target is now the band ceiling and this cell
    // pins that value.
    let result = calc_ipe_6mm();
    let rpm = result.rpm;
    let flutes = 2.0_f64;

    assert!(rpm > 0.0, "engine produced rpm = {rpm}");

    let band = result
        .chipload_bounds
        .expect("the Ipe cell matches a chipload-bearing row");
    assert!(
        (band.max_mm_per_tooth - IPE_DERATED_BAND_MAX_MM_TOOTH).abs() < 1e-9,
        "the cell's derated band ceiling moved: {:.9} vs pinned {IPE_DERATED_BAND_MAX_MM_TOOTH:.9}. \
         A scaling-law change must re-pin this file, not slide past it.",
        band.max_mm_per_tooth,
    );
    assert!(
        band.max_mm_per_tooth < RUBBING_FLOOR_MM_TOOTH,
        "fixture precondition: this cell exists because the Ipe band ceiling \
         {:.6} sits BELOW the {RUBBING_FLOOR_MM_TOOTH} global floor. If that is no \
         longer true the cell is testing something else.",
        band.max_mm_per_tooth,
    );

    let effective_floor = RUBBING_FLOOR_MM_TOOTH.min(band.max_mm_per_tooth);
    let chipload = result.feed_rate_mm_min / (rpm * flutes);
    assert!(
        chipload >= effective_floor - 1e-9,
        "Ipe-derated chipload {chipload:.6} fell below the effective rubbing floor \
         {effective_floor:.6} (feed_rate={}, rpm={rpm}, flutes={flutes}). \
         The Step-9b rubbing-floor clamp regressed.",
        result.feed_rate_mm_min,
    );
    assert!(
        chipload <= band.max_mm_per_tooth + 1e-9,
        "the clamp raised Ipe chipload to {chipload:.6}, past the matched row's \
         derated band maximum {:.6} — the floor is once again overshooting the \
         band it protects (FEEDS_CENSUS C-12).",
        band.max_mm_per_tooth,
    );
}

#[test]
fn ipe_pocket_emits_chipload_clamped_warning() {
    let result = calc_ipe_6mm();
    let clamp = result
        .warnings
        .iter()
        .find_map(|w| match w {
            FeedsWarning::ChiploadClampedToFloor {
                requested,
                floor,
                band_capped_from,
            } => Some((*requested, *floor, *band_capped_from)),
            _ => None,
        })
        .unwrap_or_else(|| {
            panic!(
                "Expected FeedsWarning::ChiploadClampedToFloor for Ipe pocket cell \
                 (pre-clamp chipload ~0.0141 < band ceiling 0.0227). Warnings: {:?}",
                result.warnings,
            )
        });
    let (requested, floor, band_capped_from) = clamp;

    assert!(
        requested < floor,
        "the warning must report a genuine clamp: requested {requested:.6} \
         should be below the applied floor {floor:.6}",
    );
    assert!(
        (floor - IPE_DERATED_BAND_MAX_MM_TOOTH).abs() < 1e-9,
        "RE-PINNED 2026-08-06: the applied floor is the band ceiling \
         {IPE_DERATED_BAND_MAX_MM_TOOTH:.9}, not the global \
         {RUBBING_FLOOR_MM_TOOTH}. Got {floor:.9}.",
    );
    assert_eq!(
        band_capped_from,
        Some(RUBBING_FLOOR_MM_TOOTH),
        "the operator must be told the global chip-formation threshold was \
         NOT reached — this recipe is still in the rubbing regime and no feed \
         inside the vendor band escapes it.",
    );
}

fn calc_oak_6mm() -> rs_cam_core::feeds::FeedsResult {
    let lut = embedded_vendor_lut();
    let machine = MachineProfile::generic_wood_router();
    let material = Material::SolidWood {
        species: WoodSpecies::WhiteOak,
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
fn oak_pocket_chipload_above_floor_is_not_clamped() {
    // Anti-regression: a normal-Janka species (oak) must NOT trigger
    // the clamp warning — the LUT chipload sits comfortably above the
    // floor and the engine should pass it through verbatim. This locks
    // in that the clamp only fires on the rubbing-floor edge case.
    let result = calc_oak_6mm();

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
