//! D0 — kinematics histogram on real adaptive3d fixtures.
//!
//! Drives the C1 stopgap-vs-roll-forward decision (D3) by measuring how
//! prevalent `CutKinematics::Helix` samples actually are in adaptive3d
//! cuts, and how their chip-thickness distribution compares to Linear
//! steady-state samples. See `planning/STRUCTURAL_ENTRY_SPANS_AND_LOCALITY.md`.
//!
//! Run: `cargo test --release --test kinematics_histogram -- --nocapture`
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr,
    clippy::print_stdout
)]

use rs_cam_core::session::{ProjectSession, SimulationOptions};
use rs_cam_core::simulation_cut::{CutKinematics, SimulationCutSample};
use rs_cam_core::toolpath_spans::SpanKind;
use std::path::Path;
use std::sync::atomic::AtomicBool;

const WANAKA_FIXTURE: &str = "/home/ricky/Downloads/wanaka100/wanaka_full_tuned.toml";

#[test]
fn kinematics_histogram_wanaka() {
    let toml_path = Path::new(WANAKA_FIXTURE);
    if !toml_path.exists() {
        eprintln!("skip: {} not found", WANAKA_FIXTURE);
        return;
    }

    let mut session = ProjectSession::load(toml_path).expect("load wanaka");
    let cancel = AtomicBool::new(false);

    // Apply post-C1 winner params to TP 1 (Back Rough).
    // Snapshot from prior MCP smoke: feed=4000, stepover=2.2, DOC=3.0.
    session
        .set_toolpath_param(1, "feed_rate", serde_json::json!(4000.0))
        .expect("set tp1 feed_rate");
    session
        .set_toolpath_param(1, "stepover", serde_json::json!(2.2))
        .expect("set tp1 stepover");
    session
        .set_toolpath_param(1, "depth_per_pass", serde_json::json!(3.0))
        .expect("set tp1 depth_per_pass");

    // Generate everything in setup order so simulation has a complete
    // stock state for each toolpath.
    let tp_count = session.toolpath_count();
    for idx in 0..tp_count {
        match session.generate_toolpath(idx, &cancel) {
            Ok(_) => {}
            Err(e) => {
                eprintln!("warn: failed to generate toolpath {}: {}", idx, e);
            }
        }
    }

    let opts = SimulationOptions {
        resolution: 0.5,
        skip_ids: vec![],
        metrics_enabled: true,
        auto_resolution: false,
    };
    let samples: Vec<SimulationCutSample> = {
        let sim_result = session.run_simulation(&opts, &cancel).expect("sim");
        let cut_trace = sim_result.cut_trace.as_ref().expect("cut trace");
        println!(
            "Total samples across all toolpaths: {}",
            cut_trace.samples.len()
        );
        println!();
        cut_trace.samples.clone()
    };

    // Adaptive3d targets: index 1 (Back Rough) and index 6 (3D Rough 6).
    for tp_index in [1usize, 6] {
        let Some(tc) = session.get_toolpath_config(tp_index) else {
            eprintln!("skip: tp_index {} not found", tp_index);
            continue;
        };
        let tp_id = tc.id;
        let tp_name = tc.name.clone();
        let op_label = tc.operation.label().to_owned();
        let Some(tp_result) = session.get_result(tp_index) else {
            eprintln!("skip: tp_index {} has no compute result", tp_index);
            continue;
        };
        let toolpath = tp_result.toolpath();

        println!("==================================================================");
        println!(
            "Toolpath {} (id={}): {}  [{}]",
            tp_index, tp_id, tp_name, op_label
        );
        println!("  total moves: {}", toolpath.moves.len());
        // D4 — count Entry spans, sanity-check coverage.
        let annotated = &tp_result.annotated;
        let entry_spans: Vec<_> = annotated
            .spans
            .iter()
            .filter(|s| s.kind == SpanKind::Entry)
            .collect();
        let entry_move_count: usize = entry_spans.iter().map(|s| s.end_move - s.start_move).sum();
        println!(
            "  Entry spans: {}  ({} moves total, {} labels)",
            entry_spans.len(),
            entry_move_count,
            entry_spans
                .iter()
                .map(|s| s.label.as_ref())
                .collect::<std::collections::BTreeSet<_>>()
                .len()
        );
        if let Some(first) = entry_spans.first() {
            println!(
                "  First entry: range {}..{}, label '{}'",
                first.start_move, first.end_move, first.label
            );
        }
        println!("==================================================================");

        report_for_toolpath(tp_id, &samples, &toolpath.moves);
        println!();
    }
}

fn report_for_toolpath(
    tp_id: usize,
    samples: &[SimulationCutSample],
    moves: &[rs_cam_core::toolpath::Move],
) {
    // Filter to this toolpath's in-cut samples.
    let in_cut: Vec<&SimulationCutSample> = samples
        .iter()
        .filter(|s| s.toolpath_id == tp_id && s.is_cutting)
        .collect();

    if in_cut.is_empty() {
        println!("  (no in-cut samples)");
        return;
    }
    println!("  In-cut samples: {}", in_cut.len());

    // Per-kinematics histogram.
    let mut counts = [0usize; 5]; // Linear, Plunge, Helix, Arc, Rapid
    for s in &in_cut {
        counts[kin_idx(s.cut_kinematics)] += 1;
    }
    let total = in_cut.len() as f64;
    println!("  Kinematics histogram:");
    for (i, name) in ["Linear", "Plunge", "Helix", "Arc", "Rapid"].iter().enumerate() {
        let c = counts[i];
        if c > 0 {
            println!(
                "    {:>7}: {:>7}  ({:>5.2}%)",
                name,
                c,
                100.0 * c as f64 / total
            );
        }
    }

    // Per-kinematics chip-thickness percentiles.
    println!("  Chip-thickness (mm) percentiles by kinematics:");
    for (i, name) in ["Linear", "Plunge", "Helix", "Arc", "Rapid"].iter().enumerate() {
        if counts[i] == 0 {
            continue;
        }
        let kin = idx_kin(i);
        let mut chips: Vec<f64> = in_cut
            .iter()
            .filter(|s| s.cut_kinematics == kin)
            .filter_map(|s| s.effective_chip_thickness_mm)
            .collect();
        if chips.is_empty() {
            println!(
                "    {:>7}: (no effective_chip_thickness_mm samples)",
                name
            );
            continue;
        }
        chips.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let p50 = pct(&chips, 0.50);
        let p90 = pct(&chips, 0.90);
        let p95 = pct(&chips, 0.95);
        let p99 = pct(&chips, 0.99);
        let max = *chips.last().unwrap();
        let n = chips.len();
        println!(
            "    {:>7} (n={:>5}): p50={:.4}  p90={:.4}  p95={:.4}  p99={:.4}  max={:.4}",
            name, n, p50, p90, p95, p99, max
        );
    }

    // Within Helix samples: stratify by |dz/path_length| over the
    // sample's parent move (target_i - target_{i-1}).
    let helix: Vec<&SimulationCutSample> = in_cut
        .iter()
        .copied()
        .filter(|s| s.cut_kinematics == CutKinematics::Helix)
        .collect();
    if helix.is_empty() {
        return;
    }
    println!(
        "  Helix samples stratified by |dz/path_length| of parent move:"
    );
    // Buckets: [0,0.1), [0.1,0.3), [0.3,0.6), [0.6,0.9), [0.9,1.0]
    let bucket_edges: &[(f64, f64, &str)] = &[
        (0.00, 0.10, "0.0–0.1 (near-flat)"),
        (0.10, 0.30, "0.1–0.3"),
        (0.30, 0.60, "0.3–0.6"),
        (0.60, 0.90, "0.6–0.9"),
        (0.90, 1.0001, "0.9–1.0 (near-vertical)"),
    ];
    let mut bucket_chips: Vec<Vec<f64>> = vec![Vec::new(); bucket_edges.len()];
    let mut bucket_count = vec![0usize; bucket_edges.len()];

    for s in &helix {
        let mi = s.move_index;
        if mi == 0 || mi >= moves.len() {
            continue;
        }
        let prev = moves[mi - 1].target;
        let curr = moves[mi].target;
        let dx = curr.x - prev.x;
        let dy = curr.y - prev.y;
        let dz = curr.z - prev.z;
        let len = (dx * dx + dy * dy + dz * dz).sqrt();
        if len < 1e-9 {
            continue;
        }
        let ratio = dz.abs() / len;
        let mut bi = 0usize;
        for (i, (lo, hi, _)) in bucket_edges.iter().enumerate() {
            if ratio >= *lo && ratio < *hi {
                bi = i;
                break;
            }
        }
        bucket_count[bi] += 1;
        if let Some(c) = s.effective_chip_thickness_mm {
            bucket_chips[bi].push(c);
        }
    }
    let helix_total: usize = bucket_count.iter().sum();
    for (i, (_, _, label)) in bucket_edges.iter().enumerate() {
        let count = bucket_count[i];
        if count == 0 {
            println!("    {:<22}: 0", label);
            continue;
        }
        let mut chips = bucket_chips[i].clone();
        chips.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let pct_share = 100.0 * count as f64 / helix_total as f64;
        if chips.is_empty() {
            println!(
                "    {:<22}: n={:>6}  ({:>5.2}% of Helix)  (no chip data)",
                label, count, pct_share
            );
            continue;
        }
        let p50 = pct(&chips, 0.50);
        let p90 = pct(&chips, 0.90);
        let p99 = pct(&chips, 0.99);
        let max = *chips.last().unwrap();
        println!(
            "    {:<22}: n={:>6}  ({:>5.2}% of Helix)  chip p50={:.4} p90={:.4} p99={:.4} max={:.4}",
            label, count, pct_share, p50, p90, p99, max
        );
    }
}

fn kin_idx(k: CutKinematics) -> usize {
    match k {
        CutKinematics::Linear => 0,
        CutKinematics::Plunge => 1,
        CutKinematics::Helix => 2,
        CutKinematics::Arc => 3,
        CutKinematics::Rapid => 4,
    }
}

fn idx_kin(i: usize) -> CutKinematics {
    match i {
        0 => CutKinematics::Linear,
        1 => CutKinematics::Plunge,
        2 => CutKinematics::Helix,
        3 => CutKinematics::Arc,
        _ => CutKinematics::Rapid,
    }
}

/// Linear-interpolated percentile on a sorted slice.
fn pct(sorted: &[f64], q: f64) -> f64 {
    if sorted.is_empty() {
        return f64::NAN;
    }
    if sorted.len() == 1 {
        return sorted[0];
    }
    let pos = q * (sorted.len() - 1) as f64;
    let lo = pos.floor() as usize;
    let hi = (lo + 1).min(sorted.len() - 1);
    let frac = pos - lo as f64;
    sorted[lo] * (1.0 - frac) + sorted[hi] * frac
}
