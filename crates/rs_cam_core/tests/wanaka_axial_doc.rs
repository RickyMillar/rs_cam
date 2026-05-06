//! Reproduce the live wanaka peak_axial_doc=19.71mm via ProjectSession.
//! Synthetic test passes (3mm). This loads wanaka.toml the same way MCP
//! does — finds where the live-only path differs.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr,
    clippy::print_stdout
)]

use rs_cam_core::session::ProjectSession;
use std::path::Path;
use std::sync::atomic::AtomicBool;

#[test]
fn wanaka_back_rough_axial_doc() {
    let toml_path = Path::new("/home/ricky/Downloads/wanaka100/wanaka_full_tuned.toml");
    if !toml_path.exists() {
        eprintln!("skip: wanaka.toml not found");
        return;
    }

    let mut session = ProjectSession::load(toml_path).expect("load wanaka");
    println!(
        "loaded {} setups, {} toolpaths",
        session.list_setups().len(),
        session.list_toolpaths().len()
    );

    let cancel = AtomicBool::new(false);

    // Generate only Back Rough (TP1) plus its predecessor in the same setup.
    // Pin Drill (TP0) runs first in Setup 1 — include it for identical state.
    session
        .generate_toolpath(0, &cancel)
        .expect("gen pin drill");
    session
        .generate_toolpath(1, &cancel)
        .expect("gen back rough");

    let opts = rs_cam_core::session::SimulationOptions {
        resolution: 0.5,
        skip_ids: vec![],
        metrics_enabled: true,
        auto_resolution: false,
    };

    let tp_id = session.list_toolpaths()[1].id;
    let result = session.run_simulation(&opts, &cancel).expect("sim");
    let cut_trace = result.cut_trace.as_ref().expect("cut trace");
    println!("samples: {}", cut_trace.samples.len());
    println!("Back Rough toolpath_id = {}", tp_id);

    let mut peak = None;
    for s in &cut_trace.samples {
        if s.toolpath_id != tp_id {
            continue;
        }
        match peak {
            None => peak = Some(s.clone()),
            Some(ref p) if s.axial_doc_mm > p.axial_doc_mm => peak = Some(s.clone()),
            _ => {}
        }
    }
    let peak = peak.expect("at least one Back Rough sample");
    println!(
        "peak axial DOC: {:.3}mm at sample {} (move {})",
        peak.axial_doc_mm, peak.sample_index, peak.move_index
    );
    println!(
        "  position: ({:.3}, {:.3}, {:.3})  arc {:.3}  chip_eff {:.4}",
        peak.position[0],
        peak.position[1],
        peak.position[2],
        peak.arc_engagement_radians.unwrap_or(0.0),
        peak.effective_chip_thickness_mm.unwrap_or(0.0)
    );

    // Get the actual toolpath move at peak.move_index.
    let tp_result = session.get_result(1).expect("back rough result");
    let mv = &tp_result.toolpath.moves[peak.move_index];
    let prev = &tp_result.toolpath.moves[peak.move_index.saturating_sub(1)];
    println!(
        "  move {}: type={:?}, from ({:.3},{:.3},{:.3}) -> ({:.3},{:.3},{:.3})",
        peak.move_index,
        mv.move_type,
        prev.target.x,
        prev.target.y,
        prev.target.z,
        mv.target.x,
        mv.target.y,
        mv.target.z
    );
    println!("  surrounding moves:");
    let lo = peak.move_index.saturating_sub(3);
    let hi = (peak.move_index + 3).min(tp_result.toolpath.moves.len() - 1);
    for j in lo..=hi {
        let m = &tp_result.toolpath.moves[j];
        println!(
            "    move {}: type={:?} target=({:.3},{:.3},{:.3})",
            j, m.move_type, m.target.x, m.target.y, m.target.z
        );
    }

    // Check earlier moves to identify what stage this is. move 98 is early.
    println!("  first 10 moves of Back Rough:");
    for j in 0..10.min(tp_result.toolpath.moves.len()) {
        let m = &tp_result.toolpath.moves[j];
        println!(
            "    move {}: type={:?} target=({:.3},{:.3},{:.3})",
            j, m.move_type, m.target.x, m.target.y, m.target.z
        );
    }

    // Total moves to understand position in toolpath
    println!(
        "  total moves: {}, peak at move {} ({:.1}% through)",
        tp_result.toolpath.moves.len(),
        peak.move_index,
        100.0 * peak.move_index as f64 / tp_result.toolpath.moves.len() as f64
    );

    // Probe simulator dexel state RIGHT BEFORE the peak DOC sample's move.
    // This tells us whether the peak cell was stamped at any earlier point
    // in the toolpath.
    {
        use rs_cam_core::dexel_stock::{StockCutDirection, TriDexelStock};
        use rs_cam_core::geo::BoundingBox3 as Bbox3;
        use rs_cam_core::tool::FlatEndmill;
        let probe_cutter = FlatEndmill::new(6.0, 25.0);
        let probe_cancel = || false;
        // Hard-code wanaka stock bounds for setup-local frame (face_up=Bottom).
        let local_bbox = Bbox3 {
            min: rs_cam_core::geo::P3::new(0.0, 0.0, 0.0),
            max: rs_cam_core::geo::P3::new(140.0, 150.0, 25.0),
        };
        let mut probe = TriDexelStock::from_bounds(&local_bbox, 0.5);
        // Truncate Back Rough up to but not including the peak move.
        let mut up_to = (*tp_result.toolpath).clone();
        if peak.move_index < up_to.moves.len() {
            up_to.moves.truncate(peak.move_index);
        }
        let _ = probe.simulate_toolpath_with_metrics_with_cancel(
            &up_to,
            &probe_cutter,
            StockCutDirection::FromTop,
            tp_id,
            18000,
            2,
            5000.0,
            0.5,
            None,
            true,
            &probe_cancel,
        );
        // Inspect the cell at peak XY.
        let cs = probe.z_grid.cell_size;
        let mid_x = peak.position[0];
        let mid_y = peak.position[1];
        let radius = 3.0_f64;
        let mut max_top_in_fp = f32::NEG_INFINITY;
        let mut peak_cell = (0usize, 0usize);
        for row in 0..probe.z_grid.rows {
            let y = probe.z_grid.origin_v + row as f64 * cs;
            if (y - mid_y).abs() > radius {
                continue;
            }
            for col in 0..probe.z_grid.cols {
                let x = probe.z_grid.origin_u + col as f64 * cs;
                let d = ((x - mid_x).powi(2) + (y - mid_y).powi(2)).sqrt();
                if d > radius {
                    continue;
                }
                let ray = &probe.z_grid.rays[row * probe.z_grid.cols + col];
                let top = ray.iter().map(|s| s.exit).fold(f32::NEG_INFINITY, f32::max);
                if top > max_top_in_fp {
                    max_top_in_fp = top;
                    peak_cell = (row, col);
                }
            }
        }
        let (row, col) = peak_cell;
        let x = probe.z_grid.origin_u + col as f64 * cs;
        let y = probe.z_grid.origin_v + row as f64 * cs;
        println!(
            "  PROBE before move {}: peak ray_top in footprint at row={} col={} (world {:.2},{:.2}): top={:.2}",
            peak.move_index, row, col, x, y, max_top_in_fp
        );
    }

    // Search for feed moves matching the perimeter sweep corners at z=22.
    let perim_targets = [
        (22.75, 27.75, 22.0),
        (117.25, 27.75, 22.0),
        (117.25, 121.75, 22.0),
        (22.75, 121.75, 22.0),
        (22.75, 28.0, 22.0),
    ];
    println!("  searching for perimeter sweep z=22 corner feeds:");
    for &(tx, ty, tz) in &perim_targets {
        let mut found_count = 0;
        let mut found_idx = 0usize;
        for (j, m) in tp_result.toolpath.moves.iter().enumerate() {
            if (m.target.x - tx).abs() < 0.01
                && (m.target.y - ty).abs() < 0.01
                && (m.target.z - tz).abs() < 0.01
            {
                if found_count == 0 {
                    found_idx = j;
                }
                found_count += 1;
            }
        }
        println!(
            "    target ({:.2}, {:.2}, {:.2}): {} match(es), first @ move {}",
            tx, ty, tz, found_count, found_idx
        );
    }

    // Sanity: any feed move at z=22 within 5mm of (47.76, 119.0)?
    let target = (peak.position[0], peak.position[1]);
    let mut z22_near_count = 0;
    for m in &tp_result.toolpath.moves {
        if matches!(
            m.move_type,
            rs_cam_core::toolpath::MoveType::Linear { .. }
                | rs_cam_core::toolpath::MoveType::ArcCW { .. }
                | rs_cam_core::toolpath::MoveType::ArcCCW { .. }
        ) && (m.target.z - 22.0).abs() < 0.5
        {
            let d = ((m.target.x - target.0).powi(2) + (m.target.y - target.1).powi(2)).sqrt();
            if d < 5.0 {
                z22_near_count += 1;
            }
        }
    }
    println!(
        "  feed/arc moves at z≈22 within 5mm of {:?}: {}",
        target, z22_near_count
    );

    // Count Linear (feed) moves at various Z levels
    let mut z_level_feeds = std::collections::BTreeMap::<i32, usize>::new();
    for m in &tp_result.toolpath.moves {
        if matches!(m.move_type, rs_cam_core::toolpath::MoveType::Linear { .. }) {
            let z_round = m.target.z.round() as i32;
            *z_level_feeds.entry(z_round).or_insert(0) += 1;
        }
    }
    println!("  Linear (feed) moves by Z level:");
    for (z, count) in z_level_feeds.iter() {
        println!("    z≈{}: {} feeds", z, count);
    }
    // Find first/last z=22 feeds and show context
    let mut z22_feeds = Vec::new();
    for (j, m) in tp_result.toolpath.moves.iter().enumerate() {
        if matches!(m.move_type, rs_cam_core::toolpath::MoveType::Linear { .. })
            && (m.target.z - 22.0).abs() < 0.5
        {
            z22_feeds.push(j);
        }
    }
    if let Some(&first) = z22_feeds.first() {
        println!("  first z=22 feed at move {}", first);
        let lo = first.saturating_sub(2);
        let hi = (first + 5).min(tp_result.toolpath.moves.len() - 1);
        for j in lo..=hi {
            let m = &tp_result.toolpath.moves[j];
            println!(
                "    move {}: type={:?} ({:.3},{:.3},{:.3})",
                j, m.move_type, m.target.x, m.target.y, m.target.z
            );
        }
    }

    // First 80 moves (verbatim) to see what z=22 cuts look like
    println!("  first 80 moves verbatim:");
    for j in 0..80.min(tp_result.toolpath.moves.len()) {
        let m = &tp_result.toolpath.moves[j];
        println!(
            "    move {}: type={:?} ({:.3},{:.3},{:.3})",
            j, m.move_type, m.target.x, m.target.y, m.target.z
        );
    }

    // Dump transitions: list move ranges by approximate z bucket.
    println!("  z transitions across all moves:");
    let mut prev_z_bucket: Option<i32> = None;
    let mut bucket_start = 0usize;
    for (j, m) in tp_result.toolpath.moves.iter().enumerate() {
        let bucket = (m.target.z / 3.0).round() as i32;
        if Some(bucket) != prev_z_bucket {
            if let Some(b) = prev_z_bucket {
                println!(
                    "    moves {}..{}: ~z={:.1}",
                    bucket_start,
                    j - 1,
                    b as f64 * 3.0
                );
            }
            bucket_start = j;
            prev_z_bucket = Some(bucket);
        }
    }
    if let Some(b) = prev_z_bucket {
        let last = tp_result.toolpath.moves.len() - 1;
        println!(
            "    moves {}..{}: ~z={:.1}",
            bucket_start,
            last,
            b as f64 * 3.0
        );
    }
}
