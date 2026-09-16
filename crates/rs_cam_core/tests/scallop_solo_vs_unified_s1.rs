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

mod common;
use common::scallop_oracle::{EnvelopeOracle, OracleGrid, OracleParams, StampKernel};

use rayon::prelude::*;
use rs_cam_core::classify_probe::ClassificationSampler;
use rs_cam_core::finish_planner::{FinishPlannerParams, decompose};
use rs_cam_core::finish_setup::{
    FinishResolutionPolicy, build_classification_surface_with_sampler_and_cancel,
};
use rs_cam_core::geo::P3;
use rs_cam_core::geometry::region_set::RegionSet;
use rs_cam_core::machine_kinematics::{LinkKinematics, MachineKinematics, compute_cycle_time};
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::scallop::{
    RingSource, ScallopDirection, ScallopParams, ScallopRingBudget, ScallopStepoverPolicy,
    StepoverGeometry, scallop_generation_resolution, scallop_toolpath_research,
    scallop_toolpath_structured_annotated_with_cancel,
    scallop_toolpath_structured_annotated_with_resolution_and_ring_budget,
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

// ── coverage audit: RESIDUAL ABOVE THE BALL-REACHABLE ENVELOPE ──────────
//
// Two operator-caught defects killed the mesh-centroid audit: (1) a real
// 40 mm hole diluted to +0.117 pp because ~57 % of raw mesh points are
// SUB-TOOL-RADIUS texture no R1.5 path can touch — the audit measured the
// tool, not the arms; (2) the equal-cusp law puts spec-spaced midpoints at
// exactly the coverage radius (knife edge). This audit instead scores each
// point of the drop-cutter ENVELOPE (the surface the ball CAN sculpt, same
// reference for every arm) by the residual height the arm leaves above it:
// residual = min over nearby path tips of ball-bottom height at that XY,
// minus the envelope z. Covered iff residual ≤ 1.25 × the cusp spec
// (≈ a 1.12× spacing exceedance). Absolute numbers are finally meaningful.

struct TipIndex {
    ox: f64,
    oy: f64,
    cell: f64,
    nx: usize,
    ny: usize,
    buckets: Vec<Vec<u32>>,
    tips: Vec<P3>,
}

impl TipIndex {
    /// Cutting-intent moves, resampled to ≤ `step` chords.
    fn build(toolpath: &Toolpath, step: f64, cell: f64, bbox: [f64; 4]) -> Self {
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
            tips: Vec::new(),
        };
        let mut push = |p: P3| {
            let c = (((p.x - me.ox) / me.cell).floor().max(0.0) as usize).min(nx - 1);
            let r = (((p.y - me.oy) / me.cell).floor().max(0.0) as usize).min(ny - 1);
            let id = me.tips.len() as u32;
            me.tips.push(p);
            me.buckets[r * nx + c].push(id);
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
            let len = (b - a).norm();
            let n = ((len / step).ceil() as usize).max(1);
            for k in 0..=n {
                let t = k as f64 / n as f64;
                push(a + (b - a) * t);
            }
        }
        me
    }

    /// Minimum ball-bottom height any nearby tip's ball reaches at `(x, y)`.
    fn ball_floor_at(&self, x: f64, y: f64, r: f64) -> f64 {
        let cc = ((x - self.ox) / self.cell).floor();
        let cr = ((y - self.oy) / self.cell).floor();
        let mut best = f64::INFINITY;
        for dr in -1isize..=1 {
            for dc in -1isize..=1 {
                let rr = cr as isize + dr;
                let c2 = cc as isize + dc;
                if rr < 0 || c2 < 0 || rr >= self.ny as isize || c2 >= self.nx as isize {
                    continue;
                }
                for &id in &self.buckets[rr as usize * self.nx + c2 as usize] {
                    let t = self.tips[id as usize];
                    let d2 = (t.x - x).powi(2) + (t.y - y).powi(2);
                    if d2 >= r * r {
                        continue;
                    }
                    best = best.min(t.z + r - (r * r - d2).sqrt());
                }
            }
        }
        best
    }
}

struct EnvelopeGrid {
    origin_x: f64,
    origin_y: f64,
    step: f64,
    cols: usize,
    rows: usize,
    z: Vec<f64>,
}

fn coverage_pct(
    env: &EnvelopeGrid,
    toolpath: &Toolpath,
    ball_r: f64,
    spec_cusp: f64,
    bbox: [f64; 4],
    window: [f64; 4],
    scatter_path: &std::path::Path,
) -> (f64, f64) {
    let tips = TipIndex::build(toolpath, 0.2, 1.25 * ball_r, bbox);
    let tol = 1.25 * spec_cusp;
    let [wx0, wy0, wx1, wy1] = window;
    let rows_out: Vec<(usize, bool)> = (0..env.rows * env.cols)
        .into_par_iter()
        .filter_map(|i| {
            let z = env.z[i];
            if !z.is_finite() {
                return None;
            }
            let x = env.origin_x + (i % env.cols) as f64 * env.step;
            let y = env.origin_y + (i / env.cols) as f64 * env.step;
            let floor = tips.ball_floor_at(x, y, ball_r);
            Some((i, floor - z > tol))
        })
        .collect();
    let mut tot = 0usize;
    let mut unc = 0usize;
    let mut wtot = 0usize;
    let mut wunc = 0usize;
    let mut svg = "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 200 200\" \
                   width=\"1000\" height=\"1000\">\n\
                   <rect width=\"100%\" height=\"100%\" fill=\"#101418\"/>\n\
                   <g fill=\"#ff5252\">\n"
        .to_owned();
    for (i, u) in rows_out {
        let x = env.origin_x + (i % env.cols) as f64 * env.step;
        let y = env.origin_y + (i / env.cols) as f64 * env.step;
        let in_win = x >= wx0 && x <= wx1 && y >= wy0 && y <= wy1;
        tot += 1;
        if in_win {
            wtot += 1;
        }
        if u {
            unc += 1;
            if in_win {
                wunc += 1;
            }
            let _ = write!(
                svg,
                "<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"0.15\"/>",
                x,
                200.0 - y
            );
        }
    }
    svg.push_str("</g>\n</svg>\n");
    let _ = std::fs::write(scatter_path, svg);
    (
        100.0 * unc as f64 / tot.max(1) as f64,
        100.0 * wunc as f64 / wtot.max(1) as f64,
    )
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
    let (tp_c, _, rep_c) = scallop_toolpath_structured_annotated_with_cancel(
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
        "arm C  (shipped budget) generated: {} moves, {:.0} s — cascade left uncut: \
         core {:.0} mm², net {:.0} mm² (G-SCALLOPBASIN attribution)",
        tp_c.moves.len(),
        t1.elapsed().as_secs_f64(),
        rep_c.uncut_core_mm2,
        rep_c.untouched_mm2,
    );
    // The HONEST full-coverage arms: cascade run to completion under the
    // PR-8c research budgets. C2 = reach-policy budget, C3 = the naive
    // clamp-floor raise (v3's +92 % control) — completion guaranteed.
    let budget_arm = |label: &str, budget: ScallopRingBudget| {
        let t = std::time::Instant::now();
        let (tp, _, rep) = scallop_toolpath_structured_annotated_with_resolution_and_ring_budget(
            &mesh,
            &index,
            &r15,
            &sparams,
            None,
            None,
            scallop_generation_resolution(&r15, TOLERANCE),
            budget,
            &never_cancel,
        )
        .expect("scallop budget arm");
        eprintln!(
            "arm {label} generated: {} moves, {:.0} s — cascade left uncut: core {:.0} mm², \
             net {:.0} mm²",
            tp.moves.len(),
            t.elapsed().as_secs_f64(),
            rep.uncut_core_mm2,
            rep.untouched_mm2,
        );
        tp
    };
    let tp_c2 = budget_arm(
        "C2 (reach-policy budget)",
        ScallopRingBudget::ReachPolicyStepover,
    );
    let tp_c3 = budget_arm(
        "C3 (clamp-floor budget) ",
        ScallopRingBudget::LoopClampFloor,
    );

    // ── arm ISO: M8 — the iso-field ring source (per-point spacing) ──────
    let t_iso = std::time::Instant::now();
    let iso_policy = ScallopStepoverPolicy {
        ring_source: RingSource::IsoField,
        ..ScallopStepoverPolicy::SHIPPED
    };
    let (tp_iso, _, rep_iso, _) = scallop_toolpath_research(
        &mesh,
        &index,
        &r15,
        &sparams,
        None,
        None,
        scallop_generation_resolution(&r15, TOLERANCE),
        ScallopRingBudget::LoopClampFloor,
        iso_policy,
        &never_cancel,
    )
    .expect("iso-field scallop");
    eprintln!(
        "arm ISO (iso-field rings) generated: {} moves, {:.0} s — cascade left uncut: \
         core {:.0} mm², net {:.0} mm²",
        tp_iso.moves.len(),
        t_iso.elapsed().as_secs_f64(),
        rep_iso.uncut_core_mm2,
        rep_iso.untouched_mm2,
    );

    // ── arm ISO-F: the field-resolution probe (M8b) — same policy, the
    //    field cell forced from the 0.75 mm envelope-quarter default down to
    //    an explicit 0.35 mm. Probes whether the grid-contour error floor is
    //    what costs ISO its 1.6 pp against C3, and what the sawtooth is. ──
    let t_isof = std::time::Instant::now();
    let (tp_isof, _, rep_isof, _) = scallop_toolpath_research(
        &mesh,
        &index,
        &r15,
        &sparams,
        None,
        None,
        FinishResolutionPolicy::explicit(0.35),
        ScallopRingBudget::LoopClampFloor,
        iso_policy,
        &never_cancel,
    )
    .expect("iso-field scallop, fine field");
    eprintln!(
        "arm ISO-F (0.35 mm field) generated: {} moves, {:.0} s — cascade left uncut: \
         core {:.0} mm², net {:.0} mm²",
        tp_isof.moves.len(),
        t_isof.elapsed().as_secs_f64(),
        rep_isof.uncut_core_mm2,
        rep_isof.untouched_mm2,
    );

    // ── arm ISO-C: the operator-caught INVERTED SLOPE LAW corrected —
    //    `StepoverGeometry::CosineSlope` (the measured-correct form,
    //    unfixable in the cascade because of the ring budget it no longer
    //    has). IsoField + CosineSlope + 0.35 mm field. This is the honest
    //    spec-correct candidate. ──
    let t_isoc = std::time::Instant::now();
    let isoc_policy = ScallopStepoverPolicy {
        ring_source: RingSource::IsoField,
        geometry: StepoverGeometry::CosineSlope,
        ..ScallopStepoverPolicy::SHIPPED
    };
    let (tp_isoc, _, rep_isoc, _) = scallop_toolpath_research(
        &mesh,
        &index,
        &r15,
        &sparams,
        None,
        None,
        FinishResolutionPolicy::explicit(0.35),
        ScallopRingBudget::LoopClampFloor,
        isoc_policy,
        &never_cancel,
    )
    .expect("iso-field scallop, cosine law");
    eprintln!(
        "arm ISO-C (cosine law, 0.35 mm field) generated: {} moves, {:.0} s — cascade left \
         uncut: core {:.0} mm², net {:.0} mm²",
        tp_isoc.moves.len(),
        t_isoc.elapsed().as_secs_f64(),
        rep_isoc.uncut_core_mm2,
        rep_isoc.untouched_mm2,
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
    // The shared reference envelope: what the R1.5 ball CAN sculpt.
    let envg = {
        let g = rs_cam_core::surface::dropcutter::batch_drop_cutter(
            &mesh,
            &index,
            &r15,
            0.25,
            0.0,
            mesh.bbox.min.z - 0.1,
        );
        let min_z = mesh.bbox.min.z - 0.1;
        let z: Vec<f64> = g
            .points
            .iter()
            .map(|p| if p.z > min_z + 0.001 { p.z } else { f64::NAN })
            .collect();
        EnvelopeGrid {
            origin_x: g.u_start,
            origin_y: g.v_start,
            step: g.x_step,
            cols: g.cols,
            rows: g.rows,
            z,
        }
    };
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
        "\ncoverage population: the R1.5-reachable ENVELOPE at 0.25 mm \
         ({} finite samples); uncovered = residual > 1.25 x cusp spec\n",
        envg.z.iter().filter(|z| z.is_finite()).count()
    );
    eprintln!(
        "     {:>38}  {:>9}  {:>9}  {:>9}  {:>8}  {:>8}  {:>9}",
        "arm", "time s", "cut mm", "rapid mm", "plunges", "moves", "unmach %"
    );
    let arms: [(&str, &Toolpath); 8] = [
        ("U   unified band mix (production)", &tp_u),
        ("C   scallop, shipped budget (truncates)", &tp_c),
        ("C2  scallop, reach-policy budget", &tp_c2),
        ("C3  scallop, clamp-floor budget", &tp_c3),
        ("ISO iso-field rings (per-point spacing)", &tp_iso),
        ("ISOF iso-field, 0.35 mm field cell", &tp_isof),
        ("ISOC iso-field + COSINE LAW, 0.35 mm", &tp_isoc),
        ("R   scallop on planner regions (1-step)", &tp_r),
    ];
    let centre_window = [80.0, 80.0, 120.0, 120.0];
    let out_dir = std::path::Path::new("target/scallop_vs_unified_s1");
    let _ = std::fs::create_dir_all(out_dir);
    let mut results: Vec<(String, ArmStats, f64)> = Vec::new();
    for (ai, (label, tp)) in arms.iter().enumerate() {
        let (label, tp) = (*label, *tp);
        let st = arm_stats(tp, &kinematics);
        let tag = format!("{ai}_{}", label.chars().next().unwrap_or('x'));
        // Ground truth beside the audit: cutting moves inside the window.
        let mut win_moves = 0usize;
        for i in 1..tp.moves.len() {
            let m = &tp.moves[i];
            if matches!(m.move_type, MoveType::Rapid)
                || !matches!(m.intent, MoveIntent::ClearingCut | MoveIntent::FinishingCut)
            {
                continue;
            }
            let t = m.target;
            if t.x >= centre_window[0]
                && t.x <= centre_window[2]
                && t.y >= centre_window[1]
                && t.y <= centre_window[3]
            {
                win_moves += 1;
            }
        }
        let (cov, win_cov) = coverage_pct(
            &envg,
            tp,
            r15.cusp_radius_mm(),
            SCALLOP_HEIGHT,
            bbox,
            centre_window,
            &out_dir.join(format!("uncovered_{tag}.svg")),
        );
        eprintln!(
            "     {:>38}  {:>9.1}  {:>9.0}  {:>9.0}  {:>8}  {:>8}  {:>8.3}%  \
             [centre 40mm window: {win_cov:.2} % unmach, {win_moves} cut moves]",
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
    for (name, tp) in [
        ("armU", &tp_u),
        ("armC", &tp_c),
        ("armC3", &tp_c3),
        ("armISO", &tp_iso),
        ("armISOC", &tp_isoc),
        ("armR", &tp_r),
    ] {
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
    // ── M8b gouge oracle (the adoption gate the envelope ruler cannot
    //    see): analytic envelope truth at 0.15 mm, deepest gouge per arm. ──
    {
        let t = std::time::Instant::now();
        const ORACLE_CELL_MM: f64 = 0.15;
        let kernel = StampKernel::new(&r15, ORACLE_CELL_MM, Some(r15.cusp_radius_mm() * 5.0));
        let grid = OracleGrid::for_mesh(&mesh, &r15, ORACLE_CELL_MM);
        let truth = EnvelopeOracle::true_surface_from_mesh(grid, &mesh, &index);
        for (label, tp) in [
            ("C3", &tp_c3),
            ("ISO", &tp_iso),
            ("ISO-F", &tp_isof),
            ("ISO-C", &tp_isoc),
        ] {
            let oracle = EnvelopeOracle::score(grid, truth.clone(), tp, &kernel, ORACLE_CELL_MM);
            let r = oracle.report(OracleParams::new(SCALLOP_HEIGHT));
            eprintln!(
                "  gouge oracle {label:>5}: deepest gouge {:.1} µm (normal {:.1} µm), \
                 gouged {:.1} mm², untouched {:.0} mm², standing {:.0} mm²",
                r.deepest_gouge_um,
                r.deepest_gouge_normal_um,
                r.gouge_mm2,
                r.untouched_mm2,
                r.standing_mm2
            );
        }
        eprintln!("  gouge oracle wall {:.0} s", t.elapsed().as_secs_f64());
    }

    // ── the operator's "see it milled": dexel-simulate the ISO toolpath
    //    alone and write the 6-view composite. ──
    {
        let t = std::time::Instant::now();
        let mut stock = rs_cam_core::dexel_stock::TriDexelStock::from_stock(
            mesh.bbox.min.x,
            mesh.bbox.min.y,
            mesh.bbox.max.x,
            mesh.bbox.max.y,
            mesh.bbox.min.z - 1.0,
            mesh.bbox.max.z,
            0.25,
        );
        stock.simulate_toolpath(
            &tp_isoc,
            &r15,
            rs_cam_core::dexel_stock::StockCutDirection::FromTop,
        );
        let (w, h) = (1800u32, 1200u32);
        let pixels = rs_cam_core::export::fingerprint::render_stock_composite(&stock, w, h);
        if let Some(img) = image::RgbaImage::from_raw(w, h, pixels) {
            let _ = img.save(out.join("iso_milled.png"));
        }
        eprintln!(
            "  ISO milled: dexel sim at 0.25 mm + composite render, {:.0} s → \
             target/scallop_vs_unified_s1/iso_milled.png",
            t.elapsed().as_secs_f64()
        );
    }

    eprintln!(
        "\nSVGs written to target/scallop_vs_unified_s1/; total wall {:.0} s",
        started.elapsed().as_secs_f64()
    );
}
