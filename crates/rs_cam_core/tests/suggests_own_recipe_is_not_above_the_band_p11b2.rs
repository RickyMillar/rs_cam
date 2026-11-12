//! Feeds contract P11b2 (`planning/feeds_contract_2026-11-03/STATUS.md` item
//! 3a; a consequence of rulings L1 and L6): the post-sim chipload gate judges
//! the PEAK chip, the chip that Suggest sizes the feed for.
//!
//! P11b made Suggest lift the feed by radial chip thinning (Garr, the one law
//! `feeds::geometry::peak_chip_thinning_lift`): at a light stepover the
//! advance per tooth is the band chip over `sin(psi)`, so the PEAK chip is
//! the band chip. Before P11b2 the gate and the `commanded_above_band`
//! diagnostic compared the advance per tooth with the band maximum, so they
//! flagged Suggest's own recipe.
//!
//! The fixture is the P11b lift fixture (D 6, Z 2, white oak roughing pocket,
//! stepover 0.1 D, depth 1 mm, a machine with no power or feed limit; test
//! values, not a machine claim). The trace is hand-built: every sample cuts
//! at the stated width (the arc `psi` with `cos psi = 1 - ae / r`) at the
//! Suggest feed and RPM, so no generator or simulator takes part.
//!
//! Arms:
//!
//! 1. The fixture premise: the Suggest advance per tooth is above the band
//!    maximum (the lift is larger than 1).
//! 2. The post-sim diagnostics hold no `load.chipload.commanded_above_band`
//!    (red before P11b2).
//! 3. The gate observation is the peak chip, `fz / lift`, which is the
//!    Suggest band chip, and the gate is not `Exceeds` (red before P11b2).

#![expect(
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::operation_configs::PocketConfig;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolFact, ToolId, ToolMaterial, ToolType};
use rs_cam_core::diagnostics::adapters::from_tool_load::diagnostics_from_load_verdict;
use rs_cam_core::feeds::FeedsInput;
use rs_cam_core::feeds::{FeedsResult, SpindleStrategy, calculate, embedded_vendor_lut};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::machine::kinematics::PredictedFeedMap;
use rs_cam_core::machine::{MachineProfile, PowerModel};
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::stock::simulation_cut::{
    CutKinematics, Engagement, SimulationCutSample, SimulationCutTrace,
};
use rs_cam_core::tool::{FlatEndmill, ToolDefinition};
use rs_cam_core::tool_load::verdict::ChiploadVerdict;
use rs_cam_core::tool_load::{ToleranceBands, ToolpathLoadContext, evaluate_toolpath};

const D_MM: f64 = 6.0;
const AP_MM: f64 = 1.0;
const AE_MM: f64 = 0.1 * D_MM;
const FLUTES: u32 = 2;
const CUTTING_LENGTH_MM: f64 = 25.0;
const SAMPLES: usize = 24;
const TOOLPATH: ToolpathId = ToolpathId::from_raw(0);

fn end_mill_config() -> ToolConfig {
    let mut tool = ToolConfig::test_default(ToolId::from_raw(3), ToolType::EndMill);
    tool.diameter = D_MM;
    tool.helix_deg = 30.0;
    tool.set_cutting_length(ToolFact::Stated(CUTTING_LENGTH_MM));
    tool.shaft_diameter = 6.35;
    tool.set_shank_diameter(ToolFact::Stated(6.35));
    tool.set_stickout(ToolFact::Stated(45.0));
    tool.set_flute_count(ToolFact::Stated(FLUTES));
    tool
}

fn end_mill() -> ToolDefinition {
    ToolDefinition::new(
        Box::new(FlatEndmill::new(D_MM, CUTTING_LENGTH_MM)),
        6.35,
        45.0,
        FLUTES,
        ToolMaterial::Carbide,
    )
}

fn material() -> Material {
    Material::SolidWood {
        species: WoodSpecies::WhiteOak,
    }
}

fn pocket() -> OperationConfig {
    OperationConfig::Pocket(PocketConfig {
        floor: rs_cam_core::compute::floor::Floor::top(12.0),
        depth_per_pass: Some(AP_MM),
        stepover: Some(AE_MM),
        feed_rate: Some(1800.0),
        plunge_rate: Some(500.0),
        spindle_rpm: Some(18_000),
        ..PocketConfig::default()
    })
}

/// No spindle-power and no feed-ceiling limit binds (test values).
fn open_machine() -> MachineProfile {
    MachineProfile {
        power: PowerModel::ConstantPower { power_kw: 50.0 },
        max_feed_mm_min: 1.0e7,
        max_cutting_feed_mm_min: Some(1.0e7),
        ..MachineProfile::default()
    }
}

fn suggest() -> FeedsResult {
    let tool = end_mill_config();
    let material = material();
    let machine = open_machine();
    let op = pocket();
    let input = FeedsInput::of_operation(
        &op,
        &tool,
        &material,
        &machine,
        embedded_vendor_lut(),
        SpindleStrategy::MatchChart,
    );
    calculate(&input)
}

/// The engagement arc of the stated width: `cos psi = 1 - ae / r`.
fn stated_arc_rad() -> f64 {
    (1.0 - AE_MM / (D_MM / 2.0)).acos()
}

fn trace(feed_mm_min: f64, rpm: u32) -> SimulationCutTrace {
    let tool = end_mill();
    let arc = stated_arc_rad();
    let fz = feed_mm_min / (f64::from(rpm) * f64::from(FLUTES));
    let chip =
        rs_cam_core::dexel_stock::effective_chip_thickness_mm(&tool, AP_MM, Some(arc), fz, FLUTES)
            .expect("the chip model resolves at this depth and arc");
    let samples = (0..SAMPLES)
        .map(|i| SimulationCutSample {
            toolpath_id: TOOLPATH,
            move_index: i + 1,
            sample_index: i,
            segment_time_s: 0.1,
            is_cutting: true,
            cut_kinematics: CutKinematics::Linear,
            feed_rate_mm_min: feed_mm_min,
            spindle_rpm: rpm,
            flute_count: FLUTES,
            axial_doc_mm: AP_MM,
            axial_engagement_mm: AP_MM,
            arc_engagement_radians: Some(arc),
            chipload_mm_per_tooth: fz,
            effective_chip_thickness_mm: Some(chip),
            engagement: Engagement::with_radial_woc(AE_MM / D_MM),
            removed_volume_est_mm3: 0.1,
            mrr_mm3_s: 1.0,
            ..SimulationCutSample::test_fixture()
        })
        .collect();
    SimulationCutTrace {
        sample_step_mm: 1.0,
        samples,
        predicted_feeds: PredictedFeedMap::new(),
        ..SimulationCutTrace::test_fixture()
    }
}

#[test]
fn suggests_own_light_stepover_recipe_is_not_above_the_band_p11b2() {
    let r = suggest();
    assert!(
        r.derates.power_limit >= 1.0 && r.derates.feed_clamp >= 1.0,
        "fixture: no power or feed-ceiling limit binds ({:?})",
        r.warnings
    );
    let rpm = r.rpm.round();
    assert!(
        (rpm - r.rpm).abs() < 1e-9,
        "fixture: Suggest ships a whole RPM ({})",
        r.rpm
    );
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "test: a whole, positive RPM checked above"
    )]
    let rpm_u32 = rpm as u32;
    let fz = r.feed_rate_mm_min / (rpm * f64::from(FLUTES));
    let lift = rs_cam_core::feeds::geometry::peak_chip_thinning_lift(AE_MM, D_MM);
    assert!(lift > 1.0, "fixture: a light cut lifts the feed ({lift})");

    let tool = end_mill();
    let material = material();
    let trace = trace(r.feed_rate_mm_min, rpm_u32);
    let verdict = evaluate_toolpath(
        &ToolpathLoadContext::of_operation(
            TOOLPATH,
            &rs_cam_core::compute::catalog::OperationConfig::new_default(OperationType::Pocket),
            &tool,
            &material,
            rs_cam_core::machine::SteadyFeedMmMin::op_feed_only(r.feed_rate_mm_min),
            None,
            rs_cam_core::limits::CapTable::empty(),
            None,
        ),
        Some(&trace),
        None,
        &ToleranceBands::default(),
    );

    // Arm 1: the premise. The advance per tooth is above the band maximum.
    let band_max = match &verdict.chipload {
        ChiploadVerdict::Within {
            approach_to_max, ..
        } => approach_to_max.bounds.max_mm_per_tooth,
        ChiploadVerdict::Exceeds { triggering, .. } => triggering.bounds.max_mm_per_tooth,
        ChiploadVerdict::Unmodeled { reason } => {
            panic!("fixture: the gate must reach a modelled verdict; got Unmodeled({reason:?})")
        }
    };
    assert!(
        fz > band_max,
        "fixture: the Suggest advance per tooth {fz} is above the band max {band_max} \
         (band chip {}, lift {lift})",
        r.chip_load_mm
    );

    // Arm 2: no commanded-above-band diagnostic on Suggest's own recipe.
    let ids: Vec<String> = diagnostics_from_load_verdict(&verdict)
        .iter()
        .map(|d| d.id.as_str().to_owned())
        .collect();
    assert!(
        !ids.iter()
            .any(|id| id == "load.chipload.commanded_above_band"),
        "P11b2: Suggest's own recipe (advance {fz}, peak chip {}) must not raise \
         commanded_above_band against the band max {band_max}; diagnostics {ids:?}",
        fz / lift
    );

    // Arm 3: the gate observes the peak chip, which is the band chip.
    let observed = match &verdict.chipload {
        ChiploadVerdict::Within {
            approach_to_max, ..
        } => approach_to_max.observed_mm_per_tooth,
        ChiploadVerdict::Exceeds {
            side, triggering, ..
        } => panic!(
            "P11b2: Suggest's own recipe must not trip the gate: Exceeds({side:?}) at {}",
            triggering.observed_mm_per_tooth
        ),
        ChiploadVerdict::Unmodeled { reason } => panic!("unreachable: Unmodeled({reason:?})"),
    };
    let want = fz / lift;
    assert!(
        (observed - want).abs() <= 1e-9 * want.max(1.0),
        "P11b2: the gate observes the peak chip fz / lift = {want}, got {observed}"
    );
    assert!(
        (observed - r.chip_load_mm).abs() <= 1e-6 * r.chip_load_mm,
        "P11b2: the peak chip {observed} is the Suggest band chip {}",
        r.chip_load_mm
    );
}
