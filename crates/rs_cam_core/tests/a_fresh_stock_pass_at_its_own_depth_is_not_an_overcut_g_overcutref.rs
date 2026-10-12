//! **G-OVERCUTREF — a fresh-stock pass at its own depth is not an over-cut.**
//!
//! The defect (measured 2026-10-12, the optimizer's R0 run on
//! "Channel BACK_CHANNEL_Z22.00" of `planning/rivmap_block_job/testpiece_mini.toml`,
//! a 22 mm pocket on fresh stock): the finding
//! `project.crosses_standing_material` fired at every depth per pass above
//! the baseline, with a peak EQUAL to the cell's own commanded step
//! (dpp 0.8148 -> peak 0.81, 1.2222 -> 1.22, 2.75 -> 2.75). Its reference was
//! "3x this pass's own median bite". The ring overlap and the entry ramps of a
//! pocket bite less than a full level, so the median sits far below the step,
//! and a pass that cut its OWN step read as "standing material". The verdict
//! also changed with the stepover, because the stepover moves the median.
//!
//! The rule now: an op with a commanded depth per pass is read against that
//! step plus one dexel cell (`stock::sim_triage::commanded_step_mm`). These
//! sentries hold the two halves of that rule:
//!
//! 1. A pass whose cutting samples bite exactly its step, with a population
//!    of smaller bites (ring overlap, entry ramps), is silent at two
//!    stepovers (two medians). The first trace is the shape that tripped the
//!    old rule; the test proves that on the own-median arm first, so it is
//!    not vacuous.
//! 2. A pass where 10 % of the samples bite 3x the step fires, with the right
//!    peak, percentage and reference in its text.
//!
//! Run: `cargo test -p rs_cam_core -q --test a_fresh_stock_pass_at_its_own_depth_is_not_an_overcut_g_overcutref`

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::BTreeMap;

use rs_cam_core::ToolpathId;
use rs_cam_core::diagnostics::{DiagnosticEvidence, ids};
use rs_cam_core::stock::sim_measurability::MeasurabilityReport;
use rs_cam_core::stock::sim_triage::{Finding, SimulationTriage, TriageInputs};
use rs_cam_core::stock::simulation_cut::{Engagement, SimulationCutSample, SimulationCutTrace};
use rs_cam_core::toolpath::MoveIntent;

/// The toolpath of every hand-built trace here.
const TP: ToolpathId = ToolpathId(1);

/// The realised step of the measured pocket: 22 mm at a requested 0.8148 mm
/// is `22 / ceil(22 / 0.8148)` = 22 / 27.
const STEP_MM: f64 = 22.0 / 27.0;

/// The dexel cell, in mm. Any value under the step shows the rule; 0.25 mm
/// is a cell the session's resolution dial offers.
const CELL_MM: f64 = 0.25;

/// The samples of one pass. Each `(count, bite_mm, intent)` row gives
/// `count` cutting samples that remove `bite_mm` on the lateral axis.
fn trace(rows: &[(usize, f64, MoveIntent)]) -> SimulationCutTrace {
    let mut samples = Vec::new();
    for &(count, bite_mm, intent) in rows {
        for _ in 0..count {
            let i = samples.len();
            samples.push(SimulationCutSample {
                toolpath_id: TP,
                move_index: i,
                sample_index: i,
                segment_time_s: 0.01,
                cumulative_time_s: i as f64 * 0.01,
                is_cutting: true,
                position: [i as f64 * 0.1, 0.0, -bite_mm],
                axial_engagement_mm: bite_mm,
                engagement: Engagement::with_radial_woc(0.4),
                source_intent: Some(intent),
                removed_volume_est_mm3: 1.0,
                ..SimulationCutSample::test_fixture()
            });
        }
    }
    SimulationCutTrace::from_samples(0.5, samples)
}

/// The standing-material finding of the triage on `trace`, if any.
/// `step` is the commanded step of `TP`; `None` reads the own-median arm.
fn finding(trace: &SimulationCutTrace, step: Option<f64>) -> Option<Finding> {
    let measurability = MeasurabilityReport {
        cell_mm: Some(CELL_MM),
        ..MeasurabilityReport::default()
    };
    let steps: BTreeMap<ToolpathId, f64> = step.map(|s| (TP, s)).into_iter().collect();
    let triage = SimulationTriage::build(&TriageInputs {
        trace,
        measurability: &measurability,
        diagnostics: &[],
        rapid_collisions: &[],
        toolpath_names: &BTreeMap::new(),
        holder_collisions: &[],
        tool_diameters_mm: &BTreeMap::new(),
        commanded_depth_per_pass_mm: &steps,
        kinematic_utilization: &BTreeMap::new(),
        region_of: None,
    });
    triage
        .actions
        .into_iter()
        .find(|f| f.diagnostic.id.as_str() == ids::PROJECT_CROSSES_STANDING_MATERIAL)
}

/// A small stepover: the ring overlap is wide, so most samples bite a
/// fraction of the step. 110 ring-overlap samples at 0.3 x step, 10 entry
/// ramp samples at 0.2 x step, 80 full-level samples at the step. Median:
/// 0.3 x step; 3 x median = 0.9 x step, under the step.
fn small_stepover() -> SimulationCutTrace {
    trace(&[
        (10, 0.2 * STEP_MM, MoveIntent::EntryRamp),
        (110, 0.3 * STEP_MM, MoveIntent::ClearingCut),
        (80, STEP_MM, MoveIntent::ClearingCut),
    ])
}

/// A large stepover: the ring overlap is narrow. 30 ring-overlap samples at
/// 0.3 x step, 10 entry ramp samples at 0.2 x step, 160 full-level samples
/// at the step. Median: the step.
fn large_stepover() -> SimulationCutTrace {
    trace(&[
        (10, 0.2 * STEP_MM, MoveIntent::EntryRamp),
        (30, 0.3 * STEP_MM, MoveIntent::ClearingCut),
        (160, STEP_MM, MoveIntent::ClearingCut),
    ])
}

#[test]
fn a_fresh_stock_pass_at_its_own_step_is_silent_at_two_stepovers() {
    // Non-vacuity: the small-stepover trace IS the measured defect. Read on
    // the own-median arm (no commanded step), it fires with a peak equal to
    // the step, exactly as the R0 log showed.
    let old = finding(&small_stepover(), None)
        .expect("the small-stepover trace must trip the own-median arm");
    match old.diagnostic.evidence {
        Some(DiagnosticEvidence::SampleRange { observed, .. }) => {
            assert!((observed - STEP_MM).abs() < 1e-9, "observed {observed}");
        }
        other => panic!("expected SampleRange evidence, got {other:?}"),
    }
    // And the instability: the same pass at a larger stepover is silent
    // on that arm. A verdict that turns on the stepover is not a measure
    // of standing material.
    assert!(
        finding(&large_stepover(), None).is_none(),
        "the large-stepover trace is silent on the own-median arm"
    );

    // The rule: with the commanded step, both stepovers are silent.
    for (name, t) in [
        ("small stepover", small_stepover()),
        ("large stepover", large_stepover()),
    ] {
        let f = finding(&t, Some(STEP_MM));
        assert!(
            f.is_none(),
            "{name}: a fresh-stock pass at its commanded step must not read as \
             standing material; got {:?}",
            f.map(|f| f.diagnostic.message)
        );
    }
}

#[test]
fn a_thick_bite_into_standing_material_fires_with_its_peak() {
    // 10 % of 200 samples meet material an upstream op left standing and
    // bite 3 x the step. The rest cut the step.
    let deep = 3.0 * STEP_MM;
    let t = trace(&[
        (180, STEP_MM, MoveIntent::ClearingCut),
        (20, deep, MoveIntent::ClearingCut),
    ]);
    let f = finding(&t, Some(STEP_MM)).expect("a 3x-step bite on 10 % of samples must fire");

    assert_eq!(f.occurrences, 20, "10 % of 200 samples");
    match &f.diagnostic.evidence {
        Some(DiagnosticEvidence::SampleRange {
            observed,
            threshold,
            unit,
            ..
        }) => {
            assert!((observed - deep).abs() < 1e-9, "observed {observed}");
            let bar = threshold.expect("the bar is published");
            assert!((bar - (STEP_MM + CELL_MM)).abs() < 1e-9, "bar {bar}");
            assert_eq!(unit, "mm");
        }
        other => panic!("expected SampleRange evidence, got {other:?}"),
    }
    let msg = &f.diagnostic.message;
    assert!(
        msg.contains("commanded 0.81 mm depth per pass"),
        "the text names its reference: {msg}"
    );
    assert!(
        msg.contains("10.0%"),
        "the text gives the percentage: {msg}"
    );
    assert!(
        msg.contains("peaking at 2.44 mm"),
        "the text gives the peak: {msg}"
    );
    assert!(
        !msg.contains("median"),
        "a stepped op is not read against a median: {msg}"
    );
}
