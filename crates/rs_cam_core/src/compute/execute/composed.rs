//! W6.4: the runner of the Composed-surface shape. The sequence itself
//! (classify, cut each region, route) is `finish::composed::compose`;
//! [`run_composed`] wraps it for the dispatch and owns, for every composed
//! op (Unified Finish, Steep and Shallow):
//!
//! - the reads of the territory, once ([`ComposedInputs`]): the mesh, the
//!   boundary regions, the reference stock, the one link ceiling
//!   (`shared::op_link_ceiling`), the one link stage
//!   (`shared::finishing_link_stage`, [`ComposedLink`]) and the
//!   `ContourFilter` rest rule (`standing_material::ContourFilterRule`);
//! - the cooperative cancel of the context;
//! - the record of the op's report, through the W2.3 record door;
//! - the spans: the op's own labels, then the barriers of its
//!   `order_constraints` (`finish::composed::composed_spans`);
//! - the semantic trace ([`ComposedAnnotate`]);
//! - the side channel (`ComposedSurfaceOp::take_produced`).
//!
//! The adapter builds the op from its config and the [`ComposedInputs`],
//! and adds any finding that reads the stitched toolpath after the run.

use crate::dexel_stock::TriDexelStock;
use crate::finish::composed::{Composed, ComposedRest, ComposedSurfaceOp, compose, composed_spans};
use crate::finish::surface_link::{FinishingLinkStage, LinkCeiling};
use crate::geometry::region_set::RegionSet;
use crate::machine::kinematics::LinkKinematics;
use crate::tool::MillingCutter;
use crate::toolpath::Toolpath;
use crate::trace::semantic_trace::ToolpathSemanticContext;
use crate::trace::toolpath_spans::Span;

use super::findings::RecordFindings;
use super::shared::{finishing_link_stage, generated_with_spans, op_link_ceiling};
use super::standing_material::ContourFilterRule;
use super::{ExecutionContext, GeneratedToolpath, MeshInput, OperationError, Prepared};

/// How a composed op links.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum ComposedLink {
    /// The op relinks inside its own kernel and reads only the one link
    /// ceiling ([`ComposedInputs::link_ceiling`]). Unified Finish.
    CeilingOnly,
    /// The op runs the shared finishing link stage
    /// ([`ComposedInputs::link_stage`]) with this cap (mm). `<= 0.0`: no
    /// stage. Steep and Shallow (`hookup_mm`).
    Stage { hookup_mm: f64 },
}

/// What [`run_composed`] reads for the op, once, from the territory and
/// the context. The adapter builds the op from it.
pub(crate) struct ComposedInputs<'c> {
    /// The mesh the prologue checked.
    pub mesh: MeshInput<'c>,
    /// The boundary regions (the territory's XY part).
    pub boundary: Option<&'c RegionSet<'c>>,
    /// The remaining stock after the W2.1 guard
    /// (`Territory::reference_stock`). `None`: a fresh op, or no reference.
    pub reference_stock: Option<&'c TriDexelStock>,
    /// The one link ceiling (`shared::op_link_ceiling`, ruling R1).
    pub link_ceiling: Option<LinkCeiling<'c>>,
    /// The one link stage ([`ComposedLink::Stage`] with a cap above zero).
    pub link_stage: Option<FinishingLinkStage<'c>>,
    /// The `ContourFilter` rule on a rest pass of an op whose rest rule is
    /// `ContourFilter`. `None` for every other op and on fresh stock.
    pub rest: Option<ComposedRest<'c>>,
    /// The machine envelope of the context.
    pub link_kinematics: Option<&'c LinkKinematics>,
}

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
/// `build` makes the op from the [`ComposedInputs`].
pub(crate) fn run_composed<'c, O, B>(
    ctx: &'c ExecutionContext<'_>,
    prepared: &'c Prepared<'_>,
    link: ComposedLink,
    build: B,
) -> Result<GeneratedToolpath, OperationError>
where
    B: FnOnce(ComposedInputs<'c>) -> O,
    O: ComposedAnnotate,
    O::Report: RecordFindings,
{
    let op_type = prepared.op_type();
    let territory = prepared.territory();
    let rule = ContourFilterRule::resolve(
        op_type.capabilities().rest_use,
        territory,
        ctx.tool_def.radius(),
        ctx.link_kinematics.as_ref(),
    );
    let op = build(ComposedInputs {
        mesh: prepared.mesh()?,
        boundary: territory.regions(),
        reference_stock: territory.reference_stock(),
        link_ceiling: op_link_ceiling(op_type, ctx, territory),
        link_stage: match link {
            ComposedLink::CeilingOnly => None,
            ComposedLink::Stage { hookup_mm } => {
                finishing_link_stage(op_type, ctx, territory, hookup_mm)
            }
        },
        rest: rule.map(|rule| ComposedRest {
            standing: Box::new(move |p| rule.keeps(p)),
            split_timing: rule.split_timing(),
        }),
        link_kinematics: ctx.link_kinematics.as_ref(),
    });
    let cancel = ctx.cancel_fn();
    let Composed {
        toolpath,
        annotations,
        mut report,
    } = compose(&op, &cancel)?;
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
