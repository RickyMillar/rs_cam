//! Scratch probe (2026-10-02): the T5 arm of the tier trial (UnifiedFinish
//! tiers [R2, R1]) shows columns cut to the stock bottom, charged to the
//! tier-0 op. This walks the rivmap100 tiered fixture the way the CLI does
//! and audits the tier ops' fed moves against the drop-cutter surface.
//!
//! Run: `cargo test --release -p rs_cam_core --test tier_unified_gouge_probe
//! -- --ignored --nocapture`

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::config::StockSource;
use rs_cam_core::compute::cutter::build_cutter;
use rs_cam_core::tool::{BallEndmill, MillingCutter};
use rs_cam_core::compute::sim_prefix::{SimMemo, SimPrefixCache};
use rs_cam_core::dressup::entry_audit::buried_fed_chords;
use rs_cam_core::dressup::{EntrySurfaceProbe, OffMeshEntry};
use rs_cam_core::mesh::SpatialIndex;
use rs_cam_core::session::generation_plan::{self, Scope, Step};
use rs_cam_core::session::{
    Command, ProjectSession, SetSimulationResolutionArgs, SetToolpathEnabledArgs, SetStockSourceArgs, SimulationOptions, SimulationResolution,
};
use rs_cam_core::toolpath::MoveType;

const CELL: f64 = 0.25;

#[test]
#[ignore = "scratch probe: walks the rivmap100 tiered fixture (minutes)"]
fn tier_unified_gouge_probe() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
        std::env::var("PROBE_FIXTURE").unwrap_or_else(|_| {
            "../../planning/fixtures/rivmap100/rivmap100_tiered_finish.toml".into()
        }),
    );
    let mut session = ProjectSession::load(&path).expect("load");
    let _ = session
        .apply(Command::SetSimulationResolution(
            SetSimulationResolutionArgs {
                resolution: SimulationResolution::Fixed(CELL),
            },
        ))
        .unwrap();
    if std::env::var("PROBE_SKIP_T1").is_ok() {
        let i = session
            .toolpath_configs()
            .iter()
            .position(|tc| tc.planner_origin.as_ref().is_some_and(|o| o.tier == 1))
            .unwrap();
        let _ = session
            .apply(Command::SetToolpathEnabled(SetToolpathEnabledArgs {
                index: i,
                enabled: false,
            }))
            .unwrap();
    }
    let gen_only = std::env::var("PROBE_GEN_ONLY").is_ok();
    if gen_only {
        let n = session.toolpath_configs().len();
        for i in 0..n {
            let keep = session.toolpath_configs()[i]
                .planner_origin
                .as_ref()
                .is_some_and(|o| o.tier == 0);
            let _ = session
                .apply(Command::SetToolpathEnabled(SetToolpathEnabledArgs {
                    index: i,
                    enabled: keep,
                }))
                .unwrap();
            if keep {
                let _ = session
                    .apply(Command::SetStockSource(SetStockSourceArgs {
                        index: i,
                        source: StockSource::Fresh,
                    }))
                    .unwrap();
            }
        }
    }
    let opts = SimulationOptions {
        resolution: CELL,
        ..SimulationOptions::default()
    };
    let cancel = AtomicBool::new(false);
    let mut cache = SimPrefixCache::new();
    for step in &generation_plan::plan(&session, Scope::Project) {
        match *step {
            Step::Simulate { .. } => {
                eprintln!("  plan sim");
                session
                    .run_simulation_memoized(
                        &opts,
                        &cancel,
                        Some(SimMemo {
                            cache: &mut cache,
                            store: true,
                        }),
                    )
                    .unwrap();
            }
            Step::Generate { toolpath, index } => {
                let t = std::time::Instant::now();
                session.generate_toolpath(index, &cancel).unwrap();
                eprintln!("  generated {} in {:.1}s", toolpath.0, t.elapsed().as_secs_f64());
            }
        }
    }
    if !gen_only {
        session
            .run_simulation_memoized(
                &opts,
                &cancel,
                Some(SimMemo {
                    cache: &mut cache,
                    store: false,
                }),
            )
            .unwrap();
    }
    drop(cache);

    let stock = session.stock_bbox();
    eprintln!("stock z {:.2}..{:.2}", stock.min.z, stock.max.z);
    let mesh = session
        .models()
        .iter()
        .find(|m| m.id == 1)
        .and_then(|m| m.mesh.clone())
        .unwrap();
    let sidx = SpatialIndex::build_auto(&mesh);

    for (idx, tc) in session.toolpath_configs().iter().enumerate() {
        if !tc.enabled || tc.planner_origin.is_none() {
            continue;
        }
        let res = session.get_result(idx).unwrap();
        let ann = res.annotated();
        let tp = &ann.toolpath;
        let tool = session
            .tools()
            .iter()
            .find(|t| t.id.0 == tc.tool_id)
            .unwrap();
        let cutter = build_cutter(tool);
        let probe = EntrySurfaceProbe {
            mesh: &mesh,
            index: &sidx,
            cutter: &cutter,
            stock_to_leave: 0.0,
            off_mesh: OffMeshEntry::PlungeFallback,
            rest_stock: None,
        };
        let fed_min = tp
            .moves
            .iter()
            .filter(|m| !matches!(m.move_type, MoveType::Rapid))
            .map(|m| m.target.z)
            .fold(f64::INFINITY, f64::min);
        let nonfinite = tp
            .moves
            .iter()
            .filter(|m| !(m.target.x.is_finite() && m.target.y.is_finite() && m.target.z.is_finite()))
            .count();
        let arcs_bad = tp
            .moves
            .iter()
            .filter(|m| match m.move_type {
                MoveType::ArcCW { i, j, .. } | MoveType::ArcCCW { i, j, .. } => {
                    !(i.is_finite() && j.is_finite()) || (i * i + j * j).sqrt() > 50.0
                }
                _ => false,
            })
            .count();
        let fed_lowest = tp
            .moves
            .iter()
            .enumerate()
            .filter(|(_, m)| !matches!(m.move_type, MoveType::Rapid))
            .min_by(|a, b| a.1.target.z.total_cmp(&b.1.target.z))
            .map(|(i, m)| (i, m.target));
        eprintln!("  nonfinite targets {nonfinite}, arcs with nonfinite or r>50 centre {arcs_bad}, lowest fed {fed_lowest:?}");
        let rep = buried_fed_chords(tp, &probe, 0.1, 0.1, |_| true);
        let over1 = rep.iter().filter(|r| r.max_burial_mm > 1.0).count();
        eprintln!(
            "== op {} '{}' moves {} fed min z {:.3}; buried >0.1: {}, >1: {}",
            tc.id.0,
            tc.name,
            tp.moves.len(),
            fed_min,
            rep.len(),
            over1
        );
        let mut sorted = rep.clone();
        sorted.sort_by(|a, b| b.max_burial_mm.total_cmp(&a.max_burial_mm));
        for r in sorted.iter().take(25) {
            let spans: Vec<String> = ann
                .spans
                .iter()
                .filter(|s| s.start_move <= r.move_index && r.move_index < s.end_move)
                .map(|s| format!("{:?}:{}", s.kind, s.label))
                .collect();
            let m = &tp.moves[r.move_index];
            let p = &tp.moves[r.move_index - 1];
            eprintln!(
                "  mv {} {:?} {:?} bur {:.3} at ({:.2},{:.2},{:.3}) from ({:.2},{:.2},{:.3}) to ({:.2},{:.2},{:.3}) chord {:.2} spans {:?}",
                r.move_index,
                r.intent,
                m.move_type,
                r.max_burial_mm,
                r.worst.x,
                r.worst.y,
                r.worst.z,
                p.target.x,
                p.target.y,
                p.target.z,
                m.target.x,
                m.target.y,
                m.target.z,
                r.chord_len_mm,
                spans
            );
        }
        // Low fed moves: below model bbox min.
        let low: Vec<usize> = tp
            .moves
            .iter()
            .enumerate()
            .filter(|(_, m)| !matches!(m.move_type, MoveType::Rapid) && m.target.z < mesh.bbox.min.z - 0.5)
            .map(|(i, _)| i)
            .collect();
        // Cone hypothesis: the same audit against a plain R2 ball floor.
        let ball = BallEndmill::new(cutter.cusp_radius_mm() * 2.0, 25.0);
        let ball_probe = EntrySurfaceProbe {
            mesh: &mesh,
            index: &sidx,
            cutter: &ball,
            stock_to_leave: 0.0,
            off_mesh: OffMeshEntry::PlungeFallback,
            rest_stock: None,
        };
        let rep_ball = buried_fed_chords(tp, &ball_probe, 0.1, 0.1, |_| true);
        eprintln!(
            "  vs plain ball floor (d {}): buried >0.1: {}, >1: {}, max {:.3}",
            cutter.cusp_radius_mm() * 2.0,
            rep_ball.len(),
            rep_ball.iter().filter(|r| r.max_burial_mm > 1.0).count(),
            rep_ball.iter().map(|r| r.max_burial_mm).fold(0.0, f64::max)
        );
        let mut by_span = std::collections::BTreeMap::<String, (usize, f64)>::new();
        for r in &rep {
            let key = ann
                .spans
                .iter()
                .filter(|s| s.start_move <= r.move_index && r.move_index < s.end_move)
                .filter(|s| !matches!(s.kind, rs_cam_core::trace::toolpath_spans::SpanKind::Operation))
                .map(|s| format!("{:?}:{}", s.kind, s.label.split(' ').next().unwrap_or("")))
                .collect::<Vec<_>>()
                .join("/");
            let e = by_span.entry(format!("{:?} {key}", r.intent)).or_default();
            e.0 += 1;
            e.1 = e.1.max(r.max_burial_mm);
        }
        for (k, (n, mx)) in &by_span {
            eprintln!("  by stage: {k}: {n} (max {mx:.3})");
        }
        eprintln!("  fed moves below model min z - 0.5: {} (first {:?})", low.len(), &low[..low.len().min(10)]);
    }

    if gen_only {
        let hwm = std::fs::read_to_string("/proc/self/status")
            .ok()
            .and_then(|s| s.lines().find(|l| l.starts_with("VmHWM")).map(str::to_owned));
        eprintln!("{hwm:?}");
        return;
    }
    let sim = session.simulation_result().unwrap();
    let cols = sim.column_deviations.as_ref().unwrap();
    let deep: Vec<_> = cols.iter().filter(|c| c.dev < -1.0).collect();
    let mid = cols.iter().filter(|c| c.dev < -0.1).count();
    let min = cols.iter().map(|c| c.dev).fold(f32::INFINITY, f32::min);
    eprintln!(
        "sim cols {} dev<-0.1: {} dev<-1: {} min dev {:.3}",
        cols.len(),
        mid,
        deep.len(),
        min
    );
    for c in deep.iter().step_by((deep.len() / 20).max(1)) {
        eprintln!("  deep col ({:.2},{:.2}) dev {:.3} top {:.3}", c.x, c.y, c.dev, c.top_z);
    }
    eprintln!("rapid collisions {}", sim.rapid_collision_move_indices.len());
    let hwm = std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| s.lines().find(|l| l.starts_with("VmHWM")).map(str::to_owned));
    eprintln!("{hwm:?}");
}
