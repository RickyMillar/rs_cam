//! O6: chip-thickness convention must match the LUT cap convention.
//!
//! The chipload gate (`tool_load::chipload::evaluate`) compares each
//! steady-state sample's `effective_chip_thickness_mm` against the
//! vendor LUT row's `chip_load_max_mm`. If the simulator and the LUT
//! disagree about which chip-thickness convention the value represents
//! (peak instantaneous on the engagement arc vs. arc-average vs. some
//! equivalent rectangular), the gate trips on otherwise-healthy cuts.
//!
//! Symptom on wanaka Back Rough (TP1): the simulator currently exposes
//! `geometry.max_chip_thickness_mm` to the gate. At arc engagement
//! `≥ π/2` (a common occurrence at adaptive corner cleanup or any
//! ≥50% radial step), the formula's `h_max = feed_per_tooth × sin(arc)`
//! peaks at `feed_per_tooth` itself — the unscaled commanded chipload.
//! Hardwood pocket roughing LUT caps live around 0.025–0.060 mm; the
//! commanded 0.0875 mm/tooth on wanaka exceeds that immediately. The
//! gate fires `Exceeds(ChiploadBreakageRisk)` regardless of axial DOC,
//! and the optimizer refuses with `BipolarEngagement`.
//!
//! Fix direction (per planning/AGENTSEARCH_NEXT_SESSION.md, option A):
//! switch the simulator to expose the AVERAGE chip thickness across
//! the engagement arc (`geometry.mean_chip_thickness_mm`). The mean
//! formula is the integral-average of `feed × sin(φ)` over the arc and
//! is what vendor LUT chip-load bounds are calibrated against.

#![allow(clippy::expect_used, clippy::panic)]

use std::f64::consts::FRAC_PI_2;

use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::compute::tool_config::ToolMaterial;
use rs_cam_core::dexel_stock::effective_chip_thickness_mm;
use rs_cam_core::feeds::vendor_lut::{LutOperationFamily, LutPassRole};
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::simulation_cut::{
    CutKinematics, SimulationCutSample, SimulationCutSummary, SimulationCutTrace,
};
use rs_cam_core::tool::{EngagementMode, FlatEndmill, MillingCutter, ToolDefinition};
use rs_cam_core::tool_load::chipload;
use rs_cam_core::tool_load::verdict::Confidence;

/// Wanaka-like nominal chipload (mm/tooth) — matches the value that
/// trips the gate end-to-end on wanaka Back Rough.
const WANAKA_FEED_PER_TOOTH_MM: f64 = 0.0875;
/// Wanaka Back Rough commanded depth-per-pass.
const WANAKA_AXIAL_DOC_MM: f64 = 3.0;
/// Wanaka tool: 1.587mm 2-flute. We use a 6.0 mm 2-flute for the repro
/// so the LUT diameter exactly matches the calibrated row (the
/// `flat_end / pocket / roughing / hardwood` 6 mm row at janka 1450) —
/// avoids any extrapolation scaling that would otherwise shift the
/// chipload bounds and obscure what this test is checking
/// (chip-thickness convention, peak vs arc-average).
fn wanaka_tool() -> ToolDefinition {
    ToolDefinition::new(
        Box::new(FlatEndmill::new(6.0, 20.0)),
        6.0,
        30.0,
        20.0,
        30.0,
        2,
        ToolMaterial::Carbide,
    )
}

/// Sample template at half-engagement (arc = π/2), wanaka-commanded
/// feed/RPM/flutes. The `effective_chip_thickness_mm` is filled in by
/// the simulator helper `effective_chip_thickness_mm(...)` — this is
/// the line the fix changes.
#[allow(dead_code)]
fn half_engagement_sample(
    cutter: &dyn MillingCutter,
    tp_id: usize,
    idx: usize,
) -> SimulationCutSample {
    let arc = FRAC_PI_2;
    let chipload = WANAKA_FEED_PER_TOOTH_MM;
    let exposed = effective_chip_thickness_mm(cutter, WANAKA_AXIAL_DOC_MM, Some(arc), chipload, 2);
    SimulationCutSample {
        toolpath_id: tp_id,
        move_index: idx,
        sample_index: idx,
        position: [0.0, 0.0, 0.0],
        cumulative_time_s: 0.0,
        segment_time_s: 0.1,
        is_cutting: true,
        cut_kinematics: CutKinematics::Linear,
        feed_rate_mm_min: 1000.0,
        spindle_rpm: 18000,
        flute_count: 2,
        axial_doc_mm: WANAKA_AXIAL_DOC_MM,
        radial_engagement: 0.5,
        arc_engagement_radians: Some(arc),
        chipload_mm_per_tooth: chipload,
        effective_chip_thickness_mm: exposed,
        removed_volume_est_mm3: 0.1,
        mrr_mm3_s: 1.0,
        semantic_item_id: None,
        span_path: Vec::new(),
    }
}

fn trace(samples: Vec<SimulationCutSample>) -> SimulationCutTrace {
    SimulationCutTrace {
        schema_version: 1,
        sample_step_mm: 1.0,
        summary: SimulationCutSummary {
            sample_count: samples.len(),
            toolpath_count: 1,
            issue_count: 0,
            hotspot_count: 0,
            total_runtime_s: 1.0,
            cutting_runtime_s: 1.0,
            rapid_runtime_s: 0.0,
            air_cut_time_s: 0.0,
            low_engagement_time_s: 0.0,
            average_engagement: 0.5,
            peak_chipload_mm_per_tooth: WANAKA_FEED_PER_TOOTH_MM,
            peak_axial_doc_mm: WANAKA_AXIAL_DOC_MM,
            total_removed_volume_est_mm3: 1.0,
            average_mrr_mm3_s: 1.0,
        },
        toolpath_summaries: Vec::new(),
        semantic_summaries: Vec::new(),
        hotspots: Vec::new(),
        issues: Vec::new(),
        samples,
        provenance: None,
    }
}

/// Formula-only check: at arc = π/2 the chip-thickness value the
/// simulator exposes to the gate must reflect arc-AVERAGE chip
/// thickness, not peak instantaneous. The peak is feed_per_tooth itself
/// (= 0.0875 on wanaka), which exceeds every hardwood-roughing LUT cap
/// even though the cut is actually well-tuned.
#[test]
fn exposed_chip_thickness_at_half_engagement_uses_arc_average_convention() {
    let tool = FlatEndmill::new(6.35, 20.0);

    let exposed = effective_chip_thickness_mm(
        &tool,
        WANAKA_AXIAL_DOC_MM,
        Some(FRAC_PI_2),
        WANAKA_FEED_PER_TOOTH_MM,
        2,
    )
    .expect("flat endmill chip geometry supported at half engagement");

    // Closed-form arc-average for h(φ) = feed × sin(φ) integrated over
    // an arc symmetric about φ = π/2:
    //   mean = (2 feed / arc) × (1 - cos(arc/2))
    // For arc = π/2, feed = 0.0875:
    //   mean = (0.175 / (π/2)) × (1 - cos(π/4))
    //        ≈ 0.1114 × 0.2929 ≈ 0.03264 mm.
    let expected_mean =
        (2.0 * WANAKA_FEED_PER_TOOTH_MM / FRAC_PI_2) * (1.0 - (FRAC_PI_2 * 0.5).cos());
    assert!(
        (exposed - expected_mean).abs() < 1e-6,
        "exposed chip thickness must equal arc-average mean ({expected_mean:.5}); \
         got {exposed:.5}. The peak-instantaneous convention \
         (feed × sin(arc) = {peak:.5}) overstates the chipload by ~2.6× \
         and is not what vendor LUT caps are calibrated against.",
        peak = WANAKA_FEED_PER_TOOTH_MM,
    );

    // Also verify the trait-level chip_geometry exposes both
    // conventions distinctly so the simulator's pick is unambiguous.
    let geom = tool
        .chip_geometry(
            WANAKA_AXIAL_DOC_MM,
            FRAC_PI_2,
            WANAKA_FEED_PER_TOOTH_MM,
            2,
            EngagementMode::Slot,
        )
        .expect("flat geometry supported");
    assert!(
        (geom.max_chip_thickness_mm - WANAKA_FEED_PER_TOOTH_MM).abs() < 1e-9,
        "geometry struct still reports peak (max) — kept for consumers \
         that want the instantaneous value"
    );
    assert!(
        geom.mean_chip_thickness_mm < geom.max_chip_thickness_mm,
        "mean must be strictly below max at partial engagement"
    );
}

/// Gate-verdict check: a sample whose feed-per-tooth lands in the
/// LUT's published per-flute envelope should pass the chipload gate
/// when sampled at the LUT row's nominal engagement.
///
/// History: pre-G17 this test exercised a wanaka-feed sample (0.0875
/// mm/tooth) at half engagement — incidentally close enough to the
/// LUT row's nominal arc that the arc-average mean (~0.0326) just
/// cleared the LUT min (0.032), so the test passed without ever
/// modeling the engagement explicitly. Post-D9 (engagement-aware LUT
/// comparison), the half-engagement reading gets normalized to the
/// LUT row's nominal engagement (`acos(1 − 2·1.6/6) ≈ 1.084 rad`,
/// ~27 % radial), which surfaces a real semantic gap: at any
/// engagement, wanaka's commanded 0.0875 mm/tooth is below the
/// Amana-published per-flute minimum for HardMaple roughing
/// (`min_mean / M(arc_lut) ≈ 0.032 / 0.233 ≈ 0.137 mm/tooth`). That
/// is its own optimization conversation; the convention check below
/// just verifies that with a feed inside the published envelope, the
/// gate accepts.
#[test]
fn lut_nominal_engagement_sample_within_published_envelope_passes() {
    let tool = wanaka_tool();
    let cutter = FlatEndmill::new(6.0, 20.0);

    // Feed per tooth chosen to land safely inside the LUT envelope
    // (0.032..=0.055 mean / 0.233 factor → 0.137..=0.236 mm/tooth).
    let feed_per_tooth = 0.18;
    // Sample at the LUT row's nominal engagement so D9's per-sample
    // normalization is a no-op.
    let arc_lut_nominal = (1.0_f64 - 2.0 * 1.6 / 6.0).acos();
    let exposed = effective_chip_thickness_mm(
        &cutter,
        WANAKA_AXIAL_DOC_MM,
        Some(arc_lut_nominal),
        feed_per_tooth,
        2,
    )
    .expect("flat endmill chip geometry supported at LUT-nominal engagement");

    let sample = SimulationCutSample {
        toolpath_id: 0,
        move_index: 0,
        sample_index: 0,
        position: [0.0, 0.0, 0.0],
        cumulative_time_s: 0.0,
        segment_time_s: 0.1,
        is_cutting: true,
        cut_kinematics: CutKinematics::Linear,
        feed_rate_mm_min: 1000.0,
        spindle_rpm: 18000,
        flute_count: 2,
        axial_doc_mm: WANAKA_AXIAL_DOC_MM,
        radial_engagement: 0.27,
        arc_engagement_radians: Some(arc_lut_nominal),
        chipload_mm_per_tooth: feed_per_tooth,
        effective_chip_thickness_mm: Some(exposed),
        removed_volume_est_mm3: 0.1,
        mrr_mm3_s: 1.0,
        semantic_item_id: None,
        span_path: Vec::new(),
    };
    let trace = trace(vec![sample]);

    let verdict = chipload::evaluate(
        0,
        &tool,
        &Material::SolidWood {
            species: WoodSpecies::HardMaple,
        },
        Some(&trace),
        LutOperationFamily::Pocket,
        LutPassRole::Roughing,
        1000.0,
        OperationType::Pocket,
        &rs_cam_core::tool_load::ToleranceBands::default(),
    );

    match verdict {
        rs_cam_core::tool_load::ChiploadVerdict::Within {
            confidence: Confidence::Validated,
            ..
        } => {}
        other => panic!("unexpected verdict: {other:?}"),
    }
}

/// D9 — slot-engagement transient sample (the wanaka TP 1 surface
/// pattern: terrain-following XY+Z move that briefly slot-engages on
/// a vertical face) at a feed-per-tooth that's well inside the LUT
/// envelope must NOT trip the gate post-D9. Pre-D9 this read as
/// `mean ≈ 0.637 × feed_per_tooth` and got compared raw against the
/// LUT cap calibrated at narrow engagement, over-penalizing slot
/// transients on terrain.
#[test]
fn slot_engagement_sample_at_safe_feed_passes_after_d9_normalization() {
    use std::f64::consts::PI;

    let tool = wanaka_tool();
    let cutter = FlatEndmill::new(6.0, 20.0);

    // Feed at the middle of the LUT envelope, expressed as feed/tooth.
    // Mean chip at slot = 0.18 × 0.637 ≈ 0.115 mm — far above LUT max
    // 0.055 mm if compared raw. Post-D9 normalization scales to the
    // LUT-nominal arc (~1.084 rad, factor ≈ 0.233), giving
    // `0.115 × 0.233 / 0.637 ≈ 0.042 mm` — within bounds.
    let feed_per_tooth = 0.18;
    let arc_slot = PI;
    let exposed = effective_chip_thickness_mm(
        &cutter,
        WANAKA_AXIAL_DOC_MM,
        Some(arc_slot),
        feed_per_tooth,
        2,
    )
    .expect("flat endmill chip geometry supported at slot");
    assert!(
        exposed > 0.10,
        "raw mean at slot should be ≥ 0.10 mm to exercise normalization meaningfully (got {exposed:.4})"
    );

    let sample = SimulationCutSample {
        toolpath_id: 0,
        move_index: 0,
        sample_index: 0,
        position: [0.0, 0.0, 0.0],
        cumulative_time_s: 0.0,
        segment_time_s: 0.1,
        is_cutting: true,
        cut_kinematics: CutKinematics::Linear,
        feed_rate_mm_min: 1000.0,
        spindle_rpm: 18000,
        flute_count: 2,
        axial_doc_mm: WANAKA_AXIAL_DOC_MM,
        radial_engagement: 1.0,
        arc_engagement_radians: Some(arc_slot),
        chipload_mm_per_tooth: feed_per_tooth,
        effective_chip_thickness_mm: Some(exposed),
        removed_volume_est_mm3: 0.1,
        mrr_mm3_s: 1.0,
        semantic_item_id: None,
        span_path: Vec::new(),
    };
    let trace = trace(vec![sample]);

    let verdict = chipload::evaluate(
        0,
        &tool,
        &Material::SolidWood {
            species: WoodSpecies::HardMaple,
        },
        Some(&trace),
        LutOperationFamily::Pocket,
        LutPassRole::Roughing,
        1000.0,
        OperationType::Pocket,
        &rs_cam_core::tool_load::ToleranceBands::default(),
    );

    match verdict {
        rs_cam_core::tool_load::ChiploadVerdict::Within { .. } => {}
        rs_cam_core::tool_load::ChiploadVerdict::Exceeds {
            side: rs_cam_core::tool_load::verdict::ChipSide::High,
            triggering,
            ..
        } => panic!(
            "slot-engagement sample at safe feed-per-tooth (= {feed_per_tooth} mm/tooth) trips the chipload gate (observed = {:.4}). \
             D9 normalization should scale the slot-engagement raw mean back to LUT-nominal-engagement equivalent before comparing. \
             See planning/STRUCTURAL_ENTRY_SPANS_AND_LOCALITY.md D2.",
            triggering.observed_mm_per_tooth
        ),
        other => panic!("unexpected verdict: {other:?}"),
    }
}
