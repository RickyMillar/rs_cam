//! `TriDexelStock::simulate_*` methods — toolpath simulation implementations.
//!
//! These live in a separate `impl TriDexelStock { ... }` block so the main
//! `mod.rs` file stays focused on the type, construction, and single-shot
//! stamp operations. Behavior is unchanged from the monolithic version.

use super::stamping::{
    CuttingCaptureParams, SegmentSampleParams, build_move_semantic_lookup, chipload_mm_per_tooth,
    lerp_point, sample_segment_runtime, stamp_segment_with_metrics,
};
use super::{StockCutDirection, TriDexelStock};

use crate::arc_util::linearize_arc_into;
use crate::geo::P3;
use crate::interrupt::{CancelCheck, Cancelled, check_cancel};
use crate::radial_profile::RadialProfileLUT;
use crate::semantic_trace::ToolpathSemanticTrace;
use crate::simulation_cut::SimulationCutSample;
use crate::tool::MillingCutter;
use crate::toolpath::{MoveType, Toolpath};

impl TriDexelStock {
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
        let lut = RadialProfileLUT::from_cutter(cutter, 256);
        self.simulate_toolpath_with_lut_cancel(toolpath, &lut, cutter.radius(), direction, cancel)
    }

    /// Simulate with a pre-built LUT (avoids rebuilding for repeated same-tool calls).
    #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
    pub fn simulate_toolpath_with_lut_cancel(
        &mut self,
        toolpath: &Toolpath,
        lut: &RadialProfileLUT,
        radius: f64,
        direction: StockCutDirection,
        cancel: &dyn CancelCheck,
    ) -> Result<(), Cancelled> {
        if toolpath.moves.is_empty() {
            return Ok(());
        }

        let mut arc_buf = Vec::new();

        for i in 1..toolpath.moves.len() {
            check_cancel(cancel)?;
            let start = toolpath.moves[i - 1].target;
            let end = toolpath.moves[i].target;

            match toolpath.moves[i].move_type {
                MoveType::Rapid => {}
                MoveType::Linear { .. } => {
                    self.stamp_linear_segment(lut, radius, start, end, direction);
                }
                MoveType::ArcCW { i, j, .. } => {
                    let cs = self.z_grid.cell_size;
                    linearize_arc_into(&mut arc_buf, start, end, i, j, true, cs);
                    for w in arc_buf.windows(2) {
                        check_cancel(cancel)?;
                        self.stamp_linear_segment(lut, radius, w[0], w[1], direction);
                    }
                }
                MoveType::ArcCCW { i, j, .. } => {
                    let cs = self.z_grid.cell_size;
                    linearize_arc_into(&mut arc_buf, start, end, i, j, false, cs);
                    for w in arc_buf.windows(2) {
                        check_cancel(cancel)?;
                        self.stamp_linear_segment(lut, radius, w[0], w[1], direction);
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
        let lut = RadialProfileLUT::from_cutter(cutter, 256);
        self.simulate_toolpath_with_lut_metrics_cancel(
            toolpath,
            &lut,
            cutter.radius(),
            direction,
            toolpath_id,
            spindle_rpm,
            flute_count,
            rapid_feed_mm_min,
            sample_step_mm,
            semantic_trace,
            cancel,
        )
    }

    /// Simulate with metrics using a pre-built LUT.
    #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
    #[allow(clippy::too_many_arguments)]
    pub fn simulate_toolpath_with_lut_metrics_cancel(
        &mut self,
        toolpath: &Toolpath,
        lut: &RadialProfileLUT,
        radius: f64,
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
        let sample_step_mm = sample_step_mm.max(1e-3);
        let semantic_lookup = build_move_semantic_lookup(toolpath.moves.len(), semantic_trace);

        let mut samples = Vec::with_capacity(toolpath.moves.len() * 2);
        let mut cumulative_time_s = 0.0;
        let mut next_sample_index = 0usize;
        let mut arc_buf = Vec::new();

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
                        lut,
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
                    linearize_arc_into(&mut arc_buf, start, end, i, j, true, self.z_grid.cell_size);
                    for window in arc_buf.windows(2) {
                        check_cancel(cancel)?;
                        self.capture_cutting_segment(
                            lut,
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
                    linearize_arc_into(
                        &mut arc_buf,
                        start,
                        end,
                        i,
                        j,
                        false,
                        self.z_grid.cell_size,
                    );
                    for window in arc_buf.windows(2) {
                        check_cancel(cancel)?;
                        self.capture_cutting_segment(
                            lut,
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
        let lut = RadialProfileLUT::from_cutter(cutter, 256);
        self.simulate_toolpath_range_with_lut(
            toolpath,
            &lut,
            cutter.radius(),
            direction,
            start_move,
            end_move,
        );
    }

    /// Simulate a range of moves using a pre-built LUT.
    #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
    pub fn simulate_toolpath_range_with_lut(
        &mut self,
        toolpath: &Toolpath,
        lut: &RadialProfileLUT,
        radius: f64,
        direction: StockCutDirection,
        start_move: usize,
        end_move: usize,
    ) {
        if toolpath.moves.len() < 2 {
            return;
        }
        let first = start_move.max(1);
        let last = end_move.min(toolpath.moves.len());
        let mut arc_buf = Vec::new();

        for i in first..last {
            let start = toolpath.moves[i - 1].target;
            let end = toolpath.moves[i].target;

            match toolpath.moves[i].move_type {
                MoveType::Rapid => {}
                MoveType::Linear { .. } => {
                    self.stamp_linear_segment(lut, radius, start, end, direction);
                }
                MoveType::ArcCW { i, j, .. } => {
                    let cs = self.z_grid.cell_size;
                    linearize_arc_into(&mut arc_buf, start, end, i, j, true, cs);
                    for w in arc_buf.windows(2) {
                        self.stamp_linear_segment(lut, radius, w[0], w[1], direction);
                    }
                }
                MoveType::ArcCCW { i, j, .. } => {
                    let cs = self.z_grid.cell_size;
                    linearize_arc_into(&mut arc_buf, start, end, i, j, false, cs);
                    for w in arc_buf.windows(2) {
                        self.stamp_linear_segment(lut, radius, w[0], w[1], direction);
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
        stamp_segment_with_metrics(
            grid,
            lut,
            radius,
            (su, sv, sd),
            (eu, ev, ed),
            mu,
            mv,
            md,
            from_high,
        )
    }
}
