//! Embedded MCP server that runs inside the GUI process.
//!
//! Each tool method constructs an `McpRequestKind`, sends it to the GUI thread
//! via a channel, calls `request_repaint()` to wake the GUI, and awaits the
//! oneshot response.

use rmcp::handler::server::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{Meta, ProgressNotificationParam, ServerInfo};
use rmcp::{Peer, RoleServer, ServerHandler, tool, tool_router};

use crate::mcp_bridge::{McpRequest, McpRequestKind, ProgressUpdate};

// Re-use parameter structs from the standalone MCP crate.
use rs_cam_mcp::server::{
    AddAlignmentPinParam, AddToolParam, AddToolpathParam, CollisionCheckParam, CutTraceParam,
    ExportParam, IndexParam, LoadProjectParam, ModelIdParam, RemoveAlignmentPinParam,
    RemoveToolParam, RemoveToolpathParam, SaveProjectParam, ScreenshotSimParam,
    ScreenshotToolpathParam, SetBoundaryConfigParam, SetDressupConfigParam, SetStockConfigParam,
    SetToolParamInput, SetToolpathParamInput, SimJumpToMoveParam, SimJumpToToolpathBoundaryParam,
    SimScrubToolpathParam, SimulationParam,
};

/// Embedded MCP server that forwards requests to the GUI thread.
#[derive(Clone)]
pub struct EmbeddedCamServer {
    request_tx: std::sync::mpsc::Sender<McpRequest>,
    egui_ctx: egui::Context,
    #[allow(dead_code)]
    tool_router: ToolRouter<Self>,
}

impl EmbeddedCamServer {
    pub fn new(request_tx: std::sync::mpsc::Sender<McpRequest>, egui_ctx: egui::Context) -> Self {
        let tool_router = Self::tool_router();
        Self {
            request_tx,
            egui_ctx,
            tool_router,
        }
    }

    pub fn into_tool_router() -> ToolRouter<Self> {
        Self::tool_router()
    }

    /// Send a request to the GUI and await the response (no progress tracking).
    async fn send_request(&self, kind: McpRequestKind) -> Result<String, String> {
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();
        let request = McpRequest {
            kind,
            response_tx,
            progress_tx: None,
        };
        self.request_tx
            .send(request)
            .map_err(|e| format!("Failed to send MCP request: {e}"))?;
        self.egui_ctx.request_repaint();
        match response_rx.await {
            Ok(resp) => resp.result,
            Err(e) => Err(format!("MCP response channel closed: {e}")),
        }
    }

    /// Send a request to the GUI and forward progress notifications to the MCP
    /// client while awaiting the final response.
    async fn send_with_progress(
        &self,
        kind: McpRequestKind,
        meta: Meta,
        peer: Peer<RoleServer>,
    ) -> Result<String, String> {
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();
        let (progress_tx, mut progress_rx) = tokio::sync::mpsc::channel::<ProgressUpdate>(32);

        let request = McpRequest {
            kind,
            response_tx,
            progress_tx: Some(progress_tx),
        };

        self.request_tx
            .send(request)
            .map_err(|e| format!("Failed to send MCP request: {e}"))?;
        self.egui_ctx.request_repaint();

        // If we have a progress token, forward progress notifications to the client.
        let progress_token = meta.get_progress_token();
        if let Some(token) = progress_token {
            let mut resp_rx = std::pin::pin!(response_rx);
            loop {
                tokio::select! {
                    Some(update) = progress_rx.recv() => {
                        let _ = peer.notify_progress(ProgressNotificationParam {
                            progress_token: token.clone(),
                            progress: update.progress,
                            total: update.total,
                            message: Some(update.message),
                        }).await;
                    }
                    result = &mut resp_rx => {
                        return match result {
                            Ok(resp) => resp.result,
                            Err(e) => Err(format!("MCP response channel closed: {e}")),
                        };
                    }
                }
            }
        } else {
            // No progress token -- just await the response.
            match response_rx.await {
                Ok(resp) => resp.result,
                Err(e) => Err(format!("MCP response channel closed: {e}")),
            }
        }
    }

    /// Format a result into the final tool return string.
    fn format_result(result: Result<String, String>) -> String {
        match result {
            Ok(s) => s,
            Err(e) => {
                let err_json = serde_json::json!({"error": e});
                serde_json::to_string_pretty(&err_json)
                    .unwrap_or_else(|_| format!("{{\"error\": \"{e}\"}}"))
            }
        }
    }
}

#[tool_router]
impl EmbeddedCamServer {
    // ── Read tools ───────────────────────────────────────────────────

    #[tool(
        name = "project_summary",
        description = "Get project summary: name, stock dimensions, setup count, toolpath count, tools"
    )]
    async fn project_summary(&self) -> String {
        Self::format_result(self.send_request(McpRequestKind::ProjectSummary).await)
    }

    #[tool(
        name = "list_toolpaths",
        description = "List all toolpaths with name, operation type, enabled status, and tool"
    )]
    async fn list_toolpaths(&self) -> String {
        Self::format_result(self.send_request(McpRequestKind::ListToolpaths).await)
    }

    #[tool(
        name = "list_tools",
        description = "List all tools with type and dimensions"
    )]
    async fn list_tools(&self) -> String {
        Self::format_result(self.send_request(McpRequestKind::ListTools).await)
    }

    #[tool(
        name = "list_setups",
        description = "List all setups in the loaded project with name, face orientation, and toolpath indices"
    )]
    async fn list_setups(&self) -> String {
        Self::format_result(self.send_request(McpRequestKind::ListSetups).await)
    }

    #[tool(
        name = "get_toolpath_params",
        description = "Get operation parameters for a toolpath by index"
    )]
    async fn get_toolpath_params(
        &self,
        Parameters(IndexParam { index }): Parameters<IndexParam>,
    ) -> String {
        Self::format_result(
            self.send_request(McpRequestKind::GetToolpathParams { index })
                .await,
        )
    }

    #[tool(
        name = "get_diagnostics",
        description = "Get project diagnostics: per-toolpath stats, collision counts, air cutting %, verdict"
    )]
    async fn get_diagnostics(&self) -> String {
        Self::format_result(self.send_request(McpRequestKind::GetDiagnostics).await)
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
        Self::format_result(
            self.send_request(McpRequestKind::GetCutTrace {
                toolpath_id,
                max_hotspots,
                max_issues,
            })
            .await,
        )
    }

    #[tool(
        name = "inspect_model",
        description = "Inspect all loaded models: mesh stats, bbox, BREP face summary, polygon summary. Returns a JSON array."
    )]
    async fn inspect_model(&self) -> String {
        Self::format_result(self.send_request(McpRequestKind::InspectModel).await)
    }

    #[tool(
        name = "inspect_stock",
        description = "Inspect stock configuration: dimensions, origin, material, padding, alignment pins, workholding rigidity."
    )]
    async fn inspect_stock(&self) -> String {
        Self::format_result(self.send_request(McpRequestKind::InspectStock).await)
    }

    #[tool(
        name = "inspect_machine",
        description = "Inspect machine profile: spindle, power, feeds limits, rigidity factors."
    )]
    async fn inspect_machine(&self) -> String {
        Self::format_result(self.send_request(McpRequestKind::InspectMachine).await)
    }

    #[tool(
        name = "inspect_brep_faces",
        description = "Inspect BREP faces and edges for a STEP model. Returns detailed surface types, normals, radii, bboxes, and edge dihedral angles."
    )]
    async fn inspect_brep_faces(
        &self,
        Parameters(ModelIdParam { model_id }): Parameters<ModelIdParam>,
    ) -> String {
        Self::format_result(
            self.send_request(McpRequestKind::InspectBrepFaces { model_id })
                .await,
        )
    }

    // ── Mutation tools ───────────────────────────────────────────────

    #[tool(
        name = "add_alignment_pin",
        description = "Add an alignment pin to the stock config at the given position. Used for multi-setup registration."
    )]
    async fn add_alignment_pin(
        &self,
        Parameters(AddAlignmentPinParam { x, y, diameter }): Parameters<AddAlignmentPinParam>,
    ) -> String {
        Self::format_result(
            self.send_request(McpRequestKind::AddAlignmentPin { x, y, diameter })
                .await,
        )
    }

    #[tool(
        name = "remove_alignment_pin",
        description = "Remove an alignment pin by index (0-based) from the stock config."
    )]
    async fn remove_alignment_pin(
        &self,
        Parameters(RemoveAlignmentPinParam { index }): Parameters<RemoveAlignmentPinParam>,
    ) -> String {
        Self::format_result(
            self.send_request(McpRequestKind::RemoveAlignmentPin { index })
                .await,
        )
    }

    #[tool(
        name = "import_model",
        description = "Import a model file into the current project. Supported formats: .stl (3D mesh), .dxf (2D vectors), .svg (2D vectors), .step/.stp (BREP CAD). Auto-detects format from file extension. Returns model ID and geometry summary."
    )]
    async fn import_model(
        &self,
        Parameters(rs_cam_mcp::server::ImportModelParam { path }): Parameters<
            rs_cam_mcp::server::ImportModelParam,
        >,
    ) -> String {
        Self::format_result(
            self.send_request(McpRequestKind::ImportModel { path })
                .await,
        )
    }

    #[tool(
        name = "load_project",
        description = "Load a project TOML file. Must be called before other tools if no project was specified on startup."
    )]
    async fn load_project(
        &self,
        Parameters(LoadProjectParam { path }): Parameters<LoadProjectParam>,
    ) -> String {
        Self::format_result(
            self.send_request(McpRequestKind::LoadProject { path })
                .await,
        )
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
        Self::format_result(
            self.send_request(McpRequestKind::SaveProject { path })
                .await,
        )
    }

    #[tool(name = "export_gcode", description = "Export G-code to a file path")]
    async fn export_gcode(
        &self,
        Parameters(ExportParam { path }): Parameters<ExportParam>,
    ) -> String {
        Self::format_result(
            self.send_request(McpRequestKind::ExportGcode { path })
                .await,
        )
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
        Self::format_result(
            self.send_request(McpRequestKind::SetToolpathParam {
                index,
                param,
                value,
            })
            .await,
        )
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
        Self::format_result(
            self.send_request(McpRequestKind::SetToolParam {
                index,
                param,
                value,
            })
            .await,
        )
    }

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
        Self::format_result(
            self.send_request(McpRequestKind::AddToolpath {
                setup_index,
                operation_type,
                tool_index,
                model_id,
                name,
            })
            .await,
        )
    }

    #[tool(
        name = "remove_toolpath",
        description = "Remove a toolpath by index. Updates setup indices automatically."
    )]
    async fn remove_toolpath(
        &self,
        Parameters(RemoveToolpathParam { index }): Parameters<RemoveToolpathParam>,
    ) -> String {
        Self::format_result(
            self.send_request(McpRequestKind::RemoveToolpath { index })
                .await,
        )
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
        Self::format_result(
            self.send_request(McpRequestKind::AddTool {
                name,
                tool_type,
                diameter,
            })
            .await,
        )
    }

    #[tool(
        name = "remove_tool",
        description = "Remove a tool by index. Fails if any toolpath still references the tool."
    )]
    async fn remove_tool(
        &self,
        Parameters(RemoveToolParam { index }): Parameters<RemoveToolParam>,
    ) -> String {
        Self::format_result(
            self.send_request(McpRequestKind::RemoveTool { index })
                .await,
        )
    }

    #[tool(
        name = "set_stock_config",
        description = "Set stock dimensions (width x depth x height in mm). Invalidates simulation — re-run to update."
    )]
    async fn set_stock_config(
        &self,
        Parameters(SetStockConfigParam { x, y, z }): Parameters<SetStockConfigParam>,
    ) -> String {
        Self::format_result(
            self.send_request(McpRequestKind::SetStockConfig { x, y, z })
                .await,
        )
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
        Self::format_result(
            self.send_request(McpRequestKind::SetBoundaryConfig {
                index,
                enabled,
                source,
                containment,
                offset,
            })
            .await,
        )
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
        Self::format_result(
            self.send_request(McpRequestKind::SetDressupConfig { index, dressup })
                .await,
        )
    }

    // ── Compute tools ────────────────────────────────────────────────

    #[tool(
        name = "generate_toolpath",
        description = "Generate a single toolpath by index. Returns move count and distances."
    )]
    async fn generate_toolpath(
        &self,
        Parameters(IndexParam { index }): Parameters<IndexParam>,
        meta: Meta,
        peer: Peer<RoleServer>,
    ) -> String {
        Self::format_result(
            self.send_with_progress(McpRequestKind::GenerateToolpath { index }, meta, peer)
                .await,
        )
    }

    #[tool(
        name = "generate_all",
        description = "Generate all enabled toolpaths. Returns count of newly generated toolpaths."
    )]
    async fn generate_all(&self, meta: Meta, peer: Peer<RoleServer>) -> String {
        Self::format_result(
            self.send_with_progress(McpRequestKind::GenerateAll, meta, peer)
                .await,
        )
    }

    #[tool(
        name = "run_simulation",
        description = "Run tri-dexel stock simulation. Returns air cutting %, engagement, collisions, and verdict."
    )]
    async fn run_simulation(
        &self,
        Parameters(SimulationParam { resolution }): Parameters<SimulationParam>,
        meta: Meta,
        peer: Peer<RoleServer>,
    ) -> String {
        Self::format_result(
            self.send_with_progress(McpRequestKind::RunSimulation { resolution }, meta, peer)
                .await,
        )
    }

    #[tool(
        name = "collision_check",
        description = "Run a holder/shank collision check for a specific toolpath. Requires the toolpath to be generated first. Returns collision count, positions, and minimum safe stickout."
    )]
    async fn collision_check(
        &self,
        Parameters(CollisionCheckParam { index }): Parameters<CollisionCheckParam>,
        meta: Meta,
        peer: Peer<RoleServer>,
    ) -> String {
        Self::format_result(
            self.send_with_progress(McpRequestKind::CollisionCheck { index }, meta, peer)
                .await,
        )
    }

    // ── Simulation scrubbing tools ──────────────────────────────────

    #[tool(
        name = "sim_jump_to_move",
        description = "Jump the simulation playback to a specific move index. Updates the GUI viewport in real-time. Use after run_simulation to scrub through the cutting process. Move 0 is the start, total_moves is the end."
    )]
    async fn sim_jump_to_move(
        &self,
        Parameters(SimJumpToMoveParam { move_index }): Parameters<SimJumpToMoveParam>,
    ) -> String {
        Self::format_result(
            self.send_request(McpRequestKind::SimJumpToMove { move_index })
                .await,
        )
    }

    #[tool(
        name = "sim_jump_to_start",
        description = "Jump the simulation playback to the very start (move 0). Convenience shortcut for sim_jump_to_move(0)."
    )]
    async fn sim_jump_to_start(&self) -> String {
        Self::format_result(self.send_request(McpRequestKind::SimJumpToStart).await)
    }

    #[tool(
        name = "sim_jump_to_end",
        description = "Jump the simulation playback to the very end (last move). Convenience shortcut for sim_jump_to_move(total_moves)."
    )]
    async fn sim_jump_to_end(&self) -> String {
        Self::format_result(self.send_request(McpRequestKind::SimJumpToEnd).await)
    }

    // ── Per-toolpath simulation scrubbing tools ───────────────────────

    #[tool(
        name = "sim_scrub_toolpath",
        description = "Scrub the simulation to a percentage position within a specific toolpath. Returns the computed move index, toolpath name, and total moves in that toolpath. Requires a simulation to have been run first."
    )]
    async fn sim_scrub_toolpath(
        &self,
        Parameters(SimScrubToolpathParam { index, percent }): Parameters<SimScrubToolpathParam>,
    ) -> String {
        Self::format_result(
            self.send_request(McpRequestKind::SimScrubToolpath { index, percent })
                .await,
        )
    }

    #[tool(
        name = "sim_jump_to_toolpath_start",
        description = "Jump the simulation to the first move of a specific toolpath. Convenience shortcut for sim_scrub_toolpath(index, 0.0)."
    )]
    async fn sim_jump_to_toolpath_start(
        &self,
        Parameters(SimJumpToToolpathBoundaryParam { index }): Parameters<
            SimJumpToToolpathBoundaryParam,
        >,
    ) -> String {
        Self::format_result(
            self.send_request(McpRequestKind::SimJumpToToolpathStart { index })
                .await,
        )
    }

    #[tool(
        name = "sim_jump_to_toolpath_end",
        description = "Jump the simulation to the last move of a specific toolpath. Convenience shortcut for sim_scrub_toolpath(index, 100.0)."
    )]
    async fn sim_jump_to_toolpath_end(
        &self,
        Parameters(SimJumpToToolpathBoundaryParam { index }): Parameters<
            SimJumpToToolpathBoundaryParam,
        >,
    ) -> String {
        Self::format_result(
            self.send_request(McpRequestKind::SimJumpToToolpathEnd { index })
                .await,
        )
    }

    // ── Screenshot tools ─────────────────────────────────────────────

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
        Self::format_result(
            self.send_request(McpRequestKind::ScreenshotSimulation {
                path,
                width,
                height,
                checkpoint,
                include_toolpaths,
            })
            .await,
        )
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
        Self::format_result(
            self.send_request(McpRequestKind::ScreenshotToolpath {
                index,
                path,
                width,
                height,
                show_stock,
                include_rapids,
            })
            .await,
        )
    }
}

impl ServerHandler for EmbeddedCamServer {
    fn get_info(&self) -> ServerInfo {
        let mut info = ServerInfo::default();
        info.server_info.name = "rs-cam".into();
        info.server_info.version = "0.1.0".into();
        info.capabilities.tools = Some(rmcp::model::ToolsCapability { list_changed: None });
        info
    }
}
