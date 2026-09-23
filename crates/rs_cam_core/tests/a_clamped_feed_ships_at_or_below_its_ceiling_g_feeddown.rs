//! Sentry: **the feed quantisation never raises a feed a clamp already
//! bound** (G-FEEDDOWN, T-9, 2026-09-18).
//!
//! ## The defect this pins
//!
//! `feeds::calculate` applies every ceiling — the Step 6 spindle power gate
//! and the Step 7 machine cutting-feed ceiling — and returns a feed that
//! satisfies all of them. `feeds::suggest::apply` then quantises that feed
//! to a whole mm/min. It used `round_suggestion_value`, which snaps to the
//! NEAREST multiple and therefore goes UP about half the time. Nothing
//! re-checks a limit after the quantisation, so the shipped recipe sat up
//! to +0.5 mm/min above the ceiling the clamp exists to enforce.
//!
//! Measured before the fix (`planning/TECH_DEBT_REGISTER.md`, T-9):
//! Shapeoko 1.5 kW VFD / Ipe / Ø12 slot, calculator 323.6993 mm/min ->
//! shipped 324.0000, which is 100.013 % of the gate ceiling.
//!
//! `MachineProfile::next_rpm_at_or_below` is the same fix on the RPM axis,
//! and `a_downward_traverse_rounds_down_g_rpmdown.rs` is this file's model.
//!
//! ## What is asserted
//!
//! 1. `round_suggestion_value_down` floors. A value with a fraction of .5
//!    or more goes DOWN, and a whole value is unchanged — the second half
//!    is the anchor that stops the arm passing on a helper that always
//!    subtracts.
//! 2. Through the public apply door, over every shipped preset x every
//!    shipped species: the shipped feed never exceeds the calculator feed.
//!    The sweep is restricted to the pairs whose calculator feed has a
//!    fraction of .5 or more, because those are the pairs the NEAREST
//!    rounding sent up.
//! 3. `round_suggestion_value` still rounds to the nearest, so the two
//!    helpers are distinct. Without this the arm above could pass on a
//!    build where the down helper is a plain alias.
//!
//! Arm 2 carries its own non-vacuity anchor: it asserts the sweep found at
//! least one pair with a fraction of .5 or more, and at least one pair the
//! calculator actually clamped.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::PocketConfig;
use rs_cam_core::compute::{ToolConfig, ToolId, ToolType};
use rs_cam_core::feeds::suggest::{
    ApplyContext, ApplyScope, FeedsPreview, SuggestContext, round_suggestion_value,
    round_suggestion_value_down,
};
use rs_cam_core::feeds::{
    FeedsInput, FeedsWarning, OperationFamily, PassRole, SetupContext, SpindleStrategy,
    ToolGeometryHint, embedded_vendor_lut,
};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, WoodSpecies};

/// Every shipped wood species — the same sweep population
/// `suggest_power_ceiling_after_pass9_g_suggest_powerstale.rs` uses.
const ALL_SPECIES: [WoodSpecies; 10] = [
    WoodSpecies::GenericSoftwood,
    WoodSpecies::RadiataPine,
    WoodSpecies::LongleafPine,
    WoodSpecies::GenericHardwood,
    WoodSpecies::HardMaple,
    WoodSpecies::Walnut,
    WoodSpecies::Birch,
    WoodSpecies::WhiteOak,
    WoodSpecies::Jarrah,
    WoodSpecies::Ipe,
];

/// Ø12, not Ø6. The sibling power instrument records the reason: at Ø6 a
/// full-width slot trips `SlottingDetected`, the cross-section collapses
/// before Step 6, and the sweep never reaches a ceiling at all.
const DIAMETER_MM: f64 = 12.0;

/// The quantisation step the apply path uses for the feed and the plunge.
const FEED_STEP_MM_MIN: f64 = 1.0;

fn endmill() -> ToolConfig {
    let mut tool = ToolConfig::new_default(ToolId(1), ToolType::EndMill);
    tool.diameter = DIAMETER_MM;
    tool.flute_count = 2;
    tool
}

/// A heavy 2.5D rough: a full-width slot at an aggressive DOC, so the
/// ceilings have something to bind.
fn heavy_pocket() -> OperationConfig {
    OperationConfig::Pocket(PocketConfig {
        stepover: DIAMETER_MM,
        depth: 4.0 * DIAMETER_MM,
        depth_per_pass: 2.0 * DIAMETER_MM,
        feed_rate: 1000.0,
        plunge_rate: 500.0,
        climb: true,
        ..PocketConfig::default()
    })
}

fn feeds_input<'a>(
    machine: &'a MachineProfile,
    material: &'a Material,
    ap_mm: f64,
    ae_mm: f64,
) -> FeedsInput<'a> {
    FeedsInput {
        tool_diameter: DIAMETER_MM,
        flute_count: 2,
        flute_length: 3.0 * DIAMETER_MM,
        shank_diameter: None,
        tool_geometry: ToolGeometryHint::Flat,
        material,
        machine,
        operation: OperationFamily::Pocket,
        operation_kind: None,
        pass_role: PassRole::Roughing,
        axial_depth_mm: Some(ap_mm),
        radial_width_mm: Some(ae_mm),
        target_scallop_mm: None,
        vendor_lut: Some(embedded_vendor_lut()),
        setup: SetupContext::default(),
        spindle_strategy: SpindleStrategy::MatchChart,
    }
}

/// What one swept pair produced.
struct Shipped {
    /// The calculator's feed, before the quantisation. Every ceiling is
    /// satisfied at this value.
    calculator_feed: f64,
    /// The feed the apply funnel wrote onto the operation.
    shipped_feed: f64,
    /// True when the calculator reports that a ceiling bound this point.
    clamped: bool,
}

/// Run the WHOLE public funnel — `FeedsPreview::build` ->
/// `suggest::apply` — and report what lands on the operation. The
/// calculator's recommendation and the shipped operation are different
/// numbers, and only one of them reaches the machine.
fn run_funnel(machine: &MachineProfile, material: &Material) -> Option<Shipped> {
    let requested_ap = 2.0 * DIAMETER_MM;
    let tool = endmill();
    let mut operation = heavy_pocket();
    let mut provenance = rs_cam_core::feeds::FeedsProvenance::default();

    let input = feeds_input(machine, material, requested_ap, DIAMETER_MM);
    let preview = FeedsPreview::build(&input);
    let rec = preview.applicable()?;
    let result = rec.result().clone();

    let _warnings = rs_cam_core::feeds::suggest::apply(
        &rec,
        ApplyScope::Both,
        &mut operation,
        &mut provenance,
        ApplyContext {
            tool: &tool,
            machine,
            material,
            pass_role: PassRole::Roughing,
            suggest: SuggestContext::default(),
        },
    );

    let ceiling = machine.cutting_feed_ceiling_mm_min();
    let clamped = result.warnings.iter().any(|w| {
        matches!(
            w,
            FeedsWarning::FeedRateClamped { .. }
                | FeedsWarning::PowerLimited { .. }
                // Ruling R4 Q10: a ceiling that lowered the RPM bound the
                // feed too.
                | FeedsWarning::RpmLoweredForFeedCeiling { .. }
        )
    }) || (result.feed_rate_mm_min - ceiling).abs() < 1e-9;

    Some(Shipped {
        calculator_feed: result.feed_rate_mm_min,
        shipped_feed: operation.feed_rate(),
        clamped,
    })
}

#[test]
fn the_down_helper_floors_and_leaves_a_whole_value_alone_g_feeddown() {
    // A fraction of .5 or more is exactly where `f64::round` goes UP.
    assert!(
        (round_suggestion_value_down(1234.6, FEED_STEP_MM_MIN) - 1234.0).abs() < 1e-9,
        "1234.6 floored to {} at step 1, not 1234.0",
        round_suggestion_value_down(1234.6, FEED_STEP_MM_MIN)
    );
    assert!(
        (round_suggestion_value_down(1234.5, FEED_STEP_MM_MIN) - 1234.0).abs() < 1e-9,
        "1234.5 floored to {} at step 1, not 1234.0",
        round_suggestion_value_down(1234.5, FEED_STEP_MM_MIN)
    );
    // The register's own measured case.
    assert!(
        (round_suggestion_value_down(323.6993, FEED_STEP_MM_MIN) - 323.0).abs() < 1e-9,
        "the measured T-9 case 323.6993 floored to {}, not 323.0",
        round_suggestion_value_down(323.6993, FEED_STEP_MM_MIN)
    );

    // The non-vacuity anchor: a value already on a step is NOT moved. A
    // helper that simply subtracts one step would pass every assertion
    // above and fail this one.
    assert!(
        (round_suggestion_value_down(1234.0, FEED_STEP_MM_MIN) - 1234.0).abs() < 1e-9,
        "1234.0 is already a whole mm/min and moved to {}",
        round_suggestion_value_down(1234.0, FEED_STEP_MM_MIN)
    );
    assert!(
        (round_suggestion_value_down(0.0, FEED_STEP_MM_MIN)).abs() < 1e-9,
        "zero moved"
    );

    // A non-positive step is a no-op, the same contract the nearest helper
    // carries.
    assert!(
        (round_suggestion_value_down(1234.6, 0.0) - 1234.6).abs() < 1e-9,
        "a zero step must return the value unchanged"
    );
}

#[test]
fn the_quantisation_never_raises_a_clamped_feed_g_feeddown() {
    let mut checked = 0usize;
    let mut half_or_more = 0usize;
    let mut clamped_pairs = 0usize;
    let mut worst: Option<(String, f64, f64)> = None;

    for (label, machine) in MachineProfile::presets() {
        for species in ALL_SPECIES {
            let material = Material::SolidWood { species };
            let Some(shipped) = run_funnel(&machine, &material) else {
                continue;
            };
            checked += 1;
            if shipped.clamped {
                clamped_pairs += 1;
            }
            let fraction = shipped.calculator_feed - shipped.calculator_feed.floor();
            if fraction < 0.5 {
                continue;
            }
            half_or_more += 1;
            let overshoot = shipped.shipped_feed - shipped.calculator_feed;
            if worst.as_ref().is_none_or(|(_, _, w)| overshoot > *w) {
                worst = Some((
                    format!("{label} / {species:?}"),
                    shipped.calculator_feed,
                    overshoot,
                ));
            }
        }
    }

    // Non-vacuity, both halves. Without these the assertion below passes on
    // a sweep that never reached a ceiling and never met a fraction the
    // nearest rounding would have raised.
    assert!(checked >= 20, "only {checked} (preset, species) pairs ran");
    assert!(
        half_or_more > 0,
        "no swept pair produced a calculator feed with a fraction of .5 or more, \
         so this arm exercises nothing. Widen the sweep; do not delete it."
    );
    assert!(
        clamped_pairs > 0,
        "no swept pair was clamped by a ceiling, so a feed at or below the \
         calculator's value proves nothing about a ceiling"
    );

    if let Some((where_, calculator, overshoot)) = worst {
        assert!(
            overshoot <= 1e-9,
            "{where_}: the calculator returned {calculator} mm/min, every ceiling \
             satisfied, and the funnel shipped {} mm/min — {overshoot} mm/min ABOVE \
             it. The quantisation must never raise a feed a clamp already bound. \
             T-9.",
            calculator + overshoot
        );
    }
}

#[test]
fn the_nearest_helper_still_rounds_up_so_the_two_are_distinct_g_feeddown() {
    // The non-vacuity partner of the arm above, and the direct copy of
    // `clamp_rpm_does_round_up_so_the_helper_is_not_redundant_g_rpmdown`.
    // If the two helpers agreed everywhere, the fix would be a no-op and
    // the sweep would prove nothing.
    let probes = [1234.6_f64, 1234.5, 323.6993, 0.75];
    let mut saw_disagreement = false;
    for value in probes {
        let nearest = round_suggestion_value(value, FEED_STEP_MM_MIN);
        let down = round_suggestion_value_down(value, FEED_STEP_MM_MIN);
        assert!(
            down <= value + 1e-9,
            "the down helper raised {value} to {down}"
        );
        if nearest > value + 1e-9 {
            saw_disagreement = true;
            assert!(
                down < nearest,
                "the nearest helper raised {value} to {nearest} and the down helper \
                 returned the same number"
            );
        }
    }
    assert!(
        saw_disagreement,
        "round_suggestion_value never rounded up across the probe set, so the \
         arms above prove nothing"
    );
}
