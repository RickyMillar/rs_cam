//! GUI-only runtime overlay state.
//!
//! These types hold presentation and interaction state that is NOT part of
//! the persisted project or the core compute model.  They sit alongside
//! `ProjectSession` in `AppState`.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use rs_cam_core::compute::stock_config::ModelId;
use rs_cam_core::feeds::FeedsResult;
use rs_cam_core::session::ToolpathConfig;

use super::job::{PostConfig, PostFormat};

// Re-export ComputeStatus from core (canonical definition).
pub use rs_cam_core::compute::config::ComputeStatus;

// Re-export ToolpathResult from the existing toolpath module.
pub use super::toolpath::ToolpathResult;

// ── Per-toolpath runtime state ────────────────────────────────────────

/// GUI-only state for a single toolpath — display, compute status, cached results.
pub struct ToolpathRuntime {
    pub visible: bool,
    pub locked: bool,
    pub auto_regen: bool,
    pub status: ComputeStatus,
    pub result: Option<ToolpathResult>,
    pub stale_since: Option<std::time::Instant>,
    pub feeds_result: Option<FeedsResult>,
    pub debug_trace: Option<Arc<rs_cam_core::debug_trace::ToolpathDebugTrace>>,
    pub semantic_trace: Option<Arc<rs_cam_core::semantic_trace::ToolpathSemanticTrace>>,
    pub debug_trace_path: Option<PathBuf>,
}

impl ToolpathRuntime {
    /// Create runtime state for a new toolpath with sensible defaults.
    pub fn new(auto_regen: bool) -> Self {
        Self {
            visible: true,
            locked: false,
            auto_regen,
            status: ComputeStatus::Pending,
            result: None,
            stale_since: None,
            feeds_result: None,
            debug_trace: None,
            semantic_trace: None,
            debug_trace_path: None,
        }
    }

    /// Clear all computed/cached state (e.g. after param change or on load).
    pub fn clear_runtime(&mut self) {
        self.status = ComputeStatus::Pending;
        self.result = None;
        self.stale_since = None;
        self.feeds_result = None;
        self.debug_trace = None;
        self.semantic_trace = None;
        self.debug_trace_path = None;
    }
}

// ── Per-setup runtime state ───────────────────────────────────────────

/// Which corner of the stock to probe for XY datum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Corner {
    FrontLeft,
    FrontRight,
    BackLeft,
    BackRight,
}

impl Corner {
    pub const ALL: &[Corner] = &[
        Corner::FrontLeft,
        Corner::FrontRight,
        Corner::BackLeft,
        Corner::BackRight,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            Corner::FrontLeft => "Front-Left",
            Corner::FrontRight => "Front-Right",
            Corner::BackLeft => "Back-Left",
            Corner::BackRight => "Back-Right",
        }
    }

    pub fn to_key(&self) -> &'static str {
        match self {
            Corner::FrontLeft => "fl",
            Corner::FrontRight => "fr",
            Corner::BackLeft => "bl",
            Corner::BackRight => "br",
        }
    }

    pub fn from_key(s: &str) -> Self {
        match s {
            "fr" => Corner::FrontRight,
            "bl" => Corner::BackLeft,
            "br" => Corner::BackRight,
            _ => Corner::FrontLeft,
        }
    }
}

/// How the operator establishes XY zero for this setup.
#[derive(Debug, Clone, PartialEq)]
pub enum XYDatum {
    CornerProbe(Corner),
    CenterOfStock,
    AlignmentPins,
    Manual,
}

impl Default for XYDatum {
    fn default() -> Self {
        XYDatum::CornerProbe(Corner::FrontLeft)
    }
}

impl XYDatum {
    pub fn label(&self) -> &str {
        match self {
            XYDatum::CornerProbe(c) => match c {
                Corner::FrontLeft => "Corner Probe (Front-Left)",
                Corner::FrontRight => "Corner Probe (Front-Right)",
                Corner::BackLeft => "Corner Probe (Back-Left)",
                Corner::BackRight => "Corner Probe (Back-Right)",
            },
            XYDatum::CenterOfStock => "Center of Stock",
            XYDatum::AlignmentPins => "Alignment Pins",
            XYDatum::Manual => "Manual",
        }
    }

    pub fn to_key(&self) -> String {
        match self {
            XYDatum::CornerProbe(c) => format!("corner_{}", c.to_key()),
            XYDatum::CenterOfStock => "center".into(),
            XYDatum::AlignmentPins => "pins".into(),
            XYDatum::Manual => "manual".into(),
        }
    }

    pub fn from_key(s: &str) -> Self {
        if let Some(corner) = s.strip_prefix("corner_") {
            XYDatum::CornerProbe(Corner::from_key(corner))
        } else {
            match s {
                "center" => XYDatum::CenterOfStock,
                "pins" => XYDatum::AlignmentPins,
                "manual" => XYDatum::Manual,
                _ => XYDatum::default(),
            }
        }
    }
}

/// How the operator establishes Z zero for this setup.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum ZDatum {
    #[default]
    StockTop,
    MachineTable,
    FixedOffset(f64),
    Manual,
}

impl ZDatum {
    pub fn label(&self) -> String {
        match self {
            ZDatum::StockTop => "Stock Top".into(),
            ZDatum::MachineTable => "Machine Table".into(),
            ZDatum::FixedOffset(z) => format!("Fixed Offset ({z:.1} mm)"),
            ZDatum::Manual => "Manual".into(),
        }
    }

    pub fn to_key(&self) -> String {
        match self {
            ZDatum::StockTop => "stock_top".into(),
            ZDatum::MachineTable => "table".into(),
            ZDatum::FixedOffset(z) => format!("offset:{z}"),
            ZDatum::Manual => "manual".into(),
        }
    }

    pub fn from_key(s: &str) -> Self {
        if let Some(val) = s.strip_prefix("offset:") {
            ZDatum::FixedOffset(val.parse().unwrap_or(0.0))
        } else {
            match s {
                "table" => ZDatum::MachineTable,
                "manual" => ZDatum::Manual,
                _ => ZDatum::StockTop,
            }
        }
    }
}

/// How to establish the work coordinate system for a setup.
#[derive(Debug, Clone, Default)]
pub struct DatumConfig {
    pub xy_method: XYDatum,
    pub z_method: ZDatum,
    pub notes: String,
}

/// GUI-only state for a single setup — datum config and model filtering.
#[derive(Default)]
pub struct SetupRuntime {
    pub datum: DatumConfig,
    /// Models relevant to this setup. Empty means all models are available.
    pub model_ids: Vec<ModelId>,
}

// ── Combined view for UI code ─────────────────────────────────────────

/// A read-only combined view of a toolpath's config (from session) and
/// runtime state (from GUI overlay).  Used by UI drawing code.
pub struct ToolpathView<'a> {
    pub config: &'a ToolpathConfig,
    pub runtime: &'a ToolpathRuntime,
    /// Index in the session's `toolpath_configs` vec.
    pub index: usize,
}

// ── Project-level GUI state ───────────────────────────────────────────

/// GUI-only project-level state.
pub struct GuiState {
    pub file_path: Option<PathBuf>,
    pub dirty: bool,
    pub edit_counter: u64,
    /// Viz-friendly post config view (mirrors session post config with enum format).
    pub post: PostConfig,
    /// Per-toolpath GUI runtime state, keyed by toolpath semantic ID.
    pub toolpath_rt: HashMap<usize, ToolpathRuntime>,
    /// Per-setup GUI runtime state, keyed by setup semantic ID.
    pub setup_rt: HashMap<usize, SetupRuntime>,
    /// Recently changed parameters from MCP, with timestamp for fade-out.
    /// Key: "toolpath_{id}_{param}" or "tool_{id}_{param}" or "stock_{param}"
    #[cfg(feature = "mcp")]
    pub mcp_highlights: HashMap<String, std::time::Instant>,
}

impl GuiState {
    pub fn new() -> Self {
        Self {
            file_path: None,
            dirty: false,
            edit_counter: 0,
            post: PostConfig::default(),
            toolpath_rt: HashMap::new(),
            setup_rt: HashMap::new(),
            #[cfg(feature = "mcp")]
            mcp_highlights: HashMap::new(),
        }
    }

    /// Build a `PostConfig` (viz enum format) from the session's string-based config.
    pub fn post_from_session(session_post: &rs_cam_core::session::ProjectPostConfig) -> PostConfig {
        PostConfig {
            format: match session_post.format.to_ascii_lowercase().as_str() {
                "linuxcnc" => PostFormat::LinuxCnc,
                "mach3" => PostFormat::Mach3,
                _ => PostFormat::Grbl,
            },
            spindle_speed: session_post.spindle_speed,
            safe_z: session_post.safe_z,
            high_feedrate_mode: session_post.high_feedrate_mode,
            high_feedrate: session_post.high_feedrate,
        }
    }

    /// Sync session post config from the viz-friendly PostConfig.
    pub fn post_to_session(post: &PostConfig) -> rs_cam_core::session::ProjectPostConfig {
        rs_cam_core::session::ProjectPostConfig {
            format: match post.format {
                PostFormat::Grbl => "grbl",
                PostFormat::LinuxCnc => "linuxcnc",
                PostFormat::Mach3 => "mach3",
            }
            .to_owned(),
            spindle_speed: post.spindle_speed,
            safe_z: post.safe_z,
            high_feedrate_mode: post.high_feedrate_mode,
            high_feedrate: post.high_feedrate,
        }
    }

    /// Mark the project as having unsaved changes.
    pub fn mark_edited(&mut self) {
        self.dirty = true;
        self.edit_counter += 1;
    }

    /// Get or create a toolpath runtime entry.
    pub fn toolpath_rt_or_default(&mut self, id: usize) -> &mut ToolpathRuntime {
        self.toolpath_rt
            .entry(id)
            .or_insert_with(|| ToolpathRuntime::new(true))
    }

    /// Get or create a setup runtime entry.
    pub fn setup_rt_or_default(&mut self, id: usize) -> &mut SetupRuntime {
        self.setup_rt.entry(id).or_default()
    }
}

impl Default for GuiState {
    fn default() -> Self {
        Self::new()
    }
}
