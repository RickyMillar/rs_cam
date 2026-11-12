//! Feeds contract P23a (`planning/feeds_contract_2026-11-03/phases/P23a.md`;
//! operator ruling Q2, `P22_DESIGN.md` section 9): the chip-thinning lift of
//! Suggest reads the widest width that the op's planner holds at the full
//! feed. For a plan that holds the pass ceiling that width is `c D`, not the
//! nominal stepover; the pre-sim power case "planner cap" reads the same
//! width.
//!
//! The pass ceiling as a radial fraction is
//! `c = (1 - cos((1 + t) x acos(1 - s/R))) / 2` with the planner tolerance
//! `t` = 0.05 (`adaptive/search.rs`, `PASS_ENGAGEMENT_TOLERANCE`; 0.3626 at
//! s 2, R 3 in `adaptive/CLAUDE.md`). The lift is Garr's
//! `1 / sqrt(1 - (1 - 2 ae/D)^2)` below half the diameter.
//!
//! The arms:
//!
//! - (a) 3D Rough under Geodesic Adaptive (the wanaka idx 1 cut: 18 mm
//!   2-flute tool, white oak, stepover 5.5, depth 10): the lift is the lift
//!   at `c D` = 5.9913 mm, and the feed is the unlifted feed times that lift;
//! - (b) the same op under Contour Parallel: the lift is the lift at the
//!   nominal 5.5 mm (that plan holds no bound). P29-5b (lead ruling Q1):
//!   the perimeter sweep of that plan is a full slot on every level, so the
//!   power case is the slot. At the 10 mm depth the slot needs more than
//!   the spindle power, so the cut ships `Slot` with `PowerLimited` and the
//!   power ladder lowers the feed. The lift claim runs at a depth where the
//!   slot power is under the rated power, so no ladder step moves the feed;
//! - (c) 2D Adaptive (6 mm tool, stepover 2): the lift at `c D`, c = 0.3626;
//! - (d) a Pocket: the lift at its nominal stepover;
//! - (e) the unlifted feed (the plunge and ramp door) is the feed over the
//!   same lift on every arm.
//!
//! Red: read `cut.ae` in `feeds/calculator/feed.rs::step5_feed`.

#![expect(
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::fixture_values::fixture_op;
use rs_cam_core::compute::operation_configs::ClearingStrategy;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolFact, ToolId, ToolType};
use rs_cam_core::feeds::engagement::PowerCase;
use rs_cam_core::feeds::suggest::{
    NoProject, SuggestContext, SuggestForOperationInput, suggest_for_operation,
};
use rs_cam_core::feeds::{FeedsResult, FeedsWarning, SpindleStrategy, embedded_vendor_lut};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, WoodSpecies};

/// The planner tolerance of the pass ceiling (`PASS_ENGAGEMENT_TOLERANCE`).
const PASS_TOLERANCE: f64 = 0.05;

/// The pass ceiling width `c D` (mm), by the closed form of the file header.
fn pass_ceiling_mm(stepover: f64, diameter: f64) -> f64 {
    let radius = diameter / 2.0;
    let arc = (1.0 - stepover.min(diameter) / radius)
        .clamp(-1.0, 1.0)
        .acos();
    let c = (1.0
        - ((1.0 + PASS_TOLERANCE) * arc)
            .min(std::f64::consts::PI)
            .cos())
        / 2.0;
    c * diameter
}

/// Garr's lift at the width `ae` on the diameter `d`.
fn garr_lift(ae: f64, d: f64) -> f64 {
    if ae >= d / 2.0 {
        1.0
    } else {
        1.0 / (1.0 - (1.0 - 2.0 * ae / d).powi(2)).sqrt()
    }
}

/// The machine values of `planning/rivmap_block_job/nz_wanaka_21_operator.toml`
/// on the Shapeoko VFD preset.
fn wanaka_like() -> MachineProfile {
    MachineProfile {
        max_feed_mm_min: 15_000.0,
        ..MachineProfile::shapeoko_vfd()
    }
    .with_axis_limits([15_000.0, 15_000.0, 2500.0], [1200.0, 1200.0, 550.0])
}

fn flat_2f(diameter: f64, flute: f64) -> ToolConfig {
    let mut t = ToolConfig::test_default(ToolId::from_raw(1), ToolType::EndMill);
    t.diameter = diameter;
    t.set_flute_count(ToolFact::Stated(2));
    t.set_cutting_length(ToolFact::Stated(flute));
    t.set_shank_diameter(ToolFact::Stated(6.0));
    t.shaft_diameter = 6.0;
    t.set_stickout(ToolFact::Stated(25.0));
    t
}

fn run(op: &OperationConfig, tool: &ToolConfig) -> FeedsResult {
    suggest_for_operation(SuggestForOperationInput {
        operation: op,
        tool,
        machine: &wanaka_like(),
        material: &Material::SolidWood {
            species: WoodSpecies::WhiteOak,
        },
        lut: embedded_vendor_lut(),
        spindle_strategy: SpindleStrategy::ConstantChip {
            rpm_ceiling: Some(24_000),
        },
        context: SuggestContext::no_project(NoProject::NONE),
    })
    .unwrap_or_else(|e| panic!("Suggest serves {:?}: {e}", op.op_type()))
    .feeds_result
}

/// The depth of the wanaka idx 1 cut (mm).
const WANAKA_DEPTH_MM: f64 = 10.0;

fn rough_3d(strategy: ClearingStrategy) -> OperationConfig {
    rough_3d_at(strategy, WANAKA_DEPTH_MM)
}

fn rough_3d_at(strategy: ClearingStrategy, depth: f64) -> OperationConfig {
    let mut op = fixture_op(OperationType::Adaptive3d);
    assert!(op.set_stepover(5.5) && op.set_depth_per_pass(depth));
    let OperationConfig::Adaptive3d(config) = &mut op else {
        panic!("the fixture is a 3D Rough");
    };
    config.clearing_strategy = strategy;
    op
}

/// The lift of `r` is Garr's lift at `width`, and the feed and the unlifted
/// feed agree with it.
fn assert_lift_at(r: &FeedsResult, width: f64, diameter: f64, label: &str) {
    let want = garr_lift(width, diameter);
    let lift = r.derates.radial_chip_thinning;
    assert!(
        (lift - want).abs() < 1e-9,
        "{label}: the lift {lift} is not the lift {want} at the width {width}"
    );
    // The feed is chip x Z x RPM x lift (no later step moved it here).
    let unlifted = r.chip_load_mm * 2.0 * r.rpm;
    assert!(
        (r.feed_rate_mm_min - unlifted * want).abs() < 1e-6,
        "{label}: feed {} vs {unlifted} x {want}",
        r.feed_rate_mm_min
    );
    // (e) The plunge and ramp door divides by the same lift.
    assert!(
        (r.unlifted_feed().mm_min() - unlifted).abs() < 1e-6,
        "{label}: the unlifted feed {} vs {unlifted}",
        r.unlifted_feed().mm_min()
    );
}

/// (a) and (b): the geodesic arm reads `c D`; Contour Parallel reads the
/// nominal stepover.
#[test]
fn a_geodesic_rough_lifts_at_the_pass_ceiling_and_contour_parallel_at_the_stepover_p23a() {
    let tool = flat_2f(18.0, 20.0);
    let ceiling = pass_ceiling_mm(5.5, 18.0);
    // `P22_DESIGN.md` section 9, Q2: 5.99 mm on wanaka idx 1.
    assert!((ceiling - 5.9913).abs() < 1e-4, "c D = {ceiling}");

    let geodesic = run(&rough_3d(ClearingStrategy::GeodesicAdaptive), &tool);
    assert_lift_at(&geodesic, ceiling, 18.0, "geodesic");
    assert_eq!(geodesic.radial_width_mm, 5.5, "the nominal width holds");
    assert_eq!(geodesic.power_case, PowerCase::PlannerCap);
    // The wanaka idx 1 Suggest feed: 0.17913 x 2 x 24000 x 1.06104.
    assert_eq!(geodesic.feed_rate_mm_min.floor(), 9123.0);

    // P29-5b: the deep contour-parallel cut ships the slot of its sweep.
    // The slot needs more than the spindle power, so the cut warns and the
    // power ladder lowers the feed.
    let deep = run(&rough_3d(ClearingStrategy::ContourParallel), &tool);
    assert_eq!(deep.power_case, PowerCase::Slot, "{deep:?}");
    let (required, available) = deep
        .warnings
        .iter()
        .find_map(|w| {
            if let FeedsWarning::PowerLimited {
                required_kw,
                available_kw,
            } = w
            {
                Some((*required_kw, *available_kw))
            } else {
                None
            }
        })
        .unwrap_or_else(|| panic!("the deep slot is power limited: {:?}", deep.warnings));
    assert!(required > available, "{required} kW vs {available} kW");
    assert!(
        deep.feed_rate_mm_min < geodesic.feed_rate_mm_min,
        "the power ladder lowers the feed of the deep slot"
    );

    // The lift claim of (b) at a depth where the slot power is under the
    // rated power. At one feed the slot power is linear in the depth (the
    // section and the edge term both scale with it), so the depth
    // `WANAKA_DEPTH_MM x available / required` meets the rated power. Half
    // of it leaves a margin, and the assert below holds that no ladder step
    // moved the feed.
    let shallow_depth = WANAKA_DEPTH_MM * available / required / 2.0;
    let contour = run(
        &rough_3d_at(ClearingStrategy::ContourParallel, shallow_depth),
        &tool,
    );
    assert!(
        !contour
            .warnings
            .iter()
            .any(|w| matches!(w, FeedsWarning::PowerLimited { .. })),
        "the shallow slot is under the rated power: {:?}",
        contour.warnings
    );
    assert_lift_at(&contour, 5.5, 18.0, "contour parallel");
    assert_eq!(contour.power_case, PowerCase::Slot);
    assert!(
        geodesic.derates.radial_chip_thinning < contour.derates.radial_chip_thinning,
        "the lift at the wider width is the smaller lift"
    );
}

/// (c) and (d): 2D Adaptive reads `c D`; a Pocket reads its stepover.
#[test]
fn a_2d_adaptive_lifts_at_the_pass_ceiling_and_a_pocket_at_its_stepover_p23a() {
    let tool = flat_2f(6.0, 24.0);
    let mut adaptive = fixture_op(OperationType::Adaptive);
    assert!(adaptive.set_stepover(2.0));
    let ceiling = pass_ceiling_mm(2.0, 6.0);
    // `adaptive/CLAUDE.md`: the ceiling is 0.3626 at s 2, R 3.
    assert!(
        (ceiling / 6.0 - 0.3626).abs() < 5e-5,
        "c = {}",
        ceiling / 6.0
    );
    let r = run(&adaptive, &tool);
    assert_lift_at(&r, ceiling, 6.0, "2D adaptive");
    assert_eq!(r.radial_width_mm, 2.0, "the nominal width holds");

    let mut pocket = fixture_op(OperationType::Pocket);
    assert!(pocket.set_stepover(2.0));
    let r = run(&pocket, &tool);
    assert_lift_at(&r, 2.0, 6.0, "pocket");
    assert!(
        garr_lift(2.0, 6.0) > garr_lift(ceiling, 6.0),
        "non-vacuity: the two widths give two lifts"
    );
}
