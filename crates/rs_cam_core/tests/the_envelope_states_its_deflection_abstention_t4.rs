//! **T-4 on the production envelope — the envelope states its deflection
//! abstention.**
//!
//! `a_refused_deflection_is_not_a_zero_g_t4` pinned that the deflection
//! back-off (pass 6) said so when it could not run: an unvalidated plastic
//! filed `DeflectionBackoffUnmodeled` with its reason. Feeds contract P5
//! deleted pass 6 (lead ruling, 2026-11-03), and the safe envelope holds the
//! deflection limit now (`feeds::envelope`, the `Deflection` limit). Before
//! this sentry the envelope dropped that limit with no word when the
//! material has no force line, so an absence read as "no deflection limit".
//!
//! The arms, on a roughing pocket with a 6 mm carbide end mill:
//!
//! 1. Acrylic (no force line, ruling B6): the envelope carries a
//!    `Deflection` limit whose edge is `Unmodelled`, and the reason is the
//!    `DeflectionUnmodeled::MaterialUnvalidated` clause. The serialised
//!    envelope (the MCP `get_safe_envelope` reply) carries the same entry.
//! 2. Hard maple (a force line) on a slender end mill (3 mm, flute 12,
//!    stickout 30): the `Deflection` limit is modelled, an `ap(ae)` curve.
//!    This is the non-vacuity partner of arm 1. P11a (D-B): the stubby
//!    6 mm tool of arm 1 reaches no target depth below its flute, so its
//!    entry states "no deflection limit below the flute" (before P11a it
//!    drew the search bracket as a curve).
//!
//! The arms read the envelope through its serialised form, so the sentry
//! compiles on the code before the fix and fails there.

#![expect(
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::PocketConfig;
use rs_cam_core::compute::tool_config::ToolFact;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::feeds::envelope::{EnvelopeInput, SafeEnvelope};
use rs_cam_core::feeds::predict::DeflectionUnmodeled;
use rs_cam_core::feeds::suggest::{NoProject, SuggestContext};
use rs_cam_core::feeds::{SpindleStrategy, embedded_vendor_lut};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, PlasticFamily, WoodSpecies};

/// A stubby carbide end mill: 6 mm, four flutes, 45 mm stickout (the t4
/// fixture).
fn carbide_endmill() -> ToolConfig {
    let mut tool = ToolConfig::test_default(ToolId::from_raw(1), ToolType::EndMill);
    tool.diameter = 6.0;
    tool.set_cutting_length(ToolFact::Stated(25.0));
    tool.shaft_diameter = 6.0;
    tool.set_shank_diameter(ToolFact::Stated(6.0));
    tool.set_stickout(ToolFact::Stated(45.0));
    tool.set_flute_count(ToolFact::Stated(4));
    tool
}

/// A roughing pocket that states its cut (the t4 fixture).
fn roughing_pocket() -> OperationConfig {
    OperationConfig::Pocket(PocketConfig {
        floor: rs_cam_core::compute::floor::Floor::top(12.0),
        depth_per_pass: Some(3.0),
        stepover: Some(2.0),
        // P11b (L7): the envelope is sized at the operator's chip, 4800 /
        // (18 000 x Z); 1800 left the slender tool with no root below its
        // flute.
        feed_rate: Some(4800.0),
        plunge_rate: Some(500.0),
        spindle_rpm: Some(18_000),
        ..PocketConfig::default()
    })
}

/// A slender carbide end mill: 3 mm, two flutes, flute 12, stickout 30.
fn slender_endmill() -> ToolConfig {
    let mut tool = ToolConfig::test_default(ToolId::from_raw(1), ToolType::EndMill);
    tool.diameter = 3.0;
    tool.set_cutting_length(ToolFact::Stated(12.0));
    tool.shaft_diameter = 3.0;
    tool.set_shank_diameter(ToolFact::Stated(3.0));
    tool.set_stickout(ToolFact::Stated(30.0));
    tool.set_flute_count(ToolFact::Stated(2));
    tool
}

/// The serialised `Deflection` limit of the envelope for `material` and
/// `tool`, as the MCP reply carries it.
fn deflection_entry(material: &Material, tool: &ToolConfig) -> Option<serde_json::Value> {
    let op = roughing_pocket();
    let machine = MachineProfile::default();
    let envelope = SafeEnvelope::for_cut(EnvelopeInput {
        operation: &op,
        tool,
        machine: &machine,
        material,
        lut: embedded_vendor_lut(),
        spindle_strategy: SpindleStrategy::MatchChart,
        context: SuggestContext::no_project(NoProject::NONE),
        modulation: rs_cam_core::dressup::feed_modulation::RunModulation::Off,
    })
    .expect("a flat end mill on a roughing pocket has an envelope");
    let json = serde_json::to_value(&envelope).expect("the envelope serialises");
    json.get("boundary")
        .and_then(serde_json::Value::as_array)
        .and_then(|limits| {
            limits
                .iter()
                .find(|l| l.get("kind").and_then(serde_json::Value::as_str) == Some("deflection"))
                .cloned()
        })
}

#[test]
fn the_envelope_states_its_deflection_abstention_t4() {
    let acrylic = Material::Plastic {
        family: PlasticFamily::Acrylic,
    };
    let entry = deflection_entry(&acrylic, &carbide_endmill()).expect(
        "an acrylic roughing envelope must carry a Deflection entry that states the \
         abstention; it dropped the limit with no word",
    );
    let edge = entry.get("edge").expect("the entry has an edge");
    assert_eq!(
        edge.get("kind").and_then(serde_json::Value::as_str),
        Some("unmodelled"),
        "the Deflection entry must be Unmodelled: {entry}"
    );
    assert_eq!(
        edge.get("reason").and_then(serde_json::Value::as_str),
        Some(DeflectionUnmodeled::MaterialUnvalidated.clause()),
        "the abstention must carry its reason: {entry}"
    );
}

#[test]
fn a_modelled_material_keeps_its_deflection_curve_t4() {
    let maple = Material::SolidWood {
        species: WoodSpecies::HardMaple,
    };
    let entry = deflection_entry(&maple, &slender_endmill())
        .expect("hard maple carries a Deflection limit");
    assert_eq!(
        entry
            .get("edge")
            .and_then(|e| e.get("kind"))
            .and_then(serde_json::Value::as_str),
        Some("ap_curve"),
        "a modelled material keeps the ap(ae) curve: {entry}"
    );
}
