//! Diagnostic: how many Back Rough cutting moves land outside the model
//! silhouette? The user reports "circular arc / cuts outside boundary on
//! every lap" — this test answers definitively.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr,
    clippy::print_stdout
)]

use std::path::Path;
use std::sync::atomic::AtomicBool;

use rs_cam_core::geo::P2;
use rs_cam_core::session::ProjectSession;
use rs_cam_core::toolpath::MoveType;

#[ignore = "wanaka boundary diagnostic — run with --ignored"]
#[test]
fn wanaka_back_rough_cuts_outside_silhouette() {
    let toml = "/home/ricky/Downloads/wanaka100/wanaka_full_tuned.toml";
    if !Path::new(toml).exists() {
        eprintln!("skip: {toml} not present");
        return;
    }

    let mut session = ProjectSession::load(Path::new(toml)).expect("load");
    let cancel = AtomicBool::new(false);
    session.generate_toolpath(0, &cancel).expect("pin drill");
    session.generate_toolpath(1, &cancel).expect("back rough");

    // Compute the model silhouette ourselves (same code path as
    // resolve_containment_polygon when source = ModelSilhouette).
    // Back Rough has model_id = 1; find that loaded model.
    let model = session
        .models()
        .iter()
        .find(|m| m.id == 1)
        .expect("model id 1 (terrain.stl)");
    let mesh = model.mesh.as_ref().expect("terrain.stl has mesh");
    let silhouettes = rs_cam_core::boundary::model_silhouette(mesh.as_ref(), None);
    let silhouette = silhouettes
        .into_iter()
        .max_by(|a, b| {
            a.area()
                .partial_cmp(&b.area())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .expect("at least one silhouette polygon");
    let xs: Vec<f64> = silhouette.exterior.iter().map(|p| p.x).collect();
    let ys: Vec<f64> = silhouette.exterior.iter().map(|p| p.y).collect();
    let xmin = xs.iter().cloned().fold(f64::INFINITY, f64::min);
    let xmax = xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let ymin = ys.iter().cloned().fold(f64::INFINITY, f64::min);
    let ymax = ys.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let bbox_xy = (xmin, ymin, xmax, ymax);
    println!(
        "silhouette: {} verts, area={:.1} mm², bbox=[{xmin:.2}..{xmax:.2}] x [{ymin:.2}..{ymax:.2}]",
        silhouette.exterior.len(),
        silhouette.area(),
    );

    // Walk Back Rough's moves
    let tp_result = session.get_result(1).expect("back rough result");
    let toolpath = &tp_result.toolpath;
    let mut total_cut_moves = 0_usize;
    let mut outside_count = 0_usize;
    let mut max_outside_dist_mm = 0.0_f64;
    let mut sample_outside: Vec<(usize, f64, f64, f64)> = Vec::new(); // (idx, x, y, z)
    for (i, m) in toolpath.moves.iter().enumerate() {
        let is_cut = matches!(
            m.move_type,
            MoveType::Linear { .. } | MoveType::ArcCW { .. } | MoveType::ArcCCW { .. }
        );
        if !is_cut {
            continue;
        }
        total_cut_moves += 1;
        let p = P2::new(m.target.x, m.target.y);
        if !silhouette.contains_point(&p) {
            outside_count += 1;
            // Distance from polygon edge: we don't have a fast accurate
            // method, but bbox-based fallback gives a usable signal.
            let dx_outside = if m.target.x < bbox_xy.0 {
                bbox_xy.0 - m.target.x
            } else if m.target.x > bbox_xy.2 {
                m.target.x - bbox_xy.2
            } else {
                0.0
            };
            let dy_outside = if m.target.y < bbox_xy.1 {
                bbox_xy.1 - m.target.y
            } else if m.target.y > bbox_xy.3 {
                m.target.y - bbox_xy.3
            } else {
                0.0
            };
            let outside_bbox = (dx_outside * dx_outside + dy_outside * dy_outside).sqrt();
            if outside_bbox > max_outside_dist_mm {
                max_outside_dist_mm = outside_bbox;
            }
            if sample_outside.len() < 20 {
                sample_outside.push((i, m.target.x, m.target.y, m.target.z));
            }
        }
    }
    println!(
        "cut moves: {total_cut_moves} total, {outside_count} outside silhouette \
         ({:.1}%), max outside-bbox distance = {max_outside_dist_mm:.3} mm",
        100.0 * outside_count as f64 / total_cut_moves.max(1) as f64,
    );

    // Bbox of all cut moves
    let mut cxmin = f64::INFINITY;
    let mut cxmax = f64::NEG_INFINITY;
    let mut cymin = f64::INFINITY;
    let mut cymax = f64::NEG_INFINITY;
    for m in &toolpath.moves {
        if matches!(
            m.move_type,
            MoveType::Linear { .. } | MoveType::ArcCW { .. } | MoveType::ArcCCW { .. }
        ) {
            cxmin = cxmin.min(m.target.x);
            cxmax = cxmax.max(m.target.x);
            cymin = cymin.min(m.target.y);
            cymax = cymax.max(m.target.y);
        }
    }
    println!("cut moves bbox: [{cxmin:.2}..{cxmax:.2}] x [{cymin:.2}..{cymax:.2}]");

    // What's the setup transform for Back Rough?
    let setups = session.list_setups();
    for (i, s) in setups.iter().enumerate() {
        println!(
            "setup {i}: name={:?} face_up={:?} tp_indices={:?}",
            s.name, s.face_up, s.toolpath_indices
        );
    }
    println!("stock bbox (project file): x=[-20..120], y=[-25..125], z=[-20..5] (140x150x25)");
    println!("first 20 outside moves (idx, x, y, z):");
    for (i, x, y, z) in &sample_outside {
        println!("  {i}: ({x:.2}, {y:.2}, {z:.2})");
    }
}
