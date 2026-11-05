//! Feeds contract P11b, decision L6 (`planning/feeds_contract_2026-11-03/phases/P11.md`):
//! the vendor depth table (Onsrud, Freud, Amana: 1 x D full chip, 2 x D
//! less 25 %, 3 x D less 50 %) is a drawn limit of the envelope, "vendor
//! depth ceiling (width not printed)", and an operator point past it is
//! outside the envelope (the K6 warning `project.outside_envelope`, ruling
//! Q6: warn, never refuse). The table no longer scales the chip (sentry
//! `the_depth_table_no_longer_moves_the_chip_p11b`).
//!
//! The ceiling is read at the operator's PEAK chip: the deepest `ap` where
//! `band max x doc_derating_scale(ap / D)` still holds it. At half the
//! diameter the peak chip is the advance per tooth.
//!
//! Fixture: a 6 mm end mill (flute 25 mm) on a roughing pocket in white oak,
//! `ae = 0.5 D`, 18 000 rpm stated.
//!
//! Arms (red before P11b: the limit kind does not exist):
//!
//! 1. At a stated chip equal to the band maximum, the limit at a width of
//!    half the diameter or more is the 1 x D ceiling, and the operator's
//!    2 x D point is outside it and outside the envelope.
//! 2. At half the band maximum the table binds no width (the chip meets it
//!    at 3 x D, the last printed point), so no table limit is listed; at
//!    three quarters of the band maximum the ceiling is 2 x D.

#![expect(
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::PocketConfig;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolFact, ToolId, ToolType};
use rs_cam_core::feeds::envelope::{
    CutPoint, Edge, EnvelopeInput, LimitKind, Placement, SafeEnvelope,
};
use rs_cam_core::feeds::suggest::{SuggestContext, feeds_input_for_operation};
use rs_cam_core::feeds::{SpindleStrategy, calculate, embedded_vendor_lut};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, WoodSpecies};

const D_MM: f64 = 6.0;
const RPM: u32 = 18_000;
const AP_MM: f64 = 2.0 * D_MM;

fn end_mill() -> ToolConfig {
    let mut tool = ToolConfig::test_default(ToolId::from_raw(3), ToolType::EndMill);
    tool.diameter = D_MM;
    tool.set_cutting_length(ToolFact::Stated(25.0));
    tool.set_stickout(ToolFact::Stated(45.0));
    tool.set_flute_count(ToolFact::Stated(2));
    tool
}

fn oak() -> Material {
    Material::SolidWood {
        species: WoodSpecies::WhiteOak,
    }
}

fn pocket(feed: f64) -> OperationConfig {
    OperationConfig::Pocket(PocketConfig {
        floor: rs_cam_core::compute::floor::Floor::top(30.0),
        depth_per_pass: Some(AP_MM),
        stepover: Some(0.5 * D_MM),
        feed_rate: Some(feed),
        plunge_rate: Some(500.0),
        spindle_rpm: Some(RPM),
        ..PocketConfig::default()
    })
}

/// The printed band maximum (or the printed point) of the matched row.
fn band_max() -> f64 {
    let tool = end_mill();
    let material = oak();
    let machine = MachineProfile::default();
    let op = pocket(1000.0);
    let r = calculate(&feeds_input_for_operation(
        &op,
        &tool,
        &material,
        &machine,
        embedded_vendor_lut(),
        SpindleStrategy::MatchChart,
    ));
    r.chipload_bounds
        .map(|b| b.max_mm_per_tooth)
        .or(r.chipload_point_mm)
        .expect("fixture: the row prints a band or a point")
}

fn envelope_at_chip(fz: f64) -> SafeEnvelope {
    let op = pocket(fz * f64::from(RPM) * 2.0);
    let tool = end_mill();
    let material = oak();
    let machine = MachineProfile::default();
    SafeEnvelope::for_cut(EnvelopeInput {
        operation: &op,
        tool: &tool,
        machine: &machine,
        material: &material,
        lut: embedded_vendor_lut(),
        spindle_strategy: SpindleStrategy::MatchChart,
        context: SuggestContext::default(),
    })
    .expect("a flat end mill on a roughing pocket has an envelope")
}

/// The ceiling at the full width (the last width sample): there the peak
/// chip is the advance per tooth, and the value is a sample, not an
/// interpolation.
#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "test: an unexpected variant panics"
)]
fn ceiling_at_full_width(env: &SafeEnvelope) -> f64 {
    let l = env
        .limit(LimitKind::VendorDepthTable)
        .expect("L6: the envelope lists the vendor depth table");
    assert_eq!(l.kind.label(), "vendor depth ceiling (width not printed)");
    match &l.edge {
        Edge::ApCeiling { .. } | Edge::ApCurve { .. } => {}
        other => panic!("L6: the depth table is a depth ceiling, got {other:?}"),
    }
    l.ap_ceiling_at(D_MM).expect("a ceiling at the width")
}

#[test]
fn a_point_past_the_depth_table_is_outside_the_envelope_p11b() {
    let max = band_max();
    let env = envelope_at_chip(max);
    let ceiling = ceiling_at_full_width(&env);
    assert!(
        (ceiling - D_MM).abs() < 1e-6,
        "L6: at the band maximum the table allows 1 x D = {D_MM} mm; the ceiling is {ceiling}"
    );
    let p = CutPoint {
        ap_mm: AP_MM,
        ae_mm: 0.5 * D_MM,
    };
    let margin = env
        .limit(LimitKind::VendorDepthTable)
        .and_then(|l| l.margin(p))
        .expect("a margin at the point");
    assert!(
        margin < 0.0,
        "L6: the 2 x D point is past the ceiling: {margin}"
    );
    assert!(
        matches!(env.operator, Some(Placement::Outside { .. })),
        "Q6: the operator point is outside the envelope (it warns): {:?}",
        env.operator
    );
}

#[test]
fn half_the_band_chip_meets_the_table_at_every_printed_depth_p11b() {
    // At half the band maximum the chip is inside the table at 3 x D, the
    // last printed point; deeper, the vendors print nothing and the static
    // check `feeds.depth_beyond_published_table` names it. So the table
    // binds no column and the envelope lists no table limit.
    let env = envelope_at_chip(0.5 * band_max());
    assert!(
        env.limit(LimitKind::VendorDepthTable).is_none(),
        "L6: half the band chip meets the table: {:?}",
        env.limit(LimitKind::VendorDepthTable)
    );
    // The partner: three quarters of the band maximum binds at 2 x D.
    let env = envelope_at_chip(0.75 * band_max());
    let ceiling = ceiling_at_full_width(&env);
    assert!(
        (ceiling - 2.0 * D_MM).abs() < 1e-6,
        "L6: at 0.75 of the band maximum the table allows 2 x D; the ceiling is {ceiling}"
    );
}
