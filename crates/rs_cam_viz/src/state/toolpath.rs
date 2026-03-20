use std::sync::Arc;

use rs_cam_core::toolpath::Toolpath;

use super::job::ToolId;

/// Unique identifier for a toolpath.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ToolpathId(pub usize);

/// Pocket clearing pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PocketPattern {
    Contour,
    Zigzag,
}

/// Operation-specific configuration.
#[derive(Debug, Clone)]
pub enum OperationConfig {
    Pocket(PocketConfig),
}

impl OperationConfig {
    pub fn label(&self) -> &'static str {
        match self {
            OperationConfig::Pocket(_) => "Pocket",
        }
    }
}

/// Pocket operation parameters.
#[derive(Debug, Clone)]
pub struct PocketConfig {
    pub stepover: f64,
    pub depth: f64,
    pub depth_per_pass: f64,
    pub feed_rate: f64,
    pub plunge_rate: f64,
    pub climb: bool,
    pub pattern: PocketPattern,
    pub angle: f64,
}

impl Default for PocketConfig {
    fn default() -> Self {
        Self {
            stepover: 2.0,
            depth: 3.0,
            depth_per_pass: 1.5,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            climb: true,
            pattern: PocketPattern::Contour,
            angle: 0.0,
        }
    }
}

/// Computation status of a toolpath.
#[derive(Debug, Clone)]
pub enum ComputeStatus {
    Pending,
    Computing(f32),
    Done,
    Error(String),
}

/// Computed toolpath statistics.
#[derive(Debug, Clone, Default)]
pub struct ToolpathStats {
    pub move_count: usize,
    pub cutting_distance: f64,
    pub rapid_distance: f64,
}

/// A toolpath entry in the job.
pub struct ToolpathEntry {
    pub id: ToolpathId,
    pub name: String,
    pub enabled: bool,
    pub visible: bool,
    pub tool_id: ToolId,
    pub model_id: super::job::ModelId,
    pub operation: OperationConfig,
    pub status: ComputeStatus,
    pub result: Option<ToolpathResult>,
}

/// Result of toolpath computation.
pub struct ToolpathResult {
    pub toolpath: Arc<Toolpath>,
    pub stats: ToolpathStats,
}
