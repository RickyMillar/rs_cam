//! Convert runtime annotations from individual operation algorithms
//! into hierarchical semantic trace items.
//!
//! Each annotated operation emits a flat list of runtime events tagged with
//! the toolpath move index where they occurred.  This module walks those
//! events and opens/closes semantic scopes so that the toolpath viewer can
//! display a structured breakdown of the algorithm's behaviour.

use crate::semantic_trace::{SemanticKey, ToolpathSemanticContext, ToolpathSemanticKind};
use crate::toolpath::Toolpath;
use crate::toolpath_spans::{RegionSpanRole, Span, SpanKind, SpanPayload};

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

/// Semantic trace for Trace engraving: ONE region, depth levels inside it,
/// chains inside those.
///
/// C8: `narrate_toolpath` reported `regions 0` for Trace. The cause is not a
/// missing projection — it is a NAMING divergence. Fifteen operation
/// families route their structural `SpanKind::Region` spans through
/// [`annotate_depth_run_spans`] and get `Region` items; Trace calls the same
/// helper with `ToolpathSemanticKind::Chain` instead, because an engraving
/// run is a contour chain rather than an area, and `sim_debug` colours the
/// two differently. Both readings are defensible, and the result was that
/// Trace's structure appeared in narration's `depth levels D, regions R,
/// rings G` line as no structure at all.
///
/// So the chains stay chains — they are chains — and the operation gains
/// the region it genuinely has: ONE. Trace engraves the model's contour set;
/// it does not partition anything, and claiming a per-contour partition
/// would report a structure the generator does not have. Narration now
/// reads `regions 1` and, with the C8 chain counter, `chains N`.
pub(super) fn annotate_trace_spans(
    spans: &[Span],
    toolpath: &Toolpath,
    op_context: &ToolpathSemanticContext,
) {
    let region_scope = op_context.start_item(
        ToolpathSemanticKind::Region,
        "Region 1/1 (trace)".to_owned(),
    );
    region_scope.set_param(SemanticKey::RegionIndex, 0usize);
    region_scope.set_param(SemanticKey::RegionTotal, 1usize);
    region_scope.set_param(SemanticKey::Strategy, "trace");
    if !toolpath.moves.is_empty() {
        region_scope.bind_to_toolpath(toolpath, 0, toolpath.moves.len());
    }
    annotate_depth_run_spans_with_region_kind(
        spans,
        toolpath,
        &region_scope.context(),
        &ToolpathSemanticKind::Chain,
    );
    region_scope.finish();
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
            scope.set_param(SemanticKey::ZLevel, *z_level);
            scope.set_param(SemanticKey::PassIndex, *pass_index);
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
    if let Some(SpanPayload::Region { region_id, .. }) = &span.payload {
        scope.set_param(SemanticKey::RegionId, *region_id);
    }
    bind_span_scope(&scope, toolpath, span);
    scope.finish();
}

/// Semantic trace for drill-like operations: `Hole` items with child `Cycle`
/// items for each plunge/peck feed move.
///
/// C4: the hole/peck split is read off [`RegionSpanRole`], not off the label.
/// This function used to reconstruct the nesting with
/// `label.starts_with("Hole ") && !label.contains("plunge")` for the parent
/// and `label.contains("plunge")` for the child — a contract
/// `spans_from_drill_holes` was never obliged to keep, one `format!` away
/// from silently producing a flat trace with every peck promoted to a hole.
/// The labels still travel through to the semantic items, because that is
/// what a human reads; nothing branches on them.
pub(super) fn annotate_drill_spans(
    spans: &[Span],
    toolpath: &Toolpath,
    op_context: &ToolpathSemanticContext,
) {
    for hole_span in spans
        .iter()
        .filter(|span| span.has_region_role(RegionSpanRole::DrillHole))
    {
        let scope = op_context.start_item(ToolpathSemanticKind::Hole, hole_span.label.clone());
        if let Some(SpanPayload::Region { region_id, .. }) = &hole_span.payload {
            scope.set_param(SemanticKey::HoleIndex, *region_id);
        }
        bind_span_scope(&scope, toolpath, hole_span);
        let child_ctx = scope.context();
        for plunge_span in spans.iter().filter(|candidate| {
            candidate.has_region_role(RegionSpanRole::DrillPeck)
                && candidate.start_move >= hole_span.start_move
                && candidate.end_move <= hole_span.end_move
        }) {
            let cycle =
                child_ctx.start_item(ToolpathSemanticKind::Cycle, plunge_span.label.clone());
            if let Some(SpanPayload::Region { region_id, .. }) = &plunge_span.payload {
                cycle.set_param(SemanticKey::CycleIndex, *region_id);
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
                scope.set_param(SemanticKey::RegionIndex, *region_index);
                scope.set_param(SemanticKey::RegionTotal, *region_total);
                scope.set_param(SemanticKey::CellCount, *cell_count);
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
                scope.set_param(SemanticKey::ZLevel, *z_level);
                scope.set_param(SemanticKey::LevelIndex, *level_index);
                scope.set_param(SemanticKey::LevelTotal, *level_total);
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
                scope.set_param(SemanticKey::ZLevel, *z_level);
                scope.set_param(SemanticKey::LevelIndex, *level_index);
                scope.set_param(SemanticKey::LevelTotal, *level_total);
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
                scope.set_param(SemanticKey::PassIndex, *pass_index);
                scope.set_param(SemanticKey::EntryX, *entry_x);
                scope.set_param(SemanticKey::EntryY, *entry_y);
                scope.set_param(SemanticKey::EntryZ, *entry_z);
                scope.set_param(SemanticKey::Style, *style_label);
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
                scope.set_param(SemanticKey::PassIndex, *pass_index);
                scope.set_param(SemanticKey::Skipped, true);
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
                scope.set_param(SemanticKey::PassIndex, *pass_index);
                scope.set_param(SemanticKey::StepCount, *step_count);
                scope.set_param(SemanticKey::ExitReason, exit_reason.clone());
                scope.set_param(SemanticKey::YieldRatio, *yield_ratio);
                scope.set_param(SemanticKey::Short, *short);
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
    scope.set_param(
        SemanticKey::MarchingSquaresRegions,
        metrics.marching_squares_regions,
    );
    scope.set_param(
        SemanticKey::RegionAreasMm2,
        metrics.region_areas_mm2.clone(),
    );
    scope.set_param(
        SemanticKey::DroppedMicroRegionCount,
        metrics.dropped_micro_region_count,
    );
    scope.set_param(
        SemanticKey::PerimeterSweepLengthMm,
        metrics.perimeter_sweep_length_mm,
    );
    scope.set_param(
        SemanticKey::AgentWalkCutLengthMm,
        metrics.agent_walk_cut_length_mm,
    );
    scope.set_param(
        SemanticKey::ResidualCleanupCellCount,
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
                scope.set_param(SemanticKey::LineIndex, *line_index);
                scope.set_param(SemanticKey::LineTotal, *line_total);
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
                scope.set_param(SemanticKey::PassIndex, *pass_index);
                scope.set_param(SemanticKey::EntryX, *entry_x);
                scope.set_param(SemanticKey::EntryY, *entry_y);
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
                scope.set_param(SemanticKey::PassIndex, *pass_index);
                scope.set_param(SemanticKey::StepCount, *step_count);
                scope.set_param(SemanticKey::IdleCount, *idle_count);
                scope.set_param(SemanticKey::SearchEvaluations, *search_evaluations);
                scope.set_param(SemanticKey::ExitReason, exit_reason.clone());
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
                scope.set_param(SemanticKey::PassIndex, *pass_index);
                scope.set_param(SemanticKey::CenterX, *center_x);
                scope.set_param(SemanticKey::CenterY, *center_y);
                scope.set_param(SemanticKey::Radius, *radius);
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
                scope.set_param(SemanticKey::ContourIndex, *contour_index);
                scope.set_param(SemanticKey::ContourTotal, *contour_total);
                scope.bind_to_toolpath(toolpath, ann.move_index, end);
                scope.finish();
            }
        }
    }
}

// ── Scallop ─────────────────────────────────────────────────────────

/// Whether [`annotate_scallop`] wraps its rings in `Region` items (C8).
///
/// The two `Region` populations in this crate are not the same thing, and
/// `narrate_toolpath` counts them with one counter:
///
/// * the PLANNER's territory nodes, which A/M8 projects for `UnifiedFinish`
///   and whose semantic items must reconcile 1:1 with the structural
///   `RegionSpanRole::Node` spans;
/// * the GENERATOR's own pass groupings, which the 15 families routed
///   through [`annotate_depth_run_spans`] have always emitted under the same
///   kind.
///
/// Scallop is BOTH, depending on who called it. Standalone, its
/// per-boundary ring cascades are the only region structure it has, and
/// hiding them is what produced `regions 0`. Inside `UnifiedFinish` it is a
/// sub-generator filling ONE of the planner's mid-steep nodes, and adding a
/// second region there both double-counts and breaks A/M8's reconciliation
/// gate — which is exactly what it did when this was unconditional.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ScallopRegionGrouping {
    /// Standalone scallop: emit one `Region` per boundary region.
    ByBoundaryRegion,
    /// Scallop as a sub-generator: rings only, so the caller's own region
    /// nodes stay the single region population.
    Flat,
}

/// Rings with no region wrapper — the pre-C8 shape, kept for sub-generator
/// callers. See [`ScallopRegionGrouping`].
fn annotate_scallop_rings_flat(
    events: &[crate::scallop::ScallopRuntimeAnnotation],
    toolpath: &Toolpath,
    op_context: &ToolpathSemanticContext,
) {
    let move_indices: Vec<usize> = events.iter().map(|a| a.move_index).collect();
    let tp_len = toolpath.moves.len();
    for (i, ann) in events.iter().enumerate() {
        let end = move_end(&move_indices, i, tp_len);
        let crate::scallop::ScallopRuntimeEvent::Ring {
            ring_index,
            ring_total,
            continuous,
            ..
        } = &ann.event;
        let scope = op_context.start_item(
            ToolpathSemanticKind::Ring,
            format!("Ring {}/{ring_total}", ring_index + 1),
        );
        scope.set_param(SemanticKey::RingIndex, *ring_index);
        scope.set_param(SemanticKey::RingTotal, *ring_total);
        scope.set_param(SemanticKey::Continuous, *continuous);
        scope.bind_to_toolpath(toolpath, ann.move_index, end);
        scope.finish();
    }
}

/// C8: `Region` items per BOUNDARY REGION, each parenting the `Ring` items
/// its own independent ring cascade produced.
///
/// `narrate_toolpath` reported `regions 0` for scallop — the same structural
/// gap A/M8 closed for `UnifiedFinish`, in a milder form. Scallop generates
/// "one region at a time, concatenated in region order" (P2.3: multiple
/// disjoint machining-boundary regions each get an independent ring set),
/// but the runtime annotation stream carried only a GLOBAL ring index, so
/// the region structure was gone before the semantic trace was built.
///
/// The count is honest at both ends: with no machining boundary the whole
/// mesh footprint is ONE region, so narration reads `regions 1` — which is
/// the truth, and distinguishable from the `0` that used to mean "nothing
/// looked".
///
/// Rings keep their global index in their label, so an operator comparing
/// narration against the GUI span list still sees the same numbering.
pub(super) fn annotate_scallop(
    events: &[crate::scallop::ScallopRuntimeAnnotation],
    toolpath: &Toolpath,
    op_context: &ToolpathSemanticContext,
    group_by_region: ScallopRegionGrouping,
) {
    if events.is_empty() {
        return;
    }
    if group_by_region == ScallopRegionGrouping::Flat {
        annotate_scallop_rings_flat(events, toolpath, op_context);
        return;
    }

    let move_indices: Vec<usize> = events.iter().map(|a| a.move_index).collect();
    let tp_len = toolpath.moves.len();

    // Group CONSECUTIVE events by region. The ring list is concatenated in
    // region order (and reversed wholesale for `InsideOut`), so consecutive
    // grouping reproduces the generator's own partition without assuming
    // the regions arrive in index order.
    let region_of = |ann: &crate::scallop::ScallopRuntimeAnnotation| {
        let crate::scallop::ScallopRuntimeEvent::Ring {
            region_index,
            region_total,
            ..
        } = ann.event;
        (region_index, region_total)
    };

    let mut i = 0usize;
    while i < events.len() {
        let Some((region_index, region_total)) = events.get(i).map(region_of) else {
            break;
        };

        let j = events
            .iter()
            .enumerate()
            .skip(i)
            .find(|(_, ann)| region_of(ann).0 != region_index)
            .map_or(events.len(), |(k, _)| k);

        let region_start = events.get(i).map_or(0, |ann| ann.move_index);
        let region_end = move_end(&move_indices, j.saturating_sub(1), tp_len);
        let region_scope = op_context.start_item(
            ToolpathSemanticKind::Region,
            format!("Region {}/{region_total} (scallop)", region_index + 1),
        );
        region_scope.set_param(SemanticKey::RegionIndex, region_index);
        region_scope.set_param(SemanticKey::RegionTotal, region_total);
        region_scope.set_param(SemanticKey::Strategy, "scallop");
        region_scope.set_param(SemanticKey::RingTotal, j - i);
        if region_end > region_start && region_end <= tp_len {
            region_scope.bind_to_toolpath(toolpath, region_start, region_end);
        }
        let ring_ctx = region_scope.context();

        for (k, ann) in events.iter().enumerate().take(j).skip(i) {
            let end = move_end(&move_indices, k, tp_len);

            let crate::scallop::ScallopRuntimeEvent::Ring {
                ring_index,
                ring_total,
                continuous,
                ..
            } = &ann.event;

            let scope = ring_ctx.start_item(
                ToolpathSemanticKind::Ring,
                format!("Ring {}/{ring_total}", ring_index + 1),
            );
            scope.set_param(SemanticKey::RingIndex, *ring_index);
            scope.set_param(SemanticKey::RingTotal, *ring_total);
            scope.set_param(SemanticKey::Continuous, *continuous);
            scope.bind_to_toolpath(toolpath, ann.move_index, end);
            scope.finish();
        }

        region_scope.finish();
        i = j;
    }
}

// ── UnifiedFinish ───────────────────────────────────────────────────

/// Semantic trace for UnifiedFinish: one `Region` item per routed region
/// node, labelled with its BAND and the STRATEGY that generated it.
///
/// Plan A/M8. UnifiedFinish already emitted STRUCTURAL region-node spans
/// (`unified_finish::unified_finish_spans`), but the semantic trace
/// `narrate_toolpath` reads is a separate system that the op never
/// populated — so the agent-facing diagnostic reported `regions 0` for the
/// one operation whose entire premise is mixing strategies.
///
/// Both systems are built from the same
/// [`crate::unified_finish::RegionAnnotation`] table, and
/// `tests/unified_finish_semantic_regions.rs` asserts they agree on count
/// and move range. This is annotation only: no move is added, removed, or
/// moved.
pub(super) fn annotate_unified_finish_regions(
    regions: &[crate::unified_finish::RegionAnnotation],
    toolpath: &Toolpath,
    op_context: &ToolpathSemanticContext,
) {
    for region in regions {
        let scope = op_context.start_item(ToolpathSemanticKind::Region, region.semantic_label());
        scope.set_param(SemanticKey::RegionId, region.region_id);
        scope.set_param(SemanticKey::Band, region.kind.band_label());
        scope.set_param(SemanticKey::Strategy, region.kind.strategy().label());
        if let Some(area) = region.area_mm2 {
            // M1 serde decision (PR-0): this stays a RAW JSON NUMBER. The
            // semantic-trace wire is read by string key (narration, the MCP
            // `narrate_toolpath` path, the GUI item list), so promoting it to
            // a tagged `{value, domain, stage}` object would break every
            // consumer for provenance that is CONSTANT for this key — the
            // value is always a `ProjectedXyAreaMm2` from the band
            // decomposition. The domain therefore lives in the key's
            // documentation and in
            // `UnifiedFinishReport::provenance`, which the same generation
            // carries, rather than being restated per item.
            //
            // Documented loss: a consumer holding ONLY this JSON cannot tell
            // which grid the area was measured on (X-6). If that ever matters
            // on the wire, add a sibling `area_provenance` string param —
            // additive, no key rename.
            scope.set_param(SemanticKey::AreaMm2, area);
        }
        // Same guard `bind_span_scope` applies to structural spans: an
        // empty or out-of-range node contributes an unlinked item rather
        // than a bogus range.
        if region.move_range.end > region.move_range.start
            && region.move_range.end <= toolpath.moves.len()
        {
            scope.bind_to_toolpath(toolpath, region.move_range.start, region.move_range.end);
        }
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
        scope.set_param(SemanticKey::TerraceIndex, *terrace_index);
        scope.set_param(SemanticKey::TerraceTotal, *terrace_total);
        scope.set_param(SemanticKey::UpperLevelIndex, *upper_level_index);
        scope.set_param(SemanticKey::LowerLevelIndex, *lower_level_index);
        scope.set_param(SemanticKey::UpperZ, *upper_z);
        scope.set_param(SemanticKey::LowerZ, *lower_z);
        scope.set_param(SemanticKey::RampIndex, *ramp_index);
        scope.set_param(SemanticKey::RampTotal, *ramp_total);
        scope.bind_to_toolpath(toolpath, ann.move_index, end);
        scope.finish();
    }
}

// ── SpiralFinish ────────────────────────────────────────────────────

/// C8: ONE `Region` item covering the whole spiral, parenting its `Ring`
/// items.
///
/// `narrate_toolpath` reported `regions 0` here too, and the honest answer
/// is `1` — NOT a per-boundary partition like scallop's. Spiral finish does
/// not generate an independent ring set per machining-boundary region: it
/// walks one continuous archimedean spiral over the whole footprint and
/// SKIPS sample points that fall outside every region
/// (`spiral_finish.rs`'s `in_region` pre-clip). One traversal, one region,
/// however many boundaries were supplied.
///
/// That distinction is why this is not the same code as
/// [`annotate_scallop`]: inventing a per-boundary partition here would
/// report a structure the generator does not have.
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

    let region_start = events.first().map_or(0, |a| a.move_index);
    let region_end = move_end(&move_indices, events.len().saturating_sub(1), tp_len);
    let region_scope = op_context.start_item(
        ToolpathSemanticKind::Region,
        "Region 1/1 (spiral)".to_owned(),
    );
    region_scope.set_param(SemanticKey::RegionIndex, 0usize);
    region_scope.set_param(SemanticKey::RegionTotal, 1usize);
    region_scope.set_param(SemanticKey::Strategy, "spiral");
    region_scope.set_param(SemanticKey::RingTotal, events.len());
    if region_end > region_start && region_end <= tp_len {
        region_scope.bind_to_toolpath(toolpath, region_start, region_end);
    }
    let ring_ctx = region_scope.context();

    for (i, ann) in events.iter().enumerate() {
        let end = move_end(&move_indices, i, tp_len);

        let crate::spiral_finish::SpiralFinishRuntimeEvent::Ring {
            ring_index,
            ring_total,
            radius_mm,
        } = &ann.event;

        let scope = ring_ctx.start_item(
            ToolpathSemanticKind::Ring,
            format!("Ring {}/{ring_total}", ring_index + 1),
        );
        scope.set_param(SemanticKey::RingIndex, *ring_index);
        scope.set_param(SemanticKey::RingTotal, *ring_total);
        scope.set_param(SemanticKey::RadiusMm, *radius_mm);
        scope.bind_to_toolpath(toolpath, ann.move_index, end);
        scope.finish();
    }

    region_scope.finish();
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
        scope.set_param(SemanticKey::ChainIndex, *chain_index);
        scope.set_param(SemanticKey::ChainTotal, *chain_total);
        scope.set_param(SemanticKey::OffsetIndex, *offset_index);
        scope.set_param(SemanticKey::OffsetTotal, *offset_total);
        scope.set_param(SemanticKey::OffsetMm, *offset_mm);
        scope.set_param(SemanticKey::IsCenterline, *is_centerline);
        scope.bind_to_toolpath(toolpath, ann.move_index, end);
        scope.finish();
    }
}
