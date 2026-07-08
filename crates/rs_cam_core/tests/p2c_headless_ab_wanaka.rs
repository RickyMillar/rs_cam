//! P2.c A/B checkpoint #1 (build order, `planning/unified_finish_planner_design.md`):
//! the UnifiedFinish op (naive per-band concatenation, no router) vs the
//! P1-optimized stack's "3D Finish 6" drop-cutter raster, on the full
//! wanaka chain, both branches measured in the same process.
//!
//! Branch A: the unmodified chain (re-measured, not the stale 8920 s
//! constant). Branch B: identical chain with "3D Finish 6"'s operation
//! swapped IN PLACE to UnifiedFinish via `set_toolpath_operation` — same
//! tool, heights, boundary, dressups, and (critically) the same position in
//! the machining order, so the downstream R2 pencil still references the
//! stock the finish pass leaves.
//!
//! Quality parity dials for B: raster_stepover matches A's 0.3 mm;
//! scallop_height = 0.3²/(8·3) ≈ 0.004 mm (the flat-surface cusp A's
//! stepover produces on the Ø6 ball — scallop then holds that cusp ON the
//! slope, where A's cusp degrades by 1/cos); z_step 0.3 mm (wall spacing
//! parity). B's quality is ≥ A's by construction; the checkpoint question
//! is what that costs (or saves) in integrator time.
//!
//! `#[ignore]` — two full-project dexel simulation ladders (many minutes).
//! Run with:
//! `cargo test -p rs_cam_core --test p2c_headless_ab_wanaka --release -- --ignored --nocapture`

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::indexing_slicing
)]

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::{
    DropCutterConfig, ScallopConfig, ScallopDirection, UnifiedFinishConfig,
};
use rs_cam_core::session::{ProjectSession, SimulationOptions};

/// Pre-existing full-chain collision count (P0/P1 baseline, 2026-07-07).
const BASELINE_RAPID_COLLISIONS: usize = 4;
const FINISH_OP_NAME: &str = "3D Finish 6";

fn wanaka_project_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("planning")
        .join("airrun_2026-06-01")
        .join("wanaka.toml")
}

struct ChainOutcome {
    project_total_s: f64,
    /// Sum over every op named [`FINISH_OP_NAME`] — branch C splits the
    /// finish pass into two same-named ops, so this is a += accumulation.
    finish_total_s: f64,
    collisions: usize,
    /// Dexel-estimated removed volume, finish op(s) only (mm³).
    finish_removed_mm3: f64,
    /// Dexel-estimated removed volume, whole project (mm³).
    project_removed_mm3: f64,
}

/// Generate + F.4 ladder + final GUI-options simulation, print the per-op
/// intent table (+ removed volume) and the final stock-vs-model deviation
/// stats, return the totals. Mirrors `p1_headless_ab_wanaka.rs`.
fn run_chain(label: &str, s: &mut ProjectSession) -> ChainOutcome {
    let cancel = AtomicBool::new(false);
    let n = s.toolpath_count();
    let enabled: Vec<usize> = (0..n)
        .filter(|&i| s.get_toolpath_config(i).is_some_and(|tc| tc.enabled))
        .collect();

    // Pass 1: rest ops fail hard from fresh state by design (F.4).
    let mut pending: Vec<usize> = Vec::new();
    for &i in &enabled {
        if s.generate_toolpath(i, &cancel).is_err() {
            pending.push(i);
        }
    }

    // F.4 ladder: each simulation unlocks the first pending rest op.
    let mut ladder_rounds = 0usize;
    while !pending.is_empty() {
        ladder_rounds += 1;
        assert!(
            ladder_rounds <= enabled.len() + 2,
            "[{label}] ladder failed to converge; still pending: {pending:?}"
        );
        s.run_simulation(&SimulationOptions::default(), &cancel)
            .expect("ladder simulation");
        let before = pending.len();
        pending.retain(|&i| s.generate_toolpath(i, &cancel).is_err());
        assert!(
            pending.len() < before,
            "[{label}] ladder made no progress at round {ladder_rounds}; still pending: {pending:?}"
        );
    }

    let final_opts = SimulationOptions {
        adaptive_feed_modulation: true,
        modulation_strategy: rs_cam_core::feed_modulation::ModulationStrategy::ConstrainedMax,
        modulation_aggressiveness: 1.0,
        ..Default::default()
    };
    s.run_simulation(&final_opts, &cancel)
        .expect("final simulation");
    let sim = s.simulation_result().expect("sim result");
    let trace = sim.cut_trace.as_ref().expect("cut trace");

    eprintln!("== P2.c A/B [{label}] ==");
    eprintln!(
        "rapid_collisions={} (baseline {BASELINE_RAPID_COLLISIONS})",
        sim.rapid_collisions.len()
    );
    let mut finish_total_s = 0.0f64;
    let mut finish_removed_mm3 = 0.0f64;
    for tp in &trace.toolpath_summaries {
        let name = (0..n)
            .filter_map(|i| s.get_toolpath_config(i))
            .find(|tc| tc.id == tp.toolpath_id)
            .map(|tc| tc.name.clone())
            .unwrap_or_else(|| format!("{:?}", tp.toolpath_id));
        if name == FINISH_OP_NAME {
            finish_total_s += tp.total_runtime_s;
            finish_removed_mm3 += tp.total_removed_volume_est_mm3;
        }
        match tp.runtime_by_intent {
            Some(b) => eprintln!(
                "op={name:<22} total={:8.1}s cutting={:8.1} entry={:8.1} linking={:6.1} rapid={:7.1} retract={:5.1} unknown={:7.1} removed={:9.0}mm3",
                tp.total_runtime_s,
                b.cutting_s,
                b.entry_s,
                b.linking_s,
                b.rapid_s,
                b.retract_s,
                b.unknown_s,
                tp.total_removed_volume_est_mm3
            ),
            None => eprintln!(
                "op={name:<22} total={:8.1}s removed={:9.0}mm3 (no runtime_by_intent — kinematics off?)",
                tp.total_runtime_s, tp.total_removed_volume_est_mm3
            ),
        }
    }
    let project_total_s = trace.summary.total_runtime_s;
    let project_removed_mm3 = trace.summary.total_removed_volume_est_mm3;
    if let Some(p) = trace.summary.runtime_by_intent {
        eprintln!(
            "PROJECT total={:8.1}s cutting={:8.1} entry={:8.1} linking={:6.1} rapid={:7.1} retract={:5.1} unknown={:7.1} removed={:9.0}mm3",
            project_total_s,
            p.cutting_s,
            p.entry_s,
            p.linking_s,
            p.rapid_s,
            p.retract_s,
            p.unknown_s,
            project_removed_mm3
        );
    }

    // Final stock vs model: per-vertex `sim_z − model_z` from the sim's
    // deviation pass (positive = leftover material, negative = overcut;
    // 0.0 = vertex not relevant, e.g. stock bottom). This is the actual
    // machined-surface quality measure the cusp math only predicts.
    match sim.deviations.as_ref() {
        Some(devs) => {
            const EPS: f32 = 1e-4;
            let mut leftover_n = 0usize;
            let mut leftover_sum = 0.0f64;
            let mut leftover_max = 0.0f32;
            let mut gouge_n = 0usize;
            let mut gouge_min = 0.0f32;
            for &d in devs {
                if d > EPS {
                    leftover_n += 1;
                    leftover_sum += f64::from(d);
                    leftover_max = leftover_max.max(d);
                } else if d < -EPS {
                    gouge_n += 1;
                    gouge_min = gouge_min.min(d);
                }
            }
            let leftover_mean = leftover_sum / (leftover_n as f64).max(1.0);
            eprintln!(
                "DEVIATION verts={} leftover: n={leftover_n} mean={leftover_mean:.4}mm max={leftover_max:.4}mm | overcut: n={gouge_n} worst={gouge_min:.4}mm",
                devs.len()
            );
        }
        None => eprintln!("DEVIATION unavailable (sim ran without a reference model mesh)"),
    }

    ChainOutcome {
        project_total_s,
        finish_total_s,
        collisions: sim.rapid_collisions.len(),
        finish_removed_mm3,
        project_removed_mm3,
    }
}

// ── P2.f fidelity instrument ────────────────────────────────────────────
//
// The band-fidelity defect (user-caught, 2026-07-08) hid inside the blunt
// project-wide leftover MEAN twice. This instrument makes it impossible to
// miss again: every stock-mesh vertex's deviation is attributed to the
// planner band that owns its XY (the same decomposition the unified op
// cuts from), histogrammed with BOTH signs (beheaded detail = OVERCUT,
// which the old leftover-only stats never surfaced), and rendered as a
// top-down deviation map PNG so the "smooshed mountains" pattern is
// visible without the GUI.

/// Per-cell band ownership rasterized from the planner's conditioned
/// regions onto the classification grid. Code 0 = no region (off-model or
/// unclassified), 1 = Shallow, 2 = MidSteep, 3 = VerySteep. Band-overlap
/// cells attribute to the STEEPER band (rasterized in ascending steepness,
/// later overwrites).
struct BandMap {
    origin_x: f64,
    origin_y: f64,
    cell: f64,
    rows: usize,
    cols: usize,
    codes: Vec<u8>,
}

const BAND_NAMES: [&str; 4] = ["off-region", "shallow", "mid-steep", "very-steep"];

impl BandMap {
    fn code_at(&self, x: f64, y: f64) -> u8 {
        let col = ((x - self.origin_x) / self.cell).round();
        let row = ((y - self.origin_y) / self.cell).round();
        if col < 0.0 || row < 0.0 || col >= self.cols as f64 || row >= self.rows as f64 {
            return 0;
        }
        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
        let idx = row as usize * self.cols + col as usize;
        self.codes[idx]
    }
}

fn p2f_output_dir() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("p2f_fidelity");
    std::fs::create_dir_all(&dir).expect("create target/p2f_fidelity");
    dir
}

/// Build the band map with the SAME classification + decomposition the
/// unified op runs (Ø6 ball, `for_tool(3.0)`, overlap 2.0, locked 45/75
/// thresholds), so deviations are attributed to the regions the op
/// actually routed. Branch A is scored against the same map — the
/// comparison question is "what did each strategy's territory look like
/// under A vs under B", so the territory must be identical.
fn build_band_map(s: &ProjectSession) -> BandMap {
    use rs_cam_core::finish_planner::{FinishBand, FinishPlannerParams, decompose};
    use rs_cam_core::finish_setup::build_classification_surface_with_cancel;
    use rs_cam_core::geo::P2;
    use rs_cam_core::mesh::SpatialIndex;
    use rs_cam_core::region_set::RegionSet;
    use rs_cam_core::tool::BallEndmill;

    let mesh = s
        .models()
        .iter()
        .find_map(|m| m.mesh.clone())
        .expect("wanaka terrain mesh");
    let index = SpatialIndex::build(&mesh, 10.0);
    let cutter = BallEndmill::new(6.0, 25.0);
    let never = || false;
    let surface = build_classification_surface_with_cancel(&mesh, &index, &cutter, 0.05, &never)
        .expect("classification surface");
    let mut planner = FinishPlannerParams::for_tool(3.0);
    planner.overlap_mm = 2.0;
    let planned = decompose(
        &surface.slope_map,
        &surface.heightmap.covered,
        &[],
        3.0,
        &planner,
    );

    let hm = &surface.heightmap;
    let (rows, cols, cell) = (hm.rows, hm.cols, hm.cell_size);
    let mut codes = vec![0u8; rows * cols];
    for (band, code) in [
        (FinishBand::Shallow, 1u8),
        (FinishBand::MidSteep, 2u8),
        (FinishBand::VerySteep, 3u8),
    ] {
        let polys: Vec<_> = planned
            .regions
            .iter()
            .filter(|r| r.band == band)
            .map(|r| r.polygon.clone())
            .collect();
        if polys.is_empty() {
            continue;
        }
        let rs = RegionSet::new(polys);
        for r in 0..rows {
            for c in 0..cols {
                let x = hm.origin_x + c as f64 * cell;
                let y = hm.origin_y + r as f64 * cell;
                if rs.contains(&P2::new(x, y)) {
                    codes[r * cols + c] = code;
                }
            }
        }
    }

    let counts = codes.iter().fold([0usize; 4], |mut acc, &c| {
        acc[c as usize] += 1;
        acc
    });
    eprintln!(
        "BAND MAP {rows}x{cols} @ {cell:.3}mm: off-region={} shallow={} mid-steep={} very-steep={}",
        counts[0], counts[1], counts[2], counts[3]
    );

    BandMap {
        origin_x: hm.origin_x,
        origin_y: hm.origin_y,
        cell,
        rows,
        cols,
        codes,
    }
}

/// Save the band map itself as a PNG (top-down; gray = off-region, green =
/// shallow, orange = mid-steep, red = very-steep) so deviation maps can be
/// read against the territory that produced them.
fn save_band_map_png(bm: &BandMap, path: &std::path::Path) {
    let mut px = vec![0u8; bm.rows * bm.cols * 4];
    for r in 0..bm.rows {
        for c in 0..bm.cols {
            let code = bm.codes[r * bm.cols + c];
            let (rr, gg, bb) = match code {
                1 => (40u8, 140u8, 60u8),
                2 => (220u8, 140u8, 30u8),
                3 => (200u8, 40u8, 40u8),
                _ => (60u8, 60u8, 60u8),
            };
            // Image row 0 is the TOP of the picture; grid row 0 is min-Y.
            let ir = bm.rows - 1 - r;
            let i = (ir * bm.cols + c) * 4;
            px[i] = rr;
            px[i + 1] = gg;
            px[i + 2] = bb;
            px[i + 3] = 255;
        }
    }
    image::save_buffer(
        path,
        &px,
        bm.cols as u32,
        bm.rows as u32,
        image::ColorType::Rgba8,
    )
    .expect("save band map png");
    eprintln!("band map png: {}", path.display());
}

/// Histogram bin edges (mm). Deviations below the first edge land in bin
/// 0; at or above the last edge in the final bin. Negative = OVERCUT
/// (beheaded detail), positive = leftover material.
const DEV_EDGES: [f32; 12] = [
    -0.5, -0.3, -0.2, -0.1, -0.05, -0.01, 0.01, 0.05, 0.1, 0.2, 0.3, 0.5,
];
const DEV_BIN_COUNT: usize = DEV_EDGES.len() + 1;
const DEV_BIN_LABELS: [&str; DEV_BIN_COUNT] = [
    "<-.5", "-.5", "-.3", "-.2", "-.1", "-.05", "on-size", "+.05", "+.1", "+.2", "+.3", "+.5",
    ">+.5",
];

#[derive(Default, Clone, Copy)]
struct BandAcc {
    bins: [usize; DEV_BIN_COUNT],
    leftover_n: usize,
    leftover_sum: f64,
    leftover_max: f32,
    overcut_n: usize,
    overcut_sum: f64,
    overcut_min: f32,
}

/// Re-simulate the chain at measurement resolution (0.25 mm — the default
/// 0.5 mm dexel grid aliases away exactly the 0.3–1 mm terrain texture the
/// band-fidelity defect beheads). Runs AFTER `run_chain` so the reported
/// times/collisions still come from the standard options the pinned-A
/// constants were measured with; this sim exists only for `deviations`.
fn run_measurement_sim(s: &mut ProjectSession) {
    let cancel = AtomicBool::new(false);
    let opts = SimulationOptions {
        resolution: 0.25,
        ..Default::default()
    };
    s.run_simulation(&opts, &cancel)
        .expect("hi-res measurement simulation");
}

/// Per-band deviation histogram + top-down deviation PNG for the CURRENT
/// simulation result on `s`. Call after `run_chain` (and, for measurement
/// sensitivity, after [`run_measurement_sim`]). Prints a table, writes
/// `{tag}_deviation.png` (red = overcut/gouge, blue = leftover, gray =
/// on-size, black = no data) and a raw little-endian f32 grid dump
/// (`{tag}_deviation_{rows}x{cols}.f32`, signed max-|dev| per image cell,
/// NaN = no data) so cross-run diff maps can be computed offline.
fn fidelity_report(tag: &str, s: &ProjectSession, bm: &BandMap) {
    let sim = s.simulation_result().expect("sim result");
    let devs = sim
        .deviations
        .as_ref()
        .expect("deviations (sim ran without a reference model mesh?)");
    let verts = &sim.mesh.vertices;
    assert_eq!(verts.len(), devs.len() * 3, "vertex/deviation mismatch");

    let bin_of = |d: f32| -> usize {
        DEV_EDGES
            .iter()
            .position(|&e| d < e)
            .unwrap_or(DEV_BIN_COUNT - 1)
    };

    let mut accs = [BandAcc::default(); 4];
    // Signed max-|dev| per IMAGE cell for the picture + raw dump. The image
    // grid is 2× finer than the band map so the measurement sim's 0.25 mm
    // vertices don't alias back away.
    let icell = bm.cell * 0.5;
    let icols = bm.cols * 2;
    let irows = bm.rows * 2;
    let mut img: Vec<f32> = vec![0.0; irows * icols];
    let mut img_hit: Vec<bool> = vec![false; irows * icols];

    const EPS: f32 = 1e-4;
    for (i, &d) in devs.iter().enumerate() {
        if d == 0.0 {
            continue; // sentinel: vertex not relevant (stock bottom etc.)
        }
        let x = f64::from(verts[i * 3]);
        let y = f64::from(verts[i * 3 + 1]);
        let code = bm.code_at(x, y) as usize;
        let a = &mut accs[code];
        a.bins[bin_of(d)] += 1;
        if d > EPS {
            a.leftover_n += 1;
            a.leftover_sum += f64::from(d);
            a.leftover_max = a.leftover_max.max(d);
        } else if d < -EPS {
            a.overcut_n += 1;
            a.overcut_sum += f64::from(d);
            a.overcut_min = a.overcut_min.min(d);
        }

        let col = ((x - bm.origin_x) / icell).round();
        let row = ((y - bm.origin_y) / icell).round();
        if col >= 0.0 && row >= 0.0 && col < icols as f64 && row < irows as f64 {
            #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
            let idx = row as usize * icols + col as usize;
            if !img_hit[idx] || d.abs() > img[idx].abs() {
                img[idx] = d;
                img_hit[idx] = true;
            }
        }
    }

    eprintln!("== P2.f FIDELITY [{tag}] (negative = overcut/beheaded, positive = leftover) ==");
    eprintln!(
        "{:<11} | {:>9} {:>9} {:>8} | {:>9} {:>9} {:>8} | histogram",
        "band", "over_n", "over_mean", "worst", "left_n", "left_mean", "max"
    );
    for (code, acc) in accs.iter().enumerate() {
        let over_mean = acc.overcut_sum / (acc.overcut_n as f64).max(1.0);
        let left_mean = acc.leftover_sum / (acc.leftover_n as f64).max(1.0);
        let hist: Vec<String> = DEV_BIN_LABELS
            .iter()
            .zip(acc.bins.iter())
            .map(|(l, n)| format!("{l}:{n}"))
            .collect();
        eprintln!(
            "{:<11} | {:>9} {:>9.4} {:>8.4} | {:>9} {:>9.4} {:>8.4} | {}",
            BAND_NAMES[code],
            acc.overcut_n,
            over_mean,
            acc.overcut_min,
            acc.leftover_n,
            left_mean,
            acc.leftover_max,
            hist.join(" ")
        );
    }

    // Deviation picture: saturate the color ramp at ±0.3 mm (the texture
    // scale) so mid-range damage is visible, not just the extremes.
    let mut px = vec![0u8; irows * icols * 4];
    for r in 0..irows {
        for c in 0..icols {
            let idx = r * icols + c;
            let (rr, gg, bb) = if !img_hit[idx] {
                (0u8, 0u8, 0u8)
            } else {
                let d = img[idx];
                if d < -EPS {
                    let t = (f64::from(-d) / 0.3).min(1.0);
                    // gray -> red
                    let g = (110.0 * (1.0 - t)) as u8;
                    ((110.0 + 145.0 * t) as u8, g, g)
                } else if d > EPS {
                    let t = (f64::from(d) / 0.3).min(1.0);
                    // gray -> blue
                    let g = (110.0 * (1.0 - t)) as u8;
                    (g, g, (110.0 + 145.0 * t) as u8)
                } else {
                    (110u8, 110u8, 110u8)
                }
            };
            let ir = irows - 1 - r;
            let i = (ir * icols + c) * 4;
            px[i] = rr;
            px[i + 1] = gg;
            px[i + 2] = bb;
            px[i + 3] = 255;
        }
    }
    let dev_path = p2f_output_dir().join(format!("{tag}_deviation.png"));
    image::save_buffer(
        &dev_path,
        &px,
        icols as u32,
        irows as u32,
        image::ColorType::Rgba8,
    )
    .expect("save deviation png");

    // Raw grid dump for offline cross-run diffs (NaN = no data).
    let raw: Vec<u8> = img
        .iter()
        .zip(img_hit.iter())
        .flat_map(|(&d, &hit)| (if hit { d } else { f32::NAN }).to_le_bytes())
        .collect();
    let raw_path = p2f_output_dir().join(format!("{tag}_deviation_{irows}x{icols}.f32"));
    std::fs::write(&raw_path, raw).expect("write raw deviation grid");

    let composite_path = p2f_output_dir().join(format!("{tag}_stock_composite.png"));
    rs_cam_core::fingerprint::save_mesh_composite_png(&sim.mesh, &composite_path, 1800, 1200)
        .expect("save stock composite png");

    eprintln!(
        "deviation png: {} (red=overcut, blue=leftover, sat ±0.3mm) | composite: {}",
        dev_path.display(),
        composite_path.display()
    );
}

/// The B-branch dials. Quality parity with A on the band each strategy
/// owns: raster_stepover matches A's 0.3 mm on shallows; scallop_height
/// 0.011 mm is A's EFFECTIVE cusp on the mid-steep slopes (A's 0.3 mm
/// horizontal stepover stretches to ~0.52 mm along a 55° surface →
/// 0.52²/(8·3) ≈ 0.011 — scallop holds that cusp everywhere in the band);
/// z_step 0.3 mm gives wall spacing parity for any waterline band.
fn ab_unified_config() -> UnifiedFinishConfig {
    UnifiedFinishConfig {
        steep_threshold_deg: 45.0,
        // P2.e-locked default (checkpoints #1/#2 measured 65; the Tier-2
        // sweep measured 65→75 at −11.7% finish, collisions unchanged —
        // see `p2e_threshold_chain_sweep`). Tracking the shipping default
        // keeps B-only reruns measuring what the op does out of the box.
        waterline_threshold_deg: 75.0,
        overlap_mm: 2.0,
        scallop_height: 0.011,
        tolerance: 0.05,
        raster_stepover: 0.3,
        z_step: 0.3,
        sampling: 0.5,
        stock_to_leave: 0.0,
        feed_rate: 3000.0,
        plunge_rate: 150.0,
        spindle_rpm: Some(21000),
    }
}

/// Branch A's measured totals (this harness, 2026-07-08): project 8919.5 s,
/// finish op 6883.4 s, collisions 0 — reproduced the P1 headless baseline
/// (8920 s) to within 0.5 s. `p2c_unified_finish_branch_b` compares against
/// these pinned values so a B-only iteration doesn't pay A's ~35-minute
/// re-measurement; rerun `p2c_unified_finish_ab` for a fresh two-branch
/// measurement whenever the chain or simulator changes materially.
const PINNED_A_PROJECT_S: f64 = 8919.5;
const PINNED_A_FINISH_S: f64 = 6883.4;

#[test]
#[ignore = "one full-project dexel simulation ladder; run with --ignored --nocapture"]
fn p2c_unified_finish_branch_b() {
    let path = wanaka_project_path();
    let mut b = ProjectSession::load(&path).expect("load wanaka.toml (B)");
    let finish_idx = (0..b.toolpath_count())
        .find(|&i| {
            b.get_toolpath_config(i)
                .is_some_and(|tc| tc.name == FINISH_OP_NAME)
        })
        .expect("wanaka must contain '3D Finish 6'");
    b.set_toolpath_operation(
        finish_idx,
        OperationConfig::UnifiedFinish(ab_unified_config()),
    )
    .expect("swap Finish 6 operation to UnifiedFinish");
    let out_b = run_chain("B: unified finish", &mut b);

    let d_project = out_b.project_total_s - PINNED_A_PROJECT_S;
    let d_finish = out_b.finish_total_s - PINNED_A_FINISH_S;
    eprintln!("== P2.c checkpoint #1 verdict (vs pinned A) ==");
    eprintln!(
        "finish op : A={PINNED_A_FINISH_S:8.1}s  B={:8.1}s  Δ={d_finish:+8.1}s ({:+.1}%)",
        out_b.finish_total_s,
        100.0 * d_finish / PINNED_A_FINISH_S
    );
    eprintln!(
        "project   : A={PINNED_A_PROJECT_S:8.1}s  B={:8.1}s  Δ={d_project:+8.1}s ({:+.1}%)",
        out_b.project_total_s,
        100.0 * d_project / PINNED_A_PROJECT_S
    );
    eprintln!(
        "collisions: B={} (baseline {BASELINE_RAPID_COLLISIONS})",
        out_b.collisions
    );
    assert!(
        out_b.collisions <= BASELINE_RAPID_COLLISIONS,
        "P2.c SAFETY GATE FAILED: {} rapid collisions vs baseline {}",
        out_b.collisions,
        BASELINE_RAPID_COLLISIONS
    );
    assert!(
        out_b.finish_total_s > 0.0,
        "UnifiedFinish produced no runtime"
    );
}

#[test]
#[ignore = "two full-project dexel simulation ladders; run with --ignored --nocapture"]
fn p2c_unified_finish_ab() {
    let path = wanaka_project_path();
    assert!(
        path.exists(),
        "wanaka.toml not found at {} — harness requires the canonical project",
        path.display()
    );

    // ── Branch A: unmodified chain ───────────────────────────────────────
    let mut a = ProjectSession::load(&path).expect("load wanaka.toml (A)");
    let out_a = run_chain("A: drop_cutter finish", &mut a);

    // ── Branch B: Finish 6 swapped in place to UnifiedFinish ────────────
    let mut b = ProjectSession::load(&path).expect("load wanaka.toml (B)");
    let finish_idx = (0..b.toolpath_count())
        .find(|&i| {
            b.get_toolpath_config(i)
                .is_some_and(|tc| tc.name == FINISH_OP_NAME)
        })
        .expect("wanaka must contain '3D Finish 6'");

    b.set_toolpath_operation(
        finish_idx,
        OperationConfig::UnifiedFinish(ab_unified_config()),
    )
    .expect("swap Finish 6 operation to UnifiedFinish");
    let out_b = run_chain("B: unified finish", &mut b);

    // ── Verdict ──────────────────────────────────────────────────────────
    let d_project = out_b.project_total_s - out_a.project_total_s;
    let d_finish = out_b.finish_total_s - out_a.finish_total_s;
    eprintln!("== P2.c checkpoint #1 verdict ==");
    eprintln!(
        "finish op : A={:8.1}s  B={:8.1}s  Δ={:+8.1}s ({:+.1}%)",
        out_a.finish_total_s,
        out_b.finish_total_s,
        d_finish,
        100.0 * d_finish / out_a.finish_total_s.max(1e-9)
    );
    eprintln!(
        "project   : A={:8.1}s  B={:8.1}s  Δ={:+8.1}s ({:+.1}%)",
        out_a.project_total_s,
        out_b.project_total_s,
        d_project,
        100.0 * d_project / out_a.project_total_s.max(1e-9)
    );
    eprintln!(
        "collisions: A={}  B={}  (baseline {BASELINE_RAPID_COLLISIONS})",
        out_a.collisions, out_b.collisions
    );

    // Safety gate: the swap must not add collisions over the baseline.
    assert!(
        out_b.collisions <= BASELINE_RAPID_COLLISIONS,
        "P2.c SAFETY GATE FAILED: {} rapid collisions vs baseline {}",
        out_b.collisions,
        BASELINE_RAPID_COLLISIONS
    );
    assert!(
        out_b.finish_total_s > 0.0,
        "UnifiedFinish op produced no measured runtime — generation failed?"
    );
}

/// Generation-only probe: the orchestrator on the bare wanaka mesh with the
/// A/B's exact dials — no session, no stock chain, no simulation. Separates
/// "unified generation is pathological" from "the generated toolpath is so
/// large the ladder sims balloon" when the full A/B stalls in branch B.
#[test]
#[ignore = "wanaka mesh generation probe; run with --ignored --nocapture"]
fn p2c_unified_generation_probe() {
    use rs_cam_core::finish_planner::FinishPlannerParams;
    use rs_cam_core::mesh::SpatialIndex;
    use rs_cam_core::tool::BallEndmill;
    use rs_cam_core::unified_finish::{UnifiedFinishParams, unified_finish_toolpath_with_cancel};
    use std::time::Instant;

    let session = ProjectSession::load(&wanaka_project_path()).expect("load wanaka.toml");
    let mesh = session
        .models()
        .iter()
        .find_map(|m| m.mesh.clone())
        .expect("mesh");
    let index = SpatialIndex::build(&mesh, 10.0);
    let cutter = BallEndmill::new(6.0, 25.0);

    let params = UnifiedFinishParams {
        scallop_height: 0.004,
        tolerance: 0.05,
        raster_stepover: 0.3,
        z_step: 0.3,
        sampling: 0.5,
        stock_to_leave: 0.0,
        feed_rate: 3000.0,
        plunge_rate: 150.0,
        safe_z: 15.0,
    };
    let mut planner = FinishPlannerParams::for_tool(3.0);
    planner.overlap_mm = 2.0;

    let t0 = Instant::now();
    let cancel = || false;
    let (tp, anns, report) = unified_finish_toolpath_with_cancel(
        &mesh, &index, &cutter, 10.0, -10.0, &params, &planner, None, None, None, &cancel,
    )
    .expect("unified generation");
    eprintln!(
        "generation: {:.1}s | moves={} anns={}",
        t0.elapsed().as_secs_f64(),
        tp.moves.len(),
        anns.len()
    );
    eprintln!(
        "bands: very_steep {} regions/{} moves, mid_steep {} regions/{} moves, shallow {} regions/{} moves",
        report.very_steep.region_count,
        report.very_steep.move_count,
        report.mid_steep.region_count,
        report.mid_steep.move_count,
        report.shallow.region_count,
        report.shallow.move_count
    );
}
/// Phase-split probe: replicates the orchestrator body step by step with a
/// wall-clock print between phases, to pinpoint which stage of B's unified
/// generation is pathological on wanaka. Streams via eprintln (unbuffered).
#[test]
#[ignore = "phase-timing probe; run with --ignored --nocapture"]
fn p2c_unified_phase_probe() {
    use rs_cam_core::dropcutter::batch_drop_cutter_with_cancel;
    use rs_cam_core::finish_planner::{FinishBand, FinishPlannerParams, decompose};
    use rs_cam_core::finish_setup::{
        SLOPE_FILTER_MAX_DEG, SLOPE_FILTER_MIN_DEG, build_classification_surface_with_cancel,
    };
    use rs_cam_core::mesh::SpatialIndex;
    use rs_cam_core::region_set::RegionSet;
    use rs_cam_core::scallop::{
        ScallopDirection, ScallopParams, scallop_toolpath_structured_annotated_with_cancel,
    };
    use rs_cam_core::tool::BallEndmill;
    use rs_cam_core::toolpath::raster_toolpath_from_grid;
    use rs_cam_core::waterline::{WaterlineParams, waterline_toolpath_with_cancel};
    use std::time::Instant;

    let session = ProjectSession::load(&wanaka_project_path()).expect("load wanaka.toml");
    let mesh = session
        .models()
        .iter()
        .find_map(|m| m.mesh.clone())
        .expect("mesh");
    let index = SpatialIndex::build(&mesh, 10.0);
    let cutter = BallEndmill::new(6.0, 25.0);
    let cancel = || false;

    let t = Instant::now();
    let surface = build_classification_surface_with_cancel(&mesh, &index, &cutter, 0.05, &cancel)
        .expect("classification");
    eprintln!(
        "[{:8.1}s] classification surface",
        t.elapsed().as_secs_f64()
    );

    let mut planner = FinishPlannerParams::for_tool(3.0);
    planner.overlap_mm = 2.0;
    let planned = decompose(
        &surface.slope_map,
        &surface.heightmap.covered,
        &[],
        3.0,
        &planner,
    );
    eprintln!(
        "[{:8.1}s] decompose: {} regions",
        t.elapsed().as_secs_f64(),
        planned.stats.region_count
    );

    let mut very = Vec::new();
    let mut mid = Vec::new();
    let mut shallow = Vec::new();
    for r in &planned.regions {
        let vcount = r.polygon.exterior.len() + r.polygon.holes.iter().map(Vec::len).sum::<usize>();
        eprintln!("  region {:?}: {} vertices", r.band, vcount);
        match r.band {
            FinishBand::VerySteep => very.push(r.polygon.clone()),
            FinishBand::MidSteep => mid.push(r.polygon.clone()),
            FinishBand::Shallow => shallow.push(r.polygon.clone()),
        }
    }

    if !very.is_empty() {
        let rs = RegionSet::new(very);
        let wp = WaterlineParams {
            sampling: 0.5,
            feed_rate: 3000.0,
            plunge_rate: 150.0,
            safe_z: 15.0,
        };
        let tp = waterline_toolpath_with_cancel(
            &mesh,
            &index,
            &cutter,
            4.0,
            -2.1,
            0.3,
            &wp,
            Some(&rs),
            &cancel,
        )
        .expect("waterline");
        eprintln!(
            "[{:8.1}s] waterline: {} moves",
            t.elapsed().as_secs_f64(),
            tp.moves.len()
        );
    } else {
        eprintln!("[{:8.1}s] waterline: band empty", t.elapsed().as_secs_f64());
    }

    if !mid.is_empty() {
        let rs = RegionSet::new(mid);
        let sp = ScallopParams {
            scallop_height: 0.004,
            tolerance: 0.05,
            direction: ScallopDirection::default(),
            continuous: true,
            slope_from: SLOPE_FILTER_MIN_DEG,
            slope_to: SLOPE_FILTER_MAX_DEG,
            feed_rate: 3000.0,
            plunge_rate: 150.0,
            safe_z: 15.0,
            stock_to_leave: 0.0,
        };
        let (tp, _anns) = scallop_toolpath_structured_annotated_with_cancel(
            &mesh,
            &index,
            &cutter,
            &sp,
            None,
            Some(&rs),
            &cancel,
        )
        .expect("scallop");
        eprintln!(
            "[{:8.1}s] scallop: {} moves",
            t.elapsed().as_secs_f64(),
            tp.moves.len()
        );
    }

    if !shallow.is_empty() {
        let rs = RegionSet::new(shallow);
        let never = || false;
        let grid = batch_drop_cutter_with_cancel(
            &mesh,
            &index,
            &cutter,
            0.3,
            0.0,
            mesh.bbox.min.z - 0.1,
            &never,
        )
        .expect("batch");
        eprintln!(
            "[{:8.1}s] raster grid: {} pts",
            t.elapsed().as_secs_f64(),
            grid.points.len()
        );
        let tp = raster_toolpath_from_grid(
            &grid,
            3000.0,
            150.0,
            15.0,
            Some(mesh.bbox.min.z - 0.1),
            Some(&rs),
        );
        eprintln!(
            "[{:8.1}s] raster: {} moves",
            t.elapsed().as_secs_f64(),
            tp.moves.len()
        );
    }
    eprintln!("[{:8.1}s] PROBE DONE", t.elapsed().as_secs_f64());
}

/// Scallop-height cost curve on the wanaka mid-steep band: times the
/// selective scallop call at descending heights. Establishes whether the
/// h=0.004 hang is a smooth cost curve (fix = dial sanity + a cost guard)
/// or a cliff (fix = algorithmic).
#[test]
#[ignore = "scallop cost-curve probe; run with --ignored --nocapture"]
fn p2c_scallop_height_cost_curve() {
    use rs_cam_core::finish_planner::{FinishBand, FinishPlannerParams, decompose};
    use rs_cam_core::finish_setup::{
        SLOPE_FILTER_MAX_DEG, SLOPE_FILTER_MIN_DEG, build_classification_surface_with_cancel,
    };
    use rs_cam_core::mesh::SpatialIndex;
    use rs_cam_core::region_set::RegionSet;
    use rs_cam_core::scallop::{
        ScallopDirection, ScallopParams, scallop_toolpath_structured_annotated_with_cancel,
    };
    use rs_cam_core::tool::BallEndmill;
    use std::time::Instant;

    let session = ProjectSession::load(&wanaka_project_path()).expect("load wanaka.toml");
    let mesh = session
        .models()
        .iter()
        .find_map(|m| m.mesh.clone())
        .expect("mesh");
    let index = SpatialIndex::build(&mesh, 10.0);
    let cutter = BallEndmill::new(6.0, 25.0);
    let cancel = || false;

    let surface = build_classification_surface_with_cancel(&mesh, &index, &cutter, 0.05, &cancel)
        .expect("classification");
    let mut planner = FinishPlannerParams::for_tool(3.0);
    planner.overlap_mm = 2.0;
    let planned = decompose(
        &surface.slope_map,
        &surface.heightmap.covered,
        &[],
        3.0,
        &planner,
    );
    let mid: Vec<_> = planned
        .regions
        .iter()
        .filter(|r| r.band == FinishBand::MidSteep)
        .map(|r| r.polygon.clone())
        .collect();
    assert!(!mid.is_empty());
    let rs = RegionSet::new(mid);

    for h in [0.1, 0.05, 0.02, 0.011] {
        let sp = ScallopParams {
            scallop_height: h,
            tolerance: 0.05,
            direction: ScallopDirection::default(),
            continuous: true,
            slope_from: SLOPE_FILTER_MIN_DEG,
            slope_to: SLOPE_FILTER_MAX_DEG,
            feed_rate: 3000.0,
            plunge_rate: 150.0,
            safe_z: 15.0,
            stock_to_leave: 0.0,
        };
        let t = Instant::now();
        let (tp, _anns) = scallop_toolpath_structured_annotated_with_cancel(
            &mesh,
            &index,
            &cutter,
            &sp,
            None,
            Some(&rs),
            &cancel,
        )
        .expect("scallop");
        eprintln!(
            "h={h:<6} -> {:8.1}s, {} moves",
            t.elapsed().as_secs_f64(),
            tp.moves.len()
        );
    }
}

/// Offset-cascade probe: runs `offset_polygon` inward repeatedly on the
/// actual wanaka mid-steep polygon at the h=0.02-equivalent stepover,
/// printing fragment/vertex counts per iteration — isolates whether the
/// scallop cliff lives in polygon offsetting on dendritic shapes.
#[test]
#[ignore = "offset-cascade probe; run with --ignored --nocapture"]
fn p2c_offset_cascade_probe() {
    use rs_cam_core::finish_planner::{FinishBand, FinishPlannerParams, decompose};
    use rs_cam_core::finish_setup::build_classification_surface_with_cancel;
    use rs_cam_core::mesh::SpatialIndex;
    use rs_cam_core::polygon::{Polygon2, offset_polygon};
    use rs_cam_core::tool::BallEndmill;
    use std::time::Instant;

    let session = ProjectSession::load(&wanaka_project_path()).expect("load wanaka.toml");
    let mesh = session
        .models()
        .iter()
        .find_map(|m| m.mesh.clone())
        .expect("mesh");
    let index = SpatialIndex::build(&mesh, 10.0);
    let cutter = BallEndmill::new(6.0, 25.0);
    let cancel = || false;
    let surface = build_classification_surface_with_cancel(&mesh, &index, &cutter, 0.05, &cancel)
        .expect("classification");
    let mut planner = FinishPlannerParams::for_tool(3.0);
    planner.overlap_mm = 2.0;
    let planned = decompose(
        &surface.slope_map,
        &surface.heightmap.covered,
        &[],
        3.0,
        &planner,
    );
    let mid: Vec<Polygon2> = planned
        .regions
        .iter()
        .filter(|r| r.band == FinishBand::MidSteep)
        .map(|r| r.polygon.clone())
        .collect();

    // h=0.02 flat stepover on r=3: sqrt(8*3*0.02) ≈ 0.69 mm.
    let stepover = 0.69f64;
    let mut current = mid;
    let t = Instant::now();
    for ring in 0..200 {
        if current.is_empty() {
            eprintln!(
                "[{:7.1}s] ring {ring}: cascade exhausted",
                t.elapsed().as_secs_f64()
            );
            break;
        }
        let verts: usize = current
            .iter()
            .map(|p| p.exterior.len() + p.holes.iter().map(Vec::len).sum::<usize>())
            .sum();
        let r0 = Instant::now();
        let mut next = Vec::new();
        for p in &current {
            next.extend(offset_polygon(p, stepover));
        }
        eprintln!(
            "[{:7.1}s] ring {ring}: {} polys, {} verts -> offset took {:6.2}s -> {} polys",
            t.elapsed().as_secs_f64(),
            current.len(),
            verts,
            r0.elapsed().as_secs_f64(),
            next.len()
        );
        current = next;
    }
}

/// P2.e Tier-2 sweep (design risk R4): the EXPOSED threshold dials through
/// the FULL wanaka chain — F.4 ladder, GUI-options modulated sim, F-034
/// integrator — scored exactly like the checkpoints (project/finish time +
/// collision count vs the pinned A). OFAT around the shipping defaults
/// (45/65); the conditioning dials (hysteresis/close/min-area) sweep at the
/// decomposition level instead (`finish_planner_wanaka_decompose.rs::
/// p2e_conditioning_dial_sweep`) — they aren't exposed on the op config
/// (one-new-dial rule) and their effect is structural, not chain-dependent.
///
/// ~8 configs × ~1 min per B chain. REPORTING test: a bad dial's collision
/// count is a finding for the table, not a failure — only a chain that
/// fails to complete (or a default row breaking the safety gate) asserts.
#[test]
#[ignore = "8 full wanaka chains (~10 min); run with --ignored --nocapture"]
fn p2e_threshold_chain_sweep() {
    let path = wanaka_project_path();
    let rows: [(f64, f64); 8] = [
        (45.0, 65.0), // shipping default — the anchor row
        (35.0, 65.0),
        (40.0, 65.0),
        (50.0, 65.0),
        (55.0, 65.0),
        (45.0, 55.0),
        (45.0, 75.0),
        (45.0, 85.0),
    ];

    let mut results: Vec<(f64, f64, ChainOutcome)> = Vec::new();
    for (steep, waterline) in rows {
        let mut s = ProjectSession::load(&path).expect("load wanaka.toml");
        let finish_idx = (0..s.toolpath_count())
            .find(|&i| {
                s.get_toolpath_config(i)
                    .is_some_and(|tc| tc.name == FINISH_OP_NAME)
            })
            .expect("wanaka must contain '3D Finish 6'");
        let cfg = UnifiedFinishConfig {
            steep_threshold_deg: steep,
            waterline_threshold_deg: waterline,
            ..ab_unified_config()
        };
        s.set_toolpath_operation(finish_idx, OperationConfig::UnifiedFinish(cfg))
            .expect("swap Finish 6 operation to UnifiedFinish");
        let label = format!("P2.e steep={steep} waterline={waterline}");
        let out = run_chain(&label, &mut s);
        results.push((steep, waterline, out));
    }

    eprintln!(
        "== P2.e Tier-2 verdict table (vs pinned A {PINNED_A_PROJECT_S:.1}s / {PINNED_A_FINISH_S:.1}s) =="
    );
    eprintln!(
        "{:>5} | {:>9} | {:>9} ({:>6}) | {:>9} ({:>6}) | {:>4}",
        "steep", "waterline", "finish_s", "d%", "project_s", "d%", "coll"
    );
    for (steep, waterline, out) in &results {
        eprintln!(
            "{steep:>5} | {waterline:>9} | {:>9.1} ({:>+5.1}%) | {:>9.1} ({:>+5.1}%) | {:>4}{}",
            out.finish_total_s,
            100.0 * (out.finish_total_s - PINNED_A_FINISH_S) / PINNED_A_FINISH_S,
            out.project_total_s,
            100.0 * (out.project_total_s - PINNED_A_PROJECT_S) / PINNED_A_PROJECT_S,
            out.collisions,
            if out.collisions > BASELINE_RAPID_COLLISIONS {
                "  << OVER BASELINE"
            } else {
                ""
            }
        );
    }

    // Hard gate on the anchor row only: the shipping default must hold the
    // safety baseline whatever the exploratory rows do.
    let (_, _, default_out) = results.first().expect("anchor row ran");
    assert!(
        default_out.collisions <= BASELINE_RAPID_COLLISIONS,
        "P2.e SAFETY GATE FAILED on the default dials: {} rapid collisions vs baseline {}",
        default_out.collisions,
        BASELINE_RAPID_COLLISIONS
    );
    assert!(default_out.finish_total_s > 0.0);
}

/// Branch A re-measure with the removal + deviation columns (they landed
/// after A was pinned, so the pinned constants carry no material data).
/// Prints drift vs the pinned times as a sanity check on the pin itself.
#[test]
#[ignore = "one full-project dexel simulation ladder on the all-over raster (long); run with --ignored --nocapture"]
fn p2e_branch_a_remeasure() {
    let path = wanaka_project_path();
    let mut a = ProjectSession::load(&path).expect("load wanaka.toml (A)");
    let out = run_chain("A: unmodified chain, remeasured", &mut a);
    eprintln!(
        "pin drift: project {:+.1}s vs {PINNED_A_PROJECT_S:.1}, finish {:+.1}s vs {PINNED_A_FINISH_S:.1}",
        out.project_total_s - PINNED_A_PROJECT_S,
        out.finish_total_s - PINNED_A_FINISH_S
    );
    assert!(out.finish_total_s > 0.0);
}

/// Branch C — "doing the toolpaths one at a time": the SAME strategies as
/// the unified op, but hand-chained as today's STANDALONE ops with per-op
/// slope windows instead of the planner's conditioned regions + router.
/// C1 = Scallop confined to slopes ≥45° (continuous rings, B's parity
/// height); C2 = the original drop-cutter raster confined to <45°. Both
/// keep A's tool/heights/boundary/feeds.
///
/// The structural handicap this measures: standalone slope windows
/// classify on each op's own OFFSET (ball-center) surface — the blind
/// spot the unified op's true-surface classification fixed. On wanaka the
/// offset surface reads 0.1% of cells ≥45°, so C1 is expected to find
/// almost nothing and C2 to raster nearly all-over — i.e. "one at a time"
/// collapses toward branch A no matter which strategies you chain. That
/// expectation is exactly what this test measures rather than assumes.
#[test]
#[ignore = "one full-project dexel simulation ladder (long, raster-sized); run with --ignored --nocapture"]
fn p2e_separate_ops_branch_c() {
    let path = wanaka_project_path();
    let mut c = ProjectSession::load(&path).expect("load wanaka.toml (C)");
    let finish_idx = (0..c.toolpath_count())
        .find(|&i| {
            c.get_toolpath_config(i)
                .is_some_and(|tc| tc.name == FINISH_OP_NAME)
        })
        .expect("wanaka must contain '3D Finish 6'");

    // C2 inherits A's toolpath config field-by-field (`ToolpathConfig`
    // deliberately isn't Clone — fresh IDs come from `add_toolpath`) and
    // A's raster op VERBATIM (same stepover/feeds/min_z), narrowed to the
    // shallow window. Snapshot everything before C1's swap destroys it.
    let template = c
        .get_toolpath_config(finish_idx)
        .expect("finish toolpath config");
    let OperationConfig::DropCutter(a_raster) = template.operation.clone() else {
        panic!("expected '3D Finish 6' to be a DropCutter raster (branch A shape)");
    };
    // C1 reads these after `a_raster` is consumed by C2's struct update.
    let (a_feed, a_plunge, a_rpm) = (
        a_raster.feed_rate,
        a_raster.plunge_rate,
        a_raster.spindle_rpm,
    );
    let mut c2 = rs_cam_core::session::ToolpathConfig {
        id: template.id, // reassigned by add_toolpath
        name: template.name.clone(),
        enabled: template.enabled,
        operation: OperationConfig::DropCutter(DropCutterConfig {
            slope_from: 0.0,
            slope_to: 45.0,
            ..a_raster
        }),
        dressups: template.dressups.clone(),
        heights: template.heights.clone(),
        tool_id: template.tool_id,
        model_id: template.model_id,
        pre_gcode: template.pre_gcode.clone(),
        post_gcode: template.post_gcode.clone(),
        boundary: template.boundary.clone(),
        boundary_inherit: template.boundary_inherit,
        rest_analysis: template.rest_analysis.clone(),
        stock_source: template.stock_source,
        coolant: template.coolant,
        face_selection: template.face_selection.clone(),
        debug_options: template.debug_options,
        feeds_provenance: template.feeds_provenance.clone(),
    };
    c2.name = FINISH_OP_NAME.to_owned();

    // C1: standalone Scallop on the steep window (B's mid-steep parity
    // dials: height 0.011 = A's effective cusp at ~55°, continuous rings).
    c.set_toolpath_operation(
        finish_idx,
        OperationConfig::Scallop(ScallopConfig {
            scallop_height: 0.011,
            tolerance: 0.05,
            direction: ScallopDirection::OutsideIn,
            continuous: true,
            slope_from: 45.0,
            slope_to: 90.0,
            feed_rate: a_feed,
            plunge_rate: a_plunge,
            stock_to_leave: 0.0,
            spindle_rpm: a_rpm,
        }),
    )
    .expect("swap Finish 6 to slope-windowed Scallop");

    // C2 appended to the chain end (order after the roughs is what
    // matters; nothing downstream references the finish stock in this
    // headless chain). Same name so run_chain sums both into finish
    // totals.
    c.add_toolpath(0, c2).expect("append shallow raster op");

    let out = run_chain("C: separate slope-windowed ops", &mut c);

    eprintln!("== P2.e branch C verdict (one-at-a-time vs pinned A / locked-default B) ==");
    eprintln!(
        "finish : A={PINNED_A_FINISH_S:8.1}s  C={:8.1}s ({:+.1}%)  [B locked: 5766.5s (-16.2%)]",
        out.finish_total_s,
        100.0 * (out.finish_total_s - PINNED_A_FINISH_S) / PINNED_A_FINISH_S
    );
    eprintln!(
        "project: A={PINNED_A_PROJECT_S:8.1}s  C={:8.1}s ({:+.1}%)  [B locked: 7802.6s (-12.5%)]",
        out.project_total_s,
        100.0 * (out.project_total_s - PINNED_A_PROJECT_S) / PINNED_A_PROJECT_S
    );
    eprintln!(
        "removed: finish {:.0} mm3, project {:.0} mm3 | collisions: {} (baseline {BASELINE_RAPID_COLLISIONS})",
        out.finish_removed_mm3, out.project_removed_mm3, out.collisions
    );
    assert!(
        out.finish_total_s > 0.0,
        "branch C produced no finish runtime"
    );
}

/// P2.f Task 3 probe: why did branch A's totals stay byte-identical after
/// the raster serpentine? Census the finish op's move structure — if the
/// serpentine fired, rapids collapse to ~2; if A's emission never linked
/// (or a dressup/clip re-introduced the cycles), rapids stay ~2/row.
#[test]
#[ignore = "one generation ladder; run with --ignored --nocapture"]
fn p2f_a_move_census() {
    let cancel = AtomicBool::new(false);
    let path = wanaka_project_path();
    let mut a = ProjectSession::load(&path).expect("load wanaka.toml");
    let n = a.toolpath_count();
    let enabled: Vec<usize> = (0..n)
        .filter(|&i| a.get_toolpath_config(i).is_some_and(|tc| tc.enabled))
        .collect();
    let mut pending: Vec<usize> = Vec::new();
    for &i in &enabled {
        if a.generate_toolpath(i, &cancel).is_err() {
            pending.push(i);
        }
    }
    while !pending.is_empty() {
        a.run_simulation(&SimulationOptions::default(), &cancel)
            .expect("ladder sim");
        let before = pending.len();
        pending.retain(|&i| a.generate_toolpath(i, &cancel).is_err());
        assert!(pending.len() < before, "ladder stalled: {pending:?}");
    }
    let finish_idx = (0..n)
        .find(|&i| {
            a.get_toolpath_config(i)
                .is_some_and(|tc| tc.name == FINISH_OP_NAME)
        })
        .expect("finish op");
    let tp = a
        .get_result(finish_idx)
        .expect("finish toolpath generated")
        .annotated();
    use rs_cam_core::toolpath::{MoveIntent, MoveType};
    let mut rapids = 0usize;
    let mut entries = 0usize;
    let mut cuts = 0usize;
    let mut links = 0usize;
    for m in &tp.toolpath.moves {
        match (m.move_type, m.intent) {
            (MoveType::Rapid, _) => rapids += 1,
            (_, MoveIntent::EntryPlunge) => entries += 1,
            (_, MoveIntent::FinishingCut) => cuts += 1,
            (_, MoveIntent::Linking) => links += 1,
            _ => {}
        }
    }
    eprintln!(
        "A finish census: moves={} rapids={rapids} entry_plunges={entries} cuts={cuts} link_feeds={links}",
        tp.toolpath.moves.len()
    );
}

/// P2.f Task 1 instrument, branch A: the unmodified chain scored with the
/// per-band deviation histogram + deviation-map PNG. This is the fidelity
/// REFERENCE — A's exact per-point 0.3 mm raster is the quality bar the
/// unified op must match per band, not just on the blunt mean.
#[test]
#[ignore = "one full-project dexel simulation ladder (long); run with --ignored --nocapture"]
fn p2f_fidelity_branch_a() {
    let path = wanaka_project_path();
    let mut a = ProjectSession::load(&path).expect("load wanaka.toml (A)");
    let out = run_chain("A: fidelity reference", &mut a);
    eprintln!(
        "pin drift: project {:+.1}s vs {PINNED_A_PROJECT_S:.1}, finish {:+.1}s vs {PINNED_A_FINISH_S:.1}",
        out.project_total_s - PINNED_A_PROJECT_S,
        out.finish_total_s - PINNED_A_FINISH_S
    );
    run_measurement_sim(&mut a);
    let bm = build_band_map(&a);
    save_band_map_png(&bm, &p2f_output_dir().join("band_map.png"));
    fidelity_report("p2f_A", &a, &bm);
    assert!(out.finish_total_s > 0.0);
}

/// P2.f Task 1 instrument, branch B: the unified op (locked defaults)
/// scored per band against the same band map as A. The smooshed-mountain
/// defect must show here as mid-steep OVERCUT mass that branch A doesn't
/// have; after the fidelity fix this test is the acceptance rerun.
#[test]
#[ignore = "one full-project dexel simulation ladder; run with --ignored --nocapture"]
fn p2f_fidelity_branch_b() {
    let path = wanaka_project_path();
    let mut b = ProjectSession::load(&path).expect("load wanaka.toml (B)");
    let finish_idx = (0..b.toolpath_count())
        .find(|&i| {
            b.get_toolpath_config(i)
                .is_some_and(|tc| tc.name == FINISH_OP_NAME)
        })
        .expect("wanaka must contain '3D Finish 6'");
    b.set_toolpath_operation(
        finish_idx,
        OperationConfig::UnifiedFinish(ab_unified_config()),
    )
    .expect("swap Finish 6 operation to UnifiedFinish");
    let out = run_chain("B: unified finish (fidelity)", &mut b);
    eprintln!(
        "vs pinned A: finish {:+.1}% project {:+.1}% collisions={}",
        100.0 * (out.finish_total_s - PINNED_A_FINISH_S) / PINNED_A_FINISH_S,
        100.0 * (out.project_total_s - PINNED_A_PROJECT_S) / PINNED_A_PROJECT_S,
        out.collisions
    );
    run_measurement_sim(&mut b);
    let bm = build_band_map(&b);
    fidelity_report("p2f_B", &b, &bm);
    assert!(
        out.collisions <= BASELINE_RAPID_COLLISIONS,
        "P2.f SAFETY GATE FAILED: {} rapid collisions vs baseline {}",
        out.collisions,
        BASELINE_RAPID_COLLISIONS
    );
    assert!(out.finish_total_s > 0.0);
}
