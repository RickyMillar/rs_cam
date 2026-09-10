//! **G-RESCALESTALE** (F4.7) — a model refresh that replaces the geometry
//! must drop the results generated against the previous geometry.
//!
//! # The defect
//!
//! `AppController::rescale_model`
//! (`crates/rs_cam_viz/src/controller/io.rs`) re-imports the model at new
//! declared units and replaces its geometry, so every polygon moves by the
//! unit scale — 25.4× from millimetres to inches. It then invalidated
//! NOTHING. No `invalidate_model`, no stale sweep. Every dependent toolpath
//! result stayed cached and every card stayed green, over geometry that had
//! moved by 25.4×, and export emitted those cached paths.
//!
//! `reload_model` and `relink_model` both call
//! `ProjectSession::invalidate_model` and then stamp `stale_since` on each
//! affected toolpath's runtime entry. `rescale_model` is the odd one out.
//! `research/R0.7.md` §3.3 asked for this; `reports/F4.4.md` §6 declined it
//! deliberately, because it is a behaviour change and a different defect
//! from the field split that lane fixed.
//!
//! # What this file pins, and how it is generic
//!
//! The rule is per DOOR, not per fixture: **a door that replaces a model's
//! geometry drops the results of the toolpaths bound to that model, and
//! only those.** `a_reload_drops_the_dependent_results_control` runs the
//! identical assertion against `reload_model` — it passes before the fix and
//! after it, and it is the reason the rescale arm reads as a defect rather
//! than as a broken assertion. `a_step_rescale_changes_nothing_and_drops_
//! nothing` pins the other side: `rescale_model` returns early for
//! `ModelKind::Step` BEFORE it imports anything, so no geometry moves and
//! there is nothing to invalidate.
//!
//! The second toolpath, bound to a DIFFERENT model, is what distinguishes
//! `invalidate_model` from `drop_all_results`.
//!
//! # Why the fixture is an SVG
//!
//! An STL would let `rescale_model`'s `auto_from_model` branch call
//! `stock_mut().update_from_bbox`. That is a different mechanism, and a
//! mesh fixture could make this file pass for a reason it is not testing.
//! An SVG carries no mesh, so `stock_bbox_update` stays `None` and the only
//! thing under test is the door's own invalidation.
//!
//! Source: `planning/ui_fix_2026-09-09/reports/F4.4.md` §7 row F4.7, and
//! `research/R0.7.md` §3.3 (the "Reload / Rescale" row).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::config::ToolpathStats;
use rs_cam_core::compute::stock_config::{ModelId, ModelKind, ModelUnits};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::drill_op::OpData;
use rs_cam_core::session::{LoadedModel, ToolpathComputeResult, ToolpathConfig};
use rs_cam_core::toolpath::Toolpath;
use rs_cam_core::toolpath_spans::AnnotatedToolpath;
use rs_cam_viz::compute::{
    CollisionRequest, ComputeBackend, ComputeLane, ComputeMessage, ComputeRequest,
    GenerationControl, LaneSnapshot, OptimizeRequest, SimulationRequest, ToolpathSubmitOutcome,
};
use rs_cam_viz::controller::AppController;

// ── harness ─────────────────────────────────────────────────────────────

struct SilentBackend;

impl ComputeBackend for SilentBackend {
    fn submit_toolpath(&mut self, _request: ComputeRequest) -> ToolpathSubmitOutcome {
        ToolpathSubmitOutcome::Queued
    }
    fn submit_simulation(&mut self, _request: SimulationRequest) {}
    fn submit_collision(&mut self, _request: CollisionRequest) {}
    fn submit_optimize(&mut self, _request: OptimizeRequest) {}
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

/// A unique scratch directory for one test.
fn scratch(name: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock before epoch")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("rs_cam_rescalestale_{name}_{nanos}"));
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    dir
}

fn drawing(width: f64) -> String {
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"200\" height=\"120\">\
         <rect x=\"10\" y=\"10\" width=\"{width}\" height=\"60\"/></svg>"
    )
}

fn write(path: &Path, body: &str) {
    std::fs::write(path, body).expect("write drawing");
}

/// A result with no geometry. Its CONTENT is irrelevant: this file asks
/// whether the cache entry survives a door, never what is in it.
fn stub_result() -> ToolpathComputeResult {
    ToolpathComputeResult {
        op_data: OpData::Toolpath(Arc::new(AnnotatedToolpath::new(Toolpath::new()))),
        stats: ToolpathStats::default(),
        debug_trace: None,
        semantic_trace: None,
    }
}

fn toolpath(name: &str, model_id: usize) -> ToolpathConfig {
    ToolpathConfig {
        id: rs_cam_core::ToolpathId(0),
        name: name.to_owned(),
        enabled: true,
        operation: OperationConfig::new_default(OperationType::Pocket),
        dressups: Default::default(),
        heights: Default::default(),
        tool_id: 1,
        model_id,
        pre_gcode: None,
        post_gcode: None,
        boundary: Default::default(),
        boundary_inherit: true,
        rest_analysis: Default::default(),
        stock_source: Default::default(),
        coolant: Default::default(),
        face_selection: None,
        debug_options: Default::default(),
        feeds_provenance: Default::default(),
        planner_origin: None,
    }
}

/// The whole fixture: two SVG models, one toolpath on each, a cached result
/// on both.
///
/// Returns the controller, the id of the model under test, and the path it
/// was imported from.
fn seeded(dir: &Path) -> (AppController<SilentBackend>, ModelId, PathBuf) {
    let under_test = dir.join("under_test.svg");
    let bystander = dir.join("bystander.svg");
    write(&under_test, &drawing(80.0));
    write(&bystander, &drawing(40.0));

    let mut controller = AppController::with_backend(SilentBackend);
    controller
        .state
        .session
        .tools_mut()
        .push(ToolConfig::new_default(ToolId(1), ToolType::EndMill));
    controller
        .import_svg_path(&under_test)
        .expect("the fixture must import");
    controller
        .import_svg_path(&bystander)
        .expect("the bystander must import");

    let id_under_test = model_id_of(&controller, &under_test);
    let id_bystander = model_id_of(&controller, &bystander);

    let session = &mut controller.state.session;
    session
        .add_toolpath(0, toolpath("Under test", id_under_test.0))
        .expect("add the dependent toolpath");
    session
        .add_toolpath(0, toolpath("Bystander", id_bystander.0))
        .expect("add the bystander toolpath");
    session.insert_result(0, stub_result()).expect("cache 0");
    session.insert_result(1, stub_result()).expect("cache 1");

    (controller, id_under_test, under_test)
}

fn model_id_of(controller: &AppController<SilentBackend>, path: &Path) -> ModelId {
    let id = controller
        .state
        .session
        .models()
        .iter()
        .find(|m| m.path == path)
        .expect("the imported fixture must be in the session")
        .id;
    ModelId(id)
}

/// Is the runtime entry for toolpath slot `idx` marked stale?
///
/// Read through `and_then`: before the fix the entry may not exist at all,
/// and "absent" must read as "not stale" rather than panicking.
fn is_stale(controller: &AppController<SilentBackend>, idx: usize) -> bool {
    let id = controller.state.session.toolpath_configs()[idx].id;
    controller
        .state
        .gui
        .toolpath_rt
        .get(&id)
        .and_then(|rt| rt.stale_since)
        .is_some()
}

/// The model's bbox width, to micron resolution.
fn width_microns(controller: &AppController<SilentBackend>, id: ModelId) -> i64 {
    let bbox = controller
        .state
        .session
        .models()
        .iter()
        .find(|m| m.id == id.0)
        .expect("the model must still be in the session")
        .bbox()
        .expect("the fixture must carry geometry");
    ((bbox.max.x - bbox.min.x) * 1000.0).round() as i64
}

// ── non-vacuity ─────────────────────────────────────────────────────────

/// Without this every assertion below could be reading a cache that was
/// never populated, or a door that never moved anything.
#[test]
fn the_fixture_caches_two_results_and_the_rescale_moves_the_geometry() {
    let dir = scratch("vacuity");
    let (mut controller, id, _path) = seeded(&dir);

    assert!(
        controller.state.session.get_result(0).is_some(),
        "the dependent toolpath must start with a cached result"
    );
    assert!(
        controller.state.session.get_result(1).is_some(),
        "the bystander toolpath must start with a cached result"
    );
    assert!(
        !is_stale(&controller, 0),
        "the dependent toolpath must not start stale"
    );

    let before = width_microns(&controller, id);
    controller
        .rescale_model(id, ModelUnits::Inches)
        .expect("rescale must succeed");
    let after = width_microns(&controller, id);

    assert!(
        after > before * 10,
        "the rescale must MOVE the geometry, else there is nothing to \
         invalidate — width went {before} → {after} microns"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

// ── the defect ──────────────────────────────────────────────────────────

/// **The operator's case.** A declared-units change moves every polygon by
/// the unit scale. The cached results answer the previous size, and export
/// emits them.
#[test]
fn a_rescale_drops_the_dependent_results() {
    let dir = scratch("rescale");
    let (mut controller, id, _path) = seeded(&dir);

    controller
        .rescale_model(id, ModelUnits::Inches)
        .expect("rescale must succeed");

    assert!(
        controller.state.session.get_result(0).is_none(),
        "the rescale replaced the geometry this result was generated \
         against, so the result must be dropped. `rescale_model` calls no \
         `invalidate_model` and runs no stale sweep, so the card stays green \
         over geometry that moved by 25.4x and export emits the cached path."
    );
    assert!(
        is_stale(&controller, 0),
        "and the runtime entry must read stale, as `reload_model`'s own \
         sweep stamps it"
    );
    assert!(
        controller.state.session.get_result(1).is_some(),
        "the bystander is bound to a DIFFERENT model, so its result must \
         survive — this is `invalidate_model`, not `drop_all_results`"
    );
    assert!(
        !is_stale(&controller, 1),
        "and the bystander must not be marked stale"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

// ── the controls ────────────────────────────────────────────────────────

/// **The control.** The identical assertion against `reload_model`, which
/// already invalidates. It passes before the fix and after it, so the arm
/// above reads as a defect rather than as a broken assertion.
#[test]
fn a_reload_drops_the_dependent_results_control() {
    let dir = scratch("reload");
    let (mut controller, id, path) = seeded(&dir);

    write(&path, &drawing(120.0));
    controller.reload_model(id).expect("reload must succeed");

    assert!(
        controller.state.session.get_result(0).is_none(),
        "reload already drops the dependent result"
    );
    assert!(is_stale(&controller, 0), "and stamps it stale");
    assert!(
        controller.state.session.get_result(1).is_some(),
        "and leaves the bystander alone"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// **The other side of the rule.** `rescale_model` returns early for
/// `ModelKind::Step`, BEFORE it imports anything, so no geometry moves.
/// A door that changes nothing must invalidate nothing — otherwise the
/// invalidation is not tied to the geometry being replaced.
#[test]
fn a_step_rescale_changes_nothing_and_drops_nothing() {
    let mut controller = AppController::with_backend(SilentBackend);
    controller
        .state
        .session
        .tools_mut()
        .push(ToolConfig::new_default(ToolId(1), ToolType::EndMill));

    // The path is never read: the STEP arm returns before the import.
    let record = LoadedModel {
        id: 0,
        name: "step part".to_owned(),
        mesh: None,
        polygons: None,
        drill_targets: Arc::new(Vec::new()),
        layers: Arc::new(Vec::new()),
        path: PathBuf::from("synthetic://never_read.step"),
        kind: Some(ModelKind::Step),
        units: Some(ModelUnits::Millimeters),
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
    };
    let session = &mut controller.state.session;
    let model_id = session.add_model(record);
    session
        .add_toolpath(0, toolpath("On the STEP", model_id))
        .expect("add the dependent toolpath");
    session.insert_result(0, stub_result()).expect("cache 0");

    controller
        .rescale_model(ModelId(model_id), ModelUnits::Inches)
        .expect("a STEP rescale must not error");

    assert!(
        controller.state.session.get_result(0).is_some(),
        "the STEP arm returns before the import, so nothing moved and the \
         result must survive"
    );
    assert!(
        !is_stale(&controller, 0),
        "and nothing must be marked stale"
    );
}
