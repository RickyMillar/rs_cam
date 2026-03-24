use crate::state::job::ToolId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PocketPattern {
    Contour,
    Zigzag,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileSide {
    Outside,
    Inside,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryStyle {
    Plunge,
    Helix,
    Ramp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RegionOrdering {
    Global,
    ByArea,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClearingStrategy {
    ContourParallel,
    Adaptive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScallopDirection {
    OutsideIn,
    InsideOut,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CutDirection {
    Climb,
    Conventional,
    BothWays,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FaceDirection {
    OneWay,
    Zigzag,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TraceCompensation {
    None,
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DrillCycleType {
    Simple,
    Dwell,
    Peck,
    ChipBreak,
}

/// Whether tool compensation is computed in CAM or on the controller.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompensationType {
    InComputer,
    InControl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpiralDirection {
    InsideOut,
    OutsideIn,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaceConfig {
    pub stepover: f64,
    pub depth: f64,
    pub depth_per_pass: f64,
    pub feed_rate: f64,
    pub plunge_rate: f64,
    pub stock_offset: f64,
    pub direction: FaceDirection,
}

impl Default for FaceConfig {
    fn default() -> Self {
        Self {
            stepover: 5.0,
            depth: 0.0,
            depth_per_pass: 1.0,
            feed_rate: 1500.0,
            plunge_rate: 500.0,
            stock_offset: 5.0,
            direction: FaceDirection::Zigzag,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceConfig {
    pub depth: f64,
    pub depth_per_pass: f64,
    pub feed_rate: f64,
    pub plunge_rate: f64,
    pub compensation: TraceCompensation,
}

impl Default for TraceConfig {
    fn default() -> Self {
        Self {
            depth: 1.0,
            depth_per_pass: 0.5,
            feed_rate: 800.0,
            plunge_rate: 400.0,
            compensation: TraceCompensation::None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrillConfig {
    pub depth: f64,
    pub cycle: DrillCycleType,
    pub peck_depth: f64,
    pub dwell_time: f64,
    pub retract_amount: f64,
    pub feed_rate: f64,
    pub retract_z: f64,
}

impl Default for DrillConfig {
    fn default() -> Self {
        Self {
            depth: 10.0,
            cycle: DrillCycleType::Peck,
            peck_depth: 3.0,
            dwell_time: 0.5,
            retract_amount: 0.5,
            feed_rate: 300.0,
            retract_z: 2.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlignmentPinDrillConfig {
    /// Pin hole XY positions (snapshot from stock.alignment_pins at submit time).
    #[serde(default)]
    pub holes: Vec<[f64; 2]>,
    /// How far below stock bottom to drill into spoilboard (mm).
    pub spoilboard_penetration: f64,
    pub cycle: DrillCycleType,
    pub peck_depth: f64,
    pub feed_rate: f64,
    pub retract_z: f64,
}

impl Default for AlignmentPinDrillConfig {
    fn default() -> Self {
        Self {
            holes: Vec::new(),
            spoilboard_penetration: 2.0,
            cycle: DrillCycleType::Peck,
            peck_depth: 3.0,
            feed_rate: 300.0,
            retract_z: 2.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChamferConfig {
    pub chamfer_width: f64,
    pub tip_offset: f64,
    pub feed_rate: f64,
    pub plunge_rate: f64,
}

impl Default for ChamferConfig {
    fn default() -> Self {
        Self {
            chamfer_width: 1.0,
            tip_offset: 0.1,
            feed_rate: 800.0,
            plunge_rate: 400.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PocketConfig {
    pub stepover: f64,
    pub depth: f64,
    pub depth_per_pass: f64,
    pub feed_rate: f64,
    pub plunge_rate: f64,
    pub climb: bool,
    pub pattern: PocketPattern,
    pub angle: f64,
    pub finishing_passes: usize,
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
            finishing_passes: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub finishing_passes: usize,
    pub compensation: CompensationType,
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
            finishing_passes: 0,
            compensation: CompensationType::InComputer,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DropCutterConfig {
    pub stepover: f64,
    pub feed_rate: f64,
    pub plunge_rate: f64,
    pub min_z: f64,
    pub slope_from: f64,
    pub slope_to: f64,
}

impl Default for DropCutterConfig {
    fn default() -> Self {
        Self {
            stepover: 1.0,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            min_z: -50.0,
            slope_from: 0.0,
            slope_to: 90.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Adaptive3dConfig {
    pub stepover: f64,
    pub depth_per_pass: f64,
    pub stock_to_leave_radial: f64,
    pub stock_to_leave_axial: f64,
    pub feed_rate: f64,
    pub plunge_rate: f64,
    pub tolerance: f64,
    pub min_cutting_radius: f64,
    pub stock_top_z: f64,
    pub entry_style: EntryStyle,
    pub fine_stepdown: f64,
    pub detect_flat_areas: bool,
    pub region_ordering: RegionOrdering,
    #[serde(default = "default_clearing_strategy")]
    pub clearing_strategy: ClearingStrategy,
    #[serde(default)]
    pub z_blend: bool,
}

impl Default for Adaptive3dConfig {
    fn default() -> Self {
        Self {
            stepover: 2.0,
            depth_per_pass: 3.0,
            stock_to_leave_radial: 0.5,
            stock_to_leave_axial: 0.5,
            feed_rate: 1500.0,
            plunge_rate: 500.0,
            tolerance: 0.1,
            min_cutting_radius: 0.0,
            stock_top_z: 30.0,
            entry_style: EntryStyle::Plunge,
            fine_stepdown: 0.0,
            detect_flat_areas: false,
            region_ordering: RegionOrdering::Global,
            clearing_strategy: ClearingStrategy::ContourParallel,
            z_blend: false,
        }
    }
}

fn default_clearing_strategy() -> ClearingStrategy {
    ClearingStrategy::ContourParallel
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaterlineConfig {
    pub z_step: f64,
    pub sampling: f64,
    pub start_z: f64,
    pub final_z: f64,
    pub feed_rate: f64,
    pub plunge_rate: f64,
    pub continuous: bool,
}

impl Default for WaterlineConfig {
    fn default() -> Self {
        Self {
            z_step: 1.0,
            sampling: 0.5,
            start_z: 0.0,
            final_z: -25.0,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            continuous: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PencilConfig {
    pub bitangency_angle: f64,
    pub min_cut_length: f64,
    pub hookup_distance: f64,
    pub num_offset_passes: usize,
    pub offset_stepover: f64,
    pub sampling: f64,
    pub feed_rate: f64,
    pub plunge_rate: f64,
    pub stock_to_leave: f64,
}

impl Default for PencilConfig {
    fn default() -> Self {
        Self {
            bitangency_angle: 160.0,
            min_cut_length: 2.0,
            hookup_distance: 5.0,
            num_offset_passes: 1,
            offset_stepover: 0.5,
            sampling: 0.5,
            feed_rate: 800.0,
            plunge_rate: 400.0,
            stock_to_leave: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScallopConfig {
    pub scallop_height: f64,
    pub tolerance: f64,
    pub direction: ScallopDirection,
    pub continuous: bool,
    pub slope_from: f64,
    pub slope_to: f64,
    pub feed_rate: f64,
    pub plunge_rate: f64,
    pub stock_to_leave: f64,
}

impl Default for ScallopConfig {
    fn default() -> Self {
        Self {
            scallop_height: 0.1,
            tolerance: 0.05,
            direction: ScallopDirection::OutsideIn,
            continuous: false,
            slope_from: 0.0,
            slope_to: 90.0,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            stock_to_leave: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SteepShallowConfig {
    pub threshold_angle: f64,
    pub overlap_distance: f64,
    pub wall_clearance: f64,
    pub steep_first: bool,
    pub stepover: f64,
    pub z_step: f64,
    pub feed_rate: f64,
    pub plunge_rate: f64,
    pub sampling: f64,
    pub stock_to_leave: f64,
    pub tolerance: f64,
}

impl Default for SteepShallowConfig {
    fn default() -> Self {
        Self {
            threshold_angle: 45.0,
            overlap_distance: 1.0,
            wall_clearance: 0.5,
            steep_first: true,
            stepover: 1.0,
            z_step: 1.0,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            sampling: 0.5,
            stock_to_leave: 0.0,
            tolerance: 0.05,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RampFinishConfig {
    pub max_stepdown: f64,
    pub slope_from: f64,
    pub slope_to: f64,
    pub direction: CutDirection,
    pub order_bottom_up: bool,
    pub feed_rate: f64,
    pub plunge_rate: f64,
    pub sampling: f64,
    pub stock_to_leave: f64,
    pub tolerance: f64,
}

impl Default for RampFinishConfig {
    fn default() -> Self {
        Self {
            max_stepdown: 0.5,
            slope_from: 30.0,
            slope_to: 90.0,
            direction: CutDirection::Climb,
            order_bottom_up: false,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            sampling: 0.5,
            stock_to_leave: 0.0,
            tolerance: 0.05,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpiralFinishConfig {
    pub stepover: f64,
    pub direction: SpiralDirection,
    pub feed_rate: f64,
    pub plunge_rate: f64,
    pub stock_to_leave: f64,
}

impl Default for SpiralFinishConfig {
    fn default() -> Self {
        Self {
            stepover: 1.0,
            direction: SpiralDirection::InsideOut,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            stock_to_leave: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RadialFinishConfig {
    pub angular_step: f64,
    pub point_spacing: f64,
    pub feed_rate: f64,
    pub plunge_rate: f64,
    pub stock_to_leave: f64,
}

impl Default for RadialFinishConfig {
    fn default() -> Self {
        Self {
            angular_step: 5.0,
            point_spacing: 0.5,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            stock_to_leave: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HorizontalFinishConfig {
    pub angle_threshold: f64,
    pub stepover: f64,
    pub feed_rate: f64,
    pub plunge_rate: f64,
    pub stock_to_leave: f64,
}

impl Default for HorizontalFinishConfig {
    fn default() -> Self {
        Self {
            angle_threshold: 5.0,
            stepover: 1.0,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            stock_to_leave: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectCurveConfig {
    pub depth: f64,
    pub point_spacing: f64,
    pub feed_rate: f64,
    pub plunge_rate: f64,
}

impl Default for ProjectCurveConfig {
    fn default() -> Self {
        Self {
            depth: 1.0,
            point_spacing: 0.5,
            feed_rate: 800.0,
            plunge_rate: 400.0,
        }
    }
}
