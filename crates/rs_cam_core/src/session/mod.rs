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

mod compute;
mod mutation;
pub mod project_file;
mod save;

// Re-export all public project_file types so external crates see no path change.
pub use project_file::{
    ProjectFile, ProjectFixtureSection, ProjectJobSection, ProjectKeepOutSection,
    ProjectModelSection, ProjectPostConfig, ProjectSetupSection, ProjectStockConfig,
    ProjectToolSection, ProjectToolpathSection,
};

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use crate::compute::catalog::OperationConfig;
use crate::compute::config::{
    BoundaryConfig, DressupConfig, FeedsAutoMode, HeightsConfig, StockSource, ToolpathStats,
};
use crate::compute::simulate::SimulationResult;
use crate::compute::stock_config::{FixtureId, KeepOutId, ModelKind, ModelUnits, StockConfig};
use crate::compute::tool_config::{ToolConfig, ToolId, ToolType};
use crate::compute::transform::{FaceUp, ZRotation};
use crate::debug_trace::{ToolpathDebugOptions, ToolpathDebugTrace};
use crate::enriched_mesh::{EnrichedMesh, FaceGroupId};
use crate::gcode::CoolantMode;
use crate::geo::{BoundingBox3, P3};
use crate::mesh::TriangleMesh;
use crate::polygon::Polygon2;
use crate::semantic_trace::ToolpathSemanticTrace;
use crate::toolpath::Toolpath;

use crate::compute::collision_check::CollisionCheckError;
use crate::compute::simulate::SimulationError;

// ── Error types ────────────────────────────────────────────────────────

/// Errors that can occur during session operations.
#[derive(Debug)]
pub enum SessionError {
    /// I/O error (file not found, permission denied, etc.).
    Io(std::io::Error),
    /// TOML parsing error.
    TomlParse(String),
    /// TOML serialization error.
    TomlSerialize(String),
    /// Model loading failure.
    ModelLoad { name: String, detail: String },
    /// Toolpath not found by index.
    ToolpathNotFound(usize),
    /// Tool not found by id.
    ToolNotFound(ToolId),
    /// Setup not found by index.
    SetupNotFound(usize),
    /// Tool still referenced by toolpaths — cannot remove.
    ToolInUse(ToolId),
    /// Setup still has toolpaths — cannot remove.
    SetupHasToolpaths(usize),
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
    /// Invalid parameter name or value.
    InvalidParam(String),
}

impl std::fmt::Display for SessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error: {e}"),
            Self::TomlParse(e) => write!(f, "TOML parse error: {e}"),
            Self::TomlSerialize(e) => write!(f, "TOML serialize error: {e}"),
            Self::ModelLoad { name, detail } => {
                write!(f, "Failed to load model '{name}': {detail}")
            }
            Self::ToolpathNotFound(id) => write!(f, "Toolpath {id} not found"),
            Self::ToolNotFound(id) => write!(f, "Tool {} not found", id.0),
            Self::SetupNotFound(id) => write!(f, "Setup {id} not found"),
            Self::ToolInUse(id) => write!(f, "Tool {} is still referenced by toolpaths", id.0),
            Self::SetupHasToolpaths(id) => {
                write!(f, "Setup {id} still has toolpaths — remove them first")
            }
            Self::MissingGeometry(msg) => write!(f, "Missing geometry: {msg}"),
            Self::OperationFailed(msg) => write!(f, "Operation failed: {msg}"),
            Self::Simulation(e) => write!(f, "Simulation error: {e}"),
            Self::CollisionCheck(e) => write!(f, "Collision check error: {e}"),
            Self::Export(msg) => write!(f, "Export error: {msg}"),
            Self::InvalidParam(msg) => write!(f, "Invalid parameter: {msg}"),
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

// ── Loaded state types ─────────────────────────────────────────────────

/// Geometry loaded from a model file.
pub(crate) enum LoadedGeometry {
    Mesh(TriangleMesh),
    Polygons(Vec<Polygon2>),
}

/// A loaded model with its geometry.
pub struct LoadedModel {
    pub id: usize,
    pub name: String,
    pub mesh: Option<Arc<TriangleMesh>>,
    pub polygons: Option<Arc<Vec<Polygon2>>>,
    /// Original file path (for save round-trip).
    pub path: std::path::PathBuf,
    /// File kind (stl, svg, dxf, step).
    pub kind: Option<ModelKind>,
    /// Assumed units of the source file (determines scale factor to mm).
    pub units: Option<ModelUnits>,
    /// Enriched mesh with BREP face groups (for STEP/CAD models).
    pub enriched_mesh: Option<Arc<EnrichedMesh>>,
    /// Percentage of inconsistent winding edges. `None` if not STL.
    pub winding_report: Option<f64>,
    /// Load/import failure preserved so broken references can round-trip.
    pub load_error: Option<String>,
}

/// Kind of workholding fixture (compute-relevant subset of viz `FixtureKind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum FixtureKind {
    #[default]
    Clamp,
    Vise,
    VacuumPod,
    Custom,
}

impl FixtureKind {
    pub fn from_key(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "vise" => Self::Vise,
            "vacuum_pod" | "vacuumpod" => Self::VacuumPod,
            "custom" => Self::Custom,
            _ => Self::Clamp,
        }
    }
}

/// A physical workholding fixture — compute-relevant fields only.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Fixture {
    pub id: FixtureId,
    pub name: String,
    pub kind: FixtureKind,
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Position of the fixture's min corner in workpiece coordinates (mm).
    #[serde(default)]
    pub origin_x: f64,
    #[serde(default)]
    pub origin_y: f64,
    #[serde(default)]
    pub origin_z: f64,
    /// Dimensions of the fixture bounding box (mm).
    #[serde(default = "default_fixture_size")]
    pub size_x: f64,
    #[serde(default = "default_fixture_size")]
    pub size_y: f64,
    #[serde(default = "default_fixture_height")]
    pub size_z: f64,
    /// Extra clearance around the fixture for tool avoidance (mm).
    #[serde(default = "default_fixture_clearance")]
    pub clearance: f64,
}

fn default_true() -> bool {
    true
}
fn default_fixture_size() -> f64 {
    30.0
}
fn default_fixture_height() -> f64 {
    20.0
}
fn default_fixture_clearance() -> f64 {
    3.0
}

impl Fixture {
    /// XY footprint polygon (with clearance) for boundary subtraction.
    pub fn footprint(&self) -> Polygon2 {
        let min_x = self.origin_x - self.clearance;
        let min_y = self.origin_y - self.clearance;
        let max_x = self.origin_x + self.size_x + self.clearance;
        let max_y = self.origin_y + self.size_y + self.clearance;
        Polygon2::rectangle(min_x, min_y, max_x, max_y)
    }
}

/// A rectangular region the tool must avoid (XY only, full Z extent).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct KeepOutZone {
    pub id: KeepOutId,
    pub name: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Position of the zone's min corner (mm).
    #[serde(default)]
    pub origin_x: f64,
    #[serde(default)]
    pub origin_y: f64,
    /// Dimensions of the zone (mm).
    #[serde(default = "default_keep_out_size")]
    pub size_x: f64,
    #[serde(default = "default_keep_out_size")]
    pub size_y: f64,
}

fn default_keep_out_size() -> f64 {
    20.0
}

impl KeepOutZone {
    /// XY footprint polygon for boundary subtraction.
    pub fn footprint(&self) -> Polygon2 {
        Polygon2::rectangle(
            self.origin_x,
            self.origin_y,
            self.origin_x + self.size_x,
            self.origin_y + self.size_y,
        )
    }
}

/// A setup's orientation and toolpath indices.
pub struct SetupData {
    pub id: usize,
    pub name: String,
    pub face_up: FaceUp,
    /// Rotation of the stock about the vertical (Z) axis.
    pub z_rotation: ZRotation,
    /// Workholding fixtures in this setup.
    pub fixtures: Vec<Fixture>,
    /// Keep-out zones in this setup.
    pub keep_out_zones: Vec<KeepOutZone>,
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
    /// Raw G-code to emit before this toolpath's moves.
    pub pre_gcode: Option<String>,
    /// Raw G-code to emit after this toolpath's moves.
    pub post_gcode: Option<String>,
    /// Machining boundary configuration.
    pub boundary: BoundaryConfig,
    /// When true, inherit boundary from stock default.
    pub boundary_inherit: bool,
    /// Where this toolpath's stock material comes from.
    pub stock_source: StockSource,
    /// Coolant mode for G-code output.
    pub coolant: CoolantMode,
    /// Optional BREP face selection (for STEP/CAD models).
    pub face_selection: Option<Vec<FaceGroupId>>,
    /// Tracks which feed parameters are auto-calculated vs user-overridden.
    pub feeds_auto: FeedsAutoMode,
    /// Debug trace options for this toolpath.
    pub debug_options: ToolpathDebugOptions,
}

/// Result of generating a single toolpath.
pub struct ToolpathComputeResult {
    pub toolpath: Arc<Toolpath>,
    pub stats: ToolpathStats,
    pub debug_trace: Option<ToolpathDebugTrace>,
    pub semantic_trace: Option<ToolpathSemanticTrace>,
}

/// Summary of a toolpath for listing.
#[derive(serde::Serialize)]
pub struct ToolpathSummary {
    pub index: usize,
    pub id: usize,
    pub name: String,
    pub operation_label: String,
    pub enabled: bool,
    pub tool_name: String,
}

/// Summary of a tool for listing.
#[derive(serde::Serialize)]
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
    /// When `true`, override `resolution` with an auto-computed value based
    /// on the smallest tool radius and the stock footprint (matching the GUI's
    /// auto-resolution logic).
    pub auto_resolution: bool,
}

impl Default for SimulationOptions {
    fn default() -> Self {
        Self {
            resolution: 0.5,
            skip_ids: Vec::new(),
            metrics_enabled: true,
            auto_resolution: false,
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
    pub(crate) name: String,
    pub(crate) stock: StockConfig,
    pub(crate) post: ProjectPostConfig,
    pub(crate) machine: crate::machine::MachineProfile,

    // Loaded state
    pub(crate) models: Vec<LoadedModel>,
    pub(crate) tools: Vec<ToolConfig>,
    pub(crate) setups: Vec<SetupData>,
    pub(crate) toolpath_configs: Vec<ToolpathConfig>,

    // Computed results (keyed by toolpath index)
    pub(crate) results: HashMap<usize, ToolpathComputeResult>,
    pub(crate) simulation: Option<SimulationResult>,

    // ID generators (max existing ID + 1)
    pub(crate) next_toolpath_id: usize,
    pub(crate) next_tool_id: usize,
    pub(crate) next_setup_id: usize,
    pub(crate) next_model_id: usize,
}

impl ProjectSession {
    // ── Lifecycle ───────────────────────────────────────────────────

    /// Create an empty session (for untitled / new projects).
    pub fn new_empty() -> Self {
        Self {
            name: String::new(),
            stock: StockConfig::default(),
            post: ProjectPostConfig::default(),
            machine: crate::machine::MachineProfile::default(),
            models: Vec::new(),
            tools: Vec::new(),
            setups: vec![SetupData {
                id: 0,
                name: "Setup 1".to_owned(),
                face_up: FaceUp::default(),
                z_rotation: ZRotation::default(),
                fixtures: Vec::new(),
                keep_out_zones: Vec::new(),
                toolpath_indices: Vec::new(),
            }],
            toolpath_configs: Vec::new(),
            results: HashMap::new(),
            simulation: None,
            next_toolpath_id: 0,
            next_tool_id: 0,
            next_setup_id: 1,
            next_model_id: 0,
        }
    }

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
        project_file::build_session_from_project(project, base_dir)
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

    /// Machine profile.
    pub fn machine(&self) -> &crate::machine::MachineProfile {
        &self.machine
    }

    /// All loaded tools.
    pub fn tools(&self) -> &[ToolConfig] {
        &self.tools
    }

    /// All loaded models.
    pub fn models(&self) -> &[LoadedModel] {
        &self.models
    }

    /// All toolpath configurations.
    pub fn toolpath_configs(&self) -> &[ToolpathConfig] {
        &self.toolpath_configs
    }

    // ── Mutable accessors ─────────────────────────────────────────

    /// Mutable access to stock configuration.
    pub fn stock_mut(&mut self) -> &mut StockConfig {
        &mut self.stock
    }

    /// Mutable access to machine profile.
    pub fn machine_mut(&mut self) -> &mut crate::machine::MachineProfile {
        &mut self.machine
    }

    /// Mutable access to all tools.
    pub fn tools_mut(&mut self) -> &mut Vec<ToolConfig> {
        &mut self.tools
    }

    /// Mutable access to all loaded models.
    pub fn models_mut(&mut self) -> &mut Vec<LoadedModel> {
        &mut self.models
    }

    /// Mutable access to post-processor configuration.
    pub fn post_mut(&mut self) -> &mut ProjectPostConfig {
        &mut self.post
    }

    /// Replace the project name.
    pub fn set_name(&mut self, name: String) {
        self.name = name;
    }

    // ── ID lookup helpers ─────────────────────────────────────────

    /// Find a toolpath config by its semantic ID (not vec index).
    pub fn find_toolpath_config_by_id(&self, id: usize) -> Option<(usize, &ToolpathConfig)> {
        self.toolpath_configs
            .iter()
            .enumerate()
            .find(|(_, tc)| tc.id == id)
    }

    /// Find a mutable toolpath config by its semantic ID.
    pub fn find_toolpath_config_by_id_mut(
        &mut self,
        id: usize,
    ) -> Option<(usize, &mut ToolpathConfig)> {
        self.toolpath_configs
            .iter_mut()
            .enumerate()
            .find(|(_, tc)| tc.id == id)
    }

    /// Find which setup (by index) owns a toolpath with the given semantic ID.
    pub fn setup_of_toolpath_id(&self, tp_id: usize) -> Option<usize> {
        let tp_index = self
            .toolpath_configs
            .iter()
            .position(|tc| tc.id == tp_id)?;
        self.setups
            .iter()
            .position(|s| s.toolpath_indices.contains(&tp_index))
    }

    /// Find a setup by its semantic ID (not vec index).
    pub fn find_setup_by_id(&self, id: usize) -> Option<(usize, &SetupData)> {
        self.setups
            .iter()
            .enumerate()
            .find(|(_, s)| s.id == id)
    }

    /// Find a mutable setup by its semantic ID.
    pub fn find_setup_by_id_mut(&mut self, id: usize) -> Option<(usize, &mut SetupData)> {
        self.setups
            .iter_mut()
            .enumerate()
            .find(|(_, s)| s.id == id)
    }

    /// Collect all toolpath semantic IDs.
    pub fn all_toolpath_ids(&self) -> Vec<usize> {
        self.toolpath_configs.iter().map(|tc| tc.id).collect()
    }

    /// Mutable access to all toolpath configs.
    pub fn toolpath_configs_mut(&mut self) -> &mut Vec<ToolpathConfig> {
        &mut self.toolpath_configs
    }

    /// Mutable access to all setups.
    pub fn setups_mut(&mut self) -> &mut Vec<SetupData> {
        &mut self.setups
    }

    // ── Internal helpers ───────────────────────────────────────────

    pub(crate) fn find_tool_by_raw_id(&self, raw_id: usize) -> Option<&ToolConfig> {
        self.tools
            .iter()
            .find(|t| t.id.0 == raw_id)
            .or_else(|| self.tools.first())
    }

    pub(crate) fn find_model_by_raw_id(&self, raw_id: usize) -> Option<&LoadedModel> {
        self.models
            .iter()
            .find(|m| m.id == raw_id)
            .or_else(|| self.models.first())
    }

    pub(crate) fn find_setup_for_toolpath_index(&self, tp_index: usize) -> Option<&SetupData> {
        self.setups
            .iter()
            .find(|s| s.toolpath_indices.contains(&tp_index))
    }

    /// Compute the stock bounding box in setup-local coordinates, accounting for
    /// both face-up orientation and Z rotation.
    pub(crate) fn effective_stock_bbox_with_rotation(
        &self,
        face_up: FaceUp,
        z_rotation: ZRotation,
    ) -> BoundingBox3 {
        let (eff_w, eff_d, eff_h) = {
            let (w, d, h) = face_up.effective_stock(self.stock.x, self.stock.y, self.stock.z);
            z_rotation.effective_stock(w, d, h)
        };
        // Setup-local frame always has origin at (0,0,0).
        BoundingBox3 {
            min: P3::new(0.0, 0.0, 0.0),
            max: P3::new(eff_w, eff_d, eff_h),
        }
    }

    // ── Geometry transforms for setup-local frame ────────────────

    /// Transform a 3D point from global/world coordinates to setup-local frame.
    ///
    /// The transform chain is:
    /// 1. Translate from world to stock-relative (origin at 0,0,0)
    /// 2. Apply face-up flip on stock-relative coordinates
    /// 3. Apply Z rotation
    fn transform_point_to_setup(&self, p: P3, face_up: FaceUp, z_rotation: ZRotation) -> P3 {
        // 1. Translate world -> stock-relative
        let rel = P3::new(
            p.x - self.stock.origin_x,
            p.y - self.stock.origin_y,
            p.z - self.stock.origin_z,
        );
        // 2. Apply FaceUp flip
        let flipped = face_up.transform_point(rel, self.stock.x, self.stock.y, self.stock.z);
        // 3. Apply ZRotation
        let (eff_w, eff_d, _) = face_up.effective_stock(self.stock.x, self.stock.y, self.stock.z);
        z_rotation.transform_point(flipped, eff_w, eff_d)
    }

    /// Inverse transform: from setup-local frame back to global/world coordinates.
    ///
    /// Undoes ZRotation, then FaceUp, then translates back to world coords.
    pub fn inverse_transform_point_from_setup(
        &self,
        p: P3,
        face_up: FaceUp,
        z_rotation: ZRotation,
    ) -> P3 {
        // 1. Undo ZRotation
        let (eff_w, eff_d, _) = face_up.effective_stock(self.stock.x, self.stock.y, self.stock.z);
        let unrotated = z_rotation.inverse_transform_point(p, eff_w, eff_d);
        // 2. Undo FaceUp flip -> stock-relative
        let rel =
            face_up.inverse_transform_point(unrotated, self.stock.x, self.stock.y, self.stock.z);
        // 3. Translate stock-relative -> world
        P3::new(
            rel.x + self.stock.origin_x,
            rel.y + self.stock.origin_y,
            rel.z + self.stock.origin_z,
        )
    }

    /// Transform a triangle mesh from global to setup-local coordinates.
    pub(crate) fn transform_mesh_to_setup(
        &self,
        mesh: &TriangleMesh,
        face_up: FaceUp,
        z_rotation: ZRotation,
    ) -> TriangleMesh {
        let new_verts: Vec<P3> = mesh
            .vertices
            .iter()
            .map(|v| self.transform_point_to_setup(*v, face_up, z_rotation))
            .collect();
        TriangleMesh::from_raw(new_verts, mesh.triangles.clone())
    }

    /// Transform 2D polygons from global to setup-local XY coordinates.
    pub(crate) fn transform_polygons_to_setup(
        &self,
        polygons: &[Polygon2],
        face_up: FaceUp,
        z_rotation: ZRotation,
    ) -> Vec<Polygon2> {
        use crate::geo::P2;
        polygons
            .iter()
            .map(|poly| {
                let ext: Vec<P2> = poly
                    .exterior
                    .iter()
                    .map(|p| {
                        let p3 = self.transform_point_to_setup(
                            P3::new(p.x, p.y, 0.0),
                            face_up,
                            z_rotation,
                        );
                        P2::new(p3.x, p3.y)
                    })
                    .collect();
                let holes: Vec<Vec<P2>> = poly
                    .holes
                    .iter()
                    .map(|hole| {
                        hole.iter()
                            .map(|p| {
                                let p3 = self.transform_point_to_setup(
                                    P3::new(p.x, p.y, 0.0),
                                    face_up,
                                    z_rotation,
                                );
                                P2::new(p3.x, p3.y)
                            })
                            .collect()
                    })
                    .collect();
                Polygon2::with_holes(ext, holes)
            })
            .collect()
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
    use super::project_file::parse_tool_type;
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

    /// Create a session with one tool and one Pocket toolpath for mutation tests.
    fn session_with_toolpath() -> ProjectSession {
        use crate::compute::catalog::OperationConfig;
        let project = ProjectFile {
            format_version: 3,
            job: ProjectJobSection::default(),
            tools: vec![ProjectToolSection {
                id: Some(0),
                name: "Test EndMill".to_owned(),
                tool_type: "end_mill".to_owned(),
                diameter: 6.35,
                cutting_length: 25.0,
                corner_radius: 2.0,
                included_angle: 90.0,
                taper_half_angle: 15.0,
                shaft_diameter: 6.35,
                holder_diameter: 25.0,
                shank_diameter: 6.35,
                shank_length: 20.0,
                stickout: 45.0,
                flute_count: 2,
                tool_number: None,
                tool_material: "carbide".to_owned(),
                cut_direction: "up_cut".to_owned(),
                vendor: String::new(),
                product_id: String::new(),
            }],
            models: Vec::new(),
            setups: vec![ProjectSetupSection {
                id: Some(0),
                name: "Setup 1".to_owned(),
                face_up: "top".to_owned(),
                z_rotation: String::new(),
                fixtures: Vec::new(),
                keep_out_zones: Vec::new(),
                toolpaths: vec![ProjectToolpathSection {
                    id: Some(0),
                    name: "Test Pocket".to_owned(),
                    operation: Some(OperationConfig::new_default(
                        crate::compute::catalog::OperationType::Pocket,
                    )),
                    enabled: true,
                    tool_id: Some(0),
                    model_id: Some(0),
                    dressups: crate::compute::config::DressupConfig::default(),
                    heights: crate::compute::config::HeightsConfig::default(),
                    pre_gcode: None,
                    post_gcode: None,
                    boundary: crate::compute::config::BoundaryConfig::default(),
                    boundary_inherit: true,
                    stock_source: crate::compute::config::StockSource::default(),
                    coolant: crate::gcode::CoolantMode::default(),
                    face_selection: None,
                    feeds_auto: crate::compute::config::FeedsAutoMode::default(),
                    debug_options: crate::debug_trace::ToolpathDebugOptions::default(),
                }],
            }],
            toolpaths: Vec::new(),
        };
        ProjectSession::from_project_file(project, Path::new(".")).unwrap()
    }

    #[test]
    fn set_common_param_feed_rate() {
        let mut session = session_with_toolpath();
        let original = session.toolpath_configs[0].operation.feed_rate();

        session
            .set_toolpath_param(0, "feed_rate", serde_json::json!(1500.0))
            .unwrap();

        let updated = session.toolpath_configs[0].operation.feed_rate();
        assert!(
            (updated - 1500.0).abs() < 1e-6,
            "feed_rate should be 1500.0, got {updated} (was {original})"
        );
    }

    #[test]
    fn set_config_specific_param() {
        let mut session = session_with_toolpath();

        // Pocket has a config-specific "angle" parameter
        session
            .set_toolpath_param(0, "angle", serde_json::json!(45.0))
            .unwrap();

        // Verify it changed via serde round-trip
        let json = serde_json::to_value(&session.toolpath_configs[0].operation).unwrap();
        let angle = json["params"]["angle"].as_f64().unwrap();
        assert!(
            (angle - 45.0).abs() < 1e-6,
            "angle should be 45.0, got {angle}"
        );
    }

    #[test]
    fn invalid_param_name_returns_error() {
        let mut session = session_with_toolpath();

        let result =
            session.set_toolpath_param(0, "nonexistent_param_xyz", serde_json::json!(42.0));

        assert!(result.is_err(), "Should fail for unknown param name");
        assert!(
            matches!(result.unwrap_err(), SessionError::InvalidParam(_)),
            "Should be InvalidParam error"
        );
    }

    #[test]
    fn set_tool_param_diameter() {
        let mut session = session_with_toolpath();

        session
            .set_tool_param(0, "diameter", &serde_json::json!(10.0))
            .unwrap();

        let updated = session.tools[0].diameter;
        assert!(
            (updated - 10.0).abs() < 1e-6,
            "diameter should be 10.0, got {updated}"
        );
    }

    #[test]
    fn set_toolpath_param_invalidates_cached_result() {
        let mut session = session_with_toolpath();

        // Manually insert a fake cached result
        session.results.insert(
            0,
            ToolpathComputeResult {
                toolpath: std::sync::Arc::new(crate::toolpath::Toolpath::new()),
                stats: crate::compute::config::ToolpathStats::default(),
                debug_trace: None,
                semantic_trace: None,
            },
        );
        assert!(
            session.results.contains_key(&0),
            "Precondition: result cached"
        );

        session
            .set_toolpath_param(0, "feed_rate", serde_json::json!(2000.0))
            .unwrap();

        assert!(
            !session.results.contains_key(&0),
            "Cached result should be invalidated after set_toolpath_param"
        );
    }

    // ── CRUD mutation tests ───────────────────────────────────────

    #[test]
    fn add_toolpath_then_list() {
        use crate::compute::catalog::{OperationConfig, OperationType};

        let mut session = session_with_toolpath();
        assert_eq!(session.toolpath_count(), 1);

        let new_tp = ToolpathConfig {
            id: 0, // will be overwritten by add_toolpath
            name: "New Profile".to_owned(),
            enabled: true,
            operation: OperationConfig::new_default(OperationType::Profile),
            dressups: crate::compute::config::DressupConfig::default(),
            heights: crate::compute::config::HeightsConfig::default(),
            tool_id: 0,
            model_id: 0,
            pre_gcode: None,
            post_gcode: None,
            boundary: crate::compute::config::BoundaryConfig::default(),
            boundary_inherit: true,
            stock_source: crate::compute::config::StockSource::default(),
            coolant: crate::gcode::CoolantMode::default(),
            face_selection: None,
            feeds_auto: crate::compute::config::FeedsAutoMode::default(),
            debug_options: crate::debug_trace::ToolpathDebugOptions::default(),
        };

        let idx = session.add_toolpath(0, new_tp).unwrap();
        assert_eq!(idx, 1);
        assert_eq!(session.toolpath_count(), 2);

        let summaries = session.list_toolpaths();
        assert_eq!(summaries.len(), 2);
        assert_eq!(summaries[1].name, "New Profile");
    }

    #[test]
    fn remove_toolpath_then_list() {
        let mut session = session_with_toolpath();
        assert_eq!(session.toolpath_count(), 1);

        session.remove_toolpath(0).unwrap();
        assert_eq!(session.toolpath_count(), 0);
        assert!(session.list_toolpaths().is_empty());

        // Setup should have no more toolpath indices
        assert!(session.setups[0].toolpath_indices.is_empty());
    }

    #[test]
    fn save_reload_roundtrip() {
        let session = session_with_toolpath();
        let dir = std::env::temp_dir().join("rs_cam_test_roundtrip");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("roundtrip_test.toml");

        // Save
        session.save(&path).unwrap();

        // Reload
        let content = std::fs::read_to_string(&path).unwrap();
        let reloaded_project: ProjectFile = toml::from_str(&content).unwrap();
        let reloaded = ProjectSession::from_project_file(reloaded_project, Path::new(".")).unwrap();

        // Verify key state matches
        assert_eq!(reloaded.name(), session.name());
        assert_eq!(reloaded.toolpath_count(), session.toolpath_count());
        assert_eq!(reloaded.setup_count(), session.setup_count());
        assert_eq!(reloaded.list_tools().len(), session.list_tools().len());

        // Stock dimensions
        let orig_bbox = session.stock_bbox();
        let reload_bbox = reloaded.stock_bbox();
        assert!((orig_bbox.max.x - reload_bbox.max.x).abs() < 1e-6);
        assert!((orig_bbox.max.y - reload_bbox.max.y).abs() < 1e-6);
        assert!((orig_bbox.max.z - reload_bbox.max.z).abs() < 1e-6);

        // Toolpath name preserved
        let orig_tps = session.list_toolpaths();
        let reload_tps = reloaded.list_toolpaths();
        assert_eq!(orig_tps[0].name, reload_tps[0].name);
        assert_eq!(orig_tps[0].enabled, reload_tps[0].enabled);

        // Cleanup
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn set_stock_invalidates_simulation() {
        use crate::compute::simulate::SimulationResult;
        use crate::stock_mesh::StockMesh;

        let mut session = session_with_toolpath();

        // Manually set a fake simulation result
        session.simulation = Some(SimulationResult {
            mesh: StockMesh {
                vertices: Vec::new(),
                indices: Vec::new(),
                colors: Vec::new(),
            },
            total_moves: 0,
            deviations: None,
            boundaries: Vec::new(),
            checkpoints: Vec::new(),
            rapid_collisions: Vec::new(),
            rapid_collision_move_indices: Vec::new(),
            cut_trace: None,
            resolution_clamped: false,
        });
        assert!(
            session.simulation_result().is_some(),
            "Precondition: simulation present"
        );

        let new_stock = StockConfig {
            x: 200.0,
            y: 200.0,
            z: 50.0,
            ..StockConfig::default()
        };
        session.set_stock_config(new_stock);

        assert!(
            session.simulation_result().is_none(),
            "Simulation should be invalidated after set_stock_config"
        );
        assert!((session.stock_config().x - 200.0).abs() < 1e-6);
    }
}
