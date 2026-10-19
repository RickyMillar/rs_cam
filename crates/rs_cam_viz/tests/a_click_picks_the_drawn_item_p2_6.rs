//! Geometry selection P2.6 — a viewport click picks the item where the
//! viewport draws it, on every setup frame.
//!
//! # The defect
//!
//! The viewport draws the hole markers and the STEP faces in the frame of
//! the setup (`SetupFrame::transform_point`). The pick tested the hole
//! markers at the raw model XY on the world stock top, and cast the face
//! ray against the raw model mesh. On a flipped or a rotated setup a click
//! on a drawn marker picked another hole or none, and a click on a drawn
//! face picked the face on the other side of the part.
//!
//! # The claim
//!
//! 1. Holes, on a `Bottom` setup and on a `Deg90` setup: a click on the
//!    drawn marker of a hole hits that hole (`PickHit::Item` with its
//!    key), and the `TogglePick` event of that hit takes the hole out of
//!    the drill selection. The raw model point of the old pick is more
//!    than the pick threshold away on the screen, so the test tells the
//!    two frames apart.
//! 2. Faces, on a `Bottom` setup: a click on the drawn top face of the
//!    part hits the model face that the flip draws on top (the model's
//!    DOWN face), and the `TogglePick` of its key adds that face pick.
//!
//! ```text
//! cargo test -p rs_cam_viz -q --test a_click_picks_the_drawn_item_p2_6
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
use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits, StockConfig};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::compute::transform::{FaceUp, ZRotation};
use rs_cam_core::geo::{P2, P3};
use rs_cam_core::geometry::enriched_mesh::FaceGroupId;
use rs_cam_core::selection::{Item, ItemSet, MatchKey, Shape, SourceRef};
use rs_cam_core::session::{
    Command, LoadedModel, ProjectSessionBuilder, SetSetupFaceArgs, SetSetupRotationArgs,
    ToolpathConfig,
};
use rs_cam_viz::compute::{
    CollisionRequest, ComputeBackend, ComputeLane, ComputeMessage, ComputeRequest,
    GenerationControl, LaneSnapshot, SimulationRequest, ToolpathSubmitOutcome,
};
use rs_cam_viz::controller::AppController;
use rs_cam_viz::interaction::picking::{PickContext, PickHit, pick};
use rs_cam_viz::render::camera::OrbitCamera;
use rs_cam_viz::state::Workspace;
use rs_cam_viz::state::job::SetupFrame;
use rs_cam_viz::state::selection::Selection;
use rs_cam_viz::state::viewport::ToolpathDrawFilter;
use rs_cam_viz::ui::AppEvent;

/// The pick threshold of a point marker (`picking::PICK_THRESHOLD_POINT`).
const PICK_THRESHOLD_PX: f32 = 12.0;
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

fn toolpath(name: &str, operation: OperationConfig) -> ToolpathConfig {
    ToolpathConfig {
        shoe: Default::default(),
        id: rs_cam_core::ToolpathId(0),
        name: name.to_owned(),
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

fn model(name: &str, items: ItemSet) -> LoadedModel {
    LoadedModel {
        id: 0,
        name: name.to_owned(),
        mesh: None,
        items: Arc::new(items),
        path: PathBuf::from(name),
        kind: Some(ModelKind::Dxf),
        units: Some(ModelUnits::Millimeters),
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
    }
}

/// A controller with one drill op on a drawing of two circles, off every
/// axis of symmetry of the stock, on a setup of `face_up` and `turn`.
fn drill_controller(face_up: FaceUp, turn: ZRotation) -> AppController<SilentBackend> {
    let holes = [(20.0, 15.0, 6.0), (70.0, 50.0, 4.0)];
    let items = ItemSet::new(
        holes
            .iter()
            .map(|&(x, y, d)| {
                Item::new(
                    SourceRef::ToolpathModel,
                    Some("H".into()),
                    Shape::circle(P2::new(x, y), d),
                )
                .unwrap()
            })
            .collect(),
    );
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
    let model_id = builder.add_model(model("holes.dxf", items));
    let mut tc = toolpath("Drill", OperationConfig::Drill(Default::default()));
    tc.model_id = model_id;
    let _ = builder.add_toolpath(0, tc).unwrap();
    let mut c = AppController::with_backend(SilentBackend);
    c.state_mut().session = builder.build();
    orient(&mut c, face_up, turn);
    c
}

fn orient(c: &mut AppController<SilentBackend>, face_up: FaceUp, turn: ZRotation) {
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
    state.selection = Selection::Toolpath(id);
}

/// A camera that sees the whole displayed stock from above.
fn camera(frame: &SetupFrame, stock: &StockConfig) -> OrbitCamera {
    let (w, d, h) = frame.effective_stock(stock);
    let mut camera = OrbitCamera::new();
    camera.fit_to_bounds([0.0, 0.0, 0.0], [w as f32, d as f32, h as f32]);
    camera
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

/// The pick of a click at `at`, as the viewport runs it.
fn click(
    c: &AppController<SilentBackend>,
    camera: &OrbitCamera,
    at: [f32; 2],
    drill: Option<rs_cam_core::ToolpathId>,
) -> Option<PickHit> {
    let state = c.state();
    let ctx = PickContext {
        camera,
        screen_x: at[0],
        screen_y: at[1],
        aspect: VIEW_W / VIEW_H,
        vw: VIEW_W,
        vh: VIEW_H,
    };
    pick(
        &ctx,
        &state.session,
        &state.gui,
        &[],
        state.workspace,
        ToolpathDrawFilter::from_state(state),
        drill,
        rs_cam_viz::state::pin_view::active_display_setup(&state.session, &state.selection)
            .as_ref(),
    )
}

/// The key of a hole the drill selection holds.
fn is_selected(c: &AppController<SilentBackend>, key: &MatchKey) -> bool {
    let tc = &c.state().session.toolpath_configs()[0];
    let holes = c.state().session.hole_selection(tc).unwrap();
    holes
        .items
        .iter()
        .any(|i| i.selected && i.key.names_same_geometry(key))
}

fn a_click_on_a_drawn_hole_toggles_it(face_up: FaceUp, turn: ZRotation) {
    let mut c = drill_controller(face_up, turn);
    let stock = c.state().session.stock_config().clone();
    let frame = SetupFrame::of(&c.state().session.list_setups()[0]);
    assert_eq!((frame.face_up, frame.z_rotation), (face_up, turn));
    let camera = camera(&frame, &stock);
    let tp_id = c.state().session.toolpath_configs()[0].id;
    let tc = c.state().session.toolpath_configs()[0].clone();
    let items = c.state().session.hole_selection(&tc).unwrap().items;
    assert_eq!(items.len(), 2, "fixture: two holes");

    for item in &items {
        // Where the viewport draws the hole, computed here from the setup
        // transform, not from the helper the code shares.
        let drawn_xy = frame.transform_point(P3::new(item.xy.x, item.xy.y, 0.0), &stock);
        let drawn = P3::new(drawn_xy.x, drawn_xy.y, frame.effective_stock(&stock).2);
        let raw = P3::new(item.xy.x, item.xy.y, stock.origin_z + stock.z);
        let at = screen(&camera, drawn);
        let old = screen(&camera, raw);
        assert!(
            (old[0] - at[0]).hypot(old[1] - at[1]) > PICK_THRESHOLD_PX,
            "the test does not separate the two frames for {:?}",
            item.xy
        );

        let hit = click(&c, &camera, at, Some(tp_id));
        let Some(PickHit::Item { toolpath_id, key }) = hit else {
            panic!("a click on the drawn hole {:?} missed: {hit:?}", item.xy);
        };
        assert_eq!(toolpath_id, tp_id);
        assert!(
            key.names_same_geometry(&item.key),
            "the click on hole {:?} picked another item: {key:?}",
            item.xy
        );

        // The default rule holds the hole; the click takes it out.
        assert!(is_selected(&c, &key), "fixture: the default holds the hole");
        c.handle_internal_event(AppEvent::TogglePick {
            toolpath_id: tp_id,
            target: rs_cam_core::session::SelectionTarget::Op,
            key: key.clone(),
        });
        assert!(
            !is_selected(&c, &key),
            "the click on hole {:?} did not toggle it",
            item.xy
        );
        // A second click puts it back.
        c.handle_internal_event(AppEvent::TogglePick {
            toolpath_id: tp_id,
            target: rs_cam_core::session::SelectionTarget::Op,
            key: key.clone(),
        });
        assert!(
            is_selected(&c, &key),
            "the second click did not toggle back"
        );
    }
}

#[test]
fn a_click_on_a_drawn_hole_toggles_it_on_a_flipped_setup() {
    a_click_on_a_drawn_hole_toggles_it(FaceUp::Bottom, ZRotation::Deg0);
}

#[test]
fn a_click_on_a_drawn_hole_toggles_it_on_a_rotated_setup() {
    a_click_on_a_drawn_hole_toggles_it(FaceUp::Top, ZRotation::Deg90);
}

#[test]
fn a_click_on_a_drawn_face_picks_it_on_a_flipped_setup() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../rs_cam_core/tests/fixtures/step/occt-cube.step");
    let enriched = Arc::new(rs_cam_core::io::step_input::load_step(&path, 0.1).unwrap());
    let bbox = enriched.mesh.bbox;
    let items = rs_cam_core::io::brep_face_items(&enriched);
    let part = LoadedModel {
        mesh: Some(enriched.mesh_arc()),
        enriched_mesh: Some(Arc::clone(&enriched)),
        kind: Some(ModelKind::Step),
        ..model("occt-cube.step", items)
    };
    // The stock is the part box, so the flip maps the part onto itself.
    let stock = StockConfig {
        x: bbox.max.x - bbox.min.x,
        y: bbox.max.y - bbox.min.y,
        z: bbox.max.z - bbox.min.z,
        origin_x: bbox.min.x,
        origin_y: bbox.min.y,
        origin_z: bbox.min.z,
        auto_from_model: false,
        ..StockConfig::default()
    };
    let mut builder = ProjectSessionBuilder::new()
        .stock(stock.clone())
        .tool(ToolConfig::new_default(ToolId(1), ToolType::EndMill));
    let model_id = builder.add_model(part);
    let mut tc = toolpath("Pocket", OperationConfig::Pocket(Default::default()));
    tc.model_id = model_id;
    let _ = builder.add_toolpath(0, tc).unwrap();
    let mut c = AppController::with_backend(SilentBackend);
    c.state_mut().session = builder.build();
    orient(&mut c, FaceUp::Bottom, ZRotation::Deg0);

    // The model's up and down faces (flat, horizontal).
    let face_with = |want_up: bool| -> FaceGroupId {
        enriched
            .face_groups
            .iter()
            .find(|g| {
                let top = (g.bbox.min.z - bbox.max.z).abs() < 1e-6;
                let bottom = (g.bbox.max.z - bbox.min.z).abs() < 1e-6;
                if want_up { top } else { bottom }
            })
            .expect("the cube has a top and a bottom face")
            .id
    };
    let model_up = face_with(true);
    let model_down = face_with(false);

    // Click the centre of the drawn top of the flipped part.
    let frame = SetupFrame::of(&c.state().session.list_setups()[0]);
    let camera = camera(&frame, &stock);
    let centre = P3::new(
        (bbox.min.x + bbox.max.x) / 2.0,
        (bbox.min.y + bbox.max.y) / 2.0,
        bbox.min.z,
    );
    let drawn = frame.transform_point(centre, &stock);
    assert!(
        (drawn.z - frame.effective_stock(&stock).2).abs() < 1e-6,
        "fixture: the flip draws the model's down face on top"
    );
    let hit = click(&c, &camera, screen(&camera, drawn), None);
    let Some(PickHit::ModelFace { face_id, .. }) = hit else {
        panic!("a click on the drawn top face missed: {hit:?}");
    };
    assert_ne!(
        face_id, model_up,
        "the click picked the model's up face, which the flip draws underneath"
    );
    assert_eq!(face_id, model_down, "the click picked the wrong face");

    // The face click sends the key of that face as a pick.
    let tp_id = c.state().session.toolpath_configs()[0].id;
    let key = {
        let session = &c.state().session;
        session
            .face_pick_key(&session.toolpath_configs()[0], model_id, face_id)
            .unwrap()
    };
    c.handle_internal_event(AppEvent::TogglePick {
        toolpath_id: tp_id,
        target: rs_cam_core::session::SelectionTarget::Op,
        key,
    });
    let picks: Vec<u16> = c.state().session.toolpath_configs()[0]
        .operation
        .selection()
        .unwrap()
        .face_picks()
        .filter_map(|k| k.brep)
        .collect();
    assert_eq!(picks, vec![model_down.0], "the face pick did not land");
}
