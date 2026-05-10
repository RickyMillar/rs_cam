//! Convert runtime annotations from individual operation algorithms
//! into hierarchical semantic trace items.
//!
//! Each annotated operation emits a flat list of runtime events tagged with
//! the toolpath move index where they occurred.  This module walks those
//! events and opens/closes semantic scopes so that the toolpath viewer can
//! display a structured breakdown of the algorithm's behaviour.

use crate::semantic_trace::{ToolpathSemanticContext, ToolpathSemanticKind};
use crate::toolpath::Toolpath;
use crate::toolpath_spans::{Span, SpanKind, SpanPayload};

// ── Helpers ──────────────────────────────────────────────────────────

/// Compute the exclusive move-end index for annotation `i`.
///
/// Item `i` covers moves from `annotations[i].move_index` (inclusive) to
/// `annotations[i+1].move_index` (exclusive).  The last item extends to the
/// end of the toolpath.
fn move_end(move_indices: &[usize], i: usize, toolpath_len: usize) -> usize {
    move_indices.get(i + 1).copied().unwrap_or(toolpath_len)
}

/// Build generic semantic `DepthLevel` + child items from structural spans.
///
/// This is used for operations whose structure can be derived from spans even
/// when the generator has no native semantic event stream yet.
pub(super) fn annotate_depth_run_spans(
    spans: &[Span],
    toolpath: &Toolpath,
    op_context: &ToolpathSemanticContext,
) {
    annotate_depth_run_spans_with_region_kind(
        spans,
        toolpath,
        op_context,
        &ToolpathSemanticKind::Region,
    );
}

/// Semantic trace for Trace engraving: depth levels with child chains.
pub(super) fn annotate_trace_spans(
    spans: &[Span],
    toolpath: &Toolpath,
    op_context: &ToolpathSemanticContext,
) {
    annotate_depth_run_spans_with_region_kind(
        spans,
        toolpath,
        op_context,
        &ToolpathSemanticKind::Chain,
    );
}

fn annotate_depth_run_spans_with_region_kind(
    spans: &[Span],
    toolpath: &Toolpath,
    op_context: &ToolpathSemanticContext,
    region_kind: &ToolpathSemanticKind,
) {
    let depth_spans: Vec<&Span> = spans
        .iter()
        .filter(|span| span.kind == SpanKind::DepthPass && !span.is_boundary())
        .collect();
    if depth_spans.is_empty() {
        annotate_region_spans(spans, toolpath, op_context, region_kind);
        return;
    }

    for (depth_index, depth_span) in depth_spans.iter().enumerate() {
        let label = match &depth_span.payload {
            Some(SpanPayload::DepthPass {
                z_level,
                pass_index,
            }) => format!("Z {z_level:.3} pass {}", pass_index + 1),
            _ => format!("Depth pass {}", depth_index + 1),
        };
        let scope = op_context.start_item(ToolpathSemanticKind::DepthLevel, label);
        if let Some(SpanPayload::DepthPass {
            z_level,
            pass_index,
        }) = &depth_span.payload
        {
            scope.set_param("z_level", *z_level);
            scope.set_param("pass_index", *pass_index);
        }
        bind_span_scope(&scope, toolpath, depth_span);
        let child_ctx = scope.context();
        for child in spans.iter().filter(|candidate| {
            candidate.kind == SpanKind::Region
                && !candidate.is_boundary()
                && candidate.start_move >= depth_span.start_move
                && candidate.end_move <= depth_span.end_move
        }) {
            annotate_one_region_span(child, toolpath, &child_ctx, region_kind);
        }
        scope.finish();
    }
}

fn annotate_region_spans(
    spans: &[Span],
    toolpath: &Toolpath,
    op_context: &ToolpathSemanticContext,
    region_kind: &ToolpathSemanticKind,
) {
    for span in spans
        .iter()
        .filter(|span| span.kind == SpanKind::Region && !span.is_boundary())
    {
        annotate_one_region_span(span, toolpath, op_context, region_kind);
    }
}

fn annotate_one_region_span(
    span: &Span,
    toolpath: &Toolpath,
    context: &ToolpathSemanticContext,
    kind: &ToolpathSemanticKind,
) {
    let label = if span.label.is_empty() {
        "Run".to_owned()
    } else {
        span.label.clone().into_owned()
    };
    let scope = context.start_item(kind.clone(), label);
    if let Some(SpanPayload::Region { region_id }) = &span.payload {
        scope.set_param("region_id", *region_id);
    }
    bind_span_scope(&scope, toolpath, span);
    scope.finish();
}

/// Semantic trace for drill-like operations: `Hole` items with child `Cycle`
/// items for each plunge/peck feed move.
pub(super) fn annotate_drill_spans(
    spans: &[Span],
    toolpath: &Toolpath,
    op_context: &ToolpathSemanticContext,
) {
    for hole_span in spans.iter().filter(|span| {
        span.kind == SpanKind::Region
            && !span.is_boundary()
            && span.label.starts_with("Hole ")
            && !span.label.contains("plunge")
    }) {
        let scope = op_context.start_item(ToolpathSemanticKind::Hole, hole_span.label.clone());
        if let Some(SpanPayload::Region { region_id }) = &hole_span.payload {
            scope.set_param("hole_index", *region_id);
        }
        bind_span_scope(&scope, toolpath, hole_span);
        let child_ctx = scope.context();
        for plunge_span in spans.iter().filter(|candidate| {
            candidate.kind == SpanKind::Region
                && !candidate.is_boundary()
                && candidate.label.contains("plunge")
                && candidate.start_move >= hole_span.start_move
                && candidate.end_move <= hole_span.end_move
        }) {
            let cycle =
                child_ctx.start_item(ToolpathSemanticKind::Cycle, plunge_span.label.clone());
            if let Some(SpanPayload::Region { region_id }) = &plunge_span.payload {
                cycle.set_param("cycle_index", *region_id);
            }
            bind_span_scope(&cycle, toolpath, plunge_span);
            cycle.finish();
        }
        scope.finish();
    }
}

fn bind_span_scope(
    scope: &crate::semantic_trace::ToolpathSemanticScope,
    toolpath: &Toolpath,
    span: &Span,
) {
    if span.end_move > span.start_move && span.end_move <= toolpath.moves.len() {
        scope.bind_to_toolpath(toolpath, span.start_move, span.end_move);
    }
}

// ── Adaptive 3D ─────────────────────────────────────────────────────

pub(super) fn annotate_adaptive3d(
    events: &[crate::adaptive3d::Adaptive3dRuntimeAnnotation],
    toolpath: &Toolpath,
    op_context: &ToolpathSemanticContext,
) {
    use crate::adaptive3d::Adaptive3dRuntimeEvent;

    if events.is_empty() {
        return;
    }

    let move_indices: Vec<usize> = events.iter().map(|a| a.move_index).collect();
    let tp_len = toolpath.moves.len();

    // We track open scopes so we can bind them when the next event arrives.
    let mut region_scope: Option<crate::semantic_trace::ToolpathSemanticScope> = None;
    let mut region_ctx: Option<ToolpathSemanticContext> = None;
    let mut level_scope: Option<crate::semantic_trace::ToolpathSemanticScope> = None;
    let mut level_ctx: Option<ToolpathSemanticContext> = None;

    for (i, ann) in events.iter().enumerate() {
        let end = move_end(&move_indices, i, tp_len);

        match &ann.event {
            Adaptive3dRuntimeEvent::RegionStart {
                region_index,
                region_total,
                cell_count,
            } => {
                // Close any prior level/region scopes
                if let Some(ls) = level_scope.take() {
                    ls.finish();
                }
                level_ctx = None;
                if let Some(rs) = region_scope.take() {
                    rs.finish();
                }
                let scope = op_context.start_item(
                    ToolpathSemanticKind::Region,
                    format!("Region {}", region_index + 1),
                );
                scope.set_param("region_index", *region_index);
                scope.set_param("region_total", *region_total);
                scope.set_param("cell_count", *cell_count);
                let ctx = scope.context();
                region_ctx = Some(ctx);
                region_scope = Some(scope);
            }
            Adaptive3dRuntimeEvent::RegionZLevel {
                region_index: _,
                z_level,
                level_index,
                level_total,
                metrics,
            } => {
                if let Some(ls) = level_scope.take() {
                    ls.finish();
                }
                let parent = region_ctx.as_ref().unwrap_or(op_context);
                let scope =
                    parent.start_item(ToolpathSemanticKind::DepthLevel, format!("Z {z_level:.2}"));
                scope.set_param("z_level", *z_level);
                scope.set_param("level_index", *level_index);
                scope.set_param("level_total", *level_total);
                set_z_level_plan_metrics(&scope, metrics);
                scope.bind_to_toolpath(toolpath, ann.move_index, end);
                let ctx = scope.context();
                level_ctx = Some(ctx);
                level_scope = Some(scope);
            }
            Adaptive3dRuntimeEvent::GlobalZLevel {
                z_level,
                level_index,
                level_total,
                metrics,
            } => {
                // Close any prior level/region scopes
                if let Some(ls) = level_scope.take() {
                    ls.finish();
                }
                level_ctx.take();
                if let Some(rs) = region_scope.take() {
                    rs.finish();
                }
                region_ctx.take();
                let scope = op_context.start_item(
                    ToolpathSemanticKind::DepthLevel,
                    format!("Global Z {z_level:.2}"),
                );
                scope.set_param("z_level", *z_level);
                scope.set_param("level_index", *level_index);
                scope.set_param("level_total", *level_total);
                set_z_level_plan_metrics(&scope, metrics);
                scope.bind_to_toolpath(toolpath, ann.move_index, end);
                let ctx = scope.context();
                level_ctx = Some(ctx);
                level_scope = Some(scope);
            }
            Adaptive3dRuntimeEvent::WaterlineCleanup => {
                let parent = level_ctx.as_ref().unwrap_or(op_context);
                let scope = parent.start_item(
                    ToolpathSemanticKind::Cleanup,
                    "Waterline cleanup".to_owned(),
                );
                scope.bind_to_toolpath(toolpath, ann.move_index, end);
                scope.finish();
            }
            Adaptive3dRuntimeEvent::PassEntry {
                pass_index,
                entry_x,
                entry_y,
                entry_z,
                entry_end_move_idx,
                style_label,
            } => {
                let parent = level_ctx.as_ref().unwrap_or(op_context);
                let scope = parent.start_item(
                    ToolpathSemanticKind::Entry,
                    format!("Pass {} {}", pass_index + 1, style_label),
                );
                scope.set_param("pass_index", *pass_index);
                scope.set_param("entry_x", *entry_x);
                scope.set_param("entry_y", *entry_y);
                scope.set_param("entry_z", *entry_z);
                scope.set_param("style", *style_label);
                // D4 — entry sequence runs from the event's emit
                // index (entry_start) to its captured end index. Use
                // the explicit end so the semantic scope matches the
                // structural span built in `compute::spans`.
                scope.bind_to_toolpath(toolpath, ann.move_index, *entry_end_move_idx);
                let _ = end;
                scope.finish();
            }
            Adaptive3dRuntimeEvent::PassPreflightSkip { pass_index } => {
                let parent = level_ctx.as_ref().unwrap_or(op_context);
                let scope = parent.start_item(
                    ToolpathSemanticKind::Pass,
                    format!("Pass {} skipped (preflight)", pass_index + 1),
                );
                scope.set_param("pass_index", *pass_index);
                scope.set_param("skipped", true);
                scope.finish();
            }
            Adaptive3dRuntimeEvent::PassSummary {
                pass_index,
                step_count,
                exit_reason,
                yield_ratio,
                short,
            } => {
                let parent = level_ctx.as_ref().unwrap_or(op_context);
                let scope = parent.start_item(
                    ToolpathSemanticKind::Pass,
                    format!("Pass {}", pass_index + 1),
                );
                scope.set_param("pass_index", *pass_index);
                scope.set_param("step_count", *step_count);
                scope.set_param("exit_reason", exit_reason.clone());
                scope.set_param("yield_ratio", *yield_ratio);
                scope.set_param("short", *short);
                scope.bind_to_toolpath(toolpath, ann.move_index, end);
                scope.finish();
            }
        }
    }

    // Close any remaining open scopes
    if let Some(ls) = level_scope {
        ls.finish();
    }
    if let Some(rs) = region_scope {
        rs.finish();
    }
}

fn set_z_level_plan_metrics(
    scope: &crate::semantic_trace::ToolpathSemanticScope,
    metrics: &crate::adaptive3d::ZLevelPlanMetrics,
) {
    if !metrics.available {
        return;
    }
    scope.set_param("marching_squares_regions", metrics.marching_squares_regions);
    scope.set_param("region_areas_mm2", metrics.region_areas_mm2.clone());
    scope.set_param(
        "dropped_micro_region_count",
        metrics.dropped_micro_region_count,
    );
    scope.set_param(
        "perimeter_sweep_length_mm",
        metrics.perimeter_sweep_length_mm,
    );
    scope.set_param("agent_walk_cut_length_mm", metrics.agent_walk_cut_length_mm);
    scope.set_param(
        "residual_cleanup_cell_count",
        metrics.residual_cleanup_cell_count,
    );
}

// ── Adaptive 2D ─────────────────────────────────────────────────────

pub(super) fn annotate_adaptive2d(
    events: &[crate::adaptive::AdaptiveRuntimeAnnotation],
    toolpath: &Toolpath,
    op_context: &ToolpathSemanticContext,
) {
    use crate::adaptive::AdaptiveRuntimeEvent;

    if events.is_empty() {
        return;
    }

    let move_indices: Vec<usize> = events.iter().map(|a| a.move_index).collect();
    let tp_len = toolpath.moves.len();

    for (i, ann) in events.iter().enumerate() {
        let end = move_end(&move_indices, i, tp_len);

        match &ann.event {
            AdaptiveRuntimeEvent::SlotClearing {
                line_index,
                line_total,
            } => {
                let scope = op_context.start_item(
                    ToolpathSemanticKind::SlotClearing,
                    format!("Slot line {}/{line_total}", line_index + 1),
                );
                scope.set_param("line_index", *line_index);
                scope.set_param("line_total", *line_total);
                scope.bind_to_toolpath(toolpath, ann.move_index, end);
                scope.finish();
            }
            AdaptiveRuntimeEvent::PassEntry {
                pass_index,
                entry_x,
                entry_y,
            } => {
                let scope = op_context.start_item(
                    ToolpathSemanticKind::Entry,
                    format!("Pass {} entry", pass_index + 1),
                );
                scope.set_param("pass_index", *pass_index);
                scope.set_param("entry_x", *entry_x);
                scope.set_param("entry_y", *entry_y);
                scope.bind_to_toolpath(toolpath, ann.move_index, end);
                scope.finish();
            }
            AdaptiveRuntimeEvent::PassSummary {
                pass_index,
                step_count,
                idle_count,
                search_evaluations,
                exit_reason,
            } => {
                let scope = op_context.start_item(
                    ToolpathSemanticKind::Pass,
                    format!("Pass {}", pass_index + 1),
                );
                scope.set_param("pass_index", *pass_index);
                scope.set_param("step_count", *step_count);
                scope.set_param("idle_count", *idle_count);
                scope.set_param("search_evaluations", *search_evaluations);
                scope.set_param("exit_reason", exit_reason.clone());
                scope.bind_to_toolpath(toolpath, ann.move_index, end);
                scope.finish();
            }
            AdaptiveRuntimeEvent::ForcedClear {
                pass_index,
                center_x,
                center_y,
                radius,
            } => {
                let scope = op_context.start_item(
                    ToolpathSemanticKind::ForcedClear,
                    format!("Forced clear (pass {})", pass_index + 1),
                );
                scope.set_param("pass_index", *pass_index);
                scope.set_param("center_x", *center_x);
                scope.set_param("center_y", *center_y);
                scope.set_param("radius", *radius);
                scope.bind_to_toolpath(toolpath, ann.move_index, end);
                scope.finish();
            }
            AdaptiveRuntimeEvent::BoundaryCleanup {
                contour_index,
                contour_total,
            } => {
                let scope = op_context.start_item(
                    ToolpathSemanticKind::Cleanup,
                    format!("Boundary cleanup {}/{contour_total}", contour_index + 1),
                );
                scope.set_param("contour_index", *contour_index);
                scope.set_param("contour_total", *contour_total);
                scope.bind_to_toolpath(toolpath, ann.move_index, end);
                scope.finish();
            }
        }
    }
}

// ── Scallop ─────────────────────────────────────────────────────────

pub(super) fn annotate_scallop(
    events: &[crate::scallop::ScallopRuntimeAnnotation],
    toolpath: &Toolpath,
    op_context: &ToolpathSemanticContext,
) {
    if events.is_empty() {
        return;
    }

    let move_indices: Vec<usize> = events.iter().map(|a| a.move_index).collect();
    let tp_len = toolpath.moves.len();

    for (i, ann) in events.iter().enumerate() {
        let end = move_end(&move_indices, i, tp_len);

        let crate::scallop::ScallopRuntimeEvent::Ring {
            ring_index,
            ring_total,
            continuous,
        } = &ann.event;

        let scope = op_context.start_item(
            ToolpathSemanticKind::Ring,
            format!("Ring {}/{ring_total}", ring_index + 1),
        );
        scope.set_param("ring_index", *ring_index);
        scope.set_param("ring_total", *ring_total);
        scope.set_param("continuous", *continuous);
        scope.bind_to_toolpath(toolpath, ann.move_index, end);
        scope.finish();
    }
}

// ── RampFinish ──────────────────────────────────────────────────────

pub(super) fn annotate_ramp_finish(
    events: &[crate::ramp_finish::RampFinishRuntimeAnnotation],
    toolpath: &Toolpath,
    op_context: &ToolpathSemanticContext,
) {
    if events.is_empty() {
        return;
    }

    let move_indices: Vec<usize> = events.iter().map(|a| a.move_index).collect();
    let tp_len = toolpath.moves.len();

    for (i, ann) in events.iter().enumerate() {
        let end = move_end(&move_indices, i, tp_len);

        let crate::ramp_finish::RampFinishRuntimeEvent::Ramp {
            terrace_index,
            terrace_total,
            upper_level_index,
            lower_level_index,
            upper_z,
            lower_z,
            ramp_index,
            ramp_total,
        } = &ann.event;

        let scope = op_context.start_item(
            ToolpathSemanticKind::Ramp,
            format!(
                "Terrace {} ramp {}/{}",
                terrace_index + 1,
                ramp_index + 1,
                ramp_total
            ),
        );
        scope.set_param("terrace_index", *terrace_index);
        scope.set_param("terrace_total", *terrace_total);
        scope.set_param("upper_level_index", *upper_level_index);
        scope.set_param("lower_level_index", *lower_level_index);
        scope.set_param("upper_z", *upper_z);
        scope.set_param("lower_z", *lower_z);
        scope.set_param("ramp_index", *ramp_index);
        scope.set_param("ramp_total", *ramp_total);
        scope.bind_to_toolpath(toolpath, ann.move_index, end);
        scope.finish();
    }
}

// ── SpiralFinish ────────────────────────────────────────────────────

pub(super) fn annotate_spiral_finish(
    events: &[crate::spiral_finish::SpiralFinishRuntimeAnnotation],
    toolpath: &Toolpath,
    op_context: &ToolpathSemanticContext,
) {
    if events.is_empty() {
        return;
    }

    let move_indices: Vec<usize> = events.iter().map(|a| a.move_index).collect();
    let tp_len = toolpath.moves.len();

    for (i, ann) in events.iter().enumerate() {
        let end = move_end(&move_indices, i, tp_len);

        let crate::spiral_finish::SpiralFinishRuntimeEvent::Ring {
            ring_index,
            ring_total,
            radius_mm,
        } = &ann.event;

        let scope = op_context.start_item(
            ToolpathSemanticKind::Ring,
            format!("Ring {}/{ring_total}", ring_index + 1),
        );
        scope.set_param("ring_index", *ring_index);
        scope.set_param("ring_total", *ring_total);
        scope.set_param("radius_mm", *radius_mm);
        scope.bind_to_toolpath(toolpath, ann.move_index, end);
        scope.finish();
    }
}

// ── Pencil ──────────────────────────────────────────────────────────

pub(super) fn annotate_pencil(
    events: &[crate::pencil::PencilRuntimeAnnotation],
    toolpath: &Toolpath,
    op_context: &ToolpathSemanticContext,
) {
    if events.is_empty() {
        return;
    }

    let move_indices: Vec<usize> = events.iter().map(|a| a.move_index).collect();
    let tp_len = toolpath.moves.len();

    for (i, ann) in events.iter().enumerate() {
        let end = move_end(&move_indices, i, tp_len);

        let crate::pencil::PencilRuntimeEvent::OffsetPass {
            chain_index,
            chain_total,
            offset_index,
            offset_total,
            offset_mm,
            is_centerline,
        } = &ann.event;

        let kind = if *is_centerline {
            ToolpathSemanticKind::Centerline
        } else {
            ToolpathSemanticKind::OffsetPass
        };
        let label = if *is_centerline {
            format!("Chain {} centerline", chain_index + 1)
        } else {
            format!(
                "Chain {} offset {}/{}",
                chain_index + 1,
                offset_index + 1,
                offset_total,
            )
        };

        let scope = op_context.start_item(kind, label);
        scope.set_param("chain_index", *chain_index);
        scope.set_param("chain_total", *chain_total);
        scope.set_param("offset_index", *offset_index);
        scope.set_param("offset_total", *offset_total);
        scope.set_param("offset_mm", *offset_mm);
        scope.set_param("is_centerline", *is_centerline);
        scope.bind_to_toolpath(toolpath, ann.move_index, end);
        scope.finish();
    }
}
