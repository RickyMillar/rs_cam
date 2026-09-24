#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

use std::sync::atomic::AtomicBool;

use rs_cam_core::session::SimulationOptions;
use rs_cam_core::session::ProjectSession;
use rs_cam_core::toolpath::MoveType;

#[test]
#[ignore]
fn probe() {
    let path = std::env::var("PROBE_TOML").unwrap();
    let res: f64 = std::env::var("PROBE_RES").map_or(0.5, |s| s.parse().unwrap());
    let mut session = ProjectSession::load(std::path::Path::new(&path)).unwrap();
    let cancel = AtomicBool::new(false);
    let opts = SimulationOptions {
        resolution: res,
        skip_ids: Vec::new(),
        metrics_enabled: true,
        auto_resolution: false,
        use_predicted_feed_in_gates: false,
        adaptive_feed_modulation: true,
        modulation_strategy:
            rs_cam_core::dressup::feed_modulation::ModulationStrategy::ConstrainedMax,
        modulation_feed_scale: 1.0,
    };
    session.generate_toolpath(0, &cancel).unwrap();
    session.run_simulation(&opts, &cancel).unwrap();
    if let Err(e) = session.generate_toolpath(1, &cancel) { panic!("gen1: {e:?}"); }
    session.run_simulation(&opts, &cancel).unwrap();
    let sim = session.simulation_result().unwrap();
    for rc in &sim.rapid_collisions {
        eprintln!("HIT move {} start {:?} end {:?}", rc.move_index, rc.start, rc.end);
    }
    let hits: Vec<usize> = sim.rapid_collisions.iter().map(|r| r.move_index).collect();
    let prior = sim
        .prior_stocks
        .get(&rs_cam_core::ToolpathId(1))
        .cloned();
    let tp = session.get_result(1).unwrap().toolpath().clone();
    eprintln!("tp1 moves {}", tp.moves.len());
    let tool = Box::new(rs_cam_core::tool::FlatEndmill::new(6.0, 25.0));
    let r = 3.0;
    let mut hits = hits;
    if hits.is_empty() {
        for (i, m) in tp.moves.iter().enumerate() {
            if matches!(m.move_type, MoveType::Rapid) && (m.target.x - 51.0).abs() < 0.3 && (m.target.y - 83.35).abs() < 0.3 && m.target.z < 10.0 {
                eprintln!("NEAR rapid {i} {:?}", m.target);
                hits.push(i);
            }
        }
    }
    for &h in &hits {
        let lo = h.saturating_sub(25);
        for i in lo..(h + 6).min(tp.moves.len()) {
            let m = &tp.moves[i];
            let kind = match m.move_type {
                MoveType::Rapid => "G0".to_owned(),
                MoveType::Linear { feed_rate } => format!("G1 f{feed_rate:.0}"),
                MoveType::ArcCW { .. } => "G2".to_owned(),
                MoveType::ArcCCW { .. } => "G3".to_owned(),
            };
            eprintln!(
                "{i:6} {kind:10} {:?} ({:.3},{:.3},{:.3})",
                m.intent, m.target.x, m.target.y, m.target.z
            );
        }
        let start = tp.moves[h - 1].target;
        let end = tp.moves[h].target;
        // Planner-independent reading: the stock before TP1 (after Face) and
        // the live stock after moves 0..h of TP1.
        if let Some(prior) = prior.as_ref() {
            let mut live = (**prior).clone();
            live.simulate_toolpath_range(
                &tp,
                tool.as_ref(),
                rs_cam_core::dexel_stock::StockCutDirection::FromTop,
                1,
                h,
            );
            {
                let g = &live.z_grid;
                let cs = g.cell_size;
                eprintln!("cell {cs} origin ({},{})", g.origin_u, g.origin_v);
                for row in 0..g.rows {
                    for col in 0..g.cols {
                        let (x, y) = g.cell_to_world(row, col);
                        let d = ((x - end.x).powi(2) + (y - end.y).powi(2)).sqrt();
                        if d > 4.5 { continue; }
                        let ct = g.conservative_top_at(row, col);
                        let t = g.top_z_at(row, col);
                        let pt = prior.z_grid.top_z_at(row, col);
                        if f64::from(ct) > end.z - 0.4 {
                            eprintln!("  cell ({x:.2},{y:.2}) d {d:.2} ctop {ct:.3} top {t:?} prior {pt:?}");
                        }
                    }
                }
            }
            if let Ok(fine_cs) = std::env::var("PROBE_FINE") {
                let fine_cs: f64 = fine_cs.parse().unwrap();
                let mut fine = rs_cam_core::dexel_stock::TriDexelStock::from_bounds(&session.stock_bbox(), fine_cs);
                let tp0 = session.get_result(0).unwrap().toolpath().clone();
                fine.simulate_toolpath(&tp0, tool.as_ref(), rs_cam_core::dexel_stock::StockCutDirection::FromTop);
                fine.simulate_toolpath_range(&tp, tool.as_ref(), rs_cam_core::dexel_stock::StockCutDirection::FromTop, 1, h);
                let g = &fine.z_grid;
                let mut worst = (f64::NEG_INFINITY, 0.0, 0.0, 0.0);
                for row in 0..g.rows { for col in 0..g.cols {
                    let (x, y) = g.cell_to_world(row, col);
                    let d = ((x - end.x).powi(2) + (y - end.y).powi(2)).sqrt();
                    if d > 3.0 { continue; }
                    if let Some(t) = g.top_z_at(row, col) {
                        let t = f64::from(t);
                        if t > worst.0 { worst = (t, d, x, y); }
                        if t > end.z { eprintln!("  FINE above-tip cell ({x:.3},{y:.3}) d {d:.3} top {t:.3} ctop {:.3}", g.conservative_top_at(row, col)); }
                    }
                }}
                eprintln!("FINE cs {fine_cs}: max point top inside r<=3 = {:.3} at d {:.3} ({:.3},{:.3}); rapid tip z {:.3}; clearance(r=3) {:?}",
                    worst.0, worst.1, worst.2, worst.3, end.z,
                    fine.max_clearance_tip_z_for_profile(end.x, end.y, r, tool.as_ref()));
            }
            let n = 20;
            for s in 0..=n {
                let t = s as f64 / n as f64;
                let p = rs_cam_core::geo::P3::new(start.x + (end.x - start.x) * t, start.y + (end.y - start.y) * t, start.z + (end.z - start.z) * t);
                let pre = prior.max_clearance_tip_z_for_profile(p.x, p.y, r, tool.as_ref());
                let now = live.max_clearance_tip_z_for_profile(p.x, p.y, r, tool.as_ref());
                let axis_pre = prior
                    .z_grid
                    .world_to_cell(p.x, p.y)
                    .and_then(|(a, b)| prior.z_grid.top_z_at(a, b));
                let axis_now = live
                    .z_grid
                    .world_to_cell(p.x, p.y)
                    .and_then(|(a, b)| live.z_grid.top_z_at(a, b));
                eprintln!(
                    "t {t:.2} ({:.3},{:.3},{:.3}) clear_pre {pre:?} clear_live {now:?} axis_pre {axis_pre:?} axis_live {axis_now:?}",
                    p.x, p.y, p.z
                );
            }
        }
    }
}
