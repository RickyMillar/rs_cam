use serde::{Deserialize, Serialize};

use super::catalog::{DepthSemantics, OperationParams};
use super::tool_config::ToolId;

// Re-export operation parameter enums from core (single source of truth).
pub use crate::face::FaceDirection;
pub use crate::profile::ProfileSide;
pub use crate::ramp_finish::CutDirection;
pub use crate::scallop::ScallopDirection;
pub use crate::spiral_finish::SpiralDirection;
pub use crate::trace::TraceCompensation;
pub use crate::unified_finish::{ClaimsReference, CreaseReference};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PocketPattern {
    Contour,
    Zigzag,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Adaptive3dEntryStyle {
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
    /// Fast contour-parallel offset clearing via EDT. Default.
    ContourParallel,
    /// Curvature-adjusted adaptive clearing via variable-offset EDT.
    Adaptive,
    /// Per-step direction search with preflight skip and widen-band
    /// recovery. Slow to generate — reach for it when ContourParallel
    /// and Adaptive leave uncut bands on difficult geometry.
    AgentSearch,
    /// Constructive inside-out contour spiral per slice: one continuous
    /// stay-down pass per region, engagement bounded by wrap spacing
    /// (Stage 1, algorithm review 2026-06-12).
    ContourSpiral,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DrillCycleType {
    Simple,
    Dwell,
    Peck,
    ChipBreak,
}

impl DrillCycleType {
    /// Convert to core `DrillCycle` using parameters from the config struct.
    pub fn to_core(self, cfg: &DrillConfig) -> crate::drill::DrillCycle {
        use crate::drill::DrillCycle;
        match self {
            Self::Simple => DrillCycle::Simple,
            Self::Dwell => DrillCycle::Dwell(cfg.dwell_time),
            Self::Peck => DrillCycle::Peck(cfg.peck_depth),
            Self::ChipBreak => DrillCycle::ChipBreak(cfg.peck_depth, cfg.retract_amount),
        }
    }
}

/// Whether tool compensation is computed in CAM or on the controller.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompensationType {
    InComputer,
    InControl,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spindle_rpm: Option<u32>,
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
            spindle_rpm: None,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spindle_rpm: Option<u32>,
}

impl Default for TraceConfig {
    fn default() -> Self {
        Self {
            depth: 1.0,
            depth_per_pass: 0.5,
            feed_rate: 800.0,
            plunge_rate: 400.0,
            compensation: TraceCompensation::None,
            spindle_rpm: None,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spindle_rpm: Option<u32>,
    /// Explicit drill locations (XY, mm) picked from the model — DXF POINT
    /// entities or circle/arc centres, chosen in the viewport or by layer.
    ///
    /// `None` (the default) preserves the legacy behaviour of drilling the
    /// centroid of every closed polygon in the model. `Some(_)` means the
    /// user has taken control of the selection; an empty list then means
    /// "no targets selected" rather than "all centroids".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_holes: Option<Vec<[f64; 2]>>,
    /// Layer name(s) the user bulk-selected with "select all in layer".
    /// Display/round-trip only — the resolved positions live in
    /// [`Self::selected_holes`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub selected_layers: Vec<String>,
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
            spindle_rpm: None,
            selected_holes: None,
            selected_layers: Vec::new(),
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spindle_rpm: Option<u32>,
    /// Extra drill locations (XY, mm) picked from the model — DXF POINT
    /// entities or circle/arc centres. These are drilled in addition to the
    /// stock alignment pins in [`Self::holes`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_holes: Option<Vec<[f64; 2]>>,
    /// Layer name(s) bulk-selected with "select all in layer" (display/round-trip).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub selected_layers: Vec<String>,
}

impl AlignmentPinDrillConfig {
    /// Convert to the core [`crate::drill::DrillCycle`]. Pin drilling
    /// fixes dwell to 0.5 s and chip-break retract to 0.5 mm — the
    /// config carries no knobs for them (alignment pins are a fixture
    /// cycle, not a tunable drill op). T11 dedup: this conversion was
    /// previously inlined at both consumer sites in `execute.rs`
    /// (toolpath arm + `build_drill_op_for_config`).
    pub fn drill_cycle(&self) -> crate::drill::DrillCycle {
        use crate::drill::DrillCycle;
        match self.cycle {
            DrillCycleType::Simple => DrillCycle::Simple,
            DrillCycleType::Dwell => DrillCycle::Dwell(0.5),
            DrillCycleType::Peck => DrillCycle::Peck(self.peck_depth),
            DrillCycleType::ChipBreak => DrillCycle::ChipBreak(self.peck_depth, 0.5),
        }
    }
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
            spindle_rpm: None,
            selected_holes: None,
            selected_layers: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChamferConfig {
    pub chamfer_width: f64,
    pub tip_offset: f64,
    pub feed_rate: f64,
    pub plunge_rate: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spindle_rpm: Option<u32>,
}

impl Default for ChamferConfig {
    fn default() -> Self {
        Self {
            chamfer_width: 1.0,
            tip_offset: 0.1,
            feed_rate: 800.0,
            plunge_rate: 400.0,
            spindle_rpm: None,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spindle_rpm: Option<u32>,
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
            spindle_rpm: None,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spindle_rpm: Option<u32>,
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
            spindle_rpm: None,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spindle_rpm: Option<u32>,
    /// Residue cleanup strategy. Defaults to `ContourParallelHybrid`
    /// (helical starter pocket + concentric spiral + gradient-following
    /// in narrow strips + contour-parallel residue sweep). Legacy
    /// behaviour can be restored by setting this to `Legacy`.
    #[serde(default)]
    pub cleanup_strategy: crate::adaptive::CleanupStrategy,
    /// Engagement quantity the direction search measures against the
    /// α/2π target. Defaults to the historical `DiskArea`; `LeadingArc`
    /// is the units-correct measure (algorithm review 2026-06-12, F1).
    #[serde(default)]
    pub engagement_measure: crate::adaptive::EngagementMeasure,
    /// How the main clearing passes are generated: the historical
    /// reactive `Agent`, or the constructive `ContourSpiral` (Stage 1).
    #[serde(default)]
    pub path_strategy: crate::adaptive::PathStrategy2d,
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
            spindle_rpm: None,
            cleanup_strategy: crate::adaptive::CleanupStrategy::ContourParallelHybrid,
            engagement_measure: crate::adaptive::EngagementMeasure::DiskArea,
            path_strategy: crate::adaptive::PathStrategy2d::Agent,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spindle_rpm: Option<u32>,
}

impl Default for VCarveConfig {
    fn default() -> Self {
        Self {
            max_depth: 5.0,
            stepover: 0.5,
            feed_rate: 800.0,
            plunge_rate: 400.0,
            tolerance: 0.05,
            spindle_rpm: None,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spindle_rpm: Option<u32>,
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
            spindle_rpm: None,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spindle_rpm: Option<u32>,
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
            spindle_rpm: None,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spindle_rpm: Option<u32>,
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
            spindle_rpm: None,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spindle_rpm: Option<u32>,
    /// Target scallop (cusp) height in mm for the Suggest pipeline. When
    /// `Some(h)`, the feeds calculator derives `stepover` from the tool's
    /// ball/tapered-ball tip radius via the chord-height formula instead
    /// of the `ae_factor × diameter` default — letting the operator dial a
    /// finish quality directly (e.g. 10 μm scallop → ~0.18 mm step on a
    /// 1 mm ball) rather than a sub-micron formula stepover. `None`
    /// preserves the legacy formula-based stepover and the F-037 smoke
    /// baseline. Has no effect on flat/bull tools (no spherical tip).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scallop_height: Option<f64>,
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
            spindle_rpm: None,
            scallop_height: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Adaptive3dConfig {
    pub stepover: f64,
    pub depth_per_pass: f64,
    /// Deprecated + inert sidewall leave allowance. NOT honored by the
    /// planner: `adaptive3d`'s drop-cutter / dexel heightmap engine only
    /// supports a single vertical (Z) leave offset — see
    /// `compute::execute::adaptive3d_effective_stock_to_leave`, which
    /// consumes `stock_to_leave_axial` alone. The GUI dial for this field
    /// was removed 2026-07-06 (finishing_stack_review_2026-07.md F.1 —
    /// decided against building the wall-offset mechanism). The field is
    /// kept solely so existing project `.toml` files with this key still
    /// deserialize; changing it has no effect on generated toolpaths.
    pub stock_to_leave_radial: f64,
    /// Vertical leave allowance above the surface heightmap. The only
    /// leave-stock dial the `adaptive3d` planner actually applies (see
    /// `stock_to_leave_radial` above).
    pub stock_to_leave_axial: f64,
    pub feed_rate: f64,
    pub plunge_rate: f64,
    pub tolerance: f64,
    pub min_cutting_radius: f64,
    pub entry_style: Adaptive3dEntryStyle,
    /// Ramp entry: maximum descent angle in degrees. Only honored when
    /// `entry_style == Ramp`.
    #[serde(default = "default_adaptive3d_ramp_angle")]
    pub ramp_angle_deg: f64,
    /// Helix entry: helix radius as a fraction of the tool's envelope
    /// diameter. Only honored when `entry_style == Helix`.
    #[serde(default = "default_adaptive3d_helix_radius_factor")]
    pub helix_radius_factor: f64,
    /// Helix entry: vertical pitch in mm. Only honored when `entry_style == Helix`.
    #[serde(default = "default_adaptive3d_helix_pitch")]
    pub helix_pitch: f64,
    pub fine_stepdown: f64,
    pub detect_flat_areas: bool,
    pub region_ordering: RegionOrdering,
    #[serde(default = "default_clearing_strategy")]
    pub clearing_strategy: ClearingStrategy,
    /// Trochoid trigger cap for the ContourSpiral strategy: trochoidal
    /// relief loops fire when predicted leading-arc engagement exceeds
    /// `target × this`. Low (≈1.0–1.2) = flattest load, more loops, more
    /// travel; high (≈2.0–3.0) = relaxed, fewer loops, less travel.
    /// Surfaced in the GUI as the "Nibble" dial. Default 1.6 is the
    /// balanced knee (load still flat, ~25-30% less distance than 1.2 —
    /// see the trochoid-cap sweep in
    /// `planning/ADAPTIVE_CLEARING_ALGO_REVIEW_2026-06-12.md`). Ignored by
    /// the ContourParallel/Adaptive/AgentSearch strategies.
    #[serde(default = "default_trochoid_cap_mult")]
    pub trochoid_cap_mult: f64,
    /// Engagement quantity for the AgentSearch 2D sub-pass. Defaults to
    /// the historical `DiskArea`; `LeadingArc` is the units-correct
    /// measure (algorithm review 2026-06-12, F1). Ignored by the
    /// ContourParallel/Adaptive strategies.
    #[serde(default)]
    pub engagement_measure: crate::adaptive::EngagementMeasure,
    #[serde(default)]
    pub z_blend: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spindle_rpm: Option<u32>,
    /// Insert fine sub-passes within each DPP descent, restricted to
    /// cells whose surface slope is below `shallow_angle_deg`. Steep
    /// areas keep stepping down at `depth_per_pass` as before; shallow
    /// areas get a finer staircase straight from the rough so finishing
    /// passes have less terracing to remove. Off by default.
    #[serde(default)]
    pub mill_shallow_areas: bool,
    /// Slope angle threshold (degrees from horizontal) below which a
    /// cell counts as "shallow." Typical 25-35°. `None` ⇒ defaults to
    /// 30° at planning time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shallow_angle_deg: Option<f64>,
    /// Stepdown within shallow regions. `None` ⇒ defaults to half of
    /// `depth_per_pass` at planning time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shallow_stepdown: Option<f64>,
    /// F-038: minimum total horizontal cutting length (mm) a marching-squares
    /// region must produce in its 2D adaptive sub-pass (perimeter sweep +
    /// adaptive walk) before the planner commits an entry plunge to it.
    /// Regions whose forecast cut length is below this threshold are dropped
    /// — the entry plunge + tiny cut + retract would otherwise spend more
    /// cycle time on travel than on material removal. Default 5.0 mm matches
    /// the Wanaka Back Rough measurement (90/149 plunges cut ≤ 10 mm pre-fix).
    #[serde(default = "default_min_region_cut_length_mm")]
    pub min_region_cut_length_mm: f64,
    /// F-038b: maximum XY distance (mm) to attempt a keep-tool-down link
    /// between cut groups instead of retract-rapid-plunge. `None` falls
    /// back to a planner-side default of 8 × tool diameter (Fusion HSM's
    /// typical "stay down distance" for roughing). `Some(0.0)` disables
    /// the feature.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_stay_down_distance_mm: Option<f64>,
    /// F-038b: vertical clearance (mm) added on top of the maximum mesh
    /// heightfield sample along a stay-down link. Default 0.5 mm absorbs
    /// dexel/mesh discretisation noise.
    #[serde(default = "default_stay_down_clearance_mm")]
    pub stay_down_clearance_mm: f64,
}

fn default_stay_down_clearance_mm() -> f64 {
    // F-038b: matches dexel cell-height resolution at standard sim
    // settings (~0.5 mm). Larger values forfeit cycle-time savings on
    // tight terrain; smaller values risk grazing the surface on
    // imprecise heightfields.
    0.5
}

fn default_min_region_cut_length_mm() -> f64 {
    // Threshold tuned against the Wanaka Back Rough wall-clock measurement
    // (F-038 finding, 2026-05-27). At 5.0 mm the entry-plunge count dropped
    // from 131→93 (29 %); at 10.0 mm it dropped to 65 (50 %); at 15.0 mm
    // it dropped further with no measurable surface-quality regression on
    // the synthetic terrain fixture. The 15.0 mm value keeps the smoke
    // suite green and represents the "amortise entry overhead over at
    // least 2× tool diameter of cut" heuristic for the Wanaka 6 mm tool.
    15.0
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
            entry_style: Adaptive3dEntryStyle::Plunge,
            ramp_angle_deg: default_adaptive3d_ramp_angle(),
            helix_radius_factor: default_adaptive3d_helix_radius_factor(),
            helix_pitch: default_adaptive3d_helix_pitch(),
            fine_stepdown: 0.0,
            detect_flat_areas: false,
            region_ordering: RegionOrdering::Global,
            clearing_strategy: ClearingStrategy::ContourParallel,
            trochoid_cap_mult: default_trochoid_cap_mult(),
            engagement_measure: crate::adaptive::EngagementMeasure::DiskArea,
            z_blend: false,
            spindle_rpm: None,
            mill_shallow_areas: false,
            shallow_angle_deg: None,
            shallow_stepdown: None,
            min_region_cut_length_mm: default_min_region_cut_length_mm(),
            // F-038b: leave the planner to pick 8×diameter at toolpath
            // build time (None) unless the operator explicitly overrides
            // it through the per-op config / CLI flag.
            max_stay_down_distance_mm: None,
            stay_down_clearance_mm: default_stay_down_clearance_mm(),
        }
    }
}

/// Tuned trochoid trigger cap for the ContourSpiral strategy. 1.6 is the
/// balanced knee from the cap sweep — load stays flat (p99 well under the
/// spiky strategies) while cutting ~25-30% less distance than the
/// flattest-load 1.2. Matches `adaptive3d::clearing::TROCHOID_CAP_MULT_3D`
/// (the const this default replaces, kept as the in-engine fallback).
fn default_trochoid_cap_mult() -> f64 {
    1.6
}

fn default_clearing_strategy() -> ClearingStrategy {
    ClearingStrategy::ContourParallel
}

fn default_adaptive3d_ramp_angle() -> f64 {
    10.0
}

fn default_adaptive3d_helix_radius_factor() -> f64 {
    0.3
}

fn default_adaptive3d_helix_pitch() -> f64 {
    2.0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaterlineConfig {
    pub z_step: f64,
    pub sampling: f64,
    pub feed_rate: f64,
    pub plunge_rate: f64,
    pub continuous: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spindle_rpm: Option<u32>,
}

impl Default for WaterlineConfig {
    fn default() -> Self {
        Self {
            z_step: 1.0,
            sampling: 0.5,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            continuous: false,
            spindle_rpm: None,
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
    /// Reach-gap tolerance (mm): minimum uncut valley depth for the tool-radius-
    /// aware gate to keep a concave seam. Higher = ignore shallow surface texture,
    /// keep only deeper channels. `#[serde(default)]` so older project files load.
    #[serde(default = "crate::pencil::reach_gap_threshold")]
    pub min_valley_depth: f64,
    /// Bisector positioning strength (0 = off, 1 = geometrically correct). Shifts
    /// the trace out along the wall bisector in asymmetric corners so the ball
    /// nestles instead of riding up the steep wall. `#[serde(default)]` so older
    /// project files load.
    #[serde(default = "crate::pencil::bisector_strength_default")]
    pub bisector_strength: f64,
    /// Diameter (mm) of the bigger reference (finishing) tool this pencil pass
    /// cleans up after. The gate keeps a seam by how much deeper the pencil tool
    /// reaches than this reference could, so it traces the valleys a bigger bit
    /// missed and skips reachable walls + sub-pencil texture. `#[serde(default)]`
    /// so older project files load.
    #[serde(default = "crate::pencil::reference_tool_diameter_default")]
    pub reference_tool_diameter: f64,
    /// Valley-detection algorithm: `"dihedral"` (mesh crease detection, default)
    /// or `"curvature"` (curvature crest lines, best for noisy organic relief).
    /// `#[serde(default)]` so older project files load.
    #[serde(default = "crate::pencil::detector_string_default")]
    pub detector: String,
    /// Minimum concave curvature |κ₂| (1/mm) a valley must reach for the
    /// `curvature` detector to trace it — the valley significance dial. Low →
    /// every concave seam; high → only deep sharp valleys. `#[serde(default)]`.
    #[serde(default = "crate::pencil::valley_saliency_default")]
    pub valley_saliency: f64,
    /// Curvature-tensor smoothing iterations for the `curvature` detector (the
    /// literature denoise — smooths the curvature field, not the geometry).
    /// `#[serde(default)]` so older project files load.
    #[serde(default = "crate::pencil::curvature_smoothing_default")]
    pub curvature_smoothing: usize,
    /// XY grid cell size (mm) for the `rest_depth` detector's rest field. Smaller
    /// = finer regions, more drops. `#[serde(default)]` so older files load.
    #[serde(default = "crate::pencil::rest_cell_default")]
    pub rest_cell_mm: f64,
    /// **RETIRED (PR-5, H2.2) — deserialized, saved, and NOT READ.**
    ///
    /// It used to be the `rest_depth` routing threshold: a rest region routed
    /// to a pencil centreline when its half-width was
    /// `≤ route_width_factor × pencil_radius`. The pencil/clearing decision
    /// is now the COVERAGE criterion — the reachable band against the fan the
    /// operation can actually emit — because the old rule was fed the SAME
    /// scalar as the offset-pass fit equation, so fixing one broke the other
    /// (`CHECKPOINT_A_EVIDENCE.md` §8.3).
    ///
    /// Kept as a field on purpose: removing it would break every saved
    /// project for a dial that was never load-bearing. A project that carries
    /// a NON-DEFAULT value raises
    /// [`crate::compute::config::DeprecatedDialFinding`] →
    /// `diagnostics::ids::CONFIG_DEPRECATED_DIAL`, so the operator is told
    /// once rather than left with a dial that quietly does nothing.
    #[serde(default = "crate::pencil::route_width_factor_default")]
    pub route_width_factor: f64,
    /// R1: optional library tool id whose *real* cutter geometry defines the
    /// rest reference (all three detectors). `None` = legacy nominal-diameter
    /// behaviour via `reference_tool_diameter`. Mirrors `RestConfig.prev_tool_id`
    /// — `#[serde(default, skip_serializing_if)]` so older project files load
    /// and files that never set it stay byte-identical on save.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference_tool_id: Option<ToolId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spindle_rpm: Option<u32>,
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
            min_valley_depth: crate::pencil::reach_gap_threshold(),
            bisector_strength: crate::pencil::bisector_strength_default(),
            reference_tool_diameter: crate::pencil::reference_tool_diameter_default(),
            detector: crate::pencil::detector_string_default(),
            valley_saliency: crate::pencil::valley_saliency_default(),
            curvature_smoothing: crate::pencil::curvature_smoothing_default(),
            rest_cell_mm: crate::pencil::rest_cell_default(),
            route_width_factor: crate::pencil::route_width_factor_default(),
            reference_tool_id: None,
            spindle_rpm: None,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spindle_rpm: Option<u32>,
    /// A/M7 — cap (mm) on the XY gap a ring-to-ring surface link may span
    /// instead of a full `retract → rapid → replunge` round trip. `0.0`
    /// disables it. See [`crate::scallop::ScallopParams::intra_pass_hookup_mm`]
    /// for what the relink does and refuses to do.
    #[serde(default = "default_scallop_intra_pass_hookup_mm")]
    pub intra_pass_hookup_mm: f64,
}

/// 3.0 mm — a little over one Ø3-ball diameter: far enough to catch
/// adjacent-ring junctions, short enough that a link never crosses a
/// feature it did not machine.
///
/// **ON since wave 12**, on a measurement that took two waves to read
/// correctly. On a corrugated all-over scallop fixture: retract round trips
/// 24 → 3 (−87.5%), integrated cycle time 407.9 s → 166.2 s (−59.3%),
/// swept-footprint throughput 4.68 → 11.47 mm²/s (+144.9%), **zero** new
/// collisions at the finest 0.1 mm grid.
///
/// Wave 11 measured all of that and shipped the dial OFF, because a fourth
/// gate reported that `surface_link::relink_fragments` dropped one cut
/// position per link. It does not. The positions it removes are `LeadOut`
/// arc endpoints, relabelled `FinishingCut` by an arc fitter that groups by
/// feed rate rather than intent — one per link because a link deletes a
/// fragment boundary and a lead-out is what terminates one. See
/// `tests/scallop_intra_pass_relink_am7.rs`, which now gates on surface
/// membership rather than on a label, and on the relinker's own
/// position-preservation unit tests in `crate::surface_link`.
///
/// The value is a CAP, not a target: every candidate within it is still
/// drop-cutter sampled for gouge, refused if it would leave the operation's
/// boundary, and (with kinematics) kept only when it beats the retract it
/// replaces.
fn default_scallop_intra_pass_hookup_mm() -> f64 {
    3.0
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
            spindle_rpm: None,
            intra_pass_hookup_mm: default_scallop_intra_pass_hookup_mm(),
        }
    }
}

/// P2.c orchestrator config (`planning/unified_finish_planner_design.md`):
/// bands the surface by true-surface slope and runs waterline/scallop/raster
/// per band. Field defaults mirror the standalone `ScallopConfig` /
/// `WaterlineConfig` / `DropCutterConfig` so switching between the
/// standalone three-op stack and this orchestrator at the same tool
/// doesn't silently change feeds/quality (see
/// `unified_finish::UnifiedFinishParams::default`'s doc comment).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedFinishConfig {
    /// Slope entering the mid-steep scallop band (deg from horizontal).
    /// Primary new dial — the raster→scallop boundary.
    pub steep_threshold_deg: f64,
    /// Slope entering the very-steep waterline band (deg). Advanced
    /// dial — the scallop→waterline boundary.
    pub waterline_threshold_deg: f64,
    /// Band overlap at polygon extraction (mm) — `steep_shallow`'s
    /// `overlap_distance` analogue.
    pub overlap_mm: f64,
    pub scallop_height: f64,
    pub tolerance: f64,
    pub raster_stepover: f64,
    pub z_step: f64,
    pub sampling: f64,
    pub stock_to_leave: f64,
    pub feed_rate: f64,
    pub plunge_rate: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spindle_rpm: Option<u32>,
    /// v3 S1 claims pipeline (`planning/unified_v3_design.md` §2.1): run
    /// the rest-depth detector inside this op's own generation (against
    /// its own tip cutter) and let claimed creases cut a pencil pass
    /// ADDITIVELY alongside the banded regions (S1 — corridors are not
    /// carved). `false` reproduces the pre-v3 op exactly — no detector
    /// run, no crease claims, no crease node.
    ///
    /// Default `false` (EXPERIMENTAL): the 2026-07-13 A/B measured the
    /// self-probe crease signal as self-defeating for a single-tool op —
    /// the float field marks exactly the valleys the tool cannot reach,
    /// so the crease node cost +22.5% finish time for zero measured
    /// quality gain (on-size share unchanged to the column). A follow-on
    /// region-level territory filter (informally "S2") was tried and
    /// measured ineffective on real terrain (see `min_rest_depth_mm` and
    /// `unified_finish` module doc); `territory_clip` below is the
    /// mechanism that actually delivers rest-territory confinement.
    #[serde(default = "default_unified_finish_pencil_claims")]
    pub pencil_claims: bool,
    /// Territory gate (mm) for the rest-island MEASUREMENT (design doc
    /// §2.1 step 4): under a machined-stock reference, cells where the
    /// measured rest (stock top − pencil drop) is below this are the
    /// skippable territory `territory_clip` (S4) masks into coverage
    /// BEFORE `decompose` runs. An earlier region-level drop filter
    /// (informally "S2") consumed a separate per-cell verdict grid built
    /// from this same threshold and was removed 2026-07-27: it measured
    /// dead on both sides of `territory_clip` (see `unified_finish`
    /// module doc for the full account). Default 0.02mm.
    #[serde(default = "default_unified_finish_min_rest_depth_mm")]
    pub min_rest_depth_mm: f64,
    /// Which reference the crease detector runs against — a three-valued
    /// dial since A/M6 (`unified_finish::ClaimsReference` doc), resolved to
    /// the two-valued `CreaseReference` at generation time by
    /// `ClaimsReferenceResolution::resolve` and recorded in
    /// [`crate::compute::config::ClaimsReferenceFinding`].
    ///
    /// `Auto` (**the default since A/M6**) uses the machined prior stock when
    /// one is in scope and the analytic self-probe when none is. `SelfProbe`
    /// and `MachinedStock` pin the choice; both keep their exact pre-A/M6
    /// wire names and meanings, so no existing project file changes
    /// behaviour. Only an ABSENT field is affected, and only for an
    /// operation running `pencil_claims` over remaining stock — which is the
    /// case A/M6 measured at −88.7% cutting once corrected.
    ///
    /// Pin `SelfProbe` on a rough→finish chain: a stock-referenced detector
    /// reads roughing terraces as a phantom dendritic crease network (the S1
    /// lesson). `Auto` cannot see the difference — it keys on the presence of
    /// a prior stock, not on its quality — and says so in its finding.
    #[serde(default = "default_unified_finish_claims_reference")]
    pub claims_reference: ClaimsReference,
    /// S4 rest-territory CONFINEMENT (`unified_finish::ClaimsConfig::
    /// territory_clip` doc, process-proof build-list, `planning/
    /// unified_v3_design.md` §0.a / §2.1 step 4): AND a per-cell rest
    /// keep-mask (built from the claims detector's own stock-referenced
    /// rest field, thresholded at `min_rest_depth_mm`) into coverage
    /// BEFORE `decompose` runs, so the conditioning pipeline itself
    /// normalizes the rest islands and every emitted band region IS a
    /// conditioned rest island. Landed because an earlier region-level
    /// whole-island keep-or-drop filter couldn't shrink a giant
    /// conditioned island that merely CONTAINS above-dial rest
    /// somewhere — the wanaka ×2 cascade A/B measured Op B at +47% over
    /// the all-over-tip baseline for exactly that reason (unified_finish
    /// module doc). Meaningful only alongside `pencil_claims = true` and
    /// `claims_reference = MachinedStock`; under `SelfProbe` the
    /// orchestrator warns and skips clipping (the "rest islands" there are
    /// geometric, not material). Default `false` = off, byte-identical to
    /// the pre-S4 op.
    #[serde(default = "default_unified_finish_territory_clip")]
    pub territory_clip: bool,
    /// INTRA-region stay-down linking (`planning/unified_v3_design.md`
    /// §9): the maximum XY gap (mm) a gouge-checked, surface-following
    /// link may span between two consecutive cut fragments inside ONE
    /// region. `0.0` disables the pass and reproduces the previous op
    /// byte-for-byte.
    ///
    /// The router has always linked BETWEEN regions this way. Within a
    /// region every fragment junction still paid a full retract-to-safe-Z
    /// round trip, and §9 measured that as the dominant cost: on wanaka ×2
    /// the unified rest-clearer makes 12 780 of them for 17 077 s of
    /// rapids, of which the XY hop — the only part reordering can shorten
    /// — is roughly a tenth.
    ///
    /// A candidate must be within this gap AND gouge-safe (every sampled
    /// point keeps surface contact) AND, when machine kinematics are in
    /// scope, actually faster than the retract it replaces.
    #[serde(default = "default_unified_finish_intra_region_hookup_mm")]
    pub intra_region_hookup_mm: f64,
    /// XY gap (mm) the crease-claims node's emitter may bridge with a
    /// stay-down surface feed instead of retracting. See
    /// [`crate::unified_finish::ClaimsConfig::crease_hookup_mm`] — that
    /// emitter links with NO territory boundary, so this is the only lever
    /// on crease links leaving their rest island. Meaningful only
    /// alongside `pencil_claims = true`. Default 5.0 (the historical
    /// `PencilParams::default()` value); `0.0` disables crease linking.
    #[serde(default = "default_unified_finish_crease_hookup_mm")]
    pub crease_hookup_mm: f64,
    /// Which sampler builds this op's classification grid (M3 wave 7b —
    /// `planning/review_2026-07-29/CLASSIFICATION_PERF_STUDY.md`).
    ///
    /// **Not a quality or speed dial for users to tune.** It exists so the
    /// M3 COLUMNS A/B can drive the pre-switch drop-cutter classifier and the
    /// production tile-raster one through the identical pipeline, and so a
    /// project that hits a regression has a documented escape hatch that does
    /// not need a rebuild.
    ///
    /// Defaults to [`ClassificationSampler::PRODUCTION`] and is omitted from
    /// serialisation while it holds that value, so no existing project file
    /// changes and no newly written one grows a key unless it deliberately
    /// pinned a non-production sampler. An absent field loads as production.
    #[serde(
        default,
        skip_serializing_if = "crate::classify_probe::ClassificationSampler::is_production"
    )]
    pub classification_sampler: crate::classify_probe::ClassificationSampler,
}

impl Default for UnifiedFinishConfig {
    /// Thresholds locked by the P2.e sweep (2026-07-08) — mirrors
    /// `FinishPlannerParams::for_tool` (see its doc comment for the
    /// measured tradeoffs; waterline 75 because Z-contouring is the most
    /// expensive strategy per area, and 65→55 measured +27% finish time).
    fn default() -> Self {
        Self {
            steep_threshold_deg: 45.0,
            waterline_threshold_deg: 75.0,
            overlap_mm: 2.0,
            scallop_height: 0.1,
            tolerance: 0.05,
            raster_stepover: 1.0,
            z_step: 1.0,
            sampling: 0.5,
            stock_to_leave: 0.0,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            spindle_rpm: None,
            pencil_claims: default_unified_finish_pencil_claims(),
            min_rest_depth_mm: default_unified_finish_min_rest_depth_mm(),
            claims_reference: default_unified_finish_claims_reference(),
            territory_clip: default_unified_finish_territory_clip(),
            intra_region_hookup_mm: default_unified_finish_intra_region_hookup_mm(),
            crease_hookup_mm: default_unified_finish_crease_hookup_mm(),
            classification_sampler: crate::classify_probe::ClassificationSampler::PRODUCTION,
        }
    }
}

fn default_unified_finish_pencil_claims() -> bool {
    false
}

fn default_unified_finish_min_rest_depth_mm() -> f64 {
    0.02
}

/// A/M6: **`Auto`, not `SelfProbe`.**
///
/// The decision, and why it is not "keep the old default and warn":
/// `self_probe` is the right reference for a first finish op and the wrong
/// one for a rest op in a cascade, so the correct value is contextual and a
/// fixed default is wrong half the time by construction. Deriving it makes
/// the common case right; the two explicit variants remain for the case the
/// derivation cannot see (a rough prior).
///
/// The blast radius is deliberately small. `pencil_claims` defaults `false`,
/// and with it off this dial is inert — no claims pipeline runs at all — so
/// the only projects whose behaviour can move are those that turned claims
/// on AND left `claims_reference` unwritten AND cut remaining stock. Any
/// project file that names a value keeps it (`ClaimsReference` doc: the two
/// pre-A/M6 wire names are unchanged), and every resolution is recorded in
/// [`crate::compute::config::ClaimsReferenceFinding`], so a behaviour change
/// arrives with its own explanation rather than silently.
fn default_unified_finish_claims_reference() -> ClaimsReference {
    ClaimsReference::Auto
}

fn default_unified_finish_territory_clip() -> bool {
    false
}

/// **ON at 6.0 mm since 2026-08-03 — an operator's call, on measured
/// evidence.**
///
/// This dial was default-OFF through waves 11 and 12 on the rule "flip after
/// the measurement, not before it". Wave 12 took the measurement, on the
/// synthetic two-groove plateau, and cleared the blocker wave 11 had cited
/// (`relink_fragments` does not drop cut positions — bisected four ways):
///
/// | | off | on (6.0 mm) |
/// |---|---|---|
/// | moves | 1437 | 1301 |
/// | retract trips | 85 | **8** (−90.6%) |
/// | cycle time | 238.94 s | **130.65 s** (−45.3%) |
/// | swept footprint | 850 mm² | **850 mm²** (identical) |
/// | mm²/s | 3.5574 | **6.5061** (+82.9%) |
///
/// Wave 12 still declined to flip it, and was right to: unlike scallop's
/// ring-to-ring case this is a region-level trade whose value depends on how
/// the planner's router links regions afterwards, and that belongs to an
/// operator with a real part in front of them rather than to a synthetic
/// plateau. **That operator was asked directly, in the Checkpoint D session
/// (2026-08-03), with the table above as the evidence, and answered: ON at
/// 6.0** — to be re-checked at the end-of-programme live validation.
///
/// So the authority for this value is not a measurement and not a default
/// anyone drifted into; it is the ruling wave 12 explicitly deferred to.
/// The safety invariant travels with it: keeping the tool down must never ADD
/// retract round trips
/// (`retract_trip_channel_am7::intra_region_hookup_ships_on_by_operator_ruling`).
fn default_unified_finish_intra_region_hookup_mm() -> f64 {
    6.0
}

/// Historical `PencilParams::default().hookup_distance`: what every
/// crease-node measurement before 2026-07-27 ran with.
fn default_unified_finish_crease_hookup_mm() -> f64 {
    5.0
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spindle_rpm: Option<u32>,
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
            spindle_rpm: None,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spindle_rpm: Option<u32>,
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
            spindle_rpm: None,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spindle_rpm: Option<u32>,
}

impl Default for SpiralFinishConfig {
    fn default() -> Self {
        Self {
            stepover: 1.0,
            direction: SpiralDirection::InsideOut,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            stock_to_leave: 0.0,
            spindle_rpm: None,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spindle_rpm: Option<u32>,
}

impl Default for RadialFinishConfig {
    fn default() -> Self {
        Self {
            angular_step: 5.0,
            point_spacing: 0.5,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            stock_to_leave: 0.0,
            spindle_rpm: None,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spindle_rpm: Option<u32>,
}

impl Default for HorizontalFinishConfig {
    fn default() -> Self {
        Self {
            angle_threshold: 5.0,
            stepover: 1.0,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            stock_to_leave: 0.0,
            spindle_rpm: None,
        }
    }
}

/// Which side of the mesh to project the curve onto.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ProjectCurveDirection {
    /// Project from above — tool contacts the top surface.
    #[default]
    FromAbove,
    /// Project from below — tool contacts the bottom surface.
    FromBelow,
}

impl ProjectCurveDirection {
    pub fn label(self) -> &'static str {
        match self {
            Self::FromAbove => "From Above",
            Self::FromBelow => "From Below",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectCurveSide {
    /// Tool centerline rides exactly on the curve.
    #[default]
    Center,
    /// Closed rings only — offset inward by tool radius before projecting.
    /// Open rings fall back to Center.
    Inside,
    /// Closed rings only — offset outward by tool radius before projecting.
    /// Open rings fall back to Center.
    Outside,
}

impl ProjectCurveSide {
    pub fn label(self) -> &'static str {
        match self {
            Self::Center => "On Line",
            Self::Inside => "Inside",
            Self::Outside => "Outside",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectCurveConfig {
    pub depth: f64,
    pub point_spacing: f64,
    pub feed_rate: f64,
    pub plunge_rate: f64,
    /// Optional separate surface model for projection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surface_model_id: Option<super::stock_config::ModelId>,
    /// Project from above (Z-down, default) or below (Z-up).
    #[serde(default)]
    pub direction: ProjectCurveDirection,
    /// Tool radius compensation side. Closed rings only; open rings ignore it.
    #[serde(default)]
    pub side: ProjectCurveSide,
    /// Set by the compute pipeline when the mesh has already been Z-inverted
    /// by a bottom-facing setup transform. Not persisted.
    #[serde(skip)]
    pub setup_z_flipped: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spindle_rpm: Option<u32>,
}

impl Default for ProjectCurveConfig {
    fn default() -> Self {
        Self {
            depth: 1.0,
            point_spacing: 0.5,
            feed_rate: 800.0,
            plunge_rate: 400.0,
            surface_model_id: None,
            direction: ProjectCurveDirection::FromAbove,
            side: ProjectCurveSide::Center,
            setup_z_flipped: false,
            spindle_rpm: None,
        }
    }
}

// ---------------------------------------------------------------------------
// OperationParams trait implementations
// ---------------------------------------------------------------------------

impl OperationParams for FaceConfig {
    fn feed_rate(&self) -> f64 {
        self.feed_rate
    }
    fn set_feed_rate(&mut self, value: f64) {
        self.feed_rate = value;
    }
    fn plunge_rate(&self) -> f64 {
        self.plunge_rate
    }
    fn set_plunge_rate(&mut self, value: f64) {
        self.plunge_rate = value;
    }
    fn stepover(&self) -> Option<f64> {
        Some(self.stepover)
    }
    fn set_stepover(&mut self, value: f64) {
        self.stepover = value;
    }
    fn depth_per_pass(&self) -> Option<f64> {
        Some(self.depth_per_pass)
    }
    fn set_depth_per_pass(&mut self, value: f64) {
        self.depth_per_pass = value;
    }
    fn depth_semantics(&self) -> DepthSemantics {
        DepthSemantics::Explicit(self.depth)
    }
    fn spindle_rpm(&self) -> Option<u32> {
        self.spindle_rpm
    }
    fn set_spindle_rpm(&mut self, rpm: Option<u32>) {
        self.spindle_rpm = rpm;
    }
}

impl OperationParams for PocketConfig {
    fn feed_rate(&self) -> f64 {
        self.feed_rate
    }
    fn set_feed_rate(&mut self, value: f64) {
        self.feed_rate = value;
    }
    fn plunge_rate(&self) -> f64 {
        self.plunge_rate
    }
    fn set_plunge_rate(&mut self, value: f64) {
        self.plunge_rate = value;
    }
    fn stepover(&self) -> Option<f64> {
        Some(self.stepover)
    }
    fn set_stepover(&mut self, value: f64) {
        self.stepover = value;
    }
    fn depth_per_pass(&self) -> Option<f64> {
        Some(self.depth_per_pass)
    }
    fn set_depth_per_pass(&mut self, value: f64) {
        self.depth_per_pass = value;
    }
    fn depth_semantics(&self) -> DepthSemantics {
        DepthSemantics::Explicit(self.depth)
    }
    fn spindle_rpm(&self) -> Option<u32> {
        self.spindle_rpm
    }
    fn set_spindle_rpm(&mut self, rpm: Option<u32>) {
        self.spindle_rpm = rpm;
    }
}

impl OperationParams for ProfileConfig {
    fn feed_rate(&self) -> f64 {
        self.feed_rate
    }
    fn set_feed_rate(&mut self, value: f64) {
        self.feed_rate = value;
    }
    fn plunge_rate(&self) -> f64 {
        self.plunge_rate
    }
    fn set_plunge_rate(&mut self, value: f64) {
        self.plunge_rate = value;
    }
    fn depth_per_pass(&self) -> Option<f64> {
        Some(self.depth_per_pass)
    }
    fn set_depth_per_pass(&mut self, value: f64) {
        self.depth_per_pass = value;
    }
    fn depth_semantics(&self) -> DepthSemantics {
        DepthSemantics::Explicit(self.depth)
    }
    fn spindle_rpm(&self) -> Option<u32> {
        self.spindle_rpm
    }
    fn set_spindle_rpm(&mut self, rpm: Option<u32>) {
        self.spindle_rpm = rpm;
    }
}

impl OperationParams for AdaptiveConfig {
    fn feed_rate(&self) -> f64 {
        self.feed_rate
    }
    fn set_feed_rate(&mut self, value: f64) {
        self.feed_rate = value;
    }
    fn plunge_rate(&self) -> f64 {
        self.plunge_rate
    }
    fn set_plunge_rate(&mut self, value: f64) {
        self.plunge_rate = value;
    }
    fn stepover(&self) -> Option<f64> {
        Some(self.stepover)
    }
    fn set_stepover(&mut self, value: f64) {
        self.stepover = value;
    }
    fn depth_per_pass(&self) -> Option<f64> {
        Some(self.depth_per_pass)
    }
    fn set_depth_per_pass(&mut self, value: f64) {
        self.depth_per_pass = value;
    }
    fn depth_semantics(&self) -> DepthSemantics {
        DepthSemantics::Explicit(self.depth)
    }
    fn spindle_rpm(&self) -> Option<u32> {
        self.spindle_rpm
    }
    fn set_spindle_rpm(&mut self, rpm: Option<u32>) {
        self.spindle_rpm = rpm;
    }
}

impl OperationParams for VCarveConfig {
    fn feed_rate(&self) -> f64 {
        self.feed_rate
    }
    fn set_feed_rate(&mut self, value: f64) {
        self.feed_rate = value;
    }
    fn plunge_rate(&self) -> f64 {
        self.plunge_rate
    }
    fn set_plunge_rate(&mut self, value: f64) {
        self.plunge_rate = value;
    }
    fn stepover(&self) -> Option<f64> {
        Some(self.stepover)
    }
    fn set_stepover(&mut self, value: f64) {
        self.stepover = value;
    }
    fn depth_semantics(&self) -> DepthSemantics {
        DepthSemantics::Explicit(self.max_depth)
    }
    fn spindle_rpm(&self) -> Option<u32> {
        self.spindle_rpm
    }
    fn set_spindle_rpm(&mut self, rpm: Option<u32>) {
        self.spindle_rpm = rpm;
    }
}

impl OperationParams for RestConfig {
    fn feed_rate(&self) -> f64 {
        self.feed_rate
    }
    fn set_feed_rate(&mut self, value: f64) {
        self.feed_rate = value;
    }
    fn plunge_rate(&self) -> f64 {
        self.plunge_rate
    }
    fn set_plunge_rate(&mut self, value: f64) {
        self.plunge_rate = value;
    }
    fn stepover(&self) -> Option<f64> {
        Some(self.stepover)
    }
    fn set_stepover(&mut self, value: f64) {
        self.stepover = value;
    }
    fn depth_per_pass(&self) -> Option<f64> {
        Some(self.depth_per_pass)
    }
    fn set_depth_per_pass(&mut self, value: f64) {
        self.depth_per_pass = value;
    }
    fn depth_semantics(&self) -> DepthSemantics {
        DepthSemantics::Explicit(self.depth)
    }
    fn spindle_rpm(&self) -> Option<u32> {
        self.spindle_rpm
    }
    fn set_spindle_rpm(&mut self, rpm: Option<u32>) {
        self.spindle_rpm = rpm;
    }
}

impl OperationParams for InlayConfig {
    fn feed_rate(&self) -> f64 {
        self.feed_rate
    }
    fn set_feed_rate(&mut self, value: f64) {
        self.feed_rate = value;
    }
    fn plunge_rate(&self) -> f64 {
        self.plunge_rate
    }
    fn set_plunge_rate(&mut self, value: f64) {
        self.plunge_rate = value;
    }
    fn stepover(&self) -> Option<f64> {
        Some(self.stepover)
    }
    fn set_stepover(&mut self, value: f64) {
        self.stepover = value;
    }
    fn depth_semantics(&self) -> DepthSemantics {
        DepthSemantics::Explicit(self.pocket_depth)
    }
    fn spindle_rpm(&self) -> Option<u32> {
        self.spindle_rpm
    }
    fn set_spindle_rpm(&mut self, rpm: Option<u32>) {
        self.spindle_rpm = rpm;
    }
}

impl OperationParams for ZigzagConfig {
    fn feed_rate(&self) -> f64 {
        self.feed_rate
    }
    fn set_feed_rate(&mut self, value: f64) {
        self.feed_rate = value;
    }
    fn plunge_rate(&self) -> f64 {
        self.plunge_rate
    }
    fn set_plunge_rate(&mut self, value: f64) {
        self.plunge_rate = value;
    }
    fn stepover(&self) -> Option<f64> {
        Some(self.stepover)
    }
    fn set_stepover(&mut self, value: f64) {
        self.stepover = value;
    }
    fn depth_per_pass(&self) -> Option<f64> {
        Some(self.depth_per_pass)
    }
    fn set_depth_per_pass(&mut self, value: f64) {
        self.depth_per_pass = value;
    }
    fn depth_semantics(&self) -> DepthSemantics {
        DepthSemantics::Explicit(self.depth)
    }
    fn spindle_rpm(&self) -> Option<u32> {
        self.spindle_rpm
    }
    fn set_spindle_rpm(&mut self, rpm: Option<u32>) {
        self.spindle_rpm = rpm;
    }
}

impl OperationParams for TraceConfig {
    fn feed_rate(&self) -> f64 {
        self.feed_rate
    }
    fn set_feed_rate(&mut self, value: f64) {
        self.feed_rate = value;
    }
    fn plunge_rate(&self) -> f64 {
        self.plunge_rate
    }
    fn set_plunge_rate(&mut self, value: f64) {
        self.plunge_rate = value;
    }
    fn depth_per_pass(&self) -> Option<f64> {
        Some(self.depth_per_pass)
    }
    fn set_depth_per_pass(&mut self, value: f64) {
        self.depth_per_pass = value;
    }
    fn depth_semantics(&self) -> DepthSemantics {
        DepthSemantics::Explicit(self.depth)
    }
    fn spindle_rpm(&self) -> Option<u32> {
        self.spindle_rpm
    }
    fn set_spindle_rpm(&mut self, rpm: Option<u32>) {
        self.spindle_rpm = rpm;
    }
}

impl OperationParams for DrillConfig {
    fn feed_rate(&self) -> f64 {
        self.feed_rate
    }
    fn set_feed_rate(&mut self, value: f64) {
        self.feed_rate = value;
    }
    /// Drill ops are purely vertical -- feed_rate IS the plunge rate.
    fn plunge_rate(&self) -> f64 {
        self.feed_rate
    }
    fn set_plunge_rate(&mut self, value: f64) {
        self.feed_rate = value;
    }
    fn depth_semantics(&self) -> DepthSemantics {
        DepthSemantics::Explicit(self.depth)
    }
    fn spindle_rpm(&self) -> Option<u32> {
        self.spindle_rpm
    }
    fn set_spindle_rpm(&mut self, rpm: Option<u32>) {
        self.spindle_rpm = rpm;
    }
}

impl OperationParams for AlignmentPinDrillConfig {
    fn feed_rate(&self) -> f64 {
        self.feed_rate
    }
    fn set_feed_rate(&mut self, value: f64) {
        self.feed_rate = value;
    }
    /// Drill ops are purely vertical -- feed_rate IS the plunge rate.
    fn plunge_rate(&self) -> f64 {
        self.feed_rate
    }
    fn set_plunge_rate(&mut self, value: f64) {
        self.feed_rate = value;
    }
    fn depth_semantics(&self) -> DepthSemantics {
        DepthSemantics::Explicit(self.spoilboard_penetration)
    }
    fn spindle_rpm(&self) -> Option<u32> {
        self.spindle_rpm
    }
    fn set_spindle_rpm(&mut self, rpm: Option<u32>) {
        self.spindle_rpm = rpm;
    }
}

impl OperationParams for ChamferConfig {
    fn feed_rate(&self) -> f64 {
        self.feed_rate
    }
    fn set_feed_rate(&mut self, value: f64) {
        self.feed_rate = value;
    }
    fn plunge_rate(&self) -> f64 {
        self.plunge_rate
    }
    fn set_plunge_rate(&mut self, value: f64) {
        self.plunge_rate = value;
    }
    fn depth_semantics(&self) -> DepthSemantics {
        DepthSemantics::Explicit(self.chamfer_width)
    }
    fn spindle_rpm(&self) -> Option<u32> {
        self.spindle_rpm
    }
    fn set_spindle_rpm(&mut self, rpm: Option<u32>) {
        self.spindle_rpm = rpm;
    }
}

impl OperationParams for DropCutterConfig {
    fn feed_rate(&self) -> f64 {
        self.feed_rate
    }
    fn set_feed_rate(&mut self, value: f64) {
        self.feed_rate = value;
    }
    fn plunge_rate(&self) -> f64 {
        self.plunge_rate
    }
    fn set_plunge_rate(&mut self, value: f64) {
        self.plunge_rate = value;
    }
    fn stepover(&self) -> Option<f64> {
        Some(self.stepover)
    }
    fn set_stepover(&mut self, value: f64) {
        self.stepover = value;
    }
    fn scallop_height(&self) -> Option<f64> {
        self.scallop_height
    }
    fn set_scallop_height(&mut self, value: f64) {
        self.scallop_height = Some(value);
    }
    fn depth_semantics(&self) -> DepthSemantics {
        DepthSemantics::DerivedStockTop(self.min_z.abs())
    }
    fn spindle_rpm(&self) -> Option<u32> {
        self.spindle_rpm
    }
    fn set_spindle_rpm(&mut self, rpm: Option<u32>) {
        self.spindle_rpm = rpm;
    }
}

impl OperationParams for Adaptive3dConfig {
    fn feed_rate(&self) -> f64 {
        self.feed_rate
    }
    fn set_feed_rate(&mut self, value: f64) {
        self.feed_rate = value;
    }
    fn plunge_rate(&self) -> f64 {
        self.plunge_rate
    }
    fn set_plunge_rate(&mut self, value: f64) {
        self.plunge_rate = value;
    }
    fn stepover(&self) -> Option<f64> {
        Some(self.stepover)
    }
    fn set_stepover(&mut self, value: f64) {
        self.stepover = value;
    }
    fn depth_per_pass(&self) -> Option<f64> {
        Some(self.depth_per_pass)
    }
    fn set_depth_per_pass(&mut self, value: f64) {
        self.depth_per_pass = value;
    }
    fn depth_semantics(&self) -> DepthSemantics {
        DepthSemantics::None
    }
    fn spindle_rpm(&self) -> Option<u32> {
        self.spindle_rpm
    }
    fn set_spindle_rpm(&mut self, rpm: Option<u32>) {
        self.spindle_rpm = rpm;
    }
}

impl OperationParams for WaterlineConfig {
    fn feed_rate(&self) -> f64 {
        self.feed_rate
    }
    fn set_feed_rate(&mut self, value: f64) {
        self.feed_rate = value;
    }
    fn plunge_rate(&self) -> f64 {
        self.plunge_rate
    }
    fn set_plunge_rate(&mut self, value: f64) {
        self.plunge_rate = value;
    }
    /// Z step between successive horizontal contour passes — same
    /// physical role as `depth_per_pass` for 2.5D ops (axial cutter
    /// engagement on each pass). G3 (2026-05-08) surfaces this so
    /// Stage 1 can sweep it.
    fn depth_per_pass(&self) -> Option<f64> {
        Some(self.z_step)
    }
    fn set_depth_per_pass(&mut self, value: f64) {
        self.z_step = value;
    }
    fn depth_semantics(&self) -> DepthSemantics {
        DepthSemantics::None
    }
    fn spindle_rpm(&self) -> Option<u32> {
        self.spindle_rpm
    }
    fn set_spindle_rpm(&mut self, rpm: Option<u32>) {
        self.spindle_rpm = rpm;
    }
}

impl OperationParams for PencilConfig {
    fn feed_rate(&self) -> f64 {
        self.feed_rate
    }
    fn set_feed_rate(&mut self, value: f64) {
        self.feed_rate = value;
    }
    fn plunge_rate(&self) -> f64 {
        self.plunge_rate
    }
    fn set_plunge_rate(&mut self, value: f64) {
        self.plunge_rate = value;
    }
    /// Pencil's `offset_stepover` only takes effect when
    /// `num_offset_passes > 1`. Single-pass pencil has no spacing knob;
    /// returning `None` in that case keeps Stage 1 from generating
    /// duplicate sims. G3 (2026-05-08).
    fn stepover(&self) -> Option<f64> {
        if self.num_offset_passes > 1 {
            Some(self.offset_stepover)
        } else {
            None
        }
    }
    fn set_stepover(&mut self, value: f64) {
        self.offset_stepover = value;
    }
    fn depth_semantics(&self) -> DepthSemantics {
        DepthSemantics::None
    }
    fn spindle_rpm(&self) -> Option<u32> {
        self.spindle_rpm
    }
    fn set_spindle_rpm(&mut self, rpm: Option<u32>) {
        self.spindle_rpm = rpm;
    }
}

impl OperationParams for ScallopConfig {
    fn feed_rate(&self) -> f64 {
        self.feed_rate
    }
    fn set_feed_rate(&mut self, value: f64) {
        self.feed_rate = value;
    }
    fn plunge_rate(&self) -> f64 {
        self.plunge_rate
    }
    fn set_plunge_rate(&mut self, value: f64) {
        self.plunge_rate = value;
    }
    fn scallop_height(&self) -> Option<f64> {
        Some(self.scallop_height)
    }
    fn set_scallop_height(&mut self, value: f64) {
        self.scallop_height = value;
    }
    fn depth_semantics(&self) -> DepthSemantics {
        DepthSemantics::None
    }
    fn spindle_rpm(&self) -> Option<u32> {
        self.spindle_rpm
    }
    fn set_spindle_rpm(&mut self, rpm: Option<u32>) {
        self.spindle_rpm = rpm;
    }
}

impl OperationParams for UnifiedFinishConfig {
    fn feed_rate(&self) -> f64 {
        self.feed_rate
    }
    fn set_feed_rate(&mut self, value: f64) {
        self.feed_rate = value;
    }
    fn plunge_rate(&self) -> f64 {
        self.plunge_rate
    }
    fn set_plunge_rate(&mut self, value: f64) {
        self.plunge_rate = value;
    }
    fn scallop_height(&self) -> Option<f64> {
        Some(self.scallop_height)
    }
    fn set_scallop_height(&mut self, value: f64) {
        self.scallop_height = value;
    }
    fn depth_semantics(&self) -> DepthSemantics {
        DepthSemantics::None
    }
    fn spindle_rpm(&self) -> Option<u32> {
        self.spindle_rpm
    }
    fn set_spindle_rpm(&mut self, rpm: Option<u32>) {
        self.spindle_rpm = rpm;
    }
}

impl OperationParams for SteepShallowConfig {
    fn feed_rate(&self) -> f64 {
        self.feed_rate
    }
    fn set_feed_rate(&mut self, value: f64) {
        self.feed_rate = value;
    }
    fn plunge_rate(&self) -> f64 {
        self.plunge_rate
    }
    fn set_plunge_rate(&mut self, value: f64) {
        self.plunge_rate = value;
    }
    fn stepover(&self) -> Option<f64> {
        Some(self.stepover)
    }
    fn set_stepover(&mut self, value: f64) {
        self.stepover = value;
    }
    fn depth_semantics(&self) -> DepthSemantics {
        DepthSemantics::None
    }
    fn spindle_rpm(&self) -> Option<u32> {
        self.spindle_rpm
    }
    fn set_spindle_rpm(&mut self, rpm: Option<u32>) {
        self.spindle_rpm = rpm;
    }
}

impl OperationParams for RampFinishConfig {
    fn feed_rate(&self) -> f64 {
        self.feed_rate
    }
    fn set_feed_rate(&mut self, value: f64) {
        self.feed_rate = value;
    }
    fn plunge_rate(&self) -> f64 {
        self.plunge_rate
    }
    fn set_plunge_rate(&mut self, value: f64) {
        self.plunge_rate = value;
    }
    /// Maximum Z descent per ramp pass. Same physical role as
    /// `depth_per_pass` (axial cutter engagement) — `max_stepdown` is a
    /// cap; the planner uses `min(max_stepdown, slope-derived)` as the
    /// effective per-pass descent. Sweeping the cap moves the actual
    /// descent for every pass that hits the cap, so it's a meaningful
    /// Stage 1 axis. G3 (2026-05-08).
    fn depth_per_pass(&self) -> Option<f64> {
        Some(self.max_stepdown)
    }
    fn set_depth_per_pass(&mut self, value: f64) {
        self.max_stepdown = value;
    }
    fn depth_semantics(&self) -> DepthSemantics {
        DepthSemantics::None
    }
    fn spindle_rpm(&self) -> Option<u32> {
        self.spindle_rpm
    }
    fn set_spindle_rpm(&mut self, rpm: Option<u32>) {
        self.spindle_rpm = rpm;
    }
}

impl OperationParams for SpiralFinishConfig {
    fn feed_rate(&self) -> f64 {
        self.feed_rate
    }
    fn set_feed_rate(&mut self, value: f64) {
        self.feed_rate = value;
    }
    fn plunge_rate(&self) -> f64 {
        self.plunge_rate
    }
    fn set_plunge_rate(&mut self, value: f64) {
        self.plunge_rate = value;
    }
    fn stepover(&self) -> Option<f64> {
        Some(self.stepover)
    }
    fn set_stepover(&mut self, value: f64) {
        self.stepover = value;
    }
    fn depth_semantics(&self) -> DepthSemantics {
        DepthSemantics::None
    }
    fn spindle_rpm(&self) -> Option<u32> {
        self.spindle_rpm
    }
    fn set_spindle_rpm(&mut self, rpm: Option<u32>) {
        self.spindle_rpm = rpm;
    }
}

impl OperationParams for RadialFinishConfig {
    fn feed_rate(&self) -> f64 {
        self.feed_rate
    }
    fn set_feed_rate(&mut self, value: f64) {
        self.feed_rate = value;
    }
    fn plunge_rate(&self) -> f64 {
        self.plunge_rate
    }
    fn set_plunge_rate(&mut self, value: f64) {
        self.plunge_rate = value;
    }
    fn depth_semantics(&self) -> DepthSemantics {
        DepthSemantics::None
    }
    fn spindle_rpm(&self) -> Option<u32> {
        self.spindle_rpm
    }
    fn set_spindle_rpm(&mut self, rpm: Option<u32>) {
        self.spindle_rpm = rpm;
    }
}

impl OperationParams for HorizontalFinishConfig {
    fn feed_rate(&self) -> f64 {
        self.feed_rate
    }
    fn set_feed_rate(&mut self, value: f64) {
        self.feed_rate = value;
    }
    fn plunge_rate(&self) -> f64 {
        self.plunge_rate
    }
    fn set_plunge_rate(&mut self, value: f64) {
        self.plunge_rate = value;
    }
    fn stepover(&self) -> Option<f64> {
        Some(self.stepover)
    }
    fn set_stepover(&mut self, value: f64) {
        self.stepover = value;
    }
    fn depth_semantics(&self) -> DepthSemantics {
        DepthSemantics::None
    }
    fn spindle_rpm(&self) -> Option<u32> {
        self.spindle_rpm
    }
    fn set_spindle_rpm(&mut self, rpm: Option<u32>) {
        self.spindle_rpm = rpm;
    }
}

impl OperationParams for ProjectCurveConfig {
    fn feed_rate(&self) -> f64 {
        self.feed_rate
    }
    fn set_feed_rate(&mut self, value: f64) {
        self.feed_rate = value;
    }
    fn plunge_rate(&self) -> f64 {
        self.plunge_rate
    }
    fn set_plunge_rate(&mut self, value: f64) {
        self.plunge_rate = value;
    }
    fn depth_semantics(&self) -> DepthSemantics {
        DepthSemantics::Explicit(self.depth)
    }
    fn spindle_rpm(&self) -> Option<u32> {
        self.spindle_rpm
    }
    fn set_spindle_rpm(&mut self, rpm: Option<u32>) {
        self.spindle_rpm = rpm;
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    /// ClearingStrategy must serialize and deserialize all three variants.
    /// Regression guard for the GUI/MCP exposure of AgentSearch —
    /// adding a variant to the core ClearingStrategy3d enum is not
    /// enough; this config-layer enum is the one serde sees from TOML
    /// and the one the MCP's set_toolpath_param dispatches through.
    #[test]
    fn clearing_strategy_serde_round_trip() {
        for variant in [
            ClearingStrategy::ContourParallel,
            ClearingStrategy::Adaptive,
            ClearingStrategy::AgentSearch,
        ] {
            let json = serde_json::to_string(&variant).unwrap();
            let round_trip: ClearingStrategy = serde_json::from_str(&json).unwrap();
            assert_eq!(round_trip, variant);
        }
        // Also verify the snake_case wire format — agent_search, not AgentSearch.
        let json = serde_json::to_string(&ClearingStrategy::AgentSearch).unwrap();
        assert_eq!(json, "\"agent_search\"");
        let from_wire: ClearingStrategy = serde_json::from_str("\"agent_search\"").unwrap();
        assert_eq!(from_wire, ClearingStrategy::AgentSearch);
    }

    /// The A/M6 deserialization-compatibility matrix, all three cells
    /// pinned in one place.
    ///
    /// | project file says | must mean |
    /// |---|---|
    /// | `"self_probe"` | `ClaimsReference::SelfProbe` — pinned, unchanged |
    /// | `"machined_stock"` | `ClaimsReference::MachinedStock` — pinned, unchanged |
    /// | *field absent* | `ClaimsReference::Auto` — the A/M6 default flip |
    ///
    /// Binding, because the default is the ONLY thing A/M6 was allowed to
    /// move: an operator who wrote a value into a project file keeps it to
    /// the letter, and the wire names are the pre-A/M6 ones so no migration
    /// is needed. `CreaseReference` — now the RESOLVED type rather than the
    /// config type — keeps its own two-variant wire format because it is
    /// still what `ClaimsConfig` carries.
    #[test]
    fn unified_finish_claims_reference_serde_round_trip_and_backcompat() {
        // Explicit values: wire name in, same meaning out, for all three.
        for (wire, expected) in [
            ("\"auto\"", ClaimsReference::Auto),
            ("\"self_probe\"", ClaimsReference::SelfProbe),
            ("\"machined_stock\"", ClaimsReference::MachinedStock),
        ] {
            let parsed: ClaimsReference = serde_json::from_str(wire).unwrap();
            assert_eq!(parsed, expected, "wire {wire} must parse to {expected:?}");
            assert_eq!(
                serde_json::to_string(&expected).unwrap(),
                wire,
                "{expected:?} must serialize back to {wire}"
            );
        }

        // The resolved-side enum keeps its own two-valued wire format —
        // `ClaimsConfig::crease_reference` still carries it.
        for variant in [CreaseReference::SelfProbe, CreaseReference::MachinedStock] {
            let json = serde_json::to_string(&variant).unwrap();
            let round_trip: CreaseReference = serde_json::from_str(&json).unwrap();
            assert_eq!(round_trip, variant);
        }
        assert_eq!(
            serde_json::to_string(&CreaseReference::MachinedStock).unwrap(),
            "\"machined_stock\""
        );

        // Cell 1: a project file that PINS `self_probe` keeps it. This is the
        // A/M6 footgun value, and it stays honoured — the fix reports it, it
        // does not overrule it.
        let pinned_self_probe = r#"{
            "steep_threshold_deg": 45.0,
            "waterline_threshold_deg": 75.0,
            "overlap_mm": 2.0,
            "scallop_height": 0.1,
            "tolerance": 0.05,
            "raster_stepover": 1.0,
            "z_step": 1.0,
            "sampling": 0.5,
            "stock_to_leave": 0.0,
            "feed_rate": 1000.0,
            "plunge_rate": 500.0,
            "claims_reference": "self_probe"
        }"#;
        let cfg: UnifiedFinishConfig = serde_json::from_str(pinned_self_probe).unwrap();
        assert_eq!(cfg.claims_reference, ClaimsReference::SelfProbe);

        // Cell 2: a project file that PINS `machined_stock` keeps it.
        let pinned_machined = pinned_self_probe.replace("self_probe", "machined_stock");
        let cfg: UnifiedFinishConfig = serde_json::from_str(&pinned_machined).unwrap();
        assert_eq!(cfg.claims_reference, ClaimsReference::MachinedStock);

        // Cell 3: legacy payload predating `claims_reference` and its S1/S2
        // siblings entirely — must still deserialize, and the ABSENT field is
        // the one case A/M6 moved: `Auto`, not `SelfProbe`.
        let legacy = r#"{
            "steep_threshold_deg": 45.0,
            "waterline_threshold_deg": 75.0,
            "overlap_mm": 2.0,
            "scallop_height": 0.1,
            "tolerance": 0.05,
            "raster_stepover": 1.0,
            "z_step": 1.0,
            "sampling": 0.5,
            "stock_to_leave": 0.0,
            "feed_rate": 1000.0,
            "plunge_rate": 500.0
        }"#;
        let cfg: UnifiedFinishConfig = serde_json::from_str(legacy).unwrap();
        assert_eq!(cfg.claims_reference, ClaimsReference::Auto);
        // …and the flip is INERT on this payload, because claims are off. A
        // legacy project only sees the new default act if it also turned
        // `pencil_claims` on.
        assert!(!cfg.pencil_claims);
        // S4 (`territory_clip`) postdates this legacy payload too — must
        // default off, same backcompat contract as its S1/S2 siblings.
        assert!(!cfg.territory_clip);
    }

    /// A/M6: the resolution table, exhaustively. Six inputs, six outcomes,
    /// and the three properties every consumer reads off them.
    #[test]
    fn claims_reference_resolution_is_total_and_names_its_provenance() {
        use crate::unified_finish::ClaimsReferenceResolution as R;

        let cases = [
            (ClaimsReference::Auto, true, R::DerivedMachinedStock),
            (ClaimsReference::Auto, false, R::DerivedSelfProbeNoPrior),
            (
                ClaimsReference::SelfProbe,
                true,
                R::ExplicitSelfProbeOverridingPrior,
            ),
            (
                ClaimsReference::SelfProbe,
                false,
                R::ExplicitSelfProbeNoPrior,
            ),
            (
                ClaimsReference::MachinedStock,
                true,
                R::ExplicitMachinedStock,
            ),
            (
                ClaimsReference::MachinedStock,
                false,
                R::ExplicitMachinedStockWithoutPrior,
            ),
        ];
        for (setting, prior, expected) in cases {
            let got = R::resolve(setting, prior);
            assert_eq!(got, expected, "resolve({setting:?}, {prior})");
            // Provenance must round-trip the inputs it was built from.
            assert_eq!(got.setting(), setting);
            assert_eq!(got.prior_stock_in_scope(), prior);
            assert_eq!(got.is_derived(), setting == ClaimsReference::Auto);
        }

        // Exactly the two outcomes that must be loud: the footgun, and a
        // `machined_stock` dial that could not be honoured.
        assert!(R::ExplicitSelfProbeOverridingPrior.needs_attention());
        assert!(R::ExplicitMachinedStockWithoutPrior.needs_attention());
        for quiet in [
            R::DerivedMachinedStock,
            R::DerivedSelfProbeNoPrior,
            R::ExplicitSelfProbeNoPrior,
            R::ExplicitMachinedStock,
        ] {
            assert!(!quiet.needs_attention(), "{quiet:?} must not be loud");
        }

        // Only a stock IN SCOPE can produce a machined-stock reference —
        // nothing may degrade silently into claiming one it does not have.
        for r in [
            R::DerivedMachinedStock,
            R::DerivedSelfProbeNoPrior,
            R::ExplicitSelfProbeNoPrior,
            R::ExplicitSelfProbeOverridingPrior,
            R::ExplicitMachinedStock,
            R::ExplicitMachinedStockWithoutPrior,
        ] {
            if r.reference() == CreaseReference::MachinedStock {
                assert!(r.prior_stock_in_scope(), "{r:?} claims stock it lacks");
            }
            assert!(!r.label().is_empty());
            assert!(!r.why().is_empty());
        }
    }

    /// Drill selection fields must round-trip, and legacy TOML/JSON that
    /// predates them must deserialize to the legacy "all centroids" behaviour
    /// (`selected_holes: None`, empty `selected_layers`).
    #[test]
    fn drill_selection_serde_round_trip_and_backcompat() {
        // Legacy payload with neither selection key present.
        let legacy = r#"{
            "depth": 10.0,
            "cycle": "peck",
            "peck_depth": 3.0,
            "dwell_time": 0.5,
            "retract_amount": 0.5,
            "feed_rate": 300.0,
            "retract_z": 2.0
        }"#;
        let cfg: DrillConfig = serde_json::from_str(legacy).unwrap();
        assert_eq!(cfg.selected_holes, None, "legacy => all-centroids (None)");
        assert!(cfg.selected_layers.is_empty());

        // Round-trip with an explicit selection.
        let chosen = DrillConfig {
            selected_holes: Some(vec![[1.0, 2.0], [3.0, 4.0]]),
            selected_layers: vec!["holes".to_owned()],
            ..DrillConfig::default()
        };
        let json = serde_json::to_string(&chosen).unwrap();
        let back: DrillConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.selected_holes, chosen.selected_holes);
        assert_eq!(back.selected_layers, chosen.selected_layers);

        // None must be omitted from the wire form (skip_serializing_if).
        let default_json = serde_json::to_string(&DrillConfig::default()).unwrap();
        assert!(!default_json.contains("selected_holes"));
        assert!(!default_json.contains("selected_layers"));
    }
}
