//! Main adaptive clearing path generation: orchestrator + utilities.
//!
//! Consumes MaterialGrid, compute_engagement, search_direction_with_metrics,
//! and find_entry_point from the sibling submodules and produces a sequence
//! of `AdaptiveSegment` items that `segments_to_toolpath` converts into a
//! final Toolpath with rapids, plunges, feeds, and runtime annotations.

use super::material_grid::polygon_bbox;
use super::search::{
    find_entry_point, find_entry_via_distance_transform, path_bounds, search_direction_gradient,
    search_direction_with_metrics,
};
use super::{
    AdaptiveParams, AdaptiveRuntimeAnnotation, AdaptiveRuntimeEvent, CleanupStrategy, MaterialGrid,
    average_angles, blend_corners_to_moves, target_engagement_fraction,
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
#[derive(Clone)]
pub(crate) enum AdaptiveSegment {
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
        cleanup_strategy: crate::adaptive::CleanupStrategy::Legacy,
        engagement_measure: crate::adaptive::EngagementMeasure::DiskArea,
        path_strategy: crate::adaptive::PathStrategy2d::Agent,
        trochoid_cap_mult: 1.2,
    };
    adaptive_segments_with_debug(polygon, &params, cancel, None, None)
}

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// Generate 2D adaptive segments and optionally record detailed debug spans.
pub(crate) fn adaptive_segments_with_debug(
    polygon: &Polygon2,
    params: &AdaptiveParams,
    cancel: &dyn CancelCheck,
    debug: Option<&ToolpathDebugContext>,
    // Stage 4 — optional sink for the contour-spiral's per-point predicted
    // leading-arc engagement (α/2π), collected 1:1 with the emitted Cut
    // path. `None` for every caller except the adaptive3d slice assembly,
    // which feeds it into the planner-engagement sampler.
    engagement_sink: Option<&mut Vec<(P2, f64)>>,
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

    // ── Narrow-region gate ───────────────────────────────────────────
    // For thin ring-shaped pockets (donut-topology with a hole hugging
    // the bounds, narrow strips), the engagement-target spiral has no
    // room to swing its ~21-candidate angle search and degenerates
    // into a sawtooth wiggle. When the largest inscribed disk inside
    // the machinable region is ≤ 2 × stepover (i.e. the cutter
    // diameter + a stepover doesn't fit on either side), skip the
    // spiral entirely and emit concentric contour-parallel offset
    // loops. See doc on `CleanupStrategy::ContourParallelNarrow`.
    if matches!(
        params.cleanup_strategy,
        CleanupStrategy::ContourParallelNarrow | CleanupStrategy::ContourParallelHybrid
    ) && is_narrow_machinable(machinable, tool_radius, stepover)
    {
        return contour_parallel_segments(
            machinable,
            &mut grid,
            &machinable_mask,
            tool_radius,
            stepover,
            cell_size,
            cancel,
            None,
        );
    }

    let target_frac = target_engagement_fraction(stepover, tool_radius);
    let step_len = cell_size * 3.0;
    let mut segments = Vec::new();
    let mut last_pos: Option<P2> = None;
    let mut pass_endpoints = super::search::EndpointGrid::new(tool_radius * 3.0);

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

    // ── Helical starter pocket ────────────────────────────────────────
    // Pre-clear a 2 × tool_radius disc at the medial-axis maximum so
    // the engagement-target spiral has full swing room from move 1
    // and skips the bootstrap convergence wiggle. Gated on a non-
    // Legacy cleanup strategy — adaptive3d and other downstream
    // consumers that rely on the original boundary-entry spiral
    // sweep (for full-coverage guarantees on concave shapes) keep
    // their current behaviour via the Legacy path. If no DT-max
    // cell qualifies (≥ 2 × tool_radius clearance), proceed without
    // a starter pocket — the spiral will bootstrap as before.
    let helical_entry_pos: Option<P2> =
        if matches!(params.cleanup_strategy, CleanupStrategy::Legacy) {
            None
        } else if let Some((helix_segments, helix_end)) = emit_helical_starter_pocket(
            &mut grid,
            &machinable_mask,
            &boundary_distances,
            tool_radius,
        ) {
            segments.extend(helix_segments);
            last_pos = Some(helix_end);
            Some(helix_end)
        } else {
            None
        };

    // ── Contour-spiral passes (Stage 1, constructive) ─────────────────
    // Replaces only the agent loop below; the narrow gate, starter
    // pocket, residue cleanup and emission stay shared. Requires the
    // starter pocket (the first wrap rides flush with its cleared rim);
    // when it isn't available — or the spiral degenerates — fall through
    // to the agent loop.
    if matches!(
        params.path_strategy,
        crate::adaptive::PathStrategy2d::ContourSpiral
    ) && let Some(starter_end) = helical_entry_pos
    {
        let spiral_scope = debug.map(|ctx| ctx.start_span("contour_spiral", "Contour spiral"));
        let applied = super::spiral::spiral_passes(
            &mut grid,
            &machinable_mask,
            tool_radius,
            stepover,
            starter_end,
            params.trochoid_cap_mult,
            &mut segments,
            &mut last_pos,
            engagement_sink,
            cancel,
        )?;
        if let Some(scope) = spiral_scope.as_ref() {
            scope.set_counter("applied", if applied { 1.0 } else { 0.0 });
        }
        if applied {
            return Ok(segments);
        }
    }

    // ── Adaptive passes ───────────────────────────────────────────────
    let max_passes = 500; // safety limit
    let mut pass_count = 0;

    while grid.material_fraction() > 0.01 && pass_count < max_passes {
        check_cancel(cancel)?;
        pass_count += 1;

        // For non-Legacy strategies with a successful helical entry:
        // only the helical-entry-driven pass 1 produces useful spiral
        // arcs. Subsequent passes would enter at the machinable
        // boundary and run engagement-target search through whatever
        // remains — which, by construction, is now narrow strips
        // where the search degenerates into wiggle. The cleanup phase
        // (boundary cleanup + contour-parallel sweep + cell-walking
        // mop) handles those strips with clean boundary walks
        // instead.
        //
        // When helical entry was skipped (region too small to fit a
        // 2 × tool_radius starter pocket — common in adaptive3d's
        // per-region calls), the spiral runs Legacy-style across
        // multiple passes: we don't have the helical bootstrap to
        // make a single pass cover the region, so capping it would
        // leave material uncleared.
        if pass_count > 1
            && helical_entry_pos.is_some()
            && !matches!(params.cleanup_strategy, CleanupStrategy::Legacy)
        {
            break;
        }

        let pass_started = Instant::now();
        let material_before = grid.material_fraction();
        let pass_scope =
            debug.map(|ctx| ctx.start_span("adaptive_pass", format!("Pass {pass_count}")));
        if let Some(scope) = pass_scope.as_ref() {
            scope.set_z_level(cut_depth);
            scope.set_counter("material_fraction_before", material_before);
        }
        let pass_ctx = pass_scope.as_ref().map(|scope| scope.context());

        // Find entry point.
        //
        // For pass 1, if a helical starter pocket was emitted upstream
        // the spiral picks up from the helix end directly — that
        // already puts the cutter inside a 2 × tool_radius cleared
        // disc, so the engagement-target search has its full swing
        // band on move 1 and skips the bootstrap wiggle. Falls
        // through to the boundary walk otherwise.
        let entry_scope = pass_ctx
            .as_ref()
            .map(|ctx| ctx.start_span("entry_search", format!("Entry {pass_count}")));
        let entry = if pass_count == 1 {
            if let Some(p) = helical_entry_pos {
                Some(p)
            } else {
                find_entry_point(
                    &grid,
                    &machinable_mask,
                    machinable,
                    tool_radius,
                    last_pos,
                    &pass_endpoints,
                )
            }
        } else {
            find_entry_point(
                &grid,
                &machinable_mask,
                machinable,
                tool_radius,
                last_pos,
                &pass_endpoints,
            )
        };
        let Some(entry) = entry else {
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
        // CONVERGENCE DETECTOR — disabled, kept here for context.
        //
        // Two variants tried: engagement-based (exit when engagement <
        // 0.4–0.7× target for N steps) and angle-oscillation (exit
        // when |Δangle| > π/2…2.5 for N steps). Both failed because
        // the visible sawtooth-at-spiral-end is *small-amplitude
        // continuous noise* (~60° per-step changes that integrate to
        // a wiggle), not a sharp signal. Engagement stays near target
        // through the wiggle; per-step angle changes don't exceed
        // what a tight wrap legitimately needs. Set the threshold
        // strict enough to catch the wiggle → fragments healthy
        // spirals; set it loose enough to spare healthy spirals →
        // wiggle survives.
        //
        // Right fix is structural: path-smoothing post-process OR
        // algorithm-swap to offset-loops when boundary distance gets
        // tight. Filed as I2-followup.
        for _step_idx in 0..max_steps {
            check_cancel(cancel)?;
            let before = grid.material_count;

            // Smoothed direction: average recent angles for prev_angle hint
            let smoothed_angle = if angle_buf.len() >= 2 {
                average_angles(&angle_buf)
            } else {
                prev_angle
            };

            // Search for next direction. When the cutter is in a
            // region too thin for the engagement-target search to
            // swing (dt_here < tool_radius + stepover), switch to
            // gradient-following: pick the direction perpendicular
            // to ∇boundary_distance, riding the strip's centerline.
            // Single clean pass instead of wiggle.
            //
            // Gated additionally on helical entry having succeeded —
            // without the bootstrap, the cutter starts at the
            // boundary with dt_here ≈ tool_radius < threshold, and
            // we'd switch to gradient mode from step 1 with no
            // momentum. Better to let engagement-target run.
            let dt_here = grid.boundary_distance_at(&boundary_distances, cx, cy);
            let use_gradient = helical_entry_pos.is_some()
                && !matches!(params.cleanup_strategy, CleanupStrategy::Legacy)
                && dt_here < tool_radius + stepover;
            let search_result_opt = if use_gradient {
                search_direction_gradient(
                    &grid,
                    &machinable_mask,
                    &boundary_distances,
                    cx,
                    cy,
                    step_len,
                    smoothed_angle,
                )
            } else {
                search_direction_with_metrics(
                    &grid,
                    &machinable_mask,
                    cx,
                    cy,
                    tool_radius,
                    step_len,
                    target_frac,
                    smoothed_angle,
                    &boundary_distances,
                    params.engagement_measure,
                )
            };
            let Some(search_result) = search_result_opt else {
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
            pass_endpoints.insert(endpoint);
            segments.push(AdaptiveSegment::Cut(path));
        } else {
            last_pos = Some(entry);
            pass_endpoints.insert(entry);
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
    //
    // ContourParallelHybrid mode SKIPS this pass — its post-process
    // contour-parallel sweep walks the same boundaries and continues
    // inward at stepover intervals, which subsumes this single-loop
    // sweep and exposes residue-region differences cleanly.
    let mut contours: Vec<&Vec<P2>> = Vec::new();
    if !matches!(
        params.cleanup_strategy,
        CleanupStrategy::ContourParallelHybrid
    ) {
        if machinable.exterior.len() >= 3 {
            contours.push(&machinable.exterior);
        }
        for hole in &machinable.holes {
            if hole.len() >= 3 {
                contours.push(hole);
            }
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

// ── ResidueMop cleanup strategy ────────────────────────────────────────
//
// Post-processes a segment stream produced by `adaptive_segments_with_debug`:
//
//   1. Drop short Cut groups (< MIN_KEEP_STEPS), along with their preceding
//      Rapid/Link approach. The main spiral, any subsequent long adaptive
//      sweeps, and the boundary_cleanup pass survive verbatim.
//   2. Lookahead-filter orphan Rapid/Link runs left by step 1 (a Marker
//      can sit between consecutive transitions, defeating naive
//      consecutive-collapse).
//   3. Replay the kept segments on a MaterialGrid to determine remaining
//      residue.
//   4. Walk the residue with `mop_residue_into_segments`, emitting one
//      Cut per residue patch with Rapid/Link transitions between.
//
// Cf. `CleanupStrategy::ResidueMop` doc on `AdaptiveParams`.
const MIN_KEEP_STEPS: usize = 40;

/// Threshold for keeping an otherwise-short Cut: if at least this
/// fraction of its sampled path points lies over material that would
/// not be cleared by the long Cuts alone, the short Cut is kept. The
/// principle is that *overlap is cheap, travel is expensive* — dropping
/// a short Cut and re-cleaning the same material via a mop patch
/// costs an extra Rapid (retract + travel + plunge), which is far
/// more cycle-time than the small overlap of keeping the short Cut.
const SHORT_CUT_UNIQUE_FRACTION: f64 = 0.10;

/// Apply the smart short-Cut drop rule on `segments`: returns a
/// filtered list where Cuts with `len() >= MIN_KEEP_STEPS` are always
/// kept, and shorter Cuts are kept only if they clear material that
/// the long Cuts wouldn't (i.e. `unique_fraction >= SHORT_CUT_UNIQUE_FRACTION`).
/// Preceding Rapid/Link approaches are dropped along with the Cuts
/// they introduce (consistent with the original blunt filter).
#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
fn filter_short_cuts_by_redundancy(
    segments: &[AdaptiveSegment],
    polygon: &Polygon2,
    params: &AdaptiveParams,
    cell_size: f64,
) -> Vec<AdaptiveSegment> {
    let tool_radius = params.tool_radius;

    // Build a grid populated with the cells cleared by the LONG Cuts only.
    let mut long_grid = MaterialGrid::from_polygon(polygon, cell_size);
    if let Some(stock) = &params.initial_stock {
        long_grid.apply_initial_stock(stock, params.cut_depth);
    }
    for seg in segments {
        if let AdaptiveSegment::Cut(path) = seg
            && path.len() >= MIN_KEEP_STEPS
        {
            for p in path {
                long_grid.clear_circle(p.x, p.y, tool_radius);
            }
        }
    }

    // Walk short Cuts in order, accumulating their clearing into the
    // grid so subsequent short Cuts see each other's contributions.
    let mut keep_index: Vec<bool> = Vec::with_capacity(segments.len());
    for seg in segments {
        if let AdaptiveSegment::Cut(path) = seg {
            if path.len() >= MIN_KEEP_STEPS {
                keep_index.push(true);
                continue;
            }
            if path.is_empty() {
                keep_index.push(false);
                continue;
            }
            // Sample-fraction of points over still-material cells.
            let unique = path
                .iter()
                .filter(|p| long_grid.is_material(p.x, p.y))
                .count();
            let frac = unique as f64 / path.len() as f64;
            if frac >= SHORT_CUT_UNIQUE_FRACTION {
                keep_index.push(true);
                for p in path {
                    long_grid.clear_circle(p.x, p.y, tool_radius);
                }
            } else {
                keep_index.push(false);
            }
        } else {
            keep_index.push(true); // R/L/Marker handled below by orphan trim
        }
    }

    // Walk segments, dropping non-kept Cuts and their preceding
    // contiguous Rapid/Link runs.
    let mut filtered: Vec<AdaptiveSegment> = Vec::with_capacity(segments.len());
    for (i, seg) in segments.iter().enumerate() {
        match seg {
            AdaptiveSegment::Cut(_) => {
                if keep_index[i] {
                    filtered.push(seg.clone());
                } else {
                    while let Some(last) = filtered.last() {
                        if matches!(last, AdaptiveSegment::Rapid(_) | AdaptiveSegment::Link(_)) {
                            filtered.pop();
                        } else {
                            break;
                        }
                    }
                }
            }
            AdaptiveSegment::Rapid(_) | AdaptiveSegment::Link(_) => filtered.push(seg.clone()),
            AdaptiveSegment::Marker(_) => filtered.push(seg.clone()),
        }
    }
    while let Some(last) = filtered.last() {
        if matches!(last, AdaptiveSegment::Rapid(_) | AdaptiveSegment::Link(_)) {
            filtered.pop();
        } else {
            break;
        }
    }
    filtered
}

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
pub(crate) fn apply_residue_mop_cleanup(
    polygon: &Polygon2,
    params: &AdaptiveParams,
    segments: &[AdaptiveSegment],
) -> Vec<AdaptiveSegment> {
    let tool_radius = params.tool_radius;
    let cell_size = (tool_radius / 6.0).max(params.tolerance);
    let step_len = cell_size * 3.0;

    // Step 1 — smart short-Cut drop. Short Cuts are kept when they
    // clear material the long Cuts wouldn't (overlap is cheap; travel
    // is expensive — see `SHORT_CUT_UNIQUE_FRACTION`).
    let filtered = filter_short_cuts_by_redundancy(segments, polygon, params, cell_size);

    // Step 2 — lookahead-filter orphan R/L (Markers can sit between).
    let mut out: Vec<AdaptiveSegment> = Vec::with_capacity(filtered.len());
    for (i, seg) in filtered.iter().enumerate() {
        if matches!(seg, AdaptiveSegment::Rapid(_) | AdaptiveSegment::Link(_)) {
            let mut has_cut_next = false;
            for next in &filtered[i + 1..] {
                match next {
                    AdaptiveSegment::Cut(_) => {
                        has_cut_next = true;
                        break;
                    }
                    AdaptiveSegment::Rapid(_) | AdaptiveSegment::Link(_) => break,
                    AdaptiveSegment::Marker(_) => continue,
                }
            }
            if !has_cut_next {
                continue;
            }
        }
        out.push(seg.clone());
    }

    // Step 3 — replay grid state and last cutter position.
    let mut grid = MaterialGrid::from_polygon(polygon, cell_size);
    if let Some(stock) = &params.initial_stock {
        grid.apply_initial_stock(stock, params.cut_depth);
    }
    let mut last_pos: Option<P2> = None;
    for seg in &out {
        if let AdaptiveSegment::Cut(path) = seg {
            for p in path {
                grid.clear_circle(p.x, p.y, tool_radius);
                last_pos = Some(*p);
            }
        }
    }

    // Step 4 — mop remaining residue.
    let machinable_vec = crate::polygon::offset_polygon(polygon, tool_radius);
    if machinable_vec.is_empty() {
        return out;
    }
    let machinable = &machinable_vec[0];
    let machinable_mask = MaterialGrid::build_machinable_mask(
        machinable,
        grid.origin_x,
        grid.origin_y,
        grid.rows,
        grid.cols,
        grid.cell_size,
    );
    let mop_segments =
        mop_residue_into_segments(&mut grid, &machinable_mask, tool_radius, step_len, last_pos);
    out.extend(mop_segments);
    out
}

// ── Hybrid cleanup strategy ────────────────────────────────────────────
//
// Same filter/replay as `apply_residue_mop_cleanup`, but the residue
// phase walks `machinable` inward at stepover offsets (via
// `contour_parallel_segments`, which skips contours that don't pass
// through material). After the contour-parallel sweep, any tiny patches
// the contour walks didn't reach get cleaned up by the cell-walking
// `mop_residue_into_segments` fallback.
//
// On shapes with a wide core and narrow extensions (tadpole, key), the
// spiral handles the core and the contour-parallel sweep handles the
// extensions with clean concentric loops. See `CleanupStrategy::ContourParallelHybrid`.
pub(crate) fn apply_contour_parallel_residue_cleanup(
    polygon: &Polygon2,
    params: &AdaptiveParams,
    segments: &[AdaptiveSegment],
) -> Vec<AdaptiveSegment> {
    let tool_radius = params.tool_radius;
    let cell_size = (tool_radius / 6.0).max(params.tolerance);
    let step_len = cell_size * 3.0;
    let stepover = params.stepover;

    // Step 1 — smart short-Cut drop. Short Cuts are kept when they
    // clear material the long Cuts wouldn't (overlap is cheap; travel
    // is expensive — see `SHORT_CUT_UNIQUE_FRACTION`).
    let filtered = filter_short_cuts_by_redundancy(segments, polygon, params, cell_size);

    // Step 2 — lookahead-filter orphan R/L (Markers can sit between).
    let mut out: Vec<AdaptiveSegment> = Vec::with_capacity(filtered.len());
    for (i, seg) in filtered.iter().enumerate() {
        if matches!(seg, AdaptiveSegment::Rapid(_) | AdaptiveSegment::Link(_)) {
            let mut has_cut_next = false;
            #[allow(clippy::indexing_slicing)] // i < filtered.len()
            for next in &filtered[i + 1..] {
                match next {
                    AdaptiveSegment::Cut(_) => {
                        has_cut_next = true;
                        break;
                    }
                    AdaptiveSegment::Rapid(_) | AdaptiveSegment::Link(_) => break,
                    AdaptiveSegment::Marker(_) => continue,
                }
            }
            if !has_cut_next {
                continue;
            }
        }
        out.push(seg.clone());
    }

    // Step 3 — replay grid state and last cutter position.
    let mut grid = MaterialGrid::from_polygon(polygon, cell_size);
    if let Some(stock) = &params.initial_stock {
        grid.apply_initial_stock(stock, params.cut_depth);
    }
    let mut last_pos: Option<P2> = None;
    for seg in &out {
        if let AdaptiveSegment::Cut(path) = seg {
            for p in path {
                grid.clear_circle(p.x, p.y, tool_radius);
                last_pos = Some(*p);
            }
        }
    }

    // Step 4 — contour-parallel sweep of residue (filtered by material
    // presence; sweeps only the offsets that actually cross residue).
    let machinable_vec = crate::polygon::offset_polygon(polygon, tool_radius);
    if machinable_vec.is_empty() {
        return out;
    }
    #[allow(clippy::indexing_slicing)] // machinable_vec non-empty checked above
    let machinable = &machinable_vec[0];
    let machinable_mask = MaterialGrid::build_machinable_mask(
        machinable,
        grid.origin_x,
        grid.origin_y,
        grid.rows,
        grid.cols,
        grid.cell_size,
    );
    let never_cancel: &dyn CancelCheck = &|| false;
    if let Ok(contour_segments) = contour_parallel_segments(
        machinable,
        &mut grid,
        &machinable_mask,
        tool_radius,
        stepover,
        cell_size,
        never_cancel,
        last_pos,
    ) {
        if let Some(last_cut) = contour_segments.iter().rev().find_map(|s| match s {
            AdaptiveSegment::Cut(p) => p.last().copied(),
            _ => None,
        }) {
            last_pos = Some(last_cut);
        }
        out.extend(contour_segments);
    }

    // Step 5 — tiny-patch fallback. Anything the contour walks missed
    // (sub-stepover slivers, far-off-axis residue) gets cleaned by the
    // cell-walking mop.
    let mop_segments =
        mop_residue_into_segments(&mut grid, &machinable_mask, tool_radius, step_len, last_pos);
    out.extend(mop_segments);
    out
}

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// Walks the MaterialGrid directly, emitting one Cut per reachable
/// residue patch. See doc on `apply_residue_mop_cleanup`.
pub(crate) fn mop_residue_into_segments(
    grid: &mut MaterialGrid,
    machinable_mask: &[bool],
    tool_radius: f64,
    step_len: f64,
    start_pos: Option<P2>,
) -> Vec<AdaptiveSegment> {
    const MAX_PATCHES: usize = 400;
    const MAX_STEPS_PER_PATCH: usize = 600;
    // Lowered from 0.005 → 0.001 so the mop chases thin islands left
    // between the spiral's outermost reach and the boundary band.
    const RESIDUE_DONE_FRACTION: f64 = 0.001;
    let max_link_dist = tool_radius * 6.0;

    let mut segments: Vec<AdaptiveSegment> = Vec::new();
    let mut last_pos = start_pos;

    for _ in 0..MAX_PATCHES {
        if grid.material_fraction() < RESIDUE_DONE_FRACTION {
            break;
        }
        let search_from = last_pos.unwrap_or_else(|| P2::new(0.0, 0.0));
        let Some((mx, my)) = grid.find_nearest_material(search_from.x, search_from.y) else {
            break;
        };
        if !grid.is_machinable(machinable_mask, mx, my) {
            grid.clear_circle(mx, my, tool_radius);
            continue;
        }

        let start = P2::new(mx, my);
        let approach = match last_pos {
            None => Some(AdaptiveSegment::Rapid(start)),
            Some(prev) => {
                let dx = start.x - prev.x;
                let dy = start.y - prev.y;
                let dist = (dx * dx + dy * dy).sqrt();
                if dist < 1e-6 {
                    None
                } else if is_clear_path(grid, machinable_mask, prev, start, tool_radius) {
                    // Path is over already-cleared cells — tool-down
                    // traverse is safe regardless of distance. Saves
                    // the retract + plunge cycle of a Rapid.
                    Some(AdaptiveSegment::Link(start))
                } else if dist > max_link_dist {
                    Some(AdaptiveSegment::Rapid(start))
                } else {
                    Some(AdaptiveSegment::Link(start))
                }
            }
        };
        if let Some(seg) = approach {
            segments.push(seg);
        }

        let mut path: Vec<P2> = vec![start];
        let mut cur = start;
        grid.clear_circle(cur.x, cur.y, tool_radius);

        for _ in 0..MAX_STEPS_PER_PATCH {
            let Some((mx, my)) = grid.find_nearest_material(cur.x, cur.y) else {
                break;
            };
            let dx = mx - cur.x;
            let dy = my - cur.y;
            let dist = (dx * dx + dy * dy).sqrt();
            // Chain across short cleared gaps: stay tool-down and
            // walk to the next material as part of the same Cut,
            // rather than ending this Cut and starting a new one
            // with its own approach. Each Rapid costs a retract +
            // plunge cycle, which dominates over the cheap overlap
            // of walking through cleared cells.
            if dist > tool_radius * 6.0 {
                break;
            }
            // Refuse the chain hop if it would cross outside the
            // machinable region (e.g. across a thin neck where
            // straight-line travel would clip a wall).
            if dist > tool_radius * 2.0
                && !is_clear_path(grid, machinable_mask, cur, P2::new(mx, my), tool_radius)
            {
                break;
            }
            if dist < 1e-9 {
                grid.clear_circle(cur.x, cur.y, tool_radius);
                break;
            }
            let step = step_len.min(dist).max(1e-9);
            let nx = cur.x + step * dx / dist;
            let ny = cur.y + step * dy / dist;
            if !grid.is_machinable(machinable_mask, nx, ny) {
                let nx2 = cur.x + (step * 0.5) * dx / dist;
                let ny2 = cur.y + (step * 0.5) * dy / dist;
                if grid.is_machinable(machinable_mask, nx2, ny2) {
                    cur = P2::new(nx2, ny2);
                } else {
                    grid.clear_circle(mx, my, tool_radius);
                    break;
                }
            } else {
                cur = P2::new(nx, ny);
            }
            path.push(cur);
            grid.clear_circle(cur.x, cur.y, tool_radius);
        }

        if path.len() >= 2 {
            segments.push(AdaptiveSegment::Cut(path));
            last_pos = Some(cur);
        } else {
            segments.pop();
        }
    }
    segments
}

// ── Narrow-region contour-parallel strategy ────────────────────────────
//
// When the largest inscribed disk inside the machinable mask is small
// relative to the stepover (≤ 3 × stepover by default), the engagement-
// target spiral has no room to settle and produces a per-step sawtooth.
// For those regions, the planner emits concentric inward offsets of the
// machinable polygon and walks each contour as one continuous Cut. The
// final residue mop still runs on the output to catch any leftover
// strip between concentric loops. See `CleanupStrategy::ContourParallelNarrow`.

/// True when the machinable region is too narrow for the engagement-
/// target spiral to settle: the largest inscribed disk fits within
/// 2 × stepover. Implemented by checking that an inward offset of
/// 2 × stepover collapses the machinable region to empty.
///
/// We use a Euclidean offset (via `offset_polygon`) rather than the
/// Manhattan-grid distance transform on `boundary_distances` because
/// the Manhattan metric over-estimates depth at corners (e.g. a donut
/// ring corner reads ~12 mm Manhattan but ~8 mm Euclidean), which
/// matters for the gate threshold.
/// Emit a circular starter pocket centred on the largest inscribed
/// disk inside `machinable`, returning the cutter end position so the
/// engagement-target spiral can continue from there with full swing
/// room from move 1.
///
/// The cutter walks a circle of radius `tool_radius` around the
/// medial-axis maximum, clearing a disc of radius `2 × tool_radius`.
/// In 3D production this would be a helical plunge; for the 2D
/// planner it's a single circular pass after a Rapid + Z-plunge.
///
/// Returns `None` when no DT-maximum cell with ≥ `2 × tool_radius`
/// clearance exists (uniformly-narrow regions — the narrow gate
/// would already have handled those).
///
/// Reference: Autodesk patent US7831332 (Fusion HSM transition
/// portion), Bieterman & Sandström (Boeing, ~2003), Ren & Bi (2014).
#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
fn emit_helical_starter_pocket(
    grid: &mut MaterialGrid,
    machinable_mask: &[bool],
    boundary_distances: &[f64],
    tool_radius: f64,
) -> Option<(Vec<AdaptiveSegment>, P2)> {
    let medial =
        find_entry_via_distance_transform(grid, machinable_mask, boundary_distances, tool_radius)?;
    // Need the medial-axis disk to fit the helix (radius `tool_radius`)
    // plus the cutter (radius `tool_radius`) plus a small safety margin.
    let dt_at = grid.boundary_distance_at(boundary_distances, medial.x, medial.y);
    let required = 2.0 * tool_radius;
    if dt_at < required {
        return None;
    }

    let helix_r = tool_radius;
    let n_steps = 64;
    let mut segments: Vec<AdaptiveSegment> = Vec::new();

    // Clear the medial cell itself (the Z-plunge centre).
    grid.clear_circle(medial.x, medial.y, tool_radius);

    let start = P2::new(medial.x + helix_r, medial.y);
    segments.push(AdaptiveSegment::Rapid(start));
    grid.clear_circle(start.x, start.y, tool_radius);

    let mut path: Vec<P2> = vec![start];
    for i in 1..=n_steps {
        let theta = (i as f64 / n_steps as f64) * std::f64::consts::TAU;
        let x = medial.x + helix_r * theta.cos();
        let y = medial.y + helix_r * theta.sin();
        grid.clear_circle(x, y, tool_radius);
        path.push(P2::new(x, y));
    }
    let end = *path.last()?;
    segments.push(AdaptiveSegment::Cut(path));
    Some((segments, end))
}

fn is_narrow_machinable(machinable: &Polygon2, tool_radius: f64, stepover: f64) -> bool {
    // Probe radius: tool_radius + stepover. If the machinable region
    // inset by this much collapses to nothing or only tiny fragments,
    // the engagement-target spiral has no room to settle. A "tiny
    // fragment" is one with area < 2 × (cutter footprint). Donut-
    // topology rings break into corner residues well below this
    // threshold; convex pockets (square / rect / circle / L) produce
    // a single large fragment that exceeds it.
    let probe = tool_radius + stepover;
    let result = offset_polygon(machinable, probe);
    if result.is_empty() {
        return true;
    }
    let cutter_area = std::f64::consts::PI * tool_radius * tool_radius;
    let min_viable_area = 2.0 * cutter_area;
    let max_fragment_area = result.iter().map(|p| p.area()).fold(0.0_f64, f64::max);
    max_fragment_area < min_viable_area
}

/// Emit concentric inward-offset loops of `machinable` at stride `stepover`,
/// clearing the grid along each contour. Contours that pass through no
/// remaining material are skipped (no emission) — so a fresh-grid call
/// emits every loop, but a post-spiral call emits only the loops that
/// would actually remove residue. Adjacent emitted loops are connected
/// by Link when within 6R via a clear path, else by Rapid.
#[allow(clippy::too_many_arguments)]
fn contour_parallel_segments(
    machinable: &Polygon2,
    grid: &mut MaterialGrid,
    machinable_mask: &[bool],
    tool_radius: f64,
    stepover: f64,
    cell_size: f64,
    cancel: &dyn CancelCheck,
    start_pos: Option<P2>,
) -> Result<Vec<AdaptiveSegment>, Cancelled> {
    const MAX_LOOPS: usize = 120;
    const RESIDUE_DONE_FRACTION: f64 = 0.001;
    // Overlap factor < 1.0 makes consecutive offset loops overlap so
    // thin rings between the spiral's outermost reach and the boundary
    // band don't survive as uncleared islands. 0.85 → 15% overlap.
    const OFFSET_OVERLAP: f64 = 0.85;
    let max_link_dist = tool_radius * 6.0;
    let mut segments: Vec<AdaptiveSegment> = Vec::new();
    let mut last_pos: Option<P2> = start_pos;

    for k in 0..MAX_LOOPS {
        check_cancel(cancel)?;
        if grid.material_fraction() < RESIDUE_DONE_FRACTION {
            break;
        }
        let dist = stepover * OFFSET_OVERLAP * (k as f64);
        let polys: Vec<Polygon2> = if dist <= 1e-9 {
            vec![machinable.clone()]
        } else {
            offset_polygon(machinable, dist)
        };
        if polys.is_empty() {
            break;
        }

        let material_before = grid.material_count;
        for poly in &polys {
            let mut contours: Vec<&Vec<P2>> = Vec::new();
            if poly.exterior.len() >= 3 {
                contours.push(&poly.exterior);
            }
            for hole in &poly.holes {
                if hole.len() >= 3 {
                    contours.push(hole);
                }
            }

            for contour in contours {
                if !contour_passes_material(contour, grid) {
                    continue;
                }
                // Begin walking each closed loop at the vertex nearest the
                // cutter's previous exit, so the inter-loop hop is as short
                // as possible. Same cells cleared (identical closed path,
                // different start vertex) — this only shrinks the link/rapid
                // travel between concentric offset loops.
                let walk_owned: Vec<P2>;
                let walk: &[P2] = match last_pos {
                    Some(prev) => {
                        walk_owned = rotate_contour_to_nearest(contour, prev);
                        &walk_owned
                    }
                    None => contour,
                };
                let path = walk_contour_clearing(walk, cell_size, grid, tool_radius);
                if path.len() < 2 {
                    continue;
                }
                #[allow(clippy::indexing_slicing)] // path.len() >= 2 checked above
                let entry = path[0];
                #[allow(clippy::expect_used)]
                let end = *path.last().expect("path non-empty");

                match last_pos {
                    None => segments.push(AdaptiveSegment::Rapid(entry)),
                    Some(prev) => {
                        let dx = entry.x - prev.x;
                        let dy = entry.y - prev.y;
                        let d = (dx * dx + dy * dy).sqrt();
                        if d < 1e-6 {
                            // already at entry — no approach needed
                        } else if is_clear_path(grid, machinable_mask, prev, entry, tool_radius) {
                            // Cleared-cell traverse — Link at any
                            // distance, saving the retract + plunge
                            // cycle of a Rapid.
                            segments.push(AdaptiveSegment::Link(entry));
                        } else if d < max_link_dist {
                            segments.push(AdaptiveSegment::Link(entry));
                        } else {
                            segments.push(AdaptiveSegment::Rapid(entry));
                        }
                    }
                }
                segments.push(AdaptiveSegment::Cut(path));
                last_pos = Some(end);
            }
        }

        // If a full pass at this offset couldn't reduce material, stop
        // (further offsets would just spin).
        if grid.material_count >= material_before {
            break;
        }
    }

    Ok(segments)
}

/// True when a meaningful fraction of the contour lies over uncleared
/// (material) cells. Samples ~128 points; emits if ≥ 25% are material.
///
/// A simple "any material" filter over-emits: the spiral leaves thin
/// wiggle-slivers between passes, so every inward offset contour
/// crosses *some* material at a sliver and walks the whole loop just
/// to clear a 5%-coverage band. The fraction threshold ignores these
/// tracer-sliver intersections and emits only when the contour covers
/// substantial residue — typically the outermost band where the
/// spiral didn't reach, or a residue ring in a narrow strip.
fn contour_passes_material(contour: &[P2], grid: &MaterialGrid) -> bool {
    const COVERAGE_THRESHOLD: f64 = 0.25;
    if contour.len() < 3 {
        return false;
    }
    let n_samples = 128.min(contour.len());
    let stride = (contour.len() / n_samples).max(1);
    let mut hits = 0usize;
    let mut total = 0usize;
    let mut i = 0;
    while i < contour.len() {
        #[allow(clippy::indexing_slicing)] // i < contour.len() bounded above
        let p = contour[i];
        if grid.is_material(p.x, p.y) {
            hits += 1;
        }
        total += 1;
        i += stride;
    }
    total > 0 && (hits as f64 / total as f64) >= COVERAGE_THRESHOLD
}

/// Rotate a closed contour's vertex order so the vertex nearest to
/// `target` becomes index 0. The loop covers the same cells regardless of
/// where it starts, so this is a pure travel optimisation: walking begins
/// at the point closest to the cutter's previous position, minimising the
/// link/rapid hop between consecutive concentric offset loops.
///
/// A trailing duplicate-of-first closing vertex (some offset outputs carry
/// one) is dropped first so it can't land mid-loop as a zero-length edge.
fn rotate_contour_to_nearest(contour: &[P2], target: P2) -> Vec<P2> {
    let mut verts = contour;
    if verts.len() >= 2 {
        #[allow(clippy::indexing_slicing)] // len >= 2 checked
        let (first, last) = (verts[0], verts[verts.len() - 1]);
        let dx = first.x - last.x;
        let dy = first.y - last.y;
        if dx * dx + dy * dy < 1e-18 {
            #[allow(clippy::indexing_slicing)] // len >= 2 checked
            let trimmed = &contour[..contour.len() - 1];
            verts = trimmed;
        }
    }
    if verts.len() < 2 {
        return verts.to_vec();
    }
    let mut best_idx = 0usize;
    let mut best_d2 = f64::INFINITY;
    for (i, p) in verts.iter().enumerate() {
        let dx = p.x - target.x;
        let dy = p.y - target.y;
        let d2 = dx * dx + dy * dy;
        if d2 < best_d2 {
            best_d2 = d2;
            best_idx = i;
        }
    }
    #[allow(clippy::indexing_slicing)] // best_idx in 0..verts.len()
    let mut rotated = verts[best_idx..].to_vec();
    #[allow(clippy::indexing_slicing)] // best_idx in 0..verts.len()
    rotated.extend_from_slice(&verts[..best_idx]);
    rotated
}

/// Walk a closed contour in world coords, subdividing each edge to
/// `cell_size * 1.5` and clearing the grid at every step. Returns the
/// emitted Cut path (start point repeated at the end to close the loop).
fn walk_contour_clearing(
    contour: &[P2],
    cell_size: f64,
    grid: &mut MaterialGrid,
    tool_radius: f64,
) -> Vec<P2> {
    let mut path = Vec::new();
    if contour.len() < 2 {
        return path;
    }
    #[allow(clippy::indexing_slicing)] // contour.len() >= 2 checked above
    let start = contour[0];
    path.push(start);
    grid.clear_circle(start.x, start.y, tool_radius);

    let n = contour.len();
    for i in 0..n {
        #[allow(clippy::indexing_slicing)] // i, (i+1)%n bounded by contour len
        let a = contour[i];
        #[allow(clippy::indexing_slicing)]
        let b = contour[(i + 1) % n];
        let dx = b.x - a.x;
        let dy = b.y - a.y;
        let len = (dx * dx + dy * dy).sqrt();
        let n_steps = (len / (cell_size * 1.5)).ceil() as usize;
        let steps = n_steps.max(1);
        for j in 1..=steps {
            let t = j as f64 / steps as f64;
            let x = a.x + t * dx;
            let y = a.y + t * dy;
            grid.clear_circle(x, y, tool_radius);
            path.push(P2::new(x, y));
        }
    }
    path
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
                tp.rapid_to_with_intent(
                    crate::geo::P3::new(entry.x, entry.y, params.safe_z),
                    crate::toolpath::MoveIntent::Linking,
                );
                tp.feed_to_with_intent(
                    crate::geo::P3::new(entry.x, entry.y, params.cut_depth),
                    params.plunge_rate,
                    crate::toolpath::MoveIntent::EntryPlunge,
                );
            }
            AdaptiveSegment::Link(entry) => {
                tp.feed_to_with_intent(
                    crate::geo::P3::new(entry.x, entry.y, params.cut_depth),
                    params.feed_rate,
                    crate::toolpath::MoveIntent::Linking,
                );
            }
            AdaptiveSegment::Cut(path) => {
                let simplified = simplify_path(path, params.tolerance);
                if params.min_cutting_radius > 0.0 {
                    let moves = blend_corners_to_moves(&simplified, params.min_cutting_radius);
                    for m in moves.iter().skip(1) {
                        match m {
                            BlendedMove::Linear(p) => {
                                tp.feed_to_with_intent(
                                    crate::geo::P3::new(p.x, p.y, params.cut_depth),
                                    params.feed_rate,
                                    crate::toolpath::MoveIntent::ClearingCut,
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
                                    tp.arc_cw_to_with_intent(
                                        target,
                                        i,
                                        j,
                                        params.feed_rate,
                                        crate::toolpath::MoveIntent::ClearingCut,
                                    );
                                } else {
                                    tp.arc_ccw_to_with_intent(
                                        target,
                                        i,
                                        j,
                                        params.feed_rate,
                                        crate::toolpath::MoveIntent::ClearingCut,
                                    );
                                }
                            }
                        }
                    }
                } else {
                    for p in simplified.iter().skip(1) {
                        tp.feed_to_with_intent(
                            crate::geo::P3::new(p.x, p.y, params.cut_depth),
                            params.feed_rate,
                            crate::toolpath::MoveIntent::ClearingCut,
                        );
                    }
                }
            }
        }
    }

    if let Some(last) = tp.moves.last() {
        tp.rapid_to_with_intent(
            crate::geo::P3::new(last.target.x, last.target.y, params.safe_z),
            crate::toolpath::MoveIntent::Retract,
        );
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
