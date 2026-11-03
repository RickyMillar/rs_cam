//! G-STALEXPORT sentry (F2.3, 2026-09-10) — an operation EDITED after it
//! was generated is never cut from its previous geometry in silence.
//!
//! `io::export::emitted_result_toolpath` read `session.results` first and
//! `gui.toolpath_rt[id].result` — the compute worker's output, kept so the
//! viewport can draw the old path — second, silently. G-FRESHSTATE (F2.1)
//! made EVERY input edit drop the core result, which turned that fallback
//! into the normal path: edit a stepover, export, and the file was the
//! geometry from before the edit, with nothing on any surface saying so.
//!
//! K6 (feeds contract P2, lead decision 4, ruling Q6): an edited op has no
//! current result, so the one rule of G-EXPORTSKIP holds: every surface
//! exports the rest, leaves the op out and names it (`export.op_left_out`).
//! The "accept the previous geometry" step is gone. What this file pins:
//!
//! 1. Editing a generated operation makes every export entry point leave it
//!    out and name it; the previous geometry is never written.
//! 2. The warning says what happened and what the remedy is — it does not
//!    read as "not generated", which is a different situation.
//! 3. A missing result and a generating op keep their own sentences.
//! 4. The pre-flight rows and the export warning come from one builder,
//!    so the modal and the file cannot disagree.

#![expect(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::path::PathBuf;
use std::sync::Arc;

use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::diagnostics::ids;
use rs_cam_core::gcode::ExportedGcode;
use rs_cam_core::geo::P3;
use rs_cam_core::mesh::make_test_flat;
use rs_cam_core::session::{
    AdoptResultArgs, Command, ForgetResultArgs, InvalidateToolpathInputsArgs, LoadedModel,
    ProjectSession, ProjectSessionBuilder, SetToolpathEnabledArgs, ToolpathComputeResult,
    ToolpathConfig,
};
use rs_cam_core::toolpath::Toolpath;
use rs_cam_core::trace::toolpath_spans::AnnotatedToolpath;
use rs_cam_viz::error::VizError;
use rs_cam_viz::io::export::{
    export_gcode_from_session_reporting, export_setup_gcode_from_session_reporting,
    export_single_toolpath_reporting, left_out_message, left_out_toolpaths,
};
use rs_cam_viz::state::freshness::{FreshnessState, freshness_at};
use rs_cam_viz::state::job::SetupId;
use rs_cam_viz::state::runtime::{GuiState, ToolpathRuntime};
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
        op_data: rs_cam_core::ops::drill_op::OpData::Toolpath(Arc::new(AnnotatedToolpath::new(
            path,
        ))),
        stats: Default::default(),
        debug_trace: None,
        semantic_trace: None,
        provenance: rs_cam_core::session::ResultProvenance::Dressed,
        lineage: std::sync::Arc::new(rs_cam_core::trace::lineage::Lineage::unrecorded()),
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
        lineage: std::sync::Arc::new(rs_cam_core::trace::lineage::Lineage::unrecorded()),
    }
}

/// A one-operation project whose operation is GENERATED: a result in the
/// core cache and a drawable copy in the viz store, exactly as
/// `drain_compute_results` leaves it.
fn build_state() -> (ProjectSession, GuiState, SimulationState) {
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

    let tc = ToolpathConfig {
        shoe: Default::default(),
        feeds_lock: Default::default(),
        id: rs_cam_core::ToolpathId::from_raw(0),
        name: OP_NAME.to_owned(),
        enabled: true,
        operation: OperationConfig::Scallop(
            rs_cam_core::compute::operation_configs::ScallopConfig::default(),
        ),
        dressups: Default::default(),
        heights: Default::default(),
        tool_id: Some(rs_cam_core::ids::ToolId::from_raw(1)),
        model_id: rs_cam_core::ids::ModelId::from_raw(0),
        pre_gcode: None,
        post_gcode: None,
        boundary: Default::default(),
        boundary_inherit: true,
        stock_source: Default::default(),
        coolant: Default::default(),
        debug_options: Default::default(),
        feeds_provenance: Default::default(),
        rest_analysis: Default::default(),
        planner_origin: None,
    };
    let id = tc.id;
    let _ = builder.add_toolpath(0, tc).expect("add toolpath");
    let mut session = builder.build();
    let revision = session.toolpath_revision_at(0);
    let _ = session
        .apply(Command::AdoptResult(AdoptResultArgs {
            index: 0,
            revision,
            result: Box::new(core_result(sample_toolpath(PREVIOUS_FEED))),
        }))
        .expect("seed the core result");

    let mut gui = GuiState::new();
    let mut rt = ToolpathRuntime::new(true);
    rt.run =
        rs_cam_viz::state::runtime::ToolpathRun::Done(viz_result(sample_toolpath(PREVIOUS_FEED)));
    gui.toolpath_rt.insert(id, rt);

    (session, gui, SimulationState::new())
}

/// What an operator edit does now: the core result dies, the viz copy
/// stays so the viewport can draw the old path. Reached here through the
/// core door the GUI panel calls (`invalidate_toolpath_inputs`).
fn edit_the_operation(session: &mut ProjectSession) {
    let _ = session
        .apply(Command::InvalidateToolpathInputs(
            InvalidateToolpathInputsArgs { index: 0 },
        ))
        .expect("toolpath 0 exists");
    assert!(
        session.result_at(0).is_none(),
        "the edit must drop the core result — otherwise this file is not \
         exercising the gate"
    );
}

/// The op-left-out text of `exported`. Panics when the export does not
/// name the op, or when the program holds the previous geometry.
fn left_out_text(exported: Result<ExportedGcode, VizError>, what: &str) -> String {
    let exported = exported.unwrap_or_else(|e| panic!("{what}: K6: the export writes, got {e}"));
    let rows: Vec<&str> = exported
        .warnings
        .iter()
        .filter(|w| w.id == ids::EXPORT_OP_LEFT_OUT)
        .map(|w| w.detail.as_str())
        .collect();
    assert!(
        !rows.is_empty(),
        "{what}: the edited op is left out in silence"
    );
    assert!(
        !f_words(&rs_cam_core::gcode::program_without_comments(
            &exported.gcode
        ))
        .contains(&PREVIOUS_FEED),
        "{what}: the program holds the previous geometry:\n{}",
        exported.gcode
    );
    rows.join("; ")
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

// ── 1. Left out and named, on every entry point ─────────────────────────

#[test]
fn export_leaves_out_an_operation_edited_since_generation() {
    let (mut session, gui, sim) = build_state();

    // Before the edit the export writes the op — otherwise a warning below
    // would prove nothing about the edit.
    let before = export_gcode_from_session_reporting(&session, &gui, &sim)
        .expect("a generated operation exports");
    assert!(f_words(&before.gcode).contains(&PREVIOUS_FEED));
    assert!(
        !before
            .warnings
            .iter()
            .any(|w| w.id == ids::EXPORT_OP_LEFT_OUT)
    );

    edit_the_operation(&mut session);
    assert_eq!(
        freshness_at(&session, &gui, 0),
        Some(FreshnessState::EditedSince),
        "the fixture must be in the state this file is about"
    );

    let msg = left_out_text(
        export_gcode_from_session_reporting(&session, &gui, &sim),
        "export_gcode_from_session_reporting",
    );
    assert!(
        msg.contains(&format!("'{OP_NAME}' was edited after it was generated")),
        "the warning must name the operation and say what happened; got: {msg}"
    );
    assert!(
        msg.contains("regenerate it"),
        "the warning must state the remedy; got: {msg}"
    );
    assert!(
        !msg.contains("is not generated"),
        "an EDITED operation is not the same situation as an ungenerated \
         one and must not borrow its sentence; got: {msg}"
    );
}

#[test]
fn every_export_entry_point_leaves_out_the_edited_operation() {
    let (mut session, gui, sim) = build_state();
    edit_the_operation(&mut session);

    let combined = left_out_text(
        export_gcode_from_session_reporting(&session, &gui, &sim),
        "combined",
    );
    let per_setup = left_out_text(
        export_setup_gcode_from_session_reporting(&session, &gui, &sim, SetupId::from_raw(0)),
        "per-setup",
    );
    let single = left_out_text(
        export_single_toolpath_reporting(
            &session,
            &gui,
            &sim,
            rs_cam_core::ToolpathId::from_raw(0),
        ),
        "single",
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

// ── 2. Other states keep their own sentence ─────────────────────────────

/// An operation with no result anywhere is named as not generated.
#[test]
fn a_missing_result_keeps_its_own_sentence() {
    let (mut session, mut gui, sim) = build_state();
    let _ = session
        .apply(Command::ForgetResult(ForgetResultArgs {
            index: 0,
            refusal: None,
        }))
        .expect("toolpath 0 exists");
    if let Some(rt) = gui
        .toolpath_rt
        .get_mut(&rs_cam_core::ToolpathId::from_raw(0))
    {
        rt.run = rs_cam_viz::state::runtime::ToolpathRun::Pending;
    }
    assert_eq!(
        freshness_at(&session, &gui, 0),
        Some(FreshnessState::NoResult)
    );

    let msg = left_out_text(
        export_gcode_from_session_reporting(&session, &gui, &sim),
        "export with an ungenerated op",
    );
    assert!(
        msg.contains(&format!("'{OP_NAME}' is not generated")),
        "a missing result keeps its own sentence; got: {msg}"
    );
}

/// An operation the compute lane is working on right now still holds its
/// PREVIOUS core result — `submit_toolpath_compute` clears the viz copy
/// and leaves the core slot alone until the drain. Exporting mid-generate
/// used to emit that previous program silently; now the op is left out
/// and named.
#[test]
fn export_leaves_out_an_operation_that_is_still_generating() {
    let (session, mut gui, sim) = build_state();
    if let Some(rt) = gui
        .toolpath_rt
        .get_mut(&rs_cam_core::ToolpathId::from_raw(0))
    {
        rt.run = rs_cam_viz::state::runtime::ToolpathRun::Computing;
    }
    assert_eq!(
        freshness_at(&session, &gui, 0),
        Some(FreshnessState::Regenerating)
    );

    let msg = left_out_text(
        export_gcode_from_session_reporting(&session, &gui, &sim),
        "export while the lane is generating",
    );
    assert!(
        msg.contains(&format!("'{OP_NAME}' is still generating")),
        "a generating operation says so rather than exporting the program \
         it is replacing; got: {msg}"
    );
}

// ── 3. One text, every surface ──────────────────────────────────────────

/// `ui/preflight.rs` counts its chips from `left_out_toolpaths` and lists
/// `row.message` on hover; the export warning carries those same messages.
#[test]
fn preflight_rows_and_the_export_warning_share_one_text() {
    let (mut session, gui, sim) = build_state();
    edit_the_operation(&mut session);

    let scope = 0..session.toolpath_configs().len();
    let rows = left_out_toolpaths(&session, &gui, scope);
    assert_eq!(rows.len(), 1, "exactly one operation is not current");
    let row = &rows[0];
    assert_eq!(row.index, 0);
    assert_eq!(row.name, OP_NAME);
    assert_eq!(row.freshness, FreshnessState::EditedSince);
    assert_eq!(
        row.message,
        left_out_message(OP_NAME, &FreshnessState::EditedSince),
        "the row text IS the shared builder's text"
    );

    let warning = left_out_text(
        export_gcode_from_session_reporting(&session, &gui, &sim),
        "export_gcode_from_session_reporting",
    );
    assert_eq!(
        warning, row.message,
        "the export warning prints the pre-flight row's text verbatim"
    );
}

/// A disabled operation is skipped, not left out — G-EXPORTSKIP's rule,
/// which G-STALEXPORT must not break for an edited-and-disabled op.
#[test]
fn a_disabled_edited_operation_is_not_left_out() {
    let (mut session, gui, _sim) = build_state();
    edit_the_operation(&mut session);
    let _ = session
        .apply(Command::SetToolpathEnabled(SetToolpathEnabledArgs {
            index: 0,
            enabled: false,
        }))
        .expect("toolpath 0 exists");

    let rows = left_out_toolpaths(&session, &gui, 0..session.toolpath_configs().len());
    assert!(
        rows.is_empty(),
        "a disabled operation is skipped by the export, so it raises no \
         left-out row; got {rows:?}"
    );
}
