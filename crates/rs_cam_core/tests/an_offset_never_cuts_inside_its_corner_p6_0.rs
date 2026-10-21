//! P6.0 (geometry selection, design D-C, SAFETY): an offset ring never
//! cuts inside its corner.
//!
//! # The defect
//!
//! The single-shot offset (`polygon::offset_polygon_reported`) calls
//! cavalier, which joins two offset edges with an ARC. Before P6.0 the
//! offset kept only the two end points of each arc join
//! (`Polygon2::from_pline`), so the tool centre took the chord. The chord
//! lies `r (1 - cos(theta / 2))` nearer to the input corner than the arc:
//! 0.29 r at a 90 degree corner. The chord always falls on the INPUT side,
//! so the tool cuts into the corner of the part:
//!
//! - Profile Outside: at each convex corner of the part.
//! - Profile Inside and Pocket: at each reflex corner of the outline.
//! - Pocket round an island: at each convex corner of the island.
//!
//! # The rule under test
//!
//! Every fed move of the tool centre at a cut level keeps a distance of at
//! least `r - TOL` from every edge of the part outline. The fix flattens
//! each arc join to points ON the arc, with the one flatten rule
//! (`FlattenPolicy::untoleranced`). A chord of that flattening lies inside
//! the arc by its sagitta, and cavalier's `arcs_to_approx_lines` sets the
//! sagitta to the policy deviation. So `TOL` is
//! `FlattenPolicy::UNTOLERANCED_MM` (0.010 mm) plus float slack.
//!
//! Each arm also checks the side: the cut stays outside the part for
//! Profile Outside and inside the outline for Profile Inside and Pocket.
//!
//! # Red proof on HEAD (before the fix)
//!
//! Tool radius 3, 90 degree corners: the expected intrusion is
//! `3 (1 - cos 45 deg)` = 0.879 mm at each arc join. The P6.0 report holds
//! the measured value of each arm.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::sync::atomic::AtomicBool;

use common::op_fixtures::no_dressups;
use common::session::{polygon_model, single_op_session_with};
use common::tools::endmill_tool_config;
use rs_cam_core::compute::StockConfig;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::floor::Floor;
use rs_cam_core::compute::operation_configs::{PocketConfig, PocketPattern, ProfileConfig};
use rs_cam_core::geo::P2;
use rs_cam_core::interrupt::NeverCancel;
use rs_cam_core::ops::pocket::pocket_contours_reported_with_cancel;
use rs_cam_core::ops::profile::{ProfileSide, profile_contour_reported};
use rs_cam_core::polygon::{FlattenPolicy, Polygon2};
use rs_cam_core::toolpath::{MoveType, Toolpath};

/// Tool diameter (mm): radius 3.
const TOOL_D: f64 = 6.0;
const R: f64 = TOOL_D / 2.0;
/// Cut depth (mm). One level: `depth_per_pass` = `DEPTH`.
const DEPTH: f64 = 3.0;
/// The allowed intrusion (mm): the flatten deviation of the one flatten
/// rule (`FlattenPolicy::UNTOLERANCED_MM`, `polygon.rs`), plus float slack
/// for the offset arithmetic.
const TOL: f64 = FlattenPolicy::UNTOLERANCED_MM + 1e-9;

/// A 200 x 200 x 10 mm board centred on X0 Y0, its top at Z0.
fn board() -> StockConfig {
    StockConfig {
        x: 200.0,
        y: 200.0,
        z: 10.0,
        origin_x: -100.0,
        origin_y: -100.0,
        origin_z: -10.0,
        auto_from_model: false,
        ..StockConfig::default()
    }
}

/// The 40 x 20 mm part: four convex 90 degree corners.
fn part() -> Polygon2 {
    Polygon2::rectangle(-20.0, -10.0, 20.0, 10.0)
}

/// An L-shaped outline, CCW, with ONE reflex 90 degree corner at (15, 15).
fn l_outline() -> Polygon2 {
    Polygon2::new(vec![
        P2::new(0.0, 0.0),
        P2::new(40.0, 0.0),
        P2::new(40.0, 15.0),
        P2::new(15.0, 15.0),
        P2::new(15.0, 40.0),
        P2::new(0.0, 40.0),
    ])
}

/// A 50 x 40 mm pocket with a 10 x 10 mm island: four convex corners on
/// the island. The hole is CW, as `Polygon2` holds holes.
fn island_pocket() -> Polygon2 {
    Polygon2::with_holes(
        Polygon2::rectangle(0.0, 0.0, 50.0, 40.0).exterior,
        vec![vec![
            P2::new(20.0, 15.0),
            P2::new(20.0, 25.0),
            P2::new(30.0, 25.0),
            P2::new(30.0, 15.0),
        ]],
    )
}

fn profile(side: ProfileSide) -> OperationConfig {
    OperationConfig::Profile(ProfileConfig {
        side,
        floor: Floor::top(DEPTH),
        depth_per_pass: DEPTH,
        tab_count: 0,
        finishing_passes: 0,
        ..ProfileConfig::default()
    })
}

fn pocket() -> OperationConfig {
    OperationConfig::Pocket(PocketConfig {
        pattern: PocketPattern::Contour,
        stepover: 2.0,
        floor: Floor::top(DEPTH),
        depth_per_pass: DEPTH,
        finishing_passes: 0,
        ..PocketConfig::default()
    })
}

/// The op output through the session door, with every dressup off.
fn generate(outline: Polygon2, op: OperationConfig) -> Toolpath {
    let mut session = single_op_session_with(
        board(),
        endmill_tool_config(TOOL_D),
        polygon_model(vec![outline], "p6_0_corner"),
        "p6_0",
        op,
        |tc| tc.dressups = no_dressups(),
    );
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(0, &cancel)
        .expect("the op generates");
    session
        .get_result(0)
        .expect("a result")
        .annotated()
        .toolpath
        .clone()
}

/// The XY segments of every fed move whose both ends sit below the stock
/// top (Z0): the cut. A plunge has no XY length and is skipped.
fn cut_segments(tp: &Toolpath) -> Vec<(P2, P2)> {
    let mut out = Vec::new();
    for pair in tp.moves.windows(2) {
        let (a, b) = (&pair[0], &pair[1]);
        if matches!(b.move_type, MoveType::Rapid) {
            continue;
        }
        assert!(
            matches!(b.move_type, MoveType::Linear { .. }),
            "the dressups are off, so the op emits no arc move"
        );
        if a.target.z >= -1e-9 || b.target.z >= -1e-9 {
            continue;
        }
        let (pa, pb) = (
            P2::new(a.target.x, a.target.y),
            P2::new(b.target.x, b.target.y),
        );
        if (pb - pa).norm() > 1e-12 {
            out.push((pa, pb));
        }
    }
    out
}

/// The closed ring as segments.
fn ring_segments(ring: &[P2]) -> Vec<(P2, P2)> {
    (0..ring.len())
        .map(|i| (ring[i], ring[(i + 1) % ring.len()]))
        .collect()
}

fn point_segment_distance(p: P2, a: P2, b: P2) -> f64 {
    let ab = b - a;
    let len_sq = ab.norm_squared();
    if len_sq <= 0.0 {
        return (p - a).norm();
    }
    let t = ((p - a).dot(&ab) / len_sq).clamp(0.0, 1.0);
    (p - (a + ab * t)).norm()
}

fn cross(o: P2, a: P2, b: P2) -> f64 {
    (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x)
}

/// Exact distance between two segments: zero when they cross, else the
/// least of the four end-point-to-segment distances.
fn segment_segment_distance(a: P2, b: P2, c: P2, d: P2) -> f64 {
    let (d1, d2) = (cross(c, d, a), cross(c, d, b));
    let (d3, d4) = (cross(a, b, c), cross(a, b, d));
    if d1 * d2 < 0.0 && d3 * d4 < 0.0 {
        return 0.0;
    }
    point_segment_distance(a, c, d)
        .min(point_segment_distance(b, c, d))
        .min(point_segment_distance(c, a, b))
        .min(point_segment_distance(d, a, b))
}

/// The worst intrusion (mm) of the cut into the band of radius `R` round
/// the outline edges, and the cut point nearest to an edge.
fn worst_intrusion(cut: &[(P2, P2)], outline: &Polygon2) -> (f64, P2) {
    let mut edges = ring_segments(&outline.exterior);
    for hole in &outline.holes {
        edges.extend(ring_segments(hole));
    }
    let mut worst = (f64::NEG_INFINITY, P2::new(f64::NAN, f64::NAN));
    for &(a, b) in cut {
        for &(c, d) in &edges {
            let intrusion = R - segment_segment_distance(a, b, c, d);
            if intrusion > worst.0 {
                let near = if point_segment_distance(a, c, d) <= point_segment_distance(b, c, d) {
                    a
                } else {
                    b
                };
                worst = (intrusion, near);
            }
        }
    }
    worst
}

/// The rule: the cut is on its side of the outline, and no cut point comes
/// nearer than `R - TOL` to an outline edge.
fn assert_clear(arm: &str, cut: &[(P2, P2)], outline: &Polygon2, outside: bool) {
    assert!(!cut.is_empty(), "{arm}: the op cut nothing");
    for &(a, _) in cut {
        assert_eq!(
            !outline.contains_point(&a),
            outside,
            "{arm}: the cut point ({:.4}, {:.4}) is on the wrong side of the outline",
            a.x,
            a.y
        );
    }
    let (intrusion, near) = worst_intrusion(cut, outline);
    assert!(
        intrusion <= TOL,
        "{arm}: the tool centre cuts {intrusion:.4} mm inside the corner band near ({:.4}, \
         {:.4}); the bound is {TOL:.4} mm (FlattenPolicy::UNTOLERANCED_MM). Expected on HEAD \
         before P6.0: R (1 - cos 45 deg) = {:.4} mm.",
        near.x,
        near.y,
        R * (1.0 - std::f64::consts::FRAC_PI_4.cos()),
    );
}

#[test]
fn profile_outside_kernel_keeps_the_radius_at_every_convex_corner() {
    let outline = part();
    let (ring, failures) = profile_contour_reported(&outline, R, ProfileSide::Outside);
    assert_eq!(failures, 0);
    let ring = ring.expect("the outside offset of a rectangle is one ring");
    assert_clear(
        "profile outside (kernel)",
        &ring_segments(&ring),
        &outline,
        true,
    );
}

#[test]
fn profile_outside_generated_keeps_the_radius_at_every_convex_corner() {
    let outline = part();
    let tp = generate(outline.clone(), profile(ProfileSide::Outside));
    assert_clear(
        "profile outside (generated)",
        &cut_segments(&tp),
        &outline,
        true,
    );
}

#[test]
fn profile_inside_generated_keeps_the_radius_at_a_reflex_corner() {
    let outline = l_outline();
    let tp = generate(outline.clone(), profile(ProfileSide::Inside));
    assert_clear(
        "profile inside, L (generated)",
        &cut_segments(&tp),
        &outline,
        false,
    );
}

#[test]
fn pocket_generated_keeps_the_radius_at_a_reflex_corner() {
    let outline = l_outline();
    let tp = generate(outline.clone(), pocket());
    assert_clear("pocket, L (generated)", &cut_segments(&tp), &outline, false);
}

#[test]
fn pocket_kernel_keeps_the_radius_round_an_island() {
    let outline = island_pocket();
    let (rings, _report) = pocket_contours_reported_with_cancel(&outline, R, 2.0, &NeverCancel)
        .expect("never cancelled");
    assert!(!rings.is_empty(), "the pocket has rings");
    let cut: Vec<(P2, P2)> = rings
        .iter()
        .flat_map(|r| ring_segments(&r.points))
        .collect();
    assert_clear("pocket round an island (kernel)", &cut, &outline, false);
}
