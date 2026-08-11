//! Bridge types for communication between the embedded MCP server (tokio thread)
//! and the GUI main thread. The GUI owns all state; the MCP server sends requests
//! and receives responses via channels.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use serde::Serialize;

pub use rs_cam_mcp::response::CutTraceCaps;

use crate::state::toolpath::ToolpathId;

/// The no-argument, cheap-to-render read payloads the GUI main thread
/// republishes once per frame so the MCP server thread can answer them while
/// the frame loop is stalled behind a long generation (A/M12).
///
/// This is deliberately NOT a lock over `ProjectSession`. There never was one:
/// the pre-A/M12 serializer was the single-threaded, frame-driven
/// `RsCamApp::drain_mcp_requests` dispatch, so relaxing a session lock would
/// have relaxed nothing. Publishing already-rendered strings keeps the reader
/// side lock-free-ish (an uncontended `RwLock` read of five `String`s) and
/// keeps every field's meaning identical to the live call it mirrors — these
/// are the *same* functions' output, captured a frame or two ago.
#[derive(Default)]
pub struct McpReadSnapshot {
    pub list_toolpaths: String,
    pub project_summary: String,
    pub inspect_model: String,
    pub inspect_stock: String,
    pub inspect_machine: String,
    /// `None` until the GUI has published at least once.
    pub published_at: Option<Instant>,
}

/// Which cached payload a fallback read wants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpReadKind {
    ListToolpaths,
    ProjectSummary,
    InspectModel,
    InspectStock,
    InspectMachine,
}

impl McpReadKind {
    pub fn tool_name(self) -> &'static str {
        match self {
            Self::ListToolpaths => "list_toolpaths",
            Self::ProjectSummary => "project_summary",
            Self::InspectModel => "inspect_model",
            Self::InspectStock => "inspect_stock",
            Self::InspectMachine => "inspect_machine",
        }
    }

    fn pick(self, snapshot: &McpReadSnapshot) -> &str {
        match self {
            Self::ListToolpaths => &snapshot.list_toolpaths,
            Self::ProjectSummary => &snapshot.project_summary,
            Self::InspectModel => &snapshot.inspect_model,
            Self::InspectStock => &snapshot.inspect_stock,
            Self::InspectMachine => &snapshot.inspect_machine,
        }
    }
}

/// How long the GUI frame loop may go without a frame before the escape
/// hatches call it **parked** rather than merely busy.
///
/// The MCP heartbeat in `RsCamApp::update` schedules a repaint every 100 ms
/// while an MCP server is wired, so a live loop beats at ≥10 Hz even with
/// nothing happening. Two seconds is twenty missed heartbeats: far outside
/// frame jitter, far inside the timescale of the live stall this bounds
/// (268 s and counting).
pub const PARKED_FRAME_LOOP: Duration = Duration::from_secs(2);

/// Frame-loop liveness and outstanding-MCP-work counters, readable from the
/// MCP server thread without touching the GUI.
///
/// **G-LV.1 (live, 2026-08-07).** A `generate_all` fixpoint run over MCP
/// stalled indefinitely between rounds once the GUI window stopped
/// repainting: the lane finished `3D Rough 6` and went idle, the next
/// simulate-round handoff never ran, and `generation_status` — truthfully,
/// and uselessly — answered "idle: no toolpath generation in flight". Every
/// MCP request and every fixpoint round handoff is dispatched from
/// `RsCamApp::update`, which only runs on a repaint, so with no repaints
/// nothing advances and the lane state alone reads exactly like success.
///
/// **`request_repaint()` cannot fix that**, which is why this type exists
/// instead of another wake-up call. On Wayland (`eframe` 0.34.3 /
/// `winit` 0.30.13) a repaint request ends in `Window::request_redraw`, and
/// winit's Wayland loop refuses to emit `RedrawRequested` while the surface
/// is waiting on a compositor frame callback
/// (`wayland/event_loop/mod.rs`: `if window.frame_callback_state() ==
/// FrameCallbackState::Requested { return None }`). A hidden, occluded or
/// screen-locked surface gets no frame callbacks, so the loop parks and no
/// amount of requesting can unpark it. eframe's own rescue — painting
/// invisible windows directly in `check_redraw_requests` — is gated on
/// `is_invisible_or_minimized`, and winit's Wayland backend returns `None`
/// from both `is_visible()` and `is_minimized()`, so it never fires there.
///
/// What is left, and what this carries, is the truth: how long ago the frame
/// loop last ran, and how much MCP work is stranded behind it. That turns an
/// indefinite silent hang into a diagnosis an agent can act on.
#[derive(Debug)]
pub struct FrameLoopBeat {
    origin: Instant,
    /// Milliseconds since `origin` at the last [`FrameLoopBeat::beat`].
    /// Meaningless while `frames` is 0.
    last_frame_ms: AtomicU64,
    /// Milliseconds since `origin` at the last [`FrameLoopBeat::frame_begin`].
    frame_started_ms: AtomicU64,
    /// Set between `frame_begin` and `beat`. **The whole reason parked and
    /// busy are separable**: a frame loop stuck inside one long in-band read
    /// (C6's case) and a frame loop that is not running at all (G-LV.1's)
    /// both show "no recent frame", and only one of them ends by itself.
    in_frame: AtomicBool,
    /// Frames that have drained the MCP channel. 0 = the loop has never run.
    frames: AtomicU64,
    /// Requests handed to the channel by the MCP server thread.
    sent: AtomicU64,
    /// Requests the frame loop has dispatched. `sent - handled` is the
    /// channel backlog: work that has not been *started*.
    handled: AtomicU64,
    /// MCP callers still awaiting a deferred completion as of the last beat
    /// (`PendingMcpCompute`). Work that was started and has not *finished* —
    /// the generate_all case, which no channel counter can see.
    awaiting: AtomicU64,
    /// Whether a `generate_all` was among them. Named because it is the one
    /// that drives multi-round work off future frames.
    awaiting_generate_all: AtomicBool,
}

impl Default for FrameLoopBeat {
    fn default() -> Self {
        Self {
            origin: Instant::now(),
            last_frame_ms: AtomicU64::new(0),
            frame_started_ms: AtomicU64::new(0),
            in_frame: AtomicBool::new(false),
            frames: AtomicU64::new(0),
            sent: AtomicU64::new(0),
            handled: AtomicU64::new(0),
            awaiting: AtomicU64::new(0),
            awaiting_generate_all: AtomicBool::new(false),
        }
    }
}

impl FrameLoopBeat {
    fn now_ms(&self) -> u64 {
        u64::try_from(self.origin.elapsed().as_millis()).unwrap_or(u64::MAX)
    }

    /// Called by the GUI main thread as it enters the MCP dispatch.
    pub fn frame_begin(&self) {
        self.frame_started_ms
            .store(self.now_ms(), Ordering::Relaxed);
        self.in_frame.store(true, Ordering::Release);
    }

    /// Called by the GUI main thread once per frame, after the MCP channel is
    /// drained, with what the frame left outstanding.
    pub fn beat(&self, awaiting: u64, awaiting_generate_all: bool) {
        let ms = self.now_ms();
        self.awaiting.store(awaiting, Ordering::Relaxed);
        self.awaiting_generate_all
            .store(awaiting_generate_all, Ordering::Relaxed);
        self.last_frame_ms.store(ms, Ordering::Relaxed);
        self.in_frame.store(false, Ordering::Release);
        // Release-store last: a reader that sees a non-zero frame count is
        // guaranteed to see a timestamp that was written for some frame.
        self.frames.fetch_add(1, Ordering::Release);
    }

    /// Called by the MCP server thread after a request reaches the channel.
    pub fn record_sent(&self) {
        self.sent.fetch_add(1, Ordering::Relaxed);
    }

    /// Called by the GUI main thread once per dispatched request.
    pub fn record_handled(&self) {
        self.handled.fetch_add(1, Ordering::Relaxed);
    }

    /// Frames that have drained the MCP channel since startup.
    pub fn frames(&self) -> u64 {
        self.frames.load(Ordering::Acquire)
    }

    /// Time since the frame loop last ran, or `None` if it never has.
    pub fn frame_age(&self) -> Option<Duration> {
        if self.frames.load(Ordering::Acquire) == 0 {
            return None;
        }
        let ms = self.last_frame_ms.load(Ordering::Relaxed);
        Some(
            self.origin
                .elapsed()
                .saturating_sub(Duration::from_millis(ms)),
        )
    }

    /// Requests sitting in the channel that no frame has picked up.
    pub fn backlog(&self) -> u64 {
        self.sent
            .load(Ordering::Relaxed)
            .saturating_sub(self.handled.load(Ordering::Relaxed))
    }

    /// Deferred completions an MCP caller was still awaiting at the last beat.
    pub fn awaiting(&self) -> u64 {
        self.awaiting.load(Ordering::Relaxed)
    }

    /// Whether a `generate_all` was among them.
    pub fn awaiting_generate_all(&self) -> bool {
        self.awaiting_generate_all.load(Ordering::Relaxed)
    }

    /// Total MCP work that cannot progress without a frame.
    pub fn stranded(&self) -> u64 {
        self.backlog().saturating_add(self.awaiting())
    }

    /// Whether the GUI thread is currently inside its MCP dispatch.
    pub fn in_frame(&self) -> bool {
        self.in_frame.load(Ordering::Acquire)
    }

    /// How long the in-progress frame has been running, or `None` when the
    /// loop is between frames. A large value is C6's long in-band read.
    pub fn current_frame_age(&self) -> Option<Duration> {
        if !self.in_frame() {
            return None;
        }
        let started = self.frame_started_ms.load(Ordering::Relaxed);
        Some(
            self.origin
                .elapsed()
                .saturating_sub(Duration::from_millis(started)),
        )
    }

    /// The loop has not run for [`PARKED_FRAME_LOOP`] — or has never run at
    /// all, which is the same thing once work has been sent to it.
    ///
    /// A loop that is *inside* a frame is never parked however long it has
    /// been in there: that is C6's long in-band read, which ends on its own.
    /// Conflating the two would send a caller to wait out a stall that does
    /// not end.
    pub fn is_parked(&self) -> bool {
        if self.in_frame() {
            return false;
        }
        match self.frame_age() {
            Some(age) => age >= PARKED_FRAME_LOOP,
            None => self.sent.load(Ordering::Relaxed) > 0,
        }
    }

    /// The liveness block the escape hatches publish.
    ///
    /// Reported unconditionally, healthy or not: an agent that only ever sees
    /// the field when something is wrong has no baseline to compare against.
    pub fn report(&self) -> serde_json::Value {
        serde_json::json!({
            "healthy": !self.is_parked(),
            "in_frame": self.in_frame(),
            "current_frame_age_s": self.current_frame_age().map(|d| d.as_secs_f64()),
            "last_frame_age_s": self.frame_age().map(|d| d.as_secs_f64()),
            "frames": self.frames(),
            "requests_in_channel": self.backlog(),
            "awaiting_completion": self.awaiting(),
            "awaiting_generate_all": self.awaiting_generate_all(),
        })
    }
}

/// Shared handle onto [`McpReadSnapshot`] and the [`FrameLoopBeat`]. Cloned
/// into the MCP server thread.
///
/// The two travel together because they answer the same question from
/// opposite sides: the snapshot is what the frame loop last *said*, the beat
/// is when it last *ran* and what it left undone.
#[derive(Clone, Default)]
pub struct McpReadCache {
    snapshot: Arc<RwLock<McpReadSnapshot>>,
    frame_loop: Arc<FrameLoopBeat>,
}

impl McpReadCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Frame-loop liveness, shared with the GUI thread.
    pub fn frame_loop(&self) -> &FrameLoopBeat {
        &self.frame_loop
    }

    /// Called by the GUI main thread. Cheap: five `String` moves under a
    /// write guard nobody holds for longer than that.
    pub fn publish(&self, snapshot: McpReadSnapshot) {
        let mut guard = self.snapshot.write().unwrap_or_else(|e| e.into_inner());
        *guard = McpReadSnapshot {
            published_at: Some(Instant::now()),
            ..snapshot
        };
    }

    /// Age of the most recent publish, or `None` if the GUI never published.
    pub fn age(&self) -> Option<Duration> {
        let guard = self.snapshot.read().unwrap_or_else(|e| e.into_inner());
        guard.published_at.map(|at| at.elapsed())
    }

    /// The cached payload for `kind` plus its age, or `None` when the GUI has
    /// not published yet.
    pub fn get(&self, kind: McpReadKind) -> Option<(String, Duration)> {
        let guard = self.snapshot.read().unwrap_or_else(|e| e.into_inner());
        let age = guard.published_at?.elapsed();
        Some((kind.pick(&guard).to_owned(), age))
    }
}

/// A progress update sent from GUI to MCP during long operations.
#[derive(Debug, Clone)]
pub struct ProgressUpdate {
    /// Human-readable status message.
    pub message: String,
    /// Progress value (0.0 to total).
    pub progress: f64,
    /// Total steps (if known).
    pub total: Option<f64>,
}

/// A request from the MCP server to the GUI thread.
pub struct McpRequest {
    pub kind: McpRequestKind,
    pub response_tx: tokio::sync::oneshot::Sender<McpResponse>,
    /// Optional channel for streaming progress updates back to the MCP client.
    pub progress_tx: Option<tokio::sync::mpsc::Sender<ProgressUpdate>>,
}

/// What the MCP server is asking the GUI to do.
pub enum McpRequestKind {
    // ── Reads (instant) ──────────────────────────────────────────────
    ProjectSummary,
    ListToolpaths,
    ListTools,
    /// Top level of the tool-library drill-down: list catalog names with
    /// tool counts and the tool types each contains. Independent of the
    /// loaded project. Dig into one with `ListToolCatalog`.
    ListToolLibrary,
    /// Drill into one catalog: compact tool rows with a 0-based `index`
    /// for `AddToolFromLibrary`.
    ListToolCatalog {
        catalog: String,
    },
    ListSetups,
    GetToolpathParams {
        index: usize,
    },
    GetOperationSchema {
        operation_type: String,
    },
    GetDiagnostics,
    GetToolLoadReport,
    /// PR-3: unified diagnostic list for a single toolpath. Returns
    /// the deduped [`rs_cam_core::diagnostics::Diagnostic`] vector
    /// (post-supersession) for the toolpath at the given index.
    GetToolpathDiagnostics {
        index: usize,
    },
    /// PR-3: project-wide diagnostic list (collisions, air-cut, etc.)
    /// derived from `ProjectDiagnostics::verdicts`.
    GetProjectDiagnostics,
    /// Run the optimizer on a single toolpath and return the
    /// OptimizeOutcome as JSON. Long-running (~1-2 min for a typical
    /// 3D op); the GUI thread is blocked for the duration.
    OptimizeToolpath {
        index: usize,
    },
    GetCutTrace {
        /// Project-level toolpath **id** (not index) — the key the cut
        /// trace itself is stored under. An unmatched id is refused.
        toolpath_id: Option<usize>,
        max_hotspots: Option<usize>,
        max_issues: Option<usize>,
        /// Optional `SpanKind` filter (snake_case, e.g. "depth_pass").
        span_kind: Option<String>,
        /// Optional exact `SpanId` (vec index) match.
        span_id: Option<u32>,
        /// Optional `DepthPass` `pass_index` payload match.
        pass_index: Option<u32>,
        /// When true, include the per-peck `drill_samples` array in the
        /// response (defaults to false; verbose).
        include_drill_samples: bool,
        /// Checkpoint L per-array caps. `None` takes the ruled default
        /// (`rs_cam_mcp::response`); an explicit `usize::MAX` is uncapped.
        caps: CutTraceCaps,
    },
    GetGenerationDebugTrace {
        index: usize,
        span_kind: Option<String>,
        exit_reason: Option<String>,
        max_yield_ratio: Option<f64>,
        max_spans: Option<usize>,
    },
    NarrateToolpath {
        index: usize,
    },
    /// Strategy advisor (`STRATEGY_ADVISOR_2026-06-17`): compare clearing
    /// strategies for the `Adaptive3d` toolpath at `index` and recommend the
    /// one with the minimum acceleration-aware wall-clock at the load limit.
    /// Plans one toolpath per candidate strategy, so it is heavy (~30-60 s).
    RecommendClearingStrategy {
        index: usize,
    },
    /// v3.2 (2026-06-04): Run the combined-Suggest orchestrator
    /// against the toolpath at `index` and return the
    /// [`rs_cam_core::feeds::rationale::SuggestRationale`] tree as JSON.
    /// The agent can read why Suggest would back off a value before
    /// accepting / rejecting individual parameter changes.
    GetSuggestRationale {
        index: usize,
    },

    InspectModel,
    InspectStock,
    InspectMachine,
    InspectBrepFaces {
        model_id: usize,
    },
    /// Per-toolpath listing of rapid + holder collisions: which moves they
    /// happen at, what XYZ position, and the local move index within the
    /// toolpath. Localizes the project-wide `rapid_collision_count` from
    /// run_simulation. Run simulation first.
    InspectCollisions,
    /// Dump the structural spans (Operation, DepthPass, Region, Entry, LeadOut,
    /// LinkBridge, DressupArtifact, RapidOrderBarrier) of a generated toolpath.
    /// Lets agents inspect toolpath anatomy without parsing the raw move list.
    /// Defaults to a summary (kind_counts + outermost spans). Set any filter
    /// to retrieve detail spans.
    InspectSpans {
        index: usize,
        /// `SpanKind` filter (snake_case e.g. "depth_pass").
        kind: Option<String>,
        /// Restrict to spans contained within the span at this id (vec index).
        parent_id: Option<u32>,
        /// `DepthPass` `pass_index` payload match.
        pass_index: Option<u32>,
        /// `Region` `region_id` payload match.
        region_id: Option<u32>,
        /// Cap on returned spans (default 50).
        max_spans: Option<usize>,
    },

    // ── Mutations (instant) ──────────────────────────────────────────
    AddAlignmentPin {
        x: f64,
        y: f64,
        diameter: f64,
    },
    RemoveAlignmentPin {
        index: usize,
    },
    ImportModel {
        path: String,
    },
    AddSetup {
        name: Option<String>,
    },
    SetSetupFace {
        setup_index: usize,
        face_up: String,
    },
    MoveToolpathToSetup {
        toolpath_index: usize,
        target_setup_index: usize,
    },
    LoadProject {
        path: String,
    },
    SaveProject {
        path: String,
    },
    ExportGcode {
        path: String,
        accept_unmodeled_tool_load: bool,
        accept_exceeded_tool_load: bool,
        tool_change_mode: Option<String>,
        split_setups: bool,
    },
    SetToolpathParam {
        index: usize,
        param: String,
        value: serde_json::Value,
    },
    SetToolParam {
        index: usize,
        param: String,
        value: serde_json::Value,
    },
    /// Set a toolpath's clearance/retract/feed/top/bottom Z planes.
    /// Each field is optional; `None` leaves that plane unchanged. A
    /// `Some(v)` pins the plane to absolute Z `v` (HeightMode::Manual).
    SetToolpathHeights {
        index: usize,
        clearance_z: Option<f64>,
        retract_z: Option<f64>,
        feed_z: Option<f64>,
        top_z: Option<f64>,
        bottom_z: Option<f64>,
    },
    AddToolpath {
        setup_index: usize,
        operation_type: String,
        tool_index: usize,
        model_id: usize,
        name: Option<String>,
    },
    RemoveToolpath {
        index: usize,
    },
    AddTool {
        name: String,
        tool_type: String,
        diameter: f64,
    },
    /// Import a snapshot of a catalog tool into the project. Identified
    /// by catalog name + 0-based index from `ListToolLibrary`.
    AddToolFromLibrary {
        catalog: String,
        index: usize,
    },
    RemoveTool {
        index: usize,
    },
    SetStockConfig {
        x: f64,
        y: f64,
        z: f64,
    },
    SetBoundaryConfig {
        index: usize,
        enabled: bool,
        source: Option<String>,
        containment: Option<String>,
        offset: Option<f64>,
        /// Required when `source` is `"derived_rest_regions"` — the stable
        /// id (`ToolpathConfig.id`, not an index) of the toolpath whose
        /// cached pencil rest-depth result supplies the boundary polygons.
        source_toolpath_id: Option<usize>,
    },
    SetRestAnalysisConfig {
        index: usize,
        enabled: bool,
        reference_tool_id: Option<usize>,
        cell_mm: Option<f64>,
        min_valley_depth: Option<f64>,
        region_margin_mm: Option<f64>,
        /// PR-7 (H2.5): `None` = size the routing fan from the canonical
        /// reach policy, which is what an operator who does not name a
        /// downstream operation wants.
        offset_stepover_mm: Option<f64>,
        num_offset_passes: Option<usize>,
    },
    SetDressupConfig {
        index: usize,
        dressup: serde_json::Value,
    },
    SetDressupField {
        index: usize,
        key: String,
        value: serde_json::Value,
    },
    SetToolpathEnabled {
        index: usize,
        enabled: bool,
    },
    SetStockSource {
        index: usize,
        source: String,
    },
    /// Set the project-level spindle policy ("match_chart" or
    /// "max_speed"). See [`rs_cam_core::feeds::SpindleStrategy`].
    /// Mirrors the GUI's Feeds & Speeds modal radio toggle.
    SetSpindleStrategy {
        strategy: String,
    },
    /// Checkpoint I-5 (2026-08-12): apply the Feeds & Speeds recommendation
    /// to one toolpath through the same funnel both GUI surfaces use, with
    /// the scope stated explicitly ("speeds" / "cut_geometry" / "both").
    /// Refuses — it does not silently no-op — when the tool × operation
    /// pairing is one `validate_tool_for_operation` declines.
    ApplyFeeds {
        index: usize,
        scope: String,
    },

    // ── Compute (async — response sent when compute finishes) ────────
    GenerateToolpath {
        index: usize,
    },
    /// A/M11: generate every enabled toolpath, iterating to a fixpoint over
    /// the rest-stock chain when `fixpoint` is set. See [`FixpointPlan`].
    GenerateAll {
        /// `None` = fixpoint on (the default). `Some(false)` = the
        /// pre-A/M11 single pass.
        fixpoint: Option<bool>,
        /// Cell size in mm for the loop's own simulations. Required whenever
        /// the loop is on AND the project has rest-machining ops; never
        /// defaulted (A/M10).
        simulation_resolution_mm: Option<f64>,
    },
    // A/M12: `CancelGeneration` used to live here, which is exactly why it
    // could not do its job — it queued behind the generation it was meant to
    // abort. It is now served on the MCP server thread through
    // `crate::compute::GenerationControl` and never reaches this channel.
    RunSimulation {
        resolution: Option<f64>,
    },
    CollisionCheck {
        index: usize,
    },

    // ── Simulation scrubbing ───────────────────────────────────────────
    SimJumpToMove {
        move_index: usize,
    },
    SimJumpToStart,
    SimJumpToEnd,
    SimScrubToolpath {
        index: usize,
        percent: f64,
    },
    SimJumpToToolpathStart {
        index: usize,
    },
    SimJumpToToolpathEnd {
        index: usize,
    },

    // ── Screenshots ──────────────────────────────────────────────────
    ScreenshotSimulation {
        path: String,
        width: Option<u32>,
        height: Option<u32>,
        checkpoint: Option<usize>,
        include_toolpaths: Option<bool>,
    },
    ScreenshotToolpath {
        index: usize,
        path: String,
        width: Option<u32>,
        height: Option<u32>,
        show_stock: Option<bool>,
        include_rapids: Option<bool>,
    },
    /// Capture the full application window (all panels) to a PNG. The
    /// response is deferred: the GUI issues
    /// `ViewportCommand::Screenshot` and completes the response 1-2
    /// frames later when `egui::Event::Screenshot` arrives.
    ScreenshotGui {
        path: String,
        /// Optional window resize (logical points) applied before capture.
        /// The new size persists after the capture.
        width: Option<f32>,
        height: Option<f32>,
    },

    // ── UI navigation ────────────────────────────────────────────────
    /// Drive the GUI to a specific view state (workspace, toolpath
    /// selection, properties tab, modal) so `ScreenshotGui` can capture
    /// any UI surface. Fields are applied in declaration order.
    SetUiView {
        workspace: Option<String>,
        toolpath_index: Option<usize>,
        properties_tab: Option<String>,
        select: Option<String>,
        modal: Option<String>,
    },
    /// Import a GRBL `$$` settings dump onto the live machine profile
    /// (headless equivalent of the GUI Machine panel's `$$` import).
    ImportMachineSettings {
        dump: String,
    },
    /// List the reusable machines in the per-user machine library, each
    /// with a compact spec summary. No project required.
    ListMachineLibrary,
    /// Snapshot-import the named library machine into the project (copies
    /// it into the inline machine; no live link).
    LoadMachineFromLibrary {
        name: String,
    },
}

/// Render the `cancel_generation` reply from what the lane actually did.
///
/// A/M12 moved this off the GUI thread: the cancel now runs synchronously on
/// the MCP server thread via [`crate::compute::GenerationControl`], so
/// `was_busy` describes the lane at the instant the caller asked rather than
/// whenever the frame loop got round to it.
pub fn build_cancel_generation_response(
    outcome: &crate::compute::CancelOutcome,
    frame_loop: &FrameLoopBeat,
) -> String {
    let snapshot = &outcome.snapshot;
    if outcome.was_busy {
        let job = snapshot.current_job.as_deref().unwrap_or("(unnamed job)");
        rs_cam_mcp::server::json_str(serde_json::json!({
            "ok": true,
            "summary": format!("Cancel requested for in-flight generation: {job}"),
            "was_busy": true,
            "toolpath_index": snapshot.active_toolpath_index,
            "toolpath_id": snapshot.active_toolpath_id,
            "stage": snapshot.current_phase,
            "elapsed_s": snapshot.elapsed().map(|d| d.as_secs_f64()),
            "frame_loop": frame_loop.report(),
            "note": "The flag is set synchronously; the worker observes it at its \
                     next cancellation checkpoint. The toolpath's status reverts to \
                     pending (not Done) and any pending generate_toolpath / \
                     generate_all call for it resolves on its own with a cancelled \
                     outcome.",
        }))
    } else {
        // G-LV.1: "nothing to cancel" is only good news if the frame loop is
        // alive. Parked, it means the job you wanted to stop already ended
        // and its *result* is what is stranded — a different problem, and
        // one cancelling cannot solve.
        let summary = match parked_frame_loop_warning(frame_loop) {
            Some(warning) => {
                format!("No toolpath generation in flight — nothing to cancel. {warning}")
            }
            None => "No toolpath generation in flight — nothing to cancel".to_owned(),
        };
        rs_cam_mcp::server::json_str(serde_json::json!({
            "ok": true,
            "summary": summary,
            "was_busy": false,
            "frame_loop": frame_loop.report(),
            "note": "This reports the lane state at the moment you asked — the call \
                     is serviced on the MCP server thread and never queues behind a \
                     generation.",
        }))
    }
}

/// The sentence that turns G-LV.1's silent hang into a diagnosis, or `None`
/// when the frame loop is doing its job.
///
/// Deliberately gated on *stranded work*, not on staleness alone: a GUI
/// nobody is driving and nobody is waiting on is idle, not broken, and
/// warning about it would train agents to ignore the field.
fn parked_frame_loop_warning(frame_loop: &FrameLoopBeat) -> Option<String> {
    let stranded = frame_loop.stranded();
    if !frame_loop.is_parked() || stranded == 0 {
        return None;
    }
    let age = frame_loop.frame_age().map_or_else(
        || "has never completed a frame".to_owned(),
        |d| format!("has not run for {:.1} s", d.as_secs_f64()),
    );
    let what = if frame_loop.awaiting_generate_all() {
        "including a generate_all, whose round handoffs are driven off future frames"
    } else {
        "and none of it can start or finish without one"
    };
    Some(format!(
        "WARNING — the GUI frame loop {age} and {stranded} MCP request(s) are stranded \
         behind it, {what}. An idle lane here does NOT mean your call completed: every \
         MCP request and every generate_all round handoff is dispatched from a GUI \
         repaint. A hidden, occluded or screen-locked window gets no repaints — on \
         Wayland the compositor withholds the frame callbacks winit needs before it will \
         emit RedrawRequested, so request_repaint() cannot break the park. Make the GUI \
         window visible and focused to resume it, or relaunch the GUI with WAYLAND_DISPLAY \
         UNSET (winit then picks X11/XWayland, where redraws are client-driven and never \
         gated on the compositor). Do NOT set WINIT_UNIX_BACKEND — winit removed that \
         variable in 0.29 and it does nothing on the 0.30 this build uses."
    ))
}

/// Render the `generation_status` reply from a live toolpath-lane snapshot.
///
/// A/M12: without this there is no way to attribute a long generation's cost
/// to an operation, and every "it's been 40 minutes" is unfalsifiable — the
/// only diagnosis available on 2026-07-30 was reading `/proc` thread
/// accounting from outside the process.
pub fn build_generation_status_response(
    snapshot: &crate::compute::LaneSnapshot,
    frame_loop: &FrameLoopBeat,
) -> String {
    use crate::compute::LaneState;

    let lane_state = match snapshot.state {
        LaneState::Idle => "idle",
        LaneState::Queued => "queued",
        LaneState::Running => "running",
        LaneState::Cancelling => "cancelling",
    };
    let elapsed_s = snapshot.elapsed().map(|d| d.as_secs_f64());
    let summary = if snapshot.is_active() {
        let job = snapshot.current_job.as_deref().unwrap_or("(unnamed job)");
        let index = snapshot
            .active_toolpath_index
            .map_or_else(|| "?".to_owned(), |i| i.to_string());
        let stage = snapshot.current_phase.as_deref().unwrap_or(NO_STAGE);
        format!(
            "{lane_state}: toolpath index {index} — {job}, stage '{stage}', {:.1}s elapsed, \
             {} more queued",
            elapsed_s.unwrap_or(0.0),
            snapshot.queue_depth,
        )
    } else {
        "idle: no toolpath generation in flight".to_owned()
    };

    // G-LV.1: this is the exact string the live stall answered with, and it
    // was true — and it read as "your generate_all finished". Append what the
    // lane cannot know: whether anything is still waiting on a frame loop
    // that has stopped running.
    let summary = match parked_frame_loop_warning(frame_loop) {
        Some(warning) => format!("{summary}. {warning}"),
        None => summary,
    };

    rs_cam_mcp::server::json_str(serde_json::json!({
        "ok": true,
        "busy": snapshot.is_active(),
        "lane_state": lane_state,
        "toolpath_index": snapshot.active_toolpath_index,
        "toolpath_id": snapshot.active_toolpath_id,
        "job": snapshot.current_job,
        "stage": snapshot.current_phase,
        "elapsed_s": elapsed_s,
        "queue_depth": snapshot.queue_depth,
        "summary": summary,
        "frame_loop": frame_loop.report(),
        "note": "Live: read straight off the compute lane on the MCP server thread, \
                 never through the GUI frame loop. `stage` is whatever the planner \
                 last reported — a null means the op reports no stages, NOT that it \
                 is doing nothing. `lane_state` describes the COMPUTE LANE only — \
                 read `frame_loop` before reading an idle lane as \"my call finished\".",
    }))
}

/// What `generation_status` prints when the planner has reported no stage.
/// Spelled out rather than left blank: a missing stage means the operation
/// publishes none, not that the lane is stalled.
const NO_STAGE: &str = "(none reported)";

/// Response from the GUI thread to the MCP server.
pub struct McpResponse {
    pub result: Result<String, String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MutationResult<T>
where
    T: Serialize,
{
    pub ok: bool,
    pub summary: String,
    pub applied: T,
    pub stale_toolpaths: Vec<usize>,
    pub warnings: Vec<MutationWarning>,
    pub gui_banners: Vec<GuiBanner>,
    pub diagnostic_delta: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MutationWarning {
    pub level: String,
    pub field: Option<String>,
    pub message: String,
    pub recommendation: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GuiBanner {
    pub kind: String,
    pub severity: String,
    pub title: String,
    pub detail: Option<String>,
}

/// Tracks pending MCP compute operations awaiting async results.
#[derive(Default)]
pub struct PendingMcpCompute {
    /// Toolpath ID -> oneshot sender for when that toolpath finishes.
    pub toolpath: HashMap<ToolpathId, tokio::sync::oneshot::Sender<McpResponse>>,
    /// Oneshot sender for when the simulation finishes.
    pub simulation: Option<tokio::sync::oneshot::Sender<McpResponse>>,
    /// Oneshot sender for when collision check finishes.
    pub collision: Option<tokio::sync::oneshot::Sender<McpResponse>>,
    /// For generate_all: track pending toolpaths and a final response sender.
    pub generate_all: Option<PendingGenerateAll>,
    /// For screenshot_gui: the in-flight full-window capture. The GUI
    /// pumps this every frame (`pump_mcp_gui_screenshot`) until the
    /// `egui::Event::Screenshot` result lands and the response is sent.
    pub gui_screenshot: Option<PendingGuiScreenshot>,
}

impl PendingMcpCompute {
    /// How many MCP callers are still awaiting a completion this thread has
    /// to deliver.
    ///
    /// G-LV.1: every one of these needs a *future frame* — the compute drain,
    /// the fixpoint round handoff and the screenshot pump all live in
    /// `RsCamApp::update`. A non-zero count is therefore a standing reason to
    /// keep repainting, and (published through [`FrameLoopBeat`]) the only
    /// evidence available off-thread that a parked loop is stranding work.
    pub fn awaiting_gui(&self) -> u64 {
        let count = self.toolpath.len()
            + usize::from(self.simulation.is_some())
            + usize::from(self.collision.is_some())
            + usize::from(self.generate_all.is_some())
            + usize::from(self.gui_screenshot.is_some());
        count as u64
    }

    /// Whether a `generate_all` is among them.
    pub fn awaiting_generate_all(&self) -> bool {
        self.generate_all.is_some()
    }
}

/// State for a pending full-window GUI screenshot (`screenshot_gui`).
pub struct PendingGuiScreenshot {
    /// Output PNG path.
    pub path: String,
    /// Frames to wait before issuing `ViewportCommand::Screenshot`.
    /// Non-zero when the request resized the window first, so the
    /// resize has settled by the time the backend captures. 0 = issue
    /// on the next pump.
    pub frames_before_capture: u8,
    /// Set once the `ViewportCommand::Screenshot` has been sent; the
    /// next `egui::Event::Screenshot` in raw input completes this slot.
    pub capture_requested: bool,
    pub response_tx: tokio::sync::oneshot::Sender<McpResponse>,
}

impl PendingGuiScreenshot {
    /// Advance the per-frame countdown. Returns `true` exactly once —
    /// on the frame the `ViewportCommand::Screenshot` should be issued.
    pub fn should_capture_now(&mut self) -> bool {
        if self.capture_requested {
            return false;
        }
        if self.frames_before_capture > 0 {
            self.frames_before_capture -= 1;
            return false;
        }
        self.capture_requested = true;
        true
    }
}

/// State for tracking a "generate all" MCP request.
pub struct PendingGenerateAll {
    pub remaining: Vec<ToolpathId>,
    pub completed: usize,
    pub failed: usize,
    /// Per-toolpath error messages for genuinely failed generations.
    pub errors: Vec<(usize, String)>,
    /// A/M11 — ops that could not generate *yet* because their upstream
    /// simulated stock does not exist. Deliberately NOT counted in `failed`:
    /// "cannot yet" and "cannot ever" are different states, and only the
    /// former is worth retrying.
    pub blocked: Vec<(ToolpathId, String)>,
    /// A/M11 — the fixpoint loop's own state.
    pub fixpoint: FixpointPlan,
    /// Set when the loop itself failed (e.g. its simulation errored), as
    /// distinct from any individual toolpath failing.
    pub loop_error: Option<String>,
    pub response_tx: tokio::sync::oneshot::Sender<McpResponse>,
    /// Optional channel for streaming per-toolpath progress back to the MCP client.
    pub progress_tx: Option<tokio::sync::mpsc::Sender<ProgressUpdate>>,
}

/// A/M11 — `generate_all` iterating to a fixpoint over the rest-stock chain.
///
/// The ladder: generate everything, simulate, regenerate whatever was blocked
/// only on missing upstream stock, repeat. Before this, a chain of `k`
/// dependent rest ops needed `k` manual sim->generate rounds and nothing told
/// the operator what `k` was.
///
/// **Termination.** A round only continues when (a) at least one op is
/// blocked *purely* on sequencing and (b) the previous round generated at
/// least one new op. Genuine failures record `Error` and are never retried,
/// so they cannot keep (a) true. An op reaches `Done` at most once per call,
/// so (b) can hold at most `enabled_count` times. On top of that the loop is
/// hard-bounded by [`Self::max_rounds`] = the number of rest-dependent ops
/// plus one, because a stock chain cannot be longer than that.
pub struct FixpointPlan {
    /// `false` = the pre-A/M11 single pass. The caller can always opt out.
    pub enabled: bool,
    /// Simulation cell size for the loop's own simulations, in mm.
    ///
    /// **Caller-specified, never defaulted** (A/M10). A silently chosen
    /// resolution is the resolution-mismatch trap: collision counts and
    /// engagement change with cell size, so a loop that picked its own would
    /// hand back verdicts nobody asked for. `None` is only legal alongside
    /// `enabled: false`; otherwise the call refuses at request time.
    pub resolution_mm: Option<f64>,
    /// 1-based; the first generate pass is round 1.
    pub round: usize,
    pub max_rounds: usize,
    /// Ops that reached `Done` in the current round — condition (b).
    pub completed_this_round: usize,
    /// How many simulations the loop ran.
    pub simulations: usize,
    /// True between submitting the loop's simulation and its completion.
    pub awaiting_simulation: bool,
}

impl FixpointPlan {
    /// A plan that does exactly what `generate_all` did before A/M11.
    pub fn single_pass() -> Self {
        Self {
            enabled: false,
            resolution_mm: None,
            round: 1,
            max_rounds: 1,
            completed_this_round: 0,
            simulations: 0,
            awaiting_simulation: false,
        }
    }

    pub fn looping(resolution_mm: f64, rest_dependent_ops: usize) -> Self {
        Self {
            enabled: true,
            resolution_mm: Some(resolution_mm),
            round: 1,
            max_rounds: rest_dependent_ops.saturating_add(1),
            completed_this_round: 0,
            simulations: 0,
            awaiting_simulation: false,
        }
    }
}

impl PendingGenerateAll {
    /// Freeze this run into the shape the response builder consumes.
    pub fn completed_summary(&self) -> GenerateAllSummary {
        GenerateAllSummary {
            generated: self.completed,
            failed: self.failed,
            errors: self.errors.clone(),
            blocked: self
                .blocked
                .iter()
                .map(|(id, msg)| (id.0, msg.clone()))
                .collect(),
            rounds: self.fixpoint.round,
            simulations: self.fixpoint.simulations,
            loop_error: self.loop_error.clone(),
        }
    }
}

/// The outcome of one `generate_all` call, ready to render.
pub struct GenerateAllSummary {
    pub generated: usize,
    pub failed: usize,
    /// `(toolpath id, message)` for genuine failures.
    pub errors: Vec<(usize, String)>,
    /// A/M11 — `(toolpath id, message)` for ops still waiting on upstream
    /// simulated stock. Separate from `errors` on purpose: an agent must be
    /// able to tell "cannot yet" from "cannot ever" without parsing prose.
    pub blocked: Vec<(usize, String)>,
    /// How many internal generate rounds it took. 1 = no ladder was needed.
    pub rounds: usize,
    /// How many simulations the loop ran on the caller's behalf.
    pub simulations: usize,
    /// The loop itself failed (not an individual toolpath).
    pub loop_error: Option<String>,
}

/// Render the `generate_all` reply.
///
/// A/M11: reports blocked ops on their own channel, and never as failures.
pub fn build_generate_all_response(summary: &GenerateAllSummary) -> String {
    let mut headline = format!("Generated {} toolpaths", summary.generated);
    if summary.failed > 0 {
        headline.push_str(&format!(", {} failed", summary.failed));
    }
    if !summary.blocked.is_empty() {
        headline.push_str(&format!(
            ", {} still waiting on upstream simulated stock",
            summary.blocked.len()
        ));
    }
    headline.push_str(&format!(
        " (in {} generate round{}, {} simulation{})",
        summary.rounds,
        if summary.rounds == 1 { "" } else { "s" },
        summary.simulations,
        if summary.simulations == 1 { "" } else { "s" },
    ));
    if let Some(err) = &summary.loop_error {
        headline.push_str(&format!(". The fixpoint loop stopped early: {err}"));
    }

    let render = |rows: &[(usize, String)]| -> Vec<serde_json::Value> {
        rows.iter()
            .map(|(id, message)| serde_json::json!({"toolpath_id": id, "message": message}))
            .collect()
    };

    rs_cam_mcp::server::json_str(serde_json::json!({
        "ok": summary.failed == 0 && summary.loop_error.is_none(),
        "summary": headline,
        "generated": summary.generated,
        "failed": summary.failed,
        "errors": render(&summary.errors),
        "awaiting_prior_stock": render(&summary.blocked),
        "rounds": summary.rounds,
        "simulations": summary.simulations,
        "loop_error": summary.loop_error,
    }))
}

impl PendingMcpCompute {
    pub fn new() -> Self {
        Self::default()
    }
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
    use crate::compute::{CancelOutcome, ComputeLane, LaneSnapshot, LaneState};

    fn busy_outcome() -> CancelOutcome {
        CancelOutcome {
            was_busy: true,
            snapshot: LaneSnapshot {
                lane: ComputeLane::Toolpath,
                state: LaneState::Cancelling,
                queue_depth: 0,
                current_job: Some("Adaptive Rough (adaptive3d)".to_owned()),
                current_phase: Some("clear_z_level".to_owned()),
                started_at: Some(Instant::now()),
                active_toolpath_id: Some(7),
                active_toolpath_index: Some(3),
            },
        }
    }

    /// A frame loop that has just completed a frame with nothing pending —
    /// the healthy baseline every response is measured against.
    fn live_beat() -> FrameLoopBeat {
        let beat = FrameLoopBeat::default();
        beat.frame_begin();
        beat.beat(0, false);
        beat
    }

    /// A frame loop whose last frame ran `age` ago, holding `awaiting` MCP
    /// completions.
    ///
    /// Built field-wise rather than by `beat()`-then-sleep: `beat` timestamps
    /// itself against `origin`, so backdating the origin and then beating
    /// yields an age of zero — and a unit test should not spend
    /// [`PARKED_FRAME_LOOP`] proving a threshold.
    fn beat_last_ran(age: Duration, awaiting: u64, generate_all: bool) -> FrameLoopBeat {
        FrameLoopBeat {
            origin: Instant::now() - age,
            // The one frame that ran, ran at `origin`.
            last_frame_ms: AtomicU64::new(0),
            frames: AtomicU64::new(1),
            awaiting: AtomicU64::new(awaiting),
            awaiting_generate_all: AtomicBool::new(generate_all),
            ..FrameLoopBeat::default()
        }
    }

    /// G-LV.1's live condition, reconstructed: a frame loop that ran, was
    /// left holding a `generate_all`, and then stopped 268 s ago.
    fn parked_beat_with_generate_all() -> FrameLoopBeat {
        beat_last_ran(Duration::from_secs(268), 1, true)
    }

    /// The beat must separate the three states an outside observer would
    /// otherwise conflate: never ran, running now, ran a while ago.
    #[test]
    fn frame_beat_separates_never_ran_from_in_frame_from_parked() {
        let beat = FrameLoopBeat::default();
        assert!(beat.frame_age().is_none(), "no frame has run yet");
        assert_eq!(beat.frames(), 0);
        assert!(
            !beat.is_parked(),
            "a GUI nobody has asked anything of is idle, not parked"
        );

        beat.record_sent();
        assert!(
            beat.is_parked(),
            "once a request is in the channel, a loop that has never run IS parked"
        );
        assert_eq!(beat.backlog(), 1, "sent but not handled");

        beat.frame_begin();
        assert!(
            !beat.is_parked(),
            "a loop inside a frame is busy, not parked — however long it is in there"
        );
        assert!(beat.in_frame());
        beat.record_handled();
        beat.beat(0, false);

        assert_eq!(beat.frames(), 1);
        assert_eq!(beat.backlog(), 0);
        assert!(!beat.in_frame());
        assert!(!beat.is_parked());
        assert!(beat.frame_age().is_some_and(|d| d < PARKED_FRAME_LOOP));
    }

    /// The trap, pinned. `generation_status` answered "idle: no toolpath
    /// generation in flight" for 268 s while a `generate_all` sat waiting for
    /// a frame that never came — true, and read as "your call finished".
    #[test]
    fn generation_status_on_an_idle_lane_names_a_parked_frame_loop() {
        let resp = build_generation_status_response(
            &LaneSnapshot::idle(ComputeLane::Toolpath),
            &parked_beat_with_generate_all(),
        );
        let v: serde_json::Value = serde_json::from_str(&resp).unwrap();

        // The lane facts are unchanged — this adds a channel, it does not
        // relabel an idle lane as busy.
        assert_eq!(v["busy"], false);
        assert_eq!(v["lane_state"], "idle");

        assert_eq!(v["frame_loop"]["healthy"], false);
        assert_eq!(v["frame_loop"]["awaiting_generate_all"], true);
        assert!(v["frame_loop"]["last_frame_age_s"].as_f64().unwrap() > 200.0);

        let summary = v["summary"].as_str().unwrap();
        assert!(
            summary.contains("stranded"),
            "an idle lane with stranded work must say so, got: {summary}"
        );
        assert!(
            summary.contains("generate_all"),
            "and must name the call that is stranded, got: {summary}"
        );
        assert!(
            summary.contains("does NOT mean your call completed"),
            "the whole defect is that idle READS as done, got: {summary}"
        );
    }

    /// The warning is gated on stranded work, not on staleness. A GUI nobody
    /// is driving is idle; crying wolf there trains agents to skip the field.
    #[test]
    fn a_parked_frame_loop_with_nothing_waiting_is_not_a_warning() {
        let beat = beat_last_ran(Duration::from_secs(600), 0, false);

        let resp =
            build_generation_status_response(&LaneSnapshot::idle(ComputeLane::Toolpath), &beat);
        let v: serde_json::Value = serde_json::from_str(&resp).unwrap();
        assert_eq!(
            v["summary"], "idle: no toolpath generation in flight",
            "no stranded work means no warning: {resp}"
        );
        assert_eq!(
            v["frame_loop"]["healthy"], false,
            "the fact is still reported — only the alarm is withheld"
        );
    }

    /// "Nothing to cancel" is good news only if the frame loop is alive.
    #[test]
    fn cancel_on_a_parked_loop_does_not_read_as_all_clear() {
        let outcome = CancelOutcome {
            was_busy: false,
            snapshot: LaneSnapshot::idle(ComputeLane::Toolpath),
        };
        let resp = build_cancel_generation_response(&outcome, &parked_beat_with_generate_all());
        let v: serde_json::Value = serde_json::from_str(&resp).unwrap();
        assert_eq!(v["was_busy"], false);
        let summary = v["summary"].as_str().unwrap();
        assert!(summary.contains("nothing to cancel"), "got: {summary}");
        assert!(
            summary.contains("stranded"),
            "a no-op cancel on a parked loop must not read as all-clear: {summary}"
        );
    }

    /// The count the beat publishes has to come from the pending struct, not
    /// from the channel: a `generate_all` is dispatched once and then waits
    /// for frames it never queued a request for.
    #[test]
    fn awaiting_gui_counts_deferred_completions() {
        let mut pending = PendingMcpCompute::new();
        assert_eq!(pending.awaiting_gui(), 0);
        assert!(!pending.awaiting_generate_all());

        let (tx, _rx) = tokio::sync::oneshot::channel();
        pending.simulation = Some(tx);
        assert_eq!(pending.awaiting_gui(), 1);
        assert!(!pending.awaiting_generate_all());

        let (tx, _rx) = tokio::sync::oneshot::channel();
        pending.generate_all = Some(PendingGenerateAll {
            remaining: Vec::new(),
            completed: 0,
            failed: 0,
            errors: Vec::new(),
            blocked: Vec::new(),
            fixpoint: FixpointPlan::single_pass(),
            loop_error: None,
            response_tx: tx,
            progress_tx: None,
        });
        assert_eq!(pending.awaiting_gui(), 2);
        assert!(pending.awaiting_generate_all());
    }

    /// MCP `cancel_generation` on an idle toolpath lane must report a
    /// no-op (`was_busy: false`) rather than claiming it cancelled
    /// something that was never running.
    #[test]
    fn cancel_generation_response_idle_lane_is_no_op() {
        let outcome = CancelOutcome {
            was_busy: false,
            snapshot: LaneSnapshot::idle(ComputeLane::Toolpath),
        };
        let resp = build_cancel_generation_response(&outcome, &live_beat());
        let v: serde_json::Value = serde_json::from_str(&resp).unwrap();
        assert_eq!(v["ok"], true);
        assert_eq!(v["was_busy"], false);
        assert!(
            v["summary"].as_str().unwrap().contains("nothing to cancel"),
            "idle-lane summary should read as a no-op, got: {resp}"
        );
    }

    /// A busy toolpath lane must report `was_busy: true`, name the in-flight
    /// job, and — A/M12 — carry the index/stage the caller needs to attribute
    /// the cost to an operation.
    #[test]
    fn cancel_generation_response_busy_lane_names_the_job() {
        let resp = build_cancel_generation_response(&busy_outcome(), &live_beat());
        let v: serde_json::Value = serde_json::from_str(&resp).unwrap();
        assert_eq!(v["ok"], true);
        assert_eq!(v["was_busy"], true);
        assert_eq!(v["toolpath_index"], 3);
        assert_eq!(v["stage"], "clear_z_level");
        assert!(
            v["summary"]
                .as_str()
                .unwrap()
                .contains("Adaptive Rough (adaptive3d)"),
            "busy-lane summary should name the in-flight job, got: {resp}"
        );
    }

    /// The read cache answers `None` before the GUI has ever published, and
    /// the exact payload afterwards. A caller must be able to tell "the GUI
    /// never spoke" from "the GUI spoke a while ago".
    #[test]
    fn read_cache_distinguishes_never_published_from_stale() {
        let cache = McpReadCache::new();
        assert!(cache.age().is_none());
        assert!(cache.get(McpReadKind::ListToolpaths).is_none());

        cache.publish(McpReadSnapshot {
            list_toolpaths: "[{\"index\":0}]".to_owned(),
            project_summary: "{\"name\":\"p\"}".to_owned(),
            ..Default::default()
        });

        let (payload, age) = cache.get(McpReadKind::ListToolpaths).unwrap();
        assert_eq!(payload, "[{\"index\":0}]");
        assert!(age.as_secs() < 5);
        assert_eq!(
            cache.get(McpReadKind::ProjectSummary).unwrap().0,
            "{\"name\":\"p\"}"
        );
    }

    fn pending(
        frames: u8,
    ) -> (
        PendingGuiScreenshot,
        tokio::sync::oneshot::Receiver<McpResponse>,
    ) {
        let (tx, rx) = tokio::sync::oneshot::channel();
        (
            PendingGuiScreenshot {
                path: "/tmp/test.png".to_owned(),
                frames_before_capture: frames,
                capture_requested: false,
                response_tx: tx,
            },
            rx,
        )
    }

    /// No-resize request: the Screenshot command fires on the first pump
    /// and never again.
    #[test]
    fn gui_screenshot_captures_immediately_without_resize() {
        let (mut p, _rx) = pending(0);
        assert!(p.should_capture_now(), "first pump must request capture");
        assert!(p.capture_requested);
        assert!(!p.should_capture_now(), "capture must be requested once");
        assert!(!p.should_capture_now());
    }

    /// Resize request: the countdown defers the Screenshot command so the
    /// InnerSize resize settles first, then fires exactly once.
    #[test]
    fn gui_screenshot_countdown_defers_capture_after_resize() {
        let (mut p, _rx) = pending(3);
        assert!(!p.should_capture_now());
        assert!(!p.should_capture_now());
        assert!(!p.should_capture_now());
        assert!(
            p.should_capture_now(),
            "capture fires after the countdown drains"
        );
        assert!(!p.should_capture_now(), "and only once");
    }

    /// The pending slot starts empty and `take()` empties it again —
    /// the per-frame scan relies on this to complete at most one
    /// response per Screenshot event.
    #[test]
    fn pending_compute_gui_screenshot_slot_take_semantics() {
        let mut pending_mcp = PendingMcpCompute::new();
        assert!(pending_mcp.gui_screenshot.is_none());

        let (slot, _rx) = pending(0);
        pending_mcp.gui_screenshot = Some(slot);
        let taken = pending_mcp.gui_screenshot.take();
        assert!(taken.is_some());
        assert!(pending_mcp.gui_screenshot.is_none());
    }
}
