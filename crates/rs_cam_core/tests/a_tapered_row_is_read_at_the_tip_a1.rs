//! A1 — a tapered-ball row is read at the tip (operator ruling A1,
//! 2026-09-24, `planning/extrapolation_2026-09-24/RULINGS.md`).
//!
//! Every tapered chart (Onsrud, Amana, SpeTool, Whiteside) prints the
//! chipload against the TIP diameter. Before A1 the engine looked the row
//! up at the engaged cone diameter at the cut depth (feeds matrix R2). A1
//! moves the lookup key back to the tip. The depth half of R2 stands: the
//! depth ladder, the depth cap, the band de-rate and the deflection gate
//! still read the engaged cone diameter.
//!
//! The fixture is the wanaka 3D Finish tool: a 1.0 mm tip, 7 deg taper,
//! 6 mm shank, 2 flutes, on a Parallel finish in hardwood. At a 2.0 mm cut
//! depth the cone diameter is about 1.38 mm, not the 1.0 mm tip.
//!
//! This sentry asserts:
//!
//! - the Suggest lookup key (`vendor_normalize::to_lookup_query`) is the tip;
//! - the gate lookup key (`LutBandStage::queried_diameter_mm`, from
//!   `tool_load::evaluate_toolpath`) is the tip;
//! - the two keys agree, the two key helpers agree, and Suggest and the gate
//!   match the same row;
//! - the depth de-rate still divides by the cone diameter, not by the tip.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::compute::cutter::build_cutter;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::feeds::geometry::{
    depth_cap_diameter_mm, doc_derating_scale, feed_ladder_diameter_mm,
    lut_key_diameter_for_cutter, lut_key_diameter_mm,
};
use rs_cam_core::feeds::vendor_lut::{LutOperationFamily, LutPassRole};
use rs_cam_core::feeds::{
    FeedsInput, OperationFamily, PassRole, SetupContext, SpindleStrategy, calculate,
    embedded_vendor_lut, vendor_normalize,
};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::machine::kinematics::PredictedFeedMap;
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::stock::simulation_cut::{
    Engagement, SimulationCutSample, SimulationCutSummary, SimulationCutTrace,
};
use rs_cam_core::tool::MillingCutter;
use rs_cam_core::tool_load::{ToleranceBands, ToolpathLoadContext, evaluate_toolpath};

/// The ball tip (mm).
const TIP_MM: f64 = 1.0;
/// The shank (mm).
const SHANK_MM: f64 = 6.0;
/// A cut depth in the cone, where the engaged diameter is not the tip.
const DEPTH_MM: f64 = 2.0;
const RPM: u32 = 18_000;
const FLUTES: u32 = 2;
/// The printed row that answers: Amana ZrN v8, 1.0 mm tip, 2 flutes.
const ROW_ID: &str = "amana-tapered-hardwood-parallel-1000-2f-zrn-v8";
/// That row's printed maximum (0.002 in/tooth).
const ROW_MAX_MM: f64 = 0.0508;

fn tool_config() -> ToolConfig {
    let mut t = ToolConfig::new_default(ToolId(1), ToolType::TaperedBallNose);
    t.diameter = TIP_MM;
    t.flute_count = FLUTES;
    t.taper_half_angle = 7.0;
    t.cutting_length = 25.0;
    t.shank_diameter = SHANK_MM;
    t.shaft_diameter = SHANK_MM;
    t.stickout = 35.0;
    t
}

fn hardwood() -> Material {
    Material::SolidWood {
        species: WoodSpecies::GenericHardwood,
    }
}

/// One steady-state cutting sample at `DEPTH_MM`.
fn trace(feed_mm_min: f64) -> SimulationCutTrace {
    let id = ToolpathId(0);
    let mut predicted_feeds = PredictedFeedMap::new();
    predicted_feeds.insert((id, 0), feed_mm_min);
    let sample = SimulationCutSample {
        toolpath_id: id,
        move_index: 0,
        sample_index: 0,
        segment_time_s: 0.1,
        is_cutting: true,
        feed_rate_mm_min: feed_mm_min,
        spindle_rpm: RPM,
        flute_count: FLUTES,
        axial_doc_mm: DEPTH_MM,
        axial_engagement_mm: DEPTH_MM,
        arc_engagement_radians: Some(1.0),
        // The gate needs a chip value in the sample to keep it in the
        // population. It does not read the value.
        effective_chip_thickness_mm: Some(0.01),
        engagement: Engagement::with_radial_woc(0.5),
        removed_volume_est_mm3: 0.1,
        mrr_mm3_s: 1.0,
        ..SimulationCutSample::test_fixture()
    };
    SimulationCutTrace {
        predicted_feeds,
        sample_step_mm: 1.0,
        summary: SimulationCutSummary {
            sample_count: 1,
            toolpath_count: 1,
            cutting_runtime_s: 1.0,
            total_runtime_s: 1.0,
            average_engagement: 0.5,
            peak_axial_doc_mm: DEPTH_MM,
            ..SimulationCutSummary::default()
        },
        samples: vec![sample],
        ..SimulationCutTrace::test_fixture()
    }
}

#[test]
fn a_tapered_row_is_read_at_the_tip_a1() {
    let tool = tool_config();
    let cutter = build_cutter(&tool);
    let hint = cutter.geometry_hint();
    let material = hardwood();
    let machine = MachineProfile::shapeoko_vfd();
    let lut = embedded_vendor_lut();

    // Non-vacuity: at this depth the cone diameter is well clear of the tip,
    // and `diameter()` of a tapered ball is its shaft, not its tip.
    let cone_mm = cutter.lookup_diameter_at(DEPTH_MM);
    assert!(
        cone_mm > TIP_MM + 0.2 && cone_mm < SHANK_MM,
        "vacuous: {DEPTH_MM} mm deep must sit in the cone, got {cone_mm} mm"
    );
    assert!((cutter.diameter() - SHANK_MM).abs() < 1e-12);

    // 1. The Suggest key: the query that `feeds::calculate` looks up.
    let input = FeedsInput {
        tool_diameter: tool.diameter,
        flute_count: tool.flute_count,
        flute_length: tool.cutting_length,
        shank_diameter: Some(tool.shank_diameter),
        tool_geometry: hint,
        material: &material,
        machine: &machine,
        operation: OperationFamily::Parallel,
        operation_kind: Some(OperationType::DropCutter),
        pass_role: PassRole::Finish,
        axial_depth_mm: Some(DEPTH_MM),
        radial_width_mm: None,
        target_scallop_mm: None,
        vendor_lut: Some(lut),
        setup: SetupContext::default(),
        spindle_strategy: SpindleStrategy::default(),
    };
    let suggest_key = vendor_normalize::to_lookup_query(&input)
        .expect("a tapered ball on a DropCutter routes to Parallel / Finish")
        .diameter_mm;
    assert!(
        (suggest_key - TIP_MM).abs() < 1e-12,
        "the Suggest key must be the {TIP_MM} mm tip, not the {cone_mm} mm cone: {suggest_key}"
    );

    // 2. The two key helpers agree on this tool, and both give the tip.
    let hint_key = lut_key_diameter_mm(hint, DEPTH_MM, tool.diameter, tool.shank_diameter);
    let cutter_key = lut_key_diameter_for_cutter(&cutter, DEPTH_MM);
    assert!((hint_key - TIP_MM).abs() < 1e-12, "{hint_key}");
    assert!((cutter_key - TIP_MM).abs() < 1e-12, "{cutter_key}");

    // 3. The gate key: the stage record of the shipped gate.
    let feed = 0.03 * f64::from(RPM) * f64::from(FLUTES);
    let t = trace(feed);
    let verdict = evaluate_toolpath(
        &ToolpathLoadContext {
            toolpath_id: ToolpathId(0),
            tool: &cutter,
            material: &material,
            operation_family: LutOperationFamily::Parallel,
            pass_role: LutPassRole::Finish,
            operation_feed_rate_mm_min: feed,
            operation_kind: OperationType::DropCutter,
            spans: None,
            drill_op: None,
        },
        Some(&t),
        None,
        &ToleranceBands::default(),
    );
    let band = &verdict
        .feed_explanation
        .as_ref()
        .expect("the chipload gate reached a modelled verdict and filed its stages")
        .band;
    assert!(
        (band.queried_diameter_mm - TIP_MM).abs() < 1e-12,
        "the gate key must be the {TIP_MM} mm tip, not the {cone_mm} mm cone: {}",
        band.queried_diameter_mm
    );
    assert!(
        (band.queried_diameter_mm - suggest_key).abs() < 1e-12,
        "Suggest and the gate must query one key: {suggest_key} vs {}",
        band.queried_diameter_mm
    );

    // 4. One key gives one row: Suggest's recipe row is the gate's row.
    let recipe = calculate(&input);
    let recipe_row = recipe
        .matched_lut_row
        .as_ref()
        .expect("Suggest matched a vendor row");
    assert_eq!(recipe_row.observation_id, ROW_ID);
    assert_eq!(band.observation_id, ROW_ID);
    assert!((band.row_diameter_mm - TIP_MM).abs() < 1e-12);
    assert!(
        (band.diameter_scale - 1.0).abs() < 1e-12,
        "the key is the row's own tip, so no diameter scale applies: {}",
        band.diameter_scale
    );

    // 5. The depth half of R2 stands: the de-rate divides by the cone.
    let ladder = feed_ladder_diameter_mm(hint, DEPTH_MM, tool.diameter, tool.shank_diameter);
    let cap = depth_cap_diameter_mm(&cutter, DEPTH_MM);
    assert!((ladder - cone_mm).abs() < 1e-9, "{ladder} vs {cone_mm}");
    assert!((cap - cone_mm).abs() < 1e-9, "{cap} vs {cone_mm}");
    let on_cone = ROW_MAX_MM * doc_derating_scale(DEPTH_MM / cone_mm);
    let on_tip = ROW_MAX_MM * doc_derating_scale(DEPTH_MM / TIP_MM);
    assert!(
        (on_cone - on_tip).abs() > 1e-3,
        "vacuous: the two de-rates must differ at this depth ({on_cone} vs {on_tip})"
    );
    assert!(
        (band.max_mm_per_tooth - on_cone).abs() < 1e-12,
        "the gate's band maximum must be de-rated at the cone diameter: expected \
         {on_cone} (on the tip it would be {on_tip}), got {}",
        band.max_mm_per_tooth
    );
}
