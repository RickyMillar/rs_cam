//! K2b (feeds contract P5b): a click on the envelope plot writes nothing
//! until the ⚡ applies it; the nomogram follows the candidate.
//!
//! # The claim
//!
//! 1. A click inside the plot moves the candidate ring to the point under
//!    the pointer (GUI state, `state::panels::EnvelopeCandidates`) and
//!    sends no event. The ⚡ then sends ONE
//!    `AppEvent::ApplyFeedsPick { pick: Point { ap, ae } }` at the ring.
//! 2. Live link (operator ruling 2026-11-03): while the candidate exists,
//!    the nomogram input (`envelope::candidate_speeds`, what the Passes tab
//!    hands `explore::draw_inline`) is the `SpeedsMap` at the candidate's
//!    (ap, ae), not at the proposal. Esc drops the candidate.
//! 3. The session is unchanged until the apply. The controller then writes
//!    the picked point through `Command::ApplyFeeds` and drops the ring. A
//!    point outside the envelope is written too (ruling Q6: no refusal),
//!    and the op's placement names the crossed limit, as the
//!    `project.outside_envelope` warning does.
//!
//! Red before P5b: the plot, the candidate state and `ApplyFeedsPick` do
//! not exist.

#![expect(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

mod envelope_k2b;

use envelope_k2b::{ID, click_at, ctx, envelope_for, frame};
use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::feeds::envelope::{CutPoint, PickRequest, Placement};
use rs_cam_core::session::{ProjectSessionBuilder, ToolpathConfig};
use rs_cam_viz::compute::{
    CollisionRequest, ComputeBackend, ComputeLane, ComputeMessage, ComputeRequest,
    GenerationControl, LaneSnapshot, SimulationRequest, ToolpathSubmitOutcome,
};
use rs_cam_viz::controller::AppController;
use rs_cam_viz::state::panels::EnvelopeCandidates;
use rs_cam_viz::ui::AppEvent;
use rs_cam_viz::ui::feeds::envelope::{APPLY_GLYPH, EnvelopeShape, candidate_speeds};

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

/// The point at 40 % of each span: inside the plot, off the proposal.
fn target(env: &rs_cam_core::feeds::envelope::SafeEnvelope) -> CutPoint {
    let scene = rs_cam_viz::ui::feeds::envelope::EnvelopeScene::of(env, None);
    CutPoint {
        ap_mm: scene.ap_range.1 * 0.4,
        ae_mm: scene.ae_range.1 * 0.4,
    }
}

#[test]
fn a_plot_click_moves_the_ring_and_the_apply_sends_the_pick_k2b() {
    let (config, env) = envelope_for(OperationType::Pocket).expect("the Pocket is backed");
    let ctx = ctx();
    let mut candidates = EnvelopeCandidates::default();
    // Three frames settle the plot bounds (set on every frame).
    let _ = frame(&ctx, &config, Some(&env), &mut candidates, Vec::new());
    let _ = frame(&ctx, &config, Some(&env), &mut candidates, Vec::new());
    let first = frame(&ctx, &config, Some(&env), &mut candidates, Vec::new());
    let want = target(&env);
    let pos = first
        .response
        .screen_of(want, EnvelopeShape::Plane)
        .expect("the plot has a frame");
    let mut sent = Vec::new();
    for events in click_at(pos) {
        let f = frame(&ctx, &config, Some(&env), &mut candidates, events);
        sent.extend(f.events);
    }
    assert!(sent.is_empty(), "a click writes nothing: {sent:?}");
    let got = candidates.get(ID).expect("the click made a candidate");
    let tol = |range: (f64, f64)| (range.1 - range.0) * 0.02;
    assert!(
        (got.ap_mm - want.ap_mm).abs() < tol(first.response.ap_range)
            && (got.ae_mm - want.ae_mm).abs() < tol(first.response.ae_range),
        "the ring at {got:?}, the click at {want:?}"
    );

    // 2. The nomogram input is the candidate's cut.
    let speeds = candidate_speeds(&env, candidates.get(ID)).expect("a lookup at the candidate");
    let at_candidate = env
        .speeds
        .speeds_at(got.ap_mm, got.ae_mm)
        .expect("the grid answers");
    assert_eq!(speeds, at_candidate, "the nomogram reads the candidate");
    let p = env.proposal.get();
    let at_proposal = env.speeds.speeds_at(p.ap_mm, p.ae_mm).expect("the grid");
    assert_ne!(
        speeds, at_proposal,
        "fixture: the candidate is not the proposal"
    );
    assert!(
        candidate_speeds(&env, None).is_none(),
        "no candidate, the stored cut"
    );

    // 1. The ⚡ sends one pick at the ring.
    let f = frame(&ctx, &config, Some(&env), &mut candidates, Vec::new());
    let chip = f
        .shapes
        .iter()
        .find_map(|c| {
            if let egui::Shape::Text(t) = &c.shape
                && t.galley.job.text == APPLY_GLYPH
            {
                Some(t.visual_bounding_rect().center())
            } else {
                None
            }
        })
        .expect("the header paints the ⚡");
    let mut sent = Vec::new();
    for events in click_at(chip) {
        let f = frame(&ctx, &config, Some(&env), &mut candidates, events);
        sent.extend(f.events);
    }
    assert_eq!(sent.len(), 1, "one apply: {sent:?}");
    let Some(AppEvent::ApplyFeedsPick {
        toolpath_id,
        pick: PickRequest::Point { ap_mm, ae_mm },
    }) = sent.first()
    else {
        panic!("the ⚡ sent {sent:?}");
    };
    assert_eq!(*toolpath_id, ID);
    assert_eq!((*ap_mm, *ae_mm), (got.ap_mm, got.ae_mm));

    // Esc puts the ring back.
    let _ = frame(
        &ctx,
        &config,
        Some(&env),
        &mut candidates,
        vec![egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    assert!(candidates.get(ID).is_none(), "Esc drops the candidate");
}

/// Feeds contract P11c (D13 item 6): the Pocket is 6 mm deep, not the
/// fixture's 3 mm. The hobby grade cap (0.25 x 6.35 = 1.5875 mm) leaves the
/// stated 1.5 mm as the proposal, and on a 3 mm floor the outside pick
/// (about 2.02 mm) snaps to two passes of 1.5 mm, the proposal itself. On a
/// 6 mm floor it snaps to three passes of 2.0 mm, above the proposal.
#[test]
fn the_session_is_unchanged_until_the_apply_and_an_outside_pick_warns_k2b() {
    let mut operation = rs_cam_core::compute::fixture_values::fixture_op(OperationType::Pocket);
    if let rs_cam_core::compute::catalog::OperationConfig::Pocket(cfg) = &mut operation {
        cfg.floor = rs_cam_core::compute::floor::Floor::top(6.0);
    }
    let mut controller = AppController::with_backend(SilentBackend);
    let mut builder = ProjectSessionBuilder::new().tool(ToolConfig::test_default(
        ToolId::from_raw(1),
        ToolType::EndMill,
    ));
    let _ = builder
        .add_toolpath(
            0,
            ToolpathConfig {
                shoe: Default::default(),
                feeds_lock: Default::default(),
                id: rs_cam_core::ToolpathId::from_raw(0),
                name: "Pocket".to_owned(),
                enabled: true,
                operation,
                dressups: Default::default(),
                heights: Default::default(),
                tool_id: Some(rs_cam_core::ids::ToolId::from_raw(1)),
                model_id: rs_cam_core::ids::ModelId::from_raw(0),
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
            },
        )
        .expect("add the Pocket");
    controller.state.replace_session(builder.build());
    let id = controller.state.session().toolpath_configs()[0].id;
    let tc = controller.state.session().toolpath_configs()[0].clone();
    let env = controller
        .state
        .session()
        .safe_envelope(&tc)
        .expect("a tool")
        .expect("a backed Pocket");
    // A point past the top of the depth span: outside the envelope.
    let scene = rs_cam_viz::ui::feeds::envelope::EnvelopeScene::of(&env, None);
    let outside = CutPoint {
        ap_mm: scene.ap_range.1 * 0.98,
        ae_mm: env.proposal.get().ae_mm,
    };
    let Placement::Outside { binding, .. } = env.placement_of(outside) else {
        panic!("fixture: {outside:?} must sit outside");
    };

    // A candidate is GUI state: the session does not move.
    let before = format!("{:?}", tc.operation);
    controller.state.panels.envelope_candidates.set(id, outside);
    assert_eq!(
        format!(
            "{:?}",
            controller.state.session().toolpath_configs()[0].operation
        ),
        before,
        "a candidate writes nothing"
    );

    // The apply writes it, warns, and drops the ring.
    controller.handle_internal_event(AppEvent::ApplyFeedsPick {
        toolpath_id: id,
        pick: PickRequest::Point {
            ap_mm: outside.ap_mm,
            ae_mm: outside.ae_mm,
        },
    });
    let after = controller.state.session().toolpath_configs()[0].clone();
    assert_ne!(format!("{:?}", after.operation), before, "the apply writes");
    assert!(
        controller
            .state
            .panels
            .envelope_candidates
            .get(id)
            .is_none(),
        "the apply drops the candidate"
    );
    assert!(
        after.operation.depth_per_pass().expect("a Pocket depth") > env.proposal.get().ap_mm,
        "Q6: the outside depth is written, not refused: picked {} mm, written {:?} mm, \
         proposal {} mm",
        outside.ap_mm,
        after.operation.depth_per_pass(),
        env.proposal.get().ap_mm
    );
    let placed = controller
        .state
        .session()
        .safe_envelope(&after)
        .expect("a tool")
        .expect("a backed Pocket");
    match placed.operator {
        Some(Placement::Outside { binding: b, .. }) => assert_eq!(b, binding),
        other => panic!("the written point sits {other:?}, not outside {binding:?}"),
    }
}
