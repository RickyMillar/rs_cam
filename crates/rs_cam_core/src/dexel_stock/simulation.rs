//! `TriDexelStock::simulate_*` methods — toolpath simulation implementations.
//!
//! These live in a separate `impl TriDexelStock { ... }` block so the main
//! `mod.rs` file stays focused on the type, construction, and single-shot
//! stamp operations. Behavior is unchanged from the monolithic version.

use super::band;
use super::stamping::StampPartial;
use super::stamping::{
    CuttingCaptureParams, SegmentSampleParams, build_move_semantic_lookup, chipload_mm_per_tooth,
    lerp_point, sample_segment_runtime, stamp_segment_with_metrics,
};
use super::swept::{SweptDispatch, SweptJob, classify_subsegment};
use super::tile_mip::TileMaxTop;
use super::whole_path::{BandDispatch, StampDispatch, StampJob};
use super::{StockCutDirection, TriDexelStock};
use crate::dexel::DexelGrid;
use crate::ids::ToolpathId;

use crate::arc_util::linearize_arc_into;
use crate::geo::P3;
use crate::interrupt::{CancelCheck, Cancelled, check_cancel};
use crate::radial_profile::RadialProfileLUT;
use crate::semantic_trace::ToolpathSemanticTrace;
use crate::simulation_cut::{CutKinematics, SimulationCutSample};
use crate::tool::{EngagementMode, MillingCutter};
use crate::toolpath::{MoveType, Toolpath};
use crate::toolpath_spans::SpanId;
use rayon::prelude::*;

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
        let lut = RadialProfileLUT::from_cutter(cutter, crate::radial_profile::LUT_SAMPLES);
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
        // S2 — same air-skip as the metric path. This is the playback stamp
        // `compute/simulate.rs` runs against `global_stock` for EVERY toolpath,
        // so it is a second full replay of the whole project.
        let mut air_mip = self.build_air_mip(lut, direction);

        for i in 1..toolpath.moves.len() {
            check_cancel(cancel)?;
            let start = toolpath.moves[i - 1].target;
            let end = toolpath.moves[i].target;

            match toolpath.moves[i].move_type {
                MoveType::Rapid => {}
                MoveType::Linear { .. } => {
                    self.stamp_linear_segment_with_mip(
                        lut,
                        radius,
                        start,
                        end,
                        direction,
                        &mut air_mip,
                    );
                }
                MoveType::ArcCW { i, j, .. } => {
                    let cs = self.z_grid.cell_size;
                    linearize_arc_into(&mut arc_buf, start, end, i, j, true, cs);
                    for w in arc_buf.windows(2) {
                        check_cancel(cancel)?;
                        self.stamp_linear_segment_with_mip(
                            lut,
                            radius,
                            w[0],
                            w[1],
                            direction,
                            &mut air_mip,
                        );
                    }
                }
                MoveType::ArcCCW { i, j, .. } => {
                    let cs = self.z_grid.cell_size;
                    linearize_arc_into(&mut arc_buf, start, end, i, j, false, cs);
                    for w in arc_buf.windows(2) {
                        check_cancel(cancel)?;
                        self.stamp_linear_segment_with_mip(
                            lut,
                            radius,
                            w[0],
                            w[1],
                            direction,
                            &mut air_mip,
                        );
                    }
                }
            }
        }
        Ok(())
    }

    /// Build the S2 air-skip mip for `direction`, or `None` where the skip
    /// cannot be shown exact.
    ///
    /// Two preconditions, both read off the objects that will actually be
    /// used rather than assumed from the call site: `conservative_top` is a
    /// high-side channel only, and the skip's `tip <= tip + h(d)` step needs
    /// `h >= 0` over a total profile. Returning `None` turns every downstream
    /// early-out off, which is the safe direction.
    fn build_air_mip(
        &mut self,
        lut: &RadialProfileLUT,
        direction: StockCutDirection,
    ) -> Option<TileMaxTop> {
        if direction.cuts_from_high_side() && lut.profile_is_nonneg_total() {
            Some(TileMaxTop::build(self.ensure_grid(direction)))
        } else {
            None
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn simulate_toolpath_with_metrics_with_cancel(
        &mut self,
        toolpath: &Toolpath,
        cutter: &dyn MillingCutter,
        direction: StockCutDirection,
        toolpath_id: ToolpathId,
        spindle_rpm: u32,
        flute_count: u32,
        rapid_feed_mm_min: f64,
        sample_step_mm: f64,
        semantic_trace: Option<&ToolpathSemanticTrace>,
        span_paths_by_move: &[Vec<SpanId>],
        transit_moves: &[bool],
        capture_arc_engagement: bool,
        cancel: &dyn CancelCheck,
    ) -> Result<Vec<SimulationCutSample>, Cancelled> {
        let lut = RadialProfileLUT::from_cutter(cutter, crate::radial_profile::LUT_SAMPLES);
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
            span_paths_by_move,
            transit_moves,
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
        toolpath_id: ToolpathId,
        spindle_rpm: u32,
        flute_count: u32,
        rapid_feed_mm_min: f64,
        sample_step_mm: f64,
        semantic_trace: Option<&ToolpathSemanticTrace>,
        span_paths_by_move: &[Vec<SpanId>],
        transit_moves: &[bool],
        capture_arc_engagement: bool,
        cancel: &dyn CancelCheck,
    ) -> Result<Vec<SimulationCutSample>, Cancelled> {
        if toolpath.moves.len() < 2 {
            return Ok(Vec::new());
        }
        let sample_step_mm = sample_step_mm.max(1e-3);
        let semantic_lookup = build_move_semantic_lookup(toolpath.moves.len(), semantic_trace);
        let empty_span_path: Vec<SpanId> = Vec::new();

        let mut samples = Vec::with_capacity(estimate_sample_count(toolpath, sample_step_mm));
        let mut cumulative_time_s = 0.0;
        let mut next_sample_index = 0usize;
        let mut arc_buf = Vec::new();

        // S2 (PERF_REVIEW): coarse max-`conservative_top` mip, so a stamp over
        // ground already cut below it costs a handful of tile reads instead of
        // a full swept-bbox sweep. Built once per toolpath and refreshed on an
        // amortised budget — see `tile_mip`. Only the high side has a
        // `conservative_top` channel, and only a non-negative total profile
        // makes the skip exact, so the mip is simply not built otherwise and
        // every early-out downstream degrades to "never fires".
        let mut air_mip = self.build_air_mip(lut, direction);
        // S3: reused across every stamp so the band fan-out costs no
        // allocation. `collect_into_vec` writes into it in band order.
        let mut band_scratch: Vec<StampPartial> = Vec::new();
        // S3 wave 4: `Some` selects whole-toolpath dispatch — one `par_bands`
        // per batch of stamps instead of one per stamp. A schedule only; see
        // `whole_path.rs`.
        let mut dispatch = {
            let mode = self.stamp_dispatch;
            let grid = self.ensure_grid(direction);
            BandDispatch::for_grid(grid, mode)
        };
        // S1 wave 5: a THIRD scheduler, and deliberately a separate `Option`
        // rather than a third arm of `dispatch`. The two are mutually
        // exclusive by construction (`BandDispatch::for_grid` returns `None`
        // for `Swept`), and keeping them apart means the per-stamp and
        // whole-path code paths below are untouched — which is what makes the
        // A/B same-binary and the "old mode unchanged" claim checkable by
        // reading the diff rather than by trusting it.
        let mut swept = match self.stamp_dispatch {
            StampDispatch::Swept | StampDispatch::SweptPlungeOnly => {
                let lateral = self.stamp_dispatch == StampDispatch::Swept;
                let grid = self.ensure_grid(direction);
                SweptDispatch::for_grid(grid, lateral)
            }
            _ => None,
        };
        self.last_stamp_dispatch = super::StampDispatchStats::default();
        let from_high = direction.cuts_from_high_side();
        let flute_length = cutter.length().max(1e-9);

        for move_index in 1..toolpath.moves.len() {
            check_cancel(cancel)?;
            let start = toolpath.moves[move_index - 1].target;
            let end = toolpath.moves[move_index].target;
            let semantic_item_id = semantic_lookup.get(move_index).copied().flatten();
            let span_path: &[SpanId] = span_paths_by_move
                .get(move_index)
                .map(|v| v.as_slice())
                .unwrap_or(empty_span_path.as_slice());
            // P3: rapids are inherently transit. Linear/Arc moves are
            // transit only when sitting in a transit-style span.
            let in_transit_span = transit_moves.get(move_index).copied().unwrap_or(false)
                || matches!(toolpath.moves[move_index].move_type, MoveType::Rapid);

            // §6.C / §6.I: a Linear feed tagged `MoveIntent::Retract` is a
            // lift through cleared air, not a cutting move. Treat it like a
            // Rapid for accumulator + dexel purposes — same time accounting,
            // no stamping, `is_cutting = false`. The kinematic-heuristic
            // fallback below remains for moves with `MoveIntent::Unknown`.
            let intent = toolpath.moves[move_index].intent;
            let is_retract_feed = matches!(intent, crate::toolpath::MoveIntent::Retract)
                && matches!(
                    toolpath.moves[move_index].move_type,
                    MoveType::Linear { .. }
                );

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
                            span_path,
                            in_transit_span,
                            source_intent: Some(intent),
                        },
                        &mut cumulative_time_s,
                        &mut next_sample_index,
                        &mut samples,
                    );
                }
                MoveType::Linear { feed_rate } if is_retract_feed => {
                    sample_segment_runtime(
                        start,
                        end,
                        &SegmentSampleParams {
                            move_index,
                            toolpath_id,
                            sample_step_mm,
                            feed_rate_mm_min: feed_rate.max(1.0),
                            is_cutting: false,
                            cut_kinematics: CutKinematics::Linear,
                            spindle_rpm,
                            flute_count,
                            semantic_item_id,
                            span_path,
                            in_transit_span,
                            source_intent: Some(intent),
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
                            span_path,
                            sample_step_mm,
                            cut_kinematics: classify_cut_kinematics(start, end, false),
                            capture_arc_engagement,
                            in_transit_span,
                            source_intent: Some(intent),
                        },
                        cancel,
                        &mut cumulative_time_s,
                        &mut next_sample_index,
                        &mut samples,
                        &mut air_mip,
                        &mut band_scratch,
                        &mut dispatch,
                        &mut swept,
                        flute_length,
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
                                span_path,
                                sample_step_mm,
                                cut_kinematics: CutKinematics::Arc,
                                capture_arc_engagement,
                                in_transit_span,
                                source_intent: Some(intent),
                            },
                            cancel,
                            &mut cumulative_time_s,
                            &mut next_sample_index,
                            &mut samples,
                            &mut air_mip,
                            &mut band_scratch,
                            &mut dispatch,
                            &mut swept,
                            flute_length,
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
                                span_path,
                                sample_step_mm,
                                cut_kinematics: CutKinematics::Arc,
                                capture_arc_engagement,
                                in_transit_span,
                                source_intent: Some(intent),
                            },
                            cancel,
                            &mut cumulative_time_s,
                            &mut next_sample_index,
                            &mut samples,
                            &mut air_mip,
                            &mut band_scratch,
                            &mut dispatch,
                            &mut swept,
                            flute_length,
                        )?;
                    }
                }
            }
        }

        // The tail batch. Everything queued after the last mid-toolpath flush
        // still has to reach the grid and the sample stream.
        if let Some(queue) = dispatch.as_mut()
            && !queue.is_empty()
        {
            let grid = self.ensure_grid(direction);
            run_batch_into_samples(
                queue,
                grid,
                lut,
                radius,
                from_high,
                capture_arc_engagement,
                &mut air_mip,
                cancel,
                &mut samples,
                cutter,
                flute_length,
            )?;
        }
        if let Some(queue) = dispatch.as_ref() {
            self.last_stamp_dispatch = queue.stats();
        }
        // The swept tail, same contract as the batched one above.
        if let Some(queue) = swept.as_mut()
            && !queue.is_empty()
        {
            let grid = self.ensure_grid(direction);
            run_swept_batch_into_samples(
                queue,
                grid,
                lut,
                radius,
                from_high,
                capture_arc_engagement,
                &mut air_mip,
                cancel,
                &mut samples,
                cutter,
                flute_length,
            )?;
        }
        if let Some(queue) = swept.as_ref() {
            self.last_stamp_dispatch = queue.stats();
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
        let lut = RadialProfileLUT::from_cutter(cutter, crate::radial_profile::LUT_SAMPLES);
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
        let mut air_mip = self.build_air_mip(lut, direction);

        for i in first..last {
            let start = toolpath.moves[i - 1].target;
            let end = toolpath.moves[i].target;

            match toolpath.moves[i].move_type {
                MoveType::Rapid => {}
                MoveType::Linear { .. } => {
                    self.stamp_linear_segment_with_mip(
                        lut,
                        radius,
                        start,
                        end,
                        direction,
                        &mut air_mip,
                    );
                }
                MoveType::ArcCW { i, j, .. } => {
                    let cs = self.z_grid.cell_size;
                    linearize_arc_into(&mut arc_buf, start, end, i, j, true, cs);
                    for w in arc_buf.windows(2) {
                        self.stamp_linear_segment_with_mip(
                            lut,
                            radius,
                            w[0],
                            w[1],
                            direction,
                            &mut air_mip,
                        );
                    }
                }
                MoveType::ArcCCW { i, j, .. } => {
                    let cs = self.z_grid.cell_size;
                    linearize_arc_into(&mut arc_buf, start, end, i, j, false, cs);
                    for w in arc_buf.windows(2) {
                        self.stamp_linear_segment_with_mip(
                            lut,
                            radius,
                            w[0],
                            w[1],
                            direction,
                            &mut air_mip,
                        );
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
        air_mip: &mut Option<TileMaxTop>,
        band_scratch: &mut Vec<StampPartial>,
        dispatch: &mut Option<BandDispatch>,
        swept: &mut Option<SweptDispatch>,
        flute_length: f64,
    ) -> Result<(), Cancelled> {
        if let Some(queue) = swept.as_mut() {
            return self.capture_cutting_segment_swept(
                lut,
                cutter,
                radius,
                start,
                end,
                direction,
                params,
                cancel,
                cumulative_time_s,
                next_sample_index,
                samples,
                air_mip,
                queue,
                flute_length,
            );
        }
        let segment_length = (end - start).norm();
        if segment_length <= 1e-9 {
            return Ok(());
        }
        let from_high = direction.cuts_from_high_side();
        // Constant across the whole move; it used to be recomputed per
        // subsegment, from the same three arguments, inside the loop.
        let chipload = chipload_mm_per_tooth(
            params.feed_rate_mm_min,
            params.spindle_rpm,
            params.flute_count,
        );

        // Subdivision must be Z-AWARE, not just length-based: the per-cell
        // stamp surface is `z(t_closest_approach) + h(d)`, which ignores Z
        // variation along the subsegment, so a subsegment descending (or
        // crossing a slope) under-removes by up to its own Z-drop. Capping
        // per-subsegment Z-drop at 0.02 mm bounds that error below the
        // finish-cusp scale (P2.g stamper probe, 2026-07-09: 0.25 mm
        // subsegments left 5–35 µm above the true envelope on ~25 % of
        // steep-finish columns, and biased move streams with longer
        // descending segments — the false fine-tier matrix verdict). Flat
        // segments (dz ≈ 0) keep the pure length-based count, so roughing
        // cost is unchanged where it dominates.
        const MAX_SUBSEGMENT_Z_DROP_MM: f64 = 0.02;
        let z_drop = (end.z - start.z).abs();
        let by_length = (segment_length / params.sample_step_mm).ceil() as usize;
        let by_z = (z_drop / MAX_SUBSEGMENT_Z_DROP_MM).ceil() as usize;
        let subsegments = by_length.max(by_z).max(1);
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

            // The stamp does not feed the clock, so accumulating time before it
            // rather than after is not observable — and it lets the sample be
            // emitted before its metrics are known, which is what whole-path
            // dispatch needs.
            *cumulative_time_s += segment_time_s;
            let slot = push_cutting_sample(
                samples,
                next_sample_index,
                &params,
                midpoint,
                segment_time_s,
                *cumulative_time_s,
                chipload,
            );

            match dispatch.as_mut() {
                // Per-stamp dispatch (wave 2): stamp now, patch now.
                None => {
                    let metrics = self.estimate_and_stamp_cutting_subsegment(
                        lut,
                        radius,
                        seg_start,
                        seg_end,
                        midpoint,
                        direction,
                        params.capture_arc_engagement,
                        air_mip,
                        band_scratch,
                    );
                    if let Some(sample) = samples.get_mut(slot) {
                        apply_subsegment_metrics(sample, cutter, flute_length, metrics);
                    }
                }
                // Whole-toolpath dispatch (wave 4): queue, and run the batch
                // when it is full. `slot` is the only thing carried forward.
                Some(queue) => {
                    let (su, sv, sd) = direction.decompose(seg_start.x, seg_start.y, seg_start.z);
                    let (eu, ev, ed) = direction.decompose(seg_end.x, seg_end.y, seg_end.z);
                    let (mu, mv, _) = direction.decompose(midpoint.x, midpoint.y, midpoint.z);
                    let grid = self.ensure_grid(direction);
                    queue.push(
                        grid,
                        radius,
                        StampJob {
                            start: (su, sv, sd),
                            end: (eu, ev, ed),
                            mid_u: mu,
                            mid_v: mv,
                            sample_slot: slot,
                        },
                    );
                    if queue.batch_is_due() {
                        run_batch_into_samples(
                            queue,
                            grid,
                            lut,
                            radius,
                            from_high,
                            params.capture_arc_engagement,
                            air_mip,
                            cancel,
                            samples,
                            cutter,
                            flute_length,
                        )?;
                    }
                }
            }
        }
        Ok(())
    }
}

/// Emit one cutting subsegment's sample with its metric fields at their zero
/// values, and return the slot those metrics will be written into.
///
/// **This is the single enumerator.** Both dispatch shapes push a sample here
/// and nowhere else, so the stream's order, its `sample_index` numbering and
/// its timings are identical between them by construction rather than by
/// keeping two loops in step — which `DELTA_sim_w2.md` §3f called the hardest
/// part of the restructure, correctly.
#[allow(clippy::too_many_arguments)]
fn push_cutting_sample(
    samples: &mut Vec<SimulationCutSample>,
    next_sample_index: &mut usize,
    params: &CuttingCaptureParams<'_>,
    midpoint: P3,
    segment_time_s: f64,
    cumulative_time_s: f64,
    chipload_mm_per_tooth: f64,
) -> usize {
    let slot = samples.len();
    samples.push(SimulationCutSample {
        toolpath_id: params.toolpath_id,
        move_index: params.move_index,
        sample_index: *next_sample_index,
        position: [midpoint.x, midpoint.y, midpoint.z],
        cumulative_time_s,
        segment_time_s,
        is_cutting: true,
        cut_kinematics: params.cut_kinematics,
        feed_rate_mm_min: params.feed_rate_mm_min,
        spindle_rpm: params.spindle_rpm,
        flute_count: params.flute_count,
        // Every field below is overwritten by `apply_subsegment_metrics`
        // before the run returns; none of these placeholders can be observed.
        axial_doc_mm: 0.0,
        axial_engagement_mm: 0.0,
        plunge_descent_mm: 0.0,
        arc_engagement_radians: None,
        chipload_mm_per_tooth,
        effective_chip_thickness_mm: None,
        engagement: crate::simulation_cut::Engagement::with_radial_woc(0.0),
        removed_volume_est_mm3: 0.0,
        mrr_mm3_s: 0.0,
        semantic_item_id: params.semantic_item_id,
        span_path: params.span_path.to_vec(),
        in_transit_span: params.in_transit_span,
        source_intent: params.source_intent,
    });
    *next_sample_index += 1;
    slot
}

/// Write one subsegment's four published stamp metrics, and everything derived
/// from them, onto its already-emitted sample.
///
/// The other half of the single-enumerator argument: both dispatch shapes end
/// here, so a divergence would have to be a divergence in the *stamp*, which is
/// what the bit-identity sentries test.
fn apply_subsegment_metrics(
    sample: &mut SimulationCutSample,
    cutter: &dyn MillingCutter,
    flute_length: f64,
    metrics: (f64, f64, Option<f64>, f64),
) {
    let (measured_axial_mm, radial_engagement, arc_engagement_radians, removed_volume_est_mm3) =
        metrics;
    let (axial_engagement_mm, plunge_descent_mm) = if sample.cut_kinematics == CutKinematics::Plunge
    {
        (0.0, measured_axial_mm)
    } else {
        (measured_axial_mm, 0.0)
    };
    // F-4: one chip-model evaluation, both statistics off it. The arc-MEAN is
    // what the chipload gate reads as `effective_chip_thickness_mm`; the
    // arc-PEAK is what the `Engagement` vector's `peak_chip_thickness_mm` is
    // documented to carry. Pre-fix the peak slot held the mean and the mean
    // slot held the commanded advance per tooth.
    let chip_stats = chip_thickness_stats(
        cutter,
        axial_engagement_mm,
        arc_engagement_radians,
        sample.chipload_mm_per_tooth,
        sample.flute_count,
    );
    sample.axial_doc_mm = axial_engagement_mm;
    sample.axial_engagement_mm = axial_engagement_mm;
    sample.plunge_descent_mm = plunge_descent_mm;
    sample.arc_engagement_radians = arc_engagement_radians;
    sample.effective_chip_thickness_mm = chip_stats.map(|stats| stats.mean_mm);
    sample.engagement = crate::simulation_cut::Engagement {
        radial_woc_fraction: radial_engagement,
        // Always measured on this path: the cutter has a flute length, so the
        // fraction is defined (C2 — `None` is reserved for emitters that have
        // nothing to divide by).
        axial_doc_fraction: Some((axial_engagement_mm / flute_length).clamp(0.0, 1.0)),
        arc_radians: arc_engagement_radians,
        // F-4 (census T1.3, Checkpoint B Q2): these two carried each other's
        // values — `mean_` held the commanded advance per tooth (already
        // published as `chipload_mm_per_tooth` on the sample) and `peak_` held
        // the arc-MEAN chip, so the "peak" read *below* the "mean" on every
        // partial-immersion cut. Both now come off the shipped chip model under
        // their own names.
        mean_chip_thickness_mm: chip_stats.map(|stats| stats.mean_mm),
        peak_chip_thickness_mm: chip_stats.map(|stats| stats.peak_mm),
        leading_edge_speed_mm_min: sample.feed_rate_mm_min,
        // Step 2 carries direction as a substrate; climb/conventional
        // discrimination needs perp-axis side info from stamping (which side of
        // the engaged arc has fresh material) — to be threaded in a follow-up.
        // `Mixed` is the safe fallback.
        direction: crate::simulation_cut::EngagementDirection::Mixed,
    };
    sample.removed_volume_est_mm3 = removed_volume_est_mm3;
    sample.mrr_mm3_s = if sample.segment_time_s <= 1e-9 {
        0.0
    } else {
        removed_volume_est_mm3 / sample.segment_time_s
    };
}

/// Run one queued batch and patch its metrics onto the sample stream.
///
/// Split out only so the closure that borrows `samples` has one construction
/// site, shared by the mid-toolpath flush and the final one.
#[allow(clippy::too_many_arguments)]
fn run_batch_into_samples(
    queue: &mut BandDispatch,
    grid: &mut DexelGrid,
    lut: &RadialProfileLUT,
    radius: f64,
    from_high: bool,
    capture_arc_engagement: bool,
    air_mip: &mut Option<TileMaxTop>,
    cancel: &dyn CancelCheck,
    samples: &mut [SimulationCutSample],
    cutter: &dyn MillingCutter,
    flute_length: f64,
) -> Result<(), Cancelled> {
    queue.run_batch(
        grid,
        lut,
        radius,
        from_high,
        capture_arc_engagement,
        air_mip,
        cancel,
        |slot, metrics| {
            if let Some(sample) = samples.get_mut(slot) {
                apply_subsegment_metrics(sample, cutter, flute_length, metrics);
            }
        },
    )
}

/// Run one queued swept batch and patch its metrics onto the sample stream.
/// The twin of [`run_batch_into_samples`]; both end in the same
/// [`apply_subsegment_metrics`], so a divergence would have to be a divergence
/// in the stamp.
#[allow(clippy::too_many_arguments)]
fn run_swept_batch_into_samples(
    queue: &mut SweptDispatch,
    grid: &mut DexelGrid,
    lut: &RadialProfileLUT,
    radius: f64,
    from_high: bool,
    capture_arc_engagement: bool,
    air_mip: &mut Option<TileMaxTop>,
    cancel: &dyn CancelCheck,
    samples: &mut [SimulationCutSample],
    cutter: &dyn MillingCutter,
    flute_length: f64,
) -> Result<(), Cancelled> {
    queue.run_batch(
        grid,
        lut,
        radius,
        from_high,
        capture_arc_engagement,
        air_mip,
        cancel,
        |slot, metrics| {
            if let Some(sample) = samples.get_mut(slot) {
                apply_subsegment_metrics(sample, cutter, flute_length, metrics);
            }
        },
    )
}

impl TriDexelStock {
    /// The S1 enumerator: same sample stream, chunked geometry.
    ///
    /// The subsegment loop, the subdivision arithmetic and every argument to
    /// [`push_cutting_sample`] are **verbatim** from
    /// [`Self::capture_cutting_segment`]. What differs is only what happens to
    /// the geometry: instead of queueing one stamp per subsegment, consecutive
    /// subsegments of the same [`ChunkKind`] are accumulated into one
    /// [`SweptJob`] and stamped in a single pass.
    ///
    /// A chunk is closed whenever the class changes, the chunk stops being
    /// worth stamping in one pass (`SweptDispatch::chunk_may_grow`), or a
    /// subsegment is skipped — the last one because a chunk's sample slots are
    /// `first_slot..first_slot + bins` and a skipped subsegment would put a
    /// hole in that run.
    #[allow(clippy::too_many_arguments)]
    fn capture_cutting_segment_swept(
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
        air_mip: &mut Option<TileMaxTop>,
        queue: &mut SweptDispatch,
        flute_length: f64,
    ) -> Result<(), Cancelled> {
        let segment_length = (end - start).norm();
        if segment_length <= 1e-9 {
            return Ok(());
        }
        let from_high = direction.cuts_from_high_side();
        let chipload = chipload_mm_per_tooth(
            params.feed_rate_mm_min,
            params.spindle_rpm,
            params.flute_count,
        );

        // Verbatim from `capture_cutting_segment`. The subdivision still sets
        // the SAMPLE rate — S1 changes how the geometry is stamped, not how
        // many samples a move emits, so the two dispatch shapes stay
        // comparable sample-for-sample.
        const MAX_SUBSEGMENT_Z_DROP_MM: f64 = 0.02;
        let z_drop = (end.z - start.z).abs();
        let by_length = (segment_length / params.sample_step_mm).ceil() as usize;
        let by_z = (z_drop / MAX_SUBSEGMENT_Z_DROP_MM).ceil() as usize;
        let subsegments = by_length.max(by_z).max(1);

        let m_start = direction.decompose(start.x, start.y, start.z);
        let m_end = direction.decompose(end.x, end.y, end.z);
        let cs = self.z_grid.cell_size;
        let mut pending: Option<SweptJob> = None;

        for subsegment in 0..subsegments {
            check_cancel(cancel)?;
            let t0 = subsegment as f64 / subsegments as f64;
            let t1 = (subsegment + 1) as f64 / subsegments as f64;
            let seg_start = lerp_point(start, end, t0);
            let seg_end = lerp_point(start, end, t1);
            let midpoint = lerp_point(seg_start, seg_end, 0.5);
            let segment_len = (seg_end - seg_start).norm();
            if segment_len <= 1e-9 {
                if let Some(job) = pending.take() {
                    self.flush_swept_job(
                        job,
                        queue,
                        direction,
                        lut,
                        radius,
                        from_high,
                        params.capture_arc_engagement,
                        air_mip,
                        cancel,
                        samples,
                        cutter,
                        flute_length,
                    )?;
                }
                continue;
            }
            let segment_time_s = (segment_len / params.feed_rate_mm_min.max(1.0)) * 60.0;
            *cumulative_time_s += segment_time_s;
            let slot = push_cutting_sample(
                samples,
                next_sample_index,
                &params,
                midpoint,
                segment_time_s,
                *cumulative_time_s,
                chipload,
            );

            let kind = classify_subsegment(m_start, m_end, subsegments, subsegment);
            let extend = match pending.as_ref() {
                Some(job) => {
                    job.kind == kind
                        && job.bin0 + job.bins == subsegment
                        && job.first_slot + job.bins == slot
                        && queue.chunk_may_grow(job, radius, cs)
                }
                None => false,
            };
            if extend {
                if let Some(job) = pending.as_mut() {
                    job.bins += 1;
                }
            } else {
                if let Some(job) = pending.take() {
                    self.flush_swept_job(
                        job,
                        queue,
                        direction,
                        lut,
                        radius,
                        from_high,
                        params.capture_arc_engagement,
                        air_mip,
                        cancel,
                        samples,
                        cutter,
                        flute_length,
                    )?;
                }
                pending = Some(SweptJob {
                    m_start,
                    m_end,
                    subsegments,
                    bin0: subsegment,
                    bins: 1,
                    first_slot: slot,
                    kind,
                });
            }
        }
        if let Some(job) = pending.take() {
            self.flush_swept_job(
                job,
                queue,
                direction,
                lut,
                radius,
                from_high,
                params.capture_arc_engagement,
                air_mip,
                cancel,
                samples,
                cutter,
                flute_length,
            )?;
        }
        Ok(())
    }

    /// Queue one finished chunk, and run the batch if it is due.
    #[allow(clippy::too_many_arguments)]
    fn flush_swept_job(
        &mut self,
        job: SweptJob,
        queue: &mut SweptDispatch,
        direction: StockCutDirection,
        lut: &RadialProfileLUT,
        radius: f64,
        from_high: bool,
        capture_arc_engagement: bool,
        air_mip: &mut Option<TileMaxTop>,
        cancel: &dyn CancelCheck,
        samples: &mut [SimulationCutSample],
        cutter: &dyn MillingCutter,
        flute_length: f64,
    ) -> Result<(), Cancelled> {
        let grid = self.ensure_grid(direction);
        queue.push(grid, radius, job);
        if queue.batch_is_due() {
            run_swept_batch_into_samples(
                queue,
                grid,
                lut,
                radius,
                from_high,
                capture_arc_engagement,
                air_mip,
                cancel,
                samples,
                cutter,
                flute_length,
            )?;
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
        air_mip: &mut Option<TileMaxTop>,
        band_scratch: &mut Vec<StampPartial>,
    ) -> (f64, f64, Option<f64>, f64) {
        let (su, sv, sd) = direction.decompose(seg_start.x, seg_start.y, seg_start.z);
        let (eu, ev, ed) = direction.decompose(seg_end.x, seg_end.y, seg_end.z);
        let (mu, mv, _md) = direction.decompose(midpoint.x, midpoint.y, midpoint.z);
        let from_high = direction.cuts_from_high_side();
        let grid = self.ensure_grid(direction);
        if let Some(m) = air_mip.as_mut() {
            m.refresh_if_due(grid);
        }

        // S3: one stamp, decomposed into row bands. The bands are the same
        // whether they run on one thread or many — `stamp_wants_threads` picks
        // a SCHEDULE, never a decomposition — so this choice cannot move a
        // result. Partials are collected in band order and folded in that
        // order, so the reassociation of the volume sums is fixed by the grid
        // geometry rather than by which worker finished first.
        let start = (su, sv, sd);
        let end = (eu, ev, ed);
        let span = band::stamp_row_span(grid, radius, start, end);
        let parallel = span.is_some() && band::stamp_wants_threads(grid, radius, start, end);
        let mip = air_mip.as_ref();
        match (span, parallel) {
            // A stamp whose rows miss the grid visits no band at all.
            (None, _) => band_scratch.clear(),
            (Some((row_lo, row_hi)), true) => {
                grid.par_bands(row_lo, row_hi)
                    .map(|mut band| {
                        stamp_segment_with_metrics(
                            &mut band, lut, radius, start, end, mu, mv, from_high, mip,
                        )
                    })
                    .collect_into_vec(band_scratch);
            }
            (Some((row_lo, row_hi)), false) => {
                band_scratch.clear();
                for mut band in grid.serial_bands(row_lo, row_hi) {
                    band_scratch.push(stamp_segment_with_metrics(
                        &mut band, lut, radius, start, end, mu, mv, from_high, mip,
                    ));
                }
            }
        }
        let mut reduced = StampPartial::empty();
        for partial in band_scratch.iter() {
            reduced.merge(partial);
        }
        // Restricting the fan-out to the stamp's own rows means a stamp whose
        // footprint misses the grid entirely visits NO band, so these two —
        // which are properties of the segment rather than of any cell — have
        // to come from the driver, not from a band that may never run.
        if (eu - su) * (eu - su) + (ev - sv) * (ev - sv) < 1e-20 {
            reduced.degenerate = true;
            reduced.descent = (sd - ed).abs();
        }
        if let Some(m) = air_mip.as_mut() {
            m.absorb(&reduced);
        }
        reduced.finish(radius, capture_arc_engagement)
    }
}

/// Per-sample chip thickness, published as
/// `SimulationCutSample::effective_chip_thickness_mm`.
///
/// **The chipload gate no longer compares this against the vendor LUT
/// band** (2026-08-06). The vendor column was verified from primary
/// sources to be an *advance per tooth*, not a chip thickness, so the
/// gate's observation became `effective_feed ÷ (rpm · flutes)` and the
/// chip-geometry step was deleted rather than inverted. What survives
/// here: this value still feeds the gate's *sample-validity* predicate
/// (a sample with no resolvable chip model is still refused), and it is
/// still the honest per-sample chip figure for anyone who wants one.
/// The vestigial predicate is recorded in `tool_load::chipload`'s own
/// docs, with its own re-open condition; widening it moves the gate's
/// population, so it was deliberately left byte-identical across the
/// conversion.
///
/// Convention: AVERAGE chip thickness across the engagement arc
/// (`geometry.mean_chip_thickness_mm`), not the peak instantaneous
/// value at the most favorable angle. Returning the peak
/// (`geometry.max_chip_thickness_mm`) overstates by ~2.6× at half
/// immersion (`arc = π/2`). See
/// `tests/chipload_formula_calibration.rs`.
pub fn effective_chip_thickness_mm(
    cutter: &dyn MillingCutter,
    axial_doc_mm: f64,
    arc_engagement_radians: Option<f64>,
    feed_per_tooth_mm: f64,
    flute_count: u32,
) -> Option<f64> {
    chip_thickness_stats(
        cutter,
        axial_doc_mm,
        arc_engagement_radians,
        feed_per_tooth_mm,
        flute_count,
    )
    .map(|stats| stats.mean_mm)
}

/// PEAK (maximum instantaneous) chip thickness across the engagement
/// arc — `ChipGeometry::max_chip_thickness_mm`, the thickness the flute
/// sees at its most-engaged angular position.
///
/// The sibling of [`effective_chip_thickness_mm`], added by F-4 (census
/// T1.3). It is **not** the value any gate compares against: the
/// chipload gate is deliberately calibrated on the arc-average (see the
/// note on [`effective_chip_thickness_mm`], and
/// `tests/chipload_formula_calibration.rs`). Its consumer is the
/// report-only `Engagement::peak_chip_thickness_mm`, whose doc comment
/// has always described this quantity while the field carried the mean.
///
/// Below full slotting `peak > mean` always, by the closed form
/// `mean/peak = (2/arc)·(1 − cos(arc/2))`.
pub fn peak_chip_thickness_mm(
    cutter: &dyn MillingCutter,
    axial_doc_mm: f64,
    arc_engagement_radians: Option<f64>,
    feed_per_tooth_mm: f64,
    flute_count: u32,
) -> Option<f64> {
    chip_thickness_stats(
        cutter,
        axial_doc_mm,
        arc_engagement_radians,
        feed_per_tooth_mm,
        flute_count,
    )
    .map(|stats| stats.peak_mm)
}

/// The two chip-thickness statistics the simulator publishes, from one
/// [`MillingCutter::chip_geometry`] evaluation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChipThicknessStats {
    /// Arc-AVERAGE chip thickness (mm) — `ChipGeometry::mean_chip_thickness_mm`.
    pub mean_mm: f64,
    /// Arc-PEAK chip thickness (mm) — `ChipGeometry::max_chip_thickness_mm`.
    pub peak_mm: f64,
}

/// Evaluate the shipped chip model once and return both statistics.
/// `None` when there is no engagement arc (Z-only moves) or the cutter
/// declines the geometry.
pub fn chip_thickness_stats(
    cutter: &dyn MillingCutter,
    axial_doc_mm: f64,
    arc_engagement_radians: Option<f64>,
    feed_per_tooth_mm: f64,
    flute_count: u32,
) -> Option<ChipThicknessStats> {
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
        .map(|geometry| ChipThicknessStats {
            mean_mm: geometry.mean_chip_thickness_mm,
            peak_mm: geometry.max_chip_thickness_mm,
        })
}

/// Per-subsegment Z-drop cap. Kept in lockstep with the constant of the same
/// name inside [`TriDexelStock::capture_cutting_segment`] — see the reasoning
/// there. Duplicated rather than shared because the two live at different
/// scopes and the estimator must not be able to change the stamp.
const ESTIMATOR_MAX_SUBSEGMENT_Z_DROP_MM: f64 = 0.02;

/// How many `SimulationCutSample`s a toolpath will emit, near enough to
/// reserve for (PERF_REVIEW S6).
///
/// The old reserve was `moves.len() * 2`, which is not an estimate of anything
/// — the emitter pushes **one sample per subsegment**, and a move is cut into
/// `max(⌈len/sample_step⌉, ⌈|Δz|/0.02⌉)` of them. At the shipped sample steps
/// that is 5–20× the old figure on a raster pass and far more on a plunge, so
/// the Vec re-allocated and memcpy'd its way up through every doubling.
///
/// This walks the move list once (O(moves), against an O(moves × cells) stamp)
/// and reproduces the emitter's own arithmetic. It is deliberately an
/// **under**-estimate in one case and exact otherwise:
///
/// * arcs are counted by their **chord**, because the true count comes from
///   `linearize_arc_into` and reproducing it here would mean linearising every
///   arc twice. An under-reserve costs at most the doublings above the
///   estimate, which is what the old code paid on every move.
/// * `MoveIntent::Retract` Linear moves and rapids are counted by length only,
///   matching `sample_segment_runtime`, which has no Z-drop subdivision.
///
/// Never an over-estimate by construction, so no cap is needed: the Vec cannot
/// be asked to reserve more than the run will actually push.
#[allow(clippy::indexing_slicing)] // bounded by the loop range
fn estimate_sample_count(toolpath: &Toolpath, sample_step_mm: f64) -> usize {
    let step = sample_step_mm.max(1e-3);
    let mut total = 0usize;
    for move_index in 1..toolpath.moves.len() {
        let start = toolpath.moves[move_index - 1].target;
        let end = toolpath.moves[move_index].target;
        let length = (end - start).norm();
        if length <= 1e-9 {
            continue;
        }
        let by_length = (length / step).ceil() as usize;
        let mv = &toolpath.moves[move_index];
        // Length-only (no Z-drop subdivision): rapids, and Linear moves the
        // generator tagged `Retract` — both go through `sample_segment_runtime`.
        let length_only = matches!(mv.move_type, MoveType::Rapid)
            || (matches!(mv.intent, crate::toolpath::MoveIntent::Retract)
                && matches!(mv.move_type, MoveType::Linear { .. }));
        let n = if length_only {
            by_length
        } else {
            let by_z =
                ((end.z - start.z).abs() / ESTIMATOR_MAX_SUBSEGMENT_Z_DROP_MM).ceil() as usize;
            by_length.max(by_z)
        };
        total = total.saturating_add(n.max(1));
    }
    total
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
                ToolpathId(0),
                12_000,
                2,
                3000.0,
                2.0,
                None,
                &[],
                &[],
                true,
                &never_cancel,
            )
            .expect("simulation succeeds")
    }

    /// S6: the reserve must never ask for more than the run will push, and
    /// must not be the near-useless under-estimate it replaced.
    ///
    /// The over-estimate half is the load-bearing one — `estimate_sample_count`
    /// claims "never an over-estimate by construction", and a claim like that
    /// is worth exactly as much as the test that checks it. The lower bound
    /// (≥ half the actual) is what distinguishes this from `moves.len() * 2`,
    /// which under-reserves this very fixture by more than 10×.
    #[test]
    fn sample_count_estimate_never_exceeds_the_run() {
        let bbox = BoundingBox3 {
            min: P3::new(-20.0, -20.0, -5.0),
            max: P3::new(20.0, 20.0, 5.0),
        };
        let cutter = FlatEndmill::new(6.0, 20.0);
        let never_cancel = || false;

        // Laterals, a plunge (the `by_z` arm), a ramp (both arms) and a
        // rapid — every branch the estimator distinguishes.
        let mut toolpath = Toolpath::new();
        toolpath.rapid_to(P3::new(-10.0, -5.0, 2.0));
        toolpath.feed_to(P3::new(-10.0, -5.0, -1.0), 300.0); // plunge
        toolpath.feed_to(P3::new(10.0, -5.0, -1.0), 600.0); // lateral
        toolpath.feed_to(P3::new(10.0, 5.0, -2.5), 600.0); // ramp
        toolpath.feed_to(P3::new(-10.0, 5.0, -2.5), 600.0); // lateral
        toolpath.rapid_to(P3::new(-10.0, -5.0, 2.0));

        for step in [0.25_f64, 1.0, 4.0] {
            let mut stock = TriDexelStock::from_bounds(&bbox, 0.5);
            let samples = stock
                .simulate_toolpath_with_metrics_with_cancel(
                    &toolpath,
                    &cutter,
                    StockCutDirection::FromTop,
                    ToolpathId(0),
                    12_000,
                    2,
                    3000.0,
                    step,
                    None,
                    &[],
                    &[],
                    true,
                    &never_cancel,
                )
                .expect("simulation succeeds");
            let estimate = estimate_sample_count(&toolpath, step);
            assert!(
                estimate <= samples.len(),
                "step {step}: estimate {estimate} EXCEEDS the {} samples actually \
                 emitted — the reserve is documented as never over-estimating",
                samples.len()
            );
            assert!(
                estimate * 2 >= samples.len(),
                "step {step}: estimate {estimate} is less than half the {} samples \
                 emitted — that is the `moves.len() * 2` failure mode again",
                samples.len()
            );
        }
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
        // Tolerance relaxed from 0.12 → 0.5 rad under F.a sub-cell stamping
        // (DEXEL_Z_ONLY_INVESTIGATION.md §6.F / §8 Step 4). The perp-extent
        // coverage gate excludes the outermost annular band of cells whose
        // coverage < 1.0, so a true full slot reads radial ≈ 0.95 (limited
        // by grid discretization and the gate) instead of exactly 1.0.
        // Because `arc = arccos(1 - 2·radial)` has a near-vertical slope at
        // radial ≈ 1, that maps to arc ≈ 2.69 rad rather than π = 3.14.
        // The half-immersion test is unaffected (arccos has a finite slope
        // at radial = 0.5).
        assert!((arc - std::f64::consts::PI).abs() <= 0.5, "arc={arc}");
    }

    #[test]
    fn axial_metrics_are_segregated_by_kinematics() {
        let lateral = simulate_line(
            -20.0,
            20.0,
            P3::new(-10.0, 0.0, -1.0),
            P3::new(10.0, 0.0, -1.0),
        );
        assert!(
            lateral
                .iter()
                .any(|sample| sample.cut_kinematics == CutKinematics::Linear
                    && sample.axial_engagement_mm > 0.0
                    && sample.plunge_descent_mm == 0.0),
            "lateral samples should populate axial engagement only"
        );

        let bbox = BoundingBox3 {
            min: P3::new(-5.0, -5.0, -5.0),
            max: P3::new(5.0, 5.0, 5.0),
        };
        let mut stock = TriDexelStock::from_bounds(&bbox, 0.5);
        let cutter = FlatEndmill::new(3.0, 10.0);
        let mut toolpath = Toolpath::new();
        toolpath.rapid_to(P3::new(0.0, 0.0, 5.0));
        toolpath.feed_to(P3::new(0.0, 0.0, -1.0), 100.0);
        let never_cancel = || false;
        let plunge = stock
            .simulate_toolpath_with_metrics_with_cancel(
                &toolpath,
                &cutter,
                StockCutDirection::FromTop,
                ToolpathId(0),
                12_000,
                2,
                3000.0,
                1.0,
                None,
                &[],
                &[],
                true,
                &never_cancel,
            )
            .expect("simulation succeeds");
        assert!(
            plunge
                .iter()
                .any(|sample| sample.cut_kinematics == CutKinematics::Plunge
                    && sample.axial_engagement_mm == 0.0
                    && sample.axial_doc_mm == 0.0
                    && sample.plunge_descent_mm > 0.0),
            "plunge samples should populate plunge descent only"
        );
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
                ToolpathId(0),
                12_000,
                2,
                3000.0,
                1.0,
                None,
                &[],
                &[],
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
