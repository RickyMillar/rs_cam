//! Geometry selection P4.6 — the viewport hand-tab editor of a Profile.
//!
//! # The claims
//!
//! 1. `an_auto_tab_drag_writes_every_tab_of_its_loop_in_one_step_p4_6`
//!    (L-5): the release of a drag of an auto tab writes every tab of its
//!    loop as a hand tab, the dragged one at its new point, with ONE
//!    `SetToolpathParam` and ONE undo step. The inspector undo snapshot
//!    moves to the state after the act, so the flush on leaving the
//!    toolpath does not repeat the act. One undo gives the auto tabs back.
//! 2. `an_add_and_a_delete_are_one_step_each_p4_6`: an add and a delete
//!    are one undo step each, and each undo restores the points before it.
//! 3. `the_last_tab_of_a_loop_is_not_deleted_o4`: the delete of the last
//!    tab of a loop writes nothing, pushes no undo step and gives a notice
//!    that names the loop.
//! 4. `the_viewport_picks_and_draws_tabs_where_the_loop_is_drawn_p4_6`: on a
//!    flipped and turned setup the marks draw where the loop draws, a click
//!    on a mark picks that mark, and a click on the drawn loop gives the
//!    model point of the loop under the pointer (the point an add stores).
//!
//! ```text
//! cargo test -p rs_cam_viz -q --test the_tab_editor_writes_one_step_per_act_p4_6
//! ```

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::PathBuf;
use std::sync::Arc;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::ProfileConfig;
use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits, StockConfig};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::compute::transform::{FaceUp, ZRotation};
use rs_cam_core::dressup::tabs::TabSource;
use rs_cam_core::geo::{P2, P3};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::selection::{Item, ItemSet, Shape, SourceRef};
use rs_cam_core::session::{
    Command, LoadedModel, ProjectSessionBuilder, SetSetupFaceArgs, SetSetupRotationArgs, TabEdit,
    TabLayoutAnswer, ToolpathConfig,
};
use rs_cam_viz::compute::{
    CollisionRequest, ComputeBackend, ComputeLane, ComputeMessage, ComputeRequest,
    GenerationControl, LaneSnapshot, SimulationRequest, ToolpathSubmitOutcome,
};
use rs_cam_viz::controller::{AppController, Severity};
use rs_cam_viz::interaction::picking::{
    PickContext, drawing_plane_z, pick_tab, pick_tab_loop, tab_scene,
};
use rs_cam_viz::render::camera::OrbitCamera;
use rs_cam_viz::state::Workspace;
use rs_cam_viz::state::job::SetupFrame;
use rs_cam_viz::state::selection::Selection as UiSelection;
use rs_cam_viz::state::toolpath::ToolpathId;
use rs_cam_viz::ui::AppEvent;

const VIEW_W: f32 = 800.0;
const VIEW_H: f32 = 600.0;
const EPS: f64 = 1e-9;

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

/// The loop: a 70 x 40 mm rectangle. Each side holds an auto tab.
const RECT: [(f64, f64); 4] = [(10.0, 10.0), (80.0, 10.0), (80.0, 50.0), (10.0, 50.0)];

fn drawing() -> ItemSet {
    let ring = Polygon2::new(RECT.iter().map(|&(x, y)| P2::new(x, y)).collect());
    ItemSet::new(vec![
        Item::new(
            SourceRef::ToolpathModel,
            Some("CUT".into()),
            Shape::Loop(ring),
        )
        .unwrap(),
    ])
}

fn toolpath(tab_points: Vec<[f64; 2]>) -> ToolpathConfig {
    ToolpathConfig {
        shoe: Default::default(),
        id: rs_cam_core::ToolpathId(0),
        name: "Profile".to_owned(),
        enabled: true,
        operation: OperationConfig::Profile(ProfileConfig {
            tab_count: 2,
            tab_points,
            ..ProfileConfig::default()
        }),
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

/// A controller with the rectangle and one Profile, the Toolpaths
/// workspace, the Profile selected and its tab editor on.
fn controller(
    tab_points: Vec<[f64; 2]>,
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
    let mut tool = ToolConfig::new_default(ToolId(1), ToolType::EndMill);
    tool.diameter = 6.0;
    let mut builder = ProjectSessionBuilder::new().stock(stock).tool(tool);
    let model_id = builder.add_model(LoadedModel {
        id: 0,
        name: "rect.dxf".to_owned(),
        mesh: None,
        items: Arc::new(drawing()),
        path: PathBuf::from("rect.dxf"),
        kind: Some(ModelKind::Dxf),
        units: Some(ModelUnits::Millimeters),
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
    });
    let mut tc = toolpath(tab_points);
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
    state.gui.selection_ui.set_tab_edit(id, true);
    assert!(rs_cam_viz::state::tab_edit::refresh_layout(state));
    c
}

fn id(c: &AppController<SilentBackend>) -> ToolpathId {
    c.state().session.toolpath_configs()[0].id
}

fn points(c: &AppController<SilentBackend>) -> Vec<[f64; 2]> {
    match &c.state().session.toolpath_configs()[0].operation {
        OperationConfig::Profile(cfg) => cfg.tab_points.clone(),
        other => panic!("not a Profile: {other:?}"),
    }
}

/// The layout the editor shows, read again first (as the frame does).
fn layout(c: &mut AppController<SilentBackend>) -> TabLayoutAnswer {
    let _ = rs_cam_viz::state::tab_edit::refresh_layout(c.state_mut());
    let id = id(c);
    c.state()
        .gui
        .selection_ui
        .tab_edit(id)
        .and_then(|mode| mode.answer())
        .cloned()
        .expect("the editor has a layout")
}

/// The panel takes its undo snapshot on its first frame of the toolpath.
fn take_panel_snapshot(c: &mut AppController<SilentBackend>) {
    let tc = c.state().session.toolpath_configs()[0].clone();
    c.state_mut().history.toolpath_snapshot = Some((
        tc.id,
        tc.operation,
        tc.dressups,
        tc.boundary,
        tc.feeds_provenance,
    ));
}

fn edit(c: &mut AppController<SilentBackend>, edit: TabEdit) {
    let id = id(c);
    c.handle_internal_event(AppEvent::EditTabs {
        toolpath_id: id,
        edit,
    });
}

fn has(points: &[[f64; 2]], p: [f64; 2]) -> bool {
    points
        .iter()
        .any(|q| (q[0] - p[0]).abs() < EPS && (q[1] - p[1]).abs() < EPS)
}

#[test]
fn an_auto_tab_drag_writes_every_tab_of_its_loop_in_one_step_p4_6() {
    let mut c = controller(Vec::new(), FaceUp::Top, ZRotation::Deg0);
    let before = layout(&mut c);
    assert_eq!(before.marks.len(), 4, "one auto tab per side");
    assert!(before.marks.iter().all(|m| m.source == TabSource::Auto));
    take_panel_snapshot(&mut c);
    let steps = c.state().history.undo_len();

    let to = [30.0, 10.0];
    edit(&mut c, TabEdit::Move { mark: 0, to });
    let after = points(&c);
    assert_eq!(
        after.len(),
        4,
        "every tab of the loop is a hand tab: {after:?}"
    );
    assert!(has(&after, to), "the dragged tab is at {to:?}: {after:?}");
    for m in &before.marks[1..] {
        assert!(has(&after, m.on_loop), "the tab at {:?} stays", m.on_loop);
    }
    assert_eq!(c.state().history.undo_len(), steps + 1, "one undo step");
    // The snapshot moved to the state after the act.
    let snapshot = c.state().history.toolpath_snapshot.clone().unwrap();
    match snapshot.1 {
        OperationConfig::Profile(cfg) => assert_eq!(cfg.tab_points, after),
        other => panic!("not a Profile: {other:?}"),
    }
    let layout_after = layout(&mut c);
    assert!(
        layout_after
            .marks
            .iter()
            .all(|m| matches!(m.source, TabSource::Hand(_)))
    );

    c.handle_internal_event(AppEvent::Undo);
    assert!(points(&c).is_empty(), "one undo gives the auto tabs back");
    assert_eq!(c.state().history.undo_len(), steps);
}

#[test]
fn an_add_and_a_delete_are_one_step_each_p4_6() {
    let start = vec![[30.0, 10.0], [50.0, 50.0]];
    let mut c = controller(start.clone(), FaceUp::Top, ZRotation::Deg0);
    let answer = layout(&mut c);
    let steps = c.state().history.undo_len();

    edit(
        &mut c,
        TabEdit::Add {
            loop_key: answer.loops[0].clone(),
            at: [60.0, 10.0],
        },
    );
    let added = points(&c);
    assert_eq!(added.len(), 3);
    assert!(has(&added, [60.0, 10.0]));
    assert_eq!(c.state().history.undo_len(), steps + 1);

    let answer = layout(&mut c);
    let first = answer
        .marks
        .iter()
        .position(|m| m.source == TabSource::Hand(0))
        .unwrap();
    edit(&mut c, TabEdit::Delete { mark: first });
    let deleted = points(&c);
    assert_eq!(deleted, vec![[50.0, 50.0], [60.0, 10.0]]);
    assert_eq!(c.state().history.undo_len(), steps + 2);

    c.handle_internal_event(AppEvent::Undo);
    assert_eq!(points(&c), added, "the undo of the delete");
    c.handle_internal_event(AppEvent::Undo);
    assert_eq!(points(&c), start, "the undo of the add");
}

/// A panel edit made after the panel took its snapshot and before a tab
/// act is its own undo step, and the act is the next one: the snapshot
/// does not fold the two together.
#[test]
fn a_panel_edit_before_a_tab_act_keeps_its_own_step_p4_6() {
    let mut c = controller(
        vec![[30.0, 10.0], [50.0, 50.0]],
        FaceUp::Top,
        ZRotation::Deg0,
    );
    let answer = layout(&mut c);
    take_panel_snapshot(&mut c);
    let steps = c.state().history.undo_len();
    let _ = c
        .state_mut()
        .session
        .apply(Command::SetToolpathParam(
            rs_cam_core::session::SetToolpathParamArgs {
                index: 0,
                param: "tab_width".to_owned(),
                value: serde_json::json!(8.0),
            },
        ))
        .unwrap();
    let width = |c: &AppController<SilentBackend>| match &c.state().session.toolpath_configs()[0]
        .operation
    {
        OperationConfig::Profile(cfg) => cfg.tab_width,
        other => panic!("not a Profile: {other:?}"),
    };
    let width_before = 6.0;
    assert!((width(&c) - 8.0).abs() < EPS);
    edit(
        &mut c,
        TabEdit::Add {
            loop_key: answer.loops[0].clone(),
            at: [60.0, 10.0],
        },
    );
    assert_eq!(
        c.state().history.undo_len(),
        steps + 2,
        "the panel edit, then the act"
    );
    c.handle_internal_event(AppEvent::Undo);
    assert_eq!(points(&c).len(), 2, "the first undo takes the act back");
    assert!((width(&c) - 8.0).abs() < EPS, "and keeps the panel edit");
    c.handle_internal_event(AppEvent::Undo);
    assert!(
        (width(&c) - width_before).abs() < EPS,
        "the second undo takes the panel edit back"
    );
}

#[test]
fn the_last_tab_of_a_loop_is_not_deleted_o4() {
    let mut c = controller(vec![[30.0, 10.0]], FaceUp::Top, ZRotation::Deg0);
    let answer = layout(&mut c);
    assert_eq!(answer.marks.len(), 1);
    let steps = c.state().history.undo_len();
    edit(&mut c, TabEdit::Delete { mark: 0 });
    assert_eq!(points(&c), vec![[30.0, 10.0]], "nothing is written");
    assert_eq!(c.state().history.undo_len(), steps, "no undo step");
    let notice = c
        .notifications()
        .iter()
        .find(|n| n.severity == Severity::Warning)
        .expect("a notice");
    let name = rs_cam_core::selection::ItemRef::new(answer.loops[0].clone()).to_string();
    assert!(
        notice.message.contains(&name),
        "the notice names {name}: {}",
        notice.message
    );
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

/// Where the viewport draws the model point `p` on the screen: the setup
/// transform, computed here, on the drawing plane of the op.
fn drawn(c: &AppController<SilentBackend>, camera: &OrbitCamera, p: P2) -> [f32; 2] {
    let state = c.state();
    let stock = state.session.stock_config();
    let frame = SetupFrame::of(&state.session.list_setups()[0]);
    let tc = &state.session.toolpath_configs()[0];
    let z = drawing_plane_z(&state.session, tc, Some(&frame));
    let q = frame.transform_point(P3::new(p.x, p.y, 0.0), stock);
    camera
        .project_to_screen(
            [q.x as f32, q.y as f32, z as f32],
            VIEW_W / VIEW_H,
            VIEW_W,
            VIEW_H,
        )
        .expect("the point is in view")
}

fn ctx(camera: &OrbitCamera, at: [f32; 2]) -> PickContext<'_> {
    PickContext {
        camera,
        screen_x: at[0],
        screen_y: at[1],
        aspect: VIEW_W / VIEW_H,
        vw: VIEW_W,
        vh: VIEW_H,
    }
}

#[test]
fn the_viewport_picks_and_draws_tabs_where_the_loop_is_drawn_p4_6() {
    let mut c = controller(Vec::new(), FaceUp::Bottom, ZRotation::Deg90);
    let answer = layout(&mut c);
    let camera = camera(&c);
    let id = id(&c);
    let scene = {
        let state = c.state();
        tab_scene(
            &state.session,
            id,
            state.gui.selection_ui.tab_edit(id).unwrap(),
        )
        .unwrap()
    };
    assert_eq!(scene.tabs.len(), answer.marks.len());
    for (tab, mark) in scene.tabs.iter().zip(&answer.marks) {
        // The mark draws where the setup transform puts its drawing point.
        let want = drawn(&c, &camera, P2::new(mark.at[0], mark.at[1]));
        let got = camera
            .project_to_screen(
                [tab.at.x as f32, tab.at.y as f32, scene.z as f32],
                VIEW_W / VIEW_H,
                VIEW_W,
                VIEW_H,
            )
            .unwrap();
        assert!(
            (want[0] - got[0]).abs() < 0.5 && (want[1] - got[1]).abs() < 0.5,
            "the mark {tab:?} draws at {got:?}, the loop at {want:?}"
        );
        // A click on the mark picks it.
        assert_eq!(pick_tab(&ctx(&camera, want), &scene), Some(tab.mark));
    }
    // A click on the drawn bottom side, clear of every tab, gives the loop
    // and the model point under the pointer.
    let at = P2::new(18.0, 10.0);
    let click = drawn(&c, &camera, at);
    let (key, p) = pick_tab_loop(&ctx(&camera, click), &c.state().session, id, &answer.loops)
        .expect("the click is on the loop");
    assert_eq!(key, answer.loops[0]);
    assert!(
        (p[0] - at.x).abs() < 0.05 && (p[1] - at.y).abs() < 1e-6,
        "the add stores the model point {at:?}, not {p:?}"
    );
}
