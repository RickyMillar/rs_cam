//! Track G — the strategy ledger's harness arms on the REAL wanaka board.
//!
//! `planning/ledger_2026-09-01/FINDINGS.md` pre-registers the arms and the
//! D1 derate rule. This instrument runs:
//!
//! * **D2** — whole-board EDT spiral, XY spacing at the flat-law stepover
//!   (the legacy raster's spacing convention).
//! * **D1** — whole-board EDT spiral, spec-honest ring spacing (per-ring
//!   worst-cross-slope derate, clamped at 45 degrees — the production
//!   clamp).
//! * **CAL** — the shipped `unified_finish` generator on the same mesh,
//!   costed by the same integrator, so the harness scale and the CLI scale
//!   meet on one op.
//!
//! The board outline is the terrain mesh XY bbox. The rectangle EDT is
//! closed-form: level sets are nested axis-aligned rectangles about the
//! centre, so the level schedule is a march of insets. The rings go through
//! `spiral_finish_compact::bridge_nested_levels` (the C1 mechanism), every
//! point re-drops through the drop-cutter with the PRODUCTION R1.0 tapered
//! ball (gouge check), and costing is the C1/F-034 relink block.
//!
//! Instrument conventions restated from
//! `tests/spiral_finish_compact_c1.rs` (C1) and
//! `tests/shipped_raster_spacing_b1.rs` (Track B):
//!
//! * achieved spacing: min 3D distance from ring-k surface samples to
//!   ring-(k+1)'s surface polyline; exceedance = `> 1.05 x s_flat`;
//!   spec met when <= 10 % of samples exceed.
//! * coverage audit: every board triangle centroid, lifted `h` along its
//!   face normal, vs the FINAL relinked CL path at radius `K_c`.
//! * `link_ceiling: None` — the fresh-stock first-experiment exception.
//!
//! One disclosed instrument refinement over the registration's wording:
//! the derate and spacing samples are matched along the ring's inward SIDE
//! NORMAL, not along hub rays. On a rectangle a hub-ray gap overstates the
//! pass gap everywhere off the axis midlines; the normal gap is the pass
//! gap. The registered `gap_xy` is unchanged (`s_flat`, or the placed
//! `s_k`).
//!
//! Run:
//!
//! ```text
//! cargo test --release -p rs_cam_core --test whole_board_spiral_ledger_g1 \
//!   -- --ignored --nocapture
//! ```

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use rs_cam_core::finish::spiral_finish_compact::{CompactSpiralParams, bridge_nested_levels};
use rs_cam_core::geo::{P2, P3};
use rs_cam_core::geometry::region_set::RegionSet;
use rs_cam_core::machine::kinematics::{LinkKinematics, MachineKinematics, compute_cycle_time};
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::metrology::costing::{
    CandidateCost, CostingContext, CostingFeeds, relink_and_cost as metrology_relink_and_cost,
};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::tool::{BallEndmill, MillingCutter, TaperedBallEndmill};
use rs_cam_core::toolpath::{MoveIntent, Toolpath};

// ── the production job's constants (wanaka200_mt2.toml) ─────────────────

const MESH_PATH: &str = "/home/ricky/Downloads/wanaka200/rivmap_export/terrain.stl";

const BALL_RADIUS_MM: f64 = 1.0;
const CUSP_HEIGHT_MM: f64 = 0.03;
/// The production tier-1 `raster_stepover` (flat law, K_c 1.0, h 0.03).
const S_FLAT_MM: f64 = 0.486_209_831_245_728_7;
/// The production clamp: `steep_threshold_deg`.
const THETA_CLAMP_DEG: f64 = 45.0;

const MACHINE_ACCEL_XYZ: [f64; 3] = [500.0, 500.0, 270.0];
const MACHINE_ACCEL_SCALAR: f64 = 423.333_333_333_333_3;
const JUNCTION_DEVIATION_MM: f64 = 0.02;
const MAX_FEED_MM_MIN: f64 = 10_000.0;
const RAPID_FEED_MM_MIN: f64 = 5_000.0;
const FEED_MM_MIN: f64 = 735.0;
const PLUNGE_MM_MIN: f64 = 180.0;

/// Perimeter sample step (mm) for ring construction and surface probing —
/// the production `sampling` dial.
const RING_STEP_MM: f64 = 0.5;

/// Spec bars restated from Track B: exceedance and material fraction.
const EXCEED_FACTOR: f64 = 1.05;
const EXCEED_MATERIAL_BAR_PCT: f64 = 10.0;
/// Coverage bar restated from C1-F4.
const MAX_UNMACHINED_FRACTION: f64 = 0.02;

const FRESH_STOCK_LABEL: &str = "link_ceiling: None — FRESH-STOCK FIRST-EXPERIMENT EXCEPTION \
                                 (no time claim is final until a rest-stock re-run)";

fn production_cutter() -> TaperedBallEndmill {
    // tool_id 2: ball dia 2.0, taper half-angle 5.7 deg, shaft dia 6.0,
    // cutting length 20.
    TaperedBallEndmill::new(2.0, 5.7, 6.0, 20.0)
}

fn machine_kinematics() -> MachineKinematics {
    MachineKinematics {
        acceleration_mm_s2: MACHINE_ACCEL_SCALAR,
        acceleration_xyz_mm_s2: Some(MACHINE_ACCEL_XYZ),
        junction_deviation_mm: JUNCTION_DEVIATION_MM,
        ..MachineKinematics::default()
    }
}

fn load_board() -> (TriangleMesh, SpatialIndex) {
    let path = Path::new(MESH_PATH);
    assert!(
        path.exists(),
        "the wanaka terrain mesh is not at {MESH_PATH} — this evidence instrument needs the \
         production model"
    );
    let mesh = TriangleMesh::from_stl_scaled(path, 1.0).expect("terrain STL loads");
    let index = SpatialIndex::build_auto(&mesh);
    (mesh, index)
}

// ── rectangle rings (the closed-form rectangle EDT's level sets) ─────────

#[derive(Clone, Copy)]
struct Rect {
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
}

impl Rect {
    fn inset(self, d: f64) -> Rect {
        Rect {
            x0: self.x0 + d,
            y0: self.y0 + d,
            x1: self.x1 - d,
            y1: self.y1 - d,
        }
    }
    fn width(self) -> f64 {
        self.x1 - self.x0
    }
    fn height(self) -> f64 {
        self.y1 - self.y0
    }
}

/// One perimeter sample: XY point plus the inward side normal.
#[derive(Clone, Copy)]
struct RingSample {
    x: f64,
    y: f64,
    nx: f64,
    ny: f64,
}

/// Sample the rectangle ring CCW: bottom, right, top, left. Corners land
/// exactly once, at each side's start.
fn rect_ring_samples(r: Rect, step: f64) -> Vec<RingSample> {
    let mut out: Vec<RingSample> = Vec::new();
    let mut side = |ax: f64, ay: f64, bx: f64, by: f64, nx: f64, ny: f64| {
        let len = (bx - ax).hypot(by - ay);
        let n = ((len / step).ceil() as usize).max(2);
        for k in 0..n {
            let f = (k as f64) / (n as f64);
            out.push(RingSample {
                x: ax + (bx - ax) * f,
                y: ay + (by - ay) * f,
                nx,
                ny,
            });
        }
    };
    side(r.x0, r.y0, r.x1, r.y0, 0.0, 1.0);
    side(r.x1, r.y0, r.x1, r.y1, -1.0, 0.0);
    side(r.x1, r.y1, r.x0, r.y1, 0.0, -1.0);
    side(r.x0, r.y1, r.x0, r.y0, 1.0, 0.0);
    out
}

/// A ring as a closed XY loop for the bridge (z advisory 0; every point is
/// re-dropped later).
fn ring_loop(r: Rect, step: f64) -> Vec<P3> {
    let samples = rect_ring_samples(r, step);
    let mut pts: Vec<P3> = samples.iter().map(|s| P3::new(s.x, s.y, 0.0)).collect();
    if let Some(&first) = pts.first() {
        pts.push(first);
    }
    pts
}

// ── surface probing ──────────────────────────────────────────────────────

/// Surface z at (x, y) through a tiny-ball drop probe.
fn surface_z(
    x: f64,
    y: f64,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    probe: &BallEndmill,
) -> f64 {
    let cl = rs_cam_core::surface::dropcutter::point_drop_cutter(x, y, mesh, index, probe);
    if cl.contacted && cl.z.is_finite() {
        cl.z
    } else {
        mesh.bbox.min.z
    }
}

/// Worst cross-ring slope (deg) between ring samples and the points one
/// `gap` inward along each sample's side normal.
fn worst_cross_slope_deg(
    samples: &[RingSample],
    gap: f64,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    probe: &BallEndmill,
) -> f64 {
    let mut worst = 0.0_f64;
    for s in samples {
        let z0 = surface_z(s.x, s.y, mesh, index, probe);
        let z1 = surface_z(s.x + s.nx * gap, s.y + s.ny * gap, mesh, index, probe);
        let theta = ((z1 - z0).abs() / gap).atan().to_degrees();
        worst = worst.max(theta);
    }
    worst
}

/// The pre-registered inset march. `derate: false` is D2 (constant
/// `S_FLAT_MM`); `derate: true` is D1 (per-ring worst-cross-slope derate,
/// clamped at 45 degrees). Returns the insets and the per-ring theta.
fn march_insets(
    board: Rect,
    derate: bool,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    probe: &BallEndmill,
) -> (Vec<f64>, Vec<f64>) {
    let d_max = board.width().min(board.height()) / 2.0 - 0.05;
    let mut insets: Vec<f64> = Vec::new();
    let mut thetas: Vec<f64> = Vec::new();
    let mut d = 0.0_f64;
    while d < d_max {
        insets.push(d);
        let theta = if derate {
            let samples = rect_ring_samples(board.inset(d), RING_STEP_MM);
            worst_cross_slope_deg(&samples, S_FLAT_MM, mesh, index, probe).min(THETA_CLAMP_DEG)
        } else {
            0.0
        };
        thetas.push(theta);
        d += S_FLAT_MM * theta.to_radians().cos();
    }
    // A final ring at d_max when the last placed ring leaves more than one
    // lateral reach to the innermost rectangle.
    if let Some(&last) = insets.last()
        && d_max - last > 0.1
    {
        insets.push(d_max);
        thetas.push(0.0);
    }
    (insets, thetas)
}

// ── achieved spacing (Track B semantics on ring pairs) ──────────────────

struct SpacingStats {
    samples: usize,
    exceeding: usize,
    min: f64,
    p50: f64,
    p90: f64,
    max: f64,
}

/// Min 3D distance from every 4th surface sample on ring k to ring
/// (k+1)'s surface polyline, windowed around the matched perimeter
/// fraction.
fn achieved_spacing(
    board: Rect,
    insets: &[f64],
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    probe: &BallEndmill,
) -> SpacingStats {
    let surface_ring = |d: f64| -> Vec<P3> {
        rect_ring_samples(board.inset(d), RING_STEP_MM)
            .iter()
            .map(|s| P3::new(s.x, s.y, surface_z(s.x, s.y, mesh, index, probe)))
            .collect()
    };
    let mut population: Vec<f64> = Vec::new();
    let mut inner = surface_ring(insets[0]);
    for pair in insets.windows(2) {
        let outer = inner;
        inner = surface_ring(pair[1]);
        let n_outer = outer.len();
        let n_inner = inner.len();
        if n_inner < 3 {
            break;
        }
        for (j, p) in outer.iter().enumerate().step_by(4) {
            let centre = (j * n_inner) / n_outer;
            let mut best = f64::INFINITY;
            for w in -60_i64..=60 {
                let a = (centre as i64 + w).rem_euclid(n_inner as i64) as usize;
                let b = (a + 1) % n_inner;
                best = best.min(point_segment_distance_sq(*p, inner[a], inner[b]));
            }
            population.push(best.sqrt());
        }
    }
    population.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = population.len();
    let bar = EXCEED_FACTOR * S_FLAT_MM;
    SpacingStats {
        samples: n,
        exceeding: population.iter().filter(|&&v| v > bar).count(),
        min: population.first().copied().unwrap_or(0.0),
        p50: population.get(n / 2).copied().unwrap_or(0.0),
        p90: population.get(n * 9 / 10).copied().unwrap_or(0.0),
        max: population.last().copied().unwrap_or(0.0),
    }
}

// ── CL conversion, toolpath, relink, audit (C1 conventions) ─────────────

struct ClConversion {
    polylines: Vec<Vec<P3>>,
    input_points: usize,
    dropped_points: usize,
}

fn cl_polylines(
    contact: &[Vec<P3>],
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &TaperedBallEndmill,
) -> ClConversion {
    let mut out = ClConversion {
        polylines: Vec::with_capacity(contact.len()),
        input_points: 0,
        dropped_points: 0,
    };
    for line in contact {
        out.input_points += line.len();
        let mut cl: Vec<P3> = Vec::with_capacity(line.len());
        for point in line {
            let cl_point = rs_cam_core::surface::dropcutter::point_drop_cutter(
                point.x, point.y, mesh, index, cutter,
            );
            if cl_point.contacted && cl_point.z.is_finite() {
                cl.push(P3::new(cl_point.x, cl_point.y, cl_point.z));
            } else {
                out.dropped_points += 1;
            }
        }
        if cl.len() >= 2 {
            out.polylines.push(cl);
        }
    }
    out
}

fn polylines_to_toolpath(
    polylines: &[Vec<P3>],
    feed_rate: f64,
    plunge_rate: f64,
    safe_z: f64,
) -> Toolpath {
    let mut tp = Toolpath::new();
    let mut down_at: Option<P3> = None;
    for line in polylines {
        let Some(first) = line.first() else { continue };
        if let Some(prev) = down_at.take() {
            tp.rapid_to_with_intent(P3::new(prev.x, prev.y, safe_z), MoveIntent::Retract);
        }
        tp.rapid_to_with_intent(P3::new(first.x, first.y, safe_z), MoveIntent::Linking);
        tp.feed_to_with_intent(*first, plunge_rate, MoveIntent::EntryPlunge);
        for point in &line[1..] {
            tp.feed_to_with_intent(*point, feed_rate, MoveIntent::FinishingCut);
        }
        down_at = line.last().copied();
    }
    if let Some(prev) = down_at {
        tp.rapid_to_with_intent(P3::new(prev.x, prev.y, safe_z), MoveIntent::Retract);
    }
    tp
}

// PROMOTED (Track M, 2026-09-02): the comparison kernel lives in
// `rs_cam_core::metrology::costing`, extracted from
// `thin_organic_island_widths.rs`; this file's copy was byte-equivalent up
// to the cutter's concrete type and which `CandidateCost` fields it kept.
// The adapter below keeps this instrument's original call shape; the feed
// pins are this file's own constants, unchanged.
fn relink_and_cost(
    raw: Toolpath,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &TaperedBallEndmill,
    boundary: &RegionSet<'_>,
    kinematics: &MachineKinematics,
    safe_z: f64,
) -> CandidateCost {
    let ctx = CostingContext {
        mesh,
        index,
        cutter,
        kinematics: Some(kinematics),
        feeds: CostingFeeds {
            feed_mm_min: FEED_MM_MIN,
            plunge_mm_min: PLUNGE_MM_MIN,
            max_feed_mm_min: MAX_FEED_MM_MIN,
            rapid_feed_mm_min: RAPID_FEED_MM_MIN,
        },
    };
    metrology_relink_and_cost(&ctx, raw, boundary, safe_z)
}

fn point_segment_distance_sq(p: P3, a: P3, b: P3) -> f64 {
    let (abx, aby, abz) = (b.x - a.x, b.y - a.y, b.z - a.z);
    let denominator = abx * abx + aby * aby + abz * abz;
    let (apx, apy, apz) = (p.x - a.x, p.y - a.y, p.z - a.z);
    let t = if denominator <= 1e-18 {
        0.0
    } else {
        ((apx * abx + apy * aby + apz * abz) / denominator).clamp(0.0, 1.0)
    };
    let (dx, dy, dz) = (apx - t * abx, apy - t * aby, apz - t * abz);
    dx * dx + dy * dy + dz * dz
}

struct SegmentGrid {
    cell_mm: f64,
    segments: Vec<(P3, P3)>,
    buckets: HashMap<(i64, i64), Vec<usize>>,
}

impl SegmentGrid {
    fn from_cut_moves(path: &Toolpath, cell_mm: f64) -> Self {
        let mut segments: Vec<(P3, P3)> = Vec::new();
        let mut position: Option<P3> = None;
        for mv in &path.moves {
            if mv.move_type.is_cutting()
                && matches!(mv.intent, MoveIntent::FinishingCut | MoveIntent::Linking)
                && let Some(prev) = position
            {
                segments.push((prev, mv.target));
            }
            position = Some(mv.target);
        }
        let mut buckets: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
        for (i, (a, b)) in segments.iter().enumerate() {
            let (x0, x1) = (a.x.min(b.x), a.x.max(b.x));
            let (y0, y1) = (a.y.min(b.y), a.y.max(b.y));
            let (cx0, cx1) = ((x0 / cell_mm).floor() as i64, (x1 / cell_mm).floor() as i64);
            let (cy0, cy1) = ((y0 / cell_mm).floor() as i64, (y1 / cell_mm).floor() as i64);
            for cx in cx0..=cx1 {
                for cy in cy0..=cy1 {
                    buckets.entry((cx, cy)).or_default().push(i);
                }
            }
        }
        Self {
            cell_mm,
            segments,
            buckets,
        }
    }

    fn covers(&self, p: P3, radius: f64) -> bool {
        let reach = ((radius / self.cell_mm).ceil() as i64) + 1;
        let (cx, cy) = (
            (p.x / self.cell_mm).floor() as i64,
            (p.y / self.cell_mm).floor() as i64,
        );
        let r2 = radius * radius;
        for dx in -reach..=reach {
            for dy in -reach..=reach {
                let Some(ids) = self.buckets.get(&(cx + dx, cy + dy)) else {
                    continue;
                };
                for &i in ids {
                    let (a, b) = self.segments[i];
                    if point_segment_distance_sq(p, a, b) <= r2 {
                        return true;
                    }
                }
            }
        }
        false
    }
}

struct AuditResult {
    tested: usize,
    uncovered: usize,
    unmachined_mm2: f64,
    region_mm2: f64,
}

/// C1-F4 on a mesh without an analytic jet: the lift direction is the FACE
/// normal (oriented +z — the terrain faces up).
fn audit_coverage(mesh: &TriangleMesh, path: &Toolpath) -> AuditResult {
    let grid = SegmentGrid::from_cut_moves(path, 0.5);
    let mut out = AuditResult {
        tested: 0,
        uncovered: 0,
        unmachined_mm2: 0.0,
        region_mm2: 0.0,
    };
    for face in &mesh.faces {
        let e1 = face.v[1] - face.v[0];
        let e2 = face.v[2] - face.v[0];
        let cross = e1.cross(&e2);
        let norm = cross.norm();
        if norm.is_nan() || norm <= 0.0 {
            continue;
        }
        let area = 0.5 * norm;
        out.region_mm2 += area;
        let mut n = cross / norm;
        if n.z < 0.0 {
            n = -n;
        }
        let cx = (face.v[0].x + face.v[1].x + face.v[2].x) / 3.0;
        let cy = (face.v[0].y + face.v[1].y + face.v[2].y) / 3.0;
        let cz = (face.v[0].z + face.v[1].z + face.v[2].z) / 3.0;
        let sh = P3::new(
            cx + n.x * CUSP_HEIGHT_MM,
            cy + n.y * CUSP_HEIGHT_MM,
            cz + n.z * CUSP_HEIGHT_MM,
        );
        out.tested += 1;
        if grid.covers(sh, BALL_RADIUS_MM) {
            continue;
        }
        out.uncovered += 1;
        out.unmachined_mm2 += area;
    }
    out
}

// ── SVG (the operator's picture; C1 renderer, restated) ─────────────────

fn svg_output_dir() -> PathBuf {
    std::env::var_os("THIN_ORGANIC_SVG_DIR")
        .map_or_else(|| PathBuf::from("target/ledger_g1"), PathBuf::from)
}

fn write_path_svg(path: &Toolpath, label: &str, colour: &str, file: &PathBuf) {
    let mut cut = String::new();
    let mut link = String::new();
    let mut air = String::new();
    let mut position: Option<P3> = None;
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    );
    for mv in &path.moves {
        let to = mv.target;
        min_x = min_x.min(to.x);
        min_y = min_y.min(to.y);
        max_x = max_x.max(to.x);
        max_y = max_y.max(to.y);
        if let Some(from) = position {
            let target = if !mv.move_type.is_cutting() {
                &mut air
            } else {
                match mv.intent {
                    MoveIntent::FinishingCut
                    | MoveIntent::ClearingCut
                    | MoveIntent::LeadIn
                    | MoveIntent::LeadOut => &mut cut,
                    MoveIntent::Linking => &mut link,
                    _ => &mut air,
                }
            };
            write!(
                target,
                "M {:.3} {:.3} L {:.3} {:.3} ",
                from.x, -from.y, to.x, -to.y
            )
            .expect("svg path");
        }
        position = Some(to);
    }
    let pad = 2.0;
    let (w, h) = (max_x - min_x + 2.0 * pad, max_y - min_y + 2.0 * pad);
    let svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"{:.3} {:.3} {:.3} {:.3}\" \
         width=\"1000\" height=\"1000\">\n\
         <rect x=\"{:.3}\" y=\"{:.3}\" width=\"{:.3}\" height=\"{:.3}\" fill=\"white\"/>\n\
         <path d=\"{air}\" fill=\"none\" stroke=\"#cc2222\" stroke-width=\"0.15\" \
         stroke-dasharray=\"0.9 0.6\"/>\n\
         <path d=\"{link}\" fill=\"none\" stroke=\"#22aa44\" stroke-width=\"0.18\"/>\n\
         <path d=\"{cut}\" fill=\"none\" stroke=\"{colour}\" stroke-width=\"0.06\"/>\n\
         <text x=\"{:.3}\" y=\"{:.3}\" font-size=\"2.4\" fill=\"#333\">{label}</text>\n\
         </svg>\n",
        min_x - pad,
        -(max_y) - pad,
        w,
        h,
        min_x - pad,
        -(max_y) - pad,
        w,
        h,
        min_x - pad + 1.0,
        -(max_y) - pad + 3.0,
    );
    std::fs::write(file, svg).expect("write SVG");
}

// ── one spiral arm, end to end ───────────────────────────────────────────

fn run_spiral_arm(label: &str, slug: &str, derate: bool) {
    eprintln!("\n══════════ {label} ══════════");
    let (mesh, index) = load_board();
    let board = Rect {
        x0: mesh.bbox.min.x,
        y0: mesh.bbox.min.y,
        x1: mesh.bbox.max.x,
        y1: mesh.bbox.max.y,
    };
    eprintln!(
        "   board: {:.1} x {:.1} mm, {} triangles, z [{:.3}, {:.3}]",
        board.width(),
        board.height(),
        mesh.faces.len(),
        mesh.bbox.min.z,
        mesh.bbox.max.z
    );
    let probe = BallEndmill::new(0.02, 5.0);
    let cutter = production_cutter();
    let kinematics = machine_kinematics();
    let safe_z = mesh.bbox.max.z + 5.0;

    // Reference floor (flat law): SUM area / s_flat. The flat law carries no
    // per-triangle curvature; stated as a reference, not an exact floor.
    // The area term is the promoted `metrology::floor::mesh_area_mm2`
    // (Track M, 2026-09-02).
    let area_mm2 = rs_cam_core::metrology::floor::mesh_area_mm2(&mesh);
    let l_min_flat = area_mm2 / S_FLAT_MM;
    eprintln!("   3D area {area_mm2:.0} mm^2; flat-law reference floor L_min = {l_min_flat:.0} mm");

    // -- the inset march (the registered rule) ------------------------------
    let march_start = std::time::Instant::now();
    let (insets, thetas) = march_insets(board, derate, &mesh, &index, &probe);
    let clamped = thetas
        .iter()
        .filter(|&&t| t >= THETA_CLAMP_DEG - 1e-9)
        .count();
    let theta_p50 = {
        let mut sorted = thetas.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        sorted.get(sorted.len() / 2).copied().unwrap_or(0.0)
    };
    eprintln!(
        "   inset march: {} rings in {:.1} s; theta p50 {:.1} deg, {} rings at the 45-deg \
         clamp ({:.1}%)",
        insets.len(),
        march_start.elapsed().as_secs_f64(),
        theta_p50,
        clamped,
        100.0 * (clamped as f64) / (thetas.len().max(1) as f64)
    );

    // -- achieved spacing (the spec column) ----------------------------------
    let spacing_start = std::time::Instant::now();
    let spacing = achieved_spacing(board, &insets, &mesh, &index, &probe);
    let exceed_pct = 100.0 * (spacing.exceeding as f64) / (spacing.samples.max(1) as f64);
    let spec_met = exceed_pct <= EXCEED_MATERIAL_BAR_PCT;
    eprintln!(
        "   ACHIEVED SPACING (surface, ring-to-ring; Track B semantics; {:.1} s): \
         min/p50/p90/max = {:.4}/{:.4}/{:.4}/{:.4} mm over {} samples",
        spacing_start.elapsed().as_secs_f64(),
        spacing.min,
        spacing.p50,
        spacing.p90,
        spacing.max,
        spacing.samples
    );
    eprintln!(
        "     exceeding {:.3} mm (1.05 x s_flat): {} samples = {:.2}% (bar \
         {EXCEED_MATERIAL_BAR_PCT}%) -> SPEC {}",
        EXCEED_FACTOR * S_FLAT_MM,
        spacing.exceeding,
        exceed_pct,
        if spec_met { "MET" } else { "NOT MET" }
    );

    // -- rings -> ONE spiral --------------------------------------------------
    let levels: Vec<Vec<Vec<P3>>> = insets
        .iter()
        .map(|&d| vec![ring_loop(board.inset(d), RING_STEP_MM)])
        .collect();
    let lateral_reach = {
        let v = BALL_RADIUS_MM - CUSP_HEIGHT_MM;
        (BALL_RADIUS_MM * BALL_RADIUS_MM - v * v).max(0.0).sqrt()
    };
    let bridge_params = CompactSpiralParams {
        max_chord_mm: RING_STEP_MM,
        hub_cover_reach_mm: lateral_reach,
        near_hub_bridge_span_rad: rs_cam_core::finish::conformal_spiral::PAPER_INITIAL_BRIDGE_SHIFT,
        ..CompactSpiralParams::default()
    };
    let bridge_start = std::time::Instant::now();
    let (bridged, bridge_report) = bridge_nested_levels(&levels, &bridge_params);
    let spiral = match bridged {
        Ok(s) => s,
        Err(refusal) => panic!(
            "the bridge refused with {refusal:?} on the closed-form rectangle EDT — a \
             pre-registered surprise. Report so far: {bridge_report:?}"
        ),
    };
    eprintln!(
        "   bridge ({:.1} s): {} rings on a {}-cell lattice, {} bridges, overhead {:.2}%, hub \
         blend {}, {} monotonized vertices, polar order violations {}",
        bridge_start.elapsed().as_secs_f64(),
        bridge_report.rings,
        bridge_report.lattice_angles,
        bridge_report.bridge_count,
        100.0 * bridge_report.bridge_overhead_fraction,
        bridge_report.hub_blend_added,
        bridge_report.monotonized_vertices,
        bridge_report.polar_order_violations
    );
    assert_eq!(
        bridge_report.polar_order_violations, 0,
        "self-intersection tripwire fired"
    );

    // -- gouge check: re-drop every point with the PRODUCTION tapered ball ---
    let drop_start = std::time::Instant::now();
    let conversion = cl_polylines(
        std::slice::from_ref(&spiral.contact),
        &mesh,
        &index,
        &cutter,
    );
    eprintln!(
        "   CL conversion (THE GOUGE CHECK, production R1.0 tapered ball; {:.1} s): {} in, {} \
         dropped points",
        drop_start.elapsed().as_secs_f64(),
        conversion.input_points,
        conversion.dropped_points
    );
    assert_eq!(
        conversion.polylines.len(),
        1,
        "the spiral must survive CL conversion as ONE polyline"
    );

    // -- cost through the production relink -----------------------------------
    let polygon = Polygon2::new(vec![
        P2::new(board.x0, board.y0),
        P2::new(board.x1, board.y0),
        P2::new(board.x1, board.y1),
        P2::new(board.x0, board.y1),
    ]);
    let region_set = RegionSet::new(vec![polygon]);
    eprintln!("   {FRESH_STOCK_LABEL}");
    let raw = polylines_to_toolpath(&conversion.polylines, FEED_MM_MIN, PLUNGE_MM_MIN, safe_z);
    let cost_start = std::time::Instant::now();
    let cost = relink_and_cost(
        raw,
        &mesh,
        &index,
        &cutter,
        &region_set,
        &kinematics,
        safe_z,
    );
    eprintln!(
        "   F-034 COST ({:.1} s): {} moves, cutting {:.0} mm, rapid {:.0} mm, {} fragments, {} \
         links, {} kept retracts, integrated {:.1} s ({:.2} h), x flat-floor {:.3}",
        cost_start.elapsed().as_secs_f64(),
        cost.moves,
        cost.cutting_mm,
        cost.rapid_mm,
        cost.fragments,
        cost.linked,
        cost.kept_retracts,
        cost.time_s,
        cost.time_s / 3600.0,
        cost.cutting_mm / l_min_flat
    );

    // -- coverage audit --------------------------------------------------------
    let audit_start = std::time::Instant::now();
    let audit = audit_coverage(&mesh, &cost.path);
    let unmachined_pct = 100.0 * audit.unmachined_mm2 / audit.region_mm2;
    eprintln!(
        "   COVERAGE AUDIT ({:.1} s): {} of {} centroids uncovered = {:.4}% of {:.0} mm^2 (bar \
         {:.0}%)",
        audit_start.elapsed().as_secs_f64(),
        audit.uncovered,
        audit.tested,
        unmachined_pct,
        audit.region_mm2,
        100.0 * MAX_UNMACHINED_FRACTION
    );
    assert!(
        unmachined_pct <= 100.0 * MAX_UNMACHINED_FRACTION,
        "coverage audit fired: {unmachined_pct:.4}% unmachined"
    );

    let dir = svg_output_dir();
    std::fs::create_dir_all(&dir).expect("create SVG output dir");
    let svg = dir.join(format!("{slug}.svg"));
    write_path_svg(
        &cost.path,
        &format!(
            "{label} — cut {:.0} mm, {:.1} s, {} retracts",
            cost.cutting_mm, cost.time_s, cost.kept_retracts
        ),
        "#2244cc",
        &svg,
    );
    eprintln!("   SVG: {}", svg.display());

    eprintln!(
        "\n   {label} LEDGER ROW: cutting {:.0} mm | rapid {:.0} mm | retracts {} | F-034 \
         {:.1} s | spec {} (p50 {:.4}, exceed {:.2}%) | rings {} | coverage {:.4}% unmachined",
        cost.cutting_mm,
        cost.rapid_mm,
        cost.kept_retracts,
        cost.time_s,
        if spec_met { "MET" } else { "NOT MET" },
        spacing.p50,
        exceed_pct,
        insets.len(),
        unmachined_pct
    );
}

// ── the arms ─────────────────────────────────────────────────────────────

#[test]
#[ignore = "Track G evidence instrument — run explicitly with --ignored --nocapture"]
fn ledger_d2_xy_spaced_spiral() {
    run_spiral_arm(
        "ARM D2 — whole-board spiral, XY-spaced (legacy raster spacing)",
        "wanaka_d2_spiral_xy",
        false,
    );
}

#[test]
#[ignore = "Track G evidence instrument — run explicitly with --ignored --nocapture"]
fn ledger_d1_spec_honest_spiral() {
    run_spiral_arm(
        "ARM D1 — whole-board spiral, spec-honest (per-ring worst-slope derate)",
        "wanaka_d1_spiral_honest",
        true,
    );
}

/// CAL — the shipped `unified_finish` generator (arm B's core path) on the
/// same mesh, costed by the harness integrator. Pairs with CLI arm B to
/// calibrate the two scales.
#[test]
#[ignore = "Track G evidence instrument — run explicitly with --ignored --nocapture"]
fn ledger_cal_unified_whole_board() {
    use rs_cam_core::finish::finish_planner::FinishPlannerParams;
    use rs_cam_core::finish::unified_finish::{
        UnifiedFinishParams, unified_finish_toolpath_with_cancel,
    };

    eprintln!(
        "\n══════════ ARM CAL — shipped unified_finish, whole board, harness scale ══════════"
    );
    let (mesh, index) = load_board();
    let cutter = production_cutter();
    let kinematics = machine_kinematics();
    let safe_z = mesh.bbox.max.z + 5.0;
    let link_kinematics = LinkKinematics {
        kinematics,
        max_feed_mm_min: MAX_FEED_MM_MIN,
        rapid_feed_mm_min: RAPID_FEED_MM_MIN,
    };
    // Tier-1 params verbatim (wanaka200_mt2.toml, Finish tier 1).
    let params = UnifiedFinishParams {
        scallop_height: CUSP_HEIGHT_MM,
        tolerance: 0.05,
        raster_stepover: S_FLAT_MM,
        z_step: S_FLAT_MM,
        sampling: 0.5,
        stock_to_leave: 0.0,
        feed_rate: FEED_MM_MIN,
        plunge_rate: PLUNGE_MM_MIN,
        safe_z,
        intra_region_hookup_mm: 25.0,
        monotone_cell_decomposition: true,
        ..UnifiedFinishParams::default()
    };
    let planner = FinishPlannerParams::for_tool(cutter.cusp_radius_mm());
    let never_cancel = || false;
    let generate_start = std::time::Instant::now();
    let (toolpath, _anns, report) = unified_finish_toolpath_with_cancel(
        &mesh,
        &index,
        &cutter,
        mesh.bbox.max.z + 1.0,
        mesh.bbox.min.z - 1.0,
        &params,
        &planner,
        None,
        Some(&link_kinematics),
        None,
        None,
        &never_cancel,
    )
    .expect("uncancelled generation");
    eprintln!(
        "   generation: {:.1} s; {} planned regions; {} shallow slope derates",
        generate_start.elapsed().as_secs_f64(),
        report.region_table.len(),
        report.shallow_slope_derates.len()
    );
    let retracts = toolpath
        .moves
        .iter()
        .filter(|m| !m.move_type.is_cutting() && matches!(m.intent, MoveIntent::Retract))
        .count();
    let time_s = compute_cycle_time(&toolpath, &kinematics, MAX_FEED_MM_MIN, RAPID_FEED_MM_MIN);
    eprintln!(
        "\n   ARM CAL LEDGER ROW: cutting {:.0} mm | rapid {:.0} mm | retracts {} | F-034 \
         {:.1} s ({:.2} h) | moves {}",
        toolpath.total_cutting_distance(),
        toolpath.total_rapid_distance(),
        retracts,
        time_s,
        time_s / 3600.0,
        toolpath.moves.len()
    );

    let dir = svg_output_dir();
    std::fs::create_dir_all(&dir).expect("create SVG output dir");
    let svg = dir.join("wanaka_cal_unified.svg");
    write_path_svg(
        &toolpath,
        &format!(
            "ARM CAL — shipped unified_finish, cut {:.0} mm, {time_s:.1} s",
            toolpath.total_cutting_distance()
        ),
        "#886622",
        &svg,
    );
    eprintln!("   SVG: {}", svg.display());
}
