//! GUI-side processing of MCP requests. Called from `RsCamApp::update()`.

#![deny(clippy::indexing_slicing)]

use std::path::Path;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::{
    BoundaryConfig, BoundaryContainment, BoundarySource, DressupConfig,
};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId};

use crate::controller::Severity;
use crate::mcp_bridge::{
    McpRequest, McpRequestKind, McpResponse, PendingGenerateAll, ProgressUpdate,
};
use crate::state::Workspace;
use crate::state::selection::Selection;
use crate::state::toolpath::ToolpathId;
use crate::ui::AppEvent;

use rs_cam_mcp::server::{json_str, no_project_error, parse_operation_type, parse_tool_type, text};

impl super::RsCamApp {
    /// Non-blocking drain of MCP requests from the channel.
    /// Called once per frame from `update()`.
    pub(crate) fn drain_mcp_requests(&mut self) {
        // Garbage-collect expired MCP highlights (older than 3 seconds).
        self.controller
            .state_mut()
            .gui
            .mcp_highlights
            .retain(|_, when| when.elapsed().as_secs() < 3);

        let Some(receiver) = self.mcp_receiver.as_ref() else {
            return;
        };

        // Drain all pending requests (non-blocking).
        let mut requests = Vec::new();
        loop {
            match receiver.try_recv() {
                Ok(req) => requests.push(req),
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => break,
            }
        }

        for request in requests {
            self.handle_mcp_request(request);
        }
    }

    fn handle_mcp_request(&mut self, request: McpRequest) {
        let McpRequest {
            kind,
            response_tx,
            progress_tx,
        } = request;

        match kind {
            // ── Read operations ──────────────────────────────────────
            McpRequestKind::ProjectSummary => {
                let resp = self.mcp_project_summary();
                let _ = response_tx.send(McpResponse { result: Ok(resp) });
            }
            McpRequestKind::ListToolpaths => {
                let resp = self.mcp_list_toolpaths();
                let _ = response_tx.send(McpResponse { result: Ok(resp) });
            }
            McpRequestKind::ListTools => {
                let resp = self.mcp_list_tools();
                let _ = response_tx.send(McpResponse { result: Ok(resp) });
            }
            McpRequestKind::ListSetups => {
                let resp = self.mcp_list_setups();
                let _ = response_tx.send(McpResponse { result: Ok(resp) });
            }
            McpRequestKind::GetToolpathParams { index } => {
                let resp = self.mcp_get_toolpath_params(index);
                let _ = response_tx.send(McpResponse { result: Ok(resp) });
            }
            McpRequestKind::GetDiagnostics => {
                let resp = self.mcp_get_diagnostics();
                let _ = response_tx.send(McpResponse { result: Ok(resp) });
            }
            McpRequestKind::GetCutTrace {
                toolpath_id,
                max_hotspots,
                max_issues,
            } => {
                let resp = self.mcp_get_cut_trace(toolpath_id, max_hotspots, max_issues);
                let _ = response_tx.send(McpResponse { result: Ok(resp) });
            }
            McpRequestKind::InspectModel => {
                let resp = self.mcp_inspect_model();
                let _ = response_tx.send(McpResponse { result: Ok(resp) });
            }
            McpRequestKind::InspectStock => {
                let resp = self.mcp_inspect_stock();
                let _ = response_tx.send(McpResponse { result: Ok(resp) });
            }
            McpRequestKind::InspectMachine => {
                let resp = self.mcp_inspect_machine();
                let _ = response_tx.send(McpResponse { result: Ok(resp) });
            }
            McpRequestKind::InspectBrepFaces { model_id } => {
                let resp = self.mcp_inspect_brep_faces(model_id);
                let _ = response_tx.send(McpResponse { result: Ok(resp) });
            }

            // ── Mutation operations ──────────────────────────────────
            McpRequestKind::AddAlignmentPin { x, y, diameter } => {
                self.controller
                    .events_mut()
                    .push(AppEvent::SwitchWorkspace(Workspace::Setup));
                let resp = self.mcp_add_alignment_pin(x, y, diameter);
                let _ = response_tx.send(McpResponse { result: Ok(resp) });
            }
            McpRequestKind::RemoveAlignmentPin { index } => {
                let resp = self.mcp_remove_alignment_pin(index);
                let _ = response_tx.send(McpResponse { result: Ok(resp) });
            }
            McpRequestKind::LoadProject { path } => {
                // Extract file name for the toast before loading.
                let name = Path::new(&path)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or(&path)
                    .to_owned();
                self.controller
                    .push_notification(format!("MCP: Loaded '{name}'"), Severity::Info);
                let resp = self.mcp_load_project(&path);
                let _ = response_tx.send(resp);
            }
            McpRequestKind::SaveProject { path } => {
                self.controller
                    .push_notification("MCP: Saved project".to_owned(), Severity::Info);
                let resp = self.mcp_save_project(&path);
                let _ = response_tx.send(resp);
            }
            McpRequestKind::ExportGcode { path } => {
                let resp = self.mcp_export_gcode(&path);
                let _ = response_tx.send(McpResponse { result: Ok(resp) });
            }
            McpRequestKind::SetToolpathParam {
                index,
                param,
                value,
            } => {
                // Look up toolpath name for the toast and highlight key.
                let tp_info = self
                    .controller
                    .state()
                    .session
                    .toolpath_configs()
                    .get(index)
                    .map(|tc| (tc.name.clone(), tc.id));
                let tp_name = tp_info
                    .as_ref()
                    .map_or_else(|| format!("#{index}"), |(name, _)| name.clone());
                let value_str = match &value {
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                self.controller.push_notification(
                    format!("MCP: Set {param} = {value_str} on '{tp_name}'"),
                    Severity::Info,
                );
                self.controller
                    .events_mut()
                    .push(AppEvent::SwitchWorkspace(Workspace::Toolpaths));
                // Record highlight for the changed parameter and select the toolpath.
                if let Some((_, tp_id)) = tp_info {
                    let key = format!("toolpath_{tp_id}_{param}");
                    self.controller
                        .state_mut()
                        .gui
                        .mcp_highlights
                        .insert(key, std::time::Instant::now());
                    self.controller.state_mut().selection = Selection::Toolpath(ToolpathId(tp_id));
                }
                let resp = self.mcp_set_toolpath_param(index, &param, value);
                let _ = response_tx.send(McpResponse { result: Ok(resp) });
            }
            McpRequestKind::SetToolParam {
                index,
                param,
                value,
            } => {
                // Look up tool name for the toast and highlight key.
                let tools = self.controller.state().session.list_tools();
                let tool_info = tools.get(index).map(|t| (t.name.clone(), t.id.0));
                let tool_name = tool_info
                    .as_ref()
                    .map_or_else(|| format!("#{index}"), |(name, _)| name.clone());
                let value_str = match &value {
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                self.controller.push_notification(
                    format!("MCP: Set {param} = {value_str} on '{tool_name}'"),
                    Severity::Info,
                );
                // Record highlight for the changed parameter and select the tool.
                if let Some((_, tool_id)) = tool_info {
                    let key = format!("tool_{tool_id}_{param}");
                    self.controller
                        .state_mut()
                        .gui
                        .mcp_highlights
                        .insert(key, std::time::Instant::now());
                    self.controller.state_mut().selection = Selection::Tool(ToolId(tool_id));
                }
                let resp = self.mcp_set_tool_param(index, &param, &value);
                let _ = response_tx.send(McpResponse { result: Ok(resp) });
            }
            McpRequestKind::AddToolpath {
                setup_index,
                operation_type,
                tool_index,
                model_id,
                name,
            } => {
                let display_name = name.as_deref().unwrap_or(&operation_type);
                self.controller.push_notification(
                    format!("MCP: Added toolpath '{display_name}'"),
                    Severity::Info,
                );
                self.controller
                    .events_mut()
                    .push(AppEvent::SwitchWorkspace(Workspace::Toolpaths));
                let resp =
                    self.mcp_add_toolpath(setup_index, &operation_type, tool_index, model_id, name);
                // Select the newly added toolpath so its properties are visible.
                let tp_count = self.controller.state().session.toolpath_count();
                if tp_count > 0
                    && let Some(tc) = self
                        .controller
                        .state()
                        .session
                        .toolpath_configs()
                        .get(tp_count - 1)
                {
                    self.controller.state_mut().selection = Selection::Toolpath(ToolpathId(tc.id));
                }
                let _ = response_tx.send(McpResponse { result: Ok(resp) });
            }
            McpRequestKind::RemoveToolpath { index } => {
                self.controller
                    .push_notification(format!("MCP: Removed toolpath {index}"), Severity::Info);
                let resp = self.mcp_remove_toolpath(index);
                let _ = response_tx.send(McpResponse { result: Ok(resp) });
            }
            McpRequestKind::AddTool {
                name,
                tool_type,
                diameter,
            } => {
                self.controller
                    .push_notification(format!("MCP: Added tool '{name}'"), Severity::Info);
                let resp = self.mcp_add_tool(&name, &tool_type, diameter);
                let _ = response_tx.send(McpResponse { result: Ok(resp) });
            }
            McpRequestKind::RemoveTool { index } => {
                let resp = self.mcp_remove_tool(index);
                let _ = response_tx.send(McpResponse { result: Ok(resp) });
            }
            McpRequestKind::SetStockConfig { x, y, z } => {
                self.controller
                    .events_mut()
                    .push(AppEvent::SwitchWorkspace(Workspace::Setup));
                // Record highlight for stock dimensions.
                let key = "stock_dimensions".to_owned();
                self.controller
                    .state_mut()
                    .gui
                    .mcp_highlights
                    .insert(key, std::time::Instant::now());
                let resp = self.mcp_set_stock_config(x, y, z);
                let _ = response_tx.send(McpResponse { result: Ok(resp) });
            }
            McpRequestKind::SetBoundaryConfig {
                index,
                enabled,
                source,
                containment,
                offset,
            } => {
                let resp = self.mcp_set_boundary_config(
                    index,
                    enabled,
                    source.as_deref(),
                    containment.as_deref(),
                    offset,
                );
                let _ = response_tx.send(McpResponse { result: Ok(resp) });
            }
            McpRequestKind::SetDressupConfig { index, dressup } => {
                let resp = self.mcp_set_dressup_config(index, dressup);
                let _ = response_tx.send(McpResponse { result: Ok(resp) });
            }

            // ── Compute operations (async — store oneshot) ───────────
            McpRequestKind::GenerateToolpath { index } => {
                // Look up toolpath name for the toast.
                let tp_name = self
                    .controller
                    .state()
                    .session
                    .toolpath_configs()
                    .get(index)
                    .map_or_else(|| format!("#{index}"), |tc| tc.name.clone());
                self.controller.push_notification(
                    format!("MCP: Generating toolpath '{tp_name}'..."),
                    Severity::Info,
                );
                self.controller
                    .events_mut()
                    .push(AppEvent::SwitchWorkspace(Workspace::Toolpaths));
                self.mcp_send_progress(&progress_tx, "Generating toolpath...", 0.0, Some(1.0));
                self.mcp_generate_toolpath(index, response_tx);
            }
            McpRequestKind::GenerateAll => {
                self.controller.push_notification(
                    "MCP: Generating all toolpaths...".to_owned(),
                    Severity::Info,
                );
                self.mcp_generate_all(response_tx, progress_tx);
            }
            McpRequestKind::RunSimulation { resolution } => {
                self.controller
                    .push_notification("MCP: Running simulation...".to_owned(), Severity::Info);
                self.controller
                    .events_mut()
                    .push(AppEvent::SwitchWorkspace(Workspace::Simulation));
                self.mcp_send_progress(&progress_tx, "Starting simulation...", 0.0, Some(1.0));
                self.mcp_run_simulation(resolution, response_tx);
            }
            McpRequestKind::CollisionCheck { index } => {
                self.mcp_send_progress(&progress_tx, "Running collision check...", 0.0, Some(1.0));
                self.mcp_collision_check(index, response_tx);
            }

            // ── Simulation scrubbing operations ─────────────────────
            McpRequestKind::SimJumpToMove { move_index } => {
                self.controller
                    .events_mut()
                    .push(AppEvent::SwitchWorkspace(Workspace::Simulation));
                let resp = self.mcp_sim_jump_to_move(move_index);
                let _ = response_tx.send(McpResponse { result: Ok(resp) });
            }
            McpRequestKind::SimJumpToStart => {
                self.controller
                    .events_mut()
                    .push(AppEvent::SwitchWorkspace(Workspace::Simulation));
                let resp = self.mcp_sim_jump_to_start();
                let _ = response_tx.send(McpResponse { result: Ok(resp) });
            }
            McpRequestKind::SimJumpToEnd => {
                self.controller
                    .events_mut()
                    .push(AppEvent::SwitchWorkspace(Workspace::Simulation));
                let resp = self.mcp_sim_jump_to_end();
                let _ = response_tx.send(McpResponse { result: Ok(resp) });
            }
            McpRequestKind::SimScrubToolpath { index, percent } => {
                self.controller
                    .events_mut()
                    .push(AppEvent::SwitchWorkspace(Workspace::Simulation));
                let resp = self.mcp_sim_scrub_toolpath(index, percent);
                let _ = response_tx.send(McpResponse { result: resp });
            }
            McpRequestKind::SimJumpToToolpathStart { index } => {
                self.controller
                    .events_mut()
                    .push(AppEvent::SwitchWorkspace(Workspace::Simulation));
                let resp = self.mcp_sim_scrub_toolpath(index, 0.0);
                let _ = response_tx.send(McpResponse { result: resp });
            }
            McpRequestKind::SimJumpToToolpathEnd { index } => {
                self.controller
                    .events_mut()
                    .push(AppEvent::SwitchWorkspace(Workspace::Simulation));
                let resp = self.mcp_sim_scrub_toolpath(index, 100.0);
                let _ = response_tx.send(McpResponse { result: resp });
            }

            // ── Screenshot operations ────────────────────────────────
            McpRequestKind::ScreenshotSimulation {
                path,
                width,
                height,
                checkpoint,
                include_toolpaths,
            } => {
                let resp = self.mcp_screenshot_simulation(
                    &path,
                    width,
                    height,
                    checkpoint,
                    include_toolpaths,
                );
                let _ = response_tx.send(McpResponse { result: Ok(resp) });
            }
            McpRequestKind::ScreenshotToolpath {
                index,
                path,
                width,
                height,
                show_stock,
                include_rapids,
            } => {
                let resp = self.mcp_screenshot_toolpath(
                    index,
                    &path,
                    width,
                    height,
                    show_stock,
                    include_rapids,
                );
                let _ = response_tx.send(McpResponse { result: Ok(resp) });
            }
        }
    }

    /// Send a progress update to the MCP client (non-blocking).
    fn mcp_send_progress(
        &self,
        progress_tx: &Option<tokio::sync::mpsc::Sender<ProgressUpdate>>,
        message: &str,
        progress: f64,
        total: Option<f64>,
    ) {
        if let Some(ref tx) = *progress_tx {
            let _ = tx.try_send(ProgressUpdate {
                message: message.to_owned(),
                progress,
                total,
            });
        }
    }

    // ── Read implementations ─────────────────────────────────────────

    fn mcp_project_summary(&self) -> String {
        let session = &self.controller.state().session;
        if session.toolpath_count() == 0 && session.models().is_empty() {
            return no_project_error();
        }
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

    fn mcp_list_toolpaths(&self) -> String {
        let session = &self.controller.state().session;
        json_str(serde_json::to_value(session.list_toolpaths()).unwrap_or_default())
    }

    fn mcp_list_tools(&self) -> String {
        let session = &self.controller.state().session;
        json_str(serde_json::to_value(session.list_tools()).unwrap_or_default())
    }

    fn mcp_list_setups(&self) -> String {
        let session = &self.controller.state().session;
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

    fn mcp_get_toolpath_params(&self, index: usize) -> String {
        let session = &self.controller.state().session;
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

    fn mcp_get_diagnostics(&self) -> String {
        json_str(self.controller.build_mcp_diagnostics())
    }

    fn mcp_get_cut_trace(
        &self,
        toolpath_id: Option<usize>,
        max_hotspots: Option<usize>,
        max_issues: Option<usize>,
    ) -> String {
        let sim_state = &self.controller.state().simulation;
        let Some(results) = sim_state.results.as_ref() else {
            return json_str(
                serde_json::json!({"error": "No simulation result. Run run_simulation first."}),
            );
        };
        let Some(ct) = results.cut_trace.as_ref() else {
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
        let hotspots_val: Vec<&_> = hotspots.iter().take(max_h).collect();
        let hotspots_val =
            serde_json::to_value(&hotspots_val).unwrap_or_else(|_| serde_json::json!([]));
        let issues_val: Vec<&_> = issues.iter().take(max_i).collect();
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

    // ── Inspection implementations ──────────────────────────────────

    fn mcp_inspect_model(&self) -> String {
        let session = &self.controller.state().session;
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

            // Mesh-level stats (STL and STEP)
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

            // Winding consistency (STL-specific)
            if let Some(winding) = model.winding_report {
                map.insert(
                    "winding_consistency".into(),
                    serde_json::json!(1.0 - winding),
                );
            }

            // BREP summary (STEP-specific)
            if let Some(ref em) = model.enriched_mesh {
                let concave_count = em.edges.iter().filter(|e| e.is_concave).count();
                let faces_json: Vec<serde_json::Value> = em
                    .face_groups
                    .iter()
                    .map(|fg| {
                        serde_json::json!({
                            "id": fg.id.0,
                            "surface_type": format!("{:?}", fg.surface_type),
                            "bbox": {
                                "min": [fg.bbox.min.x, fg.bbox.min.y, fg.bbox.min.z],
                                "max": [fg.bbox.max.x, fg.bbox.max.y, fg.bbox.max.z],
                            },
                        })
                    })
                    .collect();
                map.insert(
                    "brep".into(),
                    serde_json::json!({
                        "face_count": em.face_groups.len(),
                        "edge_count": em.edges.len(),
                        "concave_edge_count": concave_count,
                        "faces": faces_json,
                    }),
                );
            }

            // Polygon summary (SVG/DXF-specific)
            if let Some(ref polys) = model.polygons {
                let count = polys.len();
                let total_area: f64 = polys.iter().map(|p| p.area()).sum();
                let total_perimeter: f64 = polys.iter().map(|p| p.perimeter()).sum();
                let hole_count: usize = polys.iter().map(|p| p.holes.len()).sum();

                // Compute 2D bbox from polygon exteriors
                let mut min_x = f64::MAX;
                let mut min_y = f64::MAX;
                let mut max_x = f64::MIN;
                let mut max_y = f64::MIN;
                for poly in polys.iter() {
                    for pt in &poly.exterior {
                        if pt.x < min_x {
                            min_x = pt.x;
                        }
                        if pt.y < min_y {
                            min_y = pt.y;
                        }
                        if pt.x > max_x {
                            max_x = pt.x;
                        }
                        if pt.y > max_y {
                            max_y = pt.y;
                        }
                    }
                }

                let bbox_2d = if min_x <= max_x {
                    serde_json::json!({ "min": [min_x, min_y], "max": [max_x, max_y] })
                } else {
                    serde_json::json!(null)
                };

                map.insert(
                    "polygons".into(),
                    serde_json::json!({
                        "count": count,
                        "total_area": total_area,
                        "total_perimeter": total_perimeter,
                        "hole_count": hole_count,
                        "bbox_2d": bbox_2d,
                    }),
                );
            }

            result.push(serde_json::Value::Object(map));
        }

        json_str(serde_json::json!(result))
    }

    fn mcp_inspect_stock(&self) -> String {
        let session = &self.controller.state().session;
        let stock = session.stock_config();

        let pins: Vec<serde_json::Value> = stock
            .alignment_pins
            .iter()
            .map(|pin| {
                serde_json::json!({
                    "x": pin.x,
                    "y": pin.y,
                    "diameter": pin.diameter,
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

    fn mcp_inspect_machine(&self) -> String {
        let session = &self.controller.state().session;
        let machine = session.machine();

        let spindle = match &machine.spindle {
            rs_cam_core::machine::SpindleConfig::Variable { min_rpm, max_rpm } => {
                serde_json::json!({
                    "type": "Variable",
                    "min_rpm": min_rpm,
                    "max_rpm": max_rpm,
                })
            }
            rs_cam_core::machine::SpindleConfig::Discrete { speeds } => {
                serde_json::json!({
                    "type": "Discrete",
                    "speeds": speeds,
                })
            }
        };

        let power = match &machine.power {
            rs_cam_core::machine::PowerModel::ConstantPower { power_kw } => {
                serde_json::json!({
                    "type": "ConstantPower",
                    "power_kw": power_kw,
                })
            }
            rs_cam_core::machine::PowerModel::VfdConstantTorque {
                rated_power_kw,
                rated_rpm,
            } => {
                serde_json::json!({
                    "type": "VfdConstantTorque",
                    "rated_power_kw": rated_power_kw,
                    "rated_rpm": rated_rpm,
                })
            }
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

    fn mcp_inspect_brep_faces(&self, model_id: usize) -> String {
        let session = &self.controller.state().session;
        let models = session.models();
        let model = models.iter().find(|m| m.id == model_id);
        let Some(model) = model else {
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
                map.insert(
                    "has_2d_boundary".into(),
                    serde_json::json!(fg.boundary_loops_2d.is_some()),
                );

                // Surface-type-specific fields
                match &fg.surface_params {
                    rs_cam_core::enriched_mesh::SurfaceParams::Plane { normal, .. } => {
                        map.insert(
                            "normal".into(),
                            serde_json::json!([normal.x, normal.y, normal.z]),
                        );
                        map.insert(
                            "is_horizontal".into(),
                            serde_json::json!(normal.z.abs() > 0.95),
                        );
                    }
                    rs_cam_core::enriched_mesh::SurfaceParams::Cylinder {
                        radius,
                        axis_dir,
                        ..
                    } => {
                        map.insert("radius".into(), serde_json::json!(radius));
                        map.insert(
                            "axis".into(),
                            serde_json::json!([axis_dir.x, axis_dir.y, axis_dir.z]),
                        );
                    }
                    rs_cam_core::enriched_mesh::SurfaceParams::Cone {
                        half_angle, axis, ..
                    } => {
                        map.insert(
                            "half_angle_deg".into(),
                            serde_json::json!(half_angle.to_degrees()),
                        );
                        map.insert("axis".into(), serde_json::json!([axis.x, axis.y, axis.z]));
                    }
                    rs_cam_core::enriched_mesh::SurfaceParams::Sphere { radius, .. } => {
                        map.insert("radius".into(), serde_json::json!(radius));
                    }
                    rs_cam_core::enriched_mesh::SurfaceParams::Torus {
                        major_radius,
                        minor_radius,
                        axis,
                        ..
                    } => {
                        map.insert("major_radius".into(), serde_json::json!(major_radius));
                        map.insert("minor_radius".into(), serde_json::json!(minor_radius));
                        map.insert("axis".into(), serde_json::json!([axis.x, axis.y, axis.z]));
                    }
                    _ => {}
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
                    "vertex_count": edge.vertices.len(),
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

    // ── Alignment pin implementations ───────────────────────────────

    fn mcp_add_alignment_pin(&mut self, x: f64, y: f64, diameter: f64) -> String {
        use rs_cam_core::compute::stock_config::AlignmentPin;

        let mut stock = self.controller.state().session.stock_config().clone();
        stock.alignment_pins.push(AlignmentPin::new(x, y, diameter));
        let pin_count = stock.alignment_pins.len();
        self.controller.state_mut().session.set_stock_config(stock);
        self.controller.state_mut().gui.mark_edited();
        self.controller.set_pending_upload();
        json_str(serde_json::json!({
            "ok": true,
            "message": format!("Added alignment pin at ({x:.1}, {y:.1}) dia {diameter:.1}mm"),
            "pin_count": pin_count,
        }))
    }

    fn mcp_remove_alignment_pin(&mut self, index: usize) -> String {
        let mut stock = self.controller.state().session.stock_config().clone();
        if index >= stock.alignment_pins.len() {
            return json_str(serde_json::json!({
                "error": format!("Pin index {index} out of range (have {})", stock.alignment_pins.len())
            }));
        }
        stock.alignment_pins.remove(index);
        let pin_count = stock.alignment_pins.len();
        self.controller.state_mut().session.set_stock_config(stock);
        self.controller.state_mut().gui.mark_edited();
        self.controller.set_pending_upload();
        json_str(serde_json::json!({
            "ok": true,
            "message": format!("Removed alignment pin {index}"),
            "pin_count": pin_count,
        }))
    }

    // ── Mutation implementations ─────────────────────────────────────

    fn mcp_load_project(&mut self, path: &str) -> McpResponse {
        match self.controller.open_job_from_path(Path::new(path)) {
            Ok(()) => {
                let name = self.controller.state().session.name().to_owned();
                let tp_count = self.controller.state().session.toolpath_count();
                let setup_count = self.controller.state().session.setup_count();
                McpResponse {
                    result: Ok(text(format!(
                        "Loaded '{name}' -- {setup_count} setups, {tp_count} toolpaths"
                    ))),
                }
            }
            Err(e) => McpResponse {
                result: Ok(text(format!("Failed to load: {e}"))),
            },
        }
    }

    fn mcp_save_project(&mut self, path: &str) -> McpResponse {
        match self.controller.save_job_to_path(Path::new(path)) {
            Ok(()) => McpResponse {
                result: Ok(text(format!("Project saved to {path}"))),
            },
            Err(e) => McpResponse {
                result: Ok(text(format!("Save failed: {e}"))),
            },
        }
    }

    fn mcp_export_gcode(&self, path: &str) -> String {
        let session = &self.controller.state().session;
        match session.export_gcode(Path::new(path), None) {
            Ok(()) => text(format!("G-code exported to {path}")),
            Err(e) => text(format!("Export failed: {e}")),
        }
    }

    fn mcp_set_toolpath_param(
        &mut self,
        index: usize,
        param: &str,
        value: serde_json::Value,
    ) -> String {
        match self
            .controller
            .state_mut()
            .session
            .set_toolpath_param(index, param, value)
        {
            Ok(()) => {
                self.controller.state_mut().gui.mark_edited();
                text(format!(
                    "Set toolpath {index} param '{param}'. Regenerate to apply."
                ))
            }
            Err(e) => text(format!("Error: {e}")),
        }
    }

    fn mcp_set_tool_param(
        &mut self,
        index: usize,
        param: &str,
        value: &serde_json::Value,
    ) -> String {
        match self
            .controller
            .state_mut()
            .session
            .set_tool_param(index, param, value)
        {
            Ok(()) => {
                self.controller.state_mut().gui.mark_edited();
                text(format!(
                    "Set tool {index} param '{param}'. Regenerate affected toolpaths to apply."
                ))
            }
            Err(e) => text(format!("Error: {e}")),
        }
    }

    fn mcp_add_toolpath(
        &mut self,
        setup_index: usize,
        operation_type: &str,
        tool_index: usize,
        model_id: usize,
        name: Option<String>,
    ) -> String {
        let op_type = match parse_operation_type(operation_type) {
            Ok(ot) => ot,
            Err(e) => return json_str(serde_json::json!({"error": e})),
        };

        let session = &self.controller.state().session;
        let tools = session.list_tools();
        let tool_raw_id = match tools.get(tool_index) {
            Some(info) => info.id.0,
            None => {
                return json_str(
                    serde_json::json!({"error": format!("Tool index {tool_index} not found")}),
                );
            }
        };

        let op_config = OperationConfig::new_default(op_type);
        let label = op_type.label();
        let tp_name = name.unwrap_or_else(|| label.to_owned());

        let config = rs_cam_core::session::ToolpathConfig {
            id: 0,
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

        match self
            .controller
            .state_mut()
            .session
            .add_toolpath(setup_index, config)
        {
            Ok(idx) => {
                // Create GUI runtime entry for the new toolpath.
                // Read the id and auto_regen flag before mutating gui.
                let tp_info = self
                    .controller
                    .state()
                    .session
                    .toolpath_configs()
                    .get(idx)
                    .map(|tc| (tc.id, tc.operation.default_auto_regen()));
                if let Some((id, auto_regen)) = tp_info {
                    self.controller
                        .state_mut()
                        .gui
                        .toolpath_rt
                        .insert(id, crate::state::runtime::ToolpathRuntime::new(auto_regen));
                }
                self.controller.state_mut().gui.mark_edited();
                json_str(serde_json::json!({
                    "index": idx,
                    "operation": label,
                }))
            }
            Err(e) => json_str(serde_json::json!({"error": format!("{e}")})),
        }
    }

    fn mcp_remove_toolpath(&mut self, index: usize) -> String {
        // Find the toolpath ID before removing
        let tp_id = self
            .controller
            .state()
            .session
            .toolpath_configs()
            .get(index)
            .map(|tc| tc.id);

        match self.controller.state_mut().session.remove_toolpath(index) {
            Ok(()) => {
                if let Some(id) = tp_id {
                    self.controller.state_mut().gui.toolpath_rt.remove(&id);
                }
                self.controller.state_mut().gui.mark_edited();
                self.controller.set_pending_upload();
                text(format!("Removed toolpath {index}"))
            }
            Err(e) => text(format!("Error: {e}")),
        }
    }

    fn mcp_add_tool(&mut self, name: &str, tool_type: &str, diameter: f64) -> String {
        let tt = match parse_tool_type(tool_type) {
            Ok(t) => t,
            Err(e) => return json_str(serde_json::json!({"error": e})),
        };

        let mut config = ToolConfig::new_default(ToolId(0), tt);
        config.name = name.to_owned();
        config.diameter = diameter;

        let idx = self.controller.state_mut().session.add_tool(config);
        self.controller.state_mut().gui.mark_edited();
        json_str(serde_json::json!({
            "index": idx,
            "tool_type": tool_type,
            "diameter": diameter,
        }))
    }

    fn mcp_remove_tool(&mut self, index: usize) -> String {
        match self.controller.state_mut().session.remove_tool(index) {
            Ok(()) => {
                self.controller.state_mut().gui.mark_edited();
                text(format!("Removed tool {index}"))
            }
            Err(e) => text(format!("Error: {e}")),
        }
    }

    fn mcp_set_stock_config(&mut self, x: f64, y: f64, z: f64) -> String {
        let mut stock = self.controller.state().session.stock_config().clone();
        stock.x = x;
        stock.y = y;
        stock.z = z;
        self.controller.state_mut().session.set_stock_config(stock);
        self.controller.state_mut().gui.mark_edited();
        text(format!(
            "Stock set to {x:.1} x {y:.1} x {z:.1} mm. Regenerate toolpaths and simulation to apply."
        ))
    }

    fn mcp_set_boundary_config(
        &mut self,
        index: usize,
        enabled: bool,
        source: Option<&str>,
        containment: Option<&str>,
        offset: Option<f64>,
    ) -> String {
        let boundary_source = match source {
            Some("stock") | None => BoundarySource::Stock,
            Some("model_silhouette") => BoundarySource::ModelSilhouette,
            Some(other) => {
                return json_str(serde_json::json!({
                    "error": format!("Unknown boundary source '{other}'. Use 'stock' or 'model_silhouette'.")
                }));
            }
        };

        let boundary_containment = match containment {
            Some("center") | None => BoundaryContainment::Center,
            Some("inside") => BoundaryContainment::Inside,
            Some("outside") => BoundaryContainment::Outside,
            Some(other) => {
                return json_str(serde_json::json!({
                    "error": format!("Unknown containment '{other}'. Use 'center', 'inside', or 'outside'.")
                }));
            }
        };

        let boundary = BoundaryConfig {
            enabled,
            source: boundary_source,
            containment: boundary_containment,
            offset: offset.unwrap_or(0.0),
        };

        match self
            .controller
            .state_mut()
            .session
            .set_boundary_config(index, boundary)
        {
            Ok(()) => {
                self.controller.state_mut().gui.mark_edited();
                text(format!(
                    "Boundary set on toolpath {index}. Regenerate to apply."
                ))
            }
            Err(e) => text(format!("Error: {e}")),
        }
    }

    fn mcp_set_dressup_config(&mut self, index: usize, dressup: serde_json::Value) -> String {
        let dressup_config: DressupConfig = match serde_json::from_value(dressup) {
            Ok(dc) => dc,
            Err(e) => {
                return json_str(serde_json::json!({
                    "error": format!("Invalid dressup config: {e}")
                }));
            }
        };

        match self
            .controller
            .state_mut()
            .session
            .set_dressup_config(index, dressup_config)
        {
            Ok(()) => {
                self.controller.state_mut().gui.mark_edited();
                text(format!(
                    "Dressup config set on toolpath {index}. Regenerate to apply."
                ))
            }
            Err(e) => text(format!("Error: {e}")),
        }
    }

    // ── Compute operations ───────────────────────────────────────────

    fn mcp_generate_toolpath(
        &mut self,
        index: usize,
        response_tx: tokio::sync::oneshot::Sender<McpResponse>,
    ) {
        // Find the toolpath ID from the index
        let session = &self.controller.state().session;
        let Some(tc) = session.toolpath_configs().get(index) else {
            let _ = response_tx.send(McpResponse {
                result: Ok(json_str(
                    serde_json::json!({"error": format!("Toolpath index {index} not found")}),
                )),
            });
            return;
        };
        let tp_id = ToolpathId(tc.id);

        // Push the generate event via the controller
        self.controller
            .events_mut()
            .push(crate::ui::AppEvent::GenerateToolpath(tp_id));

        // Store the oneshot sender for when the compute result arrives
        if let Some(ref mut pending) = self.controller.pending_mcp {
            pending.toolpath.insert(tp_id, response_tx);
        } else {
            // If pending_mcp is None, respond immediately with error
            let _ = response_tx.send(McpResponse {
                result: Err("MCP compute tracking not initialized".to_owned()),
            });
        }
    }

    fn mcp_generate_all(
        &mut self,
        response_tx: tokio::sync::oneshot::Sender<McpResponse>,
        progress_tx: Option<tokio::sync::mpsc::Sender<ProgressUpdate>>,
    ) {
        let ids: Vec<ToolpathId> = self
            .controller
            .state()
            .session
            .toolpath_configs()
            .iter()
            .filter(|tc| tc.enabled)
            .map(|tc| ToolpathId(tc.id))
            .collect();

        if ids.is_empty() {
            let _ = response_tx.send(McpResponse {
                result: Ok(text("No enabled toolpaths to generate")),
            });
            return;
        }

        let total = ids.len();
        self.mcp_send_progress(
            &progress_tx,
            &format!("Generating {total} toolpaths..."),
            0.0,
            Some(total as f64),
        );

        // Push generate events for each
        for &id in &ids {
            self.controller
                .events_mut()
                .push(crate::ui::AppEvent::GenerateToolpath(id));
        }

        // Store pending generate_all tracker
        if let Some(ref mut pending) = self.controller.pending_mcp {
            pending.generate_all = Some(PendingGenerateAll {
                remaining: ids,
                completed: 0,
                failed: 0,
                errors: Vec::new(),
                response_tx,
                progress_tx,
            });
        } else {
            let _ = response_tx.send(McpResponse {
                result: Err("MCP compute tracking not initialized".to_owned()),
            });
        }
    }

    fn mcp_run_simulation(
        &mut self,
        resolution: Option<f64>,
        response_tx: tokio::sync::oneshot::Sender<McpResponse>,
    ) {
        // Set resolution if provided
        if let Some(res) = resolution {
            self.controller.state_mut().simulation.resolution = res;
            self.controller.state_mut().simulation.auto_resolution = false;
        }

        // Always enable metrics when MCP triggers simulation — the standalone
        // MCP server hardcodes this, and diagnostics/cut_trace require it.
        self.controller
            .state_mut()
            .simulation
            .metric_options
            .enabled = true;

        // Push the simulation event
        self.controller
            .events_mut()
            .push(crate::ui::AppEvent::RunSimulation);

        // Store the oneshot sender
        if let Some(ref mut pending) = self.controller.pending_mcp {
            pending.simulation = Some(response_tx);
        } else {
            let _ = response_tx.send(McpResponse {
                result: Err("MCP compute tracking not initialized".to_owned()),
            });
        }
    }

    fn mcp_collision_check(
        &mut self,
        _index: usize,
        response_tx: tokio::sync::oneshot::Sender<McpResponse>,
    ) {
        // Push the collision check event
        self.controller
            .events_mut()
            .push(crate::ui::AppEvent::RunCollisionCheck);

        // Store the oneshot sender
        if let Some(ref mut pending) = self.controller.pending_mcp {
            pending.collision = Some(response_tx);
        } else {
            let _ = response_tx.send(McpResponse {
                result: Err("MCP compute tracking not initialized".to_owned()),
            });
        }
    }

    // ── Screenshot implementations ───────────────────────────────────

    fn mcp_screenshot_simulation(
        &self,
        path: &str,
        width: Option<u32>,
        height: Option<u32>,
        checkpoint: Option<usize>,
        include_toolpaths: Option<bool>,
    ) -> String {
        let sim_state = &self.controller.state().simulation;
        let Some(results) = sim_state.results.as_ref() else {
            return text("No simulation result. Run run_simulation first.");
        };

        if path.ends_with(".png") {
            let w = width.unwrap_or(1200);
            let h = height.unwrap_or(800);
            let cp_idx = checkpoint.unwrap_or_else(|| results.checkpoints.len().saturating_sub(1));
            let pixels = if let Some(cp) = results.checkpoints.get(cp_idx)
                && let Some(ref stock) = cp.stock
            {
                rs_cam_core::fingerprint::render_stock_composite(stock, w, h)
            } else {
                rs_cam_core::fingerprint::render_mesh_composite(&results.mesh, w, h)
            };
            match image::save_buffer(Path::new(path), &pixels, w, h, image::ColorType::Rgba8) {
                Ok(()) => text(format!(
                    "6-view composite exported to {path} ({w}x{h}, checkpoint {cp_idx})",
                )),
                Err(e) => text(format!("Failed to save PNG: {e}")),
            }
        } else {
            let session = &self.controller.state().session;
            let toolpaths: Vec<&rs_cam_core::toolpath::Toolpath> =
                if include_toolpaths.unwrap_or(true) {
                    self.controller
                        .state()
                        .gui
                        .toolpath_rt
                        .values()
                        .filter_map(|rt| rt.result.as_ref())
                        .map(|r| r.toolpath.as_ref())
                        .collect()
                } else {
                    Vec::new()
                };

            let html = rs_cam_core::viz::stock_mesh_to_3d_html(
                &results.mesh,
                &toolpaths,
                &format!("{} -- Simulation", session.name()),
            );

            match std::fs::write(path, &html) {
                Ok(()) => text(format!(
                    "Simulation view exported to {path} ({} vertices, {} triangles)",
                    results.mesh.vertex_count(),
                    results.mesh.indices.len() / 3,
                )),
                Err(e) => text(format!("Failed to write: {e}")),
            }
        }
    }

    fn mcp_screenshot_toolpath(
        &self,
        index: usize,
        path: &str,
        width: Option<u32>,
        height: Option<u32>,
        show_stock: Option<bool>,
        include_rapids: Option<bool>,
    ) -> String {
        // Find the toolpath result from GUI runtime
        let session = &self.controller.state().session;
        let gui = &self.controller.state().gui;
        let Some(tc) = session.toolpath_configs().get(index) else {
            return text(format!("Toolpath {index} not found."));
        };

        let result = gui
            .toolpath_rt
            .get(&tc.id)
            .and_then(|rt| rt.result.as_ref());
        let Some(result) = result else {
            return text(format!(
                "Toolpath {index} not generated. Run generate_toolpath first."
            ));
        };

        if path.ends_with(".png") {
            let w = width.unwrap_or(1200);
            let h = height.unwrap_or(800);
            let bg = if show_stock.unwrap_or(false) {
                self.controller
                    .state()
                    .simulation
                    .results
                    .as_ref()
                    .map(|sim| {
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
            match image::save_buffer(Path::new(path), &pixels, w, h, image::ColorType::Rgba8) {
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

            match std::fs::write(path, &html) {
                Ok(()) => text(format!(
                    "Toolpath view exported to {path} ({} moves, {:.0}mm cutting)",
                    result.toolpath.moves.len(),
                    result.stats.cutting_distance,
                )),
                Err(e) => text(format!("Failed to write: {e}")),
            }
        }
    }

    // ── Simulation scrubbing implementations ────────────────────────

    fn mcp_sim_jump_to_move(&mut self, move_index: usize) -> String {
        let sim = &self.controller.state().simulation;
        if !sim.has_results() {
            return json_str(
                serde_json::json!({"error": "No simulation result. Run run_simulation first."}),
            );
        }
        self.controller
            .events_mut()
            .push(AppEvent::SimJumpToMove(move_index));
        self.mcp_sim_playback_state(move_index)
    }

    fn mcp_sim_jump_to_start(&mut self) -> String {
        let sim = &self.controller.state().simulation;
        if !sim.has_results() {
            return json_str(
                serde_json::json!({"error": "No simulation result. Run run_simulation first."}),
            );
        }
        self.controller.events_mut().push(AppEvent::SimJumpToStart);
        self.mcp_sim_playback_state(0)
    }

    fn mcp_sim_jump_to_end(&mut self) -> String {
        let sim = &self.controller.state().simulation;
        if !sim.has_results() {
            return json_str(
                serde_json::json!({"error": "No simulation result. Run run_simulation first."}),
            );
        }
        let total = sim.total_moves();
        self.controller.events_mut().push(AppEvent::SimJumpToEnd);
        self.mcp_sim_playback_state(total)
    }

    /// Scrub to a percentage position within a specific toolpath.
    fn mcp_sim_scrub_toolpath(&mut self, index: usize, percent: f64) -> Result<String, String> {
        let sim = &self.controller.state().simulation;
        if !sim.has_results() {
            return Err("No simulation result. Run run_simulation first.".to_owned());
        }

        let boundaries = sim.boundaries();
        let boundary = boundaries.get(index).ok_or_else(|| {
            format!(
                "Toolpath index {index} not found in simulation boundaries (have {})",
                boundaries.len()
            )
        })?;

        let clamped_percent = percent.clamp(0.0, 100.0);
        let start = boundary.start_move;
        let end = boundary.end_move;
        let range = end.saturating_sub(start);
        let move_index = start + (range as f64 * clamped_percent / 100.0) as usize;
        let tp_name = boundary.name.clone();
        let total_moves_in_toolpath = range;

        self.controller
            .events_mut()
            .push(AppEvent::SimJumpToMove(move_index));

        Ok(json_str(serde_json::json!({
            "move_index": move_index,
            "toolpath_name": tp_name,
            "percent": clamped_percent,
            "total_moves_in_toolpath": total_moves_in_toolpath,
            "start_move": start,
            "end_move": end,
        })))
    }

    /// Build a JSON response with the current simulation playback state.
    fn mcp_sim_playback_state(&self, move_index: usize) -> String {
        let sim = &self.controller.state().simulation;
        let total = sim.total_moves();
        let clamped = move_index.min(total);

        // Find which toolpath is active at this move index.
        let active_toolpath = sim
            .boundaries()
            .iter()
            .find(|b| clamped >= b.start_move && clamped <= b.end_move)
            .map(|b| {
                serde_json::json!({
                    "name": b.name,
                    "tool": b.tool_name,
                    "start_move": b.start_move,
                    "end_move": b.end_move,
                })
            });

        // Find the checkpoint index (boundary index of the nearest completed toolpath).
        let checkpoint = sim
            .boundaries()
            .iter()
            .enumerate()
            .filter(|(_, b)| clamped >= b.end_move)
            .map(|(i, _)| i)
            .next_back();

        json_str(serde_json::json!({
            "move_index": clamped,
            "total_moves": total,
            "active_toolpath": active_toolpath,
            "checkpoint": checkpoint,
        }))
    }
}
