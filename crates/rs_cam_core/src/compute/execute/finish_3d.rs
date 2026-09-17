//! The 3D finishing family: 3D adaptive, pencil, unified finish and the five
//! single-strategy finishers.
//!
//! Split out of `compute/execute.rs` (P4).

use std::sync::atomic::Ordering;

use crate::compute::catalog::{OperationConfig, OperationType};
use crate::polygon::Polygon2;
use crate::tool::MillingCutter;

use super::dressup_apply::SHALLOW_SLOPE_DERATE_BASIS;
use super::findings::{
    record_claims_reference, record_clipped_band, record_deprecated_dial, record_derived_stepover,
    record_dropped_band, record_inert_claims_dial, record_monotone_cells, record_pencil_link,
    record_ramp_reach_clamp, record_relink_totals, record_tip_float, record_truncated_core,
    record_zero_removal,
};
use super::shared::{
    generated_with_cut_run_spans, generated_with_spans, require_index, require_mesh,
    with_depth_run_annotation,
};
use super::{ExecutionContext, GeneratedToolpath, OperationError};

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

/// Adaptive3d family adapter. Cancellable; consumes `ctx.boundary`
/// (F-027 world-stock XY bounds + machining-boundary pre-clear) and
/// `ctx.initial_stock`; spans come from
/// `spans_from_adaptive3d_annotations` + annotate_adaptive3d.
pub(crate) fn generate_adaptive3d(
    ctx: &ExecutionContext<'_>,
    op: &OperationConfig,
) -> Result<GeneratedToolpath, OperationError> {
    let cfg = config_guard!(op, Adaptive3d, "generate_adaptive3d");
    let m = require_mesh(ctx.mesh)?;
    let idx = require_index(ctx.index, "Adaptive3D")?;

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
            crate::adaptive3d::EntryStyle3d::Helix {
                radius: ctx.tool_def.diameter() * cfg.helix_radius_factor,
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
        tool_radius: engagement_radius,
        envelope_radius: ctx.tool_def.envelope_radius_mm(),
        stepover: cfg.stepover,
        depth_per_pass: cfg.depth_per_pass,
        stock_to_leave: adaptive3d_effective_stock_to_leave(cfg),
        feed_rate: op.feed_rate(),
        plunge_rate: op.plunge_rate(),
        tolerance: cfg.tolerance,
        min_cutting_radius: cfg.min_cutting_radius,
        // Heights audit 2026-06-12, finding 2: the rough always anchors
        // its top on the stock bbox — a pinned `top_z` is deliberately
        // NOT honored here. Roughing must start from the real material
        // top: anchoring lower leaves the overhead band unplanned while
        // the simulator still carries it, and the first pass would sweep
        // the full pre-stamp ray in one bite (the F-027 failure shape).
        // It would also rewrite tuned historical projects whose pinned
        // tops were vacuous while heights were ignored (wanaka100 Back
        // Rough pins `top_z = model_top` under an 18 mm overhead — the
        // F-034 machine-calibration anchor measures that exact path).
        stock_top_z: ctx.stock_bbox.max.z,
        // A user-pinned `bottom_z` IS honored: it floors the Z-level
        // plan (raising the floor only removes deep passes — always
        // safe; it's the lever that stops a rough descending through
        // holes in open meshes). Auto leaves the floor to the surface
        // heightmap (Auto resolves to `top - op_depth`, which has no
        // meaning for surface-driven roughing).
        z_floor: ctx.heights.bottom_pinned.then_some(ctx.heights.bottom_z),
        entry_style,
        fine_stepdown: if cfg.fine_stepdown > 0.0 {
            Some(cfg.fine_stepdown)
        } else {
            None
        },
        detect_flat_areas: cfg.detect_flat_areas,
        max_stay_down_dist: None,
        region_ordering,
        initial_stock: ctx.initial_stock.cloned(),
        safe_z: ctx.heights.retract_z,
        clearing_strategy,
        // "Nibble" dial — forwarded to the ContourSpiral slice path.
        trochoid_cap_mult: cfg.trochoid_cap_mult,
        engagement_measure: cfg.engagement_measure,
        z_blend: cfg.z_blend,
        boundary: ctx.boundary.cloned(),
        mill_shallow_areas: cfg.mill_shallow_areas,
        shallow_angle_rad: if cfg.mill_shallow_areas {
            Some(cfg.shallow_angle_deg.unwrap_or(30.0).to_radians())
        } else {
            None
        },
        shallow_stepdown: if cfg.mill_shallow_areas {
            cfg.shallow_stepdown
                .or(Some(cfg.depth_per_pass * 0.5))
                .filter(|&s| s > 0.0 && s < cfg.depth_per_pass)
        } else {
            None
        },
        // F-027: forward the world stock XY bounds so the planner's
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
        // F-038: drop marching-squares regions whose forecast cut
        // length (perimeter + 2D adaptive walk) is below this floor.
        // Only honored by the AgentSearch strategy.
        min_region_cut_length_mm: cfg.min_region_cut_length_mm,
        // F-038b: keep-tool-down link policy. None lets the planner
        // default to 8 × tool diameter; Some(0.0) disables the
        // feature; Some(x) caps stay-down at x mm.
        max_stay_down_distance_mm: cfg.max_stay_down_distance_mm,
        stay_down_clearance_mm: cfg.stay_down_clearance_mm,
    };
    let (tp, annotations, planner_engagement) =
        crate::adaptive3d::adaptive_3d_toolpath_structured_annotated_traced_with_cancel(
            m,
            idx,
            ctx.tool_def,
            &params,
            &(|| ctx.cancel.load(Ordering::SeqCst)),
            ctx.debug_ctx,
        )
        .map_err(|_e| OperationError::Cancelled)?;
    if let Some(sem) = ctx.semantic_ctx {
        crate::compute::annotate::annotate_adaptive3d(&annotations, &tp, sem);
    }
    let spans =
        crate::compute::spans::spans_from_adaptive3d_annotations(&annotations, tp.moves.len());
    // Stage 4 — carry the planner-predicted engagement samples on the
    // AnnotatedToolpath so the feed modulator can read them post-dressup.
    let mut annotated = generated_with_spans(tp, spans);
    annotated.planner_engagement = planner_engagement;
    Ok(annotated)
}

/// Pencil family adapter (labeled-event spans + annotate_pencil).
/// Cancellable: the cooperative cancel closure is rebuilt from
/// `ctx.cancel` (pinned by `cancellable_families_honour_a_preset_cancel_flag`).
pub(crate) fn generate_pencil(
    ctx: &ExecutionContext<'_>,
    op: &OperationConfig,
) -> Result<GeneratedToolpath, OperationError> {
    let cfg = config_guard!(op, Pencil, "generate_pencil");
    let m = require_mesh(ctx.mesh)?;
    let idx = require_index(ctx.index, "Pencil")?;
    let params = crate::finish::pencil::PencilParams {
        bitangency_angle: cfg.bitangency_angle,
        min_cut_length: cfg.min_cut_length,
        hookup_distance: cfg.hookup_distance,
        num_offset_passes: cfg.num_offset_passes,
        offset_stepover: cfg.offset_stepover,
        sampling: cfg.sampling,
        feed_rate: op.feed_rate(),
        plunge_rate: op.plunge_rate(),
        safe_z: ctx.heights.retract_z,
        stock_to_leave: cfg.stock_to_leave,
        min_valley_depth: cfg.min_valley_depth,
        bisector_strength: cfg.bisector_strength,
        reference_tool_diameter: cfg.reference_tool_diameter,
        detector: crate::finish::pencil::PencilDetector::parse(&cfg.detector),
        valley_saliency: cfg.valley_saliency,
        curvature_smoothing: cfg.curvature_smoothing,
        rest_cell_mm: cfg.rest_cell_mm,
        route_width_factor: cfg.route_width_factor,
        // R1: real reference tool geometry when the op names one; else None →
        // the pencil detectors fall back to the nominal `reference_tool_diameter`.
        reference_cutter: ctx
            .reference_tool_cfg
            .as_ref()
            .map(crate::compute::cutter::build_cutter),
        // P1 W4a: cost the surface-link-vs-retract emit decision against
        // the real machine envelope when one is in scope.
        link_kinematics: ctx.link_kinematics.clone(),
        // G-LINKSTAGE: the clearance-hop tier's own cap. `None` — the
        // default — keeps the at-depth tier's cap, which is the shipped
        // emission byte for byte. `Some(0.0)` refuses every hop and leaves
        // the at-depth tier alone, which is the control arm the measured
        // pair needs (`planning/pencil_linking_2026-09-04.md`,
        // `planning/linking_2026-09-09/SPEC.md` §8).
        link_hop_distance_mm: cfg.link_hop_distance_mm,
    };
    // PR-5: `route_width_factor` is still deserialized so every saved
    // project loads unchanged, but the pencil/clearing decision is now the
    // coverage criterion and nothing reads it. An operator who tuned it is
    // told once, here, rather than left with a dial that quietly does
    // nothing.
    record_deprecated_dial(
        ctx.findings,
        crate::compute::config::DeprecatedDialFinding {
            dial: "route_width_factor",
            value: cfg.route_width_factor,
            default_value: crate::finish::pencil::route_width_factor_default(),
            replaced_by: "the coverage criterion (reachable band vs \
                          num_offset_passes x offset_stepover)",
        },
    );
    let mut rest_grid_out: Option<crate::surface::rest_field::RestGrid> = None;
    let mut rest_regions_out: Option<Vec<Polygon2>> = None;
    let mut tip_float_out: Option<crate::compute::config::TipFloatFinding> = None;
    let mut link_report_out: Option<crate::finish::pencil::PencilLinkReport> = None;
    let (tp, annotations) =
        crate::finish::pencil::pencil_toolpath_structured_annotated_with_cancel(
            m,
            idx,
            ctx.tool_def,
            &params,
            // R2: the prior-op machined stock (FromRemainingStock + a prior sim);
            // the RestDepth detector prefers it as the rest reference.
            ctx.initial_stock,
            ctx.debug_ctx,
            &mut rest_grid_out,
            &mut rest_regions_out,
            &mut tip_float_out,
            &mut link_report_out,
            &(|| ctx.cancel.load(Ordering::SeqCst)),
        )
        .map_err(|_e| OperationError::Cancelled)?;
    // Wave D1: pencil is a centreline op, so it always MEASURES float — even
    // when the answer is zero. That is the whole point: a silent pass and a
    // pass that proved the tool reached the floor must not look alike.
    if let Some(float) = tip_float_out {
        record_tip_float(ctx.findings, float);
    }
    // G-LINKVISIBLE: the generator sets this exactly when the emitter ran,
    // so pass its `Option` through. `None` reaches the stats channel as "not
    // measured" — no centreline, so no junction decision was ever taken.
    if let Some(link) = link_report_out {
        record_pencil_link(ctx.findings, link);
    }
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
    // Attach the RestDepth heatmap grid (if any) for the GUI overlay.
    generated.rest_grid = rest_grid_out.map(std::sync::Arc::new);
    // Attach the derived machining-region polygons (if any) — P2.2
    // selective-finishing boundary source.
    generated.rest_regions = rest_regions_out.map(std::sync::Arc::new);
    Ok(generated)
}

/// UnifiedFinish family adapter (P2.c orchestrator —
/// `planning/unified_finish_planner_design.md`). Mirrors `generate_scallop`
/// closely: ball-tip refusal, mesh/index guards, inherited-dial params
/// build, the P2.b `FinishPlannerParams::for_tool` base with the op's three
/// new dials (`steep_threshold_deg` / `waterline_threshold_deg` /
/// `overlap_mm`) overridden on top (one-new-dial rule — everything else
/// inherits the tool-derived conditioning defaults), then the same
/// cancellable-core-call / annotate / spans tail Scallop uses (the core
/// call returns the same `ScallopRuntimeAnnotation` type since the
/// mid-steep band is itself a scallop pass).
pub(crate) fn generate_unified_finish(
    ctx: &ExecutionContext<'_>,
    op: &OperationConfig,
) -> Result<GeneratedToolpath, OperationError> {
    let cfg = config_guard!(op, UnifiedFinish, "generate_unified_finish");
    // Membership pinned by
    // `tool_constraints_allows_matches_runtime_refusal_semantics`.
    if !OperationType::UnifiedFinish
        .registry_entry()
        .tool_constraints
        .allows(ctx.tool_cfg.tool_type.cutter_kind())
    {
        return Err(OperationError::InvalidTool(
            "Unified Finish requires a ball-tip tool (Ball Nose or Tapered Ball Nose)".into(),
        ));
    }
    let m = require_mesh(ctx.mesh)?;
    let idx = require_index(ctx.index, "UnifiedFinish")?;
    let params = crate::finish::unified_finish::UnifiedFinishParams {
        scallop_height: cfg.scallop_height,
        tolerance: cfg.tolerance,
        raster_stepover: cfg.raster_stepover,
        z_step: cfg.z_step,
        sampling: cfg.sampling,
        stock_to_leave: cfg.stock_to_leave,
        feed_rate: op.feed_rate(),
        plunge_rate: op.plunge_rate(),
        safe_z: ctx.heights.retract_z,
        intra_region_hookup_mm: cfg.intra_region_hookup_mm,
        classification_sampler: cfg.classification_sampler,
        monotone_cell_decomposition: cfg.monotone_cell_decomposition,
    };
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
    record_inert_claims_dial(ctx.findings, cfg);

    let claims_cfg = cfg.pencil_claims.then(|| {
        // The stock always feeds the TERRITORY mask; it ALSO feeds crease
        // detection when the resolved reference is
        // `CreaseReference::MachinedStock` (build-list item 3 —
        // `CreaseReference` doc). The same XY frame guard
        // `resolve_rest_reference` applies, minus its reference fallback
        // chain.
        let territory_stock = ctx.initial_stock.filter(|stock| {
            let (sb, mb) = (&stock.stock_bbox, &m.bbox);
            sb.min.x <= mb.max.x
                && sb.max.x >= mb.min.x
                && sb.min.y <= mb.max.y
                && sb.max.y >= mb.min.y
        });
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
        record_claims_reference(
            ctx.findings,
            crate::compute::config::ClaimsReferenceFinding {
                resolution,
                territory_clip_requested: cfg.territory_clip,
            },
        );
        // Detector tuning mirrors `attach_generic_rest_analysis`: use the
        // configured `rest_analysis` cell/depth/margin when present, else
        // the detector's own defaults. `route_width_factor`/
        // `min_cut_length` have no `RestAnalysisConfig` equivalent yet, so
        // they always fall back to `RestFieldParams::default()`.
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
    let link_ceiling = ctx
        .initial_stock
        .map(|stock| crate::finish::surface_link::LinkCeiling {
            stock: Some(stock),
            tool_radius: ctx.tool_def.envelope_radius_mm(),
            // The analytic stock top in the emission frame — the same
            // fallback `optimize_entry_descents` takes where the dexel
            // query has no answer.
            fallback_top_z: ctx.stock_bbox.max.z,
        });
    let (tp, annotations, report) =
        crate::finish::unified_finish::unified_finish_toolpath_with_cancel_and_ceiling(
            m,
            idx,
            ctx.tool_def,
            ctx.heights.top_z,
            ctx.heights.bottom_z,
            &params,
            &planner,
            ctx.boundary_regions,
            // P2.d: cost the region route against the real machine envelope
            // when one is in scope (same plumbing as pencil's P1 W4a hookup).
            ctx.link_kinematics.as_ref(),
            claims_cfg.as_ref(),
            ctx.debug_ctx,
            link_ceiling,
            &(|| ctx.cancel.load(Ordering::SeqCst)),
        )
        .map_err(|_e| OperationError::Cancelled)?;
    // Report-only, and only when the pass actually ran — `None` on the
    // stats side means "not measured", never "nothing retracted".
    if cfg.intra_region_hookup_mm > 0.0 {
        record_relink_totals(ctx.findings, report.relink);
    }
    // C2: same rule — the generator already decided whether anything was
    // measured, so pass its `Option` through rather than re-deriving the
    // question from the config (a dial that is ON but met no Shallow region
    // measured nothing).
    if let Some(totals) = report.monotone_cells {
        record_monotone_cells(ctx.findings, totals);
    }
    record_truncated_core(
        ctx.findings,
        report.uncut_core_mm2,
        report.untouched_mm2,
        report.standing_mm2,
    );
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
        record_zero_removal(ctx.findings, &tp, stock, ctx.tool_def);
    }
    // Wave D1: an unmachined band is a generation-time finding with no home
    // on the toolpath — the whole reason `GenerationFindings` exists.
    record_dropped_band(
        ctx.findings,
        crate::finish::unified_finish::dropped_band_finding(&report),
    );
    // C8: the quiet sibling — bands the heights SHORTENED but did not erase.
    record_clipped_band(
        ctx.findings,
        crate::finish::unified_finish::clipped_band_finding(&report),
    );
    // Wave D1: the crease node's centrelines are pencil centrelines and
    // float for the same reasons. `None` when claims never ran.
    if let Some(float) = report.tip_float {
        record_tip_float(ctx.findings, float);
    }
    // Honest raster (Track B fix, 2026-09-01): one entry per Shallow region
    // whose raster stepover was derated by cos(theta_max). The operator's
    // `raster_stepover` dial is not rewritten, so this audit trail is the
    // only surface the derived value appears on.
    for d in &report.shallow_slope_derates {
        record_derived_stepover(
            ctx.findings,
            crate::compute::config::DerivedStepoverFinding {
                site: "UnifiedFinish shallow raster slope derate",
                stepover_mm: d.derated_stepover_mm,
                // Not a depth-keyed derivation: the derate is slope-keyed.
                reference_depth_mm: 0.0,
                reference_depth_basis: SHALLOW_SLOPE_DERATE_BASIS,
                envelope_rule_mm: d.configured_stepover_mm,
                slope_derate: Some(crate::compute::config::SlopeDerateDetail {
                    region_index: d.region_index,
                    slope_max_deg: d.slope_max_deg,
                }),
            },
        );
    }
    // PR-6a (H2.3): the crease/pencil fan's stepover is derived from the
    // canonical reach policy, not from any dial the operator can see. `None`
    // when the claims pipeline never ran, so "not derived" stays distinct
    // from "derived and unchanged".
    if let Some(claims) = report.claims {
        record_derived_stepover(
            ctx.findings,
            crate::compute::config::DerivedStepoverFinding {
                site: "UnifiedFinish crease/pencil claims",
                stepover_mm: claims.offset_stepover_mm,
                reference_depth_mm: claims.offset_stepover_reference_depth_mm,
                reference_depth_basis: crate::finish::unified_finish::CLAIMS_STEPOVER_DEPTH_BASIS,
                envelope_rule_mm: claims.envelope_rule_stepover_mm,
                slope_derate: None,
            },
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
    generated.rest_grid = report.rest_grid;
    generated.rest_regions = report.rest_regions;
    Ok(generated)
}

/// SteepShallow family adapter. Cancellable: the cooperative cancel
/// closure is rebuilt from `ctx.cancel` (pinned by
/// `cancellable_families_honour_a_preset_cancel_flag`).
pub(crate) fn generate_steep_shallow(
    ctx: &ExecutionContext<'_>,
    op: &OperationConfig,
) -> Result<GeneratedToolpath, OperationError> {
    let cfg = config_guard!(op, SteepShallow, "generate_steep_shallow");
    let m = require_mesh(ctx.mesh)?;
    let idx = require_index(ctx.index, "SteepShallow")?;
    let params = crate::finish::steep_shallow::SteepShallowParams {
        threshold_angle: cfg.threshold_angle,
        overlap_distance: cfg.overlap_distance,
        wall_clearance: cfg.wall_clearance,
        steep_first: cfg.steep_first,
        stepover: cfg.stepover,
        z_step: cfg.z_step,
        feed_rate: op.feed_rate(),
        plunge_rate: op.plunge_rate(),
        safe_z: ctx.heights.retract_z,
        sampling: cfg.sampling,
        stock_to_leave: cfg.stock_to_leave,
        tolerance: cfg.tolerance,
    };
    let (tp, split) = crate::finish::steep_shallow::steep_shallow_toolpath_split_with_cancel(
        m,
        idx,
        ctx.tool_def,
        &params,
        ctx.boundary_regions,
        &(|| ctx.cancel.load(Ordering::SeqCst)),
    )
    .map_err(|_e| OperationError::Cancelled)?;
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

/// RampFinish family adapter (labeled-event spans + annotate_ramp_finish).
/// Cancellable: the cooperative cancel closure is rebuilt from
/// `ctx.cancel` (pinned by `cancellable_families_honour_a_preset_cancel_flag`).
pub(crate) fn generate_ramp_finish(
    ctx: &ExecutionContext<'_>,
    op: &OperationConfig,
) -> Result<GeneratedToolpath, OperationError> {
    let cfg = config_guard!(op, RampFinish, "generate_ramp_finish");
    let m = require_mesh(ctx.mesh)?;
    let idx = require_index(ctx.index, "RampFinish")?;
    let params = crate::finish::ramp_finish::RampFinishParams {
        max_stepdown: cfg.max_stepdown,
        slope_from: cfg.slope_from,
        slope_to: cfg.slope_to,
        direction: cfg.direction,
        order_bottom_up: cfg.order_bottom_up,
        feed_rate: op.feed_rate(),
        plunge_rate: op.plunge_rate(),
        safe_z: ctx.heights.retract_z,
        sampling: cfg.sampling,
        stock_to_leave: cfg.stock_to_leave,
        tolerance: cfg.tolerance,
    };
    let (tp, annotations, reach_clamp) =
        crate::finish::ramp_finish::ramp_finish_toolpath_structured_annotated_with_cancel(
            m,
            idx,
            ctx.tool_def,
            &params,
            ctx.debug_ctx,
            ctx.boundary_regions,
            &(|| ctx.cancel.load(Ordering::SeqCst)),
        )
        .map_err(|_e| OperationError::Cancelled)?;
    // PR-8b: unconditional, including when the clamp was inert — this is the
    // only adapter that runs a ramp descent, so `None` downstream means "no
    // descent ran" and never "a descent ran and I did not look".
    record_ramp_reach_clamp(ctx.findings, reach_clamp);
    if let Some(sem) = ctx.semantic_ctx {
        crate::compute::annotate::annotate_ramp_finish(&annotations, &tp, sem);
    }
    let spans = crate::compute::spans::spans_from_labeled_events(
        tp.moves.len(),
        annotations
            .iter()
            .map(|ann| (ann.move_index, ann.event.label())),
    );
    Ok(generated_with_spans(tp, spans))
}

/// SpiralFinish family adapter. NOTE: spans come from
/// `spans_from_labeled_events` over the generator's annotations, and
/// the family annotate fn is `annotate_spiral_finish` — both part of
/// the contract. Cancellable: the cooperative cancel closure is rebuilt
/// from `ctx.cancel` (pinned by
/// `cancellable_families_honour_a_preset_cancel_flag`).
pub(crate) fn generate_spiral_finish(
    ctx: &ExecutionContext<'_>,
    op: &OperationConfig,
) -> Result<GeneratedToolpath, OperationError> {
    let cfg = config_guard!(op, SpiralFinish, "generate_spiral_finish");
    let m = require_mesh(ctx.mesh)?;
    let idx = require_index(ctx.index, "SpiralFinish")?;
    let params = crate::finish::spiral_finish::SpiralFinishParams {
        stepover: cfg.stepover,
        direction: cfg.direction,
        feed_rate: op.feed_rate(),
        plunge_rate: op.plunge_rate(),
        safe_z: ctx.heights.retract_z,
        stock_to_leave: cfg.stock_to_leave,
    };
    let (tp, annotations) =
        crate::finish::spiral_finish::spiral_finish_toolpath_structured_annotated_with_cancel(
            m,
            idx,
            ctx.tool_def,
            &params,
            ctx.debug_ctx,
            ctx.boundary_regions,
            &(|| ctx.cancel.load(Ordering::SeqCst)),
        )
        .map_err(|_e| OperationError::Cancelled)?;
    if let Some(sem) = ctx.semantic_ctx {
        crate::compute::annotate::annotate_spiral_finish(&annotations, &tp, sem);
    }
    let spans = crate::compute::spans::spans_from_labeled_events(
        tp.moves.len(),
        annotations
            .iter()
            .map(|ann| (ann.move_index, ann.event.label())),
    );
    Ok(generated_with_spans(tp, spans))
}

/// RadialFinish family adapter. Cancellable: the cooperative cancel
/// closure is rebuilt from `ctx.cancel` (pinned by
/// `cancellable_families_honour_a_preset_cancel_flag`).
pub(crate) fn generate_radial_finish(
    ctx: &ExecutionContext<'_>,
    op: &OperationConfig,
) -> Result<GeneratedToolpath, OperationError> {
    let cfg = config_guard!(op, RadialFinish, "generate_radial_finish");
    // N10: the registry gate in `set_toolpath_param` guards the MCP and
    // CLI route only. A project file and the GUI inspector write the
    // config directly, so the generator reads the SAME declared domain
    // and refuses here. `ParamRange::accepts` also rejects a non-finite
    // value, which a bare `<= 0.0` comparison lets through.
    let op_type = OperationType::RadialFinish;
    for (name, value) in [
        ("angular_step", cfg.angular_step),
        ("point_spacing", cfg.point_spacing),
    ] {
        let Some(range) = OperationConfig::param_range_for_type(op_type, name) else {
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
    let m = require_mesh(ctx.mesh)?;
    let idx = require_index(ctx.index, "RadialFinish")?;
    let params = crate::finish::radial_finish::RadialFinishParams {
        angular_step: cfg.angular_step,
        point_spacing: cfg.point_spacing,
        feed_rate: op.feed_rate(),
        plunge_rate: op.plunge_rate(),
        safe_z: ctx.heights.retract_z,
        stock_to_leave: cfg.stock_to_leave,
    };
    let tp = crate::finish::radial_finish::radial_finish_toolpath_with_cancel(
        m,
        idx,
        ctx.tool_def,
        &params,
        ctx.boundary_regions,
        &(|| ctx.cancel.load(Ordering::SeqCst)),
    )
    .map_err(|_e| OperationError::Cancelled)?;
    Ok(with_depth_run_annotation(
        generated_with_cut_run_spans(tp, "Radial ray"),
        ctx.semantic_ctx,
    ))
}

/// HorizontalFinish family adapter. Cancellable: the cooperative
/// cancel closure is rebuilt from `ctx.cancel` (pinned by
/// `cancellable_families_honour_a_preset_cancel_flag`).
pub(crate) fn generate_horizontal_finish(
    ctx: &ExecutionContext<'_>,
    op: &OperationConfig,
) -> Result<GeneratedToolpath, OperationError> {
    let cfg = config_guard!(op, HorizontalFinish, "generate_horizontal_finish");
    let m = require_mesh(ctx.mesh)?;
    let idx = require_index(ctx.index, "HorizontalFinish")?;
    let params = crate::finish::horizontal_finish::HorizontalFinishParams {
        angle_threshold: cfg.angle_threshold,
        stepover: cfg.stepover,
        feed_rate: op.feed_rate(),
        plunge_rate: op.plunge_rate(),
        safe_z: ctx.heights.retract_z,
        stock_to_leave: cfg.stock_to_leave,
    };
    let tp = crate::finish::horizontal_finish::horizontal_finish_toolpath_with_cancel(
        m,
        idx,
        ctx.tool_def,
        &params,
        ctx.boundary_regions,
        &(|| ctx.cancel.load(Ordering::SeqCst)),
    )
    .map_err(|_e| OperationError::Cancelled)?;
    Ok(with_depth_run_annotation(
        generated_with_cut_run_spans(tp, "Horizontal slice"),
        ctx.semantic_ctx,
    ))
}
