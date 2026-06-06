use serde::{Deserialize, Serialize};

use crate::feeds::{OperationFamily as FeedsOperationFamily, PassRole};

use super::config::StockSource;
use super::operation_configs::{
    Adaptive3dConfig, AdaptiveConfig, AlignmentPinDrillConfig, ChamferConfig, DrillConfig,
    DropCutterConfig, FaceConfig, HorizontalFinishConfig, InlayConfig, PencilConfig, PocketConfig,
    ProfileConfig, ProjectCurveConfig, RadialFinishConfig, RampFinishConfig, RestConfig,
    ScallopConfig, SpiralFinishConfig, SteepShallowConfig, TraceConfig, VCarveConfig,
    WaterlineConfig, ZigzagConfig,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationFamily {
    TwoPointFiveD,
    ThreeD,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeometryRequirement {
    Stock,
    Polygons,
    Mesh,
    Both,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiOperationFamily {
    Pocket,
    Contour,
    Trace,
    Parallel,
    Scallop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiProcessRole {
    Roughing,
    SemiFinish,
    Finish,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DepthSemantics {
    Explicit(f64),
    DerivedStockTop(f64),
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OperationSpec {
    pub label: &'static str,
    pub description: &'static str,
    pub family: OperationFamily,
    pub geometry: GeometryRequirement,
    pub default_auto_regen: bool,
    pub ui_family: UiOperationFamily,
    pub ui_process_role: UiProcessRole,
    pub feeds_family: FeedsOperationFamily,
    pub feeds_pass_role: PassRole,
}

/// Safety metadata for dressups that can alter topology or move order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OperationTransformCapabilities {
    /// Cutting segments can be globally reordered by XY proximity without
    /// violating depth/material assumptions.
    pub allows_global_rapid_reorder: bool,
    /// The emitted toolpath must maintain its depth-pass ordering.
    pub requires_depth_order: bool,
    /// The operation encodes a continuous trace/finish path where reordering
    /// or stay-down linking can change the intended cut.
    pub continuous_path_required: bool,
}

impl OperationTransformCapabilities {
    pub const fn new(
        allows_global_rapid_reorder: bool,
        requires_depth_order: bool,
        continuous_path_required: bool,
    ) -> Self {
        Self {
            allows_global_rapid_reorder,
            requires_depth_order,
            continuous_path_required,
        }
    }

    pub fn allows_barriered_rapid_reorder(self) -> bool {
        !self.continuous_path_required
    }

    pub fn allows_unbarriered_rapid_reorder(self) -> bool {
        self.allows_global_rapid_reorder
            && !self.requires_depth_order
            && !self.continuous_path_required
    }

    pub fn allows_link_moves(self) -> bool {
        !self.requires_depth_order && !self.continuous_path_required
    }
}

/// Operation type for creating new toolpaths.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationType {
    Face,
    Pocket,
    Profile,
    Adaptive,
    VCarve,
    Rest,
    Inlay,
    Zigzag,
    Trace,
    Drill,
    Chamfer,
    DropCutter,
    Adaptive3d,
    Waterline,
    Pencil,
    Scallop,
    SteepShallow,
    RampFinish,
    SpiralFinish,
    RadialFinish,
    HorizontalFinish,
    ProjectCurve,
    /// Auto-generated drilling operation for stock alignment pin holes.
    AlignmentPinDrill,
}

impl OperationType {
    pub const ALL: &[OperationType] = &[
        OperationType::Face,
        OperationType::Pocket,
        OperationType::Profile,
        OperationType::Adaptive,
        OperationType::VCarve,
        OperationType::Rest,
        OperationType::Inlay,
        OperationType::Zigzag,
        OperationType::Trace,
        OperationType::Drill,
        OperationType::Chamfer,
        OperationType::DropCutter,
        OperationType::Adaptive3d,
        OperationType::Waterline,
        OperationType::Pencil,
        OperationType::Scallop,
        OperationType::SteepShallow,
        OperationType::RampFinish,
        OperationType::SpiralFinish,
        OperationType::RadialFinish,
        OperationType::HorizontalFinish,
        OperationType::ProjectCurve,
        OperationType::AlignmentPinDrill,
    ];

    pub const ALL_2D: &[OperationType] = &[
        OperationType::Face,
        OperationType::Pocket,
        OperationType::Profile,
        OperationType::Adaptive,
        OperationType::VCarve,
        OperationType::Rest,
        OperationType::Inlay,
        OperationType::Zigzag,
        OperationType::Trace,
        OperationType::Drill,
        OperationType::Chamfer,
    ];

    pub const ALL_3D: &[OperationType] = &[
        OperationType::DropCutter,
        OperationType::Adaptive3d,
        OperationType::Waterline,
        OperationType::Pencil,
        OperationType::Scallop,
        OperationType::SteepShallow,
        OperationType::RampFinish,
        OperationType::SpiralFinish,
        OperationType::RadialFinish,
        OperationType::HorizontalFinish,
        OperationType::ProjectCurve,
    ];

    pub const fn spec(self) -> OperationSpec {
        self.registry_entry().spec
    }

    /// Phase 1 registry accessor (architectural refactor 2026-06-06):
    /// THE single place to ask "what is this operation?". Exhaustive
    /// match — adding an `OperationType` variant does not compile until
    /// it has a registry entry, and an entry cannot be written without
    /// explicitly deciding its spec, settable-param schema, and tool
    /// constraints. Behavior (generation) stays in `compute::execute`
    /// until the Phase 5 adapters.
    pub const fn registry_entry(self) -> &'static OpRegistryEntry {
        match self {
            OperationType::Face => &REG_FACE,
            OperationType::Pocket => &REG_POCKET,
            OperationType::Profile => &REG_PROFILE,
            OperationType::Adaptive => &REG_ADAPTIVE,
            OperationType::VCarve => &REG_VCARVE,
            OperationType::Rest => &REG_REST,
            OperationType::Inlay => &REG_INLAY,
            OperationType::Zigzag => &REG_ZIGZAG,
            OperationType::Trace => &REG_TRACE,
            OperationType::Drill => &REG_DRILL,
            OperationType::Chamfer => &REG_CHAMFER,
            OperationType::DropCutter => &REG_DROP_CUTTER,
            OperationType::Adaptive3d => &REG_ADAPTIVE3D,
            OperationType::Waterline => &REG_WATERLINE,
            OperationType::Pencil => &REG_PENCIL,
            OperationType::Scallop => &REG_SCALLOP,
            OperationType::SteepShallow => &REG_STEEP_SHALLOW,
            OperationType::RampFinish => &REG_RAMP_FINISH,
            OperationType::SpiralFinish => &REG_SPIRAL_FINISH,
            OperationType::RadialFinish => &REG_RADIAL_FINISH,
            OperationType::HorizontalFinish => &REG_HORIZONTAL_FINISH,
            OperationType::ProjectCurve => &REG_PROJECT_CURVE,
            OperationType::AlignmentPinDrill => &REG_ALIGNMENT_PIN_DRILL,
        }
    }

    pub fn label(self) -> &'static str {
        self.spec().label
    }

    /// Stable snake_case identifier for serialized / diagnostic use. Unlike
    /// [`Self::label`] (which is for human-facing UI text), this is suitable
    /// for JSON wire formats and for consumers that need to branch on op
    /// kind without parsing the prose label.
    pub fn kind_str(self) -> &'static str {
        match self {
            Self::Face => "face",
            Self::Pocket => "pocket",
            Self::Profile => "profile",
            Self::Adaptive => "adaptive",
            Self::VCarve => "v_carve",
            Self::Rest => "rest",
            Self::Inlay => "inlay",
            Self::Zigzag => "zigzag",
            Self::Trace => "trace",
            Self::Drill => "drill",
            Self::Chamfer => "chamfer",
            Self::DropCutter => "drop_cutter",
            Self::Adaptive3d => "adaptive3d",
            Self::Waterline => "waterline",
            Self::Pencil => "pencil",
            Self::Scallop => "scallop",
            Self::SteepShallow => "steep_shallow",
            Self::RampFinish => "ramp_finish",
            Self::SpiralFinish => "spiral_finish",
            Self::RadialFinish => "radial_finish",
            Self::HorizontalFinish => "horizontal_finish",
            Self::ProjectCurve => "project_curve",
            Self::AlignmentPinDrill => "alignment_pin_drill",
        }
    }

    /// True for op kinds whose kinematics are Z-only (peck-plunge drilling).
    /// Used by the verdict layer to suppress rapid:cut-ratio and engagement
    /// signals that don't apply to drilling — see fix-plan §1 A12.
    pub fn is_drill_kinematics(self) -> bool {
        matches!(self, Self::Drill | Self::AlignmentPinDrill)
    }

    pub fn transform_capabilities(self) -> OperationTransformCapabilities {
        use OperationType::{
            Adaptive, Adaptive3d, AlignmentPinDrill, Chamfer, Drill, DropCutter, Face,
            HorizontalFinish, Inlay, Pencil, Pocket, Profile, ProjectCurve, RadialFinish,
            RampFinish, Rest, Scallop, SpiralFinish, SteepShallow, Trace, VCarve, Waterline,
            Zigzag,
        };

        match self {
            // XY-independent ops: TSP can reorder by proximity safely.
            // Drill/AlignmentPinDrill: each hole is fully completed (peck cycle is intra-hole) before
            // moving to the next, so XY visit order has no material-state effect.
            DropCutter | Drill | AlignmentPinDrill => {
                OperationTransformCapabilities::new(true, false, false)
            }
            // HorizontalFinish: generator sorts regions high-to-low Z for collision-avoidance
            // (horizontal_finish.rs:170 "machine top shelves first to avoid collisions"); TSP
            // would override that safety ordering.
            // Face/Chamfer/Inlay/VCarve/Pencil/RadialFinish: no cross-segment material dependency,
            // segments are retract-separated, so link moves and (eventually) barriered TSP are safe.
            HorizontalFinish | Face | Chamfer | Inlay | VCarve | Pencil | RadialFinish => {
                OperationTransformCapabilities::new(false, false, false)
            }
            // Trace: multi-pass depth stepping; depth order is the constraint, not continuity.
            Pocket | Profile | Adaptive | Rest | Zigzag | Adaptive3d | Waterline | Trace => {
                OperationTransformCapabilities::new(false, true, false)
            }
            // Genuinely continuous traces: helical/spiral/projected paths.
            Scallop | SteepShallow | RampFinish | SpiralFinish | ProjectCurve => {
                OperationTransformCapabilities::new(false, false, true)
            }
        }
    }

    /// Op-kind air-cut percentage band above which a per-toolpath warning
    /// should fire. `None` means "metric not applicable" (drill kinematics
    /// — dexel can't measure Z-only moves; see `planning/P1_AIR_CUT_THRESHOLDS_RCA.md`).
    ///
    /// Calibrated from `WANAKA_ASSESSMENT_2026-05-19.md` expectation bands.
    /// Returning `Some(threshold)` means: a TP whose `air_cut_time_s /
    /// total_runtime_s` exceeds `threshold/100` is a real signal.
    pub fn air_cut_high_threshold_pct(self) -> Option<f64> {
        use OperationType::{
            Adaptive, Adaptive3d, AlignmentPinDrill, Chamfer, Drill, DropCutter, Face,
            HorizontalFinish, Inlay, Pencil, Pocket, Profile, ProjectCurve, RadialFinish,
            RampFinish, Rest, Scallop, SpiralFinish, SteepShallow, Trace, VCarve, Waterline,
            Zigzag,
        };
        match self {
            // Drill kinematics: dexel polygon-to-material init can't see Z-only
            // moves, so air-cut % is unusable. Suppress entirely (Priority 4).
            Drill | AlignmentPinDrill => None,
            // ProjectCurve is inherently sparse: rivers/curves are tiny features
            // in big stock; rapids dominate by construction. Wanaka TPs read
            // 78–92% air-cut at-baseline. Only flag near-total air (~97%+).
            ProjectCurve => Some(97.0),
            // 3D finish ops: close-contact passes expected; >30% indicates poor
            // boundary or excess retraction.
            DropCutter | Scallop | Waterline | Pencil | HorizontalFinish | SteepShallow
            | RampFinish | SpiralFinish | RadialFinish => Some(30.0),
            // 2.5D clearing and 3D rough: boundary overshoot + Z-level transitions
            // make 40% the high-water mark.
            Pocket | Face | Adaptive | Rest | Zigzag | Adaptive3d => Some(40.0),
            // 2D contour-style ops.
            Profile | Chamfer | Inlay | VCarve | Trace => Some(40.0),
        }
    }
}

/// Common parameter accessors for all operation configs.
///
/// Implemented by each config struct to eliminate per-variant match arms.
/// Optional fields (stepover, depth_per_pass) return None by default.
pub trait OperationParams {
    fn feed_rate(&self) -> f64;
    fn set_feed_rate(&mut self, value: f64);

    /// Returns the plunge rate. Drill operations return feed_rate since they're purely vertical.
    fn plunge_rate(&self) -> f64;
    fn set_plunge_rate(&mut self, value: f64);

    fn stepover(&self) -> Option<f64> {
        None
    }
    fn set_stepover(&mut self, _value: f64) {}

    fn depth_per_pass(&self) -> Option<f64> {
        None
    }
    fn set_depth_per_pass(&mut self, _value: f64) {}

    /// Maximum scallop ridge height between adjacent passes (mm). Used
    /// by surface-following finish ops (currently only `ScallopConfig`)
    /// where the planner derives stepover from this + tool ball radius
    /// via the chord-height formula. Distinct from `stepover()` because
    /// units and magnitudes differ — a 0.1 mm scallop on a 6 mm ball
    /// gives ~1.55 mm radial step.
    fn scallop_height(&self) -> Option<f64> {
        None
    }
    fn set_scallop_height(&mut self, _value: f64) {}

    fn depth_semantics(&self) -> DepthSemantics;

    /// Per-toolpath spindle speed override. `None` means "use the project
    /// default" (`PostConfig.spindle_speed`); resolve via
    /// [`effective_spindle_rpm`].
    fn spindle_rpm(&self) -> Option<u32>;
    fn set_spindle_rpm(&mut self, rpm: Option<u32>);
}

/// Operation-specific configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "params", rename_all = "snake_case")]
pub enum OperationConfig {
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
    AlignmentPinDrill(AlignmentPinDrillConfig),
}

/// G16: explicit optimization classification for every `OperationConfig`
/// variant. The match in [`OperationConfig::optimization_surface`] has no
/// wildcard arm, so adding a new variant without classifying it is a
/// compile-time error.
pub enum OptimizationSurface<'op> {
    /// The optimizer can search this op. Carries an `AxisView` into the
    /// op plus the static binding list describing which axes are
    /// available.
    Optimizable(crate::tool_load::optimize::axes::AxisView<'op>),
    /// The op is intentionally not optimizable (e.g., drilling cycles
    /// where chipload is meaningless and DOC is fixed by hole depth).
    /// `reason` surfaces in the orchestrator's outcome.
    NotOptimizable {
        reason: crate::tool_load::RefuseReason,
    },
}

impl OperationConfig {
    /// G16 Step 3: classify this op for optimization. Every variant is
    /// named explicitly — the match has **no wildcard arm**. Adding a
    /// new `OperationConfig` variant without classifying it here is a
    /// compile error, which is the type-safety win this method provides
    /// over the previous `has_doc_knob` allowlist.
    ///
    /// See `planning/STEP3_PREP_OPTIMIZATION_SURFACE.md` for the
    /// design rationale per variant.
    pub fn optimization_surface(&self) -> OptimizationSurface<'_> {
        use crate::tool_load::RefuseReason;
        use crate::tool_load::optimize::axes::{
            AxisView, FEED_RPM_DOC, FEED_RPM_DOC_STEPOVER, FEED_RPM_ONLY, FEED_RPM_SCALLOP,
            FEED_RPM_STEPOVER,
        };

        macro_rules! optimizable {
            ($bindings:expr, $op_type:expr) => {
                OptimizationSurface::Optimizable(AxisView {
                    op: self,
                    bindings: $bindings,
                    op_type: $op_type,
                })
            };
        }

        match self {
            OperationConfig::Face(_) => optimizable!(FEED_RPM_DOC_STEPOVER, OperationType::Face),
            OperationConfig::Pocket(_) => {
                optimizable!(FEED_RPM_DOC_STEPOVER, OperationType::Pocket)
            }
            OperationConfig::Profile(_) => optimizable!(FEED_RPM_DOC, OperationType::Profile),
            OperationConfig::Adaptive(_) => {
                optimizable!(FEED_RPM_DOC_STEPOVER, OperationType::Adaptive)
            }
            OperationConfig::VCarve(_) => optimizable!(FEED_RPM_STEPOVER, OperationType::VCarve),
            OperationConfig::Rest(_) => {
                optimizable!(FEED_RPM_DOC_STEPOVER, OperationType::Rest)
            }
            OperationConfig::Inlay(_) => optimizable!(FEED_RPM_STEPOVER, OperationType::Inlay),
            OperationConfig::Zigzag(_) => {
                optimizable!(FEED_RPM_DOC_STEPOVER, OperationType::Zigzag)
            }
            OperationConfig::Trace(_) => optimizable!(FEED_RPM_DOC, OperationType::Trace),
            OperationConfig::Drill(_) => OptimizationSurface::NotOptimizable {
                reason: RefuseReason::SteadyStateSamplesNotPresent,
            },
            OperationConfig::Chamfer(_) => optimizable!(FEED_RPM_ONLY, OperationType::Chamfer),
            OperationConfig::DropCutter(_) => {
                optimizable!(FEED_RPM_STEPOVER, OperationType::DropCutter)
            }
            OperationConfig::Adaptive3d(_) => {
                optimizable!(FEED_RPM_DOC_STEPOVER, OperationType::Adaptive3d)
            }
            OperationConfig::Waterline(_) => optimizable!(FEED_RPM_DOC, OperationType::Waterline),
            OperationConfig::Pencil(_) => optimizable!(FEED_RPM_STEPOVER, OperationType::Pencil),
            OperationConfig::Scallop(_) => optimizable!(FEED_RPM_SCALLOP, OperationType::Scallop),
            OperationConfig::SteepShallow(_) => {
                optimizable!(FEED_RPM_STEPOVER, OperationType::SteepShallow)
            }
            OperationConfig::RampFinish(_) => {
                optimizable!(FEED_RPM_DOC, OperationType::RampFinish)
            }
            OperationConfig::SpiralFinish(_) => {
                optimizable!(FEED_RPM_STEPOVER, OperationType::SpiralFinish)
            }
            OperationConfig::RadialFinish(_) => {
                optimizable!(FEED_RPM_ONLY, OperationType::RadialFinish)
            }
            OperationConfig::HorizontalFinish(_) => {
                optimizable!(FEED_RPM_STEPOVER, OperationType::HorizontalFinish)
            }
            OperationConfig::ProjectCurve(_) => {
                optimizable!(FEED_RPM_ONLY, OperationType::ProjectCurve)
            }
            OperationConfig::AlignmentPinDrill(_) => OptimizationSurface::NotOptimizable {
                reason: RefuseReason::SteadyStateSamplesNotPresent,
            },
            // No wildcard arm. Adding a new OperationConfig variant
            // forces explicit classification at compile time.
        }
    }

    pub fn op_type(&self) -> OperationType {
        match self {
            OperationConfig::Face(_) => OperationType::Face,
            OperationConfig::Pocket(_) => OperationType::Pocket,
            OperationConfig::Profile(_) => OperationType::Profile,
            OperationConfig::Adaptive(_) => OperationType::Adaptive,
            OperationConfig::VCarve(_) => OperationType::VCarve,
            OperationConfig::Rest(_) => OperationType::Rest,
            OperationConfig::Inlay(_) => OperationType::Inlay,
            OperationConfig::Zigzag(_) => OperationType::Zigzag,
            OperationConfig::Trace(_) => OperationType::Trace,
            OperationConfig::Drill(_) => OperationType::Drill,
            OperationConfig::Chamfer(_) => OperationType::Chamfer,
            OperationConfig::DropCutter(_) => OperationType::DropCutter,
            OperationConfig::Adaptive3d(_) => OperationType::Adaptive3d,
            OperationConfig::Waterline(_) => OperationType::Waterline,
            OperationConfig::Pencil(_) => OperationType::Pencil,
            OperationConfig::Scallop(_) => OperationType::Scallop,
            OperationConfig::SteepShallow(_) => OperationType::SteepShallow,
            OperationConfig::RampFinish(_) => OperationType::RampFinish,
            OperationConfig::SpiralFinish(_) => OperationType::SpiralFinish,
            OperationConfig::RadialFinish(_) => OperationType::RadialFinish,
            OperationConfig::HorizontalFinish(_) => OperationType::HorizontalFinish,
            OperationConfig::ProjectCurve(_) => OperationType::ProjectCurve,
            OperationConfig::AlignmentPinDrill(_) => OperationType::AlignmentPinDrill,
        }
    }

    pub fn spec(&self) -> OperationSpec {
        self.op_type().spec()
    }

    pub fn label(&self) -> &'static str {
        self.spec().label
    }

    pub fn family(&self) -> OperationFamily {
        self.spec().family
    }

    pub fn geometry_requirement(&self) -> GeometryRequirement {
        self.spec().geometry
    }

    pub fn default_auto_regen(&self) -> bool {
        self.spec().default_auto_regen
    }

    pub fn ui_style(&self) -> (UiOperationFamily, UiProcessRole) {
        let spec = self.spec();
        (spec.ui_family, spec.ui_process_role)
    }

    pub fn feeds_style(&self) -> (FeedsOperationFamily, PassRole) {
        let spec = self.spec();
        (spec.feeds_family, spec.feeds_pass_role)
    }

    pub fn transform_capabilities(&self) -> OperationTransformCapabilities {
        self.op_type().transform_capabilities()
    }

    pub fn is_3d(&self) -> bool {
        self.family() == OperationFamily::ThreeD
    }

    pub fn is_stock_based(&self) -> bool {
        self.geometry_requirement() == GeometryRequirement::Stock
    }

    pub fn needs_both(&self) -> bool {
        self.geometry_requirement() == GeometryRequirement::Both
    }

    pub fn as_params(&self) -> &dyn OperationParams {
        match self {
            OperationConfig::Face(c) => c,
            OperationConfig::Pocket(c) => c,
            OperationConfig::Profile(c) => c,
            OperationConfig::Adaptive(c) => c,
            OperationConfig::VCarve(c) => c,
            OperationConfig::Rest(c) => c,
            OperationConfig::Inlay(c) => c,
            OperationConfig::Zigzag(c) => c,
            OperationConfig::Trace(c) => c,
            OperationConfig::Drill(c) => c,
            OperationConfig::Chamfer(c) => c,
            OperationConfig::DropCutter(c) => c,
            OperationConfig::Adaptive3d(c) => c,
            OperationConfig::Waterline(c) => c,
            OperationConfig::Pencil(c) => c,
            OperationConfig::Scallop(c) => c,
            OperationConfig::SteepShallow(c) => c,
            OperationConfig::RampFinish(c) => c,
            OperationConfig::SpiralFinish(c) => c,
            OperationConfig::RadialFinish(c) => c,
            OperationConfig::HorizontalFinish(c) => c,
            OperationConfig::ProjectCurve(c) => c,
            OperationConfig::AlignmentPinDrill(c) => c,
        }
    }

    pub fn as_params_mut(&mut self) -> &mut dyn OperationParams {
        match self {
            OperationConfig::Face(c) => c,
            OperationConfig::Pocket(c) => c,
            OperationConfig::Profile(c) => c,
            OperationConfig::Adaptive(c) => c,
            OperationConfig::VCarve(c) => c,
            OperationConfig::Rest(c) => c,
            OperationConfig::Inlay(c) => c,
            OperationConfig::Zigzag(c) => c,
            OperationConfig::Trace(c) => c,
            OperationConfig::Drill(c) => c,
            OperationConfig::Chamfer(c) => c,
            OperationConfig::DropCutter(c) => c,
            OperationConfig::Adaptive3d(c) => c,
            OperationConfig::Waterline(c) => c,
            OperationConfig::Pencil(c) => c,
            OperationConfig::Scallop(c) => c,
            OperationConfig::SteepShallow(c) => c,
            OperationConfig::RampFinish(c) => c,
            OperationConfig::SpiralFinish(c) => c,
            OperationConfig::RadialFinish(c) => c,
            OperationConfig::HorizontalFinish(c) => c,
            OperationConfig::ProjectCurve(c) => c,
            OperationConfig::AlignmentPinDrill(c) => c,
        }
    }

    pub fn feed_rate(&self) -> f64 {
        self.as_params().feed_rate()
    }

    pub fn set_feed_rate(&mut self, value: f64) {
        self.as_params_mut().set_feed_rate(value);
    }

    /// Returns the plunge rate.
    /// Drill ops are purely vertical -- feed_rate IS the plunge rate.
    pub fn plunge_rate(&self) -> f64 {
        self.as_params().plunge_rate()
    }

    pub fn set_plunge_rate(&mut self, value: f64) {
        self.as_params_mut().set_plunge_rate(value);
    }

    pub fn stepover(&self) -> Option<f64> {
        self.as_params().stepover()
    }

    pub fn set_stepover(&mut self, value: f64) {
        self.as_params_mut().set_stepover(value);
    }

    pub fn depth_per_pass(&self) -> Option<f64> {
        self.as_params().depth_per_pass()
    }

    pub fn set_depth_per_pass(&mut self, value: f64) {
        self.as_params_mut().set_depth_per_pass(value);
    }

    pub fn scallop_height(&self) -> Option<f64> {
        self.as_params().scallop_height()
    }

    pub fn set_scallop_height(&mut self, value: f64) {
        self.as_params_mut().set_scallop_height(value);
    }

    pub fn depth_semantics(&self) -> DepthSemantics {
        self.as_params().depth_semantics()
    }

    /// Per-toolpath spindle override. `None` means "use the project default".
    /// Resolve through [`effective_spindle_rpm`] to apply the fallback.
    pub fn spindle_rpm(&self) -> Option<u32> {
        self.as_params().spindle_rpm()
    }

    pub fn set_spindle_rpm(&mut self, rpm: Option<u32>) {
        self.as_params_mut().set_spindle_rpm(rpm);
    }

    pub fn default_depth_for_heights(&self) -> f64 {
        match self.depth_semantics() {
            DepthSemantics::Explicit(value) | DepthSemantics::DerivedStockTop(value) => value.abs(),
            DepthSemantics::None => 0.0,
        }
    }

    /// Operation params serialized as a plain object, with optional fields
    /// explicitly present as JSON null instead of omitted by
    /// `skip_serializing_if = "Option::is_none"`.
    pub fn params_value_including_nulls(&self) -> serde_json::Value {
        let mut value = serde_json::to_value(self).unwrap_or_else(|_| serde_json::json!({}));
        let mut params = value
            .get_mut("params")
            .and_then(|v| v.as_object_mut())
            .cloned()
            .unwrap_or_default();
        for def in param_defs_for_type(self.op_type()) {
            params
                .entry(def.name.to_owned())
                .or_insert(serde_json::Value::Null);
        }
        serde_json::Value::Object(params)
    }

    /// Schema hints keyed by param name for this operation kind.
    pub fn param_schema_hints(&self) -> std::collections::BTreeMap<String, ParamHint> {
        let defaults = Self::new_default(self.op_type()).params_value_including_nulls();
        let default_map = defaults.as_object();
        param_defs_for_type(self.op_type())
            .iter()
            .map(|def| {
                let default = default_map
                    .and_then(|m| m.get(def.name))
                    .cloned()
                    .unwrap_or(serde_json::Value::Null);
                (
                    def.name.to_owned(),
                    ParamHint {
                        type_name: def.type_name.to_owned(),
                        required: !def.optional,
                        optional: def.optional,
                        default,
                    },
                )
            })
            .collect()
    }

    /// Return the schema type string for one settable parameter, if known.
    pub fn param_type_name(&self, param: &str) -> Option<&'static str> {
        param_defs_for_type(self.op_type())
            .iter()
            .find(|def| def.name == param)
            .map(|def| def.type_name)
    }

    /// Schema-backed settable param names for this operation.
    pub fn param_names(&self) -> Vec<&'static str> {
        Self::param_names_for_type(self.op_type())
    }

    /// Schema-backed settable param names for an operation type.
    pub fn param_names_for_type(op_type: OperationType) -> Vec<&'static str> {
        param_defs_for_type(op_type)
            .iter()
            .map(|def| def.name)
            .collect()
    }

    /// Full operation schema for clients that need to discover params
    /// before a toolpath exists.
    pub fn schema_for_type(op_type: OperationType) -> OperationSchema {
        let defaults = Self::new_default(op_type).params_value_including_nulls();
        let default_map = defaults.as_object();
        let params = param_defs_for_type(op_type)
            .iter()
            .map(|def| OperationParamSchema {
                name: def.name.to_owned(),
                type_name: def.type_name.to_owned(),
                optional: def.optional,
                default: default_map
                    .and_then(|m| m.get(def.name))
                    .cloned()
                    .unwrap_or(serde_json::Value::Null),
                range: None,
                description: def.description.map(str::to_owned),
            })
            .collect();
        OperationSchema {
            operation_type: op_type.kind_str().to_owned(),
            label: op_type.label().to_owned(),
            params,
            tool_constraints: tool_constraints_for_type(op_type),
        }
    }
}

/// Lightweight per-field hint included beside `get_toolpath_params`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamHint {
    #[serde(rename = "type")]
    pub type_name: String,
    pub required: bool,
    pub optional: bool,
    pub default: serde_json::Value,
}

/// One operation parameter entry returned by `operation_schema`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationParamSchema {
    pub name: String,
    #[serde(rename = "type")]
    pub type_name: String,
    pub optional: bool,
    pub default: serde_json::Value,
    pub range: Option<serde_json::Value>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolConstraints {
    pub required_tool_type: Vec<String>,
    pub supports_v_bit: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationSchema {
    pub operation_type: String,
    pub label: String,
    pub params: Vec<OperationParamSchema>,
    pub tool_constraints: ToolConstraints,
}

#[derive(Debug, Clone, Copy)]
pub struct ParamDef {
    pub name: &'static str,
    pub type_name: &'static str,
    pub optional: bool,
    pub description: Option<&'static str>,
}

impl ParamDef {
    const fn required(name: &'static str, type_name: &'static str) -> Self {
        Self {
            name,
            type_name,
            optional: false,
            description: None,
        }
    }

    const fn optional(name: &'static str, type_name: &'static str) -> Self {
        Self {
            name,
            type_name,
            optional: true,
            description: None,
        }
    }

    const fn optional_desc(
        name: &'static str,
        type_name: &'static str,
        description: &'static str,
    ) -> Self {
        Self {
            name,
            type_name,
            optional: true,
            description: Some(description),
        }
    }
}

// ── Phase 1 operation registry (architectural refactor 2026-06-06) ────
//
// Data-only per-operation metadata table. Each entry pairs the
// operation's `OperationSpec`, its settable-param schema, and its tool
// constraints in one place. `OperationType::registry_entry` is the
// exhaustive accessor; the public helpers below delegate to it. There
// are deliberately NO wildcard fallbacks here — every field of every
// entry is an explicit decision ("miss nothing, or don't compile").

/// Static-friendly tool-constraint data for a registry entry.
/// Materialized into the serde-facing [`ToolConstraints`] by
/// [`Self::to_schema`].
#[derive(Debug, Clone, Copy)]
pub struct ToolConstraintsDef {
    /// Tool types (snake_case serde reprs) the operation requires; empty
    /// means any tool geometry is accepted.
    pub required_tool_type: &'static [&'static str],
    /// Whether a V-bit can run this operation at all.
    pub supports_v_bit: bool,
}

impl ToolConstraintsDef {
    /// Named "no restriction" policy: any tool geometry, V-bit included.
    /// Referenced explicitly by every unrestricted entry so the
    /// unrestricted set is a recorded decision, not a wildcard fallback.
    pub const ANY_TOOL: Self = Self {
        required_tool_type: &[],
        supports_v_bit: true,
    };

    /// Materialize the serde-facing [`ToolConstraints`].
    pub fn to_schema(&self) -> ToolConstraints {
        ToolConstraints {
            required_tool_type: self
                .required_tool_type
                .iter()
                .copied()
                .map(str::to_owned)
                .collect(),
            supports_v_bit: self.supports_v_bit,
        }
    }
}

/// One row of the Phase 1 operation registry. Data only — no behavior
/// function pointers until the Phase 5 adapters.
#[derive(Debug, Clone, Copy)]
pub struct OpRegistryEntry {
    pub op_type: OperationType,
    pub spec: OperationSpec,
    pub param_defs: &'static [ParamDef],
    pub tool_constraints: ToolConstraintsDef,
}

const FACE_PARAMS: &[ParamDef] = &[
    ParamDef::required("stepover", "f64"),
    ParamDef::required("depth", "f64"),
    ParamDef::required("depth_per_pass", "f64"),
    ParamDef::required("feed_rate", "f64"),
    ParamDef::required("plunge_rate", "f64"),
    ParamDef::required("stock_offset", "f64"),
    ParamDef::required("direction", "enum:one_way|zigzag"),
    ParamDef::optional("spindle_rpm", "option<u32>"),
];

const POCKET_PARAMS: &[ParamDef] = &[
    ParamDef::required("stepover", "f64"),
    ParamDef::required("depth", "f64"),
    ParamDef::required("depth_per_pass", "f64"),
    ParamDef::required("feed_rate", "f64"),
    ParamDef::required("plunge_rate", "f64"),
    ParamDef::required("climb", "bool"),
    ParamDef::required("pattern", "enum:contour|zigzag"),
    ParamDef::required("angle", "f64"),
    ParamDef::required("finishing_passes", "usize"),
    ParamDef::optional("spindle_rpm", "option<u32>"),
];

const PROFILE_PARAMS: &[ParamDef] = &[
    ParamDef::required("side", "enum:on|inside|outside"),
    ParamDef::required("depth", "f64"),
    ParamDef::required("depth_per_pass", "f64"),
    ParamDef::required("feed_rate", "f64"),
    ParamDef::required("plunge_rate", "f64"),
    ParamDef::required("climb", "bool"),
    ParamDef::required("tab_count", "usize"),
    ParamDef::required("tab_width", "f64"),
    ParamDef::required("tab_height", "f64"),
    ParamDef::required("finishing_passes", "usize"),
    ParamDef::required("compensation", "enum:in_computer|in_control"),
    ParamDef::optional("spindle_rpm", "option<u32>"),
];

const ADAPTIVE_PARAMS: &[ParamDef] = &[
    ParamDef::required("stepover", "f64"),
    ParamDef::required("depth", "f64"),
    ParamDef::required("depth_per_pass", "f64"),
    ParamDef::required("feed_rate", "f64"),
    ParamDef::required("plunge_rate", "f64"),
    ParamDef::required("tolerance", "f64"),
    ParamDef::required("slot_clearing", "bool"),
    ParamDef::required("min_cutting_radius", "f64"),
    ParamDef::optional("spindle_rpm", "option<u32>"),
    ParamDef::required(
        "cleanup_strategy",
        "enum:Legacy|ResidueMop|ContourParallelNarrow|ContourParallelHybrid",
    ),
];

const VCARVE_PARAMS: &[ParamDef] = &[
    ParamDef::required("max_depth", "f64"),
    ParamDef::required("stepover", "f64"),
    ParamDef::required("feed_rate", "f64"),
    ParamDef::required("plunge_rate", "f64"),
    ParamDef::required("tolerance", "f64"),
    ParamDef::optional("spindle_rpm", "option<u32>"),
];

const REST_PARAMS: &[ParamDef] = &[
    ParamDef::optional_desc(
        "prev_tool_id",
        "option<usize>",
        "Index of the prior (typically larger) tool used to define rest geometry",
    ),
    ParamDef::required("stepover", "f64"),
    ParamDef::required("depth", "f64"),
    ParamDef::required("depth_per_pass", "f64"),
    ParamDef::required("feed_rate", "f64"),
    ParamDef::required("plunge_rate", "f64"),
    ParamDef::required("angle", "f64"),
    ParamDef::optional("spindle_rpm", "option<u32>"),
];

const INLAY_PARAMS: &[ParamDef] = &[
    ParamDef::required("pocket_depth", "f64"),
    ParamDef::required("glue_gap", "f64"),
    ParamDef::required("flat_depth", "f64"),
    ParamDef::required("boundary_offset", "f64"),
    ParamDef::required("stepover", "f64"),
    ParamDef::required("flat_tool_radius", "f64"),
    ParamDef::required("feed_rate", "f64"),
    ParamDef::required("plunge_rate", "f64"),
    ParamDef::required("tolerance", "f64"),
    ParamDef::optional("spindle_rpm", "option<u32>"),
];

const ZIGZAG_PARAMS: &[ParamDef] = &[
    ParamDef::required("stepover", "f64"),
    ParamDef::required("depth", "f64"),
    ParamDef::required("depth_per_pass", "f64"),
    ParamDef::required("feed_rate", "f64"),
    ParamDef::required("plunge_rate", "f64"),
    ParamDef::required("angle", "f64"),
    ParamDef::optional("spindle_rpm", "option<u32>"),
];

const TRACE_PARAMS: &[ParamDef] = &[
    ParamDef::required("depth", "f64"),
    ParamDef::required("depth_per_pass", "f64"),
    ParamDef::required("feed_rate", "f64"),
    ParamDef::required("plunge_rate", "f64"),
    ParamDef::required("compensation", "enum:center|left|right"),
    ParamDef::optional("spindle_rpm", "option<u32>"),
];

const DRILL_PARAMS: &[ParamDef] = &[
    ParamDef::required("depth", "f64"),
    ParamDef::required("cycle", "enum:simple|dwell|peck|chip_break"),
    ParamDef::required("peck_depth", "f64"),
    ParamDef::required("dwell_time", "f64"),
    ParamDef::required("retract_amount", "f64"),
    ParamDef::required("feed_rate", "f64"),
    ParamDef::required("retract_z", "f64"),
    ParamDef::optional("spindle_rpm", "option<u32>"),
];

const CHAMFER_PARAMS: &[ParamDef] = &[
    ParamDef::required("chamfer_width", "f64"),
    ParamDef::required("tip_offset", "f64"),
    ParamDef::required("feed_rate", "f64"),
    ParamDef::required("plunge_rate", "f64"),
    ParamDef::optional("spindle_rpm", "option<u32>"),
];

const DROP_CUTTER_PARAMS: &[ParamDef] = &[
    ParamDef::required("stepover", "f64"),
    ParamDef::required("feed_rate", "f64"),
    ParamDef::required("plunge_rate", "f64"),
    ParamDef::required("min_z", "f64"),
    ParamDef::required("slope_from", "f64"),
    ParamDef::required("slope_to", "f64"),
    ParamDef::optional("spindle_rpm", "option<u32>"),
];

const ADAPTIVE3D_PARAMS: &[ParamDef] = &[
    ParamDef::required("stepover", "f64"),
    ParamDef::required("depth_per_pass", "f64"),
    ParamDef::required("stock_to_leave_radial", "f64"),
    ParamDef::required("stock_to_leave_axial", "f64"),
    ParamDef::required("feed_rate", "f64"),
    ParamDef::required("plunge_rate", "f64"),
    ParamDef::required("tolerance", "f64"),
    ParamDef::required("min_cutting_radius", "f64"),
    ParamDef::required("entry_style", "enum:plunge|helix|ramp"),
    ParamDef::required("ramp_angle_deg", "f64"),
    ParamDef::required("helix_radius_factor", "f64"),
    ParamDef::required("helix_pitch", "f64"),
    ParamDef::required("fine_stepdown", "f64"),
    ParamDef::required("detect_flat_areas", "bool"),
    ParamDef::required("region_ordering", "enum:global|by_area"),
    ParamDef::required(
        "clearing_strategy",
        "enum:contour_parallel|adaptive|agent_search",
    ),
    ParamDef::required("z_blend", "bool"),
    ParamDef::optional("spindle_rpm", "option<u32>"),
    ParamDef::required("mill_shallow_areas", "bool"),
    ParamDef::optional("shallow_angle_deg", "option<f64>"),
    ParamDef::optional("shallow_stepdown", "option<f64>"),
    ParamDef::required("min_region_cut_length_mm", "f64"),
    // F-038b: keep-tool-down link knobs.
    ParamDef::optional("max_stay_down_distance_mm", "option<f64>"),
    ParamDef::required("stay_down_clearance_mm", "f64"),
];

const WATERLINE_PARAMS: &[ParamDef] = &[
    ParamDef::required("z_step", "f64"),
    ParamDef::required("sampling", "f64"),
    ParamDef::required("feed_rate", "f64"),
    ParamDef::required("plunge_rate", "f64"),
    ParamDef::required("continuous", "bool"),
    ParamDef::optional("spindle_rpm", "option<u32>"),
];

const PENCIL_PARAMS: &[ParamDef] = &[
    ParamDef::required("bitangency_angle", "f64"),
    ParamDef::required("min_cut_length", "f64"),
    ParamDef::required("hookup_distance", "f64"),
    ParamDef::required("num_offset_passes", "usize"),
    ParamDef::required("offset_stepover", "f64"),
    ParamDef::required("sampling", "f64"),
    ParamDef::required("feed_rate", "f64"),
    ParamDef::required("plunge_rate", "f64"),
    ParamDef::required("stock_to_leave", "f64"),
    ParamDef::optional("spindle_rpm", "option<u32>"),
];

const SCALLOP_PARAMS: &[ParamDef] = &[
    ParamDef::required("scallop_height", "f64"),
    ParamDef::required("tolerance", "f64"),
    ParamDef::required("direction", "enum:x|y"),
    ParamDef::required("continuous", "bool"),
    ParamDef::required("slope_from", "f64"),
    ParamDef::required("slope_to", "f64"),
    ParamDef::required("feed_rate", "f64"),
    ParamDef::required("plunge_rate", "f64"),
    ParamDef::required("stock_to_leave", "f64"),
    ParamDef::optional("spindle_rpm", "option<u32>"),
];

const STEEP_SHALLOW_PARAMS: &[ParamDef] = &[
    ParamDef::required("threshold_angle", "f64"),
    ParamDef::required("overlap_distance", "f64"),
    ParamDef::required("wall_clearance", "f64"),
    ParamDef::required("steep_first", "bool"),
    ParamDef::required("stepover", "f64"),
    ParamDef::required("z_step", "f64"),
    ParamDef::required("feed_rate", "f64"),
    ParamDef::required("plunge_rate", "f64"),
    ParamDef::required("sampling", "f64"),
    ParamDef::required("stock_to_leave", "f64"),
    ParamDef::required("tolerance", "f64"),
    ParamDef::optional("spindle_rpm", "option<u32>"),
];

const RAMP_FINISH_PARAMS: &[ParamDef] = &[
    ParamDef::required("max_stepdown", "f64"),
    ParamDef::required("slope_from", "f64"),
    ParamDef::required("slope_to", "f64"),
    ParamDef::required("direction", "enum:climb|conventional"),
    ParamDef::required("order_bottom_up", "bool"),
    ParamDef::required("feed_rate", "f64"),
    ParamDef::required("plunge_rate", "f64"),
    ParamDef::required("sampling", "f64"),
    ParamDef::required("stock_to_leave", "f64"),
    ParamDef::required("tolerance", "f64"),
    ParamDef::optional("spindle_rpm", "option<u32>"),
];

const SPIRAL_FINISH_PARAMS: &[ParamDef] = &[
    ParamDef::required("stepover", "f64"),
    ParamDef::required("direction", "enum:outward|inward"),
    ParamDef::required("feed_rate", "f64"),
    ParamDef::required("plunge_rate", "f64"),
    ParamDef::required("stock_to_leave", "f64"),
    ParamDef::optional("spindle_rpm", "option<u32>"),
];

const RADIAL_FINISH_PARAMS: &[ParamDef] = &[
    ParamDef::required("angular_step", "f64"),
    ParamDef::required("point_spacing", "f64"),
    ParamDef::required("feed_rate", "f64"),
    ParamDef::required("plunge_rate", "f64"),
    ParamDef::required("stock_to_leave", "f64"),
    ParamDef::optional("spindle_rpm", "option<u32>"),
];

const HORIZONTAL_FINISH_PARAMS: &[ParamDef] = &[
    ParamDef::required("angle_threshold", "f64"),
    ParamDef::required("stepover", "f64"),
    ParamDef::required("feed_rate", "f64"),
    ParamDef::required("plunge_rate", "f64"),
    ParamDef::required("stock_to_leave", "f64"),
    ParamDef::optional("spindle_rpm", "option<u32>"),
];

const PROJECT_CURVE_PARAMS: &[ParamDef] = &[
    ParamDef::required("depth", "f64"),
    ParamDef::required("point_spacing", "f64"),
    ParamDef::required("feed_rate", "f64"),
    ParamDef::required("plunge_rate", "f64"),
    ParamDef::optional("surface_model_id", "option<usize>"),
    ParamDef::required("direction", "enum:from_above|from_below"),
    ParamDef::required("side", "enum:center|inside|outside"),
    ParamDef::optional("spindle_rpm", "option<u32>"),
];

const ALIGNMENT_PIN_DRILL_PARAMS: &[ParamDef] = &[
    ParamDef::required("holes", "array<[f64;2]>"),
    ParamDef::required("spoilboard_penetration", "f64"),
    ParamDef::required("cycle", "enum:simple|dwell|peck|chip_break"),
    ParamDef::required("peck_depth", "f64"),
    ParamDef::required("feed_rate", "f64"),
    ParamDef::required("retract_z", "f64"),
    ParamDef::optional("spindle_rpm", "option<u32>"),
];

static REG_FACE: OpRegistryEntry = OpRegistryEntry {
    op_type: OperationType::Face,
    spec: OperationSpec {
        label: "Face",
        description: "Level the stock top surface",
        family: OperationFamily::TwoPointFiveD,
        geometry: GeometryRequirement::Stock,
        default_auto_regen: true,
        ui_family: UiOperationFamily::Pocket,
        ui_process_role: UiProcessRole::Roughing,
        feeds_family: FeedsOperationFamily::Pocket,
        feeds_pass_role: PassRole::Roughing,
    },
    param_defs: FACE_PARAMS,
    tool_constraints: ToolConstraintsDef::ANY_TOOL,
};

static REG_POCKET: OpRegistryEntry = OpRegistryEntry {
    op_type: OperationType::Pocket,
    spec: OperationSpec {
        label: "Pocket",
        description: "Clear material inside a closed region",
        family: OperationFamily::TwoPointFiveD,
        geometry: GeometryRequirement::Polygons,
        default_auto_regen: true,
        ui_family: UiOperationFamily::Pocket,
        ui_process_role: UiProcessRole::Roughing,
        feeds_family: FeedsOperationFamily::Pocket,
        feeds_pass_role: PassRole::Roughing,
    },
    param_defs: POCKET_PARAMS,
    tool_constraints: ToolConstraintsDef::ANY_TOOL,
};

static REG_PROFILE: OpRegistryEntry = OpRegistryEntry {
    op_type: OperationType::Profile,
    spec: OperationSpec {
        label: "Profile",
        description: "Cut along the outside or inside of a boundary",
        family: OperationFamily::TwoPointFiveD,
        geometry: GeometryRequirement::Polygons,
        default_auto_regen: true,
        ui_family: UiOperationFamily::Contour,
        ui_process_role: UiProcessRole::Roughing,
        feeds_family: FeedsOperationFamily::Contour,
        feeds_pass_role: PassRole::Roughing,
    },
    param_defs: PROFILE_PARAMS,
    tool_constraints: ToolConstraintsDef::ANY_TOOL,
};

static REG_ADAPTIVE: OpRegistryEntry = OpRegistryEntry {
    op_type: OperationType::Adaptive,
    spec: OperationSpec {
        label: "Adaptive",
        description: "Constant-engagement rough clearing",
        family: OperationFamily::TwoPointFiveD,
        geometry: GeometryRequirement::Polygons,
        default_auto_regen: true,
        ui_family: UiOperationFamily::Pocket,
        ui_process_role: UiProcessRole::Roughing,
        feeds_family: FeedsOperationFamily::Adaptive,
        feeds_pass_role: PassRole::Roughing,
    },
    param_defs: ADAPTIVE_PARAMS,
    tool_constraints: ToolConstraintsDef::ANY_TOOL,
};

static REG_VCARVE: OpRegistryEntry = OpRegistryEntry {
    op_type: OperationType::VCarve,
    spec: OperationSpec {
        label: "VCarve",
        description: "V-bit engraving with variable depth",
        family: OperationFamily::TwoPointFiveD,
        geometry: GeometryRequirement::Polygons,
        default_auto_regen: true,
        ui_family: UiOperationFamily::Trace,
        ui_process_role: UiProcessRole::Finish,
        feeds_family: FeedsOperationFamily::Trace,
        feeds_pass_role: PassRole::Finish,
    },
    param_defs: VCARVE_PARAMS,
    tool_constraints: ToolConstraintsDef {
        required_tool_type: &["v_bit"],
        supports_v_bit: true,
    },
};

static REG_REST: OpRegistryEntry = OpRegistryEntry {
    op_type: OperationType::Rest,
    spec: OperationSpec {
        label: "Rest Machining",
        description: "Clean up areas a larger tool couldn\u{2019}t reach",
        family: OperationFamily::TwoPointFiveD,
        geometry: GeometryRequirement::Polygons,
        default_auto_regen: true,
        ui_family: UiOperationFamily::Pocket,
        ui_process_role: UiProcessRole::Roughing,
        feeds_family: FeedsOperationFamily::Pocket,
        feeds_pass_role: PassRole::Roughing,
    },
    param_defs: REST_PARAMS,
    tool_constraints: ToolConstraintsDef::ANY_TOOL,
};

static REG_INLAY: OpRegistryEntry = OpRegistryEntry {
    op_type: OperationType::Inlay,
    spec: OperationSpec {
        label: "Inlay",
        description: "V-bit pocket and plug for inlay work",
        family: OperationFamily::TwoPointFiveD,
        geometry: GeometryRequirement::Polygons,
        default_auto_regen: true,
        ui_family: UiOperationFamily::Trace,
        ui_process_role: UiProcessRole::Finish,
        feeds_family: FeedsOperationFamily::Trace,
        feeds_pass_role: PassRole::Finish,
    },
    param_defs: INLAY_PARAMS,
    tool_constraints: ToolConstraintsDef {
        required_tool_type: &["v_bit"],
        supports_v_bit: true,
    },
};

static REG_ZIGZAG: OpRegistryEntry = OpRegistryEntry {
    op_type: OperationType::Zigzag,
    spec: OperationSpec {
        label: "Zigzag",
        description: "Back-and-forth raster clearing at an angle",
        family: OperationFamily::TwoPointFiveD,
        geometry: GeometryRequirement::Polygons,
        default_auto_regen: true,
        ui_family: UiOperationFamily::Pocket,
        ui_process_role: UiProcessRole::Roughing,
        feeds_family: FeedsOperationFamily::Pocket,
        feeds_pass_role: PassRole::Roughing,
    },
    param_defs: ZIGZAG_PARAMS,
    tool_constraints: ToolConstraintsDef::ANY_TOOL,
};

static REG_TRACE: OpRegistryEntry = OpRegistryEntry {
    op_type: OperationType::Trace,
    spec: OperationSpec {
        label: "Trace",
        description: "Follow a path exactly for engraving or scoring",
        family: OperationFamily::TwoPointFiveD,
        geometry: GeometryRequirement::Polygons,
        default_auto_regen: true,
        ui_family: UiOperationFamily::Trace,
        ui_process_role: UiProcessRole::Finish,
        feeds_family: FeedsOperationFamily::Trace,
        feeds_pass_role: PassRole::Finish,
    },
    param_defs: TRACE_PARAMS,
    tool_constraints: ToolConstraintsDef::ANY_TOOL,
};

static REG_DRILL: OpRegistryEntry = OpRegistryEntry {
    op_type: OperationType::Drill,
    spec: OperationSpec {
        label: "Drill",
        description: "Drill holes from SVG circle positions",
        family: OperationFamily::TwoPointFiveD,
        geometry: GeometryRequirement::Polygons,
        default_auto_regen: true,
        ui_family: UiOperationFamily::Pocket,
        ui_process_role: UiProcessRole::Roughing,
        feeds_family: FeedsOperationFamily::Drill,
        feeds_pass_role: PassRole::Roughing,
    },
    param_defs: DRILL_PARAMS,
    tool_constraints: ToolConstraintsDef::ANY_TOOL,
};

static REG_CHAMFER: OpRegistryEntry = OpRegistryEntry {
    op_type: OperationType::Chamfer,
    spec: OperationSpec {
        label: "Chamfer",
        description: "Bevel edges with a V-bit",
        family: OperationFamily::TwoPointFiveD,
        geometry: GeometryRequirement::Polygons,
        default_auto_regen: true,
        ui_family: UiOperationFamily::Trace,
        ui_process_role: UiProcessRole::Finish,
        feeds_family: FeedsOperationFamily::Trace,
        feeds_pass_role: PassRole::Finish,
    },
    param_defs: CHAMFER_PARAMS,
    tool_constraints: ToolConstraintsDef {
        required_tool_type: &["v_bit"],
        supports_v_bit: true,
    },
};

static REG_DROP_CUTTER: OpRegistryEntry = OpRegistryEntry {
    op_type: OperationType::DropCutter,
    spec: OperationSpec {
        label: "3D Finish",
        description: "Parallel raster passes following the surface",
        family: OperationFamily::ThreeD,
        geometry: GeometryRequirement::Mesh,
        default_auto_regen: false,
        ui_family: UiOperationFamily::Parallel,
        ui_process_role: UiProcessRole::Finish,
        feeds_family: FeedsOperationFamily::Parallel,
        feeds_pass_role: PassRole::Finish,
    },
    param_defs: DROP_CUTTER_PARAMS,
    tool_constraints: ToolConstraintsDef::ANY_TOOL,
};

static REG_ADAPTIVE3D: OpRegistryEntry = OpRegistryEntry {
    op_type: OperationType::Adaptive3d,
    spec: OperationSpec {
        label: "3D Rough",
        description: "Load-limiting rough mill on a 3D surface",
        family: OperationFamily::ThreeD,
        geometry: GeometryRequirement::Mesh,
        default_auto_regen: false,
        ui_family: UiOperationFamily::Pocket,
        ui_process_role: UiProcessRole::Roughing,
        feeds_family: FeedsOperationFamily::Adaptive,
        feeds_pass_role: PassRole::Roughing,
    },
    param_defs: ADAPTIVE3D_PARAMS,
    tool_constraints: ToolConstraintsDef::ANY_TOOL,
};

static REG_WATERLINE: OpRegistryEntry = OpRegistryEntry {
    op_type: OperationType::Waterline,
    spec: OperationSpec {
        label: "Waterline",
        description: "Horizontal contours at constant Z levels",
        family: OperationFamily::ThreeD,
        geometry: GeometryRequirement::Mesh,
        default_auto_regen: false,
        ui_family: UiOperationFamily::Contour,
        ui_process_role: UiProcessRole::SemiFinish,
        feeds_family: FeedsOperationFamily::Contour,
        feeds_pass_role: PassRole::SemiFinish,
    },
    param_defs: WATERLINE_PARAMS,
    tool_constraints: ToolConstraintsDef::ANY_TOOL,
};

static REG_PENCIL: OpRegistryEntry = OpRegistryEntry {
    op_type: OperationType::Pencil,
    spec: OperationSpec {
        label: "Pencil Finish",
        description: "Trace concave edges and creases on the surface",
        family: OperationFamily::ThreeD,
        geometry: GeometryRequirement::Mesh,
        default_auto_regen: false,
        ui_family: UiOperationFamily::Trace,
        ui_process_role: UiProcessRole::Finish,
        feeds_family: FeedsOperationFamily::Trace,
        feeds_pass_role: PassRole::Finish,
    },
    param_defs: PENCIL_PARAMS,
    tool_constraints: ToolConstraintsDef::ANY_TOOL,
};

static REG_SCALLOP: OpRegistryEntry = OpRegistryEntry {
    op_type: OperationType::Scallop,
    spec: OperationSpec {
        label: "Scallop Finish",
        description: "Variable stepover for constant scallop height",
        family: OperationFamily::ThreeD,
        geometry: GeometryRequirement::Mesh,
        default_auto_regen: false,
        ui_family: UiOperationFamily::Scallop,
        ui_process_role: UiProcessRole::Finish,
        feeds_family: FeedsOperationFamily::Scallop,
        feeds_pass_role: PassRole::Finish,
    },
    param_defs: SCALLOP_PARAMS,
    tool_constraints: ToolConstraintsDef {
        required_tool_type: &["ball_nose", "tapered_ball_nose"],
        supports_v_bit: false,
    },
};

static REG_STEEP_SHALLOW: OpRegistryEntry = OpRegistryEntry {
    op_type: OperationType::SteepShallow,
    spec: OperationSpec {
        label: "Steep/Shallow",
        description: "Waterline on steep areas, raster on shallow",
        family: OperationFamily::ThreeD,
        geometry: GeometryRequirement::Mesh,
        default_auto_regen: false,
        ui_family: UiOperationFamily::Contour,
        ui_process_role: UiProcessRole::Finish,
        feeds_family: FeedsOperationFamily::Contour,
        feeds_pass_role: PassRole::Finish,
    },
    param_defs: STEEP_SHALLOW_PARAMS,
    tool_constraints: ToolConstraintsDef::ANY_TOOL,
};

static REG_RAMP_FINISH: OpRegistryEntry = OpRegistryEntry {
    op_type: OperationType::RampFinish,
    spec: OperationSpec {
        label: "Ramp Finish",
        description: "Continuous Z descent along contours, no retract",
        family: OperationFamily::ThreeD,
        geometry: GeometryRequirement::Mesh,
        default_auto_regen: false,
        ui_family: UiOperationFamily::Parallel,
        ui_process_role: UiProcessRole::Finish,
        feeds_family: FeedsOperationFamily::Parallel,
        feeds_pass_role: PassRole::Finish,
    },
    param_defs: RAMP_FINISH_PARAMS,
    tool_constraints: ToolConstraintsDef::ANY_TOOL,
};

static REG_SPIRAL_FINISH: OpRegistryEntry = OpRegistryEntry {
    op_type: OperationType::SpiralFinish,
    spec: OperationSpec {
        label: "Spiral Finish",
        description: "Archimedean spiral passes over the surface",
        family: OperationFamily::ThreeD,
        geometry: GeometryRequirement::Mesh,
        default_auto_regen: false,
        ui_family: UiOperationFamily::Scallop,
        ui_process_role: UiProcessRole::Finish,
        feeds_family: FeedsOperationFamily::Scallop,
        feeds_pass_role: PassRole::Finish,
    },
    param_defs: SPIRAL_FINISH_PARAMS,
    tool_constraints: ToolConstraintsDef::ANY_TOOL,
};

static REG_RADIAL_FINISH: OpRegistryEntry = OpRegistryEntry {
    op_type: OperationType::RadialFinish,
    spec: OperationSpec {
        label: "Radial Finish",
        description: "Spoke-pattern passes radiating from center",
        family: OperationFamily::ThreeD,
        geometry: GeometryRequirement::Mesh,
        default_auto_regen: false,
        ui_family: UiOperationFamily::Parallel,
        ui_process_role: UiProcessRole::Finish,
        feeds_family: FeedsOperationFamily::Parallel,
        feeds_pass_role: PassRole::Finish,
    },
    param_defs: RADIAL_FINISH_PARAMS,
    tool_constraints: ToolConstraintsDef::ANY_TOOL,
};

static REG_HORIZONTAL_FINISH: OpRegistryEntry = OpRegistryEntry {
    op_type: OperationType::HorizontalFinish,
    spec: OperationSpec {
        label: "Horizontal Finish",
        description: "Finish only flat areas of the surface",
        family: OperationFamily::ThreeD,
        geometry: GeometryRequirement::Mesh,
        default_auto_regen: false,
        ui_family: UiOperationFamily::Parallel,
        ui_process_role: UiProcessRole::Finish,
        feeds_family: FeedsOperationFamily::Parallel,
        feeds_pass_role: PassRole::Finish,
    },
    param_defs: HORIZONTAL_FINISH_PARAMS,
    tool_constraints: ToolConstraintsDef::ANY_TOOL,
};

static REG_PROJECT_CURVE: OpRegistryEntry = OpRegistryEntry {
    op_type: OperationType::ProjectCurve,
    spec: OperationSpec {
        label: "Project Curve",
        description: "Project 2D curves onto a 3D mesh surface",
        family: OperationFamily::ThreeD,
        geometry: GeometryRequirement::Both,
        default_auto_regen: false,
        ui_family: UiOperationFamily::Trace,
        ui_process_role: UiProcessRole::Finish,
        feeds_family: FeedsOperationFamily::Trace,
        feeds_pass_role: PassRole::Finish,
    },
    param_defs: PROJECT_CURVE_PARAMS,
    tool_constraints: ToolConstraintsDef::ANY_TOOL,
};

static REG_ALIGNMENT_PIN_DRILL: OpRegistryEntry = OpRegistryEntry {
    op_type: OperationType::AlignmentPinDrill,
    spec: OperationSpec {
        label: "Pin Drill",
        description: "Drill alignment pin holes through stock",
        family: OperationFamily::TwoPointFiveD,
        geometry: GeometryRequirement::Stock,
        default_auto_regen: true,
        ui_family: UiOperationFamily::Pocket,
        ui_process_role: UiProcessRole::Roughing,
        feeds_family: FeedsOperationFamily::Drill,
        feeds_pass_role: PassRole::Roughing,
    },
    param_defs: ALIGNMENT_PIN_DRILL_PARAMS,
    tool_constraints: ToolConstraintsDef::ANY_TOOL,
};

fn param_defs_for_type(op_type: OperationType) -> &'static [ParamDef] {
    op_type.registry_entry().param_defs
}

fn tool_constraints_for_type(op_type: OperationType) -> ToolConstraints {
    op_type.registry_entry().tool_constraints.to_schema()
}

/// Stock context for [`OperationConfig::new_default_with_ctx`] and
/// [`OperationConfig::apply_stock_defaults`]. Carries the few stock
/// dimensions a sensible per-op depth default needs to know about.
#[derive(Debug, Clone, Copy)]
pub struct NewDefaultCtx {
    /// Top of the stock in the part-Z frame (typically 0 for 3D, may be
    /// negative for 2D where the stock origin is below Z=0).
    pub stock_top_z: f64,
    /// Bottom of the stock in the part-Z frame.
    pub stock_bottom_z: f64,
    /// Stock thickness in mm (`stock_top_z - stock_bottom_z`).
    pub stock_z: f64,
    /// Stock-top minus model-top margin in mm. Drives the face skim
    /// depth default. Falls back to `5.0` when no model is loaded.
    pub stock_padding: f64,
}

impl NewDefaultCtx {
    /// Build a context from a session's stock config + bbox. Use this
    /// from production sites that have a `&ProjectSession` in scope.
    pub fn from_stock_bbox(bbox: crate::geo::BoundingBox3, padding: f64) -> Self {
        let stock_z = (bbox.max.z - bbox.min.z).max(0.0);
        Self {
            stock_top_z: bbox.max.z,
            stock_bottom_z: bbox.min.z,
            stock_z,
            stock_padding: padding,
        }
    }
}

impl OperationConfig {
    /// Construct a fresh op config and immediately apply stock-aware
    /// depth defaults via [`Self::apply_stock_defaults`]. This is the
    /// preferred constructor at production sites (controller/MCP) where
    /// the session's stock is in scope; tests can keep using
    /// [`Self::new_default`].
    pub fn new_default_with_ctx(op_type: OperationType, ctx: &NewDefaultCtx) -> Self {
        let mut cfg = Self::new_default(op_type);
        cfg.apply_stock_defaults(ctx);
        cfg
    }

    /// Apply stock-aware overrides to depth-style fields that the
    /// per-config `Default` impl can't see (it has no stock context).
    /// Roadmap B.1–B.3: drop_cutter `min_z`, face `depth`, pocket /
    /// profile / drill / adaptive `depth`. No-op for ops whose default
    /// is already stock-agnostic.
    pub fn apply_stock_defaults(&mut self, ctx: &NewDefaultCtx) {
        let stock_ctx = crate::feeds::suggest::StockContext {
            stock_top_z: ctx.stock_top_z,
            stock_bottom_z: ctx.stock_bottom_z,
            stock_z: ctx.stock_z,
            stock_padding: ctx.stock_padding,
        };
        crate::feeds::suggest::apply_stock_defaults(self, &stock_ctx);
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
            OperationType::SteepShallow => {
                OperationConfig::SteepShallow(SteepShallowConfig::default())
            }
            OperationType::RampFinish => OperationConfig::RampFinish(RampFinishConfig::default()),
            OperationType::SpiralFinish => {
                OperationConfig::SpiralFinish(SpiralFinishConfig::default())
            }
            OperationType::RadialFinish => {
                OperationConfig::RadialFinish(RadialFinishConfig::default())
            }
            OperationType::HorizontalFinish => {
                OperationConfig::HorizontalFinish(HorizontalFinishConfig::default())
            }
            OperationType::ProjectCurve => {
                OperationConfig::ProjectCurve(ProjectCurveConfig::default())
            }
            OperationType::AlignmentPinDrill => {
                OperationConfig::AlignmentPinDrill(AlignmentPinDrillConfig::default())
            }
        }
    }

    /// Pre-compute Z levels for depth stepping (top -> bottom).
    ///
    /// Returns an empty `Vec` for operations that don't use standard depth
    /// stepping (3D ops, VCarve, Chamfer, Inlay, Drill).
    pub fn cutting_levels(&self, top_z: f64) -> Vec<f64> {
        use crate::depth::{DepthDistribution, DepthStepping};
        match self {
            Self::Pocket(cfg) => DepthStepping {
                start_z: top_z,
                final_z: top_z - cfg.depth.abs(),
                max_step_down: cfg.depth_per_pass,
                distribution: DepthDistribution::Even,
                finish_allowance: 0.0,
                finishing_passes: cfg.finishing_passes,
            }
            .all_levels(),
            Self::Profile(cfg) => DepthStepping {
                start_z: top_z,
                final_z: top_z - cfg.depth.abs(),
                max_step_down: cfg.depth_per_pass,
                distribution: DepthDistribution::Even,
                finish_allowance: 0.0,
                finishing_passes: cfg.finishing_passes,
            }
            .all_levels(),
            Self::Adaptive(cfg) => {
                DepthStepping::new(top_z, top_z - cfg.depth.abs(), cfg.depth_per_pass).all_levels()
            }
            Self::Zigzag(cfg) => {
                DepthStepping::new(top_z, top_z - cfg.depth.abs(), cfg.depth_per_pass).all_levels()
            }
            Self::Rest(cfg) => {
                DepthStepping::new(top_z, top_z - cfg.depth.abs(), cfg.depth_per_pass).all_levels()
            }
            Self::Trace(cfg) => {
                DepthStepping::new(top_z, top_z - cfg.depth.abs(), cfg.depth_per_pass).all_levels()
            }
            Self::Face(cfg) => {
                if cfg.depth <= 0.0 {
                    vec![top_z]
                } else {
                    DepthStepping::new(top_z, top_z - cfg.depth.abs(), cfg.depth_per_pass)
                        .all_levels()
                }
            }
            // 3D ops, VCarve, Chamfer, Inlay, Drill, AlignmentPinDrill — no standard depth stepping
            _ => vec![],
        }
    }
}

/// Resolve the spindle RPM for an operation, falling back to the project
/// default (`PostConfig.spindle_speed` / `ProjectPostConfig.spindle_speed`)
/// when the operation has no override.
///
/// This is the single source of truth for "what RPM should this toolpath
/// run at"; consumers that emit G-code, drive the simulator, or compute
/// chip load must call this rather than reading `post.spindle_speed`
/// directly so that per-toolpath overrides are honored. Callers pass the
/// project-level default RPM as a plain `u32` so this helper does not need
/// to know about the multiple `PostConfig` types in the workspace.
pub fn effective_spindle_rpm(op: &OperationConfig, project_default_rpm: u32) -> u32 {
    op.spindle_rpm().unwrap_or(project_default_rpm)
}

pub fn feed_optimization_unavailable_reason(
    operation: &OperationConfig,
    stock_source: StockSource,
) -> Option<&'static str> {
    if stock_source == StockSource::FromRemainingStock {
        return Some(
            "Phase 1 feed optimization only supports fresh stock, not remaining-stock workflows.",
        );
    }
    if matches!(operation, OperationConfig::Rest(_)) {
        return Some(
            "Rest machining depends on prior tool removal, so feed optimization is disabled for now.",
        );
    }
    if operation.is_3d() {
        return Some(
            "Phase 1 feed optimization only supports operations that start from flat stock, not mesh-derived surfaces.",
        );
    }
    None
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn operation_catalog_is_exhaustive_and_consistent() {
        assert_eq!(OperationType::ALL.len(), 23);
        for &op_type in OperationType::ALL {
            let config = OperationConfig::new_default(op_type);
            assert_eq!(config.op_type(), op_type);
            assert_eq!(config.label(), op_type.label());

            // Phase 1 registry self-consistency: one entry per op, and
            // the entry agrees with the op it claims to describe.
            let entry = op_type.registry_entry();
            assert_eq!(entry.op_type, op_type, "registry entry op_type mismatch");
            assert_eq!(entry.spec.label, op_type.label());
            assert!(
                !entry.param_defs.is_empty(),
                "{op_type:?}: registry entry has no settable params — \
                 every op exposes at least one"
            );
        }
    }

    /// Phase 1 wildcard kill (architectural refactor T3): tool
    /// constraints are now an explicit per-entry registry field. This
    /// pins (a) the four restricted ops exactly, and (b) that the 19
    /// previously-wildcard-defaulted ops still resolve to the named
    /// `ANY_TOOL` policy — proving the consolidation changed no
    /// behavior. A new op must reference a policy explicitly; there is
    /// no fallback arm left to inherit silently.
    #[test]
    fn tool_constraints_are_an_explicit_per_op_decision() {
        let mut unrestricted = 0;
        for &op_type in OperationType::ALL {
            let tc = op_type.registry_entry().tool_constraints;
            match op_type {
                OperationType::VCarve | OperationType::Inlay | OperationType::Chamfer => {
                    assert_eq!(tc.required_tool_type, ["v_bit"], "{op_type:?}");
                    assert!(tc.supports_v_bit, "{op_type:?}");
                }
                OperationType::Scallop => {
                    assert_eq!(tc.required_tool_type, ["ball_nose", "tapered_ball_nose"]);
                    assert!(!tc.supports_v_bit);
                }
                _ => {
                    // Pre-registry these 19 fell through `_ => (Vec::new(), true)`.
                    assert!(
                        tc.required_tool_type.is_empty(),
                        "{op_type:?}: expected the ANY_TOOL policy"
                    );
                    assert!(tc.supports_v_bit, "{op_type:?}");
                    unrestricted += 1;
                }
            }
            // The serde-facing materialization agrees with the def.
            let schema = tc.to_schema();
            assert_eq!(schema.required_tool_type, tc.required_tool_type);
            assert_eq!(schema.supports_v_bit, tc.supports_v_bit);
        }
        assert_eq!(
            unrestricted, 19,
            "unrestricted-op count changed — decide deliberately"
        );
    }

    /// Parity freeze (architectural refactor §7.2): `ALL` is exactly the
    /// disjoint union of `ALL_2D`, `ALL_3D`, and the NAMED system-only
    /// set. A new op added to `ALL` without being placed in a menu
    /// sublist (or explicitly listed as system-only here) fails this
    /// test — placement is a recorded decision, not an accident.
    #[test]
    fn operation_partitions_cover_all_variants_once() {
        use std::collections::HashSet;

        // The ops deliberately absent from both user menus. Keep this
        // list in sync with intent, not convenience.
        const SYSTEM_ONLY: &[OperationType] = &[OperationType::AlignmentPinDrill];

        let all: HashSet<_> = OperationType::ALL.iter().collect();
        assert_eq!(
            all.len(),
            OperationType::ALL.len(),
            "OperationType::ALL contains duplicates"
        );

        let twod: HashSet<_> = OperationType::ALL_2D.iter().collect();
        let threed: HashSet<_> = OperationType::ALL_3D.iter().collect();
        let system: HashSet<_> = SYSTEM_ONLY.iter().collect();
        assert!(twod.is_disjoint(&threed), "ALL_2D and ALL_3D overlap");
        assert!(system.is_disjoint(&twod), "system-only op listed in ALL_2D");
        assert!(
            system.is_disjoint(&threed),
            "system-only op listed in ALL_3D"
        );

        let union: HashSet<_> = twod
            .union(&threed)
            .copied()
            .collect::<HashSet<_>>()
            .union(&system)
            .copied()
            .collect();
        assert_eq!(
            union, all,
            "ALL_2D ∪ ALL_3D ∪ SYSTEM_ONLY must equal OperationType::ALL exactly \
             — place every new op in a menu sublist or name it system-only"
        );
    }

    /// Parity freeze (architectural refactor §7.2): the externally-tagged
    /// `{kind, params}` serde shape of [`OperationConfig`]. The MCP
    /// `set_toolpath_param` round-trip depends on `params` being a
    /// mutable object and `kind` being the snake_case op name; project
    /// TOML on disk depends on the same shape. A careless registry /
    /// X-macro change that alters this breaks saved projects and the MCP
    /// surface silently — this test makes it loud.
    #[test]
    fn operation_config_serde_shape_is_kind_params() {
        for &op_type in OperationType::ALL {
            let config = OperationConfig::new_default(op_type);

            // JSON view (MCP surface).
            let json = serde_json::to_value(&config).expect("serialize op config to JSON");
            let obj = json.as_object().expect("op config must be a JSON object");
            assert_eq!(
                obj.keys().collect::<Vec<_>>(),
                ["kind", "params"],
                "{op_type:?}: serde shape must be exactly {{kind, params}}"
            );
            assert_eq!(
                obj.get("kind").and_then(|k| k.as_str()),
                Some(op_type.kind_str()),
                "{op_type:?}: `kind` tag must equal kind_str()"
            );
            assert!(
                obj.get("params").is_some_and(serde_json::Value::is_object),
                "{op_type:?}: `params` must be a mutable JSON object"
            );

            // TOML view (project files on disk) — must round-trip.
            let toml_str = toml::to_string(&config).expect("serialize op config to TOML");
            assert!(
                toml_str.contains("kind = "),
                "{op_type:?}: TOML must carry the `kind` tag"
            );
            let back: OperationConfig =
                toml::from_str(&toml_str).expect("round-trip op config from TOML");
            assert_eq!(
                back.op_type(),
                op_type,
                "{op_type:?}: TOML round-trip changed the operation kind"
            );
        }
    }

    /// Parity freeze (architectural refactor §7.2): the snake_case serde
    /// repr of every [`OperationType`] — these strings are canonical in
    /// project TOML, the MCP wire format, and MCP error messages. Pinned
    /// as literals (not derived) so a rename anywhere fails here first.
    #[test]
    fn operation_type_serde_repr_pinned() {
        const PINNED: &[(&str, OperationType)] = &[
            ("face", OperationType::Face),
            ("pocket", OperationType::Pocket),
            ("profile", OperationType::Profile),
            ("adaptive", OperationType::Adaptive),
            ("v_carve", OperationType::VCarve),
            ("rest", OperationType::Rest),
            ("inlay", OperationType::Inlay),
            ("zigzag", OperationType::Zigzag),
            ("trace", OperationType::Trace),
            ("drill", OperationType::Drill),
            ("chamfer", OperationType::Chamfer),
            ("drop_cutter", OperationType::DropCutter),
            ("adaptive3d", OperationType::Adaptive3d),
            ("waterline", OperationType::Waterline),
            ("pencil", OperationType::Pencil),
            ("scallop", OperationType::Scallop),
            ("steep_shallow", OperationType::SteepShallow),
            ("ramp_finish", OperationType::RampFinish),
            ("spiral_finish", OperationType::SpiralFinish),
            ("radial_finish", OperationType::RadialFinish),
            ("horizontal_finish", OperationType::HorizontalFinish),
            ("project_curve", OperationType::ProjectCurve),
            ("alignment_pin_drill", OperationType::AlignmentPinDrill),
        ];
        assert_eq!(PINNED.len(), OperationType::ALL.len());

        for &(repr, op_type) in PINNED {
            assert_eq!(
                serde_json::to_value(op_type).expect("serialize op type"),
                serde_json::Value::String(repr.to_owned()),
                "{op_type:?}: serde repr drifted from the pinned canonical name"
            );
            assert_eq!(
                op_type.kind_str(),
                repr,
                "{op_type:?}: kind_str() disagrees with the serde repr"
            );
            let parsed: OperationType =
                serde_json::from_value(serde_json::Value::String(repr.to_owned()))
                    .expect("canonical name must deserialize");
            assert_eq!(parsed, op_type);
        }
    }

    #[test]
    fn operation_transform_capabilities_are_explicit() {
        for &op_type in OperationType::ALL {
            let caps = op_type.transform_capabilities();
            assert!(
                !(caps.allows_global_rapid_reorder && caps.requires_depth_order),
                "{op_type:?} cannot globally reorder while requiring depth order"
            );
            assert!(
                !(caps.allows_global_rapid_reorder && caps.continuous_path_required),
                "{op_type:?} cannot globally reorder a continuous path"
            );
        }
        assert!(
            OperationType::Adaptive3d
                .transform_capabilities()
                .requires_depth_order
        );
        assert!(
            OperationType::DropCutter
                .transform_capabilities()
                .allows_global_rapid_reorder
        );
        assert!(
            OperationType::ProjectCurve
                .transform_capabilities()
                .continuous_path_required
        );
    }

    #[test]
    fn air_cut_threshold_suppresses_drill_kinds() {
        assert!(
            OperationType::Drill.air_cut_high_threshold_pct().is_none(),
            "Drill should suppress air-cut metric (dexel can't measure Z-only)"
        );
        assert!(
            OperationType::AlignmentPinDrill
                .air_cut_high_threshold_pct()
                .is_none(),
            "AlignmentPinDrill should suppress air-cut metric"
        );
    }

    #[test]
    fn air_cut_threshold_permissive_for_project_curve() {
        let t = OperationType::ProjectCurve
            .air_cut_high_threshold_pct()
            .expect("ProjectCurve should have an air-cut threshold");
        assert!(
            t >= 95.0,
            "ProjectCurve threshold must accept sparse-pattern baseline (Wanaka rivers read 78–92%); got {t}"
        );
    }

    #[test]
    fn air_cut_threshold_strict_for_finish_ops() {
        for op in [
            OperationType::DropCutter,
            OperationType::Scallop,
            OperationType::Waterline,
            OperationType::Pencil,
            OperationType::HorizontalFinish,
            OperationType::SteepShallow,
            OperationType::RampFinish,
            OperationType::SpiralFinish,
            OperationType::RadialFinish,
        ] {
            let t = op
                .air_cut_high_threshold_pct()
                .unwrap_or_else(|| panic!("{op:?} should have an air-cut threshold"));
            assert!(t <= 30.0, "{op:?} finish threshold expected ≤30, got {t}");
        }
    }

    #[test]
    fn air_cut_threshold_band_for_clearing_ops() {
        for op in [
            OperationType::Adaptive3d,
            OperationType::Adaptive,
            OperationType::Pocket,
            OperationType::Face,
            OperationType::Zigzag,
            OperationType::Rest,
        ] {
            let t = op
                .air_cut_high_threshold_pct()
                .unwrap_or_else(|| panic!("{op:?} should have an air-cut threshold"));
            assert!(
                (35.0..=45.0).contains(&t),
                "{op:?} clearing threshold expected ~40, got {t}"
            );
        }
    }

    #[test]
    fn air_cut_threshold_exhaustive_for_all_ops() {
        // Adding a new OperationType variant should require classifying it.
        // We can't enforce this at compile time (the method returns Option),
        // so this test ensures someone touched every variant deliberately.
        for &op in OperationType::ALL {
            let _ = op.air_cut_high_threshold_pct();
        }
    }

    #[test]
    fn depthless_finishing_ops_resolve_to_none() {
        for op in [
            OperationConfig::Pencil(PencilConfig::default()),
            OperationConfig::Scallop(ScallopConfig::default()),
            OperationConfig::SteepShallow(SteepShallowConfig::default()),
            OperationConfig::RampFinish(RampFinishConfig::default()),
            OperationConfig::SpiralFinish(SpiralFinishConfig::default()),
            OperationConfig::RadialFinish(RadialFinishConfig::default()),
            OperationConfig::HorizontalFinish(HorizontalFinishConfig::default()),
        ] {
            assert!(matches!(op.depth_semantics(), DepthSemantics::None));
            assert_eq!(op.default_depth_for_heights(), 0.0);
        }
    }
}
