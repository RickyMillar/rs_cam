//! Sample accumulation: the per-toolpath and per-kinematics accumulators
//! and the span grouping that feeds them.

use super::{
    CutKinematics, KinematicsAccumulator, KinematicsSummary, SimulationCutSample,
    SimulationCutSummary, SimulationToolpathCutSummary, SummaryAccumulator,
};
use crate::ids::ToolpathId;
use crate::trace::toolpath_spans::SpanId;
use std::collections::BTreeMap;

/// Fold the array-of-accumulators into a sparse `BTreeMap` of finished
/// summaries, dropping empty (no-sample) entries. The reporting layer wants
/// "only the kinematics classes that were observed"; the accumulation hot
/// path wants O(1) array indexing.
pub(crate) fn finalize_per_kinematics(
    accs: [KinematicsAccumulator; CutKinematics::COUNT],
) -> BTreeMap<CutKinematics, KinematicsSummary> {
    let mut out = BTreeMap::new();
    for (kind, acc) in CutKinematics::ALL.into_iter().zip(accs) {
        if acc.sample_count > 0 {
            out.insert(kind, acc.finish());
        }
    }
    out
}

impl KinematicsAccumulator {
    fn observe_cutting(&mut self, sample: &SimulationCutSample) {
        let dt = sample.segment_time_s;
        self.cutting_runtime_s += dt;
        self.sample_count += 1;
        let eng = &sample.engagement;
        self.radial_woc_time_weighted_sum += eng.radial_woc_fraction * dt;
        self.peak_radial_woc_fraction = self.peak_radial_woc_fraction.max(eng.radial_woc_fraction);
        if let Some(axial) = eng.axial_doc_fraction {
            self.axial_doc_fraction_time_weighted_sum += axial * dt;
            self.axial_doc_observed_runtime_s += dt;
            self.peak_axial_doc_fraction =
                Some(self.peak_axial_doc_fraction.map_or(axial, |p| p.max(axial)));
        }
        // P3: transit-span samples produce dexel-bridge artifacts on peak
        // DOC. Defer to the same gating the top-level accumulator uses.
        if !sample.in_transit_span {
            self.peak_axial_doc_mm = self
                .peak_axial_doc_mm
                .max(sample.axial_engagement_mm.max(0.0));
        }
        self.peak_plunge_descent_mm = self
            .peak_plunge_descent_mm
            .max(sample.plunge_descent_mm.max(0.0));
        if let Some(arc) = eng.arc_radians {
            self.arc_time_weighted_sum += arc * dt;
            self.arc_observed_runtime_s += dt;
        }
        if let Some(mct) = eng.mean_chip_thickness_mm {
            self.mean_chip_thickness_time_weighted_sum += mct * dt;
            self.mean_chip_thickness_observed_runtime_s += dt;
        }
        if let Some(pct) = eng.peak_chip_thickness_mm {
            self.peak_chip_thickness_mm = self.peak_chip_thickness_mm.max(pct);
        }
        self.leading_edge_speed_time_weighted_sum += eng.leading_edge_speed_mm_min * dt;
    }

    pub fn finish(self) -> KinematicsSummary {
        let t = self.cutting_runtime_s.max(1e-12);
        KinematicsSummary {
            cutting_runtime_s: self.cutting_runtime_s,
            average_radial_woc_fraction: if self.cutting_runtime_s > 1e-9 {
                self.radial_woc_time_weighted_sum / t
            } else {
                0.0
            },
            peak_radial_woc_fraction: self.peak_radial_woc_fraction,
            average_axial_doc_fraction: if self.axial_doc_observed_runtime_s > 1e-9 {
                Some(self.axial_doc_fraction_time_weighted_sum / self.axial_doc_observed_runtime_s)
            } else {
                None
            },
            peak_axial_doc_fraction: self.peak_axial_doc_fraction,
            peak_axial_doc_mm: self.peak_axial_doc_mm,
            peak_plunge_descent_mm: self.peak_plunge_descent_mm,
            average_arc_radians: if self.arc_observed_runtime_s > 1e-9 {
                Some(self.arc_time_weighted_sum / self.arc_observed_runtime_s)
            } else {
                None
            },
            average_mean_chip_thickness_mm: if self.mean_chip_thickness_observed_runtime_s > 1e-9 {
                Some(
                    self.mean_chip_thickness_time_weighted_sum
                        / self.mean_chip_thickness_observed_runtime_s,
                )
            } else {
                None
            },
            peak_chip_thickness_mm: if self.peak_chip_thickness_mm > 0.0 {
                Some(self.peak_chip_thickness_mm)
            } else {
                None
            },
            average_leading_edge_speed_mm_min: if self.cutting_runtime_s > 1e-9 {
                self.leading_edge_speed_time_weighted_sum / t
            } else {
                0.0
            },
            sample_count: self.sample_count,
        }
    }
}

impl SummaryAccumulator {
    pub fn observe(&mut self, sample: &SimulationCutSample) {
        self.sample_count += 1;
        self.total_runtime_s += sample.segment_time_s;
        self.total_removed_volume_est_mm3 += sample.removed_volume_est_mm3.max(0.0);
        self.peak_engagement = self
            .peak_engagement
            .max(sample.engagement.radial_woc_fraction.max(0.0));
        // P3: skip extreme-value updates for transit-span samples. The
        // dexel reports `stock_top − cutter_z` over uncleared neighbouring
        // stock during link bridges, helix entries, lead-outs, and
        // waterline-cleanup spans — not steady-state engagement. Including
        // those samples inflates peak DOC to multiples of the configured
        // depth_per_pass (Wanaka TP3 reads 18.59 mm on a 6 mm tool).
        // See `planning/P3_TRANSIT_PEAK_DOC_RCA.md`.
        if !sample.in_transit_span {
            self.peak_chipload_mm_per_tooth = self
                .peak_chipload_mm_per_tooth
                .max(sample.chipload_mm_per_tooth.max(0.0));
            self.peak_axial_doc_mm = self
                .peak_axial_doc_mm
                .max(sample.axial_engagement_mm.max(0.0));
        }
        self.peak_plunge_descent_mm = self
            .peak_plunge_descent_mm
            .max(sample.plunge_descent_mm.max(0.0));
        self.peak_mrr_mm3_s = self.peak_mrr_mm3_s.max(sample.mrr_mm3_s.max(0.0));

        if sample.is_cutting {
            self.cutting_runtime_s += sample.segment_time_s;
            self.engagement_time_weighted_sum +=
                sample.engagement.radial_woc_fraction * sample.segment_time_s;
            if sample.engagement.radial_woc_fraction < 0.02 {
                self.air_cut_time_s += sample.segment_time_s;
                self.air_cut_issue_count += 1;
            } else if sample.engagement.radial_woc_fraction < 0.10 {
                self.low_engagement_time_s += sample.segment_time_s;
                self.low_engagement_issue_count += 1;
            }
            // Step 2 D substrate: route cutting samples to their kinematics
            // sub-accumulator. `Rapid` is skipped (samples emitted as
            // `is_cutting=true` with `Rapid` kinematics would be a bug in the
            // emitter; we honour the `is_cutting` gate first). `Linear`
            // samples with `MoveIntent::Retract` were already reclassified
            // to `is_cutting=false` in Step 1, so they don't reach this
            // branch.
            #[allow(clippy::indexing_slicing)]
            // SAFETY: `cut_kinematics.index()` is bounded by COUNT.
            self.per_kinematics[sample.cut_kinematics.index()].observe_cutting(sample);
        } else {
            self.rapid_runtime_s += sample.segment_time_s;
        }
    }

    pub fn average_engagement(&self) -> f64 {
        if self.cutting_runtime_s <= 1e-9 {
            0.0
        } else {
            self.engagement_time_weighted_sum / self.cutting_runtime_s
        }
    }

    pub fn average_mrr(&self) -> f64 {
        if self.cutting_runtime_s <= 1e-9 {
            0.0
        } else {
            self.total_removed_volume_est_mm3 / self.cutting_runtime_s
        }
    }

    pub fn finish_toolpath(self, toolpath_id: ToolpathId) -> SimulationToolpathCutSummary {
        let average_engagement = self.average_engagement();
        let average_mrr_mm3_s = self.average_mrr();
        let per_kinematics = finalize_per_kinematics(self.per_kinematics);
        SimulationToolpathCutSummary {
            toolpath_id,
            sample_count: self.sample_count,
            total_runtime_s: self.total_runtime_s,
            cutting_runtime_s: self.cutting_runtime_s,
            rapid_runtime_s: self.rapid_runtime_s,
            air_cut_time_s: self.air_cut_time_s,
            low_engagement_time_s: self.low_engagement_time_s,
            average_engagement,
            peak_chipload_mm_per_tooth: self.peak_chipload_mm_per_tooth,
            peak_axial_doc_mm: self.peak_axial_doc_mm,
            peak_plunge_descent_mm: self.peak_plunge_descent_mm,
            total_removed_volume_est_mm3: self.total_removed_volume_est_mm3,
            average_mrr_mm3_s,
            metrics_not_applicable: false,
            per_kinematics,
            runtime_by_intent: None,
        }
    }

    pub(super) fn finish_summary(
        self,
        sample_count: usize,
        toolpath_count: usize,
        issue_count: usize,
        hotspot_count: usize,
    ) -> SimulationCutSummary {
        let average_engagement = self.average_engagement();
        let average_mrr_mm3_s = self.average_mrr();
        let per_kinematics = finalize_per_kinematics(self.per_kinematics);
        SimulationCutSummary {
            sample_count,
            toolpath_count,
            issue_count,
            hotspot_count,
            total_runtime_s: self.total_runtime_s,
            cutting_runtime_s: self.cutting_runtime_s,
            rapid_runtime_s: self.rapid_runtime_s,
            air_cut_time_s: self.air_cut_time_s,
            low_engagement_time_s: self.low_engagement_time_s,
            average_engagement,
            peak_chipload_mm_per_tooth: self.peak_chipload_mm_per_tooth,
            peak_axial_doc_mm: self.peak_axial_doc_mm,
            peak_plunge_descent_mm: self.peak_plunge_descent_mm,
            total_removed_volume_est_mm3: self.total_removed_volume_est_mm3,
            average_mrr_mm3_s,
            per_kinematics,
            runtime_by_intent: None,
        }
    }
}

/// Scatter one toolpath's samples into per-span accumulators in **one pass**
/// over the sample vector.
///
/// Returns a vector of length `span_count`, indexed by span id. A span with
/// no samples comes back as a `SummaryAccumulator::default()` whose
/// `sample_count` is `0` — callers filter on that, exactly as the per-span
/// loop they are replacing did.
///
/// **A sample belongs to every span in its `span_path`, not to one of them.**
/// Spans nest (Operation ⊃ Region ⊃ DepthPass ⊃ Entry …), and the per-span
/// summaries are read as "everything that happened inside this span", so the
/// scatter fans each sample out across its whole path. That is the one
/// behavioural detail a single-pass rewrite can get wrong, and it is what
/// separates this from `build_per_depth_pass_summary`'s sibling loop, which
/// picks the *first* matching id because depth passes do not nest.
///
/// `accept` optionally restricts which span ids accumulate (the
/// `get_cut_trace` span filter). `None` accumulates every span.
///
/// # Why this exists
///
/// The GUI's `get_cut_trace` used to run this as a nested loop — for every
/// span, a full scan of the project's entire sample vector — so a bare
/// `get_cut_trace()` cost `Σ_toolpaths (spans × total_samples)`, on the egui
/// frame-loop thread, with every other queued MCP request waiting behind it.
/// An accidental quadratic, diagnosed as C1 in
/// `planning/review_2026-08-04/FINISHING_OPEN_DEFECTS_EVIDENCE.md` §3.D.2.
/// Cost here is `Σ_samples |span_path|` plus `span_count`, i.e. linear in the
/// trace with a small constant.
pub fn accumulate_by_span(
    samples: &[SimulationCutSample],
    toolpath_id: ToolpathId,
    span_count: usize,
    accept: Option<&std::collections::HashSet<u32>>,
) -> Vec<SummaryAccumulator> {
    let mut accs: Vec<SummaryAccumulator> = (0..span_count)
        .map(|_| SummaryAccumulator::default())
        .collect();
    if span_count == 0 {
        return accs;
    }
    for sample in samples.iter().filter(|s| s.toolpath_id == toolpath_id) {
        for (pos, &SpanId(id)) in sample.span_path.iter().enumerate() {
            // The loop this replaces asked `span_path.contains(id)` once per
            // span, so a path that repeats an id counted the sample ONCE.
            // Preserve that: skip an id already seen earlier in this path.
            if sample.span_path.iter().take(pos).any(|&SpanId(p)| p == id) {
                continue;
            }
            if accept.is_some_and(|set| !set.contains(&id)) {
                continue;
            }
            let Some(acc) = accs.get_mut(id as usize) else {
                // A span id past the end of this toolpath's span table.
                // Dropped rather than panicking: span tables and traces can
                // be regenerated independently, and a stale id is not worth
                // taking the frame loop down for.
                continue;
            };
            acc.observe(sample);
        }
    }
    accs
}
