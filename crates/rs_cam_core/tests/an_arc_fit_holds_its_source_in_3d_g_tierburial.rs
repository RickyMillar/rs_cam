//! G-TIERBURIAL / tiered-finish plan F4 (`planning/tiered_finish_2026-09-30/
//! RESULTS.md`, "Fine-tier burial, part 3"): an arc the fitter emits stays
//! within its tolerance of the polyline it replaces, measured in 3D.
//!
//! The fitter accepted a run on three separate tests, each on one axis: every
//! vertex within `tolerance` of the circle, every chord's sagitta within
//! `tolerance`, every vertex Z within `tolerance` of the helix. Two defects
//! got past them:
//!
//! - The per-axis budgets add. A chord between two vertices that sit inside
//!   the circle has its own sagitta on top, so the arc runs up to twice the
//!   tolerance off the cut (`the_arc_holds_the_tolerance_where_the_axis_tests_add_up`).
//! - A semicircle's direction is decided by floating-point noise: both
//!   half circles have the same length, and the reflex test picked the one
//!   with the smaller sweep, which a centre a hair off the chord makes the
//!   wrong one. The fitter emitted the MIRRORED semicircle, 8 mm off the
//!   path at its apex (`a_semicircular_row_turn_is_not_emitted_mirrored`,
//!   the `arc_raster` fingerprint fixture's row turn).
//!
//! Both tests measure the emitted arcs directly: every point of every arc,
//! sampled along its sweep, against the nearest point of the source
//! polyline.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

use rs_cam_core::dressup::arcfit::fit_arcs;
use rs_cam_core::geo::P3;
use rs_cam_core::toolpath::{MoveType, Toolpath};
use rs_cam_core::trace::toolpath_spans::AnnotatedToolpath;
use rs_cam_core::trace::transform_provenance::ReconcileSet;

const TOL: f64 = 0.05;

/// The distance from `p` to the segment `a -> b` in 3D.
fn to_segment(p: P3, a: P3, b: P3) -> f64 {
    let (dx, dy, dz) = (b.x - a.x, b.y - a.y, b.z - a.z);
    let len2 = dx * dx + dy * dy + dz * dz;
    let t = if len2 > 0.0 {
        (((p.x - a.x) * dx + (p.y - a.y) * dy + (p.z - a.z) * dz) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    ((a.x + dx * t - p.x).powi(2) + (a.y + dy * t - p.y).powi(2) + (a.z + dz * t - p.z).powi(2))
        .sqrt()
}

/// The largest distance from a point of an emitted arc to the source
/// polyline (256 samples per arc, Z linear in the swept angle as GRBL
/// interpolates it).
fn worst_arc_deviation(source: &Toolpath, fitted: &Toolpath) -> (f64, usize) {
    let polyline: Vec<P3> = source.moves.iter().map(|m| m.target).collect();
    let mut worst = (0.0f64, 0usize);
    for (k, m) in fitted.moves.iter().enumerate().skip(1) {
        let (i, j, cw) = match m.move_type {
            MoveType::ArcCW { i, j, .. } => (i, j, true),
            MoveType::ArcCCW { i, j, .. } => (i, j, false),
            _ => continue,
        };
        let a = fitted.moves[k - 1].target;
        let b = m.target;
        let (cx, cy) = (a.x + i, a.y + j);
        let r = i.hypot(j);
        let a0 = (a.y - cy).atan2(a.x - cx);
        let a1 = (b.y - cy).atan2(b.x - cx);
        let mut sweep = if cw { a0 - a1 } else { a1 - a0 };
        while sweep <= 0.0 {
            sweep += std::f64::consts::TAU;
        }
        let dir = if cw { -1.0 } else { 1.0 };
        for s in 0..=256 {
            let f = s as f64 / 256.0;
            let ang = a0 + dir * sweep * f;
            let p = P3::new(
                cx + r * ang.cos(),
                cy + r * ang.sin(),
                a.z + (b.z - a.z) * f,
            );
            let d = polyline
                .windows(2)
                .map(|w| to_segment(p, w[0], w[1]))
                .fold(f64::INFINITY, f64::min);
            if d > worst.0 {
                worst = (d, k);
            }
        }
    }
    worst
}

fn fit(tp: &Toolpath) -> Toolpath {
    fit_arcs(AnnotatedToolpath::new(tp.clone()), TOL, f64::INFINITY)
        .reconcile(&mut ReconcileSet::new(None, None))
        .into_inner()
        .toolpath
}

fn arc_count(tp: &Toolpath) -> usize {
    tp.moves
        .iter()
        .filter(|m| {
            matches!(
                m.move_type,
                MoveType::ArcCW { .. } | MoveType::ArcCCW { .. }
            )
        })
        .count()
}

/// The `arc_raster` fingerprint fixture's row: a 24-segment half circle of
/// radius 8 centred at (10, y0), then a straight run. Rows at y0 = 0, 4, 8,
/// 12; the fitted centre sits off the chord by rounding at some of them.
#[test]
fn a_semicircular_row_turn_is_not_emitted_mirrored() {
    let mut worst = (0.0f64, 0usize, 0.0f64);
    let mut arcs = 0usize;
    for row in 0..4 {
        let y0 = row as f64 * 4.0;
        let z = -1.0 - row as f64;
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(2.0, y0, 10.0));
        tp.feed_to(P3::new(2.0, y0, z), 400.0);
        for step in 0..=24 {
            let theta = std::f64::consts::PI * (step as f64) / 24.0;
            tp.feed_to(
                P3::new(10.0 - 8.0 * theta.cos(), y0 + 8.0 * theta.sin(), z),
                1200.0,
            );
        }
        for step in 1..=20 {
            tp.feed_to(P3::new(18.0 + step as f64 * 0.2, y0, z), 1200.0);
        }
        let out = fit(&tp);
        arcs += arc_count(&out);
        let (d, k) = worst_arc_deviation(&tp, &out);
        eprintln!(
            "row {row}: {} arcs, worst arc point {d:.4} mm off the source (move {k})",
            arc_count(&out)
        );
        if d > worst.0 {
            worst = (d, k, y0);
        }
    }
    assert!(arcs > 0, "non-vacuity: the half circles fit arcs");
    assert!(
        worst.0 <= TOL + 1e-9,
        "an emitted arc (row y0 = {}, move {}) runs {:.4} mm off its source; the tolerance is {TOL}",
        worst.2,
        worst.1,
        worst.0
    );
}

/// A circle of radius 10 sampled every 8.8 degrees (chord sagitta 0.0294
/// mm), 30 vertices: the two ends on the circle, vertices 1-9 and 20-28
/// 0.035 mm outside it and vertices 10-19 0.035 mm inside. Every vertex is
/// within the tolerance of the fitted circle and every chord's sagitta is
/// within it, but a chord between two inside vertices runs the offset plus
/// its sagitta inside the arc.
#[test]
fn the_arc_holds_the_tolerance_where_the_axis_tests_add_up() {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(10.0, 0.0, 5.0));
    tp.feed_to(P3::new(10.0, 0.0, 0.0), 400.0);
    for k in 1..=29 {
        let a = (8.8 * k as f64).to_radians();
        let off = match k {
            29 => 0.0,
            10..=19 => -0.035,
            _ => 0.035,
        };
        let r = 10.0 + off;
        tp.feed_to(P3::new(r * a.cos(), r * a.sin(), 0.0), 1200.0);
    }
    let out = fit(&tp);
    let (d, k) = worst_arc_deviation(&tp, &out);
    eprintln!(
        "{} arcs, worst arc point {d:.4} mm off the source (move {k})",
        arc_count(&out)
    );
    assert!(arc_count(&out) > 0, "non-vacuity: the run fits arcs");
    assert!(
        d <= TOL + 1e-9,
        "an emitted arc (move {k}) runs {d:.4} mm off its source; the tolerance is {TOL}"
    );
}
