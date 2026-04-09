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
pub mod project_file;

// Re-export all public project_file types so external crates see no path change.
pub use project_file::{
    ProjectFile, ProjectJobSection, ProjectModelSection, ProjectPostConfig, ProjectSetupSection,
    ProjectStockConfig, ProjectToolSection, ProjectToolpathSection,
};

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use crate::compute::catalog::OperationConfig;
use crate::compute::config::{DressupConfig, HeightsConfig, ToolpathStats};
use crate::compute::simulate::SimulationResult;
use crate::compute::stock_config::StockConfig;
use crate::compute::tool_config::{ToolConfig, ToolId, ToolType};
use crate::compute::transform::FaceUp;
use crate::debug_trace::ToolpathDebugTrace;
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
    /// Invalid parameter name or value.
    InvalidParam(String),
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
    /// Raw G-code to emit before this toolpath's moves.
    pub pre_gcode: Option<String>,
    /// Raw G-code to emit after this toolpath's moves.
    pub post_gcode: Option<String>,
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
    pub(crate) name: String,
    pub(crate) stock: StockConfig,
    pub(crate) post: ProjectPostConfig,
    #[allow(dead_code)] // Will be used in Phase 5+ (CLI/MCP wiring)
    pub(crate) machine: crate::machine::MachineProfile,

    // Loaded state
    pub(crate) models: Vec<LoadedModel>,
    pub(crate) tools: Vec<ToolConfig>,
    pub(crate) setups: Vec<SetupData>,
    pub(crate) toolpath_configs: Vec<ToolpathConfig>,

    // Computed results (keyed by toolpath index)
    pub(crate) results: HashMap<usize, ToolpathComputeResult>,
    pub(crate) simulation: Option<SimulationResult>,
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

    pub(crate) fn effective_stock_bbox(&self, face_up: FaceUp) -> BoundingBox3 {
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
    use super::project_file::parse_tool_type;

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
            }],
            models: Vec::new(),
            setups: vec![ProjectSetupSection {
                id: Some(0),
                name: "Setup 1".to_owned(),
                face_up: "top".to_owned(),
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

        let result = session.set_toolpath_param(
            0,
            "nonexistent_param_xyz",
            serde_json::json!(42.0),
        );

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
        assert!(session.results.contains_key(&0), "Precondition: result cached");

        session
            .set_toolpath_param(0, "feed_rate", serde_json::json!(2000.0))
            .unwrap();

        assert!(
            !session.results.contains_key(&0),
            "Cached result should be invalidated after set_toolpath_param"
        );
    }
}
