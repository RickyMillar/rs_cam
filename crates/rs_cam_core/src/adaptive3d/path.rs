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
    ClearZLevelContext, clear_z_level_adaptive, clear_z_level_agent_2d_slice,
    clear_z_level_contour_parallel, detect_material_regions, waterline_cleanup,
};
use super::search::{blend_corners_3d, material_remaining_at_level_diag};
use super::{
    Adaptive3dParams, Adaptive3dRuntimeAnnotation, Adaptive3dRuntimeEvent, ClearingStrategy3d,
    EntryStyle3d, RegionOrdering, ZLevelPlanMetrics,
};

/// Peck the descent from `start_z` down to `entry.z` at constant XY.
/// Each peck steps down by `params.depth_per_pass` and rapid-retracts a
/// small clearance for chip break before the next peck. Final feed
/// completes the descent to exactly `entry.z`. No-op if `start_z <=
/// entry.z` (already at or below the target).
///
/// Without this, entries chosen at fresh-stock XYs on deep Z levels
/// would carve a column from `start_z` down through full stock
/// thickness in one shot ("punched hole" symptom).
fn emit_peck_plunge(tp: &mut Toolpath, entry: &P3, start_z: f64, params: &Adaptive3dParams) {
    const PECK_CLEARANCE_MM: f64 = 0.5;
    let dpp = params.depth_per_pass.max(0.1);
    let mut current_z = start_z;
    while current_z - entry.z > dpp + 1e-6 {
        let next_z = current_z - dpp;
        tp.feed_to(P3::new(entry.x, entry.y, next_z), params.plunge_rate);
        let retract_z = next_z + PECK_CLEARANCE_MM;
        tp.rapid_to(P3::new(entry.x, entry.y, retract_z));
        current_z = retract_z;
    }
    tp.feed_to(*entry, params.plunge_rate);
}

pub(super) enum Adaptive3dSegment {
    /// 3D cutting path with variable Z
    Cut(Vec<P3>),
    /// Retract to safe_z, rapid XY, peck-plunge from safe_z to entry.
    Rapid(P3),
    /// Retract to safe_z, rapid XY, **rapid Z-descent through cleared
    /// air down to `rapid_floor_z`**, then peck-plunge from there to
    /// entry. Used when the clearing function knows the previous Z
    /// level already cleared above this XY — turning what would be a
    /// long peck-feed through cleared air into a fast rapid descent
    /// followed by a short peck through the remaining fresh material.
    /// Falls back to plain `Rapid` semantics when `rapid_floor_z >=
    /// safe_z` (no air gap to skip).
    RapidWithFloor { entry: P3, rapid_floor_z: f64 },
    /// Feed directly at cutting depth (no retract)
    Link(P3),
    /// Structured runtime marker at the current point in the toolpath
    Marker(Adaptive3dRuntimeEvent),
}

// ── Per-Z debug span ──────────────────────────────────────────────────

/// Per-Z tally returned by [`tally_segments_for_z_level`].
///
/// `cut_path_points` is the upper bound on planner stamp calls for the
/// level (each Cut path point becomes one `stamp_tool_at`). Pair it
/// with `rapid_segs + link_segs` — each rapid expands to at least one
/// peck-plunge feed that the simulator stamps but the planner does
/// not, so a large `rapid_segs` value next to a small
/// `material_remaining_post` is a hint the planner's accounting is
/// missing real material removal happening at entry/transit moves.
struct ZLevelSegmentTally {
    cut_segs: u64,
    rapid_segs: u64,
    link_segs: u64,
    cut_mm: f64,
    cut_path_points: u64,
}

/// Tally Cut/Rapid/Link counts and Cut path length+points for a slice
/// of segments emitted by one Z-level's clearing dispatch.
fn tally_segments_for_z_level(segments: &[Adaptive3dSegment]) -> ZLevelSegmentTally {
    let mut tally = ZLevelSegmentTally {
        cut_segs: 0,
        rapid_segs: 0,
        link_segs: 0,
        cut_mm: 0.0,
        cut_path_points: 0,
    };
    for seg in segments {
        match seg {
            Adaptive3dSegment::Cut(path) => {
                tally.cut_segs += 1;
                tally.cut_path_points += path.len() as u64;
                for pair in path.windows(2) {
                    if let [a, b] = pair {
                        let dx = b.x - a.x;
                        let dy = b.y - a.y;
                        let dz = b.z - a.z;
                        tally.cut_mm += (dx * dx + dy * dy + dz * dz).sqrt();
                    }
                }
            }
            Adaptive3dSegment::Rapid(_) | Adaptive3dSegment::RapidWithFloor { .. } => {
                tally.rapid_segs += 1;
            }
            Adaptive3dSegment::Link(_) => tally.link_segs += 1,
            Adaptive3dSegment::Marker(_) => {}
        }
    }
    tally
}

// ── Main loop ─────────────────────────────────────────────────────────

/// Output of [`adaptive_3d_segments`]. The `final_material_stock` and
/// `surface_heightmap` fields are exposed so tests can compare the
/// planner's internal dexel state against an independent simulator
/// replay of the emitted toolpath — see
/// `tests/adaptive3d_planner_sim_dexel_parity.rs`.
pub(super) struct Adaptive3dSegmentsResult {
    pub segments: Vec<Adaptive3dSegment>,
    /// Test-only: planner's internal dexel state at the end of the run.
    #[allow(dead_code)]
    pub final_material_stock: TriDexelStock,
    /// Test-only: heightmap used to surface-drape Cut paths.
    #[allow(dead_code)]
    pub surface_heightmap: SurfaceHeightmap,
}

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
pub(super) fn adaptive_3d_segments(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &Adaptive3dParams,
    debug_ctx: Option<&ToolpathDebugContext>,
    cancel: &dyn CancelCheck,
) -> Result<Adaptive3dSegmentsResult, Cancelled> {
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
        None => {
            // Stopgap validation: stock_top_z has no auto-derivation from
            // stock or mesh, and its default (30.0) is arbitrary. Warn when
            // it's clearly below the mesh — in that case the top of the
            // model won't be cut and the user probably forgot to set it.
            // See planning/adaptive_review_2026-04.md F-4.
            if params.stock_top_z < bbox.max.z - 0.5 {
                tracing::warn!(
                    stock_top_z = params.stock_top_z,
                    mesh_top_z = bbox.max.z,
                    "stock_top_z is below mesh top by {:.1}mm — that band of material will \
                     NOT be cut. Set stock_top_z to match your actual stock height.",
                    bbox.max.z - params.stock_top_z
                );
            }
            TriDexelStock::from_stock(
                origin_x,
                origin_y,
                extent_x,
                extent_y,
                bbox.min.z,
                params.stock_top_z,
                cell_size,
            )
        }
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

    // Boundary clip on the internal stock. When a 2D boundary polygon is
    // provided (e.g. the model silhouette inset by tool_radius), clear
    // cells whose center is outside the boundary. This forces the bool-grid
    // polygon at every z-level to respect the boundary, so cuts emitted by
    // AgentSearch and ContourParallel/Adaptive stay inside it. Without this,
    // toolpath generation produces cuts across the full stock, the
    // post-generation toolpath clip then converts outside-boundary cuts to
    // rapids, and the dexel for those cells is left unstamped — deeper
    // z-levels then bite through fresh stock with full-depth axial DOC.
    // See planning/AGENTSEARCH_INVESTIGATION_LOG.md O5b for repro details.
    if let Some(ref boundary) = params.boundary {
        let mut boundary_cleared = 0u32;
        for row in 0..material_stock.z_grid.rows {
            if row % 16 == 0 {
                check_cancel(cancel)?;
            }
            for col in 0..material_stock.z_grid.cols {
                let (x, y) = material_stock.z_grid.cell_to_world(row, col);
                if !boundary.contains_point(&crate::geo::P2::new(x, y)) {
                    let i = row * material_stock.z_grid.cols + col;
                    let clear_z = surface_hm.z_values[i] as f32;
                    ray_subtract_above(material_stock.z_grid.ray_mut(row, col), clear_z);
                    boundary_cleared += 1;
                }
            }
        }
        if boundary_cleared > 0 {
            debug!(
                cells = boundary_cleared,
                "Cleared cells outside boundary polygon"
            );
        }
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
    // Maximum stay-down link distance. When a pass finishes and the next
    // entry point is within this distance, the tool stays down and feeds
    // between them (subject to is_clear_path_3d); otherwise it retracts
    // to safe_z, rapids, and plunges. The previous formula was just
    // `tool_radius * 6.0` — a magic constant that under-scaled when the
    // user chose a coarse stepover, because inter-pass gaps grow with
    // stepover while the tool radius is fixed. The stepover term below
    // only takes effect when stepover exceeds tool_radius (i.e. the
    // operator is choosing an unusually coarse step); for typical
    // stepover ≤ radius the formula is unchanged. See
    // planning/adaptive_review_2026-04.md F-6.
    let max_link_dist = params
        .max_stay_down_dist
        .unwrap_or_else(|| (tool_radius * 6.0).max(params.stepover * 6.0));

    // Bbox margins use the envelope radius (full shank for tapered tools) so
    // the tool body can't overrun the working footprint. Engagement-related
    // computations below use tool_radius (effective contact radius at DOC).
    let envelope_radius = params.envelope_radius;
    let bbox_x_min = origin_x + envelope_radius;
    let bbox_x_max = extent_x - envelope_radius;
    let bbox_y_min = origin_y + envelope_radius;
    let bbox_y_max = extent_y - envelope_radius;

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
        safe_z: params.safe_z,
        min_cutting_radius: params.min_cutting_radius,
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
            if regions.len() == 1 {
                info!(
                    "region_ordering=ByArea detected a single material region — \
                     pass ordering matches Global. If you expected multiple \
                     regions, check mesh for connected islands or adjust \
                     min_cells detection threshold."
                );
            }
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
                    let level_event = Adaptive3dRuntimeEvent::RegionZLevel {
                        region_index: region_idx + 1,
                        z_level,
                        level_index: li + 1,
                        level_total: region_z_levels.len(),
                        metrics: ZLevelPlanMetrics::default(),
                    };
                    let level_scope = debug_ctx.map(|dctx| {
                        let scope = dctx.start_span(
                            "z_level_clear",
                            format!(
                                "Region {} Z {:.3} ({}/{})",
                                region_idx + 1,
                                z_level,
                                li + 1,
                                region_z_levels.len()
                            ),
                        );
                        scope.set_z_level(z_level);
                        scope.set_counter("region_index", (region_idx + 1) as f64);
                        let diag = material_remaining_at_level_diag(
                            &material_stock,
                            &surface_hm,
                            z_level,
                            ctx.stock_to_leave,
                        );
                        scope.set_counter("material_remaining_pre", diag.fraction);
                        scope.set_counter("floor_cells_total", diag.cells_total as f64);
                        scope.set_counter("floor_cells_at_z", diag.cells_at_z as f64);
                        scope.set_counter("floor_cells_surf_above", diag.cells_surf_above as f64);
                        scope.set_counter(
                            "floor_cells_with_material",
                            diag.cells_with_material as f64,
                        );
                        scope
                    });
                    let segs_before = segments.len();
                    match ctx.clearing_strategy {
                        ClearingStrategy3d::ContourParallel => {
                            segments.push(Adaptive3dSegment::Marker(level_event));
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
                            segments.push(Adaptive3dSegment::Marker(level_event));
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
                            clear_z_level_agent_2d_slice(
                                &ctx,
                                &mut material_stock,
                                &surface_hm,
                                z_level,
                                &mut segments,
                                &mut last_pos,
                                Some(region),
                                Some(level_event),
                                cancel,
                            )?;
                        }
                    }
                    if let Some(scope) = level_scope {
                        let tally = tally_segments_for_z_level(&segments[segs_before..]);
                        scope.set_counter("planner_cut_segments", tally.cut_segs as f64);
                        scope.set_counter("planner_rapid_segments", tally.rapid_segs as f64);
                        scope.set_counter("planner_link_segments", tally.link_segs as f64);
                        scope.set_counter("planner_cut_mm", tally.cut_mm);
                        scope.set_counter("planner_cut_path_points", tally.cut_path_points as f64);
                        let diag_post = material_remaining_at_level_diag(
                            &material_stock,
                            &surface_hm,
                            z_level,
                            ctx.stock_to_leave,
                        );
                        scope.set_counter("material_remaining_post", diag_post.fraction);
                        scope.set_counter(
                            "floor_cells_with_material_post",
                            diag_post.cells_with_material as f64,
                        );
                        scope.finish();
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
                    params.safe_z,
                    params.tolerance,
                    params.min_cutting_radius,
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
                let level_event = Adaptive3dRuntimeEvent::GlobalZLevel {
                    z_level,
                    level_index: level_idx + 1,
                    level_total: z_levels.len(),
                    metrics: ZLevelPlanMetrics::default(),
                };
                let level_scope = debug_ctx.map(|dctx| {
                    let scope = dctx.start_span(
                        "z_level_clear",
                        format!("Z {:.3} ({}/{})", z_level, level_idx + 1, z_levels.len()),
                    );
                    scope.set_z_level(z_level);
                    let diag = material_remaining_at_level_diag(
                        &material_stock,
                        &surface_hm,
                        z_level,
                        ctx.stock_to_leave,
                    );
                    scope.set_counter("material_remaining_pre", diag.fraction);
                    scope.set_counter("floor_cells_total", diag.cells_total as f64);
                    scope.set_counter("floor_cells_at_z", diag.cells_at_z as f64);
                    scope.set_counter("floor_cells_surf_above", diag.cells_surf_above as f64);
                    scope.set_counter("floor_cells_with_material", diag.cells_with_material as f64);
                    scope
                });
                let segs_before = segments.len();
                match ctx.clearing_strategy {
                    ClearingStrategy3d::ContourParallel => {
                        segments.push(Adaptive3dSegment::Marker(level_event));
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
                        segments.push(Adaptive3dSegment::Marker(level_event));
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
                        clear_z_level_agent_2d_slice(
                            &ctx,
                            &mut material_stock,
                            &surface_hm,
                            z_level,
                            &mut segments,
                            &mut last_pos,
                            None,
                            Some(level_event),
                            cancel,
                        )?;
                    }
                }
                if let Some(scope) = level_scope {
                    let tally = tally_segments_for_z_level(&segments[segs_before..]);
                    scope.set_counter("planner_cut_segments", tally.cut_segs as f64);
                    scope.set_counter("planner_rapid_segments", tally.rapid_segs as f64);
                    scope.set_counter("planner_link_segments", tally.link_segs as f64);
                    scope.set_counter("planner_cut_mm", tally.cut_mm);
                    scope.set_counter("planner_cut_path_points", tally.cut_path_points as f64);
                    let diag_post = material_remaining_at_level_diag(
                        &material_stock,
                        &surface_hm,
                        z_level,
                        ctx.stock_to_leave,
                    );
                    scope.set_counter("material_remaining_post", diag_post.fraction);
                    scope.set_counter(
                        "floor_cells_with_material_post",
                        diag_post.cells_with_material as f64,
                    );
                    scope.finish();
                }

                // Waterline cleanup at every Z-level. Historically this only
                // ran on `is_last_level`, which meant adaptive misses at upper
                // levels stayed for the finish pass to deal with. Running it
                // per-level trades some generation time for cleaner roughing
                // output and reduces load on the subsequent finish.
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
                    params.safe_z,
                    params.tolerance,
                    params.min_cutting_radius,
                    &mut segments,
                    &mut last_pos,
                    debug_ctx,
                    cancel,
                )?;
            }
        }
    }

    Ok(Adaptive3dSegmentsResult {
        segments,
        final_material_stock: material_stock,
        surface_heightmap: surface_hm,
    })
}

// ── Public API ────────────────────────────────────────────────────────

/// Convert segments to a toolpath and collect annotations.
pub(super) fn segments_to_toolpath(
    segments: &[Adaptive3dSegment],
    params: &Adaptive3dParams,
) -> (Toolpath, Vec<Adaptive3dRuntimeAnnotation>) {
    let mut tp = Toolpath::new();
    let mut annotations = Vec::new();
    // D4 — running pass index for `PassEntry` annotations. Increments
    // once per `Adaptive3dSegment::Rapid` / `RapidWithFloor` (each is
    // an entry into a new pass). Used as the operator-facing pass
    // ordinal in narrate / semantic output and as the structural
    // Entry span's label.
    let mut pass_counter: usize = 0;

    // Lift the tool to safe_z above its current XY before any
    // traverse-then-plunge sequence. Without this, a Rapid segment
    // emitted immediately after a Cut (which leaves the tool at cut
    // depth) produces a single diagonal rapid from the cut-depth
    // position to (entry.xy, safe_z) — and that diagonal can cross
    // material the cutter would rather not run through at rapid
    // feed. Skip the lift if the tool is already at or above
    // safe_z. See planning/adaptive_remediation_phase2_probes_2026-04-12.md
    // for the empirical regression this fixes (F-5, F-6).
    let lift_to_safe_z = |tp: &mut Toolpath, safe_z: f64| {
        if let Some(last) = tp.moves.last()
            && last.target.z < safe_z
        {
            tp.rapid_to(P3::new(last.target.x, last.target.y, safe_z));
        }
    };

    // D4 — operator-facing label of the configured entry style. Used
    // both as the `style_label` field on the emitted `PassEntry`
    // annotation and downstream in `compute::spans` / `compute::annotate`
    // to label the resulting `SpanKind::Entry` / semantic `Entry`.
    let entry_style_label: &'static str = match params.entry_style {
        EntryStyle3d::Plunge => "plunge entry",
        EntryStyle3d::Helix { .. } => "helix entry",
        EntryStyle3d::Ramp { .. } => "ramp entry",
    };

    for segment in segments {
        match segment {
            Adaptive3dSegment::Marker(event) => {
                annotations.push(Adaptive3dRuntimeAnnotation {
                    move_index: tp.moves.len(),
                    event: event.clone(),
                });
            }
            Adaptive3dSegment::Rapid(entry) => {
                let entry_start = tp.moves.len();
                match params.entry_style {
                    EntryStyle3d::Plunge => {
                        lift_to_safe_z(&mut tp, params.safe_z);
                        tp.rapid_to(P3::new(entry.x, entry.y, params.safe_z));
                        emit_peck_plunge(&mut tp, entry, params.safe_z, params);
                    }
                    EntryStyle3d::Helix { radius, pitch } => {
                        lift_to_safe_z(&mut tp, params.safe_z);
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
                        lift_to_safe_z(&mut tp, params.safe_z);
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
                };
                let entry_end = tp.moves.len();
                if entry_end > entry_start {
                    annotations.push(Adaptive3dRuntimeAnnotation {
                        move_index: entry_start,
                        event: Adaptive3dRuntimeEvent::PassEntry {
                            pass_index: pass_counter,
                            entry_x: entry.x,
                            entry_y: entry.y,
                            entry_z: entry.z,
                            entry_end_move_idx: entry_end,
                            style_label: entry_style_label,
                        },
                    });
                    pass_counter += 1;
                }
            }
            Adaptive3dSegment::RapidWithFloor {
                entry,
                rapid_floor_z,
            } => {
                let entry_start = tp.moves.len();
                match params.entry_style {
                    EntryStyle3d::Plunge => {
                        // Skip the peck-feed through cleared air. The clearing
                        // function sampled stock_top at this XY and tells us
                        // there's nothing solid down to `rapid_floor_z` —
                        // rapid through it, then peck only the remaining
                        // fresh-material descent.
                        //
                        // Buffer above the sampled stock_top in case the dexel
                        // sample under-reports by a fraction of a cell height
                        // (sub-mm safety margin keeps the plunge from biting
                        // material at rapid speed if the sample was slightly
                        // off).
                        const RAPID_DESCENT_BUFFER_MM: f64 = 0.5;
                        lift_to_safe_z(&mut tp, params.safe_z);
                        tp.rapid_to(P3::new(entry.x, entry.y, params.safe_z));
                        let descent_floor = (*rapid_floor_z + RAPID_DESCENT_BUFFER_MM)
                            .min(params.safe_z)
                            .max(entry.z);
                        if descent_floor < params.safe_z - 1e-6 {
                            tp.rapid_to(P3::new(entry.x, entry.y, descent_floor));
                        }
                        emit_peck_plunge(&mut tp, entry, descent_floor, params);
                    }
                    // Helix and Ramp entries already self-pace their descent;
                    // the rapid-floor optimisation doesn't apply (the helix /
                    // ramp's whole point is to descend at a controlled rate).
                    EntryStyle3d::Helix { radius, pitch } => {
                        lift_to_safe_z(&mut tp, params.safe_z);
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
                        lift_to_safe_z(&mut tp, params.safe_z);
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
                };
                let entry_end = tp.moves.len();
                if entry_end > entry_start {
                    annotations.push(Adaptive3dRuntimeAnnotation {
                        move_index: entry_start,
                        event: Adaptive3dRuntimeEvent::PassEntry {
                            pass_index: pass_counter,
                            entry_x: entry.x,
                            entry_y: entry.y,
                            entry_z: entry.z,
                            entry_end_move_idx: entry_end,
                            style_label: entry_style_label,
                        },
                    });
                    pass_counter += 1;
                }
            }
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

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;
    use crate::toolpath::MoveType;

    fn minimal_params() -> Adaptive3dParams {
        Adaptive3dParams {
            tool_radius: 3.175,
            envelope_radius: 3.175,
            stepover: 2.0,
            depth_per_pass: 3.0,
            stock_to_leave: 0.5,
            feed_rate: 1500.0,
            plunge_rate: 500.0,
            safe_z: 10.0,
            tolerance: 0.1,
            min_cutting_radius: 0.0,
            stock_top_z: 5.0,
            entry_style: EntryStyle3d::Plunge,
            fine_stepdown: None,
            detect_flat_areas: false,
            max_stay_down_dist: None,
            region_ordering: RegionOrdering::Global,
            initial_stock: None,
            boundary: None,
            clearing_strategy: ClearingStrategy3d::ContourParallel,
            z_blend: false,
        }
    }

    /// Closes the F-5/F-6 regression found during the April 2026 Phase 2
    /// empirical probes: a Rapid segment emitted after a Cut must lift
    /// to safe_z FIRST (at the current XY), THEN traverse XY at safe_z,
    /// THEN plunge. Before this fix, `segments_to_toolpath` only emitted
    /// the traverse — a single diagonal rapid from the cut depth to
    /// (entry.xy, safe_z) — which can cut through material as a rapid.
    ///
    /// See planning/adaptive_remediation_phase2_probes_2026-04-12.md.
    #[test]
    fn rapid_segment_lifts_to_safe_z_before_traverse() {
        let params = minimal_params();
        // Cut path: end at (5, 5, -3) — tool is deep in material.
        let cut1 = Adaptive3dSegment::Cut(vec![P3::new(0.0, 0.0, -3.0), P3::new(5.0, 5.0, -3.0)]);
        // Rapid to a new entry point at (20, 20, -2). This is what
        // Package F emits when is_clear_path_3d rejects a would-be Link.
        let rapid = Adaptive3dSegment::Rapid(P3::new(20.0, 20.0, -2.0));
        let cut2 =
            Adaptive3dSegment::Cut(vec![P3::new(20.0, 20.0, -2.0), P3::new(25.0, 25.0, -2.0)]);

        let (tp, _) = segments_to_toolpath(&[cut1, rapid, cut2], &params);

        // Sanity: there should be moves.
        assert!(!tp.moves.is_empty());

        // After cut1 ends at (5,5,-3), expect:
        //   - Rapid to (5,5,safe_z)        — lift in place
        //   - Rapid to (20,20,safe_z)      — traverse at safe_z
        //   - One or more peck feeds at (20,20,...) descending toward
        //     entry.z = -2 (peck behaviour added 2026-05-02 to avoid
        //     punched-hole plunges; see segments_to_toolpath).
        //   - Feed to (20,20,-2)            — final plunge
        let cut1_end = tp.moves.iter().position(|m| {
            (m.target.x - 5.0).abs() < 1e-9
                && (m.target.y - 5.0).abs() < 1e-9
                && (m.target.z - (-3.0)).abs() < 1e-9
        });
        let i = cut1_end.expect("cut1 endpoint not found");
        assert!(
            matches!(tp.moves[i + 1].move_type, MoveType::Rapid)
                && (tp.moves[i + 1].target.x - 5.0).abs() < 1e-9
                && (tp.moves[i + 1].target.y - 5.0).abs() < 1e-9
                && (tp.moves[i + 1].target.z - params.safe_z).abs() < 1e-9,
            "expected lift in place to safe_z, got {:?}",
            tp.moves[i + 1]
        );
        assert!(
            matches!(tp.moves[i + 2].move_type, MoveType::Rapid)
                && (tp.moves[i + 2].target.x - 20.0).abs() < 1e-9
                && (tp.moves[i + 2].target.y - 20.0).abs() < 1e-9
                && (tp.moves[i + 2].target.z - params.safe_z).abs() < 1e-9,
            "expected traverse to (20,20,safe_z), got {:?}",
            tp.moves[i + 2]
        );
        // Walk past peck moves (all at XY = 20,20) until we land at z = -2.
        let final_plunge_idx = tp.moves[i + 3..]
            .iter()
            .position(|m| {
                matches!(m.move_type, MoveType::Linear { .. })
                    && (m.target.x - 20.0).abs() < 1e-9
                    && (m.target.y - 20.0).abs() < 1e-9
                    && (m.target.z - (-2.0)).abs() < 1e-9
            })
            .map(|p| p + i + 3)
            .expect("final plunge to entry.z = -2 not found after lift+traverse");
        // All moves between traverse and final plunge must be at the
        // entry XY (peck phase doesn't drift in XY).
        for m in &tp.moves[i + 3..final_plunge_idx] {
            assert!(
                (m.target.x - 20.0).abs() < 1e-9 && (m.target.y - 20.0).abs() < 1e-9,
                "peck move drifted off entry XY: {:?}",
                m
            );
        }
    }

    /// The lift-to-safe-z move should only be emitted when the tool is
    /// currently BELOW safe_z. If the previous move already left the
    /// tool at or above safe_z, we shouldn't add a redundant rapid.
    #[test]
    fn rapid_segment_skips_redundant_lift_when_already_at_safe_z() {
        let params = minimal_params();
        // An empty tp (no previous moves): first Rapid shouldn't emit a
        // spurious lift either. With peck-plunge, the move sequence is
        // rapid_to(safe_z) → peck feeds/retracts → final feed → final
        // retract. The "no spurious lift" property is verified by
        // checking that the first move IS the lateral approach rapid
        // to safe_z, not a redundant in-place lift before it.
        let rapid = Adaptive3dSegment::Rapid(P3::new(20.0, 20.0, -2.0));
        let (tp, _) = segments_to_toolpath(&[rapid], &params);
        assert!(
            matches!(tp.moves[0].move_type, MoveType::Rapid),
            "first move should be Rapid, got {:?}",
            tp.moves[0]
        );
        assert!(
            (tp.moves[0].target.x - 20.0).abs() < 1e-9
                && (tp.moves[0].target.y - 20.0).abs() < 1e-9
                && (tp.moves[0].target.z - params.safe_z).abs() < 1e-9,
            "first move should be the approach rapid to (entry.xy, safe_z), got {:?}",
            tp.moves[0]
        );
    }
}
