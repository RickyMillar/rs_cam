//! PSG-TEMP: G-PLANSIMGAP measurement probe. Remove before any commit.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr,
    clippy::cast_precision_loss
)]

use std::fmt::Write as _;
use std::sync::atomic::AtomicBool;

use rs_cam_core::dexel_stock::{StockCutDirection, TriDexelStock};
use rs_cam_core::session::{ProjectSession, SimulationOptions};
use rs_cam_core::tool::MillingCutter;
use rs_cam_core::toolpath::{MoveType, Toolpath};

fn tops(s: &TriDexelStock) -> Vec<f64> {
    let g = &s.z_grid;
    let floor = s.stock_bbox.min.z;
    let mut v = Vec::with_capacity(g.rows * g.cols);
    for r in 0..g.rows {
        for c in 0..g.cols {
            v.push(g.top_z_at(r, c).map_or(floor, f64::from));
        }
    }
    v
}

fn move_counts(tp: &Toolpath) -> String {
    let (mut r, mut l, mut a) = (0, 0, 0);
    for m in &tp.moves {
        match m.move_type {
            MoveType::Rapid => r += 1,
            MoveType::Linear { .. } => l += 1,
            _ => a += 1,
        }
    }
    format!("moves {} (G0 {r}, G1 {l}, G2/3 {a})", tp.moves.len())
}

struct Stats {
    text: String,
    over: Vec<bool>,
}

/// d = sim - planner (positive: the planner under-reads the stock).
fn stats(label: &str, p: &[f64], s: &[f64], cols: usize, grid: &rs_cam_core::stock::dexel::DexelGrid, floor: f64) -> Stats {
    let mut excluded = 0usize;
    let edges = [-1.0e9, -0.5, -0.25, -0.1, 0.1, 0.25, 0.5, 1.0, 2.0, 1.0e9];
    let mut hist = [0usize; 9];
    let (mut mx, mut mn) = (f64::NEG_INFINITY, f64::INFINITY);
    let (mut mxi, mut mni) = (0, 0);
    let (mut c1, mut c25, mut c5) = (0, 0, 0);
    let (mut n1, mut n25, mut n5) = (0, 0, 0);
    let mut over = vec![false; p.len()];
    for i in 0..p.len() {
        if p[i] <= floor + 1e-6 {
            excluded += 1;
            continue;
        }
        let d = s[i] - p[i];
        for b in 0..9 {
            if d >= edges[b] && d < edges[b + 1] {
                hist[b] += 1;
            }
        }
        if d > mx {
            mx = d;
            mxi = i;
        }
        if d < mn {
            mn = d;
            mni = i;
        }
        if d > 0.1 { c1 += 1; }
        if d > 0.25 { c25 += 1; over[i] = true; }
        if d > 0.5 { c5 += 1; }
        if d < -0.1 { n1 += 1; }
        if d < -0.25 { n25 += 1; }
        if d < -0.5 { n5 += 1; }
    }
    let xy = |i: usize| grid.cell_to_world(i / cols, i % cols);
    let mut t = String::new();
    let _ = writeln!(t, "[{label}] cells {} (excluded planner-empty {excluded})", p.len() - excluded);
    let _ = writeln!(
        t,
        "  hist d=sim-plan: <-0.5 {} | -0.5..-0.25 {} | -0.25..-0.1 {} | +-0.1 {} | 0.1..0.25 {} | 0.25..0.5 {} | 0.5..1 {} | 1..2 {} | >2 {}",
        hist[0], hist[1], hist[2], hist[3], hist[4], hist[5], hist[6], hist[7], hist[8]
    );
    let (ax, ay) = xy(mxi);
    let (bx, by) = xy(mni);
    let _ = writeln!(t, "  max +{mx:.3} at ({ax:.2},{ay:.2}) plan {:.3} sim {:.3}; min {mn:.3} at ({bx:.2},{by:.2})", p[mxi], s[mxi]);
    let _ = writeln!(t, "  sim higher: >0.1 {c1}  >0.25 {c25}  >0.5 {c5};  plan higher: >0.1 {n1}  >0.25 {n25}  >0.5 {n5}");
    Stats { text: t, over }
}

fn replay(prior: &TriDexelStock, tp: &Toolpath, cutter: &dyn MillingCutter) -> TriDexelStock {
    let mut s = prior.clone();
    s.simulate_toolpath(tp, cutter, StockCutDirection::FromTop);
    s
}

#[test]
#[ignore]
fn psg_probe() {
    let arms = std::env::var("PSG_ARMS").unwrap();
    let out_dir = std::env::var("PSG_OUT").unwrap();
    let cancel = AtomicBool::new(false);
    let opts = SimulationOptions {
        resolution: 0.5,
        skip_ids: Vec::new(),
        metrics_enabled: false,
        auto_resolution: false,
        use_predicted_feed_in_gates: false,
        adaptive_feed_modulation: false,
        modulation_strategy:
            rs_cam_core::dressup::feed_modulation::ModulationStrategy::ConstrainedMax,
        modulation_feed_scale: 1.0,
    };
    for arm in arms.split(',') {
        let path = format!("{out_dir}/{arm}.toml");
        let mut rep = String::new();
        let t0 = std::time::Instant::now();
        let mut session = ProjectSession::load(std::path::Path::new(&path)).unwrap();
        session.generate_toolpath(0, &cancel).unwrap();
        session.run_simulation(&opts, &cancel).unwrap();
        rs_cam_core::psg_stash::PATHS.lock().unwrap().clear();
        *rs_cam_core::psg_stash::PLANNER.lock().unwrap() = None;
        session.generate_toolpath(1, &cancel).unwrap();
        let stages: Vec<(String, Toolpath)> =
            std::mem::take(&mut *rs_cam_core::psg_stash::PATHS.lock().unwrap());
        let planner = rs_cam_core::psg_stash::PLANNER.lock().unwrap().take().unwrap();
        session.run_simulation(&opts, &cancel).unwrap();
        let sim = session.simulation_result().unwrap();
        let prior = sim.prior_stocks.get(&rs_cam_core::ToolpathId(1)).cloned().unwrap();
        let tp = session.get_result(1).unwrap().toolpath().clone();
        let tool_cfg = session.tools().iter().find(|t| t.id.0 == 3).unwrap().clone();
        let tool = rs_cam_core::compute::cutter::build_cutter(&tool_cfg);
        let _ = writeln!(rep, "=== arm {arm}  (gen+2 sims {:.1}s)", t0.elapsed().as_secs_f64());
        let _ = writeln!(
            rep,
            "cutter: type {:?} radius {} corner_radius_mm {} tool_cfg.corner_radius {} diameter {}",
            tool_cfg.tool_type, tool.radius(), tool.corner_radius_mm(), tool_cfg.corner_radius, tool_cfg.diameter
        );
        let g = &planner.z_grid;
        let pg = &prior.z_grid;
        let _ = writeln!(
            rep,
            "grid planner {}x{} cs {} o ({},{})  prior {}x{} cs {} o ({},{})",
            g.rows, g.cols, g.cell_size, g.origin_u, g.origin_v, pg.rows, pg.cols, pg.cell_size, pg.origin_u, pg.origin_v
        );
        assert_eq!((g.rows, g.cols), (pg.rows, pg.cols));
        let cols = g.cols;
        let floor = planner.stock_bbox.min.z;
        let pt = tops(&planner);
        // Final emitted toolpath equals the last stage?
        let last = &stages.last().unwrap().1;
        let _ = writeln!(rep, "final result {}; last stage '{}' {}; equal len {}", move_counts(&tp), stages.last().unwrap().0, move_counts(last), tp.moves.len() == last.moves.len());

        // Session checkpoint vs own replay of the final path.
        let final_stock = replay(&prior, &tp, &tool);
        let ft = tops(&final_stock);
        let idx = sim.boundaries.iter().position(|b| b.id == rs_cam_core::ToolpathId(1));
        if let Some(bi) = idx {
            if let Some(cp) = sim.checkpoints.iter().find(|c| c.boundary_index == bi) {
                if cp.stock.z_grid.rows == g.rows && cp.stock.z_grid.cols == g.cols {
                    let ct = tops(&cp.stock);
                    let st = stats("session-sim checkpoint vs own replay (d=ckpt-replay)", &ft, &ct, cols, g, floor);
                    rep.push_str(&st.text);
                    let st = stats("PLANNER vs SESSION SIM", &pt, &ct, cols, g, floor);
                    rep.push_str(&st.text);
                } else {
                    let _ = writeln!(rep, "checkpoint grid differs {}x{}", cp.stock.z_grid.rows, cp.stock.z_grid.cols);
                }
            }
        }
        let fstats = stats("PLANNER vs final path replay", &pt, &ft, cols, g, floor);
        rep.push_str(&fstats.text);

        // Named probe points.
        for (x, y) in [(48.0, 82.5), (48.0, 83.0), (48.0, 83.5), (47.0, 82.5)] {
            if let Some((r, c)) = g.world_to_cell(x, y) {
                let i = r * cols + c;
                let _ = writeln!(rep, "  point ({x},{y}): planner {:.3} final-sim {:.3} prior {:.3}", pt[i], ft[i], tops(&prior)[i]);
            }
        }

        // Each stage: replay the stage path and compare with the planner.
        let mut first_over: Vec<Option<usize>> = vec![None; pt.len()];
        let mut stage_tops = Vec::new();
        for (k, (name, stp)) in stages.iter().enumerate() {
            let s = replay(&prior, stp, &tool);
            let stt = tops(&s);
            let st = stats(&format!("stage {k} '{name}' {}", move_counts(stp)), &pt, &stt, cols, g, floor);
            rep.push_str(&st.text);
            for (i, o) in st.over.iter().enumerate() {
                if *o && first_over[i].is_none() {
                    first_over[i] = Some(k);
                }
            }
            stage_tops.push(stt);
        }
        // Attribution of the final >0.25 cells to the first stage that shows them.
        let mut attr = vec![0usize; stages.len()];
        let mut none = 0;
        for i in 0..pt.len() {
            if fstats.over[i] {
                match first_over[i] {
                    Some(k) => attr[k] += 1,
                    None => none += 1,
                }
            }
        }
        let _ = writeln!(rep, "final >0.25 cells by first stage that shows >0.25:");
        for (k, (name, _)) in stages.iter().enumerate() {
            if attr[k] > 0 {
                let _ = writeln!(rep, "  {k} {name}: {}", attr[k]);
            }
        }
        let _ = writeln!(rep, "  (none earlier) {none}");
        // Wall adjacency of the final >0.25 cells: max planner-top step to an 8-neighbour.
        let rows = g.rows;
        let mut steep = 0;
        let mut flat = 0;
        let mut dump = String::new();
        for i in 0..pt.len() {
            if !fstats.over[i] {
                continue;
            }
            let (r, c) = (i / cols, i % cols);
            let mut step: f64 = 0.0;
            for dr in -1i64..=1 {
                for dc in -1i64..=1 {
                    let (rr, cc) = (r as i64 + dr, c as i64 + dc);
                    if rr < 0 || cc < 0 || rr >= rows as i64 || cc >= cols as i64 {
                        continue;
                    }
                    let j = rr as usize * cols + cc as usize;
                    step = step.max((pt[j] - pt[i]).abs());
                }
            }
            if step > 1.0 { steep += 1; } else { flat += 1; }
            let (x, y) = g.cell_to_world(r, c);
            let _ = write!(dump, "{x:.2},{y:.2},{:.3},{:.3},{step:.3}", pt[i], ft[i]);
            for stt in &stage_tops {
                let _ = write!(dump, ",{:.3}", stt[i]);
            }
            dump.push('\n');
        }
        if let (Some(a), Some(m)) = (
            stages.iter().find(|x| x.0 == "arc_fit").map(|x| &x.1),
            stages.iter().find(|x| x.0 == "segment_merge").map(|x| &x.1),
        ) {
            let mut j = 0usize;
            let mut dropped: Vec<(f64, f64, f64)> = Vec::new();
            let mut max_dev: f64 = 0.0;
            for mv in &a.moves {
                if j < m.moves.len() && m.moves[j].target == mv.target {
                    j += 1;
                    continue;
                }
                if j == 0 || j >= m.moves.len() {
                    continue;
                }
                let (c0, c1) = (m.moves[j - 1].target, m.moves[j].target);
                let (dx, dy, dzc) = (c1.x - c0.x, c1.y - c0.y, c1.z - c0.z);
                let l2 = dx * dx + dy * dy + dzc * dzc;
                let t = if l2 > 0.0 {
                    (((mv.target.x - c0.x) * dx + (mv.target.y - c0.y) * dy + (mv.target.z - c0.z) * dzc) / l2).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let q = (c0.x + dx * t, c0.y + dy * t, c0.z + dzc * t);
                let dev = ((mv.target.x - q.0).powi(2) + (mv.target.y - q.1).powi(2) + (mv.target.z - q.2).powi(2)).sqrt();
                max_dev = max_dev.max(dev);
                dropped.push((mv.target.x, mv.target.y, mv.target.z - q.2));
            }
            let below = dropped.iter().filter(|d| d.2 < -0.1).count();
            let above = dropped.iter().filter(|d| d.2 > 0.1).count();
            let deepest = dropped.iter().map(|d| d.2).fold(0.0, f64::min);
            let highest = dropped.iter().map(|d| d.2).fold(0.0, f64::max);
            let _ = writeln!(rep, "segment_merge dropped points {} (merge matched {j}/{}), max 3D dev {max_dev:.3}; dz<-0.1 (point below chord) {below}, dz>0.1 (point above chord) {above}; min dz {deepest:.3} max dz {highest:.3}", dropped.len(), m.moves.len());
            let merge_k = stages.iter().position(|x| x.0 == "segment_merge");
            let mut bins = [0usize; 5];
            for i in 0..pt.len() {
                if !(fstats.over[i] && first_over[i] == merge_k) {
                    continue;
                }
                let (x, y) = g.cell_to_world(i / cols, i % cols);
                let dmin = dropped.iter().map(|d| (d.0 - x).hypot(d.1 - y)).fold(f64::INFINITY, f64::min);
                let b = if dmin < 1.0 { 0 } else if dmin < 3.0 { 1 } else if dmin < 3.5 { 2 } else if dmin < 6.0 { 3 } else { 4 };
                bins[b] += 1;
            }
            let _ = writeln!(rep, "merge-born >0.25 cells, XY distance to nearest dropped point: <1 {} | 1-3 {} | 3-3.5 {} | 3.5-6 {} | >6 {}", bins[0], bins[1], bins[2], bins[3], bins[4]);
        }
        let _ = writeln!(rep, "final >0.25 cells: neighbour step >1 mm (wall) {steep}, else {flat}");
        std::fs::write(format!("{out_dir}/{arm}_cells.csv"), format!("x,y,plan,final,maxstep,{}\n{dump}", stages.iter().map(|s| s.0.clone()).collect::<Vec<_>>().join(","))).unwrap();
        // Planner/final tops for the whole grid.
        let mut grid_csv = String::from("x,y,plan,final,prior\n");
        let prt = tops(&prior);
        for i in 0..pt.len() {
            let (x, y) = g.cell_to_world(i / cols, i % cols);
            let _ = writeln!(grid_csv, "{x:.2},{y:.2},{:.3},{:.3},{:.3}", pt[i], ft[i], prt[i]);
        }
        std::fs::write(format!("{out_dir}/{arm}_grid.csv"), grid_csv).unwrap();
        // Per-entry clearance: each G0 followed by a feed. Stock = prior plus
        // moves before that G0; clearance = G0 target z - highest safe tip z.
        {
            let mut live = (*prior).clone();
            let mut done = 1usize;
            let mut rows_csv = String::from("move,x,y,z,tipz,clear\n");
            let mut clears = Vec::new();
            for h in 1..tp.moves.len().saturating_sub(1) {
                let is_entry = matches!(tp.moves[h].move_type, MoveType::Rapid)
                    && !matches!(tp.moves[h + 1].move_type, MoveType::Rapid);
                if !is_entry {
                    continue;
                }
                live.simulate_toolpath_range(&tp, &tool, StockCutDirection::FromTop, done, h);
                done = h;
                let t = tp.moves[h].target;
                if let Some(tip) = live.max_clearance_tip_z_for_profile(t.x, t.y, tool.radius(), &tool) {
                    let c = t.z - tip;
                    clears.push(c);
                    let _ = writeln!(rows_csv, "{h},{:.4},{:.4},{:.4},{tip:.4},{c:.4}", t.x, t.y, t.z);
                }
            }
            live.simulate_toolpath_range(&tp, &tool, StockCutDirection::FromTop, done, tp.moves.len());
            let lt = tops(&live);
            let same = lt.iter().zip(ft.iter()).filter(|(a, b)| (*a - *b).abs() > 1e-4).count();
            let minc = clears.iter().copied().fold(f64::INFINITY, f64::min);
            let n = |th: f64| clears.iter().filter(|c| **c < th).count();
            let _ = writeln!(
                rep,
                "entries {}: min clearance {minc:.3}; < 0.5 {} | < 0.4 {} | < 0.25 {} | < 0.1 {} | < 0 {}; incremental vs full replay differing cells {same}",
                clears.len(), n(0.5), n(0.4), n(0.25), n(0.1), n(0.0)
            );
            std::fs::write(format!("{out_dir}/{arm}_entries.csv"), rows_csv).unwrap();
        }
        eprintln!("{rep}");
        std::fs::write(format!("{out_dir}/{arm}_report.txt"), rep).unwrap();
    }
}
