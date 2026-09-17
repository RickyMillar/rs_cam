//! Tests for candidate evaluation: the LUT lookup context and the
//! engaged-diameter inputs one candidate is scored with.
//!
//! Moved out of `tool_load/optimize/mod.rs` by P4; the module body is
//! unchanged.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use super::context::{diameter_for_lut_lookup, lut_op_family_from, lut_pass_role_from};
use super::*;
use crate::compute::operation_configs::PocketConfig;
use crate::feeds::vendor_lut::{LutOperationFamily, LutPassRole};
use crate::feeds::{OperationFamily, PassRole};
use crate::tool::MillingCutter;

fn baseline_op() -> OperationConfig {
    OperationConfig::Pocket(PocketConfig {
        feed_rate: 1500.0,
        stepover: 2.0,
        depth_per_pass: 1.5,
        spindle_rpm: Some(18_000),
        ..PocketConfig::default()
    })
}

#[test]
fn delta_against_baseline_detects_only_changed_fields() {
    let base = baseline_op();
    let mut candidate = base.clone();
    candidate.set_feed_rate(2100.0);
    let delta = delta_against_baseline(&base, &candidate);
    assert_eq!(delta.feed_mm_min, Some(2100.0));
    assert!(delta.spindle_rpm.is_none());
    assert!(delta.stepover_mm.is_none());
    assert!(delta.depth_per_pass_mm.is_none());
}

#[test]
fn delta_against_baseline_picks_up_doc_change() {
    let base = baseline_op();
    let mut candidate = base.clone();
    candidate.set_depth_per_pass(2.5);
    candidate.set_stepover(2.4);
    let delta = delta_against_baseline(&base, &candidate);
    assert_eq!(delta.depth_per_pass_mm, Some(2.5));
    assert_eq!(delta.stepover_mm, Some(2.4));
    // Feed and rpm unchanged.
    assert!(delta.feed_mm_min.is_none());
    assert!(delta.spindle_rpm.is_none());
}

#[test]
fn delta_against_baseline_ignores_subhalf_feed_drift() {
    // Tiny floating-point drift (under 0.5 mm/min) should NOT be
    // reported as a change. The 0.5-mm/min floor matches typical
    // CNC controller granularity.
    let base = baseline_op();
    let mut candidate = base.clone();
    candidate.set_feed_rate(1500.000001);
    let delta = delta_against_baseline(&base, &candidate);
    assert!(delta.feed_mm_min.is_none(), "got {delta:?}");
}

#[test]
fn lut_op_family_mapping_covers_all_variants() {
    // Cover every variant so the match arms can't drift apart from
    // the suggest module's mapping.
    assert_eq!(
        lut_op_family_from(OperationFamily::Adaptive),
        LutOperationFamily::Adaptive
    );
    assert_eq!(
        lut_op_family_from(OperationFamily::Pocket),
        LutOperationFamily::Pocket
    );
    assert_eq!(
        lut_op_family_from(OperationFamily::Contour),
        LutOperationFamily::Contour
    );
    assert_eq!(
        lut_op_family_from(OperationFamily::Parallel),
        LutOperationFamily::Parallel
    );
    assert_eq!(
        lut_op_family_from(OperationFamily::Scallop),
        LutOperationFamily::Scallop
    );
    assert_eq!(
        lut_op_family_from(OperationFamily::Trace),
        LutOperationFamily::Trace
    );
    assert_eq!(
        lut_op_family_from(OperationFamily::Face),
        LutOperationFamily::Face
    );
}

#[test]
fn lut_pass_role_mapping_covers_all_variants() {
    assert_eq!(
        lut_pass_role_from(PassRole::Roughing),
        LutPassRole::Roughing
    );
    assert_eq!(
        lut_pass_role_from(PassRole::SemiFinish),
        LutPassRole::SemiFinish
    );
    assert_eq!(lut_pass_role_from(PassRole::Finish), LutPassRole::Finish);
}

/// Build a `ToolDefinition` wrapping any `MillingCutter` for testing.
/// Stickout / shank / holder values are arbitrary — the LUT-lookup
/// helper only reads the cutter's diameter / engaged-diameter.
fn wrap_cutter(cutter: Box<dyn crate::tool::MillingCutter>) -> crate::tool::ToolDefinition {
    crate::tool::ToolDefinition::new(
        cutter,
        6.35, // shank
        20.0, // shank length
        40.0, // holder diameter
        60.0, // stickout
        2,    // flutes
        crate::compute::tool_config::ToolMaterial::Carbide,
    )
}

#[test]
fn diameter_for_lut_lookup_tapered_ball_uses_engaged_at_doc() {
    // Tapered ball nose: ball_dia 2 mm, half-angle 10°, shaft 8 mm,
    // cutting length 30 mm. At a shallow DOC of 0.1 mm we're in
    // the ball region — engaged diameter is much less than the
    // 8 mm shaft.
    let cutter = Box::new(crate::tool::TaperedBallEndmill::new(2.0, 10.0, 8.0, 30.0));
    let tool = wrap_cutter(cutter);

    // Sanity: nominal "diameter" is the shaft (max) — the bug we're
    // fixing is the optimizer matching LUT rows against this value.
    assert!((tool.diameter() - 8.0).abs() < 1e-9);

    let engaged = diameter_for_lut_lookup(&tool, Some(0.1));
    assert!(
        engaged < 1.0,
        "expected engaged diameter <1 mm at DOC=0.1 mm on a 2 mm \
         ball-tip taper, got {engaged}"
    );
    assert!(
        engaged > 0.0,
        "engaged diameter should be positive at DOC>0"
    );
}

#[test]
fn diameter_for_lut_lookup_endmill_returns_nominal_diameter() {
    // Cylindrical endmill: lookup_diameter_at returns nominal at
    // any DOC. The helper must preserve that — non-tapered tools
    // are unaffected by this change.
    let cutter = Box::new(crate::tool::FlatEndmill::new(6.0, 25.0));
    let tool = wrap_cutter(cutter);
    assert!((diameter_for_lut_lookup(&tool, Some(3.0)) - 6.0).abs() < 1e-9);
    assert!((diameter_for_lut_lookup(&tool, Some(0.05)) - 6.0).abs() < 1e-9);
}

#[test]
fn diameter_for_lut_lookup_falls_back_to_nominal_when_doc_missing() {
    // Drilling, V-carve, scallop don't carry depth_per_pass; the
    // helper should still produce a usable diameter rather than
    // refusing the lookup.
    let cutter = Box::new(crate::tool::TaperedBallEndmill::new(2.0, 10.0, 8.0, 30.0));
    let tool = wrap_cutter(cutter);

    assert!((diameter_for_lut_lookup(&tool, None) - 8.0).abs() < 1e-9);
    // Defensive: zero / negative DOC also falls back.
    assert!((diameter_for_lut_lookup(&tool, Some(0.0)) - 8.0).abs() < 1e-9);
    assert!((diameter_for_lut_lookup(&tool, Some(-1.0)) - 8.0).abs() < 1e-9);
    // NaN / infinite likewise.
    assert!((diameter_for_lut_lookup(&tool, Some(f64::NAN)) - 8.0).abs() < 1e-9);
    assert!((diameter_for_lut_lookup(&tool, Some(f64::INFINITY)) - 8.0).abs() < 1e-9);
}
