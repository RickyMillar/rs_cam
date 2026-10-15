//! W6.4: the runner of the Composed-surface shape. The sequence itself
//! (classify, cut each region, route) is `finish::composed::compose`;
//! [`run_composed`] wraps it for the dispatch and owns, for every composed
//! op (Unified Finish, Steep and Shallow):
//!
//! - the cooperative cancel of the context;
//! - the record of the op's report, through the W2.3 record door;
//! - the spans: the op's own labels, then the barriers of its
//!   `order_constraints` (`finish::composed::composed_spans`);
//! - the semantic trace ([`ComposedAnnotate`]);
//! - the side channel (`ComposedSurfaceOp::take_produced`).
//!
//! The adapter builds the op from the context and the territory, and adds
//! any finding that reads the stitched toolpath after the run.

use crate::finish::composed::{Composed, ComposedSurfaceOp, compose, composed_spans};
use crate::toolpath::Toolpath;
use crate::trace::semantic_trace::ToolpathSemanticContext;
use crate::trace::toolpath_spans::Span;

use super::findings::RecordFindings;
use super::shared::generated_with_spans;
use super::{ExecutionContext, GeneratedToolpath, OperationError};

/// The semantic-trace hook of a composed op. It lives on the compute side
/// because the trace writers are compute code.
pub(crate) trait ComposedAnnotate: ComposedSurfaceOp {
    /// Write the semantic trace. `spans` are the final spans of the result.
    fn annotate(
        &self,
        annotations: &[Self::Annotation],
        toolpath: &Toolpath,
        spans: &[Span],
        report: &Self::Report,
        sem: &ToolpathSemanticContext,
    );
}

/// THE runner of a composed op. See the module doc for what it owns.
pub(crate) fn run_composed<O>(
    op: &O,
    ctx: &ExecutionContext<'_>,
) -> Result<GeneratedToolpath, OperationError>
where
    O: ComposedAnnotate,
    O::Report: RecordFindings,
{
    let cancel = ctx.cancel_fn();
    let Composed {
        toolpath,
        annotations,
        mut report,
    } = compose(op, &cancel)?;
    // W2.3: the whole report in one record.
    ctx.record(&report);
    let spans = composed_spans::<O>(&toolpath, &annotations, &report);
    let mut generated = generated_with_spans(toolpath, spans);
    if let Some(sem) = ctx.semantic_ctx {
        op.annotate(
            &annotations,
            &generated.toolpath,
            &generated.spans,
            &report,
            sem,
        );
    }
    generated.produced = O::take_produced(&mut report);
    Ok(generated)
}

impl ComposedAnnotate for crate::finish::unified_finish::UnifiedFinishOp<'_> {
    fn annotate(
        &self,
        annotations: &[Self::Annotation],
        toolpath: &Toolpath,
        _spans: &[Span],
        report: &Self::Report,
        sem: &ToolpathSemanticContext,
    ) {
        // C8: FLAT. Scallop is a sub-generator here, filling one of the
        // planner's mid-steep nodes; the region population this operation
        // publishes is `annotate_unified_finish_regions`' below, and a
        // second one would double-count and break A/M8's 1:1 gate.
        crate::compute::annotate::annotate_scallop(
            annotations,
            toolpath,
            sem,
            crate::compute::annotate::ScallopRegionGrouping::Flat,
        );
        // A/M8: the SEMANTIC region trace `narrate_toolpath` reads, built
        // from the same `RegionAnnotation` table `unified_finish_spans`
        // builds the STRUCTURAL region-node spans from. Annotation only —
        // no move is touched.
        crate::compute::annotate::annotate_unified_finish_regions(
            &crate::finish::unified_finish::unified_finish_region_annotations(report),
            toolpath,
            sem,
        );
    }
}

impl ComposedAnnotate for crate::finish::steep_shallow::SteepShallowOp<'_> {
    /// The generic depth-run annotation over the final spans.
    fn annotate(
        &self,
        _annotations: &[()],
        toolpath: &Toolpath,
        spans: &[Span],
        _report: &Self::Report,
        sem: &ToolpathSemanticContext,
    ) {
        crate::compute::annotate::annotate_depth_run_spans(spans, toolpath, sem);
    }
}
