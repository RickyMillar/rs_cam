//! UNIFIED-TIER-COVERAGE probe (`#[ignore]`, research instrument).
//!
//! Question (`planning/tier_trial_2026-10-01/`, arm T5): why do the two
//! Unified Finish tier ops leave broad flank bands uncut (> 0.3 mm) where an
//! IsoScallop tier ladder does not?
//!
//! The probe loads a tiered project (default
//! `planning/fixtures/rivmap100/rivmap100_tiered_finish.toml`; override
//! with `PROBE_FIXTURE`), walks core's generation plan, runs one closing
//! simulation at 0.25 mm, and then attributes every population column with
//! a deviation above 0.3 mm to:
//!
//! - the band region of each Unified Finish op that contains it (the op's
//!   classification + `decompose` re-run here with the op's own dials; the
//!   op's tier boundary, when it has one, is applied as the shipped op does);
//! - each op's generation findings (`truncated_core_mm2`, ...).
//!
//! Run:
//! `cargo test --release -p rs_cam_core --features test-support
//!  --test unified_tier_coverage_probe -- --ignored --nocapture`

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::cutter::build_cutter;
use rs_cam_core::compute::operation_configs::OpMotion;
use rs_cam_core::compute::sim_prefix::{SimMemo, SimPrefixCache};
use rs_cam_core::finish::finish_planner::{FinishBand, decompose};
use rs_cam_core::finish::finish_setup::build_classification_surface_with_sampler_and_cancel;
use rs_cam_core::finish::unified_finish::unified_finish_classification_resolution;
use rs_cam_core::geo::P2;
use rs_cam_core::geometry::boundary::{model_silhouette, silhouette_machining_outline};
use rs_cam_core::mesh::SpatialIndex;
use rs_cam_core::polygon::{Polygon2, offset_polygon};
use rs_cam_core::session::generation_plan::{self, Scope, Step};
use rs_cam_core::tool::MillingCutter;
use rs_cam_core::session::{
    Command, ProjectSession, SetSimulationResolutionArgs, SimulationOptions, SimulationResolution,
};

const CELL_MM: f64 = 0.25;
const UNCUT_MM: f64 = 0.30;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
#[ignore = "research probe; minutes of release time"]
fn unified_tier_coverage_probe() {
    let fixture = std::env::var("PROBE_FIXTURE").map_or_else(
        |_| repo_root().join("planning/fixtures/rivmap100/rivmap100_tiered_finish.toml"),
        PathBuf::from,
    );
    let out_csv = std::env::var("PROBE_CSV").ok();
    eprintln!("== probe fixture {}", fixture.display());
    let mut session = ProjectSession::load(&fixture).expect("load fixture");
    let mesh = session
        .models()
        .iter()
        .find(|m| m.id == 1)
        .and_then(|m| m.mesh.clone())
        .expect("model 1 mesh");
    eprintln!(
        "   model bbox {:?} .. {:?}",
        mesh.bbox.min, mesh.bbox.max
    );

    let _ = session
        .apply(Command::SetSimulationResolution(SetSimulationResolutionArgs {
            resolution: SimulationResolution::Fixed(CELL_MM),
        }))
        .unwrap();
    let opts = SimulationOptions {
        resolution: CELL_MM,
        ..SimulationOptions::default()
    };
    let cancel = AtomicBool::new(false);
    let mut cache = SimPrefixCache::new();
    for step in &generation_plan::plan(&session, Scope::Project) {
        match *step {
            Step::Simulate { .. } => {
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
                eprintln!("   generated {} in {:.1} s", toolpath.0, t.elapsed().as_secs_f64());
            }
        }
    }
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
    drop(cache);

    // Per-op findings.
    struct UfOp {
        id: usize,
        samples: std::collections::HashMap<(i64, i64), Vec<[f64; 3]>>,
        regions: Vec<(FinishBand, Polygon2)>,
        bounded: bool,
    }
    let mut uf_ops = Vec::new();
    for (idx, tc) in session.toolpath_configs().iter().enumerate() {
        if !tc.enabled {
            continue;
        }
        let r = session.get_result(idx).expect("result");
        let s = &r.stats;
        eprintln!(
            "   op {} '{}' cut {:.0} mm  truncated_core {:?} untouched {:?} reached_uncut_est {:?} monotone {:?} dropped_band {:?} clipped_band {:?}",
            tc.id.0,
            tc.name,
            s.cutting_distance,
            s.truncated_core_mm2,
            s.untouched_material_mm2,
            s.reached_uncut_estimate_mm2,
            s.monotone_cells,
            s.dropped_band.is_some(),
            s.clipped_band.is_some(),
        );
        if let OperationConfig::UnifiedFinish(cfg) = &tc.operation {
            let tool = session
                .tools()
                .iter()
                .find(|t| t.id.0 == tc.tool_id)
                .expect("tool")
                .clone();
            let cutter = build_cutter(&tool);
            let index = SpatialIndex::build_auto(&mesh);
            let params = cfg.params(OpMotion {
                feed_rate: 1000.0,
                plunge_rate: 500.0,
                safe_z: 50.0,
            });
            let planner = cfg.planner_params(cutter.cusp_radius_mm());
            let surface = build_classification_surface_with_sampler_and_cancel(
                &mesh,
                &index,
                &cutter,
                unified_finish_classification_resolution(&cutter, params.tolerance),
                params.classification_sampler,
                &|| false,
            )
            .unwrap();
            let covered = surface.heightmap.covered_flags().to_vec();
            let planned = decompose(&surface.slope_map, &covered, &[], &planner);
            eprintln!(
                "   op {} classification cell {:.3} mm, {} regions; steep {} waterline {} overlap {} z_step {:.3} raster_so {:.3} tol {}",
                tc.id.0,
                surface.cell_size(),
                planned.regions.len(),
                planner.steep_threshold_deg,
                planner.waterline_threshold_deg,
                planner.overlap_mm,
                params.z_step,
                params.raster_stepover,
                params.tolerance,
            );
            for (k, reg) in planned.regions.iter().enumerate() {
                eprintln!(
                    "     region {k} {:?} area {:.1} holes {} verts {}",
                    reg.band,
                    reg.polygon.area(),
                    reg.polygon.holes.len(),
                    reg.polygon.exterior.len()
                );
            }
            // Feed-move samples every 0.2 mm, binned on a 1 mm hash.
            let mut samples = std::collections::HashMap::<(i64, i64), Vec<[f64; 3]>>::new();
            let tp = r.toolpath();
            for w in tp.moves.windows(2) {
                if matches!(w[1].move_type, rs_cam_core::toolpath::MoveType::Rapid) {
                    continue;
                }
                let (a, b) = (w[0].target, w[1].target);
                let len = ((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt();
                let n = (len / 0.2).ceil().max(1.0) as usize;
                for i in 0..=n {
                    let t = i as f64 / n as f64;
                    let q = [a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t, a.z + (b.z - a.z) * t];
                    samples
                        .entry((q[0].floor() as i64, q[1].floor() as i64))
                        .or_default()
                        .push(q);
                }
            }
            if let Ok(dir) = std::env::var("PROBE_DUMP") {
                let mut out = String::new();
                for v in samples.values() {
                    for q in v {
                        out.push_str(&format!("{:.3},{:.3},{:.3}\n", q[0], q[1], q[2]));
                    }
                }
                std::fs::write(format!("{dir}/op{}_feed.csv", tc.id.0), out).unwrap();
            }
            uf_ops.push(UfOp {
                samples,
                id: tc.id.0,
                regions: planned
                    .regions
                    .iter()
                    .map(|r| (r.band, r.polygon.clone()))
                    .collect(),
                bounded: tc.boundary.enabled,
            });
        }
    }

    // Population: the silhouette inset 3 mm (the trial's rule).
    let sil = model_silhouette(&mesh, None);
    let outline = silhouette_machining_outline(&sil).unwrap();
    let pop = offset_polygon(&outline, 3.0);
    let sim = session.simulation_result().unwrap();
    let cols = sim.column_deviations.as_ref().unwrap();
    let cell2 = sim.column_grid_cell_mm * sim.column_grid_cell_mm;
    let mut n_pop = 0usize;
    let mut n_uncut = 0usize;
    let mut uncut_pts = Vec::new();
    for c in cols {
        let p = P2::new(c.x, c.y);
        if !pop.iter().any(|q| q.contains_point(&p)) {
            continue;
        }
        n_pop += 1;
        if f64::from(c.dev) > UNCUT_MM {
            n_uncut += 1;
            uncut_pts.push((c.x, c.y, f64::from(c.dev)));
        }
    }
    eprintln!(
        "== population {:.0} mm2, dev > {UNCUT_MM}: {:.0} mm2 ({:.1} %)",
        n_pop as f64 * cell2,
        n_uncut as f64 * cell2,
        100.0 * n_uncut as f64 / n_pop.max(1) as f64
    );

    // Attribution per op: which band region(s) contain the uncut columns.
    let mut csv = String::from("x,y,dev");
    for op in &uf_ops {
        csv.push_str(&format!(",op{}_band,op{}_region", op.id, op.id));
        let mut by_band = std::collections::BTreeMap::<String, (usize, f64)>::new();
        let mut by_region = std::collections::BTreeMap::<usize, usize>::new();
        for &(x, y, d) in &uncut_pts {
            let p = P2::new(x, y);
            let hit: Vec<usize> = op
                .regions
                .iter()
                .enumerate()
                .filter(|(_, (_, poly))| poly.contains_point(&p))
                .map(|(k, _)| k)
                .collect();
            let key = if hit.is_empty() {
                "none".to_string()
            } else {
                let mut b: Vec<String> =
                    hit.iter().map(|&k| format!("{:?}", op.regions[k].0)).collect();
                b.sort();
                b.dedup();
                b.join("+")
            };
            let e = by_band.entry(key).or_default();
            e.0 += 1;
            e.1 += d;
            for k in hit {
                *by_region.entry(k).or_default() += 1;
            }
        }
        eprintln!("== op {} (boundary enabled {}): uncut columns by band", op.id, op.bounded);
        for (k, (n, sum)) in &by_band {
            eprintln!(
                "     {k:24} {:8.1} mm2  mean dev {:.3}",
                *n as f64 * cell2,
                sum / (*n).max(1) as f64
            );
        }
        // Nearest feed sample of THIS op to each uncut column.
        let mut hist = [0usize; 6]; // <0.5, <1, <2, <3, <5, >=5 (or none within 5)
        let mut zgap = Vec::new();
        for &(x, y, d) in &uncut_pts {
            let (cx, cy) = (x.floor() as i64, y.floor() as i64);
            let mut best = (f64::INFINITY, 0.0);
            for dx in -5..=5 {
                for dy in -5..=5 {
                    if let Some(v) = op.samples.get(&(cx + dx, cy + dy)) {
                        for q in v {
                            let dd = ((q[0] - x).powi(2) + (q[1] - y).powi(2)).sqrt();
                            if dd < best.0 {
                                best = (dd, q[2]);
                            }
                        }
                    }
                }
            }
            let k = if best.0 < 0.5 { 0 } else if best.0 < 1.0 { 1 } else if best.0 < 2.0 { 2 } else if best.0 < 3.0 { 3 } else if best.0 < 5.0 { 4 } else { 5 };
            hist[k] += 1;
            if best.0 < 0.5 {
                zgap.push(d);
            }
        }
        eprintln!(
            "     nearest feed sample of op {} to uncut columns (mm2): <0.5 {:.1}, 0.5-1 {:.1}, 1-2 {:.1}, 2-3 {:.1}, 3-5 {:.1}, >=5 {:.1}",
            op.id,
            hist[0] as f64 * cell2, hist[1] as f64 * cell2, hist[2] as f64 * cell2,
            hist[3] as f64 * cell2, hist[4] as f64 * cell2, hist[5] as f64 * cell2
        );
        let mut top: Vec<_> = by_region.into_iter().collect();
        top.sort_by_key(|&(_, n)| std::cmp::Reverse(n));
        for (k, n) in top.iter().take(12) {
            let (band, poly) = &op.regions[*k];
            eprintln!(
                "     region {k:4} {band:?} area {:9.1}  uncut {:8.1} mm2 ({:.1} % of region)",
                poly.area(),
                *n as f64 * cell2,
                100.0 * *n as f64 * cell2 / poly.area().max(1e-9)
            );
        }
    }
    if let Some(path) = out_csv {
        csv.push('\n');
        for &(x, y, d) in &uncut_pts {
            csv.push_str(&format!("{x:.3},{y:.3},{d:.4}"));
            for op in &uf_ops {
                let p = P2::new(x, y);
                let k = op.regions.iter().position(|(_, poly)| poly.contains_point(&p));
                match k {
                    Some(k) => csv.push_str(&format!(",{:?},{k}", op.regions[k].0)),
                    None => csv.push_str(",none,-1"),
                }
            }
            csv.push('\n');
        }
        std::fs::write(path, csv).unwrap();
    }
}
