//! B19 sentry — a feeds Apply that changes no value stales nothing.
//!
//! `AppController::apply_feeds_through_funnel` stamps the stale set that
//! core returns (`stamp_stale(&effects.stale)`). Before B19 it ALSO stamped
//! `stale_since = now` on the row it wrote, whatever core said. So an Apply
//! that changed no generation input still armed `process_auto_regen`, and
//! the 500 ms sweep regenerated an operation whose result was current.
//!
//! The claims, on a generated `Fresh` Pocket:
//!
//! 1. Non-vacuity: the first `Speeds` Apply changes the feed.
//! 2. A second `Speeds` Apply changes no value: core stales nothing and
//!    keeps the result.
//! 3. The row's `stale_since` stays `None`.
//! 4. After the auto-regen delay, `process_auto_regen` submits nothing.
//!
//! Red proof: before the fix, claim 3 fails.

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::config::StockSource;
use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::feeds::suggest::ApplyScope;
use rs_cam_core::geo::P3;
use rs_cam_core::mesh::make_test_flat;
use rs_cam_core::session::{
    AdoptResultArgs, Command, LoadedModel, ProjectSessionBuilder, ToolpathComputeResult,
    ToolpathConfig,
};
use rs_cam_core::toolpath::Toolpath;
use rs_cam_core::trace::toolpath_spans::AnnotatedToolpath;
use rs_cam_viz::compute::{
    CollisionRequest, ComputeBackend, ComputeLane, ComputeMessage, ComputeRequest,
    GenerationControl, LaneSnapshot, SimulationRequest, ToolpathSubmitOutcome,
};
use rs_cam_viz::controller::AppController;
use rs_cam_viz::state::runtime::ToolpathRuntime;
use rs_cam_viz::state::toolpath::ToolpathResult;

/// A backend that counts the toolpath submissions and returns nothing.
struct CountingBackend {
    submits: Arc<AtomicUsize>,
}

impl ComputeBackend for CountingBackend {
    fn submit_toolpath(&mut self, _request: ComputeRequest) -> ToolpathSubmitOutcome {
        let _ = self.submits.fetch_add(1, Ordering::SeqCst);
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

fn sample_toolpath() -> Toolpath {
    let mut path = Toolpath::new();
    path.rapid_to(P3::new(0.0, 0.0, 5.0));
    path.feed_to(P3::new(10.0, 0.0, -1.0), 600.0);
    path.feed_to(P3::new(10.0, 10.0, -1.0), 600.0);
    path.rapid_to(P3::new(10.0, 10.0, 5.0));
    path
}

fn core_result() -> ToolpathComputeResult {
    ToolpathComputeResult {
        op_data: rs_cam_core::ops::drill_op::OpData::Toolpath(Arc::new(AnnotatedToolpath::new(
            sample_toolpath(),
        ))),
        stats: Default::default(),
        debug_trace: None,
        semantic_trace: None,
        provenance: rs_cam_core::session::ResultProvenance::Dressed,
    }
}

fn viz_result() -> ToolpathResult {
    ToolpathResult {
        annotated: Arc::new(AnnotatedToolpath::new(sample_toolpath())),
        stats: Default::default(),
        debug_trace: None,
        semantic_trace: None,
        debug_trace_path: None,
        drill_op: None,
    }
}

fn pocket() -> ToolpathConfig {
    ToolpathConfig {
        shoe: Default::default(),
        id: rs_cam_core::ToolpathId::from_raw(0),
        name: "Pocket".to_owned(),
        enabled: true,
        operation: OperationConfig::new_default(OperationType::Pocket),
        dressups: Default::default(),
        heights: Default::default(),
        tool_id: rs_cam_core::ids::ToolId::from_raw(1),
        model_id: rs_cam_core::ids::ModelId::from_raw(0),
        pre_gcode: None,
        post_gcode: None,
        boundary: Default::default(),
        boundary_inherit: true,
        rest_analysis: Default::default(),
        stock_source: StockSource::Fresh,
        coolant: Default::default(),
        debug_options: Default::default(),
        feeds_provenance: Default::default(),
        planner_origin: None,
    }
}

/// Seed toolpath 0 as GENERATED at its current revision, in core and in
/// the view, with no stale stamp.
fn seed_generated(controller: &mut AppController<CountingBackend>) {
    let state = &mut controller.state;
    let revision = state.session().toolpath_revision_at(0);
    let _ = state
        .apply(Command::AdoptResult(AdoptResultArgs {
            index: 0,
            revision,
            result: Box::new(core_result()),
        }))
        .expect("seed the core result");
    let id = controller.state.session().toolpath_configs()[0].id;
    let mut rt = ToolpathRuntime::new(true);
    rt.run = rs_cam_viz::state::runtime::ToolpathRun::Done(viz_result());
    let _ = controller.state.gui.toolpath_rt.insert(id, rt);
}

fn build_controller(submits: Arc<AtomicUsize>) -> AppController<CountingBackend> {
    let mut controller = AppController::with_backend(CountingBackend { submits });
    let tool = ToolConfig::new_default(ToolId::from_raw(1), ToolType::EndMill);
    let mut builder = ProjectSessionBuilder::new().tool(tool);
    let _ = builder.add_model(LoadedModel {
        id: rs_cam_core::ids::ModelId::from_raw(0),
        path: PathBuf::from("flat.stl"),
        name: "Flat".to_owned(),
        kind: Some(ModelKind::Stl),
        mesh: Some(Arc::new(make_test_flat(40.0))),
        items: Default::default(),
        enriched_mesh: None,
        units: Some(ModelUnits::Millimeters),
        winding_report: None,
        load_error: None,
        import_options: Default::default(),
        import_report: None,
        drawing: Default::default(),
    });
    let _ = builder.add_toolpath(0, pocket()).expect("add the pocket");
    controller.state.replace_session(builder.build());
    seed_generated(&mut controller);
    controller
}

#[test]
fn a_feeds_apply_that_changes_nothing_stales_nothing_b19() {
    let submits = Arc::new(AtomicUsize::new(0));
    let mut controller = build_controller(Arc::clone(&submits));
    let id = controller.state.session().toolpath_configs()[0].id;

    // Claim 1: the first Apply writes a new feed.
    let feed_before = controller.state.session().toolpath_configs()[0]
        .operation
        .feed_rate();
    let first = controller
        .apply_feeds_recommendation(id, ApplyScope::Speeds)
        .expect("a Pocket on a flat end mill is a runnable pairing");
    assert_ne!(
        controller.state.session().toolpath_configs()[0]
            .operation
            .feed_rate(),
        feed_before,
        "fixture: the first Apply wrote no feed, so the second proves nothing"
    );
    assert!(
        first.stale.contains(&0),
        "fixture: the first Apply must stale the toolpath it wrote"
    );

    // Generate again at the applied values, as the operator would.
    seed_generated(&mut controller);
    let config_before =
        serde_json::to_string(&controller.state.session().toolpath_configs()[0].operation)
            .expect("serialise the operation");

    // Claim 2: the second Apply changes no value and core stales nothing.
    let second = controller
        .apply_feeds_recommendation(id, ApplyScope::Speeds)
        .expect("the same pairing applies again");
    assert_eq!(
        serde_json::to_string(&controller.state.session().toolpath_configs()[0].operation)
            .expect("serialise the operation"),
        config_before,
        "fixture: the second Apply must change no value"
    );
    assert!(
        second.stale.is_empty(),
        "fixture: core stales nothing on an unchanged config; got {:?}",
        second.stale
    );
    assert!(
        controller.state.session().result_at(0).is_some(),
        "fixture: core keeps the result of an unchanged config"
    );

    // Claim 3.
    assert!(
        controller.state.gui.toolpath_rt[&id].stale_since.is_none(),
        "B19: the feeds Apply funnel stamped stale_since on a row that core \
         did not stale"
    );

    // Claim 4: past the 500 ms delay, the sweep submits nothing.
    std::thread::sleep(std::time::Duration::from_millis(600));
    let toasts_before = controller.notifications().len();
    controller.process_auto_regen();
    assert_eq!(
        submits.load(Ordering::SeqCst),
        0,
        "B19: auto-regen submitted a toolpath after an Apply that changed nothing"
    );
    assert!(
        !controller.notifications()[toasts_before..]
            .iter()
            .any(|n| n.message.contains("Auto-regenerating")),
        "B19: auto-regen announced a regeneration after an Apply that changed nothing"
    );
}
