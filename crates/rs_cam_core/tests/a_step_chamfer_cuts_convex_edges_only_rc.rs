//! R-C + F21 (operator ruling 2026-10-23, SAFETY): a Chamfer on a STEP
//! model bevels the convex top edges only, each at its own Z.
//!
//! # The defects (FR-1, FR-2 flow reviews)
//!
//! - F5: the default rule of a STEP model takes the face rings (ruling Q3).
//!   A ring holds concave edges too: the Z 20 ring of `stepped_block.step`
//!   meets the Z 30 step wall at X 20. A Chamfer along that edge cuts into
//!   the wall above it, and no diagnostic fired (Chamfer was exempt).
//! - F21: the Chamfer cut every item at the Z of the highest item, so the
//!   lower items got air passes.
//!
//! # The claims
//!
//! 1. The STEP door states the form of each edge (`selection::EdgeForm`):
//!    on the stepped block the 10 top chains with a wall below are convex,
//!    the chain at Z 20 X 20 and the chain at Z 10 X 40 (each under a step
//!    wall) are concave, the Z 20 and Z 10 rings are concave, the Z 30 ring
//!    is convex, and the bottom chains are neither. On the blind hole block
//!    the rim circle is convex (material outside) and the floor circle is
//!    concave.
//! 2. The default Chamfer (the add doors, `default_operation`) takes the 10
//!    convex chains and nothing else (`selection_status`).
//! 3. On the EMITTED motion of that default Chamfer: no cutting move runs
//!    along a concave edge, and each convex edge is cut at its own Z, on the
//!    air side of the edge by the tip offset, at `edge Z - (w + t) /
//!    tan(a)` (F21). The cut Z levels are exactly one per edge Z.
//! 4. The guard (`ModelGougeRule::Bevel`): four chains end at a step wall.
//!    Chamfer Clearance (operator ruling 2026-10-23) stops each of them
//!    short of the wall (`compute::edge_clear`), so the default Chamfer
//!    measures clean, and the guard judged samples. Before the ruling the
//!    cone cut into the walls at X 20 and X 40 and the finding gouged; the
//!    length of each stop is the claim of
//!    `a_step_chamfer_stops_short_of_a_wall_chamferclear`. A Chamfer of the
//!    four Z 30 chains (convex corners only) measures clean: the bevel
//!    itself is no gouge.
//! 5. A pick of a concave chain or of a concave ring refuses
//!    `NotAConvexEdge { Concave }` and names the item; a pick of the
//!    convex Z 30 ring cuts at Z 30 only.
//! 6. On fresh stock with an auto top, the default Chamfer refuses
//!    `BuriedFace` by its LOWEST item (the Z 10 chains are 20 mm under the
//!    stock top): each item cuts below its own Z
//!    (`FaceTopRule::RefuseBuriedEach`).
//! 7. A `Bottom` setup turns the model: the edge form does not hold there,
//!    so the default Chamfer refuses `NotAConvexEdge { TurnedSetup }`.
//!
//! The motion claims run with a pinned Top Z at the model top (Manual
//! Z 30): the operator's statement that the material is gone, which skips
//! the buried checks (claim 6 tests them). A model edge cuts from its own
//! Z under a pinned top too.
//!
//! Red proofs (run by hand, 2026-10-23; each went red, then was restored):
//! - the STEP door calls a rising face convex (`FaceFacts::side_of` in
//!   `io::brep_items`): claims 1, 2, 3 and 5 (claim 3: a cut runs along
//!   the Z 20 concave edge at X 19.9, Z 18.9);
//! - `item_serves` accepts a concave edge and `served_by_default` takes
//!   the concave chains too: claims 2 and 5;
//! - `ChamferPass::outline_ladders` returns `None` (every outline at the
//!   one op top, F21): claim 3.
//!
//! ```text
//! cargo test -p rs_cam_core -q --features step --test a_step_chamfer_cuts_convex_edges_only_rc
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
use rs_cam_core::compute::config::HeightMode;
use rs_cam_core::compute::execute::{BuriedUnder, OperationError};
use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolType};
use rs_cam_core::compute::transform::FaceUp;
use rs_cam_core::feeds::suggest::{StockContext, default_operation};
use rs_cam_core::geo::{P2, P3, V2};
use rs_cam_core::ids::{ModelId, ToolId};
use rs_cam_core::selection::{
    EdgeForm, EdgeMaterial, EdgeRefusal, Item, ItemKind, RefusalCause, SelectionEdit, Shape,
};
use rs_cam_core::session::{
    AddToolpathArgs, Command, EditSelectionArgs, ItemState, LoadedModel, ProjectSession,
    ProjectSessionBuilder, SelectionTarget, SessionError, SetSetupFaceArgs,
};
use rs_cam_core::toolpath::{MoveType, Toolpath};

const STEPPED: &str = "tests/fixtures/step/stepped_block.step";
const HOLE: &str = "tests/fixtures/step/block_with_blind_hole.step";

/// The model top of the stepped block (mm).
const TOP: f64 = 30.0;
/// The two concave edges of the stepped block: (X, Z), Y 0..40. The Z 30
/// step wall stands over the Z 20 face at X 20, the Z 20 wall over the
/// Z 10 face at X 40.
const CONCAVE_EDGES: [(f64, f64); 2] = [(20.0, 20.0), (40.0, 10.0)];
/// The V-bit included angle (degrees): 90, so `tan(a) = 1`.
const VBIT_ANGLE: f64 = 90.0;

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

/// A 90° V-bit, 12 mm.
fn vbit() -> ToolConfig {
    let mut t = ToolConfig::new_default(ToolId::from_raw(0), ToolType::VBit);
    t.diameter = 12.0;
    t.cutting_length = 10.0;
    t.included_angle = VBIT_ANGLE;
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

/// A session with the stepped block, the V-bit and one Chamfer as the add
/// doors make it (`default_operation`), with the Top Z `top`.
fn chamfer_session(top: HeightMode) -> ProjectSession {
    let mut builder = ProjectSessionBuilder::new().stock(part_stock());
    let _ = builder.add_tool(vbit());
    let _ = builder.add_model(load(STEPPED));
    let mut s = builder.build();
    let stock = StockContext::from_stock_bbox(s.stock_bbox(), s.stock_config().padding);
    let op = default_operation(OperationType::Chamfer, &stock);
    let mut cfg =
        common::session::toolpath_config("rc chamfer", op, s.tools()[0].id, s.models()[0].id);
    cfg.heights.top_z = top;
    let _ = s
        .apply(Command::AddToolpath(AddToolpathArgs {
            setup_index: 0,
            config: Box::new(cfg),
        }))
        .expect("the add");
    s
}

/// The bevel depth below the edge: `(w + t) / tan(a)`, from the op.
fn tip_depth(s: &ProjectSession) -> (f64, f64) {
    let OperationConfig::Chamfer(cfg) = &s.toolpath_configs()[0].operation else {
        panic!("a Chamfer");
    };
    let tan = (VBIT_ANGLE / 2.0).to_radians().tan();
    ((cfg.chamfer_width + cfg.tip_offset) / tan, cfg.tip_offset)
}

fn generate(s: &mut ProjectSession) -> Result<Toolpath, SessionError> {
    let cancel = AtomicBool::new(false);
    s.generate_toolpath(0, &cancel)
        .map(|r| r.op_data.annotated().toolpath.clone())
}

/// The cutting segments of `tp` (every move that is not a rapid).
fn cut_segments(tp: &Toolpath) -> Vec<(P3, P3)> {
    tp.moves
        .windows(2)
        .filter(|pair| !matches!(pair[1].move_type, MoveType::Rapid))
        .map(|pair| (pair[0].target, pair[1].target))
        .collect()
}

/// Points along the cutting segments, at most 0.25 mm apart.
fn cut_samples(tp: &Toolpath) -> Vec<P3> {
    let mut out = Vec::new();
    for (a, b) in cut_segments(tp) {
        let n = ((b - a).norm() / 0.25).ceil().max(1.0) as usize;
        for k in 0..=n {
            out.push(a + (b - a) * (k as f64 / n as f64));
        }
    }
    out
}

/// The distance from `p` to the segment `a`-`b`, in 3D.
fn dist_to_segment(p: P3, a: P3, b: P3) -> f64 {
    let ab = b - a;
    let len2 = ab.norm_squared();
    let t = if len2 > 0.0 {
        ((p - a).dot(&ab) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (p - (a + ab * t)).norm()
}

/// The model edge items of `kind` whose form passes `form`.
fn edges(model: &LoadedModel, kind: ItemKind, form: impl Fn(EdgeForm) -> bool) -> Vec<&Item> {
    model
        .items
        .iter()
        .filter(|i| i.kind() == kind && i.z().is_some())
        .filter(|i| i.model_edge().is_some_and(|e| !e.ring && form(e.form)))
        .collect()
}

fn is_convex(form: EdgeForm) -> bool {
    matches!(form, EdgeForm::ConvexTop { .. })
}

/// The two points of a straight chain (the stepped block has no curve).
fn ends(item: &Item) -> (P2, P2) {
    let Shape::Path(p) = item.shape() else {
        panic!("a chain");
    };
    (p.exterior[0], *p.exterior.last().unwrap())
}

/// Pick `item` into the op selection of toolpath 0.
/// W3h: a pick on a new op (nothing selected) is the selection. The
/// fixture op holds the drawing rule (the default of the default claims),
/// so a pick on it writes that one pick.
fn pick(s: &mut ProjectSession, item: &Item) {
    let key = item.key().clone().for_toolpath_model(s.models()[0].id);
    let _ = s
        .apply(Command::EditSelection(EditSelectionArgs {
            index: 0,
            target: SelectionTarget::Op,
            edit: SelectionEdit::Replace(Box::new(rs_cam_core::selection::Selection::from_picks(
                vec![key],
            ))),
        }))
        .expect("the op holds a selection");
}

/// Claim 1: the form of each edge.
#[test]
fn the_step_door_states_the_form_of_each_edge_rc() {
    let m = load(STEPPED);
    let convex = edges(&m, ItemKind::Path, is_convex);
    let concave = edges(&m, ItemKind::Path, |f| f == EdgeForm::Concave);
    let other = edges(&m, ItemKind::Path, |f| f == EdgeForm::Other);
    assert_eq!(convex.len(), 10, "the convex top chains");
    assert_eq!(other.len(), 4, "the bottom chains");
    let mut concave_at: Vec<(f64, f64)> = concave
        .iter()
        .map(|i| (i.z().unwrap(), ends(i).0.x))
        .collect();
    concave_at.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mut want: Vec<(f64, f64)> = CONCAVE_EDGES.iter().map(|&(x, z)| (z, x)).collect();
    want.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert_eq!(concave_at, want, "{concave_at:?}");
    for item in &other {
        assert!(item.z().unwrap().abs() < 1e-9, "a bottom chain at Z 0");
    }
    // Each convex chain names its material side: the up face beside it.
    for item in &convex {
        let Some(EdgeForm::ConvexTop { material }) = item.model_edge().map(|e| e.form) else {
            panic!("convex");
        };
        let (a, b) = ends(item);
        let d = (b - a).normalize();
        let left = V2::new(-d.y, d.x);
        let mid = P2::new(0.5 * (a.x + b.x), 0.5 * (a.y + b.y));
        let side = match material {
            EdgeMaterial::Left => left,
            EdgeMaterial::Right => -left,
            EdgeMaterial::Inside | EdgeMaterial::Outside => panic!("an open chain"),
        };
        let p = mid + side * 0.5;
        assert!(
            (0.0..=60.0).contains(&p.x) && (0.0..=40.0).contains(&p.y),
            "the material side of {:?} lies on the part: {p:?}",
            item.key()
        );
    }
    let rings: Vec<(f64, EdgeForm)> = m
        .items
        .iter()
        .filter(|i| i.kind() == ItemKind::Loop && i.model_edge().is_some_and(|e| e.ring))
        .map(|i| (i.z().unwrap(), i.model_edge().unwrap().form))
        .collect();
    assert_eq!(rings.len(), 3);
    for (z, form) in rings {
        let want_convex = (z - TOP).abs() < 1e-9;
        assert_eq!(is_convex(form), want_convex, "the ring at Z {z}: {form:?}");
        if !want_convex {
            assert_eq!(form, EdgeForm::Concave, "the ring at Z {z}");
        }
    }

    let h = load(HOLE);
    let circles: Vec<(f64, EdgeForm)> = edges(&h, ItemKind::Circle, |_| true)
        .iter()
        .map(|i| (i.z().unwrap(), i.model_edge().unwrap().form))
        .collect();
    assert_eq!(circles.len(), 2);
    for (z, form) in circles {
        if (z - 20.0).abs() < 1e-9 {
            assert_eq!(
                form,
                EdgeForm::ConvexTop {
                    material: EdgeMaterial::Outside
                },
                "the rim"
            );
        } else {
            assert_eq!(form, EdgeForm::Concave, "the floor circle at Z {z}");
        }
    }
}

/// Claim 2: the default takes the 10 convex chains only.
#[test]
fn the_default_chamfer_takes_the_convex_chains_only_rc() {
    let s = chamfer_session(HeightMode::Manual(TOP));
    let tc = s.toolpath_configs()[0].clone();
    let report = s.selection_status(&tc, SelectionTarget::Op).unwrap();
    assert_eq!(report.refusal, None);
    let mut cut = 0;
    for (item, state) in s.models()[0].items.iter().zip(&report.item_states) {
        let convex_chain = item.kind() == ItemKind::Path
            && item
                .model_edge()
                .is_some_and(|e| !e.ring && e.is_convex_top());
        let selected = *state != ItemState::NotSelected;
        assert_eq!(
            selected,
            convex_chain,
            "{:?} at Z {:?} ({:?}) is {state:?}",
            item.kind(),
            item.z(),
            item.model_edge()
        );
        cut += usize::from(*state == ItemState::Cut);
    }
    assert_eq!(cut, 10);
}

/// Claim 3 (and 4): the emitted motion of the default Chamfer.
#[test]
fn the_default_chamfer_cuts_each_convex_edge_at_its_own_z_rc() {
    let mut s = chamfer_session(HeightMode::Manual(TOP));
    let (depth, t) = tip_depth(&s);
    let tp = generate(&mut s).expect("the default Chamfer generates");
    let segments = cut_segments(&tp);
    let samples = cut_samples(&tp);
    assert!(!samples.is_empty());
    let model = s.models()[0].clone();

    // Each convex edge: its tip line at its own Z, on its air side.
    for item in edges(&model, ItemKind::Path, is_convex) {
        let z = item.z().unwrap();
        let Some(EdgeForm::ConvexTop { material }) = item.model_edge().map(|e| e.form) else {
            unreachable!()
        };
        let (a, b) = ends(item);
        let d = (b - a).normalize();
        let left = V2::new(-d.y, d.x);
        let air = match material {
            EdgeMaterial::Left => -left,
            EdgeMaterial::Right | EdgeMaterial::Inside | EdgeMaterial::Outside => left,
        };
        let mid = P2::new(0.5 * (a.x + b.x), 0.5 * (a.y + b.y)) + air * t;
        let want = P3::new(mid.x, mid.y, z - depth);
        let nearest = segments
            .iter()
            .map(|&(p, q)| dist_to_segment(want, p, q))
            .fold(f64::INFINITY, f64::min);
        assert!(
            nearest < 1e-3,
            "the edge at Z {z} from {a:?} to {b:?} is cut at {want:?} (nearest cut {nearest:.4})"
        );
    }

    // F21: one cut level per edge Z, no other (the level of each
    // horizontal cutting move; a plunge only descends to it).
    let mut levels: Vec<f64> = segments
        .iter()
        .filter(|(p, q)| (p.z - q.z).abs() < 1e-9 && (p.xy() - q.xy()).norm() > 0.0)
        .map(|(p, _)| p.z)
        .filter(|z| *z < TOP - depth + 1e-6)
        .map(|z| (z * 1000.0).round() / 1000.0)
        .collect();
    levels.sort_by(f64::total_cmp);
    levels.dedup();
    let want: Vec<f64> = [10.0, 20.0, 30.0]
        .iter()
        .map(|z| ((z - depth) * 1000.0_f64).round() / 1000.0)
        .collect();
    assert_eq!(levels, want, "the cut levels");

    // No cut runs along a concave edge: no sample within 0.5 mm of its
    // inside (away from its two ends by the cone reach) at or below its Z.
    // The two concave edges are fixed by the fixture geometry (each step
    // wall meets the face below it), not read from the door, so a door
    // that calls them convex cannot make this check empty.
    let reach = depth + t + 0.5;
    for (x, z) in CONCAVE_EDGES {
        let (a, b) = (P2::new(x, 0.0), P2::new(x, 40.0));
        let len = (b - a).norm();
        let d = (b - a) / len;
        for p in &samples {
            let u = (P2::new(p.x, p.y) - a).dot(&d);
            if u < reach || u > len - reach || p.z > z + 0.5 {
                continue;
            }
            let off = (P2::new(p.x, p.y) - (a + d * u)).norm();
            assert!(
                off > 0.5,
                "a cut runs along the concave edge at Z {z} X {x}: {p:?}"
            );
        }
    }

    // Claim 4: the four chains that end at a step wall stop short of it,
    // so nothing cuts into a wall (Chamfer Clearance).
    let finding = s
        .result_at(0)
        .unwrap()
        .stats
        .model_gouge
        .expect("the guard ran");
    assert!(finding.judged > 0, "non-vacuity: the guard judged samples");
    assert!(!finding.gouges(), "{}", finding.message());
}

/// Claim 4: the bevel of the Z 30 chains alone measures clean.
#[test]
fn a_bevel_of_convex_corners_only_measures_clean_rc() {
    let mut s = chamfer_session(HeightMode::Manual(TOP));
    let model = s.models()[0].clone();
    let top_chains: Vec<&Item> = edges(&model, ItemKind::Path, is_convex)
        .into_iter()
        .filter(|i| (i.z().unwrap() - TOP).abs() < 1e-9)
        .collect();
    assert_eq!(top_chains.len(), 4);
    for item in top_chains {
        pick(&mut s, item);
    }
    let _ = generate(&mut s).expect("the Z 30 chains generate");
    let finding = s.result_at(0).unwrap().stats.model_gouge.unwrap();
    assert!(finding.judged > 0, "non-vacuity: the guard judged samples");
    assert!(!finding.gouges(), "{}", finding.message());
}

/// Claim 5: concave picks refuse by name; the convex ring cuts at Z 30.
#[test]
fn a_concave_pick_refuses_and_a_convex_ring_cuts_rc() {
    let model = load(STEPPED);
    let concave_chain = edges(&model, ItemKind::Path, |f| f == EdgeForm::Concave)
        .into_iter()
        .find(|i| (i.z().unwrap() - 20.0).abs() < 1e-9)
        .unwrap()
        .clone();
    let ring = |z: f64| {
        model
            .items
            .iter()
            .find(|i| {
                i.kind() == ItemKind::Loop
                    && i.model_edge().is_some_and(|e| e.ring)
                    && (i.z().unwrap() - z).abs() < 1e-9
            })
            .unwrap()
            .clone()
    };
    for (item, name) in [
        (concave_chain, "open path (starts at X 20.000 Y 0.000"),
        (ring(20.0), "closed loop (centre X 30.000 Y 20.000"),
    ] {
        let mut s = chamfer_session(HeightMode::Manual(TOP));
        pick(&mut s, &item);
        let Err(SessionError::Selection(r)) = generate(&mut s) else {
            panic!("a pick of a concave edge must refuse");
        };
        assert_eq!(
            r.cause,
            RefusalCause::NotAConvexEdge {
                why: EdgeRefusal::Concave
            },
            "{r}"
        );
        let text = r.to_string();
        assert!(text.contains(name), "the refusal names the item: {text}");
        assert!(text.contains("this edge is concave"), "{text}");
    }

    let mut s = chamfer_session(HeightMode::Manual(TOP));
    let (depth, _) = tip_depth(&s);
    pick(&mut s, &ring(TOP));
    let tp = generate(&mut s).expect("the convex Z 30 ring generates");
    let samples = cut_samples(&tp);
    assert!(!samples.is_empty());
    let lowest = samples.iter().map(|p| p.z).fold(f64::INFINITY, f64::min);
    assert!(
        (lowest - (TOP - depth)).abs() < 1e-6,
        "at Z 30 only: {lowest}"
    );
}

/// Claim 6: fresh stock and an auto top refuse by the lowest item.
#[test]
fn a_fresh_auto_top_chamfer_is_buried_by_its_lowest_edge_rc() {
    let mut s = chamfer_session(HeightMode::Auto);
    match generate(&mut s) {
        Err(SessionError::Operation(OperationError::BuriedFace(refusal))) => {
            assert_eq!(refusal.under, BuriedUnder::StockTop, "{refusal}");
            assert!(
                (refusal.buried_mm - 20.0).abs() < 1e-6,
                "the Z 10 chains are 20 mm under the stock top: {refusal}"
            );
        }
        Err(other) => panic!("want the buried refusal, got: {other}"),
        Ok(_) => panic!("the Z 10 and Z 20 chains are under fresh stock"),
    }
}

/// Claim 7: a Bottom setup refuses the model edges.
#[test]
fn a_turned_setup_refuses_the_model_edges_rc() {
    let mut s = chamfer_session(HeightMode::Manual(TOP));
    let _ = s
        .apply(Command::SetSetupFace(SetSetupFaceArgs {
            setup_index: 0,
            face_up: FaceUp::Bottom,
        }))
        .unwrap();
    let Err(SessionError::Selection(r)) = generate(&mut s) else {
        panic!("a turned setup must refuse");
    };
    assert_eq!(
        r.cause,
        RefusalCause::NotAConvexEdge {
            why: EdgeRefusal::TurnedSetup
        },
        "{r}"
    );
}
