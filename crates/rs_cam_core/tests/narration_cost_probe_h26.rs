//! H2.6 — what does `narrate_toolpath` actually COST?
//!
//! ## The number this replaces
//!
//! "~12 min" has been quoted for `narrate_toolpath` since
//! `planning/review_2026-07-29/ORCHESTRATION_LOG.md:1402`, restated at
//! `:1960`, and it reached the shipped MCP tool description. W8 traced it
//! (`planning/review_2026-08-04/FINISHING_OPEN_DEFECTS_EVIDENCE.md` §3.D.2):
//! it is a **single wall-clock observation taken during a >40-minute
//! `generate_all` in which everything queued**. No profile was ever taken,
//! and sizing narration's own loops against that run's figures (148,429
//! moves, 202 semantic items, 0 depth levels) does not reach 720 s.
//!
//! Checkpoint F3 retired the figure and asked for one timed run on an idle
//! lane to replace it. **This is that run.**
//!
//! ## What it measures, and what it deliberately does not
//!
//! `ProjectSession::narrate_toolpath` — narration compute with **no queue in
//! front of it**: no GUI thread, no `drain_mcp_requests`, no generation in
//! flight. That isolates the one thing the "~12 min" figure could not
//! separate. It does NOT measure the GUI handler's extra work
//! (`MeasurabilityReport::from_trace` once per toolpath before narration
//! emits a character) — that is a viz-side cost and is named as such rather
//! than folded in.
//!
//! The structural finding stands regardless of the number: narration carries
//! two super-linear scans (`find_or_create_level_accumulator`, `O(moves ×
//! levels)`; `fallback_semantic_region_count_at_z`, `O(items²)` per level) on
//! the shared frame-loop thread with no bound. This probe sizes them on a
//! real generated toolpath rather than on an anecdote.
//!
//! `#[ignore]`d: it generates and simulates, so it costs real seconds and has
//! no place in the default run. Run it with
//! `cargo test --release -p rs_cam_core --test narration_cost_probe_h26 --
//! --ignored --nocapture`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]

use std::sync::atomic::AtomicBool;
use std::time::Instant;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::ScallopConfig;
use rs_cam_core::session::SimulationOptions;

mod common;

use common::meshes::sawtooth_plate;
use common::session::{generate, mesh_model, single_op_session, stock_over};
use common::tools::ball_tool_config;

const HALF_MM: f64 = 20.0;
const BALL_DIAMETER_MM: f64 = 3.0;
const SIM_RESOLUTION_MM: f64 = 0.25;

#[test]
#[ignore = "H2.6 narration cost probe — generates and simulates a real toolpath, then times \
            narration on an idle lane. Prints; asserts only a sanity bound."]
fn narrate_toolpath_on_an_idle_lane_is_timed() {
    let mesh = sawtooth_plate(HALF_MM, 6.0, 2.0);
    let mut session = single_op_session(
        stock_over(HALF_MM, 6.0),
        ball_tool_config(BALL_DIAMETER_MM),
        mesh_model(mesh, "corrugated"),
        "Scallop",
        OperationConfig::Scallop(ScallopConfig {
            scallop_height: 0.02,
            tolerance: 0.02,
            continuous: false,
            ..ScallopConfig::default()
        }),
    );

    let gen_started = Instant::now();
    generate(&mut session, 0);
    let gen_s = gen_started.elapsed().as_secs_f64();

    let move_count = session
        .get_result(0)
        .expect("a generated result")
        .toolpath()
        .moves
        .len();

    let cancel = AtomicBool::new(false);
    let sim_started = Instant::now();
    session
        .run_simulation(
            &SimulationOptions {
                resolution: SIM_RESOLUTION_MM,
                metrics_enabled: true,
                auto_resolution: false,
                ..SimulationOptions::default()
            },
            &cancel,
        )
        .expect("simulation completes");
    let sim_s = sim_started.elapsed().as_secs_f64();

    // ── the measurement ─────────────────────────────────────────────────
    let started = Instant::now();
    let text = session
        .narrate_toolpath(0)
        .expect("narration of a generated toolpath");
    let narrate_s = started.elapsed().as_secs_f64();

    let sample_count = session
        .simulation_result()
        .and_then(|s| s.cut_trace.as_ref())
        .map_or(0, |t| t.samples.len());

    println!(
        "\n== H2.6 narration cost, idle lane ==\n\
         fixture: sawtooth_plate({HALF_MM}, 6, 2), Ø{BALL_DIAMETER_MM} ball, scallop 0.02 mm cusp\n\
         moves: {move_count} | cut-trace samples: {sample_count} | sim cell {SIM_RESOLUTION_MM} mm\n\
         generate: {gen_s:.2} s | simulate: {sim_s:.2} s\n\
         >>> narrate_toolpath: {narrate_s:.3} s  ({} chars of output)\n\
         \n\
         The retired figure was \"~12 min\" (720 s), taken during a >40-minute\n\
         generate_all in which every MCP call queued. This run has no queue.",
        text.len(),
    );

    // Non-vacuity: narration must have had something to narrate.
    assert!(
        move_count > 1000,
        "the fixture emitted only {move_count} moves — too small to size anything"
    );
    assert!(
        text.len() > 500,
        "narration produced {} characters; it cannot have run",
        text.len()
    );

    // A sanity bound, not a performance gate: this exists so the probe fails
    // loudly if narration ever DOES reach the order of the retired anecdote.
    assert!(
        narrate_s < 60.0,
        "narration took {narrate_s:.1} s on {move_count} moves — that is the order the \
         retired \"~12 min\" figure implied, and it would mean the super-linear scans \
         §3.D.2 names have found a fixture that exercises them. Investigate before \
         relaxing this."
    );
}
