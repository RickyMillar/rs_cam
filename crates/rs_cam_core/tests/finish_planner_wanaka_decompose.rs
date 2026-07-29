//! P2.b R1 acceptance: the unified-finish decomposition must condition the
//! wanaka terrain down to O(10) planned regions, not the O(100) island storm
//! a naive threshold classification produces
//! (`planning/unified_finish_planner_design.md`, risk R1).
//!
//! Also writes the SVG debug view — the P2.b visual surface — to
//! `target/finish_planner_debug/wanaka_regions.svg` so the decomposition can
//! be inspected before anything cuts.
//!
//! `#[ignore]` — drop-cutter samples the full wanaka mesh at finish
//! resolution (~a minute). Run with:
//! `cargo test -p rs_cam_core --test finish_planner_wanaka_decompose -- --ignored --nocapture`

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr
)]

use std::path::PathBuf;

use rs_cam_core::finish_planner::{
    FinishBand, FinishPlannerParams, decompose_surface, planned_regions_to_svg,
};
use rs_cam_core::finish_setup::{
    build_classification_surface_with_cancel, build_finish_surface_with_cancel,
};
use rs_cam_core::measurement::{ProjectedXyAreaMm2, SurfaceAreaMm2};
use rs_cam_core::mesh::SpatialIndex;
use rs_cam_core::session::ProjectSession;
use rs_cam_core::tool::{BallEndmill, TaperedBallEndmill};

fn wanaka_project_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("planning")
        .join("airrun_2026-06-01")
        .join("wanaka.toml")
}

/// M1 gate at the crime scene (plan §M1 acceptance 1; `MEASUREMENT_DOMAINS.md`
/// LH-3 / §5).
///
/// This file prints an XY-projected band-area column and a 3D mesh-face-area
/// column ~30 lines apart. §14r divided one by the other and published "313 of
/// 482 mm² recovered (65%)"; the claim was retracted in `63d5e8b` and, until
/// now, the only thing stopping the next reader was a comment.
///
/// Both columns are now newtypes with **no `Div` between them**, so the
/// division is a compile error. That is proven by a `compile_fail` doctest on
/// [`rs_cam_core::measurement::ProjectedXyAreaMm2`] (run by
/// `cargo test -p rs_cam_core --doc`); the arithmetic reason lives in
/// `measurement.rs`'s `projected_area_is_not_a_share_of_surface_area`.
///
/// This test is fast, synthetic, and NOT `#[ignore]`d — it is the runnable
/// reminder in the file where the mistake was made. It asserts the two
/// measures of ONE 84° ribbon differ by ~10×, which is the magnitude that
/// made the retracted ratio wrong.
#[test]
fn projected_and_surface_areas_are_not_interchangeable_here() {
    // One near-vertical ribbon, exactly the wanaka feature class: 84° from
    // horizontal, so |normal.z| = cos(84°) ≈ 0.105.
    let cos_slope = 84.0_f64.to_radians().cos();
    let face_3d = SurfaceAreaMm2::new(482.0);
    let face_xy = face_3d.project_onto_xy(cos_slope);

    assert!(
        face_3d.mm2() / face_xy.mm2() > 9.0,
        "an 84° ribbon must project ~10× smaller ({:.1} vs {:.1} mm²) — that \
         factor is why a projected numerator over a 3D denominator reads as \
         'a third recovered' on a fully recovered surface",
        face_3d.mm2(),
        face_xy.mm2()
    );

    // The legal comparison — projected against projected — is available and
    // needs no escape hatch.
    let recovered = ProjectedXyAreaMm2::new(313.0);
    let share = recovered / face_xy;
    assert!(share.is_finite());

    // `recovered / face_3d` would be the retracted ratio. It does not
    // compile: see the `compile_fail` doctest cited above.
}

#[test]
#[ignore = "full-mesh drop-cutter sampling; run with --ignored --nocapture"]
fn wanaka_decomposes_to_order_ten_regions() {
    let path = wanaka_project_path();
    assert!(
        path.exists(),
        "wanaka.toml not found at {} — acceptance requires the canonical project",
        path.display()
    );
    let session = ProjectSession::load(&path).expect("load wanaka.toml");

    let mesh = session
        .models()
        .iter()
        .find_map(|m| m.mesh.clone())
        .expect("wanaka project must contain a mesh model");
    let index = SpatialIndex::build(&mesh, 10.0);

    // Ø6 ball nose — wanaka's finishing tool class; radius 3.0 drives both
    // the sampling resolution and the corridor/min-area defaults.
    let cutter = BallEndmill::new(6.0, 25.0);
    let tool_radius = 3.0;

    let cancel = || false;
    // Classification reads the TRUE surface (tiny-probe sampling): the Ø6
    // offset surface hides nearly all of wanaka's steepness (38.5% of true
    // area ≥45° reads as 0.1% there — see the diagnostic test below).
    let surface = build_classification_surface_with_cancel(&mesh, &index, &cutter, 0.05, &cancel)
        .expect("classification surface sampling");

    let params = FinishPlannerParams::for_tool(tool_radius);
    let planned = decompose_surface(&surface, &[], &params);

    // M1: these are `ProjectedXyAreaMm2`, not bare `f64`. The ground-truth
    // block below prints `SurfaceAreaMm2`, and the compiler will not let the
    // two be divided — that division is the retracted §14r "313 of 482".
    let band_stats = |band: FinishBand| -> (usize, ProjectedXyAreaMm2) {
        let regions = planned.regions.iter().filter(|r| r.band == band);
        let (mut n, mut area) = (0usize, ProjectedXyAreaMm2::default());
        for r in regions {
            n += 1;
            area += r.projected_xy_area_mm2();
        }
        (n, area)
    };
    let (n_shallow, a_shallow) = band_stats(FinishBand::Shallow);
    let (n_mid, a_mid) = band_stats(FinishBand::MidSteep);
    let (n_very, a_very) = band_stats(FinishBand::VerySteep);

    eprintln!("── wanaka decomposition (P2.b R1 acceptance) ──");
    eprintln!(
        "grid: {} x {} cells @ {:.3} mm",
        surface.rows(),
        surface.cols(),
        surface.cell_size()
    );
    eprintln!("measurement: {}", planned.stats.provenance.describe());
    // M1 4.3 / LH-3: the area column is XY-PROJECTED mm^2 and says so on every
    // row. The only 3D-surface area in this file is the ground-truth block in
    // `wanaka_band_mix_vs_cusp_radius`, and the two are different TYPES.
    eprintln!(
        "Shallow   : {n_shallow:3} regions, {:10.0} mm^2 XY-proj",
        a_shallow.mm2()
    );
    eprintln!(
        "MidSteep  : {n_mid:3} regions, {:10.0} mm^2 XY-proj",
        a_mid.mm2()
    );
    eprintln!(
        "VerySteep : {n_very:3} regions, {:10.0} mm^2 XY-proj",
        a_very.mm2()
    );
    eprintln!(
        "raw islands pre-conditioning: steep {}, very-steep {}",
        planned.stats.raw_steep_islands, planned.stats.raw_very_steep_islands
    );
    eprintln!(
        "absorbed: {}, final region_count: {}",
        planned.stats.absorbed_regions, planned.stats.region_count
    );

    let out_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("finish_planner_debug");
    std::fs::create_dir_all(&out_dir).expect("create debug output dir");

    let svg = planned_regions_to_svg(&planned, 1200.0, 1200.0);
    let out_path = out_dir.join("wanaka_regions.svg");
    std::fs::write(&out_path, &svg).expect("write debug SVG");
    eprintln!("debug SVG: {}", out_path.display());

    // Companion view with min-area absorption effectively off, so the raw
    // steep structure the conditioning absorbed stays visible — this is the
    // "what did conditioning do" picture, not an acceptance input.
    let raw_params = FinishPlannerParams {
        min_region_area_mm2: 1.0,
        ..FinishPlannerParams::for_tool(tool_radius)
    };
    let raw = decompose_surface(&surface, &[], &raw_params);
    let raw_mid = raw
        .regions
        .iter()
        .filter(|r| r.band == FinishBand::MidSteep)
        .count();
    let raw_very = raw
        .regions
        .iter()
        .filter(|r| r.band == FinishBand::VerySteep)
        .count();
    eprintln!(
        "min-area off: {} regions total ({raw_mid} MidSteep, {raw_very} VerySteep)",
        raw.stats.region_count
    );
    let raw_svg = planned_regions_to_svg(&raw, 1200.0, 1200.0);
    let raw_path = out_dir.join("wanaka_regions_no_min_area.svg");
    std::fs::write(&raw_path, &raw_svg).expect("write raw debug SVG");
    eprintln!("debug SVG (min-area off): {}", raw_path.display());

    assert!(
        !planned.regions.is_empty(),
        "wanaka must decompose to at least one region"
    );
    assert!(
        planned.stats.region_count <= 24,
        "R1 acceptance: wanaka must decompose to O(10) regions, got {}",
        planned.stats.region_count
    );
}

/// Diagnostic (no assertions beyond sanity): compare the TRUE surface slope
/// distribution (area-weighted mesh normals, upward faces only) against the
/// slope map the planner classifies on — which is built from the drop-cutter
/// heightmap, i.e. the BALL-CENTER OFFSET surface. A ball of radius r rounds
/// a steep wall into a ramp spread over ~r, so the offset surface
/// systematically under-reads steepness. This quantifies how much of
/// wanaka's real steepness the Ø6-ball offset surface hides.
#[test]
#[ignore = "full-mesh sampling; run with --ignored --nocapture"]
fn wanaka_slope_distribution_diagnostic() {
    let path = wanaka_project_path();
    let session = ProjectSession::load(&path).expect("load wanaka.toml");
    let mesh = session
        .models()
        .iter()
        .find_map(|m| m.mesh.clone())
        .expect("wanaka project must contain a mesh model");
    let index = SpatialIndex::build(&mesh, 10.0);

    const BANDS: [(f64, f64, &str); 7] = [
        (0.0, 15.0, "  0-15"),
        (15.0, 25.0, " 15-25"),
        (25.0, 35.0, " 25-35"),
        (35.0, 45.0, " 35-45"),
        (45.0, 55.0, " 45-55"),
        (55.0, 65.0, " 55-65"),
        (65.0, 90.1, " 65-90"),
    ];
    let band_of = |deg: f64| BANDS.iter().position(|&(lo, hi, _)| deg >= lo && deg < hi);

    // A: true surface — area-weighted face-normal angles, upward faces only
    // (excludes the solid STL's base and vertical side skirts).
    let mut face_area = [0.0f64; 7];
    let mut total_up_area = 0.0f64;
    let mut max_face_deg = 0.0f64;
    for f in &mesh.faces {
        if f.normal.z <= 0.01 {
            continue;
        }
        let e1 = f.v[1] - f.v[0];
        let e2 = f.v[2] - f.v[0];
        let area = e1.cross(&e2).norm() * 0.5;
        if area <= 1e-12 {
            continue;
        }
        let deg = f.normal.z.clamp(0.0, 1.0).acos().to_degrees();
        max_face_deg = max_face_deg.max(deg);
        if let Some(b) = band_of(deg)
            && let Some(slot) = face_area.get_mut(b)
        {
            *slot += area;
        }
        total_up_area += area;
    }

    // B: what the planner sees — ball-center offset surface at Ø6.
    let cutter = BallEndmill::new(6.0, 25.0);
    let cancel = || false;
    let surface = build_finish_surface_with_cancel(&mesh, &index, &cutter, 0.05, &cancel)
        .expect("finish surface sampling");
    let mut cell_count = [0usize; 7];
    let mut covered_cells = 0usize;
    let mut max_cell_deg = 0.0f64;
    for (i, &angle) in surface.slope_map.angles.iter().enumerate() {
        if !surface.heightmap.covered.get(i).copied().unwrap_or(false) {
            continue;
        }
        let deg = angle.to_degrees();
        max_cell_deg = max_cell_deg.max(deg);
        if let Some(b) = band_of(deg)
            && let Some(slot) = cell_count.get_mut(b)
        {
            *slot += 1;
        }
        covered_cells += 1;
    }

    eprintln!("── wanaka slope distribution: true surface vs Ø6 offset surface ──");
    eprintln!(
        "mesh: {} faces, bbox z {:.2}..{:.2} ({:.2} mm relief)",
        mesh.faces.len(),
        mesh.bbox.min.z,
        mesh.bbox.max.z,
        mesh.bbox.max.z - mesh.bbox.min.z
    );
    // LH-3: two DIFFERENT denominators printed side by side - column A is a
    // share of 3D mesh face area, column B a share of covered classification
    // cells on the Ø6 offset surface. They are not a ratio pair and this
    // table never divides one by the other; the header says which is which.
    eprintln!(
        "band    | 3D-surface area% (of {total_up_area:.0} mm^2 upward faces) | \
         grid cell% (of {covered_cells} covered cells)"
    );
    for (b, &(_, _, name)) in BANDS.iter().enumerate() {
        let a_pct = 100.0 * face_area.get(b).copied().unwrap_or(0.0) / total_up_area.max(1e-9);
        let c_pct = 100.0 * cell_count.get(b).copied().unwrap_or(0) as f64
            / (covered_cells as f64).max(1.0);
        eprintln!("{name}  | {a_pct:19.1}% 3D-surf | {c_pct:20.1}% cells");
    }
    eprintln!("max     | {max_face_deg:19.1}° face | {max_cell_deg:20.1}° cell");
    assert!(total_up_area > 0.0);
    assert!(covered_cells > 0);
}

/// P2.e Tier-1 sweep (design risk R4): OFAT over the CONDITIONING dials at
/// the decomposition level — the classification surface is sampled once and
/// `decompose_surface` re-runs per row (milliseconds each), so this sweeps
/// wide where the full-chain harness can't afford to. Structural scores
/// only: per-band region counts + areas, raw-island counts, absorption.
/// The chain-scored threshold sweep lives in `p2c_headless_ab_wanaka.rs`
/// (`p2e_threshold_chain_sweep`), sharing the same defaults so the two
/// tables cross-reference.
///
/// VerySteep area doubles as the COASTLINE-SURVIVAL proxy: the shoreline
/// ribbon is the thin VerySteep structure min-area tends to eat (the
/// design doc's "thin-ring absorption" datapoint).
#[test]
#[ignore = "wanaka conditioning dial sweep; run with --ignored --nocapture"]
fn p2e_conditioning_dial_sweep() {
    let path = wanaka_project_path();
    let session = ProjectSession::load(&path).expect("load wanaka.toml");
    let mesh = session
        .models()
        .iter()
        .find_map(|m| m.mesh.clone())
        .expect("wanaka project must contain a mesh model");
    let index = SpatialIndex::build(&mesh, 10.0);
    let cutter = BallEndmill::new(6.0, 25.0);
    let tool_radius = 3.0;
    let cancel = || false;
    let surface = build_classification_surface_with_cancel(&mesh, &index, &cutter, 0.05, &cancel)
        .expect("classification surface sampling");

    let base = FinishPlannerParams::for_tool(tool_radius);
    let mut rows: Vec<(String, FinishPlannerParams)> = vec![("default".into(), base.clone())];
    for v in [35.0, 40.0, 50.0, 55.0] {
        rows.push((
            format!("steep={v}"),
            FinishPlannerParams {
                steep_threshold_deg: v,
                ..base.clone()
            },
        ));
    }
    for v in [55.0, 75.0, 85.0] {
        rows.push((
            format!("waterline={v}"),
            FinishPlannerParams {
                waterline_threshold_deg: v,
                ..base.clone()
            },
        ));
    }
    for v in [0.0, 5.0, 15.0] {
        rows.push((
            format!("hysteresis={v}"),
            FinishPlannerParams {
                hysteresis_deg: v,
                ..base.clone()
            },
        ));
    }
    for v in [0.0, 0.75, 3.0] {
        rows.push((
            format!("close={v}"),
            FinishPlannerParams {
                close_radius_mm: v,
                ..base.clone()
            },
        ));
    }
    // Default min-area = 4·(2r)² = 144 mm² at r=3. ×0.25 keeps thin coast
    // ribbons independent; ×4 absorbs aggressively.
    for v in [36.0, 576.0] {
        rows.push((
            format!("min_area={v}"),
            FinishPlannerParams {
                min_region_area_mm2: v,
                ..base.clone()
            },
        ));
    }

    eprintln!("── P2.e Tier-1: conditioning dial sweep (wanaka, decompose-only) ──");
    // LH-3: every `mm2` column below is XY-PROJECTED area at polygon
    // extraction on the classification grid. Never a share of a 3D face area.
    eprintln!("area columns: XY-projected mm^2 at polygon extraction");
    eprintln!(
        "{:<16} | {:>7} | {:>13} | {:>13} | {:>13} | {:>8} | {:>9}",
        "row",
        "regions",
        "shallow n/xy-mm2",
        "mid n/xy-mm2",
        "very n/xy-mm2",
        "absorbed",
        "raw s/vs"
    );
    for (label, params) in &rows {
        let planned = decompose_surface(&surface, &[], params);
        let stat = |band: FinishBand| -> (usize, ProjectedXyAreaMm2) {
            planned
                .regions
                .iter()
                .filter(|r| r.band == band)
                .fold((0usize, ProjectedXyAreaMm2::default()), |(n, a), r| {
                    (n + 1, a + r.projected_xy_area_mm2())
                })
        };
        let (sn, sa) = stat(FinishBand::Shallow);
        let (mn, ma) = stat(FinishBand::MidSteep);
        let (vn, va) = stat(FinishBand::VerySteep);
        let (sa, ma, va) = (sa.mm2(), ma.mm2(), va.mm2());
        eprintln!(
            "{label:<16} | {:>7} | {sn:>3}/{sa:>9.0} | {mn:>3}/{ma:>9.0} | {vn:>3}/{va:>9.0} | {:>8} | {:>4}/{:>4}{}",
            planned.stats.region_count,
            planned.stats.absorbed_regions,
            planned.stats.raw_steep_islands,
            planned.stats.raw_very_steep_islands,
            if planned.stats.region_count > 24 {
                "  << OVER R1 BOUND"
            } else {
                ""
            }
        );
        // Stability invariants — every dial row must stay conditioned, not
        // just the default (the R1 "island storm" regression net). Rows
        // over the O(10) acceptance bound are flagged in the table (a
        // finding, not a failure); an O(100) storm fails the sweep.
        assert!(
            planned.stats.region_count <= 100,
            "[{label}] conditioning collapsed into an island storm: {} regions",
            planned.stats.region_count
        );
        assert!(
            !planned.regions.is_empty(),
            "[{label}] decomposition produced nothing"
        );
    }

    // Determinism at the default row (the sweep's anchor).
    let a = decompose_surface(&surface, &[], &base);
    let b = decompose_surface(&surface, &[], &base);
    assert_eq!(a.stats.region_count, b.stats.region_count);
    assert_eq!(a.regions.len(), b.regions.len());
}

/// §14q — how much steep territory does the min-area floor absorb, as a
/// function of the tool's CUSP radius?
///
/// `FinishPlannerParams::for_tool` derives every feature-scale dial from
/// the cusp-forming radius: `min_region_area_mm2 = (2r)²·4`,
/// `close_radius_mm = r/2`. Before the §14q fix the callers passed
/// `MillingCutter::radius()`, which on a TAPERED ball is the SHAFT radius
/// — 3.0 mm for a Ø1 tip on a 6 mm shank. That made the area floor
/// 144 mm² instead of 4 mm² (36×) and the close radius 1.5 mm instead of
/// 0.25 mm (6×).
///
/// Wanaka is 38.5% steeper than 45° by true area but has only ~6 mm of
/// relief, so its steep faces are RIBBONS a few millimetres wide — exactly
/// the scale a 1.5 mm close merges away and a 144 mm² floor absorbs.
///
/// **This does NOT isolate the dials.** Each row rebuilds the surface,
/// because the classification cell size follows the cusp radius too
/// (§14q) — so every row varies resolution AND dials together, and the
/// rows cannot attribute a change to either. The docstring claimed
/// isolation for a while after `32c5e48` made it false; the independent
/// audit caught it (§14t). The isolating experiment is the second table
/// below, which pins ONE grid and varies only the dials on it — and it
/// shows the dials barely matter at this resolution.
///
/// Diagnostic, not a gate: it prints band mixes and asserts nothing.
#[test]
#[ignore = "full-mesh sampling; run with --ignored --nocapture"]
fn wanaka_band_mix_vs_cusp_radius() {
    let path = wanaka_project_path();
    let session = ProjectSession::load(&path).expect("load wanaka project");
    let mesh = session
        .models()
        .iter()
        .find_map(|m| m.mesh.clone())
        .expect("wanaka project must contain a mesh model");
    let index = SpatialIndex::build(&mesh, 10.0);
    // Each row builds its own probe below; this fixture keeps no shared cutter.
    let cancel = || false;

    eprintln!("── wanaka band mix vs cusp radius (§14q) ──");
    // LH-3: the three band columns are `n regions / XY-PROJECTED mm^2`. The
    // ground-truth block below prints TRUE 3D face area in its own column,
    // typed `SurfaceAreaMm2` - dividing one by the other does not compile.
    eprintln!("band columns: regions / XY-projected mm^2 (NOT 3D surface area)");
    eprintln!(
        "{:>7} {:>9} {:>8} {:>11} {:>8}  {:>13} {:>13} {:>13}",
        "cusp_r", "min_area", "close_r", "grid", "sample_s", "Shallow", "MidSteep", "VerySteep"
    );

    // 3.0 = Ø6 ball (radius == cusp radius, unaffected by the fix).
    // 0.5 = the Ø1 tapered ball this project actually finishes with —
    //       what `radius()` used to report as 3.0.
    for cusp_r in [3.0_f64, 1.0, 0.5, 0.25] {
        // Surface rebuilt per row: the classification CELL SIZE follows the
        // cusp radius too (§14q), so the grid must track the dial or the
        // finer dials measure a grid that cannot represent their features.
        let probe = TaperedBallEndmill::new(cusp_r * 2.0, 7.0, 6.0, 25.0);
        let t0 = std::time::Instant::now();
        let surface =
            build_classification_surface_with_cancel(&mesh, &index, &probe, 0.05, &cancel)
                .expect("classification surface sampling");
        let sample_s = t0.elapsed().as_secs_f64();
        let (rows, cols) = (surface.heightmap.rows, surface.heightmap.cols);
        let params = FinishPlannerParams::for_tool(cusp_r);
        let planned = decompose_surface(&surface, &[], &params);
        // M1 slice 2: `ProjectedXyAreaMm2`, so this column cannot be divided
        // by the 3D `SurfaceAreaMm2` column printed ~30 lines below — the
        // exact pair §14r divided ("313 of 482").
        let stats = |band: FinishBand| -> (usize, ProjectedXyAreaMm2) {
            let (mut n, mut area) = (0usize, ProjectedXyAreaMm2::default());
            for r in planned.regions.iter().filter(|r| r.band == band) {
                n += 1;
                area += r.projected_xy_area_mm2();
            }
            (n, area)
        };
        let (ns, as_) = stats(FinishBand::Shallow);
        let (nm, am) = stats(FinishBand::MidSteep);
        let (nv, av) = stats(FinishBand::VerySteep);
        eprintln!(
            "{cusp_r:>7.2} {:>9.1} {:>8.2} {:>11} {:>8.1}  {:>5}/{:>7.0} {:>5}/{:>7.0} {:>5}/{:>7.0}",
            params.min_region_area_mm2,
            params.close_radius_mm,
            format!("{rows}x{cols}"),
            sample_s,
            ns,
            as_.mm2(),
            nm,
            am.mm2(),
            nv,
            av.mm2()
        );
    }

    // ── Ground truth, in BOTH measures ──────────────────────────────────
    //
    // `PlannedRegion::polygon.area()` is XY-PROJECTED. Mesh face area is
    // 3D. On near-vertical ribbons those differ by ~10×, so comparing an
    // emitted region area against a 3D face area is meaningless — §14r did
    // exactly that ("313 of 482 mm² recovered") and the audit killed it.
    // Print both so the mistake cannot be repeated silently.
    //
    // M1 (PR-0 slice 2): the prose above is no longer the only guard. The
    // two columns are now DIFFERENT TYPES — `SurfaceAreaMm2` and
    // `ProjectedXyAreaMm2` — with no `Div` between them, so `a3 / a_very`
    // does not compile. The only sanctioned crossing is
    // `SurfaceAreaMm2::project_onto_xy`, applied PER FACE below (never to an
    // aggregate over mixed slopes) and never in reverse.
    let mut truth: Vec<(f64, SurfaceAreaMm2, ProjectedXyAreaMm2)> = Vec::new();
    for threshold_deg in [45.0_f64, 65.0, 75.0] {
        let cos_min = threshold_deg.to_radians().cos();
        let (mut area_3d, mut area_xy) = (SurfaceAreaMm2::default(), ProjectedXyAreaMm2::default());
        for f in &mesh.faces {
            // `Triangle::normal` is unit-length, so |n.z| IS cos(slope).
            let cos_slope = f.normal.z.abs();
            if cos_slope > cos_min {
                continue; // shallower than the threshold
            }
            let (a, b, c) = (f.v[0], f.v[1], f.v[2]);
            let a3 = SurfaceAreaMm2::new(0.5 * (b - a).cross(&(c - a)).norm());
            area_3d += a3;
            // The projection onto XY shrinks by exactly cos(slope).
            area_xy += a3.project_onto_xy(cos_slope);
        }
        truth.push((threshold_deg, area_3d, area_xy));
    }
    eprintln!("── ground truth from the mesh (§14t) ──");
    eprintln!(
        "{:>10} {:>14} {:>16}",
        "threshold", "true 3D area", "PROJECTED area"
    );
    for (deg, a3, axy) in &truth {
        eprintln!("{deg:>9.0}° {:>13.1} {:>15.1}", a3.mm2(), axy.mm2());
    }
    eprintln!(
        "Compare emitted VerySteep polygon area against the PROJECTED column, \
         and against the ≥65° row — 10° hysteresis grows the band down to there."
    );

    // ── The isolating experiment: ONE grid, dials varied ────────────────
    //
    // The table above cannot separate resolution from dials. This can: it
    // pins the surface at the real Ø1 tip and varies only
    // `FinishPlannerParams`. If the rows barely move, the residual is NOT
    // "legitimate conditioning by the dials" — that was §14r's claim.
    let probe = TaperedBallEndmill::new(1.0, 7.0, 6.0, 25.0);
    let surface = build_classification_surface_with_cancel(&mesh, &index, &probe, 0.05, &cancel)
        .expect("classification surface sampling");
    eprintln!("── dials varied on ONE fixed 0.5mm-cusp grid (§14t) ──");
    eprintln!(
        "{:>7} {:>9} {:>8}  {:>13} {:>13} {:>13}",
        "dial_r", "min_area", "close_r", "Shallow", "MidSteep", "VerySteep"
    );
    for dial_r in [3.0_f64, 1.0, 0.5, 0.25] {
        let params = FinishPlannerParams::for_tool(dial_r);
        let planned = decompose_surface(&surface, &[], &params);
        let stats = |band: FinishBand| -> (usize, ProjectedXyAreaMm2) {
            let (mut n, mut area) = (0usize, ProjectedXyAreaMm2::default());
            for r in planned.regions.iter().filter(|r| r.band == band) {
                n += 1;
                area += r.projected_xy_area_mm2();
            }
            (n, area)
        };
        let (ns, as_) = stats(FinishBand::Shallow);
        let (nm, am) = stats(FinishBand::MidSteep);
        let (nv, av) = stats(FinishBand::VerySteep);
        eprintln!(
            "{dial_r:>7.2} {:>9.1} {:>8.2}  {:>5}/{:>7.0} {:>5}/{:>7.0} {:>5}/{:>7.0}",
            params.min_region_area_mm2,
            params.close_radius_mm,
            ns,
            as_.mm2(),
            nm,
            am.mm2(),
            nv,
            av.mm2()
        );
    }
}
