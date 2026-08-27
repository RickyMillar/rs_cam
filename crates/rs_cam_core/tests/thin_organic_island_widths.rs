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
//! cargo test -p rs_cam_core --test thin_organic_island_widths -- --ignored --nocapture
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
use rs_cam_core::finish_planner::{FinishBand, FinishPlannerParams, decompose};
use rs_cam_core::finish_setup::build_classification_surface_with_sampler_and_cancel;
use rs_cam_core::geo::P2;
use rs_cam_core::grid_field::distance_transform_2d;
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::polygon::Polygon2;
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
fn report_widths(label: &str, rows: &mut Vec<(f64, f64)>, stepover: f64) {
    if rows.is_empty() {
        println!("   {label}: (empty)\n");
        return;
    }
    rows.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    println!("   {label}: {} region(s)", rows.len());
    println!("     {:>10}  {:>10}  {:>12}", "area mm²", "width mm", "stepovers");
    for (w, a) in rows.iter().take(10) {
        let so = if stepover > 0.0 { w / stepover } else { f64::NAN };
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
        let share: f64 = rows.iter().filter(|(w, _)| *w <= bar).map(|(_, a)| *a).sum();
        let pct = if total > 0.0 { 100.0 * share / total } else { 0.0 };
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
    println!("========== STAGE B — decompose regions inside tier 1 (what the generator sees) ==========\n");
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

    for band in [FinishBand::Shallow, FinishBand::MidSteep, FinishBand::VerySteep] {
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
        report_widths("planned regions (post overlap dilation)", &mut rows, stepover);
    }

    println!(
        "DECISION: Lever 1 is worth building only if the SHALLOW band's area is \
         dominated by regions under ~8 stepovers wide. Stage B is the honest \
         test — Stage A is upstream of the overlap dilation and of decompose's \
         own close/min-area, both of which fatten and merge."
    );
}
