//! R-A (operator ruling 2026-10-23; the Kiri:Moto "Outline" and FreeCAD
//! Profile default): a mesh model (STEP or STL) gives ONE part-outline
//! `Loop` at the model bottom Z, and the default Profile on a mesh model
//! cuts that outline only, Outside, to the model bottom, with no tabs.
//!
//! # The claims
//!
//! 1. The outline of `stepped_block.step` is its box: 4 points, the corners
//!    of the model bounding box within `CAD_ROUNDING_MM`, at the model
//!    bottom Z, bound to the model, last in `items`, first point at the
//!    lowest Y then X. The outline of an STL (the stepped mesh of P4.7,
//!    welded by the STL door) is its 20 x 10 rectangle at Z 5, and it
//!    stays beside the flat faces when a selection reads the faces.
//! 2. Two imports give equal keys and one point order (STEP and STL).
//! 3. The default Profile (THE draft door, then `AddToolpath`) on the
//!    stepped block cuts only the outline: one selected row, the outline,
//!    side Outside, no tabs, the floor "Model bottom" depth 0 with its
//!    note, and the report note `PART_FREE_TEXT`. It generates; the guard
//!    measures it clean; the deepest cut is at the model bottom; every
//!    cutting sample lies outside the part by the tool radius.
//! 4. Every area or edge op of the registry refuses a pick of the outline
//!    (`PartOutlineIsThePart`), and the default of every op but Profile
//!    Outside does not take it. The default Pocket keeps its 3 face rings.
//! 5. A turned setup (`Bottom`) maps the outline to the part bottom of
//!    that setup: the row Z equals the resolved "Model bottom" floor Z.
//! 6. `list_model_items` gives the outline row with `outline` set.
//!
//! Red proofs (run by hand, 2026-10-23; each went red, then was restored):
//! - no outline item (the model door as at HEAD a68cf423): 7 of 8 fail,
//!   all but the Pocket claim;
//! - `served_by_default` keeps the face rings for Profile Outside: claim 3
//!   (the face rings are rows too);
//! - `view` does not refuse a picked outline: claim 4.
//!
//! ```text
//! cargo test -p rs_cam_core -q --features step,test-support --test a_mesh_model_gives_its_part_outline_ra
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

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use common::tools::endmill_tool_config;
use rs_cam_core::compute::StockConfig;
use rs_cam_core::compute::catalog::{OperationConfig, OperationType, OutlineUse};
use rs_cam_core::compute::floor::Floor;
use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits};
use rs_cam_core::compute::transform::FaceUp;
use rs_cam_core::geo::P3;
use rs_cam_core::ids::ModelId;
use rs_cam_core::mesh::TriangleMesh;
use rs_cam_core::selection::{
    CAD_ROUNDING_MM, Item, ItemKind, PART_FREE_TEXT, RefusalCause, SelectionEdit, Shape, SourceRef,
};
use rs_cam_core::session::{
    AddToolpathArgs, Command, EditSelectionArgs, Effects, LoadedModel, ProjectSession,
    ProjectSessionBuilder, SelectionTarget, SessionError, SetSetupFaceArgs, ToolpathConfig,
};
use rs_cam_core::toolpath::{MoveType, Toolpath};

const STEPPED: &str = "tests/fixtures/step/stepped_block.step";
/// The part box of the stepped block (mm): X 0..60, Y 0..40, Z 0..30.
const PART: [f64; 4] = [0.0, 0.0, 60.0, 40.0];
const MODEL_TOP: f64 = 30.0;
const TOOL_D: f64 = 6.0;

fn load_step() -> LoadedModel {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(STEPPED);
    rs_cam_core::io::load_model_file(
        &path,
        ModelId::from_raw(0),
        ModelKind::Step,
        ModelUnits::Millimeters,
        Default::default(),
    )
    .unwrap()
}

/// Write `mesh` as a binary STL.
fn write_stl(mesh: &TriangleMesh, path: &Path) {
    let vertex = |i: u32| {
        let p = mesh.vertices[i as usize];
        stl_io::Vector([p.x as f32, p.y as f32, p.z as f32])
    };
    let triangles: Vec<stl_io::Triangle> = mesh
        .triangles
        .iter()
        .map(|t| stl_io::Triangle {
            normal: stl_io::Vector([0.0, 0.0, 0.0]),
            vertices: [vertex(t[0]), vertex(t[1]), vertex(t[2])],
        })
        .collect();
    let mut file = std::fs::File::create(path).unwrap();
    stl_io::write_stl(&mut file, triangles.iter()).unwrap();
}

/// Two loads of the stepped mesh (`common::meshes::stepped_top`) as an STL.
fn load_stl_twice() -> (LoadedModel, LoadedModel) {
    let dir = std::env::temp_dir().join(format!("rs_cam_ra_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("stepped.stl");
    write_stl(&common::meshes::stepped_top(), &path);
    let load = || LoadedModel::from_file(ModelId::from_raw(0), "stepped", &path, None, None, &dir);
    let (a, b) = (load().unwrap(), load().unwrap());
    let _ = std::fs::remove_dir_all(&dir);
    (a, b)
}

/// The one part-outline item of `model`.
fn outline_of(model: &LoadedModel) -> (usize, Item) {
    let hits: Vec<(usize, &Item)> = model
        .items
        .iter()
        .enumerate()
        .filter(|(_, i)| i.is_part_outline())
        .collect();
    assert_eq!(hits.len(), 1, "one part outline");
    (hits[0].0, hits[0].1.clone())
}

fn ring_points(item: &Item) -> Vec<[f64; 2]> {
    let Shape::Loop(ring) = item.shape() else {
        panic!("the outline is a loop");
    };
    ring.exterior.iter().map(|p| [p.x, p.y]).collect()
}

#[track_caller]
fn assert_ring(got: &[[f64; 2]], want: &[[f64; 2]]) {
    assert_eq!(got.len(), want.len(), "{got:?}");
    for (g, w) in got.iter().zip(want) {
        assert!(
            (g[0] - w[0]).abs() <= CAD_ROUNDING_MM && (g[1] - w[1]).abs() <= CAD_ROUNDING_MM,
            "{got:?} != {want:?}"
        );
    }
}

/// Claim 1, STEP.
#[test]
fn the_outline_of_the_stepped_block_is_its_box_ra() {
    let model = load_step();
    let (at, outline) = outline_of(&model);
    assert_eq!(at, model.items.len() - 1, "the outline is the last item");
    assert_eq!(outline.kind(), ItemKind::Loop);
    assert_eq!(outline.source(), SourceRef::Model(ModelId::from_raw(0)));
    let bbox = model.mesh.as_ref().unwrap().bbox;
    assert_eq!(outline.z(), Some(bbox.min.z));
    assert_eq!(outline.part_outline().unwrap().top_z, bbox.max.z);
    assert!((bbox.max.z - MODEL_TOP).abs() <= CAD_ROUNDING_MM);
    let [x0, y0, x1, y1] = PART;
    assert_ring(
        &ring_points(&outline),
        &[[x0, y0], [x1, y0], [x1, y1], [x0, y1]],
    );
    assert!(
        outline.model_edge().is_none(),
        "the outline is no face ring"
    );
}

/// Claim 1, STL: exact, and beside the flat faces.
#[test]
fn the_outline_of_an_stl_is_exact_and_keeps_its_place_beside_the_faces_ra() {
    let (model, _) = load_stl_twice();
    let (at, outline) = outline_of(&model);
    assert_eq!(
        (at, model.items.len()),
        (0, 1),
        "an STL holds the outline only"
    );
    assert_eq!(outline.z(), Some(5.0));
    assert_eq!(outline.part_outline().unwrap().top_z, 8.0);
    assert_ring(
        &ring_points(&outline),
        &[[0.0, 0.0], [20.0, 0.0], [20.0, 10.0], [0.0, 10.0]],
    );
    let faces = model.face_items_built();
    assert!(faces.iter().next().unwrap().is_part_outline(), "index 0");
    assert_eq!(
        faces.iter().filter(|i| i.kind() == ItemKind::Face).count(),
        2,
        "the two flat faces follow the outline"
    );
}

/// Claim 2.
#[test]
fn two_imports_give_one_outline_ra() {
    let (a, b) = (load_step(), load_step());
    let (_, oa) = outline_of(&a);
    let (_, ob) = outline_of(&b);
    assert_eq!(oa.key(), ob.key());
    assert_eq!(ring_points(&oa), ring_points(&ob), "one point order");
    let (a, b) = load_stl_twice();
    let (_, oa) = outline_of(&a);
    let (_, ob) = outline_of(&b);
    assert_eq!(oa.key(), ob.key());
    assert_eq!(ring_points(&oa), ring_points(&ob), "one point order");
}

/// The stock of the stepped block: X -5..65, Y -5..45, Z 0..30.
fn stepped_stock() -> StockConfig {
    StockConfig {
        x: 70.0,
        y: 50.0,
        z: Some(MODEL_TOP),
        origin_x: -5.0,
        origin_y: -5.0,
        origin_z: 0.0,
        auto_from_model: false,
        ..StockConfig::default()
    }
}

fn stepped_session() -> ProjectSession {
    let mut builder = ProjectSessionBuilder::new().stock(stepped_stock());
    let _ = builder.add_tool(endmill_tool_config(TOOL_D));
    let _ = builder.add_model(load_step());
    builder.build()
}

/// Add `op_type` as every add door does: THE draft door, then the
/// `AddToolpath` row. The fixture states the feeds that a tool with no
/// feeds data leaves unset; the floor stays as the door leaves it.
fn add_as_the_doors_do(session: &mut ProjectSession, op_type: OperationType) -> Effects {
    let tool_id = session.tools()[0].id;
    let model_id = session.models()[0].id;
    let mut cfg = session
        .draft_toolpath(op_type, tool_id, model_id, op_type.label().to_owned())
        .expect("the tool fits the op")
        .config;
    let floor = cfg.operation.floor();
    rs_cam_core::compute::fixture_values::fill_unset(&mut cfg.operation);
    if let Some(floor) = floor {
        let _ = cfg.operation.set_floor(floor);
    }
    rs_cam_core::compute::fixture_values::fill_unset_dressups(&mut cfg.dressups);
    session
        .apply(Command::AddToolpath(AddToolpathArgs {
            setup_index: 0,
            config: Box::new(cfg),
        }))
        .expect("the add")
}

fn generate(session: &mut ProjectSession, index: usize) -> Toolpath {
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(index, &cancel)
        .unwrap_or_else(|e| panic!("generation: {e}"))
        .op_data
        .annotated()
        .toolpath
        .clone()
}

/// Points along every cutting move, at most 0.5 mm apart. An arc is
/// sampled on the arc (from the target before it round the centre
/// `start + (i, j)`); a line along the line.
fn cut_samples(tp: &Toolpath) -> Vec<P3> {
    let mut out = Vec::new();
    for pair in tp.moves.windows(2) {
        let (a, b) = (pair[0].target, pair[1].target);
        match pair[1].move_type {
            MoveType::Rapid => {}
            MoveType::ArcCW { i, j, .. } | MoveType::ArcCCW { i, j, .. } => {
                let (cx, cy) = (a.x + i, a.y + j);
                let radius = i.hypot(j);
                let a0 = (a.y - cy).atan2(a.x - cx);
                let a1 = (b.y - cy).atan2(b.x - cx);
                let ccw = matches!(pair[1].move_type, MoveType::ArcCCW { .. });
                let mut sweep = a1 - a0;
                if ccw && sweep <= 0.0 {
                    sweep += std::f64::consts::TAU;
                }
                if !ccw && sweep >= 0.0 {
                    sweep -= std::f64::consts::TAU;
                }
                let n = (radius * sweep.abs() / 0.5).ceil().max(1.0) as usize;
                for k in 0..=n {
                    let t = k as f64 / n as f64;
                    let ang = a0 + sweep * t;
                    out.push(P3::new(
                        cx + radius * ang.cos(),
                        cy + radius * ang.sin(),
                        a.z + (b.z - a.z) * t,
                    ));
                }
            }
            MoveType::Linear { .. } => {
                let n = ((b - a).norm() / 0.5).ceil().max(1.0) as usize;
                for k in 0..=n {
                    out.push(a + (b - a) * (k as f64 / n as f64));
                }
            }
        }
    }
    out
}

/// The distance from `(x, y)` to the part box (0 inside it).
fn distance_to_part(x: f64, y: f64) -> f64 {
    let [x0, y0, x1, y1] = PART;
    let dx = (x0 - x).max(x - x1).max(0.0);
    let dy = (y0 - y).max(y - y1).max(0.0);
    dx.hypot(dy)
}

fn profile_config(s: &ProjectSession) -> &rs_cam_core::compute::operation_configs::ProfileConfig {
    let OperationConfig::Profile(cfg) = &s.toolpath_configs()[0].operation else {
        panic!("a Profile");
    };
    cfg
}

/// Claim 3.
#[test]
fn the_default_mesh_profile_cuts_the_outline_to_the_bottom_ra() {
    let mut s = stepped_session();
    let effects = add_as_the_doors_do(&mut s, OperationType::Profile);
    assert_eq!(effects.floor_set, Some(Floor::ModelBottom { depth: 0.0 }));
    assert_eq!(
        effects.floor_set_note(),
        Some(Effects::MODEL_BOTTOM_FLOOR_NOTE)
    );
    let cfg = profile_config(&s);
    assert_eq!(cfg.side, rs_cam_core::ops::profile::ProfileSide::Outside);
    assert_eq!((cfg.tab_count, cfg.tab_points.len()), (0, 0), "no tabs");
    assert_eq!(cfg.floor, Floor::ModelBottom { depth: 0.0 });

    let tc = s.toolpath_configs()[0].clone();
    let report = s.selection_status(&tc, SelectionTarget::Op).unwrap();
    assert_eq!(report.refusal, None);
    assert_eq!(report.rows.len(), 1, "the outline only: {:?}", report.rows);
    assert!(report.rows[0].outline);
    assert!(
        report.notes.iter().any(|n| n.to_string() == PART_FREE_TEXT),
        "the report note: {:?}",
        report.notes
    );

    let tp = generate(&mut s, 0);
    let stats = &s.result_at(0).unwrap().stats;
    let finding = stats.model_gouge.expect("the guard ran");
    // The guard ran and found no gouge. It judges a sample only where the
    // probe (the tool less `MODEL_GOUGE_TOL_MM`) touches the mesh; each
    // sample here is one tool radius off the part, so it judges none, and
    // the distance check below is the stronger statement.
    assert!(!finding.gouges(), "{}", finding.message());
    assert!(
        stats
            .selection_note
            .as_ref()
            .is_some_and(|f| f.notes.iter().any(|n| n.to_string() == PART_FREE_TEXT)),
        "generation gives the note too"
    );
    let samples = cut_samples(&tp);
    let deepest = samples.iter().map(|p| p.z).fold(f64::INFINITY, f64::min);
    assert!(
        deepest.abs() <= 1e-6,
        "the deepest cut is the model bottom: {deepest}"
    );
    let worst = samples
        .iter()
        .map(|p| distance_to_part(p.x, p.y))
        .fold(f64::INFINITY, f64::min);
    assert!(
        worst >= TOOL_D / 2.0 - 1e-6,
        "a cutting sample lies {worst} mm from the part"
    );
}

/// The registry ops that hold a selection, each with its config.
fn selection_ops() -> Vec<OperationConfig> {
    let mut ops: Vec<OperationConfig> = OperationType::ALL
        .iter()
        .map(|t| OperationConfig::new_default(*t))
        .filter(|op| op.selection().is_some())
        .collect();
    for side in [
        rs_cam_core::ops::profile::ProfileSide::Inside,
        rs_cam_core::ops::profile::ProfileSide::On,
    ] {
        let mut op = OperationConfig::new_default(OperationType::Profile);
        if let OperationConfig::Profile(cfg) = &mut op {
            cfg.side = side;
        }
        ops.push(op);
    }
    ops
}

/// Claim 4: one test over the registry.
#[test]
fn no_area_op_takes_the_part_outline_ra() {
    let model = load_step();
    let (_, outline) = outline_of(&model);
    let key = outline.key().clone().for_toolpath_model(model.id);
    let mut refusing = 0;
    for op in selection_ops() {
        let name = format!("{:?} {:?}", op.op_type(), op.outline_use());
        let mut s = stepped_session();
        let tool_id = s.tools()[0].id;
        let mut cfg = common::session::toolpath_config(&name, op.clone(), tool_id, model.id);
        cfg.heights.top_z = rs_cam_core::compute::config::HeightMode::Auto;
        let _ = s
            .apply(Command::AddToolpath(AddToolpathArgs {
                setup_index: 0,
                config: Box::new(cfg),
            }))
            .expect("the add");
        let tc = s.toolpath_configs()[0].clone();
        let report = s.selection_status(&tc, SelectionTarget::Op).unwrap();
        let takes = report.rows.iter().any(|r| r.outline);
        assert_eq!(
            takes,
            op.outline_use() == OutlineUse::Default,
            "{name}: the default takes the outline"
        );
        if op.outline_use() != OutlineUse::Refuse {
            continue;
        }
        refusing += 1;
        let _ = s
            .apply(Command::EditSelection(EditSelectionArgs {
                index: 0,
                target: SelectionTarget::Op,
                edit: SelectionEdit::AddPick(key.clone()),
            }))
            .expect("the pick edit applies");
        let tc = s.toolpath_configs()[0].clone();
        let report = s.selection_status(&tc, SelectionTarget::Op).unwrap();
        let refusal = report.refusal.as_ref().unwrap_or_else(|| panic!("{name}"));
        assert_eq!(refusal.cause, RefusalCause::PartOutlineIsThePart, "{name}");
        let cancel = AtomicBool::new(false);
        match s.generate_toolpath(0, &cancel) {
            Err(SessionError::Selection(r)) => {
                assert_eq!(r.cause, RefusalCause::PartOutlineIsThePart, "{name}");
            }
            Err(other) => panic!("{name}: another refusal: {other}"),
            Ok(_) => panic!("{name}: generation cut the part outline"),
        }
    }
    // Pocket, Face, 2D Adaptive, Zigzag, V-Carve, Inlay, Inlay Plug,
    // Chamfer, Drill, Bore, Pin Drill, Profile Inside, Profile On.
    assert_eq!(refusing, 13, "non-vacuity: every refusing op ran");
}

/// Claim 4: the default Pocket keeps its 3 face rings.
#[test]
fn the_default_pocket_keeps_its_three_rings_ra() {
    let mut s = stepped_session();
    let _ = add_as_the_doors_do(&mut s, OperationType::Pocket);
    let tc = s.toolpath_configs()[0].clone();
    let report = s.selection_status(&tc, SelectionTarget::Op).unwrap();
    assert_eq!(report.rows.len(), 3, "{:?}", report.rows);
    assert!(report.rows.iter().all(|r| !r.outline));
}

/// Claim 5: a `Bottom` setup sees the model top as the part bottom.
#[test]
fn a_turned_setup_puts_the_outline_at_its_part_bottom_ra() {
    let mut s = stepped_session();
    let _ = add_as_the_doors_do(&mut s, OperationType::Profile);
    let _ = s
        .apply(Command::SetSetupFace(SetSetupFaceArgs {
            setup_index: 0,
            face_up: FaceUp::Bottom,
        }))
        .unwrap();
    let tc: ToolpathConfig = s.toolpath_configs()[0].clone();
    assert_eq!(
        tc.operation.floor(),
        Some(Floor::ModelBottom { depth: 0.0 })
    );
    let report = s.selection_status(&tc, SelectionTarget::Op).unwrap();
    let row = report.rows.iter().find(|r| r.outline).expect("the row");
    let floor_z = s
        .toolpath_heights(&tc)
        .unwrap()
        .floor_z(&tc.heights)
        .expect("a floor")
        .expect("the floor resolves");
    let row_z = row.plane_z.expect("the row Z");
    assert!(
        (row_z - floor_z).abs() <= 1e-6,
        "the outline Z {row_z} is the part bottom {floor_z} of the setup"
    );
}

/// Claim 6.
#[test]
fn list_model_items_names_the_outline_ra() {
    let s = stepped_session();
    let rows = s
        .list_model_items(ModelId::from_raw(0), None, None)
        .unwrap();
    let outline: Vec<_> = rows.iter().filter(|r| r.outline).collect();
    assert_eq!(outline.len(), 1);
    assert_eq!(outline[0].kind, ItemKind::Loop);
}

/// The build time of the outline on a large terrain mesh. The mesh is
/// `RS_CAM_RA_MESH` (the R-A timing used the rivmap100 roughing benchmark
/// terrain), else `fixtures/terrain.stl`. Run by hand in release:
/// `cargo test -p rs_cam_core --release --features step,test-support
/// --test a_mesh_model_gives_its_part_outline_ra -- --ignored --nocapture`.
#[test]
#[ignore = "instrument: the outline build time, by hand in release"]
#[expect(clippy::print_stderr, reason = "the timing run reports its time")]
fn the_outline_build_time_on_a_terrain_ra() {
    let path = std::env::var_os("RS_CAM_RA_MESH").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/terrain.stl"),
        PathBuf::from,
    );
    let t = std::time::Instant::now();
    let mesh = TriangleMesh::from_stl_scaled(&path, 1.0).unwrap();
    let load = t.elapsed();
    let t = std::time::Instant::now();
    let item = rs_cam_core::selection::part_outline_item(&mesh).expect("an outline");
    let build = t.elapsed();
    eprintln!(
        "{}: {} triangles, STL load {load:?}, outline {} points, build {build:?}",
        path.display(),
        mesh.triangles.len(),
        ring_points(&item).len(),
    );
}
