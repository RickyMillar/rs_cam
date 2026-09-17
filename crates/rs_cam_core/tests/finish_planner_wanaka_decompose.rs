//! P2.b R1 acceptance: the unified-finish decomposition must condition the
//! wanaka terrain down to O(10) planned regions, not the O(100) island storm
//! a naive threshold classification produces
//! (`planning/unified_finish_planner_design.md`, risk R1).
//!
//! Also writes the SVG debug view — the P2.b visual surface — to
//! `target/finish_planner_debug/wanaka_regions.svg` so the decomposition can
//! be inspected before anything cuts.
//!
//! **One gate, one reminder.** FIN-05: this file held four `#[ignore]`
//! tests, of which one is a gate and three print tables and assert nothing.
//! The three moved to `finish_planner_wanaka_diagnostics.rs`, which builds
//! only under `heavy-tests`. A diagnostic that never ran in a gate does not
//! belong beside one.
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

use rs_cam_core::finish::finish_planner::{
    FinishBand, FinishPlannerParams, decompose_surface, planned_regions_to_svg,
};
use rs_cam_core::finish::finish_setup::build_classification_surface_with_cancel;
use rs_cam_core::measurement::{ProjectedXyAreaMm2, SurfaceAreaMm2};
use rs_cam_core::mesh::SpatialIndex;
use rs_cam_core::session::ProjectSession;
use rs_cam_core::tool::BallEndmill;

/// The pinned wanaka project this gate decomposes.
///
/// FIN-06: the harness read `planning/airrun_2026-06-01/wanaka.toml`, a
/// document the operator edits between machining sessions, and
/// `planning/CLAUDE.md` calls that directory evidence rather than a fixture
/// store. The fixture here is the dated snapshot the feeds sentries already
/// share (`wanaka_suggest_integration.rs` records the re-pin and its
/// reasons).
///
/// The swap cannot move this gate's numbers. The decomposition reads the
/// MESH and nothing else from the project, and both files name the same
/// `terrain.stl`. The dials that differ between the two are toolpath dials,
/// which this test never reads.
fn wanaka_project_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("wanaka_2026-08-16_f530995a.toml")
}

/// M1 gate at the crime scene (plan §M1 acceptance 1; `MEASUREMENT_DOMAINS.md`
/// LH-3 / §5).
///
/// This file prints an XY-projected band-area column and a 3D mesh-face-area
/// column ~30 lines apart. §14r divided one by the other and published "313 of
/// 482 mm² recovered (65%)"; the claim was retracted in `49047c4` and, until
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

/// FIN-06 guard: every project or baseline a test loads sits under
/// `tests/fixtures/`, not under `planning/`.
///
/// `planning/CLAUDE.md` calls that directory evidence. A test that reads a
/// file from it takes a document the operator edits as its input, and the
/// 2026-09-17 purge deleted 1010 files there — the four fixtures below
/// survived by chance.
///
/// Fast and NOT `#[ignore]`d, because it is the guard for the move: it goes
/// red the moment one of these files is renamed or dropped, which is a
/// failure the `#[ignore]` harnesses that read them cannot report.
#[test]
fn the_pinned_fixtures_live_under_tests_fixtures() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures");
    for name in [
        // This gate and the wanaka diagnostics.
        "wanaka_2026-08-16_f530995a.toml",
        // `p1_headless_ab_wanaka.rs`.
        "wanaka_airrun_2026-06-01_2c908dca.toml",
        // `tier_band_overlap_g_overlapfill.rs`.
        "t3b_r10_scallop_islands_2026-09-08_e4d817e9.toml",
        // `smoke_baseline_regression_f037.rs`.
        "smoke_baseline_2026-06-04_142f4a80.csv",
    ] {
        let path = dir.join(name);
        assert!(
            path.exists(),
            "fixture {name} is missing from tests/fixtures — a harness that \
             loads it fails only when somebody runs it with --ignored"
        );
    }
}

#[test]
#[ignore = "full-mesh drop-cutter sampling; run with --ignored --nocapture"]
fn wanaka_decomposes_to_order_ten_regions() {
    let path = wanaka_project_path();
    assert!(
        path.exists(),
        "the wanaka fixture is not at {} — acceptance requires the pinned project",
        path.display()
    );
    let session = ProjectSession::load(&path).expect("load the wanaka fixture");

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
