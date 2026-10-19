//! P2.2 sentries: the rules, the resolver, the frame door and the view
//! (DESIGN section 14, row P2.2; lead ruling D2/D13).
//!
//! `cargo test -p rs_cam_core -q --lib selection`

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::sync::Arc;

use crate::compute::catalog::ItemUse;
use crate::compute::transform::{FaceUp, SetupTransformInfo, ZRotation};
use crate::geo::{BoundingBox3, P2, P3, V3};
use crate::geometry::enriched_mesh::FaceGroupId;
use crate::ids::ModelId;
use crate::polygon::Polygon2;
use crate::session::LoadedModel;

use super::*;

const MODEL_ID: usize = 3;

fn square(x0: f64, y0: f64, s: f64) -> Polygon2 {
    Polygon2::rectangle(x0, y0, x0 + s, y0 + s)
}

fn item(layer: &str, shape: Shape) -> Item {
    Item::new(SourceRef::ToolpathModel, Some(layer.into()), shape).unwrap()
}

fn path(layer: &str, pts: &[[f64; 2]]) -> Item {
    item(
        layer,
        Shape::Path(Polygon2::open_path(
            pts.iter().map(|p| P2::new(p[0], p[1])).collect(),
        )),
    )
}

fn face(plane_z: f64, normal_z: f64, brep: u16) -> Item {
    Item::new(
        SourceRef::ToolpathModel,
        None,
        Shape::Face(FaceItem {
            rings: square(0.0, 0.0, 10.0),
            plane_z,
            normal: V3::new(0.0, 0.0, normal_z),
            brep: Some(FaceGroupId(brep)),
        }),
    )
    .unwrap()
}

/// A model of `items`, bound to `Model(MODEL_ID)` as the session binds it.
fn model(items: Vec<Item>) -> LoadedModel {
    let mut m = LoadedModel {
        id: MODEL_ID,
        name: "p2_2".to_owned(),
        mesh: None,
        items: Arc::new(ItemSet::new(items)),
        path: std::path::PathBuf::from("synthetic://p2_2.dxf"),
        kind: None,
        units: None,
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
    };
    m.bind_items();
    m
}

fn stock_bbox() -> BoundingBox3 {
    BoundingBox3 {
        min: P3::new(0.0, 0.0, 0.0),
        max: P3::new(100.0, 80.0, 10.0),
    }
}

/// A `Bottom` flip of the 100 x 80 x 10 stock at the origin.
fn flipped() -> SetupTransformInfo {
    SetupTransformInfo {
        face_up: FaceUp::Bottom,
        z_rotation: ZRotation::Deg0,
        stock_x: 100.0,
        stock_y: 80.0,
        stock_z: 10.0,
        stock_origin_x: 0.0,
        stock_origin_y: 0.0,
        stock_origin_z: 0.0,
    }
}

fn resolve_on(sel: &Selection, m: &LoadedModel) -> ResolvedSelection {
    resolve_in(sel, m, None)
}

fn resolve_in(
    sel: &Selection,
    m: &LoadedModel,
    setup: Option<&SetupTransformInfo>,
) -> ResolvedSelection {
    let bbox = stock_bbox();
    resolve(
        sel,
        &ResolveScope {
            toolpath_model: Ok(m),
            models: std::slice::from_ref(m),
            stock_bbox: &bbox,
            setup,
        },
    )
}

fn rule(filter: Filter) -> Selection {
    Selection::from_rules(vec![Rule {
        source: SourceRef::ToolpathModel,
        filter,
    }])
}

fn only(kinds: &[ItemKind]) -> Filter {
    Filter {
        kinds: KindFilter::Only(kinds.to_vec()),
        ..Filter::default()
    }
}

/// The key of `item` as a stored pick names it: source `ToolpathModel`.
fn pick_key(item: &Item) -> MatchKey {
    MatchKey {
        source: SourceRef::ToolpathModel,
        ..item.key().clone()
    }
}

fn kinds(r: &ResolvedSelection) -> Vec<ItemKind> {
    r.items().iter().map(|i| i.item().kind()).collect()
}

fn mixed() -> LoadedModel {
    model(vec![
        item("A", Shape::Loop(square(0.0, 0.0, 10.0))),
        path("TEXT", &[[0.0, 20.0], [5.0, 25.0]]),
        item("B", Shape::Point(P2::new(50.0, 50.0))),
        item("B", Shape::circle(P2::new(60.0, 60.0), 6.0)),
        face(5.0, 1.0, 0),
    ])
}

// ── Filters ─────────────────────────────────────────────────────────────

#[test]
fn the_served_filter_matches_every_drawing_kind_and_no_face() {
    let r = resolve_on(&Selection::drawing(), &mixed());
    assert_eq!(
        kinds(&r),
        [
            ItemKind::Loop,
            ItemKind::Path,
            ItemKind::Point,
            ItemKind::Circle
        ]
    );
    assert!(
        r.items()
            .iter()
            .all(|i| i.by() == MatchedBy::Default { rule: 0 })
    );
}

#[test]
fn an_only_filter_matches_its_kinds_and_is_explicit() {
    let r = resolve_on(&rule(only(&[ItemKind::Face, ItemKind::Point])), &mixed());
    assert_eq!(kinds(&r), [ItemKind::Point, ItemKind::Face]);
    assert!(r.items().iter().all(|i| i.by().is_explicit()));
}

#[test]
fn a_layer_filter_matches_its_layers_only() {
    let r = resolve_on(
        &rule(Filter {
            layers: vec!["B".into()],
            ..Filter::default()
        }),
        &mixed(),
    );
    assert_eq!(kinds(&r), [ItemKind::Point, ItemKind::Circle]);
    assert!(r.unmatched_layers().is_empty());
}

#[test]
fn a_diameter_filter_admits_circles_in_range_only() {
    let m = model(vec![
        item("H", Shape::circle(P2::new(0.0, 0.0), 5.0)),
        item("H", Shape::circle(P2::new(10.0, 0.0), 6.0)),
        item("H", Shape::circle(P2::new(20.0, 0.0), 6.4)),
        item("H", Shape::Loop(square(30.0, 0.0, 6.0))),
    ]);
    let r = resolve_on(
        &rule(Filter {
            diameter: Some(DiameterRange {
                nominal_mm: 6.0,
                tolerance_mm: 0.5,
            }),
            ..Filter::default()
        }),
        &m,
    );
    let diameters: Vec<f64> = r.items().iter().map(|i| i.identity().measure).collect();
    assert_eq!(diameters, [6.0, 6.4]);
}

#[test]
fn a_face_filter_reads_the_facing_and_the_z_in_the_setup_frame() {
    let m = model(vec![face(10.0, 1.0, 0), face(0.0, -1.0, 1)]);
    let face_rule = |facing, z| {
        rule(Filter {
            kinds: KindFilter::Only(vec![ItemKind::Face]),
            face: Some(FaceFilter { facing, z }),
            ..Filter::default()
        })
    };
    let breps = |r: &ResolvedSelection| -> Vec<Option<u16>> {
        r.items().iter().map(|i| i.identity().brep).collect()
    };
    let up = face_rule(Facing::Up, None);
    assert_eq!(breps(&resolve_on(&up, &m)), [Some(0)]);
    let down = face_rule(Facing::Down, None);
    assert_eq!(breps(&resolve_on(&down, &m)), [Some(1)]);
    let high = face_rule(
        Facing::Either,
        Some(ZRange {
            min: 5.0,
            max: 20.0,
        }),
    );
    assert_eq!(breps(&resolve_on(&high, &m)), [Some(0)]);
    // On a `Bottom` flip the model bottom faces up, at the setup top.
    let info = flipped();
    assert_eq!(breps(&resolve_in(&up, &m, Some(&info))), [Some(1)]);
    assert_eq!(breps(&resolve_in(&high, &m, Some(&info))), [Some(1)]);
    // A face filter admits no other kind.
    let r = resolve_on(
        &rule(Filter {
            face: Some(FaceFilter {
                facing: Facing::Either,
                z: None,
            }),
            ..Filter::default()
        }),
        &mixed(),
    );
    assert!(r.items().is_empty());
}

// ── Picks and excludes ──────────────────────────────────────────────────

#[test]
fn a_pick_matches_the_bound_item_of_its_source() {
    let m = mixed();
    let circle = m.items.iter().nth(3).unwrap().clone();
    assert_eq!(circle.source(), SourceRef::Model(ModelId(MODEL_ID)));
    let r = resolve_on(&Selection::from_picks(vec![pick_key(&circle)]), &m);
    assert!(r.lost().is_empty());
    assert_eq!(kinds(&r), [ItemKind::Circle]);
    assert_eq!(r.items()[0].by(), MatchedBy::Pick);
    // A pick that names the model by id matches too.
    let by_id = MatchKey {
        source: SourceRef::Model(ModelId(MODEL_ID)),
        ..pick_key(&circle)
    };
    let r = resolve_on(&Selection::from_picks(vec![by_id]), &m);
    assert_eq!(kinds(&r), [ItemKind::Circle]);
}

#[test]
fn a_moved_item_loses_its_pick() {
    let m = mixed();
    let circle = m.items.iter().nth(3).unwrap();
    let mut key = pick_key(circle);
    key.anchor[0] += 0.01;
    let r = resolve_on(&Selection::from_picks(vec![key.clone()]), &m);
    assert!(r.items().is_empty());
    assert_eq!(r.lost(), [LostPick { key, matches: 0 }]);
    let refusal = view(
        &r,
        ItemUse::Holes {
            may_be_empty: false,
        },
    )
    .unwrap_err();
    assert_eq!(refusal.cause, RefusalCause::LostPick);
    assert!(refusal.item.is_some());
}

#[test]
fn two_equal_items_make_a_pick_ambiguous_and_stay_two_items() {
    let twin = || item("H", Shape::circle(P2::new(5.0, 5.0), 3.0));
    let m = model(vec![twin(), twin()]);
    // The rule keeps both: two entities, as the old target list did.
    assert_eq!(resolve_on(&Selection::drawing(), &m).items().len(), 2);
    let key = pick_key(m.items.iter().next().unwrap());
    let r = resolve_on(&Selection::from_picks(vec![key]), &m);
    assert_eq!(r.lost()[0].matches, 2);
    let refusal = view(
        &r,
        ItemUse::Holes {
            may_be_empty: false,
        },
    )
    .unwrap_err();
    assert_eq!(refusal.cause, RefusalCause::AmbiguousPick { matches: 2 });
}

#[test]
fn an_exclude_removes_a_rule_match_and_not_a_pick() {
    let m = mixed();
    let point = m.items.iter().nth(2).unwrap().clone();
    let default = Selection::drawing();
    let mut sel = Selection::drawing();
    sel.apply(SelectionEdit::RemovePick(pick_key(&point)), &default)
        .unwrap();
    let r = resolve_on(&sel, &m);
    assert_eq!(
        kinds(&r),
        [ItemKind::Loop, ItemKind::Path, ItemKind::Circle]
    );
    // An exclude and a pick of one item (stored by hand): the pick wins.
    let key = serde_json::to_value(pick_key(&point)).unwrap();
    let both: Selection =
        serde_json::from_value(serde_json::json!({ "excludes": [key], "picks": [key] })).unwrap();
    let r = resolve_on(&both, &m);
    assert_eq!(kinds(&r), [ItemKind::Point]);
}

#[test]
fn a_rule_and_a_pick_of_one_item_give_it_once_as_the_pick() {
    let m = mixed();
    let path_item = m.items.iter().nth(1).unwrap().clone();
    let mut sel = Selection::drawing();
    sel.apply(
        SelectionEdit::AddPick(pick_key(&path_item)),
        &Selection::drawing(),
    )
    .unwrap();
    let r = resolve_on(&sel, &m);
    assert_eq!(r.items().len(), 4);
    assert_eq!(r.items()[1].by(), MatchedBy::Pick);
}

// ── Sources and layers ──────────────────────────────────────────────────

#[test]
fn a_missing_model_and_an_unknown_layer_refuse() {
    let m = mixed();
    let gone = Selection::from_rules(vec![Rule {
        source: SourceRef::Model(ModelId(99)),
        filter: Filter::default(),
    }]);
    let r = resolve_on(&gone, &m);
    assert_eq!(r.missing_sources(), [SourceRef::Model(ModelId(99))]);
    assert_eq!(
        view(&r, ItemUse::Regions).unwrap_err().cause,
        RefusalCause::MissingSource(SourceRef::Model(ModelId(99)))
    );
    let typo = rule(Filter {
        layers: vec!["A".into(), "CUTT".into()],
        ..Filter::default()
    });
    let r = resolve_on(&typo, &m);
    assert_eq!(r.unmatched_layers(), ["CUTT"]);
    assert_eq!(
        view(&r, ItemUse::Regions).unwrap_err().cause,
        RefusalCause::UnknownLayer("CUTT".into())
    );
}

#[test]
fn the_stock_selection_is_the_stock_rectangle_and_does_not_move() {
    let m = mixed();
    let r = resolve_on(&Selection::stock(), &m);
    assert_eq!(kinds(&r), [ItemKind::Loop]);
    let r = r.to_setup_frame(&flipped()).unwrap();
    let (items, _) = view(&r, ItemUse::Regions).unwrap();
    let ring = &items.regions().unwrap().as_slice()[0];
    let b = ring.bbox();
    assert_eq!(b, [0.0, 0.0, 100.0, 80.0]);
}

// ── Containment ─────────────────────────────────────────────────────────

#[test]
fn containment_runs_over_the_selected_loops_only() {
    let m = model(vec![
        item("A", Shape::Loop(square(0.0, 0.0, 40.0))),
        item("B", Shape::Loop(square(10.0, 10.0, 10.0))),
        item("B", Shape::circle(P2::new(30.0, 30.0), 4.0)),
    ]);
    let regions = |sel: &Selection| -> Vec<(usize, usize)> {
        let (items, _) = view(&resolve_on(sel, &m), ItemUse::Regions).unwrap();
        items
            .regions()
            .unwrap()
            .as_slice()
            .iter()
            .map(|p| (p.exterior.len(), p.holes.len()))
            .collect()
    };
    // Every loop: the square holds two islands (a loop and a circle ring).
    assert_eq!(regions(&Selection::drawing()), [(4, 2)]);
    // Layer A alone: no hole comes from layer B.
    let a = rule(Filter {
        layers: vec!["A".into()],
        ..Filter::default()
    });
    assert_eq!(regions(&a), [(4, 0)]);
    // Layer B alone: two top-level regions.
    let b = rule(Filter {
        layers: vec!["B".into()],
        ..Filter::default()
    });
    assert_eq!(regions(&b), [(4, 0), (72, 0)]);
}

#[test]
fn a_face_keeps_its_own_inner_loops_and_does_not_nest() {
    let mut rings = square(0.0, 0.0, 40.0);
    let mut hole = square(5.0, 5.0, 5.0).exterior;
    hole.reverse();
    rings.holes.push(hole);
    let big = Item::new(
        SourceRef::ToolpathModel,
        None,
        Shape::Face(FaceItem {
            rings,
            plane_z: 0.0,
            normal: V3::new(0.0, 0.0, 1.0),
            brep: Some(FaceGroupId(0)),
        }),
    )
    .unwrap();
    // A second face inside the first: it is its own region, not a hole.
    let m = model(vec![big, face(5.0, 1.0, 1)]);
    let (items, _) = view(
        &resolve_on(&rule(only(&[ItemKind::Face])), &m),
        ItemUse::Regions,
    )
    .unwrap();
    let holes: Vec<usize> = items
        .regions()
        .unwrap()
        .as_slice()
        .iter()
        .map(|p| p.holes.len())
        .collect();
    assert_eq!(holes, [1, 0]);
}

// ── The skip-versus-refuse rule (lead ruling D2/D13) ────────────────────

fn outline_and_text() -> LoadedModel {
    model(vec![
        item("OUTER", Shape::Loop(square(0.0, 0.0, 30.0))),
        path("TEXT", &[[40.0, 0.0], [45.0, 5.0]]),
        path("TEXT", &[[40.0, 10.0], [45.0, 15.0]]),
    ])
}

#[test]
fn the_default_rule_skips_an_item_the_op_cannot_cut_with_a_note() {
    let m = outline_and_text();
    let (items, notes) = view(&resolve_on(&Selection::drawing(), &m), ItemUse::Regions).unwrap();
    assert_eq!(items.regions().unwrap().as_slice().len(), 1);
    assert_eq!(notes.len(), 1);
    let note = &notes[0];
    assert_eq!(
        (note.kind, note.layer.as_deref(), note.count),
        (ItemKind::Path, Some("TEXT"), 2)
    );
    assert_eq!(note.first.key.anchor, [40.0, 0.0]);
    let text = note.to_string();
    assert!(
        text.starts_with("2 open paths on layer 'TEXT' are not cut"),
        "{text}"
    );
    // The same paths are lines for a `Curves` op: no note.
    let (items, notes) = view(&resolve_on(&Selection::drawing(), &m), ItemUse::Curves).unwrap();
    assert_eq!(items.curves().unwrap().as_slice().len(), 3);
    assert!(notes.is_empty());
}

#[test]
fn a_named_item_the_op_cannot_cut_refuses() {
    let m = outline_and_text();
    // A rule that names the path kind.
    let named = rule(only(&[ItemKind::Loop, ItemKind::Path]));
    let refusal = view(&resolve_on(&named, &m), ItemUse::Regions).unwrap_err();
    assert_eq!(
        refusal.cause,
        RefusalCause::KindNotCut {
            kind: ItemKind::Path,
            item_use: ItemUse::Regions
        }
    );
    assert_eq!(
        refusal.item.as_ref().unwrap().key.layer.as_deref(),
        Some("TEXT")
    );
    // A pick of the path, on top of the default rule.
    let mut picked = Selection::drawing();
    picked
        .apply(
            SelectionEdit::AddPick(pick_key(m.items.iter().nth(1).unwrap())),
            &Selection::drawing(),
        )
        .unwrap();
    let refusal = view(&resolve_on(&picked, &m), ItemUse::Regions)
        .unwrap_err()
        .for_op("Pocket");
    let text = refusal.to_string();
    assert!(
        text.starts_with("Pocket: open path on layer 'TEXT' (starts at X 40.000 Y 0.000"),
        "{text}"
    );
    assert!(text.contains("Fix: "), "{text}");
}

#[test]
fn nothing_left_to_cut_refuses_empty_and_names_the_skipped_items() {
    let m = model(vec![
        path("TEXT", &[[0.0, 0.0], [5.0, 5.0]]),
        path("TEXT", &[[0.0, 10.0], [5.0, 15.0]]),
    ]);
    let refusal = view(&resolve_on(&Selection::drawing(), &m), ItemUse::Regions).unwrap_err();
    let RefusalCause::Empty { skipped } = &refusal.cause else {
        panic!("{refusal:?}");
    };
    assert_eq!(skipped.len(), 1);
    assert_eq!(skipped[0].count, 2);
    let text = refusal.to_string();
    assert!(text.contains("2 open paths on layer 'TEXT'"), "{text}");
    // A STEP model with the default selection: no face is a drawing item.
    let step = model(vec![face(5.0, 1.0, 0)]);
    let refusal = view(&resolve_on(&Selection::drawing(), &step), ItemUse::Regions).unwrap_err();
    assert_eq!(refusal.cause, RefusalCause::Empty { skipped: vec![] });
    // A drill with no hole refuses; a pin drill may be empty.
    let r = resolve_on(&Selection::drawing(), &m);
    assert!(matches!(
        view(
            &r,
            ItemUse::Holes {
                may_be_empty: false
            }
        )
        .unwrap_err()
        .cause,
        RefusalCause::Empty { .. }
    ));
    let r = resolve_on(&Selection::empty(), &m);
    let (items, _) = view(&r, ItemUse::Holes { may_be_empty: true }).unwrap();
    assert!(items.holes().unwrap().as_slice().is_empty());
}

/// Lead ruling "notes" (P2.2): a skipped point gives no note, only a
/// count. `Empty` still names it.
#[test]
fn only_a_skipped_open_path_gives_a_note() {
    let m = model(vec![
        item("A", Shape::Loop(square(0.0, 0.0, 10.0))),
        item("B", Shape::Point(P2::new(50.0, 50.0))),
    ]);
    let (items, notes) = view(&resolve_on(&Selection::drawing(), &m), ItemUse::Regions).unwrap();
    assert!(notes.is_empty(), "{notes:?}");
    assert_eq!(items.skipped_by_kind().get(&ItemKind::Point), Some(&1));
    let points = model(vec![item("B", Shape::Point(P2::new(50.0, 50.0)))]);
    let refusal = view(
        &resolve_on(&Selection::drawing(), &points),
        ItemUse::Regions,
    )
    .unwrap_err();
    let RefusalCause::Empty { skipped } = &refusal.cause else {
        panic!("{refusal:?}");
    };
    assert_eq!(skipped.len(), 1);
    assert_eq!(skipped[0].kind, ItemKind::Point);
}

#[test]
fn the_holes_are_the_points_and_the_circle_centres() {
    let (items, notes) = view(
        &resolve_on(&Selection::drawing(), &mixed()),
        ItemUse::Holes {
            may_be_empty: false,
        },
    )
    .unwrap();
    let holes = items.holes().unwrap().as_slice();
    assert_eq!(holes.len(), 2);
    assert_eq!(
        (holes[0].xy, holes[0].diameter),
        (P2::new(50.0, 50.0), None)
    );
    assert_eq!(
        (holes[1].xy, holes[1].diameter),
        (P2::new(60.0, 60.0), Some(6.0))
    );
    // The loop and the path are left out. Both count; only the open path
    // gives a note (lead ruling "notes", P2.2).
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].kind, ItemKind::Path);
    assert_eq!(
        items
            .skipped_by_kind()
            .iter()
            .map(|(k, n)| (*k, *n))
            .collect::<Vec<_>>(),
        [(ItemKind::Loop, 1), (ItemKind::Path, 1)]
    );
}

// ── Keys and the frame door ─────────────────────────────────────────────

#[test]
fn a_face_key_has_no_z() {
    let low = face(0.0, 1.0, 4);
    let high = face(25.0, 1.0, 4);
    assert!(low.key().names_same_geometry(high.key()));
    // A pick made at Z 0 finds the face after it moves to Z 25.
    let m = model(vec![high]);
    let r = resolve_on(&Selection::from_picks(vec![pick_key(&low)]), &m);
    assert!(r.lost().is_empty());
    assert_eq!(kinds(&r), [ItemKind::Face]);
}

#[test]
fn the_frame_door_turns_a_face_down_on_a_flipped_setup() {
    let m = model(vec![
        face(10.0, 1.0, 0),
        item("A", Shape::Loop(square(10.0, 10.0, 20.0))),
        item("H", Shape::circle(P2::new(20.0, 30.0), 5.0)),
        path("T", &[[1.0, 2.0], [3.0, 4.0]]),
    ]);
    let sel = Selection::from_rules(vec![Rule {
        source: SourceRef::ToolpathModel,
        filter: only(&[
            ItemKind::Face,
            ItemKind::Loop,
            ItemKind::Circle,
            ItemKind::Path,
        ]),
    }]);
    let r = resolve_on(&sel, &m);
    let identity_info = SetupTransformInfo {
        face_up: FaceUp::Top,
        ..flipped()
    };
    // An identity setup changes nothing.
    let same = r.clone().to_setup_frame(&identity_info).unwrap();
    let Shape::Face(f) = same.items()[0].item().shape() else {
        panic!()
    };
    assert_eq!((f.plane_z, f.normal.z), (10.0, 1.0));
    // A `Bottom` flip: the top face (Z 10 of 10) is at Z 0 and faces down.
    let moved = r.to_setup_frame(&flipped()).unwrap();
    let Shape::Face(f) = moved.items()[0].item().shape() else {
        panic!()
    };
    assert!(f.plane_z.abs() < 1e-12, "{}", f.plane_z);
    assert!((f.normal.z + 1.0).abs() < 1e-12, "{:?}", f.normal);
    // The rings take the drawing door: Y mirrors about the stock (80), and
    // a closed ring stays counter-clockwise.
    let Shape::Loop(ring) = moved.items()[1].item().shape() else {
        panic!()
    };
    let b = ring.bbox();
    assert_eq!([b[1], b[3]], [50.0, 70.0]);
    assert!(ring.area() > 0.0);
    let Shape::Circle { centre, .. } = moved.items()[2].item().shape() else {
        panic!()
    };
    assert_eq!(*centre, P2::new(20.0, 50.0));
    // An open path keeps its point order (the machining direction).
    let Shape::Path(p) = moved.items()[3].item().shape() else {
        panic!()
    };
    assert_eq!(p.exterior, [P2::new(1.0, 78.0), P2::new(3.0, 76.0)]);
    // The identity key stays in the frame of the drawing.
    assert_eq!(moved.items()[2].identity().anchor, [20.0, 30.0]);
    assert_eq!(moved.items()[2].item().key().anchor, [20.0, 50.0]);
}
