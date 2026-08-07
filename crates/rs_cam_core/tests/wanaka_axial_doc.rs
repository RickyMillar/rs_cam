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

use rs_cam_core::dropcutter::point_drop_cutter;
use rs_cam_core::session::ProjectSession;
use std::path::Path;
use std::sync::atomic::AtomicBool;

/// Answers the user's question: "why does the rest of the surface that is
/// about the same height not get the same treatment?" — i.e. are there mesh
/// cells at the SAME height as the deep gouge that did NOT get cut deep
/// (engine bug), or is the gouge genuinely the deepest mesh point and every
/// equally-low cell is cut equally deep (mesh-driven, engine correct)?
///
/// Sim-free: generates the Back Rough toolpath, then for a coarse XY grid
/// reports the mesh keep-surface (drop-cutter, setup-local) vs the deepest
/// toolpath feed Z in that cell. Groups cells by mesh-height bucket and
/// shows the spread of cut depth per bucket. A tight spread per bucket =
/// equal heights cut equally (engine conforms to mesh). A wide spread =
/// equal heights cut unequally (the user is right; engine bug).
#[test]
#[ignore = "expensive WANAKA diagnostic; run with `cargo test --test wanaka_axial_doc -- --ignored`"]
fn wanaka_deep_region_localization() {
    use rs_cam_core::compute::transform::{FaceUp, ZRotation};
    use rs_cam_core::mesh::SpatialIndex;
    use rs_cam_core::tool::FlatEndmill;
    use rs_cam_core::toolpath::MoveType;

    let toml_path = Path::new("/home/ricky/Downloads/wanaka100/wanaka_full_tuned.toml");
    if !toml_path.exists() {
        eprintln!("skip: wanaka.toml not found");
        return;
    }
    let mut session = ProjectSession::load(toml_path).expect("load wanaka");
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(0, &cancel)
        .expect("gen pin drill");
    session
        .generate_toolpath(1, &cancel)
        .expect("gen back rough");

    let tp_id = session.list_toolpaths()[1].id;
    let tp_result = session.get_result(1).expect("back rough result");
    let moves = &tp_result.toolpath().moves;

    // Setup-local mesh for drop-cutter.
    let tp_setup_idx = session
        .setup_of_toolpath_id(tp_id)
        .expect("setup for back rough");
    let (face_up, z_rot) = session
        .list_setups()
        .get(tp_setup_idx)
        .map(|s| (s.face_up, s.z_rotation))
        .unwrap_or((FaceUp::Bottom, ZRotation::Deg0));
    let model_mesh = session
        .models()
        .iter()
        .find_map(|m| m.mesh.as_ref().map(|mm| mm.as_ref().clone()))
        .expect("wanaka has a mesh model");
    let local_mesh = session
        .setup_transform_info(face_up, z_rot)
        .apply_to_mesh(&model_mesh);
    let index = SpatialIndex::build(&local_mesh, 5.0);
    let cutter = FlatEndmill::new(6.0, 25.0);

    // Feed-move targets → XY bbox + per-cell deepest feed Z.
    let is_feed = |m: &rs_cam_core::toolpath::Move| {
        matches!(
            m.move_type,
            MoveType::Linear { .. } | MoveType::ArcCW { .. } | MoveType::ArcCCW { .. }
        )
    };
    let (mut xmin, mut xmax, mut ymin, mut ymax) = (
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
    );
    for m in moves.iter().filter(|m| is_feed(m)) {
        xmin = xmin.min(m.target.x);
        xmax = xmax.max(m.target.x);
        ymin = ymin.min(m.target.y);
        ymax = ymax.max(m.target.y);
    }
    let cell = 3.0_f64;
    let cols = ((xmax - xmin) / cell).ceil() as usize + 1;
    let rows = ((ymax - ymin) / cell).ceil() as usize + 1;
    let idx = |r: usize, c: usize| r * cols + c;
    let mut min_cut_z = vec![f64::INFINITY; rows * cols];
    for m in moves.iter().filter(|m| is_feed(m)) {
        let c = ((m.target.x - xmin) / cell) as usize;
        let r = ((m.target.y - ymin) / cell) as usize;
        if r < rows && c < cols && m.target.z < min_cut_z[idx(r, c)] {
            min_cut_z[idx(r, c)] = m.target.z;
        }
    }

    // Per-cell mesh height + collect (mesh_z, cut_z) for cut cells.
    let mut pairs: Vec<(f64, f64, f64, f64)> = Vec::new(); // (mesh_z, cut_z, x, y)
    for r in 0..rows {
        for c in 0..cols {
            let cz = min_cut_z[idx(r, c)];
            if !cz.is_finite() {
                continue; // cell never cut
            }
            let x = xmin + c as f64 * cell;
            let y = ymin + r as f64 * cell;
            let mz = point_drop_cutter(x, y, &local_mesh, &index, &cutter).z;
            if mz.is_finite() {
                pairs.push((mz, cz, x, y));
            }
        }
    }
    println!(
        "grid {}x{} cells @ {}mm over X[{:.1},{:.1}] Y[{:.1},{:.1}]; {} cut cells with valid mesh",
        rows,
        cols,
        cell,
        xmin,
        xmax,
        ymin,
        ymax,
        pairs.len()
    );

    // Group by mesh-height bucket (2mm) → spread of cut depth.
    use std::collections::BTreeMap;
    let mut buckets: BTreeMap<i32, Vec<f64>> = BTreeMap::new();
    for &(mz, cz, _, _) in &pairs {
        buckets
            .entry((mz / 2.0).round() as i32)
            .or_default()
            .push(cz);
    }
    println!("mesh-height bucket (2mm) → cut-Z spread [n: min/mean/max, delta(mean cut - mesh)]:");
    for (b, czs) in &buckets {
        let mesh_h = *b as f64 * 2.0;
        let n = czs.len();
        let cmin = czs.iter().cloned().fold(f64::INFINITY, f64::min);
        let cmax = czs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let cmean = czs.iter().sum::<f64>() / n as f64;
        println!(
            "  mesh≈{:>6.1}: n={:<5} cut min/mean/max = {:>6.2}/{:>6.2}/{:>6.2}  Δmean={:>6.2}  span={:>5.2}",
            mesh_h,
            n,
            cmin,
            cmean,
            cmax,
            cmean - mesh_h,
            cmax - cmin
        );
    }

    // Direct test of "same height, different treatment": take cells whose
    // MESH height is in a tight band (4-6mm, where the bulk of the surface
    // sits) and split them by how deep they were actually cut. If both a
    // deep-cut group AND a shallow-cut group exist at the same mesh height,
    // the engine is treating identical heights unequally.
    let band_cells: Vec<&(f64, f64, f64, f64)> =
        pairs.iter().filter(|p| p.0 >= 4.0 && p.0 <= 6.0).collect();
    let deep: Vec<&&(f64, f64, f64, f64)> = band_cells.iter().filter(|p| p.1 <= 7.0).collect();
    let shallow: Vec<&&(f64, f64, f64, f64)> = band_cells.iter().filter(|p| p.1 >= 10.0).collect();
    println!(
        "SAME-HEIGHT TEST — cells with mesh height in [4,6]mm: {} total; \
         {} cut DEEP (cut_z<=7, ate the leave) vs {} cut SHALLOW (cut_z>=10, kept the leave)",
        band_cells.len(),
        deep.len(),
        shallow.len()
    );
    // XY centroid + extent of the deep-cut group (is it one localized patch?).
    if !deep.is_empty() {
        let n = deep.len() as f64;
        let cx = deep.iter().map(|p| p.2).sum::<f64>() / n;
        let cy = deep.iter().map(|p| p.3).sum::<f64>() / n;
        let (mut dxmin, mut dxmax, mut dymin, mut dymax) = (
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
        );
        for p in &deep {
            dxmin = dxmin.min(p.2);
            dxmax = dxmax.max(p.2);
            dymin = dymin.min(p.3);
            dymax = dymax.max(p.3);
        }
        println!(
            "  deep-cut group centroid ({:.1},{:.1}), bbox X[{:.1},{:.1}] Y[{:.1},{:.1}] \
             ({:.0}x{:.0}mm) — localized patch if small vs the {:.0}x{:.0}mm working area",
            cx,
            cy,
            dxmin,
            dxmax,
            dymin,
            dymax,
            dxmax - dxmin,
            dymax - dymin,
            xmax - xmin,
            ymax - ymin
        );
    }
}

/// DEFINITIVE test of "same height, different treatment": simulate the full
/// Back Rough onto a dexel and read the ACTUAL final stock surface left at
/// each cell (real removed material — immune to transit moves and the
/// drop-cutter smoothing that confound the move-target proxy). Then, among
/// cells at the same mesh keep-surface height, ask whether the final stock
/// is left at the same height. If equal-height cells are left at very
/// different final heights, the engine is over-cutting some same-height
/// stock (the user's complaint); if not, the rough is uniform and the deep
/// bits are genuinely deeper mesh.
#[test]
#[ignore = "expensive WANAKA diagnostic; run with `cargo test --test wanaka_axial_doc -- --ignored`"]
fn wanaka_final_surface_vs_mesh() {
    use rs_cam_core::compute::transform::{FaceUp, ZRotation};
    use rs_cam_core::dexel_stock::{StockCutDirection, TriDexelStock};
    use rs_cam_core::geo::BoundingBox3 as Bbox3;
    use rs_cam_core::geo::P3;
    use rs_cam_core::mesh::SpatialIndex;
    use rs_cam_core::tool::FlatEndmill;

    let toml_path = Path::new("/home/ricky/Downloads/wanaka100/wanaka_full_tuned.toml");
    if !toml_path.exists() {
        eprintln!("skip: wanaka.toml not found");
        return;
    }
    let mut session = ProjectSession::load(toml_path).expect("load wanaka");
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(0, &cancel)
        .expect("gen pin drill");
    session
        .generate_toolpath(1, &cancel)
        .expect("gen back rough");
    let tp_id = session.list_toolpaths()[1].id;
    let tp_result = session.get_result(1).expect("back rough result");

    // Setup-local mesh for drop-cutter.
    let tp_setup_idx = session
        .setup_of_toolpath_id(tp_id)
        .expect("setup for back rough");
    let (face_up, z_rot) = session
        .list_setups()
        .get(tp_setup_idx)
        .map(|s| (s.face_up, s.z_rotation))
        .unwrap_or((FaceUp::Bottom, ZRotation::Deg0));
    let model_mesh = session
        .models()
        .iter()
        .find_map(|m| m.mesh.as_ref().map(|mm| mm.as_ref().clone()))
        .expect("wanaka has a mesh model");
    let local_mesh = session
        .setup_transform_info(face_up, z_rot)
        .apply_to_mesh(&model_mesh);
    let index = SpatialIndex::build(&local_mesh, 5.0);
    let cutter = FlatEndmill::new(6.0, 25.0);

    // Simulate the FULL Back Rough onto a fresh dexel (setup-local stock).
    let sb = session.stock_bbox();
    let (sx, sy, sz) = (
        sb.max.x - sb.min.x,
        sb.max.y - sb.min.y,
        sb.max.z - sb.min.z,
    );
    let local_bbox = Bbox3 {
        min: P3::new(0.0, 0.0, 0.0),
        max: P3::new(sx, sy, sz),
    };
    println!("local stock bbox max = ({sx:.1},{sy:.1},{sz:.1})");
    let mut stock = TriDexelStock::from_bounds(&local_bbox, 0.5);
    let scancel = || false;
    let _ = stock.simulate_toolpath_with_metrics_with_cancel(
        tp_result.toolpath(),
        &cutter,
        StockCutDirection::FromTop,
        tp_id,
        18000,
        2,
        5000.0,
        0.5,
        None,
        &[],
        &[],
        true,
        &scancel,
    );

    // Per dexel cell: final stock top (max exit) and mesh keep-surface.
    // Collect (mesh_z, final_top) for cells inside the toolpath footprint.
    let g = &stock.z_grid;
    let cs = g.cell_size;
    let mut pairs: Vec<(f64, f64, f64, f64)> = Vec::new(); // (mesh_z, final_top, x, y)
    // Sample every 6th cell (~3mm) to keep the analysis light.
    for row in (0..g.rows).step_by(6) {
        let y = g.origin_v + row as f64 * cs;
        for col in (0..g.cols).step_by(6) {
            let x = g.origin_u + col as f64 * cs;
            let ray = &g.rays[row * g.cols + col];
            let top = ray.iter().map(|s| s.exit).fold(f32::NEG_INFINITY, f32::max);
            if !top.is_finite() {
                continue; // fully cut away / empty
            }
            let mz = point_drop_cutter(x, y, &local_mesh, &index, &cutter).z;
            if !mz.is_finite() {
                continue;
            }
            // Restrict to the working footprint (skip untouched stock far from
            // any cut): final_top well below the virgin stock top.
            if (top as f64) > sz - 1.0 {
                continue;
            }
            pairs.push((mz, top as f64, x, y));
        }
    }
    println!("{} sampled cut cells with valid mesh", pairs.len());

    // Group by mesh-height bucket → spread of FINAL stock top (the real
    // measure of "what's left"). leave = final_top - mesh_z. Correct rough
    // keeps leave ~constant; over-cut shows a wide spread / leave near 0.
    use std::collections::BTreeMap;
    let mut buckets: BTreeMap<i32, Vec<f64>> = BTreeMap::new();
    for &(mz, top, _, _) in &pairs {
        buckets
            .entry((mz / 2.0).round() as i32)
            .or_default()
            .push(top - mz);
    }
    println!("mesh-height bucket (2mm) → LEAVE (final_top - mesh) [n: min/mean/max, span]:");
    for (b, leaves) in &buckets {
        let n = leaves.len();
        let lmin = leaves.iter().cloned().fold(f64::INFINITY, f64::min);
        let lmax = leaves.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let lmean = leaves.iter().sum::<f64>() / n as f64;
        println!(
            "  mesh≈{:>6.1}: n={:<5} leave min/mean/max = {:>6.2}/{:>6.2}/{:>6.2}  span={:>5.2}",
            *b as f64 * 2.0,
            n,
            lmin,
            lmean,
            lmax,
            lmax - lmin
        );
    }

    // Same-height test on the real final surface: cells at mesh in [4,6]mm,
    // split by leave. Over-cut = leave < 1mm (ate the ~4mm leave); proper =
    // leave >= 3mm.
    let band: Vec<&(f64, f64, f64, f64)> =
        pairs.iter().filter(|p| p.0 >= 4.0 && p.0 <= 6.0).collect();
    let overcut: Vec<&&(f64, f64, f64, f64)> = band.iter().filter(|p| (p.1 - p.0) < 1.0).collect();
    let proper: Vec<&&(f64, f64, f64, f64)> = band.iter().filter(|p| (p.1 - p.0) >= 3.0).collect();
    println!(
        "SAME-HEIGHT (final surface) — mesh in [4,6]mm: {} cells; {} OVER-CUT (leave<1mm) vs {} PROPER (leave>=3mm)",
        band.len(),
        overcut.len(),
        proper.len()
    );
    if !overcut.is_empty() {
        let n = overcut.len() as f64;
        let (mut xmn, mut xmx, mut ymn, mut ymx) = (
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
        );
        for p in &overcut {
            xmn = xmn.min(p.2);
            xmx = xmx.max(p.2);
            ymn = ymn.min(p.3);
            ymx = ymx.max(p.3);
        }
        println!(
            "  over-cut group: n={} bbox X[{:.1},{:.1}] Y[{:.1},{:.1}] ({:.0}x{:.0}mm); centroid ({:.1},{:.1})",
            overcut.len(),
            xmn,
            xmx,
            ymn,
            ymx,
            xmx - xmn,
            ymx - ymn,
            overcut.iter().map(|p| p.2).sum::<f64>() / n,
            overcut.iter().map(|p| p.3).sum::<f64>() / n,
        );
    }

    // Regression gate for the drape / gouge-guard fix (2026-06-16). The rough
    // must HOLD the stock-to-leave over the textured mesh: no same-height cell
    // may be cut to bare mesh / below it. Pre-fix: 67 of 642 over-cut, worst
    // leave -1.9 mm (cut below the keep surface). Post-fix: 0 over-cut, worst
    // leave >= 0. Allow a tiny slack for grid/dexel discretisation.
    let worst_neg_leave = pairs
        .iter()
        .map(|p| p.1 - p.0)
        .fold(f64::INFINITY, f64::min);
    println!("worst (most-negative) leave across all sampled cut cells = {worst_neg_leave:.2} mm");
    assert!(
        overcut.len() <= 1,
        "stock-to-leave not held: {} same-height cells over-cut (leave<1mm). Pre-fix this was 67; \
         the drape guard should bring it to ~0.",
        overcut.len()
    );
    assert!(
        worst_neg_leave > -0.5,
        "rough cut BELOW the keep surface: worst leave {worst_neg_leave:.2} mm (should be >= 0 \
         minus discretisation slack). The drape guard must hold the leave."
    );
}

#[test]
#[ignore = "expensive WANAKA diagnostic; run with `cargo test --test wanaka_axial_doc -- --ignored`"]
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
        use_predicted_feed_in_gates: false,
        adaptive_feed_modulation: false,
        modulation_strategy: rs_cam_core::feed_modulation::ModulationStrategy::ConstrainedMax,
        modulation_aggressiveness: 1.0,
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
    // FORENSIC 1 — classify the peak sample. Is the 6mm "gouge" a steady
    // lateral cut, or a transit/entry/link sample (which bypasses the
    // surface lift and dives to a target Z)?
    println!(
        "  peak sample: kinematics={:?}  in_transit_span={}  axial_doc={:.3}  axial_engagement={:.3}  plunge_descent={:.3}",
        peak.cut_kinematics,
        peak.in_transit_span,
        peak.axial_doc_mm,
        peak.axial_engagement_mm,
        peak.plunge_descent_mm,
    );
    // Also find the worst STEADY-STATE (non-transit, non-plunge) sample — if
    // that's far lower than the global peak, the gouge is a transit artifact.
    {
        use rs_cam_core::simulation_cut::CutKinematics;
        let mut steady_peak: Option<&rs_cam_core::simulation_cut::SimulationCutSample> = None;
        for s in &cut_trace.samples {
            if s.toolpath_id != tp_id
                || !s.is_cutting
                || s.cut_kinematics == CutKinematics::Plunge
                || s.in_transit_span
            {
                continue;
            }
            match steady_peak {
                None => steady_peak = Some(s),
                Some(p) if s.axial_engagement_mm > p.axial_engagement_mm => steady_peak = Some(s),
                _ => {}
            }
        }
        if let Some(sp) = steady_peak {
            println!(
                "  worst STEADY-STATE sample: axial_engagement={:.3} at ({:.2},{:.2},{:.2}) kin={:?}",
                sp.axial_engagement_mm,
                sp.position[0],
                sp.position[1],
                sp.position[2],
                sp.cut_kinematics
            );
        }
    }
    // FORENSIC 2 — the mesh keep-surface at the peak XY, in the SAME setup-
    // local frame the toolpath uses. If the channel "should be level with the
    // hills", the mesh surface here should be HIGH (~hill level). If the cut
    // Z is far below it, drop-cutter was bypassed → real sub-surface gouge.
    // If the mesh surface here is ~the cut Z, the cut depth is correct and the
    // high axial came from shearing uncleared neighbour stock (parity gap).
    {
        use rs_cam_core::compute::transform::{FaceUp, ZRotation};
        use rs_cam_core::mesh::SpatialIndex;
        use rs_cam_core::tool::FlatEndmill;

        // Resolve the Back Rough's setup face/rotation.
        let tp_setup_idx = session
            .setup_of_toolpath_id(tp_id)
            .expect("setup for back rough");
        let (face_up, z_rot) = session
            .list_setups()
            .get(tp_setup_idx)
            .map(|s| (s.face_up, s.z_rotation))
            .unwrap_or((FaceUp::Bottom, ZRotation::Deg0));
        println!(
            "  Back Rough setup {}: face_up={:?} z_rot={:?}",
            tp_setup_idx, face_up, z_rot
        );

        let model_mesh = session
            .models()
            .iter()
            .find_map(|m| m.mesh.as_ref().map(|mm| mm.as_ref().clone()))
            .expect("wanaka has a mesh model");
        let local_mesh = session
            .setup_transform_info(face_up, z_rot)
            .apply_to_mesh(&model_mesh);
        let index = SpatialIndex::build(&local_mesh, 5.0);
        let cutter = FlatEndmill::new(6.0, 25.0);

        let surf_at =
            |x: f64, y: f64| -> f64 { point_drop_cutter(x, y, &local_mesh, &index, &cutter).z };
        let px = peak.position[0];
        let py = peak.position[1];
        println!(
            "  mesh keep-surface (setup-local) at peak XY ({:.2},{:.2}) = {:.3} ; cut Z = {:.3} ; \
             delta(cut below surface) = {:.3}",
            px,
            py,
            surf_at(px, py),
            peak.position[2],
            surf_at(px, py) - peak.position[2]
        );
        // Cross-channel profile: mesh surface across +/-9mm in X and Y around
        // the peak, to see whether the mesh really is ~level here (hills) or
        // genuinely dips (channel).
        println!("  mesh keep-surface profile around peak (offset: surfX / surfY):");
        for d in [-9.0, -6.0, -3.0, 0.0, 3.0, 6.0, 9.0] {
            println!(
                "    d={:+.1}: surf(x{:+.0})={:.2}  surf(y{:+.0})={:.2}",
                d,
                d,
                surf_at(px + d, py),
                d,
                surf_at(px, py + d)
            );
        }
    }

    // Get the actual toolpath move at peak.move_index.
    let tp_result = session.get_result(1).expect("back rough result");
    let mv = &tp_result.toolpath().moves[peak.move_index];
    let prev = &tp_result.toolpath().moves[peak.move_index.saturating_sub(1)];
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
    let hi = (peak.move_index + 3).min(tp_result.toolpath().moves.len() - 1);
    for j in lo..=hi {
        let m = &tp_result.toolpath().moves[j];
        println!(
            "    move {}: type={:?} target=({:.3},{:.3},{:.3})",
            j, m.move_type, m.target.x, m.target.y, m.target.z
        );
    }

    // Check earlier moves to identify what stage this is. move 98 is early.
    println!("  first 10 moves of Back Rough:");
    for j in 0..10.min(tp_result.toolpath().moves.len()) {
        let m = &tp_result.toolpath().moves[j];
        println!(
            "    move {}: type={:?} target=({:.3},{:.3},{:.3})",
            j, m.move_type, m.target.x, m.target.y, m.target.z
        );
    }

    // Total moves to understand position in toolpath
    println!(
        "  total moves: {}, peak at move {} ({:.1}% through)",
        tp_result.toolpath().moves.len(),
        peak.move_index,
        100.0 * peak.move_index as f64 / tp_result.toolpath().moves.len() as f64
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
        let mut up_to = (*tp_result.toolpath()).clone();
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
            &[],
            &[],
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
        for (j, m) in tp_result.toolpath().moves.iter().enumerate() {
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
    for m in &tp_result.toolpath().moves {
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
    for m in &tp_result.toolpath().moves {
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
    for (j, m) in tp_result.toolpath().moves.iter().enumerate() {
        if matches!(m.move_type, rs_cam_core::toolpath::MoveType::Linear { .. })
            && (m.target.z - 22.0).abs() < 0.5
        {
            z22_feeds.push(j);
        }
    }
    if let Some(&first) = z22_feeds.first() {
        println!("  first z=22 feed at move {}", first);
        let lo = first.saturating_sub(2);
        let hi = (first + 5).min(tp_result.toolpath().moves.len() - 1);
        for j in lo..=hi {
            let m = &tp_result.toolpath().moves[j];
            println!(
                "    move {}: type={:?} ({:.3},{:.3},{:.3})",
                j, m.move_type, m.target.x, m.target.y, m.target.z
            );
        }
    }

    // First 80 moves (verbatim) to see what z=22 cuts look like
    println!("  first 80 moves verbatim:");
    for j in 0..80.min(tp_result.toolpath().moves.len()) {
        let m = &tp_result.toolpath().moves[j];
        println!(
            "    move {}: type={:?} ({:.3},{:.3},{:.3})",
            j, m.move_type, m.target.x, m.target.y, m.target.z
        );
    }

    // Dump transitions: list move ranges by approximate z bucket.
    println!("  z transitions across all moves:");
    let mut prev_z_bucket: Option<i32> = None;
    let mut bucket_start = 0usize;
    for (j, m) in tp_result.toolpath().moves.iter().enumerate() {
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
        let last = tp_result.toolpath().moves.len() - 1;
        println!(
            "    moves {}..{}: ~z={:.1}",
            bucket_start,
            last,
            b as f64 * 3.0
        );
    }
}
