//! Guards and span annotators every operation family uses.
//!
//! `require_*` turn an absent input into an `OperationError`; the
//! `generated_with_*` wrappers attach depth-run, cut-run or drill spans to a
//! finished `Toolpath`. Split out of `compute/execute.rs` (P4). No family owns
//! them, so they live here.

use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::polygon::Polygon2;
use crate::toolpath::Toolpath;
use crate::trace::semantic_trace::ToolpathSemanticContext;
use crate::trace::toolpath_spans::AnnotatedToolpath;

use crate::compute::catalog::OperationType;
use crate::dexel_stock::TriDexelStock;
use crate::finish::surface_link::LinkCeiling;
use crate::tool::MillingCutter;

use super::{ExecutionContext, GeneratedToolpath, OperationError};

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

/// CMP-13: the refusal names the operation, as [`require_index`] already
/// did. The instance is not lost — `session/compute.rs` wraps the error
/// beside the toolpath — but the KIND was, and a reader of the message
/// could not tell which of the 24 operations refused.
pub(super) fn require_polygons<'a>(
    polygons: Option<&'a [Polygon2]>,
    op_name: &str,
) -> Result<&'a [Polygon2], OperationError> {
    polygons
        .filter(|p| !p.is_empty())
        .ok_or_else(|| OperationError::MissingGeometry(format!("{op_name} requires 2D geometry")))
}

pub(super) fn require_mesh<'a>(
    mesh: Option<&'a TriangleMesh>,
    op_name: &str,
) -> Result<&'a TriangleMesh, OperationError> {
    mesh.ok_or_else(|| OperationError::MissingGeometry(format!("{op_name} requires a 3D mesh")))
}

/// R2.4: the spatial-index guard duplicated identically across every
/// mesh-driven family adapter (Adaptive3d, ProjectCurve, Pencil, Scallop,
/// SteepShallow, RampFinish, SpiralFinish, RadialFinish, HorizontalFinish,
/// DropCutter, Waterline) — same refusal shape as [`require_mesh`] /
/// [`require_polygons`], parameterized on the operation name for the
/// error message.
///
/// Every call site passes `op.op_type().name()`. It used to pass a hand
/// written literal per adapter, and those had already drifted from the
/// variant spelling ("Adaptive3D" against `Adaptive3d`).
pub(super) fn require_index<'a>(
    index: Option<&'a SpatialIndex>,
    op_name: &str,
) -> Result<&'a SpatialIndex, OperationError> {
    index.ok_or_else(|| OperationError::Other(format!("{op_name} requires a spatial index")))
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

/// When an op's links need a [`LinkCeiling`] (ruling R1, W1.1).
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

/// Each op's [`LinkCeilingNeed`]. The match is exhaustive on purpose: a new
/// operation does not compile until it states its need.
pub const fn link_ceiling_need(op: OperationType) -> LinkCeilingNeed {
    match op {
        OperationType::ProjectCurve => LinkCeilingNeed::Always,
        OperationType::DropCutter
        | OperationType::Waterline
        | OperationType::Pencil
        | OperationType::Scallop
        | OperationType::UnifiedFinish => LinkCeilingNeed::WithRemainingStock,
        OperationType::Face
        | OperationType::Pocket
        | OperationType::Profile
        | OperationType::Adaptive
        | OperationType::VCarve
        | OperationType::Inlay
        | OperationType::InlayPlug
        | OperationType::Zigzag
        | OperationType::Trace
        | OperationType::Drill
        | OperationType::Chamfer
        | OperationType::Adaptive3d
        | OperationType::SteepShallow
        | OperationType::RampFinish
        | OperationType::SpiralFinish
        | OperationType::RadialFinish
        | OperationType::HorizontalFinish
        | OperationType::AlignmentPinDrill => LinkCeilingNeed::NoLink,
    }
}

/// THE link-ceiling builder (ruling R1, W1.1). Every op that links calls it,
/// so one rule sets the ceiling for every op:
///
/// - `stock` is the remaining stock, when the op has a snapshot;
/// - `tool_radius` is the tool's ENVELOPE radius, the search bound of
///   [`LinkCeiling::required_tip_z`];
/// - `fallback_top_z` is [`crate::dressup::fresh_stock_top_z`], the
///   G-PECKSPLIT rule the entry-descent split also reads.
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
) -> Option<LinkCeiling<'a>> {
    let build = match link_ceiling_need(op) {
        LinkCeilingNeed::Always => true,
        LinkCeilingNeed::WithRemainingStock => stock.is_some(),
        LinkCeilingNeed::NoLink => false,
    };
    build.then(|| LinkCeiling {
        stock,
        tool_radius: tool.envelope_radius_mm(),
        fallback_top_z: crate::dressup::fresh_stock_top_z(stock_top_z, heights_top_z),
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
    )
}
