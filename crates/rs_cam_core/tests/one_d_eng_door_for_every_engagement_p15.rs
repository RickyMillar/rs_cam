//! Feeds contract P15: one engaged-diameter (`d_eng`) door.
//!
//! The defect (R30): the hint rule `feeds::effective_diameter` gave a bull
//! nose below its corner radius the corner circle only
//! (`ball_effective_diameter(2 r, ap)`) and dropped the flat centre
//! `D - 2 r`. A 6 mm r1 tool at ap 0.2 got 1.2 mm against the true 5.2 mm.
//! The cutter rule `2 x engagement_radius(ap)` was right.
//!
//! The claim: a 6 mm r1 bull nose at ap 0.2, 0.5 and 1.0 gives the same
//! `d_eng` through every `Engagement` constructor (`from_cut` through
//! `EngagedDiameter::of_hint`, `from_tool_cut`, `from_sample`) and through
//! the modulator's `ModulationCutter::d_eng_at`. The value is the corner
//! chord plus the flat centre, derived here from the geometry:
//! `2 sqrt(r^2 - (r - ap)^2) + (D - 2 r)`.
//!
//! P22-0c-6b closes the tapered residual: a tapered ball whose shank (6.35)
//! differs from its shaft (6) gives one `d_eng` through the same four doors
//! at every depth, the cutter's width at `min(ap, L)` (the body's cutting
//! cap, ruling 1). Before 6b the hint capped at the shank and the cutter at
//! the shaft, so the two split above the flute top.

#![expect(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_core::compute::tool_config::ToolMaterial;
use rs_cam_core::dressup::feed_modulation::ModulationCutter;
use rs_cam_core::feeds::ToolGeometryHint;
use rs_cam_core::feeds::engagement::{EngagedDiameter, Engagement};
use rs_cam_core::feeds::envelope::CutPoint;
use rs_cam_core::stock::simulation_cut::SimulationCutSample;
use rs_cam_core::tool::{BullNoseEndmill, MillingCutter, TaperedBallEndmill, ToolDefinition};

/// The fixture tool: a 6 mm bull nose with a 1 mm corner radius.
const D: f64 = 6.0;
const R: f64 = 1.0;
/// The depths of the claim: below, inside and at the corner radius.
const DEPTHS: [f64; 3] = [0.2, 0.5, 1.0];

fn bull_tool() -> ToolDefinition {
    ToolDefinition::new(
        Box::new(BullNoseEndmill::new(D, R, 20.0).unwrap()),
        D,
        30.0,
        2,
        ToolMaterial::Carbide,
    )
}

fn hint() -> ToolGeometryHint {
    ToolGeometryHint::Bull { corner_radius: R }
}

/// A cutting sample at depth `ap` with a quarter-turn arc.
fn sample(ap: f64) -> SimulationCutSample {
    SimulationCutSample {
        is_cutting: true,
        spindle_rpm: 18_000,
        flute_count: 2,
        axial_doc_mm: ap,
        axial_engagement_mm: ap,
        arc_engagement_radians: Some(std::f64::consts::FRAC_PI_2),
        engagement: rs_cam_core::stock::simulation_cut::Engagement {
            // Above the air threshold: the sample removes material.
            radial_woc_fraction: 0.5,
            ..Default::default()
        },
        ..SimulationCutSample::test_fixture()
    }
}

/// The corner chord plus the flat centre, from the geometry.
fn expected_d_eng(ap: f64) -> f64 {
    2.0 * (R * R - (R - ap) * (R - ap)).sqrt() + (D - 2.0 * R)
}

#[test]
fn a_bull_nose_has_one_engaged_diameter_through_every_door_p15() {
    let tool = bull_tool();
    let modulator = ModulationCutter {
        geometry: hint(),
        diameter_mm: D,
        max_cut_diameter_mm: D,
        helix_deg: 30.0,
    };
    for ap in DEPTHS {
        let cut = CutPoint {
            ap_mm: ap,
            ae_mm: 0.25 * D,
        };
        let planned = Engagement::from_cut(
            cut,
            hint(),
            EngagedDiameter::of_hint(hint(), D, D, ap),
            None,
        )
        .d_eng_mm();
        let of_tool = Engagement::from_tool_cut(cut, &tool).d_eng_mm();
        let measured = Engagement::from_sample(&sample(ap), &tool)
            .expect("a cutting sample engages")
            .d_eng_mm();
        let modulated = modulator.d_eng_at(ap);

        // One door: the four readings are equal bit for bit.
        assert_eq!(planned, of_tool, "from_cut vs from_tool_cut at ap {ap}");
        assert_eq!(planned, measured, "from_cut vs from_sample at ap {ap}");
        assert_eq!(planned, modulated, "from_cut vs the modulator at ap {ap}");

        // The geometry: the corner chord plus the flat centre. The cutter
        // adds the terms in a different order, so the bound is a few ulp.
        let want = expected_d_eng(ap);
        assert!(
            (planned - want).abs() <= 1e-12 * want,
            "d_eng at ap {ap}: {planned} mm, the geometry gives {want} mm"
        );
        // Non-vacuity: the retired rule (the corner circle only) is far
        // from the claim at every depth of the fixture.
        let corner_circle_only = 2.0 * (ap * (2.0 * R - ap)).sqrt();
        assert!(
            (planned - corner_circle_only).abs() >= 0.5 * (D - 2.0 * R),
            "the flat centre is in d_eng at ap {ap}"
        );
    }
}

/// The tapered fixture (P22-0c-6b): wanaka tool 4, tip 2 mm, 5.7106 deg per
/// side, shaft 6, flute 20, with the legacy shank 6.35 (shank != shaft).
const TAPER_TIP: f64 = 2.0;
const TAPER_DEG: f64 = 5.7106;
const TAPER_SHAFT: f64 = 6.0;
const TAPER_FLUTE: f64 = 20.0;
const TAPER_SHANK: f64 = 6.35;
/// Inside the ball, on the cone, at the flute top and above it.
const TAPER_DEPTHS: [f64; 5] = [0.2, 10.0, 20.0, 21.0, 25.0];

#[test]
fn a_tapered_ball_has_one_engaged_diameter_through_every_door_p22_0c6() {
    let tool = ToolDefinition::new(
        Box::new(TaperedBallEndmill::new(TAPER_TIP, TAPER_DEG, TAPER_SHAFT, TAPER_FLUTE).unwrap()),
        TAPER_SHANK,
        40.0,
        2,
        ToolMaterial::Carbide,
    );
    let hint = ToolGeometryHint::TaperedBall {
        tip_radius: TAPER_TIP / 2.0,
        taper_angle_deg: TAPER_DEG,
    };
    let cap = tool.body().max_cut_diameter_mm();
    // Non-vacuity: the shank is not the cap, and the cap is below the shaft.
    assert!(cap < TAPER_SHAFT && TAPER_SHANK > TAPER_SHAFT, "cap {cap}");
    let modulator = ModulationCutter {
        geometry: hint,
        diameter_mm: TAPER_TIP,
        max_cut_diameter_mm: cap,
        helix_deg: 30.0,
    };
    for ap in TAPER_DEPTHS {
        let cut = CutPoint {
            ap_mm: ap,
            ae_mm: 0.5,
        };
        let planned = Engagement::from_cut(
            cut,
            hint,
            EngagedDiameter::of_hint(hint, TAPER_TIP, cap, ap),
            None,
        )
        .d_eng_mm();
        let of_tool = Engagement::from_tool_cut(cut, &tool).d_eng_mm();
        let measured = Engagement::from_sample(&sample(ap), &tool)
            .expect("a cutting sample engages")
            .d_eng_mm();
        let modulated = modulator.d_eng_at(ap);
        let want = 2.0 * tool.width_at_height(ap.min(TAPER_FLUTE));
        for (door, value) in [
            ("from_cut", planned),
            ("from_tool_cut", of_tool),
            ("from_sample", measured),
            ("modulator", modulated),
        ] {
            assert!(
                (value - want).abs() <= 1e-12 * want,
                "{door} at ap {ap}: {value} mm, the cutter at min(ap, L) gives {want} mm"
            );
        }
    }
}
