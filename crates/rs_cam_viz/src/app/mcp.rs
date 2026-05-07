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
                span_kind,
                span_id,
                pass_index,
            } => {
                let resp = self.mcp_get_cut_trace(
                    toolpath_id,
                    max_hotspots,
                    max_issues,
                    span_kind.as_deref(),
                    span_id,
                    pass_index,
                );
                let _ = response_tx.send(McpResponse { result: Ok(resp) });
            }
            McpRequestKind::GetGenerationDebugTrace {
                index,
                span_kind,
                exit_reason,
                max_yield_ratio,
                max_spans,
            } => {
                let resp = self.mcp_get_generation_debug_trace(
                    index,
                    span_kind.as_deref(),
                    exit_reason.as_deref(),
                    max_yield_ratio,
                    max_spans,
                );
                let _ = response_tx.send(McpResponse { result: Ok(resp) });
            }
            McpRequestKind::NarrateToolpath { index } => {
                let resp = self.mcp_narrate_toolpath(index);
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
            McpRequestKind::InspectSpans { index } => {
                let resp = self.mcp_inspect_spans(index);
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
            McpRequestKind::AddSetup { name } => {
                self.controller
                    .push_notification("MCP: Adding setup".to_owned(), Severity::Info);
                let resp = self.mcp_add_setup(name.as_deref());
                let _ = response_tx.send(McpResponse { result: Ok(resp) });
            }
            McpRequestKind::SetSetupFace {
                setup_index,
                face_up,
            } => {
                self.controller.push_notification(
                    format!("MCP: Set setup {setup_index} face to '{face_up}'"),
                    Severity::Info,
                );
                self.controller
                    .events_mut()
                    .push(crate::ui::AppEvent::SwitchWorkspace(
                        crate::state::Workspace::Setup,
                    ));
                let resp = self.mcp_set_setup_face(setup_index, &face_up);
                let _ = response_tx.send(McpResponse { result: Ok(resp) });
            }
            McpRequestKind::MoveToolpathToSetup {
                toolpath_index,
                target_setup_index,
            } => {
                self.controller.push_notification(
                    format!("MCP: Moving toolpath {toolpath_index} to setup {target_setup_index}"),
                    Severity::Info,
                );
                let resp = self.mcp_move_toolpath_to_setup(toolpath_index, target_setup_index);
                let _ = response_tx.send(McpResponse { result: Ok(resp) });
            }
            McpRequestKind::ImportModel { path } => {
                let name = Path::new(&path)
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or(&path)
                    .to_owned();
                self.controller
                    .push_notification(format!("MCP: Importing '{name}'"), Severity::Info);
                let resp = self.mcp_import_model(&path);
                let _ = response_tx.send(McpResponse { result: Ok(resp) });
            }
            McpRequestKind::LoadProject { path } => {
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
            McpRequestKind::ExportGcode {
                path,
                accept_unmodeled_tool_load,
                accept_exceeded_tool_load,
            } => {
                let resp = self.mcp_export_gcode(
                    &path,
                    accept_unmodeled_tool_load,
                    accept_exceeded_tool_load,
                );
                let _ = response_tx.send(McpResponse { result: Ok(resp) });
            }
            McpRequestKind::GetToolLoadReport => {
                let resp = self.mcp_get_tool_load_report();
                let _ = response_tx.send(McpResponse { result: Ok(resp) });
            }
            McpRequestKind::OptimizeToolpath { index } => {
                let resp = self.mcp_optimize_toolpath(index);
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
            McpRequestKind::SetDressupField { index, key, value } => {
                let resp = self.mcp_set_dressup_field(index, &key, value);
                let _ = response_tx.send(McpResponse { result: Ok(resp) });
            }
            McpRequestKind::SetToolpathEnabled { index, enabled } => {
                let resp = self.mcp_set_toolpath_enabled(index, enabled);
                let _ = response_tx.send(McpResponse { result: Ok(resp) });
            }
            McpRequestKind::SetStockSource { index, source } => {
                let resp = self.mcp_set_stock_source(index, &source);
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

    fn mcp_narrate_toolpath(&self, index: usize) -> String {
        let state = self.controller.state();
        let Some(tc) = state.session.get_toolpath_config(index) else {
            return format!("Error: Toolpath index {index} not found");
        };
        let Some(rt) = state.gui.toolpath_rt.get(&tc.id) else {
            return format!("Error: Toolpath {index} not generated. Run generate_toolpath first.");
        };
        let Some(result) = rt.result.as_ref() else {
            return format!("Error: Toolpath {index} not generated. Run generate_toolpath first.");
        };
        let Some(tool_config) = state
            .session
            .tools()
            .iter()
            .find(|tool| tool.id.0 == tc.tool_id)
            .or_else(|| state.session.tools().first())
        else {
            return "Error: no tools are configured for this project".to_owned();
        };

        let tool = rs_cam_core::compute::build_cutter(tool_config);
        let cut_trace = state
            .simulation
            .results
            .as_ref()
            .and_then(|sim| sim.cut_trace.as_deref());
        let semantic_trace = rt
            .semantic_trace
            .as_deref()
            .or(result.semantic_trace.as_deref());
        let debug_trace = rt.debug_trace.as_deref().or(result.debug_trace.as_deref());
        let context = rs_cam_core::narrate::ToolpathNarrationContext {
            toolpath_id: Some(tc.id),
            toolpath_name: Some(tc.name.as_str()),
            operation_label: Some(tc.operation.label()),
            depth_per_pass_mm: tc.operation.depth_per_pass(),
            stepover_mm: tc.operation.stepover(),
            tool_diameter_mm: Some(tool_config.diameter),
            feed_rate_mm_min: Some(tc.operation.feed_rate()),
            spindle_rpm: Some(
                tc.operation
                    .spindle_rpm()
                    .unwrap_or(state.session.post_config().spindle_speed),
            ),
            flute_count: Some(tool_config.flute_count),
        };

        rs_cam_core::narrate::narrate_toolpath_with_context(
            result.annotated.as_ref(),
            semantic_trace,
            cut_trace,
            debug_trace,
            &tool,
            &context,
        )
    }

    fn mcp_get_cut_trace(
        &self,
        toolpath_id: Option<usize>,
        max_hotspots: Option<usize>,
        max_issues: Option<usize>,
        span_kind: Option<&str>,
        span_id: Option<u32>,
        pass_index: Option<u32>,
    ) -> String {
        use rs_cam_core::toolpath_spans::{SpanId, SpanPayload};

        let state = self.controller.state();
        let sim_state = &state.simulation;
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

        // Translate the span filter args into a per-toolpath set of accepted
        // SpanIds. A span_path matches when it contains any accepted SpanId
        // (or the filter is unset).
        //
        // Filter resolution requires the AnnotatedToolpath to look up
        // SpanKind / SpanPayload for each span index. When toolpath_id is
        // unset and any span filter is set, we resolve per toolpath.
        let want_kind = span_kind
            .map(parse_span_kind_filter)
            .transpose()
            .unwrap_or(None);
        let span_filter_active = span_kind.is_some() || span_id.is_some() || pass_index.is_some();
        let resolve_accepted = |tp_index_for_id: usize| -> Option<std::collections::HashSet<u32>> {
            if !span_filter_active {
                return None;
            }
            let tc = state.session.get_toolpath_config(tp_index_for_id)?;
            let rt = state.gui.toolpath_rt.get(&tc.id)?;
            let result = rt.result.as_ref()?;
            let spans = result.spans();
            let mut set = std::collections::HashSet::<u32>::new();
            for (i, span) in spans.iter().enumerate() {
                let id = i as u32;
                let mut accept = true;
                if let Some(kind) = want_kind {
                    accept &= span.kind == kind;
                }
                if let Some(want_id) = span_id {
                    accept &= id == want_id;
                }
                if let Some(want_pi) = pass_index {
                    accept &= matches!(
                        &span.payload,
                        Some(SpanPayload::DepthPass { pass_index, .. }) if *pass_index == want_pi
                    );
                }
                if accept {
                    set.insert(id);
                }
            }
            Some(set)
        };
        // Build a {toolpath_id_raw → accepted SpanId set}. Toolpath_id arg is
        // the project-level raw id (matching SimulationCutSample.toolpath_id),
        // while accepted-set lookup needs the index — translate via session.
        let mut accepted_by_toolpath: std::collections::HashMap<
            usize,
            Option<std::collections::HashSet<u32>>,
        > = std::collections::HashMap::new();
        if span_filter_active {
            let n = state.session.toolpath_count();
            for idx in 0..n {
                if let Some(tc) = state.session.get_toolpath_config(idx)
                    && toolpath_id.is_none_or(|raw_id| tc.id == raw_id)
                {
                    accepted_by_toolpath.insert(tc.id, resolve_accepted(idx));
                }
            }
        }
        let span_path_matches = |tp_id: usize, path: &[SpanId]| -> bool {
            if !span_filter_active {
                return true;
            }
            match accepted_by_toolpath.get(&tp_id) {
                Some(Some(accepted)) => path.iter().any(|sid| accepted.contains(&sid.0)),
                _ => false,
            }
        };

        let summaries: Vec<&_> = ct
            .semantic_summaries
            .iter()
            .filter(|s| toolpath_id.is_none_or(|id| s.toolpath_id == id))
            .collect();
        let hotspots: Vec<&_> = ct
            .hotspots
            .iter()
            .filter(|h| toolpath_id.is_none_or(|id| h.toolpath_id == id))
            .filter(|h| span_path_matches(h.toolpath_id, &h.span_path))
            .collect();
        let issues: Vec<&_> = ct
            .issues
            .iter()
            .filter(|i| toolpath_id.is_none_or(|id| i.toolpath_id == id))
            .filter(|i| span_path_matches(i.toolpath_id, &i.span_path))
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

    fn mcp_get_generation_debug_trace(
        &self,
        index: usize,
        span_kind: Option<&str>,
        exit_reason: Option<&str>,
        max_yield_ratio: Option<f64>,
        max_spans: Option<usize>,
    ) -> String {
        let state = self.controller.state();
        let Some(tc) = state.session.get_toolpath_config(index) else {
            return json_str(
                serde_json::json!({"error": format!("Toolpath index {index} not found")}),
            );
        };
        let Some(rt) = state.gui.toolpath_rt.get(&tc.id) else {
            return json_str(serde_json::json!({
                "error": format!("Toolpath {index} not generated. Run generate_toolpath first.")
            }));
        };
        let Some(trace) = rt.debug_trace.as_ref() else {
            return json_str(serde_json::json!({
                "error": format!("Toolpath {index} has no debug trace — the operation generator didn't capture one.")
            }));
        };

        // span_kind accepts EITHER a generation-debug string (e.g.
        // "adaptive_pass", "z_level_clear", "preflight") OR a structural
        // SpanKind synonym in snake_case (e.g. "depth_pass", "entry"). The
        // latter expands to the set of debug-trace kinds that participate in
        // that structural span. This unifies the agent vocabulary across
        // get_cut_trace + inspect_spans + get_generation_debug_trace.
        let kind_filter: Box<dyn Fn(&str) -> bool> = match span_kind {
            None => Box::new(|_| true),
            Some(needle) => {
                let synonyms = expand_span_kind_synonyms(needle);
                Box::new(move |k: &str| synonyms.iter().any(|s| s == k))
            }
        };
        let limit = max_spans.unwrap_or(100);
        let filtered: Vec<_> = trace
            .spans
            .iter()
            .filter(|s| kind_filter(s.kind.as_str()))
            .filter(|s| {
                exit_reason.is_none_or(|needle| {
                    s.exit_reason.as_deref().is_some_and(|r| r.contains(needle))
                })
            })
            .filter(|s| {
                max_yield_ratio
                    .is_none_or(|max_y| s.counters.get("yield_ratio").is_some_and(|&y| y <= max_y))
            })
            .collect();
        let total_matching = filtered.len();
        let visible_spans: Vec<_> = if limit == 0 {
            filtered.clone()
        } else {
            filtered.iter().copied().take(limit).collect()
        };
        // Each returned span is enriched with `span_kind_hint`: the
        // structural `SpanKind` (snake_case) that this generation-time span
        // contributes to, or null when there's no clean mapping (e.g.
        // op-internal "preflight", "widen_band").
        let visible: Vec<serde_json::Value> = visible_spans
            .iter()
            .map(|span| {
                let mut value =
                    serde_json::to_value(span).unwrap_or_else(|_| serde_json::json!({}));
                if let serde_json::Value::Object(map) = &mut value {
                    map.insert(
                        "span_kind_hint".into(),
                        serde_json::json!(map_debug_kind_to_span_kind(&span.kind)),
                    );
                }
                value
            })
            .collect();

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
        // Arc-quality aggregates across all adaptive_pass spans:
        let mut mean_delta_sum = 0.0f64;
        let mut mean_delta_count = 0usize;
        let mut sinuosity_sum = 0.0f64;
        let mut sinuosity_count = 0usize;
        let mut max_sinuosity = 0.0f64;
        let mut max_sinuosity_span_id: Option<u64> = None;
        let mut zigzag_passes = 0usize; // sign_flip_rate > 0.3
        // Engagement aggregates
        let mut engagement_sum = 0.0f64;
        let mut engagement_count = 0usize;
        let mut global_max_engagement = 0.0f64;
        let mut high_engagement_passes = 0usize; // max_engagement > 0.5
        let mut over_target_sum = 0.0f64;
        let mut target_frac_first: Option<f64> = None;
        let mut worst: Vec<(f64, u64, &rs_cam_core::debug_trace::ToolpathDebugSpan)> = Vec::new();
        let mut worst_arc: Vec<(f64, &rs_cam_core::debug_trace::ToolpathDebugSpan)> = Vec::new();
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
            if let Some(&d) = span.counters.get("mean_angle_delta") {
                mean_delta_sum += d;
                mean_delta_count += 1;
                worst_arc.push((d, span));
            }
            if let Some(&s) = span.counters.get("sinuosity") {
                sinuosity_sum += s;
                sinuosity_count += 1;
                if s > max_sinuosity {
                    max_sinuosity = s;
                    max_sinuosity_span_id = Some(span.id);
                }
            }
            if span.counters.get("sign_flip_rate").copied().unwrap_or(0.0) > 0.3 {
                zigzag_passes += 1;
            }
            if let Some(&me) = span.counters.get("mean_engagement") {
                engagement_sum += me;
                engagement_count += 1;
            }
            if let Some(&mx) = span.counters.get("max_engagement") {
                if mx > global_max_engagement {
                    global_max_engagement = mx;
                }
                if mx > 0.5 {
                    high_engagement_passes += 1;
                }
            }
            if let Some(&ot) = span.counters.get("over_target_rate") {
                over_target_sum += ot;
            }
            if target_frac_first.is_none()
                && let Some(&tf) = span.counters.get("target_frac")
            {
                target_frac_first = Some(tf);
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
        worst_arc.sort_by(|a, b| b.0.total_cmp(&a.0)); // descending — worst first
        let worst_arc_json: Vec<_> = worst_arc
            .iter()
            .take(10)
            .map(|(d, span)| {
                serde_json::json!({
                    "id": span.id,
                    "label": span.label,
                    "exit_reason": span.exit_reason,
                    "mean_angle_delta": d,
                    "angle_delta_std": span.counters.get("angle_delta_std").copied().unwrap_or(0.0),
                    "sign_flip_rate": span.counters.get("sign_flip_rate").copied().unwrap_or(0.0),
                    "sinuosity": span.counters.get("sinuosity").copied().unwrap_or(0.0),
                    "step_count": span.counters.get("step_count").copied().unwrap_or(0.0),
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
                "arc_quality": {
                    "avg_mean_angle_delta": if mean_delta_count > 0 { mean_delta_sum / mean_delta_count as f64 } else { 0.0 },
                    "avg_sinuosity": if sinuosity_count > 0 { sinuosity_sum / sinuosity_count as f64 } else { 0.0 },
                    "max_sinuosity": max_sinuosity,
                    "max_sinuosity_span_id": max_sinuosity_span_id,
                    "zigzag_passes": zigzag_passes,
                    "worst_arc_passes": worst_arc_json,
                },
                "engagement": {
                    "target_frac": target_frac_first,
                    "avg_mean_engagement": if engagement_count > 0 { engagement_sum / engagement_count as f64 } else { 0.0 },
                    "max_engagement": global_max_engagement,
                    "high_engagement_passes": high_engagement_passes,
                    "avg_over_target_rate": if engagement_count > 0 { over_target_sum / engagement_count as f64 } else { 0.0 },
                },
            },
            "spans_returned": visible.len(),
            "spans_total_matching": total_matching,
            "spans": visible,
            "hotspots": trace.hotspots,
            "annotations": trace.annotations,
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

    fn mcp_inspect_spans(&self, index: usize) -> String {
        use rs_cam_core::toolpath_spans::SpanKind;

        let session = &self.controller.state().session;
        let gui = &self.controller.state().gui;
        let Some(tc) = session.toolpath_configs().get(index) else {
            return text(format!("Toolpath {index} not found."));
        };
        let Some(rt) = gui.toolpath_rt.get(&tc.id) else {
            return text(format!(
                "Toolpath {index} has no runtime entry. Run generate_toolpath first."
            ));
        };
        let Some(result) = rt.result.as_ref() else {
            return text(format!(
                "Toolpath {index} not generated. Run generate_toolpath first."
            ));
        };

        let n_moves = result.toolpath().moves.len();
        let spans = result.spans();

        let kind_label = |k: SpanKind| -> &'static str {
            match k {
                SpanKind::Operation => "Operation",
                SpanKind::DepthPass => "DepthPass",
                SpanKind::Region => "Region",
                SpanKind::Entry => "Entry",
                SpanKind::LeadOut => "LeadOut",
                SpanKind::LinkBridge => "LinkBridge",
                SpanKind::DressupArtifact => "DressupArtifact",
                SpanKind::RapidOrderBarrier => "RapidOrderBarrier",
            }
        };

        // Per-kind tally for the summary row.
        let mut kind_counts: std::collections::BTreeMap<&'static str, usize> =
            std::collections::BTreeMap::new();
        for s in spans {
            *kind_counts.entry(kind_label(s.kind)).or_insert(0) += 1;
        }

        let spans_json: Vec<serde_json::Value> = spans
            .iter()
            .enumerate()
            .map(|(id, s)| {
                serde_json::json!({
                    "id": id,
                    "kind": kind_label(s.kind),
                    "start_move": s.start_move,
                    "end_move": s.end_move,
                    "is_boundary": s.is_boundary(),
                    "label": &*s.label,
                    "payload": s.payload.as_ref().map(|p| format!("{p:?}")),
                })
            })
            .collect();

        json_str(serde_json::json!({
            "toolpath_id": tc.id,
            "toolpath_index": index,
            "name": tc.name,
            "operation": tc.operation.label(),
            "move_count": n_moves,
            "span_count": spans.len(),
            "spans_valid": result.spans_valid(),
            "kind_counts": kind_counts,
            "spans": spans_json,
        }))
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
        let added = self
            .controller
            .state_mut()
            .session
            .add_alignment_pin(x, y, diameter);
        let pin_count = self
            .controller
            .state()
            .session
            .stock_config()
            .alignment_pins
            .len();
        self.controller.state_mut().gui.mark_edited();
        self.controller.set_pending_upload();
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

    fn mcp_remove_alignment_pin(&mut self, index: usize) -> String {
        let result = self
            .controller
            .state_mut()
            .session
            .remove_alignment_pin(index);
        match result {
            Ok(()) => {
                self.controller.state_mut().gui.mark_edited();
                self.controller.set_pending_upload();
                let pin_count = self
                    .controller
                    .state()
                    .session
                    .stock_config()
                    .alignment_pins
                    .len();
                json_str(serde_json::json!({
                    "ok": true,
                    "message": format!("Removed alignment pin {index}"),
                    "pin_count": pin_count,
                }))
            }
            Err(e) => json_str(serde_json::json!({ "error": e.to_string() })),
        }
    }

    // ── Mutation implementations ─────────────────────────────────────

    fn mcp_add_setup(&mut self, name: Option<&str>) -> String {
        self.controller.handle_add_setup();
        let setup_count = self.controller.state().session.list_setups().len();
        let setup_id = self
            .controller
            .state()
            .session
            .list_setups()
            .last()
            .map(|s| s.id);
        let Some(sid) = setup_id else {
            return json_str(serde_json::json!({"error": "Failed to add setup"}));
        };
        if let Some(n) = name
            && let Some((_, sd)) = self
                .controller
                .state_mut()
                .session
                .find_setup_by_id_mut(sid)
        {
            sd.name = n.to_owned();
        }
        let final_name = self
            .controller
            .state()
            .session
            .find_setup_by_id(sid)
            .map(|(_, s)| s.name.clone())
            .unwrap_or_default();
        json_str(serde_json::json!({
            "index": setup_count - 1,
            "id": sid,
            "name": final_name,
        }))
    }

    fn mcp_set_setup_face(&mut self, setup_index: usize, face_up: &str) -> String {
        let setups = self.controller.state().session.list_setups();
        let Some(setup) = setups.get(setup_index) else {
            return json_str(
                serde_json::json!({"error": format!("Setup index {setup_index} not found")}),
            );
        };
        let setup_id = setup.id;

        let face = match face_up.to_lowercase().as_str() {
            "top" => rs_cam_core::compute::transform::FaceUp::Top,
            "bottom" => rs_cam_core::compute::transform::FaceUp::Bottom,
            "front" => rs_cam_core::compute::transform::FaceUp::Front,
            "back" => rs_cam_core::compute::transform::FaceUp::Back,
            "left" => rs_cam_core::compute::transform::FaceUp::Left,
            "right" => rs_cam_core::compute::transform::FaceUp::Right,
            _ => {
                return json_str(serde_json::json!({
                    "error": format!("Unknown face '{face_up}'. Use: top, bottom, front, back, left, right")
                }));
            }
        };

        if let Some((_, sd)) = self
            .controller
            .state_mut()
            .session
            .find_setup_by_id_mut(setup_id)
        {
            sd.face_up = face;
            self.controller.state_mut().gui.mark_edited();
            self.controller.set_pending_upload();
            json_str(serde_json::json!({
                "setup_index": setup_index,
                "face_up": face_up.to_lowercase(),
            }))
        } else {
            json_str(serde_json::json!({"error": "Setup not found"}))
        }
    }

    fn mcp_move_toolpath_to_setup(
        &mut self,
        toolpath_index: usize,
        target_setup_index: usize,
    ) -> String {
        let session = &self.controller.state().session;
        let Some(tc) = session.toolpath_configs().get(toolpath_index) else {
            return json_str(
                serde_json::json!({"error": format!("Toolpath index {toolpath_index} not found")}),
            );
        };
        let tp_id = crate::state::toolpath::ToolpathId(tc.id);
        let Some(target_setup) = session.list_setups().get(target_setup_index) else {
            return json_str(
                serde_json::json!({"error": format!("Setup index {target_setup_index} not found")}),
            );
        };
        let target_setup_id = crate::state::job::SetupId(target_setup.id);

        self.controller
            .events_mut()
            .push(crate::ui::AppEvent::MoveToolpathToSetup(
                tp_id,
                target_setup_id,
                0,
            ));
        self.controller.state_mut().gui.mark_edited();

        json_str(serde_json::json!({
            "toolpath_index": toolpath_index,
            "target_setup_index": target_setup_index,
        }))
    }

    fn mcp_import_model(&mut self, path: &str) -> String {
        let file_path = Path::new(path);
        let ext = file_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        let result = match ext.as_str() {
            "stl" => self.controller.import_stl_path(file_path),
            "dxf" => self.controller.import_dxf_path(file_path),
            "svg" => self.controller.import_svg_path(file_path),
            "step" | "stp" => self.controller.import_step_path(file_path),
            _ => {
                return json_str(serde_json::json!({
                    "error": format!("Unsupported file format '.{ext}'. Use .stl, .dxf, .svg, .step, or .stp")
                }));
            }
        };

        match result {
            Ok(bbox) => {
                // Find the most recently added model to report its details.
                let models = self.controller.state().session.models();
                let model = models.last();
                let name = model.map(|m| m.name.as_str()).unwrap_or("unknown");
                let id = model.map(|m| m.id).unwrap_or(0);
                let kind = model
                    .and_then(|m| m.kind)
                    .map(|k| format!("{k:?}"))
                    .unwrap_or_else(|| ext.clone());

                let mut resp = serde_json::json!({
                    "id": id,
                    "name": name,
                    "kind": kind.to_lowercase(),
                });

                if let Some(bbox) = bbox {
                    // SAFETY: resp is a known JSON object we just constructed
                    #[allow(clippy::indexing_slicing)]
                    {
                        resp["bbox"] = serde_json::json!({
                            "min": [bbox.min.x, bbox.min.y, bbox.min.z],
                            "max": [bbox.max.x, bbox.max.y, bbox.max.z],
                        });
                        resp["dimensions"] = serde_json::json!({
                            "x": bbox.max.x - bbox.min.x,
                            "y": bbox.max.y - bbox.min.y,
                            "z": bbox.max.z - bbox.min.z,
                        });
                    }
                }

                json_str(resp)
            }
            Err(e) => json_str(serde_json::json!({"error": format!("{e}")})),
        }
    }

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

    fn mcp_export_gcode(
        &self,
        path: &str,
        accept_unmodeled_tool_load: bool,
        accept_exceeded_tool_load: bool,
    ) -> String {
        let session = &self.controller.state().session;
        let policy = rs_cam_core::gcode::ToolLoadExportPolicy {
            accept_unmodeled: accept_unmodeled_tool_load,
            accept_exceeded: accept_exceeded_tool_load,
        };
        match session.export_gcode_with_policy(Path::new(path), None, policy) {
            Ok(()) => text(format!("G-code exported to {path}")),
            Err(e) => text(format!("Export failed: {e}")),
        }
    }

    fn mcp_get_tool_load_report(&self) -> String {
        let state = self.controller.state();
        // The cut trace is held in viz simulation state, not in
        // `session.simulation`. Pull it from there so chipload/power can be
        // evaluated against the active simulation run.
        let sim_trace = state
            .simulation
            .results
            .as_ref()
            .and_then(|r| r.cut_trace.as_deref());
        let report = rs_cam_core::gcode::project_load_report(&state.session, sim_trace);

        // Per-DepthPass MRR/feed/engagement histogram (S2.5). Keyed by
        // toolpath raw id, then a list of one entry per `SpanKind::DepthPass`
        // span, in span order (pass 0, pass 1, …). Lets agents distinguish
        // the high-DOC first pass from the steady-state passes for any
        // multi-pass operation without re-grouping the raw sample stream.
        let per_depth_pass = build_per_depth_pass_summary(state, sim_trace);

        let load_value = serde_json::to_value(&report).unwrap_or(serde_json::Value::Null);
        json_str(serde_json::json!({
            "load_report": load_value,
            "per_depth_pass": per_depth_pass,
        }))
    }

    /// Run the optimizer on a single toolpath synchronously and
    /// return the OptimizeOutcome as JSON. The GUI thread blocks
    /// for the duration of the search (~1-2 min). MCP automation
    /// expects this — the LLM/agent waits on the response.
    fn mcp_optimize_toolpath(&mut self, index: usize) -> String {
        let trace_clone = self
            .controller
            .state()
            .simulation
            .results
            .as_ref()
            .and_then(|r| r.cut_trace.clone());
        let Some(trace) = trace_clone else {
            return json_str(serde_json::json!({
                "error": "Run a simulation first — optimize_toolpath needs a baseline trace.",
            }));
        };
        let cancel = std::sync::atomic::AtomicBool::new(false);
        let outcome = rs_cam_core::tool_load::optimize::optimize_toolpath(
            &mut self.controller.state_mut().session,
            &trace,
            index,
            &cancel,
        );
        match serde_json::to_value(&outcome) {
            Ok(v) => json_str(v),
            Err(e) => json_str(serde_json::json!({
                "error": format!("Failed to serialize optimize outcome: {e}")
            })),
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

    fn mcp_set_dressup_field(
        &mut self,
        index: usize,
        key: &str,
        value: serde_json::Value,
    ) -> String {
        match self
            .controller
            .state_mut()
            .session
            .set_dressup_field(index, key, value)
        {
            Ok(()) => {
                self.controller.state_mut().gui.mark_edited();
                text(format!(
                    "Dressup field '{key}' set on toolpath {index}. Regenerate to apply."
                ))
            }
            Err(e) => text(format!("Error: {e}")),
        }
    }

    fn mcp_set_toolpath_enabled(&mut self, index: usize, enabled: bool) -> String {
        match self
            .controller
            .state_mut()
            .session
            .set_toolpath_enabled(index, enabled)
        {
            Ok(()) => {
                self.controller.state_mut().gui.mark_edited();
                text(format!(
                    "Toolpath {index} {}",
                    if enabled { "enabled" } else { "disabled" }
                ))
            }
            Err(e) => text(format!("Error: {e}")),
        }
    }

    fn mcp_set_stock_source(&mut self, index: usize, source: &str) -> String {
        let parsed = match source {
            "fresh" => rs_cam_core::compute::config::StockSource::Fresh,
            "from_remaining_stock" => rs_cam_core::compute::config::StockSource::FromRemainingStock,
            other => {
                return text(format!(
                    "Error: unknown stock_source '{other}'. Expected 'fresh' or 'from_remaining_stock'."
                ));
            }
        };
        match self
            .controller
            .state_mut()
            .session
            .set_stock_source(index, parsed)
        {
            Ok(()) => {
                self.controller.state_mut().gui.mark_edited();
                text(format!(
                    "Stock source set to '{source}' on toolpath {index}. Regenerate to apply."
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

        // MCP diagnostics depend on generation debug + semantic traces; enable
        // capture before queuing compute so get_generation_debug_trace and
        // narrate_toolpath have structured planner data.
        if let Some(tc) = self
            .controller
            .state_mut()
            .session
            .toolpath_configs_mut()
            .get_mut(index)
        {
            tc.debug_options.enabled = true;
        }

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
        for tc in self.controller.state_mut().session.toolpath_configs_mut() {
            if tc.enabled {
                tc.debug_options.enabled = true;
            }
        }

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
        // `capture_arc_engagement` is also forced on so the tool-load `power`
        // criterion can evaluate against the run.
        let metric_options = &mut self.controller.state_mut().simulation.metric_options;
        metric_options.enabled = true;
        metric_options.capture_arc_engagement = true;

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
                        .map(|r| r.toolpath())
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
                result.toolpath(),
                bg.as_ref(),
                w,
                h,
                include_rapids.unwrap_or(true),
            );
            match image::save_buffer(Path::new(path), &pixels, w, h, image::ColorType::Rgba8) {
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

            match std::fs::write(path, &html) {
                Ok(()) => text(format!(
                    "Toolpath view exported to {path} ({} moves, {:.0}mm cutting)",
                    result.toolpath().moves.len(),
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

/// Build the per-DepthPass histogram for [`mcp_get_tool_load_report`].
/// Returns `{ "<toolpath_raw_id>": [ { ... }, … ] }` keyed by stringified
/// toolpath id. Toolpaths without DepthPass spans or without samples in the
/// trace are omitted.
fn build_per_depth_pass_summary(
    state: &crate::state::AppState,
    sim_trace: Option<&rs_cam_core::simulation_cut::SimulationCutTrace>,
) -> serde_json::Value {
    use rs_cam_core::toolpath_spans::{SpanId, SpanKind, SpanPayload};

    let Some(trace) = sim_trace else {
        return serde_json::Value::Null;
    };

    let mut out = serde_json::Map::<String, serde_json::Value>::new();
    let n = state.session.toolpath_count();
    for idx in 0..n {
        let Some(tc) = state.session.get_toolpath_config(idx) else {
            continue;
        };
        let Some(rt) = state.gui.toolpath_rt.get(&tc.id) else {
            continue;
        };
        let Some(result) = rt.result.as_ref() else {
            continue;
        };
        let spans = result.spans();
        if !result.spans_valid() || spans.is_empty() {
            continue;
        }
        // Map each DepthPass span vec-index to its (z_level, pass_index)
        // payload, filling defaults when payload is missing.
        let mut depth_pass_meta: Vec<(usize, Option<f64>, Option<u32>)> = Vec::new();
        for (i, span) in spans.iter().enumerate() {
            if span.kind == SpanKind::DepthPass {
                let (z, p) = match &span.payload {
                    Some(SpanPayload::DepthPass { z_level, pass_index }) => {
                        (Some(*z_level), Some(*pass_index))
                    }
                    _ => (None, None),
                };
                depth_pass_meta.push((i, z, p));
            }
        }
        if depth_pass_meta.is_empty() {
            continue;
        }
        let depth_pass_ids: std::collections::HashSet<u32> =
            depth_pass_meta.iter().map(|(i, _, _)| *i as u32).collect();
        // Accumulate per-pass stats in lock-step with depth_pass_meta.
        let mut accs: Vec<DepthPassAcc> = (0..depth_pass_meta.len())
            .map(|_| DepthPassAcc::default())
            .collect();
        let pass_index_of: std::collections::HashMap<u32, usize> = depth_pass_meta
            .iter()
            .enumerate()
            .map(|(i, (sid, _, _))| (*sid as u32, i))
            .collect();

        for sample in trace.samples.iter().filter(|s| s.toolpath_id == tc.id) {
            // Find the DepthPass id in this sample's span_path.
            let pass_pos = sample
                .span_path
                .iter()
                .find_map(|SpanId(id)| {
                    if depth_pass_ids.contains(id) {
                        pass_index_of.get(id).copied()
                    } else {
                        None
                    }
                });
            if let Some(pos) = pass_pos {
                #[allow(clippy::indexing_slicing)] // pos came from pass_index_of
                accs[pos].observe(sample);
            }
        }

        let entries: Vec<serde_json::Value> = depth_pass_meta
            .iter()
            .zip(accs.into_iter())
            .map(|((span_id, z, pass_idx), acc)| {
                serde_json::json!({
                    "span_id": *span_id,
                    "z_level": z,
                    "pass_index": pass_idx,
                    "sample_count": acc.sample_count,
                    "cutting_runtime_s": acc.cutting_runtime_s,
                    "total_removed_volume_est_mm3": acc.removed_volume_mm3,
                    "average_mrr_mm3_s": acc.average_mrr(),
                    "average_engagement": acc.average_engagement(),
                    "peak_chipload_mm_per_tooth": acc.peak_chipload,
                    "peak_axial_doc_mm": acc.peak_axial_doc,
                })
            })
            .collect();
        out.insert(tc.id.to_string(), serde_json::Value::Array(entries));
    }

    if out.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::Value::Object(out)
    }
}

#[derive(Default)]
struct DepthPassAcc {
    sample_count: usize,
    cutting_runtime_s: f64,
    removed_volume_mm3: f64,
    engagement_time_weighted_sum: f64,
    peak_chipload: f64,
    peak_axial_doc: f64,
}

impl DepthPassAcc {
    fn observe(&mut self, sample: &rs_cam_core::simulation_cut::SimulationCutSample) {
        self.sample_count += 1;
        if sample.is_cutting {
            self.cutting_runtime_s += sample.segment_time_s;
            self.engagement_time_weighted_sum +=
                sample.radial_engagement * sample.segment_time_s;
            self.removed_volume_mm3 += sample.removed_volume_est_mm3.max(0.0);
        }
        self.peak_chipload = self.peak_chipload.max(sample.chipload_mm_per_tooth.max(0.0));
        self.peak_axial_doc = self.peak_axial_doc.max(sample.axial_doc_mm.max(0.0));
    }

    fn average_mrr(&self) -> f64 {
        if self.cutting_runtime_s <= 1e-9 {
            0.0
        } else {
            self.removed_volume_mm3 / self.cutting_runtime_s
        }
    }

    fn average_engagement(&self) -> f64 {
        if self.cutting_runtime_s <= 1e-9 {
            0.0
        } else {
            self.engagement_time_weighted_sum / self.cutting_runtime_s
        }
    }
}

/// Generation-debug span "kind" strings that participate in a given
/// structural `SpanKind`. When the input is itself a generation-debug kind
/// (e.g. "adaptive_pass"), the synonym set is just `{input}` so the filter
/// keeps backward compatibility with the existing string vocabulary.
fn expand_span_kind_synonyms(span_kind: &str) -> Vec<String> {
    match span_kind {
        // Structural SpanKind synonyms expand to the matching debug kinds.
        "depth_pass" => vec![
            "z_level_clear".to_owned(),
            "adaptive_pass".to_owned(),
            "z_level".to_owned(),
        ],
        "entry" => vec!["entry_search".to_owned()],
        // Other SpanKind names have no debug-trace generators yet — return
        // an empty set so the filter matches nothing rather than falsely
        // matching by string.
        "operation" | "region" | "lead_out" | "link_bridge" | "dressup_artifact"
        | "rapid_order_barrier" => Vec::new(),
        // Fallback: treat as a literal debug-trace kind.
        other => vec![other.to_owned()],
    }
}

/// Map a debug-trace span "kind" string back to the structural `SpanKind`
/// (snake_case) it most directly contributes to. Op-internal phases
/// (preflight, widen_band, etc.) have no structural equivalent and return
/// `None`.
fn map_debug_kind_to_span_kind(debug_kind: &str) -> Option<&'static str> {
    match debug_kind {
        "z_level_clear" | "adaptive_pass" | "z_level" => Some("depth_pass"),
        "entry_search" => Some("entry"),
        _ => None,
    }
}

/// Map an MCP `span_kind` string (snake_case) to the `SpanKind` enum.
fn parse_span_kind_filter(s: &str) -> Result<rs_cam_core::toolpath_spans::SpanKind, String> {
    use rs_cam_core::toolpath_spans::SpanKind;
    match s {
        "operation" => Ok(SpanKind::Operation),
        "depth_pass" => Ok(SpanKind::DepthPass),
        "region" => Ok(SpanKind::Region),
        "entry" => Ok(SpanKind::Entry),
        "lead_out" => Ok(SpanKind::LeadOut),
        "link_bridge" => Ok(SpanKind::LinkBridge),
        "dressup_artifact" => Ok(SpanKind::DressupArtifact),
        "rapid_order_barrier" => Ok(SpanKind::RapidOrderBarrier),
        other => Err(format!("unknown span_kind {other:?}")),
    }
}
