//! Avenue F, phase F1 — the spacing-prize split
//! (`planning/metrology_2026-09-02/FINDINGS.md` §M3, pre-registered).
//!
//! # The question
//!
//! The shipped raster pays one spacing per region: the region's worst
//! covered cell (clamped at 45°) sets the derate for every pass in it.
//! The prize — the distance between that and per-cell perfect spacing —
//! has two candidate capture routes:
//!
//! * **decomposition** — redraw regions so each region's worst point is
//!   less bad; keep one spacing per region;
//! * **along-pass** — spacing that varies within a pass.
//!
//! This instrument prices the decomposition route honestly: a slope-banded
//! partition of the owned Shallow cells, where every 8-connected band
//! component smaller than the planner's own `min_region_area_mm2` is
//! merged into the steeper neighbouring band before costing (a region the
//! tool cannot usefully enter cannot hold its own spacing). The operator's
//! pre-run suspicion — "lots of tiny islands" — is bar B-confetti.
//!
//! # Model (stated limits)
//!
//! Allowed XY pitch ∝ `cos θ(cell)`, θ from the PRODUCTION classification
//! slope map, clamped at the 45° threshold. Slope only — no curvature
//! refund (conservative). Cutting length is modelled as cells ÷ cos θ;
//! links, retracts and path overhead are excluded — this prices the
//! SPACING POLICY, nothing else. Owned cells steeper than the clamp pay
//! the clamp in every arm (their spec story is the absorption ticket, not
//! this one).
//!
//! Setup (mesh, tiers, classification, decompose) is verbatim
//! `band_cell_ownership_g2.rs`, which is verbatim
//! `valley_branch_falsifier_h1.rs`.
//!
//! NUMBERS ONLY — the B-capture / B-confetti ruling goes to the FINDINGS
//! file beside the pre-registration.
//!
//! `#[ignore]` — evidence run; needs the operator's wanaka mesh. SKIPs
//! when absent.

#![allow(
    clippy::print_stderr,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::too_many_lines
)]

use std::path::Path;

use rs_cam_core::classify_probe::ClassificationSampler;
use rs_cam_core::finish_planner::{FinishBand, FinishPlannerParams, decompose};
use rs_cam_core::finish_setup::build_classification_surface_with_sampler_and_cancel;
use rs_cam_core::geo::P2;
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::tier_islands::{TierIslandParams, extract_tier_islands};
use rs_cam_core::tier_map::{ResidualTreatment, TierLadder, TierMapParams, compute_tier_map};
use rs_cam_core::tool::{MillingCutter, TaperedBallEndmill};
use rs_cam_core::unified_finish::unified_finish_classification_resolution;

// ── fixtures and dials — verbatim from `band_cell_ownership_g2.rs` ──────

const WANAKA_MESH: &str = "/home/ricky/Downloads/wanaka200/rivmap_export/terrain.stl";

const CELL_MM: f64 = 0.3;
const TOLERANCE_MM: f64 = 0.05;
const MARGIN_MM: f64 = 0.5;
const COARSENESS: f64 = 1.0;
const OVERLAP_MM: f64 = 2.0;
const MAX_REGIONS_PER_TIER: usize = 24;
const OP_TOLERANCE_MM: f64 = 0.05;

/// The banding ladders under test (degrees; last edge is the clamp).
/// K counts the bands. Pre-registered in FINDINGS §M3.
const LADDERS: [&[f64]; 3] = [
    &[30.0, 45.0],                   // K = 2
    &[20.0, 40.0, 45.0],             // K = 3
    &[10.0, 20.0, 30.0, 40.0, 45.0], // K = 5
];

#[test]
#[ignore = "evidence run — needs the operator's wanaka mesh (not in repo)"]
fn wanaka_spacing_prize_split_f1() {
    eprintln!(
        "\n========== Avenue F, F1 — SPACING-PRIZE SPLIT ==========\n\
         Pre-registration: planning/metrology_2026-09-02/FINDINGS.md §M3\n\
         (bars B-capture and B-confetti fixed before this run). NUMBERS ONLY.\n"
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

    // Owned-cell θ (degrees, clamped) and region assignment.
    let owned: Vec<bool> = planned
        .labels
        .iter()
        .map(|l| *l == Some(FinishBand::Shallow))
        .collect();
    let theta_deg: Vec<f64> = slope
        .angles
        .iter()
        .map(|a| a.to_degrees().min(clamp_deg))
        .collect();

    // Shallow region polygons, largest first; per-cell region id via
    // bbox scan (first polygon wins — same-band polygons rarely overlap).
    let mut shallow: Vec<&rs_cam_core::polygon::Polygon2> = planned
        .regions
        .iter()
        .filter(|r| r.band == FinishBand::Shallow)
        .map(|r| &r.polygon)
        .collect();
    shallow.sort_by(|a, b| b.area().total_cmp(&a.area()));

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

    // Production θ_max per region: ALL covered in-polygon cells (the
    // shipped derate population), clamped. The derate's coverage-cliff
    // neighbour filter is a micro-refinement the clamp makes moot on this
    // board (G2: every large region pins at the clamp); stated, not
    // mirrored here.
    let mut theta_max_region = vec![0.0_f64; shallow.len()];
    for i in 0..rows * cols {
        if !covered[i] {
            continue;
        }
        if let Some(r) = region_of[i] {
            theta_max_region[r] = theta_max_region[r].max(theta_deg[i]);
        }
    }

    // ── The three length quantities (units: cells / cos, ∝ cutting mm) ──
    let mut l_today = 0.0_f64;
    let mut l_floor = 0.0_f64;
    let mut owned_cells = 0usize;
    let mut owned_no_region = 0usize;
    let mut theta_pop: Vec<f64> = Vec::new();
    for i in 0..rows * cols {
        if !owned[i] {
            continue;
        }
        owned_cells += 1;
        theta_pop.push(theta_deg[i]);
        l_floor += 1.0 / theta_deg[i].to_radians().cos();
        match region_of[i] {
            Some(r) => l_today += 1.0 / theta_max_region[r].to_radians().cos(),
            None => {
                // Owned but in no polygon (extraction edge case): pays the
                // clamp, today's worst.
                owned_no_region += 1;
                l_today += 1.0 / clamp_deg.to_radians().cos();
            }
        }
    }
    theta_pop.sort_by(f64::total_cmp);
    let q = |f: f64| theta_pop[((theta_pop.len() - 1) as f64 * f).round() as usize];
    let prize = l_today - l_floor;
    eprintln!(
        "population: {owned_cells} owned Shallow cells ({:.0} mm² XY), {owned_no_region} in no polygon\n\
         owned θ (clamped): p50 {:.1}° p90 {:.1}° p99 {:.1}° max {:.1}°\n\
         L_today {:.0}  L_floor {:.0}  → today = {:.3}× floor; PRIZE = {:.0} ({:.1} % of L_today)",
        owned_cells as f64 * cell_area,
        q(0.5),
        q(0.9),
        q(0.99),
        q(1.0),
        l_today,
        l_floor,
        l_today / l_floor,
        prize,
        100.0 * prize / l_today,
    );

    // ── Banded decompositions ────────────────────────────────────────────
    for edges in LADDERS {
        let k = edges.len();
        let band_of_theta =
            |t: f64| -> usize { edges.iter().position(|&e| t <= e).unwrap_or(k - 1) };
        // Idealized: each cell pays its band's upper edge.
        let mut band_assign: Vec<Option<usize>> = vec![None; rows * cols];
        let mut l_ideal = 0.0_f64;
        for i in 0..rows * cols {
            if !owned[i] {
                continue;
            }
            let b = band_of_theta(theta_deg[i]);
            band_assign[i] = Some(b);
            l_ideal += 1.0 / edges[b].to_radians().cos();
        }

        // Machinable-honest: components of each band below the planner's
        // own min-region area merge into the steeper neighbour band, from
        // shallowest band upward, until stable (bounded by k passes). The
        // steepest band keeps its confetti — it already pays the clamp.
        let mut census_lines = String::new();
        for (pass_band, &edge) in edges.iter().enumerate().take(k - 1) {
            let (moved, comps, mach_area, total_area) = merge_submachinable(
                &mut band_assign,
                rows,
                cols,
                pass_band,
                cell_area,
                min_region_area_mm2,
            );
            census_lines.push_str(&format!(
                "     band ≤{:>4.0}°: {:>7.0} mm² in {:>5} components, machinable {:>5.1} %, \
                 merged up {:>7.0} mm²\n",
                edge,
                total_area,
                comps,
                if total_area > 0.0 {
                    100.0 * mach_area / total_area
                } else {
                    0.0
                },
                moved,
            ));
        }
        let mut l_mach = 0.0_f64;
        for b in band_assign.iter().flatten() {
            l_mach += 1.0 / edges[*b].to_radians().cos();
        }
        let cap_ideal = 100.0 * (l_today - l_ideal) / prize;
        let cap_mach = 100.0 * (l_today - l_mach) / prize;
        eprintln!(
            "\n---------- K = {k} bands, edges {edges:?} ----------\n\
             {census_lines}\
             \x20  L_ideal {l_ideal:.0} → captures {cap_ideal:.1} % of the prize (contiguity ignored)\n\
             \x20  L_machinable {l_mach:.0} → captures {cap_mach:.1} % of the prize (confetti merged up)\n\
             \x20  confetti retention (machinable ÷ idealized capture): {:.1} %",
            if cap_ideal > 0.0 {
                100.0 * cap_mach / cap_ideal
            } else {
                0.0
            }
        );
    }
    eprintln!("\ntotal wall time {:.0} s", started.elapsed().as_secs_f64());
}

/// Merge every 8-connected component of `band` cells whose area is below
/// `min_area_mm2` into `band + 1`. Returns (moved mm², component count,
/// machinable mm², band total mm²) measured BEFORE the merge.
fn merge_submachinable(
    band_assign: &mut [Option<usize>],
    rows: usize,
    cols: usize,
    band: usize,
    cell_area: f64,
    min_area_mm2: f64,
) -> (f64, usize, f64, f64) {
    let mut visited = vec![false; rows * cols];
    let mut moved_mm2 = 0.0_f64;
    let mut components = 0usize;
    let mut mach_mm2 = 0.0_f64;
    let mut total_mm2 = 0.0_f64;
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
        components += 1;
        let area = comp.len() as f64 * cell_area;
        total_mm2 += area;
        if area < min_area_mm2 {
            moved_mm2 += area;
            for &i in &comp {
                band_assign[i] = Some(band + 1);
            }
        } else {
            mach_mm2 += area;
        }
    }
    (moved_mm2, components, mach_mm2, total_mm2)
}
