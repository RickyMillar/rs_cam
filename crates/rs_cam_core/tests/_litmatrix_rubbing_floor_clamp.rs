//! Literature-matrix regression — the 0.025 mm/tooth rubbing floor WARNS
//! when an extreme-Janka material derates a vendor row below the
//! chip-formation threshold. It does not lift the feed.
//!
//! ## RE-PINNED 2026-09-23 — ruling R4 WP2a, and the cell moved
//!
//! Ruling R4 WP2a (`planning/feeds_matrix_2026-09-23/R4_AGGRESSIVENESS_SPEC.md`
//! §3.6) removed the Step-9b lift. The warning stays; the recipe ships its
//! computed feed. The two Ipe arms therefore read "warns, feed unchanged"
//! where they read "clamped up to the floor".
//!
//! The cell also moved, twice. On the tree of 2026-09-23 the Ipe Ø6 flat
//! pocket with the default setup no longer reaches the floor: R3 (aef54c83,
//! one linear depth scale) and the R5 printed rows (3885c8bf..bab9a236) put
//! the derated band at a maximum of 0.046088898 mm/tooth (was 0.035350302)
//! and the commanded advance above 0.025, so no warning fired.
//!
//! Measured by the verifier (2026-09-23), the chain:
//!
//! | setup | L/D | workholding | advance (mm/tooth) | warns |
//! |---|---:|---:|---:|---|
//! | default (no stickout, Medium) | 1.00 | 1.00 | 0.034567 (= 0.025925 / 0.75) | no |
//! | stickout 45 mm, Medium | 0.75 | 1.00 | **0.025925** | no (0.0009 above) |
//! | stickout 45 mm, **Low** | 0.75 | 0.85 | 0.025925 x 0.85 = **0.022036** | **yes** |
//!
//! The first move (stickout 45 mm, the stickout `ToolConfig::new_default`
//! gives every GUI tool) left the cell 0.0009 above the floor. The second
//! move adds `WorkholdingRigidity::Low`, the 0.85 workholding factor, a
//! real setup (a part held by tape or a vacuum table). That puts the cell
//! 12 % under the floor. The arms pin the relation (0.75 x 0.85 of the
//! default-setup advance) and the magnitude to 1e-5.
//!
//! ## History before 2026-09-23
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
//! | | parent `91f3580` | after floor (`aff98bd`) | after laws |
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
    ToolGeometryHint, WorkholdingRigidity, calculate, embedded_vendor_lut,
};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, WoodSpecies};

/// The literature value, re-declared locally on purpose: a
/// literature-matrix cell must pin the published number independently
/// of whatever the crate currently believes it is.
const RUBBING_FLOOR_MM_TOOTH: f64 = 0.025;

/// The Ipe cell's derated band ceiling. Pinned as a literal so a change to
/// the scaling law shows up here as a diff and not as a silently-tracking
/// assertion.
///
/// **RE-PINNED 2026-09-23: 0.035350302 → 0.046088898** (x1.304), measured by
/// the verifier on the tree after R3 (aef54c83, the linear depth scale) and
/// the R5 printed rows (3885c8bf..bab9a236). Before that: measured
/// 2026-08-06 (row `amana-flat-hardwood-pocket-6000-2f`, raw hardness ratio
/// 1290/3510 = 0.3675 applied at `^0.5`); **was 0.022720797720797720**
/// under the retired `^1.0` hardness law. The stickout does not enter the
/// band, so the moved cell reads the same ceiling.
const IPE_DERATED_BAND_MAX_MM_TOOTH: f64 = 0.046_088_898;

/// The pin above came from a nine-decimal print, so it can be off by up to
/// 5e-10. The tolerance is ten times that.
const BAND_PIN_TOLERANCE: f64 = 5e-9;

/// The stickout `ToolConfig::new_default` gives every GUI tool (mm). On the
/// Ø6 cell it is L/D 7.5, above the 6 x D threshold of the long-tool de-rate.
const DEFAULT_TOOL_STICKOUT_MM: f64 = 45.0;

/// The long-tool de-rate factor above 6 x D (`feeds/mod.rs`, Step 5b).
const LONG_TOOL_FACTOR: f64 = 0.75;

/// The low-rigidity workholding factor (`feeds/mod.rs`, Step 5b).
const LOW_WORKHOLDING_FACTOR: f64 = 0.85;

/// The moved cell's advance, 0.025925 x 0.85, from the verifier's measured
/// 0.025925 mm/tooth at stickout 45 mm and Medium workholding (a six-decimal
/// print, so the pin carries a 1e-5 tolerance).
const IPE_MOVED_CELL_ADVANCE_MM_TOOTH: f64 = 0.022_036;

fn calc_ipe_6mm(setup: SetupContext) -> rs_cam_core::feeds::FeedsResult {
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
        setup,
        spindle_strategy: SpindleStrategy::MatchChart,
    })
}

/// The moved cell: the Ipe Ø6 pocket with the default GUI stickout and
/// low-rigidity workholding.
fn calc_ipe_6mm_long_tool() -> rs_cam_core::feeds::FeedsResult {
    calc_ipe_6mm(SetupContext {
        tool_overhang_mm: Some(DEFAULT_TOOL_STICKOUT_MM),
        workholding_rigidity: WorkholdingRigidity::Low,
    })
}

/// Records the cell's live numbers so the re-pin table in this file's
/// docstring can be re-measured rather than trusted.
/// `cargo test -p rs_cam_core --test _litmatrix_rubbing_floor_clamp -- --nocapture`
#[test]
fn record_the_cell() {
    for (label, result) in [
        ("ipe", calc_ipe_6mm(SetupContext::default())),
        ("ipe long tool", calc_ipe_6mm_long_tool()),
        ("oak", calc_oak_6mm()),
    ] {
        let fpt = result.feed_rate_mm_min / (result.rpm * 2.0);
        println!(
            "{label}: rpm={:.0} feed={:.4} fpt={:.6} band={:?} warnings={:?}",
            result.rpm, result.feed_rate_mm_min, fpt, result.chipload_bounds, result.warnings
        );
    }
}

#[test]
fn ipe_pocket_ships_its_computed_feed_under_the_floor() {
    // RE-PINNED 2026-09-23 (ruling R4 WP2a). This arm read "never drops
    // below the effective rubbing floor"; the lift that made that true is
    // gone. It now pins that the moved cell (Ipe Ø6, stickout 45 mm) ships
    // the advance its derates compute, and that the advance is under the
    // floor, which is what makes the warning arm below meaningful.
    let result = calc_ipe_6mm_long_tool();
    let rpm = result.rpm;
    let flutes = 2.0_f64;

    assert!(rpm > 0.0, "engine produced rpm = {rpm}");

    let band = result
        .chipload_bounds
        .expect("the Ipe cell matches a chipload-bearing row");
    assert!(
        (band.max_mm_per_tooth - IPE_DERATED_BAND_MAX_MM_TOOTH).abs() < BAND_PIN_TOLERANCE,
        "the cell's derated band ceiling moved: {:.9} vs pinned {IPE_DERATED_BAND_MAX_MM_TOOTH:.9}. \
         A scaling-law change must re-pin this file, not slide past it.",
        band.max_mm_per_tooth,
    );
    assert!(
        band.max_mm_per_tooth > RUBBING_FLOOR_MM_TOOTH,
        "fixture precondition: the Ipe band ceiling {:.6} sits ABOVE the \
         {RUBBING_FLOOR_MM_TOOTH} global floor, so this cell exercises the \
         un-subordinated branch of `effective_rubbing_floor`.",
        band.max_mm_per_tooth,
    );

    let chipload = result.feed_rate_mm_min / (rpm * flutes);
    let computed = result.derates.effective_chip_load_mm();
    assert!(
        (chipload - computed).abs() <= computed * 1e-9,
        "the shipped advance {chipload:.9} is not the computed advance \
         {computed:.9}: a floor lift moved the feed, which ruling R4 WP2a removed.",
    );
    assert!(
        chipload < RUBBING_FLOOR_MM_TOOTH,
        "the moved cell must sit under the floor: advance {chipload:.6} vs \
         {RUBBING_FLOOR_MM_TOOTH}. If it does not, move the cell again (a harder \
         species or a smaller tool), as this file's docstring says.",
    );

    assert!(
        (chipload - IPE_MOVED_CELL_ADVANCE_MM_TOOTH).abs() < 1e-5,
        "the moved cell's advance moved: {chipload:.6} vs pinned \
         {IPE_MOVED_CELL_ADVANCE_MM_TOOTH} (0.025925 x 0.85)",
    );

    // The move is the long-tool and workholding factors and nothing else:
    // the default-setup cell's advance times 0.75 x 0.85. Neither cell
    // touches the machine ceiling or the power ladder at this feed.
    let short = calc_ipe_6mm(SetupContext::default());
    let short_chipload = short.feed_rate_mm_min / (short.rpm * flutes);
    let expected = short_chipload * LONG_TOOL_FACTOR * LOW_WORKHOLDING_FACTOR;
    assert!(
        (chipload - expected).abs() <= short_chipload * 1e-9,
        "the moved-cell advance {chipload:.9} is not {LONG_TOOL_FACTOR} x \
         {LOW_WORKHOLDING_FACTOR} x the default-setup advance {short_chipload:.9}",
    );
    assert_eq!(result.derates.workholding, LOW_WORKHOLDING_FACTOR);
    assert_eq!(
        result.derates.ld_overhang, LONG_TOOL_FACTOR,
        "L/D 7.5 takes the 0.75 long-tool factor"
    );
}

#[test]
fn ipe_pocket_emits_chipload_below_floor_warning() {
    let result = calc_ipe_6mm_long_tool();
    let warning = result
        .warnings
        .iter()
        .find_map(|w| match w {
            FeedsWarning::ChiploadBelowRubbingFloor {
                commanded,
                floor,
                band_max,
            } => Some((*commanded, *floor, *band_max)),
            _ => None,
        })
        .unwrap_or_else(|| {
            panic!(
                "Expected FeedsWarning::ChiploadBelowRubbingFloor for the Ipe Ø6 pocket \
                 at stickout {DEFAULT_TOOL_STICKOUT_MM} mm, Low workholding. \
                 Warnings: {:?}",
                result.warnings,
            )
        });
    let (commanded, floor, band_max) = warning;
    let shipped = result.feed_rate_mm_min / (result.rpm * 2.0);

    assert!(
        commanded < floor,
        "the warning must report a genuine sub-floor advance: {commanded:.6} \
         should be below the floor {floor:.6}",
    );
    assert!(
        (commanded - shipped).abs() <= shipped * 1e-9,
        "the warning reports {commanded:.9} but the recipe ships {shipped:.9}: \
         the feed must ship unchanged (ruling R4 WP2a)",
    );
    assert!(
        (floor - RUBBING_FLOOR_MM_TOOTH).abs() < 1e-12,
        "the band ceiling {IPE_DERATED_BAND_MAX_MM_TOOTH:.9} clears the global floor, \
         so the floor is the global {RUBBING_FLOOR_MM_TOOTH}. Got {floor:.9}.",
    );
    assert!(
        band_max.is_some_and(|m| (m - IPE_DERATED_BAND_MAX_MM_TOOTH).abs() < BAND_PIN_TOLERANCE),
        "the warning carries the band maximum that the floor read: {band_max:?}",
    );
    assert!(
        result
            .warnings
            .iter()
            .any(|w| matches!(w, FeedsWarning::LongToolDerate { .. })),
        "the cause of the sub-floor advance, the long-tool de-rate, is visible \
         too (R4 WP1). Warnings: {:?}",
        result.warnings,
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
fn oak_pocket_chipload_above_floor_does_not_warn() {
    // Anti-regression: a normal-Janka species (oak) must NOT trigger
    // the clamp warning — the LUT chipload sits comfortably above the
    // floor and the engine should pass it through verbatim. This locks
    // in that the clamp only fires on the rubbing-floor edge case.
    let result = calc_oak_6mm();

    let has_clamp_warning = result
        .warnings
        .iter()
        .any(|w| matches!(w, FeedsWarning::ChiploadBelowRubbingFloor { .. }));
    assert!(
        !has_clamp_warning,
        "Oak pocket should not have triggered ChiploadBelowRubbingFloor; \
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
