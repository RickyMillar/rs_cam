//! A-3/A-4 — the apply contract: characterized, then fixed.
//!
//! The operator's 2026-08-07 feeds/speeds architecture review raised a [high]
//! finding: the Feeds & Speeds *modal* and the properties *panel* were two
//! user-visible "apply the recommendation" surfaces with different validation
//! and different cut-geometry effects. A-3 censused every write path
//! (`planning/review_2026-08-08/APPLY_CONTRACT_CENSUS.md` — 20 paths, 13 GUI
//! apply affordances, 2 validated, 7 bypassing the invariant funnel entirely)
//! and pinned the behaviour with nine characterization tests. Checkpoint I
//! ruled on 2026-08-12; A-4 executed it and **inverted these tests in place**,
//! per I-6. The measured pre-fix numbers stay in the doc comments — they are
//! the permanent reproduction plan §0.1 requires, and the bars A-4 was
//! checked against.
//!
//! What the fix is, in one paragraph. `FeedsExplain` is an infallible chart
//! payload; it was never meant to be a write source, and it has no slot in
//! which to say "this tool cannot run this operation", so every surface
//! holding one could write a recipe the engine had refused. It is now wrapped
//! by `feeds::suggest::FeedsPreview`, which carries the refusal alongside the
//! numbers and hands out an `ApplicableRecommendation` — the sole input to the
//! sole write function, `feeds::suggest::apply` — only when validation
//! succeeded. The six per-field modal Apply buttons were deleted outright
//! (I-1); the batch paths and the explore apply were rerouted through the
//! funnel with explicit `ApplyScope`s and "changes the cut" attribution.
//!
//! Evidence class: controller-level integration, plus two source-level
//! sentries. These drive the production event handlers with a scripted compute
//! backend; they do not render egui. The button → `AppEvent` mapping is
//! therefore read from source (cited at each test) rather than clicked; the
//! rendered surfaces are captured as screenshots in
//! `planning/review_2026-08-08/artifacts/a4/`.

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
use rs_cam_viz::ui::AppEvent;

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

/// The modal's entry point since A-4 — `ui/feeds_modal.rs::compute_preview`,
/// and the same call the apply handlers make internally
/// (`controller/events/mod.rs::apply_feeds_through_funnel`). Pre-fix this was
/// `feeds_explain_for_operation`, which is infallible and therefore could not
/// tell the modal that the pairing had been refused.
fn modal_preview(
    controller: &AppController<SilentBackend>,
) -> rs_cam_core::feeds::suggest::FeedsPreview {
    let session = &controller.state.session;
    let stock = session.stock_config();
    rs_cam_core::feeds::suggest::feeds_preview_for_operation(
        &op_of(controller),
        &tool_of(controller),
        &stock.material,
        session.machine(),
        stock.workholding_rigidity,
        rs_cam_core::feeds::embedded_vendor_lut(),
        session.post_config().spindle_strategy,
    )
}

/// Production sources these tests make source-level assertions against. Read
/// at compile time, so a reintroduction of a deleted affordance breaks the
/// build of this file rather than sliding past a runtime check.
const UI_MOD_SRC: &str = include_str!("../src/ui/mod.rs");
const COMPARE_SRC: &str = include_str!("../src/ui/components/compare.rs");
const FEEDS_MODAL_SRC: &str = include_str!("../src/ui/feeds_modal.rs");
const EVENTS_SRC: &str = include_str!("../src/controller/events/mod.rs");

// ── the two surfaces now agree on a refused pairing ────────────────────────

/// The fixture the whole finding rested on: a flat end mill on a Scallop op.
/// `validate_tool_for_operation` (`feeds/mod.rs:702`) refuses it — a zero tip
/// radius makes the scallop-stepover formula `2·√(2·R·h − h²)` undefined.
///
/// **Pre-fix (measured 2026-08-12):** the panel got
/// `Err(WrongToolForOperation { operation: Scallop, actual_geometry: Flat, … })`
/// and rendered no apply affordance at all, while the modal — reading the
/// infallible `explain` payload — showed feed **2677.07 mm/min**, plunge
/// **793.75**, RPM **10025.5** with live Apply buttons beside them.
///
/// **Post-fix:** the numbers are still shown (Checkpoint I-3 keeps the
/// explanatory job: the modal opens, the charts draw), but the preview yields
/// **nothing writable**. That last assertion is the inversion — it is the
/// structural guarantee, and it is the one that did not exist before.
#[test]
fn panel_and_modal_agree_on_a_refused_pairing() {
    let controller = controller_with(OperationType::Scallop);

    let refusal = panel_recipe(&controller).expect_err(
        "flat end mill on Scallop must be refused by the validated panel path — if this \
         now succeeds the validator changed and the whole fixture needs rebasing",
    );
    assert!(
        matches!(
            refusal,
            rs_cam_core::feeds::FeedsError::WrongToolForOperation { .. }
        ),
        "unexpected refusal kind: {refusal:?}"
    );

    let preview = modal_preview(&controller);
    // The explanation survives — this is I-3, not a silent suppression.
    assert!(
        preview.recommended().feed_rate_mm_min > 0.0,
        "the preview stopped producing numbers on a refused pairing; I-3 requires the \
         charts to keep drawing so the operator can see WHY"
    );
    // …but it is not applicable, and the refusal is legible.
    assert!(
        preview.applicable().is_none(),
        "a refused preview handed out a writable recommendation — the funnel's only \
         structural guarantee has been lost"
    );
    assert!(
        matches!(
            preview.refusal(),
            Some(rs_cam_core::feeds::FeedsError::WrongToolForOperation { .. })
        ),
        "the preview did not carry the panel's refusal: {:?}",
        preview.refusal()
    );
}

/// `⚡ Apply all` (`ui/feeds_modal.rs::draw_apply_column` →
/// `AppEvent::ApplyFeedsAll`) on a pairing the panel refuses.
///
/// **Pre-fix (measured 2026-08-12):** it wrote the refused recipe straight in
/// — feed **1000 → 2677**, plunge **500 → 794**, RPM **None → Some(10026)** —
/// on a toolpath where the panel rendered `Feeds unavailable: …` and offered
/// no button at all.
///
/// **Post-fix:** nothing moves. The modal does not draw the button on a
/// refused pairing (I-3), and if the event arrives anyway — the project state
/// can change under an open modal — the handler refuses and notifies.
#[test]
fn modal_apply_all_refuses_the_pairing_the_panel_refuses() {
    let mut controller = controller_with(OperationType::Scallop);
    assert!(panel_recipe(&controller).is_err(), "fixture precondition");

    let before = op_of(&controller);
    let id = id_at(&controller, 0);
    controller.handle_internal_event(AppEvent::ApplyFeedsAll(id));
    let after = op_of(&controller);

    assert_eq!(
        before.feed_rate(),
        after.feed_rate(),
        "Apply all wrote feed on a pairing the panel refuses (pre-fix: 1000 → 2677)"
    );
    assert_eq!(
        before.plunge_rate(),
        after.plunge_rate(),
        "Apply all wrote plunge on a refused pairing (pre-fix: 500 → 794)"
    );
    assert_eq!(
        before.spindle_rpm(),
        after.spindle_rpm(),
        "Apply all wrote RPM on a refused pairing (pre-fix: None → Some(10026))"
    );
    // The user is told, rather than left with a button that did nothing.
    assert!(
        controller
            .active_notifications()
            .any(|n| n.message.contains("Feeds not applied")),
        "the refusal was silent — a no-op button is its own defect"
    );
}

/// The six per-field Apply buttons (census rows M1–M6) are **gone**, and their
/// absence is the fix for the sharpest number in the census.
///
/// **Pre-fix (measured 2026-08-12):** `ui/components/compare.rs:139-146`
/// rendered a `small_button("Apply")` in every comparison row, pushing
/// `AppEvent::ApplyFeedsField`, whose handler
/// (`controller/events/mod.rs:796-817`) wrote `explain.recommended.*` directly
/// — no `validate_tool_for_operation`, and none of `enforce_invariants`'
/// passes: not `clamp_plunge_to_feed`, `clamp_stepover_to_diameter`,
/// `backoff_stepover_for_runtime`, `clamp_dpp_to_rigidity`,
/// `clamp_dpp_to_cutting_length`, `backoff_dpp_for_deflection`,
/// `recalibrate_feed_for_chipload`, nor `round_suggestion_value`. On the
/// Scallop fixture three of five wrote (RPM, Feed, Plunge) and the other two
/// wrote nothing only because `ScallopConfig` carries neither dial — an absent
/// dial, not a guard.
///
/// This test cannot dispatch the event any more, because the event does not
/// exist: that is the strongest form of the assertion and it is enforced by
/// the compiler. What is checked here is that nothing grew back — the enum,
/// the event, the builder and the handler are all absent from the production
/// sources.
#[test]
fn per_field_apply_affordance_no_longer_exists() {
    assert!(
        !UI_MOD_SRC.contains("ApplyFeedsField {\n        toolpath_id"),
        "AppEvent::ApplyFeedsField was reintroduced in ui/mod.rs"
    );
    assert!(
        !UI_MOD_SRC.contains("pub enum FeedsField"),
        "the viz-side FeedsField enum was reintroduced in ui/mod.rs"
    );
    assert!(
        !COMPARE_SRC.contains("small_button(\"Apply\")"),
        "a per-row Apply button was reintroduced in the compare component — it is the \
         affordance that wrote 4.445 mm of DOC where the funnel writes 1.27 mm"
    );
    assert!(
        !FEEDS_MODAL_SRC.contains("ApplyFeedsField"),
        "the feeds modal pushes a per-field apply event again"
    );
    assert!(
        !EVENTS_SRC.contains("fn apply_feeds_field"),
        "the per-field apply handler was reintroduced in the controller"
    );
    // And the surviving modal write is the funnel, not a setter.
    assert!(
        EVENTS_SRC.contains("fn apply_feeds_through_funnel"),
        "the funnel entry point is gone — every apply must route through it"
    );
}

/// A refused pairing that *does* carry a cut-geometry dial: `DropCutter` with
/// a scallop-height target on a flat end mill trips the **second** arm of
/// `validate_tool_for_operation` (`feeds/mod.rs:703-704`: family `Parallel`
/// plus `target_scallop_mm.is_some()`), and unlike Scallop it has a real
/// `stepover`.
///
/// **Pre-fix (measured 2026-08-12):** the per-field WOC Apply wrote
/// **1.0 → 0.1905 mm** and `⚡ Apply all` wrote **1.0 → 0.19 mm** — a 5.3×
/// finer raster, i.e. a runtime multiplier, written into the cut geometry of
/// an operation the engine says cannot be run at all.
///
/// **Post-fix:** the per-field button is gone and `⚡ Apply all` refuses, so
/// the cut geometry of a refused pairing is untouchable from the modal.
#[test]
fn refused_pairing_with_a_geometry_dial_takes_no_write() {
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

    let before = op_of(&controller);
    assert!(
        before.stepover().is_some(),
        "fixture must carry a stepover dial for this to mean anything"
    );
    assert!(
        modal_preview(&controller).applicable().is_none(),
        "the preview is willing to write to a refused DropCutter"
    );

    let id = id_at(&controller, 0);
    controller.handle_internal_event(AppEvent::ApplyFeedsAll(id));
    assert_eq!(
        op_of(&controller).stepover(),
        before.stepover(),
        "Apply all rewrote the cut geometry of a refused pairing (pre-fix: 1.0 → 0.19 mm)"
    );
    assert_eq!(
        op_of(&controller).feed_rate(),
        before.feed_rate(),
        "Apply all rewrote the feed of a refused pairing (pre-fix: 1000 → 2450)"
    );
}

// ── the modal and the panel write the same numbers ─────────────────────────

/// Hazard (b) was that the modal's one button moved the cut where the panel's
/// speeds button promised not to, with no way for the user to tell.
///
/// **Pre-fix (measured 2026-08-12), Pocket fixture, one recommendation:**
///
/// | | feed | plunge | RPM | WOC | DOC |
/// |---|---|---|---|---|---|
/// | before | 1000 | 500 | None | 2.0 | 1.5 |
/// | panel `⚡⚡ Apply recommended speeds` | 3000 | 794 | 18000 | 2.0 | 1.5 |
/// | modal `⚡ Apply all` | 3000 | 794 | 18000 | **2.222** | **1.27** |
///
/// **Post-fix:** the numbers are unchanged — Checkpoint I-1 kept the combined
/// apply, it did not neuter it — but the divergence is no longer silent on two
/// counts. The button now reads `⚡ Apply all — changes the cut` and says so
/// again in its tooltip, and the values it writes are asserted here to be
/// **exactly** the panel's speeds-apply plus the panel's cut-geometry apply.
/// One recommendation, one funnel, two spellings of the same result.
#[test]
fn modal_apply_all_writes_what_the_panel_writes() {
    let mut controller = controller_with(OperationType::Pocket);
    let recipe = panel_recipe(&controller).expect("flat end mill on Pocket is a valid pairing");
    let before = op_of(&controller);
    assert!(
        before.stepover().is_some() && before.depth_per_pass().is_some(),
        "fixture op must carry both cut-geometry dials"
    );

    // Panel: both buttons, on a clone of the same operation.
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
    let speeds_only_woc = panel_op.stepover();
    let speeds_only_doc = panel_op.depth_per_pass();
    rs_cam_core::feeds::suggest::apply_cut_geometry_to_op(
        &mut panel_op,
        &mut panel_prov,
        &recipe,
        &tool,
        &session_machine,
        &session_material,
        pass_role,
        rs_cam_core::feeds::suggest::SuggestContext::default(),
    );

    // The panel's speeds-apply still keeps its own promise.
    assert_eq!(
        speeds_only_woc,
        before.stepover(),
        "the panel's speeds-apply moved WOC — that contract is supposed to be speed-only"
    );
    assert_eq!(
        speeds_only_doc,
        before.depth_per_pass(),
        "the panel's speeds-apply moved DOC — that contract is supposed to be speed-only"
    );

    // Modal: `⚡ Apply all` through the real handler.
    let id = id_at(&controller, 0);
    controller.handle_internal_event(AppEvent::ApplyFeedsAll(id));
    let modal_op = op_of(&controller);

    for (name, a, b) in [
        ("feed", modal_op.feed_rate(), panel_op.feed_rate()),
        ("plunge", modal_op.plunge_rate(), panel_op.plunge_rate()),
    ] {
        assert_eq!(a, b, "{name}: modal {a} vs panel {b}");
    }
    assert_eq!(modal_op.spindle_rpm(), panel_op.spindle_rpm(), "rpm");
    assert_eq!(modal_op.stepover(), panel_op.stepover(), "woc");
    assert_eq!(modal_op.depth_per_pass(), panel_op.depth_per_pass(), "doc");
    // And the attribution the divergence needed is on the button's face.
    assert!(
        FEEDS_MODAL_SRC.contains("⚡ Apply all — changes the cut"),
        "the combined apply lost its 'changes the cut' attribution; it moves DOC/WOC and \
         the operator has to be able to see that before clicking"
    );
}

// ── no surviving path writes the raw preview value ─────────────────────────

/// **Pre-fix (measured 2026-08-12):** `apply_feeds_field` wrote
/// `explain.recommended.*` bit-exactly into the operation — the clean
/// structural proof that no funnel stood between the preview and the write.
/// The same was true of the explore-chart apply, which set feed and RPM with
/// bare setters.
///
/// **Post-fix:** every surviving GUI apply goes through
/// `apply_feeds_through_funnel`, so the written value is the
/// *invariant-resolved, rounded* one. This test asserts the negative directly:
/// after `⚡ Apply all` on the Pocket fixture, the operation's DOC is **not**
/// the raw preview number, and it **is** the funnel's number.
#[test]
fn no_apply_path_writes_the_raw_preview_value() {
    let mut controller = controller_with(OperationType::Pocket);
    let raw = modal_preview(&controller).recommended().clone();

    let id = id_at(&controller, 0);
    controller.handle_internal_event(AppEvent::ApplyFeedsAll(id));
    let applied = op_of(&controller);

    assert_ne!(
        applied.depth_per_pass(),
        Some(raw.axial_depth_mm),
        "the raw preview DOC ({}) reached the operation — some path is still bypassing \
         enforce_invariants",
        raw.axial_depth_mm
    );
    // Feed and WOC agree with the raw value to within rounding; DOC is where
    // the clamp chain bites. Assert the rounding grid rather than equality, so
    // this stays a statement about the funnel and not about one number.
    for (name, applied_v) in [
        ("woc", applied.stepover()),
        ("doc", applied.depth_per_pass()),
    ] {
        let v = applied_v.expect("pocket carries both dials");
        let rounded = rs_cam_core::feeds::suggest::round_suggestion_value(v, 0.001);
        assert!(
            (v - rounded).abs() < 1e-12,
            "applied {name} {v} is not on the funnel's 0.001 mm grid"
        );
    }
    // The controller source no longer contains a direct recommendation write.
    assert!(
        !EVENTS_SRC.contains("set_depth_per_pass(r.axial_depth_mm)"),
        "a direct DOC write from the recommendation was reintroduced"
    );
    assert!(
        !EVENTS_SRC.contains("set_stepover(r.radial_width_mm)"),
        "a direct WOC write from the recommendation was reintroduced"
    );
}

/// **The bar this wave was measured against.** A-3 §3.4 measured the modal's
/// per-field DOC Apply at **4.445 mm** against the panel's `⚡ Apply cut
/// geometry` at **1.27 mm** on the default Ø6.35 2-flute flat end mill in a
/// Pocket op — **3.50×**, the calculator's raw axial recommendation versus
/// what survives `clamp_dpp_to_rigidity`, `clamp_dpp_to_cutting_length` and
/// `backoff_dpp_for_deflection` at 45 mm stickout.
///
/// Checkpoint I's bar 3 is that the gap closes to **1.00×**: the surviving
/// modal write paths produce exactly what the panel produces. The affordance
/// that produced 4.445 no longer exists, so what is checkable — and what is
/// checked here — is that the modal's remaining DOC write is bit-equal to the
/// panel's.
#[test]
fn surviving_apply_paths_produce_the_funnelled_doc_exactly() {
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

    // Modal: the only DOC write it still has.
    let id = id_at(&controller, 0);
    controller.handle_internal_event(AppEvent::ApplyFeedsAll(id));

    let modal_doc = op_of(&controller).depth_per_pass().expect("pocket has DOC");
    let panel_doc = panel_op.depth_per_pass().expect("pocket has DOC");
    assert_eq!(
        modal_doc.to_bits(),
        panel_doc.to_bits(),
        "modal DOC {modal_doc} mm vs panel DOC {panel_doc} mm — the ratio is {:.3}×, and \
         Checkpoint I's bar 3 is exactly 1.000×",
        modal_doc / panel_doc
    );
}

/// The funnelled counterpart, unchanged from A-3: the panel's cut-geometry
/// apply writes the rounded, invariant-resolved value. Kept as-is because it
/// was always a statement of correct behaviour — it is the surface the modal
/// has now been made to match, so it is the one that must not move.
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

/// **Checkpoint I bar 2 — the fingerprint that must not move.** A-3 measured
/// the Pocket fixture's applied operating point as
/// **3000 / 794 / 18000 / 2.222 / 1.27** (feed / plunge / RPM / WOC / DOC)
/// through the panel's two buttons. A-4 rewrote the modal and the controller
/// but was forbidden from moving a recipe number; a moved fingerprint is a
/// STOP under plan §0.2, not a re-pin.
///
/// This pins all five through the validated panel path, which is the path
/// whose numbers the census recorded.
#[test]
fn pocket_fixture_recipe_fingerprint_is_unmoved() {
    let controller = controller_with(OperationType::Pocket);
    let recipe = panel_recipe(&controller).expect("valid pairing");
    let mut op = op_of(&controller);
    let mut prov = rs_cam_core::feeds::FeedsProvenance::default();
    let tool = tool_of(&controller);
    let machine = controller.state.session.machine().clone();
    let material = controller.state.session.stock_config().material.clone();
    let pass_role = op.feeds_style().1;
    let ctx = || rs_cam_core::feeds::suggest::SuggestContext::default();

    rs_cam_core::feeds::suggest::apply_speeds_to_op(
        &mut op,
        &mut prov,
        &recipe,
        &tool,
        &machine,
        &material,
        pass_role,
        ctx(),
    );
    rs_cam_core::feeds::suggest::apply_cut_geometry_to_op(
        &mut op,
        &mut prov,
        &recipe,
        &tool,
        &machine,
        &material,
        pass_role,
        ctx(),
    );

    assert_eq!(op.feed_rate(), 3000.0, "feed");
    assert_eq!(op.plunge_rate(), 794.0, "plunge");
    assert_eq!(op.spindle_rpm(), Some(18_000), "rpm");
    assert_eq!(op.stepover(), Some(2.222), "woc");
    assert_eq!(op.depth_per_pass(), Some(1.27), "doc");
}

// ── the project batch is allowed to be partial, never quiet ────────────────

/// `⚡⚡ Apply all toolpaths` (`feeds_modal.rs`) fans `ApplyFeedsAll` over
/// every enabled toolpath.
///
/// **Pre-fix (measured 2026-08-12):** the loop called the infallible path per
/// id, so a refused pairing sitting anywhere in the project took the write
/// **silently**, inside a batch the user believed they understood — the second
/// toolpath's feed moved 1000 → 2677 with no row-level signal anywhere.
///
/// **Post-fix:** the refused row is skipped, and the batch says which rows it
/// skipped and why. A partial batch is fine; a quiet one is not.
#[test]
fn project_apply_all_skips_a_refused_toolpath_and_reports_it() {
    let mut controller = controller_with(OperationType::Pocket);
    controller
        .state
        .session
        .add_toolpath(0, toolpath(1, OperationType::Scallop))
        .expect("add second toolpath");

    let before_valid = controller.state.session.toolpath_configs()[0]
        .operation
        .clone();
    let before_refused = controller.state.session.toolpath_configs()[1]
        .operation
        .clone();
    controller.handle_internal_event(AppEvent::ApplyFeedsProject);
    let after_valid = controller.state.session.toolpath_configs()[0]
        .operation
        .clone();
    let after_refused = controller.state.session.toolpath_configs()[1]
        .operation
        .clone();

    assert_eq!(
        before_refused.feed_rate(),
        after_refused.feed_rate(),
        "the project-wide apply wrote to the refused toolpath (pre-fix: 1000 → 2677)"
    );
    // The batch still does its job on the rows it can.
    assert_ne!(
        before_valid.feed_rate(),
        after_valid.feed_rate(),
        "the batch skipped the VALID row too — refusing everything is not the fix"
    );
    let reported = controller
        .active_notifications()
        .any(|n| n.message.contains("skipped") && n.message.contains("Scallop"));
    assert!(
        reported,
        "the batch skipped a row without naming it; messages were {:?}",
        controller
            .active_notifications()
            .map(|n| n.message.clone())
            .collect::<Vec<_>>()
    );
}

// ── the explore apply joins the funnel ─────────────────────────────────────

/// The drag-to-explore apply (census row M8) was the seventh raw write: it set
/// feed and RPM with bare setters, so a feed dragged below the operation's
/// plunge rate left the machine plunging faster than it cut.
///
/// Checkpoint I-1 routed it through the funnel with `ApplyScope::Speeds`. Two
/// properties have to hold together, and this test asserts both: the operator's
/// dragged numbers survive (a funnel that silently re-solves them would be a
/// new defect, not a fix), and the clamps run on top of them.
#[test]
fn explore_apply_takes_the_clamps_but_keeps_the_dragged_point() {
    let mut controller = controller_with(OperationType::Pocket);
    let id = id_at(&controller, 0);
    {
        let tc = &mut controller.state.session.toolpath_configs_mut()[0];
        tc.operation.set_plunge_rate(900.0);
    }
    let before_geometry = (
        op_of(&controller).stepover(),
        op_of(&controller).depth_per_pass(),
    );

    controller.handle_internal_event(AppEvent::ApplyFeedsExplore {
        toolpath_id: id,
        feed_mm_min: 120.0,
        rpm: 14_000.0,
    });
    let after = op_of(&controller);

    assert_eq!(after.feed_rate(), 120.0, "the dragged feed was re-solved");
    assert_eq!(after.spindle_rpm(), Some(14_000), "the dragged RPM moved");
    assert_eq!(
        after.plunge_rate(),
        120.0,
        "plunge was not clamped down to the dragged feed — pre-fix this left plunge at \
         900 mm/min under a 120 mm/min cut"
    );
    assert_eq!(
        (after.stepover(), after.depth_per_pass()),
        before_geometry,
        "a Speeds-scoped apply moved the cut geometry"
    );
}

/// The explore apply is an apply, so it answers to the same refusal rule
/// (I-3): on a pairing the panel refuses, the modal draws the refusal in place
/// of the button, and the handler refuses if the event arrives anyway.
#[test]
fn explore_apply_refuses_a_refused_pairing() {
    let mut controller = controller_with(OperationType::Scallop);
    let id = id_at(&controller, 0);
    let before = op_of(&controller);

    controller.handle_internal_event(AppEvent::ApplyFeedsExplore {
        toolpath_id: id,
        feed_mm_min: 2500.0,
        rpm: 20_000.0,
    });
    let after = op_of(&controller);

    assert_eq!(before.feed_rate(), after.feed_rate(), "explore wrote feed");
    assert_eq!(
        before.spindle_rpm(),
        after.spindle_rpm(),
        "explore wrote RPM"
    );
    assert!(
        controller
            .active_notifications()
            .any(|n| n.message.contains("Explored values not applied")),
        "the explore refusal was silent"
    );
}
