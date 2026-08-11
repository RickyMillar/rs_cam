//! A-3 — characterization of the two apply contracts.
//!
//! The operator's 2026-08-07 feeds/speeds architecture review raised a [high]
//! finding: the Feeds & Speeds *modal* and the properties *panel* are two
//! user-visible "apply the recommendation" surfaces with different validation
//! and different cut-geometry effects. This file pins that difference at the
//! **controller** level — it dispatches the same `AppEvent`s the modal's
//! buttons push (`ui/feeds_modal.rs`) through the real production handlers in
//! `controller/events/mod.rs`, and calls the same core entry points the panel
//! calls (`ui/properties/mod.rs:1582`, `:1990`, `:2032`).
//!
//! These are CHARACTERIZATION tests: they assert the behaviour that ships
//! today, including the parts that are wrong. When A-4 lands the one-funnel
//! design ruled at Checkpoint I, every `hazard_*` test here is expected to go
//! red and be rewritten as the sentry for the fixed contract. That is the
//! point — the pre-fix reproduction stays in the file permanently (plan §0.1).
//!
//! Evidence class: controller-level integration. These drive the production
//! event handlers with a scripted compute backend; they do not render egui.
//! The button → `AppEvent` mapping is therefore read from source (cited at
//! each test) rather than clicked. See
//! `planning/review_2026-08-08/APPLY_CONTRACT_CENSUS.md` §3 for why this is
//! the strongest class available (the operator's live GUI/MCP session was
//! disconnected for the duration of the wave, and MCP cannot inject a modal
//! click).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::session::{ProjectSession, ToolpathConfig};
use rs_cam_viz::compute::{
    CollisionRequest, ComputeBackend, ComputeLane, ComputeMessage, ComputeRequest,
    GenerationControl, LaneSnapshot, OptimizeRequest, SimulationRequest,
};
use rs_cam_viz::controller::AppController;
use rs_cam_viz::ui::{AppEvent, FeedsField};

// ── harness ────────────────────────────────────────────────────────────────

/// A compute backend that accepts every submission and returns nothing. The
/// apply handlers only ever mark the toolpath stale; they never wait on a
/// lane, so a lane that is permanently idle is a faithful stand-in.
struct SilentBackend;

impl ComputeBackend for SilentBackend {
    fn submit_toolpath(&mut self, _request: ComputeRequest) {}
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

/// A toolpath of `op_type` bound to tool 1.
fn toolpath(id: usize, op_type: OperationType) -> ToolpathConfig {
    ToolpathConfig {
        id: rs_cam_core::ToolpathId(id),
        name: format!("{op_type:?} fixture"),
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
        stock_source: Default::default(),
        coolant: Default::default(),
        face_selection: None,
        debug_options: Default::default(),
        feeds_provenance: Default::default(),
    }
}

/// One toolpath of `op_type` on a default Ø6.35 2-flute flat end mill —
/// `ToolType::EndMill` maps to `ToolGeometryHint::Flat` via
/// `ToolDefinition::to_geometry_hint`.
fn controller_with(op_type: OperationType) -> AppController<SilentBackend> {
    let mut controller = AppController::with_backend(SilentBackend);
    let session: &mut ProjectSession = &mut controller.state.session;
    session
        .tools_mut()
        .push(ToolConfig::new_default(ToolId(1), ToolType::EndMill));
    session
        .add_toolpath(0, toolpath(0, op_type))
        .expect("add toolpath");
    controller
}

/// The id the session actually assigned to toolpath slot `idx`.
fn id_at(controller: &AppController<SilentBackend>, idx: usize) -> rs_cam_core::ToolpathId {
    controller.state.session.toolpath_configs()[idx].id
}

fn tool_of(controller: &AppController<SilentBackend>) -> ToolConfig {
    controller.state.session.tools()[0].clone()
}

fn op_of(controller: &AppController<SilentBackend>) -> OperationConfig {
    controller.state.session.toolpath_configs()[0]
        .operation
        .clone()
}

/// The panel's entry point — `ui/properties/mod.rs:1582`. `Err` here is the
/// refusal the panel renders as `Feeds unavailable: …` in place of the whole
/// feeds card, apply buttons included (`:1603-1646`).
fn panel_recipe(
    controller: &AppController<SilentBackend>,
) -> Result<rs_cam_core::feeds::FeedsResult, rs_cam_core::feeds::FeedsError> {
    let session = &controller.state.session;
    let stock = session.stock_config();
    rs_cam_core::feeds::suggest::feeds_result_for_operation(
        &op_of(controller),
        &tool_of(controller),
        &stock.material,
        session.machine(),
        stock.workholding_rigidity,
        rs_cam_core::feeds::embedded_vendor_lut(),
        session.post_config().spindle_strategy,
    )
}

/// The modal's entry point — `ui/feeds_modal.rs:440`, and the same call the
/// apply handlers make internally (`controller/events/mod.rs:1001`).
fn modal_preview(controller: &AppController<SilentBackend>) -> rs_cam_core::feeds::FeedsExplain {
    let session = &controller.state.session;
    let stock = session.stock_config();
    rs_cam_core::feeds::suggest::feeds_explain_for_operation(
        &op_of(controller),
        &tool_of(controller),
        &stock.material,
        session.machine(),
        stock.workholding_rigidity,
        rs_cam_core::feeds::embedded_vendor_lut(),
        session.post_config().spindle_strategy,
    )
}

// ── hazard (a): the modal applies a pairing the panel refuses ──────────────

/// The fixture the whole hazard rests on: a flat end mill on a Scallop op.
/// `validate_tool_for_operation` (`feeds/mod.rs:702`) refuses it — a zero tip
/// radius makes the scallop-stepover formula `2·√(2·R·h − h²)` undefined —
/// so the panel offers no recipe and no apply button at all. The modal's
/// preview is produced by `explain`, which never calls the validator, so the
/// modal shows a full Recommendation table with live Apply buttons.
#[test]
fn hazard_a_panel_refuses_the_pairing_the_modal_previews() {
    let controller = controller_with(OperationType::Scallop);

    let refusal = panel_recipe(&controller).expect_err(
        "flat end mill on Scallop must be refused by the validated panel path — if this \
         now succeeds the validator changed and the whole A-3 fixture needs rebasing",
    );
    assert!(
        matches!(
            refusal,
            rs_cam_core::feeds::FeedsError::WrongToolForOperation { .. }
        ),
        "unexpected refusal kind: {refusal:?}"
    );

    // Same inputs, modal entry point: a numeric recommendation, no refusal.
    let preview = modal_preview(&controller);
    assert!(
        preview.recommended.feed_rate_mm_min > 0.0,
        "the infallible preview produced no feed — fixture is not exercising the hazard"
    );
}

/// `⚡ Apply all` (`ui/feeds_modal.rs:586-593` → `AppEvent::ApplyFeedsAll`)
/// writes the refused pairing's recipe straight into the operation. The panel
/// cannot reach this state: on the same toolpath it renders a refusal.
#[test]
fn hazard_a_modal_apply_all_writes_the_refused_recipe() {
    let mut controller = controller_with(OperationType::Scallop);
    assert!(panel_recipe(&controller).is_err(), "fixture precondition");

    let before = op_of(&controller);
    let id = id_at(&controller, 0);
    controller.handle_internal_event(AppEvent::ApplyFeedsAll(id));
    let after = op_of(&controller);

    assert_ne!(
        before.feed_rate(),
        after.feed_rate(),
        "Apply all did not write feed on a pairing the panel refuses — hazard (a) \
         may have been fixed; if so this test is the sentry that should now be inverted"
    );
    assert!(
        after.spindle_rpm().is_some(),
        "Apply all wrote no RPM on the refused pairing"
    );
}

/// The per-field Apply buttons (`ui/components/compare.rs:139-146`, rendered
/// from `feeds_modal.rs:517-553`) have the same reach with a finer grain —
/// including WOC, which changes the cut.
///
/// Measured 2026-08-12: three of the five fields write on THIS fixture, and
/// the two that don't are not being refused — `ScallopConfig` carries neither
/// a depth-per-pass nor a stepover dial (a surface-following finish op steps
/// by scallop chord, not by Z layers or a raster pitch), so
/// `set_depth_per_pass` / `set_stepover` land on nothing. The assertion below
/// separates those cases explicitly, because "the apply was refused" and "the
/// operation had nowhere to put it" are not the same fact and only the first
/// would be a safety property. For a refused pairing that DOES carry a
/// cut-geometry dial see
/// `hazard_ab_refused_pairing_with_a_geometry_dial_takes_the_write` below.
#[test]
fn hazard_a_modal_per_field_apply_writes_the_refused_recipe() {
    let mut wrote: Vec<FeedsField> = Vec::new();
    for field in [
        FeedsField::Rpm,
        FeedsField::Feed,
        FeedsField::Plunge,
        FeedsField::Doc,
        FeedsField::Woc,
    ] {
        let mut controller = controller_with(OperationType::Scallop);
        assert!(panel_recipe(&controller).is_err(), "fixture precondition");
        let before = op_of(&controller);
        let id = id_at(&controller, 0);
        controller.handle_internal_event(AppEvent::ApplyFeedsField {
            toolpath_id: id,
            field,
        });
        let after = op_of(&controller);
        let moved = before.feed_rate() != after.feed_rate()
            || before.plunge_rate() != after.plunge_rate()
            || before.spindle_rpm() != after.spindle_rpm()
            || before.stepover() != after.stepover()
            || before.depth_per_pass() != after.depth_per_pass();
        if moved {
            wrote.push(field);
        }
    }

    assert_eq!(
        wrote,
        vec![FeedsField::Rpm, FeedsField::Feed, FeedsField::Plunge],
        "the set of per-field applies that reach a REFUSED pairing changed"
    );

    // The two that didn't write, didn't write because the dials are absent.
    let controller = controller_with(OperationType::Scallop);
    let op = op_of(&controller);
    assert!(
        op.depth_per_pass().is_none() && op.stepover().is_none(),
        "Scallop grew a cut-geometry dial — the Doc/Woc rows above now need their own \
         refused-pairing assertions instead of this explanation"
    );
}

/// Hazard (a) and hazard (b) in one click. `DropCutter` with a scallop-height
/// target on a flat end mill hits the *second* arm of
/// `validate_tool_for_operation` (`feeds/mod.rs:703-704`: family `Parallel`
/// plus `target_scallop_mm.is_some()`), so the panel refuses it outright — and
/// unlike Scallop, DropCutter carries a real `stepover` dial. The modal
/// therefore previews a recipe the panel will not show and writes it into the
/// cut geometry of an operation the engine has declared unrunnable.
#[test]
fn hazard_ab_refused_pairing_with_a_geometry_dial_takes_the_write() {
    let mut controller = controller_with(OperationType::DropCutter);
    {
        let tc = &mut controller.state.session.toolpath_configs_mut()[0];
        tc.operation.set_scallop_height(0.01);
    }

    let refusal = panel_recipe(&controller)
        .expect_err("flat end mill + scallop-target DropCutter must be refused");
    assert!(
        matches!(
            refusal,
            rs_cam_core::feeds::FeedsError::WrongToolForOperation { .. }
        ),
        "unexpected refusal kind: {refusal:?}"
    );

    let before_woc = op_of(&controller).stepover();
    assert!(
        before_woc.is_some(),
        "fixture must carry a stepover dial for this to mean anything"
    );

    let id = id_at(&controller, 0);
    controller.handle_internal_event(AppEvent::ApplyFeedsField {
        toolpath_id: id,
        field: FeedsField::Woc,
    });
    assert_ne!(
        op_of(&controller).stepover(),
        before_woc,
        "per-field WOC apply left the cut geometry alone on a refused pairing — \
         if the modal now validates, invert this sentry"
    );

    // And the same reach via the single-click path.
    let mut controller = controller_with(OperationType::DropCutter);
    {
        let tc = &mut controller.state.session.toolpath_configs_mut()[0];
        tc.operation.set_scallop_height(0.01);
    }
    let before = op_of(&controller);
    let id = id_at(&controller, 0);
    controller.handle_internal_event(AppEvent::ApplyFeedsAll(id));
    assert_ne!(
        op_of(&controller).stepover(),
        before.stepover(),
        "Apply all left the cut geometry alone on a refused pairing"
    );
}

// ── hazard (b): the modal's Apply all changes cut geometry ─────────────────

/// The panel splits apply in two and says so on the buttons: `⚡⚡ Apply
/// recommended speeds` is documented "Does not change the cut (DOC/WOC)"
/// (`properties/mod.rs:1982-2001`), and `⚡ Apply cut geometry` is separate
/// and attributed (`:2024-2043`). The modal's single `⚡ Apply all` writes
/// both halves. Same fixture, same recommendation, two contracts.
#[test]
fn hazard_b_modal_apply_all_moves_geometry_panel_speeds_apply_does_not() {
    let mut controller = controller_with(OperationType::Pocket);
    let recipe = panel_recipe(&controller).expect("flat end mill on Pocket is a valid pairing");
    let before = op_of(&controller);
    assert!(
        before.stepover().is_some() && before.depth_per_pass().is_some(),
        "fixture op must carry both cut-geometry dials"
    );

    // Panel: speed-only apply on a clone of the same operation.
    let mut panel_op = before.clone();
    let mut panel_prov = rs_cam_core::feeds::FeedsProvenance::default();
    let tool = tool_of(&controller);
    let session_machine = controller.state.session.machine().clone();
    let session_material = controller.state.session.stock_config().material.clone();
    let pass_role = before.feeds_style().1;
    rs_cam_core::feeds::suggest::apply_speeds_to_op(
        &mut panel_op,
        &mut panel_prov,
        &recipe,
        &tool,
        &session_machine,
        &session_material,
        pass_role,
        rs_cam_core::feeds::suggest::SuggestContext::default(),
    );

    // Modal: `⚡ Apply all` through the real handler.
    let id = id_at(&controller, 0);
    controller.handle_internal_event(AppEvent::ApplyFeedsAll(id));
    let modal_op = op_of(&controller);

    assert_eq!(
        panel_op.stepover(),
        before.stepover(),
        "the panel's speeds-apply moved WOC — that contract is supposed to be speed-only"
    );
    assert_eq!(
        panel_op.depth_per_pass(),
        before.depth_per_pass(),
        "the panel's speeds-apply moved DOC — that contract is supposed to be speed-only"
    );

    let geometry_moved = modal_op.stepover() != before.stepover()
        || modal_op.depth_per_pass() != before.depth_per_pass();
    assert!(
        geometry_moved,
        "Apply all left the cut geometry alone on this fixture (WOC {:?}→{:?}, DOC {:?}→{:?}); \
         the divergence is structural (`ApplySubset::Both` vs `Speeds`) but this fixture \
         no longer demonstrates it",
        before.stepover(),
        modal_op.stepover(),
        before.depth_per_pass(),
        modal_op.depth_per_pass()
    );
}

// ── hazard (c): per-field Apply bypasses the invariant funnel ──────────────

/// Not in the review's two named hazards, found while censusing. Every
/// `apply_*_to_op` entry point runs the recommendation through
/// `enforce_invariants` on a scratch clone and rounds it
/// (`feeds/suggest.rs:727-793`) — that is where the plunge-to-feed clamp, the
/// stepover-to-diameter clamp, the rigidity and cutting-length DOC clamps, the
/// deflection back-off and the chipload feed recalibration live.
/// `apply_feeds_field` (`controller/events/mod.rs:796-817`) writes
/// `explain.recommended.*` directly, so **none** of those passes run. The
/// assertion below is bit-exact against the raw preview value, which is the
/// clean structural proof that no funnel stands between them.
#[test]
fn hazard_c_per_field_apply_writes_the_raw_preview_value() {
    let mut controller = controller_with(OperationType::Pocket);
    let preview = modal_preview(&controller);

    let id = id_at(&controller, 0);
    controller.handle_internal_event(AppEvent::ApplyFeedsField {
        toolpath_id: id,
        field: FeedsField::Feed,
    });
    assert_eq!(
        op_of(&controller).feed_rate(),
        preview.recommended.feed_rate_mm_min,
        "per-field Feed apply is expected to be a raw write of the preview value"
    );

    controller.handle_internal_event(AppEvent::ApplyFeedsField {
        toolpath_id: id,
        field: FeedsField::Doc,
    });
    assert_eq!(
        op_of(&controller).depth_per_pass(),
        Some(preview.recommended.axial_depth_mm),
        "per-field DOC apply is expected to be a raw write of the preview value"
    );

    controller.handle_internal_event(AppEvent::ApplyFeedsField {
        toolpath_id: id,
        field: FeedsField::Woc,
    });
    assert_eq!(
        op_of(&controller).stepover(),
        Some(preview.recommended.radial_width_mm),
        "per-field WOC apply is expected to be a raw write of the preview value"
    );
}

/// What hazard (c) costs, in millimetres. Same fixture, same recommendation,
/// two buttons: the modal's per-field DOC Apply and the panel's `⚡ Apply cut
/// geometry`. Measured 2026-08-12 on the default Ø6.35 2-flute flat end mill
/// in a Pocket op — the modal writes **4.445 mm**, the panel writes **1.27
/// mm**. The 3.5× is the DOC clamp/back-off chain inside `enforce_invariants`
/// (`clamp_dpp_to_rigidity`, `clamp_dpp_to_cutting_length`,
/// `backoff_dpp_for_deflection`) that the per-field path never reaches.
///
/// This is the sharpest number in the census: the modal's finest-grained,
/// most innocuous-looking affordance — a small `Apply` next to one row — is
/// the one that writes the deepest unguarded cut.
#[test]
fn hazard_c_per_field_doc_is_3x_the_funnelled_doc() {
    let mut controller = controller_with(OperationType::Pocket);
    let recipe = panel_recipe(&controller).expect("valid pairing");

    // Panel: `⚡ Apply cut geometry` (`properties/mod.rs:2032`).
    let mut panel_op = op_of(&controller);
    let mut prov = rs_cam_core::feeds::FeedsProvenance::default();
    let tool = tool_of(&controller);
    let machine = controller.state.session.machine().clone();
    let material = controller.state.session.stock_config().material.clone();
    let pass_role = panel_op.feeds_style().1;
    rs_cam_core::feeds::suggest::apply_cut_geometry_to_op(
        &mut panel_op,
        &mut prov,
        &recipe,
        &tool,
        &machine,
        &material,
        pass_role,
        rs_cam_core::feeds::suggest::SuggestContext::default(),
    );

    // Modal: the per-row `Apply` on the DOC line.
    let id = id_at(&controller, 0);
    controller.handle_internal_event(AppEvent::ApplyFeedsField {
        toolpath_id: id,
        field: FeedsField::Doc,
    });

    let modal_doc = op_of(&controller).depth_per_pass().expect("pocket has DOC");
    let panel_doc = panel_op.depth_per_pass().expect("pocket has DOC");
    assert!(
        modal_doc > panel_doc * 3.0,
        "the per-field DOC apply no longer overshoots the funnelled DOC by >3× \
         (modal {modal_doc} mm vs panel {panel_doc} mm) — if the funnel now covers \
         the per-field path this sentry should be inverted, not relaxed"
    );
}

/// The funnelled counterpart, for contrast: the panel's cut-geometry apply on
/// the same fixture writes the *rounded, invariant-resolved* value.
#[test]
fn panel_cut_geometry_apply_goes_through_the_invariant_funnel() {
    let controller = controller_with(OperationType::Pocket);
    let recipe = panel_recipe(&controller).expect("valid pairing");
    let mut op = op_of(&controller);
    let mut prov = rs_cam_core::feeds::FeedsProvenance::default();
    let tool = tool_of(&controller);
    let machine = controller.state.session.machine().clone();
    let material = controller.state.session.stock_config().material.clone();
    let pass_role = op.feeds_style().1;

    rs_cam_core::feeds::suggest::apply_cut_geometry_to_op(
        &mut op,
        &mut prov,
        &recipe,
        &tool,
        &machine,
        &material,
        pass_role,
        rs_cam_core::feeds::suggest::SuggestContext::default(),
    );

    let dpp = op.depth_per_pass().expect("pocket carries DOC");
    let rounded = rs_cam_core::feeds::suggest::round_suggestion_value(dpp, 0.001);
    assert!(
        (dpp - rounded).abs() < 1e-12,
        "panel-applied DOC {dpp} is not on the funnel's 0.001 mm grid"
    );
}

// ── the project-wide reach of the modal's infallible contract ──────────────

/// `⚡⚡ Apply all toolpaths` (`feeds_modal.rs:2766-2772`) fans
/// `ApplyFeedsAll` over **every enabled toolpath** (`events/mod.rs:939-951`),
/// so one click applies the unvalidated, geometry-changing contract to a
/// refused pairing sitting anywhere in the project — with no per-row refusal
/// surfaced, because the loop calls the infallible path per id.
#[test]
fn project_apply_all_reaches_a_refused_toolpath_silently() {
    let mut controller = controller_with(OperationType::Pocket);
    controller
        .state
        .session
        .add_toolpath(0, toolpath(1, OperationType::Scallop))
        .expect("add second toolpath");

    let before = controller.state.session.toolpath_configs()[1]
        .operation
        .clone();
    controller.handle_internal_event(AppEvent::ApplyFeedsProject);
    let after = controller.state.session.toolpath_configs()[1]
        .operation
        .clone();

    assert_ne!(
        before.feed_rate(),
        after.feed_rate(),
        "project-wide apply skipped the refused toolpath — if it now refuses per row, \
         invert this sentry"
    );
}
