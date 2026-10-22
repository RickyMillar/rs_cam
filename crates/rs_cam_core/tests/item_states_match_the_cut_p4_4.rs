//! Geometry selection P4.4: the item states of `selection_status` name what
//! generation cuts.
//!
//! # The claim
//!
//! `SelectionReport::item_states` gives one state per item of the toolpath
//! model, index-aligned with `LoadedModel::items`; `foreign` gives the
//! state of every other selected item by key. The viewport colours each
//! item by this state, so each state must agree with generation:
//!
//! 1. `Cut` and `Island`: the rings of the `Cut` loops and circles are the
//!    exteriors of the regions that the prologue receives, and the rings
//!    of the `Island` items are their holes (a `Regions` op). A `Curves`
//!    op cuts every served ring, a `Holes` op every served point and
//!    circle centre.
//! 2. `Skipped`: the report counts its kind in `skipped_by_kind`, and a
//!    skipped open path is in a note of its layer.
//! 3. `Excluded`: an exclude of the selection names the item.
//! 4. `NotSelected`: no rule and no pick of the selection holds the item.
//! 5. A refused selection marks no item `Cut` or `Island`.
//! 6. `foreign` never holds an item of the toolpath model. A Face op on
//!    its stock default gives the stock item as `Cut` in `foreign`.
//! 7. A re-import (a new item set) builds the report again with the same
//!    states.
//! 8. Geometry selection P6.4: a Pocket with Extend to stock gives the open
//!    path the state `Cut` (it is the exterior of its extended area), a
//!    loop inside the area `Island`, and `extended` the outer ring of that
//!    area. The reference grows the stock by the tool radius, as the
//!    prologue does.
//!
//! The reference is the selection of `resolve_generation_inputs` (the
//! input of the prologue) through `selection::view`, the call the
//! prologue makes.
//!
//! # The fixture
//!
//! `fixtures/selection_p4_4/states.dxf` (mm), written with ezdxf:
//!
//! - layer `OUTER`: three nested squares, 0..60, 10..50 and 20..40 (an
//!   island, and a region inside the island);
//! - layer `HOLES`: a circle of diameter 4 at (5, 5), inside the outer
//!   square only (an island of a Pocket, a hole of a Drill), and a point
//!   at (80, 10);
//! - layer `TEXT`: an open path (0,70) (15,80) (30,70);
//! - layer `KEEP`: a square 70..90 x 30..50 (the exclude arm);
//! - layer `SPARE`: a square 100..110 x 0..10.
//!
//! Red proof (run, red, reverted): `view.rs` `nest_closed_rings_at` with
//! the hole set "every ring but the largest" (an even area order, not the
//! nesting): the region inside the island became an island, and claim 1
//! went red on the Pocket arm.
//!
//! ```text
//! cargo test -p rs_cam_core -q --test item_states_match_the_cut_p4_4
//! ```

#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

mod common;

use std::collections::BTreeSet;
use std::sync::atomic::AtomicBool;

use common::session::single_op_session;
use common::tools::endmill_tool_config;
use rs_cam_core::compute::catalog::{ItemUse, OperationConfig, OperationType};
use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits, StockConfig};
use rs_cam_core::geo::P2;
use rs_cam_core::selection::{
    Filter, HoleFit, ItemKind, MatchKey, NoteCause, Rule, Selection, SelectionEdit, Shape,
    SourceRef, fit_holes, view,
};
use rs_cam_core::session::{
    AdoptModelGeometryArgs, Command, EditSelectionArgs, ItemState, LoadedModel, ProjectSession,
    SelectionReport, SelectionTarget,
};

fn load() -> LoadedModel {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/selection_p4_4/states.dxf");
    rs_cam_core::io::load_model_file(
        &path,
        rs_cam_core::ids::ModelId::from_raw(0),
        ModelKind::Dxf,
        ModelUnits::Millimeters,
        Default::default(),
    )
    .expect("the drawing loads")
}

fn stock() -> StockConfig {
    StockConfig {
        x: 120.0,
        y: 90.0,
        z: 10.0,
        origin_x: 0.0,
        origin_y: 0.0,
        origin_z: -10.0,
        auto_from_model: false,
        ..StockConfig::default()
    }
}

/// The tool diameter of every arm (mm).
const TOOL_DIAMETER: f64 = 3.0;
/// The diameter of the `HOLES` circle of the fixture (mm).
const CIRCLE_DIAMETER: f64 = 4.0;

fn session(op: OperationType) -> ProjectSession {
    session_with_tool(op, TOOL_DIAMETER)
}

fn session_with_tool(op: OperationType, tool_diameter: f64) -> ProjectSession {
    single_op_session(
        stock(),
        endmill_tool_config(tool_diameter),
        load(),
        op.kind_str(),
        OperationConfig::new_default(op),
    )
}

fn edit(s: &mut ProjectSession, edit: SelectionEdit) {
    let _ = s
        .apply(Command::EditSelection(EditSelectionArgs {
            index: 0,
            target: SelectionTarget::Op,
            edit,
        }))
        .expect("the edit applies");
}

fn report(s: &ProjectSession) -> SelectionReport {
    let tc = s.toolpath_configs()[0].clone();
    SelectionReport::clone(&s.selection_status(&tc, SelectionTarget::Op).unwrap())
}

fn model(s: &ProjectSession) -> &LoadedModel {
    &s.models()[0]
}

/// A ring compared by its box, rounded to 1 µm: the nesting may reverse the
/// winding of a ring, never its box.
type RingBox = [i64; 4];

fn ring_box(points: &[P2]) -> RingBox {
    let r = |v: f64| (v * 1000.0).round() as i64;
    let (mut x0, mut y0) = (f64::MAX, f64::MAX);
    let (mut x1, mut y1) = (f64::MIN, f64::MIN);
    for p in points {
        x0 = x0.min(p.x);
        y0 = y0.min(p.y);
        x1 = x1.max(p.x);
        y1 = y1.max(p.y);
    }
    [r(x0), r(y0), r(x1), r(y1)]
}

/// The boxes of the drawing rings of the items of the model in `state`.
fn rings_in(s: &ProjectSession, r: &SelectionReport, state: ItemState) -> BTreeSet<RingBox> {
    model(s)
        .items
        .iter()
        .zip(&r.item_states)
        .filter(|(_, st)| **st == state)
        .filter_map(|(item, _)| item.shape().drawing_ring())
        .map(|ring| ring_box(&ring.exterior))
        .collect()
}

/// The resolved selection that generation hands to the prologue, through
/// the prologue's two calls: the view, then the hole fit step with the tool
/// diameter and the size tolerance of the op (P6.5; it changes a `Holes`
/// view only).
fn generation_items(s: &ProjectSession, item_use: ItemUse) -> rs_cam_core::selection::OpItems {
    let cancel = AtomicBool::new(false);
    let inputs = s.resolve_generation_inputs(0, &cancel).unwrap();
    let tool_diameter = s.tools()[0].diameter;
    // P6.4: the prologue grows the stock by the tool radius for an extend
    // reader (O-2 (B)).
    let grown = inputs
        .selection()
        .clone()
        .with_stock_grown(tool_diameter / 2.0);
    let resolved = match item_use {
        ItemUse::RegionsExtendOpen { .. } => &grown,
        ItemUse::None | ItemUse::Regions | ItemUse::Curves | ItemUse::Holes { .. } => {
            inputs.selection()
        }
    };
    let mut items = view(resolved, item_use).unwrap().0;
    let size = s.toolpath_configs()[0].operation.drill_size(tool_diameter);
    let _ = fit_holes(resolved, &mut items, item_use, size).unwrap();
    items
}

/// Claims 2 to 4 and 6 for every item of one report.
fn check_states(s: &ProjectSession, r: &SelectionReport, what: &str) {
    let m = model(s);
    assert_eq!(
        r.item_states.len(),
        m.items.len(),
        "{what}: one state per item of the toolpath model"
    );
    let selection = s.toolpath_configs()[0]
        .operation
        .selection()
        .unwrap()
        .clone();
    let own = SourceRef::Model(m.id);
    for (item, state) in m.items.iter().zip(&r.item_states) {
        let key = item.key();
        let held = r.selected.iter().any(|k| k.matches(key));
        match state {
            ItemState::Cut | ItemState::Island => {
                assert!(held, "{what}: a {state:?} item is not in the selection");
            }
            ItemState::Skipped => {
                assert!(held, "{what}: a skipped item is not in the selection");
                if r.refusal.is_none() {
                    assert!(
                        r.skipped_by_kind.get(&item.kind()).copied().unwrap_or(0) > 0,
                        "{what}: a skipped {:?} is not counted",
                        item.kind()
                    );
                }
                if r.refusal.is_none() && item.kind() == ItemKind::Path {
                    assert!(
                        r.notes.iter().any(|n| n.kind == ItemKind::Path
                            && n.layer.as_deref() == item.layer()),
                        "{what}: a skipped open path has no note"
                    );
                }
            }
            ItemState::Excluded => {
                assert!(!held, "{what}: an excluded item is in the selection");
                assert!(
                    selection
                        .excludes()
                        .iter()
                        .any(|k| k.names_same_geometry(key)),
                    "{what}: no exclude names the excluded item"
                );
            }
            ItemState::NotSelected => {
                assert!(!held, "{what}: an idle item is in the selection");
                assert!(
                    !selection
                        .excludes()
                        .iter()
                        .any(|k| k.names_same_geometry(key)),
                    "{what}: an exclude names an idle item"
                );
            }
        }
    }
    assert!(
        r.foreign.iter().all(|(k, _)| k.source != own),
        "{what}: `foreign` holds an item of the toolpath model"
    );
}

fn state_of(s: &ProjectSession, r: &SelectionReport, layer: &str, kind: ItemKind) -> ItemState {
    let at = model(s)
        .items
        .iter()
        .position(|i| i.layer() == Some(layer) && i.kind() == kind)
        .unwrap_or_else(|| panic!("no {kind:?} on layer {layer}"));
    r.item_states[at]
}

#[test]
fn a_pocket_cuts_its_cut_rings_round_its_islands() {
    let s = session(OperationType::Pocket);
    let r = report(&s);
    check_states(&s, &r, "Pocket default");
    assert_eq!(model(&s).items.len(), 8, "the fixture holds eight items");

    let items = generation_items(&s, ItemUse::Regions);
    let regions = items.regions().unwrap().as_slice();
    let exteriors: BTreeSet<RingBox> = regions.iter().map(|p| ring_box(&p.exterior)).collect();
    let holes: BTreeSet<RingBox> = regions
        .iter()
        .flat_map(|p| p.holes.iter().map(|h| ring_box(h)))
        .collect();
    assert_eq!(
        rings_in(&s, &r, ItemState::Cut),
        exteriors,
        "the Cut rings are the region exteriors that generation cuts"
    );
    assert_eq!(
        rings_in(&s, &r, ItemState::Island),
        holes,
        "the Island rings are the region holes that generation cuts round"
    );
    // The numbers, so an agreement on an empty answer cannot pass: the
    // outer square, the square in the island, KEEP and SPARE are cut; the
    // middle square and the circle are islands.
    assert_eq!(exteriors.len(), 4);
    assert_eq!(holes.len(), 2);
    assert_eq!(
        state_of(&s, &r, "HOLES", ItemKind::Circle),
        ItemState::Island
    );
    assert_eq!(state_of(&s, &r, "TEXT", ItemKind::Path), ItemState::Skipped);
    assert_eq!(
        state_of(&s, &r, "HOLES", ItemKind::Point),
        ItemState::Skipped
    );
    assert!(r.foreign.is_empty());
}

/// Claim 8 (P6.4): a Pocket with Extend to stock, right side, over the
/// layers `TEXT` (the open path) and `SPARE` (a square below the path and
/// its end ray, so inside the right area).
#[test]
fn an_extended_open_path_is_cut_and_a_loop_in_its_area_is_an_island_p6_4() {
    use rs_cam_core::compute::operation_configs::{OpenPaths, PocketConfig};
    use rs_cam_core::selection::PathSide;
    let rule = Rule {
        source: SourceRef::ToolpathModel,
        filter: Filter {
            layers: vec!["TEXT".into(), "SPARE".into()],
            ..Filter::default()
        },
    };
    let s = single_op_session(
        stock(),
        endmill_tool_config(TOOL_DIAMETER),
        load(),
        "pocket",
        OperationConfig::Pocket(PocketConfig {
            open_paths: OpenPaths::ExtendRight,
            selection: Selection::from_rules(vec![rule]),
            ..PocketConfig::default()
        }),
    );
    let r = report(&s);
    assert!(r.refusal.is_none(), "{:?}", r.refusal);
    check_states(&s, &r, "Pocket extend right");
    assert_eq!(state_of(&s, &r, "TEXT", ItemKind::Path), ItemState::Cut);
    assert_eq!(state_of(&s, &r, "SPARE", ItemKind::Loop), ItemState::Island);
    assert_eq!(
        state_of(&s, &r, "KEEP", ItemKind::Loop),
        ItemState::NotSelected
    );

    let item_use = ItemUse::RegionsExtendOpen {
        side: PathSide::Right,
    };
    assert_eq!(
        s.toolpath_configs()[0].operation.capabilities().item_use,
        item_use
    );
    let items = generation_items(&s, item_use);
    let regions = items.regions().unwrap();
    assert_eq!(regions.as_slice().len(), 1, "one extended area");
    let text = model(&s)
        .items
        .iter()
        .position(|i| i.layer() == Some("TEXT"))
        .unwrap();
    let area = &regions.as_slice()[0];
    let holes: BTreeSet<RingBox> = area.holes.iter().map(|h| ring_box(h)).collect();
    assert_eq!(
        rings_in(&s, &r, ItemState::Island),
        holes,
        "the Island ring is the hole of the extended area"
    );
    assert_eq!(r.extended.len(), 1);
    assert_eq!(
        ring_box(&r.extended[0]),
        ring_box(&area.exterior),
        "the report gives the extended area that generation cuts"
    );
    // The area reaches one tool radius past the stock edge (O-2 (B)):
    // X -1.5 on the west, Y -1.5 on the south.
    let [x0, y0, _, _] = ring_box(&area.exterior);
    assert_eq!([x0, y0], [-1500, -1500]);
    assert_eq!(r.item_states[text], ItemState::Cut);
}

#[test]
fn an_exclude_and_a_layer_rule_give_excluded_and_not_selected() {
    let mut s = session(OperationType::Pocket);
    let keep = s
        .list_model_items(model(&s).id, Some("KEEP"), Some(ItemKind::Loop))
        .unwrap();
    let tc = s.toolpath_configs()[0].clone();
    let removal = s
        .pick_edit(&tc, SelectionTarget::Op, keep[0].key.clone())
        .unwrap();
    assert!(
        matches!(removal, SelectionEdit::RemovePick(_)),
        "a click on a rule match removes it"
    );
    edit(&mut s, removal);
    let r = report(&s);
    check_states(&s, &r, "Pocket with an exclude");
    assert_eq!(
        state_of(&s, &r, "KEEP", ItemKind::Loop),
        ItemState::Excluded
    );
    let regions = generation_items(&s, ItemUse::Regions);
    let exteriors: BTreeSet<RingBox> = regions
        .regions()
        .unwrap()
        .as_slice()
        .iter()
        .map(|p| ring_box(&p.exterior))
        .collect();
    assert_eq!(rings_in(&s, &r, ItemState::Cut), exteriors);

    // A rule on OUTER only: the other layers are not selected.
    edit(
        &mut s,
        SelectionEdit::Replace(Box::new(Selection::from_rules(vec![Rule {
            source: SourceRef::ToolpathModel,
            filter: Filter {
                layers: vec!["OUTER".into()],
                ..Filter::default()
            },
        }]))),
    );
    let r = report(&s);
    check_states(&s, &r, "Pocket on OUTER");
    for (layer, kind) in [
        ("SPARE", ItemKind::Loop),
        ("KEEP", ItemKind::Loop),
        ("TEXT", ItemKind::Path),
        ("HOLES", ItemKind::Circle),
        ("HOLES", ItemKind::Point),
    ] {
        assert_eq!(
            state_of(&s, &r, layer, kind),
            ItemState::NotSelected,
            "{layer} {kind:?}"
        );
    }
}

#[test]
fn a_trace_cuts_every_ring_and_a_drill_every_hole() {
    let s = session(OperationType::Trace);
    let r = report(&s);
    check_states(&s, &r, "Trace default");
    let items = generation_items(&s, ItemUse::Curves);
    let rings: BTreeSet<RingBox> = items
        .curves()
        .unwrap()
        .as_slice()
        .iter()
        .flat_map(|p| {
            std::iter::once(ring_box(&p.exterior)).chain(p.holes.iter().map(|h| ring_box(h)))
        })
        .collect();
    assert_eq!(rings_in(&s, &r, ItemState::Cut), rings);
    assert_eq!(rings.len(), 7, "six closed rings and the open path");
    assert!(rings_in(&s, &r, ItemState::Island).is_empty());
    assert_eq!(
        state_of(&s, &r, "HOLES", ItemKind::Point),
        ItemState::Skipped
    );

    // The tool is the circle size, so the Drill drills the circle (P6.5).
    let s = session_with_tool(OperationType::Drill, CIRCLE_DIAMETER);
    let r = report(&s);
    check_states(&s, &r, "Drill default");
    let item_use = s.toolpath_configs()[0].operation.capabilities().item_use;
    let items = generation_items(&s, item_use);
    let drilled: BTreeSet<[i64; 2]> = items
        .holes()
        .unwrap()
        .as_slice()
        .iter()
        .map(|h| {
            [
                (h.xy.x * 1000.0).round() as i64,
                (h.xy.y * 1000.0).round() as i64,
            ]
        })
        .collect();
    let cut: BTreeSet<[i64; 2]> = model(&s)
        .items
        .iter()
        .zip(&r.item_states)
        .filter(|(_, st)| **st == ItemState::Cut)
        .filter_map(|(item, _)| match item.shape() {
            Shape::Point(p) | Shape::Circle { centre: p, .. } => {
                Some([(p.x * 1000.0).round() as i64, (p.y * 1000.0).round() as i64])
            }
            Shape::Loop(_) | Shape::Path(_) | Shape::Face(_) => None,
        })
        .collect();
    assert_eq!(
        cut, drilled,
        "the Cut holes are the holes generation drills"
    );
    assert_eq!(cut.len(), 2);
    assert_eq!(state_of(&s, &r, "HOLES", ItemKind::Circle), ItemState::Cut);
    assert_eq!(
        state_of(&s, &r, "OUTER", ItemKind::Loop),
        ItemState::Skipped
    );
}

/// Geometry selection P6.5 (ruling O-3): with a 3 mm tool the 4 mm circle
/// is not of the tool size. The Drill leaves it out under the default rule:
/// its state is `Skipped`, one `HoleFit` note names it, and generation
/// drills the point only.
#[test]
fn a_drill_skips_a_circle_that_is_not_the_tool_size_p6_5() {
    let s = session(OperationType::Drill);
    let r = report(&s);
    check_states(&s, &r, "Drill with a 3 mm tool");
    assert!(r.refusal.is_none(), "{:?}", r.refusal);
    assert_eq!(
        state_of(&s, &r, "HOLES", ItemKind::Circle),
        ItemState::Skipped
    );
    assert_eq!(state_of(&s, &r, "HOLES", ItemKind::Point), ItemState::Cut);
    let fits: Vec<_> = r
        .notes
        .iter()
        .filter(|n| matches!(n.cause, NoteCause::HoleFit { .. }))
        .collect();
    assert_eq!(fits.len(), 1, "{:?}", r.notes);
    assert_eq!(
        fits[0].cause,
        NoteCause::HoleFit {
            fit: HoleFit::Larger,
            size: rs_cam_core::selection::hole_fit::DrillSize::strict(TOOL_DIAMETER),
        }
    );
    assert_eq!(fits[0].first.key.kind, ItemKind::Circle);
    let item_use = s.toolpath_configs()[0].operation.capabilities().item_use;
    let holes = generation_items(&s, item_use);
    let holes = holes.holes().unwrap().as_slice();
    assert_eq!(holes.len(), 1, "{holes:?}");
    assert_eq!(holes[0].diameter, None, "the point is the one hole");
    assert_eq!(r.holes, holes, "the report holes are the drilled holes");
}

#[test]
fn a_refused_selection_cuts_nothing_and_a_stock_item_is_foreign() {
    let mut s = session(OperationType::Pocket);
    let outer = s
        .list_model_items(model(&s).id, Some("OUTER"), Some(ItemKind::Loop))
        .unwrap();
    let mut gone: MatchKey = outer[0].key.clone();
    gone.anchor = [500.0, 500.0];
    edit(
        &mut s,
        SelectionEdit::Replace(Box::new(Selection::from_picks(vec![
            outer[0].key.clone(),
            gone,
        ]))),
    );
    let r = report(&s);
    assert!(r.refusal.is_some(), "a lost pick refuses");
    check_states(&s, &r, "Pocket with a lost pick");
    assert!(
        !r.item_states
            .iter()
            .any(|st| matches!(st, ItemState::Cut | ItemState::Island)),
        "a refused selection marks an item cut: {:?}",
        r.item_states
    );
    assert_eq!(r.item_states[outer_index(&s)], ItemState::Skipped);

    let s = session(OperationType::Face);
    let r = report(&s);
    check_states(&s, &r, "Face default");
    assert!(
        r.item_states.iter().all(|st| *st == ItemState::NotSelected),
        "the stock default selects no drawing item"
    );
    assert_eq!(r.foreign.len(), 1, "{:?}", r.foreign);
    assert_eq!(r.foreign[0].0.source, SourceRef::Stock);
    assert_eq!(r.foreign[0].1, ItemState::Cut);
}

fn outer_index(s: &ProjectSession) -> usize {
    model(s)
        .items
        .iter()
        .position(|i| i.layer() == Some("OUTER"))
        .unwrap()
}

#[test]
fn a_reimport_builds_the_same_states_again() {
    let mut s = session(OperationType::Pocket);
    let before = report(&s);
    let builds = s.selection_status_builds();
    let model_id = model(&s).id;
    let _ = s
        .apply(Command::AdoptModelGeometry(AdoptModelGeometryArgs {
            model_id,
            geometry: Box::new(load()),
            units: None,
        }))
        .unwrap();
    let after = report(&s);
    assert_eq!(
        s.selection_status_builds(),
        builds + 1,
        "a re-import builds the report again"
    );
    assert_eq!(after.item_states, before.item_states);
    check_states(&s, &after, "Pocket after a re-import");
}
