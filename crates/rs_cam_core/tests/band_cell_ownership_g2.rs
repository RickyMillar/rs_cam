//! Avenue G, phase G2 — the two-hypothesis attribution from Track H V1
//! finding 2 (`planning/valley_tracing_2026-09-02/FINDINGS.md`).
//!
//! # The question this decides
//!
//! Track H's V1 coverage audit read ~47 % of the largest Shallow region
//! polygon unmachined at spec, and the slope census attributed it: 42.90 %
//! of the polygon's 3D area is steeper than the 45° derate clamp — ground
//! a Shallow raster cannot finish to spec BY CONSTRUCTION. Two hypotheses
//! were recorded, unresolved:
//!
//! * **(a)** the region polygons include ground the planner's own CELL
//!   labels do not assign to the Shallow band — the polygon, not the
//!   classification, is the lying surface; or
//! * **(b)** the planner's slope classification disagrees with the
//!   true-surface (mesh-triangle) census — a planner classification defect.
//!
//! # What it measures
//!
//! It rebuilds the production decomposition EXACTLY as
//! `valley_branch_falsifier_h1.rs` does (same mesh, same tier-1 island
//! coverage clip, same `ClassificationSampler::PRODUCTION` surface, same
//! `FinishPlannerParams::for_tool`), then reads the planner's
//! post-conditioning label grid (`PlannedRegions::labels`, exposed for
//! this instrument) beside the mesh-triangle slope census, per Shallow
//! region polygon.
//!
//! Every in-polygon triangle STEEPER than the clamp is attributed to
//! exactly one class by the label + planner angle at its centroid cell:
//!
//! * `A_fringe`    — cell labeled MidSteep/VerySteep, within `overlap_mm`
//!   of an owned Shallow cell: the polygon holds it only because of the
//!   overlap dilation. Hypothesis (a), fringe source.
//! * `A_interior`  — cell labeled MidSteep/VerySteep, farther than
//!   `overlap_mm` from owned Shallow cells: the polygon holds it through
//!   hole-filling or extraction artifacts. Hypothesis (a), non-fringe.
//! * `A_uncovered` — cell label `None` (not covered): polygon spans
//!   uncovered ground. Hypothesis (a).
//! * `B1_underread` — cell labeled Shallow AND the planner's own slope
//!   angle at the cell is ≤ the clamp, but the triangle is steeper: the
//!   planner grid does not SEE the steepness (resolution or sampler).
//!   Hypothesis (b), the defect arm.
//! * `B2_relabel`  — cell labeled Shallow but the planner's own angle is
//!   ABOVE the clamp: the planner saw the steepness and the conditioning
//!   (hysteresis flood, morphological close, min-area absorption)
//!   relabeled it Shallow. By-design behaviour, not a classification
//!   defect — but ground the band still cannot finish to spec.
//!
//! The classes partition the steep 3D area, so their sum must reconcile
//! with V1's 42.90 % on the same region. The instrument also prints the
//! CELL-ownership view per region — in-polygon covered cells owned vs not
//! owned by Shallow — which is the number the Track M band-cell audit
//! hardening changes.
//!
//! NUMBERS ONLY: the hypothesis ruling is written in
//! `planning/metrology_2026-09-02/FINDINGS.md` beside this run's output,
//! not printed as a verdict here.
//!
//! `#[ignore]` — evidence run; needs the operator's wanaka mesh (not in
//! the repo). SKIPs when absent; never substitutes a fixture mesh.

#![allow(
    clippy::print_stderr,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::too_many_lines
)]

use std::path::Path;

use rayon::prelude::*;
use rs_cam_core::finish::classify_probe::ClassificationSampler;
use rs_cam_core::finish::finish_planner::{FinishBand, FinishPlannerParams, decompose};
use rs_cam_core::finish::finish_setup::build_classification_surface_with_sampler_and_cancel;
use rs_cam_core::finish::unified_finish::unified_finish_classification_resolution;
use rs_cam_core::geo::P2;
use rs_cam_core::geometry::grid_field::distance_transform_2d;
use rs_cam_core::maps::tier_islands::{TierIslandParams, extract_tier_islands};
use rs_cam_core::maps::tier_map::{ResidualTreatment, TierLadder, TierMapParams, compute_tier_map};
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::tool::{MillingCutter, TaperedBallEndmill};

// ── fixtures and dials — verbatim from `valley_branch_falsifier_h1.rs`,
//    which restates `planning/multitool_2026-08-23/wanaka200_mt2.toml`.
//    Restated rather than imported: an integration test cannot `use`
//    another one. ─────────────────────────────────────────────────────────

const WANAKA_MESH: &str = "/home/ricky/Downloads/wanaka200/rivmap_export/terrain.stl";

const CELL_MM: f64 = 0.3;
const TOLERANCE_MM: f64 = 0.05;
const MARGIN_MM: f64 = 0.5;
const COARSENESS: f64 = 1.0;
const OVERLAP_MM: f64 = 2.0;
const MAX_REGIONS_PER_TIER: usize = 24;
const OP_TOLERANCE_MM: f64 = 0.05;

/// How many Shallow regions get the full attribution table (largest area
/// first). All regions land in the totals row regardless.
const MAX_DETAILED_REGIONS: usize = 16;

/// One attribution class's accumulator: steep 3D area (mm²) and triangle
/// count.
#[derive(Default, Clone, Copy)]
struct Bucket {
    area_mm2: f64,
    tris: usize,
}

impl Bucket {
    fn add(&mut self, area: f64) {
        self.area_mm2 += area;
        self.tris += 1;
    }
}

#[derive(Default, Clone, Copy)]
struct RegionAttribution {
    /// All in-polygon triangles, 3D.
    total_area_mm2: f64,
    /// In-polygon triangles steeper than the clamp, 3D.
    steep_area_mm2: f64,
    a_fringe: Bucket,
    a_interior: Bucket,
    a_uncovered: Bucket,
    b1_underread: Bucket,
    b2_relabel: Bucket,
    /// B1 diagnostic: sum over B1 triangles of (triangle − planner) angle,
    /// area-weighted, so the mean under-read is printable.
    b1_underread_deg_area: f64,
    /// Cells: in-polygon covered cells owned by Shallow / total in-polygon
    /// covered cells.
    cells_owned: usize,
    cells_in_poly: usize,
}

#[test]
#[ignore = "evidence run — needs the operator's wanaka mesh (not in repo)"]
fn wanaka_band_cell_ownership_g2() {
    eprintln!(
        "\n========== Avenue G, G2 — BAND-CELL OWNERSHIP ATTRIBUTION ==========\n\
         Decides Track H V1 finding 2's two hypotheses. Classes are fixed in the\n\
         file header before the run. NUMBERS ONLY — the ruling goes to\n\
         planning/metrology_2026-09-02/FINDINGS.md.\n"
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

    // Tier map + tier-1 fine island — the same coverage clip H1 used.
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
    assert_eq!(
        planned.labels.len(),
        rows * cols,
        "decompose must return the full label grid"
    );

    // Distance (cells) from every cell to the nearest OWNED Shallow cell —
    // the fringe test. `distance_transform_2d` distances are in cell units.
    let owned_shallow: Vec<bool> = planned
        .labels
        .iter()
        .map(|l| *l == Some(FinishBand::Shallow))
        .collect();
    let dist_to_owned = distance_transform_2d(&owned_shallow, rows, cols);
    let fringe_cells = OVERLAP_MM / cell;

    eprintln!(
        "setup: {} triangles, grid {rows}x{cols} @ {cell:.3} mm, clamp {clamp_deg:.1} deg, \
         overlap {OVERLAP_MM:.1} mm ({fringe_cells:.1} cells), {} planned regions \
         ({} Shallow), setup {:.0} s",
        mesh.triangles.len(),
        planned.regions.len(),
        planned
            .regions
            .iter()
            .filter(|r| r.band == FinishBand::Shallow)
            .count(),
        started.elapsed().as_secs_f64()
    );

    // Shallow regions, largest XY area first — the V1 region is the largest.
    let mut shallow: Vec<&Polygon2> = planned
        .regions
        .iter()
        .filter(|r| r.band == FinishBand::Shallow)
        .map(|r| &r.polygon)
        .collect();
    shallow.sort_by(|a, b| b.area().total_cmp(&a.area()));

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

    let mut totals = RegionAttribution::default();
    for (ridx, poly) in shallow.iter().enumerate() {
        let mut att = attribute_region(
            &mesh,
            poly,
            &planned.labels,
            &slope.angles,
            &dist_to_owned,
            fringe_cells,
            clamp_deg,
            &cell_at,
        );

        // Cell-ownership view: in-polygon covered cells vs owned cells.
        // Owned-cell angles mirror the production derate's population
        // (`shallow_region_max_slope_deg`): geometrically covered with all
        // in-grid 4-neighbours covered (the coverage-cliff filter), inside
        // the polygon — restricted to OWNED cells. Their max is the
        // avenue-G refund quantity: the derate the band would use if the
        // non-owned inclusions were excised.
        let geom = heightmap.covered_flags();
        let geom_at = |row: usize, col: usize| -> bool {
            geom.get(row * cols + col).copied().unwrap_or(false)
        };
        let mut owned_angles: Vec<f64> = Vec::new();
        let [bx0, by0, bx1, by1] = poly.bbox();
        let c0 = (((bx0 - slope.origin_x) / cell).floor().max(0.0)) as usize;
        let c1 = ((((bx1 - slope.origin_x) / cell).ceil()) as usize).min(cols - 1);
        let r0 = (((by0 - slope.origin_y) / cell).floor().max(0.0)) as usize;
        let r1 = ((((by1 - slope.origin_y) / cell).ceil()) as usize).min(rows - 1);
        for row in r0..=r1 {
            for col in c0..=c1 {
                let i = row * cols + col;
                if planned.labels[i].is_none() {
                    continue;
                }
                let p = P2::new(
                    slope.origin_x + col as f64 * cell,
                    slope.origin_y + row as f64 * cell,
                );
                if !poly.contains_point(&p) {
                    continue;
                }
                att.cells_in_poly += 1;
                if owned_shallow[i] {
                    att.cells_owned += 1;
                    let neighbours_ok = geom_at(row, col)
                        && (row == 0 || geom_at(row - 1, col))
                        && (row + 1 >= rows || geom_at(row + 1, col))
                        && (col == 0 || geom_at(row, col - 1))
                        && (col + 1 >= cols || geom_at(row, col + 1));
                    if neighbours_ok {
                        owned_angles.push(slope.angles[i].to_degrees());
                    }
                }
            }
        }

        accumulate(&mut totals, &att);
        if ridx < MAX_DETAILED_REGIONS {
            print_region(
                &format!("Shallow region #{ridx}"),
                poly.area(),
                &att,
                clamp_deg,
            );
            print_owned_refund(&mut owned_angles, clamp_deg);
        }
    }
    print_region("ALL SHALLOW REGIONS (totals)", 0.0, &totals, clamp_deg);
    eprintln!("\ntotal wall time {:.0} s", started.elapsed().as_secs_f64());
}

#[allow(clippy::too_many_arguments)]
fn attribute_region(
    mesh: &TriangleMesh,
    poly: &Polygon2,
    labels: &[Option<FinishBand>],
    angles: &[f64],
    dist_to_owned: &[f64],
    fringe_cells: f64,
    clamp_deg: f64,
    cell_at: &(impl Fn(f64, f64) -> Option<usize> + Sync),
) -> RegionAttribution {
    let [bx0, by0, bx1, by1] = poly.bbox();
    // Per-triangle rows: (steep 3D area or 0, class id, area, underread deg).
    // Membership + slope: the `region_triangles` convention from
    // `valley_branch_falsifier_h1.rs` — centroid containment, upward face
    // normal, 3D area — so the steep share reconciles with V1's census.
    let rows: Vec<(f64, f64, usize, f64)> = (0..mesh.triangles.len())
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
            let slope_deg = n.z.clamp(-1.0, 1.0).acos().to_degrees();

            let steep = slope_deg > clamp_deg;
            if !steep {
                return Some((area, 0.0, usize::MAX, 0.0));
            }
            let Some(i) = cell_at(cx, cy) else {
                // Centroid off the classification grid — polygon past the
                // grid edge. Counted with A_uncovered.
                return Some((area, area, 2, 0.0));
            };
            let class = match labels[i] {
                Some(FinishBand::MidSteep) | Some(FinishBand::VerySteep) => {
                    if dist_to_owned[i] <= fringe_cells {
                        0 // A_fringe
                    } else {
                        1 // A_interior
                    }
                }
                None => 2, // A_uncovered
                Some(FinishBand::Shallow) => {
                    // `SlopeMap::angles` is in RADIANS; the clamp is degrees.
                    if angles[i].to_degrees() > clamp_deg {
                        4 // B2_relabel
                    } else {
                        3 // B1_underread
                    }
                }
            };
            let underread = if class == 3 {
                (slope_deg - angles[i].to_degrees()) * area
            } else {
                0.0
            };
            Some((area, area, class, underread))
        })
        .collect();

    let mut att = RegionAttribution::default();
    for (area, steep_area, class, underread) in rows {
        att.total_area_mm2 += area;
        att.steep_area_mm2 += steep_area;
        match class {
            0 => att.a_fringe.add(steep_area),
            1 => att.a_interior.add(steep_area),
            2 => att.a_uncovered.add(steep_area),
            3 => {
                att.b1_underread.add(steep_area);
                att.b1_underread_deg_area += underread;
            }
            4 => att.b2_relabel.add(steep_area),
            _ => {}
        }
    }
    att
}

fn accumulate(t: &mut RegionAttribution, a: &RegionAttribution) {
    t.total_area_mm2 += a.total_area_mm2;
    t.steep_area_mm2 += a.steep_area_mm2;
    for (dst, src) in [
        (&mut t.a_fringe, a.a_fringe),
        (&mut t.a_interior, a.a_interior),
        (&mut t.a_uncovered, a.a_uncovered),
        (&mut t.b1_underread, a.b1_underread),
        (&mut t.b2_relabel, a.b2_relabel),
    ] {
        dst.area_mm2 += src.area_mm2;
        dst.tris += src.tris;
    }
    t.b1_underread_deg_area += a.b1_underread_deg_area;
    t.cells_owned += a.cells_owned;
    t.cells_in_poly += a.cells_in_poly;
}

fn print_region(label: &str, xy_area_mm2: f64, att: &RegionAttribution, clamp_deg: f64) {
    let steep = att.steep_area_mm2.max(1e-9);
    let total = att.total_area_mm2.max(1e-9);
    let pct = |b: &Bucket| 100.0 * b.area_mm2 / steep;
    eprintln!(
        "\n---------- {label} ----------\n\
         \x20  polygon XY area {xy_area_mm2:>10.1} mm² | triangle 3D area {:.1} mm²\n\
         \x20  3D area steeper than the {clamp_deg:.1} deg clamp: {:.1} mm² = {:.2} % of the region\n\
         \x20  STEEP-AREA ATTRIBUTION (partition of that steep area):\n\
         \x20    A_fringe    (non-owned label, within overlap of owned) {:>10.1} mm²  {:>6.2} %  ({} tris)\n\
         \x20    A_interior  (non-owned label, beyond overlap)          {:>10.1} mm²  {:>6.2} %  ({} tris)\n\
         \x20    A_uncovered (label None / off-grid)                    {:>10.1} mm²  {:>6.2} %  ({} tris)\n\
         \x20    B1_underread (Shallow label, planner angle <= clamp)   {:>10.1} mm²  {:>6.2} %  ({} tris)\n\
         \x20    B2_relabel   (Shallow label, planner angle >  clamp)   {:>10.1} mm²  {:>6.2} %  ({} tris)",
        total,
        att.steep_area_mm2,
        100.0 * att.steep_area_mm2 / total,
        att.a_fringe.area_mm2,
        pct(&att.a_fringe),
        att.a_fringe.tris,
        att.a_interior.area_mm2,
        pct(&att.a_interior),
        att.a_interior.tris,
        att.a_uncovered.area_mm2,
        pct(&att.a_uncovered),
        att.a_uncovered.tris,
        att.b1_underread.area_mm2,
        pct(&att.b1_underread),
        att.b1_underread.tris,
        att.b2_relabel.area_mm2,
        pct(&att.b2_relabel),
        att.b2_relabel.tris,
    );
    if att.b1_underread.area_mm2 > 0.0 {
        eprintln!(
            "\x20    B1 mean under-read (triangle − planner angle, area-weighted): {:.2} deg",
            att.b1_underread_deg_area / att.b1_underread.area_mm2
        );
    }
    if att.cells_in_poly > 0 {
        eprintln!(
            "\x20  CELL OWNERSHIP: {} of {} in-polygon covered cells owned by Shallow \
             = {:.2} % owned, {:.2} % non-owned",
            att.cells_owned,
            att.cells_in_poly,
            100.0 * att.cells_owned as f64 / att.cells_in_poly as f64,
            100.0 * (att.cells_in_poly - att.cells_owned) as f64 / att.cells_in_poly as f64
        );
    }
}

/// The avenue-G refund view: owned-cell planner angles (production filter
/// mirrored), their max/p99, and the stepover refund `cos(owned max) /
/// cos(clamp)` the band would get if non-owned inclusions were excised.
fn print_owned_refund(owned_angles: &mut [f64], clamp_deg: f64) {
    if owned_angles.is_empty() {
        eprintln!("\x20  OWNED-CELL SLOPES: no owned cells passed the derate filter");
        return;
    }
    owned_angles.sort_by(f64::total_cmp);
    let n = owned_angles.len();
    let max = owned_angles[n - 1];
    let p99 = owned_angles[((n - 1) as f64 * 0.99).round() as usize];
    let refund_max = max.min(clamp_deg).to_radians().cos() / clamp_deg.to_radians().cos();
    let refund_p99 = p99.min(clamp_deg).to_radians().cos() / clamp_deg.to_radians().cos();
    eprintln!(
        "\x20  OWNED-CELL SLOPES ({} cells, derate filter mirrored): max {:.2} deg, p99 {:.2} deg\n\
         \x20    stepover refund if excised (cos owned-max / cos clamp): {:.3}x  (p99 basis {:.3}x)",
        n, max, p99, refund_max, refund_p99
    );
}
