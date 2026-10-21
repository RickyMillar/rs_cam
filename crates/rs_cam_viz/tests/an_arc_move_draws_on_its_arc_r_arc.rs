//! R-ARC (operator report, 2026-10-21): "on the small circles the sim looks
//! good. but I see no circles on the toolpath? they look like zigzags in the
//! toolpath render".
//!
//! The toolpath renderer drew a G2/G3 move as one chord from the previous
//! target to the move target. A full circle (start = end) drew as a point,
//! and a half arc drew as its diameter. The simulator walks the arc with
//! core `linearize_arc_into`. The renderer and the pick now walk the same
//! points (`move_draw_points_into`).
//!
//! The test builds a toolpath with a half arc (G2) and a full circle (G3).
//! It runs `ToolpathLines::normal`, the function that the GPU upload
//! (`ToolpathGpuData::from_toolpath`) reads, and `nearest_drawn_move`, the
//! function that the viewport pick reads.
//!
//! Red proof (run once, red, reverted): `move_draw_points_into` gave
//! `[from, target]` for an arc too (the chord rule). Arm 1 failed with
//! "move 2 draws 1 segments: the arc draws as a chord". Arm 3 failed with
//! "the click at the mid-point of move 2 picks Some((0, 12.218919))": the
//! nearest tested point was the target of the rapid move 0 at 12 px, not
//! the arc.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::geo::P3;
use rs_cam_core::toolpath::{Move, MoveIntent, MoveType, Toolpath};
use rs_cam_core::trace::toolpath_spans::AnnotatedToolpath;
use rs_cam_viz::interaction::picking::nearest_drawn_move;
use rs_cam_viz::render::camera::OrbitCamera;
use rs_cam_viz::render::toolpath_render::{ToolpathLines, arc_draw_deviation_mm};
use rs_cam_viz::state::viewport::SpanKindFilter;

/// The half arc: centre (0, 0), radius 10, at Z -1. G2 from (10, 0) to
/// (-10, 0) goes through (0, -10).
const HALF_ARC_MOVE: usize = 2;
const HALF_CENTRE: [f64; 2] = [0.0, 0.0];
const HALF_R: f64 = 10.0;
/// The full circle: centre (30, 0), radius 5, at Z -1. G3 from (35, 0)
/// back to (35, 0).
const CIRCLE_MOVE: usize = 6;
const CIRCLE_CENTRE: [f64; 2] = [30.0, 0.0];
const CIRCLE_R: f64 = 5.0;
const Z: f64 = -1.0;

/// The pick threshold of the viewport (`PICK_THRESHOLD_TOOLPATH`).
const PICK_PX: f32 = 15.0;
const VIEW_W: f32 = 1200.0;
const VIEW_H: f32 = 800.0;

fn mv(x: f64, y: f64, z: f64, move_type: MoveType) -> Move {
    Move {
        target: P3::new(x, y, z),
        move_type,
        intent: MoveIntent::Unknown,
    }
}

fn toolpath() -> Toolpath {
    let feed = 1000.0;
    let mut tp = Toolpath::new();
    tp.moves = vec![
        mv(10.0, 0.0, 5.0, MoveType::Rapid),
        mv(10.0, 0.0, Z, MoveType::Linear { feed_rate: feed }),
        mv(
            -10.0,
            0.0,
            Z,
            MoveType::ArcCW {
                i: -10.0,
                j: 0.0,
                feed_rate: feed,
            },
        ),
        mv(-10.0, 0.0, 5.0, MoveType::Rapid),
        mv(35.0, 0.0, 5.0, MoveType::Rapid),
        mv(35.0, 0.0, Z, MoveType::Linear { feed_rate: feed }),
        mv(
            35.0,
            0.0,
            Z,
            MoveType::ArcCCW {
                i: -5.0,
                j: 0.0,
                feed_rate: feed,
            },
        ),
        mv(35.0, 0.0, 5.0, MoveType::Rapid),
    ];
    tp
}

/// The cut vertices that move `i` (from 1) draws.
fn move_vertices(lines: &ToolpathLines, i: usize) -> Vec<[f32; 3]> {
    let end = lines.move_cut_counts[i - 1] as usize;
    let start = if i >= 2 {
        lines.move_cut_counts[i - 2] as usize
    } else {
        0
    };
    lines.cut[start..end].iter().map(|v| v.position).collect()
}

/// Arm 1 and 2: every drawn vertex of an arc lies on the arc, each chord
/// mid-point is within the deviation of the arc, and an arc draws more than
/// two points.
fn assert_draws_on_arc(lines: &ToolpathLines, i: usize, centre: [f64; 2], r: f64) -> usize {
    let dev = arc_draw_deviation_mm();
    // f32 vertices: allow 1e-4 mm of rounding.
    let eps = 1e-4;
    let verts = move_vertices(lines, i);
    let segments = verts.len() / 2;
    assert!(
        segments >= 2,
        "move {i} draws {segments} segments: the arc draws as a chord"
    );
    let radius = |p: [f32; 3]| {
        let (dx, dy) = (f64::from(p[0]) - centre[0], f64::from(p[1]) - centre[1]);
        (dx * dx + dy * dy).sqrt()
    };
    let mut worst_vertex = 0.0_f64;
    let mut worst_chord = 0.0_f64;
    for pair in verts.chunks(2) {
        for p in pair {
            worst_vertex = worst_vertex.max((radius(*p) - r).abs());
            assert!((f64::from(p[2]) - Z).abs() < eps, "move {i}: Z {}", p[2]);
        }
        let mid = [
            (pair[0][0] + pair[1][0]) * 0.5,
            (pair[0][1] + pair[1][1]) * 0.5,
            Z as f32,
        ];
        worst_chord = worst_chord.max(r - radius(mid));
    }
    assert!(
        worst_vertex < eps,
        "move {i}: a drawn vertex is {worst_vertex:.6} mm off the arc"
    );
    assert!(
        worst_chord <= dev + eps,
        "move {i}: a drawn chord is {worst_chord:.4} mm inside the arc (budget {dev} mm)"
    );
    segments
}

#[test]
fn an_arc_move_draws_on_its_arc_r_arc() {
    let annotated = AnnotatedToolpath::new(toolpath());
    let lines = ToolpathLines::normal(&annotated, 0, false, &SpanKindFilter::default(), usize::MAX);
    let dev = arc_draw_deviation_mm();

    let half = assert_draws_on_arc(&lines, HALF_ARC_MOVE, HALF_CENTRE, HALF_R);
    let circle = assert_draws_on_arc(&lines, CIRCLE_MOVE, CIRCLE_CENTRE, CIRCLE_R);

    // The full circle goes round: it reaches the far side, x = 25.
    let far = move_vertices(&lines, CIRCLE_MOVE)
        .iter()
        .map(|p| f64::from(p[0]))
        .fold(f64::INFINITY, f64::min);
    assert!(
        far < CIRCLE_CENTRE[0] - CIRCLE_R + dev,
        "the full circle stops at x {far:.3}: it does not go round"
    );

    // Bounded: no more chords than the deviation needs, plus one for the
    // ceiling. A chord over the angle a is r (1 - cos(a / 2)) inside the arc.
    let need = |sweep: f64, r: f64| (sweep / (2.0 * (1.0 - dev / r).acos())).ceil() as usize;
    let half_need = need(std::f64::consts::PI, HALF_R);
    let circle_need = need(std::f64::consts::TAU, CIRCLE_R);
    assert!(
        half <= half_need + 1,
        "the half arc draws {half} chords; the deviation needs {half_need}"
    );
    assert!(
        circle <= circle_need + 1,
        "the full circle draws {circle} chords; the deviation needs {circle_need}"
    );

    // A line still draws as one segment.
    assert_eq!(move_vertices(&lines, 1).len(), 2);
}

#[test]
fn a_click_on_an_arc_picks_the_arc_move_r_arc() {
    let tp = toolpath();
    let mut camera = OrbitCamera::new();
    camera.fit_to_bounds([-12.0, -12.0, -2.0], [37.0, 12.0, 6.0]);
    let project = |p: [f32; 3]| camera.project_to_screen(p, VIEW_W / VIEW_H, VIEW_W, VIEW_H);

    // The mid-point of each arc, far from its chord: (0, -10) on the half
    // arc (its chord passes through (0, 0)), and (25, 0) on the full circle
    // (its chord is the point (35, 0)).
    let cases = [
        (HALF_ARC_MOVE, [HALF_CENTRE[0], HALF_CENTRE[1] - HALF_R]),
        (CIRCLE_MOVE, [CIRCLE_CENTRE[0] - CIRCLE_R, CIRCLE_CENTRE[1]]),
    ];
    for (want, at) in cases {
        let cursor = project([at[0] as f32, at[1] as f32, Z as f32]).expect("the point is in view");
        let hit = nearest_drawn_move(&tp.moves, cursor, PICK_PX, project);
        assert_eq!(
            hit.map(|(j, _)| j),
            Some(want),
            "the click at the mid-point of move {want} picks {hit:?}"
        );
    }
}
