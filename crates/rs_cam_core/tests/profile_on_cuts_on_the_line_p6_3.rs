//! P6.3 (geometry selection) — a Profile with side `On` cuts the tool
//! centre on the line.
//!
//! # The claim
//!
//! Through the session door (`generate_toolpath`):
//!
//! - (a) An open L and an open 2-point line are cut exactly on the drawn
//!   line, from the first point to the last point (the drawn direction).
//!   No fed move goes back towards the start (no chord between the ends),
//!   and no fed move leaves the line (no lead past an end normal). This
//!   holds with no dressups, with the default Profile dressups and a lead
//!   asked for, and with a ramp entry asked for (design risk "ramp entry
//!   on a short open path").
//! - (b) `climb` does not reverse an open path (L-1).
//! - (c) A closed square with side `On` is cut on its outline, and the
//!   pass closes. With the default dressups and a lead asked for, the
//!   square takes no lead: an `On` ring has material on both sides
//!   (`RingMaterial::BothSides`), so no side is free for the arc.
//! - (e) Side `On` with the floor "Each picked face" is refused at every
//!   door (`OpDomainRefusal::SideOnEachFace`, run by `validate_operation`):
//!   AddToolpath, the op change, `SetToolpathParam` (side), `SetToolpathFloor`,
//!   ReplaceToolpathConfig and load. A line has no face floor, and the one
//!   ladder would cut a shallower face below its own floor.
//! - (d) Tabs (L-8): with `tab_count > 0`, the square takes its tabs, the
//!   open L takes none, and ONE `TabNote::OpenPath` note names the L. A
//!   hand tab next to the L only is refused: a hand tab snaps to a closed
//!   ring only.
//!
//! Red proof (P6.3): send the open path through the closed emitter in
//! `ProfilePass::emit_level`; (a) fails on the chord.
//!
//! ```text
//! cargo test -p rs_cam_core -q --test profile_on_cuts_on_the_line_p6_3
//! ```

#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

mod common;

use std::sync::atomic::AtomicBool;

use common::op_fixtures::no_dressups;
use common::session::{polygon_model, single_op_session_with, toolpath_config};
use common::tools::endmill_tool_config;
use rs_cam_core::compute::StockConfig;
use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::config::{ArcFitParams, DressupConfig, DressupEntryStyle};
use rs_cam_core::compute::domain::{DomainRefusal, OpDomainRefusal};
use rs_cam_core::compute::floor::Floor;
use rs_cam_core::compute::operation_configs::ProfileConfig;
use rs_cam_core::compute::toolpath_stats::{TabNote, TabPlacementFinding};
use rs_cam_core::geo::P2;
use rs_cam_core::ops::profile::ProfileSide;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{
    AddToolpathArgs, Command, ProjectSession, ProjectSessionBuilder, ReplaceToolpathConfigArgs,
    SessionError, SetToolpathFloorArgs, SetToolpathOperationArgs, SetToolpathParamArgs,
};
use rs_cam_core::toolpath::{MoveIntent, MoveType, Toolpath};

/// Tool diameter (mm).
const TOOL_D: f64 = 6.0;
/// Cut depth (mm), in two levels.
const DEPTH: f64 = 6.0;
/// Tab width and height (mm).
const TAB_W: f64 = 6.0;
const TAB_H: f64 = 2.0;
/// Float slack for points that the kernels copy (mm).
const EPS: f64 = 1e-6;

/// The open L, drawn from its top end down and then east.
const L: [[f64; 2]; 3] = [[-30.0, 20.0], [-30.0, -10.0], [-5.0, -10.0]];
/// A 2 mm line, drawn east.
const SHORT: [[f64; 2]; 2] = [[10.0, -20.0], [12.0, -20.0]];
/// A 40 x 40 mm square, drawn counter-clockwise, away from the L.
const SQUARE: [[f64; 2]; 4] = [[10.0, 0.0], [50.0, 0.0], [50.0, 40.0], [10.0, 40.0]];

/// A 160 x 120 x 10 mm board centred on X0 Y0, its top at Z0.
fn board() -> StockConfig {
    StockConfig {
        x: 160.0,
        y: 120.0,
        z: 10.0,
        origin_x: -80.0,
        origin_y: -60.0,
        origin_z: -10.0,
        auto_from_model: false,
        ..StockConfig::default()
    }
}

fn pts(points: &[[f64; 2]]) -> Vec<P2> {
    points.iter().map(|&[x, y]| P2::new(x, y)).collect()
}

fn open(points: &[[f64; 2]]) -> Polygon2 {
    Polygon2::open_path(pts(points))
}

fn on(climb: bool, tab_count: usize, tab_points: Vec<[f64; 2]>) -> ProfileConfig {
    ProfileConfig {
        side: ProfileSide::On,
        floor: rs_cam_core::compute::floor::Floor::top(DEPTH),
        depth_per_pass: DEPTH / 2.0,
        climb,
        tab_count,
        tab_width: TAB_W,
        tab_height: TAB_H,
        tab_points,
        finishing_passes: 0,
        ..ProfileConfig::default()
    }
}

/// The default Profile dressups, with a lead asked for.
fn dressed() -> DressupConfig {
    let mut d = DressupConfig::for_op(OperationType::Profile);
    if d.lead_in_out.is_none() {
        d.lead_in_out = DressupConfig::for_op(OperationType::Trace).lead_in_out;
    }
    assert!(d.lead_in_out.is_some(), "non-vacuity: a lead is asked for");
    d
}

/// The toolpath and the tab finding of one Profile On, or the refusal.
fn run(
    lines: Vec<Polygon2>,
    cfg: ProfileConfig,
    dressups: DressupConfig,
) -> Result<(Toolpath, Option<TabPlacementFinding>), String> {
    let mut session = single_op_session_with(
        board(),
        endmill_tool_config(TOOL_D),
        polygon_model(lines, "p6_3_on"),
        "p6_3",
        OperationConfig::Profile(cfg),
        |tc| tc.dressups = dressups,
    );
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(0, &cancel)
        .map_err(|e| e.to_string())?;
    let result = session.result_at(0).expect("a result");
    Ok((
        result.annotated().toolpath.clone(),
        result.stats.tab_placement.clone(),
    ))
}

/// The distance from `p` to the segment `a`-`b`, and the position of the
/// nearest point as a fraction of the segment.
fn to_segment(p: P2, a: P2, b: P2) -> (f64, f64) {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len2 = dx * dx + dy * dy;
    let t = if len2 > 0.0 {
        (((p.x - a.x) * dx + (p.y - a.y) * dy) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let q = P2::new(a.x + t * dx, a.y + t * dy);
    ((p.x - q.x).hypot(p.y - q.y), t)
}

/// The distance from `p` to the polyline `line` (closed adds the closing
/// segment), and the arc length (mm) of the nearest point along it.
fn to_line(p: P2, line: &[P2], closed: bool) -> (f64, f64) {
    let n = line.len();
    let segs = if closed { n } else { n - 1 };
    let mut best = (f64::INFINITY, 0.0);
    let mut walked = 0.0;
    for i in 0..segs {
        let (a, b) = (line[i], line[(i + 1) % n]);
        let len = (b.x - a.x).hypot(b.y - a.y);
        let (d, t) = to_segment(p, a, b);
        if d < best.0 - EPS {
            best = (d, walked + t * len);
        }
        walked += len;
    }
    best
}

/// The fed moves (not rapids) of `tp`: the XY, the Z and the intent.
fn fed(tp: &Toolpath) -> Vec<(P2, f64, MoveIntent)> {
    tp.moves
        .iter()
        .filter(|m| m.move_type != MoveType::Rapid)
        .map(|m| (P2::new(m.target.x, m.target.y), m.target.z, m.intent))
        .collect()
}

/// (a): every fed move of `tp` lies on the open `line` (within `tol`),
/// takes no lead, and each level runs forward along the line from its
/// first point to its last point, with no move back towards the start.
fn assert_on_open_line(what: &str, tp: &Toolpath, line: &[P2], tol: f64) {
    let moves = fed(tp);
    assert!(!moves.is_empty(), "{what}: non-vacuity, the Profile cuts");
    let length: f64 = line
        .windows(2)
        .map(|w| (w[1].x - w[0].x).hypot(w[1].y - w[0].y))
        .sum();
    let mut levels = 0;
    let mut in_entry = false;
    let mut along = f64::NEG_INFINITY;
    for &(p, z, intent) in &moves {
        assert!(
            !matches!(intent, MoveIntent::LeadIn | MoveIntent::LeadOut),
            "{what}: an open pass takes no lead: {p:?}"
        );
        let (d, s) = to_line(p, line, false);
        assert!(
            d <= tol,
            "{what}: a fed move leaves the drawn line by {d:.4} mm: {p:?} Z{z} ({intent:?})"
        );
        if matches!(
            intent,
            MoveIntent::EntryPlunge | MoveIntent::EntryRamp | MoveIntent::EntryHelix
        ) {
            // An entry starts a level. A ramp walks on the line (checked
            // above) and ends where the cut starts.
            if !in_entry {
                levels += 1;
            }
            in_entry = true;
            along = s;
            continue;
        }
        if in_entry {
            assert!(
                along <= tol,
                "{what}: a level starts {along:.4} mm along the line, not at its start"
            );
            in_entry = false;
        }
        assert!(
            s >= along - tol,
            "{what}: a fed move runs back along the line from {along:.4} to {s:.4} mm \
             (a chord between the ends): {p:?}"
        );
        along = along.max(s);
    }
    assert_eq!(levels, 2, "{what}: two levels, each one entry");
    assert!(
        (along - length).abs() <= tol,
        "{what}: the cut ends {along:.4} mm along the line, not at its end {length:.4}"
    );
}

/// The arc-fit budget of the default dressups.
fn tol() -> f64 {
    ArcFitParams::default().tolerance
}

/// (a): the L and the 2 mm line, each alone, with no dressups and with
/// the defaults and a lead.
#[test]
fn an_open_path_is_cut_on_the_line_with_no_chord_p6_3() {
    for (name, line) in [("L", &L[..]), ("short", &SHORT[..])] {
        for (dressed_name, dressups, tol) in [
            ("no dressups", no_dressups(), EPS),
            ("defaults", dressed(), tol()),
        ] {
            let what = format!("{name} {dressed_name}");
            let (tp, _) = run(vec![open(line)], on(false, 0, vec![]), dressups)
                .unwrap_or_else(|e| panic!("{what}: Profile On generates: {e}"));
            assert_on_open_line(&what, &tp, &pts(line), tol);
        }
    }
}

/// (a): a ramp entry asked for on the L and on the 2 mm line. No fed
/// move leaves the line, so none goes past its end normals. The ramp is
/// emitted on the L (it walks back and forth on the first leg). On the
/// 2 mm line the ramp has no room, and the entry stays a plunge (measured
/// 2026-10-22: each level is `EntryPlunge` then `FinishingCut`).
#[test]
fn a_ramp_entry_stays_on_an_open_path_p6_3() {
    let mut dressups = dressed();
    dressups.entry_style = DressupEntryStyle::Ramp;
    for (name, line, ramps) in [("L", &L[..], true), ("short", &SHORT[..], false)] {
        let (tp, _) = run(vec![open(line)], on(false, 0, vec![]), dressups.clone()).unwrap();
        let moves = fed(&tp);
        assert_eq!(
            moves.iter().any(|(_, _, i)| *i == MoveIntent::EntryRamp),
            ramps,
            "{name}: the ramp is emitted = {ramps}: {:?}",
            moves.iter().map(|m| (m.1, m.2)).collect::<Vec<_>>()
        );
        let path = pts(line);
        for (p, z, intent) in moves {
            let (d, _) = to_line(p, &path, false);
            assert!(
                d <= tol(),
                "{name} ramp: a fed move leaves the line by {d:.4} mm: {p:?} Z{z} ({intent:?})"
            );
        }
    }
}

/// (b): climb does not reverse an open path.
#[test]
fn climb_does_not_reverse_an_open_path_l1() {
    let (conventional, _) = run(vec![open(&L)], on(false, 0, vec![]), no_dressups()).unwrap();
    let (climb, _) = run(vec![open(&L)], on(true, 0, vec![]), no_dressups()).unwrap();
    assert_on_open_line("L climb", &climb, &pts(&L), EPS);
    assert_eq!(
        fed(&conventional),
        fed(&climb),
        "climb changed the cut of an open path"
    );
}

/// (c): a closed square with side On is cut on its outline, and each
/// level closes back to its start.
#[test]
fn a_closed_square_is_cut_on_its_outline_p6_3() {
    let square = Polygon2::new(pts(&SQUARE));
    let outline = pts(&SQUARE);
    let (tp, _) = run(vec![square], on(false, 0, vec![]), no_dressups()).unwrap();
    let moves = fed(&tp);
    assert!(moves.len() >= 8, "non-vacuity: two closed levels");
    let mut start: Option<P2> = None;
    let mut closes = 0;
    for &(p, _z, intent) in &moves {
        let (d, _) = to_line(p, &outline, true);
        assert!(d <= EPS, "a cut point is {d:.4} mm off the outline: {p:?}");
        if intent == MoveIntent::EntryPlunge {
            start = Some(p);
        } else if start.is_some_and(|s| (s.x - p.x).hypot(s.y - p.y) <= EPS) {
            closes += 1;
        }
    }
    assert_eq!(closes, 2, "each of the two levels closes back to its start");
    // Every corner is reached: the tool centre runs on the outline itself.
    for c in &outline {
        assert!(
            moves
                .iter()
                .any(|(p, _, _)| (p.x - c.x).hypot(p.y - c.y) <= EPS),
            "the pass does not reach the corner {c:?}"
        );
    }
}

/// (d): tabs on the square only, and ONE note for the open L.
#[test]
fn tabs_go_on_the_closed_ring_only_l8() {
    let (tp, finding) = run(
        vec![Polygon2::new(pts(&SQUARE)), open(&L)],
        on(false, 2, vec![]),
        no_dressups(),
    )
    .unwrap();
    let final_z = -DEPTH;
    let square = pts(&SQUARE);
    let l = pts(&L);
    // A tab is a vertical fed rise at the final level.
    let mut lifts = 0;
    for w in tp.moves.windows(2) {
        let (a, b) = (w[0].target, w[1].target);
        if w[1].move_type == MoveType::Rapid {
            continue;
        }
        let vertical = (a.x - b.x).hypot(a.y - b.y) < EPS;
        if vertical && (a.z - final_z).abs() < EPS && (b.z - (final_z + TAB_H)).abs() < EPS {
            lifts += 1;
            let p = P2::new(a.x, a.y);
            assert!(
                to_line(p, &square, true).0 <= EPS,
                "a tab lift is not on the square: {p:?}"
            );
            assert!(
                to_line(p, &l, false).0 > 1.0,
                "a tab lift is on the open L: {p:?}"
            );
        }
    }
    // `tab_count` is the minimum per loop: the side rule puts one on each
    // side of the square (P5).
    assert!(lifts >= 2, "the square takes the 2 tabs asked: {lifts}");
    let finding = finding.expect("the open L is named");
    let open_notes: Vec<&TabNote> = finding
        .notes
        .iter()
        .filter(|n| matches!(n, TabNote::OpenPath { .. }))
        .collect();
    match open_notes.as_slice() {
        [TabNote::OpenPath { start, asked: 2 }] => assert!(
            (start[0] - L[0][0]).abs() < EPS && (start[1] - L[0][1]).abs() < EPS,
            "the note names the start of the L: {start:?}"
        ),
        other => panic!("one OpenPath note for the L: {other:?}"),
    }
    assert!(
        finding.message().contains("closed loops only"),
        "{}",
        finding.message()
    );
}

/// (d): a hand tab 1 mm from the open L and far from the square is
/// refused: a hand tab snaps to a closed ring only.
#[test]
fn a_hand_tab_does_not_snap_to_an_open_path_l8() {
    let error = run(
        vec![Polygon2::new(pts(&SQUARE)), open(&L)],
        on(false, 0, vec![[-31.0, 5.0]]),
        no_dressups(),
    )
    .expect_err("a hand tab next to the open L only is refused");
    assert!(
        error.contains("hand tab") && error.contains("X-31.000"),
        "{error}"
    );
    // The control: the same point next to the square snaps.
    let (tp, _) = run(
        vec![Polygon2::new(pts(&SQUARE)), open(&L)],
        on(false, 0, vec![[30.0, -1.0]]),
        no_dressups(),
    )
    .expect("a hand tab next to the square snaps");
    assert!(!tp.moves.is_empty());
}

/// (c): with the default dressups and a lead asked for, the closed square
/// of side On takes no lead (`RingMaterial::BothSides`), and every fed
/// move stays on the outline.
#[test]
fn a_closed_on_ring_takes_no_lead_p6_3() {
    let outline = pts(&SQUARE);
    let (tp, _) = run(
        vec![Polygon2::new(outline.clone())],
        on(false, 0, vec![]),
        dressed(),
    )
    .unwrap();
    let moves = fed(&tp);
    assert!(!moves.is_empty(), "non-vacuity: the square is cut");
    for (p, z, intent) in moves {
        assert!(
            !matches!(intent, MoveIntent::LeadIn | MoveIntent::LeadOut),
            "an On ring takes no lead: {p:?} ({intent:?})"
        );
        let (d, _) = to_line(p, &outline, true);
        assert!(
            d <= tol(),
            "a fed move leaves the outline by {d:.4} mm: {p:?} Z{z} ({intent:?})"
        );
    }
}

/// (e): the pair side On + floor "Each picked face" is one door rule
/// (`OpDomainRefusal::SideOnEachFace`, run by `validate_operation`).
#[track_caller]
fn assert_side_on_each_face(result: Result<(), SessionError>) {
    match result {
        Err(SessionError::OutOfDomain(DomainRefusal::Operation { refusal, .. })) => {
            assert!(
                matches!(refusal, OpDomainRefusal::SideOnEachFace { .. }),
                "expected the side On + each face refusal, got {refusal}"
            );
            let text = refusal.to_string();
            assert!(
                text.contains("side On") && text.contains("Each picked face"),
                "{text}"
            );
        }
        other => panic!("expected an operation domain refusal, got {other:?}"),
    }
}

fn each_face() -> Floor {
    Floor::EachFace { depth: 1.0 }
}

/// A Profile config with `side` and `floor`.
fn profile_with(side: ProfileSide, floor: Floor) -> OperationConfig {
    OperationConfig::Profile(ProfileConfig {
        side,
        floor,
        ..ProfileConfig::default()
    })
}

/// A session with one tool and one valid Profile (side Outside, floor
/// below the top).
fn door_session() -> ProjectSession {
    let mut builder = ProjectSessionBuilder::new();
    let _ = builder.add_tool(endmill_tool_config(TOOL_D));
    let _ = builder
        .add_toolpath(
            0,
            toolpath_config(
                "p6_3_door",
                profile_with(ProfileSide::Outside, Floor::top(DEPTH)),
                rs_cam_core::ids::ToolId::from_raw(0),
                rs_cam_core::ids::ModelId::from_raw(0),
            ),
        )
        .unwrap();
    builder.build()
}

fn stored(s: &ProjectSession) -> (ProfileSide, Floor) {
    match &s.toolpath_configs()[0].operation {
        OperationConfig::Profile(c) => (c.side, c.floor),
        other => panic!("expected a Profile, got {:?}", other.op_type()),
    }
}

/// (e): AddToolpath, the op change and ReplaceToolpathConfig refuse the
/// pair and write nothing. Positive controls: On with a top floor, and
/// Outside with each face.
#[test]
fn side_on_with_each_face_refuses_at_the_config_doors_p6_3() {
    let mut s = door_session();
    let before = s.toolpath_count();
    let add = |s: &mut ProjectSession, operation: OperationConfig| {
        s.apply(Command::AddToolpath(AddToolpathArgs {
            setup_index: 0,
            config: Box::new(toolpath_config(
                "p6_3_add",
                operation,
                rs_cam_core::ids::ToolId::from_raw(0),
                rs_cam_core::ids::ModelId::from_raw(0),
            )),
        }))
        .map(|_| ())
    };
    assert_side_on_each_face(add(&mut s, profile_with(ProfileSide::On, each_face())));
    assert_eq!(s.toolpath_count(), before, "nothing added");
    add(&mut s, profile_with(ProfileSide::On, Floor::top(DEPTH))).unwrap();
    add(&mut s, profile_with(ProfileSide::Outside, each_face())).unwrap();

    let mut s = door_session();
    let refused = s
        .apply(Command::SetToolpathOperation(SetToolpathOperationArgs {
            index: 0,
            operation: Box::new(profile_with(ProfileSide::On, each_face())),
        }))
        .map(|_| ());
    assert_side_on_each_face(refused);
    assert_eq!(
        stored(&s),
        (ProfileSide::Outside, Floor::top(DEPTH)),
        "nothing written"
    );

    let refused = s
        .apply(Command::ReplaceToolpathConfig(ReplaceToolpathConfigArgs {
            index: 0,
            config: Box::new(toolpath_config(
                "p6_3_door",
                profile_with(ProfileSide::On, each_face()),
                rs_cam_core::ids::ToolId::from_raw(0),
                rs_cam_core::ids::ModelId::from_raw(0),
            )),
        }))
        .map(|_| ());
    assert_side_on_each_face(refused);
    assert_eq!(
        stored(&s),
        (ProfileSide::Outside, Floor::top(DEPTH)),
        "nothing written"
    );
}

/// (e): the setter refuses either half of the pair: side On under each
/// face (`SetToolpathParam`), and each face under side On
/// (`SetToolpathFloor`).
#[test]
fn side_on_with_each_face_refuses_either_half_p6_3() {
    let set_side = |s: &mut ProjectSession, value: &str| {
        s.apply(Command::SetToolpathParam(SetToolpathParamArgs {
            index: 0,
            param: "side".to_owned(),
            value: serde_json::json!(value),
        }))
        .map(|_| ())
    };
    let set_floor = |s: &mut ProjectSession, floor: Floor| {
        s.apply(Command::SetToolpathFloor(SetToolpathFloorArgs {
            index: 0,
            floor,
        }))
        .map(|_| ())
    };

    // The side half: the floor is each face, then side On refuses.
    let mut s = door_session();
    set_floor(&mut s, each_face()).unwrap();
    assert_side_on_each_face(set_side(&mut s, "on"));
    assert_eq!(
        stored(&s),
        (ProfileSide::Outside, each_face()),
        "nothing written"
    );

    // The floor half: side is On, then each face refuses.
    let mut s = door_session();
    set_side(&mut s, "on").unwrap();
    assert_side_on_each_face(set_floor(&mut s, each_face()));
    assert_eq!(
        stored(&s),
        (ProfileSide::On, Floor::top(DEPTH)),
        "nothing written"
    );
    // Positive control: back to Inside, then each face is accepted.
    set_side(&mut s, "inside").unwrap();
    set_floor(&mut s, each_face()).unwrap();
    assert_eq!(stored(&s), (ProfileSide::Inside, each_face()));
}

/// (e): the loader refuses a file that holds the pair (the raw builder
/// checks nothing, so it can save one).
#[test]
fn side_on_with_each_face_refuses_at_load_p6_3() {
    let mut builder = ProjectSessionBuilder::new();
    let _ = builder.add_tool(endmill_tool_config(TOOL_D));
    let _ = builder
        .add_toolpath(
            0,
            toolpath_config(
                "p6_3_load",
                profile_with(ProfileSide::On, each_face()),
                rs_cam_core::ids::ToolId::from_raw(0),
                rs_cam_core::ids::ModelId::from_raw(0),
            ),
        )
        .unwrap();
    let raw = builder.build();
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("p6_3_each_face_load");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("project.toml");
    raw.save(&path).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(
        text.contains("side = \"on\""),
        "the file holds side On:\n{text}"
    );
    let loaded = ProjectSession::load(&path).map(|_| ());
    let _ = std::fs::remove_dir_all(&dir);
    assert_side_on_each_face(loaded);
}
