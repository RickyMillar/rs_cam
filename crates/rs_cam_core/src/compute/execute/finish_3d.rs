//! The 3D finishing family: pencil, unified finish and the five
//! single-strategy finishers. 3D Rough moved to `rough_3d.rs` (W6.5).
//!
//! Horizontal, Radial, Spiral and Ramp Finish run on the height-field
//! runner (`height_field.rs`, W6.3). Split out of `compute/execute.rs` (P4).

use crate::compute::catalog::{OperationConfig, OperationType};
use crate::compute::operation_configs::OpMotion;
use crate::tool::MillingCutter;
use crate::toolpath::Toolpath;
use crate::trace::semantic_trace::ToolpathSemanticContext;

use super::composed::{ComposedLink, run_composed};
use super::height_field::{
    HeightFieldOp, HeightQuery, NoLabel, PlanCtx, Planned, SpanRule, run_height_field,
};
use super::shared::{generated_with_spans, op_link_ceiling};
use super::territory::Territory;
use super::{ExecutionContext, GeneratedToolpath, MeshInput, OperationError, Prepared};

/// Pencil family adapter (labeled-event spans + annotate_pencil).
/// Cancellable: the cooperative cancel closure is built by
/// `ctx.cancel_fn()` (pinned by `every_generator_polls_a_preset_cancel_flag_past_the_prologue`).
pub(crate) fn generate_pencil(
    ctx: &ExecutionContext<'_>,
    prepared: &Prepared<'_>,
    op: &OperationConfig,
) -> Result<GeneratedToolpath, OperationError> {
    let cfg = config_guard!(op, Pencil, "generate_pencil");
    let MeshInput {
        mesh: m,
        index: idx,
    } = prepared.mesh()?;
    let params = cfg.params(
        OpMotion {
            feed_rate: op.feed_rate(),
            plunge_rate: op.plunge_rate(),
            safe_z: ctx.heights.retract_z,
        },
        ctx.reference_tool_cfg
            .as_ref()
            .map(crate::compute::cutter::build_cutter)
            .transpose()?,
        ctx.link_kinematics.clone(),
    );
    let (tp, annotations, report) =
        crate::finish::pencil::pencil_toolpath_structured_annotated_with_cancel(
            m,
            idx,
            ctx.tool_def,
            &params,
            // The prior-op machined stock (FromRemainingStock + a prior sim)
            // for the entry ramps, and (R2) the same stock after the W2.1
            // guard, which the territory resolved once (W6.2). The RestDepth
            // detector prefers it as the rest reference.
            crate::finish::pencil::PencilStock {
                remaining: prepared.territory().unguarded_stock(),
                rest_reference: prepared.territory().reference_stock(),
                // R1 / C8: the link lift reads the one builder's ceiling
                // (its fallback top and its obstacles). `None` on fresh
                // stock, where the lift never runs.
                link_ceiling: op_link_ceiling(op.op_type(), ctx, prepared.territory()),
            },
            ctx.debug_ctx,
            &ctx.cancel_fn(),
        )?;
    // Wave D1: the tip float (a centreline op always MEASURES it, zero
    // included) and G-LINKVISIBLE: the link report, `Some` exactly when the
    // emitter ran.
    ctx.record(&report);
    if let Some(sem) = ctx.semantic_ctx {
        crate::compute::annotate::annotate_pencil(&annotations, &tp, sem);
    }
    let spans = crate::compute::spans::spans_from_labeled_events(
        tp.moves.len(),
        annotations
            .iter()
            .map(|ann| (ann.move_index, ann.event.label())),
    );
    let mut generated = generated_with_spans(tp, spans);
    // W6.2: the RestDepth detector's rest grid (the GUI overlay) and its
    // derived region polygons (the P2.2 selective-finishing boundary
    // source), when that detector ran.
    generated.produced = report.produced;
    Ok(generated)
}

/// UnifiedFinish family adapter (P2.c orchestrator —
/// `planning/unified_finish_planner_design.md`): `UnifiedFinishOp` on the
/// composed runner. The adapter builds the params, the P2.b
/// `FinishPlannerParams::for_tool` base with the op's own dials on top
/// (one-new-dial rule — everything else inherits the tool-derived
/// conditioning defaults) and the claims config. The runner reads the
/// territory and the one link ceiling (`ComposedLink::CeilingOnly`).
pub(crate) fn generate_unified_finish(
    ctx: &ExecutionContext<'_>,
    prepared: &Prepared<'_>,
    op: &OperationConfig,
) -> Result<GeneratedToolpath, OperationError> {
    let cfg = config_guard!(op, UnifiedFinish, "generate_unified_finish");
    let params = cfg.params(OpMotion {
        feed_rate: op.feed_rate(),
        plunge_rate: op.plunge_rate(),
        safe_z: ctx.heights.retract_z,
    });
    // F2: the `for_tool` derivation plus this op's own dials, built in ONE
    // place — `UnifiedFinishConfig::planner_params`, whose doc carries the
    // cusp-vs-envelope rule this line used to carry and the `None` =
    // derive contract of the three island-filter overrides.
    //
    // `cusp_radius_mm()`, NOT `radius()`: on a tapered ball the latter is the
    // SHAFT radius, and every dial `for_tool` derives is a feature scale.
    let planner = cfg.planner_params(ctx.tool_def.cusp_radius_mm());
    // F4: the claims dials are read from here down. Report — do not refuse,
    // do not start applying — when the operator dialled a rest-territory
    // number that this configuration never applies.
    ctx.record(&crate::compute::toolpath_stats::InertClaimsDialFinding::of_config(cfg));

    // The claims config ([`unified_claims_config`]). The op borrows it, and
    // the A4 finding below reads it after the run, so it lives in this
    // cell. The runner's build closure fills it from the inputs.
    let claims_cell = std::cell::OnceCell::new();
    // W6.4: the composed op. `run_composed` records the whole report (the
    // link totals and the C2 cells only when their pass ran, the cascade
    // residual, the dropped and clipped bands, the crease tip float and the
    // derived stepovers — the `UnifiedFinishReport` impl in `findings.rs`),
    // builds the spans (the Region attribution, then the barriers of
    // `order_constraints`), writes the semantic trace and moves the claims
    // detector's rest output onto the result.
    let generated = run_composed(ctx, prepared, ComposedLink::CeilingOnly, |inputs| {
        let claims = claims_cell
            .get_or_init(|| {
                cfg.pencil_claims
                    .then(|| unified_claims_config(ctx, cfg, inputs.reference_stock))
            })
            .as_ref();
        // B3 / G-LINKLOAD (Phase O item 3): an intra-region link rides the MESH,
        // which is the material only on a fresh-stock pass. This op is handed
        // the remaining stock exactly when it cuts what a prior op left — there,
        // material stands above the design surface wherever nothing has cut yet,
        // and a surface-riding link is a cutting feed straight through it.
        //
        // `None` when no snapshot is in scope, which is the byte-identical
        // fresh-stock arm (`tests/island_stay_down_links_o3.rs` pins that the
        // two entry points agree).
        //
        // The ENVELOPE radius is the SEARCH BOUND — the furthest lateral offset at
        // which material could reach this cutter at all — and it is no longer the
        // whole answer. Inside it the tool's own profile decides
        // (`LinkCeiling::required_tip_z`): past its tip a cutter RISES, so material
        // at offset `r` can only strike it if it stands more than
        // `height_at_radius(r)` above the tip. This comment used to justify a flat
        // disc with "material anywhere under the tool is material the tool will
        // hit, and on a tapered ball the shank is what would strike it" — that is
        // the reasoning the profile rule corrects. Measured on the operator's
        // 200x200x9.81 relief with the shipped R1.0 tapered ball, the shank stands
        // 10.97 mm above the tip at r = 2.0, above the board's entire relief: only
        // material within ~1.5 mm laterally can touch it, so the flat disc
        // over-reached 2x in radius and lifted every link to the height of ridges
        // that cannot contact the cutter (one region 898 s -> 1411 s, 1.57x, with
        // the path held identical). For a FLAT endmill the profile rule is
        // byte-identical to the flat disc, which is the safety anchor.
        // R1: the one builder (`op_link_ceiling`) sets all three values; the
        // runner calls it (`ComposedInputs::link_ceiling`).
        crate::finish::unified_finish::UnifiedFinishOp {
            mesh: inputs.mesh.mesh,
            index: inputs.mesh.index,
            cutter: ctx.tool_def,
            top_z: ctx.heights.top_z,
            bottom_z: ctx.heights.bottom_z,
            params: &params,
            planner: &planner,
            machining_boundary: inputs.boundary,
            // P2.d: cost the region route against the real machine envelope
            // when one is in scope (same plumbing as pencil's P1 W4a hookup).
            link_kinematics: inputs.link_kinematics,
            claims,
            debug: ctx.debug_ctx,
            link_ceiling: inputs.link_ceiling,
        }
    })?;
    // A4: the pass has just been planned against a reference stock, and this
    // is the only point where the emitted geometry and that reference are
    // both in scope. Gated on the reference having RESOLVED to a real
    // machined prior — under a self-probe reference `territory_clip` never
    // ran and the op is not a rest pass in the sense this finding is about.
    if let Some(cfg) = claims_cell.get().and_then(Option::as_ref)
        && matches!(
            cfg.crease_reference,
            crate::finish::unified_finish::CreaseReference::MachinedStock
        )
        && let Some(stock) = cfg.territory_stock
    {
        ctx.record(
            &crate::compute::toolpath_stats::ZeroRemovalFinding::measure(
                &generated.toolpath,
                stock,
                ctx.tool_def,
            ),
        );
    }
    Ok(generated)
}

/// The claims config of a Unified Finish with `pencil_claims` on, over
/// `territory_stock` (the runner's `ComposedInputs::reference_stock`). It
/// records the `ClaimsReferenceFinding`.
fn unified_claims_config<'s>(
    ctx: &ExecutionContext<'_>,
    cfg: &crate::compute::operation_configs::UnifiedFinishConfig,
    territory_stock: Option<&'s crate::dexel_stock::TriDexelStock>,
) -> crate::finish::unified_finish::ClaimsConfig<'s> {
    // The stock always feeds the TERRITORY mask; it ALSO feeds crease
    // detection when the resolved reference is
    // `CreaseReference::MachinedStock` (build-list item 3 —
    // `CreaseReference` doc). W6.2: the territory resolved it once,
    // through the W2.1 stock-vs-mesh guard (the runner's
    // `ComposedInputs::reference_stock`); this op has no tool tier
    // after it (`CreaseReference` decides).
    // A/M6: resolve the three-valued DIAL into the two-valued field the
    // detector switches on, HERE — this is the only site that can see
    // both the operator's setting and what is actually in scope. The
    // resolution is recorded whatever it is: `Auto`'s derivation is
    // invisible in every config surface, and pinning `self_probe` over a
    // real machined prior is the A/M6 footgun measured at −88.7% cutting
    // when corrected.
    //
    // The "prior wanted but absent" case does NOT get a second signal
    // invented for it here. An op that cuts `FromRemainingStock` with no
    // snapshot never reaches this function at all — it is refused
    // upstream as `ComputeStatus::AwaitingPriorStock` (A/M11), which is
    // the taxonomy for blocked-on-sequencing. What reaches here with no
    // stock in scope is an op that asked for FRESH stock, and that is a
    // configuration statement, not a block; the finding's `why()` names
    // both remedies.
    let resolution = crate::finish::unified_finish::ClaimsReferenceResolution::resolve(
        cfg.claims_reference,
        territory_stock.is_some(),
    );
    ctx.record(&crate::compute::toolpath_stats::ClaimsReferenceFinding {
        resolution,
        territory_clip_requested: cfg.territory_clip,
    });
    // Detector tuning: the one map of the `rest_analysis` dials
    // (`dressup_apply::rest_field_params`, which the generic rest analysis
    // reads too), else the detector's own defaults. The claims pipeline
    // reads only the cell, the depth, the margin and `min_cut_length`; it
    // routes on its own fan (`unified_finish_toolpath_with_cancel`).
    let mut rest_field_params = ctx.rest_analysis.map_or_else(
        crate::surface::rest_field::RestFieldParams::default,
        super::dressup_apply::rest_field_params,
    );
    // S4 threshold coupling (`unified_finish::ClaimsConfig::
    // territory_clip` doc): the detector's own rest field (whose
    // valleys are gated on `rest_field_params.min_valley_depth`) feeds
    // the S4 mask-AND, which thresholds it at a DIFFERENT value —
    // `cfg.min_rest_depth_mm` (the S4 per-cell territory gate) —
    // unless we floor it here. Left to disagree, the mask-AND would
    // confine generation to islands measuring a different "rest" than
    // the one S4's mask-AND decided was worth keeping. Only floors
    // when no deliberate `rest_analysis` dial is
    // in scope: the session populates `ctx.rest_analysis`
    // UNCONDITIONALLY from the toolpath config (`session/compute.rs`),
    // so mere presence is not intent — an untouched default block
    // (`enabled: false`, dials == `RestFieldParams::default()`) must
    // not shadow the coupling. `enabled` is the explicit-override
    // signal, same as `attach_generic_rest_analysis` keys on.
    if cfg.territory_clip && !ctx.rest_analysis.is_some_and(|ra| ra.enabled) {
        rest_field_params.min_valley_depth = cfg.min_rest_depth_mm;
    }
    crate::finish::unified_finish::ClaimsConfig {
        territory_stock,
        // A/M6: the RESOLVED reference, never the raw dial. `ClaimsConfig`
        // takes `CreaseReference` (two-valued, what the detector reads);
        // `UnifiedFinishConfig::claims_reference` is `ClaimsReference`
        // (three-valued, what the operator asked for). The resolution
        // above is the only bridge, and it is recorded.
        crease_reference: resolution.reference(),
        rest_field_params,
        min_rest_depth_mm: cfg.min_rest_depth_mm,
        territory_clip: cfg.territory_clip,
        crease_hookup_mm: cfg.crease_hookup_mm,
    }
}

/// SteepShallow family adapter (W6.4.4): `SteepShallowOp` on the composed
/// runner. Cancellable: the cooperative cancel closure is built by
/// `ctx.cancel_fn()` (pinned by
/// `every_generator_polls_a_preset_cancel_flag_past_the_prologue`).
///
/// W6.4.6 (ruling R-W6.4b): on a rest pass both halves keep only the
/// points that pass material in the remaining stock: the Waterline rule
/// (`RestUse::ContourFilter`), which the runner owns and gives the op as
/// `ComposedInputs::rest`. The `hookup_mm` link stage is the runner's
/// (`ComposedLink::Stage`), with the one link ceiling.
pub(crate) fn generate_steep_shallow(
    ctx: &ExecutionContext<'_>,
    prepared: &Prepared<'_>,
    op: &OperationConfig,
) -> Result<GeneratedToolpath, OperationError> {
    let cfg = config_guard!(op, SteepShallow, "generate_steep_shallow");
    let params = cfg.params(OpMotion {
        feed_rate: op.feed_rate(),
        plunge_rate: op.plunge_rate(),
        safe_z: ctx.heights.retract_z,
    });
    // W6.4.4: the composed op. Two concatenated passes, not one continuous
    // trace: the barriers of its `order_constraints` keep the halves in
    // their emitted order and keep the steep half's Z ladder, which is what
    // lets `UnifiedFinish`-style intra-node reordering apply here too
    // (capability arm in `compute/catalog.rs`).
    run_composed(
        ctx,
        prepared,
        ComposedLink::Stage {
            hookup_mm: cfg.hookup_mm,
        },
        |inputs| crate::finish::steep_shallow::SteepShallowOp {
            mesh: inputs.mesh.mesh,
            index: inputs.mesh.index,
            cutter: ctx.tool_def,
            params: &params,
            boundary_regions: inputs.boundary,
            resolution: crate::finish::steep_shallow::steep_shallow_generation_resolution(
                ctx.tool_def,
                params.tolerance,
            ),
            rest: inputs.rest,
            link_stage: inputs.link_stage,
        },
    )
}

/// RampFinish family adapter (W6.3): [`RampPass`] on the height-field
/// runner. The spans come from the kernel's labels, and the semantic trace
/// from `annotate_ramp_finish`. The kernel polls the cancel (pinned by
/// `every_generator_polls_a_preset_cancel_flag_past_the_prologue`).
pub(crate) fn generate_ramp_finish(
    ctx: &ExecutionContext<'_>,
    prepared: &Prepared<'_>,
    op: &OperationConfig,
) -> Result<GeneratedToolpath, OperationError> {
    let cfg = config_guard!(op, RampFinish, "generate_ramp_finish");
    let pass = RampPass {
        params: cfg.params(op_motion(ctx, op)),
    };
    run_height_field(&pass, ctx, prepared)
}

/// The Ramp Finish op: ramps between the levels of its own terrace ladder.
/// The ladder stays inside the kernel: it cuts between two levels, and its
/// bottom comes from the surface reach clamp. Nothing outside the kernel
/// reads it.
struct RampPass {
    params: crate::finish::ramp_finish::RampFinishParams,
}

impl HeightFieldOp for RampPass {
    type Annotation = crate::finish::ramp_finish::RampFinishRuntimeAnnotation;
    /// PR-8b: the runner records it always, an inert clamp too. This is the
    /// only op that runs a ramp descent, so `None` downstream means "no
    /// descent ran" and never "a descent ran and nobody looked".
    type Report = crate::finish::ramp_finish::RampReachClamp;

    fn op_type(&self) -> OperationType {
        OperationType::RampFinish
    }

    fn span_rule(&self) -> SpanRule {
        SpanRule::LabeledEvents
    }

    fn annotate(
        &self,
        annotations: &[Self::Annotation],
        toolpath: &Toolpath,
        sem: &ToolpathSemanticContext,
    ) {
        crate::compute::annotate::annotate_ramp_finish(annotations, toolpath, sem);
    }

    fn plan(
        &self,
        query: HeightQuery<'_>,
        territory: &Territory<'_>,
        pctx: &PlanCtx<'_>,
    ) -> Result<Planned<Self::Annotation, Self::Report>, OperationError> {
        let (surface, cutter) = query.mesh(self.op_type())?;
        let (tp, annotations, reach_clamp) =
            crate::finish::ramp_finish::ramp_finish_toolpath_structured_annotated_with_cancel(
                surface.mesh,
                surface.index,
                cutter,
                &self.params,
                pctx.debug,
                territory.regions(),
                pctx.cancel,
            )?;
        Ok(Planned::new(tp, reach_clamp).with_annotations(annotations))
    }
}

/// SpiralFinish family adapter (W6.3): [`SpiralPass`] on the height-field
/// runner. The spans come from the kernel's labels, and the semantic trace
/// from `annotate_spiral_finish`. The kernel polls the cancel (pinned by
/// `every_generator_polls_a_preset_cancel_flag_past_the_prologue`).
pub(crate) fn generate_spiral_finish(
    ctx: &ExecutionContext<'_>,
    prepared: &Prepared<'_>,
    op: &OperationConfig,
) -> Result<GeneratedToolpath, OperationError> {
    let cfg = config_guard!(op, SpiralFinish, "generate_spiral_finish");
    let pass = SpiralPass {
        params: cfg.params(op_motion(ctx, op)),
    };
    run_height_field(&pass, ctx, prepared)
}

/// The Spiral Finish op: one spiral about the mesh bbox centre. The centre
/// is not read from the territory (that is an output change).
struct SpiralPass {
    params: crate::finish::spiral_finish::SpiralFinishParams,
}

impl HeightFieldOp for SpiralPass {
    type Annotation = crate::finish::spiral_finish::SpiralFinishRuntimeAnnotation;
    type Report = ();

    fn op_type(&self) -> OperationType {
        OperationType::SpiralFinish
    }

    fn span_rule(&self) -> SpanRule {
        SpanRule::LabeledEvents
    }

    fn annotate(
        &self,
        annotations: &[Self::Annotation],
        toolpath: &Toolpath,
        sem: &ToolpathSemanticContext,
    ) {
        crate::compute::annotate::annotate_spiral_finish(annotations, toolpath, sem);
    }

    fn plan(
        &self,
        query: HeightQuery<'_>,
        territory: &Territory<'_>,
        pctx: &PlanCtx<'_>,
    ) -> Result<Planned<Self::Annotation, ()>, OperationError> {
        let (surface, cutter) = query.mesh(self.op_type())?;
        let (tp, annotations) =
            crate::finish::spiral_finish::spiral_finish_toolpath_structured_annotated_with_cancel(
                surface.mesh,
                surface.index,
                cutter,
                &self.params,
                pctx.debug,
                territory.regions(),
                pctx.cancel,
            )?;
        Ok(Planned::new(tp, ()).with_annotations(annotations))
    }
}

/// RadialFinish family adapter (W6.3): [`RadialPass`] on the height-field
/// runner. The kernel polls the cancel (pinned by
/// `every_generator_polls_a_preset_cancel_flag_past_the_prologue`).
pub(crate) fn generate_radial_finish(
    ctx: &ExecutionContext<'_>,
    prepared: &Prepared<'_>,
    op: &OperationConfig,
) -> Result<GeneratedToolpath, OperationError> {
    let cfg = config_guard!(op, RadialFinish, "generate_radial_finish");
    let pass = RadialPass {
        params: cfg.params(op_motion(ctx, op)),
    };
    run_height_field(&pass, ctx, prepared)
}

/// The Radial Finish op: spokes from the mesh bbox centre. The centre is
/// not read from the territory (that is an output change).
struct RadialPass {
    params: crate::finish::radial_finish::RadialFinishParams,
}

impl HeightFieldOp for RadialPass {
    type Annotation = NoLabel;
    type Report = ();

    fn op_type(&self) -> OperationType {
        OperationType::RadialFinish
    }

    /// N10: the registry gate in `set_toolpath_param` guards the MCP and
    /// CLI route only. A project file and the GUI inspector write the
    /// config directly, so the generator reads the SAME declared domain
    /// and refuses here. `ParamRange::accepts` also rejects a non-finite
    /// value, which a bare `<= 0.0` comparison lets through.
    fn refuse(&self) -> Result<(), OperationError> {
        for (name, value) in [
            ("angular_step", self.params.angular_step),
            ("point_spacing", self.params.point_spacing),
        ] {
            let Some(range) = OperationConfig::param_range_for_type(self.op_type(), name) else {
                continue;
            };
            if !range.accepts(value) {
                return Err(OperationError::Other(format!(
                    "Radial Finish '{name}' = {value} is outside the accepted range ({}). \
                     The generator divides by this dial, so a value outside the range \
                     emits nothing or runs without end. Set a value inside the range.",
                    range.describe()
                )));
            }
        }
        Ok(())
    }

    fn span_rule(&self) -> SpanRule {
        SpanRule::CutRuns("Radial ray")
    }

    fn plan(
        &self,
        query: HeightQuery<'_>,
        territory: &Territory<'_>,
        pctx: &PlanCtx<'_>,
    ) -> Result<Planned<NoLabel, ()>, OperationError> {
        let (surface, cutter) = query.mesh(self.op_type())?;
        let tp = crate::finish::radial_finish::radial_finish_toolpath_with_cancel(
            surface.mesh,
            surface.index,
            cutter,
            &self.params,
            territory.regions(),
            pctx.cancel,
        )?;
        Ok(Planned::new(tp, ()))
    }
}

/// HorizontalFinish family adapter (W6.3): [`HorizontalPass`] on the
/// height-field runner. The kernel polls the cancel (pinned by
/// `every_generator_polls_a_preset_cancel_flag_past_the_prologue`).
pub(crate) fn generate_horizontal_finish(
    ctx: &ExecutionContext<'_>,
    prepared: &Prepared<'_>,
    op: &OperationConfig,
) -> Result<GeneratedToolpath, OperationError> {
    let cfg = config_guard!(op, HorizontalFinish, "generate_horizontal_finish");
    let pass = HorizontalPass {
        params: cfg.params(op_motion(ctx, op)),
    };
    run_height_field(&pass, ctx, prepared)
}

/// The Horizontal Finish op: a raster over each group of flat triangles.
/// The kernel tests the triangle normals, not the territory's `Flat` cell
/// mask (`height_field` module doc).
struct HorizontalPass {
    params: crate::finish::horizontal_finish::HorizontalFinishParams,
}

impl HeightFieldOp for HorizontalPass {
    type Annotation = NoLabel;
    type Report = ();

    fn op_type(&self) -> OperationType {
        OperationType::HorizontalFinish
    }

    /// F030: the raster divides by the stepover. A project file and the GUI
    /// write the config past the setter's domain check, so the generator
    /// refuses here, as `RadialPass::refuse` does.
    fn refuse(&self) -> Result<(), OperationError> {
        self.params.refuse().map_err(OperationError::Other)
    }

    fn span_rule(&self) -> SpanRule {
        SpanRule::CutRuns("Horizontal slice")
    }

    fn plan(
        &self,
        query: HeightQuery<'_>,
        territory: &Territory<'_>,
        pctx: &PlanCtx<'_>,
    ) -> Result<Planned<NoLabel, ()>, OperationError> {
        let (surface, cutter) = query.mesh(self.op_type())?;
        let tp = crate::finish::horizontal_finish::horizontal_finish_toolpath_with_cancel(
            surface.mesh,
            surface.index,
            cutter,
            &self.params,
            territory.regions(),
            pctx.cancel,
        )?;
        Ok(Planned::new(tp, ()))
    }
}

/// The feed, the plunge and the safe Z of one op.
fn op_motion(ctx: &ExecutionContext<'_>, op: &OperationConfig) -> OpMotion {
    OpMotion {
        feed_rate: op.feed_rate(),
        plunge_rate: op.plunge_rate(),
        safe_z: ctx.heights.retract_z,
    }
}
