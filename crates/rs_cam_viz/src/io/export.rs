use rs_cam_core::gcode::{GcodePhase, emit_gcode_phased, get_post_processor};

use crate::state::job::{JobState, PostFormat};

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
        .toolpaths
        .iter()
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

    Ok(emit_gcode_phased(&phases, post.as_ref()))
}
