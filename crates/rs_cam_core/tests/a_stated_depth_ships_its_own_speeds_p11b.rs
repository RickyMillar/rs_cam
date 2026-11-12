//! Feeds contract P11b, the P11a lead item (`planning/feeds_contract_2026-11-03/phases/P11.md`,
//! "Result P11a", "For the lead"): a STATED depth above the flute guard
//! ships, so Suggest sizes the feed and the power at that depth. P16 (D12)
//! deleted the slot cap and its arm.
//!
//! Before P11b, `calculate` (`DepthClamps::Apply`) lowered the depth to the
//! cap, sized the feed and the power there, and the stated depth shipped
//! with them. Now the stated depth stays and the warning stays (ruling Q6:
//! warn, never refuse).
//!
//! Fixture: a 6 mm end mill cuts a full-width slot (Profile states `ae = D`)
//! 3 mm deep in white oak.
//!
//! Arms (each red before P11b, which returned the capped 1.5 mm depth):
//!
//! 1. The result's depth is the stated depth.
//! 2. The result's power equals `power_at_operating_point` of an op that
//!    states that depth, width, feed and RPM.
//! 3. The flute guard: a stated depth above it ships, with its warning.

#![expect(
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolFact, ToolId, ToolType};
use rs_cam_core::feeds::FeedsInput;
use rs_cam_core::feeds::suggest::CalculatorOperatingPoint;
use rs_cam_core::feeds::{
    FeedsResult, FeedsWarning, SpindleStrategy, calculate, embedded_vendor_lut,
    power_at_operating_point,
};
use rs_cam_core::machine::{MachineProfile, PowerModel};
use rs_cam_core::material::{Material, WoodSpecies};

const D_MM: f64 = 6.0;

fn end_mill() -> ToolConfig {
    let mut tool = ToolConfig::test_default(ToolId::from_raw(3), ToolType::EndMill);
    tool.diameter = D_MM;
    tool.set_cutting_length(ToolFact::Stated(10.0));
    tool.set_stickout(ToolFact::Stated(30.0));
    tool.set_flute_count(ToolFact::Stated(2));
    tool
}

fn open_machine() -> MachineProfile {
    MachineProfile {
        power: PowerModel::ConstantPower { power_kw: 50.0 },
        max_feed_mm_min: 1.0e7,
        max_cutting_feed_mm_min: Some(1.0e7),
        ..MachineProfile::default()
    }
}

fn oak() -> Material {
    Material::SolidWood {
        species: WoodSpecies::WhiteOak,
    }
}

fn slot_op(depth_mm: f64) -> OperationConfig {
    let mut op = rs_cam_core::compute::fixture_values::fixture_op(OperationType::Profile);
    assert!(
        op.set_depth_per_pass(depth_mm),
        "fixture: Profile has a DPP"
    );
    op
}

fn result_of(op: &OperationConfig) -> FeedsResult {
    let tool = end_mill();
    let material = oak();
    let machine = open_machine();
    let input = FeedsInput::of_operation(
        op,
        &tool,
        &material,
        &machine,
        embedded_vendor_lut(),
        SpindleStrategy::MatchChart,
    );
    calculate(&input)
}

fn assert_ships(stated: f64, warned: impl Fn(&FeedsWarning) -> bool) {
    let op = slot_op(stated);
    let r = result_of(&op);
    assert!(
        (r.axial_depth_mm - stated).abs() < 1e-12,
        "the stated depth {stated} mm ships; the result holds {} mm",
        r.axial_depth_mm
    );
    assert!(
        r.warnings.iter().any(&warned),
        "the warning stays: {:?}",
        r.warnings
    );

    // The shipped point: the stated depth and the result's width, feed and
    // RPM (the op holds no RPM, so the reader takes the exact one).
    let mut shipped = op;
    shipped.set_feed_rate(r.feed_rate_mm_min);
    shipped.set_spindle_rpm(None);
    let point = CalculatorOperatingPoint {
        radial_width_mm: r.radial_width_mm,
        axial_depth_mm: r.axial_depth_mm,
        feed_rate_mm_min: r.feed_rate_mm_min,
        rpm: r.rpm,
    };
    let want =
        power_at_operating_point(&shipped, &end_mill(), &oak(), &open_machine(), Some(point))
            .expect("white oak has a power figure")
            .required_kw;
    let published = r.power_kw.expect("white oak has a calculator power");
    assert!(
        (published - want).abs() <= 1e-9 * want.max(1e-12),
        "the published power {published} kW is the power of the depth that ships, {want} kW"
    );
}

#[test]
fn a_stated_depth_above_the_flute_guard_ships_its_speeds_p11b() {
    // The guard is 0.8 x 10 = 8 mm.
    assert_ships(9.0, |w| matches!(w, FeedsWarning::DocExceedsFlute { .. }));
}
