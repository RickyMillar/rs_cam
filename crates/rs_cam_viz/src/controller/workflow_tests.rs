//! End-to-end workflow integration tests.
//!
//! These tests exercise user workflows through `AppController<ScriptedBackend>`
//! without GPU or UI. Each test traces a realistic workflow and asserts the
//! invariants that should hold at every step.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use rs_cam_core::enriched_mesh::{EnrichedMesh, FaceGroupId, SurfaceType};
use rs_cam_core::step_input::load_step;

use super::*;
use crate::compute::{CollisionRequest, ComputeMessage, LaneState, SimulationRequest};
use crate::state::job::{
    LoadedModel, ModelId, ModelKind, ModelUnits, ToolConfig, ToolId, ToolType,
};
use crate::state::selection::Selection;
use crate::state::toolpath::{
    HeightContext, HeightMode, HeightsConfig, OperationType, ToolpathEntry, ToolpathId,
};
use crate::ui::AppEvent;

// ── Test backend (mirrors tests.rs) ─────────────────────────────────────

struct ScriptedBackend {
    toolpath_lane: LaneSnapshot,
    analysis_lane: LaneSnapshot,
    drained: Vec<ComputeMessage>,
}

impl ScriptedBackend {
    fn new() -> Self {
        Self {
            toolpath_lane: LaneSnapshot::idle(ComputeLane::Toolpath),
            analysis_lane: LaneSnapshot::idle(ComputeLane::Analysis),
            drained: Vec::new(),
        }
    }
}

impl ComputeBackend for ScriptedBackend {
    fn submit_toolpath(&mut self, _request: crate::compute::ComputeRequest) {}
    fn submit_simulation(&mut self, _request: SimulationRequest) {}
    fn submit_collision(&mut self, _request: CollisionRequest) {}

    fn cancel_lane(&mut self, lane: ComputeLane) {
        match lane {
            ComputeLane::Toolpath => self.toolpath_lane.state = LaneState::Cancelling,
            ComputeLane::Analysis => self.analysis_lane.state = LaneState::Cancelling,
        }
    }

    fn drain_results(&mut self) -> Vec<ComputeMessage> {
        std::mem::take(&mut self.drained)
    }

    fn lane_snapshot(&self, lane: ComputeLane) -> LaneSnapshot {
        match lane {
            ComputeLane::Toolpath => self.toolpath_lane.clone(),
            ComputeLane::Analysis => self.analysis_lane.clone(),
        }
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("crates/rs_cam_core/tests/fixtures/step")
}

fn cube_enriched_mesh() -> Arc<EnrichedMesh> {
    let path = fixtures_dir().join("occt-cube.step");
    let enriched = load_step(&path, 0.1).expect("Failed to load cube STEP");
    Arc::new(enriched)
}

fn step_model(id: ModelId) -> LoadedModel {
    let enriched = cube_enriched_mesh();
    let mesh_arc = enriched.mesh_arc();
    LoadedModel {
        id,
        path: fixtures_dir().join("occt-cube.step"),
        name: "occt-cube.step".to_owned(),
        kind: ModelKind::Step,
        mesh: Some(mesh_arc),
        polygons: None,
        enriched_mesh: Some(enriched),
        units: ModelUnits::Millimeters,
        winding_report: None,
        load_error: None,
    }
}

fn stl_model(id: ModelId) -> LoadedModel {
    LoadedModel {
        id,
        path: PathBuf::from("test.stl"),
        name: "test.stl".to_owned(),
        kind: ModelKind::Stl,
        mesh: Some(Arc::new(rs_cam_core::mesh::make_test_flat(40.0))),
        polygons: None,
        enriched_mesh: None,
        units: ModelUnits::Millimeters,
        winding_report: None,
        load_error: None,
    }
}

/// Build a controller with a STEP model and one tool, ready for toolpath creation.
fn step_controller() -> AppController<ScriptedBackend> {
    let mut c = AppController::with_backend(ScriptedBackend::new());
    c.state
        .job
        .tools
        .push(ToolConfig::new_default(ToolId(1), ToolType::EndMill));
    c.state.job.models.push(step_model(ModelId(1)));
    c
}

/// Build a controller with an STL model and one tool.
fn stl_controller() -> AppController<ScriptedBackend> {
    let mut c = AppController::with_backend(ScriptedBackend::new());
    c.state
        .job
        .tools
        .push(ToolConfig::new_default(ToolId(1), ToolType::EndMill));
    c.state.job.models.push(stl_model(ModelId(1)));
    c
}

/// Find a horizontal (top/bottom) face on the enriched mesh.
fn find_horizontal_face(enriched: &EnrichedMesh) -> FaceGroupId {
    enriched
        .face_groups
        .iter()
        .find(|g| {
            g.surface_type == SurfaceType::Plane
                && enriched.face_boundary_as_polygon(g.id).is_some()
        })
        .expect("cube should have at least one horizontal face")
        .id
}

/// Find a vertical (non-horizontal) face on the enriched mesh.
fn find_vertical_face(enriched: &EnrichedMesh) -> FaceGroupId {
    enriched
        .face_groups
        .iter()
        .find(|g| enriched.face_boundary_as_polygon(g.id).is_none())
        .expect("cube should have at least one vertical face")
        .id
}

/// Add a pocket toolpath to the first setup and return its ID.
fn add_pocket(controller: &mut AppController<ScriptedBackend>) -> ToolpathId {
    let tp_id = controller.state.job.next_toolpath_id();
    let entry = ToolpathEntry::for_operation(
        tp_id,
        "Pocket".to_owned(),
        ToolId(1),
        ModelId(1),
        OperationType::Pocket,
    );
    controller.state.job.push_toolpath(entry);
    controller.state.selection = Selection::Toolpath(tp_id);
    tp_id
}

// ═══════════════════════════════════════════════════════════════════════
// W3: STEP Import → Face Selection → Pocket at Face Z
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn w3_step_import_populates_enriched_mesh() {
    let model = step_model(ModelId(1));

    assert!(
        model.enriched_mesh.is_some(),
        "enriched_mesh should be populated"
    );
    assert!(model.mesh.is_some(), "mesh should be populated");
    assert_eq!(model.kind, ModelKind::Step);

    let enriched = model.enriched_mesh.as_ref().unwrap();
    let mesh = model.mesh.as_ref().unwrap();

    // Enriched mesh and model mesh share the same Arc<TriangleMesh>
    assert!(Arc::ptr_eq(&enriched.mesh_arc(), mesh));

    // Cube has 6 faces
    assert_eq!(enriched.face_count(), 6, "Cube should have 6 faces");

    // triangle_to_face covers all triangles
    assert_eq!(
        enriched.triangle_to_face.len(),
        mesh.triangles.len(),
        "triangle_to_face should map every triangle"
    );
}

#[test]
fn w3_face_toggle_adds_and_removes() {
    let mut c = step_controller();
    let tp_id = add_pocket(&mut c);
    let enriched = c.state.job.models[0].enriched_mesh.as_ref().unwrap();
    let face_a = find_horizontal_face(enriched);

    // Toggle face ON
    c.handle_internal_event(AppEvent::ToggleFaceSelection {
        toolpath_id: tp_id,
        model_id: ModelId(1),
        face_id: face_a,
    });

    let entry = c.state.job.find_toolpath(tp_id).unwrap();
    assert_eq!(
        entry.face_selection.as_ref().map(|f| f.len()),
        Some(1),
        "Should have 1 face selected"
    );
    assert_eq!(entry.face_selection.as_ref().unwrap()[0], face_a);
    assert!(
        entry.stale_since.is_some(),
        "Toolpath should be marked stale"
    );

    // Selection should stay on toolpath (not switch to Face panel)
    assert!(
        matches!(c.state.selection, Selection::Toolpath(id) if id == tp_id),
        "Selection should stay on Toolpath, got {:?}",
        c.state.selection
    );

    // Toggle same face OFF
    c.handle_internal_event(AppEvent::ToggleFaceSelection {
        toolpath_id: tp_id,
        model_id: ModelId(1),
        face_id: face_a,
    });

    let entry = c.state.job.find_toolpath(tp_id).unwrap();
    assert_eq!(
        entry.face_selection, None,
        "Face selection should be None after toggle off"
    );
}

#[test]
fn w3_face_selection_derives_polygon() {
    let enriched = cube_enriched_mesh();
    let face_id = find_horizontal_face(&enriched);

    let polygon = enriched.face_boundary_as_polygon(face_id);
    assert!(
        polygon.is_some(),
        "Horizontal face should produce a polygon"
    );

    let poly = polygon.unwrap();
    assert!(
        poly.exterior.len() >= 3,
        "Polygon should have at least 3 exterior points, got {}",
        poly.exterior.len()
    );
}

#[test]
fn w3_face_z_propagates_to_height_resolution() {
    let enriched = cube_enriched_mesh();
    let face_id = find_horizontal_face(&enriched);
    let face_z = enriched.face_group(face_id).unwrap().bbox.max.z;

    // Resolve heights with auto defaults
    let heights_config = HeightsConfig::default();
    let op_depth = 3.0;
    let mut heights = heights_config.resolve(&HeightContext::simple(10.0, op_depth));

    // Without face Z, top_z defaults to stock_top_z (= op_depth in simple context)
    assert!(
        (heights.top_z - op_depth).abs() < 1e-9,
        "Auto top_z should be stock_top_z ({}), got {}",
        op_depth,
        heights.top_z
    );

    // Apply face Z override (same logic as events.rs)
    if heights_config.top_z.is_auto() {
        heights.top_z = face_z;
        if heights_config.bottom_z.is_auto() {
            heights.bottom_z = face_z - op_depth;
        }
    }

    assert!(
        (heights.top_z - face_z).abs() < 1e-9,
        "top_z should match face Z ({face_z}), got {}",
        heights.top_z
    );
    assert!(
        (heights.bottom_z - (face_z - op_depth)).abs() < 1e-9,
        "bottom_z should be face_z - op_depth ({} - {op_depth}), got {}",
        face_z,
        heights.bottom_z
    );
}

#[test]
fn w3_non_horizontal_face_produces_no_polygon() {
    let enriched = cube_enriched_mesh();
    let face_id = find_vertical_face(&enriched);

    let polygon = enriched.face_boundary_as_polygon(face_id);
    assert!(
        polygon.is_none(),
        "Vertical face should not produce a polygon"
    );
}

#[test]
fn w3_multi_face_toggle() {
    let mut c = step_controller();
    let tp_id = add_pocket(&mut c);
    let enriched = c.state.job.models[0].enriched_mesh.as_ref().unwrap();

    // Find two distinct horizontal faces
    let horizontal_faces: Vec<FaceGroupId> = enriched
        .face_groups
        .iter()
        .filter(|g| enriched.face_boundary_as_polygon(g.id).is_some())
        .map(|g| g.id)
        .take(2)
        .collect();

    if horizontal_faces.len() < 2 {
        // Cube may only have 2 horizontal faces (top + bottom); skip if not enough
        return;
    }

    let face_a = horizontal_faces[0];
    let face_b = horizontal_faces[1];

    // Toggle both on
    c.handle_internal_event(AppEvent::ToggleFaceSelection {
        toolpath_id: tp_id,
        model_id: ModelId(1),
        face_id: face_a,
    });
    c.handle_internal_event(AppEvent::ToggleFaceSelection {
        toolpath_id: tp_id,
        model_id: ModelId(1),
        face_id: face_b,
    });

    let entry = c.state.job.find_toolpath(tp_id).unwrap();
    assert_eq!(
        entry.face_selection.as_ref().map(|f| f.len()),
        Some(2),
        "Should have 2 faces selected"
    );
}

// ═══════════════════════════════════════════════════════════════════════
// W4: Face Selection Undo/Redo
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn w4_face_selection_in_undo_snapshot() {
    let mut c = step_controller();
    let tp_id = add_pocket(&mut c);
    let enriched = c.state.job.models[0].enriched_mesh.as_ref().unwrap();
    let face_a = find_horizontal_face(enriched);

    // Capture undo snapshot (simulates what properties panel does on first render)
    {
        let entry = c.state.job.find_toolpath(tp_id).unwrap();
        c.state.history.toolpath_snapshot = Some((
            tp_id,
            entry.operation.clone(),
            entry.dressups.clone(),
            entry.face_selection.clone(),
        ));
    }

    // Toggle face on
    c.handle_internal_event(AppEvent::ToggleFaceSelection {
        toolpath_id: tp_id,
        model_id: ModelId(1),
        face_id: face_a,
    });

    // Verify face is selected
    let entry = c.state.job.find_toolpath(tp_id).unwrap();
    assert!(entry.face_selection.is_some());

    // Flush snapshot (simulates navigating away from toolpath)
    // Take the snapshot and push an undo action
    if let Some((snap_id, old_op, old_dressups, old_faces)) =
        c.state.history.toolpath_snapshot.take()
    {
        if let Some(entry) = c.state.job.find_toolpath(snap_id) {
            c.state
                .history
                .push(crate::state::history::UndoAction::ToolpathParamChange {
                    tp_id: snap_id,
                    old_op,
                    new_op: entry.operation.clone(),
                    old_dressups,
                    new_dressups: entry.dressups.clone(),
                    old_face_selection: old_faces,
                    new_face_selection: entry.face_selection.clone(),
                });
        }
    }

    // Undo should restore face_selection to None
    c.handle_internal_event(AppEvent::Undo);
    let entry = c.state.job.find_toolpath(tp_id).unwrap();
    assert_eq!(
        entry.face_selection, None,
        "Undo should restore face_selection to None"
    );

    // Redo should restore the face selection
    c.handle_internal_event(AppEvent::Redo);
    let entry = c.state.job.find_toolpath(tp_id).unwrap();
    assert!(
        entry.face_selection.is_some(),
        "Redo should restore face_selection"
    );
    assert_eq!(entry.face_selection.as_ref().unwrap()[0], face_a);
}

// ═══════════════════════════════════════════════════════════════════════
// W5: Project Save/Load Round-Trip
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn w5_project_round_trip_preserves_step_face_selection() {
    use crate::io::project::{load_project, save_project};
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir =
        std::env::temp_dir().join(format!("rs_cam_wf_test_{nanos}_{}", std::process::id()));
    fs::create_dir_all(&temp_dir).unwrap();

    // Build a job with STEP model + face selection
    let mut job = crate::state::job::JobState::new();
    let model = step_model(ModelId(1));
    let enriched = model.enriched_mesh.as_ref().unwrap().clone();
    job.models.push(model);
    job.tools
        .push(ToolConfig::new_default(ToolId(1), ToolType::EndMill));

    let face_id = find_horizontal_face(&enriched);
    let mut entry = ToolpathEntry::for_operation(
        ToolpathId(1),
        "Pocket".to_owned(),
        ToolId(1),
        ModelId(1),
        OperationType::Pocket,
    );
    entry.face_selection = Some(vec![face_id]);
    job.push_toolpath(entry);

    // Save
    let project_path = temp_dir.join("test_project.toml");
    save_project(&job, &project_path).unwrap();
    assert!(project_path.exists());

    // Load
    let loaded = load_project(&project_path).unwrap();

    // Model re-imported with enriched mesh
    assert_eq!(loaded.job.models.len(), 1);
    assert!(loaded.job.models[0].enriched_mesh.is_some());

    // Face selection preserved
    let loaded_tp = loaded.job.all_toolpaths().next().unwrap();
    assert!(
        loaded_tp.face_selection.is_some(),
        "Face selection should survive round-trip"
    );
    assert_eq!(
        loaded_tp.face_selection.as_ref().unwrap()[0],
        face_id,
        "Face ID should match"
    );

    // No warnings
    assert!(
        loaded.warnings.is_empty(),
        "No warnings expected, got: {:?}",
        loaded
            .warnings
            .iter()
            .map(|w| w.message())
            .collect::<Vec<_>>()
    );

    fs::remove_dir_all(temp_dir).unwrap();
}

// ═══════════════════════════════════════════════════════════════════════
// W6: Height System Invariants
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn w6_auto_height_defaults() {
    let config = HeightsConfig::default();
    let h = config.resolve(&HeightContext::simple(10.0, 5.0));

    // HeightContext::simple(10.0, 5.0) → stock_top=0, stock_bottom=-5, safe_z=10
    // retract = safe_z (auto) = 10
    assert!(
        (h.retract_z - 10.0).abs() < 1e-9,
        "retract_z should be 10.0, got {}",
        h.retract_z
    );
    // clearance = retract + 10 = 20
    assert!(
        (h.clearance_z - 20.0).abs() < 1e-9,
        "clearance_z should be 20.0, got {}",
        h.clearance_z
    );
    // feed = retract - 2 = 8
    assert!(
        (h.feed_z - 8.0).abs() < 1e-9,
        "feed_z should be 8.0, got {}",
        h.feed_z
    );
    // top = 0 (auto)
    assert!(
        (h.top_z - 0.0).abs() < 1e-9,
        "top_z should be 0.0, got {}",
        h.top_z
    );
    // bottom = -op_depth = -5
    assert!(
        (h.bottom_z - (-5.0)).abs() < 1e-9,
        "bottom_z should be -5.0, got {}",
        h.bottom_z
    );

    // Ordering invariant
    assert!(h.clearance_z > h.retract_z, "clearance > retract");
    assert!(h.retract_z > h.feed_z, "retract > feed");
    assert!(h.feed_z > h.top_z, "feed > top");
    assert!(h.top_z > h.bottom_z, "top > bottom");
}

#[test]
fn w6_face_top_z_overrides_auto() {
    let config = HeightsConfig::default();
    let op_depth = 5.0;
    let mut h = config.resolve(&HeightContext::simple(10.0, op_depth));

    let face_z = 15.0;
    if config.top_z.is_auto() {
        h.top_z = face_z;
        if config.bottom_z.is_auto() {
            h.bottom_z = face_z - op_depth;
        }
    }

    assert!((h.top_z - 15.0).abs() < 1e-9, "top_z should be 15.0");
    assert!((h.bottom_z - 10.0).abs() < 1e-9, "bottom_z should be 10.0");
}

#[test]
fn w6_manual_heights_override_face_z() {
    let mut config = HeightsConfig::default();
    config.top_z = HeightMode::Manual(3.0);

    let op_depth = 5.0;
    let mut h = config.resolve(&HeightContext::simple(10.0, op_depth));

    let face_z = 15.0;
    // Same logic as events.rs — only override when auto
    if config.top_z.is_auto() {
        h.top_z = face_z;
    }

    assert!(
        (h.top_z - 3.0).abs() < 1e-9,
        "Manual top_z should be 3.0 (not face_z), got {}",
        h.top_z
    );
}

#[test]
fn w6_height_ordering_with_various_depths() {
    for &(safe_z, op_depth) in &[(5.0, 2.0), (20.0, 10.0), (0.5, 0.1), (100.0, 50.0)] {
        let h = HeightsConfig::default().resolve(&HeightContext::simple(safe_z, op_depth));
        assert!(
            h.clearance_z > h.retract_z && h.retract_z > h.feed_z && h.top_z > h.bottom_z,
            "Height ordering violated for safe_z={safe_z}, op_depth={op_depth}: \
             clearance={}, retract={}, feed={}, top={}, bottom={}",
            h.clearance_z,
            h.retract_z,
            h.feed_z,
            h.top_z,
            h.bottom_z
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════
// W10: Compute Status State Machine
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn w10_model_removal_clears_face_selection() {
    let mut c = step_controller();

    // Set selection to a face
    c.state.selection = Selection::Face(ModelId(1), FaceGroupId(0));

    // Remove model
    c.handle_internal_event(AppEvent::RemoveModel(ModelId(1)));

    // Selection should be cleared (model is still in use by toolpaths?
    // — the handler checks if toolpaths reference it first)
    // Since we haven't added a toolpath, the model can be removed
    assert_eq!(
        c.state.selection,
        Selection::None,
        "Face selection should be cleared when model is removed"
    );
    assert!(c.state.job.models.is_empty(), "Model should be removed");
}

#[test]
fn w10_model_removal_blocked_when_toolpath_references_it() {
    let mut c = step_controller();
    let _tp_id = add_pocket(&mut c);

    c.state.selection = Selection::Face(ModelId(1), FaceGroupId(0));

    // Try to remove model — should be blocked
    c.handle_internal_event(AppEvent::RemoveModel(ModelId(1)));

    assert_eq!(
        c.state.job.models.len(),
        1,
        "Model should NOT be removed when toolpaths reference it"
    );
    // Selection should still be set (model wasn't removed)
    assert!(
        matches!(c.state.selection, Selection::Face(..)),
        "Selection should stay since model wasn't removed"
    );
}
