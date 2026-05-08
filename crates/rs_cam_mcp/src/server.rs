//! MCP tool definitions wrapping `ProjectSession`.

use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex as TokioMutex;

use rmcp::handler::server::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::ServerInfo;
use rmcp::schemars;
use rmcp::{tool, tool_router, ServerHandler};
use serde::Deserialize;

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::config::{
    BoundaryConfig, BoundaryContainment, BoundarySource, DressupConfig,
};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::compute::transform::FaceUp;
use rs_cam_core::session::{ProjectSession, SimulationOptions};

// ── Parameter structs ─────────────────────────────────────────────────

#[allow(dead_code)]
#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct AddSetupParam {
    /// Optional name for the new setup
    pub name: Option<String>,
}

#[allow(dead_code)]
#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct SetSetupFaceParam {
    /// Setup index (0-based)
    pub setup_index: usize,
    /// Face orientation: "top", "bottom", "front", "back", "left", "right"
    pub face_up: String,
}

#[allow(dead_code)]
#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct MoveToolpathToSetupParam {
    /// Toolpath index (0-based, global across all setups)
    pub toolpath_index: usize,
    /// Target setup index (0-based)
    pub target_setup_index: usize,
}

#[allow(dead_code)] // Used by rs_cam_viz embedded MCP, not the standalone binary
#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct ImportModelParam {
    /// File path to import. Supported formats: .stl, .dxf, .svg, .step/.stp
    pub path: String,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct LoadProjectParam {
    /// Path to the project TOML file
    pub path: String,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct IndexParam {
    /// Toolpath index (0-based)
    pub index: usize,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct SimulationParam {
    /// Simulation resolution in mm (default 0.5)
    pub resolution: Option<f64>,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct ExportParam {
    /// Output file path for G-code
    pub path: String,
    /// Bypass the tool-load gate for criteria that returned `Unmodeled`
    /// (e.g. no simulation run, no vendor data). Default false.
    #[serde(default)]
    pub accept_unmodeled_tool_load: bool,
    /// Bypass the tool-load gate for criteria that returned `Exceeds`
    /// (the toolpath is predicted to break or burn the tool). Default
    /// false. Set this knowingly; it is the "I accept that this toolpath
    /// will damage tooling" override and is recorded separately from
    /// `accept_unmodeled_tool_load`.
    #[serde(default)]
    pub accept_exceeded_tool_load: bool,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct ScreenshotSimParam {
    /// Output file path (.png for 6-view composite, .html for interactive 3D)
    pub path: String,
    /// Image width in pixels (default 1200, PNG only)
    pub width: Option<u32>,
    /// Image height in pixels (default 800, PNG only)
    pub height: Option<u32>,
    /// Checkpoint index to render (default: last). Each toolpath produces one
    /// checkpoint. Use a lower index to see intermediate states.
    pub checkpoint: Option<usize>,
    /// Include toolpath overlay lines (HTML only, default true)
    pub include_toolpaths: Option<bool>,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct ScreenshotToolpathParam {
    /// Toolpath index (0-based)
    pub index: usize,
    /// Output file path (.png for 6-view composite, .html for interactive 3D)
    pub path: String,
    /// Image width in pixels (default 1200, PNG only)
    pub width: Option<u32>,
    /// Image height in pixels (default 800, PNG only)
    pub height: Option<u32>,
    /// Show machined stock as dimmed background context (default false, PNG only).
    /// Requires simulation to have been run first.
    pub show_stock: Option<bool>,
    /// Include rapid moves in the rendering (default true, PNG only)
    pub include_rapids: Option<bool>,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct SetToolpathParamInput {
    /// Toolpath index (0-based)
    pub index: usize,
    /// Parameter name (e.g. "feed_rate", "stepover", "depth_per_pass", "plunge_rate",
    /// or any config-specific field like "angle", "min_z", "passes")
    pub param: String,
    /// New value (numeric)
    pub value: serde_json::Value,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct OptimizeToolpathInput {
    /// Toolpath index (0-based)
    pub index: usize,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct SetToolParamInput {
    /// Tool index (0-based)
    pub index: usize,
    /// Parameter name (e.g. "diameter", "flute_count", "stickout", "corner_radius",
    /// "cutting_length", "shaft_diameter", "shank_diameter", "shank_length", "holder_diameter")
    pub param: String,
    /// New value (numeric)
    pub value: serde_json::Value,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct CollisionCheckParam {
    /// Toolpath index (0-based)
    pub index: usize,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct CutTraceParam {
    /// Optional: filter results to a single toolpath by index
    pub toolpath_id: Option<usize>,
    /// Maximum hotspots to return (default: 20)
    pub max_hotspots: Option<usize>,
    /// Maximum issues to return (default: 50)
    pub max_issues: Option<usize>,
    /// Optional: only include samples/issues/hotspots whose `span_path` contains
    /// a span of this kind. Accepted values match `SpanKind`:
    /// "operation", "depth_pass", "region", "entry", "lead_out", "link_bridge",
    /// "dressup_artifact", "rapid_order_barrier".
    /// Read by the embedded GUI MCP; standalone CLI MCP currently ignores it.
    #[allow(dead_code)]
    pub span_kind: Option<String>,
    /// Optional: only include samples/issues/hotspots whose `span_path` contains
    /// this exact span id. SpanIds come from `inspect_spans`. Read by the
    /// embedded GUI MCP; standalone CLI MCP currently ignores it.
    #[allow(dead_code)]
    pub span_id: Option<u32>,
    /// Optional: only include samples/issues/hotspots whose `span_path` contains
    /// a `DepthPass` span with this `pass_index` payload value (0-based).
    /// Read by the embedded GUI MCP; standalone CLI MCP currently ignores it.
    #[allow(dead_code)]
    pub pass_index: Option<u32>,
}

#[allow(dead_code)] // Used by rs_cam_viz embedded MCP, not the standalone binary
#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct InspectSpansParam {
    /// Toolpath index (0-based). Must have been generated first.
    pub index: usize,
    /// Optional `SpanKind` filter (snake_case). Accepted values:
    /// "operation", "depth_pass", "region", "entry", "lead_out",
    /// "link_bridge", "dressup_artifact", "rapid_order_barrier".
    pub kind: Option<String>,
    /// Optional parent span id (vec index). Restricts results to spans whose
    /// move range is contained within the parent's range. Pair with `kind` to
    /// drill from "Operation 0" → its DepthPasses → a single DepthPass's Regions.
    pub parent_id: Option<u32>,
    /// Optional `DepthPass` `pass_index` payload match (0-based).
    pub pass_index: Option<u32>,
    /// Optional `Region` `region_id` payload match.
    pub region_id: Option<u32>,
    /// Hard cap on returned spans (default 50). Result includes `truncated`
    /// and `total_matching` when capped.
    pub max_spans: Option<usize>,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct GenDebugTraceParam {
    /// Toolpath index (0-based). Must have been generated first.
    pub index: usize,
    /// Optional filter. Accepts EITHER a generation-debug kind string
    /// ("z_level", "adaptive_pass", "entry_search", "preflight", "pre_stamp",
    /// "widen_band", "waterline_cleanup") OR a structural `SpanKind` synonym
    /// in snake_case ("depth_pass" → matches z_level/adaptive_pass/z_level_clear,
    /// "entry" → matches entry_search). Omit to include all kinds. Each
    /// returned span carries a `span_kind_hint` field that maps the debug
    /// kind back to its structural SpanKind when one applies.
    pub span_kind: Option<String>,
    /// Optional filter: only include spans whose exit_reason contains this
    /// substring. Useful values for AgentSearch diagnosis:
    /// "loop closed", "idle", "no entry", "preflight skip", "no viable direction".
    pub exit_reason: Option<String>,
    /// Optional filter: only include spans where the `yield_ratio` counter is
    /// at most this value (e.g. 0.1 to see all low-yield passes). Spans without
    /// a `yield_ratio` counter are skipped when this filter is set.
    pub max_yield_ratio: Option<f64>,
    /// Maximum span count in the response (default: 100). Set to 0 for unlimited.
    pub max_spans: Option<usize>,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct AddToolpathParam {
    /// Setup index (0-based) to add the toolpath into
    pub setup_index: usize,
    /// Operation type (e.g. "pocket", "adaptive3d", "drop_cutter", "profile")
    pub operation_type: String,
    /// Tool index (0-based) from list_tools
    pub tool_index: usize,
    /// Model ID (raw numeric ID shown in toolpath configs, usually 0 for the first model)
    pub model_id: usize,
    /// Optional name for the toolpath
    pub name: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct RemoveToolpathParam {
    /// Toolpath index (0-based)
    pub index: usize,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct AddToolParam {
    /// Display name for the tool
    pub name: String,
    /// Tool type (e.g. "end_mill", "ball_nose", "bull_nose", "v_bit", "tapered_ball_nose")
    pub tool_type: String,
    /// Tool diameter in mm
    pub diameter: f64,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct RemoveToolParam {
    /// Tool index (0-based)
    pub index: usize,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct SetStockConfigParam {
    /// Stock width (X) in mm
    pub x: f64,
    /// Stock depth (Y) in mm
    pub y: f64,
    /// Stock height (Z) in mm
    pub z: f64,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct SetBoundaryConfigParam {
    /// Toolpath index (0-based)
    pub index: usize,
    /// Enable or disable boundary
    pub enabled: bool,
    /// Boundary source: "stock" or "model_silhouette"
    pub source: Option<String>,
    /// Containment mode: "center", "inside", or "outside"
    pub containment: Option<String>,
    /// Additional offset in mm (positive = expand, negative = shrink)
    pub offset: Option<f64>,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct SetDressupConfigParam {
    /// Toolpath index (0-based)
    pub index: usize,
    /// Dressup configuration as a JSON object (fields match DressupConfig)
    pub dressup: serde_json::Value,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct SetDressupFieldParam {
    /// Toolpath index (0-based)
    pub index: usize,
    /// Dressup field name (e.g. "link_moves", "arc_fitting", "retract_strategy")
    pub key: String,
    /// New value for the field (JSON)
    pub value: serde_json::Value,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct SetToolpathEnabledParam {
    /// Toolpath index (0-based)
    pub index: usize,
    /// `true` to enable, `false` to disable
    pub enabled: bool,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct SetStockSourceParam {
    /// Toolpath index (0-based)
    pub index: usize,
    /// Either "fresh" or "from_remaining_stock"
    pub source: String,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct SaveProjectParam {
    /// File path to save the project TOML to (required)
    pub path: String,
}

/// Model ID parameter (used by embedded MCP server in rs_cam_viz).
#[derive(Deserialize, schemars::JsonSchema, Default)]
#[allow(dead_code)]
pub struct ModelIdParam {
    /// Model ID (0-based)
    pub model_id: usize,
}

/// Add alignment pin parameter (used by embedded MCP server in rs_cam_viz).
#[derive(Deserialize, schemars::JsonSchema, Default)]
#[allow(dead_code)]
pub struct AddAlignmentPinParam {
    /// X position of the alignment pin in mm
    pub x: f64,
    /// Y position of the alignment pin in mm
    pub y: f64,
    /// Diameter of the alignment pin in mm
    pub diameter: f64,
}

/// Remove alignment pin parameter (used by embedded MCP server in rs_cam_viz).
#[derive(Deserialize, schemars::JsonSchema, Default)]
#[allow(dead_code)]
pub struct RemoveAlignmentPinParam {
    /// Index of the alignment pin to remove (0-based)
    pub index: usize,
}

/// Simulation jump-to-move parameter (used by embedded MCP server in rs_cam_viz).
#[derive(Deserialize, schemars::JsonSchema, Default)]
#[allow(dead_code)]
pub struct SimJumpToMoveParam {
    /// Move index to jump to (0-based, up to total_moves)
    pub move_index: usize,
}

/// Per-toolpath percentage-based simulation scrub parameter.
#[derive(Deserialize, schemars::JsonSchema, Default)]
#[allow(dead_code)]
pub struct SimScrubToolpathParam {
    /// Toolpath index (0-based)
    pub index: usize,
    /// Position within this toolpath as percentage (0.0 = start, 100.0 = end)
    pub percent: f64,
}

/// Jump to the start or end of a specific toolpath in the simulation.
#[derive(Deserialize, schemars::JsonSchema, Default)]
#[allow(dead_code)]
pub struct SimJumpToToolpathBoundaryParam {
    /// Toolpath index (0-based)
    pub index: usize,
}

/// Parse a string into an `OperationType` (snake_case).
pub fn parse_operation_type(s: &str) -> Result<OperationType, String> {
    serde_json::from_value(serde_json::Value::String(s.to_owned()))
        .map_err(|_| format!("Unknown operation type '{s}'. Valid types: face, pocket, profile, adaptive, v_carve, rest, inlay, zigzag, trace, drill, chamfer, drop_cutter, adaptive3d, waterline, pencil, scallop, steep_shallow, ramp_finish, spiral_finish, radial_finish, horizontal_finish, project_curve, alignment_pin_drill"))
}

/// Parse a string into a `ToolType` (snake_case).
pub fn parse_tool_type(s: &str) -> Result<ToolType, String> {
    serde_json::from_value(serde_json::Value::String(s.to_owned()))
        .map_err(|_| format!("Unknown tool type '{s}'. Valid types: end_mill, ball_nose, bull_nose, v_bit, tapered_ball_nose"))
}

pub fn text(msg: impl Into<String>) -> String {
    msg.into()
}

pub fn json_str(data: serde_json::Value) -> String {
    serde_json::to_string_pretty(&data).unwrap_or_else(|e| format!("{{\"error\": \"{e}\"}}"))
}

/// Standardized error response when no project is loaded.
pub fn no_project_error() -> String {
    json_str(serde_json::json!({"error": "No project loaded. Call load_project first."}))
}

// ── Server ────────────────────────────────────────────────────────────

/// MCP server that exposes rs_cam's ProjectSession as tools.
#[derive(Clone)]
pub struct CamServer {
    session: Arc<TokioMutex<Option<ProjectSession>>>,
    #[allow(dead_code)]
    tool_router: ToolRouter<Self>,
}

impl CamServer {
    pub fn new(session: Arc<TokioMutex<Option<ProjectSession>>>) -> Self {
        let tool_router = Self::tool_router();
        Self {
            session,
            tool_router,
        }
    }

    pub fn into_tool_router() -> ToolRouter<Self> {
        Self::tool_router()
    }
}

#[tool_router]
impl CamServer {
    #[tool(
        name = "load_project",
        description = "Load a project TOML file. Must be called before other tools if no project was specified on startup."
    )]
    async fn load_project(
        &self,
        Parameters(LoadProjectParam { path }): Parameters<LoadProjectParam>,
    ) -> String {
        match ProjectSession::load(Path::new(&path)) {
            Ok(session) => {
                let name = session.name().to_owned();
                let tp_count = session.toolpath_count();
                let setup_count = session.setup_count();
                *self.session.lock().await = Some(session);
                text(format!(
                    "Loaded '{name}' — {setup_count} setups, {tp_count} toolpaths"
                ))
            }
            Err(e) => text(format!("Failed to load: {e}")),
        }
    }

    #[tool(
        name = "project_summary",
        description = "Get project summary: name, stock dimensions, setup count, toolpath count, tools"
    )]
    async fn project_summary(&self) -> String {
        let guard = self.session.lock().await;
        let Some(session) = guard.as_ref() else {
            return no_project_error();
        };
        let bbox = session.stock_bbox();
        json_str(serde_json::json!({
            "name": session.name(),
            "stock": {
                "width": bbox.max.x - bbox.min.x,
                "depth": bbox.max.y - bbox.min.y,
                "height": bbox.max.z - bbox.min.z,
            },
            "setup_count": session.setup_count(),
            "toolpath_count": session.toolpath_count(),
            "tools": session.list_tools(),
        }))
    }

    #[tool(
        name = "list_toolpaths",
        description = "List all toolpaths with name, operation type, enabled status, and tool"
    )]
    async fn list_toolpaths(&self) -> String {
        let guard = self.session.lock().await;
        let Some(session) = guard.as_ref() else {
            return no_project_error();
        };
        json_str(serde_json::to_value(session.list_toolpaths()).unwrap_or_default())
    }

    #[tool(
        name = "list_tools",
        description = "List all tools with type and dimensions"
    )]
    async fn list_tools(&self) -> String {
        let guard = self.session.lock().await;
        let Some(session) = guard.as_ref() else {
            return no_project_error();
        };
        json_str(serde_json::to_value(session.list_tools()).unwrap_or_default())
    }

    #[tool(
        name = "get_toolpath_params",
        description = "Get operation parameters for a toolpath by index"
    )]
    async fn get_toolpath_params(
        &self,
        Parameters(IndexParam { index }): Parameters<IndexParam>,
    ) -> String {
        let guard = self.session.lock().await;
        let Some(session) = guard.as_ref() else {
            return no_project_error();
        };
        match session.get_toolpath_config(index) {
            Some(tc) => {
                let op_value =
                    serde_json::to_value(&tc.operation).unwrap_or_else(|_| serde_json::json!({}));
                json_str(serde_json::json!({
                    "id": tc.id,
                    "name": tc.name,
                    "enabled": tc.enabled,
                    "tool_id": tc.tool_id,
                    "model_id": tc.model_id,
                    "operation": op_value,
                }))
            }
            None => {
                json_str(serde_json::json!({"error": format!("Toolpath index {index} not found")}))
            }
        }
    }

    #[tool(
        name = "generate_toolpath",
        description = "Generate a single toolpath by index. Returns move count and distances."
    )]
    async fn generate_toolpath(
        &self,
        Parameters(IndexParam { index }): Parameters<IndexParam>,
    ) -> String {
        let session = Arc::clone(&self.session);
        let result = tokio::task::spawn_blocking(move || {
            let cancel = std::sync::atomic::AtomicBool::new(false);
            let mut guard = session.blocking_lock();
            let Some(s) = guard.as_mut() else {
                return Err("No project loaded. Call load_project first.".to_owned());
            };
            s.generate_toolpath(index, &cancel)
                .map(|r| {
                    serde_json::json!({
                        "index": index,
                        "move_count": r.stats.move_count,
                        "cutting_distance_mm": r.stats.cutting_distance,
                        "rapid_distance_mm": r.stats.rapid_distance,
                    })
                })
                .map_err(|e| e.to_string())
        })
        .await;

        match result {
            Ok(Ok(v)) => json_str(v),
            Ok(Err(e)) => json_str(serde_json::json!({"error": e})),
            Err(e) => json_str(serde_json::json!({"error": format!("Task failed: {e}")})),
        }
    }

    #[tool(
        name = "generate_all",
        description = "Generate all enabled toolpaths. Returns count of newly generated toolpaths."
    )]
    async fn generate_all(&self) -> String {
        let session = Arc::clone(&self.session);
        let result = tokio::task::spawn_blocking(move || {
            let cancel = std::sync::atomic::AtomicBool::new(false);
            let mut guard = session.blocking_lock();
            let Some(s) = guard.as_mut() else {
                return Err("No project loaded. Call load_project first.".to_owned());
            };
            let before: usize = (0..s.toolpath_count())
                .filter(|i| s.get_result(*i).is_some())
                .count();
            s.generate_all(&[], &cancel).map_err(|e| e.to_string())?;
            let after: usize = (0..s.toolpath_count())
                .filter(|i| s.get_result(*i).is_some())
                .count();
            Ok::<_, String>(after - before)
        })
        .await;

        match result {
            Ok(Ok(n)) => text(format!("Generated {n} toolpaths")),
            Ok(Err(e)) => text(format!("Error: {e}")),
            Err(e) => text(format!("Task failed: {e}")),
        }
    }

    #[tool(
        name = "run_simulation",
        description = "Run tri-dexel stock simulation. Returns air cutting %, engagement, collisions, and verdict."
    )]
    async fn run_simulation(
        &self,
        Parameters(SimulationParam { resolution }): Parameters<SimulationParam>,
    ) -> String {
        let session = Arc::clone(&self.session);
        let res = resolution.unwrap_or(0.5);
        let result = tokio::task::spawn_blocking(move || {
            let cancel = std::sync::atomic::AtomicBool::new(false);
            let opts = SimulationOptions {
                resolution: res,
                skip_ids: Vec::new(),
                metrics_enabled: true,
                auto_resolution: false,
            };
            let mut guard = session.blocking_lock();
            let Some(s) = guard.as_mut() else {
                return Err("No project loaded. Call load_project first.".to_owned());
            };
            s.run_simulation(&opts, &cancel)
                .map_err(|e| e.to_string())?;
            let diag = s.diagnostics();
            let mut resp = serde_json::json!({
                "total_runtime_s": diag.total_runtime_s,
                "air_cut_percentage": diag.air_cut_percentage,
                "average_engagement": diag.average_engagement,
                "collision_count": diag.collision_count,
                "rapid_collision_count": diag.rapid_collision_count,
                "verdict": diag.verdict,
                "per_toolpath": diag.per_toolpath,
            });
            if let Some(sim) = s.simulation_result() {
                if let Some(ct) = sim.cut_trace.as_ref() {
                    // SAFETY: resp is a known JSON object we just constructed
                    #[allow(clippy::indexing_slicing)]
                    {
                        resp["semantic_summary_count"] =
                            serde_json::json!(ct.semantic_summaries.len());
                        resp["hotspot_count"] = serde_json::json!(ct.hotspots.len());
                        resp["issue_count"] = serde_json::json!(ct.issues.len());
                    }
                }
            }
            Ok::<_, String>(resp)
        })
        .await;

        match result {
            Ok(Ok(v)) => json_str(v),
            Ok(Err(e)) => json_str(serde_json::json!({"error": e})),
            Err(e) => json_str(serde_json::json!({"error": format!("Task failed: {e}")})),
        }
    }

    #[tool(
        name = "get_diagnostics",
        description = "Get project diagnostics: per-toolpath stats, collision counts, air cutting %, verdict"
    )]
    async fn get_diagnostics(&self) -> String {
        let guard = self.session.lock().await;
        let Some(session) = guard.as_ref() else {
            return no_project_error();
        };
        json_str(serde_json::to_value(session.diagnostics()).unwrap_or_default())
    }

    #[tool(
        name = "export_gcode",
        description = "Export G-code to a file path. Refuses if any toolpath has tool-load Exceeds or Unmodeled verdicts unless the corresponding accept flag is set."
    )]
    async fn export_gcode(
        &self,
        Parameters(ExportParam {
            path,
            accept_unmodeled_tool_load,
            accept_exceeded_tool_load,
        }): Parameters<ExportParam>,
    ) -> String {
        let guard = self.session.lock().await;
        let Some(session) = guard.as_ref() else {
            return no_project_error();
        };
        let policy = rs_cam_core::gcode::ToolLoadExportPolicy {
            accept_unmodeled: accept_unmodeled_tool_load,
            accept_exceeded: accept_exceeded_tool_load,
        };
        match session.export_gcode_with_policy(Path::new(&path), None, policy) {
            Ok(()) => text(format!("G-code exported to {path}")),
            Err(e) => text(format!("Export failed: {e}")),
        }
    }

    #[tool(
        name = "get_tool_load_report",
        description = "Per-toolpath tool-load report: chipload, power, deflection verdicts. Each criterion is independent (no scalar load %). Verdicts are Within/Exceeds/Unmodeled with typed reasons."
    )]
    async fn get_tool_load_report(&self) -> String {
        let guard = self.session.lock().await;
        let Some(session) = guard.as_ref() else {
            return no_project_error();
        };
        let report = session.tool_load_report();
        match serde_json::to_value(&report) {
            Ok(v) => json_str(v),
            Err(e) => json_str(serde_json::json!({
                "error": format!("Failed to serialize tool load report: {e}")
            })),
        }
    }

    #[tool(
        name = "optimize_toolpath",
        description = "Run the optimizer on one toolpath. Searches across feed/RPM (Stage F: analytical headroom-up for safe baselines, RCTF-compensated re-target for chipload-Exceeds baselines) and DOC × stepover variants (Stage 1/2 sims). Each candidate is sim-verified end-to-end. Returns OptimizeOutcome JSON, one of: Ranked(candidates) — at least one candidate is faster AND has no gate regression (auto-recommendation); TradeOff(candidates) — at least one candidate is faster AND improves the failing baseline gate but worsens another (user must explicitly accept); NoSafeImprovement(reason, explanation, attempted) — pre-flight refusal (BipolarEngagement, DeflectionSetupLocked) or no candidate improved over baseline; Skipped(reason) — toolpath is unmodelable (Drill, Custom material, etc.). Each non-baseline candidate carries gate_deltas (chipload/power/deflection: improved/same/worsened/unmodeled) so consumers can render trade-offs without recomputing. Long-running — the call blocks until the search completes (~1-2 min for a 3D op, faster for 2D). Run simulation first; the optimizer scores candidates against the existing baseline trace."
    )]
    async fn optimize_toolpath(
        &self,
        #[allow(clippy::needless_pass_by_value)]
        Parameters(OptimizeToolpathInput { index }): Parameters<OptimizeToolpathInput>,
    ) -> String {
        let mut guard = self.session.lock().await;
        let Some(session) = guard.as_mut() else {
            return no_project_error();
        };
        // Pull the trace from the session's cached sim. Same gate the
        // GUI applies — Optimize requires a baseline.
        let trace_clone = session
            .simulation_result()
            .and_then(|r| r.cut_trace.clone());
        let Some(trace) = trace_clone else {
            return json_str(serde_json::json!({
                "error": "Run a simulation first — optimize_toolpath needs a baseline trace.",
            }));
        };
        let cancel = std::sync::atomic::AtomicBool::new(false);
        let outcome =
            rs_cam_core::tool_load::optimize::optimize_toolpath(session, &trace, index, &cancel);
        match serde_json::to_value(&outcome) {
            Ok(v) => json_str(v),
            Err(e) => json_str(serde_json::json!({
                "error": format!("Failed to serialize optimize outcome: {e}")
            })),
        }
    }

    #[tool(
        name = "set_toolpath_param",
        description = "Set a toolpath parameter. Common params: feed_rate, plunge_rate, stepover, depth_per_pass. Config-specific params vary by operation type. Marks the toolpath as stale — regenerate to apply."
    )]
    async fn set_toolpath_param(
        &self,
        #[allow(clippy::needless_pass_by_value)] Parameters(SetToolpathParamInput {
            index,
            param,
            value,
        }): Parameters<SetToolpathParamInput>,
    ) -> String {
        let mut guard = self.session.lock().await;
        let Some(session) = guard.as_mut() else {
            return no_project_error();
        };
        match session.set_toolpath_param(index, &param, value) {
            Ok(()) => text(format!(
                "Set toolpath {index} param '{param}'. Regenerate to apply."
            )),
            Err(e) => text(format!("Error: {e}")),
        }
    }

    #[tool(
        name = "set_tool_param",
        description = "Set a tool parameter (e.g. diameter, flute_count, stickout, corner_radius). Invalidates all toolpaths using this tool — regenerate to apply."
    )]
    async fn set_tool_param(
        &self,
        #[allow(clippy::needless_pass_by_value)] Parameters(SetToolParamInput {
            index,
            param,
            value,
        }): Parameters<SetToolParamInput>,
    ) -> String {
        let mut guard = self.session.lock().await;
        let Some(session) = guard.as_mut() else {
            return no_project_error();
        };
        match session.set_tool_param(index, &param, &value) {
            Ok(()) => text(format!(
                "Set tool {index} param '{param}'. Regenerate affected toolpaths to apply."
            )),
            Err(e) => text(format!("Error: {e}")),
        }
    }

    #[tool(
        name = "screenshot_simulation",
        description = "Export simulated stock as a 6-view composite PNG (default) or interactive 3D HTML. Run simulation first. Use .png for agent-viewable images, .html for interactive browser views."
    )]
    async fn screenshot_simulation(
        &self,
        #[allow(clippy::needless_pass_by_value)] Parameters(ScreenshotSimParam {
            path,
            width,
            height,
            checkpoint,
            include_toolpaths,
        }): Parameters<ScreenshotSimParam>,
    ) -> String {
        let guard = self.session.lock().await;
        let Some(session) = guard.as_ref() else {
            return no_project_error();
        };
        let Some(sim) = session.simulation_result() else {
            return text("No simulation result. Run run_simulation first.");
        };

        if path.ends_with(".png") {
            let w = width.unwrap_or(1200);
            let h = height.unwrap_or(800);
            // Render from checkpoint's dexel stock (same pipeline as GUI).
            // Default to last checkpoint; fall back to composite mesh.
            let cp_idx = checkpoint.unwrap_or_else(|| sim.checkpoints.len().saturating_sub(1));
            let pixels = if let Some(cp) = sim.checkpoints.get(cp_idx) {
                rs_cam_core::fingerprint::render_stock_composite(&cp.stock, w, h)
            } else {
                rs_cam_core::fingerprint::render_mesh_composite(&sim.mesh, w, h)
            };
            match image::save_buffer(Path::new(&path), &pixels, w, h, image::ColorType::Rgba8) {
                Ok(()) => text(format!(
                    "6-view composite exported to {path} ({w}x{h}, checkpoint {cp_idx})",
                )),
                Err(e) => text(format!("Failed to save PNG: {e}")),
            }
        } else {
            let toolpaths: Vec<&rs_cam_core::toolpath::Toolpath> =
                if include_toolpaths.unwrap_or(true) {
                    (0..session.toolpath_count())
                        .filter_map(|i| session.get_result(i).map(|r| r.toolpath()))
                        .collect()
                } else {
                    Vec::new()
                };

            let html = rs_cam_core::viz::stock_mesh_to_3d_html(
                &sim.mesh,
                &toolpaths,
                &format!("{} — Simulation", session.name()),
            );

            match std::fs::write(&path, &html) {
                Ok(()) => text(format!(
                    "Simulation view exported to {path} ({} vertices, {} triangles)",
                    sim.mesh.vertex_count(),
                    sim.mesh.indices.len() / 3,
                )),
                Err(e) => text(format!("Failed to write: {e}")),
            }
        }
    }

    #[tool(
        name = "screenshot_toolpath",
        description = "Export a single generated toolpath as a 6-view composite PNG or interactive 3D HTML. Green = cutting, orange = rapid. Use show_stock=true to overlay on dimmed machined stock for context."
    )]
    async fn screenshot_toolpath(
        &self,
        #[allow(clippy::needless_pass_by_value)] Parameters(ScreenshotToolpathParam {
            index,
            path,
            width,
            height,
            show_stock,
            include_rapids,
        }): Parameters<ScreenshotToolpathParam>,
    ) -> String {
        let guard = self.session.lock().await;
        let Some(session) = guard.as_ref() else {
            return no_project_error();
        };
        let Some(result) = session.get_result(index) else {
            return text(format!(
                "Toolpath {index} not generated. Run generate_toolpath first."
            ));
        };

        if path.ends_with(".png") {
            let w = width.unwrap_or(1200);
            let h = height.unwrap_or(800);
            let bg = if show_stock.unwrap_or(false) {
                session.simulation_result().map(|sim| {
                    let mut m = sim.mesh.clone();
                    m.apply_height_gradient();
                    m
                })
            } else {
                None
            };
            let pixels = rs_cam_core::fingerprint::render_toolpath_composite(
                result.toolpath(),
                bg.as_ref(),
                w,
                h,
                include_rapids.unwrap_or(true),
            );
            match image::save_buffer(Path::new(&path), &pixels, w, h, image::ColorType::Rgba8) {
                Ok(()) => text(format!(
                    "Toolpath {index} exported to {path} ({w}x{h}, {} moves, {:.0}mm cutting)",
                    result.toolpath().moves.len(),
                    result.stats.cutting_distance,
                )),
                Err(e) => text(format!("Failed to save PNG: {e}")),
            }
        } else {
            let bbox = session.stock_bbox();
            let bounds = [
                bbox.min.x, bbox.min.y, bbox.min.z, bbox.max.x, bbox.max.y, bbox.max.z,
            ];
            let html =
                rs_cam_core::viz::toolpath_standalone_3d_html(result.toolpath(), Some(bounds));

            match std::fs::write(&path, &html) {
                Ok(()) => text(format!(
                    "Toolpath view exported to {path} ({} moves, {:.0}mm cutting)",
                    result.toolpath().moves.len(),
                    result.stats.cutting_distance,
                )),
                Err(e) => text(format!("Failed to write: {e}")),
            }
        }
    }

    #[tool(
        name = "collision_check",
        description = "Run a holder/shank collision check for a specific toolpath. Requires the toolpath to be generated first. Returns collision count, positions, and minimum safe stickout."
    )]
    async fn collision_check(
        &self,
        Parameters(CollisionCheckParam { index }): Parameters<CollisionCheckParam>,
    ) -> String {
        let session = Arc::clone(&self.session);
        let result = tokio::task::spawn_blocking(move || {
            let cancel = std::sync::atomic::AtomicBool::new(false);
            let guard = session.blocking_lock();
            let Some(s) = guard.as_ref() else {
                return Err("No project loaded. Call load_project first.".to_owned());
            };
            s.collision_check(index, &cancel)
                .map(|r| {
                    serde_json::json!({
                        "index": index,
                        "collision_count": r.collision_report.collisions.len(),
                        "min_safe_stickout_mm": r.collision_report.min_safe_stickout,
                        "is_clear": r.collision_report.is_clear(),
                        "collision_positions": r.collision_positions,
                    })
                })
                .map_err(|e| e.to_string())
        })
        .await;

        match result {
            Ok(Ok(v)) => json_str(v),
            Ok(Err(e)) => json_str(serde_json::json!({"error": e})),
            Err(e) => json_str(serde_json::json!({"error": format!("Task failed: {e}")})),
        }
    }

    #[tool(
        name = "narrate_toolpath",
        description = "Return a concise prose narration of one generated toolpath: Z-level structure, perimeter-sweep estimates, suspicious large arcs, peak axial DOC, and air-cut percentage. Prefer this first for agent debugging before raw traces/screenshots. Run generate_toolpath first; run_simulation first for DOC/air-cut metrics."
    )]
    async fn narrate_toolpath(
        &self,
        Parameters(IndexParam { index }): Parameters<IndexParam>,
    ) -> String {
        let guard = self.session.lock().await;
        let Some(session) = guard.as_ref() else {
            return no_project_error();
        };
        match session.narrate_toolpath(index) {
            Ok(report) => text(report),
            Err(e) => text(format!("Error: {e}")),
        }
    }

    #[tool(
        name = "get_cut_trace",
        description = "Get simulation cut trace data: semantic summaries, hotspots, and issues. Run simulation first. Use toolpath_id to filter to a single toolpath."
    )]
    async fn get_cut_trace(
        &self,
        #[allow(clippy::needless_pass_by_value)] Parameters(CutTraceParam {
            toolpath_id,
            max_hotspots,
            max_issues,
            // Standalone CLI MCP path doesn't yet wire spans through
            // ToolpathComputeResult (S1.5); span filters are accepted but
            // produce no extra filtering until that path lifts to
            // AnnotatedToolpath.
            span_kind: _,
            span_id: _,
            pass_index: _,
        }): Parameters<CutTraceParam>,
    ) -> String {
        let guard = self.session.lock().await;
        let Some(session) = guard.as_ref() else {
            return no_project_error();
        };
        let Some(sim) = session.simulation_result() else {
            return json_str(
                serde_json::json!({"error": "No simulation result. Run run_simulation first."}),
            );
        };
        let Some(ct) = sim.cut_trace.as_ref() else {
            return json_str(
                serde_json::json!({"error": "No cut trace data. Run simulation with a loaded project."}),
            );
        };

        let max_h = max_hotspots.unwrap_or(20);
        let max_i = max_issues.unwrap_or(50);

        let summaries: Vec<&_> = ct
            .semantic_summaries
            .iter()
            .filter(|s| toolpath_id.is_none_or(|id| s.toolpath_id == id))
            .collect();
        let hotspots: Vec<&_> = ct
            .hotspots
            .iter()
            .filter(|h| toolpath_id.is_none_or(|id| h.toolpath_id == id))
            .collect();
        let issues: Vec<&_> = ct
            .issues
            .iter()
            .filter(|i| toolpath_id.is_none_or(|id| i.toolpath_id == id))
            .collect();

        let hotspot_count = hotspots.len();
        let issue_count = issues.len();

        let summaries_val =
            serde_json::to_value(&summaries).unwrap_or_else(|_| serde_json::json!([]));
        let hotspots_val: Vec<_> = hotspots.iter().take(max_h).collect();
        let hotspots_val =
            serde_json::to_value(&hotspots_val).unwrap_or_else(|_| serde_json::json!([]));
        let issues_val: Vec<_> = issues.iter().take(max_i).collect();
        let issues_val =
            serde_json::to_value(&issues_val).unwrap_or_else(|_| serde_json::json!([]));
        let summary_val =
            serde_json::to_value(&ct.summary).unwrap_or_else(|_| serde_json::json!({}));

        json_str(serde_json::json!({
            "summary": summary_val,
            "semantic_summaries": summaries_val,
            "hotspots": hotspots_val,
            "hotspot_count": hotspot_count,
            "issue_count": issue_count,
            "issues": issues_val,
        }))
    }

    /// Retrieve the generation-time debug trace for a toolpath.
    ///
    /// Unlike `get_cut_trace` (which reports simulation-time metrics),
    /// this tool exposes the `ToolpathDebugTrace` captured by the
    /// operation generator. For adaptive3d with clearing_strategy =
    /// AgentSearch it's the primary diagnostic surface for wandering
    /// and looping: every pass records its exit_reason, step_count,
    /// idle_count, yield_ratio, and xy_bbox on the per-pass span.
    ///
    /// See planning/agent_search_diagnosis_plan.md (or Phase 3 of the
    /// April 2026 adaptive remediation series) for the diagnostic
    /// rubric.
    #[tool(
        name = "get_generation_debug_trace",
        description = "Get the generation-time debug trace for a toolpath: per-pass spans with exit_reason, idle_count, yield_ratio, xy_bbox, plus a diagnostic summary. Primary surface for diagnosing adaptive3d AgentSearch wandering/looping. Run generate_toolpath first. Filter by span_kind, exit_reason, or max_yield_ratio to narrow the response."
    )]
    async fn get_generation_debug_trace(
        &self,
        #[allow(clippy::needless_pass_by_value)] Parameters(GenDebugTraceParam {
            index,
            span_kind,
            exit_reason,
            max_yield_ratio,
            max_spans,
        }): Parameters<GenDebugTraceParam>,
    ) -> String {
        let guard = self.session.lock().await;
        let Some(session) = guard.as_ref() else {
            return no_project_error();
        };
        let Some(result) = session.get_result(index) else {
            return json_str(serde_json::json!({
                "error": format!("Toolpath {index} not generated. Run generate_toolpath first.")
            }));
        };
        let Some(trace) = result.debug_trace.as_ref() else {
            return json_str(serde_json::json!({
                "error": format!("Toolpath {index} has no debug trace — the operation generator didn't capture one.")
            }));
        };

        // Apply filters.
        let limit = max_spans.unwrap_or(100);
        let filtered: Vec<_> = trace
            .spans
            .iter()
            .filter(|s| span_kind.as_deref().is_none_or(|k| s.kind == k))
            .filter(|s| {
                exit_reason.as_deref().is_none_or(|needle| {
                    s.exit_reason.as_deref().is_some_and(|r| r.contains(needle))
                })
            })
            .filter(|s| {
                max_yield_ratio
                    .is_none_or(|max_y| s.counters.get("yield_ratio").is_some_and(|&y| y <= max_y))
            })
            .collect();
        let total_matching = filtered.len();
        let visible: Vec<_> = if limit == 0 {
            filtered.clone()
        } else {
            filtered.iter().copied().take(limit).collect()
        };

        // Diagnostic summary: aggregate adaptive_pass spans by exit_reason
        // and compute aggregate wandering/looping indicators.
        let pass_spans: Vec<_> = trace
            .spans
            .iter()
            .filter(|s| s.kind == "adaptive_pass")
            .collect();
        let mut passes_by_exit: std::collections::BTreeMap<String, usize> =
            std::collections::BTreeMap::new();
        let mut yield_sum = 0.0f64;
        let mut yield_count = 0usize;
        let mut low_yield_passes = 0usize;
        let mut looped_passes = 0usize;
        let mut idle_passes = 0usize;
        let mut worst: Vec<(f64, u64, &rs_cam_core::debug_trace::ToolpathDebugSpan)> = Vec::new();
        for span in &pass_spans {
            if let Some(reason) = span.exit_reason.as_deref() {
                *passes_by_exit.entry(reason.to_owned()).or_insert(0) += 1;
                if reason.contains("loop") {
                    looped_passes += 1;
                }
                if reason.contains("idle") {
                    idle_passes += 1;
                }
            }
            if let Some(&y) = span.counters.get("yield_ratio") {
                yield_sum += y;
                yield_count += 1;
                if y < 0.1 {
                    low_yield_passes += 1;
                }
                let steps = span.counters.get("step_count").copied().unwrap_or(0.0) as u64;
                worst.push((y, steps, span));
            }
        }
        worst.sort_by(|a, b| a.0.total_cmp(&b.0));
        let worst_json: Vec<_> = worst
            .iter()
            .take(10)
            .map(|(y, steps, span)| {
                serde_json::json!({
                    "id": span.id,
                    "label": span.label,
                    "exit_reason": span.exit_reason,
                    "yield_ratio": y,
                    "step_count": steps,
                    "idle_count": span.counters.get("idle_count").copied().unwrap_or(0.0),
                    "search_evaluations": span.counters.get("search_evaluations").copied().unwrap_or(0.0),
                    "z_level": span.z_level,
                    "xy_bbox": span.xy_bbox,
                })
            })
            .collect();

        json_str(serde_json::json!({
            "summary": {
                "schema_version": trace.schema_version,
                "toolpath_name": trace.toolpath_name,
                "operation_label": trace.operation_label,
                "span_count": trace.spans.len(),
                "hotspot_count": trace.hotspots.len(),
                "annotation_count": trace.annotations.len(),
                "dominant_span_kind": trace.summary.dominant_span_kind,
                "dominant_span_elapsed_us": trace.summary.dominant_span_elapsed_us,
            },
            "diagnostics": {
                "pass_count": pass_spans.len(),
                "passes_by_exit_reason": passes_by_exit,
                "low_yield_passes": low_yield_passes,
                "looped_passes": looped_passes,
                "idle_passes": idle_passes,
                "avg_yield_ratio": if yield_count > 0 { yield_sum / yield_count as f64 } else { 0.0 },
                "worst_yields": worst_json,
            },
            "spans_returned": visible.len(),
            "spans_total_matching": total_matching,
            "spans": visible,
            "hotspots": trace.hotspots,
            "annotations": trace.annotations,
        }))
    }

    // ── Mutation tools (Phase 6) ──────────────────────────────────

    #[tool(
        name = "add_toolpath",
        description = "Add a new toolpath with default parameters to a setup. Returns the new toolpath index. Supported operation types: face, pocket, profile, adaptive, v_carve, rest, inlay, zigzag, trace, drill, chamfer, drop_cutter, adaptive3d, waterline, pencil, scallop, steep_shallow, ramp_finish, spiral_finish, radial_finish, horizontal_finish, project_curve."
    )]
    async fn add_toolpath(
        &self,
        #[allow(clippy::needless_pass_by_value)] Parameters(AddToolpathParam {
            setup_index,
            operation_type,
            tool_index,
            model_id,
            name,
        }): Parameters<AddToolpathParam>,
    ) -> String {
        let mut guard = self.session.lock().await;
        let Some(session) = guard.as_mut() else {
            return no_project_error();
        };

        let op_type = match parse_operation_type(&operation_type) {
            Ok(ot) => ot,
            Err(e) => return json_str(serde_json::json!({"error": e})),
        };

        // Resolve tool_id (raw ID) from the tool at tool_index
        let tools = session.list_tools();
        let tool_raw_id = match tools.get(tool_index) {
            Some(info) => info.id.0,
            None => {
                return json_str(
                    serde_json::json!({"error": format!("Tool index {tool_index} not found")}),
                )
            }
        };

        let op_config = OperationConfig::new_default(op_type);
        let label = op_type.label();
        let tp_name = name.unwrap_or_else(|| label.to_owned());

        let config = rs_cam_core::session::ToolpathConfig {
            id: 0, // overwritten by add_toolpath
            name: tp_name,
            enabled: true,
            operation: op_config,
            dressups: DressupConfig::for_op(op_type),
            heights: rs_cam_core::compute::config::HeightsConfig::default(),
            tool_id: tool_raw_id,
            model_id,
            pre_gcode: None,
            post_gcode: None,
            boundary: BoundaryConfig::default(),
            boundary_inherit: true,
            stock_source: rs_cam_core::compute::config::StockSource::default(),
            coolant: rs_cam_core::gcode::CoolantMode::default(),
            face_selection: None,
            feeds_auto: rs_cam_core::compute::config::FeedsAutoMode::default(),
            debug_options: rs_cam_core::debug_trace::ToolpathDebugOptions::default(),
        };

        match session.add_toolpath(setup_index, config) {
            Ok(idx) => json_str(serde_json::json!({
                "index": idx,
                "operation": label,
            })),
            Err(e) => json_str(serde_json::json!({"error": format!("{e}")})),
        }
    }

    #[tool(
        name = "remove_toolpath",
        description = "Remove a toolpath by index. Updates setup indices automatically."
    )]
    async fn remove_toolpath(
        &self,
        Parameters(RemoveToolpathParam { index }): Parameters<RemoveToolpathParam>,
    ) -> String {
        let mut guard = self.session.lock().await;
        let Some(session) = guard.as_mut() else {
            return no_project_error();
        };
        match session.remove_toolpath(index) {
            Ok(()) => text(format!("Removed toolpath {index}")),
            Err(e) => text(format!("Error: {e}")),
        }
    }

    #[tool(
        name = "add_tool",
        description = "Add a new tool to the project. Supported types: end_mill, ball_nose, bull_nose, v_bit, tapered_ball_nose. Returns the new tool index."
    )]
    async fn add_tool(
        &self,
        #[allow(clippy::needless_pass_by_value)] Parameters(AddToolParam {
            name,
            tool_type,
            diameter,
        }): Parameters<AddToolParam>,
    ) -> String {
        let mut guard = self.session.lock().await;
        let Some(session) = guard.as_mut() else {
            return no_project_error();
        };

        let tt = match parse_tool_type(&tool_type) {
            Ok(t) => t,
            Err(e) => return json_str(serde_json::json!({"error": e})),
        };

        let mut config = ToolConfig::new_default(ToolId(0), tt);
        config.name = name;
        config.diameter = diameter;

        let idx = session.add_tool(config);
        json_str(serde_json::json!({
            "index": idx,
            "tool_type": tool_type,
            "diameter": diameter,
        }))
    }

    #[tool(
        name = "remove_tool",
        description = "Remove a tool by index. Fails if any toolpath still references the tool."
    )]
    async fn remove_tool(
        &self,
        Parameters(RemoveToolParam { index }): Parameters<RemoveToolParam>,
    ) -> String {
        let mut guard = self.session.lock().await;
        let Some(session) = guard.as_mut() else {
            return no_project_error();
        };
        match session.remove_tool(index) {
            Ok(()) => text(format!("Removed tool {index}")),
            Err(e) => text(format!("Error: {e}")),
        }
    }

    #[tool(
        name = "set_stock_config",
        description = "Set stock dimensions (width x depth x height in mm). Invalidates simulation — re-run to update."
    )]
    async fn set_stock_config(
        &self,
        Parameters(SetStockConfigParam { x, y, z }): Parameters<SetStockConfigParam>,
    ) -> String {
        let mut guard = self.session.lock().await;
        let Some(session) = guard.as_mut() else {
            return no_project_error();
        };
        let mut stock = session.stock_config().clone();
        stock.x = x;
        stock.y = y;
        stock.z = z;
        session.set_stock_config(stock);
        text(format!(
            "Stock set to {x:.1} x {y:.1} x {z:.1} mm. Regenerate toolpaths and simulation to apply."
        ))
    }

    #[tool(
        name = "set_boundary_config",
        description = "Set the machining boundary for a toolpath. Sources: 'stock', 'model_silhouette'. Containment: 'center', 'inside', 'outside'. Invalidates cached result."
    )]
    async fn set_boundary_config(
        &self,
        #[allow(clippy::needless_pass_by_value)] Parameters(SetBoundaryConfigParam {
            index,
            enabled,
            source,
            containment,
            offset,
        }): Parameters<SetBoundaryConfigParam>,
    ) -> String {
        let mut guard = self.session.lock().await;
        let Some(session) = guard.as_mut() else {
            return no_project_error();
        };

        let boundary_source = match source.as_deref() {
            Some("stock") | None => BoundarySource::Stock,
            Some("model_silhouette") => BoundarySource::ModelSilhouette,
            Some(other) => {
                return json_str(serde_json::json!({
                    "error": format!("Unknown boundary source '{other}'. Use 'stock' or 'model_silhouette'.")
                }))
            }
        };

        let boundary_containment = match containment.as_deref() {
            Some("center") | None => BoundaryContainment::Center,
            Some("inside") => BoundaryContainment::Inside,
            Some("outside") => BoundaryContainment::Outside,
            Some(other) => {
                return json_str(serde_json::json!({
                    "error": format!("Unknown containment '{other}'. Use 'center', 'inside', or 'outside'.")
                }))
            }
        };

        let boundary = BoundaryConfig {
            enabled,
            source: boundary_source,
            containment: boundary_containment,
            offset: offset.unwrap_or(0.0),
        };

        match session.set_boundary_config(index, boundary) {
            Ok(()) => text(format!(
                "Boundary set on toolpath {index}. Regenerate to apply."
            )),
            Err(e) => text(format!("Error: {e}")),
        }
    }

    #[tool(
        name = "set_dressup_config",
        description = "Set dressup configuration for a toolpath. Pass a JSON object with dressup fields: entry_style, ramp_angle, helix_radius, helix_pitch, dogbone, lead_in_out, lead_radius, link_moves, link_max_distance, link_feed_rate, arc_fitting, arc_tolerance, feed_optimization, feed_max_rate, feed_ramp_rate, optimize_rapid_order, retract_strategy."
    )]
    async fn set_dressup_config(
        &self,
        #[allow(clippy::needless_pass_by_value)]
        Parameters(SetDressupConfigParam { index, dressup }): Parameters<
            SetDressupConfigParam,
        >,
    ) -> String {
        let mut guard = self.session.lock().await;
        let Some(session) = guard.as_mut() else {
            return no_project_error();
        };

        let dressup_config: DressupConfig = match serde_json::from_value(dressup) {
            Ok(dc) => dc,
            Err(e) => {
                return json_str(serde_json::json!({
                    "error": format!("Invalid dressup config: {e}")
                }))
            }
        };

        match session.set_dressup_config(index, dressup_config) {
            Ok(()) => text(format!(
                "Dressup config set on toolpath {index}. Regenerate to apply."
            )),
            Err(e) => text(format!("Error: {e}")),
        }
    }

    #[tool(
        name = "set_dressup_field",
        description = "Update a single dressup field on a toolpath (partial patch). Accepts any field name from the DressupConfig schema."
    )]
    async fn set_dressup_field(
        &self,
        #[allow(clippy::needless_pass_by_value)]
        Parameters(SetDressupFieldParam { index, key, value }): Parameters<
            SetDressupFieldParam,
        >,
    ) -> String {
        let mut guard = self.session.lock().await;
        let Some(session) = guard.as_mut() else {
            return no_project_error();
        };
        match session.set_dressup_field(index, &key, value) {
            Ok(()) => text(format!(
                "Dressup field '{key}' set on toolpath {index}. Regenerate to apply."
            )),
            Err(e) => text(format!("Error: {e}")),
        }
    }

    #[tool(
        name = "set_toolpath_enabled",
        description = "Enable or disable a toolpath for generation and simulation."
    )]
    async fn set_toolpath_enabled(
        &self,
        #[allow(clippy::needless_pass_by_value)]
        Parameters(SetToolpathEnabledParam { index, enabled }): Parameters<
            SetToolpathEnabledParam,
        >,
    ) -> String {
        let mut guard = self.session.lock().await;
        let Some(session) = guard.as_mut() else {
            return no_project_error();
        };
        match session.set_toolpath_enabled(index, enabled) {
            Ok(()) => text(format!(
                "Toolpath {index} {}",
                if enabled { "enabled" } else { "disabled" }
            )),
            Err(e) => text(format!("Error: {e}")),
        }
    }

    #[tool(
        name = "set_stock_source",
        description = "Set stock_source for a toolpath: 'fresh' (default) or 'from_remaining_stock' (rest machining). Invalidates the toolpath result."
    )]
    async fn set_stock_source(
        &self,
        #[allow(clippy::needless_pass_by_value)]
        Parameters(SetStockSourceParam { index, source }): Parameters<SetStockSourceParam>,
    ) -> String {
        let mut guard = self.session.lock().await;
        let Some(session) = guard.as_mut() else {
            return no_project_error();
        };
        let parsed = match source.as_str() {
            "fresh" => rs_cam_core::compute::config::StockSource::Fresh,
            "from_remaining_stock" => rs_cam_core::compute::config::StockSource::FromRemainingStock,
            other => {
                return text(format!(
                    "Error: unknown stock_source '{other}'. Expected 'fresh' or 'from_remaining_stock'."
                ));
            }
        };
        match session.set_stock_source(index, parsed) {
            Ok(()) => text(format!(
                "Stock source set to '{source}' on toolpath {index}. Regenerate to apply."
            )),
            Err(e) => text(format!("Error: {e}")),
        }
    }

    // ── Model import ─────────────────────────────────────────────────

    #[tool(
        name = "import_model",
        description = "Import a model file (.stl, .svg, .dxf, .step/.stp) into the current project. Returns the assigned model id and kind."
    )]
    async fn import_model(
        &self,
        #[allow(clippy::needless_pass_by_value)] Parameters(ImportModelParam { path }): Parameters<
            ImportModelParam,
        >,
    ) -> String {
        let mut guard = self.session.lock().await;
        let Some(session) = guard.as_mut() else {
            return no_project_error();
        };
        let path_buf = Path::new(&path);
        let Some(kind) = rs_cam_core::io::infer_kind_from_path(path_buf) else {
            return json_str(serde_json::json!({
                "error": format!("Unsupported or missing extension for '{path}'. Supported: .stl, .svg, .dxf, .step, .stp")
            }));
        };
        let next_id = session
            .models()
            .iter()
            .map(|m| m.id)
            .max()
            .map_or(0, |id| id + 1);
        match rs_cam_core::io::load_model_file(
            path_buf,
            next_id,
            kind,
            rs_cam_core::compute::stock_config::ModelUnits::Millimeters,
        ) {
            Ok(model) => {
                let assigned_id = session.add_model(model);
                json_str(serde_json::json!({
                    "model_id": assigned_id,
                    "kind": format!("{kind:?}").to_lowercase(),
                    "path": path,
                }))
            }
            Err(e) => json_str(serde_json::json!({"error": e.to_string()})),
        }
    }

    // ── Inspection tools ─────────────────────────────────────────────

    #[tool(
        name = "inspect_model",
        description = "Inspect all loaded models: kind, units, bbox, triangle count, polygon stats, BREP summary for STEP models."
    )]
    async fn inspect_model(&self) -> String {
        let guard = self.session.lock().await;
        let Some(session) = guard.as_ref() else {
            return no_project_error();
        };
        let models = session.models();
        if models.is_empty() {
            return json_str(serde_json::json!([]));
        }

        let mut result = Vec::new();
        for model in models {
            let mut map = serde_json::Map::new();
            map.insert("id".into(), serde_json::json!(model.id));
            map.insert("name".into(), serde_json::json!(model.name));
            map.insert(
                "kind".into(),
                serde_json::json!(model.kind.map(|k| format!("{k:?}").to_lowercase())),
            );
            map.insert(
                "units".into(),
                serde_json::json!(model.units.as_ref().map(|u| u.label())),
            );
            map.insert(
                "path".into(),
                serde_json::json!(model.path.display().to_string()),
            );
            map.insert("load_error".into(), serde_json::json!(model.load_error));

            if let Some(ref mesh) = model.mesh {
                let bbox = &mesh.bbox;
                map.insert(
                    "bbox".into(),
                    serde_json::json!({
                        "min": [bbox.min.x, bbox.min.y, bbox.min.z],
                        "max": [bbox.max.x, bbox.max.y, bbox.max.z],
                    }),
                );
                map.insert(
                    "dimensions".into(),
                    serde_json::json!({
                        "x": bbox.max.x - bbox.min.x,
                        "y": bbox.max.y - bbox.min.y,
                        "z": bbox.max.z - bbox.min.z,
                    }),
                );
                map.insert(
                    "triangle_count".into(),
                    serde_json::json!(mesh.triangles.len()),
                );
                map.insert(
                    "vertex_count".into(),
                    serde_json::json!(mesh.vertices.len()),
                );
            }

            if let Some(winding) = model.winding_report {
                map.insert(
                    "winding_consistency".into(),
                    serde_json::json!(1.0 - winding),
                );
            }

            if let Some(ref em) = model.enriched_mesh {
                let concave_count = em.edges.iter().filter(|e| e.is_concave).count();
                map.insert(
                    "brep".into(),
                    serde_json::json!({
                        "face_count": em.face_groups.len(),
                        "edge_count": em.edges.len(),
                        "concave_edge_count": concave_count,
                    }),
                );
            }

            if let Some(ref polys) = model.polygons {
                let count = polys.len();
                let total_area: f64 = polys.iter().map(|p| p.area()).sum();
                let total_perimeter: f64 = polys.iter().map(|p| p.perimeter()).sum();
                let hole_count: usize = polys.iter().map(|p| p.holes.len()).sum();
                map.insert(
                    "polygons".into(),
                    serde_json::json!({
                        "count": count,
                        "total_area": total_area,
                        "total_perimeter": total_perimeter,
                        "hole_count": hole_count,
                    }),
                );
            }

            result.push(serde_json::Value::Object(map));
        }

        json_str(serde_json::json!(result))
    }

    #[tool(
        name = "inspect_stock",
        description = "Inspect stock configuration: dimensions, origin, material, padding, alignment pins, workholding rigidity."
    )]
    async fn inspect_stock(&self) -> String {
        let guard = self.session.lock().await;
        let Some(session) = guard.as_ref() else {
            return no_project_error();
        };
        let stock = session.stock_config();
        let pins: Vec<serde_json::Value> = stock
            .alignment_pins
            .iter()
            .map(|p| {
                serde_json::json!({
                    "x": p.x,
                    "y": p.y,
                    "diameter": p.diameter,
                })
            })
            .collect();
        json_str(serde_json::json!({
            "dimensions": { "x": stock.x, "y": stock.y, "z": stock.z },
            "origin": { "x": stock.origin_x, "y": stock.origin_y, "z": stock.origin_z },
            "material": stock.material.label(),
            "padding": stock.padding,
            "auto_from_model": stock.auto_from_model,
            "workholding_rigidity": format!("{:?}", stock.workholding_rigidity),
            "alignment_pins": pins,
            "flip_axis": stock.flip_axis.map(|fa| fa.label()),
        }))
    }

    #[tool(
        name = "inspect_machine",
        description = "Inspect machine profile: name, max feed, max shank, safety factor, spindle, power, rigidity factors."
    )]
    async fn inspect_machine(&self) -> String {
        let guard = self.session.lock().await;
        let Some(session) = guard.as_ref() else {
            return no_project_error();
        };
        let machine = session.machine();

        let spindle = match &machine.spindle {
            rs_cam_core::machine::SpindleConfig::Variable { min_rpm, max_rpm } => {
                serde_json::json!({ "type": "Variable", "min_rpm": min_rpm, "max_rpm": max_rpm })
            }
            rs_cam_core::machine::SpindleConfig::Discrete { speeds } => {
                serde_json::json!({ "type": "Discrete", "speeds": speeds })
            }
        };
        let power = match &machine.power {
            rs_cam_core::machine::PowerModel::ConstantPower { power_kw } => {
                serde_json::json!({ "type": "ConstantPower", "power_kw": power_kw })
            }
            rs_cam_core::machine::PowerModel::VfdConstantTorque {
                rated_power_kw,
                rated_rpm,
            } => serde_json::json!({
                "type": "VfdConstantTorque",
                "rated_power_kw": rated_power_kw,
                "rated_rpm": rated_rpm,
            }),
        };
        let r = &machine.rigidity;
        json_str(serde_json::json!({
            "name": machine.name,
            "max_feed_mm_min": machine.max_feed_mm_min,
            "max_shank_mm": machine.max_shank_mm,
            "safety_factor": machine.safety_factor,
            "spindle": spindle,
            "power": power,
            "rigidity": {
                "doc_roughing_factor": r.doc_roughing_factor,
                "doc_finishing_factor": r.doc_finishing_factor,
                "woc_roughing_factor": r.woc_roughing_factor,
                "woc_roughing_max_mm": r.woc_roughing_max_mm,
                "woc_finishing_mm": r.woc_finishing_mm,
                "adaptive_doc_factor": r.adaptive_doc_factor,
                "adaptive_woc_factor": r.adaptive_woc_factor,
            },
        }))
    }

    #[tool(
        name = "inspect_brep_faces",
        description = "Inspect BREP faces of a STEP model: surface types, bboxes, normals, radii, dihedral edges."
    )]
    async fn inspect_brep_faces(
        &self,
        Parameters(ModelIdParam { model_id }): Parameters<ModelIdParam>,
    ) -> String {
        let guard = self.session.lock().await;
        let Some(session) = guard.as_ref() else {
            return no_project_error();
        };
        let Some(model) = session.models().iter().find(|m| m.id == model_id) else {
            return json_str(serde_json::json!({"error": format!("Model {model_id} not found")}));
        };
        let Some(ref em) = model.enriched_mesh else {
            return json_str(serde_json::json!({
                "error": format!("Model '{}' has no BREP data (not a STEP model)", model.name)
            }));
        };

        let faces_json: Vec<serde_json::Value> = em
            .face_groups
            .iter()
            .map(|fg| {
                let mut map = serde_json::Map::new();
                map.insert("id".into(), serde_json::json!(fg.id.0));
                map.insert(
                    "surface_type".into(),
                    serde_json::json!(format!("{:?}", fg.surface_type)),
                );
                map.insert(
                    "bbox".into(),
                    serde_json::json!({
                        "min": [fg.bbox.min.x, fg.bbox.min.y, fg.bbox.min.z],
                        "max": [fg.bbox.max.x, fg.bbox.max.y, fg.bbox.max.z],
                    }),
                );
                map.insert(
                    "triangle_count".into(),
                    serde_json::json!(fg.triangle_range.len()),
                );
                if let rs_cam_core::enriched_mesh::SurfaceParams::Plane { normal, .. } =
                    &fg.surface_params
                {
                    map.insert(
                        "normal".into(),
                        serde_json::json!([normal.x, normal.y, normal.z]),
                    );
                }
                serde_json::Value::Object(map)
            })
            .collect();

        let edges_json: Vec<serde_json::Value> = em
            .edges
            .iter()
            .map(|edge| {
                serde_json::json!({
                    "id": edge.id,
                    "face_a": edge.face_a.0,
                    "face_b": edge.face_b.0,
                    "dihedral_angle_deg": edge.dihedral_angle.to_degrees(),
                    "is_concave": edge.is_concave,
                })
            })
            .collect();

        json_str(serde_json::json!({
            "model_id": model_id,
            "face_count": em.face_groups.len(),
            "faces": faces_json,
            "edges": edges_json,
        }))
    }

    // ── Setup management ─────────────────────────────────────────────

    #[tool(
        name = "add_setup",
        description = "Add a new setup (workholding orientation). Returns the new setup index and ID."
    )]
    async fn add_setup(
        &self,
        #[allow(clippy::needless_pass_by_value)] Parameters(AddSetupParam { name }): Parameters<
            AddSetupParam,
        >,
    ) -> String {
        let mut guard = self.session.lock().await;
        let Some(session) = guard.as_mut() else {
            return no_project_error();
        };
        let next_num = session.list_setups().len() + 1;
        let name = name.unwrap_or_else(|| format!("Setup {next_num}"));
        let idx = session.add_setup(name.clone(), FaceUp::Top);
        let setup_id = session.list_setups().get(idx).map(|s| s.id).unwrap_or(0);
        json_str(serde_json::json!({
            "index": idx,
            "id": setup_id,
            "name": name,
        }))
    }

    #[tool(
        name = "set_setup_face",
        description = "Change the face orientation of a setup: top, bottom, front, back, left, right."
    )]
    async fn set_setup_face(
        &self,
        #[allow(clippy::needless_pass_by_value)] Parameters(SetSetupFaceParam {
            setup_index,
            face_up,
        }): Parameters<SetSetupFaceParam>,
    ) -> String {
        let mut guard = self.session.lock().await;
        let Some(session) = guard.as_mut() else {
            return no_project_error();
        };
        let face = match face_up.to_lowercase().as_str() {
            "top" => FaceUp::Top,
            "bottom" => FaceUp::Bottom,
            "front" => FaceUp::Front,
            "back" => FaceUp::Back,
            "left" => FaceUp::Left,
            "right" => FaceUp::Right,
            _ => {
                return json_str(serde_json::json!({
                    "error": format!("Unknown face '{face_up}'. Use: top, bottom, front, back, left, right")
                }));
            }
        };
        // Direct field mutation through setups_mut (no dedicated session method yet)
        #[allow(deprecated)]
        let result = session
            .setups_mut()
            .get_mut(setup_index)
            .map(|s| s.face_up = face);
        if result.is_none() {
            return json_str(
                serde_json::json!({"error": format!("Setup index {setup_index} not found")}),
            );
        }
        // Changing face_up invalidates toolpath results in that setup
        json_str(serde_json::json!({
            "setup_index": setup_index,
            "face_up": face_up.to_lowercase(),
        }))
    }

    #[tool(
        name = "move_toolpath_to_setup",
        description = "Move a toolpath from its current setup to a different setup. Invalidates cached result."
    )]
    async fn move_toolpath_to_setup(
        &self,
        Parameters(MoveToolpathToSetupParam {
            toolpath_index,
            target_setup_index,
        }): Parameters<MoveToolpathToSetupParam>,
    ) -> String {
        let mut guard = self.session.lock().await;
        let Some(session) = guard.as_mut() else {
            return no_project_error();
        };
        match session.move_toolpath_to_setup(toolpath_index, target_setup_index) {
            Ok(()) => json_str(serde_json::json!({
                "toolpath_index": toolpath_index,
                "target_setup_index": target_setup_index,
                "message": format!("Moved toolpath {toolpath_index} to setup {target_setup_index}"),
            })),
            Err(e) => json_str(serde_json::json!({"error": e.to_string()})),
        }
    }

    // ── Alignment pins ───────────────────────────────────────────────

    #[tool(
        name = "add_alignment_pin",
        description = "Add a workholding alignment pin to the stock at (x, y) with given diameter."
    )]
    async fn add_alignment_pin(
        &self,
        Parameters(AddAlignmentPinParam { x, y, diameter }): Parameters<AddAlignmentPinParam>,
    ) -> String {
        let mut guard = self.session.lock().await;
        let Some(session) = guard.as_mut() else {
            return no_project_error();
        };
        let added = session.add_alignment_pin(x, y, diameter);
        let pin_count = session.stock_config().alignment_pins.len();
        let message = if added {
            format!("Added alignment pin at ({x:.1}, {y:.1}) dia {diameter:.1}mm")
        } else {
            format!("Pin already present at ({x:.1}, {y:.1}); skipped duplicate")
        };
        json_str(serde_json::json!({
            "ok": true,
            "added": added,
            "message": message,
            "pin_count": pin_count,
        }))
    }

    #[tool(
        name = "remove_alignment_pin",
        description = "Remove an alignment pin by index (0-based)."
    )]
    async fn remove_alignment_pin(
        &self,
        Parameters(RemoveAlignmentPinParam { index }): Parameters<RemoveAlignmentPinParam>,
    ) -> String {
        let mut guard = self.session.lock().await;
        let Some(session) = guard.as_mut() else {
            return no_project_error();
        };
        match session.remove_alignment_pin(index) {
            Ok(()) => json_str(serde_json::json!({
                "ok": true,
                "message": format!("Removed alignment pin {index}"),
                "pin_count": session.stock_config().alignment_pins.len(),
            })),
            Err(e) => json_str(serde_json::json!({ "error": e.to_string() })),
        }
    }

    #[tool(
        name = "save_project",
        description = "Save the current project state to a TOML file."
    )]
    async fn save_project(
        &self,
        #[allow(clippy::needless_pass_by_value)] Parameters(SaveProjectParam { path }): Parameters<
            SaveProjectParam,
        >,
    ) -> String {
        let guard = self.session.lock().await;
        let Some(session) = guard.as_ref() else {
            return no_project_error();
        };
        match session.save(Path::new(&path)) {
            Ok(()) => text(format!("Project saved to {path}")),
            Err(e) => text(format!("Save failed: {e}")),
        }
    }

    // ── Query tools ──────────────────────────────────────────────────

    #[tool(
        name = "list_setups",
        description = "List all setups in the loaded project with name, face orientation, and toolpath indices"
    )]
    async fn list_setups(&self) -> String {
        let guard = self.session.lock().await;
        let Some(session) = guard.as_ref() else {
            return no_project_error();
        };
        let setups: Vec<serde_json::Value> = session
            .list_setups()
            .iter()
            .map(|s| {
                serde_json::json!({
                    "id": s.id,
                    "name": s.name,
                    "face_up": s.face_up.label(),
                    "toolpath_indices": s.toolpath_indices,
                })
            })
            .collect();
        json_str(serde_json::json!({ "setups": setups }))
    }
}

impl ServerHandler for CamServer {
    fn get_info(&self) -> ServerInfo {
        let mut info = ServerInfo::default();
        info.server_info.name = "rs-cam".into();
        info.server_info.version = "0.1.0".into();
        info.capabilities.tools = Some(rmcp::model::ToolsCapability { list_changed: None });
        info
    }
}
