//! TOML project file types and loading/saving helpers.

use std::path::Path;
use std::sync::Arc;

use serde::Deserialize;

use crate::compute::catalog::OperationConfig;
use crate::compute::config::{DressupConfig, HeightsConfig};
use crate::compute::stock_config::{ModelKind, ModelUnits, StockConfig};
use crate::compute::tool_config::{ToolConfig, ToolId, ToolType};
use crate::compute::transform::FaceUp;
use crate::mesh::TriangleMesh;
use super::{LoadedGeometry, LoadedModel, SessionError, SetupData, ToolpathConfig};

// ── Project file types (TOML deserialization) ──────────────────────────

/// Top-level project file structure (format_version=3).
#[derive(Debug, Clone, Deserialize)]
pub struct ProjectFile {
    #[serde(default = "default_format_version")]
    pub format_version: u32,
    #[serde(default)]
    pub job: ProjectJobSection,
    #[serde(default)]
    pub tools: Vec<ProjectToolSection>,
    #[serde(default)]
    pub models: Vec<ProjectModelSection>,
    #[serde(default)]
    pub setups: Vec<ProjectSetupSection>,
    /// Legacy: top-level toolpaths (pre-setup format).
    #[serde(default)]
    pub toolpaths: Vec<ProjectToolpathSection>,
}

fn default_format_version() -> u32 {
    1
}

/// Job-level settings (name, stock, post).
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ProjectJobSection {
    #[serde(default = "default_job_name")]
    pub name: String,
    #[serde(default)]
    pub stock: ProjectStockConfig,
    #[serde(default)]
    pub post: ProjectPostConfig,
    #[serde(default)]
    pub machine: crate::machine::MachineProfile,
}

fn default_job_name() -> String {
    "Untitled".to_owned()
}

/// Stock dimensions as saved in the project file.
#[derive(Debug, Clone, Deserialize)]
pub struct ProjectStockConfig {
    #[serde(default = "default_stock_dim")]
    pub x: f64,
    #[serde(default = "default_stock_dim")]
    pub y: f64,
    #[serde(default = "default_stock_z")]
    pub z: f64,
    #[serde(default)]
    pub origin_x: f64,
    #[serde(default)]
    pub origin_y: f64,
    #[serde(default)]
    pub origin_z: f64,
    #[serde(default)]
    pub material: crate::material::Material,
    #[serde(default)]
    pub alignment_pins: Vec<crate::compute::stock_config::AlignmentPin>,
    #[serde(default)]
    pub flip_axis: Option<crate::compute::stock_config::FlipAxis>,
}

impl Default for ProjectStockConfig {
    fn default() -> Self {
        Self {
            x: 100.0,
            y: 100.0,
            z: 25.0,
            origin_x: 0.0,
            origin_y: 0.0,
            origin_z: 0.0,
            material: crate::material::Material::default(),
            alignment_pins: Vec::new(),
            flip_axis: None,
        }
    }
}

fn default_stock_dim() -> f64 {
    100.0
}
fn default_stock_z() -> f64 {
    25.0
}

/// Post-processor configuration from the project file.
#[derive(Debug, Clone, Deserialize)]
pub struct ProjectPostConfig {
    #[serde(default)]
    pub format: String,
    #[serde(default = "default_spindle_speed")]
    pub spindle_speed: u32,
    #[serde(default = "default_safe_z")]
    pub safe_z: f64,
    #[serde(default)]
    pub high_feedrate_mode: bool,
    #[serde(default = "default_high_feedrate")]
    pub high_feedrate: f64,
}

impl Default for ProjectPostConfig {
    fn default() -> Self {
        Self {
            format: "grbl".to_owned(),
            spindle_speed: 18000,
            safe_z: 10.0,
            high_feedrate_mode: false,
            high_feedrate: 5000.0,
        }
    }
}

fn default_spindle_speed() -> u32 {
    18000
}
fn default_safe_z() -> f64 {
    10.0
}
fn default_high_feedrate() -> f64 {
    5000.0
}

/// Tool definition in the project file.
#[derive(Debug, Clone, Deserialize)]
pub struct ProjectToolSection {
    #[serde(default)]
    pub id: Option<usize>,
    #[serde(default = "default_tool_name")]
    pub name: String,
    #[serde(rename = "type", default = "default_tool_type_str")]
    pub tool_type: String,
    #[serde(default = "default_tool_diameter")]
    pub diameter: f64,
    #[serde(default = "default_cutting_length")]
    pub cutting_length: f64,
    #[serde(default = "default_corner_radius")]
    pub corner_radius: f64,
    #[serde(default = "default_included_angle")]
    pub included_angle: f64,
    #[serde(default = "default_taper_half_angle")]
    pub taper_half_angle: f64,
    #[serde(default = "default_shaft_diameter")]
    pub shaft_diameter: f64,
    #[serde(default = "default_holder_diameter")]
    pub holder_diameter: f64,
    #[serde(default = "default_shank_diameter")]
    pub shank_diameter: f64,
    #[serde(default = "default_shank_length")]
    pub shank_length: f64,
    #[serde(default = "default_stickout")]
    pub stickout: f64,
    #[serde(default = "default_flute_count")]
    pub flute_count: u32,
}

fn default_tool_name() -> String {
    "Tool".to_owned()
}
fn default_tool_type_str() -> String {
    "end_mill".to_owned()
}
fn default_tool_diameter() -> f64 {
    6.35
}
fn default_cutting_length() -> f64 {
    25.0
}
fn default_corner_radius() -> f64 {
    2.0
}
fn default_included_angle() -> f64 {
    90.0
}
fn default_taper_half_angle() -> f64 {
    15.0
}
fn default_shaft_diameter() -> f64 {
    6.35
}
fn default_holder_diameter() -> f64 {
    25.0
}
fn default_shank_diameter() -> f64 {
    6.35
}
fn default_shank_length() -> f64 {
    20.0
}
fn default_stickout() -> f64 {
    45.0
}
fn default_flute_count() -> u32 {
    2
}

/// Model reference in the project file.
#[derive(Debug, Clone, Deserialize)]
pub struct ProjectModelSection {
    #[serde(default)]
    pub id: Option<usize>,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub kind: Option<ModelKind>,
    #[serde(default)]
    pub units: Option<ModelUnits>,
}

/// Setup section in the project file.
#[derive(Debug, Clone, Deserialize)]
pub struct ProjectSetupSection {
    #[serde(default)]
    pub id: Option<usize>,
    #[serde(default = "default_setup_name")]
    pub name: String,
    #[serde(default = "default_face_up")]
    pub face_up: String,
    #[serde(default)]
    pub toolpaths: Vec<ProjectToolpathSection>,
}

fn default_setup_name() -> String {
    "Setup 1".to_owned()
}
fn default_face_up() -> String {
    "top".to_owned()
}

/// Toolpath section in the project file.
#[derive(Debug, Clone, Deserialize)]
pub struct ProjectToolpathSection {
    #[serde(default)]
    pub id: Option<usize>,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub operation: Option<OperationConfig>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub tool_id: Option<usize>,
    #[serde(default)]
    pub model_id: Option<usize>,
    #[serde(default)]
    pub dressups: DressupConfig,
    #[serde(default)]
    pub heights: HeightsConfig,
    /// Raw G-code to emit before this toolpath's moves.
    #[serde(default)]
    pub pre_gcode: Option<String>,
    /// Raw G-code to emit after this toolpath's moves.
    #[serde(default)]
    pub post_gcode: Option<String>,
}

fn default_true() -> bool {
    true
}

// ── Free functions: project file to session state ─────────────────────

pub(crate) fn stock_from_project(ps: &ProjectStockConfig) -> StockConfig {
    StockConfig {
        x: ps.x,
        y: ps.y,
        z: ps.z,
        origin_x: ps.origin_x,
        origin_y: ps.origin_y,
        origin_z: ps.origin_z,
        auto_from_model: false,
        padding: 5.0,
        material: ps.material.clone(),
        alignment_pins: ps.alignment_pins.clone(),
        flip_axis: ps.flip_axis,
        workholding_rigidity: crate::feeds::WorkholdingRigidity::Medium,
    }
}

pub(crate) fn parse_tool_type(s: &str) -> ToolType {
    match s.to_ascii_lowercase().as_str() {
        "ball_nose" | "ballnose" => ToolType::BallNose,
        "bull_nose" | "bullnose" => ToolType::BullNose,
        "v_bit" | "vbit" => ToolType::VBit,
        "tapered_ball_nose" | "taperedballnose" => ToolType::TaperedBallNose,
        _ => ToolType::EndMill,
    }
}

pub(crate) fn tool_from_project_section(ts: &ProjectToolSection, idx: usize) -> ToolConfig {
    use crate::compute::tool_config::{BitCutDirection, ToolMaterial};
    ToolConfig {
        id: ToolId(ts.id.unwrap_or(idx)),
        name: ts.name.clone(),
        tool_number: (ts.id.unwrap_or(idx) + 1) as u32,
        tool_type: parse_tool_type(&ts.tool_type),
        diameter: ts.diameter,
        cutting_length: ts.cutting_length,
        corner_radius: ts.corner_radius,
        included_angle: ts.included_angle,
        taper_half_angle: ts.taper_half_angle,
        shaft_diameter: ts.shaft_diameter,
        holder_diameter: ts.holder_diameter,
        shank_diameter: ts.shank_diameter,
        shank_length: ts.shank_length,
        stickout: ts.stickout,
        flute_count: ts.flute_count,
        tool_material: ToolMaterial::Carbide,
        cut_direction: BitCutDirection::UpCut,
        vendor: String::new(),
        product_id: String::new(),
    }
}

fn infer_model_kind(path: &Path) -> Option<ModelKind> {
    path.extension()
        .and_then(|ext| ext.to_str())
        .and_then(|ext| match ext.to_ascii_lowercase().as_str() {
            "stl" => Some(ModelKind::Stl),
            "svg" => Some(ModelKind::Svg),
            "dxf" => Some(ModelKind::Dxf),
            "step" | "stp" => Some(ModelKind::Step),
            _ => None,
        })
}

pub(crate) fn load_model_geometry(
    model: &ProjectModelSection,
    base_dir: &Path,
) -> Result<LoadedGeometry, SessionError> {
    let raw_path = Path::new(&model.path);
    let full_path = if raw_path.is_absolute() {
        raw_path.to_path_buf()
    } else {
        base_dir.join(raw_path)
    };

    let kind = model
        .kind
        .or_else(|| infer_model_kind(&full_path))
        .ok_or_else(|| SessionError::ModelLoad {
            name: model.name.clone(),
            detail: format!("Cannot determine file type for '{}'", full_path.display()),
        })?;

    let scale = model
        .units
        .as_ref()
        .map(|u| u.scale_factor())
        .unwrap_or(1.0);

    match kind {
        ModelKind::Stl => {
            let mesh = TriangleMesh::from_stl_scaled(&full_path, scale).map_err(|e| {
                SessionError::ModelLoad {
                    name: model.name.clone(),
                    detail: format!("STL load failed: {e}"),
                }
            })?;
            Ok(LoadedGeometry::Mesh(mesh))
        }
        ModelKind::Dxf => {
            let polys = crate::dxf_input::load_dxf(&full_path, 5.0).map_err(|e| {
                SessionError::ModelLoad {
                    name: model.name.clone(),
                    detail: format!("DXF load failed: {e}"),
                }
            })?;
            Ok(LoadedGeometry::Polygons(polys))
        }
        ModelKind::Svg => {
            let polys = crate::svg_input::load_svg(&full_path, 0.1).map_err(|e| {
                SessionError::ModelLoad {
                    name: model.name.clone(),
                    detail: format!("SVG load failed: {e}"),
                }
            })?;
            Ok(LoadedGeometry::Polygons(polys))
        }
        ModelKind::Step => {
            #[cfg(feature = "step")]
            {
                let enriched = crate::step_input::load_step(&full_path, 0.1).map_err(|e| {
                    SessionError::ModelLoad {
                        name: model.name.clone(),
                        detail: format!("STEP load failed: {e}"),
                    }
                })?;
                Ok(LoadedGeometry::Mesh((*enriched.mesh).clone()))
            }
            #[cfg(not(feature = "step"))]
            {
                Err(SessionError::ModelLoad {
                    name: model.name.clone(),
                    detail: "STEP support not enabled (compile with --features step)".to_owned(),
                })
            }
        }
    }
}

/// Build a [`ProjectSession`](super::ProjectSession) from a parsed [`ProjectFile`].
///
/// This is the core of `ProjectSession::from_project_file` but lives here so
/// that all TOML-to-session conversion logic is co-located.
pub(super) fn build_session_from_project(
    project: ProjectFile,
    base_dir: &Path,
) -> Result<super::ProjectSession, SessionError> {
    let stock = stock_from_project(&project.job.stock);

    // Load tools
    let tools: Vec<ToolConfig> = project
        .tools
        .iter()
        .enumerate()
        .map(|(idx, ts)| tool_from_project_section(ts, idx))
        .collect();

    // Load models
    let mut models = Vec::new();
    for (idx, model_section) in project.models.iter().enumerate() {
        let model_id = model_section.id.unwrap_or(idx);
        match load_model_geometry(model_section, base_dir) {
            Ok(LoadedGeometry::Mesh(mesh)) => {
                tracing::info!(
                    name = %model_section.name,
                    tris = mesh.triangles.len(),
                    "Loaded mesh model"
                );
                models.push(LoadedModel {
                    id: model_id,
                    name: model_section.name.clone(),
                    mesh: Some(Arc::new(mesh)),
                    polygons: None,
                });
            }
            Ok(LoadedGeometry::Polygons(polys)) => {
                tracing::info!(
                    name = %model_section.name,
                    polygons = polys.len(),
                    "Loaded 2D model"
                );
                models.push(LoadedModel {
                    id: model_id,
                    name: model_section.name.clone(),
                    mesh: None,
                    polygons: Some(Arc::new(polys)),
                });
            }
            Err(e) => {
                tracing::warn!(
                    name = %model_section.name,
                    error = %e,
                    "Failed to load model, skipping"
                );
                models.push(LoadedModel {
                    id: model_id,
                    name: model_section.name.clone(),
                    mesh: None,
                    polygons: None,
                });
            }
        }
    }

    // Collect toolpaths from setups
    let mut setups = Vec::new();
    let mut toolpath_configs = Vec::new();

    if !project.setups.is_empty() {
        for (setup_idx, setup_section) in project.setups.iter().enumerate() {
            let setup_id = setup_section.id.unwrap_or(setup_idx);
            let face_up = FaceUp::from_key(&setup_section.face_up);
            let mut tp_indices = Vec::new();

            for tp_section in &setup_section.toolpaths {
                let tp_idx = toolpath_configs.len();
                let tp_id = tp_section.id.unwrap_or(tp_idx);
                if let Some(operation) = &tp_section.operation {
                    toolpath_configs.push(ToolpathConfig {
                        id: tp_id,
                        name: tp_section.name.clone(),
                        enabled: tp_section.enabled,
                        operation: operation.clone(),
                        dressups: tp_section.dressups.clone(),
                        heights: tp_section.heights.clone(),
                        tool_id: tp_section.tool_id.unwrap_or(0),
                        model_id: tp_section.model_id.unwrap_or(0),
                        pre_gcode: tp_section.pre_gcode.clone(),
                        post_gcode: tp_section.post_gcode.clone(),
                    });
                    tp_indices.push(tp_idx);
                }
            }

            setups.push(SetupData {
                id: setup_id,
                name: setup_section.name.clone(),
                face_up,
                toolpath_indices: tp_indices,
            });
        }
    } else {
        // Legacy: top-level toolpaths -> single default setup
        let mut tp_indices = Vec::new();
        for tp_section in &project.toolpaths {
            let tp_idx = toolpath_configs.len();
            let tp_id = tp_section.id.unwrap_or(tp_idx);
            if let Some(operation) = &tp_section.operation {
                toolpath_configs.push(ToolpathConfig {
                    id: tp_id,
                    name: tp_section.name.clone(),
                    enabled: tp_section.enabled,
                    operation: operation.clone(),
                    dressups: tp_section.dressups.clone(),
                    heights: tp_section.heights.clone(),
                    tool_id: tp_section.tool_id.unwrap_or(0),
                    model_id: tp_section.model_id.unwrap_or(0),
                    pre_gcode: tp_section.pre_gcode.clone(),
                    post_gcode: tp_section.post_gcode.clone(),
                });
                tp_indices.push(tp_idx);
            }
        }
        if !tp_indices.is_empty() {
            setups.push(SetupData {
                id: 0,
                name: "Default".to_owned(),
                face_up: FaceUp::Top,
                toolpath_indices: tp_indices,
            });
        }
    }

    Ok(super::ProjectSession {
        name: project.job.name.clone(),
        stock,
        post: project.job.post,
        machine: project.job.machine,
        models,
        tools,
        setups,
        toolpath_configs,
        results: std::collections::HashMap::new(),
        simulation: None,
    })
}
