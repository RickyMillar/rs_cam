//! PR-8d (H3): the minimum-segment floor on steep/shallow emission.
//!
//! Oracle: `planning/review_2026-07-29/CHECKPOINT_B_EVIDENCE.md` §8.1 —
//! "`steep_shallow` emits 0.9 µm cutting segments on the mixed-slope ribbon
//! at *every* resolution (`min seg 0.0009 mm`). Degenerate G-code, unrelated
//! to H3."
//!
//! ## Root cause, measured before it was fixed
//!
//! Probed on the four Checkpoint B fixtures × the four A/B arms:
//!
//! | fixture | shortest cutting segment | count < 1 mm | which half |
//! |---|---|---|---|
//! | mixed-slope ribbon | **0.000891 mm** | 2 | **steep (waterline)**, both |
//! | narrow ridge | 0.122003 mm | 0 | — |
//! | narrow valley | 0.500000 mm | 0 | — |
//! | patches + hole | 0.239323 mm | 0 | — |
//!
//! `0.000891` is bit-identical on all four arms, so the generation cell is
//! not the cause and a finer grid is not the cure. Both offenders sit inside
//! `SteepShallowSplit::steep`, i.e. the waterline pass, which emits
//! `contour_extract::weave_contours` vertices RAW.
//!
//! The source is marching squares placing each cell-edge vertex at an EXACT
//! fiber interval boundary (`find_interval_boundary_x`/`_y` return the
//! interval endpoint inside the edge span, not the edge midpoint), so two
//! adjacent cells whose boundaries resolve to the same crossing chain two
//! coincident-to-noise vertices.
//!
//! `waterline.rs` shares the source and the raw emission and is deliberately
//! NOT fixed here — one operation's blast radius per commit.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]

use rs_cam_core::geo::P3;
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::steep_shallow::{SteepShallowParams, steep_shallow_toolpath_split_with_cancel};
use rs_cam_core::tool::{MillingCutter, TaperedBallEndmill};
use rs_cam_core::toolpath::{MIN_EMITTED_SEGMENT_MM, Toolpath, drop_sub_minimum_segments};

fn taper() -> TaperedBallEndmill {
    TaperedBallEndmill::new(1.0, 7.0, 6.0, 25.0)
}

const HALF: f64 = 8.0;
const MESH_STEP: f64 = 0.25;

fn height_field(z: impl Fn(f64, f64) -> f64) -> TriangleMesh {
    let n = ((2.0 * HALF) / MESH_STEP).round() as usize + 1;
    let mut vertices = Vec::with_capacity(n * n);
    for j in 0..n {
        let y = -HALF + j as f64 * MESH_STEP;
        for i in 0..n {
            let x = -HALF + i as f64 * MESH_STEP;
            vertices.push(P3::new(x, y, z(x, y)));
        }
    }
    let mut triangles = Vec::with_capacity((n - 1) * (n - 1) * 2);
    for j in 0..(n - 1) {
        for i in 0..(n - 1) {
            let a = (j * n + i) as u32;
            let b = a + 1;
            let c = ((j + 1) * n + i) as u32;
            let d = c + 1;
            triangles.push([a, c, b]);
            triangles.push([b, c, d]);
        }
    }
    TriangleMesh::from_raw(vertices, triangles)
}

/// The Checkpoint B `mixed-slope ribbon`, verbatim — the only one of the four
/// fixtures that exhibits §8.1.
fn mixed_slope_ribbon() -> TriangleMesh {
    const KNOTS: [(f64, f64); 6] = [
        (-8.0, 0.0),
        (-3.0, 0.526),
        (-1.0, 4.0),
        (-0.65, 8.0),
        (0.65, 8.0),
        (8.0, 8.0 - 0.79),
    ];
    height_field(|x, _y| {
        if x <= KNOTS[0].0 {
            return KNOTS[0].1;
        }
        for w in KNOTS.windows(2) {
            let (x0, z0) = w[0];
            let (x1, z1) = w[1];
            if x <= x1 {
                let t = (x - x0) / (x1 - x0);
                return z0 + t * (z1 - z0);
            }
        }
        KNOTS[KNOTS.len() - 1].1
    })
}

fn params() -> SteepShallowParams {
    SteepShallowParams {
        threshold_angle: 45.0,
        stepover: 0.5,
        z_step: 0.5,
        tolerance: 0.10,
        ..Default::default()
    }
}

/// Every non-zero cutting-segment length in emitted order.
fn cutting_segments(tp: &Toolpath) -> Vec<f64> {
    let mut out = Vec::new();
    let mut prev: Option<P3> = None;
    for mv in &tp.moves {
        if let (true, Some(p)) = (mv.move_type.is_cutting(), prev) {
            let d = ((mv.target.x - p.x).powi(2)
                + (mv.target.y - p.y).powi(2)
                + (mv.target.z - p.z).powi(2))
            .sqrt();
            if d > 0.0 {
                out.push(d);
            }
        }
        prev = Some(mv.target);
    }
    out
}

fn run(mesh: &TriangleMesh, cutter: &dyn MillingCutter) -> Toolpath {
    let index = SpatialIndex::build(mesh, 2.0);
    let cancel = || false;
    let (tp, _split) =
        steep_shallow_toolpath_split_with_cancel(mesh, &index, cutter, &params(), None, &cancel)
            .expect("steep/shallow");
    tp
}

// ── 1. The floor's provenance ────────────────────────────────────────────

/// [`MIN_EMITTED_SEGMENT_MM`] is not a number anyone picked: it is the
/// coordinate quantum of the COARSEST shipped post. Derived from the post
/// TOMLs rather than restated, so adding a post with fewer XYZ decimals is a
/// red test and not a silent regression.
#[test]
fn the_floor_is_the_coarsest_shipped_post_quantum() {
    let posts_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("posts");
    let mut coarsest_quantum = 0.0_f64;
    let mut seen = 0usize;
    for entry in std::fs::read_dir(&posts_dir).expect("posts dir") {
        let path = entry.expect("dir entry").path();
        if path.extension().is_none_or(|e| e != "toml") {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("read post");
        // The `[decimals]` table's `xyz` key. Read textually on purpose: the
        // point is what the SHIPPED FILES say, and a parser change must not
        // be able to move this bound quietly.
        let Some(rest) = text.split("[decimals]").nth(1) else {
            continue;
        };
        let xyz: usize = rest
            .lines()
            .find_map(|l| l.trim().strip_prefix("xyz"))
            .and_then(|v| v.trim_start_matches([' ', '=']).trim().parse().ok())
            .expect("every post's [decimals] must declare xyz");
        seen += 1;
        coarsest_quantum = coarsest_quantum.max(10.0_f64.powi(-(xyz as i32)));
    }
    assert!(seen >= 4, "expected the four shipped posts, found {seen}");
    assert!(
        MIN_EMITTED_SEGMENT_MM >= coarsest_quantum - 1e-15,
        "the floor {MIN_EMITTED_SEGMENT_MM} mm is FINER than the coarsest \
         shipped post's coordinate quantum ({coarsest_quantum} mm), so a \
         segment can still round to a duplicate coordinate word"
    );
    println!(
        "{seen} shipped posts; coarsest XYZ quantum {coarsest_quantum} mm; \
         floor {MIN_EMITTED_SEGMENT_MM} mm"
    );
}

// ── 2. The helper's contract ─────────────────────────────────────────────

/// Greedy against the RETAINED point, not the raw predecessor — so a run of
/// pairwise-close points collapses to segments that each clear the floor,
/// instead of every point being dropped for being close to its immediate
/// neighbour and the run's real travel disappearing with them.
///
/// Ten 0.4 µm steps followed by a 1 mm jump: comparing each point to its raw
/// PREDECESSOR would drop all ten (each is 0.4 µm from the one before) and
/// silently lose the 4 µm the run actually covers. Comparing to the last
/// RETAINED point keeps one vertex per ~1 µm of real travel and preserves
/// the endpoints.
#[test]
fn the_floor_measures_against_the_last_retained_point() {
    let mut pts = vec![P3::new(0.0, 0.0, 0.0)];
    for i in 1..=10 {
        pts.push(P3::new(i as f64 * 0.0004, 0.0, 0.0));
    }
    pts.push(P3::new(1.0, 0.0, 0.0));
    let kept = drop_sub_minimum_segments(&pts, MIN_EMITTED_SEGMENT_MM, false);

    // THE contract: nothing survives closer than the floor to its neighbour.
    for w in kept.windows(2) {
        let gap = w[1].x - w[0].x;
        assert!(
            gap >= MIN_EMITTED_SEGMENT_MM,
            "a {gap:.6} mm gap survived the {MIN_EMITTED_SEGMENT_MM} mm floor"
        );
    }
    // The run collapsed (12 -> 5) but did not vanish: the 4 µm it covers is
    // still represented, and both endpoints are exact.
    assert!(
        kept.len() < pts.len() && kept.len() >= 3,
        "12 points -> {} is not a collapse: {kept:?}",
        kept.len()
    );
    assert!((kept.first().unwrap().x - 0.0).abs() < 1e-15);
    assert!((kept.last().unwrap().x - 1.0).abs() < 1e-15);
    let travel: f64 = kept.windows(2).map(|w| w[1].x - w[0].x).sum();
    assert!(
        (travel - 1.0).abs() < MIN_EMITTED_SEGMENT_MM,
        "travel {travel:.6} mm lost more than one floor"
    );
}

/// A closed contour chords back to its start, so a trailing vertex within
/// the floor of the FIRST point makes the CLOSING move the degenerate one —
/// which a straight consecutive-pair filter cannot see.
#[test]
fn a_closed_loop_drops_a_vertex_that_would_close_degenerately() {
    let square = vec![
        P3::new(0.0, 0.0, 0.0),
        P3::new(1.0, 0.0, 0.0),
        P3::new(1.0, 1.0, 0.0),
        P3::new(0.0, 1.0, 0.0),
        P3::new(0.0000004, 0.0, 0.0), // 0.4 µm from the start
    ];
    let open = drop_sub_minimum_segments(&square, MIN_EMITTED_SEGMENT_MM, false);
    assert_eq!(open.len(), 5, "as an OPEN path the last point is legitimate");
    let closed = drop_sub_minimum_segments(&square, MIN_EMITTED_SEGMENT_MM, true);
    assert_eq!(closed.len(), 4, "as a CLOSED loop it is the closing move");
}

// ── 3. The distribution, on the fixture §8.1 named ───────────────────────

/// The gate: **every** cutting segment on the offending fixture clears the
/// floor, and the total path length barely moves.
///
/// Both halves matter. A floor that removed real geometry would show up as a
/// path-length change; a floor that removed nothing would leave the minimum
/// where §8.1 found it. Recorded numbers, not just inequalities, so a future
/// reader can see the size of what was removed.
#[test]
fn no_cutting_segment_survives_below_the_floor_on_the_ribbon() {
    let tp = run(&mixed_slope_ribbon(), &taper());
    let segs = cutting_segments(&tp);
    assert!(segs.len() > 1000, "non-vacuity: {} segments", segs.len());
    let min = segs.iter().copied().fold(f64::INFINITY, f64::min);
    let total: f64 = segs.iter().sum();
    assert!(
        min >= MIN_EMITTED_SEGMENT_MM,
        "shortest cutting segment {min:.6} mm is below the {MIN_EMITTED_SEGMENT_MM} mm floor"
    );
    // §8.1's own number, as live red evidence of what was removed: without
    // the floor this fixture's minimum is 0.000891 mm.
    assert!(
        min > 0.0009 * 1.5,
        "the minimum is still within noise of §8.1's 0.000891 mm — the floor \
         is not reaching this emission site"
    );
    // The pre-PR-8d total was 4757.9 mm across 2723 segments (envelope/4
    // arm; this test runs the shipped default policy, which is the same for
    // this op). Removing two 0.9 µm segments is a 0.0018 mm change — four
    // parts in ten million.
    assert!(
        (total - 4757.9).abs() < 1.0,
        "total cutting length {total:.4} mm moved off the pre-PR-8d 4757.9 mm \
         by more than a millimetre — the floor removed real geometry"
    );
    println!(
        "mixed-slope ribbon: {} cutting segments, shortest {min:.6} mm, total \
         {total:.4} mm (pre-PR-8d: 2723 segments, shortest 0.000891 mm, total \
         4757.9 mm)",
        segs.len()
    );
}

/// The three fixtures that never exhibited §8.1 must be untouched: their
/// shortest segments are 0.122 / 0.500 / 0.239 mm, orders of magnitude above
/// the floor, so nothing may be dropped there. Without this, "the floor
/// helped" would be indistinguishable from "the floor is eating geometry".
#[test]
fn the_floor_is_inert_where_nothing_was_degenerate() {
    let ridge = height_field(|x, _y| 4.0 * (1.0 - x.abs()).max(0.0));
    let valley = height_field(|x, _y| 4.0 - 4.0 * (1.0 - x.abs()).max(0.0));
    for (name, mesh, recorded_min, recorded_total) in [
        ("narrow ridge", ridge, 0.122_003, 3795.2),
        ("narrow valley", valley, 0.500_000, 3625.5),
    ] {
        let segs = cutting_segments(&run(&mesh, &taper()));
        let min = segs.iter().copied().fold(f64::INFINITY, f64::min);
        let total: f64 = segs.iter().sum();
        assert!(
            (min - recorded_min).abs() < 1e-5,
            "{name}: shortest segment {min:.6} mm, was {recorded_min:.6} mm \
             before PR-8d — the floor must not have touched this fixture"
        );
        assert!(
            (total - recorded_total).abs() < 0.5,
            "{name}: total cutting length {total:.4} mm, was {recorded_total} mm"
        );
    }
}
