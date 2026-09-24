//! Tests for the stage-1 grid search: the knob variants each
//! operation exposes and the candidate space they build.
//!
//! Moved out of `tool_load/optimize/mod.rs` by P4; the module body is
//! unchanged.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use super::candidate::{
    build_doc_variants, build_scallop_height_variants, build_stepover_variants, has_doc_knob,
};
use super::*;
use crate::compute::catalog::OperationParams;
use crate::compute::operation_configs::{
    PencilConfig, ProfileConfig, RampFinishConfig, ScallopConfig, TraceConfig, WaterlineConfig,
    ZigzagConfig,
};

#[test]
fn has_doc_knob_includes_g3_ops() {
    // G3 (2026-05-08) — Trace was always exposed via the trait but
    // missing from `has_doc_knob`; RampFinish and Waterline now
    // wrap their semantically-equivalent Z-step knobs.
    assert!(has_doc_knob(OperationType::Trace));
    assert!(has_doc_knob(OperationType::RampFinish));
    assert!(has_doc_knob(OperationType::Waterline));
    // RadialFinish stays out — its `angular_step` is degrees,
    // structurally different from the existing mm-based axes.
    assert!(!has_doc_knob(OperationType::RadialFinish));
}

#[test]
fn ramp_finish_depth_per_pass_wraps_max_stepdown() {
    let mut r = RampFinishConfig::default();
    let initial = r
        .depth_per_pass()
        .expect("RampFinish should expose depth_per_pass after G3");
    assert!((initial - r.max_stepdown).abs() < 1e-9);
    // Set-through writes max_stepdown so existing planner code
    // that consumes max_stepdown picks up the new value.
    r.set_depth_per_pass(0.8);
    assert!((r.max_stepdown - 0.8).abs() < 1e-9);
    assert!((r.depth_per_pass().expect("present") - 0.8).abs() < 1e-9);
}

#[test]
fn waterline_depth_per_pass_wraps_z_step() {
    let mut w = WaterlineConfig::default();
    let initial = w
        .depth_per_pass()
        .expect("Waterline should expose depth_per_pass after G3");
    assert!((initial - w.z_step).abs() < 1e-9);
    w.set_depth_per_pass(2.0);
    assert!((w.z_step - 2.0).abs() < 1e-9);
}

#[test]
fn pencil_stepover_only_when_multipass() {
    // Default `num_offset_passes = 1` → no stepover knob exposed.
    let single = PencilConfig {
        num_offset_passes: 1,
        ..PencilConfig::default()
    };
    assert!(single.stepover().is_none());
    // Multipass → exposes offset_stepover.
    let mut multi = PencilConfig {
        num_offset_passes: 3,
        ..PencilConfig::default()
    };
    let s = multi.stepover().expect("stepover exposed when multipass");
    assert!((s - multi.offset_stepover).abs() < 1e-9);
    multi.set_stepover(0.75);
    assert!((multi.offset_stepover - 0.75).abs() < 1e-9);
}

#[test]
fn trace_config_exposes_depth_per_pass() {
    // Trace's accessor already existed pre-G3 — this test pins it
    // alongside the `has_doc_knob` membership change so a future
    // refactor can't silently drop Trace from Stage 1 again.
    let t = TraceConfig::default();
    assert!(t.depth_per_pass().is_some());
    assert!(
        t.stepover().is_none(),
        "Trace is a path-follow, no stepover"
    );
}

#[test]
fn scallop_config_exposes_scallop_height_not_stepover() {
    // G2 (2026-05-08): ScallopConfig has no `stepover` field —
    // its spacing knob is `scallop_height` (default 0.1 mm).
    // Stage 1 sweeps this as a third axis distinct from
    // `stepover` because units differ (a 0.1 mm scallop on a
    // 6 mm ball produces ~1.55 mm radial step).
    let s = ScallopConfig::default();
    assert!(
        s.scallop_height().is_some(),
        "Scallop should expose scallop_height"
    );
    assert!(s.stepover().is_none(), "Scallop should NOT expose stepover");
    assert!(
        s.depth_per_pass().is_none(),
        "Scallop should NOT expose depth_per_pass — it's surface-following"
    );
    assert!(
        (s.scallop_height().expect("scallop_height present") - 0.1).abs() < 1e-9,
        "expected default 0.1 mm scallop height"
    );
}

#[test]
fn set_scallop_height_writes_through() {
    let mut s = ScallopConfig::default();
    s.set_scallop_height(0.05);
    assert!((s.scallop_height().expect("scallop_height present") - 0.05).abs() < 1e-9);
}

#[test]
fn build_scallop_height_variants_three_around_baseline() {
    // 0.10 mm baseline → [0.07, 0.10, 0.13].
    let variants = build_scallop_height_variants(0.10);
    assert_eq!(variants.len(), 3, "got {variants:?}");
    assert!((variants[0] - 0.07).abs() < 1e-6);
    assert!((variants[1] - 0.10).abs() < 1e-6);
    assert!((variants[2] - 0.13).abs() < 1e-6);
}

#[test]
fn build_scallop_height_variants_floored_at_minimum() {
    // 0.005 mm baseline (below the 0.01 floor) → all variants
    // collapse to the floor.
    let variants = build_scallop_height_variants(0.005);
    for v in &variants {
        assert!(
            *v >= search_policy().axes.scallop_height.hard_floor.value - 1e-9,
            "got {variants:?}"
        );
    }
}

#[test]
fn has_doc_knob_includes_profile_and_zigzag() {
    // G1 (2026-05-08): Profile and Zigzag both expose
    // `depth_per_pass()` via the trait but were excluded from the
    // original 5-op list, so Stage 1 silently skipped them.
    assert!(has_doc_knob(OperationType::Profile));
    assert!(has_doc_knob(OperationType::Zigzag));
    // Sanity: the original 5 stay in.
    assert!(has_doc_knob(OperationType::Pocket));
    assert!(has_doc_knob(OperationType::Adaptive));
    assert!(has_doc_knob(OperationType::Adaptive3d));
    assert!(has_doc_knob(OperationType::Rest));
    assert!(has_doc_knob(OperationType::Face));
    // Sanity: ops that genuinely lack a DOC knob stay out.
    assert!(!has_doc_knob(OperationType::Drill));
    assert!(!has_doc_knob(OperationType::DropCutter));
    assert!(!has_doc_knob(OperationType::Scallop));
}

#[test]
fn profile_config_exposes_doc_but_not_stepover() {
    // `AxisGridStrategy` collapses the stepover dimension for ops
    // without a stepover knob — Profile is a contour follow.
    let p = ProfileConfig::default();
    assert!(p.depth_per_pass().is_some(), "Profile should expose DOC");
    assert!(p.stepover().is_none(), "Profile should NOT expose stepover");
}

#[test]
fn zigzag_config_exposes_both_doc_and_stepover() {
    // Zigzag is a full DOC × stepover grid op.
    let z = ZigzagConfig::default();
    assert!(z.depth_per_pass().is_some(), "Zigzag should expose DOC");
    assert!(z.stepover().is_some(), "Zigzag should expose stepover");
}

fn synthetic_lut_row(ap_min: Option<f64>, ap_max: Option<f64>) -> MatchedRow {
    MatchedRow {
        chip_load_mm: 0.04,
        chip_load_min_mm: Some(0.025),
        chip_load_max_mm: Some(0.055),
        rpm_nominal: Some(15_000.0),
        rpm_min: Some(14_000.0),
        rpm_max: Some(16_000.0),
        ap_min_mm: ap_min,
        ap_max_mm: ap_max,
        ap_min_factor: None,
        ap_max_factor: None,
        ae_min_mm: None,
        ae_max_mm: None,
        observation_id: "synthetic".to_owned(),
        source_vendor: crate::feeds::vendor_lut::Vendor::Amana,
        score: 100,
        diameter_match_score: 200,
        row_diameter_mm: 6.0,
        chipload_diameter_scale: 1.0,
        chipload_hardness_scale: 1.0,
        chipload_diameter_ratio_raw: 1.0,
        chipload_hardness_ratio_raw: 1.0,
        is_extrapolated: false,
        row_pass_role: crate::feeds::vendor_lut::LutPassRole::Roughing,
        size_basis: crate::feeds::extrapolation::SizeBasis::Exact,
        hardness_basis: crate::feeds::extrapolation::HardnessBasis::Unscaled,
        family_basis: crate::feeds::extrapolation::FamilyBasis::Printed,
        drill_basis: crate::feeds::extrapolation::DrillBasis::Printed,
        material_label: String::new(),
        evidence_grade: crate::feeds::vendor_lut::EvidenceGrade::A,
        row_kind: crate::feeds::vendor_lut::ObservationKind::Exact,
    }
}

#[test]
fn three_variant_op_with_no_lut_row() {
    // Adaptive3d with baseline 3.0mm and no LUT bounds.
    // Expected: [0.7×3.0, 3.0, 1.3×3.0] = [2.1, 3.0, 3.9]
    let variants = build_doc_variants(3.0, None, OperationType::Adaptive3d);
    assert_eq!(variants.len(), 3, "got {variants:?}");
    assert!((variants[0] - 2.1).abs() < 1e-6);
    assert!((variants[1] - 3.0).abs() < 1e-6);
    assert!((variants[2] - 3.9).abs() < 1e-6);
}

#[test]
fn four_variant_op_with_no_lut_row() {
    // Pocket with baseline 1.5mm and no LUT bounds.
    // Expected: [0.7×1.5, 1.5, mid(1.5, 2.1), 2.1] = [1.05, 1.5, 1.8, 2.1]
    let variants = build_doc_variants(1.5, None, OperationType::Pocket);
    assert_eq!(variants.len(), 4, "got {variants:?}");
    assert!((variants[0] - 1.05).abs() < 1e-6);
    assert!((variants[1] - 1.5).abs() < 1e-6);
    assert!((variants[2] - 1.8).abs() < 1e-6);
    assert!((variants[3] - 2.1).abs() < 1e-6);
}

#[test]
fn lut_row_does_not_clamp_warm_start_when_baseline_inside_preferred() {
    // Baseline 3.0mm, LUT ap_min = 2.5mm (above 0.7×3.0 = 2.1).
    // Old behaviour: lo clamped to ap_min = 2.5.
    // G16 Step 4: LUT becomes preferred, not hard. Warm-start uses
    // baseline × mults so lo = 2.1; an outside-preferred probe at
    // ap_min × 0.85 = 2.125 sits between baseline-mult-lo and ap_min.
    let row = synthetic_lut_row(Some(2.5), Some(5.0));
    let variants = build_doc_variants(3.0, Some(&row), OperationType::Adaptive3d);
    // The smallest variant should be the multiplier-anchored 2.1, not the LUT lo.
    assert!(
        variants[0] < 2.5,
        "smallest variant should sit below LUT ap_min (warm-start unclamped): got {variants:?}"
    );
    // Baseline survives.
    assert!(variants.iter().any(|v| (v - 3.0).abs() < 1e-6));
}

#[test]
fn lut_row_does_not_clamp_warm_start_hi_when_baseline_inside_preferred() {
    // Baseline 3.0mm, LUT ap_max = 3.5mm (below 1.3×3.0 = 3.9).
    // Old behaviour: hi clamped to ap_max = 3.5.
    // G16 Step 4: warm-start hi = 3.9, plus an outside-preferred
    // probe at ap_max × 1.15 = 4.025 — search now exceeds the LUT
    // upper bound.
    let row = synthetic_lut_row(Some(1.0), Some(3.5));
    let variants = build_doc_variants(3.0, Some(&row), OperationType::Adaptive3d);
    let last = *variants.last().unwrap();
    assert!(
        last > 3.5,
        "search should now extend above LUT ap_max via outside-preferred probe: got {variants:?}"
    );
}

#[test]
fn always_includes_baseline() {
    // Even when LUT bounds shrink the envelope to almost nothing,
    // baseline always survives.
    let row = synthetic_lut_row(Some(2.95), Some(3.05));
    let variants = build_doc_variants(3.0, Some(&row), OperationType::Adaptive3d);
    assert!(
        variants.iter().any(|v| (v - 3.0).abs() < 1e-6),
        "baseline must be present: got {variants:?}"
    );
}

#[test]
fn tight_lut_envelope_no_longer_collapses_search() {
    // LUT row tight to within microns of baseline. Old behaviour:
    // intersection with mult envelope collapsed to ~baseline.
    // G16 Step 4: warm-start uses pure multipliers so the search
    // still spans [0.7×3.0, 3.0, 1.3×3.0]; the tight LUT bounds
    // generate near-baseline outside-preferred probes that simply
    // dedupe against the warm-start grid.
    let row = synthetic_lut_row(Some(2.9999), Some(3.0001));
    let variants = build_doc_variants(3.0, Some(&row), OperationType::Adaptive3d);
    // We retain a useful spread, not a collapsed pair.
    assert!(
        variants.len() >= 3,
        "warm-start should not collapse against tight LUT: got {variants:?}"
    );
    // Baseline still present, low and high endpoints span around it.
    assert!(variants.first().unwrap() < &3.0);
    assert!(variants.last().unwrap() > &3.0);
}

#[test]
fn floors_at_hard_minimum_for_tiny_baseline() {
    // Baseline 0.01mm — below the hard floor. Floor brings it up.
    let variants = build_doc_variants(0.01, None, OperationType::Pocket);
    // Every variant respects the floor (0.05mm).
    for v in &variants {
        assert!(
            *v >= search_policy().axes.doc.hard_floor.value - 1e-9,
            "got {variants:?}"
        );
    }
}

#[test]
fn lut_row_with_ap_min_above_baseline_does_not_crash() {
    // Degenerate case: LUT row's ap_min > baseline. The user's
    // baseline is below the calibrated range. Grid should sort and
    // dedupe sanely without crashing.
    let row = synthetic_lut_row(Some(5.0), Some(8.0));
    let variants = build_doc_variants(2.0, Some(&row), OperationType::Adaptive3d);
    // Variants are sorted ascending.
    for w in variants.windows(2) {
        assert!(
            w[0] <= w[1] + 1e-9,
            "variants must be sorted: got {variants:?}"
        );
    }
    // Baseline survives.
    assert!(variants.iter().any(|v| (v - 2.0).abs() < 1e-6));
}

fn synthetic_lut_row_with_ae(ae_min: Option<f64>, ae_max: Option<f64>) -> MatchedRow {
    let mut row = synthetic_lut_row(None, None);
    row.ae_min_mm = ae_min;
    row.ae_max_mm = ae_max;
    row
}

#[test]
fn stepover_three_variant_op_with_no_lut_row() {
    // Adaptive3d with baseline 0.8mm and no LUT bounds.
    // Multiplier envelope: [0.7×0.8, 0.8, 1.3×0.8] = [0.56, 0.8, 1.04].
    // Factory default anchor for Adaptive3d stepover is 2.0mm — this
    // is far enough from the baseline range that it survives the
    // dedup, giving 4 sorted variants.
    let variants = build_stepover_variants(0.8, None, OperationType::Adaptive3d);
    assert_eq!(variants.len(), 4, "got {variants:?}");
    assert!((variants[0] - 0.56).abs() < 1e-6);
    assert!((variants[1] - 0.8).abs() < 1e-6);
    assert!((variants[2] - 1.04).abs() < 1e-6);
    assert!((variants[3] - 2.0).abs() < 1e-6);
}

#[test]
fn stepover_factory_default_anchored_when_baseline_low() {
    // Regression test for the wanaka case: a user-set stepover well
    // below the operation's factory default should still produce a
    // candidate at the default value, so the optimizer can walk the
    // toolpath back up into the well-known safe envelope.
    let variants = build_stepover_variants(0.84, None, OperationType::Adaptive3d);
    assert!(
        variants.iter().any(|v| (v - 2.0).abs() < 1e-6),
        "expected the Adaptive3d default stepover (2.0) to be \
         included as an anchor candidate, got {variants:?}"
    );
}

#[test]
fn doc_factory_default_anchored_when_baseline_low() {
    // Same regression for DOC: a tiny user baseline should still
    // produce the factory default DOC as a candidate.
    let variants = build_doc_variants(0.5, None, OperationType::Adaptive3d);
    assert!(
        variants.iter().any(|v| (v - 3.0).abs() < 1e-6),
        "expected the Adaptive3d default DOC (3.0) to be included \
         as an anchor candidate, got {variants:?}"
    );
}

#[test]
fn stepover_four_variant_op_with_no_lut_row() {
    // Pocket with baseline 2.0mm and no LUT bounds.
    // Expected: [0.7×2.0, 2.0, mid(2.0, 2.8), 2.8] = [1.4, 2.0, 2.4, 2.8]
    let variants = build_stepover_variants(2.0, None, OperationType::Pocket);
    assert_eq!(variants.len(), 4, "got {variants:?}");
    assert!((variants[0] - 1.4).abs() < 1e-6);
    assert!((variants[1] - 2.0).abs() < 1e-6);
    assert!((variants[2] - 2.4).abs() < 1e-6);
    assert!((variants[3] - 2.8).abs() < 1e-6);
}

#[test]
fn stepover_warm_start_unclamped_by_lut_with_outside_preferred_probes() {
    // Baseline 1.0mm, LUT ae_min 0.8mm, ae_max 1.2mm.
    // Old behaviour: warm-start clamped to LUT [0.8, 1.2].
    // G16 Step 4: warm-start uses pure mults [0.7, 1.0, 1.3]; LUT
    // is preferred and produces outside-preferred probes at 0.68
    // (ae_min × 0.85) and 1.38 (ae_max × 1.15).
    let row = synthetic_lut_row_with_ae(Some(0.8), Some(1.2));
    let variants = build_stepover_variants(1.0, Some(&row), OperationType::Adaptive3d);
    // Warm-start lo = 0.7 must be present (not LUT's 0.8).
    assert!(
        variants.iter().any(|v| (v - 0.7).abs() < 1e-6),
        "warm-start lo (0.7) should be unclamped: got {variants:?}"
    );
    // Warm-start hi = 1.3 must be present (not LUT's 1.2).
    assert!(
        variants.iter().any(|v| (v - 1.3).abs() < 1e-6),
        "warm-start hi (1.3) should be unclamped: got {variants:?}"
    );
    // Outside-preferred probe above LUT: 1.2 × 1.15 = 1.38.
    assert!(
        variants.iter().any(|v| (v - 1.38).abs() < 1e-6),
        "expected outside-preferred probe above LUT: got {variants:?}"
    );
}

#[test]
fn wanaka_tp4_stepover_search_extends_beyond_lut_max() {
    // Regression for the wanaka TP 4 bug (G16 §1.3): baseline 0.84mm
    // Adaptive3d stepover, LUT ae_max = 0.95mm capped the search at
    // [0.59, 0.84, 0.95, 2.0]. After Step 4, search must extend
    // beyond LUT ae_max via outside-preferred probes and unclamped
    // warm-start hi.
    let row = synthetic_lut_row_with_ae(Some(0.40), Some(0.95));
    let variants = build_stepover_variants(0.84, Some(&row), OperationType::Adaptive3d);
    // At least one variant must sit strictly above LUT ae_max.
    assert!(
        variants.iter().any(|v| *v > 0.95 + 1e-6),
        "wanaka TP 4 stepover search should now extend above LUT ae_max=0.95, got {variants:?}"
    );
    // Baseline still present.
    assert!(variants.iter().any(|v| (v - 0.84).abs() < 1e-6));
    // Factory-default anchor (Adaptive3d default 2.0mm) still present.
    assert!(variants.iter().any(|v| (v - 2.0).abs() < 1e-6));
}

#[test]
fn stepover_floors_at_hard_minimum() {
    let variants = build_stepover_variants(0.01, None, OperationType::Pocket);
    for v in &variants {
        assert!(
            *v >= search_policy().axes.stepover.hard_floor.value - 1e-9,
            "got {variants:?}"
        );
    }
}

#[test]
fn stepover_always_includes_baseline() {
    // Tight LUT bounds — baseline must still survive the dedup.
    let row = synthetic_lut_row_with_ae(Some(0.99), Some(1.01));
    let variants = build_stepover_variants(1.0, Some(&row), OperationType::Adaptive3d);
    assert!(
        variants.iter().any(|v| (v - 1.0).abs() < 1e-6),
        "baseline must be present: got {variants:?}"
    );
}
