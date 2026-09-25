use serde::{Deserialize, Serialize};

use crate::feeds::{OperationFamily as FeedsOperationFamily, PassRole};

use super::config::StockSource;
use super::operation_configs::{
    Adaptive3dConfig, AdaptiveConfig, AlignmentPinDrillConfig, ChamferConfig, DrillConfig,
    DropCutterConfig, FaceConfig, HorizontalFinishConfig, InlayConfig, PencilConfig, PocketConfig,
    ProfileConfig, ProjectCurveConfig, RadialFinishConfig, RampFinishConfig, RestConfig,
    ScallopConfig, SpiralFinishConfig, SteepShallowConfig, TraceConfig, UnifiedFinishConfig,
    VCarveConfig, WaterlineConfig, ZigzagConfig,
};

mod registry;
mod schema;

pub use schema::{
    DressupPolicy, EntryStylePolicy, Kinematics, OpPolicy, OpRegistryEntry, OperationParamSchema,
    OperationSchema, ParamDef, ParamHint, ParamRange, TOOLPATH_PARAM_DEFS, ToolConstraints,
    ToolConstraintsDef,
};

use registry::{
    REG_ADAPTIVE, REG_ADAPTIVE3D, REG_ALIGNMENT_PIN_DRILL, REG_CHAMFER, REG_DRILL, REG_DROP_CUTTER,
    REG_FACE, REG_HORIZONTAL_FINISH, REG_INLAY, REG_PENCIL, REG_POCKET, REG_PROFILE,
    REG_PROJECT_CURVE, REG_RADIAL_FINISH, REG_RAMP_FINISH, REG_REST, REG_SCALLOP,
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
    pub planner_applies_segment_merge: bool,
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
            (Rest,               RestConfig,                Menu2d,      "rest"),
            (Inlay,              InlayConfig,               Menu2d,      "inlay"),
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
    /// power / deflection gates, the optimizer skip, and the narrate
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

    pub fn transform_capabilities(self) -> OperationTransformCapabilities {
        self.transform_capabilities_without_lap_cap()
            .with_ramp_fold_lap_cap(self.ramp_fold_lap_cap())
    }

    /// The order and link capabilities alone; `transform_capabilities`
    /// adds the R10 lap cap on top.
    fn transform_capabilities_without_lap_cap(self) -> OperationTransformCapabilities {
        use OperationType::{
            Adaptive, Adaptive3d, AlignmentPinDrill, Chamfer, Drill, DropCutter, Face,
            HorizontalFinish, Inlay, Pencil, Pocket, Profile, ProjectCurve, RadialFinish,
            RampFinish, Rest, Scallop, SpiralFinish, SteepShallow, Trace, UnifiedFinish, VCarve,
            Waterline, Zigzag,
        };

        match self {
            // XY-independent ops: TSP can reorder by proximity safely.
            // Drill/AlignmentPinDrill: each hole is fully completed (peck cycle is intra-hole) before
            // moving to the next, so XY visit order has no material-state effect.
            // ProjectCurve: the generator (project_curve.rs) emits each contiguous
            // mesh-contact chain as its own independent rapid→plunge→cut→retract
            // unit — a chain flushes on any gap over air or a mesh hole, with no
            // depth-order or safety-order dependency between chains — so TSP can
            // freely reorder them by proximity too (audited fix-family Phase 1).
            DropCutter | Drill | AlignmentPinDrill | ProjectCurve => {
                OperationTransformCapabilities::new(true, false, false, true)
            }
            // HorizontalFinish: generator sorts regions high-to-low Z for collision-avoidance
            // (horizontal_finish.rs:170 "machine top shelves first to avoid collisions"); TSP
            // would override that safety ordering.
            // Face's one-way rows and generated depth barriers encode the
            // machining sequence. It must veto every rapid-order permutation,
            // while retaining other dressups; it is neither continuous nor
            // eligible for straight link moves.
            Face => OperationTransformCapabilities::new(false, false, false, false)
                .without_rapid_reorder(),
            // Measured gouging under link moves even WITH the swept-corridor
            // check in `dressup::apply_link_moves` (2026-08-03): Inlay 8.21mm,
            // VCarve 5.78mm — all strictly DEEPER (110/103 columns, zero
            // columns left proud), pinned by the `*_link_moves_*` sentries.
            // The corridor check is necessary but not sufficient here: VCarve
            // and Inlay's female pass are V-bit paths whose cut WIDTH depends
            // on depth, so "the tip passed within tool_radius at this Z" does
            // not imply the corridor was cleared to the width the bridge
            // needs. Until the check models depth-dependent width (or link
            // decisions move into the generators, where geometry is in scope),
            // these forbid links but retain rapid-order eligibility.
            Inlay | VCarve => OperationTransformCapabilities::new(false, false, false, false),
            HorizontalFinish | Chamfer | Pencil | RadialFinish => {
                OperationTransformCapabilities::new(false, false, false, true)
            }
            // Trace: multi-pass depth stepping; depth order is the constraint, not continuity.
            Pocket | Profile | Adaptive | Rest | Zigzag | Waterline | Trace => {
                OperationTransformCapabilities::new(false, true, false, false)
            }
            // Adaptive3d plans every run against its own dexel stock, in the
            // order it emits them. Each entry's rapid floor and helix start,
            // each keep-down proof and each ring's engagement assume that the
            // runs before it have cut. A rapid-order permutation breaks all
            // of these, also inside one depth pass. The rapid-order pass also
            // rebuilds the framing rapids, so it drops each planner rapid
            // floor. Measured on rivmap100 (2026-09-25, helix entry at
            // (9.5, 41.5)): the planner read the column at 7.47 mm after a
            // ring at x = 9.75 had cut it, but the reorder put that ring
            // 2 700 moves later. The tool fed straight down from 14.0 to
            // 7.78 mm through standing stock (to 12.0), and 448 entry
            // samples took more than twice the median bite (peak 6.08 mm).
            // With the order kept: 0 samples, peak 1.61 mm.
            // The planner also applies the segment merge before it stamps
            // each cut (G-PLANSIMGAP): the merge after the planner left up
            // to 2.34 mm of material that the planner stock held as cut
            // (rivmap100, 2026-09-25).
            Adaptive3d => OperationTransformCapabilities::new(false, true, false, false)
                .without_rapid_reorder()
                .with_planner_segment_merge(),
            // UnifiedFinish is stitched from independently generated region
            // nodes, not one continuous trace. `generate_unified_finish`
            // emits a `RapidOrderBarrier` at every node start (plus per-Z
            // barriers inside waterline nodes), so the barriered TSP can
            // reorder runs WITHIN a node while the router's cross-node
            // sequence — costed against the machine envelope, and carrying
            // the surface links — stays exactly as routed. Links stay
            // forbidden: `apply_link_moves` has no view of the 3D surface
            // between two fragment endpoints, which is why the op builds its
            // own via `surface_link::build_surface_link`.
            // SteepShallow is the same shape one level simpler: a Z-laddered
            // waterline pass over steep territory concatenated with a raster
            // over shallow territory. `generate_steep_shallow` barriers the
            // two halves and the steep half's Z levels, so the TSP reorders
            // within a half and never across one.
            UnifiedFinish | SteepShallow => {
                OperationTransformCapabilities::new(false, false, false, false)
            }
            // Genuinely continuous traces: helical/spiral paths whose passes
            // are not retract-separated, single-tool-down runs.
            Scallop | RampFinish | SpiralFinish => {
                OperationTransformCapabilities::new(false, false, true, false)
            }
        }
    }

    /// Op-kind air-cut percentage band above which a per-toolpath warning
    /// should fire. `None` means "metric not applicable" (drill kinematics
    /// — dexel can't measure Z-only moves; see `planning/P1_AIR_CUT_THRESHOLDS_RCA.md`).
    ///
    /// Originally calibrated from `WANAKA_ASSESSMENT_2026-05-19.md`
    /// expectation bands; **recalibrated 2026-08-21 (W5B-F4)** against the
    /// swept stamping kernel that became the `StampDispatch::Auto` resolution
    /// at `34d8917a`. The pre-flip readings these bands were fitted to were
    /// largely a simulation-cell artifact (one unchanged toolpath read 0.31 %
    /// at cell 0.25 and 89.61 % at cell 1.0); the evidence table, the flip
    /// list and the rejected alternatives are in
    /// `planning/perf_review_2026-08-19/DELTA_w5b_f4_aircut_DECISION.md`.
    ///
    /// Returning `Some(threshold)` means: a TP whose
    /// [`crate::stock::simulation_cut::AirCutRatios::air_cut_pct_of_total_runtime`]
    /// exceeds `threshold` is a real signal.
    ///
    /// **The denominator is TOTAL runtime (cutting + rapids)** — these bands
    /// were tuned against that measure and must not be compared against
    /// `air_cut_pct_of_cutting_time`, which is always larger and would fire
    /// these thresholds spuriously (`MEASUREMENT_DOMAINS.md` LH-1).
    pub fn air_cut_high_threshold_pct(self) -> Option<f64> {
        use OperationType::{
            Adaptive, Adaptive3d, AlignmentPinDrill, Chamfer, Drill, DropCutter, Face,
            HorizontalFinish, Inlay, Pencil, Pocket, Profile, ProjectCurve, RadialFinish,
            RampFinish, Rest, Scallop, SpiralFinish, SteepShallow, Trace, UnifiedFinish, VCarve,
            Waterline, Zigzag,
        };
        match self {
            // Drill kinematics: dexel polygon-to-material init can't see Z-only
            // moves, so air-cut % is unusable. Suppress entirely (Priority 4).
            Drill | AlignmentPinDrill => None,
            // ProjectCurve is inherently sparse: rivers/curves are tiny features
            // in big stock; rapids dominate by construction. The old 97 cited
            // "Wanaka TPs read 78–92% at-baseline" — post-flip the same project's
            // project-curve ops read 15.97 and 10.90, and an isolated river reads
            // 13.2–28.7, so 97 had become unreachable (a dead gate reads as
            // exoneration on `get_diagnostics`). 60 is ~2x the highest post-flip
            // reading; deliberately loose because ProjectCurve is the ONE op that
            // did not stabilise under swept (−15.6 pp across two cell sizes,
            // DECISION §4.b), and a tight band on an unstable measure is the
            // defect this review keeps finding.
            ProjectCurve => Some(60.0),
            // 3D finish ops: close-contact passes expected. The old 30 fired on
            // EIGHT of the nine family members measured on clean, defect-free
            // geometry with the correct tool — a bar that fires on essentially
            // every well-formed instance distinguishes nothing. The defect-free
            // cluster sits at 34.3–42.5 (DropCutter 34.3, SpiralFinish 40.5,
            // Waterline 40.5, Scallop 41.6, RampFinish 42.5); 45 clears it by
            // ~2.5 pp and still flags the two genuine outliers — SteepShallow 78
            // and RadialFinish 82 — by 1.7–1.8x. Judgement on a cluster of nine,
            // not a derived constant (DECISION §5.1).
            DropCutter | Scallop | UnifiedFinish | Waterline | Pencil | HorizontalFinish
            | SteepShallow | RampFinish | SpiralFinish | RadialFinish => Some(45.0),
            // 2.5D clearing and 3D rough: boundary overshoot + Z-level transitions
            // make 40% the high-water mark.
            Pocket | Face | Adaptive | Rest | Zigzag | Adaptive3d => Some(40.0),
            // 2D contour-style ops.
            Profile | Chamfer | Inlay | VCarve | Trace => Some(40.0),
        }
    }

    /// Whether a pinned Bottom Z on the Heights tab reaches this operation's
    /// EMITTED motion (F1.19).
    ///
    /// Three of the twenty-four operations read
    /// [`crate::compute::config::ResolvedHeights::bottom_z`] when they
    /// generate. The three sites are in `compute/execute.rs`:
    ///
    /// * `Adaptive3d` — `z_floor: heights.bottom_pinned.then_some(heights.bottom_z)`
    /// * `UnifiedFinish` — hands `heights.bottom_z` to the band ladder
    /// * `Waterline` — `waterline_z_levels(heights.top_z, heights.bottom_z, z_step)`
    ///
    /// Every other operation anchors its floor at `heights.top_z` minus its
    /// OWN depth dial, and a pinned bottom changes nothing it emits. For the
    /// seven depth-stepping operations the mechanism is
    /// [`OperationConfig::cutting_levels`], which takes `top_z` and nothing
    /// else: `session/compute.rs` builds the ladder before generation, and
    /// `execute.rs`'s `effective_levels` returns that ladder whenever it is
    /// non-empty — which it always is for those seven. The `else` branch of
    /// `effective_levels` is the one place a pinned bottom WOULD reach a 2.5D
    /// ladder, through `ResolvedHeights::depth()`, and it is unreachable from
    /// all six of its callers.
    ///
    /// This states what the generator does. It is not a recommendation. An
    /// operator who pins Bottom Z on a pocket sets a dial that moves no
    /// motion, and a surface that cautions on that number describes a cut the
    /// machine does not make — which is why F1.18's caution predicate reads
    /// the operation's own depth and never the pin.
    ///
    /// The repo has ruled this way before, in the other direction: adaptive3d
    /// "deliberately ignores a pinned top" because roughing must start at the
    /// real material top (`compute/config.rs`, `ResolvedHeights::top_pinned`).
    /// A family declaring one pin inert is established practice here.
    pub fn honors_pinned_bottom_z(self) -> bool {
        use OperationType::{
            Adaptive, Adaptive3d, AlignmentPinDrill, Chamfer, Drill, DropCutter, Face,
            HorizontalFinish, Inlay, Pencil, Pocket, Profile, ProjectCurve, RadialFinish,
            RampFinish, Rest, Scallop, SpiralFinish, SteepShallow, Trace, UnifiedFinish, VCarve,
            Waterline, Zigzag,
        };
        match self {
            // The three that read heights.bottom_z.
            Adaptive3d | UnifiedFinish | Waterline => true,
            // Depth-stepping family: the floor is `top_z - cfg.depth`, carried
            // in the pre-computed `cutting_levels` ladder.
            Pocket | Profile | Adaptive | Zigzag | Rest | Trace | Face => false,
            // Top-anchored own-depth ops that do not step: VCarve max_depth,
            // Inlay pocket_depth, Chamfer width, Drill depth, and the pin
            // drill's spoilboard allowance.
            VCarve | Inlay | Chamfer | Drill | AlignmentPinDrill => false,
            // Surface-riding finish ops: the mesh is the floor. They read
            // `heights.retract_z` only.
            DropCutter | Scallop | Pencil | HorizontalFinish | SteepShallow | RampFinish
            | SpiralFinish | RadialFinish => false,
            // ProjectCurve drops its depth below the mesh surface it projects
            // onto, not below the stock top, and reads `fallback_top_z`.
            ProjectCurve => false,
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
    UnifiedFinish(UnifiedFinishConfig),
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
    /// Whether the entry-move surface probe applies to this operation,
    /// and the leave allowance it must protect (G-RAMPTERRAIN,
    /// `planning/entry_moves_2026-09-03/`, design amendment 1).
    ///
    /// `Some(leave)` marks a SURFACE-RIDING operation: its own cut
    /// moves never go below `drop-cutter CL + leave`, so an entry leg
    /// must not either — the probe floor enforces exactly that.
    ///
    /// `None` marks a prism operation: its passes legitimately descend
    /// BELOW the model surface (a pocket cut into a block), so a
    /// mesh-surface floor would silently destroy its ramp entries.
    /// These keep the audited 2D blind-leg behaviour (A0 finding 4).
    ///
    /// Adaptive3d is `None` here on purpose: its dressup-door entries
    /// are stripped (`FORCE_NO_ENTRY`), and its planner door builds
    /// its own probe with `params.stock_to_leave`.
    ///
    /// Every variant is named — no wildcard arm — so a new operation
    /// must classify itself.
    pub fn entry_probe_leave(&self) -> Option<f64> {
        match self {
            OperationConfig::Face(_)
            | OperationConfig::Pocket(_)
            | OperationConfig::Profile(_)
            | OperationConfig::Adaptive(_)
            | OperationConfig::VCarve(_)
            | OperationConfig::Rest(_)
            | OperationConfig::Inlay(_)
            | OperationConfig::Zigzag(_)
            | OperationConfig::Trace(_)
            | OperationConfig::Drill(_)
            | OperationConfig::Chamfer(_)
            | OperationConfig::Adaptive3d(_)
            | OperationConfig::ProjectCurve(_)
            | OperationConfig::AlignmentPinDrill(_) => None,
            // Surface-riding: no leave dial, floor is the CL surface.
            OperationConfig::DropCutter(_) | OperationConfig::Waterline(_) => Some(0.0),
            OperationConfig::Pencil(c) => Some(c.stock_to_leave),
            OperationConfig::Scallop(c) => Some(c.stock_to_leave),
            OperationConfig::UnifiedFinish(c) => Some(c.stock_to_leave),
            OperationConfig::SteepShallow(c) => Some(c.stock_to_leave),
            OperationConfig::RampFinish(c) => Some(c.stock_to_leave),
            OperationConfig::SpiralFinish(c) => Some(c.stock_to_leave),
            OperationConfig::RadialFinish(c) => Some(c.stock_to_leave),
            OperationConfig::HorizontalFinish(c) => Some(c.stock_to_leave),
        }
    }

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
            OperationConfig::UnifiedFinish(_) => {
                optimizable!(FEED_RPM_SCALLOP, OperationType::UnifiedFinish)
            }
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

    /// CONFIG-aware transform capabilities — **the entry point production
    /// code should call** (both current call sites already do:
    /// `session/compute.rs`'s `tc.operation.transform_capabilities()` and
    /// `rs_cam_viz`'s `worker/helpers.rs`'s `req.operation.transform_capabilities()`
    /// go through this method, not [`OperationType::transform_capabilities`]).
    ///
    /// [`OperationType::transform_capabilities`] is a *static per-op-type*
    /// table: it has no way to see per-instance config, so it must answer
    /// for the worst case a given op type can produce. That conservatism is
    /// correct as a fallback for callers with no `OperationConfig` in hand,
    /// but it is provably too strict for ops whose safe-transform
    /// classification depends on a config field.
    ///
    /// Scallop is the first such op (fix-family Phase 1b). Its op-type
    /// table entry stays pinned at the conservative
    /// `(false, false, true, false)` — continuous-path-required — because
    /// `ScallopConfig::continuous` (`operation_configs.rs:820`, default
    /// `false` at `operation_configs.rs:836`) controls which of two
    /// structurally different emitters `scallop.rs` runs:
    ///
    /// - `continuous: false` (**the default**) takes the discrete-ring
    ///   branch (`scallop.rs:913-975`): every ring is split into
    ///   `keep_point`-contiguous runs and each run gets its own
    ///   rapid→plunge→cut→retract (`scallop.rs:953-975`). Rings are radial
    ///   offsets of ONE finishing pass down to the same final surface — no
    ///   ring depends on another ring's material state, there is no depth
    ///   order, and no generator-imposed safety order (contrast
    ///   `HorizontalFinish`, which sorts high-to-low for collision
    ///   avoidance). That is structurally identical to the "XY-independent,
    ///   TSP can reorder by proximity safely" bucket `DropCutter` /
    ///   `Drill` / `ProjectCurve` already sit in above.
    ///
    ///   **RE-ENABLED (fix-family Phase 1c).** The earlier revert reasoning
    ///   was sound about ring INDEPENDENCE but attributed the measured
    ///   GOUGE to the wrong knob. The sentry
    ///   `scallop_discrete_capability_currently_blocked_reorder_gouges`
    ///   measured a real over-cut when reorder was enabled: 146 dexel
    ///   columns cut DEEPER vs 20 shallower, net −128.9 mm of extra
    ///   material removed, worst column 9.33 mm over-cut — while rapid
    ///   travel fell 33.5 % (3263 → 2169 mm). That gouge was produced by
    ///   `apply_link_moves` bridging retract→rapid→plunge triples with a
    ///   straight feed that plows laterally through material, NOT by the
    ///   reorder itself: holding link moves off and varying only
    ///   `optimize_rapid_order`, cutting distance is byte-identical
    ///   (2437.8mm → 2437.8mm, only rapids change) while rapid travel still
    ///   drops ~27%. `OperationTransformCapabilities` used to have no way
    ///   to express "reorder yes, link moves no" — `allows_link_moves()`,
    ///   `allows_barriered_rapid_reorder()`, and
    ///   `allows_unbarriered_rapid_reorder()` all hung off the same two
    ///   booleans. Now that `allows_link_moves` is its own field (fix-family
    ///   Phase 1c), discrete Scallop can state the true, narrower
    ///   capability below: global rapid reorder allowed, link moves
    ///   forbidden. See
    ///   `crates/rs_cam_core/tests/capability_link_moves_safety.rs` for the
    ///   measurement backing both halves of that split.
    /// - `continuous: true` takes the spiral branch (`scallop.rs:812-912`):
    ///   rings are stitched into one helical stay-down path with
    ///   ring-to-ring cutting-feed connectors (`scallop.rs:874-903`).
    ///   Reordering or link-inserting into that sequence would corrupt the
    ///   single continuous cut, so this falls through to the op-type
    ///   default, which stays `(false, false, true, false)`.
    ///
    /// This method is the intended extension point for any future
    /// config-dependent op: add a match arm here rather than trying to
    /// force more nuance into the static [`OperationType`] table (which by
    /// design only sees the op kind, not its parameters).
    pub fn transform_capabilities(&self) -> OperationTransformCapabilities {
        match self {
            // Discrete-ring Scallop: rings are radial offsets of one
            // finishing pass to the same final surface, independent of each
            // other, so global reorder-by-proximity is safe (measured
            // cutting-distance-preserving above). Link moves stay forbidden
            // — `apply_link_moves` bridges with a straight feed that can
            // plow through material on a 3D surface (measured 9.7mm
            // over-cut); see the doc comment above.
            OperationConfig::Scallop(cfg) if !cfg.continuous => {
                OperationTransformCapabilities::new(true, false, false, false)
            }
            // Everything else (including continuous Scallop) falls through
            // to the conservative op-type default.
            _ => self.op_type().transform_capabilities(),
        }
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
    /// Every def's own name, then every alias it declares (CMP-08). The
    /// refusal message of `set_toolpath_param` prints this list, so a
    /// name the setter accepts and this list omits is a lie told to the
    /// caller that just got refused.
    pub fn param_names_for_type(op_type: OperationType) -> Vec<&'static str> {
        param_defs_for_type(op_type)
            .iter()
            .flat_map(|def| std::iter::once(def.name).chain(def.aliases.iter().copied()))
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
                aliases: def.aliases.iter().map(|a| (*a).to_owned()).collect(),
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
                    aliases: Vec::new(),
                })
                .collect(),
            tool_constraints: tool_constraints_for_type(op_type),
        }
    }
}

/// The def one settable NAME resolves to — its own name, or an alias it
/// declares (CMP-08).
fn param_def_for_type(op_type: OperationType, param: &str) -> Option<&'static ParamDef> {
    param_defs_for_type(op_type)
        .iter()
        .find(|def| def.name == param || def.aliases.contains(&param))
}

fn param_defs_for_type(op_type: OperationType) -> &'static [ParamDef] {
    op_type.registry_entry().param_defs
}

fn tool_constraints_for_type(op_type: OperationType) -> ToolConstraints {
    op_type.registry_entry().tool_constraints.to_schema()
}

/// Operation-specific hints handed to the feeds calculator (registry
/// companion to [`OpRegistryEntry`]). Hints depend on live config values
/// (z-step, scallop target, …), so this is an accessor on
/// [`OperationConfig`] rather than static registry data — see
/// [`OperationConfig::feeds_hints`].
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
    /// Extract this operation's feeds-calculator hints. Exhaustive match
    /// — adding an operation does not compile until it decides its hints
    /// (the pre-registry `_ => (None, None, None)` wildcard in
    /// `feeds::suggest::operation_feeds_hints` is gone; that function now
    /// delegates here).
    pub fn feeds_hints(&self) -> FeedsHints {
        match self {
            OperationConfig::Scallop(cfg) => FeedsHints {
                target_scallop_mm: Some(cfg.scallop_height),
                ..FeedsHints::NONE
            },
            OperationConfig::UnifiedFinish(cfg) => FeedsHints {
                target_scallop_mm: Some(cfg.scallop_height),
                ..FeedsHints::NONE
            },
            // DropCutter (the "3D Finish" parallel-raster op) optionally
            // derives stepover from a scallop target the same way Scallop
            // does. `None` keeps the legacy `ae_factor × diameter` stepover.
            OperationConfig::DropCutter(cfg) => FeedsHints {
                target_scallop_mm: cfg.scallop_height,
                ..FeedsHints::NONE
            },
            OperationConfig::Waterline(cfg) => FeedsHints {
                axial_depth_mm: Some(cfg.z_step),
                ..FeedsHints::NONE
            },
            OperationConfig::SteepShallow(cfg) => FeedsHints {
                axial_depth_mm: Some(cfg.z_step),
                ..FeedsHints::NONE
            },
            OperationConfig::VCarve(cfg) => FeedsHints {
                axial_depth_mm: Some(cfg.max_depth),
                ..FeedsHints::NONE
            },
            OperationConfig::RampFinish(cfg) => FeedsHints {
                axial_depth_mm: Some(cfg.max_stepdown),
                ..FeedsHints::NONE
            },
            // Explicit no-hint decisions — the calculator falls back to
            // LUT / diameter-factor defaults for these:
            OperationConfig::Face(_)
            | OperationConfig::Pocket(_)
            | OperationConfig::Profile(_)
            | OperationConfig::Adaptive(_)
            | OperationConfig::Rest(_)
            | OperationConfig::Inlay(_)
            | OperationConfig::Zigzag(_)
            | OperationConfig::Trace(_)
            | OperationConfig::Drill(_)
            | OperationConfig::Chamfer(_)
            | OperationConfig::Adaptive3d(_)
            | OperationConfig::Pencil(_)
            | OperationConfig::SpiralFinish(_)
            | OperationConfig::RadialFinish(_)
            | OperationConfig::HorizontalFinish(_)
            | OperationConfig::ProjectCurve(_)
            | OperationConfig::AlignmentPinDrill(_) => FeedsHints::NONE,
        }
    }
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
            Self::Rest(cfg) => uniform(cfg.depth, cfg.depth_per_pass),
            Self::Trace(cfg) => uniform(cfg.depth, cfg.depth_per_pass),
            Self::Face(cfg) => {
                if cfg.depth <= 0.0 {
                    vec![top_z]
                } else {
                    uniform(cfg.depth, cfg.depth_per_pass)
                }
            }
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
mod tests;
