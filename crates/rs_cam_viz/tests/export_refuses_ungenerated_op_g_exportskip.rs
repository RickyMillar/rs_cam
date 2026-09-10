//! G-EXPORTSKIP sentry (R05 §5, 2026-09-10) — an ENABLED operation with no
//! result refuses the export by name; it is never skipped.
//!
//! Until this fix `io::export::emitted_toolpaths` was a `filter_map` over
//! the export scope: an enabled toolpath with no result in either store
//! (`session.results`, then the `gui.toolpath_rt` fallback) returned `None`
//! and the program was emitted without it. Nothing on any export surface
//! said so. The operator read "Generated 3 toolpaths, 1 still waiting on
//! upstream simulated stock", opened the wizard, and saved a file that ran
//! three operations under a project that lists four. A rest op blocked on
//! `AwaitingPriorStock` is the reachable case; a `Pending` op that was
//! never generated and an `Error` op are the other two shapes.
//!
//! Three things this file pins:
//!
//! 1. Every whole-project / per-setup export entry point returns `Err`
//!    naming the ungenerated op, with one text per status, and no program
//!    string exists for a caller to write.
//! 2. A DISABLED op with no result still exports fine — skipping a
//!    disabled op is the intended behaviour, and it must not become a
//!    refusal.
//! 3. The preflight rows come from the same text builder the export
//!    refusal uses, so the two surfaces cannot disagree.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::geo::P3;
use rs_cam_core::mesh::make_test_flat;
use rs_cam_core::session::{LoadedModel, ProjectSession, ToolpathComputeResult, ToolpathConfig};
use rs_cam_core::toolpath::Toolpath;
use rs_cam_core::toolpath_spans::AnnotatedToolpath;
use rs_cam_viz::error::VizError;
use rs_cam_viz::io::export::{
    blocking_toolpath_message, blocking_toolpaths, export_combined_gcode_from_session,
    export_gcode_from_session_with_policy, export_setup_gcode_from_session_with_policy,
};
use rs_cam_viz::state::freshness::freshness_at;
use rs_cam_viz::state::job::SetupId;
use rs_cam_viz::state::runtime::{GuiState, StaleResultPolicy, ToolpathRuntime};
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

fn toolpath_config(id: u32, name: &str, enabled: bool) -> ToolpathConfig {
    ToolpathConfig {
        id: rs_cam_core::ToolpathId(id as usize),
        name: name.to_owned(),
        enabled,
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
    let mut session = ProjectSession::new_empty();
    session.set_name("g-exportskip sentry".to_owned());
    session
        .tools_mut()
        .push(ToolConfig::new_default(ToolId(1), ToolType::EndMill));

    let mesh = Arc::new(make_test_flat(40.0));
    session.add_model(LoadedModel {
        id: 0,
        path: PathBuf::from("flat.stl"),
        name: "Flat".to_owned(),
        kind: Some(ModelKind::Stl),
        mesh: Some(mesh),
        polygons: None,
        drill_targets: Arc::new(Vec::new()),
        layers: Arc::new(Vec::new()),
        enriched_mesh: None,
        units: Some(ModelUnits::Millimeters),
        winding_report: None,
        load_error: None,
    });

    session
        .add_toolpath(0, toolpath_config(0, GENERATED_NAME, true))
        .expect("add generated toolpath");
    let second_enabled = !matches!(second, SecondOp::Disabled);
    // Both ops sit in the default setup (id 0), one after the other.
    session
        .add_toolpath(0, toolpath_config(1, UNGENERATED_NAME, second_enabled))
        .expect("add second toolpath");

    let first_id = session.toolpath_configs()[0].id;
    let second_id = session.toolpath_configs()[1].id;

    session
        .insert_result(0, core_result(sample_toolpath()))
        .expect("insert core result for the generated op");

    let mut gui = GuiState::new();
    let mut first_rt = ToolpathRuntime::new(true);
    first_rt.status = ComputeStatus::Done;
    first_rt.result = Some(viz_result(sample_toolpath()));
    gui.toolpath_rt.insert(first_id, first_rt);

    let mut second_rt = ToolpathRuntime::new(true);
    second_rt.result = None;
    second_rt.status = match second {
        SecondOp::Ungenerated(status) => status,
        SecondOp::Disabled => ComputeStatus::Pending,
    };
    gui.toolpath_rt.insert(second_id, second_rt);

    // No simulation runs here, so every tool-load criterion reads
    // Unmodeled(SimulationRequired). The gate is covered elsewhere; this
    // file is about which operations the emitter is handed.
    gui.tool_load_overrides.accept_unmodeled = true;

    (session, gui, SimulationState::new())
}

fn policy() -> rs_cam_core::gcode::ToolLoadExportPolicy {
    rs_cam_core::gcode::ToolLoadExportPolicy {
        accept_unmodeled: true,
        accept_exceeded: true,
    }
}

fn temp_path(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock before epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("rs_cam_g_exportskip_{name}_{nanos}.nc"))
}

/// The shape every file-writing caller has (`app/export.rs::write_one`,
/// MCP `export_gcode`): export, and write ONLY on `Ok`. Returns the export
/// error so the caller can assert its text.
fn export_and_write(
    session: &ProjectSession,
    gui: &GuiState,
    sim: &SimulationState,
    path: &std::path::Path,
) -> Result<(), VizError> {
    let gcode = export_gcode_from_session_with_policy(
        session,
        gui,
        sim,
        policy(),
        StaleResultPolicy::Refuse,
    )?;
    std::fs::write(path, gcode).expect("write g-code");
    Ok(())
}

fn awaiting_prior_stock() -> ComputeStatus {
    ComputeStatus::AwaitingPriorStock(rs_cam_core::compute::AwaitingPriorStock {
        blocking_toolpath_id: Some(rs_cam_core::ToolpathId(0)),
        blocking_toolpath_index: Some(0),
        message: "waiting on 'Rough Pass' (index 0) to be simulated".to_owned(),
    })
}

fn expect_export_error(result: Result<String, VizError>, surface: &str) -> String {
    match result {
        Ok(gcode) => panic!(
            "G-EXPORTSKIP: {surface} emitted a program ({} lines) with an ENABLED \
             ungenerated operation in scope — the op was silently skipped",
            gcode.lines().count()
        ),
        Err(VizError::Export(msg)) => msg,
        Err(other) => panic!("{surface}: expected VizError::Export, got {other:?}"),
    }
}

// ── 1. Refusal, one text per status ─────────────────────────────────────

#[test]
fn export_refuses_an_enabled_pending_op_and_writes_no_file() {
    let (session, gui, sim) = build_state(SecondOp::Ungenerated(ComputeStatus::Pending));
    let path = temp_path("pending");

    let err = export_and_write(&session, &gui, &sim, &path)
        .err()
        .unwrap_or_else(|| {
            let _ = std::fs::remove_file(&path);
            panic!(
                "G-EXPORTSKIP: export succeeded and wrote {} with an ENABLED, never-generated \
                 operation in scope — '{UNGENERATED_NAME}' was silently skipped",
                path.display()
            )
        });
    assert!(
        !path.exists(),
        "no file may be produced when the export refuses"
    );
    let msg = err.to_string();
    assert!(
        msg.contains(&format!("'{UNGENERATED_NAME}' is not generated")),
        "the refusal must name the op and say it is not generated; got: {msg}"
    );
    assert!(
        !msg.contains(GENERATED_NAME),
        "the refusal must not blame the generated op; got: {msg}"
    );
}

#[test]
fn export_refuses_an_op_awaiting_prior_stock_with_the_upstream_text() {
    let (session, gui, sim) = build_state(SecondOp::Ungenerated(awaiting_prior_stock()));

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
fn export_refuses_an_errored_op_and_carries_the_generator_error() {
    let (session, gui, sim) = build_state(SecondOp::Ungenerated(ComputeStatus::Error(
        "offset collapsed on ring 3".to_owned(),
    )));

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
        msg.contains(&format!(
            "'{UNGENERATED_NAME}' failed to generate: offset collapsed on ring 3"
        )),
        "an Error op must carry the generator's own error text; got: {msg}"
    );
}

/// The per-setup and combined entry points are what the PerSetup wizard
/// layout and the MCP `split_setups` export write from. They must refuse
/// too — otherwise the whole-project door refuses and the per-setup door
/// still ships the short program.
#[test]
fn per_setup_and_combined_exports_refuse_the_same_op() {
    let (session, gui, sim) = build_state(SecondOp::Ungenerated(awaiting_prior_stock()));
    let setup_id = SetupId(session.list_setups()[0].id);

    let per_setup = expect_export_error(
        export_setup_gcode_from_session_with_policy(
            &session,
            &gui,
            &sim,
            setup_id,
            policy(),
            StaleResultPolicy::Refuse,
        ),
        "export_setup_gcode_from_session_with_policy",
    );
    assert!(
        per_setup.contains(UNGENERATED_NAME),
        "per-setup refusal must name the op; got: {per_setup}"
    );

    let combined = expect_export_error(
        export_combined_gcode_from_session(&session, &gui, &sim),
        "export_combined_gcode_from_session",
    );
    assert!(
        combined.contains(UNGENERATED_NAME),
        "combined refusal must name the op; got: {combined}"
    );
}

// ── 2. A disabled op is still skipped ───────────────────────────────────

#[test]
fn a_disabled_op_with_no_result_is_skipped_and_the_export_succeeds() {
    let (session, gui, sim) = build_state(SecondOp::Disabled);
    let path = temp_path("disabled");

    export_and_write(&session, &gui, &sim, &path)
        .expect("a disabled op with no result is an intended skip, not a refusal");
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
}

// ── 3. The pre-flight rows and the refusal share one text ───────────────

/// `ui/preflight.rs` draws one `Fail` card per row of
/// `blocking_toolpaths` with `row.message` as its detail. The export
/// refusal is those same messages joined. Assert the two agree for every
/// status shape, and that the row list is empty when nothing blocks.
///
/// G-STALEXPORT renamed the builder and keyed it on `FreshnessState`
/// instead of the raw `ComputeStatus`, because the status alone cannot
/// tell a never-generated op from one whose result belongs to a previous
/// parameter set. The three texts asserted here are unchanged.
#[test]
fn preflight_rows_carry_the_same_text_as_the_export_refusal() {
    for status in [
        ComputeStatus::Pending,
        awaiting_prior_stock(),
        ComputeStatus::Error("offset collapsed on ring 3".to_owned()),
    ] {
        let label = status.label();
        let (session, gui, sim) = build_state(SecondOp::Ungenerated(status.clone()));
        let scope = 0..session.toolpath_configs().len();

        let rows = blocking_toolpaths(&session, &gui, scope, StaleResultPolicy::Refuse);
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
            blocking_toolpath_message(UNGENERATED_NAME, &freshness),
            "[{label}] the row text IS the shared builder's text"
        );
        assert!(
            !row.waived_by_operator,
            "[{label}] a missing result can never be waived — there is no \
             geometry to put in its place"
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
            "[{label}] the export refusal must print the pre-flight row's text verbatim; \
             row: {:?}, refusal: {refusal:?}",
            row.message
        );
    }

    // Nothing blocks when the second op is disabled — no row, no refusal.
    let (session, gui, _sim) = build_state(SecondOp::Disabled);
    let rows = blocking_toolpaths(
        &session,
        &gui,
        0..session.toolpath_configs().len(),
        StaleResultPolicy::Refuse,
    );
    assert!(
        rows.is_empty(),
        "a disabled op must not produce a blocking row; got {rows:?}"
    );
}
