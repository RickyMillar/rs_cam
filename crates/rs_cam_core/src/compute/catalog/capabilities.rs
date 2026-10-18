//! What an operation can do and how each consumer must treat it (W2.4).
//!
//! Every registry row states one [`Capabilities`] value, so a new operation
//! does not compile until it states each field. The row is the static
//! answer for the op kind. `OperationConfig::capabilities` starts from the
//! row and overrides the fields that read the config (Scallop's
//! `continuous`, a surface op's `stock_to_leave`, the feeds hints).
//!
//! Before W2.4 these answers were `match` lists outside the registry:
//! six in `catalog.rs`, one in `execute/shared.rs`, two in
//! `feeds/predict.rs` (one ended `_ => {}`, so a new op failed open), one
//! in `feeds/suggest/axial_envelope.rs` and three in `trace/narrate.rs`.
//! A consumer now reads the field. A consumer that maps a class to its own
//! numbers or words matches on the class enum, which is exhaustive.
//!
//! C2 moved six more answers here: the two narrate lists (`plans_bands`,
//! `ramp_descent`), the validation and static-check lists
//! (`adaptive_woc`, `flat_floor_clearing`), the advisor guard
//! (`clearing_strategy_dial`), the modulation planner floor
//! (`planner_uniform_engagement`) and two height checks
//! (`top_on_stock_box`, `floor_inside_stock`).
//!
//! D2 moved the vendor-table routing of the feeds lookup here
//! (`lut_route`, read by `feeds::vendor_normalize::lut_query_for` and the
//! explain payload).

use super::{FeedsHints, OperationTransformCapabilities};
use crate::compute::execute::LinkCeilingNeed;

/// One operation's capabilities. See the module doc.
///
/// `P` is the type of [`Self::entry_probe`]. A registry row holds
/// [`EntryProbeRule`] ([`RowCapabilities`]): the row states a RULE for the
/// leave, never a placeholder number. `OperationConfig::capabilities`
/// resolves the rule against the config and returns the default `P`,
/// [`EntryProbe`], which holds the real leave.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Capabilities<P = EntryProbe> {
    /// The order and link safety of the dressups. The row holds the value
    /// without the R10 lap cap; `OperationType::capabilities` adds the cap
    /// from the process role (`OperationType::ramp_fold_lap_cap`).
    pub transform: OperationTransformCapabilities,
    /// The air-cut percentage of TOTAL runtime above which a per-toolpath
    /// warning fires. `None`: the metric does not apply (drill kinematics:
    /// the dexel stock cannot see Z-only moves).
    ///
    /// Recalibrated 2026-08-21 (W5B-F4) against the swept stamping kernel.
    /// The evidence and the rejected alternatives are in
    /// `planning/perf_review_2026-08-19/DELTA_w5b_f4_aircut_DECISION.md`.
    /// The denominator is TOTAL runtime (cutting + rapids), never
    /// `air_cut_pct_of_cutting_time` (`MEASUREMENT_DOMAINS.md` LH-1).
    pub air_cut_high_threshold_pct: Option<f64>,
    /// A pinned Bottom Z on the Heights tab reaches the EMITTED motion
    /// (F1.19). Only three operations read `ResolvedHeights::bottom_z`:
    /// Adaptive3d (`z_floor`), UnifiedFinish (the band ladder) and
    /// Waterline (the level ladder). Every other operation anchors its floor
    /// at `top_z` minus its own depth dial, or rides the mesh, so a pinned
    /// bottom changes nothing it emits.
    ///
    /// This states what the generator does. It is not a recommendation.
    pub honors_pinned_bottom_z: bool,
    /// The entry-move surface probe (G-RAMPTERRAIN, design amendment 1).
    /// A row holds the rule ([`EntryProbeRule`]); a config's capabilities
    /// hold the resolved probe ([`EntryProbe`]).
    pub entry_probe: P,
    /// When the op's links need a link ceiling (ruling R1).
    pub link_ceiling: LinkCeilingNeed,
    /// The hints the feeds calculator must respect. The row holds
    /// [`FeedsHints::NONE`]; the hint values are config values, so
    /// `OperationConfig::capabilities` fills them.
    pub feeds_hints: FeedsHints,
    /// How Suggest's move-count predictor models this op.
    pub move_count: MoveCountModel,
    /// What the narration compares an axial-DOC sample against when the
    /// op has no commanded depth per pass.
    pub doc_basis: DocBasis,
    /// The air-cut hint the narration adds to its air-cut line.
    pub air_cut_hint: AirCutHint,
    /// The config field of the hookup dial that tunes this op's finishing
    /// link stage (G-LINKVISIBLE), or `None` when the op has none.
    pub relink_dial: Option<&'static str>,
    /// The class of axial envelope Suggest builds for this op.
    pub axial_envelope: AxialEnvelope,
    /// How the op uses the remaining stock when its stock source is
    /// `FromRemainingStock` (W3.4, AUDIT.md "Rest is not one thing").
    pub rest_use: RestUse,
    /// The XY containment the op reads while it plans (W6.2, AUDIT.md
    /// "Territory is three concepts"), and whether the session boundary
    /// clip applies after generation (D2). `compute::execute::Territory`
    /// resolves it once per op.
    pub territory_xy: TerritoryXy,
    /// The op plans finish bands (C2). The narration then states a
    /// measured "none" for the dropped-band and clipped-band lines,
    /// instead of "does not apply". Unified Finish.
    pub plans_bands: bool,
    /// The op descends by a ramp (C2). The narration then states that a
    /// missing ramp reach clamp is "not measured" for this run. Ramp
    /// Finish.
    pub ramp_descent: bool,
    /// The stepover is the narrow WOC of an adaptive planner (C2). The
    /// static validation then applies the 0.15 x D wood floor to it.
    /// 2D Adaptive and 3D Rough.
    pub adaptive_woc: bool,
    /// The op clears a flat floor (C2). A ball nose leaves scallops on
    /// that floor, so the static check gives a hint. Pocket, Face,
    /// Zigzag.
    pub flat_floor_clearing: bool,
    /// The config field of the clearing strategy that the strategy
    /// advisor compares (C2), or `None` when the op has no such dial.
    /// The advisor writes each candidate through this field name.
    pub clearing_strategy_dial: Option<&'static str>,
    /// The planner holds a uniform engagement equal to the target of its
    /// stepover (C2), so feed modulation may read that target as a floor
    /// under the simulated engagement. A row states `false`;
    /// `OperationConfig::capabilities` sets it for a contour-spiral
    /// strategy, which is a config value.
    pub planner_uniform_engagement: bool,
    /// The op starts at the stock box top and ignores a pinned Top Z
    /// (C2). A pinned bottom is then compared with the stock top. 3D
    /// Rough (`VolumetricOp::HEIGHTS.top`).
    pub top_on_stock_box: bool,
    /// The op's depth dial is a floor that must stay inside the stock
    /// (C2), so the depth-beyond-stock check applies. Project Curve
    /// measures its depth below the mesh, and Pin Drill goes into the
    /// spoilboard on purpose: both state `false`.
    pub floor_inside_stock: bool,
    /// How the feeds lookup routes the op's (family, role) query to the
    /// vendor table rows (D2). Read by
    /// `feeds::vendor_normalize::lut_query_for` and `feeds::explain`.
    pub lut_route: LutRoute,
    /// How the op cuts the drawing items it gets (P1, geometry selection
    /// DESIGN 4.2). The prologue gives a [`ItemUse::Regions`] op only the
    /// closed polygons, and a [`ItemUse::Curves`] op every polygon.
    /// `OperationConfig::capabilities` overrides it for Trace, whose
    /// compensation decides it.
    pub item_use: ItemUse,
}

/// The capabilities a registry row states: the entry probe is a rule.
pub type RowCapabilities = Capabilities<EntryProbeRule>;

impl<P> Capabilities<P> {
    /// True when the op's GENERATION reads the machine (B7).
    ///
    /// An op with a link stage (`relink_fragments` or the pencil link)
    /// costs each link against a retract with
    /// `MachineProfile::link_kinematics`, so a machine edit that moves
    /// that envelope can move its emitted links. [`LinkCeilingNeed::NoLink`]
    /// states that the op makes no such link. Every other op reads the
    /// machine at simulation time only, and a machine edit keeps its
    /// result.
    pub fn reads_link_kinematics(&self) -> bool {
        match self.link_ceiling {
            LinkCeilingNeed::Always | LinkCeilingNeed::WithRemainingStock => true,
            LinkCeilingNeed::NoLink => false,
        }
    }

    /// The same capabilities with `entry_probe` in place of this one. The
    /// body names every field, so a new field cannot be dropped here.
    pub fn with_entry_probe<Q>(self, entry_probe: Q) -> Capabilities<Q> {
        Capabilities {
            transform: self.transform,
            air_cut_high_threshold_pct: self.air_cut_high_threshold_pct,
            honors_pinned_bottom_z: self.honors_pinned_bottom_z,
            entry_probe,
            link_ceiling: self.link_ceiling,
            feeds_hints: self.feeds_hints,
            move_count: self.move_count,
            doc_basis: self.doc_basis,
            air_cut_hint: self.air_cut_hint,
            relink_dial: self.relink_dial,
            axial_envelope: self.axial_envelope,
            rest_use: self.rest_use,
            territory_xy: self.territory_xy,
            plans_bands: self.plans_bands,
            ramp_descent: self.ramp_descent,
            adaptive_woc: self.adaptive_woc,
            flat_floor_clearing: self.flat_floor_clearing,
            clearing_strategy_dial: self.clearing_strategy_dial,
            planner_uniform_engagement: self.planner_uniform_engagement,
            top_on_stock_box: self.top_on_stock_box,
            floor_inside_stock: self.floor_inside_stock,
            lut_route: self.lut_route,
            item_use: self.item_use,
        }
    }
}

/// How an operation cuts the drawing items it gets (P1; geometry selection
/// `DESIGN.md` 4.2). It is a config answer: Trace with compensation `Left`
/// or `Right` cannot follow an open path, with `None` it can.
///
/// The prologue (`compute/execute/prologue.rs`) matches on it with no
/// wildcard, so a new use states what it gets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemUse {
    /// The op reads no drawing item: the mesh ops and Face (the stock).
    None,
    /// Closed areas, nested by containment. The op cuts inside, outside or
    /// along a closed loop, so an open path has no area for it. The
    /// prologue leaves the open paths out and names them in a finding;
    /// it refuses when no closed polygon is left.
    Regions,
    /// Lines to follow, closed and open: the op cuts on the line. Trace
    /// with compensation `None`, Project Curve.
    Curves,
    /// Hole positions. `may_be_empty`: the op may have no hole to cut (Pin
    /// Drill, whose holes come from the alignment pins).
    Holes { may_be_empty: bool },
}

/// How an operation uses the remaining stock (W3.4, AUDIT.md "Rest is not
/// one thing"). A rest pass reads the machined stock; each kind says what
/// the op does with it. The Layered runner (`compute/execute/layered.rs`)
/// acts on `LevelClip` and `LevelSkip`; a `ContourFilter` op filters in its
/// own `emit_level`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestUse {
    /// The op ignores the remaining stock: a rest pass cuts what a fresh
    /// pass cuts.
    None,
    /// An area op clips its INPUT region to the standing material at each
    /// level, and skips a level with no material (G-RESTLEVELS). The op
    /// insets the clipped region by the tool radius again. Pocket, Zigzag,
    /// Face (on the facing rectangle).
    LevelClip,
    /// A contour op skips a level when no material stands in the reach
    /// band of its tool-centre path. It never clips the outline, because a
    /// clipped outline cuts a new wall. Profile, Trace, Chamfer (W3.5: its
    /// one level is the whole outline or nothing).
    LevelSkip,
    /// The per-level material mask seeds the planner's grid, so the
    /// planner does not clear a cell that a prior op cleared. 2D Adaptive.
    LevelMask,
    /// Each hole starts at the remaining stock top over the hole, not at
    /// the stock box top (D8). Drill.
    StockColumn,
    /// A contour op keeps, at each level, only the contour runs that pass
    /// material: a point stays when material stands above
    /// `z + FRESH_MATERIAL_THRESHOLD_MM` within `R + cell` of it, or when
    /// the stock grid holds no answer there. A stretch in air is dropped
    /// only when a retract and a re-entry take less time than the feed
    /// through it. The op splits a contour into runs as its boundary
    /// filter does (W6.1). Waterline; both halves of Steep and Shallow
    /// (W6.4.6: the contours of the steep half, the raster rows of the
    /// shallow half, each point at its own tool tip Z).
    ContourFilter,
    /// The planner owns the stock: it plans each run against its own
    /// dexel stock, seeded from the remaining stock. 3D Rough.
    Planner,
    /// The remaining stock is the rest reference of the op's claims or
    /// detectors (W2.1 `StockReference`). Pencil, Unified Finish.
    Reference,
    /// The remaining stock only builds the link ceiling of the op's links
    /// (ruling R1). The cut paths do not read it.
    LinkSafetyOnly,
}

impl RestUse {
    /// True when the rule compares the remaining stock with the surface,
    /// so the territory resolves the stock through the W2.1 guard
    /// (`Territory::reference_stock`): `Reference` and `ContourFilter`.
    /// Every other rule reads the stock without the guard, or not at all.
    ///
    /// A new rule must answer here and in the Layered runner's
    /// `prepare_outline` (both matches are exhaustive). An adapter that
    /// owns one rule compares with `==`.
    #[must_use]
    pub const fn reads_reference_stock(self) -> bool {
        match self {
            Self::Reference | Self::ContourFilter => true,
            Self::None
            | Self::LevelClip
            | Self::LevelSkip
            | Self::LevelMask
            | Self::StockColumn
            | Self::Planner
            | Self::LinkSafetyOnly => false,
        }
    }
}

/// The XY containment an operation reads while it plans (W6.2). The
/// session clips the result to the machining boundary after generation,
/// except for [`Self::SetupMap`] ([`Self::session_clip_applies`], D2);
/// this states what the op reads BEFORE that clip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerritoryXy {
    /// The op reads no XY containment while it plans. The session clip
    /// after generation is its only boundary. Pencil and the 2D ops.
    ClipAfter,
    /// The op pre-clips its sampling and its link stage to the boundary
    /// region set (the regions input of `TerritoryInputs`, P2.3). The 3D
    /// finishes, Waterline and Project Curve (its link stage).
    Regions,
    /// The planner reads the boundary outline (the pre-clear) and the
    /// centre containment (G-BOUNDARYPHANTOM), both inputs of
    /// `TerritoryInputs`. 3D Rough.
    PlannerBounds,
    /// No XY territory applies, before or after generation (D2 ruling).
    /// The op's positions are absolute setup-map positions that go
    /// through the whole stock, so a boundary must not move or drop them.
    /// The obstacle step still applies. Pin Drill.
    SetupMap,
}

impl TerritoryXy {
    /// True when the session clips the generated toolpath to the
    /// machining boundary (`clip_and_split_entries`). Every rule except
    /// [`Self::SetupMap`].
    #[must_use]
    pub const fn session_clip_applies(self) -> bool {
        match self {
            Self::ClipAfter | Self::Regions | Self::PlannerBounds => true,
            Self::SetupMap => false,
        }
    }
}

/// How the feeds lookup routes an op's (family, role) query to the vendor
/// table rows (D2). Before D2 this was three op-type compares in
/// `feeds::vendor_normalize::lut_query_for` and one in `feeds::explain`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LutRoute {
    /// The query reads the rows of the op's own family and role.
    AsStated,
    /// An `Adaptive` family query reads the `Pocket` rows of the same
    /// role. 3D Rough.
    AdaptiveAsPocket,
    /// G-DCFLAT: a flat end mill on a `Parallel` query reads the
    /// (`Pocket`, `Roughing`) rows, as the 3D Rough does. The table has no
    /// flat-end `Parallel` row. Every other query reads as stated. Drop
    /// Cutter.
    FlatParallelAsRoughPocket,
    /// The cutter class picks the rows, not the op's family (operator
    /// ruling 2026-09-25): ball, tapered ball and bull nose read
    /// (`Parallel`, `Finish`), a flat end (`Contour`, `Finish`), a V-bit
    /// (`Trace`, `Finish`), and a facing bit refuses. Project Curve.
    ByCutter,
}

/// The air-cut bar of a 3D finish op (W5B-F4 §5.1). The defect-free cluster
/// read 34.3–42.5 % (DropCutter 34.3, SpiralFinish 40.5, Waterline 40.5,
/// Scallop 41.6, RampFinish 42.5); 45 clears it by about 2.5 pp and still
/// flags SteepShallow (78) and RadialFinish (82). The old 30 fired on eight
/// of nine clean family members. Judgement on a cluster of nine, not a
/// derived constant.
pub const AIR_CUT_BAR_SURFACE_FINISH_PCT: f64 = 45.0;

/// The air-cut bar of Project Curve. Rivers and curves are small features
/// in big stock, so rapids dominate. Post-flip readings were 15.97 and
/// 10.90 on wanaka and 13.2–28.7 on an isolated river; 60 is about 2x the
/// highest. Loose on purpose: Project Curve is the one op that did not
/// settle under the swept kernel (−15.6 pp across two cell sizes, DECISION
/// §4.b).
pub const AIR_CUT_BAR_PROJECT_CURVE_PCT: f64 = 60.0;

/// The air-cut bar of 2.5D clearing, 3D rough and the 2D contour ops:
/// boundary overshoot and Z-level transitions make 40 % the high-water
/// mark.
pub const AIR_CUT_BAR_CLEARING_PCT: f64 = 40.0;

/// The entry-probe rule a registry row states. It holds no placeholder:
/// an op whose leave is a config dial says so, and
/// `OperationConfig::capabilities` reads the dial.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EntryProbeRule {
    /// A prism op. See [`EntryProbe::Prism`].
    Prism,
    /// A surface-riding op with no leave dial. Its cuts ride the CL
    /// surface plus this fixed leave (Drop Cutter and Waterline: `0.0`).
    SurfaceFixed { leave_mm: f64 },
    /// A surface-riding op whose leave is its config's `stock_to_leave`.
    SurfaceFromConfig,
}

/// The entry-move surface probe of an operation, resolved against its
/// config.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EntryProbe {
    /// A prism op: its passes go BELOW the model surface (a pocket cut into
    /// a block), so a mesh-surface floor would destroy its ramp entries.
    /// It keeps the audited 2D blind-leg behaviour (A0 finding 4).
    ///
    /// Adaptive3d is a prism op here on purpose: its dressup entries are
    /// stripped (`FORCE_NO_ENTRY`), and its planner builds its own probe
    /// with `params.stock_to_leave`.
    Prism,
    /// A SURFACE-RIDING op: its own cuts never go below the drop-cutter CL
    /// surface plus `leave_mm`, so an entry leg must not either. The
    /// value is the row's fixed leave or the config's `stock_to_leave`
    /// ([`EntryProbeRule`]).
    Surface { leave_mm: f64 },
}

impl EntryProbe {
    /// The leave allowance the probe protects, or `None` for a prism op.
    pub fn leave_mm(self) -> Option<f64> {
        match self {
            Self::Prism => None,
            Self::Surface { leave_mm } => Some(leave_mm),
        }
    }
}

/// How `feeds::predict::predict_move_count` models an op's move count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveCountModel {
    /// The input geometry (curve length, perimeter, hole count) sets the
    /// move count, not the bbox and stepover. No gate (`0`).
    FeatureDriven,
    /// No envelope model is written for this op. No gate (`0`). Before
    /// W2.4 UnifiedFinish reached this answer through a `_` arm.
    NotModelled,
    /// Passes along Y at the stepover, samples along X.
    Raster,
    /// Area fill at a narrow WOC per Z level, times the Z levels.
    Adaptive3d,
    /// One Z level of area fill at a narrow WOC.
    Adaptive2d,
    /// One full stepover x stepover cell per move.
    PocketCell,
}

/// What the narration names as the depth basis of an op with no commanded
/// depth per pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocBasis {
    /// The op has a depth per pass, or the narration has no special words
    /// for it.
    DepthPerPass,
    /// The op follows the MODEL SURFACE (H4 wave 15), so it has no
    /// depth per pass to compare against.
    SurfaceHeights,
    /// The op follows the curve at a fixed surface offset.
    CurveOnSurface,
    /// The op advances by peck depth.
    PeckDepth,
    /// The op cuts to a target V-bit depth.
    VBitDepth,
}

/// The hint the narration adds to an air-cut line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AirCutHint {
    /// No special hint.
    NoHint,
    /// A roughing op: boundary and stepover tuning, or stale stock.
    Roughing,
    /// A finishing op: surface terrain dominates the figure.
    SurfaceFinish,
    /// A curve-following op: rapids and approaches dominate the figure.
    CurveFollowing,
}

/// The axial envelope Suggest builds (`feeds/suggest/axial_envelope.rs`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AxialEnvelope {
    /// No envelope: 2D pocket, contour and drill ops, and the finish ops
    /// with no stepover basis. The other invariant passes cover them.
    NotApplied,
    /// A 3D rough: rough deflection limit, WOC = the stepover.
    Roughing3d,
    /// A V-bit groove: rough limit, WOC = half the groove width at the
    /// op's imposed axial depth.
    VBitGroove,
    /// A curve projected on the surface: rough limit, a fixed WOC.
    CurveRough,
    /// A 3D finish: finish limit and the default scallop target.
    Finish3d,
}

// ── The transform classes the rows name ─────────────────────────────────

/// XY-independent ops: TSP can reorder by proximity safely, and links are
/// allowed. Drill and the pin drill finish each hole (the peck cycle is
/// inside the hole) before the next, so the visit order changes no
/// material state. Project Curve emits each mesh-contact chain as its own
/// rapid→plunge→cut→retract unit with no order between chains (audited
/// fix-family Phase 1). Drop Cutter rows are independent too.
pub const TRANSFORM_XY_INDEPENDENT: OperationTransformCapabilities =
    OperationTransformCapabilities::new(true, false, false, true);

/// Face: its one-way rows and depth barriers encode the machining
/// sequence. It vetoes every rapid-order permutation and keeps the other
/// dressups; it is not continuous and takes no straight link.
pub const TRANSFORM_FACE: OperationTransformCapabilities =
    OperationTransformCapabilities::new(false, false, false, false).without_rapid_reorder();

/// V-bit ops (VCarve, Inlay, Inlay Plug): link moves gouged even WITH the
/// swept-corridor check (2026-08-03: Inlay 8.21 mm, VCarve 5.78 mm, all
/// deeper; pinned by the `*_link_moves_*` sentries). The cut WIDTH of a
/// V-bit depends on depth, so "the tip passed within the radius at this
/// Z" does not prove the corridor is clear to the width a bridge needs.
/// No links; rapid-order eligibility stays.
pub const TRANSFORM_V_WIDTH: OperationTransformCapabilities =
    OperationTransformCapabilities::new(false, false, false, false);

/// Ordered ops that may take links: Horizontal Finish (the generator sorts
/// regions high to low Z to avoid collisions, `horizontal_finish.rs`, so
/// TSP would override that safety order), Chamfer, Pencil, Radial Finish.
pub const TRANSFORM_ORDERED_LINKABLE: OperationTransformCapabilities =
    OperationTransformCapabilities::new(false, false, false, true);

/// Depth-stepping ops (Pocket, Profile, Zigzag, Waterline, Trace): depth
/// order is the constraint, not continuity.
pub const TRANSFORM_DEPTH_ORDER: OperationTransformCapabilities =
    OperationTransformCapabilities::new(false, true, false, false);

/// 2D Adaptive: the planner clears one material grid per level in the
/// order it emits its runs, and admits a keep-down link only when the grid
/// that EARLIER runs left holds it to the pass load
/// (`adaptive/path.rs`, `feed_link_within_pass_load`). The rapid-order pass
/// moves whole keep-down chains and can put a chain before the run that
/// cleared its corridor. Measured (G-ADAPTORDER, 2026-09-26, 120 x 80
/// pocket, six islands, 6 mm, 2 levels): the six links over 6 x R cut
/// 61.7 mm^3 in the planner order and 349.3 mm^3 reordered. The planner
/// stamps each re-entry's helix hole as cut (G-ADAPTPASSLOAD), so the
/// helix finishes its floor flat; it emits the path it walked, so the
/// dressup merge does not move its cuts.
pub const TRANSFORM_ADAPTIVE_2D: OperationTransformCapabilities =
    OperationTransformCapabilities::new(false, true, false, false)
        .without_rapid_reorder()
        .with_helix_floor_lap()
        .with_stock_aware_plunge()
        .with_planner_segment_merge();

/// Adaptive3d plans every run against its own dexel stock, in the order it
/// emits them. Each entry's rapid floor and helix start, each keep-down
/// proof and each ring's engagement assume the runs before it have cut. A
/// rapid-order permutation breaks all of these, also inside one depth pass,
/// and drops each planner rapid floor. Measured on rivmap100 (2026-09-25):
/// the reorder fed the tool from 14.0 to 7.78 mm through standing stock
/// (448 entry samples over twice the median bite, peak 6.08 mm); with the
/// order kept, 0 samples, peak 1.61 mm. The planner also applies the
/// segment merge before it stamps each cut (G-PLANSIMGAP: a merge after it
/// left up to 2.34 mm of material the planner held as cut).
pub const TRANSFORM_ADAPTIVE_3D: OperationTransformCapabilities =
    OperationTransformCapabilities::new(false, true, false, false)
        .without_rapid_reorder()
        .with_planner_segment_merge();

/// Ops stitched from independent nodes. `generate_unified_finish` emits a
/// `RapidOrderBarrier` at every node start (and per Z inside waterline
/// nodes), so the barriered TSP reorders runs WITHIN a node while the
/// router's cross-node order, costed against the machine envelope, stays
/// as routed. Links stay forbidden: `apply_link_moves` cannot see the 3D
/// surface between two fragments, which is why the op builds its own via
/// `surface_link::build_surface_link`. Steep/Shallow is the same shape one
/// level simpler: the two halves and the steep Z levels are barriered.
pub const TRANSFORM_BARRIERED_NODES: OperationTransformCapabilities =
    OperationTransformCapabilities::new(false, false, false, false);

/// Continuous traces (Scallop, Ramp Finish, Spiral Finish): helical or
/// spiral paths whose passes are not retract-separated.
pub const TRANSFORM_CONTINUOUS: OperationTransformCapabilities =
    OperationTransformCapabilities::new(false, false, true, false);

/// Discrete-ring Scallop (`ScallopConfig::continuous == false`, the
/// default): rings are radial offsets of ONE finishing pass to the same
/// final surface, with no order between them, so global reorder by
/// proximity is safe (cutting distance byte-identical, 2437.8 mm, rapid
/// travel −27 %). Link moves stay forbidden: `apply_link_moves` bridges
/// with a straight feed that ploughs through material on a 3D surface
/// (measured 9.7 mm over-cut). See `tests/capability_link_moves_safety.rs`.
///
/// This value carries NO R10 lap cap, as the per-instance override did
/// before W2.4. The static Scallop row (continuous) gets the finishing cap.
pub const TRANSFORM_DISCRETE_SCALLOP: OperationTransformCapabilities =
    OperationTransformCapabilities::new(true, false, false, false);
