use rs_cam_core::gcode::{
    ControllerCompensation, GcodePhase, GcodeSetupPhase, emit_gcode_multi_setup, emit_gcode_phased,
    replace_rapids_with_feed,
};
use rs_cam_core::session::ProjectSession;

use crate::state::job::ToolConfig;
use crate::state::runtime::GuiState;
use crate::state::toolpath::{CompensationType, OperationConfig, ProfileSide};

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
        toolpath: &result.toolpath,
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

/// Export all enabled toolpaths as a single G-code file (session-based).
pub fn export_gcode_from_session(
    session: &ProjectSession,
    gui: &GuiState,
) -> Result<String, crate::error::VizError> {
    let post = gui.post.format.post_processor();

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

    let mut gcode = emit_gcode_phased(&phases, post.as_ref());

    if gui.post.high_feedrate_mode {
        gcode = replace_rapids_with_feed(&gcode, gui.post.high_feedrate);
    }

    Ok(gcode)
}

/// Export all setups as a single G-code file with M0 pauses (session-based).
pub fn export_combined_gcode_from_session(
    session: &ProjectSession,
    gui: &GuiState,
) -> Result<String, crate::error::VizError> {
    let post = gui.post.format.post_processor();

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
                })
            }
        })
        .collect();

    if setup_phases.is_empty() {
        return Err(crate::error::VizError::Export(
            "No computed toolpaths to export".to_owned(),
        ));
    }

    let mut gcode = emit_gcode_multi_setup(&setup_phases, post.as_ref(), gui.post.safe_z);

    if gui.post.high_feedrate_mode {
        gcode = replace_rapids_with_feed(&gcode, gui.post.high_feedrate);
    }

    Ok(gcode)
}

/// Export only the toolpaths from a single setup as G-code (session-based).
pub fn export_setup_gcode_from_session(
    session: &ProjectSession,
    gui: &GuiState,
    setup_id: crate::state::job::SetupId,
) -> Result<String, crate::error::VizError> {
    let setup = session
        .list_setups()
        .iter()
        .find(|s| s.id == setup_id.0)
        .ok_or_else(|| crate::error::VizError::Export(format!("Setup {setup_id:?} not found")))?;

    let post = gui.post.format.post_processor();

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

    let mut gcode = emit_gcode_phased(&phases, post.as_ref());

    if gui.post.high_feedrate_mode {
        gcode = replace_rapids_with_feed(&gcode, gui.post.high_feedrate);
    }

    Ok(gcode)
}
