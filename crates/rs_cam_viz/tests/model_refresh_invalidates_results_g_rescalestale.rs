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

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::compute::toolpath_stats::ToolpathStats;
use rs_cam_core::ids::ModelId;
use rs_cam_core::ops::drill_op::OpData;
use rs_cam_core::session::{
    AddToolpathArgs, AdoptResultArgs, Command, LoadedModel, ProjectSessionBuilder,
    ToolpathComputeResult, ToolpathConfig,
};
use rs_cam_core::toolpath::Toolpath;
use rs_cam_core::trace::toolpath_spans::AnnotatedToolpath;
use rs_cam_viz::compute::{
    CollisionRequest, ComputeBackend, ComputeLane, ComputeMessage, ComputeRequest,
    GenerationControl, LaneSnapshot, SimulationRequest, ToolpathSubmitOutcome,
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
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"200mm\" height=\"120mm\" viewBox=\"0 0 200 120\">\
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
        provenance: rs_cam_core::session::ResultProvenance::Dressed,
    }
}

fn toolpath(name: &str, model_id: usize) -> ToolpathConfig {
    ToolpathConfig {
        shoe: Default::default(),
        id: rs_cam_core::ToolpathId::from_raw(0),
        name: name.to_owned(),
        enabled: true,
        operation: OperationConfig::new_default(OperationType::Pocket),
        dressups: Default::default(),
        heights: Default::default(),
        tool_id: rs_cam_core::ids::ToolId::from_raw(1),
        model_id: rs_cam_core::ids::ModelId::from_raw(model_id),
        pre_gcode: None,
        post_gcode: None,
        boundary: Default::default(),
        boundary_inherit: true,
        rest_analysis: Default::default(),
        stock_source: Default::default(),
        coolant: Default::default(),
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
    controller.state.replace_session(
        ProjectSessionBuilder::new()
            .tool(ToolConfig::new_default(
                ToolId::from_raw(1),
                ToolType::EndMill,
            ))
            .build(),
    );
    controller.open_import_dialog(
        &under_test,
        rs_cam_core::compute::stock_config::ModelKind::Svg,
    );
    controller
        .confirm_import_dialog()
        .expect("the fixture must import");
    controller.open_import_dialog(
        &bystander,
        rs_cam_core::compute::stock_config::ModelKind::Svg,
    );
    controller
        .confirm_import_dialog()
        .expect("the bystander must import");

    let id_under_test = model_id_of(&controller, &under_test);
    let id_bystander = model_id_of(&controller, &bystander);

    let state = &mut controller.state;
    let _ = state
        .apply(Command::AddToolpath(AddToolpathArgs {
            setup_index: 0,
            config: Box::new(toolpath("Under test", id_under_test.raw())),
        }))
        .expect("add the dependent toolpath");
    let _ = state
        .apply(Command::AddToolpath(AddToolpathArgs {
            setup_index: 0,
            config: Box::new(toolpath("Bystander", id_bystander.raw())),
        }))
        .expect("add the bystander toolpath");
    for index in [0usize, 1usize] {
        let revision = state.session().toolpath_revision_at(index);
        let _ = state
            .apply(Command::AdoptResult(AdoptResultArgs {
                index,
                revision,
                result: Box::new(stub_result()),
            }))
            .expect("cache the seeded result");
    }
    // F096: the fixture writes through `AppState::apply`, which stamps the
    // added rows stale. The fixture is a project at rest, so it clears the
    // stamps; the tests measure what the refresh door stamps.
    for rt in controller.state.gui.toolpath_rt.values_mut() {
        rt.stale_since = None;
    }

    (controller, id_under_test, under_test)
}

fn model_id_of(controller: &AppController<SilentBackend>, path: &Path) -> ModelId {
    controller
        .state
        .session()
        .models()
        .iter()
        .find(|m| m.path == path)
        .expect("the imported fixture must be in the session")
        .id
}

/// Is the runtime entry for toolpath slot `idx` marked stale?
///
/// Read through `and_then`: before the fix the entry may not exist at all,
/// and "absent" must read as "not stale" rather than panicking.
fn is_stale(controller: &AppController<SilentBackend>, idx: usize) -> bool {
    let id = controller.state.session().toolpath_configs()[idx].id;
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
        .session()
        .models()
        .iter()
        .find(|m| m.id == id)
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
        controller.state.session().result_at(0).is_some(),
        "the dependent toolpath must start with a cached result"
    );
    assert!(
        controller.state.session().result_at(1).is_some(),
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
        controller.state.session().result_at(0).is_none(),
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
        controller.state.session().result_at(1).is_some(),
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
        controller.state.session().result_at(0).is_none(),
        "reload already drops the dependent result"
    );
    assert!(is_stale(&controller, 0), "and stamps it stale");
    assert!(
        controller.state.session().result_at(1).is_some(),
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
    let tool = ToolConfig::new_default(ToolId::from_raw(1), ToolType::EndMill);
    let mut builder = ProjectSessionBuilder::new().tool(tool);

    // The path is never read: the STEP arm returns before the import.
    let record = LoadedModel {
        id: rs_cam_core::ids::ModelId::from_raw(0),
        name: "step part".to_owned(),
        mesh: None,
        items: Default::default(),
        path: PathBuf::from("synthetic://never_read.step"),
        kind: Some(ModelKind::Step),
        units: Some(ModelUnits::Millimeters),
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
        import_options: Default::default(),
        import_report: None,
        drawing: Default::default(),
    };
    let model_id = builder.add_model(record);
    let _ = builder
        .add_toolpath(0, toolpath("On the STEP", model_id.raw()))
        .expect("add the dependent toolpath");
    controller.state.replace_session(builder.build());
    let state = &mut controller.state;
    let revision = state.session().toolpath_revision_at(0);
    let _ = state
        .apply(Command::AdoptResult(AdoptResultArgs {
            index: 0,
            revision,
            result: Box::new(stub_result()),
        }))
        .expect("cache 0");

    controller
        .rescale_model(model_id, ModelUnits::Inches)
        .expect("a STEP rescale must not error");

    assert!(
        controller.state.session().result_at(0).is_some(),
        "the STEP arm returns before the import, so nothing moved and the \
         result must survive"
    );
    assert!(
        !is_stale(&controller, 0),
        "and nothing must be marked stale"
    );
}

// ── P4.8: the import options are a fourth refresh door ──────────────────

fn join(join_tolerance_mm: f64) -> rs_cam_core::io::ImportOptions {
    rs_cam_core::io::ImportOptions { join_tolerance_mm }
}

/// A new join tolerance reads the file again through the same refresh row
/// (`AdoptModelGeometry`), so it drops the results that read the model and
/// stamps them stale. The bystander keeps its result.
#[test]
fn new_import_options_drop_the_dependent_results_p4_8() {
    let dir = scratch("import_options");
    let (mut controller, id, _path) = seeded(&dir);

    controller
        .set_model_import_options(id, join(0.5))
        .expect("the options apply");

    let model = &controller.state.session().models()[0];
    assert_eq!(model.id, id);
    assert_eq!(
        model.import_options,
        join(0.5),
        "the record takes the options"
    );
    assert!(
        controller.state.session().result_at(0).is_none(),
        "the join can change the items, so the dependent result goes"
    );
    assert!(is_stale(&controller, 0), "and reads stale");
    assert!(
        controller.state.session().result_at(1).is_some(),
        "the bystander reads another model"
    );
    assert!(!is_stale(&controller, 1));
    let _ = std::fs::remove_dir_all(&dir);
}

/// The same options again change nothing and drop nothing.
#[test]
fn unchanged_import_options_drop_nothing_p4_8() {
    let dir = scratch("import_options_same");
    let (mut controller, id, _path) = seeded(&dir);
    controller
        .set_model_import_options(id, rs_cam_core::io::ImportOptions::default())
        .expect("a no-op applies");
    assert!(controller.state.session().result_at(0).is_some());
    assert!(!is_stale(&controller, 0));
    let _ = std::fs::remove_dir_all(&dir);
}

/// The other three refresh doors keep the record's options: a reload, a
/// rescale and a relink read the file with them, so none resets the
/// tolerance to the default.
#[test]
fn every_refresh_door_keeps_the_import_options_p4_8() {
    let dir = scratch("import_options_kept");
    let (mut controller, id, path) = seeded(&dir);
    controller
        .set_model_import_options(id, join(0.5))
        .expect("the options apply");
    let options_of = |c: &AppController<SilentBackend>| {
        c.state
            .session()
            .models()
            .iter()
            .find(|m| m.id == id)
            .expect("the model")
            .import_options
    };

    controller.reload_model(id).expect("reload");
    assert_eq!(options_of(&controller), join(0.5), "reload");
    controller
        .rescale_model(id, ModelUnits::Inches)
        .expect("rescale");
    assert_eq!(options_of(&controller), join(0.5), "rescale");
    let moved = dir.join("moved.svg");
    std::fs::copy(&path, &moved).expect("copy the drawing");
    controller.relink_model(id, &moved).expect("relink");
    assert_eq!(options_of(&controller), join(0.5), "relink");
    let _ = std::fs::remove_dir_all(&dir);
}
