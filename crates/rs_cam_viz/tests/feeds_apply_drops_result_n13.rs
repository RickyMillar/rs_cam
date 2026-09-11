//! N13 sentry (2026-09-11) — a feeds Apply drops the cached core result.
//!
//! `AppController::apply_feeds_through_funnel` is the one write door for
//! every feeds Apply: the modal's `⚡ Apply all`, the project rollup's
//! `⚡⚡ Apply all toolpaths` and its selected-rows sibling, the explore
//! apply, and the MCP `apply_feeds` tool through the public
//! `apply_feeds_recommendation`. It writes `tc.operation` directly through
//! `session.toolpath_configs_mut()`, so no core setter runs.
//!
//! Before N13 the funnel stamped `ToolpathRuntime::stale_since` and stopped
//! there. `FreshnessState` is derived from the core result cache and ignores
//! `stale_since` (`state/freshness.rs`), so the operation read `Current`
//! straight after an Apply, and `io::export` emitted the geometry and the
//! feed words of the PREVIOUS parameter set. This is the same defect
//! G-FRESHSTATE closed for the GUI inspector, on the door the inspector does
//! not use.
//!
//! One objection this file answers directly: an `ApplyScope::Speeds` apply
//! "does not change the cut", so it looks safe to keep the result. It is not.
//! The feed word is baked into the generated motion, so a stale result emits
//! the old `F` value from a program the operator believes carries the new
//! one. The MCP test below applies `Speeds` alone and demands the drop.
//!
//! Evidence class: controller-level integration with a silent compute
//! backend, plus the real export gate. Nothing renders egui.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::PathBuf;
use std::sync::Arc;

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::config::StockSource;
use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::geo::P3;
use rs_cam_core::mesh::make_test_flat;
use rs_cam_core::session::{
    AdoptResultArgs, Command, LoadedModel, ProjectSession, ProjectSessionBuilder,
    ToolpathComputeResult, ToolpathConfig,
};
use rs_cam_core::toolpath::Toolpath;
use rs_cam_core::toolpath_spans::AnnotatedToolpath;
use rs_cam_viz::compute::{
    CollisionRequest, ComputeBackend, ComputeLane, ComputeMessage, ComputeRequest,
    GenerationControl, LaneSnapshot, OptimizeRequest, SimulationRequest, ToolpathSubmitOutcome,
};
use rs_cam_viz::controller::AppController;
use rs_cam_viz::io::export::export_gcode_from_session_with_policy;
use rs_cam_viz::state::freshness::{FreshnessState, freshness_at};
use rs_cam_viz::state::runtime::{ComputeStatus, StaleResultPolicy, ToolpathRuntime};
use rs_cam_viz::state::toolpath::ToolpathResult;
use rs_cam_viz::ui::AppEvent;

/// The feed word in the geometry the operator last generated, on the
/// toolpath the Apply writes.
const PREVIOUS_FEED: f64 = 600.0;
/// The same, on the downstream `FromRemainingStock` toolpath.
const DOWNSTREAM_FEED: f64 = 700.0;

const LEAD_NAME: &str = "Rough Pocket";

// ── harness ────────────────────────────────────────────────────────────────

/// A compute backend that accepts every submission and returns nothing.
/// The apply handlers never wait on a lane, so a permanently idle lane is a
/// faithful stand-in. Copied from `apply_contract_a3.rs`.
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

fn sample_toolpath(feed: f64) -> Toolpath {
    let mut path = Toolpath::new();
    path.rapid_to(P3::new(0.0, 0.0, 5.0));
    path.feed_to(P3::new(10.0, 0.0, -1.0), feed);
    path.feed_to(P3::new(10.0, 10.0, -1.0), feed);
    path.rapid_to(P3::new(10.0, 10.0, 5.0));
    path
}

fn core_result(path: Toolpath) -> ToolpathComputeResult {
    ToolpathComputeResult {
        op_data: rs_cam_core::drill_op::OpData::Toolpath(Arc::new(AnnotatedToolpath::new(path))),
        stats: Default::default(),
        debug_trace: None,
        semantic_trace: None,
    }
}

fn viz_result(path: Toolpath) -> ToolpathResult {
    ToolpathResult {
        annotated: Arc::new(AnnotatedToolpath::new(path)),
        stats: Default::default(),
        debug_trace: None,
        semantic_trace: None,
        debug_trace_path: None,
        drill_op: None,
    }
}

fn policy() -> rs_cam_core::gcode::ToolLoadExportPolicy {
    rs_cam_core::gcode::ToolLoadExportPolicy {
        accept_unmodeled: true,
        accept_exceeded: true,
    }
}

fn toolpath(name: &str, op_type: OperationType, stock_source: StockSource) -> ToolpathConfig {
    ToolpathConfig {
        id: rs_cam_core::ToolpathId(0),
        name: name.to_owned(),
        enabled: true,
        operation: OperationConfig::new_default(op_type),
        dressups: Default::default(),
        heights: Default::default(),
        tool_id: 1,
        model_id: 0,
        pre_gcode: None,
        post_gcode: None,
        boundary: Default::default(),
        boundary_inherit: true,
        rest_analysis: Default::default(),
        stock_source,
        coolant: Default::default(),
        face_selection: None,
        debug_options: Default::default(),
        feeds_provenance: Default::default(),
        planner_origin: None,
    }
}

/// A two-operation project, both operations GENERATED: a result in the core
/// cache and a drawable copy in the viz store, exactly as
/// `drain_compute_results` leaves them.
///
/// Index 0 is the Pocket the Apply writes. Index 1 sits after it in the same
/// setup and takes `StockSource::FromRemainingStock`, so it is what
/// `invalidate_result_chain` must reach.
fn build_controller() -> AppController<SilentBackend> {
    let mut controller = AppController::with_backend(SilentBackend);
    controller.state.session = ProjectSessionBuilder::new()
        .tool(ToolConfig::new_default(ToolId(1), ToolType::EndMill))
        .build();
    let session: &mut ProjectSession = &mut controller.state.session;
    let _ = session.add_model(LoadedModel {
        id: 0,
        path: PathBuf::from("flat.stl"),
        name: "Flat".to_owned(),
        kind: Some(ModelKind::Stl),
        mesh: Some(Arc::new(make_test_flat(40.0))),
        polygons: None,
        drill_targets: Arc::new(Vec::new()),
        layers: Arc::new(Vec::new()),
        enriched_mesh: None,
        units: Some(ModelUnits::Millimeters),
        winding_report: None,
        load_error: None,
    });
    let _ = session
        .add_toolpath(
            0,
            toolpath(LEAD_NAME, OperationType::Pocket, StockSource::default()),
        )
        .expect("add the lead toolpath");
    let _ = session
        .add_toolpath(
            0,
            toolpath(
                "Rest Pocket",
                OperationType::Pocket,
                StockSource::FromRemainingStock,
            ),
        )
        .expect("add the downstream toolpath");
    let lead_revision = session.toolpath_revision(0);
    let _ = session
        .apply(Command::AdoptResult(AdoptResultArgs {
            index: 0,
            revision: lead_revision,
            result: Box::new(core_result(sample_toolpath(PREVIOUS_FEED))),
        }))
        .expect("seed the lead core result");
    let downstream_revision = session.toolpath_revision(1);
    let _ = session
        .apply(Command::AdoptResult(AdoptResultArgs {
            index: 1,
            revision: downstream_revision,
            result: Box::new(core_result(sample_toolpath(DOWNSTREAM_FEED))),
        }))
        .expect("seed the downstream core result");

    for (index, feed) in [(0usize, PREVIOUS_FEED), (1usize, DOWNSTREAM_FEED)] {
        let id = controller.state.session.toolpath_configs()[index].id;
        let mut rt = ToolpathRuntime::new(true);
        rt.status = ComputeStatus::Done;
        rt.result = Some(viz_result(sample_toolpath(feed)));
        controller.state.gui.toolpath_rt.insert(id, rt);
    }
    controller
}

fn id_at(controller: &AppController<SilentBackend>, index: usize) -> rs_cam_core::ToolpathId {
    controller.state.session.toolpath_configs()[index].id
}

fn feed_at(controller: &AppController<SilentBackend>, index: usize) -> Option<f64> {
    Some(
        controller.state.session.toolpath_configs()[index]
            .operation
            .feed_rate(),
    )
}

fn export(controller: &AppController<SilentBackend>) -> Result<String, String> {
    export_gcode_from_session_with_policy(
        &controller.state.session,
        &controller.state.gui,
        &controller.state.simulation,
        policy(),
        StaleResultPolicy::Refuse,
    )
    .map_err(|e| e.to_string())
}

fn f_words(gcode: &str) -> Vec<f64> {
    let mut seen = Vec::new();
    for line in gcode.lines() {
        let mut rest = line;
        while let Some(pos) = rest.find('F') {
            let after = &rest[pos + 1..];
            let end = after
                .find(|c: char| !(c.is_ascii_digit() || c == '.'))
                .unwrap_or(after.len());
            if let Ok(v) = after[..end].parse::<f64>()
                && !seen.contains(&v)
            {
                seen.push(v);
            }
            rest = &after[end..];
        }
    }
    seen
}

/// Non-vacuity: the fixture really is generated, really reads `Current`, and
/// really exports the previous feed word. Every claim below is a statement
/// about what the Apply changed, so all three must hold FIRST.
fn assert_fixture_is_generated(controller: &AppController<SilentBackend>) {
    assert!(
        controller.state.session.get_result(0).is_some(),
        "fixture: the lead operation must hold a core result"
    );
    assert!(
        controller.state.session.get_result(1).is_some(),
        "fixture: the downstream operation must hold a core result"
    );
    assert_eq!(
        freshness_at(&controller.state.session, &controller.state.gui, 0),
        Some(FreshnessState::Current),
        "fixture: the lead operation must read Current before the Apply"
    );
    let gcode = export(controller).expect("fixture: a generated project exports");
    assert!(
        f_words(&gcode).contains(&PREVIOUS_FEED),
        "fixture: the export must carry the previous feed word; got {:?}",
        f_words(&gcode)
    );
}

// ── 1. The GUI `⚡ Apply all` door ──────────────────────────────────────

/// `AppEvent::ApplyFeedsAll` is what the modal's `⚡ Apply all` button
/// pushes (`ui/feeds_modal.rs:671`), and what the project rollup pushes per
/// row (`:2999`).
#[test]
fn apply_feeds_all_drops_the_cached_result_n13() {
    let mut controller = build_controller();
    assert_fixture_is_generated(&controller);
    let before = feed_at(&controller, 0);

    controller.handle_internal_event(AppEvent::ApplyFeedsAll(id_at(&controller, 0)));

    assert_ne!(
        feed_at(&controller, 0),
        before,
        "the Apply wrote nothing, so this test proves nothing about it"
    );
    assert!(
        controller.state.session.get_result(0).is_none(),
        "N13: the feeds Apply funnel left the core result in place. Export \
         then emits the geometry and the feed words of the PRE-APPLY \
         parameter set."
    );
    assert_eq!(
        freshness_at(&controller.state.session, &controller.state.gui, 0),
        Some(FreshnessState::EditedSince),
        "N13: an operation edited by the Apply funnel still reads Current, \
         so every surface calls the stale geometry the answer"
    );
}

/// The chain half. `invalidate_result_chain` reaches the downstream
/// `FromRemainingStock` operation, which is machined out of the stock the
/// edited operation leaves. The single-id door is what proves this: the
/// rollup would write index 1 itself.
#[test]
fn apply_feeds_all_drops_the_downstream_chain_n13() {
    let mut controller = build_controller();
    assert_fixture_is_generated(&controller);

    controller.handle_internal_event(AppEvent::ApplyFeedsAll(id_at(&controller, 0)));

    assert!(
        controller.state.session.get_result(1).is_none(),
        "N13: the downstream FromRemainingStock operation kept its result. \
         Its stock came from the operation the Apply just changed."
    );
}

/// The consequence the operator can be cut by: the file. Under
/// `StaleResultPolicy::Refuse` the export must name the operation instead of
/// emitting the program it is replacing.
#[test]
fn apply_feeds_all_stales_the_export_n13() {
    let mut controller = build_controller();
    assert_fixture_is_generated(&controller);

    controller.handle_internal_event(AppEvent::ApplyFeedsAll(id_at(&controller, 0)));

    match export(&controller) {
        Ok(gcode) => panic!(
            "N13: the export succeeded after a feeds Apply and emitted the \
             pre-apply feed words {:?} (the operation now asks for {:?}). \
             The Apply funnel must drop the core result through \
             invalidate_toolpath_inputs.",
            f_words(&gcode),
            feed_at(&controller, 0)
        ),
        Err(msg) => {
            assert!(
                msg.contains(&format!("'{LEAD_NAME}' was edited after it was generated")),
                "the refusal must name the operation and say what happened; \
                 got: {msg}"
            );
        }
    }
}

// ── 2. The project rollup door ──────────────────────────────────────────

/// `AppEvent::ApplyFeedsProject` is the modal's `⚡⚡ Apply all toolpaths`
/// (`ui/feeds_modal.rs:2912`). It fans the same funnel over every enabled
/// toolpath, so EVERY index it writes must lose its result.
#[test]
fn apply_feeds_project_drops_every_written_result_n13() {
    let mut controller = build_controller();
    assert_fixture_is_generated(&controller);
    let before = [feed_at(&controller, 0), feed_at(&controller, 1)];

    controller.handle_internal_event(AppEvent::ApplyFeedsProject);

    assert_ne!(
        [feed_at(&controller, 0), feed_at(&controller, 1)],
        before,
        "the rollup wrote nothing, so this test proves nothing about it"
    );
    for index in [0usize, 1usize] {
        assert!(
            controller.state.session.get_result(index).is_none(),
            "N13: the project rollup left toolpath {index}'s core result in \
             place after writing its operation"
        );
    }
}

// ── 3. The MCP `apply_feeds` door ───────────────────────────────────────

/// `AppController::apply_feeds_recommendation` is the public function the
/// MCP `apply_feeds` tool calls (`app/mcp.rs:5405-5407`). The tool's own
/// `mcp_apply_stale` stamps `stale_since` and nothing else, so the agent
/// route carries the identical defect.
///
/// The scope here is `Speeds` on purpose. That scope promises not to change
/// the cut GEOMETRY, and it keeps that promise — but the feed word is part
/// of the emitted motion, so the cached result is wrong all the same.
#[test]
fn agent_apply_feeds_drops_the_cached_result_n13() {
    use rs_cam_core::feeds::suggest::ApplyScope;

    let mut controller = build_controller();
    assert_fixture_is_generated(&controller);
    let before = feed_at(&controller, 0);
    let id = id_at(&controller, 0);

    controller
        .apply_feeds_recommendation(id, ApplyScope::Speeds)
        .expect("a Pocket on a flat end mill is a runnable pairing");

    assert_ne!(
        feed_at(&controller, 0),
        before,
        "the agent Apply wrote no feed, so this test proves nothing about it"
    );
    assert!(
        controller.state.session.get_result(0).is_none(),
        "N13: the MCP apply_feeds door left the core result in place. A \
         'speeds only' apply still changes the F word the file carries."
    );
    assert!(
        export(&controller).is_err(),
        "N13: the export emitted the pre-apply feed word after an agent \
         apply_feeds call"
    );
}
