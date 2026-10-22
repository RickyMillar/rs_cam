//! P5b (geometry selection, operator rulings 2026-10-19): a Profile tab
//! leaves a bridge of `tab_width`, and keeps its full `tab_height` at any
//! depth per pass.
//!
//! The two defects before P5b:
//!
//! 1. The lift on the tool-centre path was `tab_width` long. The tool cuts
//!    one radius past each end of the lift, so the bridge was
//!    `tab_width − tool_diameter`: a 6 mm tab with a 6.35 mm tool left no
//!    bridge. Now the lift ends stand `tab_width + tool_diameter` apart
//!    (`dressup::tabs::tab_lift_length`, the chord rule).
//! 2. Only the final level lifted over a tab. A level above the final Z
//!    but below the tab top cut the tab down. Now every level below the
//!    tab top (`final floor + tab_height`) lifts to the tab top.
//!
//! The bridge is measured by geometry on the generated moves: the length
//! of the line between the two lift ends that no move below the tab top
//! reaches within one tool radius. That line is the narrowest part of the
//! bridge.
//!
//! Every case generates through the session door (`generate_toolpath`)
//! with the dressups off, so the moves are the Profile runner's output.

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
use rs_cam_core::geo::{P2, P3};
use rs_cam_core::ops::profile::ProfileSide;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::toolpath::{MoveType, Toolpath};

/// A 1/4 inch end mill (6.35 mm): with the default 6 mm tab the old lift
/// left no bridge.
const TOOL_D: f64 = 6.35;
const R: f64 = TOOL_D / 2.0;
/// The bridge width (mm), the default `tab_width`.
const W: f64 = 6.0;
/// Float slack for positions that come from offsets and interpolation (mm).
const EPS: f64 = 1e-6;
/// The sample step (mm) along the bridge line. A sampled interval end is
/// off by at most one step, so a measured width is within two steps of
/// the true width.
const STEP: f64 = 1e-3;
const BRIDGE_TOL: f64 = 2.0 * STEP;

/// A 400 x 400 x 20 mm board centred on X0 Y0, its top at Z0.
fn board() -> StockConfig {
    StockConfig {
        x: 400.0,
        y: 400.0,
        z: 20.0,
        origin_x: -200.0,
        origin_y: -200.0,
        origin_z: -20.0,
        auto_from_model: false,
        ..StockConfig::default()
    }
}

fn profile(tab_count: usize, depth: f64, depth_per_pass: f64, tab_height: f64) -> ProfileConfig {
    ProfileConfig {
        side: ProfileSide::Outside,
        floor: rs_cam_core::compute::floor::Floor::top(depth),
        depth_per_pass,
        tab_count,
        tab_width: W,
        tab_height,
        finishing_passes: 0,
        ..ProfileConfig::default()
    }
}

fn run(loops: Vec<Polygon2>, cfg: ProfileConfig) -> Toolpath {
    let mut session = single_op_session_with(
        board(),
        endmill_tool_config(TOOL_D),
        polygon_model(loops, "p5b_tabs"),
        "p5b",
        OperationConfig::Profile(cfg),
        |tc| tc.dressups = no_dressups(),
    );
    let cancel = AtomicBool::new(false);
    session.generate_toolpath(0, &cancel).expect("generates");
    session
        .result_at(0)
        .expect("a result")
        .annotated()
        .toolpath
        .clone()
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

/// A CCW circle of radius `r` in 5 degree facets (the arc step of the
/// drawing door, `selection::item::CIRCLE_ARC_STEP_DEG`).
fn circle(r: f64) -> Polygon2 {
    Polygon2::new(
        (0..72)
            .map(|k| {
                let a = (5.0 * k as f64).to_radians();
                P2::new(r * a.cos(), r * a.sin())
            })
            .collect(),
    )
}

/// The tabs cut from the level at `from_z`: the XY of each vertical fed
/// rise from `from_z` to `top`, paired with the next vertical fed descent
/// from `top` back to `from_z`.
fn lifts(tp: &Toolpath, from_z: f64, top: f64) -> Vec<(P2, P2)> {
    let mut out = Vec::new();
    let mut up: Option<P2> = None;
    for w in tp.moves.windows(2) {
        let (a, b) = (w[0].target, w[1].target);
        if !matches!(w[1].move_type, MoveType::Linear { .. }) || (a.x - b.x).hypot(a.y - b.y) > EPS
        {
            continue;
        }
        if (a.z - from_z).abs() < EPS && (b.z - top).abs() < EPS {
            up = Some(P2::new(a.x, a.y));
        }
        if (a.z - top).abs() < EPS
            && (b.z - from_z).abs() < EPS
            && let Some(u) = up.take()
        {
            out.push((u, P2::new(a.x, a.y)));
        }
    }
    out
}

/// The distance from `p` to the XY segment `a`–`b`.
fn dist_to_segment(p: P2, a: P3, b: P3) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len2 = dx * dx + dy * dy;
    let t = if len2 < 1e-24 {
        0.0
    } else {
        (((p.x - a.x) * dx + (p.y - a.y) * dy) / len2).clamp(0.0, 1.0)
    };
    (p.x - (a.x + t * dx)).hypot(p.y - (a.y + t * dy))
}

/// The bridge of the tab between `up` and `down`: the length of the line
/// `up`–`down` that no fed move below `top` reaches within one tool
/// radius. Both ends of such a move stand below `top`, so the tool is
/// in the tab material along all of it.
fn bridge_width(tp: &Toolpath, up: P2, down: P2, top: f64) -> f64 {
    let low: Vec<(P3, P3)> = tp
        .moves
        .windows(2)
        .filter(|w| matches!(w[1].move_type, MoveType::Linear { .. }))
        .map(|w| (w[0].target, w[1].target))
        .filter(|(a, b)| a.z < top - EPS && b.z < top - EPS)
        .collect();
    let len = (down.x - up.x).hypot(down.y - up.y);
    let n = (len / STEP).ceil() as usize;
    let uncut = (0..=n)
        .filter(|&k| {
            let t = k as f64 / n as f64;
            let p = P2::new(up.x + t * (down.x - up.x), up.y + t * (down.y - up.y));
            low.iter().all(|&(a, b)| dist_to_segment(p, a, b) > R)
        })
        .count();
    uncut as f64 * len / n as f64
}

/// (a) On a straight side the bridge is `tab_width`: a 6 mm tab and a
/// 6.35 mm tool. Before P5b the lift was 6 mm long and the bridge was 0.
#[test]
fn tabs_straight_bridge_is_the_tab_width_p5b() {
    let depth = 6.0;
    let tp = run(
        vec![rect(-40.0, -20.0, 40.0, 20.0)],
        profile(4, depth, depth, 2.0),
    );
    let (floor, top) = (-depth, -depth + 2.0);
    let tabs = lifts(&tp, floor, top);
    assert_eq!(tabs.len(), 4, "one tab per side: {tabs:?}");
    for (up, down) in tabs {
        let bridge = bridge_width(&tp, up, down, top);
        assert!(
            (bridge - W).abs() <= BRIDGE_TOL,
            "a bridge of {bridge} mm, not {W}, between {up:?} and {down:?}"
        );
    }
}

/// (c) On a curved side the bridge is `tab_width` too. A circle has no
/// straight side, so its tabs are spaced evenly on the arc. The lift ends
/// stand `tab_width + tool_diameter` apart (the chord rule), so the lift
/// along the arc is longer than on a straight: a lift of the straight
/// length leaves about 0.1 mm less bridge on this 28 mm centre radius.
#[test]
fn tabs_curved_bridge_is_the_tab_width_p5b() {
    let depth = 6.0;
    let tp = run(vec![circle(25.0)], profile(3, depth, depth, 2.0));
    let (floor, top) = (-depth, -depth + 2.0);
    let tabs = lifts(&tp, floor, top);
    assert_eq!(tabs.len(), 3, "the even spacing: {tabs:?}");
    for (up, down) in tabs {
        let bridge = bridge_width(&tp, up, down, top);
        assert!(
            (bridge - W).abs() <= BRIDGE_TOL,
            "a bridge of {bridge} mm, not {W}, between {up:?} and {down:?}"
        );
    }
}

/// (b) With `tab_height` = 3 × `depth_per_pass`, no move over a tab goes
/// below the tab top on any level. Levels -1 to -6, tab top -3: the
/// levels -4, -5 and -6 each lift over every tab. Before P5b only the
/// final level lifted, and the levels -4 and -5 cut the tab down to -5.
#[test]
fn tabs_keep_full_height_on_every_level_p5b() {
    let (depth, dpp) = (6.0, 1.0);
    let tab_height = 3.0 * dpp;
    let tp = run(
        vec![rect(-40.0, -20.0, 40.0, 20.0)],
        profile(4, depth, dpp, tab_height),
    );
    let floor = -depth;
    let top = floor + tab_height;
    let tabs = lifts(&tp, floor, top);
    assert_eq!(tabs.len(), 4, "one tab per side: {tabs:?}");
    // Each level below the tab top lifts over each tab.
    for level in [-4.0, -5.0, -6.0] {
        assert_eq!(
            lifts(&tp, level, top).len(),
            4,
            "the level at Z{level} lifts over every tab"
        );
    }
    // No point of any move inside a tab zone is below the tab top. A zone
    // is the open line between the lift ends, less the float slack.
    let mut inside = 0;
    for w in tp.moves.windows(2) {
        let (a, b) = (w[0].target, w[1].target);
        let n = ((a.x - b.x).hypot(a.y - b.y) / 0.05).ceil().max(1.0) as usize;
        for k in 0..=n {
            let t = k as f64 / n as f64;
            let p = P3::new(
                a.x + t * (b.x - a.x),
                a.y + t * (b.y - a.y),
                a.z + t * (b.z - a.z),
            );
            for &(up, down) in &tabs {
                let (dx, dy) = (down.x - up.x, down.y - up.y);
                let len = dx.hypot(dy);
                let s = ((p.x - up.x) * dx + (p.y - up.y) * dy) / len;
                let off = ((p.x - up.x) * dy - (p.y - up.y) * dx).abs() / len;
                if off < EPS && s > EPS && s < len - EPS {
                    inside += 1;
                    assert!(
                        p.z >= top - EPS,
                        "a move at Z{} inside the tab {up:?}..{down:?} (top Z{top})",
                        p.z
                    );
                }
            }
        }
    }
    assert!(inside > 0, "the check sampled points inside the tab zones");
}
