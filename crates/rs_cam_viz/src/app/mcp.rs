//! GUI-side processing of MCP requests. Called from `RsCamApp::update()`.

#![deny(clippy::indexing_slicing)]

pub(crate) mod commands;
mod diagnostics;
mod generation;
mod simulation;

use std::path::Path;

use rs_cam_core::compute::config::ComputeStatus;
use rs_cam_core::session::{GetOperationSchemaArgs, Query, QueryAnswer};

use crate::controller::Severity;
use crate::mcp_bridge::{
    GuiBanner, McpOutcome, McpRequest, McpRequestKind, McpResponse, MutationResult,
    MutationWarning, ProgressUpdate, mutation_error_json,
};
use crate::state::Workspace;
use crate::state::selection::Selection;
use crate::ui::AppEvent;
use crate::ui_command::{
    GetCutTraceArgs, GetNotificationsArgs, NoArgs, Reach, ScreenshotGuiArgs,
    ScreenshotSimulationArgs, ScreenshotToolpathArgs, SetUiViewArgs, SimJumpToMoveArgs,
    SimJumpToToolpathEndArgs, SimJumpToToolpathStartArgs, SimScrubToolpathArgs, UiCommand, UiQuery,
    UiQueryAnswer,
};

use commands::CorePlan;
pub(crate) use diagnostics::viz_project_evidence;
use simulation::sim_mesh_in_world_frame;

use rs_cam_mcp::server::{json_str, no_project_error, text};

/// The optional dials `screenshot_toolpath` accepts, grouped for the same
/// reason [`RestAnalysisDials`] is: six positional arguments was already the
/// readable limit, and P5's reach flag made seven `Option`s in a row. A
/// caller that transposed two of them would compile and capture the wrong
/// picture.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ScreenshotToolpathOptions {
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub show_stock: Option<bool>,
    pub include_rapids: Option<bool>,
    /// P5 — shade the model surface by the per-tool reach map. Mutually
    /// exclusive with `show_stock`; this one wins.
    pub reach_overlay: Option<bool>,
}

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

/// The dial half of a multi-tool planner request — the fields
/// `plan_multitool_finishing` and `preview_tier_map` accept identically.
///
/// Grouped into one struct so `RsCamApp::multitool_plan_spec` is the ONE
/// place either tool's defaults are resolved. Two tools with two copies of
/// the same seven fallbacks is two things to drift, and the failure would be
/// silent: a preview and the plan that followed it would describe different
/// territory with nothing on either surface saying so.
///
/// Every field is `Option` with the same meaning throughout: `None` = "the
/// caller did not say", resolved to the CORE's default, never to zero.
pub(crate) struct MultitoolDials {
    pub cell_mm: Option<f64>,
    pub tolerance_mm: Option<f64>,
    pub margin_mm: Option<f64>,
    pub cusp_height_mm: Option<f64>,
    pub coarseness: Option<f64>,
    pub overlap_mm: Option<f64>,
    pub max_regions_per_tier: Option<usize>,
    pub coarse_skips_fine_islands: Option<bool>,
    pub monotone_cell_decomposition: Option<bool>,
    /// Per-tier strategy strings, LADDER order (coarse → fine). `None` =
    /// all unified (the historical planner).
    pub tier_strategies: Option<Vec<String>>,
}

pub(crate) fn parse_tier_strategies(
    raw: &Option<Vec<String>>,
) -> Result<Vec<rs_cam_core::session::TierStrategy>, String> {
    use rs_cam_core::session::TierStrategy;
    let Some(raw) = raw else {
        return Ok(Vec::new());
    };
    raw.iter()
        .map(|s| match s.as_str() {
            "unified_finish" | "unified" => Ok(TierStrategy::UnifiedFinish),
            "scallop" => Ok(TierStrategy::Scallop),
            "iso_scallop" | "iso" => Ok(TierStrategy::IsoScallop),
            other => Err(format!(
                "unknown tier strategy '{other}' — expected unified_finish | scallop | \
                 iso_scallop"
            )),
        })
        .collect()
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
            self.mcp_reads.frame_loop().record_handled();
        }

        self.publish_mcp_read_snapshot();
    }

    /// G-LV.1: open the frame bracket. It closes in [`Self::end_mcp_frame`]
    /// at the very bottom of `draw_frame`, so "in a frame" spans the whole
    /// frame body — dispatch, event handling and render alike. That is what
    /// lets the MCP server thread tell a loop that is *inside* something long
    /// from one that is not running at all: both look like "no recent frame"
    /// from outside, and only the first ends on its own.
    ///
    /// **B-4 moved this out of [`Self::drain_mcp_requests`].** The drain is
    /// now also reachable from the off-frame pump, and a pump that opened the
    /// bracket without a paint closing it would leave `in_frame` latched
    /// `true` forever — which makes `is_parked()` permanently `false` and
    /// turns the one honest report of a non-rendering window into a
    /// guaranteed lie. The bracket belongs to the paint, so it is opened and
    /// closed on the paint path and nowhere else.
    pub(crate) fn begin_mcp_frame(&mut self) {
        if self.mcp_receiver.is_none() {
            return;
        }
        self.mcp_reads.frame_loop().frame_begin();
    }

    /// G-LV.1: close the frame bracket opened in [`Self::begin_mcp_frame`]
    /// and publish what the frame left undone. Called last in `update`.
    ///
    /// The outstanding count has to come from `PendingMcpCompute`, not from
    /// the request channel: a `generate_all` is dispatched once and then
    /// spends its whole life waiting on *future* frames it never queued a
    /// request for, so a channel counter reads zero for exactly the call
    /// whose stall started this.
    pub(crate) fn end_mcp_frame(&mut self) {
        if self.mcp_receiver.is_none() {
            return;
        }
        let awaiting = self.controller.awaiting_deferred_completions();
        let awaiting_generate_all = self.controller.awaiting_generate_all();
        self.mcp_reads
            .frame_loop()
            .beat(awaiting, awaiting_generate_all);
    }

    /// The off-frame equivalent of [`Self::end_mcp_frame`]: refresh the
    /// outstanding-work counters after a dispatch that ran without a paint.
    pub(crate) fn mcp_pump_beat(&mut self) {
        if self.mcp_receiver.is_none() {
            return;
        }
        let awaiting = self.controller.awaiting_deferred_completions();
        let awaiting_generate_all = self.controller.awaiting_generate_all();
        self.mcp_reads
            .frame_loop()
            .pump_beat(awaiting, awaiting_generate_all);
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
            // WP14a: a `Job` submit, not a synchronous read. The arm starts
            // the job on the frame loop and stores the oneshot; the drain
            // answers it. The wire is unchanged — the CLIENT still waits as
            // long as it did, and the FRAME LOOP no longer does.
            McpRequestKind::RecommendClearingStrategy { index } => {
                self.mcp_recommend_clearing_strategy(index, response_tx);
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
            //
            // WP4: ONE arm for every registry `Command` row the wire
            // reaches. `core_command_for` converts the wire parameters
            // into the row's core payload and validates what only this
            // surface can validate; `ProjectSession::apply` is the one
            // mutation door; `describe_core` builds the reply, the toast
            // and the view writes that belong after the mutation.
            //
            // G-MCPTOAST (UX-R03-003): the toast is pushed AFTER the
            // mutation, from its outcome, through `push_mcp_outcome`. A
            // refused request shows the refusal at Warning; the success
            // text is unchanged and is only pushed once it is true. The
            // three asynchronous compute arms ("Generating…", "Running
            // simulation…") keep their progress toasts — the lane
            // reports the outcome later.
            McpRequestKind::Core(request) => {
                // The toast text is read off the REQUEST, before the
                // conversion, so a request the conversion refuses still
                // reports that refusal on screen. It is PUSHED once,
                // below, after the mutation.
                let toast = self.core_toast_for(&request);
                self.mcp_before_core(&request);
                // WP17: a row whose command needs one mutation BEFORE it
                // asks for a pair. The preparation runs here, not in the
                // conversion, which reads.
                let plan = self.core_command_for(request);
                let (resp, stated) = match self.run_core_preparation(plan) {
                    CorePlan::Answered(reply) => (reply, None),
                    // `run_core_preparation` reduces every pair to its
                    // own command, so the second pattern binds nothing
                    // the first does not. It names the variant rather
                    // than a wildcard, so a fourth plan still has to
                    // compile here.
                    CorePlan::Apply(command, before) | CorePlan::ApplyPair(_, command, before) => {
                        let id = command.id();
                        let outcome = self.controller.state_mut().session.apply(command);
                        let described = self.describe_core(id, outcome, &before);
                        (described.reply, described.outcome)
                    }
                };
                if let Some(toast) = toast {
                    let outcome = stated.unwrap_or_else(|| McpOutcome::from_json_response(&resp));
                    self.controller.push_mcp_outcome(toast, &outcome);
                }
                let _ = response_tx.send(McpResponse { result: Ok(resp) });
            }
            McpRequestKind::LoadProject {
                path,
                discard_unsaved,
            } => {
                let name = Path::new(&path)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or(&path)
                    .to_owned();
                let (resp, outcome) = self.mcp_load_project(&path, discard_unsaved);
                self.controller
                    .push_mcp_outcome(format!("MCP: Loaded '{name}'"), &outcome);
                let _ = response_tx.send(resp);
            }
            McpRequestKind::ExportGcode {
                path,
                accept_unmodeled_tool_load,
                accept_exceeded_tool_load,
                tool_change_mode,
                split_setups,
                accept_previous_geometry,
            } => {
                let resp = self.mcp_export_gcode(
                    &path,
                    accept_unmodeled_tool_load,
                    accept_exceeded_tool_load,
                    tool_change_mode.as_deref(),
                    split_setups,
                    accept_previous_geometry,
                );
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
            // WP14b: a `Job` submit, not a synchronous run. The arm
            // starts the job on the frame loop and stores the oneshot;
            // the drain answers it. The wire is unchanged — the CLIENT
            // still waits as long as it did, and the FRAME LOOP no
            // longer does.
            McpRequestKind::OptimizeToolpath { index } => {
                self.mcp_optimize_toolpath(index, response_tx);
            }
            McpRequestKind::AddToolpathViaGui {
                operation_type,
                setup_index,
            } => {
                // NO toast is pushed here, deliberately. Every other
                // synchronous arm pushes one from its reply (F1.1,
                // G-MCPTOAST) because its handler pushes none. This
                // handler is the GUI's own add path, which already pushes
                // exactly the toast F1.1 asks for — the refusal text at
                // Warning — and a second one would break the very rule
                // F1.1 established: one toast per request. The reply
                // reports the toasts the GUI path pushed instead.
                self.controller
                    .events_mut()
                    .push(AppEvent::Ui(UiCommand::SwitchWorkspace(
                        Workspace::Toolpaths,
                    )));
                let resp = self.mcp_add_toolpath_via_gui(&operation_type, setup_index);
                let _ = response_tx.send(McpResponse { result: Ok(resp) });
            }
            McpRequestKind::PlanMultitoolFinishing { spec } => {
                let resp = self.mcp_plan_multitool_finishing(&spec);
                let _ = response_tx.send(McpResponse { result: Ok(resp) });
            }
            // WP14a: a `Job` submit. See the RecommendClearingStrategy arm.
            McpRequestKind::PreviewTierMap { spec } => {
                self.mcp_preview_tier_map(&spec, response_tx);
            }
            McpRequestKind::ApplyFeeds { index, scope } => {
                let resp = self.mcp_apply_feeds(index, &scope);
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
                    .push(AppEvent::Ui(UiCommand::SwitchWorkspace(
                        Workspace::Toolpaths,
                    )));
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
                    .push(AppEvent::Ui(UiCommand::SwitchWorkspace(
                        Workspace::Simulation,
                    )));
                self.mcp_send_progress(&progress_tx, "Starting simulation...", 0.0, Some(1.0));
                self.mcp_run_simulation(resolution, response_tx);
            }
            McpRequestKind::CollisionCheck { index } => {
                self.mcp_send_progress(&progress_tx, "Running collision check...", 0.0, Some(1.0));
                self.mcp_collision_check(index, response_tx);
            }

            // ── Measurements over the model ─────────────────────────
            McpRequestKind::ReachMap { spec } => {
                let resp = self.mcp_reach_map(&spec);
                let _ = response_tx.send(McpResponse { result: Ok(resp) });
            }

            // ── View reads (WP13) ───────────────────────────────────
            //
            // One arm for all eight rows. `ui_query` takes `&self`, so a
            // read cannot write the session.
            McpRequestKind::UiQuery(query) => {
                let resp = self.ui_query(query).into_json();
                let _ = response_tx.send(McpResponse { result: Ok(resp) });
            }

            // ── View commands (WP13) ────────────────────────────────
            //
            // Scrubbing, screenshots and UI navigation. Each arm keeps the
            // body it had as a variant of its own.
            McpRequestKind::Ui(command) => match command {
                UiCommand::SimJumpToMove(SimJumpToMoveArgs { move_index }) => {
                    self.mcp_show_simulation_workspace();
                    let resp = self.mcp_sim_jump_to_move(move_index);
                    let _ = response_tx.send(McpResponse { result: Ok(resp) });
                }
                UiCommand::SimJumpToStart(NoArgs) => {
                    self.mcp_show_simulation_workspace();
                    let resp = self.mcp_sim_jump_to_start();
                    let _ = response_tx.send(McpResponse { result: Ok(resp) });
                }
                UiCommand::SimJumpToEnd(NoArgs) => {
                    self.mcp_show_simulation_workspace();
                    let resp = self.mcp_sim_jump_to_end();
                    let _ = response_tx.send(McpResponse { result: Ok(resp) });
                }
                UiCommand::SimScrubToolpath(SimScrubToolpathArgs { index, percent }) => {
                    self.mcp_show_simulation_workspace();
                    let resp = self.mcp_sim_scrub_toolpath(index, percent);
                    let _ = response_tx.send(McpResponse { result: resp });
                }
                UiCommand::SimJumpToToolpathStart(SimJumpToToolpathStartArgs { index }) => {
                    self.mcp_show_simulation_workspace();
                    let resp = self.mcp_sim_scrub_toolpath(index, 0.0);
                    let _ = response_tx.send(McpResponse { result: resp });
                }
                UiCommand::SimJumpToToolpathEnd(SimJumpToToolpathEndArgs { index }) => {
                    self.mcp_show_simulation_workspace();
                    let resp = self.mcp_sim_scrub_toolpath(index, 100.0);
                    let _ = response_tx.send(McpResponse { result: resp });
                }
                UiCommand::ScreenshotSimulation(ScreenshotSimulationArgs {
                    path,
                    width,
                    height,
                    checkpoint,
                    include_toolpaths,
                }) => {
                    let resp = self.mcp_screenshot_simulation(
                        &path,
                        width,
                        height,
                        checkpoint,
                        include_toolpaths,
                    );
                    let _ = response_tx.send(McpResponse { result: Ok(resp) });
                }
                UiCommand::ScreenshotToolpath(ScreenshotToolpathArgs {
                    index,
                    path,
                    width,
                    height,
                    show_stock,
                    include_rapids,
                    reach_overlay,
                }) => {
                    let resp = self.mcp_screenshot_toolpath(
                        index,
                        &path,
                        ScreenshotToolpathOptions {
                            width,
                            height,
                            show_stock,
                            include_rapids,
                            reach_overlay,
                        },
                    );
                    let _ = response_tx.send(McpResponse { result: Ok(resp) });
                }
                UiCommand::ScreenshotGui(ScreenshotGuiArgs {
                    path,
                    width,
                    height,
                }) => {
                    // Deferred response: the handler stores response_tx in
                    // the pending slot and the capture completes 1-2 frames
                    // later.
                    self.mcp_screenshot_gui(ctx, &path, width, height, response_tx);
                }
                UiCommand::SetUiView(SetUiViewArgs {
                    workspace,
                    toolpath_index,
                    properties_tab,
                    select,
                    modal,
                    overlays,
                }) => {
                    let resp = self.mcp_set_ui_view(
                        workspace.as_deref(),
                        toolpath_index,
                        properties_tab.as_deref(),
                        select.as_deref(),
                        modal.as_deref(),
                        overlays.as_ref(),
                    );
                    let _ = response_tx.send(McpResponse { result: Ok(resp) });
                }
                // The view registry holds 62 view commands. The other 52
                // carry no MCP tool, and the row says which surface does
                // reach each one.
                other => {
                    let reason = match other.id().surfaces().mcp {
                        Reach::Reached => "the row declares a tool, and this dispatch has no arm",
                        Reach::Skip(reason) => reason,
                    };
                    let resp = mutation_error_json(
                        &format!(
                            "the {} view command is not on the wire: {reason}",
                            other.id().wire_name(),
                        ),
                        None,
                    );
                    let _ = response_tx.send(McpResponse { result: Ok(resp) });
                }
            },
        }
    }

    /// Move the view to the simulation workspace.
    ///
    /// Six scrubbing rows did this inline, each pushing the same event.
    /// The push is the whole body, so one name carries it.
    fn mcp_show_simulation_workspace(&mut self) {
        self.controller
            .events_mut()
            .push(AppEvent::Ui(UiCommand::SwitchWorkspace(
                Workspace::Simulation,
            )));
    }

    /// Run one view read, and report its answer (WP13 ruling 4).
    ///
    /// This is the read door of the view registry, beside
    /// [`ProjectSession::apply`](rs_cam_core::session::ProjectSession::apply)
    /// and
    /// [`ProjectSession::query`](rs_cam_core::session::ProjectSession::query)
    /// for the core registry. It takes `&self`, so a view read cannot
    /// write the session and cannot write the view.
    ///
    /// **The receiver is the application, not `AppState`.** Two of the
    /// eight rows read state `AppState` does not hold: `get_notifications`
    /// reads the toast stack and `get_diagnostics` reads the controller's
    /// own triage builder. Both hang on the controller, one level above
    /// `AppState`, so a door on `AppState` could answer neither.
    fn ui_query(&self, query: UiQuery) -> UiQueryAnswer {
        match query {
            UiQuery::ListToolLibrary(NoArgs) => {
                UiQueryAnswer::ListToolLibrary(self.mcp_list_tool_library())
            }
            UiQuery::ListToolCatalog(catalog) => {
                UiQueryAnswer::ListToolCatalog(self.mcp_list_tool_catalog(&catalog))
            }
            UiQuery::ListMachineLibrary(NoArgs) => {
                UiQueryAnswer::ListMachineLibrary(self.mcp_list_machine_library())
            }
            UiQuery::GetDiagnostics(NoArgs) => {
                UiQueryAnswer::GetDiagnostics(self.mcp_get_diagnostics())
            }
            UiQuery::GetToolLoadReport(NoArgs) => {
                UiQueryAnswer::GetToolLoadReport(self.mcp_get_tool_load_report())
            }
            UiQuery::InspectCollisions(NoArgs) => {
                UiQueryAnswer::InspectCollisions(self.mcp_inspect_collisions())
            }
            UiQuery::GetNotifications(GetNotificationsArgs {
                include_expired,
                limit,
            }) => {
                UiQueryAnswer::GetNotifications(self.mcp_get_notifications(include_expired, limit))
            }
            UiQuery::GetCutTrace(GetCutTraceArgs {
                toolpath_id,
                max_hotspots,
                max_issues,
                span_kind,
                span_id,
                pass_index,
                include_drill_samples,
                caps,
            }) => UiQueryAnswer::GetCutTrace(self.mcp_get_cut_trace(
                toolpath_id,
                max_hotspots,
                max_issues,
                span_kind.as_deref(),
                span_id,
                pass_index,
                include_drill_samples,
                caps,
            )),
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
        use rs_cam_core::io::tool_library;
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
        use rs_cam_core::io::tool_library;
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

    /// Report the parameter schema of one operation kind.
    ///
    /// WP13 ruling 3: this is a core `Query` row, not a view read. The
    /// answer comes from the operation CATALOG, so the read never touches
    /// the view — and it takes the core read door like any other query.
    fn mcp_get_operation_schema(&self, operation_type: &str) -> String {
        let query = Query::GetOperationSchema(GetOperationSchemaArgs {
            operation_type: operation_type.to_owned(),
        });
        match self.controller.state().session.query(query) {
            Ok(QueryAnswer::GetOperationSchema(answer)) => {
                json_str(serde_json::to_value(answer.schema).unwrap_or_default())
            }
            Ok(other) => json_str(serde_json::json!({
                "error": format!("the get_operation_schema read answered {other:?}"),
            })),
            Err(e) => json_str(serde_json::json!({ "error": e.to_string() })),
        }
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

        // Acceleration-aware kinematics ($11 + per-axis $120-122 + per-axis
        // rates $110-112). `None` means the cycle-time model falls back to
        // the naive distance/feed sum (the F-034 feature flag).
        let kinematics = match &machine.kinematics {
            Some(k) => {
                let per_axis = match k.acceleration_xyz_mm_s2 {
                    Some([ax, ay, az]) => serde_json::json!([ax, ay, az]),
                    None => serde_json::Value::Null,
                };
                let per_axis_rate = match k.max_rate_xyz_mm_min {
                    Some([rx, ry, rz]) => serde_json::json!([rx, ry, rz]),
                    None => serde_json::Value::Null,
                };
                serde_json::json!({
                    "configured": true,
                    "acceleration_mm_s2": k.acceleration_mm_s2,
                    "acceleration_xyz_mm_s2": per_axis,
                    "max_rate_xyz_mm_min": per_axis_rate,
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

    /// List the per-user machine library with a compact spec summary per
    /// entry (snapshot model — these are import sources, not live links).
    fn mcp_list_machine_library(&self) -> String {
        let names = rs_cam_core::io::machine_library::list();
        let machines: Vec<serde_json::Value> = names
            .iter()
            .map(|name| match rs_cam_core::io::machine_library::load(name) {
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
                    rs_cam_core::geometry::enriched_mesh::SurfaceParams::Plane {
                        normal, ..
                    } => {
                        map.insert(
                            "normal".into(),
                            serde_json::json!([normal.x, normal.y, normal.z]),
                        );
                        map.insert(
                            "is_horizontal".into(),
                            serde_json::json!(normal.z.abs() > 0.95),
                        );
                    }
                    rs_cam_core::geometry::enriched_mesh::SurfaceParams::Cylinder {
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
                    rs_cam_core::geometry::enriched_mesh::SurfaceParams::Cone {
                        half_angle,
                        axis,
                        ..
                    } => {
                        map.insert(
                            "half_angle_deg".into(),
                            serde_json::json!(half_angle.to_degrees()),
                        );
                        map.insert("axis".into(), serde_json::json!([axis.x, axis.y, axis.z]));
                    }
                    rs_cam_core::geometry::enriched_mesh::SurfaceParams::Sphere {
                        radius, ..
                    } => {
                        map.insert("radius".into(), serde_json::json!(radius));
                    }
                    rs_cam_core::geometry::enriched_mesh::SurfaceParams::Torus {
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

    /// The refusal reply. The shape lives in
    /// [`crate::mcp_bridge::mutation_error_json`] so the toast classifier
    /// (`McpOutcome::from_json_response`) reads the same document.
    fn mcp_mutation_error(&self, summary: impl Into<String>, field: Option<&str>) -> String {
        mutation_error_json(&summary.into(), field)
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

    /// Load a project. The reply is plain text, so the toast outcome rides
    /// beside it as the controller's own `Result` (G-MCPTOAST).
    fn mcp_load_project(&mut self, path: &str, discard_unsaved: bool) -> (McpResponse, McpOutcome) {
        // G-OPENGUARD (F1.12): a load replaces the open project. The GUI
        // asks a human (Save / Discard / Cancel); an agent has nobody to
        // ask, so the equivalent is a refusal that names what would go and
        // an explicit flag to override it. Checked BEFORE the load, so a
        // refusal leaves the project, the camera and the file untouched.
        if !discard_unsaved && let Some(refusal) = self.unsaved_project_refusal() {
            return (
                McpResponse {
                    result: Ok(text(refusal.clone())),
                },
                McpOutcome::Refused(refusal),
            );
        }
        let loaded = self.controller.open_job_from_path(Path::new(path));
        let outcome = McpOutcome::from_result(&loaded);
        let resp = match loaded {
            Ok(()) => {
                // G-WSMENU (2026-09-10): the same fit the File > Open route
                // does. An agent's very next call is usually
                // `screenshot_gui`, and before this the viewport still held
                // the previous project's framing.
                self.fit_camera_to_first_model();
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
        };
        (resp, outcome)
    }

    /// The refusal text when the open project has unsaved changes, or
    /// `None` when a load may proceed (G-OPENGUARD).
    ///
    /// The sentence naming what would be lost is
    /// [`crate::state::unsaved_project_summary`], shared with anything
    /// else that has to describe the same situation; this adds only the
    /// two ways out.
    fn unsaved_project_refusal(&self) -> Option<String> {
        crate::state::unsaved_project_summary(self.controller.state()).map(|summary| {
            format!(
                "Refused: {summary}. Loading would replace it and throw those \
                 changes away. Save it first (save_project), or pass \
                 discard_unsaved: true."
            )
        })
    }

    fn mcp_export_gcode(
        &mut self,
        path: &str,
        accept_unmodeled_tool_load: bool,
        accept_exceeded_tool_load: bool,
        tool_change_mode: Option<&str>,
        split_setups: bool,
        accept_previous_geometry: bool,
    ) -> String {
        // Apply the requested tool-change handling to the GUI wizard
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
            self.controller.state_mut().gui.wizard.tool_change_override = Some(mode);
        }
        // Route through the viz-side exporter so the gate sees the viz cut
        // trace (`state.simulation.results.cut_trace`) — the async sim
        // worker leaves its trace there, and the core-side
        // `session.export_gcode_with_policy` used to refuse with a spurious
        // SimulationRequired (UX_PAIN_POINTS_2026-05-11.md, Roadmap A).
        // Since N12 item 10 the drain adopts the same simulation into the
        // session, so both doors read ONE trace `Arc` from that adopt until
        // a session mutation clears the slot. This route stays, because it
        // is the one the GUI export button takes.
        //
        // The trace is the ONLY reason left. This comment used to also
        // claim `session.results` "the GUI/MCP path never populates" —
        // stale since the F1_RCA sync in
        // `controller/events/compute.rs::drain_compute_results`, and while
        // it stood it was the root of G-MODEXPORT: the viz exporter read
        // the worker's pre-modulation IR out of `gui.toolpath_rt` while the
        // gate graded the modulated trace. `io::export::emitted_toolpaths`
        // now reads `session.results` — the store the feed-modulation
        // post-pass writes — so bytes and verdict describe one schedule.
        //
        // G-EXPORTSKIP (2026-09-10): the same exporter refuses when an
        // ENABLED toolpath has no result ("'X' is still waiting on upstream
        // stock …" / "'X' failed to generate: …" / "'X' is not generated").
        // Every `Err` arm below returns that text verbatim, so this tool
        // and the GUI pre-flight modal name the same operation for the same
        // reason. Pre-fix the op was silently dropped from the file.
        //
        // G-STALEXPORT (2026-09-10): and when an ENABLED toolpath was
        // EDITED after it was generated. `accept_previous_geometry` waives
        // that one case — never a missing result — and is passed here
        // explicitly rather than read from `gui.stale_export`, so an
        // automation client cannot inherit a checkbox a human left ticked
        // in the pre-flight modal.
        let state = self.controller.state();
        let policy = rs_cam_core::gcode::ToolLoadExportPolicy {
            accept_unmodeled: accept_unmodeled_tool_load,
            accept_exceeded: accept_exceeded_tool_load,
        };
        let stale = if accept_previous_geometry {
            crate::state::runtime::StaleResultPolicy::AcceptPreviousGeometry
        } else {
            crate::state::runtime::StaleResultPolicy::Refuse
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
                        stale,
                    ) {
                        Ok(g) => g,
                        Err(e) => return text(format!("Export failed (setup '{name}'): {e}")),
                    };
                    // G-EXPORT-DATUM + Z-datum (2026-09-07): name the
                    // datum. X0 Y0 is the stock's min corner in EVERY file
                    // (the export re-expresses every setup in the
                    // stock-relative frame), so the operator KEEPS the XY
                    // zero across the flip. Z now follows the setup's own
                    // `datum.z_method` (see
                    // `rs_cam_core::gcode::export_datum_shift_for_toolpath`):
                    // the header states that declared datum, and for the
                    // `StockTop` default the coordinates are already zeroed
                    // to the top.
                    let setup_ref = state.session.list_setups().iter().find(|s| s.id == *id);
                    let z_datum = match setup_ref.map(|s| &s.datum.z_method) {
                        None | Some(rs_cam_core::session::ZDatum::StockTop) => {
                            "Z0 = top of stock".to_owned()
                        }
                        Some(rs_cam_core::session::ZDatum::MachineTable) => {
                            "Z0 = machine table / spoilboard -- zero Z there \
                             (NOT applied to coordinates)"
                                .to_owned()
                        }
                        Some(rs_cam_core::session::ZDatum::FixedOffset(z)) => format!(
                            "Z0 = fixed offset {z:.3}mm from machine home \
                             (declared; NOT applied to coordinates)"
                        ),
                        Some(rs_cam_core::session::ZDatum::Manual) => {
                            "Z0 = set manually per setup notes (NOT applied to coordinates)"
                                .to_owned()
                        }
                    };
                    let mut header =
                        post.render_comment(&format!("rs_cam setup {}/{total}: \"{name}\"", i + 1));
                    header.push_str(&post.render_comment(&format!(
                        "DATUM: X0 Y0 = stock min corner (SAME in every setup file); {z_datum}"
                    )));
                    if i > 0 {
                        header.push_str(&post.render_comment(
                            "FLIP PART BEFORE RUNNING -- re-zero Z to the datum above; KEEP the same X/Y zero",
                        ));
                    }
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
            stale,
        ) {
            Ok(g) => g,
            Err(e) => return text(format!("Export failed: {e}")),
        };
        match std::fs::write(Path::new(path), &gcode) {
            Ok(()) => text(format!("G-code exported to {path}")),
            Err(e) => text(format!("Export failed: {e}")),
        }
    }

    /// Put the operator's screen on the toolpath an MCP mutation is about
    /// to change: switch to the Toolpaths workspace and select the row.
    /// The same two moves the `SetToolpathParam` arm makes inline (that
    /// arm also records an `mcp_highlights` key for the param it changed,
    /// which a rebind has no equivalent of).
    fn select_toolpath_for_mcp(&mut self, index: usize) {
        let tp_id = self
            .controller
            .state()
            .session
            .toolpath_configs()
            .get(index)
            .map(|tc| tc.id);
        self.controller
            .events_mut()
            .push(AppEvent::Ui(UiCommand::SwitchWorkspace(
                Workspace::Toolpaths,
            )));
        if let Some(tp_id) = tp_id {
            self.controller.state_mut().selection = Selection::Toolpath(tp_id);
        }
    }

    /// One toast-stack entry, as F3.5 publishes it. `index` is the
    /// position in the stack (0 = oldest), so a caller can line two reads
    /// up against each other.
    #[cfg(feature = "mcp")]
    fn notification_json(index: usize, n: &crate::controller::Notification) -> serde_json::Value {
        let age = n.created_at.elapsed();
        let ttl = n.ttl();
        serde_json::json!({
            "index": index,
            "message": n.message,
            "severity": match n.severity {
                crate::controller::Severity::Info => "info",
                crate::controller::Severity::Warning => "warning",
                crate::controller::Severity::Error => "error",
            },
            "age_seconds": age.as_secs_f64(),
            "ttl_seconds": ttl.as_secs_f64(),
            "visible": !n.is_expired(),
        })
    }

    /// F3.5 — the toast stack, newest first (PLAN.md §5). Read-only.
    ///
    /// `&self`: this cannot clear the stack even by accident. A reader
    /// that consumed what it read would change what the next reader sees,
    /// and the next reader is usually the operator.
    fn mcp_get_notifications(&self, include_expired: bool, limit: Option<usize>) -> String {
        let all = self.controller.notifications();
        let mut rows: Vec<serde_json::Value> = all
            .iter()
            .enumerate()
            .filter(|(_, n)| include_expired || !n.is_expired())
            .map(|(i, n)| Self::notification_json(i, n))
            .collect();
        rows.reverse(); // newest first
        let total = rows.len();
        if let Some(limit) = limit {
            rows.truncate(limit);
        }
        json_str(serde_json::json!({
            "notifications": rows,
            "total": total,
            "returned": rows.len(),
            "visible_count": all.iter().filter(|n| !n.is_expired()).count(),
            "include_expired": include_expired,
            "note": "Read-only; nothing was removed. Ages are measured from the push and \
                     live only in this GUI process — the stack is not persisted and is empty \
                     after a restart. Expired entries are collected on the frame loop, so an \
                     expired one may already be gone.",
        }))
    }

    // ── Compute operations ───────────────────────────────────────────

    // ── Screenshot implementations ───────────────────────────────────

    /// The model surface as a render background, shaded by the reach map
    /// for `index`'s tool.
    ///
    /// The mesh is the one the map was built on — the SETUP-TRANSFORMED
    /// mesh, which is also the frame the toolpath was emitted in, so the
    /// two composite layers register.
    fn reach_overlay_background(
        &self,
        index: usize,
    ) -> Result<rs_cam_core::stock::stock_mesh::StockMesh, String> {
        let session = &self.controller.state().session;
        let Some(spec) = session.reach_map_spec(index, None) else {
            return Err(format!(
                "Toolpath {index} has no reach map. A reach map answers for a FINISHING \
                 operation on a 3D mesh (drop_cutter, waterline, pencil, scallop, \
                 unified_finish, steep_shallow, ramp_finish, spiral_finish, radial_finish, \
                 horizontal_finish) that has both a model mesh and a tool."
            ));
        };
        // Fresh flag, never armed: this call is not on the generate lane,
        // so `cancel_generation` is not its owner.
        let cancel = std::sync::atomic::AtomicBool::new(false);
        let cancel_fn = || cancel.load(std::sync::atomic::Ordering::SeqCst);
        let map = rs_cam_core::maps::reach_map_cache::cached_reach_map(&spec, &cancel_fn)
            .map_err(|e| format!("Reach map for toolpath {index} could not be built — {e}"))?;
        let gaps = map.vertex_gaps(spec.mesh.as_ref(), spec.index.as_ref());
        let floors = map.vertex_floors(spec.mesh.as_ref());
        Ok(rs_cam_core::maps::reach_map::reach_overlay_stock_mesh(
            spec.mesh.as_ref(),
            &gaps,
            &floors,
            map.ramp(),
        ))
    }

    /// P5 — the reach map as numbers.
    ///
    /// A pure READ: it builds (or fetches) the map and reports it. Nothing
    /// is emitted, no parameter moves, no result is invalidated — the reply
    /// says `modified: false` and that is a statement about this function,
    /// not a hope. It takes `&self` so that stays true by type.
    fn mcp_reach_map(&self, spec: &rs_cam_mcp::server::ReachMapParam) -> String {
        let index = spec.index;
        let session = &self.controller.state().session;
        let Some(request) = session.reach_map_spec(index, spec.tolerance_mm) else {
            return json_str(serde_json::json!({
                "ok": false,
                "modified": false,
                "error": format!(
                    "reach_map: toolpath {index} has no reach map. A reach map answers for a \
                     FINISHING operation on a 3D mesh that has both a model mesh and a tool; \
                     a roughing pass, a 2D operation and a drill have no reach question."
                ),
            }));
        };
        let cancel = std::sync::atomic::AtomicBool::new(false);
        let cancel_fn = || cancel.load(std::sync::atomic::Ordering::SeqCst);
        let map = match rs_cam_core::maps::reach_map_cache::cached_reach_map(&request, &cancel_fn) {
            Ok(map) => map,
            Err(e) => {
                return json_str(serde_json::json!({
                    "ok": false,
                    "modified": false,
                    "error": format!("reach_map: the walk could not finish — {e}"),
                }));
            }
        };
        let bins = spec.histogram_bins.unwrap_or(8).clamp(1, 64);
        let (edges, counts, not_measured) = map.gap_histogram(bins);
        json_str(serde_json::json!({
            "ok": true,
            // Plan-time read: this handler takes `&self`.
            "modified": false,
            // FIRST LINE, deliberately: the grid the answer sits on. Two
            // percentages from two calls are comparable only on one grid, and
            // an agent that reads the number before the grid cannot know
            // (F5, 2026-09-08).
            "grid_note": map.grid_note(),
            // P5.2: the base, on the wire, because a comparison against any
            // other instrument is meaningless until both sides share it.
            "area_basis": "true 3D surface area (each triangle by its own \
                           area, never its XY footprint), over the rim-eroded \
                           measured population only \u{2014} both numerator \
                           and denominator",
            "toolpath_index": index,
            "tool_id": map.tool_id,
            "model_id": map.model_id,
            "tolerance_mm": map.tolerance_mm,
            "tolerance_source": request.tolerance_source.key(),
            "tolerance_note": request.tolerance_source.describe(map.tolerance_mm),
            "is_measured": map.is_measured(),
            "unreachable_pct_of_measured_area": map.unreachable_pct(),
            "unresolved_pct_of_measured_area": map.unresolved_pct(),
            "tolerance_below_floor": map.tolerance_below_floor(),
            "max_gap_mm": map.max_gap_mm,
            "area_mm2": {
                "surface": map.surface_area_mm2,
                "measured": map.measured_area_mm2,
                "unreachable": map.unreachable_area_mm2,
                "unresolved": map.unresolved_area_mm2,
            },
            "grid": {
                "cell_mm": map.cell_mm,
                "nx": map.nx,
                "ny": map.ny,
                "cells": map.cells.len(),
                "not_measured_cells": not_measured,
                "cell_rule": "the TOOL's tip sphere and the model bbox, never the tolerance \
                              \u{2014} so a series of probes at moving tolerances is comparable",
            },
            "gap_histogram": {
                "upper_edges_mm": edges,
                "counts": counts,
            },
            "discretisation_floor_mm": map.discretisation_floor_mm,
            "profile_floor_mm": map.profile_floor_mm,
            "curvature_floor_p95_mm": map.curvature_floor_p95_mm,
            "rim_erosion_mm": map.rim_erosion_mm,
            "note": "Percentages are 3D-SURFACE-AREA weighted over the rim-eroded \
                     measured population (see `area_basis`); putting a planar or \
                     whole-board figure beside one of these compares two different \
                     questions \u{2014} on a 1.34 mean-sec-theta terrain that alone is \
                     5 points. Top-down measure. Undersides, walls and a band one envelope radius wide \
                     inside the part outline are NOT MEASURED \u{2014} read `is_measured` and \
                     the measured-against-surface areas before believing the percentage. \
                     `discretisation_floor_mm` is what this grid can resolve ON THIS SURFACE: \
                     the larger of the plane-only `profile_floor_mm` and the curvature term \
                     `curvature_floor_p95_mm`. When `tolerance_below_floor` is true the bar is \
                     under the arithmetic. The grid's gap bias is NON-NEGATIVE \u{2014} a \
                     minimum over a sampled CL set sits at or above the continuum minimum \
                     \u{2014} so `unreachable_pct_of_measured_area` OVER-states: the true \
                     unreachable share is AT OR BELOW it, and \
                     `unresolved_pct_of_measured_area` is the band the grid cannot classify \
                     either way. `reached` is the sound side: a cell called reached really is \
                     formed. Raise the tolerance (the operation's own cusp is the honest bar) \
                     rather than reading the residual as tool geometry.",
        }))
    }

    fn mcp_screenshot_toolpath(
        &self,
        index: usize,
        path: &str,
        options: ScreenshotToolpathOptions,
    ) -> String {
        let ScreenshotToolpathOptions {
            width,
            height,
            show_stock,
            include_rapids,
            reach_overlay,
        } = options;
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
            // The two backgrounds are mutually exclusive and the reach map
            // wins: they answer different questions (what the machine has
            // removed against what this tool can form), and compositing one
            // over the other would leave the reader unable to say which
            // colour they were looking at.
            // F4: when the reach shading is the subject, the moves must not
            // bury it. See `CompositeSubject` for the measurement.
            let mut subject = rs_cam_core::export::fingerprint::CompositeSubject::Moves;
            let bg = if reach_overlay.unwrap_or(false) {
                match self.reach_overlay_background(index) {
                    Ok(mesh) => {
                        subject = rs_cam_core::export::fingerprint::CompositeSubject::Background;
                        Some(mesh)
                    }
                    Err(message) => return text(message),
                }
            } else if show_stock.unwrap_or(false) {
                self.controller
                    .state()
                    .simulation
                    .results
                    .as_ref()
                    .map(|sim| {
                        let mut m = sim_mesh_in_world_frame(&sim.mesh, session);
                        m.apply_height_gradient();
                        m
                    })
            } else {
                None
            };
            let pixels = rs_cam_core::export::fingerprint::render_toolpath_composite_subject(
                &result.annotated,
                bg.as_ref(),
                None,
                w,
                h,
                include_rapids.unwrap_or(true),
                subject,
            );
            let layer_note = match subject {
                rs_cam_core::export::fingerprint::CompositeSubject::Background => {
                    " \u{2014} reach shading at full brightness, moves drawn thin and \
                     dimmed so the shading reads from above"
                }
                rs_cam_core::export::fingerprint::CompositeSubject::Moves => "",
            };
            match image::save_buffer(Path::new(path), &pixels, w, h, image::ColorType::Rgba8) {
                Ok(()) => text(format!(
                    "Toolpath {index} exported to {path} ({w}x{h}, {} moves, \
                     {:.0}mm cutting){layer_note}",
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
            let html = rs_cam_core::export::viz::toolpath_standalone_3d_html(
                result.toolpath(),
                Some(bounds),
            );

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
            // M-4's refusal clock starts here, not at the last frame — see
            // `PendingGuiScreenshot::park_refusal` for why an idle window must
            // not be refused.
            requested_at: std::time::Instant::now(),
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
        // Checkpoint M-4. Read the refusal BEFORE arming the capture: arming it
        // on a loop that will never paint is exactly the hang this replaces.
        let refusal = {
            let Some(pending) = self
                .controller
                .pending_mcp
                .as_ref()
                .and_then(|p| p.gui_screenshot.as_ref())
            else {
                return;
            };
            pending.park_refusal(self.mcp_reads.frame_loop())
        };
        if let Some(reason) = refusal {
            // `take` the slot rather than leaving it armed — a refused request
            // is finished, and a stale slot would reject the caller's next
            // attempt with "another capture is already in flight".
            if let Some(slot) = self
                .controller
                .pending_mcp
                .as_mut()
                .and_then(|p| p.gui_screenshot.take())
            {
                tracing::warn!("{reason}");
                let _ = slot.response_tx.send(McpResponse {
                    result: Ok(text(reason)),
                });
            }
            return;
        }

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
        overlays: Option<&std::collections::BTreeMap<String, bool>>,
    ) -> String {
        // 1. Workspace.
        //
        // Applied SYNCHRONOUSLY, not through the event queue. A workspace
        // carries per-workspace overlay defaults (P6), and the event lands
        // later in this frame — so an `overlays` request in the same call
        // would be written first and then clobbered by the arriving
        // defaults, and every precondition below would have answered about
        // the OLD workspace. `UiCommand::SwitchWorkspace`'s handler calls the
        // same function, so this is the identical mutation, in the right
        // order.
        let mut workspace_applied: Option<&'static str> = None;
        if let Some(ws) = workspace {
            let Some(target) = parse_workspace(ws) else {
                return json_str(serde_json::json!({
                    "error": format!(
                        "Unknown workspace '{ws}'. Valid: setup, toolpaths, simulation, readiness"
                    )
                }));
            };
            crate::ui::overlays::registry::switch_workspace(self.controller.state_mut(), target);
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
            // The selection write is not the whole selection. Overlays whose
            // precondition reads a DERIVED per-selection artefact — the reach
            // map is the one — are answered from state the pump refreshes,
            // not from `selection` itself, and the pump runs after this call
            // returns. So `set_ui_view(toolpath_index: 13, overlays:
            // {reach_map: true})` in ONE call was refused with "select a
            // finishing operation" while the same overlays map in a SECOND
            // call applied (F3, 2026-09-08).
            //
            // Pumping it here is the same fix, and for the same reason, as
            // the synchronous workspace switch above: everything an
            // `overlays` map in this call will be judged against must
            // already be in force. `process_reach_overlay` resolves
            // `reach_map_spec` (two memoised geometry reads, no drop-cutter
            // work) and hands the walk to the Reach lane, so this stays a
            // cheap call on the request thread.
            self.controller.process_reach_overlay();
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
                crate::ui::overlays::registry::switch_workspace(
                    self.controller.state_mut(),
                    Workspace::Setup,
                );
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
                    events.push(AppEvent::Ui(UiCommand::CloseFeedsModal(NoArgs)));
                    events.push(AppEvent::Ui(UiCommand::CloseOptimizeModal(NoArgs)));
                    events.push(AppEvent::Ui(UiCommand::CloseOptimizeProject(NoArgs)));
                    events.push(AppEvent::Ui(UiCommand::CloseExportWizard(NoArgs)));
                    events.push(AppEvent::Ui(UiCommand::CloseToolLibrary(NoArgs)));
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
                        .push(AppEvent::Ui(UiCommand::OpenExportWizard(NoArgs)));
                }
                "tool_library" => {
                    self.controller
                        .events_mut()
                        .push(AppEvent::Ui(UiCommand::OpenToolLibrary(NoArgs)));
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

        // 5. Overlays (P6). Last, so the workspace switch above has already
        //    applied its defaults and the preconditions read the workspace
        //    the caller asked for. The registry is the single source of both
        //    the ids and the refusal strings, so this reply and the panel
        //    can never disagree.
        let overlay_report = overlays.map(|requested| {
            crate::ui::overlays::registry::apply_overlays(self.controller.state_mut(), requested)
        });

        // Echo the resulting view. Modal mutations route through the event
        // queue and land later this same frame, so echo the requested
        // targets plus the already-applied selection.
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
            "overlays": overlay_report.map(|report| serde_json::json!({
                "applied": report.applied,
                "refused": report.refused,
            })),
            "note": "view changes render on the next frame; call screenshot_gui to capture",
        }))
    }

    // ── Simulation scrubbing implementations ────────────────────────
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

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests;
