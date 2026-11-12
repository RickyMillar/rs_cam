//! Feeds contract P22-0c-6b, rulings 1 and 2
//! (`planning/feeds_contract_2026-11-03/phases/P22-0c-6.md`, sections 2.4
//! and 3): a tapered ball cuts no wider than its width at the flute top.
//!
//! The defect (D5): the hint doors capped the engaged diameter of a tapered
//! ball at the collet shank, the cutter doors at the shaft. Neither stopped
//! at the flute length `L`. Operator ruling Q3: the cutting zone is the
//! fluted length; the shank above it does not cut. The cap is
//! `ToolBody::max_cut_diameter_mm` = `2 x min(width(L), shaft / 2)`.
//!
//! The claim: the worked table of section 2.4 (wanaka tool 4: tip 2 mm,
//! 5.7106 deg per side, shaft 6, `L` 20) holds through every door that reads
//! an engaged diameter: the hint door (`EngagedDiameter::of_hint`), Suggest
//! (`FeedsResult::effective_diameter_mm`), the operating point
//! (`force_at_operating_point`), the modulator (`ModulationCutter::d_eng_at`),
//! the predictor (`predict_peak_deflection_um`), the cutter door
//! (`EngagedDiameter::of_tool`, `Engagement::from_tool_cut`) and the
//! simulation sample (`Engagement::from_sample`). The hint equals the cutter
//! at every depth, and the stated shank (6.35, the legacy default, or 3.175,
//! below the shaft) moves no value. Red before: the 21 mm and 25 mm rows
//! (the hint gave 6.009980 and 6.35 with the 6.35 shank, the cutter 6.0).
//!
//! The C3 tool (tip 1, 5.26 deg, shaft 6.35, `L` 12) keeps its widths below
//! `L`, bit for bit.

#![expect(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::cutter::build_cutter;
use rs_cam_core::compute::operation_configs::PocketConfig;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolFact, ToolId, ToolType};
use rs_cam_core::dressup::feed_modulation::ModulationCutter;
use rs_cam_core::feeds::FeedsInput;
use rs_cam_core::feeds::engagement::{EngagedDiameter, Engagement};
use rs_cam_core::feeds::envelope::CutPoint;
use rs_cam_core::feeds::force::immersion_angle;
use rs_cam_core::feeds::operating_point::force_at_operating_point;
use rs_cam_core::feeds::predict::tip_deflection_from_engagement;
use rs_cam_core::feeds::{
    CutBody, SpindleStrategy, calculate, embedded_vendor_lut, predict_peak_deflection_um,
};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::force_line::mean_chip_mm;
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::stock::simulation_cut::SimulationCutSample;
use rs_cam_core::tool::{MillingCutter, TaperedBallEndmill};

/// Wanaka tool 4 as the operator file states it.
const TIP_MM: f64 = 2.0;
const HALF_ANGLE_DEG: f64 = 5.7106;
const SHAFT_MM: f64 = 6.0;
const FLUTE_MM: f64 = 20.0;
/// The stickout: above the deepest row, so the predictor has a cantilever.
const STICKOUT_MM: f64 = 40.0;

/// The radial width of the operating point: below every engaged radius of
/// the table, so the arc (and the chip) reads the engaged diameter.
const AE_MM: f64 = 1.0;
const FEED_MM_MIN: f64 = 1500.0;
const RPM: u32 = 18_000;

/// The six-decimal values of the table hold to half a unit in the last
/// place.
const TABLE_TOL: f64 = 5e-7;

/// The worked table of section 2.4: `(ap, the width after 6b)`. The cap is
/// `width(20) = 5.809980`.
const TABLE: [(f64, f64); 6] = [
    (0.2, 1.200000),
    (10.0, 3.809977),
    (20.0, 5.809980),
    (20.5, 5.809980),
    (21.0, 5.809980),
    (25.0, 5.809980),
];

/// The two stated shanks: the legacy default beside a stated shaft, and a
/// shank below the shaft.
const SHANKS: [f64; 2] = [6.35, 3.175];

fn tapered(shank_mm: f64) -> ToolConfig {
    let mut tool = ToolConfig::test_default(ToolId::from_raw(4), ToolType::TaperedBallNose);
    tool.diameter = TIP_MM;
    tool.shaft_diameter = SHAFT_MM;
    tool.set_taper_half_angle(ToolFact::Stated(HALF_ANGLE_DEG));
    tool.set_cutting_length(ToolFact::Stated(FLUTE_MM));
    tool.set_shank_diameter(ToolFact::Stated(shank_mm));
    tool.set_stickout(ToolFact::Stated(STICKOUT_MM));
    tool.set_flute_count(ToolFact::Stated(2));
    tool
}

fn oak() -> Material {
    Material::SolidWood {
        species: WoodSpecies::WhiteOak,
    }
}

fn pocket(ap_mm: f64) -> OperationConfig {
    OperationConfig::Pocket(PocketConfig {
        floor: rs_cam_core::compute::floor::Floor::top(30.0),
        depth_per_pass: Some(ap_mm),
        stepover: Some(AE_MM),
        feed_rate: Some(FEED_MM_MIN),
        plunge_rate: Some(500.0),
        spindle_rpm: Some(RPM),
        ..PocketConfig::default()
    })
}

/// A cutting sample at depth `ap` with a quarter-turn arc.
fn sample(ap_mm: f64) -> SimulationCutSample {
    SimulationCutSample {
        is_cutting: true,
        spindle_rpm: RPM,
        flute_count: 2,
        axial_doc_mm: ap_mm,
        axial_engagement_mm: ap_mm,
        arc_engagement_radians: Some(std::f64::consts::FRAC_PI_2),
        engagement: rs_cam_core::stock::simulation_cut::Engagement {
            // Above the air threshold: the sample removes material.
            radial_woc_fraction: 0.5,
            ..Default::default()
        },
        ..SimulationCutSample::test_fixture()
    }
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-12 * b.abs().max(1.0)
}

#[test]
fn a_tapered_ball_cuts_only_to_its_flute_top_p22_0c6() {
    let cutter = TaperedBallEndmill::new(TIP_MM, HALF_ANGLE_DEG, SHAFT_MM, FLUTE_MM).unwrap();
    let machine = MachineProfile::generic_wood_router();
    let material = oak();
    for shank in SHANKS {
        let tool = tapered(shank);
        let tool_def = build_cutter(&tool).expect("the tool is in its domain");
        let hint = tool.geometry_hint();
        let cap = CutBody::of_tool(&tool).max_cut_diameter_mm;
        assert!(
            (cap - 5.809980).abs() < TABLE_TOL,
            "shank {shank}: cap {cap}"
        );
        let modulator = ModulationCutter {
            geometry: hint,
            diameter_mm: tool.diameter,
            max_cut_diameter_mm: cap,
            helix_deg: tool.helix_deg,
        };
        for (ap, want) in TABLE {
            // The geometry: the cutter's own width at `min(ap, L)`.
            let geometric = 2.0 * cutter.width_at_height(ap.min(FLUTE_MM));
            assert!((geometric - want).abs() < TABLE_TOL, "ap {ap}: {geometric}");

            let of_hint = EngagedDiameter::of_hint(hint, tool.diameter, cap, ap).mm();
            let of_tool = EngagedDiameter::of_tool(&tool_def, ap).mm();
            let tool_cut = Engagement::from_tool_cut(
                CutPoint {
                    ap_mm: ap,
                    ae_mm: AE_MM,
                },
                &tool_def,
            )
            .d_eng_mm();
            let measured = Engagement::from_sample(&sample(ap), &tool_def)
                .expect("a cutting sample engages")
                .d_eng_mm();
            let modulated = modulator.d_eng_at(ap);
            for (door, value) in [
                ("of_hint", of_hint),
                ("of_tool", of_tool),
                ("from_tool_cut", tool_cut),
                ("from_sample", measured),
                ("modulator", modulated),
            ] {
                assert!(
                    close(value, geometric),
                    "shank {shank}, ap {ap}: {door} gives {value}, the cutter {geometric}"
                );
            }

            // Suggest: the Step 5 diameter at the stated depth.
            let result = calculate(&FeedsInput::of_operation(
                &pocket(ap),
                &tool,
                &material,
                &machine,
                embedded_vendor_lut(),
                SpindleStrategy::MatchChart,
            ));
            assert_eq!(
                result.axial_depth_mm.to_bits(),
                ap.to_bits(),
                "stated depth ships"
            );
            assert!(
                close(result.effective_diameter_mm, geometric),
                "shank {shank}, ap {ap}: Suggest gives {}, the cutter {geometric}",
                result.effective_diameter_mm
            );

            // The operating point: the mean chip of the stated cut reads
            // the engaged diameter through the arc.
            let force = force_at_operating_point(&pocket(ap), &tool, &material, None)
                .expect("the white-oak pocket has an operating point");
            let fz = FEED_MM_MIN / (f64::from(RPM) * 2.0);
            let want_chip = mean_chip_mm(fz, immersion_angle(AE_MM, of_tool / 2.0)).unwrap();
            assert!(
                close(force.mean_chip_mm, want_chip),
                "shank {shank}, ap {ap}: operating point chip {} vs the cutter's {want_chip}",
                force.mean_chip_mm
            );

            // The predictor: the cantilever of the cutter's engagement.
            let prediction = predict_peak_deflection_um(&pocket(ap), &tool, &material, &machine)
                .expect("the predictor models the pocket");
            let cutter_um = 1000.0
                * tip_deflection_from_engagement(
                    &tool_def,
                    &material,
                    &Engagement::from_tool_cut(
                        CutPoint {
                            ap_mm: ap,
                            ae_mm: prediction.breakdown.radial_woc_mm,
                        },
                        &tool_def,
                    ),
                    prediction.breakdown.chipload_per_tooth_mm,
                )
                .unwrap();
            assert!(
                close(prediction.predicted_um, cutter_um),
                "shank {shank}, ap {ap}: predictor {} um vs the cutter's {cutter_um} um",
                prediction.predicted_um
            );
        }
    }
}

/// The C3 tool keeps its widths below `L`: the cap does not bind there.
#[test]
fn the_c3_tool_keeps_its_widths_below_the_flute_top_p22_0c6() {
    let mut tool = ToolConfig::test_default(ToolId::from_raw(5), ToolType::TaperedBallNose);
    tool.diameter = 1.0;
    tool.shaft_diameter = 6.35;
    tool.set_taper_half_angle(ToolFact::Stated(5.26));
    tool.set_cutting_length(ToolFact::Stated(12.0));
    tool.set_shank_diameter(ToolFact::Stated(6.35));
    let tool_def = build_cutter(&tool).expect("the C3 tool is in its domain");
    let cutter = TaperedBallEndmill::new(1.0, 5.26, 6.35, 12.0).unwrap();
    let cap = CutBody::of_tool(&tool).max_cut_diameter_mm;
    // Non-vacuity: the cap is below the shaft (the cone meets 6.35 mm at
    // 29.53 mm, above the 12 mm flute).
    assert!(cap < 6.35, "cap {cap}");
    for ap in [0.05, 0.1, 0.25, 0.5, 1.0, 2.0, 4.0] {
        let canonical = 2.0 * cutter.width_at_height(ap);
        let of_tool = EngagedDiameter::of_tool(&tool_def, ap).mm();
        assert_eq!(of_tool.to_bits(), canonical.to_bits(), "ap {ap}");
        let of_hint = EngagedDiameter::of_hint(tool.geometry_hint(), 1.0, cap, ap).mm();
        assert!(
            close(of_hint, canonical),
            "ap {ap}: {of_hint} vs {canonical}"
        );
    }
}
