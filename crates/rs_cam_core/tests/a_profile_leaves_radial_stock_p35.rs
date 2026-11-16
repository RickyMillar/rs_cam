//! P35 part A: the radial stock to leave of a Profile
//! (`ProfileConfig::stock_to_leave_radial`), through the session door.
//!
//! The need (`planning/rivmap_block_job/LEAD_PROPOSAL_2026-11-15.md`): a
//! relief band that reaches 1.6 mm outside a closed outline with a 6 mm
//! tool, so the tool centre is 1.4 mm inside the outline.
//!
//! # The claims
//!
//! - (a) Side `On`, value -1.4: each fed point is 1.4 mm inside the square,
//!   and the simulated cut reaches 1.6 mm outside the line and 4.4 mm
//!   inside it. The cut is measured on the stored path and on the
//!   simulated stock.
//! - (b) Side `Outside`, value 0.5: the centre is 3.5 mm outside. Side
//!   `Inside`, value -1.6: the centre is 1.4 mm inside.
//! - (c) A ring drawn clockwise, and a Bottom setup, give the side of (a).
//! - (d) No key, and the value 0, give the same moves.
//! - (e) A loop that collapses to no ring refuses, an open path with a
//!   value refuses, and the text names the start point.
//! - (h) P35 D: a loop that splits cuts each ring and warns.
//! - (f) Controller compensation: the programmed ring is the loop moved by
//!   the value alone.
//! - (g) The tabs sit on the moved ring.
//!
//! Red proof: make `moves_the_ring` in `ops/profile.rs` return `false`;
//! (a), (b), (c), (e), (f) and (g) fail.
//!
//! ```text
//! cargo test -p rs_cam_core -q --test a_profile_leaves_radial_stock_p35
//! ```

#![expect(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

mod common;

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use common::op_fixtures::{no_dressups, simulate};
use common::session::{polygon_model, single_op_session_with};
use common::tools::endmill_tool_config;
use rs_cam_core::compute::StockConfig;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::{CompensationType, ProfileConfig};
use rs_cam_core::dexel_stock::TriDexelStock;
use rs_cam_core::geo::P2;
use rs_cam_core::ops::profile::ProfileSide;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{Command, ProjectSession, SetToolpathParamArgs};
use rs_cam_core::toolpath::{MoveType, Toolpath};
use serde_json::json;

/// Tool diameter and radius (mm).
const TOOL_D: f64 = 6.0;
const R: f64 = TOOL_D / 2.0;
/// Cut depth (mm), one level.
const DEPTH: f64 = 3.0;
/// The square: 40 x 40 mm, its low corner at (10, 0).
const X0: f64 = 10.0;
const Y0: f64 = 0.0;
const SIDE: f64 = 40.0;
/// The offset library puts a corner arc on points within 0.010 mm of the
/// arc (`polygon::offset_polygon`, P6.0), so a moved ring is within that
/// of the exact ring.
const RING_TOL: f64 = 0.011;

/// A 160 x 120 x 10 mm board centred on X0 Y0, its top at Z0.
fn board() -> StockConfig {
    StockConfig {
        x: 160.0,
        y: 120.0,
        z: Some(10.0),
        origin_x: -80.0,
        origin_y: -60.0,
        origin_z: -10.0,
        auto_from_model: false,
        ..StockConfig::default()
    }
}

/// The square, counter-clockwise or clockwise.
fn square(ccw: bool) -> Vec<P2> {
    let mut pts = vec![
        P2::new(X0, Y0),
        P2::new(X0 + SIDE, Y0),
        P2::new(X0 + SIDE, Y0 + SIDE),
        P2::new(X0, Y0 + SIDE),
    ];
    if !ccw {
        pts.reverse();
    }
    pts
}

fn profile(side: ProfileSide, leave: Option<f64>) -> ProfileConfig {
    ProfileConfig {
        side,
        floor: rs_cam_core::compute::floor::Floor::top(DEPTH),
        depth_per_pass: Some(DEPTH),
        stock_to_leave_radial: leave,
        ..ProfileConfig::default()
    }
}

fn session(lines: Vec<Polygon2>, cfg: ProfileConfig) -> ProjectSession {
    single_op_session_with(
        board(),
        endmill_tool_config(TOOL_D),
        polygon_model(lines, "p35_leave"),
        "p35",
        OperationConfig::Profile(cfg),
        |tc| tc.dressups = no_dressups(),
    )
}

/// The stored toolpath of the one op, or the refusal text.
fn run(lines: Vec<Polygon2>, cfg: ProfileConfig) -> Result<Toolpath, String> {
    let mut session = session(lines, cfg);
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(0, &cancel)
        .map_err(|e| e.to_string())?;
    Ok(session
        .result_at(0)
        .expect("a result")
        .annotated()
        .toolpath
        .clone())
}

/// The XY of each fed move at the cut depth.
fn cut_points(tp: &Toolpath) -> Vec<P2> {
    tp.moves
        .iter()
        .filter(|m| m.move_type != MoveType::Rapid && (m.target.z + DEPTH).abs() < 1e-6)
        .map(|m| P2::new(m.target.x, m.target.y))
        .collect()
}

/// The signed distance from `p` to the square line: negative inside. A
/// point outside past a corner reads the distance to the corner.
fn signed_to_square(p: P2) -> f64 {
    let dx = (X0 - p.x).max(p.x - (X0 + SIDE));
    let dy = (Y0 - p.y).max(p.y - (Y0 + SIDE));
    if dx > 0.0 && dy > 0.0 {
        dx.hypot(dy)
    } else {
        dx.max(dy)
    }
}

/// Every cut point of `tp` is `expected` mm from the square line (negative
/// inside), and the points go round the square (non-vacuity).
#[track_caller]
fn assert_centre_at(tp: &Toolpath, expected: f64, what: &str) {
    let points = cut_points(tp);
    assert!(points.len() >= 4, "{what}: {} cut points", points.len());
    for p in &points {
        let d = signed_to_square(*p);
        assert!(
            (d - expected).abs() < RING_TOL,
            "{what}: the cut point ({:.4}, {:.4}) is {d:.4} mm from the line; expected {expected}",
            p.x,
            p.y
        );
    }
    let (min_x, max_x) = points
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), p| {
            (lo.min(p.x), hi.max(p.x))
        });
    assert!(
        max_x - min_x > SIDE - 2.0 * R - 4.0,
        "{what}: the points span {:.2} mm in X: the pass goes round the square",
        max_x - min_x
    );
}

fn last_stock(session: &ProjectSession) -> Arc<TriDexelStock> {
    session
        .simulation_result()
        .and_then(|sim| sim.checkpoints.last().map(|c| Arc::clone(&c.mesh_stock)))
        .expect("the simulation holds a checkpoint")
}

/// (a) The claim of the brief.
#[test]
fn side_on_with_minus_1_4_cuts_1_6_outside_and_4_4_inside_p35() {
    let lines = vec![Polygon2::new(square(true))];
    let cfg = profile(ProfileSide::On, Some(-1.4));
    let tp = run(lines.clone(), cfg.clone()).expect("the op generates");
    assert_centre_at(&tp, -1.4, "side On, -1.4");

    // The simulated stock, on the middle of the left side (x = X0), away
    // from the corners. The band is from 1.6 mm outside the line to 4.4 mm
    // inside it. A probe is one cell (0.5 mm) clear of each band edge.
    let mut session = session(lines, cfg);
    let cancel = AtomicBool::new(false);
    let _ = session
        .generate_toolpath(0, &cancel)
        .expect("the op generates");
    simulate(&mut session);
    let stock = last_stock(&session);
    let cell = stock.z_grid.cell_size;
    let y = Y0 + SIDE / 2.0;
    let top_at = |x: f64| {
        let (row, col) = stock
            .z_grid
            .world_to_cell(x, y)
            .expect("the probe is on the grid");
        f64::from(stock.z_grid.top_z_at(row, col).expect("material"))
    };
    let cut = |x: f64| top_at(x) < -DEPTH + 0.1;
    let whole = |x: f64| top_at(x) > -0.1;
    // Outside the square is x < X0.
    assert!(
        cut(X0 - 1.6 + cell),
        "{cell} mm inside the outer band edge is cut"
    );
    assert!(
        whole(X0 - 1.6 - cell),
        "{cell} mm outside the outer band edge is not cut: the band reaches 1.6 mm outside"
    );
    assert!(
        cut(X0 + 4.4 - cell),
        "{cell} mm inside the inner band edge is cut"
    );
    assert!(
        whole(X0 + 4.4 + cell),
        "{cell} mm past the inner band edge is not cut: the band reaches 4.4 mm inside"
    );
}

/// (b) The two sides with a wall.
#[test]
fn outside_and_inside_move_by_the_radius_plus_the_value_p35() {
    let lines = || vec![Polygon2::new(square(true))];
    let out = run(lines(), profile(ProfileSide::Outside, Some(0.5))).expect("generates");
    assert_centre_at(&out, R + 0.5, "side Outside, 0.5");
    let inside = run(lines(), profile(ProfileSide::Inside, Some(-1.6))).expect("generates");
    assert_centre_at(&inside, -(R - 1.6), "side Inside, -1.6");
    // The values of today, for the reference.
    let plain_out = run(lines(), profile(ProfileSide::Outside, None)).expect("generates");
    assert_centre_at(&plain_out, R, "side Outside, no key");
}

/// (c) The winding of the drawn ring and the setup do not change the side.
#[test]
fn a_clockwise_ring_and_a_bottom_setup_keep_the_side_p35() {
    let cw = run(
        vec![Polygon2::new(square(false))],
        profile(ProfileSide::On, Some(-1.4)),
    )
    .expect("generates");
    assert_centre_at(&cw, -1.4, "side On, -1.4, a clockwise ring");

    // A Bottom setup mirrors the drawing. The square is not symmetric about
    // the stock centre line, so the test reads the distance to the mirrored
    // square: the centre must be 1.4 mm inside it.
    use common::op_fixtures::{CaseSpec, Fixture, Frame, Prior, case_session};
    use rs_cam_core::compute::catalog::OperationType;
    use rs_cam_core::compute::config::StockSource;
    let leave = -0.5;
    let OperationConfig::Profile(base) = OperationConfig::new_default(OperationType::Profile)
    else {
        panic!("a Profile");
    };
    let flat = |frame: Frame, leave: Option<f64>| {
        let (mut session, index) = case_session(CaseSpec {
            name: "p35_flip",
            op: OperationConfig::Profile(ProfileConfig {
                side: ProfileSide::On,
                stock_to_leave_radial: leave,
                ..base.clone()
            }),
            prior: Prior::None,
            stock_source: StockSource::Fresh,
            frame,
            fixture: Fixture::Standard,
            tool: None,
        });
        let cancel = AtomicBool::new(false);
        let _ = session
            .generate_toolpath(index, &cancel)
            .expect("the op generates");
        let tp = session
            .result_at(index)
            .expect("a result")
            .annotated()
            .toolpath
            .clone();
        let lowest = tp
            .moves
            .iter()
            .map(|m| m.target.z)
            .fold(f64::INFINITY, f64::min);
        tp.moves
            .iter()
            .filter(|m| m.move_type != MoveType::Rapid && (m.target.z - lowest).abs() < 1e-6)
            .map(|m| P2::new(m.target.x, m.target.y))
            .collect::<Vec<_>>()
    };
    for frame in [Frame::TopUp, Frame::Flipped] {
        let on_line = flat(frame, None);
        let moved = flat(frame, Some(leave));
        let area = |pts: &[P2]| Polygon2::new(pts.to_vec()).signed_area().abs();
        assert!(on_line.len() >= 3 && moved.len() >= 3, "{frame:?}: a ring");
        assert!(
            area(&moved) < area(&on_line) - 1.0,
            "{frame:?}: a negative value moves the ring INTO its own area: {:.2} mm2 on the \
             line, {:.2} mm2 moved",
            area(&on_line),
            area(&moved)
        );
    }
}

/// (d) No key and the value 0 give the moves of before the field.
#[test]
fn no_key_and_zero_cut_the_same_path_p35() {
    for side in [ProfileSide::Outside, ProfileSide::Inside, ProfileSide::On] {
        let lines = || vec![Polygon2::new(square(true))];
        let none = run(lines(), profile(side, None)).expect("generates");
        let zero = run(lines(), profile(side, Some(0.0))).expect("generates");
        assert_eq!(
            format!("{:?}", none.moves),
            format!("{:?}", zero.moves),
            "{side:?}"
        );
    }
    // No file gains the key on a save.
    let text = toml::to_string(&profile(ProfileSide::On, None)).expect("serialises");
    assert!(!text.contains("stock_to_leave_radial"), "{text}");
}

/// (e) The refusals.
#[test]
fn a_collapsed_loop_and_an_open_path_refuse_p35() {
    // A 2 mm square moved in by 1.4 mm has no ring.
    let tiny = vec![Polygon2::new(vec![
        P2::new(0.0, 0.0),
        P2::new(2.0, 0.0),
        P2::new(2.0, 2.0),
        P2::new(0.0, 2.0),
    ])];
    let text = run(tiny, profile(ProfileSide::On, Some(-1.4))).expect_err("refuses");
    assert!(
        text.contains("stock_to_leave_radial") && text.contains("no ring"),
        "{text}"
    );
    assert!(text.contains("(0.000, 0.000)"), "names the start: {text}");

    // An open path closes no area.
    let open = vec![Polygon2::open_path(vec![
        P2::new(-30.0, 20.0),
        P2::new(-30.0, -10.0),
        P2::new(-5.0, -10.0),
    ])];
    let text = run(open.clone(), profile(ProfileSide::On, Some(-1.4))).expect_err("refuses");
    assert!(
        text.contains("open path") && text.contains("(-30.000, 20.000)"),
        "{text}"
    );
    // With no value the open path cuts, as before.
    let tp = run(open, profile(ProfileSide::On, None)).expect("an open path cuts on the line");
    assert!(!cut_points(&tp).is_empty());
}

/// The side (mm) of each square of the dumbbell.
const DB: f64 = 20.0;

/// A dumbbell: two 20 mm squares joined by a 2 mm neck. Moved in by 1.4 mm
/// the neck closes and the loop splits into two rings.
fn dumbbell() -> Vec<P2> {
    let (lo, hi) = (DB / 2.0 - 1.0, DB / 2.0 + 1.0);
    [
        (0.0, 0.0),
        (DB, 0.0),
        (DB, lo),
        (2.0 * DB, lo),
        (2.0 * DB, 0.0),
        (3.0 * DB, 0.0),
        (3.0 * DB, DB),
        (2.0 * DB, DB),
        (2.0 * DB, hi),
        (DB, hi),
        (DB, DB),
        (0.0, DB),
    ]
    .iter()
    .map(|&(x, y)| P2::new(x, y))
    .collect()
}

/// The distance from `p` to the closed polyline `ring`.
fn to_ring(p: P2, ring: &[P2]) -> f64 {
    let n = ring.len();
    (0..n)
        .map(|i| {
            let (a, b) = (ring[i], ring[(i + 1) % n]);
            let (dx, dy) = (b.x - a.x, b.y - a.y);
            let len2 = dx * dx + dy * dy;
            let t = (((p.x - a.x) * dx + (p.y - a.y) * dy) / len2).clamp(0.0, 1.0);
            (p.x - (a.x + t * dx)).hypot(p.y - (a.y + t * dy))
        })
        .fold(f64::INFINITY, f64::min)
}

/// (h) P35 D (lead ruling 2026-11-16): a loop that the value splits does
/// not refuse. The op cuts each ring that the offset gives back, for side
/// `On` and side `Inside`, and one warning names the start point of the
/// loop and the ring count. The hand tabs of the loop go on its first
/// ring; each ring takes automatic tabs by its own sides.
#[test]
fn a_split_loop_cuts_each_ring_and_warns_p35d() {
    use rs_cam_core::compute::toolpath_stats::TabNote;
    use rs_cam_core::diagnostics::Severity;
    for (side, leave) in [(ProfileSide::On, -1.4), (ProfileSide::Inside, -1.6)] {
        let mut session = session(vec![Polygon2::new(dumbbell())], profile(side, Some(leave)));
        let cancel = AtomicBool::new(false);
        session
            .generate_toolpath(0, &cancel)
            .unwrap_or_else(|e| panic!("{side:?}: a split loop generates: {e}"));
        let result = session.result_at(0).expect("a result");
        let tp = result.annotated().toolpath.clone();

        // Each cut point is 1.4 mm inside the loop: no ring is a chord
        // across the neck, and no point is outside the rule.
        let points = cut_points(&tp);
        assert!(points.len() >= 4, "{side:?}: {} cut points", points.len());
        for p in &points {
            let d = to_ring(*p, &dumbbell());
            assert!(
                (d - 1.4).abs() < RING_TOL,
                "{side:?}: the cut point ({:.3}, {:.3}) is {d:.4} mm from the loop",
                p.x,
                p.y
            );
        }
        // Two rings: points in each square, none in the neck (X 20 to 40).
        let left = points.iter().filter(|p| p.x < DB).count();
        let right = points.iter().filter(|p| p.x > 2.0 * DB).count();
        assert!(
            left >= 4 && right >= 4,
            "{side:?}: {left} left, {right} right"
        );
        assert_eq!(left + right, points.len(), "{side:?}: no cut in the neck");
        // No fed move at the cut depth crosses the neck.
        let mut prev: Option<P2> = None;
        for m in &tp.moves {
            let here = P2::new(m.target.x, m.target.y);
            let at_depth = (m.target.z + DEPTH).abs() < 1e-6;
            if m.move_type != MoveType::Rapid
                && at_depth
                && let Some(from) = prev
            {
                assert!(
                    (from.x < 1.5 * DB) == (here.x < 1.5 * DB),
                    "{side:?}: a fed move crosses the neck"
                );
            }
            prev = at_depth.then_some(here);
        }

        // The warning: one note for the loop, with its start and count.
        let finding = result
            .stats
            .tab_placement
            .clone()
            .unwrap_or_else(|| panic!("{side:?}: the op records the split"));
        let splits: Vec<_> = finding
            .notes
            .iter()
            .filter_map(|n| match *n {
                TabNote::SplitLoop { start, rings, .. } => Some((start, rings)),
                TabNote::EvenSpacing { .. }
                | TabNote::FewerThanAsked { .. }
                | TabNote::OpenPath { .. }
                | TabNote::OffsetSplit { .. }
                | TabNote::LiftedWithinFlute { .. } => None,
            })
            .collect();
        assert_eq!(splits, vec![([0.0, 0.0], 2)], "{side:?}");
        let text = finding.message();
        assert!(
            text.contains("2 rings") && text.contains("X0.00 Y0.00"),
            "{side:?}: {text}"
        );
        let diagnostics = session
            .diagnose_toolpath_with_trace(0, None)
            .expect("diagnose");
        let warning = diagnostics
            .iter()
            .find(|d| d.id.as_str() == "geom.profile_loop_split")
            .unwrap_or_else(|| panic!("{side:?}: the split is a diagnostic"));
        assert_eq!(warning.severity, Severity::Caution, "{side:?}");
        assert!(
            warning.message.contains("2 rings") && warning.message.contains("X0.00 Y0.00"),
            "{side:?}: {}",
            warning.message
        );
        assert!(
            !diagnostics
                .iter()
                .any(|d| d.id.as_str() == "geom.tab_placement"),
            "{side:?}: a split is no tab remark"
        );

        // The tabs: the cut of each ring lifts over a tab. The tab top is
        // the floor plus the tab height (2 mm): a fed move there is a lift.
        let mut tabbed = self::session(
            vec![Polygon2::new(dumbbell())],
            ProfileConfig {
                tab_count: 2,
                tab_width: 2.0,
                ..profile(side, Some(leave))
            },
        );
        tabbed
            .generate_toolpath(0, &cancel)
            .unwrap_or_else(|e| panic!("{side:?}: the tabbed op generates: {e}"));
        let tab_top = -DEPTH + 2.0;
        let lifts: Vec<P2> = tabbed
            .result_at(0)
            .expect("a result")
            .annotated()
            .toolpath
            .moves
            .iter()
            .filter(|m| m.move_type != MoveType::Rapid && (m.target.z - tab_top).abs() < 1e-6)
            .map(|m| P2::new(m.target.x, m.target.y))
            .collect();
        let lifts_left = lifts.iter().filter(|p| p.x < DB).count();
        let lifts_right = lifts.iter().filter(|p| p.x > 2.0 * DB).count();
        assert!(
            lifts_left >= 2 && lifts_right >= 2,
            "{side:?}: {lifts_left} lift moves on the left ring, {lifts_right} on the right ring"
        );
        for p in &lifts {
            let d = to_ring(*p, &dumbbell());
            assert!((d - 1.4).abs() < RING_TOL, "{side:?}: a tab off its ring");
        }
    }
}

/// (f) Controller compensation: the programmed ring is the loop moved by
/// the value alone; the controller adds its radius.
#[test]
fn in_control_programs_the_loop_moved_by_the_value_p35() {
    let cfg = ProfileConfig {
        compensation: CompensationType::InControl,
        ..profile(ProfileSide::Outside, Some(0.5))
    };
    let tp = run(vec![Polygon2::new(square(true))], cfg).expect("generates");
    assert_centre_at(&tp, 0.5, "in control, Outside, 0.5");
}

/// (g) The tabs sit on the moved ring; the parameter door takes the key,
/// holds its range and clears it with `null`.
#[test]
fn the_tabs_follow_the_ring_and_the_door_takes_the_key_p35() {
    let mut session = session(
        vec![Polygon2::new(square(true))],
        ProfileConfig {
            tab_count: 4,
            ..profile(ProfileSide::On, None)
        },
    );
    let set = |session: &mut ProjectSession, value: serde_json::Value| {
        session
            .apply(Command::SetToolpathParam(SetToolpathParamArgs {
                index: 0,
                param: "stock_to_leave_radial".to_owned(),
                value,
            }))
            .map(|_| ())
            .map_err(|e| e.to_string())
    };
    set(&mut session, json!(-1.4)).expect("the door takes the key");
    let layout = session
        .tab_layout(&session.toolpath_configs()[0])
        .expect("the tab layout");
    assert!(layout.marks.len() >= 4, "{} tabs", layout.marks.len());
    for tab in &layout.marks {
        let d = signed_to_square(P2::new(tab.at[0], tab.at[1]));
        assert!(
            (d + 1.4).abs() < RING_TOL,
            "a tab sits {d:.4} mm from the line; the ring is 1.4 mm inside"
        );
    }
    let text = set(&mut session, json!(10.5)).expect_err("outside the range");
    assert!(text.contains("stock_to_leave_radial"), "{text}");
    set(&mut session, serde_json::Value::Null).expect("null clears the key");
    let OperationConfig::Profile(cfg) = &session.toolpath_configs()[0].operation else {
        panic!("a Profile");
    };
    assert_eq!(cfg.stock_to_leave_radial, None);
}
