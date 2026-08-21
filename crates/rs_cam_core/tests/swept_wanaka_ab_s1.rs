//! S1 swept-volume A/B harness — wanaka200 by default, headless, one dispatch
//! mode per process (2026-08-20; project/resolution overrides 2026-08-21).
//!
//! Drives `planning/airrun_2026-08-19/wanaka200.toml` end to end without a GUI
//! — fresh load → generate every enabled toolpath → F.4 ladder (simulate →
//! regenerate the rest ops that were waiting on simulated stock → repeat until
//! convergence) → one final metric simulation — and prints every number the S1
//! decision package needs, tagged with the stamp-dispatch mode the run used.
//!
//! # Why the caller has to run it twice
//!
//! The mode lives on `TriDexelStock::stamp_dispatch`, but the simulator builds
//! its own `TriDexelStock` inside `compute::simulate`, so a test can't set the
//! field. The reachable lever is the process-wide env hook
//! `RS_CAM_STAMP_DISPATCH`, parsed by `dexel_stock::whole_path::dispatch_override`
//! into a `OnceLock` — **once per process, permanently**. One test binary is
//! therefore one mode, and the A/B is two invocations of the binary:
//!
//! ```text
//! RS_CAM_STAMP_DISPATCH=per_stamp cargo test -p rs_cam_core --test swept_wanaka_ab_s1 -- --ignored --nocapture
//! RS_CAM_STAMP_DISPATCH=swept     cargo test -p rs_cam_core --test swept_wanaka_ab_s1 -- --ignored --nocapture
//! ```
//!
//! (Accepted values: `auto`, `per_stamp`, `whole_path`, `swept`,
//! `swept_plunge`. Unset resolves to `auto`. `swept_plunge` is the
//! bit-identical half of S1 and is the third arm worth taking.)
//!
//! # Driving a different project (W5B-F3, 2026-08-21)
//!
//! Two further env hooks let the same harness A/B any project with a
//! `FromRemainingStock` chain, which is what W5B-F3's rest-chain geometry
//! diff needs. **Both default to the wanaka200 configuration 0D was captured
//! with**, so an invocation that sets neither reproduces 0D exactly.
//!
//! * `S1AB_PROJECT` — path to the project `.toml`. Absolute, or relative to
//!   the workspace root. Unset ⇒ `planning/airrun_2026-08-19/wanaka200.toml`.
//! * `S1AB_RESOLUTION_MM` — simulation cell size for **every** simulation the
//!   run performs (ladder rounds included — see `DEFAULT_SIM_RESOLUTION_MM`). Unset
//!   ⇒ 0.4. A value that does not parse as a positive finite float is a hard
//!   failure, not a silent fallback: an A/B that quietly ran two different
//!   cell sizes would compare two geometries.
//! * `S1AB_FEED_MODULATION` — `0`/`false`/`off`/`no` disables the F-036b
//!   adaptive feed-modulation post-pass. Unset ⇒ enabled, which is the
//!   shipped `SimulationOptions` default. See `feed_modulation_enabled`.
//!
//! The `REQUIRED_MODEL` pre-flight check only applies to the default project;
//! an overridden project reports whatever `ProjectSession::load` reports.
//!
//! Capture both runs and diff them:
//!
//! ```text
//! ... RS_CAM_STAMP_DISPATCH=per_stamp ... 2>&1 | grep '^S1AB ' | sort > /tmp/a.txt
//! ... RS_CAM_STAMP_DISPATCH=swept     ... 2>&1 | grep '^S1AB ' | sort > /tmp/b.txt
//! diff /tmp/a.txt /tmp/b.txt
//! ```
//!
//! # Output contract
//!
//! One `S1AB`-prefixed line per datum, `key=value` fields only:
//!
//! ```text
//! S1AB mode=swept scope=project metric=rapid_collisions value=0
//! S1AB mode=swept scope=tp5 metric=air_cut_pct_of_total_runtime value=12.345
//! S1AB mode=swept scope=tp5 metric=measurability.air_cut state=not_measurable reason="…"
//! ```
//!
//! `scope` is `project`, `tp<toolpath_id>`, or `phase` (wall clock).
//! Timing lines carry `metric=wall_s`. Everything numeric uses `value=`; the
//! measurability rows use `state=` + `reason=` instead, because an abstention
//! is not a number (that is the whole point of `sim_measurability`).
//!
//! # What this harness asserts
//!
//! Almost nothing, on purpose: it is a measurement instrument, not a gate. The
//! single assertion is that generation produced at least one toolpath, so a
//! run that silently did no work is visible instead of printing a tidy table of
//! zeros. Every other outcome — collisions, abstentions, a ladder that fails to
//! converge — is *printed* and the run continues, because on an A/B the
//! divergence itself is the result.
//!
//! # Not reachable from a test, and therefore not printed
//!
//! * `TriDexelStock::last_stamp_dispatch` (`StampDispatchStats`: batches
//!   closed, batch sizes). The simulator's `TriDexelStock` is created and
//!   dropped inside `compute::simulate` and is not carried on
//!   `SimulationResult`, so the harness cannot confirm *how many* batches the
//!   selected mode actually formed — only that the mode was selected. Confirm
//!   non-vacuousness with the wave-4 sentries instead.
//! * Per-toolpath simulation wall clock. `run_simulation` is one call over the
//!   whole project; only phase-level timing is observable from here.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use rs_cam_core::dexel_stock::StampDispatch;
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::session::{ProjectSession, SimulationOptions};
use rs_cam_core::sim_measurability::{Measurability, MeasurabilityReport};
use rs_cam_core::simulation_cut::AirCutRatios;

/// Default simulation cell size for EVERY simulation this harness runs — the
/// ladder rounds as well as the final one. Ladder rounds feed the rest ops
/// their stock, so running them at a different resolution than the final pass
/// would A/B two different geometries. Overridable per run with
/// `S1AB_RESOLUTION_MM`; the override applies to every simulation for the same
/// reason.
const DEFAULT_SIM_RESOLUTION_MM: f64 = 0.4;

/// Project the harness drives when `S1AB_PROJECT` is unset — the wanaka200
/// configuration reference 0D was captured with.
const DEFAULT_PROJECT_REL: &str = "planning/airrun_2026-08-19/wanaka200.toml";

/// The STL the default project references by absolute path. Checked separately
/// from the project file so a missing model reports the model, not a load
/// error thirty lines deep. Only meaningful for the default project.
const REQUIRED_MODEL: &str = "/home/ricky/Downloads/wanaka200/rivmap_export/terrain.stl";

fn repo_root() -> PathBuf {
    // tests run from the crate dir; the project lives at the workspace root.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

/// `(path, is_default_project)`. A relative `S1AB_PROJECT` resolves against the
/// workspace root, not the crate dir, so the value can be pasted straight from
/// a `git`-relative path.
fn project_path() -> (PathBuf, bool) {
    match std::env::var("S1AB_PROJECT") {
        Ok(raw) if !raw.trim().is_empty() => {
            let p = PathBuf::from(raw.trim());
            let resolved = if p.is_absolute() {
                p
            } else {
                repo_root().join(p)
            };
            (resolved, false)
        }
        _ => (repo_root().join(DEFAULT_PROJECT_REL), true),
    }
}

/// Cell size for this run. A malformed override panics rather than falling
/// back: the two arms of an A/B must agree on the cell size, and a silent
/// fallback in one arm would compare two different geometries.
fn sim_resolution_mm() -> f64 {
    match std::env::var("S1AB_RESOLUTION_MM") {
        Ok(raw) if !raw.trim().is_empty() => {
            let v: f64 = raw.trim().parse().unwrap_or_else(|e| {
                panic!("S1AB_RESOLUTION_MM={raw:?} does not parse as a float: {e}")
            });
            assert!(
                v.is_finite() && v > 0.0,
                "S1AB_RESOLUTION_MM={raw:?} must be a positive finite cell size"
            );
            v
        }
        _ => DEFAULT_SIM_RESOLUTION_MM,
    }
}

/// Effective dispatch mode for this process, as a short greppable tag.
///
/// Read through `StampDispatch::default()` rather than off the env var
/// directly: that is the exact value the simulator will resolve, including the
/// `OnceLock` parse and the unset → `Auto` fallback, so a typo'd env value
/// reports the mode that actually ran instead of the one that was asked for.
fn mode_tag() -> &'static str {
    match StampDispatch::default() {
        StampDispatch::Auto => "auto",
        StampDispatch::PerStamp => "per_stamp",
        StampDispatch::WholeToolpath => "whole_path",
        StampDispatch::Swept => "swept",
        StampDispatch::SweptPlungeOnly => "swept_plunge",
    }
}

fn num(mode: &str, scope: &str, metric: &str, value: f64) {
    println!("S1AB mode={mode} scope={scope} metric={metric} value={value:.6}");
}

fn int(mode: &str, scope: &str, metric: &str, value: usize) {
    println!("S1AB mode={mode} scope={scope} metric={metric} value={value}");
}

fn tp_scope(id: ToolpathId) -> String {
    format!("tp{}", id.0)
}

/// Whether the F-036b adaptive feed-modulation post-pass runs. `true` is the
/// shipped `SimulationOptions` default and the value 0D was captured with;
/// `S1AB_FEED_MODULATION=0` turns it off.
///
/// It is a knob because that post-pass is the **only** code path by which a
/// runtime can move while the emitted geometry is bit-identical: it reads the
/// measured per-move engagement, re-solves the feeds, and swaps the modulated
/// toolpath into `self.results` for the emitter and the next ladder round to
/// read. Turning it off is therefore the control that separates "swept changed
/// the path" from "swept changed the engagement the modulator was fed".
fn feed_modulation_enabled() -> bool {
    match std::env::var("S1AB_FEED_MODULATION") {
        Ok(raw) => !matches!(raw.trim(), "0" | "false" | "off" | "no"),
        Err(_) => true,
    }
}

fn sim_options(resolution: f64) -> SimulationOptions {
    SimulationOptions {
        resolution,
        metrics_enabled: true,
        // Explicit: auto-resolution would silently override the cell size the
        // whole A/B is pinned to.
        auto_resolution: false,
        adaptive_feed_modulation: feed_modulation_enabled(),
        ..Default::default()
    }
}

#[test]
#[ignore = "full-project dexel simulation ladder over wanaka200; run with --ignored --nocapture and RS_CAM_STAMP_DISPATCH set"]
fn swept_wanaka_ab_s1() {
    let mode = mode_tag();
    let raw_env = std::env::var("RS_CAM_STAMP_DISPATCH").unwrap_or_else(|_| "<unset>".to_owned());

    let (path, is_default_project) = project_path();
    let resolution = sim_resolution_mm();
    if !path.exists() {
        println!("SKIPPED: project not found at {}", path.display());
        return;
    }
    if is_default_project && !PathBuf::from(REQUIRED_MODEL).exists() {
        println!("SKIPPED: model referenced by the project not found at {REQUIRED_MODEL}");
        return;
    }

    println!(
        "== S1 swept A/B — {} — mode={mode} (RS_CAM_STAMP_DISPATCH={raw_env}) ==",
        path.display()
    );
    println!("S1AB mode={mode} scope=run metric=env_raw text=\"{raw_env}\"");
    println!(
        "S1AB mode={mode} scope=run metric=project text=\"{}\"",
        path.display()
    );
    num(mode, "run", "requested_resolution_mm", resolution);
    int(
        mode,
        "run",
        "adaptive_feed_modulation",
        usize::from(feed_modulation_enabled()),
    );

    let t_load = Instant::now();
    let mut s = match ProjectSession::load(&path) {
        Ok(s) => s,
        Err(e) => {
            println!("SKIPPED: failed to load {}: {e}", path.display());
            return;
        }
    };
    num(mode, "phase", "load.wall_s", t_load.elapsed().as_secs_f64());

    let cancel = AtomicBool::new(false);

    let n = s.toolpath_count();
    let enabled: Vec<usize> = (0..n)
        .filter(|&i| s.get_toolpath_config(i).is_some_and(|tc| tc.enabled))
        .collect();

    // Index → id/name map, captured before any mutable borrow so the printing
    // pass below never needs the session back.
    let names: Vec<(usize, ToolpathId, String)> = (0..n)
        .filter_map(|i| {
            s.get_toolpath_config(i)
                .map(|tc| (i, tc.id, tc.name.clone()))
        })
        .collect();
    for (idx, id, name) in &names {
        let scope = tp_scope(*id);
        println!("S1AB mode={mode} scope={scope} metric=name index={idx} text=\"{name}\"");
    }
    int(mode, "project", "toolpaths_total", n);
    int(mode, "project", "toolpaths_enabled", enabled.len());

    // ── Pass 1: generate everything that can generate from fresh state ──
    // Rest ops fail hard here by design (F.4) — collected for the ladder.
    let t_gen = Instant::now();
    let mut pending: Vec<usize> = Vec::new();
    let mut generated = 0usize;
    for &i in &enabled {
        if s.generate_toolpath(i, &cancel).is_err() {
            pending.push(i);
        } else {
            generated += 1;
        }
    }
    num(
        mode,
        "phase",
        "generate.wall_s",
        t_gen.elapsed().as_secs_f64(),
    );
    int(mode, "project", "generated_pass1", generated);
    int(mode, "project", "pending_after_pass1", pending.len());

    // The one assertion: a run that generated nothing must not look like a
    // clean measurement.
    assert!(
        generated > 0 || !pending.is_empty(),
        "generation produced no toolpaths at all — the harness measured nothing"
    );

    // ── F.4 ladder: each simulation unlocks the rest ops waiting on it ──
    let mut ladder_rounds = 0usize;
    let ladder_cap = enabled.len() + 2;
    while !pending.is_empty() {
        ladder_rounds += 1;
        if ladder_rounds > ladder_cap {
            println!(
                "S1AB mode={mode} scope=project metric=ladder_converged value=0 still_pending={pending:?}"
            );
            break;
        }
        let t_round = Instant::now();
        if let Err(e) = s.run_simulation(&sim_options(resolution), &cancel) {
            println!(
                "S1AB mode={mode} scope=project metric=ladder_sim_error \
                 round={ladder_rounds} text=\"{e}\""
            );
            break;
        }
        let sim_s = t_round.elapsed().as_secs_f64();
        let t_regen = Instant::now();
        let before = pending.len();
        pending.retain(|&i| s.generate_toolpath(i, &cancel).is_err());
        let regen_s = t_regen.elapsed().as_secs_f64();

        num(
            mode,
            "phase",
            &format!("ladder{ladder_rounds}.sim.wall_s"),
            sim_s,
        );
        num(
            mode,
            "phase",
            &format!("ladder{ladder_rounds}.regen.wall_s"),
            regen_s,
        );
        int(
            mode,
            "phase",
            &format!("ladder{ladder_rounds}.resolved"),
            before - pending.len(),
        );

        if pending.len() >= before {
            println!(
                "S1AB mode={mode} scope=project metric=ladder_converged value=0 stalled_at={ladder_rounds} still_pending={pending:?}"
            );
            break;
        }
    }
    int(mode, "project", "ladder_rounds", ladder_rounds);
    int(mode, "project", "pending_final", pending.len());

    // ── Final simulation over the complete chain ──
    let t_sim = Instant::now();
    if let Err(e) = s.run_simulation(&sim_options(resolution), &cancel) {
        println!("S1AB mode={mode} scope=project metric=final_sim_error text=\"{e}\"");
        return;
    }
    let final_sim_s = t_sim.elapsed().as_secs_f64();
    num(mode, "phase", "final_sim.wall_s", final_sim_s);

    // Snapshot everything off the borrowed result before touching the session
    // again (`diagnostics()` takes its own borrow).
    let (rapid_collisions, total_moves, resolution_clamped, effective_cell_mm, trace) = {
        let Some(sim) = s.simulation_result() else {
            println!(
                "S1AB mode={mode} scope=project metric=final_sim_error \
                 text=\"no simulation result\""
            );
            return;
        };
        (
            sim.rapid_collisions.len(),
            sim.total_moves,
            sim.resolution_clamped,
            sim.column_grid_cell_mm,
            sim.cut_trace.clone(),
        )
    };

    int(mode, "project", "rapid_collisions", rapid_collisions);
    int(mode, "project", "total_moves", total_moves);
    int(
        mode,
        "project",
        "resolution_clamped",
        usize::from(resolution_clamped),
    );
    num(mode, "project", "effective_cell_mm", effective_cell_mm);

    // ── Project + per-toolpath diagnostics ──
    // NOTE: `diagnostics()` runs the holder/shank collision sweep for every
    // computed toolpath (spatial-index build per toolpath). It is timed
    // separately so it never contaminates the simulation wall clock, which is
    // the number the A/B is actually about.
    let t_diag = Instant::now();
    let diag = s.diagnostics();
    num(
        mode,
        "phase",
        "diagnostics.wall_s",
        t_diag.elapsed().as_secs_f64(),
    );

    num(mode, "project", "total_runtime_s", diag.total_runtime_s);
    num(
        mode,
        "project",
        "air_cut_pct_of_total_runtime",
        diag.air_cut_pct_of_total_runtime,
    );
    num(
        mode,
        "project",
        "air_cut_pct_of_cutting_time",
        diag.air_cut_pct_of_cutting_time,
    );
    num(
        mode,
        "project",
        "average_engagement",
        diag.average_engagement,
    );
    int(mode, "project", "collision_count", diag.collision_count);
    int(
        mode,
        "project",
        "diag_rapid_collision_count",
        diag.rapid_collision_count,
    );
    int(mode, "project", "verdict_count", diag.verdicts.len());
    println!(
        "S1AB mode={mode} scope=project metric=verdict text=\"{}\"",
        diag.verdict
    );
    for (k, v) in diag.verdicts.iter().enumerate() {
        println!(
            "S1AB mode={mode} scope=project metric=verdict{k} severity={:?} kind={} text=\"{}\"",
            v.severity,
            v.kind.as_str(),
            v.headline
        );
    }

    for td in &diag.per_toolpath {
        let scope = tp_scope(td.toolpath_id);
        println!(
            "S1AB mode={mode} scope={scope} metric=op_kind text=\"{}\" tool=\"{}\"",
            td.op_kind, td.tool_name
        );
        int(mode, &scope, "move_count", td.move_count);
        int(mode, &scope, "collision_count", td.collision_count);
        int(
            mode,
            &scope,
            "rapid_collision_count",
            td.rapid_collision_count,
        );
        num(mode, &scope, "cutting_distance_mm", td.cutting_distance_mm);
        num(mode, &scope, "rapid_distance_mm", td.rapid_distance_mm);
    }

    // ── Cut-trace metrics ──
    let Some(trace) = trace else {
        println!("S1AB mode={mode} scope=project metric=cut_trace text=\"absent\"");
        return;
    };

    let sum = &trace.summary;
    int(mode, "project", "sample_count", sum.sample_count);
    int(mode, "project", "trace_toolpath_count", sum.toolpath_count);
    num(
        mode,
        "project",
        "trace_total_runtime_s",
        sum.total_runtime_s,
    );
    num(
        mode,
        "project",
        "trace_cutting_runtime_s",
        sum.cutting_runtime_s,
    );
    num(
        mode,
        "project",
        "trace_rapid_runtime_s",
        sum.rapid_runtime_s,
    );
    num(mode, "project", "air_cut_time_s", sum.air_cut_time_s);
    num(
        mode,
        "project",
        "trace_air_cut_pct_of_total_runtime",
        sum.air_cut_pct_of_total_runtime(),
    );
    num(
        mode,
        "project",
        "trace_air_cut_pct_of_cutting_time",
        sum.air_cut_pct_of_cutting_time(),
    );
    num(
        mode,
        "project",
        "trace_average_engagement",
        sum.average_engagement,
    );
    num(mode, "project", "peak_axial_doc_mm", sum.peak_axial_doc_mm);
    num(
        mode,
        "project",
        "peak_plunge_descent_mm",
        sum.peak_plunge_descent_mm,
    );
    num(
        mode,
        "project",
        "peak_chipload_mm_per_tooth",
        sum.peak_chipload_mm_per_tooth,
    );
    num(
        mode,
        "project",
        "total_removed_volume_est_mm3",
        sum.total_removed_volume_est_mm3,
    );
    num(mode, "project", "average_mrr_mm3_s", sum.average_mrr_mm3_s);

    for tp in &trace.toolpath_summaries {
        let scope = tp_scope(tp.toolpath_id);
        int(mode, &scope, "sample_count", tp.sample_count);
        num(mode, &scope, "total_runtime_s", tp.total_runtime_s);
        num(mode, &scope, "cutting_runtime_s", tp.cutting_runtime_s);
        num(mode, &scope, "rapid_runtime_s", tp.rapid_runtime_s);
        num(mode, &scope, "air_cut_time_s", tp.air_cut_time_s);
        num(
            mode,
            &scope,
            "air_cut_pct_of_total_runtime",
            tp.air_cut_pct_of_total_runtime(),
        );
        num(
            mode,
            &scope,
            "air_cut_pct_of_cutting_time",
            tp.air_cut_pct_of_cutting_time(),
        );
        num(
            mode,
            &scope,
            "low_engagement_time_s",
            tp.low_engagement_time_s,
        );
        num(mode, &scope, "average_engagement", tp.average_engagement);
        num(mode, &scope, "peak_axial_doc_mm", tp.peak_axial_doc_mm);
        num(
            mode,
            &scope,
            "peak_plunge_descent_mm",
            tp.peak_plunge_descent_mm,
        );
        num(
            mode,
            &scope,
            "peak_chipload_mm_per_tooth",
            tp.peak_chipload_mm_per_tooth,
        );
        num(
            mode,
            &scope,
            "total_removed_volume_est_mm3",
            tp.total_removed_volume_est_mm3,
        );
        num(mode, &scope, "average_mrr_mm3_s", tp.average_mrr_mm3_s);
        int(
            mode,
            &scope,
            "metrics_not_applicable",
            usize::from(tp.metrics_not_applicable),
        );
    }

    // ── Measurability: which of the above are not measurements ──
    // Printed for every non-`Measurable` row. A `Measurable` row is the
    // default and would triple the line count for no information.
    //
    // `column_grid_cell_mm` is the whole-stock figure; a per-setup grid over a
    // smaller local bbox can be finer. It only ever enriches a reason payload —
    // `from_trace` never lets it change a verdict — so the approximation is
    // reportable, not load-bearing.
    let report = MeasurabilityReport::from_trace(&trace, Some(effective_cell_mm));
    let mut degraded = 0usize;
    let mut not_measurable = 0usize;
    for e in &report.entries {
        let scope = tp_scope(e.toolpath_id);
        let (state, reason) = match e.measurability {
            Measurability::Measurable => continue,
            Measurability::Degraded(r) => {
                degraded += 1;
                ("degraded", r)
            }
            Measurability::NotMeasurable(r) => {
                not_measurable += 1;
                ("not_measurable", r)
            }
        };
        println!(
            "S1AB mode={mode} scope={scope} metric=measurability.{} state={state} reason=\"{}\"",
            e.metric.as_str(),
            reason.describe()
        );
    }
    int(mode, "project", "measurability_degraded_rows", degraded);
    int(
        mode,
        "project",
        "measurability_not_measurable_rows",
        not_measurable,
    );
    int(
        mode,
        "project",
        "measurability_all_measurable",
        usize::from(report.all_measurable()),
    );

    println!("== S1 swept A/B done — mode={mode} final_sim={final_sim_s:.1}s ==");
}
