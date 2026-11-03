//! Chamfer Clearance (operator ruling 2026-10-23, industry default: the
//! Fusion "Chamfer Clearance"): a STEP Chamfer edge run that ends at a
//! higher wall stops short of that wall. It is not blocked, and it does
//! not cut into the wall.
//!
//! # The defect
//!
//! On `stepped_block.step` four convex chains end at a step wall: the Z 20
//! chains at Y 0 and Y 40 meet the Z 30 wall at X 20, and the Z 10 chains
//! meet the Z 20 wall at X 40. The V-bit along such a chain cuts into the
//! wall with its cone, so the gouge guard (`ModelGougeRule::Bevel`) BLOCKED
//! the default Chamfer. On "Rest machining: from previous ops" the buried
//! check read a band along each chain that reached into the higher step,
//! and the Chamfer refused "the face is 10.0 mm below the material that
//! stands over it".
//!
//! # The claims
//!
//! (a) The default Chamfer on fresh stock (Top Z pinned at the model top,
//!     as the R-C motion claims) generates, and the gouge guard measures
//!     it clean. Each of the four runs ends short of its wall by the
//!     clearance length `T = sqrt(R^2 - t^2)` (the full cone radius `R`
//!     reaches the wall top, the tip runs `t` into the air) plus the CAD
//!     rounding. The other end of each run is not trimmed. One note says
//!     "4 edge ends stop short of a wall", in generation and in the
//!     selection status.
//! (b) Pocket, then Waterline, then the default Chamfer on "Rest
//!     machining: from previous ops" (auto top), the live case: the trim
//!     runs on the rest stock as on fresh stock (the same note), and the
//!     stretch it keeps next to the wall is clear. KNOWN RED: the op still
//!     refuses, by a corner of stock that the Pocket leaves at the free end
//!     of each lower chain (see the test; ruling R-D clears it).
//! (c) A run that the trim leaves empty is skipped with a note; the op
//!     cuts the other runs.
//! (d) An edge with no wall is not trimmed: the top edges of
//!     `block_with_blind_hole.step` give no trim note, and the cut runs
//!     cover the full edges.
//!
//! The clearance length is computed here in closed form from the fixture
//! geometry, not read from the code.
//!
//! Red proof: (a) ran red against HEAD 74af0f8e (the guard measured the
//! four wall ends as a gouge).
//!
//! ```text
//! cargo test -p rs_cam_core -q --features step --test a_step_chamfer_stops_short_of_a_wall_chamferclear
//! ```

#![cfg(feature = "step")]
#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

mod common;

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::StockConfig;
use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::config::{HeightMode, StockSource};
use rs_cam_core::compute::execute::{BuriedUnder, OperationError};
use rs_cam_core::compute::floor::Floor;
use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolType};
use rs_cam_core::feeds::suggest::{StockContext, default_operation};
use rs_cam_core::geo::{P2, P3, V2};
use rs_cam_core::ids::{ModelId, ToolId};
use rs_cam_core::selection::{
    CAD_ROUNDING_MM, EdgeForm, EdgeMaterial, ItemKind, NoteCause, SelectionEdit, SelectionNote,
    Shape,
};
use rs_cam_core::session::{
    AddToolpathArgs, Command, EditSelectionArgs, ItemState, LoadedModel, ProjectSession,
    ProjectSessionBuilder, SelectionTarget, SessionError, SimulationOptions, SimulationResolution,
};
use rs_cam_core::toolpath::{MoveType, Toolpath};

const STEPPED: &str = "tests/fixtures/step/stepped_block.step";
const HOLE: &str = "tests/fixtures/step/block_with_blind_hole.step";

/// The model top of the stepped block (mm).
const TOP: f64 = 30.0;
/// The V-bit: 90° included angle, 12 mm diameter.
const VBIT_ANGLE: f64 = 90.0;
const VBIT_DIAMETER: f64 = 12.0;

fn load(rel: &str) -> LoadedModel {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel);
    rs_cam_core::io::load_model_file(
        &path,
        ModelId::from_raw(0),
        ModelKind::Step,
        ModelUnits::Millimeters,
        Default::default(),
    )
    .unwrap_or_else(|e| panic!("{rel}: {e}"))
}

fn vbit(diameter: f64, angle: f64) -> ToolConfig {
    let mut t = ToolConfig::new_default(ToolId::from_raw(0), ToolType::VBit);
    t.diameter = diameter;
    t.cutting_length = 10.0;
    t.included_angle = angle;
    t.shank_diameter = 6.0;
    t.stickout = 30.0;
    t.flute_count = 2;
    t
}

/// The stock of the part box: X 0..60, Y 0..40, Z 0..30.
fn part_stock() -> StockConfig {
    StockConfig {
        x: 60.0,
        y: 40.0,
        z: Some(TOP),
        origin_x: 0.0,
        origin_y: 0.0,
        origin_z: 0.0,
        auto_from_model: false,
        ..StockConfig::default()
    }
}

/// A session with `model`, `tool` and one Chamfer as the add doors make
/// it (`default_operation`), with the Top Z pinned at `top`.
fn chamfer_session(model: &str, tool: ToolConfig, stock: StockConfig, top: f64) -> ProjectSession {
    let mut builder = ProjectSessionBuilder::new().stock(stock);
    let _ = builder.add_tool(tool);
    let _ = builder.add_model(load(model));
    let mut s = builder.build();
    let ctx = StockContext::from_stock_bbox(s.stock_bbox(), s.stock_config().padding);
    let op = default_operation(OperationType::Chamfer, &ctx);
    let mut cfg =
        common::session::toolpath_config("chamfer", op, s.tools()[0].id, s.models()[0].id);
    cfg.heights.top_z = HeightMode::Manual(top);
    let _ = s
        .apply(Command::AddToolpath(AddToolpathArgs {
            setup_index: 0,
            config: Box::new(cfg),
        }))
        .expect("the add");
    s
}

/// The tip depth below the edge `(w + t) / tan(a)` and the tip offset `t`.
fn tip_depth(s: &ProjectSession, index: usize, angle: f64) -> (f64, f64) {
    let OperationConfig::Chamfer(cfg) = &s.toolpath_configs()[index].operation else {
        panic!("a Chamfer");
    };
    let tan = (angle / 2.0).to_radians().tan();
    ((cfg.chamfer_width + cfg.tip_offset) / tan, cfg.tip_offset)
}

fn generate(s: &mut ProjectSession, index: usize) -> Result<Toolpath, SessionError> {
    let cancel = AtomicBool::new(false);
    s.generate_toolpath(index, &cancel)
        .map(|r| r.op_data.annotated().toolpath.clone())
}

/// The horizontal cutting segments of `tp` at the level `z`.
fn level_segments(tp: &Toolpath, z: f64) -> Vec<(P3, P3)> {
    tp.moves
        .windows(2)
        .filter(|pair| !matches!(pair[1].move_type, MoveType::Rapid))
        .map(|pair| (pair[0].target, pair[1].target))
        .filter(|(p, q)| (p.z - z).abs() < 1e-6 && (q.z - z).abs() < 1e-6)
        .collect()
}

/// The X span of the cut on the line `y = y_line` at the level `z`.
fn x_span(tp: &Toolpath, z: f64, y_line: f64) -> Option<(f64, f64)> {
    let xs: Vec<f64> = level_segments(tp, z)
        .into_iter()
        .filter(|(p, q)| (p.y - y_line).abs() < 1e-3 && (q.y - y_line).abs() < 1e-3)
        .flat_map(|(p, q)| [p.x, q.x])
        .collect();
    if xs.is_empty() {
        return None;
    }
    Some((
        xs.iter().copied().fold(f64::INFINITY, f64::min),
        xs.iter().copied().fold(f64::NEG_INFINITY, f64::max),
    ))
}

/// The four chains of the stepped block that end at a step wall: (edge Z,
/// edge Y, the air side of the edge in Y, wall X, free end X).
const WALL_RUNS: [(f64, f64, f64, f64, f64); 4] = [
    (20.0, 0.0, -1.0, 20.0, 40.0),
    (20.0, 40.0, 1.0, 20.0, 40.0),
    (10.0, 0.0, -1.0, 40.0, 60.0),
    (10.0, 40.0, 1.0, 40.0, 60.0),
];

/// Claim (a): the default Chamfer generates clean, and each run that met a
/// wall ends short of it by the clearance length.
#[test]
fn a_default_step_chamfer_stops_short_of_each_wall_chamferclear() {
    let mut s = chamfer_session(STEPPED, vbit(VBIT_DIAMETER, VBIT_ANGLE), part_stock(), TOP);
    let (depth, t) = tip_depth(&s, 0, VBIT_ANGLE);
    let tp = generate(&mut s, 0).expect("the default Chamfer generates");

    let finding = s
        .result_at(0)
        .unwrap()
        .stats
        .model_gouge
        .expect("the guard ran");
    assert!(finding.judged > 0, "non-vacuity: the guard judged samples");
    assert!(!finding.gouges(), "{}", finding.message());

    // The full cone radius reaches the wall top (the wall is 10 mm high,
    // `10 tan(a) > R`), so the tool axis stays `R` from the wall corner.
    // The axis runs `t` into the air from the edge line.
    let r = VBIT_DIAMETER / 2.0;
    let clearance = (r * r - t * t).sqrt();
    for (z, y, air, wall_x, free_x) in WALL_RUNS {
        let level = z - depth;
        let y_line = y + air * t;
        let (lo, hi) =
            x_span(&tp, level, y_line).unwrap_or_else(|| panic!("the run at Z {z} Y {y} is cut"));
        let short = lo - wall_x;
        assert!(
            short >= clearance + CAD_ROUNDING_MM - 1e-6
                && short <= clearance + 3.0 * CAD_ROUNDING_MM,
            "the run at Z {z} Y {y} stops {short:.4} mm from the wall at X {wall_x} \
             (want {clearance:.4} + the CAD rounding)"
        );
        assert!(
            (hi - free_x).abs() < 1e-6,
            "the free end of the run at Z {z} Y {y} is not trimmed: {hi}"
        );
    }
}

/// The notes of toolpath `index`: from generation (the finding) and from
/// the selection status.
fn notes(s: &ProjectSession, index: usize) -> (Vec<SelectionNote>, Vec<SelectionNote>) {
    let generated = s
        .result_at(index)
        .unwrap()
        .stats
        .selection_note
        .clone()
        .map(|f| f.notes)
        .unwrap_or_default();
    let tc = s.toolpath_configs()[index].clone();
    let report = s.selection_status(&tc, SelectionTarget::Op).unwrap();
    (generated, report.notes.clone())
}

fn clearance_notes(notes: &[SelectionNote]) -> Vec<SelectionNote> {
    notes
        .iter()
        .filter(|n| matches!(n.cause, NoteCause::StopsShort { .. } | NoteCause::AtAWall))
        .cloned()
        .collect()
}

/// Claim (a), the text: one note names the four ends and the clearance,
/// the same in generation and in the selection status; each chain is Cut.
#[test]
fn the_stopped_ends_are_named_in_one_note_chamferclear() {
    let mut s = chamfer_session(STEPPED, vbit(VBIT_DIAMETER, VBIT_ANGLE), part_stock(), TOP);
    let (_, t) = tip_depth(&s, 0, VBIT_ANGLE);
    let _ = generate(&mut s, 0).expect("the default Chamfer generates");
    let (generated, status) = notes(&s, 0);
    let generated = clearance_notes(&generated);
    assert_eq!(generated, clearance_notes(&status), "generation = status");
    assert_eq!(generated.len(), 1, "{generated:?}");
    let r = VBIT_DIAMETER / 2.0;
    let clearance = (r * r - t * t).sqrt();
    let NoteCause::StopsShort {
        ends,
        min_mm,
        max_mm,
    } = generated[0].cause
    else {
        panic!("a stop note: {:?}", generated[0]);
    };
    assert_eq!(ends, 4);
    assert_eq!(generated[0].count, 4, "four edges");
    for stop in [min_mm, max_mm] {
        assert!(
            stop >= clearance + CAD_ROUNDING_MM - 1e-6 && stop <= clearance + 3.0 * CAD_ROUNDING_MM,
            "{stop} against {clearance}"
        );
    }
    let text = generated[0].to_string();
    assert!(
        text.starts_with("4 edge ends stop short of a wall by "),
        "{text}"
    );
    assert!(
        text.contains("so the V-bit does not cut into the wall"),
        "{text}"
    );
    let tc = s.toolpath_configs()[0].clone();
    let report = s.selection_status(&tc, SelectionTarget::Op).unwrap();
    let cut = report
        .item_states
        .iter()
        .filter(|st| **st == ItemState::Cut)
        .count();
    assert_eq!(cut, 10, "every convex chain is still cut");
}

/// The Pocket tool diameter and the XY margin of the stock (mm). The
/// margin is less than the tool diameter, so one Waterline ring clears it.
const ENDMILL_DIAMETER: f64 = 6.0;
const XY_MARGIN: f64 = 5.0;
/// The simulation cell (mm), as the UX-1c rest sentry.
const SIM_CELL: f64 = 0.5;

/// Claim (b), the live case: Pocket ("Each picked face"), then Waterline,
/// then the default Chamfer on "Rest machining: from previous ops" (auto
/// top).
///
/// - The trim runs on the rest stock as on fresh stock: the rest Chamfer
///   holds the same note of four stopped ends, and the stretch that the
///   trim keeps next to the X 20 wall is clear stock (Z 20).
/// - KNOWN RED (an inverted claim, `tests/CLAUDE.md`): the rest Chamfer
///   still refuses `BuriedFace` by 10 mm of standing material. The cause
///   is not the wall end. The Pocket of each face ring leaves a corner of
///   stock (the tool radius) at each outer corner of the Z 20 and Z 10
///   rings, for example X 37..40 at Y 0, up to the stock top Z 30, and
///   the Waterline does not reach it (at Z 20..30 it runs round the Z 30
///   step only). The free end of each lower chain is under that corner.
///   Ruling R-D (a face pocket crosses its open edges to the stock edge)
///   clears it; when R-D lands, this arm fails and becomes "generates".
#[test]
fn a_rest_chamfer_after_pocket_and_waterline_chamferclear() {
    let stock = StockConfig {
        x: 60.0 + 2.0 * XY_MARGIN,
        y: 40.0 + 2.0 * XY_MARGIN,
        z: Some(TOP),
        origin_x: -XY_MARGIN,
        origin_y: -XY_MARGIN,
        origin_z: 0.0,
        auto_from_model: false,
        ..StockConfig::default()
    };
    let mut builder = ProjectSessionBuilder::new().stock(stock);
    let endmill = builder.add_tool(common::tools::endmill_tool_config(ENDMILL_DIAMETER));
    let endmill_id = builder.tools()[endmill].id;
    let vbit_idx = builder.add_tool(vbit(VBIT_DIAMETER, VBIT_ANGLE));
    let vbit_id = builder.tools()[vbit_idx].id;
    let model_id = builder.add_model(load(STEPPED));
    let mut s = builder.build();
    let ctx = StockContext::from_stock_bbox(s.stock_bbox(), s.stock_config().padding);
    let mut pocket = default_operation(OperationType::Pocket, &ctx);
    let _ = pocket.set_floor(Floor::EachFace { depth: 0.0 });
    let waterline = default_operation(OperationType::Waterline, &ctx);
    let chamfer = default_operation(OperationType::Chamfer, &ctx);
    for (name, op, tool) in [
        ("pocket", pocket, endmill_id),
        ("waterline", waterline, endmill_id),
        ("chamfer", chamfer, vbit_id),
    ] {
        let mut cfg = common::session::toolpath_config(name, op, tool, model_id);
        cfg.heights.top_z = HeightMode::Auto;
        if name == "chamfer" {
            cfg.stock_source = StockSource::FromRemainingStock;
        }
        let _ = s
            .apply(Command::AddToolpath(AddToolpathArgs {
                setup_index: 0,
                config: Box::new(cfg),
            }))
            .expect("the add");
    }
    let cancel = AtomicBool::new(false);
    for index in 0..2 {
        let _ = generate(&mut s, index).unwrap_or_else(|e| panic!("op {index}: {e}"));
    }
    let _ = s
        .run_simulation(
            &SimulationOptions {
                resolution: SimulationResolution::Fixed(SIM_CELL),
                adaptive_feed_modulation: false,
                ..SimulationOptions::default()
            },
            &cancel,
        )
        .expect("the simulation completes");

    // The trim on the rest stock: the same note as on fresh stock.
    let tc = s.toolpath_configs()[2].clone();
    let report = s.selection_status(&tc, SelectionTarget::Op).unwrap();
    let stops = clearance_notes(&report.notes);
    assert_eq!(stops.len(), 1, "{:?}", report.notes);
    assert!(
        matches!(stops[0].cause, NoteCause::StopsShort { ends: 4, .. }),
        "{:?}",
        stops[0]
    );

    // The stock is clear next to the X 20 wall, where the trimmed run
    // starts (the display mesh of the simulation, stock-relative).
    let mesh = &s.simulation_result().expect("the simulation").mesh;
    let top_near = |x: f64, y: f64, half: f64| {
        mesh.vertices
            .as_chunks::<3>()
            .0
            .iter()
            .map(|&[vx, vy, vz]| {
                (
                    f64::from(vx) - XY_MARGIN,
                    f64::from(vy) - XY_MARGIN,
                    f64::from(vz),
                )
            })
            .filter(|(vx, vy, _)| (vx - x).abs() <= half && (vy - y).abs() <= half)
            .map(|(_, _, vz)| vz)
            .fold(f64::NEG_INFINITY, f64::max)
    };
    let near_wall = top_near(27.0, 0.5, 0.6);
    assert!(
        near_wall.is_finite() && near_wall <= 20.0 + SIM_CELL,
        "the trimmed run starts on clear stock: top {near_wall}"
    );
    // KNOWN RED: the corner buries the free end of the Z 20 chain.
    match generate(&mut s, 2) {
        Err(SessionError::Operation(OperationError::BuriedFace(refusal))) => {
            assert_eq!(refusal.under, BuriedUnder::StandingMaterial, "{refusal}");
            assert!(
                (refusal.buried_mm - 10.0).abs() < SIM_CELL,
                "the corner stands 10 mm over the Z 20 edge: {refusal}"
            );
        }
        Err(other) => panic!("want the buried refusal of the corner, got: {other}"),
        Ok(_) => panic!(
            "the rest Chamfer generates: the corner is cleared (R-D?). Re-pin this arm as \
             \"generates\" and check the guard."
        ),
    }
}

/// Claim (c): a V-bit wide enough to reach a wall along all of an edge
/// leaves that edge out with a note, and cuts the edges that stay clear.
///
/// A 150° bit of 44 mm: the cone reaches `(H tan 75°) + t` (37 mm for a
/// 10 mm wall) and is cut off at `R` = 22 mm, more than the 20 mm length
/// of each lower chain. So the six chains at Z 20 and Z 10 have no clear
/// stretch; the four Z 30 chains have no higher material.
#[test]
fn an_edge_with_no_clear_stretch_is_skipped_with_a_note_chamferclear() {
    let angle = 150.0;
    let mut s = chamfer_session(STEPPED, vbit(44.0, angle), part_stock(), TOP);
    let (depth, _) = tip_depth(&s, 0, angle);
    let tp = generate(&mut s, 0).expect("the Z 30 chains generate");
    let finding = s.result_at(0).unwrap().stats.model_gouge.unwrap();
    assert!(!finding.gouges(), "{}", finding.message());
    let (generated, status) = notes(&s, 0);
    let generated = clearance_notes(&generated);
    assert_eq!(generated, clearance_notes(&status), "generation = status");
    let at_a_wall: Vec<&SelectionNote> = generated
        .iter()
        .filter(|n| n.cause == NoteCause::AtAWall)
        .collect();
    assert_eq!(at_a_wall.len(), 1, "{generated:?}");
    assert_eq!(at_a_wall[0].count, 6);
    assert_eq!(at_a_wall[0].kind, ItemKind::Path);
    let text = at_a_wall[0].to_string();
    assert!(
        text.starts_with("6 open paths are not cut: the V-bit reaches a wall along all of it."),
        "{text}"
    );
    let tc = s.toolpath_configs()[0].clone();
    let report = s.selection_status(&tc, SelectionTarget::Op).unwrap();
    for (item, state) in s.models()[0].items.iter().zip(&report.item_states) {
        if !(item.kind() == ItemKind::Path
            && item
                .model_edge()
                .is_some_and(|e| !e.ring && e.is_convex_top()))
        {
            continue;
        }
        let want = if (item.z().unwrap() - TOP).abs() < 1e-9 {
            ItemState::Cut
        } else {
            ItemState::Skipped
        };
        assert_eq!(*state, want, "the chain at Z {:?}", item.z());
    }
    // Only the Z 30 level is cut.
    let levels: Vec<f64> = tp
        .moves
        .windows(2)
        .filter(|pair| !matches!(pair[1].move_type, MoveType::Rapid))
        .filter(|pair| (pair[0].target.z - pair[1].target.z).abs() < 1e-9)
        .map(|pair| pair[1].target.z)
        .filter(|z| *z < TOP - depth + 1e-6)
        .collect();
    assert!(!levels.is_empty());
    assert!(
        levels.iter().all(|z| (z - (TOP - depth)).abs() < 1e-6),
        "{levels:?}"
    );
}

/// Claim (d): an edge with no wall is not trimmed. The default Chamfer on
/// the blind hole block (top Z 20) gives no clearance note, every served
/// item is Cut, and the cut reaches each point of each edge.
#[test]
fn an_edge_with_no_wall_is_not_trimmed_chamferclear() {
    let stock = StockConfig {
        x: 40.0,
        y: 30.0,
        z: Some(20.0),
        origin_x: 0.0,
        origin_y: 0.0,
        origin_z: 0.0,
        auto_from_model: false,
        ..StockConfig::default()
    };
    let mut s = chamfer_session(HOLE, vbit(VBIT_DIAMETER, VBIT_ANGLE), stock, 20.0);
    let (depth, t) = tip_depth(&s, 0, VBIT_ANGLE);
    let tp = generate(&mut s, 0).expect("the default Chamfer generates");
    let finding = s.result_at(0).unwrap().stats.model_gouge.unwrap();
    assert!(!finding.gouges(), "{}", finding.message());
    let (generated, status) = notes(&s, 0);
    assert!(clearance_notes(&generated).is_empty(), "{generated:?}");
    assert!(clearance_notes(&status).is_empty(), "{status:?}");
    let tc = s.toolpath_configs()[0].clone();
    let report = s.selection_status(&tc, SelectionTarget::Op).unwrap();
    let model = s.models()[0].clone();
    let segments = level_segments(&tp, 20.0 - depth);
    let mut checked = 0;
    for (item, state) in model.items.iter().zip(&report.item_states) {
        if *state == ItemState::NotSelected {
            continue;
        }
        assert_eq!(
            *state,
            ItemState::Cut,
            "{:?} at Z {:?}",
            item.kind(),
            item.z()
        );
        let Some(EdgeForm::ConvexTop { material }) = item.model_edge().map(|e| e.form) else {
            continue;
        };
        // Each straight edge: its two ends, moved `t` into the air.
        let Shape::Path(p) = item.shape() else {
            continue;
        };
        for pair in p.exterior.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            let d = (b - a).normalize();
            let left = V2::new(-d.y, d.x);
            let air = match material {
                EdgeMaterial::Left => -left,
                EdgeMaterial::Right | EdgeMaterial::Inside | EdgeMaterial::Outside => left,
            };
            for end in [a, b] {
                let want = end + air * t;
                let hit = segments
                    .iter()
                    .any(|(p, q)| dist_xy(want, P2::new(p.x, p.y), P2::new(q.x, q.y)) < 1e-3);
                assert!(hit, "the cut reaches {want:?} (edge {a:?} to {b:?})");
                checked += 1;
            }
        }
    }
    assert!(checked > 0, "non-vacuity: straight edges checked");
}

fn dist_xy(p: P2, a: P2, b: P2) -> f64 {
    let ab = b - a;
    let len2 = ab.norm_squared();
    let u = if len2 > 0.0 {
        ((p - a).dot(&ab) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (p - (a + ab * u)).norm()
}

/// Point 4 of the ruling: a picked chain gets the same trim. The trim keeps
/// the tool out of the model; it is not a default, so a pick does not
/// refuse. The Z 20 chain at Y 0, picked alone, stops short of the X 20
/// wall by the clearance length, and it is Cut.
#[test]
fn a_picked_chain_stops_short_as_the_default_does_chamferclear() {
    let mut s = chamfer_session(STEPPED, vbit(VBIT_DIAMETER, VBIT_ANGLE), part_stock(), TOP);
    let (depth, t) = tip_depth(&s, 0, VBIT_ANGLE);
    let model = s.models()[0].clone();
    let chain = model
        .items
        .iter()
        .find(|i| {
            i.kind() == ItemKind::Path
                && i.model_edge().is_some_and(|e| !e.ring && e.is_convex_top())
                && (i.z().unwrap() - 20.0).abs() < 1e-9
                && matches!(i.shape(), Shape::Path(p)
                    if p.exterior.iter().all(|q| q.y.abs() < 1e-9))
        })
        .expect("the Z 20 chain at Y 0");
    let key = chain.key().clone().for_toolpath_model(model.id);
    // W3h: a pick on a new op (nothing selected) is the selection. The
    // fixture op holds the drawing rule, so the case writes that one pick.
    let _ = s
        .apply(Command::EditSelection(EditSelectionArgs {
            index: 0,
            target: SelectionTarget::Op,
            edit: SelectionEdit::Replace(Box::new(rs_cam_core::selection::Selection::from_picks(
                vec![key],
            ))),
        }))
        .expect("the pick");
    let tp = generate(&mut s, 0).expect("the picked chain generates");
    let finding = s.result_at(0).unwrap().stats.model_gouge.unwrap();
    assert!(finding.judged > 0);
    assert!(!finding.gouges(), "{}", finding.message());
    let r = VBIT_DIAMETER / 2.0;
    let clearance = (r * r - t * t).sqrt();
    let (lo, hi) = x_span(&tp, 20.0 - depth, -t).expect("the chain is cut");
    let short = lo - 20.0;
    assert!(
        short >= clearance + CAD_ROUNDING_MM - 1e-6 && short <= clearance + 3.0 * CAD_ROUNDING_MM,
        "{short} against {clearance}"
    );
    assert!((hi - 40.0).abs() < 1e-6, "{hi}");
    let tc = s.toolpath_configs()[0].clone();
    let report = s.selection_status(&tc, SelectionTarget::Op).unwrap();
    assert!(report.refusal.is_none(), "{:?}", report.refusal);
    let cut = report
        .item_states
        .iter()
        .filter(|st| **st == ItemState::Cut)
        .count();
    assert_eq!(cut, 1, "the one picked chain is Cut");
}
