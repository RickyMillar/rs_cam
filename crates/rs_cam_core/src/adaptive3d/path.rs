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
    clear_z_level_contour_parallel, clear_z_level_dispatch_no_marker, detect_material_regions,
    waterline_cleanup,
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
    use crate::toolpath::MoveIntent;
    const PECK_CLEARANCE_MM: f64 = 0.5;
    let dpp = params.depth_per_pass.max(0.1);
    let mut current_z = start_z;
    while current_z - entry.z > dpp + 1e-6 {
        let next_z = current_z - dpp;
        tp.feed_to_with_intent(
            P3::new(entry.x, entry.y, next_z),
            params.plunge_rate,
            MoveIntent::EntryPlunge,
        );
        let retract_z = next_z + PECK_CLEARANCE_MM;
        tp.rapid_to_with_intent(P3::new(entry.x, entry.y, retract_z), MoveIntent::Retract);
        // Track the committed cut floor, not the retract height. If the
        // retract clearance equals or exceeds depth_per_pass, using the
        // retract height here makes the loop non-progressing.
        current_z = next_z;
    }
    tp.feed_to_with_intent(*entry, params.plunge_rate, MoveIntent::EntryPlunge);
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
    /// Stage 4 — planner-predicted leading-arc engagement `(cut_point,
    /// α/2π)` collected across all ContourSpiral slices. Empty for other
    /// strategies. Consumed by the feed modulator via positional lookup.
    pub planner_engagement: Vec<(P3, f64)>,
    /// Test-only: planner's internal dexel state at the end of the run.
    #[allow(dead_code)]
    pub final_material_stock: TriDexelStock,
    /// Test-only: heightmap used to surface-drape Cut paths.
    #[allow(dead_code)]
    pub surface_heightmap: SurfaceHeightmap,
}

/// F-029 probe: build the planner-internal segments + final material_stock
/// for an adaptive3d run with the given params. Exposed so the F-029
/// acceptance test can compare planner stock state to simulator stock state
/// at the same cell layout to confirm parity.
#[doc(hidden)]
#[allow(clippy::type_complexity)] // diagnostic probe; returns raw cell-grid metadata
pub fn debug_adaptive_3d_segments_for_f029_probe(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &super::Adaptive3dParams,
    cancel: &dyn CancelCheck,
) -> Result<(Vec<f32>, usize, usize, f64, f64, f64, f64, f64), Cancelled> {
    let res = adaptive_3d_segments(mesh, index, cutter, params, None, cancel)?;
    let _ = &res.segments;
    let _ = &res.surface_heightmap;
    let grid = &res.final_material_stock.z_grid;
    // Return the ray top z values (using top of last segment) per cell
    let mut tops = Vec::with_capacity(grid.rows * grid.cols);
    for row in 0..grid.rows {
        for col in 0..grid.cols {
            let ray = grid.ray(row, col);
            let top = ray.iter().map(|s| s.exit).fold(f32::NEG_INFINITY, f32::max);
            tops.push(top);
        }
    }
    Ok((
        tops,
        grid.rows,
        grid.cols,
        grid.cell_size,
        grid.origin_u,
        grid.origin_v,
        res.final_material_stock.stock_bbox.min.z,
        res.final_material_stock.stock_bbox.max.z,
    ))
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

    // Grid geometry: expand mesh bbox by cutter radius. If a world stock
    // XY bbox is provided (F-027), union it with the mesh-derived bounds
    // so the planner's internal `material_stock` extends across every
    // cell the simulator's per-setup dexel grid will look at. Without
    // this, cells inside the simulator grid but outside the planner grid
    // never see planner stamps; the simulator carries them as virgin
    // material across the entire toolpath, and the final pass scrapes
    // the full pre-stamp ray in one stamp → axial spike → deflection
    // Exceeds at the model-edge outliers. See finding F-027.
    let bbox = &mesh.bbox;
    let (origin_x, origin_y, extent_x, extent_y) =
        if let Some((wx_min, wy_min, wx_max, wy_max)) = params.world_stock_xy_bbox {
            (
                (bbox.min.x - r).min(wx_min),
                (bbox.min.y - r).min(wy_min),
                (bbox.max.x + r).max(wx_max),
                (bbox.max.y + r).max(wy_max),
            )
        } else {
            (
                bbox.min.x - r,
                bbox.min.y - r,
                bbox.max.x + r,
                bbox.max.y + r,
            )
        };
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

    // Pre-compute the shallow-area mask once (it only depends on the
    // surface geometry, not on the running stock). Indexed row-major,
    // same layout as surface_hm.z_values / slope_map.angles. Cell is
    // `true` when its surface slope < shallow_angle_rad. We toggle
    // ctx.shallow_mask between this and None at the per-Z-level loop
    // boundary: None for the main DPP clear, Some(mask) for shallow
    // sub-passes within each DPP descent.
    let shallow_mask: Option<Vec<bool>> = match (
        params.mill_shallow_areas,
        params.shallow_angle_rad,
        params.shallow_stepdown,
    ) {
        (true, Some(angle), Some(step)) if step > 0.0 && step < params.depth_per_pass => {
            Some(slope_map.angles.iter().map(|&a| a < angle).collect())
        }
        _ => None,
    };

    // Clear material at cells outside the mesh XY footprint.
    // Drop-cutter returns min_z for cells beyond the mesh edge, creating phantom
    // "deep material" that the tool can never reach. Mark these as already cleared
    // so the adaptive doesn't waste passes trying to cut in empty space.
    // Only clear cells whose XY center is outside the mesh bbox (with tolerance).
    //
    // F-027: when `world_stock_xy_bbox` is supplied, the planner grid was
    // widened to enclose the world stock footprint (potentially much
    // larger than `mesh.bbox + tool_radius`). The cells between the mesh
    // boundary and the world stock boundary are **real stock material**
    // that the simulator carries from the start of the toolpath — they
    // sit under workholding / off-model fixturing, not phantom drop-
    // cutter floor. Pre-fix the planner pre-cleared them (mesh-boundary
    // condition above) and emitted no cuts to touch them; the simulator
    // then carried them as virgin material until the cutter's footprint
    // swept in and the simulator's first stamp removed the full pre-
    // stamp ray (axial_engagement_mm reading the full stock height →
    // deflection Exceeds). Treating those cells as already-cleared in
    // the planner is what causes the planner↔simulator mismatch.
    //
    // Fix: only border-clear cells outside the world stock bbox. Cells
    // inside the world stock bbox keep their planner-side material so
    // the planner's clearing strategy (region detection + EDT-based
    // contour parallel / adaptive) plans passes that stamp them down
    // DPP at a time. The simulator then sees pre-cleared rays at the
    // boundary cells the same way it does for in-mesh cells.
    let border_scope = debug_ctx.map(|ctx| ctx.start_span("border_clear", "Border clear"));
    let border_margin = r * 0.5;
    let world_xy = params.world_stock_xy_bbox;
    let mut border_cleared = 0u32;
    for row in 0..material_stock.z_grid.rows {
        if row % 16 == 0 {
            check_cancel(cancel)?;
        }
        for col in 0..material_stock.z_grid.cols {
            let (x, y) = material_stock.z_grid.cell_to_world(row, col);
            let outside_mesh = x < bbox.min.x - border_margin
                || x > bbox.max.x + border_margin
                || y < bbox.min.y - border_margin
                || y > bbox.max.y + border_margin;
            if !outside_mesh {
                continue;
            }
            // F-027 inhibition: keep cells inside the world stock bbox as
            // material so the planner emits cuts to clear them in step
            // with the simulator. Outside-world-stock cells are still
            // border-cleared (they're phantom — neither in the model nor
            // in the user's stock block).
            if let Some((wx_min, wy_min, wx_max, wy_max)) = world_xy
                && x >= wx_min
                && x <= wx_max
                && y >= wy_min
                && y <= wy_max
            {
                continue;
            }
            // Clear material above the surface Z at this cell. Uncovered
            // cells (no triangle under the ray) carry a BFS rim-fill in
            // `z_values` post-audit; pre-audit they read the drop-cutter
            // floor, which made this a full-ray clear — preserve that by
            // clearing from the grid bottom for uncovered cells.
            let i = row * material_stock.z_grid.cols + col;
            let clear_z = if surface_hm.covered[i] {
                surface_hm.z_values[i] as f32
            } else {
                material_stock.stock_bbox.min.z as f32
            };
            ray_subtract_above(material_stock.z_grid.ray_mut(row, col), clear_z);
            border_cleared += 1;
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
                    // Same covered/uncovered split as the border clear:
                    // uncovered cells outside the boundary must stay fully
                    // cleared or O5b's unstamped-cell deep bite returns.
                    let i = row * material_stock.z_grid.cols + col;
                    let clear_z = if surface_hm.covered[i] {
                        surface_hm.z_values[i] as f32
                    } else {
                        material_stock.stock_bbox.min.z as f32
                    };
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

    // Compute Z levels: stock_top down to surface bottom + stock_to_leave.
    // A user-pinned heights `bottom_z` (params.z_floor) clamps the plan —
    // the surface heightmap reads the mesh-bbox floor through holes in
    // open meshes, and pre-clamp there was no lever to stop the final
    // level diving there (heights audit 2026-06-12, findings 2 + 3).
    let z_plan_scope = debug_ctx.map(|ctx| ctx.start_span("z_level_plan", "Compute Z levels"));
    let surface_bottom = surface_hm.min_z();
    let z_bottom =
        (surface_bottom + params.stock_to_leave).max(params.z_floor.unwrap_or(f64::NEG_INFINITY));
    let mut z_levels = Vec::new();
    let mut z = params.stock_top_z - params.depth_per_pass;
    while z > z_bottom {
        z_levels.push(z);
        z -= params.depth_per_pass;
    }
    if z_bottom < params.stock_top_z {
        z_levels.push(z_bottom); // Always include final level at the surface
    }

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
    let mut ctx = ClearZLevelContext {
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
        feed_rate: params.feed_rate,
        plunge_rate: params.plunge_rate,
        target_frac,
        step_len,
        max_link_dist,
        bbox_x_min,
        bbox_x_max,
        bbox_y_min,
        bbox_y_max,
        clearing_strategy: params.clearing_strategy,
        engagement_measure: params.engagement_measure,
        z_blend: params.z_blend,
        safe_z: params.safe_z,
        min_cutting_radius: params.min_cutting_radius,
        // Default to no mask. The per-Z-level loop toggles this to
        // Some(&shallow_mask) for the shallow sub-passes only.
        shallow_mask: None,
        min_region_cut_length_mm: params.min_region_cut_length_mm,
    };

    let mut segments = Vec::new();
    // Stage 4 — planner-predicted leading-arc engagement samples
    // `(cut_point, α/2π)`, accumulated across all spiral slices and
    // returned for the feed modulator's positional lookup.
    let mut planner_eng: Vec<(P3, f64)> = Vec::new();
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
                        ClearingStrategy3d::AgentSearch | ClearingStrategy3d::ContourSpiral => {
                            clear_z_level_agent_2d_slice(
                                &ctx,
                                &mut material_stock,
                                &surface_hm,
                                z_level,
                                &mut segments,
                                &mut last_pos,
                                &mut planner_eng,
                                Some(region),
                                Some(level_event),
                                cancel,
                            )?;
                        }
                    }
                    // Shallow sub-passes within this DPP descent — strategy-
                    // agnostic. Restricted to low-slope cells via
                    // ctx.shallow_mask, then dispatched back into the same
                    // clear function the main pass used.
                    if let (Some(mask), Some(step)) =
                        (shallow_mask.as_deref(), params.shallow_stepdown)
                    {
                        ctx.shallow_mask = Some(mask);
                        let next_main_z = z_level - params.depth_per_pass;
                        let mut sub_z = z_level - step;
                        while sub_z > next_main_z + 1e-3 {
                            check_cancel(cancel)?;
                            clear_z_level_dispatch_no_marker(
                                &ctx,
                                &mut material_stock,
                                &surface_hm,
                                sub_z,
                                &mut segments,
                                &mut last_pos,
                                &mut planner_eng,
                                Some(region),
                                cancel,
                            )?;
                            sub_z -= step;
                        }
                        ctx.shallow_mask = None;
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
                    ClearingStrategy3d::AgentSearch | ClearingStrategy3d::ContourSpiral => {
                        clear_z_level_agent_2d_slice(
                            &ctx,
                            &mut material_stock,
                            &surface_hm,
                            z_level,
                            &mut segments,
                            &mut last_pos,
                            &mut planner_eng,
                            None,
                            Some(level_event),
                            cancel,
                        )?;
                    }
                }
                // Shallow sub-passes — strategy-agnostic, see ByArea branch
                // for rationale.
                if let (Some(mask), Some(step)) = (shallow_mask.as_deref(), params.shallow_stepdown)
                {
                    ctx.shallow_mask = Some(mask);
                    let next_main_z = z_level - params.depth_per_pass;
                    let mut sub_z = z_level - step;
                    while sub_z > next_main_z + 1e-3 {
                        check_cancel(cancel)?;
                        clear_z_level_dispatch_no_marker(
                            &ctx,
                            &mut material_stock,
                            &surface_hm,
                            sub_z,
                            &mut segments,
                            &mut last_pos,
                            &mut planner_eng,
                            None,
                            cancel,
                        )?;
                        sub_z -= step;
                    }
                    ctx.shallow_mask = None;
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
        planner_engagement: planner_eng,
        final_material_stock: material_stock,
        surface_heightmap: surface_hm,
    })
}

// ── Public API ────────────────────────────────────────────────────────

/// F-038b: maximum mesh Z along the XY straight line between two points.
///
/// Drops a flat-tip probe down at evenly-spaced samples along
/// `from`→`to` using `crate::dropcutter::point_drop_cutter`. Returns the
/// maximum Z reported by any sample, or `None` if the cutter never
/// contacted the mesh along the line (line is entirely off-mesh).
///
/// Used by `segments_to_toolpath` to decide whether a `Rapid` / `RapidWithFloor`
/// transition can be replaced with a keep-tool-down feed link.
///
/// Note we use the operative `cutter` so the height respects the tool's
/// actual contact geometry (ball-tip vs flat-tip vs taper). Sampling
/// frequency is 10 stations (incl. endpoints) — matches the F-038b spec.
fn max_mesh_z_along_line(
    mesh: &crate::mesh::TriangleMesh,
    index: &crate::mesh::SpatialIndex,
    cutter: &dyn crate::tool::MillingCutter,
    from_xy: (f64, f64),
    to_xy: (f64, f64),
    samples: usize,
) -> Option<f64> {
    let samples = samples.max(2);
    let mut max_z = f64::NEG_INFINITY;
    let mut any_contact = false;
    for i in 0..samples {
        let t = i as f64 / (samples - 1) as f64;
        let x = from_xy.0 + t * (to_xy.0 - from_xy.0);
        let y = from_xy.1 + t * (to_xy.1 - from_xy.1);
        let cl = crate::dropcutter::point_drop_cutter(x, y, mesh, index, cutter);
        if cl.contacted && cl.z.is_finite() && cl.z > max_z {
            max_z = cl.z;
            any_contact = true;
        }
    }
    if any_contact { Some(max_z) } else { None }
}

/// F-038b: attempt to emit a keep-tool-down feed link between a previous
/// tool position and a new entry point. Returns `true` if the link was
/// emitted (caller skips the retract+rapid+plunge sequence), `false`
/// otherwise (caller falls back to the legacy retract path).
///
/// Algorithm (per F-038b spec):
///   1. Require previous tool position (`from`); short-circuit if absent.
///   2. Reject if `xy_distance(from, to) > max_stay_down_distance_mm`.
///   3. Sample mesh heightfield (`samples` evenly spaced incl. endpoints).
///   4. `link_z = max(samples_max, from.z, to.z) + clearance_mm`.
///   5. Reject if `link_z > safe_z` (terrain peak exceeds safe-Z guard).
///   6. Reject if `link_z > to.z + cutter_length` (shank would enter
///      uncut material above the new entry point that the tool can't
///      cut).
///   7. Emit three feed moves @ feed_rate, all tagged `MoveIntent::Linking`:
///      (a) ascend at the start XY to link_z, (b) XY traverse at
///      link_z to the entry XY, (c) descend to the entry point. Each
///      step is skipped if the start/end Z is already at link_z.
#[allow(clippy::too_many_arguments)]
fn try_emit_stay_down_link(
    tp: &mut Toolpath,
    from: P3,
    to: P3,
    mesh: &crate::mesh::TriangleMesh,
    index: &crate::mesh::SpatialIndex,
    cutter: &dyn crate::tool::MillingCutter,
    max_stay_down_distance_mm: f64,
    clearance_mm: f64,
    safe_z: f64,
    feed_rate: f64,
) -> bool {
    if max_stay_down_distance_mm <= 0.0 {
        return false;
    }
    let dx = to.x - from.x;
    let dy = to.y - from.y;
    let xy_dist = (dx * dx + dy * dy).sqrt();
    if xy_dist > max_stay_down_distance_mm {
        return false;
    }

    // F-038b spec sample count = 10 (incl. endpoints).
    let max_mesh = max_mesh_z_along_line(mesh, index, cutter, (from.x, from.y), (to.x, to.y), 10);
    // If the line is entirely off-mesh, the terrain doesn't constrain
    // us; fall back to from.z/to.z as the height ceiling.
    let terrain_max = max_mesh.unwrap_or(f64::NEG_INFINITY);
    let link_z = terrain_max.max(from.z).max(to.z) + clearance_mm;

    // Safety guard 1: link Z above safe_z means the terrain peak is
    // above the safe height. Retract is the right answer.
    if link_z > safe_z + 1e-9 {
        return false;
    }
    // Safety guard 2: the cutter's cutting length bounds how far above
    // `to.z` the shank can engage material. If the link Z requires the
    // shank to rise above `to.z + cutter_length`, the un-cuttable shank
    // would intersect the heightfield-sampled material. Retract.
    let cutter_length = cutter.length();
    if cutter_length > 0.0 && link_z > to.z + cutter_length + 1e-9 {
        return false;
    }

    use crate::toolpath::MoveIntent;
    // Step (a): ascend at the current XY to link_z if needed.
    if link_z > from.z + 1e-9 {
        tp.feed_to_with_intent(
            P3::new(from.x, from.y, link_z),
            feed_rate,
            MoveIntent::Linking,
        );
    }
    // Step (b): XY traverse at link_z.
    tp.feed_to_with_intent(P3::new(to.x, to.y, link_z), feed_rate, MoveIntent::Linking);
    // Step (c): descend to the entry point.
    if to.z < link_z - 1e-9 {
        tp.feed_to_with_intent(to, feed_rate, MoveIntent::Linking);
    }
    true
}

/// Convert segments to a toolpath and collect annotations.
///
/// `mesh` + `index` + `cutter` were added in F-038b to enable per-link
/// mesh-heightfield queries for the keep-tool-down planner. Call sites
/// that don't have a real mesh handy (pure-unit tests of segment shape)
/// can still build a trivial flat mesh via `crate::mesh::make_test_flat`
/// — the F-038b probe is a no-op when `params.max_stay_down_distance_mm`
/// resolves to 0.0, so the cutter/mesh values don't affect behaviour in
/// that case.
pub(super) fn segments_to_toolpath(
    segments: &[Adaptive3dSegment],
    params: &Adaptive3dParams,
    mesh: &crate::mesh::TriangleMesh,
    index: &crate::mesh::SpatialIndex,
    cutter: &dyn crate::tool::MillingCutter,
) -> (Toolpath, Vec<Adaptive3dRuntimeAnnotation>) {
    let mut tp = Toolpath::new();
    let mut annotations = Vec::new();
    // D4 — running pass index for `PassEntry` annotations. Increments
    // once per `Adaptive3dSegment::Rapid` / `RapidWithFloor` (each is
    // an entry into a new pass). Used as the operator-facing pass
    // ordinal in narrate / semantic output and as the structural
    // Entry span's label.
    let mut pass_counter: usize = 0;

    // F-038b: resolve the stay-down distance knob. `None` ⇒ planner
    // default of 8 × tool diameter (Fusion HSM roughing default). The
    // operator can override with `Some(x)` (incl. `Some(0.0)` to disable).
    let stay_down_dist = params
        .max_stay_down_distance_mm
        .unwrap_or_else(|| (cutter.radius() * 2.0) * 8.0);
    let stay_down_clearance = params.stay_down_clearance_mm.max(0.0);

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
            tp.rapid_to_with_intent(
                P3::new(last.target.x, last.target.y, safe_z),
                crate::toolpath::MoveIntent::Retract,
            );
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
                // F-038b: try a keep-tool-down feed link from the previous
                // tool position to `entry` before falling back to retract.
                // Only attempted for Plunge entries — Helix and Ramp have
                // their own entry geometry and aren't candidates for a
                // direct feed-to-depth link (the helix/ramp's plunge angle
                // is what protects the cutter on those styles).
                let prev_pos = tp.moves.last().map(|m| m.target);
                let stay_down_used = matches!(params.entry_style, EntryStyle3d::Plunge)
                    && prev_pos.is_some_and(|from| {
                        try_emit_stay_down_link(
                            &mut tp,
                            from,
                            *entry,
                            mesh,
                            index,
                            cutter,
                            stay_down_dist,
                            stay_down_clearance,
                            params.safe_z,
                            params.feed_rate,
                        )
                    });
                if stay_down_used {
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
                                style_label: "keep-down link",
                            },
                        });
                        pass_counter += 1;
                    }
                    continue;
                }
                match params.entry_style {
                    EntryStyle3d::Plunge => {
                        lift_to_safe_z(&mut tp, params.safe_z);
                        tp.rapid_to_with_intent(
                            P3::new(entry.x, entry.y, params.safe_z),
                            crate::toolpath::MoveIntent::Linking,
                        );
                        emit_peck_plunge(&mut tp, entry, params.safe_z, params);
                    }
                    EntryStyle3d::Helix { radius, pitch } => {
                        lift_to_safe_z(&mut tp, params.safe_z);
                        tp.rapid_to_with_intent(
                            P3::new(entry.x, entry.y, params.safe_z),
                            crate::toolpath::MoveIntent::Linking,
                        );
                        let helix_start = P3::new(entry.x, entry.y, params.safe_z);
                        // No `rapid_floor_z` was supplied for this entry, so the
                        // column below safe_z may still hold uncut stock. Pass
                        // `safe_z` as the stock_top guard so emit_helix does not
                        // emit a rapid descent below safe_z; it will plunge-feed
                        // instead. (UX-dial-in B1.)
                        crate::dressup::emit_helix(
                            &mut tp,
                            &helix_start,
                            entry,
                            radius,
                            pitch,
                            params.plunge_rate,
                            params.safe_z,
                        );
                    }
                    EntryStyle3d::Ramp { max_angle_deg } => {
                        lift_to_safe_z(&mut tp, params.safe_z);
                        tp.rapid_to_with_intent(
                            P3::new(entry.x, entry.y, params.safe_z),
                            crate::toolpath::MoveIntent::Linking,
                        );
                        let ramp_start = P3::new(entry.x, entry.y, params.safe_z);
                        crate::dressup::emit_ramp(
                            &mut tp,
                            &ramp_start,
                            entry,
                            (1.0, 0.0),
                            max_angle_deg,
                            params.plunge_rate,
                            params.safe_z,
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
                // F-038b: same keep-tool-down attempt as the plain Rapid
                // arm above. Only Plunge entries are candidates.
                let prev_pos = tp.moves.last().map(|m| m.target);
                let stay_down_used = matches!(params.entry_style, EntryStyle3d::Plunge)
                    && prev_pos.is_some_and(|from| {
                        try_emit_stay_down_link(
                            &mut tp,
                            from,
                            *entry,
                            mesh,
                            index,
                            cutter,
                            stay_down_dist,
                            stay_down_clearance,
                            params.safe_z,
                            params.feed_rate,
                        )
                    });
                if stay_down_used {
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
                                style_label: "keep-down link",
                            },
                        });
                        pass_counter += 1;
                    }
                    // Silence the unused-warning for `rapid_floor_z` in
                    // the stay-down branch — it's only consulted when we
                    // fall through to the rapid-descent code path below.
                    let _ = rapid_floor_z;
                    continue;
                }
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
                        tp.rapid_to_with_intent(
                            P3::new(entry.x, entry.y, params.safe_z),
                            crate::toolpath::MoveIntent::Linking,
                        );
                        let descent_floor = (*rapid_floor_z + RAPID_DESCENT_BUFFER_MM)
                            .min(params.safe_z)
                            .max(entry.z);
                        if descent_floor < params.safe_z - 1e-6 {
                            tp.rapid_to_with_intent(
                                P3::new(entry.x, entry.y, descent_floor),
                                crate::toolpath::MoveIntent::Linking,
                            );
                        }
                        emit_peck_plunge(&mut tp, entry, descent_floor, params);
                    }
                    // Fix 4 (helix/agent_search RCA): honor `rapid_floor_z`
                    // for Helix and Ramp the same way Plunge does. Rapid
                    // through pre-cleared air down to the floor, then start
                    // the controlled descent from there. Without this, the
                    // helix/ramp descends at plunge_rate from safe_z through
                    // every previously-cleared Z level — which (a) inflates
                    // air-cut % and (b) can intersect uncleared neighbouring
                    // stock at the helix radius during the multi-Z drop,
                    // producing rapid-into-material collisions. See
                    // `planning/F4_HELIX_AGENT_SEARCH_RCA.md`.
                    EntryStyle3d::Helix { radius, pitch } => {
                        const RAPID_DESCENT_BUFFER_MM: f64 = 0.5;
                        let descent_floor = (*rapid_floor_z + RAPID_DESCENT_BUFFER_MM)
                            .min(params.safe_z)
                            .max(entry.z);
                        lift_to_safe_z(&mut tp, params.safe_z);
                        tp.rapid_to_with_intent(
                            P3::new(entry.x, entry.y, params.safe_z),
                            crate::toolpath::MoveIntent::Linking,
                        );
                        if descent_floor < params.safe_z - 1e-6 {
                            tp.rapid_to_with_intent(
                                P3::new(entry.x, entry.y, descent_floor),
                                crate::toolpath::MoveIntent::Linking,
                            );
                        }
                        let helix_start = P3::new(entry.x, entry.y, descent_floor);
                        // `descent_floor` already sits at the dexel-sampled
                        // cleared-air floor; everything below is uncut material.
                        // Pass it as `stock_top` so emit_helix plunge-feeds the
                        // rest rather than rapid-descending into stock.
                        crate::dressup::emit_helix(
                            &mut tp,
                            &helix_start,
                            entry,
                            radius,
                            pitch,
                            params.plunge_rate,
                            descent_floor,
                        );
                    }
                    EntryStyle3d::Ramp { max_angle_deg } => {
                        const RAPID_DESCENT_BUFFER_MM: f64 = 0.5;
                        let descent_floor = (*rapid_floor_z + RAPID_DESCENT_BUFFER_MM)
                            .min(params.safe_z)
                            .max(entry.z);
                        lift_to_safe_z(&mut tp, params.safe_z);
                        tp.rapid_to_with_intent(
                            P3::new(entry.x, entry.y, params.safe_z),
                            crate::toolpath::MoveIntent::Linking,
                        );
                        if descent_floor < params.safe_z - 1e-6 {
                            tp.rapid_to_with_intent(
                                P3::new(entry.x, entry.y, descent_floor),
                                crate::toolpath::MoveIntent::Linking,
                            );
                        }
                        let ramp_start = P3::new(entry.x, entry.y, descent_floor);
                        // Same rationale as the helix variant above: descent_floor
                        // is the boundary between cleared air and uncut material.
                        crate::dressup::emit_ramp(
                            &mut tp,
                            &ramp_start,
                            entry,
                            (1.0, 0.0),
                            max_angle_deg,
                            params.plunge_rate,
                            descent_floor,
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
                tp.feed_to_with_intent(
                    *target,
                    params.feed_rate,
                    crate::toolpath::MoveIntent::Linking,
                );
            }
            Adaptive3dSegment::Cut(path) => {
                if path.len() < 2 {
                    continue;
                }
                let simplified = simplify_path_3d(path, params.tolerance);
                let blended = blend_corners_3d(&simplified, params.min_cutting_radius);
                for pt in blended.iter().skip(1) {
                    tp.feed_to_with_intent(
                        *pt,
                        params.feed_rate,
                        crate::toolpath::MoveIntent::ClearingCut,
                    );
                }
            }
        }
    }

    if let Some(last) = tp.moves.last() {
        tp.rapid_to_with_intent(
            P3::new(last.target.x, last.target.y, params.safe_z),
            crate::toolpath::MoveIntent::Retract,
        );
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
    use crate::toolpath::{MoveIntent, MoveType};

    // F-038b: legacy `segments_to_toolpath` tests need a mesh + index +
    // cutter trio to call the new signature. Since they explicitly
    // disable the keep-tool-down feature (`max_stay_down_distance_mm:
    // Some(0.0)` in `minimal_params`), the heightfield never gets
    // queried — a trivial flat mesh and an arbitrary endmill are fine.
    fn legacy_test_mesh() -> (crate::mesh::TriangleMesh, crate::mesh::SpatialIndex) {
        let m = crate::mesh::make_test_flat(100.0);
        let si = crate::mesh::SpatialIndex::build(&m, 10.0);
        (m, si)
    }
    fn legacy_test_cutter() -> crate::tool::FlatEndmill {
        crate::tool::FlatEndmill::new(6.35, 25.0)
    }

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
            z_floor: None,
            entry_style: EntryStyle3d::Plunge,
            fine_stepdown: None,
            detect_flat_areas: false,
            max_stay_down_dist: None,
            region_ordering: RegionOrdering::Global,
            initial_stock: None,
            boundary: None,
            clearing_strategy: ClearingStrategy3d::ContourParallel,
            engagement_measure: crate::adaptive::EngagementMeasure::DiskArea,
            z_blend: false,
            mill_shallow_areas: false,
            shallow_angle_rad: None,
            shallow_stepdown: None,
            world_stock_xy_bbox: None,
            min_region_cut_length_mm: 0.0,
            // F-038b: legacy `segments_to_toolpath` unit tests expect the
            // pre-F-038b retract-rapid-plunge sequence between Rapid
            // segments. Disable stay-down explicitly (Some(0.0)) so those
            // tests keep their existing structural assertions.
            max_stay_down_distance_mm: Some(0.0),
            stay_down_clearance_mm: 0.5,
        }
    }

    #[test]
    fn peck_plunge_progresses_when_depth_per_pass_equals_retract_clearance() {
        let mut params = minimal_params();
        params.safe_z = 1.0;
        params.depth_per_pass = 0.5;

        let rapid = Adaptive3dSegment::Rapid(P3::new(0.0, 0.0, 0.0));
        let (mesh, si) = legacy_test_mesh();
        let cutter = legacy_test_cutter();
        let (tp, _) = segments_to_toolpath(&[rapid], &params, &mesh, &si, &cutter);

        assert!(
            tp.moves.len() <= 6,
            "DPP equal to peck clearance should not create a runaway plunge loop; got {} moves",
            tp.moves.len()
        );
        let entry_plunges = tp
            .moves
            .iter()
            .filter(|m| m.intent == MoveIntent::EntryPlunge)
            .count();
        assert_eq!(entry_plunges, 2);
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

        let (mesh, si) = legacy_test_mesh();
        let cutter = legacy_test_cutter();
        let (tp, _) = segments_to_toolpath(&[cut1, rapid, cut2], &params, &mesh, &si, &cutter);

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
        let (mesh, si) = legacy_test_mesh();
        let cutter = legacy_test_cutter();
        let (tp, _) = segments_to_toolpath(&[rapid], &params, &mesh, &si, &cutter);
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
