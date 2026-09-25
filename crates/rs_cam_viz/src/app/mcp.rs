//! GUI-side processing of MCP requests. Called from `RsCamApp::update()`.

#![deny(clippy::indexing_slicing)]

pub(crate) mod commands;
mod diagnostics;
mod generation;
mod project;
mod simulation;
mod view;

pub use generation::suggest_basis_json;

use std::path::Path;

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

use rs_cam_mcp::server::{json_str, text};

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
                // EDG-06: one findings list over every per-setup file, so
                // the reply names an error-severity finding in file 2 as
                // plainly as one in file 1.
                let mut machine_safety: Vec<rs_cam_core::export::gcode_validator::Finding> =
                    Vec::new();
                for (i, (id, name)) in setups.iter().enumerate() {
                    let exported =
                        match crate::io::export::export_setup_gcode_from_session_reporting(
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
                            Ok(e) => e,
                            Err(e) => return text(format!("Export failed (setup '{name}'): {e}")),
                        };
                    let gcode = exported.gcode;
                    machine_safety.extend(exported.machine_safety);
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
                return text(crate::io::export::export_success_text(
                    format!(
                        "Exported {total} per-setup G-code files:\n{}",
                        written.join("\n")
                    ),
                    &machine_safety,
                ));
            }
        }

        // EDG-06: take the reporting door. The machine-safety pass ran on
        // every export before this row and only the server log read it, so
        // an export that tripped `Severity::Error` — a rapid below the
        // clearance plane, a Z below the program floor — read to the
        // calling agent exactly like a clean one.
        let exported = match crate::io::export::export_gcode_from_session_reporting(
            &state.session,
            &state.gui,
            &state.simulation,
            policy,
            stale,
        ) {
            Ok(e) => e,
            Err(e) => return text(format!("Export failed: {e}")),
        };
        match std::fs::write(Path::new(path), &exported.gcode) {
            Ok(()) => text(crate::io::export::export_success_text(
                format!("G-code exported to {path}"),
                &exported.machine_safety,
            )),
            Err(e) => text(format!("Export failed: {e}")),
        }
    }

    /// Put the operator's screen on the toolpath an MCP mutation is about
    /// to change: switch to the Toolpaths workspace and select the row.
    /// The same two moves the `SetToolpathParam` arm makes inline (that
    /// arm also records an `mcp_highlights` key for the param it changed,
    /// which a rebind has no equivalent of).
    fn select_toolpath_for_mcp(&mut self, index: usize) {
        // The workspace switch is the caller's: SHL-02 moved it onto the
        // `declare_core_requests!` table's workspace column, and both
        // callers of this helper carry `Some(Workspace::Toolpaths)` there.
        let tp_id = self
            .controller
            .state()
            .session
            .toolpath_configs()
            .get(index)
            .map(|tc| tc.id);
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
