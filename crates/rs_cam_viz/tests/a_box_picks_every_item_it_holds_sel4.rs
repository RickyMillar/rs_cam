//! B6 (SEL-4): a box in pick mode picks every item it holds.
//!
//! # The claim
//!
//! 1. A left-to-right box over two loops gives ONE `AddPicks` with the two
//!    keys (`pick_items_in_rect`, then `gesture_events(BoxRelease)`); on the
//!    op default it replaces the default (O-PICK), and it is one undo step.
//! 2. Enclose (left to right) takes an item whose every vertex is inside;
//!    crossing (right to left) also takes an item that the box cuts.
//! 3. The chips filter the box: a chip-off kind is not taken.
//! 4. A Ctrl box removes (`RemovePicks`); a press that moves less than
//!    egui's `max_click_dist` (6 pt) is a click, not a box.
//! 5. Escape during a box cancels it and keeps the pick mode; the release
//!    then sends nothing, and the next Escape ends the mode.
//! 6. B3: a left drag is a box in pick mode (`Intent::Select` in both
//!    schemes); outside pick mode it does nothing (O-1).
//! 7. The box paints over the scene: in `app/viewport.rs` the
//!    `paint_box(` call comes after the paint callback push
//!    (`Callback::new_paint_callback(`). A shape added before the callback
//!    is under the 3D scene (the live look, 2026-10-29).

#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::path::PathBuf;
use std::sync::Arc;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits, StockConfig};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::geo::{P2, P3};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::selection::{Item, ItemKind, ItemSet, SelectionEdit, Shape, SourceRef};
use rs_cam_core::session::{LoadedModel, ProjectSessionBuilder, SelectionTarget, ToolpathConfig};
use rs_cam_core::settings::MouseScheme;
use rs_cam_viz::compute::{
    CollisionRequest, ComputeBackend, ComputeLane, ComputeMessage, ComputeRequest,
    GenerationControl, LaneSnapshot, SimulationRequest, ToolpathSubmitOutcome,
};
use rs_cam_viz::controller::AppController;
use rs_cam_viz::interaction::input::{InputAction, InputEnv, InputSample, step};
use rs_cam_viz::interaction::intent::{ViewContext, ViewKey, ViewportGesture, gesture_events};
use rs_cam_viz::interaction::picking::{PickContext, ScreenRect, pick_items_in_rect};
use rs_cam_viz::render::camera::OrbitCamera;
use rs_cam_viz::state::Workspace;
use rs_cam_viz::state::display_frame::drawing_plane_z;
use rs_cam_viz::state::job::SetupFrame;
use rs_cam_viz::state::pick::{PickFilter, PickMode};
use rs_cam_viz::state::selection::Selection as UiSelection;
use rs_cam_viz::state::viewport_input::{BoxMode, BoxOp, DragState};
use rs_cam_viz::ui::AppEvent;
use rs_cam_viz::ui_command::UiCommand;

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

fn ring(points: &[(f64, f64)]) -> Polygon2 {
    Polygon2::new(points.iter().map(|&(x, y)| P2::new(x, y)).collect())
}

fn rect_ring(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<P2> {
    vec![
        P2::new(x0, y0),
        P2::new(x1, y0),
        P2::new(x1, y1),
        P2::new(x0, y1),
    ]
}

/// 0: a loop on CUT (10, 10)-(40, 30); 1: a circle on HOLES inside it,
/// centre (14, 20), diameter 6; 2: a loop on TEXT (55, 45)-(75, 60).
fn drawing() -> ItemSet {
    let item = |layer: &str, shape| {
        Item::new(SourceRef::ToolpathModel, Some(layer.into()), shape).unwrap()
    };
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
        item(
            "TEXT",
            Shape::Loop(ring(&[
                (55.0, 45.0),
                (75.0, 45.0),
                (75.0, 60.0),
                (55.0, 60.0),
            ])),
        ),
    ])
}

/// One Pocket at its default selection, focused, its pick mode on.
fn controller() -> AppController<SilentBackend> {
    let mut builder = ProjectSessionBuilder::new()
        .stock(StockConfig {
            x: 100.0,
            y: 80.0,
            z: Some(20.0),
            origin_x: -10.0,
            origin_y: -5.0,
            origin_z: -20.0,
            auto_from_model: false,
            ..StockConfig::default()
        })
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
    let tc = ToolpathConfig {
        shoe: Default::default(),
        id: rs_cam_core::ToolpathId::from_raw(0),
        name: "Pocket".to_owned(),
        enabled: true,
        operation: OperationConfig::Pocket(Default::default()),
        dressups: Default::default(),
        heights: Default::default(),
        tool_id: Some(rs_cam_core::ids::ToolId::from_raw(1)),
        model_id,
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
    };
    let _ = builder.add_toolpath(0, tc).unwrap();
    let mut c = AppController::with_backend(SilentBackend);
    c.state_mut().replace_session(builder.build());
    let tc = c.state().session().toolpath_configs()[0].clone();
    let state = c.state_mut();
    state.workspace = Workspace::Toolpaths;
    let _ = state.set_focus(UiSelection::Toolpath(tc.id));
    state.gui.selection_ui.set_pick(Some(PickMode::new(
        tc.id,
        SelectionTarget::Op,
        tc.operation.capabilities().item_use,
    )));
    c
}

fn camera(c: &AppController<SilentBackend>) -> OrbitCamera {
    let stock = c.state().session().stock_config().clone();
    let frame = SetupFrame::of(&c.state().session().list_setups()[0]);
    let (w, d, h) = frame.effective_stock(&stock);
    let mut camera = OrbitCamera::new();
    camera.set_pose(camera.fit_pose([0.0, 0.0, 0.0], [w as f32, d as f32, h as f32]));
    camera
}

/// The screen points of the model points `pts`, as the drawing draws them.
fn screen_of(c: &AppController<SilentBackend>, camera: &OrbitCamera, pts: &[P2]) -> Vec<[f64; 2]> {
    let state = c.state();
    let stock = state.session().stock_config();
    let frame = SetupFrame::of(&state.session().list_setups()[0]);
    let tc = &state.session().toolpath_configs()[0];
    let z = drawing_plane_z(state.session(), tc, Some(&frame));
    pts.iter()
        .map(|p| {
            let q = frame.transform_point(P3::new(p.x, p.y, 0.0), stock);
            let s = camera
                .project_to_screen(
                    [q.x as f32, q.y as f32, z as f32],
                    VIEW_W / VIEW_H,
                    VIEW_W,
                    VIEW_H,
                )
                .expect("in view");
            [f64::from(s[0]), f64::from(s[1])]
        })
        .collect()
}

/// The screen rectangle around `pts`, grown by `margin` points.
fn around(pts: &[[f64; 2]], margin: f64) -> ScreenRect {
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for p in pts {
        x0 = x0.min(p[0]);
        y0 = y0.min(p[1]);
        x1 = x1.max(p[0]);
        y1 = y1.max(p[1]);
    }
    ScreenRect::from_corners([x0 - margin, y0 - margin], [x1 + margin, y1 + margin])
}

fn boxed(
    c: &AppController<SilentBackend>,
    camera: &OrbitCamera,
    kinds: &[ItemKind],
    rect: ScreenRect,
    mode: BoxMode,
) -> Vec<rs_cam_core::selection::MatchKey> {
    let state = c.state();
    let tc = &state.session().toolpath_configs()[0];
    pick_items_in_rect(
        &PickContext {
            camera,
            screen_x: 0.0,
            screen_y: 0.0,
            aspect: VIEW_W / VIEW_H,
            vw: VIEW_W,
            vh: VIEW_H,
            visibility: &state.overlays.model_visibility,
        },
        state.session(),
        tc.id,
        &PickFilter::served(tc.operation.capabilities().item_use),
        kinds,
        rect,
        mode,
    )
}

fn release_events(
    c: &AppController<SilentBackend>,
    keys: Vec<rs_cam_core::selection::MatchKey>,
    op: BoxOp,
) -> Vec<AppEvent> {
    let state = c.state();
    gesture_events(
        &ViewportGesture::BoxRelease { keys, op },
        &ViewContext {
            workspace: state.workspace,
            focus: state.focus(),
            selection_ui: &state.gui.selection_ui,
            session: state.session(),
        },
    )
}

#[test]
fn a_left_drag_in_pick_mode_sends_one_add_picks_sel4() {
    let mut c = controller();
    let camera = camera(&c);
    let mut corners = screen_of(&c, &camera, &rect_ring(10.0, 10.0, 40.0, 30.0));
    corners.extend(screen_of(&c, &camera, &rect_ring(55.0, 45.0, 75.0, 60.0)));
    let keys = boxed(
        &c,
        &camera,
        &[ItemKind::Loop],
        around(&corners, 4.0),
        BoxMode::Enclose,
    );
    assert_eq!(keys.len(), 2, "the box holds both loops: {keys:?}");

    let events = release_events(&c, keys, BoxOp::Add);
    assert!(matches!(
        events.as_slice(),
        [AppEvent::EditToolpathSelection { edit: SelectionEdit::AddPicks(k), .. }] if k.len() == 2
    ));
    let undo_before = c.state().history.undo_len();
    for event in events {
        c.handle_internal_event(event);
    }
    let stored = c.state().session().toolpath_configs()[0]
        .operation
        .selection()
        .unwrap()
        .clone();
    assert!(
        stored.rules().is_empty(),
        "the first box replaced the default (O-PICK)"
    );
    assert_eq!(stored.picks().len(), 2);
    assert_eq!(
        c.state().history.undo_len(),
        undo_before + 1,
        "one undo step"
    );
}

#[test]
fn left_to_right_encloses_and_right_to_left_crosses_sel4() {
    let c = controller();
    let camera = camera(&c);
    // Around loop 0, and on to the middle of loop 2.
    let mut pts = screen_of(&c, &camera, &rect_ring(10.0, 10.0, 40.0, 30.0));
    pts.extend(screen_of(&c, &camera, &[P2::new(65.0, 52.0)]));
    let rect = around(&pts, 2.0);
    let enclose = boxed(&c, &camera, &[ItemKind::Loop], rect, BoxMode::Enclose);
    let crossing = boxed(&c, &camera, &[ItemKind::Loop], rect, BoxMode::Crossing);
    assert_eq!(enclose.len(), 1, "enclose takes loop 0 only: {enclose:?}");
    assert_eq!(
        crossing.len(),
        2,
        "crossing also takes loop 2: {crossing:?}"
    );
    assert_eq!(
        DragState::box_mode(egui::pos2(0.0, 0.0), egui::pos2(10.0, 5.0)),
        BoxMode::Enclose
    );
    assert_eq!(
        DragState::box_mode(egui::pos2(10.0, 0.0), egui::pos2(0.0, 5.0)),
        BoxMode::Crossing
    );
}

#[test]
fn the_chips_filter_the_box_sel4() {
    let c = controller();
    let camera = camera(&c);
    let rect = around(
        &screen_of(&c, &camera, &rect_ring(10.0, 10.0, 40.0, 30.0)),
        2.0,
    );
    let both = boxed(
        &c,
        &camera,
        &[ItemKind::Loop, ItemKind::Circle],
        rect,
        BoxMode::Enclose,
    );
    assert_eq!(both.len(), 2, "loop 0 and the circle inside it: {both:?}");
    let loops = boxed(&c, &camera, &[ItemKind::Loop], rect, BoxMode::Enclose);
    assert_eq!(loops.len(), 1, "the circle chip is off: {loops:?}");
    assert_eq!(loops[0].kind, ItemKind::Loop);
}

fn press(button: egui::PointerButton, modifiers: egui::Modifiers) -> InputSample {
    InputSample {
        started: Some(button),
        dragging: Some(button),
        stopped: false,
        delta: egui::vec2(20.0, 10.0),
        press_origin: Some(egui::pos2(100.0, 100.0)),
        pointer: Some(egui::pos2(120.0, 110.0)),
        hover_pos: None,
        modifiers,
        clicked: false,
        secondary_clicked: false,
        hovered: true,
        zoom_delta: 1.0,
        scroll: egui::Vec2::ZERO,
    }
}

fn released() -> InputSample {
    InputSample {
        started: None,
        dragging: None,
        stopped: true,
        delta: egui::Vec2::ZERO,
        press_origin: None,
        pointer: None,
        hover_pos: None,
        modifiers: egui::Modifiers::NONE,
        clicked: false,
        secondary_clicked: false,
        hovered: true,
        zoom_delta: 1.0,
        scroll: egui::Vec2::ZERO,
    }
}

const PICK_ENV: InputEnv = InputEnv {
    scheme: MouseScheme::DesktopCad,
    pick_mode_on: true,
    press_on_tab: false,
    drag_anchor: None,
};

#[test]
fn a_ctrl_box_removes_and_a_short_press_is_a_click_sel4() {
    // Ctrl box: Remove.
    let mut drag = DragState::Idle;
    let _ = step(
        &mut drag,
        &press(egui::PointerButton::Primary, egui::Modifiers::COMMAND),
        &PICK_ENV,
    );
    assert!(matches!(
        drag,
        DragState::BoxSelect {
            op: BoxOp::Remove,
            ..
        }
    ));
    let actions = step(&mut drag, &released(), &PICK_ENV);
    let Some(InputAction::BoxRelease { op, .. }) = actions.first() else {
        panic!("the release sends the box: {actions:?}");
    };
    assert_eq!(*op, BoxOp::Remove);
    let c = controller();
    let key = c.state().session().models()[0]
        .items
        .iter()
        .next()
        .unwrap()
        .key()
        .clone();
    assert!(matches!(
        release_events(&c, vec![key], BoxOp::Remove).as_slice(),
        [AppEvent::EditToolpathSelection {
            edit: SelectionEdit::RemovePicks(_),
            ..
        }]
    ));

    // A press that moves less than egui's click distance is a click: egui
    // reports `clicked` and no drag start, and `step` gives a click.
    assert!(egui::InputOptions::default().max_click_dist > 5.0);
    let mut drag = DragState::Idle;
    let click = InputSample {
        clicked: true,
        pointer: Some(egui::pos2(103.0, 104.0)),
        ..released()
    };
    let click = InputSample {
        stopped: false,
        ..click
    };
    let actions = step(&mut drag, &click, &PICK_ENV);
    assert!(
        matches!(actions.as_slice(), [InputAction::Click { .. }]),
        "{actions:?}"
    );
    assert_eq!(drag, DragState::Idle);
}

#[test]
fn escape_cancels_a_box_and_keeps_pick_mode_sel4() {
    let mut c = controller();
    let mut drag = DragState::Idle;
    let _ = step(
        &mut drag,
        &press(egui::PointerButton::Primary, egui::Modifiers::NONE),
        &PICK_ENV,
    );
    c.state_mut().gui.selection_ui.drag = drag;
    // The first Escape cancels the box.
    assert!(c.state_mut().gui.selection_ui.cancel_box());
    assert_eq!(c.state().gui.selection_ui.drag, DragState::Cancelled);
    assert!(c.state().gui.selection_ui.mode.is_some(), "the mode stays");
    // The release of a cancelled box sends nothing.
    let mut drag = c.state().gui.selection_ui.drag.clone();
    assert!(step(&mut drag, &released(), &PICK_ENV).is_empty());
    assert_eq!(drag, DragState::Idle);
    c.state_mut().gui.selection_ui.drag = drag;
    // The next Escape ends the mode.
    assert!(!c.state_mut().gui.selection_ui.cancel_box());
    let state = c.state();
    let events = gesture_events(
        &ViewportGesture::Key {
            key: ViewKey::Escape,
        },
        &ViewContext {
            workspace: state.workspace,
            focus: state.focus(),
            selection_ui: &state.gui.selection_ui,
            session: state.session(),
        },
    );
    assert!(matches!(
        events.as_slice(),
        [AppEvent::Ui(UiCommand::EndViewportMode(_))]
    ));
}

#[test]
fn a_left_drag_boxes_in_pick_mode_and_does_nothing_outside_it_sel4() {
    let mut drag = DragState::Idle;
    let actions = step(
        &mut drag,
        &press(egui::PointerButton::Primary, egui::Modifiers::NONE),
        &PICK_ENV,
    );
    assert!(actions.is_empty(), "a box sends nothing until its release");
    assert!(matches!(drag, DragState::BoxSelect { op: BoxOp::Add, .. }));
    let mut drag = DragState::Idle;
    let outside = InputEnv {
        pick_mode_on: false,
        ..PICK_ENV
    };
    let actions = step(
        &mut drag,
        &press(egui::PointerButton::Primary, egui::Modifiers::NONE),
        &outside,
    );
    assert!(
        actions.is_empty(),
        "outside pick mode a left drag does nothing: {actions:?}"
    );
    assert_eq!(drag, DragState::Idle);
}

#[test]
fn the_box_paints_after_the_scene_callback_sel4() {
    let src = include_str!("../src/app/viewport.rs");
    let callback = src
        .find("Callback::new_paint_callback(")
        .expect("the viewport pushes the scene paint callback");
    let paint = src.find("paint_box(").expect("the viewport paints the box");
    assert_eq!(
        src.matches("paint_box(").count(),
        1,
        "one box paint in the viewport"
    );
    assert!(
        paint > callback,
        "the box paints after the scene callback, so the scene does not cover it"
    );
}
