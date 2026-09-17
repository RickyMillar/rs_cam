//! EDG-06 sentry (design audit 2026-09-17) — an MCP export that trips an
//! error-severity machine-safety finding names it.
//!
//! `io::export::log_machine_safety` ran `validate_machine_safety` on every
//! emitted program and then `tracing::warn!`ed the count and the first
//! message. The findings never left the function. `mcp_export_gcode`
//! returned `"G-code exported to {path}"` on success, so an agent that
//! exported a program with a `RapidBelowClearance` or a
//! `ZBelowProgramFloor` at `Severity::Error` — a confirmed safety or
//! correctness issue, by that severity's own definition — read the same
//! text as a clean export. The GUI export wizard showed those findings.
//!
//! Three things this file pins:
//!
//! 1. The reporting export door hands the findings back to its caller.
//! 2. The text the MCP surface builds from them names the error severity
//!    and the finding's own message.
//! 3. A clean export's text is byte-identical to the pre-row text, and
//!    the reporting door emits the same bytes as the plain door.
//!
//! The findings must travel WITH the text: the pass runs before the
//! high-feedrate rapid-to-feed rewrite, so a caller that re-ran the
//! validator on the returned program would miss every rapid finding.

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
use rs_cam_core::export::gcode_validator::{Finding, FindingKind, Severity};
use rs_cam_core::geo::P3;
use rs_cam_core::mesh::make_test_flat;
use rs_cam_core::session::{
    AdoptResultArgs, Command, LoadedModel, ProjectSession, ProjectSessionBuilder,
    ToolpathComputeResult, ToolpathConfig,
};
use rs_cam_core::toolpath::Toolpath;
use rs_cam_core::trace::toolpath_spans::AnnotatedToolpath;
use rs_cam_viz::io::export::{
    export_gcode_from_session_reporting, export_gcode_from_session_with_policy,
    export_success_text, machine_safety_report,
};
use rs_cam_viz::state::runtime::{GuiState, StaleResultPolicy, ToolpathRuntime};
use rs_cam_viz::state::simulation::SimulationState;
use rs_cam_viz::state::toolpath::{ComputeStatus, OperationConfig, ToolpathResult};

const OP_NAME: &str = "Rough Pass";

/// The text the MCP `export_gcode` tool builds on a successful write.
const EXPORT_HEADER: &str = "G-code exported to /tmp/edg06.nc";

fn sample_toolpath() -> Toolpath {
    let mut path = Toolpath::new();
    path.rapid_to(P3::new(0.0, 0.0, 5.0));
    path.feed_to(P3::new(10.0, 0.0, -1.0), 600.0);
    path.feed_to(P3::new(10.0, 10.0, -1.0), 600.0);
    // The rapid this sentry rides: it repositions in X and Y at Z=5, which
    // is below a 60 mm clearance plane.
    path.rapid_to(P3::new(30.0, 30.0, 5.0));
    path.feed_to(P3::new(30.0, 30.0, -1.0), 600.0);
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

fn toolpath_config(id: u32, name: &str) -> ToolpathConfig {
    ToolpathConfig {
        id: rs_cam_core::ToolpathId(id as usize),
        name: name.to_owned(),
        enabled: true,
        operation: OperationConfig::Scallop(
            rs_cam_core::compute::operation_configs::ScallopConfig::default(),
        ),
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

/// One generated operation, and the GUI post block the export reads.
/// `safe_z` is the clearance plane the machine-safety pass compares every
/// rapid against.
fn build_state(safe_z: f64) -> (ProjectSession, GuiState, SimulationState) {
    let tool = ToolConfig::new_default(ToolId(1), ToolType::EndMill);
    let mut builder = ProjectSessionBuilder::new()
        .tool(tool)
        .name("edg06 sentry".to_owned());

    let mesh = Arc::new(make_test_flat(40.0));
    let _ = builder.add_model(LoadedModel {
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
    let _ = builder
        .add_toolpath(0, toolpath_config(0, OP_NAME))
        .expect("add toolpath");
    let mut session = builder.build();

    let tp_id = session.toolpath_configs()[0].id;
    let revision = session.toolpath_revision(0);
    let _ = session
        .apply(Command::AdoptResult(AdoptResultArgs {
            index: 0,
            revision,
            result: Box::new(core_result(sample_toolpath())),
        }))
        .expect("insert the core result");

    let mut gui = GuiState::new();
    let mut rt = ToolpathRuntime::new(true);
    rt.status = ComputeStatus::Done;
    rt.result = Some(viz_result(sample_toolpath()));
    gui.toolpath_rt.insert(tp_id, rt);
    gui.post.safe_z = safe_z;
    // No simulation runs here, so every tool-load criterion reads
    // Unmodeled. The gate is covered elsewhere; this file is about what
    // the export surface says after a successful emit.
    gui.tool_load_overrides.accept_unmodeled = true;

    (session, gui, SimulationState::new())
}

fn policy() -> rs_cam_core::gcode::ToolLoadExportPolicy {
    rs_cam_core::gcode::ToolLoadExportPolicy {
        accept_unmodeled: true,
        accept_exceeded: true,
    }
}

// -- 1. The findings reach the caller, and the reply names the error -----

#[test]
fn an_mcp_export_that_trips_an_error_finding_names_it() {
    // A clearance plane far above every rapid in the program: each rapid
    // that moves in X or Y at Z=5 is then below clearance.
    let (session, gui, sim) = build_state(60.0);

    let exported = export_gcode_from_session_reporting(
        &session,
        &gui,
        &sim,
        policy(),
        StaleResultPolicy::Refuse,
    )
    .expect("the export succeeds");

    let errors: Vec<&Finding> = exported
        .machine_safety
        .iter()
        .filter(|f| f.severity == Severity::Error)
        .collect();
    assert!(
        !errors.is_empty(),
        "EDG-06 fixture is vacuous: a safe-Z of 60 mm over a program whose rapids \
         reposition at Z=5 raised no error-severity finding (got {} findings)",
        exported.machine_safety.len()
    );

    let reply = export_success_text(EXPORT_HEADER.to_owned(), &exported.machine_safety);
    assert!(
        reply.starts_with(EXPORT_HEADER),
        "the reply dropped the path it used to report: {reply}"
    );
    assert!(
        reply.contains("MACHINE-SAFETY ERROR"),
        "EDG-06: the MCP reply does not say the export tripped an error-severity \
         machine-safety finding; it reads:\n{reply}"
    );
    let first = errors[0];
    assert!(
        reply.contains(&first.message),
        "EDG-06: the MCP reply names no finding message. Expected {:?} in:\n{reply}",
        first.message
    );
    assert!(
        reply.contains(&format!("line {}", first.line)),
        "EDG-06: the MCP reply names no line number for the first finding:\n{reply}"
    );
}

// -- 2. A clean export is unchanged --------------------------------------

#[test]
fn a_clean_export_reports_exactly_the_text_it_always_did() {
    assert_eq!(machine_safety_report(&[]), None);
    assert_eq!(
        export_success_text(EXPORT_HEADER.to_owned(), &[]),
        EXPORT_HEADER,
        "a clean pass must leave the export message byte-identical"
    );
}

// -- 3. The reporting door emits the same program as the plain door ------

#[test]
fn the_reporting_door_emits_the_same_program() {
    let (session, gui, sim) = build_state(60.0);
    let plain = export_gcode_from_session_with_policy(
        &session,
        &gui,
        &sim,
        policy(),
        StaleResultPolicy::Refuse,
    )
    .expect("the plain door exports");
    let reporting = export_gcode_from_session_reporting(
        &session,
        &gui,
        &sim,
        policy(),
        StaleResultPolicy::Refuse,
    )
    .expect("the reporting door exports");
    assert_eq!(
        plain, reporting.gcode,
        "the two export doors must emit one program"
    );
}

// -- 4. The report separates a warning-only pass from an error pass ------

#[test]
fn a_warning_only_pass_does_not_claim_an_error() {
    let warning = Finding {
        severity: Severity::Warning,
        kind: FindingKind::SpindleLeftRunning,
        line: 12,
        message: "program ends with the spindle running".to_owned(),
    };
    let report =
        machine_safety_report(std::slice::from_ref(&warning)).expect("a warning is still reported");
    assert!(
        !report.contains("MACHINE-SAFETY ERROR"),
        "a warning-only pass must not read as an error: {report}"
    );
    assert!(report.contains(&warning.message));
    assert!(report.contains("line 12"));
}
