//! Unified ProjectSession API — a single entry point for project state + compute
//! that GUI, CLI, and a future MCP server can all use.
//!
//! # Usage
//!
//! ```ignore
//! let mut session = ProjectSession::load(Path::new("my_project.toml"))?;
//! let cancel = AtomicBool::new(false);
//! session.generate_all(&[], &cancel)?;
//! session.run_simulation(SimulationOptions::default(), &cancel)?;
//! let diag = session.diagnostics();
//! ```

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use serde::Deserialize;

use crate::collision::check_rapid_collisions;
use crate::compute::catalog::OperationConfig;
use crate::compute::collision_check::{
    CollisionCheckError, CollisionCheckRequest, CollisionCheckResult, run_collision_check,
};
use crate::compute::config::{
    DressupConfig, HeightContext, HeightsConfig, ToolpathStats,
};
use crate::compute::cutter::build_cutter;
use crate::compute::simulate::{
    SimGroupEntry, SimToolpathEntry, SimulationError, SimulationRequest, SimulationResult,
    run_simulation,
};
use crate::compute::stock_config::{ModelKind, ModelUnits, StockConfig};
use crate::compute::tool_config::{ToolConfig, ToolId, ToolType};
use crate::compute::transform::FaceUp;
use crate::debug_trace::{ToolpathDebugRecorder, ToolpathDebugTrace};
use crate::dexel_stock::StockCutDirection;
use crate::geo::{BoundingBox3, P3};
use crate::mesh::TriangleMesh;
use crate::polygon::Polygon2;
use crate::semantic_trace::{ToolpathSemanticRecorder, ToolpathSemanticTrace, enrich_traces};
use crate::simulation_cut::SimulationMetricOptions;
use crate::toolpath::Toolpath;

// ── Error types ────────────────────────────────────────────────────────

/// Errors that can occur during session operations.
#[derive(Debug)]
pub enum SessionError {
    /// I/O error (file not found, permission denied, etc.).
    Io(std::io::Error),
    /// TOML parsing error.
    TomlParse(String),
    /// Model loading failure.
    ModelLoad { name: String, detail: String },
    /// Toolpath not found by index.
    ToolpathNotFound(usize),
    /// Tool not found by id.
    ToolNotFound(ToolId),
    /// Geometry missing for the requested operation.
    MissingGeometry(String),
    /// Operation execution failure.
    OperationFailed(String),
    /// Simulation error.
    Simulation(SimulationError),
    /// Collision check error.
    CollisionCheck(CollisionCheckError),
    /// Export error.
    Export(String),
}

impl std::fmt::Display for SessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error: {e}"),
            Self::TomlParse(e) => write!(f, "TOML parse error: {e}"),
            Self::ModelLoad { name, detail } => {
                write!(f, "Failed to load model '{name}': {detail}")
            }
            Self::ToolpathNotFound(id) => write!(f, "Toolpath {id} not found"),
            Self::ToolNotFound(id) => write!(f, "Tool {} not found", id.0),
            Self::MissingGeometry(msg) => write!(f, "Missing geometry: {msg}"),
            Self::OperationFailed(msg) => write!(f, "Operation failed: {msg}"),
            Self::Simulation(e) => write!(f, "Simulation error: {e}"),
            Self::CollisionCheck(e) => write!(f, "Collision check error: {e}"),
            Self::Export(msg) => write!(f, "Export error: {msg}"),
        }
    }
}

impl std::error::Error for SessionError {}

impl From<std::io::Error> for SessionError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<SimulationError> for SessionError {
    fn from(e: SimulationError) -> Self {
        Self::Simulation(e)
    }
}

impl From<CollisionCheckError> for SessionError {
    fn from(e: CollisionCheckError) -> Self {
        Self::CollisionCheck(e)
    }
}

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
}

fn default_true() -> bool {
    true
}

// ── Loaded state types ─────────────────────────────────────────────────

/// Geometry loaded from a model file.
enum LoadedGeometry {
    Mesh(TriangleMesh),
    Polygons(Vec<Polygon2>),
}

/// A loaded model with its geometry.
pub struct LoadedModel {
    pub id: usize,
    pub name: String,
    pub mesh: Option<Arc<TriangleMesh>>,
    pub polygons: Option<Arc<Vec<Polygon2>>>,
}

/// A setup's orientation and toolpath indices.
pub struct SetupData {
    pub id: usize,
    pub name: String,
    pub face_up: FaceUp,
    /// Indices into the session's `toolpath_configs` vec.
    pub toolpath_indices: Vec<usize>,
}

/// Configuration for a single toolpath within the session.
pub struct ToolpathConfig {
    pub id: usize,
    pub name: String,
    pub enabled: bool,
    pub operation: OperationConfig,
    pub dressups: DressupConfig,
    pub heights: HeightsConfig,
    pub tool_id: usize,
    pub model_id: usize,
}

/// Result of generating a single toolpath.
pub struct ToolpathComputeResult {
    pub toolpath: Toolpath,
    pub stats: ToolpathStats,
    pub debug_trace: Option<ToolpathDebugTrace>,
    pub semantic_trace: Option<ToolpathSemanticTrace>,
}

/// Summary of a toolpath for listing.
pub struct ToolpathSummary {
    pub index: usize,
    pub id: usize,
    pub name: String,
    pub operation_label: String,
    pub enabled: bool,
    pub tool_name: String,
}

/// Summary of a tool for listing.
pub struct ToolSummary {
    pub id: ToolId,
    pub name: String,
    pub tool_type: ToolType,
    pub diameter: f64,
}

/// Options for running simulation.
pub struct SimulationOptions {
    /// Resolution in mm for tri-dexel stock.
    pub resolution: f64,
    /// Toolpath IDs to skip.
    pub skip_ids: Vec<usize>,
    /// Whether to collect detailed cut metrics.
    pub metrics_enabled: bool,
}

impl Default for SimulationOptions {
    fn default() -> Self {
        Self {
            resolution: 0.5,
            skip_ids: Vec::new(),
            metrics_enabled: true,
        }
    }
}

/// Per-toolpath diagnostic summary.
#[derive(Debug, Clone)]
pub struct ToolpathDiagnostic {
    pub toolpath_id: usize,
    pub name: String,
    pub operation_type: String,
    pub tool_name: String,
    pub move_count: usize,
    pub cutting_distance_mm: f64,
    pub rapid_distance_mm: f64,
    pub collision_count: usize,
    pub rapid_collision_count: usize,
}

/// Project-level diagnostics summary.
#[derive(Debug, Clone)]
pub struct ProjectDiagnostics {
    pub total_runtime_s: f64,
    pub air_cut_percentage: f64,
    pub average_engagement: f64,
    pub collision_count: usize,
    pub rapid_collision_count: usize,
    pub per_toolpath: Vec<ToolpathDiagnostic>,
    pub verdict: String,
}

// ── ProjectSession ─────────────────────────────────────────────────────

/// Unified project session that owns state and provides compute methods.
///
/// Use [`ProjectSession::load`] to load from a TOML project file, or
/// [`ProjectSession::from_project_file`] to construct from a parsed file.
pub struct ProjectSession {
    // Project metadata
    name: String,
    stock: StockConfig,
    post: ProjectPostConfig,
    #[allow(dead_code)] // Will be used in Phase 5+ (CLI/MCP wiring)
    machine: crate::machine::MachineProfile,

    // Loaded state
    models: Vec<LoadedModel>,
    tools: Vec<ToolConfig>,
    setups: Vec<SetupData>,
    toolpath_configs: Vec<ToolpathConfig>,

    // Computed results (keyed by toolpath index)
    results: HashMap<usize, ToolpathComputeResult>,
    simulation: Option<SimulationResult>,
}

impl ProjectSession {
    // ── Lifecycle ───────────────────────────────────────────────────

    /// Load a project from a TOML file path.
    pub fn load(path: &Path) -> Result<Self, SessionError> {
        let content = std::fs::read_to_string(path)?;
        let project: ProjectFile =
            toml::from_str(&content).map_err(|e| SessionError::TomlParse(e.to_string()))?;
        let base_dir = path.parent().unwrap_or(Path::new("."));
        Self::from_project_file(project, base_dir)
    }

    /// Construct a session from a parsed project file.
    pub fn from_project_file(project: ProjectFile, base_dir: &Path) -> Result<Self, SessionError> {
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
            // Legacy: top-level toolpaths → single default setup
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

        Ok(Self {
            name: project.job.name.clone(),
            stock,
            post: project.job.post,
            machine: project.job.machine,
            models,
            tools,
            setups,
            toolpath_configs,
            results: HashMap::new(),
            simulation: None,
        })
    }

    // ── Queries ────────────────────────────────────────────────────

    /// Project name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Stock configuration.
    pub fn stock_config(&self) -> &StockConfig {
        &self.stock
    }

    /// Bounding box of the stock.
    pub fn stock_bbox(&self) -> BoundingBox3 {
        self.stock.bbox()
    }

    /// List all toolpaths with summary info.
    pub fn list_toolpaths(&self) -> Vec<ToolpathSummary> {
        self.toolpath_configs
            .iter()
            .enumerate()
            .map(|(idx, tc)| {
                let tool_name = self
                    .find_tool_by_raw_id(tc.tool_id)
                    .map(|t| t.name.clone())
                    .unwrap_or_else(|| "Unknown tool".to_owned());
                ToolpathSummary {
                    index: idx,
                    id: tc.id,
                    name: tc.name.clone(),
                    operation_label: tc.operation.label().to_owned(),
                    enabled: tc.enabled,
                    tool_name,
                }
            })
            .collect()
    }

    /// List all tools with summary info.
    pub fn list_tools(&self) -> Vec<ToolSummary> {
        self.tools
            .iter()
            .map(|t| ToolSummary {
                id: t.id,
                name: t.name.clone(),
                tool_type: t.tool_type,
                diameter: t.diameter,
            })
            .collect()
    }

    /// Get the operation config for a toolpath by index.
    pub fn get_toolpath_params(&self, index: usize) -> Option<&OperationConfig> {
        self.toolpath_configs.get(index).map(|tc| &tc.operation)
    }

    /// Get a tool by its `ToolId`.
    pub fn get_tool(&self, id: ToolId) -> Option<&ToolConfig> {
        self.tools.iter().find(|t| t.id == id)
    }

    /// Get a computed toolpath result by index.
    pub fn get_result(&self, index: usize) -> Option<&ToolpathComputeResult> {
        self.results.get(&index)
    }

    /// Get the simulation result, if one has been run.
    pub fn simulation_result(&self) -> Option<&SimulationResult> {
        self.simulation.as_ref()
    }

    /// Number of toolpath configs in the session.
    pub fn toolpath_count(&self) -> usize {
        self.toolpath_configs.len()
    }

    /// Number of setups.
    pub fn setup_count(&self) -> usize {
        self.setups.len()
    }

    /// Access all setups (for setup filtering, etc.).
    pub fn list_setups(&self) -> &[SetupData] {
        &self.setups
    }

    /// Get a toolpath config by index.
    pub fn get_toolpath_config(&self, index: usize) -> Option<&ToolpathConfig> {
        self.toolpath_configs.get(index)
    }

    /// Post-processor configuration.
    pub fn post_config(&self) -> &ProjectPostConfig {
        &self.post
    }

    // ── Compute ────────────────────────────────────────────────────

    /// Generate a single toolpath by index.
    pub fn generate_toolpath(
        &mut self,
        index: usize,
        cancel: &AtomicBool,
    ) -> Result<&ToolpathComputeResult, SessionError> {
        let tc = self
            .toolpath_configs
            .get(index)
            .ok_or(SessionError::ToolpathNotFound(index))?;

        let tool = self
            .find_tool_by_raw_id(tc.tool_id)
            .ok_or(SessionError::ToolNotFound(ToolId(tc.tool_id)))?
            .clone();

        let model = self.find_model_by_raw_id(tc.model_id);

        let mesh = model.and_then(|m| m.mesh.clone());
        let polygons = model.and_then(|m| m.polygons.clone());

        // Validate geometry requirements
        if tc.operation.is_3d() && mesh.is_none() {
            return Err(SessionError::MissingGeometry(
                "Operation requires a 3D mesh (STL/STEP)".to_owned(),
            ));
        }
        if !tc.operation.is_3d() && !tc.operation.is_stock_based() && polygons.is_none() {
            return Err(SessionError::MissingGeometry(
                "Operation requires 2D geometry (SVG/DXF)".to_owned(),
            ));
        }

        // Find the setup for orientation
        let setup = self.find_setup_for_toolpath_index(index);
        let face_up = setup.map(|s| s.face_up).unwrap_or(FaceUp::Top);

        // Build effective stock bbox (apply setup orientation)
        let effective_stock_bbox = self.effective_stock_bbox(face_up);

        // Resolve heights
        let model_bbox = mesh.as_ref().map(|m| &m.bbox);
        let height_ctx = HeightContext {
            safe_z: self.post.safe_z,
            op_depth: tc.operation.default_depth_for_heights(),
            stock_top_z: effective_stock_bbox.max.z,
            stock_bottom_z: effective_stock_bbox.min.z,
            model_top_z: model_bbox.map(|b| b.max.z),
            model_bottom_z: model_bbox.map(|b| b.min.z),
        };
        let heights = tc.heights.resolve(&height_ctx);

        // Build tool definition
        let tool_def = build_cutter(&tool);

        // Build spatial index for 3D ops
        let spatial_index = mesh
            .as_ref()
            .map(|m| crate::mesh::SpatialIndex::build_auto(m));

        // Create recorders
        let debug_recorder = ToolpathDebugRecorder::new(tc.name.clone(), tc.operation.label());
        let semantic_recorder =
            ToolpathSemanticRecorder::new(tc.name.clone(), tc.operation.label());
        let debug_root = debug_recorder.root_context();
        let _semantic_root = semantic_recorder.root_context();

        let core_scope = debug_root.start_span("core_generate", tc.operation.label());
        let core_ctx = core_scope.context();

        // Execute the operation via the shared compute::execute module
        let tp_result = crate::compute::execute::execute_operation(
            &tc.operation,
            mesh.as_deref(),
            spatial_index.as_ref(),
            polygons.as_deref().map(|v| v.as_slice()),
            &tool_def,
            &tool,
            &heights,
            &[],  // no pre-computed cutting levels; DepthStepping used internally
            &effective_stock_bbox,
            None, // no prev_tool_radius for session path
            Some(&core_ctx),
            cancel,
        );

        match tp_result {
            Ok(mut toolpath) => {
                if !toolpath.moves.is_empty() {
                    core_scope.set_move_range(0, toolpath.moves.len().saturating_sub(1));
                }
                drop(core_scope);

                // Apply dressups
                toolpath = crate::compute::execute::apply_dressups(
                    toolpath,
                    &tc.dressups,
                    tool.diameter,
                    heights.retract_z,
                );

                let stats = ToolpathStats {
                    move_count: toolpath.moves.len(),
                    cutting_distance: toolpath.total_cutting_distance(),
                    rapid_distance: toolpath.total_rapid_distance(),
                };

                let mut debug_trace = debug_recorder.finish();
                let mut semantic_trace = semantic_recorder.finish();
                enrich_traces(&mut debug_trace, &mut semantic_trace);

                self.results.insert(
                    index,
                    ToolpathComputeResult {
                        toolpath,
                        stats,
                        debug_trace: Some(debug_trace),
                        semantic_trace: Some(semantic_trace),
                    },
                );
                // SAFETY: we just inserted at this key
                #[allow(clippy::indexing_slicing)]
                Ok(&self.results[&index])
            }
            Err(e) => {
                drop(core_scope);
                let _ = debug_recorder.finish();
                let _ = semantic_recorder.finish();
                Err(SessionError::OperationFailed(e.to_string()))
            }
        }
    }

    /// Generate all enabled toolpaths, skipping those whose IDs are in `skip`.
    pub fn generate_all(
        &mut self,
        skip_ids: &[usize],
        cancel: &AtomicBool,
    ) -> Result<(), SessionError> {
        // Collect info needed for skip/logging before mutable borrow
        let tp_info: Vec<(usize, usize, String, bool)> = self
            .toolpath_configs
            .iter()
            .enumerate()
            .map(|(idx, tc)| (idx, tc.id, tc.name.clone(), tc.enabled))
            .collect();

        for (idx, tp_id, tp_name, enabled) in &tp_info {
            if !enabled {
                continue;
            }
            if skip_ids.contains(tp_id) {
                tracing::info!(id = tp_id, name = %tp_name, "Skipping toolpath (skip list)");
                continue;
            }
            match self.generate_toolpath(*idx, cancel) {
                Ok(_) => {}
                Err(SessionError::MissingGeometry(msg)) => {
                    tracing::warn!(id = tp_id, name = %tp_name, reason = %msg, "Skipping toolpath");
                }
                Err(e) => {
                    tracing::error!(id = tp_id, name = %tp_name, error = %e, "Toolpath failed");
                }
            }
        }
        Ok(())
    }

    // ── Analysis ───────────────────────────────────────────────────

    /// Run tri-dexel stock simulation over all computed toolpaths.
    pub fn run_simulation(
        &mut self,
        opts: &SimulationOptions,
        cancel: &AtomicBool,
    ) -> Result<&SimulationResult, SessionError> {
        let stock_bbox = self.stock_bbox();

        // Build simulation groups from setups
        let mut groups = Vec::new();
        for setup in &self.setups {
            let direction = match setup.face_up {
                FaceUp::Bottom => StockCutDirection::FromBottom,
                _ => StockCutDirection::FromTop,
            };

            let mut entries = Vec::new();
            for &tp_idx in &setup.toolpath_indices {
                if let Some(result) = self.results.get(&tp_idx) {
                    let Some(tc) = self.toolpath_configs.get(tp_idx) else {
                        continue;
                    };
                    if opts.skip_ids.contains(&tc.id) {
                        continue;
                    }
                    if result.toolpath.moves.len() < 2 {
                        continue;
                    }

                    let tool_config = self.find_tool_by_raw_id(tc.tool_id);
                    let flute_count = tool_config.map(|t| t.flute_count).unwrap_or(2);
                    let tool_summary = tool_config
                        .map(|t| t.summary())
                        .unwrap_or_else(|| "Unknown".to_owned());
                    let tool_def = tool_config.map(build_cutter).unwrap_or_else(|| {
                        build_cutter(&ToolConfig::new_default(ToolId(0), ToolType::EndMill))
                    });

                    entries.push(SimToolpathEntry {
                        id: tc.id,
                        name: tc.name.clone(),
                        toolpath: Arc::new(result.toolpath.clone()),
                        tool: tool_def,
                        flute_count,
                        tool_summary,
                        semantic_trace: result.semantic_trace.as_ref().map(|t| Arc::new(t.clone())),
                    });
                }
            }

            if !entries.is_empty() {
                groups.push(SimGroupEntry {
                    toolpaths: entries,
                    direction,
                });
            }
        }

        let request = SimulationRequest {
            groups,
            stock_bbox,
            stock_top_z: stock_bbox.max.z,
            resolution: opts.resolution,
            metric_options: SimulationMetricOptions {
                enabled: opts.metrics_enabled,
            },
            spindle_rpm: self.post.spindle_speed,
            rapid_feed_mm_min: if self.post.high_feedrate_mode {
                self.post.high_feedrate
            } else {
                5000.0
            },
            model_mesh: self.models.iter().find_map(|m| m.mesh.clone()),
        };

        let result = run_simulation(&request, cancel)?;
        self.simulation = Some(result);
        // SAFETY: we just assigned Some
        #[allow(clippy::unwrap_used)]
        Ok(self.simulation.as_ref().unwrap())
    }

    /// Run a collision check for a specific toolpath by index.
    pub fn collision_check(
        &self,
        index: usize,
        cancel: &AtomicBool,
    ) -> Result<CollisionCheckResult, SessionError> {
        let tc = self
            .toolpath_configs
            .get(index)
            .ok_or(SessionError::ToolpathNotFound(index))?;
        let result = self
            .results
            .get(&index)
            .ok_or(SessionError::ToolpathNotFound(index))?;
        let model = self
            .find_model_by_raw_id(tc.model_id)
            .and_then(|m| m.mesh.as_ref())
            .ok_or_else(|| {
                SessionError::MissingGeometry("Collision check requires a 3D mesh".to_owned())
            })?;

        let tool = self
            .find_tool_by_raw_id(tc.tool_id)
            .ok_or(SessionError::ToolNotFound(ToolId(tc.tool_id)))?;
        let tool_def = build_cutter(tool);

        let request = CollisionCheckRequest {
            toolpath: result.toolpath.clone(),
            tool: tool_def,
            mesh: model.as_ref().clone(),
        };
        let check_result = run_collision_check(&request, cancel)?;
        Ok(check_result)
    }

    /// Compute project diagnostics from current results and simulation.
    pub fn diagnostics(&self) -> ProjectDiagnostics {
        let stock_bbox = self.stock_bbox();

        let mut per_toolpath = Vec::new();
        let total_collision_count: usize = 0;
        let mut total_rapid_collision_count: usize = 0;

        for (idx, tc) in self.toolpath_configs.iter().enumerate() {
            if let Some(result) = self.results.get(&idx) {
                let rapid_collisions = check_rapid_collisions(&result.toolpath, &stock_bbox);
                let rapid_count = rapid_collisions.len();
                total_rapid_collision_count += rapid_count;

                let tool_name = self
                    .find_tool_by_raw_id(tc.tool_id)
                    .map(|t| t.name.clone())
                    .unwrap_or_default();

                per_toolpath.push(ToolpathDiagnostic {
                    toolpath_id: tc.id,
                    name: tc.name.clone(),
                    operation_type: tc.operation.label().to_owned(),
                    tool_name,
                    move_count: result.stats.move_count,
                    cutting_distance_mm: result.stats.cutting_distance,
                    rapid_distance_mm: result.stats.rapid_distance,
                    collision_count: 0, // Full collision check requires explicit call
                    rapid_collision_count: rapid_count,
                });
            }
        }

        // Extract simulation metrics if available
        let (total_runtime_s, air_cut_percentage, average_engagement) =
            if let Some(sim) = &self.simulation {
                if let Some(trace) = &sim.cut_trace {
                    let summary = &trace.summary;
                    let air_pct = if summary.total_runtime_s > 0.0 {
                        summary.air_cut_time_s / summary.total_runtime_s * 100.0
                    } else {
                        0.0
                    };
                    (summary.total_runtime_s, air_pct, summary.average_engagement)
                } else {
                    (0.0, 0.0, 0.0)
                }
            } else {
                (0.0, 0.0, 0.0)
            };

        let verdict = if total_collision_count > 0 {
            format!(
                "ERROR: {} holder/shank collisions detected",
                total_collision_count
            )
        } else if total_rapid_collision_count > 0 {
            format!(
                "WARNING: {} rapid-through-stock collisions",
                total_rapid_collision_count
            )
        } else if air_cut_percentage > 40.0 {
            format!("WARNING: {air_cut_percentage:.1}% air cutting")
        } else {
            "OK".to_owned()
        };

        ProjectDiagnostics {
            total_runtime_s,
            air_cut_percentage,
            average_engagement,
            collision_count: total_collision_count,
            rapid_collision_count: total_rapid_collision_count,
            per_toolpath,
            verdict,
        }
    }

    // ── Export ──────────────────────────────────────────────────────

    /// Export G-code for all computed toolpaths.
    pub fn export_gcode(&self, path: &Path, _setup_id: Option<usize>) -> Result<(), SessionError> {
        use crate::gcode::{CoolantMode, GcodePhase, PostFormat, emit_gcode_phased};

        let post_format = match self.post.format.to_ascii_lowercase().as_str() {
            "linuxcnc" | "linux_cnc" => PostFormat::LinuxCnc,
            "mach3" => PostFormat::Mach3,
            _ => PostFormat::Grbl,
        };
        let post = post_format.post_processor();

        // Collect all computed toolpaths as phases
        let mut phases: Vec<GcodePhase<'_>> = Vec::new();
        for (idx, tc) in self.toolpath_configs.iter().enumerate() {
            if let Some(result) = self.results.get(&idx) {
                phases.push(GcodePhase {
                    toolpath: &result.toolpath,
                    spindle_rpm: self.post.spindle_speed,
                    label: &tc.name,
                    tool_number: None,
                    coolant: CoolantMode::Off,
                    pre_gcode: None,
                    post_gcode: None,
                });
            }
        }

        let gcode = emit_gcode_phased(&phases, post.as_ref());
        std::fs::write(path, gcode).map_err(|e| {
            SessionError::Export(format!("Failed to write G-code to {}: {e}", path.display()))
        })
    }

    /// Export diagnostics as JSON files to an output directory.
    pub fn export_diagnostics_json(&self, output_dir: &Path) -> Result<(), SessionError> {
        std::fs::create_dir_all(output_dir)?;
        let diag = self.diagnostics();
        let json = serde_json::to_string_pretty(&diag)
            .map_err(|e| SessionError::Export(format!("Failed to serialize diagnostics: {e}")))?;
        let path = output_dir.join("summary.json");
        std::fs::write(&path, json)
            .map_err(|e| SessionError::Export(format!("Failed to write {}: {e}", path.display())))
    }

    // ── Internal helpers ───────────────────────────────────────────

    fn find_tool_by_raw_id(&self, raw_id: usize) -> Option<&ToolConfig> {
        self.tools
            .iter()
            .find(|t| t.id.0 == raw_id)
            .or_else(|| self.tools.first())
    }

    fn find_model_by_raw_id(&self, raw_id: usize) -> Option<&LoadedModel> {
        self.models
            .iter()
            .find(|m| m.id == raw_id)
            .or_else(|| self.models.first())
    }

    fn find_setup_for_toolpath_index(&self, tp_index: usize) -> Option<&SetupData> {
        self.setups
            .iter()
            .find(|s| s.toolpath_indices.contains(&tp_index))
    }

    fn effective_stock_bbox(&self, face_up: FaceUp) -> BoundingBox3 {
        let bbox = self.stock.bbox();
        match face_up {
            FaceUp::Bottom => {
                // For bottom-up setups, use a local frame
                BoundingBox3 {
                    min: P3::new(0.0, 0.0, 0.0),
                    max: P3::new(self.stock.x, self.stock.y, self.stock.z),
                }
            }
            _ => bbox,
        }
    }
}

// ── Free functions ─────────────────────────────────────────────────────

fn stock_from_project(ps: &ProjectStockConfig) -> StockConfig {
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
        alignment_pins: Vec::new(),
        flip_axis: None,
        workholding_rigidity: crate::feeds::WorkholdingRigidity::Medium,
    }
}

fn parse_tool_type(s: &str) -> ToolType {
    match s.to_ascii_lowercase().as_str() {
        "ball_nose" | "ballnose" => ToolType::BallNose,
        "bull_nose" | "bullnose" => ToolType::BullNose,
        "v_bit" | "vbit" => ToolType::VBit,
        "tapered_ball_nose" | "taperedballnose" => ToolType::TaperedBallNose,
        _ => ToolType::EndMill,
    }
}

fn tool_from_project_section(ts: &ProjectToolSection, idx: usize) -> ToolConfig {
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

fn load_model_geometry(
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

// ── Serde for ProjectDiagnostics (for JSON export) ─────────────────────

impl serde::Serialize for ToolpathDiagnostic {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut s = serializer.serialize_struct("ToolpathDiagnostic", 9)?;
        s.serialize_field("toolpath_id", &self.toolpath_id)?;
        s.serialize_field("name", &self.name)?;
        s.serialize_field("operation_type", &self.operation_type)?;
        s.serialize_field("tool_name", &self.tool_name)?;
        s.serialize_field("move_count", &self.move_count)?;
        s.serialize_field("cutting_distance_mm", &self.cutting_distance_mm)?;
        s.serialize_field("rapid_distance_mm", &self.rapid_distance_mm)?;
        s.serialize_field("collision_count", &self.collision_count)?;
        s.serialize_field("rapid_collision_count", &self.rapid_collision_count)?;
        s.end()
    }
}

impl serde::Serialize for ProjectDiagnostics {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut s = serializer.serialize_struct("ProjectDiagnostics", 7)?;
        s.serialize_field("total_runtime_s", &self.total_runtime_s)?;
        s.serialize_field("air_cut_percentage", &self.air_cut_percentage)?;
        s.serialize_field("average_engagement", &self.average_engagement)?;
        s.serialize_field("collision_count", &self.collision_count)?;
        s.serialize_field("rapid_collision_count", &self.rapid_collision_count)?;
        s.serialize_field("per_toolpath", &self.per_toolpath)?;
        s.serialize_field("verdict", &self.verdict)?;
        s.end()
    }
}

// ── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;

    #[test]
    fn empty_project_loads() {
        let project = ProjectFile {
            format_version: 3,
            job: ProjectJobSection::default(),
            tools: Vec::new(),
            models: Vec::new(),
            setups: Vec::new(),
            toolpaths: Vec::new(),
        };
        let session = ProjectSession::from_project_file(project, Path::new(".")).unwrap();
        // Default derives empty string; "Untitled" only comes from serde deserialization
        assert!(session.name().is_empty());
        assert_eq!(session.toolpath_count(), 0);
        assert_eq!(session.setup_count(), 0);
        assert!(session.list_toolpaths().is_empty());
        assert!(session.list_tools().is_empty());
    }

    #[test]
    fn stock_bbox_from_defaults() {
        let project = ProjectFile {
            format_version: 3,
            job: ProjectJobSection::default(),
            tools: Vec::new(),
            models: Vec::new(),
            setups: Vec::new(),
            toolpaths: Vec::new(),
        };
        let session = ProjectSession::from_project_file(project, Path::new(".")).unwrap();
        let bbox = session.stock_bbox();
        assert!((bbox.max.x - bbox.min.x - 100.0).abs() < 1e-6);
        assert!((bbox.max.y - bbox.min.y - 100.0).abs() < 1e-6);
        assert!((bbox.max.z - bbox.min.z - 25.0).abs() < 1e-6);
    }

    #[test]
    fn diagnostics_empty_session() {
        let project = ProjectFile {
            format_version: 3,
            job: ProjectJobSection::default(),
            tools: Vec::new(),
            models: Vec::new(),
            setups: Vec::new(),
            toolpaths: Vec::new(),
        };
        let session = ProjectSession::from_project_file(project, Path::new(".")).unwrap();
        let diag = session.diagnostics();
        assert_eq!(diag.verdict, "OK");
        assert!(diag.per_toolpath.is_empty());
    }

    #[test]
    fn tool_type_parsing() {
        assert!(matches!(parse_tool_type("end_mill"), ToolType::EndMill));
        assert!(matches!(parse_tool_type("ball_nose"), ToolType::BallNose));
        assert!(matches!(parse_tool_type("bull_nose"), ToolType::BullNose));
        assert!(matches!(parse_tool_type("v_bit"), ToolType::VBit));
        assert!(matches!(
            parse_tool_type("tapered_ball_nose"),
            ToolType::TaperedBallNose
        ));
        assert!(matches!(parse_tool_type("unknown"), ToolType::EndMill));
    }
}
