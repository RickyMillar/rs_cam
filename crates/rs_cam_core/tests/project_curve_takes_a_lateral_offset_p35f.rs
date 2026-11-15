//! P35 part F: the lateral offset of a Project Curve
//! (`ProjectCurveConfig::lateral_offset`), through the session door.
//!
//! Operator ruling 2026-11-16 ("2 mm tool slot"): two laps of a small tool,
//! 0.6 mm left and 0.6 mm right of the curves that a V-bit traces.
//!
//! # The claims
//!
//! - (a) An open path drawn along +X: +0.6 puts each fed point 0.6 mm to
//!   the left of the drawn direction (Y +0.6), -0.6 to the right. The
//!   piece keeps the X span of the path (it ends on the end normals).
//! - (b) A closed ring: +0.6 moves it out of its own area, for a ring
//!   drawn counter-clockwise and one drawn clockwise. The value adds to
//!   `side`.
//! - (c) A path with a corner: each fed point is 0.6 mm from the path on
//!   the two sides (the join).
//! - (d) The projection runs at the OFFSET position: with the cutter
//!   projection the tool disc of the moved path reaches the high ground of
//!   a step and the tip rises; the point projection stays low.
//! - (e) A ring that splits and a hairpin with no room are cut piece by
//!   piece, with one warning each. The op does not refuse.
//! - (f) With `depth_per_pass` and remaining stock the ladder of P33 and
//!   P35 B and the air-cut filter work on the moved path.
//! - (g) No key and the value 0 give the same moves; `null` clears the key.
//!
//! Red proof: before P35 F the parameter door refuses `lateral_offset`
//! (every test but (g) fails at its first write).
//!
//! ```text
//! cargo test -p rs_cam_core -q --test project_curve_takes_a_lateral_offset_p35f
//! ```

#![expect(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

mod common;

use std::sync::atomic::AtomicBool;

use common::meshes::height_field;
use common::op_fixtures::{SIM_CELL_MM, no_dressups, run_prior, simulate};
use common::session::{mesh_model, pinned_heights, polygon_model, toolpath_config};
use common::tools::endmill_tool_config;
use rs_cam_core::compute::StockConfig;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::StockSource;
use rs_cam_core::compute::operation_configs::{PocketConfig, ProjectCurveConfig};
use rs_cam_core::compute::toolpath_stats::TabNote;
use rs_cam_core::diagnostics::Severity;
use rs_cam_core::geo::P2;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{Command, ProjectSession, ProjectSessionBuilder, SetToolpathParamArgs};
use rs_cam_core::toolpath::{MoveType, Toolpath};
use serde_json::json;

const TOOL_D: f64 = 6.0;
/// The board top and the flat surface (mm). The tip is 1 mm below the
/// surface (`depth = 1.0`).
const STOCK_TOP: f64 = 20.0;
const GROUND_Z: f64 = 9.0;
const DEPTH: f64 = 1.0;
/// The offset library puts a corner arc on points within 0.010 mm of the
/// arc (`polygon::offset_open_path_reported`), and the kernel samples the
/// path between its vertices.
const TOL: f64 = 0.011;

fn board() -> StockConfig {
    StockConfig {
        x: 100.0,
        y: 60.0,
        z: Some(STOCK_TOP),
        origin_x: -50.0,
        origin_y: -30.0,
        origin_z: 0.0,
        auto_from_model: false,
        ..StockConfig::default()
    }
}

/// What one case builds.
struct Case {
    curves: Vec<Polygon2>,
    /// The surface height at (x, y).
    ground: fn(f64, f64) -> f64,
    /// The mesh grid step (mm).
    grid: f64,
    /// A pocket before the op (its X span, floor Z 12), and the stock
    /// source of the op.
    pocket: Option<(f64, f64)>,
    source: StockSource,
    params: Vec<(&'static str, serde_json::Value)>,
}

impl Case {
    fn flat(curves: Vec<Polygon2>, params: Vec<(&'static str, serde_json::Value)>) -> Self {
        Self {
            curves,
            ground: |_, _| GROUND_Z,
            grid: 5.0,
            pocket: None,
            source: StockSource::Fresh,
            params,
        }
    }
}

/// The session of a case and the index of the Project Curve. Each
/// parameter goes through `SetToolpathParam`; the first refusal is the
/// error.
fn session(case: Case) -> Result<(ProjectSession, usize), String> {
    let mut builder = ProjectSessionBuilder::new().stock(board());
    let tool_idx = builder.add_tool(endmill_tool_config(TOOL_D));
    let tool_id = rs_cam_core::ids::ToolId::from_raw(builder.tools()[tool_idx].id.raw());
    let surface = builder.add_model(mesh_model(
        height_field(60.0, case.grid, case.ground),
        "p35f_ground",
    ));
    let curve = builder.add_model(polygon_model(case.curves, "p35f_curves"));
    if let Some((x0, x1)) = case.pocket {
        let pocket_model = builder.add_model(polygon_model(
            vec![Polygon2::new(vec![
                P2::new(x0, -10.0),
                P2::new(x1, -10.0),
                P2::new(x1, 10.0),
                P2::new(x0, 10.0),
            ])],
            "p35f_pocket",
        ));
        let _ = builder
            .add_toolpath(
                0,
                toolpath_config(
                    "pocket",
                    OperationConfig::Pocket(PocketConfig {
                        stepover: Some(2.0),
                        floor: rs_cam_core::compute::floor::Floor::top(STOCK_TOP - 12.0),
                        depth_per_pass: Some(4.0),
                        ..PocketConfig::default()
                    }),
                    tool_id,
                    pocket_model,
                ),
            )
            .expect("add the pocket");
    }
    let mut op = toolpath_config(
        "curve",
        OperationConfig::ProjectCurve(ProjectCurveConfig {
            depth: Some(DEPTH),
            surface_model_id: Some(surface),
            ..ProjectCurveConfig::default()
        }),
        tool_id,
        curve,
    );
    op.heights = pinned_heights(STOCK_TOP, 0.0);
    op.stock_source = case.source;
    if case.source == StockSource::Fresh {
        op.dressups = no_dressups();
    }
    let index = builder.add_toolpath(0, op).expect("add the curve op");
    let mut session = builder.build();
    for (param, value) in case.params {
        let _ = session
            .apply(Command::SetToolpathParam(SetToolpathParamArgs {
                index,
                param: param.to_owned(),
                value,
            }))
            .map_err(|e| format!("`{param}`: {e}"))?;
    }
    Ok((session, index))
}

/// The stored toolpath of a Fresh case.
fn run(case: Case) -> Result<(ProjectSession, usize, Toolpath), String> {
    let (mut session, index) = session(case)?;
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(index, &cancel)
        .map_err(|e| e.to_string())?;
    let tp = session
        .result_at(index)
        .expect("a result")
        .annotated()
        .toolpath
        .clone();
    Ok((session, index, tp))
}

/// The fed moves at or below `z_max`: the cut, without the plunge from the
/// retract plane.
fn cut(tp: &Toolpath, z_max: f64) -> Vec<(P2, f64)> {
    tp.moves
        .iter()
        .filter(|m| m.move_type != MoveType::Rapid && m.target.z <= z_max + 1e-6)
        .map(|m| (P2::new(m.target.x, m.target.y), m.target.z))
        .collect()
}

/// The distance from `p` to the polyline `line` (closed adds the closing
/// segment).
fn to_line(p: P2, line: &[P2], closed: bool) -> f64 {
    let n = line.len();
    let segs = if closed { n } else { n - 1 };
    (0..segs)
        .map(|i| {
            let (a, b) = (line[i], line[(i + 1) % n]);
            let (dx, dy) = (b.x - a.x, b.y - a.y);
            let t = (((p.x - a.x) * dx + (p.y - a.y) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
            (p.x - (a.x + t * dx)).hypot(p.y - (a.y + t * dy))
        })
        .fold(f64::INFINITY, f64::min)
}

fn straight() -> Polygon2 {
    Polygon2::open_path(vec![P2::new(-30.0, 0.0), P2::new(30.0, 0.0)])
}

fn square(ccw: bool) -> Vec<P2> {
    let mut pts = vec![
        P2::new(-10.0, -10.0),
        P2::new(10.0, -10.0),
        P2::new(10.0, 10.0),
        P2::new(-10.0, 10.0),
    ];
    if !ccw {
        pts.reverse();
    }
    pts
}

/// The signed distance from `p` to the 20 mm square: negative inside.
fn signed_to_square(p: P2) -> f64 {
    let dx = p.x.abs() - 10.0;
    let dy = p.y.abs() - 10.0;
    if dx > 0.0 && dy > 0.0 {
        dx.hypot(dy)
    } else {
        dx.max(dy)
    }
}

/// (a)
#[test]
fn an_open_path_moves_left_for_a_positive_value_p35f() {
    for (offset, y) in [(0.6, 0.6), (-0.6, -0.6)] {
        let (_, _, tp) = run(Case::flat(
            vec![straight()],
            vec![("lateral_offset", json!(offset))],
        ))
        .unwrap_or_else(|e| panic!("offset {offset}: {e}"));
        let points = cut(&tp, GROUND_Z - DEPTH);
        assert!(points.len() > 20, "{} cut points", points.len());
        for (p, z) in &points {
            assert!(
                (p.y - y).abs() < 1e-9,
                "offset {offset}: a fed point at Y {:.6}; the drawn direction is +X, so the \
                 left is +Y",
                p.y
            );
            assert!((z - (GROUND_Z - DEPTH)).abs() < 1e-6);
        }
        let (lo, hi) = points
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), (p, _)| {
                (lo.min(p.x), hi.max(p.x))
            });
        assert!(
            (lo + 30.0).abs() < 1e-6 && (hi - 30.0).abs() < 1e-6,
            "offset {offset}: the piece spans X {lo} to {hi}; the path spans -30 to 30"
        );
    }
}

/// (b)
#[test]
fn a_closed_ring_moves_out_of_its_own_area_p35f() {
    for ccw in [true, false] {
        let (_, _, tp) = run(Case::flat(
            vec![Polygon2::new(square(ccw))],
            vec![("lateral_offset", json!(0.6))],
        ))
        .unwrap_or_else(|e| panic!("ccw {ccw}: {e}"));
        let points = cut(&tp, GROUND_Z - DEPTH);
        assert!(points.len() > 40, "{} cut points", points.len());
        for (p, _) in &points {
            let d = signed_to_square(*p);
            assert!(
                (d - 0.6).abs() < TOL,
                "ccw {ccw}: a fed point is {d:.4} mm from the ring; expected 0.6 outside"
            );
        }
    }
    // The value adds to `side`: inside by the radius (3.0), out by 0.6.
    let (_, _, tp) = run(Case::flat(
        vec![Polygon2::new(square(true))],
        vec![("side", json!("inside")), ("lateral_offset", json!(0.6))],
    ))
    .expect("side inside with a value");
    for (p, _) in &cut(&tp, GROUND_Z - DEPTH) {
        let d = signed_to_square(*p);
        assert!(
            (d + 2.4).abs() < 2.0 * TOL,
            "side inside, +0.6: a fed point is {d:.4} mm from the ring; expected 2.4 inside"
        );
    }
}

/// (c)
#[test]
fn a_corner_keeps_the_distance_on_the_two_sides_p35f() {
    let corner = vec![P2::new(-20.0, 0.0), P2::new(0.0, 0.0), P2::new(0.0, 20.0)];
    for offset in [0.6, -0.6] {
        let (_, _, tp) = run(Case::flat(
            vec![Polygon2::open_path(corner.clone())],
            vec![("lateral_offset", json!(offset))],
        ))
        .unwrap_or_else(|e| panic!("offset {offset}: {e}"));
        let points = cut(&tp, GROUND_Z - DEPTH);
        assert!(points.len() > 20, "{} cut points", points.len());
        for (p, _) in &points {
            let d = to_line(*p, &corner, false);
            assert!(
                (d - 0.6).abs() < TOL,
                "offset {offset}: a fed point is {d:.4} mm from the path"
            );
        }
        // The path turns left at the corner. The left side is inside the
        // bend (X below 0, Y above 0); the right side goes round it.
        let outside_the_bend = points.iter().any(|(p, _)| p.x > 1e-6 && p.y < -1e-6);
        assert_eq!(
            outside_the_bend,
            offset < 0.0,
            "offset {offset}: the right side has the arc join round the corner"
        );
    }
}

/// (d)
#[test]
fn the_projection_runs_at_the_offset_position_p35f() {
    // Low ground (Z 9) for Y at or below 0, high ground (Z 12) from Y 1.
    let step: fn(f64, f64) -> f64 = |_, y| {
        if y >= 1.0 {
            12.0
        } else if y <= 0.0 {
            GROUND_Z
        } else {
            GROUND_Z + 3.0 * y
        }
    };
    let path = || {
        vec![Polygon2::open_path(vec![
            P2::new(-30.0, -4.0),
            P2::new(30.0, -4.0),
        ])]
    };
    let tip_z = |params: Vec<(&'static str, serde_json::Value)>| {
        let (_, _, tp) = run(Case {
            ground: step,
            grid: 1.0,
            ..Case::flat(path(), params)
        })
        .expect("generates");
        let points = cut(&tp, 15.0);
        assert!(points.len() > 20);
        let (lo, hi) = points
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), (_, z)| {
                (lo.min(*z), hi.max(*z))
            });
        assert!(hi - lo < 1e-6, "one tip Z along the path: {lo} to {hi}");
        (lo, points[0].0.y)
    };
    // The 6 mm tool at Y -4 reaches Y -1: the low ground.
    let (z, y) = tip_z(vec![]);
    assert!((z - (GROUND_Z - DEPTH)).abs() < 1e-6 && (y + 4.0).abs() < 1e-9);
    // Moved 2 mm to the left (Y -2) the tool disc reaches Y +1: the high
    // ground. The cutter projection lifts the tip by the step.
    let (z, y) = tip_z(vec![("lateral_offset", json!(2.0))]);
    assert!((y + 2.0).abs() < 1e-9, "the path is at Y {y}");
    assert!(
        (z - (12.0 - DEPTH)).abs() < 1e-6,
        "cutter projection at the offset position: tip Z {z}, expected {}",
        12.0 - DEPTH
    );
    // The point projection reads the surface under the moved centre: low.
    let (z, _) = tip_z(vec![
        ("projection", json!("point")),
        ("lateral_offset", json!(2.0)),
    ]);
    assert!(
        (z - (GROUND_Z - DEPTH)).abs() < 1e-6,
        "point projection: {z}"
    );
}

/// (e)
#[test]
fn a_split_is_cut_piece_by_piece_with_a_warning_p35f() {
    // A dumbbell ring (two 20 mm squares, a 2 mm neck): moved in by 1.4 mm
    // it is two rings. A hairpin with legs 1 mm apart: the left side
    // (inside the turn) has no room for 0.6 mm on each leg.
    let dumbbell = Polygon2::new(
        [
            (-30.0, -25.0),
            (-10.0, -25.0),
            (-10.0, -16.0),
            (10.0, -16.0),
            (10.0, -25.0),
            (30.0, -25.0),
            (30.0, -5.0),
            (10.0, -5.0),
            (10.0, -14.0),
            (-10.0, -14.0),
            (-10.0, -5.0),
            (-30.0, -5.0),
        ]
        .iter()
        .map(|&(x, y)| P2::new(x, y))
        .collect(),
    );
    let (session, index, tp) = run(Case::flat(
        vec![dumbbell],
        vec![("lateral_offset", json!(-1.4))],
    ))
    .expect("a split ring generates");
    let points = cut(&tp, GROUND_Z - DEPTH);
    let left = points.iter().filter(|(p, _)| p.x < -10.0).count();
    let right = points.iter().filter(|(p, _)| p.x > 10.0).count();
    assert!(left > 20 && right > 20, "{left} left, {right} right");
    assert_eq!(left + right, points.len(), "no cut in the neck");
    let notes = session
        .result_at(index)
        .expect("a result")
        .stats
        .tab_placement
        .clone()
        .expect("the op records the split")
        .notes;
    assert!(
        matches!(
            notes.as_slice(),
            [TabNote::OffsetSplit {
                pieces: 2,
                closed: true,
                ..
            }]
        ),
        "{notes:?}"
    );
    let diagnostics = session
        .diagnose_toolpath_with_trace(index, None)
        .expect("diagnose");
    let warning = diagnostics
        .iter()
        .find(|d| d.id.as_str() == "geom.profile_loop_split")
        .expect("the split is a diagnostic");
    assert_eq!(warning.severity, Severity::Caution);
    assert!(
        warning.message.contains("Project Curve") && warning.message.contains("2 pieces"),
        "{}",
        warning.message
    );

    // The hairpin, with a straight path beside it so that the op has a cut.
    let hairpin = Polygon2::open_path(vec![
        P2::new(-20.0, 9.5),
        P2::new(0.0, 9.5),
        P2::new(0.0, 10.5),
        P2::new(-20.0, 10.5),
    ]);
    let (session, index, tp) = run(Case::flat(
        vec![straight(), hairpin],
        vec![("lateral_offset", json!(0.6))],
    ))
    .expect("a hairpin with no room does not refuse the op");
    assert!(
        cut(&tp, GROUND_Z - DEPTH)
            .iter()
            .any(|(p, _)| (p.y - 0.6).abs() < 1e-9),
        "the straight path is cut"
    );
    let notes = session
        .result_at(index)
        .expect("a result")
        .stats
        .tab_placement
        .clone()
        .expect("the op records the hairpin")
        .notes;
    match notes.as_slice() {
        [
            TabNote::OffsetSplit {
                start,
                pieces,
                closed: false,
                ..
            },
        ] => {
            assert_eq!(*start, [-20.0, 9.5]);
            assert_ne!(*pieces, 1);
        }
        other => panic!("one note for the hairpin: {other:?}"),
    }
}

/// (f)
#[test]
fn the_ladder_and_the_remaining_stock_read_the_moved_path_p35f() {
    // The stepped-stock fixture of P35 B: a pocket floor at Z 12 over X -45
    // to X 20, the full height Z 20 beyond it; step 2.0 mm.
    let (mut session, index) = session(Case {
        pocket: Some((-45.0, 20.0)),
        source: StockSource::FromRemainingStock,
        ..Case::flat(
            vec![Polygon2::open_path(vec![
                P2::new(-40.0, 0.0),
                P2::new(40.0, 0.0),
            ])],
            vec![
                ("depth_per_pass", json!(2.0)),
                ("lateral_offset", json!(0.6)),
            ],
        )
    })
    .expect("the door takes the keys");
    run_prior(&mut session);
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(index, &cancel)
        .expect("the op generates");
    let tp = session
        .result_at(index)
        .expect("a result")
        .annotated()
        .toolpath
        .clone();
    let fed: Vec<_> = tp
        .moves
        .iter()
        .filter(|m| m.move_type != MoveType::Rapid)
        .collect();
    assert!(fed.len() > 100);
    for m in &fed {
        assert!(
            (m.target.y - 0.6).abs() < 1e-9,
            "a fed move at Y {}",
            m.target.y
        );
    }
    let mut levels: Vec<f64> = Vec::new();
    for m in &fed {
        if (m.target.z * 2.0).fract().abs() < 1e-9
            && m.target.z < STOCK_TOP - 1e-6
            && !levels.iter().any(|z| (z - m.target.z).abs() < 1e-6)
        {
            levels.push(m.target.z);
        }
    }
    levels.sort_by(|a, b| b.total_cmp(a));
    assert_eq!(levels, vec![18.0, 16.0, 14.0, 12.0, 10.0, 8.0]);
    // The air-cut filter: no fed move above the floor over the floor part.
    assert_eq!(
        fed.iter()
            .filter(|m| m.target.z > 12.1 + 1e-6 && m.target.x < 20.0 - TOOL_D - SIM_CELL_MM)
            .count(),
        0
    );
    simulate(&mut session);
    let id = session.toolpath_configs()[index].id;
    let trace = session
        .simulation_result()
        .expect("a simulation")
        .cut_trace
        .clone()
        .expect("a cut trace");
    let deepest = trace
        .samples
        .iter()
        .filter(|s| s.toolpath_id == id && s.is_cutting)
        .map(|s| s.axial_engagement_mm.max(s.plunge_descent_mm))
        .fold(0.0_f64, f64::max);
    assert!(
        deepest > 0.5 && deepest <= 2.0 + SIM_CELL_MM,
        "the deepest sample cuts {deepest:.2} mm; one step is 2.0 mm"
    );
}

/// (g)
#[test]
fn no_key_and_zero_cut_the_same_path_p35f() {
    let moves = |params| {
        let (_, _, tp) = run(Case::flat(
            vec![straight(), Polygon2::new(square(true))],
            params,
        ))
        .expect("generates");
        format!("{:?}", tp.moves)
    };
    let plain = moves(vec![]);
    assert_eq!(plain, moves(vec![("lateral_offset", json!(0.0))]));
    assert_eq!(
        plain,
        moves(vec![
            ("lateral_offset", json!(0.6)),
            ("lateral_offset", serde_json::Value::Null)
        ]),
        "null clears the key"
    );
    let text = toml::to_string(&ProjectCurveConfig::default()).expect("serialises");
    assert!(!text.contains("lateral_offset"), "{text}");
    let refusal = session(Case::flat(
        vec![straight()],
        vec![("lateral_offset", json!(10.5))],
    ))
    .err()
    .expect("outside the range");
    assert!(refusal.contains("lateral_offset"), "{refusal}");
}
