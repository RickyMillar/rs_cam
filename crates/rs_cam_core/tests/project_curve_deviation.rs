//! Regression test for the "phantom cutting move spanning the stock"
//! symptom in `project_curve`.
//!
//! Loads `rivers_aligned.dxf` (the actual file the user hit the bug on),
//! generates a project_curve toolpath over a flat mesh, and measures the
//! perpendicular distance from every cutting move midpoint to the nearest
//! DXF polygon edge.
//!
//! A phantom "close the ring" segment that jumps from a river endpoint back
//! to its start point will sit ~tens of millimetres away from any edge of
//! the DXF. Good cutting moves sit on top of the DXF (distance ~0). So if
//! any move's midpoint is far from all DXF edges, the test fails.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout,
    clippy::collapsible_if,
    clippy::field_reassign_with_default,
    clippy::let_unit_value,
    dead_code
)]

use std::path::PathBuf;

use rs_cam_core::boundary::{ToolContainment, clip_toolpath_to_boundary, effective_boundary};
use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::compute::config::{DressupConfig, DressupEntryStyle};
use rs_cam_core::compute::execute::apply_dressups;
use rs_cam_core::dxf_input::load_dxf;
use rs_cam_core::geo::{P2, P3};
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::project_curve::{ProjectCurveParams, ProjectDirection, project_curve_toolpath};
use rs_cam_core::tool::{FlatEndmill, MillingCutter};
use rs_cam_core::toolpath::{MoveType, Toolpath};
use rs_cam_core::toolpath_spans::AnnotatedToolpath;

/// A flat square mesh covering xy in [x0, x1] × [y0, y1] at z = 0.
fn flat_mesh(x0: f64, x1: f64, y0: f64, y1: f64) -> TriangleMesh {
    let verts = vec![
        P3::new(x0, y0, 0.0),
        P3::new(x1, y0, 0.0),
        P3::new(x1, y1, 0.0),
        P3::new(x0, y1, 0.0),
    ];
    let tris = vec![[0, 1, 2], [0, 2, 3]];
    TriangleMesh::from_raw(verts, tris)
}

/// Flat mesh with circular holes cut out, to model drilled holes in the
/// terrain. Tessellated as a grid where cells whose centers fall inside
/// any hole are dropped.
fn flat_mesh_with_holes(
    x0: f64,
    x1: f64,
    y0: f64,
    y1: f64,
    cell: f64,
    holes: &[(f64, f64, f64)],
) -> TriangleMesh {
    let nx = ((x1 - x0) / cell).round() as usize;
    let ny = ((y1 - y0) / cell).round() as usize;
    let mut verts: Vec<P3> = Vec::with_capacity((nx + 1) * (ny + 1));
    for j in 0..=ny {
        for i in 0..=nx {
            verts.push(P3::new(x0 + i as f64 * cell, y0 + j as f64 * cell, 0.0));
        }
    }
    let in_hole = |cx: f64, cy: f64| -> bool {
        holes.iter().any(|&(hx, hy, hr)| {
            let dx = cx - hx;
            let dy = cy - hy;
            (dx * dx + dy * dy).sqrt() < hr
        })
    };
    let mut tris: Vec<[u32; 3]> = Vec::new();
    for j in 0..ny {
        for i in 0..nx {
            let cx = x0 + (i as f64 + 0.5) * cell;
            let cy = y0 + (j as f64 + 0.5) * cell;
            if in_hole(cx, cy) {
                continue;
            }
            let row = (nx + 1) as u32;
            let v00 = j as u32 * row + i as u32;
            let v10 = v00 + 1;
            let v01 = v00 + row;
            let v11 = v01 + 1;
            tris.push([v00, v10, v11]);
            tris.push([v00, v11, v01]);
        }
    }
    TriangleMesh::from_raw(verts, tris)
}

/// Perpendicular distance from point `p` to segment `(a, b)` in 2D.
fn dist_point_to_segment(p: P2, a: P2, b: P2) -> f64 {
    let ab_x = b.x - a.x;
    let ab_y = b.y - a.y;
    let ap_x = p.x - a.x;
    let ap_y = p.y - a.y;
    let ab_sq = ab_x * ab_x + ab_y * ab_y;
    let t = if ab_sq > 1e-12 {
        ((ap_x * ab_x + ap_y * ab_y) / ab_sq).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let dx = p.x - (a.x + t * ab_x);
    let dy = p.y - (a.y + t * ab_y);
    (dx * dx + dy * dy).sqrt()
}

/// Minimum distance from `p` to any edge of any polygon ring.
fn min_dist_to_polygons(p: P2, polygons: &[Polygon2]) -> f64 {
    let mut best = f64::INFINITY;
    for poly in polygons {
        for ring in std::iter::once(&poly.exterior).chain(poly.holes.iter()) {
            if ring.len() < 2 {
                continue;
            }
            // Walk consecutive edges. If the polygon is closed, also
            // include the last→first closing edge — a valid cutting move
            // that lies on this closing segment is *not* phantom.
            for w in ring.windows(2) {
                let d = dist_point_to_segment(p, w[0], w[1]);
                if d < best {
                    best = d;
                }
            }
            if poly.closed && ring.len() >= 3 {
                let first = ring[0];
                let last = ring[ring.len() - 1];
                let d = dist_point_to_segment(p, last, first);
                if d < best {
                    best = d;
                }
            }
        }
    }
    best
}

fn collect_cut_move_midpoints(tp: &Toolpath) -> Vec<P2> {
    collect_cut_moves(tp)
        .into_iter()
        .map(|(start, end)| P2::new((start.x + end.x) * 0.5, (start.y + end.y) * 0.5))
        .collect()
}

/// Return every feed-class move as (start, end) XY pairs.
fn collect_cut_moves(tp: &Toolpath) -> Vec<(P2, P2)> {
    let mut out = Vec::new();
    let mut prev: Option<P3> = None;
    for mv in &tp.moves {
        let target = mv.target;
        if let Some(p) = prev {
            let is_cut = matches!(
                mv.move_type,
                MoveType::Linear { .. } | MoveType::ArcCW { .. } | MoveType::ArcCCW { .. }
            );
            if is_cut {
                out.push((P2::new(p.x, p.y), P2::new(target.x, target.y)));
            }
        }
        prev = Some(target);
    }
    out
}

fn fixture_path(name: &str) -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("tests");
    p.push("fixtures");
    p.push(name);
    p
}

#[test]
fn live_project_pc6_has_no_phantom_cuts() {
    // Load the actual project the user is running: bottom-face setup,
    // terrain.stl + rivers_aligned.dxf, PC6 at toolpath id 12. Runs the
    // full session-level pipeline — the same one the GUI invokes — and
    // inspects the final toolpath for phantom lateral cuts at depth.
    use rs_cam_core::session::ProjectSession;

    let path = fixture_path("test_job.toml");
    let mut session = ProjectSession::load(&path).expect("live project loads");
    println!(
        "Loaded session: {} setups, {} toolpaths, {} models",
        session.list_setups().len(),
        session.toolpath_configs().len(),
        session.models().len()
    );
    for m in session.models() {
        println!(
            "  model id={} name={} kind={:?} mesh={} polys={}",
            m.id,
            m.name,
            m.kind,
            m.mesh.is_some(),
            m.polygons.as_ref().map(|p| p.len()).unwrap_or(0)
        );
    }

    // Find PC6 (id 12)
    let (pc6_idx, _pc6) = session
        .toolpath_configs()
        .iter()
        .enumerate()
        .find(|(_, tc)| tc.id == 12)
        .expect("PC6 exists in fixture");

    let cancel = std::sync::atomic::AtomicBool::new(false);
    let _ = session.generate_all(&[], &cancel);
    let sim_opts = rs_cam_core::session::SimulationOptions {
        resolution: 0.5,
        auto_resolution: true,
        metrics_enabled: false,
        skip_ids: Vec::new(),
    };
    let _ = session.run_simulation(&sim_opts, &cancel);
    let result = session
        .generate_toolpath(pc6_idx, &cancel)
        .expect("PC6 generates");

    println!(
        "PC6 toolpath: {} moves, cutting={:.1}mm, rapid={:.1}mm",
        result.stats.move_count, result.stats.cutting_distance, result.stats.rapid_distance
    );

    // Load polygons fresh to compare. Polygons live in setup-local frame
    // after the compute step transformed them, but since face_up=Bottom
    // only flips Z (not XY), the 2D footprint matches the DXF.
    let polygons = rs_cam_core::dxf_input::load_dxf(&fixture_path("rivers_aligned.dxf"), 5.0)
        .expect("DXF loads");

    let final_tp = &result.toolpath;
    let lateral_count = count_lateral_cuts_at_depth(final_tp, 9.5, 1.0);
    println!(
        "Live PC6 pipeline lateral feed moves > 1mm at cut-depth: {}",
        lateral_count
    );

    // Dump the longest lateral cut-depth moves so we can see exactly what
    // the phantom cuts are (if any).
    let mut prev: Option<P3> = None;
    let mut laterals: Vec<(f64, P3, P3)> = Vec::new();
    for mv in &final_tp.moves {
        let target = mv.target;
        if matches!(mv.move_type, MoveType::Linear { .. }) {
            if let Some(p) = prev {
                let dxy = ((target.x - p.x).powi(2) + (target.y - p.y).powi(2)).sqrt();
                if p.z < 9.5 && target.z < 9.5 && dxy > 1.0 {
                    laterals.push((dxy, p, target));
                }
            }
        }
        prev = Some(target);
    }
    laterals.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    for (dxy, p, t) in laterals.iter().take(20) {
        let mid = P2::new((p.x + t.x) * 0.5, (p.y + t.y) * 0.5);
        let dmid = min_dist_to_polygons(mid, &polygons);
        println!(
            "  dxy={:>7.2}mm  ({:>7.2},{:>7.2},{:>6.2}) → ({:>7.2},{:>7.2},{:>6.2})  mid-d-from-dxf={:.2}mm",
            dxy, p.x, p.y, p.z, t.x, t.y, t.z, dmid
        );
    }

    // This is the test that mirrors what the user sees. Fail loudly.
    assert_eq!(
        lateral_count, 0,
        "Live PC6 pipeline produced {lateral_count} lateral feed moves > 1mm at cut depth. \
         These are the phantom cuts visible in the stock."
    );
}

#[test]
fn project_curve_cutting_moves_follow_rivers_dxf() {
    let dxf_path = fixture_path("rivers_aligned.dxf");
    let polygons = load_dxf(&dxf_path, 5.0).expect("rivers_aligned.dxf loads");
    assert!(!polygons.is_empty(), "DXF should contain polygons");

    // Flat mesh spanning the DXF footprint with a small margin.
    let mut bx0 = f64::INFINITY;
    let mut bx1 = f64::NEG_INFINITY;
    let mut by0 = f64::INFINITY;
    let mut by1 = f64::NEG_INFINITY;
    for poly in &polygons {
        for p in &poly.exterior {
            bx0 = bx0.min(p.x);
            bx1 = bx1.max(p.x);
            by0 = by0.min(p.y);
            by1 = by1.max(p.y);
        }
    }
    // Load the actual terrain.stl from the live project — that's the mesh
    // PC6 projects onto. The user reports phantom cuts persist with real
    // terrain even after DXF + hole fixes, so testing against a flat mesh
    // hides the bug.
    let _ = bx0;
    let _ = bx1;
    let _ = by0;
    let _ = by1;
    let mesh_path = fixture_path("terrain.stl");
    let mesh = TriangleMesh::from_stl_scaled(&mesh_path, 1.0).expect("terrain.stl fixture loads");
    let spatial = SpatialIndex::build_auto(&mesh);
    println!(
        "terrain mesh: {} tris, bbox x=[{:.2},{:.2}] y=[{:.2},{:.2}] z=[{:.2},{:.2}]",
        mesh.faces.len(),
        mesh.bbox.min.x,
        mesh.bbox.max.x,
        mesh.bbox.min.y,
        mesh.bbox.max.y,
        mesh.bbox.min.z,
        mesh.bbox.max.z,
    );

    let cutter = FlatEndmill::new(1.0, 10.0);
    let params = ProjectCurveParams {
        depth: 0.5,
        feed_rate: 1000.0,
        plunge_rate: 300.0,
        safe_z: 10.0,
        point_spacing: 0.5,
        direction: ProjectDirection::FromAbove,
        tool_radius: 0.0,
        side: rs_cam_core::project_curve::ProjectSide::Center,
        setup_z_flipped: false,
    };

    // Generate a toolpath per polygon, measuring each against ONLY that
    // polygon's edges — otherwise a phantom close-ring segment on polygon A
    // could coincidentally pass near polygon B and get masked. Per-polygon
    // checking eliminates that false-negative.
    let mut all_moves = Toolpath::new();
    let mut per_poly_offenders = 0usize;
    let mut per_poly_total = 0usize;
    let mut per_poly_worst = 0.0f64;
    let mut worst_poly_idx = 0usize;
    let mut worst_closed_flag = false;
    for (idx, poly) in polygons.iter().enumerate() {
        let tp = project_curve_toolpath(
            poly,
            &mesh,
            &spatial,
            &cutter as &dyn MillingCutter,
            &params,
        );
        let midpoints = collect_cut_move_midpoints(&tp);
        let single = std::slice::from_ref(poly);
        for mp in &midpoints {
            let d = min_dist_to_polygons(*mp, single);
            per_poly_total += 1;
            if d > 1.0 {
                per_poly_offenders += 1;
            }
            if d > per_poly_worst {
                per_poly_worst = d;
                worst_poly_idx = idx;
                worst_closed_flag = poly.closed;
            }
        }
        for mv in &tp.moves {
            all_moves.moves.push(mv.clone());
        }
    }
    println!(
        "per-polygon:                 {:>6}/{:<6} midpoints >1mm from OWN polygon \
         ({:>5.2}%, worst = {:.2}mm @ poly #{} closed={})",
        per_poly_offenders,
        per_poly_total,
        (per_poly_offenders as f64 / per_poly_total.max(1) as f64) * 100.0,
        per_poly_worst,
        worst_poly_idx,
        worst_closed_flag,
    );

    report("project_curve (raw)", &all_moves, &polygons);

    // Now run the same toolpath through apply_dressups with link_moves=true
    // — that mirrors how a stale/default PC6 toolpath config actually
    // renders. Link moves bridge fragments at cutting depth, which is
    // exactly the "phantom straight line across the stock" the user sees.
    let mut with_links = DressupConfig::default();
    with_links.link_moves = true;
    with_links.link_max_distance = 10.0;
    let tp_with_links = apply_dressups(
        AnnotatedToolpath::new(all_moves.clone()),
        &with_links,
        1.0,
        10.0,
        None,
        None,
        None,
        OperationType::ProjectCurve.transform_capabilities(),
        None,
        None,
    )
    .toolpath;
    report("project_curve + link_moves", &tp_with_links, &polygons);

    // And without link_moves — the fixed default.
    let tp_no_links = apply_dressups(
        AnnotatedToolpath::new(all_moves.clone()),
        &DressupConfig::default(),
        1.0,
        10.0,
        None,
        None,
        None,
        OperationType::ProjectCurve.transform_capabilities(),
        None,
        None,
    )
    .toolpath;
    let (offenders, total, worst) = report("project_curve (no links)", &tp_no_links, &polygons);

    // PC6's dressup defaults *come from* `for_role(Finish)`, which enables
    // entry_style=Ramp + lead_in_out=true. Model what those produce so we
    // can see which ones introduce phantom lateral moves at cut depth.
    let mut finish_defaults = DressupConfig::default();
    finish_defaults.entry_style = DressupEntryStyle::Ramp;
    finish_defaults.ramp_angle = 3.0;
    finish_defaults.lead_in_out = true;
    finish_defaults.lead_radius = 2.0;
    let tp_finish = apply_dressups(
        AnnotatedToolpath::new(all_moves.clone()),
        &finish_defaults,
        1.0,
        10.0,
        None,
        None,
        None,
        OperationType::ProjectCurve.transform_capabilities(),
        None,
        None,
    )
    .toolpath;
    report("+ finish defaults", &tp_finish, &polygons);

    // Same but ramp only (no lead-in/out).
    let mut ramp_only = DressupConfig::default();
    ramp_only.entry_style = DressupEntryStyle::Ramp;
    ramp_only.ramp_angle = 3.0;
    let tp_ramp = apply_dressups(
        AnnotatedToolpath::new(all_moves.clone()),
        &ramp_only,
        1.0,
        10.0,
        None,
        None,
        None,
        OperationType::ProjectCurve.transform_capabilities(),
        None,
        None,
    )
    .toolpath;
    report("+ ramp entry only", &tp_ramp, &polygons);

    // Dump the longest feed-at-cut-depth moves in each variant so we can
    // see exactly which segments produce the "straight lines across the
    // stock" symptom. Ignores plunges/retracts (XY distance < 0.01 mm).
    dump_longest_moves("raw project_curve", &all_moves, &polygons);
    dump_longest_moves("+ link_moves", &tp_with_links, &polygons);
    dump_longest_moves("+ no links", &tp_no_links, &polygons);
    dump_longest_moves("+ finish defaults", &tp_finish, &polygons);
    dump_longest_moves("+ ramp entry only", &tp_ramp, &polygons);

    // Z-jump report: consecutive feed moves whose Z differs by more than
    // stepover suggest the tool is jumping vertically between very
    // different surface heights — often a sign of contact on a mesh-hole
    // rim or a sudden geometry transition.
    let mut z_jumps: Vec<(f64, P3, P3)> = Vec::new();
    let mut prev: Option<P3> = None;
    for mv in &all_moves.moves {
        let target = mv.target;
        if let Some(p) = prev {
            if matches!(mv.move_type, MoveType::Linear { .. }) {
                let dz = (target.z - p.z).abs();
                let dxy = ((target.x - p.x).powi(2) + (target.y - p.y).powi(2)).sqrt();
                // Flag moves that dive/climb >0.5mm over short XY (non-plunge)
                if dz > 0.5 && dxy > 0.1 {
                    z_jumps.push((dz, p, target));
                }
            }
        }
        prev = Some(target);
    }
    z_jumps.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    println!(
        "Z-jumps in raw project_curve output ({} total, top 10):",
        z_jumps.len()
    );
    for (dz, p, t) in z_jumps.iter().take(20) {
        let dxy = ((t.x - p.x).powi(2) + (t.y - p.y).powi(2)).sqrt();
        println!(
            "  dz={:>6.2}mm  dxy={:>6.2}mm  ({:>6.2},{:>6.2},{:>6.2}) → ({:>6.2},{:>6.2},{:>6.2})",
            dz, dxy, p.x, p.y, p.z, t.x, t.y, t.z
        );
    }

    // Dedicated phantom-cut check: a **cut-depth lateral feed move** is
    // any Linear move where BOTH endpoints are at the cutting surface
    // (Z well below safe_z) and XY distance is meaningful. Those must
    // never be > point_spacing * safety_factor; anything longer is a
    // bridge between fragments at depth — the actual phantom symptom.
    let safe_z = 10.0;
    let cut_threshold_z = safe_z - 0.5; // anything below this is "at cut depth"
    let mut lateral_cuts: Vec<(f64, P3, P3)> = Vec::new();
    let mut prev_cut: Option<P3> = None;
    for mv in &all_moves.moves {
        let target = mv.target;
        if matches!(mv.move_type, MoveType::Linear { .. }) {
            if let Some(p) = prev_cut {
                let dxy = ((target.x - p.x).powi(2) + (target.y - p.y).powi(2)).sqrt();
                let both_cut = p.z < cut_threshold_z && target.z < cut_threshold_z;
                if both_cut && dxy > 1.0 {
                    lateral_cuts.push((dxy, p, target));
                }
            }
        }
        prev_cut = Some(target);
    }
    lateral_cuts.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    println!(
        "Lateral feed moves at cut-depth > 1mm ({} — these are the phantom cuts):",
        lateral_cuts.len()
    );
    for (dxy, p, t) in lateral_cuts.iter().take(15) {
        println!(
            "  dxy={:>6.2}mm  ({:>6.2},{:>6.2},{:>6.2}) → ({:>6.2},{:>6.2},{:>6.2})",
            dxy, p.x, p.y, p.z, t.x, t.y, t.z
        );
    }

    // Also flag long 3D feed moves explicitly — these combine XY and Z
    // into a single diagonal line that's visible in any viewport.
    let mut long3d: Vec<(f64, P3, P3)> = Vec::new();
    let mut prev: Option<P3> = None;
    for mv in &all_moves.moves {
        let target = mv.target;
        if let Some(p) = prev {
            if matches!(mv.move_type, MoveType::Linear { .. }) {
                let d = ((target.x - p.x).powi(2)
                    + (target.y - p.y).powi(2)
                    + (target.z - p.z).powi(2))
                .sqrt();
                if d > 1.5 {
                    long3d.push((d, p, target));
                }
            }
        }
        prev = Some(target);
    }
    long3d.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    println!(
        "Long 3D feed moves (>1.5mm, {} total, top 15):",
        long3d.len()
    );
    for (d, p, t) in long3d.iter().take(15) {
        let dxy = ((t.x - p.x).powi(2) + (t.y - p.y).powi(2)).sqrt();
        let dz = (t.z - p.z).abs();
        println!(
            "  d3d={:>6.2}  xy={:>6.2}  z={:>5.2}  ({:>6.2},{:>6.2},{:>6.2}) → ({:>6.2},{:>6.2},{:>6.2})",
            d, dxy, dz, p.x, p.y, p.z, t.x, t.y, t.z
        );
    }

    // Diagnose the DXF itself: look for long edges between *adjacent*
    // vertices of a ring. A CAD exporter may emit a polyline whose
    // vertices encode a pen-up by a long straight segment to the next
    // sub-path; project_curve's resampling then produces an uninterrupted
    // line of feed moves between them.
    let mut long_edges: Vec<(f64, P2, P2, usize)> = Vec::new();
    for (i, poly) in polygons.iter().enumerate() {
        let mut scan = |ring: &[P2]| {
            for pair in ring.windows(2) {
                let a = pair[0];
                let b = pair[1];
                let dx = b.x - a.x;
                let dy = b.y - a.y;
                let d = (dx * dx + dy * dy).sqrt();
                if d > 1.0 {
                    long_edges.push((d, a, b, i));
                }
            }
        };
        scan(&poly.exterior);
        for hole in &poly.holes {
            scan(hole);
        }
    }
    long_edges.sort_by(|x, y| y.0.partial_cmp(&x.0).unwrap_or(std::cmp::Ordering::Equal));
    println!(
        "DXF edges > 1mm between adjacent ring vertices (top 15 of {}):",
        long_edges.len()
    );
    for (len, a, b, poly_idx) in long_edges.iter().take(15) {
        println!(
            "  len={:>7.2}mm  poly #{}  ({:>7.2},{:>7.2}) → ({:>7.2},{:>7.2})",
            len, poly_idx, a.x, a.y, b.x, b.y
        );
    }

    // Threshold assertion: the no-dressup / no-link variant must stay on
    // the DXF. Allows a handful of sub-millimetre outliers from resampling
    // drift, but flags any substantial fraction as phantom.
    let pct = (offenders as f64 / total as f64) * 100.0;
    assert!(
        pct < 0.5,
        "project_curve produced {offenders}/{total} cutting-move midpoints ({pct:.2}%) \
         more than 1 mm away from any DXF edge (worst = {worst:.2} mm). Phantom cutting \
         moves detected — likely from close_ring on an open path, or from an importer \
         auto-closing open geometry."
    );

    // Stage-by-stage check with boundary clipping (the final stage in the
    // session compute pipeline).
    let stock_poly = rs_cam_core::polygon::Polygon2::rectangle(-5.0, -5.0, 115.0, 115.0);
    let boundaries = effective_boundary(&stock_poly, ToolContainment::Inside, 0.5);
    if let Some(boundary) = boundaries.first() {
        let clipped = clip_toolpath_to_boundary(&all_moves, boundary, 10.0);
        let post_clip_lat = count_lateral_cuts_at_depth(&clipped, 9.5, 1.0);
        println!(
            "clip_toolpath_to_boundary: {} lateral feed moves > 1mm at cut-depth",
            post_clip_lat,
        );
    }

    // XY-footprint rasterization check: trace every Linear feed move at
    // cut depth into a grid and compare with the DXF mask. Any cell
    // rasterized but not near any DXF edge is a phantom carve.
    footprint_rasterization_check(&all_moves, &polygons, -5.0, 115.0, -5.0, 115.0, 0.5);

    // Real dexel-stock simulation: the toolpath is clean per the checks
    // above, but the user sees phantom gouges in the sim stock. Stamp the
    // generated toolpath onto a fresh dexel stock and look for carved
    // voxels whose (x, y) lies outside any DXF edge. Those would be
    // bugs in the simulator, not the toolpath.
    dexel_sim_carve_check(&all_moves, &polygons, &cutter);

    // Hard guard against lateral phantom cuts at cut depth. Any feed move
    // that spans > 1mm in XY while both endpoints are below safe_z is a
    // bridge between fragments — the exact symptom of "straight green
    // lines across the stock".
    let lateral_count = {
        let mut count = 0usize;
        let mut prev: Option<P3> = None;
        for mv in &all_moves.moves {
            let target = mv.target;
            if matches!(mv.move_type, MoveType::Linear { .. }) {
                if let Some(p) = prev {
                    let dxy = ((target.x - p.x).powi(2) + (target.y - p.y).powi(2)).sqrt();
                    let both_cut = p.z < 9.5 && target.z < 9.5;
                    if both_cut && dxy > 1.0 {
                        count += 1;
                    }
                }
            }
            prev = Some(target);
        }
        count
    };
    assert_eq!(
        lateral_count, 0,
        "project_curve emitted {lateral_count} lateral feed moves > 1mm at cut depth. \
         These are phantom bridges between path fragments."
    );
}

/// Stamp the toolpath onto a dexel stock and look for carved columns
/// whose (x, y) lies more than 2mm from any DXF edge. A clean toolpath
/// that produces carved stock far from the DXF indicates a simulator bug.
fn dexel_sim_carve_check(tp: &Toolpath, polygons: &[Polygon2], cutter: &FlatEndmill) {
    use rs_cam_core::dexel_stock::{StockCutDirection, TriDexelStock};
    use rs_cam_core::geo::BoundingBox3;

    let bbox = BoundingBox3 {
        min: P3::new(-5.0, -5.0, -10.0),
        max: P3::new(115.0, 115.0, 5.0),
    };
    let mut stock = TriDexelStock::from_bounds(&bbox, 0.5);
    let _ = stock.simulate_toolpath(tp, cutter as &dyn MillingCutter, StockCutDirection::FromTop);

    // Scan Z-grid columns for "carved" cells (surface height below initial top)
    // and check if they're near any DXF edge.
    let grid = &stock.z_grid;
    let initial_top = bbox.max.z;
    let mut phantom_carves = 0usize;
    let mut examples: Vec<(f64, f64, f32, f64)> = Vec::new();
    let cols = grid.cols;
    let rows = grid.rows;
    for row in 0..rows {
        for col in 0..cols {
            let Some(cell_top) = grid.top_z_at(row, col) else {
                continue;
            };
            if cell_top as f64 >= initial_top - 0.05 {
                continue; // not carved
            }
            let x = grid.origin_u + col as f64 * grid.cell_size + grid.cell_size * 0.5;
            let y = grid.origin_v + row as f64 * grid.cell_size + grid.cell_size * 0.5;
            let d = min_dist_to_polygons(P2::new(x, y), polygons);
            if d > 2.0 {
                phantom_carves += 1;
                if examples.len() < 20 {
                    examples.push((x, y, cell_top, d));
                }
            }
        }
    }
    println!(
        "Dexel sim stock: {} carved columns > 2mm from any DXF edge.",
        phantom_carves
    );
    for (x, y, z, d) in &examples {
        println!(
            "  phantom column carved to z={:.2} at ({:.2},{:.2}) d={:.2}mm",
            z, x, y, d
        );
    }
}

/// Rasterize every Linear cut-depth move onto a grid and compare with the
/// DXF footprint. Any raster cell that gets carved but is > 2mm from any
/// DXF edge is a phantom carve — these are the ones producing gouges in
/// the voxel stock that don't match the DXF.
fn footprint_rasterization_check(
    tp: &Toolpath,
    polygons: &[Polygon2],
    x0: f64,
    x1: f64,
    y0: f64,
    y1: f64,
    cell: f64,
) {
    let nx = ((x1 - x0) / cell).ceil() as usize;
    let ny = ((y1 - y0) / cell).ceil() as usize;
    let mut carved = vec![false; nx * ny];
    let mark = |carved: &mut [bool], x: f64, y: f64| {
        let ix = ((x - x0) / cell) as isize;
        let iy = ((y - y0) / cell) as isize;
        if ix >= 0 && iy >= 0 && (ix as usize) < nx && (iy as usize) < ny {
            carved[iy as usize * nx + ix as usize] = true;
        }
    };
    let mut prev: Option<P3> = None;
    for mv in &tp.moves {
        let target = mv.target;
        if matches!(mv.move_type, MoveType::Linear { .. }) {
            if let Some(p) = prev {
                let both_cut = p.z < 9.5 && target.z < 9.5;
                if both_cut {
                    // Raster sample along the segment at cell/2 steps
                    let dxy = ((target.x - p.x).powi(2) + (target.y - p.y).powi(2)).sqrt();
                    let n = (dxy / (cell * 0.5)).ceil() as usize + 1;
                    for i in 0..=n {
                        let t = if n == 0 { 0.0 } else { i as f64 / n as f64 };
                        let x = p.x + t * (target.x - p.x);
                        let y = p.y + t * (target.y - p.y);
                        mark(&mut carved, x, y);
                    }
                }
            }
        }
        prev = Some(target);
    }
    let mut phantom_cells = 0usize;
    let mut phantom_examples: Vec<(f64, f64, f64)> = Vec::new();
    for iy in 0..ny {
        for ix in 0..nx {
            if !carved[iy * nx + ix] {
                continue;
            }
            let cx = x0 + (ix as f64 + 0.5) * cell;
            let cy = y0 + (iy as f64 + 0.5) * cell;
            let d = min_dist_to_polygons(P2::new(cx, cy), polygons);
            if d > 2.0 {
                phantom_cells += 1;
                if phantom_examples.len() < 20 {
                    phantom_examples.push((cx, cy, d));
                }
            }
        }
    }
    println!(
        "Footprint raster: {} carved cells > 2mm from any DXF edge (cell size {:.1}mm).",
        phantom_cells, cell
    );
    for (cx, cy, d) in &phantom_examples {
        println!(
            "  phantom carve at ({:.2},{:.2})  d={:.2}mm from nearest DXF edge",
            cx, cy, d
        );
    }
}

fn count_lateral_cuts_at_depth(tp: &Toolpath, cut_threshold_z: f64, dxy_min: f64) -> usize {
    let mut count = 0usize;
    let mut prev: Option<P3> = None;
    for mv in &tp.moves {
        let target = mv.target;
        if matches!(mv.move_type, MoveType::Linear { .. }) {
            if let Some(p) = prev {
                let dxy = ((target.x - p.x).powi(2) + (target.y - p.y).powi(2)).sqrt();
                let both_cut = p.z < cut_threshold_z && target.z < cut_threshold_z;
                if both_cut && dxy > dxy_min {
                    count += 1;
                }
            }
        }
        prev = Some(target);
    }
    count
}

fn dump_longest_moves(label: &str, tp: &Toolpath, polygons: &[Polygon2]) {
    let moves = collect_cut_moves(tp);
    let mut ranked: Vec<(f64, P2, P2)> = moves
        .iter()
        .map(|(a, b)| {
            let dx = b.x - a.x;
            let dy = b.y - a.y;
            ((dx * dx + dy * dy).sqrt(), *a, *b)
        })
        // Plunges/retracts have near-zero XY length; we want lateral cut moves.
        .filter(|(len, _, _)| *len > 0.01)
        .collect();
    ranked.sort_by(|x, y| y.0.partial_cmp(&x.0).unwrap_or(std::cmp::Ordering::Equal));
    println!("top 10 longest feed moves (XY) in {label}:");
    for (len, a, b) in ranked.iter().take(10) {
        let da = min_dist_to_polygons(*a, polygons);
        let db = min_dist_to_polygons(*b, polygons);
        let mid = P2::new((a.x + b.x) * 0.5, (a.y + b.y) * 0.5);
        let dmid = min_dist_to_polygons(mid, polygons);
        println!(
            "  len={:>7.2}mm  start=({:>7.2},{:>7.2}) [d={:.2}]  end=({:>7.2},{:>7.2}) [d={:.2}]  mid-d={:.2}",
            len, a.x, a.y, da, b.x, b.y, db, dmid,
        );
    }
}

fn report(label: &str, tp: &Toolpath, polygons: &[Polygon2]) -> (usize, usize, f64) {
    let midpoints = collect_cut_move_midpoints(tp);
    const PHANTOM_THRESHOLD_MM: f64 = 1.0;
    let mut worst = 0.0f64;
    let mut offenders = 0usize;
    for mp in &midpoints {
        let d = min_dist_to_polygons(*mp, polygons);
        if d > PHANTOM_THRESHOLD_MM {
            offenders += 1;
        }
        if d > worst {
            worst = d;
        }
    }
    let total = midpoints.len();
    let pct = (offenders as f64 / total.max(1) as f64) * 100.0;
    println!(
        "{label:30} {offenders:>6}/{total:<6} midpoints >1mm from DXF ({pct:>5.2}%, worst = {worst:.2}mm)",
    );
    (offenders, total, worst)
}
