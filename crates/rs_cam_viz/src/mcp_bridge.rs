//! Bridge types for communication between the embedded MCP server (tokio thread)
//! and the GUI main thread. The GUI owns all state; the MCP server sends requests
//! and receives responses via channels.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use serde::Serialize;

pub use rs_cam_mcp::response::CutTraceCaps;

use rs_cam_core::session::CommandId;
use rs_cam_mcp::server::json_str;

use crate::state::Workspace;
use crate::state::toolpath::ToolpathId;
use crate::ui_command::{UiCommand, UiQuery};

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

pub use crate::GuiWaker;

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
    /// Off-frame pumps — dispatches that ran from the event loop's
    /// `about_to_wait` rather than from a paint.
    ///
    /// **Deliberately NOT counted as frames.** Once dispatch comes off the
    /// paint path, "the loop is answering" and "the window is rendering" stop
    /// being the same statement, and this type's whole job is to report the
    /// second one honestly. Folding pumps into `frames` would make
    /// [`Self::is_parked`] read `false` on a window that is not drawing a
    /// single pixel — which is exactly the false reassurance G-LV.1 was
    /// about, rebuilt one layer up.
    pumps: AtomicU64,
    /// Milliseconds since `origin` at the last [`FrameLoopBeat::pump_beat`].
    last_pump_ms: AtomicU64,
    /// Event-loop wakeups **sent** by the MCP thread through the [`GuiWaker`].
    ///
    /// Counted separately from `sent` so the two halves of the wakeup path can
    /// be told apart from outside the process: `wakeups` climbing while
    /// `pumps` does not means the ping was issued and the loop did not answer
    /// it, which is a different bug from the ping never being issued. Without
    /// this the two are indistinguishable, and B-4 spent a measurement round
    /// unable to tell them apart.
    wakeups: AtomicU64,
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
            pumps: AtomicU64::new(0),
            last_pump_ms: AtomicU64::new(0),
            wakeups: AtomicU64::new(0),
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

    /// Called by the GUI main thread after an **off-frame** dispatch, with
    /// what that dispatch left outstanding.
    ///
    /// Refreshes the same `awaiting` counters [`Self::beat`] does — they must
    /// not go stale just because no paint happened — but touches neither the
    /// frame count, the frame timestamp, nor the `in_frame` flag. See
    /// [`Self::pumps`] for why that separation is the point.
    pub fn pump_beat(&self, awaiting: u64, awaiting_generate_all: bool) {
        let ms = self.now_ms();
        self.awaiting.store(awaiting, Ordering::Relaxed);
        self.awaiting_generate_all
            .store(awaiting_generate_all, Ordering::Relaxed);
        self.last_pump_ms.store(ms, Ordering::Relaxed);
        self.pumps.fetch_add(1, Ordering::Release);
    }

    /// Dispatches that ran off the paint path since startup.
    pub fn pumps(&self) -> u64 {
        self.pumps.load(Ordering::Acquire)
    }

    /// Time since the last off-frame dispatch, or `None` if there has never
    /// been one.
    ///
    /// A large value is **not** a fault: the event loop only wakes when
    /// something asks it to, so an idle session with nothing pending pumps
    /// rarely and correctly. Reported as a fact, with no health verdict
    /// attached, because there is no threshold that would separate "dispatch
    /// is broken" from "nobody has called anything".
    pub(crate) fn pump_age(&self) -> Option<Duration> {
        if self.pumps.load(Ordering::Acquire) == 0 {
            return None;
        }
        let ms = self.last_pump_ms.load(Ordering::Relaxed);
        Some(
            self.origin
                .elapsed()
                .saturating_sub(Duration::from_millis(ms)),
        )
    }

    /// Called by the MCP server thread after it pings the event loop.
    pub fn record_wakeup(&self) {
        self.wakeups.fetch_add(1, Ordering::Relaxed);
    }

    /// Event-loop wakeups sent since startup.
    pub fn wakeups(&self) -> u64 {
        self.wakeups.load(Ordering::Relaxed)
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
    pub(crate) fn current_frame_age(&self) -> Option<Duration> {
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
            // Dispatches that ran WITHOUT a paint. `healthy` above is still
            // about pixels: a window can be parked (healthy false) and
            // answering (pumps climbing) at the same time, and both halves
            // matter to a caller deciding whether to trust a screenshot.
            "pumps": self.pumps(),
            "wakeups": self.wakeups(),
            "last_pump_age_s": self.pump_age().map(|d| d.as_secs_f64()),
            "requests_in_channel": self.backlog(),
            "awaiting_completion": self.awaiting(),
            "awaiting_generate_all": self.awaiting_generate_all(),
            // Checkpoint O-2: the negotiated present mode travels with the
            // liveness block because it is the *cause* half of the same
            // question. `healthy: false` says the loop is not painting;
            // `present_mode.negotiated` says whether the mode that strands it
            // is in force. A silent Fifo fallback under AutoNoVsync restores
            // the G-LV.1 hazard, and without this field it is invisible.
            "present_mode": crate::present_mode::report(),
        })
    }
}

/// Where the generation plan has got to, shared with the MCP server thread.
///
/// `generation_status` is answered on the server thread, off the frame loop,
/// so a controller field is unreachable there. This is a second cell beside
/// [`FrameLoopBeat`] on the same struct, for the same reason and with no rate
/// limit: a plan step changes faster than the 500 ms read snapshot, and the
/// snapshot stops when the frame loop parks, which is the exact failure
/// `generation_status` exists to see through.
///
/// The GUI thread stamps it on every cursor move and once more, with `None`,
/// when the plan ends. A reader that sees `None` therefore knows no plan
/// runs, rather than that the stamp went stale.
#[derive(Debug, Default)]
pub struct PlanBeat {
    cell: RwLock<Option<PlanBeatRow>>,
}

/// One plan position, as the wire renders it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlanBeatRow {
    /// 1-based position of the step in flight.
    pub step: usize,
    pub of: usize,
    /// Is that step a simulation rather than a generation?
    pub simulating: bool,
}

impl PlanBeat {
    /// Called by the GUI main thread. `None` clears the cell.
    pub fn stamp(&self, row: Option<PlanBeatRow>) {
        let mut guard = self.cell.write().unwrap_or_else(|e| e.into_inner());
        *guard = row;
    }

    /// Called by the MCP server thread.
    #[must_use]
    pub fn read(&self) -> Option<PlanBeatRow> {
        *self.cell.read().unwrap_or_else(|e| e.into_inner())
    }
}

/// Shared handle onto [`McpReadSnapshot`], the [`FrameLoopBeat`] and the
/// [`PlanBeat`]. Cloned into the MCP server thread.
///
/// The two travel together because they answer the same question from
/// opposite sides: the snapshot is what the frame loop last *said*, the beat
/// is when it last *ran* and what it left undone.
#[derive(Clone, Default)]
pub struct McpReadCache {
    snapshot: Arc<RwLock<McpReadSnapshot>>,
    frame_loop: Arc<FrameLoopBeat>,
    plan: Arc<PlanBeat>,
}

impl McpReadCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Frame-loop liveness, shared with the GUI thread.
    pub fn frame_loop(&self) -> &FrameLoopBeat {
        &self.frame_loop
    }

    /// Where the generation plan has got to, shared with the GUI thread.
    pub fn plan(&self) -> &PlanBeat {
        &self.plan
    }

    /// The handle the controller stamps. Taken once, on the GUI thread.
    #[must_use]
    pub fn plan_handle(&self) -> Arc<PlanBeat> {
        Arc::clone(&self.plan)
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
    ListSetups,
    GetToolpathParams {
        index: usize,
    },
    GetOperationSchema {
        operation_type: String,
    },
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
    //
    // WP4: every MCP mutation that is a registry `Command` row travels in
    // `Core` and is applied through `ProjectSession::apply`. Only the
    // compositions stay here as variants of their own — see
    // `CoreRequest` for why the wire struct crosses the channel rather
    // than the core payload.
    /// One command row, as the wire sent it. The GUI thread converts,
    /// applies and describes it in one arm.
    Core(CoreRequest),
    LoadProject {
        path: String,
        /// G-OPENGUARD: throw away the open project's unsaved changes.
        discard_unsaved: bool,
    },
    ExportGcode {
        path: String,
        accept_unmodeled_tool_load: bool,
        accept_exceeded_tool_load: bool,
        tool_change_mode: Option<String>,
        split_setups: bool,
        /// G-STALEXPORT: emit an edited operation's previous geometry
        /// rather than refusing. See `ExportParam::accept_previous_geometry`.
        accept_previous_geometry: bool,
    },
    /// F3.1 — add a toolpath through `handle_add_toolpath`, the same
    /// `AppEvent::AddToolpath` the Add menu emits, so the GUI's own
    /// tool/model binding and its add-time refusal path run.
    AddToolpathViaGui {
        operation_type: String,
        setup_index: Option<usize>,
    },
    /// Phase O — plan a multi-tool island finishing chain. Emits k enabled
    /// `unified_finish` ops, coarse to fine, each carrying its tier's islands
    /// and a `planner_origin` provenance stamp. Cheap: no tier map is built
    /// here, so it is answered on the frame loop like any other mutation.
    PlanMultitoolFinishing {
        spec: rs_cam_mcp::server::PlanMultitoolFinishingParam,
    },
    /// Phase U — the look-before-emit twin of
    /// [`Self::PlanMultitoolFinishing`]: build the tier map and its islands,
    /// report the territory, and modify NOTHING. Answered on the frame loop
    /// like the planner, but unlike it this one does real work — a full-grid
    /// residual walk, ~8 s at 0.6 mm and ~31 s at 0.3 mm on a 200 mm board,
    /// less on a cache hit.
    PreviewTierMap {
        spec: rs_cam_mcp::server::PreviewTierMapParam,
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
    /// Generate every enabled toolpath, simulating between the steps that
    /// need it. See [`GenerationPlan`].
    GenerateAll {
        /// `None` = the plan simulates where it must (the default).
        /// `Some(false)` = generate only, and run no simulation at all.
        fixpoint: Option<bool>,
        /// Cell size in mm for the plan's own simulations. Required whenever
        /// the plan may simulate AND the project has rest-machining ops;
        /// never defaulted (A/M10).
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

    // ── Measurements over the model ──────────────────────────────────
    /// P5 — the per-tool reach map for one finishing toolpath, as
    /// numbers: an area-weighted unreachable percentage, the worst gap, a
    /// gap histogram and the grid the answer was taken on. Modifies
    /// nothing. A cold map is a full-grid drop-cutter walk, a second or
    /// two on a board-sized terrain, free once the memo is warm.
    ReachMap {
        spec: rs_cam_mcp::server::ReachMapParam,
    },

    // ── View commands and view reads (WP13) ──────────────────────────
    //
    // Scrubbing, screenshots and UI navigation wrote the VIEW and never
    // `ProjectSession`, and five reads answered from the view's own
    // simulation slot or from the per-user library files. All eighteen
    // are rows of the view registry now
    // (`crates/rs_cam_viz/src/ui_command.rs`), and they travel in these
    // two wrappers rather than as variants of their own.
    /// One view command, as the wire sent it.
    Ui(UiCommand),
    /// One view read, as the wire sent it.
    UiQuery(UiQuery),
}

/// Declare the MCP mutation surface from one table.
///
/// **SHL-02.** Adding one MCP command used to touch nine hand-written
/// sites in five files. Four of them were this enum, its `CommandId`
/// map, the GUI's workspace switch and the completeness sentry's own
/// list of every variant — four restatements of one fact. They are one
/// table row now, and the enum cannot gain a variant without one,
/// because the enum IS the table.
///
/// Each row reads: the variant, the wire parameter struct it carries,
/// the registry row it runs, and the workspace the GUI shows before it
/// runs. `None` means the GUI stays where it is.
///
/// The four questions that are NOT here — which `Command` to build,
/// which toast to raise, which reply shape to answer with, and which
/// refusal sentence to write — stay as exhaustive matches in
/// `app/mcp/commands.rs`. Each carries a body, not a value, and a
/// closure column would read worse than the arm it replaced.
macro_rules! declare_core_requests {
    (
        $(#[$enum_meta:meta])*
        $name:ident {
            $(
                $(#[$meta:meta])*
                $variant:ident($param:ty) => $row:ident, $workspace:expr;
            )+
        }
    ) => {
        $(#[$enum_meta])*
        pub enum $name {
            $(
                $(#[$meta])*
                $variant($param),
            )+
        }

        impl $name {
            /// The registry row this request runs.
            ///
            /// The describe step matches on this, so a row that loses its
            /// arm does not compile.
            pub fn id(&self) -> CommandId {
                match self {
                    $( Self::$variant(_) => CommandId::$row, )+
                }
            }

            /// The workspace the GUI shows before this request runs.
            ///
            /// An MCP mutation moves the operator's view to the surface it
            /// changes, so a watching human sees the edit land.
            pub fn workspace_after(&self) -> Option<Workspace> {
                match self {
                    $( Self::$variant(_) => $workspace, )+
                }
            }

            /// One default-built request per variant, in table order.
            ///
            /// The completeness sentry
            /// (`tests/mcp_core_arm_describes_every_row.rs`) walks this
            /// instead of a hand-written list. A new row reaches the sentry
            /// on its own.
            pub fn all_defaults() -> Vec<Self> {
                vec![ $( Self::$variant(Default::default()), )+ ]
            }
        }
    };
}

declare_core_requests! {
    /// One MCP mutation, as the wire sent it (WP4).
    ///
    /// Every variant names a `Command` row in
    /// [`rs_cam_core::session::Command`] and carries the `rs_cam_mcp`
    /// parameter struct the tool declared. The GUI thread turns the
    /// parameter struct into the row's core `*Args` payload and applies it
    /// through [`rs_cam_core::session::ProjectSession::apply`], the one
    /// mutation door.
    ///
    /// **Why the wire struct and not the core payload.** Twenty of these
    /// conversions read the session — the heights patch reads the current
    /// heights, the stock patch reads the current stock, the spindle policy
    /// reads the current post block, the library rows read a catalog, the
    /// import reads a file — and the MCP server thread holds no session. A
    /// conversion on that thread would also report a refusal in a different
    /// ORDER from the one the operator reads today, because every handler
    /// validates the index before it parses the value. So the wire struct
    /// crosses the channel and the conversion runs beside the session, in
    /// `RsCamApp::core_command_for`.
    ///
    /// Keeping the `rs_cam_mcp` structs where they are is also what holds the
    /// WP2a wire snapshot still: a schema title is the struct's own name.
    CoreRequest {
        AddAlignmentPin(rs_cam_mcp::server::AddAlignmentPinParam)
            => AddAlignmentPin, Some(Workspace::Setup);
        RemoveAlignmentPin(rs_cam_mcp::server::RemoveAlignmentPinParam)
            => RemoveAlignmentPin, None;
        /// Row `AddModel`. The wire name is `import_model`: the surface reads
        /// the file, and core adopts the geometry that import produced.
        ImportModel(rs_cam_mcp::server::ImportModelParam)
            => AddModel, None;
        AddSetup(rs_cam_mcp::server::AddSetupParam)
            => AddSetup, None;
        SetSetupFace(rs_cam_mcp::server::SetSetupFaceParam)
            => SetSetupFace, Some(Workspace::Setup);
        SetSetupRotation(rs_cam_mcp::server::SetSetupRotationParam)
            => SetSetupRotation, Some(Workspace::Setup);
        MoveToolpathToSetup(rs_cam_mcp::server::MoveToolpathToSetupParam)
            => MoveToolpathToSetup, None;
        SaveProject(rs_cam_mcp::server::SaveProjectParam)
            => SaveProject, None;
        SetToolpathParam(rs_cam_mcp::server::SetToolpathParamInput)
            => SetToolpathParam, Some(Workspace::Toolpaths);
        SetToolParam(rs_cam_mcp::server::SetToolParamInput)
            => SetToolParam, None;
        SetToolpathTool(rs_cam_mcp::server::SetToolpathToolParam)
            => SetToolpathTool, Some(Workspace::Toolpaths);
        SetToolpathModel(rs_cam_mcp::server::SetToolpathModelParam)
            => SetToolpathModel, Some(Workspace::Toolpaths);
        SetToolpathHeights(rs_cam_mcp::server::SetToolpathHeightsParam)
            => SetToolpathHeights, None;
        AddToolpath(rs_cam_mcp::server::AddToolpathParam)
            => AddToolpath, Some(Workspace::Toolpaths);
        RemoveToolpath(rs_cam_mcp::server::RemoveToolpathParam)
            => RemoveToolpath, None;
        AddTool(rs_cam_mcp::server::AddToolParam)
            => AddTool, None;
        AddToolFromLibrary(rs_cam_mcp::server::AddToolFromLibraryParam)
            => AddToolFromLibrary, None;
        RemoveTool(rs_cam_mcp::server::RemoveToolParam)
            => RemoveTool, None;
        SetStockConfig(rs_cam_mcp::server::SetStockConfigParam)
            => SetStockConfig, Some(Workspace::Setup);
        SetStockSource(rs_cam_mcp::server::SetStockSourceParam)
            => SetStockSource, None;
        SetMachineKinematics(rs_cam_mcp::server::SetMachineKinematicsParam)
            => SetMachineKinematics, Some(Workspace::Setup);
        /// Row `ImportMachineSettings`. The GRBL parse stays on this surface.
        ImportMachineSettings(rs_cam_mcp::server::ImportMachineSettingsParam)
            => ImportMachineSettings, Some(Workspace::Setup);
        /// Row `SetMachine`. The library READ stays on this surface; the
        /// payload core receives is a whole machine profile.
        LoadMachineFromLibrary(rs_cam_mcp::server::LoadMachineFromLibraryParam)
            => SetMachine, Some(Workspace::Setup);
        /// Row `SetPostConfig`. The wire writes one field of the post block.
        SetSpindleStrategy(rs_cam_mcp::server::SetSpindleStrategyParam)
            => SetPostConfig, None;
        SetBoundaryConfig(rs_cam_mcp::server::SetBoundaryConfigParam)
            => SetBoundaryConfig, None;
        SetRestAnalysisConfig(rs_cam_mcp::server::SetRestAnalysisConfigParam)
            => SetRestAnalysisConfig, None;
        SetDressupConfig(rs_cam_mcp::server::SetDressupConfigParam)
            => SetDressupConfig, None;
        SetDressupField(rs_cam_mcp::server::SetDressupFieldParam)
            => SetDressupField, None;
        SetToolpathEnabled(rs_cam_mcp::server::SetToolpathEnabledParam)
            => SetToolpathEnabled, None;
    }
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
        "including a generate_all"
    } else {
        ""
    };
    Some(format!(
        "NOTE — the GUI {age} and {stranded} MCP request(s) are still outstanding {what}. \
         Since B-4 this is NOT the G-LV.1 hang: dispatch and generate_all round handoffs \
         are pumped from the winit event loop, which a compositor cannot park \
         (`frame_loop.pumps` counts those, `frames` counts paints). Outstanding work here \
         means work that is genuinely still running, so an idle lane plus a non-zero \
         count is worth one more poll before you conclude anything. What a non-painting \
         window really cannot do is render: `screenshot_gui` will refuse rather than \
         return a stale image, and set_ui_view / sim-scrub calls apply but report \
         `visible_on_next_frame: false`. To get pixels back, make the window visible, or \
         relaunch with WAYLAND_DISPLAY UNSET (winit then picks X11/XWayland, where \
         redraws are client-driven and never gated on the compositor). Do NOT set \
         WINIT_UNIX_BACKEND — winit removed that variable in 0.29 and it does nothing on \
         the 0.30 this build uses."
    ))
}

/// Render the `generation_status` reply from a live toolpath-lane snapshot.
///
/// A/M12: without this there is no way to attribute a long generation's cost
/// to an operation, and every "it's been 40 minutes" is unfalsifiable — the
/// only diagnosis available on 2026-07-30 was reading `/proc` thread
/// accounting from outside the process.
/// W5 item (d): `plan` reports where a Generate All has got to. It reads the
/// [`PlanBeat`] the GUI thread stamps, not the compute lane, because the plan
/// cursor lives on the GUI thread and a lane field would always read null. A
/// null `plan` means no plan runs.
pub fn build_generation_status_response(
    snapshot: &crate::compute::LaneSnapshot,
    frame_loop: &FrameLoopBeat,
    plan: Option<PlanBeatRow>,
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
        "plan": plan.map(|p| serde_json::json!({
            "step": p.step,
            "of": p.of,
            "simulating": p.simulating,
        })),
        "frame_loop": frame_loop.report(),
        "note": "Live: read straight off the compute lane on the MCP server thread, \
                 never through the GUI frame loop. `stage` is whatever the planner \
                 last reported — a null means the op reports no stages, NOT that it \
                 is doing nothing. `lane_state` describes the COMPUTE LANE only — \
                 read `frame_loop` before reading an idle lane as \"my call finished\". \
                 `plan` is the Generate All cursor, stamped by the GUI thread: a null \
                 means no plan runs.",
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

/// What a synchronous MCP mutation handler did, read from its reply
/// (G-MCPTOAST, UX-R03-003).
///
/// The GUI toast for an MCP request used to be pushed BEFORE the handler
/// ran, in the past tense, and nothing corrected it when the handler
/// refused: a Scallop on a flat end mill was refused by the Suggest door
/// and the screen still read "MCP: Added toolpath". The dispatch now
/// classifies the handler's reply with this type and pushes the toast from
/// it, through `AppController::push_mcp_outcome`.
///
/// Two reply shapes carry a refusal. Mutation handlers answer with
/// `mutation_error_json` (`"ok": false` plus a `summary`); the import and
/// library handlers answer with a bare top-level `"error"` string. Anything
/// else is a success. A reply that is not JSON at all is also a success —
/// the plain-text handlers (load / save) classify from their `Result`
/// through [`McpOutcome::from_result`] instead, never from the text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum McpOutcome {
    Succeeded,
    /// The handler's own refusal text, as it appears in the reply.
    Refused(String),
}

impl McpOutcome {
    /// Classify a JSON reply from a mutation, import or library handler.
    pub fn from_json_response(response: &str) -> Self {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(response) else {
            return Self::Succeeded;
        };
        if value.get("ok").and_then(serde_json::Value::as_bool) == Some(false) {
            let summary = value
                .get("summary")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("request refused")
                .to_owned();
            return Self::Refused(summary);
        }
        if let Some(error) = value.get("error").and_then(serde_json::Value::as_str) {
            return Self::Refused(error.to_owned());
        }
        Self::Succeeded
    }

    /// Classify from a handler's own `Result`, for handlers whose reply is
    /// plain text rather than JSON.
    pub fn from_result<T, E: std::fmt::Display>(result: &Result<T, E>) -> Self {
        match result {
            Ok(_) => Self::Succeeded,
            Err(e) => Self::Refused(e.to_string()),
        }
    }

    /// The toast for this outcome: `success_message` unchanged on success,
    /// `MCP: {refusal}` at Warning on a refusal.
    pub fn notification(&self, success_message: String) -> (String, crate::controller::Severity) {
        match self {
            Self::Succeeded => (success_message, crate::controller::Severity::Info),
            Self::Refused(refusal) => (
                format!("MCP: {refusal}"),
                crate::controller::Severity::Warning,
            ),
        }
    }
}

/// The refusal reply every synchronous MCP mutation handler returns. One
/// construction site, so [`McpOutcome::from_json_response`] and the
/// handlers cannot drift apart on the shape.
pub fn mutation_error_json(summary: &str, field: Option<&str>) -> String {
    let field_value = field
        .map(|f| serde_json::Value::String(f.to_owned()))
        .unwrap_or(serde_json::Value::Null);
    rs_cam_mcp::server::json_str(serde_json::json!({
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
/// One in-flight `Job` submit an MCP caller is waiting on (WP14a).
pub struct PendingMcpJob {
    /// Where the answer goes when the lane completes.
    pub sender: tokio::sync::oneshot::Sender<McpResponse>,
    /// What the reply has to say beyond the answer itself.
    pub render: McpJobRender,
}

/// The per-row reply context a `Job` answer needs.
///
/// The answer is the core's own type. Everything else the wire reply
/// carries — the index the caller named, the setup and model the preview
/// was taken in, the file it asked for — is the REQUEST's, so it is held
/// here from submit time rather than re-read at drain time off a session
/// the operator may have edited meanwhile.
pub enum McpJobRender {
    /// The `recommend_clearing_strategy` reply.
    StrategyRecommendation {
        /// The toolpath index the caller named.
        index: usize,
    },
    /// The `preview_tier_map` reply.
    TierMapPreview {
        /// The setup the ladder was previewed in.
        setup_index: usize,
        /// The model the tier map was measured against.
        model_id: usize,
        /// Where to write the island SVG, if the caller asked for one.
        /// The path was validated at submit time.
        svg_path: Option<String>,
    },
    /// The `optimize_toolpath` reply (WP14b).
    OptimizeOutcome {
        /// The toolpath index the caller named. Held from submit time so
        /// a refusal names the same index the caller used, whatever the
        /// operator did to the project meanwhile.
        index: usize,
    },
}

/// Render one `Job` answer as the wire reply its row publishes.
///
/// **The file write lives here, not in core.** `execute_preview_tier_map`
/// answers with a preview; writing an SVG is the requester's step, taken
/// after the answer arrives.
pub fn render_job_answer(
    render: &McpJobRender,
    answer: &Result<rs_cam_core::session::JobAnswer, crate::compute::ComputeError>,
) -> String {
    use rs_cam_core::session::JobAnswer;

    match (render, answer) {
        (
            McpJobRender::StrategyRecommendation { index },
            Ok(JobAnswer::RecommendClearingStrategy(rec)),
        ) => render_strategy_recommendation(*index, (**rec).as_ref()),
        (McpJobRender::StrategyRecommendation { .. }, Err(error)) => {
            json_str(serde_json::json!({ "error": error.to_string() }))
        }
        (
            McpJobRender::TierMapPreview {
                setup_index,
                model_id,
                svg_path,
            },
            Ok(JobAnswer::PreviewTierMap(preview)),
        ) => render_tier_map_preview(preview, *setup_index, *model_id, svg_path.as_deref()),
        (McpJobRender::TierMapPreview { .. }, Err(error)) => preview_error(&error.to_string()),
        (McpJobRender::OptimizeOutcome { index }, Ok(JobAnswer::OptimizeToolpath(outcome))) => {
            render_optimize_outcome(*index, outcome)
        }
        (McpJobRender::OptimizeOutcome { .. }, Err(error)) => {
            json_str(serde_json::json!({ "error": error.to_string() }))
        }
        // The lane answers the row it was handed, so a render context and
        // an answer that name different rows cannot meet here. The arm
        // REPORTS the mismatch rather than picking one of the two.
        (McpJobRender::StrategyRecommendation { .. }, Ok(other))
        | (McpJobRender::TierMapPreview { .. }, Ok(other))
        | (McpJobRender::OptimizeOutcome { .. }, Ok(other)) => json_str(serde_json::json!({
            "error": format!(
                "the reply context and the answer name different rows; the answer is the \
                 {} row's",
                other.id().wire_name()
            ),
        })),
    }
}

/// The `optimize_toolpath` reply.
///
/// The body is the serialized [`OptimizeOutcome`], exactly as the
/// synchronous arm published it before WP14b. `index` names the toolpath
/// on the serialization refusal alone, where the outcome itself is not
/// available to say which run failed.
fn render_optimize_outcome(
    index: usize,
    outcome: &rs_cam_core::tool_load::optimize::OptimizeOutcome,
) -> String {
    match serde_json::to_value(outcome) {
        Ok(v) => json_str(v),
        Err(e) => json_str(serde_json::json!({
            "error": format!("Failed to serialize optimize outcome for toolpath {index}: {e}")
        })),
    }
}

/// The `recommend_clearing_strategy` reply.
fn render_strategy_recommendation(
    index: usize,
    rec: Option<&rs_cam_core::machine::strategy_advisor::StrategyRecommendation>,
) -> String {
    let Some(rec) = rec else {
        return json_str(serde_json::json!({
            "error": format!(
                "Toolpath {index} is not an Adaptive3d op (or no candidate planned a usable path); the strategy advisor only applies to 3D adaptive roughing"
            ),
        }));
    };
    json_str(serde_json::json!({
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
    }))
}

/// The `preview_tier_map` refusal shape.
fn preview_error(message: &str) -> String {
    json_str(serde_json::json!({
        "ok": false,
        "modified": false,
        "error": format!("preview_tier_map: {message}"),
    }))
}

/// The `preview_tier_map` reply, and the SVG the caller asked for.
fn render_tier_map_preview(
    preview: &rs_cam_core::session::MultitoolPreview,
    setup_index: usize,
    model_id: usize,
    svg_path: Option<&str>,
) -> String {
    // Hoisted: the counter walks every label, and calling it per tier
    // would re-walk a 445 k-cell map once per rung.
    let cells_per_tier = preview.map.tier_cell_counts();
    let ladder: Vec<serde_json::Value> = (0..preview.map.tier_count)
        .map(|k| {
            serde_json::json!({
                "tier": k,
                "tool_id": preview.tool_ids.get(k),
                "tool_name": preview.tool_names.get(k),
                "cusp_radius_mm": preview.cusp_radii_mm.get(k),
                "map_cells": cells_per_tier.get(k),
            })
        })
        .collect();

    let per_tier: Vec<serde_json::Value> = preview
        .islands
        .per_tier
        .iter()
        .map(|set| {
            let k = usize::from(set.tier);
            serde_json::json!({
                "tier": set.tier,
                "tool_id": preview.tool_ids.get(k),
                "tool_name": preview.tool_names.get(k),
                "cusp_radius_mm": preview.cusp_radii_mm.get(k),
                "islands": set.islands,
                "raw_island_count": set.raw_island_count,
                "owned_area_mm2": set.owned_area_mm2,
                // G-OVERLAPFILL: what the tool SWEEPS, against what the
                // tier owns. The band reaches into the coarser tier by
                // design; the ratio is how far.
                "machining_area_mm2": set.machining_area_mm2,
                "machining_to_owned_ratio": set.machining_to_owned_ratio(),
                "overlap_mm": set.overlap_mm,
                "owned_hole_count": set.owned_hole_count,
                "machining_hole_count": set.machining_hole_count,
                "net_holes_closed_by_band": set.net_holes_closed_by_band(),
                "median_owned_hole_area_mm2": set.median_owned_hole_area_mm2,
                "cap": {
                    "acted": set.cap.acted(),
                    "total_before_cap": set.cap.islands_after_min_area,
                    "kept": set.cap.kept,
                    "final_close_radius_mm": set.cap.final_close_radius_mm,
                    // A raise WELDS a dendritic network into slabs, and
                    // `owned_area_mm2` then measures the slabs: 2 261 ->
                    // 17 812 mm2 on wanaka at tolerance 0.146.
                    "close_raises": set.cap.close_raises,
                    "first_close_radius_mm": set.cap.first_close_radius_mm,
                },
            })
        })
        .collect();

    // Only the tiers over the bound, rendered. An empty list is the
    // healthy reading, not a missing measurement.
    let band_advisories: Vec<String> = preview
        .islands
        .band_advisories()
        .map(|a| a.to_string())
        .collect();

    let svg_written = match svg_path {
        None => None,
        Some(path) => {
            let svg = rs_cam_core::maps::tier_islands::tier_islands_to_svg(
                &preview.map,
                &preview.islands,
            );
            if let Err(e) = std::fs::write(path, &svg) {
                return preview_error(&format!("could not write {path}: {e}"));
            }
            Some(path.to_owned())
        }
    };

    json_str(serde_json::json!({
        "ok": true,
        // Plan-time preview: the job writes no session state.
        "modified": false,
        "setup_index": setup_index,
        "model_id": model_id,
        "ladder": ladder,
        "per_tier": per_tier,
        "band_advisories": band_advisories,
        "total_owned_area_mm2": preview.islands.total_owned_area_mm2(),
        "total_machining_area_mm2": preview.islands.total_machining_area_mm2(),
        "map": {
            "cell_mm": preview.map.grid.cell_mm,
            "nx": preview.map.grid.nx,
            "ny": preview.map.grid.ny,
            "tier_count": preview.map.tier_count,
            "unassigned_cells": preview.map.unassigned_cells(),
        },
        "svg_written": svg_written,
        "note": "Plan-time preview — nothing was generated and the project was not \
                 modified. `plan_multitool_finishing` emits the op chain; `generate_all` \
                 runs it.",
    }))
}

#[derive(Default)]
pub struct PendingMcpCompute {
    /// Toolpath ID -> oneshot sender for when that toolpath finishes.
    pub toolpath: HashMap<ToolpathId, tokio::sync::oneshot::Sender<McpResponse>>,
    /// WP14a — the `Job` lane's in-flight submits, keyed by the submit id.
    ///
    /// A map and not an `Option`, because two jobs can be in flight at
    /// once: a `preview_tier_map` at one dial set and another at a second
    /// are two questions with two callers, and one slot would drop one of
    /// them.
    pub jobs: HashMap<crate::compute::JobRequestId, PendingMcpJob>,
    /// Oneshot sender for when the simulation finishes.
    pub simulation: Option<tokio::sync::oneshot::Sender<McpResponse>>,
    /// Oneshot sender for when collision check finishes.
    pub collision: Option<tokio::sync::oneshot::Sender<McpResponse>>,
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
    ///
    /// The `generate_all` ladder is **not** in this count: Phase O moved it to
    /// `AppController`, because a GUI-started ladder owes frames without any
    /// MCP slot existing. Read it through
    /// `AppController::awaiting_deferred_completions`, which sums both.
    pub fn awaiting_gui(&self) -> u64 {
        let count = self.toolpath.len()
            + self.jobs.len()
            + usize::from(self.simulation.is_some())
            + usize::from(self.collision.is_some())
            + usize::from(self.gui_screenshot.is_some());
        count as u64
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
    /// When the request was accepted. The refusal in
    /// [`Self::park_refusal`] is measured from here, not from the last
    /// frame, so an idle-but-healthy window is never refused: the request
    /// itself asks for a repaint, and a window that can paint paints.
    pub requested_at: Instant,
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

    /// Checkpoint M-4: **refuse, with the mechanism named** — the one place the
    /// architecture is deliberately less capable than a wish.
    ///
    /// `screenshot_gui` is the single MCP tool that genuinely needs a *rendered
    /// frame*: it sends `ViewportCommand::Screenshot` and the pixels arrive as
    /// an `egui::Event::Screenshot` one to two frames later. Everything else in
    /// the tool surface — including `screenshot_simulation` and
    /// `screenshot_toolpath`, which are CPU rasterisers — answers from the
    /// off-frame pump. So on a window that is not rendering there are exactly
    /// three options, and two are wrong: **blocking** is what it did (the slot
    /// is armed and the capture never fires, so the call hangs until the
    /// caller's timeout), and **waking to force a render** risks a hung main
    /// thread under a present-blocked park. This returns the third.
    ///
    /// **`Some(reason)` only when both conditions hold**, and the second is why
    /// this is not merely `is_parked()`:
    ///
    /// 1. the frame loop is parked — no frame for [`PARKED_FRAME_LOOP`]; and
    /// 2. at least that long has passed **since this request arrived**.
    ///
    /// Without (2) an idle session would refuse spuriously. An idle GUI with no
    /// repaint driver has an old last-frame by definition — that is what idle
    /// *is* — and `mcp_screenshot_gui` calls `request_repaint()` when it stores
    /// the slot, so a healthy window produces a frame in milliseconds. The
    /// grace period separates "has not painted recently" from "cannot paint".
    ///
    /// **What this does NOT cover, stated because it is the failure mode that
    /// started all of this.** Under a *present-blocked* park — the FIFO
    /// mechanism of G-LV.1 — the main thread is blocked below winit and
    /// `about_to_wait` does not run, so nothing calls this and the refusal
    /// cannot be delivered either. It covers a window that has stopped painting
    /// while its event loop still runs, which is what a minimised X11 window
    /// does (N-2 measured `frames` static at 252 with `pumps` climbing
    /// 255→466). The present-blocked case is addressed by not being in it:
    /// Checkpoint O-1's `--mcp` flip to `AutoNoVsync`.
    pub fn park_refusal(&self, frame_loop: &FrameLoopBeat) -> Option<String> {
        if !frame_loop.is_parked() || self.requested_at.elapsed() < PARKED_FRAME_LOOP {
            return None;
        }
        let age = frame_loop.frame_age().map_or_else(
            || "never".to_owned(),
            |d| format!("{:.1}s ago", d.as_secs_f64()),
        );
        Some(format!(
            "screenshot_gui REFUSED: this window is not rendering, so there is no frame to \
             capture. A GUI screenshot is the one MCP call that needs a real rendered frame — \
             it is taken by the render backend, not by a rasteriser. Last frame {age}; \
             frame_loop.healthy is false and has been for at least {}s. This is not a timeout \
             and retrying will not help. Mechanism: on Wayland a hidden, occluded or \
             screen-locked surface receives no compositor frame callbacks, and under the FIFO \
             present family the main thread can block inside the present itself. Remedies, in \
             order: make the window visible; or relaunch with WAYLAND_DISPLAY unset so winit \
             picks X11/XWayland; and check frame_loop.present_mode.negotiated — if it reads \
             Fifo or FifoRelaxed the park hazard is in force. screenshot_simulation and \
             screenshot_toolpath are CPU rasterisers and still work.",
            PARKED_FRAME_LOOP.as_secs()
        ))
    }
}

// `GenerationPlan` / `GenerateAllSummary` live in `controller::generate_all`
// so the GUI and the MCP tool walk one plan. Re-exported here because every
// existing `use crate::mcp_bridge::...` site names them from this module.
pub use crate::controller::generate_all::{
    BlockedRow, GenerateAllSink, GenerateAllSummary, GenerationPlan, generate_all_headline,
};

/// Render the `generate_all` reply.
///
/// A/M11: reports blocked ops on their own channel, and never as failures.
pub fn build_generate_all_response(summary: &GenerateAllSummary) -> String {
    // Phase O: shared with the GUI ladder's toast, so the two surfaces cannot
    // drift into telling the operator different stories about one run.
    let headline = generate_all_headline(summary);

    let errors: Vec<serde_json::Value> = summary
        .errors
        .iter()
        .map(|(id, message)| serde_json::json!({"toolpath_id": id, "message": message}))
        .collect();

    rs_cam_mcp::server::json_str(serde_json::json!({
        "ok": summary.failed == 0 && summary.loop_error.is_none(),
        "summary": headline,
        "generated": summary.generated,
        "failed": summary.failed,
        "errors": errors,
        // W1 tail: the array rows are `BlockedRow`, the one shape every
        // `awaiting_prior_stock` array carries (D4). They gain
        // `toolpath_index`, `name`, `blocking_toolpath_id` and
        // `blocking_toolpath_index` over the old `{toolpath_id, message}`
        // pair.
        "awaiting_prior_stock": summary.blocked,
        // W1 wire break: `rounds` is gone. The fixpoint had rounds; the plan
        // has steps, and `steps` is how many it held.
        "steps": summary.steps,
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
            None,
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
            summary.contains("outstanding"),
            "an idle lane with work still owed must say so, got: {summary}"
        );
        assert!(
            summary.contains("generate_all"),
            "and must name the call it is owed to, got: {summary}"
        );
        // B-4 inverted the second half of this contract in place. Before the
        // decoupling the text asserted the *cause* — "every MCP request and
        // every generate_all round handoff is dispatched from a GUI repaint",
        // "does NOT mean your call completed" — and that cause is now false:
        // dispatch is pumped from the event loop, which no compositor parks.
        // Keeping the old sentence would have been a lie an agent then acts
        // on, so what is pinned now is the surviving true half.
        assert!(
            !summary.contains("dispatched from a GUI repaint"),
            "dispatch is no longer frame-coupled; this text must not claim it is: {summary}"
        );
        assert!(
            summary.contains("screenshot_gui"),
            "what a non-painting window actually costs is pixels, and the reply \
             must name the call that refuses because of it, got: {summary}"
        );
    }

    /// The warning is gated on stranded work, not on staleness. A GUI nobody
    /// is driving is idle; crying wolf there trains agents to skip the field.
    #[test]
    fn a_parked_frame_loop_with_nothing_waiting_is_not_a_warning() {
        let beat = beat_last_ran(Duration::from_secs(600), 0, false);

        let resp = build_generation_status_response(
            &LaneSnapshot::idle(ComputeLane::Toolpath),
            &beat,
            None,
        );
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
        // Pre-B-4 this asserted `contains("stranded")`. The word went with the
        // claim underneath it; the property it was protecting — that a no-op
        // cancel with work still owed must not read as all-clear — did not.
        assert!(
            summary.contains("outstanding"),
            "a no-op cancel with work still owed must not read as all-clear: {summary}"
        );
    }

    /// The count the beat publishes has to come from the pending struct, not
    /// from the channel: work is dispatched once and then waits for frames it
    /// never queued a request for.
    ///
    /// Phase O moved the `generate_all` term to
    /// `AppController::awaiting_deferred_completions` (a GUI-started ladder
    /// owes frames with no MCP slot in existence); that half is asserted in
    /// `tests/generate_all_fixpoint_parity.rs`.
    #[test]
    fn awaiting_gui_counts_deferred_completions() {
        let mut pending = PendingMcpCompute::new();
        assert_eq!(pending.awaiting_gui(), 0);

        let (tx, _rx) = tokio::sync::oneshot::channel();
        pending.simulation = Some(tx);
        assert_eq!(pending.awaiting_gui(), 1);

        let (tx, _rx) = tokio::sync::oneshot::channel();
        pending.collision = Some(tx);
        assert_eq!(pending.awaiting_gui(), 2);
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
                requested_at: Instant::now(),
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

    /// Checkpoint M-4, the refusal — and the two states it must tell apart.
    ///
    /// **The pre-fix behaviour this inverts:** `pump_mcp_gui_screenshot` armed
    /// the capture and requested a repaint whatever the frame loop was doing.
    /// On a window that had stopped painting the `ViewportCommand::Screenshot`
    /// either never issued or never came back, and the call hung until the
    /// caller gave up — N-2 measured that as a 60 s timeout with 0 bytes on
    /// disk. There was no code path that could answer.
    #[test]
    fn a_parked_frame_loop_refuses_a_gui_screenshot_with_the_mechanism_named() {
        let (mut slot, _rx) = pending(0);
        // The request arrived before the park was declared, which is the real
        // ordering: a caller does not know the window has stopped painting.
        slot.requested_at = Instant::now() - (PARKED_FRAME_LOOP + Duration::from_millis(500));

        let refusal = slot
            .park_refusal(&beat_last_ran(Duration::from_secs(268), 1, false))
            .expect("a parked frame loop must refuse rather than arm a capture nobody will take");

        assert!(
            refusal.contains("REFUSED"),
            "the caller must be able to tell this from a timeout: {refusal}"
        );
        for mechanism in [
            "compositor frame callbacks",
            "WAYLAND_DISPLAY",
            "present_mode.negotiated",
            "screenshot_simulation",
        ] {
            assert!(
                refusal.contains(mechanism),
                "M-4 says refuse WITH THE MECHANISM NAMED — missing {mechanism:?}: {refusal}"
            );
        }
        assert!(
            refusal.contains("retrying will not help"),
            "a refusal an agent retries in a loop is a hang with extra steps: {refusal}"
        );
    }

    /// The false-positive this must not have: a window that simply has not
    /// painted lately is not a window that cannot paint.
    ///
    /// Both halves matter. A live loop is never refused however old the
    /// request; and a *parked* loop is not refused until the request itself has
    /// had [`PARKED_FRAME_LOOP`] to be served, because `mcp_screenshot_gui`
    /// asks for a repaint when it stores the slot and a healthy window answers
    /// that in milliseconds. Once the 100 ms MCP heartbeat is deleted (M-5),
    /// an idle session's last frame is arbitrarily old by design, and without
    /// this grace period every idle screenshot would be refused.
    #[test]
    fn a_live_or_freshly_requested_capture_is_never_refused() {
        let (mut slot, _rx) = pending(0);
        slot.requested_at = Instant::now() - Duration::from_secs(600);
        assert!(
            slot.park_refusal(&live_beat()).is_none(),
            "a rendering window is unaffected by this contract"
        );

        let (fresh, _rx2) = pending(0);
        assert!(
            fresh
                .park_refusal(&beat_last_ran(Duration::from_secs(268), 1, false))
                .is_none(),
            "a just-arrived request has not yet had its repaint answered — refusing it here \
             would turn every idle-session screenshot into a false negative"
        );
    }
}
