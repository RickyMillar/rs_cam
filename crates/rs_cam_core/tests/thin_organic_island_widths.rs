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
//! cargo test -p rs_cam_core --test thin_organic_island_widths \
//!   wanaka_monotone_cells_kept_retracts -- --ignored --nocapture
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

use std::path::Path;

use rs_cam_core::classify_probe::ClassificationSampler;
use rs_cam_core::contour_extract::marching_squares_bool_grid;
use rs_cam_core::finish_planner::{FinishBand, FinishPlannerParams, decompose};
use rs_cam_core::finish_setup::build_classification_surface_with_sampler_and_cancel;
use rs_cam_core::geo::P2;
use rs_cam_core::grid_field::distance_transform_2d;
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::polygon::{Polygon2, detect_containment, shoelace_area};
use rs_cam_core::tier_islands::{TierIslandParams, extract_tier_islands};
use rs_cam_core::tier_map::{ResidualTreatment, TierLadder, TierMapParams, compute_tier_map};
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
    stage_j(&mesh, &index, &r10, &grid, &cells);
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
    use rs_cam_core::machine_kinematics::{MachineKinematics, compute_cycle_time};
    use rs_cam_core::region_set::RegionSet;
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
    let grid = rs_cam_core::dropcutter::batch_drop_cutter(
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
        let lk = rs_cam_core::machine_kinematics::LinkKinematics {
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
    use rs_cam_core::machine_kinematics::{LinkKinematics, MachineKinematics, compute_cycle_time};
    use rs_cam_core::region_set::RegionSet;
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
        let grid = rs_cam_core::dropcutter::batch_drop_cutter(
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
    use rs_cam_core::machine_kinematics::{LinkKinematics, MachineKinematics, compute_cycle_time};
    use rs_cam_core::region_set::RegionSet;
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
            let grid = rs_cam_core::dropcutter::batch_drop_cutter(
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
    use rs_cam_core::machine_kinematics::{LinkKinematics, MachineKinematics, compute_cycle_time};
    use rs_cam_core::region_set::RegionSet;
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

    let grid = rs_cam_core::dropcutter::batch_drop_cutter(
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

fn grid_for_stage_i(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &TaperedBallEndmill,
    stepover: f64,
) -> rs_cam_core::dropcutter::DropCutterGrid {
    rs_cam_core::dropcutter::batch_drop_cutter(
        mesh,
        index,
        cutter,
        stepover,
        0.0,
        mesh.bbox.min.z - 0.1,
    )
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

fn polygons_for_lattice_cell(
    grid: &rs_cam_core::dropcutter::DropCutterGrid,
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
    let origin = grid.get(0, 0);
    let loops = marching_squares_bool_grid(
        &mask,
        rows,
        cols,
        origin.x - grid.x_step,
        origin.y - grid.y_step,
        grid.x_step,
    );
    // Keep one-lattice-point cells too: Stage J rejects any reconstructed
    // candidate that loses a baseline emitted point, so an area floor would
    // hide a comparison error rather than make the cell set healthier.
    let candidates = loops
        .into_iter()
        .filter(|points| points.len() >= 3 && shoelace_area(points).abs() > 1e-12)
        .map(Polygon2::new)
        .collect();
    let mut polygons = detect_containment(candidates);
    for polygon in &mut polygons {
        polygon.ensure_winding();
    }
    polygons
}

fn lattice_boustrophedon_cells(
    grid: &rs_cam_core::dropcutter::DropCutterGrid,
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
    grid: &rs_cam_core::dropcutter::DropCutterGrid,
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

// ── STAGE J ─────────────────────────────────────────────────────────────
//
// E1's reusable comparison kernel.  Candidate constructors hand it raw
// toolpaths; it applies the SAME production relink to every arm and then the
// F-034 integrator.  A cell candidate may only be compared after the emitted
// lattice membership check below proves it has the baseline's cut population.

struct CandidateCost {
    moves: usize,
    cutting_mm: f64,
    time_s: f64,
    fragments: usize,
    linked: usize,
    kept_retracts: usize,
}

fn relink_and_cost(
    raw: rs_cam_core::toolpath::Toolpath,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &TaperedBallEndmill,
    boundary: &rs_cam_core::region_set::RegionSet<'_>,
    kinematics: &rs_cam_core::machine_kinematics::MachineKinematics,
    safe_z: f64,
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
        safe_z,
        link_kinematics: Some(&link_kinematics),
        reorder: true,
        boundary: Some(boundary),
        link_ceiling: None,
        flush_ride: false,
        airborne_links_may_leave_territory: false,
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
    CandidateCost {
        moves: toolpath.moves.len(),
        cutting_mm: toolpath.total_cutting_distance(),
        time_s: compute_cycle_time(&toolpath, kinematics, MAX_FEED_MM_MIN, RAPID_FEED_MM_MIN),
        fragments: report.fragments,
        linked: report.surface_links,
        kept_retracts: report.retract_links,
    }
}

fn raster_candidate(
    grid: &rs_cam_core::dropcutter::DropCutterGrid,
    regions: &[Polygon2],
    safe_z: f64,
    effective_min_z: f64,
) -> rs_cam_core::toolpath::Toolpath {
    use rs_cam_core::region_set::RegionSet;
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
    grid: &rs_cam_core::dropcutter::DropCutterGrid,
    boundary: &Polygon2,
    cells: &[Polygon2],
    effective_min_z: f64,
) -> bool {
    use rs_cam_core::region_set::RegionSet;

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
    grid: &rs_cam_core::dropcutter::DropCutterGrid,
    regions: &[RegionCells],
) {
    use rs_cam_core::machine_kinematics::MachineKinematics;
    use rs_cam_core::region_set::RegionSet;

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
