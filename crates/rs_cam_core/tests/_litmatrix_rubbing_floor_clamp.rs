//! Literature-matrix regression — the 0.025 mm/tooth rubbing floor and the
//! extreme-Janka Ipe cell.
//!
//! ## RE-PINNED 2026-09-24 — ruling R4 Q8: no feed factor is left on the cell
//!
//! Ruling R4 Q8 deleted the workholding rigidity and its 0.85 / 1.00 / 1.03
//! feed factor. The machine aggressiveness dial is the one load margin. The
//! moved cell lost its `Low` workholding, because the selector is gone.
//! Predicted chain:
//!
//! | setup | feed factors | advance (mm/tooth) | warns |
//! |---|---|---:|---|
//! | default (no stickout) | none | 0.034567 / 0.75 = **0.046089** (0.048864 since P2 step 3, one Janka table) | no |
//! | stickout 45 mm | none | **0.046089** (x 1/0.85 against 0.039176) | no |
//!
//! Q8 changes the moved cell's advance by 1/0.85. It changes no warning: the
//! cell was already above the floor after WP3. No lever that the repository
//! still has puts this cell under the floor without a new species, tool or
//! row, so the arms pin "above the floor, equal to the default-setup
//! advance". The floor warning itself is pinned by
//! `rubbing_floor_warns_and_never_lifts`.
//!
//! ## RE-PINNED 2026-09-24 (earlier) — ruling R4 WP3: the cell no longer reaches the floor
//!
//! Ruling R4 WP3 removed the 0.75 machine safety factor from the feed, and
//! Q7 moved the long-tool share out of the feed into the dial's load target.
//! The moved cell (stickout 45 mm, Low workholding) kept only the 0.85
//! workholding factor (Q8 belongs to a later package). Predicted chain:
//!
//! | setup | feed factors | advance (mm/tooth) | warns |
//! |---|---|---:|---|
//! | default (no stickout, Medium) | none | 0.034567 / 0.75 = **0.046089** | no |
//! | stickout 45 mm, **Low** | 0.85 | 0.046089 x 0.85 = **0.039176** | **no** |
//!
//! So the Ipe cell now ships above the floor, and the arms pin that: the
//! advance, the relation (0.85 x the default-setup advance), and no floor
//! warning. The long-tool share 0.75 is still on the record
//! (`LongToolDerate`, `derates.ld_overhang`) as a load target, not a feed
//! factor. The floor warning itself is pinned by
//! `rubbing_floor_warns_and_never_lifts`.
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

use rs_cam_core::feeds::vendor_lookup::PrintedChipload;
use rs_cam_core::feeds::{
    FeedsInput, FeedsWarning, OperationFamily, PassRole, RubbingFloorSource, SetupContext,
    SpindleStrategy, ToolGeometryHint, calculate, embedded_vendor_lut, rubbing_floor_at,
};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, WoodSpecies};

/// The literature value, re-declared locally on purpose: a
/// literature-matrix cell must pin the published number independently
/// of whatever the crate currently believes it is.
const RUBBING_FLOOR_MM_TOOTH: f64 = 0.025;

/// The Ipe cell's derated printed point (A2, point mode). Pinned as a
/// literal so a change to the scaling law shows up here as a diff and not as
/// a silently-tracking assertion.
///
/// **RE-PREMISED 2026-09-24 for A2 (point mode); the number does not move.**
/// The row that the cell matches is `amana-compression-wood-pocket-6350-2f`,
/// which prints one value, 0.0787 mm/tooth (min == max). The derivation:
///
/// - diameter scale (6.0 / 6.35)^0.61 = 0.966007037
/// - hardness scale (1450 / 3510)^0.5 = 0.642732770
/// - point 0.0787 x 0.966007037 x 0.642732770 = 0.048863601
/// - depth de-rate 1.0 (the shipped depth is at or below 1 x D)
///
/// Before A2 the row reached the calculator as a zero-width band
/// 0.048863601..0.048863601. After A2 it is a point:
/// `chipload_bounds` is `None` and `chipload_point_mm` carries the value.
/// The earlier notes below name `amana-flat-hardwood-pocket-6000-2f`
/// (0.032..0.055); that row gives 0.055 x 0.642732770 = 0.035350302, the
/// pre-R5 value, so the R5 re-pin was already the compression row.
///
/// **RE-PINNED 2026-09-24: 0.046088898 → 0.048863601** (x1.0602 =
/// (1450/1290)^0.5), measured after extrapolation P2 step 3: the row
/// `amana-flat-hardwood-pocket-6000-2f` carries no Janka, and its family
/// default is now the query table's GenericHardwood 1450 (was 1290), so the
/// Ipe transfer starts from 1450/3510.
///
/// **RE-PINNED 2026-09-23: 0.035350302 → 0.046088898** (x1.304), measured by
/// the verifier on the tree after R3 (aef54c83, the linear depth scale) and
/// the R5 printed rows (3885c8bf..bab9a236). Before that: measured
/// 2026-08-06 (row `amana-flat-hardwood-pocket-6000-2f`, raw hardness ratio
/// 1290/3510 = 0.3675 applied at `^0.5`); **was 0.022720797720797720**
/// under the retired `^1.0` hardness law. The stickout does not enter the
/// band, so the moved cell reads the same ceiling.
const IPE_DERATED_POINT_MM_TOOTH: f64 = 0.048_863_601;

/// The row that the Ipe cell matches (see [`IPE_DERATED_POINT_MM_TOOTH`]).
const IPE_ROW_ID: &str = "amana-compression-wood-pocket-6350-2f";

/// The pin above came from a nine-decimal print, so it can be off by up to
/// 5e-10. The tolerance is ten times that.
const BAND_PIN_TOLERANCE: f64 = 5e-9;

/// The stickout `ToolConfig::new_default` gives every GUI tool (mm). On the
/// Ø6 cell it is L/D 7.5, above the 6 x D threshold of the long-tool de-rate.
const DEFAULT_TOOL_STICKOUT_MM: f64 = 45.0;

/// The long-tool share above 6 x D (`feeds::long_tool_load_share`). Since
/// ruling R4 Q7 it is a load target, not a feed factor.
const LONG_TOOL_FACTOR: f64 = 0.75;

/// The moved cell's advance after ruling R4 Q8: 0.034567 / 0.75 = 0.046089,
/// from the verifier's measured 0.034567 mm/tooth at the default setup
/// before WP3 (a six-decimal print, so the pin carries a 1e-5 tolerance).
/// PREDICTED, not yet measured. Was 0.039176 (x 0.85 workholding, WP3) and
/// before that 0.022036 (x 0.75 L/D x 0.75 safety). RE-MEASURE if it moves.
/// RE-MEASURED 2026-09-24 after extrapolation P2 step 3 (one Janka table:
/// the row's hardwood default is 1450, was 1290): 0.048864 = 0.046089 x
/// (1450/1290)^0.5. The advance sits at the derated printed point (A2).
const IPE_MOVED_CELL_ADVANCE_MM_TOOTH: f64 = 0.048_864;

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

/// The moved cell: the Ipe Ø6 pocket with the default GUI stickout.
fn calc_ipe_6mm_long_tool() -> rs_cam_core::feeds::FeedsResult {
    calc_ipe_6mm(SetupContext {
        tool_overhang_mm: Some(DEFAULT_TOOL_STICKOUT_MM),
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
            "{label}: rpm={:.0} feed={:.4} fpt={:.6} band={:?} point={:?} warnings={:?}",
            result.rpm,
            result.feed_rate_mm_min,
            fpt,
            result.chipload_bounds,
            result.chipload_point_mm,
            result.warnings
        );
    }
}

#[test]
fn ipe_pocket_ships_its_computed_feed_above_the_floor_after_r4() {
    // RE-PINNED 2026-09-24 (ruling R4 WP3, then Q8). See the file doc for the
    // chain.
    let result = calc_ipe_6mm_long_tool();
    let rpm = result.rpm;
    let flutes = 2.0_f64;

    assert!(rpm > 0.0, "engine produced rpm = {rpm}");

    // A2 (point mode): the row prints one value, so the cell has a point
    // and no band.
    let row = result
        .matched_lut_row
        .as_ref()
        .expect("the Ipe cell matches a vendor row");
    assert_eq!(row.observation_id, IPE_ROW_ID, "the Ipe cell's row moved");
    assert!(
        matches!(row.printed_chipload(), PrintedChipload::Point { .. }),
        "the Ipe row prints one value, so it is a point: {:?}",
        row.printed_chipload(),
    );
    assert!(
        result.chipload_bounds.is_none(),
        "a point is never a band: {:?}",
        result.chipload_bounds,
    );
    let point = result
        .chipload_point_mm
        .expect("the Ipe cell carries the printed point");
    assert!(
        (point - IPE_DERATED_POINT_MM_TOOTH).abs() < BAND_PIN_TOLERANCE,
        "the cell's derated point moved: {point:.9} vs pinned {IPE_DERATED_POINT_MM_TOOTH:.9}. \
         A scaling-law change must re-pin this file, not slide past it.",
    );
    // Ruling R4 Q9 with A2: with no band the floor is `min(0.025, point)`.
    // The point 0.048864 is above 0.025, so the floor stays the constant.
    assert!(
        point > RUBBING_FLOOR_MM_TOOTH,
        "fixture precondition: the Ipe point {point:.6} sits ABOVE the \
         {RUBBING_FLOOR_MM_TOOTH} global floor, so this cell exercises the \
         constant arm of `rubbing_floor_at`.",
    );
    assert_eq!(
        rubbing_floor_at(result.chipload_bounds, result.chipload_point_mm),
        (RUBBING_FLOOR_MM_TOOTH, RubbingFloorSource::RepoConstant),
    );

    let chipload = result.feed_rate_mm_min / (rpm * flutes);
    let computed = result.derates.effective_chip_load_mm();
    assert!(
        (chipload - computed).abs() <= computed * 1e-9,
        "the shipped advance {chipload:.9} is not the computed advance {computed:.9}",
    );
    assert!(
        chipload > RUBBING_FLOOR_MM_TOOTH,
        "after ruling R4 WP3 the moved cell ships above the floor: advance {chipload:.6} \
         vs {RUBBING_FLOOR_MM_TOOTH}",
    );
    assert!(
        (chipload - IPE_MOVED_CELL_ADVANCE_MM_TOOTH).abs() < 1e-5,
        "the moved cell's advance moved: {chipload:.6} vs pinned \
         {IPE_MOVED_CELL_ADVANCE_MM_TOOTH} (0.034567 / 0.75)",
    );

    // No feed factor is left between the two setups: the long-tool share is
    // a load target (ruling R4 Q7), and the workholding factor is gone (Q8).
    let short = calc_ipe_6mm(SetupContext::default());
    let short_chipload = short.feed_rate_mm_min / (short.rpm * flutes);
    assert!(
        (chipload - short_chipload).abs() <= short_chipload * 1e-9,
        "the moved-cell advance {chipload:.9} is not the default-setup advance \
         {short_chipload:.9}",
    );
    assert_eq!(
        result.derates.ld_overhang, LONG_TOOL_FACTOR,
        "L/D 7.5 takes the 0.75 long-tool share, on the record only"
    );
}

#[test]
fn ipe_pocket_does_not_warn_below_the_floor_after_r4() {
    let result = calc_ipe_6mm_long_tool();
    assert!(
        !result
            .warnings
            .iter()
            .any(|w| matches!(w, FeedsWarning::ChiploadBelowRubbingFloor { .. })),
        "the moved Ipe cell ships above the floor after ruling R4 WP3 and Q8, so no floor \
         warning. Warnings: {:?}",
        result.warnings,
    );
    assert!(
        result
            .warnings
            .iter()
            .any(|w| matches!(w, FeedsWarning::LongToolDerate { .. })),
        "the long-tool share is still visible (R4 WP1). Warnings: {:?}",
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
