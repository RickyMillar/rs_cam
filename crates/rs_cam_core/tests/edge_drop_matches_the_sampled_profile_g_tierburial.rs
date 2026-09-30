//! G-TIERBURIAL (`planning/tiered_finish_2026-09-30/RESULTS.md`, "Fine-tier
//! burial, part 2"): the drop-cutter `edge_drop` of the ball, the bull nose
//! and the tapered ball, against a drop computed from the tool profile
//! alone.
//!
//! The truth for one edge: a point `p` of the edge at horizontal distance
//! `rho` from the tool axis touches the tool when the tip is at
//! `p.z - height_at_radius(rho)`. The drop is the largest such tip height
//! over the edge. On the reachable stretch of the edge (`rho` up to the
//! tool radius) that function of the edge parameter is concave (the edge Z
//! is linear, the profile height is convex in `rho`, `rho` is convex in the
//! parameter), so a dense scan plus a ternary search finds its maximum to
//! machine precision.
//!
//! The defect: `edge_drop` placed the contact at `t_closest + s cos_a` with
//! `cos_a = -slope / sqrt(1 + slope^2)` for the upper solution. The tangent
//! point of a circle resting on a line of slope `m` is at `+ s m /
//! sqrt(1 + m^2)`, so the code evaluated the mirrored, downhill point, and
//! the drop was low on every sloped edge (exact on a level one). The edge
//! drop is compared together with the two vertex drops, because a contact
//! at an end of the edge is the vertex test's.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

use rs_cam_core::geo::P3;
use rs_cam_core::tool::{BallEndmill, BullNoseEndmill, CLPoint, MillingCutter, TaperedBallEndmill};

/// Edge slopes (dz per mm of XY): level, 26.6 degrees, 49.6 degrees (the
/// rivmap100 board-edge slope).
const SLOPES: [f64; 3] = [0.0, 0.5, 1.174];
/// Edge length in XY (mm) and its heading (degrees), off the axes.
const EDGE_LEN: f64 = 10.0;
const HEADING_DEG: f64 = 30.0;

/// The largest tip height at which the tool touches the edge `a -> b`, from
/// the profile only.
fn profile_drop(tool: &dyn MillingCutter, cx: f64, cy: f64, a: P3, b: P3) -> f64 {
    let (dx, dy, dz) = (b.x - a.x, b.y - a.y, b.z - a.z);
    let len2 = dx * dx + dy * dy;
    let f = |t: f64| {
        let (x, y) = (a.x + dx * t, a.y + dy * t);
        let rho = (x - cx).hypot(y - cy);
        tool.height_at_radius(rho)
            .map_or(f64::NEG_INFINITY, |h| a.z + dz * t - h)
    };
    // The reachable stretch: rho <= the tool's radius.
    let tc = ((cx - a.x) * dx + (cy - a.y) * dy) / len2;
    let (px, py) = (a.x + dx * tc, a.y + dy * tc);
    let d2 = (cx - px).powi(2) + (cy - py).powi(2);
    let r = tool.radius();
    if d2 > r * r {
        return f64::NEG_INFINITY;
    }
    let half = ((r * r - d2).sqrt() / len2.sqrt()) * (1.0 - 1e-12);
    let (mut lo, mut hi) = ((tc - half).max(0.0), (tc + half).min(1.0));
    // Dense scan to bracket, then ternary search on the concave function.
    let n = 20_000;
    let mut best = (f64::NEG_INFINITY, lo);
    for k in 0..=n {
        let t = lo + (hi - lo) * k as f64 / n as f64;
        let v = f(t);
        if v > best.0 {
            best = (v, t);
        }
    }
    let step = (hi - lo) / n as f64;
    (lo, hi) = ((best.1 - step).max(lo), (best.1 + step).min(hi));
    for _ in 0..200 {
        let m1 = lo + (hi - lo) / 3.0;
        let m2 = hi - (hi - lo) / 3.0;
        if f(m1) < f(m2) {
            lo = m1;
        } else {
            hi = m2;
        }
    }
    best.0.max(f(0.5 * (lo + hi)))
}

/// The kernel's drop against the edge: `edge_drop` and both vertex drops.
fn kernel_drop(tool: &dyn MillingCutter, cx: f64, cy: f64, a: P3, b: P3) -> f64 {
    let mut cl = CLPoint::new(cx, cy);
    tool.vertex_drop(&mut cl, &a);
    tool.vertex_drop(&mut cl, &b);
    tool.edge_drop(&mut cl, &a, &b);
    cl.z
}

/// Every slope, every CL offset from the edge (perpendicular, both sides),
/// the CL beside the middle of the edge; the worst |kernel - truth|.
fn worst_error(name: &str, tool: &dyn MillingCutter, offsets: &[f64]) -> f64 {
    let (hx, hy) = (
        HEADING_DEG.to_radians().cos(),
        HEADING_DEG.to_radians().sin(),
    );
    let mut worst = 0.0f64;
    for &m in &SLOPES {
        let a = P3::new(1.0, 2.0, 3.0);
        let b = P3::new(a.x + EDGE_LEN * hx, a.y + EDGE_LEN * hy, a.z + EDGE_LEN * m);
        for &d in offsets {
            for side in [1.0, -1.0] {
                // Beside the middle, shifted along the edge so the contact
                // is not symmetric about the foot of the perpendicular.
                let (cx, cy) = (
                    a.x + 0.45 * EDGE_LEN * hx - side * d * hy,
                    a.y + 0.45 * EDGE_LEN * hy + side * d * hx,
                );
                let truth = profile_drop(tool, cx, cy, a, b);
                let got = kernel_drop(tool, cx, cy, a, b);
                let err = (got - truth).abs();
                if err > 1e-6 {
                    eprintln!(
                        "{name}: slope {m}, offset {d}: kernel {got:.9}, profile {truth:.9} \
                         (error {err:.3e})"
                    );
                }
                worst = worst.max(err);
            }
        }
    }
    eprintln!("{name}: worst |kernel - profile| {worst:.3e}");
    worst
}

/// Both sides of the drop are a few mm; 1e-9 mm is nine orders below the
/// defect and above the ternary search's floating-point floor.
const TOL_MM: f64 = 1e-9;

#[test]
fn the_ball_edge_drop_matches_its_profile() {
    let tool = BallEndmill::new(2.0, 25.0);
    let worst = worst_error("ball R1", &tool, &[0.0, 0.3, 0.8, 0.99]);
    assert!(
        worst <= TOL_MM,
        "ball edge drop off its profile by {worst:.3e} mm"
    );
}

#[test]
fn the_bull_nose_edge_drop_matches_its_profile() {
    let tool = BullNoseEndmill::new(6.0, 1.0, 25.0);
    let worst = worst_error("bull 6/r1", &tool, &[0.0, 1.0, 2.2, 2.6, 2.95]);
    assert!(
        worst <= TOL_MM,
        "bull nose edge drop off its profile by {worst:.3e} mm"
    );
}

#[test]
fn the_tapered_ball_edge_drop_matches_its_profile() {
    // The rivmap100 fine tier's tool: R1.0 tip, 5.7 degrees, 6 mm shank.
    let tool = TaperedBallEndmill::new(2.0, 5.7, 6.0, 20.0);
    let worst = worst_error("taper R1/5.7/6", &tool, &[0.0, 0.3, 0.8, 0.99, 1.5, 2.5]);
    assert!(
        worst <= TOL_MM,
        "tapered ball edge drop off its profile by {worst:.3e} mm"
    );
}

/// The tapered ball on one sloped triangle whose tangency point lies off
/// the triangle (the rivmap100 board edge: a 100 mm triangle whose top edge
/// is the mesh boundary). The full `drop_cutter` must reach the profile drop
/// onto the triangle's edges. `TaperedBallEndmill::facet_drop` used to report
/// the tip-on-facet point under the axis as `found`, and `drop_cutter` then
/// skipped the edges: 11.0508 against 11.6614.
#[test]
fn a_tapered_ball_on_a_sloped_boundary_triangle_still_meets_its_edges() {
    let tool = TaperedBallEndmill::new(2.0, 5.7, 6.0, 20.0);
    let v = [
        P3::new(0.0, 0.0, 12.0),
        P3::new(100.0, 0.0, 12.0),
        P3::new(46.245, 3.0303, 8.165),
    ];
    let tri = rs_cam_core::geo::Triangle::new(v[0], v[1], v[2]);
    let (cx, cy) = (15.8, 0.75);
    // The ball's tangency with the plane is at `cl - R n` (XY); it lies off
    // the triangle (y < 0), so the drop is an edge or vertex contact.
    let (tx, ty) = (cx - tri.normal.x, cy - tri.normal.y);
    assert!(
        !tri.contains_point_xy(tx, ty),
        "non-vacuity: the tangency point ({tx:.3}, {ty:.3}) must be off the triangle"
    );
    let truth = (0..3)
        .map(|i| profile_drop(&tool, cx, cy, v[i], v[(i + 1) % 3]))
        .fold(f64::NEG_INFINITY, f64::max);
    let mut cl = CLPoint::new(cx, cy);
    tool.drop_cutter(&mut cl, &tri);
    eprintln!(
        "boundary triangle: drop_cutter {:.6}, profile {truth:.6}",
        cl.z
    );
    assert!(
        (cl.z - truth).abs() <= TOL_MM,
        "drop_cutter {:.6} against the profile drop {truth:.6}",
        cl.z
    );
}
