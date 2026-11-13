//! Feeds contract P11a, decision L4, defect D-A
//! (`planning/feeds_contract_2026-11-03/research_ae/FINDINGS.md`): the
//! speeds grid of the safe envelope sizes power at the REQUESTED depth.
//!
//! Before P11a each grid point stored the requested `ap` and the power of
//! the `ap` that the calculator moved: the slot cap (`ap <= 0.25 D` where
//! `ae > 0.85 D`) and the flute guard (`0.8 x flute`). Above the slot cap
//! the power was flat in `ap`, so the power edge could not cross there.
//! The slot cap and the flute guard are limits drawn on the envelope; they
//! do not clamp the power input. P16 (D12) deleted the slot cap; arm 1
//! now reads the whole slot column.
//!
//! The fixture is the rivmap100 end mill
//! (`planning/fixtures/rivmap100/rivmap100.toml`, tool 3: D 6, Z 2, flute
//! 25, stickout 45, helix 30) on a roughing pocket in white oak.
//!
//! The arms:
//!
//! 1. In the slot column (`ae = D`), the grid points hold more than one
//!    power value (red before: one value above the old slot cap).
//! 2. In the narrowest column, the grid points above the flute guard hold
//!    more than one power value (red before: one value).
//! 3. Each grid point holds the power that `power_at_operating_point`
//!    gives for an op that states the point's depth, width, feed and RPM.

#![expect(
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::PocketConfig;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolFact, ToolId, ToolType};
use rs_cam_core::feeds::envelope::{EnvelopeInput, SafeEnvelope, SpeedsPoint};
use rs_cam_core::feeds::suggest::{NoProject, SuggestContext};
use rs_cam_core::feeds::{SpindleStrategy, embedded_vendor_lut, power_at_operating_point};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, WoodSpecies};

const D_MM: f64 = 6.0;
const FLUTE_MM: f64 = 25.0;

fn rivmap_end_mill() -> ToolConfig {
    let mut tool = ToolConfig::test_default(ToolId::from_raw(3), ToolType::EndMill);
    tool.diameter = D_MM;
    tool.helix_deg = 30.0;
    tool.set_cutting_length(ToolFact::Stated(FLUTE_MM));
    tool.shaft_diameter = 6.35;
    tool.set_shank_diameter(ToolFact::Stated(6.35));
    tool.set_stickout(ToolFact::Stated(45.0));
    tool.set_flute_count(ToolFact::Stated(2));
    tool
}

fn roughing_pocket() -> OperationConfig {
    OperationConfig::Pocket(PocketConfig {
        floor: rs_cam_core::compute::floor::Floor::top(12.0),
        depth_per_pass: Some(3.0),
        stepover: Some(2.0),
        feed_rate: Some(1800.0),
        plunge_rate: Some(500.0),
        spindle_rpm: Some(18_000),
        ..PocketConfig::default()
    })
}

fn oak() -> Material {
    Material::SolidWood {
        species: WoodSpecies::WhiteOak,
    }
}

/// A machine whose spindle never limits the fixture, so the Step 6 power
/// ladder does not move the feed or the RPM and the grid power is the
/// model's power at each depth. A test value, not a machine claim.
fn ample_machine() -> MachineProfile {
    MachineProfile {
        power: rs_cam_core::machine::PowerModel::ConstantPower { power_kw: 50.0 },
        ..MachineProfile::default()
    }
}

fn envelope(machine: &MachineProfile) -> SafeEnvelope {
    let op = roughing_pocket();
    let tool = rivmap_end_mill();
    let material = oak();
    SafeEnvelope::for_cut(EnvelopeInput {
        operation: &op,
        tool: &tool,
        machine,
        material: &material,
        lut: embedded_vendor_lut(),
        spindle_strategy: SpindleStrategy::MatchChart,
        context: SuggestContext::no_project(NoProject::NONE),
        modulation: rs_cam_core::dressup::feed_modulation::RunModulation::Off,
    })
    .expect("a flat end mill on a roughing pocket has an envelope")
}

/// The grid points of column `j` whose depth is above `ap_mm`.
fn column_above(env: &SafeEnvelope, j: usize, ap_mm: f64) -> Vec<&SpeedsPoint> {
    (0..env.speeds.aps.len())
        .filter_map(|i| env.speeds.at(i, j))
        .filter(|p| p.ap_mm > ap_mm)
        .collect()
}

/// The number of distinct power values (relative step above 1e-9).
fn distinct_powers(points: &[&SpeedsPoint]) -> usize {
    let mut kw: Vec<f64> = points.iter().filter_map(|p| p.power_kw).collect();
    kw.sort_by(f64::total_cmp);
    kw.dedup_by(|a, b| (*a - *b).abs() <= 1e-9 * b.abs().max(1e-12));
    kw.len()
}

#[test]
fn the_slot_column_sizes_power_at_each_depth_p11a() {
    let env = envelope(&ample_machine());
    let j = env.speeds.aes.len() - 1;
    let ae = env.speeds.aes.get(j).copied().expect("a slot column");
    assert!(
        (ae - D_MM).abs() < 1e-9,
        "the last width sample is the slot: {ae}"
    );
    // P16 (D12): no slot cap; every depth of the column is a cut.
    let points = column_above(&env, j, 0.0);
    assert!(
        points.len() >= 3,
        "the fixture has grid points in the slot column: {}",
        points.len()
    );
    assert!(
        distinct_powers(&points) > 1,
        "D-A: the slot column stores one power for every depth: {:?}",
        points
            .iter()
            .map(|p| (p.ap_mm, p.power_kw))
            .collect::<Vec<_>>()
    );
}

#[test]
fn the_narrow_column_sizes_power_above_the_flute_guard_p11a() {
    let env = envelope(&ample_machine());
    // The flute guard is `0.8 x flute` (`feeds::flute_guard_of`).
    let guard = 0.8 * FLUTE_MM;
    let points = column_above(&env, 0, guard);
    assert!(
        points.len() >= 2,
        "the fixture has grid points above the flute guard: {}",
        points.len()
    );
    assert!(
        distinct_powers(&points) > 1,
        "D-A: the narrow column stores one power above the flute guard: {:?}",
        points
            .iter()
            .map(|p| (p.ap_mm, p.power_kw))
            .collect::<Vec<_>>()
    );
}

#[test]
fn each_grid_point_stores_the_power_of_its_own_depth_p11a() {
    let machine = ample_machine();
    let env = envelope(&machine);
    let tool = rivmap_end_mill();
    let material = oak();
    let mut checked = 0;
    for p in &env.speeds.points {
        let rpm = p.rpm.get().0;
        assert!(
            (rpm - rpm.round()).abs() < 1e-9 && rpm > 0.0,
            "the fixture ships a whole RPM: {rpm}"
        );
        let mut op = roughing_pocket();
        assert!(op.set_depth_per_pass(p.ap_mm));
        assert!(op.set_stepover(p.ae_mm));
        op.set_feed_rate(p.feed.get().0);
        // The RPM is a positive whole number (asserted above).
        op.set_spindle_rpm(Some(rpm.round() as u32));
        let want = power_at_operating_point(&op, &tool, &material, &machine, None)
            .expect("white oak has a power figure")
            .required_kw;
        let stored = p.power_kw.expect("white oak has a grid power");
        assert!(
            (stored - want).abs() <= 1e-9 * want.abs().max(1e-12),
            "D-A: the grid point at ap {} ae {} stores {} kW; its own depth draws {} kW",
            p.ap_mm,
            p.ae_mm,
            stored,
            want
        );
        checked += 1;
    }
    assert!(checked > 0, "the grid has points");
}
