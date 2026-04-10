//! Bridge types for communication between the embedded MCP server (tokio thread)
//! and the GUI main thread. The GUI owns all state; the MCP server sends requests
//! and receives responses via channels.

use std::collections::HashMap;

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
    ListSetups,
    GetToolpathParams {
        index: usize,
    },
    GetDiagnostics,
    GetCutTrace {
        toolpath_id: Option<usize>,
        max_hotspots: Option<usize>,
        max_issues: Option<usize>,
    },

    InspectModel,
    InspectStock,
    InspectMachine,
    InspectBrepFaces {
        model_id: usize,
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
    LoadProject {
        path: String,
    },
    SaveProject {
        path: String,
    },
    ExportGcode {
        path: String,
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
}

/// Response from the GUI thread to the MCP server.
pub struct McpResponse {
    pub result: Result<String, String>,
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
