//! Trace assembly: the `SimulationCutTrace` builders and the private
//! accumulators they use for hotspots and semantic summaries.

use super::{
    SIMULATION_CUT_TRACE_SCHEMA_VERSION, SimulationCutHotspot, SimulationCutIssue,
    SimulationCutIssueKind, SimulationCutSample, SimulationCutSummary, SimulationCutTrace,
    SimulationSemanticCutSummary, SummaryAccumulator,
};
use crate::ids::ToolpathId;
use crate::ops::drill_metrics::DrillToolpathSummary;
use crate::trace::semantic_trace::ToolpathSemanticTrace;
use crate::trace::toolpath_spans::SpanId;
use std::collections::BTreeMap;

/// Start a new issue segment from a sample known to be `is_cutting` and
/// below the engagement threshold for the given `kind`.
fn new_open_segment(
    sample: &SimulationCutSample,
    kind: SimulationCutIssueKind,
) -> SimulationCutIssue {
    let label = match kind {
        SimulationCutIssueKind::AirCut => "Air cut".to_owned(),
        SimulationCutIssueKind::LowEngagement => "Low engagement".to_owned(),
    };
    SimulationCutIssue {
        kind,
        toolpath_id: sample.toolpath_id,
        move_index: sample.move_index,
        sample_index: sample.sample_index,
        cumulative_time_s: sample.cumulative_time_s,
        position: sample.position,
        radial_engagement: sample.engagement.radial_woc_fraction,
        semantic_item_id: sample.semantic_item_id,
        label,
        end_move_index: sample.move_index,
        end_sample_index: sample.sample_index,
        end_cumulative_time_s: sample.cumulative_time_s,
        end_position: sample.position,
        sample_count: 1,
        duration_s: 0.0,
        min_radial_engagement: sample.engagement.radial_woc_fraction,
        span_path: sample.span_path.clone(),
    }
}

impl SimulationCutTrace {
    /// Neutral test fixture: an empty trace at the current
    /// [`SIMULATION_CUT_TRACE_SCHEMA_VERSION`] with no samples, summaries,
    /// issues, or drill data. Test code overrides the fields under test via
    /// struct-update syntax. Not for production paths.
    pub fn test_fixture() -> Self {
        Self {
            schema_version: SIMULATION_CUT_TRACE_SCHEMA_VERSION,
            sample_step_mm: 0.0,
            summary: SimulationCutSummary::default(),
            toolpath_summaries: Vec::new(),
            semantic_summaries: Vec::new(),
            hotspots: Vec::new(),
            issues: Vec::new(),
            samples: Vec::new(),
            provenance: None,
            drill_samples: Vec::new(),
            drill_summaries: Vec::new(),
            toolpath_runtimes: Vec::new(),
            predicted_feeds: crate::machine::kinematics::PredictedFeedMap::new(),
            modulated_feeds: std::collections::BTreeMap::new(),
            modulation_summaries: std::collections::BTreeMap::new(),
        }
    }

    pub fn from_samples(sample_step_mm: f64, samples: Vec<SimulationCutSample>) -> Self {
        Self::from_samples_with_semantics(
            sample_step_mm,
            samples,
            std::iter::empty::<(ToolpathId, &'static ToolpathSemanticTrace)>(),
        )
    }

    pub(crate) fn from_samples_with_semantics<'a, I>(
        sample_step_mm: f64,
        samples: Vec<SimulationCutSample>,
        semantic_traces: I,
    ) -> Self
    where
        I: IntoIterator<Item = (ToolpathId, &'a ToolpathSemanticTrace)>,
    {
        Self::from_samples_with_context(
            sample_step_mm,
            samples,
            semantic_traces,
            &std::collections::BTreeSet::new(),
        )
    }

    /// Trace builder with op-kind context.
    ///
    /// `metrics_not_applicable_toolpath_ids` is the set of toolpath ids whose
    /// kinematics don't fit the dexel's XY-cylinder side-engagement model
    /// (drill / alignment-pin-drill — Z-only moves). For samples in those
    /// toolpaths the builder suppresses air-cut / low-engagement
    /// `SimulationCutIssue` emission (otherwise every sample emits one and
    /// inflates `issue_count` to the thousands) and sets the per-TP
    /// `metrics_not_applicable` flag on the summary.
    /// P4 — see `planning/P4_DRILL_METRIC_SUPPRESSION_RCA.md`.
    pub fn from_samples_with_context<'a, I>(
        sample_step_mm: f64,
        samples: Vec<SimulationCutSample>,
        semantic_traces: I,
        metrics_not_applicable_toolpath_ids: &std::collections::BTreeSet<ToolpathId>,
    ) -> Self
    where
        I: IntoIterator<Item = (ToolpathId, &'a ToolpathSemanticTrace)>,
    {
        let semantic_traces: BTreeMap<ToolpathId, &ToolpathSemanticTrace> =
            semantic_traces.into_iter().collect();
        let mut toolpaths: BTreeMap<ToolpathId, SummaryAccumulator> = BTreeMap::new();
        let mut hotspot_accs: BTreeMap<(ToolpathId, Option<u64>), HotspotAccumulator> =
            BTreeMap::new();
        let mut semantic_accs: BTreeMap<(ToolpathId, u64), SemanticSummaryAccumulator> =
            BTreeMap::new();
        let mut overall = SummaryAccumulator::default();
        let mut issues = Vec::new();
        // Coalesce contiguous air-cut / low-engagement samples into a single
        // issue per segment instead of one issue per sample. Before this
        // coalescing, a 30K-move adaptive3d run emitted ~141K "air_cut"
        // issues — most of a run's sample stream — drowning out any
        // actionable signal. Tracked per-toolpath so interleaving of
        // samples across toolpaths doesn't bleed one toolpath's segment
        // into another's.
        //
        // See planning/adaptive_review_2026-04.md F-15.
        let mut open_segments: BTreeMap<ToolpathId, SimulationCutIssue> = BTreeMap::new();

        for sample in &samples {
            overall.observe(sample);
            toolpaths
                .entry(sample.toolpath_id)
                .or_default()
                .observe(sample);
            hotspot_accs
                .entry((sample.toolpath_id, sample.semantic_item_id))
                .or_insert_with(|| HotspotAccumulator::new(sample))
                .observe(sample);
            if let Some(item_id) = sample.semantic_item_id {
                semantic_accs
                    .entry((sample.toolpath_id, item_id))
                    .or_insert_with(|| SemanticSummaryAccumulator::new(sample))
                    .observe(sample);
            }

            // P4: for ops whose kinematics fall outside the dexel's
            // XY-cylinder engagement model (drill / pin-drill), every
            // cutting sample reads ~0 radial engagement. Skip issue
            // emission for these to avoid drowning the issue list in
            // thousands of false "air_cut" entries.
            let kind = if !sample.is_cutting
                || metrics_not_applicable_toolpath_ids.contains(&sample.toolpath_id)
            {
                None
            } else if sample.engagement.radial_woc_fraction < 0.02 {
                Some(SimulationCutIssueKind::AirCut)
            } else if sample.engagement.radial_woc_fraction < 0.10 {
                Some(SimulationCutIssueKind::LowEngagement)
            } else {
                None
            };

            match (open_segments.get_mut(&sample.toolpath_id), kind) {
                (Some(open), Some(k)) if open.kind == k => {
                    // Same kind — extend the current segment.
                    open.end_move_index = sample.move_index;
                    open.end_sample_index = sample.sample_index;
                    open.end_cumulative_time_s = sample.cumulative_time_s;
                    open.end_position = sample.position;
                    open.duration_s = sample.cumulative_time_s - open.cumulative_time_s;
                    if sample.engagement.radial_woc_fraction < open.min_radial_engagement {
                        open.min_radial_engagement = sample.engagement.radial_woc_fraction;
                    }
                    open.sample_count += 1;
                }
                (Some(_), Some(k)) => {
                    // Kind changed — flush the old segment and open a new one.
                    if let Some(old) = open_segments.remove(&sample.toolpath_id) {
                        issues.push(old);
                    }
                    open_segments.insert(sample.toolpath_id, new_open_segment(sample, k));
                }
                (Some(_), None) => {
                    // Issue ended (either rapid or good engagement).
                    if let Some(old) = open_segments.remove(&sample.toolpath_id) {
                        issues.push(old);
                    }
                }
                (None, Some(k)) => {
                    open_segments.insert(sample.toolpath_id, new_open_segment(sample, k));
                }
                (None, None) => {}
            }
        }

        // Flush any segments still open at the end of the sample stream.
        for (_, open) in open_segments {
            issues.push(open);
        }

        let toolpath_summaries: Vec<_> = toolpaths
            .into_iter()
            .map(|(toolpath_id, acc)| {
                let mut s = acc.finish_toolpath(toolpath_id);
                if metrics_not_applicable_toolpath_ids.contains(&toolpath_id) {
                    s.metrics_not_applicable = true;
                }
                s
            })
            .collect();
        let mut semantic_summaries: Vec<_> = semantic_accs
            .into_iter()
            .filter_map(|((toolpath_id, semantic_item_id), acc)| {
                let trace = semantic_traces.get(&toolpath_id)?;
                let item = trace
                    .items
                    .iter()
                    .find(|item| item.id == semantic_item_id)?;
                Some(acc.finish(toolpath_id, item))
            })
            .collect();
        semantic_summaries.sort_by(|left, right| {
            right
                .wasted_runtime_s
                .total_cmp(&left.wasted_runtime_s)
                .then_with(|| left.average_mrr_mm3_s.total_cmp(&right.average_mrr_mm3_s))
                .then_with(|| right.total_runtime_s.total_cmp(&left.total_runtime_s))
                .then_with(|| left.move_start.cmp(&right.move_start))
        });
        let mut hotspots: Vec<_> = hotspot_accs
            .into_values()
            .map(HotspotAccumulator::finish)
            .collect();
        hotspots.sort_by(|left, right| {
            right
                .wasted_runtime_s
                .total_cmp(&left.wasted_runtime_s)
                .then_with(|| right.total_runtime_s.total_cmp(&left.total_runtime_s))
                .then_with(|| left.move_start.cmp(&right.move_start))
        });

        let summary = overall.finish_summary(
            samples.len(),
            toolpath_summaries.len(),
            issues.len(),
            hotspots.len(),
        );

        Self {
            schema_version: SIMULATION_CUT_TRACE_SCHEMA_VERSION,
            sample_step_mm,
            summary,
            toolpath_summaries,
            semantic_summaries,
            hotspots,
            issues,
            samples,
            provenance: None,
            drill_samples: Vec::new(),
            drill_summaries: Vec::new(),
            toolpath_runtimes: Vec::new(),
            predicted_feeds: crate::machine::kinematics::PredictedFeedMap::new(),
            modulated_feeds: std::collections::BTreeMap::new(),
            modulation_summaries: std::collections::BTreeMap::new(),
        }
    }

    /// Look up the per-toolpath drill summary by id. `None` when this
    /// toolpath isn't a drill op (no entry in [`Self::drill_summaries`]).
    /// Pairs with the per-toolpath
    /// [`SimulationToolpathCutSummary::metrics_not_applicable`] flag —
    /// when that flag is set, this lookup is the right place to read
    /// drill-native metrics in lieu of engagement-axis ones.
    pub fn drill_summary_for(&self, toolpath_id: ToolpathId) -> Option<&DrillToolpathSummary> {
        self.drill_summaries
            .iter()
            .find(|s| s.toolpath_id == toolpath_id)
    }
}

struct HotspotAccumulator {
    toolpath_id: ToolpathId,
    semantic_item_id: Option<u64>,
    move_start: usize,
    move_end: usize,
    sample_index_start: usize,
    sample_index_end: usize,
    representative_position: [f64; 3],
    representative_segment_time_s: f64,
    span_path: Vec<SpanId>,
    summary: SummaryAccumulator,
}

impl HotspotAccumulator {
    fn new(sample: &SimulationCutSample) -> Self {
        Self {
            toolpath_id: sample.toolpath_id,
            semantic_item_id: sample.semantic_item_id,
            move_start: sample.move_index,
            move_end: sample.move_index,
            sample_index_start: sample.sample_index,
            sample_index_end: sample.sample_index,
            representative_position: sample.position,
            representative_segment_time_s: sample.segment_time_s,
            span_path: sample.span_path.clone(),
            summary: SummaryAccumulator::default(),
        }
    }

    fn observe(&mut self, sample: &SimulationCutSample) {
        self.move_start = self.move_start.min(sample.move_index);
        self.move_end = self.move_end.max(sample.move_index);
        self.sample_index_start = self.sample_index_start.min(sample.sample_index);
        self.sample_index_end = self.sample_index_end.max(sample.sample_index);
        if sample.segment_time_s >= self.representative_segment_time_s {
            self.representative_position = sample.position;
            self.representative_segment_time_s = sample.segment_time_s;
        }
        self.summary.observe(sample);
    }

    fn finish(self) -> SimulationCutHotspot {
        let average_engagement = self.summary.average_engagement();
        let average_mrr_mm3_s = self.summary.average_mrr();
        SimulationCutHotspot {
            toolpath_id: self.toolpath_id,
            semantic_item_id: self.semantic_item_id,
            move_start: self.move_start,
            move_end: self.move_end,
            sample_index_start: self.sample_index_start,
            sample_index_end: self.sample_index_end,
            representative_position: self.representative_position,
            total_runtime_s: self.summary.total_runtime_s,
            cutting_runtime_s: self.summary.cutting_runtime_s,
            rapid_runtime_s: self.summary.rapid_runtime_s,
            air_cut_time_s: self.summary.air_cut_time_s,
            low_engagement_time_s: self.summary.low_engagement_time_s,
            wasted_runtime_s: self.summary.air_cut_time_s + self.summary.low_engagement_time_s,
            average_engagement,
            peak_chipload_mm_per_tooth: self.summary.peak_chipload_mm_per_tooth,
            peak_axial_doc_mm: self.summary.peak_axial_doc_mm,
            peak_plunge_descent_mm: self.summary.peak_plunge_descent_mm,
            total_removed_volume_est_mm3: self.summary.total_removed_volume_est_mm3,
            average_mrr_mm3_s,
            span_path: self.span_path,
        }
    }
}

struct SemanticSummaryAccumulator {
    move_start: usize,
    move_end: usize,
    representative_sample_index: usize,
    representative_segment_time_s: f64,
    summary: SummaryAccumulator,
}

impl SemanticSummaryAccumulator {
    fn new(sample: &SimulationCutSample) -> Self {
        Self {
            move_start: sample.move_index,
            move_end: sample.move_index,
            representative_sample_index: sample.sample_index,
            representative_segment_time_s: sample.segment_time_s,
            summary: SummaryAccumulator::default(),
        }
    }

    fn observe(&mut self, sample: &SimulationCutSample) {
        self.move_start = self.move_start.min(sample.move_index);
        self.move_end = self.move_end.max(sample.move_index);
        if sample.segment_time_s >= self.representative_segment_time_s {
            self.representative_sample_index = sample.sample_index;
            self.representative_segment_time_s = sample.segment_time_s;
        }
        self.summary.observe(sample);
    }

    fn finish(
        self,
        toolpath_id: ToolpathId,
        item: &crate::trace::semantic_trace::ToolpathSemanticItem,
    ) -> SimulationSemanticCutSummary {
        SimulationSemanticCutSummary {
            toolpath_id,
            semantic_item_id: item.id,
            label: item.label.clone(),
            kind: item.kind.clone(),
            move_start: item.move_start.unwrap_or(self.move_start),
            move_end: item.move_end.unwrap_or(self.move_end),
            sample_count: self.summary.sample_count,
            representative_sample_index: self.representative_sample_index,
            total_runtime_s: self.summary.total_runtime_s,
            cutting_runtime_s: self.summary.cutting_runtime_s,
            rapid_runtime_s: self.summary.rapid_runtime_s,
            air_cut_time_s: self.summary.air_cut_time_s,
            low_engagement_time_s: self.summary.low_engagement_time_s,
            wasted_runtime_s: self.summary.air_cut_time_s + self.summary.low_engagement_time_s,
            average_engagement: self.summary.average_engagement(),
            peak_engagement: self.summary.peak_engagement,
            peak_chipload_mm_per_tooth: self.summary.peak_chipload_mm_per_tooth,
            peak_axial_doc_mm: self.summary.peak_axial_doc_mm,
            peak_plunge_descent_mm: self.summary.peak_plunge_descent_mm,
            total_removed_volume_est_mm3: self.summary.total_removed_volume_est_mm3,
            average_mrr_mm3_s: self.summary.average_mrr(),
            peak_mrr_mm3_s: self.summary.peak_mrr_mm3_s,
            air_cut_issue_count: self.summary.air_cut_issue_count,
            low_engagement_issue_count: self.summary.low_engagement_issue_count,
        }
    }
}
