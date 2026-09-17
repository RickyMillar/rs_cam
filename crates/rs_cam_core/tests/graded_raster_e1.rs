//! Avenue F, phase E1 — the GRADED RASTER: continuous variable spacing
//! (`planning/metrology_2026-09-02/FINDINGS.md` §M5, pre-registered).
//!
//! M4 closed the decomposition route: every partition of dendritic
//! ground pays its perimeter in fragments. This is the one standing
//! route — spacing that varies WITHIN a continuous pass:
//!
//! * The allowed XY pitch field `a(x, y) = stepover · cos θ` (production
//!   slope map, clamped 45°) is ERODED in y by ±stepover/2 — a pass pair
//!   must respect the worst ground BETWEEN them.
//! * Per grid column, `φ(y) = ∫ dy / a_eroded`. `∂φ/∂y = 1/a > 0`, so φ
//!   is strictly monotone in y and every level set `φ = k` is a
//!   SINGLE-VALUED continuous curve `y = f_k(x)` — a raster row that
//!   bends and bunches but cannot loop or branch. Fragments arise ONLY
//!   from region clipping, exactly where straight rows already fragment.
//!   The zero-added-fragments claim holds by construction, not by hope.
//! * Out-of-territory column gaps accumulate at the clamp rate so
//!   `f_k(x)` stays continuous across concavities.
//!
//! Arms: S — the F2 control, verbatim. E — the graded raster over the
//! same territory, costed identically. Bars E-spec / E-frag / E-time are
//! in §M5. E0 (row-graded capture, arithmetic only) is report-only.
//!
//! Costing, link regime, coverage audit and helpers are restated from
//! `valley_branch_falsifier_h1.rs` via `banded_raster_costed_f2.rs`.
//!
//! NUMBERS ONLY — the ruling goes to the FINDINGS file.
//!
//! `#[ignore]` — evidence run; needs the operator's wanaka mesh. SKIPs
//! when absent.

#![allow(
    clippy::print_stderr,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::too_many_lines,
    clippy::too_many_arguments
)]

use std::collections::HashSet;
use std::path::Path;

use rayon::prelude::*;
use rs_cam_core::dexel_stock::TriDexelStock;
use rs_cam_core::finish::classify_probe::ClassificationSampler;
use rs_cam_core::finish::conformal_spiral::{CoverageAudit, DistanceStats};
use rs_cam_core::finish::finish_planner::{FinishBand, FinishPlannerParams, decompose};
use rs_cam_core::finish::finish_setup::build_classification_surface_with_sampler_and_cancel;
use rs_cam_core::finish::surface_link::LinkCeiling;
use rs_cam_core::finish::unified_finish::unified_finish_classification_resolution;
use rs_cam_core::geo::{P2, P3};
use rs_cam_core::geometry::grid_field::distance_transform_2d;
use rs_cam_core::geometry::region_set::RegionSet;
use rs_cam_core::machine::kinematics::MachineKinematics;
use rs_cam_core::maps::tier_islands::{TierIslandParams, extract_tier_islands};
use rs_cam_core::maps::tier_map::{
    ResidualTreatment, TierLadder, TierMap, TierMapParams, compute_tier_map,
};
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::tool::{MillingCutter, TaperedBallEndmill};
use rs_cam_core::toolpath::{MoveIntent, MoveType, Toolpath};

// ── fixtures and dials — verbatim from the H1 / G2 / F1 chain ───────────

const WANAKA_MESH: &str = "/home/ricky/Downloads/wanaka200/rivmap_export/terrain.stl";

const CELL_MM: f64 = 0.3;
const TOLERANCE_MM: f64 = 0.05;
const MARGIN_MM: f64 = 0.5;
const COARSENESS: f64 = 1.0;
const OVERLAP_MM: f64 = 2.0;
const MAX_REGIONS_PER_TIER: usize = 24;
const CUSP_HEIGHT_MM: f64 = 0.03;
const OP_TOLERANCE_MM: f64 = 0.05;

const MACHINE_ACCEL_XYZ: [f64; 3] = [500.0, 500.0, 270.0];
const MACHINE_ACCEL_SCALAR: f64 = 423.333_333_333_333_3;
const JUNCTION_DEVIATION_MM: f64 = 0.02;
const MAX_FEED_MM_MIN: f64 = 10_000.0;
const RAPID_FEED_MM_MIN: f64 = 5_000.0;
const FEED_MM_MIN: f64 = 735.0;
const PLUNGE_MM_MIN: f64 = 180.0;

const ROUGH_DIAMETER_MM: f64 = 6.0;
const ROUGH_CUTTING_LENGTH_MM: f64 = 25.0;
const ROUGH_STOCK_TO_LEAVE_AXIAL_MM: f64 = 0.5;

/// E-spec tolerance: a column gap may exceed the eroded-min `a` in that
/// gap by at most this factor, for >= 95 % of gaps (the ledger exceed
/// convention).
const SPEC_GAP_TOLERANCE: f64 = 1.05;

/// The H1 coverage gate.
const COVERAGE_FAIL_FRACTION: f64 = 0.01;

/// `s = 2·sqrt(2Rh − h²)` — the equal-cusp law.
fn equal_cusp_stepover_mm(cusp_radius_mm: f64, h: f64) -> f64 {
    if h <= 0.0 || h >= cusp_radius_mm {
        return 0.0;
    }
    2.0 * (2.0 * cusp_radius_mm * h - h * h).sqrt()
}

// ═══════════════════════════════════════════════════════════════════════
// Costing (restated from valley_branch_falsifier_h1.rs, §0i regime)
// ═══════════════════════════════════════════════════════════════════════

struct CandidateCost {
    cutting_mm: f64,
    time_s: f64,
    fragments: usize,
    linked: usize,
    kept_retracts: usize,
    toolpath: Toolpath,
}

struct LinkRegime<'a> {
    safe_z: f64,
    ceiling: Option<LinkCeiling<'a>>,
    flush_ride: bool,
    airborne: bool,
}

fn relink_and_cost_under(
    raw: Toolpath,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &TaperedBallEndmill,
    boundary: &RegionSet<'_>,
    kinematics: &MachineKinematics,
    regime: &LinkRegime<'_>,
) -> CandidateCost {
    use rs_cam_core::machine::kinematics::{LinkKinematics, compute_cycle_time};

    let link_kinematics = LinkKinematics {
        kinematics: *kinematics,
        max_feed_mm_min: MAX_FEED_MM_MIN,
        rapid_feed_mm_min: RAPID_FEED_MM_MIN,
    };
    let params = rs_cam_core::finish::surface_link::RelinkParams {
        hookup_distance: 25.0,
        stock_to_leave: 0.0,
        sampling: 0.5,
        feed_rate: FEED_MM_MIN,
        plunge_rate: PLUNGE_MM_MIN,
        safe_z: regime.safe_z,
        link_kinematics: Some(&link_kinematics),
        reorder: true,
        boundary: Some(boundary),
        link_ceiling: regime.ceiling,
        flush_ride: regime.flush_ride,
        airborne_links_may_leave_territory: regime.airborne,
    };
    let (linked, report) = rs_cam_core::finish::surface_link::relink_fragments(
        rs_cam_core::trace::toolpath_spans::AnnotatedToolpath::new(raw),
        mesh,
        index,
        cutter,
        &params,
    );
    let mut channels = rs_cam_core::trace::transform_provenance::ReconcileSet::new(None, None);
    let toolpath = linked.reconcile(&mut channels).into_inner().toolpath;
    let time_s = compute_cycle_time(&toolpath, kinematics, MAX_FEED_MM_MIN, RAPID_FEED_MM_MIN);
    CandidateCost {
        cutting_mm: toolpath.total_cutting_distance(),
        time_s,
        fragments: report.fragments,
        linked: report.surface_links,
        kept_retracts: report.retract_links,
        toolpath,
    }
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

fn nearest_contact_z(
    grid: &rs_cam_core::surface::dropcutter::DropCutterGrid,
    x: f64,
    y: f64,
    min_z: f64,
) -> Option<f64> {
    if grid.rows == 0 || grid.cols == 0 {
        return None;
    }
    let col = ((x - grid.u_start) / grid.x_step)
        .round()
        .clamp(0.0, (grid.cols - 1) as f64) as usize;
    let row = ((y - grid.v_start) / grid.y_step)
        .round()
        .clamp(0.0, (grid.rows - 1) as f64) as usize;
    let z = grid.get(row, col).z;
    (z > min_z + 0.001).then_some(z)
}

/// Bilinear z on a 0° drop-cutter grid. `None` when any corner has no
/// contact — the caller drops the point, as the raster's min_z filter does.
fn bilinear_contact_z(
    grid: &rs_cam_core::surface::dropcutter::DropCutterGrid,
    x: f64,
    y: f64,
    min_z: f64,
) -> Option<f64> {
    if grid.rows < 2 || grid.cols < 2 {
        return None;
    }
    let fx = ((x - grid.u_start) / grid.x_step).clamp(0.0, (grid.cols - 1) as f64 - 1e-9);
    let fy = ((y - grid.v_start) / grid.y_step).clamp(0.0, (grid.rows - 1) as f64 - 1e-9);
    let c0 = fx.floor() as usize;
    let r0 = fy.floor() as usize;
    let tx = fx - c0 as f64;
    let ty = fy - r0 as f64;
    let z00 = grid.get(r0, c0).z;
    let z01 = grid.get(r0, c0 + 1).z;
    let z10 = grid.get(r0 + 1, c0).z;
    let z11 = grid.get(r0 + 1, c0 + 1).z;
    let floor = min_z + 0.001;
    if z00 <= floor || z01 <= floor || z10 <= floor || z11 <= floor {
        return None;
    }
    let z0 = z00 + (z01 - z00) * tx;
    let z1 = z10 + (z11 - z10) * tx;
    Some(z0 + (z1 - z0) * ty)
}

/// The machined-stock ceiling (restated from H1's `machined_stock`).
fn machined_stock(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    coarse: &TaperedBallEndmill,
    tier_map: &TierMap,
    bounds: [f64; 4],
) -> TriDexelStock {
    use rs_cam_core::tool::FlatEndmill;

    let min_z = mesh.bbox.min.z - 0.1;
    let block_top_z = mesh.bbox.max.z;
    let [x0, y0, x1, y1] = bounds;
    let mut stock =
        TriDexelStock::from_stock(x0, y0, x1, y1, mesh.bbox.min.z - 1.0, block_top_z, CELL_MM);

    let rough_tool = FlatEndmill::new(ROUGH_DIAMETER_MM, ROUGH_CUTTING_LENGTH_MM);
    let rough_grid = rs_cam_core::surface::dropcutter::batch_drop_cutter(
        mesh,
        index,
        &rough_tool,
        CELL_MM,
        0.0,
        min_z,
    );
    let coarse_grid = rs_cam_core::surface::dropcutter::batch_drop_cutter(
        mesh, index, coarse, CELL_MM, 0.0, min_z,
    );

    let tier_zero: Vec<bool> = tier_map.labels.iter().map(|&label| label == 0).collect();
    let distance_to_tier_zero =
        distance_transform_2d(&tier_zero, tier_map.grid.ny, tier_map.grid.nx);
    let reach = OVERLAP_MM + coarse.cusp_radius_mm();

    let rows = stock.z_grid.rows;
    let cols = stock.z_grid.cols;
    let origin_x = stock.z_grid.origin_u;
    let origin_y = stock.z_grid.origin_v;
    let cell = stock.z_grid.cell_size;
    for row in 0..rows {
        let y = origin_y + row as f64 * cell;
        for col in 0..cols {
            let x = origin_x + col as f64 * cell;
            let Some(rough_z) = nearest_contact_z(&rough_grid, x, y, min_z) else {
                continue;
            };
            let mut top = rough_z + ROUGH_STOCK_TO_LEAVE_AXIAL_MM;
            let coarse_here = tier_map.nearest_cell(x, y).is_some_and(|(r, c)| {
                distance_to_tier_zero[r * tier_map.grid.nx + c] * tier_map.grid.cell_mm <= reach
            });
            if coarse_here && let Some(coarse_z) = nearest_contact_z(&coarse_grid, x, y, min_z) {
                top = top.min(coarse_z);
            }
            if top < block_top_z {
                stock.clear_above_at(row, col, top as f32);
            }
        }
    }
    stock
}

// ═══════════════════════════════════════════════════════════════════════
// Coverage audit (restated from H1)
// ═══════════════════════════════════════════════════════════════════════

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
                    best = best.min(point_segment_dist_sq(p, a, b));
                }
            }
        }
        best
    }
}

fn point_segment_dist_sq(p: P3, a: P3, b: P3) -> f64 {
    let ab = b - a;
    let denom = ab.norm_squared();
    let t = if denom <= 1e-18 {
        0.0
    } else {
        ((p - a).dot(&ab) / denom).clamp(0.0, 1.0)
    };
    (p - (a + ab * t)).norm_squared()
}

fn summarise(values: &mut [f64]) -> DistanceStats {
    values.sort_by(f64::total_cmp);
    let pick = |q: f64| -> f64 {
        if values.is_empty() {
            0.0
        } else {
            values[((values.len() - 1) as f64 * q).round() as usize]
        }
    };
    DistanceStats {
        samples: values.len(),
        min_mm: values.first().copied().unwrap_or(0.0),
        median_mm: pick(0.5),
        max_mm: values.last().copied().unwrap_or(0.0),
    }
}

/// The UNION of the regions' triangles: each mesh triangle is counted once,
/// for the first polygon whose XY contains its centroid (the H1
/// `region_triangles` convention, deduped across polygons).
struct UnionTriangles {
    lifted: Vec<P3>,
    areas: Vec<f64>,
    slopes_deg: Vec<f64>,
    area_mm2: f64,
}

impl UnionTriangles {
    /// The sub-population a raster CAN cover: triangles at or below the
    /// clamp. Ground steeper than the clamp is under-covered at spec by
    /// construction for every raster arm (Track H V1 / G2) — auditing it
    /// tells you about the territory, not the arm.
    fn below(&self, clamp_deg: f64) -> UnionTriangles {
        let mut out = UnionTriangles {
            lifted: Vec::new(),
            areas: Vec::new(),
            slopes_deg: Vec::new(),
            area_mm2: 0.0,
        };
        for i in 0..self.lifted.len() {
            if self.slopes_deg[i] <= clamp_deg {
                out.lifted.push(self.lifted[i]);
                out.areas.push(self.areas[i]);
                out.slopes_deg.push(self.slopes_deg[i]);
                out.area_mm2 += self.areas[i];
            }
        }
        out
    }
}

fn union_triangles(mesh: &TriangleMesh, polys: &[&Polygon2], scallop_h_mm: f64) -> UnionTriangles {
    let mut seen: HashSet<usize> = HashSet::new();
    let mut lifted = Vec::new();
    let mut areas = Vec::new();
    let mut slopes_deg = Vec::new();
    let mut area_mm2 = 0.0;
    for poly in polys {
        let [bx0, by0, bx1, by1] = poly.bbox();
        let rows: Vec<(usize, P3, f64, f64)> = (0..mesh.triangles.len())
            .into_par_iter()
            .filter_map(|t| {
                let tri = mesh.triangles[t];
                let p0 = mesh.vertices[tri[0] as usize];
                let p1 = mesh.vertices[tri[1] as usize];
                let p2 = mesh.vertices[tri[2] as usize];
                let cx = (p0.x + p1.x + p2.x) / 3.0;
                let cy = (p0.y + p1.y + p2.y) / 3.0;
                if cx < bx0 || cx > bx1 || cy < by0 || cy > by1 {
                    return None;
                }
                if !poly.contains_point(&P2::new(cx, cy)) {
                    return None;
                }
                let cross = (p1 - p0).cross(&(p2 - p0));
                let area = 0.5 * cross.norm();
                if area <= 0.0 || !area.is_finite() {
                    return None;
                }
                let n = cross / cross.norm();
                let n = if n.z < 0.0 { -n } else { n };
                let cz = (p0.z + p1.z + p2.z) / 3.0;
                let slope_deg = n.z.clamp(-1.0, 1.0).acos().to_degrees();
                Some((t, P3::new(cx, cy, cz) + n * scallop_h_mm, area, slope_deg))
            })
            .collect();
        for (t, p, a, sd) in rows {
            if seen.insert(t) {
                lifted.push(p);
                areas.push(a);
                slopes_deg.push(sd);
                area_mm2 += a;
            }
        }
    }
    UnionTriangles {
        lifted,
        areas,
        slopes_deg,
        area_mm2,
    }
}

fn audit_arm_coverage(
    tris: &UnionTriangles,
    toolpath: &Toolpath,
    cusp_radius_mm: f64,
    bbox: [f64; 4],
) -> CoverageAudit {
    let index = SegmentIndex::build(toolpath, cusp_radius_mm, 4.0 * cusp_radius_mm, bbox);
    let r2 = cusp_radius_mm * cusp_radius_mm;
    let hits: Vec<(usize, f64)> = (0..tris.lifted.len())
        .into_par_iter()
        .filter_map(|i| {
            let d2 = index.nearest_sq(tris.lifted[i]);
            (d2 > r2).then_some((i, d2.sqrt()))
        })
        .collect();
    let mut unmachined = 0.0f64;
    let mut largest = 0.0f64;
    let mut distances: Vec<f64> = Vec::with_capacity(hits.len());
    for &(i, d) in &hits {
        unmachined += tris.areas[i];
        largest = largest.max(tris.areas[i]);
        distances.push(d);
    }
    CoverageAudit {
        centroids_tested: tris.lifted.len(),
        uncovered_centroid_triangles: hits.len(),
        unmachined_area_mm2: unmachined,
        unmachined_area_fraction: if tris.area_mm2 > 0.0 {
            unmachined / tris.area_mm2
        } else {
            0.0
        },
        region_area_mm2: tris.area_mm2,
        largest_unmachined_triangle_area_mm2: largest,
        coverage_radius_mm: cusp_radius_mm,
        uncovered_distance: summarise(&mut distances),
        ..CoverageAudit::default()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Run helpers (restated from H1)
// ═══════════════════════════════════════════════════════════════════════

fn pre_relink_fragments(toolpath: &Toolpath) -> usize {
    toolpath
        .moves
        .iter()
        .filter(|m| m.intent == MoveIntent::EntryPlunge)
        .count()
}

fn cut_runs(toolpath: &Toolpath) -> Vec<Vec<P3>> {
    let mut out: Vec<Vec<P3>> = Vec::new();
    let mut current: Vec<P3> = Vec::new();
    for i in 1..toolpath.moves.len() {
        let m = &toolpath.moves[i];
        let cutting = !matches!(m.move_type, MoveType::Rapid)
            && matches!(m.intent, MoveIntent::ClearingCut | MoveIntent::FinishingCut);
        if cutting {
            if current.is_empty() {
                current.push(toolpath.moves[i - 1].target);
            }
            current.push(m.target);
        } else if current.len() >= 2 {
            out.push(std::mem::take(&mut current));
        } else {
            current.clear();
        }
    }
    if current.len() >= 2 {
        out.push(current);
    }
    out
}

fn toolpath_from_runs(runs: &[Vec<P3>], safe_z: f64) -> Toolpath {
    let mut tp = Toolpath::new();
    for run in runs {
        if run.len() >= 2 {
            tp.emit_path_segment_with_intent(
                run,
                safe_z,
                FEED_MM_MIN,
                PLUNGE_MM_MIN,
                MoveIntent::FinishingCut,
            );
        }
    }
    tp
}

// ═══════════════════════════════════════════════════════════════════════
// SVG dump — so the operator can eyeball the arms
// ═══════════════════════════════════════════════════════════════════════

/// Write pass runs as one SVG (y flipped so +Y is up). `window` crops;
/// pass the full bbox for the whole board.
fn svg_dump(path: &std::path::Path, runs: &[Vec<P3>], window: [f64; 4], stroke: f64) {
    use std::fmt::Write as _;
    let [x0, y0, x1, y1] = window;
    let (w, h) = (x1 - x0, y1 - y0);
    let ph = 1000.0 * h / w.max(1e-9);
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {w:.1} {h:.1}\" \
         width=\"1000\" height=\"{ph:.0}\">\n\
         <rect width=\"100%\" height=\"100%\" fill=\"#101418\"/>\n\
         <g fill=\"none\" stroke=\"#7fd4ff\" stroke-width=\"{stroke}\" \
         stroke-linejoin=\"round\">\n"
    );
    for run in runs {
        let mut d = String::new();
        let mut pen_down = false;
        for pt in run {
            let inside = pt.x >= x0 && pt.x <= x1 && pt.y >= y0 && pt.y <= y1;
            if inside {
                let cmd = if pen_down { 'L' } else { 'M' };
                let _ = write!(d, "{cmd}{:.2},{:.2}", pt.x - x0, y1 - pt.y);
                pen_down = true;
            } else {
                pen_down = false;
            }
        }
        if !d.is_empty() {
            let _ = writeln!(svg, "<path d=\"{d}\"/>");
        }
    }
    svg.push_str("</g>\n</svg>\n");
    let _ = std::fs::write(path, svg);
}

// ═══════════════════════════════════════════════════════════════════════
// The instrument
// ═══════════════════════════════════════════════════════════════════════

#[test]
#[ignore = "evidence run — needs the operator's wanaka mesh (not in repo)"]
fn wanaka_graded_raster_e1() {
    eprintln!(
        "\n========== Avenue F, E1 — GRADED RASTER, COSTED ==========\n\
         Pre-registration: planning/metrology_2026-09-02/FINDINGS.md §M5\n\
         (bars E-spec / E-frag / E-time fixed before this run). NUMBERS ONLY.\n"
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
    let r10 = TaperedBallEndmill::new(2.0, 5.7, 6.0, 20.0);
    let never_cancel = || false;

    let (tier_map, cusp_radii) = {
        let tools: [&dyn MillingCutter; 2] = [&r15, &r10];
        let ladder = TierLadder::new(&tools).expect("ladder");
        let map = compute_tier_map(
            &mesh,
            &index,
            &ladder,
            &TierMapParams {
                cell_mm: CELL_MM,
                tolerance_mm: TOLERANCE_MM,
                margin_mm: MARGIN_MM,
                treatment: ResidualTreatment::SlopeCompensated,
            },
            &never_cancel,
        )
        .expect("tier map");
        let radii: Vec<f64> = tools.iter().map(|t| t.cusp_radius_mm()).collect();
        (map, radii)
    };
    let islands = extract_tier_islands(
        &tier_map,
        &TierIslandParams {
            coarseness: COARSENESS,
            overlap_mm: OVERLAP_MM,
            max_regions_per_tier: MAX_REGIONS_PER_TIER,
            ..TierIslandParams::default()
        },
        &cusp_radii,
    )
    .expect("islands");
    let Some(fine_island) = islands.per_tier.iter().find(|set| set.tier == 1) else {
        eprintln!("SKIP: no tier 1 machining region.");
        return;
    };

    let surface = build_classification_surface_with_sampler_and_cancel(
        &mesh,
        &index,
        &r10,
        unified_finish_classification_resolution(&r10, OP_TOLERANCE_MM),
        ClassificationSampler::PRODUCTION,
        &never_cancel,
    )
    .expect("classification surface");
    let heightmap = &surface.heightmap;
    let covered: Vec<bool> = heightmap
        .covered_flags()
        .iter()
        .enumerate()
        .map(|(i, &flag)| {
            if !flag {
                return false;
            }
            let row = i / heightmap.cols;
            let col = i % heightmap.cols;
            fine_island.machining.contains(&P2::new(
                heightmap.origin_x + col as f64 * heightmap.cell_size,
                heightmap.origin_y + row as f64 * heightmap.cell_size,
            ))
        })
        .collect();

    let mut planner = FinishPlannerParams::for_tool(cusp_radii[1]);
    planner.overlap_mm = OVERLAP_MM;
    let planned = decompose(&surface.slope_map, &covered, &[], &planner);
    let clamp_deg = planner.steep_threshold_deg;

    let slope = &surface.slope_map;
    let rows = slope.rows;
    let cols = slope.cols;
    let cell = slope.cell_size;
    let cell_area = cell * cell;
    let theta_deg: Vec<f64> = slope
        .angles
        .iter()
        .map(|a| a.to_degrees().min(clamp_deg))
        .collect();

    let mut shallow: Vec<&Polygon2> = planned
        .regions
        .iter()
        .filter(|r| r.band == FinishBand::Shallow)
        .map(|r| &r.polygon)
        .collect();
    shallow.sort_by(|a, b| b.area().total_cmp(&a.area()));

    // region_of + production theta_max per region (the F1 construction).
    let mut region_of: Vec<Option<usize>> = vec![None; rows * cols];
    for (ridx, poly) in shallow.iter().enumerate() {
        let [bx0, by0, bx1, by1] = poly.bbox();
        let c0 = (((bx0 - slope.origin_x) / cell).floor().max(0.0)) as usize;
        let c1 = ((((bx1 - slope.origin_x) / cell).ceil()) as usize).min(cols - 1);
        let r0 = (((by0 - slope.origin_y) / cell).floor().max(0.0)) as usize;
        let r1 = ((((by1 - slope.origin_y) / cell).ceil()) as usize).min(rows - 1);
        for row in r0..=r1 {
            for col in c0..=c1 {
                let i = row * cols + col;
                if region_of[i].is_some() {
                    continue;
                }
                let p = P2::new(
                    slope.origin_x + col as f64 * cell,
                    slope.origin_y + row as f64 * cell,
                );
                if poly.contains_point(&p) {
                    region_of[i] = Some(ridx);
                }
            }
        }
    }
    let mut theta_max_region = vec![0.0_f64; shallow.len()];
    for i in 0..rows * cols {
        if covered[i]
            && let Some(r) = region_of[i]
        {
            theta_max_region[r] = theta_max_region[r].max(theta_deg[i]);
        }
    }

    // Band assignment over ALL in-polygon covered cells (arm B's split
    // grid), F1 merge applied.

    let stepover = equal_cusp_stepover_mm(cusp_radii[1], CUSP_HEIGHT_MM);
    let kinematics = MachineKinematics {
        acceleration_mm_s2: MACHINE_ACCEL_SCALAR,
        acceleration_xyz_mm_s2: Some(MACHINE_ACCEL_XYZ),
        junction_deviation_mm: JUNCTION_DEVIATION_MM,
        ..MachineKinematics::default()
    };
    let safe_z = mesh.bbox.max.z + 5.0;
    let effective_min_z = mesh.bbox.min.z - 0.1;
    let block_top_z = mesh.bbox.max.z;
    let stock = machined_stock(
        &mesh,
        &index,
        &r15,
        &tier_map,
        [
            mesh.bbox.min.x,
            mesh.bbox.min.y,
            mesh.bbox.max.x,
            mesh.bbox.max.y,
        ],
    );
    let regime = LinkRegime {
        safe_z,
        ceiling: Some(LinkCeiling {
            stock: Some(&stock),
            tool_radius: r10.envelope_radius_mm(),
            fallback_top_z: block_top_z,
        }),
        flush_ride: true,
        airborne: true,
    };
    eprintln!(
        "setup: {} Shallow regions, stepover {stepover:.4} mm, spec gap tolerance \
         {SPEC_GAP_TOLERANCE}x, {:.0} s",
        shallow.len(),
        started.elapsed().as_secs_f64()
    );

    // Drop-cutter grids by step (0° lattice for both arms), built up front
    // over the union of the region steps and the band steps.
    let mut steps: Vec<f64> = shallow
        .iter()
        .enumerate()
        .map(|(ridx, _)| stepover * theta_max_region[ridx].to_radians().cos())
        .collect();
    steps.sort_by(f64::total_cmp);
    steps.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
    let grids: Vec<(f64, rs_cam_core::surface::dropcutter::DropCutterGrid)> = steps
        .iter()
        .map(|&step| {
            (
                step,
                rs_cam_core::surface::dropcutter::batch_drop_cutter(
                    &mesh,
                    &index,
                    &r10,
                    step,
                    0.0,
                    effective_min_z,
                ),
            )
        })
        .collect();
    let grid_for = |step: f64| -> usize {
        grids
            .iter()
            .position(|(s, _)| (s - step).abs() < 1e-9)
            .expect("grid built for every step")
    };

    // ── arm S: per-region raster at the production derate ────────────────
    let t0 = std::time::Instant::now();
    let mut runs_s: Vec<Vec<P3>> = Vec::new();
    for (ridx, poly) in shallow.iter().enumerate() {
        let step = stepover * theta_max_region[ridx].to_radians().cos();
        let gi = grid_for(step);
        let raw = raster_candidate(
            &grids[gi].1,
            std::slice::from_ref(*poly),
            safe_z,
            effective_min_z,
        );
        runs_s.extend(cut_runs(&raw));
    }
    eprintln!(
        "arm S built: {} runs, {} grids, {:.0} s",
        runs_s.len(),
        grids.len(),
        t0.elapsed().as_secs_f64()
    );

    // ── arm E family: graded rasters at horizontal erosion W (E1b sweep) ──
    let t1 = std::time::Instant::now();
    let cell_at = |x: f64, y: f64| -> Option<usize> {
        let col = ((x - slope.origin_x) / cell).round();
        let row = ((y - slope.origin_y) / cell).round();
        if col < 0.0 || row < 0.0 {
            return None;
        }
        let (col, row) = (col as usize, row as usize);
        if row >= rows || col >= cols {
            return None;
        }
        Some(row * cols + col)
    };
    let all_polys: Vec<Polygon2> = shallow.iter().map(|p| (*p).clone()).collect();
    let territory_set = RegionSet::new(all_polys.clone());
    let in_territory: Vec<bool> = (0..rows * cols)
        .map(|i| covered[i] && region_of[i].is_some())
        .collect();
    let union_cells = in_territory.iter().filter(|&&t| t).count();
    eprintln!(
        "  territory: {union_cells} in-polygon covered cells ({:.0} mm² XY)",
        union_cells as f64 * cell_area
    );
    let clamp_a = stepover * clamp_deg.to_radians().cos();
    // Horizontal decorrelation census: how far is flat ground from steep
    // ground? This is the length scale ANY smoothing window must stay
    // under to preserve the refund.
    {
        let steepish: Vec<bool> = (0..rows * cols)
            .map(|i| in_territory[i] && theta_deg[i] >= 40.0)
            .collect();
        let dt = distance_transform_2d(&steepish, rows, cols);
        let mut d_flat: Vec<f64> = (0..rows * cols)
            .filter(|&i| in_territory[i] && theta_deg[i] <= 20.0)
            .map(|i| dt[i] * cell)
            .collect();
        d_flat.sort_by(f64::total_cmp);
        let q = |f: f64| d_flat[((d_flat.len() - 1) as f64 * f).round() as usize];
        eprintln!(
            "  decorrelation census: distance from flat (θ ≤ 20°) cells to steep (θ ≥ 40°) \
             ground:\n    p50 {:.2} mm, p90 {:.2} mm, p99 {:.2} mm over {} flat cells",
            q(0.5),
            q(0.9),
            q(0.99),
            d_flat.len()
        );
    }
    let a_raw: Vec<f64> = theta_deg
        .iter()
        .map(|t| stepover * t.to_radians().cos())
        .collect();
    let erode_y = ((stepover / 2.0) / cell).ceil() as usize;

    // Fine z grid for the passes (the H1 convention: nearest lookup).
    let zgrid = rs_cam_core::surface::dropcutter::batch_drop_cutter(
        &mesh,
        &index,
        &r10,
        CELL_MM,
        0.0,
        effective_min_z,
    );
    let run_len_2d = |runs: &[Vec<P3>]| -> f64 {
        runs.iter()
            .map(|r| {
                r.windows(2)
                    .map(|w| (w[1].x - w[0].x).hypot(w[1].y - w[0].y))
                    .sum::<f64>()
            })
            .sum()
    };

    // One graded-raster arm at horizontal erosion w_mm. Returns the pass
    // runs plus (tilt %, spec exceed %, worst gap ratio).
    let build_graded = |w_mm: f64| -> (Vec<Vec<P3>>, f64, f64, f64) {
        // Horizontal min-erosion by ±w, IN-TERRITORY cells only.
        let ew = ((w_mm / cell).round() as usize).min(cols.saturating_sub(1));
        let mut a_x = a_raw.clone();
        if ew > 0 {
            for row in 0..rows {
                for col in 0..cols {
                    let lo = col.saturating_sub(ew);
                    let hi = (col + ew).min(cols - 1);
                    let mut m = f64::INFINITY;
                    for c in lo..=hi {
                        let i = row * cols + c;
                        if in_territory[i] {
                            m = m.min(a_raw[i]);
                        }
                    }
                    if m.is_finite() {
                        a_x[row * cols + col] = m;
                    }
                }
            }
        }
        // Vertical min-erosion by ±stepover/2 (the M5 conservative window).
        let mut a_er = a_x.clone();
        for col in 0..cols {
            for row in 0..rows {
                let lo = row.saturating_sub(erode_y);
                let hi = (row + erode_y).min(rows - 1);
                let mut m = f64::INFINITY;
                for r in lo..=hi {
                    let i = r * cols + col;
                    if in_territory[i] {
                        m = m.min(a_x[i]);
                    }
                }
                a_er[row * cols + col] = if m.is_finite() { m } else { clamp_a };
            }
        }
        // Column-monotone field and its integer level curves.
        let mut phi = vec![0.0_f64; rows * cols];
        for col in 0..cols {
            let mut acc = 0.0_f64;
            for row in 0..rows {
                let i = row * cols + col;
                let rate = if in_territory[i] { a_er[i] } else { clamp_a };
                acc += cell / rate;
                phi[i] = acc;
            }
        }
        let max_phi = phi.iter().cloned().fold(0.0_f64, f64::max);
        let n_levels = max_phi.floor() as usize;
        let mut levels: Vec<Vec<Option<f64>>> = vec![vec![None; cols]; n_levels + 1];
        for col in 0..cols {
            let mut k = 1usize;
            for row in 1..rows {
                let p0 = phi[(row - 1) * cols + col];
                let p1 = phi[row * cols + col];
                while k <= n_levels && (k as f64) <= p1 {
                    if (k as f64) > p0 {
                        let t = ((k as f64) - p0) / (p1 - p0).max(1e-12);
                        let y = slope.origin_y + ((row - 1) as f64 + t) * cell;
                        levels[k][col] = Some(y);
                    }
                    k += 1;
                }
            }
        }
        // Pass segments per (region, level): keep in-territory points
        // (polygon containment — arm S's clipping convention), split at
        // gaps AND at region changes, so the serpentine below chains
        // within one region only, exactly like arm S's per-polygon
        // rasters.
        let n_regions = shallow.len();
        let mut per_region: Vec<Vec<(usize, Vec<P3>)>> = vec![Vec::new(); n_regions];
        let mut tilt_len = 0.0_f64;
        let mut flat_len = 0.0_f64;
        for (kl, lvl) in levels.iter().enumerate().skip(1) {
            let mut current: Vec<P3> = Vec::new();
            let mut current_region: Option<usize> = None;
            let mut flush = |current: &mut Vec<P3>, region: Option<usize>| {
                if current.len() >= 2
                    && let Some(r) = region
                {
                    per_region[r].push((kl, std::mem::take(current)));
                } else {
                    current.clear();
                }
            };
            for (col, slot) in lvl.iter().enumerate() {
                let x = slope.origin_x + col as f64 * cell;
                let keep = slot.and_then(|y| {
                    if !territory_set.contains(&P2::new(x, y)) {
                        return None;
                    }
                    let region = cell_at(x, y).and_then(|i| region_of[i])?;
                    let z = bilinear_contact_z(&zgrid, x, y, effective_min_z)?;
                    Some((P3::new(x, y, z), region))
                });
                match keep {
                    Some((p, region)) => {
                        if current_region != Some(region) {
                            let prev_region = current_region;
                            flush(&mut current, prev_region);
                            current_region = Some(region);
                        }
                        if let Some(prev) = current.last() {
                            tilt_len += (p.x - prev.x).hypot(p.y - prev.y);
                            flat_len += (p.x - prev.x).abs();
                        }
                        current.push(p);
                    }
                    None => {
                        let prev_region = current_region;
                        flush(&mut current, prev_region);
                        current_region = None;
                    }
                }
            }
            let prev_region = current_region;
            flush(&mut current, prev_region);
        }
        // Serpentine chaining per region — the raster emitter's stay-down
        // turnaround (`raster_toolpath_from_grid`'s one-diagonal link),
        // restated so arm E's fragments compare like for like with arm S.
        let link_max = cell.hypot(stepover) * 1.05;
        let mut runs_e: Vec<Vec<P3>> = Vec::new();
        for segs in &per_region {
            let mut chained: Vec<Vec<P3>> = Vec::new();
            let mut idx = 0usize;
            while idx < segs.len() {
                let level = segs[idx].0;
                let mut level_segs: Vec<Vec<P3>> = Vec::new();
                while idx < segs.len() && segs[idx].0 == level {
                    level_segs.push(segs[idx].1.clone());
                    idx += 1;
                }
                if level % 2 == 1 {
                    level_segs.reverse();
                    for seg in &mut level_segs {
                        seg.reverse();
                    }
                }
                for seg in level_segs {
                    let join = chained.last().is_some_and(|prev: &Vec<P3>| {
                        let a = prev[prev.len() - 1];
                        let b = seg[0];
                        (a.x - b.x).hypot(a.y - b.y) <= link_max
                    });
                    if join {
                        if let Some(prev) = chained.last_mut() {
                            prev.extend(seg);
                        }
                    } else {
                        chained.push(seg);
                    }
                }
            }
            runs_e.extend(chained);
        }
        // E-spec: per column, adjacent-pass vertical gap vs eroded-min a.
        let mut gaps = 0usize;
        let mut exceed = 0usize;
        let mut worst_ratio = 0.0_f64;
        for col in 0..cols {
            let mut prev_y: Option<f64> = None;
            for lvl in levels.iter().skip(1) {
                let Some(y) = lvl[col] else { continue };
                if let Some(y0) = prev_y {
                    let mid = cell_at(slope.origin_x + col as f64 * cell, (y0 + y) / 2.0);
                    if let Some(im) = mid
                        && in_territory[im]
                    {
                        let r0 = (((y0 - slope.origin_y) / cell).floor().max(0.0)) as usize;
                        let r1 = ((((y - slope.origin_y) / cell).ceil()) as usize).min(rows - 1);
                        let mut amin = f64::INFINITY;
                        for r in r0..=r1 {
                            let i = r * cols + col;
                            if in_territory[i] {
                                amin = amin.min(a_er[i]);
                            }
                        }
                        if amin.is_finite() {
                            gaps += 1;
                            let ratio = (y - y0) / amin;
                            worst_ratio = worst_ratio.max(ratio);
                            if ratio > SPEC_GAP_TOLERANCE {
                                exceed += 1;
                            }
                        }
                    }
                }
                prev_y = Some(y);
            }
        }
        let tilt_pct = 100.0 * (tilt_len / flat_len.max(1e-9) - 1.0);
        let exceed_pct = 100.0 * exceed as f64 / gaps.max(1) as f64;
        (runs_e, tilt_pct, exceed_pct, worst_ratio)
    };

    const W_SWEEP: [f64; 6] = [0.0, 1.0, 2.0, 4.0, 8.0, 16.0];
    let mut e_arms: Vec<(String, Vec<Vec<P3>>, f64, f64)> = Vec::new();
    for w in W_SWEEP {
        let (runs, tilt_pct, exceed_pct, worst) = build_graded(w);
        eprintln!(
            "  E(W={w:>4.1} mm): {:>5} runs, 2D {:>6.0} mm (S {:.0}), tilt +{tilt_pct:>6.2} %, \
             E-spec exceed {exceed_pct:>5.2} % (worst {worst:.3})",
            runs.len(),
            run_len_2d(&runs),
            run_len_2d(&runs_s),
        );
        e_arms.push((format!("E W={w:.0}mm graded"), runs, exceed_pct, tilt_pct));
    }
    eprintln!("  arm E family built, {:.0} s", t1.elapsed().as_secs_f64());

    // ── SVG dump: arm S plus three W settings, full board + a crop on the
    //    largest Shallow region (mixed flat/gully ground) ─────────────────
    {
        let out = std::path::Path::new("target/graded_raster_e1");
        let _ = std::fs::create_dir_all(out);
        let board = [
            mesh.bbox.min.x,
            mesh.bbox.min.y,
            mesh.bbox.max.x,
            mesh.bbox.max.y,
        ];
        let [bx0, by0, bx1, by1] = shallow[0].bbox();
        let (cx, cy) = ((bx0 + bx1) / 2.0, (by0 + by1) / 2.0);
        let crop = [cx - 20.0, cy - 20.0, cx + 20.0, cy + 20.0];
        let dumps: [(&str, &Vec<Vec<P3>>); 4] = [
            ("arm_S_raster", &runs_s),
            ("arm_E_W0", &e_arms[0].1),
            ("arm_E_W2", &e_arms[2].1),
            ("arm_E_W8", &e_arms[4].1),
        ];
        for (name, runs) in dumps {
            svg_dump(&out.join(format!("{name}.svg")), runs, board, 0.06);
            svg_dump(&out.join(format!("{name}_crop.svg")), runs, crop, 0.05);
        }
        eprintln!("  SVGs written to target/graded_raster_e1/ (crop at [{crop:?}])");
    }

    // ── cost both arms identically ───────────────────────────────────────
    let boundary = RegionSet::new(all_polys);
    let tris_all = union_triangles(&mesh, &shallow, CUSP_HEIGHT_MM);
    let tris = tris_all.below(clamp_deg);
    let mut bbox = [
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    ];
    for poly in &shallow {
        let [x0, y0, x1, y1] = poly.bbox();
        bbox[0] = bbox[0].min(x0 - 5.0);
        bbox[1] = bbox[1].min(y0 - 5.0);
        bbox[2] = bbox[2].max(x1 + 5.0);
        bbox[3] = bbox[3].max(y1 + 5.0);
    }
    eprintln!(
        "\ncosting under the machined-stock ceiling.\n\
         territory: {} triangles / {:.0} mm² 3D total; the COVERAGE GATE reads the\n\
         sub-clamp (≤ {clamp_deg:.0}°) population only — {} triangles / {:.0} mm² — because\n\
         ground above the clamp is uncoverable at spec by EVERY raster arm (V1/G2).\n",
        tris_all.lifted.len(),
        tris_all.area_mm2,
        tris.lifted.len(),
        tris.area_mm2
    );
    eprintln!(
        "     {:>34}  {:>8}  {:>9}  {:>7}  {:>8}  {:>9}  {:>9}",
        "arm", "pre-frag", "fragments", "linked", "retracts", "time s", "cut mm"
    );
    let mut costs: Vec<(String, CandidateCost, CoverageAudit)> = Vec::new();
    let mut arm_list: Vec<(String, &Vec<Vec<P3>>)> =
        vec![("S   shipped per-region derate".to_owned(), &runs_s)];
    for (label, runs, _, _) in &e_arms {
        arm_list.push((label.clone(), runs));
    }
    for (label, runs) in &arm_list {
        let raw = toolpath_from_runs(runs, safe_z);
        let pre = pre_relink_fragments(&raw);
        let cost = relink_and_cost_under(raw, &mesh, &index, &r10, &boundary, &kinematics, &regime);
        let coverage = audit_arm_coverage(&tris, &cost.toolpath, r10.cusp_radius_mm(), bbox);
        eprintln!(
            "     {:>34}  {:>8}  {:>9}  {:>7}  {:>8}  {:>9.1}  {:>9.0}",
            label,
            pre,
            cost.fragments,
            cost.linked,
            cost.kept_retracts,
            cost.time_s,
            cost.cutting_mm
        );
        costs.push((label.to_owned(), cost, coverage));
    }
    eprintln!(
        "\n     {:>34}  {:>14}  {:>12}  {:>14}  {:>13}  gate (≤ {:.1} %)",
        "arm",
        "uncovered tri",
        "unmach mm²",
        "unmach frac %",
        "largest mm²",
        100.0 * COVERAGE_FAIL_FRACTION
    );
    for (label, _, c) in &costs {
        let gate = if c.unmachined_area_fraction <= COVERAGE_FAIL_FRACTION {
            "ok"
        } else {
            "FAILS COVERAGE"
        };
        eprintln!(
            "     {:>34}  {:>14}  {:>12.3}  {:>13.4}%  {:>13.5}  {gate}",
            label,
            c.uncovered_centroid_triangles,
            c.unmachined_area_mm2,
            100.0 * c.unmachined_area_fraction,
            c.largest_unmachined_triangle_area_mm2
        );
    }
    let s = &costs[0];
    let (best_i, _) = costs
        .iter()
        .enumerate()
        .skip(1)
        .min_by(|a, b| a.1.1.time_s.total_cmp(&b.1.1.time_s))
        .expect("at least one E arm");
    let e = &costs[best_i];
    let e_spec_pct = e_arms[best_i - 1].2;
    eprintln!(
        "\n---------- the comparison the bars read (best-time E arm: {}) ----------\n\
         \x20  E-time:  E {:.1} s vs S {:.1} s = {:.3}x  (bar: < 0.95x)\n\
         \x20  E-frag:  E {} vs S {} fragments = {:.3}x  (bar: <= 1.10x)\n\
         \x20  E-spec:  exceed {:.2} %  (bar: <= 5 %)\n\
         \x20  cut mm:  E {:.0} vs S {:.0} = {:.3}x\n\
         \x20  coverage: E {:.4} % vs S {:.4} % unmachined (knife-edge caveat, M4)",
        e.0,
        e.1.time_s,
        s.1.time_s,
        e.1.time_s / s.1.time_s,
        e.1.fragments,
        s.1.fragments,
        e.1.fragments as f64 / s.1.fragments.max(1) as f64,
        e_spec_pct,
        e.1.cutting_mm,
        s.1.cutting_mm,
        e.1.cutting_mm / s.1.cutting_mm,
        100.0 * e.2.unmachined_area_fraction,
        100.0 * s.2.unmachined_area_fraction,
    );
    eprintln!("\ntotal wall time {:.0} s", started.elapsed().as_secs_f64());
}
