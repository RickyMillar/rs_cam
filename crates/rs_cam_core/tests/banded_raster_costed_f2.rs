//! Avenue F, phase F2 — the banded raster vs the shipped derate, COSTED
//! (`planning/metrology_2026-09-02/FINDINGS.md` §M4, pre-registered).
//!
//! F1 showed a K = 3 slope banding captures 71.9 % of the spacing prize
//! on spacing alone. F2 prices what F1 excluded: fragments and links —
//! the mechanism that killed the V1 tracing arm. Two arms, whole Shallow
//! band, costed identically on the H1 harness:
//!
//! * **arm S** — per-region raster at the production derate (θ_max over
//!   covered in-polygon cells, clamped 45°).
//! * **arm B** — the same raster over the same polygons, split by the
//!   K = 3 slope bands (edges 20°/40°/45°), sub-machinable components
//!   (< `min_region_area_mm2`) merged into the steeper band first. A
//!   point with no assignment takes the band of its own clamped θ, so
//!   arm B's territory is arm S's exactly.
//!
//! Both arms use this instrument's 0° lattice: the shipped C2 rotation
//! is orthogonal to the spacing question and cancels between arms. Spec
//! per cell holds by construction — a cell is only ever cut at its own
//! band's spacing or tighter (merges go steeper, never flatter).
//!
//! Costing, link regime, coverage audit, and helper definitions are
//! restated from `valley_branch_falsifier_h1.rs` (an integration test
//! cannot `use` another one); setup through `decompose` is verbatim
//! `band_cell_ownership_g2.rs`.
//!
//! NUMBERS ONLY — the B-time ruling goes to the FINDINGS file.
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
use rs_cam_core::classify_probe::ClassificationSampler;
use rs_cam_core::conformal_spiral::{CoverageAudit, DistanceStats};
use rs_cam_core::dexel_stock::TriDexelStock;
use rs_cam_core::finish_planner::{FinishBand, FinishPlannerParams, decompose};
use rs_cam_core::finish_setup::build_classification_surface_with_sampler_and_cancel;
use rs_cam_core::geo::{P2, P3};
use rs_cam_core::geometry::grid_field::distance_transform_2d;
use rs_cam_core::geometry::region_set::RegionSet;
use rs_cam_core::machine_kinematics::MachineKinematics;
use rs_cam_core::maps::tier_islands::{TierIslandParams, extract_tier_islands};
use rs_cam_core::maps::tier_map::{
    ResidualTreatment, TierLadder, TierMap, TierMapParams, compute_tier_map,
};
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::surface_link::LinkCeiling;
use rs_cam_core::tool::{MillingCutter, TaperedBallEndmill};
use rs_cam_core::toolpath::{MoveIntent, MoveType, Toolpath};
use rs_cam_core::unified_finish::unified_finish_classification_resolution;

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

/// The F1 K = 3 ladder (degrees; last edge is the clamp).
const BAND_EDGES: [f64; 3] = [20.0, 40.0, 45.0];
/// Robustness arm (report-only, not a pre-registered bar): the K = 2
/// ladder, half the band boundaries — bounds how much of the fragment
/// bill is boundary count.
const BAND_EDGES_K2: [f64; 2] = [30.0, 45.0];

/// The H1 coverage gate.
const COVERAGE_FAIL_FRACTION: f64 = 0.01;
/// Coverage-equality margin (pre-registered): arm B's unmachined fraction
/// may exceed arm S's by at most this much.
const COVERAGE_EQUALITY_MARGIN: f64 = 0.005;

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
    use rs_cam_core::machine_kinematics::{LinkKinematics, compute_cycle_time};

    let link_kinematics = LinkKinematics {
        kinematics: *kinematics,
        max_feed_mm_min: MAX_FEED_MM_MIN,
        rapid_feed_mm_min: RAPID_FEED_MM_MIN,
    };
    let params = rs_cam_core::surface_link::RelinkParams {
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
    let (linked, report) = rs_cam_core::surface_link::relink_fragments(
        rs_cam_core::toolpath_spans::AnnotatedToolpath::new(raw),
        mesh,
        index,
        cutter,
        &params,
    );
    let mut channels = rs_cam_core::transform_provenance::ReconcileSet::new(None, None);
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
    let distance_to_tier_zero = distance_transform_2d(&tier_zero, tier_map.ny, tier_map.nx);
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
                distance_to_tier_zero[r * tier_map.nx + c] * tier_map.cell_mm <= reach
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

fn split_runs(runs: &[Vec<P3>], keep: &dyn Fn(P3) -> bool) -> Vec<Vec<P3>> {
    let mut out = Vec::new();
    for run in runs {
        let mut current: Vec<P3> = Vec::new();
        for &p in run {
            if keep(p) {
                current.push(p);
            } else if current.len() >= 2 {
                out.push(std::mem::take(&mut current));
            } else {
                current.clear();
            }
        }
        if current.len() >= 2 {
            out.push(current);
        }
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
// Band assignment (the F1 merge, restated)
// ═══════════════════════════════════════════════════════════════════════

/// Merge every 8-connected component of `band` cells below `min_area_mm2`
/// into `band + 1`. Returns the merged area (mm²).
fn merge_submachinable(
    band_assign: &mut [Option<usize>],
    rows: usize,
    cols: usize,
    band: usize,
    cell_area: f64,
    min_area_mm2: f64,
) -> f64 {
    let mut visited = vec![false; rows * cols];
    let mut moved_mm2 = 0.0_f64;
    let mut stack: Vec<usize> = Vec::new();
    let mut comp: Vec<usize> = Vec::new();
    for start in 0..rows * cols {
        if visited[start] || band_assign[start] != Some(band) {
            continue;
        }
        comp.clear();
        stack.push(start);
        visited[start] = true;
        while let Some(i) = stack.pop() {
            comp.push(i);
            let (r, c) = (i / cols, i % cols);
            for dr in -1i64..=1 {
                for dc in -1i64..=1 {
                    if dr == 0 && dc == 0 {
                        continue;
                    }
                    let nr = r as i64 + dr;
                    let nc = c as i64 + dc;
                    if nr < 0 || nc < 0 || nr >= rows as i64 || nc >= cols as i64 {
                        continue;
                    }
                    let ni = (nr as usize) * cols + nc as usize;
                    if !visited[ni] && band_assign[ni] == Some(band) {
                        visited[ni] = true;
                        stack.push(ni);
                    }
                }
            }
        }
        let area = comp.len() as f64 * cell_area;
        if area < min_area_mm2 {
            moved_mm2 += area;
            for &i in &comp {
                band_assign[i] = Some(band + 1);
            }
        }
    }
    moved_mm2
}

// ═══════════════════════════════════════════════════════════════════════
// The instrument
// ═══════════════════════════════════════════════════════════════════════

#[test]
#[ignore = "evidence run — needs the operator's wanaka mesh (not in repo)"]
fn wanaka_banded_raster_costed_f2() {
    eprintln!(
        "\n========== Avenue F, F2 — BANDED RASTER, COSTED ==========\n\
         Pre-registration: planning/metrology_2026-09-02/FINDINGS.md §M4\n\
         (bar B-time and the coverage precondition fixed before this run).\n\
         NUMBERS ONLY.\n"
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
    let min_region_area_mm2 = planner.min_region_area_mm2;

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
    let assign_for = |edges: &[f64]| -> (Vec<Option<usize>>, f64) {
        let band_of_theta = |t: f64| -> usize {
            edges
                .iter()
                .position(|&e| t <= e)
                .unwrap_or(edges.len() - 1)
        };
        let mut band_assign: Vec<Option<usize>> = (0..rows * cols)
            .map(|i| (covered[i] && region_of[i].is_some()).then(|| band_of_theta(theta_deg[i])))
            .collect();
        let mut merged_mm2 = 0.0;
        for b in 0..edges.len() - 1 {
            merged_mm2 += merge_submachinable(
                &mut band_assign,
                rows,
                cols,
                b,
                cell_area,
                min_region_area_mm2,
            );
        }
        (band_assign, merged_mm2)
    };
    let (band_assign, merged_mm2) = assign_for(&BAND_EDGES);
    let (band_assign_k2, merged_mm2_k2) = assign_for(&BAND_EDGES_K2);

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
        "setup: {} Shallow regions, stepover {stepover:.4} mm, bands {BAND_EDGES:?} \
         (merged {merged_mm2:.0} mm2) + K2 {BAND_EDGES_K2:?} (merged {merged_mm2_k2:.0} mm2), \
         {:.0} s",
        shallow.len(),
        started.elapsed().as_secs_f64()
    );

    // Drop-cutter grids by step (0° lattice for both arms), built up front
    // over the union of the region steps and the band steps.
    let mut steps: Vec<f64> = shallow
        .iter()
        .enumerate()
        .map(|(ridx, _)| stepover * theta_max_region[ridx].to_radians().cos())
        .chain(
            BAND_EDGES
                .iter()
                .chain(BAND_EDGES_K2.iter())
                .map(|edge| stepover * edge.to_radians().cos()),
        )
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

    // ── arm B: banded raster over the SAME polygons ──────────────────────
    // A point with no band assignment takes the band of its own clamped θ,
    // so arm B's kept territory is arm S's territory exactly.
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
    let band_at_for = |assign: &[Option<usize>], edges: &[f64], p: P3| -> usize {
        match cell_at(p.x, p.y) {
            Some(i) => assign[i].unwrap_or_else(|| {
                edges
                    .iter()
                    .position(|&e| theta_deg[i] <= e)
                    .unwrap_or(edges.len() - 1)
            }),
            None => edges.len() - 1,
        }
    };
    let all_polys: Vec<Polygon2> = shallow.iter().map(|p| (*p).clone()).collect();
    let union_cells = (0..rows * cols)
        .filter(|&i| covered[i] && region_of[i].is_some())
        .count();
    let mut band_cells = vec![0usize; BAND_EDGES.len()];
    for i in 0..rows * cols {
        if covered[i]
            && region_of[i].is_some()
            && let Some(b) = band_assign[i]
        {
            band_cells[b] += 1;
        }
    }
    eprintln!(
        "  union in-polygon covered cells: {union_cells} ({:.0} mm² XY); per-band {:?}",
        union_cells as f64 * cell_area,
        band_cells
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
    let build_banded = |assign: &[Option<usize>], edges: &[f64], label: &str| -> Vec<Vec<P3>> {
        let mut runs: Vec<Vec<P3>> = Vec::new();
        for (b, &edge) in edges.iter().enumerate() {
            let step = stepover * edge.to_radians().cos();
            let gi = grid_for(step);
            let raw = raster_candidate(&grids[gi].1, &all_polys, safe_z, effective_min_z);
            let full = cut_runs(&raw);
            let kept = split_runs(&full, &|p: P3| band_at_for(assign, edges, p) == b);
            let n = kept.len();
            let kept_2d = run_len_2d(&kept);
            eprintln!(
                "  {label} band <={edge:.0} deg: step {step:.4} mm, {n} runs kept, 2D length \
                 {kept_2d:.0} mm"
            );
            runs.extend(kept);
        }
        runs
    };
    let runs_b = build_banded(&band_assign, &BAND_EDGES, "K3");
    let runs_b2 = build_banded(&band_assign_k2, &BAND_EDGES_K2, "K2");
    let _ = band_cells;
    eprintln!(
        "  arm S 2D length for comparison: {:.0} mm",
        run_len_2d(&runs_s)
    );
    eprintln!(
        "arm B built: {} runs, {} grids total, {:.0} s",
        runs_b.len(),
        grids.len(),
        t1.elapsed().as_secs_f64()
    );

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
    for (label, runs) in [
        ("S   shipped per-region derate", &runs_s),
        ("B   K=3 banded (20/40/45)", &runs_b),
        ("B2  K=2 banded (30/45), report-only", &runs_b2),
    ] {
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
    let b = &costs[1];
    eprintln!(
        "\n---------- the comparison the bars read ----------\n\
         \x20  time:      B {:.1} s vs S {:.1} s = {:.3}x  (B-time bar: < 0.95x)\n\
         \x20  cut mm:    B {:.0} vs S {:.0} = {:.3}x  (F1 predicted ~0.81x)\n\
         \x20  coverage:  B {:.4} % vs S {:.4} % unmachined (equality margin {:.1} pp)\n\
         \x20  fragments: B {} vs S {}   links: B {} vs S {}",
        b.1.time_s,
        s.1.time_s,
        b.1.time_s / s.1.time_s,
        b.1.cutting_mm,
        s.1.cutting_mm,
        b.1.cutting_mm / s.1.cutting_mm,
        100.0 * b.2.unmachined_area_fraction,
        100.0 * s.2.unmachined_area_fraction,
        100.0 * COVERAGE_EQUALITY_MARGIN,
        b.1.fragments,
        s.1.fragments,
        b.1.linked,
        s.1.linked,
    );
    eprintln!("\ntotal wall time {:.0} s", started.elapsed().as_secs_f64());
}
