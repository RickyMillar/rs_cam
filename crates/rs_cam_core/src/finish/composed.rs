//! W6.4: the Composed-surface shape. A composed op classifies the surface
//! into regions, cuts each region with one sub-kernel, and routes the cut
//! regions into one toolpath:
//!
//! ```text
//! classify -> cut_region (one per region) -> route
//! ```
//!
//! Two ops have this shape: Unified Finish (`decompose` regions; waterline,
//! scallop and raster sub-kernels; the greedy router) and Steep and Shallow
//! (two cell-mask regions; its own contour and raster emitters; a fixed
//! order). Pencil does not: it has no classifier, only a detector stage
//! (`finish::rest_detector`).
//!
//! [`compose`] is THE sequence. It polls the cancel before the classifier
//! and before each region, and hands one region memo to every
//! [`ComposedSurfaceOp::cut_region`] call. The op binds its inputs (mesh,
//! cutter, dials, territory) when it is built, so this module reads no
//! execution context. The compute runner
//! (`compute::execute::composed::run_composed`) reads the territory, the
//! link ceiling, the link stage and the rest rule ([`ComposedRest`]) once
//! and gives them to the op it builds. It adds the record, the spans, the
//! semantic trace (its `ComposedAnnotate` hook) and the side channel.
//!
//! # The order constraints
//!
//! A composed toolpath is a stitch of nodes. The dressup TSP may reorder
//! the cutting runs inside a node, but not the nodes, and not the Z ladder
//! of a node that ladders down. [`ComposedSurfaceOp::order_constraints`]
//! states the nodes, and [`composed_spans`] turns them into barriers
//! through the ONE builder `compute::spans::region_node_barriers`. No op
//! builds a barrier list by hand.

use crate::compute::spans::{RegionNode, region_node_barriers};
use crate::geo::P3;
use crate::interrupt::{CancelCheck, Cancelled, check_cancel};
use crate::machine::kinematics::LinkKinematics;
use crate::toolpath::Toolpath;
use crate::trace::produced::Produced;
use crate::trace::toolpath_spans::Span;

/// The rest rule of a composed op on a rest pass (W6.4.6, ruling R-W6.4b):
/// the `RestUse::ContourFilter` rule that the compute runner owns
/// (`standing_material::ContourFilterRule`). Steep and Shallow folds
/// `standing` into the per-point keep of both halves.
pub(crate) struct ComposedRest<'a> {
    /// True when material stands in the reach of the tool at a point. The
    /// point carries the tool tip Z before `stock_to_leave`.
    pub standing: Box<dyn Fn(P3) -> bool + 'a>,
    /// The machine envelope that times a stretch the rule drops
    /// (`ops::waterline::bridge_air_stretches`). `None`: no time model, so
    /// the rule drops no stretch between two kept points.
    pub split_timing: Option<&'a LinkKinematics>,
}

/// An op of the Composed-surface shape. See the module doc.
pub(crate) trait ComposedSurfaceOp {
    /// The classifier output that every region cut and the route read.
    type Classified;
    /// One cut region, as the route reads it.
    type Fragment;
    /// State that the region cuts share (Unified Finish: the shared 0 deg
    /// raster grid). Built empty, once per run.
    type Memo: Default;
    /// The per-move label of the stitched toolpath.
    type Annotation;
    /// The op's report. The classifier makes it; the cuts and the route
    /// add to it.
    type Report;

    /// Classify the surface into regions.
    fn classify(
        &self,
        cancel: &dyn CancelCheck,
    ) -> Result<(Self::Classified, Self::Report), Cancelled>;

    /// The count of regions the classifier made.
    fn region_count(&self, classified: &Self::Classified) -> usize;

    /// Cut region `region`. `None`: the region gives no fragment. The
    /// sub-kernel polls the cancel inside its own loops.
    fn cut_region(
        &self,
        classified: &Self::Classified,
        region: usize,
        memo: &mut Self::Memo,
        report: &mut Self::Report,
        cancel: &dyn CancelCheck,
    ) -> Result<Option<Self::Fragment>, Cancelled>;

    /// Order the fragments and stitch them into one toolpath.
    fn route(
        &self,
        classified: Self::Classified,
        fragments: Vec<Self::Fragment>,
        report: &mut Self::Report,
        cancel: &dyn CancelCheck,
    ) -> Result<Routed<Self::Annotation>, Cancelled>;

    /// The nodes of the stitched toolpath, as the report records them.
    fn order_constraints(report: &Self::Report) -> Vec<RegionNode>;

    /// The op's own spans: labels and node regions, no barriers.
    fn label_spans(
        toolpath: &Toolpath,
        annotations: &[Self::Annotation],
        report: &Self::Report,
    ) -> Vec<Span>;

    /// Move the side channel out of the report. Default: empty.
    fn take_produced(_report: &mut Self::Report) -> Produced {
        Produced::default()
    }
}

/// What [`ComposedSurfaceOp::route`] gives back.
pub(crate) struct Routed<A> {
    pub toolpath: Toolpath,
    pub annotations: Vec<A>,
}

/// The output of [`compose`].
pub(crate) struct Composed<A, R> {
    pub toolpath: Toolpath,
    pub annotations: Vec<A>,
    pub report: R,
}

/// THE sequence of a composed op: classify, cut each region, route.
pub(crate) fn compose<O: ComposedSurfaceOp>(
    op: &O,
    cancel: &dyn CancelCheck,
) -> Result<Composed<O::Annotation, O::Report>, Cancelled> {
    check_cancel(cancel)?;
    let (classified, mut report) = op.classify(cancel)?;
    let mut memo = O::Memo::default();
    let mut fragments = Vec::new();
    for region in 0..op.region_count(&classified) {
        check_cancel(cancel)?;
        if let Some(fragment) =
            op.cut_region(&classified, region, &mut memo, &mut report, cancel)?
        {
            fragments.push(fragment);
        }
    }
    let Routed {
        toolpath,
        annotations,
    } = op.route(classified, fragments, &mut report, cancel)?;
    Ok(Composed {
        toolpath,
        annotations,
        report,
    })
}

/// The spans of a composed result: the op's own spans, then the barriers
/// of its order constraints.
pub(crate) fn composed_spans<O: ComposedSurfaceOp>(
    toolpath: &Toolpath,
    annotations: &[O::Annotation],
    report: &O::Report,
) -> Vec<Span> {
    let mut spans = O::label_spans(toolpath, annotations, report);
    spans.extend(region_node_barriers(
        toolpath,
        &O::order_constraints(report),
    ));
    spans
}
