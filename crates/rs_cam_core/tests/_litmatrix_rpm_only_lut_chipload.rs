//! Literature-matrix regression — RPM-only vendor LUT rows must fall
//! back to formula chipload, never produce a zero chipload / zero feed.
//!
//! Cell: `flat_12mm_adaptive2d_oak_power`. Pre-fix, the LUT branch in
//! `feeds::calculate` trusted `LookupResult::chip_load_mm` unconditionally.
//! RPM-only rows (e.g. `whiteside-rd5218h-roughing-down-spiral-3f-rpm`)
//! publish `rpm_nominal` / `rpm_max` as anchors but leave both
//! `chipload_min_mm_tooth` and `chipload_max_mm_tooth` unset, so
//! `chipload_midpoint` returned 0.0. With chipload = 0,
//! `raw_feed = rpm × chipload × flutes × thinning × depth_tier` collapsed
//! to 0, MRR went to 0, predicted power went to 0, and the operator
//! silently received a "do not cut" recipe — no `PowerLimited` warning,
//! no diagnostic.
//!
//! The fix keeps `result.rpm_nominal` / `result.rpm_max` as vendor RPM
//! anchors (they're the entire reason the row exists) but falls back to
//! `formula_chipload` when the row publishes no chipload band.
//!
//! This sentry locks in: a 12 mm 4F flat tool in red oak under the
//! embedded vendor LUT must produce a non-zero chipload that clears the
//! 0.025 mm/tooth rubbing floor, regardless of which row wins the
//! score-based lookup.

use rs_cam_core::compute::ToolConfig;
use rs_cam_core::compute::ToolId;
use rs_cam_core::compute::ToolType;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::feeds::{FeedsInput, SpindleStrategy, calculate, embedded_vendor_lut};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, WoodSpecies};

const RUBBING_FLOOR_MM_TOOTH: f64 = 0.025;

fn calc_oak_12mm(strategy: SpindleStrategy) -> rs_cam_core::feeds::FeedsResult {
    let lut = embedded_vendor_lut();
    // P21c: `ConstantChip` needs a machine that states its axis rates and
    // accelerations (a preset refuses at the Suggest door, and a raw
    // `calculate` keeps the chart values). The values are the ones that
    // `planning/rivmap_block_job/nz_wanaka_21_operator.toml` prints.
    let machine = match strategy {
        SpindleStrategy::MatchChart => MachineProfile::generic_wood_router(),
        SpindleStrategy::ConstantChip { .. } => MachineProfile {
            max_feed_mm_min: 15_000.0,
            ..MachineProfile::generic_wood_router()
        }
        .with_axis_limits([15_000.0, 15_000.0, 2500.0], [1200.0, 1200.0, 550.0]),
    };
    let material = Material::SolidWood {
        species: WoodSpecies::WhiteOak,
    };

    calculate(&FeedsInput::of_operation(
        &OperationConfig::new_default(OperationType::Adaptive),
        &ToolConfig::builder(ToolId::from_raw(1), ToolType::EndMill)
            .diameter(12.0)
            .flutes(4)
            .flute_mm(38.0)
            .shank_mm(12.0)
            .stickout_mm(4.0 * 12.0)
            .build(),
        &material,
        &machine,
        lut,
        strategy,
    ))
}

#[test]
fn rpm_only_lut_row_does_not_collapse_chipload_to_zero() {
    let result = calc_oak_12mm(SpindleStrategy::ConstantChip {
        rpm_ceiling: Some(24_000),
    });
    assert!(
        result.rpm_choice.is_some(),
        "the constant-chip rule ran: a machine with stated rates and a ceiling"
    );

    let rpm = result.rpm;
    // The cell uses a 4-flute tool; flute_count flows through unchanged.
    let flutes = 4.0_f64;
    assert!(rpm > 0.0, "engine produced rpm = {rpm}");

    let chipload = result.feed_rate_mm_min / (rpm * flutes);
    assert!(
        chipload > 0.0,
        "RPM-only LUT row collapsed chipload to {chipload} (feed_rate={}, rpm={rpm}, flutes={flutes})",
        result.feed_rate_mm_min,
    );
    assert!(
        chipload >= RUBBING_FLOOR_MM_TOOTH,
        "RPM-only LUT row dropped chipload {chipload:.4} below rubbing floor {RUBBING_FLOOR_MM_TOOTH} (feed_rate={}, rpm={rpm})",
        result.feed_rate_mm_min,
    );
}

#[test]
fn rpm_only_lut_row_match_chart_also_produces_chipload() {
    // MatchChart goes through the same LUT branch but skips the
    // constant-chip rise: the fix works on both paths.
    let result = calc_oak_12mm(SpindleStrategy::MatchChart);

    let rpm = result.rpm;
    // The cell uses a 4-flute tool; flute_count flows through unchanged.
    let flutes = 4.0_f64;
    assert!(rpm > 0.0, "engine produced rpm = {rpm}");

    let chipload = result.feed_rate_mm_min / (rpm * flutes);
    assert!(
        chipload > 0.0,
        "RPM-only LUT row collapsed chipload under MatchChart: {chipload} (feed_rate={}, rpm={rpm}, flutes={flutes})",
        result.feed_rate_mm_min,
    );
}
