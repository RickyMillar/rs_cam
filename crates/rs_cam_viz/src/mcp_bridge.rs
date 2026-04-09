//! Bridge types for communication between the embedded MCP server (tokio thread)
//! and the GUI main thread. The GUI owns all state; the MCP server sends requests
//! and receives responses via channels.

use std::collections::HashMap;

use crate::state::toolpath::ToolpathId;

/// A request from the MCP server to the GUI thread.
pub struct McpRequest {
    pub kind: McpRequestKind,
    pub response_tx: tokio::sync::oneshot::Sender<McpResponse>,
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

    // ── Mutations (instant) ──────────────────────────────────────────
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
    pub response_tx: tokio::sync::oneshot::Sender<McpResponse>,
}

impl PendingMcpCompute {
    pub fn new() -> Self {
        Self::default()
    }
}
