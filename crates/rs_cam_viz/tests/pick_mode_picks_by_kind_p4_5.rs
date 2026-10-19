//! Geometry selection P4.5 — pick mode, the filter bar, the hover and the
//! shift-click layer rule.
//!
//! # The claims
//!
//! 1. `a_click_picks_the_drawn_loop_p4_5`: in pick mode a click on the
//!    drawn line of a loop, an open path or a circle picks that item, on a
//!    flipped and on a turned setup. The test computes the drawn point from
//!    the setup transform itself, and the raw model point is more than the
//!    pick threshold away, so a pick in the model frame fails. The
//!    `TogglePick` of the hit takes the item out of the selection.
//! 2. `a_chip_off_skips_its_kind_p4_5`: a click on a circle ring that is
//!    also near a loop edge picks the circle with every chip on, and the
//!    loop with the circle chip off or with a circle range that leaves the
//!    circle out. With both chips off it picks nothing.
//! 3. `a_served_kind_filter_blocks_a_refusing_pick_p4_5`: the Pocket bar has
//!    no enabled `Open` or `Points` chip, and each disabled chip names the
//!    core reason (`kind_not_cut_reason`). A shift-click on a layer that
//!    holds loops and paths adds ONE rule `Only([Loop, Circle])`; under
//!    the default it replaces the default; the selection then resolves
//!    with no refusal. A circle range gives the circles their own rule.
//! 4. `the_hover_uploads_once_per_hover_change_p4_5`: along a pointer path
//!    the hover detector asks for one upload per change of the hovered
//!    item, and none while the item stays the same.
//!
//! ```text
//! cargo test -p rs_cam_viz -q --test pick_mode_picks_by_kind_p4_5
//! ```

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;

use rs_cam_core::compute::catalog::{ItemUse, OperationConfig};
use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits, StockConfig};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::compute::transform::{FaceUp, ZRotation};
use rs_cam_core::geo::{P2, P3};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::selection::{
    DiameterRange, Filter, Item, ItemKind, ItemSet, KindFilter, MatchKey, Rule, Selection, Shape,
    SourceRef, kind_not_cut_reason,
};
use rs_cam_core::session::{
    Command, ItemState, LoadedModel, ProjectSessionBuilder, SelectionTarget, SetSetupFaceArgs,
    SetSetupRotationArgs, ToolpathConfig,
};
use rs_cam_viz::compute::{
    CollisionRequest, ComputeBackend, ComputeLane, ComputeMessage, ComputeRequest,
    GenerationControl, LaneSnapshot, SimulationRequest, ToolpathSubmitOutcome,
};
use rs_cam_viz::controller::AppController;
use rs_cam_viz::interaction::picking::{PickContext, PickHit, drawing_plane_z, pick_items};
use rs_cam_viz::render::camera::OrbitCamera;
use rs_cam_viz::state::Workspace;
use rs_cam_viz::state::job::SetupFrame;
use rs_cam_viz::state::pick::{PickFilter, PickMode, held_kinds};
use rs_cam_viz::state::selection::Selection as UiSelection;
use rs_cam_viz::state::selection_highlight::HoverDetector;
use rs_cam_viz::ui::AppEvent;
use rs_cam_viz::ui::pick_bar::kind_chips;

/// The pick threshold of a line (`picking::PICK_THRESHOLD_TOOLPATH`).
const PICK_THRESHOLD_PX: f32 = 15.0;
const VIEW_W: f32 = 800.0;
const VIEW_H: f32 = 600.0;

/// A compute backend that accepts every submission and returns nothing.
struct SilentBackend;

impl ComputeBackend for SilentBackend {
    fn submit_toolpath(&mut self, _request: ComputeRequest) -> ToolpathSubmitOutcome {
        ToolpathSubmitOutcome::Queued
    }
    fn submit_simulation(&mut self, _request: SimulationRequest) {}
    fn submit_collision(&mut self, _request: CollisionRequest) {}
    fn cancel_lane(&mut self, _lane: ComputeLane) {}
    fn drain_results(&mut self) -> Vec<ComputeMessage> {
        Vec::new()
    }
    fn lane_snapshot(&self, lane: ComputeLane) -> LaneSnapshot {
        LaneSnapshot::idle(lane)
    }
    fn generation_control(&self) -> GenerationControl {
        GenerationControl::detached()
    }
}

fn toolpath(operation: OperationConfig) -> ToolpathConfig {
    ToolpathConfig {
        shoe: Default::default(),
        id: rs_cam_core::ToolpathId(0),
        name: "Op".to_owned(),
        enabled: true,
        operation,
        dressups: Default::default(),
        heights: Default::default(),
        tool_id: 1,
        model_id: 0,
        pre_gcode: None,
        post_gcode: None,
        boundary: Default::default(),
        boundary_inherit: true,
        stock_source: Default::default(),
        coolant: Default::default(),
        debug_options: Default::default(),
        feeds_provenance: Default::default(),
        rest_analysis: Default::default(),
        planner_origin: None,
    }
}

fn ring(points: &[(f64, f64)]) -> Polygon2 {
    Polygon2::new(points.iter().map(|&(x, y)| P2::new(x, y)).collect())
}

fn item(layer: &str, shape: Shape) -> Item {
    Item::new(SourceRef::ToolpathModel, Some(layer.into()), shape).unwrap()
}

/// The drawing, off every axis of symmetry of the stock:
///
/// - 0: a loop on CUT, (10, 10)–(40, 30);
/// - 1: a circle on HOLES, centre (14, 20), diameter 6: its ring passes
///   1 mm from the loop edge x = 10;
/// - 2: an open path on TEXT;
/// - 3: a loop on TEXT, (55, 45)–(75, 60);
/// - 4: a point on HOLES.
fn drawing() -> ItemSet {
    let mut path = ring(&[(50.0, 10.0), (80.0, 20.0), (85.0, 35.0)]);
    path.closed = false;
    ItemSet::new(vec![
        item(
            "CUT",
            Shape::Loop(ring(&[
                (10.0, 10.0),
                (40.0, 10.0),
                (40.0, 30.0),
                (10.0, 30.0),
            ])),
        ),
        item("HOLES", Shape::circle(P2::new(14.0, 20.0), 6.0)),
        item("TEXT", Shape::Path(path)),
        item(
            "TEXT",
            Shape::Loop(ring(&[
                (55.0, 45.0),
                (75.0, 45.0),
                (75.0, 60.0),
                (55.0, 60.0),
            ])),
        ),
        item("HOLES", Shape::Point(P2::new(20.0, 65.0))),
    ])
}

fn controller(
    operation: OperationConfig,
    face_up: FaceUp,
    turn: ZRotation,
) -> AppController<SilentBackend> {
    let stock = StockConfig {
        x: 100.0,
        y: 80.0,
        z: 20.0,
        origin_x: -10.0,
        origin_y: -5.0,
        origin_z: -20.0,
        auto_from_model: false,
        ..StockConfig::default()
    };
    let mut builder = ProjectSessionBuilder::new()
        .stock(stock)
        .tool(ToolConfig::new_default(ToolId(1), ToolType::EndMill));
    let model_id = builder.add_model(LoadedModel {
        id: 0,
        name: "drawing.dxf".to_owned(),
        mesh: None,
        items: Arc::new(drawing()),
        path: PathBuf::from("drawing.dxf"),
        kind: Some(ModelKind::Dxf),
        units: Some(ModelUnits::Millimeters),
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
    });
    let mut tc = toolpath(operation);
    tc.model_id = model_id;
    let _ = builder.add_toolpath(0, tc).unwrap();
    let mut c = AppController::with_backend(SilentBackend);
    c.state_mut().session = builder.build();
    let session = &mut c.state_mut().session;
    let _ = session
        .apply(Command::SetSetupFace(SetSetupFaceArgs {
            setup_index: 0,
            face_up,
        }))
        .unwrap();
    let _ = session
        .apply(Command::SetSetupRotation(SetSetupRotationArgs {
            setup_index: 0,
            z_rotation: turn,
        }))
        .unwrap();
    let id = session.toolpath_configs()[0].id;
    let state = c.state_mut();
    state.workspace = Workspace::Toolpaths;
    state.selection = UiSelection::Toolpath(id);
    c
}

/// Turn on the pick mode of the op selection, with every served kind on.
fn pick_on(c: &mut AppController<SilentBackend>) -> PickMode {
    let tc = c.state().session.toolpath_configs()[0].clone();
    let mode = PickMode::new(
        tc.id,
        SelectionTarget::Op,
        tc.operation.capabilities().item_use,
    );
    c.state_mut().gui.selection_ui.set_pick(Some(mode.clone()));
    mode
}

fn held(c: &AppController<SilentBackend>) -> BTreeSet<ItemKind> {
    held_kinds(c.state().session.models()[0].items.iter())
}

/// A camera that sees the whole displayed stock from above.
fn camera(c: &AppController<SilentBackend>) -> OrbitCamera {
    let stock = c.state().session.stock_config().clone();
    let frame = SetupFrame::of(&c.state().session.list_setups()[0]);
    let (w, d, h) = frame.effective_stock(&stock);
    let mut camera = OrbitCamera::new();
    camera.fit_to_bounds([0.0, 0.0, 0.0], [w as f32, d as f32, h as f32]);
    camera
}

/// Where the viewport draws the model point `p` on the screen. The XY is
/// the setup transform, computed here and not by the helper the code
/// shares; the Z is the drawing plane of the op.
fn drawn(c: &AppController<SilentBackend>, camera: &OrbitCamera, p: P2) -> [f32; 2] {
    let state = c.state();
    let stock = state.session.stock_config();
    let frame = SetupFrame::of(&state.session.list_setups()[0]);
    let tc = &state.session.toolpath_configs()[0];
    let z = drawing_plane_z(&state.session, tc, Some(&frame));
    let q = frame.transform_point(P3::new(p.x, p.y, 0.0), stock);
    screen(camera, P3::new(q.x, q.y, z))
}

/// Where the raw model point `p` falls on the screen (the old pick frame).
fn raw(c: &AppController<SilentBackend>, camera: &OrbitCamera, p: P2) -> [f32; 2] {
    let stock = c.state().session.stock_config();
    screen(camera, P3::new(p.x, p.y, stock.origin_z + stock.z))
}

fn screen(camera: &OrbitCamera, p: P3) -> [f32; 2] {
    camera
        .project_to_screen(
            [p.x as f32, p.y as f32, p.z as f32],
            VIEW_W / VIEW_H,
            VIEW_W,
            VIEW_H,
        )
        .expect("the point is in view")
}

/// The pick of the pointer at `at` in pick mode, as the viewport runs it.
fn pick_at(
    c: &AppController<SilentBackend>,
    camera: &OrbitCamera,
    filter: &PickFilter,
    at: [f32; 2],
) -> Option<MatchKey> {
    let state = c.state();
    let tc = &state.session.toolpath_configs()[0];
    let item_use = tc.operation.capabilities().item_use;
    let kinds = filter.effective_kinds(item_use, &held(c));
    let ctx = PickContext {
        camera,
        screen_x: at[0],
        screen_y: at[1],
        aspect: VIEW_W / VIEW_H,
        vw: VIEW_W,
        vh: VIEW_H,
    };
    match pick_items(&ctx, &state.session, tc.id, filter, &kinds)? {
        PickHit::Item { key, .. } => Some(key),
        other => panic!("pick mode gave a hit that is not an item: {other:?}"),
    }
}

/// The state of item `index` in the op selection report.
fn state_of(c: &AppController<SilentBackend>, index: usize) -> ItemState {
    let tc = &c.state().session.toolpath_configs()[0];
    c.state()
        .session
        .selection_status(tc, SelectionTarget::Op)
        .unwrap()
        .item_states[index]
}

fn mid(a: P2, b: P2) -> P2 {
    P2::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0)
}

fn a_click_picks_each_drawn_kind(face_up: FaceUp, turn: ZRotation) {
    let mut c = controller(OperationConfig::Trace(Default::default()), face_up, turn);
    let mode = pick_on(&mut c);
    let camera = camera(&c);
    let items: Vec<Item> = c.state().session.models()[0]
        .items
        .iter()
        .cloned()
        .collect();
    // The loop, the circle and the path: a point on the first edge of each
    // drawn line.
    for index in [0, 1, 2] {
        let item = &items[index];
        let ring = match item.shape() {
            Shape::Loop(r) | Shape::Path(r) | Shape::Circle { ring: r, .. } => r,
            Shape::Point(_) | Shape::Face(_) => unreachable!("fixture"),
        };
        let p = mid(ring.exterior[0], ring.exterior[1]);
        let at = drawn(&c, &camera, p);
        let old = raw(&c, &camera, p);
        assert!(
            (old[0] - at[0]).hypot(old[1] - at[1]) > 3.0 * PICK_THRESHOLD_PX,
            "the test does not separate the two frames for {:?}",
            item.kind()
        );
        let key = pick_at(&c, &camera, &mode.filter, at).unwrap_or_else(|| {
            panic!(
                "{face_up:?}/{turn:?}: a click on the drawn {:?} missed",
                item.kind()
            )
        });
        assert!(
            key.names_same_geometry(item.key()),
            "{face_up:?}/{turn:?}: the click on the {:?} picked {key:?}",
            item.kind()
        );
        // The default rule holds the item; the click takes it out.
        assert_eq!(
            state_of(&c, index),
            ItemState::Cut,
            "fixture: the default cuts it"
        );
        c.handle_internal_event(AppEvent::TogglePick {
            toolpath_id: mode.toolpath_id,
            target: mode.target,
            key,
        });
        assert_eq!(
            state_of(&c, index),
            ItemState::Excluded,
            "{face_up:?}/{turn:?}: the click did not take the {:?} out",
            item.kind()
        );
    }
}

#[test]
fn a_click_picks_the_drawn_loop_p4_5() {
    a_click_picks_each_drawn_kind(FaceUp::Bottom, ZRotation::Deg0);
    a_click_picks_each_drawn_kind(FaceUp::Top, ZRotation::Deg90);
}

#[test]
fn a_chip_off_skips_its_kind_p4_5() {
    let mut c = controller(
        OperationConfig::Pocket(Default::default()),
        FaceUp::Bottom,
        ZRotation::Deg0,
    );
    let mode = pick_on(&mut c);
    let camera = camera(&c);
    let items: Vec<Item> = c.state().session.models()[0]
        .items
        .iter()
        .cloned()
        .collect();
    // On the circle ring, 1 mm from the loop edge x = 10.
    let at = drawn(&c, &camera, P2::new(11.0, 20.0));
    let picked = |filter: &PickFilter| pick_at(&c, &camera, filter, at);

    let all = mode.filter.clone();
    assert!(
        picked(&all).is_some_and(|k| k.names_same_geometry(items[1].key())),
        "every chip on: the circle under the pointer wins"
    );
    let mut no_circles = all.clone();
    no_circles.kinds.remove(&ItemKind::Circle);
    assert!(
        picked(&no_circles).is_some_and(|k| k.names_same_geometry(items[0].key())),
        "circle chip off: the loop, the next item under the pointer, wins"
    );
    let mut other_range = all;
    other_range.circle = Some(DiameterRange {
        nominal_mm: 5.0,
        tolerance_mm: 0.1,
    });
    assert!(
        picked(&other_range).is_some_and(|k| k.names_same_geometry(items[0].key())),
        "a 5 mm range leaves the 6 mm circle out: the loop wins"
    );
    let mut nothing = no_circles.clone();
    nothing.kinds.remove(&ItemKind::Loop);
    assert_eq!(picked(&nothing), None, "loops and circles off: no pick");

    // The controller arm: the pick of the filtered click lands on the loop.
    let key = picked(&no_circles).unwrap();
    c.handle_internal_event(AppEvent::TogglePick {
        toolpath_id: mode.toolpath_id,
        target: mode.target,
        key,
    });
    assert_eq!(
        state_of(&c, 0),
        ItemState::Excluded,
        "the loop left the cut"
    );
    // The circle stays selected; with its outer loop gone it is no island.
    assert_eq!(
        state_of(&c, 1),
        ItemState::Cut,
        "the circle stays in the cut"
    );
}

#[test]
fn a_served_kind_filter_blocks_a_refusing_pick_p4_5() {
    let mut c = controller(
        OperationConfig::Pocket(Default::default()),
        FaceUp::Top,
        ZRotation::Deg0,
    );
    let mode = pick_on(&mut c);
    let held = held(&c);

    // The bar: no enabled Open or Points chip; each names the core reason.
    let chips = kind_chips(&mode.filter, ItemUse::Regions, &held);
    let chip = |kind: ItemKind| chips.iter().find(|ch| ch.kind == kind).unwrap();
    for kind in [ItemKind::Path, ItemKind::Point] {
        assert!(!chip(kind).on, "{kind:?} chip must be off");
        assert_eq!(
            chip(kind).disabled_reason.as_deref(),
            Some(kind_not_cut_reason(kind, ItemUse::Regions).as_str()),
            "the disabled {kind:?} chip names the core reason"
        );
    }
    assert!(
        chip(ItemKind::Face).disabled_reason.is_some(),
        "the drawing holds no face"
    );
    for kind in [ItemKind::Loop, ItemKind::Circle] {
        assert!(chip(kind).on && chip(kind).disabled_reason.is_none());
    }
    let kinds = mode.filter.effective_kinds(ItemUse::Regions, &held);
    assert_eq!(kinds, vec![ItemKind::Loop, ItemKind::Circle]);

    // Shift-click on the TEXT loop: the layer holds a loop and a path.
    let items: Vec<Item> = c.state().session.models()[0]
        .items
        .iter()
        .cloned()
        .collect();
    let text_loop = items[3].key().clone();
    c.handle_internal_event(AppEvent::PickLayer {
        toolpath_id: mode.toolpath_id,
        target: mode.target,
        key: text_loop,
        kinds: kinds.clone(),
        circle: None,
    });
    let rule = |layer: &str, kinds: Vec<ItemKind>, diameter| Rule {
        source: SourceRef::ToolpathModel,
        filter: Filter {
            layers: vec![layer.to_owned()],
            kinds: KindFilter::Only(kinds),
            diameter,
            face: None,
        },
    };
    let stored = |c: &AppController<SilentBackend>| {
        c.state().session.toolpath_configs()[0]
            .operation
            .selection()
            .unwrap()
            .clone()
    };
    assert_eq!(
        stored(&c),
        Selection::from_rules(vec![rule(
            "TEXT",
            vec![ItemKind::Loop, ItemKind::Circle],
            None
        )]),
        "under the default, the first layer rule replaces the default"
    );
    let tc = c.state().session.toolpath_configs()[0].clone();
    let report = c
        .state()
        .session
        .selection_status(&tc, SelectionTarget::Op)
        .unwrap();
    assert!(report.refusal.is_none(), "{:?}", report.refusal);
    assert_eq!(
        report.item_states[3],
        ItemState::Cut,
        "the TEXT loop is cut"
    );
    assert_eq!(
        report.item_states[2],
        ItemState::NotSelected,
        "the path is not named"
    );

    // A second shift-click, with a circle range: the circles get their own
    // rule, in one edit after the first rule.
    let range = DiameterRange {
        nominal_mm: 6.0,
        tolerance_mm: 0.1,
    };
    c.handle_internal_event(AppEvent::PickLayer {
        toolpath_id: mode.toolpath_id,
        target: mode.target,
        key: items[1].key().clone(),
        kinds,
        circle: Some(range),
    });
    assert_eq!(
        stored(&c).rules(),
        &[
            rule("TEXT", vec![ItemKind::Loop, ItemKind::Circle], None),
            rule("HOLES", vec![ItemKind::Loop], None),
            rule("HOLES", vec![ItemKind::Circle], Some(range)),
        ],
        "after the default, the rules append"
    );
}

#[test]
fn the_hover_uploads_once_per_hover_change_p4_5() {
    let mut c = controller(
        OperationConfig::Pocket(Default::default()),
        FaceUp::Top,
        ZRotation::Deg0,
    );
    let mode = pick_on(&mut c);
    let camera = camera(&c);
    // A pointer path: off every item, along the loop edge y = 10 (three
    // frames), onto the TEXT loop, then off again (two frames).
    let path = [
        P2::new(90.0, 75.0),
        P2::new(25.0, 10.0),
        P2::new(26.0, 10.0),
        P2::new(27.0, 10.0),
        P2::new(65.0, 45.0),
        P2::new(90.0, 75.0),
        P2::new(91.0, 75.0),
    ];
    let mut detector = HoverDetector::default();
    let mut uploads = 0;
    let mut hovered = Vec::new();
    for p in path {
        let at = drawn(&c, &camera, p);
        let key = pick_at(&c, &camera, &mode.filter, at);
        hovered.push(key.clone());
        if detector.changed(key) {
            uploads += 1;
        }
    }
    assert!(
        hovered[0].is_none() && hovered[5].is_none(),
        "fixture: off every item"
    );
    assert!(
        hovered[1].is_some() && hovered[1] == hovered[3],
        "fixture: one loop"
    );
    assert!(
        hovered[4].is_some() && hovered[4] != hovered[1],
        "fixture: another loop"
    );
    assert_eq!(
        uploads, 3,
        "one upload per change (loop, TEXT loop, nothing), none while the item stays"
    );
    assert_eq!(detector.hovered(), None);
}
