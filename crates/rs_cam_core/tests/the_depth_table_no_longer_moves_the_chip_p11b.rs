//! Feeds contract P11b, decision L6 (`planning/feeds_contract_2026-11-03/phases/P11.md`):
//! chip first. The vendor depth table (Onsrud, Freud, Amana: 1 x D full,
//! 2 x D less 25 %, 3 x D less 50 %) no longer scales the chip or the feed.
//!
//! Step 0 (`research_p11_step0/REPORT.md` section 5) shows that the table
//! holds neither the force, the chip nor the peak chip, so the table is a
//! drawn limit of the envelope (sentry
//! `the_vendor_depth_table_is_a_limit_that_warns_p11b`), not a feed factor.
//!
//! Fixture: a 6 mm end mill (flute 25 mm) on a roughing pocket in white oak
//! at half the diameter (no slot cap, no chip thinning), on a machine whose
//! spindle power and feed ceiling do not bind (test values).
//!
//! Arm: at 1 x D, 2 x D and 3 x D the advance per tooth and the chipload
//! band (or the printed point) are the same (red before P11b: 0.75 and 0.50 of the 1 x D value).

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::PocketConfig;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolFact, ToolId, ToolType};
use rs_cam_core::feeds::FeedsInput;
use rs_cam_core::feeds::{FeedsResult, SpindleStrategy, calculate, embedded_vendor_lut};
use rs_cam_core::machine::{MachineProfile, PowerModel};
use rs_cam_core::material::{Material, WoodSpecies};

const D_MM: f64 = 6.0;

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

fn result_at(ap_mm: f64) -> FeedsResult {
    let tool = end_mill();
    let material = Material::SolidWood {
        species: WoodSpecies::WhiteOak,
    };
    let machine = open_machine();
    let op = OperationConfig::Pocket(PocketConfig {
        floor: rs_cam_core::compute::floor::Floor::top(30.0),
        depth_per_pass: Some(ap_mm),
        stepover: Some(0.5 * D_MM),
        feed_rate: Some(1800.0),
        plunge_rate: Some(500.0),
        spindle_rpm: Some(18_000),
        ..PocketConfig::default()
    });
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

#[test]
fn the_chip_and_the_band_hold_at_every_printed_depth_p11b() {
    let one = result_at(D_MM);
    let chip_one = one.feed_rate_mm_min / (one.rpm * 2.0);
    let band_one = one.chipload_bounds;
    let point_one = one.chipload_point_mm;
    assert!(
        band_one.is_some() || point_one.is_some(),
        "fixture: the row prints a band or a point"
    );
    for k in [2.0, 3.0] {
        let r = result_at(k * D_MM);
        assert!(
            (r.rpm - one.rpm).abs() < 1e-9,
            "fixture: one RPM at every depth"
        );
        let chip = r.feed_rate_mm_min / (r.rpm * 2.0);
        assert!(
            (chip - chip_one).abs() <= 1e-9 * chip_one,
            "L6: at {k} x D the advance per tooth is {chip}; at 1 x D it is {chip_one}"
        );
        assert_eq!(r.chipload_bounds, band_one, "L6: at {k} x D the band moved");
        assert_eq!(
            r.chipload_point_mm, point_one,
            "L6: at {k} x D the printed point moved"
        );
    }
}
