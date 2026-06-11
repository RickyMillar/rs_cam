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
    export_combined_gcode_from_session, export_gcode_from_session,
    export_gcode_from_session_with_policy, export_setup_gcode_from_session,
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
    session
        .tools_mut()
        .push(ToolConfig::new_default(ToolId(1), ToolType::EndMill));

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
        id: rs_cam_core::ToolpathId(0),
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
        debug_options: Default::default(),
        feeds_provenance: Default::default(),
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
        drill_op: None,
    });
    gui.toolpath_rt.insert(tp_id, rt);

    // C1 (2026-06-11): the phase-level checked exports now enforce the
    // tool-load gate. This harness never runs a simulation, so every
    // criterion reads Unmodeled(SimulationRequired) — accept it, exactly
    // as a user would via the export gate override toggle. The gate
    // itself is covered by dedicated tests in core and below.
    gui.tool_load_overrides.accept_unmodeled = true;

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
    assert!(
        gcode.contains("G1"),
        "expected at least one feed move in output"
    );

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

    let ids: Vec<rs_cam_core::ToolpathId> = session
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

/// Wizard overrides — set on `WizardState` — must round-trip into the
/// emitted g-code through `export_gcode_from_session`. Exercises the
/// full data path: WizardState → overlay_for() → export helpers →
/// export_gcode_phases_with_overlay_checked → emitter.
#[test]
fn wizard_overlay_overrides_reflect_in_emitted_gcode() {
    let (mut session, mut gui, sim) = build_session();

    // grblHAL preamble emits {wcs_line} + {units_word} + M3 S{rpm}, so
    // overrides on those words show up in the rendered preamble.
    gui.post.format = rs_cam_core::gcode::PostFormat::GrblHal;

    session.wizard_mut().wcs_override = Some(rs_cam_core::gcode::WcsCode::G56);
    session.wizard_mut().spindle_warmup_secs = 9;

    let gcode = export_gcode_from_session(&session, &gui, &sim).expect("overlay export succeeds");

    assert!(
        gcode.contains("G56\n"),
        "WCS override should land in preamble: {gcode}"
    );
    assert!(
        gcode.contains("G4 P9\n"),
        "warmup override should inject dwell after preamble: {gcode}"
    );

    // Order check: dwell sits between preamble's M3 and the first move.
    let m3 = gcode.find("M3 S").expect("preamble M3");
    let dwell = gcode.find("G4 P9").expect("warmup dwell");
    let first_move = gcode.find("G0 X").expect("first move");
    assert!(m3 < dwell && dwell < first_move);
}

/// A1 (2026-06-11): an inch units override must hard-refuse the export —
/// the emitter only swaps the G21 word for G20 while every coordinate
/// stays in millimeters (a silent 25.4× scale error on a real machine).
#[test]
fn wizard_inch_units_override_blocks_export() {
    let (mut session, gui, sim) = build_session();
    session.wizard_mut().units_override = Some(rs_cam_core::gcode::Units::Inch);

    let err = export_gcode_from_session(&session, &gui, &sim)
        .expect_err("inch override must refuse export");
    let msg = err.to_string();
    assert!(
        msg.contains("inch output not yet supported"),
        "error explains the refusal: {msg}"
    );

    // Back to mm (or post default): export works again.
    session.wizard_mut().units_override = Some(rs_cam_core::gcode::Units::Mm);
    export_gcode_from_session(&session, &gui, &sim).expect("mm override exports");
    session.wizard_mut().units_override = None;
    export_gcode_from_session(&session, &gui, &sim).expect("post-default exports");
}

/// Default `WizardState` (no overrides) must produce byte-identical
/// output to the pre-overlay export path. Mirrors the gcode-level
/// `default_overlay_is_byte_identical_to_no_overlay` test, but at the
/// viz-export entry point that the wizard's Save handler calls.
#[test]
fn default_wizard_state_does_not_mutate_export() {
    use rs_cam_core::gcode::{ToolLoadExportPolicy, export_gcode_phases_with_overlay_checked};

    let (session, gui, sim) = build_session();
    // build_session leaves WizardState at its default — no overrides set.
    assert!(session.wizard().wcs_override.is_none());
    assert!(session.wizard().units_override.is_none());
    assert!(session.wizard().safe_z_override.is_none());
    assert_eq!(session.wizard().spindle_warmup_secs, 0);

    // Exercise the same data path the wizard's Save dispatches through.
    let with_overlay = export_gcode_from_session(&session, &gui, &sim).expect("export succeeds");

    // Build the equivalent default-overlay export by hand to confirm the
    // helper isn't injecting anything unexpected. Both paths route
    // through `export_gcode_phases_with_overlay_checked` — only the
    // overlay-construction step differs.
    use rs_cam_core::gcode::{CoolantMode, GcodePhase, PhaseTool};
    let phases: Vec<GcodePhase<'_>> = session
        .toolpath_configs()
        .iter()
        .filter(|tc| tc.enabled)
        .filter_map(|tc| {
            let rt = gui.toolpath_rt.get(&tc.id)?;
            let result = rt.result.as_ref()?;
            Some(GcodePhase {
                toolpath: result.toolpath(),
                spindle_rpm: rs_cam_core::compute::catalog::effective_spindle_rpm(
                    &tc.operation,
                    gui.post.spindle_speed,
                ),
                label: &tc.name,
                pre_gcode: tc.pre_gcode.as_deref(),
                post_gcode: tc.post_gcode.as_deref(),
                tool: session
                    .tools()
                    .iter()
                    .find(|t| t.id.0 == tc.tool_id)
                    .map(|t| PhaseTool {
                        id: t.id.0,
                        number: t.tool_number,
                        label: t.name.as_str(),
                    }),
                coolant: CoolantMode::Off,
                controller_compensation: None,
            })
        })
        .collect();
    let baseline = export_gcode_phases_with_overlay_checked(
        &phases,
        gui.post.format.definition(),
        // No load evaluation on the hand-built baseline path (C1 — the
        // report parameter is non-optional; empty = nothing evaluated).
        &rs_cam_core::tool_load::ToolLoadReport {
            per_toolpath: vec![],
        },
        ToolLoadExportPolicy::default(),
        &Default::default(),
    )
    .expect("baseline export succeeds");

    assert_eq!(
        with_overlay, baseline,
        "default wizard state must not mutate output"
    );
}

/// Fix 1 regression (WANAKA): the viz phase ASSEMBLY must read each
/// op's own spindle RPM, not flatten every phase to the project
/// default. Two toolpaths — one with an op-level RPM override, one
/// without — must emit an S-change between phases.
#[test]
fn viz_phase_assembly_uses_per_op_spindle_rpm() {
    let (mut session, mut gui, sim) = build_session();

    // Second toolpath on the same tool, with an op-level RPM override.
    let mut op = OperationConfig::Scallop(rs_cam_core::compute::ScallopConfig::default());
    op.set_spindle_rpm(Some(12_345));
    let tp2 = ToolpathConfig {
        id: rs_cam_core::ToolpathId(1),
        name: "Override Path".to_owned(),
        enabled: true,
        operation: op,
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
    };
    session.add_toolpath(0, tp2).expect("add second toolpath");
    let tp2_id = session.toolpath_configs()[1].id;

    let mut path = Toolpath::new();
    path.rapid_to(P3::new(20.0, 0.0, 5.0));
    path.feed_to(P3::new(30.0, 0.0, -1.0), 600.0);
    let mut rt = ToolpathRuntime::new(true);
    rt.result = Some(ToolpathResult {
        annotated: Arc::new(AnnotatedToolpath::new(path)),
        stats: Default::default(),
        debug_trace: None,
        semantic_trace: None,
        debug_trace_path: None,
        drill_op: None,
    });
    gui.toolpath_rt.insert(tp2_id, rt);

    let default_rpm = gui.post.spindle_speed;
    assert_ne!(default_rpm, 12_345, "test premise: override differs");

    let gcode = export_gcode_from_session(&session, &gui, &sim).expect("export succeeds");

    assert!(
        gcode.contains(&format!("M3 S{default_rpm}")),
        "first phase (no override) should use the project default RPM:\n{gcode}"
    );
    assert!(
        gcode.contains("M3 S12345"),
        "second phase's op-level RPM override must be emitted as an S change:\n{gcode}"
    );
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
    session.wizard_mut().dry_run = true;
    session.wizard_mut().allow_validator_errors = true;
    session.wizard_mut().last_step_visited = 5;

    let w = session.wizard();
    assert_eq!(w.output_layout, OutputLayout::PerToolpath);
    assert_eq!(w.filename_template, "{toolpath}.{ext}");
    assert_eq!(w.wcs_override, Some(rs_cam_core::gcode::WcsCode::G55));
    assert_eq!(w.units_override, Some(rs_cam_core::gcode::Units::Inch));
    assert_eq!(w.safe_z_override, Some(20.0));
    assert_eq!(w.spindle_warmup_secs, 5);
    assert!(w.dry_run);
    assert!(w.allow_validator_errors);
    assert_eq!(w.last_step_visited, 5);
}

/// Dry-run mode must clamp every cutting move's Z to the effective
/// safe-Z, and clamp every rapid UP to that floor too (C3, 2026-06-11:
/// dry runs never remove material, so rapids that descend below the
/// floor — e.g. drill peck re-entry — would drive into solid stock).
/// Build a session with a toolpath that descends below the surface
/// (Z=-1.0), enable dry-run + a safe-Z override of 12.5, and parse
/// every motion line of the emitted output: no Z word may be below
/// 12.500, and cutting moves sit exactly at 12.500.
#[test]
fn wizard_dry_run_clamps_cutting_moves_to_safe_z() {
    let (mut session, gui, sim) = build_session();

    session.wizard_mut().dry_run = true;
    session.wizard_mut().safe_z_override = Some(12.5);

    let gcode = export_gcode_from_session(&session, &gui, &sim).expect("dry-run export succeeds");

    let mut g1_count = 0usize;
    let mut clamped_rapid_count = 0usize;
    for line in gcode.lines() {
        let tline = line.trim_start();
        let leading = tline
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_ascii_uppercase();
        let is_cut = matches!(leading.as_str(), "G1" | "G2" | "G3");
        let is_rapid = leading == "G0";
        let z = tline.split_whitespace().find_map(|tok| {
            tok.strip_prefix('Z')
                .or_else(|| tok.strip_prefix('z'))
                .and_then(|rest| rest.parse::<f64>().ok())
        });
        if is_cut && let Some(z) = z {
            assert!(
                (z - 12.5).abs() < 1e-6,
                "dry-run cutting move must have Z=12.500, got {z} in: {line}"
            );
            g1_count += 1;
        }
        // build_session emits rapids at Z=5.0 — below the dry-run floor,
        // so they must be clamped UP to 12.5 (C3). No motion line may
        // command a Z below the floor.
        if is_rapid && let Some(z) = z {
            assert!(
                z >= 12.5 - 1e-6,
                "dry-run rapid must not descend below the floor: {line}"
            );
            if (z - 12.5).abs() < 1e-6 {
                clamped_rapid_count += 1;
            }
        }
    }
    assert!(
        g1_count >= 1,
        "test fixture should have produced at least one cutting move"
    );
    assert!(
        clamped_rapid_count >= 1,
        "build_session emits rapids at Z=5.0; dry-run must clamp them \
         up to the 12.5 floor (got {clamped_rapid_count} clamped rapids)"
    );
}

/// A custom `pause_message` on a setup must replace the default
/// `(Setup change: <name>)` text in the inter-setup `M0` block of the
/// emitted gcode. `None` (the default) keeps the existing wording.
/// Mirrors how the Step 4.5 UI's `WizardSetSetupPauseMessage` handler
/// mutates `setups_mut()[idx].pause_message`.
#[test]
fn wizard_setup_pause_message_lands_in_emitted_gcode() {
    use rs_cam_core::compute::transform::FaceUp;

    let (mut session, mut gui, sim) = build_session();

    // build_session has 1 setup + 1 toolpath. Add a second setup with
    // its own toolpath so the multi-setup emit path runs and emits the
    // inter-setup M0.
    let bottom_idx = session.add_setup("Bottom".to_owned(), FaceUp::Bottom);
    let bottom_id = session.list_setups()[bottom_idx].id;

    let tp_bottom = ToolpathConfig {
        id: rs_cam_core::ToolpathId(99),
        name: "Bottom Op".to_owned(),
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
    };
    session
        .add_toolpath(bottom_idx, tp_bottom)
        .expect("add bottom toolpath");
    let bottom_tp_id = session.list_setups()[bottom_idx]
        .toolpath_indices
        .first()
        .map(|&i| session.toolpath_configs()[i].id)
        .expect("bottom setup has a toolpath");

    let mut path = Toolpath::new();
    path.rapid_to(P3::new(0.0, 0.0, 5.0));
    path.feed_to(P3::new(20.0, 0.0, -1.0), 600.0);
    let mut rt = ToolpathRuntime::new(true);
    rt.result = Some(ToolpathResult {
        annotated: Arc::new(AnnotatedToolpath::new(path)),
        stats: Default::default(),
        debug_trace: None,
        semantic_trace: None,
        debug_trace_path: None,
        drill_op: None,
    });
    gui.toolpath_rt.insert(bottom_tp_id, rt);

    // The combined-gcode emit is what produces inter-setup M0s; the
    // SingleFile path concatenates phases without setup boundaries.
    // Default emit: pause text should mention the setup name.
    let default_gcode =
        export_combined_gcode_from_session(&session, &gui, &sim).expect("default export");
    assert!(
        default_gcode.contains("Setup change: Bottom"),
        "default pause text must mention the setup name: {default_gcode}"
    );
    assert!(
        !default_gcode.contains("Run Z Probe macro"),
        "default emit must not contain the override text"
    );

    // Override the second setup's pause_message — same mutation the
    // Step 4.5 AppEvent handler performs.
    if let Some(setup) = session.setups_mut().iter_mut().find(|s| s.id == bottom_id) {
        setup.pause_message = Some("Run Z Probe macro then Resume".to_owned());
    }

    let override_gcode =
        export_combined_gcode_from_session(&session, &gui, &sim).expect("override export");
    assert!(
        override_gcode.contains("Run Z Probe macro then Resume"),
        "override pause_message must replace the default text in the M0 block: {override_gcode}"
    );
    assert!(
        !override_gcode.contains("Setup change: Bottom"),
        "override must replace (not append to) the default text"
    );
}

/// Roadmap A regression: the MCP export path must read the viz-side worker
/// results (`gui.toolpath_rt[id].result`) rather than the empty
/// `session.results`. The bug previously produced a 0-byte output file.
/// This exercises the `_with_policy` variant the MCP wraps.
#[test]
fn export_with_policy_emits_gcode_from_viz_results() {
    let (session, gui, sim) = build_session();
    let policy = rs_cam_core::gcode::ToolLoadExportPolicy {
        accept_unmodeled: true,
        accept_exceeded: true,
    };
    let gcode = export_gcode_from_session_with_policy(&session, &gui, &sim, policy)
        .expect("policy export succeeds");
    assert!(
        !gcode.is_empty(),
        "policy export must not produce empty output"
    );
    assert!(
        gcode.contains("G1"),
        "expected at least one feed move: {gcode}"
    );
}
