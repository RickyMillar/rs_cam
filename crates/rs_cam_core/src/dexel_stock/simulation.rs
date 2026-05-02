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
use crate::simulation_cut::{CutKinematics, SimulationCutSample};
use crate::tool::{EngagementMode, MillingCutter};
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
        capture_arc_engagement: bool,
        cancel: &dyn CancelCheck,
    ) -> Result<Vec<SimulationCutSample>, Cancelled> {
        let lut = RadialProfileLUT::from_cutter(cutter, 256);
        self.simulate_toolpath_with_lut_metrics_cancel(
            toolpath,
            &lut,
            cutter,
            cutter.radius(),
            direction,
            toolpath_id,
            spindle_rpm,
            flute_count,
            rapid_feed_mm_min,
            sample_step_mm,
            semantic_trace,
            capture_arc_engagement,
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
        cutter: &dyn MillingCutter,
        radius: f64,
        direction: StockCutDirection,
        toolpath_id: usize,
        spindle_rpm: u32,
        flute_count: u32,
        rapid_feed_mm_min: f64,
        sample_step_mm: f64,
        semantic_trace: Option<&ToolpathSemanticTrace>,
        capture_arc_engagement: bool,
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
                            cut_kinematics: CutKinematics::Rapid,
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
                        cutter,
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
                            cut_kinematics: classify_cut_kinematics(start, end, false),
                            capture_arc_engagement,
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
                            cutter,
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
                                cut_kinematics: CutKinematics::Arc,
                                capture_arc_engagement,
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
                            cutter,
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
                                cut_kinematics: CutKinematics::Arc,
                                capture_arc_engagement,
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
        cutter: &dyn MillingCutter,
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
            let (axial_doc_mm, radial_engagement, arc_engagement_radians, removed_volume_est_mm3) =
                self.estimate_and_stamp_cutting_subsegment(
                    lut,
                    radius,
                    seg_start,
                    seg_end,
                    midpoint,
                    direction,
                    params.capture_arc_engagement,
                );

            *cumulative_time_s += segment_time_s;
            let chipload_mm_per_tooth = chipload_mm_per_tooth(
                params.feed_rate_mm_min,
                params.spindle_rpm,
                params.flute_count,
            );
            let effective_chip_thickness_mm = effective_chip_thickness_mm(
                cutter,
                axial_doc_mm,
                arc_engagement_radians,
                chipload_mm_per_tooth,
                params.flute_count,
            );
            samples.push(SimulationCutSample {
                toolpath_id: params.toolpath_id,
                move_index: params.move_index,
                sample_index: *next_sample_index,
                position: [midpoint.x, midpoint.y, midpoint.z],
                cumulative_time_s: *cumulative_time_s,
                segment_time_s,
                is_cutting: true,
                cut_kinematics: params.cut_kinematics,
                feed_rate_mm_min: params.feed_rate_mm_min,
                spindle_rpm: params.spindle_rpm,
                flute_count: params.flute_count,
                axial_doc_mm,
                radial_engagement,
                arc_engagement_radians,
                chipload_mm_per_tooth,
                effective_chip_thickness_mm,
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

    #[allow(clippy::too_many_arguments)]
    fn estimate_and_stamp_cutting_subsegment(
        &mut self,
        lut: &RadialProfileLUT,
        radius: f64,
        seg_start: P3,
        seg_end: P3,
        midpoint: P3,
        direction: StockCutDirection,
        capture_arc_engagement: bool,
    ) -> (f64, f64, Option<f64>, f64) {
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
            capture_arc_engagement,
        )
    }
}

fn effective_chip_thickness_mm(
    cutter: &dyn MillingCutter,
    axial_doc_mm: f64,
    arc_engagement_radians: Option<f64>,
    feed_per_tooth_mm: f64,
    flute_count: u32,
) -> Option<f64> {
    let arc = arc_engagement_radians?;
    cutter
        .chip_geometry(
            axial_doc_mm,
            arc,
            feed_per_tooth_mm,
            flute_count,
            EngagementMode::Slot,
        )
        .ok()
        .map(|geometry| geometry.max_chip_thickness_mm)
}

fn classify_cut_kinematics(start: P3, end: P3, is_arc: bool) -> CutKinematics {
    if is_arc {
        return CutKinematics::Arc;
    }
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let dz = end.z - start.z;
    let xy_len_sq = dx * dx + dy * dy;
    if xy_len_sq < 1e-18 && dz.abs() > 1e-9 {
        CutKinematics::Plunge
    } else if xy_len_sq > 1e-18 && dz.abs() > 1e-9 {
        CutKinematics::Helix
    } else {
        CutKinematics::Linear
    }
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
    use crate::geo::BoundingBox3;
    use crate::tool::FlatEndmill;

    fn simulate_line(
        stock_y_min: f64,
        stock_y_max: f64,
        start: P3,
        end: P3,
    ) -> Vec<SimulationCutSample> {
        let bbox = BoundingBox3 {
            min: P3::new(-20.0, stock_y_min, -5.0),
            max: P3::new(20.0, stock_y_max, 5.0),
        };
        let mut stock = TriDexelStock::from_bounds(&bbox, 0.25);
        let cutter = FlatEndmill::new(10.0, 20.0);
        let mut toolpath = Toolpath::new();
        toolpath.rapid_to(start);
        toolpath.feed_to(end, 600.0);
        let never_cancel = || false;
        stock
            .simulate_toolpath_with_metrics_with_cancel(
                &toolpath,
                &cutter,
                StockCutDirection::FromTop,
                0,
                12_000,
                2,
                3000.0,
                2.0,
                None,
                true,
                &never_cancel,
            )
            .expect("simulation succeeds")
    }

    #[test]
    fn half_width_linear_cut_captures_quarter_turn_arc() {
        let samples = simulate_line(
            0.0,
            20.0,
            P3::new(-10.0, 0.0, -1.0),
            P3::new(10.0, 0.0, -1.0),
        );
        let sample = samples
            .iter()
            .find(|sample| sample.arc_engagement_radians.is_some())
            .expect("arc sample");
        let arc = sample.arc_engagement_radians.expect("arc captured");
        assert_eq!(sample.cut_kinematics, CutKinematics::Linear);
        assert!(
            (arc - std::f64::consts::FRAC_PI_2).abs() <= 0.12,
            "arc={arc}"
        );
    }

    #[test]
    fn full_slot_linear_cut_captures_half_turn_arc() {
        let samples = simulate_line(
            -20.0,
            20.0,
            P3::new(-10.0, 0.0, -1.0),
            P3::new(10.0, 0.0, -1.0),
        );
        let sample = samples
            .iter()
            .find(|sample| sample.arc_engagement_radians.is_some())
            .expect("arc sample");
        let arc = sample.arc_engagement_radians.expect("arc captured");
        assert_eq!(sample.cut_kinematics, CutKinematics::Linear);
        assert!((arc - std::f64::consts::PI).abs() <= 0.12, "arc={arc}");
    }

    #[test]
    fn rapid_and_plunge_kinematics_do_not_capture_arc() {
        let bbox = BoundingBox3 {
            min: P3::new(-5.0, -5.0, -5.0),
            max: P3::new(5.0, 5.0, 5.0),
        };
        let mut stock = TriDexelStock::from_bounds(&bbox, 0.5);
        let cutter = FlatEndmill::new(3.0, 10.0);
        let mut toolpath = Toolpath::new();
        toolpath.rapid_to(P3::new(0.0, 0.0, 5.0));
        toolpath.rapid_to(P3::new(1.0, 0.0, 5.0));
        toolpath.feed_to(P3::new(1.0, 0.0, -1.0), 100.0);
        let never_cancel = || false;
        let samples = stock
            .simulate_toolpath_with_metrics_with_cancel(
                &toolpath,
                &cutter,
                StockCutDirection::FromTop,
                0,
                12_000,
                2,
                3000.0,
                1.0,
                None,
                true,
                &never_cancel,
            )
            .expect("simulation succeeds");
        assert!(
            samples
                .iter()
                .any(|sample| sample.cut_kinematics == CutKinematics::Rapid
                    && sample.arc_engagement_radians.is_none())
        );
        assert!(
            samples
                .iter()
                .any(|sample| sample.cut_kinematics == CutKinematics::Plunge
                    && sample.arc_engagement_radians.is_none())
        );
    }
}
