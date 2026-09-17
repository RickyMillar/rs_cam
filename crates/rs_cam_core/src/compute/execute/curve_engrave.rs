//! The curve and V-bit engraving family: inlay, V-carve, chamfer and
//! projected curve.
//!
//! These adapters cut along a curve rather than clearing an area, so they
//! share the V-bit half-angle read and the projected-curve chaining pass.
//! Split out of `compute/execute.rs` (P4).

use std::sync::atomic::Ordering;

use crate::compute::catalog::OperationConfig;
use crate::compute::cutter::build_cutter;
use crate::compute::tool_config::{ToolConfig, ToolType};
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::tool::{MillingCutter, ToolDefinition};
use crate::toolpath::Toolpath;
use crate::trace::toolpath_spans::AnnotatedToolpath;
use crate::trace::transform_provenance::ReconcileSet;

use super::shared::{
    generated_with_cut_run_spans, require_index, require_mesh, require_polygons,
    with_depth_run_annotation,
};
use super::{ExecutionContext, GeneratedToolpath, OperationError};

/// Inlay family adapter (female + male halves concatenated with a
/// retract between; V-bit refusal preserved verbatim). Cancellable: the
/// cooperative cancel closure is rebuilt from `ctx.cancel` (pinned by
/// `cancellable_families_honour_a_preset_cancel_flag`).
pub(crate) fn generate_inlay(
    ctx: &ExecutionContext<'_>,
    op: &OperationConfig,
) -> Result<GeneratedToolpath, OperationError> {
    let cfg = config_guard!(op, Inlay, "generate_inlay");
    let polys = require_polygons(ctx.polygons)?;
    let ha = vbit_half_angle(ctx.tool_cfg, "Inlay")?;
    let safe_z = ctx.heights.retract_z;
    let cancel_fn = || ctx.cancel.load(Ordering::SeqCst);
    let mut female_out = Toolpath::new();
    let mut male_out = Toolpath::new();
    for poly in polys {
        let r = crate::ops::inlay::inlay_toolpaths_with_cancel(
            poly,
            &crate::ops::inlay::InlayParams {
                half_angle: ha,
                pocket_depth: cfg.pocket_depth,
                glue_gap: cfg.glue_gap,
                flat_depth: cfg.flat_depth,
                boundary_offset: cfg.boundary_offset,
                stepover: cfg.stepover,
                flat_tool_radius: cfg.flat_tool_radius,
                feed_rate: op.feed_rate(),
                plunge_rate: op.plunge_rate(),
                safe_z,
                tolerance: cfg.tolerance,
                top_z: ctx.heights.top_z,
            },
            &cancel_fn,
        )
        .map_err(|_e| OperationError::Cancelled)?;
        female_out.moves.extend(r.female.moves);
        male_out.moves.extend(r.male.moves);
    }
    let mut out = female_out;
    if !male_out.moves.is_empty() {
        out.final_retract(safe_z);
        out.moves.extend(male_out.moves);
    }
    Ok(with_depth_run_annotation(
        generated_with_cut_run_spans(out, "Inlay run"),
        ctx.semantic_ctx,
    ))
}

/// VCarve family adapter. Cut-run spans labeled "V-carve run"; refusal
/// for non-V-bit tools preserved verbatim. Cancellable: the cooperative
/// cancel closure is rebuilt from `ctx.cancel` (pinned by
/// `cancellable_families_honour_a_preset_cancel_flag`).
pub(crate) fn generate_vcarve(
    ctx: &ExecutionContext<'_>,
    op: &OperationConfig,
) -> Result<GeneratedToolpath, OperationError> {
    let cfg = config_guard!(op, VCarve, "generate_vcarve");
    let polys = require_polygons(ctx.polygons)?;
    let ha = vbit_half_angle(ctx.tool_cfg, "VCarve")?;
    let safe_z = ctx.heights.retract_z;
    let cancel_fn = || ctx.cancel.load(Ordering::SeqCst);
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
        )
        .map_err(|_e| OperationError::Cancelled)?;
        combined.moves.extend(tp.moves);
    }
    Ok(with_depth_run_annotation(
        generated_with_cut_run_spans(combined, "V-carve run"),
        ctx.semantic_ctx,
    ))
}

/// Chamfer family adapter. Cut-run spans labeled "Chamfer run"; refusal
/// for non-V-bit tools preserved verbatim. Cancellable since 2026-08-14
/// (O-CANC): the check is the very first statement — ahead of the
/// polygon and V-bit preconditions, so a pre-set flag reports
/// "cancelled" rather than whatever else happened to be wrong first —
/// and it is repeated per polygon, which is the only unbounded axis this
/// family has. Pinned by
/// `cancellable_families_honour_a_preset_cancel_flag`.
pub(crate) fn generate_chamfer(
    ctx: &ExecutionContext<'_>,
    op: &OperationConfig,
) -> Result<GeneratedToolpath, OperationError> {
    if ctx.cancel.load(Ordering::SeqCst) {
        return Err(OperationError::Cancelled);
    }
    let cfg = config_guard!(op, Chamfer, "generate_chamfer");
    let polys = require_polygons(ctx.polygons)?;
    let ha = vbit_half_angle(ctx.tool_cfg, "Chamfer")?;
    let safe_z = ctx.heights.retract_z;
    let mut combined = Toolpath::new();
    for poly in polys {
        if ctx.cancel.load(Ordering::SeqCst) {
            return Err(OperationError::Cancelled);
        }
        let params = crate::ops::chamfer::ChamferParams {
            chamfer_width: cfg.chamfer_width,
            tip_offset: cfg.tip_offset,
            tool_half_angle: ha,
            feed_rate: op.feed_rate(),
            plunge_rate: op.plunge_rate(),
            safe_z,
            top_z: ctx.heights.top_z,
        };
        let tp = crate::ops::chamfer::chamfer_toolpath(poly, &params);
        combined.moves.extend(tp.moves);
    }
    Ok(with_depth_run_annotation(
        generated_with_cut_run_spans(combined, "Chamfer run"),
        ctx.semantic_ctx,
    ))
}

/// R2.5: the V-Bit-only tool-geometry guard duplicated 3× (Inlay, VCarve,
/// Chamfer) — resolve the half-angle or refuse with the operation name.
fn vbit_half_angle(tool_cfg: &ToolConfig, op_name: &str) -> Result<f64, OperationError> {
    match tool_cfg.tool_type {
        ToolType::VBit => Ok((tool_cfg.included_angle / 2.0).to_radians()),
        _ => Err(OperationError::InvalidTool(format!(
            "{op_name} requires V-Bit tool"
        ))),
    }
}

/// ProjectCurve family adapter. Builds its own cutter from
/// `tool_cfg` (generator API takes the boxed cutter); reads
/// `cfg.setup_z_flipped`, which the session/viz drivers pre-set on the
/// config BEFORE dispatch (caller-side mutation preserved — plan
/// §Phase-5 task 4). Cancellable: the cooperative cancel closure is
/// rebuilt from `ctx.cancel` (pinned by
/// `cancellable_families_honour_a_preset_cancel_flag`).
pub(crate) fn generate_project_curve(
    ctx: &ExecutionContext<'_>,
    op: &OperationConfig,
) -> Result<GeneratedToolpath, OperationError> {
    let cfg = config_guard!(op, ProjectCurve, "generate_project_curve");
    let polys = require_polygons(ctx.polygons)?;
    let m = require_mesh(ctx.mesh)?;
    let idx = require_index(ctx.index, "ProjectCurve")?;
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
    let params = crate::ops::project_curve::ProjectCurveParams {
        depth: cfg.depth,
        point_spacing: cfg.point_spacing,
        feed_rate: op.feed_rate(),
        plunge_rate: op.plunge_rate(),
        safe_z: ctx.heights.retract_z,
        direction,
        tool_radius: ctx.tool_def.radius(),
        side,
        setup_z_flipped: cfg.setup_z_flipped,
    };
    let cancel_fn = || ctx.cancel.load(Ordering::SeqCst);
    let mut combined = Toolpath::new();
    for poly in polys {
        let tp = crate::ops::project_curve::project_curve_toolpath_with_cancel(
            poly, m, idx, &cutter, &params, &cancel_fn,
        )
        .map_err(|_e| OperationError::Cancelled)?;
        combined.moves.extend(tp.moves);
    }
    let combined = chain_project_curve(ctx, cfg, &params, &cutter, m, idx, combined);
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
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    tp: Toolpath,
) -> Toolpath {
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
        boundary: ctx.boundary_regions,
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
        link_ceiling: Some(crate::finish::surface_link::LinkCeiling {
            stock: ctx.initial_stock,
            tool_radius: ctx.tool_def.radius(),
            fallback_top_z: ctx.heights.top_z,
        }),
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
