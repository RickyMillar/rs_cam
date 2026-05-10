use rs_cam_core::gcode::{
    ControllerCompensation, GcodePhase, GcodeSetupPhase, WizardOverlay,
    export_gcode_multi_setup_with_overlay_checked, export_gcode_phases_with_overlay_checked,
    replace_rapids_with_feed,
};
use rs_cam_core::session::ProjectSession;

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
    }
}

fn tool_number_for_export(tool: &ToolConfig) -> u32 {
    tool.tool_number
}

fn gcode_phase_for_session_toolpath<'a>(
    session: &'a ProjectSession,
    gui: &'a GuiState,
    tc: &'a rs_cam_core::session::ToolpathConfig,
) -> Option<GcodePhase<'a>> {
    let rt = gui.toolpath_rt.get(&tc.id)?;
    let result = rt.result.as_ref()?;
    let tool = session.tools().iter().find(|t| t.id.0 == tc.tool_id);

    Some(GcodePhase {
        toolpath: result.toolpath(),
        spindle_rpm: gui.post.spindle_speed,
        label: &tc.name,
        pre_gcode: tc.pre_gcode.as_deref(),
        post_gcode: tc.post_gcode.as_deref(),
        tool_number: tool.map(tool_number_for_export),
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

/// Export all enabled toolpaths as a single G-code file (session-based).
pub fn export_gcode_from_session(
    session: &ProjectSession,
    gui: &GuiState,
    sim: &SimulationState,
) -> Result<String, crate::error::VizError> {
    let post = gui.post.format.definition();

    let phases: Vec<GcodePhase<'_>> = session
        .toolpath_configs()
        .iter()
        .filter(|tc| tc.enabled)
        .filter_map(|tc| gcode_phase_for_session_toolpath(session, gui, tc))
        .collect();

    if phases.is_empty() {
        return Err(crate::error::VizError::Export(
            "No computed toolpaths to export".to_owned(),
        ));
    }

    let mut gcode = export_gcode_phases_with_overlay_checked(
        &phases,
        post,
        viz_sim_trace(sim),
        gui.tool_load_overrides.as_policy(),
        &overlay_for(session, gui),
    )
    .map_err(|e| crate::error::VizError::Export(e.to_string()))?;

    if gui.post.high_feedrate_mode {
        gcode = replace_rapids_with_feed(&gcode, gui.post.high_feedrate);
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

    let setup_phases: Vec<GcodeSetupPhase<'_>> = session
        .list_setups()
        .iter()
        .filter_map(|setup| {
            let phases: Vec<GcodePhase<'_>> = setup
                .toolpath_indices
                .iter()
                .filter_map(|&tp_idx| session.toolpath_configs().get(tp_idx))
                .filter(|tc| tc.enabled)
                .filter_map(|tc| gcode_phase_for_session_toolpath(session, gui, tc))
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
        viz_sim_trace(sim),
        gui.tool_load_overrides.as_policy(),
        &overlay_for(session, gui),
    )
    .map_err(|e| crate::error::VizError::Export(e.to_string()))?;

    if gui.post.high_feedrate_mode {
        gcode = replace_rapids_with_feed(&gcode, gui.post.high_feedrate);
    }

    Ok(gcode)
}

/// Export a single toolpath (by semantic id) as G-code (session-based).
pub fn export_single_toolpath_from_session(
    session: &ProjectSession,
    gui: &GuiState,
    sim: &SimulationState,
    toolpath_id: usize,
) -> Result<String, crate::error::VizError> {
    let post = gui.post.format.definition();

    let tc = session
        .toolpath_configs()
        .iter()
        .find(|tc| tc.id == toolpath_id)
        .ok_or_else(|| {
            crate::error::VizError::Export(format!("Toolpath id {toolpath_id} not found"))
        })?;

    if !tc.enabled {
        return Err(crate::error::VizError::Export(format!(
            "Toolpath '{}' is disabled",
            tc.name
        )));
    }

    let phase = gcode_phase_for_session_toolpath(session, gui, tc).ok_or_else(|| {
        crate::error::VizError::Export(format!(
            "Toolpath '{}' has no computed result — generate it first",
            tc.name
        ))
    })?;

    let mut gcode = export_gcode_phases_with_overlay_checked(
        std::slice::from_ref(&phase),
        post,
        viz_sim_trace(sim),
        gui.tool_load_overrides.as_policy(),
        &overlay_for(session, gui),
    )
    .map_err(|e| crate::error::VizError::Export(e.to_string()))?;

    if gui.post.high_feedrate_mode {
        gcode = replace_rapids_with_feed(&gcode, gui.post.high_feedrate);
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
    let setup = session
        .list_setups()
        .iter()
        .find(|s| s.id == setup_id.0)
        .ok_or_else(|| crate::error::VizError::Export(format!("Setup {setup_id:?} not found")))?;

    let post = gui.post.format.definition();

    let phases: Vec<GcodePhase<'_>> = setup
        .toolpath_indices
        .iter()
        .filter_map(|&tp_idx| session.toolpath_configs().get(tp_idx))
        .filter(|tc| tc.enabled)
        .filter_map(|tc| gcode_phase_for_session_toolpath(session, gui, tc))
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
        viz_sim_trace(sim),
        gui.tool_load_overrides.as_policy(),
        &overlay_for(session, gui),
    )
    .map_err(|e| crate::error::VizError::Export(e.to_string()))?;

    if gui.post.high_feedrate_mode {
        gcode = replace_rapids_with_feed(&gcode, gui.post.high_feedrate);
    }

    Ok(gcode)
}
