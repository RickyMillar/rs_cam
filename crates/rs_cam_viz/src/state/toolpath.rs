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

/// Profile cut side.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileSide {
    Outside,
    Inside,
}

/// Operation type for creating new toolpaths.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationType {
    Pocket,
    Profile,
    Adaptive,
    VCarve,
    Rest,
    Inlay,
    Zigzag,
}

impl OperationType {
    pub const ALL_2D: &[OperationType] = &[
        OperationType::Pocket,
        OperationType::Profile,
        OperationType::Adaptive,
        OperationType::VCarve,
        OperationType::Rest,
        OperationType::Inlay,
        OperationType::Zigzag,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            OperationType::Pocket => "Pocket",
            OperationType::Profile => "Profile",
            OperationType::Adaptive => "Adaptive",
            OperationType::VCarve => "VCarve",
            OperationType::Rest => "Rest Machining",
            OperationType::Inlay => "Inlay",
            OperationType::Zigzag => "Zigzag",
        }
    }
}

/// Operation-specific configuration.
#[derive(Debug, Clone)]
pub enum OperationConfig {
    Pocket(PocketConfig),
    Profile(ProfileConfig),
    Adaptive(AdaptiveConfig),
    VCarve(VCarveConfig),
    Rest(RestConfig),
    Inlay(InlayConfig),
    Zigzag(ZigzagConfig),
}

impl OperationConfig {
    pub fn label(&self) -> &'static str {
        match self {
            OperationConfig::Pocket(_) => "Pocket",
            OperationConfig::Profile(_) => "Profile",
            OperationConfig::Adaptive(_) => "Adaptive",
            OperationConfig::VCarve(_) => "VCarve",
            OperationConfig::Rest(_) => "Rest Machining",
            OperationConfig::Inlay(_) => "Inlay",
            OperationConfig::Zigzag(_) => "Zigzag",
        }
    }

    pub fn new_default(op_type: OperationType) -> Self {
        match op_type {
            OperationType::Pocket => OperationConfig::Pocket(PocketConfig::default()),
            OperationType::Profile => OperationConfig::Profile(ProfileConfig::default()),
            OperationType::Adaptive => OperationConfig::Adaptive(AdaptiveConfig::default()),
            OperationType::VCarve => OperationConfig::VCarve(VCarveConfig::default()),
            OperationType::Rest => OperationConfig::Rest(RestConfig::default()),
            OperationType::Inlay => OperationConfig::Inlay(InlayConfig::default()),
            OperationType::Zigzag => OperationConfig::Zigzag(ZigzagConfig::default()),
        }
    }
}

// --- Per-operation config structs ---

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

#[derive(Debug, Clone)]
pub struct ProfileConfig {
    pub side: ProfileSide,
    pub depth: f64,
    pub depth_per_pass: f64,
    pub feed_rate: f64,
    pub plunge_rate: f64,
    pub climb: bool,
    pub tab_count: usize,
    pub tab_width: f64,
    pub tab_height: f64,
}

impl Default for ProfileConfig {
    fn default() -> Self {
        Self {
            side: ProfileSide::Outside,
            depth: 6.0,
            depth_per_pass: 2.0,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            climb: true,
            tab_count: 0,
            tab_width: 6.0,
            tab_height: 2.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AdaptiveConfig {
    pub stepover: f64,
    pub depth: f64,
    pub depth_per_pass: f64,
    pub feed_rate: f64,
    pub plunge_rate: f64,
    pub tolerance: f64,
    pub slot_clearing: bool,
    pub min_cutting_radius: f64,
}

impl Default for AdaptiveConfig {
    fn default() -> Self {
        Self {
            stepover: 2.0,
            depth: 6.0,
            depth_per_pass: 2.0,
            feed_rate: 1500.0,
            plunge_rate: 500.0,
            tolerance: 0.1,
            slot_clearing: true,
            min_cutting_radius: 0.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct VCarveConfig {
    pub max_depth: f64,
    pub stepover: f64,
    pub feed_rate: f64,
    pub plunge_rate: f64,
    pub tolerance: f64,
}

impl Default for VCarveConfig {
    fn default() -> Self {
        Self {
            max_depth: 5.0,
            stepover: 0.5,
            feed_rate: 800.0,
            plunge_rate: 400.0,
            tolerance: 0.05,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RestConfig {
    pub prev_tool_id: Option<ToolId>,
    pub stepover: f64,
    pub depth: f64,
    pub depth_per_pass: f64,
    pub feed_rate: f64,
    pub plunge_rate: f64,
    pub angle: f64,
}

impl Default for RestConfig {
    fn default() -> Self {
        Self {
            prev_tool_id: None,
            stepover: 1.0,
            depth: 6.0,
            depth_per_pass: 2.0,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            angle: 0.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct InlayConfig {
    pub pocket_depth: f64,
    pub glue_gap: f64,
    pub flat_depth: f64,
    pub boundary_offset: f64,
    pub stepover: f64,
    pub flat_tool_radius: f64,
    pub feed_rate: f64,
    pub plunge_rate: f64,
    pub tolerance: f64,
}

impl Default for InlayConfig {
    fn default() -> Self {
        Self {
            pocket_depth: 3.0,
            glue_gap: 0.1,
            flat_depth: 0.5,
            boundary_offset: 0.0,
            stepover: 1.0,
            flat_tool_radius: 3.175,
            feed_rate: 800.0,
            plunge_rate: 400.0,
            tolerance: 0.05,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ZigzagConfig {
    pub stepover: f64,
    pub depth: f64,
    pub depth_per_pass: f64,
    pub feed_rate: f64,
    pub plunge_rate: f64,
    pub angle: f64,
}

impl Default for ZigzagConfig {
    fn default() -> Self {
        Self {
            stepover: 2.0,
            depth: 3.0,
            depth_per_pass: 1.5,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
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
