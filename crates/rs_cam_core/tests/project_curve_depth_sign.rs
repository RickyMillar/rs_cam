//! PR-2B / A1: project_curve depth-sign convention.
//!
//! The `ProjectCurveParams::depth` field is documented as "positive = into
//! material" (see `project_curve.rs:37`). This test pins that convention
//! down with explicit numerical assertions across all four combinations of
//! `(depth_sign, direction)` so a future refactor can't silently flip the
//! sign — and so the next reader can derive the user-facing meaning from
//! the test rather than re-deriving it from the math.
//!
//! Context: a user setting `depth: -2.0, direction: from_below` and
//! expecting "2 mm deep cut" gets a 100 % air-cut path (review
//! `planning/UX_DIALIN_REVIEW_2026-05-20.md` Phase A, TP3). The PR-1
//! `GeneratedEmpty` verdict (`session/compute.rs::diagnostics()`) already
//! catches the *symptom* post-generation; the `ProjectCurveNegativeDepth`
//! validator (PR-2B, this PR) catches it *pre*-generation with an
//! auto-fix that flips the sign.
//!
//! This test stays at the `project_curve_toolpath` level so the
//! convention is locked at the geometric boundary, independent of GUI /
//! MCP wiring.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::geo::P2;
use rs_cam_core::mesh::{SpatialIndex, make_test_flat};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::project_curve::{
    ProjectCurveParams, ProjectDirection, ProjectSide, project_curve_toolpath,
};
use rs_cam_core::tool::FlatEndmill;
use rs_cam_core::toolpath::MoveType;

fn small_square() -> Polygon2 {
    Polygon2::new(vec![
        P2::new(-15.0, -15.0),
        P2::new(15.0, -15.0),
        P2::new(15.0, 15.0),
        P2::new(-15.0, 15.0),
    ])
}

/// Min and max Z of all feed moves (skips rapids). Returns None if the
/// toolpath has no feed moves.
fn feed_z_range(tp: &rs_cam_core::toolpath::Toolpath) -> Option<(f64, f64)> {
    let zs: Vec<f64> = tp
        .moves
        .iter()
        .filter(|m| matches!(m.move_type, MoveType::Linear { .. }))
        .map(|m| m.target.z)
        .collect();
    if zs.is_empty() {
        return None;
    }
    let mut zmin = f64::INFINITY;
    let mut zmax = f64::NEG_INFINITY;
    for z in zs {
        if z < zmin {
            zmin = z;
        }
        if z > zmax {
            zmax = z;
        }
    }
    Some((zmin, zmax))
}

fn run(direction: ProjectDirection, depth: f64) -> Option<(f64, f64)> {
    let mesh = make_test_flat(100.0);
    let idx = SpatialIndex::build_auto(&mesh);
    let cutter = FlatEndmill::new(6.0, 25.0);
    let poly = small_square();
    let params = ProjectCurveParams {
        depth,
        feed_rate: 800.0,
        plunge_rate: 400.0,
        safe_z: 10.0,
        point_spacing: 1.0,
        direction,
        tool_radius: 3.0,
        side: ProjectSide::Center,
        setup_z_flipped: false,
    };
    let tp = project_curve_toolpath(&poly, &mesh, &idx, &cutter, &params);
    feed_z_range(&tp)
}

#[test]
fn convention_positive_depth_from_above_cuts_below_surface() {
    // Surface is at z=0; depth=+2 should put cutter at z=-2 (into material).
    let (zmin, zmax) = run(ProjectDirection::FromAbove, 2.0).expect("feed moves emitted");
    assert!(
        (zmin + 2.0).abs() < 1e-6 && (zmax + 2.0).abs() < 1e-6,
        "FromAbove + depth=+2 should put cutter at z=-2 (in material); got z range ({zmin:.3}, {zmax:.3})"
    );
}

#[test]
fn convention_negative_depth_from_above_lifts_above_surface() {
    // The user-error case in "FromAbove" form. depth=-2 puts the cutter 2 mm
    // *above* the surface (in air). The ProjectCurveNegativeDepth validator
    // exists to catch this; the geometric convention itself stays:
    // "positive = into material, negative = away from material".
    let (zmin, zmax) = run(ProjectDirection::FromAbove, -2.0).expect("feed moves emitted");
    assert!(
        (zmin - 2.0).abs() < 1e-6 && (zmax - 2.0).abs() < 1e-6,
        "FromAbove + depth=-2 should put cutter at z=+2 (in air above surface); \
         got z range ({zmin:.3}, {zmax:.3})"
    );
}

#[test]
fn convention_positive_depth_from_below_cuts_above_bottom_surface() {
    // FromBelow flips the mesh, finds top contact, then unflips and offsets
    // by depth in the cutter-approach direction. For a flat mesh at z=0,
    // depth=+2 puts the cutter at z=+2 (into the material *above* the
    // bottom face, since the cutter approaches from below pointing up).
    let (zmin, zmax) = run(ProjectDirection::FromBelow, 2.0).expect("feed moves emitted");
    assert!(
        (zmin - 2.0).abs() < 1e-6 && (zmax - 2.0).abs() < 1e-6,
        "FromBelow + depth=+2 should put cutter at z=+2 (into material from below); \
         got z range ({zmin:.3}, {zmax:.3})"
    );
}

#[test]
fn convention_negative_depth_from_below_drops_below_bottom_surface() {
    // The exact scenario the review's user hit: depth=-2 with from_below
    // puts the cutter at z=-2 (in air below the bottom face), not into
    // material. The validator should flag this *before* the sim runs.
    let (zmin, zmax) = run(ProjectDirection::FromBelow, -2.0).expect("feed moves emitted");
    assert!(
        (zmin + 2.0).abs() < 1e-6 && (zmax + 2.0).abs() < 1e-6,
        "FromBelow + depth=-2 should put cutter at z=-2 (in air below surface, 100% air-cut); \
         got z range ({zmin:.3}, {zmax:.3})"
    );
}
