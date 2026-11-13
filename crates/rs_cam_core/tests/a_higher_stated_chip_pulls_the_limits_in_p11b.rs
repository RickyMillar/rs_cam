//! Feeds contract P11b, decision L7 (`planning/feeds_contract_2026-11-03/phases/P11.md`):
//! the envelope is sized at the operator's chip (the stated feed / (rpm x
//! Z)), not at the suggested chip. A higher stated chip pulls the force
//! limits in: the deflection edge (the peak engaged-edge force) and the
//! spindle power edge.
//!
//! Operator (2026-11-04): "I was hoping that a high feed would bring those
//! limits in (due to tool breaking)."
//!
//! Fixture: the slender end mill (D 3, Z 2, flute 12, stickout 40, helix
//! 30, the P11a t4 arm 2 shape) on a roughing pocket in white oak at 18 000 rpm. The two envelopes
//! differ only in the stated feed (the second is twice the first). The
//! machine is the default profile with a constant-torque spindle rated 0.1 kW
//! (a test value), so the power edge falls inside the depth span.
//!
//! Arms (red before P11b: both envelopes read the suggested chip, so the
//! edges are equal):
//!
//! 1. Each deflection edge point of the high-chip envelope is at or below
//!    the low-chip point, and one is strictly below.
//! 2. The same for the power edge.

#![expect(
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::PocketConfig;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolFact, ToolId, ToolType};
use rs_cam_core::feeds::envelope::{Edge, EnvelopeInput, LimitKind, SafeEnvelope};
use rs_cam_core::feeds::suggest::{NoProject, SuggestContext};
use rs_cam_core::feeds::{SpindleStrategy, embedded_vendor_lut};
use rs_cam_core::machine::{MachineProfile, PowerModel};
use rs_cam_core::material::{Material, WoodSpecies};

fn end_mill() -> ToolConfig {
    let mut tool = ToolConfig::test_default(ToolId::from_raw(3), ToolType::EndMill);
    tool.diameter = 3.0;
    tool.helix_deg = 30.0;
    tool.set_cutting_length(ToolFact::Stated(12.0));
    tool.shaft_diameter = 3.175;
    tool.set_shank_diameter(ToolFact::Stated(3.175));
    tool.set_stickout(ToolFact::Stated(40.0));
    tool.set_flute_count(ToolFact::Stated(2));
    tool
}

fn pocket(feed: f64) -> OperationConfig {
    OperationConfig::Pocket(PocketConfig {
        floor: rs_cam_core::compute::floor::Floor::top(20.0),
        depth_per_pass: Some(2.0),
        stepover: Some(1.0),
        feed_rate: Some(feed),
        plunge_rate: Some(500.0),
        spindle_rpm: Some(18_000),
        ..PocketConfig::default()
    })
}

fn machine() -> MachineProfile {
    MachineProfile {
        power: PowerModel::VfdConstantTorque {
            rated_power_kw: 0.1,
            rated_rpm: 24_000.0,
        },
        ..MachineProfile::default()
    }
}

fn envelope(feed: f64) -> SafeEnvelope {
    let op = pocket(feed);
    let tool = end_mill();
    let material = Material::SolidWood {
        species: WoodSpecies::WhiteOak,
    };
    let machine = machine();
    SafeEnvelope::for_cut(EnvelopeInput {
        operation: &op,
        tool: &tool,
        machine: &machine,
        material: &material,
        lut: embedded_vendor_lut(),
        spindle_strategy: SpindleStrategy::MatchChart,
        context: SuggestContext::no_project(NoProject::NONE),
        modulation: rs_cam_core::dressup::feed_modulation::RunModulation::Off,
    })
    .expect("a flat end mill on a roughing pocket has an envelope")
}

#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "test: an unexpected variant panics"
)]
fn curve(env: &SafeEnvelope, kind: LimitKind) -> Vec<[f64; 2]> {
    match &env.limit(kind).expect("fixture: the limit is listed").edge {
        Edge::ApCurve { points } => points.clone(),
        other => panic!("fixture: {kind:?} is a curve, got {other:?}"),
    }
}

fn assert_moves_in(kind: LimitKind) {
    let low = curve(&envelope(1800.0), kind);
    let high = curve(&envelope(3600.0), kind);
    assert_eq!(low.len(), high.len(), "one width sample set");
    let mut strictly = 0;
    for (l, h) in low.iter().zip(&high) {
        assert!((l[0] - h[0]).abs() < 1e-12, "one width sample set");
        assert!(
            h[1] <= l[1] + 1e-9,
            "L7: at ae {} the {kind:?} edge of the higher chip is {} mm, deeper than {} mm",
            l[0],
            h[1],
            l[1]
        );
        if h[1] < l[1] - 1e-6 {
            strictly += 1;
        }
    }
    assert!(
        strictly > 0,
        "L7: twice the stated chip moves no {kind:?} edge point in: {low:?} vs {high:?}"
    );
}

#[test]
fn a_higher_stated_chip_pulls_the_deflection_edge_in_p11b() {
    assert_moves_in(LimitKind::Deflection);
}

#[test]
fn a_higher_stated_chip_pulls_the_power_edge_in_p11b() {
    assert_moves_in(LimitKind::Power);
}
