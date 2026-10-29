//! Geometry selection P4.5 — pick mode, the filter bar, the hover and the
//! right-click layer rule; P3 and P4 of the reference audit (2026-10-23):
//! a shift-click adds one item, and a click on an item whose chip is off
//! turns the chip on and picks the item.
//!
//! # The claims
//!
//! 1. `a_click_picks_the_drawn_loop_p4_5`: in pick mode a click on the
//!    drawn line of a loop, an open path or a circle picks that item, on a
//!    flipped and on a turned setup. The test computes the drawn point from
//!    the setup transform itself, and the raw model point is more than the
//!    pick threshold away, so a pick in the model frame fails. The
//!    `TogglePick` of the hit on the default rule makes that item the one
//!    item that the op cuts (operator ruling O-PICK, 2026-10-22).
//! 2. `a_chip_off_skips_its_kind_p4_5`: a click on a circle ring that is
//!    also near a loop edge picks the circle with every chip on, and the
//!    loop with the circle chip off or with a circle range that leaves the
//!    circle out. With both chips off `pick_items` picks nothing; the
//!    click then tries the kinds whose chip is off (`pick_mode_item`,
//!    claim 6).
//! 3. `a_served_kind_filter_blocks_a_refusing_pick_p4_5`: the Pocket bar has
//!    no enabled `Open contours` or `Points` chip, and each disabled chip names the
//!    core reason (`kind_not_cut_reason`). A right-click on a layer that
//!    holds loops and paths adds ONE rule `Only([Loop, Circle])`; under
//!    the default it replaces the default; the selection then resolves
//!    with no refusal. A circle range gives the circles their own rule.
//! 4. `the_hover_uploads_once_per_hover_change_p4_5`: along a pointer path
//!    the hover detector asks for one upload per change of the hovered
//!    item, and none while the item stays the same.
//! 5. `a_hidden_model_takes_no_pick_p4_d`: a model that the eye of its
//!    row hides (`SetModelVisibility`) draws no drawing, so a pick-mode
//!    click on its loop and a popover click on its open path pick nothing.
//!    When the eye shows the model again, both clicks pick again.
//! 6. `a_click_on_an_item_whose_chip_is_off_picks_it_p4click`: with the
//!    loop chip off, a click on a loop (and nothing of an on chip near it)
//!    picks the loop, turns the loop chip on and adds the loop. An item of
//!    an on chip under the pointer still wins. An open path, which a Pocket
//!    cannot cut, never picks.
//! 7. `a_shift_click_adds_one_item_p3shift`: a shift-click sends
//!    `AddPick`, never `PickLayer`. Two shift-clicks give two picks and no
//!    rule; a shift-click on a picked item keeps it. A right-click sends
//!    `PickLayer`.
//! 8. `in_pick_mode_only_a_pickable_face_lights_live1` (live look,
//!    2026-10-29): `hover_target` is the one hover rule. In pick mode a
//!    pickable face lights (its key is `Ok`), a pick item rings, and a
//!    face that is not pickable (a side face) gets no hover. Outside pick
//!    mode any face under the pointer lights, as before. The pick-mode
//!    tooltip (`hover_text`) is one short line.
//! 9. E-Q6 "select other" (GUI only): `step_cycle_*` hold the pure cycle
//!    (a Tab at a new point starts on the second candidate, the same point
//!    steps and wraps, a move or no pointer ends it, a mode change clears
//!    it); `a_tab_lists_every_candidate_under_the_pointer_e_q6` lists the
//!    circle and the loop at the ring point nearest first, and a click on
//!    the cycled loop picks the loop through the ordinary `TogglePick`;
//!    `tab_reaches_the_gesture_translation_e_q6` holds the key path.
//!
//! ```text
//! cargo test -p rs_cam_viz -q --test pick_mode_picks_by_kind_p4_5
//! ```

#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
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
use rs_cam_viz::interaction::picking::{
    PickContext, PickHit, pick_candidates, pick_items, pick_mode_item, pick_open_path,
};
use rs_cam_viz::render::camera::OrbitCamera;
use rs_cam_viz::state::Workspace;
use rs_cam_viz::state::display_frame::drawing_plane_z;
use rs_cam_viz::state::job::{ModelId, SetupFrame};
use rs_cam_viz::state::pick::{
    CYCLE_STAY_PX, PickCycle, PickFilter, PickMode, held_kinds, keep_cycle, step_cycle,
};
use rs_cam_viz::state::selection::Selection as UiSelection;
use rs_cam_viz::state::selection_highlight::{HoverDetector, HoverTarget, hover_target};
use rs_cam_viz::ui::AppEvent;
use rs_cam_viz::ui::pick_bar::{PickClick, click_event, kind_chips};
use rs_cam_viz::ui_command::{SetModelVisibilityArgs, UiCommand};

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
        id: rs_cam_core::ToolpathId::from_raw(0),
        name: "Op".to_owned(),
        enabled: true,
        operation,
        dressups: Default::default(),
        heights: Default::default(),
        tool_id: Some(rs_cam_core::ids::ToolId::from_raw(1)),
        model_id: rs_cam_core::ids::ModelId::from_raw(0),
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
        z: Some(20.0),
        origin_x: -10.0,
        origin_y: -5.0,
        origin_z: -20.0,
        auto_from_model: false,
        ..StockConfig::default()
    };
    let mut builder = ProjectSessionBuilder::new()
        .stock(stock)
        .tool(ToolConfig::new_default(
            ToolId::from_raw(1),
            ToolType::EndMill,
        ));
    let model_id = builder.add_model(LoadedModel {
        id: rs_cam_core::ids::ModelId::from_raw(0),
        name: "drawing.dxf".to_owned(),
        mesh: None,
        items: Arc::new(drawing()),
        path: PathBuf::from("drawing.dxf"),
        kind: Some(ModelKind::Dxf),
        units: Some(ModelUnits::Millimeters),
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
        import_options: Default::default(),
        import_report: None,
        drawing: Default::default(),
    });
    let mut tc = toolpath(operation);
    tc.model_id = model_id;
    let _ = builder.add_toolpath(0, tc).unwrap();
    let mut c = AppController::with_backend(SilentBackend);
    c.state_mut().replace_session(builder.build());
    let state = &mut c.state_mut();
    let _ = state
        .apply(Command::SetSetupFace(SetSetupFaceArgs {
            setup_index: 0,
            face_up,
        }))
        .unwrap();
    let _ = state
        .apply(Command::SetSetupRotation(SetSetupRotationArgs {
            setup_index: 0,
            z_rotation: turn,
        }))
        .unwrap();
    let id = state.session().toolpath_configs()[0].id;
    let state = c.state_mut();
    state.workspace = Workspace::Toolpaths;
    let _ = state.set_focus(UiSelection::Toolpath(id));
    c
}

/// Turn on the pick mode of the op selection, with every served kind on.
fn pick_on(c: &mut AppController<SilentBackend>) -> PickMode {
    let tc = c.state().session().toolpath_configs()[0].clone();
    let mode = PickMode::new(
        tc.id,
        SelectionTarget::Op,
        tc.operation.capabilities().item_use,
    );
    c.state_mut().gui.selection_ui.set_pick(Some(mode.clone()));
    mode
}

fn held(c: &AppController<SilentBackend>) -> BTreeSet<ItemKind> {
    held_kinds(&c.state().session().models()[0])
}

/// A camera that sees the whole displayed stock from above.
fn camera(c: &AppController<SilentBackend>) -> OrbitCamera {
    let stock = c.state().session().stock_config().clone();
    let frame = SetupFrame::of(&c.state().session().list_setups()[0]);
    let (w, d, h) = frame.effective_stock(&stock);
    let mut camera = OrbitCamera::new();
    camera.set_pose(camera.fit_pose([0.0, 0.0, 0.0], [w as f32, d as f32, h as f32]));
    camera
}

/// Where the viewport draws the model point `p` on the screen. The XY is
/// the setup transform, computed here and not by the helper the code
/// shares; the Z is the drawing plane of the op.
fn drawn(c: &AppController<SilentBackend>, camera: &OrbitCamera, p: P2) -> [f32; 2] {
    let state = c.state();
    let stock = state.session().stock_config();
    let frame = SetupFrame::of(&state.session().list_setups()[0]);
    let tc = &state.session().toolpath_configs()[0];
    let z = drawing_plane_z(state.session(), tc, Some(&frame));
    let q = frame.transform_point(P3::new(p.x, p.y, 0.0), stock);
    screen(camera, P3::new(q.x, q.y, z))
}

/// Where the raw model point `p` falls on the screen (the old pick frame).
fn raw(c: &AppController<SilentBackend>, camera: &OrbitCamera, p: P2) -> [f32; 2] {
    let stock = c.state().session().stock_config();
    screen(camera, P3::new(p.x, p.y, stock.top_z()))
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
    let tc = &state.session().toolpath_configs()[0];
    let item_use = tc.operation.capabilities().item_use;
    let kinds = filter.effective_kinds(item_use, &held(c));
    let ctx = PickContext {
        camera,
        screen_x: at[0],
        screen_y: at[1],
        aspect: VIEW_W / VIEW_H,
        vw: VIEW_W,
        vh: VIEW_H,
        visibility: &state.overlays.model_visibility,
    };
    match pick_items(&ctx, state.session(), tc.id, filter, &kinds)? {
        PickHit::Item { key, .. } => Some(key),
        other => panic!("pick mode gave a hit that is not an item: {other:?}"),
    }
}

/// The state of item `index` in the op selection report.
fn state_of(c: &AppController<SilentBackend>, index: usize) -> ItemState {
    let tc = &c.state().session().toolpath_configs()[0];
    c.state()
        .session()
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
    let items: Vec<Item> = c.state().session().models()[0]
        .items
        .iter()
        .cloned()
        .collect();
    // The loop, the circle and the path: a point on the first edge of each
    // drawn line.
    for index in [0, 1, 2] {
        // Each item starts from the default rule.
        c.handle_internal_event(AppEvent::EditToolpathSelection {
            toolpath_id: mode.toolpath_id,
            target: mode.target,
            edit: rs_cam_core::selection::SelectionEdit::Reset,
        });
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
        // The default rule holds the item. Operator ruling O-PICK: the
        // click replaces the default, so the op cuts that item only.
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
        for other in 0..items.len() {
            let want = if other == index {
                ItemState::Cut
            } else {
                ItemState::NotSelected
            };
            assert_eq!(
                state_of(&c, other),
                want,
                "{face_up:?}/{turn:?}: the click on the {:?} must leave it the one cut item",
                item.kind()
            );
        }
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
    let items: Vec<Item> = c.state().session().models()[0]
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
    // Operator ruling O-PICK: the click on the default rule makes the loop
    // the one picked item; the circle leaves the cut.
    assert_eq!(state_of(&c, 0), ItemState::Cut, "the loop is the pick");
    assert_eq!(
        state_of(&c, 1),
        ItemState::NotSelected,
        "the circle is not picked"
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

    // Right-click on the TEXT loop: the layer holds a loop and a path.
    let items: Vec<Item> = c.state().session().models()[0]
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
            z: None,
        },
    };
    let stored = |c: &AppController<SilentBackend>| {
        c.state().session().toolpath_configs()[0]
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
    let tc = c.state().session().toolpath_configs()[0].clone();
    let report = c
        .state()
        .session()
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

    // A second right-click, with a circle range: the circles get their own
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
        if detector.changed(key.map(HoverTarget::Item)) {
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

/// The open path of model 0 under the pointer at `at`, as the P4.9
/// popover click runs it.
fn open_path_at(
    c: &AppController<SilentBackend>,
    camera: &OrbitCamera,
    at: [f32; 2],
) -> Option<MatchKey> {
    let state = c.state();
    let ctx = PickContext {
        camera,
        screen_x: at[0],
        screen_y: at[1],
        aspect: VIEW_W / VIEW_H,
        vw: VIEW_W,
        vh: VIEW_H,
        visibility: &state.overlays.model_visibility,
    };
    pick_open_path(&ctx, state.session(), ModelId::from_raw(0))
}

/// Show or hide model 0 through the eye of its row.
fn set_visible(c: &mut AppController<SilentBackend>, visible: bool) {
    c.handle_internal_event(AppEvent::Ui(UiCommand::SetModelVisibility(
        SetModelVisibilityArgs {
            model_id: rs_cam_core::ids::ModelId::from_raw(0),
            visible,
        },
    )));
    assert_eq!(
        c.state()
            .overlays
            .model_visibility
            .is_visible(rs_cam_core::ids::ModelId::from_raw(0)),
        visible,
        "the eye command did not set the visibility"
    );
}

/// Geometry selection P4.D follow-up: a hidden model takes no pick. Red
/// proof: the `ctx.shows` checks in `pick_items` and `pick_open_path`
/// removed; the clicks on the hidden drawing picked the loop and the path.
#[test]
fn a_hidden_model_takes_no_pick_p4_d() {
    let mut c = controller(
        OperationConfig::Pocket(Default::default()),
        FaceUp::Top,
        ZRotation::Deg0,
    );
    let mode = pick_on(&mut c);
    let camera = camera(&c);
    let items: Vec<Item> = c.state().session().models()[0]
        .items
        .iter()
        .cloned()
        .collect();
    let (Shape::Loop(outline), Shape::Path(path)) = (items[0].shape(), items[2].shape()) else {
        unreachable!("fixture: item 0 is a loop, item 2 an open path");
    };
    let on_loop = drawn(&c, &camera, mid(outline.exterior[0], outline.exterior[1]));
    let on_path = drawn(&c, &camera, mid(path.exterior[0], path.exterior[1]));

    // Visible: the clicks pick the loop and the open path.
    let loop_key = pick_at(&c, &camera, &mode.filter, on_loop).expect("the loop picks");
    assert!(loop_key.names_same_geometry(items[0].key()));
    let path_key = open_path_at(&c, &camera, on_path).expect("the open path picks");
    assert!(path_key.names_same_geometry(items[2].key()));

    // Hidden: the viewport draws no drawing, so nothing picks.
    set_visible(&mut c, false);
    assert_eq!(
        pick_at(&c, &camera, &mode.filter, on_loop),
        None,
        "a pick-mode click picked an item of a hidden model"
    );
    assert_eq!(
        open_path_at(&c, &camera, on_path),
        None,
        "a popover click picked an open path of a hidden model"
    );

    // Shown again: the clicks pick again.
    set_visible(&mut c, true);
    assert!(pick_at(&c, &camera, &mode.filter, on_loop).is_some());
    assert!(open_path_at(&c, &camera, on_path).is_some());
}

/// The pick-mode hit of the pointer at `at`, as the viewport click and
/// hover run it (`pick_mode_item`, P4), with the pick mode stored in the
/// controller: the key and the kind whose chip is off.
fn pick_mode_at(
    c: &AppController<SilentBackend>,
    camera: &OrbitCamera,
    at: [f32; 2],
) -> Option<(MatchKey, Option<ItemKind>)> {
    let state = c.state();
    let tc = &state.session().toolpath_configs()[0];
    let mode = state
        .gui
        .selection_ui
        .pick_mode(tc.id)
        .expect("pick mode on");
    let item_use = tc.operation.capabilities().item_use;
    let held = held(c);
    let on = mode.filter.effective_kinds(item_use, &held);
    let cut = PickFilter::cut_kinds(item_use, &held);
    let ctx = PickContext {
        camera,
        screen_x: at[0],
        screen_y: at[1],
        aspect: VIEW_W / VIEW_H,
        vw: VIEW_W,
        vh: VIEW_H,
        visibility: &state.overlays.model_visibility,
    };
    let found = pick_mode_item(&ctx, state.session(), tc.id, &mode.filter, &on, &cut)?;
    match found.hit {
        PickHit::Item { key, .. } => Some((key, found.chip_off)),
        other => panic!("pick mode gave a hit that is not an item: {other:?}"),
    }
}

/// Send the event of a pick-mode click on `key`, as the viewport does
/// (`pick_bar::click_event` on the stored mode), and return its name.
fn click(
    c: &mut AppController<SilentBackend>,
    key: MatchKey,
    chip_off: Option<ItemKind>,
    button: PickClick,
) -> &'static str {
    let tc = c.state().session().toolpath_configs()[0].clone();
    let item_use = tc.operation.capabilities().item_use;
    let held = held(c);
    let mode = c
        .state_mut()
        .gui
        .selection_ui
        .pick_mut()
        .expect("pick mode on");
    let event = click_event(mode, item_use, &held, key, chip_off, button)
        .expect("a plain, shift or right click always sends an event");
    let name = match &event {
        AppEvent::TogglePick { .. } => "TogglePick",
        AppEvent::AddPick { .. } => "AddPick",
        AppEvent::PickLayer { .. } => "PickLayer",
        other => panic!("a pick-mode click sent {other:?}"),
    };
    c.handle_internal_event(event);
    name
}

/// P4 (reference audit 2026-10-23, operator approved). Red proof: with the
/// second pass of `pick_mode_item` removed (`return None` for an empty
/// first pass), the click on the TEXT loop with the loop chip off picked
/// nothing.
#[test]
fn a_click_on_an_item_whose_chip_is_off_picks_it_p4click() {
    let mut c = controller(
        OperationConfig::Pocket(Default::default()),
        FaceUp::Top,
        ZRotation::Deg0,
    );
    let _ = pick_on(&mut c);
    let camera = camera(&c);
    let items: Vec<Item> = c.state().session().models()[0]
        .items
        .iter()
        .cloned()
        .collect();
    let set_chip = |c: &mut AppController<SilentBackend>, kind: ItemKind, on: bool| {
        let mode = c.state_mut().gui.selection_ui.pick_mut().unwrap();
        if on {
            let _ = mode.filter.kinds.insert(kind);
        } else {
            let _ = mode.filter.kinds.remove(&kind);
        }
    };
    let chip_on = |c: &AppController<SilentBackend>, kind: ItemKind| {
        let tc = &c.state().session().toolpath_configs()[0];
        c.state()
            .gui
            .selection_ui
            .pick_mode(tc.id)
            .unwrap()
            .filter
            .kinds
            .contains(&kind)
    };

    // An item of an on chip still wins: circle chip on, loop chip off, on
    // the circle ring 1 mm from the loop edge.
    set_chip(&mut c, ItemKind::Loop, false);
    let near_both = drawn(&c, &camera, P2::new(11.0, 20.0));
    let (key, chip_off) = pick_mode_at(&c, &camera, near_both).expect("a hit");
    assert!(key.names_same_geometry(items[1].key()), "the circle wins");
    assert_eq!(chip_off, None, "the circle chip is on");

    // Circle chip on, loop chip off: a click on the TEXT loop, far from
    // every circle, picks the loop and names its chip.
    let Shape::Loop(text) = items[3].shape() else {
        unreachable!("fixture: item 3 is a loop");
    };
    let on_text = drawn(&c, &camera, mid(text.exterior[0], text.exterior[1]));
    let (key, chip_off) = pick_mode_at(&c, &camera, on_text)
        .expect("a click on a loop with the loop chip off must pick it (P4)");
    assert!(key.names_same_geometry(items[3].key()), "the TEXT loop");
    assert_eq!(chip_off, Some(ItemKind::Loop));
    assert!(!chip_on(&c, ItemKind::Loop), "fixture: the chip is off");

    // The click turns the chip on and adds the loop.
    let event = click(&mut c, key, chip_off, PickClick::Plain);
    assert!(
        event == "AddPick",
        "a click on a chip-off item adds it: {event:?}"
    );
    assert!(
        chip_on(&c, ItemKind::Loop),
        "the click turns the loop chip on"
    );
    assert_eq!(state_of(&c, 3), ItemState::Cut, "the TEXT loop is picked");
    // The pointer now hits an on chip: no chip to turn on.
    assert_eq!(pick_mode_at(&c, &camera, on_text).unwrap().1, None);

    // A Pocket cannot cut an open path: with every chip off, a click on
    // the path picks nothing.
    set_chip(&mut c, ItemKind::Loop, false);
    set_chip(&mut c, ItemKind::Circle, false);
    let Shape::Path(path) = items[2].shape() else {
        unreachable!("fixture: item 2 is an open path");
    };
    let on_path = drawn(&c, &camera, mid(path.exterior[0], path.exterior[1]));
    assert_eq!(
        pick_mode_at(&c, &camera, on_path),
        None,
        "an open path is not a kind that a Pocket cuts"
    );
}

/// P3 (reference audit 2026-10-23, operator approved). Red proof: with
/// `PickClick::Shift` mapped to `PickLayer` in `click_event` (the old
/// shift-click), the first shift-click stored a TEXT layer rule.
#[test]
fn a_shift_click_adds_one_item_p3shift() {
    let mut c = controller(
        OperationConfig::Pocket(Default::default()),
        FaceUp::Top,
        ZRotation::Deg0,
    );
    let _ = pick_on(&mut c);
    let items: Vec<Item> = c.state().session().models()[0]
        .items
        .iter()
        .cloned()
        .collect();
    let stored = |c: &AppController<SilentBackend>| {
        c.state().session().toolpath_configs()[0]
            .operation
            .selection()
            .unwrap()
            .clone()
    };

    // Shift-click on the TEXT loop: one pick, no rule.
    let event = click(&mut c, items[3].key().clone(), None, PickClick::Shift);
    assert!(
        event == "AddPick",
        "a shift-click adds one item, never a layer: {event:?}"
    );
    assert!(
        stored(&c).rules().is_empty(),
        "no layer rule: {:?}",
        stored(&c)
    );
    assert_eq!(stored(&c).picks().len(), 1);
    assert_eq!(state_of(&c, 3), ItemState::Cut);
    assert_eq!(state_of(&c, 2), ItemState::NotSelected, "the TEXT path");

    // A second shift-click on the CUT loop: two picks.
    let _ = click(&mut c, items[0].key().clone(), None, PickClick::Shift);
    assert_eq!(stored(&c).picks().len(), 2);
    assert_eq!(state_of(&c, 0), ItemState::Cut);
    assert_eq!(state_of(&c, 3), ItemState::Cut);

    // A shift-click on a picked item keeps it (add, not toggle).
    let _ = click(&mut c, items[3].key().clone(), None, PickClick::Shift);
    assert_eq!(stored(&c).picks().len(), 2);
    assert_eq!(state_of(&c, 3), ItemState::Cut);

    // A plain click still toggles: it removes the picked loop.
    let event = click(&mut c, items[3].key().clone(), None, PickClick::Plain);
    assert!(event == "TogglePick");
    assert_eq!(state_of(&c, 3), ItemState::NotSelected);

    // A right-click adds the layer of the item as a rule.
    let event = click(&mut c, items[3].key().clone(), None, PickClick::Secondary);
    assert!(
        event == "PickLayer",
        "a right-click adds the layer: {event:?}"
    );
    assert_eq!(stored(&c).rules().len(), 1, "the TEXT layer rule");
    assert_eq!(state_of(&c, 3), ItemState::Cut);
}

#[test]
fn in_pick_mode_only_a_pickable_face_lights_live1() {
    use rs_cam_core::geometry::enriched_mesh::FaceGroupId;
    use rs_cam_core::ids::ModelId;
    let mut c = controller(
        OperationConfig::Pocket(Default::default()),
        FaceUp::Top,
        ZRotation::Deg0,
    );
    let mode = pick_on(&mut c);
    let camera = camera(&c);
    let key = pick_at(
        &c,
        &camera,
        &mode.filter,
        drawn(&c, &camera, P2::new(25.0, 10.0)),
    )
    .expect("fixture: the loop edge y = 10 gives a key");
    // The tooltip is one short line: the item and its measure, then the
    // acts.
    let tc = c.state().session().toolpath_configs()[0].clone();
    let tip = rs_cam_viz::ui::pick_bar::hover_text(
        c.state().session(),
        &tc,
        SelectionTarget::Op,
        &key,
        None,
    );
    assert!(!tip.contains('\n'), "one line: {tip:?}");
    assert!(
        tip.starts_with("Closed contour, ") && tip.ends_with(rs_cam_viz::ui::pick_bar::HOVER_ACTS),
        "the kind and the measure, then the acts: {tip:?}"
    );
    assert!(tip.chars().count() <= 90, "a short line: {tip:?}");
    let top = (ModelId::from_wire(1), FaceGroupId(3));
    let side = (ModelId::from_wire(1), FaceGroupId(4));

    // Pick mode, a pickable face under the pointer: the face lights.
    assert_eq!(
        hover_target(true, Some(top), Some(&key), None, Some(top)),
        Some(HoverTarget::Face(top.0, top.1))
    );
    // Pick mode, an item that is not a face: the ring.
    assert_eq!(
        hover_target(true, None, Some(&key), None, Some(side)),
        Some(HoverTarget::Item(key.clone()))
    );
    // Pick mode, a side face (its key is `Err`, or no chip admits it):
    // no hover at all.
    assert_eq!(hover_target(true, None, None, None, Some(side)), None);
    // Pick mode, the Geometry tab row still rings.
    assert_eq!(
        hover_target(true, None, None, Some(&key), Some(side)),
        Some(HoverTarget::Item(key.clone()))
    );
    // Outside pick mode any face under the pointer lights.
    assert_eq!(
        hover_target(false, None, None, None, Some(side)),
        Some(HoverTarget::Face(side.0, side.1))
    );
    assert_eq!(
        hover_target(false, None, None, Some(&key), Some(side)),
        Some(HoverTarget::Item(key))
    );
}

fn key_of(c: &AppController<SilentBackend>, index: usize) -> MatchKey {
    c.state().session().models()[0]
        .items
        .iter()
        .nth(index)
        .unwrap()
        .key()
        .clone()
}

#[test]
fn step_cycle_starts_on_the_second_candidate_and_wraps_e_q6() {
    let c = controller(
        OperationConfig::Pocket(Default::default()),
        FaceUp::Top,
        ZRotation::Deg0,
    );
    let (a, b, d) = (key_of(&c, 0), key_of(&c, 1), key_of(&c, 2));
    let list = || vec![a.clone(), b.clone(), d.clone()];
    let at = [100.0, 80.0];
    // A Tab at a new point: the list, on the second candidate (the hover
    // already shows the first).
    let one = step_cycle(None, at, list).unwrap();
    assert_eq!((one.index, one.position()), (1, (2, 3)));
    assert_eq!(one.current(), Some(&b));
    // The same point (within the stay): the next, then the wrap.
    let near = [at[0] + CYCLE_STAY_PX * 0.5, at[1]];
    let two = step_cycle(Some(one), near, || panic!("no new list at the same point")).unwrap();
    assert_eq!(two.current(), Some(&d));
    let three = step_cycle(Some(two), at, || panic!("no new list")).unwrap();
    assert_eq!(three.current(), Some(&a), "the cycle wraps");
    // A Tab beyond the stay makes a new list there.
    let far = [at[0] + CYCLE_STAY_PX * 2.0, at[1]];
    let fresh = step_cycle(Some(three), far, || vec![d.clone()]).unwrap();
    assert_eq!((fresh.at, fresh.index, fresh.position()), (far, 0, (1, 1)));
    // No candidate: no cycle.
    assert_eq!(step_cycle(None, at, Vec::new), None);
}

#[test]
fn step_cycle_ends_on_a_move_and_on_a_mode_change_e_q6() {
    let mut c = controller(
        OperationConfig::Pocket(Default::default()),
        FaceUp::Top,
        ZRotation::Deg0,
    );
    let cycle = PickCycle {
        at: [10.0, 10.0],
        candidates: vec![key_of(&c, 0), key_of(&c, 1)],
        index: 1,
    };
    assert_eq!(
        keep_cycle(Some(cycle.clone()), Some([11.0, 10.0])),
        Some(cycle.clone()),
        "a hand shake keeps it"
    );
    assert_eq!(keep_cycle(Some(cycle.clone()), Some([20.0, 10.0])), None);
    assert_eq!(
        keep_cycle(Some(cycle.clone()), None),
        None,
        "the pointer left"
    );
    let _ = pick_on(&mut c);
    let ui = &mut c.state_mut().gui.selection_ui;
    ui.cycle = Some(cycle);
    ui.cycle_request = true;
    ui.end_mode();
    assert_eq!((ui.cycle.clone(), ui.cycle_request), (None, false));
}

#[test]
fn a_tab_lists_every_candidate_under_the_pointer_e_q6() {
    let mut c = controller(
        OperationConfig::Pocket(Default::default()),
        FaceUp::Bottom,
        ZRotation::Deg0,
    );
    let mode = pick_on(&mut c);
    let camera = camera(&c);
    // On the circle ring, 1 mm from the loop edge x = 10 (claim 2).
    let at = drawn(&c, &camera, P2::new(11.0, 20.0));
    let state = c.state();
    let tc = &state.session().toolpath_configs()[0];
    let kinds = mode
        .filter
        .effective_kinds(tc.operation.capabilities().item_use, &held(&c));
    let ctx = PickContext {
        camera: &camera,
        screen_x: at[0],
        screen_y: at[1],
        aspect: VIEW_W / VIEW_H,
        vw: VIEW_W,
        vh: VIEW_H,
        visibility: &state.overlays.model_visibility,
    };
    let keys: Vec<MatchKey> = pick_candidates(&ctx, state.session(), tc.id, &mode.filter, &kinds)
        .into_iter()
        .map(|hit| match hit {
            PickHit::Item { key, .. } => key,
            other => panic!("a drawing gives item candidates only: {other:?}"),
        })
        .collect();
    let (lp, circle) = (key_of(&c, 0), key_of(&c, 1));
    assert_eq!(
        keys,
        vec![circle.clone(), lp.clone()],
        "the circle (on the pointer) first, then the loop (1 mm away)"
    );
    // The first candidate is the hover's own pick.
    assert_eq!(pick_at(&c, &camera, &mode.filter, at), Some(circle));
    // One Tab: the loop is in force; a click picks it through the
    // ordinary `TogglePick` (`pick_edit`).
    let cycle = step_cycle(None, at, || keys.clone()).unwrap();
    assert_eq!(cycle.current(), Some(&lp));
    c.handle_internal_event(AppEvent::TogglePick {
        toolpath_id: mode.toolpath_id,
        target: mode.target,
        key: cycle.current().unwrap().clone(),
    });
    assert_eq!(
        state_of(&c, 0),
        ItemState::Cut,
        "the cycled loop is the pick"
    );
    assert_eq!(state_of(&c, 1), ItemState::NotSelected);
    assert!(
        rs_cam_viz::ui::pick_bar::cycle_note(2, 3) == "2 of 3 \u{00B7} Tab for the next",
        "the one-line note"
    );
}

#[test]
fn tab_reaches_the_gesture_translation_e_q6() {
    use rs_cam_viz::interaction::intent::{ViewContext, ViewKey, ViewportGesture, gesture_events};
    use rs_cam_viz::ui_command::{NoArgs, UiCommand};
    let mut c = controller(
        OperationConfig::Pocket(Default::default()),
        FaceUp::Top,
        ZRotation::Deg0,
    );
    let tab = |c: &AppController<SilentBackend>| {
        let state = c.state();
        gesture_events(
            &ViewportGesture::Key { key: ViewKey::Tab },
            &ViewContext {
                workspace: state.workspace,
                focus: state.focus(),
                selection_ui: &state.gui.selection_ui,
                session: state.session(),
            },
        )
    };
    assert!(tab(&c).is_empty(), "no pick mode: Tab does nothing");
    let _ = pick_on(&mut c);
    let events = tab(&c);
    assert!(matches!(
        events.as_slice(),
        [AppEvent::Ui(UiCommand::CyclePickCandidate(NoArgs))]
    ));
    // The handler asks the viewport frame for the next candidate.
    for event in events {
        c.handle_internal_event(event);
    }
    assert!(c.state().gui.selection_ui.cycle_request);

    // The key path: the keyboard handler sends Tab through the gesture
    // translation, in pick mode, and consumes the key.
    let input = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/app/input.rs"),
    )
    .unwrap();
    let at = input
        .find("i.consume_key(egui::Modifiers::NONE, egui::Key::Tab)")
        .expect("the keyboard handler consumes Tab");
    let rest = &input[at..];
    let gesture = rest
        .find("ViewKey::Tab")
        .expect("Tab goes through gesture_events");
    assert!(
        rest[..gesture].contains("gesture_events("),
        "Tab reaches gesture_events"
    );
}

/// Round 4 (operator ruling): a focused toolpath in Toolpaths is in pick
/// mode at once, through THE rule `engage_pick_for_focus` that the focus
/// door and the workspace switch reach. Escape pauses it: nothing engages
/// again until the next focus or workspace change. A 3D op picks its
/// boundary when the boundary source is a selection, else nothing.
#[test]
fn a_focused_toolpath_is_in_pick_mode_at_once_round4() {
    use rs_cam_core::compute::config::BoundarySource;
    use rs_cam_core::session::AddToolpathArgs;
    use rs_cam_viz::state::pick::{ViewportMode, engage_pick_for_focus};
    use rs_cam_viz::ui_command::{NoArgs, UiCommand};
    let mut c = controller(
        OperationConfig::Pocket(Default::default()),
        FaceUp::Top,
        ZRotation::Deg0,
    );
    let pocket = c.state().session().toolpath_configs()[0].id;
    let select = |c: &mut AppController<SilentBackend>, s: UiSelection| {
        c.handle_internal_event(AppEvent::Ui(UiCommand::Select(s)));
    };
    let mode_of = |c: &AppController<SilentBackend>| match &c.state().gui.selection_ui.mode {
        Some(ViewportMode::Pick(m)) => Some((m.toolpath_id, m.target)),
        _ => None,
    };
    select(&mut c, UiSelection::None);
    assert_eq!(mode_of(&c), None, "no toolpath focus, no mode");
    select(&mut c, UiSelection::Toolpath(pocket));
    assert_eq!(
        mode_of(&c),
        Some((pocket, SelectionTarget::Op)),
        "engaged at once"
    );
    // Every served kind is on.
    let tc = c.state().session().toolpath_configs()[0].clone();
    let served = PickFilter::served(tc.operation.capabilities().item_use);
    assert_eq!(
        c.state().gui.selection_ui.pick_mode(pocket).unwrap().filter,
        served
    );

    // Escape pauses: the mode ends and the same focus does not engage it.
    c.handle_internal_event(AppEvent::Ui(UiCommand::EndViewportMode(NoArgs)));
    assert_eq!(mode_of(&c), None);
    select(&mut c, UiSelection::Toolpath(pocket));
    assert_eq!(mode_of(&c), None, "paused until the next change");

    // The workspace switch: leaving Toolpaths ends it, coming back engages.
    rs_cam_viz::ui::overlays::registry::switch_workspace(c.state_mut(), Workspace::Setup);
    assert_eq!(mode_of(&c), None);
    rs_cam_viz::ui::overlays::registry::switch_workspace(c.state_mut(), Workspace::Toolpaths);
    assert_eq!(mode_of(&c), Some((pocket, SelectionTarget::Op)));

    // A 3D op: no mode on a stock boundary; the boundary target on a
    // selection boundary.
    let mut drop = toolpath(OperationConfig::DropCutter(Default::default()));
    drop.model_id = tc.model_id;
    let _ = c
        .state_mut()
        .apply(Command::AddToolpath(AddToolpathArgs {
            setup_index: 0,
            config: Box::new(drop),
        }))
        .unwrap();
    let drop_id = c.state().session().toolpath_configs()[1].id;
    select(&mut c, UiSelection::Toolpath(drop_id));
    assert_eq!(
        mode_of(&c),
        None,
        "a 3D op on a stock boundary has no pick mode"
    );
    let mut with_selection = c.state().session().toolpath_configs()[1].clone();
    with_selection.boundary.source = BoundarySource::Selection {
        selection: rs_cam_core::selection::Selection::drawing(),
        holes: false,
    };
    let _ = c
        .state_mut()
        .apply(Command::ReplaceToolpathConfig(
            rs_cam_core::session::ReplaceToolpathConfigArgs {
                index: 1,
                config: Box::new(with_selection),
            },
        ))
        .unwrap();
    select(&mut c, UiSelection::None);
    select(&mut c, UiSelection::Toolpath(drop_id));
    assert_eq!(mode_of(&c), Some((drop_id, SelectionTarget::Boundary)));
    // The rule runs on demand too; a mode of the focused toolpath is kept.
    engage_pick_for_focus(c.state_mut());
    assert_eq!(mode_of(&c), Some((drop_id, SelectionTarget::Boundary)));
}

/// Round 4 (operator ruling): the hover follows the op's use of the item. A
/// Profile cuts a contour, so a pickable face under the pointer rings (the
/// hover slot holds the face's item key); a Pocket cuts the area, so the
/// face lights as a face.
#[test]
fn a_face_hover_follows_the_item_use_round4() {
    use rs_cam_core::compute::catalog::FaceFeedback;
    use rs_cam_core::geometry::enriched_mesh::FaceGroupId;
    use rs_cam_core::ids::ModelId;
    use rs_cam_viz::state::pick::face_feedback;
    use rs_cam_viz::state::selection_highlight::pick_mode_face;
    let face = (ModelId::from_wire(1), FaceGroupId(2));
    let key = MatchKey {
        source: rs_cam_core::selection::SourceRef::ToolpathModel,
        kind: ItemKind::Face,
        layer: None,
        anchor: [10.0, 10.0],
        measure: 20.0,
        brep: Some(2),
        z: None,
    };
    for (operation, want) in [
        (
            OperationConfig::Profile(Default::default()),
            HoverTarget::Item(key.clone()),
        ),
        (
            OperationConfig::Pocket(Default::default()),
            HoverTarget::Face(face.0, face.1),
        ),
    ] {
        let tc = toolpath(operation);
        let feedback = face_feedback(&tc, SelectionTarget::Op);
        let target = hover_target(
            true,
            pick_mode_face(feedback, Some(face)),
            Some(&key),
            None,
            Some(face),
        );
        assert_eq!(target, Some(want), "{feedback:?}");
    }
    // The boundary is an area.
    let tc = toolpath(OperationConfig::Profile(Default::default()));
    assert_eq!(
        face_feedback(&tc, SelectionTarget::Boundary),
        FaceFeedback::Fill
    );
}
