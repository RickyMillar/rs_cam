//! G-SIMMEM (2026-09-30): the cut trace records at its declared sample step,
//! not at the stamper's Z subdivision.
//!
//! # The defect
//!
//! A cutting move is stamped in `max(⌈len/step⌉, ⌈|Δz|/0.02⌉)` subsegments.
//! The `0.02 mm` Z-drop cap is a STAMPING accuracy device (the per-cell stamp
//! surface ignores the Z change along a subsegment), but the metric walk also
//! pushed one 280-byte `SimulationCutSample` per subsegment. On a terrain
//! finish every sloped move is Z-limited, so the trace carried 5–6 samples per
//! declared sample step: 1.66 M samples against 0.29 M by length on the
//! rivmap100 scallop, and about 34 M (9.5 GB, one allocation) on the
//! 350 x 350 mm repro — the operator's 20 GB (`planning/sim_memory_2026-09-30/`).
//!
//! # The fix
//!
//! The stamping is untouched: same subsegments, same stamps, same stock. After
//! the subsegments' metrics are patched, each Z-subdivided move's run of
//! samples is COALESCED into groups of `g = ⌈subsegments / by_length⌉`, so a
//! move keeps at most `by_length` samples — the count the trace's own
//! `sample_step_mm` promises. A move with `by_length >= by_z` has `g = 1` and
//! its samples are untouched, bit for bit.
//!
//! # What a coalesced sample carries, and which consumer that preserves
//!
//! * **Sums** — `segment_time_s`, `removed_volume_est_mm3` (so every volume
//!   and time total is kept), `mrr_mm3_s` = their ratio;
//!   `cumulative_time_s` is the group's last, so the clock is unchanged.
//! * **Time-weighted means** — `engagement.radial_woc_fraction`,
//!   `axial_doc_mm`, `engagement.axial_doc_fraction`: the three readings the
//!   feed modulator averages per move with the same time weights
//!   (`session::compute` per-move engagement). Its per-move inputs, hence the
//!   modulated feeds and the cycle time, are kept.
//! * **Maxima** — `axial_engagement_mm` ("maximum material height engaged at
//!   this sample"), `plunge_descent_mm`, the arc and the chip thicknesses: the
//!   peaks the depth, deflection, chipload and power gates read. A peak is
//!   kept exactly or over-stated, never under-stated.
//! * **Identity fields** (move, feed, spans, intent, kinematics) are equal
//!   across a group; `position` is the time-weighted mean of the midpoints.
//! * **Material** (S1) — `material_slot` is the slot with the most removed
//!   volume over the group, each sample counted under its own main slot;
//!   `cuts_several_materials` is set when one sample has it set or when the
//!   samples that removed material name more than one slot.
//!
//! # Memory bound
//!
//! Coalescing needs final metrics, and a queued batch may still owe some. The
//! walk therefore coalesces when the samples it would drop reach
//! [`staging_budget`] — one dexel grid's worth of bytes — draining the stamp
//! queues first (a flush only splits a batch earlier, which the dispatchers
//! document as result-neutral). The raw tail above the stored trace is thus
//! bounded by the GRID, not by the path.

use crate::geo::P3;
use crate::stock::dexel::{DexelGrid, DexelRay};
use crate::stock::material_slot::SlotVolumes;
use crate::stock::simulation_cut::SimulationCutSample;

/// Per-subsegment Z-drop cap of the stamping subdivision. See
/// `TriDexelStock::capture_cutting_segment` for why it exists; it bounds the
/// STAMP error, not the sample rate.
pub(super) const MAX_SUBSEGMENT_Z_DROP_MM: f64 = 0.02;

/// `(subsegments, by_length)` for one cutting segment: the stamping
/// subdivision and the count the declared sample step alone would give.
///
/// The ONE copy of the arithmetic both capture routes (per-stamp and swept)
/// stamp with and the coalescer groups by.
pub(super) fn cutting_subdivision(
    start: P3,
    end: P3,
    segment_length: f64,
    sample_step_mm: f64,
) -> (usize, usize) {
    let z_drop = (end.z - start.z).abs();
    let by_length = (segment_length / sample_step_mm).ceil() as usize;
    let by_z = (z_drop / MAX_SUBSEGMENT_Z_DROP_MM).ceil() as usize;
    (by_length.max(by_z).max(1), by_length.max(1))
}

/// How many consecutive subsegment samples one stored sample covers.
pub(super) fn coalesce_group(subsegments: usize, by_length: usize) -> usize {
    subsegments.div_ceil(by_length.max(1)).max(1)
}

/// The most raw samples the walk holds above what it will keep before it
/// coalesces: as many samples as fit in the bytes of ONE dexel grid of the
/// stock being cut (`rows · cols` columns of one ray plus one
/// `conservative_top` each). The simulation already holds several grids
/// (the stock, its playback twin, a snapshot per operation), so this bounds
/// the transient by the grid instead of by the toolpath length.
pub(super) fn staging_budget(grid: &DexelGrid) -> usize {
    let columns = grid.rows.saturating_mul(grid.cols);
    let grid_bytes =
        columns.saturating_mul(std::mem::size_of::<DexelRay>() + std::mem::size_of::<f32>());
    (grid_bytes / std::mem::size_of::<SimulationCutSample>()).max(1)
}

/// One Z-subdivided segment's run of raw samples, `first..first + len`, to be
/// coalesced in groups of `group`.
#[derive(Clone, Copy, Debug)]
struct CoalesceRun {
    first: usize,
    len: usize,
    group: usize,
}

/// The walk's record of what to coalesce, and when.
pub(super) struct SampleCoalescer {
    runs: Vec<CoalesceRun>,
    /// Samples below this index are final and already coalesced.
    compacted: usize,
    /// Raw samples the pending runs will drop.
    pending_savings: usize,
    budget: usize,
}

impl SampleCoalescer {
    pub(super) fn new(grid: &DexelGrid) -> Self {
        Self {
            runs: Vec::new(),
            compacted: 0,
            pending_savings: 0,
            budget: staging_budget(grid),
        }
    }

    /// Record that `samples[first..first + len]` came from one segment cut in
    /// `subsegments` stamps where the sample step alone asks for `by_length`.
    pub(super) fn record(
        &mut self,
        first: usize,
        len: usize,
        subsegments: usize,
        by_length: usize,
    ) {
        let group = coalesce_group(subsegments, by_length);
        if group <= 1 || len <= 1 {
            return;
        }
        self.pending_savings += len - len.div_ceil(group);
        self.runs.push(CoalesceRun { first, len, group });
    }

    /// True when the pending raw samples reach the staging budget.
    pub(super) fn is_due(&self) -> bool {
        self.pending_savings >= self.budget
    }

    /// Coalesce every recorded run. EVERY sample from `compacted` on must be
    /// final (no queued stamp still owes it metrics). Renumbers
    /// `sample_index` to the new positions and returns the new length, which
    /// is the next sample index.
    pub(super) fn compact(&mut self, samples: &mut Vec<SimulationCutSample>) -> usize {
        if self.runs.is_empty() {
            self.compacted = samples.len();
            return samples.len();
        }
        let mut write = self.compacted;
        let mut read = self.compacted;
        for run in std::mem::take(&mut self.runs) {
            while read < run.first {
                samples.swap(write, read);
                write += 1;
                read += 1;
            }
            let end = run.first + run.len;
            while read < end {
                let chunk_end = (read + run.group).min(end);
                merge_into_first(samples, read, chunk_end);
                samples.swap(write, read);
                write += 1;
                read = chunk_end;
            }
        }
        while read < samples.len() {
            samples.swap(write, read);
            write += 1;
            read += 1;
        }
        samples.truncate(write);
        for (index, sample) in samples.iter_mut().enumerate().skip(self.compacted) {
            sample.sample_index = index;
        }
        self.compacted = samples.len();
        self.pending_savings = 0;
        samples.len()
    }
}

fn max_opt(a: Option<f64>, b: Option<f64>) -> Option<f64> {
    match (a, b) {
        (Some(x), Some(y)) => Some(x.max(y)),
        (x, None) => x,
        (None, y) => y,
    }
}

/// Fold `samples[first + 1..end]` into `samples[first]` by the rules in the
/// module doc. All of them are one segment's subsegments.
#[allow(clippy::indexing_slicing)] // first < end <= samples.len(), by the caller
fn merge_into_first(samples: &mut [SimulationCutSample], first: usize, end: usize) {
    if end - first <= 1 {
        return;
    }
    let mut t_sum = 0.0;
    let mut removed = 0.0;
    let mut radial_t = 0.0;
    let mut axial_doc_t = 0.0;
    let mut axial_frac_t = 0.0;
    let mut axial_frac_w = 0.0;
    let mut position_t = [0.0_f64; 3];
    let mut axial_engagement = f64::NEG_INFINITY;
    let mut plunge = f64::NEG_INFINITY;
    let mut arc: Option<f64> = None;
    let mut effective_chip: Option<f64> = None;
    let mut mean_chip: Option<f64> = None;
    let mut peak_chip: Option<f64> = None;
    let mut engagement_arc: Option<f64> = None;
    let mut by_slot = SlotVolumes::default();
    let mut several = false;
    for sample in &samples[first..end] {
        if sample.removed_volume_est_mm3 > 0.0 {
            by_slot.add(sample.material_slot, sample.removed_volume_est_mm3);
        }
        several |= sample.cuts_several_materials;
        let t = sample.segment_time_s;
        t_sum += t;
        removed += sample.removed_volume_est_mm3;
        radial_t += sample.engagement.radial_woc_fraction * t;
        axial_doc_t += sample.axial_doc_mm * t;
        if let Some(fraction) = sample.engagement.axial_doc_fraction {
            axial_frac_t += fraction * t;
            axial_frac_w += t;
        }
        for (acc, p) in position_t.iter_mut().zip(sample.position) {
            *acc += p * t;
        }
        axial_engagement = axial_engagement.max(sample.axial_engagement_mm);
        plunge = plunge.max(sample.plunge_descent_mm);
        arc = max_opt(arc, sample.arc_engagement_radians);
        effective_chip = max_opt(effective_chip, sample.effective_chip_thickness_mm);
        mean_chip = max_opt(mean_chip, sample.engagement.mean_chip_thickness_mm);
        peak_chip = max_opt(peak_chip, sample.engagement.peak_chip_thickness_mm);
        engagement_arc = max_opt(engagement_arc, sample.engagement.arc_radians);
    }
    let last_cumulative = samples[end - 1].cumulative_time_s;
    let head = &mut samples[first];
    // Every subsegment has a positive time (a positive length at a feed of at
    // least 1 mm/min), so `t_sum > 0` on every group the walk records; the
    // guard only keeps a degenerate group at its head's readings.
    if t_sum > 0.0 {
        head.engagement.radial_woc_fraction = radial_t / t_sum;
        head.axial_doc_mm = axial_doc_t / t_sum;
        head.position = position_t.map(|p| p / t_sum);
    }
    if axial_frac_w > 0.0 {
        head.engagement.axial_doc_fraction = Some(axial_frac_t / axial_frac_w);
    }
    head.segment_time_s = t_sum;
    head.cumulative_time_s = last_cumulative;
    head.removed_volume_est_mm3 = removed;
    head.mrr_mm3_s = if t_sum <= 1e-9 { 0.0 } else { removed / t_sum };
    head.axial_engagement_mm = axial_engagement;
    head.plunge_descent_mm = plunge;
    head.arc_engagement_radians = arc;
    head.effective_chip_thickness_mm = effective_chip;
    head.engagement.mean_chip_thickness_mm = mean_chip;
    head.engagement.peak_chip_thickness_mm = peak_chip;
    head.engagement.arc_radians = engagement_arc;
    let material = by_slot.material_cut();
    head.material_slot = material.slot;
    head.cuts_several_materials = several || material.several;
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
    use crate::stock::simulation_cut::Engagement;

    /// Subsegment `i` of a move cut in equal 0.01 s pieces.
    fn raw(i: usize, radial: f64, axial: f64, removed: f64) -> SimulationCutSample {
        SimulationCutSample {
            move_index: 7,
            sample_index: i,
            position: [i as f64, 0.0, -(i as f64)],
            segment_time_s: 0.01,
            cumulative_time_s: 0.01 * (i + 1) as f64,
            is_cutting: true,
            removed_volume_est_mm3: removed,
            axial_engagement_mm: axial,
            axial_doc_mm: axial,
            arc_engagement_radians: Some(axial),
            engagement: Engagement {
                axial_doc_fraction: Some(axial / 10.0),
                arc_radians: Some(axial),
                peak_chip_thickness_mm: Some(axial / 100.0),
                ..Engagement::with_radial_woc(radial)
            },
            ..SimulationCutSample::test_fixture()
        }
    }

    #[test]
    fn a_z_limited_move_keeps_the_count_its_length_asks_for() {
        // 1 mm long, 1 mm deep, step 0.5: 50 stamps, 2 samples.
        let (subsegments, by_length) =
            cutting_subdivision(P3::new(0.0, 0.0, 0.0), P3::new(0.0, 0.0, -1.0), 1.0, 0.5);
        assert_eq!((subsegments, by_length), (50, 2));
        assert_eq!(coalesce_group(subsegments, by_length), 25);
        // A flat move is never coalesced.
        let (s, b) = cutting_subdivision(P3::new(0.0, 0.0, 0.0), P3::new(3.0, 0.0, 0.0), 3.0, 0.5);
        assert_eq!(coalesce_group(s, b), 1);
    }

    #[test]
    fn coalescing_keeps_sums_means_and_peaks() {
        // One untouched sample before, a 5-subsegment run in groups of 2, one after.
        let mut samples = vec![raw(0, 0.9, 9.0, 9.0)];
        let radial = [0.1, 0.3, 0.5, 0.7, 0.2];
        let axial = [1.0, 4.0, 2.0, 3.0, 5.0];
        for i in 0..5 {
            samples.push(raw(i + 1, radial[i], axial[i], 1.0 + i as f64));
        }
        samples.push(raw(6, 0.8, 8.0, 8.0));
        let removed_before: f64 = samples.iter().map(|s| s.removed_volume_est_mm3).sum();
        let time_before: f64 = samples.iter().map(|s| s.segment_time_s).sum();
        let radial_t_before: f64 = samples
            .iter()
            .map(|s| s.engagement.radial_woc_fraction * s.segment_time_s)
            .sum();
        let last_clock = samples.last().unwrap().cumulative_time_s;

        let mut coalescer = SampleCoalescer {
            runs: Vec::new(),
            compacted: 0,
            pending_savings: 0,
            budget: 1,
        };
        // 10 stamps where the length asks for 4: groups of 3 -> 5 samples
        // become ceil(5 / 3) = 2.
        coalescer.record(1, 5, 10, 4);
        assert!(coalescer.is_due());
        let len = coalescer.compact(&mut samples);
        assert_eq!(len, 4);
        assert_eq!(samples.len(), 4);

        // Indices renumbered to positions; untouched neighbours intact.
        for (i, s) in samples.iter().enumerate() {
            assert_eq!(s.sample_index, i);
        }
        assert_eq!(samples[0].axial_engagement_mm, 9.0);
        assert_eq!(samples[3].axial_engagement_mm, 8.0);

        // Group 1 = subsegments 1..=3: radial mean, axial_doc mean, peaks max.
        let g = &samples[1];
        assert!((g.engagement.radial_woc_fraction - 0.3).abs() < 1e-12);
        assert!((g.axial_doc_mm - 7.0 / 3.0).abs() < 1e-12);
        assert_eq!(g.axial_engagement_mm, 4.0);
        assert_eq!(g.arc_engagement_radians, Some(4.0));
        assert_eq!(g.engagement.peak_chip_thickness_mm, Some(0.04));
        assert!((g.segment_time_s - 0.03).abs() < 1e-15);
        assert_eq!(g.removed_volume_est_mm3, 6.0);
        assert!((g.position[0] - 2.0).abs() < 1e-12);
        // Group 2 = subsegments 4..=5.
        assert_eq!(samples[2].axial_engagement_mm, 5.0);
        assert_eq!(samples[2].removed_volume_est_mm3, 9.0);

        // Totals the consumers read are unchanged.
        let removed_after: f64 = samples.iter().map(|s| s.removed_volume_est_mm3).sum();
        let time_after: f64 = samples.iter().map(|s| s.segment_time_s).sum();
        let radial_t_after: f64 = samples
            .iter()
            .map(|s| s.engagement.radial_woc_fraction * s.segment_time_s)
            .sum();
        assert!((removed_after - removed_before).abs() < 1e-12);
        assert!((time_after - time_before).abs() < 1e-12);
        assert!((radial_t_after - radial_t_before).abs() < 1e-12);
        assert_eq!(samples.last().unwrap().cumulative_time_s, last_clock);
        assert!((samples[2].cumulative_time_s - 0.06).abs() < 1e-15);
    }

    #[test]
    fn the_staging_budget_is_one_grid_of_bytes() {
        let grid = DexelGrid::z_grid_from_bounds(
            &crate::geo::BoundingBox3 {
                min: P3::new(0.0, 0.0, 0.0),
                max: P3::new(100.0, 50.0, 10.0),
            },
            0.5,
        );
        let columns = grid.rows * grid.cols;
        let per_column = std::mem::size_of::<DexelRay>() + std::mem::size_of::<f32>();
        let budget = staging_budget(&grid);
        assert_eq!(
            budget,
            columns * per_column / std::mem::size_of::<SimulationCutSample>()
        );
        assert!(budget * std::mem::size_of::<SimulationCutSample>() <= columns * per_column);
    }

    #[test]
    fn a_coalesced_sample_keeps_the_main_slot_by_volume_and_the_flag() {
        use crate::stock::material_slot::MaterialSlot;
        let with_slot = |i: usize, removed: f64, k: u8, several: bool| SimulationCutSample {
            material_slot: MaterialSlot(k),
            cuts_several_materials: several,
            ..raw(i, 0.5, 1.0, removed)
        };
        // Slot 2 removes 3.0 over two samples; the stock removes 2.5 in one.
        let mut samples = vec![
            with_slot(0, 1.5, 2, false),
            with_slot(1, 2.5, 0, false),
            with_slot(2, 1.5, 2, false),
        ];
        merge_into_first(&mut samples, 0, 3);
        assert_eq!(samples[0].material_slot, MaterialSlot(2));
        assert!(samples[0].cuts_several_materials);

        // One slot over the whole group: no flag.
        let mut samples = vec![with_slot(0, 1.0, 1, false), with_slot(1, 1.0, 1, false)];
        merge_into_first(&mut samples, 0, 2);
        assert_eq!(samples[0].material_slot, MaterialSlot(1));
        assert!(!samples[0].cuts_several_materials);

        // A flag on one sample stays on the group.
        let mut samples = vec![with_slot(0, 1.0, 1, true), with_slot(1, 1.0, 1, false)];
        merge_into_first(&mut samples, 0, 2);
        assert!(samples[0].cuts_several_materials);
    }
}
