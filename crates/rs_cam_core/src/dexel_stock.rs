//! Tri-dexel stock representation with tool stamping and toolpath simulation.
//!
//! Replaces the 2.5-D heightmap for volumetric material removal.  The Z-grid
//! is always present; X and Y grids are created lazily when side-face cuts are
//! needed (future work).

use crate::arc_util::linearize_arc;
use crate::dexel::{
    DexelAxis, DexelGrid, ray_bottom, ray_material_length, ray_subtract_above, ray_subtract_below,
    ray_top,
};
use crate::geo::{BoundingBox3, P3};
use crate::interrupt::{CancelCheck, Cancelled, check_cancel};
use crate::radial_profile::RadialProfileLUT;
use crate::semantic_trace::ToolpathSemanticTrace;
use crate::simulation_cut::SimulationCutSample;
use crate::tool::MillingCutter;
use crate::toolpath::{MoveType, Toolpath};

// ── Cut direction ───────────────────────────────────────────────────────

/// Which side of the stock the tool approaches from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StockCutDirection {
    /// Tool enters from above (+Z) — removes material above the cutter surface (Z-grid).
    FromTop,
    /// Tool enters from below (−Z) — removes material below the cutter surface (Z-grid).
    FromBottom,
    /// Tool enters from the front face (−Y side) — stamps on Y-grid.
    FromFront,
    /// Tool enters from the back face (+Y side) — stamps on Y-grid.
    FromBack,
    /// Tool enters from the left face (−X side) — stamps on X-grid.
    FromLeft,
    /// Tool enters from the right face (+X side) — stamps on X-grid.
    FromRight,
}

impl StockCutDirection {
    /// Which grid axis this direction stamps on.
    pub fn grid_axis(self) -> DexelAxis {
        match self {
            Self::FromTop | Self::FromBottom => DexelAxis::Z,
            Self::FromFront | Self::FromBack => DexelAxis::Y,
            Self::FromLeft | Self::FromRight => DexelAxis::X,
        }
    }

    /// Whether the tool enters from the high side of the ray axis.
    ///
    /// High-side entry removes material via `subtract_above`;
    /// low-side entry removes material via `subtract_below`.
    pub fn cuts_from_high_side(self) -> bool {
        match self {
            Self::FromTop | Self::FromBack | Self::FromRight => true,
            Self::FromBottom | Self::FromFront | Self::FromLeft => false,
        }
    }

    /// Decompose a 3-D point `(x, y, z)` into `(grid_u, grid_v, ray_depth)`
    /// for the grid axis this direction stamps on.
    fn decompose(self, x: f64, y: f64, z: f64) -> (f64, f64, f64) {
        match self.grid_axis() {
            DexelAxis::Z => (x, y, z), // Z-grid: u=X, v=Y, depth=Z
            DexelAxis::Y => (x, z, y), // Y-grid: u=X, v=Z, depth=Y
            DexelAxis::X => (y, z, x), // X-grid: u=Y, v=Z, depth=X
        }
    }
}

// ── TriDexelStock ───────────────────────────────────────────────────────

/// Volumetric stock representation using three orthogonal dexel grids.
///
/// For the common top/bottom workflow only the Z-grid is needed.
pub struct TriDexelStock {
    pub z_grid: DexelGrid,
    pub x_grid: Option<DexelGrid>,
    pub y_grid: Option<DexelGrid>,
    pub stock_bbox: BoundingBox3,
}

impl Clone for TriDexelStock {
    fn clone(&self) -> Self {
        Self {
            z_grid: self.z_grid.clone(),
            x_grid: self.x_grid.clone(),
            y_grid: self.y_grid.clone(),
            stock_bbox: self.stock_bbox,
        }
    }
}

impl TriDexelStock {
    /// Create a stock from a bounding box (Z-grid only).
    pub fn from_bounds(bbox: &BoundingBox3, cell_size: f64) -> Self {
        Self {
            z_grid: DexelGrid::z_grid_from_bounds(bbox, cell_size),
            x_grid: None,
            y_grid: None,
            stock_bbox: *bbox,
        }
    }

    /// Create from explicit stock dimensions (matches `Heightmap::from_stock`).
    pub fn from_stock(
        x_min: f64,
        y_min: f64,
        x_max: f64,
        y_max: f64,
        z_min: f64,
        z_max: f64,
        cell_size: f64,
    ) -> Self {
        let bbox = BoundingBox3 {
            min: P3::new(x_min, y_min, z_min),
            max: P3::new(x_max, y_max, z_max),
        };
        Self::from_bounds(&bbox, cell_size)
    }

    /// Clone the stock state for checkpointing.
    pub fn checkpoint(&self) -> Self {
        self.clone()
    }

    // ── Lazy grid initialization ────────────────────────────────────────

    /// Ensure the grid for `direction` exists, creating it lazily if needed.
    /// Returns a mutable reference to the appropriate grid.
    fn ensure_grid(&mut self, direction: StockCutDirection) -> &mut DexelGrid {
        let bbox = self.stock_bbox;
        let cell_size = self.z_grid.cell_size;
        match direction.grid_axis() {
            DexelAxis::Z => &mut self.z_grid,
            DexelAxis::Y => self
                .y_grid
                .get_or_insert_with(|| DexelGrid::y_grid_from_bounds(&bbox, cell_size)),
            DexelAxis::X => self
                .x_grid
                .get_or_insert_with(|| DexelGrid::x_grid_from_bounds(&bbox, cell_size)),
        }
    }

    // ── Single-position stamp ───────────────────────────────────────────

    /// Stamp a tool at a 3-D position into the grid determined by `direction`.
    ///
    /// The position `(cx, cy, tip_z)` is in global stock coordinates.
    /// For Z-grid (FromTop/FromBottom): the tool footprint is in XY, ray depth is Z.
    /// For Y-grid (FromFront/FromBack): footprint in XZ, depth is Y.
    /// For X-grid (FromLeft/FromRight): footprint in YZ, depth is X.
    pub fn stamp_tool_at(
        &mut self,
        lut: &RadialProfileLUT,
        radius: f64,
        cx: f64,
        cy: f64,
        tip_z: f64,
        direction: StockCutDirection,
    ) {
        let (cu, cv, cd) = direction.decompose(cx, cy, tip_z);
        let from_high = direction.cuts_from_high_side();
        let grid = self.ensure_grid(direction);
        stamp_point_on_grid(grid, lut, radius, cu, cv, cd, from_high);
    }

    // ── Swept linear segment ────────────────────────────────────────────

    /// Stamp the tool along a linear segment from `start` to `end`.
    ///
    /// Uses closest-point-on-segment to find the cutter height at each cell,
    /// matching the existing heightmap `stamp_linear_segment_lut`.
    pub fn stamp_linear_segment(
        &mut self,
        lut: &RadialProfileLUT,
        radius: f64,
        start: P3,
        end: P3,
        direction: StockCutDirection,
    ) {
        let s = direction.decompose(start.x, start.y, start.z);
        let e = direction.decompose(end.x, end.y, end.z);
        let from_high = direction.cuts_from_high_side();
        let grid = self.ensure_grid(direction);
        stamp_segment_on_grid(grid, lut, radius, s, e, from_high);
    }

    // ── Toolpath simulation ─────────────────────────────────────────────

    /// Simulate an entire toolpath into the stock.
    pub fn simulate_toolpath(
        &mut self,
        toolpath: &Toolpath,
        cutter: &dyn MillingCutter,
        direction: StockCutDirection,
    ) {
        let never_cancel = || false;
        let _ = self.simulate_toolpath_with_cancel(toolpath, cutter, direction, &never_cancel);
    }

    /// Simulate with cancellation support.
    pub fn simulate_toolpath_with_cancel(
        &mut self,
        toolpath: &Toolpath,
        cutter: &dyn MillingCutter,
        direction: StockCutDirection,
        cancel: &dyn CancelCheck,
    ) -> Result<(), Cancelled> {
        if toolpath.moves.is_empty() {
            return Ok(());
        }

        let lut = RadialProfileLUT::from_cutter(cutter, 256);
        let radius = cutter.radius();

        for i in 1..toolpath.moves.len() {
            check_cancel(cancel)?;
            let start = toolpath.moves[i - 1].target;
            let end = toolpath.moves[i].target;

            match toolpath.moves[i].move_type {
                MoveType::Rapid => {}
                MoveType::Linear { .. } => {
                    self.stamp_linear_segment(&lut, radius, start, end, direction);
                }
                MoveType::ArcCW { i, j, .. } => {
                    let cs = self.z_grid.cell_size;
                    let points = linearize_arc(start, end, i, j, true, cs);
                    for w in points.windows(2) {
                        check_cancel(cancel)?;
                        self.stamp_linear_segment(&lut, radius, w[0], w[1], direction);
                    }
                }
                MoveType::ArcCCW { i, j, .. } => {
                    let cs = self.z_grid.cell_size;
                    let points = linearize_arc(start, end, i, j, false, cs);
                    for w in points.windows(2) {
                        check_cancel(cancel)?;
                        self.stamp_linear_segment(&lut, radius, w[0], w[1], direction);
                    }
                }
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn simulate_toolpath_with_metrics_with_cancel(
        &mut self,
        toolpath: &Toolpath,
        cutter: &dyn MillingCutter,
        direction: StockCutDirection,
        toolpath_id: usize,
        spindle_rpm: u32,
        flute_count: u32,
        rapid_feed_mm_min: f64,
        sample_step_mm: f64,
        semantic_trace: Option<&ToolpathSemanticTrace>,
        cancel: &dyn CancelCheck,
    ) -> Result<Vec<SimulationCutSample>, Cancelled> {
        if toolpath.moves.len() < 2 {
            return Ok(Vec::new());
        }

        let lut = RadialProfileLUT::from_cutter(cutter, 256);
        let radius = cutter.radius();
        let sample_step_mm = sample_step_mm.max(1e-3);
        let semantic_lookup = build_move_semantic_lookup(toolpath.moves.len(), semantic_trace);

        let mut samples = Vec::new();
        let mut cumulative_time_s = 0.0;
        let mut next_sample_index = 0usize;

        for move_index in 1..toolpath.moves.len() {
            check_cancel(cancel)?;
            let start = toolpath.moves[move_index - 1].target;
            let end = toolpath.moves[move_index].target;
            let semantic_item_id = semantic_lookup.get(move_index).copied().flatten();

            match toolpath.moves[move_index].move_type {
                MoveType::Rapid => {
                    sample_segment_runtime(
                        start,
                        end,
                        &SegmentSampleParams {
                            move_index,
                            toolpath_id,
                            sample_step_mm,
                            feed_rate_mm_min: rapid_feed_mm_min.max(1.0),
                            is_cutting: false,
                            spindle_rpm,
                            flute_count,
                            semantic_item_id,
                        },
                        &mut cumulative_time_s,
                        &mut next_sample_index,
                        &mut samples,
                    );
                }
                MoveType::Linear { feed_rate } => {
                    self.capture_cutting_segment(
                        &lut,
                        radius,
                        start,
                        end,
                        direction,
                        CuttingCaptureParams {
                            toolpath_id,
                            move_index,
                            feed_rate_mm_min: feed_rate,
                            spindle_rpm,
                            flute_count,
                            semantic_item_id,
                            sample_step_mm,
                        },
                        cancel,
                        &mut cumulative_time_s,
                        &mut next_sample_index,
                        &mut samples,
                    )?;
                }
                MoveType::ArcCW { i, j, feed_rate } => {
                    let points = linearize_arc(start, end, i, j, true, self.z_grid.cell_size);
                    for window in points.windows(2) {
                        check_cancel(cancel)?;
                        self.capture_cutting_segment(
                            &lut,
                            radius,
                            window[0],
                            window[1],
                            direction,
                            CuttingCaptureParams {
                                toolpath_id,
                                move_index,
                                feed_rate_mm_min: feed_rate,
                                spindle_rpm,
                                flute_count,
                                semantic_item_id,
                                sample_step_mm,
                            },
                            cancel,
                            &mut cumulative_time_s,
                            &mut next_sample_index,
                            &mut samples,
                        )?;
                    }
                }
                MoveType::ArcCCW { i, j, feed_rate } => {
                    let points = linearize_arc(start, end, i, j, false, self.z_grid.cell_size);
                    for window in points.windows(2) {
                        check_cancel(cancel)?;
                        self.capture_cutting_segment(
                            &lut,
                            radius,
                            window[0],
                            window[1],
                            direction,
                            CuttingCaptureParams {
                                toolpath_id,
                                move_index,
                                feed_rate_mm_min: feed_rate,
                                spindle_rpm,
                                flute_count,
                                semantic_item_id,
                                sample_step_mm,
                            },
                            cancel,
                            &mut cumulative_time_s,
                            &mut next_sample_index,
                            &mut samples,
                        )?;
                    }
                }
            }
        }

        Ok(samples)
    }

    /// Simulate only moves `start_move..end_move` (for incremental playback).
    pub fn simulate_toolpath_range(
        &mut self,
        toolpath: &Toolpath,
        cutter: &dyn MillingCutter,
        direction: StockCutDirection,
        start_move: usize,
        end_move: usize,
    ) {
        if toolpath.moves.len() < 2 {
            return;
        }
        let lut = RadialProfileLUT::from_cutter(cutter, 256);
        let radius = cutter.radius();
        let first = start_move.max(1);
        let last = end_move.min(toolpath.moves.len());

        for i in first..last {
            let start = toolpath.moves[i - 1].target;
            let end = toolpath.moves[i].target;

            match toolpath.moves[i].move_type {
                MoveType::Rapid => {}
                MoveType::Linear { .. } => {
                    self.stamp_linear_segment(&lut, radius, start, end, direction);
                }
                MoveType::ArcCW { i, j, .. } => {
                    let cs = self.z_grid.cell_size;
                    let points = linearize_arc(start, end, i, j, true, cs);
                    for w in points.windows(2) {
                        self.stamp_linear_segment(&lut, radius, w[0], w[1], direction);
                    }
                }
                MoveType::ArcCCW { i, j, .. } => {
                    let cs = self.z_grid.cell_size;
                    let points = linearize_arc(start, end, i, j, false, cs);
                    for w in points.windows(2) {
                        self.stamp_linear_segment(&lut, radius, w[0], w[1], direction);
                    }
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn capture_cutting_segment(
        &mut self,
        lut: &RadialProfileLUT,
        radius: f64,
        start: P3,
        end: P3,
        direction: StockCutDirection,
        params: CuttingCaptureParams,
        cancel: &dyn CancelCheck,
        cumulative_time_s: &mut f64,
        next_sample_index: &mut usize,
        samples: &mut Vec<SimulationCutSample>,
    ) -> Result<(), Cancelled> {
        let segment_length = (end - start).norm();
        if segment_length <= 1e-9 {
            return Ok(());
        }

        let subsegments = ((segment_length / params.sample_step_mm).ceil() as usize).max(1);
        for subsegment in 0..subsegments {
            check_cancel(cancel)?;
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
            let (axial_doc_mm, radial_engagement, removed_volume_est_mm3) = self
                .estimate_and_stamp_cutting_subsegment(
                    lut, radius, seg_start, seg_end, midpoint, direction,
                );

            *cumulative_time_s += segment_time_s;
            samples.push(SimulationCutSample {
                toolpath_id: params.toolpath_id,
                move_index: params.move_index,
                sample_index: *next_sample_index,
                position: [midpoint.x, midpoint.y, midpoint.z],
                cumulative_time_s: *cumulative_time_s,
                segment_time_s,
                is_cutting: true,
                feed_rate_mm_min: params.feed_rate_mm_min,
                spindle_rpm: params.spindle_rpm,
                flute_count: params.flute_count,
                axial_doc_mm,
                radial_engagement,
                chipload_mm_per_tooth: chipload_mm_per_tooth(
                    params.feed_rate_mm_min,
                    params.spindle_rpm,
                    params.flute_count,
                ),
                removed_volume_est_mm3,
                mrr_mm3_s: if segment_time_s <= 1e-9 {
                    0.0
                } else {
                    removed_volume_est_mm3 / segment_time_s
                },
                semantic_item_id: params.semantic_item_id,
            });
            *next_sample_index += 1;
        }
        Ok(())
    }

    fn estimate_and_stamp_cutting_subsegment(
        &mut self,
        lut: &RadialProfileLUT,
        radius: f64,
        seg_start: P3,
        seg_end: P3,
        midpoint: P3,
        direction: StockCutDirection,
    ) -> (f64, f64, f64) {
        let (su, sv, sd) = direction.decompose(seg_start.x, seg_start.y, seg_start.z);
        let (eu, ev, ed) = direction.decompose(seg_end.x, seg_end.y, seg_end.z);
        let (mu, mv, md) = direction.decompose(midpoint.x, midpoint.y, midpoint.z);
        let from_high = direction.cuts_from_high_side();
        let grid = self.ensure_grid(direction);
        let (axial_doc_mm, radial_engagement) =
            estimate_disk_cut_metrics(grid, lut, radius, mu, mv, md, from_high);
        let pre_volume = window_material_volume_for_segment(grid, radius, su, sv, eu, ev);
        stamp_segment_on_grid(grid, lut, radius, (su, sv, sd), (eu, ev, ed), from_high);
        let post_volume = window_material_volume_for_segment(grid, radius, su, sv, eu, ev);
        (
            axial_doc_mm,
            radial_engagement,
            (pre_volume - post_volume).max(0.0),
        )
    }

    /// Sum of material top-Z values in a circular window around (cx, cy).
    ///
    /// Iterates all Z-grid cells within `radius` of (cx, cy) and sums their
    /// top-Z values (or 0.0 if the ray is empty). This is the tri-dexel
    /// equivalent of adaptive3d's `local_material_sum()` which sums heightmap
    /// cell values in a local radius for engagement tracking.
    pub fn local_material_sum(&self, cx: f64, cy: f64, radius: f64) -> f64 {
        let grid = &self.z_grid;
        let cs = grid.cell_size;
        let r_cells = (radius / cs).ceil() as isize;

        // Convert world (cx, cy) to grid cell
        let center_col = ((cx - grid.origin_u) / cs).round() as isize;
        let center_row = ((cy - grid.origin_v) / cs).round() as isize;

        let col_min = (center_col - r_cells).max(0) as usize;
        let col_max = ((center_col + r_cells) as usize).min(grid.cols.saturating_sub(1));
        let row_min = (center_row - r_cells).max(0) as usize;
        let row_max = ((center_row + r_cells) as usize).min(grid.rows.saturating_sub(1));

        let r_sq = radius * radius;
        let mut sum = 0.0;

        for row in row_min..=row_max {
            let cell_y = grid.origin_v + row as f64 * cs;
            let dy = cell_y - cy;
            let dy_sq = dy * dy;
            if dy_sq > r_sq {
                continue;
            }
            for col in col_min..=col_max {
                let cell_x = grid.origin_u + col as f64 * cs;
                let dx = cell_x - cx;
                let dist_sq = dx * dx + dy_sq;
                if dist_sq > r_sq {
                    continue;
                }
                if let Some(top) = grid.top_z_at(row, col) {
                    sum += top as f64;
                }
            }
        }
        sum
    }

    /// Clear all material above `z` at the given cell on the Z-grid.
    ///
    /// After this call, no material exists above `z` at (row, col).
    /// Used for border clearing in adaptive3d where cells outside the mesh
    /// footprint are set to the surface height.
    pub fn clear_above_at(&mut self, row: usize, col: usize, z: f32) {
        let ray = &mut self.z_grid.rays[row * self.z_grid.cols + col];
        crate::dexel::ray_subtract_above(ray, z);
    }
}

#[derive(Clone, Copy)]
struct CuttingCaptureParams {
    toolpath_id: usize,
    move_index: usize,
    feed_rate_mm_min: f64,
    spindle_rpm: u32,
    flute_count: u32,
    semantic_item_id: Option<u64>,
    sample_step_mm: f64,
}

// ── Grid-generic stamp helpers ───────────────────────────────────────────

/// Stamp a tool at a single position on a grid (axis-agnostic).
///
/// `(cu, cv)` is the tool center in the grid's planar axes.
/// `tip_depth` is the tool tip coordinate along the grid's ray axis.
/// `from_high` selects `subtract_above` (true) or `subtract_below` (false).
fn stamp_point_on_grid(
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
                let surface = (tip_depth + h) as f32;
                let ray = &mut grid.rays[row * grid.cols + col];
                if from_high {
                    ray_subtract_above(ray, surface);
                } else {
                    ray_subtract_below(ray, surface);
                }
            }
        }
    }
}

/// Stamp a tool along a linear segment on a grid (axis-agnostic).
///
/// `start` and `end` are `(u, v, depth)` — the segment endpoints decomposed
/// into the grid's planar axes (u, v) and ray-depth axis (depth).
#[allow(clippy::too_many_arguments)]
fn stamp_segment_on_grid(
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
                let surface = (depth + h) as f32;
                let ray = &mut grid.rays[row * grid.cols + col];
                if from_high {
                    ray_subtract_above(ray, surface);
                } else {
                    ray_subtract_below(ray, surface);
                }
            }
        }
    }
}

/// Bundled parameters for `sample_segment_runtime`.
struct SegmentSampleParams {
    move_index: usize,
    toolpath_id: usize,
    sample_step_mm: f64,
    feed_rate_mm_min: f64,
    is_cutting: bool,
    spindle_rpm: u32,
    flute_count: u32,
    semantic_item_id: Option<u64>,
}

fn sample_segment_runtime(
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
            feed_rate_mm_min: params.feed_rate_mm_min,
            spindle_rpm: params.spindle_rpm,
            flute_count: params.flute_count,
            axial_doc_mm: 0.0,
            radial_engagement: 0.0,
            chipload_mm_per_tooth: 0.0,
            removed_volume_est_mm3: 0.0,
            mrr_mm3_s: 0.0,
            semantic_item_id: params.semantic_item_id,
        });
        *next_sample_index += 1;
    }
}

fn estimate_disk_cut_metrics(
    grid: &DexelGrid,
    lut: &RadialProfileLUT,
    radius: f64,
    center_u: f64,
    center_v: f64,
    tip_depth: f64,
    from_high: bool,
) -> (f64, f64) {
    let cs = grid.cell_size;
    let col_min = ((center_u - radius - grid.origin_u) / cs).floor() as isize;
    let col_max = ((center_u + radius - grid.origin_u) / cs).ceil() as isize;
    let row_min = ((center_v - radius - grid.origin_v) / cs).floor() as isize;
    let row_max = ((center_v + radius - grid.origin_v) / cs).ceil() as isize;

    let col_lo = col_min.max(0) as usize;
    let col_hi = (col_max.max(col_min) as usize).min(grid.cols.saturating_sub(1));
    let row_lo = row_min.max(0) as usize;
    let row_hi = (row_max.max(row_min) as usize).min(grid.rows.saturating_sub(1));
    if row_lo > row_hi || col_lo > col_hi {
        return (0.0, 0.0);
    }

    let cell_area = cs * cs;
    let mut engaged_area = 0.0f64;
    let mut total_area = 0.0f64;
    let mut max_penetration = 0.0f64;
    let radius_sq = lut.radius_sq();

    for row in row_lo..=row_hi {
        let cell_v = grid.origin_v + row as f64 * cs;
        let dv = cell_v - center_v;
        let dv_sq = dv * dv;
        if dv_sq > radius_sq {
            continue;
        }
        for col in col_lo..=col_hi {
            let cell_u = grid.origin_u + col as f64 * cs;
            let du = cell_u - center_u;
            let dist_sq = du * du + dv_sq;
            if let Some(height) = lut.height_at_dist_sq(dist_sq) {
                total_area += cell_area;
                let tool_surface = tip_depth + height;
                let ray = grid.ray(row, col);
                let penetration = if from_high {
                    ray_top(ray)
                        .map(|top| (top as f64 - tool_surface).max(0.0))
                        .unwrap_or(0.0)
                } else {
                    ray_bottom(ray)
                        .map(|bottom| (tool_surface - bottom as f64).max(0.0))
                        .unwrap_or(0.0)
                };
                if penetration > 1e-6 {
                    engaged_area += cell_area;
                    max_penetration = max_penetration.max(penetration);
                }
            }
        }
    }

    let radial_engagement = if total_area <= 1e-9 {
        0.0
    } else {
        (engaged_area / total_area).clamp(0.0, 1.0)
    };
    (max_penetration.max(0.0), radial_engagement)
}

fn window_material_volume_for_segment(
    grid: &DexelGrid,
    radius: f64,
    start_u: f64,
    start_v: f64,
    end_u: f64,
    end_v: f64,
) -> f64 {
    let cs = grid.cell_size;
    let u_min = start_u.min(end_u) - radius;
    let u_max = start_u.max(end_u) + radius;
    let v_min = start_v.min(end_v) - radius;
    let v_max = start_v.max(end_v) + radius;
    let col_lo = ((u_min - grid.origin_u) / cs).floor().max(0.0) as usize;
    let col_hi = (((u_max - grid.origin_u) / cs).ceil() as usize).min(grid.cols.saturating_sub(1));
    let row_lo = ((v_min - grid.origin_v) / cs).floor().max(0.0) as usize;
    let row_hi = (((v_max - grid.origin_v) / cs).ceil() as usize).min(grid.rows.saturating_sub(1));
    if row_lo > row_hi || col_lo > col_hi {
        return 0.0;
    }

    let cell_area = cs * cs;
    let mut volume = 0.0;
    for row in row_lo..=row_hi {
        for col in col_lo..=col_hi {
            volume += ray_material_length(grid.ray(row, col)) as f64 * cell_area;
        }
    }
    volume
}

fn chipload_mm_per_tooth(feed_rate_mm_min: f64, spindle_rpm: u32, flute_count: u32) -> f64 {
    if spindle_rpm == 0 || flute_count == 0 {
        0.0
    } else {
        feed_rate_mm_min / spindle_rpm as f64 / flute_count as f64
    }
}

fn lerp_point(start: P3, end: P3, t: f64) -> P3 {
    start + (end - start) * t
}

fn build_move_semantic_lookup(
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

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::dexel::{ray_bottom, ray_top};
    use crate::radial_profile::RadialProfileLUT;
    use crate::tool::{BallEndmill, FlatEndmill};

    /// Helper: create a TriDexelStock with the given dimensions.
    fn make_stock(
        x_min: f64,
        y_min: f64,
        x_max: f64,
        y_max: f64,
        z_min: f64,
        z_max: f64,
        cell_size: f64,
    ) -> TriDexelStock {
        TriDexelStock::from_stock(x_min, y_min, x_max, y_max, z_min, z_max, cell_size)
    }

    // ── Basic construction ──────────────────────────────────────────────

    #[test]
    fn from_bounds_dimensions_correct() {
        let bbox = BoundingBox3 {
            min: P3::new(0.0, 0.0, -5.0),
            max: P3::new(10.0, 10.0, 5.0),
        };
        let stock = TriDexelStock::from_bounds(&bbox, 1.0);
        // 10mm / 1mm cell + 1 = 11 rows and cols
        assert_eq!(stock.z_grid.rows, 11);
        assert_eq!(stock.z_grid.cols, 11);
    }

    // ── Single stamp equivalence ────────────────────────────────────────

    #[test]
    fn stamp_flat_endmill_cuts_correctly() {
        let tool = FlatEndmill::new(10.0, 25.0); // radius 5
        let mut stock = make_stock(-10.0, -10.0, 10.0, 10.0, 0.0, 5.0, 0.5);

        let lut = RadialProfileLUT::from_cutter(&tool, 256);
        stock.stamp_tool_at(
            &lut,
            tool.radius(),
            0.0,
            0.0,
            2.0,
            StockCutDirection::FromTop,
        );

        // Center cell: flat endmill tip at z=2, so top should be 2.0.
        let (cr, cc) = stock.z_grid.world_to_cell(0.0, 0.0).unwrap();
        let center_z = ray_top(stock.z_grid.ray(cr, cc)).unwrap() as f64;
        assert!((center_z - 2.0).abs() < 0.01, "center z={center_z:.4}");

        // Cell outside tool radius: should still be at stock top (5.0).
        let (or, oc) = stock.z_grid.world_to_cell(-8.0, -8.0).unwrap();
        let outer_z = ray_top(stock.z_grid.ray(or, oc)).unwrap() as f64;
        assert!((outer_z - 5.0).abs() < 0.01, "outer z={outer_z:.4}");
    }

    #[test]
    fn stamp_ball_endmill_cuts_correctly() {
        let tool = BallEndmill::new(6.0, 25.0); // radius 3
        let mut stock = make_stock(-10.0, -10.0, 10.0, 10.0, 0.0, 5.0, 0.5);

        let lut = RadialProfileLUT::from_cutter(&tool, 256);
        stock.stamp_tool_at(
            &lut,
            tool.radius(),
            0.0,
            0.0,
            1.0,
            StockCutDirection::FromTop,
        );

        // Center cell: ball tip at z=1, so top should be 1.0.
        let (cr, cc) = stock.z_grid.world_to_cell(0.0, 0.0).unwrap();
        let center_z = ray_top(stock.z_grid.ray(cr, cc)).unwrap() as f64;
        assert!((center_z - 1.0).abs() < 0.02, "center z={center_z:.4}");

        // Cell at tool radius edge (3mm away): ball profile rises to tip_z + r = 4.0,
        // but clipped to stock top 5.0, so should be near 4.0.
        let (er, ec) = stock.z_grid.world_to_cell(3.0, 0.0).unwrap();
        let edge_z = ray_top(stock.z_grid.ray(er, ec)).unwrap() as f64;
        assert!(edge_z > 3.5 && edge_z <= 5.0, "edge z={edge_z:.4}");

        // Cell outside tool radius: still at stock top.
        let (or, oc) = stock.z_grid.world_to_cell(-8.0, -8.0).unwrap();
        let outer_z = ray_top(stock.z_grid.ray(or, oc)).unwrap() as f64;
        assert!((outer_z - 5.0).abs() < 0.01, "outer z={outer_z:.4}");
    }

    // ── Linear segment equivalence ──────────────────────────────────────

    #[test]
    fn linear_segment_flat_cuts_correctly() {
        let tool = FlatEndmill::new(4.0, 20.0); // radius 2
        let mut stock = make_stock(-5.0, -5.0, 15.0, 5.0, 0.0, 5.0, 0.5);

        let start = P3::new(0.0, 0.0, 2.0);
        let end = P3::new(10.0, 0.0, 2.0);

        let lut = RadialProfileLUT::from_cutter(&tool, 256);
        stock.stamp_linear_segment(&lut, tool.radius(), start, end, StockCutDirection::FromTop);

        // Along the path center (y=0): z should be at tip_z = 2.0.
        for x in [0.0, 5.0, 10.0] {
            let (r, c) = stock.z_grid.world_to_cell(x, 0.0).unwrap();
            let z = ray_top(stock.z_grid.ray(r, c)).unwrap() as f64;
            assert!((z - 2.0).abs() < 0.02, "x={x} z={z:.4}");
        }

        // Outside tool radius (y=4): should still be stock top (5.0).
        let (r, c) = stock.z_grid.world_to_cell(5.0, 4.0).unwrap();
        let z = ray_top(stock.z_grid.ray(r, c)).unwrap() as f64;
        assert!((z - 5.0).abs() < 0.01, "outside z={z:.4}");
    }

    #[test]
    fn linear_segment_ball_diagonal_cuts_correctly() {
        let tool = BallEndmill::new(6.0, 25.0); // radius 3
        let mut stock = make_stock(0.0, 0.0, 30.0, 30.0, -5.0, 5.0, 0.25);

        let start = P3::new(5.0, 5.0, -1.0);
        let end = P3::new(25.0, 25.0, -1.0);

        let lut = RadialProfileLUT::from_cutter(&tool, 256);
        stock.stamp_linear_segment(&lut, tool.radius(), start, end, StockCutDirection::FromTop);

        // Midpoint of the diagonal (15,15): ball tip at z=-1, so center z = -1.0.
        let (r, c) = stock.z_grid.world_to_cell(15.0, 15.0).unwrap();
        let z = ray_top(stock.z_grid.ray(r, c)).unwrap() as f64;
        assert!((z - (-1.0)).abs() < 0.02, "midpoint z={z:.4}");

        // Far from the path (0,0): should still be stock top (5.0).
        let (r, c) = stock.z_grid.world_to_cell(0.0, 0.0).unwrap();
        let z = ray_top(stock.z_grid.ray(r, c)).unwrap() as f64;
        assert!((z - 5.0).abs() < 0.01, "corner z={z:.4}");
    }

    // ── Toolpath simulation equivalence ─────────────────────────────────

    #[test]
    fn simulate_toolpath_cuts_correctly() {
        let tool = FlatEndmill::new(4.0, 20.0); // radius 2
        let mut stock = make_stock(-5.0, -5.0, 15.0, 5.0, -5.0, 0.0, 0.5);

        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 10.0));
        tp.feed_to(P3::new(0.0, 0.0, -3.0), 500.0);
        tp.feed_to(P3::new(10.0, 0.0, -3.0), 1000.0);

        stock.simulate_toolpath(&tp, &tool, StockCutDirection::FromTop);

        // Along the path (y=0): plunge to z=-3, then cut at z=-3 to x=10.
        for x in [0.0, 5.0, 10.0] {
            let (r, c) = stock.z_grid.world_to_cell(x, 0.0).unwrap();
            let z = ray_top(stock.z_grid.ray(r, c)).unwrap() as f64;
            assert!((z - (-3.0)).abs() < 0.02, "x={x} z={z:.4}");
        }

        // Far from the path: should still be stock top (0.0).
        let (r, c) = stock.z_grid.world_to_cell(-4.0, -4.0).unwrap();
        let z = ray_top(stock.z_grid.ray(r, c)).unwrap() as f64;
        assert!((z - 0.0).abs() < 0.01, "outside z={z:.4}");
    }

    // ── Bottom cuts ─────────────────────────────────────────────────────

    #[test]
    fn bottom_cut_removes_from_below() {
        let tool = FlatEndmill::new(10.0, 25.0);
        let mut stock = TriDexelStock::from_stock(-10.0, -10.0, 10.0, 10.0, 0.0, 10.0, 0.5);

        let lut = RadialProfileLUT::from_cutter(&tool, 256);
        // Tip at z=3 from below: flat endmill surface at z=3, remove below.
        stock.stamp_tool_at(
            &lut,
            tool.radius(),
            0.0,
            0.0,
            3.0,
            StockCutDirection::FromBottom,
        );

        let ray = stock.z_grid.ray(
            stock.z_grid.world_to_cell(0.0, 0.0).unwrap().0,
            stock.z_grid.world_to_cell(0.0, 0.0).unwrap().1,
        );
        // Bottom should now be at 3.0, top still at 10.0.
        assert!((ray_bottom(ray).unwrap() - 3.0).abs() < 0.01);
        assert!((ray_top(ray).unwrap() - 10.0).abs() < 0.01);
    }

    #[test]
    fn top_then_bottom_cut() {
        let tool = FlatEndmill::new(10.0, 25.0);
        let mut stock = TriDexelStock::from_stock(-10.0, -10.0, 10.0, 10.0, 0.0, 10.0, 0.5);

        let lut = RadialProfileLUT::from_cutter(&tool, 256);
        // Top cut: remove above z=7
        stock.stamp_tool_at(
            &lut,
            tool.radius(),
            0.0,
            0.0,
            7.0,
            StockCutDirection::FromTop,
        );
        // Bottom cut: remove below z=3
        stock.stamp_tool_at(
            &lut,
            tool.radius(),
            0.0,
            0.0,
            3.0,
            StockCutDirection::FromBottom,
        );

        let (r, c) = stock.z_grid.world_to_cell(0.0, 0.0).unwrap();
        let ray = stock.z_grid.ray(r, c);
        assert!((ray_bottom(ray).unwrap() - 3.0).abs() < 0.01);
        assert!((ray_top(ray).unwrap() - 7.0).abs() < 0.01);
    }

    // ── Rapids don't cut ────────────────────────────────────────────────

    #[test]
    fn rapids_dont_cut() {
        let tool = FlatEndmill::new(10.0, 25.0);
        let mut stock = TriDexelStock::from_stock(-10.0, -10.0, 10.0, 10.0, 0.0, 5.0, 1.0);

        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 10.0));
        tp.rapid_to(P3::new(5.0, 5.0, 0.0));

        stock.simulate_toolpath(&tp, &tool, StockCutDirection::FromTop);

        // All rays should still have full stock.
        for ray in &stock.z_grid.rays {
            assert_eq!(ray.len(), 1);
            assert!((ray[0].exit - 5.0).abs() < 1e-4);
        }
    }

    // ── Range simulation ────────────────────────────────────────────────

    #[test]
    fn simulate_range_partial() {
        let tool = FlatEndmill::new(4.0, 20.0);
        let mut stock = TriDexelStock::from_stock(-5.0, -5.0, 15.0, 5.0, -5.0, 0.0, 0.5);

        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 10.0));
        tp.feed_to(P3::new(0.0, 0.0, -3.0), 500.0);
        tp.feed_to(P3::new(5.0, 0.0, -3.0), 1000.0);
        tp.feed_to(P3::new(10.0, 0.0, -3.0), 1000.0);

        // Simulate only the first two cutting moves.
        stock.simulate_toolpath_range(&tp, &tool, StockCutDirection::FromTop, 0, 3);

        // x=2.5 should be cut (in first segment)
        let (r, c) = stock.z_grid.world_to_cell(2.5, 0.0).unwrap();
        assert!(ray_top(stock.z_grid.ray(r, c)).unwrap() < 0.0);

        // x=7.5 should be uncut (in third segment, not simulated)
        let (r, c) = stock.z_grid.world_to_cell(7.5, 0.0).unwrap();
        assert!((ray_top(stock.z_grid.ray(r, c)).unwrap() - 0.0).abs() < 1e-4);
    }

    // ── Checkpoint ──────────────────────────────────────────────────────

    #[test]
    fn checkpoint_is_independent_copy() {
        let tool = FlatEndmill::new(10.0, 25.0);
        let mut stock = TriDexelStock::from_stock(-10.0, -10.0, 10.0, 10.0, 0.0, 5.0, 0.5);

        let saved = stock.checkpoint();

        // Cut the original.
        let lut = RadialProfileLUT::from_cutter(&tool, 256);
        stock.stamp_tool_at(
            &lut,
            tool.radius(),
            0.0,
            0.0,
            2.0,
            StockCutDirection::FromTop,
        );

        // Saved should still be at stock top.
        let (r, c) = saved.z_grid.world_to_cell(0.0, 0.0).unwrap();
        assert!((ray_top(saved.z_grid.ray(r, c)).unwrap() - 5.0).abs() < 1e-4);
    }

    // ── Phase 6: Side-face grid tests ──────────────────────────────────

    #[test]
    fn stamp_from_back_creates_y_grid_and_cuts() {
        let tool = FlatEndmill::new(10.0, 25.0); // radius 5
        let mut stock = TriDexelStock::from_stock(0.0, 0.0, 20.0, 20.0, 0.0, 20.0, 1.0);

        assert!(stock.y_grid.is_none(), "Y-grid should not exist yet");

        let lut = RadialProfileLUT::from_cutter(&tool, 256);
        // Stamp from back (+Y side): tool center at global (10, ?, 10)
        // decompose for Y-grid: u=x=10, v=z=10, depth=y
        // FromBack = subtract_above (high-Y side), tip_y = 15
        stock.stamp_tool_at(
            &lut,
            tool.radius(),
            10.0,
            15.0, // global Y: tool tip Y
            10.0, // global Z
            StockCutDirection::FromBack,
        );

        assert!(stock.y_grid.is_some(), "Y-grid should be lazily created");
        let y_grid = stock.y_grid.as_ref().unwrap();

        // Y-grid: u=X, v=Z. Cell at (x=10, z=10) should be shortened from above.
        let (row, col) = y_grid.world_to_cell(10.0, 10.0).unwrap();
        let ray = y_grid.ray(row, col);
        // Original ray: [0, 20]. After subtract_above at y=15+0 (flat endmill h=0 at center),
        // ray should be [0, 15].
        assert!(
            ray_top(ray).unwrap() < 20.0,
            "Y-grid ray should be shortened"
        );
        assert!((ray_top(ray).unwrap() - 15.0).abs() < 0.1);

        // Z-grid should be untouched.
        let (zr, zc) = stock.z_grid.world_to_cell(10.0, 10.0).unwrap();
        assert!((ray_top(stock.z_grid.ray(zr, zc)).unwrap() - 20.0).abs() < 0.01);
    }

    #[test]
    fn stamp_from_front_creates_y_grid_and_cuts() {
        let tool = FlatEndmill::new(10.0, 25.0);
        let mut stock = TriDexelStock::from_stock(0.0, 0.0, 20.0, 20.0, 0.0, 20.0, 1.0);

        let lut = RadialProfileLUT::from_cutter(&tool, 256);
        // FromFront: tool enters from -Y (low Y). subtract_below.
        // Tool tip at global y=5, center at (10, 5, 10).
        stock.stamp_tool_at(
            &lut,
            tool.radius(),
            10.0,
            5.0,  // global Y: tool tip
            10.0, // global Z
            StockCutDirection::FromFront,
        );

        let y_grid = stock.y_grid.as_ref().unwrap();
        let (row, col) = y_grid.world_to_cell(10.0, 10.0).unwrap();
        let ray = y_grid.ray(row, col);
        // subtract_below at y=5: ray bottom should be at 5.
        assert!((ray_bottom(ray).unwrap() - 5.0).abs() < 0.1);
        assert!((ray_top(ray).unwrap() - 20.0).abs() < 0.01); // top unchanged
    }

    #[test]
    fn stamp_from_left_creates_x_grid_and_cuts() {
        let tool = FlatEndmill::new(10.0, 25.0);
        let mut stock = TriDexelStock::from_stock(0.0, 0.0, 20.0, 20.0, 0.0, 20.0, 1.0);

        assert!(stock.x_grid.is_none());

        let lut = RadialProfileLUT::from_cutter(&tool, 256);
        // FromLeft: tool enters from -X. subtract_below on X-grid.
        // decompose: u=Y, v=Z, depth=X. Tool at global (5, 10, 10).
        stock.stamp_tool_at(
            &lut,
            tool.radius(),
            5.0,  // global X: tool tip
            10.0, // global Y
            10.0, // global Z
            StockCutDirection::FromLeft,
        );

        assert!(stock.x_grid.is_some(), "X-grid should be lazily created");
        let x_grid = stock.x_grid.as_ref().unwrap();

        // X-grid: u=Y, v=Z. Cell at (y=10, z=10).
        let (row, col) = x_grid.world_to_cell(10.0, 10.0).unwrap();
        let ray = x_grid.ray(row, col);
        // subtract_below at x=5: ray bottom at 5, top at 20.
        assert!((ray_bottom(ray).unwrap() - 5.0).abs() < 0.1);
        assert!((ray_top(ray).unwrap() - 20.0).abs() < 0.01);
    }

    #[test]
    fn stamp_from_right_creates_x_grid_and_cuts() {
        let tool = FlatEndmill::new(10.0, 25.0);
        let mut stock = TriDexelStock::from_stock(0.0, 0.0, 20.0, 20.0, 0.0, 20.0, 1.0);

        let lut = RadialProfileLUT::from_cutter(&tool, 256);
        // FromRight: tool enters from +X. subtract_above on X-grid.
        stock.stamp_tool_at(
            &lut,
            tool.radius(),
            15.0, // global X: tool tip
            10.0, // global Y
            10.0, // global Z
            StockCutDirection::FromRight,
        );

        let x_grid = stock.x_grid.as_ref().unwrap();
        let (row, col) = x_grid.world_to_cell(10.0, 10.0).unwrap();
        let ray = x_grid.ray(row, col);
        // subtract_above at x=15: ray top at 15, bottom at 0.
        assert!((ray_top(ray).unwrap() - 15.0).abs() < 0.1);
        assert!((ray_bottom(ray).unwrap() - 0.0).abs() < 0.01);
    }

    #[test]
    fn linear_segment_on_y_grid() {
        let tool = FlatEndmill::new(4.0, 20.0); // radius 2
        let mut stock = TriDexelStock::from_stock(0.0, 0.0, 20.0, 20.0, 0.0, 20.0, 0.5);

        let lut = RadialProfileLUT::from_cutter(&tool, 256);
        // Sweep along X at global (x, y=15, z=10) from x=2 to x=18.
        // FromBack stamps on Y-grid. decompose: u=x, v=z, depth=y
        let start = P3::new(2.0, 15.0, 10.0);
        let end = P3::new(18.0, 15.0, 10.0);
        stock.stamp_linear_segment(&lut, tool.radius(), start, end, StockCutDirection::FromBack);

        let y_grid = stock.y_grid.as_ref().unwrap();
        // Check a cell along the swept path: (x=10, z=10).
        let (row, col) = y_grid.world_to_cell(10.0, 10.0).unwrap();
        let ray = y_grid.ray(row, col);
        assert!(ray_top(ray).unwrap() < 20.0, "Y-grid ray should be cut");
        assert!((ray_top(ray).unwrap() - 15.0).abs() < 0.1);
    }

    #[test]
    fn multi_grid_simulation_preserves_z_grid() {
        // Simulate a Top setup, then a Front setup.
        // The Z-grid cuts from setup 1 should be unaffected by setup 2.
        let tool = FlatEndmill::new(6.0, 20.0); // radius 3

        let mut stock = TriDexelStock::from_stock(0.0, 0.0, 30.0, 30.0, 0.0, 20.0, 1.0);

        // Setup 1: Top cut — stamp at center.
        let lut = RadialProfileLUT::from_cutter(&tool, 256);
        stock.stamp_tool_at(
            &lut,
            tool.radius(),
            15.0,
            15.0,
            12.0, // cut Z-grid to z=12
            StockCutDirection::FromTop,
        );

        let (zr, zc) = stock.z_grid.world_to_cell(15.0, 15.0).unwrap();
        let z_top_before = ray_top(stock.z_grid.ray(zr, zc)).unwrap();
        assert!((z_top_before - 12.0).abs() < 0.1);

        // Setup 2: FromBack cut — stamp on Y-grid.
        stock.stamp_tool_at(
            &lut,
            tool.radius(),
            15.0,
            25.0, // global Y (depth for Y-grid)
            10.0, // global Z
            StockCutDirection::FromBack,
        );

        // Z-grid should be unchanged.
        let z_top_after = ray_top(stock.z_grid.ray(zr, zc)).unwrap();
        assert!(
            (z_top_after - z_top_before).abs() < 1e-6,
            "Z-grid should not be affected by Y-grid stamping"
        );

        // Y-grid should have cuts.
        let y_grid = stock.y_grid.as_ref().unwrap();
        let (yr, yc) = y_grid.world_to_cell(15.0, 10.0).unwrap();
        assert!(ray_top(y_grid.ray(yr, yc)).unwrap() < 30.0);
    }

    #[test]
    fn checkpoint_preserves_side_grids() {
        let tool = FlatEndmill::new(10.0, 25.0);
        let mut stock = TriDexelStock::from_stock(0.0, 0.0, 20.0, 20.0, 0.0, 20.0, 1.0);

        let lut = RadialProfileLUT::from_cutter(&tool, 256);
        // Create Y-grid via stamp.
        stock.stamp_tool_at(
            &lut,
            tool.radius(),
            10.0,
            15.0,
            10.0,
            StockCutDirection::FromBack,
        );

        let saved = stock.checkpoint();

        // Cut more on the original.
        stock.stamp_tool_at(
            &lut,
            tool.radius(),
            10.0,
            10.0,
            10.0,
            StockCutDirection::FromBack,
        );

        // Saved Y-grid should be unaffected by the second cut.
        let saved_y = saved.y_grid.as_ref().unwrap();
        let (row, col) = saved_y.world_to_cell(10.0, 10.0).unwrap();
        assert!(
            (ray_top(saved_y.ray(row, col)).unwrap() - 15.0).abs() < 0.1,
            "Checkpoint Y-grid should reflect only the first cut"
        );
    }

    #[test]
    fn simulate_toolpath_on_y_grid() {
        let tool = FlatEndmill::new(4.0, 20.0);
        let mut stock = TriDexelStock::from_stock(0.0, 0.0, 20.0, 20.0, 0.0, 20.0, 0.5);

        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(5.0, 25.0, 10.0)); // approach from outside
        tp.feed_to(P3::new(5.0, 15.0, 10.0), 1000.0); // plunge into stock
        tp.feed_to(P3::new(15.0, 15.0, 10.0), 1000.0); // cut along X

        stock.simulate_toolpath(&tp, &tool, StockCutDirection::FromBack);

        assert!(stock.y_grid.is_some());
        let y_grid = stock.y_grid.as_ref().unwrap();
        // Cell at (x=10, z=10) should be cut.
        let (row, col) = y_grid.world_to_cell(10.0, 10.0).unwrap();
        assert!(ray_top(y_grid.ray(row, col)).unwrap() < 20.0);
    }

    // ── Query helper tests ─────────────────────────────────────────────

    #[test]
    fn test_local_material_sum() {
        // 10x10 stock, z 0..5, cell_size=1. All cells have top=5.
        let stock = TriDexelStock::from_stock(0.0, 0.0, 10.0, 10.0, 0.0, 5.0, 1.0);

        // Sum in a radius of 1.5 around the center (5, 5).
        // Cells within radius 1.5 of (5,5): the center plus 4 axis-neighbors
        // plus 4 diagonal neighbors (dist = sqrt(2) ~= 1.414 < 1.5).
        // That's 9 cells, each with top=5 => sum = 45.
        let sum = stock.local_material_sum(5.0, 5.0, 1.5);
        assert!((sum - 45.0).abs() < 1e-6, "Expected 45.0, got {sum}");
    }

    #[test]
    fn test_local_material_sum_after_stamp() {
        let tool = FlatEndmill::new(4.0, 20.0); // radius 2
        let mut stock = TriDexelStock::from_stock(0.0, 0.0, 10.0, 10.0, 0.0, 5.0, 1.0);

        // Before stamping: sum around center should reflect full stock.
        let sum_before = stock.local_material_sum(5.0, 5.0, 3.0);

        // Stamp tool at center, cutting to z=2.
        let lut = RadialProfileLUT::from_cutter(&tool, 256);
        stock.stamp_tool_at(
            &lut,
            tool.radius(),
            5.0,
            5.0,
            2.0,
            StockCutDirection::FromTop,
        );

        let sum_after = stock.local_material_sum(5.0, 5.0, 3.0);

        // The stamp removed material, so sum should decrease.
        assert!(
            sum_after < sum_before,
            "Sum should decrease after stamp: before={sum_before}, after={sum_after}"
        );
    }

    #[test]
    fn test_clear_above_at() {
        let mut stock = TriDexelStock::from_stock(0.0, 0.0, 4.0, 4.0, 0.0, 10.0, 1.0);

        // Clear above z=6 at cell (2, 2).
        stock.clear_above_at(2, 2, 6.0);

        let ray = stock.z_grid.ray(2, 2);
        assert_eq!(ray_top(ray), Some(6.0));

        // Neighboring cell should be untouched.
        let neighbor = stock.z_grid.ray(2, 1);
        assert_eq!(ray_top(neighbor), Some(10.0));
    }

    #[test]
    fn test_clear_above_at_empty_ray() {
        let mut stock = TriDexelStock::from_stock(0.0, 0.0, 4.0, 4.0, 0.0, 10.0, 1.0);

        // Clear the ray entirely first.
        stock.z_grid.ray_mut(1, 1).clear();
        assert!(stock.z_grid.ray(1, 1).is_empty());

        // clear_above_at on an empty ray should not panic.
        stock.clear_above_at(1, 1, 5.0);
        assert!(stock.z_grid.ray(1, 1).is_empty());
    }
}
