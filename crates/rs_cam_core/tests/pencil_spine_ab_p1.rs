//! P1 — the pencil watershed-spine A/B extraction instrument.
//!
//! Charter + pre-registered bars: `planning/pencil_watershed_spine_2026-09-03/`.
//! The claim under test: **D8 flow-accumulation on the rest field's terrain
//! extracts better pencil centrelines than the current NMS+hysteresis+
//! Zhang-Suen pipeline in `rest_depth_arm`** — more coherent, no coverage
//! loss, no seam loss, no higher cost, spine no further from the valley bottom.
//!
//! # Two extractors, ONE rest field (X3)
//!
//! The rest field is built ONCE by [`rest_field::detect_rest_valleys`].
//! Extractor A is that call's own output (`result.centerlines` / `report`).
//! Extractor B reads `result.rest_grid` and runs the promoted D8 hydrology
//! (`crate::flow_accum`) on its `surface_z` — the real topography, which has
//! outlets — NOT on the rest field, which is a closed basin (amendment A1 in
//! `FINDINGS.md`). The pencil criterion is preserved by rest-GATING the
//! trunks.
//!
//! # Why the terrain, not the rest field (A1, with the falsifier baked in)
//!
//! [`priority_flood_on_negated_rest_is_flat`] is the standing falsifier: it
//! shows a rest bump on a zero rim fills flat under the priority flood, so
//! `-rest` cannot carry a routable gradient. If that test ever goes green the
//! wrong way, amendment A1 is wrong and the whole B construction must be
//! reconsidered before any ruling.
//!
//! # `#[ignore]`
//!
//! The wanaka A/B needs the operator's mesh (not in the repo); the synthetic
//! A/B is fast but is an evidence run, not a sentry, so it carries `#[ignore]`
//! too. The toy falsifier is a real gate and runs by default.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use rs_cam_core::flow_accum::{
    FILL_EPSILON_MM, FlowField, d8_accumulation, d8_receivers, priority_flood_epsilon,
    resolve_flats,
};
use rs_cam_core::geo::P3;
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::rest_field::{
    RestFieldParams, RestFieldResult, RestGrid, RestReference, detect_rest_valleys,
};
use rs_cam_core::tool::{BallEndmill, MillingCutter};

mod common;
use common::meshes::height_field;
use common::tools::wanaka_taper;

/// The operator's wanaka terrain export (same path the Track H census uses).
const WANAKA_MESH: &str = "/home/ricky/Downloads/wanaka200/rivmap_export/terrain.stl";

// ═══════════════════════════════════════════════════════════════════════
// Extractor B — flow accumulation on the terrain, rest-gated (amendment A1)
// ═══════════════════════════════════════════════════════════════════════

/// A traced spine: world-space polyline draped on the surface, with the rest
/// depth read at each point (for M4).
struct Spine {
    points: Vec<P3>,
    /// Rest depth (mm) at each point, one per [`Self::points`].
    rest_at: Vec<f64>,
}

impl Spine {
    fn length_mm(&self) -> f64 {
        self.points.windows(2).map(|w| (w[1] - w[0]).norm()).sum()
    }
}

/// Build a [`FlowField`] from the rest grid's SURFACE topography. `nodata`
/// where the drop found no surface (NaN), and — when `rest_floor` is set
/// (amendment A4) — where `rest < rest_floor`, so the REST MASK is the routing
/// domain and each rest valley drains to its own rim.
///
/// Without A4, an arbitrary part surface is a CLOSED BASIN (a raised machining
/// rim, or just bounded stock), so the priority flood fills the whole board
/// and the flat-resolver ramps it into cardinal parallel lines — a flood
/// artifact, not drainage. Masking to the rest mask supplies the outlets the
/// part geometry does not.
fn flow_field_from_surface(grid: &RestGrid, rest_floor: Option<f64>) -> FlowField {
    let n = grid.nx * grid.ny;
    let mut z = vec![0.0f64; n];
    let mut nodata = vec![false; n];
    for i in 0..n {
        let s = grid.surface_z[i];
        if !s.is_finite() {
            nodata[i] = true;
            continue;
        }
        z[i] = s as f64;
        if let Some(floor) = rest_floor
            && (grid.rest[i] as f64) < floor
        {
            nodata[i] = true;
        }
    }
    FlowField {
        nx: grid.nx,
        ny: grid.ny,
        ox: grid.origin_x,
        oy: grid.origin_y,
        cell: grid.cell_mm,
        z,
        nodata,
    }
}

/// Extractor B. Run D8 flow accumulation on the terrain, keep cells whose
/// upstream area clears `trunk_area_mm2` AND whose rest clears the hysteresis
/// LO floor (`0.5 × threshold`, matching A's reach), then trace the kept
/// forest by FOLLOWING RECEIVERS into main-stem-first polylines.
///
/// Returns `(spines, skeleton_length_mm)`, where `skeleton_length_mm` is the
/// total traced length BEFORE the `min_cut_length` filter (the coverage
/// denominator, mirroring `RestFieldReport`).
fn extractor_b(grid: &RestGrid, trunk_area_mm2: f64, min_cut_length_mm: f64) -> (Vec<Spine>, f64) {
    let lo_floor = 0.5 * grid.threshold;
    // A4: route inside the rest mask so each valley drains to its own rim.
    let field = flow_field_from_surface(grid, Some(lo_floor));
    let n = field.len();
    let cell_area = grid.cell_mm * grid.cell_mm;

    let raw_filled = priority_flood_epsilon(&field);
    let (filled, _flats, _fc, _mi) = resolve_flats(&field, &raw_filled);
    let receivers = d8_receivers(&field, &filled);
    let acc = d8_accumulation(&field, &filled, &receivers);

    // Kept = drains enough AND holds enough rest for the pencil to reach.
    let kept: Vec<bool> = (0..n)
        .map(|i| {
            !field.nodata[i]
                && acc[i] * cell_area >= trunk_area_mm2
                && (grid.rest[i] as f64) >= lo_floor
        })
        .collect();

    // Kept-donor count per cell: a cell is a source when nothing kept flows
    // into it.
    let mut kept_donors = vec![0u32; n];
    for i in 0..n {
        if !kept[i] {
            continue;
        }
        if let Some(r) = receivers[i]
            && kept[r as usize]
        {
            kept_donors[r as usize] += 1;
        }
    }
    // Sources, largest upstream area first, so the main stem is traced (and
    // claims the trunk through junctions) before its tributaries.
    let mut sources: Vec<usize> = (0..n).filter(|&i| kept[i] && kept_donors[i] == 0).collect();
    sources.sort_by(|&a, &b| acc[b].total_cmp(&acc[a]));

    let world = |i: usize| -> P3 {
        let (r, c) = (i / field.nx, i % field.nx);
        P3::new(
            field.ox + c as f64 * field.cell,
            field.oy + r as f64 * field.cell,
            grid.surface_z[i] as f64,
        )
    };

    let mut claimed = vec![u32::MAX; n];
    let mut spines: Vec<Spine> = Vec::new();
    for &src in &sources {
        let id = spines.len() as u32;
        let mut pts: Vec<P3> = Vec::new();
        let mut rest_at: Vec<f64> = Vec::new();
        let mut cur = src;
        loop {
            pts.push(world(cur));
            rest_at.push(grid.rest[cur] as f64);
            if claimed[cur] == u32::MAX {
                claimed[cur] = id;
            }
            let Some(r) = receivers[cur] else { break };
            let nxt = r as usize;
            if !kept[nxt] {
                break;
            }
            if claimed[nxt] != u32::MAX {
                // Reached an already-claimed trunk: add the junction point so
                // the tributary visibly meets it, then stop.
                pts.push(world(nxt));
                rest_at.push(grid.rest[nxt] as f64);
                break;
            }
            cur = nxt;
        }
        if pts.len() >= 2 {
            spines.push(Spine {
                points: pts,
                rest_at,
            });
        }
    }
    let skeleton_length_mm = spines.iter().map(Spine::length_mm).sum();
    // Coverage filter: drop spines under min_cut_length (mirrors A).
    spines.retain(|s| s.length_mm() >= min_cut_length_mm);
    (spines, skeleton_length_mm)
}

// ═══════════════════════════════════════════════════════════════════════
// Metrics (M1–M4, M6) — both extractors read the SAME rest field
// ═══════════════════════════════════════════════════════════════════════

/// A uniform readout of one extractor's spine set for the pre-registered bars.
struct ExtractorReport {
    label: String,
    /// M2: kept polyline count.
    polyline_count: usize,
    /// M2: median kept polyline length (mm).
    median_len_mm: f64,
    /// M1/M3: traced length (survivors) and the coverage ratio traced/skeleton.
    traced_len_mm: f64,
    coverage: f64,
    /// M4: median (spine rest depth ÷ local cross-section MAX rest depth).
    valley_bottom_fidelity: f64,
    /// The world-space polylines, for M6 and rendering.
    polylines: Vec<Vec<P3>>,
}

fn median(mut v: Vec<f64>) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

/// M4: for a spine point at cell `i`, the local cross-section MAX rest depth is
/// the largest `rest` over a disc of `radius_cells` around `i` (the reachable
/// neighbourhood a ball could nestle into). Fidelity = rest_at_point ÷ that
/// max; 1.0 = the spine sits at the deepest reachable point.
fn local_cross_section_max(grid: &RestGrid, cx: f64, cy: f64, radius_cells: i64) -> f64 {
    let col = ((cx - grid.origin_x) / grid.cell_mm).round() as i64;
    let row = ((cy - grid.origin_y) / grid.cell_mm).round() as i64;
    let mut m = 0.0f64;
    for dr in -radius_cells..=radius_cells {
        for dc in -radius_cells..=radius_cells {
            if dr * dr + dc * dc > radius_cells * radius_cells {
                continue;
            }
            let (r, c) = (row + dr, col + dc);
            if r < 0 || c < 0 || r >= grid.ny as i64 || c >= grid.nx as i64 {
                continue;
            }
            let v = grid.rest[(r as usize) * grid.nx + c as usize] as f64;
            if v.is_finite() && v > m {
                m = v;
            }
        }
    }
    m
}

fn report_from_polylines(
    label: &str,
    grid: &RestGrid,
    polylines: Vec<Vec<P3>>,
    rest_at: &[Vec<f64>],
    skeleton_len_mm: f64,
    min_cut_length_mm: f64,
    pencil_diam_mm: f64,
) -> ExtractorReport {
    let plen = |p: &[P3]| -> f64 { p.windows(2).map(|w| (w[1] - w[0]).norm()).sum() };
    let lens: Vec<f64> = polylines.iter().map(|p| plen(p)).collect();
    let traced_len_mm: f64 = lens.iter().filter(|&&l| l >= min_cut_length_mm).sum();
    let coverage = if skeleton_len_mm <= 1e-9 {
        0.0
    } else {
        traced_len_mm / skeleton_len_mm
    };
    // M4: fidelity over a uniform sample of all spine points.
    let radius_cells = ((pencil_diam_mm * 0.5) / grid.cell_mm).ceil().max(1.0) as i64;
    let mut fid: Vec<f64> = Vec::new();
    for (pl, ra) in polylines.iter().zip(rest_at.iter()) {
        for (p, &r) in pl.iter().zip(ra.iter()) {
            let m = local_cross_section_max(grid, p.x, p.y, radius_cells);
            if m > 1e-6 {
                fid.push((r / m).min(1.0));
            }
        }
    }
    ExtractorReport {
        label: label.to_owned(),
        polyline_count: polylines.len(),
        median_len_mm: median(lens),
        traced_len_mm,
        coverage,
        valley_bottom_fidelity: median(fid),
        polylines,
    }
}

/// Extractor A: the shipped detector's own output, read into the shared shape.
fn report_extractor_a(
    result: &RestFieldResult,
    grid: &RestGrid,
    min_cut_length_mm: f64,
    pencil_diam_mm: f64,
) -> ExtractorReport {
    let polylines: Vec<Vec<P3>> = result
        .centerlines
        .iter()
        .map(|c| c.points.clone())
        .collect();
    // Read rest at each A point off the SAME grid (shared field, A3).
    let rest_at: Vec<Vec<f64>> = polylines
        .iter()
        .map(|pl| pl.iter().map(|p| sample_rest(grid, p.x, p.y)).collect())
        .collect();
    let skeleton = result.report.skeleton_length_mm;
    let mut rep = report_from_polylines(
        "A — NMS ridge (shipped)",
        grid,
        polylines,
        &rest_at,
        skeleton,
        min_cut_length_mm,
        pencil_diam_mm,
    );
    // Prefer the detector's own coverage number where it published one.
    rep.coverage = result.report.coverage();
    rep.traced_len_mm = result.report.traced_length_mm;
    rep
}

fn sample_rest(grid: &RestGrid, x: f64, y: f64) -> f64 {
    let col = ((x - grid.origin_x) / grid.cell_mm).round() as i64;
    let row = ((y - grid.origin_y) / grid.cell_mm).round() as i64;
    if row < 0 || col < 0 || row >= grid.ny as i64 || col >= grid.nx as i64 {
        return 0.0;
    }
    let v = grid.rest[(row as usize) * grid.nx + col as usize] as f64;
    if v.is_finite() { v } else { 0.0 }
}

/// M6: the fraction of A's covered footprint that B also covers, and the
/// reverse. Footprint = cells within `half_width_cells` of any spine point.
fn footprint_overlap(
    grid: &RestGrid,
    a: &ExtractorReport,
    b: &ExtractorReport,
    half_width_mm: f64,
) -> (f64, f64) {
    let stamp = |rep: &ExtractorReport| -> Vec<bool> {
        let mut m = vec![false; grid.nx * grid.ny];
        let rc = (half_width_mm / grid.cell_mm).ceil().max(1.0) as i64;
        for pl in &rep.polylines {
            for p in pl {
                let col = ((p.x - grid.origin_x) / grid.cell_mm).round() as i64;
                let row = ((p.y - grid.origin_y) / grid.cell_mm).round() as i64;
                for dr in -rc..=rc {
                    for dc in -rc..=rc {
                        if dr * dr + dc * dc > rc * rc {
                            continue;
                        }
                        let (r, c) = (row + dr, col + dc);
                        if r < 0 || c < 0 || r >= grid.ny as i64 || c >= grid.nx as i64 {
                            continue;
                        }
                        m[(r as usize) * grid.nx + c as usize] = true;
                    }
                }
            }
        }
        m
    };
    let am = stamp(a);
    let bm = stamp(b);
    let a_cells = am.iter().filter(|&&x| x).count();
    let b_cells = bm.iter().filter(|&&x| x).count();
    let both = (0..am.len()).filter(|&i| am[i] && bm[i]).count();
    let b_covers_a = if a_cells == 0 {
        1.0
    } else {
        both as f64 / a_cells as f64
    };
    let a_covers_b = if b_cells == 0 {
        1.0
    } else {
        both as f64 / b_cells as f64
    };
    (b_covers_a, a_covers_b)
}

// ═══════════════════════════════════════════════════════════════════════
// Rendering (X2 — render before ruling)
// ═══════════════════════════════════════════════════════════════════════

/// A hillshade of `surface_z` with A's spines (blue) and B's spines (red)
/// overlaid. One PNG-free SVG so it needs no image crate.
fn render_overlay(path: &Path, grid: &RestGrid, a: &ExtractorReport, b: &ExtractorReport) {
    let (nx, ny) = (grid.nx, grid.ny);
    let scale = (900.0 / nx.max(ny) as f64).max(1.0);
    let w = nx as f64 * scale;
    let h = ny as f64 * scale;
    // Rest-shaded background: deeper rest = darker warm.
    let mut max_rest = 1e-6;
    for &r in &grid.rest {
        if r.is_finite() && (r as f64) > max_rest {
            max_rest = r as f64;
        }
    }
    let mut svg = String::new();
    let _ = write!(
        svg,
        "<svg xmlns='http://www.w3.org/2000/svg' width='{w:.0}' height='{h:.0}' \
         viewBox='0 0 {w:.0} {h:.0}'>\n<rect width='{w:.0}' height='{h:.0}' fill='#101418'/>\n"
    );
    // Rest field as faint cells (only where finite & > 0).
    for r in 0..ny {
        for c in 0..nx {
            let v = grid.rest[r * nx + c] as f64;
            if !v.is_finite() || v <= 0.0 {
                continue;
            }
            let t = (v / max_rest).clamp(0.0, 1.0);
            let g = (40.0 + 90.0 * t) as u32;
            let _ = writeln!(
                svg,
                "<rect x='{:.1}' y='{:.1}' width='{:.1}' height='{:.1}' fill='#{:02x}{:02x}30'/>",
                c as f64 * scale,
                (ny - 1 - r) as f64 * scale,
                scale + 0.5,
                scale + 0.5,
                g,
                g / 2,
            );
        }
    }
    let draw = |svg: &mut String, rep: &ExtractorReport, color: &str, wdt: f64| {
        for pl in &rep.polylines {
            if pl.len() < 2 {
                continue;
            }
            let mut d = String::from("M");
            for (k, p) in pl.iter().enumerate() {
                let cx = (p.x - grid.origin_x) / grid.cell_mm * scale;
                let cy = (ny as f64 - 1.0 - (p.y - grid.origin_y) / grid.cell_mm) * scale;
                let _ = write!(d, "{}{:.1} {:.1} ", if k == 0 { "" } else { "L" }, cx, cy);
            }
            let _ = writeln!(
                svg,
                "<path d='{d}' fill='none' stroke='{color}' stroke-width='{wdt}' \
                 stroke-opacity='0.85'/>"
            );
        }
    };
    draw(&mut svg, a, "#4da6ff", 1.4);
    draw(&mut svg, b, "#ff5a5a", 1.4);
    let _ = write!(
        svg,
        "<text x='8' y='16' fill='#4da6ff' font-family='monospace' font-size='13'>A NMS: \
         {} lines</text>\n<text x='8' y='32' fill='#ff5a5a' font-family='monospace' \
         font-size='13'>B flow-accum: {} lines</text>\n</svg>\n",
        a.polyline_count, b.polyline_count
    );
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(path, svg).expect("write overlay svg");
}

fn out_dir() -> PathBuf {
    let d = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/pencil_spine_ab_p1");
    let _ = std::fs::create_dir_all(&d);
    d
}

// ═══════════════════════════════════════════════════════════════════════
// A/B driver — shared by the synthetic and wanaka fixtures
// ═══════════════════════════════════════════════════════════════════════

/// Receiver-field diagnostics for extractor B.
///
/// `raised_frac_unmasked` is Track H's own flood test (`filled > z + eps`) on
/// the RAW surface — if it is ~1.0 the whole board floods into a closed basin
/// and the flat-resolver ramps it into cardinal lines (the bug the operator
/// caught). `raised_frac_masked` is the same on the A4 rest-masked surface —
/// it should be far lower, because each rest valley now drains to its own rim.
/// `recv_hist` is a 3×3 receiver-direction histogram over the kept set (index
/// `(dr+1)*3 + (dc+1)`, slot 4 = self); one dominant off-centre bin means the
/// flow is a single-direction ramp, not real drainage.
struct BDiag {
    kept_cells: usize,
    outlets: usize,
    acc_p50_mm2: f64,
    acc_p95_mm2: f64,
    raised_frac_unmasked: f64,
    raised_frac_masked: f64,
    recv_hist: [usize; 9],
}

/// Fraction of data cells the priority flood RAISED above their own height —
/// the closed-basin flood signature.
fn raised_fraction(field: &FlowField) -> f64 {
    let filled = priority_flood_epsilon(field);
    let mut data = 0usize;
    let mut raised = 0usize;
    for ((&f, &z), &nd) in filled.iter().zip(&field.z).zip(&field.nodata) {
        if nd {
            continue;
        }
        data += 1;
        if f > z + FILL_EPSILON_MM {
            raised += 1;
        }
    }
    if data == 0 {
        0.0
    } else {
        raised as f64 / data as f64
    }
}

fn flow_diag(grid: &RestGrid, trunk_area_mm2: f64) -> BDiag {
    let lo_floor = 0.5 * grid.threshold;
    let raised_frac_unmasked = raised_fraction(&flow_field_from_surface(grid, None));

    // The A4 masked pipeline — the one extractor B actually routes on.
    let field = flow_field_from_surface(grid, Some(lo_floor));
    let raised_frac_masked = raised_fraction(&field);
    let n = field.len();
    let cell_area = grid.cell_mm * grid.cell_mm;
    let raw = priority_flood_epsilon(&field);
    let (filled, ..) = resolve_flats(&field, &raw);
    let receivers = d8_receivers(&field, &filled);
    let acc = d8_accumulation(&field, &filled, &receivers);
    let kept: Vec<bool> = (0..n)
        .map(|i| !field.nodata[i] && acc[i] * cell_area >= trunk_area_mm2)
        .collect();
    let kept_cells = kept.iter().filter(|&&k| k).count();
    let mut outlet_set = std::collections::BTreeSet::new();
    let mut recv_hist = [0usize; 9];
    for i in 0..n {
        if !kept[i] {
            continue;
        }
        match receivers[i] {
            Some(r) if kept[r as usize] => {
                let (ri, ci) = (i / field.nx, i % field.nx);
                let (rr, cr) = (r as usize / field.nx, r as usize % field.nx);
                let dr = (rr as i64 - ri as i64).signum() + 1;
                let dc = (cr as i64 - ci as i64).signum() + 1;
                recv_hist[(dr * 3 + dc) as usize] += 1;
            }
            Some(r) => {
                outlet_set.insert(r as usize);
            }
            None => {
                outlet_set.insert(i);
            }
        }
    }
    let mut mask_acc: Vec<f64> = (0..n)
        .filter(|&i| !field.nodata[i])
        .map(|i| acc[i] * cell_area)
        .collect();
    mask_acc.sort_by(f64::total_cmp);
    let pick = |q: f64| {
        if mask_acc.is_empty() {
            0.0
        } else {
            mask_acc[((mask_acc.len() - 1) as f64 * q) as usize]
        }
    };
    BDiag {
        kept_cells,
        outlets: outlet_set.len(),
        acc_p50_mm2: pick(0.5),
        acc_p95_mm2: pick(0.95),
        raised_frac_unmasked,
        raised_frac_masked,
        recv_hist,
    }
}

/// M3-alt: fraction of a KNOWN centreline (sampled XY points) within `radius`
/// of any traced point — the ground-truth recall B3 reads on the synthetic.
/// This is what M6-against-A cannot say while A is itself a hairball.
fn centerline_recall(polylines: &[Vec<P3>], truth_xy: &[(f64, f64)], radius: f64) -> f64 {
    if truth_xy.is_empty() {
        return 0.0;
    }
    let hit = truth_xy
        .iter()
        .filter(|&&(tx, ty)| {
            polylines
                .iter()
                .flatten()
                .any(|p| (p.x - tx).hypot(p.y - ty) <= radius)
        })
        .count();
    hit as f64 / truth_xy.len() as f64
}

fn run_ab(fixture: &str, mesh: &TriangleMesh, cell_mm: f64, truth_xy: Option<&[(f64, f64)]>) {
    let index = SpatialIndex::build(mesh, 5.0);
    let pencil = wanaka_taper();
    // The CUTTING diameter is the tip CUSP, not the shank (the CLAUDE.md
    // radius memo). T_disc, the M6 stamp and recall all read the tip.
    let cut_radius = pencil.cusp_radius();
    let cut_diam = cut_radius * 2.0;
    // A Ø12 ball reference, as the rest-detector probes use: it cannot reach
    // the grooves, so the groove IS the rest field.
    let reference = BallEndmill::new(12.0, 25.0);
    let params = RestFieldParams {
        cell_mm,
        min_valley_depth: 0.05,
        offset_stepover_mm: 0.25,
        num_offset_passes_cap: 8,
        min_cut_length: 2.0,
        region_margin_mm: 0.5,
    };
    let result = detect_rest_valleys(
        mesh,
        &index,
        &pencil,
        RestReference::Cutter {
            tool: &reference as &dyn MillingCutter,
            is_surface_probe: false,
        },
        &params,
    );
    let grid = &result.rest_grid;

    // A2: T = area of a disc one pencil (tip) DIAMETER across, plus the
    // sensitivity sweep {0.5T, T, 2T}.
    let t_disc = std::f64::consts::PI * cut_radius.powi(2);
    let a = report_extractor_a(&result, grid, params.min_cut_length, cut_diam);

    eprintln!("\n══════ {fixture}: pencil-spine A/B (cell {cell_mm} mm) ══════");
    eprintln!(
        "  rest grid {}x{}  threshold {:.3} mm  pencil TIP Ø{:.2} (shank Ø{:.2})  T_disc {:.2} mm²",
        grid.nx,
        grid.ny,
        grid.threshold,
        cut_diam,
        pencil.radius() * 2.0,
        t_disc
    );
    eprintln!(
        "  {:32}  lines  median_len  traced_mm  coverage  valley_fid    recall",
        "extractor"
    );
    let recall_of =
        |rep: &ExtractorReport| truth_xy.map(|t| centerline_recall(&rep.polylines, t, cut_radius));
    let row = |r: &ExtractorReport| {
        let rc = recall_of(r)
            .map(|v| format!("{v:9.3}"))
            .unwrap_or_else(|| "      n/a".to_owned());
        eprintln!(
            "  {:32}  {:5}  {:9.3}  {:9.3}  {:8.3}  {:9.3}  {rc}",
            r.label,
            r.polyline_count,
            r.median_len_mm,
            r.traced_len_mm,
            r.coverage,
            r.valley_bottom_fidelity,
        );
    };
    row(&a);

    for (tag, t) in [("0.5T", 0.5 * t_disc), ("T", t_disc), ("2T", 2.0 * t_disc)] {
        let (spines, skel) = extractor_b(grid, t, params.min_cut_length);
        let polylines: Vec<Vec<P3>> = spines.iter().map(|s| s.points.clone()).collect();
        let rest_at: Vec<Vec<f64>> = spines.iter().map(|s| s.rest_at.clone()).collect();
        let b = report_from_polylines(
            &format!("B — flow-accum @ {tag}"),
            grid,
            polylines,
            &rest_at,
            skel,
            params.min_cut_length,
            cut_diam,
        );
        row(&b);
        if tag == "T" {
            let d = flow_diag(grid, t);
            eprintln!(
                "    B diag @ T: kept {} cells, {} distinct outlets; mask acc p50 {:.1} p95 {:.1} mm² (T={:.2})",
                d.kept_cells, d.outlets, d.acc_p50_mm2, d.acc_p95_mm2, t
            );
            eprintln!(
                "    FLOOD test: priority-flood RAISED {:.1}% of the raw surface (closed-basin flood), \
                 {:.1}% of the A4 rest-masked surface",
                100.0 * d.raised_frac_unmasked,
                100.0 * d.raised_frac_masked
            );
            eprintln!(
                "    recv dir 3x3 [NW N NE / W . E / SW S SE]: {:?} (one dominant off-centre bin = ramp, not drainage)",
                d.recv_hist
            );
            let (b_cov_a, a_cov_b) = footprint_overlap(grid, &a, &b, cut_radius);
            eprintln!(
                "    M6 seam overlap @ T: B covers {:.3} of A's footprint; A covers {:.3} of B's",
                b_cov_a, a_cov_b
            );
            render_overlay(
                &out_dir().join(format!("{fixture}_overlay.svg")),
                grid,
                &a,
                &b,
            );
            eprintln!("    render: target/pencil_spine_ab_p1/{fixture}_overlay.svg");
        }
    }
    eprintln!("  (no verdict written here — bars are applied in FINDINGS)");
}

// ═══════════════════════════════════════════════════════════════════════
// A1 falsifier — the rest field is a closed basin; -rest fills flat
// ═══════════════════════════════════════════════════════════════════════

/// A 1-D-style rest bump on a zero rim, embedded in a 2-D grid. On `-rest`
/// (bump becomes a pit), the priority flood fills the pit flat, so no cell in
/// the bump keeps a strictly-lower neighbour toward a spine — the D8 receiver
/// field inside the bump collapses to the epsilon gradient, not the ridge.
/// This is why extractor B runs on the terrain, not on `-rest` (amendment A1).
#[test]
fn priority_flood_on_negated_rest_is_flat() {
    let (nx, ny) = (41usize, 41usize);
    let mut rest = vec![0.0f64; nx * ny];
    // An ISOLATED 2-D rest dome centred in the grid: rest peaks at the centre
    // and falls to 0 (`rim`) well inside the border in every direction. On
    // `-rest` this is a closed bowl with no border outlet — the case A1 is
    // about (a spanning ridge would instead drain off the grid edges and would
    // NOT fill flat, which is a different topology).
    for r in 0..ny {
        for c in 0..nx {
            let d = (((c as f64 - 20.0).powi(2)) + ((r as f64 - 20.0).powi(2))).sqrt();
            rest[r * nx + c] = (2.0 - 0.15 * d).max(0.0);
        }
    }
    // Terrain door (correct): negate is NOT how B works, this is the toy.
    let neg = FlowField {
        nx,
        ny,
        ox: 0.0,
        oy: 0.0,
        cell: 1.0,
        z: rest.iter().map(|&v| -v).collect(),
        nodata: vec![false; nx * ny],
    };
    let filled = priority_flood_epsilon(&neg);
    // The interior of the bowl (the pit floor on -rest) fills to a single flat
    // level within the epsilon budget: max−min over interior cells is tiny.
    let interior: Vec<f64> = (0..ny)
        .flat_map(|r| (0..nx).map(move |c| (r, c)))
        .filter(|&(r, c)| (((c as f64 - 20.0).powi(2)) + ((r as f64 - 20.0).powi(2))).sqrt() < 8.0)
        .map(|(r, c)| filled[r * nx + c])
        .collect();
    let lo = interior.iter().cloned().fold(f64::INFINITY, f64::min);
    let hi = interior.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let spread = hi - lo;
    eprintln!("A1 toy: -rest pit interior filled spread = {spread:.6} mm (epsilon-flat)");
    assert!(
        spread < 1.0e-3,
        "the -rest pit did NOT fill flat (spread {spread:.6} mm ≥ 1e-3): amendment A1's \
         closed-basin argument is falsified — reconsider extractor B before ruling"
    );
}

// ═══════════════════════════════════════════════════════════════════════
// Synthetic Y-valley — the ground-truth connectivity fixture (crux)
// ═══════════════════════════════════════════════════════════════════════

/// A block carved with a Y-shaped groove (trunk + two tributaries) plus a
/// broad shallow basin. The reference Ø12 ball reaches the basin (rest ≈ 0,
/// MUST stay untraced) but not the groove. The Y-junction is the crux:
/// extractor A is expected to shred at the degree-3 node; B to keep the trunk
/// continuous through it (`FINDINGS.md` synthetic fixture).
/// The trunk's downstream endpoint. Flat-closed: `-30` (10 mm short of the
/// board edge, so the groove is a closed trench — the DEGENERATE case).
/// Sloped-exit: `-40` (on the board edge, so flow has a real outlet).
fn trunk_end(sloped_exit: bool) -> f64 {
    if sloped_exit { -40.0 } else { -30.0 }
}

fn y_valley_mesh(sloped_exit: bool) -> TriangleMesh {
    let half = 40.0;
    let depth = 3.0;
    let groove_half = 0.8;
    let te = trunk_end(sloped_exit);
    // A gentle along-valley slope for the sloped case: the whole surface tilts
    // so +y is uphill and the outlet at y = -40 is the low point (~1.3 mm over
    // the board). A real rest valley slopes; the flat case is degenerate.
    let tilt = if sloped_exit { 0.02 } else { 0.0 };
    let seg = |p: (f64, f64), a: (f64, f64), b: (f64, f64)| -> f64 {
        let (ax, ay) = a;
        let (bx, by) = b;
        let (px, py) = p;
        let (dx, dy) = (bx - ax, by - ay);
        let len2 = dx * dx + dy * dy;
        let t = if len2 <= 1e-9 {
            0.0
        } else {
            (((px - ax) * dx + (py - ay) * dy) / len2).clamp(0.0, 1.0)
        };
        let (qx, qy) = (ax + t * dx, ay + t * dy);
        ((px - qx).powi(2) + (py - qy).powi(2)).sqrt()
    };
    height_field(half, 0.4, move |x, y| {
        let d = seg((x, y), (0.0, te), (0.0, 0.0))
            .min(seg((x, y), (0.0, 0.0), (-15.0, 25.0)))
            .min(seg((x, y), (0.0, 0.0), (15.0, 25.0)));
        let groove = if d < groove_half {
            -depth * (1.0 - d / groove_half)
        } else {
            0.0
        };
        // A broad shallow basin in the +X,-Y quadrant, ~0.3 mm deep — the Ø12
        // ball reaches it, so rest ≈ 0 there (MUST stay untraced).
        let basin = if x > 12.0 && y < -8.0 { -0.3 } else { 0.0 };
        tilt * (y + 40.0) + groove + basin
    })
}

/// The KNOWN Y centreline sampled every 0.5 mm (XY), for `centerline_recall`.
fn y_truth_xy(sloped_exit: bool) -> Vec<(f64, f64)> {
    let te = trunk_end(sloped_exit);
    let arms: [((f64, f64), (f64, f64)); 3] = [
        ((0.0, te), (0.0, 0.0)),
        ((0.0, 0.0), (-15.0, 25.0)),
        ((0.0, 0.0), (15.0, 25.0)),
    ];
    let mut pts = Vec::new();
    for &((ax, ay), (bx, by)) in &arms {
        let len = ((bx - ax).powi(2) + (by - ay).powi(2)).sqrt();
        let n = (len / 0.5).ceil().max(1.0) as usize;
        for k in 0..=n {
            let t = k as f64 / n as f64;
            pts.push((ax + t * (bx - ax), ay + t * (by - ay)));
        }
    }
    pts
}

#[test]
#[ignore = "evidence run — fast synthetic A/B, not a sentry"]
fn synthetic_flat_closed_pencil_spine_ab() {
    // The DEGENERATE case: a flat-floored closed groove. Recorded as a bounded
    // finding, not the ruling — flow accumulation has no along-valley gradient
    // here.
    let mesh = y_valley_mesh(false);
    run_ab("synthetic_flat", &mesh, 0.25, Some(&y_truth_xy(false)));
}

#[test]
#[ignore = "evidence run — fast synthetic A/B, not a sentry"]
fn synthetic_sloped_exit_pencil_spine_ab() {
    // The FAIR case for the operator's claim: a sloping valley that exits the
    // board edge, so flow accumulation has a real gradient and outlet.
    let mesh = y_valley_mesh(true);
    run_ab("synthetic_sloped", &mesh, 0.25, Some(&y_truth_xy(true)));
}

#[test]
#[ignore = "evidence run — needs the operator's wanaka mesh (not in repo)"]
fn wanaka_pencil_spine_ab() {
    let path = Path::new(WANAKA_MESH);
    if !path.exists() {
        eprintln!("SKIP wanaka A/B: mesh not present at {WANAKA_MESH}");
        return;
    }
    let mesh = TriangleMesh::from_stl_scaled(path, 1.0).expect("load wanaka terrain");
    run_ab("wanaka", &mesh, 0.5, None);
}
