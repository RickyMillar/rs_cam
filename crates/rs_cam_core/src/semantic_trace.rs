use crate::debug_trace::{TOOLPATH_DEBUG_SCHEMA_VERSION, ToolpathDebugBounds2, ToolpathDebugTrace};
use crate::geo::{BoundingBox3, P3};
use crate::ids::ToolpathId;
use crate::toolpath::{Move, Toolpath};
use crate::toolpath_spans::{AnnotatedToolpath, Span, SpanKind, SpanPayload};
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
    /// First move this item covers, INCLUSIVE. See [`Self::move_end`] for
    /// the post-transform contract.
    pub move_start: Option<usize>,
    /// Last move this item covers, **INCLUSIVE** (the structural
    /// [`crate::toolpath_spans::Span::end_move`] is exclusive — convert
    /// with `move_end + 1` before comparing).
    ///
    /// # Post-transform contract (deleted-move policy)
    ///
    /// Move links are remapped through every post-generation transform
    /// (dressups, boundary clip, entry-descent splitting) by
    /// [`SemanticLinkCarrier`] / [`ToolpathSemanticRecorder::remap_move_links`],
    /// using the same provenance maps that remap
    /// `AnnotatedToolpath::spans`. When a transform deletes every move an
    /// item covered — or scatters them so the remapped bounds would be a
    /// lie (TSP's foreign-intrusion drop) — the item is **UNLINKED**:
    /// `move_start` and `move_end` both become `None`.
    ///
    /// Unlink, not drop and not clamp:
    /// * dropping the item would lose the planner intent it records (label,
    ///   params, band/strategy, parentage) which is still true;
    /// * clamping would fabricate a range that no longer describes those
    ///   moves — the defect this contract exists to prevent.
    ///
    /// So a linked range is always in-bounds and always means what it says;
    /// `None` means "this item's moves can no longer be identified", and
    /// `ToolpathSemanticSummary::move_linked_item_count` counts the
    /// survivors.
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
    pub toolpath_id: ToolpathId,
    pub toolpath_name: String,
    pub operation_label: String,
    pub tool_summary: String,
    pub request_snapshot: Value,
    pub debug_trace: Option<ToolpathDebugTrace>,
    pub semantic_trace: Option<ToolpathSemanticTrace>,
}

impl ToolpathTraceArtifact {
    pub fn new(
        toolpath_id: ToolpathId,
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

    // SAFETY: Mutex::lock only fails if poisoned (panic in another thread)
    #[allow(clippy::expect_used)]
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
        // SAFETY: Mutex::lock only fails if poisoned (panic in another thread)
        #[allow(clippy::expect_used)]
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

    /// Half-open `(item_id, start, end_exclusive)` for every item that
    /// currently carries a move link. The stored `move_end` is inclusive;
    /// this converts once so callers only ever deal with the half-open
    /// convention the span machinery uses.
    // SAFETY: Mutex::lock only fails if poisoned (panic in another thread)
    #[allow(clippy::expect_used)]
    fn move_links(&self) -> Vec<(u64, usize, usize)> {
        let state = self.inner.lock().expect("semantic recorder poisoned");
        state
            .items
            .values()
            .filter_map(|item| {
                let start = item.move_start?;
                let end = item.move_end?;
                Some((item.id, start, end.saturating_add(1)))
            })
            .collect()
    }

    /// Write back a batch of move links. `None` unlinks the item — see the
    /// deleted-move policy on [`ToolpathSemanticItem::move_end`].
    // SAFETY: Mutex::lock only fails if poisoned (panic in another thread)
    #[allow(clippy::expect_used)]
    fn apply_links(&self, links: &[(u64, Option<(usize, usize)>)]) {
        let mut state = self.inner.lock().expect("semantic recorder poisoned");
        for (id, link) in links {
            if let Some(item) = state.items.get_mut(id) {
                match link {
                    Some((start, end)) => {
                        item.move_start = Some(*start);
                        item.move_end = Some(*end);
                    }
                    None => {
                        item.move_start = None;
                        item.move_end = None;
                    }
                }
            }
        }
    }

    /// Remap every recorded move link through a per-input-move provenance
    /// map — the same `mapping` contract [`Span::remap`] takes
    /// (`mapping[i]` = first output index produced from input move `i`,
    /// last entry = total output move count).
    ///
    /// Used by the transforms that hand out a provenance map directly (the
    /// boundary clip, `optimize_entry_descents_with_provenance`). Transforms
    /// that only expose their remap through the span vector use
    /// [`SemanticLinkCarrier`] instead. Both apply the same deleted-move
    /// policy: an item whose moves are all gone is unlinked.
    pub fn remap_move_links(&self, mapping: &[usize], new_move_count: usize) {
        let updates: Vec<(u64, Option<(usize, usize)>)> = self
            .move_links()
            .into_iter()
            .map(|(id, start, end)| {
                let remapped = Span::new(start, end, SpanKind::SemanticLink).remap(mapping);
                (
                    id,
                    linked_range(remapped.start_move, remapped.end_move, new_move_count),
                )
            })
            .collect();
        self.apply_links(&updates);
    }
}

/// Convert a half-open remapped range into the stored inclusive link,
/// applying the deleted-move policy: an empty range, or one that starts
/// past the end of the toolpath, unlinks the item; an over-long end is
/// clamped to the last move so a linked range is always sliceable.
fn linked_range(
    start: usize,
    end_exclusive: usize,
    n_moves: usize,
) -> Option<(usize, usize)> {
    if end_exclusive <= start || start >= n_moves {
        return None;
    }
    Some((start, end_exclusive.min(n_moves) - 1))
}

// ── SemanticLinkCarrier ─────────────────────────────────────────────────

/// Carries semantic move links through a transform that remaps
/// `AnnotatedToolpath::spans` but has no other way to report its
/// provenance map (every dressup, arc-fit, segment merge, air-cut filter
/// and TSP reorder in [`crate::compute::execute::apply_dressups`]).
///
/// [`Self::attach`] appends one [`SpanKind::SemanticLink`] span per
/// DISTINCT linked range; the transform remaps it exactly as it remaps the
/// real spans; [`Self::detach`] strips the carrier spans back out and
/// writes the new ranges into the recorder. Because a carrier span is a
/// byte-identical copy of the item's range, an item survives a transform
/// **iff a structural span over the same moves would have** — which is what
/// keeps the semantic and structural region systems in agreement after the
/// pipeline, not only at generation time.
///
/// Deleted moves: a carrier span that fully collapses (air-cut filter,
/// clip) or that TSP drops for foreign intrusion comes back missing, and
/// the item is UNLINKED (`move_start`/`move_end` = `None`) — see
/// [`ToolpathSemanticItem::move_end`]. Ranges that survive are also
/// bound-checked against the post-transform move count, so a transform that
/// declines to remap (e.g. one that passes spans through untouched because
/// `spans_valid` was already false) can leave a stale link but never an
/// out-of-bounds one.
pub struct SemanticLinkCarrier {
    entries: Vec<CarrierEntry>,
}

/// One distinct move range, plus every item that shares it. `probe_id` is
/// the id written into the carrier span's payload.
struct CarrierEntry {
    probe_id: u64,
    item_ids: Vec<u64>,
}

impl SemanticLinkCarrier {
    /// Append the carrier spans. No-op (empty carrier) when nothing is
    /// linked yet.
    pub fn attach(recorder: &ToolpathSemanticRecorder, annotated: &mut AnnotatedToolpath) -> Self {
        let mut by_range: BTreeMap<(usize, usize), Vec<u64>> = BTreeMap::new();
        for (id, start, end) in recorder.move_links() {
            by_range.entry((start, end)).or_default().push(id);
        }
        let mut entries = Vec::with_capacity(by_range.len());
        for ((start, end), item_ids) in by_range {
            let Some(&probe_id) = item_ids.first() else {
                continue;
            };
            annotated.spans.push(
                Span::new(start, end, SpanKind::SemanticLink)
                    .with_payload(SpanPayload::SemanticLink { item_id: probe_id }),
            );
            entries.push(CarrierEntry { probe_id, item_ids });
        }
        Self { entries }
    }

    /// Strip every carrier span back out of `annotated` and write the
    /// remapped ranges into `recorder`. Always removes ALL
    /// [`SpanKind::SemanticLink`] spans, so a carrier can never leak into a
    /// stored toolpath even if a transform duplicated one.
    pub fn detach(self, recorder: &ToolpathSemanticRecorder, annotated: &mut AnnotatedToolpath) {
        let mut remapped: BTreeMap<u64, (usize, usize)> = BTreeMap::new();
        annotated.spans.retain(|span| {
            if span.kind != SpanKind::SemanticLink {
                return true;
            }
            if let Some(SpanPayload::SemanticLink { item_id }) = span.payload {
                remapped.insert(item_id, (span.start_move, span.end_move));
            }
            false
        });
        let n_moves = annotated.toolpath.moves.len();
        let mut updates: Vec<(u64, Option<(usize, usize)>)> = Vec::new();
        for entry in self.entries {
            let link = remapped
                .get(&entry.probe_id)
                .and_then(|&(start, end)| linked_range(start, end, n_moves));
            for id in entry.item_ids {
                updates.push((id, link));
            }
        }
        recorder.apply_links(&updates);
    }
}

impl ToolpathSemanticContext {
    /// The recorder this context writes into — the handle a transform needs
    /// to remap already-recorded move links
    /// ([`ToolpathSemanticRecorder::remap_move_links`]) while it is being
    /// handed a context to record its OWN item.
    pub fn recorder(&self) -> &ToolpathSemanticRecorder {
        &self.recorder
    }

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

    #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
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

    // SAFETY: Mutex::lock only fails if poisoned (panic in another thread)
    #[allow(clippy::expect_used)]
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

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
pub fn enrich_traces(
    debug_trace: &mut ToolpathDebugTrace,
    semantic_trace: &mut ToolpathSemanticTrace,
) {
    for span_idx in 0..debug_trace.spans.len() {
        let span_id = debug_trace.spans[span_idx].id;
        let linked_item_index =
            best_item_for_span(span_id, &debug_trace.spans[span_idx], semantic_trace);
        if let Some(item_index) = linked_item_index {
            let item = &semantic_trace.items[item_index];
            if debug_trace.spans[span_idx].move_start.is_none()
                && let (Some(move_start), Some(move_end)) = (item.move_start, item.move_end)
            {
                debug_trace.spans[span_idx].move_start = Some(move_start);
                debug_trace.spans[span_idx].move_end = Some(move_end);
            }
            if semantic_trace.items[item_index].debug_span_id.is_none() {
                semantic_trace.items[item_index].debug_span_id = Some(span_id);
            }
        }
    }

    let span_cache: Vec<_> = debug_trace
        .spans
        .iter()
        .map(|span| (span.id, span_bbox3(span)))
        .collect();
    let item_cache: Vec<_> = semantic_trace
        .items
        .iter()
        .enumerate()
        .map(|(index, item)| (index, semantic_item_bbox3(item)))
        .collect();

    for hotspot in &mut debug_trace.hotspots {
        let hotspot_bbox = hotspot_bbox3(hotspot);
        let representative = debug_trace
            .spans
            .iter()
            .enumerate()
            .filter_map(|(span_index, span)| {
                let bbox = span_cache[span_index].1.as_ref()?;
                let overlap = bbox_overlap_volume(bbox, &hotspot_bbox)?;
                let kind_match =
                    hotspot.kind.contains(&span.kind) || span.kind.contains(&hotspot.kind);
                Some((
                    span_index,
                    kind_match,
                    overlap,
                    span.elapsed_us,
                    span.move_start.is_some(),
                ))
            })
            .max_by(|left, right| {
                left.1
                    .cmp(&right.1)
                    .then_with(|| left.4.cmp(&right.4))
                    .then_with(|| left.3.cmp(&right.3))
                    .then_with(|| left.2.total_cmp(&right.2))
            })
            .map(|(span_index, _, _, _, _)| span_index);

        if let Some(span_index) = representative {
            let span = &debug_trace.spans[span_index];
            hotspot.representative_span_id = Some(span.id);
            hotspot.move_start = hotspot.move_start.or(span.move_start);
            hotspot.move_end = hotspot.move_end.or(span.move_end);
        }

        let linked_item = representative
            .and_then(|span_index| {
                let span_id = debug_trace.spans[span_index].id;
                semantic_trace
                    .items
                    .iter()
                    .enumerate()
                    .filter(|(_, item)| item.debug_span_id == Some(span_id))
                    .max_by_key(|(_, item)| {
                        std::cmp::Reverse(
                            item.move_end
                                .unwrap_or(usize::MAX)
                                .saturating_sub(item.move_start.unwrap_or(0)),
                        )
                    })
                    .map(|(item_index, _)| item_index)
            })
            .or_else(|| {
                semantic_trace
                    .items
                    .iter()
                    .enumerate()
                    .filter_map(|(item_index, item)| {
                        let bbox = item_cache[item_index].1.as_ref()?;
                        let overlap = bbox_overlap_volume(bbox, &hotspot_bbox)?;
                        Some((
                            item_index,
                            overlap,
                            item.move_start.is_some() && item.move_end.is_some(),
                            item.move_end
                                .unwrap_or(usize::MAX)
                                .saturating_sub(item.move_start.unwrap_or(0)),
                        ))
                    })
                    .max_by(|left, right| {
                        left.2
                            .cmp(&right.2)
                            .then_with(|| left.1.total_cmp(&right.1))
                            .then_with(|| right.3.cmp(&left.3))
                    })
                    .map(|(item_index, _, _, _)| item_index)
            });

        if let Some(item_index) = linked_item {
            let item = &semantic_trace.items[item_index];
            hotspot.semantic_item_id = Some(item.id);
            if hotspot.move_start.is_none()
                && let (Some(move_start), Some(move_end)) = (item.move_start, item.move_end)
            {
                hotspot.move_start = Some(move_start);
                hotspot.move_end = Some(move_end);
            }
        }
    }
}

fn best_item_for_span(
    span_id: u64,
    span: &crate::debug_trace::ToolpathDebugSpan,
    semantic_trace: &ToolpathSemanticTrace,
) -> Option<usize> {
    semantic_trace
        .items
        .iter()
        .enumerate()
        .filter_map(|(item_index, item)| {
            let direct = item.debug_span_id == Some(span_id);
            let bbox_score = match (span_bbox3(span), semantic_item_bbox3(item)) {
                (Some(span_bbox), Some(item_bbox)) => bbox_overlap_volume(&span_bbox, &item_bbox),
                _ => None,
            }
            .unwrap_or(0.0);
            let z_match = span
                .z_level
                .zip(item.z_min.zip(item.z_max))
                .is_none_or(|(z, (z_min, z_max))| z >= z_min - 1e-6 && z <= z_max + 1e-6);
            if !direct && bbox_score <= 0.0 && !z_match {
                return None;
            }
            let move_span = item
                .move_end
                .unwrap_or(usize::MAX)
                .saturating_sub(item.move_start.unwrap_or(0));
            Some((
                item_index,
                direct,
                z_match,
                bbox_score,
                std::cmp::Reverse(move_span),
            ))
        })
        .max_by(|left, right| {
            left.1
                .cmp(&right.1)
                .then_with(|| left.2.cmp(&right.2))
                .then_with(|| left.3.total_cmp(&right.3))
                .then_with(|| left.4.cmp(&right.4))
        })
        .map(|(item_index, _, _, _, _)| item_index)
}

fn semantic_item_bbox3(item: &ToolpathSemanticItem) -> Option<BoundingBox3> {
    let xy = item.xy_bbox?;
    let z_min = item.z_min?;
    let z_max = item.z_max?;
    Some(BoundingBox3 {
        min: P3::new(xy.min_x, xy.min_y, z_min),
        max: P3::new(xy.max_x, xy.max_y, z_max),
    })
}

fn span_bbox3(span: &crate::debug_trace::ToolpathDebugSpan) -> Option<BoundingBox3> {
    let xy = span.xy_bbox?;
    let z = span.z_level?;
    Some(BoundingBox3 {
        min: P3::new(xy.min_x, xy.min_y, z),
        max: P3::new(xy.max_x, xy.max_y, z),
    })
}

fn hotspot_bbox3(hotspot: &crate::debug_trace::ToolpathHotspot) -> BoundingBox3 {
    let half_xy = hotspot.bucket_size_xy * 0.5;
    let half_z = hotspot.bucket_size_z.unwrap_or(1.0) * 0.5;
    let z_center = hotspot.z_bucket_center.unwrap_or(0.0);
    BoundingBox3 {
        min: P3::new(
            hotspot.center_x - half_xy,
            hotspot.center_y - half_xy,
            z_center - half_z,
        ),
        max: P3::new(
            hotspot.center_x + half_xy,
            hotspot.center_y + half_xy,
            z_center + half_z,
        ),
    }
}

fn bbox_overlap_volume(left: &BoundingBox3, right: &BoundingBox3) -> Option<f64> {
    let overlap_x = (left.max.x.min(right.max.x) - left.min.x.max(right.min.x)).max(0.0);
    let overlap_y = (left.max.y.min(right.max.y) - left.min.y.max(right.min.y)).max(0.0);
    let overlap_z = (left.max.z.min(right.max.z) - left.min.z.max(right.min.z)).max(0.0);
    (overlap_x > 0.0 && overlap_y > 0.0 && overlap_z >= 0.0)
        .then_some(overlap_x * overlap_y * overlap_z.max(1.0))
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
        "toolpath_trace".to_owned()
    } else {
        output.to_owned()
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::str_to_string
)]
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

    fn toolpath_with_moves(n: usize) -> Toolpath {
        let mut tp = Toolpath::new();
        for i in 0..n {
            tp.feed_to(P3::new(i as f64, 0.0, -1.0), 100.0);
        }
        tp
    }

    /// The deleted-move policy, stated on the smallest possible transform:
    /// an item whose moves all survive is remapped, an item whose moves the
    /// transform dropped is UNLINKED (not clamped, not deleted), and no
    /// carrier span is left behind on the toolpath.
    #[test]
    fn carrier_remaps_survivors_and_unlinks_deleted_items() {
        use crate::toolpath_spans::MoveRemap;

        let recorder = ToolpathSemanticRecorder::new("Pocket 1", "Pocket");
        let ctx = recorder.root_context();
        let keep = ctx.start_item(ToolpathSemanticKind::Pass, "Keep");
        let lose = ctx.start_item(ToolpathSemanticKind::Pass, "Lose");
        keep.set_move_range(0, 2); // inclusive → half-open 0..3
        lose.set_move_range(3, 5); // inclusive → half-open 3..6

        let mut annotated = AnnotatedToolpath::new(toolpath_with_moves(6));
        let carrier = SemanticLinkCarrier::attach(&recorder, &mut annotated);
        assert_eq!(
            annotated
                .spans
                .iter()
                .filter(|s| s.kind == SpanKind::SemanticLink)
                .count(),
            2,
            "one carrier span per distinct linked range"
        );

        // Transform: moves 0..3 shift up by one (something was inserted in
        // front of them), moves 3..6 are deleted outright.
        let remap = MoveRemap {
            old_to_new: vec![Some(1..2), Some(2..3), Some(3..4), None, None, None],
        };
        annotated.spans = remap.remap_spans(&annotated.spans, 4);
        annotated.toolpath = toolpath_with_moves(4);
        carrier.detach(&recorder, &mut annotated);

        assert!(
            annotated.spans.is_empty(),
            "detach must strip every carrier span — they must never ship"
        );

        let trace = recorder.finish();
        let by_label = |label: &str| {
            trace
                .items
                .iter()
                .find(|i| i.label == label)
                .expect("item recorded")
                .clone()
        };
        let keep_item = by_label("Keep");
        assert_eq!((keep_item.move_start, keep_item.move_end), (Some(1), Some(3)));
        let lose_item = by_label("Lose");
        assert_eq!(
            (lose_item.move_start, lose_item.move_end),
            (None, None),
            "an item whose moves were all deleted is unlinked, not clamped"
        );
        assert_eq!(trace.summary.item_count, 2, "the item itself survives");
        assert_eq!(trace.summary.move_linked_item_count, 1);
    }

    /// The provenance-map form (boundary clip, entry-descent splitter): an
    /// item covering the whole toolpath must still cover the whole toolpath
    /// after the transform inserted moves inside its range.
    #[test]
    fn remap_move_links_follows_inserted_moves() {
        let recorder = ToolpathSemanticRecorder::new("Pocket 1", "Pocket");
        let ctx = recorder.root_context();
        let op = ctx.start_item(ToolpathSemanticKind::Operation, "Pocket");
        op.set_move_range(0, 2); // covers all 3 input moves

        // Each input move produced two output moves: mapping[i] = 2i, with
        // the total-count sentinel last.
        recorder.remap_move_links(&[0, 2, 4, 6], 6);

        let trace = recorder.finish();
        let item = &trace.items[0];
        assert_eq!((item.move_start, item.move_end), (Some(0), Some(5)));
    }

    /// A stale link (transform declined to remap because spans were already
    /// invalid) must never come back out of bounds.
    #[test]
    fn remap_move_links_never_returns_an_out_of_bounds_link() {
        let recorder = ToolpathSemanticRecorder::new("Pocket 1", "Pocket");
        let ctx = recorder.root_context();
        let inside = ctx.start_item(ToolpathSemanticKind::Pass, "Overhang");
        inside.set_move_range(0, 9);
        let outside = ctx.start_item(ToolpathSemanticKind::Pass, "Past the end");
        outside.set_move_range(8, 9);

        // Identity mapping over 10 input moves, but only 4 moves survive.
        recorder.remap_move_links(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10], 4);

        let trace = recorder.finish();
        let by_label = |label: &str| {
            trace
                .items
                .iter()
                .find(|i| i.label == label)
                .expect("item recorded")
                .clone()
        };
        assert_eq!(
            (
                by_label("Overhang").move_start,
                by_label("Overhang").move_end
            ),
            (Some(0), Some(3)),
            "an over-long end clamps to the last move so the range stays sliceable"
        );
        assert_eq!(
            (
                by_label("Past the end").move_start,
                by_label("Past the end").move_end
            ),
            (None, None),
            "a range starting past the end is unlinked, not clamped to nothing"
        );
    }

    #[test]
    fn combined_artifact_writer_creates_json_file() {
        let artifact = ToolpathTraceArtifact::new(
            ToolpathId(1),
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

    #[test]
    fn enrich_traces_links_spans_and_hotspots_to_semantic_items() {
        let mut debug_trace = ToolpathDebugTrace {
            schema_version: TOOLPATH_DEBUG_SCHEMA_VERSION,
            toolpath_name: "Adaptive".to_string(),
            operation_label: "Adaptive".to_string(),
            summary: crate::debug_trace::ToolpathDebugSummary {
                total_elapsed_us: 10_000,
                span_count: 1,
                hotspot_count: 1,
                dominant_span_kind: Some("adaptive_pass".to_string()),
                dominant_span_label: Some("Pass 1".to_string()),
                dominant_span_elapsed_us: Some(10_000),
            },
            spans: vec![crate::debug_trace::ToolpathDebugSpan {
                id: 7,
                parent_id: None,
                kind: "adaptive_pass".to_string(),
                label: "Pass 1".to_string(),
                start_us: 0,
                elapsed_us: 10_000,
                xy_bbox: Some(ToolpathDebugBounds2 {
                    min_x: 0.0,
                    max_x: 10.0,
                    min_y: 0.0,
                    max_y: 10.0,
                }),
                z_level: Some(-1.0),
                move_start: None,
                move_end: None,
                exit_reason: None,
                counters: BTreeMap::new(),
            }],
            hotspots: vec![crate::debug_trace::ToolpathHotspot {
                kind: "adaptive_pass".to_string(),
                center_x: 5.0,
                center_y: 5.0,
                z_bucket_center: Some(-1.0),
                bucket_size_xy: 10.0,
                bucket_size_z: Some(1.0),
                total_elapsed_us: 10_000,
                span_count: 1,
                pass_count: 1,
                step_count: 20,
                low_yield_exit_count: 0,
                representative_span_id: None,
                move_start: None,
                move_end: None,
                semantic_item_id: None,
            }],
            annotations: Vec::new(),
        };

        let mut semantic_trace = ToolpathSemanticTrace {
            schema_version: TOOLPATH_DEBUG_SCHEMA_VERSION,
            toolpath_name: "Adaptive".to_string(),
            operation_label: "Adaptive".to_string(),
            summary: ToolpathSemanticSummary {
                item_count: 1,
                move_linked_item_count: 1,
            },
            items: vec![ToolpathSemanticItem {
                id: 3,
                parent_id: None,
                kind: ToolpathSemanticKind::Pass,
                label: "Pass 1".to_string(),
                move_start: Some(4),
                move_end: Some(12),
                xy_bbox: Some(ToolpathDebugBounds2 {
                    min_x: 0.0,
                    max_x: 10.0,
                    min_y: 0.0,
                    max_y: 10.0,
                }),
                z_min: Some(-1.0),
                z_max: Some(-1.0),
                params: ToolpathSemanticParams::default(),
                debug_span_id: Some(7),
            }],
        };

        enrich_traces(&mut debug_trace, &mut semantic_trace);

        assert_eq!(debug_trace.spans[0].move_start, Some(4));
        assert_eq!(debug_trace.spans[0].move_end, Some(12));
        assert_eq!(debug_trace.hotspots[0].representative_span_id, Some(7));
        assert_eq!(debug_trace.hotspots[0].semantic_item_id, Some(3));
        assert_eq!(debug_trace.hotspots[0].move_start, Some(4));
        assert_eq!(debug_trace.hotspots[0].move_end, Some(12));
    }
}
