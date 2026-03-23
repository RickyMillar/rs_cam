use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

pub const TOOLPATH_DEBUG_SCHEMA_VERSION: u32 = 1;

pub trait ToolpathPhaseSink: Send + Sync {
    fn set_phase(&self, phase: Option<String>);
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolpathDebugOptions {
    pub enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ToolpathDebugBounds2 {
    pub min_x: f64,
    pub max_x: f64,
    pub min_y: f64,
    pub max_y: f64,
}

impl ToolpathDebugBounds2 {
    pub fn from_points<'a, I>(points: I) -> Option<Self>
    where
        I: IntoIterator<Item = &'a (f64, f64)>,
    {
        let mut min_x = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        let mut saw_any = false;

        for &(x, y) in points {
            min_x = min_x.min(x);
            max_x = max_x.max(x);
            min_y = min_y.min(y);
            max_y = max_y.max(y);
            saw_any = true;
        }

        saw_any.then_some(Self {
            min_x,
            max_x,
            min_y,
            max_y,
        })
    }

    pub fn center(&self) -> (f64, f64) {
        (
            (self.min_x + self.max_x) * 0.5,
            (self.min_y + self.max_y) * 0.5,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolpathDebugSpan {
    pub id: u64,
    pub parent_id: Option<u64>,
    pub kind: String,
    pub label: String,
    pub start_us: u64,
    pub elapsed_us: u64,
    pub xy_bbox: Option<ToolpathDebugBounds2>,
    pub z_level: Option<f64>,
    pub move_start: Option<usize>,
    pub move_end: Option<usize>,
    pub exit_reason: Option<String>,
    pub counters: BTreeMap<String, f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolpathHotspot {
    pub kind: String,
    pub center_x: f64,
    pub center_y: f64,
    pub z_bucket_center: Option<f64>,
    pub bucket_size_xy: f64,
    pub bucket_size_z: Option<f64>,
    pub total_elapsed_us: u64,
    pub span_count: u32,
    pub pass_count: u32,
    pub step_count: u64,
    pub low_yield_exit_count: u32,
    pub representative_span_id: Option<u64>,
    pub move_start: Option<usize>,
    pub move_end: Option<usize>,
    pub semantic_item_id: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolpathDebugAnnotation {
    pub move_index: usize,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolpathDebugSummary {
    pub total_elapsed_us: u64,
    pub span_count: usize,
    pub hotspot_count: usize,
    pub dominant_span_kind: Option<String>,
    pub dominant_span_label: Option<String>,
    pub dominant_span_elapsed_us: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolpathDebugTrace {
    pub schema_version: u32,
    pub toolpath_name: String,
    pub operation_label: String,
    pub summary: ToolpathDebugSummary,
    pub spans: Vec<ToolpathDebugSpan>,
    pub hotspots: Vec<ToolpathHotspot>,
    pub annotations: Vec<ToolpathDebugAnnotation>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolpathDebugArtifact {
    pub schema_version: u32,
    pub toolpath_id: usize,
    pub toolpath_name: String,
    pub operation_label: String,
    pub tool_summary: String,
    pub request_snapshot: Value,
    pub trace: ToolpathDebugTrace,
}

impl ToolpathDebugArtifact {
    pub fn new(
        toolpath_id: usize,
        toolpath_name: impl Into<String>,
        operation_label: impl Into<String>,
        tool_summary: impl Into<String>,
        request_snapshot: Value,
        trace: ToolpathDebugTrace,
    ) -> Self {
        Self {
            schema_version: TOOLPATH_DEBUG_SCHEMA_VERSION,
            toolpath_id,
            toolpath_name: toolpath_name.into(),
            operation_label: operation_label.into(),
            tool_summary: tool_summary.into(),
            request_snapshot,
            trace,
        }
    }
}

#[derive(Clone)]
pub struct ToolpathDebugRecorder {
    inner: Arc<Mutex<RecorderState>>,
    phase_sink: Option<Arc<dyn ToolpathPhaseSink>>,
}

#[derive(Clone)]
pub struct ToolpathDebugContext {
    recorder: ToolpathDebugRecorder,
    parent_id: Option<u64>,
}

pub struct ToolpathDebugScope {
    recorder: ToolpathDebugRecorder,
    span_id: u64,
    finished: bool,
}

struct RecorderState {
    started_at: Instant,
    next_span_id: u64,
    toolpath_name: String,
    operation_label: String,
    spans: BTreeMap<u64, ToolpathDebugSpan>,
    annotations: Vec<ToolpathDebugAnnotation>,
    hotspots: HashMap<HotspotKey, HotspotAccumulator>,
}

/// Bundled parameters for recording a hotspot measurement.
pub struct HotspotRecord {
    pub kind: String,
    pub center_x: f64,
    pub center_y: f64,
    pub z_level: Option<f64>,
    pub bucket_size_xy: f64,
    pub bucket_size_z: Option<f64>,
    pub elapsed_us: u64,
    pub pass_count: u32,
    pub step_count: u64,
    pub low_yield_exit_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct HotspotKey {
    kind: String,
    x_bucket: i64,
    y_bucket: i64,
    z_bucket: Option<i64>,
    bucket_size_xy_bits: u64,
    bucket_size_z_bits: Option<u64>,
}

#[derive(Debug, Clone)]
struct HotspotAccumulator {
    kind: String,
    x_bucket: i64,
    y_bucket: i64,
    z_bucket: Option<i64>,
    bucket_size_xy: f64,
    bucket_size_z: Option<f64>,
    total_elapsed_us: u64,
    span_count: u32,
    pass_count: u32,
    step_count: u64,
    low_yield_exit_count: u32,
}

impl ToolpathDebugRecorder {
    pub fn new(toolpath_name: impl Into<String>, operation_label: impl Into<String>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(RecorderState {
                started_at: Instant::now(),
                next_span_id: 1,
                toolpath_name: toolpath_name.into(),
                operation_label: operation_label.into(),
                spans: BTreeMap::new(),
                annotations: Vec::new(),
                hotspots: HashMap::new(),
            })),
            phase_sink: None,
        }
    }

    pub fn with_phase_sink(mut self, phase_sink: Arc<dyn ToolpathPhaseSink>) -> Self {
        self.phase_sink = Some(phase_sink);
        self
    }

    pub fn root_context(&self) -> ToolpathDebugContext {
        ToolpathDebugContext {
            recorder: self.clone(),
            parent_id: None,
        }
    }

    pub fn finish(self) -> ToolpathDebugTrace {
        self.publish_phase(None);
        let state = self.inner.lock().expect("debug recorder poisoned");
        let mut spans: Vec<_> = state.spans.values().cloned().collect();
        spans.sort_by_key(|span| (span.start_us, span.id));

        let mut hotspots: Vec<_> = state
            .hotspots
            .values()
            .map(|agg| ToolpathHotspot {
                kind: agg.kind.clone(),
                center_x: (agg.x_bucket as f64 + 0.5) * agg.bucket_size_xy,
                center_y: (agg.y_bucket as f64 + 0.5) * agg.bucket_size_xy,
                z_bucket_center: agg
                    .z_bucket
                    .zip(agg.bucket_size_z)
                    .map(|(bucket, size)| (bucket as f64 + 0.5) * size),
                bucket_size_xy: agg.bucket_size_xy,
                bucket_size_z: agg.bucket_size_z,
                total_elapsed_us: agg.total_elapsed_us,
                span_count: agg.span_count,
                pass_count: agg.pass_count,
                step_count: agg.step_count,
                low_yield_exit_count: agg.low_yield_exit_count,
                representative_span_id: None,
                move_start: None,
                move_end: None,
                semantic_item_id: None,
            })
            .collect();
        hotspots.sort_by(|a, b| {
            b.total_elapsed_us
                .cmp(&a.total_elapsed_us)
                .then_with(|| b.step_count.cmp(&a.step_count))
        });

        let dominant = spans.iter().max_by_key(|span| span.elapsed_us);
        let summary = ToolpathDebugSummary {
            total_elapsed_us: state.started_at.elapsed().as_micros() as u64,
            span_count: spans.len(),
            hotspot_count: hotspots.len(),
            dominant_span_kind: dominant.map(|span| span.kind.clone()),
            dominant_span_label: dominant.map(|span| span.label.clone()),
            dominant_span_elapsed_us: dominant.map(|span| span.elapsed_us),
        };

        ToolpathDebugTrace {
            schema_version: TOOLPATH_DEBUG_SCHEMA_VERSION,
            toolpath_name: state.toolpath_name.clone(),
            operation_label: state.operation_label.clone(),
            summary,
            spans,
            hotspots,
            annotations: state.annotations.clone(),
        }
    }

    fn start_span_with_parent(
        &self,
        parent_id: Option<u64>,
        kind: impl Into<String>,
        label: impl Into<String>,
    ) -> ToolpathDebugScope {
        let kind = kind.into();
        let label = label.into();
        let mut state = self.inner.lock().expect("debug recorder poisoned");
        let id = state.next_span_id;
        state.next_span_id += 1;
        let start_us = state.started_at.elapsed().as_micros() as u64;
        state.spans.insert(
            id,
            ToolpathDebugSpan {
                id,
                parent_id,
                kind: kind.clone(),
                label: label.clone(),
                start_us,
                elapsed_us: 0,
                xy_bbox: None,
                z_level: None,
                move_start: None,
                move_end: None,
                exit_reason: None,
                counters: BTreeMap::new(),
            },
        );
        drop(state);
        self.publish_phase(Some(if label.is_empty() { kind } else { label }));

        ToolpathDebugScope {
            recorder: self.clone(),
            span_id: id,
            finished: false,
        }
    }

    pub fn add_annotation(&self, move_index: usize, label: impl Into<String>) {
        let mut state = self.inner.lock().expect("debug recorder poisoned");
        state.annotations.push(ToolpathDebugAnnotation {
            move_index,
            label: label.into(),
        });
        state
            .annotations
            .sort_by_key(|annotation| (annotation.move_index, annotation.label.clone()));
    }

    pub fn record_hotspot(&self, record: &HotspotRecord) {
        if record.bucket_size_xy <= 0.0 {
            return;
        }

        let kind = record.kind.clone();
        let x_bucket = (record.center_x / record.bucket_size_xy).floor() as i64;
        let y_bucket = (record.center_y / record.bucket_size_xy).floor() as i64;
        let z_bucket = record
            .bucket_size_z
            .zip(record.z_level)
            .map(|(size, z)| (z / size).floor() as i64);

        let key = HotspotKey {
            kind: kind.clone(),
            x_bucket,
            y_bucket,
            z_bucket,
            bucket_size_xy_bits: record.bucket_size_xy.to_bits(),
            bucket_size_z_bits: record.bucket_size_z.map(f64::to_bits),
        };

        let mut state = self.inner.lock().expect("debug recorder poisoned");
        let entry = state
            .hotspots
            .entry(key)
            .or_insert_with(|| HotspotAccumulator {
                kind,
                x_bucket,
                y_bucket,
                z_bucket,
                bucket_size_xy: record.bucket_size_xy,
                bucket_size_z: record.bucket_size_z,
                total_elapsed_us: 0,
                span_count: 0,
                pass_count: 0,
                step_count: 0,
                low_yield_exit_count: 0,
            });
        entry.total_elapsed_us += record.elapsed_us;
        entry.span_count += 1;
        entry.pass_count += record.pass_count;
        entry.step_count += record.step_count;
        entry.low_yield_exit_count += record.low_yield_exit_count;
    }

    fn publish_phase(&self, phase: Option<String>) {
        if let Some(phase_sink) = self.phase_sink.as_ref() {
            phase_sink.set_phase(phase);
        }
    }
}

impl ToolpathDebugContext {
    pub fn start_span(
        &self,
        kind: impl Into<String>,
        label: impl Into<String>,
    ) -> ToolpathDebugScope {
        self.recorder
            .start_span_with_parent(self.parent_id, kind, label)
    }

    pub fn record_hotspot(&self, record: &HotspotRecord) {
        self.recorder.record_hotspot(record);
    }

    pub fn add_annotation(&self, move_index: usize, label: impl Into<String>) {
        self.recorder.add_annotation(move_index, label);
    }
}

impl ToolpathDebugScope {
    pub fn id(&self) -> u64 {
        self.span_id
    }

    pub fn context(&self) -> ToolpathDebugContext {
        ToolpathDebugContext {
            recorder: self.recorder.clone(),
            parent_id: Some(self.span_id),
        }
    }

    pub fn set_xy_bbox(&self, bbox: ToolpathDebugBounds2) {
        self.update_span(|span| span.xy_bbox = Some(bbox));
    }

    pub fn set_z_level(&self, z_level: f64) {
        self.update_span(|span| span.z_level = Some(z_level));
    }

    pub fn set_move_range(&self, move_start: usize, move_end: usize) {
        self.update_span(|span| {
            span.move_start = Some(move_start);
            span.move_end = Some(move_end);
        });
    }

    pub fn set_exit_reason(&self, exit_reason: impl Into<String>) {
        self.update_span(|span| span.exit_reason = Some(exit_reason.into()));
    }

    pub fn set_counter(&self, key: impl Into<String>, value: f64) {
        self.update_span(|span| {
            span.counters.insert(key.into(), value);
        });
    }

    pub fn incr_counter(&self, key: impl Into<String>, delta: f64) {
        self.update_span(|span| {
            let key = key.into();
            *span.counters.entry(key).or_insert(0.0) += delta;
        });
    }

    pub fn finish(mut self) {
        self.finish_inner();
    }

    fn update_span(&self, apply: impl FnOnce(&mut ToolpathDebugSpan)) {
        let mut state = self.recorder.inner.lock().expect("debug recorder poisoned");
        if let Some(span) = state.spans.get_mut(&self.span_id) {
            apply(span);
        }
    }

    fn finish_inner(&mut self) {
        if self.finished {
            return;
        }
        let mut state = self.recorder.inner.lock().expect("debug recorder poisoned");
        let parent_phase = state.spans.get(&self.span_id).and_then(|span| {
            span.parent_id.and_then(|parent_id| {
                state.spans.get(&parent_id).map(|parent| {
                    if parent.label.is_empty() {
                        parent.kind.clone()
                    } else {
                        parent.label.clone()
                    }
                })
            })
        });
        let elapsed_us = state.started_at.elapsed().as_micros() as u64;
        if let Some(span) = state.spans.get_mut(&self.span_id) {
            span.elapsed_us = elapsed_us.saturating_sub(span.start_us);
        }
        drop(state);
        self.recorder.publish_phase(parent_phase);
        self.finished = true;
    }
}

impl Drop for ToolpathDebugScope {
    fn drop(&mut self) {
        self.finish_inner();
    }
}

pub fn write_toolpath_debug_artifact(
    dir: &Path,
    file_stem: &str,
    artifact: &ToolpathDebugArtifact,
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
        "toolpath_debug".to_string()
    } else {
        output.to_string()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn recorder_serializes_spans_and_hotspots() {
        let recorder = ToolpathDebugRecorder::new("Adaptive 3D", "3D Rough");
        let ctx = recorder.root_context();
        let span = ctx.start_span("core_generate", "Generate");
        span.set_z_level(-2.5);
        span.set_counter("passes", 3.0);
        span.finish();
        ctx.record_hotspot(&HotspotRecord {
            kind: "adaptive_pass".into(),
            center_x: 12.0,
            center_y: -4.0,
            z_level: Some(-2.5),
            bucket_size_xy: 6.0,
            bucket_size_z: Some(0.5),
            elapsed_us: 3_200,
            pass_count: 1,
            step_count: 24,
            low_yield_exit_count: 1,
        });
        ctx.add_annotation(17, "Pass 1");
        let trace = recorder.finish();

        assert_eq!(trace.summary.span_count, 1);
        assert_eq!(trace.summary.hotspot_count, 1);
        assert_eq!(trace.annotations.len(), 1);

        let json = serde_json::to_string(&trace).expect("serialize trace");
        assert!(json.contains("\"core_generate\""));
        assert!(json.contains("\"adaptive_pass\""));
    }

    #[test]
    fn hotspot_aggregation_merges_nearby_samples() {
        let recorder = ToolpathDebugRecorder::new("Adaptive", "2D Rough");
        let ctx = recorder.root_context();
        ctx.record_hotspot(&HotspotRecord {
            kind: "adaptive_pass".into(),
            center_x: 10.0,
            center_y: 10.0,
            z_level: Some(-1.0),
            bucket_size_xy: 6.0,
            bucket_size_z: Some(0.5),
            elapsed_us: 1_000,
            pass_count: 1,
            step_count: 12,
            low_yield_exit_count: 0,
        });
        ctx.record_hotspot(&HotspotRecord {
            kind: "adaptive_pass".into(),
            center_x: 10.5,
            center_y: 9.5,
            z_level: Some(-0.9),
            bucket_size_xy: 6.0,
            bucket_size_z: Some(0.5),
            elapsed_us: 2_500,
            pass_count: 2,
            step_count: 18,
            low_yield_exit_count: 1,
        });

        let trace = recorder.finish();
        assert_eq!(trace.hotspots.len(), 1);
        assert_eq!(trace.hotspots[0].total_elapsed_us, 3_500);
        assert_eq!(trace.hotspots[0].pass_count, 3);
        assert_eq!(trace.hotspots[0].step_count, 30);
        assert_eq!(trace.hotspots[0].low_yield_exit_count, 1);
    }

    #[test]
    fn artifact_writer_creates_json_file() {
        let recorder = ToolpathDebugRecorder::new("Pocket 1", "Pocket");
        let trace = recorder.finish();
        let artifact = ToolpathDebugArtifact::new(
            1,
            "Pocket 1",
            "Pocket",
            "6.35mm End Mill",
            serde_json::json!({"stepover": 2.0}),
            trace,
        );

        let dir = std::env::temp_dir().join(format!(
            "rs_cam_debug_artifact_{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock before epoch")
                .as_nanos()
        ));
        let path = write_toolpath_debug_artifact(&dir, "Pocket 1", &artifact)
            .expect("write debug artifact");
        let text = std::fs::read_to_string(&path).expect("read debug artifact");
        assert!(text.contains("\"toolpath_name\": \"Pocket 1\""));
        std::fs::remove_file(path).ok();
        std::fs::remove_dir(dir).ok();
    }
}
