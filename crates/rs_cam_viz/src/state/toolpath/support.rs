use serde::{Deserialize, Serialize};

/// Where the toolpath's stock material comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum StockSource {
    /// Start from raw stock (default).
    #[default]
    Fresh,
    /// Simulate all prior enabled toolpaths to determine starting material.
    FromRemainingStock,
}

/// Unique identifier for a toolpath.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ToolpathId(pub usize);

#[derive(Debug, Clone)]
pub enum ComputeStatus {
    Pending,
    Computing,
    Done,
    Error(String),
}

#[derive(Debug, Clone, Default)]
pub struct ToolpathStats {
    pub move_count: usize,
    pub cutting_distance: f64,
    pub rapid_distance: f64,
}

/// Controls whether a height value is auto-computed or manually set.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "mode", content = "value", rename_all = "snake_case")]
pub enum HeightMode {
    /// Auto-compute from stock/operation context.
    Auto,
    /// User-specified absolute Z value.
    Manual(f64),
}

impl HeightMode {
    pub fn value(&self, auto_value: f64) -> f64 {
        match self {
            HeightMode::Auto => auto_value,
            HeightMode::Manual(value) => *value,
        }
    }

    pub fn is_auto(&self) -> bool {
        matches!(self, HeightMode::Auto)
    }
}

/// Five-level height system controlling vertical tool motion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeightsConfig {
    pub clearance_z: HeightMode,
    pub retract_z: HeightMode,
    pub feed_z: HeightMode,
    pub top_z: HeightMode,
    pub bottom_z: HeightMode,
}

impl Default for HeightsConfig {
    fn default() -> Self {
        Self {
            clearance_z: HeightMode::Auto,
            retract_z: HeightMode::Auto,
            feed_z: HeightMode::Auto,
            top_z: HeightMode::Auto,
            bottom_z: HeightMode::Auto,
        }
    }
}

/// Tracks which feed parameters are auto-calculated vs user-overridden.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedsAutoMode {
    pub feed_rate: bool,
    pub plunge_rate: bool,
    pub stepover: bool,
    pub depth_per_pass: bool,
    pub spindle_speed: bool,
}

impl Default for FeedsAutoMode {
    fn default() -> Self {
        Self {
            feed_rate: true,
            plunge_rate: true,
            stepover: true,
            depth_per_pass: true,
            spindle_speed: true,
        }
    }
}

impl HeightsConfig {
    /// Resolve all heights given the context values.
    /// `safe_z`: from PostConfig (the baseline retract height)
    /// `op_depth`: the operation's total depth/span (positive)
    pub fn resolve(&self, safe_z: f64, op_depth: f64) -> ResolvedHeights {
        let retract = self.retract_z.value(safe_z);
        ResolvedHeights {
            clearance_z: self.clearance_z.value(retract + 10.0),
            retract_z: retract,
            feed_z: self.feed_z.value(retract - 2.0),
            top_z: self.top_z.value(0.0),
            bottom_z: self.bottom_z.value(-op_depth.abs()),
        }
    }
}

/// Fully resolved (concrete) heights for a single operation.
#[derive(Debug, Clone, Copy)]
pub struct ResolvedHeights {
    pub clearance_z: f64,
    pub retract_z: f64,
    pub feed_z: f64,
    pub top_z: f64,
    pub bottom_z: f64,
}

impl ResolvedHeights {
    /// The depth range: distance from top_z to bottom_z (positive value).
    pub fn depth(&self) -> f64 {
        (self.top_z - self.bottom_z).abs()
    }
}

/// Tool containment mode for machining boundary (re-export from core).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum BoundaryContainment {
    #[default]
    Center,
    Inside,
    Outside,
}

/// Entry style for plunge replacement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DressupEntryStyle {
    None,
    Ramp,
    Helix,
}

/// How the tool retracts between cutting passes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetractStrategy {
    /// Always retract to retract_z (safest, default).
    Full,
    /// Retract just above the highest Z on nearby path + 2mm (faster).
    Minimum,
}

/// Configurable dressups applied after toolpath generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DressupConfig {
    pub entry_style: DressupEntryStyle,
    pub ramp_angle: f64,
    pub helix_radius: f64,
    pub helix_pitch: f64,
    pub dogbone: bool,
    pub dogbone_angle: f64,
    pub lead_in_out: bool,
    pub lead_radius: f64,
    pub link_moves: bool,
    pub link_max_distance: f64,
    pub link_feed_rate: f64,
    pub arc_fitting: bool,
    pub arc_tolerance: f64,
    pub feed_optimization: bool,
    pub feed_max_rate: f64,
    pub feed_ramp_rate: f64,
    pub optimize_rapid_order: bool,
    pub retract_strategy: RetractStrategy,
}

impl Default for DressupConfig {
    fn default() -> Self {
        Self {
            entry_style: DressupEntryStyle::None,
            ramp_angle: 3.0,
            helix_radius: 2.0,
            helix_pitch: 1.0,
            dogbone: false,
            dogbone_angle: 90.0,
            lead_in_out: false,
            lead_radius: 2.0,
            link_moves: false,
            link_max_distance: 10.0,
            link_feed_rate: 500.0,
            arc_fitting: false,
            arc_tolerance: 0.05,
            feed_optimization: false,
            feed_max_rate: 3000.0,
            feed_ramp_rate: 200.0,
            optimize_rapid_order: false,
            retract_strategy: RetractStrategy::Full,
        }
    }
}
