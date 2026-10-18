use serde::{Deserialize, Serialize};

use crate::feeds::{OperationFamily as FeedsOperationFamily, PassRole};

use super::config::StockSource;
use super::operation_configs::{
    Adaptive3dConfig, AdaptiveConfig, AlignmentPinDrillConfig, ChamferConfig, DrillConfig,
    DropCutterConfig, FaceConfig, HorizontalFinishConfig, InlayConfig, InlayPlugConfig,
    PencilConfig, PocketConfig, ProfileConfig, ProjectCurveConfig, RadialFinishConfig,
    RampFinishConfig, ScallopConfig, SpiralFinishConfig, SteepShallowConfig, TraceConfig,
    UnifiedFinishConfig, VCarveConfig, WaterlineConfig, ZigzagConfig,
};

mod capabilities;
mod registry;
mod schema;

pub use capabilities::{
    AIR_CUT_BAR_CLEARING_PCT, AIR_CUT_BAR_PROJECT_CURVE_PCT, AIR_CUT_BAR_SURFACE_FINISH_PCT,
    AirCutHint, AxialEnvelope, Capabilities, DocBasis, EntryProbe, EntryProbeRule, ItemUse,
    LutRoute, MoveCountModel, RestUse, RowCapabilities, TRANSFORM_DISCRETE_SCALLOP, TerritoryXy,
};

pub use schema::{
    DressupPolicy, DriverSetField, EntryStylePolicy, Kinematics, OpParams, OpPolicy,
    OpRegistryEntry, OperationParamSchema, OperationSchema, ParamDef, ParamHint, ParamRange,
    ParamTable, ParamType, SkippedField, TOOLPATH_PARAM_DEFS, ToolConstraints, ToolConstraintsDef,
    driver_set_field,
};

use registry::{
    REG_ADAPTIVE, REG_ADAPTIVE3D, REG_ALIGNMENT_PIN_DRILL, REG_CHAMFER, REG_DRILL, REG_DROP_CUTTER,
    REG_FACE, REG_HORIZONTAL_FINISH, REG_INLAY, REG_INLAY_PLUG, REG_PENCIL, REG_POCKET,
    REG_PROFILE, REG_PROJECT_CURVE, REG_RADIAL_FINISH, REG_RAMP_FINISH, REG_SCALLOP,
    REG_SPIRAL_FINISH, REG_STEEP_SHALLOW, REG_TRACE, REG_UNIFIED_FINISH, REG_VCARVE, REG_WATERLINE,
    REG_ZIGZAG,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationFamily {
    TwoPointFiveD,
    ThreeD,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeometryRequirement {
    /// The stock only. No model geometry.
    Stock,
    /// A 2D drawing whose outlines the operation cuts. The generation
    /// prologue refuses an absent or empty outline list. The op's
    /// `Capabilities::item_use` says which outlines it gets: a `Regions` op
    /// gets the closed ones only, a `Curves` op every one (P1).
    Polygons,
    /// A 2D drawing whose drill targets (DXF points, circle and arc
    /// centres) the operation reads, not its outlines (W2.2). A drawing of
    /// points only has no outline, so the prologue does not ask for one;
    /// the drill adapter refuses on its own target count
    /// (`drill_targets_refusal`).
    DrillTargets,
    /// A 3D mesh and its spatial index.
    Mesh,
    /// A 2D drawing and a 3D mesh.
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
    /// The static half of the feeds support declaration
    /// ([`crate::feeds::FeedsSupport`]).
    ///
    /// `Some(source)`: when no vendor row matches, the calculator's formula
    /// may answer, and `source` is its citation. `None`: the operation has
    /// no formula basis, so a cell with no row refuses with
    /// [`crate::feeds::FeedsError::Unbacked`]. Every row is `Some` today
    /// (feeds-matrix Phase 0); ruling R1 decides which rows change.
    pub feeds_formula_source: Option<&'static str>,
}

/// Safety metadata for dressups that can alter topology or move order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OperationTransformCapabilities {
    /// Master permission for either rapid-order transform. Operations may
    /// independently impose the narrower global/depth/continuity constraints
    /// below, but this veto disables all rapid-order permutation.
    pub allows_rapid_reorder: bool,
    /// Cutting segments can be globally reordered by XY proximity without
    /// violating depth/material assumptions.
    pub allows_global_rapid_reorder: bool,
    /// The emitted toolpath must maintain its depth-pass ordering.
    pub requires_depth_order: bool,
    /// The operation encodes a continuous trace/finish path where reordering
    /// or stay-down linking can change the intended cut.
    pub continuous_path_required: bool,
    /// Whether `apply_link_moves` may bridge two consecutive fragment
    /// endpoints with a straight feed move.
    ///
    /// This is a GEOMETRIC question — is a straight feed between two
    /// consecutive fragment endpoints clear of material? — not a structural
    /// one, which is why it cannot be derived from `requires_depth_order` /
    /// `continuous_path_required` the way the reorder predicates can. Those
    /// two fields describe move-ORDER safety (can segments be visited in a
    /// different sequence); this field describes move-PATH safety (can two
    /// endpoints be bridged with a straight cutting move). The two can and
    /// do diverge: discrete-ring Scallop (`ScallopConfig::continuous ==
    /// false`) can be safely globally reordered — holding link_moves
    /// constant and varying only `optimize_rapid_order`, measured cutting
    /// distance was byte-identical (2437.8mm → 2437.8mm, only rapids
    /// changed) while rapid travel dropped 27% — but it cannot safely take
    /// link moves: `apply_link_moves` collapses a retract→rapid→plunge
    /// triple into a straight feed bridge, which on a 3D surface plows
    /// laterally through material — measured 9.7mm of over-cut. See
    /// `crates/rs_cam_core/tests/capability_link_moves_safety.rs`.
    ///
    /// Forward pointer: the principled long-term fix is for
    /// `apply_link_moves` to gouge-check candidate bridges the way
    /// `surface_link::build_surface_link` already does for
    /// pencil/unified_finish, rather than trusting a static per-op boolean.
    /// This field is the interim guard, not the end state.
    pub allows_link_moves: bool,
    /// R10 (operator ruling 2026-09-18): the most laps a ramp entry may
    /// fold over the run it enters on, or `None` for no cap. Set from the
    /// operation's process role by [`OperationType::ramp_fold_lap_cap`]:
    /// the finishing roles get [`crate::dressup::RAMP_FOLD_MAX_LAPS`], a
    /// rough keeps folding. `new` leaves it `None`.
    pub ramp_fold_lap_cap: Option<u32>,
    /// G-PLANSIMGAP (2026-09-26): the planner applies the segment merge to
    /// each cut before it stamps the cut into its own stock
    /// (`Adaptive3dGeometry::segment_merge_tolerance`), so the dressup
    /// pipeline does not merge again. A merge after the planner moves cuts
    /// the planner stock already holds. `new` leaves it `false`.
    ///
    /// 2D Adaptive sets it too (G-ADAPTPASSLOAD, 2026-09-26): its planner
    /// emits the path it walked (it drops only collinear points) and holds
    /// every step to the pass load on its own stock, so a 0.3 mm merge
    /// after it moves cuts off that stock (the six-island mop read 0.65
    /// where a merged run crossed a sliver the planner had left).
    pub planner_applies_segment_merge: bool,
    /// G-ADAPTPASSLOAD (2026-09-26): a helix entry ends with one full lap
    /// at the target Z, so the hole it leaves has a flat floor of radius
    /// `R + helix radius`. Without it the last turn leaves a helical step
    /// up to one pitch high across the ring, and the first cut out of the
    /// hole reads that step as fresh stock on both sides (sim radial 0.58
    /// on the six-island starter). The 2D Adaptive planner stamps the hole
    /// as cut, so its entries need the lap. `new` leaves it `false`.
    pub helix_floor_lap: bool,
    /// G-ADAPTPASSLOAD round 4 (2026-09-27): with entry style None, a
    /// plunge goes down through air by rapid to the clearance over the
    /// op's own replayed stock (the helix's first half) and feeds only the
    /// rest (`dressup::EntryStyle::Plunge`). The 2D Adaptive planner
    /// models a plunge as its own disc, which the XY-unchanged plunge
    /// keeps. `new` leaves it `false`.
    pub stock_aware_plunge: bool,
}

impl OperationTransformCapabilities {
    pub const fn new(
        allows_global_rapid_reorder: bool,
        requires_depth_order: bool,
        continuous_path_required: bool,
        allows_link_moves: bool,
    ) -> Self {
        Self {
            allows_rapid_reorder: true,
            allows_global_rapid_reorder,
            requires_depth_order,
            continuous_path_required,
            allows_link_moves,
            ramp_fold_lap_cap: None,
            planner_applies_segment_merge: false,
            helix_floor_lap: false,
            stock_aware_plunge: false,
        }
    }

    /// With entry style None a plunge rapids through air first
    /// (G-ADAPTPASSLOAD round 4).
    pub const fn with_stock_aware_plunge(self) -> Self {
        Self {
            stock_aware_plunge: true,
            ..self
        }
    }

    /// A helix entry ends with a flat lap at the target Z (G-ADAPTPASSLOAD).
    pub const fn with_helix_floor_lap(self) -> Self {
        Self {
            helix_floor_lap: true,
            ..self
        }
    }

    /// The planner applies the segment merge itself (G-PLANSIMGAP).
    pub const fn with_planner_segment_merge(self) -> Self {
        Self {
            planner_applies_segment_merge: true,
            ..self
        }
    }

    /// Disable both barriered and unbarriered rapid-order permutation.
    pub const fn without_rapid_reorder(self) -> Self {
        Self {
            allows_rapid_reorder: false,
            ..self
        }
    }

    /// The same capabilities with the R10 lap cap set.
    pub const fn with_ramp_fold_lap_cap(self, cap: Option<u32>) -> Self {
        Self {
            ramp_fold_lap_cap: cap,
            ..self
        }
    }

    pub fn allows_barriered_rapid_reorder(self) -> bool {
        self.allows_rapid_reorder && !self.continuous_path_required
    }

    pub fn allows_unbarriered_rapid_reorder(self) -> bool {
        self.allows_rapid_reorder
            && self.allows_global_rapid_reorder
            && !self.requires_depth_order
            && !self.continuous_path_required
    }

    pub fn allows_link_moves(self) -> bool {
        self.allows_link_moves
    }
}

// ── Phase 2 operation X-macro (architectural refactor 2026-06-06) ─────
//
// THE single authoritative operation list. Every row carries
// `(Variant, ConfigType, Category)`; the callback macros below generate
// the pure-list surfaces (`OperationType` decl, `ALL`, `category()`,
// and the `OperationConfig` dispatch in the next section). Adding an
// operation = adding ONE row here (plus its registry entry and config
// struct — both compile-enforced).
//
// Deliberately NOT generated (plan §3.4): `spec()` bodies, `ParamDef`
// arrays, `OperationConfig` itself (serde-attribute regression risk),
// and execute.rs arms. `ALL_2D`/`ALL_3D` stay hand-written below and
// are sync-tested against the category tokens.
//
// Category tokens are `OpCategory` variant names: `Menu2d` / `Menu3d` /
// `SystemOnly`. A typo'd token fails with E0599 pointing at the row.
//
// The fourth column is the snake_case kind token. It is BOTH the serde
// representation (`#[serde(rename_all = "snake_case")]` produces it) and
// [`OperationType::kind_str`], which is generated from it.
// `operation_type_serde_repr_pinned` guards that the two still agree.
macro_rules! for_each_op {
    ($m:ident) => {
        $m! {
            //  variant            config type                category     serde/kind token
            (Face,               FaceConfig,                Menu2d,      "face"),
            (Pocket,             PocketConfig,              Menu2d,      "pocket"),
            (Profile,            ProfileConfig,             Menu2d,      "profile"),
            (Adaptive,           AdaptiveConfig,            Menu2d,      "adaptive"),
            (VCarve,             VCarveConfig,              Menu2d,      "v_carve"),
            (Inlay,              InlayConfig,               Menu2d,      "inlay"),
            (InlayPlug,          InlayPlugConfig,           Menu2d,      "inlay_plug"),
            (Zigzag,             ZigzagConfig,              Menu2d,      "zigzag"),
            (Trace,              TraceConfig,               Menu2d,      "trace"),
            (Drill,              DrillConfig,               Menu2d,      "drill"),
            (Chamfer,            ChamferConfig,             Menu2d,      "chamfer"),
            (DropCutter,         DropCutterConfig,          Menu3d,      "drop_cutter"),
            (Adaptive3d,         Adaptive3dConfig,          Menu3d,      "adaptive3d"),
            (Waterline,          WaterlineConfig,           Menu3d,      "waterline"),
            (Pencil,             PencilConfig,              Menu3d,      "pencil"),
            (Scallop,            ScallopConfig,             Menu3d,      "scallop"),
            (UnifiedFinish,      UnifiedFinishConfig,       Menu3d,      "unified_finish"),
            (SteepShallow,       SteepShallowConfig,        Menu3d,      "steep_shallow"),
            (RampFinish,         RampFinishConfig,          Menu3d,      "ramp_finish"),
            (SpiralFinish,       SpiralFinishConfig,        Menu3d,      "spiral_finish"),
            (RadialFinish,       RadialFinishConfig,        Menu3d,      "radial_finish"),
            (HorizontalFinish,   HorizontalFinishConfig,    Menu3d,      "horizontal_finish"),
            (ProjectCurve,       ProjectCurveConfig,        Menu3d,      "project_curve"),
            // Auto-generated drilling operation for stock alignment pin
            // holes — in `ALL`, in neither user menu.
            (AlignmentPinDrill,  AlignmentPinDrillConfig,   SystemOnly,  "alignment_pin_drill"),
        }
    };
}

/// Menu placement category, generated per op from the X-macro row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpCategory {
    /// Listed in the 2D operations menu (`OperationType::ALL_2D`).
    Menu2d,
    /// Listed in the 3D operations menu (`OperationType::ALL_3D`).
    Menu3d,
    /// In `OperationType::ALL` but in neither user menu (system-generated).
    SystemOnly,
}

macro_rules! define_operation_type {
    ($( ($variant:ident, $config:ident, $cat:ident, $kind:literal) ),+ $(,)?) => {
        /// Operation type for creating new toolpaths.
        ///
        /// GENERATED from the `for_each_op!` list — edit the list, not
        /// this block.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum OperationType {
            $($variant,)+
        }

        impl OperationType {
            /// Every operation, in canonical (list) order. GENERATED.
            pub const ALL: &[OperationType] = &[$(OperationType::$variant,)+];

            /// Menu placement for this op, from the X-macro category
            /// token. `ALL_2D`/`ALL_3D` stay hand-written and are
            /// sync-tested against this in
            /// `operation_partitions_cover_all_variants_once`.
            pub const fn category(self) -> OpCategory {
                match self {
                    $(OperationType::$variant => OpCategory::$cat,)+
                }
            }

            /// The variant's own name, for diagnostics that must name the
            /// operation that refused a field. GENERATED, so it cannot
            /// fall out of step with the operation list. Not a UI label —
            /// it is the Rust variant spelling.
            pub const fn name(self) -> &'static str {
                match self {
                    $(OperationType::$variant => stringify!($variant),)+
                }
            }

            /// Stable snake_case identifier for serialized / diagnostic
            /// use. GENERATED from the X-macro's fourth column, which is
            /// also what `#[serde(rename_all = "snake_case")]` emits for
            /// the variant — `operation_type_serde_repr_pinned` is the
            /// guard that the two stay one token.
            ///
            /// Unlike [`Self::label`] (human-facing UI text) this is
            /// suitable for JSON wire formats and for consumers that
            /// branch on op kind without parsing the prose label.
            pub const fn kind_str(self) -> &'static str {
                match self {
                    $(OperationType::$variant => $kind,)+
                }
            }
        }
    };
}
for_each_op!(define_operation_type);

macro_rules! define_operation_config_dispatch {
    ($( ($variant:ident, $config:ident, $cat:ident, $kind:literal) ),+ $(,)?) => {
        /// Per-variant dispatch surfaces. GENERATED from `for_each_op!`
        /// — edit the list, not this block.
        impl OperationConfig {
            pub fn op_type(&self) -> OperationType {
                match self {
                    $(OperationConfig::$variant(_) => OperationType::$variant,)+
                }
            }

            pub fn new_default(op_type: OperationType) -> Self {
                match op_type {
                    $(OperationType::$variant =>
                        OperationConfig::$variant(<$config>::default()),)+
                }
            }

            pub fn as_params(&self) -> &dyn OperationParams {
                match self {
                    $(OperationConfig::$variant(c) => c,)+
                }
            }

            pub fn as_params_mut(&mut self) -> &mut dyn OperationParams {
                match self {
                    $(OperationConfig::$variant(c) => c,)+
                }
            }
        }
    };
}
for_each_op!(define_operation_config_dispatch);

impl OperationType {
    pub const ALL_2D: &[OperationType] = &[
        OperationType::Face,
        OperationType::Pocket,
        OperationType::Profile,
        OperationType::Adaptive,
        OperationType::VCarve,
        OperationType::Inlay,
        OperationType::InlayPlug,
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
        OperationType::UnifiedFinish,
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

    /// W2.2: the registry's refusal of a tool of this kind, or `None` when
    /// the row's `tool_constraints` admit it. The generation prologue, the
    /// static diagnostic and the GUI validation all read this one text.
    pub fn tool_refusal(self, kind: crate::feeds::CutterKind) -> Option<String> {
        self.registry_entry()
            .tool_constraints
            .refusal(self.spec().label, kind)
    }

    /// The geometry the operation reads, from its registry row.
    pub fn geometry_requirement(self) -> GeometryRequirement {
        self.spec().geometry
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
            OperationType::Inlay => &REG_INLAY,
            OperationType::InlayPlug => &REG_INLAY_PLUG,
            OperationType::Zigzag => &REG_ZIGZAG,
            OperationType::Trace => &REG_TRACE,
            OperationType::Drill => &REG_DRILL,
            OperationType::Chamfer => &REG_CHAMFER,
            OperationType::DropCutter => &REG_DROP_CUTTER,
            OperationType::Adaptive3d => &REG_ADAPTIVE3D,
            OperationType::Waterline => &REG_WATERLINE,
            OperationType::Pencil => &REG_PENCIL,
            OperationType::Scallop => &REG_SCALLOP,
            OperationType::UnifiedFinish => &REG_UNIFIED_FINISH,
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

    /// Does a per-tool reach map ([`crate::maps::reach_map`]) say anything about
    /// this operation? P5, 2026-09-08.
    ///
    /// **Derived from the registry, not a name list**: the operation must
    /// read a 3D mesh ([`GeometryRequirement::Mesh`]) and must not be a
    /// roughing pass ([`UiProcessRole::Roughing`]). A reach map answers
    /// "which of this surface can this cutter form", which is a finishing
    /// question — a rough leaves stock everywhere on purpose.
    ///
    /// On the registry as it stands that yields exactly ten operations:
    /// `drop_cutter`, `waterline`, `pencil`, `scallop`, `unified_finish`,
    /// `steep_shallow`, `ramp_finish`, `spiral_finish`, `radial_finish` and
    /// `horizontal_finish`. `waterline` is in because it rides the same
    /// surface, even though its registry role is
    /// [`UiProcessRole::SemiFinish`]; `adaptive3d` is out because it is
    /// roughing; `project_curve` is out because its geometry requirement is
    /// [`GeometryRequirement::Both`] and its reach question is about a
    /// curve, not a surface.
    ///
    /// There is deliberately **no tool precondition.** The reach walk is a
    /// min-filter with the cutter's own
    /// [`crate::tool::MillingCutter::height_at_radius`] profile, which is
    /// exact for every shipped shape — ball, tapered ball, flat, bull nose
    /// and V-bit alike — so a shape gate would only refuse answers it can
    /// give.
    #[must_use]
    pub fn supports_reach_map(self) -> bool {
        let spec = self.spec();
        matches!(spec.geometry, GeometryRequirement::Mesh)
            && !matches!(spec.ui_process_role, UiProcessRole::Roughing)
    }

    /// Is this operation's `stepover()` a **lateral pass spacing over the
    /// surface** — the distance that leaves a cusp between neighbouring
    /// passes of a spherical tip?
    ///
    /// Only true where the closed form
    /// `cusp = R − sqrt(R² − (stepover/2)²)` describes what the operator
    /// will actually feel on the part, because
    /// [`crate::session::ProjectSession::reach_tolerance_for`] uses it as the
    /// reach map's bar for an op that declares no scallop height of its own
    /// (F2, 2026-09-08). The default 0.05 mm bar is a **finish-quality
    /// guess**; the raster's own cusp is a measurement of the same surface,
    /// and on the operator's wanaka case the two were 0.146 against 0.050 —
    /// so the map was judging a 1.5 mm raster against a bar three times
    /// finer than the pass spacing could ever deliver.
    ///
    /// Declared one arm at a time rather than derived from "has a
    /// `stepover()`", because four of the ten reach-map operations carry a
    /// `stepover()` that is NOT a surface raster spacing:
    ///
    /// * `Waterline` — its step is `z_step`, a VERTICAL drop between
    ///   contours, and it publishes no `stepover()` at all.
    /// * `Pencil` — `offset_stepover` is the spacing of an offset fan either
    ///   side of one valley seam, and only exists when
    ///   `num_offset_passes > 1`. A pencil pass does not cover the surface,
    ///   so it has no raster cusp.
    /// * `RadialFinish` — spokes at an `angular_step`, so the spacing varies
    ///   from zero at the hub to a maximum at the rim. One cusp number would
    ///   be wrong nearly everywhere.
    /// * `RampFinish` — publishes no `stepover()`.
    ///
    /// `Scallop` and `UnifiedFinish` are absent for the opposite reason:
    /// they declare a `scallop_height()`, which is consulted first.
    ///
    /// CMP-07: the membership list moved onto the registry row
    /// (`OpPolicy::raster_stepover_is_lateral`). As a `matches!` it failed
    /// OPEN — a 25th operation joined the false side without a compiler
    /// word. The set is pinned by `lateral_raster_stepover_set_is_pinned`.
    #[must_use]
    pub fn lateral_raster_stepover(self) -> bool {
        self.registry_entry().policy.raster_stepover_is_lateral
    }

    /// True for op kinds whose kinematics are Z-only (peck-plunge drilling).
    /// Used by the verdict layer to suppress rapid:cut-ratio and engagement
    /// signals that don't apply to drilling — see fix-plan §1 A12.
    ///
    /// THE canonical drill-family predicate (Phase 1 T4 consolidated the
    /// open-coded `Drill | AlignmentPinDrill` matches in the chipload /
    /// power / deflection gates and the narrate
    /// `is_drill_cycle` flag onto this). The membership set is pinned by
    /// `drill_kinematics_set_is_pinned`; extend it deliberately.
    ///
    /// CMP-07: the class is a registry row field (`OpPolicy::kinematics`)
    /// and no longer a `matches!` list that fails open.
    pub fn is_drill_kinematics(self) -> bool {
        self.registry_entry().policy.kinematics == Kinematics::DrillZOnly
    }

    /// R10: the ramp-fold lap cap for this operation's process role. The
    /// finishing roles (`Finish`, `SemiFinish`) get
    /// [`crate::dressup::RAMP_FOLD_MAX_LAPS`]; a rough gets `None` and
    /// keeps folding (operator ruling 2026-09-18: a rough that laps a
    /// short run cuts material that has to go, and the alternative is a
    /// flat end mill plunging into fresh stock).
    pub fn ramp_fold_lap_cap(self) -> Option<u32> {
        match self.spec().ui_process_role {
            UiProcessRole::Finish | UiProcessRole::SemiFinish => {
                Some(crate::dressup::RAMP_FOLD_MAX_LAPS)
            }
            UiProcessRole::Roughing => None,
        }
    }

    /// W2.4: the op kind's capabilities, from its registry row. The row's
    /// transform gets the R10 lap cap of the process role here. A caller
    /// with an [`OperationConfig`] in hand calls
    /// [`OperationConfig::capabilities`], which also reads the config.
    pub fn capabilities(self) -> RowCapabilities {
        let row = self.registry_entry().capabilities;
        Capabilities {
            transform: row
                .transform
                .with_ramp_fold_lap_cap(self.ramp_fold_lap_cap()),
            ..row
        }
    }
}

/// Common parameter accessors for all operation configs.
///
/// Implemented by each config struct to eliminate per-variant match arms.
/// Optional fields (stepover, depth_per_pass) return None by default.
///
/// The two optional setters report whether they wrote. A config that
/// carries the field overrides the setter and returns `true`; a config
/// that has no such field keeps the default body, writes nothing and
/// returns `false`. `ProjectSession::set_toolpath_param` refuses on
/// `false` instead of reporting a success it did not deliver (N5).
pub trait OperationParams {
    fn feed_rate(&self) -> f64;
    fn set_feed_rate(&mut self, value: f64);

    /// Returns the plunge rate. Drill operations return feed_rate since they're purely vertical.
    fn plunge_rate(&self) -> f64;
    fn set_plunge_rate(&mut self, value: f64);

    /// Feed (mm/min) of a helix or ramp entry through material, when the
    /// operator set one. `None` uses the plunge feed.
    fn ramp_feed_rate(&self) -> Option<f64> {
        None
    }
    /// Write the entry ramp feed. Returns `false` when this config has no
    /// such field (a drill cycle has no entry).
    fn set_ramp_feed_rate(&mut self, _value: Option<f64>) -> bool {
        false
    }

    fn stepover(&self) -> Option<f64> {
        None
    }
    /// Write the stepover. Returns `false` when this config has no such
    /// field, so the caller can refuse instead of discarding the value.
    fn set_stepover(&mut self, _value: f64) -> bool {
        false
    }

    fn depth_per_pass(&self) -> Option<f64> {
        None
    }

    /// TOTAL depth of the cut (mm), when the operation carries one.
    ///
    /// Distinct from [`Self::depth_per_pass`], which is the per-pass step.
    /// The pair is what makes the realised depth a staircase: generation
    /// cuts `total / ceil(total / per_pass)` — see
    /// [`crate::ops::depth::realised_step_down`].
    ///
    /// `None` for operations whose depth comes from the model surface rather
    /// than from a parameter (Adaptive3d, Waterline, RampFinish) and for
    /// every operation with no depth at all. Absence is the honest answer
    /// there: those cuts have no total to divide.
    fn total_depth(&self) -> Option<f64> {
        None
    }
    /// Write the depth per pass. Returns `false` when this config has no
    /// such field, so the caller can refuse instead of discarding the
    /// value.
    fn set_depth_per_pass(&mut self, _value: f64) -> bool {
        false
    }

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
    Inlay(InlayConfig),
    InlayPlug(InlayPlugConfig),
    Zigzag(ZigzagConfig),
    Trace(TraceConfig),
    Drill(DrillConfig),
    Chamfer(ChamferConfig),
    DropCutter(DropCutterConfig),
    Adaptive3d(Adaptive3dConfig),
    Waterline(WaterlineConfig),
    Pencil(PencilConfig),
    Scallop(ScallopConfig),
    UnifiedFinish(UnifiedFinishConfig),
    SteepShallow(SteepShallowConfig),
    RampFinish(RampFinishConfig),
    SpiralFinish(SpiralFinishConfig),
    RadialFinish(RadialFinishConfig),
    HorizontalFinish(HorizontalFinishConfig),
    ProjectCurve(ProjectCurveConfig),
    AlignmentPinDrill(AlignmentPinDrillConfig),
}

impl OperationConfig {
    /// W2.4: this operation's capabilities. The registry row
    /// ([`OperationType::capabilities`]) is the default; an arm overrides
    /// only a field that reads this config. The match names every variant,
    /// so a new operation states its overrides, even when it has none.
    pub fn capabilities(&self) -> Capabilities {
        let row = self
            .op_type()
            .capabilities()
            .with_entry_probe(self.entry_probe());
        match self {
            // Discrete-ring Scallop takes global reorder and no links; see
            // `TRANSFORM_DISCRETE_SCALLOP`. Continuous Scallop keeps the row.
            OperationConfig::Scallop(c) => Capabilities {
                transform: if c.continuous {
                    row.transform
                } else {
                    TRANSFORM_DISCRETE_SCALLOP
                },
                feeds_hints: FeedsHints {
                    target_scallop_mm: Some(c.scallop_height),
                    ..FeedsHints::NONE
                },
                ..row
            },
            OperationConfig::UnifiedFinish(c) => Capabilities {
                feeds_hints: FeedsHints {
                    target_scallop_mm: Some(c.scallop_height),
                    ..FeedsHints::NONE
                },
                ..row
            },
            // DropCutter (the "3D Finish" raster) can derive its stepover
            // from a scallop target the way Scallop does. `None` keeps the
            // `ae_factor x diameter` stepover.
            OperationConfig::DropCutter(c) => Capabilities {
                feeds_hints: FeedsHints {
                    target_scallop_mm: c.scallop_height,
                    ..FeedsHints::NONE
                },
                ..row
            },
            OperationConfig::Waterline(c) => Capabilities {
                feeds_hints: FeedsHints {
                    axial_depth_mm: Some(c.z_step),
                    ..FeedsHints::NONE
                },
                ..row
            },
            OperationConfig::SteepShallow(c) => Capabilities {
                feeds_hints: FeedsHints {
                    axial_depth_mm: Some(c.z_step),
                    ..FeedsHints::NONE
                },
                ..row
            },
            OperationConfig::VCarve(c) => Capabilities {
                feeds_hints: FeedsHints {
                    axial_depth_mm: Some(c.max_depth),
                    ..FeedsHints::NONE
                },
                ..row
            },
            OperationConfig::RampFinish(c) => Capabilities {
                feeds_hints: FeedsHints {
                    axial_depth_mm: Some(c.max_stepdown),
                    ..FeedsHints::NONE
                },
                ..row
            },
            // C2: a contour-spiral planner holds the engagement of its
            // stepover; the other strategies have real engagement spikes.
            OperationConfig::Adaptive3d(c) => Capabilities {
                planner_uniform_engagement: matches!(
                    c.clearing_strategy,
                    super::operation_configs::ClearingStrategy::ContourSpiral
                ),
                ..row
            },
            OperationConfig::Adaptive(c) => Capabilities {
                planner_uniform_engagement: matches!(
                    c.path_strategy,
                    crate::adaptive::PathStrategy2d::ContourSpiral
                ),
                ..row
            },
            // P1: the compensation decides how Trace cuts its items. On
            // the line it follows a closed or an open path; an offset to
            // one side needs a closed loop (the open-offset kernel is P6).
            OperationConfig::Trace(c) => Capabilities {
                item_use: match c.compensation {
                    crate::ops::trace_path::TraceCompensation::None => ItemUse::Curves,
                    crate::ops::trace_path::TraceCompensation::Left
                    | crate::ops::trace_path::TraceCompensation::Right => ItemUse::Regions,
                },
                ..row
            },
            // No field of these configs changes a capability beyond the
            // entry probe, which `entry_probe` resolved above.
            OperationConfig::Pencil(_)
            | OperationConfig::SpiralFinish(_)
            | OperationConfig::RadialFinish(_)
            | OperationConfig::HorizontalFinish(_)
            | OperationConfig::Face(_)
            | OperationConfig::Pocket(_)
            | OperationConfig::Profile(_)
            | OperationConfig::Inlay(_)
            | OperationConfig::InlayPlug(_)
            | OperationConfig::Zigzag(_)
            | OperationConfig::Drill(_)
            | OperationConfig::Chamfer(_)
            | OperationConfig::ProjectCurve(_)
            | OperationConfig::AlignmentPinDrill(_) => row,
        }
    }

    /// The row's [`EntryProbeRule`], resolved against this config.
    fn entry_probe(&self) -> EntryProbe {
        match self.op_type().capabilities().entry_probe {
            EntryProbeRule::Prism => EntryProbe::Prism,
            EntryProbeRule::SurfaceFixed { leave_mm } => EntryProbe::Surface { leave_mm },
            EntryProbeRule::SurfaceFromConfig => match self.stock_to_leave_dial() {
                Some(leave_mm) => EntryProbe::Surface { leave_mm },
                // Not reached: `every_op_states_its_capabilities_w2_4`
                // proves that every `SurfaceFromConfig` row has a dial. The
                // CL surface is the side that cannot gouge the part.
                None => EntryProbe::Surface { leave_mm: 0.0 },
            },
        }
    }

    /// W6.2: the surface cell mask this config states, or `None` when the
    /// op masks no cell by slope. The match names every variant, so a new
    /// op states whether it masks. `compute::execute::Territory` reads it.
    pub fn surface_mask(&self) -> Option<crate::compute::execute::SurfaceMask> {
        use crate::compute::execute::SurfaceMask;
        let band = |from_deg: f64, to_deg: f64| {
            crate::finish::finish_setup::slope_filter_active(from_deg, to_deg)
                .then_some(SurfaceMask::SlopeBand { from_deg, to_deg })
        };
        match self {
            OperationConfig::DropCutter(c) => band(c.slope_from, c.slope_to),
            OperationConfig::Scallop(c) => band(c.slope_from, c.slope_to),
            OperationConfig::RampFinish(c) => band(c.slope_from, c.slope_to),
            OperationConfig::SteepShallow(c) => Some(SurfaceMask::SteepShallowSplit {
                threshold_deg: c.threshold_angle,
            }),
            OperationConfig::HorizontalFinish(c) => Some(SurfaceMask::Flat {
                max_slope_deg: c.angle_threshold,
            }),
            OperationConfig::UnifiedFinish(c) => Some(SurfaceMask::Bands {
                steep_threshold_deg: c.steep_threshold_deg,
                waterline_threshold_deg: c.waterline_threshold_deg,
            }),
            OperationConfig::Face(_)
            | OperationConfig::Pocket(_)
            | OperationConfig::Profile(_)
            | OperationConfig::Adaptive(_)
            | OperationConfig::VCarve(_)
            | OperationConfig::Inlay(_)
            | OperationConfig::InlayPlug(_)
            | OperationConfig::Zigzag(_)
            | OperationConfig::Trace(_)
            | OperationConfig::Drill(_)
            | OperationConfig::Chamfer(_)
            | OperationConfig::Adaptive3d(_)
            | OperationConfig::Waterline(_)
            | OperationConfig::Pencil(_)
            | OperationConfig::SpiralFinish(_)
            | OperationConfig::RadialFinish(_)
            | OperationConfig::ProjectCurve(_)
            | OperationConfig::AlignmentPinDrill(_) => None,
        }
    }

    /// The config's `stock_to_leave` dial, or `None` when the op has none.
    /// The match names every variant, so a new op states whether it has
    /// the dial.
    pub fn stock_to_leave_dial(&self) -> Option<f64> {
        match self {
            OperationConfig::Pencil(c) => Some(c.stock_to_leave),
            OperationConfig::Scallop(c) => Some(c.stock_to_leave),
            OperationConfig::UnifiedFinish(c) => Some(c.stock_to_leave),
            OperationConfig::SteepShallow(c) => Some(c.stock_to_leave),
            OperationConfig::RampFinish(c) => Some(c.stock_to_leave),
            OperationConfig::SpiralFinish(c) => Some(c.stock_to_leave),
            OperationConfig::RadialFinish(c) => Some(c.stock_to_leave),
            OperationConfig::HorizontalFinish(c) => Some(c.stock_to_leave),
            OperationConfig::Face(_)
            | OperationConfig::Pocket(_)
            | OperationConfig::Profile(_)
            | OperationConfig::Adaptive(_)
            | OperationConfig::VCarve(_)
            | OperationConfig::Inlay(_)
            | OperationConfig::InlayPlug(_)
            | OperationConfig::Zigzag(_)
            | OperationConfig::Trace(_)
            | OperationConfig::Drill(_)
            | OperationConfig::Chamfer(_)
            | OperationConfig::DropCutter(_)
            | OperationConfig::Adaptive3d(_)
            | OperationConfig::Waterline(_)
            | OperationConfig::ProjectCurve(_)
            | OperationConfig::AlignmentPinDrill(_) => None,
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

    pub fn feeds_style(&self) -> (FeedsOperationFamily, PassRole) {
        let spec = self.spec();
        (spec.feeds_family, spec.feeds_pass_role)
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

    /// The helix and ramp entry feed through material; see
    /// [`OperationParams::ramp_feed_rate`].
    pub fn ramp_feed_rate(&self) -> Option<f64> {
        self.as_params().ramp_feed_rate()
    }

    /// The feed a helix or ramp entry runs at: the ramp feed, held at or
    /// below the cut feed (G-RAMPCLAMP). `None` uses the plunge feed.
    ///
    /// Suggest writes a ramp feed at or below the feed it ships, but an
    /// operator can lower the feed afterwards; the entry follows it down.
    /// The clamp sits here, where the operation's feed is in scope, not in
    /// the entry emitter, whose `feed_rate` is the dressup caller's feed
    /// (G10 F3).
    pub fn entry_feed_rate(&self) -> Option<f64> {
        self.ramp_feed_rate().map(|r| r.min(self.feed_rate()))
    }

    pub fn stepover(&self) -> Option<f64> {
        self.as_params().stepover()
    }

    /// Write the stepover. Returns `false` when this operation carries no
    /// stepover field and nothing was written (N5).
    pub fn set_stepover(&mut self, value: f64) -> bool {
        self.as_params_mut().set_stepover(value)
    }

    pub fn depth_per_pass(&self) -> Option<f64> {
        self.as_params().depth_per_pass()
    }

    pub fn total_depth(&self) -> Option<f64> {
        self.as_params().total_depth()
    }

    /// Write the depth per pass. Returns `false` when this operation
    /// carries no such field and nothing was written (N5).
    pub fn set_depth_per_pass(&mut self, value: f64) -> bool {
        self.as_params_mut().set_depth_per_pass(value)
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
    ///
    /// A list field (type `vec<…>`) that serde skips when it is empty reads
    /// back as `[]`, not `null`. `null` is not a value of a list, so an
    /// agent that sent the read value back got a refusal.
    pub fn params_value_including_nulls(&self) -> serde_json::Value {
        let mut value = serde_json::to_value(self).unwrap_or_else(|_| serde_json::json!({}));
        let mut params = value
            .get_mut("params")
            .and_then(|v| v.as_object_mut())
            .cloned()
            .unwrap_or_default();
        for def in param_defs_for_type(self.op_type()) {
            let absent = if def.type_name.starts_with("vec<") {
                serde_json::Value::Array(Vec::new())
            } else {
                serde_json::Value::Null
            };
            params.entry(def.name.to_owned()).or_insert(absent);
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
    ///
    /// An alias resolves to the def it names (CMP-08).
    pub fn param_type_name(&self, param: &str) -> Option<&'static str> {
        param_def_for_type(self.op_type(), param).map(|def| def.type_name)
    }

    /// Return the declared numeric domain for one settable parameter, if
    /// the registry declares one. See [`ParamRange`] for what "declares
    /// one" is worth — most params carry `None`, which means **no domain
    /// has been stated**, not "any float is fine".
    pub fn param_range(&self, param: &str) -> Option<ParamRange> {
        Self::param_range_for_type(self.op_type(), param)
    }

    /// Type-level sibling of [`Self::param_range`], for callers holding an
    /// [`OperationType`] rather than a config.
    pub fn param_range_for_type(op_type: OperationType, param: &str) -> Option<ParamRange> {
        param_def_for_type(op_type, param).and_then(|def| def.range)
    }

    /// Schema-backed settable param names for this operation.
    pub fn param_names(&self) -> Vec<&'static str> {
        Self::param_names_for_type(self.op_type())
    }

    /// Schema-backed settable param names for an operation type.
    ///
    /// Every def's own name. The refusal message of `set_toolpath_param`
    /// prints this list. A def has one name only (D3 deleted the alias
    /// field), so the setter accepts exactly this list.
    pub fn param_names_for_type(op_type: OperationType) -> Vec<&'static str> {
        param_defs_for_type(op_type)
            .iter()
            .map(|def| def.name)
            .collect()
    }

    /// The names `set_toolpath_param` accepts that are not operation
    /// parameters. The same for every operation — see
    /// [`crate::compute::catalog::TOOLPATH_PARAM_DEFS`].
    pub fn toolpath_param_names() -> Vec<&'static str> {
        TOOLPATH_PARAM_DEFS.iter().map(|def| def.name).collect()
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
                range: def.range.map(ParamRange::to_json),
                description: def.help.map(str::to_owned),
            })
            .collect();
        OperationSchema {
            operation_type: op_type.kind_str().to_owned(),
            label: op_type.label().to_owned(),
            params,
            toolpath_params: TOOLPATH_PARAM_DEFS
                .iter()
                .map(|def| OperationParamSchema {
                    name: def.name.to_owned(),
                    type_name: def.type_name.to_owned(),
                    optional: def.optional,
                    default: serde_json::Value::Null,
                    range: def.range.map(ParamRange::to_json),
                    description: def.help.map(str::to_owned),
                })
                .collect(),
            tool_constraints: tool_constraints_for_type(op_type),
        }
    }
}

/// The def one settable NAME resolves to: the def with that name.
fn param_def_for_type(op_type: OperationType, param: &str) -> Option<&'static ParamDef> {
    param_defs_for_type(op_type)
        .iter()
        .find(|def| def.name == param)
}

fn param_defs_for_type(op_type: OperationType) -> &'static [ParamDef] {
    op_type.registry_entry().params.defs
}

fn tool_constraints_for_type(op_type: OperationType) -> ToolConstraints {
    op_type.registry_entry().tool_constraints.to_schema()
}

/// Operation-specific hints handed to the feeds calculator. Hints are
/// config values (z-step, scallop target, …), so the registry row holds
/// [`FeedsHints::NONE`] and [`OperationConfig::capabilities`] fills
/// [`Capabilities::feeds_hints`] from the config (W2.4).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct FeedsHints {
    /// Operation-imposed axial depth (mm) the calculator must respect
    /// (e.g. a waterline z-step) instead of choosing its own DOC.
    pub axial_depth_mm: Option<f64>,
    /// Operation-imposed radial width (mm); none of the current ops set
    /// this, but the slot is part of the calculator contract.
    pub radial_width_mm: Option<f64>,
    /// Scallop-height target (mm) for stepover-from-chord-geometry ops.
    pub target_scallop_mm: Option<f64>,
}

impl FeedsHints {
    /// Named "no hints" policy — the calculator falls back to LUT /
    /// diameter-factor defaults. Referenced explicitly per op so the
    /// no-hint set is a recorded decision, not a wildcard fallback.
    pub const NONE: Self = Self {
        axial_depth_mm: None,
        radial_width_mm: None,
        target_scallop_mm: None,
    };
}

impl OperationConfig {
    /// Pre-compute Z levels for depth stepping (top -> bottom).
    ///
    /// Every variant is named — no wildcard arm. An operation that does not
    /// step down uniformly says so in the last arm, which is a recorded
    /// decision and not a fallback: a new 2.5D operation fails to compile
    /// until it decides, instead of silently generating at one Z level.
    pub fn cutting_levels(&self, top_z: f64) -> Vec<f64> {
        use crate::ops::depth::DepthStepping;
        // The four uniform steppers had byte-identical arms. One closure
        // now holds the construction; the arms hold only the two fields.
        let uniform = |depth: f64, step: f64| {
            DepthStepping::new(top_z, top_z - depth.abs(), step).all_levels()
        };
        match self {
            Self::Pocket(cfg) => DepthStepping {
                start_z: top_z,
                final_z: top_z - cfg.depth.abs(),
                max_step_down: cfg.depth_per_pass,
                finishing_passes: cfg.finishing_passes,
            }
            .all_levels(),
            Self::Profile(cfg) => DepthStepping {
                start_z: top_z,
                final_z: top_z - cfg.depth.abs(),
                max_step_down: cfg.depth_per_pass,
                finishing_passes: cfg.finishing_passes,
            }
            .all_levels(),
            Self::Adaptive(cfg) => uniform(cfg.depth, cfg.depth_per_pass),
            Self::Zigzag(cfg) => uniform(cfg.depth, cfg.depth_per_pass),
            Self::Trace(cfg) => uniform(cfg.depth, cfg.depth_per_pass),
            // The plug kernel walks the same `z_ladder` arm over the same
            // total (`ops::inlay::inlay_plug_toolpath_with_cancel`).
            Self::InlayPlug(cfg) => uniform(cfg.total_cut_depth(), cfg.depth_per_pass),
            // The one Face ladder (W3.3): the kernel and the Layered Face
            // read the same function.
            Self::Face(cfg) => crate::ops::face::face_levels(top_z, cfg.depth, cfg.depth_per_pass),
            // No standard depth stepping. The 3D ops drive their own Z
            // ladder from the surface; VCarve, Chamfer and Inlay derive Z
            // from the cutter geometry; the two drilling ops are Z-only.
            Self::VCarve(_)
            | Self::Inlay(_)
            | Self::Drill(_)
            | Self::Chamfer(_)
            | Self::DropCutter(_)
            | Self::Adaptive3d(_)
            | Self::Waterline(_)
            | Self::Pencil(_)
            | Self::Scallop(_)
            | Self::UnifiedFinish(_)
            | Self::SteepShallow(_)
            | Self::RampFinish(_)
            | Self::SpiralFinish(_)
            | Self::RadialFinish(_)
            | Self::HorizontalFinish(_)
            | Self::ProjectCurve(_)
            | Self::AlignmentPinDrill(_) => vec![],
        }
    }
}

/// Resolve the spindle RPM for an operation, falling back to the project
/// default (`gcode::PostConfig.spindle_speed`)
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
    if operation.is_3d() {
        return Some(
            "Phase 1 feed optimization only supports operations that start from flat stock, not mesh-derived surfaces.",
        );
    }
    None
}

#[cfg(test)]
mod tests;
