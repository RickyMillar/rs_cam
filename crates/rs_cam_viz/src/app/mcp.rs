//! GUI-side processing of MCP requests. Called from `RsCamApp::update()`.

#![deny(clippy::indexing_slicing)]

use std::path::Path;

use rs_cam_core::compute::config::{
    BoundaryConfig, BoundaryContainment, BoundarySource, DressupConfig,
};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId};
use rs_cam_core::session::MutationKind;

use crate::controller::Severity;
use crate::mcp_bridge::{
    GuiBanner, McpRequest, McpRequestKind, McpResponse, MutationResult, MutationWarning,
    PendingGenerateAll, ProgressUpdate,
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
            McpRequestKind::ListToolLibrary => {
                let resp = self.mcp_list_tool_library();
                let _ = response_tx.send(McpResponse { result: Ok(resp) });
            }
            McpRequestKind::ListToolCatalog { catalog } => {
                let resp = self.mcp_list_tool_catalog(&catalog);
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
            McpRequestKind::GetOperationSchema { operation_type } => {
                let resp = self.mcp_get_operation_schema(&operation_type);
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
                include_drill_samples,
            } => {
                let resp = self.mcp_get_cut_trace(
                    toolpath_id,
                    max_hotspots,
                    max_issues,
                    span_kind.as_deref(),
                    span_id,
                    pass_index,
                    include_drill_samples,
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
            McpRequestKind::InspectCollisions => {
                let resp = self.mcp_inspect_collisions();
                let _ = response_tx.send(McpResponse { result: Ok(resp) });
            }
            McpRequestKind::InspectSpans {
                index,
                kind,
                parent_id,
                pass_index,
                region_id,
                max_spans,
            } => {
                let resp = self.mcp_inspect_spans(
                    index,
                    kind.as_deref(),
                    parent_id,
                    pass_index,
                    region_id,
                    max_spans,
                );
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
            McpRequestKind::GetToolpathDiagnostics { index } => {
                let resp = self.mcp_get_toolpath_diagnostics(index);
                let _ = response_tx.send(McpResponse { result: Ok(resp) });
            }
            McpRequestKind::GetProjectDiagnostics => {
                let resp = self.mcp_get_project_diagnostics();
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
            McpRequestKind::SetToolpathHeights {
                index,
                clearance_z,
                retract_z,
                feed_z,
                top_z,
                bottom_z,
            } => {
                let resp = self.mcp_set_toolpath_heights(
                    index,
                    clearance_z,
                    retract_z,
                    feed_z,
                    top_z,
                    bottom_z,
                );
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
            McpRequestKind::AddToolFromLibrary { catalog, index } => {
                self.controller.push_notification(
                    format!("MCP: Imported tool from library '{catalog}' #{index}"),
                    Severity::Info,
                );
                let resp = self.mcp_add_tool_from_library(&catalog, index);
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
            McpRequestKind::SetSpindleStrategy { strategy } => {
                let resp = self.mcp_set_spindle_strategy(&strategy);
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
        let stale_defaults = rs_cam_core::compute::validate::validate_stale_defaults(session);
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
            "stale_defaults": stale_defaults,
            "build": rs_cam_mcp::server::build_info(),
        }))
    }

    fn mcp_list_toolpaths(&self) -> String {
        // Override the core listing to inject viz-only fields (`stale`,
        // `status`) so an automation client can tell at a glance whether a
        // toolpath needs regeneration. Staleness lives in
        // `ToolpathRuntime.stale_since` — viz-only — and the core summary
        // never sees it. (Roadmap E.3)
        let state = self.controller.state();
        let session = &state.session;
        let summaries = session.list_toolpaths();
        let rows: Vec<serde_json::Value> = summaries
            .into_iter()
            .map(|s| {
                let rt = state.gui.toolpath_rt.get(&s.id);
                let stale = rt.is_some_and(|r| r.stale_since.is_some());
                let (status, error) = match rt.map(|r| &r.status) {
                    Some(rs_cam_core::compute::config::ComputeStatus::Pending) => ("Pending", None),
                    Some(rs_cam_core::compute::config::ComputeStatus::Computing) => {
                        ("Computing", None)
                    }
                    Some(rs_cam_core::compute::config::ComputeStatus::Done) => ("Done", None),
                    Some(rs_cam_core::compute::config::ComputeStatus::Error(e)) => {
                        ("Error", Some(e.clone()))
                    }
                    None => ("Pending", None),
                };
                serde_json::json!({
                    "index": s.index,
                    "id": s.id,
                    "name": s.name,
                    "operation_label": s.operation_label,
                    "enabled": s.enabled,
                    "tool_name": s.tool_name,
                    "stale": stale,
                    "status": status,
                    "error": error,
                })
            })
            .collect();
        json_str(serde_json::Value::Array(rows))
    }

    fn mcp_list_tools(&self) -> String {
        let session = &self.controller.state().session;
        json_str(serde_json::to_value(session.list_tools()).unwrap_or_default())
    }

    /// Top level of the tool-library drill-down: one small row per
    /// catalog (name, tool count, the tool types it contains). Cheap —
    /// no per-tool geometry. Independent of the loaded project.
    fn mcp_list_tool_library(&self) -> String {
        use rs_cam_core::tool_library;
        let mut catalogs = Vec::new();
        for name in tool_library::list_libraries() {
            let Ok(catalog) = tool_library::load_library(&name) else {
                continue;
            };
            let mut types: Vec<String> = catalog
                .tools
                .iter()
                .filter_map(|t| {
                    serde_json::to_value(t.tool_type)
                        .ok()
                        .and_then(|v| v.as_str().map(str::to_owned))
                })
                .collect();
            types.sort();
            types.dedup();
            catalogs.push(serde_json::json!({
                "catalog": name,
                "tool_count": catalog.tools.len(),
                "tool_types": types,
            }));
        }
        json_str(serde_json::json!({
            "note": "Dig into a catalog with list_tool_catalog{catalog} to see its tools, \
                     then import one with add_tool_from_library{catalog, index}.",
            "catalogs": catalogs,
        }))
    }

    /// Drill into one catalog: compact tool rows carrying just the fields
    /// needed to choose a tool, plus the 0-based `index` that
    /// `add_tool_from_library` consumes. Full geometry comes across on
    /// import (the project keeps a snapshot).
    fn mcp_list_tool_catalog(&self, catalog: &str) -> String {
        use rs_cam_core::tool_library;
        let cat = match tool_library::load_library(catalog) {
            Ok(c) => c,
            Err(e) => return json_str(serde_json::json!({ "error": e.to_string() })),
        };
        let tools: Vec<serde_json::Value> = cat
            .tools
            .iter()
            .enumerate()
            .map(|(index, t)| {
                let ttype = serde_json::to_value(t.tool_type)
                    .ok()
                    .and_then(|v| v.as_str().map(str::to_owned))
                    .unwrap_or_default();
                let mut row = serde_json::json!({
                    "index": index,
                    "name": t.name,
                    "type": ttype,
                    "diameter_mm": t.diameter,
                    "flutes": t.flute_count,
                    "cutting_length_mm": t.cutting_length,
                    "shank_diameter_mm": t.shank_diameter,
                });
                if let Some(obj) = row.as_object_mut() {
                    use crate::state::job::ToolType;
                    match t.tool_type {
                        ToolType::VBit => {
                            obj.insert("included_angle_deg".to_owned(), serde_json::json!(t.included_angle));
                        }
                        ToolType::TaperedBallNose => {
                            obj.insert("taper_half_angle_deg".to_owned(), serde_json::json!(t.taper_half_angle));
                            obj.insert("shaft_diameter_mm".to_owned(), serde_json::json!(t.shaft_diameter));
                        }
                        _ => {}
                    }
                    if !t.vendor.is_empty() {
                        obj.insert("vendor".to_owned(), serde_json::json!(t.vendor));
                    }
                }
                row
            })
            .collect();
        json_str(serde_json::json!({
            "catalog": catalog,
            "tool_count": tools.len(),
            "tools": tools,
        }))
    }

    /// Import a snapshot of a catalog tool into the project. Mirrors the
    /// GUI's `AddToolFromLibrary` event: the project keeps its own copy
    /// and the session reassigns the tool id on insert.
    fn mcp_add_tool_from_library(&mut self, catalog: &str, index: usize) -> String {
        use rs_cam_core::tool_library;
        let before = self.mcp_diagnostic_snapshot();
        let cat = match tool_library::load_library(catalog) {
            Ok(c) => c,
            Err(e) => return self.mcp_mutation_error(format!("Error: {e}"), None),
        };
        let Some(mut tool) = cat.tools.get(index).cloned() else {
            return self.mcp_mutation_error(
                format!(
                    "Error: catalog '{catalog}' has no tool at index {index} (has {} tools)",
                    cat.tools.len()
                ),
                None,
            );
        };
        let name = tool.name.clone();
        tool.id = ToolId(0); // session reassigns on insert
        let idx = self.controller.state_mut().session.add_tool(tool);
        self.controller.state_mut().gui.mark_edited();
        self.mcp_mutation_result(
            format!("Imported '{name}' from {catalog} as tool {idx}"),
            serde_json::json!({
                "index": idx,
                "name": name,
                "source_catalog": catalog,
                "source_index": index,
            }),
            Vec::new(),
            &before,
        )
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
                let mut op_value =
                    serde_json::to_value(&tc.operation).unwrap_or_else(|_| serde_json::json!({}));
                if let Some(obj) = op_value.as_object_mut() {
                    obj.insert(
                        "params".to_owned(),
                        tc.operation.params_value_including_nulls(),
                    );
                    obj.insert(
                        "param_schema".to_owned(),
                        serde_json::to_value(tc.operation.param_schema_hints())
                            .unwrap_or_else(|_| serde_json::json!({})),
                    );
                }
                json_str(serde_json::json!({
                    "id": tc.id,
                    "name": tc.name,
                    "enabled": tc.enabled,
                    "tool_id": tc.tool_id,
                    "model_id": tc.model_id,
                    "operation": op_value,
                    "runtime": self.mcp_runtime_status_for_toolpath_id(tc.id),
                }))
            }
            None => {
                json_str(serde_json::json!({"error": format!("Toolpath index {index} not found")}))
            }
        }
    }

    fn mcp_runtime_status_for_toolpath_id(&self, toolpath_id: usize) -> serde_json::Value {
        let rt = self.controller.state().gui.toolpath_rt.get(&toolpath_id);
        let (status, error) = match rt.map(|r| &r.status) {
            Some(rs_cam_core::compute::config::ComputeStatus::Pending) => ("Pending", None),
            Some(rs_cam_core::compute::config::ComputeStatus::Computing) => ("Computing", None),
            Some(rs_cam_core::compute::config::ComputeStatus::Done) => ("Done", None),
            Some(rs_cam_core::compute::config::ComputeStatus::Error(e)) => {
                ("Error", Some(e.clone()))
            }
            None => ("Pending", None),
        };
        serde_json::json!({
            "status": status,
            "error": error,
            "stale": rt.is_some_and(|r| r.stale_since.is_some()),
        })
    }

    fn mcp_get_operation_schema(&self, operation_type: &str) -> String {
        match rs_cam_core::session::ProjectSession::operation_schema(operation_type) {
            Ok(schema) => json_str(serde_json::to_value(schema).unwrap_or_default()),
            Err(e) => json_str(serde_json::json!({ "error": e.to_string() })),
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
            // §6.C / §6.I revision: prefer the MoveIntent::Drilling signal
            // from the toolpath; fall back to op-kind for legacy generators.
            is_drill_cycle: result
                .annotated
                .toolpath
                .moves
                .iter()
                .any(|m| matches!(m.intent, rs_cam_core::toolpath::MoveIntent::Drilling))
                || matches!(
                    tc.operation.op_type(),
                    rs_cam_core::compute::catalog::OperationType::Drill
                        | rs_cam_core::compute::catalog::OperationType::AlignmentPinDrill
                ),
            material: Some(&state.session.stock_config().material),
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

    #[allow(clippy::too_many_arguments)]
    fn mcp_get_cut_trace(
        &self,
        toolpath_id: Option<usize>,
        max_hotspots: Option<usize>,
        max_issues: Option<usize>,
        span_kind: Option<&str>,
        span_id: Option<u32>,
        pass_index: Option<u32>,
        include_drill_samples: bool,
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

        let span_summaries = build_span_cut_summaries(
            state,
            ct,
            toolpath_id,
            span_filter_active,
            &accepted_by_toolpath,
        );

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

        // §6.E / Step 3 PR2 — drill-native outputs. `drill_summaries` always
        // surfaces (per-toolpath summary block, compact). `drill_samples` is
        // gated by `include_drill_samples` because the per-peck stream can be
        // verbose on cycles with many holes.
        let drill_summaries_val: Vec<&_> = ct
            .drill_summaries
            .iter()
            .filter(|s| toolpath_id.is_none_or(|id| s.toolpath_id == id))
            .collect();
        let drill_summaries_val =
            serde_json::to_value(&drill_summaries_val).unwrap_or_else(|_| serde_json::json!([]));
        let drill_samples_val = if include_drill_samples {
            let filtered: Vec<&_> = ct
                .drill_samples
                .iter()
                .filter(|s| toolpath_id.is_none_or(|id| s.toolpath_id == id))
                .collect();
            serde_json::to_value(&filtered).unwrap_or_else(|_| serde_json::json!([]))
        } else {
            serde_json::Value::Null
        };

        json_str(serde_json::json!({
            "summary": summary_val,
            "semantic_summaries": summaries_val,
            "span_summaries": span_summaries,
            "hotspots": hotspots_val,
            "hotspot_count": hotspot_count,
            "issue_count": issue_count,
            "issues": issues_val,
            "drill_summaries": drill_summaries_val,
            "drill_samples": drill_samples_val,
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

    fn mcp_inspect_spans(
        &self,
        index: usize,
        kind: Option<&str>,
        parent_id: Option<u32>,
        pass_index: Option<u32>,
        region_id: Option<u32>,
        max_spans: Option<usize>,
    ) -> String {
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
        match build_inspect_spans_response(
            tc.id,
            index,
            &tc.name,
            tc.operation.label(),
            n_moves,
            result.spans(),
            result.spans_valid(),
            kind,
            parent_id,
            pass_index,
            region_id,
            max_spans,
        ) {
            Ok(value) => json_str(value),
            Err(msg) => json_str(serde_json::json!({ "error": msg })),
        }
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

    /// Per-toolpath listing of collisions detected during simulation:
    /// holder collisions (sim.checks.collision_report) and rapid collisions
    /// (sim.checks.rapid_collision_move_indices), grouped by the toolpath
    /// each move belongs to. Localizes the project-wide
    /// `rapid_collision_count` from `run_simulation` so the user can drill
    /// to the specific lift/retract that's clipping uncleared stock.
    fn mcp_inspect_collisions(&self) -> String {
        let state = self.controller.state();
        let sim = &state.simulation;

        if !sim.has_results() {
            return json_str(serde_json::json!({
                "error": "No simulation results. Call run_simulation first."
            }));
        }

        let mut holder_by_tp: std::collections::BTreeMap<usize, Vec<serde_json::Value>> =
            std::collections::BTreeMap::new();
        let mut rapid_by_tp: std::collections::BTreeMap<usize, Vec<serde_json::Value>> =
            std::collections::BTreeMap::new();

        // Holder/shank collisions live in collision_report. Their `move_idx`
        // is in global move space; map back to (toolpath_id, local_move).
        if let Some(report) = sim.checks.collision_report.as_ref() {
            for c in &report.collisions {
                let (tp_id, local_move) = sim
                    .move_to_local_toolpath_move(c.move_idx)
                    .map(|(_, id, local)| (id.0, local))
                    .unwrap_or((usize::MAX, c.move_idx));
                holder_by_tp
                    .entry(tp_id)
                    .or_default()
                    .push(serde_json::json!({
                        "global_move": c.move_idx,
                        "local_move": local_move,
                        "segment": format!("{:?}", c.segment),
                    }));
            }
        }

        // Rapid collisions are stored as bare global move indices.
        for &global_move in &sim.checks.rapid_collision_move_indices {
            let (tp_id, local_move) = sim
                .move_to_local_toolpath_move(global_move)
                .map(|(_, id, local)| (id.0, local))
                .unwrap_or((usize::MAX, global_move));
            rapid_by_tp
                .entry(tp_id)
                .or_default()
                .push(serde_json::json!({
                    "global_move": global_move,
                    "local_move": local_move,
                }));
        }

        // Build the per-toolpath rollup. Include every TP that has at least
        // one of either kind so the agent doesn't have to merge two maps.
        let all_tps: std::collections::BTreeSet<usize> = holder_by_tp
            .keys()
            .chain(rapid_by_tp.keys())
            .copied()
            .collect();

        let session = &state.simulation;
        let by_toolpath: Vec<serde_json::Value> = all_tps
            .iter()
            .map(|tp_id| {
                let name = session
                    .boundaries()
                    .iter()
                    .find(|b| b.id.0 == *tp_id)
                    .map(|b| b.name.clone())
                    .unwrap_or_else(|| format!("(unknown {tp_id})"));
                let holder = holder_by_tp.get(tp_id).cloned().unwrap_or_default();
                let rapid = rapid_by_tp.get(tp_id).cloned().unwrap_or_default();
                serde_json::json!({
                    "toolpath_id": tp_id,
                    "toolpath_name": name,
                    "holder_collision_count": holder.len(),
                    "rapid_collision_count": rapid.len(),
                    "holder_collisions": holder,
                    "rapid_collisions": rapid,
                })
            })
            .collect();

        let total_holder: usize = holder_by_tp.values().map(|v| v.len()).sum();
        let total_rapid: usize = rapid_by_tp.values().map(|v| v.len()).sum();

        json_str(serde_json::json!({
            "holder_collision_count": total_holder,
            "rapid_collision_count": total_rapid,
            "by_toolpath": by_toolpath,
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
        let before = self.mcp_diagnostic_snapshot();
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
        self.mcp_mutation_result(
            message,
            serde_json::json!({
                "added": added,
                "pin_count": pin_count,
            }),
            Vec::new(),
            &before,
        )
    }

    fn mcp_remove_alignment_pin(&mut self, index: usize) -> String {
        let before = self.mcp_diagnostic_snapshot();
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
                self.mcp_mutation_result(
                    format!("Removed alignment pin {index}"),
                    serde_json::json!({ "pin_count": pin_count }),
                    Vec::new(),
                    &before,
                )
            }
            Err(e) => self.mcp_mutation_error(format!("Error: {e}"), None),
        }
    }

    // ── Mutation result helpers ──────────────────────────────────────

    fn mcp_diagnostic_snapshot(&self) -> Vec<serde_json::Value> {
        let state = self.controller.state();
        let sim_trace = state
            .simulation
            .results
            .as_ref()
            .and_then(|r| r.cut_trace.as_deref());
        let mut values = Vec::new();
        let evidence = viz_project_evidence(state);
        values.extend(
            state
                .session
                .diagnose_project_with_evidence(&evidence)
                .into_iter()
                .filter_map(|diag| serde_json::to_value(diag).ok()),
        );
        for index in 0..state.session.toolpath_count() {
            if let Ok(diags) = state.session.diagnose_toolpath_with_trace(index, sim_trace) {
                values.extend(
                    diags
                        .into_iter()
                        .filter_map(|diag| serde_json::to_value(diag).ok()),
                );
            }
        }
        values.extend(self.mcp_runtime_error_diagnostics());
        values
    }

    fn mcp_runtime_error_diagnostics(&self) -> Vec<serde_json::Value> {
        let state = self.controller.state();
        state
            .session
            .toolpath_configs()
            .iter()
            .enumerate()
            .filter_map(|(index, tc)| {
                let rt = state.gui.toolpath_rt.get(&tc.id)?;
                let rs_cam_core::compute::config::ComputeStatus::Error(error) = &rt.status else {
                    return None;
                };
                Some(serde_json::json!({
                    "id": format!("runtime.generate_error.{}", tc.id),
                    "scope": { "kind": "toolpath", "id": tc.id },
                    "category": "state",
                    "severity": "blocking",
                    "confidence": "verified",
                    "state": "current",
                    "source": { "kind": "gui_runtime", "toolpath_index": index },
                    "message": error,
                }))
            })
            .collect()
    }

    /// Compute the stale set for a mutation and mark `stale_since` on the
    /// corresponding GUI toolpath runtimes. Returns the toolpath indices
    /// that were marked stale so callers can emit them in the mutation
    /// envelope's `stale_toolpaths` field.
    fn mcp_apply_stale(&mut self, mutation: MutationKind) -> Vec<usize> {
        let stale =
            rs_cam_core::session::compute_stale_set(&self.controller.state().session, mutation)
                .toolpath_indices;
        let now = std::time::Instant::now();
        let ids: Vec<usize> = stale
            .iter()
            .filter_map(|&index| {
                self.controller
                    .state()
                    .session
                    .toolpath_configs()
                    .get(index)
                    .map(|tc| tc.id)
            })
            .collect();
        for id in ids {
            if let Some(rt) = self.controller.state_mut().gui.toolpath_rt.get_mut(&id) {
                rt.stale_since = Some(now);
            }
        }
        stale
    }

    fn mcp_mutation_error(&self, summary: impl Into<String>, field: Option<String>) -> String {
        let summary = summary.into();
        let field_value = field
            .map(serde_json::Value::String)
            .unwrap_or(serde_json::Value::Null);
        json_str(serde_json::json!({
            "ok": false,
            "summary": summary,
            "applied": serde_json::Value::Null,
            "stale_toolpaths": [],
            "warnings": [{
                "level": "error",
                "field": field_value,
                "message": summary,
                "recommendation": null,
            }],
            "gui_banners": [],
            "diagnostic_delta": [],
        }))
    }

    fn mcp_mutation_result<T>(
        &self,
        summary: String,
        applied: T,
        stale_toolpaths: Vec<usize>,
        before_diagnostics: &[serde_json::Value],
    ) -> String
    where
        T: serde::Serialize,
    {
        let after = self.mcp_diagnostic_snapshot();
        let diagnostic_delta: Vec<serde_json::Value> = after
            .into_iter()
            .filter(|diag| !before_diagnostics.contains(diag))
            .collect();
        let gui_banners: Vec<GuiBanner> = diagnostic_delta
            .iter()
            .filter_map(|diag| {
                let severity = diag.get("severity")?.as_str()?.to_owned();
                if !matches!(severity.as_str(), "caution" | "critical" | "blocking") {
                    return None;
                }
                Some(GuiBanner {
                    kind: "diagnostic".to_owned(),
                    severity,
                    title: diag.get("message")?.as_str()?.to_owned(),
                    detail: diag
                        .get("source")
                        .and_then(|source| serde_json::to_string(source).ok()),
                })
            })
            .collect();
        let warnings: Vec<MutationWarning> = gui_banners
            .iter()
            .map(|banner| MutationWarning {
                level: banner.severity.clone(),
                field: None,
                message: banner.title.clone(),
                recommendation: None,
            })
            .collect();
        json_str(
            serde_json::to_value(MutationResult {
                ok: true,
                summary,
                applied,
                stale_toolpaths,
                warnings,
                gui_banners,
                diagnostic_delta,
            })
            .unwrap_or_else(|e| serde_json::json!({ "error": e.to_string() })),
        )
    }

    // ── Mutation implementations ─────────────────────────────────────

    fn mcp_add_setup(&mut self, name: Option<&str>) -> String {
        let before = self.mcp_diagnostic_snapshot();
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
            return self.mcp_mutation_error("Error: Failed to add setup".to_owned(), None);
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
        self.mcp_mutation_result(
            format!("Added setup {}", setup_count - 1),
            serde_json::json!({
                "index": setup_count - 1,
                "id": sid,
                "name": final_name,
            }),
            Vec::new(),
            &before,
        )
    }

    fn mcp_set_setup_face(&mut self, setup_index: usize, face_up: &str) -> String {
        let before = self.mcp_diagnostic_snapshot();
        let setups = self.controller.state().session.list_setups();
        let Some(setup) = setups.get(setup_index) else {
            return self
                .mcp_mutation_error(format!("Error: Setup index {setup_index} not found"), None);
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
                return self.mcp_mutation_error(
                    format!("Error: Unknown face '{face_up}'. Use: top, bottom, front, back, left, right"),
                    Some("face_up".to_owned()),
                );
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
            let stale = self.mcp_apply_stale(MutationKind::SetupChanged { setup_id });
            self.mcp_mutation_result(
                format!("Set setup {setup_index} face to {}", face_up.to_lowercase()),
                serde_json::json!({
                    "setup_index": setup_index,
                    "face_up": face_up.to_lowercase(),
                }),
                stale,
                &before,
            )
        } else {
            self.mcp_mutation_error("Error: Setup not found".to_owned(), None)
        }
    }

    fn mcp_move_toolpath_to_setup(
        &mut self,
        toolpath_index: usize,
        target_setup_index: usize,
    ) -> String {
        let before = self.mcp_diagnostic_snapshot();
        let session = &self.controller.state().session;
        let Some(tc) = session.toolpath_configs().get(toolpath_index) else {
            return self.mcp_mutation_error(
                format!("Error: Toolpath index {toolpath_index} not found"),
                None,
            );
        };
        let tp_id = crate::state::toolpath::ToolpathId(tc.id);
        let Some(target_setup) = session.list_setups().get(target_setup_index) else {
            return self.mcp_mutation_error(
                format!("Error: Setup index {target_setup_index} not found"),
                None,
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

        self.mcp_mutation_result(
            format!("Moved toolpath {toolpath_index} to setup {target_setup_index}"),
            serde_json::json!({
                "toolpath_index": toolpath_index,
                "target_setup_index": target_setup_index,
            }),
            vec![toolpath_index],
            &before,
        )
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
                let mut msg =
                    format!("Loaded '{name}' -- {setup_count} setups, {tp_count} toolpaths");
                // Surface load warnings the GUI shows in a modal but the MCP
                // wrapper previously dropped on the floor (Roadmap E.1).
                let warnings = self.controller.load_warnings();
                if !warnings.is_empty() {
                    msg.push_str("\nWarnings:");
                    for w in warnings {
                        msg.push_str("\n  - ");
                        msg.push_str(w);
                    }
                }
                McpResponse {
                    result: Ok(text(msg)),
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
        // Route through the viz-side exporter so the gate sees viz worker
        // results (`gui.toolpath_rt[id].result`) and the viz cut trace
        // (`state.simulation.results.cut_trace`). The core-side
        // `session.export_gcode_with_policy` reads `session.results` and
        // `session.simulation`, which the GUI/MCP path never populates —
        // that's the root cause of the 0-byte file + spurious
        // SimulationRequired gate (UX_PAIN_POINTS_2026-05-11.md, Roadmap A).
        let state = self.controller.state();
        let policy = rs_cam_core::gcode::ToolLoadExportPolicy {
            accept_unmodeled: accept_unmodeled_tool_load,
            accept_exceeded: accept_exceeded_tool_load,
        };
        let gcode = match crate::io::export::export_gcode_from_session_with_policy(
            &state.session,
            &state.gui,
            &state.simulation,
            policy,
        ) {
            Ok(g) => g,
            Err(e) => return text(format!("Export failed: {e}")),
        };
        match std::fs::write(Path::new(path), &gcode) {
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

        // F5 — first-look summary so a single MCP read answers
        // "what's broken in this project?" without folding the
        // per-toolpath array.
        //
        // Roadmap F.11 — name resolver populates
        // `exceeds_breakdown[].toolpath_name` so agents don't need a
        // second `list_toolpaths()` round-trip to label each entry.
        let summary_value = serde_json::to_value(report.summary(|id| {
            state
                .session
                .toolpath_configs()
                .iter()
                .find(|tc| tc.id == id)
                .map(|tc| tc.name.clone())
        }))
        .unwrap_or(serde_json::Value::Null);
        let load_value = serde_json::to_value(&report).unwrap_or(serde_json::Value::Null);
        json_str(serde_json::json!({
            "summary": summary_value,
            "load_report": load_value,
            "per_depth_pass": per_depth_pass,
        }))
    }

    /// PR-3: unified per-toolpath diagnostics. Calls
    /// [`ProjectSession::diagnose_toolpath`] which runs every
    /// adapter and applies supersession. The result is a flat list
    /// of [`rs_cam_core::diagnostics::Diagnostic`] suitable for
    /// the GUI params panel or any MCP consumer that wants a single
    /// canonical view.
    fn mcp_get_toolpath_diagnostics(&self, index: usize) -> String {
        let state = self.controller.state();
        // The active sim trace lives on the viz-side state, not on
        // the core session — pass it explicitly so the load gates
        // see fresh evidence rather than `NeedsSimulation`.
        let sim_trace = state
            .simulation
            .results
            .as_ref()
            .and_then(|r| r.cut_trace.as_deref());
        match state.session.diagnose_toolpath_with_trace(index, sim_trace) {
            Ok(diagnostics) => {
                json_str(serde_json::to_value(&diagnostics).unwrap_or(serde_json::Value::Null))
            }
            Err(e) => json_str(serde_json::json!({"error": format!("{e}")})),
        }
    }

    /// PR-3: project-wide diagnostics (collisions, air-cut high,
    /// generated-empty, plunge stress). Builds a `ProjectEvidence`
    /// borrow view from viz-side simulation state — collisions
    /// and runtime/engagement readings reflect the latest run rather
    /// than the (always empty in GUI mode) core-session snapshot.
    fn mcp_get_project_diagnostics(&self) -> String {
        let state = self.controller.state();
        let evidence = viz_project_evidence(state);
        let diagnostics = state.session.diagnose_project_with_evidence(&evidence);
        json_str(serde_json::to_value(&diagnostics).unwrap_or(serde_json::Value::Null))
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
        let before = self.mcp_diagnostic_snapshot();
        match self
            .controller
            .state_mut()
            .session
            .set_toolpath_param(index, param, value)
        {
            Ok(()) => {
                self.controller.state_mut().gui.mark_edited();
                let stale = self.mcp_apply_stale(MutationKind::ToolpathParamChanged {
                    toolpath_index: index,
                });
                let applied = self
                    .controller
                    .state()
                    .session
                    .toolpath_configs()
                    .get(index)
                    .map(|tc| {
                        tc.operation
                            .params_value_including_nulls()
                            .get(param)
                            .cloned()
                            .unwrap_or(serde_json::Value::Null)
                    })
                    .unwrap_or(serde_json::Value::Null);
                self.mcp_mutation_result(
                    format!("Set toolpath {index} param '{param}'. Regenerate to apply."),
                    applied,
                    stale,
                    &before,
                )
            }
            Err(e) => self.mcp_mutation_error(format!("Error: {e}"), Some(param.to_owned())),
        }
    }

    fn mcp_set_toolpath_heights(
        &mut self,
        index: usize,
        clearance_z: Option<f64>,
        retract_z: Option<f64>,
        feed_z: Option<f64>,
        top_z: Option<f64>,
        bottom_z: Option<f64>,
    ) -> String {
        use rs_cam_core::compute::config::HeightMode;
        let before = self.mcp_diagnostic_snapshot();
        let Some(mut heights) = self
            .controller
            .state()
            .session
            .get_toolpath_config(index)
            .map(|tc| tc.heights.clone())
        else {
            return self.mcp_mutation_error(format!("Error: toolpath index {index} not found"), None);
        };
        if let Some(v) = clearance_z {
            heights.clearance_z = HeightMode::Manual(v);
        }
        if let Some(v) = retract_z {
            heights.retract_z = HeightMode::Manual(v);
        }
        if let Some(v) = feed_z {
            heights.feed_z = HeightMode::Manual(v);
        }
        if let Some(v) = top_z {
            heights.top_z = HeightMode::Manual(v);
        }
        if let Some(v) = bottom_z {
            heights.bottom_z = HeightMode::Manual(v);
        }
        match self
            .controller
            .state_mut()
            .session
            .set_heights_config(index, heights)
        {
            Ok(()) => {
                self.controller.state_mut().gui.mark_edited();
                let stale = self.mcp_apply_stale(MutationKind::ToolpathParamChanged {
                    toolpath_index: index,
                });
                self.mcp_mutation_result(
                    format!("Set toolpath {index} heights. Regenerate to apply."),
                    serde_json::json!({
                        "index": index,
                        "clearance_z": clearance_z,
                        "retract_z": retract_z,
                        "feed_z": feed_z,
                        "top_z": top_z,
                        "bottom_z": bottom_z,
                    }),
                    stale,
                    &before,
                )
            }
            Err(e) => self.mcp_mutation_error(format!("Error: {e}"), None),
        }
    }

    fn mcp_set_tool_param(
        &mut self,
        index: usize,
        param: &str,
        value: &serde_json::Value,
    ) -> String {
        let before = self.mcp_diagnostic_snapshot();
        match self
            .controller
            .state_mut()
            .session
            .set_tool_param(index, param, value)
        {
            Ok(()) => {
                self.controller.state_mut().gui.mark_edited();
                let stale =
                    self.mcp_apply_stale(MutationKind::ToolParamChanged { tool_index: index });
                let applied = self
                    .controller
                    .state()
                    .session
                    .tools()
                    .get(index)
                    .and_then(|tool| serde_json::to_value(tool).ok())
                    .and_then(|tool| tool.get(param).cloned())
                    .unwrap_or(serde_json::Value::Null);
                self.mcp_mutation_result(
                    format!(
                        "Set tool {index} param '{param}'. Regenerate affected toolpaths to apply."
                    ),
                    applied,
                    stale,
                    &before,
                )
            }
            Err(e) => self.mcp_mutation_error(format!("Error: {e}"), Some(param.to_owned())),
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
        let before = self.mcp_diagnostic_snapshot();
        let op_type = match parse_operation_type(operation_type) {
            Ok(ot) => ot,
            Err(e) => return self.mcp_mutation_error(format!("Error: {e}"), None),
        };

        let session = &self.controller.state().session;
        let tools = session.list_tools();
        let tool_raw_id = match tools.get(tool_index) {
            Some(info) => info.id.0,
            None => {
                return self
                    .mcp_mutation_error(format!("Error: Tool index {tool_index} not found"), None);
            }
        };

        // Roadmap B.1–B.3 — pull stock-aware depth defaults so a fresh
        // toolpath added via MCP gets the same sensible per-op depths
        // as a GUI add (drop_cutter min_z = stock_bottom, etc).
        let stock_bbox = session.stock_bbox();
        let stock_padding = session.stock_config().padding;
        let stock_ctx =
            rs_cam_core::feeds::suggest::StockContext::from_stock_bbox(stock_bbox, stock_padding);
        let label = op_type.label();
        let tp_name = name.unwrap_or_else(|| label.to_owned());

        // Roadmap F.5 — one-shot canonical suggest call at toolpath creation.
        // New toolpaths get recommended feeds written in once; after that,
        // fields are always user-owned.
        let Some(tool) = session.tools().iter().find(|t| t.id.0 == tool_raw_id) else {
            return self
                .mcp_mutation_error(format!("Error: Tool index {tool_index} not found"), None);
        };
        let op_config = match rs_cam_core::feeds::suggest::suggest_params(
            rs_cam_core::feeds::suggest::SuggestParamsInput {
                op_type,
                tool,
                machine: session.machine(),
                material: &session.stock_config().material,
                workholding: session.stock_config().workholding_rigidity,
                lut: rs_cam_core::feeds::embedded_vendor_lut(),
                stock_ctx: &stock_ctx,
                spindle_strategy: rs_cam_core::feeds::SpindleStrategy::default(),
            },
        ) {
            Ok(s) => s.operation,
            Err(e) => {
                return self.mcp_mutation_error(
                    format!("Cannot add toolpath: {e}"),
                    None,
                );
            }
        };

        // Roadmap B.7 — boundary auto-enable for 3D ops on mesh models.
        let has_mesh = session.models().iter().any(|m| m.mesh.is_some());
        let boundary = if op_config.is_3d() && has_mesh {
            BoundaryConfig {
                enabled: true,
                source: rs_cam_core::compute::config::BoundarySource::ModelSilhouette,
                ..BoundaryConfig::default()
            }
        } else {
            BoundaryConfig::default()
        };

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
            boundary,
            boundary_inherit: true,
            stock_source: rs_cam_core::compute::config::StockSource::default(),
            coolant: rs_cam_core::gcode::CoolantMode::default(),
            face_selection: None,
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
                self.mcp_mutation_result(
                    format!("Added toolpath {idx} ({label})."),
                    serde_json::json!({
                        "index": idx,
                        "operation": label,
                    }),
                    vec![idx],
                    &before,
                )
            }
            Err(e) => self.mcp_mutation_error(format!("Error: {e}"), None),
        }
    }

    fn mcp_remove_toolpath(&mut self, index: usize) -> String {
        let before = self.mcp_diagnostic_snapshot();
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
                self.mcp_mutation_result(
                    format!("Removed toolpath {index}"),
                    serde_json::json!({ "index": index }),
                    Vec::new(),
                    &before,
                )
            }
            Err(e) => self.mcp_mutation_error(format!("Error: {e}"), None),
        }
    }

    fn mcp_add_tool(&mut self, name: &str, tool_type: &str, diameter: f64) -> String {
        let before = self.mcp_diagnostic_snapshot();
        let tt = match parse_tool_type(tool_type) {
            Ok(t) => t,
            Err(e) => return self.mcp_mutation_error(format!("Error: {e}"), None),
        };

        let mut config = ToolConfig::new_default(ToolId(0), tt);
        config.name = name.to_owned();
        config.diameter = diameter;

        let idx = self.controller.state_mut().session.add_tool(config);
        self.controller.state_mut().gui.mark_edited();
        self.mcp_mutation_result(
            format!("Added tool '{name}'"),
            serde_json::json!({
                "index": idx,
                "tool_type": tool_type,
                "diameter": diameter,
            }),
            Vec::new(),
            &before,
        )
    }

    fn mcp_remove_tool(&mut self, index: usize) -> String {
        let before = self.mcp_diagnostic_snapshot();
        match self.controller.state_mut().session.remove_tool(index) {
            Ok(()) => {
                self.controller.state_mut().gui.mark_edited();
                self.mcp_mutation_result(
                    format!("Removed tool {index}"),
                    serde_json::json!({ "index": index }),
                    Vec::new(),
                    &before,
                )
            }
            Err(e) => self.mcp_mutation_error(format!("Error: {e}"), None),
        }
    }

    fn mcp_set_stock_config(&mut self, x: f64, y: f64, z: f64) -> String {
        let before = self.mcp_diagnostic_snapshot();
        let mut stock = self.controller.state().session.stock_config().clone();
        stock.x = x;
        stock.y = y;
        stock.z = z;
        self.controller.state_mut().session.set_stock_config(stock);
        self.controller.state_mut().gui.mark_edited();
        let stale = self.mcp_apply_stale(MutationKind::StockChanged);
        self.mcp_mutation_result(
            format!(
                "Stock set to {x:.1} x {y:.1} x {z:.1} mm. Regenerate toolpaths and simulation to apply."
            ),
            serde_json::json!({ "x": x, "y": y, "z": z }),
            stale,
            &before,
        )
    }

    fn mcp_set_boundary_config(
        &mut self,
        index: usize,
        enabled: bool,
        source: Option<&str>,
        containment: Option<&str>,
        offset: Option<f64>,
    ) -> String {
        let before = self.mcp_diagnostic_snapshot();
        let boundary_source = match source {
            Some("stock") | None => BoundarySource::Stock,
            Some("model_silhouette") => BoundarySource::ModelSilhouette,
            Some(other) => {
                return self.mcp_mutation_error(
                    format!("Error: Unknown boundary source '{other}'. Use 'stock' or 'model_silhouette'."),
                    Some("source".to_owned()),
                );
            }
        };

        let boundary_containment = match containment {
            Some("center") | None => BoundaryContainment::Center,
            Some("inside") => BoundaryContainment::Inside,
            Some("outside") => BoundaryContainment::Outside,
            Some(other) => {
                return self.mcp_mutation_error(
                    format!("Error: Unknown containment '{other}'. Use 'center', 'inside', or 'outside'."),
                    Some("containment".to_owned()),
                );
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
            .set_boundary_config(index, boundary.clone())
        {
            Ok(()) => {
                self.controller.state_mut().gui.mark_edited();
                let stale = self.mcp_apply_stale(MutationKind::ToolpathParamChanged {
                    toolpath_index: index,
                });
                self.mcp_mutation_result(
                    format!("Boundary set on toolpath {index}. Regenerate to apply."),
                    serde_json::to_value(boundary).unwrap_or(serde_json::Value::Null),
                    stale,
                    &before,
                )
            }
            Err(e) => self.mcp_mutation_error(format!("Error: {e}"), None),
        }
    }

    fn mcp_set_dressup_config(&mut self, index: usize, dressup: serde_json::Value) -> String {
        let before = self.mcp_diagnostic_snapshot();
        let dressup_config: DressupConfig = match serde_json::from_value(dressup) {
            Ok(dc) => dc,
            Err(e) => {
                return self
                    .mcp_mutation_error(format!("Error: Invalid dressup config: {e}"), None);
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
                let stale = self.mcp_apply_stale(MutationKind::ToolpathParamChanged {
                    toolpath_index: index,
                });
                let applied = self
                    .controller
                    .state()
                    .session
                    .toolpath_configs()
                    .get(index)
                    .and_then(|tc| serde_json::to_value(&tc.dressups).ok())
                    .unwrap_or(serde_json::Value::Null);
                self.mcp_mutation_result(
                    format!("Dressup config set on toolpath {index}. Regenerate to apply."),
                    applied,
                    stale,
                    &before,
                )
            }
            Err(e) => self.mcp_mutation_error(format!("Error: {e}"), None),
        }
    }

    fn mcp_set_dressup_field(
        &mut self,
        index: usize,
        key: &str,
        value: serde_json::Value,
    ) -> String {
        let before = self.mcp_diagnostic_snapshot();
        match self
            .controller
            .state_mut()
            .session
            .set_dressup_field(index, key, value)
        {
            Ok(()) => {
                self.controller.state_mut().gui.mark_edited();
                let stale = self.mcp_apply_stale(MutationKind::ToolpathParamChanged {
                    toolpath_index: index,
                });
                let applied = self
                    .controller
                    .state()
                    .session
                    .toolpath_configs()
                    .get(index)
                    .and_then(|tc| serde_json::to_value(&tc.dressups).ok())
                    .and_then(|dressups| dressups.get(key).cloned())
                    .unwrap_or(serde_json::Value::Null);
                self.mcp_mutation_result(
                    format!("Dressup field '{key}' set on toolpath {index}. Regenerate to apply."),
                    applied,
                    stale,
                    &before,
                )
            }
            Err(e) => self.mcp_mutation_error(format!("Error: {e}"), Some(key.to_owned())),
        }
    }

    fn mcp_set_toolpath_enabled(&mut self, index: usize, enabled: bool) -> String {
        let before = self.mcp_diagnostic_snapshot();
        match self
            .controller
            .state_mut()
            .session
            .set_toolpath_enabled(index, enabled)
        {
            Ok(()) => {
                self.controller.state_mut().gui.mark_edited();
                self.mcp_mutation_result(
                    format!(
                        "Toolpath {index} {}",
                        if enabled { "enabled" } else { "disabled" }
                    ),
                    serde_json::json!({ "index": index, "enabled": enabled }),
                    vec![index],
                    &before,
                )
            }
            Err(e) => self.mcp_mutation_error(format!("Error: {e}"), None),
        }
    }

    fn mcp_set_stock_source(&mut self, index: usize, source: &str) -> String {
        let before = self.mcp_diagnostic_snapshot();
        let parsed = match source {
            "fresh" => rs_cam_core::compute::config::StockSource::Fresh,
            "from_remaining_stock" => rs_cam_core::compute::config::StockSource::FromRemainingStock,
            other => {
                return self.mcp_mutation_error(
                    format!(
                        "Error: unknown stock_source '{other}'. Expected 'fresh' or 'from_remaining_stock'."
                    ),
                    Some("stock_source".to_owned()),
                );
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
                let stale = self.mcp_apply_stale(MutationKind::ToolpathParamChanged {
                    toolpath_index: index,
                });
                self.mcp_mutation_result(
                    format!(
                        "Stock source set to '{source}' on toolpath {index}. Regenerate to apply."
                    ),
                    serde_json::json!({ "index": index, "stock_source": source }),
                    stale,
                    &before,
                )
            }
            Err(e) => self.mcp_mutation_error(format!("Error: {e}"), None),
        }
    }

    fn mcp_set_spindle_strategy(&mut self, strategy: &str) -> String {
        let before = self.mcp_diagnostic_snapshot();
        let parsed = match strategy {
            "match_chart" | "MatchChart" | "matchchart" => {
                rs_cam_core::feeds::SpindleStrategy::MatchChart
            }
            "max_speed" | "MaxSpeed" | "maxspeed" => {
                rs_cam_core::feeds::SpindleStrategy::MaxSpeed
            }
            other => {
                return self.mcp_mutation_error(
                    format!(
                        "Error: unknown spindle_strategy '{other}'. Expected 'match_chart' or 'max_speed'."
                    ),
                    Some("spindle_strategy".to_owned()),
                );
            }
        };
        if self.controller.state().session.post_config().spindle_strategy == parsed {
            return self.mcp_mutation_result(
                format!("Spindle policy already set to '{strategy}'; no change."),
                serde_json::json!({ "spindle_strategy": strategy, "changed": false }),
                Vec::new(),
                &before,
            );
        }
        self.controller
            .state_mut()
            .session
            .post_mut()
            .spindle_strategy = parsed;
        self.controller
            .state_mut()
            .gui
            .post
            .spindle_strategy = parsed;
        self.controller.state_mut().gui.mark_edited();
        // Suggest is the read-side consumer — no toolpaths go stale
        // from this change. Operator/agent runs get_toolpath_params
        // or re-suggests to see new recommendations.
        self.mcp_mutation_result(
            format!(
                "Spindle policy set to '{strategy}'. Run Suggest (per toolpath or project-wide) to see new recommended feeds/RPMs."
            ),
            serde_json::json!({ "spindle_strategy": strategy, "changed": true }),
            Vec::new(),
            &before,
        )
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
                &result.annotated,
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

/// Build span-level cut summaries for `get_cut_trace`.
///
/// Returns summaries for every structural span that has samples in the current
/// trace. When a span filter is active, only accepted span ids are summarized.
fn build_span_cut_summaries(
    state: &crate::state::AppState,
    trace: &rs_cam_core::simulation_cut::SimulationCutTrace,
    toolpath_id: Option<usize>,
    span_filter_active: bool,
    accepted_by_toolpath: &std::collections::HashMap<usize, Option<std::collections::HashSet<u32>>>,
) -> serde_json::Value {
    use rs_cam_core::toolpath_spans::SpanId;

    let mut out = Vec::new();
    let n = state.session.toolpath_count();
    for idx in 0..n {
        let Some(tc) = state.session.get_toolpath_config(idx) else {
            continue;
        };
        if toolpath_id.is_some_and(|id| tc.id != id) {
            continue;
        }
        let Some(rt) = state.gui.toolpath_rt.get(&tc.id) else {
            continue;
        };
        let Some(result) = rt.result.as_ref() else {
            continue;
        };
        if !result.spans_valid() {
            continue;
        }
        let spans = result.spans();
        for (span_index, span) in spans.iter().enumerate() {
            let span_id = span_index as u32;
            if span_filter_active {
                let accepted = accepted_by_toolpath
                    .get(&tc.id)
                    .and_then(|set| set.as_ref());
                if !accepted.is_some_and(|set| set.contains(&span_id)) {
                    continue;
                }
            }

            let mut acc = rs_cam_core::simulation_cut::SummaryAccumulator::default();
            for sample in trace
                .samples
                .iter()
                .filter(|sample| sample.toolpath_id == tc.id)
            {
                if sample.span_path.iter().any(|SpanId(id)| *id == span_id) {
                    acc.observe(sample);
                }
            }
            if acc.sample_count == 0 {
                continue;
            }
            // Roadmap F.9 — the chipload key carries the per-sample
            // raw peak (typically several × the chipload gate's
            // `median_low` statistic on a 2D pocket / 3D rough). The
            // explicit `per_sample_peak_*` prefix makes the statistic
            // inline so agents/operators don't read it as a gate
            // exceedance. The gate's own statistic appears separately
            // on the load report's `chipload` verdict (see
            // `ChiploadMetric.statistic`).
            out.push(serde_json::json!({
                "toolpath_id": tc.id,
                "span_id": span_id,
                "kind": span_kind_label(span.kind),
                "label": &*span.label,
                "payload": span.payload.as_ref().map(|p| format!("{p:?}")),
                "start_move": span.start_move,
                "end_move": span.end_move,
                "is_boundary": span.is_boundary(),
                "sample_count": acc.sample_count,
                "total_runtime_s": acc.total_runtime_s,
                "cutting_runtime_s": acc.cutting_runtime_s,
                "rapid_runtime_s": acc.rapid_runtime_s,
                "air_cut_time_s": acc.air_cut_time_s,
                "low_engagement_time_s": acc.low_engagement_time_s,
                "wasted_runtime_s": acc.air_cut_time_s + acc.low_engagement_time_s,
                "average_engagement": acc.average_engagement(),
                "per_sample_peak_chipload_mm_per_tooth": acc.peak_chipload_mm_per_tooth,
                "peak_axial_doc_mm": acc.peak_axial_doc_mm,
                "peak_plunge_descent_mm": acc.peak_plunge_descent_mm,
                "total_removed_volume_est_mm3": acc.total_removed_volume_est_mm3,
                "average_mrr_mm3_s": acc.average_mrr(),
                // Step 2 D — per-kinematics axes inline so agents can read
                // axial-DOC for plunge-heavy passes and arc-WOC for lateral
                // ones without parsing a separate summary. See
                // planning/DEXEL_Z_ONLY_INVESTIGATION.md §6.D.
                "per_kinematics": render_per_kinematics_json(&acc),
            }));
        }
    }
    serde_json::Value::Array(out)
}

/// Build a `ProjectEvidence` borrow view from viz-side state so MCP
/// handlers can hand off to core diagnostics without GUI/MCP drift.
/// Pulls boundaries from `state.simulation.results`, rapid collisions
/// from `state.simulation.checks`, and the cut trace from the results
/// arc.
fn viz_project_evidence(
    state: &crate::state::AppState,
) -> rs_cam_core::session::ProjectEvidence<'_> {
    let boundaries = state
        .simulation
        .results
        .as_ref()
        .map(|r| {
            r.boundaries
                .iter()
                .map(|b| (b.id.0, b.start_move, b.end_move))
                .collect()
        })
        .unwrap_or_default();
    let cut_trace = state
        .simulation
        .results
        .as_ref()
        .and_then(|r| r.cut_trace.as_deref());
    rs_cam_core::session::ProjectEvidence {
        boundaries,
        rapid_collisions: &state.simulation.checks.rapid_collisions,
        rapid_collision_move_indices: &state.simulation.checks.rapid_collision_move_indices,
        cut_trace,
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
                    Some(SpanPayload::DepthPass {
                        z_level,
                        pass_index,
                    }) => (Some(*z_level), Some(*pass_index)),
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
        let mut accs: Vec<rs_cam_core::simulation_cut::SummaryAccumulator> = (0..depth_pass_meta
            .len())
            .map(|_| rs_cam_core::simulation_cut::SummaryAccumulator::default())
            .collect();
        let pass_index_of: std::collections::HashMap<u32, usize> = depth_pass_meta
            .iter()
            .enumerate()
            .map(|(i, (sid, _, _))| (*sid as u32, i))
            .collect();

        for sample in trace.samples.iter().filter(|s| s.toolpath_id == tc.id) {
            // Find the DepthPass id in this sample's span_path.
            let pass_pos = sample.span_path.iter().find_map(|SpanId(id)| {
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
                    "total_removed_volume_est_mm3": acc.total_removed_volume_est_mm3,
                    "average_mrr_mm3_s": acc.average_mrr(),
                    "average_engagement": acc.average_engagement(),
                    // Roadmap F.9 — name the field by its statistic so
                    // operators don't read this as the gate-trip value.
                    // The chipload gate uses `median_low(samples)` on
                    // the burn side; this RAW per-sample peak can be
                    // ~4× larger and looks like an exceedance when read
                    // in isolation. See planning/UX_PAIN_POINTS_2026-05-11.md F.9.
                    "per_sample_peak_chipload_mm_per_tooth": acc.peak_chipload_mm_per_tooth,
                    "peak_axial_doc_mm": acc.peak_axial_doc_mm,
                    "peak_plunge_descent_mm": acc.peak_plunge_descent_mm,
                    "per_kinematics": render_per_kinematics_json(&acc),
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

/// Render a `SummaryAccumulator`'s per-`CutKinematics` sub-accumulators as
/// inline JSON for MCP per-span / per-depth-pass output. Step 2 D — exposes
/// axial-DOC for plunge-heavy passes, arc-WOC for lateral ones, etc. without
/// reshuffling the surrounding payload. Empty when no cutting samples were
/// observed in this scope.
fn render_per_kinematics_json(
    acc: &rs_cam_core::simulation_cut::SummaryAccumulator,
) -> serde_json::Value {
    use rs_cam_core::simulation_cut::CutKinematics;
    let mut map = serde_json::Map::new();
    for kind in CutKinematics::ALL {
        #[allow(clippy::indexing_slicing)] // SAFETY: `kind.index()` is bounded by COUNT.
        let sub = &acc.per_kinematics[kind.index()];
        if sub.sample_count == 0 {
            continue;
        }
        let summary = sub.clone().finish();
        let label = match kind {
            CutKinematics::Linear => "linear",
            CutKinematics::Plunge => "plunge",
            CutKinematics::Helix => "helix",
            CutKinematics::Arc => "arc",
            CutKinematics::Rapid => "rapid",
        };
        map.insert(
            label.to_owned(),
            serde_json::json!({
                "cutting_runtime_s": summary.cutting_runtime_s,
                "sample_count": summary.sample_count,
                "average_radial_woc_fraction": summary.average_radial_woc_fraction,
                "peak_radial_woc_fraction": summary.peak_radial_woc_fraction,
                "average_axial_doc_fraction": summary.average_axial_doc_fraction,
                "peak_axial_doc_fraction": summary.peak_axial_doc_fraction,
                "peak_axial_doc_mm": summary.peak_axial_doc_mm,
                "peak_plunge_descent_mm": summary.peak_plunge_descent_mm,
                "average_arc_radians": summary.average_arc_radians,
                "average_mean_chip_thickness_mm": summary.average_mean_chip_thickness_mm,
                "peak_chip_thickness_mm": summary.peak_chip_thickness_mm,
                "average_leading_edge_speed_mm_min": summary.average_leading_edge_speed_mm_min,
            }),
        );
    }
    serde_json::Value::Object(map)
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
        "operation"
        | "region"
        | "lead_out"
        | "link_bridge"
        | "dressup_artifact"
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
        "waterline_cleanup" => Ok(SpanKind::WaterlineCleanup),
        "rapid_order_barrier" => Ok(SpanKind::RapidOrderBarrier),
        other => Err(format!("unknown span_kind {other:?}")),
    }
}

fn span_kind_label(k: rs_cam_core::toolpath_spans::SpanKind) -> &'static str {
    use rs_cam_core::toolpath_spans::SpanKind;
    match k {
        SpanKind::Operation => "Operation",
        SpanKind::DepthPass => "DepthPass",
        SpanKind::Region => "Region",
        SpanKind::Entry => "Entry",
        SpanKind::LeadOut => "LeadOut",
        SpanKind::LinkBridge => "LinkBridge",
        SpanKind::DressupArtifact => "DressupArtifact",
        SpanKind::WaterlineCleanup => "WaterlineCleanup",
        SpanKind::RapidOrderBarrier => "RapidOrderBarrier",
    }
}

fn span_to_json(id: usize, s: &rs_cam_core::toolpath_spans::Span) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "kind": span_kind_label(s.kind),
        "start_move": s.start_move,
        "end_move": s.end_move,
        "is_boundary": s.is_boundary(),
        "label": &*s.label,
        "payload": s.payload.as_ref().map(|p| format!("{p:?}")),
    })
}

/// Build the JSON response body for `inspect_spans`. Returns the response
/// envelope on success or an error message on bad filter input.
///
/// Default (no filter) returns a summary: `kind_counts` plus outermost spans
/// (Operation + DepthPass) under `top_level` with child counts. Setting any
/// of `kind`, `parent_id`, `pass_index`, `region_id` switches to detail mode
/// and returns matching spans under `spans`, with `total_matching` and
/// `truncated` reflecting `max_spans`.
#[allow(clippy::too_many_arguments)]
fn build_inspect_spans_response(
    toolpath_id: usize,
    toolpath_index: usize,
    name: &str,
    operation_label: &str,
    n_moves: usize,
    spans: &[rs_cam_core::toolpath_spans::Span],
    spans_valid: bool,
    kind: Option<&str>,
    parent_id: Option<u32>,
    pass_index: Option<u32>,
    region_id: Option<u32>,
    max_spans: Option<usize>,
) -> Result<serde_json::Value, String> {
    use rs_cam_core::toolpath_spans::{SpanKind, SpanPayload};

    let kind_filter = kind.map(parse_span_kind_filter).transpose()?;

    // Validate parent_id and resolve its move range.
    let parent_range: Option<(usize, usize)> = match parent_id {
        Some(pid) => {
            let parent = spans.get(pid as usize).ok_or_else(|| {
                format!("parent_id {pid} out of range (span_count={})", spans.len())
            })?;
            Some((parent.start_move, parent.end_move))
        }
        None => None,
    };

    // Per-kind tally for the summary row — always included.
    let mut kind_counts: std::collections::BTreeMap<&'static str, usize> =
        std::collections::BTreeMap::new();
    for s in spans {
        *kind_counts.entry(span_kind_label(s.kind)).or_insert(0) += 1;
    }

    let filter_active =
        kind.is_some() || parent_id.is_some() || pass_index.is_some() || region_id.is_some();

    let mut response = serde_json::json!({
        "toolpath_id": toolpath_id,
        "toolpath_index": toolpath_index,
        "name": name,
        "operation": operation_label,
        "move_count": n_moves,
        "span_count": spans.len(),
        "spans_valid": spans_valid,
        "kind_counts": kind_counts,
    });

    if !filter_active {
        // Summary mode: outermost spans only (Operation + DepthPass), with
        // child counts of contained non-boundary spans (excluding self).
        let top_level: Vec<serde_json::Value> = spans
            .iter()
            .enumerate()
            .filter(|(_, s)| matches!(s.kind, SpanKind::Operation | SpanKind::DepthPass))
            .map(|(id, s)| {
                let child_count = spans
                    .iter()
                    .enumerate()
                    .filter(|(other_id, c)| {
                        *other_id != id
                            && !c.is_boundary()
                            && c.start_move >= s.start_move
                            && c.end_move <= s.end_move
                    })
                    .count();
                let mut v = span_to_json(id, s);
                if let serde_json::Value::Object(map) = &mut v {
                    map.insert("child_count".into(), serde_json::json!(child_count));
                }
                v
            })
            .collect();

        if let serde_json::Value::Object(map) = &mut response {
            map.insert("top_level".into(), serde_json::Value::Array(top_level));
            map.insert(
                "hint".into(),
                serde_json::json!(
                    "Pass kind, parent_id, pass_index, or region_id to retrieve detail spans."
                ),
            );
        }
        return Ok(response);
    }

    // Detail mode: collect matching spans, then truncate.
    let matching: Vec<(usize, &rs_cam_core::toolpath_spans::Span)> = spans
        .iter()
        .enumerate()
        .filter(|(id, s)| {
            if let Some(want) = kind_filter
                && s.kind != want
            {
                return false;
            }
            if let Some((p_start, p_end)) = parent_range {
                // Child must be strictly different from parent and contained
                // within parent's range. Boundary spans at the parent's
                // start/end count as inside.
                if (*id as u32) == parent_id.unwrap_or(u32::MAX) {
                    return false;
                }
                if s.start_move < p_start || s.end_move > p_end {
                    return false;
                }
            }
            if let Some(want_pi) = pass_index {
                match &s.payload {
                    Some(SpanPayload::DepthPass { pass_index: pi, .. }) if *pi == want_pi => {}
                    _ => return false,
                }
            }
            if let Some(want_rid) = region_id {
                match &s.payload {
                    Some(SpanPayload::Region { region_id: rid }) if *rid == want_rid => {}
                    _ => return false,
                }
            }
            true
        })
        .collect();

    let total_matching = matching.len();
    let cap = max_spans.unwrap_or(50);
    let truncated = total_matching > cap;
    let spans_json: Vec<serde_json::Value> = matching
        .into_iter()
        .take(cap)
        .map(|(id, s)| span_to_json(id, s))
        .collect();

    if let serde_json::Value::Object(map) = &mut response {
        map.insert("total_matching".into(), serde_json::json!(total_matching));
        map.insert("truncated".into(), serde_json::json!(truncated));
        map.insert("max_spans".into(), serde_json::json!(cap));
        map.insert("spans".into(), serde_json::Value::Array(spans_json));
    }
    Ok(response)
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;
    use rs_cam_core::toolpath_spans::{Span, SpanKind, SpanPayload};

    /// Build a representative span tree:
    /// - Operation 0..30
    ///   - DepthPass 0..15 (pass_index=0)
    ///     - Region 0..7 (region_id=0)
    ///     - Region 7..15 (region_id=1)
    ///   - DepthPass 15..30 (pass_index=1)
    ///     - Region 15..22 (region_id=2)
    ///     - Region 22..30 (region_id=3)
    fn fixture_spans() -> Vec<Span> {
        vec![
            Span::new(0, 30, SpanKind::Operation),
            Span::new(0, 15, SpanKind::DepthPass).with_payload(SpanPayload::DepthPass {
                z_level: -2.0,
                pass_index: 0,
            }),
            Span::new(0, 7, SpanKind::Region).with_payload(SpanPayload::Region { region_id: 0 }),
            Span::new(7, 15, SpanKind::Region).with_payload(SpanPayload::Region { region_id: 1 }),
            Span::new(15, 30, SpanKind::DepthPass).with_payload(SpanPayload::DepthPass {
                z_level: -4.0,
                pass_index: 1,
            }),
            Span::new(15, 22, SpanKind::Region).with_payload(SpanPayload::Region { region_id: 2 }),
            Span::new(22, 30, SpanKind::Region).with_payload(SpanPayload::Region { region_id: 3 }),
        ]
    }

    fn build(
        spans: &[Span],
        kind: Option<&str>,
        parent_id: Option<u32>,
        pass_index: Option<u32>,
        region_id: Option<u32>,
        max_spans: Option<usize>,
    ) -> serde_json::Value {
        build_inspect_spans_response(
            42, 0, "tp", "pocket", 30, spans, true, kind, parent_id, pass_index, region_id,
            max_spans,
        )
        .expect("valid filter")
    }

    #[test]
    fn default_returns_summary_with_kind_counts_and_top_level() {
        let spans = fixture_spans();
        let v = build(&spans, None, None, None, None, None);

        assert_eq!(v["span_count"], 7);
        assert_eq!(v["move_count"], 30);
        assert_eq!(v["spans_valid"], true);
        // Spans array must NOT be present in summary mode.
        assert!(v.get("spans").is_none());

        let kc = &v["kind_counts"];
        assert_eq!(kc["Operation"], 1);
        assert_eq!(kc["DepthPass"], 2);
        assert_eq!(kc["Region"], 4);

        let top_level = v["top_level"].as_array().expect("top_level array");
        // Operation + 2 DepthPass = 3 entries, no Region leaves.
        assert_eq!(top_level.len(), 3);
        let kinds: Vec<&str> = top_level
            .iter()
            .map(|e| e["kind"].as_str().unwrap())
            .collect();
        assert_eq!(kinds, vec!["Operation", "DepthPass", "DepthPass"]);

        // Operation has 6 contained spans (2 DepthPass + 4 Region).
        assert_eq!(top_level[0]["child_count"], 6);
        // Each DepthPass has 2 Region children.
        assert_eq!(top_level[1]["child_count"], 2);
        assert_eq!(top_level[2]["child_count"], 2);

        assert!(v["hint"].as_str().unwrap().contains("kind"));
    }

    #[test]
    fn kind_filter_returns_only_matching_spans() {
        let spans = fixture_spans();
        let v = build(&spans, Some("depth_pass"), None, None, None, None);

        assert_eq!(v["total_matching"], 2);
        assert_eq!(v["truncated"], false);
        let arr = v["spans"].as_array().expect("spans array");
        assert_eq!(arr.len(), 2);
        for s in arr {
            assert_eq!(s["kind"], "DepthPass");
        }
    }

    #[test]
    fn parent_id_filter_narrows_to_contained_children() {
        let spans = fixture_spans();
        // parent_id=1 is the first DepthPass (covers 0..15).
        let v = build(&spans, None, Some(1), None, None, None);
        let arr = v["spans"].as_array().expect("spans array");
        // Children: Region 0..7 (id=2) and Region 7..15 (id=3). The parent
        // itself is excluded.
        let ids: Vec<u64> = arr.iter().map(|s| s["id"].as_u64().unwrap()).collect();
        assert_eq!(ids, vec![2, 3]);
    }

    #[test]
    fn parent_id_combined_with_kind_filters_correctly() {
        let spans = fixture_spans();
        // parent_id=0 (Operation), kind=region → all 4 regions.
        let v = build(&spans, Some("region"), Some(0), None, None, None);
        assert_eq!(v["total_matching"], 4);
    }

    #[test]
    fn pass_index_filters_to_matching_depth_pass() {
        let spans = fixture_spans();
        let v = build(&spans, None, None, Some(1), None, None);
        let arr = v["spans"].as_array().expect("spans array");
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["kind"], "DepthPass");
        assert_eq!(arr[0]["start_move"], 15);
    }

    #[test]
    fn region_id_filters_to_matching_region() {
        let spans = fixture_spans();
        let v = build(&spans, None, None, None, Some(2), None);
        let arr = v["spans"].as_array().expect("spans array");
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["start_move"], 15);
        assert_eq!(arr[0]["end_move"], 22);
    }

    #[test]
    fn max_spans_truncates_and_reports_total() {
        let spans = fixture_spans();
        let v = build(&spans, Some("region"), None, None, None, Some(2));
        assert_eq!(v["total_matching"], 4);
        assert_eq!(v["truncated"], true);
        assert_eq!(v["max_spans"], 2);
        let arr = v["spans"].as_array().expect("spans array");
        assert_eq!(arr.len(), 2);
    }

    #[test]
    fn default_max_spans_is_50() {
        // Build 60 region spans.
        let mut spans = vec![Span::new(0, 60, SpanKind::Operation)];
        for i in 0..60 {
            spans.push(
                Span::new(i, i + 1, SpanKind::Region).with_payload(SpanPayload::Region {
                    region_id: i as u32,
                }),
            );
        }
        let v = build_inspect_spans_response(
            1,
            0,
            "tp",
            "pocket",
            60,
            &spans,
            true,
            Some("region"),
            None,
            None,
            None,
            None,
        )
        .unwrap();
        assert_eq!(v["total_matching"], 60);
        assert_eq!(v["truncated"], true);
        assert_eq!(v["max_spans"], 50);
        assert_eq!(v["spans"].as_array().unwrap().len(), 50);
    }

    #[test]
    fn invalid_kind_returns_error() {
        let spans = fixture_spans();
        let res = build_inspect_spans_response(
            1,
            0,
            "tp",
            "pocket",
            30,
            &spans,
            true,
            Some("garbage"),
            None,
            None,
            None,
            None,
        );
        assert!(res.is_err());
    }

    #[test]
    fn invalid_parent_id_returns_error() {
        let spans = fixture_spans();
        let res = build_inspect_spans_response(
            1,
            0,
            "tp",
            "pocket",
            30,
            &spans,
            true,
            None,
            Some(999),
            None,
            None,
            None,
        );
        assert!(res.is_err());
    }
}
