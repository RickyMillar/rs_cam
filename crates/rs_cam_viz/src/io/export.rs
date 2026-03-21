use rs_cam_core::gcode::{
    GcodePhase, GcodeSetupPhase, emit_gcode_multi_setup, emit_gcode_phased, get_post_processor,
    replace_rapids_with_feed,
};

use crate::state::job::{JobState, PostFormat, SetupId};

/// Export all enabled toolpaths as a single G-code file.
pub fn export_gcode(job: &JobState) -> Result<String, String> {
    let post_name = match job.post.format {
        PostFormat::Grbl => "grbl",
        PostFormat::LinuxCnc => "linuxcnc",
        PostFormat::Mach3 => "mach3",
    };
    let post = get_post_processor(post_name)
        .ok_or_else(|| format!("Unknown post processor: {}", post_name))?;

    let phases: Vec<GcodePhase<'_>> = job
        .all_toolpaths()
        .filter(|tp| tp.enabled)
        .filter_map(|tp| {
            tp.result.as_ref().map(|r| GcodePhase {
                toolpath: &r.toolpath,
                spindle_rpm: job.post.spindle_speed,
                label: &tp.name,
            })
        })
        .collect();

    if phases.is_empty() {
        return Err("No computed toolpaths to export".to_string());
    }

    let mut gcode = emit_gcode_phased(&phases, post.as_ref());

    // Apply high feedrate mode: convert G0 rapids to G1 at specified feedrate
    if job.post.high_feedrate_mode {
        gcode = replace_rapids_with_feed(&gcode, job.post.high_feedrate);
    }

    Ok(gcode)
}

/// Export all setups as a single G-code file with M0 pauses between setups.
pub fn export_combined_gcode(job: &JobState) -> Result<String, String> {
    let post_name = match job.post.format {
        PostFormat::Grbl => "grbl",
        PostFormat::LinuxCnc => "linuxcnc",
        PostFormat::Mach3 => "mach3",
    };
    let post = get_post_processor(post_name)
        .ok_or_else(|| format!("Unknown post processor: {}", post_name))?;

    let setup_phases: Vec<GcodeSetupPhase<'_>> = job
        .setups
        .iter()
        .filter_map(|setup| {
            let phases: Vec<GcodePhase<'_>> = setup
                .toolpaths
                .iter()
                .filter(|tp| tp.enabled)
                .filter_map(|tp| {
                    tp.result.as_ref().map(|result| GcodePhase {
                        toolpath: &result.toolpath,
                        spindle_rpm: job.post.spindle_speed,
                        label: &tp.name,
                    })
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
        return Err("No computed toolpaths to export".to_string());
    }

    let mut gcode = emit_gcode_multi_setup(&setup_phases, post.as_ref(), job.post.safe_z);

    if job.post.high_feedrate_mode {
        gcode = replace_rapids_with_feed(&gcode, job.post.high_feedrate);
    }

    Ok(gcode)
}

/// Export only the toolpaths from a single setup as G-code.
pub fn export_setup_gcode(job: &JobState, setup_id: SetupId) -> Result<String, String> {
    let setup = job
        .setups
        .iter()
        .find(|setup| setup.id == setup_id)
        .ok_or_else(|| format!("Setup {:?} not found", setup_id))?;

    let post_name = match job.post.format {
        PostFormat::Grbl => "grbl",
        PostFormat::LinuxCnc => "linuxcnc",
        PostFormat::Mach3 => "mach3",
    };
    let post = get_post_processor(post_name)
        .ok_or_else(|| format!("Unknown post processor: {}", post_name))?;

    let phases: Vec<GcodePhase<'_>> = setup
        .toolpaths
        .iter()
        .filter(|tp| tp.enabled)
        .filter_map(|tp| {
            tp.result.as_ref().map(|result| GcodePhase {
                toolpath: &result.toolpath,
                spindle_rpm: job.post.spindle_speed,
                label: &tp.name,
            })
        })
        .collect();

    if phases.is_empty() {
        return Err(format!("No computed toolpaths in setup '{}'", setup.name));
    }

    let mut gcode = emit_gcode_phased(&phases, post.as_ref());

    if job.post.high_feedrate_mode {
        gcode = replace_rapids_with_feed(&gcode, job.post.high_feedrate);
    }

    Ok(gcode)
}
