//! G-STALEXPORT sentry (F2.3, 2026-09-10) — an operation EDITED after it
//! was generated refuses the export by name; its previous geometry is
//! emitted only when the operator explicitly accepts it.
//!
//! `io::export::emitted_result_toolpath` read `session.results` first and
//! `gui.toolpath_rt[id].result` — the compute worker's output, kept so the
//! viewport can draw the old path — second, silently. That fallback was
//! written for one narrow case (a GUI tool edit dropped the core result,
//! see the G-MODEXPORT note). G-FRESHSTATE (F2.1) then made EVERY input
//! edit drop the core result, which turned a narrow fallback into the
//! normal path: edit a stepover, export, and the file was the geometry
//! from before the edit, with nothing on any surface saying so.
//!
//! Drawing the previous geometry and CUTTING it are different
//! permissions. What this file pins:
//!
//! 1. Editing a generated operation makes every export entry point refuse
//!    by name, and no program string exists for a caller to write.
//! 2. The refusal says what happened and what the remedy is — it does not
//!    read as "not generated", which is a different situation.
//! 3. Under `StaleResultPolicy::AcceptPreviousGeometry` the same export
//!    succeeds and emits the previous generation's geometry.
//! 4. The acceptance waives ONLY an edit. A missing result is still a
//!    refusal under it, because there is no geometry to put in its place.
//! 5. The pre-flight rows and the export refusal come from one builder,
//!    so the modal and the file dialog cannot disagree.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::PathBuf;
use std::sync::Arc;

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
use rs_cam_viz::error::VizError;
use rs_cam_viz::io::export::{
    blocking_toolpath_message, blocking_toolpaths, export_combined_gcode_from_session,
    export_gcode_from_session_with_policy, export_setup_gcode_from_session_with_policy,
    export_single_toolpath_from_session,
};
use rs_cam_viz::state::freshness::{FreshnessState, freshness_at};
use rs_cam_viz::state::job::SetupId;
use rs_cam_viz::state::runtime::{ComputeStatus, GuiState, StaleResultPolicy, ToolpathRuntime};
use rs_cam_viz::state::simulation::SimulationState;
use rs_cam_viz::state::toolpath::{OperationConfig, ToolpathResult};

const OP_NAME: &str = "Finish Pass";
/// Feed in the geometry the operator last generated.
const PREVIOUS_FEED: f64 = 600.0;

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

/// A one-operation project whose operation is GENERATED: a result in the
/// core cache and a drawable copy in the viz store, exactly as
/// `drain_compute_results` leaves it.
fn build_state() -> (ProjectSession, GuiState, SimulationState) {
    let mut session = ProjectSessionBuilder::new()
        .tool(ToolConfig::new_default(ToolId(1), ToolType::EndMill))
        .build();
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

    let tc = ToolpathConfig {
        id: rs_cam_core::ToolpathId(0),
        name: OP_NAME.to_owned(),
        enabled: true,
        operation: OperationConfig::Scallop(rs_cam_core::compute::ScallopConfig::default()),
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
        face_selection: None,
        debug_options: Default::default(),
        feeds_provenance: Default::default(),
        rest_analysis: Default::default(),
        planner_origin: None,
    };
    let id = tc.id;
    let _ = session.add_toolpath(0, tc).expect("add toolpath");
    let revision = session.toolpath_revision(0);
    let _ = session
        .apply(Command::AdoptResult(AdoptResultArgs {
            index: 0,
            revision,
            result: Box::new(core_result(sample_toolpath(PREVIOUS_FEED))),
        }))
        .expect("seed the core result");

    let mut gui = GuiState::new();
    let mut rt = ToolpathRuntime::new(true);
    rt.status = ComputeStatus::Done;
    rt.result = Some(viz_result(sample_toolpath(PREVIOUS_FEED)));
    gui.toolpath_rt.insert(id, rt);

    (session, gui, SimulationState::new())
}

/// What an operator edit does now: the core result dies, the viz copy
/// stays so the viewport can draw the old path. Reached here through the
/// core door the GUI panel calls (`invalidate_toolpath_inputs`).
fn edit_the_operation(session: &mut ProjectSession) {
    let _ = session.invalidate_toolpath_inputs(0);
    assert!(
        session.get_result(0).is_none(),
        "the edit must drop the core result — otherwise this file is not \
         exercising the gate"
    );
}

fn expect_export_error<T>(result: Result<T, VizError>, what: &str) -> String {
    match result {
        Ok(_) => panic!("{what} must refuse an operation edited since generation"),
        Err(e) => e.to_string(),
    }
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

// ── 1. The refusal, on every entry point ────────────────────────────────

#[test]
fn export_refuses_an_operation_edited_since_generation() {
    let (mut session, gui, sim) = build_state();

    // Before the edit the export succeeds — otherwise a refusal below
    // would prove nothing about the edit.
    export_gcode_from_session_with_policy(
        &session,
        &gui,
        &sim,
        policy(),
        StaleResultPolicy::Refuse,
    )
    .expect("a generated operation exports");

    edit_the_operation(&mut session);
    assert_eq!(
        freshness_at(&session, &gui, 0),
        Some(FreshnessState::EditedSince),
        "the fixture must be in the state this file is about"
    );

    let msg = expect_export_error(
        export_gcode_from_session_with_policy(
            &session,
            &gui,
            &sim,
            policy(),
            StaleResultPolicy::Refuse,
        ),
        "export_gcode_from_session_with_policy",
    );
    assert!(
        msg.contains(&format!("'{OP_NAME}' was edited after it was generated")),
        "the refusal must name the operation and say what happened; got: {msg}"
    );
    assert!(
        msg.contains("regenerate it"),
        "the refusal must state the remedy; got: {msg}"
    );
    assert!(
        !msg.contains("is not generated"),
        "an EDITED operation is not the same situation as an ungenerated \
         one and must not borrow its sentence; got: {msg}"
    );
}

#[test]
fn every_export_entry_point_refuses_the_edited_operation() {
    let (mut session, gui, sim) = build_state();
    edit_the_operation(&mut session);

    let combined = expect_export_error(
        export_combined_gcode_from_session(&session, &gui, &sim),
        "export_combined_gcode_from_session",
    );
    let per_setup = expect_export_error(
        export_setup_gcode_from_session_with_policy(
            &session,
            &gui,
            &sim,
            SetupId(0),
            policy(),
            StaleResultPolicy::Refuse,
        ),
        "export_setup_gcode_from_session_with_policy",
    );
    let single = expect_export_error(
        export_single_toolpath_from_session(&session, &gui, &sim, rs_cam_core::ToolpathId(0)),
        "export_single_toolpath_from_session",
    );

    for (what, msg) in [
        ("combined", &combined),
        ("per-setup", &per_setup),
        ("single", &single),
    ] {
        assert!(
            msg.contains(&format!("'{OP_NAME}' was edited after it was generated")),
            "[{what}] every entry point prints the one shared sentence; got: {msg}"
        );
    }
}

// ── 2. The acceptance ───────────────────────────────────────────────────

#[test]
fn accepting_the_previous_geometry_exports_it() {
    let (mut session, gui, sim) = build_state();
    edit_the_operation(&mut session);

    let gcode = export_gcode_from_session_with_policy(
        &session,
        &gui,
        &sim,
        policy(),
        StaleResultPolicy::AcceptPreviousGeometry,
    )
    .expect("the acceptance emits the previous geometry");

    assert_eq!(
        f_words(&gcode),
        vec![PREVIOUS_FEED],
        "what the acceptance emits is the geometry from the last \
         generation, held in the viz store"
    );
}

/// The acceptance is scoped to ONE situation. An operation with no result
/// anywhere still refuses under it, because accepting previous geometry
/// cannot conjure geometry that was never produced.
#[test]
fn the_acceptance_does_not_waive_a_missing_result() {
    let (mut session, mut gui, sim) = build_state();
    let _ = session.remove_result(0);
    if let Some(rt) = gui.toolpath_rt.get_mut(&rs_cam_core::ToolpathId(0)) {
        rt.result = None;
        rt.status = ComputeStatus::Pending;
    }
    assert_eq!(
        freshness_at(&session, &gui, 0),
        Some(FreshnessState::NoResult)
    );

    let msg = expect_export_error(
        export_gcode_from_session_with_policy(
            &session,
            &gui,
            &sim,
            policy(),
            StaleResultPolicy::AcceptPreviousGeometry,
        ),
        "export with an ungenerated op under the acceptance",
    );
    assert!(
        msg.contains(&format!("'{OP_NAME}' is not generated")),
        "a missing result keeps its own refusal under the acceptance; got: {msg}"
    );

    let rows = blocking_toolpaths(
        &session,
        &gui,
        0..session.toolpath_configs().len(),
        StaleResultPolicy::AcceptPreviousGeometry,
    );
    assert_eq!(rows.len(), 1);
    assert!(
        !rows[0].waived_by_operator,
        "a missing result is never waived; got {rows:?}"
    );
}

/// An operation the compute lane is working on right now still holds its
/// PREVIOUS core result — `submit_toolpath_compute` clears the viz copy
/// and leaves the core slot alone until the drain. Exporting mid-generate
/// used to emit that previous program silently.
#[test]
fn export_refuses_an_operation_that_is_still_generating() {
    let (session, mut gui, sim) = build_state();
    if let Some(rt) = gui.toolpath_rt.get_mut(&rs_cam_core::ToolpathId(0)) {
        rt.status = ComputeStatus::Computing;
        rt.result = None;
    }
    assert_eq!(
        freshness_at(&session, &gui, 0),
        Some(FreshnessState::Regenerating)
    );

    let msg = expect_export_error(
        export_gcode_from_session_with_policy(
            &session,
            &gui,
            &sim,
            policy(),
            StaleResultPolicy::AcceptPreviousGeometry,
        ),
        "export while the lane is generating",
    );
    assert!(
        msg.contains(&format!("'{OP_NAME}' is still generating")),
        "a generating operation says so rather than exporting the program \
         it is replacing; got: {msg}"
    );
}

// ── 3. One text, every surface ──────────────────────────────────────────

/// `ui/preflight.rs` draws its rows from `blocking_toolpaths` and prints
/// `row.message`; the export refusal is those same messages joined. Under
/// the acceptance the row survives as a caution — the operator still sees
/// which operation will cut previous geometry — but it stops blocking.
#[test]
fn preflight_rows_and_the_export_refusal_share_one_text() {
    let (mut session, gui, sim) = build_state();
    edit_the_operation(&mut session);

    let scope = 0..session.toolpath_configs().len();
    let rows = blocking_toolpaths(&session, &gui, scope.clone(), StaleResultPolicy::Refuse);
    assert_eq!(rows.len(), 1, "exactly one operation is not current");
    let row = &rows[0];
    assert_eq!(row.index, 0);
    assert_eq!(row.name, OP_NAME);
    assert_eq!(row.freshness, FreshnessState::EditedSince);
    assert!(!row.waived_by_operator);
    assert_eq!(
        row.message,
        blocking_toolpath_message(OP_NAME, &FreshnessState::EditedSince),
        "the row text IS the shared builder's text"
    );

    let refusal = expect_export_error(
        export_gcode_from_session_with_policy(
            &session,
            &gui,
            &sim,
            policy(),
            StaleResultPolicy::Refuse,
        ),
        "export_gcode_from_session_with_policy",
    );
    assert!(
        refusal.contains(&row.message),
        "the export refusal prints the pre-flight row's text verbatim; \
         row: {:?}, refusal: {refusal:?}",
        row.message
    );

    // Under the acceptance the same row is still reported, marked waived,
    // so the modal can show it as a caution instead of hiding it.
    let waived = blocking_toolpaths(
        &session,
        &gui,
        scope,
        StaleResultPolicy::AcceptPreviousGeometry,
    );
    assert_eq!(waived.len(), 1, "the operator still sees the operation");
    assert!(
        waived[0].waived_by_operator,
        "an accepted edit stops blocking; got {waived:?}"
    );
    assert_eq!(
        waived[0].message, row.message,
        "waiving a row does not change what it says"
    );
}

/// A disabled operation is skipped, not blocked — G-EXPORTSKIP's rule,
/// which G-STALEXPORT must not break for an edited-and-disabled op.
#[test]
fn a_disabled_edited_operation_does_not_block() {
    let (mut session, gui, _sim) = build_state();
    edit_the_operation(&mut session);
    let _ = session
        .set_toolpath_enabled(0, false)
        .expect("toolpath 0 exists");

    let rows = blocking_toolpaths(
        &session,
        &gui,
        0..session.toolpath_configs().len(),
        StaleResultPolicy::Refuse,
    );
    assert!(
        rows.is_empty(),
        "a disabled operation is skipped by the export, so it raises no \
         blocking row; got {rows:?}"
    );
}
