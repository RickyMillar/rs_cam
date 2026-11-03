//! G-EXPORTSKIP sentry (R05 §5, 2026-09-10) — an ENABLED operation with no
//! result is never left out in silence.
//!
//! Until this fix `io::export::emitted_toolpaths` was a `filter_map` over
//! the export scope: an enabled toolpath with no result in either store
//! returned `None` and the program was emitted without it. Nothing on any
//! export surface said so. A rest op blocked on `AwaitingPriorStock` is the
//! reachable case; a `Pending` op that was never generated and an `Error`
//! op are the other two shapes. From 2026-09-10 to K6 the export refused.
//!
//! K6 (feeds contract P2, lead decision 4, ruling Q6): no finding refuses
//! a job. Every surface exports the rest and names the op it leaves out
//! (`export.op_left_out`, `Danger::Part`), with one text per status.
//!
//! Three things this file pins:
//!
//! 1. Every whole-project / per-setup export entry point writes the
//!    generated op, leaves out the ungenerated op, and names it in the
//!    result and the program header, with one text per status.
//! 2. A DISABLED op with no result is skipped with no warning — skipping a
//!    disabled op is the intended behaviour.
//! 3. The preflight rows come from the same text builder the warning uses,
//!    so the two surfaces cannot disagree.

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::diagnostics::ids;
use rs_cam_core::gcode::ExportedGcode;
use rs_cam_core::geo::P3;
use rs_cam_core::mesh::make_test_flat;
use rs_cam_core::session::{
    AdoptResultArgs, Command, LoadedModel, ProjectSession, ProjectSessionBuilder,
    ToolpathComputeResult, ToolpathConfig,
};
use rs_cam_core::toolpath::Toolpath;
use rs_cam_core::trace::toolpath_spans::AnnotatedToolpath;
use rs_cam_viz::error::VizError;
use rs_cam_viz::io::export::{
    export_gcode_from_session, export_gcode_from_session_reporting,
    export_setup_gcode_from_session_reporting, left_out_message, left_out_toolpaths,
};
use rs_cam_viz::state::freshness::freshness_at;
use rs_cam_viz::state::runtime::ToolpathRun;
use rs_cam_viz::state::runtime::{GuiState, ToolpathRuntime};
use rs_cam_viz::state::simulation::SimulationState;
use rs_cam_viz::state::toolpath::{ComputeStatus, OperationConfig, ToolpathResult};

const GENERATED_NAME: &str = "Rough Pass";
const UNGENERATED_NAME: &str = "Finish Pass";

fn sample_toolpath() -> Toolpath {
    let mut path = Toolpath::new();
    path.rapid_to(P3::new(0.0, 0.0, 5.0));
    path.feed_to(P3::new(10.0, 0.0, -1.0), 600.0);
    path.feed_to(P3::new(10.0, 10.0, -1.0), 600.0);
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

fn toolpath_config(id: u32, name: &str, enabled: bool) -> ToolpathConfig {
    ToolpathConfig {
        shoe: Default::default(),
        feeds_lock: Default::default(),
        id: rs_cam_core::ToolpathId::from_raw(id as usize),
        name: name.to_owned(),
        enabled,
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
    }
}

/// How the SECOND toolpath stands. The first is always enabled and
/// generated in both stores (the post-`drain_compute_results` state).
enum SecondOp {
    /// Enabled, no result in either store, the given runtime status.
    Ungenerated(ComputeStatus),
    /// Disabled, no result — the intended skip.
    Disabled,
}

fn build_state(second: SecondOp) -> (ProjectSession, GuiState, SimulationState) {
    let tool = ToolConfig::new_default(ToolId::from_raw(1), ToolType::EndMill);
    let mut builder = ProjectSessionBuilder::new()
        .tool(tool)
        .name("g-exportskip sentry".to_owned());

    let mesh = Arc::new(make_test_flat(40.0));
    let _ = builder.add_model(LoadedModel {
        id: rs_cam_core::ids::ModelId::from_raw(0),
        path: PathBuf::from("flat.stl"),
        name: "Flat".to_owned(),
        kind: Some(ModelKind::Stl),
        mesh: Some(mesh),
        items: Default::default(),
        enriched_mesh: None,
        units: Some(ModelUnits::Millimeters),
        winding_report: None,
        load_error: None,
        import_options: Default::default(),
        import_report: None,
        drawing: Default::default(),
    });

    let _ = builder
        .add_toolpath(0, toolpath_config(0, GENERATED_NAME, true))
        .expect("add generated toolpath");
    let second_enabled = !matches!(second, SecondOp::Disabled);
    // Both ops sit in the default setup (id 0), one after the other.
    let _ = builder
        .add_toolpath(0, toolpath_config(1, UNGENERATED_NAME, second_enabled))
        .expect("add second toolpath");
    let mut session = builder.build();

    let first_id = session.toolpath_configs()[0].id;
    let second_id = session.toolpath_configs()[1].id;

    let revision = session.toolpath_revision_at(0);
    let _ = session
        .apply(Command::AdoptResult(AdoptResultArgs {
            index: 0,
            revision,
            result: Box::new(core_result(sample_toolpath())),
        }))
        .expect("insert core result for the generated op");

    let mut gui = GuiState::new();
    let mut first_rt = ToolpathRuntime::new(true);
    first_rt.run = rs_cam_viz::state::runtime::ToolpathRun::Done(viz_result(sample_toolpath()));
    gui.toolpath_rt.insert(first_id, first_rt);

    let mut second_rt = ToolpathRuntime::new(true);
    // F031: the run record is one value. An ungenerated status holds no
    // result, so each one maps to the run arm that holds none.
    second_rt.run = match second {
        SecondOp::Ungenerated(ComputeStatus::AwaitingPriorStock(block)) => {
            ToolpathRun::AwaitingPriorStock(block)
        }
        SecondOp::Ungenerated(ComputeStatus::Error(message)) => ToolpathRun::Failed(message),
        SecondOp::Ungenerated(ComputeStatus::Computing) => ToolpathRun::Computing,
        SecondOp::Ungenerated(
            ComputeStatus::Pending | ComputeStatus::Done | ComputeStatus::Disabled,
        )
        | SecondOp::Disabled => ToolpathRun::Pending,
    };
    gui.toolpath_rt.insert(second_id, second_rt);

    // No simulation runs here, so every tool-load criterion reads
    // Unmodeled(SimulationRequired). The export warns on those rows; this
    // file is about which operations the emitter is handed.

    (session, gui, SimulationState::new())
}

fn temp_path(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock before epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("rs_cam_g_exportskip_{name}_{nanos}.nc"))
}

/// The shape every file-writing caller has (`app/export.rs::write_one`,
/// MCP `export_gcode`): export, and write the program.
fn export_and_write(
    session: &ProjectSession,
    gui: &GuiState,
    sim: &SimulationState,
    path: &std::path::Path,
) -> Result<(), VizError> {
    let gcode = export_gcode_from_session(session, gui, sim)?;
    std::fs::write(path, gcode).expect("write g-code");
    Ok(())
}

fn awaiting_prior_stock() -> ComputeStatus {
    ComputeStatus::AwaitingPriorStock(rs_cam_core::compute::AwaitingPriorStock {
        blocking_toolpath_id: Some(rs_cam_core::ToolpathId::from_raw(0)),
        blocking_toolpath_index: Some(0),
        message: "waiting on 'Rough Pass' (index 0) to be simulated".to_owned(),
    })
}

/// The text of each op-left-out warning of `exported`, joined with `; `.
/// Panics when the export names no left-out op, or when the program holds
/// the ungenerated op's label outside a warning line.
fn left_out_text(exported: &ExportedGcode, surface: &str) -> String {
    let rows: Vec<&str> = exported
        .warnings
        .iter()
        .filter(|w| w.id == ids::EXPORT_OP_LEFT_OUT)
        .map(|w| w.detail.as_str())
        .collect();
    assert!(
        !rows.is_empty(),
        "G-EXPORTSKIP: {surface} left the ENABLED ungenerated operation out in silence"
    );
    assert!(
        exported
            .gcode
            .lines()
            .any(|line| line.contains("WARNING") && line.contains(ids::EXPORT_OP_LEFT_OUT)),
        "{surface}: the program header names the op left out"
    );
    assert!(
        !exported
            .gcode
            .lines()
            .any(|line| line.contains(UNGENERATED_NAME) && !line.contains("WARNING")),
        "{surface}: the program holds the op it left out"
    );
    assert!(
        exported.gcode.contains(GENERATED_NAME),
        "{surface}: the generated op is still written"
    );
    rows.join("; ")
}

// ── 1. The op is left out and named, one text per status ────────────────

#[test]
fn export_leaves_out_an_enabled_pending_op_and_names_it() {
    let (session, gui, sim) = build_state(SecondOp::Ungenerated(ComputeStatus::Pending));
    let path = temp_path("pending");
    export_and_write(&session, &gui, &sim, &path).expect("K6: the export writes the rest");
    let written = std::fs::read_to_string(&path).expect("read the written program");
    let _ = std::fs::remove_file(&path);
    assert!(written.contains("G1"), "the generated op is written");

    let exported = export_gcode_from_session_reporting(&session, &gui, &sim)
        .expect("the export writes the rest");
    let msg = left_out_text(&exported, "export_gcode_from_session_reporting");
    assert!(
        msg.contains(&format!("'{UNGENERATED_NAME}' is not generated")),
        "the warning must name the op and say it is not generated; got: {msg}"
    );
    assert!(
        !msg.contains(GENERATED_NAME),
        "the warning must not blame the generated op; got: {msg}"
    );
}

#[test]
fn export_names_an_op_awaiting_prior_stock_with_the_upstream_text() {
    let (session, gui, sim) = build_state(SecondOp::Ungenerated(awaiting_prior_stock()));
    let exported = export_gcode_from_session_reporting(&session, &gui, &sim)
        .expect("the export writes the rest");
    let msg = left_out_text(&exported, "export_gcode_from_session_reporting");
    assert!(
        msg.contains(&format!(
            "'{UNGENERATED_NAME}' is still waiting on upstream stock"
        )),
        "an AwaitingPriorStock op must be reported as waiting on upstream stock, \
         not as merely ungenerated; got: {msg}"
    );
    assert!(
        msg.contains("Generate All"),
        "the waiting text must tell the operator the remedy; got: {msg}"
    );
}

#[test]
fn export_names_an_errored_op_and_carries_the_generator_error() {
    let (session, gui, sim) = build_state(SecondOp::Ungenerated(ComputeStatus::Error(
        "offset collapsed on ring 3".to_owned(),
    )));
    let exported = export_gcode_from_session_reporting(&session, &gui, &sim)
        .expect("the export writes the rest");
    let msg = left_out_text(&exported, "export_gcode_from_session_reporting");
    assert!(
        msg.contains(&format!(
            "'{UNGENERATED_NAME}' failed to generate: offset collapsed on ring 3"
        )),
        "an Error op must carry the generator's own error text; got: {msg}"
    );
}

/// The per-setup and combined entry points are what the PerSetup wizard
/// layout and the MCP `split_setups` export write from. Both name the op
/// they leave out.
#[test]
fn per_setup_and_combined_exports_name_the_same_op() {
    let (session, gui, sim) = build_state(SecondOp::Ungenerated(awaiting_prior_stock()));
    let setup_id = session.list_setups()[0].id;

    let per_setup = export_setup_gcode_from_session_reporting(&session, &gui, &sim, setup_id)
        .expect("the per-setup export writes the rest");
    let per_setup = left_out_text(&per_setup, "export_setup_gcode_from_session_reporting");
    assert!(
        per_setup.contains(UNGENERATED_NAME),
        "the per-setup warning must name the op; got: {per_setup}"
    );

    let combined = export_gcode_from_session_reporting(&session, &gui, &sim)
        .expect("the combined export writes the rest");
    let combined = left_out_text(&combined, "export_gcode_from_session_reporting");
    assert_eq!(combined, per_setup, "one text on both doors");
}

// ── 2. A disabled op is still skipped ───────────────────────────────────

#[test]
fn a_disabled_op_with_no_result_is_skipped_and_the_export_succeeds() {
    let (session, gui, sim) = build_state(SecondOp::Disabled);
    let path = temp_path("disabled");

    export_and_write(&session, &gui, &sim, &path)
        .expect("a disabled op with no result is an intended skip");
    let gcode = std::fs::read_to_string(&path).expect("read the written program");
    let _ = std::fs::remove_file(&path);

    assert!(gcode.contains("G1"), "the generated op must be emitted");
    assert!(
        gcode.contains(GENERATED_NAME),
        "the program must carry the generated op's label; got:\n{gcode}"
    );
    assert!(
        !gcode.contains(UNGENERATED_NAME),
        "the disabled op must not appear in the program; got:\n{gcode}"
    );
    let exported = export_gcode_from_session_reporting(&session, &gui, &sim)
        .expect("the export writes the program");
    assert!(
        !exported
            .warnings
            .iter()
            .any(|w| w.id == ids::EXPORT_OP_LEFT_OUT),
        "a disabled op is not left out: {:?}",
        exported.warnings
    );
}

// ── 3. The pre-flight rows and the warning share one text ───────────────

/// `ui/preflight.rs` counts one `Part` chip per row of `left_out_toolpaths`
/// and lists `row.message` on hover. The export warning carries those same
/// messages. Assert the two agree for every status shape, and that the row
/// list is empty when nothing is left out.
#[test]
fn preflight_rows_carry_the_same_text_as_the_export_warning() {
    for status in [
        ComputeStatus::Pending,
        awaiting_prior_stock(),
        ComputeStatus::Error("offset collapsed on ring 3".to_owned()),
    ] {
        let label = status.label();
        let (session, gui, sim) = build_state(SecondOp::Ungenerated(status.clone()));
        let scope = 0..session.toolpath_configs().len();

        let rows = left_out_toolpaths(&session, &gui, scope);
        assert_eq!(
            rows.len(),
            1,
            "[{label}] exactly one enabled op has no result"
        );
        let row = &rows[0];
        assert_eq!(
            row.index, 1,
            "[{label}] the row names the ungenerated op's index"
        );
        assert_eq!(row.name, UNGENERATED_NAME, "[{label}] the row names the op");
        let freshness = freshness_at(&session, &gui, 1).expect("toolpath 1 exists");
        assert_eq!(
            row.message,
            left_out_message(UNGENERATED_NAME, &freshness),
            "[{label}] the row text IS the shared builder's text"
        );

        let exported = export_gcode_from_session_reporting(&session, &gui, &sim)
            .expect("the export writes the rest");
        let warning = left_out_text(&exported, "export_gcode_from_session_reporting");
        assert_eq!(
            warning, row.message,
            "[{label}] the export warning prints the pre-flight row's text verbatim"
        );
    }

    // Nothing is left out when the second op is disabled — no row.
    let (session, gui, _sim) = build_state(SecondOp::Disabled);
    let rows = left_out_toolpaths(&session, &gui, 0..session.toolpath_configs().len());
    assert!(
        rows.is_empty(),
        "a disabled op must not produce a left-out row; got {rows:?}"
    );
}
