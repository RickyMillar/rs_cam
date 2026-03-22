//! Tri-dexel stock representation with tool stamping and toolpath simulation.
//!
//! Replaces the 2.5-D heightmap for volumetric material removal.  The Z-grid
//! is always present; X and Y grids are created lazily when side-face cuts are
//! needed (future work).

use crate::dexel::{
    DexelAxis, DexelGrid, ray_bottom, ray_material_length, ray_subtract_above, ray_subtract_below,
    ray_top,
};
use crate::geo::{BoundingBox3, P3};
use crate::interrupt::{CancelCheck, Cancelled, check_cancel};
use crate::semantic_trace::ToolpathSemanticTrace;
use crate::simulation::RadialProfileLUT;
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
        match direction.grid_axis() {
            DexelAxis::Z => &mut self.z_grid,
            DexelAxis::Y => {
                if self.y_grid.is_none() {
                    self.y_grid = Some(DexelGrid::y_grid_from_bounds(
                        &self.stock_bbox,
                        self.z_grid.cell_size,
                    ));
                }
                self.y_grid.as_mut().unwrap()
            }
            DexelAxis::X => {
                if self.x_grid.is_none() {
                    self.x_grid = Some(DexelGrid::x_grid_from_bounds(
                        &self.stock_bbox,
                        self.z_grid.cell_size,
                    ));
                }
                self.x_grid.as_mut().unwrap()
            }
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

    let subsegments =
        ((segment_length / params.sample_step_mm.max(1e-3)).ceil() as usize).max(1);
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

// ── Arc linearization (ported from simulation.rs) ───────────────────────

/// Linearize a circular arc into a sequence of 3-D points.
///
/// The arc goes from `start` to `end` with center offset `(i, j)` relative to
/// `start`.  Z is interpolated linearly.
pub fn linearize_arc(
    start: P3,
    end: P3,
    i: f64,
    j: f64,
    clockwise: bool,
    max_seg_len: f64,
) -> Vec<P3> {
    let cx = start.x + i;
    let cy = start.y + j;
    let r = (i * i + j * j).sqrt();

    if r < 1e-10 {
        return vec![start, end];
    }

    let start_angle = (start.y - cy).atan2(start.x - cx);
    let end_angle = (end.y - cy).atan2(end.x - cx);

    let mut sweep = if clockwise {
        start_angle - end_angle
    } else {
        end_angle - start_angle
    };
    if sweep <= 0.0 {
        sweep += std::f64::consts::TAU;
    }

    let arc_len = r * sweep;
    let samples = (arc_len / max_seg_len).ceil().max(2.0) as usize;

    let mut points = Vec::with_capacity(samples + 1);
    for s in 0..=samples {
        let t = s as f64 / samples as f64;
        let angle = if clockwise {
            start_angle - t * sweep
        } else {
            start_angle + t * sweep
        };
        let (sin_a, cos_a) = angle.sin_cos();
        let x = cx + r * cos_a;
        let y = cy + r * sin_a;
        let z = start.z + t * (end.z - start.z);
        points.push(P3::new(x, y, z));
    }
    points
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dexel::{ray_bottom, ray_top};
    use crate::simulation::{
        Heightmap, simulate_toolpath as hm_simulate_toolpath,
        stamp_linear_segment as hm_stamp_linear_segment,
    };
    use crate::tool::{BallEndmill, FlatEndmill};

    /// Helper: create a TriDexelStock and Heightmap with matching dimensions.
    fn make_pair(
        x_min: f64,
        y_min: f64,
        x_max: f64,
        y_max: f64,
        z_min: f64,
        z_max: f64,
        cell_size: f64,
    ) -> (TriDexelStock, Heightmap) {
        let stock = TriDexelStock::from_stock(x_min, y_min, x_max, y_max, z_min, z_max, cell_size);
        let hm = Heightmap::from_stock(x_min, y_min, x_max, y_max, z_max, cell_size);
        (stock, hm)
    }

    // ── Basic construction ──────────────────────────────────────────────

    #[test]
    fn from_bounds_dimensions_match_heightmap() {
        let bbox = BoundingBox3 {
            min: P3::new(0.0, 0.0, -5.0),
            max: P3::new(10.0, 10.0, 5.0),
        };
        let stock = TriDexelStock::from_bounds(&bbox, 1.0);
        let hm = Heightmap::from_bounds(&bbox, None, 1.0);
        assert_eq!(stock.z_grid.rows, hm.rows);
        assert_eq!(stock.z_grid.cols, hm.cols);
    }

    // ── Single stamp equivalence ────────────────────────────────────────

    #[test]
    fn stamp_flat_endmill_matches_heightmap() {
        let tool = FlatEndmill::new(10.0, 25.0); // radius 5
        let (mut stock, mut hm) = make_pair(-10.0, -10.0, 10.0, 10.0, 0.0, 5.0, 0.5);

        let lut = RadialProfileLUT::from_cutter(&tool, 256);
        stock.stamp_tool_at(
            &lut,
            tool.radius(),
            0.0,
            0.0,
            2.0,
            StockCutDirection::FromTop,
        );

        crate::simulation::stamp_tool_at_lut(&mut hm, &lut, tool.radius(), 0.0, 0.0, 2.0);

        // Compare: dexel ray top should match heightmap cell value.
        for row in 0..hm.rows {
            for col in 0..hm.cols {
                let hm_z = hm.get(row, col);
                let ray = stock.z_grid.ray(row, col);
                let dex_z = ray_top(ray).unwrap_or(0.0) as f64;
                assert!(
                    (dex_z - hm_z).abs() < 0.01,
                    "Mismatch at ({row},{col}): dexel={dex_z:.4}, hm={hm_z:.4}"
                );
            }
        }
    }

    #[test]
    fn stamp_ball_endmill_matches_heightmap() {
        let tool = BallEndmill::new(6.0, 25.0); // radius 3
        let (mut stock, mut hm) = make_pair(-10.0, -10.0, 10.0, 10.0, 0.0, 5.0, 0.5);

        let lut = RadialProfileLUT::from_cutter(&tool, 256);
        stock.stamp_tool_at(
            &lut,
            tool.radius(),
            0.0,
            0.0,
            1.0,
            StockCutDirection::FromTop,
        );

        crate::simulation::stamp_tool_at_lut(&mut hm, &lut, tool.radius(), 0.0, 0.0, 1.0);

        for row in 0..hm.rows {
            for col in 0..hm.cols {
                let hm_z = hm.get(row, col);
                let ray = stock.z_grid.ray(row, col);
                let dex_z = ray_top(ray).unwrap_or(0.0) as f64;
                assert!(
                    (dex_z - hm_z).abs() < 0.02,
                    "Mismatch at ({row},{col}): dexel={dex_z:.4}, hm={hm_z:.4}"
                );
            }
        }
    }

    // ── Linear segment equivalence ──────────────────────────────────────

    #[test]
    fn linear_segment_flat_matches_heightmap() {
        let tool = FlatEndmill::new(4.0, 20.0);
        let (mut stock, mut hm) = make_pair(-5.0, -5.0, 15.0, 5.0, 0.0, 5.0, 0.5);

        let start = P3::new(0.0, 0.0, 2.0);
        let end = P3::new(10.0, 0.0, 2.0);

        let lut = RadialProfileLUT::from_cutter(&tool, 256);
        stock.stamp_linear_segment(&lut, tool.radius(), start, end, StockCutDirection::FromTop);
        hm_stamp_linear_segment(&mut hm, &tool, start, end);

        let mut max_diff = 0.0_f64;
        for row in 0..hm.rows {
            for col in 0..hm.cols {
                let hm_z = hm.get(row, col);
                let dex_z = ray_top(stock.z_grid.ray(row, col)).unwrap_or(0.0) as f64;
                let diff = (dex_z - hm_z).abs();
                max_diff = max_diff.max(diff);
            }
        }
        assert!(max_diff < 0.02, "Max diff: {max_diff:.4}mm");
    }

    #[test]
    fn linear_segment_ball_diagonal_matches_heightmap() {
        let tool = BallEndmill::new(6.0, 25.0);
        let (mut stock, mut hm) = make_pair(0.0, 0.0, 30.0, 30.0, -5.0, 5.0, 0.25);

        let start = P3::new(5.0, 5.0, -1.0);
        let end = P3::new(25.0, 25.0, -1.0);

        let lut = RadialProfileLUT::from_cutter(&tool, 256);
        stock.stamp_linear_segment(&lut, tool.radius(), start, end, StockCutDirection::FromTop);
        hm_stamp_linear_segment(&mut hm, &tool, start, end);

        let mut max_diff = 0.0_f64;
        for row in 0..hm.rows {
            for col in 0..hm.cols {
                let hm_z = hm.get(row, col);
                let dex_z = ray_top(stock.z_grid.ray(row, col)).unwrap_or(-5.0) as f64;
                let diff = (dex_z - hm_z).abs();
                max_diff = max_diff.max(diff);
            }
        }
        assert!(max_diff < 0.02, "Max diff: {max_diff:.4}mm");
    }

    // ── Toolpath simulation equivalence ─────────────────────────────────

    #[test]
    fn simulate_toolpath_matches_heightmap() {
        let tool = FlatEndmill::new(4.0, 20.0);
        let (mut stock, mut hm) = make_pair(-5.0, -5.0, 15.0, 5.0, -5.0, 0.0, 0.5);

        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 10.0));
        tp.feed_to(P3::new(0.0, 0.0, -3.0), 500.0);
        tp.feed_to(P3::new(10.0, 0.0, -3.0), 1000.0);

        stock.simulate_toolpath(&tp, &tool, StockCutDirection::FromTop);
        hm_simulate_toolpath(&tp, &tool, &mut hm);

        let mut max_diff = 0.0_f64;
        for row in 0..hm.rows {
            for col in 0..hm.cols {
                let hm_z = hm.get(row, col);
                let dex_z = ray_top(stock.z_grid.ray(row, col)).unwrap_or(-5.0) as f64;
                let diff = (dex_z - hm_z).abs();
                max_diff = max_diff.max(diff);
            }
        }
        assert!(max_diff < 0.02, "Max diff: {max_diff:.4}mm");
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

    // ── Arc linearization ───────────────────────────────────────────────

    #[test]
    fn linearize_arc_semicircle() {
        let start = P3::new(5.0, 0.0, 0.0);
        let end = P3::new(-5.0, 0.0, 0.0);
        let points = linearize_arc(start, end, -5.0, 0.0, false, 0.5);

        for p in &points {
            let r = (p.x * p.x + p.y * p.y).sqrt();
            assert!((r - 5.0).abs() < 0.05, "r = {r:.3}");
        }
        let last = points.last().unwrap();
        assert!((last.x - end.x).abs() < 0.1);
    }

    #[test]
    fn linearize_arc_z_interpolation() {
        let start = P3::new(5.0, 0.0, 0.0);
        let end = P3::new(-5.0, 0.0, 10.0);
        let points = linearize_arc(start, end, -5.0, 0.0, false, 0.5);
        assert!((points.first().unwrap().z).abs() < 1e-10);
        assert!((points.last().unwrap().z - 10.0).abs() < 0.1);
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
}
