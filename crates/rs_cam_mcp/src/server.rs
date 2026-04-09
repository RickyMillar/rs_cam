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
use rs_cam_core::session::{ProjectSession, SimulationOptions};

// ── Parameter structs ─────────────────────────────────────────────────

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
pub struct SaveProjectParam {
    /// File path to save the project TOML to (required)
    pub path: String,
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

    #[tool(name = "export_gcode", description = "Export G-code to a file path")]
    async fn export_gcode(
        &self,
        Parameters(ExportParam { path }): Parameters<ExportParam>,
    ) -> String {
        let guard = self.session.lock().await;
        let Some(session) = guard.as_ref() else {
            return no_project_error();
        };
        match session.export_gcode(Path::new(&path), None) {
            Ok(()) => text(format!("G-code exported to {path}")),
            Err(e) => text(format!("Export failed: {e}")),
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
                        .filter_map(|i| session.get_result(i).map(|r| r.toolpath.as_ref()))
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
                &result.toolpath,
                bg.as_ref(),
                w,
                h,
                include_rapids.unwrap_or(true),
            );
            match image::save_buffer(Path::new(&path), &pixels, w, h, image::ColorType::Rgba8) {
                Ok(()) => text(format!(
                    "Toolpath {index} exported to {path} ({w}x{h}, {} moves, {:.0}mm cutting)",
                    result.toolpath.moves.len(),
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
                rs_cam_core::viz::toolpath_standalone_3d_html(&result.toolpath, Some(bounds));

            match std::fs::write(&path, &html) {
                Ok(()) => text(format!(
                    "Toolpath view exported to {path} ({} moves, {:.0}mm cutting)",
                    result.toolpath.moves.len(),
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
        name = "get_cut_trace",
        description = "Get simulation cut trace data: semantic summaries, hotspots, and issues. Run simulation first. Use toolpath_id to filter to a single toolpath."
    )]
    async fn get_cut_trace(
        &self,
        #[allow(clippy::needless_pass_by_value)] Parameters(CutTraceParam {
            toolpath_id,
            max_hotspots,
            max_issues,
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
            dressups: DressupConfig::default(),
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
