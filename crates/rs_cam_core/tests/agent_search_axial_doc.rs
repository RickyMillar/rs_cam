//! Diagnostic: load a terrain mesh, run AgentSearch roughing, simulate,
//! and dump the deepest axial-DOC samples plus their context. Used to
//! diagnose where wanaka's 18.7mm peak axial DOC originates.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr,
    clippy::print_stdout
)]

use rs_cam_core::adaptive3d::{
    Adaptive3dParams, ClearingStrategy3d, EntryStyle3d, RegionOrdering, adaptive_3d_toolpath,
};
use rs_cam_core::compute::transform::{FaceUp, ZRotation};
use rs_cam_core::dexel_stock::{StockCutDirection, TriDexelStock};
use rs_cam_core::geo::{BoundingBox3, P3};
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::tool::FlatEndmill;
use rs_cam_core::toolpath::MoveType;
use std::path::Path;

#[test]
fn agent_search_axial_doc_diag() {
    // Prefer the actual wanaka mesh if present; fallback to fixture.
    let wanaka = Path::new("/home/ricky/Downloads/wanaka100/rivmap_export/terrain.stl");
    let stl_path = if wanaka.exists() {
        wanaka.to_path_buf()
    } else {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/terrain.stl")
    };
    if !stl_path.exists() {
        eprintln!("skip: no fixture at {:?}", stl_path);
        return;
    }
    println!("using mesh: {:?}", stl_path);

    let mesh_world = TriangleMesh::from_stl(&stl_path).expect("stl load");
    println!("mesh world bbox: x [{:.2},{:.2}] y [{:.2},{:.2}] z [{:.2},{:.2}]",
        mesh_world.bbox.min.x, mesh_world.bbox.max.x,
        mesh_world.bbox.min.y, mesh_world.bbox.max.y,
        mesh_world.bbox.min.z, mesh_world.bbox.max.z);

    // Apply wanaka_full_tuned.toml's setup: face_up=bottom + stock origin (-20,-25,-20).
    // Stock dims: x=140, y=150, z=25.
    let stock_x = 140.0;
    let stock_y = 150.0;
    let stock_z = 25.0;
    let origin_x = -20.0;
    let origin_y = -25.0;
    let origin_z = -20.0;
    let face_up = FaceUp::Bottom;
    let z_rotation = ZRotation::Deg0;

    // Transform every mesh vertex world → setup-local: 1) translate by -stock_origin
    // 2) face_up flip 3) z_rotation
    let new_verts: Vec<P3> = mesh_world
        .vertices
        .iter()
        .map(|v| {
            let rel = P3::new(v.x - origin_x, v.y - origin_y, v.z - origin_z);
            let flipped = face_up.transform_point(rel, stock_x, stock_y, stock_z);
            let (eff_w, eff_d, _) = face_up.effective_stock(stock_x, stock_y, stock_z);
            z_rotation.transform_point(flipped, eff_w, eff_d)
        })
        .collect();
    let mesh = TriangleMesh::from_raw(new_verts, mesh_world.triangles.clone());
    let index = SpatialIndex::build(&mesh, 6.0);

    let bbox = &mesh.bbox;
    println!("mesh setup-local bbox: x [{:.2},{:.2}] y [{:.2},{:.2}] z [{:.2},{:.2}]",
        bbox.min.x, bbox.max.x, bbox.min.y, bbox.max.y, bbox.min.z, bbox.max.z);

    let cutter = FlatEndmill::new(6.0, 25.0);
    // Setup-local stock_top = effective_stock_bbox.max.z = stock_z = 25.
    let stock_top = stock_z;
    let params = Adaptive3dParams {
        tool_radius: 3.0,
        envelope_radius: 3.0,
        stepover: 0.84,
        depth_per_pass: 3.0,
        stock_to_leave: 0.5,
        feed_rate: 3150.0,
        plunge_rate: 750.0,
        safe_z: stock_top + 5.0,
        tolerance: 0.1,
        min_cutting_radius: 0.0,
        stock_top_z: stock_top,
        entry_style: EntryStyle3d::Plunge,
        fine_stepdown: None,
        detect_flat_areas: false,
        max_stay_down_dist: None,
        region_ordering: RegionOrdering::Global,
        initial_stock: None,
        clearing_strategy: ClearingStrategy3d::AgentSearch,
        z_blend: true,
        boundary: None,
    };

    let tp = adaptive_3d_toolpath(&mesh, &index, &cutter, &params);
    println!("toolpath: {} moves, cutting {:.0}mm, rapid {:.0}mm",
        tp.moves.len(), tp.total_cutting_distance(), tp.total_rapid_distance());

    // Setup-local stock bbox: (0,0,0) to (eff_w, eff_d, eff_h) — same as
    // what the GUI passes to the simulator via effective_stock_bbox().
    let (eff_w, eff_d, eff_h) = face_up.effective_stock(stock_x, stock_y, stock_z);
    let stock_bbox = BoundingBox3 {
        min: P3::new(0.0, 0.0, 0.0),
        max: P3::new(eff_w, eff_d, eff_h),
    };
    println!("stock setup-local bbox: x [0,{:.2}] y [0,{:.2}] z [0,{:.2}]",
        eff_w, eff_d, eff_h);
    let mut stock = TriDexelStock::from_bounds(&stock_bbox, 0.5);

    let never_cancel = || false;
    let samples = stock
        .simulate_toolpath_with_metrics_with_cancel(
            &tp,
            &cutter,
            StockCutDirection::FromTop,
            1,
            18000,
            2,
            5000.0,
            0.5,
            None,
            true,
            &never_cancel,
        )
        .expect("sim");

    println!("samples: {}", samples.len());

    let mut peak_axial_doc = 0.0f64;
    let mut peak_sample_idx = 0usize;
    let mut deep_count = 0usize;
    let mut peak_chipload = 0.0f64;
    let mut peak_chipload_sample_idx = 0usize;
    for (i, s) in samples.iter().enumerate() {
        if s.axial_doc_mm > peak_axial_doc {
            peak_axial_doc = s.axial_doc_mm;
            peak_sample_idx = i;
        }
        if s.axial_doc_mm > 5.0 {
            deep_count += 1;
        }
        if let Some(eff) = s.effective_chip_thickness_mm
            && eff > peak_chipload
        {
            peak_chipload = eff;
            peak_chipload_sample_idx = i;
        }
    }

    println!("peak axial DOC: {:.2}mm at sample {}", peak_axial_doc, peak_sample_idx);
    if peak_sample_idx < samples.len() {
        let s = &samples[peak_sample_idx];
        println!("  pos: ({:.2}, {:.2}, {:.2})  move {}  arc {:.2}  chip_eff {:.4}",
            s.position[0], s.position[1], s.position[2],
            s.move_index, s.arc_engagement_radians.unwrap_or(0.0),
            s.effective_chip_thickness_mm.unwrap_or(0.0));
        // Inspect surrounding moves
        for j in (s.move_index.saturating_sub(2))..=(s.move_index + 2).min(tp.moves.len() - 1) {
            let m = &tp.moves[j];
            let kind = match m.move_type {
                MoveType::Rapid => "Rapid",
                MoveType::Linear { .. } => "Feed",
                _ => "Other",
            };
            println!("  move {}: {} -> ({:.2}, {:.2}, {:.2})",
                j, kind, m.target.x, m.target.y, m.target.z);
        }
    }
    println!("samples with axial_doc > 5mm: {}", deep_count);
    println!("peak effective chipload: {:.4} at sample {}", peak_chipload, peak_chipload_sample_idx);
    if peak_chipload_sample_idx < samples.len() {
        let s = &samples[peak_chipload_sample_idx];
        println!("  pos: ({:.2}, {:.2}, {:.2})  move {}  arc {:.2}  axial {:.2}",
            s.position[0], s.position[1], s.position[2],
            s.move_index, s.arc_engagement_radians.unwrap_or(0.0),
            s.axial_doc_mm);
        for j in (s.move_index.saturating_sub(3))..=(s.move_index + 3).min(tp.moves.len() - 1) {
            let m = &tp.moves[j];
            let kind = match m.move_type {
                MoveType::Rapid => "Rapid",
                MoveType::Linear { .. } => "Feed",
                _ => "Other",
            };
            println!("  move {}: {} -> ({:.2}, {:.2}, {:.2})",
                j, kind, m.target.x, m.target.y, m.target.z);
        }
    }
    // Count samples with arc >= π/2 (the chipload-saturation region)
    let half_slot = std::f64::consts::FRAC_PI_2;
    let half_slot_count = samples
        .iter()
        .filter(|s| s.arc_engagement_radians.unwrap_or(0.0) >= half_slot)
        .count();
    let near_slot_count = samples
        .iter()
        .filter(|s| {
            let a = s.arc_engagement_radians.unwrap_or(0.0);
            a >= half_slot && a < std::f64::consts::PI - 0.1
        })
        .count();
    let full_slot_count = samples
        .iter()
        .filter(|s| s.arc_engagement_radians.unwrap_or(0.0) >= std::f64::consts::PI - 0.1)
        .count();
    println!("samples with arc >= π/2: {} ({} half-only, {} full-slot >= π-0.1)",
        half_slot_count, near_slot_count, full_slot_count);

    // Diagnostic: scan all cells in the footprint of the deep-DOC sample
    // to find which one was never cleared. AFTER the full sim, but the
    // sample-time state has the stamp applied. So we re-simulate up to
    // (but not including) move 9389, then inspect.
    {
        let mut probe_stock = TriDexelStock::from_bounds(&stock_bbox, 0.5);
        let peak_move_index = if peak_sample_idx < samples.len() {
            samples[peak_sample_idx].move_index
        } else {
            9389
        };
        let mut up_to_9388 = tp.clone();
        up_to_9388.moves.truncate(peak_move_index);
        println!("  (probing state right before move {})", peak_move_index);
        let _ = probe_stock.simulate_toolpath_with_metrics_with_cancel(
            &up_to_9388,
            &cutter,
            StockCutDirection::FromTop,
            1, 18000, 2, 5000.0, 0.5, None, true,
            &never_cancel,
        );
        let cs = probe_stock.z_grid.cell_size;
        let mid_x = 23.0_f64;
        let mid_y = 26.25_f64;
        let radius = 3.0_f64;
        let mut max_top_in_fp = f32::NEG_INFINITY;
        let mut peak_cell = (0usize, 0usize);
        for row in 0..probe_stock.z_grid.rows {
            let y = probe_stock.z_grid.origin_v + row as f64 * cs;
            if (y - mid_y).abs() > radius { continue; }
            for col in 0..probe_stock.z_grid.cols {
                let x = probe_stock.z_grid.origin_u + col as f64 * cs;
                let d = ((x - mid_x).powi(2) + (y - mid_y).powi(2)).sqrt();
                if d > radius { continue; }
                let ray = &probe_stock.z_grid.rays[row * probe_stock.z_grid.cols + col];
                let top = ray.iter().map(|s| s.exit).fold(f32::NEG_INFINITY, f32::max);
                if top > max_top_in_fp {
                    max_top_in_fp = top;
                    peak_cell = (row, col);
                }
            }
        }
        let (row, col) = peak_cell;
        let x = probe_stock.z_grid.origin_u + col as f64 * cs;
        let y = probe_stock.z_grid.origin_v + row as f64 * cs;
        println!("  BEFORE move 9389: peak ray_top in footprint at row={}, col={} (world {:.2},{:.2}): top={:.2}",
            row, col, x, y, max_top_in_fp);
    }

    // Inspect the toolpath: any feed move whose footprint covers (24, 23.5)
    // — the actual uncleared cell — at z >= 8?
    let target_xy = (24.0, 23.5);
    let radius_check = 3.0_f64;
    let mut earlier_visits: Vec<(usize, f64, f64, f64)> = Vec::new();
    for (i, m) in tp.moves.iter().enumerate() {
        if matches!(m.move_type, MoveType::Linear { .. }) && m.target.z > 7.5 {
            let dx = m.target.x - target_xy.0;
            let dy = m.target.y - target_xy.1;
            let d = (dx * dx + dy * dy).sqrt();
            if d < radius_check + 0.1 {
                earlier_visits.push((i, m.target.x, m.target.y, m.target.z));
            }
        }
    }
    println!("feed moves with cutter footprint covering {:?} at z>=8: {}", target_xy, earlier_visits.len());
    for (i, x, y, z) in earlier_visits.iter() {
        // Print move + previous move to see segment direction
        let prev = if *i > 0 { &tp.moves[*i - 1] } else { &tp.moves[0] };
        println!("  move {}: from ({:.2},{:.2},{:.2}) -> ({:.2}, {:.2}, {:.2})",
            i, prev.target.x, prev.target.y, prev.target.z, x, y, z);
    }
    // Look at ALL moves' minimum y at z=22 + count of moves within
    // 3mm of y=25 (the southern polygon boundary).
    let mut min_y_at_z22 = f64::INFINITY;
    let mut moves_y_le_27_at_z22 = 0usize;
    for m in &tp.moves {
        if matches!(m.move_type, MoveType::Linear { .. }) && (m.target.z - 22.0).abs() < 0.1 {
            min_y_at_z22 = min_y_at_z22.min(m.target.y);
            if m.target.y <= 27.0 {
                moves_y_le_27_at_z22 += 1;
            }
        }
    }
    println!("min y in z=22 feed moves: {:.2}, moves with y<=27: {}",
        min_y_at_z22, moves_y_le_27_at_z22);
    // List a sampling of feed moves at z=22 with y<26.5 (south of machinable bound).
    // Show first 20 z=22 feed moves to verify perimeter sweep was emitted
    println!("first 20 z=22 feed moves:");
    let mut shown = 0;
    for (i, m) in tp.moves.iter().enumerate() {
        if matches!(m.move_type, MoveType::Linear { .. }) && (m.target.z - 22.0).abs() < 0.1 {
            let prev = if i > 0 { tp.moves[i - 1].target } else { m.target };
            println!("  move {}: ({:.2},{:.2},{:.2}) -> ({:.2},{:.2},{:.2})",
                i, prev.x, prev.y, prev.z, m.target.x, m.target.y, m.target.z);
            shown += 1;
            if shown >= 20 { break; }
        }
    }
    println!("z=22 feed moves with y<26.5:");
    for (i, m) in tp.moves.iter().enumerate() {
        if matches!(m.move_type, MoveType::Linear { .. })
            && (m.target.z - 22.0).abs() < 0.1
            && m.target.y < 26.5
        {
            let prev = if i > 0 { tp.moves[i - 1].target } else { m.target };
            println!("  move {}: ({:.2},{:.2},{:.2}) -> ({:.2},{:.2},{:.2})",
                i, prev.x, prev.y, prev.z, m.target.x, m.target.y, m.target.z);
            if i > 30 { break; }
        }
    }
    // Now: which moves at z=22 have a SAMPLE position passing within
    // tool_radius (3mm) of (24, 23.5)? Check each move's swept line.
    let target = (24.0, 23.5);
    let mut sweeping_moves = Vec::new();
    for i in 1..tp.moves.len() {
        let prev = &tp.moves[i - 1];
        let curr = &tp.moves[i];
        if !matches!(curr.move_type, MoveType::Linear { .. }) || (curr.target.z - 22.0).abs() > 0.1 {
            continue;
        }
        // Closest point on segment (prev->curr) to target
        let dx_seg = curr.target.x - prev.target.x;
        let dy_seg = curr.target.y - prev.target.y;
        let len_sq = dx_seg * dx_seg + dy_seg * dy_seg;
        let dx_t = target.0 - prev.target.x;
        let dy_t = target.1 - prev.target.y;
        let t = if len_sq > 1e-12 {
            ((dx_t * dx_seg + dy_t * dy_seg) / len_sq).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let cx = prev.target.x + t * dx_seg;
        let cy = prev.target.y + t * dy_seg;
        let d = ((cx - target.0).powi(2) + (cy - target.1).powi(2)).sqrt();
        if d < 3.0 {
            sweeping_moves.push((i, cx, cy, d));
        }
    }
    println!("z=22 segments whose swept path passes within radius 3.0 of {:?}: {}",
        target, sweeping_moves.len());
    for (i, cx, cy, d) in sweeping_moves.iter().take(8) {
        println!("  move {}: closest_point=({:.2}, {:.2}), dist={:.3}", i, cx, cy, d);
    }
    // The 2D adaptive insets the polygon by tool_radius. For polygon
    // southern edge at y=~23.25, the inset edge (machinable region) is at
    // y=~26.25. Cutter center stays north of that → cells south of y=23.25
    // (the polygon boundary) are NEVER reached by the cutter footprint
    // (which extends only tool_radius=3 south of cutter center, so reaches
    // y=23.25 in the ideal case).
    //
    // BUT cell (24, 23.5) IS within tool_radius of cutter center at y=25.
    // So if cutter ever reaches y=25, this cell gets stamped.
    // The test shows it DOESN'T get stamped — so cutter never reaches the
    // very southern edge of the inset region OR the inset is even further.

    // Regression guard: with the perimeter-sweep + slope-aware Cut split
    // fixes, no Cut sample should have axial DOC exceeding depth_per_pass
    // by more than 0.5mm tolerance. Pre-fix this read 18mm on wanaka.
    assert!(
        peak_axial_doc <= params.depth_per_pass + 0.5,
        "axial DOC regressed: peak {:.2}mm > dpp {:.1}mm + 0.5 tolerance",
        peak_axial_doc,
        params.depth_per_pass
    );
}
