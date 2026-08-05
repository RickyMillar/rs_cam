//! Embedded MCP server that runs inside the GUI process.
//!
//! Each tool method constructs an `McpRequestKind`, sends it to the GUI thread
//! via a channel, calls `request_repaint()` to wake the GUI, and awaits the
//! oneshot response.

use std::time::Duration;

use rmcp::handler::server::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{Meta, ProgressNotificationParam, ServerInfo};
use rmcp::{Peer, RoleServer, ServerHandler, tool, tool_router};

use crate::compute::GenerationControl;
use crate::mcp_bridge::{
    McpReadCache, McpReadKind, McpRequest, McpRequestKind, ProgressUpdate,
    build_cancel_generation_response, build_generation_status_response,
};

// Re-use parameter structs from the standalone MCP crate.
use rs_cam_mcp::server::{
    AddAlignmentPinParam, AddToolFromLibraryParam, AddToolParam, AddToolpathParam,
    CollisionCheckParam, CutTraceParam, ExportParam, GenDebugTraceParam, GenerateAllParam,
    GenerateToolpathParam, ImportMachineSettingsParam, IndexParam, InspectSpansParam,
    ListToolCatalogParam, LoadMachineFromLibraryParam, LoadProjectParam, ModelIdParam,
    OperationSchemaParam, OptimizeToolpathInput, RemoveAlignmentPinParam, RemoveToolParam,
    RemoveToolpathParam, SaveProjectParam, ScreenshotGuiParam, ScreenshotSimParam,
    ScreenshotToolpathParam, SetBoundaryConfigParam, SetDressupConfigParam, SetDressupFieldParam,
    SetRestAnalysisConfigParam, SetSpindleStrategyParam, SetStockConfigParam, SetStockSourceParam,
    SetToolParamInput, SetToolpathEnabledParam, SetToolpathHeightsParam, SetToolpathParamInput,
    SetUiViewParam, SimJumpToMoveParam, SimJumpToToolpathBoundaryParam, SimScrubToolpathParam,
    SimulationParam, json_str,
};

/// How long a cheap read waits for the GUI frame loop before falling back to
/// the published snapshot.
///
/// A/M12 gate: `list_toolpaths` must answer in < 1 s. 750 ms leaves headroom
/// for the fallback render.
///
/// **C6 (2026-08-06): this deadline no longer depends on lane activity.** It
/// used to apply only while a toolpath generation was in flight, on the
/// assumption that the generation was the only thing that could hold the
/// frame loop. It is not: `drain_mcp_requests` handles every drained request
/// in one frame on the egui main thread, so a long `narrate_toolpath` or
/// `get_cut_trace` stalls the loop with the lane **idle** — and in that state
/// all five cheap reads took the unbounded path and blocked for the full
/// narration. That hole is H2.6's actual subject
/// (`planning/review_2026-08-04/FINISHING_OPEN_DEFECTS_EVIDENCE.md` §3.D.1).
/// The guarantee is now what agents were always told it was: a cheap read
/// answers within a second, whatever the GUI is doing.
const BUSY_READ_DEADLINE: Duration = Duration::from_millis(750);

/// Embedded MCP server that forwards requests to the GUI thread.
///
/// **A/M12 — two doors, on purpose.** Almost everything here still goes
/// through `request_tx`, which the egui main thread drains once per repaint in
/// `RsCamApp::drain_mcp_requests` and handles strictly sequentially. That
/// single-threaded, frame-driven dispatch — not any lock over
/// `ProjectSession`, and not the rmcp transport (which spawns a task per
/// inbound request) — is what serialized every MCP call behind a 40-minute
/// `generate_all` on 2026-07-30. The escape hatches therefore do NOT use it:
/// `cancel_generation` and `generation_status` are answered from
/// [`GenerationControl`] on this thread, and the cheap no-argument reads fall
/// back to [`McpReadCache`] when the frame loop misses [`BUSY_READ_DEADLINE`].
#[derive(Clone)]
pub struct EmbeddedCamServer {
    request_tx: std::sync::mpsc::Sender<McpRequest>,
    egui_ctx: egui::Context,
    /// Cancel + observe the toolpath lane without the GUI frame loop.
    generation: GenerationControl,
    /// Last-published read payloads, for answering during a stalled frame loop.
    reads: McpReadCache,
    #[allow(dead_code)]
    tool_router: ToolRouter<Self>,
}

impl EmbeddedCamServer {
    pub fn new(
        request_tx: std::sync::mpsc::Sender<McpRequest>,
        egui_ctx: egui::Context,
        generation: GenerationControl,
        reads: McpReadCache,
    ) -> Self {
        let tool_router = Self::tool_router();
        Self {
            request_tx,
            egui_ctx,
            generation,
            reads,
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

    /// A/M12 + C6: a cheap, no-argument read that **cannot** be trapped
    /// behind anything the GUI thread is doing.
    ///
    /// The GUI round-trip always races [`BUSY_READ_DEADLINE`]; if the frame
    /// loop loses, the answer comes from the last snapshot the GUI published,
    /// wrapped so the caller can never mistake a snapshot for a live read.
    /// When the frame loop answers in time — the overwhelmingly common case,
    /// since these five reads are O(1) once the loop reaches them — the
    /// payload is the live one, byte for byte as before.
    ///
    /// **C6 changed the condition, not the mechanism.** The deadline used to
    /// be armed only while the toolpath lane was active, so a frame loop
    /// stalled by a long READ (lane idle) blocked every cheap read for the
    /// full duration. Nothing about the busy path moved; the idle path
    /// stopped being a special case.
    async fn cheap_read(&self, kind: McpRequestKind, cached: McpReadKind) -> String {
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();
        let request = McpRequest {
            kind,
            response_tx,
            progress_tx: None,
        };
        if let Err(e) = self.request_tx.send(request) {
            return Self::format_result(Err(format!("Failed to send MCP request: {e}")));
        }
        self.egui_ctx.request_repaint();

        // Dropping `response_rx` on timeout is safe and already the house
        // pattern: every GUI-side resolution is `let _ = sender.send(..)`, and
        // this oneshot is not shared with any other call.
        match tokio::time::timeout(BUSY_READ_DEADLINE, response_rx).await {
            Ok(Ok(resp)) => Self::format_result(resp.result),
            Ok(Err(e)) => Self::format_result(Err(format!("MCP response channel closed: {e}"))),
            Err(_elapsed) => self.snapshot_fallback(cached),
        }
    }

    /// Render the stalled-frame-loop answer for a cheap read.
    fn snapshot_fallback(&self, cached: McpReadKind) -> String {
        let lane = self.generation.snapshot();
        // C6: the fallback is now reachable with the lane IDLE, so the text
        // must not assert a generation that is not running. A stalled frame
        // loop with nothing generating means a long in-band read is holding
        // it — which is the case this deadline was widened to cover, and
        // saying "a generation is in flight" there would be a lie an agent
        // would then act on.
        let stall_reason = if lane.is_active() {
            " because a toolpath generation is in flight"
        } else {
            " and no generation is running — a long in-band read (narrate_toolpath, \
             get_cut_trace, get_tool_load_report) is holding the frame loop"
        };
        let in_flight = serde_json::json!({
            "toolpath_index": lane.active_toolpath_index,
            "toolpath_id": lane.active_toolpath_id,
            "job": lane.current_job,
            "stage": lane.current_phase,
            "elapsed_s": lane.elapsed().map(|d| d.as_secs_f64()),
        });
        let tool = cached.tool_name();
        match self.reads.get(cached) {
            Some((payload, age)) => {
                let data = serde_json::from_str::<serde_json::Value>(&payload)
                    .unwrap_or(serde_json::Value::String(payload));
                json_str(serde_json::json!({
                    "ok": true,
                    "served_from": "snapshot",
                    "snapshot_age_s": age.as_secs_f64(),
                    "summary": format!(
                        "The GUI frame loop did not answer {tool} within {} ms{}. This is \
                         the last snapshot it published, {:.1} s ago — a lagging view, not \
                         a live read. `generation_status` is always live; \
                         `cancel_generation` always answers.",
                        BUSY_READ_DEADLINE.as_millis(),
                        stall_reason,
                        age.as_secs_f64(),
                    ),
                    "generation_in_flight": in_flight,
                    "data": data,
                }))
            }
            None => json_str(serde_json::json!({
                "ok": false,
                "served_from": "nothing",
                "error": format!(
                    "{tool} could not be answered: the GUI frame loop did not respond{} \
                     and has never published a snapshot (no frame has completed since \
                     startup). Use `generation_status` for live lane state, or \
                     `cancel_generation` to stop a running job.",
                    stall_reason,
                ),
                "generation_in_flight": in_flight,
            })),
        }
    }

    /// Send a request to the GUI and forward progress notifications to the MCP
    /// client while awaiting the final response, optionally bounded by a
    /// wall-clock `timeout`.
    ///
    /// On timeout this method does not cancel anything — the GUI-side
    /// compute keeps running untouched, and this call simply stops
    /// *waiting* for it, returning a `still_running_response()` instead.
    /// `response_rx` (and `progress_rx`) are then dropped: every GUI-side
    /// resolution already goes through `let _ = sender.send(...)` (see
    /// `notify_mcp_toolpath_complete` / `mcp_generate_all` in
    /// `app/mcp.rs`), so the real completion arriving later is a silent
    /// no-op send into a channel nobody is listening on anymore — it is
    /// never delivered to a subsequent, unrelated call because each call
    /// to this method owns a fresh oneshot/mpsc pair.
    async fn send_with_progress(
        &self,
        kind: McpRequestKind,
        meta: Meta,
        // `None` = no client to notify. Only an integration test passes it:
        // an rmcp `Peer` cannot be constructed outside a live service, and
        // the A/M12 gate requires driving generate -> status -> cancel over
        // this surface rather than the library API underneath it.
        peer: Option<Peer<RoleServer>>,
        timeout: Option<Duration>,
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

        // No `timeout` becomes a deadline that never resolves, so the
        // branch below is simply never the `select!` winner — avoids both
        // an `Option` inside the loop and any risk of `Instant` overflow
        // from stand-in "very large" sleep durations.
        let deadline = async move {
            match timeout {
                Some(d) => tokio::time::sleep(d).await,
                None => std::future::pending::<()>().await,
            }
        };
        let mut deadline = std::pin::pin!(deadline);

        // Forward progress notifications to the client only when the
        // caller supplied a progress token; the `if` guard keeps the
        // branch disabled (never polled) otherwise, matching the previous
        // no-token fast path.
        let progress_token = meta.get_progress_token().filter(|_| peer.is_some());
        let mut resp_rx = std::pin::pin!(response_rx);
        loop {
            tokio::select! {
                Some(update) = progress_rx.recv(), if progress_token.is_some() => {
                    if let (Some(token), Some(peer)) = (&progress_token, peer.as_ref()) {
                        let _ = peer.notify_progress(ProgressNotificationParam {
                            progress_token: token.clone(),
                            progress: update.progress,
                            total: update.total,
                            message: Some(update.message),
                        }).await;
                    }
                }
                result = &mut resp_rx => {
                    return match result {
                        Ok(resp) => resp.result,
                        Err(e) => Err(format!("MCP response channel closed: {e}")),
                    };
                }
                () = &mut deadline => {
                    return Ok(Self::still_running_response(timeout));
                }
            }
        }
    }

    /// Build the `status: "running"` response returned by `send_with_progress`
    /// when a caller-supplied `timeout_s` elapses before the GUI compute
    /// finishes. `ok: true` because nothing failed — the generate simply
    /// outlived the caller's wait budget and continues in the background.
    fn still_running_response(timeout: Option<Duration>) -> String {
        let waited = timeout.map(|d| d.as_secs()).unwrap_or(0);
        json_str(serde_json::json!({
            "ok": true,
            "status": "running",
            "summary": format!(
                "Still running after the {waited}s wait budget. Generation was NOT \
                 cancelled and continues in the background. Use generation_status for \
                 the in-flight toolpath index, stage and elapsed time; list_toolpaths \
                 for the per-op picture (served from a snapshot while the GUI is \
                 busy); cancel_generation to abort. All three answer within a second \
                 whatever the generation is doing. Avoid re-issuing the same \
                 generate_toolpath (same index) or generate_all call while this one is \
                 still in flight — resubmitting a toolpath that's already being \
                 generated cancels and restarts its in-flight job instead of checking \
                 on it."
            ),
        }))
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
        description = "Get project summary: name, stock dimensions, setup count, toolpath count, tools. Answers within 1s even while a generation is in flight — if the GUI is busy you get the last published snapshot under `served_from: \"snapshot\"` with its age."
    )]
    pub async fn project_summary(&self) -> String {
        self.cheap_read(McpRequestKind::ProjectSummary, McpReadKind::ProjectSummary)
            .await
    }

    #[tool(
        name = "list_toolpaths",
        description = "List all toolpaths with name, operation type, enabled status, and tool. Answers within 1s even while a generation is in flight — if the GUI is busy you get the last published snapshot under `served_from: \"snapshot\"` with its age. For live in-flight detail use `generation_status`."
    )]
    pub async fn list_toolpaths(&self) -> String {
        self.cheap_read(McpRequestKind::ListToolpaths, McpReadKind::ListToolpaths)
            .await
    }

    #[tool(
        name = "list_tools",
        description = "List all tools with type and dimensions"
    )]
    async fn list_tools(&self) -> String {
        Self::format_result(self.send_request(McpRequestKind::ListTools).await)
    }

    #[tool(
        name = "list_tool_library",
        description = "Top level of the tool-library drill-down: list the reusable catalogs in the user's library (~/.config/rs_cam/tools/*.toml), each with its tool count and the tool types it contains. Cheap and small. Then dig into a catalog with `list_tool_catalog` to see its tools, and import one with `add_tool_from_library`. Does NOT require a loaded project."
    )]
    async fn list_tool_library(&self) -> String {
        Self::format_result(self.send_request(McpRequestKind::ListToolLibrary).await)
    }

    #[tool(
        name = "list_tool_catalog",
        description = "Drill into one tool-library catalog (from `list_tool_library`) and list its tools as compact rows: 0-based `index`, name, type, diameter, flutes, cutting length, relevant angle, and shank diameter. Pick one (check geometry against the model + machine shank limit), then call `add_tool_from_library` with this catalog name and the row's `index`."
    )]
    async fn list_tool_catalog(
        &self,
        Parameters(ListToolCatalogParam { catalog }): Parameters<ListToolCatalogParam>,
    ) -> String {
        Self::format_result(
            self.send_request(McpRequestKind::ListToolCatalog { catalog })
                .await,
        )
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
        name = "get_operation_schema",
        description = "Returns the full param schema for one operation kind without needing a toolpath: every settable field with its JSON type, whether it is Optional, the default value, and (when known) the valid range or enum variants. Use this BEFORE `add_toolpath` / `set_toolpath_param` when you are not already familiar with the operation's params. Supported `operation_type` values match `add_toolpath`."
    )]
    async fn get_operation_schema(
        &self,
        #[allow(clippy::needless_pass_by_value)]
        Parameters(OperationSchemaParam { operation_type }): Parameters<
            OperationSchemaParam,
        >,
    ) -> String {
        Self::format_result(
            self.send_request(McpRequestKind::GetOperationSchema { operation_type })
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
        name = "narrate_toolpath",
        description = "Return a concise prose narration of one generated toolpath: Z-level structure, perimeter-sweep estimates, suspicious large arcs, peak axial DOC, and air-cut percentage. Prefer this first for agent debugging before raw traces/screenshots. Run generate_toolpath first; run_simulation first for DOC/air-cut metrics. COST: this runs on the GUI thread and scales with move count. Timed on an idle lane (2026-08-06): 4 ms on a 12.6k-move scallop pass with a 70k-sample cut trace. The \"~12 min\" figure previously quoted here was a single wall-clock observation taken DURING a 40-minute generate_all, i.e. mostly queue time, and has been retired. Other GUI-served calls do queue behind this one; `generation_status` and `cancel_generation` are unaffected, and the five cheap no-argument reads now fall back to a published snapshot rather than blocking."
    )]
    async fn narrate_toolpath(
        &self,
        Parameters(IndexParam { index }): Parameters<IndexParam>,
    ) -> String {
        Self::format_result(
            self.send_request(McpRequestKind::NarrateToolpath { index })
                .await,
        )
    }

    #[tool(
        name = "recommend_clearing_strategy",
        description = "Strategy advisor: for the Adaptive3d roughing toolpath at `index`, compare clearing strategies (ContourParallel vs ContourSpiral) by planning each at its load-limited (Suggest-backed-off) params and timing it through the acceleration-aware cycle-time integrator at the machine's effective kinematics. Returns the recommended strategy, the binding regime (tool-/machine-limited) as the human-readable why, every candidate ranked by wall-clock seconds, and the speed margin. KEY: it decides parallel-vs-spiral by MACHINE ACCELERATION, not the load regime — on a low-accel router a backed-off ContourParallel beats the spiral even when tool-limited. Only applies to Adaptive3d ops. HEAVY: it plans one toolpath per candidate, so expect ~30-60 s and a GUI freeze while it computes."
    )]
    async fn recommend_clearing_strategy(
        &self,
        Parameters(IndexParam { index }): Parameters<IndexParam>,
    ) -> String {
        Self::format_result(
            self.send_request(McpRequestKind::RecommendClearingStrategy { index })
                .await,
        )
    }

    #[tool(
        name = "get_suggest_rationale",
        description = "Run combined-Suggest against the toolpath at `index` and return the structured rationale tree explaining every parameter the orchestrator backed off or rewrote: DPP deflection back-off, runtime stepover floor, plunge-entry warnings, chipload-target feed lift, etc. Each entry carries param + reason + from/to values + a human-readable headline. Read this before set_toolpath_param when you want to know why Suggest chose a value. Does not mutate the project."
    )]
    async fn get_suggest_rationale(
        &self,
        Parameters(IndexParam { index }): Parameters<IndexParam>,
    ) -> String {
        Self::format_result(
            self.send_request(McpRequestKind::GetSuggestRationale { index })
                .await,
        )
    }

    #[tool(
        name = "get_cut_trace",
        description = "Get simulation cut trace data: semantic summaries, structural span summaries, hotspots, issues, and (for drill toolpaths) drill_summaries. Run simulation first. Filter to a single toolpath via toolpath_id, or to a structural span via span_kind (e.g. \"depth_pass\"), span_id (from inspect_spans), or pass_index (DepthPass payload, 0-based). Set include_drill_samples=true to also include the per-peck DrillSample stream (can be verbose)."
    )]
    async fn get_cut_trace(
        &self,
        #[allow(clippy::needless_pass_by_value)] Parameters(CutTraceParam {
            toolpath_id,
            max_hotspots,
            max_issues,
            span_kind,
            span_id,
            pass_index,
            include_drill_samples,
        }): Parameters<CutTraceParam>,
    ) -> String {
        Self::format_result(
            self.send_request(McpRequestKind::GetCutTrace {
                toolpath_id,
                max_hotspots,
                max_issues,
                span_kind,
                span_id,
                pass_index,
                include_drill_samples: include_drill_samples.unwrap_or(false),
            })
            .await,
        )
    }

    #[tool(
        name = "get_generation_debug_trace",
        description = "Get the generation-time debug trace for a toolpath: per-pass spans with exit_reason, idle_count, yield_ratio, xy_bbox, plus a diagnostic summary. Primary surface for diagnosing adaptive3d AgentSearch wandering/looping. Run generate_toolpath first. Filter by span_kind, exit_reason, or max_yield_ratio to narrow the response."
    )]
    async fn get_generation_debug_trace(
        &self,
        #[allow(clippy::needless_pass_by_value)] Parameters(GenDebugTraceParam {
            index,
            span_kind,
            exit_reason,
            max_yield_ratio,
            max_spans,
        }): Parameters<GenDebugTraceParam>,
    ) -> String {
        Self::format_result(
            self.send_request(McpRequestKind::GetGenerationDebugTrace {
                index,
                span_kind,
                exit_reason,
                max_yield_ratio,
                max_spans,
            })
            .await,
        )
    }

    #[tool(
        name = "inspect_model",
        description = "Inspect all loaded models: mesh stats, bbox, BREP face summary, polygon summary. Returns a JSON array."
    )]
    async fn inspect_model(&self) -> String {
        self.cheap_read(McpRequestKind::InspectModel, McpReadKind::InspectModel)
            .await
    }

    #[tool(
        name = "inspect_stock",
        description = "Inspect stock configuration: dimensions, origin, material, padding, alignment pins, workholding rigidity."
    )]
    async fn inspect_stock(&self) -> String {
        self.cheap_read(McpRequestKind::InspectStock, McpReadKind::InspectStock)
            .await
    }

    #[tool(
        name = "inspect_machine",
        description = "Inspect machine profile: spindle, power, feeds limits, rigidity factors."
    )]
    async fn inspect_machine(&self) -> String {
        self.cheap_read(McpRequestKind::InspectMachine, McpReadKind::InspectMachine)
            .await
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

    #[tool(
        name = "inspect_collisions",
        description = "List collisions detected in the last simulation, grouped by toolpath. Returns holder/shank collisions and rapid collisions with global+local move indices, so the user can drill from the project-wide rapid_collision_count down to the specific toolpath and lift/retract that's clipping uncleared stock. Run run_simulation first."
    )]
    async fn inspect_collisions(&self) -> String {
        Self::format_result(self.send_request(McpRequestKind::InspectCollisions).await)
    }

    #[tool(
        name = "inspect_spans",
        description = "Inspect the structural spans (Operation, DepthPass, Region, Entry, LeadOut, LinkBridge, DressupArtifact, GeometryRefit, RapidOrderBarrier) of a generated toolpath. With no filter, returns a summary: total `kind_counts` plus outermost spans (Operation + DepthPass) under `top_level` with child counts. Pass `kind`, `parent_id`, `pass_index`, or `region_id` to retrieve detail spans under `spans`. `kind` is a SpanKind name in snake_case. `parent_id` is a span id from a previous call (drill: Operation → DepthPass → Region). `pass_index` matches DepthPass payload; `region_id` matches Region payload. `max_spans` caps the result (default 50; report `truncated` + `total_matching` when capped). Run generate_toolpath first."
    )]
    async fn inspect_spans(
        &self,
        Parameters(InspectSpansParam {
            index,
            kind,
            parent_id,
            pass_index,
            region_id,
            max_spans,
        }): Parameters<InspectSpansParam>,
    ) -> String {
        Self::format_result(
            self.send_request(McpRequestKind::InspectSpans {
                index,
                kind,
                parent_id,
                pass_index,
                region_id,
                max_spans,
            })
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
        name = "add_setup",
        description = "Add a new setup (workholding orientation) to the project. Returns the new setup index and ID. Use set_setup_face to change orientation."
    )]
    async fn add_setup(
        &self,
        Parameters(rs_cam_mcp::server::AddSetupParam { name }): Parameters<
            rs_cam_mcp::server::AddSetupParam,
        >,
    ) -> String {
        Self::format_result(self.send_request(McpRequestKind::AddSetup { name }).await)
    }

    #[tool(
        name = "set_setup_face",
        description = "Set the face-up orientation for a setup. Options: 'top' (default), 'bottom', 'front', 'back', 'left', 'right'. Controls which face of the stock is accessible to the spindle."
    )]
    async fn set_setup_face(
        &self,
        Parameters(rs_cam_mcp::server::SetSetupFaceParam {
            setup_index,
            face_up,
        }): Parameters<rs_cam_mcp::server::SetSetupFaceParam>,
    ) -> String {
        Self::format_result(
            self.send_request(McpRequestKind::SetSetupFace {
                setup_index,
                face_up,
            })
            .await,
        )
    }

    #[tool(
        name = "move_toolpath_to_setup",
        description = "Move a toolpath from its current setup to a different setup. Both indices are 0-based."
    )]
    async fn move_toolpath_to_setup(
        &self,
        Parameters(rs_cam_mcp::server::MoveToolpathToSetupParam {
            toolpath_index,
            target_setup_index,
        }): Parameters<rs_cam_mcp::server::MoveToolpathToSetupParam>,
    ) -> String {
        Self::format_result(
            self.send_request(McpRequestKind::MoveToolpathToSetup {
                toolpath_index,
                target_setup_index,
            })
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

    #[tool(
        name = "export_gcode",
        description = "Export G-code to a file path. Refuses if any toolpath has tool-load Exceeds or Unmodeled verdicts unless the corresponding accept flag is set."
    )]
    async fn export_gcode(
        &self,
        Parameters(ExportParam {
            path,
            accept_unmodeled_tool_load,
            accept_exceeded_tool_load,
            tool_change_mode,
            split_setups,
        }): Parameters<ExportParam>,
    ) -> String {
        Self::format_result(
            self.send_request(McpRequestKind::ExportGcode {
                path,
                accept_unmodeled_tool_load,
                accept_exceeded_tool_load,
                tool_change_mode,
                split_setups,
            })
            .await,
        )
    }

    #[tool(
        name = "get_tool_load_report",
        description = "Per-toolpath tool-load report: chipload, power, deflection verdicts. Each criterion is independent (no scalar load %). Each gate uses a typed verdict: ChiploadVerdict (Within carries approach-to-min/max metrics, Exceeds carries ChipSide + ChiploadStatistic + ChipBounds), PowerVerdict (carries peak_kw + available_kw on both arms), DeflectionVerdict (carries peak_mm + DeflectionBounds with 50µm/200µm thresholds). All three states are Within/Exceeds/Unmodeled."
    )]
    async fn get_tool_load_report(&self) -> String {
        Self::format_result(self.send_request(McpRequestKind::GetToolLoadReport).await)
    }

    #[tool(
        name = "get_toolpath_diagnostics",
        description = "Unified per-toolpath diagnostic list. Returns a Vec<Diagnostic> with severity (info/hint/caution/critical/blocking), category (safety/geometry/tool_load/quality/efficiency/state), confidence (verified/approximate/static/heuristic), state (current/needs_simulation/stale_evidence/not_applicable), source, message, optional evidence (sample range, LUT row, geometric compare), optional fix payload, and supersedes graph. Heuristic pre-sim hints are auto-suppressed when verified sim evidence is current. This is the recommended view — it merges tool-load gates, drill gates, feeds-calculator warnings, stale-default defects, and static config checks into one canonical list."
    )]
    async fn get_toolpath_diagnostics(
        &self,
        #[allow(clippy::needless_pass_by_value)]
        Parameters(OptimizeToolpathInput { index }): Parameters<OptimizeToolpathInput>,
    ) -> String {
        Self::format_result(
            self.send_request(McpRequestKind::GetToolpathDiagnostics { index })
                .await,
        )
    }

    #[tool(
        name = "get_project_diagnostics",
        description = "Project-wide diagnostics (rapid collisions, holder collisions, plunge stress, high air-cut percentage, generated-empty). Returns a Vec<Diagnostic> in the same unified schema as get_toolpath_diagnostics. Pair with get_toolpath_diagnostics for per-toolpath findings."
    )]
    async fn get_project_diagnostics(&self) -> String {
        Self::format_result(
            self.send_request(McpRequestKind::GetProjectDiagnostics)
                .await,
        )
    }

    #[tool(
        name = "optimize_toolpath",
        description = "Run the optimizer on one toolpath. Searches across feed/RPM (analytical Stage 0) and DOC variants (Stage 1/2 sims). Each candidate is sim-verified end-to-end. Returns OptimizeOutcome JSON, one of: Ranked(candidates) with cycle time + verdict per row (auto-recommendation surface); MarginalSafe(candidates, explanation) — every gate is Within but at least one reading was admitted only by the layer-1 tolerance band, verify on a scrap before applying; TradeOff(candidates) — faster candidate exists but improves a failing gate while worsening a non-failing one (user must accept regression); NoSafeImprovement(narrative); or Skipped(reason). Long-running — the GUI thread blocks for the duration (~1-2 min for a 3D op). Run simulation first; the optimizer scores candidates against the existing baseline trace."
    )]
    async fn optimize_toolpath(
        &self,
        #[allow(clippy::needless_pass_by_value)]
        Parameters(OptimizeToolpathInput { index }): Parameters<OptimizeToolpathInput>,
    ) -> String {
        Self::format_result(
            self.send_request(McpRequestKind::OptimizeToolpath { index })
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
        name = "set_toolpath_heights",
        description = "Set a toolpath's clearance/retract/feed/top/bottom Z planes (the heights config that op-specific params don't expose). Each field is optional — omit to leave a plane unchanged; a value pins it to that absolute Z (in the operation's emission frame). Use this to raise the clearance plane above tall uncut features when rapids collide (e.g. a chamfer on a 2D curve over a tall stock rim). Invalidates the toolpath — regenerate to apply."
    )]
    async fn set_toolpath_heights(
        &self,
        #[allow(clippy::needless_pass_by_value)] Parameters(SetToolpathHeightsParam {
            index,
            clearance_z,
            retract_z,
            feed_z,
            top_z,
            bottom_z,
        }): Parameters<SetToolpathHeightsParam>,
    ) -> String {
        Self::format_result(
            self.send_request(McpRequestKind::SetToolpathHeights {
                index,
                clearance_z,
                retract_z,
                feed_z,
                top_z,
                bottom_z,
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
        name = "add_tool_from_library",
        description = "Import a tool from a library catalog into the loaded project as a snapshot (the project keeps its own copy, so later catalog edits don't change it). Identify the tool by `catalog` + `index` from `list_tool_library`. Returns the new project tool index for use with `add_toolpath`."
    )]
    async fn add_tool_from_library(
        &self,
        #[allow(clippy::needless_pass_by_value)]
        Parameters(AddToolFromLibraryParam { catalog, index }): Parameters<
            AddToolFromLibraryParam,
        >,
    ) -> String {
        Self::format_result(
            self.send_request(McpRequestKind::AddToolFromLibrary { catalog, index })
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
        description = "Set the machining boundary for a toolpath. Sources: 'stock', 'model_silhouette', 'derived_rest_regions' (requires source_toolpath_id — the id of another toolpath whose pencil rest-depth result supplies the regions). Containment: 'center', 'inside', 'outside'. Invalidates cached result."
    )]
    async fn set_boundary_config(
        &self,
        #[allow(clippy::needless_pass_by_value)] Parameters(SetBoundaryConfigParam {
            index,
            enabled,
            source,
            containment,
            offset,
            source_toolpath_id,
        }): Parameters<SetBoundaryConfigParam>,
    ) -> String {
        Self::format_result(
            self.send_request(McpRequestKind::SetBoundaryConfig {
                index,
                enabled,
                source,
                containment,
                offset,
                source_toolpath_id,
            })
            .await,
        )
    }

    #[tool(
        name = "set_rest_analysis_config",
        description = "Enable/configure op-agnostic rest analysis on a toolpath: runs the rest-depth detector against THIS toolpath's own tool after generation, attaching a heatmap grid + derived machining regions (usable as a 'derived_rest_regions' boundary source on another toolpath) without emitting a pencil centerline toolpath. reference_tool_id (optional) names a real library tool for the rest reference; unset prefers the machined stock, else a self-referenced probe. cell_mm/min_valley_depth/region_margin_mm default to 0.5/0.05/0.5mm. offset_stepover_mm/num_offset_passes tune the fan the ROUTING criterion assumes a downstream pencil pass would emit; leave both unset and the stepover is sized by the canonical reach policy for this toolpath's own cutter (correct unless you are modelling a specific pinned downstream operation). Invalidates cached result."
    )]
    async fn set_rest_analysis_config(
        &self,
        #[allow(clippy::needless_pass_by_value)] Parameters(SetRestAnalysisConfigParam {
            index,
            enabled,
            reference_tool_id,
            cell_mm,
            min_valley_depth,
            region_margin_mm,
            offset_stepover_mm,
            num_offset_passes,
        }): Parameters<SetRestAnalysisConfigParam>,
    ) -> String {
        Self::format_result(
            self.send_request(McpRequestKind::SetRestAnalysisConfig {
                index,
                enabled,
                reference_tool_id,
                cell_mm,
                min_valley_depth,
                region_margin_mm,
                offset_stepover_mm,
                num_offset_passes,
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

    #[tool(
        name = "set_dressup_field",
        description = "Update a single dressup field on a toolpath (partial patch). Accepts any field name from the DressupConfig schema."
    )]
    async fn set_dressup_field(
        &self,
        #[allow(clippy::needless_pass_by_value)]
        Parameters(SetDressupFieldParam { index, key, value }): Parameters<
            SetDressupFieldParam,
        >,
    ) -> String {
        Self::format_result(
            self.send_request(McpRequestKind::SetDressupField { index, key, value })
                .await,
        )
    }

    #[tool(
        name = "set_toolpath_enabled",
        description = "Enable or disable a toolpath for generation and simulation."
    )]
    async fn set_toolpath_enabled(
        &self,
        #[allow(clippy::needless_pass_by_value)]
        Parameters(SetToolpathEnabledParam { index, enabled }): Parameters<
            SetToolpathEnabledParam,
        >,
    ) -> String {
        Self::format_result(
            self.send_request(McpRequestKind::SetToolpathEnabled { index, enabled })
                .await,
        )
    }

    #[tool(
        name = "set_stock_source",
        description = "Set stock_source for a toolpath: 'fresh' (default) or 'from_remaining_stock' (rest machining). Invalidates the toolpath result."
    )]
    async fn set_stock_source(
        &self,
        #[allow(clippy::needless_pass_by_value)]
        Parameters(SetStockSourceParam { index, source }): Parameters<SetStockSourceParam>,
    ) -> String {
        Self::format_result(
            self.send_request(McpRequestKind::SetStockSource { index, source })
                .await,
        )
    }

    #[tool(
        name = "set_spindle_strategy",
        description = "Set the project-level spindle policy that drives the Feeds & Speeds Suggest path. Accepts 'match_chart' (default — use the vendor LUT row's chart-published RPM verbatim) or 'max_speed' (push RPM up the constant-chipload line toward the spindle ceiling and scale feed proportionally). Affects every Suggest call across the project on the next frame; existing toolpath param values are NOT mutated by this call — invoke get_toolpath_params or run Suggest to see the new recommendations. Mirrors the GUI's Feeds & Speeds modal radio toggle."
    )]
    async fn set_spindle_strategy(
        &self,
        #[allow(clippy::needless_pass_by_value)]
        Parameters(SetSpindleStrategyParam { strategy }): Parameters<
            SetSpindleStrategyParam,
        >,
    ) -> String {
        Self::format_result(
            self.send_request(McpRequestKind::SetSpindleStrategy { strategy })
                .await,
        )
    }

    // ── Compute tools ────────────────────────────────────────────────

    #[tool(
        name = "generate_toolpath",
        description = "Generate a single toolpath by index. Returns move count and distances. By default waits indefinitely for generation to finish; pass `timeout_s` to bound the wait — on timeout the call returns a `status: \"running\"` response instead of blocking (the generate is NOT cancelled, it keeps running in the background). While it runs, `generation_status` reports the stage and elapsed time live, `list_toolpaths` answers from a snapshot, and `cancel_generation` aborts it — all three are served off the GUI frame loop and answer within a second."
    )]
    async fn generate_toolpath(
        &self,
        #[allow(clippy::needless_pass_by_value)]
        Parameters(GenerateToolpathParam { index, timeout_s }): Parameters<
            GenerateToolpathParam,
        >,
        meta: Meta,
        peer: Peer<RoleServer>,
    ) -> String {
        Self::format_result(
            self.send_with_progress(
                McpRequestKind::GenerateToolpath { index },
                meta,
                Some(peer),
                timeout_s.map(Duration::from_secs),
            )
            .await,
        )
    }

    #[tool(
        name = "generate_all",
        description = "Generate all enabled toolpaths, iterating to a fixpoint over the rest-machining chain: generate, simulate, regenerate whatever was blocked only on missing upstream stock, repeat. A project with a k-deep chain of \"remaining stock\" operations reaches fully generated in ONE call, and the reply reports how many internal rounds and simulations it took. `simulation_resolution_mm` is REQUIRED when the project has enabled rest-machining ops (the call refuses rather than guessing a cell size — resolution changes collision counts and engagement); pass `fixpoint: false` for the old single pass. The reply separates `errors` (genuine failures) from `awaiting_prior_stock` (ops still waiting, each naming the operation it waits for). By default waits indefinitely; pass `timeout_s` to bound the wait — on timeout the call returns a `status: \"running\"` response instead of blocking (nothing is cancelled, generation continues in the background). While it runs, `generation_status` reports which toolpath index is in flight and its stage, `list_toolpaths` answers from a snapshot, and `cancel_generation` aborts it — all three are served off the GUI frame loop and answer within a second."
    )]
    async fn generate_all(
        &self,
        #[allow(clippy::needless_pass_by_value)] Parameters(GenerateAllParam {
            timeout_s,
            fixpoint,
            simulation_resolution_mm,
        }): Parameters<GenerateAllParam>,
        meta: Meta,
        peer: Peer<RoleServer>,
    ) -> String {
        Self::format_result(
            self.send_with_progress(
                McpRequestKind::GenerateAll {
                    fixpoint,
                    simulation_resolution_mm,
                },
                meta,
                Some(peer),
                timeout_s.map(Duration::from_secs),
            )
            .await,
        )
    }

    /// A/M12 test seam: the `generate_all` tool body with the rmcp `Peer`
    /// made optional. An rmcp `Peer` cannot be constructed outside a live
    /// service, and the acceptance gate requires driving
    /// generate -> status -> cancel over *this* surface rather than the
    /// library API underneath it. Behaviourally identical to the tool call
    /// except that no progress notifications are emitted.
    pub async fn generate_all_without_peer(
        &self,
        timeout_s: Option<u64>,
        fixpoint: Option<bool>,
        simulation_resolution_mm: Option<f64>,
    ) -> String {
        Self::format_result(
            self.send_with_progress(
                McpRequestKind::GenerateAll {
                    fixpoint,
                    simulation_resolution_mm,
                },
                Meta::new(),
                None,
                timeout_s.map(Duration::from_secs),
            )
            .await,
        )
    }

    #[tool(
        name = "cancel_generation",
        description = "Cancel whatever toolpath generation is currently in flight (the toolpath compute lane) — the fix for a runaway generate_toolpath/generate_all call that would otherwise hang indefinitely. Served on the MCP server thread, never queued behind the GUI frame loop, so it answers and sets the cancel flag whatever the GUI is doing. It sets a flag: the worker stops at its next cancellation checkpoint, which is not instantaneous. `was_busy` describes the lane at the moment you asked. The cancelled toolpath's status reverts to pending (not Done); any pending generate_toolpath/generate_all call for it resolves on its own shortly after with a cancelled outcome."
    )]
    pub async fn cancel_generation(&self) -> String {
        build_cancel_generation_response(&self.generation.request_cancel())
    }

    #[tool(
        name = "generation_status",
        description = "What the toolpath compute lane is doing RIGHT NOW: lane state, the in-flight toolpath index and id, the planner stage, seconds elapsed, and how many jobs are queued behind it. Read live off the lane on the MCP server thread — it answers whatever the GUI is doing, so it is the call to reach for when a generate_toolpath/generate_all is taking longer than expected and you need to attribute the cost to an operation before deciding whether to cancel_generation. A null `stage` means the operation publishes no stages, not that the lane is stalled; watch `elapsed_s` and `stage` across two calls to see progress."
    )]
    pub async fn generation_status(&self) -> String {
        build_generation_status_response(&self.generation.snapshot())
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
            self.send_with_progress(
                McpRequestKind::RunSimulation { resolution },
                meta,
                Some(peer),
                None,
            )
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
            self.send_with_progress(
                McpRequestKind::CollisionCheck { index },
                meta,
                Some(peer),
                None,
            )
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

    #[tool(
        name = "screenshot_gui",
        description = "Capture the FULL application window — all panels, menus, tabs, and open modals — to a PNG file (unlike screenshot_simulation/screenshot_toolpath, which render the 3D scene offscreen without the surrounding UI). Optional width/height (logical points) resize the window before capture; the new size persists afterwards (it is not restored). The capture lands 1-2 frames after the request. Pair with set_ui_view to navigate to the surface you want to capture first."
    )]
    async fn screenshot_gui(
        &self,
        #[allow(clippy::needless_pass_by_value)] Parameters(ScreenshotGuiParam {
            path,
            width,
            height,
        }): Parameters<ScreenshotGuiParam>,
    ) -> String {
        Self::format_result(
            self.send_request(McpRequestKind::ScreenshotGui {
                path,
                width,
                height,
            })
            .await,
        )
    }

    #[tool(
        name = "set_ui_view",
        description = "Navigate the GUI to a specific view so screenshot_gui can capture any UI surface. All params optional, applied in order: `workspace` switches the top-level workspace ('setup', 'toolpaths', 'simulation', 'readiness'); `toolpath_index` (0-based) selects that toolpath so its properties panel shows in the Toolpaths workspace; `properties_tab` activates a toolpath inspector tab ('geometry', 'feeds', 'linking', 'heights', 'dressup' — requires a selected toolpath to be visible); `select` chooses a non-toolpath properties panel ('machine' for machine setup + kinematics + GRBL $$ import, or 'stock') and switches to the Setup workspace; `modal` opens a modal ('feeds_modal', 'optimize_modal', 'export_wizard', 'tool_library') or 'none' closes all modals. Preconditions: feeds_modal and optimize_modal need a toolpath — pass toolpath_index in the same call or have one selected; optimize_modal starts a REAL Optimize run (long, ~1-2 min — without a prior simulation it shows a 'simulation required' outcome instead). Returns a JSON echo of the resulting view state. Changes render on the next frame, so call screenshot_gui after this returns."
    )]
    async fn set_ui_view(
        &self,
        #[allow(clippy::needless_pass_by_value)] Parameters(SetUiViewParam {
            workspace,
            toolpath_index,
            properties_tab,
            select,
            modal,
        }): Parameters<SetUiViewParam>,
    ) -> String {
        Self::format_result(
            self.send_request(McpRequestKind::SetUiView {
                workspace,
                toolpath_index,
                properties_tab,
                select,
                modal,
            })
            .await,
        )
    }

    #[tool(
        name = "import_machine_settings",
        description = "Import a GRBL `$$` settings dump onto the live machine profile (headless equivalent of the GUI Machine panel's '$$' import). Parses `$N=value` lines — `$11`→junction deviation, `$120/$121/$122`→per-axis acceleration, `$110/$111`→max feed — tolerating grblHAL `(description)` comments, CRLF, and unrelated settings. Sets the machine's kinematics + max feed, opting the cycle-time model into the acceleration-aware path, and breaks any machine-library link. Returns a JSON summary of what was applied. Verify afterwards with inspect_machine."
    )]
    async fn import_machine_settings(
        &self,
        #[allow(clippy::needless_pass_by_value)]
        Parameters(ImportMachineSettingsParam { dump }): Parameters<
            ImportMachineSettingsParam,
        >,
    ) -> String {
        Self::format_result(
            self.send_request(McpRequestKind::ImportMachineSettings { dump })
                .await,
        )
    }

    #[tool(
        name = "list_machine_library",
        description = "List the reusable machines in the per-user machine library (~/.config/rs_cam/machines/*.toml), each with a compact spec summary (name, max feed, and kinematics: per-axis acceleration + junction deviation when set). The library uses SNAPSHOT semantics like the tool library — import one with `load_machine_from_library` to COPY it into the project. Does NOT require a loaded project."
    )]
    async fn list_machine_library(&self) -> String {
        Self::format_result(self.send_request(McpRequestKind::ListMachineLibrary).await)
    }

    #[tool(
        name = "load_machine_from_library",
        description = "Snapshot-import a machine from the library (by name from `list_machine_library`) into the project: it is COPIED into the project's inline machine (no live link), becoming authoritative. Invalidates machine-dependent state. Verify with inspect_machine."
    )]
    async fn load_machine_from_library(
        &self,
        #[allow(clippy::needless_pass_by_value)]
        Parameters(LoadMachineFromLibraryParam { name }): Parameters<
            LoadMachineFromLibraryParam,
        >,
    ) -> String {
        Self::format_result(
            self.send_request(McpRequestKind::LoadMachineFromLibrary { name })
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
