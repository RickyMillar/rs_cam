//! Feeds contract M4-A (`planning/feeds_contract_2026-11-03/phases/
//! P29-11_DESIGN.md` section 3.5, `P34_M4A_2026-11-15.md`; operator ruling
//! 2026-11-15, "Full fix after a run"): one simulation sample has ONE depth
//! for every gate and for the feed modulator, the engaged depth
//! `axial_engagement_mm`.
//!
//! The simulation folds the samples of a move that descends or climbs
//! (`dexel_stock::sample_coalesce`). A folded sample holds two depths: the
//! MAXIMUM of the group (`axial_engagement_mm`) and the time MEAN of the
//! group (`axial_doc_mm`). Before M4-A `Engagement::from_sample` read the
//! mean, so the power gate, the chip lift, the cut of a sample, the K7
//! envelope placement and the power cap of the modulator read a shallower
//! cut than the deflection gate and the force reading of the same sample.
//!
//! The fixture (test values, not a machine claim): a 6 mm flat end mill, 2
//! flutes, white oak, 18 000 rpm, a 50 x 50 x 10 mm block. One straight
//! ramp goes from the stock top to 3 mm below it over 30 mm, in solid
//! stock, through the dexel simulation at a 2 mm sample step.
//!
//! Claims:
//!
//! (a) the fixture discriminates: at least one removing sample of the ramp
//!     has an engaged depth above its mean depth, and no sample has an
//!     engaged depth below its mean depth.
//! (b) every door reads the engaged depth: `Engagement::from_sample`
//!     (the power gate, the chip lift, the cut of a sample, the K7
//!     placement) and `tool_load::deflection::sample_engagement` (the
//!     deflection gate, the force reading) give the same depth, width and
//!     engaged diameter for each sample. Red before M4-A.
//! (c) the power gate grades each sample at the engaged depth: its figure
//!     equals the power of a planned cut at that depth and at the measured
//!     width. Red before M4-A.
//! (d) the modulator's side table (`SampleEngagement::of_sample`) holds one
//!     depth in its power and its force entry.
//! (e) the power cap of the modulator holds the ramp at or below the feed
//!     at which its deepest sample, read at the ENGAGED depth, meets the
//!     available power. Red before M4-A: the cap read the mean depth and
//!     emitted a higher feed.

#![expect(
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_core::compute::cutter::build_cutter;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolFact, ToolId, ToolType};
use rs_cam_core::dexel_stock::{StockCutDirection, TriDexelStock};
use rs_cam_core::dressup::feed_modulation::{
    BindingConstraint, ChiploadBand, ModulationContext, ModulationCutter, ModulationMode,
    ModulationStrategy, MoveSampleEngagements, PerMoveEngagement, PowerLimitInputs, RadialThinning,
    SampleEngagement, adaptive_feed_modulate,
};
use rs_cam_core::feeds::cut_load::{CutLoad, OperatingPoint};
use rs_cam_core::feeds::engagement::Engagement;
use rs_cam_core::feeds::envelope::CutPoint;
use rs_cam_core::geo::{BoundingBox3, P3};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::machine::kinematics::MachineKinematics;
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::stock::simulation_cut::SimulationCutSample;
use rs_cam_core::tool::ToolDefinition;
use rs_cam_core::toolpath::{MoveIntent, Toolpath};

const D_MM: f64 = 6.0;
const FLUTES: u32 = 2;
const RPM: u32 = 18_000;
const FLUTE_LENGTH_MM: f64 = 25.0;
const STOCK_TOP_Z: f64 = 10.0;
const RAMP_DEPTH_MM: f64 = 3.0;
const RAMP_LENGTH_MM: f64 = 30.0;
const SAMPLE_STEP_MM: f64 = 2.0;
/// The feed (mm/min) at which the deepest sample of the ramp, read at its
/// engaged depth, meets the power budget of claim (e).
const FIT_FEED: f64 = 600.0;

fn end_mill() -> ToolConfig {
    let mut tool = ToolConfig::test_default(ToolId::from_raw(3), ToolType::EndMill);
    tool.diameter = D_MM;
    tool.helix_deg = 30.0;
    tool.set_cutting_length(ToolFact::Stated(FLUTE_LENGTH_MM));
    tool.shaft_diameter = 6.35;
    tool.set_shank_diameter(ToolFact::Stated(6.35));
    tool.set_stickout(ToolFact::Stated(45.0));
    tool.set_flute_count(ToolFact::Stated(FLUTES));
    tool
}

fn white_oak() -> Material {
    Material::SolidWood {
        species: WoodSpecies::WhiteOak,
    }
}

/// The ramp: move 0 is the rapid to the stock top, move 1 the ramp.
fn ramp() -> Toolpath {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(10.0, 25.0, STOCK_TOP_Z));
    tp.feed_to_with_intent(
        P3::new(10.0 + RAMP_LENGTH_MM, 25.0, STOCK_TOP_Z - RAMP_DEPTH_MM),
        1000.0,
        MoveIntent::ClearingCut,
    );
    tp
}

/// The removing samples of the ramp, from the dexel simulation.
fn ramp_samples(tool: &ToolDefinition) -> Vec<SimulationCutSample> {
    let bbox = BoundingBox3 {
        min: P3::new(0.0, 0.0, 0.0),
        max: P3::new(50.0, 50.0, STOCK_TOP_Z),
    };
    let mut stock = TriDexelStock::from_bounds(&bbox, 0.25);
    let never_cancel = || false;
    stock
        .simulate_toolpath_with_metrics_with_cancel(
            &ramp(),
            tool,
            StockCutDirection::FromTop,
            ToolpathId::from_raw(0),
            RPM,
            FLUTES,
            3000.0,
            SAMPLE_STEP_MM,
            None,
            &[],
            &[],
            true,
            &never_cancel,
        )
        .expect("the simulation is not cancelled")
        .into_iter()
        .filter(SimulationCutSample::removes_material)
        .collect()
}

/// A planned side cut of `tool` at the depth `ap_mm` and the measured arc
/// of `sample`: the engagement that every door must give for the sample.
fn engagement_at(tool: &ToolDefinition, sample: &SimulationCutSample, ap_mm: f64) -> Engagement {
    let arc = sample
        .arc_engagement_radians
        .expect("the sample has an arc");
    let r = rs_cam_core::tool::MillingCutter::engagement_radius(tool, ap_mm);
    Engagement::from_tool_cut(
        CutPoint {
            ap_mm,
            ae_mm: r * (1.0 - arc.clamp(0.0, std::f64::consts::PI).cos()),
        },
        tool,
    )
}

/// The samples of the ramp that carry an arc and an engaged depth: the
/// samples that the power gate and the deflection gate grade.
fn graded(samples: &[SimulationCutSample]) -> Vec<&SimulationCutSample> {
    samples
        .iter()
        .filter(|s| {
            !s.is_air()
                && s.axial_engagement_mm > 0.0
                && s.arc_engagement_radians.is_some_and(|a| a > 0.0)
        })
        .collect()
}

#[test]
fn a_ramp_sample_holds_a_peak_depth_above_its_mean_depth_m4() {
    // (a)
    let tool = build_cutter(&end_mill()).expect("a valid tool");
    let samples = ramp_samples(&tool);
    let differ = samples
        .iter()
        .filter(|s| s.axial_engagement_mm > s.axial_doc_mm + 1e-6)
        .count();
    let engaged_smaller = samples
        .iter()
        .filter(|s| s.axial_engagement_mm < s.axial_doc_mm - 1e-12)
        .count();
    assert!(
        differ > 0,
        "fixture: no sample of the ramp ({} samples) has a peak depth above its mean depth",
        samples.len()
    );
    assert_eq!(
        engaged_smaller, 0,
        "a maximum is never below the time mean of the same values"
    );
}

#[test]
fn every_door_reads_the_engaged_depth_of_a_ramp_sample_m4() {
    // (b), (c), (d)
    let tool = build_cutter(&end_mill()).expect("a valid tool");
    let line = white_oak()
        .force_line()
        .expect("white oak has a force line");
    let samples = ramp_samples(&tool);
    let graded = graded(&samples);
    assert!(
        graded
            .iter()
            .any(|s| s.axial_engagement_mm > s.axial_doc_mm + 1e-6),
        "fixture: no graded sample has two depths"
    );
    for s in graded {
        let engaged = s.axial_engagement_mm;
        let power_door = Engagement::from_sample(s, &tool).expect("the power door grades it");
        let force_door = rs_cam_core::tool_load::deflection::sample_engagement(&tool, s)
            .expect("the deflection door grades it");
        // (b) one depth, one width, one engaged diameter.
        assert!(
            (power_door.ap_mm() - engaged).abs() < 1e-12,
            "M4-A (b): `Engagement::from_sample` reads {} mm; the engaged depth of the sample \
             is {engaged} mm (its mean depth is {} mm)",
            power_door.ap_mm(),
            s.axial_doc_mm
        );
        assert!((force_door.ap_mm() - engaged).abs() < 1e-12);
        assert!((power_door.ae_mm() - force_door.ae_mm()).abs() < 1e-9);
        assert!((power_door.d_eng_mm() - force_door.d_eng_mm()).abs() < 1e-9);

        // (c) the power gate's figure is the power of that cut.
        let feed = s.feed_rate_mm_min;
        let gate_kw = rs_cam_core::tool_load::power::sample_power_kw(&tool, &line, s, feed)
            .expect("the power gate grades it");
        let expected_kw = CutLoad::of_line(
            &engagement_at(&tool, s, engaged),
            OperatingPoint::of_sample(s, &tool, feed),
            Some(&tool),
            Some(line),
        )
        .power_kw()
        .expect("the planned cut has a power");
        assert!(
            (gate_kw - expected_kw).abs() <= expected_kw * 1e-9,
            "M4-A (c): the power gate grades {gate_kw} kW; the cut at the engaged depth \
             {engaged} mm needs {expected_kw} kW"
        );

        // (d) the side table of the modulator.
        let entry = SampleEngagement::of_sample(s, &tool);
        let (power, force) = (
            entry.power.expect("a power entry"),
            entry.force.expect("a force entry"),
        );
        assert!(
            (power.ap_mm() - force.ap_mm()).abs() < 1e-12,
            "M4-A (d): the side table holds {} mm for the power cap and {} mm for the force \
             caps",
            power.ap_mm(),
            force.ap_mm()
        );
    }
}

#[test]
fn the_power_cap_holds_a_ramp_at_its_engaged_depth_m4() {
    // (e)
    let tool_cfg = end_mill();
    let tool = build_cutter(&tool_cfg).expect("a valid tool");
    let line = white_oak()
        .force_line()
        .expect("white oak has a force line");
    let samples = ramp_samples(&tool);
    let graded = graded(&samples);

    // The power of one sample at `feed`, read at the ENGAGED depth through
    // the planned-cut door (not through the door under test).
    let kw_at = |s: &SimulationCutSample, feed: f64| {
        CutLoad::of_line(
            &engagement_at(&tool, s, s.axial_engagement_mm),
            OperatingPoint::of_sample(s, &tool, feed),
            Some(&tool),
            Some(line),
        )
        .power_kw()
        .expect("the planned cut has a power")
    };
    // The budget: the largest power over the samples at FIT_FEED.
    let available_kw = graded
        .iter()
        .map(|s| kw_at(s, FIT_FEED))
        .fold(0.0_f64, f64::max);
    assert!(available_kw > 0.0, "fixture: the ramp has a power");

    // The session's inputs: the per-move mean and the side table.
    let n = samples.len() as f64;
    let mean_axial_mm = samples.iter().map(|s| s.axial_doc_mm).sum::<f64>() / n;
    let mean_woc = samples
        .iter()
        .map(|s| s.engagement.radial_woc_fraction)
        .sum::<f64>()
        / n;
    let engagements = vec![
        PerMoveEngagement::default(),
        PerMoveEngagement {
            radial_woc_fraction: mean_woc,
            axial_doc_fraction: mean_axial_mm / FLUTE_LENGTH_MM,
            mean_axial_mm,
            thinning_lift: None,
        },
    ];
    let side = vec![
        MoveSampleEngagements::default(),
        MoveSampleEngagements {
            samples: samples
                .iter()
                .map(|s| SampleEngagement::of_sample(s, &tool))
                .collect(),
            mean_binds: false,
        },
    ];
    let kinematics = MachineKinematics::default();
    let machine = MachineProfile {
        max_feed_mm_min: 1.0e7,
        max_cutting_feed_mm_min: Some(1.0e7),
        ..MachineProfile::default()
    };
    // The floor (36 mm/min) is below the cap; the maximum is above it.
    let band = ChiploadBand::new(0.001, 1.0).expect("a valid band");
    let ctx = ModulationContext {
        spindle_rpm: f64::from(RPM),
        flute_count: FLUTES,
        cutting_ceiling: machine.cutting_ceiling(),
        rapid_feed_mm_min: 1.0e7,
        chipload_band: Some(band),
        radial_thinning: RadialThinning::Applied,
        kinematics: &kinematics,
        strategy: ModulationStrategy::ConstrainedMax,
        mode: ModulationMode::Full,
        feed_scale: 1.0,
        deflection_inputs: None,
        power_inputs: Some(PowerLimitInputs {
            line,
            cutter: ModulationCutter {
                geometry: tool_cfg.geometry_hint(),
                diameter_mm: tool_cfg.diameter,
                max_cut_diameter_mm: rs_cam_core::feeds::CutBody::of_tool(&tool_cfg)
                    .max_cut_diameter_mm,
                helix_deg: tool_cfg.helix_deg,
            },
            available_kw,
        }),
        force_budget_inputs: None,
        sample_engagements: &side,
        nominal_axial_doc_mm: RAMP_DEPTH_MM,
        // The ramp is 5.7 degrees: above every plunge guard of the fixture.
        plunge_rate_mm_min: 1.0e7,
    };
    // A long copy of the ramp direction is not needed: the power cap is far
    // below the reach of a 30 mm move on the default kinematics.
    let mut tp = ramp();
    let outcome = adaptive_feed_modulate(&mut tp, &engagements, &ctx).expect("a valid context");
    let (feed, binding) = *outcome.per_move.get(&1).expect("the ramp is modulated");
    assert_eq!(binding, BindingConstraint::PowerMax, "the power cap binds");
    let worst_kw = graded
        .iter()
        .map(|s| kw_at(s, feed))
        .fold(0.0_f64, f64::max);
    assert!(
        worst_kw <= available_kw * (1.0 + 1e-9),
        "M4-A (e): at the modulated feed {feed} mm/min the deepest sample of the ramp needs \
         {worst_kw} kW at its engaged depth; the available power is {available_kw} kW (the \
         fit feed is {FIT_FEED} mm/min)"
    );
    assert!(
        (feed - FIT_FEED).abs() <= FIT_FEED * 1e-6,
        "M4-A (e): the cap is exact: {feed} mm/min against the fit feed {FIT_FEED} mm/min"
    );
}
