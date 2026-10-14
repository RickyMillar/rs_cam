//! The 2.5D clearing family: zigzag, trace, profile, pocket, face and
//! adaptive.
//!
//! Every adapter here steps a 2D region down a ladder of Z levels, so they
//! share `layered::effective_levels`. Pocket runs on the Layered runner
//! (W3.1); the others move to it in W3.2 and W3.3. Split out of
//! `compute/execute.rs` (P4).

use crate::compute::catalog::OperationConfig;
use crate::compute::operation_configs::{PocketConfig, PocketPattern, RegionOrdering};
use crate::geo::P2;
use crate::polygon::Polygon2;
use crate::tool::MillingCutter;
use crate::toolpath::Toolpath;

use super::OffsetFailures;
use super::layered::{LayerCtx, LayeredOp, LevelInfo, RestUse, effective_levels, run_layered};
use super::shared::{generated_with_depth_run_spans, with_depth_run_annotation};
use super::{ExecutionContext, GeneratedToolpath, OperationError, Prepared};

/// Zigzag family adapter. Cancellable: the cooperative cancel closure is
/// built by `ctx.cancel_fn()` (pinned by
/// `every_generator_polls_a_preset_cancel_flag_past_the_prologue`).
pub(crate) fn generate_zigzag(
    ctx: &ExecutionContext<'_>,
    prepared: &Prepared<'_>,
    op: &OperationConfig,
) -> Result<GeneratedToolpath, OperationError> {
    let cfg = config_guard!(op, Zigzag, "generate_zigzag");
    let polys = prepared.polygons()?;
    let levels = effective_levels(ctx.cutting_levels, ctx.heights, cfg.depth_per_pass);
    let tool_radius = ctx.tool_def.radius();
    let safe_z = ctx.heights.retract_z;
    let cancel_fn = ctx.cancel_fn();
    let mut combined = Toolpath::new();
    // Checkpoint C: this op measures its offsets, so it records a zero
    // before its first call (W2.3). Each kernel count is recorded where
    // the kernel returns it. Deliberately not a thread-local collector
    // (`CAVALIER_SHAPE_FAILURE.md` §6 D-1 option C, declined).
    ctx.record(&OffsetFailures(0));
    for poly in polys {
        // G2: the wall inset and the scan-line build are Z-independent, so
        // they run ONCE per polygon. The per-level closure stamps Z only.
        //
        // This is also where the documented `offset_library_failures` x L
        // over-count goes away: the wall inset is now made once per polygon,
        // so the channel counts one failure per failing offset CALL, which is
        // what its own doc always claimed it counted.
        if ctx.is_cancelled() {
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
        ctx.record(&OffsetFailures(failures));
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
        )?;
        combined.moves.extend(tp.moves);
    }
    Ok(with_depth_run_annotation(
        generated_with_depth_run_spans(combined, &levels),
        ctx.semantic_ctx,
    ))
}

/// Trace family adapter. NOTE: trace uses `annotate_trace_spans`, not
/// the generic depth-run annotator — the per-family annotate fn is
/// part of the contract (plan §Phase-5 task 1). Cancellable: the
/// cooperative cancel closure is built by `ctx.cancel_fn()` (pinned by
/// `every_generator_polls_a_preset_cancel_flag_past_the_prologue`).
pub(crate) fn generate_trace(
    ctx: &ExecutionContext<'_>,
    prepared: &Prepared<'_>,
    op: &OperationConfig,
) -> Result<GeneratedToolpath, OperationError> {
    let cfg = config_guard!(op, Trace, "generate_trace");
    let polys = prepared.polygons()?;
    let levels = effective_levels(ctx.cutting_levels, ctx.heights, cfg.depth_per_pass);
    let safe_z = ctx.heights.retract_z;
    let cancel_fn = ctx.cancel_fn();
    let mut combined = Toolpath::new();
    // Checkpoint C: see `generate_zigzag` for the measured zero.
    ctx.record(&OffsetFailures(0));
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
        if ctx.is_cancelled() {
            return Err(OperationError::Cancelled);
        }
        let (rings, failures) =
            crate::ops::trace_path::trace_compensated_polygons_reported(poly, &params);
        ctx.record(&OffsetFailures(failures));
        let tp = crate::ops::depth::toolpath_at_levels_with_cancel(
            &levels,
            safe_z,
            |z| {
                Ok(crate::ops::trace_path::trace_polygons_at_z(
                    &rings, z, &params,
                ))
            },
            &cancel_fn,
        )?;
        combined.moves.extend(tp.moves);
    }
    let generated = generated_with_depth_run_spans(combined, &levels);
    if let Some(sem) = ctx.semantic_ctx {
        crate::compute::annotate::annotate_trace_spans(&generated.spans, &generated.toolpath, sem);
    }
    Ok(generated)
}

/// Profile family adapter (per-level passes; tabs on the final level).
/// Cancellable: the cooperative cancel closure is built by
/// `ctx.cancel_fn()` (pinned by `every_generator_polls_a_preset_cancel_flag_past_the_prologue`).
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
    prepared: &Prepared<'_>,
    op: &OperationConfig,
) -> Result<GeneratedToolpath, OperationError> {
    let cfg = config_guard!(op, Profile, "generate_profile");
    let polys = prepared.polygons()?;
    let levels = effective_levels(ctx.cutting_levels, ctx.heights, cfg.depth_per_pass);
    let final_z = levels
        .last()
        .copied()
        .unwrap_or(ctx.heights.top_z - cfg.depth);
    let tool_radius = ctx.tool_def.radius();
    let safe_z = ctx.heights.retract_z;
    let feed_rate = op.feed_rate();
    let plunge_rate = op.plunge_rate();
    let cancel_fn = ctx.cancel_fn();
    let mut combined = Toolpath::new();
    // Checkpoint C: see `generate_zigzag` for the measured zero.
    ctx.record(&OffsetFailures(0));
    for poly in polys {
        // G2: the compensation offset that produces the tool-centre path is
        // Z-independent, so it runs ONCE per polygon. Tabs stay inside the
        // closure — they are applied to the FINAL level only, which is the one
        // genuinely per-level thing this family does.
        if ctx.is_cancelled() {
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
        ctx.record(&OffsetFailures(failures));
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
        )?;
        combined.moves.extend(tp.moves);
    }
    Ok(with_depth_run_annotation(
        generated_with_depth_run_spans(combined, &levels),
        ctx.semantic_ctx,
    ))
}

/// Pocket family adapter (W3.1): the Contour or the Zigzag pattern on the
/// Layered runner (`layered.rs`). The runner owns the ladder, the rest clip
/// per level (G-RESTLEVELS, `RestUse::LevelClip`), one prepare per distinct
/// region, the region order, the cancel poll per level and the spans. The
/// Contour cascade also polls per offset ring (S.5: pocket's own unbounded
/// `loop {}`).
pub(crate) fn generate_pocket(
    ctx: &ExecutionContext<'_>,
    prepared: &Prepared<'_>,
    op: &OperationConfig,
) -> Result<GeneratedToolpath, OperationError> {
    let cfg = config_guard!(op, Pocket, "generate_pocket");
    let pass = PocketPass {
        cfg,
        tool_radius: ctx.tool_def.radius(),
        safe_z: ctx.heights.retract_z,
        feed_rate: op.feed_rate(),
        plunge_rate: op.plunge_rate(),
    };
    match cfg.pattern {
        PocketPattern::Contour => run_layered(&PocketContour(pass), ctx, prepared),
        PocketPattern::Zigzag => run_layered(&PocketZigzag(pass), ctx, prepared),
    }
}

/// The dials both Pocket patterns read.
struct PocketPass<'a> {
    cfg: &'a PocketConfig,
    tool_radius: f64,
    safe_z: f64,
    feed_rate: f64,
    plunge_rate: f64,
}

/// The Contour pattern: the `OffsetRingSet` cascade of each region.
struct PocketContour<'a>(PocketPass<'a>);

/// The Zigzag pattern: the wall inset and the scan lines of each region.
struct PocketZigzag<'a>(PocketPass<'a>);

impl LayeredOp for PocketContour<'_> {
    type Geometry = Vec<Vec<P2>>;
    /// One report per piece. Each cascade adds its offset failures and,
    /// when a bound stopped it (Checkpoint C, Q3, F-10), the area it
    /// leaves standing. The runner records it once per region, never once
    /// per level.
    type Report = Vec<crate::ops::pocket::PocketCascadeReport>;

    fn depth_per_pass(&self) -> f64 {
        self.0.cfg.depth_per_pass
    }

    fn region_ordering(&self) -> RegionOrdering {
        self.0.cfg.region_ordering
    }

    fn rest_use(&self) -> RestUse {
        RestUse::LevelClip
    }

    fn prepare(
        &self,
        region: &[Polygon2],
        layer: &LayerCtx<'_>,
    ) -> Result<(Self::Geometry, Self::Report), OperationError> {
        let mut contours = Vec::new();
        let mut reports = Vec::with_capacity(region.len());
        for piece in region {
            let (rings, report) = crate::ops::pocket::pocket_contours_reported_with_cancel(
                piece,
                layer.tool_radius,
                self.0.cfg.stepover,
                layer.cancel,
            )?;
            reports.push(report);
            contours.extend(rings);
        }
        Ok((contours, reports))
    }

    fn emit_level(&self, geometry: &Self::Geometry, level: LevelInfo) -> Toolpath {
        let pass = &self.0;
        crate::ops::pocket::pocket_contours_to_toolpath(
            geometry,
            &crate::ops::pocket::PocketParams {
                tool_radius: pass.tool_radius,
                stepover: pass.cfg.stepover,
                cut_depth: level.z,
                feed_rate: pass.feed_rate,
                plunge_rate: pass.plunge_rate,
                safe_z: pass.safe_z,
                climb: pass.cfg.climb,
            },
        )
    }
}

impl LayeredOp for PocketZigzag<'_> {
    type Geometry = Vec<[P2; 2]>;
    type Report = OffsetFailures;

    fn depth_per_pass(&self) -> f64 {
        self.0.cfg.depth_per_pass
    }

    fn region_ordering(&self) -> RegionOrdering {
        self.0.cfg.region_ordering
    }

    fn rest_use(&self) -> RestUse {
        RestUse::LevelClip
    }

    fn prepare(
        &self,
        region: &[Polygon2],
        layer: &LayerCtx<'_>,
    ) -> Result<(Self::Geometry, Self::Report), OperationError> {
        let mut lines = Vec::new();
        let mut failures = 0;
        for piece in region {
            let (piece_lines, piece_failures) = crate::ops::zigzag::zigzag_lines_reported(
                piece,
                layer.tool_radius,
                self.0.cfg.stepover,
                self.0.cfg.angle,
            );
            failures += piece_failures;
            lines.extend(piece_lines);
        }
        Ok((lines, OffsetFailures(failures)))
    }

    fn emit_level(&self, geometry: &Self::Geometry, level: LevelInfo) -> Toolpath {
        let pass = &self.0;
        crate::ops::zigzag::lines_to_toolpath(
            geometry,
            &crate::ops::zigzag::ZigzagParams {
                tool_radius: pass.tool_radius,
                stepover: pass.cfg.stepover,
                cut_depth: level.z,
                feed_rate: pass.feed_rate,
                plunge_rate: pass.plunge_rate,
                safe_z: pass.safe_z,
                angle: pass.cfg.angle,
            },
        )
    }
}

/// Face family adapter. Cancellable: the cooperative cancel closure is
/// built by `ctx.cancel_fn()` (pinned by
/// `every_generator_polls_a_preset_cancel_flag_past_the_prologue`).
pub(crate) fn generate_face(
    ctx: &ExecutionContext<'_>,
    _prepared: &Prepared<'_>,
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
    let cancel_fn = ctx.cancel_fn();
    let tp = crate::ops::face::face_toolpath_with_cancel(ctx.stock_bbox, &params, &cancel_fn)?;
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
    prepared: &Prepared<'_>,
    op: &OperationConfig,
) -> Result<GeneratedToolpath, OperationError> {
    let cfg = config_guard!(op, Adaptive, "generate_adaptive");
    let polys = prepared.polygons()?;
    let levels = effective_levels(ctx.cutting_levels, ctx.heights, cfg.depth_per_pass);
    let cancel_fn = ctx.cancel_fn();
    let safe_z = ctx.heights.retract_z;
    let mut combined = Toolpath::new();
    let mut all_annotations = Vec::new();
    // G-ADAPTPIECES (D2): the planner plans one piece of the machinable
    // inset, so an outline whose inset splits is planned piece by piece.
    // Each piece goes to full depth before the next, as each outline does.
    let piece_outlines: Vec<Vec<Polygon2>> = polys
        .iter()
        .map(|poly| crate::adaptive::inset_piece_outlines(poly, ctx.tool_def.radius()))
        .collect();
    let outlines = polys
        .iter()
        .zip(&piece_outlines)
        .flat_map(|(poly, pieces)| {
            if pieces.is_empty() {
                std::slice::from_ref(poly)
            } else {
                pieces.as_slice()
            }
        });
    for poly in outlines {
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
                keep_down_links: crate::adaptive::KeepDownLinks::WithinPassLoad,
                // G-ADAPTPASSLOAD: the hole the entry dressup's helix cuts.
                entry_helix_radius: ctx.entry_helix_radius.unwrap_or(0.0),
            };
            let (level_tp, mut annotations) =
                crate::adaptive::adaptive_toolpath_structured_annotated_traced_with_cancel(
                    poly,
                    &params,
                    &cancel_fn,
                    ctx.debug_ctx,
                )?;
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

// ── Moved-in-crate sentries (WP12) ────────────────────────────────────
//
// The three generation entries above are `pub(crate)`, so a test that calls
// one must live inside the crate. These four modules were integration tests
// under `crates/rs_cam_core/tests/`. WP12 moved them here unchanged: every
// assertion, every fixture and every raw pre-dressup expectation is the one
// the integration file carried. The lint allows are the ones each file's
// own header carried, because `#[cfg(test)]` exempts none of them by
// itself. See `IMPLEMENTATION_PLAN.md` §23 ruling 2.
