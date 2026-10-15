//! Guards and span annotators every operation family uses.
//!
//! The `generated_with_*` wrappers attach depth-run, cut-run or drill spans to a
//! finished `Toolpath`. Split out of `compute/execute.rs` (P4). No family owns
//! them, so they live here.

use crate::toolpath::Toolpath;
use crate::trace::semantic_trace::ToolpathSemanticContext;
use crate::trace::toolpath_spans::AnnotatedToolpath;

use crate::compute::catalog::OperationType;
use crate::dexel_stock::TriDexelStock;
use crate::finish::surface_link::{FinishingLinkStage, LinkCeiling, LinkGeometry};
use crate::finish::unified_finish::RelinkTotals;
use crate::geometry::region_set::RegionSet;
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::tool::{MillingCutter, ToolDefinition};
use crate::trace::transform_provenance::ReconcileSet;

use super::{ExecutionContext, GeneratedToolpath};

/// F2 (defect class C3): every generation funnel appends the
/// [`crate::toolpath::MoveIntent`]-derived transit spans
/// ([`crate::compute::spans::spans_from_move_intents`]) after the
/// family's structural spans, so all 22 op families get gate-visible
/// Entry / LinkBridge / LeadOut ancestry in one pass — not just the
/// families whose generator explicitly tags transients at emission.
pub(super) fn generated_with_spans(
    toolpath: Toolpath,
    mut spans: Vec<crate::trace::toolpath_spans::Span>,
) -> GeneratedToolpath {
    spans.extend(crate::compute::spans::spans_from_move_intents(&toolpath));
    AnnotatedToolpath::with_spans(toolpath, spans)
}

pub(super) fn generated_with_depth_run_spans(
    toolpath: Toolpath,
    levels: &[f64],
) -> GeneratedToolpath {
    let spans = crate::compute::spans::spans_from_depth_runs(&toolpath, levels);
    generated_with_spans(toolpath, spans)
}

pub(super) fn generated_with_cut_run_spans(
    toolpath: Toolpath,
    label_prefix: &str,
) -> GeneratedToolpath {
    let spans = crate::compute::spans::spans_from_cutting_runs(&toolpath, label_prefix);
    generated_with_spans(toolpath, spans)
}

pub(super) fn generated_with_drill_spans(toolpath: Toolpath) -> GeneratedToolpath {
    let spans = crate::compute::spans::spans_from_drill_holes(&toolpath);
    generated_with_spans(toolpath, spans)
}

/// R2.7: the `if let Some(sem) = ctx.semantic_ctx { annotate_depth_run_spans(..) }`
/// postscript duplicated after every family adapter's span-building call
/// (both the `generated_with_depth_run_spans` and `generated_with_cut_run_spans`
/// bases end up here) — apply the generic depth-run semantic annotation
/// when a context is present, then hand the toolpath back unchanged.
pub(super) fn with_depth_run_annotation(
    generated: GeneratedToolpath,
    semantic_ctx: Option<&ToolpathSemanticContext>,
) -> GeneratedToolpath {
    if let Some(sem) = semantic_ctx {
        crate::compute::annotate::annotate_depth_run_spans(
            &generated.spans,
            &generated.toolpath,
            sem,
        );
    }
    generated
}

/// When an op's links need a [`LinkCeiling`] (ruling R1, W1.1). Each op
/// states it in its registry row (`Capabilities::link_ceiling`, W2.4).
///
/// A link ceiling lifts a fed link clear of material that stands above the
/// mesh. The remaining stock gives that material when the op has a
/// snapshot. On FRESH stock the analytic stock top gives it, and the op
/// decides here whether it needs that ceiling at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkCeilingNeed {
    /// The op runs on stock that is not cut down to the mesh, so a link
    /// that rides the mesh cuts through material. It needs a ceiling on
    /// fresh stock too. Engraving and curve ops.
    Always,
    /// The op finishes a surface. On fresh stock the mesh is the material
    /// only after a prior op clears it, and the op then has a snapshot.
    /// Without a snapshot it builds no ceiling. Finishing ops.
    WithRemainingStock,
    /// The op makes no link through `relink_fragments` or the pencil link.
    /// Adaptive3d keeps its own corridor ceiling, out of R1's scope.
    NoLink,
}

/// THE link-ceiling builder (ruling R1, W1.1). Every op that links calls it,
/// so one rule sets the ceiling for every op:
///
/// - `stock` is the remaining stock, when the op has a snapshot;
/// - `tool_radius` is the tool's ENVELOPE radius, the search bound of
///   [`LinkCeiling::required_tip_z`];
/// - `fallback_top_z` is [`crate::dressup::fresh_stock_top_z`], the
///   G-PECKSPLIT rule the entry-descent split also reads;
/// - `obstacles` (K1) lifts the ceiling over each obstacle of the setup.
///
/// `stock_top_z` is the stock top in the emission frame; `heights_top_z` is
/// the op's resolved `top_z`. The result is `None` when the op's
/// [`LinkCeilingNeed`] asks for no ceiling with this stock.
pub fn link_ceiling_for<'a>(
    op: OperationType,
    stock: Option<&'a TriDexelStock>,
    tool: &dyn MillingCutter,
    stock_top_z: f64,
    heights_top_z: f64,
    obstacles: Option<&'a crate::compute::obstacle::ObstacleToolView>,
) -> Option<LinkCeiling<'a>> {
    let build = match op.capabilities().link_ceiling {
        LinkCeilingNeed::Always => true,
        LinkCeilingNeed::WithRemainingStock => stock.is_some(),
        LinkCeilingNeed::NoLink => false,
    };
    build.then(|| LinkCeiling {
        stock,
        tool_radius: tool.envelope_radius_mm(),
        fallback_top_z: crate::dressup::fresh_stock_top_z(stock_top_z, heights_top_z),
        obstacles: obstacles.filter(|view| !view.is_empty()),
    })
}

/// [`link_ceiling_for`] with its inputs read off the execution context.
pub(super) fn op_link_ceiling<'a>(
    op: OperationType,
    ctx: &ExecutionContext<'a>,
) -> Option<LinkCeiling<'a>> {
    link_ceiling_for(
        op,
        ctx.initial_stock,
        ctx.tool_def,
        ctx.stock_bbox.max.z,
        ctx.heights.top_z,
        ctx.obstacles,
    )
}

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
/// The ceiling comes from the one builder, `op_link_ceiling` (ruling R1).
/// The three callers are finishing ops (`LinkCeilingNeed::WithRemainingStock`):
/// `ctx.initial_stock` is `Some` exactly when the op cuts what a prior op
/// left. `None` is the fresh-stock arm, where the mesh IS the material; the
/// stage is then byte-identical to the legacy surface-riding link, which is
/// what `tests/island_stay_down_links_o3.rs` pins.
///
/// The boundary is the op's territory regions (W6.2): a link may not leave
/// the area the op cuts.
///
/// TWO lifetimes on purpose. `ctx.link_kinematics` is OWNED by the context, so
/// a reference to it can only live as long as the borrow of `*ctx` — while
/// the stock and the regions live as long as the context's own parameter. The
/// stage is therefore built in the SHORTER of the two, and each field narrows
/// to it by ordinary covariance at its own assignment. A single-lifetime
/// signature would instead ask the caller to coerce the whole
/// `ExecutionContext`, which is a stronger requirement for no gain.
pub(super) fn finishing_link_stage<'c, 'a: 'c>(
    op: OperationType,
    ctx: &'c ExecutionContext<'a>,
    boundary: Option<&'c RegionSet<'c>>,
    hookup_mm: f64,
) -> Option<FinishingLinkStage<'c>> {
    if hookup_mm <= 0.0 {
        return None;
    }
    Some(FinishingLinkStage {
        hookup_distance: hookup_mm,
        link_ceiling: op_link_ceiling(op, ctx),
        boundary,
        link_kinematics: ctx.link_kinematics.as_ref(),
    })
}

/// Run the shared stage over a finishing family's emitted path, in the
/// adapter, and report what it did.
///
/// For a family whose generator holds no index-carrying channel (the raster
/// and the waterline both build their spans from the RESULT), so the
/// reconcile set is empty and saying so is the whole contract. The scallop
/// does hold one (its ring annotations), which is why its stage runs inside
/// the generator instead.
///
/// G-LINKVISIBLE: the totals come BACK rather than being written to
/// `ctx.findings` in here. The two callers (the height-field runner for
/// `LinkRule::AfterPlan`, and the Waterline `link` hook) each hold their own
/// `ctx` and record the totals unconditionally: reaching this function at
/// all IS the measurement.
pub(super) fn relink_in_adapter(
    stage: &FinishingLinkStage<'_>,
    geom: &LinkGeometry,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &ToolDefinition,
    tp: Toolpath,
    family: &'static str,
) -> (Toolpath, RelinkTotals) {
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
    let mut totals = RelinkTotals::default();
    totals.add(&rep);
    (
        linked
            .reconcile(&mut ReconcileSet::empty())
            .into_inner()
            .toolpath,
        totals,
    )
}
