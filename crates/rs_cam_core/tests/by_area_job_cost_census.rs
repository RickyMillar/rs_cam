//! Research probe: where By Area spends its time, job by job, against the
//! same cells in a Global run of the same rough.
//!
//! `planning/by_area_merge_tree_2026-09-25/RESULTS.md`, "Phase 3
//! measurement": on rivmap100 By Area (rest first) is 187 s slower than
//! Global. This probe splits the cycle time of both runs by job.
//!
//! # What it does
//!
//! 1. It loads the project, sets the rough's params (`JOB_COST_SET`, a
//!    comma list of `param=value`), walks the plan to the rough with By Area
//!    and then again with Global.
//! 2. It times every move with the cycle-time integrator the simulation
//!    uses (`compute_cycle_time_breakdown`, the machine's kinematics, its
//!    max feed and the post's rapid feed). A move's time is the difference
//!    of two prefix integrations, so a contiguous range sums exactly.
//! 3. By Area moves belong to the job of their `Adaptive region k` span;
//!    moves after the last job are the cleanup. Global moves belong to the
//!    job whose cell (in the By Area map) is under the move's target XY.
//! 4. Per job it writes: total, cut, entry and air (rapid + link + retract)
//!    time, entry runs, rings (runs of clearing cuts), cut length, and the
//!    rim length: clearing-cut length with both ends within one stepover
//!    of a job boundary that touches a valley.
//!
//! # How to run
//!
//! `JOB_COST_SET="depth_per_pass=8" cargo test -p rs_cam_core -q --test
//! by_area_job_cost_census -- --ignored --nocapture`. `JOB_COST_PROJECT`
//! picks the project (default rivmap100 live); `JOB_COST_OUT` names the CSV.
//! It asserts nothing: it is an instrument.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

use std::collections::{BTreeMap, VecDeque};
use std::fmt::Write as _;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use rs_cam_core::adaptive3d::{AreaRegionKind, AreaRegionMap};
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::machine::kinematics::{CycleTimeBreakdown, compute_cycle_time_breakdown};
use rs_cam_core::session::generation_plan::{Scope, Step, plan};
use rs_cam_core::session::{
    Command, ProjectSession, SetSimulationResolutionArgs, SetToolpathParamArgs, SimulationOptions,
    SimulationResolution,
};
use rs_cam_core::toolpath::{MoveIntent, MoveType, Toolpath};
use rs_cam_core::trace::toolpath_spans::SpanKind;

const DEFAULT_PROJECT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../planning/fixtures/rivmap100/rivmap100_live_0925.toml"
);
const SIM_CELL_MM: f64 = 0.5;

fn walk_to(session: &mut ProjectSession, rough_index: usize) {
    let cancel = AtomicBool::new(false);
    let rough_id = session.toolpath_configs()[rough_index].id;
    for step in plan(session, Scope::Ancestors(rough_id)) {
        match step {
            Step::Simulate { .. } => {
                let opts = SimulationOptions {
                    resolution: SIM_CELL_MM,
                    adaptive_feed_modulation: false,
                    ..SimulationOptions::default()
                };
                let _ = session.run_simulation(&opts, &cancel).expect("simulate");
            }
            Step::Generate { index, .. } => {
                session
                    .generate_toolpath(index, &cancel)
                    .unwrap_or_else(|e| panic!("generate {index}: {e}"));
            }
        }
    }
}

fn set(session: &mut ProjectSession, index: usize, param: &str, value: serde_json::Value) {
    let _ = session
        .apply(Command::SetToolpathParam(SetToolpathParamArgs {
            index,
            param: param.to_owned(),
            value,
        }))
        .unwrap_or_else(|e| panic!("set {param}: {e}"));
}

/// The integrated time of each move: `T(0..=i) - T(0..i)`.
fn move_times(
    tp: &Toolpath,
    session: &ProjectSession,
) -> Vec<CycleTimeBreakdown> {
    let machine = session.machine();
    let kin = machine.kinematics.expect("the machine declares kinematics");
    let max_feed = machine.max_feed_mm_min.max(1.0);
    let post = session.post_config();
    let rapid = if post.high_feedrate_mode {
        post.high_feedrate
    } else {
        max_feed
    };
    let mut prefix = Toolpath::new();
    let mut prev = CycleTimeBreakdown::default();
    let mut out = Vec::with_capacity(tp.moves.len());
    for m in &tp.moves {
        prefix.moves.push(m.clone());
        let b = compute_cycle_time_breakdown(&prefix, &kin, max_feed, rapid);
        out.push(CycleTimeBreakdown {
            total_s: b.total_s - prev.total_s,
            rapid_s: b.rapid_s - prev.rapid_s,
            cutting_s: b.cutting_s - prev.cutting_s,
            entry_s: b.entry_s - prev.entry_s,
            linking_s: b.linking_s - prev.linking_s,
            retract_s: b.retract_s - prev.retract_s,
            unknown_s: b.unknown_s - prev.unknown_s,
        });
        prev = b;
    }
    out
}

fn is_entry(i: MoveIntent) -> bool {
    matches!(
        i,
        MoveIntent::EntryPlunge | MoveIntent::EntryHelix | MoveIntent::EntryRamp | MoveIntent::LeadIn
    )
}

/// The cells within `k` cells (Chebyshev) of a job boundary that touches a
/// valley.
fn rim_band(map: &AreaRegionMap, k: usize) -> Vec<bool> {
    let (rows, cols) = (map.rows, map.cols);
    let valley = |l: u16| {
        map.regions
            .iter()
            .any(|r| r.order == l && r.kind == AreaRegionKind::Valley)
    };
    let mut dist = vec![usize::MAX; rows * cols];
    let mut q = VecDeque::new();
    for r in 0..rows {
        for c in 0..cols {
            let l = map.labels[r * cols + c];
            if l == 0 {
                continue;
            }
            let mut rim = false;
            for dr in -1i64..=1 {
                for dc in -1i64..=1 {
                    let (nr, nc) = (r as i64 + dr, c as i64 + dc);
                    if nr < 0 || nc < 0 || nr >= rows as i64 || nc >= cols as i64 {
                        continue;
                    }
                    let m = map.labels[nr as usize * cols + nc as usize];
                    if m != 0 && m != l && (valley(l) || valley(m)) {
                        rim = true;
                    }
                }
            }
            if rim {
                dist[r * cols + c] = 0;
                q.push_back(r * cols + c);
            }
        }
    }
    while let Some(i) = q.pop_front() {
        let (r, c) = (i / cols, i % cols);
        if dist[i] >= k {
            continue;
        }
        for dr in -1i64..=1 {
            for dc in -1i64..=1 {
                let (nr, nc) = (r as i64 + dr, c as i64 + dc);
                if nr < 0 || nc < 0 || nr >= rows as i64 || nc >= cols as i64 {
                    continue;
                }
                let j = nr as usize * cols + nc as usize;
                if dist[j] == usize::MAX {
                    dist[j] = dist[i] + 1;
                    q.push_back(j);
                }
            }
        }
    }
    dist.iter().map(|&d| d <= k).collect()
}

fn cell(map: &AreaRegionMap, x: f64, y: f64) -> Option<usize> {
    let c = ((x - map.origin_x) / map.cell_mm).round();
    let r = ((y - map.origin_y) / map.cell_mm).round();
    if c < 0.0 || r < 0.0 || c as usize >= map.cols || r as usize >= map.rows {
        return None;
    }
    Some(r as usize * map.cols + c as usize)
}

#[derive(Default, Clone)]
struct Row {
    t: CycleTimeBreakdown,
    entries: usize,
    rings: usize,
    cut_mm: f64,
    rim_mm: f64,
}

/// Fold the moves of `tp` into rows keyed by `job_of(i)`.
fn fold(
    tp: &Toolpath,
    times: &[CycleTimeBreakdown],
    map: &AreaRegionMap,
    band: &[bool],
    job_of: &dyn Fn(usize) -> String,
) -> BTreeMap<String, Row> {
    let mut rows: BTreeMap<String, Row> = BTreeMap::new();
    let in_band = |x: f64, y: f64| cell(map, x, y).is_some_and(|i| band[i]);
    for (i, m) in tp.moves.iter().enumerate() {
        let row = rows.entry(job_of(i)).or_default();
        let t = &times[i];
        row.t.total_s += t.total_s;
        row.t.rapid_s += t.rapid_s;
        row.t.cutting_s += t.cutting_s;
        row.t.entry_s += t.entry_s;
        row.t.linking_s += t.linking_s;
        row.t.retract_s += t.retract_s;
        row.t.unknown_s += t.unknown_s;
        let prev = i.checked_sub(1).map(|p| &tp.moves[p]);
        let prev_intent = prev.map(|p| p.intent);
        if is_entry(m.intent) && !prev_intent.is_some_and(is_entry) {
            row.entries += 1;
        }
        let clearing = m.intent == MoveIntent::ClearingCut && m.move_type != MoveType::Rapid;
        if clearing {
            let prev_clearing = prev.is_some_and(|p| {
                p.intent == MoveIntent::ClearingCut && p.move_type != MoveType::Rapid
            });
            if !prev_clearing {
                row.rings += 1;
            }
            if let Some(p) = prev {
                let len = (m.target - p.target).norm();
                row.cut_mm += len;
                if in_band(p.target.x, p.target.y) && in_band(m.target.x, m.target.y) {
                    row.rim_mm += len;
                }
            }
        }
    }
    rows
}

fn name(map: &AreaRegionMap, order: u16) -> String {
    map.regions
        .iter()
        .find(|r| r.order == order)
        .map_or_else(
            || "none".to_owned(),
            |r| {
                format!(
                    "{} {} ({:.0},{:.0})",
                    r.order,
                    r.kind.as_str(),
                    r.anchor_xy[0],
                    r.anchor_xy[1]
                )
            },
        )
}

#[test]
#[ignore = "research probe; reads planning/fixtures/rivmap100"]
fn by_area_job_cost_against_global() {
    let project = PathBuf::from(
        std::env::var("JOB_COST_PROJECT").unwrap_or_else(|_| DEFAULT_PROJECT.to_owned()),
    );
    let mut session = ProjectSession::load(&project).expect("load project");
    let _ = session
        .apply(Command::SetSimulationResolution(
            SetSimulationResolutionArgs {
                resolution: SimulationResolution::Fixed(SIM_CELL_MM),
            },
        ))
        .expect("a positive cell");
    let rough = session
        .toolpath_configs()
        .iter()
        .position(|tc| tc.enabled && matches!(tc.operation, OperationConfig::Adaptive3d(_)))
        .expect("an enabled 3D Rough");
    if let Ok(sets) = std::env::var("JOB_COST_SET") {
        for spec in sets.split(',').filter(|s| !s.is_empty()) {
            let (param, value) = spec.split_once('=').expect("param=value");
            let value = value
                .parse::<f64>()
                .map_or_else(|_| serde_json::json!(value), |v| serde_json::json!(v));
            set(&mut session, rough, param, value);
        }
    }
    let OperationConfig::Adaptive3d(cfg) = &session.toolpath_configs()[rough].operation else {
        unreachable!();
    };
    let stepover = cfg.stepover;

    set(&mut session, rough, "region_ordering", serde_json::json!("by_area"));
    walk_to(&mut session, rough);
    let area = session.get_result(rough).expect("rough").annotated().clone();
    let map = area.area_regions.clone().expect("By Area records its map");
    set(&mut session, rough, "region_ordering", serde_json::json!("global"));
    walk_to(&mut session, rough);
    let global = session.get_result(rough).expect("rough").annotated().clone();

    let k = (stepover / map.cell_mm).ceil() as usize;
    let band = rim_band(&map, k);

    // By Area: the job of each move from its region span.
    let mut job_of_move = vec![String::from("0 pre"); area.toolpath.moves.len()];
    let mut last_end = 0usize;
    for s in &area.spans {
        if s.kind != SpanKind::Region || s.is_boundary() {
            continue;
        }
        let Some(order) = s
            .label
            .strip_prefix("Adaptive region ")
            .and_then(|n| n.parse::<u16>().ok())
        else {
            continue;
        };
        for slot in job_of_move
            .iter_mut()
            .take(s.end_move.min(area.toolpath.moves.len()))
            .skip(s.start_move)
        {
            *slot = name(&map, order);
        }
        last_end = last_end.max(s.end_move);
    }
    for slot in job_of_move.iter_mut().skip(last_end) {
        *slot = "9 cleanup".to_owned();
    }
    let area_times = move_times(&area.toolpath, &session);
    let area_rows = fold(&area.toolpath, &area_times, &map, &band, &|i| {
        job_of_move[i].clone()
    });

    // Global: the job whose cell is under the move target.
    let global_times = move_times(&global.toolpath, &session);
    let global_rows = fold(&global.toolpath, &global_times, &map, &band, &|i| {
        let m = &global.toolpath.moves[i];
        cell(&map, m.target.x, m.target.y)
            .map(|c| map.labels[c])
            .filter(|&l| l != 0)
            .map_or_else(|| "8 unlabelled".to_owned(), |l| name(&map, l))
    });

    let mut csv = String::from(
        "arm,job,total_s,cut_s,entry_s,air_s,entries,rings,cut_mm,rim_mm\n",
    );
    for (arm, rows) in [("by_area", &area_rows), ("global", &global_rows)] {
        let mut sum = Row::default();
        for (job, r) in rows {
            let air = r.t.rapid_s + r.t.linking_s + r.t.retract_s + r.t.unknown_s;
            let _ = writeln!(
                csv,
                "{arm},{job},{:.1},{:.1},{:.1},{:.1},{},{},{:.0},{:.0}",
                r.t.total_s, r.t.cutting_s, r.t.entry_s, air, r.entries, r.rings, r.cut_mm,
                r.rim_mm
            );
            sum.t.total_s += r.t.total_s;
            sum.t.cutting_s += r.t.cutting_s;
            sum.t.entry_s += r.t.entry_s;
            sum.t.rapid_s += air;
            sum.entries += r.entries;
            sum.rings += r.rings;
            sum.cut_mm += r.cut_mm;
            sum.rim_mm += r.rim_mm;
        }
        let _ = writeln!(
            csv,
            "{arm},TOTAL,{:.1},{:.1},{:.1},{:.1},{},{},{:.0},{:.0}",
            sum.t.total_s,
            sum.t.cutting_s,
            sum.t.entry_s,
            sum.t.rapid_s,
            sum.entries,
            sum.rings,
            sum.cut_mm,
            sum.rim_mm
        );
    }
    let _ = writeln!(
        csv,
        "# rim band: {k} cells ({:.2} mm) of a valley boundary; stepover {stepover}",
        k as f64 * map.cell_mm
    );
    eprint!("{csv}");
    if let Ok(out) = std::env::var("JOB_COST_OUT") {
        std::fs::write(out, &csv).expect("write csv");
    }
}
