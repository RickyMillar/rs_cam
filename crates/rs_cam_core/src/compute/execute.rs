//! Unified operation execution — one entry point for all 23 toolpath operations.
//!
//! Both `ProjectSession` and the GUI compute worker delegate here so the
//! operation dispatch logic exists exactly once.

use std::sync::atomic::AtomicBool;

use crate::compute::catalog::OperationConfig;
use crate::compute::config::ResolvedHeights;
use crate::compute::tool_config::ToolConfig;
use crate::geo::BoundingBox3;
use crate::geometry::region_set::RegionSet;
use crate::io::dxf_input::DrillTarget;
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::polygon::Polygon2;
use crate::tool::ToolDefinition;
use crate::toolpath::Toolpath;
use crate::trace::debug_trace::ToolpathDebugContext;
use crate::trace::semantic_trace::ToolpathSemanticContext;
use crate::trace::toolpath_spans::AnnotatedToolpath;

// ── Error type ────────────────────────────────────────────────────────

/// Errors that can occur during operation execution.
#[derive(Debug, Clone)]
pub enum OperationError {
    /// Required geometry (mesh, polygons) is missing or invalid.
    MissingGeometry(String),
    /// Tool type doesn't match operation requirements.
    InvalidTool(String),
    /// Operation was cancelled.
    Cancelled,
    /// Other operation failure.
    Other(String),
}

// ── Generated toolpath helpers ───────────────────────────────────────

pub type GeneratedToolpath = AnnotatedToolpath;

/// Facts an operation learned about the geometry while generating, which
/// are NOT properties of the emitted toolpath.
///
/// `GeneratedToolpath` is an alias for [`AnnotatedToolpath`], so an adapter
/// has historically had exactly one way to say anything: put it in the
/// toolpath. A finding like "the ring cascade could not reach the middle of
/// this region" has no home there — it is not geometry the machine will
/// execute — so it lived and died in a `tracing::warn!`. That is why a
/// 28 mm block of standing material shipped for weeks, and why the GUI's
/// diagnostics list has nothing to say about it (design doc §13/§14c/§14h).
///
/// Carried on [`ExecutionContext::findings`] as a [`Cell`] so adapters can
/// record without any signature change, and returned alongside the toolpath
/// by [`execute_operation_annotated_with_regions`]. Deliberately NOT part of
/// `AnnotatedToolpath`: a diagnostic finding must not have to survive the
/// dressup pipeline, where every carrier is one missed field-copy away from
/// silently vanishing.
/// C8: no longer `Copy`, and carried in a `RefCell` rather than a `Cell`,
/// because [`Self::derived_stepovers`] is a collection. The `Cell` was only
/// ever a convenience for `Copy` scalars, and it is what made a
/// first-writer-wins slot look like a design instead of a shrug.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GenerationFindings {
    /// Region-interior area (mm², XY-projected) a scallop ring cascade left
    /// UNCUT because it hit `max_rings` before collapsing — summed over
    /// every region, and over the mid-steep bands of a `UnifiedFinish`.
    ///
    /// `None` means **no cascade ran**, so nothing was measured; `Some(0.0)`
    /// means a cascade ran and collapsed. Only the adapters that actually
    /// run one write here, which is what keeps the two apart all the way to
    /// [`crate::compute::config::ToolpathStats::truncated_core_mm2`]
    /// (A/M9 / `MEASUREMENT_DOMAINS.md` X-19). See
    /// [`crate::finish::scallop::ScallopReport::uncut_core_mm2`].
    pub truncated_core_mm2: Option<f64>,
    /// M4 §5b: the hole-aware sibling of [`Self::truncated_core_mm2`] —
    /// summed the same way, over the same adapters, straight off
    /// [`crate::finish::scallop::ScallopReport::untouched_mm2`]. Same X-19
    /// three-valued contract: `None` = no cascade ran.
    pub untouched_material_mm2: Option<f64>,
    /// M4 §5b: the ESTIMATED reached-but-dropped sibling of
    /// [`Self::truncated_core_mm2`], off
    /// [`crate::finish::scallop::ScallopReport::standing_mm2`]. Same X-19 contract.
    /// Remember this one is an estimator, not an exact area — see the
    /// source field's doc for what it cannot distinguish.
    pub reached_uncut_estimate_mm2: Option<f64>,
    /// Wave D1: a planned finish band whose cutting was entirely erased by
    /// height resolution — an unmachined feature. `None` = no banded
    /// decomposition ran, or every band it planned survived.
    /// See [`crate::compute::config::DroppedBandFinding`].
    pub dropped_band: Option<crate::compute::config::DroppedBandFinding>,
    /// C8: a planned finish band whose Z ladder height resolution SHORTENED
    /// while it still cut. `None` = no band was partially clipped, or no
    /// banded decomposition ran. Disjoint from [`Self::dropped_band`].
    /// See [`crate::compute::config::ClippedBandFinding`].
    pub clipped_band: Option<crate::compute::config::ClippedBandFinding>,
    /// Wave D1: tip float on the emitted valley centrelines. `None` = the
    /// operation emits no centrelines, so nothing was measured.
    /// See [`crate::compute::config::TipFloatFinding`].
    pub tip_float: Option<crate::compute::config::TipFloatFinding>,
    /// PR-5: a retired dial still set to a non-default value in the loaded
    /// project. `None` = nothing retired is set.
    /// See [`crate::compute::config::DeprecatedDialFinding`].
    pub deprecated_dial: Option<crate::compute::config::DeprecatedDialFinding>,
    /// PR-6a: the offset stepovers this operation derived from the canonical
    /// reach policy. EMPTY = the operation derives none.
    ///
    /// C8: a `Vec`, not a slot. One toolpath can derive a stepover TWICE —
    /// the operation's own routing/fit site during generation, then PR-7's
    /// generic rest-analysis post-pass — and the slot resolved that by
    /// first-writer-wins, which the code itself called "a shrug, not a
    /// decision" (`ANTIPATTERNS_BACKLOG.md` P8). The second derivation was
    /// dropped on the floor: a `UnifiedFinish` with claims on AND generic
    /// rest analysis on published one of its two numbers and no hint that
    /// the other existed. Both are recorded now, in the order they fired,
    /// each naming its own `site`.
    ///
    /// See [`crate::compute::config::DerivedStepoverFinding`].
    pub derived_stepovers: Vec<crate::compute::config::DerivedStepoverFinding>,
    /// PR-8b: what the ramp-finish reach clamp did. `None` = no ramp descent
    /// ran, so nothing was measured; `Some` with an inert clamp is a
    /// measured-clean descent. See [`crate::finish::ramp_finish::RampReachClamp`].
    pub ramp_reach_clamp: Option<crate::finish::ramp_finish::RampReachClamp>,
    /// A/M6: which rest reference the crease/pencil claims pipeline resolved
    /// to, and whether it was pinned or derived. `None` = the claims
    /// pipeline did not run, so nothing was resolved.
    /// See [`crate::compute::config::ClaimsReferenceFinding`].
    pub claims_reference: Option<crate::compute::config::ClaimsReferenceFinding>,
    /// A4: this rest pass's emitted cutting geometry never reaches under the
    /// reference stock it was planned on, so it will remove nothing. `None` =
    /// the measurement did not run (no resolved machined-stock reference, or
    /// no cutting geometry) or it ran and found real engagement.
    /// See [`crate::compute::config::ZeroRemovalFinding`].
    pub zero_removal: Option<crate::compute::config::ZeroRemovalFinding>,
    /// Checkpoint C (Q1 / D-2): how many of this generation's 2D offset
    /// calls came back with a [`crate::polygon::OffsetFailure`].
    ///
    /// `None` = the operation made no offset call through the reporting
    /// name, so nothing was measured; `Some(0)` = it did and every one was
    /// clean. Only the adapters that opt into
    /// [`crate::polygon::offset_polygon_reported`] write here, which is what
    /// keeps those two apart all the way to
    /// [`crate::compute::config::ToolpathStats::offset_library_failures`].
    pub offset_library_failures: Option<usize>,
    /// Checkpoint C (Q2 / D-3a option b): the machining-boundary containment
    /// collapsed and the clip was therefore not applied. `None` = nothing was
    /// dropped. See [`crate::compute::config::BoundaryClipDroppedFinding`].
    ///
    /// Unlike every other field here this one is recorded AFTER the operation
    /// adapter has returned — the boundary clip is a post-dressup step in
    /// `ProjectSession::generate_toolpath` and in the GUI worker — so it is
    /// written through `&mut GenerationFindings` rather than through
    /// [`ExecutionContext::findings`]. Both writers hold the findings by then;
    /// the join has not run yet.
    pub boundary_clip_dropped: Option<crate::compute::config::BoundaryClipDroppedFinding>,
    /// F4: a non-default rest-claims dial this operation's own configuration
    /// never applies. `None` = nothing inert is set.
    /// See [`crate::compute::config::InertClaimsDialFinding`].
    pub inert_claims_dial: Option<crate::compute::config::InertClaimsDialFinding>,
    /// F3: what the [`crate::geometry::region_mask::MAX_REST_REGIONS`] cap did to this
    /// operation's rest-region extraction. `None` = no extraction ran, so
    /// nothing was measured — never "nothing was truncated". Written by the
    /// rest-analysis attach; see
    /// [`crate::compute::config::ToolpathStats::region_cap`].
    pub region_cap: Option<crate::geometry::region_mask::RegionCapReport>,
    /// Phase O item 3 / G-LINKVISIBLE: what this operation's finishing link
    /// stage did, and why it declined. `None` = the stage never ran (a
    /// family that has none, or one whose hookup dial is `0.0`). Written by
    /// `unified_finish`, `scallop`, `drop_cutter` and `waterline`. See
    /// [`crate::compute::config::ToolpathStats::relink`].
    pub relink: Option<crate::finish::unified_finish::RelinkTotals>,
    /// G-LINKVISIBLE: what the PENCIL's own link stage did. `None` = not a
    /// pencil, or a pencil whose detector produced no centreline, so the
    /// emitter — and with it every junction decision — never ran. Its own
    /// slot rather than [`Self::relink`] because its counter set differs;
    /// see [`crate::compute::config::ToolpathStats::pencil_link`].
    pub pencil_link: Option<crate::finish::pencil::PencilLinkReport>,
    /// C2: what the shallow band's monotone-cell decomposition did. `None` =
    /// the pass never ran (not a `UnifiedFinish`, or its
    /// `monotone_cell_decomposition` is off, or the op emitted no Shallow
    /// region). See [`crate::compute::config::ToolpathStats::monotone_cells`].
    pub monotone_cells: Option<crate::finish::unified_finish::MonotoneCellTotals>,
}

impl std::fmt::Display for OperationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingGeometry(s) => write!(f, "Missing geometry: {s}"),
            Self::InvalidTool(s) => write!(f, "Invalid tool: {s}"),
            Self::Cancelled => write!(f, "Operation cancelled"),
            Self::Other(s) => write!(f, "{s}"),
        }
    }
}

impl std::error::Error for OperationError {}

impl From<String> for OperationError {
    fn from(s: String) -> Self {
        Self::Other(s)
    }
}

// ── Phase-5 family adapters (T11) ────────────────────────────────────

/// Bundled inputs for one operation execution — everything
/// [`execute_operation_annotated`] receives, minus the operation
/// itself. Family adapters ([`GenerateFn`]) take this context so every
/// migrated family shares ONE signature; in particular `cancel` is
/// always in scope for every adapter to read.
///
/// Having `cancel` in scope does NOT by itself guarantee an adapter polls
/// it — nothing stops a `GenerateFn` from ignoring the field entirely (that
/// was exactly the 2026-07 incident: a fine-stepover mesh-finish generation
/// hung the GUI for two hours because its adapter never rebuilt the
/// `|| cancel.load(Ordering::SeqCst)` closure). The only families with a
/// verified guarantee are the ones exercised by
/// `cancellable_families_honour_a_preset_cancel_flag`: Adaptive, DropCutter,
/// Adaptive3d, Waterline, Pencil, Scallop, SteepShallow, RampFinish,
/// SpiralFinish, RadialFinish, HorizontalFinish (the 2026-07 mesh-finish
/// fix's 11), plus Pocket, Profile, Zigzag, Trace, Face, ProjectCurve,
/// VCarve, Inlay (the flat-2D S.5 fix,
/// planning/finishing_stack_review_2026-07.md), plus Rest and Drill
/// (Checkpoint C Q3, 2026-08-05 — W4's F-4 measured those two ignoring a
/// pre-set flag entirely: `generate_rest` built no `cancel_fn` and called the
/// non-cancellable `depth::toolpath_at_levels`, and `generate_drill` never
/// read `ctx.cancel`), plus UnifiedFinish, plus AlignmentPinDrill and
/// Chamfer (O-CANC, 2026-08-14 — neither was in the 2D campaign's nine,
/// so W4's evidence never reached them).
///
/// **24 of 24 registered families**, and the denominator is the
/// correction. This paragraph read "21 of 23" until 2026-08-14: the 21
/// was the sentry's case count and the 23 was an undercount of
/// `OperationType::ALL`, which has **24** entries (`for_each_op!`).
/// UnifiedFinish was the family that fell through the gap — it has
/// routed through `unified_finish_toolpath_with_cancel` all along and
/// was simply never enumerated, so it was neither in the claimed
/// numerator nor named as an exception. The count is no longer written
/// by hand: `cancellable_families_honour_a_preset_cancel_flag` asserts
/// its own case list covers `OperationType::ALL`, so a 25th family
/// cannot join without either polling the flag or turning that sentry
/// red.
pub struct ExecutionContext<'a> {
    /// Write-only sink for generation-time findings (see
    /// [`GenerationFindings`]). A `Cell` rather than a return value so an
    /// adapter can report one without changing the `GenerateFn` signature
    /// every family shares.
    pub findings: &'a std::cell::RefCell<GenerationFindings>,
    pub mesh: Option<&'a TriangleMesh>,
    pub index: Option<&'a SpatialIndex>,
    pub polygons: Option<&'a [Polygon2]>,
    /// The model's pickable drill targets — DXF `POINT` entities and
    /// circle/arc centres — in model coordinates, like `selected_holes`.
    /// The `Drill` family's only hole source besides an explicit pick
    /// (G-DRILLCENTROID); every other family ignores it. Empty for meshes
    /// and SVG, and for callers that resolve no model.
    pub drill_targets: &'a [DrillTarget],
    pub tool_def: &'a ToolDefinition,
    pub tool_cfg: &'a ToolConfig,
    pub heights: &'a ResolvedHeights,
    pub cutting_levels: &'a [f64],
    pub stock_bbox: &'a BoundingBox3,
    /// The setup's world→local transform, or `None` for an identity setup.
    ///
    /// Everything else in this context arrives ALREADY in the emission
    /// frame — `mesh`, `polygons` and `boundary` are transformed by the
    /// driver before generation, `stock_bbox` is the emission-frame bbox.
    /// The one class of input that cannot be pre-transformed that way is
    /// config-carried geometry: coordinates that live inside the
    /// `OperationConfig` and never pass through the driver's geometry
    /// pipeline. Today that is the drill families' `selected_holes` —
    /// world-frame picks off `LoadedModel::drill_targets`, consumed
    /// verbatim and therefore off by the whole setup transform on any
    /// non-identity setup (G-DRILLPICK-FRAME).
    ///
    /// `None` MUST be a no-op: identity setups already emit in world, so an
    /// adapter that reaches for this has to leave world coordinates alone
    /// rather than inventing an identity matrix to run them through.
    pub setup_transform: Option<&'a crate::compute::transform::SetupTransformInfo>,
    pub prev_tool_radius: Option<f64>,
    /// R1 (pencil): the resolved *real* reference tool config, when the pencil
    /// op's `reference_tool_id` names a library tool. Owned clone (small);
    /// resolution happens upstream because the context has no tool list, exactly
    /// like `prev_tool_radius`. `generate_pencil` turns it into a `ToolDefinition`.
    pub reference_tool_cfg: Option<ToolConfig>,
    pub debug_ctx: Option<&'a ToolpathDebugContext>,
    pub cancel: &'a AtomicBool,
    pub initial_stock: Option<&'a crate::dexel_stock::TriDexelStock>,
    pub semantic_ctx: Option<&'a ToolpathSemanticContext>,
    pub boundary: Option<&'a Polygon2>,
    /// P2.3: sibling of `boundary` — the multi-region set the mesh-finish
    /// family (scallop / radial / spiral / steep-shallow / ramp / horizontal
    /// / waterline / drop_cutter) pre-clips generation to, so sampling never
    /// wastes work outside the machining boundary and never has to be
    /// discarded at post-clip. `boundary` remains the adaptive3d
    /// single-polygon pre-clear path; the two carry independent semantics
    /// today and consolidating them is deferred. Consolidated onto
    /// `RegionSet` (region_set.rs) so containment tests share one
    /// implementation across every family.
    pub boundary_regions: Option<&'a RegionSet<'a>>,
    /// P1 quantitative linker (unified-finishing-pass W4a): the machine
    /// envelope the pencil generator (and, in future, other finishing
    /// families with a hookup/link decision) costs surface-link vs.
    /// retract-link candidates against with the F-034 integrator. `None`
    /// keeps the legacy distance-only hookup decision — production
    /// builders that have a machine profile in scope populate `Some`;
    /// callers without one (or that never reach a linking decision) pass
    /// `None`.
    pub link_kinematics: Option<crate::machine::kinematics::LinkKinematics>,
    /// P2.5's generic rest-analysis config, threaded through so an adapter
    /// can run its OWN in-op rest-depth pass instead of (or in addition
    /// to) the generic post-generation attach below — currently only
    /// `generate_unified_finish`'s v3 S1 claims pipeline. `None` is a
    /// byte-identical no-op for every other family, same shape as
    /// `link_kinematics`.
    pub rest_analysis: Option<&'a crate::compute::config::RestAnalysisConfig>,
}

/// A family adapter: generate the toolpath (with spans + annotations)
/// for one operation family. Registered on `OpRegistryEntry.generate`
/// per family as the Phase-5 cutover proves each one; unmigrated
/// families fall back to the exhaustive match in
/// [`execute_operation_annotated`]. An adapter must reproduce its
/// family's span helper AND annotate fn — wrong/empty spans simulate
/// fine and are only caught by span-aware tests, not the compiler.
pub type GenerateFn =
    fn(&ExecutionContext<'_>, &OperationConfig) -> Result<GeneratedToolpath, OperationError>;

/// R2.3: the config-guard boilerplate duplicated 23× across every family
/// adapter (`let OperationConfig::X(cfg) = op else { return Err(refusal) }`).
/// Expands to the identical `let`-else guard; `$fn_name` is passed as a
/// literal (macro hygiene has no way to recover the enclosing fn's name)
/// so the error text stays byte-identical to what it was before the
/// macro existed — nothing downstream matches on this string, but
/// keeping it stable avoids surprising anyone grepping logs for it.
macro_rules! config_guard {
    ($op:expr, $variant:ident, $fn_name:literal) => {
        match $op {
            OperationConfig::$variant(cfg) => cfg,
            _ => {
                return Err(OperationError::Other(format!(
                    "registry adapter mismatch: {} received a non-{} config",
                    $fn_name,
                    stringify!($variant)
                )));
            }
        }
    };
}

// ── Operation families ────────────────────────────────────────────────
//
// These declarations sit BELOW `config_guard!` on purpose. A `macro_rules!`
// macro has textual scope: a module declared above the definition cannot see
// it, and every family child calls `config_guard!`.

mod clearing_2d;
mod curve_engrave;
mod dressup_apply;
mod drilling;
mod findings;
mod finish_3d;
mod finish_raster;
mod shared;

pub(crate) use clearing_2d::{
    generate_adaptive, generate_face, generate_pocket, generate_profile, generate_rest,
    generate_trace, generate_zigzag,
};
pub(crate) use curve_engrave::{
    generate_chamfer, generate_inlay, generate_project_curve, generate_vcarve,
};
pub use dressup_apply::apply_dressups;
use dressup_apply::attach_generic_rest_analysis;
#[cfg(test)]
pub(crate) use drilling::STALE_DRILL_PICKS_PHRASE;
pub use drilling::{
    DRILL_PICK_MATCH_EPS_MM, NO_DRILL_TARGETS_MSG, NO_DRILL_TARGETS_SELECTED_MSG,
    build_drill_op_for_config, drill_pick_matches, drill_targets_refusal,
    stale_drill_picks_refusal,
};
pub(crate) use drilling::{generate_alignment_pin_drill, generate_drill};
pub use findings::record_boundary_clip_dropped;
pub(crate) use findings::record_offset_library_failures;
pub(crate) use finish_3d::{
    generate_adaptive3d, generate_horizontal_finish, generate_pencil, generate_radial_finish,
    generate_ramp_finish, generate_spiral_finish, generate_steep_shallow, generate_unified_finish,
};
pub(crate) use finish_raster::{generate_drop_cutter, generate_scallop, generate_waterline};

/// Execute a single operation, producing a raw toolpath.
///
/// Dispatches to the correct core algorithm based on the [`OperationConfig`]
/// variant. When `cutting_levels` is non-empty, depth-stepped operations use
/// those levels directly; otherwise they build levels from the heights config.
///
/// WP12 made this entry `pub(crate)`, which showed that every caller it has
/// left is a test. No production path in this crate calls it. The entry
/// stays, per §23 ruling 1, so the `dead_code` allow applies to the
/// non-test build only. A production caller removes the need for it.
#[allow(clippy::too_many_arguments)]
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn execute_operation(
    op: &OperationConfig,
    mesh: Option<&TriangleMesh>,
    index: Option<&SpatialIndex>,
    polygons: Option<&[Polygon2]>,
    tool_def: &ToolDefinition,
    tool_cfg: &ToolConfig,
    heights: &ResolvedHeights,
    cutting_levels: &[f64],
    stock_bbox: &BoundingBox3,
    prev_tool_radius: Option<f64>,
    reference_tool_cfg: Option<ToolConfig>,
    debug_ctx: Option<&ToolpathDebugContext>,
    cancel: &AtomicBool,
    initial_stock: Option<&crate::dexel_stock::TriDexelStock>,
) -> Result<Toolpath, OperationError> {
    execute_operation_annotated(
        op,
        mesh,
        index,
        polygons,
        tool_def,
        tool_cfg,
        heights,
        cutting_levels,
        stock_bbox,
        prev_tool_radius,
        reference_tool_cfg,
        debug_ctx,
        cancel,
        initial_stock,
        None,
        None,
    )
    .map(|at| at.toolpath)
}

/// Execute a single operation, producing a toolpath together with structural
/// spans from the underlying algorithm (when available).
///
/// This is the single source-of-truth dispatch; [`execute_operation`] delegates
/// here and discards the spans for backwards compatibility.
///
/// Thin wrapper over [`execute_operation_annotated_with_regions`] with
/// `boundary_regions = None` — kept as a stable, unchanged signature for the
/// pre-existing callers (the GUI compute worker, the strategy advisor) that
/// don't participate in the P2.3 mesh-finish pre-clip.
#[allow(clippy::too_many_arguments)]
pub(crate) fn execute_operation_annotated(
    op: &OperationConfig,
    mesh: Option<&TriangleMesh>,
    index: Option<&SpatialIndex>,
    polygons: Option<&[Polygon2]>,
    tool_def: &ToolDefinition,
    tool_cfg: &ToolConfig,
    heights: &ResolvedHeights,
    cutting_levels: &[f64],
    stock_bbox: &BoundingBox3,
    prev_tool_radius: Option<f64>,
    reference_tool_cfg: Option<ToolConfig>,
    debug_ctx: Option<&ToolpathDebugContext>,
    cancel: &AtomicBool,
    initial_stock: Option<&crate::dexel_stock::TriDexelStock>,
    semantic_ctx: Option<&ToolpathSemanticContext>,
    // `boundary`: effective machining boundary polygon (model silhouette inset
    // by tool_radius for containment=Inside). When provided, AgentSearch /
    // ContourParallel / Adaptive pre-clear cells outside this boundary in
    // their internal stock so the bool-grid polygon at every z-level reflects
    // the boundary. Without this, generation produces cuts across the full
    // stock that the post-generation toolpath clip then converts to rapids —
    // leaving stock unstamped and deeper z-levels biting through fresh
    // material with full-depth axial DOC.
    boundary: Option<&Polygon2>,
) -> Result<GeneratedToolpath, OperationError> {
    // Thin wrapper: callers on this path have no diagnostics pipeline to
    // feed, so the findings are dropped here rather than rippling a tuple
    // through every test and CLI call site.
    //
    // A/M9: dropping them is now HONEST rather than lossy-silent. Stats
    // built off this path leave `truncated_core_mm2` at `None` — "not
    // measured" — instead of the `0.0` that used to read as "nothing
    // standing". A caller that needs the finding must use
    // `execute_operation_annotated_with_regions`, which both production
    // drivers (the core session and the GUI worker) already do.
    execute_operation_annotated_with_regions(
        op,
        mesh,
        index,
        polygons,
        tool_def,
        tool_cfg,
        heights,
        cutting_levels,
        stock_bbox,
        prev_tool_radius,
        reference_tool_cfg,
        debug_ctx,
        cancel,
        initial_stock,
        semantic_ctx,
        boundary,
        None,
        None,
        None,
        // `setup_transform`: this wrapper's callers (tests, the strategy
        // advisor) hand in geometry they already resolved themselves and
        // carry no setup, so there is nothing to declare. `None` means
        // "world is the emission frame" — which is what a caller with no
        // setup is asserting anyway.
        None,
        // `drill_targets`: the same callers resolve no model, so a `Drill`
        // op on this path drills only what it carries in `selected_holes`.
        &[],
    )
    .map(|(generated, _findings)| generated)
}

/// [`execute_operation_annotated`] plus `boundary_regions` (P2.3): the
/// multi-region machining-boundary set the mesh-finish family (scallop /
/// radial / spiral / steep-shallow / ramp / horizontal / waterline)
/// pre-clips generation to. Session's `generate_toolpath` is the one caller
/// that resolves a `DerivedRestRegions` boundary and has real regions to
/// pass; every other caller passes `None` through [`execute_operation_annotated`]'s
/// unchanged signature, which is a byte-identical no-op for those families
/// (see each op's own `boundary_regions` doc comment).
///
/// Also carries `rest_analysis` (P2.5): when `Some` and `.enabled`, and the
/// dispatched op didn't already attach its own rest artifacts (pencil's
/// `RestDepth` detector arm does — see the precedence check right after
/// dispatch below), this runs the same rest-depth detector generically
/// against THIS toolpath's own tool as the fine cutter, attaching
/// `rest_grid` / `rest_regions` to the result. `None` is a byte-identical
/// no-op, same shape as `boundary_regions`.
///
/// Also carries `link_kinematics` (P1 W4a): the machine envelope the
/// pencil family costs its surface-link-vs-retract emit decision
/// against. `None` is a byte-identical no-op (legacy distance-only
/// hookup decision); production builders that have a machine profile
/// in scope (session's `generate_toolpath`) populate `Some`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn execute_operation_annotated_with_regions(
    op: &OperationConfig,
    mesh: Option<&TriangleMesh>,
    index: Option<&SpatialIndex>,
    polygons: Option<&[Polygon2]>,
    tool_def: &ToolDefinition,
    tool_cfg: &ToolConfig,
    heights: &ResolvedHeights,
    cutting_levels: &[f64],
    stock_bbox: &BoundingBox3,
    prev_tool_radius: Option<f64>,
    reference_tool_cfg: Option<ToolConfig>,
    debug_ctx: Option<&ToolpathDebugContext>,
    cancel: &AtomicBool,
    initial_stock: Option<&crate::dexel_stock::TriDexelStock>,
    semantic_ctx: Option<&ToolpathSemanticContext>,
    boundary: Option<&Polygon2>,
    boundary_regions: Option<&[Polygon2]>,
    rest_analysis: Option<&crate::compute::config::RestAnalysisConfig>,
    // P1 quantitative linker (W4a): the machine envelope the pencil
    // family's emit-time surface-link-vs-retract decision costs
    // candidates against. `None` is a byte-identical no-op — the
    // legacy distance-only hookup decision.
    link_kinematics: Option<crate::machine::kinematics::LinkKinematics>,
    // G-DRILLPICK-FRAME: the setup's world→local transform, for the
    // config-carried coordinates the driver's geometry pipeline never
    // touches. `None` = identity setup = no-op. See
    // [`ExecutionContext::setup_transform`].
    setup_transform: Option<&crate::compute::transform::SetupTransformInfo>,
    // G-DRILLCENTROID: the target model's drill targets (DXF points and
    // circle/arc centres), the `Drill` family's hole source when nothing is
    // picked. Config-carried picks and these share one frame. `&[]` for a
    // caller with no model — a `Drill` op then refuses unless it carries an
    // explicit pick.
    drill_targets: &[DrillTarget],
) -> Result<(GeneratedToolpath, GenerationFindings), OperationError> {
    // Phase-5 (T11) family adapters: when the registry carries a
    // GenerateFn for this op's family, dispatch through it. The
    // exhaustive match below remains the fallback for unmigrated
    // families AND the compile-time net (a new op variant fails to
    // compile until it has an arm — migrated arms delegate to the SAME
    // adapter fn the registry references, so the two paths cannot
    // diverge).
    //
    // This function's own signature stays slice-based (`Option<&[Polygon2]>`)
    // — both callers (session's `generate_toolpath`, the GUI worker's
    // `generate_via_core`) already resolve a plain `Vec<Polygon2>`/slice via
    // `RegionSet::processed` and pass it straight through, so changing this
    // signature to `&RegionSet` would only add a wrap/unwrap at both call
    // sites for no benefit. The borrow into `RegionSet` happens right here,
    // where it's used.
    let region_set = boundary_regions.map(RegionSet::from_slice);
    let findings = std::cell::RefCell::new(GenerationFindings::default());
    let ctx = ExecutionContext {
        findings: &findings,
        mesh,
        index,
        polygons,
        drill_targets,
        tool_def,
        tool_cfg,
        heights,
        cutting_levels,
        stock_bbox,
        setup_transform,
        prev_tool_radius,
        reference_tool_cfg,
        debug_ctx,
        cancel,
        initial_stock,
        semantic_ctx,
        boundary,
        boundary_regions: region_set.as_ref(),
        link_kinematics,
        rest_analysis,
    };
    // CMP-01: one dispatch, no fallback. The `else` branch here was a
    // 24-arm match kept as a compile net after the T11 cutover, and every
    // arm called the same adapter the registry row already holds. The net
    // is `OperationType::registry_entry`, which is exhaustive: a new
    // variant does not compile until it names a row, and a row cannot be
    // written without an adapter.
    let mut generated = (op.op_type().registry_entry().generate)(&ctx, op)?;

    // P2.5: op-agnostic rest analysis. Precedence — an op that already
    // attached its own rest artifacts (pencil's `RestDepth` detector arm,
    // via `generate_pencil`) is left alone: one source of truth per
    // toolpath, and pencil's detector is parameterized for centerline
    // extraction, a richer job than the generic pass below needs to redo.
    // Only runs when a mesh (+ its spatial index) is present — 2D ops have
    // no terrain to rest-analyze.
    if let Some(ra) = rest_analysis
        && ra.enabled
        && generated.rest_grid.is_none()
        && generated.rest_regions.is_none()
        && let (Some(m), Some(idx)) = (mesh, index)
    {
        attach_generic_rest_analysis(
            &mut generated,
            m,
            idx,
            tool_def,
            ctx.reference_tool_cfg.as_ref(),
            initial_stock,
            ra,
            &findings,
        );
    }

    Ok((generated, findings.into_inner()))
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]
mod pinned_bottom_z_reaches_motion_g_bottompin;

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]
mod project_curve_chaining;

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod unified_finish_ring_collapse_g_unifiedcrash;

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout,
    clippy::print_stderr
)]
mod unified_finish_semantic_regions;

#[cfg(test)]
mod tests;
