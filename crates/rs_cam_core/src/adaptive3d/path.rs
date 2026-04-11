//! 3D adaptive orchestration: main loop over Z levels + segment →
//! toolpath conversion for adaptive3d.

use crate::adaptive_shared::target_engagement_fraction;
use crate::debug_trace::ToolpathDebugContext;
use crate::dexel::ray_subtract_above;
use crate::dexel_stock::TriDexelStock;
use crate::geo::P3;
use crate::interrupt::{CancelCheck, Cancelled, check_cancel};
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::radial_profile::RadialProfileLUT;
use crate::slope::SurfaceHeightmap;
use crate::tool::MillingCutter;
use crate::toolpath::{Toolpath, simplify_path_3d};
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;
use tracing::{debug, info};

use super::clearing::{
    ClearZLevelContext, clear_z_level, clear_z_level_adaptive, clear_z_level_contour_parallel,
    detect_material_regions, waterline_cleanup,
};
use super::search::blend_corners_3d;
use super::{
    Adaptive3dParams, Adaptive3dRuntimeAnnotation, Adaptive3dRuntimeEvent, ClearingStrategy3d,
    EntryStyle3d, RegionOrdering,
};

pub(super) enum Adaptive3dSegment {
    /// 3D cutting path with variable Z
    Cut(Vec<P3>),
    /// Retract to safe_z, rapid XY, plunge to entry
    Rapid(P3),
    /// Feed directly at cutting depth (no retract)
    Link(P3),
    /// Structured runtime marker at the current point in the toolpath
    Marker(Adaptive3dRuntimeEvent),
}

// ── Main loop ─────────────────────────────────────────────────────────

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
pub(super) fn adaptive_3d_segments(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &Adaptive3dParams,
    debug_ctx: Option<&ToolpathDebugContext>,
    cancel: &dyn CancelCheck,
) -> Result<Vec<Adaptive3dSegment>, Cancelled> {
    let tool_radius = params.tool_radius;
    let r = cutter.radius();

    // Grid geometry: expand mesh bbox by cutter radius
    let bbox = &mesh.bbox;
    let origin_x = bbox.min.x - r;
    let origin_y = bbox.min.y - r;
    let extent_x = bbox.max.x + r;
    let extent_y = bbox.max.y + r;
    let cell_size = (tool_radius / 6.0).max(params.tolerance);

    // Initialize tri-dexel material stock
    let mut material_stock = match &params.initial_stock {
        Some(stock) => stock.clone(),
        None => TriDexelStock::from_stock(
            origin_x,
            origin_y,
            extent_x,
            extent_y,
            bbox.min.z,
            params.stock_top_z,
            cell_size,
        ),
    };

    // Precompute surface heightmap (rayon parallel drop-cutter)
    #[cfg(not(target_arch = "wasm32"))]
    let t_surface = Instant::now();
    let surface_scope =
        debug_ctx.map(|ctx| ctx.start_span("surface_heightmap", "Surface heightmap"));
    debug!(
        cols = material_stock.z_grid.cols,
        rows = material_stock.z_grid.rows,
        "Precomputing surface heightmap"
    );
    let surface_hm = SurfaceHeightmap::from_mesh_with_cancel(
        mesh,
        index,
        cutter,
        material_stock.z_grid.origin_u,
        material_stock.z_grid.origin_v,
        material_stock.z_grid.rows,
        material_stock.z_grid.cols,
        material_stock.z_grid.cell_size,
        bbox.min.z,
        cancel,
    )?;
    #[cfg(not(target_arch = "wasm32"))]
    info!(
        elapsed_ms = t_surface.elapsed().as_millis() as u64,
        "Surface heightmap complete"
    );
    #[cfg(target_arch = "wasm32")]
    info!("Surface heightmap complete");
    if let Some(scope) = surface_scope.as_ref() {
        scope.set_counter("rows", material_stock.z_grid.rows as f64);
        scope.set_counter("cols", material_stock.z_grid.cols as f64);
    }

    // Compute slope map for slope-aware pre-stamping and selective waterline cleanup.
    let slope_map = surface_hm.slope_map();

    // Clear material at cells outside the mesh XY footprint.
    // Drop-cutter returns min_z for cells beyond the mesh edge, creating phantom
    // "deep material" that the tool can never reach. Mark these as already cleared
    // so the adaptive doesn't waste passes trying to cut in empty space.
    // Only clear cells whose XY center is outside the mesh bbox (with tolerance).
    let border_scope = debug_ctx.map(|ctx| ctx.start_span("border_clear", "Border clear"));
    let border_margin = r * 0.5;
    let mut border_cleared = 0u32;
    for row in 0..material_stock.z_grid.rows {
        if row % 16 == 0 {
            check_cancel(cancel)?;
        }
        for col in 0..material_stock.z_grid.cols {
            let (x, y) = material_stock.z_grid.cell_to_world(row, col);
            if x < bbox.min.x - border_margin
                || x > bbox.max.x + border_margin
                || y < bbox.min.y - border_margin
                || y > bbox.max.y + border_margin
            {
                // Clear material above the surface Z at this cell
                let i = row * material_stock.z_grid.cols + col;
                let clear_z = surface_hm.z_values[i] as f32;
                ray_subtract_above(material_stock.z_grid.ray_mut(row, col), clear_z);
                border_cleared += 1;
            }
        }
    }
    if border_cleared > 0 {
        debug!(
            cells = border_cleared,
            "Cleared border cells outside mesh footprint"
        );
    }
    if let Some(scope) = border_scope.as_ref() {
        scope.set_counter("cells", border_cleared as f64);
    }

    // Compute Z levels: stock_top down to surface bottom + stock_to_leave
    let z_plan_scope = debug_ctx.map(|ctx| ctx.start_span("z_level_plan", "Compute Z levels"));
    let surface_bottom = surface_hm.min_z();
    let z_bottom = surface_bottom + params.stock_to_leave;
    let mut z_levels = Vec::new();
    let mut z = params.stock_top_z - params.depth_per_pass;
    while z > z_bottom {
        z_levels.push(z);
        z -= params.depth_per_pass;
    }
    z_levels.push(z_bottom); // Always include final level at the surface

    // Fix 5: Flat area detection — histogram surface Z, insert levels at shelves
    if params.detect_flat_areas {
        let total_cells = surface_hm.z_values.len();
        if total_cells > 0 {
            // Build histogram of surface Z values binned at tolerance resolution
            let bin_size = params.tolerance.max(0.05);
            let z_min_surf = surface_bottom;
            let z_max_surf = params.stock_top_z;
            let n_bins = ((z_max_surf - z_min_surf) / bin_size).ceil() as usize + 1;
            let mut histogram = vec![0u32; n_bins];
            for &sz in &surface_hm.z_values {
                let bin = ((sz - z_min_surf) / bin_size).floor() as usize;
                if bin < n_bins {
                    histogram[bin] += 1;
                }
            }
            // Bins with >2% of total cells represent flat features
            let threshold = (total_cells as f64 * 0.02) as u32;
            let mut flat_levels = Vec::new();
            for (i, &count) in histogram.iter().enumerate() {
                if count > threshold {
                    let flat_z = z_min_surf + (i as f64 + 0.5) * bin_size + params.stock_to_leave;
                    // Only insert if within the working range and not too close to existing levels
                    if flat_z > z_bottom + bin_size && flat_z < params.stock_top_z - bin_size {
                        let too_close = z_levels.iter().any(|&zl| (zl - flat_z).abs() < bin_size);
                        if !too_close {
                            flat_levels.push(flat_z);
                        }
                    }
                }
            }
            if !flat_levels.is_empty() {
                debug!(count = flat_levels.len(), "Detected flat area Z levels");
                z_levels.extend(flat_levels);
                z_levels.sort_by(|a, b| b.total_cmp(a)); // Top-down order
                z_levels.dedup_by(|a, b| (*a - *b).abs() < 0.01);
            }
        }
    }

    // Fix 4: Fine stepdown — insert intermediate Z levels between major levels
    if let Some(fine_step) = params.fine_stepdown
        && fine_step > 0.0
        && fine_step < params.depth_per_pass
    {
        let major_levels = z_levels.clone();
        let mut all_levels = Vec::new();
        // Insert intermediates between stock_top and first level
        let first_start = params.stock_top_z;
        for window in std::iter::once(&first_start)
            .chain(major_levels.iter())
            .collect::<Vec<_>>()
            .windows(2)
        {
            let z_top = *window[0];
            let z_bot = *window[1];
            let mut iz = z_top - fine_step;
            while iz > z_bot + fine_step * 0.5 {
                all_levels.push(iz);
                iz -= fine_step;
            }
            all_levels.push(z_bot); // Always include the major level
        }
        all_levels.sort_by(|a, b| b.total_cmp(a));
        all_levels.dedup_by(|a, b| (*a - *b).abs() < 0.01);
        debug!(
            from = z_levels.len(),
            to = all_levels.len(),
            fine_step = fine_step,
            "Fine stepdown expanded Z levels"
        );
        z_levels = all_levels;
    }

    info!(
        count = z_levels.len(),
        z_top = z_levels.first().copied().unwrap_or(0.0),
        z_bottom = z_levels.last().copied().unwrap_or(0.0),
        depth_per_pass = params.depth_per_pass,
        "Z levels computed"
    );
    if let Some(scope) = z_plan_scope.as_ref() {
        scope.set_counter("count", z_levels.len() as f64);
        scope.set_counter("z_top", z_levels.first().copied().unwrap_or(0.0));
        scope.set_counter("z_bottom", z_levels.last().copied().unwrap_or(0.0));
    }

    let target_frac = target_engagement_fraction(params.stepover, tool_radius);
    let step_len = cell_size * 1.5;
    let max_link_dist = params.max_stay_down_dist.unwrap_or(tool_radius * 6.0);

    let bbox_x_min = origin_x + tool_radius;
    let bbox_x_max = extent_x - tool_radius;
    let bbox_y_min = origin_y + tool_radius;
    let bbox_y_max = extent_y - tool_radius;

    let lut = RadialProfileLUT::from_cutter(cutter, 256);
    let ctx = ClearZLevelContext {
        mesh,
        index,
        cutter,
        lut: &lut,
        slope_map: &slope_map,
        debug: debug_ctx.cloned(),
        tool_radius,
        stepover: params.stepover,
        stock_to_leave: params.stock_to_leave,
        depth_per_pass: params.depth_per_pass,
        tolerance: params.tolerance,
        target_frac,
        step_len,
        max_link_dist,
        bbox_x_min,
        bbox_x_max,
        bbox_y_min,
        bbox_y_max,
        clearing_strategy: params.clearing_strategy,
        z_blend: params.z_blend,
    };

    let mut segments = Vec::new();
    let mut last_pos: Option<P3> = None;

    match params.region_ordering {
        RegionOrdering::ByArea => {
            let region_scope =
                debug_ctx.map(|ctx| ctx.start_span("region_detect", "Detect regions"));
            let regions = detect_material_regions(
                &material_stock,
                &surface_hm,
                params.stock_to_leave,
                tool_radius,
            );
            info!(
                regions = regions.len(),
                "Detected material regions for by-area ordering"
            );
            if let Some(scope) = region_scope.as_ref() {
                scope.set_counter("regions", regions.len() as f64);
            }

            for (region_idx, region) in regions.iter().enumerate() {
                check_cancel(cancel)?;
                debug!(
                    region = region_idx,
                    cells = region.cell_count,
                    z_min = format!("{:.1}", region.surface_z_min),
                    z_max = format!("{:.1}", region.surface_z_max),
                    "Processing region"
                );
                segments.push(Adaptive3dSegment::Marker(
                    Adaptive3dRuntimeEvent::RegionStart {
                        region_index: region_idx + 1,
                        region_total: regions.len(),
                        cell_count: region.cell_count,
                    },
                ));

                let region_z_levels: Vec<f64> = z_levels
                    .iter()
                    .copied()
                    .filter(|&z| z >= region.surface_z_min + params.stock_to_leave - 0.01)
                    .collect();

                for (li, &z_level) in region_z_levels.iter().enumerate() {
                    check_cancel(cancel)?;
                    segments.push(Adaptive3dSegment::Marker(
                        Adaptive3dRuntimeEvent::RegionZLevel {
                            region_index: region_idx + 1,
                            z_level,
                            level_index: li + 1,
                            level_total: region_z_levels.len(),
                        },
                    ));
                    match ctx.clearing_strategy {
                        ClearingStrategy3d::ContourParallel => {
                            clear_z_level_contour_parallel(
                                &ctx,
                                &mut material_stock,
                                &surface_hm,
                                z_level,
                                &mut segments,
                                &mut last_pos,
                                Some(region),
                                cancel,
                            )?;
                        }
                        ClearingStrategy3d::Adaptive => {
                            clear_z_level_adaptive(
                                &ctx,
                                &mut material_stock,
                                &surface_hm,
                                z_level,
                                &mut segments,
                                &mut last_pos,
                                Some(region),
                                cancel,
                            )?;
                        }
                        ClearingStrategy3d::AgentSearch => {
                            clear_z_level(
                                &ctx,
                                &mut material_stock,
                                &surface_hm,
                                z_level,
                                &mut segments,
                                &mut last_pos,
                                Some(region),
                                cancel,
                            )?;
                        }
                    }
                }
            }

            // Waterline cleanup once at bottom Z
            if let Some(&z_bottom_level) = z_levels.last() {
                segments.push(Adaptive3dSegment::Marker(
                    Adaptive3dRuntimeEvent::WaterlineCleanup,
                ));
                waterline_cleanup(
                    mesh,
                    index,
                    cutter,
                    &lut,
                    &slope_map,
                    &mut material_stock,
                    z_bottom_level,
                    tool_radius,
                    cell_size,
                    &mut segments,
                    &mut last_pos,
                    debug_ctx,
                    cancel,
                )?;
            }
        }
        RegionOrdering::Global => {
            for (level_idx, &z_level) in z_levels.iter().enumerate() {
                check_cancel(cancel)?;
                segments.push(Adaptive3dSegment::Marker(
                    Adaptive3dRuntimeEvent::GlobalZLevel {
                        z_level,
                        level_index: level_idx + 1,
                        level_total: z_levels.len(),
                    },
                ));
                match ctx.clearing_strategy {
                    ClearingStrategy3d::ContourParallel => {
                        clear_z_level_contour_parallel(
                            &ctx,
                            &mut material_stock,
                            &surface_hm,
                            z_level,
                            &mut segments,
                            &mut last_pos,
                            None,
                            cancel,
                        )?;
                    }
                    ClearingStrategy3d::Adaptive => {
                        clear_z_level_adaptive(
                            &ctx,
                            &mut material_stock,
                            &surface_hm,
                            z_level,
                            &mut segments,
                            &mut last_pos,
                            None,
                            cancel,
                        )?;
                    }
                    ClearingStrategy3d::AgentSearch => {
                        clear_z_level(
                            &ctx,
                            &mut material_stock,
                            &surface_hm,
                            z_level,
                            &mut segments,
                            &mut last_pos,
                            None,
                            cancel,
                        )?;
                    }
                }

                let is_last_level = level_idx == z_levels.len() - 1;
                if is_last_level {
                    segments.push(Adaptive3dSegment::Marker(
                        Adaptive3dRuntimeEvent::WaterlineCleanup,
                    ));
                    waterline_cleanup(
                        mesh,
                        index,
                        cutter,
                        &lut,
                        &slope_map,
                        &mut material_stock,
                        z_level,
                        tool_radius,
                        cell_size,
                        &mut segments,
                        &mut last_pos,
                        debug_ctx,
                        cancel,
                    )?;
                }
            }
        }
    }

    Ok(segments)
}

// ── Public API ────────────────────────────────────────────────────────

/// Convert segments to a toolpath and collect annotations.
pub(super) fn segments_to_toolpath(
    segments: &[Adaptive3dSegment],
    params: &Adaptive3dParams,
) -> (Toolpath, Vec<Adaptive3dRuntimeAnnotation>) {
    let mut tp = Toolpath::new();
    let mut annotations = Vec::new();

    for segment in segments {
        match segment {
            Adaptive3dSegment::Marker(event) => {
                annotations.push(Adaptive3dRuntimeAnnotation {
                    move_index: tp.moves.len(),
                    event: event.clone(),
                });
            }
            Adaptive3dSegment::Rapid(entry) => match params.entry_style {
                EntryStyle3d::Plunge => {
                    tp.rapid_to(P3::new(entry.x, entry.y, params.safe_z));
                    tp.feed_to(*entry, params.plunge_rate);
                }
                EntryStyle3d::Helix { radius, pitch } => {
                    tp.rapid_to(P3::new(entry.x, entry.y, params.safe_z));
                    let helix_start = P3::new(entry.x, entry.y, params.safe_z);
                    crate::dressup::emit_helix(
                        &mut tp,
                        &helix_start,
                        entry,
                        radius,
                        pitch,
                        params.plunge_rate,
                    );
                }
                EntryStyle3d::Ramp { max_angle_deg } => {
                    tp.rapid_to(P3::new(entry.x, entry.y, params.safe_z));
                    let ramp_start = P3::new(entry.x, entry.y, params.safe_z);
                    crate::dressup::emit_ramp(
                        &mut tp,
                        &ramp_start,
                        entry,
                        (1.0, 0.0),
                        max_angle_deg,
                        params.plunge_rate,
                    );
                }
            },
            Adaptive3dSegment::Link(target) => {
                tp.feed_to(*target, params.feed_rate);
            }
            Adaptive3dSegment::Cut(path) => {
                if path.len() < 2 {
                    continue;
                }
                let simplified = simplify_path_3d(path, params.tolerance);
                let blended = blend_corners_3d(&simplified, params.min_cutting_radius);
                for pt in blended.iter().skip(1) {
                    tp.feed_to(*pt, params.feed_rate);
                }
            }
        }
    }

    if let Some(last) = tp.moves.last() {
        tp.rapid_to(P3::new(last.target.x, last.target.y, params.safe_z));
    }

    (tp, annotations)
}

pub(super) fn runtime_annotations_to_labels(
    annotations: &[Adaptive3dRuntimeAnnotation],
) -> Vec<(usize, String)> {
    annotations
        .iter()
        .map(|annotation| (annotation.move_index, annotation.event.label()))
        .collect()
}
