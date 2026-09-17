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

use super::{GeneratedToolpath, OperationError};

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

pub(super) fn require_polygons(
    polygons: Option<&[Polygon2]>,
) -> Result<&[Polygon2], OperationError> {
    polygons
        .filter(|p| !p.is_empty())
        .ok_or_else(|| OperationError::MissingGeometry("Operation requires 2D geometry".into()))
}

pub(super) fn require_mesh(mesh: Option<&TriangleMesh>) -> Result<&TriangleMesh, OperationError> {
    mesh.ok_or_else(|| OperationError::MissingGeometry("Operation requires a 3D mesh".into()))
}

/// R2.4: the spatial-index guard duplicated identically across every
/// mesh-driven family adapter (Adaptive3d, ProjectCurve, Pencil, Scallop,
/// SteepShallow, RampFinish, SpiralFinish, RadialFinish, HorizontalFinish,
/// DropCutter, Waterline) — same refusal shape as [`require_mesh`] /
/// [`require_polygons`], parameterized on the operation name for the
/// error message.
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
