//! Bridge types for communication between the embedded MCP server (tokio thread)
//! and the GUI main thread. The GUI owns all state; the MCP server sends requests
//! and receives responses via channels.

use std::collections::HashMap;

use serde::Serialize;

use crate::state::toolpath::ToolpathId;

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

    // ── Compute (async — response sent when compute finishes) ────────
    GenerateToolpath {
        index: usize,
    },
    GenerateAll,
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
}

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
    /// Per-toolpath error messages for failed generations.
    pub errors: Vec<(usize, String)>,
    pub response_tx: tokio::sync::oneshot::Sender<McpResponse>,
    /// Optional channel for streaming per-toolpath progress back to the MCP client.
    pub progress_tx: Option<tokio::sync::mpsc::Sender<ProgressUpdate>>,
}

impl PendingMcpCompute {
    pub fn new() -> Self {
        Self::default()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

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
