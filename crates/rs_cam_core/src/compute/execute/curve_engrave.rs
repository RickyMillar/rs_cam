//! The curve and V-bit engraving family: inlay, V-carve, chamfer and
//! projected curve.
//!
//! These adapters cut along a curve rather than clearing an area, so they
//! share the V-bit half-angle read and the projected-curve chaining pass.
//! Chamfer runs on the Layered runner (`layered.rs`, W3.5): one level from
//! the tool angle, and the `LevelSkip` rest rule.
//! Split out of `compute/execute.rs` (P4).

use crate::compute::catalog::{OperationConfig, RestUse};
use crate::compute::cutter::build_cutter;
use crate::compute::operation_configs::RegionOrdering;
use crate::compute::tool_config::ToolConfig;
use crate::geo::P2;
use crate::geometry::region_set::RegionSet;
use crate::interrupt::CancelCheck;
use crate::polygon::Polygon2;
use crate::tool::{MillingCutter, ToolDefinition};
use crate::toolpath::Toolpath;
use crate::trace::toolpath_spans::AnnotatedToolpath;
use crate::trace::transform_provenance::ReconcileSet;

use super::layered::{LayerCtx, LayeredOp, LevelInfo, LevelStock, ReachBand, run_layered};
use super::shared::{generated_with_cut_run_spans, op_link_ceiling, with_depth_run_annotation};
use super::{
    ExecutionContext, GeneratedToolpath, MeshInput, OffsetFailures, OperationError, Prepared,
};

/// Inlay adapter: the V walls of the female pocket only (ruling R2). The
/// plug is [`generate_inlay_plug`], in its own project. Cancellable: the
/// cooperative cancel closure is built by `ctx.cancel_fn()` (pinned by
/// `every_generator_polls_a_preset_cancel_flag_past_the_prologue`).
pub(crate) fn generate_inlay(
    ctx: &ExecutionContext<'_>,
    prepared: &Prepared<'_>,
    op: &OperationConfig,
) -> Result<GeneratedToolpath, OperationError> {
    let cfg = config_guard!(op, Inlay, "generate_inlay");
    let polys = prepared.polygons()?;
    let ha = vbit_half_angle(ctx.tool_cfg);
    let safe_z = ctx.heights.retract_z;
    let cancel_fn = ctx.cancel_fn();
    let mut out = Toolpath::new();
    for poly in polys {
        let tp = crate::ops::inlay::inlay_pocket_toolpath_with_cancel(
            poly,
            &crate::ops::inlay::InlayPocketParams {
                half_angle: ha,
                pocket_depth: cfg.pocket_depth,
                floor_overlap: cfg.floor_overlap,
                floor_ridge: cfg.floor_ridge,
                // `ToolConfig` holds no tip flat for a V-bit: a sharp tip.
                tip_flat: 0.0,
                stepover: cfg.stepover,
                feed_rate: op.feed_rate(),
                plunge_rate: op.plunge_rate(),
                safe_z,
                tolerance: cfg.tolerance,
                top_z: ctx.heights.top_z,
            },
            &cancel_fn,
        )?;
        out.moves.extend(tp.moves);
    }
    Ok(with_depth_run_annotation(
        generated_with_cut_run_spans(out, "Inlay run"),
        ctx.semantic_ctx,
    ))
}

/// Inlay plug adapter: the mirrored male plug of the whole design, on the
/// plug board of its own project (ruling R2). All polygons go to the
/// kernel at once, so one polygon is never cut as the background of
/// another. Refuses a glue gap that is not smaller than the pocket depth.
/// Cancellable as [`generate_inlay`].
pub(crate) fn generate_inlay_plug(
    ctx: &ExecutionContext<'_>,
    prepared: &Prepared<'_>,
    op: &OperationConfig,
) -> Result<GeneratedToolpath, OperationError> {
    let cfg = config_guard!(op, InlayPlug, "generate_inlay_plug");
    let polys = prepared.polygons()?;
    let ha = vbit_half_angle(ctx.tool_cfg);
    if cfg.start_depth() <= 0.0 {
        return Err(OperationError::Other(format!(
            "InlayPlug: glue_gap {} mm must be smaller than pocket_depth {} mm",
            cfg.glue_gap, cfg.pocket_depth
        )));
    }
    let safe_z = ctx.heights.retract_z;
    let cancel_fn = ctx.cancel_fn();
    let out = crate::ops::inlay::inlay_plug_toolpath_with_cancel(
        polys,
        &crate::ops::inlay::InlayPlugParams {
            half_angle: ha,
            pocket_depth: cfg.pocket_depth,
            glue_gap: cfg.glue_gap,
            proud_height: cfg.proud_height,
            outer_margin: cfg.outer_margin,
            depth_per_pass: cfg.depth_per_pass,
            stepover: cfg.stepover,
            feed_rate: op.feed_rate(),
            plunge_rate: op.plunge_rate(),
            safe_z,
            tolerance: cfg.tolerance,
            top_z: ctx.heights.top_z,
        },
        &cancel_fn,
    )?;
    Ok(with_depth_run_annotation(
        generated_with_cut_run_spans(out, "Inlay plug run"),
        ctx.semantic_ctx,
    ))
}

/// VCarve family adapter. Cut-run spans labeled "V-carve run"; the
/// dispatch prologue refuses a tool that is not a V-bit. Cancellable: the cooperative
/// cancel closure is built by `ctx.cancel_fn()` (pinned by
/// `every_generator_polls_a_preset_cancel_flag_past_the_prologue`).
pub(crate) fn generate_vcarve(
    ctx: &ExecutionContext<'_>,
    prepared: &Prepared<'_>,
    op: &OperationConfig,
) -> Result<GeneratedToolpath, OperationError> {
    let cfg = config_guard!(op, VCarve, "generate_vcarve");
    let polys = prepared.polygons()?;
    let ha = vbit_half_angle(ctx.tool_cfg);
    let safe_z = ctx.heights.retract_z;
    let cancel_fn = ctx.cancel_fn();
    let mut combined = Toolpath::new();
    for poly in polys {
        let tp = crate::ops::vcarve::vcarve_toolpath_with_cancel(
            poly,
            &crate::ops::vcarve::VCarveParams {
                half_angle: ha,
                max_depth: cfg.max_depth,
                stepover: cfg.stepover,
                feed_rate: op.feed_rate(),
                plunge_rate: op.plunge_rate(),
                safe_z,
                tolerance: cfg.tolerance,
                top_z: ctx.heights.top_z,
            },
            &cancel_fn,
        )?;
        combined.moves.extend(tp.moves);
    }
    Ok(with_depth_run_annotation(
        generated_with_cut_run_spans(combined, "V-carve run"),
        ctx.semantic_ctx,
    ))
}

/// Chamfer family adapter (W3.5): [`ChamferPass`] on the Layered runner.
/// The ladder is one level, `top_z - (w + t) / tan(a)`, from the tool angle
/// and not from the heights. The dispatch prologue refuses a tool that is
/// not a V-bit, and checks the cancel flag ahead of the polygon and V-bit
/// refusals (W2.2). The kernel has no cancel of its own; the runner polls
/// the cancel before each (outline, level), which is one poll per polygon
/// here, as before (pinned by
/// `every_generator_polls_a_preset_cancel_flag_past_the_prologue`).
pub(crate) fn generate_chamfer(
    ctx: &ExecutionContext<'_>,
    prepared: &Prepared<'_>,
    op: &OperationConfig,
) -> Result<GeneratedToolpath, OperationError> {
    let cfg = config_guard!(op, Chamfer, "generate_chamfer");
    let pass = ChamferPass {
        rest_use: op.capabilities().rest_use,
        params: crate::ops::chamfer::ChamferParams {
            chamfer_width: cfg.chamfer_width,
            tip_offset: cfg.tip_offset,
            tool_half_angle: vbit_half_angle(ctx.tool_cfg),
            side: cfg.side,
            feed_rate: op.feed_rate(),
            plunge_rate: op.plunge_rate(),
            safe_z: ctx.heights.retract_z,
            top_z: ctx.heights.top_z,
        },
    };
    run_layered(&pass, ctx, prepared)
}

/// The Chamfer op: the tip rings of each outline (the W1.3b side rule,
/// `ops::chamfer::chamfer_rings_reported`), cut at the one level the tool
/// angle gives.
struct ChamferPass {
    rest_use: RestUse,
    params: crate::ops::chamfer::ChamferParams,
}

impl ChamferPass {
    /// The cone radius (mm) at `top_z`: `depth · tan(a) = w + t`.
    fn cone_radius_at_top(&self) -> f64 {
        self.params.chamfer_width + self.params.tip_offset
    }
}

impl LayeredOp for ChamferPass {
    type Input = Polygon2;
    type Geometry = Vec<Vec<P2>>;
    type Report = OffsetFailures;

    /// The chamfer depth. [`Self::ladder`] does not read it: the one level
    /// comes from the tool angle.
    fn depth_per_pass(&self) -> f64 {
        crate::ops::chamfer::chamfer_depth(&self.params)
    }

    /// One level, so the order of the outlines is the order of the cuts in
    /// both modes. Chamfer has no ordering option.
    fn region_ordering(&self) -> RegionOrdering {
        RegionOrdering::ByArea
    }

    /// `LevelSkip`, as Profile: a chamfer whose edge a prior op removed
    /// skips its level, and a chamfer is never clipped, because a clipped
    /// ring starts and ends its bevel in the middle of an edge. With one
    /// level, the skip is the whole outline or nothing.
    fn rest_use(&self) -> RestUse {
        self.rest_use
    }

    /// The tip rings and the cone radius at `top_z`. The cone removes
    /// material only within `w + t` of a tip ring, between the level and
    /// `top_z` (`ops/chamfer.rs`, the geometry of `chamfer_toolpath`). The
    /// V-bit radius would read material at the full flute height, which
    /// the bevel never cuts.
    fn reach_band(&self, geometry: &Self::Geometry, _tool_radius: f64) -> ReachBand {
        ReachBand {
            rings: geometry.clone(),
            half_width: self.cone_radius_at_top(),
        }
    }

    /// One level: the tip at `top_z - (w + t) / tan(a)`. The catalogue
    /// gives Chamfer no `cutting_levels`, and the heights' depth is not its
    /// depth.
    fn ladder(&self, _ctx: &ExecutionContext<'_>) -> Vec<f64> {
        vec![self.params.top_z - crate::ops::chamfer::chamfer_depth(&self.params)]
    }

    fn prepare(
        &self,
        region: &[Polygon2],
        _layer: &LayerCtx<'_>,
    ) -> Result<(Self::Geometry, Self::Report), OperationError> {
        let mut rings = Vec::new();
        let mut failures = 0;
        for piece in region {
            let (piece_rings, piece_failures) =
                crate::ops::chamfer::chamfer_rings_reported(piece, &self.params);
            failures += piece_failures;
            rings.extend(piece_rings);
        }
        Ok((rings, OffsetFailures(failures)))
    }

    fn emit_level(
        &self,
        geometry: &Self::Geometry,
        level: LevelInfo,
        _stock: &LevelStock<'_>,
        _cancel: &dyn CancelCheck,
    ) -> Result<Toolpath, OperationError> {
        Ok(crate::ops::chamfer::chamfer_rings_at_z(
            geometry,
            level.z,
            &self.params,
        ))
    }

    /// One cut run per ring, labelled "Chamfer run", as before W3.5.
    fn spans(&self, toolpath: Toolpath, _levels: &[f64]) -> GeneratedToolpath {
        generated_with_cut_run_spans(toolpath, "Chamfer run")
    }
}

/// The V-bit half-angle, in radians. The dispatch prologue already refused
/// any tool that is not a V-bit (W2.2: the row's `tool_constraints`), so
/// this only converts.
fn vbit_half_angle(tool_cfg: &ToolConfig) -> f64 {
    (tool_cfg.included_angle / 2.0).to_radians()
}

/// ProjectCurve family adapter. Builds its own cutter from
/// `tool_cfg` (generator API takes the boxed cutter); reads
/// `cfg.setup_z_flipped`, which the session/viz drivers pre-set on the
/// config BEFORE dispatch (caller-side mutation preserved — plan
/// §Phase-5 task 4). Cancellable: the cooperative cancel closure is
/// built by `ctx.cancel_fn()` (pinned by
/// `every_generator_polls_a_preset_cancel_flag_past_the_prologue`).
pub(crate) fn generate_project_curve(
    ctx: &ExecutionContext<'_>,
    prepared: &Prepared<'_>,
    op: &OperationConfig,
) -> Result<GeneratedToolpath, OperationError> {
    let cfg = config_guard!(op, ProjectCurve, "generate_project_curve");
    let polys = prepared.polygons()?;
    let MeshInput {
        mesh: m,
        index: idx,
    } = prepared.mesh()?;
    let cutter = build_cutter(ctx.tool_cfg);
    let direction = match cfg.direction {
        crate::compute::operation_configs::ProjectCurveDirection::FromAbove => {
            crate::ops::project_curve::ProjectDirection::FromAbove
        }
        crate::compute::operation_configs::ProjectCurveDirection::FromBelow => {
            crate::ops::project_curve::ProjectDirection::FromBelow
        }
    };
    let side = match cfg.side {
        crate::compute::operation_configs::ProjectCurveSide::Center => {
            crate::ops::project_curve::ProjectSide::Center
        }
        crate::compute::operation_configs::ProjectCurveSide::Inside => {
            crate::ops::project_curve::ProjectSide::Inside
        }
        crate::compute::operation_configs::ProjectCurveSide::Outside => {
            crate::ops::project_curve::ProjectSide::Outside
        }
    };
    let projection = match cfg.projection {
        crate::compute::operation_configs::ProjectCurveProjection::Cutter => {
            crate::ops::project_curve::ProjectProjection::Cutter
        }
        crate::compute::operation_configs::ProjectCurveProjection::Point => {
            crate::ops::project_curve::ProjectProjection::Point
        }
    };
    let params = crate::ops::project_curve::ProjectCurveParams {
        depth: cfg.depth,
        point_spacing: cfg.point_spacing,
        feed_rate: op.feed_rate(),
        plunge_rate: op.plunge_rate(),
        safe_z: ctx.heights.retract_z,
        direction,
        tool_radius: ctx.tool_def.radius(),
        side,
        projection,
        setup_z_flipped: cfg.setup_z_flipped,
    };
    let cancel_fn = ctx.cancel_fn();
    let mut combined = Toolpath::new();
    for poly in polys {
        let tp = crate::ops::project_curve::project_curve_toolpath_with_cancel(
            poly, m, idx, &cutter, &params, &cancel_fn,
        )?;
        combined.moves.extend(tp.moves);
    }
    // W6.2: the link stage may not leave the op's territory regions.
    let surface = MeshInput {
        mesh: m,
        index: idx,
    };
    let regions = prepared.territory().regions();
    let combined = chain_project_curve(ctx, cfg, &params, &cutter, surface, regions, combined);
    Ok(with_depth_run_annotation(
        generated_with_cut_run_spans(combined, "Projected curve"),
        ctx.semantic_ctx,
    ))
}

fn chain_project_curve(
    ctx: &ExecutionContext<'_>,
    cfg: &crate::compute::operation_configs::ProjectCurveConfig,
    params: &crate::ops::project_curve::ProjectCurveParams,
    cutter: &ToolDefinition,
    surface: MeshInput<'_>,
    regions: Option<&RegionSet<'_>>,
    tp: Toolpath,
) -> Toolpath {
    let MeshInput { mesh, index } = surface;
    if cfg.chain_distance_mm <= 0.0 {
        return tp;
    }
    // Frame guard. The standalone `FromBelow` arm projects against a
    // Z-FLIPPED copy of the mesh it builds internally and flips the result
    // back, so the emitted cut Z lives in a frame `ctx.mesh` cannot answer
    // for. A relink here would drop-cutter the un-flipped mesh and link
    // through the workpiece. The GUI/session pipeline never reaches this
    // (a bottom-facing setup sets `setup_z_flipped`), but a standalone
    // caller can, so refuse rather than emit geometry from two frames.
    if matches!(
        cfg.direction,
        crate::compute::operation_configs::ProjectCurveDirection::FromBelow
    ) && !cfg.setup_z_flipped
    {
        tracing::info!(
            chain_distance_mm = cfg.chain_distance_mm,
            "Project-curve chaining skipped: standalone FromBelow projects \
             against an internally Z-flipped mesh, so a link costed against \
             the un-flipped mesh would be geometrically wrong"
        );
        return tp;
    }
    let rp = crate::finish::surface_link::RelinkParams {
        link_tolerance: None,
        hookup_distance: cfg.chain_distance_mm,
        // The link does not ride the surface at all — see `link_ceiling`
        // below — so there is no crest to stand off from here.
        stock_to_leave: 0.0,
        sampling: cfg.point_spacing.max(0.01),
        // Read off the SAME resolved params the generator emitted with, so
        // a link and the cut it joins can never quote different rates.
        feed_rate: params.feed_rate,
        plunge_rate: params.plunge_rate,
        safe_z: params.safe_z,
        link_kinematics: ctx.link_kinematics.as_ref(),
        // Chains arrive in DXF-entity order, which is authoring order, not
        // travel order. Forward-only nearest-first: no chain is reversed,
        // so no projected curve changes direction.
        reorder: true,
        // A link is a fed move over the workpiece; one that leaves the
        // operation's territory travels over ground the boundary
        // deliberately excluded.
        boundary: regions,
        // …and being AIRBORNE does not buy an exemption here. This op's
        // boundary is machining territory, which the operator may have drawn
        // around a clamp or a keep-out; the ceiling below reads its clearance
        // from the dexel stock, which does not model workholding at all. The
        // finishing families waive this for their region polygons (see the
        // field); an engraving pass must not.
        airborne_links_may_leave_territory: false,
        // THE reason this op needs more than the finishing families do:
        // project_curve engraves into stock that has usually NOT been
        // cleared down to the mesh, so the mesh is not the material. Links
        // travel above whatever the input stock still has standing.
        // Engraving prior: flush ground is the RAW workpiece face — a fed
        // slide across it drags the cutter over stock this op must not
        // touch (`stock_safety_links_clear_standing_material` pins it).
        flush_ride: false,
        // R1: the one builder. Project Curve needs the ceiling on fresh
        // stock too (`LinkCeilingNeed::Always`), so this is always `Some`.
        // On fresh stock it reads the stock top, not a pinned `top_z` below
        // it: a link at the pinned top rides inside the standing stock.
        link_ceiling: op_link_ceiling(crate::compute::catalog::OperationType::ProjectCurve, ctx),
    };
    let (linked, rep) = crate::finish::surface_link::relink_fragments(
        AnnotatedToolpath::new(tp),
        mesh,
        index,
        cutter,
        &rp,
    );
    tracing::info!(
        chain_distance_mm = cfg.chain_distance_mm,
        fragments = rep.fragments,
        surface_links = rep.surface_links,
        retract_links = rep.retract_links,
        too_far = rep.too_far,
        off_surface = rep.off_surface,
        slower_than_retract = rep.slower_than_retract,
        outside_boundary = rep.outside_boundary,
        ceiling_above_safe_z = rep.ceiling_above_safe_z,
        "Project-curve chaining"
    );
    // Spans are built from the RESULT (`generated_with_cut_run_spans` runs
    // after this), and this adapter owns no other index-carrying channel,
    // so there is nothing to reconcile — but the type still has to be told
    // so, rather than the provenance being dropped by convention.
    linked
        .reconcile(&mut ReconcileSet::empty())
        .into_inner()
        .toolpath
}
