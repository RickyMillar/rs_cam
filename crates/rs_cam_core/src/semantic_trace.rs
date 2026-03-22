use crate::debug_trace::{TOOLPATH_DEBUG_SCHEMA_VERSION, ToolpathDebugBounds2, ToolpathDebugTrace};
use crate::toolpath::{Move, Toolpath};
use serde::Serialize;
use serde::{Deserialize, Serialize as DeriveSerialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, Eq, DeriveSerialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolpathSemanticKind {
    Operation,
    DepthLevel,
    Region,
    Pass,
    Entry,
    SlotClearing,
    Cleanup,
    ForcedClear,
    Contour,
    Raster,
    Row,
    Slice,
    Hole,
    Cycle,
    Chain,
    Band,
    Ramp,
    Ring,
    Ray,
    Curve,
    Dressup,
    FinishPass,
    OffsetPass,
    Centerline,
    BoundaryClip,
    Optimization,
}

#[derive(Debug, Clone, Default, PartialEq, DeriveSerialize, Deserialize)]
pub struct ToolpathSemanticParams {
    pub values: BTreeMap<String, Value>,
}

impl ToolpathSemanticParams {
    pub fn insert_json(&mut self, key: impl Into<String>, value: Value) {
        self.values.insert(key.into(), value);
    }

    pub fn insert<T: Serialize>(&mut self, key: impl Into<String>, value: T) {
        if let Ok(value) = serde_json::to_value(value) {
            self.values.insert(key.into(), value);
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, DeriveSerialize, Deserialize)]
pub struct ToolpathSemanticSummary {
    pub item_count: usize,
    pub move_linked_item_count: usize,
}

#[derive(Debug, Clone, PartialEq, DeriveSerialize, Deserialize)]
pub struct ToolpathSemanticItem {
    pub id: u64,
    pub parent_id: Option<u64>,
    pub kind: ToolpathSemanticKind,
    pub label: String,
    pub move_start: Option<usize>,
    pub move_end: Option<usize>,
    pub xy_bbox: Option<ToolpathDebugBounds2>,
    pub z_min: Option<f64>,
    pub z_max: Option<f64>,
    pub params: ToolpathSemanticParams,
    pub debug_span_id: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, DeriveSerialize, Deserialize)]
pub struct ToolpathSemanticTrace {
    pub schema_version: u32,
    pub toolpath_name: String,
    pub operation_label: String,
    pub summary: ToolpathSemanticSummary,
    pub items: Vec<ToolpathSemanticItem>,
}

#[derive(Debug, Clone, PartialEq, DeriveSerialize, Deserialize)]
pub struct ToolpathTraceArtifact {
    pub schema_version: u32,
    pub toolpath_id: usize,
    pub toolpath_name: String,
    pub operation_label: String,
    pub tool_summary: String,
    pub request_snapshot: Value,
    pub debug_trace: Option<ToolpathDebugTrace>,
    pub semantic_trace: Option<ToolpathSemanticTrace>,
}

impl ToolpathTraceArtifact {
    pub fn new(
        toolpath_id: usize,
        toolpath_name: impl Into<String>,
        operation_label: impl Into<String>,
        tool_summary: impl Into<String>,
        request_snapshot: Value,
        debug_trace: Option<ToolpathDebugTrace>,
        semantic_trace: Option<ToolpathSemanticTrace>,
    ) -> Self {
        Self {
            schema_version: TOOLPATH_DEBUG_SCHEMA_VERSION,
            toolpath_id,
            toolpath_name: toolpath_name.into(),
            operation_label: operation_label.into(),
            tool_summary: tool_summary.into(),
            request_snapshot,
            debug_trace,
            semantic_trace,
        }
    }
}

#[derive(Clone)]
pub struct ToolpathSemanticRecorder {
    inner: Arc<Mutex<SemanticState>>,
}

#[derive(Clone)]
pub struct ToolpathSemanticContext {
    recorder: ToolpathSemanticRecorder,
    parent_id: Option<u64>,
}

pub struct ToolpathSemanticScope {
    recorder: ToolpathSemanticRecorder,
    item_id: u64,
    finished: bool,
}

struct SemanticState {
    next_item_id: u64,
    toolpath_name: String,
    operation_label: String,
    items: BTreeMap<u64, ToolpathSemanticItem>,
}

impl ToolpathSemanticRecorder {
    pub fn new(toolpath_name: impl Into<String>, operation_label: impl Into<String>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(SemanticState {
                next_item_id: 1,
                toolpath_name: toolpath_name.into(),
                operation_label: operation_label.into(),
                items: BTreeMap::new(),
            })),
        }
    }

    pub fn root_context(&self) -> ToolpathSemanticContext {
        ToolpathSemanticContext {
            recorder: self.clone(),
            parent_id: None,
        }
    }

    pub fn finish(self) -> ToolpathSemanticTrace {
        let state = self.inner.lock().expect("semantic recorder poisoned");
        let mut items: Vec<_> = state.items.values().cloned().collect();
        items.sort_by_key(|item| (item.move_start.unwrap_or(usize::MAX), item.id));
        let move_linked_item_count = items
            .iter()
            .filter(|item| item.move_start.is_some() && item.move_end.is_some())
            .count();
        ToolpathSemanticTrace {
            schema_version: TOOLPATH_DEBUG_SCHEMA_VERSION,
            toolpath_name: state.toolpath_name.clone(),
            operation_label: state.operation_label.clone(),
            summary: ToolpathSemanticSummary {
                item_count: items.len(),
                move_linked_item_count,
            },
            items,
        }
    }

    fn start_item_with_parent(
        &self,
        parent_id: Option<u64>,
        kind: ToolpathSemanticKind,
        label: impl Into<String>,
    ) -> ToolpathSemanticScope {
        let mut state = self.inner.lock().expect("semantic recorder poisoned");
        let id = state.next_item_id;
        state.next_item_id += 1;
        state.items.insert(
            id,
            ToolpathSemanticItem {
                id,
                parent_id,
                kind,
                label: label.into(),
                move_start: None,
                move_end: None,
                xy_bbox: None,
                z_min: None,
                z_max: None,
                params: ToolpathSemanticParams::default(),
                debug_span_id: None,
            },
        );
        ToolpathSemanticScope {
            recorder: self.clone(),
            item_id: id,
            finished: false,
        }
    }
}

impl ToolpathSemanticContext {
    pub fn start_item(
        &self,
        kind: ToolpathSemanticKind,
        label: impl Into<String>,
    ) -> ToolpathSemanticScope {
        self.recorder
            .start_item_with_parent(self.parent_id, kind, label)
    }
}

impl ToolpathSemanticScope {
    pub fn id(&self) -> u64 {
        self.item_id
    }

    pub fn context(&self) -> ToolpathSemanticContext {
        ToolpathSemanticContext {
            recorder: self.recorder.clone(),
            parent_id: Some(self.item_id),
        }
    }

    pub fn set_move_range(&self, move_start: usize, move_end: usize) {
        self.update_item(|item| {
            item.move_start = Some(move_start);
            item.move_end = Some(move_end);
        });
    }

    pub fn set_xy_bbox(&self, bbox: ToolpathDebugBounds2) {
        self.update_item(|item| item.xy_bbox = Some(bbox));
    }

    pub fn set_z_range(&self, z_min: f64, z_max: f64) {
        self.update_item(|item| {
            item.z_min = Some(z_min);
            item.z_max = Some(z_max);
        });
    }

    pub fn set_param<T: Serialize>(&self, key: impl Into<String>, value: T) {
        self.update_item(|item| item.params.insert(key, value));
    }

    pub fn set_param_json(&self, key: impl Into<String>, value: Value) {
        self.update_item(|item| item.params.insert_json(key, value));
    }

    pub fn set_debug_span_id(&self, debug_span_id: u64) {
        self.update_item(|item| item.debug_span_id = Some(debug_span_id));
    }

    pub fn bind_to_toolpath(
        &self,
        toolpath: &Toolpath,
        move_start: usize,
        move_end_exclusive: usize,
    ) {
        if move_end_exclusive <= move_start || move_end_exclusive > toolpath.moves.len() {
            return;
        }
        let moves = &toolpath.moves[move_start..move_end_exclusive];
        if moves.is_empty() {
            return;
        }
        let mut z_min = f64::INFINITY;
        let mut z_max = f64::NEG_INFINITY;
        let mut xy_points = Vec::new();
        if move_start > 0 {
            let prev = &toolpath.moves[move_start - 1].target;
            xy_points.push((prev.x, prev.y));
            z_min = z_min.min(prev.z);
            z_max = z_max.max(prev.z);
        }
        for mv in moves {
            xy_points.push((mv.target.x, mv.target.y));
            z_min = z_min.min(mv.target.z);
            z_max = z_max.max(mv.target.z);
        }
        self.set_move_range(move_start, move_end_exclusive - 1);
        if let Some(bounds) = ToolpathDebugBounds2::from_points(xy_points.iter()) {
            self.set_xy_bbox(bounds);
        }
        self.set_z_range(z_min, z_max);
    }

    pub fn finish(mut self) {
        self.finish_inner();
    }

    fn update_item(&self, apply: impl FnOnce(&mut ToolpathSemanticItem)) {
        let mut state = self
            .recorder
            .inner
            .lock()
            .expect("semantic recorder poisoned");
        if let Some(item) = state.items.get_mut(&self.item_id) {
            apply(item);
        }
    }

    fn finish_inner(&mut self) {
        if self.finished {
            return;
        }
        self.finished = true;
    }
}

impl Drop for ToolpathSemanticScope {
    fn drop(&mut self) {
        self.finish_inner();
    }
}

pub struct ToolpathSemanticWriter<'a> {
    toolpath: &'a mut Toolpath,
}

impl<'a> ToolpathSemanticWriter<'a> {
    pub fn new(toolpath: &'a mut Toolpath) -> Self {
        Self { toolpath }
    }

    pub fn move_count(&self) -> usize {
        self.toolpath.moves.len()
    }

    pub fn append_toolpath(&mut self, scope: Option<&ToolpathSemanticScope>, mut other: Toolpath) {
        let start = self.toolpath.moves.len();
        self.toolpath.moves.append(&mut other.moves);
        if let Some(scope) = scope {
            scope.bind_to_toolpath(self.toolpath, start, self.toolpath.moves.len());
        }
    }

    pub fn push_move(&mut self, scope: Option<&ToolpathSemanticScope>, mv: Move) {
        let start = self.toolpath.moves.len();
        self.toolpath.moves.push(mv);
        if let Some(scope) = scope {
            scope.bind_to_toolpath(self.toolpath, start, self.toolpath.moves.len());
        }
    }

    pub fn bind_scope_to_current_range(&self, scope: &ToolpathSemanticScope, move_start: usize) {
        scope.bind_to_toolpath(self.toolpath, move_start, self.toolpath.moves.len());
    }

    pub fn toolpath(&self) -> &Toolpath {
        self.toolpath
    }

    pub fn finish(self) {}
}

pub fn item_ids_covering_move(trace: &ToolpathSemanticTrace, move_idx: usize) -> Vec<u64> {
    let mut item_ids = BTreeSet::new();
    for item in &trace.items {
        if item.move_start.is_some_and(|start| start <= move_idx)
            && item.move_end.is_some_and(|end| move_idx <= end)
        {
            item_ids.insert(item.id);
        }
    }
    item_ids.into_iter().collect()
}

pub fn write_toolpath_trace_artifact(
    dir: &Path,
    file_stem: &str,
    artifact: &ToolpathTraceArtifact,
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
        "toolpath_trace".to_string()
    } else {
        output.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geo::P3;

    #[test]
    fn semantic_recorder_serializes_items() {
        let recorder = ToolpathSemanticRecorder::new("Pocket 1", "Pocket");
        let ctx = recorder.root_context();
        let item = ctx.start_item(ToolpathSemanticKind::DepthLevel, "Level -1.0");
        item.set_param("z", -1.0);
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 5.0));
        tp.feed_to(P3::new(0.0, 0.0, -1.0), 100.0);
        tp.feed_to(P3::new(10.0, 0.0, -1.0), 200.0);
        item.bind_to_toolpath(&tp, 0, tp.moves.len());
        item.finish();

        let trace = recorder.finish();
        assert_eq!(trace.summary.item_count, 1);
        assert_eq!(trace.summary.move_linked_item_count, 1);
        let json = serde_json::to_string(&trace).expect("serialize semantic trace");
        assert!(json.contains("\"depth_level\""));
        assert!(json.contains("\"Level -1.0\""));
    }

    #[test]
    fn combined_artifact_writer_creates_json_file() {
        let artifact = ToolpathTraceArtifact::new(
            1,
            "Pocket 1",
            "Pocket",
            "6.35mm End Mill",
            serde_json::json!({"stepover": 2.0}),
            None,
            Some(ToolpathSemanticRecorder::new("Pocket 1", "Pocket").finish()),
        );

        let dir = std::env::temp_dir().join(format!(
            "rs_cam_trace_artifact_{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock before epoch")
                .as_nanos()
        ));
        let path = write_toolpath_trace_artifact(&dir, "Pocket 1", &artifact)
            .expect("write trace artifact");
        let text = std::fs::read_to_string(&path).expect("read trace artifact");
        assert!(text.contains("\"toolpath_name\": \"Pocket 1\""));
        std::fs::remove_file(path).ok();
        std::fs::remove_dir(dir).ok();
    }
}
