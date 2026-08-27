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

    println!(
        "SHALLOW+MIDSTEEP TOTAL: {grand_frag} raster fragments vs {grand_rings} rings.\n\
         Read against FINDINGS.md: the width-based routing rule (§1.1) is dead — \
         Stage B measured 0% of shallow area under 8 stepovers. If Stage C's \
         fragment ratio is large anyway, the LEVER survives and only its \
         TRIGGER needs replacing (elongation/crossings, not width)."
    );
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
