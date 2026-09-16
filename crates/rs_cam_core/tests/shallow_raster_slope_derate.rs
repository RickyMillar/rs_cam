//! Sentries for the honest-raster slope derate (Track B fix, 2026-09-01,
//! operator ruling: ALWAYS ON, no dial).
//!
//! The shipped `FinishBand::Shallow` arm spaces its passes in XY
//! projection, so on a slope `theta` the achieved surface spacing is
//! `s_XY / cos(theta)` — the configured scallop is exceeded by up to
//! 1.31x at 40 deg (`planning/honest_raster_2026-09-01/FINDINGS.md`).
//! The fix derates each region's effective stepover by `cos(theta_max)`
//! of that region before any lattice is built.
//!
//! Two sentries, both fast and NOT `#[ignore]`d:
//!
//! * **Flat identity (F-FIX3):** a flat region carries `theta_max = 0`,
//!   takes no derate, stays on the shared 0-degree memo, and emits rows
//!   exactly at the configured stepover. `cos 0 = 1` — the pre-fix
//!   emission, unchanged.
//! * **Sloped derate:** a 30-degree plane gets exactly the
//!   `cos(theta_max)` derate; the report's `ShallowSlopeDerate` entry,
//!   the `derived_stepovers` audit trail contract, and the emitted row
//!   spacing must all agree.
//!
//! The full spacing measurement against the analytic surface lives in the
//! `shipped_raster_spacing_b1` evidence instrument (fix-acceptance gate).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::BTreeMap;

use rs_cam_core::finish::finish_planner::{FinishBand, FinishPlannerParams};
use rs_cam_core::finish::unified_finish::{
    UnifiedFinishParams, UnifiedFinishReport, unified_finish_toolpath_with_cancel,
};
use rs_cam_core::geo::P3;
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::tool::{BallEndmill, MillingCutter};
use rs_cam_core::toolpath::{MoveIntent, Toolpath};

/// Half-side (mm) of the plane patches in XY.
const PLANE_HALF_MM: f64 = 6.0;
const RASTER_STEPOVER_MM: f64 = 0.5;

/// A plane tilted `theta_deg` about X, slope rising along +Y — the
/// cross-feed direction of the 0-degree raster. Same construction as
/// `shipped_raster_spacing_b1::tilted_plane_mesh`.
fn tilted_plane_mesh(theta_deg: f64, cells: usize) -> TriangleMesh {
    let m = theta_deg.to_radians().tan();
    let n = cells.max(1);
    let step = 2.0 * PLANE_HALF_MM / (n as f64);
    let mut verts: Vec<P3> = Vec::with_capacity((n + 1) * (n + 1));
    for iy in 0..=n {
        let y = -PLANE_HALF_MM + (iy as f64) * step;
        for ix in 0..=n {
            let x = -PLANE_HALF_MM + (ix as f64) * step;
            verts.push(P3::new(x, y, m * (y + PLANE_HALF_MM)));
        }
    }
    let idx = |ix: usize, iy: usize| -> u32 { (iy * (n + 1) + ix) as u32 };
    let mut tris: Vec<[u32; 3]> = Vec::with_capacity(2 * n * n);
    for iy in 0..n {
        for ix in 0..n {
            let (a, b) = (idx(ix, iy), idx(ix + 1, iy));
            let (c, d) = (idx(ix, iy + 1), idx(ix + 1, iy + 1));
            tris.push([a, b, d]);
            tris.push([a, d, c]);
        }
    }
    TriangleMesh::from_raw(verts, tris)
}

/// Run the shipped orchestrator with the default parameter shape (the
/// `shipped_raster_spacing_b1` construction, condensed).
fn run_shipped(mesh: &TriangleMesh) -> (Toolpath, UnifiedFinishReport) {
    let cutter = BallEndmill::new(2.0, 10.0);
    let index = SpatialIndex::build_auto(mesh);
    let params = UnifiedFinishParams {
        raster_stepover: RASTER_STEPOVER_MM,
        intra_region_hookup_mm: 0.0,
        ..UnifiedFinishParams::default()
    };
    let planner = FinishPlannerParams::for_tool(cutter.cusp_radius_mm());
    let never_cancel = || false;
    let (toolpath, _anns, report) = unified_finish_toolpath_with_cancel(
        mesh,
        &index,
        &cutter,
        mesh.bbox.max.z + 1.0,
        mesh.bbox.min.z - 1.0,
        &params,
        &planner,
        /* machining_boundary */ None,
        /* link_kinematics */ None,
        /* claims */ None,
        /* debug */ None,
        &never_cancel,
    )
    .expect("uncancelled generation");
    (toolpath, report)
}

fn assert_all_shallow(report: &UnifiedFinishReport) {
    assert!(
        !report.region_table.is_empty(),
        "planner produced no regions"
    );
    for entry in &report.region_table {
        assert_eq!(
            entry.kind.band(),
            Some(FinishBand::Shallow),
            "fixture must decompose to the Shallow band only"
        );
    }
}

/// Sorted Delta-y values between adjacent emitted raster rows.
fn row_dys(toolpath: &Toolpath) -> Vec<f64> {
    let mut rows: BTreeMap<i64, usize> = BTreeMap::new();
    for mv in &toolpath.moves {
        if mv.intent != MoveIntent::FinishingCut || !mv.move_type.is_cutting() {
            continue;
        }
        let key = (mv.target.y * 1e6).round() as i64;
        *rows.entry(key).or_default() += 1;
    }
    let ys: Vec<f64> = rows.keys().map(|&k| k as f64 * 1e-6).collect();
    ys.windows(2).map(|p| p[1] - p[0]).collect()
}

/// F-FIX3: flat ground takes no derate and keeps the configured stepover —
/// the shared-memo, pre-fix emission (`cos 0 = 1`).
#[test]
fn flat_region_takes_no_derate_and_keeps_the_configured_stepover() {
    let mesh = tilted_plane_mesh(0.0, 12);
    let (toolpath, report) = run_shipped(&mesh);
    assert_all_shallow(&report);
    assert!(
        report.shallow_slope_derates.is_empty(),
        "a flat region must not be derated, got {:?}",
        report.shallow_slope_derates
    );
    let dys = row_dys(&toolpath);
    assert!(!dys.is_empty(), "no adjacent raster rows emitted");
    for dy in &dys {
        assert!(
            (dy - RASTER_STEPOVER_MM).abs() < 1e-6,
            "flat rows must sit exactly at the configured stepover; got {dy}"
        );
    }
}

/// A 30-degree plane is derated by `cos(theta_max)`, the report entry and
/// the emitted motion agree, and the derate reads the region's own slope.
#[test]
fn sloped_region_derates_by_cos_theta_max() {
    let mesh = tilted_plane_mesh(30.0, 96);
    let (toolpath, report) = run_shipped(&mesh);
    assert_all_shallow(&report);
    assert_eq!(
        report.shallow_slope_derates.len(),
        1,
        "one Shallow region, one derate entry"
    );
    let d = report.shallow_slope_derates[0];
    assert!(
        (d.configured_stepover_mm - RASTER_STEPOVER_MM).abs() < 1e-12,
        "the operator's dial value must be carried unchanged"
    );
    assert!(
        (28.0..=33.0).contains(&d.slope_max_deg),
        "theta_max must read the fixture's 30-degree slope, got {}",
        d.slope_max_deg
    );
    let expected = d.configured_stepover_mm * d.slope_max_deg.to_radians().cos();
    assert!(
        (d.derated_stepover_mm - expected).abs() < 1e-12,
        "derated value must be configured x cos(theta_max)"
    );
    // Non-vacuity: the derate moved the lattice.
    assert!(d.derated_stepover_mm < RASTER_STEPOVER_MM - 1e-3);
    let dys = row_dys(&toolpath);
    assert!(!dys.is_empty(), "no adjacent raster rows emitted");
    for dy in &dys {
        assert!(
            (dy - d.derated_stepover_mm).abs() < 1e-6,
            "sloped rows must sit exactly at the derated stepover \
             {}; got {dy}",
            d.derated_stepover_mm
        );
    }
}
