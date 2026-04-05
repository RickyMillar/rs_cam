use crate::debug_trace::TOOLPATH_DEBUG_SCHEMA_VERSION;
use crate::semantic_trace::{ToolpathSemanticKind, ToolpathSemanticTrace};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SimulationMetricOptions {
    pub enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SimulationCutIssueKind {
    AirCut,
    LowEngagement,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimulationCutSample {
    pub toolpath_id: usize,
    pub move_index: usize,
    pub sample_index: usize,
    pub position: [f64; 3],
    pub cumulative_time_s: f64,
    pub segment_time_s: f64,
    pub is_cutting: bool,
    pub feed_rate_mm_min: f64,
    pub spindle_rpm: u32,
    pub flute_count: u32,
    pub axial_doc_mm: f64,
    pub radial_engagement: f64,
    pub chipload_mm_per_tooth: f64,
    pub removed_volume_est_mm3: f64,
    pub mrr_mm3_s: f64,
    pub semantic_item_id: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimulationCutIssue {
    pub kind: SimulationCutIssueKind,
    pub toolpath_id: usize,
    pub move_index: usize,
    pub sample_index: usize,
    pub cumulative_time_s: f64,
    pub position: [f64; 3],
    pub radial_engagement: f64,
    pub semantic_item_id: Option<u64>,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimulationToolpathCutSummary {
    pub toolpath_id: usize,
    pub sample_count: usize,
    pub total_runtime_s: f64,
    pub cutting_runtime_s: f64,
    pub rapid_runtime_s: f64,
    pub air_cut_time_s: f64,
    pub low_engagement_time_s: f64,
    pub average_engagement: f64,
    pub peak_chipload_mm_per_tooth: f64,
    pub peak_axial_doc_mm: f64,
    pub total_removed_volume_est_mm3: f64,
    pub average_mrr_mm3_s: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimulationSemanticCutSummary {
    pub toolpath_id: usize,
    pub semantic_item_id: u64,
    pub label: String,
    pub kind: ToolpathSemanticKind,
    pub move_start: usize,
    pub move_end: usize,
    pub sample_count: usize,
    pub representative_sample_index: usize,
    pub total_runtime_s: f64,
    pub cutting_runtime_s: f64,
    pub rapid_runtime_s: f64,
    pub air_cut_time_s: f64,
    pub low_engagement_time_s: f64,
    pub wasted_runtime_s: f64,
    pub average_engagement: f64,
    pub peak_engagement: f64,
    pub peak_chipload_mm_per_tooth: f64,
    pub peak_axial_doc_mm: f64,
    pub total_removed_volume_est_mm3: f64,
    pub average_mrr_mm3_s: f64,
    pub peak_mrr_mm3_s: f64,
    pub air_cut_issue_count: usize,
    pub low_engagement_issue_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimulationCutHotspot {
    pub toolpath_id: usize,
    pub semantic_item_id: Option<u64>,
    pub move_start: usize,
    pub move_end: usize,
    pub sample_index_start: usize,
    pub sample_index_end: usize,
    pub representative_position: [f64; 3],
    pub total_runtime_s: f64,
    pub cutting_runtime_s: f64,
    pub rapid_runtime_s: f64,
    pub air_cut_time_s: f64,
    pub low_engagement_time_s: f64,
    pub wasted_runtime_s: f64,
    pub average_engagement: f64,
    pub peak_chipload_mm_per_tooth: f64,
    pub peak_axial_doc_mm: f64,
    pub total_removed_volume_est_mm3: f64,
    pub average_mrr_mm3_s: f64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SimulationCutSummary {
    pub sample_count: usize,
    pub toolpath_count: usize,
    pub issue_count: usize,
    pub hotspot_count: usize,
    pub total_runtime_s: f64,
    pub cutting_runtime_s: f64,
    pub rapid_runtime_s: f64,
    pub air_cut_time_s: f64,
    pub low_engagement_time_s: f64,
    pub average_engagement: f64,
    pub peak_chipload_mm_per_tooth: f64,
    pub peak_axial_doc_mm: f64,
    pub total_removed_volume_est_mm3: f64,
    pub average_mrr_mm3_s: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimulationCutTrace {
    pub schema_version: u32,
    pub sample_step_mm: f64,
    pub summary: SimulationCutSummary,
    pub toolpath_summaries: Vec<SimulationToolpathCutSummary>,
    pub semantic_summaries: Vec<SimulationSemanticCutSummary>,
    pub hotspots: Vec<SimulationCutHotspot>,
    pub issues: Vec<SimulationCutIssue>,
    pub samples: Vec<SimulationCutSample>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimulationCutArtifact {
    pub schema_version: u32,
    pub resolution_mm: f64,
    pub sample_step_mm: f64,
    pub stock_bbox_min: [f64; 3],
    pub stock_bbox_max: [f64; 3],
    pub included_toolpath_ids: Vec<usize>,
    pub request_snapshot: Value,
    pub trace: SimulationCutTrace,
}

impl SimulationCutArtifact {
    pub fn new(
        resolution_mm: f64,
        sample_step_mm: f64,
        stock_bbox_min: [f64; 3],
        stock_bbox_max: [f64; 3],
        included_toolpath_ids: Vec<usize>,
        request_snapshot: Value,
        trace: SimulationCutTrace,
    ) -> Self {
        Self {
            schema_version: TOOLPATH_DEBUG_SCHEMA_VERSION,
            resolution_mm,
            sample_step_mm,
            stock_bbox_min,
            stock_bbox_max,
            included_toolpath_ids,
            request_snapshot,
            trace,
        }
    }
}

impl SimulationCutTrace {
    pub fn from_samples(sample_step_mm: f64, samples: Vec<SimulationCutSample>) -> Self {
        Self::from_samples_with_semantics(
            sample_step_mm,
            samples,
            std::iter::empty::<(usize, &'static ToolpathSemanticTrace)>(),
        )
    }

    pub fn from_samples_with_semantics<'a, I>(
        sample_step_mm: f64,
        samples: Vec<SimulationCutSample>,
        semantic_traces: I,
    ) -> Self
    where
        I: IntoIterator<Item = (usize, &'a ToolpathSemanticTrace)>,
    {
        let semantic_traces: BTreeMap<usize, &ToolpathSemanticTrace> =
            semantic_traces.into_iter().collect();
        let mut toolpaths: BTreeMap<usize, SummaryAccumulator> = BTreeMap::new();
        let mut hotspot_accs: BTreeMap<(usize, Option<u64>), HotspotAccumulator> = BTreeMap::new();
        let mut semantic_accs: BTreeMap<(usize, u64), SemanticSummaryAccumulator> = BTreeMap::new();
        let mut overall = SummaryAccumulator::default();
        let mut issues = Vec::new();

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

            if sample.is_cutting && sample.radial_engagement < 0.02 {
                issues.push(SimulationCutIssue {
                    kind: SimulationCutIssueKind::AirCut,
                    toolpath_id: sample.toolpath_id,
                    move_index: sample.move_index,
                    sample_index: sample.sample_index,
                    cumulative_time_s: sample.cumulative_time_s,
                    position: sample.position,
                    radial_engagement: sample.radial_engagement,
                    semantic_item_id: sample.semantic_item_id,
                    label: "Air cut".to_owned(),
                });
            } else if sample.is_cutting && sample.radial_engagement < 0.10 {
                issues.push(SimulationCutIssue {
                    kind: SimulationCutIssueKind::LowEngagement,
                    toolpath_id: sample.toolpath_id,
                    move_index: sample.move_index,
                    sample_index: sample.sample_index,
                    cumulative_time_s: sample.cumulative_time_s,
                    position: sample.position,
                    radial_engagement: sample.radial_engagement,
                    semantic_item_id: sample.semantic_item_id,
                    label: "Low engagement".to_owned(),
                });
            }
        }

        let toolpath_summaries: Vec<_> = toolpaths
            .into_iter()
            .map(|(toolpath_id, acc)| acc.finish_toolpath(toolpath_id))
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
            schema_version: TOOLPATH_DEBUG_SCHEMA_VERSION,
            sample_step_mm,
            summary,
            toolpath_summaries,
            semantic_summaries,
            hotspots,
            issues,
            samples,
        }
    }
}

#[derive(Default)]
struct SummaryAccumulator {
    total_runtime_s: f64,
    cutting_runtime_s: f64,
    rapid_runtime_s: f64,
    air_cut_time_s: f64,
    low_engagement_time_s: f64,
    engagement_time_weighted_sum: f64,
    peak_engagement: f64,
    peak_chipload_mm_per_tooth: f64,
    peak_axial_doc_mm: f64,
    total_removed_volume_est_mm3: f64,
    peak_mrr_mm3_s: f64,
    air_cut_issue_count: usize,
    low_engagement_issue_count: usize,
    sample_count: usize,
}

impl SummaryAccumulator {
    fn observe(&mut self, sample: &SimulationCutSample) {
        self.sample_count += 1;
        self.total_runtime_s += sample.segment_time_s;
        self.total_removed_volume_est_mm3 += sample.removed_volume_est_mm3.max(0.0);
        self.peak_engagement = self.peak_engagement.max(sample.radial_engagement.max(0.0));
        self.peak_chipload_mm_per_tooth = self
            .peak_chipload_mm_per_tooth
            .max(sample.chipload_mm_per_tooth.max(0.0));
        self.peak_axial_doc_mm = self.peak_axial_doc_mm.max(sample.axial_doc_mm.max(0.0));
        self.peak_mrr_mm3_s = self.peak_mrr_mm3_s.max(sample.mrr_mm3_s.max(0.0));

        if sample.is_cutting {
            self.cutting_runtime_s += sample.segment_time_s;
            self.engagement_time_weighted_sum += sample.radial_engagement * sample.segment_time_s;
            if sample.radial_engagement < 0.02 {
                self.air_cut_time_s += sample.segment_time_s;
                self.air_cut_issue_count += 1;
            } else if sample.radial_engagement < 0.10 {
                self.low_engagement_time_s += sample.segment_time_s;
                self.low_engagement_issue_count += 1;
            }
        } else {
            self.rapid_runtime_s += sample.segment_time_s;
        }
    }

    fn average_engagement(&self) -> f64 {
        if self.cutting_runtime_s <= 1e-9 {
            0.0
        } else {
            self.engagement_time_weighted_sum / self.cutting_runtime_s
        }
    }

    fn average_mrr(&self) -> f64 {
        if self.cutting_runtime_s <= 1e-9 {
            0.0
        } else {
            self.total_removed_volume_est_mm3 / self.cutting_runtime_s
        }
    }

    fn finish_toolpath(self, toolpath_id: usize) -> SimulationToolpathCutSummary {
        SimulationToolpathCutSummary {
            toolpath_id,
            sample_count: self.sample_count,
            total_runtime_s: self.total_runtime_s,
            cutting_runtime_s: self.cutting_runtime_s,
            rapid_runtime_s: self.rapid_runtime_s,
            air_cut_time_s: self.air_cut_time_s,
            low_engagement_time_s: self.low_engagement_time_s,
            average_engagement: self.average_engagement(),
            peak_chipload_mm_per_tooth: self.peak_chipload_mm_per_tooth,
            peak_axial_doc_mm: self.peak_axial_doc_mm,
            total_removed_volume_est_mm3: self.total_removed_volume_est_mm3,
            average_mrr_mm3_s: self.average_mrr(),
        }
    }

    fn finish_summary(
        self,
        sample_count: usize,
        toolpath_count: usize,
        issue_count: usize,
        hotspot_count: usize,
    ) -> SimulationCutSummary {
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
            average_engagement: self.average_engagement(),
            peak_chipload_mm_per_tooth: self.peak_chipload_mm_per_tooth,
            peak_axial_doc_mm: self.peak_axial_doc_mm,
            total_removed_volume_est_mm3: self.total_removed_volume_est_mm3,
            average_mrr_mm3_s: self.average_mrr(),
        }
    }
}

struct HotspotAccumulator {
    toolpath_id: usize,
    semantic_item_id: Option<u64>,
    move_start: usize,
    move_end: usize,
    sample_index_start: usize,
    sample_index_end: usize,
    representative_position: [f64; 3],
    representative_segment_time_s: f64,
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
            total_removed_volume_est_mm3: self.summary.total_removed_volume_est_mm3,
            average_mrr_mm3_s,
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
        toolpath_id: usize,
        item: &crate::semantic_trace::ToolpathSemanticItem,
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
            total_removed_volume_est_mm3: self.summary.total_removed_volume_est_mm3,
            average_mrr_mm3_s: self.summary.average_mrr(),
            peak_mrr_mm3_s: self.summary.peak_mrr_mm3_s,
            air_cut_issue_count: self.summary.air_cut_issue_count,
            low_engagement_issue_count: self.summary.low_engagement_issue_count,
        }
    }
}

pub fn write_simulation_cut_artifact(
    dir: &Path,
    file_stem: &str,
    artifact: &SimulationCutArtifact,
) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let timestamp_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let file_name = format!(
        "{}_{}.json",
        timestamp_ms,
        sanitize_filename_component(file_stem)
    );
    let path = dir.join(file_name);
    let payload = serde_json::to_vec_pretty(artifact)?;
    std::fs::write(&path, payload)?;
    Ok(path)
}

fn sanitize_filename_component(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            output.push(ch.to_ascii_lowercase());
        } else if matches!(ch, '-' | '_') {
            output.push(ch);
        } else {
            output.push('_');
        }
    }
    let output = output.trim_matches('_');
    if output.is_empty() {
        "simulation_cut_trace".to_owned()
    } else {
        output.to_owned()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::semantic_trace::{ToolpathSemanticKind, ToolpathSemanticRecorder};
    use crate::toolpath::Toolpath;

    #[test]
    fn trace_from_samples_accumulates_summary_and_issues() {
        let trace = SimulationCutTrace::from_samples(
            0.5,
            vec![
                SimulationCutSample {
                    toolpath_id: 1,
                    move_index: 4,
                    sample_index: 0,
                    position: [1.0, 2.0, -1.0],
                    cumulative_time_s: 0.2,
                    segment_time_s: 0.2,
                    is_cutting: true,
                    feed_rate_mm_min: 600.0,
                    spindle_rpm: 18_000,
                    flute_count: 2,
                    axial_doc_mm: 1.5,
                    radial_engagement: 0.01,
                    chipload_mm_per_tooth: 0.0166,
                    removed_volume_est_mm3: 2.0,
                    mrr_mm3_s: 10.0,
                    semantic_item_id: Some(9),
                },
                SimulationCutSample {
                    toolpath_id: 1,
                    move_index: 5,
                    sample_index: 1,
                    position: [2.0, 2.0, -1.0],
                    cumulative_time_s: 0.5,
                    segment_time_s: 0.3,
                    is_cutting: true,
                    feed_rate_mm_min: 600.0,
                    spindle_rpm: 18_000,
                    flute_count: 2,
                    axial_doc_mm: 2.0,
                    radial_engagement: 0.08,
                    chipload_mm_per_tooth: 0.0166,
                    removed_volume_est_mm3: 3.0,
                    mrr_mm3_s: 10.0,
                    semantic_item_id: Some(9),
                },
                SimulationCutSample {
                    toolpath_id: 1,
                    move_index: 6,
                    sample_index: 2,
                    position: [3.0, 2.0, 5.0],
                    cumulative_time_s: 0.6,
                    segment_time_s: 0.1,
                    is_cutting: false,
                    feed_rate_mm_min: 5000.0,
                    spindle_rpm: 18_000,
                    flute_count: 2,
                    axial_doc_mm: 0.0,
                    radial_engagement: 0.0,
                    chipload_mm_per_tooth: 0.0,
                    removed_volume_est_mm3: 0.0,
                    mrr_mm3_s: 0.0,
                    semantic_item_id: None,
                },
            ],
        );

        assert_eq!(trace.summary.sample_count, 3);
        assert_eq!(trace.summary.issue_count, 2);
        assert_eq!(trace.toolpath_summaries.len(), 1);
        assert_eq!(trace.hotspots.len(), 2);
        assert!((trace.summary.total_runtime_s - 0.6).abs() < 1e-9);
        assert!((trace.summary.air_cut_time_s - 0.2).abs() < 1e-9);
        assert!((trace.summary.low_engagement_time_s - 0.3).abs() < 1e-9);
        assert!((trace.summary.total_removed_volume_est_mm3 - 5.0).abs() < 1e-9);
    }

    #[test]
    fn trace_from_samples_with_semantics_emits_per_item_summaries() {
        let recorder = ToolpathSemanticRecorder::new("Metrics", "Metrics");
        let root = recorder.root_context();
        let op = root.start_item(ToolpathSemanticKind::Operation, "Metrics");
        let mut tp = Toolpath::new();
        tp.rapid_to(crate::geo::P3::new(0.0, 0.0, 5.0));
        tp.feed_to(crate::geo::P3::new(0.0, 0.0, -1.0), 300.0);
        tp.feed_to(crate::geo::P3::new(10.0, 0.0, -1.0), 300.0);
        tp.rapid_to(crate::geo::P3::new(10.0, 0.0, 5.0));
        let pass = op
            .context()
            .start_item(ToolpathSemanticKind::Pass, "Pass 1");
        pass.bind_to_toolpath(&tp, 0, tp.moves.len());
        let trace = recorder.finish();

        let cut_trace = SimulationCutTrace::from_samples_with_semantics(
            0.5,
            vec![
                SimulationCutSample {
                    toolpath_id: 1,
                    move_index: 1,
                    sample_index: 0,
                    position: [0.0, 0.0, -1.0],
                    cumulative_time_s: 0.2,
                    segment_time_s: 0.2,
                    is_cutting: true,
                    feed_rate_mm_min: 300.0,
                    spindle_rpm: 18_000,
                    flute_count: 2,
                    axial_doc_mm: 1.0,
                    radial_engagement: 0.08,
                    chipload_mm_per_tooth: 0.0083,
                    removed_volume_est_mm3: 0.2,
                    mrr_mm3_s: 1.0,
                    semantic_item_id: Some(2),
                },
                SimulationCutSample {
                    toolpath_id: 1,
                    move_index: 2,
                    sample_index: 1,
                    position: [5.0, 0.0, -1.0],
                    cumulative_time_s: 0.4,
                    segment_time_s: 0.2,
                    is_cutting: true,
                    feed_rate_mm_min: 300.0,
                    spindle_rpm: 18_000,
                    flute_count: 2,
                    axial_doc_mm: 1.2,
                    radial_engagement: 0.15,
                    chipload_mm_per_tooth: 0.0083,
                    removed_volume_est_mm3: 0.4,
                    mrr_mm3_s: 2.0,
                    semantic_item_id: Some(2),
                },
            ],
            [(1, &trace)],
        );

        let summary = cut_trace
            .semantic_summaries
            .iter()
            .find(|summary| summary.semantic_item_id == 2)
            .expect("semantic summary");
        assert_eq!(summary.label, "Pass 1");
        assert_eq!(summary.kind, ToolpathSemanticKind::Pass);
        assert_eq!(summary.sample_count, 2);
        assert!(summary.peak_engagement >= summary.average_engagement);
        assert!(summary.peak_mrr_mm3_s >= summary.average_mrr_mm3_s);
    }

    #[test]
    fn simulation_cut_artifact_writer_creates_json() {
        let artifact = SimulationCutArtifact::new(
            0.25,
            0.25,
            [0.0, 0.0, 0.0],
            [10.0, 10.0, 10.0],
            vec![1, 2],
            serde_json::json!({"resolution": 0.25}),
            SimulationCutTrace::from_samples(0.25, Vec::new()),
        );
        let dir = std::env::temp_dir().join(format!(
            "rs_cam_sim_cut_artifact_{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock before epoch")
                .as_nanos()
        ));
        let path = write_simulation_cut_artifact(&dir, "Adaptive 3D", &artifact)
            .expect("write sim cut artifact");
        let text = std::fs::read_to_string(&path).expect("read sim cut artifact");
        assert!(text.contains("\"included_toolpath_ids\""));
        std::fs::remove_file(path).ok();
        std::fs::remove_dir(dir).ok();
    }

    // --- Task E-simcut: Empty samples produce valid defaults ---

    #[test]
    fn trace_from_empty_samples() {
        let trace = SimulationCutTrace::from_samples(1.0, Vec::new());

        assert_eq!(trace.summary.sample_count, 0);
        assert_eq!(trace.summary.toolpath_count, 0);
        assert_eq!(trace.summary.issue_count, 0);
        assert_eq!(trace.summary.hotspot_count, 0);
        assert!((trace.summary.total_runtime_s).abs() < 1e-9);
        assert!((trace.summary.cutting_runtime_s).abs() < 1e-9);
        assert!((trace.summary.rapid_runtime_s).abs() < 1e-9);
        assert!((trace.summary.average_engagement).abs() < 1e-9);
        assert!((trace.summary.total_removed_volume_est_mm3).abs() < 1e-9);
        assert!(trace.toolpath_summaries.is_empty());
        assert!(trace.issues.is_empty());
        assert!(trace.hotspots.is_empty());
        assert!(trace.samples.is_empty());
    }

    // --- Rapid-only toolpath (no cutting) ---

    #[test]
    fn trace_rapid_only_has_zero_cutting_time() {
        let samples = vec![
            SimulationCutSample {
                toolpath_id: 0,
                move_index: 0,
                sample_index: 0,
                position: [0.0, 0.0, 10.0],
                cumulative_time_s: 0.1,
                segment_time_s: 0.1,
                is_cutting: false,
                feed_rate_mm_min: 5000.0,
                spindle_rpm: 18_000,
                flute_count: 2,
                axial_doc_mm: 0.0,
                radial_engagement: 0.0,
                chipload_mm_per_tooth: 0.0,
                removed_volume_est_mm3: 0.0,
                mrr_mm3_s: 0.0,
                semantic_item_id: None,
            },
            SimulationCutSample {
                toolpath_id: 0,
                move_index: 1,
                sample_index: 1,
                position: [50.0, 0.0, 10.0],
                cumulative_time_s: 0.3,
                segment_time_s: 0.2,
                is_cutting: false,
                feed_rate_mm_min: 5000.0,
                spindle_rpm: 18_000,
                flute_count: 2,
                axial_doc_mm: 0.0,
                radial_engagement: 0.0,
                chipload_mm_per_tooth: 0.0,
                removed_volume_est_mm3: 0.0,
                mrr_mm3_s: 0.0,
                semantic_item_id: None,
            },
        ];

        let trace = SimulationCutTrace::from_samples(1.0, samples);

        assert_eq!(trace.summary.sample_count, 2);
        assert_eq!(trace.summary.toolpath_count, 1);
        assert!((trace.summary.total_runtime_s - 0.3).abs() < 1e-9);
        assert!((trace.summary.cutting_runtime_s).abs() < 1e-9);
        assert!((trace.summary.rapid_runtime_s - 0.3).abs() < 1e-9);
        assert_eq!(trace.summary.issue_count, 0);
        assert!((trace.summary.average_engagement).abs() < 1e-9);
        assert!((trace.summary.total_removed_volume_est_mm3).abs() < 1e-9);
    }

    // --- Engagement metrics classification ---

    #[test]
    fn trace_classifies_air_cut_and_low_engagement() {
        let samples = vec![
            // Air cut: is_cutting=true, engagement < 0.02
            SimulationCutSample {
                toolpath_id: 0,
                move_index: 0,
                sample_index: 0,
                position: [0.0, 0.0, -1.0],
                cumulative_time_s: 0.1,
                segment_time_s: 0.1,
                is_cutting: true,
                feed_rate_mm_min: 600.0,
                spindle_rpm: 18_000,
                flute_count: 2,
                axial_doc_mm: 1.0,
                radial_engagement: 0.01,
                chipload_mm_per_tooth: 0.01,
                removed_volume_est_mm3: 0.1,
                mrr_mm3_s: 1.0,
                semantic_item_id: None,
            },
            // Low engagement: is_cutting=true, 0.02 <= engagement < 0.10
            SimulationCutSample {
                toolpath_id: 0,
                move_index: 1,
                sample_index: 1,
                position: [1.0, 0.0, -1.0],
                cumulative_time_s: 0.2,
                segment_time_s: 0.1,
                is_cutting: true,
                feed_rate_mm_min: 600.0,
                spindle_rpm: 18_000,
                flute_count: 2,
                axial_doc_mm: 1.0,
                radial_engagement: 0.05,
                chipload_mm_per_tooth: 0.01,
                removed_volume_est_mm3: 0.3,
                mrr_mm3_s: 3.0,
                semantic_item_id: None,
            },
            // Good engagement: is_cutting=true, engagement >= 0.10
            SimulationCutSample {
                toolpath_id: 0,
                move_index: 2,
                sample_index: 2,
                position: [2.0, 0.0, -1.0],
                cumulative_time_s: 0.3,
                segment_time_s: 0.1,
                is_cutting: true,
                feed_rate_mm_min: 600.0,
                spindle_rpm: 18_000,
                flute_count: 2,
                axial_doc_mm: 2.0,
                radial_engagement: 0.50,
                chipload_mm_per_tooth: 0.02,
                removed_volume_est_mm3: 1.0,
                mrr_mm3_s: 10.0,
                semantic_item_id: None,
            },
        ];

        let trace = SimulationCutTrace::from_samples(0.5, samples);

        // Two issues: one air cut and one low engagement
        assert_eq!(trace.summary.issue_count, 2);
        assert_eq!(trace.issues.len(), 2);

        let air_cuts: Vec<_> = trace
            .issues
            .iter()
            .filter(|i| i.kind == SimulationCutIssueKind::AirCut)
            .collect();
        let low_engs: Vec<_> = trace
            .issues
            .iter()
            .filter(|i| i.kind == SimulationCutIssueKind::LowEngagement)
            .collect();
        assert_eq!(air_cuts.len(), 1, "Should have exactly 1 air cut issue");
        assert_eq!(
            low_engs.len(),
            1,
            "Should have exactly 1 low engagement issue"
        );

        // Air cut time = 0.1s, low engagement time = 0.1s
        assert!((trace.summary.air_cut_time_s - 0.1).abs() < 1e-9);
        assert!((trace.summary.low_engagement_time_s - 0.1).abs() < 1e-9);
    }

    // --- Peak metrics tracking ---

    #[test]
    fn trace_tracks_peak_chipload_and_axial_doc() {
        let samples = vec![
            SimulationCutSample {
                toolpath_id: 0,
                move_index: 0,
                sample_index: 0,
                position: [0.0, 0.0, -1.0],
                cumulative_time_s: 0.1,
                segment_time_s: 0.1,
                is_cutting: true,
                feed_rate_mm_min: 600.0,
                spindle_rpm: 18_000,
                flute_count: 2,
                axial_doc_mm: 1.5,
                radial_engagement: 0.30,
                chipload_mm_per_tooth: 0.05,
                removed_volume_est_mm3: 2.0,
                mrr_mm3_s: 20.0,
                semantic_item_id: None,
            },
            SimulationCutSample {
                toolpath_id: 0,
                move_index: 1,
                sample_index: 1,
                position: [1.0, 0.0, -2.0],
                cumulative_time_s: 0.2,
                segment_time_s: 0.1,
                is_cutting: true,
                feed_rate_mm_min: 600.0,
                spindle_rpm: 18_000,
                flute_count: 2,
                axial_doc_mm: 3.0,
                radial_engagement: 0.60,
                chipload_mm_per_tooth: 0.08,
                removed_volume_est_mm3: 5.0,
                mrr_mm3_s: 50.0,
                semantic_item_id: None,
            },
        ];

        let trace = SimulationCutTrace::from_samples(0.5, samples);

        assert!(
            (trace.summary.peak_chipload_mm_per_tooth - 0.08).abs() < 1e-9,
            "Peak chipload should be 0.08, got {}",
            trace.summary.peak_chipload_mm_per_tooth
        );
        assert!(
            (trace.summary.peak_axial_doc_mm - 3.0).abs() < 1e-9,
            "Peak axial DOC should be 3.0, got {}",
            trace.summary.peak_axial_doc_mm
        );
        assert!(
            (trace.summary.total_removed_volume_est_mm3 - 7.0).abs() < 1e-9,
            "Total removed volume should be 7.0, got {}",
            trace.summary.total_removed_volume_est_mm3
        );
    }

    // --- Multiple toolpaths produce separate summaries ---

    #[test]
    fn trace_multiple_toolpaths_produce_separate_summaries() {
        let samples = vec![
            SimulationCutSample {
                toolpath_id: 0,
                move_index: 0,
                sample_index: 0,
                position: [0.0, 0.0, -1.0],
                cumulative_time_s: 0.1,
                segment_time_s: 0.1,
                is_cutting: true,
                feed_rate_mm_min: 600.0,
                spindle_rpm: 18_000,
                flute_count: 2,
                axial_doc_mm: 1.0,
                radial_engagement: 0.40,
                chipload_mm_per_tooth: 0.01,
                removed_volume_est_mm3: 1.0,
                mrr_mm3_s: 10.0,
                semantic_item_id: None,
            },
            SimulationCutSample {
                toolpath_id: 1,
                move_index: 0,
                sample_index: 1,
                position: [10.0, 0.0, -2.0],
                cumulative_time_s: 0.3,
                segment_time_s: 0.2,
                is_cutting: true,
                feed_rate_mm_min: 300.0,
                spindle_rpm: 18_000,
                flute_count: 2,
                axial_doc_mm: 2.0,
                radial_engagement: 0.50,
                chipload_mm_per_tooth: 0.02,
                removed_volume_est_mm3: 3.0,
                mrr_mm3_s: 15.0,
                semantic_item_id: None,
            },
        ];

        let trace = SimulationCutTrace::from_samples(1.0, samples);

        assert_eq!(trace.summary.sample_count, 2);
        assert_eq!(trace.summary.toolpath_count, 2);
        assert_eq!(trace.toolpath_summaries.len(), 2);

        let tp0 = trace
            .toolpath_summaries
            .iter()
            .find(|s| s.toolpath_id == 0)
            .expect("should have toolpath 0");
        let tp1 = trace
            .toolpath_summaries
            .iter()
            .find(|s| s.toolpath_id == 1)
            .expect("should have toolpath 1");

        assert_eq!(tp0.sample_count, 1);
        assert_eq!(tp1.sample_count, 1);
        assert!((tp0.total_runtime_s - 0.1).abs() < 1e-9);
        assert!((tp1.total_runtime_s - 0.2).abs() < 1e-9);
        assert!(
            (tp0.total_removed_volume_est_mm3 - 1.0).abs() < 1e-9,
            "TP0 removed volume should be 1.0, got {}",
            tp0.total_removed_volume_est_mm3
        );
        assert!(
            (tp1.total_removed_volume_est_mm3 - 3.0).abs() < 1e-9,
            "TP1 removed volume should be 3.0, got {}",
            tp1.total_removed_volume_est_mm3
        );
    }

    // --- Average engagement is time-weighted ---

    #[test]
    fn trace_average_engagement_is_time_weighted() {
        let samples = vec![
            // 0.1s at engagement=0.20
            SimulationCutSample {
                toolpath_id: 0,
                move_index: 0,
                sample_index: 0,
                position: [0.0, 0.0, -1.0],
                cumulative_time_s: 0.1,
                segment_time_s: 0.1,
                is_cutting: true,
                feed_rate_mm_min: 600.0,
                spindle_rpm: 18_000,
                flute_count: 2,
                axial_doc_mm: 1.0,
                radial_engagement: 0.20,
                chipload_mm_per_tooth: 0.01,
                removed_volume_est_mm3: 0.5,
                mrr_mm3_s: 5.0,
                semantic_item_id: None,
            },
            // 0.3s at engagement=0.80
            SimulationCutSample {
                toolpath_id: 0,
                move_index: 1,
                sample_index: 1,
                position: [1.0, 0.0, -1.0],
                cumulative_time_s: 0.4,
                segment_time_s: 0.3,
                is_cutting: true,
                feed_rate_mm_min: 600.0,
                spindle_rpm: 18_000,
                flute_count: 2,
                axial_doc_mm: 2.0,
                radial_engagement: 0.80,
                chipload_mm_per_tooth: 0.02,
                removed_volume_est_mm3: 2.0,
                mrr_mm3_s: 6.67,
                semantic_item_id: None,
            },
        ];

        let trace = SimulationCutTrace::from_samples(0.5, samples);

        // Time-weighted average: (0.20*0.1 + 0.80*0.3) / (0.1+0.3)
        // = (0.02 + 0.24) / 0.4 = 0.26 / 0.4 = 0.65
        let expected_avg = (0.20 * 0.1 + 0.80 * 0.3) / (0.1 + 0.3);
        assert!(
            (trace.summary.average_engagement - expected_avg).abs() < 1e-6,
            "Average engagement should be {}, got {}",
            expected_avg,
            trace.summary.average_engagement
        );
    }

    // --- Hotspots are created per (toolpath_id, semantic_item_id) ---

    #[test]
    fn trace_hotspots_grouped_by_toolpath_and_semantic_id() {
        let samples = vec![
            SimulationCutSample {
                toolpath_id: 0,
                move_index: 0,
                sample_index: 0,
                position: [0.0, 0.0, -1.0],
                cumulative_time_s: 0.1,
                segment_time_s: 0.1,
                is_cutting: true,
                feed_rate_mm_min: 600.0,
                spindle_rpm: 18_000,
                flute_count: 2,
                axial_doc_mm: 1.0,
                radial_engagement: 0.50,
                chipload_mm_per_tooth: 0.01,
                removed_volume_est_mm3: 1.0,
                mrr_mm3_s: 10.0,
                semantic_item_id: Some(1),
            },
            SimulationCutSample {
                toolpath_id: 0,
                move_index: 1,
                sample_index: 1,
                position: [1.0, 0.0, -1.0],
                cumulative_time_s: 0.2,
                segment_time_s: 0.1,
                is_cutting: true,
                feed_rate_mm_min: 600.0,
                spindle_rpm: 18_000,
                flute_count: 2,
                axial_doc_mm: 1.0,
                radial_engagement: 0.50,
                chipload_mm_per_tooth: 0.01,
                removed_volume_est_mm3: 1.0,
                mrr_mm3_s: 10.0,
                semantic_item_id: Some(2),
            },
            SimulationCutSample {
                toolpath_id: 0,
                move_index: 2,
                sample_index: 2,
                position: [2.0, 0.0, -1.0],
                cumulative_time_s: 0.3,
                segment_time_s: 0.1,
                is_cutting: true,
                feed_rate_mm_min: 600.0,
                spindle_rpm: 18_000,
                flute_count: 2,
                axial_doc_mm: 1.0,
                radial_engagement: 0.50,
                chipload_mm_per_tooth: 0.01,
                removed_volume_est_mm3: 1.0,
                mrr_mm3_s: 10.0,
                semantic_item_id: Some(1),
            },
        ];

        let trace = SimulationCutTrace::from_samples(0.5, samples);

        // Hotspots are keyed by (toolpath_id, semantic_item_id).
        // We have 2 unique keys: (0, Some(1)) and (0, Some(2))
        assert_eq!(
            trace.hotspots.len(),
            2,
            "Should have 2 hotspots for 2 distinct semantic_item_ids, got {}",
            trace.hotspots.len()
        );

        // The hotspot for semantic_item_id=1 should have 2 samples
        let hotspot_1 = trace
            .hotspots
            .iter()
            .find(|h| h.semantic_item_id == Some(1))
            .expect("should have hotspot for semantic_item_id=1");
        assert_eq!(
            hotspot_1.sample_index_start, 0,
            "Hotspot 1 sample start should be 0"
        );
        assert_eq!(
            hotspot_1.sample_index_end, 2,
            "Hotspot 1 sample end should be 2"
        );
    }

    // --- Sanitize filename component ---

    #[test]
    fn sanitize_filename_handles_special_chars() {
        assert_eq!(sanitize_filename_component("Adaptive 3D"), "adaptive_3d");
        assert_eq!(
            sanitize_filename_component("my/file:name.ext"),
            "my_file_name_ext"
        );
        assert_eq!(sanitize_filename_component("---test---"), "---test---");
        assert_eq!(sanitize_filename_component(""), "simulation_cut_trace");
        assert_eq!(sanitize_filename_component("___"), "simulation_cut_trace");
    }
}
