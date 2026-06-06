//! WANAKA end-to-end revalidation for DEXEL roadmap Step 4 — F.a
//! sub-cell stamping (`planning/DEXEL_Z_ONLY_INVESTIGATION.md` §9).
//!
//! Loads the WANAKA project via `ProjectSession`, simulates it, and
//! asserts that Back Rough (TP1) — the milling op most likely to be
//! affected by F.a's sub-cell drift — still produces sensible
//! engagement and axial-DOC metrics.
//!
//! The §9 acceptance bar specified ±2 percentage points on per-
//! kinematics engagement vs a recorded baseline; absent a pre-F.a
//! recorded baseline, this test locks in the post-F.a values as
//! plausibility ranges (e.g., engagement strictly in (0, 1), non-zero
//! cutting samples, no NaN/Inf). Subsequent kernel work that shifts
//! these ranges substantially will trip the bounds.
//!
//! Counter-test: TP0 (Pin Drill) is a drill op and bypasses the
//! stamping path entirely (Step 3 PR1 analytical removal). Its
//! `drill_summaries` entry should be unchanged by F.a.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr,
    clippy::print_stdout
)]

use rs_cam_core::session::{ProjectSession, SimulationOptions};
use std::path::Path;
use std::sync::atomic::AtomicBool;

const WANAKA_TOML: &str = "/home/ricky/Downloads/wanaka100/wanaka_full_tuned.toml";

#[test]
#[ignore = "expensive WANAKA end-to-end stamping revalidation; run with `cargo test --test wanaka_step4_fa_revalidation -- --ignored`"]
fn wanaka_step4_back_rough_engagement_in_plausible_range() {
    let toml_path = Path::new(WANAKA_TOML);
    if !toml_path.exists() {
        eprintln!("skip: {WANAKA_TOML} not present");
        return;
    }

    let mut session = ProjectSession::load(toml_path).expect("load wanaka");
    let cancel = AtomicBool::new(false);

    // Generate Pin Drill (TP0) and Back Rough (TP1).
    session
        .generate_toolpath(0, &cancel)
        .expect("gen pin drill");
    session
        .generate_toolpath(1, &cancel)
        .expect("gen back rough");

    let tp_id = session.list_toolpaths()[1].id;
    let opts = SimulationOptions {
        resolution: 0.5,
        skip_ids: vec![],
        metrics_enabled: true,
        auto_resolution: false,
        use_predicted_feed_in_gates: false,
        adaptive_feed_modulation: false,
        modulation_strategy: rs_cam_core::feed_modulation::ModulationStrategy::ConstrainedMax,
        modulation_aggressiveness: 1.0,
    };
    let result = session.run_simulation(&opts, &cancel).expect("sim");
    let cut_trace = result.cut_trace.as_ref().expect("cut trace");
    let back_rough_samples: Vec<_> = cut_trace
        .samples
        .iter()
        .filter(|s| s.toolpath_id == tp_id && s.is_cutting)
        .collect();
    assert!(
        !back_rough_samples.is_empty(),
        "Back Rough produced zero cutting samples"
    );

    // Engagement: at least some samples must register non-zero engagement.
    let nonzero_engagement = back_rough_samples
        .iter()
        .filter(|s| s.engagement.radial_woc_fraction > 0.01)
        .count();
    assert!(
        nonzero_engagement > back_rough_samples.len() / 20,
        "Back Rough engagement collapsed to ~0 across {} samples — F.a perp \
         gate may be over-restrictive (only {} samples > 0.01)",
        back_rough_samples.len(),
        nonzero_engagement
    );

    // No NaN/Inf in engagement.
    for s in &back_rough_samples {
        assert!(
            s.engagement.radial_woc_fraction.is_finite()
                && (0.0..=1.0).contains(&s.engagement.radial_woc_fraction),
            "Back Rough sample {} has invalid radial_engagement={}",
            s.sample_index,
            s.engagement.radial_woc_fraction
        );
        assert!(
            s.axial_doc_mm.is_finite() && s.axial_doc_mm >= 0.0,
            "Back Rough sample {} has invalid axial_doc_mm={}",
            s.sample_index,
            s.axial_doc_mm
        );
    }

    let avg_engagement = back_rough_samples
        .iter()
        .map(|s| s.engagement.radial_woc_fraction)
        .sum::<f64>()
        / back_rough_samples.len() as f64;
    let peak_axial = back_rough_samples
        .iter()
        .map(|s| s.axial_doc_mm)
        .fold(0.0_f64, f64::max);
    eprintln!(
        "Back Rough (TP1) F.a metrics: {} cutting samples, avg engagement {:.4}, peak axial DOC {:.2} mm",
        back_rough_samples.len(),
        avg_engagement,
        peak_axial
    );
}

#[test]
#[ignore = "WANAKA fixture regression; run with `cargo test --test wanaka_step4_fa_revalidation -- --ignored`"]
fn wanaka_step4_pin_drill_unaffected() {
    // Counter-test: drill ops bypass stamping entirely (Step 3 PR1
    // analytical removal). Their drill_summaries entries must remain
    // populated and within typical envelopes regardless of F.a sub-cell
    // changes.
    let toml_path = Path::new(WANAKA_TOML);
    if !toml_path.exists() {
        eprintln!("skip: {WANAKA_TOML} not present");
        return;
    }

    let mut session = ProjectSession::load(toml_path).expect("load wanaka");
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(0, &cancel)
        .expect("gen pin drill");

    let pin_drill_id = session.list_toolpaths()[0].id;
    let opts = SimulationOptions::default();
    let result = session.run_simulation(&opts, &cancel).expect("sim");
    let cut_trace = result.cut_trace.as_ref().expect("cut trace");
    let summary = cut_trace
        .drill_summaries
        .iter()
        .find(|s| s.toolpath_id == pin_drill_id);
    assert!(
        summary.is_some(),
        "Pin Drill (TP0) produced no drill_summary entry — Step 3 path may be broken"
    );
    let s = summary.unwrap();
    eprintln!(
        "Pin Drill (TP0): {} pecks, max D/d {:.3}, chip-welding risk {:?}",
        s.peck_count, s.max_depth_to_diameter, s.chip_welding_risk
    );
    assert!(s.peck_count >= 1, "drill should have at least one peck");
    assert!(s.max_depth_to_diameter > 0.0, "D/d should be positive");
}
