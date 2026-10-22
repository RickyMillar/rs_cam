//! **Phase 4 / gate G3 instrument — how many wanaka small-tool regions does
//! the compact spiral accept?** (`planning/adaptive_research_2026-10-23/PLAN.md`,
//! "Phase 4 — spiral finish for the many-island case", step 1.)
//!
//! The question is a COUNT, nothing else: no gouge check, no costing, no
//! ranking. For every owned island of every tier finer than the 6 mm end
//! mill, this builds the island's nested iso-distance level sets and hands
//! them to [`rs_cam_core::finish::spiral_finish_compact::bridge_nested_levels`],
//! then records `Ok` or the exact [`CompactSpiralRefusal`].
//!
//! # The islands
//!
//! The planner's own path: `ProjectSession::preview_multitool_plan` on
//! `planning/multitool_2026-08-23/wanaka200_mt1.toml`, setup 1 (the front, 3D
//! setup), ladder = tools 0-4 (6 mm end mill + R2.0/R1.5/R1.0/R0.5 tapered
//! balls; the planner sorts them coarse to fine by cusp radius). The V-bit
//! (tool 5) is an engraving tool and is not on the finishing ladder. Every
//! other dial is the planner default (`MultitoolPlanSpec::default()`): cell
//! 0.4, tolerance 0.05, margin 0.5, slope-compensated residual, overlap 2.0,
//! cap 24 per tier, cusp 0.03.
//!
//! # The level sets — a stated deviation from C1
//!
//! The module doc defines its input as "the medial/EDT field ... whose level
//! sets are iso-distance offsets of the region boundary". C1
//! (`spiral_finish_compact_c1.rs`) reached those through
//! `direction_field::solve_paths_with_target` with a CLOSED-FORM EDT, which
//! exists only on its disk-shaped fixtures. On a polygon island this
//! instrument takes the iso-distance offsets DIRECTLY: a grid EDT of the
//! island (`geometry::grid_field::distance_transform_2d`, as the F2 medial arm
//! does) thresholded at `k · stepover` and traced with
//! `geometry::contour_extract::marching_squares_bool_grid`. Every refusal
//! variant tests loop topology and shape (count, closure, winding, star shape,
//! nesting), not spacing, so the direct offsets answer the same question. A
//! disk control runs through the identical pipeline first; it must be `Ok`,
//! or a refusal on wanaka could be this pipeline's staircase and not the
//! island's shape.
//!
//! Restated, with sources (integration tests cannot import from each other):
//!
//! * `equal_cusp_stepover_mm` — `spiral_finish_compact_c1.rs:118`
//!   (also `session::multitool::equal_cusp_stepover_mm`).
//! * the lateral reach and the bridge params — `spiral_finish_compact_c1.rs`
//!   `run_fixture` (`hub_cover_reach_mm` = lateral reach,
//!   `near_hub_bridge_span_rad` = `PAPER_INITIAL_BRIDGE_SHIFT`).
//! * the grid rule (4 cells per stepover, 48..=1024 cells) and the even-odd
//!   scanline fill — `conformal_spiral_synthetic_f2.rs`
//!   (`MEDIAL_CELLS_PER_STEPOVER`, `MEDIAL_MIN_CELLS`, `MEDIAL_MAX_CELLS`,
//!   `rasterise_polygons`), here on the island's own padded bbox instead of
//!   an origin-centred square.
//!
//! # Running it
//!
//! ```text
//! cargo test --release -p rs_cam_core --features research,test-support \
//!     --test spiral_finish_compact_wanaka_g3 -- --ignored --nocapture
//! ```

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::print_stderr,
    reason = "test code: a panic is how a test reports a failure; the test prints its measurements for the reader"
)]

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use rs_cam_core::finish::conformal_spiral::PAPER_INITIAL_BRIDGE_SHIFT;
use rs_cam_core::finish::spiral_finish_compact::{
    CompactSpiralParams, CompactSpiralRefusal, bridge_nested_levels,
};
use rs_cam_core::geo::{P2, P3};
use rs_cam_core::geometry::contour_extract::marching_squares_bool_grid;
use rs_cam_core::geometry::grid_field::distance_transform_2d;
use rs_cam_core::ids::{ModelId, ToolId};
use rs_cam_core::maps::tier_map::TierMap;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::project_file::ProjectFile;
use rs_cam_core::session::{MultitoolPlanSpec, ProjectSession};

const WANAKA_MESH: &str = "/home/ricky/Downloads/wanaka200/rivmap_export/terrain.stl";
/// The fixture project, relative to this crate.
const PROJECT_REL: &str = "../../planning/multitool_2026-08-23/wanaka200_mt1.toml";
/// Setup 1 = "Setup 2 — front", the setup that carries the 3D finish ops.
const SETUP_INDEX: usize = 1;
/// Model 0 = `terrain.stl`.
const MODEL_ID: usize = 0;
/// Tools 0-4: 6 mm end mill, R0.5, R1.0, R1.5, R2.0 tapered balls.
const LADDER_TOOL_IDS: [usize; 5] = [0, 1, 2, 3, 4];

/// `conformal_spiral_synthetic_f2.rs` `MEDIAL_CELLS_PER_STEPOVER`.
const CELLS_PER_STEPOVER: f64 = 4.0;
/// `conformal_spiral_synthetic_f2.rs` `MEDIAL_MIN_CELLS`.
const MIN_CELLS: usize = 48;
/// `conformal_spiral_synthetic_f2.rs` `MEDIAL_MAX_CELLS`.
const MAX_CELLS: usize = 1024;

/// `s = 2·√(2Rh − h²)` (spiral_finish_compact_c1.rs:118).
fn equal_cusp_stepover_mm(cusp_radius_mm: f64, h: f64) -> f64 {
    if h <= 0.0 || h >= cusp_radius_mm {
        return 0.0;
    }
    2.0 * (2.0 * cusp_radius_mm * h - h * h).sqrt()
}

/// Lateral reach of a ball at cusp `h` (spiral_finish_compact_c1.rs
/// `run_fixture`, `lateral_reach`).
fn lateral_reach_mm(ball_radius_mm: f64, h: f64) -> f64 {
    let v = ball_radius_mm - h;
    (ball_radius_mm * ball_radius_mm - v * v).max(0.0).sqrt()
}

/// One island's padded raster.
struct IslandGrid {
    nx: usize,
    ny: usize,
    /// World XY of cell (row 0, col 0)'s CENTRE.
    x0: f64,
    y0: f64,
    cell: f64,
    inside: Vec<bool>,
}

/// Even-odd scanline fill of one polygon (exterior and holes together) onto
/// its own padded bbox grid. Restates `rasterise_polygons` from
/// `conformal_spiral_synthetic_f2.rs` on a rectangle that is not centred on
/// the origin.
fn rasterise_island(polygon: &Polygon2, stepover: f64) -> IslandGrid {
    let [bx0, by0, bx1, by1] = polygon.bbox();
    let pad = stepover + 2.0 * stepover / CELLS_PER_STEPOVER;
    let (wx, wy) = (bx1 - bx0 + 2.0 * pad, by1 - by0 + 2.0 * pad);
    let extent = wx.max(wy);
    let mut cell = stepover / CELLS_PER_STEPOVER;
    if extent / cell > MAX_CELLS as f64 {
        cell = extent / MAX_CELLS as f64;
    }
    if extent / cell < MIN_CELLS as f64 {
        cell = extent / MIN_CELLS as f64;
    }
    let nx = (wx / cell).ceil() as usize + 1;
    let ny = (wy / cell).ceil() as usize + 1;
    let x0 = bx0 - pad + 0.5 * cell;
    let y0 = by0 - pad + 0.5 * cell;

    let mut rings: Vec<&[P2]> = vec![&polygon.exterior];
    for hole in &polygon.holes {
        rings.push(hole);
    }
    let mut inside = vec![false; nx * ny];
    let mut crossings: Vec<f64> = Vec::new();
    let last = (nx - 1) as f64;
    for row in 0..ny {
        let y = y0 + cell * row as f64;
        crossings.clear();
        for ring in &rings {
            let n = ring.len();
            if n < 3 {
                continue;
            }
            for (i, a) in ring.iter().enumerate() {
                let b = ring[(i + 1) % n];
                // Half-open in Y, as F2: a vertex on the scanline counts once.
                if (a.y <= y) == (b.y <= y) {
                    continue;
                }
                let t = (y - a.y) / (b.y - a.y);
                let x = a.x + t * (b.x - a.x);
                if x.is_finite() {
                    crossings.push(x);
                }
            }
        }
        crossings.sort_by(f64::total_cmp);
        for pair in crossings.as_chunks::<2>().0 {
            let lo = ((pair[0] - x0) / cell).ceil();
            let hi = ((pair[1] - x0) / cell).floor();
            if !lo.is_finite() || !hi.is_finite() || hi < 0.0 || lo > last || hi < lo {
                continue;
            }
            let lo = lo.max(0.0) as usize;
            let hi = hi.min(last) as usize;
            for col in lo..=hi {
                inside[row * nx + col] = true;
            }
        }
    }
    IslandGrid {
        nx,
        ny,
        x0,
        y0,
        cell,
        inside,
    }
}

/// The island's nested iso-distance level sets: level `k` is the boundary of
/// `{EDT ≥ k·stepover}`, `k = 0, 1, …` (level 0 is the island outline). Each
/// loop is closed by repeating its first point (the convention
/// `bridge_nested_levels` takes); `chain_segments` drops that repeat.
/// Empty levels past the EDT peak are not emitted (C1 drops them too).
fn iso_distance_levels(
    grid: &IslandGrid,
    stepover: f64,
    z_at: &dyn Fn(f64, f64) -> f64,
) -> Vec<Vec<Vec<P3>>> {
    let outside: Vec<bool> = grid.inside.iter().map(|&v| !v).collect();
    let edt_mm: Vec<f64> = distance_transform_2d(&outside, grid.ny, grid.nx)
        .into_iter()
        .map(|d| d * grid.cell)
        .collect();
    let peak = edt_mm.iter().copied().fold(0.0f64, f64::max);
    let mut levels = Vec::new();
    let mut k = 0usize;
    loop {
        let d = k as f64 * stepover;
        if k > 0 && d > peak {
            break;
        }
        let mask: Vec<bool> = if k == 0 {
            grid.inside.clone()
        } else {
            edt_mm.iter().map(|&e| e >= d).collect()
        };
        let loops =
            marching_squares_bool_grid(&mask, grid.ny, grid.nx, grid.x0, grid.y0, grid.cell);
        let closed: Vec<Vec<P3>> = loops
            .into_iter()
            .filter(|l| l.len() >= 3)
            .map(|l| {
                let mut out: Vec<P3> = l
                    .iter()
                    .map(|p| P3::new(p.x, p.y, z_at(p.x, p.y)))
                    .collect();
                out.push(out[0]);
                out
            })
            .collect();
        if !closed.is_empty() {
            levels.push(closed);
        }
        k += 1;
    }
    levels
}

/// The C1 bridge params for a tool of cusp radius `cusp_r`.
fn bridge_params(cusp_r: f64, h: f64) -> CompactSpiralParams {
    CompactSpiralParams {
        hub_cover_reach_mm: lateral_reach_mm(cusp_r, h),
        near_hub_bridge_span_rad: PAPER_INITIAL_BRIDGE_SHIFT,
        ..CompactSpiralParams::default()
    }
}

/// The variant name, the key of the count table.
fn variant(r: &CompactSpiralRefusal) -> &'static str {
    match r {
        CompactSpiralRefusal::NoLevels => "NoLevels",
        CompactSpiralRefusal::LevelNotSingleLoop { .. } => "LevelNotSingleLoop",
        CompactSpiralRefusal::OpenLevelSet { .. } => "OpenLevelSet",
        CompactSpiralRefusal::TooFewVertices { .. } => "TooFewVertices",
        CompactSpiralRefusal::DoesNotEncircleHub { .. } => "DoesNotEncircleHub",
        CompactSpiralRefusal::NotStarShaped { .. } => "NotStarShaped",
        CompactSpiralRefusal::NotNested { .. } => "NotNested",
    }
}

/// Run one polygon through the pipeline; returns (levels, result text, key).
fn run_island(
    polygon: &Polygon2,
    cusp_r: f64,
    h: f64,
    z_at: &dyn Fn(f64, f64) -> f64,
) -> (usize, String, &'static str) {
    let stepover = equal_cusp_stepover_mm(cusp_r, h);
    let grid = rasterise_island(polygon, stepover);
    let levels = iso_distance_levels(&grid, stepover, z_at);
    let (result, report) = bridge_nested_levels(&levels, &bridge_params(cusp_r, h));
    match result {
        Ok(spiral) => (
            levels.len(),
            format!(
                "Ok ({} rings, {} spiral points, grid {}x{} @ {:.3} mm)",
                report.rings,
                spiral.contact.len(),
                grid.nx,
                grid.ny,
                grid.cell
            ),
            "Ok",
        ),
        Err(refusal) => (
            levels.len(),
            format!(
                "{refusal:?} (grid {}x{} @ {:.3} mm)",
                grid.nx, grid.ny, grid.cell
            ),
            variant(&refusal),
        ),
    }
}

/// Nearest-cell `finest_z` of the tier map; 0 off the map or where `NaN`.
/// The bridge reads XY only for every refusal; Z is carried for honesty.
fn finest_z_at(map: &TierMap, x: f64, y: f64) -> f64 {
    let g = &map.grid;
    let col = ((x - g.origin_x) / g.cell_mm).round();
    let row = ((y - g.origin_y) / g.cell_mm).round();
    if col < 0.0 || row < 0.0 {
        return 0.0;
    }
    let (col, row) = (col as usize, row as usize);
    if col >= g.nx || row >= g.ny {
        return 0.0;
    }
    let z = f64::from(map.finest_z[row * g.nx + col]);
    if z.is_finite() { z } else { 0.0 }
}

/// `text` with every `[[setups.toolpaths]]` section (and its sub-tables)
/// removed, and the number removed. A section ends at the first table header
/// that is not a `setups.toolpaths` header.
fn without_toolpath_sections(text: &str) -> (String, usize) {
    let mut out = String::with_capacity(text.len());
    let mut skipping = false;
    let mut dropped = 0usize;
    for line in text.lines() {
        let t = line.trim_start();
        if t.starts_with('[') {
            let is_toolpath =
                t.starts_with("[[setups.toolpaths") || t.starts_with("[setups.toolpaths.");
            if t.starts_with("[[setups.toolpaths]]") {
                dropped += 1;
            }
            skipping = is_toolpath;
        }
        if !skipping {
            out.push_str(line);
            out.push('\n');
        }
    }
    (out, dropped)
}

/// One row of the per-tier table.
struct TierRow {
    tier: u8,
    name: String,
    polys: usize,
    ok: usize,
    area: f64,
    islands: usize,
    counts: BTreeMap<&'static str, usize>,
}

/// A regular `n`-gon disk of radius `r` centred at (cx, cy).
fn disk(cx: f64, cy: f64, r: f64, n: usize) -> Polygon2 {
    let pts: Vec<P2> = (0..n)
        .map(|i| {
            let t = std::f64::consts::TAU * i as f64 / n as f64;
            P2::new(cx + r * t.cos(), cy + r * t.sin())
        })
        .collect();
    Polygon2::new(pts)
}

#[test]
#[ignore = "evidence: real wanaka mesh and project, under ten seconds in release"]
fn spiral_finish_compact_wanaka_g3_refusal_count() {
    let run_start = Instant::now();

    // -- the pipeline control: a disk must be accepted -----------------------
    let h = MultitoolPlanSpec::default().cusp_height_mm;
    for (radius, cusp_r) in [6.0, 3.0]
        .into_iter()
        .flat_map(|r| [0.5, 1.0, 1.5, 2.0].map(move |c| (r, c)))
    {
        let (levels, text, key) =
            run_island(&disk(37.0, -12.0, radius, 256), cusp_r, h, &|_, _| 0.0);
        eprintln!("CONTROL disk r{radius} mm, cusp R{cusp_r:.1}: {levels} levels -> {text}");
        assert_eq!(
            key, "Ok",
            "the disk control refused: the EDT/marching-squares pipeline, not the island \
             shape, would decide the wanaka count"
        );
    }

    if !Path::new(WANAKA_MESH).exists() {
        eprintln!("SKIP: {WANAKA_MESH} not present on this machine.");
        return;
    }
    let project = Path::new(env!("CARGO_MANIFEST_DIR")).join(PROJECT_REL);
    // The fixture's toolpath sections carry keys the loader has retired
    // (`operation.params.depth` on the Drill). This instrument needs only the
    // tools, the models and the setups, so it drops every toolpath section IN
    // MEMORY and parses the rest. The fixture file is not written.
    let text = std::fs::read_to_string(&project).expect("read project");
    let (kept, dropped) = without_toolpath_sections(&text);
    let parsed = ProjectFile::from_toml_str(&kept).expect("parse project without toolpaths");
    let base_dir = project.parent().expect("project dir");
    let session = ProjectSession::from_project_file(parsed, base_dir).expect("build session");
    eprintln!(
        "project: {} ({dropped} [[setups.toolpaths]] sections dropped in memory)",
        project.display()
    );

    let spec = MultitoolPlanSpec {
        setup_index: SETUP_INDEX,
        model_id: ModelId::from_raw(MODEL_ID),
        tool_ids: LADDER_TOOL_IDS
            .iter()
            .map(|&i| ToolId::from_raw(i))
            .collect(),
        ..MultitoolPlanSpec::default()
    };
    eprintln!(
        "dials: cell {} mm, tolerance {} mm, margin {} mm, treatment {:?}, overlap {} mm, \
         max {} regions per tier, coarseness {}, cusp {} mm",
        spec.cell_mm,
        spec.tolerance_mm,
        spec.margin_mm,
        spec.treatment,
        spec.islands.overlap_mm,
        spec.islands.max_regions_per_tier,
        spec.islands.coarseness,
        spec.cusp_height_mm
    );
    let cancel = AtomicBool::new(false);
    let map_start = Instant::now();
    let preview = session
        .preview_multitool_plan(&spec, &cancel)
        .expect("preview_multitool_plan");
    let map_s = map_start.elapsed().as_secs_f64();
    eprintln!(
        "tier map + islands: {} x {} cells @ {} mm, {} tiers, {:.1} s",
        preview.map.grid.nx,
        preview.map.grid.ny,
        preview.map.grid.cell_mm,
        preview.map.tier_count,
        map_s
    );
    for (k, (name, r)) in preview
        .tool_names
        .iter()
        .zip(&preview.cusp_radii_mm)
        .enumerate()
    {
        eprintln!("  tier {k}: {name} (cusp radius {r:.3} mm)");
    }

    // -- per island ----------------------------------------------------------
    let bridge_start = Instant::now();
    let mut lines: Vec<String> = Vec::new();
    let mut table: Vec<TierRow> = Vec::new();
    let z_at = |x: f64, y: f64| finest_z_at(&preview.map, x, y);
    for set in &preview.islands.per_tier {
        let tier = set.tier as usize;
        let cusp_r = preview.cusp_radii_mm[tier];
        let name = preview.tool_names[tier].clone();
        let stepover = equal_cusp_stepover_mm(cusp_r, h);
        let owned = set.owned.as_slice();
        if owned.len() != set.islands {
            eprintln!(
                "  NOTE tier {tier}: owned polygons {} != island count {} (see TierIslandSet::islands)",
                owned.len(),
                set.islands
            );
        }
        let mut counts: BTreeMap<&'static str, usize> = BTreeMap::new();
        let mut ok = 0usize;
        for (i, polygon) in owned.iter().enumerate() {
            let (levels, text, key) = run_island(polygon, cusp_r, h, &z_at);
            *counts.entry(key).or_default() += 1;
            if key == "Ok" {
                ok += 1;
            }
            let largest_hole = polygon
                .holes
                .iter()
                .map(|hole| Polygon2::new(hole.clone()).area())
                .fold(0.0f64, f64::max);
            lines.push(format!(
                "| {tier} | {i} | {:.1} | {} | {largest_hole:.2} | {levels} | {text} |",
                polygon.area(),
                polygon.holes.len()
            ));
        }
        eprintln!(
            "  tier {tier} {name}: stepover {stepover:.4} mm, {} owned polygons, {:.0} mm2, \
             raw islands {}, close radius {:.3} mm, cap acted {}",
            owned.len(),
            set.owned_area_mm2,
            set.raw_island_count,
            set.close_radius_mm,
            set.cap.acted()
        );
        table.push(TierRow {
            tier: set.tier,
            name,
            polys: owned.len(),
            ok,
            area: set.owned_area_mm2,
            islands: set.islands,
            counts,
        });
    }
    let bridge_s = bridge_start.elapsed().as_secs_f64();

    eprintln!("\n=== PER TIER ===");
    eprintln!(
        "| tier | tool | islands (set.islands) | owned polygons | area mm2 | Ok | refusals by variant |"
    );
    eprintln!("|---|---|---|---|---|---|---|");
    let (mut total, mut total_ok) = (0usize, 0usize);
    for TierRow {
        tier,
        name,
        polys,
        ok,
        area,
        islands,
        counts,
    } in &table
    {
        let refusals: Vec<String> = counts
            .iter()
            .filter(|(k, _)| **k != "Ok")
            .map(|(k, v)| format!("{k} {v}"))
            .collect();
        eprintln!(
            "| {tier} | {name} | {islands} | {polys} | {area:.0} | {ok} | {} |",
            if refusals.is_empty() {
                "none".to_owned()
            } else {
                refusals.join(", ")
            }
        );
        total += polys;
        total_ok += ok;
    }
    eprintln!(
        "TOTAL: {total_ok} Ok of {total} owned polygons; {} refused",
        total - total_ok
    );

    eprintln!("\n=== PER ISLAND ===");
    eprintln!("| tier | island | area mm2 | holes | largest hole mm2 | levels | result |");
    eprintln!("|---|---|---|---|---|---|---|");
    for line in &lines {
        eprintln!("{line}");
    }
    eprintln!(
        "\nwall clock: tier map {map_s:.1} s, level sets + bridge {bridge_s:.1} s, run total {:.1} s",
        run_start.elapsed().as_secs_f64()
    );
}
