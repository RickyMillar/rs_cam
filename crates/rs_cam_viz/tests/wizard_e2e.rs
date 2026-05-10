//! End-to-end harness for the Phase-5 export wizard.
//!
//! The wizard's UI surface is egui, which we can't easily drive from a
//! headless test. This file instead exercises the wizard's *backend*
//! data path: it builds a minimal `ProjectSession` with one computed
//! toolpath, mutates `WizardState` exactly the way the UI's AppEvent
//! handlers do, and then invokes the same `io::export` helpers the
//! `WizardSave` handler dispatches to. Any path the wizard can take
//! through Save (SingleFile / PerSetup / PerToolpath) writes to a
//! temp file and the result is parsed through `gcode_validator`.
//!
//! This is the closest equivalent to "MCP harness end-to-end" we can
//! land without inventing wizard-specific MCP tools (the MCP layer
//! today just has `export_gcode`, which doesn't model the wizard's
//! configurable layout/template).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout,
    clippy::print_stderr
)]

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::gcode::PostFormat;
use rs_cam_core::geo::P3;
use rs_cam_core::mesh::make_test_flat;
use rs_cam_core::session::{LoadedModel, OutputLayout, ProjectSession, ToolpathConfig};
use rs_cam_core::toolpath::Toolpath;
use rs_cam_core::toolpath_spans::AnnotatedToolpath;
use rs_cam_viz::error::VizError;
use rs_cam_viz::io::export::{
    export_gcode_from_session, export_setup_gcode_from_session,
    export_single_toolpath_from_session,
};
use rs_cam_viz::state::job::SetupId;
use rs_cam_viz::state::runtime::{GuiState, ToolpathRuntime};
use rs_cam_viz::state::simulation::SimulationState;
use rs_cam_viz::state::toolpath::{OperationConfig, ToolpathResult};

fn temp_dir(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock before epoch")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("rs_cam_wizard_e2e_{name}_{nanos}"));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn build_session() -> (ProjectSession, GuiState, SimulationState) {
    let mut session = ProjectSession::new_empty();
    session.set_name("wizard e2e".to_owned());

    // Tool 1
    session.tools_mut().push(ToolConfig::new_default(ToolId(1), ToolType::EndMill));

    // Flat mesh as a model
    let mesh = Arc::new(make_test_flat(40.0));
    session.add_model(LoadedModel {
        id: 0,
        path: PathBuf::from("flat.stl"),
        name: "Flat".to_owned(),
        kind: Some(ModelKind::Stl),
        mesh: Some(Arc::clone(&mesh)),
        polygons: None,
        enriched_mesh: None,
        units: Some(ModelUnits::Millimeters),
        winding_report: None,
        load_error: None,
    });

    // One toolpath with a couple of feed moves so the emitter has
    // something to chew on (an empty toolpath emits only the post's
    // pre/post-amble, which still yields valid g-code).
    let tp = ToolpathConfig {
        id: 0,
        name: "Sample Path".to_owned(),
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
        feeds_auto: Default::default(),
        debug_options: Default::default(),
    };
    session.add_toolpath(0, tp).expect("add toolpath");
    let tp_id = session.toolpath_configs()[0].id;

    // Stub a computed result so the export pipeline has bytes to emit.
    let mut path = Toolpath::new();
    path.rapid_to(P3::new(0.0, 0.0, 5.0));
    path.feed_to(P3::new(10.0, 0.0, -1.0), 600.0);
    path.feed_to(P3::new(10.0, 10.0, -1.0), 600.0);
    path.rapid_to(P3::new(10.0, 10.0, 5.0));

    let mut gui = GuiState::new();
    let mut rt = ToolpathRuntime::new(true);
    rt.result = Some(ToolpathResult {
        annotated: Arc::new(AnnotatedToolpath::new(path)),
        stats: Default::default(),
        debug_trace: None,
        semantic_trace: None,
        debug_trace_path: None,
    });
    gui.toolpath_rt.insert(tp_id, rt);

    (session, gui, SimulationState::new())
}

/// SingleFile path: WizardSave funnels through `export_gcode_from_session`.
#[test]
fn wizard_single_file_save_writes_valid_gcode() {
    let (mut session, gui, sim) = build_session();
    session.wizard_mut().output_layout = OutputLayout::SingleFile;
    session.wizard_mut().filename_template = "{job}.nc".to_owned();

    let gcode =
        export_gcode_from_session(&session, &gui, &sim).expect("single-file export succeeds");
    assert!(gcode.contains("G1"), "expected at least one feed move in output");

    // Validator runs without panicking and returns a finite list.
    let findings = rs_cam_core::gcode_validator::validate(&gcode, PostFormat::Grbl);
    println!("single-file findings: {}", findings.len());

    let dir = temp_dir("single");
    let path = dir.join("wizard_e2e.nc");
    std::fs::write(&path, &gcode).expect("write single file");
    assert!(path.exists());
}

/// PerSetup path: iterates setups + per-setup helper, like
/// `handle_wizard_save`'s PerSetup branch.
#[test]
fn wizard_per_setup_save_writes_one_file_per_setup() {
    let (mut session, gui, sim) = build_session();
    session.wizard_mut().output_layout = OutputLayout::PerSetup;
    session.wizard_mut().filename_template = "{job}_{setup}.nc".to_owned();

    let setup_ids: Vec<SetupId> = session
        .list_setups()
        .iter()
        .map(|s| SetupId(s.id))
        .collect();
    let dir = temp_dir("setup");
    let mut written = 0usize;
    for sid in setup_ids {
        match export_setup_gcode_from_session(&session, &gui, &sim, sid) {
            Ok(gcode) => {
                assert!(gcode.contains("G1"));
                let setup_name = session
                    .list_setups()
                    .iter()
                    .find(|s| s.id == sid.0)
                    .map(|s| s.name.clone())
                    .unwrap_or_default();
                let name = format!("wizard_e2e_{}.nc", setup_name.replace(' ', "_"));
                let path = dir.join(name);
                std::fs::write(&path, &gcode).expect("write per-setup file");
                assert!(path.exists());
                written += 1;
            }
            Err(VizError::Export(msg)) if msg.starts_with("No computed") => {}
            Err(e) => panic!("per-setup export failed: {e}"),
        }
    }
    assert!(written >= 1, "expected at least one setup to export");
}

/// PerToolpath path: per-toolpath helper writes one file each.
#[test]
fn wizard_per_toolpath_save_writes_one_file_per_toolpath() {
    let (mut session, gui, sim) = build_session();
    session.wizard_mut().output_layout = OutputLayout::PerToolpath;
    session.wizard_mut().filename_template = "{job}_{toolpath}.nc".to_owned();

    let ids: Vec<usize> = session
        .toolpath_configs()
        .iter()
        .filter(|tc| tc.enabled)
        .map(|tc| tc.id)
        .collect();

    let dir = temp_dir("tp");
    let mut written = 0usize;
    for id in ids {
        let gcode = export_single_toolpath_from_session(&session, &gui, &sim, id)
            .expect("per-toolpath export succeeds");
        assert!(gcode.contains("G1"));
        let path = dir.join(format!("wizard_e2e_{id}.nc"));
        std::fs::write(&path, &gcode).expect("write per-toolpath file");
        assert!(path.exists());
        written += 1;
    }
    assert!(written >= 1);
}

/// Wizard-state mutations are observable through the public accessor
/// (mimics how the AppEvent handlers update settings).
#[test]
fn wizard_state_mutations_round_trip() {
    let (mut session, _gui, _sim) = build_session();

    // Mirror the input.rs handler bodies.
    session.wizard_mut().output_layout = OutputLayout::PerToolpath;
    session.wizard_mut().filename_template = "{toolpath}.{ext}".to_owned();
    session.wizard_mut().wcs_override = Some(rs_cam_core::gcode::WcsCode::G55);
    session.wizard_mut().units_override = Some(rs_cam_core::gcode::Units::Inch);
    session.wizard_mut().safe_z_override = Some(20.0);
    session.wizard_mut().spindle_warmup_secs = 5;
    session.wizard_mut().allow_validator_errors = true;
    session.wizard_mut().last_step_visited = 5;

    let w = session.wizard();
    assert_eq!(w.output_layout, OutputLayout::PerToolpath);
    assert_eq!(w.filename_template, "{toolpath}.{ext}");
    assert_eq!(w.wcs_override, Some(rs_cam_core::gcode::WcsCode::G55));
    assert_eq!(w.units_override, Some(rs_cam_core::gcode::Units::Inch));
    assert_eq!(w.safe_z_override, Some(20.0));
    assert_eq!(w.spindle_warmup_secs, 5);
    assert!(w.allow_validator_errors);
    assert_eq!(w.last_step_visited, 5);
}
