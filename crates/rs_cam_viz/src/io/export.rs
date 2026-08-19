use rs_cam_core::gcode::{
    ControllerCompensation, GcodePhase, GcodeSetupPhase, PhaseTool, ToolLoadExportPolicy,
    WizardOverlay, export_gcode_multi_setup_with_overlay_checked,
    export_gcode_phases_with_overlay_checked, replace_rapids_with_feed,
};
use rs_cam_core::gcode_validator::{MachineSafety, Severity, validate_machine_safety};
use rs_cam_core::session::ProjectSession;

/// Run the machine-safety pass over freshly emitted G-code and log any
/// findings (non-blocking). Uses the post's safe-Z as the clearance
/// plane; the depth-floor and feed-cap checks stay off until the machine
/// profile is plumbed through (turning them on without correct limits
/// would risk false positives). Runs before any high-feedrate rapid→feed
/// rewrite so the rapid structure is still intact to check.
fn log_machine_safety(gcode: &str, safe_z: f64) {
    let findings = validate_machine_safety(
        gcode,
        MachineSafety {
            clearance_z: safe_z,
            min_z: None,
            max_feed_mm_min: None,
        },
    );
    if findings.is_empty() {
        return;
    }
    let errors = findings
        .iter()
        .filter(|f| f.severity == Severity::Error)
        .count();
    tracing::warn!(
        total = findings.len(),
        errors,
        "g-code machine-safety pass raised findings; first: line {} — {}",
        findings.first().map(|f| f.line).unwrap_or(0),
        findings.first().map(|f| f.message.as_str()).unwrap_or(""),
    );
}

use crate::state::job::ToolConfig;
use crate::state::runtime::GuiState;
use crate::state::simulation::SimulationState;
use crate::state::toolpath::{CompensationType, OperationConfig, ProfileSide};

/// Pull the wizard's per-job overrides into a `WizardOverlay` for the
/// emit step. The default overlay (no fields set) is byte-identical to
/// the pre-overlay export path.
///
/// Dry-run resolution: when `wizard.dry_run` is true, `dry_run_safe_z`
/// is set to the effective safe-Z (`wizard.safe_z_override` or
/// `gui.post.safe_z`). Otherwise it stays `None` and the overlay's
/// `apply_to_program` no-ops on cutting Z values.
fn overlay_for(session: &ProjectSession, gui: &GuiState) -> WizardOverlay {
    let w = session.wizard();
    let dry_run_safe_z = if w.dry_run {
        Some(w.safe_z_override.unwrap_or(gui.post.safe_z))
    } else {
        None
    };
    WizardOverlay {
        wcs_override: w.wcs_override,
        units_override: w.units_override,
        safe_z_override: w.safe_z_override,
        spindle_warmup_secs: w.spindle_warmup_secs,
        dry_run_safe_z,
        tool_change_override: w.tool_change_override,
    }
}

fn phase_tool_for_export(tool: &ToolConfig) -> PhaseTool<'_> {
    PhaseTool {
        id: tool.id.0,
        number: tool.tool_number,
        label: tool.name.as_str(),
    }
}

/// G-EXPORT-DATUM (2026-08-19): collect each toolpath re-expressed in the
/// shared export datum (stock-relative XY — see
/// [`rs_cam_core::gcode::export_datum_shift_for_toolpath`]). Returned as an
/// owning vec because [`GcodePhase`] borrows its toolpath and the shifted
/// copy has to outlive the phases. Zero-shift toolpaths (non-identity
/// setups, zero-origin stock) stay borrowed — no clone.
///
/// `indices` are indices into `session.toolpath_configs()`; the setup a
/// toolpath belongs to (hence its emission frame) is resolved from there.
fn emitted_toolpaths<'a>(
    session: &'a ProjectSession,
    gui: &'a GuiState,
    indices: impl Iterator<Item = usize>,
) -> Vec<(usize, std::borrow::Cow<'a, rs_cam_core::toolpath::Toolpath>)> {
    indices
        .filter_map(|idx| {
            let tc = session.toolpath_configs().get(idx)?;
            if !tc.enabled {
                return None;
            }
            let result = gui.toolpath_rt.get(&tc.id)?.result.as_ref()?;
            let shift = rs_cam_core::gcode::export_datum_shift_for_toolpath(session, idx);
            Some((
                idx,
                rs_cam_core::gcode::toolpath_in_export_datum(result.toolpath(), shift),
            ))
        })
        .collect()
}

fn gcode_phase_for_session_toolpath<'a>(
    session: &'a ProjectSession,
    gui: &'a GuiState,
    tc: &'a rs_cam_core::session::ToolpathConfig,
    toolpath: &'a rs_cam_core::toolpath::Toolpath,
) -> Option<GcodePhase<'a>> {
    let tool = session.tools().iter().find(|t| t.id.0 == tc.tool_id);

    Some(GcodePhase {
        toolpath,
        // Per-toolpath spindle RPM: the op's own override wins, the
        // project default is only a fallback — same resolution as the
        // core export path (`export_gcode_checked`). Pre-fix this read
        // `gui.post.spindle_speed` unconditionally, flattening every
        // op's RPM (WANAKA's 12194/10610/21000) to the project default.
        spindle_rpm: rs_cam_core::compute::catalog::effective_spindle_rpm(
            &tc.operation,
            gui.post.spindle_speed,
        ),
        label: &tc.name,
        pre_gcode: tc.pre_gcode.as_deref(),
        post_gcode: tc.post_gcode.as_deref(),
        tool: tool.map(phase_tool_for_export),
        coolant: tc.coolant,
        controller_compensation: controller_comp_for_session_toolpath(tc),
    })
}

fn controller_comp_for_session_toolpath(
    tc: &rs_cam_core::session::ToolpathConfig,
) -> Option<ControllerCompensation> {
    if let OperationConfig::Profile(ref cfg) = tc.operation
        && cfg.compensation == CompensationType::InControl
    {
        let dir = match (cfg.side, cfg.climb) {
            (ProfileSide::Outside, true) => ControllerCompensation::Right,
            (ProfileSide::Outside, false) => ControllerCompensation::Left,
            (ProfileSide::Inside, true) => ControllerCompensation::Left,
            (ProfileSide::Inside, false) => ControllerCompensation::Right,
        };
        return Some(dir);
    }
    None
}

/// The cut trace lives on viz `SimulationState`, not on `session.simulation`.
/// Pull from there so the export gate evaluates chipload/power against the
/// active simulation run.
fn viz_sim_trace(
    sim: &SimulationState,
) -> Option<&rs_cam_core::simulation_cut::SimulationCutTrace> {
    sim.results.as_ref().and_then(|r| r.cut_trace.as_deref())
}

/// Build the tool-load report the checked export functions enforce the
/// policy against (C1, 2026-06-11). Evaluated from the shared session +
/// the viz-side cut trace — the same inputs the readiness panel and the
/// MCP `tool_load_report` tool use, so what the user sees is what the
/// gate enforces.
fn viz_load_report(
    session: &ProjectSession,
    sim: &SimulationState,
) -> rs_cam_core::tool_load::ToolLoadReport {
    rs_cam_core::gcode::project_load_report(session, viz_sim_trace(sim))
}

/// Export all enabled toolpaths as a single G-code file (session-based).
pub fn export_gcode_from_session(
    session: &ProjectSession,
    gui: &GuiState,
    sim: &SimulationState,
) -> Result<String, crate::error::VizError> {
    export_gcode_from_session_with_policy(session, gui, sim, gui.tool_load_overrides.as_policy())
}

/// Same as [`export_gcode_from_session`], but lets the caller supply an
/// explicit tool-load policy instead of reading `gui.tool_load_overrides`.
/// Used by the MCP export path so an automation client can pass
/// accept_unmodeled / accept_exceeded directly without mutating GUI state.
pub fn export_gcode_from_session_with_policy(
    session: &ProjectSession,
    gui: &GuiState,
    sim: &SimulationState,
    policy: ToolLoadExportPolicy,
) -> Result<String, crate::error::VizError> {
    let post = gui.post.format.definition();

    let emitted = emitted_toolpaths(session, gui, 0..session.toolpath_configs().len());
    let phases: Vec<GcodePhase<'_>> = emitted
        .iter()
        .filter_map(|(idx, tp)| {
            let tc = session.toolpath_configs().get(*idx)?;
            gcode_phase_for_session_toolpath(session, gui, tc, tp.as_ref())
        })
        .collect();

    if phases.is_empty() {
        return Err(crate::error::VizError::Export(
            "No computed toolpaths to export".to_owned(),
        ));
    }

    let mut gcode = export_gcode_phases_with_overlay_checked(
        &phases,
        post,
        &viz_load_report(session, sim),
        policy,
        &overlay_for(session, gui),
    )
    .map_err(|e| crate::error::VizError::Export(e.to_string()))?;

    log_machine_safety(&gcode, gui.post.safe_z);

    if gui.post.high_feedrate_mode {
        gcode = replace_rapids_with_feed(&gcode, gui.post.high_feedrate, post);
    }

    Ok(gcode)
}

/// Export all setups as a single G-code file with M0 pauses (session-based).
pub fn export_combined_gcode_from_session(
    session: &ProjectSession,
    gui: &GuiState,
    sim: &SimulationState,
) -> Result<String, crate::error::VizError> {
    let post = gui.post.format.definition();

    // Shifted toolpaths must outlive the borrowed phases, so build the
    // whole project's set up front and slice it per setup below.
    let emitted = emitted_toolpaths(session, gui, 0..session.toolpath_configs().len());
    let setup_phases: Vec<GcodeSetupPhase<'_>> = session
        .list_setups()
        .iter()
        .filter_map(|setup| {
            // Driven by `setup.toolpath_indices`, NOT by `emitted`'s own
            // order: the setup's list is the machining order and it is
            // not necessarily ascending after an op reorder.
            let phases: Vec<GcodePhase<'_>> = setup
                .toolpath_indices
                .iter()
                .filter_map(|tp_idx| {
                    let (_, tp) = emitted.iter().find(|(idx, _)| idx == tp_idx)?;
                    let tc = session.toolpath_configs().get(*tp_idx)?;
                    gcode_phase_for_session_toolpath(session, gui, tc, tp.as_ref())
                })
                .collect();
            if phases.is_empty() {
                None
            } else {
                Some(GcodeSetupPhase {
                    setup_label: &setup.name,
                    phases,
                    pause_message: setup.pause_message.as_deref(),
                })
            }
        })
        .collect();

    if setup_phases.is_empty() {
        return Err(crate::error::VizError::Export(
            "No computed toolpaths to export".to_owned(),
        ));
    }

    let mut gcode = export_gcode_multi_setup_with_overlay_checked(
        &setup_phases,
        post,
        gui.post.safe_z,
        &viz_load_report(session, sim),
        gui.tool_load_overrides.as_policy(),
        &overlay_for(session, gui),
    )
    .map_err(|e| crate::error::VizError::Export(e.to_string()))?;

    log_machine_safety(&gcode, gui.post.safe_z);

    if gui.post.high_feedrate_mode {
        gcode = replace_rapids_with_feed(&gcode, gui.post.high_feedrate, post);
    }

    Ok(gcode)
}

/// Export a single toolpath (by semantic id) as G-code (session-based).
pub fn export_single_toolpath_from_session(
    session: &ProjectSession,
    gui: &GuiState,
    sim: &SimulationState,
    toolpath_id: rs_cam_core::ToolpathId,
) -> Result<String, crate::error::VizError> {
    let post = gui.post.format.definition();

    let tp_index = session
        .toolpath_configs()
        .iter()
        .position(|tc| tc.id == toolpath_id)
        .ok_or_else(|| {
            crate::error::VizError::Export(format!("Toolpath id {toolpath_id} not found"))
        })?;
    let tc = session.toolpath_configs().get(tp_index).ok_or_else(|| {
        crate::error::VizError::Export(format!("Toolpath id {toolpath_id} not found"))
    })?;

    if !tc.enabled {
        return Err(crate::error::VizError::Export(format!(
            "Toolpath '{}' is disabled",
            tc.name
        )));
    }

    let emitted = emitted_toolpaths(session, gui, std::iter::once(tp_index));
    let (_, emitted_toolpath) = emitted.first().ok_or_else(|| {
        crate::error::VizError::Export(format!(
            "Toolpath '{}' has no computed result — generate it first",
            tc.name
        ))
    })?;
    let phase = gcode_phase_for_session_toolpath(session, gui, tc, emitted_toolpath.as_ref())
        .ok_or_else(|| {
            crate::error::VizError::Export(format!(
                "Toolpath '{}' has no computed result — generate it first",
                tc.name
            ))
        })?;

    let mut gcode = export_gcode_phases_with_overlay_checked(
        std::slice::from_ref(&phase),
        post,
        &viz_load_report(session, sim),
        gui.tool_load_overrides.as_policy(),
        &overlay_for(session, gui),
    )
    .map_err(|e| crate::error::VizError::Export(e.to_string()))?;

    log_machine_safety(&gcode, gui.post.safe_z);

    if gui.post.high_feedrate_mode {
        gcode = replace_rapids_with_feed(&gcode, gui.post.high_feedrate, post);
    }

    Ok(gcode)
}

/// Export only the toolpaths from a single setup as G-code (session-based).
pub fn export_setup_gcode_from_session(
    session: &ProjectSession,
    gui: &GuiState,
    sim: &SimulationState,
    setup_id: crate::state::job::SetupId,
) -> Result<String, crate::error::VizError> {
    export_setup_gcode_from_session_with_policy(
        session,
        gui,
        sim,
        setup_id,
        gui.tool_load_overrides.as_policy(),
    )
}

/// Same as [`export_setup_gcode_from_session`], but lets the caller supply
/// an explicit tool-load policy (e.g. the MCP `accept_*` flags) instead of
/// defaulting to the GUI's overrides.
pub fn export_setup_gcode_from_session_with_policy(
    session: &ProjectSession,
    gui: &GuiState,
    sim: &SimulationState,
    setup_id: crate::state::job::SetupId,
    policy: ToolLoadExportPolicy,
) -> Result<String, crate::error::VizError> {
    let setup = session
        .list_setups()
        .iter()
        .find(|s| s.id == setup_id.0)
        .ok_or_else(|| crate::error::VizError::Export(format!("Setup {setup_id:?} not found")))?;

    let post = gui.post.format.definition();

    let emitted = emitted_toolpaths(session, gui, setup.toolpath_indices.iter().copied());
    let phases: Vec<GcodePhase<'_>> = emitted
        .iter()
        .filter_map(|(idx, tp)| {
            let tc = session.toolpath_configs().get(*idx)?;
            gcode_phase_for_session_toolpath(session, gui, tc, tp.as_ref())
        })
        .collect();

    if phases.is_empty() {
        return Err(crate::error::VizError::Export(format!(
            "No computed toolpaths in setup '{}'",
            setup.name,
        )));
    }

    let mut gcode = export_gcode_phases_with_overlay_checked(
        &phases,
        post,
        &viz_load_report(session, sim),
        policy,
        &overlay_for(session, gui),
    )
    .map_err(|e| crate::error::VizError::Export(e.to_string()))?;

    log_machine_safety(&gcode, gui.post.safe_z);

    if gui.post.high_feedrate_mode {
        gcode = replace_rapids_with_feed(&gcode, gui.post.high_feedrate, post);
    }

    Ok(gcode)
}
