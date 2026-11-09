//! The raster finishing family: scallop, drop cutter and waterline.
//!
//! All three relink their fragments through the shared finishing link stage
//! before they return. Scallop and Drop Cutter run on the height-field
//! runner (`height_field.rs`, W6.3); Waterline runs on the Layered runner
//! (`layered.rs`, W6.1). Split out of `compute/execute.rs` (P4).

use std::borrow::Cow;

use crate::compute::catalog::{OperationConfig, OperationType, RestUse};
use crate::compute::operation_configs::{OpMotion, RegionOrdering};
use crate::geo::P2;
use crate::geometry::region_set::RegionSet;
use crate::interrupt::CancelCheck;
use crate::tool::ToolDefinition;
use crate::toolpath::{SidedToolpath, Toolpath};
use crate::trace::semantic_trace::ToolpathSemanticContext;

use super::height_field::{
    HeightFieldOp, HeightQuery, LinkRule, NoLabel, PlanCtx, Planned, SpanRule, run_height_field,
};
use super::layered::{LayerCtx, LayeredOp, LevelInfo, LevelRest, run_layered};
use super::shared::AfterPlanLink;
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
                feed_rate: op.cut_feeds()?.feed_rate,
                plunge_rate: op.cut_feeds()?.plunge_rate,
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

    /// Both ring sources link INSIDE the kernel: the ring annotations carry
    /// move indices that the kernel reconciles beside the relink. The
    /// runner builds the one stage, so both read the one link ceiling
    /// (`link_ceiling_for`, ruling R1).
    ///
    /// - The offset cascade runs the stage itself (reorder, loop rotation).
    /// - The iso field (SPEC §3.7) keeps the ring order: its ring list is
    ///   level sets, a different population from the cascade's. It reads
    ///   only the stage's ceiling (D1). On fresh stock the ceiling is
    ///   `None`, so its fresh output does not change.
    fn link_rule(&self) -> LinkRule {
        LinkRule::InPlan {
            hookup_mm: self.hookup_mm,
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
                pctx.link_stage.and_then(|stage| stage.link_ceiling),
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
        stepover: crate::compute::not_set::required_geometry(
            cfg.stepover,
            op.op_type().label(),
            "stepover",
        )?,
        min_z: cfg.min_z,
        stock_bottom_z: ctx.stock_bbox.min.z,
        hookup_mm: cfg.hookup_mm,
        feed_rate: op.cut_feeds()?.feed_rate,
        plunge_rate: op.cut_feeds()?.plunge_rate,
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
        feed_rate: op.cut_feeds()?.feed_rate,
        plunge_rate: op.cut_feeds()?.plunge_rate,
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
        crate::compute::not_set::required_geometry(cfg.z_step, op.op_type().label(), "z_step")?,
        ctx.stock_bbox.min.z,
    );
    // W4 (D69 §4, operator ruling 2026-10-28): on a through part (the
    // model bottom at the stock bottom, to within the ladder's own "on a
    // face" tolerance) the Waterline cuts no outer ring at or below the web
    // the job holds (`ExecutionContext::through_cut_hold`, from the
    // session). A model that stands on stock keeps every ring.
    let through_part =
        surface.mesh.bbox.min.z <= ctx.stock_bbox.min.z + crate::ops::waterline::FLAT_FACE_SNAP_MM;
    let hold = ctx.through_cut_hold.filter(|_| through_part);
    let web = hold.and_then(|h| h.web);
    let mut finding = crate::compute::toolpath_stats::WaterlineLadderFinding {
        requested_top_z_mm: ctx.heights.top_z,
        requested_bottom_z_mm: ctx.heights.bottom_z,
        stock_bottom_z_mm: ctx.stock_bbox.min.z,
        z_step_mm: crate::compute::not_set::required_geometry(
            cfg.z_step,
            op.op_type().label(),
            "z_step",
        )?,
        planned_levels: ladder.planned_levels,
        nudged_levels: ladder.nudged_levels,
        dropped_below_stock: ladder.dropped_below_stock,
        delivered_levels: ladder.levels.len(),
        web,
        outer_rings_skipped: 0,
        part_unheld: hold.is_some_and(|h| !h.tabbed_profile_follows),
    };
    ctx.record(&finding);
    // D69 §2: the helical waterline judges each link between two chained
    // rings with the model gouge guard's reduced tool. A tool that cannot
    // shrink (no cutter) joins no level: the op cuts the stacked form.
    let link_probe = if cfg.helical {
        crate::compute::cutter::build_cutter(&crate::compute::model_gouge::reduced_tool_config(
            ctx.tool_cfg,
            crate::compute::model_gouge::MODEL_GOUGE_TOL_MM,
        ))
        .ok()
    } else {
        None
    };
    // W2 (D69 §3): the inputs of the wall step. All four must be known;
    // else the helix keeps every ring (the W1 ladder).
    let wall = match (ctx.material, ctx.rest_axial_step_mm, cfg.spindle_rpm) {
        (Some(material), Some(max_step_mm), Some(rpm))
            if rpm > 0 && ctx.tool_cfg.flute_count_planning() > 0 =>
        {
            Some(WallInputs {
                material,
                max_step_mm,
                // The feed per tooth: feed / (rpm x flutes).
                fz_mm: params.feed_rate
                    / (f64::from(rpm) * f64::from(ctx.tool_cfg.flute_count_planning())),
            })
        }
        _ => None,
    };
    let pass = WaterlinePass {
        surface,
        cutter: ctx.tool_def,
        params,
        link_probe,
        wall,
        outer_floor_z: web.map(|w| w.z_mm),
        outer_rings_skipped: std::cell::Cell::new(0),
        levels: ladder.levels,
        hookup_mm: cfg.hookup_mm,
        op_type: op.op_type(),
        rest_use: op.capabilities().rest_use,
        boundary_regions: prepared.territory().regions(),
    };
    let generated = run_layered(&pass, ctx, prepared)?;
    // W4: the count is known only after the levels are cut.
    finding.outer_rings_skipped = pass.outer_rings_skipped.get();
    ctx.record(&finding);
    Ok(generated)
}

/// W2 (D69 §3): what the wall step of the helical Waterline reads.
struct WallInputs<'a> {
    material: &'a crate::material::Material,
    /// The rough's axial step (`ExecutionContext::rest_axial_step_mm`).
    max_step_mm: f64,
    /// The feed per tooth of the op (mm).
    fz_mm: f64,
}

/// W2 (D69 §3): the deflection check of a wall step. The tool `tool` cuts
/// a wall at axial depth `ap` (mm) with radial skin `skin` (mm) at feed per
/// tooth `fz` (mm). It passes when the shared force and cantilever model of
/// the deflection gate (`feeds::predict::tip_deflection_from_engagement`,
/// which `tool_load::deflection::sample_tip_deflection_mm` also reads; the
/// cantilever is `ToolDefinition::tip_deflection_mm`) predicts at most
/// `tool_load::deflection::WITHIN_BOUND_MM` (the gate's "Within, Validated"
/// bound; lead ruling 2026-10-28). No skin: no load, so it passes. A model
/// refusal (no force line, no stickout) fails.
fn wall_step_admits(
    tool: &ToolDefinition,
    material: &crate::material::Material,
    skin: f64,
    ap: f64,
    fz: f64,
) -> bool {
    if skin <= 0.0 {
        return true;
    }
    // P11a: the one engagement of the wall step, at the engaged radius of
    // its depth (the open P9 diameters).
    let engagement = crate::feeds::engagement::Engagement::from_tool_cut(
        crate::feeds::envelope::CutPoint {
            ap_mm: ap,
            ae_mm: skin,
        },
        tool,
    );
    crate::feeds::predict::tip_deflection_from_engagement(tool, material, &engagement, fz)
        .is_some_and(|deflection| deflection <= crate::tool_load::deflection::WITHIN_BOUND_MM)
}

impl WaterlinePass<'_> {
    /// W2 (D69 §3): [`wall_step_admits`] along `ring`, with the widest
    /// radial skin that the remaining stock shows there
    /// (`StandingNear::radial_skin_mm`). No rest rule (fresh stock): fails.
    fn wall_admits(
        &self,
        wall: &WallInputs<'_>,
        standing: Option<&crate::compute::execute::standing_material::StandingNear<'_>>,
        ring: &[crate::geo::P3],
        ap: f64,
    ) -> bool {
        let Some(standing) = standing else {
            return false;
        };
        let radius = crate::tool::MillingCutter::radius(self.cutter);
        let skin = ring
            .iter()
            .map(|p| standing.radial_skin_mm(P2::new(p.x, p.y), radius))
            .fold(0.0_f64, f64::max);
        wall_step_admits(self.cutter, wall.material, skin, ap, wall.fz_mm)
    }
}

/// The Waterline op: the push-cutter contours of the surface at each level
/// of its ladder. The Z-independent geometry is the surface itself; each
/// level queries it (W6.1).
struct WaterlinePass<'a> {
    surface: MeshInput<'a>,
    cutter: &'a ToolDefinition,
    params: crate::ops::waterline::WaterlineParams,
    /// D69 §2 (`WaterlineConfig::helical`): the reduced tool that
    /// judges a chain link. `Some`: the op cuts the helical waterline;
    /// `None`: the stacked form, one entry per ring and level.
    link_probe: Option<ToolDefinition>,
    /// W2 (D69 §3): the inputs of the wall step, or `None` (no material, no
    /// rough step or no spindle speed): the helix keeps every ring.
    wall: Option<WallInputs<'a>>,
    /// W4 (D69 §4): the web Z of a through cut (`RunFilter::outer_floor_z`),
    /// or `None`.
    outer_floor_z: Option<f64>,
    /// W4: the outer rings the web kept from the cut, over every level.
    outer_rings_skipped: std::cell::Cell<usize>,
    /// The ladder of `waterline_ladder`: nudged off the flat facets and
    /// floored at the stock bottom.
    levels: Vec<f64>,
    /// The cap (mm) of the after-plan link stage. `0.0`: no stage.
    hookup_mm: f64,
    op_type: OperationType,
    rest_use: RestUse,
    /// The boundary regions of the contour split (the territory's XY
    /// part).
    boundary_regions: Option<&'a RegionSet<'a>>,
}

impl<'a> LayeredOp for WaterlinePass<'a> {
    type Input = MeshInput<'a>;
    /// The surfaces of one region: the one surface.
    type Geometry = Vec<MeshInput<'a>>;
    /// The surface needs no Z-free work, so a prepare measures nothing.
    type Report = ();

    /// Waterline offsets no polygon, so it claims no offset zero.
    const MEASURES_OFFSETS: bool = false;

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
        _prepared: &'p Prepared<'p>,
    ) -> Result<Cow<'p, [MeshInput<'a>]>, OperationError>
    where
        MeshInput<'a>: 'p,
    {
        Ok(Cow::Borrowed(std::slice::from_ref(&self.surface)))
    }

    fn ladder(&self, _ctx: &ExecutionContext<'_>) -> Result<Vec<f64>, OperationError> {
        Ok(self.levels.clone())
    }

    fn prepare(
        &self,
        region: &[MeshInput<'a>],
        _layer: &LayerCtx<'_, MeshInput<'a>>,
    ) -> Result<(Self::Geometry, Self::Report), OperationError> {
        Ok((region.to_vec(), ()))
    }

    /// The contours at `level.z`, split by the boundary regions and, on a
    /// rest pass, by the runner's `ContourFilter` rule at the level
    /// ([`LevelRest`], W6.1). A stretch in air is dropped only when the
    /// retract saves time.
    fn emit_level(
        &self,
        geometry: &Self::Geometry,
        level: LevelInfo,
        rest: &LevelRest<'_>,
        cancel: &dyn CancelCheck,
    ) -> Result<SidedToolpath, OperationError> {
        let standing = rest.standing.as_ref();
        let keep = |p: P2| standing.is_none_or(|s| s.keeps(p));
        let filter = crate::ops::waterline::RunFilter {
            boundary_regions: self.boundary_regions,
            keep: standing.is_some().then_some(&keep as &dyn Fn(P2) -> bool),
            split_timing: rest.split_timing,
            outer_floor_z: self.outer_floor_z,
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
            self.outer_rings_skipped
                .set(self.outer_rings_skipped.get() + piece.outer_rings_skipped);
            tp.moves.extend(piece.toolpath.moves);
        }
        rest.tally(kept, dropped);
        // P6.0c: a surface op states no material side (the lead dressup's
        // surface probe holds the lead).
        Ok(SidedToolpath::without_closed_passes(tp))
    }

    /// D69 §2: the helical waterline. The rings of every level, with the
    /// rest rule and its tally of [`Self::emit_level`], then each chain of
    /// whole rings over consecutive levels as one entry
    /// (`ops::waterline::helical_waterline`). `None` when the dial is off.
    fn emit_ladder<'r>(
        &self,
        geometry: &Self::Geometry,
        levels: &[f64],
        rest: &dyn Fn(f64) -> LevelRest<'r>,
        cancel: &dyn CancelCheck,
    ) -> Option<Result<SidedToolpath, OperationError>> {
        let reduced_tool = self.link_probe.as_ref()?;
        let probe = crate::ops::waterline::ChainLinkProbe {
            mesh: self.surface.mesh,
            index: self.surface.index,
            reduced_tool,
        };
        let run = || -> Result<SidedToolpath, OperationError> {
            let mut rings = Vec::with_capacity(levels.len());
            for &z in levels {
                crate::interrupt::check_cancel(cancel)?;
                let rest = rest(z);
                let standing = rest.standing.as_ref();
                let keep = |p: P2| standing.is_none_or(|s| s.keeps(p));
                let filter = crate::ops::waterline::RunFilter {
                    boundary_regions: self.boundary_regions,
                    keep: standing.is_some().then_some(&keep as &dyn Fn(P2) -> bool),
                    split_timing: rest.split_timing,
                    outer_floor_z: self.outer_floor_z,
                };
                let mut level = crate::ops::waterline::LevelRings {
                    contours: Vec::new(),
                    runs: Vec::new(),
                    rest_kept: false,
                    rest_dropped: false,
                    outer_rings_skipped: 0,
                };
                for surface in geometry {
                    let piece = crate::ops::waterline::waterline_level_rings(
                        surface.mesh,
                        surface.index,
                        self.cutter,
                        z,
                        &self.params,
                        &filter,
                        cancel,
                    )?;
                    let offset = level.contours.len();
                    level.contours.extend(piece.contours);
                    level.runs.extend(piece.runs.into_iter().map(|mut run| {
                        run.contour += offset;
                        run
                    }));
                    level.rest_kept |= piece.rest_kept;
                    level.rest_dropped |= piece.rest_dropped;
                    level.outer_rings_skipped += piece.outer_rings_skipped;
                }
                rest.tally(level.rest_kept, level.rest_dropped);
                self.outer_rings_skipped
                    .set(self.outer_rings_skipped.get() + level.outer_rings_skipped);
                rings.push(level);
            }
            let admits = |k: usize, ring: &[crate::geo::P3], ap: f64| {
                let Some((wall, &z)) = self.wall.as_ref().zip(levels.get(k)) else {
                    return false;
                };
                let rest = rest(z);
                self.wall_admits(wall, rest.standing.as_ref(), ring, ap)
            };
            let wall_step = self
                .wall
                .as_ref()
                .map(|wall| crate::ops::waterline::WallStep {
                    max_step_mm: wall.max_step_mm,
                    admits: &admits,
                });
            let tp = crate::ops::waterline::helical_waterline(
                &rings,
                &self.params,
                &probe,
                wall_step.as_ref(),
                // P22-0c-2: each descending lap holds the core dwell rule.
                crate::tool::body::core_of(self.cutter),
                cancel,
            )?;
            // P6.0c: a surface op states no material side.
            Ok(SidedToolpath::without_closed_passes(tp))
        };
        Some(run())
    }

    /// The depth-run spans, and (D69 §2) a barrier round each helical
    /// chain, so the rapid-order pass keeps the chain in its place.
    fn spans(&self, toolpath: Toolpath, levels: &[f64]) -> GeneratedToolpath {
        let mut spans = crate::compute::spans::spans_from_depth_runs(&toolpath, levels);
        spans.extend(crate::compute::spans::chain_barriers(&toolpath, levels));
        super::shared::generated_with_spans(toolpath, spans)
    }

    /// G-LINKSTAGE, OFF by default (`hookup_mm` ships at `0.0`), so this
    /// family's fingerprint does not move. The runner runs the one
    /// after-plan link (`shared::link_after_plan`).
    ///
    /// No fragment kind is declared yet. A waterline level IS a closed loop
    /// and would benefit from rotation, but this stage cannot see which of
    /// the emitted fragments are whole levels and which are arcs the
    /// boundary split, and a mis-declared kind rotates the wrong fragment.
    /// Declaring them in `ops::waterline::waterline_level`, the way the
    /// scallop does, is the follow-up.
    fn after_plan_link(&self) -> Option<AfterPlanLink<'_>> {
        Some(AfterPlanLink {
            op_type: self.op_type,
            hookup_mm: self.hookup_mm,
            geometry: crate::finish::surface_link::LinkGeometry {
                stock_to_leave: self.params.stock_to_leave,
                sampling: self.params.sampling.max(0.01),
                feed_rate: self.params.feed_rate,
                plunge_rate: self.params.plunge_rate,
                safe_z: self.params.safe_z,
                // `WaterlineConfig` has no path tolerance: the link is
                // not refined (G-TIERBURIAL, open).
                link_tolerance: None,
            },
            surface: self.surface,
            cutter: self.cutter,
        })
    }
}

#[cfg(test)]
#[expect(
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]
mod tests {
    use super::wall_step_admits;
    use crate::compute::tool_config::ToolFact;
    use crate::compute::tool_config::{ToolConfig, ToolType};
    use crate::ids::ToolId;
    use crate::material::{Material, WoodSpecies};

    fn endmill(diameter: f64, stickout: f64) -> crate::tool::ToolDefinition {
        let mut cfg = ToolConfig::test_default(ToolId::from_raw(0), ToolType::EndMill);
        cfg.diameter = diameter;
        cfg.set_shank_diameter(ToolFact::Stated(diameter));
        cfg.set_stickout(ToolFact::Stated(stickout));
        crate::compute::cutter::build_cutter(&cfg).expect("a valid end mill")
    }

    /// W2: the wall step passes the deflection check for a stiff tool and
    /// a thin skin, and fails for a long thin tool at the same cut. No skin
    /// always passes.
    #[test]
    fn the_wall_step_reads_the_deflection_model_w2() {
        let maple = Material::SolidWood {
            species: WoodSpecies::HardMaple,
        };
        let (skin, ap, fz) = (0.5, 6.0, 0.05);
        let stiff = endmill(6.0, 20.0);
        let thin = endmill(2.0, 40.0);
        assert!(wall_step_admits(&stiff, &maple, skin, ap, fz));
        assert!(!wall_step_admits(&thin, &maple, skin, ap, fz));
        assert!(wall_step_admits(&thin, &maple, 0.0, ap, fz));
    }
}
