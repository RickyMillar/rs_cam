//! The raster finishing family: scallop, drop cutter and waterline.
//!
//! All three relink their fragments through the shared finishing link stage
//! before they return. Scallop and Drop Cutter run on the height-field
//! runner (`height_field.rs`, W6.3); Waterline runs on the Layered runner
//! (`layered.rs`, W6.1). Split out of `compute/execute.rs` (P4).

use std::borrow::Cow;
use std::cell::Cell;

use crate::compute::catalog::{OperationConfig, OperationType, RestUse};
use crate::compute::operation_configs::{OpMotion, RegionOrdering};
use crate::compute::toolpath_stats::RestLevelsFinding;
use crate::geo::P2;
use crate::interrupt::CancelCheck;
use crate::tool::{MillingCutter, ToolDefinition};
use crate::toolpath::Toolpath;
use crate::trace::semantic_trace::ToolpathSemanticContext;

use super::height_field::{
    HeightFieldOp, HeightQuery, LinkRule, NoLabel, PlanCtx, Planned, SpanRule, run_height_field,
};
use super::layered::{LayerCtx, LayeredOp, LevelInfo, LevelStock, run_layered};
use super::shared::{finishing_link_stage, relink_in_adapter};
use super::standing_material::standing_near;
use super::territory::Territory;
use super::{
    ExecutionContext, GeneratedToolpath, MeshInput, OperationError, Prepared, SurfaceMask,
};

/// Scallop family adapter (W6.3): [`ScallopPass`] on the height-field
/// runner. The dispatch prologue refuses a tool the registry row does not
/// admit (W2.2). The kernel polls the cancel (pinned by
/// `every_generator_polls_a_preset_cancel_flag_past_the_prologue`).
pub(crate) fn generate_scallop(
    ctx: &ExecutionContext<'_>,
    prepared: &Prepared<'_>,
    op: &OperationConfig,
) -> Result<GeneratedToolpath, OperationError> {
    let cfg = config_guard!(op, Scallop, "generate_scallop");
    let pass = ScallopPass {
        params: cfg.params(
            OpMotion {
                feed_rate: op.feed_rate(),
                plunge_rate: op.plunge_rate(),
                safe_z: ctx.heights.retract_z,
            },
            ctx.link_kinematics.clone(),
        ),
        iso_field: cfg.iso_field,
        hookup_mm: cfg.intra_pass_hookup_mm,
    };
    run_height_field(&pass, ctx, prepared)
}

/// The Scallop op: constant-scallop rings, from the offset cascade or the
/// iso field. The territory regions are a planner INPUT: each region gets
/// its own ring set, and the keep predicate tests them again.
struct ScallopPass {
    params: crate::finish::scallop::ScallopParams,
    iso_field: bool,
    hookup_mm: f64,
}

impl HeightFieldOp for ScallopPass {
    type Annotation = crate::finish::scallop::ScallopRuntimeAnnotation;
    /// The cascade residual, and the link totals when the stage ran. The
    /// kernel decides that, so a hookup above zero on a `continuous` pass,
    /// or one whose cascade made no ring, measured nothing.
    type Report = crate::finish::scallop::ScallopReport;

    fn op_type(&self) -> OperationType {
        OperationType::Scallop
    }

    /// The offset cascade IS the op §2 of the linking spec measured
    /// (breadth-first ring order, no reorder, no loop rotation, no stock
    /// ceiling), and it opts in. It links INSIDE its kernel: the ring
    /// annotations carry move indices that the kernel reconciles beside the
    /// relink.
    ///
    /// The iso field is NOT on the shared stage (SPEC §3.7). Its ring list
    /// is level sets, a different population from the cascade's, and its
    /// fingerprint is pinned by `scallop_isofield_gouge_m4` and the
    /// multitool tiers. It keeps the legacy relink at the same
    /// `intra_pass_hookup_mm`, byte for byte.
    fn link_rule(&self) -> LinkRule {
        if self.iso_field {
            LinkRule::Off
        } else {
            LinkRule::InPlan {
                hookup_mm: self.hookup_mm,
            }
        }
    }

    fn span_rule(&self) -> SpanRule {
        SpanRule::LabeledEvents
    }

    fn annotate(
        &self,
        annotations: &[Self::Annotation],
        toolpath: &Toolpath,
        sem: &ToolpathSemanticContext,
    ) {
        crate::compute::annotate::annotate_scallop(
            annotations,
            toolpath,
            sem,
            crate::compute::annotate::ScallopRegionGrouping::ByBoundaryRegion,
        );
    }

    fn plan(
        &self,
        query: HeightQuery<'_>,
        territory: &Territory<'_>,
        pctx: &PlanCtx<'_>,
    ) -> Result<Planned<Self::Annotation, Self::Report>, OperationError> {
        let (surface, cutter) = query.mesh(self.op_type())?;
        let regions = territory.regions();
        let (tp, annotations, report) = if self.iso_field {
            // M8 iso-field rings: per-point spacing, cosine slope law,
            // completion by construction. See the kernel's doc for evidence.
            crate::finish::scallop::scallop_toolpath_iso_field_with_cancel(
                surface.mesh,
                surface.index,
                cutter,
                &self.params,
                pctx.debug,
                regions,
                pctx.cancel,
            )?
        } else {
            crate::finish::scallop::scallop_toolpath_structured_annotated_with_cancel_and_stage(
                surface.mesh,
                surface.index,
                cutter,
                &self.params,
                pctx.debug,
                regions,
                pctx.link_stage,
                pctx.cancel,
            )?
        };
        Ok(Planned::new(tp, report).with_annotations(annotations))
    }
}

/// DropCutter family adapter (W6.3): [`DropCutterPass`] on the height-field
/// runner. The grid sampler polls the cancel (pinned by
/// `every_generator_polls_a_preset_cancel_flag_past_the_prologue`).
pub(crate) fn generate_drop_cutter(
    ctx: &ExecutionContext<'_>,
    prepared: &Prepared<'_>,
    op: &OperationConfig,
) -> Result<GeneratedToolpath, OperationError> {
    let cfg = config_guard!(op, DropCutter, "generate_drop_cutter");
    let pass = DropCutterPass {
        stepover: cfg.stepover,
        min_z: cfg.min_z,
        stock_bottom_z: ctx.stock_bbox.min.z,
        hookup_mm: cfg.hookup_mm,
        feed_rate: op.feed_rate(),
        plunge_rate: op.plunge_rate(),
        safe_z: ctx.heights.retract_z,
    };
    run_height_field(&pass, ctx, prepared)
}

/// The Drop Cutter op: a raster over the drop-cutter grid, filtered by the
/// slope band of the territory when the band is active.
struct DropCutterPass {
    stepover: f64,
    min_z: f64,
    stock_bottom_z: f64,
    hookup_mm: f64,
    feed_rate: f64,
    plunge_rate: f64,
    safe_z: f64,
}

impl HeightFieldOp for DropCutterPass {
    type Annotation = NoLabel;
    type Report = ();

    fn op_type(&self) -> OperationType {
        OperationType::DropCutter
    }

    /// G-LINKSTAGE, OFF by default (`hookup_mm` ships at `0.0`), so this
    /// op's fingerprint does not move until an operator asks for it.
    ///
    /// Why it is wired at all: the raster's only linker is the serpentine
    /// hookup in `toolpath.rs`, whose cap is one grid diagonal, so two runs
    /// of one row split by an excluded cell (two steps apart) cannot join.
    /// On the wanaka island raster that is 953 row fragments and 954
    /// retracts (`planning/linking_2026-09-09/SPEC.md` §1). Raster rows are
    /// OPEN runs, so no fragment kind is declared: a reversed row flips its
    /// cut direction, and rotation does not apply.
    fn link_rule(&self) -> LinkRule {
        LinkRule::AfterPlan {
            hookup_mm: self.hookup_mm,
        }
    }

    fn span_rule(&self) -> SpanRule {
        SpanRule::CutRuns("Raster row")
    }

    fn plan(
        &self,
        query: HeightQuery<'_>,
        territory: &Territory<'_>,
        pctx: &PlanCtx<'_>,
    ) -> Result<Planned<NoLabel, ()>, OperationError> {
        let (surface, cutter) = query.mesh(self.op_type())?;
        let (grid, floor_z) = crate::surface::dropcutter::finish_raster_grid(
            surface.mesh,
            surface.index,
            cutter,
            self.stepover,
            self.min_z,
            self.stock_bottom_z,
            pctx.cancel,
        )?;
        // The raster drops the points at the floor (off the mesh), so the
        // finish never cuts past the mesh edge.
        let min_z_filter = Some(floor_z);
        let regions = territory.regions();
        // W6.2: the territory holds the slope band only when it is active
        // (`OperationConfig::surface_mask`).
        let tp = if let Some(SurfaceMask::SlopeBand { from_deg, to_deg }) = territory.mask {
            // P1.3 (planning/finishing_stack_review_2026-07.md): the slope
            // filter reads `SlopeMap` (radians + normals + curvature). The
            // grid is always sampled at `direction_deg = 0.0`, so its
            // sampling frame is the world frame and its square cells read
            // straight into `SlopeMap::from_z_grid`. The degree bounds of
            // the config become radians here: `SlopeMap::angles` is radians.
            debug_assert!(
                (grid.x_step - grid.y_step).abs() < 1e-9,
                "drop_cutter grid must have square cells for SlopeMap conversion"
            );
            let z_values: Vec<f64> = grid.points.iter().map(|cl| cl.z).collect();
            let slope_map = crate::surface::slope::SlopeMap::from_z_grid(
                &z_values,
                grid.rows,
                grid.cols,
                grid.u_start,
                grid.v_start,
                grid.x_step,
            );
            crate::toolpath::raster_toolpath_from_grid_with_slope_filter(
                &grid,
                &slope_map.angles,
                from_deg.to_radians(),
                to_deg.to_radians(),
                self.feed_rate,
                self.plunge_rate,
                self.safe_z,
                min_z_filter,
                crate::toolpath::MoveIntent::FinishingCut,
                regions,
            )
        } else {
            crate::toolpath::raster_toolpath_from_grid(
                &grid,
                self.feed_rate,
                self.plunge_rate,
                self.safe_z,
                min_z_filter,
                regions,
            )
        };
        Ok(
            Planned::new(tp, ()).with_link_geometry(crate::finish::surface_link::LinkGeometry {
                // The raster rides the drop-cutter grid itself, so there is
                // no crest to stand off from and no leave dial on this op.
                stock_to_leave: 0.0,
                sampling: grid.x_step.max(0.01),
                feed_rate: self.feed_rate,
                plunge_rate: self.plunge_rate,
                safe_z: self.safe_z,
                // `DropCutterConfig` has no path tolerance: the link is not
                // refined (G-TIERBURIAL, open).
                link_tolerance: None,
            }),
        )
    }
}

/// Waterline family adapter (W6.1): [`WaterlinePass`] on the Layered
/// runner. The runner polls the cancel before each level, and the push
/// cutter polls it between its fiber chunks (pinned by
/// `every_generator_polls_a_preset_cancel_flag_past_the_prologue`).
pub(crate) fn generate_waterline(
    ctx: &ExecutionContext<'_>,
    prepared: &Prepared<'_>,
    op: &OperationConfig,
) -> Result<GeneratedToolpath, OperationError> {
    let cfg = config_guard!(op, Waterline, "generate_waterline");
    let surface = prepared.mesh()?;
    let params = crate::ops::waterline::WaterlineParams {
        sampling: cfg.sampling,
        feed_rate: op.feed_rate(),
        plunge_rate: op.plunge_rate(),
        safe_z: ctx.heights.retract_z,
        // `WaterlineConfig` exposes no stock-to-leave dial, so the
        // standalone op has an ABSENT capability, not a dropped one: no
        // operator can set a value here and get nothing back. Same
        // disposition as standalone DropCutter
        // (`FINISHING_OPEN_DEFECTS_EVIDENCE.md` §3.C).
        stock_to_leave: 0.0,
    };
    // R3 / R4 (Corne case, 2026-09-18): the ladder is built ONCE, here, and
    // the same `Vec` is the runner's ladder: it cuts the levels and spans
    // them. `waterline_ladder` nudges a level off a horizontal facet (§4.5
    // of the analysis: levels at 18.0 and 15.0 wove junk across the wall
    // top and the notch floor) and drops every level at or below the
    // emission-frame stock bottom (a level AT the stock bottom cuts the
    // bed). The as-found `-1.37` pin therefore yields an EMPTY ladder plus
    // the finding, never a cut below the stock. The resolved heights
    // themselves are the R3 half: an Auto bottom on this zero-depth op is
    // now the model bottom (`HeightsConfig::auto_bottom_z`), so the Auto
    // ladder spans the model.
    let ladder = crate::ops::waterline::waterline_ladder(
        surface.mesh,
        ctx.heights.top_z,
        ctx.heights.bottom_z,
        cfg.z_step,
        ctx.stock_bbox.min.z,
    );
    ctx.record(&crate::compute::toolpath_stats::WaterlineLadderFinding {
        requested_top_z_mm: ctx.heights.top_z,
        requested_bottom_z_mm: ctx.heights.bottom_z,
        stock_bottom_z_mm: ctx.stock_bbox.min.z,
        z_step_mm: cfg.z_step,
        planned_levels: ladder.planned_levels,
        nudged_levels: ladder.nudged_levels,
        dropped_below_stock: ladder.dropped_below_stock,
        delivered_levels: ladder.levels.len(),
    });
    let pass = WaterlinePass {
        surface,
        cutter: ctx.tool_def,
        params,
        levels: ladder.levels,
        z_step: cfg.z_step,
        hookup_mm: cfg.hookup_mm,
        op_type: op.op_type(),
        rest_use: op.capabilities().rest_use,
        territory: prepared.territory(),
        link_kinematics: ctx.link_kinematics.clone(),
        rest_tally: Cell::new(None),
    };
    let generated = run_layered(&pass, ctx, prepared)?;
    // G-RESTLEVELS: the per-level tally of the contour filter. `None` (no
    // record) when the filter did not run: a fresh op, or a rule that is
    // not `ContourFilter`.
    ctx.record(&pass.rest_tally.get());
    Ok(generated)
}

/// The Waterline op: the push-cutter contours of the surface at each level
/// of its ladder. The Z-independent geometry is the surface itself; each
/// level queries it (W6.1).
struct WaterlinePass<'a> {
    surface: MeshInput<'a>,
    cutter: &'a ToolDefinition,
    params: crate::ops::waterline::WaterlineParams,
    /// The ladder of `waterline_ladder`: nudged off the flat facets and
    /// floored at the stock bottom.
    levels: Vec<f64>,
    z_step: f64,
    hookup_mm: f64,
    op_type: OperationType,
    rest_use: RestUse,
    /// The op's territory: the boundary regions of the contour split and
    /// of the link stage, and the stock of the link ceiling (C3).
    territory: &'a Territory<'a>,
    /// The machine envelope that times a stretch the contour filter drops
    /// (`machine::kinematics::retract_saves_time`). `None`: the filter
    /// drops nothing.
    link_kinematics: Option<crate::machine::kinematics::LinkKinematics>,
    /// The `ContourFilter` tally over the levels: a level whose filter
    /// dropped every point is skipped, and one that dropped some points is
    /// clipped. `None` until the filter runs.
    rest_tally: Cell<Option<RestLevelsFinding>>,
}

impl<'a> LayeredOp for WaterlinePass<'a> {
    type Input = MeshInput<'a>;
    /// The surfaces of one region: the one surface.
    type Geometry = Vec<MeshInput<'a>>;
    /// The surface needs no Z-free work, so a prepare measures nothing.
    type Report = ();

    /// Waterline offsets no polygon, so it claims no offset zero.
    const MEASURES_OFFSETS: bool = false;

    /// The Z step. [`Self::ladder`] reads the ladder built from it.
    fn depth_per_pass(&self) -> f64 {
        self.z_step
    }

    /// One surface, so the order changes nothing.
    fn region_ordering(&self) -> RegionOrdering {
        RegionOrdering::ByArea
    }

    fn rest_use(&self) -> RestUse {
        self.rest_use
    }

    /// The one input: the surface the prologue checked.
    fn outlines<'p>(
        &'p self,
        _prepared: &Prepared<'p>,
    ) -> Result<Cow<'p, [MeshInput<'a>]>, OperationError>
    where
        MeshInput<'a>: 'p,
    {
        Ok(Cow::Borrowed(std::slice::from_ref(&self.surface)))
    }

    fn ladder(&self, _ctx: &ExecutionContext<'_>) -> Vec<f64> {
        self.levels.clone()
    }

    fn prepare(
        &self,
        region: &[MeshInput<'a>],
        _layer: &LayerCtx<'_, MeshInput<'a>>,
    ) -> Result<(Self::Geometry, Self::Report), OperationError> {
        Ok((region.to_vec(), ()))
    }

    /// The contours at `level.z`, split by the boundary regions and, for
    /// a rest op whose rule is `ContourFilter`, by the material that stands
    /// at the level (`standing_material::standing_near`, W6.1). A stretch
    /// in air is dropped only when the retract saves time.
    fn emit_level(
        &self,
        geometry: &Self::Geometry,
        level: LevelInfo,
        stock: &LevelStock<'_>,
        cancel: &dyn CancelCheck,
    ) -> Result<Toolpath, OperationError> {
        let standing = match self.rest_use {
            RestUse::ContourFilter => standing_near(stock.remaining, level.z, self.cutter.radius()),
            RestUse::None
            | RestUse::LevelClip
            | RestUse::LevelSkip
            | RestUse::LevelMask
            | RestUse::StockColumn
            | RestUse::Planner
            | RestUse::Reference
            | RestUse::LinkSafetyOnly => None,
        };
        let keep = |p: P2| standing.as_ref().is_none_or(|s| s.keeps(p));
        let filter = crate::ops::waterline::RunFilter {
            boundary_regions: self.territory.regions(),
            keep: standing.is_some().then_some(&keep as &dyn Fn(P2) -> bool),
            split_timing: self.link_kinematics.as_ref(),
        };
        let mut tp = Toolpath::new();
        let (mut kept, mut dropped) = (false, false);
        for surface in geometry {
            let piece = crate::ops::waterline::waterline_level(
                surface.mesh,
                surface.index,
                self.cutter,
                level.z,
                &self.params,
                &filter,
                cancel,
            )?;
            kept |= piece.rest_kept;
            dropped |= piece.rest_dropped;
            tp.moves.extend(piece.toolpath.moves);
        }
        if standing.is_some() {
            let mut tally = self.rest_tally.get().unwrap_or(RestLevelsFinding {
                planned_levels: 0,
                skipped_levels: 0,
                clipped_levels: 0,
                fallback: None,
            });
            tally.planned_levels += 1;
            match (kept, dropped) {
                (false, true) => tally.skipped_levels += 1,
                (true, true) => tally.clipped_levels += 1,
                (_, false) => {}
            }
            self.rest_tally.set(Some(tally));
        }
        Ok(tp)
    }

    /// G-LINKSTAGE, OFF by default (`hookup_mm` ships at `0.0`), so this
    /// family's fingerprint does not move.
    ///
    /// No fragment kind is declared yet. A waterline level IS a closed loop
    /// and would benefit from rotation, but this stage cannot see which of
    /// the emitted fragments are whole levels and which are arcs the
    /// boundary split, and a mis-declared kind rotates the wrong fragment.
    /// Declaring them in `ops::waterline::waterline_level`, the way the
    /// scallop does, is the follow-up.
    /// G-LINKVISIBLE: same contract as the raster's — an unrun stage stays
    /// `None` on the stats channel.
    fn link(&self, toolpath: Toolpath, ctx: &ExecutionContext<'_>) -> Toolpath {
        let Some(stage) = finishing_link_stage(self.op_type, ctx, self.territory, self.hookup_mm)
        else {
            return toolpath;
        };
        let (linked, totals) = relink_in_adapter(
            &stage,
            &crate::finish::surface_link::LinkGeometry {
                stock_to_leave: self.params.stock_to_leave,
                sampling: self.params.sampling.max(0.01),
                feed_rate: self.params.feed_rate,
                plunge_rate: self.params.plunge_rate,
                safe_z: self.params.safe_z,
                // `WaterlineConfig` has no path tolerance: the link is
                // not refined (G-TIERBURIAL, open).
                link_tolerance: None,
            },
            self.surface.mesh,
            self.surface.index,
            self.cutter,
            toolpath,
            "waterline",
        );
        ctx.record(&totals);
        linked
    }
}
// ── Public API ────────────────────────────────────────────────────────
