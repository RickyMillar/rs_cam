//! Main adaptive clearing path generation: orchestrator + utilities.
//!
//! Consumes MaterialGrid, compute_engagement, search_direction_with_metrics,
//! and find_entry_point from the sibling submodules and produces a sequence
//! of `AdaptiveSegment` items that `segments_to_toolpath` converts into a
//! final Toolpath with rapids, plunges, feeds, and runtime annotations.

use super::material_grid::polygon_bbox;
use super::search::{find_entry_point, path_bounds, search_direction_with_metrics};
use super::{
    AdaptiveParams, AdaptiveRuntimeAnnotation, AdaptiveRuntimeEvent, MaterialGrid, average_angles,
    blend_corners_to_moves, target_engagement_fraction,
};
use crate::adaptive_shared::BlendedMove;
use crate::debug_trace::{HotspotRecord, ToolpathDebugBounds2, ToolpathDebugContext};
use crate::geo::P2;
use crate::interrupt::{CancelCheck, Cancelled, check_cancel};
use crate::polygon::{Polygon2, offset_polygon};
use crate::toolpath::Toolpath;

use std::time::Instant;

// ── Link vs retract ────────────────────────────────────────────────────

/// Check if the straight line from `from` to `to` is safe to traverse at
/// cut depth. The entire path must be within the machinable region, and
/// at most 20% of the path may cross uncut material (thin strips are OK —
/// the tool handles light engagement during a link move).
pub(super) fn is_clear_path(
    grid: &MaterialGrid,
    mask: &[bool],
    from: P2,
    to: P2,
    _tool_radius: f64,
) -> bool {
    let dx = to.x - from.x;
    let dy = to.y - from.y;
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1e-10 {
        return true;
    }

    let n_steps = (len / (grid.cell_size * 2.0)).ceil() as usize;
    let mut material_hits = 0;
    let mut total = 0;

    for i in 0..=n_steps {
        let t = i as f64 / n_steps.max(1) as f64;
        let x = from.x + t * dx;
        let y = from.y + t * dy;
        total += 1;

        // Hard fail: outside machinable region
        if !grid.is_machinable(mask, x, y) {
            return false;
        }
        if grid.is_material(x, y) {
            material_hits += 1;
        }
    }

    // Safe if less than 20% of the path crosses material
    total > 0 && (material_hits as f64 / total as f64) <= 0.2
}

// ── Main adaptive path generation ──────────────────────────────────────

/// A segment of the adaptive path: cutting, rapid reposition, or link (tool-down reposition).
pub(super) enum AdaptiveSegment {
    /// Cutting moves: a sequence of 2D points.
    Cut(Vec<P2>),
    /// Rapid reposition to a new entry point (retract → rapid → plunge).
    Rapid(P2),
    /// Link move: reposition at cut depth without retracting (cleared path).
    Link(P2),
    /// Structured runtime marker at the current point in the toolpath.
    Marker(AdaptiveRuntimeEvent),
}

/// Generate the 2D adaptive clearing path segments.
#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn adaptive_segments(
    polygon: &Polygon2,
    tool_radius: f64,
    stepover: f64,
    tolerance: f64,
    slot_clearing: bool,
    cancel: &dyn CancelCheck,
) -> Result<Vec<AdaptiveSegment>, Cancelled> {
    let params = AdaptiveParams {
        tool_radius,
        stepover,
        tolerance,
        slot_clearing,
        cut_depth: 0.0,
        feed_rate: 0.0,
        plunge_rate: 0.0,
        safe_z: 0.0,
        min_cutting_radius: 0.0,
        initial_stock: None,
    };
    adaptive_segments_with_debug(polygon, &params, cancel, None)
}

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// Generate 2D adaptive segments and optionally record detailed debug spans.
pub(super) fn adaptive_segments_with_debug(
    polygon: &Polygon2,
    params: &AdaptiveParams,
    cancel: &dyn CancelCheck,
    debug: Option<&ToolpathDebugContext>,
) -> Result<Vec<AdaptiveSegment>, Cancelled> {
    let tool_radius = params.tool_radius;
    let stepover = params.stepover;
    let tolerance = params.tolerance;
    let slot_clearing = params.slot_clearing;
    let cut_depth = params.cut_depth;
    // Inset polygon by tool radius to get the machinable region
    let machinable_vec = offset_polygon(polygon, tool_radius);
    if machinable_vec.is_empty() {
        return Ok(Vec::new());
    }
    let machinable = &machinable_vec[0];

    // Build material grid from the original polygon (not inset)
    let cell_size = (tool_radius / 6.0).max(tolerance);
    let mut grid = MaterialGrid::from_polygon(polygon, cell_size);

    // If prior stock state is available, mark cells already cleared by
    // earlier operations so the adaptive algorithm does not re-cut them.
    if let Some(ref stock) = params.initial_stock {
        grid.apply_initial_stock(stock, cut_depth);
    }

    // Cache the machinable region as a boolean mask for fast lookups
    let machinable_mask = MaterialGrid::build_machinable_mask(
        machinable,
        grid.origin_x,
        grid.origin_y,
        grid.rows,
        grid.cols,
        grid.cell_size,
    );

    // Precompute boundary distance field for wall-tangent bias
    let boundary_distances = grid.compute_boundary_distances();

    let target_frac = target_engagement_fraction(stepover, tool_radius);
    let step_len = cell_size * 3.0;
    let mut segments = Vec::new();
    let mut last_pos: Option<P2> = None;
    let mut pass_endpoints: Vec<P2> = Vec::new();

    // ── Slot clearing (Fusion-style first pass) ───────────────────────
    // Generate sparse zigzag lines at wide spacing to open pockets across
    // all regions of the polygon. Uses tool_diameter spacing so each line
    // creates a slot the adaptive spiral can expand from.
    if slot_clearing {
        let slot_scope = debug.map(|ctx| ctx.start_span("slot_clearing", "Slot clearing"));
        let (x_min, y_min, x_max, y_max) = polygon_bbox(&polygon.exterior);
        let w = x_max - x_min;
        let h = y_max - y_min;
        // Slot along the longest axis
        let slot_angle = if w >= h { 0.0 } else { 90.0 };
        // Target ~3 seeding lines across the pocket's narrow axis.
        // This opens pockets in all regions without doing the adaptive's job.
        let narrow_span = if w >= h { h } else { w };
        let slot_spacing = (narrow_span / 3.0).max(tool_radius * 4.0);
        let slot_lines =
            crate::zigzag::zigzag_lines(polygon, tool_radius, slot_spacing, slot_angle);

        for (line_idx, line) in slot_lines.iter().enumerate() {
            check_cancel(cancel)?;
            segments.push(AdaptiveSegment::Marker(
                AdaptiveRuntimeEvent::SlotClearing {
                    line_index: line_idx + 1,
                    line_total: slot_lines.len(),
                },
            ));
            segments.push(AdaptiveSegment::Rapid(line[0]));

            // Walk along the line and clear material in the grid
            let dx = line[1].x - line[0].x;
            let dy = line[1].y - line[0].y;
            let len = (dx * dx + dy * dy).sqrt();
            let n_steps = (len / (cell_size * 1.5)).ceil() as usize;
            for j in 0..=n_steps {
                let t = j as f64 / n_steps.max(1) as f64;
                let x = line[0].x + t * dx;
                let y = line[0].y + t * dy;
                grid.clear_circle(x, y, tool_radius);
            }

            segments.push(AdaptiveSegment::Cut(vec![line[0], line[1]]));
            last_pos = Some(line[1]);
        }
        if let Some(scope) = slot_scope.as_ref() {
            scope.set_counter("line_count", slot_lines.len() as f64);
        }
    }

    // ── Adaptive passes ───────────────────────────────────────────────
    let max_passes = 500; // safety limit
    let mut pass_count = 0;

    while grid.material_fraction() > 0.01 && pass_count < max_passes {
        check_cancel(cancel)?;
        pass_count += 1;

        let pass_started = Instant::now();
        let material_before = grid.material_fraction();
        let pass_scope =
            debug.map(|ctx| ctx.start_span("adaptive_pass", format!("Pass {pass_count}")));
        if let Some(scope) = pass_scope.as_ref() {
            scope.set_z_level(cut_depth);
            scope.set_counter("material_fraction_before", material_before);
        }
        let pass_ctx = pass_scope.as_ref().map(|scope| scope.context());

        // Find entry point (spread away from previous endpoints)
        let entry_scope = pass_ctx
            .as_ref()
            .map(|ctx| ctx.start_span("entry_search", format!("Entry {pass_count}")));
        let Some(entry) = find_entry_point(
            &grid,
            &machinable_mask,
            machinable,
            tool_radius,
            last_pos,
            &pass_endpoints,
        ) else {
            if let Some(scope) = pass_scope.as_ref() {
                scope.set_exit_reason("no entry");
                scope.set_counter("pass_index", pass_count as f64);
            }
            break;
        };
        if let Some(scope) = entry_scope.as_ref() {
            scope.set_xy_bbox(ToolpathDebugBounds2 {
                min_x: entry.x,
                max_x: entry.x,
                min_y: entry.y,
                max_y: entry.y,
            });
        }

        // Link or retract to entry point
        let max_link_dist = tool_radius * 6.0; // ~3 tool diameters
        segments.push(AdaptiveSegment::Marker(AdaptiveRuntimeEvent::PassEntry {
            pass_index: pass_count,
            entry_x: entry.x,
            entry_y: entry.y,
        }));
        if let Some(last) = last_pos {
            let dx = entry.x - last.x;
            let dy = entry.y - last.y;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist < max_link_dist
                && is_clear_path(&grid, &machinable_mask, last, entry, tool_radius)
            {
                segments.push(AdaptiveSegment::Link(entry));
            } else {
                segments.push(AdaptiveSegment::Rapid(entry));
            }
        } else {
            segments.push(AdaptiveSegment::Rapid(entry));
        }

        // Walk the adaptive path from this entry
        let mut path = vec![entry];
        let mut cx = entry.x;
        let mut cy = entry.y;

        // Initial direction: toward nearest material
        let mut prev_angle = if let Some(pos) = last_pos {
            (entry.y - pos.y).atan2(entry.x - pos.x)
        } else if let Some((mx, my)) = grid.find_nearest_material(cx, cy) {
            (my - cy).atan2(mx - cx)
        } else {
            0.0
        };

        // Clear material at entry position
        grid.clear_circle(cx, cy, tool_radius);

        // Direction smoothing buffer (gyro) — average last N directions
        // for smooth curves instead of jagged steps. Inspired by Freesteel.
        const SMOOTH_BUF_LEN: usize = 3;
        let mut angle_buf: Vec<f64> = Vec::with_capacity(SMOOTH_BUF_LEN);

        let max_steps = 5000;
        let mut idle_count = 0;
        let mut search_evaluations = 0u32;
        for _ in 0..max_steps {
            check_cancel(cancel)?;
            let before = grid.material_count;

            // Smoothed direction: average recent angles for prev_angle hint
            let smoothed_angle = if angle_buf.len() >= 2 {
                average_angles(&angle_buf)
            } else {
                prev_angle
            };

            // Search for next direction
            let Some(search_result) = search_direction_with_metrics(
                &grid,
                &machinable_mask,
                cx,
                cy,
                tool_radius,
                step_len,
                target_frac,
                smoothed_angle,
                &boundary_distances,
            ) else {
                break;
            };
            search_evaluations += search_result.evaluations;
            let angle = search_result.angle;

            // Move in that direction
            cx += step_len * angle.cos();
            cy += step_len * angle.sin();
            path.push(P2::new(cx, cy));

            // Clear material at new position
            grid.clear_circle(cx, cy, tool_radius);

            // Update direction smoothing buffer
            if angle_buf.len() >= SMOOTH_BUF_LEN {
                angle_buf.remove(0);
            }
            angle_buf.push(angle);

            // Idle detection: if no material was cleared for many steps, we're
            // going in circles over already-cleared area.
            if grid.material_count == before {
                idle_count += 1;
                if idle_count > 15 {
                    break;
                }
            } else {
                idle_count = 0;
            }

            prev_angle = angle;
        }

        let was_idle = idle_count > 15;
        let exit_reason = if was_idle { "idle" } else { "no direction" };

        let path_len = path.len();
        let path_debug_bounds = path_bounds(&path);

        if path_len >= 2 {
            // SAFETY: path.len() >= 2 checked on line above
            #[allow(clippy::expect_used)]
            let endpoint = *path.last().expect("path is non-empty after loop");
            last_pos = Some(endpoint);
            pass_endpoints.push(endpoint);
            segments.push(AdaptiveSegment::Cut(path));
        } else {
            last_pos = Some(entry);
            pass_endpoints.push(entry);
        }

        // If the pass ended due to idle detection, the remaining material
        // nearby is too small or inaccessible. Force-clear a wider area
        // around the last position to prevent revisiting the same spot.
        if was_idle {
            let forced_clear_scope = pass_ctx
                .as_ref()
                .map(|ctx| ctx.start_span("forced_clear", format!("Forced clear {pass_count}")));
            grid.clear_circle(cx, cy, tool_radius * 2.0);
            segments.push(AdaptiveSegment::Marker(AdaptiveRuntimeEvent::ForcedClear {
                pass_index: pass_count,
                center_x: cx,
                center_y: cy,
                radius: tool_radius * 2.0,
            }));
            if let Some(scope) = forced_clear_scope.as_ref() {
                scope.set_xy_bbox(ToolpathDebugBounds2 {
                    min_x: cx - tool_radius * 2.0,
                    max_x: cx + tool_radius * 2.0,
                    min_y: cy - tool_radius * 2.0,
                    max_y: cy + tool_radius * 2.0,
                });
                scope.set_z_level(cut_depth);
            }
        }

        if let Some(scope) = pass_scope.as_ref() {
            scope.set_counter("pass_index", pass_count as f64);
            scope.set_counter("step_count", path_len as f64);
            scope.set_counter("idle_count", idle_count as f64);
            scope.set_counter("search_evaluations", search_evaluations as f64);
            scope.set_counter("material_fraction_after", grid.material_fraction());
            scope.set_exit_reason(exit_reason);
            if let Some(bounds) = path_debug_bounds {
                scope.set_xy_bbox(bounds);
                let (center_x, center_y) = bounds.center();
                if let Some(ctx) = pass_ctx.as_ref() {
                    ctx.record_hotspot(&HotspotRecord {
                        kind: "adaptive_pass".into(),
                        center_x,
                        center_y,
                        z_level: Some(cut_depth),
                        bucket_size_xy: tool_radius * 2.0,
                        bucket_size_z: Some(tolerance.max(step_len)),
                        elapsed_us: pass_started.elapsed().as_micros() as u64,
                        pass_count: 1,
                        step_count: path_len as u64,
                        low_yield_exit_count: 0,
                    });
                }
            }
            scope.set_z_level(cut_depth);
        }
        segments.push(AdaptiveSegment::Marker(AdaptiveRuntimeEvent::PassSummary {
            pass_index: pass_count,
            step_count: path_len,
            idle_count,
            search_evaluations: search_evaluations as usize,
            exit_reason: exit_reason.to_owned(),
        }));
    }

    // ── Boundary cleanup pass ─────────────────────────────────────────
    // Trace ALL machinable boundaries (exterior + hole contours) to sweep
    // any thin strip of material left along the walls. This is the
    // tool-center contour that puts the tool edge right on each wall.
    let mut contours: Vec<&Vec<P2>> = Vec::new();
    if machinable.exterior.len() >= 3 {
        contours.push(&machinable.exterior);
    }
    for hole in &machinable.holes {
        if hole.len() >= 3 {
            contours.push(hole);
        }
    }

    let cleanup_scope = debug.map(|ctx| ctx.start_span("boundary_cleanup", "Boundary cleanup"));
    for (contour_idx, boundary) in contours.iter().enumerate() {
        check_cancel(cancel)?;
        segments.push(AdaptiveSegment::Marker(
            AdaptiveRuntimeEvent::BoundaryCleanup {
                contour_index: contour_idx + 1,
                contour_total: contours.len(),
            },
        ));
        segments.push(AdaptiveSegment::Rapid(boundary[0]));

        let mut cleanup_path = vec![boundary[0]];
        // Walk the contour, clearing material and interpolating between
        // vertices so no cells are missed on long edges.
        for i in 0..boundary.len() {
            let a = boundary[i];
            let b = boundary[(i + 1) % boundary.len()];
            let dx = b.x - a.x;
            let dy = b.y - a.y;
            let len = (dx * dx + dy * dy).sqrt();
            let n_steps = (len / (cell_size * 1.5)).ceil() as usize;
            for j in 1..=n_steps {
                let t = j as f64 / n_steps.max(1) as f64;
                let x = a.x + t * dx;
                let y = a.y + t * dy;
                grid.clear_circle(x, y, tool_radius);
                cleanup_path.push(P2::new(x, y));
            }
        }
        // Close the loop back to the start
        grid.clear_circle(boundary[0].x, boundary[0].y, tool_radius);
        cleanup_path.push(boundary[0]);
        segments.push(AdaptiveSegment::Cut(cleanup_path));
    }
    if let Some(scope) = cleanup_scope.as_ref() {
        scope.set_counter("contour_count", contours.len() as f64);
        scope.set_z_level(cut_depth);
    }

    Ok(segments)
}

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// Simplify a path using the Douglas-Peucker algorithm.
pub(crate) fn simplify_path(points: &[P2], tolerance: f64) -> Vec<P2> {
    if points.len() <= 2 {
        return points.to_vec();
    }

    // Find the point farthest from the line between first and last
    let first = points[0];
    let last = points[points.len() - 1];
    let dx = last.x - first.x;
    let dy = last.y - first.y;
    let line_len = (dx * dx + dy * dy).sqrt();

    let mut max_dist = 0.0;
    let mut max_idx = 0;

    if line_len > 1e-10 {
        for (i, pt) in points.iter().enumerate().take(points.len() - 1).skip(1) {
            let d = ((pt.x - first.x) * dy - (pt.y - first.y) * dx).abs() / line_len;
            if d > max_dist {
                max_dist = d;
                max_idx = i;
            }
        }
    } else {
        // Degenerate case: all points are close together
        for (i, pt) in points.iter().enumerate().take(points.len() - 1).skip(1) {
            let ddx = pt.x - first.x;
            let ddy = pt.y - first.y;
            let d = (ddx * ddx + ddy * ddy).sqrt();
            if d > max_dist {
                max_dist = d;
                max_idx = i;
            }
        }
    }

    if max_dist > tolerance {
        let mut left = simplify_path(&points[..=max_idx], tolerance);
        let right = simplify_path(&points[max_idx..], tolerance);
        left.pop(); // Remove duplicate junction point
        left.extend(right);
        left
    } else {
        vec![first, last]
    }
}

pub(super) fn segments_to_toolpath(
    segments: &[AdaptiveSegment],
    params: &AdaptiveParams,
) -> (Toolpath, Vec<AdaptiveRuntimeAnnotation>) {
    let mut tp = Toolpath::new();
    let mut annotations = Vec::new();

    for segment in segments {
        match segment {
            AdaptiveSegment::Marker(event) => {
                annotations.push(AdaptiveRuntimeAnnotation {
                    move_index: tp.moves.len(),
                    event: event.clone(),
                });
            }
            AdaptiveSegment::Rapid(entry) => {
                tp.rapid_to(crate::geo::P3::new(entry.x, entry.y, params.safe_z));
                tp.feed_to(
                    crate::geo::P3::new(entry.x, entry.y, params.cut_depth),
                    params.plunge_rate,
                );
            }
            AdaptiveSegment::Link(entry) => {
                tp.feed_to(
                    crate::geo::P3::new(entry.x, entry.y, params.cut_depth),
                    params.feed_rate,
                );
            }
            AdaptiveSegment::Cut(path) => {
                let simplified = simplify_path(path, params.tolerance);
                if params.min_cutting_radius > 0.0 {
                    let moves = blend_corners_to_moves(&simplified, params.min_cutting_radius);
                    for m in moves.iter().skip(1) {
                        match m {
                            BlendedMove::Linear(p) => {
                                tp.feed_to(
                                    crate::geo::P3::new(p.x, p.y, params.cut_depth),
                                    params.feed_rate,
                                );
                            }
                            BlendedMove::Arc {
                                end,
                                center,
                                clockwise,
                            } => {
                                // SAFETY: Cut always follows Plunge/Link, so tp.moves
                                // is non-empty; unwrap_or is a defensive fallback.
                                let prev =
                                    tp.moves.last().map(|mv| mv.target).unwrap_or(
                                        crate::geo::P3::new(end.x, end.y, params.cut_depth),
                                    );
                                let i = center.x - prev.x;
                                let j = center.y - prev.y;
                                let target = crate::geo::P3::new(end.x, end.y, params.cut_depth);
                                if *clockwise {
                                    tp.arc_cw_to(target, i, j, params.feed_rate);
                                } else {
                                    tp.arc_ccw_to(target, i, j, params.feed_rate);
                                }
                            }
                        }
                    }
                } else {
                    for p in simplified.iter().skip(1) {
                        tp.feed_to(
                            crate::geo::P3::new(p.x, p.y, params.cut_depth),
                            params.feed_rate,
                        );
                    }
                }
            }
        }
    }

    if let Some(last) = tp.moves.last() {
        tp.rapid_to(crate::geo::P3::new(
            last.target.x,
            last.target.y,
            params.safe_z,
        ));
    }

    (tp, annotations)
}

pub(super) fn runtime_annotations_to_labels(
    annotations: &[AdaptiveRuntimeAnnotation],
) -> Vec<(usize, String)> {
    annotations
        .iter()
        .map(|annotation| (annotation.move_index, annotation.event.label()))
        .collect()
}
