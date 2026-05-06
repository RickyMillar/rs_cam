//! Free-function stamping helpers used by `TriDexelStock::simulate_*` and
//! `TriDexelStock::stamp_*` methods.
//!
//! These functions operate directly on a `DexelGrid` and are axis-agnostic:
//! the caller decomposes world coordinates into `(grid_u, grid_v, ray_depth)`
//! via `StockCutDirection::decompose` and then calls into this module.
//!
//! All items are `pub(super)` so they are reachable from `mod.rs` and
//! `simulation.rs` within the `dexel_stock` module, but do not leak out of
//! the crate.

use crate::dexel::{
    DexelGrid, ray_material_length, ray_material_length_above, ray_subtract_above,
    ray_subtract_below,
};
use crate::geo::P3;
use crate::radial_profile::RadialProfileLUT;
use crate::semantic_trace::ToolpathSemanticTrace;
use crate::simulation_cut::{CutKinematics, SimulationCutSample};

#[derive(Clone, Copy)]
pub(super) struct CuttingCaptureParams {
    pub(super) toolpath_id: usize,
    pub(super) move_index: usize,
    pub(super) feed_rate_mm_min: f64,
    pub(super) spindle_rpm: u32,
    pub(super) flute_count: u32,
    pub(super) semantic_item_id: Option<u64>,
    pub(super) sample_step_mm: f64,
    pub(super) cut_kinematics: CutKinematics,
    pub(super) capture_arc_engagement: bool,
}

// ── Grid-generic stamp helpers ───────────────────────────────────────────

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// Stamp a tool at a single position on a grid (axis-agnostic).
///
/// `(cu, cv)` is the tool center in the grid's planar axes.
/// `tip_depth` is the tool tip coordinate along the grid's ray axis.
/// `from_high` selects `subtract_above` (true) or `subtract_below` (false).
pub(super) fn stamp_point_on_grid(
    grid: &mut DexelGrid,
    lut: &RadialProfileLUT,
    radius: f64,
    cu: f64,
    cv: f64,
    tip_depth: f64,
    from_high: bool,
) {
    let cs = grid.cell_size;

    let col_min = ((cu - radius - grid.origin_u) / cs).floor() as isize;
    let col_max = ((cu + radius - grid.origin_u) / cs).ceil() as isize;
    let row_min = ((cv - radius - grid.origin_v) / cs).floor() as isize;
    let row_max = ((cv + radius - grid.origin_v) / cs).ceil() as isize;

    let col_lo = col_min.max(0) as usize;
    let col_hi = (col_max as usize).min(grid.cols.saturating_sub(1));
    let row_lo = row_min.max(0) as usize;
    let row_hi = (row_max as usize).min(grid.rows.saturating_sub(1));

    let r_sq = lut.radius_sq();

    for row in row_lo..=row_hi {
        let cell_v = grid.origin_v + row as f64 * cs;
        let dv = cell_v - cv;
        let dv_sq = dv * dv;
        if dv_sq > r_sq {
            continue;
        }
        for col in col_lo..=col_hi {
            let cell_u = grid.origin_u + col as f64 * cs;
            let du = cell_u - cu;
            let dist_sq = du * du + dv_sq;
            if let Some(h) = lut.height_at_dist_sq(dist_sq) {
                let ray = &mut grid.rays[row * grid.cols + col];
                if from_high {
                    // Tool enters from +Z: cutter surface is above the tip.
                    let surface = (tip_depth + h) as f32;
                    ray_subtract_above(ray, surface);
                } else {
                    // Tool enters from -Z: cutter surface is below the tip.
                    let surface = (tip_depth - h) as f32;
                    ray_subtract_below(ray, surface);
                }
            }
        }
    }
}

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// Stamp a tool along a linear segment on a grid (axis-agnostic).
///
/// `start` and `end` are `(u, v, depth)` — the segment endpoints decomposed
/// into the grid's planar axes (u, v) and ray-depth axis (depth).
#[allow(clippy::too_many_arguments)]
pub(super) fn stamp_segment_on_grid(
    grid: &mut DexelGrid,
    lut: &RadialProfileLUT,
    radius: f64,
    start: (f64, f64, f64),
    end: (f64, f64, f64),
    from_high: bool,
) {
    let (su, sv, sd) = start;
    let (eu, ev, ed) = end;
    let seg_du = eu - su;
    let seg_dv = ev - sv;
    let seg_dd = ed - sd;
    let seg_len_sq = seg_du * seg_du + seg_dv * seg_dv;

    // Degenerate segment (zero planar length) — stamp at the min depth.
    if seg_len_sq < 1e-20 {
        let d = sd.min(ed);
        stamp_point_on_grid(grid, lut, radius, su, sv, d, from_high);
        return;
    }

    let inv_seg_len_sq = 1.0 / seg_len_sq;
    let cs = grid.cell_size;

    let u_min = su.min(eu) - radius;
    let u_max = su.max(eu) + radius;
    let v_min = sv.min(ev) - radius;
    let v_max = sv.max(ev) + radius;

    let col_lo = ((u_min - grid.origin_u) / cs).floor().max(0.0) as usize;
    let col_hi = (((u_max - grid.origin_u) / cs).ceil() as usize).min(grid.cols.saturating_sub(1));
    let row_lo = ((v_min - grid.origin_v) / cs).floor().max(0.0) as usize;
    let row_hi = (((v_max - grid.origin_v) / cs).ceil() as usize).min(grid.rows.saturating_sub(1));

    for row in row_lo..=row_hi {
        let cell_v = grid.origin_v + row as f64 * cs;
        let pv = cell_v - sv;

        for col in col_lo..=col_hi {
            let cell_u = grid.origin_u + col as f64 * cs;
            let pu = cell_u - su;

            let t = ((pu * seg_du + pv * seg_dv) * inv_seg_len_sq).clamp(0.0, 1.0);

            let closest_u = t * seg_du;
            let closest_v = t * seg_dv;
            let du = pu - closest_u;
            let dv = pv - closest_v;
            let dist_sq = du * du + dv * dv;

            if let Some(h) = lut.height_at_dist_sq(dist_sq) {
                let depth = sd + t * seg_dd;
                let ray = &mut grid.rays[row * grid.cols + col];
                if from_high {
                    let surface = (depth + h) as f32;
                    ray_subtract_above(ray, surface);
                } else {
                    let surface = (depth - h) as f32;
                    ray_subtract_below(ray, surface);
                }
            }
        }
    }
}

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
#[allow(clippy::too_many_arguments)]
/// Stamp a tool along a linear segment AND compute metrics in a single pass.
///
/// Fuses the work of `stamp_segment_on_grid`, `estimate_disk_cut_metrics`, and
/// two calls to `window_material_volume_for_segment` into one row/col loop.
///
/// `radial_engagement` is the conventional CAM "radial width of cut over tool
/// diameter" — measured as the perpendicular extent of cells that contained
/// fresh material above the cutter surface BEFORE the stamp. This is
/// independent of sample density: dense overlapping samples in already-cleared
/// territory don't double-count, but a leading-edge bite of stepover σ
/// reads σ/(2R) regardless of how finely we sample the toolpath.
///
/// `arc_engagement_radians` uses the same pre-stamp-fresh-material gate
/// (binned by bearing relative to motion, restricted to the leading half).
/// The chipload formula consumes this value, so the same density-independence
/// matters: dense samples on adaptive cuts must not collapse arc engagement
/// to a thin sliver.
///
/// Returns `(axial_doc_mm, radial_engagement, arc_engagement_radians, volume_removed_mm3)`,
/// where `axial_doc_mm` is the maximum per-cell material length removed by
/// this stamp.
pub(super) fn stamp_segment_with_metrics(
    grid: &mut DexelGrid,
    lut: &RadialProfileLUT,
    radius: f64,
    start: (f64, f64, f64),
    end: (f64, f64, f64),
    mid_u: f64,
    mid_v: f64,
    from_high: bool,
    capture_arc_engagement: bool,
) -> (f64, f64, Option<f64>, f64) {
    let (su, sv, sd) = start;
    let (eu, ev, ed) = end;
    let seg_du = eu - su;
    let seg_dv = ev - sv;
    let seg_dd = ed - sd;
    let seg_len_sq = seg_du * seg_du + seg_dv * seg_dv;

    // Degenerate segment (pure-vertical, e.g. drill plunge): planar offset is
    // zero, so the metric formulas below would divide by zero. Stamp at the
    // segment's bottom and compute volume by measuring the ray-by-ray drop
    // in material height. axial_doc is the Z descent; radial_engagement is
    // 1.0 when material is actually being removed (drill bites full-flute).
    if seg_len_sq < 1e-20 {
        let d = sd.min(ed);
        let descent = (sd - ed).abs();

        let cs = grid.cell_size;
        let cell_area = cs * cs;
        let r_sq = lut.radius_sq();

        let col_min = ((su - radius - grid.origin_u) / cs).floor() as isize;
        let col_max = ((su + radius - grid.origin_u) / cs).ceil() as isize;
        let row_min = ((sv - radius - grid.origin_v) / cs).floor() as isize;
        let row_max = ((sv + radius - grid.origin_v) / cs).ceil() as isize;
        let col_lo = col_min.max(0) as usize;
        let col_hi = (col_max as usize).min(grid.cols.saturating_sub(1));
        let row_lo = row_min.max(0) as usize;
        let row_hi = (row_max as usize).min(grid.rows.saturating_sub(1));

        let mut removed_volume = 0.0_f64;

        for row in row_lo..=row_hi {
            let cell_v = grid.origin_v + row as f64 * cs;
            let dv = cell_v - sv;
            let dv_sq = dv * dv;
            if dv_sq > r_sq {
                continue;
            }
            for col in col_lo..=col_hi {
                let cell_u = grid.origin_u + col as f64 * cs;
                let du = cell_u - su;
                let dist_sq = du * du + dv_sq;
                if let Some(h) = lut.height_at_dist_sq(dist_sq) {
                    let ray = &mut grid.rays[row * grid.cols + col];
                    if from_high {
                        let surface = (d + h) as f32;
                        let above = ray_material_length_above(ray, surface) as f64;
                        ray_subtract_above(ray, surface);
                        removed_volume += above * cell_area;
                    } else {
                        // Mirror: count material below `surface` then subtract.
                        // For an axis flipped this direction, the same area
                        // arithmetic applies.
                        let surface = (d - h) as f32;
                        let total_before = ray_material_length(ray) as f64;
                        ray_subtract_below(ray, surface);
                        let total_after = ray_material_length(ray) as f64;
                        removed_volume += (total_before - total_after) * cell_area;
                    }
                }
            }
        }

        let radial = if removed_volume > 1e-9 { 1.0 } else { 0.0 };
        return (descent, radial, None, removed_volume);
    }

    let inv_seg_len_sq = 1.0 / seg_len_sq;
    let cs = grid.cell_size;
    let cell_area = cs * cs;
    let radius_sq = lut.radius_sq();

    // Bounding box of segment sweep + tool radius (superset of all footprints).
    let u_min = su.min(eu) - radius;
    let u_max = su.max(eu) + radius;
    let v_min = sv.min(ev) - radius;
    let v_max = sv.max(ev) + radius;

    let col_lo = ((u_min - grid.origin_u) / cs).floor().max(0.0) as usize;
    let col_hi = (((u_max - grid.origin_u) / cs).ceil() as usize).min(grid.cols.saturating_sub(1));
    let row_lo = ((v_min - grid.origin_v) / cs).floor().max(0.0) as usize;
    let row_hi = (((v_max - grid.origin_v) / cs).ceil() as usize).min(grid.rows.saturating_sub(1));

    // Metrics accumulators.
    let mut pre_volume = 0.0f64;
    let mut post_volume = 0.0f64;
    let mut max_penetration = 0.0f64;
    // Perpendicular-to-bearing extent of cells that contained fresh material
    // above the cutter surface before the stamp. Used to derive the
    // conventional "radial width of cut / diameter" engagement metric in a
    // way that is independent of sample density.
    let mut perp_min = f64::INFINITY;
    let mut perp_max = f64::NEG_INFINITY;
    let seg_len = seg_len_sq.sqrt();
    let inv_seg_len = if seg_len > 1e-9 { 1.0 / seg_len } else { 0.0 };
    // Threshold for "fresh material exists above the cutter at this cell".
    // 0.05 mm filters out near-flush dexel artifacts where the previous stamp
    // left material fractionally above the cutter surface due to floating
    // point. Real bites are mm-scale.
    const FRESH_MATERIAL_THRESHOLD_MM: f64 = 0.05;

    for row in row_lo..=row_hi {
        let cell_v = grid.origin_v + row as f64 * cs;
        let pv = cell_v - sv;

        for col in col_lo..=col_hi {
            let cell_u = grid.origin_u + col as f64 * cs;
            let pu = cell_u - su;

            // Closest point on segment to this cell center.
            let t = ((pu * seg_du + pv * seg_dv) * inv_seg_len_sq).clamp(0.0, 1.0);
            let closest_u = t * seg_du;
            let closest_v = t * seg_dv;
            let du = pu - closest_u;
            let dv = pv - closest_v;
            let dist_sq = du * du + dv * dv;

            // Only process cells within the tool radius (circular footprint).
            if let Some(h) = lut.height_at_dist_sq(dist_sq) {
                let ray = &mut grid.rays[row * grid.cols + col];

                // 1. Pre-stamp material totals (read before mutation). pre_len
                //    is total material height; pre_fresh is material above the
                //    cutter surface at this cell — i.e. material this stamp
                //    will (or already would have) removed. pre_fresh drives
                //    the radial engagement metric: it is independent of
                //    sample density because dense overlapping samples in
                //    already-cleared territory see pre_fresh ≈ 0.
                let pre_len = ray_material_length(ray) as f64;
                pre_volume += pre_len * cell_area;
                let depth = sd + t * seg_dd;
                let cell_tool_surface = if from_high { depth + h } else { depth - h };
                let above = ray_material_length_above(ray, cell_tool_surface as f32) as f64;
                let pre_fresh = if from_high { above } else { pre_len - above };

                // 2. Apply the stamp using per-cell t-projected depth and
                //    per-cell h. This is the geometry that determines what
                //    material the cutter ACTUALLY removed at this cell.
                if from_high {
                    ray_subtract_above(ray, cell_tool_surface as f32);
                } else {
                    ray_subtract_below(ray, cell_tool_surface as f32);
                }

                // 3. Post-stamp material height (read after mutation).
                let post_len = ray_material_length(ray) as f64;
                post_volume += post_len * cell_area;

                // 4. Engagement metrics. The midpoint disk defines the
                //    reference footprint for both arc binning and the
                //    width-of-cut measurement. radial_engagement and
                //    arc_engagement_radians both gate on pre-stamp fresh
                //    material above the cutter surface (independent of
                //    sample density). max_penetration (axial DOC) still
                //    gates on actual removal — it's the per-cell removed
                //    height, which is well-defined per stamp regardless of
                //    overlap with prior stamps.
                let dm_u = cell_u - mid_u;
                let dm_v = cell_v - mid_v;
                let mid_dist_sq = dm_u * dm_u + dm_v * dm_v;
                if mid_dist_sq <= radius_sq && lut.height_at_dist_sq(mid_dist_sq).is_some() {
                    if pre_fresh > FRESH_MATERIAL_THRESHOLD_MM {
                        // Perpendicular projection (signed) for width-of-cut.
                        let perp = (-seg_dv * dm_u + seg_du * dm_v) * inv_seg_len;
                        if perp < perp_min {
                            perp_min = perp;
                        }
                        if perp > perp_max {
                            perp_max = perp;
                        }
                        // Arc engagement is derived geometrically from the
                        // perp extent below; no per-cell bearing binning is
                        // needed (the per-cell scan suffers from the same
                        // dense-sample lune artifact that motivated the
                        // perp-extent metric in the first place).
                    }
                    let removed_here = (pre_len - post_len).max(0.0);
                    if removed_here > 1e-6 {
                        max_penetration = max_penetration.max(removed_here);
                    }
                }
            }
        }
    }

    // Width of cut perpendicular to motion / tool diameter.
    let radial_engagement = if perp_max > perp_min {
        ((perp_max - perp_min) / (2.0 * radius)).clamp(0.0, 1.0)
    } else {
        0.0
    };
    // Engagement arc derived geometrically from radial engagement. The
    // conventional CAM relationship (one-sided side bite of width W = D ·
    // radial_engagement on a cutter of diameter D): arc = arccos(1 − 2 ·
    // radial). Reads π/2 at half-immersion (RWoC=0.5) and π at full slot
    // (RWoC=1.0) — matching the tooth-load arc convention the chipload
    // formula expects.
    //
    // Computing this from radial rather than per-cell bearing binning
    // avoids the dense-sample lune artifact: a per-cell scan over engaged
    // cells in the midpoint disk only sees the thin sliver of fresh
    // material between consecutive overlapping samples, and its bearing
    // extent is much smaller than the steady-state engagement arc the
    // chipload formula expects. The radial measurement (perp extent of
    // fresh cells) is more robust because the bite zone has nontrivial
    // perp extent even when the sliver is thin.
    let arc_engagement_radians = if capture_arc_engagement {
        let arc = if radial_engagement > 0.0 {
            let one_minus_two_w_over_d = 1.0 - 2.0 * radial_engagement;
            one_minus_two_w_over_d.clamp(-1.0, 1.0).acos()
        } else {
            0.0
        };
        Some(arc.clamp(0.0, std::f64::consts::TAU))
    } else {
        None
    };

    (
        max_penetration.max(0.0),
        radial_engagement,
        arc_engagement_radians,
        (pre_volume - post_volume).max(0.0),
    )
}

/// Bundled parameters for `sample_segment_runtime`.
pub(super) struct SegmentSampleParams {
    pub(super) move_index: usize,
    pub(super) toolpath_id: usize,
    pub(super) sample_step_mm: f64,
    pub(super) feed_rate_mm_min: f64,
    pub(super) is_cutting: bool,
    pub(super) cut_kinematics: CutKinematics,
    pub(super) spindle_rpm: u32,
    pub(super) flute_count: u32,
    pub(super) semantic_item_id: Option<u64>,
}

pub(super) fn sample_segment_runtime(
    start: P3,
    end: P3,
    params: &SegmentSampleParams,
    cumulative_time_s: &mut f64,
    next_sample_index: &mut usize,
    samples: &mut Vec<SimulationCutSample>,
) {
    let segment_length = (end - start).norm();
    if segment_length <= 1e-9 {
        return;
    }

    let subsegments = ((segment_length / params.sample_step_mm.max(1e-3)).ceil() as usize).max(1);
    for subsegment in 0..subsegments {
        let t0 = subsegment as f64 / subsegments as f64;
        let t1 = (subsegment + 1) as f64 / subsegments as f64;
        let seg_start = lerp_point(start, end, t0);
        let seg_end = lerp_point(start, end, t1);
        let midpoint = lerp_point(seg_start, seg_end, 0.5);
        let segment_len = (seg_end - seg_start).norm();
        if segment_len <= 1e-9 {
            continue;
        }
        let segment_time_s = (segment_len / params.feed_rate_mm_min.max(1.0)) * 60.0;
        *cumulative_time_s += segment_time_s;
        samples.push(SimulationCutSample {
            toolpath_id: params.toolpath_id,
            move_index: params.move_index,
            sample_index: *next_sample_index,
            position: [midpoint.x, midpoint.y, midpoint.z],
            cumulative_time_s: *cumulative_time_s,
            segment_time_s,
            is_cutting: params.is_cutting,
            cut_kinematics: params.cut_kinematics,
            feed_rate_mm_min: params.feed_rate_mm_min,
            spindle_rpm: params.spindle_rpm,
            flute_count: params.flute_count,
            axial_doc_mm: 0.0,
            radial_engagement: 0.0,
            arc_engagement_radians: None,
            chipload_mm_per_tooth: 0.0,
            effective_chip_thickness_mm: None,
            removed_volume_est_mm3: 0.0,
            mrr_mm3_s: 0.0,
            semantic_item_id: params.semantic_item_id,
        });
        *next_sample_index += 1;
    }
}

pub(super) fn chipload_mm_per_tooth(
    feed_rate_mm_min: f64,
    spindle_rpm: u32,
    flute_count: u32,
) -> f64 {
    if spindle_rpm == 0 || flute_count == 0 {
        0.0
    } else {
        feed_rate_mm_min / spindle_rpm as f64 / flute_count as f64
    }
}

pub(super) fn lerp_point(start: P3, end: P3, t: f64) -> P3 {
    start + (end - start) * t
}

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
pub(super) fn build_move_semantic_lookup(
    move_count: usize,
    trace: Option<&ToolpathSemanticTrace>,
) -> Vec<Option<u64>> {
    let Some(trace) = trace else {
        return vec![None; move_count];
    };

    let mut item_index_by_id = std::collections::HashMap::with_capacity(trace.items.len());
    for (item_index, item) in trace.items.iter().enumerate() {
        item_index_by_id.insert(item.id, item_index);
    }

    let mut depths = vec![0usize; trace.items.len()];
    for (item_index, item) in trace.items.iter().enumerate() {
        let mut depth = 0usize;
        let mut parent = item.parent_id;
        while let Some(parent_id) = parent {
            depth += 1;
            parent = item_index_by_id
                .get(&parent_id)
                .and_then(|parent_index| trace.items.get(*parent_index))
                .and_then(|parent_item| parent_item.parent_id);
        }
        depths[item_index] = depth;
    }

    let mut lookup = vec![None; move_count];
    let mut best_depth = vec![0usize; move_count];
    let mut best_span = vec![usize::MAX; move_count];

    for (item_index, item) in trace.items.iter().enumerate() {
        let (Some(move_start), Some(move_end)) = (item.move_start, item.move_end) else {
            continue;
        };
        if move_count == 0 || move_start >= move_count {
            continue;
        }
        let last = move_end.min(move_count.saturating_sub(1));
        let span = last.saturating_sub(move_start);
        for move_index in move_start..=last {
            let replace = lookup[move_index].is_none()
                || depths[item_index] > best_depth[move_index]
                || (depths[item_index] == best_depth[move_index] && span < best_span[move_index]);
            if replace {
                lookup[move_index] = Some(item.id);
                best_depth[move_index] = depths[item_index];
                best_span[move_index] = span;
            }
        }
    }

    lookup
}
