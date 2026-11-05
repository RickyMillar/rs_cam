//! UX-7b (SAFETY): a 2.5D op on a STEP model never cuts into the model in
//! silence.
//!
//! # The defects (measured on the live GUI at a51f9620)
//!
//! 1. UX-7 gave the default rule the face rings of a STEP model. A default
//!    Pocket on `stepped_block.step` then cut ALL THREE rings to one floor,
//!    5 mm below the highest ring: 5 mm into the Z 30 face.
//! 2. Q2 (a face pick sets "Each picked face") never fired on an op that an
//!    add door made: the add doors write the stock default floor (`Top Z`
//!    5 on this stock), and Q2 compared with the registry default (3).
//! 3. A default Profile cut round each ring down to Z -2, through the
//!    higher steps beside it.
//! 4. A Drill on the blind hole of `block_with_blind_hole.step`, depth 32,
//!    drilled through the hole floor into the part.
//!
//! # The claims
//!
//! 1. Part 1 (each-face default): the `AddToolpath` row, with the op of
//!    the add doors (`ProjectSession::draft_toolpath`, S11), sets the Pocket
//!    floor "Each picked face", depth 0, and names it in `floor_set`. The
//!    Pocket generates; inside each ring (shrunk by the tool radius) no
//!    cutting sample lies below the ring Z, and the deepest one lies AT the
//!    ring Z. The guard measured it clean.
//! 2. Q2 at the stock default floor: a face pick and a ring pick each set
//!    "Each picked face" (defect 2).
//! 3. Part 2 (the gouge guard): a Pocket on the Z 30 ring with the floor
//!    `Top Z` 5 generates, gives the blocking diagnostic
//!    `safety.cuts_into_model` (5 mm deep), and the export writes the
//!    program and warns on it (K6, ruling Q6: no finding refuses).
//! 4. A Profile on the Z 30 ring through the stock (the floor of the
//!    default Profile before R-A) gives the same diagnostic and export
//!    warning. R-A (2026-10-23): the default Profile on a mesh model now
//!    cuts the part outline only (`a_mesh_model_gives_its_part_outline_ra`).
//! 5. The tangent wall: a Pocket on the Z 20 ring, next to the Z 30 wall,
//!    cuts inside the ring and measures clean; a move 0.2 mm into that
//!    wall gouges. The ring is clockwise in the file: a lone ring was not
//!    wound (`polygon::detect_containment_tagged` returned early), so a
//!    Pocket of one STEP ring cleared the stock round it.
//! 6. The Hole rule: the blind hole drilled to depth 32 gouges, to depth 8
//!    (the hole floor) does not; a plunge at a flat spot of the block does
//!    not (the model shows no hole there: a dowel hole is the intent).
//! 7. A DXF Pocket has no mesh: the guard does not measure (`None`).
//! 8. An STL flat-region face Pocket at "Each picked face" measures clean.
//! 9. A bull nose with a full-radius corner still generates (the reduced
//!    probe shrinks its corner too).
//! 10. A Bore on the blind hole (Ø4 tool) gouges below the hole floor and
//!     is clean at it.
//! 11. F30 (claim 6): the Drill text names the depth to type, from the
//!     stock top that the Depth field measures from, and the hole floor Z:
//!     "Set the depth to 8.000 mm (the hole floor is at Z 12.000)".
//!
//! Red proofs (run by hand, 2026-10-22; each went red, then was restored):
//! - the lone-ring winding back after the early return: claims 3 and 5;
//! - `floor_is_default` on the registry default only: claims 1, 2 and 5;
//! - the each-face rule off in `AddToolpath`: claim 1;
//! - the guard judges no sample: claims 1, 3, 4, 5 and 6;
//! - the probe at the full tool radius: claims 1 and 5;
//! - the Hole rule with no "has a hole" test: claim 6;
//! - a ring pick not counted as an area: claim 2;
//! - the corner radius not shrunk: claim 9;
//! - no depth reaches the verdict: claims 3, 4, 5, 6 and 10;
//! - (2026-10-23) the Drill row with `names_depth: false`: claim 11.
//!
//! The Z tolerance of claim 8 (band plus CAD rounding) is a derivation: the
//! fixture face is exactly flat, so 0.01 mm also passes.
//!
//! ```text
//! cargo test -p rs_cam_core -q --features step --test a_default_step_pocket_never_cuts_below_a_ring_ux7b
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

use common::session::{polygon_model, single_op_session, square_polygon};
use common::tools::endmill_tool_config;
use rs_cam_core::compute::StockConfig;
use rs_cam_core::compute::catalog::{ModelGougeRule, OperationConfig, OperationType};
use rs_cam_core::compute::floor::Floor;
use rs_cam_core::compute::model_gouge::{
    GougeProbe, MODEL_GOUGE_TOL_MM, ModelGougeAllowance, measure,
};
use rs_cam_core::compute::operation_configs::{DrillConfig, PocketConfig};
use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits};
use rs_cam_core::compute::tool_config::ToolConfig;
use rs_cam_core::diagnostics::{Severity, ids};
use rs_cam_core::feeds::suggest::StockContext;
use rs_cam_core::gcode::ExportWarning;
use rs_cam_core::geo::P3;
use rs_cam_core::ids::ModelId;
use rs_cam_core::mesh::SpatialIndex;
use rs_cam_core::selection::{Item, ItemKind, Selection, SelectionEdit};
use rs_cam_core::session::{
    AddToolpathArgs, Command, EditSelectionArgs, Effects, LoadedModel, ProjectSession,
    ProjectSessionBuilder, SelectionTarget, SetToolpathFloorArgs,
};
use rs_cam_core::toolpath::{MoveIntent, MoveType, Toolpath};

const STEPPED: &str = "tests/fixtures/step/stepped_block.step";
const HOLE: &str = "tests/fixtures/step/block_with_blind_hole.step";

/// The rings of the stepped block: (X from, X to, Z), Y 0..40.
const RINGS: [(f64, f64, f64); 3] = [(0.0, 20.0, 30.0), (20.0, 40.0, 20.0), (40.0, 60.0, 10.0)];
/// The stock top of the live finding (mm).
const STOCK_TOP: f64 = 32.0;
const TOOL_D: f64 = 6.0;
const R: f64 = TOOL_D / 2.0;

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

/// The stock of the live finding: X -5..65, Y -5..45, Z 0..32.
fn stepped_stock() -> StockConfig {
    StockConfig {
        x: 70.0,
        y: 50.0,
        z: Some(STOCK_TOP),
        origin_x: -5.0,
        origin_y: -5.0,
        origin_z: 0.0,
        auto_from_model: false,
        ..StockConfig::default()
    }
}

/// A session with the stepped block, a 6 mm end mill and no toolpath.
fn stepped_session() -> ProjectSession {
    let mut builder = ProjectSessionBuilder::new().stock(stepped_stock());
    let _ = builder.add_tool(endmill_tool_config(TOOL_D));
    let _ = builder.add_model(load(STEPPED));
    builder.build()
}

/// Add `op_type` as every add door does: THE draft door
/// (`ProjectSession::draft_toolpath`, S11), through the `AddToolpath` row.
/// W3h: the op holds nothing.
/// The effects of the add. S11: the fixture states the feeds that a tool
/// with no feeds data leaves unset, and the dressup values before S11; the
/// floor stays as the door leaves it.
fn add_drafted(session: &mut ProjectSession, op_type: OperationType) -> Effects {
    let tool_id = session.tools()[0].id;
    let model_id = session.models()[0].id;
    let mut cfg = session
        .draft_toolpath(
            rs_cam_core::session::DraftRequest::new(op_type, 0, op_type.label().to_owned())
                .tool(tool_id)
                .model(model_id),
        )
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

/// [`add_drafted`], then select every item the op can cut (W3h).
fn add_as_the_doors_do(session: &mut ProjectSession, op_type: OperationType) -> Effects {
    let add = add_drafted(session, op_type);
    // W3h: the draft starts with nothing selected. The op then takes the
    // rule "every item it can cut" (the default of the claims), and that
    // edit runs the add-time floor rules; its effects name the floor.
    let index = add.created.expect("the add creates a toolpath");
    if !session.toolpath_configs()[index]
        .operation
        .selection()
        .is_some_and(rs_cam_core::selection::Selection::is_empty)
    {
        return add;
    }
    session
        .apply(Command::EditSelection(EditSelectionArgs {
            index,
            target: SelectionTarget::Op,
            edit: SelectionEdit::Replace(Box::new(rs_cam_core::selection::Selection::drawing())),
        }))
        .expect("the op holds a selection")
}

fn floor_of(session: &ProjectSession, index: usize) -> Floor {
    session.toolpath_configs()[index].operation.floor().unwrap()
}

fn set_floor(session: &mut ProjectSession, index: usize, floor: Floor) {
    let _ = session
        .apply(Command::SetToolpathFloor(SetToolpathFloorArgs {
            index,
            floor,
        }))
        .expect("the floor door");
}

fn pick(session: &mut ProjectSession, index: usize, item: &Item) -> Effects {
    let model_id = session.models()[0].id;
    let key = item.key().clone().for_toolpath_model(model_id);
    session
        .apply(Command::EditSelection(EditSelectionArgs {
            index,
            target: SelectionTarget::Op,
            edit: SelectionEdit::AddPick(key),
        }))
        .expect("the op holds a selection")
}

/// The one item of `kind` with plane Z `z` (a ring or an up face).
fn item_at(session: &ProjectSession, kind: ItemKind, z: f64) -> Item {
    let hits: Vec<&Item> = session.models()[0]
        .items
        .iter()
        .filter(|i| i.kind() == kind && i.plane_z().is_some_and(|p| (p - z).abs() < 1e-6))
        .collect();
    assert_eq!(hits.len(), 1, "one {kind:?} at Z {z}");
    hits[0].clone()
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

/// Points along every cutting move of `tp`, at most 0.5 mm apart (an arc
/// by its chord: the 2.5D ops here cut lines and the claims need a lower
/// bound on the tip Z only).
fn cut_samples(tp: &Toolpath) -> Vec<P3> {
    let mut out = Vec::new();
    for pair in tp.moves.windows(2) {
        let (a, b) = (pair[0].target, pair[1].target);
        if matches!(pair[1].move_type, MoveType::Rapid) {
            continue;
        }
        let n = ((b - a).norm() / 0.5).ceil().max(1.0) as usize;
        for k in 0..=n {
            out.push(a + (b - a) * (k as f64 / n as f64));
        }
    }
    out
}

fn gouge_diagnostics(
    session: &ProjectSession,
    index: usize,
) -> Vec<rs_cam_core::diagnostics::Diagnostic> {
    session
        .diagnose_toolpath(index)
        .unwrap()
        .into_iter()
        .filter(|d| d.id.as_str() == ids::SAFETY_CUTS_INTO_MODEL)
        .collect()
}

/// K6: the export writes the program; these are its cut-into-model
/// warnings.
fn gouge_warnings(session: &ProjectSession) -> Vec<ExportWarning> {
    session
        .export_gcode_checked(None)
        .expect("no finding refuses the export")
        .warnings
        .into_iter()
        .filter(|w| w.id == ids::SAFETY_CUTS_INTO_MODEL)
        .collect()
}

/// Claim 1.
#[test]
fn a_default_step_pocket_cuts_each_ring_to_its_own_z_ux7b() {
    let mut s = stepped_session();
    let effects = add_as_the_doors_do(&mut s, OperationType::Pocket);
    assert_eq!(effects.floor_set, Some(Floor::EachFace { depth: 0.0 }));
    assert_eq!(floor_of(&s, 0), Floor::EachFace { depth: 0.0 });
    let tp = generate(&mut s, 0);
    let samples = cut_samples(&tp);
    for (x0, x1, z) in RINGS {
        let inside: Vec<&P3> = samples
            .iter()
            .filter(|p| {
                p.x > x0 + R - 1e-3
                    && p.x < x1 - R + 1e-3
                    && p.y > R - 1e-3
                    && p.y < 40.0 - R + 1e-3
            })
            .collect();
        assert!(!inside.is_empty(), "the Pocket cuts the ring at Z {z}");
        let lowest = inside.iter().map(|p| p.z).fold(f64::INFINITY, f64::min);
        assert!(
            lowest >= z - MODEL_GOUGE_TOL_MM,
            "inside the ring at Z {z} the Pocket cuts to Z {lowest}"
        );
        assert!(
            (lowest - z).abs() < 1e-3,
            "the ring at Z {z} is cut down to its own Z, not {lowest}"
        );
    }
    let finding = s
        .result_at(0)
        .unwrap()
        .stats
        .model_gouge
        .expect("the guard ran");
    assert!(finding.judged > 0, "non-vacuity: the guard judged samples");
    assert!(!finding.gouges(), "{}", finding.message());
    assert!(gouge_diagnostics(&s, 0).is_empty());
}

/// Claim 2 (defect 2): Q2 fires at the stock default floor, for a face and
/// for a ring.
#[test]
fn a_face_or_ring_pick_at_the_stock_default_floor_sets_each_face_ux7b() {
    // S11: a new Pocket has no floor; the default floor is `Floor::Unset`
    // (before S11 the stock default `Top Z` 5).
    let stock_default = Floor::Unset;
    for kind in [ItemKind::Face, ItemKind::Loop] {
        let mut s = stepped_session();
        let _ = add_drafted(&mut s, OperationType::Pocket);
        set_floor(&mut s, 0, stock_default.clone());
        assert!(
            s.toolpath_configs()[0]
                .operation
                .floor_is_default(&StockContext::from_stock_bbox(
                    s.stock_bbox(),
                    s.stock_config().padding
                )),
            "non-vacuity: the unset floor is a default floor"
        );
        let item = item_at(&s, kind, 10.0);
        let effects = pick(&mut s, 0, &item);
        assert_eq!(
            effects.floor_set,
            Some(Floor::EachFace { depth: 0.0 }),
            "a {kind:?} pick sets 'Each picked face'"
        );
        assert_eq!(floor_of(&s, 0), Floor::EachFace { depth: 0.0 });
    }
}

/// Claim 3.
#[test]
fn a_pocket_below_the_z30_ring_is_warned_ux7b() {
    let mut s = stepped_session();
    let _ = add_drafted(&mut s, OperationType::Pocket);
    let ring = item_at(&s, ItemKind::Loop, 30.0);
    let _ = pick(&mut s, 0, &ring);
    set_floor(&mut s, 0, Floor::top(5.0));
    let _ = generate(&mut s, 0);
    let finding = s.result_at(0).unwrap().stats.model_gouge.unwrap();
    assert!(finding.gouges(), "{finding:?}");
    assert!(
        (finding.deepest_mm - 5.0).abs() < 1e-3,
        "5 mm into the Z 30 face: {}",
        finding.message()
    );
    let diagnostics = gouge_diagnostics(&s, 0);
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].severity, Severity::Severe);
    assert!(
        diagnostics[0].message.contains("Each picked face"),
        "{}",
        diagnostics[0].message
    );
    let warnings = gouge_warnings(&s);
    assert_eq!(warnings.len(), 1, "{warnings:?}");
}

/// Claim 4 (defect 3). R-A: the default Profile takes the part outline
/// only, so the ring is a pick, and the floor the old default wrote (the
/// stock bottom, `Top` less the stock thickness) is stated.
#[test]
fn a_ring_profile_through_the_stock_is_warned_ux7b() {
    let mut s = stepped_session();
    let _ = add_drafted(&mut s, OperationType::Profile);
    let ring = item_at(&s, ItemKind::Loop, 30.0);
    let _ = pick(&mut s, 0, &ring);
    set_floor(&mut s, 0, Floor::top(STOCK_TOP));
    let _ = generate(&mut s, 0);
    let finding = s.result_at(0).unwrap().stats.model_gouge.unwrap();
    assert!(finding.gouges(), "{finding:?}");
    let diagnostics = gouge_diagnostics(&s, 0);
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].severity, Severity::Severe);
    assert!(
        diagnostics[0].message.contains("part outline"),
        "{}",
        diagnostics[0].message
    );
    assert_eq!(gouge_warnings(&s).len(), 1);
}

/// Claim 5: the tangent wall.
#[test]
fn a_tool_tangent_to_a_wall_is_clean_and_a_tool_in_it_is_not_ux7b() {
    // A Pocket of the Z 20 ring, at the tool radius from the Z 30 wall.
    let mut s = stepped_session();
    let _ = add_drafted(&mut s, OperationType::Pocket);
    let ring = item_at(&s, ItemKind::Loop, 20.0);
    let _ = pick(&mut s, 0, &ring);
    assert_eq!(floor_of(&s, 0), Floor::EachFace { depth: 0.0 });
    let tp = generate(&mut s, 0);
    let near_wall = cut_samples(&tp)
        .iter()
        .any(|p| (p.x - (20.0 + R)).abs() < 1e-3 && (p.z - 20.0).abs() < 1e-3);
    assert!(
        near_wall,
        "non-vacuity: the Pocket runs tangent to the x = 20 wall"
    );
    // The lone ring is clockwise in the file; the Pocket still cuts inside
    // it (`polygon::detect_containment_tagged` winds one ring too).
    for m in tp
        .moves
        .iter()
        .filter(|m| m.intent == MoveIntent::ClearingCut)
    {
        let p = m.target;
        assert!(
            p.x > 20.0 + R - 1e-3
                && p.x < 40.0 - R + 1e-3
                && p.y > R - 1e-3
                && p.y < 40.0 - R + 1e-3,
            "a clearing cut outside the Z 20 ring: {p:?}"
        );
    }
    let finding = s.result_at(0).unwrap().stats.model_gouge.unwrap();
    assert!(!finding.gouges(), "{}", finding.message());
    assert!(gouge_diagnostics(&s, 0).is_empty());

    // The same line by hand: tangent, then 0.2 mm into the wall.
    let model = load(STEPPED);
    let mesh = model.mesh.as_deref().unwrap();
    let index = SpatialIndex::build_auto(mesh);
    let reduced = rs_cam_core::compute::cutter::build_cutter(
        &endmill_tool_config(TOOL_D)
            .to_builder()
            .diameter(TOOL_D - 2.0 * MODEL_GOUGE_TOL_MM)
            .build(),
    )
    .unwrap();
    let probe = GougeProbe {
        mesh,
        index: &index,
        reduced_tool: &reduced,
        tool_radius: R,
        bevel_depth_mm: 0.0,
        allowance: ModelGougeAllowance::NONE,
        depth_bases: &[],
    };
    let rule = OperationType::Pocket.capabilities().model_gouge;
    let line = |x: f64| {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(x, 5.0, 40.0));
        tp.feed_to(P3::new(x, 5.0, 20.0), 500.0);
        tp.feed_to(P3::new(x, 35.0, 20.0), 500.0);
        measure(&tp, rule, &probe, &AtomicBool::new(false))
            .unwrap()
            .unwrap()
    };
    let tangent = line(20.0 + R);
    assert!(tangent.judged > 0 && !tangent.gouges(), "{tangent:?}");
    let into = line(20.0 + R - 0.2);
    assert!(into.gouges(), "0.2 mm into the wall gouges: {into:?}");
    assert!((into.deepest_mm - 10.0).abs() < 1e-3, "{into:?}");
}

/// The stock of the blind-hole block: X -5..45, Y -5..35, Z 0..20.
fn hole_session(depth: f64) -> ProjectSession {
    let model = load(HOLE);
    let rim = model
        .items
        .iter()
        .find(|i| i.kind() == ItemKind::Circle && i.z().is_some_and(|z| (z - 20.0).abs() < 1e-6))
        .unwrap()
        .clone();
    let op = OperationConfig::Drill(DrillConfig {
        floor: rs_cam_core::compute::floor::Floor::Top { depth },
        selection: Selection::empty(),
        ..DrillConfig::default()
    });
    let stock = StockConfig {
        x: 50.0,
        y: 40.0,
        z: Some(20.0),
        origin_x: -5.0,
        origin_y: -5.0,
        origin_z: 0.0,
        auto_from_model: false,
        ..StockConfig::default()
    };
    let mut s = single_op_session(stock, endmill_tool_config(6.0), model, "ux7b drill", op);
    let _ = pick(&mut s, 0, &rim);
    s
}

/// Claim 6: the Hole rule.
#[test]
fn a_drill_gouges_only_below_the_floor_of_a_model_hole_ux7b() {
    let mut deep = hole_session(32.0);
    let _ = generate(&mut deep, 0);
    let finding = deep.result_at(0).unwrap().stats.model_gouge.unwrap();
    assert!(finding.gouges(), "{finding:?}");
    assert!(
        (finding.surface_z - 12.0).abs() < 1e-3,
        "the hole floor is Z 12: {}",
        finding.message()
    );
    // F30 (F19): the fix names the depth to type (from the rim of the
    // hole, Z 20, the base the floor depth measures from) and the hole
    // floor Z.
    assert!(
        finding
            .message()
            .contains("Set the depth to 8.000 mm (the hole floor is at Z 12.000)"),
        "{}",
        finding.message()
    );
    assert_eq!(gouge_diagnostics(&deep, 0).len(), 1);
    assert!(
        gouge_diagnostics(&deep, 0)[0]
            .message
            .contains("Set the depth to 8.000 mm"),
        "the diagnostic carries the text"
    );
    assert_eq!(gouge_warnings(&deep).len(), 1);

    let mut to_floor = hole_session(8.0);
    let _ = generate(&mut to_floor, 0);
    let finding = to_floor.result_at(0).unwrap().stats.model_gouge.unwrap();
    assert!(finding.judged > 0, "non-vacuity: the hole is judged");
    assert!(!finding.gouges(), "{}", finding.message());
    assert!(gouge_diagnostics(&to_floor, 0).is_empty());

    // A plunge at a flat spot of the block top: the model shows no hole.
    let model = load(HOLE);
    let mesh = model.mesh.as_deref().unwrap();
    let index = SpatialIndex::build_auto(mesh);
    let tool = rs_cam_core::compute::cutter::build_cutter(&endmill_tool_config(6.0)).unwrap();
    let probe = GougeProbe {
        mesh,
        index: &index,
        reduced_tool: &tool,
        tool_radius: 3.0,
        bevel_depth_mm: 0.0,
        allowance: ModelGougeAllowance::NONE,
        depth_bases: &[],
    };
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(30.0, 15.0, 25.0));
    tp.feed_to(P3::new(30.0, 15.0, -12.0), 300.0);
    let rule = OperationType::Drill.capabilities().model_gouge;
    assert!(matches!(rule, ModelGougeRule::Hole { .. }));
    let flat = measure(&tp, rule, &probe, &AtomicBool::new(false))
        .unwrap()
        .unwrap();
    assert!(
        !flat.gouges(),
        "a dowel hole at a flat spot is the intent: {flat:?}"
    );
}

/// Claim 7.
#[test]
fn a_drawing_pocket_is_not_measured_ux7b() {
    let op = OperationConfig::Pocket(PocketConfig {
        floor: Floor::top(3.0),
        ..PocketConfig::default()
    });
    let stock = common::session::stock_over(25.0, 10.0);
    let mut s = single_op_session(
        stock,
        endmill_tool_config(6.0),
        polygon_model(vec![square_polygon(20.0)], "ux7b dxf"),
        "ux7b dxf pocket",
        op,
    );
    let _ = generate(&mut s, 0);
    assert_eq!(s.result_at(0).unwrap().stats.model_gouge, None);
    assert!(gouge_diagnostics(&s, 0).is_empty());
}

/// Claim 8: an STL flat-region face at "Each picked face" measures clean.
/// The face plane is the Z of its lowest triangle, and the region holds
/// triangles up to `FLAT_REGION_Z_BAND_MM` above it
/// (`MODEL_GOUGE_Z_TOL_MM`).
#[test]
fn an_stl_face_pocket_at_its_own_floor_is_clean_ux7b() {
    use rs_cam_core::selection::{MatchKey, SourceRef, flat_region_items};
    let stock = StockConfig {
        x: 20.0,
        y: 10.0,
        z: Some(8.0),
        origin_x: 0.0,
        origin_y: 0.0,
        origin_z: 0.0,
        auto_from_model: false,
        ..StockConfig::default()
    };
    let mut s = single_op_session(
        stock,
        endmill_tool_config(2.0),
        common::session::mesh_model(common::meshes::stepped_top(), "ux7b stl"),
        "ux7b stl pocket",
        OperationConfig::new_default(OperationType::Pocket),
    );
    let set = flat_region_items(&common::meshes::stepped_top()).unwrap();
    let z5 = set.iter().next().expect("the Z 5 face is first");
    let key = MatchKey {
        source: SourceRef::ToolpathModel,
        ..z5.key().clone()
    };
    let _ = s
        .apply(Command::EditSelection(EditSelectionArgs {
            index: 0,
            target: SelectionTarget::Op,
            edit: SelectionEdit::AddPick(key),
        }))
        .unwrap();
    // The flat regions of an STL are built by generation (`FaceAccess`,
    // P4.7); a cold cache gives the pick no face Z yet, so the floor is set
    // by its door here.
    set_floor(&mut s, 0, Floor::EachFace { depth: 0.0 });
    let _ = generate(&mut s, 0);
    let finding = s.result_at(0).unwrap().stats.model_gouge.unwrap();
    assert!(finding.judged > 0, "non-vacuity");
    assert!(!finding.gouges(), "{}", finding.message());
}

/// Claim 9: a bull nose whose corner radius is the full radius still
/// generates (the reduced probe shrinks its corner with it).
#[test]
fn a_full_radius_bull_nose_still_generates_ux7b() {
    let mut tool = ToolConfig::test_default(
        rs_cam_core::compute::tool_config::ToolId::from_raw(0),
        rs_cam_core::compute::tool_config::ToolType::BullNose,
    );
    tool.diameter = TOOL_D;
    tool.corner_radius = R;
    let mut builder = ProjectSessionBuilder::new().stock(stepped_stock());
    let _ = builder.add_tool(tool);
    let _ = builder.add_model(load(STEPPED));
    let mut s = builder.build();
    let _ = add_as_the_doors_do(&mut s, OperationType::Pocket);
    let _ = generate(&mut s, 0);
    let finding = s.result_at(0).unwrap().stats.model_gouge.unwrap();
    assert!(finding.judged > 0 && !finding.gouges(), "{finding:?}");
}

/// Claim 10: a Bore on the blind hole (Ø6, floor Z 12) with a Ø4 tool:
/// below the floor gouges, at the floor does not.
#[test]
fn a_bore_gouges_only_below_the_hole_floor_ux7b() {
    use rs_cam_core::compute::operation_configs::BoreConfig;
    let bore = |depth: f64| {
        let model = load(HOLE);
        let rim = model
            .items
            .iter()
            .find(|i| {
                i.kind() == ItemKind::Circle && i.z().is_some_and(|z| (z - 20.0).abs() < 1e-6)
            })
            .unwrap()
            .clone();
        let op = OperationConfig::Bore(BoreConfig {
            floor: Floor::StockTop { depth },
            selection: Selection::empty(),
            ..BoreConfig::default()
        });
        let stock = StockConfig {
            x: 50.0,
            y: 40.0,
            z: Some(20.0),
            origin_x: -5.0,
            origin_y: -5.0,
            origin_z: 0.0,
            auto_from_model: false,
            ..StockConfig::default()
        };
        let mut s = single_op_session(stock, endmill_tool_config(4.0), model, "ux7b bore", op);
        let _ = pick(&mut s, 0, &rim);
        let _ = generate(&mut s, 0);
        s.result_at(0).unwrap().stats.model_gouge.unwrap()
    };
    let deep = bore(12.0);
    assert!(deep.gouges(), "{deep:?}");
    assert!((deep.deepest_mm - 4.0).abs() < 1e-3, "{}", deep.message());
    let to_floor = bore(8.0);
    assert!(to_floor.judged > 0, "non-vacuity: the hole is judged");
    assert!(!to_floor.gouges(), "{}", to_floor.message());
}
