//! **R0 — the headline instrument of the optimizer rebuild.**
//!
//! Pre-registered in `planning/optimize_review_2026-10-12/PLAN.md` §2,
//! package R0. An `#[ignore]` evidence run, not a sentry: it prints and
//! asserts only that the run completed.
//!
//! The run:
//!
//! 1. Copy `planning/rivmap_block_job/testpiece_mini.toml` of the master
//!    checkout into a fresh scratch directory. The project names its model
//!    files by absolute path under
//!    `/home/ricky/cnc_jobs/rivmap_block/testpiece_mini/`, so the copy
//!    loads. The run never saves the project.
//! 2. Load it with `ProjectSession::load`, generate every enabled
//!    toolpath, and run one project simulation with the GUI operating
//!    point (`candidate_sim_options`).
//! 3. Run the `optimize_toolpath` Job on the toolpath named
//!    "Channel BACK_CHANNEL_Z22.00".
//! 4. Print the baseline (cycle, cutting, rapid and air seconds, the
//!    full-cut readings), every candidate (depth per pass, stepover,
//!    feed, RPM, links, cycle, the gate rows, the full-cut readings and
//!    verdict), the outcome, and the wall time.
//!
//! The review's numbers (`REVIEW.md` beside the plan): baseline 1 315 s;
//! deep-DOC sims 371-590 s with the depth row `Exceeds` 0.75 mm; a CLI
//! sweep saw "crosses material ... peaking at 1.47 mm" at a depth per
//! pass of 1.5 mm.
//!
//! Run it:
//! `cargo test -p rs_cam_core -q --test optimize_z22_headline -- --ignored --nocapture`

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use rs_cam_core::session::{
    Job, JobHandle, OptimizeToolpathArgs, ProjectSession, execute_optimize_toolpath,
};
use rs_cam_core::tool_load::optimize::{OptimizeCandidate, candidate_sim_options};

/// The project the review measured.
const SOURCE_PROJECT: &str =
    "/home/ricky/personal_repos/rs_cam/planning/rivmap_block_job/testpiece_mini.toml";

/// The model folder the project names by absolute path.
const MODEL_DIR: &str = "/home/ricky/cnc_jobs/rivmap_block/testpiece_mini";

/// The scratch folder of this session (the operator's instruction).
const SCRATCH_DIR: &str = "/tmp/claude-1001/-home-ricky-personal-repos-rs-cam/065a14e8-82c6-476f-9331-96d953a0dcb4/scratchpad/z22";

/// The toolpath the review measured.
const TOOLPATH_NAME: &str = "Channel BACK_CHANNEL_Z22.00";

/// Print one line of the report.
fn line(text: &str) {
    // SAFETY: an `#[ignore]` evidence instrument; its output is the
    // deliverable, and only `--nocapture` shows it.
    #[allow(clippy::print_stderr)]
    {
        eprintln!("{text}");
    }
}

fn candidate_line(index: usize, c: &OptimizeCandidate) -> String {
    let p = &c.params;
    let rows: Vec<String> = c
        .verdict
        .criteria()
        .iter()
        .map(|row| {
            format!(
                "{}={:?}{}{}",
                row.kind.label(),
                row.state,
                row.display_peak
                    .map(|v| format!(" peak {v:.4}"))
                    .unwrap_or_default(),
                row.bound
                    .map(|v| format!(" bound {v:.4}"))
                    .unwrap_or_default(),
            )
        })
        .collect();
    format!(
        "  [{index}] {:?} dpp {} stepover {} feed {:.0} plunge {:.0} rpm {:?} links {} | \
         cycle {:.1} s | gates: {} | full cut: volume {:?} mm3, collisions {:?}, overcut peak {:?} mm, \
         verdict {:?} | delta {:?}",
        c.stage,
        p.depth_per_pass()
            .map_or_else(|| "-".to_owned(), |v| format!("{v:.4}")),
        p.stepover()
            .map_or_else(|| "-".to_owned(), |v| format!("{v:.3}")),
        p.feed_rate(),
        p.plunge_rate(),
        p.spindle_rpm(),
        c.dressups.link_moves.is_some(),
        c.cycle_time_s,
        rows.join(", "),
        c.full_cut.removed_volume_mm3,
        c.full_cut.rapid_collisions,
        c.full_cut.overcut_peak_mm,
        c.full_cut.verdict,
        c.delta,
    )
}

#[test]
#[ignore = "evidence instrument: loads the mini project and runs nine simulations"]
fn optimize_the_z22_channel_of_the_mini_project() {
    let wall = Instant::now();
    if !Path::new(MODEL_DIR).is_dir() || !Path::new(SOURCE_PROJECT).is_file() {
        line(&format!(
            "R0 NOT RUN: {MODEL_DIR} or {SOURCE_PROJECT} is missing on this machine"
        ));
        return;
    }
    let run_dir: PathBuf = Path::new(SCRATCH_DIR).join(format!(
        "run-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&run_dir).expect("the scratch run folder");
    let project = run_dir.join("testpiece_mini.toml");
    std::fs::copy(SOURCE_PROJECT, &project).expect("copy the project");
    line(&format!("R0 project copy: {}", project.display()));

    let mut session = ProjectSession::load(&project).expect("the mini project loads");
    let cancel = AtomicBool::new(false);
    let t_gen = Instant::now();
    session
        .generate_all(&[], &cancel)
        .expect("every enabled toolpath generates");
    line(&format!(
        "R0 generation: {:.1} s",
        t_gen.elapsed().as_secs_f64()
    ));

    let opts = candidate_sim_options(&session);
    line(&format!(
        "R0 operating point: {:?}",
        rs_cam_core::tool_load::optimize::OperatingPointStamp::from_options(&opts)
    ));
    let t_sim = Instant::now();
    let trace = session
        .run_simulation(&opts, &cancel)
        .expect("the project simulates")
        .cut_trace
        .clone()
        .expect("the simulation keeps its cut trace");
    line(&format!(
        "R0 baseline project simulation: {:.1} s",
        t_sim.elapsed().as_secs_f64()
    ));

    let (index, tc) = session
        .toolpath_configs()
        .iter()
        .enumerate()
        .find(|(_, tc)| tc.name == TOOLPATH_NAME)
        .map(|(i, tc)| (i, tc.clone()))
        .expect("the Z22 channel is in the project");
    line(&format!(
        "R0 toolpath: index {index}, id {}, name {:?}",
        tc.id.0, tc.name
    ));
    if let Some(s) = trace
        .toolpath_summaries
        .iter()
        .find(|s| s.toolpath_id == tc.id)
    {
        line(&format!(
            "R0 baseline trace: total {:.1} s, cutting {:.1} s, rapid {:.1} s, air {:.1} s, \
             removed volume {:.0} mm3, peak axial {:.3} mm, by intent {:?}",
            s.total_runtime_s,
            s.cutting_runtime_s,
            s.rapid_runtime_s,
            s.air_cut_time_s,
            s.total_removed_volume_est_mm3,
            s.peak_axial_doc_mm,
            s.runtime_by_intent,
        ));
    }

    let t_opt = Instant::now();
    let started = session.start(
        Job::OptimizeToolpath(OptimizeToolpathArgs { index }),
        &cancel,
    );
    let JobHandle::OptimizeToolpath(mut handle) = started.expect("the Job captures") else {
        panic!("the optimizer row answers its own handle variant");
    };
    let outcome = execute_optimize_toolpath(&mut handle, &cancel);
    line(&format!(
        "R0 optimize wall: {:.1} s",
        t_opt.elapsed().as_secs_f64()
    ));

    line(&format!("R0 outcome kind: {:?}", outcome.kind));
    line(&format!(
        "R0 recommended index: {:?}",
        outcome.recommended_index
    ));
    line(&format!("R0 headline: {}", outcome.narrative.headline));
    line(&format!(
        "R0 explanation: {}",
        outcome.narrative.explanation
    ));
    line(&format!("R0 unmodeled: {:?}", outcome.narrative.unmodeled));
    line(&format!(
        "R0 refused cells ({}):",
        outcome.narrative.refused_cells.len()
    ));
    for r in &outcome.narrative.refused_cells {
        line(&format!("  - {r}"));
    }
    line(&format!(
        "R0 limiting gates: {:?}",
        outcome.narrative.limiting_gates
    ));
    line(&format!("R0 candidates ({}):", outcome.candidates.len()));
    for (i, c) in outcome.candidates.iter().enumerate() {
        line(&candidate_line(i, c));
    }
    line(&format!("R0 stamp: {:?}", outcome.assumptions));
    line(&format!(
        "R0 total wall time: {:.1} s",
        wall.elapsed().as_secs_f64()
    ));
    assert!(
        !outcome.candidates.is_empty(),
        "the run scored the baseline"
    );
}
