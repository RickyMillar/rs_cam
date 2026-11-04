//! K7 (feeds contract P9): one engagement definition before and after the
//! simulation.
//!
//! Before P9 three readers built the power inputs of one cut three ways
//! (`planning/feeds_contract_2026-11-03/research/B_feeds_pipeline.md` 4.2):
//!
//! - Suggest (`power_at_operating_point`) read the geometric width `ae`
//!   with `cos psi = 1 - ae / r`.
//! - The post-sim gate (`sample_power_kw`) read the arc-equivalent slab
//!   `(psi / pi) x 2 r`. The two agree at psi = pi/2 and pi only.
//! - The modulator power cap read a rectangle `ap x ae` for every tool; the
//!   gate and Suggest read the cutter's own section (a V-bit is half).
//!
//! Lead decision D-W (2026-11-03): the sample constructor uses the geometric
//! inverse `ae = r (1 - cos psi)`, the exact inverse of `immersion_angle`.
//!
//! Every expected value comes from a function under test: a power from one
//! door is compared with the power of another door at the same cut.

#![expect(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::print_stderr,
    reason = "test code: a panic is how a test reports a failure, and a parity sentry prints the two numbers it compares"
)]

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::FaceConfig;
use rs_cam_core::compute::tool_config::ToolFact;
use rs_cam_core::compute::{ToolConfig, ToolId, ToolType, build_cutter};
use rs_cam_core::dressup::feed_modulation::{
    BindingConstraint, ChiploadBand, ModulationContext, ModulationStrategy, PerMoveEngagement,
    PowerLimitInputs, adaptive_feed_modulate,
};
use rs_cam_core::feeds::engagement::Engagement;
use rs_cam_core::feeds::envelope::CutPoint;
use rs_cam_core::feeds::{effective_diameter, power_at_operating_point};
use rs_cam_core::geo::P3;
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::machine::kinematics::MachineKinematics;
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::stock::simulation_cut::SimulationCutSample;
use rs_cam_core::tool::MillingCutter;
use rs_cam_core::tool_load::power::sample_power_kw;
use rs_cam_core::toolpath::{MoveIntent, MoveType, Toolpath};

const FEED_MM_MIN: f64 = 1500.0;
const RPM: u32 = 18_000;
const FLUTES: u32 = 2;

fn hardwood() -> Material {
    Material::SolidWood {
        species: WoodSpecies::GenericHardwood,
    }
}

fn flat(diameter: f64) -> ToolConfig {
    let mut t = ToolConfig::test_default(ToolId::from_raw(1), ToolType::EndMill);
    t.diameter = diameter;
    t.set_flute_count(ToolFact::Stated(FLUTES));
    t
}

/// A Face states its width and its depth and has no slot case
/// (`FirstPass::NotSlot`, `Radial::Width`), so its pre-sim power is the
/// nominal cut.
fn face(ap: f64, ae: f64) -> OperationConfig {
    OperationConfig::Face(FaceConfig {
        stepover: ae,
        depth: ap,
        depth_per_pass: ap,
        feed_rate: Some(FEED_MM_MIN),
        plunge_rate: Some(300.0),
        spindle_rpm: Some(RPM),
        ..FaceConfig::default()
    })
}

/// A cutting sample of `tool` at depth `ap` and arc `psi`.
fn sample(ap: f64, psi: f64) -> SimulationCutSample {
    SimulationCutSample {
        is_cutting: true,
        spindle_rpm: RPM,
        flute_count: FLUTES,
        axial_doc_mm: ap,
        axial_engagement_mm: ap,
        arc_engagement_radians: Some(psi),
        engagement: rs_cam_core::stock::simulation_cut::Engagement {
            // Above the air threshold: the sample removes material.
            radial_woc_fraction: 0.5,
            ..Default::default()
        },
        ..SimulationCutSample::test_fixture()
    }
}

/// (a) One cut, two doors: the pre-sim door and the post-sim gate give one
/// power. The width sits between r and D (psi between pi/2 and pi), where
/// the old slab and the geometric width differ.
#[test]
fn suggest_and_the_gate_give_one_power_for_one_cut() {
    let machine = MachineProfile::generic_wood_router();
    let material = hardwood();
    let line = material.force_line().expect("hardwood has a force line");
    let diameter = 6.0;
    let tool = flat(diameter);
    let tool_def = build_cutter(&tool).expect("a flat end mill builds");
    let r = diameter / 2.0;
    // Fixture choices: a width of 3/4 D and a depth of D/2.
    let (ap, ae) = (diameter / 2.0, 0.75 * diameter);
    let psi = rs_cam_core::feeds::force::immersion_angle(ae, r);
    assert!(
        psi > std::f64::consts::FRAC_PI_2 && psi < std::f64::consts::PI,
        "the fixture must sit where the two widths differed: psi = {psi}"
    );

    let suggest = power_at_operating_point(&face(ap, ae), &tool, &material, &machine, None)
        .expect("the face cut is modelled")
        .required_kw;
    let gate = sample_power_kw(&tool_def, &line, &sample(ap, psi), FEED_MM_MIN)
        .expect("the sample is a cutting sample");
    eprintln!("suggest {suggest:.9} kW, gate {gate:.9} kW");
    assert!(
        (suggest - gate).abs() <= 1e-9 * suggest.max(1.0),
        "one cut, one power: Suggest {suggest} kW, the gate {gate} kW"
    );

    // The two constructors build one engagement for this cut.
    let d_eng = effective_diameter(tool.geometry_hint(), diameter, diameter, ap);
    let planned = Engagement::from_cut(
        CutPoint {
            ap_mm: ap,
            ae_mm: ae,
        },
        tool.geometry_hint(),
        d_eng,
        Some(tool.helix_deg),
    );
    let measured =
        Engagement::from_sample(&sample(ap, psi), &tool_def).expect("a cutting sample engages");
    let close = |a: f64, b: f64| (a - b).abs() <= 1e-12 * a.abs().max(1.0);
    assert!(
        close(planned.ap_mm(), measured.ap_mm()),
        "{planned:?} vs {measured:?}"
    );
    assert!(
        close(planned.ae_mm(), measured.ae_mm()),
        "{planned:?} vs {measured:?}"
    );
    assert!(
        close(planned.psi_rad(), measured.psi_rad()),
        "{planned:?} vs {measured:?}"
    );
    assert!(
        close(planned.d_eng_mm(), measured.d_eng_mm()),
        "{planned:?} vs {measured:?}"
    );
    assert!(
        close(planned.section_mm2(), measured.section_mm2()),
        "{planned:?} vs {measured:?}"
    );
    assert_eq!(
        planned.helix_deg(),
        measured.helix_deg(),
        "one tool, one helix"
    );
}

fn straight_toolpath(n_cuts: usize) -> Toolpath {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(0.0, 0.0, 0.0));
    for i in 0..n_cuts {
        let x = (i + 1) as f64 * 50.0;
        tp.feed_to_with_intent(P3::new(x, 0.0, -2.0), FEED_MM_MIN, MoveIntent::ClearingCut);
    }
    tp
}

fn cutting_ceiling(mm_min: f64) -> rs_cam_core::machine::CuttingCeilingMmMin {
    MachineProfile {
        max_feed_mm_min: mm_min,
        max_cutting_feed_mm_min: Some(mm_min),
        ..MachineProfile::default()
    }
    .cutting_ceiling()
}

/// (b) A V-bit cut: the modulator's power cap and the gate read one
/// section. The modulator caps the feed at a power budget; the gate,
/// asked for the power at that feed, must give the budget back.
///
/// The width is r (psi = pi/2), where the old slab and the geometric width
/// agree, so only the section can differ here.
#[test]
fn a_vbit_cut_has_one_section_in_the_modulator_and_the_gate() {
    let material = hardwood();
    let line = material.force_line().expect("hardwood has a force line");
    let mut tool = ToolConfig::test_default(ToolId::from_raw(2), ToolType::VBit);
    tool.diameter = 12.0;
    tool.set_flute_count(ToolFact::Stated(FLUTES));
    let tool_def = build_cutter(&tool).expect("a V-bit builds");
    let ap = 2.0;
    let d_eng = 2.0 * tool_def.engagement_radius(ap);
    assert!(d_eng > 0.0, "the V-bit cuts at this depth");
    let woc = 0.5;
    let psi = rs_cam_core::feeds::force::immersion_angle(woc * d_eng, d_eng / 2.0);

    // The budget: the gate's own power at a reference feed, so the budget
    // binds where the gate can read it back. The chip band is wide, so
    // the power cap is the one that binds.
    let reference_feed = 1000.0;
    let budget = sample_power_kw(&tool_def, &line, &sample(ap, psi), reference_feed)
        .expect("the sample is a cutting sample");

    let mut tp = straight_toolpath(2);
    let engagements: Vec<_> = tp
        .moves
        .iter()
        .map(|m| {
            if matches!(m.move_type, MoveType::Rapid) {
                PerMoveEngagement::default()
            } else {
                PerMoveEngagement {
                    radial_woc_fraction: woc,
                    axial_doc_fraction: 1.0,
                    axial_doc_mm: ap,
                }
            }
        })
        .collect();
    let kinematics = MachineKinematics::shapeoko_xxl_stock();
    let ctx = ModulationContext {
        spindle_rpm: f64::from(RPM),
        flute_count: FLUTES,
        cutting_ceiling: cutting_ceiling(100_000.0),
        rapid_feed_mm_min: 100_000.0,
        chipload_band: Some(ChiploadBand::new(1e-6, 10.0).expect("a valid band")),
        kinematics: &kinematics,
        strategy: ModulationStrategy::ConstrainedMax,
        feed_scale: 1.0,
        deflection_inputs: None,
        power_inputs: Some(PowerLimitInputs {
            line,
            engagement_diameter_mm: d_eng,
            geometry: tool.geometry_hint(),
            available_kw: budget,
        }),
        nominal_axial_doc_mm: ap,
        plunge_rate_mm_min: f64::INFINITY,
    };
    let outcome = adaptive_feed_modulate(&mut tp, &engagements, &ctx).unwrap();
    let (feed, binding) = outcome
        .per_move
        .values()
        .find(|(_, b)| *b == BindingConstraint::PowerMax)
        .copied()
        .expect("the power cap binds");
    assert_eq!(binding, BindingConstraint::PowerMax);
    let gate = sample_power_kw(&tool_def, &line, &sample(ap, psi), feed)
        .expect("the sample is a cutting sample");
    eprintln!("budget {budget:.9} kW, modulated feed {feed:.3}, gate at that feed {gate:.9} kW");

    // Suggest's section for the same cut: the planned engagement of the
    // V-bit's own shape equals the sample's.
    let planned = Engagement::from_cut(
        CutPoint {
            ap_mm: ap,
            ae_mm: woc * d_eng,
        },
        tool.geometry_hint(),
        d_eng,
        None,
    );
    let measured =
        Engagement::from_sample(&sample(ap, psi), &tool_def).expect("a cutting sample engages");
    assert!(
        (planned.section_mm2() - measured.section_mm2()).abs()
            <= 1e-12 * planned.section_mm2().max(1.0),
        "one V-bit cut, one section: Suggest {planned:?}, the gate {measured:?}"
    );
    assert!(
        (gate - budget).abs() <= 1e-9 * budget.max(1.0),
        "the modulator capped at {feed} mm/min for {budget} kW; the gate reads {gate} kW there"
    );
}
