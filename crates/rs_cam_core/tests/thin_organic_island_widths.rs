//! **Evidence run** — how WIDE are the fine tier's regions on the real wanaka
//! board? (`planning/thin_organic_2026-08-27/FINDINGS.md` §1.1, §6 item 4.)
//!
//! # Why this exists
//!
//! FINDINGS ranks "contour-parallel for thin regions" (Lever 1) first and
//! predicts it cuts the fine tier's junction count by 20–100×. That prediction
//! rests on one unmeasured claim: the fine tier's territory is a dendritic web
//! of narrow fingers, so a fixed-direction raster crosses each finger and
//! fragments, while an offset cascade would run ALONG it and collapse after
//! `⌈w / 2s⌉` rings.
//!
//! The "fingers of order 1.5–3 mm" figure is **inferred**, not measured — from
//! dividing the ledger's ~17k intra-node retracts by the scan-row count. §6
//! lists it as the document's biggest open assumption. This measures it.
//!
//! # It measures TWO populations, and the second is the decisive one
//!
//! **Stage A — tier islands.** What the planner hands to the op as a machining
//! boundary (`tier_islands::extract_tier_islands`).
//!
//! **Stage B — the regions the GENERATOR actually receives.** `unified_finish`
//! does not machine the tier islands directly: it re-decomposes its own slope
//! bands *inside* that boundary (`finish_planner::decompose`), and Step 6
//! extracts each band's polygons through `region_polygons_from_mask_clamped`,
//! which **dilates the mask by `overlap_mm`** (2.0 mm live) before marching
//! squares. Two consequences a Stage-A-only reading would miss entirely:
//!
//! * a 1.5 mm mask finger reaches the generator as a **≥ 5.5 mm polygon**, so
//!   a routing rule written against mask width is measuring the wrong object;
//! * fingers less than `2 × overlap_mm` = **4 mm** apart **merge**, so the
//!   shallow band may already be ONE connected polygon with holes rather than
//!   a population of thin islands.
//!
//! If Stage B says the shallow band is a few fat polygons, Lever 1's premise
//! is false on this fixture and the §1.2 ranking needs revisiting **before**
//! anyone writes production code. That is the point of running this first.
//!
//! # Units
//!
//! Width is the **local inscribed diameter**: a Euclidean distance transform,
//! so a cell reads its distance to the nearest cell outside the shape, and
//! width is `2 · max(EDT) · cell`. Exact for a strip (`= W`) and a disc
//! (`= 2r`) — unlike an area/perimeter proxy, which halves on blobs and would
//! mis-route them.
//!
//! Reported in mm **and in stepovers**, because Lever 1's routing rule is
//! `min_width <= THIN_K · s` and `s` differs per tier under the equal-cusp law.
//! The decision number is the last line of each block: the **share of area** in
//! regions narrower than K stepovers.
//!
//! # Running it
//!
//! ```text
//! # Historic Stages A–H (all evidence):
//! cargo test -p rs_cam_core --test thin_organic_island_widths \
//!   wanaka_tier_and_band_region_widths -- --ignored --nocapture
//!
//! # Focused C1+E1 cell/retract evidence (Stages I/J):
//! THIN_ORGANIC_SVG_DIR=/home/ricky/Downloads/svg \
//! cargo test -p rs_cam_core --test thin_organic_island_widths \
//!   wanaka_monotone_cells_kept_retracts -- --ignored --nocapture
//! The environment override writes one coloured-cell overlay SVG for each of
//! the three measured regions; without it, they land in `target/thin_organic/`.
//!
//! # A3 re-baseline under a realistic link ceiling (Stage L):
//! cargo test -p rs_cam_core --test thin_organic_island_widths \
//!   wanaka_ceiling_rebaseline_a3 -- --ignored --nocapture
//!
//! # D1 per-cell sweep direction, priced against A3's table (Stage M):
//! cargo test -p rs_cam_core --test thin_organic_island_widths \
//!   wanaka_per_cell_direction_d1 -- --ignored --nocapture
//!
//! # D2 per-cell PATTERN — contour rings vs raster, per cell (Stage N):
//! cargo test -p rs_cam_core --test thin_organic_island_widths \
//!   wanaka_per_cell_pattern_d2 -- --ignored --nocapture
//! ```
//!
//! `#[ignore]` because it needs the operator's wanaka mesh, which is not in the
//! repo — the same meaning `#[ignore]` carries everywhere else here (evidence
//! runs, invoked explicitly, never by a gate). SKIPS rather than fails when the
//! mesh is absent.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]

use std::{
    fmt::Write as _,
    path::{Path, PathBuf},
};

use rs_cam_core::classify_probe::ClassificationSampler;
use rs_cam_core::finish_planner::{FinishBand, FinishPlannerParams, decompose};
use rs_cam_core::finish_setup::build_classification_surface_with_sampler_and_cancel;
use rs_cam_core::geo::P2;
use rs_cam_core::geometry::contour_extract::marching_squares_bool_grid;
use rs_cam_core::geometry::grid_field::distance_transform_2d;
use rs_cam_core::maps::tier_islands::{TierIslandParams, extract_tier_islands};
use rs_cam_core::maps::tier_map::{ResidualTreatment, TierLadder, TierMapParams, compute_tier_map};
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::metrology::costing::{
    CandidateCost, CostingContext, CostingFeeds, LinkRegime,
    relink_and_cost_under as metrology_relink_and_cost_under,
};
use rs_cam_core::polygon::{Polygon2, detect_containment, shoelace_area};
use rs_cam_core::tool::{MillingCutter, TaperedBallEndmill};
use rs_cam_core::unified_finish::unified_finish_classification_resolution;

/// The operator's wanaka board. Absolute, outside the repo, by nature.
const WANAKA_MESH: &str = "/home/ricky/Downloads/wanaka200/rivmap_export/terrain.stl";

// ── dials, copied verbatim from `planning/multitool_2026-08-23/wanaka200_mt2.toml` ──
//
// Re-read 2026-08-27 late: that file is UNTRACKED and was regenerated during
// the session, so it now describes an **R1.5 → R1.0** ladder at a 30 µm cusp,
// not the R2.0 → R1.0 at 10 µm an earlier FINDINGS draft quoted. Constants
// here follow the file; if it moves again, re-read it rather than trusting
// these.
const CELL_MM: f64 = 0.3;
const TOLERANCE_MM: f64 = 0.05;
const MARGIN_MM: f64 = 0.5;
const COARSENESS: f64 = 1.0;
const OVERLAP_MM: f64 = 2.0;
const MAX_REGIONS_PER_TIER: usize = 24;
/// `scallop_height` on both tier ops — the cusp target the equal-cusp
/// stepovers are derived from (0.596992 for R1.5, 0.486210 for R1.0, both of
/// which reproduce from the law below to 7 significant figures).
const CUSP_HEIGHT_MM: f64 = 0.03;
/// The unified op's own path tolerance, which sizes its classification grid.
const OP_TOLERANCE_MM: f64 = 0.05;

/// `s = 2·√(2Rh − h²)` — the equal-cusp law. One site in production
/// (`session::multitool`); restated here because an instrument should show its
/// own arithmetic.
fn equal_cusp_stepover_mm(cusp_radius_mm: f64, h: f64) -> f64 {
    if h <= 0.0 || h >= cusp_radius_mm {
        return 0.0;
    }
    2.0 * (2.0 * cusp_radius_mm * h - h * h).sqrt()
}

/// Connected components of `mask` (4-connectivity) as cell-index lists.
fn components(mask: &[bool], nx: usize, ny: usize) -> Vec<Vec<usize>> {
    let mut seen = vec![false; mask.len()];
    let mut out = Vec::new();
    for start in 0..mask.len() {
        if !mask[start] || seen[start] {
            continue;
        }
        let mut stack = vec![start];
        seen[start] = true;
        let mut comp = Vec::new();
        while let Some(i) = stack.pop() {
            comp.push(i);
            let (r, c) = (i / nx, i % nx);
            let mut push = |rr: usize, cc: usize, st: &mut Vec<usize>| {
                let j = rr * nx + cc;
                if mask[j] && !seen[j] {
                    seen[j] = true;
                    st.push(j);
                }
            };
            if r > 0 {
                push(r - 1, c, &mut stack);
            }
            if r + 1 < ny {
                push(r + 1, c, &mut stack);
            }
            if c > 0 {
                push(r, c - 1, &mut stack);
            }
            if c + 1 < nx {
                push(r, c + 1, &mut stack);
            }
        }
        out.push(comp);
    }
    out
}

/// Inscribed diameter of one polygon, by rasterising it at `cell` and running
/// an EDT. Holes are honoured because `contains_point` honours them, so a
/// polygon that is a fat blob with a hole through it reads as the width of the
/// remaining wall — which is the number that matters here.
fn polygon_width_mm(poly: &Polygon2, cell: f64) -> f64 {
    let [x0, y0, x1, y1] = poly.bbox();
    // One cell of padding so the EDT always sees an outside.
    let nx = (((x1 - x0) / cell).ceil() as usize).saturating_add(3);
    let ny = (((y1 - y0) / cell).ceil() as usize).saturating_add(3);
    if nx < 3 || ny < 3 || nx.saturating_mul(ny) > 40_000_000 {
        return 0.0;
    }
    let mut inside = vec![false; nx * ny];
    for r in 0..ny {
        for c in 0..nx {
            let x = x0 + (c as f64 - 1.0) * cell;
            let y = y0 + (r as f64 - 1.0) * cell;
            inside[r * nx + c] = poly.contains_point(&P2::new(x, y));
        }
    }
    let complement: Vec<bool> = inside.iter().map(|&v| !v).collect();
    let dist = distance_transform_2d(&complement, ny, nx);
    let max_d = inside
        .iter()
        .zip(dist.iter())
        .filter(|(is_inside, _)| **is_inside)
        .fold(0.0_f64, |acc, (_, d)| acc.max(*d));
    2.0 * max_d * cell
}

fn percentile(sorted: &[f64], q: f64) -> f64 {
    if sorted.is_empty() {
        return f64::NAN;
    }
    let idx = ((sorted.len() - 1) as f64 * q).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

/// Print the width/area distribution and the area-share decision line.
fn report_widths(label: &str, rows: &mut [(f64, f64)], stepover: f64) {
    if rows.is_empty() {
        println!("   {label}: (empty)\n");
        return;
    }
    rows.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    println!("   {label}: {} region(s)", rows.len());
    println!(
        "     {:>10}  {:>10}  {:>12}",
        "area mm²", "width mm", "stepovers"
    );
    for (w, a) in rows.iter().take(10) {
        let so = if stepover > 0.0 {
            w / stepover
        } else {
            f64::NAN
        };
        println!("     {a:>10.1}  {w:>10.2}  {so:>12.1}");
    }
    let mut widths: Vec<f64> = rows.iter().map(|(w, _)| *w).collect();
    widths.sort_by(f64::total_cmp);
    println!(
        "     width mm  min {:.2}  p25 {:.2}  median {:.2}  p75 {:.2}  max {:.2}",
        percentile(&widths, 0.0),
        percentile(&widths, 0.25),
        percentile(&widths, 0.5),
        percentile(&widths, 0.75),
        percentile(&widths, 1.0)
    );
    let total: f64 = rows.iter().map(|(_, a)| *a).sum();
    print!("     AREA in regions narrower than:");
    for k in [4.0_f64, 8.0, 16.0] {
        let bar = k * stepover;
        let share: f64 = rows
            .iter()
            .filter(|(w, _)| *w <= bar)
            .map(|(_, a)| *a)
            .sum();
        let pct = if total > 0.0 {
            100.0 * share / total
        } else {
            0.0
        };
        print!("  {k:.0}·s ({bar:.2}mm): {pct:.1}%");
    }
    println!("\n");
}

#[test]
#[ignore = "evidence run — needs the operator's wanaka mesh (not in repo)"]
fn wanaka_tier_and_band_region_widths() {
    let path = Path::new(WANAKA_MESH);
    if !path.exists() {
        println!("SKIP: {WANAKA_MESH} not present on this machine.");
        return;
    }

    let mesh = TriangleMesh::from_stl_scaled(path, 1.0).expect("load wanaka terrain");
    println!(
        "mesh: {} triangles, bbox {:.1} x {:.1} x {:.1} mm",
        mesh.faces.len(),
        mesh.bbox.max.x - mesh.bbox.min.x,
        mesh.bbox.max.y - mesh.bbox.min.y,
        mesh.bbox.max.z - mesh.bbox.min.z
    );
    let index = SpatialIndex::build_auto(&mesh);

    // Live ladder: tool_ids [3, 2] = R1.5 -> R1.0, geometry from the same file.
    // `TaperedBallEndmill::new(ball_diameter, taper_half_angle, shaft, len)`.
    let r15 = TaperedBallEndmill::new(3.0, 2.8, 6.0, 30.5);
    let r10 = TaperedBallEndmill::new(2.0, 5.7, 6.0, 20.0);
    let tools: [&dyn MillingCutter; 2] = [&r15, &r10];
    let ladder = TierLadder::new(&tools).expect("ladder");

    let params = TierMapParams {
        cell_mm: CELL_MM,
        tolerance_mm: TOLERANCE_MM,
        margin_mm: MARGIN_MM,
        treatment: ResidualTreatment::SlopeCompensated,
    };
    let never_cancel = || false;
    let t0 = std::time::Instant::now();
    let map = compute_tier_map(&mesh, &index, &ladder, &params, &never_cancel).expect("tier map");
    println!(
        "tier map: {} x {} cells @ {:.2} mm, {} tiers, {:.1} s\n",
        map.nx,
        map.ny,
        map.cell_mm,
        map.tier_count,
        t0.elapsed().as_secs_f64()
    );

    let cusp_radii: Vec<f64> = tools.iter().map(|t| t.cusp_radius_mm()).collect();
    let island_params = TierIslandParams {
        coarseness: COARSENESS,
        overlap_mm: OVERLAP_MM,
        max_regions_per_tier: MAX_REGIONS_PER_TIER,
        ..TierIslandParams::default()
    };
    let islands = extract_tier_islands(&map, &island_params, &cusp_radii).expect("islands");
    let cell_area = map.cell_mm * map.cell_mm;

    // ── STAGE A: tier islands (the machining boundary handed to the op) ──
    println!("========== STAGE A — tier islands (planner output) ==========\n");
    for set in &islands.per_tier {
        let tier = set.tier as usize;
        let cusp_r = cusp_radii[tier.min(cusp_radii.len() - 1)];
        let stepover = equal_cusp_stepover_mm(cusp_r, CUSP_HEIGHT_MM);
        println!("── tier {tier} (cusp R{cusp_r:.1}, stepover {stepover:.4} mm) ──");
        println!(
            "   islands {} (raw {}), owned {:.0} mm² / {} cells, close radius {:.3} mm",
            set.islands,
            set.raw_island_count,
            set.owned_area_mm2,
            set.owned_cells,
            set.close_radius_mm
        );
        if set.cap.acted() {
            println!("   NOTE: cap acted — {:?}", set.cap);
        }
        if set.owned_cells == 0 {
            println!("   (owns nothing)\n");
            continue;
        }
        let complement: Vec<bool> = set.owned_mask.iter().map(|&v| !v).collect();
        let dist = distance_transform_2d(&complement, map.ny, map.nx);
        let mut rows: Vec<(f64, f64)> = components(&set.owned_mask, map.nx, map.ny)
            .iter()
            .map(|comp| {
                let max_d = comp.iter().fold(0.0_f64, |acc, &i| acc.max(dist[i]));
                (2.0 * max_d * map.cell_mm, comp.len() as f64 * cell_area)
            })
            .collect();
        report_widths("owned islands", &mut rows, stepover);
    }

    // ── STAGE B: what the GENERATOR receives, per band ──
    //
    // Reproduces `unified_finish`'s Step 1-2: classification surface at the
    // op's own resolution, coverage ANDed with the tier's `machining` region
    // set, then `decompose`. This is the population Lever 1's routing rule
    // would actually be applied to.
    println!(
        "========== STAGE B — decompose regions inside tier 1 (what the generator sees) ==========\n"
    );
    let Some(fine) = islands.per_tier.iter().find(|s| s.tier == 1) else {
        println!("no tier 1; nothing to decompose");
        return;
    };
    if fine.machining.is_empty() {
        println!("tier 1 machining set is empty; nothing to decompose");
        return;
    }
    let cusp_r = cusp_radii[1];
    let stepover = equal_cusp_stepover_mm(cusp_r, CUSP_HEIGHT_MM);

    let surface = build_classification_surface_with_sampler_and_cancel(
        &mesh,
        &index,
        &r10,
        unified_finish_classification_resolution(&r10, OP_TOLERANCE_MM),
        ClassificationSampler::PRODUCTION,
        &never_cancel,
    )
    .expect("classification surface");
    println!(
        "classification grid: {} x {} @ {:.3} mm",
        surface.rows(),
        surface.cols(),
        surface.cell_size()
    );

    let hm = &surface.heightmap;
    let covered: Vec<bool> = hm
        .covered_flags()
        .iter()
        .enumerate()
        .map(|(i, &cov)| {
            if !cov {
                return false;
            }
            let r = i / hm.cols;
            let c = i % hm.cols;
            let x = hm.origin_x + c as f64 * hm.cell_size;
            let y = hm.origin_y + r as f64 * hm.cell_size;
            fine.machining.contains(&P2::new(x, y))
        })
        .collect();
    let covered_cells = covered.iter().filter(|&&v| v).count();
    println!(
        "covered ∩ tier-1 machining: {} cells ({:.0} mm²)\n",
        covered_cells,
        covered_cells as f64 * surface.cell_size() * surface.cell_size()
    );

    let mut planner = FinishPlannerParams::for_tool(cusp_r);
    planner.overlap_mm = OVERLAP_MM;
    let planned = decompose(&surface.slope_map, &covered, &[], &planner);
    println!(
        "decompose: {} regions total (post-cap), close {:.3} mm, min area {:.1} mm², overlap {:.1} mm",
        planned.regions.len(),
        planner.close_radius_mm,
        planner.min_region_area_mm2,
        planner.overlap_mm
    );
    println!(
        "   raw steep islands {}, raw very-steep {}, absorbed {}\n",
        planned.stats.raw_steep_islands,
        planned.stats.raw_very_steep_islands,
        planned.stats.absorbed_regions
    );

    for band in [
        FinishBand::Shallow,
        FinishBand::MidSteep,
        FinishBand::VerySteep,
    ] {
        let mut rows: Vec<(f64, f64)> = planned
            .regions
            .iter()
            .filter(|r| r.band == band)
            .map(|r| {
                (
                    polygon_width_mm(&r.polygon, surface.cell_size()),
                    r.polygon.area(),
                )
            })
            .collect();
        println!("── band {band:?} ──");
        report_widths(
            "planned regions (post overlap dilation)",
            &mut rows,
            stepover,
        );
    }

    // ── STAGE C: the defect itself, measured ──
    //
    // Stage B killed the width-based routing rule (0% of shallow area under 8
    // stepovers). But width was never the thing that fragments a raster —
    // CROSSINGS PER SCAN LINE is. A 10 mm-wide, 300 mm-long branching snake is
    // "wide" and still shreds a raster, because one scan row enters and leaves
    // it many times.
    //
    // So measure the defect directly, on the real polygons, with no production
    // code written: walk scan rows at the tier's own stepover, count maximal
    // inside-runs per row (that IS what `raster_toolpath_from_grid` emits as
    // separate fragments), and compare against the ring count an offset
    // cascade would need for the same region (`⌈width / 2s⌉`).
    println!("========== STAGE C — raster fragments vs cascade rings (the defect) ==========\n");
    println!(
        "   Scan rows at s = {stepover:.4} mm, axis-aligned (direction_deg = 0), \
         exactly as the Shallow band emits today.\n"
    );
    let mut grand_frag = 0usize;
    let mut grand_rings = 0usize;
    for band in [FinishBand::Shallow, FinishBand::MidSteep] {
        let regions: Vec<&Polygon2> = planned
            .regions
            .iter()
            .filter(|r| r.band == band)
            .map(|r| &r.polygon)
            .collect();
        if regions.is_empty() {
            continue;
        }
        println!("── band {band:?} ──");
        println!(
            "     {:>10}  {:>10}  {:>10}  {:>8}",
            "area mm²", "fragments", "rings", "ratio"
        );
        let (mut band_frag, mut band_rings) = (0usize, 0usize);
        let mut rows: Vec<(f64, usize, usize)> = Vec::new();
        for poly in &regions {
            let [x0, y0, x1, y1] = poly.bbox();
            let mut frags = 0usize;
            // Sample along each row finely enough that a finger narrower than
            // one sample cannot be missed: quarter-stepover sampling.
            let sample = stepover * 0.25;
            let mut y = y0;
            while y <= y1 {
                let mut inside_run = false;
                let mut x = x0;
                while x <= x1 {
                    let inside = poly.contains_point(&P2::new(x, y));
                    if inside && !inside_run {
                        frags += 1;
                        inside_run = true;
                    } else if !inside {
                        inside_run = false;
                    }
                    x += sample;
                }
                y += stepover;
            }
            let width = polygon_width_mm(poly, surface.cell_size());
            let rings = ((width / (2.0 * stepover)).ceil() as usize).max(1);
            band_frag += frags;
            band_rings += rings;
            rows.push((poly.area(), frags, rings));
        }
        rows.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        for (a, f, r) in rows.iter().take(8) {
            let ratio = if *r > 0 {
                *f as f64 / *r as f64
            } else {
                f64::NAN
            };
            println!("     {a:>10.1}  {f:>10}  {r:>10}  {ratio:>8.1}x");
        }
        println!(
            "     BAND TOTAL: {band_frag} raster fragments vs {band_rings} cascade rings \
             = {:.1}x fewer junctions if contoured\n",
            if band_rings > 0 {
                band_frag as f64 / band_rings as f64
            } else {
                f64::NAN
            }
        );
        grand_frag += band_frag;
        grand_rings += band_rings;
    }
    stage_d(&mesh, &index, &r10, &planned, stepover);
    stage_e(&mesh, &index, &r10, &planned, stepover);
    stage_f(&mesh, &index, &r10, &planned, stepover, surface.cell_size());
    stage_g(&mesh, &index, &r10, &planned, stepover);
    stage_h(&r10, &mesh);

    println!(
        "SHALLOW+MIDSTEEP TOTAL: {grand_frag} raster fragments vs {grand_rings} rings.\n\
         Read against FINDINGS.md: the width-based routing rule (§1.1) is dead — \
         Stage B measured 0% of shallow area under 8 stepovers. If Stage C's \
         fragment ratio is large anyway, the LEVER survives and only its \
         TRIGGER needs replacing (elongation/crossings, not width)."
    );
}

/// Focused C1+E1 evidence run.  The historic A–H instrument also sweeps 12
/// angles and two unrelated strategies, so running the whole file to price
/// the cell hypothesis is needlessly expensive.  This setup intentionally
/// stops once the real Wanaka Shallow regions exist, then runs only I/J.
#[test]
#[ignore = "evidence run — needs the operator's wanaka mesh (not in repo)"]
fn wanaka_monotone_cells_kept_retracts() {
    let path = Path::new(WANAKA_MESH);
    if !path.exists() {
        println!("SKIP: {WANAKA_MESH} not present on this machine.");
        return;
    }

    let mesh = TriangleMesh::from_stl_scaled(path, 1.0).expect("load wanaka terrain");
    let index = SpatialIndex::build_auto(&mesh);
    let r15 = TaperedBallEndmill::new(3.0, 2.8, 6.0, 30.5);
    let r10 = TaperedBallEndmill::new(2.0, 5.7, 6.0, 20.0);
    let tools: [&dyn MillingCutter; 2] = [&r15, &r10];
    let ladder = TierLadder::new(&tools).expect("ladder");
    let never_cancel = || false;
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
    let cusp_radii: Vec<f64> = tools.iter().map(|tool| tool.cusp_radius_mm()).collect();
    let islands = extract_tier_islands(
        &map,
        &TierIslandParams {
            coarseness: COARSENESS,
            overlap_mm: OVERLAP_MM,
            max_regions_per_tier: MAX_REGIONS_PER_TIER,
            ..TierIslandParams::default()
        },
        &cusp_radii,
    )
    .expect("islands");
    let Some(fine) = islands.per_tier.iter().find(|set| set.tier == 1) else {
        println!("SKIP: no tier 1 machining region.");
        return;
    };
    if fine.machining.is_empty() {
        println!("SKIP: tier 1 machining region is empty.");
        return;
    }

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
        .map(|(i, &covered)| {
            if !covered {
                return false;
            }
            let row = i / heightmap.cols;
            let col = i % heightmap.cols;
            fine.machining.contains(&P2::new(
                heightmap.origin_x + col as f64 * heightmap.cell_size,
                heightmap.origin_y + row as f64 * heightmap.cell_size,
            ))
        })
        .collect();
    let mut planner = FinishPlannerParams::for_tool(cusp_radii[1]);
    planner.overlap_mm = OVERLAP_MM;
    let planned = decompose(&surface.slope_map, &covered, &[], &planner);
    let stepover = equal_cusp_stepover_mm(cusp_radii[1], CUSP_HEIGHT_MM);
    println!(
        "focused C1+E1 setup: {} planned regions, Shallow raster stepover {stepover:.4} mm\n",
        planned.regions.len()
    );
    let grid = grid_for_stage_i(&mesh, &index, &r10, stepover);
    let cells = stage_i(&grid, &planned, stepover, mesh.bbox.min.z - 0.1);
    write_cell_svg_comparisons(&cells);
    stage_j(&mesh, &index, &r10, &grid, &cells);
    stage_k(&mesh, &index, &r10, &grid, &cells, stepover);
}

// ── STAGE D ─────────────────────────────────────────────────────────────
//
// The one question Stage C cannot answer: **fewer junctions is not less
// time.** `STRATEGY_ADVISOR_2026-06-17.md` measured parallel 446 s vs spiral
// 828 s on this very board at the load limit, because contour paths chain
// short chords and the Grbl junction-deviation model crawls every corner. A
// 20x junction win can still lose the wall clock.
//
// So generate BOTH patterns over the SAME region with the SAME tool, feeds and
// machine, and cost each through the F-034 integrator. No production code is
// written to do this — the two generators already exist and `unified_finish`
// already calls both, just on different bands.

/// Shapeoko Pro XXL, read from `wanaka200_mt2.toml`'s `[job.machine]` /
/// `[job.machine.kinematics]`, so the integrator sees the operator's real
/// envelope rather than a default.
const MACHINE_ACCEL_XYZ: [f64; 3] = [500.0, 500.0, 270.0];
const MACHINE_ACCEL_SCALAR: f64 = 423.333_333_333_333_3;
const JUNCTION_DEVIATION_MM: f64 = 0.02;
const MAX_FEED_MM_MIN: f64 = 10_000.0;
const RAPID_FEED_MM_MIN: f64 = 5_000.0;
/// Tier-1 op feeds from the same file.
const FEED_MM_MIN: f64 = 735.0;
const PLUNGE_MM_MIN: f64 = 180.0;

fn stage_d(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &TaperedBallEndmill,
    planned: &rs_cam_core::finish_planner::PlannedRegions,
    stepover: f64,
) {
    use rs_cam_core::geometry::region_set::RegionSet;
    use rs_cam_core::machine::kinematics::{MachineKinematics, compute_cycle_time};
    use rs_cam_core::scallop::{
        ScallopDirection, ScallopParams, scallop_toolpath_structured_annotated_with_cancel,
    };
    use rs_cam_core::toolpath::raster_toolpath_from_grid;

    println!("========== STAGE D — integrated TIME, raster vs cascade (same region) ==========\n");

    let kin = MachineKinematics {
        acceleration_mm_s2: MACHINE_ACCEL_SCALAR,
        acceleration_xyz_mm_s2: Some(MACHINE_ACCEL_XYZ),
        junction_deviation_mm: JUNCTION_DEVIATION_MM,
        ..MachineKinematics::default()
    };
    let safe_z = mesh.bbox.max.z + 5.0;
    let effective_min_z = mesh.bbox.min.z - 0.1;
    let never_cancel = || false;

    // One mesh-global raster grid, exactly as the Shallow band builds it
    // (`direction_deg = 0.0`), shared across the regions below.
    let grid = rs_cam_core::surface::dropcutter::batch_drop_cutter(
        mesh,
        index,
        cutter,
        stepover,
        0.0,
        effective_min_z,
    );

    // The three largest Shallow regions — where the time actually is.
    let mut shallow: Vec<&Polygon2> = planned
        .regions
        .iter()
        .filter(|r| r.band == FinishBand::Shallow)
        .map(|r| &r.polygon)
        .collect();
    shallow.sort_by(|a, b| {
        b.area()
            .partial_cmp(&a.area())
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    println!(
        "   machine: accel [{:.0},{:.0},{:.0}] mm/s², junction dev {JUNCTION_DEVIATION_MM} mm, \
         rapid {RAPID_FEED_MM_MIN:.0}; feed {FEED_MM_MIN:.0}, plunge {PLUNGE_MM_MIN:.0} mm/min\n",
        MACHINE_ACCEL_XYZ[0], MACHINE_ACCEL_XYZ[1], MACHINE_ACCEL_XYZ[2]
    );
    println!(
        "     {:>9}  {:>8} {:>9} {:>9}  {:>8} {:>9} {:>9}  {:>7}",
        "area mm²", "R moves", "R cut mm", "R time s", "C moves", "C cut mm", "C time s", "speedup"
    );

    let (mut tot_r, mut tot_c) = (0.0_f64, 0.0_f64);
    for poly in shallow.iter().take(3) {
        let region = RegionSet::new(vec![(*poly).clone()]);

        // (a) RASTER — the exact call the Shallow band makes today.
        let raster = raster_toolpath_from_grid(
            &grid,
            FEED_MM_MIN,
            PLUNGE_MM_MIN,
            safe_z,
            Some(effective_min_z),
            Some(&region),
        );
        // FAIR COMPARISON: production does NOT ship the bare raster — every
        // region's toolpath goes through `relink_fragments` at
        // `intra_region_hookup_mm = 25.0` (unified_finish's Step 4). Comparing
        // an unrelinked raster against a natively-chained cascade would be
        // rigged in the cascade's favour, so relink the raster the same way.
        let lk = rs_cam_core::machine::kinematics::LinkKinematics {
            kinematics: kin,
            max_feed_mm_min: MAX_FEED_MM_MIN,
            rapid_feed_mm_min: RAPID_FEED_MM_MIN,
        };
        let rp = rs_cam_core::surface_link::RelinkParams {
            hookup_distance: 25.0,
            stock_to_leave: 0.0,
            sampling: 0.5,
            feed_rate: FEED_MM_MIN,
            plunge_rate: PLUNGE_MM_MIN,
            safe_z,
            link_kinematics: Some(&lk),
            reorder: true,
            boundary: Some(&region),
            link_ceiling: None,
            flush_ride: false,
            airborne_links_may_leave_territory: false,
        };
        let (linked, rep) = rs_cam_core::surface_link::relink_fragments(
            rs_cam_core::toolpath_spans::AnnotatedToolpath::new(raster),
            mesh,
            index,
            cutter,
            &rp,
        );
        let mut channels = rs_cam_core::transform_provenance::ReconcileSet::new(None, None);
        let raster = linked.reconcile(&mut channels).into_inner().toolpath;
        let r_time = compute_cycle_time(&raster, &kin, MAX_FEED_MM_MIN, RAPID_FEED_MM_MIN);
        let r_cut = raster.total_cutting_distance();
        println!(
            "       (raster relinked: {} fragments, {} linked, {} kept retracts)",
            rep.fragments, rep.surface_links, rep.retract_links
        );

        // (b) CASCADE — the exact call the MidSteep band makes, same region.
        let sp = ScallopParams {
            scallop_height: CUSP_HEIGHT_MM,
            tolerance: OP_TOLERANCE_MM,
            direction: ScallopDirection::default(),
            continuous: true,
            slope_from: 0.0,
            slope_to: 90.0,
            feed_rate: FEED_MM_MIN,
            plunge_rate: PLUNGE_MM_MIN,
            safe_z,
            stock_to_leave: 0.0,
            intra_pass_hookup_mm: 0.0,
            link_kinematics: None,
        };
        let Ok((cascade, _, _)) = scallop_toolpath_structured_annotated_with_cancel(
            mesh,
            index,
            cutter,
            &sp,
            None,
            Some(&region),
            &never_cancel,
        ) else {
            println!("     (cascade cancelled)");
            continue;
        };
        // Same treatment for the cascade — production relinks every region's
        // toolpath regardless of which generator produced it, so anything less
        // here would rig the comparison the other way.
        let (clinked, crep) = rs_cam_core::surface_link::relink_fragments(
            rs_cam_core::toolpath_spans::AnnotatedToolpath::new(cascade),
            mesh,
            index,
            cutter,
            &rp,
        );
        let mut cchannels = rs_cam_core::transform_provenance::ReconcileSet::new(None, None);
        let cascade = clinked.reconcile(&mut cchannels).into_inner().toolpath;
        println!(
            "       (cascade relinked: {} fragments, {} linked, {} kept retracts)",
            crep.fragments, crep.surface_links, crep.retract_links
        );
        let c_time = compute_cycle_time(&cascade, &kin, MAX_FEED_MM_MIN, RAPID_FEED_MM_MIN);
        let c_cut = cascade.total_cutting_distance();

        tot_r += r_time;
        tot_c += c_time;
        let speedup = if c_time > 0.0 {
            r_time / c_time
        } else {
            f64::NAN
        };
        println!(
            "     {:>9.1}  {:>8} {:>9.0} {:>9.1}  {:>8} {:>9.0} {:>9.1}  {:>6.2}x",
            poly.area(),
            raster.moves.len(),
            r_cut,
            r_time,
            cascade.moves.len(),
            c_cut,
            c_time,
            speedup
        );
    }
    let overall = if tot_c > 0.0 { tot_r / tot_c } else { f64::NAN };
    println!(
        "\n     TOP-3 TOTAL: raster {tot_r:.1} s vs cascade {tot_c:.1} s = {overall:.2}x\n\
         \n     This is the accel-aware answer STRATEGY_ADVISOR warns about: a >1.0x\n\
         speedup means contour wins on THIS machine's envelope despite chaining\n\
         short chords; <1.0x means the junction crawl eats the junction saving\n\
         and Lever 1 should NOT ship on wall-clock grounds.\n"
    );
}

// ── STAGE E ─────────────────────────────────────────────────────────────
//
// Stage D showed contour losing to the raster. That is NOT the same as the
// raster being optimal — it rules out one alternative, nothing more. The
// operator's own read (2026-08-28): "orienting the paths down the longer
// sections of narrow paths" should be faster.
//
// That is Lever 2, which §0d dismissed on the grounds that crossings stopped
// binding once relink absorbs them. But relink does not DELETE a junction, it
// converts a retract into a feed move — region 1 keeps 466 of them — and every
// row turnaround still costs a deceleration the integrator charges for. So the
// dismissal was an assertion, not a measurement.
//
// `batch_drop_cutter` already takes `direction_deg`, so this needs no
// production code either: sweep the angle, raster + relink + cost each, and
// let the integrator say whether orientation matters.
//
// TRAP, found earlier and worked around here: `dropcutter.rs` sends 0/90/180/
// 360 down the AXIS-ALIGNED fast path, so a literal 90.0 silently returns the
// 0° grid rather than a rotated one. 89.9 is used instead.
fn stage_e(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &TaperedBallEndmill,
    planned: &rs_cam_core::finish_planner::PlannedRegions,
    stepover: f64,
) {
    use rs_cam_core::geometry::region_set::RegionSet;
    use rs_cam_core::machine::kinematics::{LinkKinematics, MachineKinematics, compute_cycle_time};
    use rs_cam_core::toolpath::raster_toolpath_from_grid;

    println!("========== STAGE E — does SWEEP ANGLE matter? (the operator's read) ==========\n");

    let kin = MachineKinematics {
        acceleration_mm_s2: MACHINE_ACCEL_SCALAR,
        acceleration_xyz_mm_s2: Some(MACHINE_ACCEL_XYZ),
        junction_deviation_mm: JUNCTION_DEVIATION_MM,
        ..MachineKinematics::default()
    };
    let lk = LinkKinematics {
        kinematics: kin,
        max_feed_mm_min: MAX_FEED_MM_MIN,
        rapid_feed_mm_min: RAPID_FEED_MM_MIN,
    };
    let safe_z = mesh.bbox.max.z + 5.0;
    let effective_min_z = mesh.bbox.min.z - 0.1;

    let mut shallow: Vec<&Polygon2> = planned
        .regions
        .iter()
        .filter(|r| r.band == FinishBand::Shallow)
        .map(|r| &r.polygon)
        .collect();
    shallow.sort_by(|a, b| {
        b.area()
            .partial_cmp(&a.area())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let regions: Vec<&Polygon2> = shallow.into_iter().take(3).collect();

    const ANGLES: [f64; 12] = [
        0.0, 15.0, 30.0, 45.0, 60.0, 75.0, 89.9, 105.0, 120.0, 135.0, 150.0, 165.0,
    ];
    // [region][angle] -> (time_s, fragments, kept_retracts, cut_mm)
    let mut table: Vec<Vec<(f64, usize, usize, f64)>> = vec![Vec::new(); regions.len()];

    for angle in ANGLES {
        let grid = rs_cam_core::surface::dropcutter::batch_drop_cutter(
            mesh,
            index,
            cutter,
            stepover,
            angle,
            effective_min_z,
        );
        for (ri, poly) in regions.iter().enumerate() {
            let region = RegionSet::new(vec![(*poly).clone()]);
            let raster = raster_toolpath_from_grid(
                &grid,
                FEED_MM_MIN,
                PLUNGE_MM_MIN,
                safe_z,
                Some(effective_min_z),
                Some(&region),
            );
            let rp = rs_cam_core::surface_link::RelinkParams {
                hookup_distance: 25.0,
                stock_to_leave: 0.0,
                sampling: 0.5,
                feed_rate: FEED_MM_MIN,
                plunge_rate: PLUNGE_MM_MIN,
                safe_z,
                link_kinematics: Some(&lk),
                reorder: true,
                boundary: Some(&region),
                link_ceiling: None,
                flush_ride: false,
                airborne_links_may_leave_territory: false,
            };
            let (linked, rep) = rs_cam_core::surface_link::relink_fragments(
                rs_cam_core::toolpath_spans::AnnotatedToolpath::new(raster),
                mesh,
                index,
                cutter,
                &rp,
            );
            let mut ch = rs_cam_core::transform_provenance::ReconcileSet::new(None, None);
            let tp = linked.reconcile(&mut ch).into_inner().toolpath;
            let t = compute_cycle_time(&tp, &kin, MAX_FEED_MM_MIN, RAPID_FEED_MM_MIN);
            if let Some(row) = table.get_mut(ri) {
                row.push((
                    t,
                    rep.fragments,
                    rep.retract_links,
                    tp.total_cutting_distance(),
                ));
            }
        }
    }

    let mut base_total = 0.0_f64;
    let mut best_total = 0.0_f64;
    for (ri, poly) in regions.iter().enumerate() {
        let Some(row) = table.get(ri) else { continue };
        let Some(&(base_t, base_f, base_r, _)) = row.first() else {
            continue;
        };
        println!("── region {:.0} mm² ──", poly.area());
        println!(
            "     {:>7}  {:>9}  {:>10}  {:>9}  {:>9}",
            "angle", "time s", "fragments", "retracts", "vs 0deg"
        );
        let mut best = (0.0_f64, f64::INFINITY);
        for (ai, &(t, f, r, _)) in row.iter().enumerate() {
            let angle = ANGLES.get(ai).copied().unwrap_or(0.0);
            if t < best.1 {
                best = (angle, t);
            }
            println!(
                "     {angle:>7.1}  {t:>9.1}  {f:>10}  {r:>9}  {:>8.2}x",
                base_t / t
            );
        }
        println!(
            "     BEST {:.1}deg at {:.1} s vs 0deg {:.1} s ({:.0} fragments, {:.0} retracts at 0deg) = {:.2}x\n",
            best.0,
            best.1,
            base_t,
            base_f as f64,
            base_r as f64,
            base_t / best.1
        );
        base_total += base_t;
        best_total += best.1;
    }
    println!(
        "     TOP-3 TOTAL: 0deg {base_total:.1} s vs per-region BEST angle {best_total:.1} s = {:.2}x\n\
         \n     If this ratio is materially above 1.0, sweep direction IS a real\n\
         lever on this geometry and FINDINGS.md \u{a7}0d's dismissal of Lever 2 was\n\
         wrong. Note the ceiling: one angle per region is still a heuristic —\n\
         a branching web has no single long axis, which is what Morse /\n\
         boustrophedon cell decomposition exists to solve.\n",
        base_total / best_total
    );
}

// ── STAGE F ─────────────────────────────────────────────────────────────
//
// Stage E proved sweep angle is worth ~1.10x, but it FOUND that angle by brute
// force: 12 grid builds + 12 relinks per region, ~2 minutes. That is far too
// expensive to do at plan time for every region.
//
// So: is the winning angle PREDICTABLE from the polygon alone? If the region's
// principal axis (second moments of its interior, free to compute) lands near
// the measured optimum, Lever 2 collapses from "sweep and cost 12 candidates"
// to "compute one number" — and becomes a small change rather than a campaign.
//
// This prints the prediction against Stage E's measured winners so the two can
// be compared directly. It asserts nothing: three regions is not a law.
fn stage_f(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &TaperedBallEndmill,
    planned: &rs_cam_core::finish_planner::PlannedRegions,
    stepover: f64,
    cell: f64,
) {
    use rs_cam_core::geometry::region_set::RegionSet;
    use rs_cam_core::machine::kinematics::{LinkKinematics, MachineKinematics, compute_cycle_time};
    use rs_cam_core::toolpath::raster_toolpath_from_grid;

    println!("========== STAGE F — is the best angle PREDICTABLE from the polygon? ==========\n");

    let mut shallow: Vec<&Polygon2> = planned
        .regions
        .iter()
        .filter(|r| r.band == FinishBand::Shallow)
        .map(|r| &r.polygon)
        .collect();
    shallow.sort_by(|a, b| {
        b.area()
            .partial_cmp(&a.area())
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut minors: Vec<f64> = Vec::new();
    println!(
        "     {:>10}  {:>12}  {:>12}  {:>10}",
        "area mm²", "PCA major", "PCA minor", "elongation"
    );
    for poly in shallow.iter().take(3) {
        // Rasterise the interior and take second moments. Area-weighted by
        // construction (every interior cell counts once), which is what makes
        // this the shape's axis rather than its outline's.
        let [x0, y0, x1, y1] = poly.bbox();
        let nx = (((x1 - x0) / cell).ceil() as usize).saturating_add(2);
        let ny = (((y1 - y0) / cell).ceil() as usize).saturating_add(2);
        let (mut n, mut sx, mut sy) = (0.0_f64, 0.0_f64, 0.0_f64);
        let mut pts: Vec<(f64, f64)> = Vec::new();
        for r in 0..ny {
            for c in 0..nx {
                let x = x0 + c as f64 * cell;
                let y = y0 + r as f64 * cell;
                if poly.contains_point(&P2::new(x, y)) {
                    n += 1.0;
                    sx += x;
                    sy += y;
                    pts.push((x, y));
                }
            }
        }
        if n < 3.0 {
            println!("     {:>10.1}  (too small to fit an axis)", poly.area());
            minors.push(0.0);
            continue;
        }
        let (cx, cy) = (sx / n, sy / n);
        let (mut sxx, mut syy, mut sxy) = (0.0_f64, 0.0_f64, 0.0_f64);
        for (x, y) in &pts {
            let (dx, dy) = (x - cx, y - cy);
            sxx += dx * dx;
            syy += dy * dy;
            sxy += dx * dy;
        }
        sxx /= n;
        syy /= n;
        sxy /= n;
        // Major-axis angle of the covariance matrix.
        let theta = 0.5 * (2.0 * sxy).atan2(sxx - syy);
        let mut major_deg = theta.to_degrees();
        while major_deg < 0.0 {
            major_deg += 180.0;
        }
        while major_deg >= 180.0 {
            major_deg -= 180.0;
        }
        let minor_deg = (major_deg + 90.0) % 180.0;
        minors.push(minor_deg);
        // Eigenvalues -> how elongated the shape is (1.0 = isotropic blob).
        let tr = sxx + syy;
        let det = sxx * syy - sxy * sxy;
        let disc = ((tr * tr / 4.0) - det).max(0.0).sqrt();
        let (l1, l2) = (tr / 2.0 + disc, (tr / 2.0 - disc).max(1e-12));
        println!(
            "     {:>10.1}  {major_deg:>11.1}°  {minor_deg:>11.1}°  {:>9.2}",
            poly.area(),
            (l1 / l2).sqrt()
        );
    }
    // Now COST the predicted angle rather than eyeballing it. The prediction
    // under test is the MINOR axis: Stage E's winners (135deg, 0deg, 45deg) sit
    // near minor, not major — the opposite of the classical "sweep along the
    // long axis" rule. The reason is that this workload is bound by kept
    // RETRACTS, i.e. whether consecutive passes land close enough to relink,
    // not by pass length.
    println!("\n     --- costing the PCA-minor prediction against 0deg ---");
    let kin = MachineKinematics {
        acceleration_mm_s2: MACHINE_ACCEL_SCALAR,
        acceleration_xyz_mm_s2: Some(MACHINE_ACCEL_XYZ),
        junction_deviation_mm: JUNCTION_DEVIATION_MM,
        ..MachineKinematics::default()
    };
    let lk = LinkKinematics {
        kinematics: kin,
        max_feed_mm_min: MAX_FEED_MM_MIN,
        rapid_feed_mm_min: RAPID_FEED_MM_MIN,
    };
    let safe_z = mesh.bbox.max.z + 5.0;
    let effective_min_z = mesh.bbox.min.z - 0.1;
    let (mut tot_base, mut tot_pred) = (0.0_f64, 0.0_f64);
    println!(
        "     {:>10}  {:>10}  {:>10}  {:>10}  {:>9}",
        "area mm2", "minor deg", "t(0deg) s", "t(pred) s", "gain"
    );
    for (poly, minor) in shallow.iter().take(3).zip(minors.iter()) {
        let region = RegionSet::new(vec![(*poly).clone()]);
        let mut times = Vec::new();
        for angle in [0.0_f64, *minor] {
            // 0/90/180/360 take dropcutter's axis-aligned fast path; nudge so a
            // predicted 90deg really does get a rotated grid.
            let a = if (angle - 90.0).abs() < 0.05 {
                89.9
            } else {
                angle
            };
            let grid = rs_cam_core::surface::dropcutter::batch_drop_cutter(
                mesh,
                index,
                cutter,
                stepover,
                a,
                effective_min_z,
            );
            let raster = raster_toolpath_from_grid(
                &grid,
                FEED_MM_MIN,
                PLUNGE_MM_MIN,
                safe_z,
                Some(effective_min_z),
                Some(&region),
            );
            let rp = rs_cam_core::surface_link::RelinkParams {
                hookup_distance: 25.0,
                stock_to_leave: 0.0,
                sampling: 0.5,
                feed_rate: FEED_MM_MIN,
                plunge_rate: PLUNGE_MM_MIN,
                safe_z,
                link_kinematics: Some(&lk),
                reorder: true,
                boundary: Some(&region),
                link_ceiling: None,
                flush_ride: false,
                airborne_links_may_leave_territory: false,
            };
            let (linked, _rep) = rs_cam_core::surface_link::relink_fragments(
                rs_cam_core::toolpath_spans::AnnotatedToolpath::new(raster),
                mesh,
                index,
                cutter,
                &rp,
            );
            let mut ch = rs_cam_core::transform_provenance::ReconcileSet::new(None, None);
            let tp = linked.reconcile(&mut ch).into_inner().toolpath;
            times.push(compute_cycle_time(
                &tp,
                &kin,
                MAX_FEED_MM_MIN,
                RAPID_FEED_MM_MIN,
            ));
        }
        let (t0, tpred) = (times[0], times[1]);
        tot_base += t0;
        tot_pred += tpred;
        println!(
            "     {:>10.1}  {minor:>10.1}  {t0:>10.1}  {tpred:>10.1}  {:>8.2}x",
            poly.area(),
            t0 / tpred
        );
    }
    println!(
        "\n     PCA-MINOR PREDICTION: 0deg {tot_base:.1} s -> predicted {tot_pred:.1} s = {:.2}x",
        tot_base / tot_pred
    );
    println!(
        "     (swept-best ceiling from Stage E was 1.10x)\n\
         If the prediction captures most of that ceiling, Lever 2 is one\n\
         covariance matrix per region - a small change, not a campaign.\n"
    );
}

// ── STAGE G ─────────────────────────────────────────────────────────────
//
// Every number in Stages D-F used `link_ceiling: None` — the FRESH-STOCK arm,
// where a link may ride the mesh surface directly. The live tier 1 is a REST
// op, which passes `Some(LinkCeiling)`, and that changes link behaviour twice
// over (`surface_link.rs`):
//
//   * every kept link is LIFTED to `max(surface, material) + PLUNGE_CLEARANCE`
//     and bracketed by a vertical exit and re-entry — one link becomes three
//     moves with two vertical legs, which is what "walls" look like in the
//     viewport; and
//   * a link is REFUSED outright once that ceiling reaches `safe_z`, because
//     feeding to retract height is strictly worse than the rapid it replaces.
//
// Operator report (2026-08-28): the parallel band still shows "huge walls of
// retracts" after the link fixes. If that is real, it should appear here as a
// large jump in kept retracts the moment a ceiling is in scope.
fn stage_g(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &TaperedBallEndmill,
    planned: &rs_cam_core::finish_planner::PlannedRegions,
    stepover: f64,
) {
    use rs_cam_core::geometry::region_set::RegionSet;
    use rs_cam_core::machine::kinematics::{LinkKinematics, MachineKinematics, compute_cycle_time};
    use rs_cam_core::surface_link::LinkCeiling;
    use rs_cam_core::toolpath::raster_toolpath_from_grid;

    println!("========== STAGE G — what a REST op's link ceiling costs (the walls) ==========\n");

    let kin = MachineKinematics {
        acceleration_mm_s2: MACHINE_ACCEL_SCALAR,
        acceleration_xyz_mm_s2: Some(MACHINE_ACCEL_XYZ),
        junction_deviation_mm: JUNCTION_DEVIATION_MM,
        ..MachineKinematics::default()
    };
    let lk = LinkKinematics {
        kinematics: kin,
        max_feed_mm_min: MAX_FEED_MM_MIN,
        rapid_feed_mm_min: RAPID_FEED_MM_MIN,
    };
    let safe_z = mesh.bbox.max.z + 5.0;
    let effective_min_z = mesh.bbox.min.z - 0.1;
    let tool_r = cutter.envelope_radius_mm();

    let mut shallow: Vec<&Polygon2> = planned
        .regions
        .iter()
        .filter(|r| r.band == FinishBand::Shallow)
        .map(|r| &r.polygon)
        .collect();
    shallow.sort_by(|a, b| {
        b.area()
            .partial_cmp(&a.area())
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let grid = rs_cam_core::surface::dropcutter::batch_drop_cutter(
        mesh,
        index,
        cutter,
        stepover,
        0.0,
        effective_min_z,
    );

    // Three ceiling regimes, worst to best:
    //   none            - fresh stock, what Stages D-F measured
    //   mesh-top        - the analytic fresh-stock fallback the code uses when
    //                     no dexel snapshot is in scope (`fallback_top_z`)
    //   surface-hugging - a prior finishing tier has already cut to near the
    //                     design surface, so material stands just above it
    let regimes: [(&str, Option<f64>); 3] = [
        ("no ceiling", None),
        ("ceiling @ mesh top", Some(mesh.bbox.max.z)),
        ("ceiling @ surface+0.1", Some(mesh.bbox.min.z)),
    ];

    println!(
        "     {:>10}  {:>22}  {:>9}  {:>9}  {:>9}  {:>10}",
        "area mm²", "regime", "linked", "retracts", "time s", "vs none"
    );
    for poly in shallow.iter().take(3) {
        let region = RegionSet::new(vec![(*poly).clone()]);
        let mut base = 0.0_f64;
        for (label, fallback) in regimes {
            let raster = raster_toolpath_from_grid(
                &grid,
                FEED_MM_MIN,
                PLUNGE_MM_MIN,
                safe_z,
                Some(effective_min_z),
                Some(&region),
            );
            let ceiling = fallback.map(|top| LinkCeiling {
                stock: None,
                tool_radius: tool_r,
                fallback_top_z: top,
            });
            let rp = rs_cam_core::surface_link::RelinkParams {
                hookup_distance: 25.0,
                stock_to_leave: 0.0,
                sampling: 0.5,
                feed_rate: FEED_MM_MIN,
                plunge_rate: PLUNGE_MM_MIN,
                safe_z,
                link_kinematics: Some(&lk),
                reorder: true,
                boundary: Some(&region),
                link_ceiling: ceiling,
                flush_ride: false,
                // Tier ops set this true (unified_finish); it only matters when
                // a ceiling is present, which is the whole point here.
                airborne_links_may_leave_territory: ceiling.is_some(),
            };
            let (linked, rep) = rs_cam_core::surface_link::relink_fragments(
                rs_cam_core::toolpath_spans::AnnotatedToolpath::new(raster),
                mesh,
                index,
                cutter,
                &rp,
            );
            let mut ch = rs_cam_core::transform_provenance::ReconcileSet::new(None, None);
            let tp = linked.reconcile(&mut ch).into_inner().toolpath;
            let t = compute_cycle_time(&tp, &kin, MAX_FEED_MM_MIN, RAPID_FEED_MM_MIN);
            if label == "no ceiling" {
                base = t;
            }
            println!(
                "     {:>10.1}  {label:>22}  {:>9}  {:>9}  {t:>9.1}  {:>9.2}x",
                poly.area(),
                rep.surface_links,
                rep.retract_links,
                base / t
            );
        }
        println!();
    }
    println!(
        "     If 'ceiling @ mesh top' shows retracts jumping and time rising, the\n\
         operator's 'walls of retracts' are the ceiling REFUSING links, not the\n\
         raster fragmenting — and the lever is the ceiling's height source, not\n\
         the path pattern. Stages D-F all measured the 'no ceiling' row, so\n\
         their margins are optimistic for a rest op by whatever this gap is.\n"
    );
}

// ── STAGE H ─────────────────────────────────────────────────────────────
//
// Stage G showed the ceiling's HEIGHT dominates (1.57x swing, same path). So
// what sets it in production? `compute/execute.rs` builds the ceiling with
// `tool_radius: ctx.tool_def.envelope_radius_mm()` and
// `max_conservative_top_z_in_disc` takes the MAX over that whole disc, with a
// half-cell dilation on top ("may only ever err high").
//
// For a TAPERED ball that flat-disc model is the wrong shape. The tool is not a
// cylinder: past the ball it climbs at 1/tan(alpha). Material at lateral offset
// r can only strike the tool if it stands HIGHER than `height_at_radius(r)`
// above the tip. So the honest clearance is
//     tip_z >= max over r of [ material_top(r) - height_at_radius(r) ]
// and the flat disc over-lifts by exactly the `height_at_radius(r)` it ignores.
//
// This prints the profile against the board's own relief, so the over-reach is
// a measured number rather than an argument.
fn stage_h(cutter: &TaperedBallEndmill, mesh: &TriangleMesh) {
    println!(
        "========== STAGE H — is the ceiling's disc radius physically justified? ==========\n"
    );
    let relief = mesh.bbox.max.z - mesh.bbox.min.z;
    let env = cutter.envelope_radius_mm();
    let cusp = cutter.cusp_radius_mm();
    println!("   board relief (max possible standing material): {relief:.2} mm");
    println!("   tool: cusp/tip radius {cusp:.2} mm, ENVELOPE radius {env:.2} mm");
    println!("   production reads the ceiling over a disc of the ENVELOPE radius.\n");
    println!(
        "     {:>10}  {:>16}  {:>34}",
        "offset r", "tool height", "can material at r reach the tool?"
    );
    let mut relevant = 0.0_f64;
    let mut r = 0.0_f64;
    while r <= env + 1e-9 {
        match cutter.height_at_radius(r) {
            Some(h) => {
                let reachable = h <= relief;
                if reachable {
                    relevant = r;
                }
                println!(
                    "     {r:>9.2}mm  {h:>14.2}mm  {:>34}",
                    if reachable {
                        "YES - within the board's relief"
                    } else {
                        "no - tool is above any material"
                    }
                );
            }
            None => println!("     {r:>9.2}mm  {:>16}  {:>34}", "(past shaft)", "-"),
        }
        r += env / 6.0;
    }
    println!(
        "\n   Only material within ~{relevant:.2} mm laterally can physically strike this\n\
       tool on this board, but the ceiling is read over {env:.2} mm — a {:.1}x\n\
       over-reach in RADIUS, which on terrain pulls in ridges that cannot touch\n\
       the cutter and lifts every link to their height.\n\
       \n   This is the same radius-semantics class the repo's radius programme\n\
       tracks (envelope where a tip/profile scale belongs). The fix is to read\n\
       the ceiling against the tool PROFILE, not a flat disc; Stage G bounds the\n\
       prize at up to 1.57x on the shallow band, and it would apply to EVERY\n\
       relinked op, not just this one.\n",
        if relevant > 1e-9 {
            env / relevant
        } else {
            f64::INFINITY
        }
    );
}

// ── STAGE I ─────────────────────────────────────────────────────────────
//
// C1 asks a deliberately narrow question before any production decomposition:
// on the lattice the Shallow raster actually emits, how many y-monotone
// boustrophedon cells are present?  A cell owns one contiguous run per scan
// row.  At a split or merge both sides start new cells; otherwise the sole
// overlapping run continues its cell.  This is the standard sweep-line cell
// event rule, sampled at the emitted raster lattice rather than pretending
// that a test-local mask is an exact `Polygon2` implementation.
//
// The output cells are reconstructed as polygons only to feed the existing
// raster generator in Stage J.  Stage J verifies their union selects exactly
// the same emitted grid points as the parent region before it costs anything.

struct RegionCells {
    boundary: Polygon2,
    cells: Vec<Polygon2>,
    topology_cells: usize,
}

struct GridRun {
    start: usize,
    end: usize,
    cell: usize,
}

fn grid_for_direction(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &TaperedBallEndmill,
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

fn grid_for_stage_i(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &TaperedBallEndmill,
    stepover: f64,
) -> rs_cam_core::surface::dropcutter::DropCutterGrid {
    grid_for_direction(mesh, index, cutter, stepover, 0.0)
}

fn runs_in_row(mask: &[bool], row: usize, cols: usize) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut start = None;
    for col in 0..cols {
        let inside = mask[row * cols + col];
        if inside && start.is_none() {
            start = Some(col);
        } else if !inside && let Some(first) = start.take() {
            out.push((first, col - 1));
        }
    }
    if let Some(first) = start {
        out.push((first, cols - 1));
    }
    out
}

fn runs_overlap(a: (usize, usize), b: (usize, usize)) -> bool {
    a.0 <= b.1 && b.0 <= a.1
}

fn grid_frame_to_world(grid: &rs_cam_core::surface::dropcutter::DropCutterGrid, point: P2) -> P2 {
    let angle = grid.direction_deg.to_radians();
    let (cosine, sine) = (angle.cos(), angle.sin());
    P2::new(
        point.x * cosine - point.y * sine,
        point.x * sine + point.y * cosine,
    )
}

fn polygons_for_lattice_cell(
    grid: &rs_cam_core::surface::dropcutter::DropCutterGrid,
    positions: &[usize],
) -> Vec<Polygon2> {
    let rows = grid.rows.saturating_add(2);
    let cols = grid.cols.saturating_add(2);
    let mut mask = vec![false; rows * cols];
    for &position in positions {
        let row = position / grid.cols;
        let col = position % grid.cols;
        mask[(row + 1) * cols + col + 1] = true;
    }
    // Marching squares works in the grid's U/V sampling frame.  The grid is
    // world-aligned at 0°, but a Stage-K candidate is rotated, so map its
    // loops back to world XY before using them as `RegionSet` boundaries.
    let loops = marching_squares_bool_grid(
        &mask,
        rows,
        cols,
        grid.u_start - grid.x_step,
        grid.v_start - grid.y_step,
        grid.x_step,
    );
    // Keep one-lattice-point cells too: Stage J rejects any reconstructed
    // candidate that loses a baseline emitted point, so an area floor would
    // hide a comparison error rather than make the cell set healthier.
    let candidates = loops
        .into_iter()
        .filter(|points| points.len() >= 3 && shoelace_area(points).abs() > 1e-12)
        .map(|points| {
            Polygon2::new(
                points
                    .into_iter()
                    .map(|point| grid_frame_to_world(grid, point))
                    .collect(),
            )
        })
        .collect();
    let mut polygons = detect_containment(candidates);
    for polygon in &mut polygons {
        polygon.ensure_winding();
    }
    polygons
}

fn lattice_boustrophedon_cells(
    grid: &rs_cam_core::surface::dropcutter::DropCutterGrid,
    boundary: &Polygon2,
    min_z: f64,
) -> (Vec<Polygon2>, usize) {
    const CLAMP_EPS: f64 = 0.001;
    let mut inside = vec![false; grid.rows * grid.cols];
    for row in 0..grid.rows {
        for col in 0..grid.cols {
            let point = grid.get(row, col);
            inside[row * grid.cols + col] =
                point.z > min_z + CLAMP_EPS && boundary.contains_point(&P2::new(point.x, point.y));
        }
    }

    let mut positions: Vec<Vec<usize>> = Vec::new();
    let mut previous: Vec<GridRun> = Vec::new();
    for row in 0..grid.rows {
        let current_runs = runs_in_row(&inside, row, grid.cols);
        let old_to_new: Vec<usize> = previous
            .iter()
            .map(|old| {
                current_runs
                    .iter()
                    .filter(|run| runs_overlap((old.start, old.end), **run))
                    .count()
            })
            .collect();
        let mut current = Vec::new();
        for run in current_runs {
            let connected: Vec<usize> = previous
                .iter()
                .enumerate()
                .filter(|(_, old)| runs_overlap((old.start, old.end), run))
                .map(|(i, _)| i)
                .collect();
            let cell = if connected.len() == 1 && old_to_new.get(connected[0]).copied() == Some(1) {
                previous[connected[0]].cell
            } else {
                positions.push(Vec::new());
                positions.len() - 1
            };
            for col in run.0..=run.1 {
                let position = row * grid.cols + col;
                if let Some(cell_positions) = positions.get_mut(cell) {
                    cell_positions.push(position);
                }
            }
            current.push(GridRun {
                start: run.0,
                end: run.1,
                cell,
            });
        }
        previous = current;
    }

    let topology_cells = positions.len();
    let cells = positions
        .iter()
        .flat_map(|positions| polygons_for_lattice_cell(grid, positions))
        .collect();
    (cells, topology_cells)
}

fn pca_minor_and_elongation(poly: &Polygon2, cell: f64) -> Option<(f64, f64)> {
    let [x0, y0, x1, y1] = poly.bbox();
    let nx = (((x1 - x0) / cell).ceil() as usize).saturating_add(2);
    let ny = (((y1 - y0) / cell).ceil() as usize).saturating_add(2);
    let mut points = Vec::new();
    for row in 0..ny {
        for col in 0..nx {
            let point = P2::new(x0 + col as f64 * cell, y0 + row as f64 * cell);
            if poly.contains_point(&point) {
                points.push(point);
            }
        }
    }
    if points.len() < 3 {
        return None;
    }
    let n = points.len() as f64;
    let (sum_x, sum_y) = points.iter().fold((0.0_f64, 0.0_f64), |(sx, sy), point| {
        (sx + point.x, sy + point.y)
    });
    let (cx, cy) = (sum_x / n, sum_y / n);
    let (sxx, syy, sxy) = points
        .iter()
        .fold((0.0_f64, 0.0_f64, 0.0_f64), |(xx, yy, xy), point| {
            let (dx, dy) = (point.x - cx, point.y - cy);
            (xx + dx * dx, yy + dy * dy, xy + dx * dy)
        });
    let (sxx, syy, sxy) = (sxx / n, syy / n, sxy / n);
    let major = 0.5 * (2.0 * sxy).atan2(sxx - syy).to_degrees();
    let minor = (major + 90.0).rem_euclid(180.0);
    let trace = sxx + syy;
    let determinant = sxx * syy - sxy * sxy;
    let spread = ((trace * trace / 4.0) - determinant).max(0.0).sqrt();
    let large = trace / 2.0 + spread;
    let small = trace / 2.0 - spread;
    if small <= 1e-9 {
        return Some((minor, f64::INFINITY));
    }
    Some((minor, (large / small).sqrt()))
}

fn stage_i(
    grid: &rs_cam_core::surface::dropcutter::DropCutterGrid,
    planned: &rs_cam_core::finish_planner::PlannedRegions,
    stepover: f64,
    effective_min_z: f64,
) -> Vec<RegionCells> {
    println!("========== STAGE I — C1 lattice boustrophedon cells (0° raster) ==========\n");
    println!(
        "   Cell rule: y-monotone in the 0° raster's scan direction; paths run X.\n\
         This is a measurement-only lattice decomposition, not production Polygon2 code.\n"
    );
    let mut shallow: Vec<&Polygon2> = planned
        .regions
        .iter()
        .filter(|region| region.band == FinishBand::Shallow)
        .map(|region| &region.polygon)
        .collect();
    shallow.sort_by(|a, b| b.area().total_cmp(&a.area()));

    let mut out = Vec::new();
    for (region_index, boundary) in shallow.into_iter().take(3).enumerate() {
        let (cells, topology_cells) = lattice_boustrophedon_cells(grid, boundary, effective_min_z);
        println!(
            "── region {} ({:.0} mm²): {} topology cells, {} extracted polygons ──",
            region_index + 1,
            boundary.area(),
            topology_cells,
            cells.len()
        );
        println!(
            "     {:>9}  {:>10}  {:>15}  {:>15}",
            "area mm²", "elongation", "PCA minor", "monotone axis"
        );
        for cell in &cells {
            let (minor, elongation) =
                pca_minor_and_elongation(cell, stepover).unwrap_or((f64::NAN, f64::NAN));
            println!(
                "     {:>9.1}  {:>10.2}  {minor:>13.1}°  {:>15}",
                cell.area(),
                elongation,
                "Y (paths X)"
            );
        }
        println!();
        out.push(RegionCells {
            boundary: boundary.clone(),
            cells,
            topology_cells,
        });
    }
    out
}

/// Where Stage I's operator-facing debug SVGs land.  The environment override
/// lets an evidence run put the artifacts straight where the operator asked,
/// while the default remains a disposable build artifact.
fn cell_svg_output_dir() -> PathBuf {
    std::env::var_os("THIN_ORGANIC_SVG_DIR").map_or_else(
        || {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("..")
                .join("target")
                .join("thin_organic")
        },
        PathBuf::from,
    )
}

fn svg_path(poly: &Polygon2) -> String {
    fn append_ring(path: &mut String, ring: &[P2]) {
        let Some(first) = ring.first() else { return };
        write!(path, "M {:.3} {:.3}", first.x, first.y).expect("write SVG path");
        for point in &ring[1..] {
            write!(path, " L {:.3} {:.3}", point.x, point.y).expect("write SVG path");
        }
        path.push_str(" Z");
    }

    let mut path = String::new();
    append_ring(&mut path, &poly.exterior);
    for hole in &poly.holes {
        append_ring(&mut path, hole);
    }
    path
}

/// Write one comparison SVG per measured region: a black original boundary
/// below the emitted-lattice monotone cells, colour-cycled by cell identity.
/// The overlay makes splits/merges and tiny one-row cells directly inspectable
/// without pretending it is a production preview.
fn write_cell_svg_comparisons(regions: &[RegionCells]) {
    const COLOURS: [&str; 12] = [
        "#e41a1c", "#377eb8", "#4daf4a", "#984ea3", "#ff7f00", "#ffff33", "#a65628", "#f781bf",
        "#999999", "#66c2a5", "#fc8d62", "#8da0cb",
    ];
    let output_dir = cell_svg_output_dir();
    std::fs::create_dir_all(&output_dir).expect("create cell SVG output directory");
    for (index, region) in regions.iter().enumerate() {
        let [x0, y0, x1, y1] = region.boundary.bbox();
        let padding = 2.0;
        let (view_x, view_y) = (x0 - padding, y0 - padding);
        let (view_w, view_h) = (x1 - x0 + 2.0 * padding, y1 - y0 + 2.0 * padding);
        let mut svg = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"{view_x:.3} {view_y:.3} {view_w:.3} {view_h:.3}\" width=\"1000\" height=\"1000\">\n\
             <title>Wanaka shallow region {}: original boundary and {} monotone cells</title>\n\
             <rect x=\"{view_x:.3}\" y=\"{view_y:.3}\" width=\"{view_w:.3}\" height=\"{view_h:.3}\" fill=\"white\"/>\n",
            index + 1,
            region.cells.len()
        );
        for (cell_index, cell) in region.cells.iter().enumerate() {
            let colour = COLOURS[cell_index % COLOURS.len()];
            writeln!(
                svg,
                "<path d=\"{}\" fill=\"{colour}\" fill-opacity=\"0.45\" stroke=\"{colour}\" stroke-width=\"0.08\" fill-rule=\"evenodd\"/>",
                svg_path(cell)
            )
            .expect("write SVG cell");
        }
        writeln!(
            svg,
            "<path d=\"{}\" fill=\"none\" stroke=\"black\" stroke-width=\"0.20\" fill-rule=\"evenodd\"/>\n</svg>",
            svg_path(&region.boundary)
        )
        .expect("write SVG boundary");
        let path = output_dir.join(format!("wanaka_monotone_cells_region_{}.svg", index + 1));
        std::fs::write(&path, svg).expect("write cell comparison SVG");
        println!("cell comparison SVG: {}", path.display());
    }
}

/// Same overlay as [`write_cell_svg_comparisons`], but named for a candidate
/// direction so it can sit next to the 0° baseline without overwriting it.
fn write_named_cell_svg(region: &RegionCells, filename: &str, title: &str) {
    const COLOURS: [&str; 12] = [
        "#e41a1c", "#377eb8", "#4daf4a", "#984ea3", "#ff7f00", "#ffff33", "#a65628", "#f781bf",
        "#999999", "#66c2a5", "#fc8d62", "#8da0cb",
    ];
    let [x0, y0, x1, y1] = region.boundary.bbox();
    let padding = 2.0;
    let (view_x, view_y) = (x0 - padding, y0 - padding);
    let (view_w, view_h) = (x1 - x0 + 2.0 * padding, y1 - y0 + 2.0 * padding);
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"{view_x:.3} {view_y:.3} {view_w:.3} {view_h:.3}\" width=\"1000\" height=\"1000\">\n\
         <title>{title}</title>\n\
         <rect x=\"{view_x:.3}\" y=\"{view_y:.3}\" width=\"{view_w:.3}\" height=\"{view_h:.3}\" fill=\"white\"/>\n"
    );
    for (cell_index, cell) in region.cells.iter().enumerate() {
        let colour = COLOURS[cell_index % COLOURS.len()];
        writeln!(
            svg,
            "<path d=\"{}\" fill=\"{colour}\" fill-opacity=\"0.45\" stroke=\"{colour}\" stroke-width=\"0.08\" fill-rule=\"evenodd\"/>",
            svg_path(cell)
        )
        .expect("write SVG cell");
    }
    writeln!(
        svg,
        "<path d=\"{}\" fill=\"none\" stroke=\"black\" stroke-width=\"0.20\" fill-rule=\"evenodd\"/>\n</svg>",
        svg_path(&region.boundary)
    )
    .expect("write SVG boundary");
    let path = cell_svg_output_dir().join(filename);
    std::fs::create_dir_all(path.parent().unwrap_or_else(|| Path::new(".")))
        .expect("create cell SVG output directory");
    std::fs::write(&path, svg).expect("write named cell comparison SVG");
    println!("cell comparison SVG: {}", path.display());
}

// ── STAGE J ─────────────────────────────────────────────────────────────
//
// E1's reusable comparison kernel.  Candidate constructors hand it raw
// toolpaths; it applies the SAME production relink to every arm and then the
// F-034 integrator.  A cell candidate may only be compared after the emitted
// lattice membership check below proves it has the baseline's cut population.

// The comparison kernel is PROMOTED (Track M, 2026-09-02): `CandidateCost`,
// `LinkRegime`, `relink_and_cost` and `relink_and_cost_under` now live in
// `rs_cam_core::metrology::costing`, extracted verbatim from this file. The
// two adapters below keep this instrument's original call shape; the feed
// pins ride `CostingFeeds` and are this file's own constants, unchanged.

fn costing_feeds() -> CostingFeeds {
    CostingFeeds {
        feed_mm_min: FEED_MM_MIN,
        plunge_mm_min: PLUNGE_MM_MIN,
        max_feed_mm_min: MAX_FEED_MM_MIN,
        rapid_feed_mm_min: RAPID_FEED_MM_MIN,
    }
}

fn relink_and_cost(
    raw: rs_cam_core::toolpath::Toolpath,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &TaperedBallEndmill,
    boundary: &rs_cam_core::geometry::region_set::RegionSet<'_>,
    kinematics: &rs_cam_core::machine::kinematics::MachineKinematics,
    safe_z: f64,
) -> CandidateCost {
    relink_and_cost_under(
        raw,
        mesh,
        index,
        cutter,
        boundary,
        kinematics,
        LinkRegime::fresh_stock(safe_z),
    )
}

fn relink_and_cost_under(
    raw: rs_cam_core::toolpath::Toolpath,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &TaperedBallEndmill,
    boundary: &rs_cam_core::geometry::region_set::RegionSet<'_>,
    kinematics: &rs_cam_core::machine::kinematics::MachineKinematics,
    regime: LinkRegime<'_>,
) -> CandidateCost {
    let ctx = CostingContext {
        mesh,
        index,
        cutter,
        kinematics: Some(kinematics),
        feeds: costing_feeds(),
    };
    metrology_relink_and_cost_under(&ctx, raw, boundary, &regime)
}

fn raster_candidate(
    grid: &rs_cam_core::surface::dropcutter::DropCutterGrid,
    regions: &[Polygon2],
    safe_z: f64,
    effective_min_z: f64,
) -> rs_cam_core::toolpath::Toolpath {
    use rs_cam_core::geometry::region_set::RegionSet;
    use rs_cam_core::toolpath::{Toolpath, raster_toolpath_from_grid};

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

fn cell_membership_matches(
    grid: &rs_cam_core::surface::dropcutter::DropCutterGrid,
    boundary: &Polygon2,
    cells: &[Polygon2],
    effective_min_z: f64,
) -> bool {
    use rs_cam_core::geometry::region_set::RegionSet;

    let cells = RegionSet::new(cells.to_vec());
    let mut mismatches = 0usize;
    for point in &grid.points {
        let baseline = point.z > effective_min_z + 0.001
            && boundary.contains_point(&P2::new(point.x, point.y));
        let candidate =
            point.z > effective_min_z + 0.001 && cells.contains(&P2::new(point.x, point.y));
        if baseline != candidate {
            mismatches += 1;
        }
    }
    if mismatches > 0 {
        println!(
            "     REFUSE comparison: cell polygons disagree with baseline on {mismatches} emitted points"
        );
    }
    mismatches == 0
}

fn stage_j(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &TaperedBallEndmill,
    grid: &rs_cam_core::surface::dropcutter::DropCutterGrid,
    regions: &[RegionCells],
) {
    use rs_cam_core::geometry::region_set::RegionSet;
    use rs_cam_core::machine::kinematics::MachineKinematics;

    println!("========== STAGE J — E1 fair A/B: undivided vs monotone cells ==========\n");
    println!(
        "   Both arms: same 0° grid, feeds, Shapeoko kinematics, full-region boundary,\n\
         `reorder: true`, `link_ceiling: None`, production relink, then F-034 costing.\n\
         The only candidate difference is the cell ownership/order before BOTH arms relink.\n"
    );
    let kinematics = MachineKinematics {
        acceleration_mm_s2: MACHINE_ACCEL_SCALAR,
        acceleration_xyz_mm_s2: Some(MACHINE_ACCEL_XYZ),
        junction_deviation_mm: JUNCTION_DEVIATION_MM,
        ..MachineKinematics::default()
    };
    let safe_z = mesh.bbox.max.z + 5.0;
    let effective_min_z = mesh.bbox.min.z - 0.1;
    println!(
        "     {:>9}  {:>7}  {:>9}  {:>9}  {:>9}  {:>9}  {:>9}",
        "area mm²", "arm", "fragments", "linked", "retracts", "time s", "cut mm"
    );

    let (mut baseline_retracts, mut cell_retracts) = (0usize, 0usize);
    let (mut baseline_time, mut cell_time) = (0.0_f64, 0.0_f64);
    let mut compared = 0usize;
    for region in regions {
        if region.cells.is_empty() {
            println!("     REFUSE comparison: no extracted cell polygons");
            continue;
        }
        if !cell_membership_matches(grid, &region.boundary, &region.cells, effective_min_z) {
            continue;
        }
        let boundary = RegionSet::new(vec![region.boundary.clone()]);
        let baseline = relink_and_cost(
            raster_candidate(
                grid,
                std::slice::from_ref(&region.boundary),
                safe_z,
                effective_min_z,
            ),
            mesh,
            index,
            cutter,
            &boundary,
            &kinematics,
            safe_z,
        );
        let cells = relink_and_cost(
            raster_candidate(grid, &region.cells, safe_z, effective_min_z),
            mesh,
            index,
            cutter,
            &boundary,
            &kinematics,
            safe_z,
        );
        println!(
            "     {:>9.1}  {:>7}  {:>9}  {:>9}  {:>9}  {:>9.1}  {:>9.0}",
            region.boundary.area(),
            "base",
            baseline.fragments,
            baseline.linked,
            baseline.kept_retracts,
            baseline.time_s,
            baseline.cutting_mm
        );
        println!(
            "     {:>9}  {:>7}  {:>9}  {:>9}  {:>9}  {:>9.1}  {:>9.0}  ({:>3} cells, {} moves)",
            "",
            "cells",
            cells.fragments,
            cells.linked,
            cells.kept_retracts,
            cells.time_s,
            cells.cutting_mm,
            region.topology_cells,
            cells.moves
        );
        baseline_retracts += baseline.kept_retracts;
        cell_retracts += cells.kept_retracts;
        baseline_time += baseline.time_s;
        cell_time += cells.time_s;
        compared += 1;
    }
    if compared == 0 {
        println!("\n     REFUSE total: no cell arm preserved the baseline cut population.\n");
        return;
    }
    println!(
        "\n     TOP-{compared} TOTAL: kept retracts {baseline_retracts} -> {cell_retracts};
         time {baseline_time:.1} s -> {cell_time:.1} s ({:.2}x).\n\
         A reduction is only a C-track premise, not a feature verdict: cells still need\n\
         C2/C3 production geometry/routing and the C4 surface-quality review.  No reduction\n\
         means the relinker already absorbed the cell topology and C2 has no time case.\n",
        baseline_time / cell_time
    );
}

// ── STAGE K ─────────────────────────────────────────────────────────────
//
// The visual objection to Stage I is correct: its cells are optimal only for
// the fixed 0° scan direction.  Before even considering per-cell D1, test the
// cheapest non-global alternative: re-decompose the one region whose PCA
// predictor was actually credible (region 1, elongation > 3) at its PCA-minor
// pass direction.  This creates a new lattice and therefore a new cell map;
// it is not a claim that the old cells can simply be rotated.
fn stage_k(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &TaperedBallEndmill,
    zero_grid: &rs_cam_core::surface::dropcutter::DropCutterGrid,
    zero_regions: &[RegionCells],
    stepover: f64,
) {
    use rs_cam_core::geometry::region_set::RegionSet;
    use rs_cam_core::machine::kinematics::MachineKinematics;

    let Some(zero) = zero_regions.first() else {
        println!("========== STAGE K — SKIP: no Shallow region 1 ==========\n");
        return;
    };
    let Some((pca_minor_deg, elongation)) = pca_minor_and_elongation(&zero.boundary, stepover)
    else {
        println!("========== STAGE K — SKIP: region 1 has no PCA axis ==========\n");
        return;
    };
    println!("========== STAGE K — rotated PCA-minor cell candidate (region 1) ==========\n");
    println!(
        "   Region 1: elongation {elongation:.2}; pass direction {pca_minor_deg:.1}°.\n\
         This is one region-level direction candidate, NOT per-cell D1.\n"
    );

    let rotated_grid = grid_for_direction(mesh, index, cutter, stepover, pca_minor_deg);
    let effective_min_z = mesh.bbox.min.z - 0.1;
    let (cells, topology_cells) =
        lattice_boustrophedon_cells(&rotated_grid, &zero.boundary, effective_min_z);
    let rotated = RegionCells {
        boundary: zero.boundary.clone(),
        cells,
        topology_cells,
    };
    println!(
        "   rotated lattice: {} topology cells, {} extracted polygons",
        rotated.topology_cells,
        rotated.cells.len()
    );
    write_named_cell_svg(
        &rotated,
        "wanaka_monotone_cells_region_1_pca_minor.svg",
        &format!("Wanaka shallow region 1: PCA-minor {pca_minor_deg:.1}° monotone cells"),
    );
    if rotated.cells.is_empty()
        || !cell_membership_matches(
            &rotated_grid,
            &rotated.boundary,
            &rotated.cells,
            effective_min_z,
        )
    {
        println!("     REFUSE rotated cost: cell polygons do not preserve the PCA lattice.");
        return;
    }

    let kinematics = MachineKinematics {
        acceleration_mm_s2: MACHINE_ACCEL_SCALAR,
        acceleration_xyz_mm_s2: Some(MACHINE_ACCEL_XYZ),
        junction_deviation_mm: JUNCTION_DEVIATION_MM,
        ..MachineKinematics::default()
    };
    let safe_z = mesh.bbox.max.z + 5.0;
    let boundary = RegionSet::new(vec![zero.boundary.clone()]);
    let zero_base = relink_and_cost(
        raster_candidate(
            zero_grid,
            std::slice::from_ref(&zero.boundary),
            safe_z,
            effective_min_z,
        ),
        mesh,
        index,
        cutter,
        &boundary,
        &kinematics,
        safe_z,
    );
    let zero_cells = relink_and_cost(
        raster_candidate(zero_grid, &zero.cells, safe_z, effective_min_z),
        mesh,
        index,
        cutter,
        &boundary,
        &kinematics,
        safe_z,
    );
    let rotated_base = relink_and_cost(
        raster_candidate(
            &rotated_grid,
            std::slice::from_ref(&rotated.boundary),
            safe_z,
            effective_min_z,
        ),
        mesh,
        index,
        cutter,
        &boundary,
        &kinematics,
        safe_z,
    );
    let rotated_cells = relink_and_cost(
        raster_candidate(&rotated_grid, &rotated.cells, safe_z, effective_min_z),
        mesh,
        index,
        cutter,
        &boundary,
        &kinematics,
        safe_z,
    );
    println!(
        "     {:>16}  {:>9}  {:>9}  {:>9}  {:>9}",
        "candidate", "fragments", "retracts", "time s", "cut mm"
    );
    for (label, cost) in [
        ("0° region", &zero_base),
        ("0° cells", &zero_cells),
        ("PCA region", &rotated_base),
        ("PCA cells", &rotated_cells),
    ] {
        println!(
            "     {label:>16}  {:>9}  {:>9}  {:>9.1}  {:>9.0}",
            cost.fragments, cost.kept_retracts, cost.time_s, cost.cutting_mm
        );
    }
    println!(
        "\n     Read the two within-direction A/Bs first: 0° region → cells and PCA region → cells.\n\
         The 0° ↔ PCA rows deliberately change the raster lattice and require C4 surface review.\n"
    );
}

// ── STAGE L ─────────────────────────────────────────────────────────────
//
// **A3** (`planning/thin_organic_2026-08-27/PROGRAMME.md` Track A).
//
// Every number in FINDINGS §0d–§0h — the refuted contour cascade (0.91×), the
// 1.10× angle sweep, the gated PCA rule, §0g's 1.03× cells and §0h's 1.20×
// rotation-plus-cells — was measured with `link_ceiling: None`. That is the
// FRESH-STOCK arm, where a link rides the mesh directly and pays only its own
// XY hop.
//
// The live tier is a REST op: `wanaka200_mt2.toml` sets `stock_source =
// "from_remaining_stock"` on BOTH finish tiers (ids 16 and 17), so production
// hands `unified_finish` a `Some(LinkCeiling)` (`compute/execute.rs:2359-2368`)
// and every kept link leaves the cut vertically, traverses at
// `max(surface, standing material) + PLUNGE_CLEARANCE_MM` (2.0 mm,
// `toolpath.rs:26`) and re-enters vertically. Stage G measured the ceiling's
// HEIGHT moving one region 898 s → 1411 s (1.57×) with the path held
// identical — a bigger lever than any path topology in this file. So the
// §0g/§0h margins are provisional until the same decision table is re-run
// under a ceiling, and that is all this stage does.
//
// It is additive: no existing stage changes. The E1 kernel keeps its signature
// and its fresh-stock behaviour and gains `relink_and_cost_under`, so BOTH arms
// relink through one site — §0d's lesson, applied to the ceiling comparison.

/// Dexel cell (mm) for the A3 ceiling stock.
///
/// `0.3` is the project's own `cell_mm` on both tier ops' `planned_tier_regions`
/// boundary source (`wanaka200_mt2.toml`), and the cell this instrument already
/// builds its tier map at ([`CELL_MM`]) — so the ceiling is read at the
/// resolution the tier decision itself was made at. The operator's simulation
/// rule (tip Ø ÷ 10, PROGRAMME F1) would ask 0.2 mm for the R1.0; the direction
/// of that difference is known and stated rather than assumed away:
/// `max_clearance_tip_z_for_profile` dilates its disc by half a cell and reads a
/// sliver-safe per-cell upper bound, so a coarser cell can only read the ceiling
/// HIGHER, never lower.
const CEILING_CELL_MM: f64 = CELL_MM;

/// `6mm 2F Carbide End Mill` — tool id 0 in `wanaka200_mt2.toml`, the tool the
/// front rough (`id = 5`, `adaptive3d`, `stock_source = "fresh"`) runs.
const ROUGH_DIAMETER_MM: f64 = 6.0;
/// `cutting_length` of that same tool row.
const ROUGH_CUTTING_LENGTH_MM: f64 = 25.0;
/// `stock_to_leave_axial` on the front rough. The rough's radial leave (0.3) is
/// deliberately NOT modelled: it stands on walls, and a dexel column reads the
/// axial one.
const ROUGH_STOCK_TO_LEAVE_AXIAL_MM: f64 = 0.5;

/// XY margin (mm) the ceiling stock must carry around the measured regions: a
/// link may reach `hookup_distance` = 25.0 mm away from the region, and the
/// ceiling is then read over the R1.0's 3.0 mm envelope radius on top of that
/// — 28.0 mm, rounded up to 30.0 so the bound is not exactly the reach.
const CEILING_STOCK_MARGIN_MM: f64 = 30.0;

/// FINDINGS §0g's recorded FRESH-STOCK arm for the 0° undivided baseline
/// (regions 1–3): `(kept retracts, F-034 seconds)`. Stage L recomputes the
/// fresh arm in the same binary and prints itself against these — if the fresh
/// arm does not reproduce, the setup drifted and no ceiling number in this
/// stage is trustworthy. Printed, never asserted: the mesh is the operator's.
const RECORDED_FRESH_UNDIVIDED: [(usize, f64); 3] = [(97, 1053.7), (30, 478.0), (40, 483.7)];
/// The same for §0g's 0° monotone-cell arm.
const RECORDED_FRESH_CELLS: [(usize, f64); 3] = [(83, 999.9), (33, 492.5), (35, 463.0)];
/// §0h's region-1 PCA-minor pair: undivided, then cells.
const RECORDED_FRESH_PCA: [(usize, f64); 2] = [(74, 963.0), (53, 875.9)];

/// Everything Stage L needs that is not a candidate. A struct rather than
/// parameters because the stage would otherwise cross clippy's
/// `too_many_arguments` bound.
struct A3Inputs<'a> {
    mesh: &'a TriangleMesh,
    index: &'a SpatialIndex,
    /// The tier-1 tool — the one whose links this stage costs (R1.0).
    fine: &'a TaperedBallEndmill,
    /// The tier-0 tool (R1.5), whose achieved surface is most of the ceiling.
    coarse: &'a TaperedBallEndmill,
    /// Production's own ownership field, at `cell_mm`. Used to decide, per
    /// dexel column, WHICH prior op last cut there.
    tier_map: &'a rs_cam_core::maps::tier_map::TierMap,
    zero_grid: &'a rs_cam_core::surface::dropcutter::DropCutterGrid,
    stepover: f64,
}

/// How far past its own tier-map territory the tier-0 op's ball actually
/// sweeps: the island dilation it is handed as a machining boundary
/// (`overlap_mm = 2.0`, [`OVERLAP_MM`]) plus one tip radius, because
/// `containment = "center"` in the toml puts the tool CENTRE on that boundary.
fn coarse_sweep_reach_mm(coarse: &TaperedBallEndmill) -> f64 {
    OVERLAP_MM + coarse.cusp_radius_mm()
}

/// Nearest sample of an **axis-aligned** (`direction_deg == 0.0`) drop-cutter
/// grid to `(x, y)`, or `None` where that sample found no contact — the
/// clamped-to-`min_z` convention every other stage in this file uses.
///
/// Only valid at 0°: `u_start`/`v_start` are world X/Y only there
/// (`dropcutter.rs:45-56`).
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

/// The XY window the ceiling stock has to cover: the measured regions plus
/// [`CEILING_STOCK_MARGIN_MM`], clamped to the mesh footprint. Outside it the
/// dexel query returns `None` and `LinkCeiling` falls back to `fallback_top_z`,
/// which is the conservative (higher) reading — never a silent zero.
fn ceiling_stock_bounds(mesh: &TriangleMesh, regions: &[RegionCells]) -> [f64; 4] {
    let mut x0 = f64::INFINITY;
    let mut y0 = f64::INFINITY;
    let mut x1 = f64::NEG_INFINITY;
    let mut y1 = f64::NEG_INFINITY;
    for region in regions {
        let [rx0, ry0, rx1, ry1] = region.boundary.bbox();
        x0 = x0.min(rx0 - CEILING_STOCK_MARGIN_MM);
        y0 = y0.min(ry0 - CEILING_STOCK_MARGIN_MM);
        x1 = x1.max(rx1 + CEILING_STOCK_MARGIN_MM);
        y1 = y1.max(ry1 + CEILING_STOCK_MARGIN_MM);
    }
    [
        x0.max(mesh.bbox.min.x),
        y0.max(mesh.bbox.min.y),
        x1.min(mesh.bbox.max.x),
        y1.min(mesh.bbox.max.y),
    ]
}

/// **The realistic ceiling: MACHINED STOCK**, built as the two ops that
/// actually precede tier 1 in the operator's project left it.
///
/// The chain in `wanaka200_mt2.toml`'s front setup is `id 5` (adaptive3d rough,
/// Ø6 flat, `stock_to_leave_axial = 0.5`) → `id 16` (tier 0, R1.5) → `id 17`
/// (tier 1, R1.0 — the op this instrument measures). So per dexel column:
///
/// 1. **rough layer, everywhere the mesh is under it:** the Ø6 flat endmill's
///    drop-cutter surface plus its axial leave. A drop-cutter surface IS a
///    flat mill's achieved surface, so this is the modelling step with the
///    least slack in it.
/// 2. **coarse layer, where tier 0 machines:** the R1.5's drop-cutter surface,
///    applied where the tier map labels the column tier 0 **or** it lies within
///    [`coarse_sweep_reach_mm`] of a tier-0 cell (the island overlap plus the
///    tool's own tip radius). Inside tier 1's own territory the coarse op is
///    boundary-excluded, which is exactly why standing material is left there —
///    that residual is the reason tier 1 exists at all.
///
/// Three honest departures from the live chain, all stated rather than hidden:
/// the rough's *toolpath* is not replayed (its drop-cutter surface is used, so
/// stepover cusps and un-entered pockets are missing, both of which would raise
/// the ceiling); the coarse layer likewise uses a per-column surface rather
/// than a stamped sweep (a per-column reading under-reports inter-pass cusps by
/// at most the 30 µm dial, and the ceiling query takes a max over a 3 mm disc
/// which recovers most of the envelope anyway); and the alignment-pin drill and
/// the two back-face V-bit ops are not modelled because they are on the other
/// face. All three biases point the SAME way — this ceiling is the optimistic
/// end of the bracket. The pessimistic end is the flat-top arm Stage L also
/// runs.
fn a3_machined_stock(
    input: &A3Inputs<'_>,
    bounds: [f64; 4],
) -> rs_cam_core::dexel_stock::TriDexelStock {
    use rs_cam_core::dexel_stock::TriDexelStock;
    use rs_cam_core::tool::FlatEndmill;

    let mesh = input.mesh;
    let min_z = mesh.bbox.min.z - 0.1;
    let block_top_z = mesh.bbox.max.z;
    let [x0, y0, x1, y1] = bounds;
    let mut stock = TriDexelStock::from_stock(
        x0,
        y0,
        x1,
        y1,
        mesh.bbox.min.z - 1.0,
        block_top_z,
        CEILING_CELL_MM,
    );

    let rough_tool = FlatEndmill::new(ROUGH_DIAMETER_MM, ROUGH_CUTTING_LENGTH_MM);
    let rough_grid = rs_cam_core::surface::dropcutter::batch_drop_cutter(
        mesh,
        input.index,
        &rough_tool,
        CEILING_CELL_MM,
        0.0,
        min_z,
    );
    let coarse_grid = rs_cam_core::surface::dropcutter::batch_drop_cutter(
        mesh,
        input.index,
        input.coarse,
        CEILING_CELL_MM,
        0.0,
        min_z,
    );

    // Distance (in cells) from every tier-map cell to the nearest tier-0 cell.
    // One EDT beats a per-column point-in-polygon test against the island set
    // by orders of magnitude, and it reads the SAME ownership field the tier
    // ops' boundaries are derived from.
    let map = input.tier_map;
    let tier_zero: Vec<bool> = map.labels.iter().map(|&label| label == 0).collect();
    let distance_to_tier_zero = distance_transform_2d(&tier_zero, map.ny, map.nx);
    let reach = coarse_sweep_reach_mm(input.coarse);

    let rows = stock.z_grid.rows;
    let cols = stock.z_grid.cols;
    let origin_x = stock.z_grid.origin_u;
    let origin_y = stock.z_grid.origin_v;
    let cell = stock.z_grid.cell_size;
    let mut machined = 0usize;
    let mut coarse_columns = 0usize;
    for row in 0..rows {
        let y = origin_y + row as f64 * cell;
        for col in 0..cols {
            let x = origin_x + col as f64 * cell;
            // No mesh under this column: nothing machined it, so it keeps the
            // full block. That is material, not an absence of information.
            let Some(rough_z) = nearest_contact_z(&rough_grid, x, y, min_z) else {
                continue;
            };
            let mut top = rough_z + ROUGH_STOCK_TO_LEAVE_AXIAL_MM;
            let coarse_here = map.nearest_cell(x, y).is_some_and(|(map_row, map_col)| {
                distance_to_tier_zero[map_row * map.nx + map_col] * map.cell_mm <= reach
            });
            if coarse_here && let Some(coarse_z) = nearest_contact_z(&coarse_grid, x, y, min_z) {
                top = top.min(coarse_z);
                coarse_columns += 1;
            }
            if top < block_top_z {
                stock.clear_above_at(row, col, top as f32);
                machined += 1;
            }
        }
    }
    println!(
        "   ceiling stock: {rows} x {cols} columns at {cell:.2} mm, block top {block_top_z:.3} mm;\n\
         {machined} columns machined, of which {coarse_columns} were reached by the tier-0 pass."
    );
    stock
}

/// Print the CEILING'S OWN POPULATION before any candidate is costed.
///
/// CLAUDE.md's standing rule: a gate handed an empty population passes and
/// looks healthy. The same applies to a ceiling — an arm whose ceiling never
/// lifts anything is not evidence that ceilings are cheap, it is evidence the
/// ceiling was not built. This prints, over region 1's emitted lattice, how far
/// above the cut surface a link would have to fly, and how often that clearance
/// reaches `safe_z` (the refusal condition).
fn report_ceiling_population(
    stock: &rs_cam_core::dexel_stock::TriDexelStock,
    input: &A3Inputs<'_>,
    region: &Polygon2,
    safe_z: f64,
) {
    let min_z = input.mesh.bbox.min.z - 0.1;
    let envelope = input.fine.envelope_radius_mm();
    let fallback_top_z = input.mesh.bbox.max.z;
    let mut lifts = Vec::new();
    let mut standing = Vec::new();
    let mut refusals = 0usize;
    for point in &input.zero_grid.points {
        if point.z <= min_z + 0.001 || !region.contains_point(&P2::new(point.x, point.y)) {
            continue;
        }
        let tip = stock
            .max_clearance_tip_z_for_profile(point.x, point.y, envelope, input.fine)
            .unwrap_or(fallback_top_z);
        let clear = point.z.max(tip) + rs_cam_core::toolpath::PLUNGE_CLEARANCE_MM;
        if clear >= safe_z {
            refusals += 1;
        }
        lifts.push(clear - point.z);
        standing.push((tip - point.z).max(0.0));
    }
    if lifts.is_empty() {
        println!("     REFUSE ceiling population: no emitted lattice point inside region 1.\n");
        return;
    }
    lifts.sort_by(f64::total_cmp);
    standing.sort_by(f64::total_cmp);
    let n = lifts.len();
    let mean = lifts.iter().sum::<f64>() / n as f64;
    println!(
        "   ceiling population over region 1's emitted lattice ({n} points):\n\
         \x20    link height above the cut surface: mean {mean:.3}, p50 {:.3}, p90 {:.3}, max {:.3} mm\n\
         \x20    of which STANDING MATERIAL (the rest is the fixed 2.00 mm PLUNGE_CLEARANCE):\n\
         \x20      p50 {:.3}, p90 {:.3}, max {:.3} mm\n\
         \x20    clearance at or above safe_z (link refused outright): {refusals} of {n}",
        percentile(&lifts, 0.5),
        percentile(&lifts, 0.9),
        lifts.last().copied().unwrap_or(f64::NAN),
        percentile(&standing, 0.5),
        percentile(&standing, 0.9),
        standing.last().copied().unwrap_or(f64::NAN),
    );
    println!(
        "     Read the refusal count with its mechanism: this stock's top is bounded by the\n\
         block top (mesh max Z) and safe_z sits 5 mm above that, so `ceiling_above_safe_z`\n\
         is near-structurally 0 here. The ceiling's cost in this regime is NOT refusal — it\n\
         is the two vertical legs per kept link (plunge {PLUNGE_MM_MIN:.0} mm/min) and the\n\
         links the F-034 gate then declines as slower_than_retract.\n"
    );
}

/// Cost one candidate toolpath under several link regimes, through the E1
/// kernel. The candidate is rebuilt per arm so no arm sees another's toolpath.
fn cost_under_arms(
    input: &A3Inputs<'_>,
    grid: &rs_cam_core::surface::dropcutter::DropCutterGrid,
    polygons: &[Polygon2],
    boundary: &rs_cam_core::geometry::region_set::RegionSet<'_>,
    kinematics: &rs_cam_core::machine::kinematics::MachineKinematics,
    arms: &[LinkRegime<'_>],
) -> Vec<CandidateCost> {
    let effective_min_z = input.mesh.bbox.min.z - 0.1;
    arms.iter()
        .map(|arm| {
            let raw = raster_candidate(grid, polygons, arm.safe_z, effective_min_z);
            relink_and_cost_under(
                raw,
                input.mesh,
                input.index,
                input.fine,
                boundary,
                kinematics,
                *arm,
            )
        })
        .collect()
}

/// One printed row: a candidate under one regime.
fn print_a3_row(
    candidate: &str,
    arm: &LinkRegime<'_>,
    cells: Option<usize>,
    cost: &CandidateCost,
    recorded: Option<(usize, f64)>,
) {
    let cell_text = cells.map_or_else(|| "-".to_owned(), |count| count.to_string());
    let check = match recorded {
        None => String::new(),
        Some((retracts, seconds)) => {
            let reproduces = cost.kept_retracts == retracts
                && (cost.time_s - seconds).abs() <= 0.01 * seconds.max(1.0);
            if reproduces {
                format!("  = FINDINGS {retracts} / {seconds:.1} s")
            } else {
                format!("  MISMATCH vs FINDINGS {retracts} / {seconds:.1} s")
            }
        }
    };
    println!(
        "     {candidate:>16}  {:>8}  {cell_text:>5}  {:>9}  {:>7}  {:>8}  {:>9.1}  {:>8.0}  {:>7}  {:>7}{check}",
        arm.label,
        cost.fragments,
        cost.linked,
        cost.kept_retracts,
        cost.time_s,
        cost.cutting_mm,
        cost.slower_than_retract,
        cost.ceiling_above_safe_z,
    );
}

fn print_a3_header() {
    println!(
        "     {:>16}  {:>8}  {:>5}  {:>9}  {:>7}  {:>8}  {:>9}  {:>8}  {:>7}  {:>7}",
        "candidate",
        "arm",
        "cells",
        "fragments",
        "linked",
        "retracts",
        "time s",
        "cut mm",
        "slower",
        "refused"
    );
}

/// `undivided / celled` — the delta the C-track candidate buys in one arm.
fn delta(undivided: &CandidateCost, celled: &CandidateCost) -> f64 {
    if celled.time_s > 0.0 {
        undivided.time_s / celled.time_s
    } else {
        f64::NAN
    }
}

fn stage_l(input: &A3Inputs<'_>, regions: &[RegionCells]) {
    use rs_cam_core::geometry::region_set::RegionSet;
    use rs_cam_core::machine::kinematics::MachineKinematics;
    use rs_cam_core::surface_link::LinkCeiling;

    println!("========== STAGE L — A3: the §0g/§0h decision table under a REAL ceiling ==========");
    if regions.is_empty() {
        println!("\n     SKIP: no Shallow regions to re-baseline.\n");
        return;
    }
    let mesh = input.mesh;
    let effective_min_z = mesh.bbox.min.z - 0.1;
    let safe_z = mesh.bbox.max.z + 5.0;
    let block_top_z = mesh.bbox.max.z;
    let kinematics = MachineKinematics {
        acceleration_mm_s2: MACHINE_ACCEL_SCALAR,
        acceleration_xyz_mm_s2: Some(MACHINE_ACCEL_XYZ),
        junction_deviation_mm: JUNCTION_DEVIATION_MM,
        ..MachineKinematics::default()
    };
    println!(
        "\n   CEILING SOURCE: MACHINED STOCK — the rough (Ø{ROUGH_DIAMETER_MM:.0} flat, axial \
         leave {ROUGH_STOCK_TO_LEAVE_AXIAL_MM:.1} mm)\n\
         \x20  then the tier-0 R1.5 where tier 0 machines. Preferred over the flat-top source\n\
         \x20  because a rest op's links fly over what the PRIOR OPS left, not over a block.\n\
         \x20  The flat-top arm is still run on region 1, as the pessimistic bracket.\n\
         \x20  Production settings mirrored on the ceiling arm: link_ceiling Some{{stock, \
         tool_radius = envelope {:.2} mm, fallback_top_z = {block_top_z:.3}}},\n\
         \x20  flush_ride true, airborne_links_may_leave_territory true (unified_finish.rs:2101-2140;\n\
         \x20  ceiling shape from compute/execute.rs:2359-2368), hookup 25.0, reorder true.\n",
        input.fine.envelope_radius_mm()
    );

    let bounds = ceiling_stock_bounds(mesh, regions);
    let stock = a3_machined_stock(input, bounds);
    report_ceiling_population(&stock, input, &regions[0].boundary, safe_z);

    let machined_ceiling = LinkCeiling {
        stock: Some(&stock),
        tool_radius: input.fine.envelope_radius_mm(),
        fallback_top_z: block_top_z,
    };
    let fresh = LinkRegime::fresh_stock(safe_z);
    let ceiling = LinkRegime::rest_op("ceiling", safe_z, machined_ceiling);
    let arms = [fresh, ceiling];

    println!(
        "   Both arms of every row: same grid, same feeds, same Shapeoko envelope, same\n\
         full-region boundary, production relink, F-034 costing. The ONLY difference is\n\
         the link regime. The `= FINDINGS` marks are the fresh arm reproducing §0g/§0h\n\
         in this binary; a MISMATCH there invalidates the ceiling rows above it too.\n"
    );
    print_a3_header();

    let mut fresh_total = [0.0_f64; 2];
    let mut ceiling_total = [0.0_f64; 2];
    let mut fresh_retracts = [0usize; 2];
    let mut ceiling_retracts = [0usize; 2];
    let mut compared = 0usize;
    for (region_index, region) in regions.iter().enumerate() {
        if region.cells.is_empty() {
            println!(
                "     REFUSE region {}: no extracted cell polygons",
                region_index + 1
            );
            continue;
        }
        if !cell_membership_matches(
            input.zero_grid,
            &region.boundary,
            &region.cells,
            effective_min_z,
        ) {
            continue;
        }
        let boundary = RegionSet::new(vec![region.boundary.clone()]);
        let undivided = cost_under_arms(
            input,
            input.zero_grid,
            std::slice::from_ref(&region.boundary),
            &boundary,
            &kinematics,
            &arms,
        );
        let celled = cost_under_arms(
            input,
            input.zero_grid,
            &region.cells,
            &boundary,
            &kinematics,
            &arms,
        );
        println!(
            "   ── region {} ({:.0} mm², {} cells) ──",
            region_index + 1,
            region.boundary.area(),
            region.topology_cells
        );
        print_a3_row(
            "0° undivided",
            &arms[0],
            None,
            &undivided[0],
            RECORDED_FRESH_UNDIVIDED.get(region_index).copied(),
        );
        print_a3_row("0° undivided", &arms[1], None, &undivided[1], None);
        print_a3_row(
            "0° cells",
            &arms[0],
            Some(region.cells.len()),
            &celled[0],
            RECORDED_FRESH_CELLS.get(region_index).copied(),
        );
        print_a3_row(
            "0° cells",
            &arms[1],
            Some(region.cells.len()),
            &celled[1],
            None,
        );
        println!(
            "       cells delta: fresh {:.3}x, ceiling {:.3}x  (ceiling / fresh = {:.3})",
            delta(&undivided[0], &celled[0]),
            delta(&undivided[1], &celled[1]),
            delta(&undivided[1], &celled[1]) / delta(&undivided[0], &celled[0])
        );
        fresh_total[0] += undivided[0].time_s;
        fresh_total[1] += celled[0].time_s;
        ceiling_total[0] += undivided[1].time_s;
        ceiling_total[1] += celled[1].time_s;
        fresh_retracts[0] += undivided[0].kept_retracts;
        fresh_retracts[1] += celled[0].kept_retracts;
        ceiling_retracts[0] += undivided[1].kept_retracts;
        ceiling_retracts[1] += celled[1].kept_retracts;
        compared += 1;
    }
    if compared == 0 {
        println!("\n     REFUSE total: no cell arm preserved the baseline cut population.\n");
        return;
    }
    println!(
        "\n   TOP-{compared} TOTAL, 0° undivided → 0° cells:\n\
         \x20    fresh   {:.1} s → {:.1} s ({:.3}x), kept retracts {} → {}\n\
         \x20    ceiling {:.1} s → {:.1} s ({:.3}x), kept retracts {} → {}\n\
         \x20    the ceiling costs the undivided arm {:.1} s ({:.3}x) on its own, path unchanged.\n",
        fresh_total[0],
        fresh_total[1],
        fresh_total[0] / fresh_total[1],
        fresh_retracts[0],
        fresh_retracts[1],
        ceiling_total[0],
        ceiling_total[1],
        ceiling_total[0] / ceiling_total[1],
        ceiling_retracts[0],
        ceiling_retracts[1],
        ceiling_total[0] - fresh_total[0],
        ceiling_total[0] / fresh_total[0],
    );

    stage_l_region_one(input, regions, &arms, &kinematics);
}

/// §0h's region-1 rows — the PCA-minor direction, and the flat-top bracket —
/// re-run under the same two arms.
///
/// The rotated cells are re-derived here rather than borrowed from Stage K:
/// Stage K does not return them, and rotating 0° cells is the invalid operation
/// §0h explicitly refuses. Same grid, same rule, same membership guard.
fn stage_l_region_one(
    input: &A3Inputs<'_>,
    regions: &[RegionCells],
    arms: &[LinkRegime<'_>; 2],
    kinematics: &rs_cam_core::machine::kinematics::MachineKinematics,
) {
    use rs_cam_core::geometry::region_set::RegionSet;
    use rs_cam_core::surface_link::LinkCeiling;

    let Some(region) = regions.first() else {
        return;
    };
    let effective_min_z = input.mesh.bbox.min.z - 0.1;
    let safe_z = input.mesh.bbox.max.z + 5.0;
    let Some((pca_minor_deg, elongation)) =
        pca_minor_and_elongation(&region.boundary, input.stepover)
    else {
        println!("     SKIP region-1 PCA rows: no PCA axis.\n");
        return;
    };
    println!(
        "   ── region 1, §0h's PCA-minor direction {pca_minor_deg:.1}° (elongation \
         {elongation:.2}) ──"
    );
    let rotated_grid = grid_for_direction(
        input.mesh,
        input.index,
        input.fine,
        input.stepover,
        pca_minor_deg,
    );
    let (rotated_cells, rotated_topology) =
        lattice_boustrophedon_cells(&rotated_grid, &region.boundary, effective_min_z);
    let boundary = RegionSet::new(vec![region.boundary.clone()]);
    if rotated_cells.is_empty()
        || !cell_membership_matches(
            &rotated_grid,
            &region.boundary,
            &rotated_cells,
            effective_min_z,
        )
    {
        println!("     REFUSE rotated rows: cell polygons do not preserve the PCA lattice.\n");
    } else {
        let undivided = cost_under_arms(
            input,
            &rotated_grid,
            std::slice::from_ref(&region.boundary),
            &boundary,
            kinematics,
            arms,
        );
        let celled = cost_under_arms(
            input,
            &rotated_grid,
            &rotated_cells,
            &boundary,
            kinematics,
            arms,
        );
        print_a3_row(
            "PCA undivided",
            &arms[0],
            None,
            &undivided[0],
            RECORDED_FRESH_PCA.first().copied(),
        );
        print_a3_row("PCA undivided", &arms[1], None, &undivided[1], None);
        print_a3_row(
            "PCA cells",
            &arms[0],
            Some(rotated_cells.len()),
            &celled[0],
            RECORDED_FRESH_PCA.get(1).copied(),
        );
        print_a3_row(
            "PCA cells",
            &arms[1],
            Some(rotated_cells.len()),
            &celled[1],
            None,
        );
        println!(
            "       ({rotated_topology} topology cells)  cells delta: fresh {:.3}x, ceiling \
             {:.3}x  (ceiling / fresh = {:.3})",
            delta(&undivided[0], &celled[0]),
            delta(&undivided[1], &celled[1]),
            delta(&undivided[1], &celled[1]) / delta(&undivided[0], &celled[0])
        );
    }

    // The pessimistic bracket: no dexel at all, the analytic fresh-stock top —
    // Stage G's "ceiling @ mesh top" regime, but now with the production
    // flush_ride / airborne priors and the profile-aware clearance A1 landed.
    // Production sits BETWEEN this and the machined arm above.
    let flat = LinkRegime::rest_op(
        "flat-top",
        safe_z,
        LinkCeiling {
            stock: None,
            tool_radius: input.fine.envelope_radius_mm(),
            fallback_top_z: input.mesh.bbox.max.z,
        },
    );
    println!("   ── region 1, flat-top ceiling (pessimistic bracket) ──");
    if region.cells.is_empty() {
        println!("     REFUSE flat-top bracket: region 1 has no extracted 0° cell polygons.\n");
        return;
    }
    let flat_arms = [flat];
    let undivided = cost_under_arms(
        input,
        input.zero_grid,
        std::slice::from_ref(&region.boundary),
        &boundary,
        kinematics,
        &flat_arms,
    );
    let celled = cost_under_arms(
        input,
        input.zero_grid,
        &region.cells,
        &boundary,
        kinematics,
        &flat_arms,
    );
    print_a3_row("0° undivided", &flat, None, &undivided[0], None);
    print_a3_row(
        "0° cells",
        &flat,
        Some(region.cells.len()),
        &celled[0],
        None,
    );
    println!(
        "       cells delta under the flat-top bracket: {:.3}x\n",
        delta(&undivided[0], &celled[0])
    );
    println!(
        "   A3 reads the `ceiling / fresh` column: > 1.00 means the ceiling AMPLIFIES the\n\
         cell candidate's advantage (the undivided arm's long hops are penalised harder),\n\
         < 1.00 means it COMPRESSES it. The absolute ceiling-arm seconds are the\n\
         operator-honest figures; the fresh-arm ones never were, on a rest tier.\n"
    );
}

/// A3 re-baseline (`PROGRAMME.md` Track A3). Same focused setup as
/// [`wanaka_monotone_cells_kept_retracts`] — deliberately duplicated rather
/// than factored out, so Stage L is additive and cannot move an existing
/// stage's numbers — then Stage L only.
///
/// ```text
/// cargo test -p rs_cam_core --test thin_organic_island_widths \
///   wanaka_ceiling_rebaseline_a3 -- --ignored --nocapture
/// ```
#[test]
#[ignore = "evidence run — needs the operator's wanaka mesh (not in repo)"]
fn wanaka_ceiling_rebaseline_a3() {
    let path = Path::new(WANAKA_MESH);
    if !path.exists() {
        println!("SKIP: {WANAKA_MESH} not present on this machine.");
        return;
    }

    let mesh = TriangleMesh::from_stl_scaled(path, 1.0).expect("load wanaka terrain");
    let index = SpatialIndex::build_auto(&mesh);
    let r15 = TaperedBallEndmill::new(3.0, 2.8, 6.0, 30.5);
    let r10 = TaperedBallEndmill::new(2.0, 5.7, 6.0, 20.0);
    let tools: [&dyn MillingCutter; 2] = [&r15, &r10];
    let ladder = TierLadder::new(&tools).expect("ladder");
    let never_cancel = || false;
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
    let cusp_radii: Vec<f64> = tools.iter().map(|tool| tool.cusp_radius_mm()).collect();
    let islands = extract_tier_islands(
        &map,
        &TierIslandParams {
            coarseness: COARSENESS,
            overlap_mm: OVERLAP_MM,
            max_regions_per_tier: MAX_REGIONS_PER_TIER,
            ..TierIslandParams::default()
        },
        &cusp_radii,
    )
    .expect("islands");
    let Some(fine) = islands.per_tier.iter().find(|set| set.tier == 1) else {
        println!("SKIP: no tier 1 machining region.");
        return;
    };
    if fine.machining.is_empty() {
        println!("SKIP: tier 1 machining region is empty.");
        return;
    }

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
        .map(|(i, &covered)| {
            if !covered {
                return false;
            }
            let row = i / heightmap.cols;
            let col = i % heightmap.cols;
            fine.machining.contains(&P2::new(
                heightmap.origin_x + col as f64 * heightmap.cell_size,
                heightmap.origin_y + row as f64 * heightmap.cell_size,
            ))
        })
        .collect();
    let mut planner = FinishPlannerParams::for_tool(cusp_radii[1]);
    planner.overlap_mm = OVERLAP_MM;
    let planned = decompose(&surface.slope_map, &covered, &[], &planner);
    let stepover = equal_cusp_stepover_mm(cusp_radii[1], CUSP_HEIGHT_MM);
    println!(
        "A3 setup: {} planned regions, Shallow raster stepover {stepover:.4} mm\n",
        planned.regions.len()
    );

    let grid = grid_for_stage_i(&mesh, &index, &r10, stepover);
    let effective_min_z = mesh.bbox.min.z - 0.1;
    let mut shallow: Vec<&Polygon2> = planned
        .regions
        .iter()
        .filter(|region| region.band == FinishBand::Shallow)
        .map(|region| &region.polygon)
        .collect();
    shallow.sort_by(|a, b| b.area().total_cmp(&a.area()));
    // Same three regions, same cell rule as Stage I — recomputed rather than
    // re-printed, because Stage L compares arms and not cells.
    let regions: Vec<RegionCells> = shallow
        .into_iter()
        .take(3)
        .map(|boundary| {
            let (cells, topology_cells) =
                lattice_boustrophedon_cells(&grid, boundary, effective_min_z);
            RegionCells {
                boundary: boundary.clone(),
                cells,
                topology_cells,
            }
        })
        .collect();

    let input = A3Inputs {
        mesh: &mesh,
        index: &index,
        fine: &r10,
        coarse: &r15,
        tier_map: &map,
        zero_grid: &grid,
        stepover,
    };
    stage_l(&input, &regions);
}

// ── STAGE M ─────────────────────────────────────────────────────────────
//
// **D1** (`planning/thin_organic_2026-08-27/PROGRAMME.md` Track D).
//
// A3 (§0i) left region 1's ranking with ONE global direction per region, on
// the machined-stock ceiling arm — the only operator-honest regime: PCA-cells
// 755.3 s > 0°-cells 772.6 > PCA-undivided 887.6 > 0°-undivided 917.5. Within
// that result the two levers moved OPPOSITELY: decomposition STRENGTHENED
// under the ceiling (cells delta 1.099× → 1.175× in the PCA direction) while
// the single global rotation COMPRESSED to 1.034×. So the prior going into D1
// is that a direction lever is worth little here, and the question is whether
// per-CELL direction — where a monotone cell is exactly the shape a single
// axis describes — recovers any of it.
//
// Three things separate this stage from K/L, and all three are limitations
// rather than features:
//
// 1. **Every cell gets its OWN LATTICE.** There is no common grid left, so
//    Stage J's membership guard — the candidate must select exactly the
//    baseline's emitted lattice points — is not merely weaker here, it is
//    UNDEFINED. What stands in its place is a COVERAGE PROXY: emitted lattice
//    points × stepover² against the cell's own polygon area. A candidate that
//    leaves a strip uncut at a cell seam reads low; one that double-covers a
//    seam reads high. That is a gap detector, not a surface-quality
//    acceptance — the C4 rendered/simulated review still binds anything built
//    on this, exactly as §0h says for its cross-direction rows.
// 2. **The per-cell rig changes the lattice ORIGIN as well as its angle.**
//    Each per-cell grid is phased to its own cell, not to the region, so a
//    raw `shared 0° cells → per-cell PCA` comparison confounds phase with
//    direction. The stage therefore runs a PHASE CONTROL — the same per-cell
//    rig with every cell held at 0°. The delta then decomposes:
//    `shared 0° cells → per-cell 0°` is phase/origin alone, `per-cell 0° →
//    per-cell PCA` is direction alone, and their product is the total.
// 3. **The "recorded monotone direction" rule is degenerate on this
//    decomposition.** Stage I's cells are monotone in the SHIPPED 0° raster,
//    so every cell's monotone direction is 0°; that rule is not a second
//    direction candidate, it IS the phase control of point 2. Said plainly
//    rather than dressed up as two rules.
//
// Still NOT a production router: no production cell geometry, no cell
// adjacency graph, no C3 cell TSP, no GUI overlay. Cell VISIT ORDER is the
// decomposition's own emission order, plus a greedy nearest-neighbour variant
// that is a BOUND on what C3 could buy — it has no adjacency information, no
// choice of cell entry/exit point, and the relinker's own `reorder: true` may
// re-sort the fragments underneath it anyway.

/// Rows/columns of margin a per-cell grid carries beyond its own polygon's
/// rotated extent, so a cell's first and last engaging raster row is never the
/// grid's own edge row.
const PER_CELL_GRID_MARGIN_STEPS: usize = 2;

/// The bar D1 asks for: how many cells want a direction more than this far
/// from the region's single global one. An AXIS difference, so it is measured
/// mod 180°.
const D1_ANGLE_DIVERGENCE_DEG: f64 = 15.0;

/// §0f's elongation gate. A region-level PCA direction is only credible above
/// it, and region 1 (4.15) was the only Wanaka Shallow region to pass — so
/// Stage M prints §0i's global-PCA reference rows (region 1's 755.3 s bar)
/// only for a region that clears it, rather than inventing a bar where §0f
/// already refused one.
const PCA_ELONGATION_GATE: f64 = 3.0;

/// Which direction rule a per-cell candidate assigns to each cell.
#[derive(Clone, Copy)]
enum CellDirectionRule {
    /// The cell's recorded MONOTONE direction. Stage I's cells are monotone in
    /// the shipped 0° raster, so this is 0° for every one of them: the rule is
    /// degenerate on this decomposition and the arm it produces is the
    /// per-cell rig's PHASE CONTROL, not a second direction candidate.
    Monotone,
    /// The cell's own PCA-minor axis — §0f's region-level predictor applied
    /// per cell. A cell too small to have one (fewer than three lattice
    /// samples inside it) falls back to 0°, and the count of those is printed
    /// rather than absorbed.
    PcaMinor,
}

/// One cell, its assigned sweep direction, and the lattice that direction
/// implies for it.
struct CellPlan {
    polygon: Polygon2,
    angle_deg: f64,
    /// The cell's own PCA elongation, or NaN when the rule did not ask for an
    /// axis. Printed beside the divergence because it is the datum that would
    /// justify (or kill) a *gated* per-cell rule later — Stage M deliberately
    /// does not build one.
    elongation: f64,
    grid: rs_cam_core::surface::dropcutter::DropCutterGrid,
    /// Points of this cell's own grid that engage the surface inside this
    /// cell's own polygon — the coverage proxy's numerator.
    lattice_points: usize,
}

/// Build ONE cell's drop-cutter lattice at `angle_deg`, covering only that
/// cell.
///
/// Deliberately not `batch_drop_cutter`: that builder covers the whole MESH
/// bbox (179 whole-mesh grids would be absurd), and its per-cell equivalent
/// `batch_sample_grid` is `pub(crate)`. Two consequences, both stated:
///
/// * **the 89.9°-not-90° trap does not apply here.** `batch_drop_cutter`
///   skips its rotation entirely within 0.01° of 0/90/180/360 and returns an
///   AXIS-ALIGNED grid still labelled with the requested angle
///   (`dropcutter.rs:118-165`), which would make `u_start`/`v_start` lie at
///   90°. This builder always applies the honest rotation — at 0° that IS the
///   identity — so `u_start`/`v_start` are true rotated-frame minima at every
///   angle and `points[i].x/.y` are world coordinates at every angle. Nothing
///   downstream of here reads the frame anyway: `raster_toolpath_from_grid`
///   uses only `rows`/`cols`/`get()`/`x_step`/`y_step`
///   (`toolpath.rs:607-748`), and Stage M never re-extracts polygons from a
///   per-cell grid — it uses Stage I's existing cell polygons.
/// * **it samples only inside the polygon.** A lattice point outside the cell
///   would be excluded by the very `RegionSet` this candidate is emitted
///   through, and `RegionSet::contains` is exactly `Polygon2::contains_point`
///   (`region_set.rs:71-73`) — the same predicate, no epsilon — so an
///   unsampled point parked at `min_z` is behaviourally identical to a
///   sampled one that is region-excluded. That turns the sampling cost from
///   "cell bbox" into "cell", which is what makes 179 per-cell grids
///   affordable in a sequential test binary.
fn cell_lattice_grid(
    input: &A3Inputs<'_>,
    polygon: &Polygon2,
    angle_deg: f64,
    min_z: f64,
) -> (rs_cam_core::surface::dropcutter::DropCutterGrid, usize) {
    use rs_cam_core::surface::dropcutter::{DropCutterGrid, point_drop_cutter};
    use rs_cam_core::tool::CLPoint;

    let radians = angle_deg.to_radians();
    let (cosine, sine) = (radians.cos(), radians.sin());
    let step = input.stepover;
    let (mut u_min, mut u_max) = (f64::INFINITY, f64::NEG_INFINITY);
    let (mut v_min, mut v_max) = (f64::INFINITY, f64::NEG_INFINITY);
    for point in &polygon.exterior {
        let u = point.x * cosine + point.y * sine;
        let v = -point.x * sine + point.y * cosine;
        u_min = u_min.min(u);
        u_max = u_max.max(u);
        v_min = v_min.min(v);
        v_max = v_max.max(v);
    }
    if !u_min.is_finite() || !v_min.is_finite() {
        return (
            DropCutterGrid {
                points: Vec::new(),
                rows: 0,
                cols: 0,
                u_start: 0.0,
                v_start: 0.0,
                x_step: step,
                y_step: step,
                direction_deg: angle_deg,
            },
            0,
        );
    }
    let margin = PER_CELL_GRID_MARGIN_STEPS as f64 * step;
    let u_start = u_min - margin;
    let v_start = v_min - margin;
    let cols = ((u_max + margin - u_start) / step).ceil() as usize + 1;
    let rows = ((v_max + margin - v_start) / step).ceil() as usize + 1;
    let mut points = Vec::with_capacity(rows * cols);
    let mut lattice_points = 0usize;
    for row in 0..rows {
        let v = v_start + row as f64 * step;
        for col in 0..cols {
            let u = u_start + col as f64 * step;
            let x = u * cosine - v * sine;
            let y = u * sine + v * cosine;
            if polygon.contains_point(&P2::new(x, y)) {
                let mut cl = point_drop_cutter(x, y, input.mesh, input.index, input.fine);
                if cl.z < min_z {
                    cl.z = min_z;
                }
                if cl.z > min_z + 0.001 {
                    lattice_points += 1;
                }
                points.push(cl);
            } else {
                let mut cl = CLPoint::new(x, y);
                cl.z = min_z;
                points.push(cl);
            }
        }
    }
    (
        DropCutterGrid {
            points,
            rows,
            cols,
            u_start,
            v_start,
            x_step: step,
            y_step: step,
            direction_deg: angle_deg,
        },
        lattice_points,
    )
}

/// Assign every cell a direction under `rule` and build its lattice. Returns
/// the plans and the number of cells that had NO PCA axis and fell back to 0°.
fn plan_cells(
    input: &A3Inputs<'_>,
    cells: &[Polygon2],
    rule: CellDirectionRule,
    min_z: f64,
) -> (Vec<CellPlan>, usize) {
    let mut fallbacks = 0usize;
    let plans = cells
        .iter()
        .map(|cell| {
            let axis = match rule {
                CellDirectionRule::Monotone => None,
                CellDirectionRule::PcaMinor => pca_minor_and_elongation(cell, input.stepover),
            };
            let angle_deg = match (rule, axis) {
                (CellDirectionRule::Monotone, _) => 0.0,
                (CellDirectionRule::PcaMinor, Some((minor, _))) => minor,
                (CellDirectionRule::PcaMinor, None) => {
                    fallbacks += 1;
                    0.0
                }
            };
            let (grid, lattice_points) = cell_lattice_grid(input, cell, angle_deg, min_z);
            CellPlan {
                polygon: cell.clone(),
                angle_deg,
                elongation: axis.map_or(f64::NAN, |(_, elongation)| elongation),
                grid,
                lattice_points,
            }
        })
        .collect();
    (plans, fallbacks)
}

/// Vertex centroid of a cell — the mean of its exterior vertices, NOT the area
/// centroid. Adequate for an ordering heuristic, and named so it cannot be
/// mistaken for a geometric claim.
fn cell_vertex_centroid(polygon: &Polygon2) -> P2 {
    let count = polygon.exterior.len().max(1) as f64;
    let (sum_x, sum_y) = polygon
        .exterior
        .iter()
        .fold((0.0_f64, 0.0_f64), |(sx, sy), point| {
            (sx + point.x, sy + point.y)
        });
    P2::new(sum_x / count, sum_y / count)
}

/// Greedy nearest-neighbour cell order over vertex centroids, starting from
/// the cell the decomposition emitted first.
///
/// **A GREEDY BOUND ON WHAT C3 COULD BUY, NOT A ROUTER.** No adjacency graph,
/// no choice of where a cell is entered or left, no 2-opt, and the production
/// relinker runs with `reorder: true` on top of it — so a null result here is
/// evidence that cell ORDER is not the lever, and a positive one is only an
/// upper hint for C3.
fn nearest_neighbour_cell_order(plans: &[CellPlan]) -> Vec<usize> {
    let mut order = Vec::with_capacity(plans.len());
    if plans.is_empty() {
        return order;
    }
    let centroids: Vec<P2> = plans
        .iter()
        .map(|plan| cell_vertex_centroid(&plan.polygon))
        .collect();
    let mut visited = vec![false; plans.len()];
    let mut current = 0usize;
    visited[0] = true;
    order.push(0);
    for _ in 1..plans.len() {
        let here = centroids[current];
        let mut best: Option<(usize, f64)> = None;
        for (index, centroid) in centroids.iter().enumerate() {
            if visited[index] {
                continue;
            }
            let distance = (centroid.x - here.x).hypot(centroid.y - here.y);
            if best.is_none_or(|(_, best_distance)| distance < best_distance) {
                best = Some((index, distance));
            }
        }
        let Some((next, _)) = best else { break };
        visited[next] = true;
        order.push(next);
        current = next;
    }
    order
}

/// Concatenate every cell's own raster, in `order`, into one candidate.
fn per_cell_raster_candidate(
    plans: &[CellPlan],
    order: &[usize],
    safe_z: f64,
    effective_min_z: f64,
) -> rs_cam_core::toolpath::Toolpath {
    use rs_cam_core::geometry::region_set::RegionSet;
    use rs_cam_core::toolpath::{Toolpath, raster_toolpath_from_grid};

    let mut out = Toolpath::new();
    for &index in order {
        let Some(plan) = plans.get(index) else {
            continue;
        };
        let region = RegionSet::new(vec![plan.polygon.clone()]);
        let toolpath = raster_toolpath_from_grid(
            &plan.grid,
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

/// [`cost_under_arms`] for a per-cell candidate: same E1 kernel, same
/// production relink parameters, rebuilt per arm so no arm sees another's
/// toolpath.
fn cost_plans_under_arms(
    input: &A3Inputs<'_>,
    plans: &[CellPlan],
    order: &[usize],
    boundary: &rs_cam_core::geometry::region_set::RegionSet<'_>,
    kinematics: &rs_cam_core::machine::kinematics::MachineKinematics,
    arms: &[LinkRegime<'_>],
) -> Vec<CandidateCost> {
    let effective_min_z = input.mesh.bbox.min.z - 0.1;
    arms.iter()
        .map(|arm| {
            let raw = per_cell_raster_candidate(plans, order, arm.safe_z, effective_min_z);
            relink_and_cost_under(
                raw,
                input.mesh,
                input.index,
                input.fine,
                boundary,
                kinematics,
                *arm,
            )
        })
        .collect()
}

/// AXIS difference (mod 180°) between two sweep directions, in degrees. A
/// raster at 179° and one at 1° differ by 2°, not 178° — they sweep the same
/// family of lines.
fn axis_difference_deg(a: f64, b: f64) -> f64 {
    let difference = (a - b).rem_euclid(180.0);
    difference.min(180.0 - difference)
}

/// D1's structural question, printed before any cost: how far do the cells'
/// own preferred axes actually sit from the region's single global one? If
/// few of them diverge, a null result on the cost rows is EXPLAINED rather
/// than merely observed.
fn report_cell_angle_distribution(plans: &[CellPlan], global_deg: Option<f64>, fallbacks: usize) {
    if plans.is_empty() {
        println!("     per-cell angles: no cells.");
        return;
    }
    let mut bins = [0usize; 6];
    for plan in plans {
        let bin = ((plan.angle_deg.rem_euclid(180.0)) / 30.0).floor() as usize;
        bins[bin.min(5)] += 1;
    }
    let mut elongations: Vec<f64> = plans
        .iter()
        .map(|plan| plan.elongation)
        .filter(|value| value.is_finite())
        .collect();
    elongations.sort_by(f64::total_cmp);
    let gated = elongations
        .iter()
        .filter(|value| **value > PCA_ELONGATION_GATE)
        .count();
    println!(
        "     per-cell PCA-minor angles over {} cells ({fallbacks} had no axis and fell back to 0°):\n\
         \x20      0-30° {}, 30-60° {}, 60-90° {}, 90-120° {}, 120-150° {}, 150-180° {}",
        plans.len(),
        bins[0],
        bins[1],
        bins[2],
        bins[3],
        bins[4],
        bins[5],
    );
    if !elongations.is_empty() {
        println!(
            "     per-cell elongation: p50 {:.2}, p90 {:.2}, max {:.2}; {gated} of {} cells clear \
             §0f's gate ({PCA_ELONGATION_GATE:.1})",
            percentile(&elongations, 0.5),
            percentile(&elongations, 0.9),
            elongations.last().copied().unwrap_or(f64::NAN),
            elongations.len(),
        );
    }
    let Some(global) = global_deg else {
        println!(
            "     the region itself has no PCA axis, so there is no global direction to diverge \
             from."
        );
        return;
    };
    let mut divergences: Vec<f64> = plans
        .iter()
        .map(|plan| axis_difference_deg(plan.angle_deg, global))
        .collect();
    let diverging = divergences
        .iter()
        .filter(|value| **value > D1_ANGLE_DIVERGENCE_DEG)
        .count();
    divergences.sort_by(f64::total_cmp);
    println!(
        "     divergence from the region's own {global:.1}°: p50 {:.1}°, p90 {:.1}°, max {:.1}°;\n\
         \x20      {diverging} of {} cells differ by more than {D1_ANGLE_DIVERGENCE_DEG:.0}° \
         ({:.0}%). Few divergences would EXPLAIN a null D1 result structurally.",
        percentile(&divergences, 0.5),
        percentile(&divergences, 0.9),
        divergences.last().copied().unwrap_or(f64::NAN),
        plans.len(),
        100.0 * diverging as f64 / plans.len() as f64,
    );
}

/// Points of a SHARED grid that engage the surface inside `polygon` — the
/// denominator the per-cell coverage proxy is read against.
fn shared_lattice_points(
    grid: &rs_cam_core::surface::dropcutter::DropCutterGrid,
    polygon: &Polygon2,
    min_z: f64,
) -> usize {
    grid.points
        .iter()
        .filter(|point| {
            point.z > min_z + 0.001 && polygon.contains_point(&P2::new(point.x, point.y))
        })
        .count()
}

/// `points × stepover²`, as a percentage of a polygon's own area — the
/// cross-lattice stand-in for Stage J's membership guard. Not a quality
/// acceptance: it detects seam gaps and double coverage, nothing else.
fn coverage_pct(points: usize, stepover: f64, area: f64) -> f64 {
    if area <= 0.0 {
        return f64::NAN;
    }
    100.0 * points as f64 * stepover * stepover / area
}

/// §0i's bar, recomputed in this binary: the region's ONE global PCA-minor
/// direction, re-decomposed in that rotated lattice (rotating 0° cells is the
/// invalid operation §0h refuses). Returns the CEILING-arm cell time — region
/// 1's 755.3 s — or `None` when the rotated arm refuses its own guard.
fn print_global_pca_rows(
    input: &A3Inputs<'_>,
    region: &RegionCells,
    arms: &[LinkRegime<'_>],
    kinematics: &rs_cam_core::machine::kinematics::MachineKinematics,
    recorded: Option<[(usize, f64); 2]>,
) -> Option<f64> {
    use rs_cam_core::geometry::region_set::RegionSet;

    let effective_min_z = input.mesh.bbox.min.z - 0.1;
    let (global_deg, elongation) = pca_minor_and_elongation(&region.boundary, input.stepover)?;
    let rotated_grid = grid_for_direction(
        input.mesh,
        input.index,
        input.fine,
        input.stepover,
        global_deg,
    );
    let (rotated_cells, rotated_topology) =
        lattice_boustrophedon_cells(&rotated_grid, &region.boundary, effective_min_z);
    if rotated_cells.is_empty()
        || !cell_membership_matches(
            &rotated_grid,
            &region.boundary,
            &rotated_cells,
            effective_min_z,
        )
    {
        println!("     REFUSE global-PCA rows: cell polygons do not preserve the PCA lattice.");
        return None;
    }
    let boundary = RegionSet::new(vec![region.boundary.clone()]);
    let undivided = cost_under_arms(
        input,
        &rotated_grid,
        std::slice::from_ref(&region.boundary),
        &boundary,
        kinematics,
        arms,
    );
    let celled = cost_under_arms(
        input,
        &rotated_grid,
        &rotated_cells,
        &boundary,
        kinematics,
        arms,
    );
    println!(
        "     (global PCA-minor {global_deg:.1}°, elongation {elongation:.2}, \
         {rotated_topology} rotated topology cells)"
    );
    print_a3_row(
        "PCA undivided",
        &arms[0],
        None,
        &undivided[0],
        recorded.map(|pair| pair[0]),
    );
    print_a3_row("PCA undivided", &arms[1], None, &undivided[1], None);
    print_a3_row(
        "PCA cells",
        &arms[0],
        Some(rotated_cells.len()),
        &celled[0],
        recorded.map(|pair| pair[1]),
    );
    print_a3_row(
        "PCA cells",
        &arms[1],
        Some(rotated_cells.len()),
        &celled[1],
        None,
    );
    Some(celled[1].time_s)
}

/// Ceiling-arm seconds for one region's D1 candidate set — what the top-N
/// summary folds. The global-PCA bar is deliberately NOT here: §0f's gate
/// refuses a region-level axis on regions 2 and 3, so there is no top-3 total
/// to fold it into and it stays a per-region row.
#[derive(Clone, Copy)]
struct StageMTotals {
    undivided: f64,
    shared_cells: f64,
    per_cell_zero: f64,
    per_cell_pca: f64,
    per_cell_pca_nn: f64,
}

fn stage_m_region(
    input: &A3Inputs<'_>,
    region: &RegionCells,
    region_index: usize,
    arms: &[LinkRegime<'_>; 2],
    kinematics: &rs_cam_core::machine::kinematics::MachineKinematics,
) -> Option<StageMTotals> {
    use rs_cam_core::geometry::region_set::RegionSet;

    let effective_min_z = input.mesh.bbox.min.z - 0.1;
    if region.cells.is_empty() {
        println!(
            "     REFUSE region {}: no extracted cell polygons",
            region_index + 1
        );
        return None;
    }
    // The SHARED-lattice rows still get the real guard — only the per-cell
    // rows fall back to the coverage proxy, because only they have no common
    // lattice to be guarded against.
    if !cell_membership_matches(
        input.zero_grid,
        &region.boundary,
        &region.cells,
        effective_min_z,
    ) {
        return None;
    }
    println!(
        "\n   ── region {} ({:.0} mm², {} cells) ──",
        region_index + 1,
        region.boundary.area(),
        region.topology_cells
    );

    let region_axis = pca_minor_and_elongation(&region.boundary, input.stepover);
    let (zero_plans, zero_fallbacks) = plan_cells(
        input,
        &region.cells,
        CellDirectionRule::Monotone,
        effective_min_z,
    );
    let (pca_plans, pca_fallbacks) = plan_cells(
        input,
        &region.cells,
        CellDirectionRule::PcaMinor,
        effective_min_z,
    );
    report_cell_angle_distribution(
        &pca_plans,
        region_axis.map(|(minor, _)| minor),
        pca_fallbacks,
    );

    let emission: Vec<usize> = (0..pca_plans.len()).collect();
    let nearest = nearest_neighbour_cell_order(&pca_plans);
    let boundary = RegionSet::new(vec![region.boundary.clone()]);

    let undivided = cost_under_arms(
        input,
        input.zero_grid,
        std::slice::from_ref(&region.boundary),
        &boundary,
        kinematics,
        arms,
    );
    let shared_cells = cost_under_arms(
        input,
        input.zero_grid,
        &region.cells,
        &boundary,
        kinematics,
        arms,
    );
    print_a3_row(
        "0° undivided",
        &arms[0],
        None,
        &undivided[0],
        RECORDED_FRESH_UNDIVIDED.get(region_index).copied(),
    );
    print_a3_row("0° undivided", &arms[1], None, &undivided[1], None);
    print_a3_row(
        "0° cells",
        &arms[0],
        Some(region.cells.len()),
        &shared_cells[0],
        RECORDED_FRESH_CELLS.get(region_index).copied(),
    );
    print_a3_row(
        "0° cells",
        &arms[1],
        Some(region.cells.len()),
        &shared_cells[1],
        None,
    );

    // §0f's gate: only a region elongated enough for a global axis to mean
    // anything gets the §0i bar rows. Region 1 is the only one that passed it.
    let gate_cleared = region_axis.is_some_and(|(_, elongation)| elongation > PCA_ELONGATION_GATE);
    let global_pca_cells = if gate_cleared {
        print_global_pca_rows(
            input,
            region,
            arms,
            kinematics,
            (region_index == 0).then_some(RECORDED_FRESH_PCA),
        )
    } else {
        println!(
            "     (no global-PCA bar: region elongation is at or below §0f's gate \
             {PCA_ELONGATION_GATE:.1}, so §0f refuses a region-level axis here)"
        );
        None
    };

    let per_cell_zero =
        cost_plans_under_arms(input, &zero_plans, &emission, &boundary, kinematics, arms);
    let per_cell_pca =
        cost_plans_under_arms(input, &pca_plans, &emission, &boundary, kinematics, arms);
    let per_cell_pca_nn =
        cost_plans_under_arms(input, &pca_plans, &nearest, &boundary, kinematics, arms);
    print_a3_row(
        "per-cell 0°",
        &arms[0],
        Some(zero_plans.len()),
        &per_cell_zero[0],
        None,
    );
    print_a3_row(
        "per-cell 0°",
        &arms[1],
        Some(zero_plans.len()),
        &per_cell_zero[1],
        None,
    );
    print_a3_row(
        "per-cell PCA",
        &arms[0],
        Some(pca_plans.len()),
        &per_cell_pca[0],
        None,
    );
    print_a3_row(
        "per-cell PCA",
        &arms[1],
        Some(pca_plans.len()),
        &per_cell_pca[1],
        None,
    );
    print_a3_row(
        "per-cell PCA NN",
        &arms[0],
        Some(pca_plans.len()),
        &per_cell_pca_nn[0],
        None,
    );
    print_a3_row(
        "per-cell PCA NN",
        &arms[1],
        Some(pca_plans.len()),
        &per_cell_pca_nn[1],
        None,
    );

    let area = region.boundary.area();
    let shared_points = shared_lattice_points(input.zero_grid, &region.boundary, effective_min_z);
    let zero_points: usize = zero_plans.iter().map(|plan| plan.lattice_points).sum();
    let pca_points: usize = pca_plans.iter().map(|plan| plan.lattice_points).sum();
    println!(
        "     COVERAGE PROXY (points × stepover² ÷ region area): shared 0° {shared_points} pts \
         = {:.1}%,\n\
         \x20      per-cell 0° {zero_points} pts = {:.1}%, per-cell PCA {pca_points} pts = \
         {:.1}%.\n\
         \x20      This REPLACES Stage J's membership guard on the per-cell rows — there is no\n\
         \x20      common lattice to guard against. A per-cell figure well below the shared one\n\
         \x20      means seam gaps; well above means double coverage at seams. Either way C4\n\
         \x20      binds before any of this is built.",
        coverage_pct(shared_points, input.stepover, area),
        coverage_pct(zero_points, input.stepover, area),
        coverage_pct(pca_points, input.stepover, area),
    );
    println!(
        "     DELTA DECOMPOSITION (ceiling arm): phase/origin {:.3}x (shared 0° cells → \
         per-cell 0°),\n\
         \x20      direction {:.3}x (per-cell 0° → per-cell PCA), total {:.3}x, cell order \
         {:.3}x (emission → NN).\n\
         \x20      Against the §0i bar (global PCA cells): {:.3}x.",
        delta(&shared_cells[1], &per_cell_zero[1]),
        delta(&per_cell_zero[1], &per_cell_pca[1]),
        delta(&shared_cells[1], &per_cell_pca[1]),
        delta(&per_cell_pca[1], &per_cell_pca_nn[1]),
        global_pca_cells.map_or(f64::NAN, |bar| bar / per_cell_pca[1].time_s),
    );
    println!(
        "     (phase control: all {} cells held at 0° — Stage I's monotone direction IS 0° by\n\
         \x20      construction, so its own no-axis fallback count is {zero_fallbacks} and the\n\
         \x20      two per-cell arms differ ONLY in the angle each cell's lattice is built at.)",
        zero_plans.len()
    );

    Some(StageMTotals {
        undivided: undivided[1].time_s,
        shared_cells: shared_cells[1].time_s,
        per_cell_zero: per_cell_zero[1].time_s,
        per_cell_pca: per_cell_pca[1].time_s,
        per_cell_pca_nn: per_cell_pca_nn[1].time_s,
    })
}

fn stage_m(input: &A3Inputs<'_>, regions: &[RegionCells]) {
    use rs_cam_core::machine::kinematics::MachineKinematics;
    use rs_cam_core::surface_link::LinkCeiling;

    println!(
        "========== STAGE M — D1: PER-CELL sweep direction vs one global direction =========="
    );
    if regions.is_empty() {
        println!("\n     SKIP: no Shallow regions to price.\n");
        return;
    }
    let mesh = input.mesh;
    let safe_z = mesh.bbox.max.z + 5.0;
    let block_top_z = mesh.bbox.max.z;
    let kinematics = MachineKinematics {
        acceleration_mm_s2: MACHINE_ACCEL_SCALAR,
        acceleration_xyz_mm_s2: Some(MACHINE_ACCEL_XYZ),
        junction_deviation_mm: JUNCTION_DEVIATION_MM,
        ..MachineKinematics::default()
    };
    println!(
        "\n   Same ceiling as Stage L / §0i (MACHINED STOCK: the Ø{ROUGH_DIAMETER_MM:.0} rough at \
         axial leave\n\
         \x20  {ROUGH_STOCK_TO_LEAVE_AXIAL_MM:.1} mm, then the tier-0 R1.5 where tier 0 machines), \
         same production relink\n\
         \x20  parameters, same F-034 integrator, BOTH regimes on every row (E1 discipline).\n\
         \x20  The operator-honest bar is the CEILING arm's global `PCA cells` row — 755.3 s on\n\
         \x20  region 1 in the 2026-08-29 A3 run. The fresh arm is kept only because §0g/§0h are\n\
         \x20  recorded there and its `= FINDINGS` marks are what prove the setup did not drift.\n"
    );

    let bounds = ceiling_stock_bounds(mesh, regions);
    let stock = a3_machined_stock(input, bounds);
    report_ceiling_population(&stock, input, &regions[0].boundary, safe_z);

    let machined_ceiling = LinkCeiling {
        stock: Some(&stock),
        tool_radius: input.fine.envelope_radius_mm(),
        fallback_top_z: block_top_z,
    };
    let arms = [
        LinkRegime::fresh_stock(safe_z),
        LinkRegime::rest_op("ceiling", safe_z, machined_ceiling),
    ];
    print_a3_header();

    let mut totals: Vec<StageMTotals> = Vec::new();
    for (region_index, region) in regions.iter().enumerate() {
        if let Some(row) = stage_m_region(input, region, region_index, &arms, &kinematics) {
            totals.push(row);
        }
    }
    if totals.is_empty() {
        println!("\n     REFUSE total: no region produced a comparable candidate set.\n");
        return;
    }
    let mut undivided = 0.0_f64;
    let mut shared_cells = 0.0_f64;
    let mut per_cell_zero = 0.0_f64;
    let mut per_cell_pca = 0.0_f64;
    let mut per_cell_pca_nn = 0.0_f64;
    for row in &totals {
        undivided += row.undivided;
        shared_cells += row.shared_cells;
        per_cell_zero += row.per_cell_zero;
        per_cell_pca += row.per_cell_pca;
        per_cell_pca_nn += row.per_cell_pca_nn;
    }
    println!(
        "\n   TOP-{} TOTAL, CEILING ARM (the operator-honest regime):\n\
         \x20    0° undivided     {undivided:.1} s\n\
         \x20    0° cells         {shared_cells:.1} s\n\
         \x20    per-cell 0°      {per_cell_zero:.1} s   (phase/origin control)\n\
         \x20    per-cell PCA     {per_cell_pca:.1} s   (D1 candidate)\n\
         \x20    per-cell PCA NN  {per_cell_pca_nn:.1} s   (greedy cell order, a BOUND on C3)\n\
         \x20    phase {:.3}x, direction {:.3}x, total vs 0° cells {:.3}x, cell order {:.3}x\n",
        totals.len(),
        shared_cells / per_cell_zero,
        per_cell_zero / per_cell_pca,
        shared_cells / per_cell_pca,
        per_cell_pca / per_cell_pca_nn,
    );
    println!(
        "   D1's verdict is the `direction` factor, read against §0i's global direction lever\n\
         (1.034× under this ceiling). It is priced on a MEASUREMENT rig: no production cell\n\
         geometry, no cell adjacency graph, no C3 cell TSP, no GUI overlay, and the per-cell\n\
         rows carry the coverage proxy in place of Stage J's membership guard. A win here is\n\
         a reason to build C2/C3 further, never a reason to ship a per-cell strategy.\n"
    );
}

/// Everything the D1 runner needs, owned. Deliberately a third copy of the A3
/// setup rather than a refactor of Stage L's runner: Stage M must be additive
/// and must not be able to move an existing stage's numbers.
struct D1Setup {
    mesh: TriangleMesh,
    index: SpatialIndex,
    coarse: TaperedBallEndmill,
    fine: TaperedBallEndmill,
    tier_map: rs_cam_core::maps::tier_map::TierMap,
    zero_grid: rs_cam_core::surface::dropcutter::DropCutterGrid,
    stepover: f64,
    regions: Vec<RegionCells>,
}

fn d1_setup() -> Option<D1Setup> {
    let path = Path::new(WANAKA_MESH);
    if !path.exists() {
        println!("SKIP: {WANAKA_MESH} not present on this machine.");
        return None;
    }

    let mesh = TriangleMesh::from_stl_scaled(path, 1.0).expect("load wanaka terrain");
    let index = SpatialIndex::build_auto(&mesh);
    let r15 = TaperedBallEndmill::new(3.0, 2.8, 6.0, 30.5);
    let r10 = TaperedBallEndmill::new(2.0, 5.7, 6.0, 20.0);
    let never_cancel = || false;
    // The ladder borrows both tools, and this setup MOVES both of them into
    // `D1Setup` at the end. Scoping that borrow explicitly is cheaper than
    // relying on NLL to prove the ladder is dead by then.
    let (map, cusp_radii) = {
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
        let cusp_radii: Vec<f64> = tools.iter().map(|tool| tool.cusp_radius_mm()).collect();
        (map, cusp_radii)
    };
    let islands = extract_tier_islands(
        &map,
        &TierIslandParams {
            coarseness: COARSENESS,
            overlap_mm: OVERLAP_MM,
            max_regions_per_tier: MAX_REGIONS_PER_TIER,
            ..TierIslandParams::default()
        },
        &cusp_radii,
    )
    .expect("islands");
    let Some(fine) = islands.per_tier.iter().find(|set| set.tier == 1) else {
        println!("SKIP: no tier 1 machining region.");
        return None;
    };
    if fine.machining.is_empty() {
        println!("SKIP: tier 1 machining region is empty.");
        return None;
    }

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
        .map(|(i, &covered)| {
            if !covered {
                return false;
            }
            let row = i / heightmap.cols;
            let col = i % heightmap.cols;
            fine.machining.contains(&P2::new(
                heightmap.origin_x + col as f64 * heightmap.cell_size,
                heightmap.origin_y + row as f64 * heightmap.cell_size,
            ))
        })
        .collect();
    let mut planner = FinishPlannerParams::for_tool(cusp_radii[1]);
    planner.overlap_mm = OVERLAP_MM;
    let planned = decompose(&surface.slope_map, &covered, &[], &planner);
    let stepover = equal_cusp_stepover_mm(cusp_radii[1], CUSP_HEIGHT_MM);
    println!(
        "D1 setup: {} planned regions, Shallow raster stepover {stepover:.4} mm\n",
        planned.regions.len()
    );

    let grid = grid_for_stage_i(&mesh, &index, &r10, stepover);
    let effective_min_z = mesh.bbox.min.z - 0.1;
    let mut shallow: Vec<&Polygon2> = planned
        .regions
        .iter()
        .filter(|region| region.band == FinishBand::Shallow)
        .map(|region| &region.polygon)
        .collect();
    shallow.sort_by(|a, b| b.area().total_cmp(&a.area()));
    let regions: Vec<RegionCells> = shallow
        .into_iter()
        .take(3)
        .map(|boundary| {
            let (cells, topology_cells) =
                lattice_boustrophedon_cells(&grid, boundary, effective_min_z);
            RegionCells {
                boundary: boundary.clone(),
                cells,
                topology_cells,
            }
        })
        .collect();

    Some(D1Setup {
        mesh,
        index,
        coarse: r15,
        fine: r10,
        tier_map: map,
        zero_grid: grid,
        stepover,
        regions,
    })
}

/// D1 — per-cell sweep direction (`PROGRAMME.md` Track D, D1).
///
/// ```text
/// cargo test -p rs_cam_core --test thin_organic_island_widths \
///   wanaka_per_cell_direction_d1 -- --ignored --nocapture
/// ```
#[test]
#[ignore = "evidence run — needs the operator's wanaka mesh (not in repo)"]
fn wanaka_per_cell_direction_d1() {
    let Some(setup) = d1_setup() else {
        return;
    };
    let input = A3Inputs {
        mesh: &setup.mesh,
        index: &setup.index,
        fine: &setup.fine,
        coarse: &setup.coarse,
        tier_map: &setup.tier_map,
        zero_grid: &setup.zero_grid,
        stepover: setup.stepover,
    };
    stage_m(&input, &setup.regions);
}

// ── STAGE N ─────────────────────────────────────────────────────────────
//
// D2 — per-cell PATTERN: does an offset-ring cascade beat the raster on a
// MONOTONE CELL?
//
// §0d refuted the contour cascade on UNDIVIDED regions at 0.91×, and named the
// mechanism: long, wildly-varying perimeters produce 3.2× the moves at 29%
// more cutting distance, and the junction-deviation model crawls through the
// resulting short chords. That mechanism is a statement about the SHAPE the
// cascade was seeded from, so it does not carry over to a cell whose perimeter
// is short and convex-ish — which is exactly why D2 exists as a separate
// question rather than as a corollary of §0d.
//
// D1 (§0j) supplies the opposing prior, and it is the sharper one: a per-cell
// candidate has NO SHARED LATTICE, and §0j measured that misaligned per-cell
// lattices break the cross-cell serpentine chords the relinker stitches — a
// per-cell direction assignment cost 0.917×/0.921× under the ceiling, at +9%
// cutting distance. A contour cell has no shared lattice either. Whether the
// cell-hugging ring geometry buys back more than the lost continuity is the
// measurement; neither prior settles it.
//
// The stage prices three WHOLE-REGION candidates through the same E1 kernel in
// both link regimes — all-raster, all-contour, and a HYBRID that picks per
// cell by a cheap standalone proxy. The hybrid is D3's cheapest bound: if the
// best achievable mix of two shipped patterns does not beat the shared-lattice
// raster, a "bent parallel" generator has to earn its whole margin from
// geometry no existing generator emits.
//
// It is additive: no existing stage changes, and it REUSES `d1_setup` rather
// than making a fourth copy of the A3 setup. That is read-only sharing — the
// discipline Stage M's own note protects (an additive stage must not be able
// to move an existing stage's numbers) is satisfied because nothing in the
// setup, in Stage L or in Stage M is edited.

/// FINDINGS §0i's CEILING arm for the 0° undivided baseline (regions 1–3),
/// from the 2026-08-29 A3 run: `(kept retracts, F-034 seconds)`.
///
/// §0i recorded only the FRESH arm as constants ([`RECORDED_FRESH_UNDIVIDED`]
/// etc.) because at the time the ceiling arm was the thing being measured.
/// It has since been measured, so Stage N pins BOTH arms — the ceiling arm is
/// the operator-honest regime and the one D2's bars live in, and a bar that is
/// not reproduction-checked is a number read from a document.
const RECORDED_CEILING_UNDIVIDED: [(usize, f64); 3] = [(4, 917.5), (3, 484.2), (3, 419.2)];
/// The same for §0i's 0° monotone-cell arm. **Region 1's 772.6 s is D2's first
/// bar** — the `0° cells ceiling` row, which Stage N's own all-raster candidate
/// reproduces by construction (it IS that candidate: the same cells, the same
/// shared 0° lattice, the same emission order).
const RECORDED_CEILING_CELLS: [(usize, f64); 3] = [(4, 772.6), (3, 419.0), (3, 384.4)];

/// D2's second bar: §0i's winner — region 1's ceiling-arm global `PCA cells`
/// row, 755.3 s (`FINDINGS.md` §0i Table 2, 2026-08-29 run). Printed for
/// orientation only; Stage N **recomputes** it in this binary through
/// [`print_global_pca_rows`], exactly as Stage M does, and compares against the
/// recomputed value rather than this constant.
const D2_WINNER_BAR_S: f64 = 755.3;

/// Which pattern a cell is machined with.
#[derive(Clone, Copy, PartialEq, Eq)]
enum CellPattern {
    /// The cell's slice of the region's SHARED 0° raster lattice — the pieces
    /// §0i's `0° cells` row is built from, reused verbatim.
    Raster,
    /// An offset-ring cascade seeded from the cell's own polygon.
    Contour,
}

impl CellPattern {
    fn label(self) -> &'static str {
        match self {
            Self::Raster => "raster",
            Self::Contour => "contour",
        }
    }
}

/// One cell, both of its candidate toolpaths, and the standalone proxy cost of
/// each.
///
/// Both toolpaths are generated ONCE and cloned per link regime. That is
/// equivalent to Stage L/M's rebuild-per-arm convention here because every
/// generator input is arm-independent (`safe_z` is identical in both arms —
/// `mesh.bbox.max.z + 5.0` — and neither generator sees the link ceiling), and
/// it halves the number of ring cascades the stage runs.
struct CellCandidates {
    /// The cell's own area, the weight every distribution below is read by.
    /// The polygon itself is deliberately NOT kept: both candidates are
    /// already generated against it here, so nothing downstream needs it and
    /// carrying it would be a dead field.
    area_mm2: f64,
    raster: rs_cam_core::toolpath::Toolpath,
    contour: rs_cam_core::toolpath::Toolpath,
    /// F-034 seconds for this cell's own moves, WITHOUT links — the selection
    /// proxy. See [`cell_pattern_proxy_s`].
    raster_proxy_s: f64,
    contour_proxy_s: f64,
    /// Cutting distance of each pattern's own moves, for the density-confounded
    /// swept-band coverage figure.
    raster_cut_mm: f64,
    contour_cut_mm: f64,
    /// `ScallopReport::untouched_mm2` for this cell's cascade — the hole-aware
    /// area no ring reached. The PRIMARY gap detector on the contour rows.
    contour_untouched_mm2: f64,
    /// `ScallopReport::uncut_core_mm2`, its hole-blind sibling, kept beside it
    /// because the gap between the two is exactly the hole-blindness.
    contour_uncut_core_mm2: f64,
    /// Rings that reached the toolpath (`ScallopReport::ring_count`).
    contour_rings: usize,
    /// `true` when the cascade emitted no moves at all for this cell. Such a
    /// cell is FORCED to raster in the hybrid and counted separately — a
    /// silently-dropped cell would make the all-contour row cheap for the
    /// wrong reason and the coverage figures would take the blame.
    contour_empty: bool,
}

impl CellCandidates {
    /// The proxy's pick for this cell.
    fn proxy_pick(&self) -> CellPattern {
        if self.contour_empty || self.contour_proxy_s >= self.raster_proxy_s {
            CellPattern::Raster
        } else {
            CellPattern::Contour
        }
    }
}

/// The SELECTION PROXY, stated exactly: the F-034 integrated time of one
/// cell's own moves, generated and costed **standalone** — no relink, no
/// links to neighbouring cells, no ceiling.
///
/// Why a proxy at all, rather than a per-cell ground truth: **relink is a
/// whole-region operation**. A cell's links, its kept retracts and its two
/// vertical ceiling legs all depend on which cells sit beside it and in what
/// order the relinker visits them, so "this cell's relinked cost" is not a
/// well-defined quantity to select on. Every per-cell selector a production
/// router could afford is therefore a proxy of this shape, and pricing THIS
/// one is the point: the hybrid row is what a cheap, local, greedy pattern
/// picker actually buys.
///
/// The proxy is deliberately NOT free of bias, and the bias is stated: it
/// charges each pattern its own intra-cell motion (including, for the cascade,
/// its helical ring-to-ring connectors) and charges NEITHER for the links that
/// join cells. A cascade that is compact inside the cell but leaves the tool
/// far from the next cell's entry is flattered by it. That is why the stage
/// prints how often the proxy's pick disagrees with the region-level outcome.
fn cell_pattern_proxy_s(
    toolpath: &rs_cam_core::toolpath::Toolpath,
    kinematics: &rs_cam_core::machine::kinematics::MachineKinematics,
) -> f64 {
    rs_cam_core::machine::kinematics::compute_cycle_time(
        toolpath,
        kinematics,
        MAX_FEED_MM_MIN,
        RAPID_FEED_MM_MIN,
    )
}

/// One cell's OFFSET-RING CASCADE.
///
/// **Provenance — this is Stage D's generator, unchanged.** §0d costed the
/// contour candidate through `scallop_toolpath_structured_annotated_with_cancel`
/// with the region polygon passed as `boundary_regions`, and every
/// `ScallopParams` field below is Stage D's value. No new ring generator was
/// written and no adapter was needed, because that call **already scopes to an
/// arbitrary polygon**: under P2.3 (`scallop.rs`, `region_boundaries`) a
/// non-empty `boundary_regions` REPLACES the hardcoded mesh-footprint rectangle
/// as the cascade's seed, so each polygon gets its own independent ring set
/// offset inward from itself. Handing it one cell polygon is therefore the
/// cell-hugging cascade D2 asks for, generated by the shipped code path.
///
/// **Ring Z placement, stated because the task asks for it:** the rings ride
/// the generator's own drop-cutter surface heightmap
/// (`finish_surface_cache::cached_finish_surface`, at
/// `scallop_generation_resolution(cutter, tolerance)`), i.e. **surface Z** —
/// the same convention Stage D used, and NOT the Stage-I raster lattice. The
/// two grids differ in resolution by construction; the cusp DIAL is what is
/// held equal between the arms, not the sampling.
///
/// **Ring spacing:** the cascade selects its own per-ring advance under the
/// cusp law. On flat ground that is `stepover_from_scallop_flat(cusp_r, h)`,
/// which is arithmetically the SAME expression as this file's
/// [`equal_cusp_stepover_mm`] — so on flat ground the two arms share a
/// stepover exactly; on slope the cascade tightens (min-across-ring), which
/// costs it time the F-034 column charges for and buys cusp quality this stage
/// does not measure.
fn cell_contour_candidate(
    input: &A3Inputs<'_>,
    polygon: &Polygon2,
    safe_z: f64,
) -> Option<(
    rs_cam_core::toolpath::Toolpath,
    rs_cam_core::scallop::ScallopReport,
)> {
    use rs_cam_core::geometry::region_set::RegionSet;
    use rs_cam_core::scallop::{
        ScallopDirection, ScallopParams, scallop_toolpath_structured_annotated_with_cancel,
    };

    let never_cancel = || false;
    let region = RegionSet::new(vec![polygon.clone()]);
    let params = ScallopParams {
        scallop_height: CUSP_HEIGHT_MM,
        tolerance: OP_TOLERANCE_MM,
        direction: ScallopDirection::default(),
        continuous: true,
        slope_from: 0.0,
        slope_to: 90.0,
        feed_rate: FEED_MM_MIN,
        plunge_rate: PLUNGE_MM_MIN,
        safe_z,
        stock_to_leave: 0.0,
        intra_pass_hookup_mm: 0.0,
        link_kinematics: None,
    };
    scallop_toolpath_structured_annotated_with_cancel(
        input.mesh,
        input.index,
        input.fine,
        &params,
        None,
        Some(&region),
        &never_cancel,
    )
    .ok()
    .map(|(toolpath, _, report)| (toolpath, report))
}

/// Build both candidates for every cell of one region.
fn build_cell_candidates(
    input: &A3Inputs<'_>,
    cells: &[Polygon2],
    kinematics: &rs_cam_core::machine::kinematics::MachineKinematics,
    safe_z: f64,
) -> Vec<CellCandidates> {
    use rs_cam_core::geometry::region_set::RegionSet;
    use rs_cam_core::toolpath::{Toolpath, raster_toolpath_from_grid};

    let effective_min_z = input.mesh.bbox.min.z - 0.1;
    cells
        .iter()
        .map(|polygon| {
            let region = RegionSet::new(vec![polygon.clone()]);
            // (i) the cell's slice of the SHARED 0° lattice — byte-for-byte the
            // piece `raster_candidate` contributes for this cell, so the
            // all-raster concatenation below IS §0i's `0° cells` candidate.
            let raster = raster_toolpath_from_grid(
                input.zero_grid,
                FEED_MM_MIN,
                PLUNGE_MM_MIN,
                safe_z,
                Some(effective_min_z),
                Some(&region),
            );
            let (contour, report) = cell_contour_candidate(input, polygon, safe_z)
                .map_or_else(|| (Toolpath::new(), None), |(tp, rep)| (tp, Some(rep)));
            let contour_empty = contour.moves.is_empty();
            CellCandidates {
                area_mm2: polygon.area(),
                raster_proxy_s: cell_pattern_proxy_s(&raster, kinematics),
                contour_proxy_s: if contour_empty {
                    f64::INFINITY
                } else {
                    cell_pattern_proxy_s(&contour, kinematics)
                },
                raster_cut_mm: raster.total_cutting_distance(),
                contour_cut_mm: contour.total_cutting_distance(),
                contour_untouched_mm2: report.as_ref().map_or(f64::NAN, |r| r.untouched_mm2),
                contour_uncut_core_mm2: report.as_ref().map_or(f64::NAN, |r| r.uncut_core_mm2),
                contour_rings: report.as_ref().map_or(0, |r| r.ring_count),
                contour_empty,
                raster,
                contour,
            }
        })
        .collect()
}

/// Concatenate one pattern choice per cell, in emission order, into one
/// whole-region candidate.
///
/// Cell VISIT ORDER is the decomposition's own emission order for all three
/// candidates. §0j measured a greedy nearest-neighbour order as producing
/// byte-identical output under the production relinker's `reorder: true`, so
/// re-running that variant here would price the same null twice.
fn mixed_pattern_candidate(
    cells: &[CellCandidates],
    picks: &[CellPattern],
) -> rs_cam_core::toolpath::Toolpath {
    let mut out = rs_cam_core::toolpath::Toolpath::new();
    for (cell, pick) in cells.iter().zip(picks.iter()) {
        let source = match pick {
            CellPattern::Raster => &cell.raster,
            CellPattern::Contour => &cell.contour,
        };
        out.moves.extend(source.moves.iter().cloned());
    }
    out
}

/// [`cost_under_arms`] for a mixed-pattern candidate: same E1 kernel, same
/// production relink parameters, the candidate rebuilt per arm so no arm sees
/// another's toolpath.
fn cost_picks_under_arms(
    input: &A3Inputs<'_>,
    cells: &[CellCandidates],
    picks: &[CellPattern],
    boundary: &rs_cam_core::geometry::region_set::RegionSet<'_>,
    kinematics: &rs_cam_core::machine::kinematics::MachineKinematics,
    arms: &[LinkRegime<'_>],
) -> Vec<CandidateCost> {
    arms.iter()
        .map(|arm| {
            relink_and_cost_under(
                mixed_pattern_candidate(cells, picks),
                input.mesh,
                input.index,
                input.fine,
                boundary,
                kinematics,
                *arm,
            )
        })
        .collect()
}

/// SWEPT-BAND coverage: `cutting length × stepover` as a share of area.
///
/// The same quantity [`coverage_pct`] reports, expressed through path length
/// instead of lattice points — for a raster the two coincide, because
/// consecutive in-row lattice points sit exactly one stepover apart, so
/// `points × stepover² == length × stepover`. Stating it that way is what lets
/// one instrument read both patterns.
///
/// **DENSITY-CONFOUNDED on the contour rows, and must not be read as a gap
/// detector there.** The cascade selects its own per-ring advance and tightens
/// it on slope, so honest extra cutting — which the F-034 column already
/// charges for — reads here as "double coverage". It also counts the helical
/// ring-to-ring connectors as cut length. The gap detector on the contour rows
/// is `ScallopReport::untouched_mm2`, printed beside it.
fn swept_coverage_pct(cutting_mm: f64, stepover: f64, area: f64) -> f64 {
    if area <= 0.0 {
        return f64::NAN;
    }
    100.0 * cutting_mm * stepover / area
}

/// The per-cell pattern-pick distribution, by count and by AREA SHARE — the
/// structural half of D2's answer, printed before any whole-region cost.
///
/// By count alone, 179 mostly-tiny cells would let a preference on the small
/// ones drown a preference on the ones that hold the time; the area share is
/// the reading that matters.
fn report_pattern_pick_distribution(cells: &[CellCandidates]) -> Vec<CellPattern> {
    let picks: Vec<CellPattern> = cells.iter().map(CellCandidates::proxy_pick).collect();
    if cells.is_empty() {
        println!("     pattern picks: no cells.");
        return picks;
    }
    let total_area: f64 = cells.iter().map(|cell| cell.area_mm2).sum();
    let contour_cells = picks
        .iter()
        .filter(|pick| **pick == CellPattern::Contour)
        .count();
    let contour_area: f64 = cells
        .iter()
        .zip(picks.iter())
        .filter(|(_, pick)| **pick == CellPattern::Contour)
        .map(|(cell, _)| cell.area_mm2)
        .sum();
    let empty = cells.iter().filter(|cell| cell.contour_empty).count();
    let empty_area: f64 = cells
        .iter()
        .filter(|cell| cell.contour_empty)
        .map(|cell| cell.area_mm2)
        .sum();
    let mut margins: Vec<f64> = cells
        .iter()
        .filter(|cell| !cell.contour_empty && cell.contour_proxy_s > 0.0)
        .map(|cell| cell.raster_proxy_s / cell.contour_proxy_s)
        .collect();
    margins.sort_by(f64::total_cmp);
    println!(
        "     PATTERN PICKS (standalone per-cell F-034 proxy, no links): contour on {contour_cells} \
         of {} cells\n\
         \x20      = {:.1}% by count, {:.1}% by AREA ({contour_area:.0} of {total_area:.0} mm²).\n\
         \x20      cells whose cascade emitted NOTHING (forced to raster, counted here so a\n\
         \x20      dropped cell cannot be mistaken for a coverage defect): {empty} ({empty_area:.0} mm²).\n\
         \x20      proxy margin raster÷contour over the {} non-empty cells: p10 {:.2}, p50 {:.2}, p90 {:.2}\n\
         \x20      (>1 favours contour). A margin clustered at 1.00 means the proxy is choosing\n\
         \x20      between near-equal candidates and its picks carry little information.",
        cells.len(),
        100.0 * contour_cells as f64 / cells.len() as f64,
        100.0 * contour_area / total_area.max(1e-9),
        margins.len(),
        percentile(&margins, 0.10),
        percentile(&margins, 0.50),
        percentile(&margins, 0.90),
    );
    picks
}

/// How often the cheap local proxy's per-cell pick disagrees with the
/// REGION-LEVEL outcome — the whole-region winner between all-raster and
/// all-contour under the ceiling.
///
/// This is the only disagreement that can be measured: a per-cell relinked
/// ground truth does not exist (see [`cell_pattern_proxy_s`]). It answers "is
/// the local proxy telling a different story from the global one, and over how
/// much area", which is what decides whether the hybrid row is a real third
/// candidate or a re-spelling of one of the first two.
fn report_proxy_disagreement(
    cells: &[CellCandidates],
    picks: &[CellPattern],
    region_winner: CellPattern,
) {
    if cells.is_empty() {
        return;
    }
    let total_area: f64 = cells.iter().map(|cell| cell.area_mm2).sum();
    let disagreeing = picks.iter().filter(|pick| **pick != region_winner).count();
    let disagreeing_area: f64 = cells
        .iter()
        .zip(picks.iter())
        .filter(|(_, pick)| **pick != region_winner)
        .map(|(cell, _)| cell.area_mm2)
        .sum();
    println!(
        "     PROXY vs REGION-LEVEL OUTCOME: the whole-region ceiling-arm winner is \
         all-{}.\n\
         \x20      The per-cell proxy disagrees with it on {disagreeing} of {} cells = {:.1}% by \
         count, {:.1}% by area.\n\
         \x20      0% means the hybrid is that whole-region candidate under another name; a high\n\
         \x20      share means the proxy sees local structure the aggregate hides — and the hybrid\n\
         \x20      row below is what that local structure is actually worth once relinked.",
        region_winner.label(),
        cells.len(),
        100.0 * disagreeing as f64 / cells.len() as f64,
        100.0 * disagreeing_area / total_area.max(1e-9),
    );
}

/// Ceiling-arm seconds for one region's D2 candidate set — what the top-N
/// summary folds.
#[derive(Clone, Copy)]
struct StageNTotals {
    undivided: f64,
    all_raster: f64,
    all_contour: f64,
    hybrid: f64,
}

fn stage_n_region(
    input: &A3Inputs<'_>,
    region: &RegionCells,
    region_index: usize,
    arms: &[LinkRegime<'_>; 2],
    kinematics: &rs_cam_core::machine::kinematics::MachineKinematics,
) -> Option<StageNTotals> {
    use rs_cam_core::geometry::region_set::RegionSet;

    let effective_min_z = input.mesh.bbox.min.z - 0.1;
    let safe_z = input.mesh.bbox.max.z + 5.0;
    if region.cells.is_empty() {
        println!(
            "     REFUSE region {}: no extracted cell polygons",
            region_index + 1
        );
        return None;
    }
    // The SHARED-lattice rows (0° undivided, all-raster) still get Stage J's
    // real membership guard. The contour and hybrid rows cannot: a ring
    // cascade is not a lattice candidate, so there is no emitted-point
    // population to compare. They carry the coverage proxy plus the cascade's
    // own `untouched_mm2` instead, and C4 binds anything built on them.
    if !cell_membership_matches(
        input.zero_grid,
        &region.boundary,
        &region.cells,
        effective_min_z,
    ) {
        return None;
    }
    println!(
        "\n   ── region {} ({:.0} mm², {} cells) ──",
        region_index + 1,
        region.boundary.area(),
        region.topology_cells
    );

    let cells = build_cell_candidates(input, &region.cells, kinematics, safe_z);
    let picks = report_pattern_pick_distribution(&cells);
    let all_raster: Vec<CellPattern> = vec![CellPattern::Raster; cells.len()];
    let all_contour: Vec<CellPattern> = cells
        .iter()
        .map(|cell| {
            if cell.contour_empty {
                CellPattern::Raster
            } else {
                CellPattern::Contour
            }
        })
        .collect();

    let boundary = RegionSet::new(vec![region.boundary.clone()]);
    let undivided = cost_under_arms(
        input,
        input.zero_grid,
        std::slice::from_ref(&region.boundary),
        &boundary,
        kinematics,
        arms,
    );
    let raster_rows =
        cost_picks_under_arms(input, &cells, &all_raster, &boundary, kinematics, arms);
    let contour_rows =
        cost_picks_under_arms(input, &cells, &all_contour, &boundary, kinematics, arms);
    let hybrid_rows = cost_picks_under_arms(input, &cells, &picks, &boundary, kinematics, arms);

    print_a3_row(
        "0° undivided",
        &arms[0],
        None,
        &undivided[0],
        RECORDED_FRESH_UNDIVIDED.get(region_index).copied(),
    );
    print_a3_row(
        "0° undivided",
        &arms[1],
        None,
        &undivided[1],
        RECORDED_CEILING_UNDIVIDED.get(region_index).copied(),
    );
    print_a3_row(
        "all-raster",
        &arms[0],
        Some(cells.len()),
        &raster_rows[0],
        RECORDED_FRESH_CELLS.get(region_index).copied(),
    );
    print_a3_row(
        "all-raster",
        &arms[1],
        Some(cells.len()),
        &raster_rows[1],
        RECORDED_CEILING_CELLS.get(region_index).copied(),
    );
    print_a3_row(
        "all-contour",
        &arms[0],
        Some(cells.len()),
        &contour_rows[0],
        None,
    );
    print_a3_row(
        "all-contour",
        &arms[1],
        Some(cells.len()),
        &contour_rows[1],
        None,
    );
    print_a3_row("hybrid", &arms[0], Some(cells.len()), &hybrid_rows[0], None);
    print_a3_row("hybrid", &arms[1], Some(cells.len()), &hybrid_rows[1], None);

    // §0f's gate: only a region elongated enough for a global axis to mean
    // anything gets §0i's winner row. Region 1 was the only one that passed.
    let region_axis = pca_minor_and_elongation(&region.boundary, input.stepover);
    let gate_cleared = region_axis.is_some_and(|(_, elongation)| elongation > PCA_ELONGATION_GATE);
    let winner_bar = if gate_cleared {
        print_global_pca_rows(
            input,
            region,
            arms,
            kinematics,
            (region_index == 0).then_some(RECORDED_FRESH_PCA),
        )
    } else {
        println!(
            "     (no global-PCA bar: region elongation is at or below §0f's gate \
             {PCA_ELONGATION_GATE:.1}, so §0f refuses a region-level axis here)"
        );
        None
    };

    let region_winner = if contour_rows[1].time_s < raster_rows[1].time_s {
        CellPattern::Contour
    } else {
        CellPattern::Raster
    };
    report_proxy_disagreement(&cells, &picks, region_winner);

    let area = region.boundary.area();
    let raster_cut: f64 = cells.iter().map(|cell| cell.raster_cut_mm).sum();
    let contour_cut: f64 = cells
        .iter()
        .zip(all_contour.iter())
        .filter(|(_, pick)| **pick == CellPattern::Contour)
        .map(|(cell, _)| cell.contour_cut_mm)
        .sum();
    let untouched: f64 = cells
        .iter()
        .filter(|cell| !cell.contour_empty)
        .map(|cell| cell.contour_untouched_mm2)
        .sum();
    let uncut_core: f64 = cells
        .iter()
        .filter(|cell| !cell.contour_empty)
        .map(|cell| cell.contour_uncut_core_mm2)
        .sum();
    let rings: usize = cells.iter().map(|cell| cell.contour_rings).sum();
    let shared_points = shared_lattice_points(input.zero_grid, &region.boundary, effective_min_z);
    println!(
        "     COVERAGE — contour rows carry this INSTEAD of Stage J's membership guard.\n\
         \x20      PRIMARY (gap detector): cascade untouched {untouched:.1} mm² = {:.2}% of the \
         region, hole-blind\n\
         \x20      uncut_core {uncut_core:.1} mm² = {:.2}%, over {rings} emitted rings. This is the \
         cascade's own\n\
         \x20      report of area no ring reached; a non-trivial figure means the all-contour row \
         is cheap\n\
         \x20      because it left material, not because it is fast.\n\
         \x20      SECONDARY (DENSITY-CONFOUNDED, not a gap detector): swept band = cut length × \
         stepover.\n\
         \x20      shared 0° lattice {shared_points} pts = {:.1}%; all-raster {raster_cut:.0} mm = \
         {:.1}%; all-contour\n\
         \x20      {contour_cut:.0} mm = {:.1}%. The cascade tightens its ring advance on slope and \
         its helical\n\
         \x20      connectors count as cut length, so a contour figure ABOVE the raster's is \
         expected and is\n\
         \x20      not double coverage — the F-034 column already charges for it.",
        100.0 * untouched / area.max(1e-9),
        100.0 * uncut_core / area.max(1e-9),
        coverage_pct(shared_points, input.stepover, area),
        swept_coverage_pct(raster_cut, input.stepover, area),
        swept_coverage_pct(contour_cut, input.stepover, area),
    );
    println!(
        "     DELTAS (ceiling arm): all-raster → all-contour {:.3}x, all-raster → hybrid {:.3}x,\n\
         \x20      0° undivided → all-contour {:.3}x.\n\
         \x20      BAR 1 — the all-raster row IS §0i's `0° cells` ceiling row (recorded \
         {:.1} s): here {:.1} s.\n\
         \x20      BAR 2 — §0i's winner, the global `PCA cells` ceiling row (recorded \
         {D2_WINNER_BAR_S:.1} s),\n\
         \x20      recomputed in this binary: {}. all-contour {:.1} s, hybrid {:.1} s.",
        delta(&raster_rows[1], &contour_rows[1]),
        delta(&raster_rows[1], &hybrid_rows[1]),
        delta(&undivided[1], &contour_rows[1]),
        RECORDED_CEILING_CELLS
            .get(region_index)
            .map_or(f64::NAN, |pair| pair.1),
        raster_rows[1].time_s,
        winner_bar.map_or_else(
            || "n/a (no global axis on this region)".to_owned(),
            |bar| format!(
                "{bar:.1} s, hybrid ÷ bar {:.3}x",
                bar / hybrid_rows[1].time_s.max(1e-9)
            )
        ),
        contour_rows[1].time_s,
        hybrid_rows[1].time_s,
    );

    Some(StageNTotals {
        undivided: undivided[1].time_s,
        all_raster: raster_rows[1].time_s,
        all_contour: contour_rows[1].time_s,
        hybrid: hybrid_rows[1].time_s,
    })
}

fn stage_n(input: &A3Inputs<'_>, regions: &[RegionCells]) {
    use rs_cam_core::machine::kinematics::MachineKinematics;
    use rs_cam_core::surface_link::LinkCeiling;

    println!("========== STAGE N — D2: per-cell PATTERN, contour rings vs raster ==========");
    if regions.is_empty() {
        println!("\n     SKIP: no Shallow regions to price.\n");
        return;
    }
    let mesh = input.mesh;
    let safe_z = mesh.bbox.max.z + 5.0;
    let block_top_z = mesh.bbox.max.z;
    let kinematics = MachineKinematics {
        acceleration_mm_s2: MACHINE_ACCEL_SCALAR,
        acceleration_xyz_mm_s2: Some(MACHINE_ACCEL_XYZ),
        junction_deviation_mm: JUNCTION_DEVIATION_MM,
        ..MachineKinematics::default()
    };
    println!(
        "\n   The question: §0d refuted the contour cascade on UNDIVIDED regions (0.91×) and \
         named the\n\
         \x20  mechanism — long, wildly-varying perimeters, 3.2× the moves, 29% more cutting. That \
         is a\n\
         \x20  claim about the SHAPE the cascade was seeded from, so it does not carry to a \
         MONOTONE CELL.\n\
         \x20  The opposing prior is §0j's: a per-cell candidate has no shared lattice, and \
         misaligned\n\
         \x20  per-cell lattices measured as a COST (+9% cutting distance). A contour cell has no \
         shared\n\
         \x20  lattice either. Which effect dominates is what this stage measures.\n\
         \x20\n\
         \x20  RING GENERATOR PROVENANCE: Stage D's, unchanged — \
         `scallop_toolpath_structured_annotated_with_cancel`\n\
         \x20  with Stage D's exact ScallopParams, handed ONE CELL polygon as its \
         `boundary_regions`.\n\
         \x20  Under P2.3 a non-empty boundary REPLACES the mesh-footprint rectangle as the \
         cascade seed,\n\
         \x20  so this is a genuine cell-hugging offset cascade from shipped code, not an adapter. \
         Rings\n\
         \x20  ride the generator's own drop-cutter surface (surface Z), at its own generation \
         resolution.\n\
         \x20\n\
         \x20  Same ceiling as Stage L / Stage M (MACHINED STOCK: the Ø{ROUGH_DIAMETER_MM:.0} \
         rough at axial\n\
         \x20  leave {ROUGH_STOCK_TO_LEAVE_AXIAL_MM:.1} mm, then the tier-0 R1.5 where tier 0 \
         machines), same\n\
         \x20  production relink parameters, same F-034 integrator, BOTH regimes on every row (E1 \
         discipline).\n\
         \x20  BARS: {:.1} s (§0i's `0° cells` ceiling row on region 1 — reproduced here as the \
         all-raster\n\
         \x20  row, with a `= FINDINGS` mark) and {D2_WINNER_BAR_S:.1} s (§0i's winner, the global \
         `PCA cells`\n\
         \x20  ceiling row, RECOMPUTED in this binary rather than read from the document).\n",
        RECORDED_CEILING_CELLS[0].1
    );

    let bounds = ceiling_stock_bounds(mesh, regions);
    let stock = a3_machined_stock(input, bounds);
    report_ceiling_population(&stock, input, &regions[0].boundary, safe_z);

    let machined_ceiling = LinkCeiling {
        stock: Some(&stock),
        tool_radius: input.fine.envelope_radius_mm(),
        fallback_top_z: block_top_z,
    };
    let arms = [
        LinkRegime::fresh_stock(safe_z),
        LinkRegime::rest_op("ceiling", safe_z, machined_ceiling),
    ];
    print_a3_header();

    let mut totals: Vec<StageNTotals> = Vec::new();
    for (region_index, region) in regions.iter().enumerate() {
        if let Some(row) = stage_n_region(input, region, region_index, &arms, &kinematics) {
            totals.push(row);
        }
    }
    if totals.is_empty() {
        println!("\n     REFUSE total: no region produced a comparable candidate set.\n");
        return;
    }
    let mut undivided = 0.0_f64;
    let mut all_raster = 0.0_f64;
    let mut all_contour = 0.0_f64;
    let mut hybrid = 0.0_f64;
    for row in &totals {
        undivided += row.undivided;
        all_raster += row.all_raster;
        all_contour += row.all_contour;
        hybrid += row.hybrid;
    }
    println!(
        "\n   TOP-{} TOTAL, CEILING ARM (the operator-honest regime):\n\
         \x20    0° undivided   {undivided:.1} s\n\
         \x20    all-raster     {all_raster:.1} s   (= §0i's `0° cells` row, reproduction-checked)\n\
         \x20    all-contour    {all_contour:.1} s   (every cell an offset-ring cascade)\n\
         \x20    hybrid         {hybrid:.1} s   (per-cell proxy pick — D3's CHEAPEST BOUND)\n\
         \x20    raster → contour {:.3}x, raster → hybrid {:.3}x\n",
        totals.len(),
        all_raster / all_contour.max(1e-9),
        all_raster / hybrid.max(1e-9),
    );
    println!(
        "   READING RULES, all of which bind whatever the numbers say:\n\
         \x20  * The rig BIASES AGAINST CONTOUR. Stage I's cells are marching-squares\n\
         \x20    reconstructions of a lattice, so their perimeters are STAIRCASED at the raster\n\
         \x20    pitch — not the smooth cell boundary a production decomposition (C2) would hand a\n\
         \x20    cascade. `tolerance` {OP_TOLERANCE_MM} mm does not simplify a staircase of that\n\
         \x20    amplitude away, so every ring inherits jagged chords the junction-deviation model\n\
         \x20    crawls through. A contour WIN despite this is robust; a NARROW contour loss is\n\
         \x20    inconclusive and is NOT a refutation.\n\
         \x20  * The hybrid is a BOUND, not a router: its picks come from a standalone per-cell\n\
         \x20    proxy because relink is a whole-region operation and a per-cell relinked cost does\n\
         \x20    not exist. Read it as the ceiling on what a cheap local pattern picker buys.\n\
         \x20  * Contour and hybrid rows carry the coverage figures INSTEAD of Stage J's membership\n\
         \x20    guard — a ring cascade is not a lattice candidate. C4's rendered-surface review\n\
         \x20    binds anything built on them, and cusp quality is assumed equal, not measured\n\
         \x20    (CHECKPOINT_C_EVIDENCE records cascade ACHIEVED cusp at 2.0–4.9× its dial).\n\
         \x20  * Cell visit order is emission order on all three candidates: §0j measured greedy\n\
         \x20    nearest-neighbour order as byte-identical output under `reorder: true`.\n\
         \x20  * Three regions, one tier, one fixture, no inter-region routing — §0d's scope\n\
         \x20    caveats carry over unchanged.\n"
    );
}

/// D2 — per-cell PATTERN, contour rings vs raster (`PROGRAMME.md` Track D, D2).
///
/// ```text
/// cargo test -p rs_cam_core --test thin_organic_island_widths \
///   wanaka_per_cell_pattern_d2 -- --ignored --nocapture
/// ```
///
/// Reuses [`d1_setup`] rather than making a fourth copy of the A3 setup: that
/// is read-only sharing, so the property Stage M's own note protects — an
/// additive stage must not be able to move an existing stage's numbers — still
/// holds, because nothing in the setup or in Stages L/M is edited.
#[test]
#[ignore = "evidence run — needs the operator's wanaka mesh (not in repo)"]
fn wanaka_per_cell_pattern_d2() {
    let Some(setup) = d1_setup() else {
        return;
    };
    let input = A3Inputs {
        mesh: &setup.mesh,
        index: &setup.index,
        fine: &setup.fine,
        coarse: &setup.coarse,
        tier_map: &setup.tier_map,
        zero_grid: &setup.zero_grid,
        stepover: setup.stepover,
    };
    stage_n(&input, &setup.regions);
}
