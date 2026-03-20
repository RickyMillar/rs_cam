use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::state::job::*;
// Re-import specific types used in serialization logic
use crate::state::job::{ToolMaterial, CutDirection};

/// Serializable job file format, compatible with the CLI TOML format.
#[derive(Serialize, Deserialize)]
pub struct ProjectFile {
    pub job: JobSection,
    #[serde(default)]
    pub tools: Vec<ToolSection>,
    #[serde(default)]
    pub toolpaths: Vec<ToolpathSection>,
}

#[derive(Serialize, Deserialize)]
pub struct JobSection {
    pub name: String,
    #[serde(default)]
    pub post: String,
    #[serde(default = "default_spindle")]
    pub spindle_speed: u32,
    #[serde(default = "default_safe_z")]
    pub safe_z: f64,
    pub stock_x: f64,
    pub stock_y: f64,
    pub stock_z: f64,
    #[serde(default)]
    pub stock_origin_x: f64,
    #[serde(default)]
    pub stock_origin_y: f64,
    #[serde(default)]
    pub stock_origin_z: f64,
    #[serde(default)]
    pub material: String,
    #[serde(default)]
    pub machine: String,
}

fn default_spindle() -> u32 { 18000 }
fn default_safe_z() -> f64 { 10.0 }

#[derive(Serialize, Deserialize)]
pub struct ToolSection {
    pub name: String,
    #[serde(rename = "type")]
    pub tool_type: String,
    pub diameter: f64,
    #[serde(default = "default_length")]
    pub cutting_length: f64,
    #[serde(default)]
    pub corner_radius: f64,
    #[serde(default = "default_angle")]
    pub included_angle: f64,
    #[serde(default)]
    pub taper_half_angle: f64,
    #[serde(default)]
    pub shaft_diameter: f64,
    #[serde(default = "default_flutes")]
    pub flute_count: u32,
    #[serde(default)]
    pub tool_material: String,
    #[serde(default)]
    pub cut_direction: String,
}

fn default_length() -> f64 { 25.0 }
fn default_angle() -> f64 { 90.0 }
fn default_flutes() -> u32 { 2 }

#[derive(Serialize, Deserialize)]
pub struct ToolpathSection {
    pub name: String,
    #[serde(rename = "type")]
    pub op_type: String,
    pub tool_index: usize,
    #[serde(default)]
    pub input: String,
    #[serde(flatten)]
    pub params: toml::Value,
}

/// Save the current job state to a TOML file.
pub fn save_project(job: &JobState, path: &Path) -> Result<(), String> {
    let tools: Vec<ToolSection> = job.tools.iter().map(|t| ToolSection {
        name: t.name.clone(),
        tool_type: match t.tool_type {
            ToolType::EndMill => "flat".into(),
            ToolType::BallNose => "ball".into(),
            ToolType::BullNose => "bullnose".into(),
            ToolType::VBit => "vbit".into(),
            ToolType::TaperedBallNose => "tapered_ball".into(),
        },
        diameter: t.diameter,
        cutting_length: t.cutting_length,
        corner_radius: t.corner_radius,
        included_angle: t.included_angle,
        taper_half_angle: t.taper_half_angle,
        shaft_diameter: t.shaft_diameter,
        flute_count: t.flute_count,
        tool_material: match t.tool_material {
            ToolMaterial::Carbide => "carbide".into(),
            ToolMaterial::Hss => "hss".into(),
        },
        cut_direction: match t.cut_direction {
            CutDirection::UpCut => "upcut".into(),
            CutDirection::DownCut => "downcut".into(),
            CutDirection::Compression => "compression".into(),
        },
    }).collect();

    let toolpaths: Vec<ToolpathSection> = job.toolpaths.iter().map(|tp| {
        let tool_idx = job.tools.iter().position(|t| t.id == tp.tool_id).unwrap_or(0);
        ToolpathSection {
            name: tp.name.clone(),
            op_type: tp.operation.label().to_lowercase().replace(' ', "_"),
            tool_index: tool_idx,
            input: job.models.iter().find(|m| m.id == tp.model_id)
                .map(|m| m.path.display().to_string()).unwrap_or_default(),
            params: toml::Value::Table(toml::map::Map::new()),
        }
    }).collect();

    let post_name = match job.post.format {
        PostFormat::Grbl => "grbl",
        PostFormat::LinuxCnc => "linuxcnc",
        PostFormat::Mach3 => "mach3",
    };

    let project = ProjectFile {
        job: JobSection {
            name: job.name.clone(),
            post: post_name.into(),
            spindle_speed: job.post.spindle_speed,
            safe_z: job.post.safe_z,
            stock_x: job.stock.x,
            stock_y: job.stock.y,
            stock_z: job.stock.z,
            stock_origin_x: job.stock.origin_x,
            stock_origin_y: job.stock.origin_y,
            stock_origin_z: job.stock.origin_z,
            material: job.stock.material.to_key(),
            machine: job.machine.to_key(),
        },
        tools,
        toolpaths,
    };

    let toml_str = toml::to_string_pretty(&project).map_err(|e| format!("Serialize error: {e}"))?;
    std::fs::write(path, toml_str).map_err(|e| format!("Write error: {e}"))?;
    Ok(())
}

/// Load a job from a TOML file (restores tools, stock, post config).
pub fn load_project(path: &Path) -> Result<(JobState, Vec<String>), String> {
    let content = std::fs::read_to_string(path).map_err(|e| format!("Read error: {e}"))?;
    let project: ProjectFile = toml::from_str(&content).map_err(|e| format!("Parse error: {e}"))?;

    let mut job = JobState::new();
    job.name = project.job.name;
    job.stock.x = project.job.stock_x;
    job.stock.y = project.job.stock_y;
    job.stock.z = project.job.stock_z;
    job.stock.origin_x = project.job.stock_origin_x;
    job.stock.origin_y = project.job.stock_origin_y;
    job.stock.origin_z = project.job.stock_origin_z;
    job.stock.auto_from_model = false;

    if !project.job.material.is_empty() {
        job.stock.material = rs_cam_core::material::Material::from_key(&project.job.material);
    }
    if !project.job.machine.is_empty() {
        job.machine = rs_cam_core::machine::MachineProfile::from_key(&project.job.machine);
    }

    job.post.spindle_speed = project.job.spindle_speed;
    job.post.safe_z = project.job.safe_z;
    job.post.format = match project.job.post.as_str() {
        "linuxcnc" => PostFormat::LinuxCnc,
        "mach3" => PostFormat::Mach3,
        _ => PostFormat::Grbl,
    };

    // Restore tools
    for ts in &project.tools {
        let id = job.next_tool_id();
        let tool_type = match ts.tool_type.as_str() {
            "ball" => ToolType::BallNose,
            "bullnose" => ToolType::BullNose,
            "vbit" => ToolType::VBit,
            "tapered_ball" => ToolType::TaperedBallNose,
            _ => ToolType::EndMill,
        };
        let mut tool = ToolConfig::new_default(id, tool_type);
        tool.name = ts.name.clone();
        tool.diameter = ts.diameter;
        tool.cutting_length = ts.cutting_length;
        tool.corner_radius = ts.corner_radius;
        tool.included_angle = ts.included_angle;
        tool.taper_half_angle = ts.taper_half_angle;
        tool.shaft_diameter = ts.shaft_diameter;
        tool.flute_count = ts.flute_count;
        tool.tool_material = match ts.tool_material.as_str() {
            "hss" => ToolMaterial::Hss,
            _ => ToolMaterial::Carbide,
        };
        tool.cut_direction = match ts.cut_direction.as_str() {
            "downcut" => CutDirection::DownCut,
            "compression" => CutDirection::Compression,
            _ => CutDirection::UpCut,
        };
        job.tools.push(tool);
    }

    // Collect input file paths that need importing
    let inputs: Vec<String> = project.toolpaths.iter()
        .map(|tp| tp.input.clone())
        .filter(|s| !s.is_empty())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    job.file_path = Some(path.to_path_buf());
    Ok((job, inputs))
}
