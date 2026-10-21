//! Geometry selection P4.6: the `tab_layout` query gives the tabs that
//! generation cuts.
//!
//! The query runs the selection resolution, `resolve_hand_tabs` and
//! `plan_profile_tabs` with no kernel; `ProfilePass` cuts from the same
//! plan. Each arm generates through the session door with the dressups
//! off and one level, then finds each lift in the moves: a vertical fed
//! rise from Z to Z + H, a run on the tab top, a vertical fed descent. The
//! lift centre is the midpoint of the rise and the descent (each tab of
//! these arms sits on a straight). The marks, moved into the setup frame
//! (`drawing_to_local`), must be those centres.
//!
//! Arms:
//!
//! 1. Auto tabs on the P5 rectangle and the P5 D shape.
//! 2. Hand tabs on a rectangle (they replace its auto tabs).
//! 3. A flipped and turned setup (Bottom, 90 degrees): the marks are in the
//!    drawing frame, the cut is in the setup frame.
//! 4. A far hand tab: the query refuses with the text of generation.
//! 5. L-5: the conversion of the auto tabs of a loop keeps the other tabs
//!    where they were and moves the one.
//! 6. O-4: the delete of the last tab of a loop is refused and names it.
//! 7. P6.3: a Profile with side `On` (it reads `Curves`) on a square and an
//!    open path. The editor accepts it: the marks are the lifts on the
//!    square, the loops name the square only (an open path is no loop).

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
use rs_cam_core::compute::operation_configs::ProfileConfig;
use rs_cam_core::compute::transform::{FaceUp, ZRotation};
use rs_cam_core::dressup::tabs::TabSource;
use rs_cam_core::geo::P2;
use rs_cam_core::ops::profile::ProfileSide;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::selection::ItemRef;
use rs_cam_core::session::{
    Command, ProjectSession, Query, QueryAnswer, SetSetupFaceArgs, SetSetupRotationArgs,
    SetToolpathParamArgs, TabEdit, TabLayoutAnswer, TabLayoutArgs, apply_tab_edit,
};
use rs_cam_core::toolpath::{MoveType, Toolpath};

/// Tool diameter (mm): radius 3.
const TOOL_D: f64 = 6.0;
/// Tab width and height (mm).
const W: f64 = 6.0;
const H: f64 = 2.0;
/// Cut depth (mm). The final Z is the lowest Z of the cut (a flipped
/// setup cuts in its own zero-rooted frame).
const DEPTH: f64 = 6.0;
/// Float slack for positions that come from offsets (mm).
const EPS: f64 = 1e-6;

/// An 800 x 800 x 10 mm board centred on X0 Y0, its top at Z0 (the P5
/// board).
fn board() -> StockConfig {
    StockConfig {
        x: 800.0,
        y: 800.0,
        z: 10.0,
        origin_x: -400.0,
        origin_y: -400.0,
        origin_z: -10.0,
        auto_from_model: false,
        ..StockConfig::default()
    }
}

fn profile(tab_count: usize, tab_points: Vec<[f64; 2]>) -> ProfileConfig {
    ProfileConfig {
        side: ProfileSide::Outside,
        floor: rs_cam_core::compute::floor::Floor::top(DEPTH),
        depth_per_pass: DEPTH,
        tab_count,
        tab_width: W,
        tab_height: H,
        tab_points,
        finishing_passes: 0,
        ..ProfileConfig::default()
    }
}

fn session(loops: Vec<Polygon2>, cfg: ProfileConfig) -> ProjectSession {
    single_op_session_with(
        board(),
        endmill_tool_config(TOOL_D),
        polygon_model(loops, "p4_6_tabs"),
        "p4_6",
        OperationConfig::Profile(cfg),
        |tc| tc.dressups = no_dressups(),
    )
}

fn layout(session: &ProjectSession) -> Result<TabLayoutAnswer, String> {
    match session.query(Query::TabLayout(TabLayoutArgs { index: 0 })) {
        Ok(QueryAnswer::TabLayout(answer)) => Ok(answer),
        Ok(other) => panic!("tab_layout answered {other:?}"),
        Err(e) => Err(e.to_string()),
    }
}

fn generate(session: &mut ProjectSession) -> Result<Toolpath, String> {
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(0, &cancel)
        .map_err(|e| e.to_string())?;
    Ok(session
        .get_result(0)
        .expect("a result")
        .annotated()
        .toolpath
        .clone())
}

/// The lift centres of the moves (setup frame): the midpoint of each
/// vertical fed rise from the final Z to the tab top (final Z + H) and the
/// descent after it.
fn lift_centres(tp: &Toolpath) -> Vec<P2> {
    #[allow(non_snake_case)]
    let Z = tp
        .moves
        .iter()
        .filter(|m| matches!(m.move_type, MoveType::Linear { .. }))
        .map(|m| m.target.z)
        .fold(f64::INFINITY, f64::min);
    let mut out = Vec::new();
    let mut up: Option<P2> = None;
    for w in tp.moves.windows(2) {
        let (a, b) = (w[0].target, w[1].target);
        if !matches!(w[1].move_type, MoveType::Linear { .. }) {
            continue;
        }
        let vertical = (a.x - b.x).hypot(a.y - b.y) < EPS;
        if vertical && (a.z - Z).abs() < EPS && (b.z - (Z + H)).abs() < EPS {
            up = Some(P2::new(a.x, a.y));
        }
        if vertical && (a.z - (Z + H)).abs() < EPS && (b.z - Z).abs() < EPS {
            let u = up.take().expect("a descent follows a lift");
            out.push(P2::new((u.x + a.x) / 2.0, (u.y + a.y) / 2.0));
        }
    }
    out
}

/// The marks of `answer` in the setup frame of the toolpath.
fn marks_in_setup(session: &ProjectSession, answer: &TabLayoutAnswer) -> Vec<P2> {
    let setup = &session.list_setups()[0];
    let info = session.setup_transform_info(setup.face_up, setup.z_rotation);
    // An identity setup moves no drawing point (`SetupEvalContext`).
    answer
        .marks
        .iter()
        .map(|m| P2::new(m.at[0], m.at[1]))
        .map(|p| {
            if info.needs_transform() {
                info.drawing_to_local(p)
            } else {
                p
            }
        })
        .collect()
}

/// Each lift centre has exactly one mark at it, and each mark one centre.
#[track_caller]
fn assert_same_points(marks: &[P2], cut: &[P2]) {
    assert_eq!(
        marks.len(),
        cut.len(),
        "the layout gives {} tabs and the cut {}: marks {marks:?} cut {cut:?}",
        marks.len(),
        cut.len()
    );
    for c in cut {
        let near = marks
            .iter()
            .filter(|m| (m.x - c.x).hypot(m.y - c.y) < EPS)
            .count();
        assert_eq!(
            near, 1,
            "the lift at {c:?} has {near} marks: marks {marks:?}"
        );
    }
}

/// A CCW rectangle from its lower-left corner.
fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Polygon2 {
    Polygon2::new(vec![
        P2::new(x0, y0),
        P2::new(x1, y0),
        P2::new(x1, y1),
        P2::new(x0, y1),
    ])
}

/// The P5 D shape: three straight sides and a half circle of 5 degree
/// facets round (20, 0), radius 20.
fn d_shape() -> Polygon2 {
    let mut pts = vec![P2::new(-30.0, -20.0), P2::new(20.0, -20.0)];
    for k in 1..36 {
        let a = (-90.0 + 5.0 * k as f64).to_radians();
        pts.push(P2::new(20.0 + 20.0 * a.cos(), 20.0 * a.sin()));
    }
    pts.push(P2::new(20.0, 20.0));
    pts.push(P2::new(-30.0, 20.0));
    Polygon2::new(pts)
}

/// Arm 1: the auto tabs of the rectangle and of the D shape.
#[test]
fn auto_tab_marks_are_the_cut_lifts_p4_6() {
    for (name, shape, count) in [
        ("rectangle", rect(-40.0, -20.0, 40.0, 20.0), 2),
        ("D shape", d_shape(), 1),
    ] {
        let mut s = session(vec![shape], profile(count, vec![]));
        let answer = layout(&s).unwrap();
        assert!(
            answer.marks.iter().all(|m| m.source == TabSource::Auto),
            "{name}: every mark is an auto tab"
        );
        assert_eq!(answer.loops.len(), 1, "{name}: one loop");
        let cut = lift_centres(&generate(&mut s).unwrap());
        assert!(!cut.is_empty(), "{name}: the cut has tabs");
        assert_same_points(&marks_in_setup(&s, &answer), &cut);
    }
}

/// Arm 2: hand tabs on a rectangle replace its auto tabs; each mark names
/// its point in `tab_points`.
#[test]
fn hand_tab_marks_are_the_cut_lifts_p4_6() {
    let points = vec![[0.0, 21.0], [-10.0, -20.0]];
    let mut s = session(vec![rect(-40.0, -20.0, 40.0, 20.0)], profile(2, points));
    let answer = layout(&s).unwrap();
    let mut sources: Vec<TabSource> = answer.marks.iter().map(|m| m.source).collect();
    sources.sort_by_key(|s| match s {
        TabSource::Hand(i) => *i,
        TabSource::Auto => usize::MAX,
    });
    assert_eq!(sources, vec![TabSource::Hand(0), TabSource::Hand(1)]);
    // A hand tab stores the point on the outline.
    let on_loop: Vec<[f64; 2]> = answer.marks.iter().map(|m| m.on_loop).collect();
    assert!(
        on_loop
            .iter()
            .any(|p| (p[0] - 0.0).abs() < EPS && (p[1] - 20.0).abs() < EPS)
    );
    let cut = lift_centres(&generate(&mut s).unwrap());
    assert_same_points(&marks_in_setup(&s, &answer), &cut);
}

/// Arm 3: a Bottom setup turned 90 degrees. The marks are drawing points;
/// moved into the setup frame they are the lifts of the cut, for auto
/// tabs and for hand tabs.
#[test]
fn marks_in_a_flipped_setup_are_the_cut_lifts_p4_6() {
    for cfg in [
        profile(2, vec![]),
        profile(2, vec![[30.0, 25.5], [90.5, 0.0]]),
    ] {
        let hand = !cfg.tab_points.is_empty();
        let mut s = session(vec![rect(10.0, -15.0, 90.0, 25.0)], cfg);
        let _face = s
            .apply(Command::SetSetupFace(SetSetupFaceArgs {
                setup_index: 0,
                face_up: FaceUp::Bottom,
            }))
            .unwrap();
        let _rotation = s
            .apply(Command::SetSetupRotation(SetSetupRotationArgs {
                setup_index: 0,
                z_rotation: ZRotation::Deg90,
            }))
            .unwrap();
        let answer = layout(&s).unwrap();
        // The drawing frame is not the setup frame here: a mark on the
        // outline is a point of the drawing rectangle.
        for m in &answer.marks {
            let [x, y] = m.on_loop;
            let on_side = ((x - 10.0).abs() < EPS || (x - 90.0).abs() < EPS)
                || ((y + 15.0).abs() < EPS || (y - 25.0).abs() < EPS);
            assert!(on_side, "hand={hand}: {m:?} is not on the drawn loop");
        }
        let cut = lift_centres(&generate(&mut s).unwrap());
        assert!(!cut.is_empty());
        assert_same_points(&marks_in_setup(&s, &answer), &cut);
    }
}

/// Arm 4: a hand tab far from every loop. The query refuses with the text
/// that generation gives.
#[test]
fn a_far_hand_tab_refuses_as_generation_refuses_p4_6() {
    let mut s = session(
        vec![rect(-40.0, -20.0, 40.0, 20.0)],
        profile(2, vec![[0.0, 60.0]]),
    );
    let query = layout(&s).expect_err("a far hand tab refuses the query");
    let generation = generate(&mut s).expect_err("a far hand tab refuses generation");
    assert_eq!(query, generation);
    assert!(query.contains("X0.000 Y60.000"), "{query}");
}

/// Write `tab_points` through the command door, as the editor does.
fn write_points(s: &mut ProjectSession, points: &[[f64; 2]]) {
    let _effects = s
        .apply(Command::SetToolpathParam(SetToolpathParamArgs {
            index: 0,
            param: "tab_points".to_owned(),
            value: serde_json::json!(points),
        }))
        .unwrap();
}

fn tab_points(s: &ProjectSession) -> Vec<[f64; 2]> {
    match &s.toolpath_configs()[0].operation {
        OperationConfig::Profile(cfg) => cfg.tab_points.clone(),
        other => panic!("not a Profile: {other:?}"),
    }
}

/// Arm 5 (L-5): the move of one auto tab of a loop writes every tab of
/// that loop as a hand tab. The other tabs stay where they were, and the
/// moved one is at its new point. The cut follows the new layout.
#[test]
fn an_auto_tab_move_keeps_the_other_tabs_of_its_loop_l5() {
    let mut s = session(
        vec![
            rect(-40.0, -20.0, 40.0, 20.0),
            rect(100.0, -20.0, 180.0, 20.0),
        ],
        profile(2, vec![]),
    );
    let before = layout(&s).unwrap();
    let first_loop = before.loops[0].clone();
    let moved = before
        .marks
        .iter()
        .position(|m| m.loop_key == first_loop)
        .unwrap();
    // A new point on the bottom side of the first loop, clear of corners.
    let to = [-12.0, -20.0];
    let points =
        apply_tab_edit(&before, &tab_points(&s), &TabEdit::Move { mark: moved, to }).unwrap();
    let on_first = before
        .marks
        .iter()
        .filter(|m| m.loop_key == first_loop)
        .count();
    assert_eq!(
        points.len(),
        on_first,
        "every tab of the loop is a hand tab"
    );
    write_points(&mut s, &points);
    let after = layout(&s).unwrap();
    // Same count on each loop.
    assert_eq!(after.marks.len(), before.marks.len());
    for (k, m) in before.marks.iter().enumerate() {
        let now: Vec<&_> = after
            .marks
            .iter()
            .filter(|n| n.loop_key == m.loop_key)
            .collect();
        if k == moved {
            assert!(
                now.iter()
                    .any(|n| (n.on_loop[0] - to[0]).abs() < EPS
                        && (n.on_loop[1] - to[1]).abs() < EPS),
                "the moved tab is at {to:?}: {now:?}"
            );
        } else {
            assert!(
                now.iter()
                    .any(|n| (n.at[0] - m.at[0]).abs() < EPS && (n.at[1] - m.at[1]).abs() < EPS),
                "the tab at {:?} stays: {now:?}",
                m.at
            );
        }
    }
    // The second loop keeps its auto tabs.
    assert!(
        after
            .marks
            .iter()
            .filter(|m| m.loop_key != first_loop)
            .all(|m| m.source == TabSource::Auto)
    );
    let cut = lift_centres(&generate(&mut s).unwrap());
    assert_same_points(&marks_in_setup(&s, &after), &cut);
}

/// Arm 6 (O-4): the delete of the last tab of a loop is refused, and the
/// text names the loop. The delete of a tab that is not the last works.
#[test]
fn the_last_tab_of_a_loop_is_not_deleted_o4() {
    let s = session(
        vec![rect(-40.0, -20.0, 40.0, 20.0)],
        profile(2, vec![[0.0, 20.0]]),
    );
    let answer = layout(&s).unwrap();
    assert_eq!(answer.marks.len(), 1, "one hand tab replaces the auto tabs");
    let refusal = apply_tab_edit(&answer, &tab_points(&s), &TabEdit::Delete { mark: 0 })
        .expect_err("the last tab of the loop");
    let name = ItemRef::new(answer.loops[0].clone()).to_string();
    assert!(refusal.contains(&name), "the text names {name}: {refusal}");

    let s = session(
        vec![rect(-40.0, -20.0, 40.0, 20.0)],
        profile(2, vec![[0.0, 20.0], [0.0, -20.0]]),
    );
    let answer = layout(&s).unwrap();
    let points = apply_tab_edit(&answer, &tab_points(&s), &TabEdit::Delete { mark: 0 })
        .expect("a loop with two tabs");
    assert_eq!(points.len(), 1);
}

/// Arm 7 (P6.3): a Profile with side `On` reads the lines of the selection
/// (`Curves`). The tab editor accepts it: the marks are the cut lifts, all
/// on the square, and the loop list holds the square only (an open path
/// takes no tab, L-8).
#[test]
fn a_side_on_profile_gives_the_tabs_of_its_closed_rings_p6_3() {
    let square = rect(-40.0, -20.0, 40.0, 20.0);
    let path = Polygon2::open_path(vec![P2::new(-60.0, 40.0), P2::new(60.0, 40.0)]);
    let cfg = ProfileConfig {
        side: ProfileSide::On,
        ..profile(2, vec![])
    };
    let mut s = session(vec![square, path], cfg);
    let answer = layout(&s).expect("the tab editor accepts a Profile with side On");
    assert_eq!(answer.loops.len(), 1, "one loop: the square");
    assert!(!answer.marks.is_empty(), "the square has tabs");
    for m in &answer.marks {
        let [x, y] = m.on_loop;
        let on_side = ((x.abs() - 40.0).abs() < EPS && y.abs() <= 20.0 + EPS)
            || ((y.abs() - 20.0).abs() < EPS && x.abs() <= 40.0 + EPS);
        assert!(on_side, "a mark is not on the square: {m:?}");
        assert_eq!(m.loop_key, answer.loops[0], "every mark names the square");
    }
    let cut = lift_centres(&generate(&mut s).unwrap());
    assert_same_points(&marks_in_setup(&s, &answer), &cut);
}
