//! GUI-side processing of MCP requests. Called from `RsCamApp::update()`.

#![deny(clippy::indexing_slicing)]

use std::path::Path;

use rs_cam_core::compute::config::{
    BoundaryConfig, BoundaryContainment, BoundarySource, ComputeStatus, DressupConfig,
};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId};
use rs_cam_core::session::MutationKind;

use crate::controller::Severity;
use crate::mcp_bridge::{
    GuiBanner, McpRequest, McpRequestKind, McpResponse, MutationResult, MutationWarning,
    ProgressUpdate,
};
use crate::state::Workspace;
use crate::state::selection::Selection;
use crate::ui::AppEvent;

use rs_cam_mcp::server::{json_str, no_project_error, parse_operation_type, parse_tool_type, text};

/// The optional numeric dials `set_rest_analysis_config` accepts, grouped so
/// the handler stays under the argument-count lint after PR-7 added the two
/// routing-fan fields. Every one is `Option` with the SAME meaning: `None` =
/// "I did not say", not "zero".
pub(crate) struct RestAnalysisDials {
    pub cell_mm: Option<f64>,
    pub min_valley_depth: Option<f64>,
    pub region_margin_mm: Option<f64>,
    /// PR-7 (H2.5): `None` here reaches the config as `None` and means "size
    /// it from the canonical reach policy", which is a real instruction, not
    /// an absent value — see `RestAnalysisConfig::offset_stepover_mm`.
    pub offset_stepover_mm: Option<f64>,
    pub num_offset_passes: Option<usize>,
}

impl super::RsCamApp {
    /// Non-blocking drain of MCP requests from the channel.
    /// Called once per frame from `update()`.
    pub(crate) fn drain_mcp_requests(&mut self, ctx: &egui::Context) {
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
            self.handle_mcp_request(ctx, request);
        }

        self.publish_mcp_read_snapshot();
    }

    /// A/M12: republish the cheap, no-argument reads so the MCP server thread
    /// can answer them while this thread is stalled behind a generation.
    ///
    /// Rate limited to [`MCP_READ_PUBLISH_INTERVAL`] so it cannot become a
    /// per-frame cost: at the MCP heartbeat's 100 ms cadence that is at most
    /// ~2 publishes/second of small-JSON rendering, on the GUI thread, and
    /// **nothing at all on the compute lane** — no synchronisation was added
    /// to the generation path.
    fn publish_mcp_read_snapshot(&mut self) {
        const MCP_READ_PUBLISH_INTERVAL: std::time::Duration =
            std::time::Duration::from_millis(500);

        if self
            .mcp_reads_published_at
            .is_some_and(|at| at.elapsed() < MCP_READ_PUBLISH_INTERVAL)
        {
            return;
        }
        self.mcp_reads_published_at = Some(std::time::Instant::now());
        self.mcp_reads.publish(crate::mcp_bridge::McpReadSnapshot {
            list_toolpaths: self.mcp_list_toolpaths(),
            project_summary: self.mcp_project_summary(),
            inspect_model: self.mcp_inspect_model(),
            inspect_stock: self.mcp_inspect_stock(),
            inspect_machine: self.mcp_inspect_machine(),
            published_at: None,
        });
    }

    fn handle_mcp_request(&mut self, ctx: &egui::Context, request: McpRequest) {
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
            McpRequestKind::RecommendClearingStrategy { index } => {
                let resp = self.mcp_recommend_clearing_strategy(index);
                let _ = response_tx.send(McpResponse { result: Ok(resp) });
            }
            McpRequestKind::GetSuggestRationale { index } => {
                let resp = self.mcp_get_suggest_rationale(index);
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
                tool_change_mode,
                split_setups,
            } => {
                let resp = self.mcp_export_gcode(
                    &path,
                    accept_unmodeled_tool_load,
                    accept_exceeded_tool_load,
                    tool_change_mode.as_deref(),
                    split_setups,
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
                    self.controller.state_mut().selection = Selection::Toolpath(tp_id);
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
                    self.controller.state_mut().selection = Selection::Toolpath(tc.id);
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
                source_toolpath_id,
            } => {
                let resp = self.mcp_set_boundary_config(
                    index,
                    enabled,
                    source.as_deref(),
                    containment.as_deref(),
                    offset,
                    source_toolpath_id,
                );
                let _ = response_tx.send(McpResponse { result: Ok(resp) });
            }
            McpRequestKind::SetRestAnalysisConfig {
                index,
                enabled,
                reference_tool_id,
                cell_mm,
                min_valley_depth,
                region_margin_mm,
                offset_stepover_mm,
                num_offset_passes,
            } => {
                let resp = self.mcp_set_rest_analysis_config(
                    index,
                    enabled,
                    reference_tool_id,
                    &RestAnalysisDials {
                        cell_mm,
                        min_valley_depth,
                        region_margin_mm,
                        offset_stepover_mm,
                        num_offset_passes,
                    },
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
            McpRequestKind::GenerateAll {
                fixpoint,
                simulation_resolution_mm,
            } => {
                self.controller.push_notification(
                    "MCP: Generating all toolpaths...".to_owned(),
                    Severity::Info,
                );
                self.mcp_generate_all(fixpoint, simulation_resolution_mm, response_tx, progress_tx);
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
            McpRequestKind::ScreenshotGui {
                path,
                width,
                height,
            } => {
                // Deferred response: the handler stores response_tx in the
                // pending slot and the capture completes 1-2 frames later.
                self.mcp_screenshot_gui(ctx, &path, width, height, response_tx);
            }

            // ── UI navigation ────────────────────────────────────────
            McpRequestKind::SetUiView {
                workspace,
                toolpath_index,
                properties_tab,
                select,
                modal,
            } => {
                let resp = self.mcp_set_ui_view(
                    workspace.as_deref(),
                    toolpath_index,
                    properties_tab.as_deref(),
                    select.as_deref(),
                    modal.as_deref(),
                );
                let _ = response_tx.send(McpResponse { result: Ok(resp) });
            }
            McpRequestKind::ImportMachineSettings { dump } => {
                self.controller
                    .events_mut()
                    .push(AppEvent::SwitchWorkspace(Workspace::Setup));
                let resp = self.mcp_import_machine_settings(&dump);
                let _ = response_tx.send(McpResponse { result: Ok(resp) });
            }
            McpRequestKind::ListMachineLibrary => {
                let resp = self.mcp_list_machine_library();
                let _ = response_tx.send(McpResponse { result: Ok(resp) });
            }
            McpRequestKind::LoadMachineFromLibrary { name } => {
                self.controller
                    .events_mut()
                    .push(AppEvent::SwitchWorkspace(Workspace::Setup));
                let resp = self.mcp_load_machine_from_library(&name);
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
                // A/M11: `enabled: false` wins over whatever the op last
                // recorded, so a switched-off toolpath can never present the
                // rest-stock error it had while it was on.
                let raw = rt.map_or(&ComputeStatus::Pending, |r| &r.status);
                let status = ComputeStatus::effective(s.enabled, raw);
                serde_json::json!({
                    "index": s.index,
                    "id": s.id,
                    "name": s.name,
                    "operation_label": s.operation_label,
                    "enabled": s.enabled,
                    "tool_name": s.tool_name,
                    "stale": stale,
                    "status": status.label(),
                    "error": status.error_text(),
                    "awaiting_prior_stock": status.blocked_on().map(|b| serde_json::json!({
                        "blocking_toolpath_id": b.blocking_toolpath_id,
                        "blocking_toolpath_index": b.blocking_toolpath_index,
                        "message": b.message,
                    })),
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
                            obj.insert(
                                "included_angle_deg".to_owned(),
                                serde_json::json!(t.included_angle),
                            );
                        }
                        ToolType::TaperedBallNose => {
                            obj.insert(
                                "taper_half_angle_deg".to_owned(),
                                serde_json::json!(t.taper_half_angle),
                            );
                            obj.insert(
                                "shaft_diameter_mm".to_owned(),
                                serde_json::json!(t.shaft_diameter),
                            );
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
                    // P2.2: reports the machining boundary, including
                    // `derived_rest_regions`'s `source_toolpath_id` — there
                    // is no dedicated get_boundary_config tool, so this is
                    // the only MCP surface for reading it back.
                    "boundary": tc.boundary,
                    // P2.5: op-agnostic rest analysis config — mirrors `boundary`'s
                    // presence here; set via `set_rest_analysis_config`.
                    "rest_analysis": tc.rest_analysis,
                    "runtime": self.mcp_runtime_status_for_toolpath_id(tc.id),
                }))
            }
            None => {
                json_str(serde_json::json!({"error": format!("Toolpath index {index} not found")}))
            }
        }
    }

    fn mcp_runtime_status_for_toolpath_id(
        &self,
        toolpath_id: rs_cam_core::ToolpathId,
    ) -> serde_json::Value {
        let state = self.controller.state();
        let rt = state.gui.toolpath_rt.get(&toolpath_id);
        let enabled = state
            .session
            .find_toolpath_config_by_id(toolpath_id)
            .is_none_or(|(_, tc)| tc.enabled);
        let raw = rt.map_or(&ComputeStatus::Pending, |r| &r.status);
        let status = ComputeStatus::effective(enabled, raw);
        serde_json::json!({
            "status": status.label(),
            "error": status.error_text(),
            "awaiting_prior_stock": status.blocked_on().map(|b| serde_json::json!({
                "blocking_toolpath_id": b.blocking_toolpath_id,
                "blocking_toolpath_index": b.blocking_toolpath_index,
                "message": b.message,
            })),
            // A/M6: what the three-valued `claims_reference` param actually
            // RESOLVED to, and why. It belongs here and not under `params`
            // because it is not a param: `auto` resolves against whether a
            // machined prior stock was in scope at generation time, which no
            // config field records. `null` until the operation has generated,
            // and on any operation that runs no claims pipeline.
            "claims_reference": rt
                .and_then(|r| r.result.as_ref())
                .and_then(|res| res.stats.claims_reference)
                .map(|f| serde_json::json!({
                    "setting": f.resolution.setting(),
                    "resolved": f.resolution.reference(),
                    "resolution": f.resolution.label(),
                    "derived": f.resolution.is_derived(),
                    "prior_stock_in_scope": f.resolution.prior_stock_in_scope(),
                    "needs_attention": f.resolution.needs_attention(),
                    "territory_clip_requested": f.territory_clip_requested,
                    "territory_clip_skipped": f.territory_clip_skipped(),
                    "why": f.resolution.why(),
                })),
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

    /// The tool a narration describes: the toolpath's OWN tool, or nothing.
    ///
    /// **B7 divergence 3 — a defect, fixed 2026-08-06.** The GUI narration
    /// used to append `.or_else(|| tools().first())` to this lookup, so when
    /// `tool_id` did not resolve it narrated with **another tool's
    /// geometry**: the diameter behind the large-arc threshold, the flute
    /// count behind every chipload sentence, and the cutter handed to
    /// `narrate_toolpath_with_context` itself. Nothing in the emitted text
    /// said so. Core's sibling narration has always errored instead
    /// (`session/compute.rs`, `SessionError::ToolNotFound`).
    ///
    /// A tool id that does not resolve is a broken project, not a routine
    /// state, so the honest answer is a refusal naming the id — which is
    /// what the caller emits. Exhibit:
    /// `narration_tool_lookup_refuses_where_the_parent_took_another_tool`.
    fn narration_tool_for(
        tools: &[rs_cam_core::compute::tool_config::ToolConfig],
        tool_id: usize,
    ) -> Option<&rs_cam_core::compute::tool_config::ToolConfig> {
        tools.iter().find(|tool| tool.id.0 == tool_id)
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
        let Some(tool_config) = Self::narration_tool_for(state.session.tools(), tc.tool_id) else {
            return format!(
                "Error: toolpath {index} references tool id {} but no such tool is configured.                  Narration refuses rather than describing this toolpath with another tool's                  geometry.",
                tc.tool_id,
            );
        };

        let tool = rs_cam_core::compute::build_cutter(tool_config);
        let cut_trace = state
            .simulation
            .results
            .as_ref()
            .and_then(|sim| sim.cut_trace.as_deref());
        // B7 divergence 4: prefer the traces carried by the RESULT being
        // narrated, and only then the runtime's. The old order preferred
        // `rt.*`, so a narration could describe `result`'s move list using a
        // trace produced by a later generation. Core has no `rt` overlay and
        // has always read `result.*`; this makes the GUI agree, while still
        // falling back to `rt.*` for the paths that only populate there.
        let semantic_trace = result
            .semantic_trace
            .as_deref()
            .or(rt.semantic_trace.as_deref());
        let debug_trace = result.debug_trace.as_deref().or(rt.debug_trace.as_deref());
        // Checkpoint D Q2: narration reads the same measurability report the
        // gates and the triage do, so the MCP narration cannot publish an
        // air-cut percentage the gates have already declined to act on.
        //
        // B7 divergence 2: the cell size is the one the TRACE was measured
        // at (`SimulationResult::column_grid_cell_mm`), not the resolution
        // dial's current value — those are two different quantities, and the
        // measurability floors are cell-size dependent, so reading the dial
        // could return a different `NotMeasurable` verdict from core's on
        // identical evidence. `state.simulation.resolution` is what the next
        // simulation WILL use; it is not a property of this trace.
        let measurability = cut_trace.map(|trace| {
            rs_cam_core::sim_measurability::MeasurabilityReport::from_trace(
                trace,
                state
                    .simulation
                    .results
                    .as_ref()
                    .map(|sim| sim.column_grid_cell_mm),
            )
        });
        let mut context = rs_cam_core::narrate::ToolpathNarrationContext {
            measurability: measurability.as_ref(),
            toolpath_id: Some(tc.id),
            toolpath_name: Some(tc.name.as_str()),
            operation_label: Some(tc.operation.label()),
            operation_kind: Some(tc.operation.op_type()),
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
            // B7 divergence 1: the shared expression. Core used op-type
            // alone (blind to a generator that emits Drilling moves without
            // declaring a drill op type); this side used op-type OR ANY
            // Drilling move (which called a v-carve with a drilled entry a
            // drill cycle and suppressed its air-cut anomaly). The shared
            // helper is neither — see its doc.
            is_drill_cycle: rs_cam_core::narrate::is_drill_cycle_for_narration(
                tc.operation.op_type(),
                &result.annotated.toolpath.moves,
            ),
            material: Some(&state.session.stock_config().material),
            // Every ToolpathStats-derived channel is filled by
            // `absorb_stats` below — the SAME join core's narration uses, so
            // a new finding cannot reach one narration and miss the other
            // (B7). The GUI worker fills `result.stats` from the core
            // generation findings; carrying them here is what puts those
            // figures in front of an agent narrating a live GUI toolpath.
            ..Default::default()
        };
        context.absorb_stats(&result.stats);

        rs_cam_core::narrate::narrate_toolpath_with_context(
            result.annotated.as_ref(),
            semantic_trace,
            cut_trace,
            debug_trace,
            &tool,
            &context,
        )
    }

    /// v3.2 (2026-06-04): Combined-Suggest rationale for the toolpath
    /// at `index`. Returns a [`rs_cam_core::feeds::rationale::SuggestRationale`]
    /// payload as JSON describing every pass the orchestrator ran and
    /// why each parameter landed where it did (DPP back-off,
    /// chipload-target feed lift, runtime-stepover floor, etc.).
    ///
    /// Shares the GUI feeds modal's invocation via
    /// `ProjectSession::cutter_op_profile` (T10 dedup) so the agent and
    /// the operator see the same surface. Does *not* mutate the
    /// project — the agent decides whether to follow up with
    /// `set_toolpath_param`.
    fn mcp_get_suggest_rationale(&self, index: usize) -> String {
        let state = self.controller.state();
        let Some(tc) = state.session.get_toolpath_config(index) else {
            return json_str(serde_json::json!({
                "error": format!("Toolpath index {index} not found")
            }));
        };
        let Some(profile) = state.session.cutter_op_profile(tc) else {
            return json_str(serde_json::json!({
                "error": format!("Tool {} for toolpath {} not found", tc.tool_id, tc.id)
            }));
        };
        match profile.feasibility {
            Ok(()) => {
                let rationale = rs_cam_core::feeds::rationale::SuggestRationale::from_warnings(
                    &profile.warnings,
                );
                json_str(serde_json::json!({
                    "toolpath_id": tc.id,
                    "toolpath_name": tc.name,
                    "rationale": serde_json::to_value(&rationale).unwrap_or_default(),
                }))
            }
            Err(e) => json_str(serde_json::json!({
                "toolpath_id": tc.id,
                "toolpath_name": tc.name,
                "error": format!("Suggest refused: {e}"),
            })),
        }
    }

    /// Strategy advisor (`STRATEGY_ADVISOR_2026-06-17`): plan each candidate
    /// clearing strategy for the Adaptive3d toolpath at `index` at its
    /// load-limited params and recommend the one with the minimum
    /// acceleration-aware wall-clock. Returns chosen strategy, the binding
    /// regime as the *why*, every candidate ranked by wall-clock, and the
    /// speed margin. Heavy (plans one toolpath per candidate) and runs
    /// synchronously, so the GUI is unresponsive while it computes. Does not
    /// mutate the project.
    fn mcp_recommend_clearing_strategy(&self, index: usize) -> String {
        let cancel = std::sync::atomic::AtomicBool::new(false);
        let state = self.controller.state();
        match state.session.recommend_clearing_strategy(index, &cancel) {
            Ok(Some(rec)) => json_str(serde_json::json!({
                "chosen": format!("{:?}", rec.chosen),
                "regime": format!("{:?}", rec.regime),
                "reason": rec.reason,
                "time_ratio_vs_runner_up": rec.time_ratio_vs_runner_up,
                "ranked": rec
                    .ranked
                    .iter()
                    .map(|r| {
                        serde_json::json!({
                            "strategy": format!("{:?}", r.strategy),
                            "wall_clock_s": r.wall_clock_s,
                            "regime": format!("{:?}", r.regime),
                        })
                    })
                    .collect::<Vec<_>>(),
            })),
            Ok(None) => json_str(serde_json::json!({
                "error": format!(
                    "Toolpath {index} is not an Adaptive3d op (or no candidate planned a usable path); the strategy advisor only applies to 3D adaptive roughing"
                ),
            })),
            Err(e) => json_str(serde_json::json!({
                "error": format!("{e}"),
            })),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn mcp_get_cut_trace(
        &self,
        toolpath_id: Option<usize>, // wire-level raw id; typed immediately below
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
        let toolpath_id = toolpath_id.map(rs_cam_core::ToolpathId);
        let mut accepted_by_toolpath: std::collections::HashMap<
            rs_cam_core::ToolpathId,
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
        let span_path_matches = |tp_id: rs_cam_core::ToolpathId, path: &[SpanId]| -> bool {
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
        // R-1 (census §8.2): both arrays are capped and, until now, nothing
        // in the response said so. An agent that read `issues` and compared
        // its length against `issue_count` saw a silent disagreement and had
        // no way to tell truncation from a filter.
        let issues_truncated = issue_count > max_i;
        let hotspots_truncated = hotspot_count > max_h;

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

        // P0 unified-finishing probe — compact per-toolpath runtime block.
        // `runtime_by_intent` is the F-034 integrator time bucketed by
        // MoveIntent class (None when the sim ran without kinematics).
        use rs_cam_core::simulation_cut::AirCutRatios;
        let toolpath_summaries_val: Vec<serde_json::Value> = ct
            .toolpath_summaries
            .iter()
            .filter(|s| toolpath_id.is_none_or(|id| s.toolpath_id == id))
            .map(|s| {
                serde_json::json!({
                    "toolpath_id": s.toolpath_id,
                    "total_runtime_s": s.total_runtime_s,
                    "cutting_runtime_s": s.cutting_runtime_s,
                    "rapid_runtime_s": s.rapid_runtime_s,
                    "air_cut_time_s": s.air_cut_time_s,
                    // LH-1: both denominators, both named. Thresholds follow
                    // the total-runtime reading; the MCP narration line
                    // reports the cutting-time one.
                    "air_cut_pct_of_total_runtime": s.air_cut_pct_of_total_runtime(),
                    "air_cut_pct_of_cutting_time": s.air_cut_pct_of_cutting_time(),
                    "low_engagement_time_s": s.low_engagement_time_s,
                    "metrics_not_applicable": s.metrics_not_applicable,
                    "runtime_by_intent": s.runtime_by_intent,
                })
            })
            .collect();

        // R-2 (census §3.5 D4): this response carried TWO fields named
        // `issue_count` measuring different populations — the top-level one
        // (this request's filters applied) and `summary.issue_count` nested
        // inside `summary` (the whole trace, filters ignored). An agent
        // reading a filtered response could pick either and both looked
        // authoritative. The legacy keys keep their exact values for wire
        // compatibility; the disambiguating names sit beside them and say
        // which population each counts.
        json_str(serde_json::json!({
            "summary": summary_val,
            "semantic_summaries": summaries_val,
            "span_summaries": span_summaries,
            "hotspots": hotspots_val,
            "hotspot_count": hotspot_count,
            "hotspots_truncated": hotspots_truncated,
            "hotspots_total_matching": hotspot_count,
            "hotspots_returned": hotspots_val.as_array().map_or(0, |a| a.len()),
            "issue_count": issue_count,
            "issues": issues_val,
            "issues_truncated": issues_truncated,
            "issues_total_matching": issue_count,
            "issues_returned": issues_val.as_array().map_or(0, |a| a.len()),
            // Explicit aliases for the two same-named counts, so neither has
            // to be inferred from where it sits in the object.
            "issue_count_matching_filter": issue_count,
            "issue_count_project_wide": ct.summary.issue_count,
            // And what the number actually IS: coalesced contiguous
            // air/low-engagement RUNS, not per-sample tallies. The per-sample
            // tallies are `air_cut_issue_count` / `low_engagement_issue_count`
            // on the semantic summaries, and on the census fixture they were
            // 43x larger under a near-identical name.
            "issue_count_population": "coalesced_segments",
            "drill_summaries": drill_summaries_val,
            "drill_samples": drill_samples_val,
            "toolpath_summaries": toolpath_summaries_val,
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

        // Acceleration-aware kinematics ($11 + per-axis $120-122). `None`
        // means the cycle-time model falls back to the naive distance/feed
        // sum (the F-034 feature flag).
        let kinematics = match &machine.kinematics {
            Some(k) => {
                let per_axis = match k.acceleration_xyz_mm_s2 {
                    Some([ax, ay, az]) => serde_json::json!([ax, ay, az]),
                    None => serde_json::Value::Null,
                };
                serde_json::json!({
                    "configured": true,
                    "acceleration_mm_s2": k.acceleration_mm_s2,
                    "acceleration_xyz_mm_s2": per_axis,
                    "junction_deviation_mm": k.junction_deviation_mm,
                    "jerk_mm_s3": k.jerk_mm_s3,
                    "max_junction_velocity_mm_min": k.max_junction_velocity_mm_min,
                })
            }
            None => serde_json::json!({
                "configured": false,
                "note": "no kinematics set — cycle time uses the naive distance/feed sum",
            }),
        };

        let r = &machine.rigidity;
        json_str(serde_json::json!({
            "name": machine.name,
            "max_feed_mm_min": machine.max_feed_mm_min,
            "cutting_feed_ceiling_mm_min": machine.cutting_feed_ceiling_mm_min(),
            "max_shank_mm": machine.max_shank_mm,
            "safety_factor": machine.safety_factor,
            "spindle": spindle,
            "power": power,
            "kinematics": kinematics,
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

    /// Import a GRBL `$$` dump onto the live machine: sets kinematics +
    /// max feed, breaks any library link. Headless twin of the GUI Machine
    /// panel's `$$` import.
    fn mcp_import_machine_settings(&mut self, dump: &str) -> String {
        use rs_cam_core::machine_kinematics::{MachineKinematics, default_junction_deviation_mm};
        let imp = MachineKinematics::from_grbl_settings(dump);
        let recognized = imp.kinematics.acceleration_xyz_mm_s2.is_some()
            || imp.max_feed_mm_min.is_some()
            || imp.arc_tolerance_mm.is_some()
            || imp.max_spindle_rpm.is_some()
            || (imp.kinematics.junction_deviation_mm - default_junction_deviation_mm()).abs()
                > 1e-12;
        if !recognized {
            return json_str(serde_json::json!({
                "ok": false,
                "error": "No GRBL settings recognised in the dump (expected $N=value lines, \
                          e.g. $11=…, $120=…).",
            }));
        }

        let prev_max_feed = self.controller.state().session.machine().max_feed_mm_min;
        {
            let session = &mut self.controller.state_mut().session;
            let machine = session.machine_mut();
            machine.kinematics = Some(imp.kinematics);
            if let Some(mf) = imp.max_feed_mm_min {
                machine.max_feed_mm_min = mf;
            }
            // Inline values now — drop any machine-library link.
            session.set_machine_ref(None);
        }
        self.controller.events_mut().push(AppEvent::MachineChanged);

        let per_axis = match imp.kinematics.acceleration_xyz_mm_s2 {
            Some([ax, ay, az]) => serde_json::json!([ax, ay, az]),
            None => serde_json::Value::Null,
        };
        let new_max_feed = self.controller.state().session.machine().max_feed_mm_min;
        json_str(serde_json::json!({
            "ok": true,
            "applied": {
                "acceleration_xyz_mm_s2": per_axis,
                "acceleration_mm_s2": imp.kinematics.acceleration_mm_s2,
                "junction_deviation_mm": imp.kinematics.junction_deviation_mm,
                "max_feed_mm_min": { "from": prev_max_feed, "to": new_max_feed },
                "arc_tolerance_mm": imp.arc_tolerance_mm,
                "max_spindle_rpm": imp.max_spindle_rpm,
                "ignored_settings": imp.ignored_count,
            },
            "note": "kinematics applied; machine-library link cleared. Verify with inspect_machine.",
        }))
    }

    /// List the per-user machine library with a compact spec summary per
    /// entry (snapshot model — these are import sources, not live links).
    fn mcp_list_machine_library(&self) -> String {
        let names = rs_cam_core::machine_library::list();
        let machines: Vec<serde_json::Value> = names
            .iter()
            .map(|name| match rs_cam_core::machine_library::load(name) {
                Ok(p) => {
                    let kinematics = match &p.kinematics {
                        Some(k) => {
                            let per_axis = match k.acceleration_xyz_mm_s2 {
                                Some([ax, ay, az]) => serde_json::json!([ax, ay, az]),
                                None => serde_json::Value::Null,
                            };
                            serde_json::json!({
                                "acceleration_xyz_mm_s2": per_axis,
                                "acceleration_mm_s2": k.acceleration_mm_s2,
                                "junction_deviation_mm": k.junction_deviation_mm,
                            })
                        }
                        None => serde_json::Value::Null,
                    };
                    serde_json::json!({
                        "name": name,
                        "profile_name": p.name,
                        "max_feed_mm_min": p.max_feed_mm_min,
                        "kinematics": kinematics,
                    })
                }
                Err(e) => serde_json::json!({ "name": name, "error": e.to_string() }),
            })
            .collect();
        let count = machines.len();
        json_str(serde_json::json!({ "count": count, "machines": machines }))
    }

    /// Snapshot-import the named library machine into the project's inline
    /// machine (a COPY; no live link), then invalidate machine-dependent
    /// state via `MachineChanged`.
    fn mcp_load_machine_from_library(&mut self, name: &str) -> String {
        let profile = match rs_cam_core::machine_library::load(name) {
            Ok(p) => p,
            Err(e) => {
                return json_str(serde_json::json!({
                    "ok": false,
                    "error": format!("could not load machine '{name}' from the library: {e}"),
                }));
            }
        };
        let profile_name = profile.name.clone();
        *self.controller.state_mut().session.machine_mut() = profile;
        self.controller.events_mut().push(AppEvent::MachineChanged);
        json_str(serde_json::json!({
            "ok": true,
            "imported": name,
            "profile_name": profile_name,
            "note": "snapshot copy applied to the project's inline machine; verify with inspect_machine",
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
                // A/M11: only genuine failures. A disabled op reports
                // `Disabled` (no error text) and a sequencing block reports
                // `AwaitingPriorStock` — neither belongs in an error list an
                // agent has to triage.
                let error = ComputeStatus::effective(tc.enabled, &rt.status).error_text()?;
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
        let ids: Vec<rs_cam_core::ToolpathId> = stale
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
        let tp_id = tc.id;
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
        &mut self,
        path: &str,
        accept_unmodeled_tool_load: bool,
        accept_exceeded_tool_load: bool,
        tool_change_mode: Option<&str>,
        split_setups: bool,
    ) -> String {
        // Apply the requested tool-change handling to the session wizard
        // before export so `overlay_for` picks it up — the MCP equivalent
        // of the export wizard's Tool Change dropdown. Without this, MCP
        // exports always used the post default (M0 manual pause), which is
        // wrong for gSender/BitSetter setups that need M6 to trigger the
        // tool-length probe on every change.
        if let Some(mode_str) = tool_change_mode {
            let mode = match mode_str.to_ascii_lowercase().as_str() {
                "pause" | "m0" | "manual" => rs_cam_core::gcode::ToolChangeMode::Pause,
                "m6" | "atc" => rs_cam_core::gcode::ToolChangeMode::M6,
                "suppress" | "none" => rs_cam_core::gcode::ToolChangeMode::Suppress,
                other => {
                    return text(format!(
                        "Export failed: unknown tool_change_mode '{other}' (expected 'pause', 'm6', or 'suppress')"
                    ));
                }
            };
            self.controller
                .state_mut()
                .session
                .wizard_mut()
                .tool_change_override = Some(mode);
        }
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

        // Two-sided / multi-setup split: one self-contained file per setup,
        // each with a header naming the setup (+ a flip/re-zero reminder on
        // setups after the first), so the operator runs setup 1 → flip &
        // re-zero → setup 2. Separate program runs are safer than an in-stream
        // M0 because the Z re-zero after a flip is a fresh job, not a
        // mid-program jog. tool_change_mode (set on the wizard above) still
        // applies within each file.
        if split_setups {
            let setups: Vec<(usize, String)> = state
                .session
                .list_setups()
                .iter()
                .map(|s| (s.id, s.name.clone()))
                .collect();
            if setups.len() > 1 {
                let path_buf = Path::new(path);
                let stem = path_buf
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("export");
                let ext = path_buf
                    .extension()
                    .and_then(|s| s.to_str())
                    .unwrap_or("nc");
                let parent = path_buf.parent();
                let post = state.gui.post.format.definition();
                let total = setups.len();
                let mut written: Vec<String> = Vec::new();
                for (i, (id, name)) in setups.iter().enumerate() {
                    let gcode = match crate::io::export::export_setup_gcode_from_session_with_policy(
                        &state.session,
                        &state.gui,
                        &state.simulation,
                        crate::state::job::SetupId(*id),
                        rs_cam_core::gcode::ToolLoadExportPolicy {
                            accept_unmodeled: accept_unmodeled_tool_load,
                            accept_exceeded: accept_exceeded_tool_load,
                        },
                    ) {
                        Ok(g) => g,
                        Err(e) => return text(format!("Export failed (setup '{name}'): {e}")),
                    };
                    let reminder = if i > 0 {
                        " -- FLIP PART + RE-ZERO Z BEFORE RUNNING"
                    } else {
                        ""
                    };
                    let header = post.render_comment(&format!(
                        "rs_cam setup {}/{total}: \"{name}\"{reminder}",
                        i + 1
                    ));
                    let safe_name: String = name
                        .chars()
                        .map(|c| if c.is_alphanumeric() { c } else { '_' })
                        .collect();
                    let file_name = format!("{stem}_{}_{safe_name}.{ext}", i + 1);
                    let out_path = match parent {
                        Some(p) => p.join(file_name),
                        None => std::path::PathBuf::from(file_name),
                    };
                    if let Err(e) = std::fs::write(&out_path, format!("{header}{gcode}")) {
                        return text(format!("Export failed writing {}: {e}", out_path.display()));
                    }
                    written.push(out_path.display().to_string());
                }
                return text(format!(
                    "Exported {total} per-setup G-code files:\n{}",
                    written.join("\n")
                ));
            }
        }

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
            return self
                .mcp_mutation_error(format!("Error: toolpath index {index} not found"), None);
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
        let (op_config, feeds_provenance) = match rs_cam_core::feeds::suggest::suggest_params(
            rs_cam_core::feeds::suggest::SuggestParamsInput {
                op_type,
                tool,
                machine: session.machine(),
                material: &session.stock_config().material,
                workholding: session.stock_config().workholding_rigidity,
                lut: rs_cam_core::feeds::embedded_vendor_lut(),
                stock_ctx: &stock_ctx,
                spindle_strategy: rs_cam_core::feeds::SpindleStrategy::default(),
                // TODO(v1.2): wire model_bbox once add_toolpath via MCP
                // takes an explicit model_id; today the model is picked
                // post-creation.
                context: rs_cam_core::feeds::suggest::SuggestContext::default(),
            },
        ) {
            Ok(s) => (s.operation, s.provenance),
            Err(e) => {
                return self.mcp_mutation_error(format!("Cannot add toolpath: {e}"), None);
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
            id: rs_cam_core::ToolpathId(0),
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
            rest_analysis: rs_cam_core::compute::config::RestAnalysisConfig::default(),
            stock_source: rs_cam_core::compute::config::StockSource::default(),
            coolant: rs_cam_core::gcode::CoolantMode::default(),
            face_selection: None,
            debug_options: rs_cam_core::debug_trace::ToolpathDebugOptions::default(),
            feeds_provenance,
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
        source_toolpath_id: Option<usize>,
    ) -> String {
        let before = self.mcp_diagnostic_snapshot();
        let boundary_source = match source {
            Some("stock") | None => BoundarySource::Stock,
            Some("model_silhouette") => BoundarySource::ModelSilhouette,
            Some("derived_rest_regions") => {
                let Some(raw_id) = source_toolpath_id else {
                    return self.mcp_mutation_error(
                        "Error: 'derived_rest_regions' requires source_toolpath_id — the id \
                         of the toolpath whose pencil rest-depth result supplies the \
                         boundary (see get_toolpath_params's 'id' field)."
                            .to_owned(),
                        Some("source_toolpath_id".to_owned()),
                    );
                };
                let source_id = rs_cam_core::ToolpathId(raw_id);
                if self
                    .controller
                    .state()
                    .session
                    .find_toolpath_config_by_id(source_id)
                    .is_none()
                {
                    return self.mcp_mutation_error(
                        format!(
                            "Error: source_toolpath_id {raw_id} does not match any \
                             toolpath in this project."
                        ),
                        Some("source_toolpath_id".to_owned()),
                    );
                }
                BoundarySource::DerivedRestRegions {
                    source_toolpath_id: source_id,
                }
            }
            Some(other) => {
                return self.mcp_mutation_error(
                    format!(
                        "Error: Unknown boundary source '{other}'. Use 'stock', \
                         'model_silhouette', or 'derived_rest_regions'."
                    ),
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
                let mut stale = self.mcp_apply_stale(MutationKind::ToolpathParamChanged {
                    toolpath_index: index,
                });
                // P2 pencil-panel consolidation: `set_boundary_config` may
                // have just auto-enabled rest analysis on the SOURCE
                // toolpath (`ProjectSession::auto_enable_rest_analysis_for_source`,
                // the demand-driven producer hook — see its doc comment).
                // Surface that toolpath as stale too so a live GUI session
                // watching this MCP-driven change sees it needs
                // regeneration, mirroring the GUI boundary picker's own
                // handling in `properties/mod.rs`. Slightly conservative:
                // fires whenever the boundary points at an already-enabled
                // source too (harmless — just an extra "needs regen" nudge).
                // Resolved to owned values in its own block so the
                // immutable `state()` borrow ends before `state_mut()`
                // below.
                let source_rest_lookup =
                    if let BoundarySource::DerivedRestRegions { source_toolpath_id } =
                        &boundary.source
                    {
                        let state = self.controller.state();
                        state
                            .session
                            .find_toolpath_config_by_id(*source_toolpath_id)
                            .map(|(idx, tc)| {
                                let is_rest_depth_pencil = matches!(
                                    &tc.operation,
                                    rs_cam_core::compute::catalog::OperationConfig::Pencil(cfg)
                                        if rs_cam_core::pencil::PencilDetector::parse(&cfg.detector)
                                            == rs_cam_core::pencil::PencilDetector::RestDepth
                                );
                                (idx, is_rest_depth_pencil)
                            })
                    } else {
                        None
                    };
                if boundary.enabled
                    && let BoundarySource::DerivedRestRegions { source_toolpath_id } =
                        &boundary.source
                    && let Some((source_index, false)) = source_rest_lookup
                {
                    if let Some(rt) = self
                        .controller
                        .state_mut()
                        .gui
                        .toolpath_rt
                        .get_mut(source_toolpath_id)
                    {
                        rt.stale_since = Some(std::time::Instant::now());
                    }
                    if !stale.contains(&source_index) {
                        stale.push(source_index);
                    }
                }
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

    fn mcp_set_rest_analysis_config(
        &mut self,
        index: usize,
        enabled: bool,
        reference_tool_id: Option<usize>,
        dials: &RestAnalysisDials,
    ) -> String {
        let before = self.mcp_diagnostic_snapshot();
        let resolved_reference_tool_id = match reference_tool_id {
            Some(raw_id) => {
                let tool_id = rs_cam_core::compute::tool_config::ToolId(raw_id);
                if !self
                    .controller
                    .state()
                    .session
                    .tools()
                    .iter()
                    .any(|t| t.id == tool_id)
                {
                    return self.mcp_mutation_error(
                        format!(
                            "Error: reference_tool_id {raw_id} does not match any tool in \
                             this project."
                        ),
                        Some("reference_tool_id".to_owned()),
                    );
                }
                Some(tool_id)
            }
            None => None,
        };

        let defaults = rs_cam_core::compute::config::RestAnalysisConfig::default();
        let rest_analysis = rs_cam_core::compute::config::RestAnalysisConfig {
            enabled,
            reference_tool_id: resolved_reference_tool_id,
            cell_mm: dials.cell_mm.unwrap_or(defaults.cell_mm),
            min_valley_depth: dials.min_valley_depth.unwrap_or(defaults.min_valley_depth),
            region_margin_mm: dials.region_margin_mm.unwrap_or(defaults.region_margin_mm),
            // PR-7 (H2.5): pass the `Option`s STRAIGHT through. Unset is not
            // a missing value to be filled in with a default here — it is
            // the instruction "size this from the reach policy", and only
            // the generation path knows the cutter to size it against.
            offset_stepover_mm: dials.offset_stepover_mm,
            num_offset_passes: dials.num_offset_passes,
        };

        match self
            .controller
            .state_mut()
            .session
            .set_rest_analysis_config(index, rest_analysis.clone())
        {
            Ok(()) => {
                self.controller.state_mut().gui.mark_edited();
                let stale = self.mcp_apply_stale(MutationKind::ToolpathParamChanged {
                    toolpath_index: index,
                });
                self.mcp_mutation_result(
                    format!("Rest analysis set on toolpath {index}. Regenerate to apply."),
                    serde_json::to_value(rest_analysis).unwrap_or(serde_json::Value::Null),
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
            "max_speed" | "MaxSpeed" | "maxspeed" => rs_cam_core::feeds::SpindleStrategy::MaxSpeed,
            other => {
                return self.mcp_mutation_error(
                    format!(
                        "Error: unknown spindle_strategy '{other}'. Expected 'match_chart' or 'max_speed'."
                    ),
                    Some("spindle_strategy".to_owned()),
                );
            }
        };
        if self
            .controller
            .state()
            .session
            .post_config()
            .spindle_strategy
            == parsed
        {
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
        self.controller.state_mut().gui.post.spindle_strategy = parsed;
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
        let tp_id = tc.id;

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
        fixpoint: Option<bool>,
        simulation_resolution_mm: Option<f64>,
        response_tx: tokio::sync::oneshot::Sender<McpResponse>,
        progress_tx: Option<tokio::sync::mpsc::Sender<ProgressUpdate>>,
    ) {
        // A/M11: the whole ladder lives on the controller, which owns the
        // compute lane, the simulation state and `pending_mcp` — the three
        // things a fixpoint loop has to coordinate. This is a thin adapter.
        self.controller.mcp_start_generate_all(
            fixpoint.unwrap_or(true),
            simulation_resolution_mm,
            response_tx,
            progress_tx,
        );
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

    // ── Full-window GUI screenshot (screenshot_gui) ──────────────────

    /// Start a full-window GUI capture. Deferred-response pattern
    /// (mirrors `pending.collision`): store the path + response sender
    /// in `pending.gui_screenshot`, optionally resize the window, and
    /// let the per-frame pump issue `ViewportCommand::Screenshot` once
    /// the resize has settled. `complete_mcp_gui_screenshot` finishes
    /// the response when the `egui::Event::Screenshot` result arrives
    /// 1-2 frames later.
    fn mcp_screenshot_gui(
        &mut self,
        ctx: &egui::Context,
        path: &str,
        width: Option<f32>,
        height: Option<f32>,
        response_tx: tokio::sync::oneshot::Sender<McpResponse>,
    ) {
        if !path.ends_with(".png") {
            let _ = response_tx.send(McpResponse {
                result: Ok(text(format!(
                    "screenshot_gui only writes PNG — path must end in .png (got '{path}')"
                ))),
            });
            return;
        }
        let Some(pending) = self.controller.pending_mcp.as_mut() else {
            let _ = response_tx.send(McpResponse {
                result: Err("MCP compute tracking not initialized".to_owned()),
            });
            return;
        };
        if pending.gui_screenshot.is_some() {
            let _ = response_tx.send(McpResponse {
                result: Ok(text(
                    "Another screenshot_gui capture is already in flight — \
                     retry after it completes.",
                )),
            });
            return;
        }

        // Optional resize before capture (logical points). The capture is
        // deferred a few frames so the window system has applied the new
        // size. The size is NOT restored afterwards — it sticks (documented
        // in the tool description).
        //
        // Even without a resize, captures settle 2 frames: view mutations
        // queued by a preceding set_ui_view (one-shot tab overrides, modal
        // opens routed through the event queue) take an event-processing
        // frame plus a render frame to reach pixels. Capturing at 0 raced
        // that pipeline — the 2026-06-11 sweep needed a throwaway "flush
        // shot" per tab change (Batch 3 item 13).
        let frames_before_capture = if width.is_some() || height.is_some() {
            let current = ctx.input(|i| i.viewport().inner_rect).map(|r| r.size());
            let w = width.unwrap_or_else(|| current.map_or(1400.0, |s| s.x));
            let h = height.unwrap_or_else(|| current.map_or(900.0, |s| s.y));
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(w, h)));
            3
        } else {
            2
        };

        pending.gui_screenshot = Some(crate::mcp_bridge::PendingGuiScreenshot {
            path: path.to_owned(),
            frames_before_capture,
            capture_requested: false,
            response_tx,
        });
        // Keep frames pumping while the app is headless-idle so the
        // capture actually happens.
        ctx.request_repaint();
    }

    /// Per-frame pump for an in-flight `screenshot_gui` capture. Issues
    /// the `ViewportCommand::Screenshot` once the resize-settle countdown
    /// drains, and keeps requesting repaints so frames pump while idle.
    pub(crate) fn pump_mcp_gui_screenshot(&mut self, ctx: &egui::Context) {
        let Some(pending) = self
            .controller
            .pending_mcp
            .as_mut()
            .and_then(|p| p.gui_screenshot.as_mut())
        else {
            return;
        };
        if pending.should_capture_now() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        }
        ctx.request_repaint();
    }

    /// Complete a pending `screenshot_gui` request with a captured frame.
    /// Returns `true` when an MCP capture consumed the screenshot event
    /// (suppressing the default F12 save-to-cwd path).
    pub(crate) fn complete_mcp_gui_screenshot(&mut self, image: &egui::ColorImage) -> bool {
        let Some(pending) = self.controller.pending_mcp.as_mut() else {
            return false;
        };
        // Only consume the event once this slot has actually issued its
        // Screenshot command (an F12 capture could land first otherwise).
        if !pending
            .gui_screenshot
            .as_ref()
            .is_some_and(|p| p.capture_requested)
        {
            return false;
        }
        let Some(slot) = pending.gui_screenshot.take() else {
            return false;
        };

        let (w, h) = (image.size[0] as u32, image.size[1] as u32);
        let pixels: Vec<u8> = image
            .pixels
            .iter()
            .flat_map(|c| [c.r(), c.g(), c.b(), c.a()])
            .collect();
        let result = match image::save_buffer(
            Path::new(&slot.path),
            &pixels,
            w,
            h,
            image::ColorType::Rgba8,
        ) {
            Ok(()) => text(format!("GUI window exported to {} ({w}x{h})", slot.path)),
            Err(e) => text(format!("Failed to save PNG: {e}")),
        };
        let _ = slot.response_tx.send(McpResponse { result: Ok(result) });
        true
    }

    // ── UI navigation (set_ui_view) ──────────────────────────────────

    /// Apply a `set_ui_view` navigation request. Mutations route through
    /// the same `AppEvent`s the GUI's own widgets push, so workspace
    /// switches and modal opens behave identically to user clicks (they
    /// land later this same frame via `handle_events`). Returns a JSON
    /// echo of the resulting view state.
    fn mcp_set_ui_view(
        &mut self,
        workspace: Option<&str>,
        toolpath_index: Option<usize>,
        properties_tab: Option<&str>,
        select: Option<&str>,
        modal: Option<&str>,
    ) -> String {
        // 1. Workspace.
        let mut workspace_applied: Option<&'static str> = None;
        if let Some(ws) = workspace {
            let Some(target) = parse_workspace(ws) else {
                return json_str(serde_json::json!({
                    "error": format!(
                        "Unknown workspace '{ws}'. Valid: setup, toolpaths, simulation, readiness"
                    )
                }));
            };
            self.controller
                .events_mut()
                .push(AppEvent::SwitchWorkspace(target));
            workspace_applied = Some(workspace_key(target));
        }

        // 2. Toolpath selection (0-based index -> semantic id).
        if let Some(idx) = toolpath_index {
            let Some(tp_id) = self
                .controller
                .state()
                .session
                .toolpath_configs()
                .get(idx)
                .map(|tc| tc.id)
            else {
                let count = self.controller.state().session.toolpath_count();
                return json_str(serde_json::json!({
                    "error": format!(
                        "Toolpath index {idx} out of range (project has {count} toolpaths)"
                    )
                }));
            };
            self.controller.state_mut().selection = Selection::Toolpath(tp_id);
        }

        // 3. Properties tab — one-shot override consumed by the
        //    properties panel the next time it renders a selected
        //    toolpath (so it only shows once a toolpath is selected).
        let mut tab_applied: Option<&str> = None;
        if let Some(tab) = properties_tab {
            const VALID_TABS: &[&str] = &[
                "geometry",
                "feeds",
                "feeds_speeds",
                "linking",
                "heights",
                "dressup",
            ];
            if !VALID_TABS.contains(&tab) {
                return json_str(serde_json::json!({
                    "error": format!(
                        "Unknown properties_tab '{tab}'. Valid: geometry, feeds, linking, heights, dressup"
                    )
                }));
            }
            // Scope the override to its target toolpath (the one selected
            // above, or the pre-existing selection) so an intervening
            // render of another toolpath can't consume it.
            let Selection::Toolpath(target_id) = self.controller.state().selection else {
                return json_str(serde_json::json!({
                    "error": "properties_tab needs a toolpath — pass toolpath_index in this \
                              call or select a toolpath first"
                }));
            };
            self.controller.state_mut().gui.pending_toolpath_tab =
                Some((target_id, tab.to_owned()));
            tab_applied = Some(tab);
        }

        // 3b. Non-toolpath properties selection (machine / stock). These
        //     panels render in the Setup workspace's properties pane, so
        //     switch there if the caller didn't pick a workspace.
        let mut select_applied: Option<&str> = None;
        if let Some(sel) = select {
            let target = match sel {
                "machine" => Selection::Machine,
                "stock" => Selection::Stock,
                other => {
                    return json_str(serde_json::json!({
                        "error": format!("Unknown select '{other}'. Valid: machine, stock")
                    }));
                }
            };
            if workspace.is_none() {
                self.controller
                    .events_mut()
                    .push(AppEvent::SwitchWorkspace(Workspace::Setup));
                workspace_applied = Some(workspace_key(Workspace::Setup));
            }
            self.controller.state_mut().selection = target;
            select_applied = Some(sel);
        }

        // 4. Modal.
        if let Some(m) = modal {
            match m {
                "none" => {
                    let events = self.controller.events_mut();
                    events.push(AppEvent::CloseFeedsModal);
                    events.push(AppEvent::CloseOptimizeModal);
                    events.push(AppEvent::CloseOptimizeProject);
                    events.push(AppEvent::CloseExportWizard);
                    events.push(AppEvent::CloseToolLibrary);
                    let state = self.controller.state_mut();
                    state.show_preflight = false;
                    state.show_shortcuts = false;
                }
                "feeds_modal" | "optimize_modal" => {
                    let Selection::Toolpath(tp_id) = self.controller.state().selection else {
                        return json_str(serde_json::json!({
                            "error": format!(
                                "'{m}' needs a toolpath — pass toolpath_index in this call \
                                 or select a toolpath first"
                            )
                        }));
                    };
                    let event = if m == "feeds_modal" {
                        AppEvent::OpenFeedsModal(tp_id)
                    } else {
                        AppEvent::OpenOptimizeModal(tp_id)
                    };
                    self.controller.events_mut().push(event);
                }
                "export_wizard" => {
                    self.controller
                        .events_mut()
                        .push(AppEvent::OpenExportWizard);
                }
                "tool_library" => {
                    self.controller.events_mut().push(AppEvent::OpenToolLibrary);
                }
                other => {
                    return json_str(serde_json::json!({
                        "error": format!(
                            "Unknown modal '{other}'. Valid: feeds_modal, optimize_modal, \
                             export_wizard, tool_library, none"
                        )
                    }));
                }
            }
        }

        // Echo the resulting view. Workspace/modal mutations route
        // through the event queue and land later this same frame, so
        // echo the requested targets plus the already-applied selection.
        let state = self.controller.state();
        let selected = match state.selection {
            Selection::Toolpath(id) => state
                .session
                .toolpath_configs()
                .iter()
                .enumerate()
                .find(|(_, tc)| tc.id == id)
                .map(|(index, tc)| {
                    serde_json::json!({
                        "index": index,
                        "id": id,
                        "name": tc.name,
                    })
                }),
            _ => None,
        };
        let selection_kind = match state.selection {
            Selection::Machine => "machine",
            Selection::Stock => "stock",
            Selection::Toolpath(_) => "toolpath",
            Selection::Tool(_) => "tool",
            _ => "other",
        };
        json_str(serde_json::json!({
            "ok": true,
            "workspace": workspace_applied.unwrap_or_else(|| workspace_key(state.workspace)),
            "selected_toolpath": selected,
            "selection": selection_kind,
            "select": select_applied,
            "properties_tab": tab_applied,
            "modal": modal,
            "note": "view changes render on the next frame; call screenshot_gui to capture",
        }))
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
///
/// **C1 (2026-08-06).** This used to nest a full scan of the project's entire
/// sample vector inside a loop over every span, so a bare `get_cut_trace()`
/// cost `Σ_toolpaths (spans × total_samples)` — on the egui frame-loop
/// thread, with every other queued MCP request waiting behind it. The
/// accumulation now happens in one pass in
/// [`rs_cam_core::simulation_cut::accumulate_by_span`], which owns the
/// nesting rule (a sample belongs to EVERY span in its `span_path`) and is
/// pinned against a verbatim transcription of the old walk by
/// `crates/rs_cam_core/tests/span_summary_single_pass_c1.rs`. This function
/// keeps only the JSON shaping; no wire key changed.
fn build_span_cut_summaries(
    state: &crate::state::AppState,
    trace: &rs_cam_core::simulation_cut::SimulationCutTrace,
    toolpath_id: Option<rs_cam_core::ToolpathId>,
    span_filter_active: bool,
    accepted_by_toolpath: &std::collections::HashMap<
        rs_cam_core::ToolpathId,
        Option<std::collections::HashSet<u32>>,
    >,
) -> serde_json::Value {
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
        // C1: ONE pass over the trace for this toolpath, scattering each
        // sample into every span of its path. An always-reject filter (a
        // span filter that matched nothing for this toolpath) short-circuits
        // to an empty accept set rather than scanning at all.
        let accepted: Option<std::collections::HashSet<u32>> = if span_filter_active {
            Some(
                accepted_by_toolpath
                    .get(&tc.id)
                    .and_then(|set| set.clone())
                    .unwrap_or_default(),
            )
        } else {
            None
        };
        let accs = rs_cam_core::simulation_cut::accumulate_by_span(
            &trace.samples,
            tc.id,
            spans.len(),
            accepted.as_ref(),
        );
        for (span_index, span) in spans.iter().enumerate() {
            let span_id = span_index as u32;

            use rs_cam_core::simulation_cut::AirCutRatios;
            let Some(acc) = accs.get(span_index) else {
                continue;
            };
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
                // LH-1: span-scope air cut, both denominators named.
                "air_cut_pct_of_total_runtime": acc.air_cut_pct_of_total_runtime(),
                "air_cut_pct_of_cutting_time": acc.air_cut_pct_of_cutting_time(),
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
                "per_kinematics": render_per_kinematics_json(acc),
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
pub(crate) fn viz_project_evidence(
    state: &crate::state::AppState,
) -> rs_cam_core::session::ProjectEvidence<'_> {
    state.simulation.project_evidence()
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
///
/// Wave D3: the structural half of this used to be a list of string
/// literals with a catch-all fallback, so a NEW `SpanKind` variant silently
/// fell through to "treat as a literal debug-trace kind" and matched
/// nothing, with no compile error and no runtime complaint. It now parses
/// the input into the enum first and matches that EXHAUSTIVELY — adding a
/// variant to `SpanKind` breaks this build until someone says what it
/// expands to. Only genuinely unparseable input (a real debug-trace kind)
/// takes the literal path.
fn expand_span_kind_synonyms(span_kind: &str) -> Vec<String> {
    use rs_cam_core::toolpath_spans::SpanKind;
    let Ok(kind) = parse_span_kind_filter(span_kind) else {
        // Not a structural kind at all — a literal debug-trace kind.
        return vec![span_kind.to_owned()];
    };
    match kind {
        // Structural SpanKind synonyms expand to the matching debug kinds.
        SpanKind::DepthPass => vec![
            "z_level_clear".to_owned(),
            "adaptive_pass".to_owned(),
            "z_level".to_owned(),
        ],
        SpanKind::Entry => vec!["entry_search".to_owned()],
        // These structural kinds have no debug-trace generators yet — an
        // empty set so the filter matches nothing rather than falsely
        // matching by string.
        SpanKind::Operation
        | SpanKind::Region
        | SpanKind::LeadOut
        | SpanKind::LinkBridge
        | SpanKind::DressupArtifact
        | SpanKind::GeometryRefit
        | SpanKind::WaterlineCleanup
        | SpanKind::RapidOrderBarrier => Vec::new(),
    }
}

/// Parse the agent-facing workspace key used by the MCP `set_ui_view`
/// tool into the GUI [`Workspace`] enum.
fn parse_workspace(s: &str) -> Option<Workspace> {
    match s {
        "setup" => Some(Workspace::Setup),
        "toolpaths" => Some(Workspace::Toolpaths),
        "simulation" | "sim" => Some(Workspace::Simulation),
        "readiness" => Some(Workspace::Readiness),
        _ => None,
    }
}

/// Inverse of [`parse_workspace`] for the `set_ui_view` JSON echo.
fn workspace_key(w: Workspace) -> &'static str {
    match w {
        Workspace::Setup => "setup",
        Workspace::Toolpaths => "toolpaths",
        Workspace::Simulation => "simulation",
        Workspace::Readiness => "readiness",
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
///
/// Wave D3: the string table lives in core
/// ([`rs_cam_core::toolpath_spans::SpanKind::as_key`], exhaustive) rather
/// than being transcribed here, so a new variant cannot be silently absent
/// from the agent vocabulary.
fn parse_span_kind_filter(s: &str) -> Result<rs_cam_core::toolpath_spans::SpanKind, String> {
    use rs_cam_core::toolpath_spans::SpanKind;
    SpanKind::from_key(s).ok_or_else(|| {
        let known: Vec<&str> = SpanKind::ALL.iter().map(|k| k.as_key()).collect();
        format!(
            "unknown span_kind {s:?} — known kinds: {}",
            known.join(", ")
        )
    })
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
        SpanKind::GeometryRefit => "GeometryRefit",
        SpanKind::WaterlineCleanup => "WaterlineCleanup",
        SpanKind::RapidOrderBarrier => "RapidOrderBarrier",
        // Transport-only carrier (task #14) — stripped before a toolpath
        // is stored, so this name only ever surfaces if one leaked.
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
        // Wave D3: a `region` span is either a planner territory NODE or one
        // GENERATOR pass, and their `region_id`s index different tables.
        // Published as its own key so an agent never has to parse the label
        // (or the Debug-formatted payload) to tell them apart. `null` on
        // every non-region span.
        "region_role": s.region_role().map(|role| role.label()),
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
    toolpath_id: rs_cam_core::ToolpathId,
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
                    Some(SpanPayload::Region { region_id: rid, .. }) if *rid == want_rid => {}
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
    use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
    use rs_cam_core::toolpath_spans::{RegionSpanRole, Span, SpanKind, SpanPayload};

    // ── B7 divergence 3: the wrong-tool fallback ────────────────────────
    //
    // Two tools with visibly different geometry, and a toolpath pointing at
    // a THIRD id that does not exist. The parent revision's lookup is
    // transcribed verbatim so the defect stays executable: checking out the
    // parent was not available to this wave (the working tree is shared with
    // another live lane), and a transcription keeps failing if anyone
    // reintroduces the fallback, which a one-off checkout would not.

    fn two_tool_fixture() -> Vec<ToolConfig> {
        let mut a = ToolConfig::new_default(ToolId(1), ToolType::EndMill);
        a.name = "Ø6 flat".to_owned();
        a.diameter = 6.0;
        a.flute_count = 2;
        let mut b = ToolConfig::new_default(ToolId(2), ToolType::BallNose);
        b.name = "Ø1 ball".to_owned();
        b.diameter = 1.0;
        b.flute_count = 4;
        vec![a, b]
    }

    /// The parent revision's lookup, transcribed from
    /// `mcp_narrate_toolpath` at parent `88ce23a`.
    fn parent_revision_tool_lookup(tools: &[ToolConfig], tool_id: usize) -> Option<&ToolConfig> {
        tools
            .iter()
            .find(|tool| tool.id.0 == tool_id)
            .or_else(|| tools.first())
    }

    /// **The exhibit.** On an unresolvable tool id the parent silently
    /// returned the FIRST tool — a Ø6 2-flute end mill standing in for a Ø1
    /// 4-flute ball nose. That diameter sets narration's large-arc threshold
    /// and every tool-scaled hint; the flute count sits under every chipload
    /// sentence; and the same `ToolConfig` builds the cutter handed to
    /// `narrate_toolpath_with_context`. Nothing in the emitted text said the
    /// numbers were about another tool.
    #[test]
    fn narration_tool_lookup_refuses_where_the_parent_took_another_tool() {
        let tools = two_tool_fixture();
        const MISSING_ID: usize = 99;

        let parent = parent_revision_tool_lookup(&tools, MISSING_ID)
            .expect("the parent revision always found *a* tool — that is the defect");
        assert_eq!(
            parent.name, "Ø6 flat",
            "the transcribed parent must reproduce the fallback, or this exhibit proves \
             nothing",
        );
        assert!(
            (parent.diameter - 1.0).abs() > 4.0 && parent.flute_count != 4,
            "the fixture must make the substitution VISIBLE — a fallback to a tool with the \
             same geometry would be harmless and would not demonstrate the defect",
        );

        assert!(
            crate::app::RsCamApp::narration_tool_for(&tools, MISSING_ID).is_none(),
            "the shipped lookup must refuse an unresolvable tool id rather than narrate \
             with another tool's geometry",
        );
    }

    /// Non-vacuity: the refusal is not a blanket one. A resolvable id still
    /// returns its OWN tool, and not the first one.
    #[test]
    fn narration_tool_lookup_still_finds_the_toolpaths_own_tool() {
        let tools = two_tool_fixture();
        let found =
            crate::app::RsCamApp::narration_tool_for(&tools, 2).expect("tool id 2 is configured");
        assert_eq!(found.name, "Ø1 ball");
        assert!((found.diameter - 1.0).abs() < 1e-9);
    }

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
            Span::new(0, 7, SpanKind::Region).with_payload(SpanPayload::Region {
                region_id: 0,
                role: RegionSpanRole::GeneratorPass,
            }),
            Span::new(7, 15, SpanKind::Region).with_payload(SpanPayload::Region {
                region_id: 1,
                role: RegionSpanRole::GeneratorPass,
            }),
            Span::new(15, 30, SpanKind::DepthPass).with_payload(SpanPayload::DepthPass {
                z_level: -4.0,
                pass_index: 1,
            }),
            Span::new(15, 22, SpanKind::Region).with_payload(SpanPayload::Region {
                region_id: 2,
                role: RegionSpanRole::GeneratorPass,
            }),
            Span::new(22, 30, SpanKind::Region).with_payload(SpanPayload::Region {
                region_id: 3,
                role: RegionSpanRole::GeneratorPass,
            }),
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
            rs_cam_core::ToolpathId(42),
            0,
            "tp",
            "pocket",
            30,
            spans,
            true,
            kind,
            parent_id,
            pass_index,
            region_id,
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
                    role: RegionSpanRole::GeneratorPass,
                }),
            );
        }
        let v = build_inspect_spans_response(
            rs_cam_core::ToolpathId(1),
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
            rs_cam_core::ToolpathId(1),
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
            rs_cam_core::ToolpathId(1),
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

    /// `set_ui_view` documents these workspace keys — every key must
    /// parse, every `Workspace` variant must round-trip through
    /// `workspace_key` → `parse_workspace`, and unknown keys stay `None`.
    #[test]
    fn workspace_keys_round_trip() {
        for ws in [
            Workspace::Setup,
            Workspace::Toolpaths,
            Workspace::Simulation,
            Workspace::Readiness,
        ] {
            assert_eq!(parse_workspace(workspace_key(ws)), Some(ws));
        }
        // "sim" alias accepted on input (matches the RS_CAM_SCREENSHOT
        // env-var vocabulary).
        assert_eq!(parse_workspace("sim"), Some(Workspace::Simulation));
        assert_eq!(parse_workspace("not_a_workspace"), None);
    }
}
