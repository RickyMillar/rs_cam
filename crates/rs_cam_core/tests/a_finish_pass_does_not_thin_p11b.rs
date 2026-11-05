//! Feeds contract P11b, decision L2 (`planning/feeds_contract_2026-11-03/phases/P11.md`):
//! a finish op does not apply radial chip thinning.
//!
//! Garr's finish row (2.5 % D) holds a low chip on purpose (step 0 claim
//! C13, a finish-quality term). An op whose feeds pass role is `Finish`
//! keeps `fz = h_band` at any stepover, and the feed basis says so. A
//! roughing op at the same light width thins (the partner arm, which
//! keeps this test from passing on a calculator that never thins).
//!
//! Red before P11b: the feed basis does not name the finish exception, and
//! the roughing partner does not thin.

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolFact, ToolId, ToolType};
use rs_cam_core::feeds::suggest::feeds_input_for_operation;
use rs_cam_core::feeds::{FeedsResult, PassRole, SpindleStrategy, calculate, embedded_vendor_lut};
use rs_cam_core::machine::{MachineProfile, PowerModel};
use rs_cam_core::material::{Material, WoodSpecies};

const D_MM: f64 = 6.0;
const AE_MM: f64 = 0.6;

fn end_mill() -> ToolConfig {
    let mut tool = ToolConfig::test_default(ToolId::from_raw(3), ToolType::EndMill);
    tool.diameter = D_MM;
    tool.set_cutting_length(ToolFact::Stated(25.0));
    tool.set_stickout(ToolFact::Stated(45.0));
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

fn result_of(op: &OperationConfig) -> FeedsResult {
    let tool = end_mill();
    let material = Material::SolidWood {
        species: WoodSpecies::WhiteOak,
    };
    let machine = open_machine();
    let input = feeds_input_for_operation(
        op,
        &tool,
        &material,
        &machine,
        embedded_vendor_lut(),
        SpindleStrategy::MatchChart,
    );
    calculate(&input)
}

fn op_at(kind: OperationType) -> OperationConfig {
    let mut op = rs_cam_core::compute::fixture_values::fixture_op(kind);
    assert!(op.set_stepover(AE_MM), "fixture: {kind:?} has a stepover");
    let _ = op.set_depth_per_pass(1.0);
    op
}

#[test]
fn a_finish_op_keeps_the_band_chip_at_a_light_stepover_p11b() {
    let finish = op_at(OperationType::DropCutter);
    assert_eq!(finish.feeds_style().1, PassRole::Finish, "fixture");
    let psi = (1.0 - AE_MM / (D_MM / 2.0)).acos();
    assert!(psi < std::f64::consts::FRAC_PI_2, "fixture: a light cut");

    let r = result_of(&finish);
    let advance = r.feed_rate_mm_min / (r.rpm * 2.0);
    assert!(
        (advance - r.chip_load_mm).abs() <= 1e-6 * r.chip_load_mm,
        "L2: a finish op keeps the band chip {}; it ships {advance}",
        r.chip_load_mm
    );
    let basis = format!("{:?}", r.bases.feed);
    assert!(
        basis.contains("finish pass: no chip thinning"),
        "L2: the feed basis names the finish exception: {basis}"
    );

    // The partner: a roughing pocket at the same width thins.
    let rough = op_at(OperationType::Pocket);
    assert_eq!(rough.feeds_style().1, PassRole::Roughing, "fixture");
    let r = result_of(&rough);
    let advance = r.feed_rate_mm_min / (r.rpm * 2.0);
    let want = r.chip_load_mm / psi.sin();
    assert!(
        (advance - want).abs() <= 1e-6 * want,
        "L1 partner: the roughing pocket thins to {want}; it ships {advance}"
    );
}
