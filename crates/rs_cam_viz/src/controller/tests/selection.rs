//! Selection cascade (E-sel): what a delete does to the selection.

use super::*;

#[test]
fn delete_selected_toolpath_clears_selection() {
    let mut controller = sample_controller();
    let tp_id = ToolpathId(0);
    controller.state.selection = Selection::Toolpath(tp_id);

    // Remove the selected toolpath
    controller.handle_internal_event(crate::ui::AppEvent::RemoveToolpath(tp_id));

    assert_eq!(
        controller.state.selection,
        Selection::None,
        "Selection should be cleared after deleting the selected toolpath"
    );
}

#[test]
fn delete_unselected_toolpath_preserves_selection() {
    let mut controller = sample_controller();

    // Add another toolpath
    controller.handle_internal_event(crate::ui::AppEvent::AddToolpath(
        crate::state::toolpath::OperationType::Pocket,
    ));
    let Selection::Toolpath(new_tp_id) = controller.state.selection else {
        panic!("Selection should be new toolpath");
    };

    // Select back to the original toolpath
    controller.state.selection = Selection::Toolpath(ToolpathId(0));

    // Delete the other toolpath
    controller.handle_internal_event(crate::ui::AppEvent::RemoveToolpath(new_tp_id));

    assert_eq!(
        controller.state.selection,
        Selection::Toolpath(ToolpathId(0)),
        "Selection should be preserved when a different toolpath is deleted"
    );
}

/// F-024 (third-site fix, 2026-05-25): regression test that the controller's
/// world `stock_bbox` helper respects `StockConfig::origin_{x,y,z}`.
///
/// Pre-fix (rounds 03/04 evidence): `build_simulation_groups` constructed the
/// world `stock_bbox` inline as `(0,0,0)..(stock.x, stock.y, stock.z)` —
/// dropping the origin. For AS001 (`origin_z=-12`) this sent a bbox of
/// `(0,0,0)..(100,100,12)` to the worker even though the actual world stock
/// spans `(-10,-10,-12)..(90,90,0)`. The F-024 viz-worker follow-up
/// (commit `1dd1aa7`) made the viz request forward
/// `local_stock_bbox = None` for identity setups so the core would fall back
/// to `request.stock_bbox` — but the fallback bbox itself was the same
/// broken bbox. Result: toolpath cuts at world Z=-2 still sat below every
/// dexel ray (which spanned the wrong Z=[0,12] instead of Z=[-12,0]),
/// `axial_engagement_mm` read the full ray length, and the deflection gate
/// stayed at ~374 µm — byte-identical to round-02.
///
/// Fix: route the world bbox construction through `ProjectSession::
/// stock_bbox()` (which delegates to `StockConfig::bbox()` and applies the
/// origin correctly). Exposed via the pure free function
/// `controller::events::simulation::build_world_stock_bbox` so this test can
/// exercise it without spinning up a full `AppController<B>`.
#[test]
fn build_world_stock_bbox_respects_stock_origin_f024() {
    use rs_cam_core::compute::stock_config::StockConfig;
    use rs_cam_core::geo::BoundingBox3;
    use rs_cam_core::material::{Material, WoodSpecies};
    let stock = StockConfig {
        x: 100.0,
        y: 100.0,
        z: 12.0,
        origin_x: -10.0,
        origin_y: -10.0,
        origin_z: -12.0,
        auto_from_model: false,
        material: Material::SolidWood {
            species: WoodSpecies::GenericHardwood,
        },
        ..StockConfig::default()
    };
    let session = ProjectSessionBuilder::new().stock(stock).build();

    let bbox: BoundingBox3 =
        crate::controller::events::simulation::build_world_stock_bbox(&session);

    assert!(
        (bbox.min.x - -10.0).abs() < 1e-9,
        "F-024: world stock bbox min.x must equal stock.origin_x; got {}",
        bbox.min.x
    );
    assert!(
        (bbox.min.y - -10.0).abs() < 1e-9,
        "F-024: world stock bbox min.y must equal stock.origin_y; got {}",
        bbox.min.y
    );
    assert!(
        (bbox.min.z - -12.0).abs() < 1e-9,
        "F-024: world stock bbox min.z must equal stock.origin_z (= -12 for \
         AS001); got {}. Pre-fix the controller built bbox.min.z = 0.0 and \
         the per-setup dexel grid spanned Z=[0, 12] instead of Z=[-12, 0], \
         so cuts at world Z=-2 sat below every ray and axial_engagement_mm \
         read the full stock height (12 mm) instead of the commanded DOC.",
        bbox.min.z
    );
    assert!(
        (bbox.max.x - 90.0).abs() < 1e-9,
        "F-024: world stock bbox max.x must equal origin_x + stock.x; got {}",
        bbox.max.x
    );
    assert!(
        (bbox.max.y - 90.0).abs() < 1e-9,
        "F-024: world stock bbox max.y must equal origin_y + stock.y; got {}",
        bbox.max.y
    );
    assert!(
        (bbox.max.z - 0.0).abs() < 1e-9,
        "F-024: world stock bbox max.z must equal origin_z + stock.z (= 0 for \
         AS001, stock top at world Z=0); got {}",
        bbox.max.z
    );
}

// ═══════════════════════════════════════════════════════════════════════
// P4.3: each edit of the Selection section undoes in one step
// ═══════════════════════════════════════════════════════════════════════

/// The P2.7 drawing, one tool, a Drill (toolpath 0) and a Pocket (toolpath
/// 1) whose boundary is a selection with no item.
fn selection_controller() -> AppController<ScriptedBackend> {
    use rs_cam_core::compute::catalog::OperationConfig;
    use rs_cam_core::compute::config::{BoundaryConfig, BoundaryContainment, BoundarySource};
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../rs_cam_core/tests/fixtures/selection_p2_7/layers.dxf");
    let model = rs_cam_core::io::load_model_file(
        &path,
        0,
        ModelKind::Dxf,
        ModelUnits::Millimeters,
        Default::default(),
    )
    .expect("the drawing loads");
    let mut builder =
        ProjectSessionBuilder::new().tool(ToolConfig::new_default(ToolId(1), ToolType::EndMill));
    let _ = builder.add_model(model);
    let config = |op: crate::state::toolpath::OperationType| ToolpathConfig {
        shoe: Default::default(),
        id: ToolpathId(0),
        name: op.label().to_owned(),
        enabled: true,
        operation: OperationConfig::new_default(op),
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
    };
    builder
        .add_toolpath(0, config(crate::state::toolpath::OperationType::Drill))
        .expect("the drill is addable");
    let mut pocket = config(crate::state::toolpath::OperationType::Pocket);
    pocket.boundary = BoundaryConfig {
        enabled: true,
        source: BoundarySource::Selection {
            selection: rs_cam_core::selection::Selection::empty(),
            holes: false,
        },
        containment: BoundaryContainment::Center,
        offset: 0.0,
    };
    builder
        .add_toolpath(0, pocket)
        .expect("the pocket is addable");
    let mut controller = AppController::with_backend(ScriptedBackend::new());
    controller.state.replace_session(builder.build());
    let ids: Vec<_> = controller
        .state
        .session()
        .toolpath_configs()
        .iter()
        .map(|tc| tc.id)
        .collect();
    for id in ids {
        controller
            .state
            .gui
            .toolpath_rt
            .insert(id, ToolpathRuntime::new(true));
    }
    controller
}

/// The selection `target` of toolpath `index`, as stored.
fn stored_selection(
    c: &AppController<ScriptedBackend>,
    index: usize,
    target: rs_cam_core::session::SelectionTarget,
) -> rs_cam_core::selection::Selection {
    c.state.session().toolpath_configs()[index]
        .selection_of(target)
        .expect("the target is a selection")
        .expect("the toolpath holds the selection")
        .clone()
}

/// One section edit inside one inspector undo snapshot, as the panel takes
/// it: the snapshot on the first frame, the edit event, the flush when the
/// operator leaves the toolpath.
fn edit_in_one_step(
    c: &mut AppController<ScriptedBackend>,
    index: usize,
    target: rs_cam_core::session::SelectionTarget,
    edit: rs_cam_core::selection::SelectionEdit,
) {
    let tc = c.state.session().toolpath_configs()[index].clone();
    c.state.history.toolpath_snapshot = Some((
        tc.id,
        tc.operation.clone(),
        tc.dressups.clone(),
        tc.boundary.clone(),
        tc.feeds_provenance.clone(),
    ));
    c.handle_internal_event(crate::ui::AppEvent::EditToolpathSelection {
        toolpath_id: tc.id,
        target,
        edit,
    });
    let (snap_id, old_op, old_dressups, old_boundary, old_provenance) =
        c.state.history.toolpath_snapshot.take().unwrap();
    let (_, now) = c
        .state
        .session()
        .find_toolpath_config_by_id(snap_id)
        .unwrap();
    let action = crate::state::history::UndoAction::ToolpathParamChange {
        tp_id: snap_id,
        old_op,
        new_op: now.operation.clone(),
        old_dressups,
        new_dressups: now.dressups.clone(),
        old_boundary,
        new_boundary: now.boundary.clone(),
        old_feeds_provenance: old_provenance,
        new_feeds_provenance: now.feeds_provenance.clone(),
    };
    c.state.history.push(action);
}

/// Geometry selection P4.3: the four edits of the Selection section
/// (`AddRule`, `RemoveRule`, `Reset`, `Replace`) on the op target of a
/// Drill and on the boundary target of a Pocket. Each edit lands, ONE undo
/// gives the selection before it back, and one redo puts the edit back.
#[test]
fn each_selection_section_edit_undoes_in_one_step_p4_3() {
    use rs_cam_core::selection::{Filter, Rule, Selection, SelectionEdit, SourceRef};
    use rs_cam_core::session::SelectionTarget;

    let rule = |layer: &str| Rule {
        source: SourceRef::ToolpathModel,
        filter: Filter {
            layers: vec![layer.to_owned()],
            ..Filter::default()
        },
    };
    let mut c = selection_controller();
    for (index, target) in [(0, SelectionTarget::Op), (1, SelectionTarget::Boundary)] {
        let edits = [
            SelectionEdit::AddRule(rule("HOLES")),
            SelectionEdit::Replace(Box::new(Selection::from_rules(vec![
                rule("CUT"),
                rule("KEEP"),
            ]))),
            SelectionEdit::RemoveRule(0),
            SelectionEdit::Reset,
        ];
        for edit in edits {
            let before = stored_selection(&c, index, target);
            edit_in_one_step(&mut c, index, target, edit.clone());
            let after = stored_selection(&c, index, target);
            assert_ne!(
                after, before,
                "{target:?} {edit:?}: the edit must land through the event"
            );
            c.handle_internal_event(crate::ui::AppEvent::Undo);
            assert_eq!(
                stored_selection(&c, index, target),
                before,
                "{target:?} {edit:?}: one undo must give the selection before the edit"
            );
            c.handle_internal_event(crate::ui::AppEvent::Redo);
            assert_eq!(
                stored_selection(&c, index, target),
                after,
                "{target:?} {edit:?}: one redo must put the edit back"
            );
        }
    }
}

// ── P4.7: an STL face click adds one pick ─────────────────────────────
//
// The fixture is the stepped mesh of the basis fork face-region test (the
// core fixture `tests/common/meshes.rs::stepped_top`, built again here: a
// 20 x 10 top at Z 5 round a 4 x 4 boss with its top at Z 8, and one
// wall), loaded as an STL (no BREP faces), under a Pocket.
//
// The steps are the steps of the viewport in pick mode:
//
// 1. The ray pick (`pick_items`) of the screen point over the Z 5 face
//    gives a `MeshFace` hit. On a cold cache `face_pick_key_at` refuses
//    `FacesNotReady`.
// 2. The faces sweep (`process_model_faces`) submits ONE
//    `build_model_faces` job and stamps `AppState::job_run`. The test runs
//    the work step and drains the answer, as the Job lane does; the sweep
//    then submits nothing more.
// 3. The click key names the Z 5 face; the `TogglePick` adds ONE pick, and
//    the report counts one face.
//
// Red proof: `face_pick_key_at` with the Z band test inverted names no
// face, so step 3 fails.

mod an_stl_face_click_p4_7 {
    use super::*;
    use rs_cam_core::compute::stock_config::StockConfig;
    use rs_cam_core::mesh::TriangleMesh;
    use rs_cam_core::selection::{ItemKind, RefusalCause};
    use rs_cam_core::session::{JobHandle, SelectionTarget};

    use crate::interaction::picking::{PickContext, PickHit, pick_items};
    use crate::render::camera::OrbitCamera;
    use crate::state::Workspace;
    use crate::state::job::SetupFrame;
    use crate::state::pick::{PickMode, held_kinds};

    const VIEW_W: f32 = 800.0;
    const VIEW_H: f32 = 600.0;

    /// The stepped mesh (see the module doc).
    fn stepped_top() -> TriangleMesh {
        let mut vertices = Vec::new();
        let mut triangles = Vec::new();
        let mut quad = |a: P3, b: P3, c: P3, d: P3| {
            let i = u32::try_from(vertices.len()).unwrap_or(0);
            vertices.extend([a, b, c, d]);
            triangles.push([i, i + 1, i + 2]);
            triangles.push([i, i + 2, i + 3]);
        };
        let p = P3::new;
        quad(p(0., 0., 5.), p(20., 0., 5.), p(20., 3., 5.), p(0., 3., 5.));
        quad(
            p(0., 7., 5.),
            p(20., 7., 5.),
            p(20., 10., 5.),
            p(0., 10., 5.),
        );
        quad(p(0., 3., 5.), p(8., 3., 5.), p(8., 7., 5.), p(0., 7., 5.));
        quad(
            p(12., 3., 5.),
            p(20., 3., 5.),
            p(20., 7., 5.),
            p(12., 7., 5.),
        );
        quad(p(8., 3., 8.), p(12., 3., 8.), p(12., 7., 8.), p(8., 7., 8.));
        quad(p(8., 3., 5.), p(12., 3., 5.), p(12., 3., 8.), p(8., 3., 8.));
        TriangleMesh::from_raw(vertices, triangles)
    }

    fn stl_controller() -> AppController<ScriptedBackend> {
        let stock = StockConfig {
            x: 20.0,
            y: 10.0,
            z: 8.0,
            origin_x: 0.0,
            origin_y: 0.0,
            origin_z: 0.0,
            auto_from_model: false,
            ..StockConfig::default()
        };
        let mut builder = ProjectSessionBuilder::new()
            .stock(stock)
            .tool(ToolConfig::new_default(ToolId(1), ToolType::EndMill));
        let model_id = builder.add_model(LoadedModel {
            id: 0,
            name: "stepped.stl".to_owned(),
            mesh: Some(Arc::new(stepped_top())),
            items: Default::default(),
            path: std::path::PathBuf::from("synthetic://stepped.stl"),
            kind: Some(ModelKind::Stl),
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
            id: ToolpathId(0),
            name: "Pocket".to_owned(),
            enabled: true,
            operation: OperationConfig::new_default(crate::state::toolpath::OperationType::Pocket),
            dressups: Default::default(),
            heights: Default::default(),
            tool_id: 1,
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
        let _ = builder.add_toolpath(0, tc).expect("the pocket is addable");
        let mut controller = AppController::with_backend(ScriptedBackend::new());
        controller.state.replace_session(builder.build());
        let id = controller.state.session().toolpath_configs()[0].id;
        controller
            .state
            .gui
            .toolpath_rt
            .insert(id, ToolpathRuntime::new(false));
        controller.state.workspace = Workspace::Toolpaths;
        controller.state.selection = Selection::Toolpath(id);
        controller
    }

    /// The pick-mode hit of the screen point over the model point `p`.
    fn hit_over(controller: &AppController<ScriptedBackend>, mode: &PickMode, p: P3) -> PickHit {
        let state = &controller.state;
        let stock = state.session().stock_config().clone();
        let frame = SetupFrame::of(&state.session().list_setups()[0]);
        let (w, d, h) = frame.effective_stock(&stock);
        let mut camera = OrbitCamera::new();
        camera.fit_to_bounds([0.0, 0.0, 0.0], [w as f32, d as f32, h as f32]);
        let q = frame.transform_point(p, &stock);
        let at = camera
            .project_to_screen(
                [q.x as f32, q.y as f32, q.z as f32],
                VIEW_W / VIEW_H,
                VIEW_W,
                VIEW_H,
            )
            .expect("the point is in view");
        let tc = &state.session().toolpath_configs()[0];
        let held = held_kinds(&state.session().models()[0]);
        let kinds = mode
            .filter
            .effective_kinds(tc.operation.capabilities().item_use, &held);
        assert!(
            kinds.contains(&ItemKind::Face),
            "the face chip is on for an STL"
        );
        let ctx = PickContext {
            camera: &camera,
            screen_x: at[0],
            screen_y: at[1],
            aspect: VIEW_W / VIEW_H,
            vw: VIEW_W,
            vh: VIEW_H,
            visibility: &state.overlays.model_visibility,
        };
        pick_items(&ctx, state.session(), tc.id, &mode.filter, &kinds)
            .expect("the ray hits the mesh")
    }

    #[test]
    fn an_stl_face_click_adds_one_pick_p4_7() {
        let mut controller = stl_controller();
        let id = controller.state.session().toolpath_configs()[0].id;
        let mode = PickMode::new(
            id,
            SelectionTarget::Op,
            controller.state.session().toolpath_configs()[0]
                .operation
                .capabilities()
                .item_use,
        );
        controller
            .state
            .gui
            .selection_ui
            .set_pick(Some(mode.clone()));

        // 1. The ray hits the mesh; the faces are not computed yet.
        let PickHit::MeshFace { model_id, point } =
            hit_over(&controller, &mode, P3::new(2.0, 2.0, 5.0))
        else {
            panic!("pick mode on an STL gives a mesh hit");
        };
        assert!((point.z - 5.0).abs() < 1e-6, "the hit is on the Z 5 face");
        let tc = controller.state.session().toolpath_configs()[0].clone();
        let cold = controller
            .state
            .session()
            .face_pick_key_at(&tc, model_id.0, point)
            .unwrap_err();
        assert_eq!(
            cold.cause,
            RefusalCause::FacesNotReady {
                model: rs_cam_core::ids::ModelId(0)
            }
        );

        // 2. The sweep submits one faces job; the work step and the drain land it.
        controller.process_model_faces();
        assert_eq!(controller.compute.job_requests.len(), 1, "one faces job");
        assert!(controller.state.job_run.is_some(), "the job is the GUI job");
        let request = controller.compute.job_requests.remove(0);
        let JobHandle::BuildModelFaces(handle) = &request.handle else {
            panic!("the sweep submits the build_model_faces row");
        };
        let answer = rs_cam_core::session::execute_build_model_faces(handle, &request.cancel)
            .map(|a| rs_cam_core::session::JobAnswer::BuildModelFaces(Box::new(a)))
            .map_err(|e| crate::compute::ComputeError::Message(e.to_string()));
        controller
            .compute
            .drained
            .push(ComputeMessage::Job(Box::new(crate::compute::JobResult {
                id: request.id,
                answer,
            })));
        controller.drain_compute_results();
        assert!(
            controller.state.job_run.is_none(),
            "the drain clears the job"
        );
        controller.process_model_faces();
        assert!(
            controller.compute.job_requests.is_empty(),
            "a warm cache submits no second job"
        );

        // 3. The click names the Z 5 face and adds one pick.
        let key = controller
            .state
            .session()
            .face_pick_key_at(&tc, model_id.0, point)
            .expect("the hit names a face");
        controller.handle_internal_event(crate::ui::AppEvent::TogglePick {
            toolpath_id: id,
            target: SelectionTarget::Op,
            key,
        });
        let tc = controller.state.session().toolpath_configs()[0].clone();
        let picks = tc
            .operation
            .selection()
            .expect("a pocket holds a selection")
            .picks();
        assert_eq!(picks.len(), 1, "one click adds one pick: {picks:?}");
        assert_eq!(picks[0].kind, ItemKind::Face);
        assert!(
            (picks[0].measure - 184.0_f64.sqrt()).abs() < 1e-9,
            "the Z 5 face"
        );
        let report = controller
            .state
            .session()
            .selection_status(&tc, SelectionTarget::Op)
            .unwrap();
        assert_eq!(report.counts.get(&ItemKind::Face), Some(&1));
        assert_eq!(report.refusal, None);
    }
}
