//! Unit tests for pencil finishing. Moved out of `finish/pencil.rs` by P4;
//! the module body is unchanged.

#![allow(clippy::unwrap_used, clippy::panic)]

use crate::finish::pencil_dihedral::{
    SharedEdge, build_edge_adjacency, chain_concave_edges, compute_shared_edges,
};
use crate::geo::resample_polyline;

use super::chain_paths::{FAIRING_PASSES, FAIRING_STRENGTH, fair_polyline_xy};
use super::detectors::resolve_reference_cutter;
use super::emission::{LinkLift, emit_paths, plan_link_lift};
use super::*;
use crate::mesh::{SpatialIndex, make_test_hemisphere};
use crate::tool::BallEndmill;

/// Create a flat-only mesh (convex-only, no concave edges). Also kept
/// (duplicated) as a tiny same-module helper in `pencil_dihedral::tests`
/// for its own concavity-detection unit tests.
fn make_convex_box(size: f64) -> TriangleMesh {
    // Simple flat square — no concave edges possible with 2 triangles
    let vertices = vec![
        P3::new(0.0, 0.0, 0.0),
        P3::new(size, 0.0, 0.0),
        P3::new(size, size, 0.0),
        P3::new(0.0, size, 0.0),
    ];
    let triangles = vec![[0, 1, 2], [0, 2, 3]];
    TriangleMesh::from_raw(vertices, triangles)
}

/// Coverage ported from `pencil_dihedral`'s retired `sample_chain` (the
/// un-offset predecessor of `resample_polyline`): a straight 20mm
/// segment sampled at 2mm spacing should yield ~11 points including both
/// endpoints, all still exactly on the source line.
#[test]
fn test_resample_polyline_spacing() {
    let line = vec![P3::new(0.0, 0.0, -5.0), P3::new(20.0, 0.0, -5.0)];
    let points = resample_polyline(&line, 2.0);

    assert!(
        points.len() >= 8,
        "Should get at least 8 sample points on 20mm line at 2mm spacing, got {}",
        points.len()
    );
    for p in &points {
        assert!(
            (p.y - 0.0).abs() < 0.1,
            "Points should be at y=0, got y={}",
            p.y
        );
        assert!(
            (p.z - (-5.0)).abs() < 0.1,
            "Points should be at z=-5, got z={}",
            p.z
        );
    }
    let first = points.first().unwrap();
    let last = points.last().unwrap();
    assert!((first.x - 0.0).abs() < 1e-9, "should preserve first vertex");
    assert!((last.x - 20.0).abs() < 1e-9, "should preserve last vertex");
}

#[test]
fn test_pencil_toolpath_v_groove() {
    let mesh = make_v_valley(30.0, 10.0, 0.5, 30, 40);
    let index = SpatialIndex::build(&mesh, 5.0);
    let tool = BallEndmill::new(6.0, 25.0);

    let params = PencilParams {
        bitangency_angle: 170.0,
        min_cut_length: 5.0,
        hookup_distance: 20.0,
        offset_stepover: 1.5,
        sampling: 1.0,
        ..Default::default()
    };

    let tp = pencil_toolpath(&mesh, &index, &tool, &params);
    assert!(
        !tp.moves.is_empty(),
        "V-groove should produce pencil toolpath moves"
    );
}

#[test]
fn test_pencil_toolpath_convex_empty() {
    let mesh = make_convex_box(50.0);
    let index = SpatialIndex::build(&mesh, 10.0);
    let tool = BallEndmill::new(6.0, 25.0);

    let params = PencilParams {
        min_cut_length: 1.0,
        hookup_distance: 20.0,
        offset_stepover: 1.5,
        ..Default::default()
    };

    let tp = pencil_toolpath(&mesh, &index, &tool, &params);
    assert!(
        tp.moves.is_empty(),
        "Convex mesh should produce empty pencil toolpath"
    );
}

#[test]
fn test_pencil_with_offset_passes() {
    let mesh = make_v_valley(30.0, 10.0, 0.5, 30, 40);
    let index = SpatialIndex::build(&mesh, 5.0);
    let tool = BallEndmill::new(6.0, 25.0);

    let params_center = PencilParams {
        bitangency_angle: 170.0,
        min_cut_length: 5.0,
        hookup_distance: 20.0,
        offset_stepover: 1.5,
        sampling: 1.0,
        ..Default::default()
    };

    let params_offset = PencilParams {
        num_offset_passes: 2,
        // Set explicitly so `..params_center` never moves the non-Copy
        // `reference_cutter` / `link_kinematics`, keeping `params_center`
        // usable below.
        reference_cutter: None,
        link_kinematics: None,
        ..params_center
    };

    let tp_center = pencil_toolpath(&mesh, &index, &tool, &params_center);
    let tp_offset = pencil_toolpath(&mesh, &index, &tool, &params_offset);

    // Offset passes should produce more moves
    assert!(
        tp_offset.moves.len() > tp_center.moves.len(),
        "Offset passes ({}) should produce more moves than center-only ({})",
        tp_offset.moves.len(),
        tp_center.moves.len()
    );
}

/// Finely-tessellated gentle sine·sine surface. Wavelength (20mm) and
/// amplitude (0.3mm) give a local concave radius of curvature ~30mm — far
/// larger than a 1mm tool's radius, so a 1mm ball REACHES every trough.
/// Correct tool-radius-aware pencil output for a 1mm tool here is ~empty.
/// Today's raw-dihedral detector instead fires on every trough edge.
fn make_gentle_undulating_surface(extent: f64, n: usize, lambda: f64, amp: f64) -> TriangleMesh {
    use std::f64::consts::PI;
    let mut vertices = Vec::with_capacity((n + 1) * (n + 1));
    let step = extent / n as f64;
    for j in 0..=n {
        for i in 0..=n {
            let x = i as f64 * step;
            let y = j as f64 * step;
            let z = amp * (2.0 * PI * x / lambda).sin() * (2.0 * PI * y / lambda).sin();
            vertices.push(P3::new(x, y, z));
        }
    }
    let idx = |i: usize, j: usize| (j * (n + 1) + i) as u32;
    let mut triangles = Vec::with_capacity(n * n * 2);
    for j in 0..n {
        for i in 0..n {
            // CCW for upward (+Z) normals on a heightfield.
            triangles.push([idx(i, j), idx(i + 1, j), idx(i + 1, j + 1)]);
            triangles.push([idx(i, j), idx(i + 1, j + 1), idx(i, j + 1)]);
        }
    }
    TriangleMesh::from_raw(vertices, triangles)
}

/// Count distinct pencil chains produced for a given tool, exercising the
/// detection→gate→chaining path (the geometry the toolpath is built from),
/// including the tool-radius-aware reach-gap gate.
fn pencil_chain_count(
    mesh: &TriangleMesh,
    tool: &dyn MillingCutter,
    params: &PencilParams,
) -> usize {
    let index = SpatialIndex::build(mesh, 5.0);
    let edge_map = build_edge_adjacency(mesh);
    let shared = compute_shared_edges(mesh, &edge_map);
    let threshold_rad = params.bitangency_angle.to_radians();
    let concave: Vec<SharedEdge> = shared
        .into_iter()
        .filter(|e| e.is_concave && e.dihedral_angle > (std::f64::consts::PI - threshold_rad))
        .collect();
    let chains_all = chain_concave_edges(&concave, mesh, params.min_cut_length);
    let resolved = resolve_reference_cutter(params, tool);
    let reference_ref = resolved.as_dyn();
    gate_chains_by_depth(
        chains_all,
        mesh,
        &index,
        tool,
        reference_ref,
        params.min_valley_depth,
    )
    .len()
}

fn default_pencil_params_1mm() -> PencilParams {
    PencilParams {
        hookup_distance: 3.0,
        offset_stepover: 0.25,
        feed_rate: 2000.0,
        plunge_rate: 132.0,
        ..Default::default()
    }
}

/// Tool-radius-aware detection acceptance. A 1mm tool on a gentle surface it
/// can fully reach yields ~no pencil chains (the reach-gap gate rejects the
/// reachable troughs); a genuine sharp valley it cannot bottom still yields a
/// chain. The valley uses a correctly-wound heightfield (`make_v_valley`) —
/// `make_v_groove`'s walls face downward, which `drop_cutter` rightly skips.
#[test]
fn test_pencil_tool_radius_aware_gentle_vs_valley() {
    let tool = BallEndmill::new(1.0, 25.0); // 1mm ball ≈ the detail tool's tip

    let gentle = make_gentle_undulating_surface(40.0, 80, 20.0, 0.3);
    let gentle_chains = pencil_chain_count(&gentle, &tool, &default_pencil_params_1mm());

    // Deep narrow valley (slope 2.0 → ~3mm rise per 1.5mm): a 1mm tool bridges it.
    let valley = make_v_valley(20.0, 4.0, 2.0, 20, 32);
    let valley_chains = pencil_chain_count(&valley, &tool, &default_pencil_params_1mm());

    assert!(
        valley_chains >= 1,
        "sharp V-valley must yield ≥1 pencil chain for a 1mm tool, got {valley_chains}"
    );
    assert!(
        gentle_chains <= 1,
        "gentle reachable surface should yield ~0 pencil chains for a 1mm tool \
         once the reach-gap gate is applied, got {gentle_chains}"
    );
}

/// Opt-in real-mesh validation against the organic relief that exploded:
///   RS_CAM_PENCIL_FIXTURE=/home/ricky/Downloads/wanaka200/rivmap_export/terrain.stl \
///   cargo test -p rs_cam_core --lib pencil_real_mesh_gate -- --ignored
/// Asserts the reach-gap gate reduces the candidate chain set and still traces
/// genuine valleys. Observed 2026-06-25 (terrain.stl, 661,212 tris, 1mm ball,
/// bitangency 160, gap 0.05mm): angle_chains=9277 → gated_chains=6994,
/// gated_moves=64,256 (vs 366k pre-gate), cutting=81m, rapid=77m. The gate is
/// correct but modest here (the surface is tool-unreachable almost everywhere);
/// the rapids are the Stage-2 hookup target.
#[test]
#[ignore = "needs RS_CAM_PENCIL_FIXTURE=/path/to/terrain.stl"]
fn pencil_real_mesh_gate() {
    let path = std::env::var("RS_CAM_PENCIL_FIXTURE").unwrap();
    let mesh = TriangleMesh::from_stl(std::path::Path::new(&path)).unwrap();
    let index = SpatialIndex::build(&mesh, 5.0);
    let tool = BallEndmill::new(1.0, 25.0);
    let params = PencilParams {
        hookup_distance: 3.0,
        offset_stepover: 0.25,
        feed_rate: 2000.0,
        plunge_rate: 132.0,
        safe_z: mesh.bbox.max.z + 5.0,
        ..Default::default()
    };

    let edge_map = build_edge_adjacency(&mesh);
    let shared = compute_shared_edges(&mesh, &edge_map);
    let thr = params.bitangency_angle.to_radians();
    let angle_owned: Vec<SharedEdge> = shared
        .into_iter()
        .filter(|e| e.is_concave && e.dihedral_angle > (std::f64::consts::PI - thr))
        .collect();
    let angle_chains = chain_concave_edges(&angle_owned, &mesh, params.min_cut_length).len();
    let gated_chains = pencil_chain_count(&mesh, &tool, &params);
    let tp = pencil_toolpath(&mesh, &index, &tool, &params);

    assert!(
        gated_chains <= angle_chains,
        "reach-gap gate must not increase chains: gated={gated_chains} angle={angle_chains}"
    );
    assert!(
        !tp.moves.is_empty(),
        "pencil must still trace genuine valleys after gating"
    );
}

/// Minimal top-down 2D line raster of a toolpath (feed moves only) — fast and
/// clear for seeing crease structure, unlike the 3D tube composite.
#[allow(clippy::indexing_slicing)] // bounded by moves.len()
fn rasterize_topdown(
    tp: &Toolpath,
    w: u32,
    h: u32,
    terrain_zmin: f64,
    terrain_zmax: f64,
) -> image::RgbaImage {
    use crate::toolpath::MoveType;
    let mut img = image::RgbaImage::from_pixel(w, h, image::Rgba([26, 26, 46, 255]));
    if tp.moves.len() < 2 {
        return img;
    }
    let (mut minx, mut miny, mut maxx, mut maxy) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for m in &tp.moves {
        minx = minx.min(m.target.x);
        maxx = maxx.max(m.target.x);
        miny = miny.min(m.target.y);
        maxy = maxy.max(m.target.y);
    }
    // Colour is absolute to the TERRAIN height: floor = blue, top = green.
    let minz = terrain_zmin;
    let zrange = (terrain_zmax - terrain_zmin).max(1e-6);
    let margin = 20.0;
    let dw = (maxx - minx).max(1e-6);
    let dh = (maxy - miny).max(1e-6);
    let scale = ((w as f64 - 2.0 * margin) / dw).min((h as f64 - 2.0 * margin) / dh);
    for i in 1..tp.moves.len() {
        let feed = matches!(
            tp.moves[i].move_type,
            MoveType::Linear { .. } | MoveType::ArcCW { .. } | MoveType::ArcCCW { .. }
        );
        if !feed {
            continue;
        }
        let a = tp.moves[i - 1].target;
        let b = tp.moves[i].target;
        // Colour by height: deep (low Z, lake floors / valley bottoms) = blue,
        // high (top rims / ridges) = bright green. Lets us tell base from top.
        let zt = (((a.z + b.z) * 0.5 - minz) / zrange).clamp(0.0, 1.0);
        let col = image::Rgba([
            (40.0 + zt * 80.0) as u8,
            (90.0 + zt * 150.0) as u8,
            (210.0 - zt * 110.0) as u8,
            255,
        ]);
        let x1 = margin + (a.x - minx) * scale;
        let y1 = h as f64 - margin - (a.y - miny) * scale;
        let x2 = margin + (b.x - minx) * scale;
        let y2 = h as f64 - margin - (b.y - miny) * scale;
        let steps = (x2 - x1).abs().max((y2 - y1).abs()).ceil().max(1.0) as i32;
        for s in 0..=steps {
            let t = s as f64 / steps as f64;
            let px = x1 + (x2 - x1) * t;
            let py = y1 + (y2 - y1) * t;
            if px >= 0.0 && py >= 0.0 {
                let (ux, uy) = (px as u32, py as u32);
                if ux < w && uy < h {
                    img.put_pixel(ux, uy, col);
                }
            }
        }
    }
    img
}

/// Fast headless visual loop (no GUI). Renders the pencil toolpath on a real
/// mesh to a top-down PNG (and a browser-openable SVG). Params come from env
/// vars so you can sweep WITHOUT recompiling — just re-run with different
/// values:
///   RS_CAM_PENCIL_FIXTURE=.../terrain.stl RS_CAM_PENCIL_OUT=/tmp/p.png \
///   RS_CAM_PENCIL_MVD=0.2 RS_CAM_PENCIL_BIT=160 \
///   cargo test -p rs_cam_core --lib render_pencil_real_mesh -- --ignored
#[test]
#[ignore = "needs RS_CAM_PENCIL_FIXTURE; writes PNG to RS_CAM_PENCIL_OUT"]
fn render_pencil_real_mesh() {
    let env_f64 = |k: &str, d: f64| {
        std::env::var(k)
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(d)
    };
    use std::time::Instant;
    let path = std::env::var("RS_CAM_PENCIL_FIXTURE").unwrap();
    let out = std::env::var("RS_CAM_PENCIL_OUT").unwrap_or_else(|_| "/tmp/pencil.png".into());
    let t = Instant::now();
    let mesh = TriangleMesh::from_stl(std::path::Path::new(&path)).unwrap();
    let t_load = t.elapsed().as_millis();
    let t = Instant::now();
    // Default to the real GUI path (build_auto); override cell for experiments.
    let cell = env_f64("RS_CAM_PENCIL_CELL", 0.0);
    let index = if cell > 0.0 {
        SpatialIndex::build(&mesh, cell)
    } else {
        SpatialIndex::build_auto(&mesh)
    };
    let t_index = t.elapsed().as_millis();
    let tool = BallEndmill::new(2.0, 25.0); // ~2mm-tip finish ball

    let mut params = default_pencil_params_1mm();
    params.bitangency_angle = env_f64("RS_CAM_PENCIL_BIT", 160.0);
    params.min_valley_depth = env_f64("RS_CAM_PENCIL_MVD", 0.05);
    params.min_cut_length = env_f64("RS_CAM_PENCIL_MINLEN", 2.0);
    params.hookup_distance = env_f64("RS_CAM_PENCIL_HOOKUP", params.hookup_distance);
    params.bisector_strength = env_f64("RS_CAM_PENCIL_BISECTOR", params.bisector_strength);
    params.reference_tool_diameter = env_f64("RS_CAM_PENCIL_REFD", params.reference_tool_diameter);
    // Detector selection + curvature tuning (RS_CAM_PENCIL_DETECTOR=curvature).
    params.detector = match std::env::var("RS_CAM_PENCIL_DETECTOR").ok().as_deref() {
        Some("curvature") => PencilDetector::Curvature,
        Some("rest_depth") => PencilDetector::RestDepth,
        _ => PencilDetector::Dihedral,
    };
    params.valley_saliency = env_f64("RS_CAM_PENCIL_SAL", params.valley_saliency);
    params.curvature_smoothing =
        env_f64("RS_CAM_PENCIL_SMOOTH", params.curvature_smoothing as f64) as usize;
    params.safe_z = mesh.bbox.max.z + 5.0;

    // Phase timings for the detection sub-steps (the suspected hot path).
    let t = Instant::now();
    let em = build_edge_adjacency(&mesh);
    let t_adj = t.elapsed().as_millis();
    let t = Instant::now();
    let _sh = compute_shared_edges(&mesh, &em);
    let t_shared = t.elapsed().as_millis();

    let t = Instant::now();
    let tp = pencil_toolpath(&mesh, &index, &tool, &params);
    let t_gen = t.elapsed().as_millis();
    let _ = std::fs::write(
        out.replace(".png", ".timing.txt"),
        format!(
            "load_ms={t_load} index_ms={t_index} edge_adj_ms={t_adj} shared_edges_ms={t_shared} full_generate_ms={t_gen} (gate+chain+lift≈{})\n",
            t_gen.saturating_sub(t_adj + t_shared)
        ),
    );
    let moves = tp.moves.len();
    let cutting = tp.total_cutting_distance();
    let rapid = tp.total_rapid_distance();
    let _ = std::fs::write(
        out.replace(".png", ".stats.txt"),
        format!("moves={moves} cutting_mm={cutting:.0} rapid_mm={rapid:.0}\n"),
    );
    // Top-down 2D PNG (agent-readable) + SVG (browser-openable, crisp lines).
    let (w, h) = (1600u32, 1600u32);
    rasterize_topdown(&tp, w, h, mesh.bbox.min.z, mesh.bbox.max.z)
        .save(&out)
        .unwrap();
    let svg = crate::export::viz::toolpath_to_svg(&tp, w as f64, h as f64);
    std::fs::write(out.replace(".png", ".svg"), svg).unwrap();
    assert!(
        moves > 0,
        "rendered {out} | mvd={} bit={} moves={moves} cutting_mm={cutting:.0}",
        params.min_valley_depth,
        params.bitangency_angle,
    );
}

/// Correctly-wound (CCW, +Z normals) V-valley heightfield: z = -slope·half_y at
/// the y=0 seam rising to 0 at the ±half_y edges. Unlike `make_v_groove`, the
/// wall facets face up, so a dropped cutter rests on them.
fn make_v_valley(len_x: f64, half_y: f64, slope: f64, nx: usize, ny: usize) -> TriangleMesh {
    let mut verts = Vec::new();
    let sx = len_x / nx as f64;
    let sy = 2.0 * half_y / ny as f64;
    for j in 0..=ny {
        for i in 0..=nx {
            let x = i as f64 * sx;
            let y = -half_y + j as f64 * sy;
            let z = -slope * half_y + slope * y.abs();
            verts.push(P3::new(x, y, z));
        }
    }
    let idx = |i: usize, j: usize| (j * (nx + 1) + i) as u32;
    let mut tris = Vec::new();
    for j in 0..ny {
        for i in 0..nx {
            tris.push([idx(i, j), idx(i + 1, j), idx(i + 1, j + 1)]);
            tris.push([idx(i, j), idx(i + 1, j + 1), idx(i, j + 1)]);
        }
    }
    TriangleMesh::from_raw(verts, tris)
}

/// Fairing straightens a facet-scale zig-zag, pins the endpoints, and keeps
/// the point count (so chains don't shrink at their tips).
#[test]
fn test_fair_polyline_straightens_and_pins_ends() {
    // Saw-tooth in Y along +X — the kind of jag a raw mesh-edge chain makes.
    let pts: Vec<P3> = (0..11)
        .map(|i| P3::new(i as f64, if i % 2 == 0 { 0.0 } else { 1.0 }, 0.0))
        .collect();
    let faired = fair_polyline_xy(&pts, FAIRING_PASSES, FAIRING_STRENGTH);

    assert_eq!(faired.len(), pts.len(), "fairing must preserve point count");

    let pf = pts.first().unwrap();
    let pl = pts.last().unwrap();
    let ff = faired.first().unwrap();
    let fl = faired.last().unwrap();
    assert!(
        (ff.x - pf.x).abs() < 1e-12 && (ff.y - pf.y).abs() < 1e-12,
        "first endpoint must be pinned"
    );
    assert!(
        (fl.x - pl.x).abs() < 1e-12 && (fl.y - pl.y).abs() < 1e-12,
        "last endpoint must be pinned"
    );

    // Peak interior Y excursion should shrink after fairing.
    let excursion = |v: &[P3]| {
        v.iter()
            .skip(1)
            .take(v.len().saturating_sub(2))
            .map(|p| p.y)
            .fold(0.0_f64, f64::max)
    };
    assert!(
        excursion(&faired) < excursion(&pts),
        "fairing should reduce zig-zag excursion ({} !< {})",
        excursion(&faired),
        excursion(&pts)
    );
}

/// Wiring `hookup_distance` joins nearby passes with a surface feed instead of
/// a retract-rapid-replunge, so the total rapid distance drops.
#[test]
fn test_hookup_linking_reduces_rapids() {
    let mesh = make_v_valley(30.0, 10.0, 0.5, 30, 40);
    let index = SpatialIndex::build(&mesh, 5.0);
    let tool = BallEndmill::new(6.0, 25.0);
    let mk = |hd: f64| PencilParams {
        bitangency_angle: 170.0,
        min_cut_length: 5.0,
        hookup_distance: hd,
        num_offset_passes: 2,
        offset_stepover: 1.5,
        sampling: 1.0,
        ..Default::default()
    };

    let unlinked = pencil_toolpath(&mesh, &index, &tool, &mk(0.0));
    let linked = pencil_toolpath(&mesh, &index, &tool, &mk(50.0));

    assert!(!linked.moves.is_empty(), "linked path must still cut");
    assert!(
        linked.total_rapid_distance() < unlinked.total_rapid_distance(),
        "hookup linking should reduce rapids: linked={:.1} unlinked={:.1}",
        linked.total_rapid_distance(),
        unlinked.total_rapid_distance()
    );
}

/// P1 W4a — shared geometry for the cost-decision tests below: two
/// independent 2-point runs 8mm apart on one continuous flat mesh (so
/// `build_surface_link` always succeeds geometrically — the surface vs.
/// retract choice is purely a cost decision, never a surface-contact
/// fallback). Both runs sit comfortably off the box's diagonal seam
/// (y=25 vs. the seam at y=x for every x in the runs' range).
fn cost_decision_fixture(feed_rate: f64) -> Toolpath {
    let mesh = make_convex_box(40.0);
    let index = SpatialIndex::build(&mesh, 10.0);
    let tool = BallEndmill::new(2.0, 25.0);

    let path_a = PencilPath {
        points: vec![P3::new(10.0, 25.0, 0.0), P3::new(12.0, 25.0, 0.0)],
        chain_index: 1,
        chain_total: 2,
        offset_index: 1,
        offset_total: 1,
        offset_mm: 0.0,
        is_centerline: true,
    };
    let path_b = PencilPath {
        points: vec![P3::new(20.0, 25.0, 0.0), P3::new(22.0, 25.0, 0.0)],
        chain_index: 2,
        chain_total: 2,
        offset_index: 1,
        offset_total: 1,
        offset_mm: 0.0,
        is_centerline: true,
    };

    // A modest, unremarkable machine — the point of both tests is the
    // FEED rate ratio, not exotic accel/kinematics behaviour.
    let kin = crate::machine::kinematics::MachineKinematics {
        acceleration_mm_s2: 300.0,
        ..crate::machine::kinematics::MachineKinematics::default()
    };
    let params = PencilParams {
        hookup_distance: 20.0,
        feed_rate,
        plunge_rate: 500.0,
        safe_z: 15.0,
        sampling: 1.0,
        link_kinematics: Some(crate::machine::kinematics::LinkKinematics {
            kinematics: kin,
            max_feed_mm_min: 6000.0,
            rapid_feed_mm_min: 5000.0,
        }),
        ..Default::default()
    };

    emit_paths(&[path_a, path_b], &mesh, &index, &tool, &params).0
}

/// A slow commanded feed makes the direct 8mm surface link expensive
/// (it cruises the whole gap at that feed) while the retract loop's
/// climb/travel/descend runs at fast rapids regardless — the F-034
/// costed decision must prefer retract even though the gap is well
/// within `hookup_distance`.
#[test]
fn cost_decision_prefers_retract_when_surface_link_is_slow() {
    use crate::toolpath::{MoveIntent, MoveType};

    let tp = cost_decision_fixture(100.0);
    let cut_indices: Vec<usize> = tp
        .moves
        .iter()
        .enumerate()
        .filter(|(_, m)| m.intent == MoveIntent::FinishingCut)
        .map(|(i, _)| i)
        .collect();
    let [cut_a, cut_b] = cut_indices.as_slice() else {
        panic!("expected exactly 2 FinishingCut moves (one per run), got {cut_indices:?}");
    };
    let between = tp.moves.get(*cut_a + 1..*cut_b).unwrap();
    assert!(
        between.iter().any(|m| m.move_type == MoveType::Rapid),
        "a slow surface feed should lose to the fast-rapid retract loop \
         — expected a rapid link between the two bodies, moves: {between:?}"
    );
}

/// A fast commanded feed makes the direct 8mm surface link cheap while
/// the retract loop still pays its fixed climb/travel/descend distance
/// regardless of rapid speed — the costed decision must prefer the
/// surface link, so no rapid appears between the two bodies.
#[test]
fn cost_decision_prefers_surface_link_when_cheap() {
    use crate::toolpath::{MoveIntent, MoveType};

    let tp = cost_decision_fixture(3000.0);
    let cut_indices: Vec<usize> = tp
        .moves
        .iter()
        .enumerate()
        .filter(|(_, m)| m.intent == MoveIntent::FinishingCut)
        .map(|(i, _)| i)
        .collect();
    let [cut_a, cut_b] = cut_indices.as_slice() else {
        panic!("expected exactly 2 FinishingCut moves (one per run), got {cut_indices:?}");
    };
    let between = tp.moves.get(*cut_a + 1..*cut_b).unwrap();
    assert!(
        !between.iter().any(|m| m.move_type == MoveType::Rapid),
        "a fast surface feed should beat the retract loop's fixed extra \
         distance — expected no rapid between the two bodies, moves: {between:?}"
    );
    assert!(
        between.iter().any(|m| m.intent == MoveIntent::Linking),
        "the winning surface link should emit Linking-intent feed moves"
    );
}

#[test]
fn test_hemisphere_pencil_produces_ring() {
    // Hemisphere on a flat base has a concave ring where it meets the base
    let mesh = make_test_hemisphere(20.0, 32);
    let index = SpatialIndex::build(&mesh, 5.0);
    let tool = BallEndmill::new(6.0, 25.0);

    let params = PencilParams {
        bitangency_angle: 170.0,
        min_cut_length: 3.0,
        hookup_distance: 20.0,
        offset_stepover: 1.5,
        sampling: 1.0,
        safe_z: 25.0,
        ..Default::default()
    };

    let edge_map = build_edge_adjacency(&mesh);
    let shared = compute_shared_edges(&mesh, &edge_map);

    // Hemisphere should have concave edges where the dome meets steeper regions
    let _concave_count = shared.iter().filter(|e| e.is_concave).count();
    // The hemisphere is all convex from outside, but some edges at base may be concave
    // depending on tessellation. At minimum, the algorithm should not crash.
    let _tp = pencil_toolpath(&mesh, &index, &tool, &params);
    // We just verify it runs without panic — hemisphere may or may not produce edges
    // depending on tessellation quality
    assert!(
        !shared.is_empty(),
        "hemisphere tessellation should produce shared edges"
    );
}

/// Two disjoint flat squares along X with a gap between them (both at
/// z=0) — used to force a genuinely isolated off-mesh point in the middle
/// of a lifted polyline, with the gap wide enough that the tool's contact
/// query radius (== cutter radius) never touches either square's material
/// from the gap's centre.
fn make_two_flat_squares(square_size: f64, gap: f64, half_width: f64) -> TriangleMesh {
    let vertices = vec![
        P3::new(0.0, -half_width, 0.0),
        P3::new(square_size, -half_width, 0.0),
        P3::new(square_size, half_width, 0.0),
        P3::new(0.0, half_width, 0.0),
        P3::new(square_size + gap, -half_width, 0.0),
        P3::new(2.0 * square_size + gap, -half_width, 0.0),
        P3::new(2.0 * square_size + gap, half_width, 0.0),
        P3::new(square_size + gap, half_width, 0.0),
    ];
    let triangles = vec![[0, 1, 2], [0, 2, 3], [4, 5, 6], [4, 6, 7]];
    TriangleMesh::from_raw(vertices, triangles)
}

/// Regression: `lift_to_surface` marks an off-mesh point
/// (bisector-shifted past the mesh edge) as non-contact via NaN-Z, and the
/// emit loop (`emit_paths`) must split the pass there instead of a single
/// cutting move bridging the gap with the stale un-lifted Z. Drives the
/// real production path: `paths_from_sampled` (which calls
/// `lift_to_surface`) feeding `emit_paths` (the extracted Step-7 logic).
#[test]
fn test_lift_to_surface_gap_splits_pass_not_stitches() {
    use crate::toolpath::MoveIntent;

    // Square A: x in [0,10]. Square B: x in [16,26]. Gap (10,16) has no
    // mesh at all, so a sample point near its centre (margin >= 3mm to
    // either edge, versus the 1mm-radius tool's query reach) is
    // unambiguously off-mesh.
    let mesh = make_two_flat_squares(10.0, 6.0, 5.0);
    let index = SpatialIndex::build(&mesh, 5.0);
    let tool = BallEndmill::new(2.0, 25.0);

    // Raw (pre-lift) polyline: a dummy sentinel Z far from the real
    // surface (0.0) so an accidentally-un-lifted point is obvious. x=13.0
    // sits in the gap's centre.
    let xs = [0.0, 2.0, 4.0, 6.0, 8.0, 13.0, 18.0, 20.0, 22.0, 24.0, 26.0];
    let raw: Vec<P3> = xs.iter().map(|&x| P3::new(x, 0.0, 999.0)).collect();

    let run_with = |stock_to_leave: f64| {
        let params = PencilParams {
            bitangency_angle: 170.0,
            min_cut_length: 1.0,
            hookup_distance: 0.0, // force retract/replunge — isolate the split from linking
            offset_stepover: 1.5,
            sampling: 1.0,
            stock_to_leave,
            ..Default::default()
        };

        let mut all_paths = Vec::new();
        paths_from_sampled(
            &raw,
            1,
            1,
            &mesh,
            &index,
            &tool,
            params.stock_to_leave,
            params.offset_stepover,
            OffsetFan::symmetric(params.num_offset_passes),
            &mut all_paths,
            &mut TipFloatFinding::default(),
        );
        assert_eq!(all_paths.len(), 1, "centerline only, no offset passes");

        emit_paths(&all_paths, &mesh, &index, &tool, &params)
    };

    let (tp0, _annotations0) = run_with(0.0);
    assert!(
        !tp0.moves.is_empty(),
        "must still emit the two on-mesh runs"
    );

    // (a) no emitted move carries a non-finite Z — the NaN marker must
    // never leak past the split into an actual toolpath move.
    for m in &tp0.moves {
        assert!(
            m.target.z.is_finite(),
            "emitted move must never carry a non-finite Z, got {:?}",
            m.target
        );
    }

    // (b) the mid-path off-mesh gap must force two independent plunge
    // entries (one per on-mesh run) instead of one cutting move bridging
    // the gap with the previous pass's stale interpolated Z.
    let plunge_count = tp0
        .moves
        .iter()
        .filter(|m| m.intent == MoveIntent::EntryPlunge)
        .count();
    assert_eq!(
        plunge_count, 2,
        "the mid-path off-mesh gap must split into two plunge entries, got {plunge_count}"
    );

    // No single cutting move should span anywhere near the ~6mm gap —
    // that would mean the two runs got stitched together instead of split.
    let mut prev: Option<P3> = None;
    for m in &tp0.moves {
        if m.intent == MoveIntent::FinishingCut
            && let Some(p) = prev
        {
            let dxy = ((m.target.x - p.x).powi(2) + (m.target.y - p.y).powi(2)).sqrt();
            assert!(
                dxy < 4.0,
                "a cutting move must not bridge the off-mesh gap: jumped {dxy:.2}mm"
            );
        }
        prev = Some(m.target);
    }

    // (c) stock_to_leave must be applied to every contacted (on-mesh) cut
    // Z — compare with/without.
    let (tp_leave, _annotations_leave) = run_with(0.5);
    let cut_zs = |tp: &Toolpath| -> Vec<f64> {
        tp.moves
            .iter()
            .filter(|m| matches!(m.intent, MoveIntent::FinishingCut | MoveIntent::EntryPlunge))
            .map(|m| m.target.z)
            .collect()
    };
    let zs0 = cut_zs(&tp0);
    let zs_leave = cut_zs(&tp_leave);
    assert_eq!(
        zs0.len(),
        zs_leave.len(),
        "stock_to_leave must not change which points are emitted as cuts"
    );
    for (z0, zl) in zs0.iter().zip(zs_leave.iter()) {
        assert!(
            (zl - z0 - 0.5).abs() < 1e-6,
            "stock_to_leave must shift every contacted cut Z by exactly 0.5mm: {z0} vs {zl}"
        );
    }
}

// ── G-LINKLOAD: the link-lift trigger ───────────────────────────────
//
// The end-to-end sentries live in
// `tests/pencil_surface_link_g_linkload.rs`. These two isolate the one
// decision those cannot separate cheaply: WHEN the lift fires. The
// discrimination they pin is the whole reason the flank test carries the
// tool's own envelope height — without it, a link riding a 45 degree
// crease reads its own valley wall as an obstruction and every junction
// in a groove buys a clearance hop that clears nothing.

/// Stock whose top follows a symmetric 45 degree V running along Y —
/// `top(x) = floor + |x|` — with an optional rib: a band of `y` left
/// standing at `rib_top` right across the valley.
fn v_valley_stock(floor: f64, rib: Option<(f64, f64, f64)>) -> crate::dexel_stock::TriDexelStock {
    let mut stock =
        crate::dexel_stock::TriDexelStock::from_stock(-5.0, -5.0, 5.0, 5.0, -6.0, 2.0, 0.25);
    let (rows, cols) = (stock.z_grid.rows, stock.z_grid.cols);
    let (cs, ou, ov) = (
        stock.z_grid.cell_size,
        stock.z_grid.origin_u,
        stock.z_grid.origin_v,
    );
    for row in 0..rows {
        let y = ov + row as f64 * cs;
        for col in 0..cols {
            let x = ou + col as f64 * cs;
            let top = match rib {
                Some((y0, y1, rib_top)) if y >= y0 && y <= y1 => rib_top,
                _ => floor + x.abs(),
            };
            stock.clear_above_at(row, col, top as f32);
        }
    }
    stock
}

/// The link path a ball rests on down the middle of that V: a straight run
/// along Y at x = 0, tip at the closed-form rest height
/// `floor + r(1/cos 45 - 1)`.
fn v_valley_link(floor: f64, r: f64, y0: f64, y1: f64) -> (P3, Vec<P3>, P3) {
    let z = floor + r * (std::f64::consts::SQRT_2 - 1.0);
    let from = P3::new(0.0, y0, z);
    let to = P3::new(0.0, y1, z);
    let n = ((y1 - y0) / 0.5).round().max(1.0) as usize;
    let pts = (1..n)
        .map(|k| P3::new(0.0, y0 + (y1 - y0) * k as f64 / n as f64, z))
        .collect();
    (from, pts, to)
}

fn link_params() -> PencilParams {
    PencilParams {
        safe_z: 5.0,
        sampling: 0.5,
        ..PencilParams::default()
    }
}

// ── G-LINKSTAGE: the pencil's own link counters ─────────────────────

/// Two runs on a flat surface, a stated XY gap apart, emitted through the
/// real emitter so the report reads the shipped decision path.
fn two_run_link_report(gap_mm: f64, hop_cap: Option<f64>) -> PencilLinkReport {
    two_run_link_report_over(gap_mm, hop_cap, None)
}

/// Flat stock whose top is `z = 0` — the mesh itself — except a rib of
/// `x` left standing at `rib_top` right across the run-to-run gap. A
/// surface link between the two runs then has material above it, which
/// is the only condition that reaches `plan_link_lift`.
fn flat_stock_with_rib(x0: f64, x1: f64, rib_top: f64) -> crate::dexel_stock::TriDexelStock {
    let mut stock =
        crate::dexel_stock::TriDexelStock::from_stock(0.0, 0.0, 40.0, 40.0, -6.0, 2.0, 0.25);
    let (rows, cols) = (stock.z_grid.rows, stock.z_grid.cols);
    let (cs, ou) = (stock.z_grid.cell_size, stock.z_grid.origin_u);
    for row in 0..rows {
        for col in 0..cols {
            let x = ou + col as f64 * cs;
            let top = if x >= x0 && x <= x1 { rib_top } else { 0.0 };
            stock.clear_above_at(row, col, top as f32);
        }
    }
    stock
}

/// [`two_run_link_report`] with a stock reading, so the lifted tier can
/// fire. `None` cannot reach `plan_link_lift` at all.
fn two_run_link_report_over(
    gap_mm: f64,
    hop_cap: Option<f64>,
    stock: Option<&crate::dexel_stock::TriDexelStock>,
) -> PencilLinkReport {
    let mesh = make_convex_box(40.0);
    let index = SpatialIndex::build(&mesh, 10.0);
    let tool = BallEndmill::new(2.0, 25.0);
    let mk = |x0: f64, i: usize| PencilPath {
        points: vec![P3::new(x0, 25.0, 0.0), P3::new(x0 + 2.0, 25.0, 0.0)],
        chain_index: i,
        chain_total: 2,
        offset_index: 1,
        offset_total: 1,
        offset_mm: 0.0,
        is_centerline: true,
    };
    let params = PencilParams {
        hookup_distance: 10.0,
        link_hop_distance_mm: hop_cap,
        feed_rate: 1000.0,
        plunge_rate: 500.0,
        safe_z: 15.0,
        sampling: 1.0,
        ..Default::default()
    };
    let (_, _, report) = emit_paths_with_entry_stock_reported(
        &[mk(10.0, 1), mk(12.0 + gap_mm, 2)],
        &mesh,
        &index,
        &tool,
        &params,
        stock,
    );
    report
}

/// The instrument the SPEC's step 1 asks for: one regen names the binding
/// constraint. Before it, the pencil emitter published nothing about its
/// refusals, so a pass that is 84 % entry motion could not say which of
/// the four gates produced it.
#[test]
fn the_link_report_separates_a_reached_junction_from_a_too_far_one() {
    let near = two_run_link_report(4.0, None);
    assert_eq!(near.junctions, 1, "{near:?}");
    assert_eq!(
        near.linked_at_depth, 1,
        "a 4 mm gap inside a 10 mm hookup on flat ground links AT DEPTH — \
         the only tier that removes the next run's entry: {near:?}"
    );
    assert_eq!(near.linked_via_hop, 0, "{near:?}");
    assert_eq!(near.too_far, 0, "{near:?}");

    let far = two_run_link_report(20.0, None);
    assert_eq!(far.junctions, 1, "{far:?}");
    assert_eq!(
        far.too_far, 1,
        "a 20 mm gap is beyond the 10 mm hookup, and the report must SAY \
         that rather than only showing a missing link: {far:?}"
    );
    assert_eq!(far.linked_at_depth, 0, "{far:?}");
}

/// The hop cap governs the LIFTED tier only, so on a junction that never
/// needed a lift it changes nothing. `entry_stock: None` cannot reach
/// `plan_link_lift` at all, which is what makes this a controlled A/B on
/// the dial itself.
#[test]
fn the_hop_cap_does_not_touch_an_at_depth_link() {
    let default_cap = two_run_link_report(4.0, None);
    let tiny_cap = two_run_link_report(4.0, Some(0.5));
    assert_eq!(
        default_cap, tiny_cap,
        "an at-depth link is not a hop; shrinking the hop cap to 0.5 mm \
         must not refuse a 4 mm at-depth link"
    );
    assert_eq!(tiny_cap.hop_too_far, 0, "{tiny_cap:?}");
}

/// G-PENCILHOP: the dial's whole purpose. On a junction that DOES need a
/// lift, `Some(0.0)` refuses the hop and says so on `hop_too_far`; the
/// at-depth tier is untouched, which is what makes the pair a control.
///
/// The operator dial that reaches this is
/// [`crate::compute::operation_configs::PencilConfig::link_hop_distance_mm`]
/// (`tests/pencil_hop_dial_g_pencilhop.rs` pins the wiring). Before it,
/// `execute.rs` hardcoded `None` and no project file, GUI or MCP call
/// could take this measurement.
#[test]
fn a_zero_hop_cap_refuses_a_lifted_link_and_keeps_the_at_depth_tier() {
    // The rib stands right across the 4 mm gap between the two runs.
    let stock = flat_stock_with_rib(13.0, 15.0, 2.0);
    let lifted = two_run_link_report_over(4.0, None, Some(&stock));

    // Population before verdict: with no hop taken every assertion
    // below is vacuous, so the fixture must fail here instead.
    assert_eq!(
        lifted.linked_via_hop, 1,
        "population: the rib must force the link to lift, or this pair              cannot see what the hop cap does: {lifted:?}"
    );

    let refused = two_run_link_report_over(4.0, Some(0.0), Some(&stock));
    assert_eq!(
        refused.linked_via_hop, 0,
        "a zero hop cap must refuse every hop: {refused:?}"
    );
    assert_eq!(
        refused.hop_too_far, 1,
        "the refused hop must be ATTRIBUTED, not merely missing — that              counter is what names the pencil's binding constraint:              {refused:?}"
    );
    assert_eq!(
        refused.linked_at_depth, lifted.linked_at_depth,
        "the hop cap must not touch the at-depth tier: {lifted:?} vs              {refused:?}"
    );
}

/// A link riding the bottom of a valley is NOT ploughing a ridge, even
/// though the walls half a tip-radius away stand well above the tip. The
/// tool is supposed to be touching them.
#[test]
fn link_lift_does_not_fire_on_a_link_riding_its_own_crease() {
    let tool = BallEndmill::new(1.0, 25.0);
    let r = tool.cusp_radius_mm();
    let rise = tool.height_at_radius(r).unwrap_or(0.0);
    let stock = v_valley_stock(-1.5, None);
    let (from, pts, to) = v_valley_link(-1.5, r, -3.0, 3.0);

    assert!(
        matches!(
            plan_link_lift(from, to, &pts, &stock, &tool, r, rise, &link_params()),
            LinkLift::NotNeeded
        ),
        "the valley wall under the flank is the surface this pass is \
         tracing, not standing material — lifting for it would cost every \
         junction in a groove a clearance hop that clears nothing"
    );
}

/// The same link with a rib left standing across it must lift, and every
/// lifted sample must clear the rib by the stated clearance.
#[test]
fn link_lift_fires_on_a_rib_standing_across_the_link() {
    let tool = BallEndmill::new(1.0, 25.0);
    let r = tool.cusp_radius_mm();
    let rise = tool.height_at_radius(r).unwrap_or(0.0);
    let rib_top = 1.0;
    let stock = v_valley_stock(-1.5, Some((0.5, 1.5, rib_top)));
    let (from, pts, to) = v_valley_link(-1.5, r, -3.0, 3.0);

    let lifted = match plan_link_lift(from, to, &pts, &stock, &tool, r, rise, &link_params()) {
        LinkLift::Lifted(v) => v,
        LinkLift::NotNeeded => panic!("a rib standing 2.3mm over the link was ridden through"),
        LinkLift::Refused => panic!("clearing a rib at z=1.0 does not reach safe_z=5.0"),
    };
    assert_eq!(
        lifted.len(),
        pts.len() + 2,
        "the lift brackets the interior samples with both endpoints, so the \
         transit leaves and re-enters vertically"
    );
    // The clearance contract is PROFILE-AWARE: a sample only has to clear
    // what its cutter can actually reach at each lateral offset. On a ball
    // that is strictly less than the flat-disc max wherever the obstacle
    // sits off-axis — and exactly equal wherever the tip is over it, which
    // is the case that matters here.
    for p in &lifted {
        let need = stock
            .max_clearance_tip_z_for_profile(p.x, p.y, r, &tool)
            .unwrap_or(2.0)
            + crate::toolpath::PLUNGE_CLEARANCE_MM;
        assert!(
            p.z >= need - 1e-9,
            "lifted sample at y={:.3} sits at z={:.3}, under its {need:.3} \
             clearance",
            p.y,
            p.z
        );
    }
    // Non-vacuity, and the half the profile rule must NOT relax: every
    // sample whose tip passes over the rib band clears the rib itself by
    // the full clearance, exactly as before this became profile-aware.
    let over_rib: Vec<&P3> = lifted.iter().filter(|p| p.y >= 0.5 && p.y <= 1.5).collect();
    assert!(
        !over_rib.is_empty(),
        "the sampled link must actually pass over the rib band"
    );
    for p in over_rib {
        assert!(
            p.z >= rib_top + crate::toolpath::PLUNGE_CLEARANCE_MM - 1e-9,
            "a sample with its TIP over the rib sits at z={:.3}, under the \
             rib top {rib_top:.3} plus clearance",
            p.z
        );
    }
}

/// The refusal arm: a rib so tall that clearing it reaches the retract
/// plane leaves nothing for a fed link to save.
#[test]
fn link_lift_refuses_when_the_clearance_reaches_safe_z() {
    let tool = BallEndmill::new(1.0, 25.0);
    let r = tool.cusp_radius_mm();
    let rise = tool.height_at_radius(r).unwrap_or(0.0);
    let stock = v_valley_stock(-1.5, Some((0.5, 1.5, 2.0)));
    let (from, pts, to) = v_valley_link(-1.5, r, -3.0, 3.0);
    let params = PencilParams {
        // 2.0 (rib) + 2.0 (clearance) = 4.0, at the retract plane.
        safe_z: 4.0,
        ..link_params()
    };
    assert!(
        matches!(
            plan_link_lift(from, to, &pts, &stock, &tool, r, rise, &params),
            LinkLift::Refused
        ),
        "a fed link at retract height is strictly worse than the rapid it \
         would replace"
    );
}

/// FIN-09 sentry: the detector is a typed dial, so an unknown token refuses
/// the load and names the key.
///
/// Before FIN-09 the field was a `String` that `PencilDetector::parse`
/// mapped with a `_ => Dihedral` arm. A project file that said `rest-depth`
/// ran the dihedral detector and reported nothing — the operator got a
/// pencil pass that was silently the wrong strategy. The three canonical
/// tokens still load.
#[test]
// SAFETY: a test that asserts on a serde refusal; a panic IS the failure
// report. The file already allows `unwrap_used` and `panic` for the same
// reason.
#[allow(clippy::expect_used)]
fn an_unknown_detector_token_refuses_the_load() {
    use crate::compute::operation_configs::PencilConfig;

    let base = toml::to_string(&PencilConfig::default()).expect("the default config serialises");
    assert!(
        base.contains("detector = \"dihedral\""),
        "the canonical token is the snake-case variant name: {base}"
    );
    let with_detector = |token: &str| {
        base.replace(
            "detector = \"dihedral\"",
            &format!("detector = \"{token}\""),
        )
    };

    for (token, want) in [
        ("dihedral", PencilDetector::Dihedral),
        ("curvature", PencilDetector::Curvature),
        ("rest_depth", PencilDetector::RestDepth),
    ] {
        let cfg: PencilConfig = toml::from_str(&with_detector(token))
            .unwrap_or_else(|e| panic!("`{token}` must load: {e}"));
        assert_eq!(cfg.detector, want, "`{token}` must load as {want:?}");
    }

    // The four aliases `parse` used to accept, plus a typo.
    for token in ["rest-depth", "restdepth", "rest", "crest", "ridgevalley"] {
        let err = toml::from_str::<PencilConfig>(&with_detector(token))
            .expect_err("an unknown detector token must refuse the load");
        let text = err.to_string();
        assert!(
            text.contains("detector"),
            "the refusal must name the key: {text}"
        );
    }
}
