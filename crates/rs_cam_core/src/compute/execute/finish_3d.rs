//! The 3D finishing family: 3D adaptive, pencil, unified finish and the five
//! single-strategy finishers.
//!
//! Horizontal, Radial, Spiral and Ramp Finish run on the height-field
//! runner (`height_field.rs`, W6.3). Split out of `compute/execute.rs` (P4).

use crate::compute::catalog::{OperationConfig, OperationType};
use crate::compute::operation_configs::OpMotion;
use crate::tool::MillingCutter;
use crate::toolpath::Toolpath;
use crate::trace::semantic_trace::ToolpathSemanticContext;

use super::height_field::{
    HeightFieldOp, HeightQuery, NoLabel, PlanCtx, Planned, SpanRule, run_height_field,
};
use super::shared::{generated_with_spans, op_link_ceiling, with_depth_run_annotation};
use super::territory::Territory;
use super::{ExecutionContext, GeneratedToolpath, MeshInput, OperationError, Prepared};

/// Read the single leave-stock scalar the adaptive3d planner consumes.
///
/// The planner (`crate::adaptive3d::{path,clearing,search}`) is a
/// drop-cutter / dexel heightmap engine: every use of `stock_to_leave`
/// raises the "protected surface" (`surf_z + stock_to_leave`) purely in
/// the Z direction — z-level floors, waterline lift, and gouge-guard
/// drape all key off a single vertical offset from `point_drop_cutter`.
/// There is no wall-normal / horizontal offset path (no polygon inset,
/// no lateral shift of the EDT-derived contours), so this engine cannot
/// honour a radial (sidewall) leave allowance at all.
///
/// L2 deleted the inert `stock_to_leave_radial` dial that used to sit
/// beside the axial one. A caller that wants a wall offset needs a
/// planner mechanism first, not a dial the planner drops.
pub(super) fn adaptive3d_effective_stock_to_leave(
    cfg: &crate::compute::operation_configs::Adaptive3dConfig,
) -> f64 {
    cfg.stock_to_leave_axial
}

/// Adaptive3d family adapter. Cancellable; consumes the territory's
/// `PlannerBounds` (the machining-boundary pre-clear and the centre
/// containment), the world-stock XY bounds (F-027) and
/// `ctx.initial_stock`; spans come from
/// `spans_from_adaptive3d_annotations` + annotate_adaptive3d.
pub(crate) fn generate_adaptive3d(
    ctx: &ExecutionContext<'_>,
    prepared: &Prepared<'_>,
    op: &OperationConfig,
) -> Result<GeneratedToolpath, OperationError> {
    let cfg = config_guard!(op, Adaptive3d, "generate_adaptive3d");
    let MeshInput {
        mesh: m,
        index: idx,
    } = prepared.mesh()?;
    // W6.2: the row states `TerritoryXy::PlannerBounds`, so the territory
    // carries the outline and the centre containment.
    let (outline, centre) = prepared.territory().planner_bounds();

    let entry_style = match cfg.entry_style {
        crate::compute::operation_configs::Adaptive3dEntryStyle::Plunge => {
            crate::adaptive3d::EntryStyle3d::Plunge
        }
        crate::compute::operation_configs::Adaptive3dEntryStyle::Ramp => {
            crate::adaptive3d::EntryStyle3d::Ramp {
                max_angle_deg: cfg.ramp_angle_deg,
            }
        }
        crate::compute::operation_configs::Adaptive3dEntryStyle::Helix => {
            // G10 Q6: D x factor, capped at the flat bottom so the helix
            // leaves no core.
            crate::adaptive3d::EntryStyle3d::Helix {
                radius: cfg.helix_radius_for(ctx.tool_def).emitted_mm,
                pitch: cfg.helix_pitch,
            }
        }
    };
    let region_ordering = match cfg.region_ordering {
        crate::compute::operation_configs::RegionOrdering::Global => {
            crate::adaptive3d::RegionOrdering::Global
        }
        crate::compute::operation_configs::RegionOrdering::ByArea => {
            crate::adaptive3d::RegionOrdering::ByArea
        }
    };
    let clearing_strategy = match cfg.clearing_strategy {
        crate::compute::operation_configs::ClearingStrategy::ContourParallel => {
            crate::adaptive3d::ClearingStrategy3d::ContourParallel
        }
        crate::compute::operation_configs::ClearingStrategy::Adaptive => {
            crate::adaptive3d::ClearingStrategy3d::Adaptive
        }
        crate::compute::operation_configs::ClearingStrategy::AgentSearch => {
            crate::adaptive3d::ClearingStrategy3d::AgentSearch
        }
        crate::compute::operation_configs::ClearingStrategy::ContourSpiral => {
            crate::adaptive3d::ClearingStrategy3d::ContourSpiral
        }
    };
    // Adaptive3d spaces passes by the tool's *engagement* radius at
    // the depth-of-cut, not the envelope radius — for tapered tools
    // these differ a lot. Floor at 0.01mm to keep stepover math safe
    // for degenerate (zero-tip) geometry.
    let engagement_radius = ctx
        .tool_def
        .engagement_radius_mm(cfg.depth_per_pass)
        .max(0.01);
    let params = crate::adaptive3d::Adaptive3dParams {
        geometry: crate::adaptive3d::Adaptive3dGeometry {
            tool_radius: engagement_radius,
            envelope_radius: ctx.tool_def.envelope_radius_mm(),
            stepover: cfg.stepover,
            tolerance: cfg.tolerance,
            segment_merge_tolerance: ctx.segment_merge_tolerance,
            min_cutting_radius: cfg.min_cutting_radius,
            boundary: outline.cloned(), // F-027: forward the world stock XY bounds so the planner's
            // internal `material_stock` extends to cover every cell the
            // simulator's per-setup dexel grid will look at. Pre-fix the
            // planner was bounded by `mesh.bbox + tool_radius`, while the
            // simulator's grid is bounded by the (auto-grown) world stock
            // bbox; cells inside the simulator grid but outside the
            // planner grid were never stamped, so the final pass carved
            // through the full stock height in one shot at model-edge
            // cells, blowing up `axial_engagement_mm` and the downstream
            // deflection gate.
            world_stock_xy_bbox: Some((
                ctx.stock_bbox.min.x,
                ctx.stock_bbox.min.y,
                ctx.stock_bbox.max.x,
                ctx.stock_bbox.max.y,
            )),
            // G-BOUNDARYPHANTOM: the containment the session clips to after
            // generation; the planner stamps only what that clip keeps.
            centre_boundary: centre
                .map(<[crate::polygon::Polygon2]>::to_vec)
                .unwrap_or_default(),
        },
        depth: crate::adaptive3d::Adaptive3dDepth {
            depth_per_pass: cfg.depth_per_pass,
            stock_to_leave: adaptive3d_effective_stock_to_leave(cfg), // Heights audit 2026-06-12, finding 2: the rough always anchors
            // its top on the stock bbox — a pinned `top_z` is deliberately
            // NOT honored here. Roughing must start from the real material
            // top: anchoring lower leaves the overhead band unplanned while
            // the simulator still carries it, and the first pass would sweep
            // the full pre-stamp ray in one bite (the F-027 failure shape).
            // It would also rewrite tuned historical projects whose pinned
            // tops were vacuous while heights were ignored (wanaka100 Back
            // Rough pins `top_z = model_top` under an 18 mm overhead — the
            // F-034 machine-calibration anchor measures that exact path).
            stock_top_z: ctx.stock_bbox.max.z, // A user-pinned `bottom_z` IS honored: it floors the Z-level
            // plan (raising the floor only removes deep passes — always
            // safe; it's the lever that stops a rough descending through
            // holes in open meshes). Auto leaves the floor to the surface
            // heightmap (Auto resolves to `top - op_depth`, which has no
            // meaning for surface-driven roughing).
            z_floor: ctx.heights.bottom_pinned.then_some(ctx.heights.bottom_z),
            detect_flat_areas: cfg.detect_flat_areas,
        },
        linking: crate::adaptive3d::Adaptive3dLinking {
            region_ordering, // F-038: drop marching-squares regions whose forecast cut
            // length (perimeter + 2D adaptive walk) is below this floor.
            // Only honored by the AgentSearch strategy.
            min_region_cut_length_mm: cfg.min_region_cut_length_mm, // F-038b: keep-tool-down link policy. None lets the planner
            // default to 8 × tool diameter; Some(0.0) disables the
            // feature; Some(x) caps stay-down at x mm.
            max_stay_down_distance_mm: cfg.max_stay_down_distance_mm,
            stay_down_clearance_mm: cfg.stay_down_clearance_mm,
        },
        feed_rate: op.feed_rate(),
        plunge_rate: op.plunge_rate(),
        // G-RAMPCLAMP: the entry never runs faster than the cut feed.
        ramp_feed_rate: op.entry_feed_rate(),
        entry_clearance_mm: cfg.entry_clearance_mm,
        entry_style,
        initial_stock: ctx.initial_stock.cloned(),
        safe_z: ctx.heights.retract_z,
        clearing_strategy, // "Nibble" dial — forwarded to the ContourSpiral slice path.
        trochoid_cap_mult: cfg.trochoid_cap_mult,
        engagement_measure: cfg.engagement_measure,
        z_blend: cfg.z_blend,
    };
    let crate::adaptive3d::Adaptive3dOutput {
        toolpath: tp,
        annotations,
        produced,
    } = crate::adaptive3d::adaptive_3d_toolpath_output_with_cancel(
        m,
        idx,
        ctx.tool_def,
        &params,
        &ctx.cancel_fn(),
        ctx.debug_ctx,
    )?;
    if let Some(sem) = ctx.semantic_ctx {
        crate::compute::annotate::annotate_adaptive3d(&annotations, &tp, sem);
    }
    let spans =
        crate::compute::spans::spans_from_adaptive3d_annotations(&annotations, tp.moves.len());
    // W6.2: the planner's side channel rides the result. The feed
    // modulator reads the engagement samples post-dressup (Stage 4); the
    // viewport overlay and MCP read the By Area detection.
    let mut annotated = generated_with_spans(tp, spans);
    annotated.produced = produced;
    Ok(annotated)
}

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
    let params = crate::finish::pencil::PencilParams {
        // R1: the link lift's fallback top comes from the one builder.
        // `None` on fresh stock, where the lift never runs.
        link_fallback_top_z: op_link_ceiling(op.op_type(), ctx).map(|c| c.fallback_top_z),
        ..cfg.params(
            OpMotion {
                feed_rate: op.feed_rate(),
                plunge_rate: op.plunge_rate(),
                safe_z: ctx.heights.retract_z,
            },
            ctx.reference_tool_cfg
                .as_ref()
                .map(crate::compute::cutter::build_cutter),
            ctx.link_kinematics.clone(),
        )
    };
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
                remaining: ctx.initial_stock,
                rest_reference: prepared.territory().reference_stock(),
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
/// `planning/unified_finish_planner_design.md`). Mirrors `generate_scallop`
/// closely: the prologue's ball-tip and mesh checks, inherited-dial params
/// build, the P2.b `FinishPlannerParams::for_tool` base with the op's three
/// new dials (`steep_threshold_deg` / `waterline_threshold_deg` /
/// `overlap_mm`) overridden on top (one-new-dial rule — everything else
/// inherits the tool-derived conditioning defaults), then the same
/// cancellable-core-call / annotate / spans tail Scallop uses (the core
/// call returns the same `ScallopRuntimeAnnotation` type since the
/// mid-steep band is itself a scallop pass).
pub(crate) fn generate_unified_finish(
    ctx: &ExecutionContext<'_>,
    prepared: &Prepared<'_>,
    op: &OperationConfig,
) -> Result<GeneratedToolpath, OperationError> {
    let cfg = config_guard!(op, UnifiedFinish, "generate_unified_finish");
    let MeshInput {
        mesh: m,
        index: idx,
    } = prepared.mesh()?;
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

    let territory = prepared.territory();
    let claims_cfg = cfg.pencil_claims.then(|| {
        // The stock always feeds the TERRITORY mask; it ALSO feeds crease
        // detection when the resolved reference is
        // `CreaseReference::MachinedStock` (build-list item 3 —
        // `CreaseReference` doc). W6.2: the territory resolved it once,
        // through the W2.1 stock-vs-mesh guard; this op has no tool tier
        // after it (`CreaseReference` decides).
        let territory_stock = territory.reference_stock();
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
        // Detector tuning mirrors `attach_generic_rest_analysis`: use the
        // configured `rest_analysis` cell/depth/margin when present, else
        // the detector's own defaults. `min_cut_length` has no
        // `RestAnalysisConfig` equivalent yet, so it always falls back to
        // `RestFieldParams::default()`.
        // `routing_radius_mm` is set by `unified_finish_toolpath_with_cancel`
        // itself (the op's own cutter), so leaving the default here is a
        // no-op either way.
        let mut rest_field_params = ctx.rest_analysis.map_or_else(
            crate::surface::rest_field::RestFieldParams::default,
            |ra| crate::surface::rest_field::RestFieldParams {
                cell_mm: ra.cell_mm,
                min_valley_depth: ra.min_valley_depth,
                region_margin_mm: ra.region_margin_mm,
                ..crate::surface::rest_field::RestFieldParams::default()
            },
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
    });

    // B3 / G-LINKLOAD (Phase O item 3): an intra-region link rides the MESH,
    // which is the material only on a fresh-stock pass. This op is handed
    // `ctx.initial_stock` exactly when it cuts what a prior op left — there,
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
    // R1: the one builder (`op_link_ceiling`) sets all three values.
    let link_ceiling = op_link_ceiling(op.op_type(), ctx);
    let (tp, annotations, report) =
        crate::finish::unified_finish::unified_finish_toolpath_with_cancel_and_ceiling(
            m,
            idx,
            ctx.tool_def,
            ctx.heights.top_z,
            ctx.heights.bottom_z,
            &params,
            &planner,
            territory.regions(),
            // P2.d: cost the region route against the real machine envelope
            // when one is in scope (same plumbing as pencil's P1 W4a hookup).
            ctx.link_kinematics.as_ref(),
            claims_cfg.as_ref(),
            ctx.debug_ctx,
            link_ceiling,
            &ctx.cancel_fn(),
        )?;
    // W2.3: the whole report in one record — the link totals and the C2
    // cells only when their pass ran, the cascade residual, the dropped and
    // clipped bands, the crease tip float and the derived stepovers (see
    // the `UnifiedFinishReport` impl in `findings.rs`).
    ctx.record(&report);
    // A4: the pass has just been planned against a reference stock, and this
    // is the only point where the emitted geometry and that reference are
    // both in scope. Gated on the reference having RESOLVED to a real
    // machined prior — under a self-probe reference `territory_clip` never
    // ran and the op is not a rest pass in the sense this finding is about.
    if let Some(cfg) = claims_cfg.as_ref()
        && matches!(
            cfg.crease_reference,
            crate::finish::unified_finish::CreaseReference::MachinedStock
        )
        && let Some(stock) = cfg.territory_stock
    {
        ctx.record(
            &crate::compute::toolpath_stats::ZeroRemovalFinding::measure(&tp, stock, ctx.tool_def),
        );
    }
    if let Some(sem) = ctx.semantic_ctx {
        // C8: FLAT. Scallop is a sub-generator here, filling one of the
        // planner's mid-steep nodes; the region population this operation
        // publishes is `annotate_unified_finish_regions`' below, and a
        // second one would double-count and break A/M8's 1:1 gate.
        crate::compute::annotate::annotate_scallop(
            &annotations,
            &tp,
            sem,
            crate::compute::annotate::ScallopRegionGrouping::Flat,
        );
        // A/M8: the SEMANTIC region trace `narrate_toolpath` reads, built
        // from the same `RegionAnnotation` table `unified_finish_spans`
        // builds the STRUCTURAL region-node spans from. Annotation only —
        // no move is touched.
        crate::compute::annotate::annotate_unified_finish_regions(
            &crate::finish::unified_finish::unified_finish_region_annotations(&report),
            &tp,
            sem,
        );
    }
    // Spans (Region attribution + the rapid-order barriers that make this
    // op's barriered TSP safe) are built by `unified_finish::
    // unified_finish_spans` so the capability sentries can assert against
    // the same definition production ships.
    let spans = crate::finish::unified_finish::unified_finish_spans(&tp, &annotations, &report);
    let mut generated = generated_with_spans(tp, spans);
    // §2.4 carry-through: the claims detector's rest field + region
    // polygons ride the generated result exactly like the pencil
    // RestDepth arm's (GUI heatmap, DerivedRestRegions, probes). `None`
    // when claims didn't run — the generic post-pass fallback still
    // applies then.
    generated.produced = report.produced;
    Ok(generated)
}

/// SteepShallow family adapter. Cancellable: the cooperative cancel
/// closure is built by `ctx.cancel_fn()` (pinned by
/// `every_generator_polls_a_preset_cancel_flag_past_the_prologue`).
pub(crate) fn generate_steep_shallow(
    ctx: &ExecutionContext<'_>,
    prepared: &Prepared<'_>,
    op: &OperationConfig,
) -> Result<GeneratedToolpath, OperationError> {
    let cfg = config_guard!(op, SteepShallow, "generate_steep_shallow");
    let MeshInput {
        mesh: m,
        index: idx,
    } = prepared.mesh()?;
    let params = cfg.params(OpMotion {
        feed_rate: op.feed_rate(),
        plunge_rate: op.plunge_rate(),
        safe_z: ctx.heights.retract_z,
    });
    let (tp, split) = crate::finish::steep_shallow::steep_shallow_toolpath_split_with_cancel(
        m,
        idx,
        ctx.tool_def,
        &params,
        prepared.territory().regions(),
        &ctx.cancel_fn(),
    )?;
    // Two concatenated passes, not one continuous trace: barriers keep the
    // halves in their emitted order and keep the steep half's Z ladder,
    // which is what lets `UnifiedFinish`-style intra-node reordering apply
    // here too (capability arm in `compute/catalog.rs`).
    let spans = crate::finish::steep_shallow::steep_shallow_spans(&tp, &split);
    Ok(with_depth_run_annotation(
        generated_with_spans(tp, spans),
        ctx.semantic_ctx,
    ))
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
