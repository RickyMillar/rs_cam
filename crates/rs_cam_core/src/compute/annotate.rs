//! Convert runtime annotations from individual operation algorithms
//! into hierarchical semantic trace items.
//!
//! Each annotated operation emits a flat list of runtime events tagged with
//! the toolpath move index where they occurred.  This module walks those
//! events and opens/closes semantic scopes so that the toolpath viewer can
//! display a structured breakdown of the algorithm's behaviour.

use crate::semantic_trace::{ToolpathSemanticContext, ToolpathSemanticKind};
use crate::toolpath::Toolpath;

use super::execute::OperationAnnotations;

/// Convert operation-specific runtime annotations into hierarchical semantic items.
pub fn annotate_from_runtime_events(
    annotations: &OperationAnnotations,
    toolpath: &Toolpath,
    op_context: &ToolpathSemanticContext,
) {
    match annotations {
        OperationAnnotations::None => {}
        OperationAnnotations::Adaptive3d(events) => {
            annotate_adaptive3d(events, toolpath, op_context);
        }
        OperationAnnotations::Adaptive2d(events) => {
            annotate_adaptive2d(events, toolpath, op_context);
        }
        OperationAnnotations::Scallop(events) => {
            annotate_scallop(events, toolpath, op_context);
        }
        OperationAnnotations::RampFinish(events) => {
            annotate_ramp_finish(events, toolpath, op_context);
        }
        OperationAnnotations::SpiralFinish(events) => {
            annotate_spiral_finish(events, toolpath, op_context);
        }
        OperationAnnotations::Pencil(events) => {
            annotate_pencil(events, toolpath, op_context);
        }
    }
}

// ── Helpers ──────────────────────────────────────────────────────────

/// Compute the exclusive move-end index for annotation `i`.
///
/// Item `i` covers moves from `annotations[i].move_index` (inclusive) to
/// `annotations[i+1].move_index` (exclusive).  The last item extends to the
/// end of the toolpath.
fn move_end(move_indices: &[usize], i: usize, toolpath_len: usize) -> usize {
    move_indices.get(i + 1).copied().unwrap_or(toolpath_len)
}

// ── Adaptive 3D ─────────────────────────────────────────────────────

fn annotate_adaptive3d(
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
                scope.bind_to_toolpath(toolpath, ann.move_index, end);
                let ctx = scope.context();
                level_ctx = Some(ctx);
                level_scope = Some(scope);
            }
            Adaptive3dRuntimeEvent::GlobalZLevel {
                z_level,
                level_index,
                level_total,
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
            } => {
                let parent = level_ctx.as_ref().unwrap_or(op_context);
                let scope = parent.start_item(
                    ToolpathSemanticKind::Entry,
                    format!("Pass {} entry", pass_index + 1),
                );
                scope.set_param("pass_index", *pass_index);
                scope.set_param("entry_x", *entry_x);
                scope.set_param("entry_y", *entry_y);
                scope.set_param("entry_z", *entry_z);
                scope.bind_to_toolpath(toolpath, ann.move_index, end);
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

// ── Adaptive 2D ─────────────────────────────────────────────────────

fn annotate_adaptive2d(
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

fn annotate_scallop(
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

fn annotate_ramp_finish(
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

fn annotate_spiral_finish(
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

fn annotate_pencil(
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
