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
    CreaseReference, DropCutterConfig, ScallopConfig, ScallopDirection, UnifiedFinishConfig,
};
use rs_cam_core::session::{ProjectSession, SimulationOptions};

/// Pre-existing full-chain collision count (P0/P1 baseline, 2026-07-07).
const BASELINE_RAPID_COLLISIONS: usize = 4;

fn wanaka_project_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("planning")
        .join("airrun_2026-06-01")
        .join("wanaka.toml")
}

/// Resolve the single ENABLED toolpath whose name contains "Finish" —
/// wanaka.toml is user-live and evolves under live sessions (2026-07-09:
/// "3D Finish 6" was disabled in favour of "Unified Finish 6 (live v2)"),
/// so probes must target the enabled slot by role, not by a name pinned to
/// history. Panics if zero or more than one enabled finish op is found.
fn enabled_finish_index(s: &ProjectSession) -> usize {
    let n = s.toolpath_count();
    let finish_indices: Vec<usize> = (0..n)
        .filter(|&i| {
            s.get_toolpath_config(i)
                .is_some_and(|tc| tc.enabled && tc.name.contains("Finish"))
        })
        .collect();
    assert_eq!(
        finish_indices.len(),
        1,
        "expected exactly one enabled finish op, found {finish_indices:?}"
    );
    let idx = finish_indices[0];
    eprintln!(
        "targeting enabled finish op '{}'",
        s.get_toolpath_config(idx).expect("cfg").name
    );
    idx
}

struct ChainOutcome {
    project_total_s: f64,
    /// Sum over every op sharing the resolved enabled finish op's NAME —
    /// branch C splits the finish pass into two same-named ops, so this is
    /// a += accumulation (see `run_chain`).
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
    // Resolve the finish op's actual name up front (before any generation
    // runs) so the accumulation below tracks whichever op is really
    // enabled, not a name pinned to a historical session state.
    let finish_name = s
        .get_toolpath_config(enabled_finish_index(s))
        .expect("finish op config")
        .name
        .clone();
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
        if name == finish_name {
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
        surface.heightmap.covered_flags(),
        &[],
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

    // Pointwise per-dexel-column table — the honest instrument. The vertex
    // table above measures on corner-averaged mesh heights, which filter
    // machined texture by its phase coherence vs the dexel grid (P2.g
    // Task 1: grid-locked ridges survive the 2×2 average, phase-diverse
    // ones cancel — a fake branch-dependent histogram shift). Columns
    // sample each dexel top directly.
    if let Some(cols) = sim.column_deviations.as_ref() {
        // Group-filtered (2026-07-09): multi-setup projects sample the same
        // world XY once per setup group, and the BOTTOM setup's columns
        // cross-attributed against the TOP surface produced the entire
        // `<-.5` "gouge" tail (11.2 k) in every earlier table. One table
        // per group keeps the attribution honest; read the group whose
        // setup machines the surface the band map describes.
        let max_group = cols.iter().map(|cd| cd.group).max().unwrap_or(0);
        for group in 0..=max_group {
            let mut caccs = [BandAcc::default(); 4];
            for cd in cols.iter().filter(|cd| cd.group == group) {
                let code = bm.code_at(cd.x, cd.y) as usize;
                let a = &mut caccs[code];
                a.bins[bin_of(cd.dev)] += 1;
                if cd.dev > EPS {
                    a.leftover_n += 1;
                    a.leftover_sum += f64::from(cd.dev);
                    a.leftover_max = a.leftover_max.max(cd.dev);
                } else if cd.dev < -EPS {
                    a.overcut_n += 1;
                    a.overcut_sum += f64::from(cd.dev);
                    a.overcut_min = a.overcut_min.min(cd.dev);
                }
            }
            eprintln!(
                "== P2.f FIDELITY-COLUMNS [{tag}] group {group} (pointwise dexel tops; negative = overcut) =="
            );
            for (code, acc) in caccs.iter().enumerate() {
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
        }
    } else {
        eprintln!("[{tag}] column deviations unavailable");
    }
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
        // v3 S1 claims pipeline (`unified_finish::ClaimsConfig`, default
        // `true` in `UnifiedFinishConfig::default()`) is explicitly OFF
        // here: this is the pinned branch-A baseline
        // (`PINNED_A_PROJECT_S`/`PINNED_A_FINISH_S`) — it must keep
        // measuring the pre-v3 op, not silently pick up the new detector
        // pass. Wave 3's claims A/B adds its own dedicated config.
        pencil_claims: false,
        min_rest_depth_mm: 0.02,
        // Build-list item 3: default arm (pinned branch-A baseline; also
        // moot with pencil_claims off above).
        claims_reference: CreaseReference::SelfProbe,
        // S4 (`unified_finish::ClaimsConfig::territory_clip` doc): off —
        // moot with pencil_claims off above, and this is the pinned
        // branch-A baseline.
        territory_clip: false,
        intra_region_hookup_mm: 0.0,
        crease_hookup_mm: 5.0,
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
    let finish_idx = enabled_finish_index(&b);
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
    let finish_idx = enabled_finish_index(&b);

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
        intra_region_hookup_mm: 0.0,
    };
    let mut planner = FinishPlannerParams::for_tool(3.0);
    planner.overlap_mm = 2.0;

    let t0 = Instant::now();
    let cancel = || false;
    let (tp, anns, report) = unified_finish_toolpath_with_cancel(
        &mesh, &index, &cutter, 10.0, -10.0, &params, &planner, None, None, None, None, &cancel,
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
        surface.heightmap.covered_flags(),
        &[],
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
        let (tp, _anns, _report) = scallop_toolpath_structured_annotated_with_cancel(
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
        surface.heightmap.covered_flags(),
        &[],
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
        let (tp, _anns, _report) = scallop_toolpath_structured_annotated_with_cancel(
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

/// P2.g ring-geometry dump: writes the cutting moves of B75's per-region
/// mid-steep scallop and D's all-over scallop (same h=0.011, same tapered
/// cutter the matrix branches ran with) to text files so pass-spacing maps
/// can be computed offline. Context: the collar probe (2026-07-09) REFUTED
/// the overlap-collar theory — B-bad/D-good mid-steep cells sit at median
/// 20 mm from shallow seams (base rate 17 mm) and form a ~1.97 mm lattice
/// (= 2 classification cells), so the suspect is region-boundary jag
/// propagating inward through the ring cascade, not the seam collar.
#[test]
#[ignore = "ring geometry dump; run with --ignored --nocapture"]
fn p2g_ring_dump() {
    use rs_cam_core::finish_planner::{FinishBand, FinishPlannerParams, decompose};
    use rs_cam_core::finish_setup::{
        SLOPE_FILTER_MAX_DEG, SLOPE_FILTER_MIN_DEG, build_classification_surface_with_cancel,
    };
    use rs_cam_core::mesh::SpatialIndex;
    use rs_cam_core::region_set::RegionSet;
    use rs_cam_core::scallop::{
        ScallopDirection, ScallopParams, scallop_toolpath_structured_annotated_with_cancel,
    };
    use rs_cam_core::tool::{BallEndmill, TaperedBallEndmill};
    use rs_cam_core::toolpath::{MoveType, Toolpath};
    use std::io::Write as _;
    use std::time::Instant;

    let session = ProjectSession::load(&wanaka_project_path()).expect("load wanaka.toml");
    let mesh = session
        .models()
        .iter()
        .find_map(|m| m.mesh.clone())
        .expect("mesh");
    let index = SpatialIndex::build(&mesh, 10.0);
    // Classification keeps the Ø6-ball convention (matches build_band_map);
    // CUTTING uses the real wanaka tool 2 (1 mm tip, 7° taper, Ø6 shank).
    let classifier = BallEndmill::new(6.0, 25.0);
    let cutter = TaperedBallEndmill::new(1.0, 7.0, 6.0, 25.0);
    let cancel = || false;

    let surface =
        build_classification_surface_with_cancel(&mesh, &index, &classifier, 0.05, &cancel)
            .expect("classification");
    let mut planner = FinishPlannerParams::for_tool(3.0);
    planner.overlap_mm = 2.0;
    let planned = decompose(
        &surface.slope_map,
        surface.heightmap.covered_flags(),
        &[],
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

    let sp = ScallopParams {
        scallop_height: 0.011,
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

    let dump = |tag: &str, tp: &Toolpath| {
        let path = p2f_output_dir().join(format!("p2g_{tag}_moves.txt"));
        let mut f = std::io::BufWriter::new(std::fs::File::create(&path).expect("create dump"));
        for m in &tp.moves {
            let kind = match m.move_type {
                MoveType::Rapid => 'R',
                MoveType::Linear { .. } => 'L',
                MoveType::ArcCW { .. } | MoveType::ArcCCW { .. } => 'A',
            };
            let p = m.target;
            writeln!(f, "{kind} {:?} {:.4} {:.4} {:.4}", m.intent, p.x, p.y, p.z)
                .expect("write move");
        }
        eprintln!("wrote {} ({} moves)", path.display(), tp.moves.len());
    };

    let t = Instant::now();
    let (tp_b, _, _) = scallop_toolpath_structured_annotated_with_cancel(
        &mesh,
        &index,
        &cutter,
        &sp,
        None,
        Some(&rs),
        &cancel,
    )
    .expect("B75 mid scallop");
    eprintln!("B75 mid-steep scallop: {:.1}s", t.elapsed().as_secs_f64());
    dump("b75_mid", &tp_b);

    let t = Instant::now();
    let (tp_d, _, _) = scallop_toolpath_structured_annotated_with_cancel(
        &mesh, &index, &cutter, &sp, None, None, &cancel,
    )
    .expect("D all-over scallop");
    eprintln!("D all-over scallop: {:.1}s", t.elapsed().as_secs_f64());
    dump("d_allover", &tp_d);
}

/// P2.g session-level op-8 dump: generates branch B75 (UnifiedFinish) and
/// branch D (all-over Scallop) through the REAL session pipeline
/// (generation ladder only — no timing/measurement sims) and dumps the
/// finish op's conditioned moves, arc parameters included. Context: direct
/// bare-mesh scallop calls produce EQUIVALENT B/D floors (phase noise,
/// ±40%/40% split), yet the chain sims show D's mid-steep distribution
/// shifted ~5-8 µm DEEPER — a Z bias, not a cusp win. The suspect is
/// session conditioning that differs between the branches (D inherits
/// Finish 6's arc_fitting=true; UnifiedFinish strips all dressups). This
/// probe shows what conditioning each branch's op actually carries.
#[test]
#[ignore = "two generation ladders; run with --ignored --nocapture"]
fn p2g_session_op8_dump() {
    use rs_cam_core::toolpath::MoveType;
    use std::io::Write as _;

    let cancel = AtomicBool::new(false);
    let run = |label: &str, op: Option<OperationConfig>| {
        let mut s = ProjectSession::load(&wanaka_project_path()).expect("load wanaka.toml");
        let n = s.toolpath_count();
        let finish_idx = enabled_finish_index(&s);
        if let Some(op) = op {
            s.set_toolpath_operation(finish_idx, op).expect("swap op");
        }
        let enabled: Vec<usize> = (0..n)
            .filter(|&i| s.get_toolpath_config(i).is_some_and(|tc| tc.enabled))
            .collect();
        let mut pending: Vec<usize> = Vec::new();
        for &i in &enabled {
            if s.generate_toolpath(i, &cancel).is_err() {
                pending.push(i);
            }
        }
        while !pending.is_empty() {
            s.run_simulation(&SimulationOptions::default(), &cancel)
                .expect("ladder sim");
            let before = pending.len();
            pending.retain(|&i| s.generate_toolpath(i, &cancel).is_err());
            assert!(pending.len() < before, "ladder stalled: {pending:?}");
        }
        let tc = s.get_toolpath_config(finish_idx).expect("op config");
        eprintln!("[{label}] op8 dressups: {:?}", tc.dressups);
        let tp = s
            .get_result(finish_idx)
            .expect("finish toolpath generated")
            .annotated();
        let (mut lines, mut arcs, mut rapids) = (0usize, 0usize, 0usize);
        let path = p2f_output_dir().join(format!("p2g_sess_{label}_moves.txt"));
        let mut f = std::io::BufWriter::new(std::fs::File::create(&path).expect("create dump"));
        for m in &tp.toolpath.moves {
            let p = m.target;
            match m.move_type {
                MoveType::Rapid => {
                    rapids += 1;
                    writeln!(f, "R {:?} {:.4} {:.4} {:.4}", m.intent, p.x, p.y, p.z)
                }
                MoveType::Linear { .. } => {
                    lines += 1;
                    writeln!(f, "L {:?} {:.4} {:.4} {:.4}", m.intent, p.x, p.y, p.z)
                }
                MoveType::ArcCW { i, j, .. } => {
                    arcs += 1;
                    writeln!(
                        f,
                        "ACW {:?} {:.4} {:.4} {:.4} {:.4} {:.4}",
                        m.intent, p.x, p.y, p.z, i, j
                    )
                }
                MoveType::ArcCCW { i, j, .. } => {
                    arcs += 1;
                    writeln!(
                        f,
                        "ACCW {:?} {:.4} {:.4} {:.4} {:.4} {:.4}",
                        m.intent, p.x, p.y, p.z, i, j
                    )
                }
            }
            .expect("write move");
        }
        eprintln!(
            "[{label}] op8 moves={} lines={lines} arcs={arcs} rapids={rapids} -> {}",
            tp.toolpath.moves.len(),
            path.display()
        );
    };

    run(
        "b75",
        Some(OperationConfig::UnifiedFinish(ab_unified_config())),
    );
    run(
        "d",
        Some(OperationConfig::Scallop(ScallopConfig {
            scallop_height: 0.011,
            tolerance: 0.05,
            direction: ScallopDirection::OutsideIn,
            continuous: true,
            slope_from: 0.0,
            slope_to: 90.0,
            feed_rate: 3000.0,
            plunge_rate: 150.0,
            stock_to_leave: 0.0,
            spindle_rpm: Some(21000),
        })),
    );
}

/// P2.g measurement-aliasing confirmation: scores B75 and D at a DIFFERENT
/// measurement resolution (0.21 mm vs the instrument's 0.25 mm). Offline
/// analysis (2026-07-09) established the branches' machined floors are
/// geometrically EQUIVALENT on mid-steep (bad-vs-control floor diff 1.3 µm,
/// four independent probes), so the matrix's "D wins the fine tier" must be
/// sampling aliasing: B75's rings restart per-region from grid-aligned
/// marching-squares polygons (cusp ridges grid-locked -> the 1.41 mm bad-
/// cell lattice), D's silhouette-offset rings are phase-diverse. If that's
/// right, the B75-vs-D histogram gap MOVES when the sim grid changes; if
/// the gap is stable across resolutions, the artifact theory is refuted.
#[test]
#[ignore = "two generation ladders + two 0.21mm sims; run with --ignored --nocapture"]
fn p2g_measurement_aliasing_probe() {
    let cancel = AtomicBool::new(false);
    let run = |label: &str, op: OperationConfig| {
        let mut s = ProjectSession::load(&wanaka_project_path()).expect("load wanaka.toml");
        let n = s.toolpath_count();
        let finish_idx = enabled_finish_index(&s);
        s.set_toolpath_operation(finish_idx, op).expect("swap op");
        let enabled: Vec<usize> = (0..n)
            .filter(|&i| s.get_toolpath_config(i).is_some_and(|tc| tc.enabled))
            .collect();
        let mut pending: Vec<usize> = Vec::new();
        for &i in &enabled {
            if s.generate_toolpath(i, &cancel).is_err() {
                pending.push(i);
            }
        }
        while !pending.is_empty() {
            s.run_simulation(&SimulationOptions::default(), &cancel)
                .expect("ladder sim");
            let before = pending.len();
            pending.retain(|&i| s.generate_toolpath(i, &cancel).is_err());
            assert!(pending.len() < before, "ladder stalled: {pending:?}");
        }
        let opts = SimulationOptions {
            resolution: 0.21,
            ..Default::default()
        };
        s.run_simulation(&opts, &cancel).expect("0.21mm sim");
        let bm = build_band_map(&s);
        fidelity_report(&format!("p2g21_{label}"), &s, &bm);
    };

    run("B", OperationConfig::UnifiedFinish(ab_unified_config()));
    run(
        "D",
        OperationConfig::Scallop(ScallopConfig {
            scallop_height: 0.011,
            tolerance: 0.05,
            direction: ScallopDirection::OutsideIn,
            continuous: true,
            slope_from: 0.0,
            slope_to: 90.0,
            feed_rate: 3000.0,
            plunge_rate: 150.0,
            stock_to_leave: 0.0,
            spindle_rpm: Some(21000),
        }),
    );
}

/// P2.g stamper probe — THE decisive experiment after every proxy test
/// failed to separate the branches (geometry equal, gap survives LUT fix +
/// resolution change): re-stamp both branches' EXACT conditioned op-8
/// moves (from the `p2g_session_op8_dump` artifacts) onto a fresh flat
/// dexel stock over the bad window, then diff each column's stamped top
/// against the exact-profile envelope (min over densely resampled
/// segments of z + height_at_radius(d)). Whatever the stamper does
/// differently between the two move streams shows up here directly,
/// isolated from roughing, measurement meshing, and deviation attribution.
#[test]
#[ignore = "needs target/p2f_fidelity/p2g_sess_*_moves.txt from p2g_session_op8_dump"]
fn p2g_stamp_probe() {
    use rs_cam_core::ToolpathId;
    use rs_cam_core::dexel_stock::{StockCutDirection, TriDexelStock};
    use rs_cam_core::geo::{BoundingBox3, P3};
    use rs_cam_core::tool::{MillingCutter, TaperedBallEndmill};
    use rs_cam_core::toolpath::{MoveIntent, Toolpath};

    // Bad window (sess/emission frame) + margin for tool radius.
    const WX0: f64 = 78.35;
    const WY0: f64 = 98.05;
    const WX1: f64 = 90.35;
    const WY1: f64 = 110.05;
    const MARGIN: f64 = 4.0;

    let cutter = TaperedBallEndmill::new(1.0, 7.0, 6.0, 25.0);
    let parse_intent = |s: &str| match s {
        "FinishingCut" => MoveIntent::FinishingCut,
        "EntryPlunge" => MoveIntent::EntryPlunge,
        "Linking" => MoveIntent::Linking,
        "Retract" => MoveIntent::Retract,
        _ => MoveIntent::Unknown,
    };

    let load = |tag: &str| -> Toolpath {
        let path = p2f_output_dir().join(format!("p2g_sess_{tag}_moves.txt"));
        let text = std::fs::read_to_string(&path).expect("run p2g_session_op8_dump first");
        let mut tp = Toolpath::new();
        for line in text.lines() {
            let p: Vec<&str> = line.split_whitespace().collect();
            let (kind, intent) = (p[0], parse_intent(p[1]));
            let target = P3::new(
                p[2].parse().expect("x"),
                p[3].parse().expect("y"),
                p[4].parse().expect("z"),
            );
            match kind {
                "R" => tp.rapid_to_with_intent(target, intent),
                "L" => tp.feed_to_with_intent(target, 3000.0, intent),
                "ACW" => tp.arc_cw_to_with_intent(
                    target,
                    p[5].parse().expect("i"),
                    p[6].parse().expect("j"),
                    3000.0,
                    intent,
                ),
                "ACCW" => tp.arc_ccw_to_with_intent(
                    target,
                    p[5].parse().expect("i"),
                    p[6].parse().expect("j"),
                    3000.0,
                    intent,
                ),
                other => panic!("unknown move kind {other}"),
            }
        }
        tp
    };

    // Exact envelope from the toolpath's cutting polyline (arcs already
    // near-linear at this scale are still sampled as chords here — the
    // stamper sees the same chords via its own arc interpolation, and the
    // dump's arc count in this window is zero).
    let envelope = |tp: &Toolpath, qx: f64, qy: f64| -> f64 {
        let mut best = f64::INFINITY;
        let r_max = cutter.radius();
        let mut prev: Option<P3> = None;
        for m in &tp.moves {
            let t = m.target;
            if let (Some(a), false) = (
                prev,
                matches!(m.move_type, rs_cam_core::toolpath::MoveType::Rapid),
            ) {
                // reject far segments
                if !(a.x.max(t.x) < qx - r_max
                    || a.x.min(t.x) > qx + r_max
                    || a.y.max(t.y) < qy - r_max
                    || a.y.min(t.y) > qy + r_max)
                {
                    let seg = ((t.x - a.x).powi(2) + (t.y - a.y).powi(2)).sqrt();
                    let n = ((seg / 0.02).ceil() as usize).max(1);
                    for k in 0..=n {
                        let f = k as f64 / n as f64;
                        let px = a.x + f * (t.x - a.x);
                        let py = a.y + f * (t.y - a.y);
                        let pz = a.z + f * (t.z - a.z);
                        let d = ((px - qx).powi(2) + (py - qy).powi(2)).sqrt();
                        if d <= r_max
                            && let Some(h) = cutter.height_at_radius(d)
                        {
                            best = best.min(pz + h);
                        }
                    }
                }
            }
            prev = Some(t);
        }
        best
    };

    for (tag, sample_step) in [("b75", 0.25), ("d", 0.25), ("b75", 0.05), ("d", 0.05)] {
        let tp = load(tag);
        let bbox = BoundingBox3 {
            min: P3::new(WX0 - MARGIN, WY0 - MARGIN, 10.0),
            max: P3::new(WX1 + MARGIN, WY1 + MARGIN, 26.0),
        };
        let mut stock = TriDexelStock::from_bounds(&bbox, 0.25);
        let never_cancel = || false;
        let _ = stock
            .simulate_toolpath_with_metrics_with_cancel(
                &tp,
                &cutter,
                StockCutDirection::FromTop,
                ToolpathId(0),
                21_000,
                2,
                5000.0,
                sample_step,
                None,
                &[],
                &[],
                false,
                &never_cancel,
            )
            .expect("stamp");

        // Per-column diff inside the window (skip the margin).
        let mut errs: Vec<f64> = Vec::new();
        let mut y = WY0 + 0.25;
        while y < WY1 - 0.25 {
            let mut x = WX0 + 0.25;
            while x < WX1 - 0.25 {
                let env = envelope(&tp, x, y);
                if env < 23.5
                    && let Some(top) = stock.max_top_z_in_disc(x, y, 0.05)
                {
                    errs.push(top - env);
                }
                x += 0.25;
            }
            y += 0.25;
        }
        errs.sort_by(|a, b| a.partial_cmp(b).expect("no NaN"));
        let q = |p: f64| errs[((errs.len() - 1) as f64 * p) as usize] * 1000.0;
        let over5 = errs.iter().filter(|&&e| e > 0.005).count();
        eprintln!(
            "[{tag} step {sample_step}] n={} stamped-minus-envelope um: p10={:.1} p50={:.1} p90={:.1} p99={:.1} | >5um: {:.1}%",
            errs.len(),
            q(0.10),
            q(0.50),
            q(0.90),
            q(0.99),
            100.0 * over5 as f64 / errs.len() as f64
        );
    }
}

/// P2.g chain-stage attribution probe — follow-up to `p2g_stamp_probe`
/// after the z-fix acceptance rerun showed the instrument gap SURVIVES
/// envelope-exact stamping (B75 mid-steep on-size 30 713 vs D 44 366;
/// on-size + first-leftover-bin sums nearly equal → a ~5–10 µm shift at
/// the +0.01 edge). Isolated op-8 stamping is clean and the branch
/// toolpaths are geometrically equal, so the divergence must enter in
/// the FULL-CHAIN path between "op-8 stamps on fresh stock" and the
/// per-vertex deviations. This probe runs the real chain per branch and
/// reads every intermediate the simulator retains, per dexel-grid cell
/// in the bad window (sess frame [78.35,90.35]×[98.05,110.05]):
///
///   rough = `prior_stocks[op8]` z-grid top       (floor op-8 inherits)
///   post  = `prior_stocks[successor]` z-grid top (floor op-8 leaves)
///   env   = exact-profile envelope of op-8's generated moves
///   mesh  = max measurement-mesh vertex z bucketed to the same cell
///   dev   = signed max-|deviation| of those vertices
///
/// Within-branch, `post − min(rough, env)` isolates chain stamping
/// (rough-aware); `mesh − post` isolates the meshing/deviation stage
/// (cells any LATER op's envelope could touch are excluded). Cross-
/// branch per-cell deltas at each stage show WHERE the B75−D shift
/// first appears: rough (ladder divergence), env (generation), post
/// (chain stamping), mesh/dev (measurement).
#[test]
#[ignore = "two full generation ladders + 0.25mm measurement sims; run with --ignored --nocapture"]
fn p2g_chain_stage_probe() {
    use std::collections::HashMap;
    use std::io::Write as _;

    use rs_cam_core::tool::{MillingCutter, TaperedBallEndmill};
    use rs_cam_core::toolpath::{MoveType, Toolpath};

    // Bad window (sess/emission frame), same as `p2g_stamp_probe`.
    const WX0: f64 = 78.35;
    const WY0: f64 = 98.05;
    const WX1: f64 = 90.35;
    const WY1: f64 = 110.05;
    // sess → model frame (2026-07-09 transform scan, rot90 setup):
    // model_x = sess_y − 21.25, model_y = 126.25 − sess_x,
    // model_z = sess_z − 20.
    const SESS_X_FROM_MODEL_Y: f64 = 126.25;
    const SESS_Y_OFFSET: f64 = 21.25;
    const SESS_Z_OFFSET: f64 = 20.0;
    /// Ownership margin between rough floor and finish envelope (mm).
    const OWNER_EPS: f64 = 0.02;

    #[derive(Clone, Copy)]
    struct Cell {
        x: f64,
        y: f64,
        rough: f64,
        post: f64,
        env: f64,
        /// min over all LATER ops' envelopes (could they lower this cell?)
        env_after: f64,
        /// max mesh vertex z in this cell, sess frame
        mesh_top: f64,
        /// signed max-|deviation| of this cell's vertices
        dev: f32,
        nverts: usize,
    }

    let cutter = TaperedBallEndmill::new(1.0, 7.0, 6.0, 25.0);

    // Exact-profile envelope of a toolpath's cutting moves at (qx, qy) —
    // same construction as `p2g_stamp_probe`.
    let envelope = |tp: &Toolpath, qx: f64, qy: f64| -> f64 {
        let mut best = f64::INFINITY;
        let r_max = cutter.radius();
        let mut prev: Option<rs_cam_core::geo::P3> = None;
        for m in &tp.moves {
            let t = m.target;
            if let (Some(a), false) = (prev, matches!(m.move_type, MoveType::Rapid))
                && !(a.x.max(t.x) < qx - r_max
                    || a.x.min(t.x) > qx + r_max
                    || a.y.max(t.y) < qy - r_max
                    || a.y.min(t.y) > qy + r_max)
            {
                let seg = ((t.x - a.x).powi(2) + (t.y - a.y).powi(2)).sqrt();
                let n = ((seg / 0.02).ceil() as usize).max(1);
                for k in 0..=n {
                    let f = k as f64 / n as f64;
                    let px = a.x + f * (t.x - a.x);
                    let py = a.y + f * (t.y - a.y);
                    let pz = a.z + f * (t.z - a.z);
                    let d = ((px - qx).powi(2) + (py - qy).powi(2)).sqrt();
                    if d <= r_max
                        && let Some(h) = cutter.height_at_radius(d)
                    {
                        best = best.min(pz + h);
                    }
                }
            }
            prev = Some(t);
        }
        best
    };

    let pct = |v: &[f64], p: f64| -> f64 {
        if v.is_empty() {
            return f64::NAN;
        }
        v[((v.len() - 1) as f64 * p) as usize] * 1000.0
    };
    let sorted = |mut v: Vec<f64>| -> Vec<f64> {
        v.sort_by(|a, b| a.partial_cmp(b).expect("no NaN"));
        v
    };

    let cancel = AtomicBool::new(false);
    let run = |label: &str, op: OperationConfig| -> HashMap<(usize, usize), Cell> {
        let mut s = ProjectSession::load(&wanaka_project_path()).expect("load wanaka.toml");
        let n = s.toolpath_count();
        let finish_idx = enabled_finish_index(&s);
        s.set_toolpath_operation(finish_idx, op).expect("swap op");
        let enabled: Vec<usize> = (0..n)
            .filter(|&i| s.get_toolpath_config(i).is_some_and(|tc| tc.enabled))
            .collect();
        let mut pending: Vec<usize> = Vec::new();
        for &i in &enabled {
            if s.generate_toolpath(i, &cancel).is_err() {
                pending.push(i);
            }
        }
        while !pending.is_empty() {
            s.run_simulation(&SimulationOptions::default(), &cancel)
                .expect("ladder sim");
            let before = pending.len();
            pending.retain(|&i| s.generate_toolpath(i, &cancel).is_err());
            assert!(pending.len() < before, "ladder stalled: {pending:?}");
        }
        run_measurement_sim(&mut s);

        let op8_id = s.get_toolpath_config(finish_idx).expect("op8 config").id;
        let sim = s.simulation_result().expect("sim result");
        let pos = sim
            .boundaries
            .iter()
            .position(|b| b.id == op8_id)
            .expect("op8 boundary");
        let rough_arc = sim.prior_stocks.get(&op8_id).expect("op8 prior stock");
        let rough = &rough_arc.z_grid;

        let ann = s.get_result(finish_idx).expect("op8 generated").annotated();
        let tp = &ann.toolpath;

        // Op-8 is the LAST op in the wanaka chain, so no successor
        // prior-stock snapshot captures the post-op8 state. Reproduce it
        // by re-stamping op-8's generated toolpath onto a clone of its
        // prior stock through the SAME metrics stamping path (and span /
        // transit inputs) the measurement sim used — deterministic, so
        // this is byte-faithful to the sim's final group stock.
        let intent_transits = ann.transit_moves_bitmap_from_intents();
        let (span_paths_by_move, transit_moves) = if ann.spans_valid {
            let mut transit = ann.transit_moves_bitmap();
            for (slot, from_intent) in transit.iter_mut().zip(intent_transits) {
                *slot = *slot || from_intent;
            }
            (ann.span_paths_by_move(), transit)
        } else {
            (
                vec![Vec::new(); tp.moves.len()],
                ann.transit_moves_bitmap_from_intents(),
            )
        };
        let mut post_stock = (**rough_arc).clone();
        let lut = rs_cam_core::radial_profile::RadialProfileLUT::from_cutter(
            &cutter,
            rs_cam_core::radial_profile::LUT_SAMPLES,
        );
        let never_cancel = || false;
        post_stock
            .simulate_toolpath_with_lut_metrics_cancel(
                tp,
                &lut,
                &cutter,
                cutter.radius(),
                rs_cam_core::dexel_stock::StockCutDirection::FromTop,
                rs_cam_core::ToolpathId(0),
                21_000,
                2,
                5000.0,
                0.25,
                None,
                &span_paths_by_move,
                &transit_moves,
                false,
                &never_cancel,
            )
            .expect("re-stamp op8 onto prior stock");
        let post = &post_stock.z_grid;
        eprintln!(
            "[{label}] op8 boundary #{pos} (last={}); grid {}x{} cell={:.6} origin=({:.4},{:.4})",
            pos + 1 == sim.boundaries.len(),
            rough.rows,
            rough.cols,
            rough.cell_size,
            rough.origin_u,
            rough.origin_v
        );

        let mut cells: HashMap<(usize, usize), Cell> = HashMap::new();
        for row in 0..rough.rows {
            for col in 0..rough.cols {
                let (x, y) = rough.cell_to_world(row, col);
                if !(WX0..=WX1).contains(&x) || !(WY0..=WY1).contains(&y) {
                    continue;
                }
                let (Some(rt), Some(pt)) = (rough.top_z_at(row, col), post.top_z_at(row, col))
                else {
                    continue;
                };
                cells.insert(
                    (row, col),
                    Cell {
                        x,
                        y,
                        rough: f64::from(rt),
                        post: f64::from(pt),
                        env: envelope(tp, x, y),
                        env_after: f64::INFINITY,
                        mesh_top: f64::NEG_INFINITY,
                        dev: f32::NAN,
                        nverts: 0,
                    },
                );
            }
        }

        // Envelopes of every LATER op: cells a later op could lower are
        // excluded from the mesh-stage stats (post is not final there).
        for later in &sim.boundaries[pos + 1..] {
            let later_idx = (0..n)
                .find(|&i| s.get_toolpath_config(i).is_some_and(|tc| tc.id == later.id))
                .expect("later boundary has a config");
            if let Some(res) = s.get_result(later_idx) {
                let ltp = &res.annotated().toolpath;
                for c in cells.values_mut() {
                    c.env_after = c.env_after.min(envelope(ltp, c.x, c.y));
                }
            }
        }

        // Mesh + deviation stage: bucket measurement-mesh vertices into
        // the same grid cells (model → sess frame).
        let devs = sim.deviations.as_ref().expect("deviations");
        let verts = &sim.mesh.vertices;
        let mut window_bins = [0usize; DEV_BIN_COUNT];
        let mut window_verts = 0usize;
        // Per-vertex ground-truth dump: `vz − dev` is the model's exact
        // surface z at the vertex xy (dev is defined as vz − model_z), and
        // `env_sess` is the exact machined-surface z at the same xy. Their
        // difference (modulo the constant sess→model z offset, identical
        // for both branches) is the TRUE leftover at vertex sampling —
        // immune to the corner-averaged vertex z the deviations use.
        let vpath = p2f_output_dir().join(format!("p2g_chain_{label}_verts.txt"));
        let mut vf =
            std::io::BufWriter::new(std::fs::File::create(&vpath).expect("create vert dump"));
        for (i, &d) in devs.iter().enumerate() {
            let vx = f64::from(verts[i * 3]);
            let vy = f64::from(verts[i * 3 + 1]);
            let vz = f64::from(verts[i * 3 + 2]);
            let sx = SESS_X_FROM_MODEL_Y - vy;
            let sy = vx + SESS_Y_OFFSET;
            if !(WX0..=WX1).contains(&sx) || !(WY0..=WY1).contains(&sy) {
                continue;
            }
            if d != 0.0 {
                window_verts += 1;
                let bin = DEV_EDGES
                    .iter()
                    .position(|&e| d < e)
                    .unwrap_or(DEV_BIN_COUNT - 1);
                window_bins[bin] += 1;
                let env_v = envelope(tp, sx, sy);
                writeln!(vf, "{vx:.4} {vy:.4} {vz:.6} {d:.6} {env_v:.6}",).expect("write vert");
            }
            if let Some(key) = rough.world_to_cell(sx, sy)
                && let Some(cell) = cells.get_mut(&key)
            {
                cell.mesh_top = cell.mesh_top.max(vz + SESS_Z_OFFSET);
                cell.nverts += 1;
                if d != 0.0 && (cell.dev.is_nan() || d.abs() > cell.dev.abs()) {
                    cell.dev = d;
                }
            }
        }
        drop(vf);
        eprintln!("[{label}] vert dump -> {}", vpath.display());

        // Within-branch stage errors.
        let mut e_env = Vec::new();
        let mut e_rough = Vec::new();
        let mut e_cont = Vec::new();
        let mut e_mesh = Vec::new();
        let mut mesh_excluded = 0usize;
        for c in cells.values() {
            if c.env.is_finite() {
                let e = c.post - c.rough.min(c.env);
                if c.env < c.rough - OWNER_EPS {
                    e_env.push(e);
                } else if c.env > c.rough + OWNER_EPS {
                    e_rough.push(e);
                } else {
                    e_cont.push(e);
                }
            }
            if c.mesh_top > f64::NEG_INFINITY {
                if c.env_after < c.post + 0.005 {
                    mesh_excluded += 1;
                } else {
                    e_mesh.push(c.mesh_top - c.post);
                }
            }
        }
        let (e_env, e_rough, e_cont, e_mesh) = (
            sorted(e_env),
            sorted(e_rough),
            sorted(e_cont),
            sorted(e_mesh),
        );
        eprintln!(
            "[{label}] cells={} | post-min(rough,env) um p10/p50/p90: env-owned n={} {:.1}/{:.1}/{:.1} | rough-owned n={} {:.1}/{:.1}/{:.1} | contested n={} {:.1}/{:.1}/{:.1}",
            cells.len(),
            e_env.len(),
            pct(&e_env, 0.10),
            pct(&e_env, 0.50),
            pct(&e_env, 0.90),
            e_rough.len(),
            pct(&e_rough, 0.10),
            pct(&e_rough, 0.50),
            pct(&e_rough, 0.90),
            e_cont.len(),
            pct(&e_cont, 0.10),
            pct(&e_cont, 0.50),
            pct(&e_cont, 0.90),
        );
        eprintln!(
            "[{label}] mesh-post um (later-op-safe cells only, n={} excl={}): p10={:.1} p50={:.1} p90={:.1}",
            e_mesh.len(),
            mesh_excluded,
            pct(&e_mesh, 0.10),
            pct(&e_mesh, 0.50),
            pct(&e_mesh, 0.90),
        );
        let hist: Vec<String> = DEV_BIN_LABELS
            .iter()
            .zip(window_bins.iter())
            .map(|(l, n)| format!("{l}:{n}"))
            .collect();
        eprintln!("[{label}] window devs n={window_verts}: {}", hist.join(" "));

        // Per-cell dump for offline analysis.
        let path = p2f_output_dir().join(format!("p2g_chain_{label}_cells.txt"));
        let mut f = std::io::BufWriter::new(std::fs::File::create(&path).expect("create dump"));
        let mut keys: Vec<&(usize, usize)> = cells.keys().collect();
        keys.sort();
        for k in keys {
            let c = &cells[k];
            writeln!(
                f,
                "{} {} {:.4} {:.4} {:.6} {:.6} {:.6} {:.6} {:.6} {:.6} {}",
                k.0,
                k.1,
                c.x,
                c.y,
                c.rough,
                c.post,
                c.env,
                c.env_after,
                c.mesh_top,
                c.dev,
                c.nverts
            )
            .expect("write cell");
        }
        eprintln!("[{label}] cell dump -> {}", path.display());
        cells
    };

    let map_b = run("b75", OperationConfig::UnifiedFinish(ab_unified_config()));
    let map_d = run(
        "d",
        OperationConfig::Scallop(ScallopConfig {
            scallop_height: 0.011,
            tolerance: 0.05,
            direction: ScallopDirection::OutsideIn,
            continuous: true,
            slope_from: 0.0,
            slope_to: 90.0,
            feed_rate: 3000.0,
            plunge_rate: 150.0,
            stock_to_leave: 0.0,
            spindle_rpm: Some(21000),
        }),
    );

    // Cross-branch stage deltas on common cells: the stage where B75−D
    // first departs from ~0 is the mechanism.
    let mut d_rough = Vec::new();
    let mut d_env = Vec::new();
    let mut d_post = Vec::new();
    let mut d_mesh = Vec::new();
    let mut d_dev = Vec::new();
    for (k, cb) in &map_b {
        let Some(cd) = map_d.get(k) else { continue };
        d_rough.push(cb.rough - cd.rough);
        d_post.push(cb.post - cd.post);
        if cb.env.is_finite() && cd.env.is_finite() {
            d_env.push(cb.env - cd.env);
        }
        if cb.mesh_top > f64::NEG_INFINITY
            && cd.mesh_top > f64::NEG_INFINITY
            && cb.env_after >= cb.post + 0.005
            && cd.env_after >= cd.post + 0.005
        {
            d_mesh.push(cb.mesh_top - cd.mesh_top);
        }
        if !cb.dev.is_nan() && !cd.dev.is_nan() {
            d_dev.push(f64::from(cb.dev) - f64::from(cd.dev));
        }
    }
    eprintln!("== P2.g CHAIN STAGE DELTAS (B75 − D, um, per common cell) ==");
    for (name, v) in [
        ("rough", d_rough),
        ("env", d_env),
        ("post", d_post),
        ("mesh", d_mesh),
        ("dev", d_dev),
    ] {
        let v = sorted(v);
        let over2 = v.iter().filter(|&&e| e.abs() > 0.002).count();
        let over5 = v.iter().filter(|&&e| e.abs() > 0.005).count();
        eprintln!(
            "{name:<6} n={:<5} p10={:>7.1} p50={:>7.1} p90={:>7.1} | |d|>2um {:.1}% |d|>5um {:.1}%",
            v.len(),
            pct(&v, 0.10),
            pct(&v, 0.50),
            pct(&v, 0.90),
            100.0 * over2 as f64 / (v.len() as f64).max(1.0),
            100.0 * over5 as f64 / (v.len() as f64).max(1.0),
        );
    }
}

/// P2.g dense-envelope probe — evaluates both branches' exact machined
/// envelopes on a DENSE 0.05 mm grid over the bad window (from the
/// current `p2g_sess_*_moves.txt` dumps — no session, no chains) and
/// dumps f32 grids for offline ground-truth analysis.
///
/// RESULT (2026-07-10): the dense quantile comparison (70 k points,
/// model-free) put B75−D at ±11 µm per quantile, mean −1.4 µm — the
/// true surfaces are EQUAL in this (ring-covered) window, killing the
/// vertex instrument's in-window gap as artifact. The band-wide COLUMNS
/// gap that remains (+.05-bin excess 13 506 columns = 16.3 % of the
/// mid-steep band) is REAL and matches the known ring under-coverage
/// (~8 %) + collar share (~8 %) cut at raster cusp — the collar /
/// coverage fix, not an instrument problem.
#[test]
#[ignore = "needs target/p2f_fidelity/p2g_sess_*_moves.txt; ~2 min; run with --ignored --nocapture"]
fn p2g_dense_env_probe() {
    use rs_cam_core::geo::P3;
    use rs_cam_core::tool::{MillingCutter, TaperedBallEndmill};

    const WX0: f64 = 78.35;
    const WY0: f64 = 98.05;
    const WX1: f64 = 90.35;
    const WY1: f64 = 110.05;
    const MARGIN: f64 = 0.6;
    const STEP: f64 = 0.05;

    let cutter = TaperedBallEndmill::new(1.0, 7.0, 6.0, 25.0);
    let r_max = cutter.radius();

    for tag in ["b75", "d"] {
        let path = p2f_output_dir().join(format!("p2g_sess_{tag}_moves.txt"));
        let text = std::fs::read_to_string(&path).expect("run p2g_session_op8_dump first");
        // Cutting segments only (skip rapids), as (start, end) pairs.
        let mut segs: Vec<(P3, P3)> = Vec::new();
        let mut prev: Option<P3> = None;
        for line in text.lines() {
            let p: Vec<&str> = line.split_whitespace().collect();
            let target = P3::new(
                p[2].parse().expect("x"),
                p[3].parse().expect("y"),
                p[4].parse().expect("z"),
            );
            if p[0] != "R"
                && let Some(a) = prev
            {
                segs.push((a, target));
            }
            prev = Some(target);
        }
        // Bucket segments by x-span for fast queries.
        let gx0 = WX0 - MARGIN - r_max;
        let ncols = (((WX1 + MARGIN + r_max) - gx0) / 1.0).ceil() as usize + 1;
        let mut buckets: Vec<Vec<usize>> = vec![Vec::new(); ncols];
        for (i, (a, b)) in segs.iter().enumerate() {
            let lo = (a.x.min(b.x) - r_max - gx0).max(0.0) as usize;
            let hi = (((a.x.max(b.x) + r_max - gx0) as usize) + 1).min(ncols - 1);
            for bucket in buckets.iter_mut().take(hi + 1).skip(lo) {
                bucket.push(i);
            }
        }

        let nx = (((WX1 + MARGIN) - (WX0 - MARGIN)) / STEP).round() as usize + 1;
        let ny = (((WY1 + MARGIN) - (WY0 - MARGIN)) / STEP).round() as usize + 1;
        let mut grid: Vec<f32> = vec![f32::NAN; nx * ny];
        for iy in 0..ny {
            let qy = WY0 - MARGIN + iy as f64 * STEP;
            for ix in 0..nx {
                let qx = WX0 - MARGIN + ix as f64 * STEP;
                let mut best = f64::INFINITY;
                #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
                let bi = ((qx - gx0).max(0.0) as usize).min(ncols - 1);
                for &si in &buckets[bi] {
                    let (a, t) = segs[si];
                    if a.x.max(t.x) < qx - r_max
                        || a.x.min(t.x) > qx + r_max
                        || a.y.max(t.y) < qy - r_max
                        || a.y.min(t.y) > qy + r_max
                    {
                        continue;
                    }
                    let seg = ((t.x - a.x).powi(2) + (t.y - a.y).powi(2)).sqrt();
                    let n = ((seg / 0.02).ceil() as usize).max(1);
                    for k in 0..=n {
                        let f = k as f64 / n as f64;
                        let px = a.x + f * (t.x - a.x);
                        let py = a.y + f * (t.y - a.y);
                        let pz = a.z + f * (t.z - a.z);
                        let d = ((px - qx).powi(2) + (py - qy).powi(2)).sqrt();
                        if d <= r_max
                            && let Some(h) = cutter.height_at_radius(d)
                        {
                            best = best.min(pz + h);
                        }
                    }
                }
                if best.is_finite() {
                    grid[iy * nx + ix] = best as f32;
                }
            }
        }
        let out = p2f_output_dir().join(format!("p2g_dense_env_{tag}_{ny}x{nx}.f32"));
        let bytes: Vec<u8> = grid.iter().flat_map(|v| v.to_le_bytes()).collect();
        std::fs::write(&out, bytes).expect("write dense env grid");
        eprintln!(
            "[{tag}] dense env {ny}x{nx} step {STEP} origin=({:.4},{:.4}) -> {}",
            WX0 - MARGIN,
            WY0 - MARGIN,
            out.display()
        );
    }
}

/// P2.g coverage-overlay dump: one B75 chain + measurement sim, then dump
/// the pointwise column deviations (world frame — the SAME frame as the
/// band map and the bare-mesh ring dump `p2g_b75_mid_moves.txt`) with
/// their band codes. Offline overlay answers whether the +.05-bin excess
/// columns sit in the ring-uncovered / collar zones (the 16.3 % ≈ 8 %
/// under-coverage + 8 % collar attribution) — and becomes the acceptance
/// baseline for the collar/coverage fix.
#[test]
#[ignore = "one generation ladder + 0.25mm sim; run with --ignored --nocapture"]
fn p2g_column_overlay_dump() {
    for (label, op) in [
        // None = run the project's enabled finish op as-is (the live v2
        // unified op — B75 family).
        ("b75", None),
        (
            "d",
            Some(OperationConfig::Scallop(ScallopConfig {
                scallop_height: 0.011,
                tolerance: 0.05,
                direction: ScallopDirection::OutsideIn,
                continuous: true,
                slope_from: 0.0,
                slope_to: 90.0,
                feed_rate: 3000.0,
                plunge_rate: 150.0,
                stock_to_leave: 0.0,
                spindle_rpm: Some(21000),
            })),
        ),
    ] {
        column_overlay_dump_branch(label, op);
    }
}

fn column_overlay_dump_branch(label: &str, op: Option<OperationConfig>) {
    use std::io::Write as _;

    let cancel = AtomicBool::new(false);
    let mut s = ProjectSession::load(&wanaka_project_path()).expect("load wanaka.toml");
    let n = s.toolpath_count();
    eprintln!("[{label}] resolving finish op");
    let finish_idx = enabled_finish_index(&s);
    if let Some(op) = op {
        s.set_toolpath_operation(finish_idx, op).expect("swap op");
    }
    let enabled: Vec<usize> = (0..n)
        .filter(|&i| s.get_toolpath_config(i).is_some_and(|tc| tc.enabled))
        .collect();
    let mut pending: Vec<usize> = Vec::new();
    for &i in &enabled {
        if s.generate_toolpath(i, &cancel).is_err() {
            pending.push(i);
        }
    }
    while !pending.is_empty() {
        s.run_simulation(&SimulationOptions::default(), &cancel)
            .expect("ladder sim");
        let before = pending.len();
        pending.retain(|&i| s.generate_toolpath(i, &cancel).is_err());
        assert!(pending.len() < before, "ladder stalled: {pending:?}");
    }
    run_measurement_sim(&mut s);

    let tc = s.get_toolpath_config(finish_idx).expect("op8 config");
    let result_state = match s.get_result(finish_idx) {
        Some(r) => format!("Some(moves={})", r.annotated().toolpath.moves.len()),
        None => "None".to_owned(),
    };
    eprintln!(
        "[{label}] op8 kind={:?} result={result_state}",
        tc.operation.op_type()
    );
    {
        let sim = s.simulation_result().expect("sim result");
        let names: Vec<&str> = sim.boundaries.iter().map(|b| b.name.as_str()).collect();
        eprintln!("[{label}] sim boundaries: {names:?}");
    }

    let bm = build_band_map(&s);
    let sim = s.simulation_result().expect("sim result");
    let cols = sim
        .column_deviations
        .as_ref()
        .expect("column deviations present");
    let path = p2f_output_dir().join(format!("p2g_columns_{label}.txt"));
    let mut f = std::io::BufWriter::new(std::fs::File::create(&path).expect("create dump"));
    let mut kept = 0usize;
    for cd in cols {
        let code = bm.code_at(cd.x, cd.y);
        if code == 0 {
            continue;
        }
        writeln!(
            f,
            "{:.4} {:.4} {:.6} {} {}",
            cd.x, cd.y, cd.dev, code, cd.group
        )
        .expect("write column");
        kept += 1;
    }
    eprintln!(
        "[{label}] columns total={} on-region={kept} -> {}",
        cols.len(),
        path.display()
    );
}

/// P2.g THREE-WAY instrument probe (2026-07-09 late-night design) — the
/// decisive run for the reopened Task-1 contradiction: the sim's stamped
/// stocks show a real unified-vs-D on-size gap (48.5 % vs 67.7 % band-wide,
/// reproduces on the live v2 op), but exact envelopes of (what we believed
/// were) the same toolpaths agree to mean −1.3 µm. Three individually-
/// validated measurements disagree pairwise — so ONE RUN per branch records
/// every stage on the same lattice:
///
///   (d) `env_pre`  — exact envelope of the finish toolpath captured
///                    immediately after generation, BEFORE any simulation
///                    ever stamps it (pre-mutation, air-cut-filter suspect);
///   (a) `env_read` — exact envelope of the toolpath AS READ back after the
///                    measurement sim;
///   (b) `post`     — re-stamp of the as-read toolpath onto the sim's own
///                    pre-finish prior stock (chain-probe construction,
///                    call-identical stamping args);
///   (c) `sim_top`  — the SIM'S OWN final column tops
///                    (`column_deviations[].top_z`, group-filtered).
///
/// Verdict key:
///   pre ≠ read (ptr or moves)  → the stored toolpath IS mutated between
///                                generation and read-back — as-stamped ≠
///                                as-read confirmed;
///   (c) ≠ (b)                  → the sim's in-chain stamping differs from
///                                an isolated identical re-stamp of the
///                                same moves onto the same prior stock;
///   all pairs equal in-run but the branch gap persists band-wide in both
///   (c)−(c) and (b)−(b)        → the gap is REAL machined geometry and the
///                                earlier "envelopes equal" verdict was a
///                                cross-run comparison of different
///                                toolpaths (ladder-dependent generation).
///
/// Targets the ENABLED finish op — wanaka.toml is user-live and
/// "3D Finish 6" is disabled (see `column_overlay_dump_branch`).
#[test]
#[ignore = "two full generation ladders + 0.25mm measurement sims; run with --ignored --nocapture"]
fn p2g_three_way_probe() {
    use std::collections::HashMap;
    use std::io::Write as _;
    use std::sync::Arc;

    use rs_cam_core::tool::{MillingCutter, TaperedBallEndmill};
    use rs_cam_core::toolpath::{MoveType, Toolpath};
    use rs_cam_core::toolpath_spans::AnnotatedToolpath;

    // Bad window (sess/emission frame), same as `p2g_chain_stage_probe`.
    const WX0: f64 = 78.35;
    const WY0: f64 = 98.05;
    const WX1: f64 = 90.35;
    const WY1: f64 = 110.05;

    let cutter = TaperedBallEndmill::new(1.0, 7.0, 6.0, 25.0);

    // Exact-profile envelope of a toolpath's cutting moves at (qx, qy) —
    // same construction as `p2g_stamp_probe` / `p2g_chain_stage_probe`.
    let envelope = |tp: &Toolpath, qx: f64, qy: f64| -> f64 {
        let mut best = f64::INFINITY;
        let r_max = cutter.radius();
        let mut prev: Option<rs_cam_core::geo::P3> = None;
        for m in &tp.moves {
            let t = m.target;
            if let (Some(a), false) = (prev, matches!(m.move_type, MoveType::Rapid))
                && !(a.x.max(t.x) < qx - r_max
                    || a.x.min(t.x) > qx + r_max
                    || a.y.max(t.y) < qy - r_max
                    || a.y.min(t.y) > qy + r_max)
            {
                let seg = ((t.x - a.x).powi(2) + (t.y - a.y).powi(2)).sqrt();
                let n = ((seg / 0.02).ceil() as usize).max(1);
                for k in 0..=n {
                    let f = k as f64 / n as f64;
                    let px = a.x + f * (t.x - a.x);
                    let py = a.y + f * (t.y - a.y);
                    let pz = a.z + f * (t.z - a.z);
                    let d = ((px - qx).powi(2) + (py - qy).powi(2)).sqrt();
                    if d <= r_max
                        && let Some(h) = cutter.height_at_radius(d)
                    {
                        best = best.min(pz + h);
                    }
                }
            }
            prev = Some(t);
        }
        best
    };

    let pct = |v: &[f64], p: f64| -> f64 {
        if v.is_empty() {
            return f64::NAN;
        }
        v[((v.len() - 1) as f64 * p) as usize] * 1000.0
    };
    let sorted = |mut v: Vec<f64>| -> Vec<f64> {
        v.sort_by(|a, b| a.partial_cmp(b).expect("no NaN"));
        v
    };
    let report = |name: &str, v: Vec<f64>| {
        let v = sorted(v);
        let share = |t: f64| {
            100.0 * v.iter().filter(|&&e| e.abs() > t).count() as f64 / (v.len() as f64).max(1.0)
        };
        eprintln!(
            "  {name:<24} n={:<6} um p10={:>7.1} p50={:>7.1} p90={:>7.1} | |d|>2um {:>5.1}% >5um {:>5.1}% >10um {:>5.1}%",
            v.len(),
            pct(&v, 0.10),
            pct(&v, 0.50),
            pct(&v, 0.90),
            share(0.002),
            share(0.005),
            share(0.010),
        );
    };

    /// Per window cell: the four stages.
    #[derive(Clone, Copy)]
    struct WCell {
        env_pre: f64,
        env_read: f64,
        post: f64,
        sim_top: f64,
    }
    /// Band-wide (no envelope): sim top vs re-stamp top + band code.
    #[derive(Clone, Copy)]
    struct BCell {
        sim_top: f64,
        post: f64,
        dev: f32,
        band: u8,
    }
    struct BranchOut {
        window: HashMap<(usize, usize), WCell>,
        band: HashMap<(usize, usize), BCell>,
    }

    let cancel = AtomicBool::new(false);
    let run = |label: &str, op: Option<OperationConfig>| -> BranchOut {
        let mut s = ProjectSession::load(&wanaka_project_path()).expect("load wanaka.toml");
        let n = s.toolpath_count();
        eprintln!("[{label}] resolving finish op");
        let finish_idx = enabled_finish_index(&s);
        if let Some(op) = op {
            s.set_toolpath_operation(finish_idx, op).expect("swap op");
        }
        let op_id = s.get_toolpath_config(finish_idx).expect("cfg").id;

        // Ladder-generate; snapshot the finish result Arc the moment its
        // generation succeeds — before any subsequent sim can see it.
        let enabled: Vec<usize> = (0..n)
            .filter(|&i| s.get_toolpath_config(i).is_some_and(|tc| tc.enabled))
            .collect();
        let mut pending: Vec<usize> = Vec::new();
        for &i in &enabled {
            if s.generate_toolpath(i, &cancel).is_err() {
                pending.push(i);
            }
        }
        let snapshot = |s: &ProjectSession| -> Option<Arc<AnnotatedToolpath>> {
            s.get_result(finish_idx).map(|r| Arc::clone(r.annotated()))
        };
        let mut pre_arc: Option<Arc<AnnotatedToolpath>> = if pending.contains(&finish_idx) {
            None
        } else {
            snapshot(&s)
        };
        let mut sims_after_snapshot = 0usize;
        while !pending.is_empty() {
            s.run_simulation(&SimulationOptions::default(), &cancel)
                .expect("ladder sim");
            if pre_arc.is_some() {
                sims_after_snapshot += 1;
            }
            let before = pending.len();
            pending.retain(|&i| s.generate_toolpath(i, &cancel).is_err());
            assert!(pending.len() < before, "ladder stalled: {pending:?}");
            if pre_arc.is_none() && !pending.contains(&finish_idx) {
                pre_arc = snapshot(&s);
            }
        }
        let pre_arc = pre_arc.expect("finish op generated during ladder");
        run_measurement_sim(&mut s);
        let read_arc = snapshot(&s).expect("finish result after measurement sim");

        // Stage 0 verdict input: was the stored toolpath replaced/mutated
        // between generation and post-sim read-back?
        let same_object = Arc::ptr_eq(&pre_arc, &read_arc);
        let (pre_tp, read_tp) = (&pre_arc.toolpath, &read_arc.toolpath);
        let common = pre_tp.moves.len().min(read_tp.moves.len());
        let mut n_diff = 0usize;
        let mut max_dxyz = 0.0f64;
        let mut first_diffs: Vec<usize> = Vec::new();
        for i in 0..common {
            let (a, b) = (&pre_tp.moves[i], &read_tp.moves[i]);
            let d = (a.target.x - b.target.x)
                .abs()
                .max((a.target.y - b.target.y).abs())
                .max((a.target.z - b.target.z).abs());
            max_dxyz = max_dxyz.max(d);
            if d > 1e-9 || a.move_type != b.move_type || a.intent != b.intent {
                n_diff += 1;
                if first_diffs.len() < 10 {
                    first_diffs.push(i);
                }
            }
        }
        eprintln!(
            "[{label}] PRE-vs-READ toolpath: same_arc={same_object} moves {}->{} (ladder sims after snapshot: {sims_after_snapshot}, +1 measurement) | differing moves (common prefix)={n_diff} max|dxyz|={:.3}um first={:?}",
            pre_tp.moves.len(),
            read_tp.moves.len(),
            max_dxyz * 1e3,
            first_diffs
        );

        // (b): re-stamp as-read (and, if it differs, pre) onto the sim's
        // own pre-finish prior stock — call-identical to the sim's stamping
        // (same LUT, cutter, direction, sample step 0.25, spans + transits).
        let sim = s.simulation_result().expect("sim result");
        let rough_arc = Arc::clone(sim.prior_stocks.get(&op_id).expect("finish prior stock"));
        let pos = sim
            .boundaries
            .iter()
            .position(|b| b.id == op_id)
            .expect("finish boundary");
        assert_eq!(
            pos + 1,
            sim.boundaries.len(),
            "probe assumes the finish op is last in the chain"
        );
        let group_ord = s
            .setup_of_toolpath_id(op_id)
            .expect("finish op belongs to a setup");
        let lut = rs_cam_core::radial_profile::RadialProfileLUT::from_cutter(
            &cutter,
            rs_cam_core::radial_profile::LUT_SAMPLES,
        );
        let never_cancel = || false;
        let restamp = |ann: &AnnotatedToolpath| -> rs_cam_core::dexel_stock::TriDexelStock {
            let intent_transits = ann.transit_moves_bitmap_from_intents();
            let (span_paths_by_move, transit_moves) = if ann.spans_valid {
                let mut transit = ann.transit_moves_bitmap();
                for (slot, from_intent) in transit.iter_mut().zip(intent_transits) {
                    *slot = *slot || from_intent;
                }
                (ann.span_paths_by_move(), transit)
            } else {
                (
                    vec![Vec::new(); ann.toolpath.moves.len()],
                    ann.transit_moves_bitmap_from_intents(),
                )
            };
            let mut stock = (*rough_arc).clone();
            stock
                .simulate_toolpath_with_lut_metrics_cancel(
                    &ann.toolpath,
                    &lut,
                    &cutter,
                    cutter.radius(),
                    rs_cam_core::dexel_stock::StockCutDirection::FromTop,
                    rs_cam_core::ToolpathId(0),
                    21_000,
                    2,
                    5000.0,
                    0.25,
                    None,
                    &span_paths_by_move,
                    &transit_moves,
                    false,
                    &never_cancel,
                )
                .expect("re-stamp finish onto prior stock");
            stock
        };
        let post_read_stock = restamp(&read_arc);
        let post_read = &post_read_stock.z_grid;
        let post_pre_stock = if same_object && n_diff == 0 {
            None
        } else {
            Some(restamp(&pre_arc))
        };

        // (c): the sim's own column tops, group-filtered, indexed by the
        // grid (row, col) the sim recorded — no frame round-trip. (The
        // first probe run mapped cd world XY back through the sess
        // transform; that scrambled cells, so ColumnDeviation now carries
        // its grid coordinates.)
        let bm = build_band_map(&s);
        let cols = sim
            .column_deviations
            .as_ref()
            .expect("column deviations present");
        let grid = &rough_arc.z_grid;
        let mut band: HashMap<(usize, usize), BCell> = HashMap::new();
        let mut unmapped = 0usize;
        for cd in cols {
            if cd.group != group_ord {
                continue;
            }
            if cd.row >= grid.rows || cd.col >= grid.cols {
                unmapped += 1;
                continue;
            }
            let key = (cd.row, cd.col);
            let Some(pt) = post_read.top_z_at(key.0, key.1) else {
                continue;
            };
            band.insert(
                key,
                BCell {
                    sim_top: f64::from(cd.top_z),
                    post: f64::from(pt),
                    dev: cd.dev,
                    band: bm.code_at(cd.x, cd.y),
                },
            );
        }
        let total_group = cols.iter().filter(|cd| cd.group == group_ord).count();
        eprintln!(
            "[{label}] group {group_ord} columns={total_group} mapped={} out-of-grid={unmapped}",
            band.len()
        );

        // On-size shares from the sim's own devs (reproduce the gap in-run).
        for (bname, code) in [("mid-steep", 2u8), ("all-bands", 255u8)] {
            let devs: Vec<f32> = band
                .values()
                .filter(|c| code == 255 || c.band == code)
                .map(|c| c.dev)
                .collect();
            let on = devs.iter().filter(|d| d.abs() < 0.01).count();
            let plus05 = devs.iter().filter(|&&d| (0.01..0.05).contains(&d)).count();
            eprintln!(
                "[{label}] {bname}: n={} on-size {:.1}% +.05-bin {:.1}%",
                devs.len(),
                100.0 * on as f64 / (devs.len() as f64).max(1.0),
                100.0 * plus05 as f64 / (devs.len() as f64).max(1.0)
            );
        }

        // Within-branch stage deltas, band-wide: (c) − (b).
        eprintln!("[{label}] WITHIN-BRANCH stage deltas (band-wide, per column):");
        report(
            "sim_top - post(all)",
            band.values().map(|c| c.sim_top - c.post).collect(),
        );
        report(
            "sim_top - post(midsteep)",
            band.values()
                .filter(|c| c.band == 2)
                .map(|c| c.sim_top - c.post)
                .collect(),
        );

        // Window cells: envelopes (a)/(d) + (b) + (c).
        let mut window: HashMap<(usize, usize), WCell> = HashMap::new();
        for (key, bc) in &band {
            let (x, y) = grid.cell_to_world(key.0, key.1);
            if !(WX0..=WX1).contains(&x) || !(WY0..=WY1).contains(&y) {
                continue;
            }
            let env_read = envelope(read_tp, x, y);
            let env_pre = if same_object && n_diff == 0 {
                env_read
            } else {
                envelope(pre_tp, x, y)
            };
            window.insert(
                *key,
                WCell {
                    env_pre,
                    env_read,
                    post: bc.post,
                    sim_top: bc.sim_top,
                },
            );
        }
        eprintln!(
            "[{label}] WINDOW stage deltas (per cell, n={}):",
            window.len()
        );
        report(
            "env_read - env_pre",
            window.values().map(|c| c.env_read - c.env_pre).collect(),
        );
        report(
            "post - env_read",
            window
                .values()
                .filter(|c| c.env_read.is_finite())
                .map(|c| c.post - c.env_read)
                .collect(),
        );
        report(
            "sim_top - post",
            window.values().map(|c| c.sim_top - c.post).collect(),
        );
        report(
            "sim_top - env_read",
            window
                .values()
                .filter(|c| c.env_read.is_finite())
                .map(|c| c.sim_top - c.env_read)
                .collect(),
        );
        if let Some(pp) = &post_pre_stock {
            report(
                "post_pre - post_read",
                window
                    .keys()
                    .filter_map(|k| {
                        let a = pp.z_grid.top_z_at(k.0, k.1)?;
                        let b = post_read.top_z_at(k.0, k.1)?;
                        Some(f64::from(a) - f64::from(b))
                    })
                    .collect(),
            );
        }

        // Per-cell dump for offline analysis.
        let path = p2f_output_dir().join(format!("p2g_3way_{label}_cells.txt"));
        let mut f = std::io::BufWriter::new(std::fs::File::create(&path).expect("create dump"));
        let mut keys: Vec<&(usize, usize)> = window.keys().collect();
        keys.sort();
        for k in keys {
            let c = &window[k];
            let (x, y) = grid.cell_to_world(k.0, k.1);
            writeln!(
                f,
                "{} {} {x:.4} {y:.4} {:.6} {:.6} {:.6} {:.6}",
                k.0, k.1, c.env_pre, c.env_read, c.post, c.sim_top
            )
            .expect("write cell");
        }
        eprintln!("[{label}] window dump -> {}", path.display());

        // Band-wide per-column dump (offline diff maps / attribution).
        let bpath = p2f_output_dir().join(format!("p2g_3way_{label}_band.txt"));
        let mut bf = std::io::BufWriter::new(std::fs::File::create(&bpath).expect("create dump"));
        let mut bkeys: Vec<&(usize, usize)> = band.keys().collect();
        bkeys.sort();
        for k in bkeys {
            let c = &band[k];
            writeln!(
                bf,
                "{} {} {} {:.6} {:.6} {:.6}",
                k.0, k.1, c.band, c.dev, c.sim_top, c.post
            )
            .expect("write band column");
        }
        eprintln!("[{label}] band dump -> {}", bpath.display());

        BranchOut { window, band }
    };

    let b = run("b75", None);
    let d = run(
        "d",
        Some(OperationConfig::Scallop(ScallopConfig {
            scallop_height: 0.011,
            tolerance: 0.05,
            direction: ScallopDirection::OutsideIn,
            continuous: true,
            slope_from: 0.0,
            slope_to: 90.0,
            feed_rate: 3000.0,
            plunge_rate: 150.0,
            stock_to_leave: 0.0,
            spindle_rpm: Some(21000),
        })),
    );

    // Cross-branch paired deltas: if the gap is REAL geometry it shows in
    // BOTH sim_top−sim_top and post−post; if it is sim-side only, post−post
    // stays ~0 while sim_top−sim_top carries the shift.
    eprintln!("== P2.g THREE-WAY CROSS-BRANCH (B75 − D, per common column) ==");
    for (bname, code) in [("mid-steep", 2u8), ("all-bands", 255u8)] {
        let mut d_sim = Vec::new();
        let mut d_post = Vec::new();
        for (k, cb) in &b.band {
            let Some(cd) = d.band.get(k) else { continue };
            if code != 255 && cb.band != code {
                continue;
            }
            d_sim.push(cb.sim_top - cd.sim_top);
            d_post.push(cb.post - cd.post);
        }
        eprintln!("[{bname}]");
        report("sim_top(B) - sim_top(D)", d_sim);
        report("post(B)    - post(D)", d_post);
    }
    let mut d_env = Vec::new();
    for (k, cb) in &b.window {
        let Some(cd) = d.window.get(k) else { continue };
        if cb.env_read.is_finite() && cd.env_read.is_finite() {
            d_env.push(cb.env_read - cd.env_read);
        }
    }
    eprintln!("[window]");
    report("env_read(B) - env_read(D)", d_env);
}

/// Tail-2 repro (unified v3 prompt, 2026-07-09): the night GUI sim showed
/// 20 rapid collisions, ALL in the live v2 unified finish op — local moves
/// 8580/9193/9252/9310/9992/10356 (early cluster) and 79 909–120 576 (late
/// cluster, worst z=19.996 at 119 234 — a rapid essentially AT the raw
/// stock top). Diagnostic hypothesis: inter-region rapids not lifting to
/// safe-Z — the exact linking machinery v3's fused router leans on. The
/// earlier headless colfix run had 0 on the OLD project file; this repro
/// runs the CURRENT file unmodified through the same chain + GUI-options
/// sim as `run_chain` and dumps every collision with per-op attribution,
/// local move index, neighborhood move context, and the prior-stock top at
/// the crossing — enough to identify the emitting code path headlessly.
#[test]
#[ignore = "one full generation ladder + GUI-options sim; run with --ignored --nocapture"]
fn p2g_live_v2_collision_repro() {
    let mut s = ProjectSession::load(&wanaka_project_path()).expect("load wanaka.toml");
    let out = run_chain("live-v2 collision repro", &mut s);
    eprintln!("total rapid collisions: {}", out.collisions);

    // Resolution sweep (2026-07-13 live finding): the GUI's auto
    // resolution for the Ø1 tip is 0.1 mm and its sim flags 20 rapid
    // collisions on the SAME toolpaths this 0.5 mm default sim passes —
    // the collision check samples rapids against the dexel tops, and
    // fine grids keep thin ridge crests the coarse grid smooths away.
    // Print counts per resolution so the divergence is pinned headlessly.
    let cancel = AtomicBool::new(false);
    for res in [0.25, 0.1] {
        let opts = SimulationOptions {
            resolution: res,
            ..Default::default()
        };
        s.run_simulation(&opts, &cancel).expect("resolution sim");
        let sim = s.simulation_result().expect("sim result");
        eprintln!(
            "rapid collisions at {res}mm: {}",
            sim.rapid_collisions.len()
        );
        for (rc, &gidx) in sim
            .rapid_collisions
            .iter()
            .zip(&sim.rapid_collision_move_indices)
            .take(6)
        {
            let b = sim
                .boundaries
                .iter()
                .find(|b| (b.start_move..b.end_move).contains(&gidx));
            eprintln!(
                "  op='{}' local={} ({:.3},{:.3},{:.3})->({:.3},{:.3},{:.3})",
                b.map_or("?", |b| b.name.as_str()),
                b.map_or(gidx, |b| gidx - b.start_move),
                rc.start.x,
                rc.start.y,
                rc.start.z,
                rc.end.x,
                rc.end.y,
                rc.end.z
            );
        }
    }

    let sim = s.simulation_result().expect("sim result");
    let n = s.toolpath_count();
    assert_eq!(
        sim.rapid_collisions.len(),
        sim.rapid_collision_move_indices.len(),
        "collision/index vectors must be parallel"
    );
    for (rc, &gidx) in sim
        .rapid_collisions
        .iter()
        .zip(&sim.rapid_collision_move_indices)
    {
        let Some(b) = sim
            .boundaries
            .iter()
            .find(|b| (b.start_move..b.end_move).contains(&gidx))
        else {
            eprintln!("collision at global {gidx} outside all boundaries?!");
            continue;
        };
        let local = gidx - b.start_move;
        // Prior-stock top at the crossing (the stock state the check ran
        // against — snapshot before this op carved).
        let stock_top = sim.prior_stocks.get(&b.id).and_then(|st| {
            let g = &st.z_grid;
            let mid_x = 0.5 * (rc.start.x + rc.end.x);
            let mid_y = 0.5 * (rc.start.y + rc.end.y);
            g.world_to_cell(mid_x, mid_y)
                .and_then(|(r, c)| g.top_z_at(r, c))
        });
        eprintln!(
            "op='{}' local={local} rapid ({:.3},{:.3},{:.3}) -> ({:.3},{:.3},{:.3}) | prior stock top at mid: {stock_top:?}",
            b.name, rc.start.x, rc.start.y, rc.start.z, rc.end.x, rc.end.y, rc.end.z
        );
        // Neighborhood move context from the op's stored toolpath.
        let cfg_idx = (0..n).find(|&i| s.get_toolpath_config(i).is_some_and(|tc| tc.id == b.id));
        if let Some(res) = cfg_idx.and_then(|i| s.get_result(i)) {
            let moves = &res.annotated().toolpath.moves;
            let lo = local.saturating_sub(3);
            let hi = (local + 4).min(moves.len());
            for (i, m) in moves.iter().enumerate().take(hi).skip(lo) {
                let marker = if i == local { ">>" } else { "  " };
                eprintln!(
                    "  {marker} #{i} {:?} {:?} -> ({:.3},{:.3},{:.3})",
                    m.move_type, m.intent, m.target.x, m.target.y, m.target.z
                );
            }
        }
    }
}

/// P2.g LUT-error probe: quantifies `RadialProfileLUT` interpolation error
/// for the wanaka tapered tool (Ø1 tip on Ø6 shank). The LUT samples
/// uniformly in dist² over the SHANK radius (256 bins over r²=9), so the
/// tip sphere occupies ~7 bins and linear interpolation of the convex ball
/// cap OVERSHOOTS (tool reads higher -> dexel stamp cuts shallower) by an
/// amount that peaks near the tip-sphere edge — exactly the ball-side
/// contact distance for mid-steep slopes (d = R·sin(slope)). Suspected
/// mechanism of the B75-vs-D fine-tier gap (terrain-parallel rings always
/// cut at that contact distance; D's straight silhouette rings don't).
#[test]
#[ignore = "pure math; run with --ignored --nocapture"]
fn p2g_lut_error_probe() {
    use rs_cam_core::radial_profile::RadialProfileLUT;
    use rs_cam_core::tool::{MillingCutter, TaperedBallEndmill};

    let cutter = TaperedBallEndmill::new(1.0, 7.0, 6.0, 25.0);
    let lut_old = RadialProfileLUT::from_cutter(&cutter, 256);
    let lut = RadialProfileLUT::from_cutter(&cutter, rs_cam_core::radial_profile::LUT_SAMPLES);
    let tip_r: f64 = 0.5;
    eprintln!("slope_deg contact_d_mm  exact_h      err256_um  err_now_um");
    for slope_deg in [30.0f64, 45.0, 50.0, 55.0, 60.0, 65.0, 70.0, 75.0, 80.0] {
        let d = tip_r * slope_deg.to_radians().sin();
        let exact = cutter.height_at_radius(d).expect("in profile");
        let h_old = lut_old.height_at_dist_sq(d * d).expect("in profile");
        let h_now = lut.height_at_dist_sq(d * d).expect("in profile");
        eprintln!(
            "{slope_deg:>9} {d:>12.4} {exact:>12.6} {:>10.2} {:>11.3}",
            (h_old - exact) * 1000.0,
            (h_now - exact) * 1000.0
        );
    }
    // full-profile scan for max error inside the tip sphere
    let mut max_err = 0.0f64;
    let mut max_d = 0.0f64;
    let mut d = 0.0;
    while d < tip_r {
        if let (Some(e), Some(l)) = (cutter.height_at_radius(d), lut.height_at_dist_sq(d * d)) {
            let err = l - e;
            if err > max_err {
                max_err = err;
                max_d = d;
            }
        }
        d += 0.001;
    }
    eprintln!(
        "max overshoot inside tip sphere: {:.1}um at d={max_d:.3}mm (slope {:.1} deg)",
        max_err * 1000.0,
        (max_d / tip_r).asin().to_degrees()
    );
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
        surface.heightmap.covered_flags(),
        &[],
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
        let finish_idx = enabled_finish_index(&s);
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
    let finish_idx = enabled_finish_index(&c);

    // C2 inherits A's toolpath config field-by-field (`ToolpathConfig`
    // deliberately isn't Clone — fresh IDs come from `add_toolpath`) and
    // A's raster op VERBATIM (same stepover/feeds/min_z), narrowed to the
    // shallow window. Snapshot everything before C1's swap destroys it.
    let template = c
        .get_toolpath_config(finish_idx)
        .expect("finish toolpath config");
    let OperationConfig::DropCutter(a_raster) = template.operation.clone() else {
        panic!(
            "expected the enabled finish op '{}' to be a DropCutter raster (branch A shape)",
            template.name
        );
    };
    // C1 reads these after `a_raster` is consumed by C2's struct update.
    let (a_feed, a_plunge, a_rpm) = (
        a_raster.feed_rate,
        a_raster.plunge_rate,
        a_raster.spindle_rpm,
    );
    let c2 = rs_cam_core::session::ToolpathConfig {
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

/// P2.f matrix: B at waterline 65 — the 65–75° band goes to Z-contour
/// waterline instead of scallop. The P2.e lock to 75 was scored on time
/// and collisions only (pre-instrument); this row supplies the per-band
/// quality data that decision never had, so the steep-wall
/// contour-vs-scallop question is decided on numbers.
#[test]
#[ignore = "one full-project dexel simulation ladder; run with --ignored --nocapture"]
fn p2f_fidelity_branch_b65() {
    let path = wanaka_project_path();
    let mut b = ProjectSession::load(&path).expect("load wanaka.toml (B65)");
    let finish_idx = enabled_finish_index(&b);
    let cfg = UnifiedFinishConfig {
        waterline_threshold_deg: 65.0,
        ..ab_unified_config()
    };
    b.set_toolpath_operation(finish_idx, OperationConfig::UnifiedFinish(cfg))
        .expect("swap Finish 6 operation to UnifiedFinish (65)");
    let out = run_chain("B65: unified, waterline band 65-75", &mut b);
    eprintln!(
        "vs pinned A: finish {:+.1}% project {:+.1}% collisions={}",
        100.0 * (out.finish_total_s - PINNED_A_FINISH_S) / PINNED_A_FINISH_S,
        100.0 * (out.project_total_s - PINNED_A_PROJECT_S) / PINNED_A_PROJECT_S,
        out.collisions
    );
    run_measurement_sim(&mut b);
    let bm = build_band_map(&b);
    fidelity_report("p2f_B65", &b, &bm);
    assert!(out.finish_total_s > 0.0);
}

/// P2.f matrix, branch D: ONE standalone all-over scallop replacing
/// Finish 6 — no planner, no regions, no raster, no waterline — at the
/// same cusp dial as B's scallop band. The user's structural question:
/// does the regioned/unified approach actually beat "just run a normal
/// full scallop" at equal quality, or does the decomposition overhead
/// eat the win? Scored with the fidelity instrument like every branch.
#[test]
#[ignore = "one full-project dexel simulation ladder; run with --ignored --nocapture"]
fn p2f_allover_scallop_branch_d() {
    let path = wanaka_project_path();
    let mut d = ProjectSession::load(&path).expect("load wanaka.toml (D)");
    let finish_idx = enabled_finish_index(&d);
    d.set_toolpath_operation(
        finish_idx,
        OperationConfig::Scallop(ScallopConfig {
            scallop_height: 0.011,
            tolerance: 0.05,
            direction: ScallopDirection::OutsideIn,
            continuous: true,
            slope_from: 0.0,
            slope_to: 90.0,
            feed_rate: 3000.0,
            plunge_rate: 150.0,
            stock_to_leave: 0.0,
            spindle_rpm: Some(21000),
        }),
    )
    .expect("swap Finish 6 operation to all-over Scallop");
    let out = run_chain("D: all-over scallop", &mut d);
    eprintln!(
        "vs pinned A: finish {:+.1}% project {:+.1}% collisions={}",
        100.0 * (out.finish_total_s - PINNED_A_FINISH_S) / PINNED_A_FINISH_S,
        100.0 * (out.project_total_s - PINNED_A_PROJECT_S) / PINNED_A_PROJECT_S,
        out.collisions
    );
    run_measurement_sim(&mut d);
    let bm = build_band_map(&d);
    fidelity_report("p2f_D", &d, &bm);
    assert!(out.finish_total_s > 0.0);
}

/// P2.f cascade feasibility probe (user question 2026-07-09: "even a
/// 3/4 mm ball will leave a lot in this terrain"): per band, what
/// fraction of the wanaka surface can a ball of radius R NOT finish
/// (bridged concavities where the ball floor floats above the true
/// surface)? This is the rest-island share a big-tool → small-tool
/// cascade would hand to the small tool — the number that decides
/// whether branch E is worth building. Pure heightmap math: ball floor
/// = drop-cutter tip Z per cell; true surface = the classification
/// probe; leftover = floor − truth. Speed context for reading it: at
/// equal cusp the stepover scales with √R, so Ø6 covers open ground
/// only 1.73× faster than the 1 mm tip, Ø3 only 1.22× — the cascade
/// needs a LOW rest share to win.
#[test]
#[ignore = "three wanaka heightmaps; run with --ignored --nocapture"]
fn p2f_ball_rest_share_probe() {
    use rs_cam_core::finish_setup::{
        build_classification_surface_with_cancel, build_finish_surface_with_cell_size_and_cancel,
    };
    use rs_cam_core::mesh::SpatialIndex;
    use rs_cam_core::tool::BallEndmill;

    let s = ProjectSession::load(&wanaka_project_path()).expect("load wanaka.toml");
    let mesh = s
        .models()
        .iter()
        .find_map(|m| m.mesh.clone())
        .expect("terrain mesh");
    let index = SpatialIndex::build(&mesh, 10.0);
    let never = || false;
    let cell = 0.25;

    // True surface + band ownership at the same grid resolution.
    let probe_cutter = BallEndmill::new(6.0, 25.0); // grid frame only
    let truth =
        build_classification_surface_with_cancel(&mesh, &index, &probe_cutter, cell, &never)
            .expect("classification surface");
    let bm = build_band_map(&s);

    eprintln!("== P2.f ball rest-share probe (leftover > threshold vs true surface) ==");
    eprintln!(
        "{:>8} | {:>22} | {:>22} | {:>22}",
        "ball Ø", "shallow rest %", "mid-steep rest %", "overall rest %"
    );
    for diameter in [6.0, 4.0, 3.0, 2.0] {
        let ball = BallEndmill::new(diameter, 25.0);
        let floor =
            build_finish_surface_with_cell_size_and_cancel(&mesh, &index, &ball, cell, &never)
                .expect("ball floor surface");
        // Same grid dims by construction? Origins differ by tool radius —
        // compare via WORLD coordinates per truth-grid cell.
        let mut counts = [[0usize; 2]; 4]; // [band][covered, rest@0.05]
        let thm = &truth.heightmap;
        let fhm = &floor.heightmap;
        for r in 0..thm.rows {
            for c in 0..thm.cols {
                if !thm.covered_at(r, c) {
                    continue;
                }
                let x = thm.origin_x + c as f64 * thm.cell_size;
                let y = thm.origin_y + r as f64 * thm.cell_size;
                let fc = ((x - fhm.origin_x) / fhm.cell_size).round();
                let fr = ((y - fhm.origin_y) / fhm.cell_size).round();
                if fc < 0.0 || fr < 0.0 || fc >= fhm.cols as f64 || fr >= fhm.rows as f64 {
                    continue;
                }
                #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
                let (fr, fc) = (fr as usize, fc as usize);
                if !fhm.covered_at(fr, fc) {
                    continue;
                }
                let leftover = fhm.z_or_bbox_floor_at(fr, fc) - thm.z_or_bbox_floor_at(r, c);
                let band = bm.code_at(x, y) as usize;
                counts[band][0] += 1;
                if leftover > 0.05 {
                    counts[band][1] += 1;
                }
            }
        }
        let pct = |band: usize| -> f64 {
            100.0 * counts[band][1] as f64 / (counts[band][0] as f64).max(1.0)
        };
        let all_cov: usize = counts.iter().map(|c| c[0]).sum();
        let all_rest: usize = counts.iter().map(|c| c[1]).sum();
        eprintln!(
            "{diameter:>7}mm | {:>21.1}% | {:>21.1}% | {:>21.1}%",
            pct(1),
            pct(2),
            100.0 * all_rest as f64 / (all_cov as f64).max(1.0)
        );
    }
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
    let finish_idx = enabled_finish_index(&a);
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
    let finish_idx = enabled_finish_index(&b);
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

// ── S1 claims-pipeline A/B (Wave 3 acceptance harness, `planning/unified_v3_design.md` §4) ──
//
// S1 wired in-op `detect_rest_valleys` (stock reference), crease-corridor
// claims, rest-mask ∩ bands, and pencil emission via `centerline_cut_paths`
// behind `UnifiedFinishConfig::pencil_claims` (Wave 2, commit b7b1ea2) —
// byte-identical no-op when `false`. This harness is the checkpoint the
// design doc calls for at the end of S1: "quality (COLUMNS) must not
// regress vs live v2; pencil corridors visible in report." Both branches
// run the pinned B75 dials (`ab_unified_config`) so the ONLY variable is
// `pencil_claims`.

/// Region spans on `spans` that are NOT strictly nested inside another
/// Region span. `unified_finish`'s node-level spans (one per
/// `UnifiedFinishReport::region_table` entry — band regions plus the
/// trailing crease node) are inserted right after the `Operation` span,
/// coarser than the finer scallop-event Region spans nested inside a
/// MidSteep band's own move range (`compute/execute.rs` doc: "consumers
/// must disambiguate by span nesting depth, not assume one shared table").
/// Move-range containment is the only structural signal available across
/// the session boundary, so that's what this uses.
/// S1 claims-mask diagnostic: the `s1_claims_ab` quality gate failed with
/// ~30 % of the mid-steep band excluded by the rest-island territory mask
/// on ROUGHED prior stock, where the physical rest is ≥ stock-to-leave
/// everywhere the OFF branch demonstrably cut — the mask values must be
/// wrong (NaN trust erosion / lookup mismatch), not the physics. This
/// probe runs the ON chain once, reads the claims detector's own
/// `rest_grid` off the generated result (§2.4 carry-through), and dumps
/// per cell: x, y, rest, surface_z (detector's pencil drop), and the
/// prior-stock top at the same XY — enough to separate NaN-share from
/// value-mismatch (`rest` should equal `prior_top − surface_z` exactly,
/// same formula) from frame misregistration (spatial pattern).
#[test]
#[ignore = "one full generation ladder + sim; run with --ignored --nocapture"]
fn s1_claims_mask_probe() {
    use std::io::Write as _;

    let mut s = ProjectSession::load(&wanaka_project_path()).expect("load wanaka.toml");
    let idx = enabled_finish_index(&s);
    let mut cfg = ab_unified_config();
    cfg.pencil_claims = true;
    s.set_toolpath_operation(idx, OperationConfig::UnifiedFinish(cfg))
        .expect("swap to claims-on unified");
    run_chain("s1 mask probe", &mut s);

    let op_id = s.get_toolpath_config(idx).expect("cfg").id;
    let ann = s.get_result(idx).expect("finish generated").annotated();
    let grid = ann
        .rest_grid
        .as_ref()
        .expect("claims ran -> rest_grid carried on the result");
    let sim = s.simulation_result().expect("sim result");
    let prior = sim.prior_stocks.get(&op_id).expect("finish prior stock");
    let pz = &prior.z_grid;

    // Under the geometric-claims semantics the grid's `rest` is the
    // ANALYTIC float-rest (op cutter above the bare surface — crease
    // evidence), while the TERRITORY quantity is computed independently as
    // `prior stock top − surface_z` (drop field vs machined stock); NaN
    // surface_z KEEPS coverage. Tally both so the dump stays the mask's
    // ground truth.
    let mut n_nan = 0usize; // untrusted drop samples (keep coverage)
    let mut n_crease_rest = 0usize; // analytic float-rest >= 0.02 (crease evidence)
    let mut n_excluded = 0usize; // finite territory rest < 0.02 (masked out)
    let mut n_kept = 0usize; // finite territory rest >= 0.02
    let path = p2f_output_dir().join("s1_mask_cells.txt");
    let mut f = std::io::BufWriter::new(std::fs::File::create(&path).expect("create dump"));
    for row in 0..grid.ny {
        for col in 0..grid.nx {
            let x = grid.origin_x + col as f64 * grid.cell_mm;
            let y = grid.origin_y + row as f64 * grid.cell_mm;
            let rest = grid.rest[row * grid.nx + col];
            let sz = grid.surface_z[row * grid.nx + col];
            let ptop = pz
                .world_to_cell(x, y)
                .and_then(|(r, c)| pz.top_z_at(r, c))
                .map(f64::from);
            if !rest.is_nan() && f64::from(rest) >= 0.02 {
                n_crease_rest += 1;
            }
            match (sz.is_nan(), ptop) {
                (true, _) | (false, None) => n_nan += 1,
                (false, Some(pt)) => {
                    if pt - f64::from(sz) < 0.02 {
                        n_excluded += 1;
                    } else {
                        n_kept += 1;
                    }
                }
            }
            writeln!(
                f,
                "{row} {col} {x:.3} {y:.3} {rest:.4} {sz:.4} {}",
                ptop.map_or(f64::NAN, |v| v)
            )
            .expect("write cell");
        }
    }
    eprintln!(
        "rest grid {}x{} cell={:.3} origin=({:.2},{:.2}) | prior grid {}x{} cell={:.4} origin=({:.2},{:.2})",
        grid.nx,
        grid.ny,
        grid.cell_mm,
        grid.origin_x,
        grid.origin_y,
        pz.rows,
        pz.cols,
        pz.cell_size,
        pz.origin_u,
        pz.origin_v
    );
    let total = (grid.nx * grid.ny) as f64;
    eprintln!(
        "cells: untrusted(keep)={n_nan} ({:.1}%)  territory-excluded={n_excluded} ({:.1}%)  territory-kept={n_kept} ({:.1}%)  crease-rest>=0.02={n_crease_rest} ({:.1}%)",
        100.0 * n_nan as f64 / total,
        100.0 * n_excluded as f64 / total,
        100.0 * n_kept as f64 / total,
        100.0 * n_crease_rest as f64 / total,
    );
    eprintln!("dump -> {}", path.display());

    // Per-column deviations at measurement resolution, same format as the
    // three-way probe's band dump — diffed offline against the claims-off
    // baseline (`p2g_3way_b75_band.txt`, dial-identical) to locate WHERE
    // the ON branch lost coverage (rim-shaped = territory mask;
    // dendritic-stripe-shaped = corridor carving).
    run_measurement_sim(&mut s);
    let bm = build_band_map(&s);
    let group = s
        .setup_of_toolpath_id(op_id)
        .expect("finish op belongs to a setup");
    let sim = s.simulation_result().expect("measurement sim");
    let cols = sim
        .column_deviations
        .as_ref()
        .expect("column deviations present");
    let bpath = p2f_output_dir().join("s1_mask_on_band.txt");
    let mut bf = std::io::BufWriter::new(std::fs::File::create(&bpath).expect("create dump"));
    for cd in cols.iter().filter(|cd| cd.group == group) {
        writeln!(
            bf,
            "{} {} {} {:.6} {:.6} {:.6}",
            cd.row,
            cd.col,
            bm.code_at(cd.x, cd.y),
            cd.dev,
            cd.top_z,
            cd.top_z
        )
        .expect("write column");
    }
    eprintln!("band dump -> {}", bpath.display());
}

fn outer_region_spans(
    spans: &[rs_cam_core::toolpath_spans::Span],
) -> Vec<&rs_cam_core::toolpath_spans::Span> {
    use rs_cam_core::toolpath_spans::SpanKind;
    let regions: Vec<&rs_cam_core::toolpath_spans::Span> = spans
        .iter()
        .filter(|s| s.kind == SpanKind::Region)
        .collect();
    regions
        .iter()
        .copied()
        .filter(|s| {
            !regions.iter().any(|other| {
                !std::ptr::eq(*other, *s)
                    && other.start_move <= s.start_move
                    && other.end_move >= s.end_move
                    && (other.start_move, other.end_move) != (s.start_move, s.end_move)
            })
        })
        .collect()
}

/// Group-filtered mid-steep on-size / `+.05` shares from
/// `sim.column_deviations` — same `DEV_EDGES`/`DEV_BIN_LABELS` binning
/// `fidelity_report` uses (index 6 = "on-size", index 7 = "+.05"),
/// restricted to `group` (the finish op's own setup group — multi-setup
/// wanaka samples the same world XY once per group, see `ColumnDeviation`
/// doc) and to mid-steep band cells (`bm` code 2, from `build_band_map`).
/// Returns `(sample_count, on_size_pct, plus05_pct)`.
fn band_shares(
    s: &ProjectSession,
    bm: &BandMap,
    group: usize,
    band_code: u8,
) -> (usize, f64, f64, usize) {
    let sim = s.simulation_result().expect("sim result");
    let cols = sim
        .column_deviations
        .as_ref()
        .expect("column deviations (sim ran without a reference model mesh?)");
    let bin_of = |d: f32| -> usize {
        DEV_EDGES
            .iter()
            .position(|&e| d < e)
            .unwrap_or(DEV_BIN_COUNT - 1)
    };
    const ON_SIZE_BIN: usize = 6;
    const PLUS_05_BIN: usize = 7;
    const TAIL_BIN: usize = DEV_BIN_COUNT - 1; // ">+.5" — big standing leftover
    let mut total = 0usize;
    let mut on_size = 0usize;
    let mut plus05 = 0usize;
    let mut tail = 0usize;
    for cd in cols.iter().filter(|cd| cd.group == group) {
        if bm.code_at(cd.x, cd.y) != band_code {
            continue;
        }
        total += 1;
        match bin_of(cd.dev) {
            ON_SIZE_BIN => on_size += 1,
            PLUS_05_BIN => plus05 += 1,
            TAIL_BIN => tail += 1,
            _ => {}
        }
    }
    let pct = |n: usize| 100.0 * n as f64 / (total.max(1) as f64);
    (total, pct(on_size), pct(plus05), tail)
}

fn mid_steep_shares(s: &ProjectSession, bm: &BandMap, group: usize) -> (usize, f64, f64) {
    let (total, on, plus05, _) = band_shares(s, bm, group, 2);
    (total, on, plus05)
}

/// S1 acceptance A/B: `pencil_claims: false` (OFF, byte-identical to the
/// shipped B75 op) vs `pencil_claims: true` (ON, same dials, claims
/// pipeline live) on the real wanaka chain. Gates (design doc §4, in
/// order): collisions must not regress, group-filtered mid-steep on-size
/// share must not regress beyond a 2 pp tolerance, the '>+.5' leftover
/// tail must not grow (per band), and total project time must not blow
/// the budget by more than 10%.
///
/// KNOWN RED on wanaka as of 2026-07-13 (why `pencil_claims` defaults
/// off): with additive claims the bands are byte-identical (on-size /
/// tail equal to the column — quality gates pass), but the crease node
/// costs +22.5% finish time for zero measured quality gain because the
/// SELF-PROBE float field marks exactly the valleys the tool cannot
/// reach. The time gate is doing its job. S2 replaces the claim signal
/// (dihedral/curvature geometry, or cascade rest vs Op A's ball); this
/// test is the acceptance harness for that work.
///
/// The claims pipeline's own telemetry (`UnifiedFinishReport::claims`,
/// `region_table`) does not survive the session boundary today — the
/// report has no slot on `ToolpathComputeResult`, so territory mode
/// (`RestIslands` vs `Full`) is NOT directly observable here. The
/// detector's `rest_grid`/`rest_regions` ARE carried on the annotated
/// result since the §2.4 carry-through (see `s1_claims_mask_probe`, which
/// reads them). What this test asserts structurally is the Region spans
/// design doc §2.4 calls a MUST: the ON branch's finish op must carry at
/// least one outer (node-level) Region span.
#[test]
#[ignore = "two full generation ladders + 0.25mm measurement sims; run with --ignored --nocapture"]
fn s1_claims_ab() {
    let path = wanaka_project_path();
    assert!(
        path.exists(),
        "wanaka.toml not found at {} — harness requires the canonical project",
        path.display()
    );

    // ── Branch OFF: pinned B75 dials, claims pipeline explicitly off ────
    let mut off = ProjectSession::load(&path).expect("load wanaka.toml (OFF)");
    let finish_idx_off = enabled_finish_index(&off);
    let off_op_id = off
        .get_toolpath_config(finish_idx_off)
        .expect("finish op config")
        .id;
    off.set_toolpath_operation(
        finish_idx_off,
        OperationConfig::UnifiedFinish(ab_unified_config()),
    )
    .expect("swap finish op to UnifiedFinish (claims off)");
    let out_off = run_chain("s1_off", &mut off);
    run_measurement_sim(&mut off);
    let bm_off = build_band_map(&off);
    fidelity_report("s1_off", &off, &bm_off);
    let group_off = off
        .setup_of_toolpath_id(off_op_id)
        .expect("finish op belongs to a setup");
    let (n_off, on_size_off, plus05_off) = mid_steep_shares(&off, &bm_off, group_off);

    // ── Branch ON: identical dials, pencil_claims flipped on ────────────
    let mut on_cfg = ab_unified_config();
    on_cfg.pencil_claims = true;
    let mut on = ProjectSession::load(&path).expect("load wanaka.toml (ON)");
    let finish_idx_on = enabled_finish_index(&on);
    let on_op_id = on
        .get_toolpath_config(finish_idx_on)
        .expect("finish op config")
        .id;
    on.set_toolpath_operation(finish_idx_on, OperationConfig::UnifiedFinish(on_cfg))
        .expect("swap finish op to UnifiedFinish (claims on)");
    let out_on = run_chain("s1_on", &mut on);
    run_measurement_sim(&mut on);
    let bm_on = build_band_map(&on);
    fidelity_report("s1_on", &on, &bm_on);
    let group_on = on
        .setup_of_toolpath_id(on_op_id)
        .expect("finish op belongs to a setup");
    let (n_on, on_size_on, plus05_on) = mid_steep_shares(&on, &bm_on, group_on);

    // ── Structural claims-pipeline check (ON branch only — OFF is the
    // pre-v3 op and must NOT emit node spans by construction) ───────────
    let on_result = on
        .get_result(finish_idx_on)
        .expect("ON finish op generated");
    let on_annotated = on_result.annotated();
    let region_span_count = on_annotated
        .spans
        .iter()
        .filter(|s| s.kind == rs_cam_core::toolpath_spans::SpanKind::Region)
        .count();
    let outer = outer_region_spans(&on_annotated.spans);
    let crease_node_present = outer.iter().any(|s| s.label.as_ref() == "Pencil claims");
    eprintln!(
        "s1_on region spans: total={region_span_count} outer(node)={} crease_node_present={crease_node_present}",
        outer.len()
    );
    assert!(
        !outer.is_empty(),
        "S1 claims-on op emitted zero outer Region spans — region-table span emission regressed \
         (design doc §2.4: Region spans are a MUST)"
    );

    // ── Verdict ───────────────────────────────────────────────────────
    let d_project = out_on.project_total_s - out_off.project_total_s;
    let d_finish = out_on.finish_total_s - out_off.finish_total_s;
    eprintln!("== S1 claims A/B verdict (OFF vs ON) ==");
    eprintln!(
        "project_total_s : OFF={:8.1}s  ON={:8.1}s  Δ={d_project:+8.1}s ({:+.1}%)",
        out_off.project_total_s,
        out_on.project_total_s,
        100.0 * d_project / out_off.project_total_s.max(1e-9)
    );
    eprintln!(
        "finish_total_s  : OFF={:8.1}s  ON={:8.1}s  Δ={d_finish:+8.1}s ({:+.1}%)",
        out_off.finish_total_s,
        out_on.finish_total_s,
        100.0 * d_finish / out_off.finish_total_s.max(1e-9)
    );
    eprintln!(
        "collisions      : OFF={}  ON={}",
        out_off.collisions, out_on.collisions
    );
    eprintln!(
        "mid-steep on-size share (group-filtered): OFF={on_size_off:5.1}% (n={n_off})  ON={on_size_on:5.1}% (n={n_on})"
    );
    eprintln!("mid-steep +.05  share (group-filtered): OFF={plus05_off:5.1}%  ON={plus05_on:5.1}%");

    // Fat-tail counts (">+.5" bin — big standing leftover). The first live
    // validation (2026-07-13) caught the territory mask leaving ~340 mm² of
    // rough standing while the on-size share moved only 1.7 pp: skipped
    // territory shows up as a TAIL, not an on-size shift, so the tail is
    // its own gate — per band, since shallow grew even more than mid-steep.
    let (_, _, _, tail_mid_off) = band_shares(&off, &bm_off, group_off, 2);
    let (_, _, _, tail_mid_on) = band_shares(&on, &bm_on, group_on, 2);
    let (_, _, _, tail_sh_off) = band_shares(&off, &bm_off, group_off, 1);
    let (_, _, _, tail_sh_on) = band_shares(&on, &bm_on, group_on, 1);
    eprintln!(
        "'>+.5' tail columns: mid-steep OFF={tail_mid_off} ON={tail_mid_on} | shallow OFF={tail_sh_off} ON={tail_sh_on}"
    );

    // ── Gates (design doc §4 order) ──────────────────────────────────
    assert!(
        out_on.collisions <= out_off.collisions,
        "S1 SAFETY GATE FAILED: claims-on collisions {} > claims-off {}",
        out_on.collisions,
        out_off.collisions
    );
    assert!(
        on_size_on >= on_size_off - 2.0,
        "S1 QUALITY GATE FAILED: mid-steep on-size share regressed beyond tolerance: \
         OFF={on_size_off:.1}%  ON={on_size_on:.1}%  (2.0pp tolerance)"
    );
    // 5% + 100 columns of slack: measurement-sim texture jitters the tail
    // by tens of columns run-to-run; the defect class this guards against
    // showed up as +2,461 / +2,949.
    let tail_gate = |off_n: usize, on_n: usize, band: &str| {
        assert!(
            on_n <= off_n + off_n / 20 + 100,
            "S1 TAIL GATE FAILED: claims-on left {on_n} '{band}' columns >0.5mm standing \
             vs claims-off {off_n} — territory mask is skipping cuttable material"
        );
    };
    tail_gate(tail_mid_off, tail_mid_on, "mid-steep");
    tail_gate(tail_sh_off, tail_sh_on, "shallow");
    assert!(
        out_on.project_total_s <= out_off.project_total_s * 1.10,
        "S1 TIME GATE FAILED: claims-on project time {:.1}s exceeds 110% of claims-off {:.1}s",
        out_on.project_total_s,
        out_off.project_total_s
    );
}
