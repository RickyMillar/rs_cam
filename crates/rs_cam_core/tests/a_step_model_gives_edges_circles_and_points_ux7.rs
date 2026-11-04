//! Geometry selection UX-7: a STEP model gives face rings, edge chains,
//! circular edges and vertices as items, each at its own Z.
//!
//! # The claims
//!
//! Operator ruling Q3 (2026-10-22): "edges circles points. but default to
//! face rings (loops) unless there is a selection for edges". GT-Q9: a round
//! hole edge on a STEP model is a circle item. GT-0: an item key holds Z.
//!
//! 1. The STEP door (`io::load_model_file`) gives, beside the faces, the
//!    rings of each up face (`Loop`), the horizontal edge chains (`Path`),
//!    the circular edges (`Circle`) and the corners (`Point`), each with a
//!    Z. The counts on two fixtures are pinned. A vertical edge gives no
//!    item.
//! 2. Two items that coincide in XY and differ in Z have two keys, and each
//!    key names exactly one item: the hole rim and the hole floor ring, the
//!    two rim circles of the hole, the top and bottom edge of a step wall,
//!    the two corners of a vertical edge.
//! 3. The keys survive a re-load of the same file, in the same order.
//! 4. A Drill that picks the rim circle of the blind hole drills at its
//!    centre to its floor (the depth is the Z of the rim less the Z of the
//!    floor circle).
//! 5. A Chamfer that picks the rim circle at the stock top cuts round it;
//!    a Chamfer that picks the floor circle 8 mm below the fresh stock top
//!    refuses (UX-1c reads the item Z as the face top). A Trace that picks
//!    one top edge (a `Path`) cuts along that edge only.
//! 6. The default rule of a Pocket on a STEP model holds the face rings
//!    only: no path, circle, point or face. R-A (2026-10-23): the default
//!    Profile (side Outside) holds the part outline only, and the Pocket
//!    does not hold it. R-B (2026-10-23,
//!    replaces the F29 pin of this claim): a Drill on the default rule
//!    takes the rim circle of the hole and nothing else, and generates.
//!
//! The fixture `tests/fixtures/step/block_with_blind_hole.step` comes from
//! `gen_block_with_blind_hole.py` beside it (block 40 x 30 x 20, hole
//! diameter 6 at (15, 15), floor at Z 12).
//!
//! Red proofs (run by hand, 2026-10-22): claim 2 goes red when
//! `MatchKey::names_same_geometry` ignores `z`; claim 6 goes red when
//! `rule::served_by_default` admits every drawing kind of an edge item.
//! R-B: claim 6 goes red when `rule::served_by_default` gives the Drill
//! the Loop rule of Q3 again (the rim is then not `Cut`).
//!
//! ```text
//! cargo test -p rs_cam_core -q --features step --test a_step_model_gives_edges_circles_and_points_ux7
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

use rs_cam_core::compute::tool_config::ToolFact;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use common::session::{single_op_session, toolpath_config};
use common::tools::endmill_tool_config;
use rs_cam_core::compute::StockConfig;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::{
    ChamferConfig, ChamferSide, DrillConfig, PocketConfig, ProfileConfig, TraceConfig,
};
use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolType};
use rs_cam_core::ids::{ModelId, ToolId};
use rs_cam_core::selection::{Item, ItemKind, MatchKey, Selection, SelectionEdit};
use rs_cam_core::session::{
    Command, EditSelectionArgs, ItemState, LoadedModel, ProjectSession, ProjectSessionBuilder,
    SelectionTarget, SessionError,
};
use rs_cam_core::toolpath::{MoveIntent, MoveType, Toolpath};

const STEPPED: &str = "../../fixtures/gui_step/stepped_block.step";
const HOLE: &str = "tests/fixtures/step/block_with_blind_hole.step";

/// The hole of the fixture (its generator script states it).
const HOLE_XY: [f64; 2] = [15.0, 15.0];
const HOLE_D: f64 = 6.0;
const RIM_Z: f64 = 20.0;
const FLOOR_Z: f64 = 12.0;

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

/// The count of the model edge items (the items with a Z) by kind.
/// The model edge items by kind. R-A: the part outline is not a model
/// edge (`a_mesh_model_gives_its_part_outline_ra` counts it).
fn edge_counts(model: &LoadedModel) -> BTreeMap<ItemKind, usize> {
    let mut out = BTreeMap::new();
    for item in model
        .items
        .iter()
        .filter(|i| i.z().is_some() && !i.is_part_outline())
    {
        *out.entry(item.kind()).or_insert(0) += 1;
    }
    out
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-3
}

/// The one edge item of `kind` at `anchor` and `z`.
fn find(model: &LoadedModel, kind: ItemKind, anchor: [f64; 2], z: f64) -> &Item {
    find_sized(model, kind, anchor, z, None)
}

/// [`find`], with the key measure when two items share the anchor (two
/// edges from one corner).
fn find_sized(
    model: &LoadedModel,
    kind: ItemKind,
    anchor: [f64; 2],
    z: f64,
    measure: Option<f64>,
) -> &Item {
    let hits: Vec<&Item> = model
        .items
        .iter()
        .filter(|i| {
            let k = i.key();
            k.kind == kind
                && near(k.anchor[0], anchor[0])
                && near(k.anchor[1], anchor[1])
                && k.z.is_some_and(|kz| near(kz, z))
                && measure.is_none_or(|m| near(k.measure, m))
        })
        .collect();
    assert_eq!(hits.len(), 1, "one {kind:?} at {anchor:?} Z {z}");
    hits[0]
}

/// The count of items of `model` whose key names the same geometry as
/// `key`.
fn matches(model: &LoadedModel, key: &MatchKey) -> usize {
    model
        .items
        .iter()
        .filter(|i| i.key().names_same_geometry(key))
        .count()
}

/// Claim 1.
#[test]
fn a_step_model_gives_edges_circles_and_points_ux7() {
    let stepped = load(STEPPED);
    assert!(
        !stepped.items.has_drawing(),
        "a STEP model is not a drawing"
    );
    assert_eq!(
        edge_counts(&stepped),
        BTreeMap::from([
            (ItemKind::Loop, 3),
            (ItemKind::Path, 16),
            (ItemKind::Point, 16),
        ]),
        "stepped block: 3 up rings, 16 horizontal edges, 16 corners"
    );
    let hole = load(HOLE);
    assert_eq!(
        edge_counts(&hole),
        BTreeMap::from([
            (ItemKind::Loop, 3),
            (ItemKind::Path, 8),
            (ItemKind::Point, 8),
            (ItemKind::Circle, 2),
        ]),
        "blind hole: top outline, rim ring, floor ring; 8 block edges; 8 corners; two rims"
    );
    for z in [RIM_Z, FLOOR_Z] {
        let d = find(&hole, ItemKind::Circle, HOLE_XY, z).key().measure;
        assert!((d - HOLE_D).abs() < 1e-3, "the rim at Z {z} is Ø6: {d}");
    }
    // No item stands for a vertical edge: every item Z is a level of the
    // part.
    for item in stepped.items.iter().filter_map(Item::z) {
        assert!(
            [0.0, 10.0, 20.0, 30.0].iter().any(|z| near(*z, item)),
            "Z {item}"
        );
    }
}

/// One coincident pair: the model, the kind, the anchor, the measure when
/// two items share the anchor, and the two Z.
type Pair<'a> = (&'a LoadedModel, ItemKind, [f64; 2], Option<f64>, f64, f64);

/// Claim 2 (GT-0).
#[test]
fn coincident_items_have_distinct_keys_ux7() {
    let hole = load(HOLE);
    let stepped = load(STEPPED);
    let pairs: [Pair<'_>; 4] = [
        (&hole, ItemKind::Loop, HOLE_XY, None, RIM_Z, FLOOR_Z),
        (&hole, ItemKind::Circle, HOLE_XY, None, RIM_Z, FLOOR_Z),
        // The wall at X 20: its top edge (Z 30) and its bottom edge (Z 20).
        (
            &stepped,
            ItemKind::Path,
            [20.0, 0.0],
            Some(40.0),
            30.0,
            20.0,
        ),
        // The two corners of the vertical edge at (20, 0).
        (&stepped, ItemKind::Point, [20.0, 0.0], None, 30.0, 20.0),
    ];
    for (model, kind, anchor, measure, z_a, z_b) in pairs {
        let a = find_sized(model, kind, anchor, z_a, measure).key();
        let b = find_sized(model, kind, anchor, z_b, measure).key();
        assert!(
            near(a.anchor[0], b.anchor[0]) && near(a.anchor[1], b.anchor[1]),
            "{kind:?}: the pair coincides in XY"
        );
        assert!(near(a.measure, b.measure), "{kind:?}: and in size");
        assert!(!a.matches(b), "{kind:?} at Z {z_a} and Z {z_b}: two keys");
        assert_eq!(matches(model, a), 1, "{kind:?} at Z {z_a}: one item");
        assert_eq!(matches(model, b), 1, "{kind:?} at Z {z_b}: one item");
    }
    // Every key of each model names exactly one item: no pick is
    // ambiguous.
    for model in [&hole, &stepped] {
        for item in model.items.iter() {
            assert_eq!(matches(model, item.key()), 1, "{:?}", item.key());
        }
    }
}

/// Claim 3.
#[test]
fn keys_survive_a_reload_ux7() {
    for rel in [STEPPED, HOLE] {
        let a = load(rel);
        let b = load(rel);
        assert_eq!(a.items.len(), b.items.len(), "{rel}");
        for (x, y) in a.items.iter().zip(b.items.iter()) {
            assert!(
                x.key().matches(y.key()),
                "{rel}: {:?} vs {:?}",
                x.key(),
                y.key()
            );
        }
        // A stored key round-trips through the file form.
        for item in a.items.iter() {
            let text = serde_json::to_string(item.key()).unwrap();
            let back: MatchKey = serde_json::from_str(&text).unwrap();
            assert!(back.matches(item.key()), "{text}");
        }
    }
}

/// The stock of the blind-hole block: X -5..45, Y -5..35, Z 0..`top`.
fn hole_stock(top: f64) -> StockConfig {
    StockConfig {
        x: 50.0,
        y: 40.0,
        z: Some(top),
        origin_x: -5.0,
        origin_y: -5.0,
        origin_z: 0.0,
        auto_from_model: false,
        ..StockConfig::default()
    }
}

/// A 90° V-bit for the Chamfer.
fn vbit() -> ToolConfig {
    let mut t = ToolConfig::new_default(ToolId::from_raw(0), ToolType::VBit);
    t.diameter = 12.0;
    t.set_cutting_length(ToolFact::Stated(10.0));
    t.included_angle = 90.0;
    t.set_shank_diameter(ToolFact::Stated(6.0));
    t.set_stickout(ToolFact::Stated(30.0));
    t.set_flute_count(ToolFact::Stated(2));
    t
}

/// Pick `item` into the op selection of toolpath 0.
fn pick(session: &mut ProjectSession, item: &Item) {
    let model_id = session.models()[0].id;
    let key = item.key().clone().for_toolpath_model(model_id);
    let _ = session
        .apply(Command::EditSelection(EditSelectionArgs {
            index: 0,
            target: SelectionTarget::Op,
            edit: SelectionEdit::AddPick(key),
        }))
        .expect("the op holds a selection");
}

fn generate(session: &mut ProjectSession) -> Result<Toolpath, SessionError> {
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(0, &cancel)
        .map(|r| r.op_data.annotated().toolpath.clone())
}

/// The cutting moves (feed moves) of `tp`.
fn cuts(tp: &Toolpath) -> Vec<[f64; 3]> {
    tp.moves
        .iter()
        .filter(|m| !matches!(m.move_type, MoveType::Rapid))
        .map(|m| [m.target.x, m.target.y, m.target.z])
        .collect()
}

/// Claim 4 (GT-Q9).
#[test]
fn a_drill_on_a_step_circle_drills_its_centre_to_its_floor_ux7() {
    let model = load(HOLE);
    let rim = find(&model, ItemKind::Circle, HOLE_XY, RIM_Z).clone();
    let floor = find(&model, ItemKind::Circle, HOLE_XY, FLOOR_Z).clone();
    let depth = rim.z().unwrap() - floor.z().unwrap();
    let op = OperationConfig::Drill(DrillConfig {
        floor: rs_cam_core::compute::floor::Floor::Top { depth },
        selection: Selection::empty(),
        ..DrillConfig::default()
    });
    let mut session = single_op_session(
        hole_stock(RIM_Z),
        endmill_tool_config(HOLE_D),
        model,
        "ux7 drill",
        op,
    );
    pick(&mut session, &rim);
    let tp = generate(&mut session).expect("the Drill generates");
    let drilling: Vec<[f64; 3]> = tp
        .moves
        .iter()
        .filter(|m| m.intent == MoveIntent::Drilling)
        .map(|m| [m.target.x, m.target.y, m.target.z])
        .collect();
    assert!(!drilling.is_empty(), "the Drill drills");
    for p in &drilling {
        assert!(
            near(p[0], HOLE_XY[0]) && near(p[1], HOLE_XY[1]),
            "at the centre: {p:?}"
        );
    }
    let lowest = drilling.iter().map(|p| p[2]).fold(f64::INFINITY, f64::min);
    assert!(near(lowest, FLOOR_Z), "to the floor Z {FLOOR_Z}: {lowest}");
}

/// Claim 5: a Chamfer on the rim at the stock top; a refusal on the floor
/// circle below the fresh stock top; a Trace on one top edge.
#[test]
fn a_chamfer_and_a_trace_cut_step_edges_ux7() {
    let chamfer = OperationConfig::Chamfer(ChamferConfig {
        side: ChamferSide::Inside,
        chamfer_width: 1.0,
        selection: Selection::empty(),
        ..ChamferConfig::default()
    });
    // The rim at the stock top.
    let model = load(HOLE);
    let rim = find(&model, ItemKind::Circle, HOLE_XY, RIM_Z).clone();
    let mut session = single_op_session(
        hole_stock(RIM_Z),
        vbit(),
        model,
        "ux7 chamfer",
        chamfer.clone(),
    );
    pick(&mut session, &rim);
    let tp = generate(&mut session).expect("the Chamfer on the rim generates");
    let cut = cuts(&tp);
    assert!(!cut.is_empty());
    for p in &cut {
        let r = (p[0] - HOLE_XY[0]).hypot(p[1] - HOLE_XY[1]);
        assert!(
            r <= HOLE_D,
            "round the rim: {p:?} at {r:.3} from the centre"
        );
        assert!(p[2] >= FLOOR_Z, "above the floor: {p:?}");
    }
    // The floor circle, 8 mm below the fresh stock top. K6 (ruling Q6,
    // decision E1): the buried face is a warning, named before generation
    // (`precondition.buried_face`, Critical). Generation then refuses the
    // pick itself: a wall rises above the floor circle, so the edge is
    // concave (a selection refusal, a missing input; before K6 the buried
    // face refused first).
    let model = load(HOLE);
    let floor = find(&model, ItemKind::Circle, HOLE_XY, FLOOR_Z).clone();
    let mut session = single_op_session(hole_stock(RIM_Z), vbit(), model, "ux7 buried", chamfer);
    pick(&mut session, &floor);
    let buried: Vec<_> = session
        .diagnose_toolpath(0)
        .expect("the toolpath diagnoses")
        .into_iter()
        .filter(|d| d.id.as_str() == rs_cam_core::diagnostics::ids::PRECOND_BURIED_FACE)
        .collect();
    assert_eq!(buried.len(), 1, "the buried floor circle is named");
    assert_eq!(
        buried[0].severity,
        rs_cam_core::diagnostics::Severity::Critical
    );
    assert!(
        buried[0]
            .message
            .contains(&format!("{:.1} mm below the stock top", RIM_Z - FLOOR_Z)),
        "{}",
        buried[0].message
    );
    match generate(&mut session) {
        Err(SessionError::Selection(refusal)) => {
            assert!(refusal.to_string().contains("concave"), "{refusal}");
        }
        Err(other) => panic!("want the concave-edge selection refusal, got: {other}"),
        Ok(_) => panic!("a Chamfer of the concave floor circle must refuse the pick"),
    }
    // A Trace on the top edge Y 0, X 0..40.
    let model = load(HOLE);
    let edge = find_sized(&model, ItemKind::Path, [0.0, 0.0], RIM_Z, Some(40.0)).clone();
    assert!(
        near(edge.key().measure, 40.0),
        "the X edge: {:?}",
        edge.key()
    );
    let trace = OperationConfig::Trace(TraceConfig {
        depth: Some(1.0),
        selection: Selection::empty(),
        ..TraceConfig::default()
    });
    let mut session = single_op_session(
        hole_stock(RIM_Z),
        endmill_tool_config(3.0),
        model,
        "ux7 trace",
        trace,
    );
    pick(&mut session, &edge);
    let tp = generate(&mut session).expect("the Trace on the edge generates");
    let cut = cuts(&tp);
    assert!(!cut.is_empty());
    for p in &cut {
        assert!(near(p[1], 0.0), "along Y 0: {p:?}");
        assert!(
            p[0] >= -1e-6 && p[0] <= 40.0 + 1e-6,
            "within X 0..40: {p:?}"
        );
    }
}

/// The item states of the op selection of toolpath 0, by item.
fn states(session: &ProjectSession) -> Vec<(Item, ItemState)> {
    let tc = session.toolpath_configs()[0].clone();
    let report = session.selection_status(&tc, SelectionTarget::Op).unwrap();
    session.models()[0]
        .items
        .iter()
        .cloned()
        .zip(report.item_states.iter().copied())
        .collect()
}

/// A session with `op` on its default selection, on `model`.
fn default_session(model: LoadedModel, op: OperationConfig) -> ProjectSession {
    let mut builder = ProjectSessionBuilder::new().stock(StockConfig {
        x: 70.0,
        y: 50.0,
        z: Some(30.0),
        origin_x: -5.0,
        origin_y: -5.0,
        origin_z: 0.0,
        auto_from_model: false,
        ..StockConfig::default()
    });
    let tool = builder.add_tool(endmill_tool_config(6.0));
    let tool_id = builder.tools()[tool].id;
    let model_id = builder.add_model(model);
    let _ = builder
        .add_toolpath(0, toolpath_config("ux7 default", op, tool_id, model_id))
        .unwrap();
    builder.build()
}

/// Claim 6 (ruling Q3).
#[test]
fn the_default_rule_on_a_step_model_holds_the_face_rings_only_ux7() {
    for (name, op) in [
        ("Pocket", OperationConfig::Pocket(PocketConfig::default())),
        (
            "Profile",
            OperationConfig::Profile(ProfileConfig::default()),
        ),
    ] {
        for rel in [STEPPED, HOLE] {
            let session = default_session(load(rel), op.clone());
            let mut held = 0;
            for (item, state) in states(&session) {
                // R-A: the Profile takes the outline only, the Pocket the
                // face rings only.
                let ring = if name == "Profile" {
                    item.is_part_outline()
                } else {
                    item.kind() == ItemKind::Loop && item.z().is_some() && !item.is_part_outline()
                };
                let selected = state != ItemState::NotSelected;
                assert_eq!(
                    selected,
                    ring,
                    "{name} on {rel}: {:?} at Z {:?} is {state:?}",
                    item.kind(),
                    item.z()
                );
                held += usize::from(selected);
            }
            let want = if name == "Profile" { 1 } else { 3 };
            assert!(
                held >= want,
                "{name} on {rel}: the default holds the rings ({held})"
            );
        }
    }
    // R-B (operator ruling 2026-10-23, replaces the F29 pin here): a Drill
    // on the default rule takes the RIM circle of the hole (Z 20) and no
    // ring; the floor circle (Z 12) does not serve. The Drill generates.
    let mut session = default_session(load(HOLE), OperationConfig::Drill(DrillConfig::default()));
    for (item, state) in states(&session) {
        let rim = item.kind() == ItemKind::Circle && item.z().is_some_and(|z| near(z, RIM_Z));
        let want = if rim {
            ItemState::Cut
        } else {
            ItemState::NotSelected
        };
        assert_eq!(
            state,
            want,
            "Drill default: {:?} at Z {:?}",
            item.kind(),
            item.z()
        );
    }
    assert!(
        generate(&mut session).is_ok(),
        "a Drill on the default rule of a STEP model drills the rim"
    );
}
