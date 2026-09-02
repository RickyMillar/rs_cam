//! S1 — pure scallop vs the unified band mix, as shipped
//! (`planning/metrology_2026-09-02/FINDINGS.md` §M7, pre-registered).
//!
//! The operator's question: the unified op pays band machinery — three
//! strategies, 2 mm inter-band overlap, seams. Do the bands earn it, or
//! does one strategy win?
//!
//! Arms (same mesh, R1.5 tool, feeds, kinematics, 0.03 mm cusp, 0.05
//! tolerance):
//!
//! * **U** — the production unified op, mt2 R1.5 params verbatim.
//! * **C** — the shipped standalone scallop, slope 0–90°, continuous,
//!   `intra_pass_hookup_mm = 3.0` with kinematics (operation defaults).
//! * **R** — the planner's OWN regions (all three bands), extracted with
//!   `overlap_mm = one stepover` instead of 2.0, every region cut by the
//!   scallop generator (operator-added arm).
//!
//! Costed as generated (`compute_cycle_time`, project kinematics) — no
//! relink; this compares the ops as shipped. Coverage: lifted-centroid
//! audit over ALL mesh triangles, differential deciding (knife-edge
//! caveat, M4). Bar C-time: an arm wins if its time beats arm U with
//! coverage equality (±0.5 pp).
//!
//! NUMBERS ONLY — the ruling goes to the FINDINGS file.
//!
//! `#[ignore]` — needs the operator's wanaka mesh. SKIPs when absent.

#![allow(
    clippy::print_stderr,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::too_many_lines
)]

use std::fmt::Write as _;
use std::path::Path;

use rayon::prelude::*;
use rs_cam_core::classify_probe::ClassificationSampler;
use rs_cam_core::finish_planner::{FinishPlannerParams, decompose};
use rs_cam_core::finish_setup::build_classification_surface_with_sampler_and_cancel;
use rs_cam_core::geo::P3;
use rs_cam_core::machine_kinematics::{LinkKinematics, MachineKinematics, compute_cycle_time};
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::region_set::RegionSet;
use rs_cam_core::scallop::{
    ScallopDirection, ScallopParams, scallop_toolpath_structured_annotated_with_cancel,
};
use rs_cam_core::tool::{MillingCutter, TaperedBallEndmill};
use rs_cam_core::toolpath::{MoveIntent, MoveType, Toolpath};
use rs_cam_core::unified_finish::{
    UnifiedFinishParams, unified_finish_classification_resolution,
    unified_finish_toolpath_with_cancel,
};

const WANAKA_MESH: &str = "/home/ricky/Downloads/wanaka200/rivmap_export/terrain.stl";

// mt2 R1.5 unified op params, verbatim (the E3 constants).
const SCALLOP_HEIGHT: f64 = 0.03;
const TOLERANCE: f64 = 0.05;
const RASTER_STEPOVER: f64 = 0.596_992_462_263_972;
const Z_STEP: f64 = 0.596_992_462_263_972;
const SAMPLING: f64 = 0.5;
const FEED: f64 = 913.0;
const PLUNGE: f64 = 270.0;
const OVERLAP_MM: f64 = 2.0;
const HOOKUP_MM: f64 = 25.0;
/// The shipped scallop OPERATION's hookup default (`ScallopConfig`).
const SCALLOP_OP_HOOKUP_MM: f64 = 3.0;

const MACHINE_ACCEL_XYZ: [f64; 3] = [500.0, 500.0, 270.0];
const MACHINE_ACCEL_SCALAR: f64 = 423.333_333_333_333_3;
const JUNCTION_DEVIATION_MM: f64 = 0.02;
const MAX_FEED_MM_MIN: f64 = 10_000.0;
const RAPID_FEED_MM_MIN: f64 = 5_000.0;

const COVERAGE_EQUALITY_MARGIN_PP: f64 = 0.5;

// ── coverage audit (restated, compact form of the F2 helpers) ────────────

struct BoardTriangles {
    lifted: Vec<P3>,
    areas: Vec<f64>,
    area_mm2: f64,
}

fn board_triangles(mesh: &TriangleMesh, scallop_h_mm: f64) -> BoardTriangles {
    let rows: Vec<(P3, f64)> = (0..mesh.triangles.len())
        .into_par_iter()
        .filter_map(|t| {
            let tri = mesh.triangles[t];
            let p0 = mesh.vertices[tri[0] as usize];
            let p1 = mesh.vertices[tri[1] as usize];
            let p2 = mesh.vertices[tri[2] as usize];
            let cross = (p1 - p0).cross(&(p2 - p0));
            let area = 0.5 * cross.norm();
            if area <= 0.0 || !area.is_finite() {
                return None;
            }
            let n = cross / cross.norm();
            let n = if n.z < 0.0 { -n } else { n };
            let c = P3::new(
                (p0.x + p1.x + p2.x) / 3.0,
                (p0.y + p1.y + p2.y) / 3.0,
                (p0.z + p1.z + p2.z) / 3.0,
            );
            Some((c + n * scallop_h_mm, area))
        })
        .collect();
    let mut lifted = Vec::with_capacity(rows.len());
    let mut areas = Vec::with_capacity(rows.len());
    let mut area_mm2 = 0.0;
    for (p, a) in rows {
        lifted.push(p);
        areas.push(a);
        area_mm2 += a;
    }
    BoardTriangles {
        lifted,
        areas,
        area_mm2,
    }
}

struct SegmentIndex {
    ox: f64,
    oy: f64,
    cell: f64,
    nx: usize,
    ny: usize,
    buckets: Vec<Vec<u32>>,
    segments: Vec<(P3, P3)>,
}

impl SegmentIndex {
    fn build(toolpath: &Toolpath, lift: f64, cell: f64, bbox: [f64; 4]) -> Self {
        let [x0, y0, x1, y1] = bbox;
        let nx = (((x1 - x0) / cell).ceil() as usize).max(1) + 2;
        let ny = (((y1 - y0) / cell).ceil() as usize).max(1) + 2;
        let mut me = Self {
            ox: x0 - cell,
            oy: y0 - cell,
            cell,
            nx,
            ny,
            buckets: vec![Vec::new(); nx * ny],
            segments: Vec::new(),
        };
        for i in 1..toolpath.moves.len() {
            let m = &toolpath.moves[i];
            if matches!(m.move_type, MoveType::Rapid) {
                continue;
            }
            if !matches!(m.intent, MoveIntent::ClearingCut | MoveIntent::FinishingCut) {
                continue;
            }
            let a = toolpath.moves[i - 1].target;
            let b = m.target;
            let seg = (P3::new(a.x, a.y, a.z + lift), P3::new(b.x, b.y, b.z + lift));
            let id = me.segments.len() as u32;
            me.segments.push(seg);
            let (sx0, sx1) = (a.x.min(b.x), a.x.max(b.x));
            let (sy0, sy1) = (a.y.min(b.y), a.y.max(b.y));
            let c0 = (((sx0 - me.ox) / cell).floor().max(0.0) as usize).min(nx - 1);
            let c1 = (((sx1 - me.ox) / cell).ceil() as usize).min(nx - 1);
            let r0 = (((sy0 - me.oy) / cell).floor().max(0.0) as usize).min(ny - 1);
            let r1 = (((sy1 - me.oy) / cell).ceil() as usize).min(ny - 1);
            for r in r0..=r1 {
                for c in c0..=c1 {
                    me.buckets[r * nx + c].push(id);
                }
            }
        }
        me
    }

    fn nearest_sq(&self, p: P3) -> f64 {
        let c = ((p.x - self.ox) / self.cell).floor();
        let r = ((p.y - self.oy) / self.cell).floor();
        let mut best = f64::INFINITY;
        for dr in -1isize..=1 {
            for dc in -1isize..=1 {
                let rr = r as isize + dr;
                let cc = c as isize + dc;
                if rr < 0 || cc < 0 || rr >= self.ny as isize || cc >= self.nx as isize {
                    continue;
                }
                for &id in &self.buckets[rr as usize * self.nx + cc as usize] {
                    let (a, b) = self.segments[id as usize];
                    let ab = b - a;
                    let denom = ab.norm_squared();
                    let t = if denom <= 1e-18 {
                        0.0
                    } else {
                        ((p - a).dot(&ab) / denom).clamp(0.0, 1.0)
                    };
                    best = best.min((p - (a + ab * t)).norm_squared());
                }
            }
        }
        best
    }
}

fn coverage_pct(
    tris: &BoardTriangles,
    toolpath: &Toolpath,
    cusp_radius: f64,
    bbox: [f64; 4],
) -> f64 {
    let index = SegmentIndex::build(toolpath, cusp_radius, 4.0 * cusp_radius, bbox);
    let r2 = cusp_radius * cusp_radius;
    let unmachined: f64 = (0..tris.lifted.len())
        .into_par_iter()
        .filter_map(|i| (index.nearest_sq(tris.lifted[i]) > r2).then_some(tris.areas[i]))
        .sum();
    100.0 * unmachined / tris.area_mm2.max(1e-9)
}

// ── arm accounting ───────────────────────────────────────────────────────

struct ArmStats {
    time_s: f64,
    cutting_mm: f64,
    rapid_mm: f64,
    entry_plunges: usize,
    moves: usize,
}

fn arm_stats(tp: &Toolpath, kin: &MachineKinematics) -> ArmStats {
    let mut rapid_mm = 0.0;
    for i in 1..tp.moves.len() {
        if matches!(tp.moves[i].move_type, MoveType::Rapid) {
            let a = tp.moves[i - 1].target;
            let b = tp.moves[i].target;
            rapid_mm += (b - a).norm();
        }
    }
    ArmStats {
        time_s: compute_cycle_time(tp, kin, MAX_FEED_MM_MIN, RAPID_FEED_MM_MIN),
        cutting_mm: tp.total_cutting_distance(),
        rapid_mm,
        entry_plunges: tp
            .moves
            .iter()
            .filter(|m| m.intent == MoveIntent::EntryPlunge)
            .count(),
        moves: tp.moves.len(),
    }
}

/// Slope-coloured SVG dump (the E3 convention).
fn dump_svg(
    path: &std::path::Path,
    toolpath: &Toolpath,
    theta_at: &dyn Fn(f64, f64) -> f64,
    window: [f64; 4],
    stroke: f64,
) {
    let [x0, y0, x1, y1] = window;
    let (w, h) = (x1 - x0, y1 - y0);
    let ph = 1000.0 * h / w.max(1e-9);
    let mut per_color: std::collections::HashMap<&'static str, String> =
        std::collections::HashMap::new();
    for i in 1..toolpath.moves.len() {
        let m = &toolpath.moves[i];
        if matches!(m.move_type, MoveType::Rapid) {
            continue;
        }
        if !matches!(m.intent, MoveIntent::ClearingCut | MoveIntent::FinishingCut) {
            continue;
        }
        let a = toolpath.moves[i - 1].target;
        let b = m.target;
        let inside = |px: f64, py: f64| px >= x0 && px <= x1 && py >= y0 && py <= y1;
        if !(inside(a.x, a.y) && inside(b.x, b.y)) {
            continue;
        }
        let t = theta_at((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
        let color = if t >= 75.0 {
            "#ff5252"
        } else if t >= 45.0 {
            "#ffb347"
        } else {
            "#7fd4ff"
        };
        let d = per_color.entry(color).or_default();
        let _ = write!(
            d,
            "M{:.2},{:.2}L{:.2},{:.2}",
            a.x - x0,
            y1 - a.y,
            b.x - x0,
            y1 - b.y
        );
    }
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {w:.1} {h:.1}\" \
         width=\"1000\" height=\"{ph:.0}\">\n\
         <rect width=\"100%\" height=\"100%\" fill=\"#101418\"/>\n"
    );
    for (color, d) in &per_color {
        let _ = writeln!(
            svg,
            "<path fill=\"none\" stroke=\"{color}\" stroke-width=\"{stroke}\" d=\"{d}\"/>"
        );
    }
    svg.push_str("</svg>\n");
    let _ = std::fs::write(path, svg);
}

#[test]
#[ignore = "evidence run — needs the operator's wanaka mesh (not in repo)"]
fn wanaka_scallop_solo_vs_unified_s1() {
    eprintln!(
        "\n========== S1 — PURE SCALLOP vs THE UNIFIED BAND MIX ==========\n\
         Pre-registration: planning/metrology_2026-09-02/FINDINGS.md §M7\n\
         (bar C-time + coverage equality fixed before this run). NUMBERS ONLY.\n"
    );
    let mesh_path = Path::new(WANAKA_MESH);
    if !mesh_path.exists() {
        eprintln!("SKIP: {WANAKA_MESH} not present on this machine.");
        return;
    }
    let started = std::time::Instant::now();
    let mesh = TriangleMesh::from_stl_scaled(mesh_path, 1.0).expect("load wanaka terrain");
    let index = SpatialIndex::build_auto(&mesh);
    let r15 = TaperedBallEndmill::new(3.0, 2.8, 6.0, 30.5);
    let never_cancel = || false;
    let safe_z = mesh.bbox.max.z + 5.0;
    let kinematics = MachineKinematics {
        acceleration_mm_s2: MACHINE_ACCEL_SCALAR,
        acceleration_xyz_mm_s2: Some(MACHINE_ACCEL_XYZ),
        junction_deviation_mm: JUNCTION_DEVIATION_MM,
        ..MachineKinematics::default()
    };
    let link_kinematics = LinkKinematics {
        kinematics,
        max_feed_mm_min: MAX_FEED_MM_MIN,
        rapid_feed_mm_min: RAPID_FEED_MM_MIN,
    };

    // ── arm U: the production unified op ─────────────────────────────────
    let mut uparams = UnifiedFinishParams {
        scallop_height: SCALLOP_HEIGHT,
        tolerance: TOLERANCE,
        raster_stepover: RASTER_STEPOVER,
        z_step: Z_STEP,
        sampling: SAMPLING,
        feed_rate: FEED,
        plunge_rate: PLUNGE,
        safe_z,
        intra_region_hookup_mm: HOOKUP_MM,
        ..UnifiedFinishParams::default()
    };
    uparams.classification_sampler = ClassificationSampler::PRODUCTION;
    let mut planner = FinishPlannerParams::for_tool(r15.cusp_radius_mm());
    planner.overlap_mm = OVERLAP_MM;
    let t0 = std::time::Instant::now();
    let (tp_u, _, _) = unified_finish_toolpath_with_cancel(
        &mesh,
        &index,
        &r15,
        mesh.bbox.max.z,
        mesh.bbox.min.z - 0.5,
        &uparams,
        &planner,
        None,
        Some(&link_kinematics),
        None,
        None,
        &never_cancel,
    )
    .expect("unified finish");
    eprintln!(
        "arm U generated: {} moves, {:.0} s",
        tp_u.moves.len(),
        t0.elapsed().as_secs_f64()
    );

    // ── arm C: the shipped standalone scallop, whole board ───────────────
    let sparams = ScallopParams {
        scallop_height: SCALLOP_HEIGHT,
        tolerance: TOLERANCE,
        direction: ScallopDirection::OutsideIn,
        continuous: true,
        slope_from: 0.0,
        slope_to: 90.0,
        feed_rate: FEED,
        plunge_rate: PLUNGE,
        safe_z,
        stock_to_leave: 0.0,
        intra_pass_hookup_mm: SCALLOP_OP_HOOKUP_MM,
        link_kinematics: Some(link_kinematics),
    };
    let t1 = std::time::Instant::now();
    let (tp_c, _, _) = scallop_toolpath_structured_annotated_with_cancel(
        &mesh,
        &index,
        &r15,
        &sparams,
        None,
        None,
        &never_cancel,
    )
    .expect("scallop whole board");
    eprintln!(
        "arm C generated: {} moves, {:.0} s",
        tp_c.moves.len(),
        t1.elapsed().as_secs_f64()
    );

    // ── arm R: the planner's regions, all scalloped, 1-stepover overlap ──
    let surface = build_classification_surface_with_sampler_and_cancel(
        &mesh,
        &index,
        &r15,
        unified_finish_classification_resolution(&r15, TOLERANCE),
        ClassificationSampler::PRODUCTION,
        &never_cancel,
    )
    .expect("classification surface");
    let mut planner_r = FinishPlannerParams::for_tool(r15.cusp_radius_mm());
    planner_r.overlap_mm = RASTER_STEPOVER;
    let planned = decompose(
        &surface.slope_map,
        surface.heightmap.covered_flags(),
        &[],
        &planner_r,
    );
    let region_polys: Vec<rs_cam_core::polygon::Polygon2> =
        planned.regions.iter().map(|r| r.polygon.clone()).collect();
    let n_regions = region_polys.len();
    let regions = RegionSet::new(region_polys);
    let t2 = std::time::Instant::now();
    let (tp_r, _, _) = scallop_toolpath_structured_annotated_with_cancel(
        &mesh,
        &index,
        &r15,
        &sparams,
        None,
        Some(&regions),
        &never_cancel,
    )
    .expect("scallop per region");
    eprintln!(
        "arm R generated: {} moves over {n_regions} regions (overlap {:.3} mm), {:.0} s",
        tp_r.moves.len(),
        RASTER_STEPOVER,
        t2.elapsed().as_secs_f64()
    );

    // ── cost + coverage, identically ──────────────────────────────────────
    let tris = board_triangles(&mesh, SCALLOP_HEIGHT);
    let bbox = [
        mesh.bbox.min.x - 5.0,
        mesh.bbox.min.y - 5.0,
        mesh.bbox.max.x + 5.0,
        mesh.bbox.max.y + 5.0,
    ];
    let slope = &surface.slope_map;
    let theta_at = |x: f64, y: f64| -> f64 {
        let col = ((x - slope.origin_x) / slope.cell_size)
            .round()
            .clamp(0.0, (slope.cols - 1) as f64) as usize;
        let row = ((y - slope.origin_y) / slope.cell_size)
            .round()
            .clamp(0.0, (slope.rows - 1) as f64) as usize;
        slope.angles[row * slope.cols + col].to_degrees()
    };
    eprintln!(
        "\ncoverage population: {} triangles, {:.0} mm² 3D (whole board, no slope filter)\n",
        tris.lifted.len(),
        tris.area_mm2
    );
    eprintln!(
        "     {:>38}  {:>9}  {:>9}  {:>9}  {:>8}  {:>8}  {:>9}",
        "arm", "time s", "cut mm", "rapid mm", "plunges", "moves", "unmach %"
    );
    let arms: [(&str, &Toolpath); 3] = [
        ("U   unified band mix (production)", &tp_u),
        ("C   scallop, whole board", &tp_c),
        ("R   scallop on planner regions (1-step)", &tp_r),
    ];
    let mut results: Vec<(String, ArmStats, f64)> = Vec::new();
    for (label, tp) in arms {
        let st = arm_stats(tp, &kinematics);
        let cov = coverage_pct(&tris, tp, r15.cusp_radius_mm(), bbox);
        eprintln!(
            "     {:>38}  {:>9.1}  {:>9.0}  {:>9.0}  {:>8}  {:>8}  {:>8.3}%",
            label, st.time_s, st.cutting_mm, st.rapid_mm, st.entry_plunges, st.moves, cov
        );
        results.push((label.to_owned(), st, cov));
    }
    let u = &results[0];
    eprintln!("\n---------- the comparison the bar reads ----------");
    for arm in &results[1..] {
        eprintln!(
            "   {}: time {:.3}x U, coverage {:+.3} pp vs U (equality margin {} pp)",
            arm.0,
            arm.1.time_s / u.1.time_s,
            arm.2 - u.2,
            COVERAGE_EQUALITY_MARGIN_PP
        );
    }

    // ── SVGs for the operator's eyeball ───────────────────────────────────
    let out = std::path::Path::new("target/scallop_vs_unified_s1");
    let _ = std::fs::create_dir_all(out);
    let board = [
        mesh.bbox.min.x,
        mesh.bbox.min.y,
        mesh.bbox.max.x,
        mesh.bbox.max.y,
    ];
    let mut peak = (0.0, 0.0, f64::NEG_INFINITY);
    for v in &mesh.vertices {
        let interior = v.x > mesh.bbox.min.x + 30.0
            && v.x < mesh.bbox.max.x - 30.0
            && v.y > mesh.bbox.min.y + 30.0
            && v.y < mesh.bbox.max.y - 30.0;
        if interior && v.z > peak.2 {
            peak = (v.x, v.y, v.z);
        }
    }
    let peak_win = [peak.0 - 20.0, peak.1 - 20.0, peak.0 + 20.0, peak.1 + 20.0];
    let flats_win = [49.75, 123.0625, 89.75, 163.0625];
    for (name, tp) in [("armU", &tp_u), ("armC", &tp_c), ("armR", &tp_r)] {
        dump_svg(
            &out.join(format!("{name}_full.svg")),
            tp,
            &theta_at,
            board,
            0.08,
        );
        dump_svg(
            &out.join(format!("{name}_peak.svg")),
            tp,
            &theta_at,
            peak_win,
            0.05,
        );
        dump_svg(
            &out.join(format!("{name}_flats.svg")),
            tp,
            &theta_at,
            flats_win,
            0.05,
        );
    }
    eprintln!(
        "\nSVGs written to target/scallop_vs_unified_s1/; total wall {:.0} s",
        started.elapsed().as_secs_f64()
    );
}
