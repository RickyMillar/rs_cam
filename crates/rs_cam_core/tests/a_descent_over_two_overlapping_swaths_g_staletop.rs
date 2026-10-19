//! **Probe: are the Z22 stepover-1.05 rapid collisions real? (G-STALETOP)**
//!
//! The optimizer rebuild (deleted 2026-10-13) read 18 rapid
//! collisions on "Channel BACK_CHANNEL_Z22.00" at depth per pass 1.2222 mm
//! and stepover 1.05 mm, 8 at 2.75 / 1.05, and 0 at every depth per pass at
//! stepover 0.838 mm. This `#[ignore]` evidence run prints every collision
//! record of that toolpath with its neighbour moves, the resolved heights,
//! and the live stock under the rapid. The live stock is the simulator's
//! own prior-stock snapshot of this toolpath, replayed up to the move
//! before the rapid.
//!
//! The run copies `planning/rivmap_block_job/testpiece_mini.toml` of the
//! master checkout into the scratch folder and never saves the project.
//!
//! Verdict (2026-10-12): every hit is a false positive of the HIGH channel
//! of `RapidClearanceCheck::strikes`. The exact scan (no grid) finds no
//! material in the tool disc. One cell row (y 31.8) sits in the 0.1 mm
//! overlap of two ring-2 swaths 2.9 mm apart. The union of the two swaths
//! cuts the row, but no single stamp covers a cell whole, so
//! `conservative_top` keeps the level above. `Z22_PROBE_REGEN=1` shows the
//! count is 0 when the toolpath is generated while a simulation stands.
//!
//! The second test, `a_descent_beside_two_overlapping_swaths_is_not_a_strike`,
//! is the two-pass reproduction. It stays red until the coverage-union fix
//! lands in `dexel_stock/stamping.rs`.
//!
//! Run it (about 160 s; `Z22_PROBE_CELLS` / `Z22_PROBE_SKIP` select cells):
//! `cargo test -p rs_cam_core -q --test a_descent_over_two_overlapping_swaths_g_staletop -- --ignored --nocapture the_z22`

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use rs_cam_core::compute::cutter::build_cutter;
use rs_cam_core::compute::simulate::locate_global_move;
use rs_cam_core::dexel_stock::{StockCutDirection, TriDexelStock};
use rs_cam_core::geo::P3;
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::session::{Command, ProjectSession, SetToolpathParamArgs, SimulationOptions};
use rs_cam_core::stock::collision::RapidClearanceCheck;
use rs_cam_core::stock::radial_profile::{LUT_SAMPLES, RadialProfileLUT};
use rs_cam_core::tool::MillingCutter;
use rs_cam_core::toolpath::{Move, MoveType, Toolpath};

const SOURCE_PROJECT: &str =
    "/home/ricky/personal_repos/rs_cam/planning/rivmap_block_job/testpiece_mini.toml";
const MODEL_DIR: &str = "/home/ricky/cnc_jobs/rivmap_block/testpiece_mini";
const SCRATCH_DIR: &str = "/tmp/claude-1001/-home-ricky-personal-repos-rs-cam/065a14e8-82c6-476f-9331-96d953a0dcb4/scratchpad/z22";
const TOOLPATH_NAME: &str = "Channel BACK_CHANNEL_Z22.00";

/// The cells: (depth per pass, stepover).
const CELLS: [(f64, f64); 3] = [
    (1.222_222_222_222_222_3, 1.05),
    (2.75, 1.05),
    (1.222_222_222_222_222_3, 0.838),
];

/// The sample step (mm) of the exact disc scan.
const EXACT_STEP_MM: f64 = 0.025;

/// The simulation options of the probe: the GUI simulation at the
/// project's stored resolution, with feed modulation on.
fn probe_sim_options(session: &ProjectSession) -> SimulationOptions {
    SimulationOptions {
        resolution: session.simulation_resolution_mm(),
        skip_ids: Vec::new(),
        auto_resolution: false,
        use_predicted_feed_in_gates: false,
        adaptive_feed_modulation: true,
        modulation_strategy:
            rs_cam_core::dressup::feed_modulation::ModulationStrategy::ConstrainedMax,
        modulation_feed_scale: 1.0,
    }
}

/// One rapid collision in both move frames.
#[derive(Clone, Copy)]
struct Attributed {
    global_move: usize,
    toolpath: Option<(ToolpathId, usize)>,
    /// The local index the simulator's own record carries.
    local_in_record: Option<usize>,
}

/// Print one line of the report.
fn line(text: &str) {
    // SAFETY: an `#[ignore]` evidence instrument; its output is the
    // deliverable, and only `--nocapture` shows it.
    #[allow(clippy::print_stderr)]
    {
        eprintln!("{text}");
    }
}

fn kind(m: &Move) -> &'static str {
    match m.move_type {
        MoveType::Rapid => "G0",
        MoveType::Linear { .. } => "G1",
        MoveType::ArcCW { .. } => "G2",
        MoveType::ArcCCW { .. } => "G3",
    }
}

fn p(v: P3) -> String {
    format!("({:.3}, {:.3}, {:.3})", v.x, v.y, v.z)
}

/// The highest ray top in a disc of `radius` around (x, y), and the ray top
/// under the axis.
fn tops_in_disc(stock: &TriDexelStock, x: f64, y: f64, radius: f64) -> (Option<f32>, Option<f32>) {
    let g = &stock.z_grid;
    let axis = g.world_to_cell(x, y).and_then(|(r, c)| g.top_z_at(r, c));
    let cs = g.cell_size;
    let n = (radius / cs).ceil() as i64;
    let mut best: Option<f32> = None;
    for dr in -n..=n {
        for dc in -n..=n {
            let (dx, dy) = (dc as f64 * cs, dr as f64 * cs);
            if dx * dx + dy * dy > radius * radius {
                continue;
            }
            if let Some((r, c)) = g.world_to_cell(x + dx, y + dy)
                && let Some(t) = g.top_z_at(r, c)
            {
                best = Some(best.map_or(t, |b| b.max(t)));
            }
        }
    }
    (best, axis)
}

/// The lowest feed Z the toolpath reached before `upto` within `radius`
/// of (x, y): what the toolpath itself has cut near the point.
fn own_floor_near(tp: &Toolpath, upto: usize, x: f64, y: f64, radius: f64) -> Option<f64> {
    tp.moves[..upto]
        .iter()
        .filter(|m| !matches!(m.move_type, MoveType::Rapid))
        .filter(|m| {
            let (dx, dy) = (m.target.x - x, m.target.y - y);
            dx * dx + dy * dy <= radius * radius
        })
        .map(|m| m.target.z)
        .reduce(f64::min)
}

/// Distance from (px, py) to the XY segment a-b.
fn seg_dist(px: f64, py: f64, a: P3, b: P3) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len_sq = dx * dx + dy * dy;
    let t = if len_sq > 1e-20 {
        (((px - a.x) * dx + (py - a.y) * dy) / len_sq).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (px - (a.x + t * dx)).hypot(py - (a.y + t * dy))
}

/// The exact answer, with no dexel grid: the material that stands above
/// `z_end` inside the open tool disc at (x, y) after moves `[0, upto)`.
///
/// A point stands at the lowest Z of a feed segment (both ends at one Z)
/// whose swept disc of `radius` covers it, else at `stock_top`. The scan
/// samples the disc every [`EXACT_STEP_MM`]. Returns (points that stand above
/// `z_end`, the smallest distance of such a point from the axis, the
/// highest such point's Z).
fn exact_material_in_disc(
    tp: &Toolpath,
    upto: usize,
    x: f64,
    y: f64,
    radius: f64,
    z_end: f64,
    stock_top: f64,
) -> (usize, Option<f64>, Option<f64>) {
    let step = EXACT_STEP_MM;
    let reach = 2.0 * radius + 0.5;
    let segs: Vec<(P3, P3)> = (1..upto)
        .filter(|&j| !matches!(tp.moves[j].move_type, MoveType::Rapid))
        .map(|j| (tp.moves[j - 1].target, tp.moves[j].target))
        .filter(|(a, b)| (a.z - b.z).abs() < 1e-6)
        .filter(|(a, b)| seg_dist(x, y, *a, *b) <= reach)
        .collect();
    let n = (radius / step).ceil() as i64;
    let (mut count, mut nearest, mut highest): (usize, Option<f64>, Option<f64>) = (0, None, None);
    for iy in -n..=n {
        for ix in -n..=n {
            let (px, py) = (x + ix as f64 * step, y + iy as f64 * step);
            let d = (px - x).hypot(py - y);
            if d >= radius - 1e-9 {
                continue;
            }
            let top = segs
                .iter()
                .filter(|(a, b)| seg_dist(px, py, *a, *b) <= radius)
                .map(|(a, _)| a.z)
                .fold(stock_top, f64::min);
            if top > z_end + 1e-6 {
                count += 1;
                nearest = Some(nearest.map_or(d, |v: f64| v.min(d)));
                highest = Some(highest.map_or(top, |v: f64| v.max(top)));
            }
        }
    }
    (count, nearest, highest)
}

/// The Z-grid cells the high channel reads at (x, y) (centre distance at
/// most `radius + cell / 2`) whose `conservative_top` stands above `z`:
/// (centre distance, x, y, ray top, conservative top).
fn high_cells_above(
    stock: &TriDexelStock,
    x: f64,
    y: f64,
    radius: f64,
    z: f64,
) -> Vec<(f64, f64, f64, Option<f32>, f32)> {
    let g = &stock.z_grid;
    let cs = g.cell_size;
    let reach = radius + 0.5 * cs;
    let mut out = Vec::new();
    for row in 0..g.rows {
        let cy = g.origin_v + row as f64 * cs;
        if (cy - y).abs() > reach {
            continue;
        }
        for col in 0..g.cols {
            let cx = g.origin_u + col as f64 * cs;
            let d = (cx - x).hypot(cy - y);
            if d > reach {
                continue;
            }
            let ct = g.conservative_top_at(row, col);
            if f64::from(ct) > z {
                out.push((d, cx, cy, g.top_z_at(row, col), ct));
            }
        }
    }
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    out
}

fn probe_cell(session: &mut ProjectSession, index: usize, dpp: f64, stepover: f64) {
    let t_cell = Instant::now();
    line(&format!(
        "\n===== CELL dpp {dpp:.4} stepover {stepover:.3} ====="
    ));
    for (param, value) in [("depth_per_pass", dpp), ("stepover", stepover)] {
        let _ = session
            .apply(Command::SetToolpathParam(SetToolpathParamArgs {
                index,
                param: param.to_owned(),
                value: serde_json::json!(value),
            }))
            .expect("the param applies");
    }
    let cancel = AtomicBool::new(false);
    session
        .generate_all(&[], &cancel)
        .expect("every enabled toolpath generates");
    let opts = probe_sim_options(session);
    let _ = session
        .run_simulation(&opts, &cancel)
        .expect("the project simulates");

    let tc = session.toolpath_configs()[index].clone();
    let answer = session.toolpath_heights(&tc).expect("the heights resolve");
    let hctx = answer.context;
    let h = answer.heights;
    line(&format!(
        "height context: safe_z {:.3}, stock_top_z {:.3}, stock_bottom_z {:.3}, model_top {:?}, model_bottom {:?}, op_depth {:.3}",
        hctx.safe_z,
        hctx.stock_top_z,
        hctx.stock_bottom_z,
        hctx.model_top_z,
        hctx.model_bottom_z,
        hctx.op_depth
    ));
    line(&format!(
        "resolved: clearance_z {:.3}, retract_z {:.3}, feed_z {:.3}, top_z {:.3}, bottom_z {:.3}",
        h.clearance_z, h.retract_z, h.feed_z, h.top_z, h.bottom_z
    ));

    let tp: Toolpath = session
        .get_result(index)
        .expect("the result")
        .toolpath()
        .clone();
    let n_rapid = tp
        .moves
        .iter()
        .filter(|m| matches!(m.move_type, MoveType::Rapid))
        .count();
    let mut zs: Vec<f64> = tp
        .moves
        .iter()
        .filter(|m| !matches!(m.move_type, MoveType::Rapid))
        .map(|m| (m.target.z * 1000.0).round() / 1000.0)
        .collect();
    zs.sort_by(f64::total_cmp);
    zs.dedup();
    let rapid_zs: Vec<f64> = {
        let mut v: Vec<f64> = tp
            .moves
            .iter()
            .filter(|m| matches!(m.move_type, MoveType::Rapid))
            .map(|m| (m.target.z * 1000.0).round() / 1000.0)
            .collect();
        v.sort_by(f64::total_cmp);
        v.dedup();
        v
    };
    line(&format!(
        "toolpath: {} moves, {} rapids; distinct rapid target Z: {:?}",
        tp.moves.len(),
        n_rapid,
        rapid_zs
    ));
    line(&format!(
        "distinct feed Z count {} (lowest {:?}, highest {:?})",
        zs.len(),
        zs.first(),
        zs.last()
    ));

    let sim = session.simulation_result().expect("the sim result");
    // The one attribution (`compute::simulate::locate_global_move`), the
    // same call `ProjectEvidence::attributed_rapid_collisions` makes.
    let bounds: Vec<_> = sim
        .boundaries
        .iter()
        .map(|b| (b.id, b.start_move, b.end_move))
        .collect();
    let all: Vec<Attributed> = sim
        .rapid_collision_move_indices
        .iter()
        .enumerate()
        .map(|(n, &global_move)| Attributed {
            global_move,
            toolpath: locate_global_move(bounds.iter().copied(), global_move)
                .map(|loc| (loc.toolpath_id, loc.local_move)),
            local_in_record: sim.rapid_collisions.get(n).map(|r| r.move_index),
        })
        .collect();
    line(&format!(
        "project rapid collisions: {}; boundaries: {:?}",
        all.len(),
        sim.boundaries
            .iter()
            .map(|b| (b.id.0, b.name.clone(), b.start_move, b.end_move))
            .collect::<Vec<_>>()
    ));
    let mine: Vec<_> = all
        .iter()
        .filter(|c| matches!(c.toolpath, Some((id, _)) if id == tc.id))
        .cloned()
        .collect();
    line(&format!("this toolpath's collisions: {}", mine.len()));

    let tool_cfg = session
        .get_tool(rs_cam_core::compute::ToolId(tc.tool_id))
        .expect("the tool")
        .clone();
    let cutter = build_cutter(&tool_cfg).unwrap();
    let radius = cutter.radius();
    let lut = RadialProfileLUT::from_cutter(&cutter, LUT_SAMPLES);
    let tol = 0.2 * (std::f64::consts::FRAC_1_SQRT_2 + 0.5 + 1.0);
    let prior = sim.prior_stocks.get(&tc.id).map(|s| (**s).clone());
    line(&format!(
        "tool radius {radius:.3}; prior stock present {}; grid cell {:?}; high-channel tolerance {tol:.4}",
        prior.is_some(),
        prior.as_ref().map(|s| s.z_grid.cell_size)
    ));
    if let Some(s) = prior.as_ref() {
        let (best, _) = tops_in_disc(s, 0.0, 0.0, 0.0);
        line(&format!(
            "prior stock bbox {:?}..{:?}; top at origin {best:?}",
            s.stock_bbox.min, s.stock_bbox.max
        ));
    }

    // Playback-route replay of the WHOLE toolpath with the rapid check, as
    // a cross-check of the metric route's hit list.
    if let Some(s) = prior.as_ref() {
        let mut stock = s.clone();
        let mut check = RapidClearanceCheck::new(&cutter);
        let _ = stock.simulate_toolpath_with_lut_cancel_rapid_checked(
            &tp,
            &lut,
            radius,
            StockCutDirection::FromTop,
            &|| false,
            Some(&mut check),
        );
        line(&format!(
            "playback-route replay hits (local): {:?}",
            check
                .hits()
                .iter()
                .map(|h| h.move_index)
                .collect::<Vec<_>>()
        ));
    }

    // Incremental replay: stock as it stands just before each colliding rapid.
    let mut stock = prior.clone();
    let mut stamped_upto = 1usize; // moves [1, stamped_upto) stamped
    let mut locals: Vec<(usize, usize)> = mine
        .iter()
        .filter_map(|c| {
            if c.toolpath.map(|(_, l)| l) != c.local_in_record {
                line(&format!(
                    "    FRAME MISMATCH: global {} attributed {:?}, record local {:?}",
                    c.global_move, c.toolpath, c.local_in_record
                ));
            }
            c.toolpath.map(|(_, l)| (c.global_move, l))
        })
        .collect();
    locals.sort_by_key(|&(_, l)| l);
    for (k, (global, i)) in locals.iter().copied().enumerate() {
        line(&format!(
            "--- collision {k}: global move {global}, local move {i}"
        ));
        let lo = i.saturating_sub(3);
        let hi = (i + 3).min(tp.moves.len());
        for j in lo..hi {
            let m = &tp.moves[j];
            line(&format!(
                "    {} [{j}] {} {:?} -> {}",
                if j == i { ">>" } else { "  " },
                kind(m),
                m.intent,
                p(m.target)
            ));
        }
        let start = tp.moves[i - 1].target;
        let end = tp.moves[i].target;
        let xy = ((end.x - start.x).powi(2) + (end.y - start.y).powi(2)).sqrt();
        line(&format!(
            "    rapid {} -> {}; xy length {xy:.3}, dz {:.3}; start z - retract_z {:.3}, end z - retract_z {:.3}; start z - stock_top {:.3}",
            p(start),
            p(end),
            end.z - start.z,
            start.z - h.retract_z,
            end.z - h.retract_z,
            start.z - hctx.stock_top_z
        ));
        if let Some(s) = stock.as_mut() {
            if i > stamped_upto {
                let slice = Toolpath {
                    moves: tp.moves[stamped_upto - 1..i].to_vec(),
                };
                let _ = s.simulate_toolpath_with_lut_cancel(
                    &slice,
                    &lut,
                    radius,
                    StockCutDirection::FromTop,
                    &|| false,
                );
                stamped_upto = i;
            }
            let check = RapidClearanceCheck::new(&cutter);
            let strikes = check.strikes(s, start, end, radius);
            line(&format!("    playback-stock re-check strikes: {strikes}"));
            let cells = high_cells_above(s, end.x, end.y, radius, end.z + tol);
            line(&format!(
                "    cells the high channel reads above end Z + tolerance: {} (centre distance, x, y, ray top, conservative top): {:?}",
                cells.len(),
                cells.iter().take(8).collect::<Vec<_>>()
            ));
            // Each such cell's whole square, judged exactly: is every point
            // of it within the tool radius of a feed at or below the rapid's
            // end Z? Then the stale `conservative_top` stands over air.
            let flat: Vec<(P3, P3)> = (1..i)
                .filter(|&j| !matches!(tp.moves[j].move_type, MoveType::Rapid))
                .map(|j| (tp.moves[j - 1].target, tp.moves[j].target))
                .filter(|(a, b)| a.z.max(b.z) <= end.z)
                .collect();
            let cs = s.z_grid.cell_size;
            let mut squares_clear = 0usize;
            let mut covered_by: std::collections::BTreeSet<String> =
                std::collections::BTreeSet::new();
            for &(_, cx, cy, _, _) in &cells {
                let mut all = true;
                for iy in 0..=10 {
                    for ix in 0..=10 {
                        let (qx, qy) = (
                            cx - 0.5 * cs + ix as f64 * cs / 10.0,
                            cy - 0.5 * cs + iy as f64 * cs / 10.0,
                        );
                        match flat
                            .iter()
                            .find(|(a, b)| seg_dist(qx, qy, *a, *b) <= radius)
                        {
                            Some((a, b)) => {
                                let _ = covered_by.insert(format!("{}->{}", p(*a), p(*b)));
                            }
                            None => all = false,
                        }
                    }
                }
                if all {
                    squares_clear += 1;
                }
            }
            line(&format!(
                "    exact: {squares_clear} of {} such cell squares are wholly cut below end Z; covering feeds (first hit per point): {:?}",
                cells.len(),
                covered_by.iter().take(6).collect::<Vec<_>>()
            ));
            if k == 0 {
                // What cut the ground around the axis: every feed before the
                // rapid that passes within the tool radius of it, below the
                // rapid's end Z.
                let near: Vec<usize> = (1..i)
                    .filter(|&j| !matches!(tp.moves[j].move_type, MoveType::Rapid))
                    .filter(|&j| {
                        let (a, b) = (tp.moves[j - 1].target, tp.moves[j].target);
                        a.z.min(b.z) < end.z && seg_dist(end.x, end.y, a, b) <= radius
                    })
                    .collect();
                line(&format!(
                    "    feeds below end Z within r of the axis: {}",
                    near.len()
                ));
                for &j in near.iter().rev().take(12) {
                    let (a, b) = (tp.moves[j - 1].target, tp.moves[j].target);
                    line(&format!(
                        "      [{j}] {} {:?} {} -> {} (axis distance {:.3})",
                        kind(&tp.moves[j]),
                        tp.moves[j].intent,
                        p(a),
                        p(b),
                        seg_dist(end.x, end.y, a, b)
                    ));
                }
            }
            let dist = (xy * xy + (end.z - start.z).powi(2)).sqrt();
            let n = (dist / 1.0).ceil().max(1.0) as usize;
            let mut printed = 0;
            for step in 0..=n {
                let t = step as f64 / n as f64;
                let (px, py, pz) = (
                    start.x + t * (end.x - start.x),
                    start.y + t * (end.y - start.y),
                    start.z + t * (end.z - start.z),
                );
                if let Some((low, high)) =
                    s.clearance_bounds_for_profile(px, py, radius, &cutter, &lut)
                {
                    let low_hit = low.is_some_and(|l| pz < l - 1e-6);
                    let high_hit = pz < high - tol;
                    if low_hit || high_hit || step == 0 || step == n {
                        let (disc_top, axis_top) = tops_in_disc(s, px, py, radius);
                        let own = own_floor_near(&tp, i, px, py, radius);
                        line(&format!(
                            "    sample {step}/{n} at ({px:.3}, {py:.3}, {pz:.3}): low {low:?} high {high:.3} (high-tol {:.3}) low_hit {low_hit} high_hit {high_hit}; ray top max in disc {disc_top:?}, under axis {axis_top:?}; own lowest feed Z within r {own:?}",
                            high - tol
                        ));
                        printed += 1;
                        if printed > 6 {
                            break;
                        }
                    }
                }
            }
        }
    }

    // The margin of every downward rapid against the two channels, on the
    // playback-route stock just before the rapid: the tightest five.
    if let Some(prior) = prior.as_ref() {
        let mut st = prior.clone();
        let mut upto = 1usize;
        let mut margins: Vec<(f64, usize, f64, Option<f64>, f64)> = Vec::new();
        for i in 1..tp.moves.len() {
            let m = &tp.moves[i];
            let a = tp.moves[i - 1].target;
            if !matches!(m.move_type, MoveType::Rapid) || m.target.z >= a.z - 1e-6 {
                continue;
            }
            if i > upto {
                let slice = Toolpath {
                    moves: tp.moves[upto - 1..i].to_vec(),
                };
                let _ = st.simulate_toolpath_with_lut_cancel(
                    &slice,
                    &lut,
                    radius,
                    StockCutDirection::FromTop,
                    &|| false,
                );
                upto = i;
            }
            if let Some((low, high)) =
                st.clearance_bounds_for_profile(m.target.x, m.target.y, radius, &cutter, &lut)
            {
                margins.push((m.target.z - (high - tol), i, m.target.z, low, high));
                // The first level's descents, all of them.
                if m.target.z >= hctx.stock_top_z - dpp + 0.5 - 1e-3 {
                    let next = tp.moves.get(i + 1);
                    line(&format!(
                        "    level-1 descent [{i}] from {} to {}: low {low:?}, high {high:.3}, margin {:.3}; next move {:?} {:?} -> {:?}",
                        p(a),
                        p(m.target),
                        m.target.z - (high - tol),
                        next.map(kind),
                        next.map(|n| n.intent),
                        next.map(|n| p(n.target))
                    ));
                }
            }
        }
        margins.sort_by(|a, b| a.0.total_cmp(&b.0));
        line(&format!(
            "high-channel margin at the descent end (end z - (high - tol)), tightest 6 of {}:",
            margins.len()
        ));
        for (margin, i, z, low, high) in margins.iter().take(6) {
            line(&format!(
                "    [{i}] end {} margin {margin:.3}: end z {z:.3}, low {low:?}, high {high:.3}",
                p(tp.moves[*i].target)
            ));
        }
    }

    // The level-1 pass lines across x 36.5 in the channel window: their Y
    // and the gaps between neighbours (2r = 3.0 mm).
    {
        let l1 = hctx.stock_top_z - dpp;
        let mut ys: Vec<f64> = (1..tp.moves.len())
            .filter(|&j| !matches!(tp.moves[j].move_type, MoveType::Rapid))
            .map(|j| (tp.moves[j - 1].target, tp.moves[j].target))
            .filter(|(a, b)| (a.z - l1).abs() < 1e-6 && (b.z - l1).abs() < 1e-6)
            .filter(|(a, b)| {
                (a.y - b.y).abs() < 1e-6 && a.x.min(b.x) <= 36.5 && a.x.max(b.x) >= 36.5
            })
            .filter(|(a, _)| (28.0..36.0).contains(&a.y))
            .map(|(a, _)| (a.y * 1000.0).round() / 1000.0)
            .collect();
        ys.sort_by(f64::total_cmp);
        ys.dedup();
        let gaps: Vec<String> = ys
            .windows(2)
            .map(|w| format!("{:.3}", w[1] - w[0]))
            .collect();
        line(&format!(
            "level-1 (z {l1:.3}) pass lines across x 36.5, y in 28..36: {ys:?}; gaps {gaps:?}"
        ));
    }

    // Every downward rapid of this toolpath, judged exactly (no grid).
    let struck: std::collections::BTreeSet<usize> = locals.iter().map(|&(_, l)| l).collect();
    let mut real = 0usize;
    let mut descents = 0usize;
    for i in 1..tp.moves.len() {
        let m = &tp.moves[i];
        let a = tp.moves[i - 1].target;
        if !matches!(m.move_type, MoveType::Rapid) || m.target.z >= a.z - 1e-6 {
            continue;
        }
        descents += 1;
        let (count, nearest, highest) = exact_material_in_disc(
            &tp,
            i,
            m.target.x,
            m.target.y,
            radius,
            m.target.z,
            hctx.stock_top_z,
        );
        if count > 0 || struck.contains(&i) {
            if count > 0 {
                real += 1;
            }
            line(&format!(
                "    exact [{i}] {} end {} xy {:.3}: material above end Z in the open disc: {count} points, nearest {nearest:?} mm from the axis, highest {highest:?}; flagged by sim {}",
                if count > 0 { "REAL" } else { "clear" },
                p(m.target),
                (m.target.x - a.x).hypot(m.target.y - a.y),
                struck.contains(&i)
            ));
        }
    }
    line(&format!(
        "exact check: {descents} downward rapids, {real} enter material, {} flagged by the sim",
        struck.len()
    ));
    line(&format!(
        "cell wall time {:.1} s",
        t_cell.elapsed().as_secs_f64()
    ));
}

/// Regenerate the toolpath while a simulation stands, then simulate again.
fn regen_with_the_simulation_standing(session: &mut ProjectSession, index: usize) {
    line(&format!(
        "\n----- REGEN with the simulation standing: {} -----",
        session.simulation_result().is_some()
    ));
    let cancel = AtomicBool::new(false);
    let _ = session
        .generate_toolpath(index, &cancel)
        .expect("the toolpath regenerates");
    let tp: Toolpath = session
        .get_result(index)
        .expect("the result")
        .toolpath()
        .clone();
    for i in 1..tp.moves.len() {
        let m = &tp.moves[i];
        if matches!(m.move_type, MoveType::Rapid)
            && (m.target.x - 37.111).abs() < 0.01
            && (m.target.y - 31.4).abs() < 0.01
            && m.target.z < tp.moves[i - 1].target.z - 1e-6
        {
            let next = tp.moves.get(i + 1);
            line(&format!(
                "    descent [{i}] to {}; next {:?} {:?} -> {:?}",
                p(m.target),
                next.map(kind),
                next.map(|n| n.intent),
                next.map(|n| p(n.target))
            ));
        }
    }
    let opts = probe_sim_options(session);
    let _ = session
        .run_simulation(&opts, &cancel)
        .expect("the project simulates");
    let tc = session.toolpath_configs()[index].clone();
    let sim = session.simulation_result().expect("the sim result");
    let bounds: Vec<_> = sim
        .boundaries
        .iter()
        .map(|b| (b.id, b.start_move, b.end_move))
        .collect();
    let mine = sim
        .rapid_collision_move_indices
        .iter()
        .filter(|&&g| {
            locate_global_move(bounds.iter().copied(), g).is_some_and(|l| l.toolpath_id == tc.id)
        })
        .count();
    line(&format!(
        "    after the regen: this toolpath's collisions {mine}"
    ));
}

#[test]
#[ignore = "evidence instrument: loads the mini project and runs three project simulations"]
fn the_z22_rapid_collisions_at_stepover_1_05() {
    let wall = Instant::now();
    if !Path::new(MODEL_DIR).is_dir() || !Path::new(SOURCE_PROJECT).is_file() {
        line(&format!(
            "PROBE NOT RUN: {MODEL_DIR} or {SOURCE_PROJECT} is missing"
        ));
        return;
    }
    let run_dir: PathBuf = Path::new(SCRATCH_DIR).join(format!("probe-{}", std::process::id()));
    std::fs::create_dir_all(&run_dir).expect("the scratch run folder");
    let project = run_dir.join("testpiece_mini.toml");
    std::fs::copy(SOURCE_PROJECT, &project).expect("copy the project");
    let mut session = ProjectSession::load(&project).expect("the mini project loads");
    let index = session
        .toolpath_configs()
        .iter()
        .position(|tc| tc.name == TOOLPATH_NAME)
        .expect("the Z22 channel is in the project");
    line(&format!("toolpath index {index}"));
    // `Z22_PROBE_CELLS=1` runs the first cell only (a quicker look);
    // `Z22_PROBE_SKIP=2` skips the first two cells.
    let env_usize = |key: &str| {
        std::env::var(key)
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
    };
    let take = env_usize("Z22_PROBE_CELLS").unwrap_or(CELLS.len());
    let skip = env_usize("Z22_PROBE_SKIP").unwrap_or(0);
    for (dpp, stepover) in CELLS.into_iter().skip(skip).take(take) {
        probe_cell(&mut session, index, dpp, stepover);
        // `Z22_PROBE_REGEN=1`: regenerate this toolpath while the simulation
        // stands (the entry dressup then replays on the 0.2 mm prior-stock
        // snapshot, not on its own prism), simulate again, and count.
        if std::env::var("Z22_PROBE_REGEN").is_ok() {
            regen_with_the_simulation_standing(&mut session, index);
        }
    }
    line(&format!(
        "total wall time {:.1} s",
        wall.elapsed().as_secs_f64()
    ));
}

/// Two flat passes at one Z, `gap` mm apart in Y (the Z22 ring-2 sides sit
/// at y 30.35 and 33.25), then a vertical rapid at y 31.4 (the ring-3
/// entry) down to 0.5 mm over the floor, on the project's 0.2 mm grid.
/// Returns whether the live rapid check flags the descent.
fn two_passes_then_descent(gap: f64) -> (bool, usize) {
    const LEVEL: f64 = 23.278;
    let y0 = 30.35;
    let cutter = rs_cam_core::tool::FlatEndmill::new(3.0, 25.0);
    let lut = RadialProfileLUT::from_cutter(&cutter, LUT_SAMPLES);
    let mut stock = TriDexelStock::from_stock(0.0, 0.0, 65.0, 65.0, 0.0, 24.5, 0.2);
    let mut tp = Toolpath::new();
    for y in [y0, y0 + gap] {
        tp.rapid_to(P3::new(26.5, y, 29.5));
        tp.feed_to(P3::new(26.5, y, LEVEL), 1000.0);
        tp.feed_to(P3::new(45.0, y, LEVEL), 1000.0);
        tp.rapid_to(P3::new(45.0, y, 29.5));
    }
    tp.rapid_to(P3::new(37.111, 31.4, 29.5));
    tp.rapid_to(P3::new(37.111, 31.4, LEVEL + 0.5));
    let last = tp.moves.len() - 1;
    let mut check = RapidClearanceCheck::new(&cutter);
    let _ = stock.simulate_toolpath_with_lut_cancel_rapid_checked(
        &tp,
        &lut,
        cutter.radius(),
        StockCutDirection::FromTop,
        &|| false,
        Some(&mut check),
    );
    let strikes = check.hits().iter().any(|h| h.move_index == last);
    // The exact scan on the same moves: its positive control.
    let (exact, _, _) = exact_material_in_disc(&tp, last, 37.111, 31.4, 1.5, LEVEL + 0.5, 24.5);
    (strikes, exact)
}

/// The proposed sentry for the coverage-union residue that the Z22 probe
/// found (the test the owner adds with the fix).
///
/// - Gap 2.9 mm (< 2r): the two swaths overlap by 0.1 mm, so their union
///   cuts every point of the row-31.8 cell squares, but neither stamp
///   covers one square whole. `conservative_top` stays at 24.5 and the
///   high channel flags a descent that enters no material. Must be clear.
/// - Gap 3.1 mm (> 2r): a real 0.1 mm strip stands 1.22 mm tall, 0.45 mm
///   from the axis, inside the footprint. Must strike (the sliver class).
#[test]
#[ignore = "red until the conservative_top coverage-union fix lands (dexel_stock/stamping.rs)"]
fn a_descent_beside_two_overlapping_swaths_is_not_a_strike() {
    let (union_covered, exact_union) = two_passes_then_descent(2.9);
    let (real_strip, exact_strip) = two_passes_then_descent(3.1);
    line(&format!(
        "gap 2.9 (union covers the band) strikes: {union_covered}, exact points in material: {exact_union}; \
         gap 3.1 (a real strip) strikes: {real_strip}, exact points in material: {exact_strip}"
    ));
    assert!(
        exact_strip > 0 && exact_union == 0,
        "the exact scan sees the real strip and nothing over the union"
    );
    assert!(
        real_strip,
        "a real 0.1 mm strip inside the footprint must strike"
    );
    assert!(
        !union_covered,
        "the two swaths cut the whole band between them; a descent over it enters no material"
    );
}
