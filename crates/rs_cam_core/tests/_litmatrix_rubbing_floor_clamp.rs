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
//! ## RE-PINNED TWICE ON 2026-08-06 — this cell pins a floor×band
//! ## INTERACTION, and both sides moved on the same day
//!
//! `LAW_MAGNITUDE_TABLES.md` §5.1 named this file as the sharpest
//! consequence of the whole feeds programme precisely because it asserts
//! a *relationship* between a global constant and a scaled vendor band,
//! and two separate rulings moved one side each. Both passes are
//! recorded; the middle column is a real committed state, not a
//! hypothetical.
//!
//! | | parent `777a78b` | after floor (`2d1bfc8`) | after laws |
//! |---|---:|---:|---:|
//! | derated band (mm/tooth) | 0.013219–0.022721 | unchanged | **0.020567–0.035350** |
//! | pre-clamp `requested` | 0.014128 | unchanged | **0.021982** |
//! | floor applied | 0.025000 | **0.022721** | **0.025000** |
//! | commanded feed-per-tooth | 0.025000 | **0.022721** | 0.025000 |
//! | feed (mm/min @ 15 000 rpm × 2F) | 750.00 | **681.62** | 750.00 |
//! | `ChiploadClampedToFloor` fires | yes | yes | **yes** |
//! | `band_capped_from` | *(no field)* | `Some(0.025)` | **`None`** |
//!
//! **Pass 1 — the floor is subordinated to the band** (FEEDS_CENSUS
//! C-12 / T3.3; `feeds::effective_rubbing_floor`). This cell was one of
//! the crossings the census found: at `^1.0` hardness scaling the Ipe
//! band topped out at 0.022721 and the global floor sat 1.10× above it,
//! so the clamp was lifting the recipe past the vendor window's own
//! ceiling. The clamp target became the band ceiling and the recipe got
//! slower.
//!
//! **Pass 2 — the hardness law moves the band out from under the
//! floor.** Adopting `Janka^-0.5` scales this row by `0.3675^0.5 =
//! 0.6062` instead of `0.3675`, i.e. **×1.556** — exactly the
//! multiplier `LAW_MAGNITUDE_TABLES.md` §5 measured in advance
//! (0.01322–0.02272 → 0.02057–0.03535). The band ceiling 0.035350 now
//! clears the 0.025 floor, `effective_rubbing_floor` returns the global
//! constant again, and every number in this cell returns to its parent
//! value.
//!
//! ### The predicted sentry break did NOT happen, and the prediction
//! ### was wrong for a stateable reason
//!
//! `LAW_MAGNITUDE_TABLES.md` §5.1 predicted that under the hardness law
//! *"the clamp would then **stop firing**,
//! `ipe_pocket_emits_chipload_clamped_warning` would go RED"*. Measured
//! here, **it still fires**. The prediction reasoned from the band
//! *midpoint* Suggest reads (0.01797 → 0.02796, above 0.025). The
//! quantity the Step-9b clamp actually tests is the **commanded
//! feed-per-tooth after the safety-factor, LD-overhang and power
//! derates**, which is 0.021982 — still below the floor. A ×1.556 band
//! move was not enough to lift a value that had already been derated by
//! ~21 % below its own midpoint. Published as failed rather than
//! quietly satisfied: the document's *direction* was right, its
//! *conclusion* was not, and the difference is the derate chain between
//! the band and the feed.
//!
//! Net effect of both passes on this cell: **no change**. That is not
//! the same as nothing happening — it is two independent moves that
//! cancel, and the file asserts each of them so the cancellation cannot
//! be mistaken for inertness.
//!
//! The **oak control** did move, slightly: `h_scale` 1.0662 → 1.0326
//! (×0.968), band 0.034118–0.058640 → 0.033042–0.056791, feed-per-tooth
//! 0.036464 → 0.035314. It still never clamps, which is what it is for.

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
/// hardness ratio 1290/3510 = 0.3675 applied at `^0.5`). Pinned as a
/// literal so a change to the scaling law shows up here as a diff and
/// not as a silently-tracking assertion. **Was 0.022720797720797720**
/// under the retired `^1.0` hardness law; ×1.556.
const IPE_DERATED_BAND_MAX_MM_TOOTH: f64 = 0.035_350_302_327_474_86;

/// The commanded feed-per-tooth Step 9b sees for this cell, *before* the
/// clamp — i.e. after the safety-factor, LD-overhang and power derates
/// have been applied to the band-derived target. Pinned because it is
/// the quantity `LAW_MAGNITUDE_TABLES.md` §5.1's prediction confused
/// with the band midpoint: the midpoint went above the floor and this
/// did not, which is why the clamp still fires. Was 0.014128326084665594
/// under the retired `^1.0` law.
///
/// **RE-PINNED 2026-08-19 (G-CHIPTHIN-HALFFIX): 0.021981648910896722 →
/// 0.020969157...**, a factor of exactly **1.0483** — this cell's
/// `observed_combined_chip_thinning`, which the calculator no longer
/// multiplies into the feed. The cell's *conclusion* is unchanged and that is
/// the point: the pre-clamp advance was already below the chip-formation floor
/// and it moved further below, so the clamp still fires and the cell still
/// demonstrates what it was written to demonstrate. Only the magnitude moved,
/// and it moved by the deleted multiplier exactly.
const IPE_PRE_CLAMP_REQUESTED_MM_TOOTH: f64 = 0.020_969_157_101_952_5;

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
        operation_kind: None,
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
    // RE-PINNED TWICE 2026-08-06 — see this file's docstring table.
    // Pass 1 (floor subordination) moved the clamp target down to the
    // band ceiling 0.022721; pass 2 (`Janka^-0.5`) moved the ceiling up
    // to 0.035350, which clears the global floor, so the global constant
    // applies again and the value returns to 0.025. Both moves are
    // asserted so the net-zero is a measured cancellation, not inertness.
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
        band.max_mm_per_tooth > RUBBING_FLOOR_MM_TOOTH,
        "fixture precondition, INVERTED by the hardness law: the Ipe band ceiling \
         {:.6} must now sit ABOVE the {RUBBING_FLOOR_MM_TOOTH} global floor, so this \
         cell exercises the un-subordinated branch of `effective_rubbing_floor`. \
         Under the retired ^1.0 law it sat below, at 0.022721.",
        band.max_mm_per_tooth,
    );

    let effective_floor = RUBBING_FLOOR_MM_TOOTH.min(band.max_mm_per_tooth);
    assert!(
        (effective_floor - RUBBING_FLOOR_MM_TOOTH).abs() < 1e-12,
        "with the band clear of the floor the effective floor IS the global constant",
    );
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
                "Expected FeedsWarning::ChiploadClampedToFloor for Ipe pocket cell. \
                 LAW_MAGNITUDE_TABLES.md §5.1 predicted this assertion would go RED \
                 under `Janka^-0.5`, reasoning from the band MIDPOINT (0.01797 → \
                 0.02796, above the floor). The clamp tests the COMMANDED \
                 feed-per-tooth after the safety/LD/power derates, which is \
                 {IPE_PRE_CLAMP_REQUESTED_MM_TOOTH:.6} — still below \
                 {RUBBING_FLOOR_MM_TOOTH}. If this panic ever fires, that prediction \
                 has finally come true and the cell must move to a harder or smaller \
                 combination. Warnings: {:?}",
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
        (requested - IPE_PRE_CLAMP_REQUESTED_MM_TOOTH).abs() < 1e-9,
        "RE-PINNED 2026-08-06 (was 0.014128326084665594 under the retired ^1.0 \
         hardness law): the pre-clamp commanded feed-per-tooth is \
         {IPE_PRE_CLAMP_REQUESTED_MM_TOOTH:.9}. Got {requested:.9}.",
    );
    assert!(
        (floor - RUBBING_FLOOR_MM_TOOTH).abs() < 1e-12,
        "RE-PINNED TWICE 2026-08-06. The floor commit made this the band ceiling \
         0.022720797720797720; the hardness law lifted the ceiling to \
         {IPE_DERATED_BAND_MAX_MM_TOOTH:.9}, clear of the global floor, so the \
         applied floor is the global {RUBBING_FLOOR_MM_TOOTH} again. Got {floor:.9}.",
    );
    assert_eq!(
        band_capped_from, None,
        "the band no longer caps the floor on this cell, so nothing is disclosed. \
         `Some(_)` here would mean the hardness law stopped clearing the band over \
         the floor — see the docstring table.",
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
        operation_kind: None,
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
