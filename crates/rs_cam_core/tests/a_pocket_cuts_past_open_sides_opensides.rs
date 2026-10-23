//! "Cut past open sides" (operator ruling 2026-10-23, opt-in; references
//! FreeCAD Pocket "Extensions", Fusion 2D Pocket): a Pocket on model faces
//! with the setting on clears the stock beyond each side of a face that has
//! no wall, out to the stock edge. A side against a wall does not move.
//!
//! # The live case
//!
//! Pocket ("Each picked face") then Waterline then a rest Chamfer left a
//! fin of stock (the tool radius) at each outer corner of the lower steps
//! of `stepped_block.step` (`a_step_chamfer_stops_short_of_a_wall_chamferclear`
//! claim (b), KNOWN RED), so the rest Chamfer refused.
//!
//! # The claims
//!
//! (a) On `stepped_block.step` in a stock 5 mm larger on each side, the
//!     default Pocket with the setting on, then the default Chamfer on
//!     "Rest machining: from previous ops", both generate, and the gouge
//!     guard measures each clean. The simulated stock stands no higher
//!     than the model step under it (Z 30, 20, 10), and outside the part
//!     no higher than the lowest face (Z 10): no wall, no corner fin. The
//!     sample reads the highest model point within one sim cell of each
//!     vertex, so a vertex on a step wall is not a false fail.
//! (b) With the setting off the Pocket motion is the motion of a Pocket
//!     whose file has no key (the default), and the file does not write the
//!     key. The motion against HEAD is pinned by
//!     `every_op_keeps_its_toolpath_g_opgolden` and
//!     `a_default_step_pocket_never_cuts_below_a_ring_ux7b`, which do not
//!     move.
//! (c) The side of the Z 20 face against the Z 30 step does not move: at
//!     the Z 20 level no tool centre comes nearer than the tool radius to
//!     the Z 30 block, and the cut touches that wall. The open side at
//!     X 40 moves: the Z 20 level cuts past it.
//! (d) A face with a wall all round (the floor of the blind hole of
//!     `block_with_blind_hole.step`) cuts as with the setting off, and
//!     generation and the selection status give one note "No side of the
//!     face is open; the pocket stays inside the face.". A drawing Pocket
//!     (no face) cuts as with the setting off, with no note.
//! (f) The rule needs the floor "Each picked face": under one floor for
//!     every area (here 5 mm below the stock top) a grown area of the Z 30
//!     face could cut into the lower steps, so nothing grows and the
//!     motion is the motion with the setting off.
//!
//! Red proof: (a) failed with THE area rule made a no-op
//! (`grow_past_open_sides` returning before it grows): the margin of the
//! stock stood at Z 30 next to the part.
//!
//! ```text
//! cargo test -p rs_cam_core -q --features step --test a_pocket_cuts_past_open_sides_opensides
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
use rs_cam_core::compute::floor::Floor;
use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolType};
use rs_cam_core::feeds::suggest::{StockContext, default_operation};
use rs_cam_core::geo::{P2, P3};
use rs_cam_core::ids::ModelId;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::selection::{NO_OPEN_SIDE_TEXT, NoteCause, SelectionEdit, SelectionNote, Shape};
use rs_cam_core::session::{
    AddToolpathArgs, Command, EditSelectionArgs, LoadedModel, ProjectSession,
    ProjectSessionBuilder, SelectionTarget, SimulationOptions, SimulationResolution,
};
use rs_cam_core::toolpath::{MoveType, Toolpath};

const STEPPED: &str = "tests/fixtures/step/stepped_block.step";
const HOLE: &str = "tests/fixtures/step/block_with_blind_hole.step";

/// The model top of the stepped block (mm).
const TOP: f64 = 30.0;
/// The Pocket tool (mm).
const ENDMILL_DIAMETER: f64 = 6.0;
/// The XY margin of the stock round the part (mm), as the chamferclear
/// rest sentry.
const XY_MARGIN: f64 = 5.0;
/// The simulation cell (mm), as the chamferclear rest sentry.
const SIM_CELL: f64 = 0.5;

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

fn vbit() -> ToolConfig {
    let mut t = ToolConfig::new_default(rs_cam_core::ids::ToolId::from_raw(0), ToolType::VBit);
    t.diameter = 12.0;
    t.cutting_length = 10.0;
    t.included_angle = 90.0;
    t.shank_diameter = 6.0;
    t.stickout = 30.0;
    t.flute_count = 2;
    t
}

/// The stepped block stock: the part box X 0..60, Y 0..40, Z 0..30, with
/// `XY_MARGIN` on each side in X and Y.
fn margin_stock() -> StockConfig {
    StockConfig {
        x: 60.0 + 2.0 * XY_MARGIN,
        y: 40.0 + 2.0 * XY_MARGIN,
        z: Some(TOP),
        origin_x: -XY_MARGIN,
        origin_y: -XY_MARGIN,
        origin_z: 0.0,
        auto_from_model: false,
        ..StockConfig::default()
    }
}

fn set_open_sides(op: &mut OperationConfig, on: bool) {
    let OperationConfig::Pocket(cfg) = op else {
        panic!("a Pocket");
    };
    cfg.cut_past_open_sides = on;
}

/// A session on `model` and `stock` with the default Pocket ("Each picked
/// face", auto top) of `tool`, the setting `open_sides`, and, when
/// `chamfer`, the default Chamfer on the remaining stock after it.
fn pocket_session(
    model: &str,
    stock: StockConfig,
    tool_diameter: f64,
    open_sides: bool,
    chamfer: bool,
) -> ProjectSession {
    let mut builder = ProjectSessionBuilder::new().stock(stock);
    let endmill = builder.add_tool(common::tools::endmill_tool_config(tool_diameter));
    let endmill_id = builder.tools()[endmill].id;
    let vbit_idx = builder.add_tool(vbit());
    let vbit_id = builder.tools()[vbit_idx].id;
    let model_id = builder.add_model(load(model));
    let mut s = builder.build();
    let ctx = StockContext::from_stock_bbox(s.stock_bbox(), s.stock_config().padding);
    let mut pocket = default_operation(OperationType::Pocket, &ctx);
    let _ = pocket.set_floor(Floor::EachFace { depth: 0.0 });
    set_open_sides(&mut pocket, open_sides);
    let mut ops = vec![("pocket", pocket, endmill_id)];
    if chamfer {
        ops.push((
            "chamfer",
            default_operation(OperationType::Chamfer, &ctx),
            vbit_id,
        ));
    }
    for (name, op, tool) in ops {
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
    s
}

fn generate(s: &mut ProjectSession, index: usize) -> Toolpath {
    let cancel = AtomicBool::new(false);
    s.generate_toolpath(index, &cancel)
        .map(|r| r.op_data.annotated().toolpath.clone())
        .unwrap_or_else(|e| panic!("op {index} generates: {e}"))
}

fn assert_no_gouge(s: &ProjectSession, index: usize) {
    let finding = s
        .result_at(index)
        .unwrap()
        .stats
        .model_gouge
        .unwrap_or_else(|| panic!("op {index}: the gouge guard ran"));
    assert!(finding.judged > 0, "op {index}: the guard judged samples");
    assert!(!finding.gouges(), "op {index}: {}", finding.message());
}

/// The height of the stepped block at `(x, y)`, or `None` outside the
/// part.
fn step_height(x: f64, y: f64) -> Option<f64> {
    if !(0.0..=60.0).contains(&x) || !(0.0..=40.0).contains(&y) {
        return None;
    }
    Some(if x <= 20.0 {
        30.0
    } else if x <= 40.0 {
        20.0
    } else {
        10.0
    })
}

/// The highest stock the claim allows at `(x, y)`: the highest model step
/// within one sim cell (a vertex on a wall reads the wall top), and the
/// lowest face (Z 10) outside the part.
fn allowed_top(x: f64, y: f64) -> f64 {
    let mut top: f64 = 10.0;
    for dx in [-SIM_CELL, 0.0, SIM_CELL] {
        for dy in [-SIM_CELL, 0.0, SIM_CELL] {
            if let Some(h) = step_height(x + dx, y + dy) {
                top = top.max(h);
            }
        }
    }
    top
}

/// Claim (a), the live case.
#[test]
fn pocket_then_rest_chamfer_leaves_no_fin_opensides() {
    let mut s = pocket_session(STEPPED, margin_stock(), ENDMILL_DIAMETER, true, true);
    let _ = generate(&mut s, 0);
    assert_no_gouge(&s, 0);
    let cancel = AtomicBool::new(false);
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
    // The display mesh of the simulation is stock-relative.
    let mesh = &s.simulation_result().expect("the simulation").mesh;
    let mut worst: Option<(f64, f64, f64, f64)> = None;
    let mut outside = 0;
    for &[vx, vy, vz] in mesh.vertices.as_chunks::<3>().0 {
        let (x, y, z) = (
            f64::from(vx) - XY_MARGIN,
            f64::from(vy) - XY_MARGIN,
            f64::from(vz),
        );
        if step_height(x, y).is_none() {
            outside += 1;
        }
        let over = z - allowed_top(x, y);
        if over > SIM_CELL && worst.is_none_or(|w| over > w.3) {
            worst = Some((x, y, z, over));
        }
    }
    assert!(outside > 0, "non-vacuity: vertices outside the part");
    assert!(
        worst.is_none(),
        "stock stands over the face Z (x, y, z, over): {worst:?}"
    );
    let _ = generate(&mut s, 1);
    assert_no_gouge(&s, 1);
}

/// Claim (b): the setting off is the default, cuts as a file with no key,
/// and writes no key.
#[test]
fn the_setting_off_is_the_default_pocket_opensides() {
    let mut off = pocket_session(STEPPED, margin_stock(), ENDMILL_DIAMETER, false, false);
    let OperationConfig::Pocket(cfg) = &off.toolpath_configs()[0].operation else {
        panic!("a Pocket");
    };
    let text = toml::to_string(cfg).unwrap();
    assert!(!text.contains("cut_past_open_sides"), "{text}");
    let back: rs_cam_core::compute::operation_configs::PocketConfig =
        toml::from_str(&text).unwrap();
    assert!(!back.cut_past_open_sides, "an absent key loads off");
    let mut on_cfg = cfg.clone();
    on_cfg.cut_past_open_sides = true;
    assert!(
        toml::to_string(&on_cfg)
            .unwrap()
            .contains("cut_past_open_sides = true"),
        "the file writes the key when it is on"
    );
    let a = generate(&mut off, 0);
    let mut again = pocket_session(STEPPED, margin_stock(), ENDMILL_DIAMETER, false, false);
    let b = generate(&mut again, 0);
    assert_eq!(format!("{:?}", a.moves), format!("{:?}", b.moves));
    let mut on = pocket_session(STEPPED, margin_stock(), ENDMILL_DIAMETER, true, false);
    let c = generate(&mut on, 0);
    assert_ne!(
        format!("{:?}", a.moves),
        format!("{:?}", c.moves),
        "non-vacuity: the setting changes the motion"
    );
}

/// The cutting XY points at the level `z`, at most `step` apart along
/// each move.
fn cut_points(tp: &Toolpath, z: f64, step: f64) -> Vec<P2> {
    let mut out = Vec::new();
    for pair in tp.moves.windows(2) {
        if matches!(pair[1].move_type, MoveType::Rapid) {
            continue;
        }
        let (p, q): (P3, P3) = (pair[0].target, pair[1].target);
        if (p.z - z).abs() > 1e-6 || (q.z - z).abs() > 1e-6 {
            continue;
        }
        match pair[1].move_type {
            MoveType::ArcCW { i, j, .. } | MoveType::ArcCCW { i, j, .. } => {
                // An arc: from `p` round the centre `p + (i, j)` to `q`.
                let c = P2::new(p.x + i, p.y + j);
                let radius = i.hypot(j);
                let a0 = (p.y - c.y).atan2(p.x - c.x);
                let a1 = (q.y - c.y).atan2(q.x - c.x);
                let ccw = matches!(pair[1].move_type, MoveType::ArcCCW { .. });
                let mut sweep = a1 - a0;
                if ccw && sweep <= 0.0 {
                    sweep += std::f64::consts::TAU;
                }
                if !ccw && sweep >= 0.0 {
                    sweep -= std::f64::consts::TAU;
                }
                let n = (radius * sweep.abs() / step).ceil().max(1.0) as usize;
                for k in 0..=n {
                    let a = a0 + sweep * k as f64 / n as f64;
                    out.push(P2::new(c.x + radius * a.cos(), c.y + radius * a.sin()));
                }
            }
            _ => {
                let len = (q.x - p.x).hypot(q.y - p.y);
                let n = (len / step).ceil().max(1.0) as usize;
                for k in 0..=n {
                    let t = k as f64 / n as f64;
                    out.push(P2::new(p.x + (q.x - p.x) * t, p.y + (q.y - p.y) * t));
                }
            }
        }
    }
    out
}

/// The distance from `p` to the rectangle `[x0, x1] x [y0, y1]` (0 inside).
fn rect_distance(p: P2, x0: f64, x1: f64, y0: f64, y1: f64) -> f64 {
    let dx = (x0 - p.x).max(0.0).max(p.x - x1);
    let dy = (y0 - p.y).max(0.0).max(p.y - y1);
    (dx * dx + dy * dy).sqrt()
}

/// Claim (c): the side against the wall does not move; the open side
/// does.
#[test]
fn a_side_against_a_wall_never_moves_opensides() {
    let mut s = pocket_session(STEPPED, margin_stock(), ENDMILL_DIAMETER, true, false);
    let tp = generate(&mut s, 0);
    let r = ENDMILL_DIAMETER / 2.0;
    let points = cut_points(&tp, 20.0, 0.1);
    assert!(!points.is_empty(), "the Z 20 level is cut");
    // The Z 30 block: X 0..20, Y 0..40.
    let nearest = points
        .iter()
        .map(|&p| rect_distance(p, 0.0, 20.0, 0.0, 40.0))
        .fold(f64::INFINITY, f64::min);
    assert!(
        nearest >= r - 1e-6,
        "a tool centre comes {nearest:.4} mm from the Z 30 block (want >= {r})"
    );
    assert!(
        nearest <= r + 0.02,
        "the cut touches the wall: nearest centre {nearest:.4} mm"
    );
    assert!(
        points
            .iter()
            .any(|p| p.x > 40.0 + r && (0.0..=40.0).contains(&p.y)),
        "the Z 20 level cuts past the open side at X 40"
    );
    assert!(
        points.iter().any(|p| p.y < -r && p.x > 20.0 && p.x < 40.0),
        "the Z 20 level cuts past the open side at Y 0"
    );
}

/// The notes of toolpath `index`: from generation and from the status.
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

fn no_open_side(notes: &[SelectionNote]) -> Vec<&SelectionNote> {
    notes
        .iter()
        .filter(|n| n.cause == NoteCause::NoOpenSide)
        .collect()
}

/// A session of the blind hole block with a Pocket of the hole floor face
/// only, a 3 mm tool (the hole is 6 mm).
fn hole_floor_session(open_sides: bool) -> ProjectSession {
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
    let mut s = pocket_session(HOLE, stock, 3.0, open_sides, false);
    let model = s.models()[0].clone();
    let floor = model
        .items
        .iter()
        .find(|i| {
            matches!(i.shape(), Shape::Face(f)
                if f.normal.z > 0.0 && (f.plane_z - 12.0).abs() < 1e-6)
        })
        .expect("the hole floor face at Z 12");
    let key = floor.key().clone().for_toolpath_model(model.id);
    let _ = s
        .apply(Command::EditSelection(EditSelectionArgs {
            index: 0,
            target: SelectionTarget::Op,
            edit: SelectionEdit::AddPick(key),
        }))
        .expect("the pick");
    s
}

/// Claim (d), a face with a wall all round.
#[test]
fn a_closed_face_cuts_as_before_with_a_note_opensides() {
    let mut off = hole_floor_session(false);
    let a = generate(&mut off, 0);
    let (gen_off, status_off) = notes(&off, 0);
    assert!(no_open_side(&gen_off).is_empty() && no_open_side(&status_off).is_empty());
    let mut on = hole_floor_session(true);
    let b = generate(&mut on, 0);
    assert_eq!(
        format!("{:?}", a.moves),
        format!("{:?}", b.moves),
        "the pocket stays inside the face"
    );
    let (generated, status) = notes(&on, 0);
    let generated = no_open_side(&generated);
    assert_eq!(generated.len(), 1, "{generated:?}");
    assert_eq!(generated, no_open_side(&status), "generation = status");
    assert_eq!(generated[0].to_string(), NO_OPEN_SIDE_TEXT);
    assert_eq!(
        NO_OPEN_SIDE_TEXT,
        "No side of the face is open; the pocket stays inside the face."
    );
    let tc = on.toolpath_configs()[0].clone();
    let report = on.selection_status(&tc, SelectionTarget::Op).unwrap();
    assert!(report.extended.is_empty(), "no area grew");
}

/// Claim (d), a drawing Pocket: no face, so nothing changes and no note.
#[test]
fn a_drawing_pocket_cuts_as_before_opensides() {
    let run = |on: bool| {
        let mut builder = ProjectSessionBuilder::new().stock(StockConfig {
            x: 100.0,
            y: 60.0,
            z: Some(10.0),
            origin_x: -50.0,
            origin_y: -30.0,
            origin_z: -10.0,
            auto_from_model: false,
            ..StockConfig::default()
        });
        let tool = builder.add_tool(common::tools::endmill_tool_config(6.0));
        let tool_id = builder.tools()[tool].id;
        let square = Polygon2::rectangle(-20.0, -10.0, 20.0, 10.0);
        let model_id = builder.add_model(common::session::polygon_model(
            vec![square],
            "opensides_square",
        ));
        let mut op = OperationConfig::Pocket(Default::default());
        set_open_sides(&mut op, on);
        let _ = builder
            .add_toolpath(
                0,
                common::session::toolpath_config("pocket", op, tool_id, model_id),
            )
            .expect("add the Pocket");
        let mut s = builder.build();
        let tp = generate(&mut s, 0);
        let (generated, status) = notes(&s, 0);
        assert!(no_open_side(&generated).is_empty() && no_open_side(&status).is_empty());
        format!("{:?}", tp.moves)
    };
    assert_eq!(run(false), run(true));
}

/// Claim (f): under a floor that is not "Each picked face" the setting
/// grows nothing.
#[test]
fn one_floor_for_every_area_grows_nothing_opensides() {
    let run = |on: bool| {
        let mut s = pocket_session(STEPPED, margin_stock(), ENDMILL_DIAMETER, on, false);
        let mut cfg = s.toolpath_configs()[0].clone();
        let _ = cfg.operation.set_floor(Floor::StockTop { depth: 5.0 });
        let _ = s
            .apply(Command::ReplaceToolpathConfig(
                rs_cam_core::session::ReplaceToolpathConfigArgs {
                    index: 0,
                    config: Box::new(cfg),
                },
            ))
            .expect("the floor edit");
        let tc = s.toolpath_configs()[0].clone();
        let report = s.selection_status(&tc, SelectionTarget::Op).unwrap();
        assert!(report.extended.is_empty(), "no area grows");
        format!("{:?}", generate(&mut s, 0).moves)
    };
    assert_eq!(run(false), run(true));
}
