//! The raster finishing family: scallop, drop cutter and waterline.
//!
//! All three relink their fragments through the shared finishing link stage
//! before they return. Split out of `compute/execute.rs` (P4).

use std::sync::atomic::Ordering;

use crate::compute::catalog::{OperationConfig, OperationType};
use crate::compute::operation_configs::OpMotion;
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::tool::{MillingCutter, ToolDefinition};
use crate::toolpath::Toolpath;
use crate::trace::toolpath_spans::AnnotatedToolpath;
use crate::trace::transform_provenance::ReconcileSet;

use super::findings::{record_relink_totals, record_truncated_core};
use super::shared::{
    generated_with_cut_run_spans, generated_with_depth_run_spans, generated_with_spans,
    require_index, require_mesh, with_depth_run_annotation,
};
use super::{ExecutionContext, GeneratedToolpath, OperationError};

/// Curve chaining for `project_curve` — the op's air is COUNT-bound.
///
/// `project_curve::emit_path_segment_with_intent` emits one
/// `rapid → EntryPlunge → cut → Retract` unit per contiguous projected
/// stretch, unconditionally, however short the hop to the next one. A rivers
/// DXF is hundreds of those units across dozens of entities, and every hop
/// pays two full safe-Z legs whatever its XY length. This runs across the
/// WHOLE combined path (not per polygon), so chains from different DXF
/// entities can join, and re-decides each junction on evidence:
/// [`crate::finish::surface_link::relink_fragments`] drop-cutter samples the link so
/// it cannot gouge the mesh, refuses it if it would leave the operation's
/// boundary, lifts it clear of anything standing in the input stock, refuses
/// it outright when that clearance reaches safe Z, and (with kinematics in
/// scope) keeps it only when it actually beats the retract on time.
/// Fragment interiors are copied verbatim.
///
/// A no-op — the input returned untouched, the relinker never entered — at
/// the shipped `chain_distance_mm` of `0.0`.
/// THE one place a finishing adapter builds the shared surface-link stage
/// (G-LINKSTAGE, `planning/linking_2026-09-09/SPEC.md` §3.1).
///
/// Every finishing family that opts in reads its ceiling, its boundary and
/// its kinematics from here, so the three cannot drift apart family by family
/// again. Before this existed the unified finish took its ceiling from
/// `ctx.initial_stock` and the scallop passed `link_ceiling: None`, which on a
/// `FromRemainingStock` island pass made every finger-to-finger hop a full
/// safe-Z retract AND left the scallop's surface-riding links unchecked
/// against the standing material they crossed.
///
/// `None` — the stage is OFF — at `hookup_mm <= 0.0`, which is the
/// byte-identity dial every family that has not opted in keeps at `0.0`
/// (`chain_distance_mm`'s pattern).
///
/// # The ceiling, and why it is the SAFETY half
///
/// A link that arrives laterally at cutting depth is exactly the shape
/// G-ISOCLIPRAPID names: both endpoints under the stock surface, the span
/// between them through whatever is standing. A surface-riding link rides the
/// MESH, and on a rest-driven pass the mesh sits BELOW the material. The
/// ceiling is what makes the kernel sample the whole chord against the input
/// stock and take the lifted shape where it must — a target-only check reads
/// 0.000 on a leg that buries itself between its endpoints.
///
/// `ctx.initial_stock` is `Some` exactly when the op cuts what a prior op
/// left. `None` is the fresh-stock arm, where the mesh IS the material; the
/// stage is then byte-identical to the legacy surface-riding link, which is
/// what `tests/island_stay_down_links_o3.rs` pins.
///
/// The ENVELOPE radius is the SEARCH BOUND, not the shape — inside it
/// `LinkCeiling::required_tip_z` lets the cutter's own profile decide. Same
/// reading, and the same evidence, as `generate_unified_finish`'s ceiling.
///
/// TWO lifetimes on purpose. `ctx.link_kinematics` is OWNED by the context, so
/// a reference to it can only live as long as the borrow of `*ctx` — while
/// the stock and the regions live as long as the context's own parameter. The
/// stage is therefore built in the SHORTER of the two, and each field narrows
/// to it by ordinary covariance at its own assignment. A single-lifetime
/// signature would instead ask the caller to coerce the whole
/// `ExecutionContext`, which is a stronger requirement for no gain.
fn finishing_link_stage<'c, 'a: 'c>(
    ctx: &'c ExecutionContext<'a>,
    hookup_mm: f64,
) -> Option<crate::finish::surface_link::FinishingLinkStage<'c>> {
    if hookup_mm <= 0.0 {
        return None;
    }
    Some(crate::finish::surface_link::FinishingLinkStage {
        hookup_distance: hookup_mm,
        link_ceiling: ctx
            .initial_stock
            .map(|stock| crate::finish::surface_link::LinkCeiling {
                stock: Some(stock),
                tool_radius: ctx.tool_def.envelope_radius_mm(),
                // The analytic stock top in the emission frame — the same
                // fallback `optimize_entry_descents` takes where the dexel
                // query has no answer.
                fallback_top_z: ctx.stock_bbox.max.z,
            }),
        boundary: ctx.boundary_regions,
        link_kinematics: ctx.link_kinematics.as_ref(),
    })
}

/// Run the shared stage over a finishing family's emitted path, in the
/// adapter, and report what it did.
///
/// For a family whose generator holds no index-carrying channel — the raster
/// and the waterline both build their spans from the RESULT — so the
/// reconcile set is empty and saying so is the whole contract. The scallop
/// does hold one (its ring annotations), which is why its stage runs inside
/// the generator instead.
///
/// G-LINKVISIBLE: the totals come BACK rather than being written to
/// `ctx.findings` in here, because the context would be an eighth parameter
/// and the two callers each already hold their own `ctx`. The caller records
/// them unconditionally — reaching this function at all IS the measurement.
fn relink_in_adapter(
    stage: &crate::finish::surface_link::FinishingLinkStage<'_>,
    geom: &crate::finish::surface_link::LinkGeometry,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &ToolDefinition,
    tp: Toolpath,
    family: &'static str,
) -> (Toolpath, crate::finish::unified_finish::RelinkTotals) {
    let rp = stage.params(geom);
    let (linked, rep) = crate::finish::surface_link::relink_fragments(
        AnnotatedToolpath::new(tp),
        mesh,
        index,
        cutter,
        &rp,
    );
    tracing::info!(
        family,
        hookup_mm = stage.hookup_distance,
        fragments = rep.fragments,
        surface_links = rep.surface_links,
        // The acceptance measure — only an at-depth link removes an entry.
        at_depth_links = rep.at_depth_links,
        clearance_hops = rep.clearance_hops,
        retract_links = rep.retract_links,
        too_far = rep.too_far,
        off_surface = rep.off_surface,
        slower_than_retract = rep.slower_than_retract,
        outside_boundary = rep.outside_boundary,
        ceiling_above_safe_z = rep.ceiling_above_safe_z,
        "Finishing link stage"
    );
    let mut totals = crate::finish::unified_finish::RelinkTotals::default();
    totals.add(&rep);
    (
        linked
            .reconcile(&mut ReconcileSet::empty())
            .into_inner()
            .toolpath,
        totals,
    )
}

/// Scallop family adapter (labeled-event spans + annotate_scallop).
/// The ball-tip refusal reads the registry constraint list (T7 PR C)
/// so refusal and published schema cannot drift. Cancellable: the
/// cooperative cancel closure is rebuilt from `ctx.cancel` (pinned by
/// `cancellable_families_honour_a_preset_cancel_flag`).
pub(crate) fn generate_scallop(
    ctx: &ExecutionContext<'_>,
    op: &OperationConfig,
) -> Result<GeneratedToolpath, OperationError> {
    let cfg = config_guard!(op, Scallop, "generate_scallop");
    // Membership pinned by
    // `tool_constraints_allows_matches_runtime_refusal_semantics`.
    if !OperationType::Scallop
        .registry_entry()
        .tool_constraints
        .allows(ctx.tool_cfg.tool_type.cutter_kind())
    {
        return Err(OperationError::InvalidTool(
            "Scallop requires a ball-tip tool (Ball Nose or Tapered Ball Nose)".into(),
        ));
    }
    let m = require_mesh(ctx.mesh)?;
    let idx = require_index(ctx.index, "Scallop")?;
    let params = cfg.params(
        OpMotion {
            feed_rate: op.feed_rate(),
            plunge_rate: op.plunge_rate(),
            safe_z: ctx.heights.retract_z,
        },
        ctx.link_kinematics.clone(),
    );
    let (tp, annotations, scallop_report) = if cfg.iso_field {
        // M8 iso-field rings — per-point spacing, cosine slope law,
        // completion by construction. See the wrapper's doc for evidence.
        //
        // NOT on the shared stage (SPEC §3.7). The field's ring list is level
        // sets, its ring-to-ring geometry is a different population from the
        // offset cascade's, and its fingerprint is pinned by
        // `scallop_isofield_gouge_m4` and the multitool tiers. It keeps the
        // legacy relink at the same `intra_pass_hookup_mm`, byte for byte.
        crate::finish::scallop::scallop_toolpath_iso_field_with_cancel(
            m,
            idx,
            ctx.tool_def,
            &params,
            ctx.debug_ctx,
            ctx.boundary_regions,
            &(|| ctx.cancel.load(Ordering::SeqCst)),
        )
        .map_err(|_e| OperationError::Cancelled)?
    } else {
        // The offset-cascade contour scallop IS the op §2 of the linking spec
        // measured: breadth-first ring order, no reorder, no loop rotation
        // and no stock ceiling. It opts in.
        let stage = finishing_link_stage(ctx, cfg.intra_pass_hookup_mm);
        crate::finish::scallop::scallop_toolpath_structured_annotated_with_cancel_and_stage(
            m,
            idx,
            ctx.tool_def,
            &params,
            ctx.debug_ctx,
            ctx.boundary_regions,
            stage.as_ref(),
            &(|| ctx.cancel.load(Ordering::SeqCst)),
        )
        .map_err(|_e| OperationError::Cancelled)?
    };
    record_truncated_core(
        ctx.findings,
        scallop_report.uncut_core_mm2,
        scallop_report.untouched_mm2,
        scallop_report.standing_mm2,
    );
    // G-LINKVISIBLE: the generator already decided whether the stage ran, so
    // pass its `Option` straight through rather than re-deriving the
    // question from the config here — a hookup above zero on a `continuous`
    // pass, or one whose cascade produced no ring, measured nothing.
    if let Some(totals) = scallop_report.relink {
        record_relink_totals(ctx.findings, totals);
    }
    if let Some(sem) = ctx.semantic_ctx {
        crate::compute::annotate::annotate_scallop(
            &annotations,
            &tp,
            sem,
            crate::compute::annotate::ScallopRegionGrouping::ByBoundaryRegion,
        );
    }
    let spans = crate::compute::spans::spans_from_labeled_events(
        tp.moves.len(),
        annotations
            .iter()
            .map(|ann| (ann.move_index, ann.event.label())),
    );
    Ok(generated_with_spans(tp, spans))
}

/// DropCutter family adapter. Cancellable: the cooperative cancel
/// closure is rebuilt from `ctx.cancel` (pinned by
/// `cancellable_families_honour_a_preset_cancel_flag`).
pub(crate) fn generate_drop_cutter(
    ctx: &ExecutionContext<'_>,
    op: &OperationConfig,
) -> Result<GeneratedToolpath, OperationError> {
    let cfg = config_guard!(op, DropCutter, "generate_drop_cutter");
    let m = require_mesh(ctx.mesh)?;
    let idx = require_index(ctx.index, "DropCutter")?;
    // Floor the drop-cutter min_z to the mesh bottom. A 3D finish
    // should only tip-track the mesh surface — anything lower is either
    // a non-contact clamp or the tool's taper forcing the tip below
    // the real surface (which would gouge). We also enforce the
    // stock-bottom floor as a safety check.
    let effective_min_z = cfg
        .min_z
        .max(m.bbox.min.z - 0.1)
        .max(ctx.stock_bbox.min.z - 1.0);
    let mut grid = crate::surface::dropcutter::batch_drop_cutter_with_cancel(
        m,
        idx,
        ctx.tool_def,
        cfg.stepover,
        0.0,
        effective_min_z,
        &(|| ctx.cancel.load(Ordering::SeqCst)),
    )
    .map_err(|_e| OperationError::Cancelled)?;
    // Drop grid points whose vertical ray misses every triangle
    // in the mesh. `point_drop_cutter` marks `contacted = true`
    // whenever the cutter (which has radius) touches ANY nearby
    // triangle — including the rim of a mesh that doesn't cover
    // that XY. Without this check the tool rides the edge and
    // carves a trench around the part.
    for pt in &mut grid.points {
        let mut over = false;
        for &tri_idx in &idx.query(pt.x, pt.y, 0.0) {
            #[allow(clippy::indexing_slicing)]
            let tri = &m.faces[tri_idx];
            if tri.contains_point_xy(pt.x, pt.y) {
                over = true;
                break;
            }
        }
        if !over {
            pt.z = effective_min_z;
            pt.contacted = false;
        }
    }
    // Non-contacted grid points get clamped to effective_min_z (floored
    // at the mesh bottom). Filter them so the finish never cuts past
    // the mesh boundary.
    let min_z_filter = Some(effective_min_z);
    let slope_filter_active =
        crate::finish::finish_setup::slope_filter_active(cfg.slope_from, cfg.slope_to);
    let feed_rate = op.feed_rate();
    let plunge_rate = op.plunge_rate();
    let safe_z = ctx.heights.retract_z;
    let tp = if slope_filter_active {
        // P1.3 (planning/finishing_stack_review_2026-07.md): derive the
        // slope filter from `SlopeMap` (radians + normals + curvature)
        // instead of the retired degrees-only `compute_grid_slopes`. The
        // grid above is always sampled at `direction_deg = 0.0`, so its
        // rotated sampling frame coincides with world frame and its
        // u_start/v_start read as world mins with square (x_step ==
        // y_step) cells — safe to feed straight into `SlopeMap::from_z_grid`.
        // Convert the config's degree bounds to radians at this boundary,
        // since `SlopeMap::angles` is radians-native.
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
            cfg.slope_from.to_radians(),
            cfg.slope_to.to_radians(),
            feed_rate,
            plunge_rate,
            safe_z,
            min_z_filter,
            crate::toolpath::MoveIntent::FinishingCut,
            ctx.boundary_regions,
        )
    } else {
        crate::toolpath::raster_toolpath_from_grid(
            &grid,
            feed_rate,
            plunge_rate,
            safe_z,
            min_z_filter,
            ctx.boundary_regions,
        )
    };
    // G-LINKSTAGE, OFF by default (`hookup_mm` ships at `0.0`), so this
    // family's fingerprint does not move until an operator asks for it.
    //
    // Why it is wired at all: the raster's only linker today is the
    // serpentine hookup in `toolpath.rs`, whose cap is one grid diagonal, so
    // two runs of the same row split by an excluded cell — two steps apart —
    // cannot join. On the wanaka island raster that is 953 row fragments and
    // 954 retracts (`planning/linking_2026-09-09/SPEC.md` §1). Raster rows
    // are OPEN runs, so no fragment kind is declared: reversing a row would
    // flip its cut direction and rotation does not apply.
    // G-LINKVISIBLE: `None` here reaches `ToolpathStats::relink` as `None`,
    // which is the honest "the stage never ran" — `hookup_mm` at its shipped
    // `0.0`. A recorded zero would claim a measurement that never happened.
    let tp = match finishing_link_stage(ctx, cfg.hookup_mm) {
        None => tp,
        Some(stage) => {
            let (tp, totals) = relink_in_adapter(
                &stage,
                &crate::finish::surface_link::LinkGeometry {
                    // The raster rides the drop-cutter grid itself, so there
                    // is no crest to stand off from and no separate leave
                    // dial on this op (`WaterlineConfig`'s
                    // absent-capability note).
                    stock_to_leave: 0.0,
                    sampling: grid.x_step.max(0.01),
                    feed_rate,
                    plunge_rate,
                    safe_z,
                },
                m,
                idx,
                ctx.tool_def,
                tp,
                "drop_cutter",
            );
            record_relink_totals(ctx.findings, totals);
            tp
        }
    };
    Ok(with_depth_run_annotation(
        generated_with_cut_run_spans(tp, "Raster row"),
        ctx.semantic_ctx,
    ))
}

/// Waterline family adapter. Cancellable: the cooperative cancel
/// closure is rebuilt from `ctx.cancel` (pinned by
/// `cancellable_families_honour_a_preset_cancel_flag`).
pub(crate) fn generate_waterline(
    ctx: &ExecutionContext<'_>,
    op: &OperationConfig,
) -> Result<GeneratedToolpath, OperationError> {
    let cfg = config_guard!(op, Waterline, "generate_waterline");
    let m = require_mesh(ctx.mesh)?;
    let idx = require_index(ctx.index, "Waterline")?;
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
    let tp = crate::ops::waterline::waterline_toolpath_with_cancel(
        m,
        idx,
        ctx.tool_def,
        ctx.heights.top_z,
        ctx.heights.bottom_z,
        cfg.z_step,
        &params,
        ctx.boundary_regions,
        &(|| ctx.cancel.load(Ordering::SeqCst)),
    )
    .map_err(|_e| OperationError::Cancelled)?;
    // G-LINKSTAGE, OFF by default (`hookup_mm` ships at `0.0`), so this
    // family's fingerprint does not move.
    //
    // No fragment kind is declared yet. A waterline level IS a closed loop
    // and would benefit from rotation, but this adapter cannot see which of
    // the emitted fragments are whole levels and which are arcs the boundary
    // split, and a mis-declared kind rotates the wrong fragment. Declaring
    // them inside `waterline_toolpath_with_cancel`, the way the scallop does,
    // is the follow-up.
    // G-LINKVISIBLE: same contract as the raster's — an unrun stage stays
    // `None` on the stats channel.
    let tp = match finishing_link_stage(ctx, cfg.hookup_mm) {
        None => tp,
        Some(stage) => {
            let (tp, totals) = relink_in_adapter(
                &stage,
                &crate::finish::surface_link::LinkGeometry {
                    stock_to_leave: params.stock_to_leave,
                    sampling: cfg.sampling.max(0.01),
                    feed_rate: params.feed_rate,
                    plunge_rate: params.plunge_rate,
                    safe_z: params.safe_z,
                },
                m,
                idx,
                ctx.tool_def,
                tp,
                "waterline",
            );
            record_relink_totals(ctx.findings, totals);
            tp
        }
    };
    // R2.8: waterline has a real Z-level ladder (unlike the single-level
    // Face op) — pass it to the span builder instead of `&[]` so
    // `spans_from_depth_runs`'s `nearest_level` snapping has real levels
    // to snap to, matching the exact ladder `waterline_toolpath_with_cancel`
    // cut at (same helper, one source of truth).
    let levels = crate::ops::waterline::waterline_z_levels(
        ctx.heights.top_z,
        ctx.heights.bottom_z,
        cfg.z_step,
    );
    Ok(with_depth_run_annotation(
        generated_with_depth_run_spans(tp, &levels),
        ctx.semantic_ctx,
    ))
}
// ── Public API ────────────────────────────────────────────────────────
