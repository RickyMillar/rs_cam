use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::io::import;
use crate::state::job::{
    AlignmentPin, BitCutDirection as ToolCutDirection, FaceUp, Fixture, FixtureId, FixtureKind,
    JobState, KeepOutId, KeepOutZone, LoadedModel, ModelId, ModelKind, ModelUnits, PostConfig,
    PostFormat, Setup, SetupId, StockConfig, ToolConfig, ToolId, ToolMaterial, ToolType, XYDatum,
    ZDatum, ZRotation,
};
use crate::state::toolpath::{
    BoundaryContainment, DressupConfig, FeedsAutoMode, HeightsConfig, OperationConfig,
    OperationType, StockSource, ToolpathEntry, ToolpathEntryInit, ToolpathId,
};
use rs_cam_core::gcode::CoolantMode;

const PROJECT_FORMAT_VERSION: u32 = 3;

pub struct LoadedProject {
    pub job: JobState,
    pub warnings: Vec<ProjectLoadWarning>,
}

#[derive(Debug, Clone)]
pub enum ProjectLoadWarning {
    MissingModelFile {
        name: String,
        path: PathBuf,
    },
    ModelImportFailed {
        name: String,
        path: PathBuf,
        error: String,
    },
    MissingModelPath {
        name: String,
        model_id: ModelId,
    },
    MissingToolReference {
        toolpath: String,
        tool_id: ToolId,
    },
    MissingModelReference {
        toolpath: String,
        model_id: ModelId,
    },
    FaceSelectionStale {
        toolpath: String,
        face_count: usize,
        invalid_id: u16,
    },
}

impl ProjectLoadWarning {
    pub fn message(&self) -> String {
        match self {
            ProjectLoadWarning::MissingModelFile { name, path } => {
                format!(
                    "Model '{name}' could not be loaded because '{}' was not found.",
                    path.display()
                )
            }
            ProjectLoadWarning::ModelImportFailed { name, path, error } => {
                format!(
                    "Model '{name}' failed to import from '{}': {error}",
                    path.display()
                )
            }
            ProjectLoadWarning::MissingModelPath { name, model_id } => {
                format!(
                    "Model '{}' (id {}) has no saved path, so its geometry could not be restored.",
                    name, model_id.0
                )
            }
            ProjectLoadWarning::MissingToolReference { toolpath, tool_id } => {
                format!(
                    "Toolpath '{toolpath}' references missing tool id {} and needs reassignment.",
                    tool_id.0
                )
            }
            ProjectLoadWarning::MissingModelReference { toolpath, model_id } => {
                format!(
                    "Toolpath '{toolpath}' references missing model id {} and needs reassignment.",
                    model_id.0
                )
            }
            ProjectLoadWarning::FaceSelectionStale {
                toolpath,
                face_count,
                invalid_id,
            } => {
                format!(
                    "Toolpath '{toolpath}' had a stale face selection (face {invalid_id} not in \
                     model with {face_count} faces). Selection was cleared."
                )
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectFile {
    #[serde(default = "default_project_format_version")]
    pub format_version: u32,
    #[serde(default)]
    pub job: ProjectJobSection,
    #[serde(default)]
    pub tools: Vec<ProjectToolSection>,
    #[serde(default)]
    pub models: Vec<ProjectModelSection>,
    #[serde(default)]
    pub setups: Vec<ProjectSetupSection>,
    #[serde(default)]
    pub toolpaths: Vec<ProjectToolpathSection>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProjectJobSection {
    #[serde(default = "default_job_name")]
    pub name: String,
    #[serde(default)]
    pub stock: StockConfig,
    #[serde(default)]
    pub post: PostConfig,
    #[serde(default)]
    pub machine: rs_cam_core::machine::MachineProfile,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectToolSection {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<ToolId>,
    #[serde(default = "default_tool_name")]
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_number: Option<u32>,
    #[serde(rename = "type", default = "default_tool_type")]
    pub tool_type: ToolType,
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
    #[serde(default = "default_tool_material")]
    pub tool_material: ToolMaterial,
    #[serde(default = "default_tool_cut_direction")]
    pub cut_direction: ToolCutDirection,
    #[serde(default)]
    pub vendor: String,
    #[serde(default)]
    pub product_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectModelSection {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<ModelId>,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<ModelKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub units: Option<ModelUnits>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectToolpathSection {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<ToolpathId>,
    #[serde(default)]
    pub name: String,
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub op_type: Option<OperationType>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation: Option<OperationConfig>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_true")]
    pub visible: bool,
    #[serde(default)]
    pub locked: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_id: Option<ToolId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_id: Option<ModelId>,
    #[serde(default)]
    pub dressups: DressupConfig,
    #[serde(default)]
    pub heights: HeightsConfig,
    #[serde(default)]
    pub boundary_enabled: bool,
    #[serde(default)]
    pub boundary_containment: BoundaryContainment,
    #[serde(default)]
    pub coolant: CoolantMode,
    #[serde(default)]
    pub pre_gcode: String,
    #[serde(default)]
    pub post_gcode: String,
    #[serde(default)]
    pub stock_source: StockSource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_regen: Option<bool>,
    #[serde(default)]
    pub feeds_auto: FeedsAutoMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub face_selection: Option<Vec<u16>>,
    #[serde(default)]
    pub debug_options: rs_cam_core::debug_trace::ToolpathDebugOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSetupSection {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<SetupId>,
    #[serde(default = "default_setup_name")]
    pub name: String,
    #[serde(default = "default_setup_face_up")]
    pub face_up: String,
    #[serde(default = "default_setup_z_rotation")]
    pub z_rotation: String,
    #[serde(default)]
    pub xy_datum: String,
    #[serde(default)]
    pub z_datum: String,
    #[serde(default)]
    pub datum_notes: String,
    #[serde(default)]
    pub alignment_pins: Vec<ProjectPinSection>,
    #[serde(default)]
    pub fixtures: Vec<ProjectFixtureSection>,
    #[serde(default)]
    pub keep_out_zones: Vec<ProjectKeepOutSection>,
    #[serde(default)]
    pub toolpaths: Vec<ProjectToolpathSection>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectPinSection {
    pub x: f64,
    pub y: f64,
    #[serde(default = "default_pin_diameter")]
    pub diameter: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectFixtureSection {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<FixtureId>,
    #[serde(default = "default_fixture_name")]
    pub name: String,
    #[serde(default = "default_fixture_kind")]
    pub kind: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub origin_x: f64,
    #[serde(default)]
    pub origin_y: f64,
    #[serde(default)]
    pub origin_z: f64,
    #[serde(default = "default_fixture_size_x")]
    pub size_x: f64,
    #[serde(default = "default_fixture_size_y")]
    pub size_y: f64,
    #[serde(default = "default_fixture_size_z")]
    pub size_z: f64,
    #[serde(default = "default_fixture_clearance")]
    pub clearance: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectKeepOutSection {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<KeepOutId>,
    #[serde(default = "default_keep_out_name")]
    pub name: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub origin_x: f64,
    #[serde(default)]
    pub origin_y: f64,
    #[serde(default = "default_keep_out_size")]
    pub size_x: f64,
    #[serde(default = "default_keep_out_size")]
    pub size_y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LegacyProjectFile {
    pub job: LegacyJobSection,
    #[serde(default)]
    pub tools: Vec<LegacyToolSection>,
    #[serde(default)]
    pub toolpaths: Vec<LegacyToolpathSection>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LegacyJobSection {
    pub name: String,
    #[serde(default)]
    pub post: String,
    #[serde(default = "default_legacy_spindle")]
    pub spindle_speed: u32,
    #[serde(default = "default_legacy_safe_z")]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LegacyToolSection {
    pub name: String,
    #[serde(rename = "type")]
    pub tool_type: String,
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
    #[serde(default = "default_flute_count")]
    pub flute_count: u32,
    #[serde(default)]
    pub tool_material: String,
    #[serde(default)]
    pub cut_direction: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LegacyToolpathSection {
    pub name: String,
    #[serde(rename = "type")]
    pub op_type: String,
    pub tool_index: usize,
    #[serde(default)]
    pub input: String,
    #[serde(flatten)]
    pub params: toml::Value,
}

pub fn save_project(job: &JobState, path: &Path) -> Result<(), crate::error::VizError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("Create directory error: {e}"))?;
    }

    let project = ProjectFile {
        format_version: PROJECT_FORMAT_VERSION,
        job: ProjectJobSection {
            name: job.name.clone(),
            stock: job.stock.clone(),
            post: job.post.clone(),
            machine: job.machine.clone(),
        },
        tools: job
            .tools
            .iter()
            .map(ProjectToolSection::from_runtime)
            .collect(),
        models: job
            .models
            .iter()
            .map(|model| ProjectModelSection::from_runtime(model, path))
            .collect(),
        setups: job
            .setups
            .iter()
            .map(ProjectSetupSection::from_runtime)
            .collect(),
        toolpaths: Vec::new(),
    };

    let toml_str = toml::to_string_pretty(&project).map_err(|e| format!("Serialize error: {e}"))?;

    // Atomic write: write to a temporary file in the same directory, then rename.
    // This prevents corruption if a crash occurs during the write.
    let tmp_path = path.with_extension("tmp");
    std::fs::write(&tmp_path, &toml_str).map_err(|e| format!("Write temp file error: {e}"))?;
    std::fs::rename(&tmp_path, path).map_err(|e| format!("Rename error: {e}"))?;
    Ok(())
}

pub fn load_project(path: &Path) -> Result<LoadedProject, crate::error::VizError> {
    let content = std::fs::read_to_string(path).map_err(|e| format!("Read error: {e}"))?;

    match toml::from_str::<ProjectFile>(&content) {
        Ok(project) => load_typed_project(path, project),
        Err(current_err) => match toml::from_str::<LegacyProjectFile>(&content) {
            Ok(legacy) => load_legacy_project(path, legacy),
            Err(legacy_err) => Err(crate::error::VizError::ProjectLoad(format!(
                "Parse error: {current_err}; legacy parse error: {legacy_err}"
            ))),
        },
    }
}

impl ProjectToolSection {
    fn from_runtime(tool: &ToolConfig) -> Self {
        Self {
            id: Some(tool.id),
            tool_number: Some(tool.tool_number),
            name: tool.name.clone(),
            tool_type: tool.tool_type,
            diameter: tool.diameter,
            cutting_length: tool.cutting_length,
            corner_radius: tool.corner_radius,
            included_angle: tool.included_angle,
            taper_half_angle: tool.taper_half_angle,
            shaft_diameter: tool.shaft_diameter,
            holder_diameter: tool.holder_diameter,
            shank_diameter: tool.shank_diameter,
            shank_length: tool.shank_length,
            stickout: tool.stickout,
            flute_count: tool.flute_count,
            tool_material: tool.tool_material,
            cut_direction: tool.cut_direction,
            vendor: tool.vendor.clone(),
            product_id: tool.product_id.clone(),
        }
    }

    fn into_runtime(self, id: ToolId) -> ToolConfig {
        ToolConfig {
            id,
            name: self.name,
            tool_number: self.tool_number.unwrap_or(id.0 as u32 + 1),
            tool_type: self.tool_type,
            diameter: self.diameter,
            cutting_length: self.cutting_length,
            corner_radius: self.corner_radius,
            included_angle: self.included_angle,
            taper_half_angle: self.taper_half_angle,
            shaft_diameter: self.shaft_diameter,
            holder_diameter: self.holder_diameter,
            shank_diameter: self.shank_diameter,
            shank_length: self.shank_length,
            stickout: self.stickout,
            flute_count: self.flute_count,
            tool_material: self.tool_material,
            cut_direction: self.cut_direction,
            vendor: self.vendor,
            product_id: self.product_id,
        }
    }
}

impl ProjectModelSection {
    fn from_runtime(model: &LoadedModel, project_path: &Path) -> Self {
        Self {
            id: Some(model.id),
            path: persist_model_path(project_path, &model.path),
            name: model.name.clone(),
            kind: Some(model.kind),
            units: Some(model.units),
        }
    }
}

impl ProjectToolpathSection {
    fn from_runtime(toolpath: &ToolpathEntry) -> Self {
        Self {
            id: Some(toolpath.id),
            name: toolpath.name.clone(),
            op_type: Some(toolpath.operation.op_type()),
            operation: Some(toolpath.operation.clone()),
            enabled: toolpath.enabled,
            visible: toolpath.visible,
            locked: toolpath.locked,
            tool_id: Some(toolpath.tool_id),
            model_id: Some(toolpath.model_id),
            dressups: toolpath.dressups.clone(),
            heights: toolpath.heights.clone(),
            boundary_enabled: toolpath.boundary_enabled,
            boundary_containment: toolpath.boundary_containment,
            coolant: toolpath.coolant,
            pre_gcode: toolpath.pre_gcode.clone(),
            post_gcode: toolpath.post_gcode.clone(),
            stock_source: toolpath.stock_source,
            auto_regen: Some(toolpath.auto_regen),
            feeds_auto: toolpath.feeds_auto.clone(),
            face_selection: toolpath
                .face_selection
                .as_ref()
                .map(|faces| faces.iter().map(|f| f.0).collect()),
            debug_options: toolpath.debug_options,
        }
    }
}

impl ProjectSetupSection {
    fn from_runtime(setup: &Setup) -> Self {
        Self {
            id: Some(setup.id),
            name: setup.name.clone(),
            face_up: setup.face_up.to_key().to_string(),
            z_rotation: setup.z_rotation.to_key().to_string(),
            xy_datum: setup.datum.xy_method.to_key(),
            z_datum: setup.datum.z_method.to_key(),
            datum_notes: setup.datum.notes.clone(),
            alignment_pins: Vec::new(), // pins now live on StockConfig
            fixtures: setup
                .fixtures
                .iter()
                .map(ProjectFixtureSection::from_runtime)
                .collect(),
            keep_out_zones: setup
                .keep_out_zones
                .iter()
                .map(ProjectKeepOutSection::from_runtime)
                .collect(),
            toolpaths: setup
                .toolpaths
                .iter()
                .map(ProjectToolpathSection::from_runtime)
                .collect(),
        }
    }
}

impl ProjectFixtureSection {
    fn from_runtime(fixture: &Fixture) -> Self {
        Self {
            id: Some(fixture.id),
            name: fixture.name.clone(),
            kind: match fixture.kind {
                FixtureKind::Clamp => "clamp".to_string(),
                FixtureKind::Vise => "vise".to_string(),
                FixtureKind::VacuumPod => "vacuum_pod".to_string(),
                FixtureKind::Custom => "custom".to_string(),
            },
            enabled: fixture.enabled,
            origin_x: fixture.origin_x,
            origin_y: fixture.origin_y,
            origin_z: fixture.origin_z,
            size_x: fixture.size_x,
            size_y: fixture.size_y,
            size_z: fixture.size_z,
            clearance: fixture.clearance,
        }
    }
}

impl ProjectKeepOutSection {
    fn from_runtime(keep_out: &KeepOutZone) -> Self {
        Self {
            id: Some(keep_out.id),
            name: keep_out.name.clone(),
            enabled: keep_out.enabled,
            origin_x: keep_out.origin_x,
            origin_y: keep_out.origin_y,
            size_x: keep_out.size_x,
            size_y: keep_out.size_y,
        }
    }
}

fn load_typed_project(
    path: &Path,
    project: ProjectFile,
) -> Result<LoadedProject, crate::error::VizError> {
    let mut job = JobState::new();
    let mut warnings = Vec::new();

    job.name = project.job.name;
    job.stock = project.job.stock;
    job.post = project.job.post;
    job.machine = project.job.machine;
    job.file_path = Some(path.to_path_buf());
    job.dirty = false;

    let mut used_tool_ids = BTreeSet::new();
    let mut next_tool_id = 0usize;
    for tool in project.tools {
        let id = ToolId(assign_unique_id(
            tool.id.map(|id| id.0),
            &mut used_tool_ids,
            &mut next_tool_id,
        ));
        job.tools.push(tool.into_runtime(id));
    }

    let mut used_model_ids = BTreeSet::new();
    let mut next_model_id = 0usize;
    for model in project.models {
        let id = ModelId(assign_unique_id(
            model.id.map(|id| id.0),
            &mut used_model_ids,
            &mut next_model_id,
        ));
        job.models
            .push(load_model_section(path, id, model, &mut warnings));
    }

    let loaded_at = Instant::now();
    let mut used_setup_ids = BTreeSet::new();
    let mut next_setup_id = 0usize;
    let mut used_fixture_ids = BTreeSet::new();
    let mut next_fixture_id = 0usize;
    let mut used_keep_out_ids = BTreeSet::new();
    let mut next_keep_out_id = 0usize;
    let mut used_toolpath_ids = BTreeSet::new();
    let mut next_toolpath_id = 0usize;

    // Migrate old-format alignment pins from setup sections to stock.
    // In format_version <= 2, pins lived on each setup.  Collect them
    // before consuming the setup sections.
    if job.stock.alignment_pins.is_empty() {
        let mut migrated: Vec<AlignmentPin> = Vec::new();
        for section in &project.setups {
            for pin in &section.alignment_pins {
                let dup = migrated
                    .iter()
                    .any(|p| (p.x - pin.x).abs() < 0.01 && (p.y - pin.y).abs() < 0.01);
                if !dup {
                    migrated.push(AlignmentPin::new(pin.x, pin.y, pin.diameter));
                }
            }
        }
        if !migrated.is_empty() {
            tracing::info!(
                "Migrated {} alignment pin(s) from setup sections to stock",
                migrated.len()
            );
            job.stock.alignment_pins = migrated;
        }
    }

    if !project.setups.is_empty() {
        job.setups.clear();
        for setup_section in project.setups {
            let setup_id = SetupId(assign_unique_id(
                setup_section.id.map(|id| id.0),
                &mut used_setup_ids,
                &mut next_setup_id,
            ));
            let (mut setup, toolpath_sections) = restore_project_setup(
                setup_section,
                setup_id,
                &mut used_fixture_ids,
                &mut next_fixture_id,
                &mut used_keep_out_ids,
                &mut next_keep_out_id,
            );
            for section in toolpath_sections {
                let id = ToolpathId(assign_unique_id(
                    section.id.map(|id| id.0),
                    &mut used_toolpath_ids,
                    &mut next_toolpath_id,
                ));
                let toolpath = restore_project_toolpath(
                    section,
                    id,
                    &job.tools,
                    &job.models,
                    loaded_at,
                    &mut warnings,
                );
                setup.toolpaths.push(toolpath);
            }
            job.setups.push(setup);
        }
    } else {
        for section in project.toolpaths {
            let id = ToolpathId(assign_unique_id(
                section.id.map(|id| id.0),
                &mut used_toolpath_ids,
                &mut next_toolpath_id,
            ));
            let toolpath = restore_project_toolpath(
                section,
                id,
                &job.tools,
                &job.models,
                loaded_at,
                &mut warnings,
            );
            job.push_toolpath(toolpath);
        }
    }

    job.sync_next_ids();
    Ok(LoadedProject { job, warnings })
}

fn load_legacy_project(
    path: &Path,
    legacy: LegacyProjectFile,
) -> Result<LoadedProject, crate::error::VizError> {
    let mut job = JobState::new();
    let mut warnings = Vec::new();

    job.name = legacy.job.name;
    job.stock.x = legacy.job.stock_x;
    job.stock.y = legacy.job.stock_y;
    job.stock.z = legacy.job.stock_z;
    job.stock.origin_x = legacy.job.stock_origin_x;
    job.stock.origin_y = legacy.job.stock_origin_y;
    job.stock.origin_z = legacy.job.stock_origin_z;
    job.stock.auto_from_model = false;
    if !legacy.job.material.is_empty() {
        job.stock.material = rs_cam_core::material::Material::from_key(&legacy.job.material);
    }
    if !legacy.job.machine.is_empty() {
        job.machine = rs_cam_core::machine::MachineProfile::from_key(&legacy.job.machine);
    }
    job.post.spindle_speed = legacy.job.spindle_speed;
    job.post.safe_z = legacy.job.safe_z;
    job.post.format = match legacy.job.post.as_str() {
        "linuxcnc" => PostFormat::LinuxCnc,
        "mach3" => PostFormat::Mach3,
        _ => PostFormat::Grbl,
    };
    job.file_path = Some(path.to_path_buf());
    job.dirty = false;

    for tool in legacy.tools {
        let id = job.next_tool_id();
        job.tools.push(restore_legacy_tool(tool, id));
    }

    let mut model_ids_by_path = HashMap::new();
    for toolpath in &legacy.toolpaths {
        if toolpath.input.is_empty() {
            continue;
        }
        if model_ids_by_path.contains_key(&toolpath.input) {
            continue;
        }

        let model_id = job.next_model_id();
        let model = load_legacy_model(path, model_id, &toolpath.input, &mut warnings);
        model_ids_by_path.insert(toolpath.input.clone(), model_id);
        job.models.push(model);
    }

    let loaded_at = Instant::now();
    for legacy_toolpath in legacy.toolpaths {
        let id = job.next_toolpath_id();
        let operation_type = parse_legacy_operation_type(&legacy_toolpath.op_type);
        let operation = OperationConfig::new_default(operation_type);
        let tool_id = job
            .tools
            .get(legacy_toolpath.tool_index)
            .map(|tool| tool.id)
            .or_else(|| job.tools.first().map(|tool| tool.id))
            .unwrap_or(ToolId(0));
        let model_id = if legacy_toolpath.input.is_empty() {
            job.models
                .first()
                .map(|model| model.id)
                .unwrap_or(ModelId(0))
        } else {
            model_ids_by_path
                .get(&legacy_toolpath.input)
                .copied()
                .unwrap_or_else(|| {
                    job.models
                        .first()
                        .map(|model| model.id)
                        .unwrap_or(ModelId(0))
                })
        };

        let mut toolpath = ToolpathEntry::from_init(ToolpathEntryInit::from_loaded_state(
            id,
            default_toolpath_name(&legacy_toolpath.name, &operation, id),
            tool_id,
            model_id,
            operation,
        ));
        toolpath.clear_runtime_state();
        toolpath.stale_since = Some(loaded_at);

        if !job.tools.iter().any(|tool| tool.id == tool_id) {
            warnings.push(ProjectLoadWarning::MissingToolReference {
                toolpath: toolpath.name.clone(),
                tool_id,
            });
        }
        if !job.models.is_empty() && !job.models.iter().any(|model| model.id == model_id) {
            warnings.push(ProjectLoadWarning::MissingModelReference {
                toolpath: toolpath.name.clone(),
                model_id,
            });
        }

        job.push_toolpath(toolpath);
    }

    job.sync_next_ids();
    Ok(LoadedProject { job, warnings })
}

fn restore_legacy_tool(tool: LegacyToolSection, id: ToolId) -> ToolConfig {
    let tool_type = match tool.tool_type.as_str() {
        "ball" => ToolType::BallNose,
        "bullnose" => ToolType::BullNose,
        "vbit" => ToolType::VBit,
        "tapered_ball" => ToolType::TaperedBallNose,
        _ => ToolType::EndMill,
    };
    let mut restored = ToolConfig::new_default(id, tool_type);
    restored.name = tool.name;
    restored.diameter = tool.diameter;
    restored.cutting_length = tool.cutting_length;
    restored.corner_radius = tool.corner_radius;
    restored.included_angle = tool.included_angle;
    restored.taper_half_angle = tool.taper_half_angle;
    restored.shaft_diameter = tool.shaft_diameter;
    restored.flute_count = tool.flute_count;
    restored.tool_material = match tool.tool_material.as_str() {
        "hss" => ToolMaterial::Hss,
        _ => ToolMaterial::Carbide,
    };
    restored.cut_direction = match tool.cut_direction.as_str() {
        "downcut" => ToolCutDirection::DownCut,
        "compression" => ToolCutDirection::Compression,
        _ => ToolCutDirection::UpCut,
    };
    restored
}

fn load_legacy_model(
    project_path: &Path,
    id: ModelId,
    raw_input: &str,
    warnings: &mut Vec<ProjectLoadWarning>,
) -> LoadedModel {
    let resolved_path = resolve_model_path(project_path, raw_input);
    let kind = infer_model_kind(&resolved_path).unwrap_or(ModelKind::Svg);
    let units = default_units_for_kind(kind);
    let name = default_model_name(&resolved_path, kind);

    if !resolved_path.exists() {
        warnings.push(ProjectLoadWarning::MissingModelFile {
            name: name.clone(),
            path: resolved_path.clone(),
        });
        return LoadedModel::placeholder(
            id,
            resolved_path,
            name,
            kind,
            units,
            "Referenced model file not found".to_string(),
        );
    }

    match import::import_model(&resolved_path, id, kind, units) {
        Ok(model) => model,
        Err(error) => {
            let error_str = error.to_string();
            warnings.push(ProjectLoadWarning::ModelImportFailed {
                name: name.clone(),
                path: resolved_path.clone(),
                error: error_str.clone(),
            });
            LoadedModel::placeholder(id, resolved_path, name, kind, units, error_str)
        }
    }
}

fn load_model_section(
    project_path: &Path,
    id: ModelId,
    model: ProjectModelSection,
    warnings: &mut Vec<ProjectLoadWarning>,
) -> LoadedModel {
    let kind = model
        .kind
        .or_else(|| infer_model_kind(Path::new(&model.path)))
        .unwrap_or(ModelKind::Svg);
    let units = model.units.unwrap_or_else(|| default_units_for_kind(kind));
    let name = if model.name.is_empty() {
        default_model_name(Path::new(&model.path), kind)
    } else {
        model.name
    };

    if model.path.is_empty() {
        warnings.push(ProjectLoadWarning::MissingModelPath {
            name: name.clone(),
            model_id: id,
        });
        return LoadedModel::placeholder(
            id,
            PathBuf::new(),
            name,
            kind,
            units,
            "Model path missing from project file".to_string(),
        );
    }

    let resolved_path = resolve_model_path(project_path, &model.path);
    if !resolved_path.exists() {
        warnings.push(ProjectLoadWarning::MissingModelFile {
            name: name.clone(),
            path: resolved_path.clone(),
        });
        return LoadedModel::placeholder(
            id,
            resolved_path,
            name,
            kind,
            units,
            "Referenced model file not found".to_string(),
        );
    }

    match import::import_model(&resolved_path, id, kind, units) {
        Ok(mut loaded) => {
            loaded.name = name;
            loaded
        }
        Err(error) => {
            let error_str = error.to_string();
            warnings.push(ProjectLoadWarning::ModelImportFailed {
                name: name.clone(),
                path: resolved_path.clone(),
                error: error_str.clone(),
            });
            LoadedModel::placeholder(id, resolved_path, name, kind, units, error_str)
        }
    }
}

fn restore_project_setup(
    section: ProjectSetupSection,
    id: SetupId,
    used_fixture_ids: &mut BTreeSet<usize>,
    next_fixture_id: &mut usize,
    used_keep_out_ids: &mut BTreeSet<usize>,
    next_keep_out_id: &mut usize,
) -> (Setup, Vec<ProjectToolpathSection>) {
    let mut setup = Setup::new(id, section.name);
    setup.face_up = FaceUp::from_key(&section.face_up);
    setup.z_rotation = ZRotation::from_key(&section.z_rotation);
    if !section.xy_datum.is_empty() {
        setup.datum.xy_method = XYDatum::from_key(&section.xy_datum);
    }
    if !section.z_datum.is_empty() {
        setup.datum.z_method = ZDatum::from_key(&section.z_datum);
    }
    setup.datum.notes = section.datum_notes;
    // Pins are now on StockConfig; old-format pins on setups are migrated
    // after all setups are loaded (see migrate_setup_pins_to_stock below).
    setup.fixtures = section
        .fixtures
        .into_iter()
        .map(|fixture| {
            let id = FixtureId(assign_unique_id(
                fixture.id.map(|id| id.0),
                used_fixture_ids,
                next_fixture_id,
            ));
            restore_project_fixture(fixture, id)
        })
        .collect();
    setup.keep_out_zones = section
        .keep_out_zones
        .into_iter()
        .map(|keep_out| {
            let id = KeepOutId(assign_unique_id(
                keep_out.id.map(|id| id.0),
                used_keep_out_ids,
                next_keep_out_id,
            ));
            restore_project_keep_out(keep_out, id)
        })
        .collect();
    (setup, section.toolpaths)
}

fn restore_project_fixture(section: ProjectFixtureSection, id: FixtureId) -> Fixture {
    Fixture {
        id,
        name: section.name,
        kind: match section.kind.as_str() {
            "vise" => FixtureKind::Vise,
            "vacuum_pod" => FixtureKind::VacuumPod,
            "custom" => FixtureKind::Custom,
            _ => FixtureKind::Clamp,
        },
        enabled: section.enabled,
        origin_x: section.origin_x,
        origin_y: section.origin_y,
        origin_z: section.origin_z,
        size_x: section.size_x,
        size_y: section.size_y,
        size_z: section.size_z,
        clearance: section.clearance,
    }
}

fn restore_project_keep_out(section: ProjectKeepOutSection, id: KeepOutId) -> KeepOutZone {
    KeepOutZone {
        id,
        name: section.name,
        enabled: section.enabled,
        origin_x: section.origin_x,
        origin_y: section.origin_y,
        size_x: section.size_x,
        size_y: section.size_y,
    }
}

fn restore_project_toolpath(
    section: ProjectToolpathSection,
    id: ToolpathId,
    tools: &[ToolConfig],
    models: &[LoadedModel],
    loaded_at: Instant,
    warnings: &mut Vec<ProjectLoadWarning>,
) -> ToolpathEntry {
    let tool_id = section
        .tool_id
        .or_else(|| tools.first().map(|tool| tool.id))
        .unwrap_or(ToolId(0));
    let model_id = section
        .model_id
        .or_else(|| models.first().map(|model| model.id))
        .unwrap_or(ModelId(0));
    let operation = section.operation.unwrap_or_else(|| {
        OperationConfig::new_default(section.op_type.unwrap_or(OperationType::Pocket))
    });
    let mut init = ToolpathEntryInit::from_loaded_state(
        id,
        default_toolpath_name(&section.name, &operation, id),
        tool_id,
        model_id,
        operation,
    );
    init.enabled = section.enabled;
    init.visible = section.visible;
    init.locked = section.locked;
    init.dressups = section.dressups;
    init.heights = section.heights;
    init.boundary_enabled = section.boundary_enabled;
    init.boundary_containment = section.boundary_containment;
    init.coolant = section.coolant;
    init.pre_gcode = section.pre_gcode;
    init.post_gcode = section.post_gcode;
    init.stock_source = section.stock_source;
    init.auto_regen = section.auto_regen;
    init.feeds_auto = section.feeds_auto;
    init.face_selection = section.face_selection.map(|ids| {
        ids.into_iter()
            .map(rs_cam_core::enriched_mesh::FaceGroupId)
            .collect()
    });
    // Validate face_selection IDs against the loaded enriched mesh
    if let Some(face_ids) = &init.face_selection {
        let face_count = models
            .iter()
            .find(|m| m.id == model_id)
            .and_then(|m| m.enriched_mesh.as_ref())
            .map(|e| e.face_groups.len())
            .unwrap_or(0);
        if face_count > 0
            && let Some(bad) = face_ids.iter().find(|f| (f.0 as usize) >= face_count)
        {
            warnings.push(ProjectLoadWarning::FaceSelectionStale {
                toolpath: init.name.clone(),
                face_count,
                invalid_id: bad.0,
            });
            init.face_selection = None;
        }
    }
    init.debug_options = section.debug_options;
    let mut toolpath = ToolpathEntry::from_init(init);
    toolpath.clear_runtime_state();
    toolpath.stale_since = Some(loaded_at);

    if !tools.iter().any(|tool| tool.id == tool_id) {
        warnings.push(ProjectLoadWarning::MissingToolReference {
            toolpath: toolpath.name.clone(),
            tool_id,
        });
    }
    if !models.iter().any(|model| model.id == model_id) {
        warnings.push(ProjectLoadWarning::MissingModelReference {
            toolpath: toolpath.name.clone(),
            model_id,
        });
    }

    toolpath
}

fn assign_unique_id(
    preferred: Option<usize>,
    used: &mut BTreeSet<usize>,
    next_fallback: &mut usize,
) -> usize {
    if let Some(id) = preferred.filter(|id| !used.contains(id)) {
        used.insert(id);
        *next_fallback = (*next_fallback).max(id + 1);
        return id;
    }

    while used.contains(next_fallback) {
        *next_fallback += 1;
    }
    let id = *next_fallback;
    used.insert(id);
    *next_fallback += 1;
    id
}

fn persist_model_path(project_path: &Path, model_path: &Path) -> String {
    let Some(project_dir) = project_path.parent() else {
        return model_path.display().to_string();
    };

    if model_path.is_absolute() {
        if let Ok(relative) = model_path.strip_prefix(project_dir) {
            return relative.to_string_lossy().to_string();
        }
        return model_path.display().to_string();
    }

    model_path.to_string_lossy().to_string()
}

fn resolve_model_path(project_path: &Path, stored_path: &str) -> PathBuf {
    let path = PathBuf::from(stored_path);
    if path.is_absolute() {
        path
    } else {
        project_path
            .parent()
            .map(|dir| dir.join(&path))
            .unwrap_or(path)
    }
}

fn infer_model_kind(path: &Path) -> Option<ModelKind> {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some(ext) if ext.eq_ignore_ascii_case("stl") => Some(ModelKind::Stl),
        Some(ext) if ext.eq_ignore_ascii_case("svg") => Some(ModelKind::Svg),
        Some(ext) if ext.eq_ignore_ascii_case("dxf") => Some(ModelKind::Dxf),
        Some(ext) if ext.eq_ignore_ascii_case("step") || ext.eq_ignore_ascii_case("stp") => {
            Some(ModelKind::Step)
        }
        _ => None,
    }
}

fn default_model_name(path: &Path, kind: ModelKind) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| match kind {
            ModelKind::Stl => "model.stl".to_string(),
            ModelKind::Svg => "model.svg".to_string(),
            ModelKind::Dxf => "model.dxf".to_string(),
            ModelKind::Step => "model.step".to_string(),
        })
}

fn default_toolpath_name(saved_name: &str, operation: &OperationConfig, id: ToolpathId) -> String {
    if saved_name.is_empty() {
        format!("{} {}", operation.label(), id.0 + 1)
    } else {
        saved_name.to_string()
    }
}

fn default_units_for_kind(_kind: ModelKind) -> ModelUnits {
    ModelUnits::Millimeters
}

fn parse_legacy_operation_type(value: &str) -> OperationType {
    match value {
        "face" => OperationType::Face,
        "pocket" => OperationType::Pocket,
        "profile" => OperationType::Profile,
        "adaptive" => OperationType::Adaptive,
        "vcarve" => OperationType::VCarve,
        "rest_machining" => OperationType::Rest,
        "rest" => OperationType::Rest,
        "inlay" => OperationType::Inlay,
        "zigzag" => OperationType::Zigzag,
        "trace" => OperationType::Trace,
        "drill" => OperationType::Drill,
        "chamfer" => OperationType::Chamfer,
        "3d_finish" => OperationType::DropCutter,
        "drop_cutter" => OperationType::DropCutter,
        "3d_rough" => OperationType::Adaptive3d,
        "adaptive_3d" => OperationType::Adaptive3d,
        "adaptive3d" => OperationType::Adaptive3d,
        "waterline" => OperationType::Waterline,
        "pencil_finish" => OperationType::Pencil,
        "pencil" => OperationType::Pencil,
        "scallop_finish" => OperationType::Scallop,
        "scallop" => OperationType::Scallop,
        "steep/shallow" => OperationType::SteepShallow,
        "steep_shallow" => OperationType::SteepShallow,
        "ramp_finish" => OperationType::RampFinish,
        "spiral_finish" => OperationType::SpiralFinish,
        "radial_finish" => OperationType::RadialFinish,
        "horizontal_finish" => OperationType::HorizontalFinish,
        "project_curve" => OperationType::ProjectCurve,
        _ => OperationType::Pocket,
    }
}

fn default_project_format_version() -> u32 {
    1
}

fn default_setup_name() -> String {
    "Setup 1".to_string()
}

fn default_setup_face_up() -> String {
    "top".to_string()
}

fn default_setup_z_rotation() -> String {
    "0".to_string()
}

fn default_pin_diameter() -> f64 {
    6.0
}

fn default_fixture_name() -> String {
    "Fixture".to_string()
}

fn default_fixture_kind() -> String {
    "clamp".to_string()
}

fn default_fixture_size_x() -> f64 {
    30.0
}

fn default_fixture_size_y() -> f64 {
    15.0
}

fn default_fixture_size_z() -> f64 {
    20.0
}

fn default_fixture_clearance() -> f64 {
    3.0
}

fn default_keep_out_name() -> String {
    "Keep-Out".to_string()
}

fn default_keep_out_size() -> f64 {
    20.0
}

fn default_job_name() -> String {
    "Untitled".to_string()
}

fn default_tool_name() -> String {
    "Tool".to_string()
}

fn default_tool_type() -> ToolType {
    ToolType::EndMill
}

fn default_tool_diameter() -> f64 {
    6.35
}

fn default_cutting_length() -> f64 {
    25.0
}

fn default_included_angle() -> f64 {
    90.0
}

fn default_corner_radius() -> f64 {
    2.0
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

fn default_tool_material() -> ToolMaterial {
    ToolMaterial::Carbide
}

fn default_tool_cut_direction() -> ToolCutDirection {
    ToolCutDirection::UpCut
}

fn default_true() -> bool {
    true
}

fn default_legacy_spindle() -> u32 {
    18_000
}

fn default_legacy_safe_z() -> f64 {
    10.0
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use rs_cam_core::gcode::CoolantMode;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::state::job::{
        AlignmentPin, FaceUp, Fixture, KeepOutZone, ModelUnits, Setup, ToolId, XYDatum, ZDatum,
        ZRotation,
    };
    use crate::state::toolpath::{
        Adaptive3dConfig, BoundaryContainment, DressupEntryStyle, HeightMode, OperationConfig,
        OperationType, ScallopConfig,
    };

    fn repo_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .unwrap()
            .to_path_buf()
    }

    fn unique_temp_dir() -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("rs_cam_viz_project_{nonce}_{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn sample_tool() -> ToolConfig {
        let mut tool = ToolConfig::new_default(ToolId(7), ToolType::BallNose);
        tool.name = "Finisher".to_string();
        tool.vendor = "Vendor".to_string();
        tool.product_id = "SKU-42".to_string();
        tool
    }

    #[test]
    fn round_trip_persists_editable_2d_state() {
        let temp_dir = unique_temp_dir();
        let project_path = temp_dir.join("job.toml");
        let fixture_path = repo_root().join("fixtures/demo_star.svg");
        let model = import::import_model(
            &fixture_path,
            ModelId(3),
            ModelKind::Svg,
            ModelUnits::Millimeters,
        )
        .unwrap();

        let mut job = JobState::new();
        job.name = "Round Trip 2D".to_string();
        job.file_path = Some(project_path.clone());
        job.tools.push(sample_tool());
        job.models.push(model);

        let mut toolpath = ToolpathEntry::for_operation(
            ToolpathId(5),
            "Pocket A".to_string(),
            job.tools[0].id,
            job.models[0].id,
            OperationType::Pocket,
        );
        job.tools[0].tool_number = 23;
        toolpath.enabled = false;
        toolpath.visible = false;
        toolpath.locked = true;
        toolpath.dressups.entry_style = DressupEntryStyle::Ramp;
        toolpath.dressups.feed_optimization = true;
        toolpath.heights.bottom_z = HeightMode::Manual(-4.2);
        toolpath.boundary_enabled = true;
        toolpath.boundary_containment = BoundaryContainment::Inside;
        toolpath.coolant = CoolantMode::Mist;
        toolpath.pre_gcode = "M7".to_string();
        toolpath.post_gcode = "M9".to_string();
        toolpath.auto_regen = false;
        toolpath.feeds_auto.feed_rate = false;
        toolpath.debug_options.enabled = true;
        job.push_toolpath(toolpath);

        save_project(&job, &project_path).unwrap();
        let loaded = load_project(&project_path).unwrap();

        assert!(loaded.warnings.is_empty());
        assert_eq!(loaded.job.name, "Round Trip 2D");
        assert_eq!(loaded.job.tools.len(), 1);
        assert_eq!(loaded.job.models.len(), 1);
        assert_eq!(loaded.job.toolpath_count(), 1);
        assert_eq!(loaded.job.models[0].path, fixture_path);
        assert_eq!(loaded.job.tools[0].tool_number, 23);
        let toolpath = loaded.job.all_toolpaths().next().unwrap();
        assert!(!toolpath.enabled);
        assert!(!toolpath.visible);
        assert!(toolpath.locked);
        assert!(
            matches!(toolpath.heights.bottom_z, HeightMode::Manual(v) if (v + 4.2).abs() < 1e-9)
        );
        assert_eq!(toolpath.coolant, CoolantMode::Mist);
        assert_eq!(toolpath.pre_gcode, "M7");
        assert_eq!(toolpath.post_gcode, "M9");
        assert!(!toolpath.auto_regen);
        assert!(!toolpath.feeds_auto.feed_rate);
        assert!(toolpath.debug_options.enabled);
        assert!(toolpath.result.is_none());
        assert!(toolpath.stale_since.is_some());
        assert!(matches!(toolpath.operation, OperationConfig::Pocket(_)));

        fs::remove_dir_all(temp_dir).unwrap();
    }

    #[test]
    fn round_trip_persists_editable_3d_state() {
        let temp_dir = unique_temp_dir();
        let project_path = temp_dir.join("job.toml");
        let fixture_path = repo_root().join("fixtures/terrain_small.stl");
        let model = import::import_model(
            &fixture_path,
            ModelId(8),
            ModelKind::Stl,
            ModelUnits::Millimeters,
        )
        .unwrap();

        let mut job = JobState::new();
        job.name = "Round Trip 3D".to_string();
        job.tools.push(sample_tool());
        job.models.push(model);

        let init = ToolpathEntryInit::from_loaded_state(
            ToolpathId(12),
            "Roughing".to_string(),
            job.tools[0].id,
            job.models[0].id,
            OperationConfig::Adaptive3d(Adaptive3dConfig {
                stock_to_leave_radial: 0.7,
                stock_to_leave_axial: 0.4,
                ..Adaptive3dConfig::default()
            }),
        );
        let mut toolpath = ToolpathEntry::from_init(init);
        toolpath.dressups.dogbone = true;
        toolpath.stock_source = StockSource::FromRemainingStock;
        job.push_toolpath(toolpath);

        save_project(&job, &project_path).unwrap();
        let saved = fs::read_to_string(&project_path).unwrap();
        assert!(saved.contains("stock_to_leave_radial = 0.7"));

        let loaded = load_project(&project_path).unwrap();
        assert!(loaded.warnings.is_empty());
        assert!(matches!(
            loaded.job.all_toolpaths().next().unwrap().operation,
            OperationConfig::Adaptive3d(Adaptive3dConfig {
                stock_to_leave_radial,
                stock_to_leave_axial,
                ..
            }) if (stock_to_leave_radial - 0.7).abs() < 1e-9 && (stock_to_leave_axial - 0.4).abs() < 1e-9
        ));

        fs::remove_dir_all(temp_dir).unwrap();
    }

    #[test]
    fn round_trip_persists_multi_setup_state() {
        let temp_dir = unique_temp_dir();
        let project_path = temp_dir.join("multi_setup.toml");
        let fixture_path = repo_root().join("fixtures/demo_star.svg");
        let model = import::import_model(
            &fixture_path,
            ModelId(3),
            ModelKind::Svg,
            ModelUnits::Millimeters,
        )
        .unwrap();

        let mut job = JobState::new();
        job.name = "Multi Setup".to_string();
        job.file_path = Some(project_path.clone());
        job.tools.push(sample_tool());
        job.models.push(model);

        let default_setup_id = job.setups[0].id;
        let fixture_id = job.next_fixture_id();
        let keep_out_id = job.next_keep_out_id();
        let second_setup_id = job.next_setup_id();

        {
            let top_setup = &mut job.setups[0];
            top_setup.name = "Top Side".to_string();
            top_setup.datum.xy_method = XYDatum::AlignmentPins;
            top_setup.datum.z_method = ZDatum::MachineTable;
            top_setup.datum.notes = "Probe pins first".to_string();
            // Pins are stock-level now.
            job.stock
                .alignment_pins
                .push(AlignmentPin::new(10.0, 20.0, 6.0));

            let mut fixture = Fixture::new_default(fixture_id);
            fixture.name = "Toe Clamp".to_string();
            fixture.origin_x = 12.0;
            fixture.origin_y = 8.0;
            top_setup.fixtures.push(fixture);

            let mut keep_out = KeepOutZone::new_default(keep_out_id);
            keep_out.name = "Clamp Swing".to_string();
            keep_out.origin_x = 14.0;
            keep_out.origin_y = 18.0;
            top_setup.keep_out_zones.push(keep_out);
        }

        let mut bottom_setup = Setup::new(second_setup_id, "Bottom Side".to_string());
        bottom_setup.face_up = FaceUp::Bottom;
        bottom_setup.z_rotation = ZRotation::Deg90;
        bottom_setup.datum.xy_method = XYDatum::AlignmentPins;
        bottom_setup.datum.notes = "Locate from dowel pins".to_string();
        job.stock
            .alignment_pins
            .push(AlignmentPin::new(15.0, 25.0, 6.0));
        job.setups.push(bottom_setup);

        job.push_toolpath_to_setup(
            default_setup_id,
            ToolpathEntry::for_operation(
                ToolpathId(5),
                "Top Pocket".to_string(),
                job.tools[0].id,
                job.models[0].id,
                OperationType::Pocket,
            ),
        );
        job.push_toolpath_to_setup(
            second_setup_id,
            ToolpathEntry::for_operation(
                ToolpathId(6),
                "Bottom Profile".to_string(),
                job.tools[0].id,
                job.models[0].id,
                OperationType::Profile,
            ),
        );

        save_project(&job, &project_path).unwrap();
        let saved = fs::read_to_string(&project_path).unwrap();
        assert!(saved.contains("format_version = 3"));
        assert!(saved.contains("[[setups]]"));
        assert!(saved.contains("[[setups.toolpaths]]"));

        let loaded = load_project(&project_path).unwrap();
        assert!(loaded.warnings.is_empty());
        assert_eq!(loaded.job.setups.len(), 2);
        assert_eq!(loaded.job.toolpath_count(), 2);

        let top_setup = &loaded.job.setups[0];
        assert_eq!(top_setup.name, "Top Side");
        assert_eq!(top_setup.datum.xy_method, XYDatum::AlignmentPins);
        assert_eq!(top_setup.datum.z_method, ZDatum::MachineTable);
        assert_eq!(top_setup.datum.notes, "Probe pins first");
        // Pins are on stock, not setup.
        assert_eq!(loaded.job.stock.alignment_pins.len(), 2);
        assert_eq!(top_setup.fixtures.len(), 1);
        assert_eq!(top_setup.fixtures[0].name, "Toe Clamp");
        assert_eq!(top_setup.keep_out_zones.len(), 1);
        assert_eq!(top_setup.keep_out_zones[0].name, "Clamp Swing");
        assert_eq!(top_setup.toolpaths.len(), 1);
        assert_eq!(top_setup.toolpaths[0].name, "Top Pocket");

        let bottom_setup = &loaded.job.setups[1];
        assert_eq!(bottom_setup.name, "Bottom Side");
        assert_eq!(bottom_setup.face_up, FaceUp::Bottom);
        assert_eq!(bottom_setup.z_rotation, ZRotation::Deg90);
        assert_eq!(bottom_setup.datum.xy_method, XYDatum::AlignmentPins);
        assert_eq!(bottom_setup.datum.notes, "Locate from dowel pins");
        assert_eq!(bottom_setup.toolpaths.len(), 1);
        assert_eq!(bottom_setup.toolpaths[0].name, "Bottom Profile");

        fs::remove_dir_all(temp_dir).unwrap();
    }

    #[test]
    fn legacy_sparse_projects_still_load() {
        let temp_dir = unique_temp_dir();
        let project_path = temp_dir.join("legacy.toml");
        let fixture_path = repo_root().join("fixtures/demo_star.svg");

        let legacy = format!(
            r#"[job]
name = "Legacy"
post = "grbl"
spindle_speed = 18000
safe_z = 10.0
stock_x = 100.0
stock_y = 100.0
stock_z = 10.0

[[tools]]
name = "Legacy Tool"
type = "flat"
diameter = 6.35

[[toolpaths]]
name = "Legacy Pocket"
type = "pocket"
tool_index = 0
input = "{}"
"#,
            fixture_path.display()
        );
        fs::write(&project_path, legacy).unwrap();

        let loaded = load_project(&project_path).unwrap();
        assert!(loaded.warnings.is_empty());
        assert_eq!(loaded.job.tools.len(), 1);
        assert_eq!(loaded.job.models.len(), 1);
        assert_eq!(loaded.job.toolpath_count(), 1);
        assert!(matches!(
            loaded.job.all_toolpaths().next().unwrap().operation,
            OperationConfig::Pocket(_)
        ));

        fs::remove_dir_all(temp_dir).unwrap();
    }

    #[test]
    fn relative_paths_are_saved_and_resolved_against_project_dir() {
        let temp_dir = unique_temp_dir();
        let project_path = temp_dir.join("nested").join("job.toml");
        let model_dir = project_path.parent().unwrap().join("assets");
        fs::create_dir_all(&model_dir).unwrap();
        let model_path = model_dir.join("demo_star.svg");
        fs::copy(repo_root().join("fixtures/demo_star.svg"), &model_path).unwrap();

        let mut job = JobState::new();
        job.tools.push(sample_tool());
        job.models.push(
            import::import_model(
                &model_path,
                ModelId(1),
                ModelKind::Svg,
                ModelUnits::Millimeters,
            )
            .unwrap(),
        );
        job.push_toolpath(ToolpathEntry::for_operation(
            ToolpathId(1),
            "Pocket".to_string(),
            job.tools[0].id,
            job.models[0].id,
            OperationType::Pocket,
        ));

        save_project(&job, &project_path).unwrap();
        let saved = fs::read_to_string(&project_path).unwrap();
        assert!(saved.contains("path = \"assets/demo_star.svg\""));

        let loaded = load_project(&project_path).unwrap();
        assert_eq!(loaded.job.models[0].path, model_path);

        fs::remove_dir_all(temp_dir).unwrap();
    }

    #[test]
    fn missing_models_warn_but_do_not_abort_load() {
        let temp_dir = unique_temp_dir();
        let project_path = temp_dir.join("job.toml");
        let content = r#"
format_version = 1

[job]
name = "Missing Model"

[[tools]]
id = 1
name = "Tool"
type = "end_mill"
diameter = 6.35

[[models]]
id = 2
name = "Missing"
path = "missing/demo.svg"
kind = "svg"
units = { kind = "millimeters" }

[[toolpaths]]
id = 3
name = "Pocket"
type = "pocket"
tool_id = 1
model_id = 2
operation = { kind = "pocket", params = { depth = 2.0, depth_per_pass = 1.0, feed_rate = 500.0, plunge_rate = 200.0, climb = true, pattern = "contour", angle = 0.0, finishing_passes = 0, stepover = 1.0 } }
"#;
        fs::write(&project_path, content).unwrap();

        let loaded = load_project(&project_path).unwrap();
        assert_eq!(loaded.job.models.len(), 1);
        assert_eq!(loaded.job.toolpath_count(), 1);
        assert_eq!(loaded.warnings.len(), 1);
        assert!(loaded.job.models[0].mesh.is_none());
        assert!(loaded.job.models[0].polygons.is_none());
        assert!(loaded.job.models[0].load_error.is_some());
        let toolpath = loaded.job.all_toolpaths().next().unwrap();
        assert!(toolpath.result.is_none());
        assert!(toolpath.stale_since.is_some());

        fs::remove_dir_all(temp_dir).unwrap();
    }

    #[test]
    fn typed_loader_defaults_missing_fields_cleanly() {
        let temp_dir = unique_temp_dir();
        let project_path = temp_dir.join("partial.toml");
        let content = r#"
format_version = 1

[job]
name = "Partial"

[[tools]]
name = "Tool"
type = "ball_nose"
diameter = 3.175

[[models]]
path = "/tmp/example.svg"
kind = "svg"

[[toolpaths]]
name = "Scallop"
type = "scallop"
"#;
        fs::write(&project_path, content).unwrap();

        let loaded = load_project(&project_path).unwrap();
        assert_eq!(loaded.job.tools.len(), 1);
        assert_eq!(loaded.job.toolpath_count(), 1);
        assert!(matches!(
            loaded.job.all_toolpaths().next().unwrap().operation,
            OperationConfig::Scallop(ScallopConfig { .. })
        ));

        fs::remove_dir_all(temp_dir).unwrap();
    }

    #[test]
    fn test_save_project_atomic_write() {
        // Verify that save_project uses temp+rename pattern:
        // after a successful save, the file should exist and be valid TOML,
        // and no .tmp file should remain.
        let temp_dir = unique_temp_dir();
        fs::create_dir_all(&temp_dir).unwrap();
        let project_path = temp_dir.join("atomic_test.rcam");

        let job = JobState::new();
        save_project(&job, &project_path).unwrap();

        // The file should exist and be valid
        assert!(
            project_path.exists(),
            "Project file should exist after save"
        );
        let content = fs::read_to_string(&project_path).unwrap();
        assert!(
            toml::from_str::<ProjectFile>(&content).is_ok(),
            "Saved file should be valid TOML"
        );

        // No .tmp file should remain
        let tmp_path = project_path.with_extension("tmp");
        assert!(
            !tmp_path.exists(),
            "Temporary file should not remain after successful save"
        );

        fs::remove_dir_all(temp_dir).unwrap();
    }
}
