//! P5 (geometry selection, S12 and S13): Profile tabs sit on the straight
//! sides of each loop, and a hand tab snaps to its loop.
//!
//! Every case generates through the session door (`generate_toolpath`)
//! with the dressups off, so the moves are the Profile runner's output.
//! One level: `depth_per_pass` = `depth`, so the one pass is the final Z
//! and carries the tabs.
//!
//! The rules under test (`dressup::tabs`):
//!
//! 1. One tab per straight side that holds one; the count is a minimum,
//!    and extra tabs go to the longest sides (`auto_tabs`).
//! 2. The lift of a tab keeps `tab_width + tool_radius` from each corner
//!    (`tab_corner_clearance`). The lift is the bridge `tab_width` plus
//!    the tool diameter (`straight_tab_lift`, P5b).
//! 3. A side can wrap past the loop start (`straight_sides`).
//! 4. A loop with no side that holds a tab (a circle) takes the even
//!    spacing by length, and a `TabPlacementFinding` says so.
//! 5. A hand tab snaps to the nearest loop and replaces the automatic tabs
//!    of that loop only; a hand tab farther than one tool diameter from
//!    every loop refuses the op and names the point.
//! 6. The lifts are those of `apply_tabs` on the cut ring for the same tab
//!    points, and the ring starts in a gap, so every tab is whole.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::sync::atomic::AtomicBool;

use serde_json::json;

use common::op_fixtures::no_dressups;
use common::session::{polygon_model, single_op_session_with};
use common::tools::endmill_tool_config;
use rs_cam_core::compute::StockConfig;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::ProfileConfig;
use rs_cam_core::compute::toolpath_stats::{TabNote, TabPlacementFinding};
use rs_cam_core::dressup::apply_tabs;
use rs_cam_core::dressup::tabs::{straight_tab_lift, tab_corner_clearance, tabbed_ring};
use rs_cam_core::geo::P2;
use rs_cam_core::ops::profile::{
    ProfileParams, ProfileSide, profile_path_reported, profile_ring, profile_ring_to_toolpath,
};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{Command, SetToolpathParamArgs};
use rs_cam_core::toolpath::{MoveType, Toolpath};

/// Tool diameter (mm): radius 3.
const TOOL_D: f64 = 6.0;
const R: f64 = TOOL_D / 2.0;
/// Tab width and height (mm).
const W: f64 = 6.0;
const H: f64 = 2.0;
/// Cut depth (mm): the final Z is -DEPTH.
const DEPTH: f64 = 6.0;
const Z: f64 = -DEPTH;
/// Float slack for positions that come from offsets (mm).
const EPS: f64 = 1e-6;

/// A 800 x 800 x 10 mm board centred on X0 Y0, its top at Z0. Large
/// enough for the 300 mm circle.
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

/// The op output and its stats, or the refusal text.
fn run(
    loops: Vec<Polygon2>,
    cfg: ProfileConfig,
) -> Result<(Toolpath, Option<TabPlacementFinding>), String> {
    let mut session = single_op_session_with(
        board(),
        endmill_tool_config(TOOL_D),
        polygon_model(loops, "p5_tabs"),
        "p5",
        OperationConfig::Profile(cfg),
        |tc| tc.dressups = no_dressups(),
    );
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(0, &cancel)
        .map_err(|e| e.to_string())?;
    let result = session.get_result(0).expect("a result");
    Ok((
        result.annotated().toolpath.clone(),
        result.stats.tab_placement.clone(),
    ))
}

/// One tab as cut: the XY of the lift onto it and of the descent off it,
/// and the length (mm) the tool travels on the tab top between them.
#[derive(Debug, Clone, Copy)]
struct CutTab {
    up: P2,
    down: P2,
    len: f64,
}

impl CutTab {
    fn centre(&self) -> P2 {
        P2::new(
            (self.up.x + self.down.x) / 2.0,
            (self.up.y + self.down.y) / 2.0,
        )
    }
}

/// The tabs of the moves: a vertical fed rise from Z to Z + H, then the
/// vertical fed descent back to Z.
fn cut_tabs(tp: &Toolpath) -> Vec<CutTab> {
    let mut out = Vec::new();
    let mut up: Option<P2> = None;
    let mut len = 0.0;
    for w in tp.moves.windows(2) {
        let (a, b) = (w[0].target, w[1].target);
        if !matches!(w[1].move_type, MoveType::Linear { .. }) {
            continue;
        }
        let vertical = (a.x - b.x).hypot(a.y - b.y) < EPS;
        if (a.z - (Z + H)).abs() < EPS && (b.z - (Z + H)).abs() < EPS {
            len += (a.x - b.x).hypot(a.y - b.y);
        }
        if vertical && (a.z - Z).abs() < EPS && (b.z - (Z + H)).abs() < EPS {
            up = Some(P2::new(a.x, a.y));
            len = 0.0;
        }
        if vertical && (a.z - (Z + H)).abs() < EPS && (b.z - Z).abs() < EPS {
            let down = P2::new(a.x, a.y);
            out.push(CutTab {
                up: up.take().expect("a descent follows a lift"),
                down,
                len,
            });
        }
    }
    assert!(up.is_none(), "every lift has its descent");
    out
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

/// Which side of the outside-offset contour of the axis rectangle
/// `x0..x1 × y0..y1` holds `p`, as "bottom", "right", "top", "left".
fn side_of(p: P2, x0: f64, y0: f64, x1: f64, y1: f64) -> &'static str {
    let near = |a: f64, b: f64| (a - b).abs() < EPS;
    if near(p.y, y0 - R) {
        "bottom"
    } else if near(p.x, x1 + R) {
        "right"
    } else if near(p.y, y1 + R) {
        "top"
    } else if near(p.x, x0 - R) {
        "left"
    } else {
        panic!("tab point {p:?} is on no straight side")
    }
}

/// The lift of a tab on a straight side (P5b): the bridge `W` plus the
/// tool diameter.
const LIFT: f64 = W + TOOL_D;

/// Every tab is whole: the ends of its lift stand `LIFT` apart (the chord
/// rule of `tab_lift_length`, P5b), and on a straight side the tool
/// travels that far on the tab top.
fn assert_whole(tabs: &[CutTab]) {
    for t in tabs {
        let chord = (t.up.x - t.down.x).hypot(t.up.y - t.down.y);
        assert!(
            (chord - LIFT).abs() < 1e-6,
            "a lift whose ends are {chord} mm apart, not {LIFT}: {t:?}"
        );
        assert!(t.len >= chord - 1e-6, "{t:?}");
    }
}

/// Rule 1 and rule 2 on a rectangle: each of the four sides holds one tab
/// (a count of 2 is a minimum), both ends of each tab on the side, and each
/// tab edge at least the clearance from the outline corners.
#[test]
fn tabs_one_per_side_on_a_rectangle_p5() {
    let (x0, y0, x1, y1) = (-40.0, -20.0, 40.0, 20.0);
    let (tp, finding) = run(vec![rect(x0, y0, x1, y1)], profile(2, vec![])).unwrap();
    let tabs = cut_tabs(&tp);
    assert_whole(&tabs);
    let mut sides: Vec<&str> = tabs
        .iter()
        .map(|t| {
            let s = side_of(t.up, x0, y0, x1, y1);
            assert_eq!(s, side_of(t.down, x0, y0, x1, y1), "a tab spans a corner");
            s
        })
        .collect();
    sides.sort_unstable();
    assert_eq!(sides, ["bottom", "left", "right", "top"], "{tabs:?}");
    assert_eq!(finding, None, "every loop followed the side rule");
}

/// Rule 1: a D shape (three straight sides and a half circle of 5 degree
/// facets) takes one tab on each straight side and none on the arc.
#[test]
fn tabs_one_per_side_on_a_d_shape_p5() {
    // Straights: bottom (-30,-20)..(20,-20), the arc round (20,0) with
    // radius 20, top (20,20)..(-30,20), left back down.
    let mut pts = vec![P2::new(-30.0, -20.0), P2::new(20.0, -20.0)];
    for k in 1..36 {
        let a = (-90.0 + 5.0 * k as f64).to_radians();
        pts.push(P2::new(20.0 + 20.0 * a.cos(), 20.0 * a.sin()));
    }
    pts.push(P2::new(20.0, 20.0));
    pts.push(P2::new(-30.0, 20.0));
    let (tp, finding) = run(vec![Polygon2::new(pts)], profile(1, vec![])).unwrap();
    let tabs = cut_tabs(&tp);
    assert_whole(&tabs);
    let mut sides: Vec<&str> = tabs
        .iter()
        .map(|t| {
            let c = t.centre();
            assert!(c.x <= 20.0 + EPS, "a tab on the arc: {t:?}");
            side_of(c, -30.0, -20.0, 20.0, 20.0)
        })
        .collect();
    sides.sort_unstable();
    assert_eq!(sides, ["bottom", "left", "top"], "{tabs:?}");
    assert_eq!(finding, None);
}

/// Rule 1: tabs beyond one per side go to the longest sides. A 120 x 40
/// rectangle with a count of 6: one per side, then one more on each long
/// side.
#[test]
fn tabs_extra_count_goes_to_the_longest_sides_p5() {
    let (x0, y0, x1, y1) = (-60.0, -20.0, 60.0, 20.0);
    let (tp, _) = run(vec![rect(x0, y0, x1, y1)], profile(6, vec![])).unwrap();
    let tabs = cut_tabs(&tp);
    assert_whole(&tabs);
    let count = |s: &str| {
        tabs.iter()
            .filter(|t| side_of(t.centre(), x0, y0, x1, y1) == s)
            .count()
    };
    assert_eq!(
        [count("bottom"), count("right"), count("top"), count("left")],
        [2, 1, 2, 1],
        "{tabs:?}"
    );
}

/// Rule 2: a side shorter than `LIFT + 2 · clearance` (30 mm here) holds
/// no tab, and both ends of every lift keep the clearance from the outline
/// corners. The 26 mm sides lie between `LIFT + 2 · W` (24 mm, the
/// clearance without the tool radius) and 30 mm, so they prove the radius
/// term.
#[test]
fn tabs_keep_clear_of_corners_p5() {
    let (x0, y0, x1, y1) = (-40.0, -13.0, 40.0, 13.0);
    let clearance = tab_corner_clearance(W, R);
    assert!((clearance - (W + R)).abs() < 1e-12);
    assert!((straight_tab_lift(W, R) - LIFT).abs() < 1e-12);
    let (tp, finding) = run(vec![rect(x0, y0, x1, y1)], profile(2, vec![])).unwrap();
    let tabs = cut_tabs(&tp);
    assert_whole(&tabs);
    assert_eq!(tabs.len(), 2, "one tab per long side: {tabs:?}");
    for t in &tabs {
        let s = side_of(t.centre(), x0, y0, x1, y1);
        assert!(s == "bottom" || s == "top", "a tab on a short side: {t:?}");
        for p in [t.up, t.down] {
            assert!(
                p.x >= x0 + clearance - EPS && p.x <= x1 - clearance + EPS,
                "a tab edge at X{} is nearer than {clearance} to a corner",
                p.x
            );
        }
    }
    assert_eq!(finding, None, "the long sides hold the count");
}

/// Rule 3: an outline that starts in the middle of its bottom side. The
/// two collinear segments either side of the start are one side, so it
/// takes one tab at the centre of the whole side (X0), not two.
#[test]
fn tabs_side_wraps_past_the_loop_start_p5() {
    let outline = Polygon2::new(vec![
        P2::new(0.0, -20.0),
        P2::new(40.0, -20.0),
        P2::new(40.0, 20.0),
        P2::new(-40.0, 20.0),
        P2::new(-40.0, -20.0),
    ]);
    let (tp, _) = run(vec![outline], profile(1, vec![])).unwrap();
    let tabs = cut_tabs(&tp);
    assert_whole(&tabs);
    let bottom: Vec<&CutTab> = tabs
        .iter()
        .filter(|t| side_of(t.centre(), -40.0, -20.0, 40.0, 20.0) == "bottom")
        .collect();
    assert_eq!(bottom.len(), 1, "one side, one tab: {tabs:?}");
    assert!(bottom[0].centre().x.abs() < EPS, "{:?}", bottom[0]);
    assert_eq!(tabs.len(), 4);
}

/// Rule 4: a circle of 300 mm radius in 5 degree facets. Each facet is
/// 26 mm long, enough for a tab, but a facet of a flattened arc is not a
/// side. So the loop takes the even spacing, each tab whole, and the
/// finding names the loop.
#[test]
fn tabs_circle_falls_back_to_even_spacing_with_a_finding_p5() {
    let pts: Vec<P2> = (0..72)
        .map(|k| {
            let a = (5.0 * k as f64).to_radians();
            P2::new(300.0 * a.cos(), 300.0 * a.sin())
        })
        .collect();
    let (tp, finding) = run(vec![Polygon2::new(pts)], profile(3, vec![])).unwrap();
    let tabs = cut_tabs(&tp);
    assert_eq!(tabs.len(), 3, "the count, spaced evenly: {tabs:?}");
    assert_whole(&tabs);
    // Evenly spaced: the three centres are 120 degrees apart.
    let mut angles: Vec<f64> = tabs
        .iter()
        .map(|t| {
            t.centre()
                .y
                .atan2(t.centre().x)
                .to_degrees()
                .rem_euclid(360.0)
        })
        .collect();
    angles.sort_by(f64::total_cmp);
    for k in 0..3 {
        let gap = (angles[(k + 1) % 3] - angles[k]).rem_euclid(360.0);
        assert!((gap - 120.0).abs() < 0.5, "angles {angles:?}");
    }
    let finding = finding.expect("the fallback is named");
    assert_eq!(finding.op, "Profile");
    match finding.notes.as_slice() {
        [
            TabNote::EvenSpacing {
                loop_index: 0,
                count: 3,
                centre,
            },
        ] => assert!(centre[0].abs() < 1.0 && centre[1].abs() < 1.0, "{centre:?}"),
        other => panic!("one EvenSpacing note for loop 0: {other:?}"),
    }
}

/// Rule 5: a hand tab near the bottom side of the left rectangle snaps to
/// it and replaces its automatic tabs; the right rectangle keeps its four.
#[test]
fn tabs_hand_tab_snaps_to_its_loop_and_replaces_its_auto_tabs_p5() {
    let left = (-100.0, -20.0, -20.0, 20.0);
    let right = (20.0, -20.0, 100.0, 20.0);
    let (tp, _) = run(
        vec![
            rect(left.0, left.1, left.2, left.3),
            rect(right.0, right.1, right.2, right.3),
        ],
        // 1 mm below the outline, within one tool diameter.
        profile(1, vec![[-55.0, -21.0]]),
    )
    .unwrap();
    let tabs = cut_tabs(&tp);
    assert_whole(&tabs);
    let (on_left, on_right): (Vec<CutTab>, Vec<CutTab>) =
        tabs.iter().partition(|t| t.centre().x < 0.0);
    assert_eq!(on_left.len(), 1, "the hand tab only: {on_left:?}");
    let c = on_left[0].centre();
    assert!(
        (c.x + 55.0).abs() < EPS && (c.y - (left.1 - R)).abs() < EPS,
        "the hand tab sits on the cut of the bottom side at X-55: {c:?}"
    );
    assert_eq!(on_right.len(), 4, "the other loop keeps its auto tabs");
}

/// Rule 5: a hand tab farther than one tool diameter from every loop
/// refuses the op, and the message names the point.
#[test]
fn tabs_far_hand_tab_refuses_and_names_the_point_p5() {
    // The centre of the rectangle: 20 mm from the nearest side.
    let err = run(
        vec![rect(-40.0, -20.0, 40.0, 20.0)],
        profile(0, vec![[0.0, 0.0]]),
    )
    .expect_err("a far hand tab refuses");
    assert!(err.contains("X0.000 Y0.000"), "{err}");
    assert!(err.contains("tab_points"), "{err}");
}

/// Rule 6: the lifts of the op are those of `apply_tabs` on the cut ring,
/// built here from the public kernels for the same tab point.
#[test]
fn tabs_lifts_match_apply_tabs_for_the_same_points_p5() {
    let outline = rect(-40.0, -20.0, 40.0, 20.0);
    let point = [13.0, 20.5];
    let cfg = profile(0, vec![point]);
    let (tp, _) = run(vec![outline.clone()], cfg.clone()).unwrap();
    let got = cut_tabs(&tp);
    assert_eq!(got.len(), 1);

    let params = ProfileParams {
        tool_radius: R,
        side: cfg.side,
        cut_depth: Z,
        feed_rate: cfg.feed_rate,
        plunge_rate: cfg.plunge_rate,
        safe_z: 10.0,
        climb: cfg.climb,
        compensate_in_controller: false,
    };
    let contour = profile_path_reported(&outline, &params)
        .0
        .expect("a contour");
    let ring = profile_ring(&contour, params.climb);
    // The point snaps to the outline at (13, 20); the ring takes its
    // nearest point.
    let tabbed = tabbed_ring(&ring, &[P2::new(13.0, 20.0)], W, R);
    let pass = profile_ring_to_toolpath(&tabbed.ring, &params);
    let want = cut_tabs(&apply_tabs(pass, &tabbed.tabs(H), Z, Z));
    assert_eq!(want.len(), 1);
    for (g, w) in got.iter().zip(&want) {
        for (a, b) in [(g.up, w.up), (g.down, w.down)] {
            assert!(
                (a.x - b.x).abs() < EPS && (a.y - b.y).abs() < EPS,
                "op {g:?} vs apply_tabs {w:?}"
            );
        }
    }
    assert!((got[0].centre().x - 13.0).abs() < EPS, "{got:?}");
}

/// The hand tabs are a published W5 param: the setter writes a list of
/// points, the params read back as that list, and `null` clears it to an
/// empty list that reads back as `[]`, not `null`.
#[test]
fn tabs_hand_points_round_trip_through_the_param_setter_p5() {
    let mut session = single_op_session_with(
        board(),
        endmill_tool_config(TOOL_D),
        polygon_model(vec![rect(-40.0, -20.0, 40.0, 20.0)], "p5_tabs"),
        "p5",
        OperationConfig::Profile(profile(0, vec![])),
        |_| {},
    );
    let read = |session: &rs_cam_core::session::ProjectSession| {
        session
            .get_toolpath_params(0)
            .expect("the op")
            .params_value_including_nulls()
            .get("tab_points")
            .cloned()
    };
    assert_eq!(read(&session), Some(json!([])), "an empty list reads as []");
    let set = |session: &mut rs_cam_core::session::ProjectSession, value| {
        session.apply(Command::SetToolpathParam(SetToolpathParamArgs {
            index: 0,
            param: "tab_points".to_owned(),
            value,
        }))
    };
    let _ =
        set(&mut session, json!([[1.5, -20.0], [0.0, 20.0]])).expect("the setter writes the list");
    assert_eq!(read(&session), Some(json!([[1.5, -20.0], [0.0, 20.0]])));
    let _ = set(&mut session, serde_json::Value::Null).expect("null clears the list");
    assert_eq!(read(&session), Some(json!([])));
}
