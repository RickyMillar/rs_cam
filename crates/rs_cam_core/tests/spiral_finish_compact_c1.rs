//! **Track C evidence instrument — the shape-selected spiral on COMPACT
//! regions** (`planning/spiral_finish_2026-09-01/TRACK.md`).
//!
//! Nested iso-distance (medial/EDT) offset rings from the field machinery of
//! synthesis §9 (`direction_field::solve_paths_with_target`, `V = −|V|·∇̂EDT`),
//! bridged into ONE continuous gouge-checked path by
//! [`rs_cam_core::spiral_finish_compact`], which reuses the F2 log-rectangle
//! bridging blend. **No conformal map anywhere** — the slit map measured
//! 1.87× slower (FINDINGS_F2 §F2-4) and is out of scope by charter.
//!
//! Two analytic fixtures, facets ≤ stepover/3 (asserted, C1-F9):
//!
//! * SPHERE — the F2 convex cap, restated: the direct comparison with
//!   synthesis §9's 1.036×-floor medial row.
//! * DISH — NEW: the same lattice mirrored into a spherical-cap POCKET, so
//!   the drop-cutter gouge check genuinely engages on concave geometry.
//!   Concavity radius 20 mm = 20× the ball radius.
//!
//! The pre-registered falsifiers C1-F1..C1-F9 live in
//! `planning/spiral_finish_2026-09-01/FINDINGS.md`, written BEFORE this ran.
//!
//! Everything restated below names its source, per the repo rule that an
//! instrument shows its own arithmetic (integration tests cannot import from
//! each other):
//!
//! * constants, kinematics, `equal_cusp_stepover_mm`, `svg_output_dir`,
//!   `grid_for_direction`, `raster_candidate`, `relink_and_cost`,
//!   `cl_polylines`, `polylines_to_toolpath`, `sweep_target_axis`,
//!   `sphere_cap_mesh`, the jets and `region_floor` — from
//!   `conformal_spiral_synthetic_f2.rs` (which restates most of them from
//!   `direction_field_wanaka_f1.rs` / `thin_organic_island_widths.rs`).
//!
//! # Running it
//!
//! ```text
//! cargo test --release -p rs_cam_core --test spiral_finish_compact_c1 -- --ignored --nocapture
//! ```

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

use std::collections::HashMap;
use std::f64::consts::TAU;
use std::fmt::Write as _;
use std::path::PathBuf;

use rs_cam_core::direction_field::{self, FieldParams};
use rs_cam_core::geo::{P2, P3, V3};
use rs_cam_core::geometry::region_set::RegionSet;
use rs_cam_core::machine::kinematics::MachineKinematics;
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::metrology::costing::{
    CandidateCost, CostingContext, CostingFeeds, relink_and_cost as metrology_relink_and_cost,
};
use rs_cam_core::metrology::floor::{FloorReport, region_floor as metrology_region_floor};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::scallop_math;
use rs_cam_core::spiral_finish_compact::{
    CompactSpiralParams, CompactSpiralRefusal, bridge_nested_levels, levels_from_field_result,
};
use rs_cam_core::tool::BallEndmill;
use rs_cam_core::toolpath::{MoveIntent, Toolpath};

// ── the tooling decision (conformal_spiral_synthetic_f2.rs:371-383) ─────

const BALL_RADIUS_MM: f64 = 1.0;
const BALL_CUTTING_LENGTH_MM: f64 = 20.0;
const CUSP_HEIGHT_MM: f64 = 0.03;

// ── the pinned machine (conformal_spiral_synthetic_f2.rs:455-467, itself
//     direction_field_wanaka_f1.rs:125-132: Shapeoko Pro XXL) ───────────

const MACHINE_ACCEL_XYZ: [f64; 3] = [500.0, 500.0, 270.0];
const MACHINE_ACCEL_SCALAR: f64 = 423.333_333_333_333_3;
const JUNCTION_DEVIATION_MM: f64 = 0.02;
const MAX_FEED_MM_MIN: f64 = 10_000.0;
const RAPID_FEED_MM_MIN: f64 = 5_000.0;
const FEED_MM_MIN: f64 = 735.0;
const PLUNGE_MM_MIN: f64 = 180.0;

/// conformal_spiral_synthetic_f2.rs:469-473, restated verbatim.
const FRESH_STOCK_LABEL: &str = "link_ceiling: None — FRESH-STOCK FIRST-EXPERIMENT EXCEPTION (PROGRAMME.md \
     'Shared experimental contract'). NO TIME CLAIM HERE IS FINAL until this \
     candidate is re-run under the corrected rest-stock ceiling.";

// ── the fixtures (conformal_spiral_synthetic_f2.rs:3530-3545) ───────────

const SPHERE_RADIUS_MM: f64 = 20.0;
const CAP_RADIUS_MM: f64 = 6.0;
/// F2 pinned 55 × 340 against the FLAT-law ceiling (0.4862/3 = 0.162). This
/// instrument asserts against the tighter CURVED-law target on the convex
/// sphere (0.4743/3 = 0.158), which 55 × 340's 0.1585 mm max edge misses by
/// 0.4 % — C1-F9 fired on the first run. Densified to 60 × 380.
const CAP_RINGS: usize = 60;
const CAP_SECTORS: usize = 380;

/// C1-F4 STOP threshold — the F2 coverage-audit bar, restated.
const MAX_UNMACHINED_FRACTION: f64 = 0.02;

/// C1-F7 STOP threshold — `MAX_BRIDGE_OVERHEAD_PCT`, restated from F2.
const MAX_BRIDGE_OVERHEAD_PCT: f64 = 25.0;

/// C1-F6 — the charter's pre-registered bar. Do not move it.
const MAX_FLOOR_RATIO: f64 = 1.25;

/// C1-F5 — the F2 spacing verdict band, restated.
const SPACING_BAND_PCT: f64 = 25.0;

// ── restated arithmetic ─────────────────────────────────────────────────

/// `s = 2·√(2Rh − h²)` (conformal_spiral_synthetic_f2.rs:512-518).
fn equal_cusp_stepover_mm(cusp_radius_mm: f64, h: f64) -> f64 {
    if h <= 0.0 || h >= cusp_radius_mm {
        return 0.0;
    }
    2.0 * (2.0 * cusp_radius_mm * h - h * h).sqrt()
}

/// SVG landing directory (conformal_spiral_synthetic_f2.rs:535-546): same
/// `THIN_ORGANIC_SVG_DIR` override, its own default directory so a C1 run
/// cannot overwrite an F1 or F2 run.
fn svg_output_dir() -> PathBuf {
    std::env::var_os("THIN_ORGANIC_SVG_DIR").map_or_else(
        || {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("..")
                .join("target")
                .join("spiral_finish_c1")
        },
        PathBuf::from,
    )
}

// ── analytic fixtures ───────────────────────────────────────────────────

/// The polar cap lattice of conformal_spiral_synthetic_f2.rs:3590-3636,
/// generalised by one sign: `concave = false` is the F2 dome
/// (`z(r) = √(R²−r²) − √(R²−a²)`, apex up); `concave = true` is the NEW dish
/// (`z(r) = √(R²−a²) − √(R²−r²)`, rim at z = 0, bottom below). Both wind
/// counter-clockwise in XY, so every face normal has `n_z > 0`.
fn spherical_cap_mesh(
    sphere_radius_mm: f64,
    cap_radius_mm: f64,
    rings: usize,
    sectors: usize,
    concave: bool,
) -> TriangleMesh {
    let rings = rings.max(1);
    let sectors = sectors.max(3);
    let rim_w = (sphere_radius_mm * sphere_radius_mm - cap_radius_mm * cap_radius_mm)
        .max(0.0)
        .sqrt();
    let height = |r: f64| {
        let w = (sphere_radius_mm * sphere_radius_mm - r * r)
            .max(0.0)
            .sqrt();
        if concave { rim_w - w } else { w - rim_w }
    };

    let mut verts: Vec<P3> = Vec::with_capacity(1 + rings * sectors);
    verts.push(P3::new(0.0, 0.0, height(0.0)));
    for i in 1..=rings {
        let r = cap_radius_mm * (i as f64) / (rings as f64);
        let z = height(r);
        for j in 0..sectors {
            let t = TAU * (j as f64) / (sectors as f64);
            verts.push(P3::new(r * t.cos(), r * t.sin(), z));
        }
    }
    let v = |i: usize, j: usize| -> u32 { (1 + (i - 1) * sectors + (j % sectors)) as u32 };
    let mut tris: Vec<[u32; 3]> = Vec::with_capacity(sectors * (2 * rings - 1));
    for j in 0..sectors {
        tris.push([0, v(1, j), v(1, j + 1)]);
    }
    for i in 2..=rings {
        for j in 0..sectors {
            let (a, b) = (v(i - 1, j), v(i - 1, j + 1));
            let (c, d) = (v(i, j), v(i, j + 1));
            tris.push([a, c, d]);
            tris.push([a, d, b]);
        }
    }
    TriangleMesh::from_raw(verts, tris)
}

/// First and second heightfield partials at one point, in closed form
/// (conformal_spiral_synthetic_f2.rs:1926-1936). Analytic on purpose: no
/// estimator inside the floor or the target field.
#[derive(Clone, Copy)]
struct SurfaceJet {
    fx: f64,
    fy: f64,
    fxx: f64,
    fxy: f64,
    fyy: f64,
}

/// One fixture's closed-form surface, convex-POSITIVE curvature convention
/// (conformal_spiral_synthetic_f2.rs:1948-2005, restated field for field).
#[derive(Clone, Copy)]
struct AnalyticSurface {
    name: &'static str,
    jet: fn(f64, f64) -> SurfaceJet,
}

impl AnalyticSurface {
    /// Normal curvature (convex-positive) along the in-plane direction `u`.
    fn normal_curvature(self, x: f64, y: f64, u: V3) -> f64 {
        let j = (self.jet)(x, y);
        let w = (1.0 + j.fx * j.fx + j.fy * j.fy).sqrt();
        let (a, b) = (u.x, u.y);
        let tangent = a * j.fx + b * j.fy;
        let first = a * a + b * b + tangent * tangent;
        if first.is_nan() || first <= 1e-15 || !w.is_finite() {
            return 0.0;
        }
        let second = j.fxx * a * a + 2.0 * j.fxy * a * b + j.fyy * b * b;
        -second / (w * first)
    }

    /// `(κ_min, κ_max)`, convex-positive.
    fn principal_curvatures(self, x: f64, y: f64) -> (f64, f64) {
        let j = (self.jet)(x, y);
        let w2 = 1.0 + j.fx * j.fx + j.fy * j.fy;
        let w = w2.sqrt();
        if !w.is_finite() || w2 <= 0.0 {
            return (0.0, 0.0);
        }
        let (e, f, g) = (1.0 + j.fx * j.fx, j.fx * j.fy, 1.0 + j.fy * j.fy);
        let (l, m, n) = (j.fxx / w, j.fxy / w, j.fyy / w);
        let denominator = e * g - f * f;
        if denominator.is_nan() || denominator <= 1e-15 {
            return (0.0, 0.0);
        }
        let mean = (e * n - 2.0 * f * m + g * l) / (2.0 * denominator);
        let gauss = (l * n - m * m) / denominator;
        let discriminant = (mean * mean - gauss).max(0.0).sqrt();
        (-(mean + discriminant), -(mean - discriminant))
    }

    /// Upward unit surface normal.
    fn normal(self, x: f64, y: f64) -> V3 {
        let j = (self.jet)(x, y);
        let n = V3::new(-j.fx, -j.fy, 1.0);
        let len = n.norm();
        if len > 1e-12 {
            n / len
        } else {
            V3::new(0.0, 0.0, 1.0)
        }
    }
}

/// conformal_spiral_synthetic_f2.rs:2013-2024, restated.
fn sphere_jet(x: f64, y: f64) -> SurfaceJet {
    let w2 = (SPHERE_RADIUS_MM * SPHERE_RADIUS_MM - x * x - y * y).max(1e-9);
    let w = w2.sqrt();
    let w3 = w2 * w;
    SurfaceJet {
        fx: -x / w,
        fy: -y / w,
        fxx: -(w2 + x * x) / w3,
        fxy: -(x * y) / w3,
        fyy: -(w2 + y * y) / w3,
    }
}

/// The NEW dish: `z = c − √(R²−x²−y²)` is the negated sphere heightfield
/// (plus a constant that derivatives kill), so its jet is the negated
/// [`sphere_jet`]. Every curvature reads `−1/R_s` convex-positive — concave.
fn dish_jet(x: f64, y: f64) -> SurfaceJet {
    let j = sphere_jet(x, y);
    SurfaceJet {
        fx: -j.fx,
        fy: -j.fy,
        fxx: -j.fxx,
        fxy: -j.fxy,
        fyy: -j.fyy,
    }
}

const SPHERE_SURFACE: AnalyticSurface = AnalyticSurface {
    name: "sphere cap  z = sqrt(R_s^2 - r^2) - c,  R_s = 20 mm  (convex, UMBILIC)",
    jet: sphere_jet,
};
const DISH_SURFACE: AnalyticSurface = AnalyticSurface {
    name: "dish        z = c - sqrt(R_s^2 - r^2),  R_s = 20 mm  (CONCAVE pocket, umbilic)",
    jet: dish_jet,
};

// PROMOTED (Track M, 2026-09-02): `FloorReport` and the floor integrand
// live in `rs_cam_core::metrology::floor` (extracted from
// conformal_spiral_synthetic_f2.rs). Disclosed divergence, closed by the
// promotion: this file's copy computed only the κ_min basis and tested only
// that basis for degeneracy; the library computes both bases and counts a
// triangle degenerate when EITHER collapses. On these umbilic fixtures
// κ_min = κ_max, so this file's numbers do not move.
fn region_floor(mesh: &TriangleMesh, surface: AnalyticSurface) -> FloorReport {
    metrology_region_floor(mesh, None, BALL_RADIUS_MM, CUSP_HEIGHT_MM, &|x, y| {
        surface.principal_curvatures(x, y)
    })
}
// ── the medial/EDT target field ─────────────────────────────────────────

/// `V_dir = n × normalise(d − n(n·d))` — the pinned convention
/// (conformal_spiral_synthetic_f2.rs:2462-2475, restated). The one line of
/// the derivation that can be silently inverted, so it is the SAME line here
/// as in both F2 field arms.
fn sweep_target_axis(n: V3, d: V3) -> Option<V3> {
    let projected = d - n * d.dot(&n);
    let norm = projected.norm();
    if norm.is_nan() || norm <= 1e-9 {
        return None;
    }
    let rotated = n.cross(&(projected / norm));
    let length = rotated.norm();
    if length > 1e-9 {
        Some(rotated / length)
    } else {
        None
    }
}

// ── candidate costing (conformal_spiral_synthetic_f2.rs:571-698) ────────

fn grid_for_direction(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &BallEndmill,
    stepover: f64,
    direction_deg: f64,
) -> rs_cam_core::surface::dropcutter::DropCutterGrid {
    rs_cam_core::surface::dropcutter::batch_drop_cutter(
        mesh,
        index,
        cutter,
        stepover,
        direction_deg,
        mesh.bbox.min.z - 0.1,
    )
}

fn raster_candidate(
    grid: &rs_cam_core::surface::dropcutter::DropCutterGrid,
    regions: &[Polygon2],
    safe_z: f64,
    effective_min_z: f64,
) -> Toolpath {
    use rs_cam_core::toolpath::raster_toolpath_from_grid;
    let mut out = Toolpath::new();
    for polygon in regions {
        let region = RegionSet::new(vec![polygon.clone()]);
        let toolpath = raster_toolpath_from_grid(
            grid,
            FEED_MM_MIN,
            PLUNGE_MM_MIN,
            safe_z,
            Some(effective_min_z),
            Some(&region),
        );
        out.moves.extend(toolpath.moves);
    }
    out
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
    cutter: &BallEndmill,
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

/// Contact polylines → drop-cutter CL polylines
/// (conformal_spiral_synthetic_f2.rs:711-748). **This is the gouge check**:
/// every point is re-dropped against the whole mesh, so the emitted CL can
/// never gouge, whatever the contact-space construction did. Uncontacted
/// probes (`z = −∞`) are dropped and counted, never absorbed.
struct ClConversion {
    polylines: Vec<Vec<P3>>,
    input_points: usize,
    dropped_points: usize,
    dropped_polylines: usize,
}

fn cl_polylines(
    contact: &[Vec<P3>],
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &BallEndmill,
) -> ClConversion {
    let mut out = ClConversion {
        polylines: Vec::with_capacity(contact.len()),
        input_points: 0,
        dropped_points: 0,
        dropped_polylines: 0,
    };
    for line in contact {
        out.input_points += line.len();
        let mut cl: Vec<P3> = Vec::with_capacity(line.len());
        for point in line {
            let probe = rs_cam_core::surface::dropcutter::point_drop_cutter(
                point.x, point.y, mesh, index, cutter,
            );
            if probe.contacted && probe.z.is_finite() {
                cl.push(probe.position());
            } else {
                out.dropped_points += 1;
            }
        }
        if cl.len() >= 2 {
            out.polylines.push(cl);
        } else {
            out.dropped_polylines += 1;
        }
    }
    out
}

/// CL polylines → raw toolpath (conformal_spiral_synthetic_f2.rs:760-786).
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

// ── the independent coverage audit ──────────────────────────────────────

/// Squared 3D point-segment distance
/// (conformal_spiral_synthetic_f2.rs:790-803, restated).
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

/// XY bucket grid over the CUT segments of a costed toolpath, so 37k
/// centroid queries do not scan 5k segments each.
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

    /// `true` when `p` lies within `radius` of any cut segment.
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
    worst_uncovered_dist_mm: f64,
    uncovered_r_min: f64,
    uncovered_r_max: f64,
}

/// The independent audit, C1-F4: every mesh triangle centroid, lifted to
/// `S^h` along the ANALYTIC normal, tested against the FINAL relinked CL
/// path at the TRUE ball radius. Same semantics as
/// `conformal_spiral::audit_coverage`, with one strengthening: the curve
/// under test is the post-gouge, post-relink motion, not a pre-lift centre
/// curve.
fn audit_coverage(mesh: &TriangleMesh, surface: AnalyticSurface, path: &Toolpath) -> AuditResult {
    let grid = SegmentGrid::from_cut_moves(path, 0.5);
    let mut out = AuditResult {
        tested: 0,
        uncovered: 0,
        unmachined_mm2: 0.0,
        region_mm2: 0.0,
        worst_uncovered_dist_mm: 0.0,
        uncovered_r_min: f64::INFINITY,
        uncovered_r_max: 0.0,
    };
    for face in &mesh.faces {
        let e1 = face.v[1] - face.v[0];
        let e2 = face.v[2] - face.v[0];
        let area = 0.5 * e1.cross(&e2).norm();
        if area.is_nan() || area <= 0.0 {
            continue;
        }
        out.region_mm2 += area;
        let cx = (face.v[0].x + face.v[1].x + face.v[2].x) / 3.0;
        let cy = (face.v[0].y + face.v[1].y + face.v[2].y) / 3.0;
        let cz = (face.v[0].z + face.v[1].z + face.v[2].z) / 3.0;
        let n = surface.normal(cx, cy);
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
        let r = cx.hypot(cy);
        out.uncovered_r_min = out.uncovered_r_min.min(r);
        out.uncovered_r_max = out.uncovered_r_max.max(r);
        // Exact distance only for the uncovered few.
        let mut best = f64::INFINITY;
        for (a, b) in &grid.segments {
            best = best.min(point_segment_distance_sq(sh, *a, *b));
        }
        if best.is_finite() {
            out.worst_uncovered_dist_mm = out.worst_uncovered_dist_mm.max(best.sqrt());
        }
    }
    out
}

// ── mesh census (C1-F9) ─────────────────────────────────────────────────

struct MeshCensus {
    triangles: usize,
    max_edge_mm: f64,
    median_edge_mm: f64,
}

fn mesh_census(mesh: &TriangleMesh) -> MeshCensus {
    let mut edges: Vec<f64> = Vec::with_capacity(mesh.faces.len() * 3);
    let mut max_edge = 0.0_f64;
    for face in &mesh.faces {
        for (a, b) in [(0usize, 1usize), (1, 2), (2, 0)] {
            let len = (face.v[a] - face.v[b]).norm();
            edges.push(len);
            max_edge = max_edge.max(len);
        }
    }
    edges.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = edges.get(edges.len() / 2).copied().unwrap_or(0.0);
    MeshCensus {
        triangles: mesh.faces.len(),
        max_edge_mm: max_edge,
        median_edge_mm: median,
    }
}

// ── SVG (the operator's picture) ────────────────────────────────────────

/// Draw one costed toolpath in XY: cuts solid, surface links green, air red
/// dashed. Classification restated from
/// conformal_spiral_synthetic_f2.rs:876-896 — by the move's OWN tags.
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
    let pad = 1.0;
    let (w, h) = (max_x - min_x + 2.0 * pad, max_y - min_y + 2.0 * pad);
    let svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"{:.3} {:.3} {:.3} {:.3}\" \
         width=\"900\" height=\"900\">\n\
         <rect x=\"{:.3}\" y=\"{:.3}\" width=\"{:.3}\" height=\"{:.3}\" fill=\"white\"/>\n\
         <path d=\"{air}\" fill=\"none\" stroke=\"#cc2222\" stroke-width=\"0.05\" \
         stroke-dasharray=\"0.3 0.2\"/>\n\
         <path d=\"{link}\" fill=\"none\" stroke=\"#22aa44\" stroke-width=\"0.06\"/>\n\
         <path d=\"{cut}\" fill=\"none\" stroke=\"{colour}\" stroke-width=\"0.04\"/>\n\
         <text x=\"{:.3}\" y=\"{:.3}\" font-size=\"0.6\" fill=\"#333\">{label} — cut \
         {colour}, links green, air red dashed (plunges are XY-degenerate)</text>\n\
         </svg>\n",
        min_x - pad,
        -(max_y) - pad,
        w,
        h,
        min_x - pad,
        -(max_y) - pad,
        w,
        h,
        min_x - pad + 0.3,
        -(max_y) - pad + 0.8,
    );
    std::fs::write(file, svg).expect("write SVG");
}

// ── one fixture, end to end ─────────────────────────────────────────────

struct FixtureSpec {
    label: &'static str,
    slug: &'static str,
    surface: AnalyticSurface,
    concave: bool,
}

#[allow(clippy::too_many_lines)]
fn run_fixture(spec: &FixtureSpec) {
    eprintln!("\n══════════ {} ══════════", spec.label);
    let mesh = spherical_cap_mesh(
        SPHERE_RADIUS_MM,
        CAP_RADIUS_MM,
        CAP_RINGS,
        CAP_SECTORS,
        spec.concave,
    );
    let index = SpatialIndex::build_auto(&mesh);
    let cutter = BallEndmill::new(BALL_RADIUS_MM * 2.0, BALL_CUTTING_LENGTH_MM);
    let kinematics = MachineKinematics {
        acceleration_mm_s2: MACHINE_ACCEL_SCALAR,
        acceleration_xyz_mm_s2: Some(MACHINE_ACCEL_XYZ),
        junction_deviation_mm: JUNCTION_DEVIATION_MM,
        ..MachineKinematics::default()
    };
    let safe_z = mesh.bbox.max.z + 5.0;
    let effective_min_z = mesh.bbox.min.z - 0.1;
    let region: Vec<u32> = (0..mesh.triangles.len() as u32).collect();

    // -- C1-F9: the fixture must resolve the measurand ---------------------
    let (kappa, _) = spec.surface.principal_curvatures(0.0, 0.0);
    let target_spacing =
        scallop_math::stepover_from_scallop_curved(BALL_RADIUS_MM, CUSP_HEIGHT_MM, kappa);
    let flat_stepover = equal_cusp_stepover_mm(BALL_RADIUS_MM, CUSP_HEIGHT_MM);
    let census = mesh_census(&mesh);
    eprintln!("   surface: {}", spec.surface.name);
    eprintln!(
        "   fixture: {} triangles, max edge {:.4} mm, median edge {:.4} mm",
        census.triangles, census.max_edge_mm, census.median_edge_mm
    );
    eprintln!(
        "   analytic spacing target (curved law, kappa {kappa:+.5}): {target_spacing:.5} mm; \
         flat law {flat_stepover:.5} mm"
    );
    eprintln!(
        "   FACET-TO-STEPOVER PROOF (C1-F9): stepover / max edge = {:.2}x, / median edge = \
         {:.2}x  (must be >= 3x)",
        target_spacing / census.max_edge_mm,
        target_spacing / census.median_edge_mm
    );
    assert!(
        census.max_edge_mm <= target_spacing / 3.0,
        "C1-F9 FIRED: max facet edge {:.4} > stepover/3 = {:.4} — the fixture cannot resolve \
         the measurand (the F2 withdrawal's lesson)",
        census.max_edge_mm,
        target_spacing / 3.0
    );

    // -- the floor ----------------------------------------------------------
    let floor = region_floor(&mesh, spec.surface);
    eprintln!(
        "   FLOOR: L_min = SUM area_t / s_max(t) = {:.4} mm over {:.4} mm^2 ({} degenerate)",
        floor.l_min_mm, floor.area_mm2, floor.degenerate
    );
    assert_eq!(
        floor.degenerate, 0,
        "no concavity here is tighter than the ball"
    );

    // -- the medial/EDT field: V = -|V| * grad_hat(EDT) ---------------------
    // On a circular-boundary region the EDT is closed-form: EDT = a - r, so
    // grad EDT = -r_hat EXACTLY. This restates the medial arm of
    // conformal_spiral_synthetic_f2.rs (synthesis §9) with the grid EDT
    // replaced by the closed form the grid was approximating; the magnitude
    // is Zou Eq. 13 on the pinned convention, unchanged.
    let field_params = FieldParams::new(BALL_RADIUS_MM, CUSP_HEIGHT_MM);
    let inverse_radius = 1.0 / BALL_RADIUS_MM;
    let flat_magnitude = (inverse_radius / 8.0).sqrt();
    let surface = spec.surface;
    let solve_start = std::time::Instant::now();
    let (result, report) = direction_field::solve_paths_with_target(
        &mesh,
        &region,
        |_global, c, n| {
            let r = c.x.hypot(c.y);
            if r <= 1e-6 {
                return V3::zeros(); // the hub: grad EDT is genuinely undefined
            }
            let g = V3::new(-c.x / r, -c.y / r, 0.0); // grad EDT, exact
            let projected = g - n * g.dot(&n);
            let norm = projected.norm();
            if norm.is_nan() || norm <= 1e-9 {
                return V3::zeros();
            }
            let feed = n.cross(&(projected / norm));
            let Some(unit) = sweep_target_axis(n, feed) else {
                return V3::zeros();
            };
            let k_s = surface.normal_curvature(c.x, c.y, unit);
            let denominator = k_s + inverse_radius;
            let magnitude = if denominator > 1e-12 {
                (denominator / 8.0).sqrt()
            } else {
                flat_magnitude
            };
            unit * magnitude
        },
        &field_params,
    );
    let solve_s = solve_start.elapsed().as_secs_f64();
    eprintln!(
        "   field solve: {} levels, {} polylines ({} closed), cg {} iters (converged {}), \
         {:.1} s",
        result.levels.len(),
        report.total_polylines,
        report.closed_loops,
        report.cg_iterations,
        report.cg_converged,
        solve_s
    );

    // -- levels -> nested rings -> ONE spiral --------------------------------
    let mut levels = levels_from_field_result(&result);
    let empty_before = levels.len();
    levels.retain(|l| !l.is_empty());
    let dropped_empty = empty_before - levels.len();

    // The rim ring — the classical boundary pass, pre-registered: the
    // schedule's outermost level can sit up to one increment inside the
    // boundary, and that band is uncoverable without a boundary pass.
    // CONDITIONAL since run 1: the sphere schedule's outermost level landed
    // 0.112 mm from the rim (inside the 0.23 mm projected reach), and an
    // unconditional rim ring then double-covers the rim band at a measured
    // +0.15x floor. So the ring is added only when the WORST rim gap exceeds
    // the XY-projected lateral reach; the independent audit (C1-F4) is the
    // backstop on this decision either way.
    let lateral_reach = {
        let v = BALL_RADIUS_MM - CUSP_HEIGHT_MM;
        (BALL_RADIUS_MM * BALL_RADIUS_MM - v * v).max(0.0).sqrt()
    };
    let rim_cos = {
        let j = (spec.surface.jet)(CAP_RADIUS_MM, 0.0);
        1.0 / (1.0 + j.fx * j.fx + j.fy * j.fy).sqrt()
    };
    let outer_r_min = levels
        .iter()
        .flat_map(|level| level.iter())
        .map(|line| {
            line.iter()
                .map(|p| p.x.hypot(p.y))
                .fold(f64::INFINITY, f64::min)
        })
        .fold(0.0_f64, f64::max);
    let rim_gap = CAP_RADIUS_MM - outer_r_min;
    let reach_xy = lateral_reach * rim_cos;
    let rim_added = rim_gap > reach_xy;
    if rim_added {
        let rim: Vec<P3> = (0..=CAP_SECTORS)
            .map(|j| {
                let t = TAU * ((j % CAP_SECTORS) as f64) / (CAP_SECTORS as f64);
                P3::new(CAP_RADIUS_MM * t.cos(), CAP_RADIUS_MM * t.sin(), 0.0)
            })
            .collect();
        levels.push(vec![rim]);
    }
    eprintln!(
        "   rings handed to the bridge: {} field levels ({} empty levels dropped); worst rim \
         gap {:.4} mm vs projected reach {:.4} mm -> RIM ring {}",
        levels.len() - usize::from(rim_added),
        dropped_empty,
        rim_gap,
        reach_xy,
        if rim_added { "ADDED" } else { "not needed" }
    );

    // DELIBERATE DEVIATION from the paper's near-centre rule (Pseudocode A-2
    // lines 6-9, 2π bridges near the centre): that rule serves the paper's
    // disk-domain ring SEARCH. Here the rings are already spaced by the field
    // schedule and the bridge ordering is proven on the lattice for ANY span,
    // so every bridge takes the outer π/10 span. Measured cost of the paper's
    // rule on these fixtures: ~19 mm of near-hub revolutions (run 2, dish
    // 1.297x). The independent audit (C1-F4) is the backstop on the coverage
    // of the steeper near-hub bridges.
    let bridge_params = CompactSpiralParams {
        hub_cover_reach_mm: lateral_reach,
        near_hub_bridge_span_rad: rs_cam_core::conformal_spiral::PAPER_INITIAL_BRIDGE_SHIFT,
        ..CompactSpiralParams::default()
    };
    let (bridged, bridge_report) = bridge_nested_levels(&levels, &bridge_params);
    let spiral = match bridged {
        Ok(s) => s,
        Err(refusal) => panic!(
            "C1-F1 FIRED on {}: the bridge refused with {refusal:?} on a compact fixture. \
             Report so far: {bridge_report:?}",
            spec.label
        ),
    };
    eprintln!(
        "   bridge: {} rings on a {}-cell lattice, hub ({:.4}, {:.4}), start angle {:.3} rad \
         ({} candidates)",
        bridge_report.rings,
        bridge_report.lattice_angles,
        bridge_report.hub_x,
        bridge_report.hub_y,
        bridge_report.start_angle_rad,
        bridge_report.start_candidates
    );
    eprintln!(
        "     ring runs {:.2} mm + bridges {:.2} mm = overhead {:.2}%  ({} bridges, hub blend \
         {}, {} monotonized vertices, levels reordered: {})",
        bridge_report.ring_run_length_mm,
        bridge_report.bridge_length_mm,
        100.0 * bridge_report.bridge_overhead_fraction,
        bridge_report.bridge_count,
        bridge_report.hub_blend_added,
        bridge_report.monotonized_vertices,
        bridge_report.levels_reordered
    );
    eprintln!(
        "     min ring separation {:.4} mm; pass separation min/p50/p90/max = {:.4} / {:.4} / \
         {:.4} / {:.4} mm over {} samples",
        bridge_report.min_ring_separation_mm,
        bridge_report.pass_separation_mm.min,
        bridge_report.pass_separation_mm.p50,
        bridge_report.pass_separation_mm.p90,
        bridge_report.pass_separation_mm.max,
        bridge_report.pass_separation_mm.samples
    );
    eprintln!(
        "     polar-domain order violations (C1-F3): {}",
        bridge_report.polar_order_violations
    );
    assert_eq!(
        bridge_report.polar_order_violations, 0,
        "C1-F3 FIRED: a lattice cell's radius increased on a revisit — self-intersection"
    );
    assert!(
        100.0 * bridge_report.bridge_overhead_fraction <= MAX_BRIDGE_OVERHEAD_PCT,
        "C1-F7 FIRED: bridging overhead {:.2}% > {MAX_BRIDGE_OVERHEAD_PCT}%",
        100.0 * bridge_report.bridge_overhead_fraction
    );

    // -- C1-F5: achieved spacing vs spec -------------------------------------
    let spacing_err_pct =
        100.0 * (bridge_report.pass_separation_mm.p50 - target_spacing).abs() / target_spacing;
    eprintln!(
        "   SPACING (C1-F5): median ring separation {:.5} mm vs analytic target \
         {target_spacing:.5} mm — {spacing_err_pct:.1}% off (band ±{SPACING_BAND_PCT}%)",
        bridge_report.pass_separation_mm.p50
    );
    assert!(
        spacing_err_pct <= SPACING_BAND_PCT,
        "C1-F5 FIRED: median spacing {:.5} outside ±{SPACING_BAND_PCT}% of {target_spacing:.5}",
        bridge_report.pass_separation_mm.p50
    );

    // -- gouge check: re-drop every contact point ----------------------------
    let conversion = cl_polylines(
        std::slice::from_ref(&spiral.contact),
        &mesh,
        &index,
        &cutter,
    );
    eprintln!(
        "   CL conversion (THE GOUGE CHECK — every point re-dropped through the mesh): {} in, \
         {} dropped points, {} dropped polylines",
        conversion.input_points, conversion.dropped_points, conversion.dropped_polylines
    );
    assert_eq!(
        conversion.polylines.len(),
        1,
        "the spiral must survive CL conversion as ONE polyline"
    );

    // -- cost, both arms through one relink ----------------------------------
    let polygon = {
        let ring: Vec<P2> = (0..CAP_SECTORS)
            .map(|k| {
                let t = TAU * (k as f64) / (CAP_SECTORS as f64);
                P2::new(CAP_RADIUS_MM * t.cos(), CAP_RADIUS_MM * t.sin())
            })
            .collect();
        Polygon2::new(ring)
    };
    let region_set = RegionSet::new(vec![polygon.clone()]);
    eprintln!("\n   -- F-034 cost through the production relink --");
    eprintln!("     {FRESH_STOCK_LABEL}");
    let raw_spiral =
        polylines_to_toolpath(&conversion.polylines, FEED_MM_MIN, PLUNGE_MM_MIN, safe_z);
    let spiral_cost = relink_and_cost(
        raw_spiral,
        &mesh,
        &index,
        &cutter,
        &region_set,
        &kinematics,
        safe_z,
    );

    let grid = grid_for_direction(&mesh, &index, &cutter, flat_stepover, 0.0);
    let raw_raster = raster_candidate(
        &grid,
        std::slice::from_ref(&polygon),
        safe_z,
        effective_min_z,
    );
    let raster_cost = relink_and_cost(
        raw_raster,
        &mesh,
        &index,
        &cutter,
        &region_set,
        &kinematics,
        safe_z,
    );

    eprintln!(
        "\n     {:<22} {:>7} {:>10} {:>8} {:>7} {:>7} {:>9} {:>9}",
        "arm", "moves", "cut mm", "frags", "linked", "retracts", "F-034 s", "x floor"
    );
    for (name, cost) in [
        ("EDT spiral (bridged)", &spiral_cost),
        ("0 deg ball raster", &raster_cost),
    ] {
        eprintln!(
            "     {:<22} {:>7} {:>10.1} {:>8} {:>7} {:>7} {:>9.1} {:>9.3}",
            name,
            cost.moves,
            cost.cutting_mm,
            cost.fragments,
            cost.linked,
            cost.kept_retracts,
            cost.time_s,
            cost.cutting_mm / floor.l_min_mm
        );
    }
    eprintln!(
        "     (the raster spaces in XY PROJECTION while the spiral spaces on the SURFACE — the \
         F2\n\
         \x20     fairness note; a raster x-floor below 1.0 is UNDER-COVERAGE, not a win)"
    );
    assert_eq!(
        spiral_cost.fragments, 1,
        "C1-F2 FIRED: the spiral arm relinked as {} fragments, not 1",
        spiral_cost.fragments
    );
    assert_eq!(
        spiral_cost.kept_retracts, 0,
        "C1-F2 FIRED: the spiral arm kept {} retracts",
        spiral_cost.kept_retracts
    );
    let floor_ratio = spiral_cost.cutting_mm / floor.l_min_mm;
    assert!(
        floor_ratio <= MAX_FLOOR_RATIO,
        "C1-F6 FIRED: {floor_ratio:.3}x floor > the pre-registered {MAX_FLOOR_RATIO}x bar"
    );

    // -- C1-F4: the independent coverage audit --------------------------------
    let audit = audit_coverage(&mesh, spec.surface, &spiral_cost.path);
    eprintln!(
        "\n   COVERAGE AUDIT (C1-F4, independent: every centroid's S^h vs the FINAL relinked CL \
         path at K_c {BALL_RADIUS_MM}):"
    );
    eprintln!(
        "     {} of {} centroids uncovered — {:.4} mm^2 of {:.4} mm^2 = {:.4}%",
        audit.uncovered,
        audit.tested,
        audit.unmachined_mm2,
        audit.region_mm2,
        100.0 * audit.unmachined_mm2 / audit.region_mm2
    );
    if audit.uncovered > 0 {
        eprintln!(
            "     uncovered centroids sit at r in [{:.3}, {:.3}] mm; worst distance beyond the \
             curve {:.4} mm",
            audit.uncovered_r_min, audit.uncovered_r_max, audit.worst_uncovered_dist_mm
        );
    }
    assert!(
        audit.unmachined_mm2 / audit.region_mm2 <= MAX_UNMACHINED_FRACTION,
        "C1-F4 FIRED: {:.4}% unmachined > {:.1}%",
        100.0 * audit.unmachined_mm2 / audit.region_mm2,
        100.0 * MAX_UNMACHINED_FRACTION
    );

    // -- the operator's picture ----------------------------------------------
    let dir = svg_output_dir();
    std::fs::create_dir_all(&dir).expect("create SVG output dir");
    let spiral_svg = dir.join(format!("{}_spiral_c1.svg", spec.slug));
    let raster_svg = dir.join(format!("{}_raster_c1.svg", spec.slug));
    write_path_svg(
        &spiral_cost.path,
        &format!(
            "{} — EDT spiral, {:.1} mm cut, {:.1} s, {} retracts, {:.3}x floor",
            spec.label,
            spiral_cost.cutting_mm,
            spiral_cost.time_s,
            spiral_cost.kept_retracts,
            floor_ratio
        ),
        "#2244cc",
        &spiral_svg,
    );
    write_path_svg(
        &raster_cost.path,
        &format!(
            "{} — 0 deg raster, {:.1} mm cut, {:.1} s, {} retracts, {:.3}x floor",
            spec.label,
            raster_cost.cutting_mm,
            raster_cost.time_s,
            raster_cost.kept_retracts,
            raster_cost.cutting_mm / floor.l_min_mm
        ),
        "#886622",
        &raster_svg,
    );
    eprintln!("\n   SVGs: {}", spiral_svg.display());
    eprintln!("         {}", raster_svg.display());

    eprintln!(
        "\n   {} VERDICT: {:.3}x floor (bar {MAX_FLOOR_RATIO}), {} retracts, {:.4}% unmachined, \
         spacing {:.1}% off target, overhead {:.2}% — ALL PRE-REGISTERED FALSIFIERS HELD",
        spec.label,
        floor_ratio,
        spiral_cost.kept_retracts,
        100.0 * audit.unmachined_mm2 / audit.region_mm2,
        spacing_err_pct,
        100.0 * bridge_report.bridge_overhead_fraction
    );
}

/// C1-F8, printed by the instrument as well as pinned by the module's unit
/// tests: a branched level refuses with a typed refusal, never a fallback.
fn print_branched_refusal() {
    let circle = |cx: f64, r: f64| -> Vec<P3> {
        let mut pts: Vec<P3> = (0..64)
            .map(|k| {
                let t = TAU * (k as f64) / 64.0;
                P3::new(cx + r * t.cos(), r * t.sin(), 0.0)
            })
            .collect();
        pts.push(pts[0]);
        pts
    };
    let levels = vec![
        vec![circle(0.0, 6.0)],
        vec![circle(-2.0, 1.0), circle(2.0, 1.0)], // an offset ring SPLIT at a branch
    ];
    let (result, _) = bridge_nested_levels(&levels, &CompactSpiralParams::default());
    eprintln!(
        "\n   REFUSAL-FIRST (C1-F8): a split offset ring (the branched signature) returns\n\
         \x20    {:?}\n\
         \x20  — a typed refusal, never a silent fallback. Pinned by the module's unit tests.",
        result.expect_err("a branched level must refuse")
    );
    assert!(matches!(
        bridge_nested_levels(&levels, &CompactSpiralParams::default()).0,
        Err(CompactSpiralRefusal::LevelNotSingleLoop { level: 1, loops: 2 })
    ));
}

#[test]
#[ignore = "Track C evidence instrument — run explicitly with --ignored --nocapture"]
fn spiral_finish_compact_c1_evidence() {
    eprintln!("\n########## TRACK C — shape-selected spiral on COMPACT regions ##########");
    eprintln!(
        "   planning/spiral_finish_2026-09-01/TRACK.md. Pre-registered falsifiers C1-F1..F9 in\n\
         \x20  FINDINGS.md, written BEFORE this run. Bar: <= {MAX_FLOOR_RATIO}x floor, 0 \
         retracts, coverage\n\
         \x20  audit <= {:.0}%, spacing within ±{SPACING_BAND_PCT}%, zero self-intersections. \
         NO CONFORMAL MAP.",
        100.0 * MAX_UNMACHINED_FRACTION
    );
    print_branched_refusal();
    run_fixture(&FixtureSpec {
        label: "ARM SPHERE (convex cap)",
        slug: "sphere_cap",
        surface: SPHERE_SURFACE,
        concave: false,
    });
    run_fixture(&FixtureSpec {
        label: "ARM DISH (concave pocket — NEW)",
        slug: "dish_pocket",
        surface: DISH_SURFACE,
        concave: true,
    });
    eprintln!("\n########## TRACK C evidence run complete. ##########\n");
}
