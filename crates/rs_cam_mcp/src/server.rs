//! MCP tool definitions wrapping `ProjectSession`.

use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex as TokioMutex;

use rmcp::handler::server::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::ServerInfo;
use rmcp::schemars;
use rmcp::{ServerHandler, tool, tool_router};
use serde::Deserialize;

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
    /// Include toolpath overlay lines (HTML only, default true)
    pub include_toolpaths: Option<bool>,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct ScreenshotToolpathParam {
    /// Toolpath index (0-based)
    pub index: usize,
    /// Output HTML file path
    pub path: String,
}

fn text(msg: impl Into<String>) -> String {
    msg.into()
}

fn json_str(data: serde_json::Value) -> String {
    serde_json::to_string_pretty(&data).unwrap_or_else(|e| format!("{{\"error\": \"{e}\"}}" ))
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
            return json_str(serde_json::json!({"error": "No project loaded. Call load_project first."}));
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
            return json_str(serde_json::json!({"error": "No project loaded"}));
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
            return json_str(serde_json::json!({"error": "No project loaded"}));
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
            return json_str(serde_json::json!({"error": "No project loaded"}));
        };
        match session.get_toolpath_config(index) {
            Some(tc) => json_str(serde_json::json!({
                "id": tc.id,
                "name": tc.name,
                "enabled": tc.enabled,
                "operation": tc.operation.label(),
                "tool_id": tc.tool_id,
                "model_id": tc.model_id,
            })),
            None => json_str(serde_json::json!({"error": format!("Toolpath index {index} not found")})),
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
                return Err("No project loaded".to_owned());
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
                return Err("No project loaded".to_owned());
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
            };
            let mut guard = session.blocking_lock();
            let Some(s) = guard.as_mut() else {
                return Err("No project loaded".to_owned());
            };
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
            return json_str(serde_json::json!({"error": "No project loaded"}));
        };
        json_str(serde_json::to_value(session.diagnostics()).unwrap_or_default())
    }

    #[tool(
        name = "export_gcode",
        description = "Export G-code to a file path"
    )]
    async fn export_gcode(
        &self,
        Parameters(ExportParam { path }): Parameters<ExportParam>,
    ) -> String {
        let guard = self.session.lock().await;
        let Some(session) = guard.as_ref() else {
            return text("No project loaded");
        };
        match session.export_gcode(Path::new(&path), None) {
            Ok(()) => text(format!("G-code exported to {path}")),
            Err(e) => text(format!("Export failed: {e}")),
        }
    }

    #[tool(
        name = "screenshot_simulation",
        description = "Export simulated stock as a 6-view composite PNG (default) or interactive 3D HTML. Run simulation first. Use .png for agent-viewable images, .html for interactive browser views."
    )]
    async fn screenshot_simulation(
        &self,
        #[allow(clippy::needless_pass_by_value)]
        Parameters(ScreenshotSimParam { path, width, height, include_toolpaths }): Parameters<ScreenshotSimParam>,
    ) -> String {
        let guard = self.session.lock().await;
        let Some(session) = guard.as_ref() else {
            return text("No project loaded");
        };
        let Some(sim) = session.simulation_result() else {
            return text("No simulation result. Run run_simulation first.");
        };

        if path.ends_with(".png") {
            let w = width.unwrap_or(1200);
            let h = height.unwrap_or(800);
            match rs_cam_core::fingerprint::save_mesh_composite_png(
                &sim.mesh,
                Path::new(&path),
                w,
                h,
            ) {
                Ok(()) => text(format!(
                    "6-view composite exported to {path} ({w}x{h})",
                )),
                Err(e) => text(format!("Failed to render: {e}")),
            }
        } else {
            let toolpaths: Vec<&rs_cam_core::toolpath::Toolpath> =
                if include_toolpaths.unwrap_or(true) {
                    (0..session.toolpath_count())
                        .filter_map(|i| session.get_result(i).map(|r| &r.toolpath))
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
        description = "Export an interactive 3D HTML view of a single generated toolpath. Opens in any browser."
    )]
    async fn screenshot_toolpath(
        &self,
        Parameters(ScreenshotToolpathParam { index, path }): Parameters<ScreenshotToolpathParam>,
    ) -> String {
        let guard = self.session.lock().await;
        let Some(session) = guard.as_ref() else {
            return text("No project loaded");
        };
        let Some(result) = session.get_result(index) else {
            return text(format!("Toolpath {index} not generated. Run generate_toolpath first."));
        };

        let bbox = session.stock_bbox();
        let bounds = [bbox.min.x, bbox.min.y, bbox.min.z, bbox.max.x, bbox.max.y, bbox.max.z];
        let html = rs_cam_core::viz::toolpath_standalone_3d_html(&result.toolpath, Some(bounds));

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

impl ServerHandler for CamServer {
    fn get_info(&self) -> ServerInfo {
        let mut info = ServerInfo::default();
        info.server_info.name = "rs-cam".into();
        info.server_info.version = "0.1.0".into();
        info.capabilities.tools = Some(rmcp::model::ToolsCapability {
            list_changed: None,
        });
        info
    }
}
