//! K7 change 3 (feeds contract P9): pre-sim power evaluates a set of cases
//! (nominal, slot, planner cap) that the op's `CutSpec` admits and ships
//! the maximum with its case name.
//!
//! Before P9 Suggest read one point, the nominal width: a Pocket's first
//! pass (a slot, `FirstPass::Slot`) and a 3D Rough's engagement excursion
//! (report 01: 0.53 kW pre-sim against 1.18 kW in the simulation) had no
//! pre-sim power.
//!
//! Each expected value comes from the door under test: the power of a slot
//! is the power of a Face (no slot case, no cap) that states the slot
//! width; the power of the planner cap is a Face at the capped width.

#![expect(
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::{Adaptive3dConfig, FaceConfig, PocketConfig};
use rs_cam_core::compute::tool_config::ToolFact;
use rs_cam_core::compute::{ToolConfig, ToolId, ToolType};
use rs_cam_core::feeds::engagement::PowerCase;
use rs_cam_core::feeds::suggest::feeds_result_for_operation;
use rs_cam_core::feeds::{SpindleStrategy, embedded_vendor_lut, power_at_operating_point};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, WoodSpecies};

const FEED_MM_MIN: f64 = 2000.0;
const RPM: u32 = 16_000;

fn oak() -> Material {
    Material::SolidWood {
        species: WoodSpecies::WhiteOak,
    }
}

fn flat(id: usize, diameter: f64) -> ToolConfig {
    let mut t = ToolConfig::test_default(ToolId::from_raw(id), ToolType::EndMill);
    t.diameter = diameter;
    t.set_flute_count(ToolFact::Stated(2));
    t
}

/// The reference: a Face states its width and has no slot and no cap.
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

fn same(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-12 * a.abs().max(1.0)
}

#[test]
fn a_pocket_ships_its_first_pass_slot() {
    let machine = MachineProfile::generic_wood_router();
    let material = oak();
    let tool = flat(1, 6.0);
    let (ap, ae) = (2.0, 2.0);
    let pocket = OperationConfig::Pocket(PocketConfig {
        stepover: ae,
        depth_per_pass: ap,
        feed_rate: Some(FEED_MM_MIN),
        plunge_rate: Some(300.0),
        spindle_rpm: Some(RPM),
        ..PocketConfig::default()
    });
    let figure = power_at_operating_point(&pocket, &tool, &material, &machine, None)
        .expect("the pocket is modelled");
    assert_eq!(figure.case, PowerCase::Slot, "{figure:?}");
    assert!(same(figure.case_ae_mm, tool.diameter), "{figure:?}");
    let slot = power_at_operating_point(&face(ap, tool.diameter), &tool, &material, &machine, None)
        .expect("the face is modelled");
    assert_eq!(slot.case, PowerCase::Nominal, "a face has no slot case");
    assert!(
        same(figure.required_kw, slot.required_kw),
        "the slot case is the power of a slot: {} vs {}",
        figure.required_kw,
        slot.required_kw
    );
    let nominal = power_at_operating_point(&face(ap, ae), &tool, &material, &machine, None)
        .expect("the face is modelled");
    assert!(
        figure.required_kw > nominal.required_kw,
        "non-vacuity: the slot draws more than the stated width"
    );

    // The calculator names the same case.
    let result = feeds_result_for_operation(
        &pocket,
        &tool,
        &material,
        &machine,
        embedded_vendor_lut(),
        SpindleStrategy::MatchChart,
    )
    .expect("a runnable pairing");
    assert_eq!(result.power_case, PowerCase::Slot);
}

#[test]
fn a_profile_states_its_slot_and_has_a_power() {
    let machine = MachineProfile::generic_wood_router();
    let material = oak();
    let tool = flat(1, 6.0);
    let ap = 2.0;
    let profile =
        OperationConfig::Profile(rs_cam_core::compute::operation_configs::ProfileConfig {
            depth_per_pass: ap,
            feed_rate: Some(FEED_MM_MIN),
            plunge_rate: Some(300.0),
            spindle_rpm: Some(RPM),
            ..Default::default()
        });
    // Before P9 a Profile (no stepover) refused with NoRadialEngagement.
    let figure = power_at_operating_point(&profile, &tool, &material, &machine, None)
        .expect("a Radial::Slot op states its width");
    assert_eq!(figure.case, PowerCase::Slot, "{figure:?}");
    let slot = power_at_operating_point(&face(ap, tool.diameter), &tool, &material, &machine, None)
        .expect("the face is modelled");
    assert!(same(figure.required_kw, slot.required_kw));
}

#[test]
fn an_op_one_rough_ships_the_planner_cap() {
    let machine = MachineProfile::generic_wood_router();
    let material = oak();
    // Report 01 op 1: an 18 mm 2F flat end mill.
    let tool = flat(2, 18.0);
    let defaults = Adaptive3dConfig::default();
    let rough = OperationConfig::Adaptive3d(Adaptive3dConfig {
        feed_rate: Some(FEED_MM_MIN),
        plunge_rate: Some(300.0),
        spindle_rpm: Some(RPM),
        ..defaults
    });
    let (ap, ae, mult) = (
        defaults.depth_per_pass,
        defaults.stepover,
        defaults.trochoid_cap_mult,
    );
    assert!(mult > 1.0, "non-vacuity: the cap widens the cut");
    let figure = power_at_operating_point(&rough, &tool, &material, &machine, None)
        .expect("the rough is modelled");
    assert_eq!(figure.case, PowerCase::PlannerCap, "{figure:?}");
    let capped = (mult * ae).min(tool.diameter);
    assert!(same(figure.case_ae_mm, capped), "{figure:?}");
    let reference = power_at_operating_point(&face(ap, capped), &tool, &material, &machine, None)
        .expect("the face is modelled");
    assert!(
        same(figure.required_kw, reference.required_kw),
        "the cap case is the power at the capped width: {} vs {}",
        figure.required_kw,
        reference.required_kw
    );
}
