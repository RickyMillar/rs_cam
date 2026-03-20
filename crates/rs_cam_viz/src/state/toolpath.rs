use std::sync::Arc;

use rs_cam_core::toolpath::Toolpath;

use super::job::ToolId;

/// Unique identifier for a toolpath.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ToolpathId(pub usize);

// --- Shared enums ---

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PocketPattern { Contour, Zigzag }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileSide { Outside, Inside }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryStyle { Plunge, Helix, Ramp }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionOrdering { Global, ByArea }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScallopDirection { OutsideIn, InsideOut }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CutDirection { Climb, Conventional, BothWays }

/// Operation type for creating new toolpaths.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationType {
    // 2.5D
    Pocket, Profile, Adaptive, VCarve, Rest, Inlay, Zigzag,
    Face, Trace, Drill, Chamfer,
    // 3D
    DropCutter, Adaptive3d, Waterline, Pencil, Scallop, SteepShallow, RampFinish,
    SpiralFinish, RadialFinish, HorizontalFinish, ProjectCurve,
}

impl OperationType {
    pub const ALL_2D: &[OperationType] = &[
        OperationType::Face, OperationType::Pocket, OperationType::Profile,
        OperationType::Adaptive, OperationType::VCarve, OperationType::Rest,
        OperationType::Inlay, OperationType::Zigzag, OperationType::Trace,
        OperationType::Drill, OperationType::Chamfer,
    ];

    pub const ALL_3D: &[OperationType] = &[
        OperationType::DropCutter, OperationType::Adaptive3d, OperationType::Waterline,
        OperationType::Pencil, OperationType::Scallop, OperationType::SteepShallow,
        OperationType::RampFinish, OperationType::SpiralFinish, OperationType::RadialFinish,
        OperationType::HorizontalFinish, OperationType::ProjectCurve,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            OperationType::Face => "Face",
            OperationType::Pocket => "Pocket",
            OperationType::Profile => "Profile",
            OperationType::Adaptive => "Adaptive",
            OperationType::VCarve => "VCarve",
            OperationType::Rest => "Rest Machining",
            OperationType::Inlay => "Inlay",
            OperationType::Zigzag => "Zigzag",
            OperationType::Trace => "Trace",
            OperationType::Drill => "Drill",
            OperationType::Chamfer => "Chamfer",
            OperationType::DropCutter => "3D Finish",
            OperationType::Adaptive3d => "3D Rough",
            OperationType::Waterline => "Waterline",
            OperationType::Pencil => "Pencil Finish",
            OperationType::Scallop => "Scallop Finish",
            OperationType::SteepShallow => "Steep/Shallow",
            OperationType::RampFinish => "Ramp Finish",
            OperationType::SpiralFinish => "Spiral Finish",
            OperationType::RadialFinish => "Radial Finish",
            OperationType::HorizontalFinish => "Horizontal Finish",
            OperationType::ProjectCurve => "Project Curve",
        }
    }
}

/// Operation-specific configuration.
#[derive(Debug, Clone)]
pub enum OperationConfig {
    // 2.5D
    Face(FaceConfig),
    Pocket(PocketConfig),
    Profile(ProfileConfig),
    Adaptive(AdaptiveConfig),
    VCarve(VCarveConfig),
    Rest(RestConfig),
    Inlay(InlayConfig),
    Zigzag(ZigzagConfig),
    Trace(TraceConfig),
    Drill(DrillConfig),
    Chamfer(ChamferConfig),
    // 3D
    DropCutter(DropCutterConfig),
    Adaptive3d(Adaptive3dConfig),
    Waterline(WaterlineConfig),
    Pencil(PencilConfig),
    Scallop(ScallopConfig),
    SteepShallow(SteepShallowConfig),
    RampFinish(RampFinishConfig),
    SpiralFinish(SpiralFinishConfig),
    RadialFinish(RadialFinishConfig),
    HorizontalFinish(HorizontalFinishConfig),
    ProjectCurve(ProjectCurveConfig),
}

impl OperationConfig {
    pub fn label(&self) -> &'static str {
        match self {
            OperationConfig::Face(_) => "Face",
            OperationConfig::Pocket(_) => "Pocket",
            OperationConfig::Profile(_) => "Profile",
            OperationConfig::Adaptive(_) => "Adaptive",
            OperationConfig::VCarve(_) => "VCarve",
            OperationConfig::Rest(_) => "Rest Machining",
            OperationConfig::Inlay(_) => "Inlay",
            OperationConfig::Zigzag(_) => "Zigzag",
            OperationConfig::Trace(_) => "Trace",
            OperationConfig::Drill(_) => "Drill",
            OperationConfig::Chamfer(_) => "Chamfer",
            OperationConfig::DropCutter(_) => "3D Finish",
            OperationConfig::Adaptive3d(_) => "3D Rough",
            OperationConfig::Waterline(_) => "Waterline",
            OperationConfig::Pencil(_) => "Pencil Finish",
            OperationConfig::Scallop(_) => "Scallop Finish",
            OperationConfig::SteepShallow(_) => "Steep/Shallow",
            OperationConfig::RampFinish(_) => "Ramp Finish",
            OperationConfig::SpiralFinish(_) => "Spiral Finish",
            OperationConfig::RadialFinish(_) => "Radial Finish",
            OperationConfig::HorizontalFinish(_) => "Horizontal Finish",
            OperationConfig::ProjectCurve(_) => "Project Curve",
        }
    }

    pub fn is_3d(&self) -> bool {
        matches!(
            self,
            OperationConfig::DropCutter(_) | OperationConfig::Adaptive3d(_)
                | OperationConfig::Waterline(_) | OperationConfig::Pencil(_)
                | OperationConfig::Scallop(_) | OperationConfig::SteepShallow(_)
                | OperationConfig::RampFinish(_) | OperationConfig::SpiralFinish(_)
                | OperationConfig::RadialFinish(_) | OperationConfig::HorizontalFinish(_)
                | OperationConfig::ProjectCurve(_)
        )
    }

    /// Whether this operation uses stock bounds (not mesh or polygon).
    pub fn is_stock_based(&self) -> bool {
        matches!(self, OperationConfig::Face(_))
    }

    /// Whether this is a 2.5D op that needs both polygons AND mesh (project curve).
    pub fn needs_both(&self) -> bool {
        matches!(self, OperationConfig::ProjectCurve(_))
    }

    pub fn new_default(op_type: OperationType) -> Self {
        match op_type {
            OperationType::Face => OperationConfig::Face(FaceConfig::default()),
            OperationType::Pocket => OperationConfig::Pocket(PocketConfig::default()),
            OperationType::Profile => OperationConfig::Profile(ProfileConfig::default()),
            OperationType::Adaptive => OperationConfig::Adaptive(AdaptiveConfig::default()),
            OperationType::VCarve => OperationConfig::VCarve(VCarveConfig::default()),
            OperationType::Rest => OperationConfig::Rest(RestConfig::default()),
            OperationType::Inlay => OperationConfig::Inlay(InlayConfig::default()),
            OperationType::Zigzag => OperationConfig::Zigzag(ZigzagConfig::default()),
            OperationType::Trace => OperationConfig::Trace(TraceConfig::default()),
            OperationType::Drill => OperationConfig::Drill(DrillConfig::default()),
            OperationType::Chamfer => OperationConfig::Chamfer(ChamferConfig::default()),
            OperationType::DropCutter => OperationConfig::DropCutter(DropCutterConfig::default()),
            OperationType::Adaptive3d => OperationConfig::Adaptive3d(Adaptive3dConfig::default()),
            OperationType::Waterline => OperationConfig::Waterline(WaterlineConfig::default()),
            OperationType::Pencil => OperationConfig::Pencil(PencilConfig::default()),
            OperationType::Scallop => OperationConfig::Scallop(ScallopConfig::default()),
            OperationType::SteepShallow => OperationConfig::SteepShallow(SteepShallowConfig::default()),
            OperationType::RampFinish => OperationConfig::RampFinish(RampFinishConfig::default()),
            OperationType::SpiralFinish => OperationConfig::SpiralFinish(SpiralFinishConfig::default()),
            OperationType::RadialFinish => OperationConfig::RadialFinish(RadialFinishConfig::default()),
            OperationType::HorizontalFinish => OperationConfig::HorizontalFinish(HorizontalFinishConfig::default()),
            OperationType::ProjectCurve => OperationConfig::ProjectCurve(ProjectCurveConfig::default()),
        }
    }
}

// =========================================================================
// 2.5D config structs
// =========================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaceDirection { OneWay, Zigzag }

#[derive(Debug, Clone)]
pub struct FaceConfig {
    pub stepover: f64, pub depth: f64, pub depth_per_pass: f64,
    pub feed_rate: f64, pub plunge_rate: f64, pub stock_offset: f64,
    pub direction: FaceDirection,
}
impl Default for FaceConfig {
    fn default() -> Self {
        Self { stepover: 5.0, depth: 0.0, depth_per_pass: 1.0, feed_rate: 1500.0,
               plunge_rate: 500.0, stock_offset: 5.0, direction: FaceDirection::Zigzag }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraceCompensation { None, Left, Right }

#[derive(Debug, Clone)]
pub struct TraceConfig {
    pub depth: f64, pub depth_per_pass: f64,
    pub feed_rate: f64, pub plunge_rate: f64,
    pub compensation: TraceCompensation,
}
impl Default for TraceConfig {
    fn default() -> Self {
        Self { depth: 1.0, depth_per_pass: 0.5, feed_rate: 800.0,
               plunge_rate: 400.0, compensation: TraceCompensation::None }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrillCycleType { Simple, Dwell, Peck, ChipBreak }

#[derive(Debug, Clone)]
pub struct DrillConfig {
    pub depth: f64, pub cycle: DrillCycleType,
    pub peck_depth: f64, pub dwell_time: f64, pub retract_amount: f64,
    pub feed_rate: f64, pub retract_z: f64,
}
impl Default for DrillConfig {
    fn default() -> Self {
        Self { depth: 10.0, cycle: DrillCycleType::Peck, peck_depth: 3.0,
               dwell_time: 0.5, retract_amount: 0.5, feed_rate: 300.0, retract_z: 2.0 }
    }
}

#[derive(Debug, Clone)]
pub struct ChamferConfig {
    pub chamfer_width: f64, pub tip_offset: f64,
    pub feed_rate: f64, pub plunge_rate: f64,
}
impl Default for ChamferConfig {
    fn default() -> Self {
        Self { chamfer_width: 1.0, tip_offset: 0.1, feed_rate: 800.0, plunge_rate: 400.0 }
    }
}

#[derive(Debug, Clone)]
pub struct PocketConfig {
    pub stepover: f64, pub depth: f64, pub depth_per_pass: f64,
    pub feed_rate: f64, pub plunge_rate: f64, pub climb: bool,
    pub pattern: PocketPattern, pub angle: f64,
}
impl Default for PocketConfig {
    fn default() -> Self {
        Self { stepover: 2.0, depth: 3.0, depth_per_pass: 1.5, feed_rate: 1000.0,
               plunge_rate: 500.0, climb: true, pattern: PocketPattern::Contour, angle: 0.0 }
    }
}

#[derive(Debug, Clone)]
pub struct ProfileConfig {
    pub side: ProfileSide, pub depth: f64, pub depth_per_pass: f64,
    pub feed_rate: f64, pub plunge_rate: f64, pub climb: bool,
    pub tab_count: usize, pub tab_width: f64, pub tab_height: f64,
}
impl Default for ProfileConfig {
    fn default() -> Self {
        Self { side: ProfileSide::Outside, depth: 6.0, depth_per_pass: 2.0,
               feed_rate: 1000.0, plunge_rate: 500.0, climb: true,
               tab_count: 0, tab_width: 6.0, tab_height: 2.0 }
    }
}

#[derive(Debug, Clone)]
pub struct AdaptiveConfig {
    pub stepover: f64, pub depth: f64, pub depth_per_pass: f64,
    pub feed_rate: f64, pub plunge_rate: f64, pub tolerance: f64,
    pub slot_clearing: bool, pub min_cutting_radius: f64,
}
impl Default for AdaptiveConfig {
    fn default() -> Self {
        Self { stepover: 2.0, depth: 6.0, depth_per_pass: 2.0, feed_rate: 1500.0,
               plunge_rate: 500.0, tolerance: 0.1, slot_clearing: true, min_cutting_radius: 0.0 }
    }
}

#[derive(Debug, Clone)]
pub struct VCarveConfig {
    pub max_depth: f64, pub stepover: f64, pub feed_rate: f64,
    pub plunge_rate: f64, pub tolerance: f64,
}
impl Default for VCarveConfig {
    fn default() -> Self {
        Self { max_depth: 5.0, stepover: 0.5, feed_rate: 800.0, plunge_rate: 400.0, tolerance: 0.05 }
    }
}

#[derive(Debug, Clone)]
pub struct RestConfig {
    pub prev_tool_id: Option<ToolId>, pub stepover: f64, pub depth: f64,
    pub depth_per_pass: f64, pub feed_rate: f64, pub plunge_rate: f64, pub angle: f64,
}
impl Default for RestConfig {
    fn default() -> Self {
        Self { prev_tool_id: None, stepover: 1.0, depth: 6.0, depth_per_pass: 2.0,
               feed_rate: 1000.0, plunge_rate: 500.0, angle: 0.0 }
    }
}

#[derive(Debug, Clone)]
pub struct InlayConfig {
    pub pocket_depth: f64, pub glue_gap: f64, pub flat_depth: f64,
    pub boundary_offset: f64, pub stepover: f64, pub flat_tool_radius: f64,
    pub feed_rate: f64, pub plunge_rate: f64, pub tolerance: f64,
}
impl Default for InlayConfig {
    fn default() -> Self {
        Self { pocket_depth: 3.0, glue_gap: 0.1, flat_depth: 0.5, boundary_offset: 0.0,
               stepover: 1.0, flat_tool_radius: 3.175, feed_rate: 800.0, plunge_rate: 400.0,
               tolerance: 0.05 }
    }
}

#[derive(Debug, Clone)]
pub struct ZigzagConfig {
    pub stepover: f64, pub depth: f64, pub depth_per_pass: f64,
    pub feed_rate: f64, pub plunge_rate: f64, pub angle: f64,
}
impl Default for ZigzagConfig {
    fn default() -> Self {
        Self { stepover: 2.0, depth: 3.0, depth_per_pass: 1.5, feed_rate: 1000.0,
               plunge_rate: 500.0, angle: 0.0 }
    }
}

// =========================================================================
// 3D config structs
// =========================================================================

#[derive(Debug, Clone)]
pub struct DropCutterConfig {
    pub stepover: f64, pub feed_rate: f64, pub plunge_rate: f64, pub min_z: f64,
}
impl Default for DropCutterConfig {
    fn default() -> Self {
        Self { stepover: 1.0, feed_rate: 1000.0, plunge_rate: 500.0, min_z: -50.0 }
    }
}

#[derive(Debug, Clone)]
pub struct Adaptive3dConfig {
    pub stepover: f64, pub depth_per_pass: f64, pub stock_to_leave: f64,
    pub feed_rate: f64, pub plunge_rate: f64, pub tolerance: f64,
    pub min_cutting_radius: f64, pub stock_top_z: f64, pub entry_style: EntryStyle,
    pub fine_stepdown: f64, pub detect_flat_areas: bool, pub region_ordering: RegionOrdering,
}
impl Default for Adaptive3dConfig {
    fn default() -> Self {
        Self { stepover: 2.0, depth_per_pass: 3.0, stock_to_leave: 0.5, feed_rate: 1500.0,
               plunge_rate: 500.0, tolerance: 0.1, min_cutting_radius: 0.0, stock_top_z: 30.0,
               entry_style: EntryStyle::Plunge, fine_stepdown: 0.0, detect_flat_areas: false,
               region_ordering: RegionOrdering::Global }
    }
}

#[derive(Debug, Clone)]
pub struct WaterlineConfig {
    pub z_step: f64, pub sampling: f64, pub start_z: f64, pub final_z: f64,
    pub feed_rate: f64, pub plunge_rate: f64,
}
impl Default for WaterlineConfig {
    fn default() -> Self {
        Self { z_step: 1.0, sampling: 0.5, start_z: 0.0, final_z: -25.0,
               feed_rate: 1000.0, plunge_rate: 500.0 }
    }
}

#[derive(Debug, Clone)]
pub struct PencilConfig {
    pub bitangency_angle: f64, pub min_cut_length: f64, pub hookup_distance: f64,
    pub num_offset_passes: usize, pub offset_stepover: f64, pub sampling: f64,
    pub feed_rate: f64, pub plunge_rate: f64, pub stock_to_leave: f64,
}
impl Default for PencilConfig {
    fn default() -> Self {
        Self { bitangency_angle: 160.0, min_cut_length: 2.0, hookup_distance: 5.0,
               num_offset_passes: 1, offset_stepover: 0.5, sampling: 0.5,
               feed_rate: 800.0, plunge_rate: 400.0, stock_to_leave: 0.0 }
    }
}

#[derive(Debug, Clone)]
pub struct ScallopConfig {
    pub scallop_height: f64, pub tolerance: f64, pub direction: ScallopDirection,
    pub continuous: bool, pub slope_from: f64, pub slope_to: f64,
    pub feed_rate: f64, pub plunge_rate: f64, pub stock_to_leave: f64,
}
impl Default for ScallopConfig {
    fn default() -> Self {
        Self { scallop_height: 0.1, tolerance: 0.05, direction: ScallopDirection::OutsideIn,
               continuous: false, slope_from: 0.0, slope_to: 90.0,
               feed_rate: 1000.0, plunge_rate: 500.0, stock_to_leave: 0.0 }
    }
}

#[derive(Debug, Clone)]
pub struct SteepShallowConfig {
    pub threshold_angle: f64, pub overlap_distance: f64, pub wall_clearance: f64,
    pub steep_first: bool, pub stepover: f64, pub z_step: f64,
    pub feed_rate: f64, pub plunge_rate: f64, pub sampling: f64,
    pub stock_to_leave: f64, pub tolerance: f64,
}
impl Default for SteepShallowConfig {
    fn default() -> Self {
        Self { threshold_angle: 45.0, overlap_distance: 1.0, wall_clearance: 0.5,
               steep_first: true, stepover: 1.0, z_step: 1.0,
               feed_rate: 1000.0, plunge_rate: 500.0, sampling: 0.5,
               stock_to_leave: 0.0, tolerance: 0.05 }
    }
}

#[derive(Debug, Clone)]
pub struct RampFinishConfig {
    pub max_stepdown: f64, pub slope_from: f64, pub slope_to: f64,
    pub direction: CutDirection, pub order_bottom_up: bool,
    pub feed_rate: f64, pub plunge_rate: f64, pub sampling: f64,
    pub stock_to_leave: f64, pub tolerance: f64,
}
impl Default for RampFinishConfig {
    fn default() -> Self {
        Self { max_stepdown: 0.5, slope_from: 30.0, slope_to: 90.0,
               direction: CutDirection::Climb, order_bottom_up: false,
               feed_rate: 1000.0, plunge_rate: 500.0, sampling: 0.5,
               stock_to_leave: 0.0, tolerance: 0.05 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpiralDirection { InsideOut, OutsideIn }

#[derive(Debug, Clone)]
pub struct SpiralFinishConfig {
    pub stepover: f64, pub direction: SpiralDirection,
    pub feed_rate: f64, pub plunge_rate: f64, pub stock_to_leave: f64,
}
impl Default for SpiralFinishConfig {
    fn default() -> Self {
        Self { stepover: 1.0, direction: SpiralDirection::InsideOut,
               feed_rate: 1000.0, plunge_rate: 500.0, stock_to_leave: 0.0 }
    }
}

#[derive(Debug, Clone)]
pub struct RadialFinishConfig {
    pub angular_step: f64, pub point_spacing: f64,
    pub feed_rate: f64, pub plunge_rate: f64, pub stock_to_leave: f64,
}
impl Default for RadialFinishConfig {
    fn default() -> Self {
        Self { angular_step: 5.0, point_spacing: 0.5,
               feed_rate: 1000.0, plunge_rate: 500.0, stock_to_leave: 0.0 }
    }
}

#[derive(Debug, Clone)]
pub struct HorizontalFinishConfig {
    pub angle_threshold: f64, pub stepover: f64,
    pub feed_rate: f64, pub plunge_rate: f64, pub stock_to_leave: f64,
}
impl Default for HorizontalFinishConfig {
    fn default() -> Self {
        Self { angle_threshold: 5.0, stepover: 1.0,
               feed_rate: 1000.0, plunge_rate: 500.0, stock_to_leave: 0.0 }
    }
}

#[derive(Debug, Clone)]
pub struct ProjectCurveConfig {
    pub depth: f64, pub point_spacing: f64,
    pub feed_rate: f64, pub plunge_rate: f64,
}
impl Default for ProjectCurveConfig {
    fn default() -> Self {
        Self { depth: 1.0, point_spacing: 0.5, feed_rate: 800.0, plunge_rate: 400.0 }
    }
}

// =========================================================================
// Dressup configuration
// =========================================================================

/// Entry style for plunge replacement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DressupEntryStyle {
    None,
    Ramp,
    Helix,
}

/// Configurable dressups applied after toolpath generation.
#[derive(Debug, Clone)]
pub struct DressupConfig {
    // Entry style
    pub entry_style: DressupEntryStyle,
    pub ramp_angle: f64,
    pub helix_radius: f64,
    pub helix_pitch: f64,

    // Dogbone overcuts at inside corners
    pub dogbone: bool,
    pub dogbone_angle: f64,

    // Lead-in/out arcs for profile cuts
    pub lead_in_out: bool,
    pub lead_radius: f64,

    // Link moves (keep tool down between nearby passes)
    pub link_moves: bool,
    pub link_max_distance: f64,
    pub link_feed_rate: f64,

    // Arc fitting (reduce G-code size)
    pub arc_fitting: bool,
    pub arc_tolerance: f64,

    // Feed rate optimization
    pub feed_optimization: bool,
    pub feed_max_rate: f64,
    pub feed_ramp_rate: f64,
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
        }
    }
}

// =========================================================================
// Computation state
// =========================================================================

#[derive(Debug, Clone)]
pub enum ComputeStatus {
    Pending,
    Computing(f32),
    Done,
    Error(String),
}

#[derive(Debug, Clone, Default)]
pub struct ToolpathStats {
    pub move_count: usize,
    pub cutting_distance: f64,
    pub rapid_distance: f64,
}

pub struct ToolpathEntry {
    pub id: ToolpathId,
    pub name: String,
    pub enabled: bool,
    pub visible: bool,
    pub tool_id: ToolId,
    pub model_id: super::job::ModelId,
    pub operation: OperationConfig,
    pub dressups: DressupConfig,
    pub status: ComputeStatus,
    pub result: Option<ToolpathResult>,
    /// When params were last changed (for debounced auto-regen). None = not stale.
    pub stale_since: Option<std::time::Instant>,
    /// Whether auto-regeneration is enabled for this toolpath.
    pub auto_regen: bool,
}

pub struct ToolpathResult {
    pub toolpath: Arc<Toolpath>,
    pub stats: ToolpathStats,
}
