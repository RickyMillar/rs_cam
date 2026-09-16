//! G-SPEEDSONLY (2026-09-16) — `Match vendor chipload` moves the speeds and
//! holds the cut.
//!
//! Phase C of `planning/load_model_2026-09-16/SPEC.md`. Phase A gave the Feeds
//! tab a verdict row that tells the operator their chipload is thin and costs
//! them 1.6× tool wear. Until this button the tab's only write was `⚡ Apply
//! all — changes the cut`, which overwrites RPM, feed, plunge, DOC and WOC
//! together — so the offered answer to "my feed is wrong" was "let me also
//! change your cut geometry".
//!
//! The claim this file pins is narrow and it is the whole point of the action:
//! driving the button's event through the real controller moves feed, plunge
//! and RPM, and leaves DOC and WOC **byte-identical**.
//!
//! Three things make the claim non-vacuous, and all three are asserted:
//!
//! - the speeds really move, so the apply is not a silent no-op;
//! - `⚡ Apply all` on the SAME fixture really does move DOC and WOC, so the
//!   first arm is not passing on a fixture where nothing would have moved;
//! - the toolpath is left stale (G-FRESHSTATE / N13). A speeds-only apply
//!   looks safe to keep a result, and it is not: the feed word is baked into
//!   the generated motion, so a kept result exports the old `F` value.
//!
//! Evidence class: controller-level integration with a silent compute backend,
//! as `apply_contract_a3.rs` and `feeds_apply_drops_result_n13.rs` do. Nothing
//! renders egui. The one source-level arm is the button FACE, which A-3's
//! contract places on the button rather than its hover.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::sync::Arc;

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::geo::P3;
use rs_cam_core::session::{
    AdoptResultArgs, Command, ProjectSessionBuilder, ToolpathComputeResult, ToolpathConfig,
};
use rs_cam_core::toolpath::Toolpath;
use rs_cam_core::trace::toolpath_spans::AnnotatedToolpath;
use rs_cam_viz::compute::{
    CollisionRequest, ComputeBackend, ComputeLane, ComputeMessage, ComputeRequest,
    GenerationControl, LaneSnapshot, OptimizeRequest, SimulationRequest, ToolpathSubmitOutcome,
};
use rs_cam_viz::controller::AppController;
use rs_cam_viz::state::freshness::{FreshnessState, freshness_at};
use rs_cam_viz::state::runtime::{ComputeStatus, ToolpathRuntime};
use rs_cam_viz::state::toolpath::ToolpathResult;
use rs_cam_viz::ui::AppEvent;

/// The comparison card's source. A-3 rules that a write's attribution sits on
/// the button's FACE, not its hover, so the face is read from source here the
/// way `apply_contract_a3.rs` reads `⚡ Apply all — changes the cut`.
const COMPARE_SRC: &str = include_str!("../src/ui/feeds/compare.rs");

/// The feed word already baked into the generated motion.
const PREVIOUS_FEED: f64 = 600.0;

// ── harness ────────────────────────────────────────────────────────────────

/// A compute backend that accepts every submission and returns nothing. The
/// apply handlers never wait on a lane, so a permanently idle lane is a
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

fn sample_toolpath() -> Toolpath {
    let mut path = Toolpath::new();
    path.rapid_to(P3::new(0.0, 0.0, 5.0));
    path.feed_to(P3::new(10.0, 0.0, -1.0), PREVIOUS_FEED);
    path.rapid_to(P3::new(10.0, 0.0, 5.0));
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

fn toolpath() -> ToolpathConfig {
    ToolpathConfig {
        id: rs_cam_core::ToolpathId(0),
        name: "Rough Pocket".to_owned(),
        enabled: true,
        operation: OperationConfig::new_default(OperationType::Pocket),
        dressups: Default::default(),
        heights: Default::default(),
        tool_id: 1,
        model_id: 0,
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

/// One GENERATED Pocket on the default Ø6.35 2-flute flat end mill — the
/// census fixture `apply_contract_a3.rs` measures the recipe fingerprint on.
/// It carries a core result and a drawable viz copy, exactly as
/// `drain_compute_results` leaves them, so the staleness arm has something to
/// drop.
fn build_controller() -> AppController<SilentBackend> {
    let mut controller = AppController::with_backend(SilentBackend);
    let mut builder =
        ProjectSessionBuilder::new().tool(ToolConfig::new_default(ToolId(1), ToolType::EndMill));
    let _ = builder.add_toolpath(0, toolpath()).expect("add toolpath");
    controller.state.session = builder.build();

    let revision = controller.state.session.toolpath_revision(0);
    let _ = controller
        .state
        .session
        .apply(Command::AdoptResult(AdoptResultArgs {
            index: 0,
            revision,
            result: Box::new(core_result()),
        }))
        .expect("seed the core result");

    let id = controller.state.session.toolpath_configs()[0].id;
    let mut rt = ToolpathRuntime::new(true);
    rt.status = ComputeStatus::Done;
    rt.result = Some(viz_result());
    controller.state.gui.toolpath_rt.insert(id, rt);
    controller
}

fn id_of(controller: &AppController<SilentBackend>) -> rs_cam_core::ToolpathId {
    controller.state.session.toolpath_configs()[0].id
}

fn op_of(controller: &AppController<SilentBackend>) -> OperationConfig {
    controller.state.session.toolpath_configs()[0]
        .operation
        .clone()
}

/// Feed / plunge / RPM.
fn speeds_of(op: &OperationConfig) -> (f64, f64, Option<u32>) {
    (op.feed_rate(), op.plunge_rate(), op.spindle_rpm())
}

/// DOC / WOC as raw bits. "Byte-identical" is the contract, so the comparison
/// is on the bit pattern and not on `==`, which would let a rewritten value
/// that happens to round back pass.
fn cut_bits(op: &OperationConfig) -> (Option<u64>, Option<u64>) {
    (
        op.depth_per_pass().map(f64::to_bits),
        op.stepover().map(f64::to_bits),
    )
}

// ── the arm that matters ───────────────────────────────────────────────────

/// `Match vendor chipload` (`ui/feeds/compare.rs::draw_apply_column` →
/// `AppEvent::ApplyFeedsSpeeds`) moves the speeds and holds the cut.
///
/// On this fixture `⚡ Apply all` writes 3000 / 794 / 18000 / 2.222 / 1.27
/// (feed / plunge / RPM / WOC / DOC) over 1000 / 500 / None / 2.0 / 1.5. The
/// speeds half of that is what this button writes; the cut half is what it
/// must not touch.
#[test]
fn the_speeds_apply_holds_the_cut_g_speedsonly() {
    let mut controller = build_controller();
    let before = op_of(&controller);
    assert!(
        before.depth_per_pass().is_some() && before.stepover().is_some(),
        "fixture: the operation must carry both cut-geometry dials, or holding \
         them asserts nothing"
    );
    let id = id_of(&controller);

    controller.handle_internal_event(AppEvent::ApplyFeedsSpeeds(id));
    let after = op_of(&controller);

    assert_ne!(
        speeds_of(&after),
        speeds_of(&before),
        "the speeds apply wrote nothing, so this test proves nothing about it"
    );
    assert_ne!(after.feed_rate(), before.feed_rate(), "feed did not move");
    assert_ne!(
        after.plunge_rate(),
        before.plunge_rate(),
        "plunge did not move"
    );
    assert_ne!(
        after.spindle_rpm(),
        before.spindle_rpm(),
        "RPM did not move"
    );

    assert_eq!(
        cut_bits(&after),
        cut_bits(&before),
        "the speeds apply moved the cut geometry: DOC {:?} → {:?}, WOC {:?} → {:?}. \
         Holding it is the whole reason the button exists.",
        before.depth_per_pass(),
        after.depth_per_pass(),
        before.stepover(),
        after.stepover(),
    );
}

/// Non-vacuity for the arm above. `⚡ Apply all` on the SAME fixture DOES move
/// the cut geometry, so "DOC and WOC are unchanged" is a statement about the
/// scope and not about a fixture where nothing would have moved anyway.
#[test]
fn apply_all_moves_the_cut_on_the_same_fixture() {
    let mut controller = build_controller();
    let before = op_of(&controller);
    let id = id_of(&controller);

    controller.handle_internal_event(AppEvent::ApplyFeedsAll(id));
    let after = op_of(&controller);

    assert_ne!(
        cut_bits(&after),
        cut_bits(&before),
        "`⚡ Apply all` left the cut geometry alone on this fixture, so the \
         speeds-only arm is vacuous — rebase the fixture"
    );
    assert_ne!(
        speeds_of(&after),
        speeds_of(&before),
        "`⚡ Apply all` left the speeds alone on this fixture too"
    );
}

/// G-FRESHSTATE / N13. A speeds-only apply looks like it could keep the
/// generated result — it did not change the cut, after all. It cannot: the
/// feed word is baked into the motion, so a kept result exports the `F` value
/// of the parameter set the operator just replaced.
#[test]
fn the_speeds_apply_leaves_the_toolpath_stale_g_speedsonly() {
    let mut controller = build_controller();
    assert!(
        controller.state.session.get_result(0).is_some(),
        "fixture: the operation must hold a core result before the apply"
    );
    assert_eq!(
        freshness_at(&controller.state.session, &controller.state.gui, 0),
        Some(FreshnessState::Current),
        "fixture: the operation must read Current before the apply"
    );
    let id = id_of(&controller);
    let before = op_of(&controller);

    controller.handle_internal_event(AppEvent::ApplyFeedsSpeeds(id));

    assert_ne!(
        op_of(&controller).feed_rate(),
        before.feed_rate(),
        "the apply wrote nothing, so the staleness claim is vacuous"
    );
    assert!(
        controller.state.session.get_result(0).is_none(),
        "the speeds apply kept the cached core result. Export then emits the \
         feed words of the PRE-APPLY parameter set."
    );
    assert_eq!(
        freshness_at(&controller.state.session, &controller.state.gui, 0),
        Some(FreshnessState::EditedSince),
        "the toolpath still reads Current after a speeds apply, so every \
         surface calls the stale geometry the answer"
    );
}

/// The apply door's refusal rule is the funnel's, not the button's, so the
/// narrower scope answers to it too: on a tool × operation pairing
/// `validate_tool_for_operation` refuses, nothing is written and the operator
/// is told.
#[test]
fn the_speeds_apply_refuses_a_refused_pairing() {
    let mut controller = AppController::with_backend(SilentBackend);
    // A flat end mill on Scallop: a zero tip radius makes the scallop-stepover
    // formula undefined, and the engine refuses the pairing outright.
    let mut builder =
        ProjectSessionBuilder::new().tool(ToolConfig::new_default(ToolId(1), ToolType::EndMill));
    let mut refused = toolpath();
    refused.operation = OperationConfig::new_default(OperationType::Scallop);
    let _ = builder.add_toolpath(0, refused).expect("add toolpath");
    controller.state.session = builder.build();

    let before = op_of(&controller);
    let id = id_of(&controller);
    controller.handle_internal_event(AppEvent::ApplyFeedsSpeeds(id));
    let after = op_of(&controller);

    assert_eq!(
        speeds_of(&after),
        speeds_of(&before),
        "the speeds apply wrote a pairing the engine refuses"
    );
    assert!(
        controller
            .active_notifications()
            .any(|n| n.message.contains("Feeds not applied")),
        "the refusal was silent — a no-op button is its own defect"
    );
}

/// A-3's attribution contract, in its mirror image. `⚡ Apply all — changes
/// the cut` says on its FACE what it changes; this button must say on its face
/// that it changes the speeds and holds the cut. A hover is not enough: the
/// operator has to read it before clicking, not after hesitating.
#[test]
fn the_speeds_apply_states_its_scope_on_the_button_face() {
    assert!(
        COMPARE_SRC.contains("⚡ Match vendor chipload — changes the speeds, not the cut"),
        "the speeds-only apply lost its face attribution; it must say what it \
         writes AND what it holds before the operator clicks"
    );
    assert!(
        COMPARE_SRC.contains("⚡ Apply all — changes the cut"),
        "the combined apply's own face attribution went away with this change"
    );
}
