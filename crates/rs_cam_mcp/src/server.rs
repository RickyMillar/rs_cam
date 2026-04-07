//! MCP tool definitions wrapping `ProjectSession`.

use std::sync::Arc;
use tokio::sync::Mutex as TokioMutex;

use rmcp::handler::server::tool::ToolRouter;
use rmcp::handler::server::wrapper::{Json, Parameters};
use rmcp::model::ServerInfo;
use rmcp::schemars;
use rmcp::{ServerHandler, tool, tool_router};
use serde::{Deserialize, Serialize};

use rs_cam_core::session::{ProjectSession, SimulationOptions};

// ── Parameter structs ─────────────────────────────────────────────────

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

// ── Output structs ────────────────────────────────────────────────────

#[derive(Serialize, schemars::JsonSchema)]
pub struct TextOutput {
    pub message: String,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct JsonOutput {
    pub data: serde_json::Value,
}

// ── Server ────────────────────────────────────────────────────────────

/// MCP server that exposes rs_cam's ProjectSession as tools.
#[derive(Clone)]
pub struct CamServer {
    session: Arc<TokioMutex<ProjectSession>>,
    #[allow(dead_code)]
    tool_router: ToolRouter<Self>,
}

impl CamServer {
    pub fn new(session: Arc<TokioMutex<ProjectSession>>) -> Self {
        let tool_router = Self::tool_router();
        Self {
            session,
            tool_router,
        }
    }
}

#[tool_router]
impl CamServer {
    #[tool(name = "project_summary", description = "Get project summary: name, stock dimensions, setup count, toolpath count, tools")]
    async fn project_summary(&self) -> Json<JsonOutput> {
        let session = self.session.lock().await;
        let bbox = session.stock_bbox();
        let data = serde_json::json!({
            "name": session.name(),
            "stock": {
                "width": bbox.max.x - bbox.min.x,
                "depth": bbox.max.y - bbox.min.y,
                "height": bbox.max.z - bbox.min.z,
            },
            "setup_count": session.setup_count(),
            "toolpath_count": session.toolpath_count(),
            "tools": session.list_tools(),
        });
        Json(JsonOutput { data })
    }

    #[tool(name = "list_toolpaths", description = "List all toolpaths with name, operation type, enabled status, and tool")]
    async fn list_toolpaths(&self) -> Json<JsonOutput> {
        let session = self.session.lock().await;
        let data = serde_json::to_value(session.list_toolpaths()).unwrap_or_default();
        Json(JsonOutput { data })
    }

    #[tool(name = "list_tools", description = "List all tools with type and dimensions")]
    async fn list_tools(&self) -> Json<JsonOutput> {
        let session = self.session.lock().await;
        let data = serde_json::to_value(session.list_tools()).unwrap_or_default();
        Json(JsonOutput { data })
    }

    #[tool(name = "get_toolpath_params", description = "Get operation parameters for a toolpath by index")]
    async fn get_toolpath_params(
        &self,
        Parameters(IndexParam { index }): Parameters<IndexParam>,
    ) -> Json<JsonOutput> {
        let session = self.session.lock().await;
        let data = match session.get_toolpath_config(index) {
            Some(tc) => serde_json::json!({
                "id": tc.id,
                "name": tc.name,
                "enabled": tc.enabled,
                "operation": tc.operation.label(),
                "tool_id": tc.tool_id,
                "model_id": tc.model_id,
            }),
            None => serde_json::json!({"error": format!("Toolpath index {index} not found")}),
        };
        Json(JsonOutput { data })
    }

    #[tool(name = "generate_toolpath", description = "Generate a single toolpath by index. Returns move count and distances.")]
    async fn generate_toolpath(
        &self,
        Parameters(IndexParam { index }): Parameters<IndexParam>,
    ) -> Json<JsonOutput> {
        let session = Arc::clone(&self.session);
        let result = tokio::task::spawn_blocking(move || {
            let cancel = std::sync::atomic::AtomicBool::new(false);
            let mut s = session.blocking_lock();
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

        let data = match result {
            Ok(Ok(v)) => v,
            Ok(Err(e)) => serde_json::json!({"error": e}),
            Err(e) => serde_json::json!({"error": format!("Task failed: {e}")}),
        };
        Json(JsonOutput { data })
    }

    #[tool(name = "generate_all", description = "Generate all enabled toolpaths. Returns count of newly generated toolpaths.")]
    async fn generate_all(&self) -> Json<TextOutput> {
        let session = Arc::clone(&self.session);
        let result = tokio::task::spawn_blocking(move || {
            let cancel = std::sync::atomic::AtomicBool::new(false);
            let mut s = session.blocking_lock();
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

        let message = match result {
            Ok(Ok(n)) => format!("Generated {n} toolpaths"),
            Ok(Err(e)) => format!("Error: {e}"),
            Err(e) => format!("Task failed: {e}"),
        };
        Json(TextOutput { message })
    }

    #[tool(name = "run_simulation", description = "Run tri-dexel stock simulation. Returns air cutting %, engagement, collisions, and verdict.")]
    async fn run_simulation(
        &self,
        Parameters(SimulationParam { resolution }): Parameters<SimulationParam>,
    ) -> Json<JsonOutput> {
        let session = Arc::clone(&self.session);
        let res = resolution.unwrap_or(0.5);
        let result = tokio::task::spawn_blocking(move || {
            let cancel = std::sync::atomic::AtomicBool::new(false);
            let opts = SimulationOptions {
                resolution: res,
                skip_ids: Vec::new(),
                metrics_enabled: true,
            };
            let mut s = session.blocking_lock();
            s.run_simulation(&opts, &cancel)
                .map_err(|e| e.to_string())?;
            let diag = s.diagnostics();
            Ok::<_, String>(serde_json::json!({
                "total_runtime_s": diag.total_runtime_s,
                "air_cut_percentage": diag.air_cut_percentage,
                "average_engagement": diag.average_engagement,
                "collision_count": diag.collision_count,
                "rapid_collision_count": diag.rapid_collision_count,
                "verdict": diag.verdict,
                "per_toolpath": diag.per_toolpath,
            }))
        })
        .await;

        let data = match result {
            Ok(Ok(v)) => v,
            Ok(Err(e)) => serde_json::json!({"error": e}),
            Err(e) => serde_json::json!({"error": format!("Task failed: {e}")}),
        };
        Json(JsonOutput { data })
    }

    #[tool(name = "get_diagnostics", description = "Get project diagnostics: per-toolpath stats, collision counts, air cutting %, verdict")]
    async fn get_diagnostics(&self) -> Json<JsonOutput> {
        let session = self.session.lock().await;
        let data = serde_json::to_value(session.diagnostics()).unwrap_or_default();
        Json(JsonOutput { data })
    }

    #[tool(name = "export_gcode", description = "Export G-code to a file path")]
    async fn export_gcode(
        &self,
        Parameters(ExportParam { path }): Parameters<ExportParam>,
    ) -> Json<TextOutput> {
        let session = self.session.lock().await;
        let message = match session.export_gcode(std::path::Path::new(&path), None) {
            Ok(()) => format!("G-code exported to {path}"),
            Err(e) => format!("Export failed: {e}"),
        };
        Json(TextOutput { message })
    }
}

impl ServerHandler for CamServer {
    fn get_info(&self) -> ServerInfo {
        let mut info = ServerInfo::default();
        info.server_info.name = "rs-cam".into();
        info.server_info.version = "0.1.0".into();
        info
    }
}
