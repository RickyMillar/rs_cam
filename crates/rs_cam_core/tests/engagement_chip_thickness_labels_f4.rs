//! **F-4 — `Engagement::{mean,peak}_chip_thickness_mm` carry each other's
//! values.**
//!
//! Census: `planning/review_2026-08-04/FEEDS_CENSUS.md` §6.3 F-4 / P-11,
//! tier-1 item T1.3. Ruled at Checkpoint B Q2 ("the swapped
//! `Engagement::{mean,peak}_chip_thickness_mm` fields corrected
//! red-first").
//!
//! ## The defect, as it stood
//!
//! `dexel_stock/simulation.rs:496-497` assigned
//!
//! ```text
//! mean_chip_thickness_mm := chipload_mm_per_tooth        // nominal ADVANCE per tooth
//! peak_chip_thickness_mm := effective_chip_thickness_mm  // arc-MEAN chip thickness
//! ```
//!
//! while the field docs (`simulation_cut.rs:88-96`) assert the reverse —
//! `mean_` is documented as the arc-mean chip and `peak_` as "what the
//! flute experiences at its most-engaged angular position". Both
//! statements were false, and in the same direction: the field named
//! `peak_` held a value that is *smaller* than the field named `mean_`
//! for every partial-immersion cut, because
//! `mean_chip / feed_per_tooth = (2·sin(arc)/arc)·(1−cos(arc/2)) < 1`
//! below slotting.
//!
//! ## What these tests pin
//!
//! 1. [`peak_chip_thickness_is_never_below_mean_on_production_samples`] —
//!    the physical ordering, on samples produced by the real dexel
//!    simulator. **Fails on the parent revision** on every partial-
//!    immersion sample.
//! 2. [`the_two_fields_are_the_two_chip_geometry_statistics`] — the
//!    positive statement: `mean_` is `ChipGeometry::mean_chip_thickness_mm`
//!    and `peak_` is `ChipGeometry::max_chip_thickness_mm`, i.e. both come
//!    from the shipped chip model rather than one being a copy of the
//!    commanded advance.
//! 3. [`mean_chip_thickness_is_not_a_duplicate_of_the_commanded_advance`] —
//!    the negative statement that localises the defect: pre-fix,
//!    `engagement.mean_chip_thickness_mm` was bit-identical to
//!    `sample.chipload_mm_per_tooth`, a different physical quantity that
//!    the sample already publishes under its own name.
//! 4. [`the_arc_mean_and_arc_peak_separate_by_the_closed_form_factor`] —
//!    the closed form, driven through the production helpers at a
//!    hand-chosen arc so the ordering claim does not depend on whatever
//!    arcs the fixture happens to produce.
//!
//! No gate consumes either field (census §6.2 P-11 rates it MEDIUM,
//! report-only): the readers are the `per_kinematics` summary block
//! (`simulation_cut.rs:357-362`) and the MCP wire
//! (`rs_cam_viz/src/app/mcp.rs:4781-4782`). This is a report-only
//! correction under Checkpoint B Q2.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::dexel_stock::{
    StockCutDirection, TriDexelStock, effective_chip_thickness_mm, peak_chip_thickness_mm,
};
use rs_cam_core::geo::{BoundingBox3, P3};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::simulation_cut::SimulationCutSample;
use rs_cam_core::tool::{EngagementMode, FlatEndmill, MillingCutter};
use rs_cam_core::toolpath::{MoveIntent, Toolpath};

const RPM: u32 = 18_000;
const FLUTES: u32 = 2;

/// Drive the real simulator with one lateral cut and return its cutting
/// samples. Deliberately the same fixture shape as
/// `tests/engagement_vector_step2.rs` so the two files agree on what a
/// production sample looks like.
fn production_cutting_samples() -> Vec<SimulationCutSample> {
    let mut tp = Toolpath::new();
    tp.rapid_to_with_intent(P3::new(0.0, 0.0, 12.0), MoveIntent::Linking);
    tp.feed_to_with_intent(P3::new(0.0, 0.0, 5.0), 200.0, MoveIntent::EntryPlunge);
    tp.feed_to_with_intent(P3::new(10.0, 0.0, 5.0), 1000.0, MoveIntent::FinishingCut);

    let bbox = BoundingBox3 {
        min: P3::new(-5.0, -5.0, 0.0),
        max: P3::new(15.0, 15.0, 10.0),
    };
    let mut stock = TriDexelStock::from_bounds(&bbox, 0.25);
    let cutter = FlatEndmill::new(2.0, 25.0);
    let never_cancel = || false;
    let samples = stock
        .simulate_toolpath_with_metrics_with_cancel(
            &tp,
            &cutter,
            StockCutDirection::FromTop,
            ToolpathId(0),
            RPM,
            FLUTES,
            5000.0,
            0.5,
            None,
            &[],
            &[],
            true,
            &never_cancel,
        )
        .expect("simulation should complete");

    let cutting: Vec<_> = samples.into_iter().filter(|s| s.is_cutting).collect();
    assert!(
        !cutting.is_empty(),
        "fixture must produce cutting samples, else the assertions below are vacuous"
    );
    cutting
}

#[test]
fn peak_chip_thickness_is_never_below_mean_on_production_samples() {
    // The whole content of the F-4 defect in one inequality. A peak is
    // by definition the largest value a quantity takes over the arc; a
    // mean cannot exceed it. Pre-fix every partial-immersion sample
    // violated this because the two fields were filled from different
    // quantities entirely.
    let samples = production_cutting_samples();
    let mut compared = 0usize;
    for s in &samples {
        let (Some(mean), Some(peak)) = (
            s.engagement.mean_chip_thickness_mm,
            s.engagement.peak_chip_thickness_mm,
        ) else {
            continue;
        };
        compared += 1;
        assert!(
            peak >= mean - 1e-12,
            "Engagement::peak_chip_thickness_mm ({peak:.9}) is BELOW \
             mean_chip_thickness_mm ({mean:.9}) at arc {:?} rad — a peak cannot \
             be smaller than the mean of the same quantity. F-4: the fields \
             carry each other's values.",
            s.engagement.arc_radians,
        );
    }
    assert!(
        compared > 0,
        "no sample carried both chip-thickness fields — the assertion never ran"
    );
}

#[test]
fn the_two_fields_are_the_two_chip_geometry_statistics() {
    // The positive statement: both fields come from the shipped chip
    // model (`MillingCutter::chip_geometry`) rather than one of them
    // being a copy of the commanded advance.
    let cutter = FlatEndmill::new(2.0, 25.0);
    let samples = production_cutting_samples();
    let mut compared = 0usize;
    for s in &samples {
        let Some(arc) = s.arc_engagement_radians else {
            continue;
        };
        let Ok(geometry) = cutter.chip_geometry(
            s.axial_engagement_mm,
            arc,
            s.chipload_mm_per_tooth,
            s.flute_count,
            EngagementMode::Slot,
        ) else {
            continue;
        };
        compared += 1;
        let mean = s
            .engagement
            .mean_chip_thickness_mm
            .expect("arc-bearing sample must carry a mean chip thickness");
        let peak = s
            .engagement
            .peak_chip_thickness_mm
            .expect("arc-bearing sample must carry a peak chip thickness");
        assert!(
            (mean - geometry.mean_chip_thickness_mm).abs() < 1e-12,
            "mean_chip_thickness_mm must be ChipGeometry::mean_chip_thickness_mm: \
             field {mean:.9} vs model {:.9}",
            geometry.mean_chip_thickness_mm
        );
        assert!(
            (peak - geometry.max_chip_thickness_mm).abs() < 1e-12,
            "peak_chip_thickness_mm must be ChipGeometry::max_chip_thickness_mm: \
             field {peak:.9} vs model {:.9}",
            geometry.max_chip_thickness_mm
        );
    }
    assert!(compared > 0, "no arc-bearing sample — assertion never ran");
}

#[test]
fn mean_chip_thickness_is_not_a_duplicate_of_the_commanded_advance() {
    // Localises the defect. Pre-fix `mean_chip_thickness_mm` was
    // bit-identical to `chipload_mm_per_tooth` — the commanded LINEAR
    // ADVANCE per tooth, which the sample already publishes under its own
    // name. Two fields holding one number is not the defect; the defect
    // is that the second one is documented as a different quantity and is
    // read as one by `per_kinematics` and the MCP wire.
    let samples = production_cutting_samples();
    let mut partial_immersion = 0usize;
    for s in &samples {
        let (Some(arc), Some(mean)) = (
            s.arc_engagement_radians,
            s.engagement.mean_chip_thickness_mm,
        ) else {
            continue;
        };
        // At full slotting the two coincide legitimately (h_max = fpt and
        // the mean factor is 2/π ≠ 1, so even there they differ — but
        // guard the boundary anyway).
        if arc >= std::f64::consts::PI - 1e-6 {
            continue;
        }
        partial_immersion += 1;
        assert!(
            (mean - s.chipload_mm_per_tooth).abs() > 1e-12,
            "Engagement::mean_chip_thickness_mm ({mean:.9}) equals the commanded \
             advance-per-tooth ({:.9}) at arc {arc:.6} rad. Those are different \
             physical quantities (mm of chip vs mm of advance) and the field's \
             doc comment claims the former.",
            s.chipload_mm_per_tooth
        );
    }
    assert!(
        partial_immersion > 0,
        "fixture produced no partial-immersion sample — assertion never ran"
    );
}

#[test]
fn the_arc_mean_and_arc_peak_separate_by_the_closed_form_factor() {
    // Independent of whatever arcs the dexel fixture happens to produce:
    // drive the two production helpers at a hand-chosen arc and check the
    // closed form. At `arc = π/2`:
    //   h_max = fz·sin(π/2)               = fz
    //   mean  = (2·h_max/arc)·(1−cos(arc/2))
    //         = (4/π)·(1−√2/2)·fz         ≈ 0.372923·fz
    // so peak/mean ≈ 2.6822. The census's red-first bar for T1.3 is
    // exactly "a sample with arc = π/2 must show peak > mean".
    let cutter = FlatEndmill::new(6.0, 25.0);
    let arc = std::f64::consts::FRAC_PI_2;
    let fz = 0.05_f64;
    let mean = effective_chip_thickness_mm(&cutter, 2.0, Some(arc), fz, FLUTES)
        .expect("flat endmill supports the chip model at half immersion");
    let peak = peak_chip_thickness_mm(&cutter, 2.0, Some(arc), fz, FLUTES)
        .expect("flat endmill supports the chip model at half immersion");

    let expected_mean = (4.0 / std::f64::consts::PI) * (1.0 - 0.5_f64.sqrt()) * fz;
    assert!(
        (mean - expected_mean).abs() < 1e-12,
        "arc-mean chip at π/2: got {mean:.12}, closed form {expected_mean:.12}"
    );
    assert!(
        (peak - fz).abs() < 1e-12,
        "arc-peak chip at π/2 must equal the feed per tooth: got {peak:.12}, fz {fz:.12}"
    );
    assert!(
        peak > mean,
        "peak ({peak:.9}) must exceed mean ({mean:.9}) at half immersion"
    );
}
