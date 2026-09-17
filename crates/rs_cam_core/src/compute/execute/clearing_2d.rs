//! The 2.5D clearing family: rest, zigzag, trace, profile, pocket, face and
//! adaptive.
//!
//! Every adapter here steps a 2D region down a ladder of Z levels, so they
//! share `effective_levels`. Split out of `compute/execute.rs` (P4).

use std::sync::atomic::Ordering;

use crate::compute::catalog::OperationConfig;
use crate::compute::config::ResolvedHeights;
use crate::tool::MillingCutter;
use crate::toolpath::Toolpath;

use super::findings::record_truncated_core_only;
use super::record_offset_library_failures;
use super::shared::{generated_with_depth_run_spans, require_polygons, with_depth_run_annotation};
use super::{ExecutionContext, GeneratedToolpath, OperationError};

/// Rest-machining family adapter. Requires `prev_tool_radius` in the
/// context (set by the session/worker drivers from the previous tool).
pub(crate) fn generate_rest(
    ctx: &ExecutionContext<'_>,
    op: &OperationConfig,
) -> Result<GeneratedToolpath, OperationError> {
    let cfg = config_guard!(op, Rest, "generate_rest");
    // Checkpoint C, Q3: checked as the very first statement, before the
    // prev-tool precondition below. A pre-set flag must short-circuit before
    // any work AND before any other refusal, or "cancelled" gets reported as
    // whatever else happened to be wrong first.
    if ctx.cancel.load(Ordering::SeqCst) {
        return Err(OperationError::Cancelled);
    }
    let polys = require_polygons(ctx.polygons)?;
    let ptr = ctx
        .prev_tool_radius
        .ok_or_else(|| OperationError::Other("Previous tool not set for rest machining".into()))?;
    let levels = effective_levels(ctx.cutting_levels, ctx.heights, cfg.depth_per_pass);
    let tool_radius = ctx.tool_def.radius();
    let safe_z = ctx.heights.retract_z;
    // Checkpoint C, Q3: rest is cancellable now. It built no `cancel_fn` at
    // all and called the non-cancellable `depth::toolpath_at_levels`, so a
    // rest pass over many Z levels could not be interrupted — one of the two
    // families W4 measured as ignoring a pre-set flag entirely (F-4).
    // Granularity is per Z level, the same as profile/trace/zigzag.
    let cancel_fn = || ctx.cancel.load(Ordering::SeqCst);
    let mut combined = Toolpath::new();
    for poly in polys {
        // G2: the rest geometry — the inward offset, the zigzag scan lines and
        // the per-sample containment walk over the large tool's reachable
        // region — is entirely XY, so it is computed ONCE per polygon here
        // instead of once per Z level inside the closure. `cut_depth` on the
        // base params is a placeholder; `rest_segments` never reads it, and
        // the per-level stamp below overrides it.
        if ctx.cancel.load(Ordering::SeqCst) {
            return Err(OperationError::Cancelled);
        }
        let base = crate::ops::rest::RestParams {
            prev_tool_radius: ptr,
            tool_radius,
            cut_depth: 0.0,
            stepover: cfg.stepover,
            feed_rate: op.feed_rate(),
            plunge_rate: op.plunge_rate(),
            safe_z,
            angle: cfg.angle,
        };
        let segments = crate::ops::rest::rest_segments(poly, &base);
        let tp = crate::ops::depth::toolpath_at_levels_with_cancel(
            &levels,
            safe_z,
            |z| {
                Ok(crate::ops::rest::rest_segments_to_toolpath(
                    &segments,
                    &crate::ops::rest::RestParams {
                        cut_depth: z,
                        ..base
                    },
                ))
            },
            &cancel_fn,
        )
        .map_err(|_e| OperationError::Cancelled)?;
        combined.moves.extend(tp.moves);
    }
    Ok(with_depth_run_annotation(
        generated_with_depth_run_spans(combined, &levels),
        ctx.semantic_ctx,
    ))
}

/// Zigzag family adapter. Cancellable: the cooperative cancel closure is
/// rebuilt from `ctx.cancel` (pinned by
/// `cancellable_families_honour_a_preset_cancel_flag`).
pub(crate) fn generate_zigzag(
    ctx: &ExecutionContext<'_>,
    op: &OperationConfig,
) -> Result<GeneratedToolpath, OperationError> {
    let cfg = config_guard!(op, Zigzag, "generate_zigzag");
    let polys = require_polygons(ctx.polygons)?;
    let levels = effective_levels(ctx.cutting_levels, ctx.heights, cfg.depth_per_pass);
    let tool_radius = ctx.tool_def.radius();
    let safe_z = ctx.heights.retract_z;
    let cancel_fn = || ctx.cancel.load(Ordering::SeqCst);
    let mut combined = Toolpath::new();
    // Checkpoint C: a plain `Cell` accumulator, local to this generate and
    // captured by the per-level closure. Deliberately not a thread-local
    // collector (`CAVALIER_SHAPE_FAILURE.md` §6 D-1 option C, declined):
    // the count is threaded, visible in the signatures it passes through,
    // and testable without a global.
    let offset_failures = std::cell::Cell::new(0usize);
    for poly in polys {
        // G2: the wall inset and the scan-line build are Z-independent, so
        // they run ONCE per polygon. The per-level closure stamps Z only.
        //
        // This is also where the documented `offset_library_failures` x L
        // over-count goes away: the wall inset is now made once per polygon,
        // so the channel counts one failure per failing offset CALL, which is
        // what its own doc always claimed it counted.
        if ctx.cancel.load(Ordering::SeqCst) {
            return Err(OperationError::Cancelled);
        }
        let base = crate::ops::zigzag::ZigzagParams {
            tool_radius,
            stepover: cfg.stepover,
            cut_depth: 0.0,
            feed_rate: op.feed_rate(),
            plunge_rate: op.plunge_rate(),
            safe_z,
            angle: cfg.angle,
        };
        let (lines, failures) =
            crate::ops::zigzag::zigzag_lines_reported(poly, tool_radius, cfg.stepover, cfg.angle);
        offset_failures.set(offset_failures.get() + failures);
        let tp = crate::ops::depth::toolpath_at_levels_with_cancel(
            &levels,
            safe_z,
            |z| {
                Ok(crate::ops::zigzag::lines_to_toolpath(
                    &lines,
                    &crate::ops::zigzag::ZigzagParams {
                        cut_depth: z,
                        ..base
                    },
                ))
            },
            &cancel_fn,
        )
        .map_err(|_e| OperationError::Cancelled)?;
        combined.moves.extend(tp.moves);
    }
    record_offset_library_failures(ctx.findings, offset_failures.get());
    Ok(with_depth_run_annotation(
        generated_with_depth_run_spans(combined, &levels),
        ctx.semantic_ctx,
    ))
}

/// Trace family adapter. NOTE: trace uses `annotate_trace_spans`, not
/// the generic depth-run annotator — the per-family annotate fn is
/// part of the contract (plan §Phase-5 task 1). Cancellable: the
/// cooperative cancel closure is rebuilt from `ctx.cancel` (pinned by
/// `cancellable_families_honour_a_preset_cancel_flag`).
pub(crate) fn generate_trace(
    ctx: &ExecutionContext<'_>,
    op: &OperationConfig,
) -> Result<GeneratedToolpath, OperationError> {
    let cfg = config_guard!(op, Trace, "generate_trace");
    let polys = require_polygons(ctx.polygons)?;
    let levels = effective_levels(ctx.cutting_levels, ctx.heights, cfg.depth_per_pass);
    let safe_z = ctx.heights.retract_z;
    let cancel_fn = || ctx.cancel.load(Ordering::SeqCst);
    let mut combined = Toolpath::new();
    // Checkpoint C: see `generate_zigzag` for why this is a local `Cell`.
    let offset_failures = std::cell::Cell::new(0usize);
    for poly in polys {
        let params = crate::ops::trace_path::TraceParams {
            tool_radius: ctx.tool_def.radius(),
            depth: cfg.depth,
            depth_per_pass: cfg.depth_per_pass,
            feed_rate: op.feed_rate(),
            plunge_rate: op.plunge_rate(),
            safe_z,
            compensation: cfg.compensation,
            top_z: ctx.heights.top_z,
        };
        // G2: the cutter-compensation offset is Z-independent, so it runs ONCE
        // per polygon instead of once per Z level. The per-level closure stamps
        // Z only — and the failure count is now one per offset CALL rather than
        // one per call x level.
        if ctx.cancel.load(Ordering::SeqCst) {
            return Err(OperationError::Cancelled);
        }
        let (rings, failures) =
            crate::ops::trace_path::trace_compensated_polygons_reported(poly, &params);
        offset_failures.set(offset_failures.get() + failures);
        let tp = crate::ops::depth::toolpath_at_levels_with_cancel(
            &levels,
            safe_z,
            |z| {
                Ok(crate::ops::trace_path::trace_polygons_at_z(
                    &rings, z, &params,
                ))
            },
            &cancel_fn,
        )
        .map_err(|_e| OperationError::Cancelled)?;
        combined.moves.extend(tp.moves);
    }
    record_offset_library_failures(ctx.findings, offset_failures.get());
    let generated = generated_with_depth_run_spans(combined, &levels);
    if let Some(sem) = ctx.semantic_ctx {
        crate::compute::annotate::annotate_trace_spans(&generated.spans, &generated.toolpath, sem);
    }
    Ok(generated)
}

/// Profile family adapter (per-level passes; tabs on the final level).
/// Cancellable: the cooperative cancel closure is rebuilt from
/// `ctx.cancel` (pinned by `cancellable_families_honour_a_preset_cancel_flag`).
///
/// Migrated onto the shared `toolpath_at_levels_with_cancel` choke point
/// (planning/finishing_stack_review_2026-07.md S.5). The old manual
/// `level_idx > 0 && !combined.moves.is_empty()` retract guard is
/// equivalent to the helper's unconditional `i > 0` retract: every
/// per-level pass already ends with its own retract-to-`safe_z` (via
/// `profile_toolpath`'s emitter), so `Toolpath::final_retract` is always a
/// no-op there regardless of which guard is used — byte-identical output.
pub(crate) fn generate_profile(
    ctx: &ExecutionContext<'_>,
    op: &OperationConfig,
) -> Result<GeneratedToolpath, OperationError> {
    let cfg = config_guard!(op, Profile, "generate_profile");
    let polys = require_polygons(ctx.polygons)?;
    let levels = effective_levels(ctx.cutting_levels, ctx.heights, cfg.depth_per_pass);
    let final_z = levels
        .last()
        .copied()
        .unwrap_or(ctx.heights.top_z - cfg.depth);
    let tool_radius = ctx.tool_def.radius();
    let safe_z = ctx.heights.retract_z;
    let feed_rate = op.feed_rate();
    let plunge_rate = op.plunge_rate();
    let cancel_fn = || ctx.cancel.load(Ordering::SeqCst);
    let mut combined = Toolpath::new();
    // Checkpoint C: see `generate_zigzag` for why this is a local `Cell`.
    let offset_failures = std::cell::Cell::new(0usize);
    for poly in polys {
        // G2: the compensation offset that produces the tool-centre path is
        // Z-independent, so it runs ONCE per polygon. Tabs stay inside the
        // closure — they are applied to the FINAL level only, which is the one
        // genuinely per-level thing this family does.
        if ctx.cancel.load(Ordering::SeqCst) {
            return Err(OperationError::Cancelled);
        }
        let base = crate::ops::profile::ProfileParams {
            tool_radius,
            side: cfg.side,
            cut_depth: 0.0,
            feed_rate,
            plunge_rate,
            safe_z,
            climb: cfg.climb,
            compensate_in_controller: cfg.compensation
                == crate::compute::operation_configs::CompensationType::InControl,
        };
        let (contour, failures) = crate::ops::profile::profile_path_reported(poly, &base);
        offset_failures.set(offset_failures.get() + failures);
        let tp = crate::ops::depth::toolpath_at_levels_with_cancel(
            &levels,
            safe_z,
            |z| {
                let pass_tp = match &contour {
                    Some(pts) => crate::ops::profile::profile_path_to_toolpath(
                        pts,
                        &crate::ops::profile::ProfileParams {
                            cut_depth: z,
                            ..base
                        },
                    ),
                    None => Toolpath::new(),
                };
                if pass_tp.moves.is_empty() {
                    return Ok(pass_tp);
                }
                let is_final = (z - final_z).abs() < 1e-9;
                if cfg.tab_count > 0 && is_final {
                    Ok(crate::dressup::apply_tabs(
                        pass_tp,
                        &crate::dressup::even_tabs(cfg.tab_count, cfg.tab_width, cfg.tab_height),
                        z,
                    ))
                } else {
                    Ok(pass_tp)
                }
            },
            &cancel_fn,
        )
        .map_err(|_e| OperationError::Cancelled)?;
        combined.moves.extend(tp.moves);
    }
    record_offset_library_failures(ctx.findings, offset_failures.get());
    Ok(with_depth_run_annotation(
        generated_with_depth_run_spans(combined, &levels),
        ctx.semantic_ctx,
    ))
}

/// Pocket family adapter (contour / zigzag pattern per config). Cancellable:
/// the cooperative cancel closure is rebuilt from `ctx.cancel` (pinned by
/// `cancellable_families_honour_a_preset_cancel_flag`). The Contour pattern
/// additionally polls per offset ring via `pocket_toolpath_with_cancel`
/// (planning/finishing_stack_review_2026-07.md S.5 — pocket's own
/// unbounded `loop {}`).
pub(crate) fn generate_pocket(
    ctx: &ExecutionContext<'_>,
    op: &OperationConfig,
) -> Result<GeneratedToolpath, OperationError> {
    let cfg = config_guard!(op, Pocket, "generate_pocket");
    let polys = require_polygons(ctx.polygons)?;
    let levels = effective_levels(ctx.cutting_levels, ctx.heights, cfg.depth_per_pass);
    let tool_radius = ctx.tool_def.radius();
    let safe_z = ctx.heights.retract_z;
    let feed_rate = op.feed_rate();
    let plunge_rate = op.plunge_rate();
    let cancel_fn = || ctx.cancel.load(Ordering::SeqCst);
    let mut combined = Toolpath::new();
    // Checkpoint C: see `generate_zigzag` for why this is a local `Cell`.
    // Pocket is the family this channel was built for — F-12 — because its
    // cascade's only exit is a collapsed ring and a contained panic looks
    // exactly like one.
    let offset_failures = std::cell::Cell::new(0usize);
    // Checkpoint C, Q3: standing area left by a cascade that hit a bound.
    // `None` = no bound fired anywhere, which is the `truncated_core_mm2`
    // "not measured" reading and must not be coerced to a zero.
    let truncated_by_bound: std::cell::Cell<Option<f64>> = std::cell::Cell::new(None);
    for poly in polys {
        // G2: the 2D geometry — tool compensation plus the whole
        // `OffsetRingSet` cascade for Contour, the wall inset plus scan lines
        // for Zigzag — is Z-independent and is computed ONCE per polygon here.
        // The per-level closures below stamp Z and nothing else. The pattern
        // match is loop-invariant too, so it is hoisted with the geometry.
        if ctx.cancel.load(Ordering::SeqCst) {
            return Err(OperationError::Cancelled);
        }
        let tp = match cfg.pattern {
            crate::compute::operation_configs::PocketPattern::Contour => {
                let base = crate::ops::pocket::PocketParams {
                    tool_radius,
                    stepover: cfg.stepover,
                    cut_depth: 0.0,
                    feed_rate,
                    plunge_rate,
                    safe_z,
                    climb: cfg.climb,
                };
                let (tp, report) =
                    crate::ops::pocket::pocket_toolpath_at_levels_reported_with_cancel(
                        poly, &levels, &base, &cancel_fn,
                    )
                    .map_err(|_e| OperationError::Cancelled)?;
                offset_failures.set(offset_failures.get() + report.offset_failures);
                // Checkpoint C, Q3 (F-10): a cascade stopped by a bound
                // leaves material, and that is exactly what
                // `truncated_core_mm2` already means — "region interior a
                // ring cascade left UNCUT because it hit a cap before
                // collapsing". Recorded on the existing channel rather
                // than a new one, so it reaches narrate, the diagnostics
                // list and MCP with no extra plumbing.
                //
                // G2: the cascade now runs once per polygon rather than once
                // per level, so this area is no longer multiplied by the level
                // count either — it is the standing area of ONE cascade, which
                // is what "material this pocket will not clear" always meant.
                if let Some(standing) = report.truncated_core_mm2 {
                    truncated_by_bound
                        .set(Some(truncated_by_bound.get().unwrap_or(0.0) + standing));
                }
                tp
            }
            crate::compute::operation_configs::PocketPattern::Zigzag => {
                let base = crate::ops::zigzag::ZigzagParams {
                    tool_radius,
                    stepover: cfg.stepover,
                    cut_depth: 0.0,
                    feed_rate,
                    plunge_rate,
                    safe_z,
                    angle: cfg.angle,
                };
                let (lines, failures) = crate::ops::zigzag::zigzag_lines_reported(
                    poly,
                    tool_radius,
                    cfg.stepover,
                    cfg.angle,
                );
                offset_failures.set(offset_failures.get() + failures);
                crate::ops::depth::toolpath_at_levels_with_cancel(
                    &levels,
                    safe_z,
                    |z| {
                        Ok(crate::ops::zigzag::lines_to_toolpath(
                            &lines,
                            &crate::ops::zigzag::ZigzagParams {
                                cut_depth: z,
                                ..base
                            },
                        ))
                    },
                    &cancel_fn,
                )
                .map_err(|_e| OperationError::Cancelled)?
            }
        };
        combined.moves.extend(tp.moves);
    }
    record_offset_library_failures(ctx.findings, offset_failures.get());
    if let Some(standing) = truncated_by_bound.get() {
        record_truncated_core_only(ctx.findings, standing);
    }
    Ok(with_depth_run_annotation(
        generated_with_depth_run_spans(combined, &levels),
        ctx.semantic_ctx,
    ))
}

/// Face family adapter. Cancellable: the cooperative cancel closure is
/// rebuilt from `ctx.cancel` (pinned by
/// `cancellable_families_honour_a_preset_cancel_flag`).
pub(crate) fn generate_face(
    ctx: &ExecutionContext<'_>,
    op: &OperationConfig,
) -> Result<GeneratedToolpath, OperationError> {
    let cfg = config_guard!(op, Face, "generate_face");
    // F-028: face anchors its depth stepping at `heights.top_z`
    // (which under Auto follows `ctx.stock_top_z` after F-028 — so
    // identity setups land at world stock top and non-identity
    // setups at local stock top, both consistent with the frame
    // the toolpath gets stamped into).
    let params = crate::ops::face::FaceParams {
        tool_radius: ctx.tool_def.radius(),
        stepover: cfg.stepover,
        depth: cfg.depth,
        depth_per_pass: cfg.depth_per_pass,
        feed_rate: op.feed_rate(),
        plunge_rate: op.plunge_rate(),
        safe_z: ctx.heights.retract_z,
        stock_offset: cfg.stock_offset,
        direction: cfg.direction,
        stock_top_z: ctx.heights.top_z,
    };
    let cancel_fn = || ctx.cancel.load(Ordering::SeqCst);
    let tp = crate::ops::face::face_toolpath_with_cancel(ctx.stock_bbox, &params, &cancel_fn)
        .map_err(|_e| OperationError::Cancelled)?;
    Ok(with_depth_run_annotation(
        generated_with_depth_run_spans(tp, &[]),
        ctx.semantic_ctx,
    ))
}

/// Adaptive (2D) family adapter. Cancellable; per-level annotation
/// move_index offsetting reproduced verbatim (annotate_adaptive2d
/// consumes the combined-toolpath indices).
pub(crate) fn generate_adaptive(
    ctx: &ExecutionContext<'_>,
    op: &OperationConfig,
) -> Result<GeneratedToolpath, OperationError> {
    let cfg = config_guard!(op, Adaptive, "generate_adaptive");
    let polys = require_polygons(ctx.polygons)?;
    let levels = effective_levels(ctx.cutting_levels, ctx.heights, cfg.depth_per_pass);
    let cancel_fn = || ctx.cancel.load(Ordering::SeqCst);
    let safe_z = ctx.heights.retract_z;
    let mut combined = Toolpath::new();
    let mut all_annotations = Vec::new();
    for poly in polys {
        for (level_i, &z) in levels.iter().enumerate() {
            let params = crate::adaptive::AdaptiveParams {
                tool_radius: ctx.tool_def.radius(),
                stepover: cfg.stepover,
                cut_depth: z,
                feed_rate: op.feed_rate(),
                plunge_rate: op.plunge_rate(),
                safe_z,
                tolerance: cfg.tolerance,
                slot_clearing: cfg.slot_clearing,
                min_cutting_radius: cfg.min_cutting_radius,
                initial_stock: ctx.initial_stock.cloned(),
                cleanup_strategy: cfg.cleanup_strategy,
                engagement_measure: cfg.engagement_measure,
                path_strategy: cfg.path_strategy,
                trochoid_cap_mult: 1.2,
            };
            let (level_tp, mut annotations) =
                crate::adaptive::adaptive_toolpath_structured_annotated_traced_with_cancel(
                    poly,
                    &params,
                    &cancel_fn,
                    ctx.debug_ctx,
                )
                .map_err(|_cancelled| OperationError::Cancelled)?;
            if level_tp.moves.is_empty() {
                continue;
            }
            // Inter-level retract (matching toolpath_at_levels behaviour)
            if level_i > 0 {
                let offset_before = combined.moves.len();
                combined.final_retract(safe_z);
                // Account for any retract moves added
                let retract_added = combined.moves.len() - offset_before;
                // Offset annotation move_index values
                let offset = offset_before + retract_added;
                for ann in &mut annotations {
                    ann.move_index += offset;
                }
            } else {
                let offset = combined.moves.len();
                for ann in &mut annotations {
                    ann.move_index += offset;
                }
            }
            all_annotations.extend(annotations);
            combined.moves.extend(level_tp.moves);
        }
    }
    if let Some(sem) = ctx.semantic_ctx {
        crate::compute::annotate::annotate_adaptive2d(&all_annotations, &combined, sem);
    }
    Ok(with_depth_run_annotation(
        generated_with_depth_run_spans(combined, &levels),
        ctx.semantic_ctx,
    ))
}

/// Compute effective depth levels from pre-computed cutting_levels or DepthStepping.
fn effective_levels(
    cutting_levels: &[f64],
    heights: &ResolvedHeights,
    depth_per_pass: f64,
) -> Vec<f64> {
    if !cutting_levels.is_empty() {
        cutting_levels.to_vec()
    } else {
        let stepping = crate::ops::depth::DepthStepping::new(
            heights.top_z,
            heights.top_z - heights.depth(),
            depth_per_pass,
        );
        stepping.all_levels()
    }
}

// ── Moved-in-crate sentries (WP12) ────────────────────────────────────
//
// The three generation entries above are `pub(crate)`, so a test that calls
// one must live inside the crate. These four modules were integration tests
// under `crates/rs_cam_core/tests/`. WP12 moved them here unchanged: every
// assertion, every fixture and every raw pre-dressup expectation is the one
// the integration file carried. The lint allows are the ones each file's
// own header carried, because `#[cfg(test)]` exempts none of them by
// itself. See `IMPLEMENTATION_PLAN.md` §23 ruling 2.
