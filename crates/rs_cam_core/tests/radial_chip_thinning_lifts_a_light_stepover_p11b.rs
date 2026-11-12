//! Feeds contract P11b, decision L1 (`planning/feeds_contract_2026-11-03/phases/P11.md`):
//! Suggest sizes the feed so that the PEAK chip equals the band chip.
//!
//! The law is radial chip thinning (Garr, `CREDITS.md`): the peak chip is
//! `h = fz sin(psi)` for `psi < 90 degrees` and `h = fz` at or above, where
//! `cos psi = 1 - ae / r`. Thus `fz = h_band / sin(psi)` below half the
//! diameter and `fz = h_band` at or above it. One formula, no cap.
//!
//! The fixture is the rivmap100 end mill (D 6, Z 2) on a roughing pocket in
//! white oak, on a machine whose spindle power and cutting-feed ceiling do
//! not bind (test values, not a machine claim), so no limit moves the feed.
//!
//! Arms:
//!
//! 1. At a light stepover (0.1 D) the advance per tooth is the band chip
//!    over `sin(psi)` (red before P11b: the advance is the band chip).
//! 2. At 0.5 D and at 1.0 D the advance per tooth is the band chip.
//! 3. The feed basis names the law "radial chip thinning (Garr)" (red
//!    before P11b).

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::PocketConfig;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolFact, ToolId, ToolType};
use rs_cam_core::feeds::FeedsInput;
use rs_cam_core::feeds::{FeedsResult, SpindleStrategy, calculate, embedded_vendor_lut};
use rs_cam_core::machine::{MachineProfile, PowerModel};
use rs_cam_core::material::{Material, WoodSpecies};

const D_MM: f64 = 6.0;
const AP_MM: f64 = 1.0;

fn end_mill() -> ToolConfig {
    let mut tool = ToolConfig::test_default(ToolId::from_raw(3), ToolType::EndMill);
    tool.diameter = D_MM;
    tool.helix_deg = 30.0;
    tool.set_cutting_length(ToolFact::Stated(25.0));
    tool.shaft_diameter = 6.35;
    tool.set_shank_diameter(ToolFact::Stated(6.35));
    tool.set_stickout(ToolFact::Stated(45.0));
    tool.set_flute_count(ToolFact::Stated(2));
    tool
}

fn pocket(ae_mm: f64) -> OperationConfig {
    OperationConfig::Pocket(PocketConfig {
        floor: rs_cam_core::compute::floor::Floor::top(12.0),
        depth_per_pass: Some(AP_MM),
        stepover: Some(ae_mm),
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

fn result_at(ae_mm: f64) -> FeedsResult {
    let tool = end_mill();
    let material = Material::SolidWood {
        species: WoodSpecies::WhiteOak,
    };
    let machine = open_machine();
    let op = pocket(ae_mm);
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

fn advance_per_tooth(r: &FeedsResult) -> f64 {
    r.feed_rate_mm_min / (r.rpm * 2.0)
}

fn assert_no_limit_moved(r: &FeedsResult) {
    assert!(
        r.derates.power_limit >= 1.0 && r.derates.feed_clamp >= 1.0,
        "fixture: no power or feed-ceiling limit binds ({:?})",
        r.warnings
    );
}

#[test]
fn a_light_stepover_lifts_the_feed_by_one_over_sin_psi_p11b() {
    let ae = 0.1 * D_MM;
    let r = result_at(ae);
    assert_no_limit_moved(&r);
    let psi = (1.0 - ae / (D_MM / 2.0)).acos();
    assert!(psi < std::f64::consts::FRAC_PI_2, "fixture: a light cut");
    let want = r.chip_load_mm / psi.sin();
    let got = advance_per_tooth(&r);
    assert!(
        (got - want).abs() <= 1e-6 * want,
        "L1: at ae {ae} mm the advance per tooth is {got}, the band chip {} over sin(psi) is \
         {want}",
        r.chip_load_mm
    );
    let feed_basis = format!("{:?}", r.bases.feed);
    assert!(
        feed_basis.contains("radial chip thinning (Garr)"),
        "L1: the feed basis names the law: {feed_basis}"
    );
}

#[test]
fn half_the_diameter_and_wider_do_not_thin_p11b() {
    for ae in [0.5 * D_MM, D_MM] {
        let r = result_at(ae);
        assert_no_limit_moved(&r);
        let got = advance_per_tooth(&r);
        assert!(
            (got - r.chip_load_mm).abs() <= 1e-6 * r.chip_load_mm,
            "L1: at ae {ae} mm (>= D/2) the advance per tooth {got} is the band chip {}",
            r.chip_load_mm
        );
    }
}
