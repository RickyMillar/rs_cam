//! W6.3: the Height-field shape. One runner drives every op that samples a
//! height field once and emits the result.
//!
//! A height-field op asks one question of a field: "how deep can the tool
//! go at this XY?". The field is the mesh under the drop cutter (Drop
//! Cutter, Scallop, Horizontal, Radial, Spiral and Ramp Finish) or an
//! analytic field over 2D outlines (V-Carve). The op writes one half:
//!
//! - [`HeightFieldOp::plan`] calls the op's kernel with the query, the
//!   territory and the cancel, and returns the moves, the per-move labels
//!   and the kernel's report ([`Planned`]).
//!
//! [`run_height_field`] owns everything else, for every height-field op:
//!
//! - the op's own refusal ([`HeightFieldOp::refuse`]), after the prologue;
//! - the query handle ([`HeightFieldOp::query`]);
//! - the ONE finishing link stage ([`super::shared::finishing_link_stage`]):
//!   the runner builds it, and runs it after the plan or gives it to the
//!   plan, as the op's [`LinkRule`] states;
//! - the record of the kernel's report and of the link totals, through the
//!   W2.3 record door;
//! - the spans, as the op's [`SpanRule`] states, and the semantic trace.
//!
//! # The query is a handle, not a grid
//!
//! The runner does not sample the field. Each kernel samples it itself, at
//! its own resolution: a point drop cutter, a grid at a step the op picks,
//! or the analytic field. One shared grid would move every fingerprint.
//!
//! # What the runner does not do
//!
//! No Z ladder (Ramp keeps its own terrace ladder inside its kernel), no
//! rest step, no boundary clip (the session clips after generation) and
//! no dressup. The kernels poll the cancel; the prologue polled it once
//! before the runner.
//!
//! # Two rules that are information, not inputs
//!
//! - Scallop links INSIDE its kernel ([`LinkRule::InPlan`]): its ring
//!   annotations carry move indices, and the kernel reconciles them beside
//!   the relink. The runner owns the stage (one builder, one ceiling, one
//!   boundary, one kinematics); the kernel only runs it.
//! - The territory's `SurfaceMask::Flat` states the threshold of Horizontal
//!   Finish, but that kernel tests triangle normals, not slope cells. Do not
//!   switch it to a cell mask without a re-bless of G-OPGOLDEN.

use crate::compute::catalog::OperationType;
use crate::compute::spans::{
    RuntimeLabel, runtime_annotations_to_labels, spans_from_labeled_events,
};
use crate::finish::surface_link::{FinishingLinkStage, LinkGeometry};
use crate::interrupt::CancelCheck;
use crate::polygon::Polygon2;
use crate::tool::ToolDefinition;
use crate::toolpath::Toolpath;
use crate::trace::debug_trace::ToolpathDebugContext;
use crate::trace::produced::Produced;
use crate::trace::semantic_trace::ToolpathSemanticContext;

use super::findings::RecordFindings;
use super::shared::{
    finishing_link_stage, generated_with_cut_run_spans, generated_with_spans, relink_in_adapter,
    with_depth_run_annotation,
};
use super::territory::Territory;
use super::{ExecutionContext, GeneratedToolpath, MeshInput, OperationError, Prepared};

/// What a height-field kernel queries. A handle: the kernel samples it
/// itself, at its own resolution.
#[derive(Clone, Copy)]
pub(crate) enum HeightQuery<'a> {
    /// The mesh drop cutter: the mesh, its index and the op's cutter. A
    /// kernel can also read the push cutter or the triangle normals from
    /// the same handle (Ramp, Horizontal).
    Mesh {
        surface: MeshInput<'a>,
        cutter: &'a ToolDefinition,
    },
    /// An analytic Z field over 2D outlines (V-Carve). No mesh.
    Analytic(&'a dyn AnalyticField),
}

impl<'a> HeightQuery<'a> {
    /// The mesh arm. Another arm is a registry defect: the op asks for a
    /// field its own `query` does not give.
    pub(crate) fn mesh(
        self,
        op: OperationType,
    ) -> Result<(MeshInput<'a>, &'a ToolDefinition), OperationError> {
        match self {
            Self::Mesh { surface, cutter } => Ok((surface, cutter)),
            Self::Analytic(_) => Err(query_mismatch(op, "a mesh")),
        }
    }

    /// The analytic arm. Another arm is a registry defect.
    pub(crate) fn analytic(
        self,
        op: OperationType,
    ) -> Result<&'a dyn AnalyticField, OperationError> {
        match self {
            Self::Analytic(field) => Ok(field),
            Self::Mesh { .. } => Err(query_mismatch(op, "an analytic field")),
        }
    }
}

fn query_mismatch(op: OperationType, what: &str) -> OperationError {
    OperationError::Other(format!(
        "height-field mismatch: {} plans on {what}, but its query gives another field",
        op.name()
    ))
}

/// An analytic height field over 2D outlines (V-Carve's V-depth field).
/// The kernel holds the depth formula; the field holds its domain.
pub(crate) trait AnalyticField {
    /// The outlines the field is defined over, in input order.
    fn outlines(&self) -> &[Polygon2];
}

/// What [`HeightFieldOp::plan`] gets besides the query and the territory.
pub(crate) struct PlanCtx<'c> {
    /// The cooperative cancel. Every kernel polls it.
    pub cancel: &'c (dyn CancelCheck + Sync),
    /// The debug trace context, for a kernel that writes one.
    pub debug: Option<&'c ToolpathDebugContext>,
    /// The shared finishing link stage, for an op whose link rule is
    /// [`LinkRule::InPlan`] and whose hookup is above zero. `None` for
    /// every other rule.
    pub link_stage: Option<&'c FinishingLinkStage<'c>>,
}

/// What [`HeightFieldOp::plan`] returns.
pub(crate) struct Planned<A, R> {
    /// The planned moves.
    pub toolpath: Toolpath,
    /// The per-move labels of [`SpanRule::LabeledEvents`]. Empty for
    /// [`SpanRule::CutRuns`].
    pub annotations: Vec<A>,
    /// The kernel's report. The runner records it, always.
    pub report: R,
    /// The link geometry the plan used, for [`LinkRule::AfterPlan`]. Only
    /// the plan knows its sampling (Drop Cutter: its grid step).
    pub link_geometry: Option<LinkGeometry>,
    /// The W6.2 side channel. Empty for every op of this shape today.
    pub produced: Produced,
}

impl<A, R> Planned<A, R> {
    /// The moves and the report, with no labels, no link geometry and an
    /// empty side channel.
    pub(crate) fn new(toolpath: Toolpath, report: R) -> Self {
        Self {
            toolpath,
            annotations: Vec::new(),
            report,
            link_geometry: None,
            produced: Produced::default(),
        }
    }

    /// The same plan with these per-move labels.
    pub(crate) fn with_annotations(mut self, annotations: Vec<A>) -> Self {
        self.annotations = annotations;
        self
    }

    /// The same plan with this link geometry.
    pub(crate) fn with_link_geometry(mut self, geometry: LinkGeometry) -> Self {
        self.link_geometry = Some(geometry);
        self
    }
}

/// Where the finishing link stage runs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum LinkRule {
    /// No finishing link stage.
    Off,
    /// The runner relinks the planned toolpath when `hookup_mm > 0`, and
    /// records the totals only when the stage ran (G-LINKVISIBLE). The plan
    /// returns its [`Planned::link_geometry`]. Drop Cutter.
    AfterPlan { hookup_mm: f64 },
    /// The runner builds the stage and gives it to the plan, which links
    /// inside its kernel (module doc). Offset-cascade Scallop.
    InPlan { hookup_mm: f64 },
}

/// How the runner builds the spans.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SpanRule {
    /// One span per cutting run, with this label prefix, and the depth-run
    /// annotation of the semantic trace.
    CutRuns(&'static str),
    /// One region span per planned label ([`Planned::annotations`]), and
    /// the op's [`HeightFieldOp::annotate`] hook. No depth-run annotation.
    LabeledEvents,
}

/// The label type of an op with no per-move labels ([`SpanRule::CutRuns`]).
/// It has no value, so such a plan cannot return a label.
pub(crate) enum NoLabel {}

impl RuntimeLabel for NoLabel {
    fn move_index(&self) -> usize {
        match *self {}
    }

    fn label(&self) -> String {
        match *self {}
    }
}

/// An op of the Height-field shape. See the module doc.
pub(crate) trait HeightFieldOp {
    /// The per-move label of the kernel. [`NoLabel`] for a
    /// [`SpanRule::CutRuns`] op.
    type Annotation: RuntimeLabel;
    /// The kernel's report. `()` for none.
    type Report: RecordFindings;

    fn op_type(&self) -> OperationType;

    /// The query handle. Default: the mesh the prologue checked and the
    /// op's cutter.
    fn query<'q>(
        &'q self,
        ctx: &'q ExecutionContext<'_>,
        prepared: &'q Prepared<'_>,
    ) -> Result<HeightQuery<'q>, OperationError> {
        Ok(HeightQuery::Mesh {
            surface: prepared.mesh()?,
            cutter: ctx.tool_def,
        })
    }

    /// A refusal the prologue does not cover. Default: none.
    fn refuse(&self) -> Result<(), OperationError> {
        Ok(())
    }

    /// Where the link stage runs. Default: [`LinkRule::Off`].
    fn link_rule(&self) -> LinkRule {
        LinkRule::Off
    }

    fn span_rule(&self) -> SpanRule;

    /// Read the labels into the semantic trace ([`SpanRule::LabeledEvents`]
    /// only). Default: nothing.
    fn annotate(
        &self,
        _annotations: &[Self::Annotation],
        _toolpath: &Toolpath,
        _sem: &ToolpathSemanticContext,
    ) {
    }

    /// Call the kernel. The territory is a planner INPUT: a kernel that
    /// keeps samples by region reads it here, before it plans.
    fn plan(
        &self,
        query: HeightQuery<'_>,
        territory: &Territory<'_>,
        pctx: &PlanCtx<'_>,
    ) -> Result<Planned<Self::Annotation, Self::Report>, OperationError>;
}

/// THE runner of a height-field op. See the module doc for what it owns.
pub(crate) fn run_height_field<O: HeightFieldOp>(
    op: &O,
    ctx: &ExecutionContext<'_>,
    prepared: &Prepared<'_>,
) -> Result<GeneratedToolpath, OperationError> {
    op.refuse()?;
    let territory = prepared.territory();
    let query = op.query(ctx, prepared)?;
    let rule = op.link_rule();
    let stage = match rule {
        LinkRule::Off => None,
        LinkRule::AfterPlan { hookup_mm } | LinkRule::InPlan { hookup_mm } => {
            finishing_link_stage(op.op_type(), ctx, territory.regions(), hookup_mm)
        }
    };
    let cancel = ctx.cancel_fn();
    let pctx = PlanCtx {
        cancel: &cancel,
        debug: ctx.debug_ctx,
        link_stage: match rule {
            LinkRule::InPlan { .. } => stage.as_ref(),
            LinkRule::Off | LinkRule::AfterPlan { .. } => None,
        },
    };
    let Planned {
        mut toolpath,
        annotations,
        report,
        link_geometry,
        produced,
    } = op.plan(query, territory, &pctx)?;
    // W2.3: always, an inert report too (PR-8b: Ramp's reach clamp).
    ctx.record(&report);
    if let (LinkRule::AfterPlan { .. }, Some(stage)) = (rule, stage.as_ref()) {
        let geometry = link_geometry.ok_or_else(|| {
            OperationError::Other(format!(
                "height-field defect: {} links after its plan but returns no link geometry",
                op.op_type().name()
            ))
        })?;
        let (surface, cutter) = query.mesh(op.op_type())?;
        let (linked, totals) = relink_in_adapter(
            stage,
            &geometry,
            surface.mesh,
            surface.index,
            cutter,
            toolpath,
            op.op_type().kind_str(),
        );
        // G-LINKVISIBLE: only a stage that ran records its totals.
        ctx.record(&totals);
        toolpath = linked;
    }
    let mut generated = match op.span_rule() {
        SpanRule::CutRuns(label) => with_depth_run_annotation(
            generated_with_cut_run_spans(toolpath, label),
            ctx.semantic_ctx,
        ),
        SpanRule::LabeledEvents => {
            if let Some(sem) = ctx.semantic_ctx {
                op.annotate(&annotations, &toolpath, sem);
            }
            let spans = spans_from_labeled_events(
                toolpath.moves.len(),
                runtime_annotations_to_labels(&annotations),
            );
            generated_with_spans(toolpath, spans)
        }
    };
    generated.produced = produced;
    Ok(generated)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    //! The runner's own contract: it records the report, it records the
    //! link totals only when the stage ran, and it keeps the span labels.

    use std::cell::RefCell;
    use std::sync::atomic::AtomicBool;

    use super::*;
    use crate::compute::catalog::OperationConfig;
    use crate::compute::config::ResolvedHeights;
    use crate::compute::cutter::build_cutter;
    use crate::compute::execute::GenerationFindings;
    use crate::compute::tool_config::{ToolConfig, ToolId, ToolType};
    use crate::finish::ramp_finish::RampReachClamp;
    use crate::geo::{BoundingBox3, P3};
    use crate::mesh::{SpatialIndex, make_test_hemisphere};
    use crate::toolpath::MoveIntent;

    /// A test label: the move it opens and its text.
    struct Label(usize, &'static str);

    impl RuntimeLabel for Label {
        fn move_index(&self) -> usize {
            self.0
        }

        fn label(&self) -> String {
            self.1.to_owned()
        }
    }

    /// The report of the test op: the reach clamp, which records into its
    /// own findings slot.
    fn clamp() -> RampReachClamp {
        RampReachClamp {
            clamped_points: 3,
            ramp_points: 7,
            max_lift_mm: 0.5,
            requested_bottom_z_mm: 0.0,
            holdable_bottom_z_mm: 0.5,
            ..RampReachClamp::default()
        }
    }

    /// A test op: two separate cut runs over the hemisphere top, labelled
    /// "first" and "second".
    struct TwoRuns {
        link: LinkRule,
        spans: SpanRule,
    }

    impl HeightFieldOp for TwoRuns {
        type Annotation = Label;
        type Report = RampReachClamp;

        fn op_type(&self) -> OperationType {
            OperationType::DropCutter
        }

        fn link_rule(&self) -> LinkRule {
            self.link
        }

        fn span_rule(&self) -> SpanRule {
            self.spans
        }

        fn plan(
            &self,
            query: HeightQuery<'_>,
            _territory: &Territory<'_>,
            _pctx: &PlanCtx<'_>,
        ) -> Result<Planned<Label, RampReachClamp>, OperationError> {
            let _ = query.mesh(self.op_type())?;
            let mut tp = Toolpath::new();
            tp.rapid_to(P3::new(-2.0, 0.0, 30.0));
            tp.feed_to(P3::new(-2.0, 0.0, 24.0), 500.0);
            tp.feed_to(P3::new(-1.0, 0.0, 24.0), 1000.0);
            tp.rapid_to(P3::new(-1.0, 0.0, 30.0));
            tp.rapid_to(P3::new(1.0, 0.0, 30.0));
            tp.feed_to(P3::new(1.0, 0.0, 24.0), 500.0);
            tp.feed_to(P3::new(2.0, 0.0, 24.0), 1000.0);
            tp.rapid_to(P3::new(2.0, 0.0, 30.0));
            for m in &mut tp.moves {
                m.intent = MoveIntent::FinishingCut;
            }
            Ok(Planned::new(tp, clamp())
                .with_annotations(vec![Label(0, "first"), Label(4, "second")])
                .with_link_geometry(LinkGeometry {
                    stock_to_leave: 0.0,
                    sampling: 0.5,
                    feed_rate: 1000.0,
                    plunge_rate: 500.0,
                    safe_z: 30.0,
                    link_tolerance: None,
                }))
        }
    }

    /// Run `op` on a hemisphere and give the result and the findings.
    fn run(op: &TwoRuns) -> (GeneratedToolpath, GenerationFindings) {
        let heights = ResolvedHeights {
            clearance_z: 35.0,
            retract_z: 30.0,
            feed_z: 28.0,
            top_z: 25.0,
            bottom_z: 0.0,
            top_pinned: false,
            bottom_pinned: false,
        };
        let bbox = BoundingBox3 {
            min: P3::new(-25.0, -25.0, 0.0),
            max: P3::new(25.0, 25.0, 25.0),
        };
        let mesh = make_test_hemisphere(25.0, 16);
        let index = SpatialIndex::build_auto(&mesh);
        let tool_cfg = ToolConfig::new_default(ToolId(0), ToolType::BallNose);
        let tool_def = build_cutter(&tool_cfg);
        let cancel = AtomicBool::new(false);
        let findings = RefCell::new(GenerationFindings::default());
        let ctx = ExecutionContext {
            mesh: Some(&mesh),
            index: Some(&index),
            ..ExecutionContext::new(
                &findings,
                &tool_def,
                &tool_cfg,
                &heights,
                &[],
                &bbox,
                &cancel,
            )
        };
        let prepared = crate::compute::execute::prologue::prepare(
            &ctx,
            &OperationConfig::new_default(OperationType::DropCutter),
        )
        .unwrap();
        let generated = run_height_field(op, &ctx, &prepared).unwrap();
        drop(ctx);
        (generated, findings.into_inner())
    }

    #[test]
    fn the_runner_records_the_report_and_keeps_the_labels() {
        let (generated, findings) = run(&TwoRuns {
            link: LinkRule::Off,
            spans: SpanRule::LabeledEvents,
        });
        assert_eq!(findings.ramp_reach_clamp, Some(clamp()));
        assert!(findings.relink.is_none(), "no stage, no link totals");
        let labels: Vec<&str> = generated.spans.iter().map(|s| s.label.as_ref()).collect();
        assert!(
            labels.contains(&"first") && labels.contains(&"second"),
            "the planned labels reach the spans: {labels:?}"
        );
    }

    #[test]
    fn the_runner_records_link_totals_only_when_the_stage_ran() {
        let (_, off) = run(&TwoRuns {
            link: LinkRule::AfterPlan { hookup_mm: 0.0 },
            spans: SpanRule::CutRuns("Test run"),
        });
        assert!(off.relink.is_none(), "a zero hookup runs no stage");
        let (generated, on) = run(&TwoRuns {
            link: LinkRule::AfterPlan { hookup_mm: 5.0 },
            spans: SpanRule::CutRuns("Test run"),
        });
        assert!(on.relink.is_some(), "a stage that ran records its totals");
        assert!(
            generated
                .spans
                .iter()
                .any(|s| s.label.starts_with("Test run")),
            "the cut-run spans carry the op's label"
        );
    }
}
