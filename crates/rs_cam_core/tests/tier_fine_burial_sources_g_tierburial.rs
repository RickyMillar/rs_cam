//! G-TIERBURIAL instrument (`planning/tiered_finish_2026-09-30/RESULTS.md`,
//! "Fine-tier burial"): where the rivmap100 fine tier's fed moves pass under
//! the drop-cutter surface, and whether the simulation shows the gouge.
//!
//! Fine tier, stock Fresh, arcs off. Every fed linear move is sampled every
//! 0.05 mm against the tier tool's own drop-cutter floor. A sample deeper
//! than 0.1 mm is classed as a `FinishingCut` chord longer than 0.25 mm, a
//! shorter one, or another fed move. Each gets a penetration estimate
//! min(vertical depth, the shortest horizontal shift that clears it): at a
//! near-vertical step in the drop-cutter surface the vertical depth
//! overstates the gouge. The `sim` arm simulates the tier alone at 0.1 mm
//! cells and reads `column_deviations`. Other arms are `_`-separated flags
//! that bisect the relink and the dressups: `nohookup`, `norapid`,
//! `nofeedopt`, `nodressups` (`ARMS=sim,nohookup_norapid`,
//! comma-separated). The flag `arcs` keeps the fixture's own arc fitting
//! (F4a); `sim_arcs` simulates with it. The burial samples read linear moves
//! only, so with `arcs` the simulation is the measure of the arcs.
//! `FIND=x,y` prints the moves around the move nearest (x, y) in each arm.
//!
//! `ARMS=sim cargo test --profile release-fast -p rs_cam_core --test
//! tier_fine_burial_sources_g_tierburial -- --ignored --nocapture` (about
//! 3 minutes per arm in release-fast; much longer in debug).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

mod common;

use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::{ArcFitParams, BoundarySource, StockSource};
use rs_cam_core::compute::cutter::build_cutter;
use rs_cam_core::dressup::{EntrySurfaceProbe, OffMeshEntry};
use rs_cam_core::geo::P3;
use rs_cam_core::mesh::SpatialIndex;
use rs_cam_core::session::{
    Command, ProjectSession, SetDressupConfigArgs, SetStockSourceArgs, SetToolpathOperationArgs,
    SimulationOptions,
};
use rs_cam_core::tool::MillingCutter;
use rs_cam_core::toolpath::{MoveIntent, MoveType, Toolpath};

const SAMPLE_MM: f64 = 0.05;
/// A move longer than this in XY is a "long chord".
const LONG_MM: f64 = 0.25;

struct Sample {
    p: P3,
    depth: f64,
    move_index: usize,
    chord: f64,
    intent: MoveIntent,
}

fn buried_samples(tp: &Toolpath, probe: &EntrySurfaceProbe<'_>, min_depth: f64) -> Vec<Sample> {
    let mut out = Vec::new();
    for (i, pair) in tp.moves.windows(2).enumerate() {
        let (prev, m) = (&pair[0], &pair[1]);
        if !matches!(m.move_type, MoveType::Linear { .. }) {
            continue;
        }
        let (a, b) = (prev.target, m.target);
        let chord = (b.x - a.x).hypot(b.y - a.y);
        let len = (chord * chord + (b.z - a.z).powi(2)).sqrt();
        let n = (len / SAMPLE_MM).ceil().max(1.0) as usize;
        for k in 1..=n {
            let t = k as f64 / n as f64;
            let p = P3::new(
                a.x + (b.x - a.x) * t,
                a.y + (b.y - a.y) * t,
                a.z + (b.z - a.z) * t,
            );
            let Some(floor) = probe.floor_z(p.x, p.y) else {
                continue;
            };
            let depth = floor - p.z;
            if depth > min_depth {
                out.push(Sample {
                    p,
                    depth,
                    move_index: i + 1,
                    chord,
                    intent: m.intent,
                });
            }
        }
    }
    out
}

/// The shortest horizontal shift (16 directions, 0.005 mm steps, up to
/// 1 mm) that brings the floor to or below the sample's Z. With the
/// vertical depth it bounds the tool's true penetration from above.
fn lateral_escape(p: P3, probe: &EntrySurfaceProbe<'_>) -> f64 {
    let mut best = f64::INFINITY;
    for k in 0..16 {
        let a = k as f64 * std::f64::consts::PI / 8.0;
        let (dx, dy) = (a.cos(), a.sin());
        let mut s = 0.005;
        while s <= 1.0 && s < best {
            let ok = probe
                .floor_z(p.x + dx * s, p.y + dy * s)
                .is_none_or(|f| f <= p.z);
            if ok {
                best = s;
                break;
            }
            s += 0.005;
        }
    }
    best
}

fn report(label: &str, s: &[Sample], probe: &EntrySurfaceProbe<'_>) -> Vec<(P3, f64, bool)> {
    let mut cls = [(0usize, 0.0f64, 0usize, 0.0f64); 3]; // long cut, short cut, other
    let mut pen = Vec::new();
    let mut other: std::collections::BTreeMap<String, (usize, f64, f64)> = Default::default();
    for x in s {
        let c = if x.intent != MoveIntent::FinishingCut {
            2
        } else if x.chord > LONG_MM {
            0
        } else {
            1
        };
        let lat = lateral_escape(x.p, probe);
        let est = x.depth.min(lat);
        cls[c].0 += 1;
        cls[c].1 = cls[c].1.max(x.depth);
        if est > 0.03 {
            cls[c].2 += 1;
        }
        cls[c].3 = cls[c].3.max(est);
        pen.push((x.p, est, c == 0));
        if c == 2 {
            let e = other
                .entry(format!(
                    "{:?} chord>{LONG_MM}={}",
                    x.intent,
                    x.chord > LONG_MM
                ))
                .or_default();
            e.0 += 1;
            e.1 = e.1.max(x.depth);
            e.2 = e.2.max(est);
        }
    }
    for (k, (n, v, e)) in &other {
        eprintln!("    other: {k}: {n} samples, max vertical {v:.3}, max est {e:.3}");
    }
    // The deepest samples by penetration estimate, one per move.
    let mut by_est: Vec<(f64, &Sample)> = s
        .iter()
        .map(|x| (x.depth.min(lateral_escape(x.p, probe)), x))
        .collect();
    by_est.sort_by(|a, b| b.0.total_cmp(&a.0));
    let mut seen = std::collections::HashSet::new();
    for (est, x) in by_est
        .iter()
        .filter(|(_, x)| seen.insert(x.move_index))
        .take(4)
    {
        eprintln!(
            "    by estimate: #{} {:?} chord {:.3}: est {est:.3}, vertical {:.3} at ({:.3},{:.3},{:.3})",
            x.move_index, x.intent, x.chord, x.depth, x.p.x, x.p.y, x.p.z
        );
    }
    for (c, name) in ["cut chord > 0.25", "cut chord <= 0.25", "other fed"]
        .iter()
        .enumerate()
    {
        eprintln!(
            "  {label}: {name:>18}: {:>6} samples > 0.1 deep, max vertical {:.3}; \
             penetration est min(vert, lateral) > 0.03: {:>6}, max {:.3}",
            cls[c].0, cls[c].1, cls[c].2, cls[c].3
        );
    }
    pen
}

/// The fixture with the fine tier on fresh stock and arcs off, its index,
/// and the arc fitting the fixture emitted (the `arcs` flag restores it).
fn load() -> (ProjectSession, usize, Option<ArcFitParams>) {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../planning/fixtures/rivmap100/rivmap100_tiered_finish.toml");
    let mut session = ProjectSession::load(&path).expect("load");
    let index = session
        .toolpath_configs()
        .iter()
        .position(|tc| {
            tc.planner_origin.as_ref().is_some_and(|o| o.tier == 1)
                && matches!(
                    tc.boundary.source,
                    BoundarySource::PlannedTierRegions { .. }
                )
        })
        .unwrap();
    let _ = session
        .apply(Command::SetStockSource(SetStockSourceArgs {
            index,
            source: StockSource::Fresh,
        }))
        .unwrap();
    let mut dressups = session.get_toolpath_config(index).unwrap().dressups.clone();
    let emitted = dressups.arc_fitting.take();
    let _ = session
        .apply(Command::SetDressupConfig(SetDressupConfigArgs {
            index,
            dressups: Box::new(dressups),
        }))
        .unwrap();
    (session, index, emitted)
}

#[test]
#[ignore = "generates the rivmap100 fine tier per arm (minutes); run with --ignored --nocapture"]
fn burial_classes_stages_and_sim_on_the_rivmap100_fine_tier() {
    let arms = std::env::var("ARMS").unwrap_or_else(|_| "base".into());
    let (mut session, index, _) = load();
    let tc = session.get_toolpath_config(index).unwrap().clone();
    let tool = session
        .tools()
        .iter()
        .find(|t| t.id.0 == tc.tool_id)
        .unwrap()
        .clone();
    let cutter = build_cutter(&tool);
    let mesh = session
        .models()
        .iter()
        .find(|m| m.id == tc.model_id)
        .and_then(|m| m.mesh.clone())
        .unwrap();
    let idx = SpatialIndex::build_auto(&mesh);
    let probe = EntrySurfaceProbe {
        mesh: &mesh,
        index: &idx,
        cutter: &cutter,
        stock_to_leave: 0.0,
        off_mesh: OffMeshEntry::PlungeFallback,
        rest_stock: None,
    };
    let cancel = AtomicBool::new(false);
    // `PROFILE=x`: the model height (0.02 mm probe ball) and the tier
    // tool's drop-cutter floor across the board edge y = 0 at that x.
    if let Some(px) = std::env::var("PROFILE")
        .ok()
        .and_then(|v| v.parse::<f64>().ok())
    {
        let probe_ball = rs_cam_core::tool::BallEndmill::new(0.02, 10.0);
        eprintln!(
            "PROFILE x = {px}: mesh bbox y {:.3}..{:.3}, z {:.3}..{:.3}",
            mesh.bbox.min.y, mesh.bbox.max.y, mesh.bbox.min.z, mesh.bbox.max.z
        );
        for k in 0..=24 {
            let y = -0.1 + k as f64 * 0.05;
            let cl = rs_cam_core::surface::dropcutter::point_drop_cutter(
                px,
                y,
                &mesh,
                &idx,
                &probe_ball,
            );
            let over = rs_cam_core::surface::dropcutter::point_is_over_mesh_xy(px, y, &mesh, &idx);
            eprintln!(
                "  y {y:.2}: model {:.3} (contact {}, over mesh {over}), tool floor {:?}",
                cl.z,
                cl.contacted,
                probe.floor_z(px, y)
            );
        }
    }
    for arm in arms.split(',') {
        let (mut s2, i2, emitted_arcs) = load();
        let s = if arm == "base" || arm == "sim" {
            &mut session
        } else {
            &mut s2
        };
        let index = if arm == "base" || arm == "sim" {
            index
        } else {
            i2
        };
        let tc = s.get_toolpath_config(index).unwrap().clone();
        // An arm is `_`-separated flags: `nohookup` (intra_region_hookup_mm
        // 0), `norapid` (rapid order off), `nofeedopt` (feed optimisation
        // off), `nodressups` (both), `arcs` (the fixture's own arc fitting,
        // F4a), `sim` (simulate the tier).
        let flags: Vec<&str> = arm.split('_').collect();
        let has = |f: &str| flags.contains(&f);
        if has("nodressups") || has("norapid") || has("nofeedopt") || has("arcs") {
            let mut d = tc.dressups.clone();
            if has("arcs") {
                d.arc_fitting = emitted_arcs;
                eprintln!("arm {arm}: arc fitting as the fixture emits it: {emitted_arcs:?}");
            }
            if has("nodressups") || has("norapid") {
                d.optimize_rapid_order = false;
            }
            if has("nodressups") || has("nofeedopt") {
                d.feed_optimization = false;
            }
            let _ = s
                .apply(Command::SetDressupConfig(SetDressupConfigArgs {
                    index,
                    dressups: Box::new(d),
                }))
                .unwrap();
        }
        if has("nohookup") {
            let OperationConfig::UnifiedFinish(mut cfg) = tc.operation.clone() else {
                panic!()
            };
            cfg.intra_region_hookup_mm = 0.0;
            let _ = s
                .apply(Command::SetToolpathOperation(SetToolpathOperationArgs {
                    index,
                    operation: Box::new(OperationConfig::UnifiedFinish(cfg)),
                }))
                .unwrap();
        }
        let t0 = std::time::Instant::now();
        s.generate_toolpath(index, &cancel).unwrap();
        let tp = s.get_result(index).unwrap().toolpath().clone();
        let samples = buried_samples(&tp, &probe, 0.1);
        let max = samples.iter().map(|x| x.depth).fold(0.0, f64::max);
        let machine = s.machine();
        let cycle_s = rs_cam_core::machine::kinematics::compute_cycle_time(
            &tp,
            &machine.kinematics.unwrap_or_default(),
            machine.max_feed_mm_min,
            machine.max_feed_mm_min.max(1.0),
        );
        eprintln!(
            "arm {arm}: {} moves, {} samples > 0.1 mm deep, max {max:.3}; \
             cycle time {cycle_s:.1} s (machine kinematics, rapid = max feed) ({:.0} s)",
            tp.moves.len(),
            samples.len(),
            t0.elapsed().as_secs_f64()
        );
        let mut worst: Vec<&Sample> = samples.iter().collect();
        worst.sort_by(|a, b| b.depth.total_cmp(&a.depth));
        for x in worst.iter().take(6) {
            let m = x.move_index;
            eprintln!(
                "    #{m} {:?} chord {:.3} depth {:.3} at ({:.3},{:.3},{:.3}); prev {:?} {:?}, next {:?}",
                x.intent,
                x.chord,
                x.depth,
                x.p.x,
                x.p.y,
                x.p.z,
                tp.moves[m - 1].intent,
                tp.moves[m - 1].move_type,
                tp.moves.get(m + 1).map(|n| n.intent)
            );
        }
        let pen = report(arm, &samples, &probe);
        // The move with the deepest penetration estimate, in context.
        if let Some(x) = samples.iter().max_by(|a, b| {
            a.depth
                .min(lateral_escape(a.p, &probe))
                .total_cmp(&b.depth.min(lateral_escape(b.p, &probe)))
        }) {
            let m = x.move_index;
            let region = s
                .get_result(index)
                .unwrap()
                .annotated()
                .spans_at(m)
                .filter(|sp| sp.kind == rs_cam_core::trace::toolpath_spans::SpanKind::Region)
                .map(|sp| sp.label.to_string())
                .collect::<Vec<_>>()
                .join("|");
            eprintln!("    deepest estimate: move #{m} in [{region}]");
            for j in m.saturating_sub(3)..(m + 3).min(tp.moves.len()) {
                let mv = &tp.moves[j];
                let f = probe.floor_z(mv.target.x, mv.target.y).unwrap_or(f64::NAN);
                eprintln!(
                    "      [{j}] {:?} {:?} -> ({:.3},{:.3},{:.3}), floor {f:.3}",
                    mv.intent, mv.move_type, mv.target.x, mv.target.y, mv.target.z
                );
            }
        }
        {
            let ann = std::sync::Arc::clone(s.get_result(index).unwrap().annotated());
            let mut by: std::collections::BTreeMap<String, (usize, f64)> = Default::default();
            for x in samples
                .iter()
                .filter(|x| x.intent == MoveIntent::FinishingCut && x.chord > LONG_MM)
            {
                let label = ann
                    .spans_at(x.move_index)
                    .filter(|sp| {
                        sp.kind == rs_cam_core::trace::toolpath_spans::SpanKind::Region
                            && sp.label.contains("band")
                    })
                    .map(|sp| sp.label.to_string())
                    .collect::<Vec<_>>()
                    .join("|");
                let e = by.entry(label).or_default();
                e.0 += 1;
                e.1 = e.1.max(x.depth);
            }
            for (k, (n, d)) in &by {
                eprintln!("    long cut chords in [{k}]: {n} samples > 0.1, max {d:.3}");
            }
        }
        if let Some(w) = worst.first()
            && w.intent != MoveIntent::FinishingCut
        {
            let m = w.move_index;
            for j in m.saturating_sub(6)..(m + 4).min(tp.moves.len()) {
                let mv = &tp.moves[j];
                eprintln!(
                    "      [{j}] {:?} {:?} -> ({:.3},{:.3},{:.3})",
                    mv.intent, mv.move_type, mv.target.x, mv.target.y, mv.target.z
                );
            }
        }
        // `FIND=x,y`: the moves around the move whose target is nearest
        // (x, y), to follow one move through the arms.
        if let Some((fx, fy)) = std::env::var("FIND").ok().and_then(|v| {
            let (a, b) = v.split_once(',')?;
            Some((a.parse::<f64>().ok()?, b.parse::<f64>().ok()?))
        }) {
            let near = tp
                .moves
                .iter()
                .enumerate()
                .min_by(|a, b| {
                    let da = (a.1.target.x - fx).hypot(a.1.target.y - fy);
                    let db = (b.1.target.x - fx).hypot(b.1.target.y - fy);
                    da.total_cmp(&db)
                })
                .map(|(i, _)| i)
                .unwrap();
            eprintln!("  FIND ({fx},{fy}): nearest move {near}");
            for j in near.saturating_sub(8)..(near + 4).min(tp.moves.len()) {
                let mv = &tp.moves[j];
                eprintln!(
                    "      [{j}] {:?} {:?} -> ({:.3},{:.3},{:.3})",
                    mv.intent, mv.move_type, mv.target.x, mv.target.y, mv.target.z
                );
            }
        }
        if !has("sim") {
            continue;
        }
        let skip: Vec<_> = s
            .toolpath_configs()
            .iter()
            .enumerate()
            .filter(|&(i, _)| i != index)
            .map(|(_, tc)| tc.id)
            .collect();
        let cell: f64 = std::env::var("SIM_CELL")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.1);
        let opts = SimulationOptions {
            resolution: cell,
            skip_ids: skip,
            ..Default::default()
        };
        let t0 = std::time::Instant::now();
        s.run_simulation(&opts, &cancel).expect("simulates");
        let sim = s.simulation_result().unwrap();
        let cols = sim.column_deviations.as_ref().unwrap();
        eprintln!(
            "  sim: cell {:.3} mm (asked {cell}), {} columns, {:.0} s",
            sim.column_grid_cell_mm,
            cols.len(),
            t0.elapsed().as_secs_f64()
        );
        let mut devs: Vec<f64> = cols.iter().map(|c| f64::from(c.dev)).collect();
        devs.sort_by(f64::total_cmp);
        let under = |t: f64| devs.iter().filter(|&&d| d < -t).count();
        eprintln!(
            "  sim: min dev {:.3}; columns below -0.03 / -0.05 / -0.1 / -0.2: {} {} {} {}",
            devs[0],
            under(0.03),
            under(0.05),
            under(0.1),
            under(0.2)
        );
        // Min dev within 0.3 mm of each buried sample, by class.
        let grid = cell.max(sim.column_grid_cell_mm);
        let mut near: std::collections::HashMap<(i64, i64), f64> = Default::default();
        for c in cols {
            let key = ((c.x / grid).round() as i64, (c.y / grid).round() as i64);
            let e = near.entry(key).or_insert(f64::INFINITY);
            *e = e.min(f64::from(c.dev));
        }
        let r = (0.3 / grid).ceil() as i64;
        let min_near = |p: P3| {
            let (kx, ky) = ((p.x / grid).round() as i64, (p.y / grid).round() as i64);
            let mut m = f64::INFINITY;
            for dx in -r..=r {
                for dy in -r..=r {
                    if let Some(&d) = near.get(&(kx + dx, ky + dy)) {
                        m = m.min(d);
                    }
                }
            }
            m
        };
        let mut long_min = f64::INFINITY;
        let mut short_min = f64::INFINITY;
        let mut long_n = [0usize; 2];
        let mut short_n = [0usize; 2];
        for (p, _est, long) in &pen {
            let d = min_near(*p);
            if *long {
                long_min = long_min.min(d);
                long_n[0] += 1;
                long_n[1] += usize::from(d < -0.05);
            } else {
                short_min = short_min.min(d);
                short_n[0] += 1;
                short_n[1] += usize::from(d < -0.05);
            }
        }
        eprintln!(
            "  sim near long-chord samples: min dev {long_min:.3}, {} of {} below -0.05",
            long_n[1], long_n[0]
        );
        eprintln!(
            "  sim near short/other samples: min dev {short_min:.3}, {} of {} below -0.05",
            short_n[1], short_n[0]
        );
        // Overcut columns with no buried sample (> 0.02 mm) within 0.5 mm.
        let fine = buried_samples(&tp, &probe, 0.02);
        let mut hash: std::collections::HashSet<(i64, i64)> = Default::default();
        for x in &fine {
            hash.insert(((x.p.x / 0.5).floor() as i64, (x.p.y / 0.5).floor() as i64));
        }
        let near_sample = |x: f64, y: f64| {
            let (kx, ky) = ((x / 0.5).floor() as i64, (y / 0.5).floor() as i64);
            (-1..=1).any(|dx| (-1..=1).any(|dy| hash.contains(&(kx + dx, ky + dy))))
        };
        for t in [0.05, 0.1, 0.2] {
            let over: Vec<_> = cols.iter().filter(|c| f64::from(c.dev) < -t).collect();
            let orphan = over.iter().filter(|c| !near_sample(c.x, c.y)).count();
            eprintln!(
                "  sim: {} columns below -{t}; {orphan} with no buried sample (> 0.02) within ~0.5-1 mm",
                over.len()
            );
        }
        let mut orphans: Vec<_> = cols
            .iter()
            .filter(|c| f64::from(c.dev) < -0.1 && !near_sample(c.x, c.y))
            .collect();
        orphans.sort_by(|a, b| a.dev.total_cmp(&b.dev));
        for c in orphans.iter().take(6) {
            eprintln!("    orphan column ({:.2},{:.2}) dev {:.3}", c.x, c.y, c.dev);
        }
        // Normal-direction overcut: the vertical deviation times the cosine
        // of the model slope at the column (slope from a 0.02 mm probe ball,
        // central differences at 0.05 mm).
        let probe_ball = rs_cam_core::tool::BallEndmill::new(0.02, 10.0);
        let model_z = |x: f64, y: f64| {
            let cl =
                rs_cam_core::surface::dropcutter::point_drop_cutter(x, y, &mesh, &idx, &probe_ball);
            cl.contacted.then_some(cl.z)
        };
        let mut normal: Vec<(f64, f64, f64, f64)> = Vec::new();
        for c in cols.iter().filter(|c| f64::from(c.dev) < -0.03) {
            let h = 0.05;
            let g = match (
                model_z(c.x + h, c.y),
                model_z(c.x - h, c.y),
                model_z(c.x, c.y + h),
                model_z(c.x, c.y - h),
            ) {
                (Some(a), Some(b), Some(cc), Some(d)) => {
                    ((a - b) / (2.0 * h)).hypot((cc - d) / (2.0 * h))
                }
                _ => f64::NAN,
            };
            let n = -f64::from(c.dev) / (1.0 + g * g).sqrt();
            normal.push((c.x, c.y, n, g.atan().to_degrees()));
        }
        let off_mesh = normal.iter().filter(|v| v.2.is_nan()).count();
        normal.retain(|v| !v.2.is_nan());
        normal.sort_by(|a, b| b.2.total_cmp(&a.2));
        let over = |t: f64| normal.iter().filter(|v| v.2 > t).count();
        eprintln!(
            "  sim normal overcut (vertical dev x cos slope): max {:.3}; columns over 0.03 / 0.05 / 0.1: {} {} {} ({off_mesh} columns at the mesh edge skipped)",
            normal.first().map_or(0.0, |v| v.2),
            over(0.03),
            over(0.05),
            over(0.1)
        );
        for v in normal.iter().take(8) {
            eprintln!(
                "    normal overcut ({:.2},{:.2}) {:.3} mm, slope {:.1} deg",
                v.0, v.1, v.2, v.3
            );
        }
        let mut worst_cols: Vec<_> = cols.iter().collect();
        worst_cols.sort_by(|a, b| a.dev.total_cmp(&b.dev));
        // Which move cuts a column: for each column, the lowest the tool's
        // profile reaches over the column centre along every move, against
        // the model height there (0.02 mm probe ball). A move that reaches
        // below the model is a real cut; none is a simulation reading.
        let mut probe_cols: Vec<(f64, f64, f32)> =
            orphans.iter().take(3).map(|c| (c.x, c.y, c.dev)).collect();
        probe_cols.extend(worst_cols.iter().take(2).map(|c| (c.x, c.y, c.dev)));
        let reach = cutter.radius();
        for &(qx, qy, dev) in &probe_cols {
            let Some(mz) = model_z(qx, qy) else {
                eprintln!("  column ({qx:.2},{qy:.2}) dev {dev:.3}: off the mesh");
                continue;
            };
            let mut best = (f64::INFINITY, 0usize);
            for i in 1..tp.moves.len() {
                let (a, b) = (tp.moves[i - 1].target, tp.moves[i].target);
                let len = (b.x - a.x).hypot(b.y - a.y);
                let n = (len / 0.02).ceil().max(1.0) as usize;
                for k in 0..=n {
                    let t = k as f64 / n as f64;
                    let (x, y, z) = (
                        a.x + (b.x - a.x) * t,
                        a.y + (b.y - a.y) * t,
                        a.z + (b.z - a.z) * t,
                    );
                    let d = (x - qx).hypot(y - qy);
                    if d > reach {
                        continue;
                    }
                    if let Some(h) = cutter.height_at_radius(d) {
                        let gap = z + h - mz;
                        if gap < best.0 {
                            best = (gap, i);
                        }
                    }
                }
            }
            let i = best.1;
            let m = &tp.moves[i];
            eprintln!(
                "  column ({qx:.2},{qy:.2}) dev {dev:.3}, model z {mz:.3}: lowest tool point over it \
                 {:.3} mm from the model, move #{i} {:?} {:?} {:?} -> {:?}",
                best.0,
                m.intent,
                m.move_type,
                tp.moves[i - 1].target,
                m.target
            );
            let region = s
                .get_result(index)
                .unwrap()
                .annotated()
                .spans_at(i)
                .filter(|sp| sp.kind == rs_cam_core::trace::toolpath_spans::SpanKind::Region)
                .map(|sp| sp.label.to_string())
                .collect::<Vec<_>>()
                .join("|");
            eprintln!("    in region span [{region}]");
        }
        for (x, y) in [(48.3, 24.6), (65.75, 7.3)] {
            let m = cols
                .iter()
                .filter(|c| (c.x - x).hypot(c.y - y) < 0.5)
                .map(|c| f64::from(c.dev))
                .fold(f64::INFINITY, f64::min);
            eprintln!("  connector site ({x},{y}): min dev within 0.5 mm {m:.3}");
        }
        for c in worst_cols.iter().take(8) {
            let nearest = samples
                .iter()
                .map(|x| ((x.p.x - c.x).hypot(x.p.y - c.y), x.chord))
                .fold((f64::INFINITY, 0.0), |a, b| if b.0 < a.0 { b } else { a });
            eprintln!(
                "    column ({:.2},{:.2}) dev {:.3}; nearest buried sample {:.2} mm away (chord {:.3})",
                c.x, c.y, c.dev, nearest.0, nearest.1
            );
        }
    }
}
