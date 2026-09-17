use serde::{Deserialize, Serialize};

use super::catalog::{DepthSemantics, OperationParams};
use super::tool_config::ToolId;
use crate::finish::finish_planner::FinishPlannerParams;

// Re-export operation parameter enums from core (single source of truth).
pub use crate::finish::ramp_finish::CutDirection;
pub use crate::finish::scallop::ScallopDirection;
pub use crate::finish::spiral_finish::SpiralDirection;
pub use crate::finish::unified_finish::{ClaimsReference, CreaseReference};
pub use crate::ops::face::FaceDirection;
pub use crate::ops::profile::ProfileSide;
pub use crate::ops::trace_path::TraceCompensation;

/// The three runtime numbers every finishing `…Params` carries and no
/// config holds (FIN-02).
///
/// The feed and plunge rates are the RESOLVED ones — `OperationConfig::
/// feed_rate()` and `plunge_rate()`, which read the tool and the feeds
/// model — not the `feed_rate` field on the config beside them. The retract
/// height comes from the operation's resolved heights. Named fields, so a
/// caller cannot swap two `f64`s without the compiler seeing it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OpMotion {
    /// Resolved cutting feed (mm/min).
    pub feed_rate: f64,
    /// Resolved plunge feed (mm/min).
    pub plunge_rate: f64,
    /// Retract height (mm) links and rapids travel at.
    pub safe_z: f64,
}

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

/// Cap a configured peck depth at the depth of the hole it is drilling.
///
/// G-WANAKA-PECK (2026-08-19). Auto-applied feeds set `peck_depth = 15.0`
/// on a hole whose `depth` was `12.0`. A peck bigger than the hole is not
/// a peck: the cycle takes one oversized bite and the chip-evacuation the
/// operator selected the cycle for never happens — in white oak with a
/// 6 mm end mill that is the difference between a cleared hole and a
/// welded one. Nothing flagged it. The peck-adequacy gate read `Within`,
/// but that gate divides by the ENVELOPE radius (R-12) and is not
/// evidence about anything here.
///
/// **Clamp, not refuse.** A refusal would fire on the product's own
/// defaults: `DrillConfig::default()` ships `peck_depth = 3.0`, so every
/// hole shallower than 3 mm — a perfectly ordinary shallow bore — would
/// stop generating. And unlike a ramped drill hole (G-WANAKA-DRILL-RAMP,
/// which asks for geometry that has no correct form) this request is
/// coherent and has an obviously-correct reading: nothing may be deeper
/// than the hole. So the request is honoured at the largest value that
/// still means what it says.
///
/// The cap is `depth`, not something strictly below it, and that is
/// enough because the cycle is **rooted at the R-plane**, not at the hole
/// top (Fanuc G83 — see [`crate::ops::drill::fed_descents`]). The R-plane sits
/// at `effective_safe_z(retract_z, top_z) >= top_z + SAFE_Z_CLEARANCE_MM`,
/// strictly above the stock, so the first descent of a `peck == depth`
/// cycle stops at `R - depth > top_z - depth = bottom_z` and a second
/// descent always follows. The cycle pecks.
///
/// A non-positive or non-finite `depth` is left alone: it is nonsense the
/// emitter's own guard in `fed_descents` already degrades deliberately,
/// and clamping to it would turn one degenerate value into another.
pub(crate) fn clamp_peck_depth(peck_depth: f64, depth: f64) -> f64 {
    if depth.is_finite() && depth > 0.0 {
        peck_depth.min(depth)
    } else {
        peck_depth
    }
}

impl DrillCycleType {
    /// Convert to core `DrillCycle` using parameters from the config struct.
    ///
    /// The peck depth passes through [`clamp_peck_depth`] so the emitted
    /// cycle and the `DrillOp` summary built beside it (§6.E dual-rep)
    /// describe the same cycle — both consumers go through here.
    pub fn to_core(self, cfg: &DrillConfig) -> crate::ops::drill::DrillCycle {
        use crate::ops::drill::DrillCycle;
        let peck_depth = clamp_peck_depth(cfg.peck_depth, cfg.depth);
        match self {
            Self::Simple => DrillCycle::Simple,
            Self::Dwell => DrillCycle::Dwell(cfg.dwell_time),
            Self::Peck => DrillCycle::Peck(peck_depth),
            Self::ChipBreak => DrillCycle::ChipBreak(peck_depth, cfg.retract_amount),
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
    /// `None` (the default) drills every drill target the model exposes,
    /// and refuses when it exposes none — a polygon outline is not a hole
    /// source (G-DRILLCENTROID, 2026-09-10; before that `None` drilled the
    /// centroid of every closed polygon). `Some(_)` means the user has
    /// taken control of the selection; an empty list then means "no
    /// targets selected" rather than "all targets".
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
    /// Convert to the core [`crate::ops::drill::DrillCycle`]. Pin drilling
    /// fixes dwell to 0.5 s and chip-break retract to 0.5 mm — the
    /// config carries no knobs for them (alignment pins are a fixture
    /// cycle, not a tunable drill op). T11 dedup: this conversion was
    /// previously inlined at both consumer sites in `execute.rs`
    /// (toolpath arm + `build_drill_op_for_config`).
    ///
    /// `depth` is the hole depth this cycle will drill — for pin holes
    /// that is `stock height + spoilboard_penetration`, which this config
    /// cannot compute on its own (it has no stock bbox), so the two
    /// `execute.rs` consumers hand it in. It exists only to cap
    /// `peck_depth`; see [`clamp_peck_depth`] for why the cap is a clamp
    /// and not a refusal (G-WANAKA-PECK).
    pub fn drill_cycle(&self, depth: f64) -> crate::ops::drill::DrillCycle {
        use crate::ops::drill::DrillCycle;
        let peck_depth = clamp_peck_depth(self.peck_depth, depth);
        match self.cycle {
            DrillCycleType::Simple => DrillCycle::Simple,
            DrillCycleType::Dwell => DrillCycle::Dwell(0.5),
            DrillCycleType::Peck => DrillCycle::Peck(peck_depth),
            DrillCycleType::ChipBreak => DrillCycle::ChipBreak(peck_depth, 0.5),
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
    /// G-LINKSTAGE: cap on the XY gap a raster-row-to-row **surface link**
    /// may span, in mm. **Ships at `0.0`, which is OFF** — the input is
    /// returned untouched before the relinker is entered, so this family's
    /// emission is byte-identical to the pre-stage build until an operator
    /// asks for a link (`chain_distance_mm`'s pattern).
    ///
    /// The raster's own linker is the serpentine hookup in
    /// [`crate::toolpath`], whose cap is one grid diagonal
    /// (`hypot(x_step, y_step) * 1.05`). Two runs of one row split by an
    /// excluded cell are two steps apart, so nothing inside a row can link:
    /// on the wanaka island raster that is 953 row fragments and 954 retracts
    /// (`planning/linking_2026-09-09/SPEC.md` §1). With this above zero the
    /// shared stage re-decides each junction on evidence — drop-cutter
    /// sampled so it cannot gouge, lifted clear of anything standing in the
    /// input stock, refused if it leaves the machining regions, and (with
    /// kinematics) kept only when it beats the retract on time.
    #[serde(default)]
    pub hookup_mm: f64,
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
            // OFF. See the field doc: byte-identity for a family that has
            // not opted in is the gate this dial exists to hold.
            hookup_mm: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Adaptive3dConfig {
    pub stepover: f64,
    pub depth_per_pass: f64,
    /// Vertical leave allowance above the surface heightmap. The only
    /// leave-stock dial the `adaptive3d` planner applies, and since L2
    /// the only one it carries: the drop-cutter / dexel heightmap engine
    /// offsets in Z alone. See
    /// `compute::execute::adaptive3d_effective_stock_to_leave`.
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
    /// G-LINKSTAGE: cap on the XY gap a level-to-level **surface link** may
    /// span, in mm. **Ships at `0.0`, which is OFF** — same contract and same
    /// reason as [`DropCutterConfig::hookup_mm`].
    ///
    /// Waterline levels are closed loops, so this family has the most to gain
    /// from the stage's loop rotation — but it does not declare a
    /// [`crate::finish::surface_link::FragmentKind`] yet (the adapter cannot tell a
    /// whole level from a boundary-split arc), so today the stage links and
    /// reorders it without rotating. Declaring the kinds in the generator is
    /// the follow-up.
    #[serde(default)]
    pub hookup_mm: f64,
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
            // OFF. See the field doc.
            hookup_mm: 0.0,
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
    #[serde(default = "crate::finish::pencil::reach_gap_threshold")]
    pub min_valley_depth: f64,
    /// Bisector positioning strength (0 = off, 1 = geometrically correct). Shifts
    /// the trace out along the wall bisector in asymmetric corners so the ball
    /// nestles instead of riding up the steep wall. `#[serde(default)]` so older
    /// project files load.
    #[serde(default = "crate::finish::pencil::bisector_strength_default")]
    pub bisector_strength: f64,
    /// Diameter (mm) of the bigger reference (finishing) tool this pencil pass
    /// cleans up after. The gate keeps a seam by how much deeper the pencil tool
    /// reaches than this reference could, so it traces the valleys a bigger bit
    /// missed and skips reachable walls + sub-pencil texture. `#[serde(default)]`
    /// so older project files load.
    #[serde(default = "crate::finish::pencil::reference_tool_diameter_default")]
    pub reference_tool_diameter: f64,
    /// Valley-detection algorithm: `dihedral` (mesh crease detection, the
    /// default), `curvature` (curvature crest lines, best for noisy organic
    /// relief) or `rest_depth` (the dual-tool rest field).
    ///
    /// FIN-09 typed this field. It was a `String` parsed by a fallback map,
    /// so a project file that said `rest-depth` ran the dihedral detector and
    /// reported nothing. An unknown token now refuses the load and names the
    /// key. `#[serde(default)]` so a project file that omits it loads.
    #[serde(default)]
    pub detector: crate::finish::pencil::PencilDetector,
    /// Minimum concave curvature |κ₂| (1/mm) a valley must reach for the
    /// `curvature` detector to trace it — the valley significance dial. Low →
    /// every concave seam; high → only deep sharp valleys. `#[serde(default)]`.
    #[serde(default = "crate::finish::pencil::valley_saliency_default")]
    pub valley_saliency: f64,
    /// Curvature-tensor smoothing iterations for the `curvature` detector (the
    /// literature denoise — smooths the curvature field, not the geometry).
    /// `#[serde(default)]` so older project files load.
    #[serde(default = "crate::finish::pencil::curvature_smoothing_default")]
    pub curvature_smoothing: usize,
    /// XY grid cell size (mm) for the `rest_depth` detector's rest field. Smaller
    /// = finer regions, more drops. `#[serde(default)]` so older files load.
    #[serde(default = "crate::finish::pencil::rest_cell_default")]
    pub rest_cell_mm: f64,
    /// R1: optional library tool id whose *real* cutter geometry defines the
    /// rest reference (all three detectors). `None` = legacy nominal-diameter
    /// behaviour via `reference_tool_diameter`. Mirrors `RestConfig.prev_tool_id`
    /// — `#[serde(default, skip_serializing_if)]` so older project files load
    /// and files that never set it stay byte-identical on save.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference_tool_id: Option<ToolId>,
    /// G-LINKSTAGE: the reach (mm) of the CLEARANCE-HOP link tier, which is
    /// separate from [`Self::hookup_distance`]. That dial caps the at-depth
    /// tier.
    ///
    /// The two tiers do not give the same result. A link that arrives at
    /// cutting depth removes the next fragment's entry outright. A link that
    /// LIFTS clear of standing material lands from above, so the fragment
    /// keeps its descent, and the lift itself costs travel time.
    ///
    /// * `None` — one cap for both tiers, `hookup_distance`. This is the
    ///   shipped emission, byte for byte. Do NOT read `None` as "the hop
    ///   tier is off": the `gap > hookup_distance` test returns first, so
    ///   the hop test cannot run at `None` (G-PENCILHOP, retracted in
    ///   `fe0227a2`).
    /// * `Some(0.0)` — the OFF switch for the hop tier alone. The pass
    ///   refuses every lifted candidate and counts it in
    ///   [`crate::finish::pencil::PencilLinkReport::hop_too_far`]. The at-depth tier
    ///   does not change, so this is the control arm that separates the two
    ///   tiers on one fixture.
    /// * `Some(d)` — hops reach `d` mm; at-depth links keep
    ///   `hookup_distance`.
    ///
    /// `#[serde(default, skip_serializing_if)]` so older project files load,
    /// and a file that never sets it stays byte-identical on save. The GUI
    /// panel has no widget for it yet — the dial is reachable from a project
    /// file and from MCP `set_toolpath_param`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link_hop_distance_mm: Option<f64>,
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
            min_valley_depth: crate::finish::pencil::reach_gap_threshold(),
            bisector_strength: crate::finish::pencil::bisector_strength_default(),
            reference_tool_diameter: crate::finish::pencil::reference_tool_diameter_default(),
            detector: crate::finish::pencil::PencilDetector::default(),
            valley_saliency: crate::finish::pencil::valley_saliency_default(),
            curvature_smoothing: crate::finish::pencil::curvature_smoothing_default(),
            rest_cell_mm: crate::finish::pencil::rest_cell_default(),
            reference_tool_id: None,
            // One cap for both tiers — the shipped emission. See the field.
            link_hop_distance_mm: None,
            spindle_rpm: None,
        }
    }
}

impl PencilConfig {
    /// Build the [`crate::finish::pencil::PencilParams`] this operation
    /// traces with — the ONE translation site (FIN-02).
    ///
    /// The two borrowed arguments are what the config cannot hold: the R1
    /// reference cutter, resolved from the library tool the op names, and
    /// the machine envelope the link stage costs against.
    #[must_use]
    pub fn params(
        &self,
        motion: OpMotion,
        reference_cutter: Option<crate::tool::ToolDefinition>,
        link_kinematics: Option<crate::machine::kinematics::LinkKinematics>,
    ) -> crate::finish::pencil::PencilParams {
        crate::finish::pencil::PencilParams {
            bitangency_angle: self.bitangency_angle,
            min_cut_length: self.min_cut_length,
            hookup_distance: self.hookup_distance,
            num_offset_passes: self.num_offset_passes,
            offset_stepover: self.offset_stepover,
            sampling: self.sampling,
            feed_rate: motion.feed_rate,
            plunge_rate: motion.plunge_rate,
            safe_z: motion.safe_z,
            stock_to_leave: self.stock_to_leave,
            min_valley_depth: self.min_valley_depth,
            bisector_strength: self.bisector_strength,
            reference_tool_diameter: self.reference_tool_diameter,
            detector: self.detector,
            valley_saliency: self.valley_saliency,
            curvature_smoothing: self.curvature_smoothing,
            rest_cell_mm: self.rest_cell_mm,
            // R1: real reference tool geometry when the op names one; else
            // `None` → the pencil detectors fall back to the nominal
            // `reference_tool_diameter`.
            reference_cutter,
            // P1 W4a: cost the surface-link-vs-retract emit decision against
            // the real machine envelope when one is in scope.
            link_kinematics,
            // G-LINKSTAGE: the clearance-hop tier's own cap. `None` — the
            // default — keeps the at-depth tier's cap, which is the shipped
            // emission byte for byte. `Some(0.0)` refuses every hop and
            // leaves the at-depth tier alone, which is the control arm the
            // measured pair needs (`planning/pencil_linking_2026-09-04.md`,
            // `planning/linking_2026-09-09/SPEC.md` §8).
            link_hop_distance_mm: self.link_hop_distance_mm,
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
    /// disables it. See [`crate::finish::scallop::ScallopParams::intra_pass_hookup_mm`]
    /// for what the relink does and refuses to do.
    #[serde(default = "default_scallop_intra_pass_hookup_mm")]
    pub intra_pass_hookup_mm: f64,
    /// M8 (2026-09-03) — take the rings from the iso-scallop FIELD instead
    /// of the offset cascade: per-point spacing (no per-ring min-reduction
    /// crawl), the SPEC-CORRECT cosine slope law, and a termination
    /// invariant instead of the `max_rings` cap (retires the
    /// G-SCALLOPBASIN truncation class). Measured on the wanaka board
    /// against the production unified op: 0.875× its time at 2.5 pp better
    /// envelope coverage (`planning/metrology_2026-09-02/FINDINGS.md`
    /// §M7–M8). Default OFF: an absent key loads the legacy cascade
    /// byte-identically. Speed is traded at `scallop_height` and the tool
    /// — never across the surface (operator ruling, 2026-09-03).
    #[serde(default)]
    pub iso_field: bool,
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
/// position-preservation unit tests in `crate::finish::surface_link`.
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
            iso_field: false,
        }
    }
}

impl ScallopConfig {
    /// Build the [`crate::finish::scallop::ScallopParams`] this operation
    /// rings with — the ONE translation site (FIN-02).
    #[must_use]
    pub fn params(
        &self,
        motion: OpMotion,
        link_kinematics: Option<crate::machine::kinematics::LinkKinematics>,
    ) -> crate::finish::scallop::ScallopParams {
        crate::finish::scallop::ScallopParams {
            scallop_height: self.scallop_height,
            tolerance: self.tolerance,
            direction: self.direction,
            continuous: self.continuous,
            slope_from: self.slope_from,
            slope_to: self.slope_to,
            feed_rate: motion.feed_rate,
            plunge_rate: motion.plunge_rate,
            safe_z: motion.safe_z,
            stock_to_leave: self.stock_to_leave,
            // A/M7: the standalone all-over pass is where the unconditional
            // ring retract actually costs — nothing above it relinks.
            intra_pass_hookup_mm: self.intra_pass_hookup_mm,
            link_kinematics,
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
    /// Material left on the finished surface (mm), applied as a vertical
    /// `+Z` offset on the cut.
    ///
    /// Honoured by **all three bands** — shallow raster, mid-steep scallop
    /// and very-steep waterline — since F3 / D-16.2 (2026-08-06). Before
    /// that only the scallop band applied it and the other two silently
    /// dropped it; the field carried no doc comment at all, and neither the
    /// GUI dial nor the MCP `ParamDef` said so.
    ///
    /// It is a VERTICAL offset, not a surface-normal one: on a wall at angle
    /// θ from horizontal what remains measured normal to the surface is
    /// `stock_to_leave · cos θ`. That is the repo-wide convention for every
    /// finish operation, so bands and their links stay flush with each other.
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
    /// [`crate::compute::toolpath_stats::ClaimsReferenceFinding`].
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
    /// C2 (`planning/thin_organic_2026-08-27/PROGRAMME.md` Track C, measured
    /// in `FINDINGS.md` §0g–§0k): split every SHALLOW region into monotone
    /// **cells** on the region's own raster lattice, and rotate that lattice
    /// to the region's PCA-minor axis when the region clears
    /// [`crate::geometry::monotone_cells::ELONGATION_GATE`] (3.0). Each cell is
    /// rastered on that ONE shared lattice, so the relinker sees cell-shaped
    /// fragments instead of one dendritic region's worth of them.
    ///
    /// Default `true` since 2026-09-01 (C4 operator surface review passed —
    /// `FINDINGS.md` §7, "C4 ruling"). Pin `false` on an operation to get
    /// the pre-C2 op byte-for-byte.
    ///
    /// What it is NOT, each having been measured and refuted: no per-cell
    /// sweep direction (§0j, a 0.917× cost), no cell TSP (§0j, byte-identical
    /// to emission order because the relinker already reorders), no contour
    /// or per-cell pattern choice (§0k, 0.686×).
    ///
    /// Measured value on the operator's wanaka relief under the realistic
    /// machined-stock link ceiling (§0i): **1.155×** across the top-three
    /// shallow regions, **1.215×** on the one region that clears the
    /// elongation gate. Those are RIG ceilings to approach, not promises.
    /// The C4 rendered-surface review bound adoption because cell seams
    /// change the cusp pattern; the operator passed it 2026-09-01.
    #[serde(default = "default_unified_finish_monotone_cell_decomposition")]
    pub monotone_cell_decomposition: bool,
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
    /// [`crate::finish::unified_finish::ClaimsConfig::crease_hookup_mm`] — that
    /// emitter links with NO territory boundary, so this is the only lever
    /// on crease links leaving their rest island. Meaningful only
    /// alongside `pencil_claims = true`. Default 5.0 (the historical
    /// `PencilParams::default()` value); `0.0` disables crease linking.
    #[serde(default = "default_unified_finish_crease_hookup_mm")]
    pub crease_hookup_mm: f64,
    /// F2 (multi-tool island finishing): island **absorption floor** (mm²)
    /// for [`crate::finish::finish_planner::decompose`]'s min-area step — a
    /// connected band island smaller than this is absorbed into its
    /// surrounding band instead of becoming its own region.
    ///
    /// `None` (**the default, and byte-identical to every pre-F2 project**)
    /// keeps the tool-derived value
    /// [`crate::finish::finish_planner::FinishPlannerParams::for_tool`] computes:
    /// `(2 · cusp_radius)² · 4`, i.e. roughly four tool-diameters². `Some(v)`
    /// overrides that ONE dial and leaves every other planner dial derived.
    ///
    /// Read `for_tool`'s doc before setting this: these are FEATURE SCALES
    /// sized off the tool's CUSP (tip) radius, not its envelope. On a Ø1-tip
    /// / Ø6-shank taper the derived value is 4 mm², not 144 mm².
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_region_area_mm2: Option<f64>,
    /// F2: island **merge radius** (mm) — the morphological close radius
    /// applied to each band mask before regions are extracted. Larger values
    /// merge neighbouring islands into one region; smaller values keep them
    /// apart.
    ///
    /// `None` (**the default, byte-identical to every pre-F2 project**) keeps
    /// the tool-derived `cusp_radius · 0.5` from
    /// [`crate::finish::finish_planner::FinishPlannerParams::for_tool`]. `Some(v)`
    /// overrides that one dial only.
    ///
    /// Cusp radius, not envelope radius — same footgun as
    /// [`Self::min_region_area_mm2`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub close_radius_mm: Option<f64>,
    /// F2: band **hysteresis width** (degrees) — a cell leaves a band only
    /// once its slope falls below `enter − hysteresis_deg`, which is what
    /// stops the raw slope masks from storming into O(100) speckled islands.
    ///
    /// `None` (**the default, byte-identical to every pre-F2 project**) keeps
    /// [`crate::finish::finish_planner::FinishPlannerParams::for_tool`]'s fixed
    /// `10.0` — the only one of the three that is a constant rather than a
    /// tool-derived scale. `Some(v)` overrides it.
    ///
    /// It is load-bearing: `for_tool`'s own doc records that `0` makes the
    /// masks storm to O(100) islands. Lower it deliberately or not at all.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hysteresis_deg: Option<f64>,
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
        skip_serializing_if = "crate::finish::classify_probe::ClassificationSampler::is_production"
    )]
    pub classification_sampler: crate::finish::classify_probe::ClassificationSampler,
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
            monotone_cell_decomposition: default_unified_finish_monotone_cell_decomposition(),
            intra_region_hookup_mm: default_unified_finish_intra_region_hookup_mm(),
            crease_hookup_mm: default_unified_finish_crease_hookup_mm(),
            // F2: absent = derive from the tool, which is what every project
            // written before F2 asks for by construction (the keys are
            // skipped while `None`, so a round-trip adds no noise either).
            min_region_area_mm2: None,
            close_radius_mm: None,
            hysteresis_deg: None,
            classification_sampler:
                crate::finish::classify_probe::ClassificationSampler::PRODUCTION,
        }
    }
}

impl UnifiedFinishConfig {
    /// Build the [`crate::finish::finish_planner::FinishPlannerParams`] this
    /// operation decomposes with — the ONE construction site, shared by the
    /// generator (`compute::execute::generate_unified_finish`) and by the F2
    /// sentries.
    ///
    /// Two layers, in order:
    ///
    /// 1. [`crate::finish::finish_planner::FinishPlannerParams::for_tool`] derives
    ///    every dial from `cusp_radius_mm`. **`cusp_radius_mm` must be the
    ///    tool's cusp-forming (TIP) radius** — `MillingCutter::cusp_radius`,
    ///    never `radius()`. Every dial `for_tool` derives is a feature scale,
    ///    and on a tapered ball `radius()` reports the SHANK: a Ø1 tip on a
    ///    6 mm shank made `min_region_area_mm2` 144 mm² instead of 4 and
    ///    `close_radius_mm` 1.5 mm instead of 0.25, which closed and absorbed
    ///    every steep ribbon (design doc §14q).
    /// 2. The op's own dials are written over that. The three band
    ///    thresholds (`steep_threshold_deg`, `waterline_threshold_deg`,
    ///    `overlap_mm`) always apply; the three F2 island-filter dials
    ///    ([`Self::min_region_area_mm2`], [`Self::close_radius_mm`],
    ///    [`Self::hysteresis_deg`]) apply only when `Some`, so `None` leaves
    ///    the derivation of that single field untouched.
    ///
    /// An all-`None` config therefore reproduces the pre-F2 planner params
    /// exactly — byte-identical output, which is what
    /// `unified_finish_planner_dials_f2.rs` pins.
    #[must_use]
    pub fn planner_params(&self, cusp_radius_mm: f64) -> FinishPlannerParams {
        let mut planner = FinishPlannerParams::for_tool(cusp_radius_mm);
        planner.steep_threshold_deg = self.steep_threshold_deg;
        planner.waterline_threshold_deg = self.waterline_threshold_deg;
        planner.overlap_mm = self.overlap_mm;
        if let Some(v) = self.min_region_area_mm2 {
            planner.min_region_area_mm2 = v;
        }
        if let Some(v) = self.close_radius_mm {
            planner.close_radius_mm = v;
        }
        if let Some(v) = self.hysteresis_deg {
            planner.hysteresis_deg = v;
        }
        planner
    }

    /// Build the [`crate::finish::unified_finish::UnifiedFinishParams`] the
    /// three band arms cut with — the ONE translation site (FIN-02), and the
    /// sibling of [`Self::planner_params`].
    ///
    /// The config carries 23 fields and this takes 12. The rest are not
    /// forgotten: [`Self::planner_params`] takes the decomposition dials,
    /// and the claims dials are read by the operation adapter, which is the
    /// only layer that can see the stock in scope.
    #[must_use]
    pub fn params(&self, motion: OpMotion) -> crate::finish::unified_finish::UnifiedFinishParams {
        crate::finish::unified_finish::UnifiedFinishParams {
            scallop_height: self.scallop_height,
            tolerance: self.tolerance,
            raster_stepover: self.raster_stepover,
            z_step: self.z_step,
            sampling: self.sampling,
            stock_to_leave: self.stock_to_leave,
            feed_rate: motion.feed_rate,
            plunge_rate: motion.plunge_rate,
            safe_z: motion.safe_z,
            intra_region_hookup_mm: self.intra_region_hookup_mm,
            classification_sampler: self.classification_sampler,
            monotone_cell_decomposition: self.monotone_cell_decomposition,
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
/// [`crate::compute::toolpath_stats::ClaimsReferenceFinding`], so a behaviour change
/// arrives with its own explanation rather than silently.
fn default_unified_finish_claims_reference() -> ClaimsReference {
    ClaimsReference::Auto
}

fn default_unified_finish_territory_clip() -> bool {
    false
}

/// **ON since 2026-09-01 — an operator's call, on the C4 surface review.**
///
/// C2 shipped INERT (X5) until the C4 rendered-surface review passed
/// (`planning/thin_organic_2026-08-27/FINDINGS.md` §7, "C4 ruling"). An
/// absent key now loads `true`: a legacy project file gets the cell
/// decomposition. A project that pins `false` keeps `false` — that is the
/// per-operation opt-out, and the X5 back-compat test pins both directions.
///
/// Kept in lockstep with
/// [`crate::finish::unified_finish::UnifiedFinishParams::default`]'s own value — a
/// core default and a serde default that disagree is a divergence class this
/// repo has already found twice.
fn default_unified_finish_monotone_cell_decomposition() -> bool {
    true
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

impl SteepShallowConfig {
    /// Build the [`crate::finish::steep_shallow::SteepShallowParams`] this
    /// operation bands with — the ONE translation site (FIN-02).
    #[must_use]
    pub fn params(&self, motion: OpMotion) -> crate::finish::steep_shallow::SteepShallowParams {
        crate::finish::steep_shallow::SteepShallowParams {
            threshold_angle: self.threshold_angle,
            overlap_distance: self.overlap_distance,
            wall_clearance: self.wall_clearance,
            steep_first: self.steep_first,
            stepover: self.stepover,
            z_step: self.z_step,
            feed_rate: motion.feed_rate,
            plunge_rate: motion.plunge_rate,
            safe_z: motion.safe_z,
            sampling: self.sampling,
            stock_to_leave: self.stock_to_leave,
            tolerance: self.tolerance,
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

impl RampFinishConfig {
    /// Build the [`crate::finish::ramp_finish::RampFinishParams`] this
    /// operation ramps with — the ONE translation site (FIN-02).
    #[must_use]
    pub fn params(&self, motion: OpMotion) -> crate::finish::ramp_finish::RampFinishParams {
        crate::finish::ramp_finish::RampFinishParams {
            max_stepdown: self.max_stepdown,
            slope_from: self.slope_from,
            slope_to: self.slope_to,
            direction: self.direction,
            order_bottom_up: self.order_bottom_up,
            feed_rate: motion.feed_rate,
            plunge_rate: motion.plunge_rate,
            safe_z: motion.safe_z,
            sampling: self.sampling,
            stock_to_leave: self.stock_to_leave,
            tolerance: self.tolerance,
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

impl SpiralFinishConfig {
    /// Build the [`crate::finish::spiral_finish::SpiralFinishParams`] this
    /// operation spirals with — the ONE translation site (FIN-02).
    #[must_use]
    pub fn params(&self, motion: OpMotion) -> crate::finish::spiral_finish::SpiralFinishParams {
        crate::finish::spiral_finish::SpiralFinishParams {
            stepover: self.stepover,
            direction: self.direction,
            feed_rate: motion.feed_rate,
            plunge_rate: motion.plunge_rate,
            safe_z: motion.safe_z,
            stock_to_leave: self.stock_to_leave,
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

impl RadialFinishConfig {
    /// Build the [`crate::finish::radial_finish::RadialFinishParams`] this
    /// operation rays with — the ONE translation site (FIN-02).
    #[must_use]
    pub fn params(&self, motion: OpMotion) -> crate::finish::radial_finish::RadialFinishParams {
        crate::finish::radial_finish::RadialFinishParams {
            angular_step: self.angular_step,
            point_spacing: self.point_spacing,
            feed_rate: motion.feed_rate,
            plunge_rate: motion.plunge_rate,
            safe_z: motion.safe_z,
            stock_to_leave: self.stock_to_leave,
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

impl HorizontalFinishConfig {
    /// Build the
    /// [`crate::finish::horizontal_finish::HorizontalFinishParams`] this
    /// operation slices with — the ONE translation site (FIN-02).
    #[must_use]
    pub fn params(
        &self,
        motion: OpMotion,
    ) -> crate::finish::horizontal_finish::HorizontalFinishParams {
        crate::finish::horizontal_finish::HorizontalFinishParams {
            angle_threshold: self.angle_threshold,
            stepover: self.stepover,
            feed_rate: motion.feed_rate,
            plunge_rate: motion.plunge_rate,
            safe_z: motion.safe_z,
            stock_to_leave: self.stock_to_leave,
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
    pub surface_model_id: Option<crate::ids::ModelId>,
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
    /// Cap (mm) on the XY gap between two projected chains that may be
    /// joined by a single clearance-height link instead of a full
    /// `retract → rapid → replunge` round trip. `0.0` — the shipped
    /// default — disables chaining entirely and keeps every existing
    /// project byte-identical.
    ///
    /// A rivers/engraving DXF is hundreds of short chains, and each one
    /// costs two safe-Z legs however short the hop between them is, so the
    /// air on this family is COUNT-bound. See
    /// [`crate::finish::surface_link::relink_fragments`] for what the link does and
    /// [`crate::finish::surface_link::LinkCeiling`] for why it travels above the
    /// standing material rather than on the mesh.
    #[serde(default = "default_project_curve_chain_distance_mm")]
    pub chain_distance_mm: f64,
}

/// `0.0` — chaining OFF.
///
/// Unlike scallop's ring relink (which ships on at 3.0 mm), project_curve
/// runs on stock that has usually NOT been cleared down to the mesh, so
/// every link is priced against a material ceiling the operator has not
/// necessarily simulated yet. Shipping it off keeps every saved project and
/// every emitted program byte-identical; an operator who wants the air back
/// opts in per operation.
fn default_project_curve_chain_distance_mm() -> f64 {
    0.0
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
            chain_distance_mm: default_project_curve_chain_distance_mm(),
        }
    }
}

// ---------------------------------------------------------------------------
// OperationParams trait implementations
// ---------------------------------------------------------------------------

/// The `DepthSemantics` value a config reports, from one declared form.
///
/// A separate macro because a `macro_rules!` body cannot take an
/// expression written at the call site that names `self` — hygiene keeps
/// the two `self`s apart. A FORM plus a field name carries the same
/// information and reads as a declaration.
macro_rules! depth_semantics_form {
    ($s:ident, Explicit($f:ident)) => {
        DepthSemantics::Explicit($s.$f)
    };
    ($s:ident, DerivedStockTop($f:ident)) => {
        DepthSemantics::DerivedStockTop($s.$f.abs())
    };
    ($s:ident, None) => {
        DepthSemantics::None
    };
}

/// One `impl OperationParams` per config, declared instead of written.
///
/// CMP-06: the 24 hand-written blocks ran 755 lines, of which 432 were the
/// six universal accessors alone — `feed_rate`, `plunge_rate`,
/// `spindle_rpm` and their setters, identical in all 24. Five blocks were
/// byte-identical to each other and five more formed pairs. The macro
/// emits the universal six with no declaration at all and takes only what
/// differs.
///
/// Clauses, in this order, all but the last two optional:
///
/// - `plunge_rate:` the field the plunge rate reads and writes. The two
///   drilling ops name `feed_rate` here: their motion is purely vertical,
///   so the plunge rate IS the feed rate.
/// - `stepover:` the field behind `stepover` / `set_stepover`. Absent
///   means the config has no stepover and the trait default refuses the
///   write. Pencil declares its alias `offset_stepover` here, and
///   Waterline's `z_step` and RampFinish's `max_stepdown` are the same
///   shape on `depth_per_pass` — which is what makes CMP-08's alias list
///   greppable in one place.
/// - `depth_per_pass:` the field behind `depth_per_pass` /
///   `set_depth_per_pass`.
/// - `total_depth:` the field behind `total_depth`.
/// - `scallop_height:` an `f64` field; `scallop_height_opt:` an
///   `Option<f64>` field. The two differ only in where the `Option` sits.
/// - `extra { ... }` — verbatim trait methods, for a config whose
///   accessor is not a plain field read. One config uses it.
/// - `depth_semantics:` one of the three forms above. Required, last, and
///   with no trailing comma.
macro_rules! impl_operation_params {
    (
        $config:ty {
            plunge_rate: $plunge:ident,
            $(stepover: $so:ident,)?
            $(depth_per_pass: $dpp:ident,)?
            $(total_depth: $td:ident,)?
            $(scallop_height: $sh:ident,)?
            $(scallop_height_opt: $sho:ident,)?
            $(extra { $($extra:tt)* })?
            depth_semantics: $($sem:tt)+
        }
    ) => {
        impl OperationParams for $config {
            fn feed_rate(&self) -> f64 {
                self.feed_rate
            }
            fn set_feed_rate(&mut self, value: f64) {
                self.feed_rate = value;
            }
            fn plunge_rate(&self) -> f64 {
                self.$plunge
            }
            fn set_plunge_rate(&mut self, value: f64) {
                self.$plunge = value;
            }
            $(
                fn stepover(&self) -> Option<f64> {
                    Some(self.$so)
                }
                fn set_stepover(&mut self, value: f64) -> bool {
                    self.$so = value;
                    true
                }
            )?
            $(
                fn depth_per_pass(&self) -> Option<f64> {
                    Some(self.$dpp)
                }
                fn set_depth_per_pass(&mut self, value: f64) -> bool {
                    self.$dpp = value;
                    true
                }
            )?
            $(
                fn total_depth(&self) -> Option<f64> {
                    Some(self.$td)
                }
            )?
            $(
                fn scallop_height(&self) -> Option<f64> {
                    Some(self.$sh)
                }
                fn set_scallop_height(&mut self, value: f64) {
                    self.$sh = value;
                }
            )?
            $(
                fn scallop_height(&self) -> Option<f64> {
                    self.$sho
                }
                fn set_scallop_height(&mut self, value: f64) {
                    self.$sho = Some(value);
                }
            )?
            $($($extra)*)?
            fn depth_semantics(&self) -> DepthSemantics {
                depth_semantics_form!(self, $($sem)+)
            }
            fn spindle_rpm(&self) -> Option<u32> {
                self.spindle_rpm
            }
            fn set_spindle_rpm(&mut self, rpm: Option<u32>) {
                self.spindle_rpm = rpm;
            }
        }
    };
}

impl_operation_params!(FaceConfig {
    plunge_rate: plunge_rate,
    stepover: stepover,
    depth_per_pass: depth_per_pass,
    total_depth: depth,
    depth_semantics: Explicit(depth)
});

impl_operation_params!(PocketConfig {
    plunge_rate: plunge_rate,
    stepover: stepover,
    depth_per_pass: depth_per_pass,
    total_depth: depth,
    depth_semantics: Explicit(depth)
});

impl_operation_params!(ProfileConfig {
    plunge_rate: plunge_rate,
    depth_per_pass: depth_per_pass,
    total_depth: depth,
    depth_semantics: Explicit(depth)
});

impl_operation_params!(AdaptiveConfig {
    plunge_rate: plunge_rate,
    stepover: stepover,
    depth_per_pass: depth_per_pass,
    total_depth: depth,
    depth_semantics: Explicit(depth)
});

impl_operation_params!(VCarveConfig {
    plunge_rate: plunge_rate,
    stepover: stepover,
    depth_semantics: Explicit(max_depth)
});

impl_operation_params!(RestConfig {
    plunge_rate: plunge_rate,
    stepover: stepover,
    depth_per_pass: depth_per_pass,
    total_depth: depth,
    depth_semantics: Explicit(depth)
});

impl_operation_params!(InlayConfig {
    plunge_rate: plunge_rate,
    stepover: stepover,
    depth_semantics: Explicit(pocket_depth)
});

impl_operation_params!(ZigzagConfig {
    plunge_rate: plunge_rate,
    stepover: stepover,
    depth_per_pass: depth_per_pass,
    total_depth: depth,
    depth_semantics: Explicit(depth)
});

impl_operation_params!(TraceConfig {
    plunge_rate: plunge_rate,
    depth_per_pass: depth_per_pass,
    total_depth: depth,
    depth_semantics: Explicit(depth)
});

// Drilling is purely vertical: the plunge rate IS the feed rate.
impl_operation_params!(DrillConfig {
    plunge_rate: feed_rate,
    depth_semantics: Explicit(depth)
});

impl_operation_params!(AlignmentPinDrillConfig {
    plunge_rate: feed_rate,
    depth_semantics: Explicit(spoilboard_penetration)
});

impl_operation_params!(ChamferConfig {
    plunge_rate: plunge_rate,
    depth_semantics: Explicit(chamfer_width)
});

impl_operation_params!(DropCutterConfig {
    plunge_rate: plunge_rate,
    stepover: stepover,
    scallop_height_opt: scallop_height,
    depth_semantics: DerivedStockTop(min_z)
});

impl_operation_params!(Adaptive3dConfig {
    plunge_rate: plunge_rate,
    stepover: stepover,
    depth_per_pass: depth_per_pass,
    depth_semantics: None
});

// `z_step` is the Waterline spelling of the per-pass step.
impl_operation_params!(WaterlineConfig {
    plunge_rate: plunge_rate,
    depth_per_pass: z_step,
    depth_semantics: None
});

impl_operation_params!(PencilConfig {
    plunge_rate: plunge_rate,
    extra {
        /// Pencil's stepover is the OFFSET stepover, and it only exists
        /// when the pass count asks for offsets. A single-pass pencil has
        /// no stepover to report, so the getter is a gate and not a plain
        /// field read — the one config the macro cannot declare.
        fn stepover(&self) -> Option<f64> {
            if self.num_offset_passes > 1 {
                Some(self.offset_stepover)
            } else {
                None
            }
        }
        fn set_stepover(&mut self, value: f64) -> bool {
            self.offset_stepover = value;
            true
        }
    }
    depth_semantics: None
});

impl_operation_params!(ScallopConfig {
    plunge_rate: plunge_rate,
    scallop_height: scallop_height,
    depth_semantics: None
});

impl_operation_params!(UnifiedFinishConfig {
    plunge_rate: plunge_rate,
    scallop_height: scallop_height,
    depth_semantics: None
});

impl_operation_params!(SteepShallowConfig {
    plunge_rate: plunge_rate,
    stepover: stepover,
    depth_semantics: None
});

// `max_stepdown` is the RampFinish spelling of the per-pass step.
impl_operation_params!(RampFinishConfig {
    plunge_rate: plunge_rate,
    depth_per_pass: max_stepdown,
    depth_semantics: None
});

impl_operation_params!(SpiralFinishConfig {
    plunge_rate: plunge_rate,
    stepover: stepover,
    depth_semantics: None
});

impl_operation_params!(RadialFinishConfig {
    plunge_rate: plunge_rate,
    depth_semantics: None
});

impl_operation_params!(HorizontalFinishConfig {
    plunge_rate: plunge_rate,
    stepover: stepover,
    depth_semantics: None
});

impl_operation_params!(ProjectCurveConfig {
    plunge_rate: plunge_rate,
    depth_semantics: Explicit(depth)
});

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
        // C2 (`monotone_cell_decomposition`) postdates all of them. The dial
        // shipped inert under X5 until the C4 operator surface review passed
        // (2026-09-01, `planning/thin_organic_2026-08-27/FINDINGS.md` §7 "C4
        // ruling"); the default is now ON. An absent key loads ON: a legacy
        // file gets the cell decomposition.
        assert!(cfg.monotone_cell_decomposition);
        // …and a project that PINS `false` keeps `false`, so per-operation
        // opt-out stays expressible and the dial is reachable through
        // project IO in both directions.
        let pinned = legacy.replace(
            "\"plunge_rate\": 500.0",
            "\"plunge_rate\": 500.0, \"monotone_cell_decomposition\": false",
        );
        let cfg: UnifiedFinishConfig = serde_json::from_str(&pinned).unwrap();
        assert!(!cfg.monotone_cell_decomposition);
    }

    /// A/M6: the resolution table, exhaustively. Six inputs, six outcomes,
    /// and the three properties every consumer reads off them.
    #[test]
    fn claims_reference_resolution_is_total_and_names_its_provenance() {
        use crate::finish::unified_finish::ClaimsReferenceResolution as R;

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
        assert_eq!(cfg.selected_holes, None, "legacy => all targets (None)");
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
