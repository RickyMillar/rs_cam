use rs_cam_core::gcode::{
    GcodePhase, GcodeSetupPhase, emit_gcode_multi_setup, emit_gcode_phased, get_post_processor,
    replace_rapids_with_feed,
};

use crate::state::job::{JobState, PostFormat, SetupId, ToolConfig};
use crate::state::toolpath::{ToolpathEntry, ToolpathResult};

fn gcode_phase_for_toolpath<'a>(
    job: &'a JobState,
    toolpath: &'a ToolpathEntry,
    result: &'a ToolpathResult,
) -> GcodePhase<'a> {
    let tool = job.tools.iter().find(|tool| tool.id == toolpath.tool_id);

    GcodePhase {
        toolpath: &result.toolpath,
        spindle_rpm: job.post.spindle_speed,
        label: &toolpath.name,
        pre_gcode: if toolpath.pre_gcode.is_empty() {
            None
        } else {
            Some(&toolpath.pre_gcode)
        },
        post_gcode: if toolpath.post_gcode.is_empty() {
            None
        } else {
            Some(&toolpath.post_gcode)
        },
        tool_number: tool.map(tool_number_for_export),
        coolant: toolpath.coolant,
    }
}

fn tool_number_for_export(tool: &ToolConfig) -> u32 {
    tool.tool_number
}

/// Export all enabled toolpaths as a single G-code file.
pub fn export_gcode(job: &JobState) -> Result<String, crate::error::VizError> {
    let post_name = match job.post.format {
        PostFormat::Grbl => "grbl",
        PostFormat::LinuxCnc => "linuxcnc",
        PostFormat::Mach3 => "mach3",
    };
    let post = get_post_processor(post_name).ok_or_else(|| {
        crate::error::VizError::Export(format!("Unknown post processor: {post_name}"))
    })?;

    let phases: Vec<GcodePhase<'_>> = job
        .all_toolpaths()
        .filter(|tp| tp.enabled)
        .filter_map(|tp| {
            tp.result
                .as_ref()
                .map(|result| gcode_phase_for_toolpath(job, tp, result))
        })
        .collect();

    if phases.is_empty() {
        return Err(crate::error::VizError::Export(
            "No computed toolpaths to export".to_string(),
        ));
    }

    let mut gcode = emit_gcode_phased(&phases, post.as_ref());

    // Apply high feedrate mode: convert G0 rapids to G1 at specified feedrate
    if job.post.high_feedrate_mode {
        gcode = replace_rapids_with_feed(&gcode, job.post.high_feedrate);
    }

    Ok(gcode)
}

/// Export all setups as a single G-code file with M0 pauses between setups.
pub fn export_combined_gcode(job: &JobState) -> Result<String, crate::error::VizError> {
    let post_name = match job.post.format {
        PostFormat::Grbl => "grbl",
        PostFormat::LinuxCnc => "linuxcnc",
        PostFormat::Mach3 => "mach3",
    };
    let post = get_post_processor(post_name).ok_or_else(|| {
        crate::error::VizError::Export(format!("Unknown post processor: {post_name}"))
    })?;

    let setup_phases: Vec<GcodeSetupPhase<'_>> = job
        .setups
        .iter()
        .filter_map(|setup| {
            let phases: Vec<GcodePhase<'_>> = setup
                .toolpaths
                .iter()
                .filter(|tp| tp.enabled)
                .filter_map(|tp| {
                    tp.result
                        .as_ref()
                        .map(|result| gcode_phase_for_toolpath(job, tp, result))
                })
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
            "No computed toolpaths to export".to_string(),
        ));
    }

    let mut gcode = emit_gcode_multi_setup(&setup_phases, post.as_ref(), job.post.safe_z);

    if job.post.high_feedrate_mode {
        gcode = replace_rapids_with_feed(&gcode, job.post.high_feedrate);
    }

    Ok(gcode)
}

/// Export only the toolpaths from a single setup as G-code.
pub fn export_setup_gcode(
    job: &JobState,
    setup_id: SetupId,
) -> Result<String, crate::error::VizError> {
    let setup = job
        .setups
        .iter()
        .find(|setup| setup.id == setup_id)
        .ok_or_else(|| crate::error::VizError::Export(format!("Setup {setup_id:?} not found")))?;

    let post_name = match job.post.format {
        PostFormat::Grbl => "grbl",
        PostFormat::LinuxCnc => "linuxcnc",
        PostFormat::Mach3 => "mach3",
    };
    let post = get_post_processor(post_name).ok_or_else(|| {
        crate::error::VizError::Export(format!("Unknown post processor: {post_name}"))
    })?;

    let phases: Vec<GcodePhase<'_>> = setup
        .toolpaths
        .iter()
        .filter(|tp| tp.enabled)
        .filter_map(|tp| {
            tp.result
                .as_ref()
                .map(|result| gcode_phase_for_toolpath(job, tp, result))
        })
        .collect();

    if phases.is_empty() {
        return Err(crate::error::VizError::Export(format!(
            "No computed toolpaths in setup '{}'",
            setup.name,
        )));
    }

    let mut gcode = emit_gcode_phased(&phases, post.as_ref());

    if job.post.high_feedrate_mode {
        gcode = replace_rapids_with_feed(&gcode, job.post.high_feedrate);
    }

    Ok(gcode)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::sync::Arc;

    use rs_cam_core::gcode::CoolantMode;
    use rs_cam_core::geo::P3;
    use rs_cam_core::toolpath::Toolpath;

    use super::*;
    use crate::state::job::{JobState, ModelId, ToolConfig, ToolId, ToolType};
    use crate::state::toolpath::{OperationType, ToolpathEntry, ToolpathId, ToolpathResult};

    fn sample_tool(id: ToolId, tool_number: u32) -> ToolConfig {
        let mut tool = ToolConfig::new_default(id, ToolType::EndMill);
        tool.name = format!("Tool {}", id.0 + 1);
        tool.tool_number = tool_number;
        tool
    }

    fn sample_result() -> ToolpathResult {
        let mut toolpath = Toolpath::new();
        toolpath.rapid_to(P3::new(0.0, 0.0, 5.0));
        toolpath.feed_to(P3::new(5.0, 0.0, -1.0), 500.0);
        ToolpathResult {
            toolpath: Arc::new(toolpath),
            stats: Default::default(),
            debug_trace: None,
            semantic_trace: None,
            debug_trace_path: None,
        }
    }

    #[test]
    fn export_gcode_threads_tool_numbers_and_coolant() {
        let mut job = JobState::new();
        job.tools.push(sample_tool(ToolId(0), 1));
        job.tools.push(sample_tool(ToolId(1), 7));

        let mut tp1 = ToolpathEntry::for_operation(
            ToolpathId(0),
            "Pocket".to_string(),
            ToolId(0),
            ModelId(0),
            OperationType::Pocket,
        );
        tp1.coolant = CoolantMode::Flood;
        tp1.result = Some(sample_result());

        let mut tp2 = ToolpathEntry::for_operation(
            ToolpathId(1),
            "Profile".to_string(),
            ToolId(1),
            ModelId(0),
            OperationType::Profile,
        );
        tp2.coolant = CoolantMode::Mist;
        tp2.result = Some(sample_result());

        job.push_toolpath(tp1);
        job.push_toolpath(tp2);

        let gcode = export_gcode(&job).expect("export gcode");

        assert!(
            gcode.contains("M8"),
            "first phase should enable flood coolant"
        );
        assert!(
            gcode.contains("M6 T7"),
            "second phase should emit configured tool number"
        );
        assert!(
            gcode.contains("M7"),
            "second phase should enable mist coolant"
        );
    }
}
