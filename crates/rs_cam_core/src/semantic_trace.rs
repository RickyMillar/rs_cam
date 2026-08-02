use crate::debug_trace::{TOOLPATH_DEBUG_SCHEMA_VERSION, ToolpathDebugBounds2, ToolpathDebugTrace};
use crate::geo::{BoundingBox3, P3};
use crate::ids::ToolpathId;
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

/// The typed vocabulary of [`ToolpathSemanticParams`] keys (C4).
///
/// `ToolpathSemanticParams` is a `BTreeMap<String, Value>` bag, and PR-0
/// recorded that typing and provenance die at that boundary. The bag stays —
/// the values are genuinely heterogeneous and the JSON wire is a
/// compatibility surface — but the KEY half of it does not have to be free
/// text. Before this enum the vocabulary existed only as 74 string literals
/// spread across `compute/annotate.rs`, `compute/execute.rs`,
/// `session/compute.rs` and the GUI worker, with the readers in `narrate.rs`
/// carrying their own copies of six of them. A typo on either side produced
/// a silently absent parameter, never an error.
///
/// Every constructor and reader now takes a `SemanticKey`. [`Self::as_str`]
/// is the ONLY place a key string exists, so the emitted JSON is unchanged
/// and a misspelling is a compile error.
///
/// **H4's mix tables — and any other report that groups semantic items —
/// must be built on this enum, on
/// [`crate::toolpath_spans::RegionSpanRole`], or on
/// [`crate::unified_finish::RegionKind::from_span_label`]. Never on a
/// `label`, and never on a key literal spelled out at the consumer.**
///
/// Adding a parameter means adding a variant here: the match in
/// [`Self::as_str`] is exhaustive, and [`Self::ALL`] is what
/// [`Self::from_key`] derives from, the same contract
/// [`crate::toolpath_spans::SpanKind::ALL`] carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SemanticKey {
    /// `agent_walk_cut_length_mm`
    AgentWalkCutLengthMm,
    /// `angle_deg`
    AngleDeg,
    /// `area_mm2`
    AreaMm2,
    /// `band`
    Band,
    /// `barrier_count`
    BarrierCount,
    /// `cell_count`
    CellCount,
    /// `center_x`
    CenterX,
    /// `center_y`
    CenterY,
    /// `chain_index`
    ChainIndex,
    /// `chain_total`
    ChainTotal,
    /// `containment`
    Containment,
    /// `continuous`
    Continuous,
    /// `contour_index`
    ContourIndex,
    /// `contour_total`
    ContourTotal,
    /// `cycle_index`
    CycleIndex,
    /// `dropped_micro_region_count`
    DroppedMicroRegionCount,
    /// `entry_x`
    EntryX,
    /// `entry_y`
    EntryY,
    /// `entry_z`
    EntryZ,
    /// `exit_reason`
    ExitReason,
    /// `hole_index`
    HoleIndex,
    /// `idle_count`
    IdleCount,
    /// `is_centerline`
    IsCenterline,
    /// `keep_out_count`
    KeepOutCount,
    /// `kind`
    Kind,
    /// `lead_in_feed_rate`
    LeadInFeedRate,
    /// `lead_out_feed_rate`
    LeadOutFeedRate,
    /// `level_index`
    LevelIndex,
    /// `level_total`
    LevelTotal,
    /// `line_index`
    LineIndex,
    /// `line_total`
    LineTotal,
    /// `link_feed_rate`
    LinkFeedRate,
    /// `lower_level_index`
    LowerLevelIndex,
    /// `lower_z`
    LowerZ,
    /// `marching_squares_regions`
    MarchingSquaresRegions,
    /// `max_angle_deg`
    MaxAngleDeg,
    /// `max_feed_rate`
    MaxFeedRate,
    /// `max_link_distance`
    MaxLinkDistance,
    /// `move_scope`
    MoveScope,
    /// `nominal_feed_rate`
    NominalFeedRate,
    /// `offset_index`
    OffsetIndex,
    /// `offset_mm`
    OffsetMm,
    /// `offset_total`
    OffsetTotal,
    /// `pass_index`
    PassIndex,
    /// `perimeter_sweep_length_mm`
    PerimeterSweepLengthMm,
    /// `pitch`
    Pitch,
    /// `radius`
    Radius,
    /// `radius_mm`
    RadiusMm,
    /// `ramp_index`
    RampIndex,
    /// `ramp_rate`
    RampRate,
    /// `ramp_total`
    RampTotal,
    /// `region_areas_mm2`
    RegionAreasMm2,
    /// `region_count`
    RegionCount,
    /// `region_id`
    RegionId,
    /// `region_index`
    RegionIndex,
    /// `region_total`
    RegionTotal,
    /// `residual_cleanup_cell_count`
    ResidualCleanupCellCount,
    /// `ring_index`
    RingIndex,
    /// `ring_total`
    RingTotal,
    /// `safe_z`
    SafeZ,
    /// `search_evaluations`
    SearchEvaluations,
    /// `short`
    Short,
    /// `skipped`
    Skipped,
    /// `step_count`
    StepCount,
    /// `strategy`
    Strategy,
    /// `style`
    Style,
    /// `terrace_index`
    TerraceIndex,
    /// `terrace_total`
    TerraceTotal,
    /// `tolerance`
    Tolerance,
    /// `tool_radius`
    ToolRadius,
    /// `upper_level_index`
    UpperLevelIndex,
    /// `upper_z`
    UpperZ,
    /// `yield_ratio`
    YieldRatio,
    /// `z_level`
    ZLevel,
}

impl SemanticKey {
    /// Every variant, in declaration order (alphabetical by wire key).
    pub const ALL: [Self; 74] = [
        Self::AgentWalkCutLengthMm,
        Self::AngleDeg,
        Self::AreaMm2,
        Self::Band,
        Self::BarrierCount,
        Self::CellCount,
        Self::CenterX,
        Self::CenterY,
        Self::ChainIndex,
        Self::ChainTotal,
        Self::Containment,
        Self::Continuous,
        Self::ContourIndex,
        Self::ContourTotal,
        Self::CycleIndex,
        Self::DroppedMicroRegionCount,
        Self::EntryX,
        Self::EntryY,
        Self::EntryZ,
        Self::ExitReason,
        Self::HoleIndex,
        Self::IdleCount,
        Self::IsCenterline,
        Self::KeepOutCount,
        Self::Kind,
        Self::LeadInFeedRate,
        Self::LeadOutFeedRate,
        Self::LevelIndex,
        Self::LevelTotal,
        Self::LineIndex,
        Self::LineTotal,
        Self::LinkFeedRate,
        Self::LowerLevelIndex,
        Self::LowerZ,
        Self::MarchingSquaresRegions,
        Self::MaxAngleDeg,
        Self::MaxFeedRate,
        Self::MaxLinkDistance,
        Self::MoveScope,
        Self::NominalFeedRate,
        Self::OffsetIndex,
        Self::OffsetMm,
        Self::OffsetTotal,
        Self::PassIndex,
        Self::PerimeterSweepLengthMm,
        Self::Pitch,
        Self::Radius,
        Self::RadiusMm,
        Self::RampIndex,
        Self::RampRate,
        Self::RampTotal,
        Self::RegionAreasMm2,
        Self::RegionCount,
        Self::RegionId,
        Self::RegionIndex,
        Self::RegionTotal,
        Self::ResidualCleanupCellCount,
        Self::RingIndex,
        Self::RingTotal,
        Self::SafeZ,
        Self::SearchEvaluations,
        Self::Short,
        Self::Skipped,
        Self::StepCount,
        Self::Strategy,
        Self::Style,
        Self::TerraceIndex,
        Self::TerraceTotal,
        Self::Tolerance,
        Self::ToolRadius,
        Self::UpperLevelIndex,
        Self::UpperZ,
        Self::YieldRatio,
        Self::ZLevel,
    ];

    /// The wire key. This is the single source of the JSON vocabulary —
    /// changing one of these strings changes the emitted trace.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AgentWalkCutLengthMm => "agent_walk_cut_length_mm",
            Self::AngleDeg => "angle_deg",
            Self::AreaMm2 => "area_mm2",
            Self::Band => "band",
            Self::BarrierCount => "barrier_count",
            Self::CellCount => "cell_count",
            Self::CenterX => "center_x",
            Self::CenterY => "center_y",
            Self::ChainIndex => "chain_index",
            Self::ChainTotal => "chain_total",
            Self::Containment => "containment",
            Self::Continuous => "continuous",
            Self::ContourIndex => "contour_index",
            Self::ContourTotal => "contour_total",
            Self::CycleIndex => "cycle_index",
            Self::DroppedMicroRegionCount => "dropped_micro_region_count",
            Self::EntryX => "entry_x",
            Self::EntryY => "entry_y",
            Self::EntryZ => "entry_z",
            Self::ExitReason => "exit_reason",
            Self::HoleIndex => "hole_index",
            Self::IdleCount => "idle_count",
            Self::IsCenterline => "is_centerline",
            Self::KeepOutCount => "keep_out_count",
            Self::Kind => "kind",
            Self::LeadInFeedRate => "lead_in_feed_rate",
            Self::LeadOutFeedRate => "lead_out_feed_rate",
            Self::LevelIndex => "level_index",
            Self::LevelTotal => "level_total",
            Self::LineIndex => "line_index",
            Self::LineTotal => "line_total",
            Self::LinkFeedRate => "link_feed_rate",
            Self::LowerLevelIndex => "lower_level_index",
            Self::LowerZ => "lower_z",
            Self::MarchingSquaresRegions => "marching_squares_regions",
            Self::MaxAngleDeg => "max_angle_deg",
            Self::MaxFeedRate => "max_feed_rate",
            Self::MaxLinkDistance => "max_link_distance",
            Self::MoveScope => "move_scope",
            Self::NominalFeedRate => "nominal_feed_rate",
            Self::OffsetIndex => "offset_index",
            Self::OffsetMm => "offset_mm",
            Self::OffsetTotal => "offset_total",
            Self::PassIndex => "pass_index",
            Self::PerimeterSweepLengthMm => "perimeter_sweep_length_mm",
            Self::Pitch => "pitch",
            Self::Radius => "radius",
            Self::RadiusMm => "radius_mm",
            Self::RampIndex => "ramp_index",
            Self::RampRate => "ramp_rate",
            Self::RampTotal => "ramp_total",
            Self::RegionAreasMm2 => "region_areas_mm2",
            Self::RegionCount => "region_count",
            Self::RegionId => "region_id",
            Self::RegionIndex => "region_index",
            Self::RegionTotal => "region_total",
            Self::ResidualCleanupCellCount => "residual_cleanup_cell_count",
            Self::RingIndex => "ring_index",
            Self::RingTotal => "ring_total",
            Self::SafeZ => "safe_z",
            Self::SearchEvaluations => "search_evaluations",
            Self::Short => "short",
            Self::Skipped => "skipped",
            Self::StepCount => "step_count",
            Self::Strategy => "strategy",
            Self::Style => "style",
            Self::TerraceIndex => "terrace_index",
            Self::TerraceTotal => "terrace_total",
            Self::Tolerance => "tolerance",
            Self::ToolRadius => "tool_radius",
            Self::UpperLevelIndex => "upper_level_index",
            Self::UpperZ => "upper_z",
            Self::YieldRatio => "yield_ratio",
            Self::ZLevel => "z_level",
        }
    }

    /// Inverse of [`Self::as_str`], named to match
    /// [`crate::toolpath_spans::SpanKind::from_key`]. `None` means the key is
    /// not part of the
    /// vocabulary — treat that as a loud error, not a silent no-match.
    #[must_use]
    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.as_str() == key)
    }
}

impl std::fmt::Display for SemanticKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Default, PartialEq, DeriveSerialize, Deserialize)]
pub struct ToolpathSemanticParams {
    pub values: BTreeMap<String, Value>,
}

impl ToolpathSemanticParams {
    /// Insert a pre-serialised value under a typed key.
    pub fn insert_json(&mut self, key: SemanticKey, value: Value) {
        self.values.insert(key.as_str().to_owned(), value);
    }

    /// Serialise and insert under a typed key.
    ///
    /// Silently drops values that fail to serialise — a parameter is a
    /// diagnostic, never a reason to fail a generation.
    pub fn insert<T: Serialize>(&mut self, key: SemanticKey, value: T) {
        if let Ok(value) = serde_json::to_value(value) {
            self.values.insert(key.as_str().to_owned(), value);
        }
    }

    /// Read a parameter by typed key. The reader half of C4: `narrate.rs`
    /// used to carry its own copies of six key literals.
    #[must_use]
    pub fn get(&self, key: SemanticKey) -> Option<&Value> {
        self.values.get(key.as_str())
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
    /// (dressups, boundary clip, entry-descent splitting) because this
    /// channel is registered in
    /// [`crate::transform_provenance::ReconcileSet`] and no transform can
    /// hand its result back without reconciling (C1). When a transform
    /// deletes every move an item covered — or scatters them so the
    /// remapped bounds would be a lie (the reorder's foreign-intrusion
    /// drop) — the item is **UNLINKED**: `move_start` and `move_end` both
    /// become `None`.
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
    /// XY extent of the linked moves.
    ///
    /// Follows the SAME post-transform contract as [`Self::move_end`]
    /// (Wave D3): re-derived from the surviving moves when the item stays
    /// linked, `None` when it is unlinked. Coordinates never outlive the
    /// moves they describe — an item that reports a bbox is reporting the
    /// bbox of the moves it currently points at, not of the moves it pointed
    /// at when it was generated.
    pub xy_bbox: Option<ToolpathDebugBounds2>,
    /// Lowest Z of the linked moves. Same post-transform contract as
    /// [`Self::xy_bbox`].
    pub z_min: Option<f64>,
    /// Highest Z of the linked moves. Same post-transform contract as
    /// [`Self::xy_bbox`].
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

    /// Write back a batch of move links, then bring each touched item's
    /// recorded GEOMETRY back into agreement with the moves it now points at.
    ///
    /// `toolpath` is the post-transform toolpath — the one the new indices
    /// index. See [`Self::rederive_geometry_for`] for the geometry policy.
    // SAFETY: Mutex::lock only fails if poisoned (panic in another thread)
    #[allow(clippy::expect_used)]
    fn apply_links(&self, links: &[(u64, Option<(usize, usize)>)], toolpath: &Toolpath) {
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
                Self::rederive_geometry_for(item, toolpath);
            }
        }
    }

    /// Bring one item's `xy_bbox` / `z_min` / `z_max` back into agreement
    /// with its (just-remapped) move link.
    ///
    /// # The policy (task #14 follow-up, Wave D3)
    ///
    /// `ae10cb2` remapped the move INDICES through every post-generation
    /// transform but left the coordinates exactly as generation recorded
    /// them — so after a boundary clip deleted moves, an item's bbox could
    /// describe geometry that is no longer in the toolpath. Same class of
    /// defect as a stale index, same remedy, stated the same way:
    ///
    /// * **Still linked** → RE-DERIVE from the surviving moves. The moves are
    ///   right there and the derivation is the one
    ///   [`ToolpathSemanticScope::bind_to_toolpath`] used at generation time,
    ///   so this is a recomputation, not an estimate.
    /// * **Unlinked** → `None` out all three. Its moves can no longer be
    ///   identified, so neither can their extent. Never keep the old numbers:
    ///   that is the fabrication the UNLINK policy exists to prevent.
    /// * **Never had geometry** → leave it alone. An item that made no
    ///   geometric claim at generation does not acquire one here.
    fn rederive_geometry_for(item: &mut ToolpathSemanticItem, toolpath: &Toolpath) {
        let had_geometry = item.xy_bbox.is_some() || item.z_min.is_some() || item.z_max.is_some();
        if !had_geometry {
            return;
        }
        let derived = item
            .move_start
            .zip(item.move_end)
            // Stored end is INCLUSIVE; the deriver takes a half-open range.
            .and_then(|(start, end)| range_geometry(toolpath, start, end.saturating_add(1)));
        match derived {
            Some((bbox, z_min, z_max)) => {
                item.xy_bbox = bbox;
                item.z_min = Some(z_min);
                item.z_max = Some(z_max);
            }
            None => {
                item.xy_bbox = None;
                item.z_min = None;
                item.z_max = None;
            }
        }
    }

    /// Rewrite every recorded move link through one transform's provenance
    /// report — the [`crate::transform_provenance::RemapConsumer`] body for
    /// this channel, kept here because it needs the private link accessors.
    ///
    /// Replaces ae10cb2's two entry points (`remap_move_links` for
    /// transforms that handed out a mapping directly, `SemanticLinkCarrier`
    /// for transforms that only exposed their remap through the span
    /// vector). One route now, because the transform is obliged to report
    /// its provenance either way.
    ///
    /// The rules are unchanged, and they live in
    /// [`crate::transform_provenance::MoveProvenance::remap_range`]:
    /// insertion mappings clamp exactly as [`Span::remap`] does, deletions
    /// unlink, and a reorder that scattered an item's moves unlinks it via
    /// the same foreign-intrusion predicate the span filter uses.
    ///
    /// `toolpath` is the POST-transform toolpath (the one the remapped
    /// indices index). It is taken rather than a bare move count because the
    /// recorded geometry is re-derived from it — an index that moved and a
    /// bbox that did not is exactly the stale-coordinate defect Wave D3
    /// closed. See [`Self::rederive_geometry_for`].
    pub(crate) fn consume_provenance(
        &self,
        provenance: &crate::transform_provenance::MoveProvenance,
        toolpath: &Toolpath,
    ) {
        let new_move_count = toolpath.moves.len();
        let updates: Vec<(u64, Option<(usize, usize)>)> = self
            .move_links()
            .into_iter()
            .map(|(id, start, end)| {
                let link = provenance
                    .remap_range(start, end, new_move_count)
                    .and_then(|r| linked_range(r.start, r.end, new_move_count));
                (id, link)
            })
            .collect();
        self.apply_links(&updates, toolpath);
    }
}

/// The XY bounds and Z range of `toolpath.moves[start..end_exclusive)`,
/// including the *previous* move's target when there is one (that point is
/// where the range's first segment starts, so the swept extent covers it).
///
/// The single derivation shared by [`ToolpathSemanticScope::bind_to_toolpath`]
/// (generation time) and [`ToolpathSemanticRecorder::rederive_geometry_for`]
/// (after a transform) — so a re-derived bbox is the same function of the
/// same moves, and an item that survives a transform untouched keeps
/// byte-identical geometry.
///
/// `None` when the range is empty or out of bounds.
#[allow(clippy::indexing_slicing)] // bounds checked on the line above each index
fn range_geometry(
    toolpath: &Toolpath,
    start: usize,
    end_exclusive: usize,
) -> Option<(Option<ToolpathDebugBounds2>, f64, f64)> {
    if end_exclusive <= start || end_exclusive > toolpath.moves.len() {
        return None;
    }
    let moves = &toolpath.moves[start..end_exclusive];
    if moves.is_empty() {
        return None;
    }
    let mut z_min = f64::INFINITY;
    let mut z_max = f64::NEG_INFINITY;
    let mut xy_points = Vec::with_capacity(moves.len() + 1);
    if start > 0 {
        let prev = &toolpath.moves[start - 1].target;
        xy_points.push((prev.x, prev.y));
        z_min = z_min.min(prev.z);
        z_max = z_max.max(prev.z);
    }
    for mv in moves {
        xy_points.push((mv.target.x, mv.target.y));
        z_min = z_min.min(mv.target.z);
        z_max = z_max.max(mv.target.z);
    }
    Some((
        ToolpathDebugBounds2::from_points(xy_points.iter()),
        z_min,
        z_max,
    ))
}

/// Convert a half-open remapped range into the stored inclusive link,
/// applying the deleted-move policy: an empty range, or one that starts
/// past the end of the toolpath, unlinks the item; an over-long end is
/// clamped to the last move so a linked range is always sliceable.
fn linked_range(start: usize, end_exclusive: usize, n_moves: usize) -> Option<(usize, usize)> {
    if end_exclusive <= start || start >= n_moves {
        return None;
    }
    Some((start, end_exclusive.min(n_moves) - 1))
}

impl ToolpathSemanticContext {
    /// The recorder this context writes into — the handle a call site needs
    /// to register this channel in a
    /// [`crate::transform_provenance::ReconcileSet`] while it is being
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

    /// Attach a parameter to the open item under a typed key (C4 — see
    /// [`SemanticKey`]; the key vocabulary is closed, the value is not).
    pub fn set_param<T: Serialize>(&self, key: SemanticKey, value: T) {
        self.update_item(|item| item.params.insert(key, value));
    }

    /// [`Self::set_param`] with a pre-serialised value.
    pub fn set_param_json(&self, key: SemanticKey, value: Value) {
        self.update_item(|item| item.params.insert_json(key, value));
    }

    pub fn set_debug_span_id(&self, debug_span_id: u64) {
        self.update_item(|item| item.debug_span_id = Some(debug_span_id));
    }

    /// Link this item to `toolpath.moves[move_start..move_end_exclusive)` and
    /// record that range's geometry.
    ///
    /// The geometry is derived by [`range_geometry`], the SAME function the
    /// post-transform re-derivation uses — so a bbox recomputed after a clip
    /// is comparable with the one recorded here, and an untouched item keeps
    /// byte-identical numbers.
    pub fn bind_to_toolpath(
        &self,
        toolpath: &Toolpath,
        move_start: usize,
        move_end_exclusive: usize,
    ) {
        let Some((bounds, z_min, z_max)) = range_geometry(toolpath, move_start, move_end_exclusive)
        else {
            return;
        };
        self.set_move_range(move_start, move_end_exclusive - 1);
        if let Some(bounds) = bounds {
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
    use crate::toolpath_spans::AnnotatedToolpath;
    use crate::transform_provenance::{MoveProvenance, ReconcileSet, Transformed};

    /// C4 wire sentry. The typed key layer must be invisible on the wire:
    /// `as_str` is the only place a key string exists, and this pins the
    /// whole vocabulary as literal text so a variant rename cannot quietly
    /// move the JSON an agent or a script reads.
    ///
    /// The list is transcribed by hand ON PURPOSE. Deriving it from
    /// `SemanticKey::as_str` would make the test tautological — it would
    /// assert the enum equals itself. A key that changes here is a change to
    /// a published vocabulary and should cost one deliberate edit.
    #[test]
    fn the_semantic_key_wire_vocabulary_is_pinned() {
        const WIRE: [&str; 74] = [
            "agent_walk_cut_length_mm",
            "angle_deg",
            "area_mm2",
            "band",
            "barrier_count",
            "cell_count",
            "center_x",
            "center_y",
            "chain_index",
            "chain_total",
            "containment",
            "continuous",
            "contour_index",
            "contour_total",
            "cycle_index",
            "dropped_micro_region_count",
            "entry_x",
            "entry_y",
            "entry_z",
            "exit_reason",
            "hole_index",
            "idle_count",
            "is_centerline",
            "keep_out_count",
            "kind",
            "lead_in_feed_rate",
            "lead_out_feed_rate",
            "level_index",
            "level_total",
            "line_index",
            "line_total",
            "link_feed_rate",
            "lower_level_index",
            "lower_z",
            "marching_squares_regions",
            "max_angle_deg",
            "max_feed_rate",
            "max_link_distance",
            "move_scope",
            "nominal_feed_rate",
            "offset_index",
            "offset_mm",
            "offset_total",
            "pass_index",
            "perimeter_sweep_length_mm",
            "pitch",
            "radius",
            "radius_mm",
            "ramp_index",
            "ramp_rate",
            "ramp_total",
            "region_areas_mm2",
            "region_count",
            "region_id",
            "region_index",
            "region_total",
            "residual_cleanup_cell_count",
            "ring_index",
            "ring_total",
            "safe_z",
            "search_evaluations",
            "short",
            "skipped",
            "step_count",
            "strategy",
            "style",
            "terrace_index",
            "terrace_total",
            "tolerance",
            "tool_radius",
            "upper_level_index",
            "upper_z",
            "yield_ratio",
            "z_level",
        ];
        let actual: Vec<&str> = SemanticKey::ALL.iter().map(|k| k.as_str()).collect();
        assert_eq!(actual.as_slice(), WIRE.as_slice());

        // Every key round-trips, and the keys are distinct — a copy-pasted
        // arm in `as_str` would otherwise make one variant unreachable by
        // name while silently colliding on the wire.
        for key in SemanticKey::ALL {
            assert_eq!(SemanticKey::from_key(key.as_str()), Some(key));
        }
        let mut sorted = actual.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), SemanticKey::ALL.len());
        assert_eq!(SemanticKey::from_key("not_a_key"), None);
    }

    /// The typed setters must write exactly the string the untyped ones did.
    #[test]
    fn typed_keys_serialise_as_their_wire_strings() {
        let mut params = ToolpathSemanticParams::default();
        params.insert(SemanticKey::ZLevel, -1.5);
        params.insert(SemanticKey::MarchingSquaresRegions, 3usize);
        params.insert_json(SemanticKey::Band, Value::String("MidSteep".to_owned()));
        let json = serde_json::to_string(&params).expect("serialize params");
        assert!(json.contains("\"z_level\":-1.5"), "{json}");
        assert!(json.contains("\"marching_squares_regions\":3"), "{json}");
        assert!(json.contains("\"band\":\"MidSteep\""), "{json}");
        // …and read back through the typed accessor.
        assert_eq!(
            params.get(SemanticKey::ZLevel).and_then(Value::as_f64),
            Some(-1.5)
        );
        assert_eq!(params.get(SemanticKey::Pitch), None);
    }

    #[test]
    fn semantic_recorder_serializes_items() {
        let recorder = ToolpathSemanticRecorder::new("Pocket 1", "Pocket");
        let ctx = recorder.root_context();
        let item = ctx.start_item(ToolpathSemanticKind::DepthLevel, "Level -1.0");
        item.set_param(SemanticKey::ZLevel, -1.0);
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
    /// transform dropped is UNLINKED (not clamped, not deleted).
    ///
    /// C1: was `carrier_remaps_survivors_and_unlinks_deleted_items`, driven
    /// through `SemanticLinkCarrier::attach`/`detach`. Same remap, same
    /// assertions on the outcome; the carrier's own bookkeeping assertions
    /// (one carrier span per distinct range, all of them stripped again)
    /// are gone because there are no carrier spans to count or strip — the
    /// transform reports its provenance instead of smuggling the link
    /// through the span vector. What replaces them is the assertion below
    /// that the span vector is untouched by the reconcile.
    #[test]
    fn provenance_remaps_survivors_and_unlinks_deleted_items() {
        use crate::toolpath_spans::MoveRemap;

        let recorder = ToolpathSemanticRecorder::new("Pocket 1", "Pocket");
        let ctx = recorder.root_context();
        let keep = ctx.start_item(ToolpathSemanticKind::Pass, "Keep");
        let lose = ctx.start_item(ToolpathSemanticKind::Pass, "Lose");
        keep.set_move_range(0, 2); // inclusive → half-open 0..3
        lose.set_move_range(3, 5); // inclusive → half-open 3..6

        // Transform: moves 0..3 shift up by one (something was inserted in
        // front of them), moves 3..6 are deleted outright.
        let remap = MoveRemap {
            old_to_new: vec![Some(1..2), Some(2..3), Some(3..4), None, None, None],
        };
        let transformed = Transformed::new(
            AnnotatedToolpath::new(toolpath_with_moves(4)),
            MoveProvenance::Remap(remap),
        );
        let shipped = transformed
            .reconcile(&mut ReconcileSet::new(Some(&recorder)))
            .into_inner();

        assert!(
            shipped.spans.is_empty(),
            "reconciling a channel must not add spans to the shipped toolpath"
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
        assert_eq!(
            (keep_item.move_start, keep_item.move_end),
            (Some(1), Some(3))
        );
        let lose_item = by_label("Lose");
        assert_eq!(
            (lose_item.move_start, lose_item.move_end),
            (None, None),
            "an item whose moves were all deleted is unlinked, not clamped"
        );
        assert_eq!(trace.summary.item_count, 2, "the item itself survives");
        assert_eq!(trace.summary.move_linked_item_count, 1);
    }

    /// Wave D3 — the coordinates follow the indices.
    ///
    /// `ae10cb2` remapped move INDICES through every transform and
    /// deliberately left `xy_bbox` / `z_min` / `z_max` at their
    /// generation-time values. That made a surviving item describe an extent
    /// that no longer existed: here the clip deletes the far half of a
    /// 6-move path, and pre-fix the "Kept" item still reported
    /// `max_x = 5, z_min = -6` — the bbox of moves that are gone.
    ///
    /// The RED, recorded: with the re-derivation removed this asserts
    /// `max_x` 2.0 and reads 5.0, and `z_min` -3.0 and reads -6.0.
    #[test]
    fn clip_re_derives_geometry_it_cannot_leave_describing_deleted_moves() {
        use crate::toolpath_spans::MoveRemap;

        // A staircase: move i sits at x = i, z = -(i + 1), so the XY bbox
        // and the Z range both grow monotonically along the path — a
        // deleted tail is visible in BOTH.
        let staircase = |n: usize| {
            let mut tp = Toolpath::new();
            for i in 0..n {
                tp.feed_to(P3::new(i as f64, 0.0, -(i as f64 + 1.0)), 100.0);
            }
            tp
        };
        let full = staircase(6);

        let recorder = ToolpathSemanticRecorder::new("Pocket 1", "Pocket");
        let ctx = recorder.root_context();
        let kept = ctx.start_item(ToolpathSemanticKind::Pass, "Kept");
        // Generation-time bind over the WHOLE path: x 0..5, z -6..-1.
        kept.bind_to_toolpath(&full, 0, full.moves.len());
        let gone = ctx.start_item(ToolpathSemanticKind::Pass, "Gone");
        gone.bind_to_toolpath(&full, 3, 6);

        // The clip keeps moves 0..3 and deletes 3..6.
        let remap = MoveRemap {
            old_to_new: vec![Some(0..1), Some(1..2), Some(2..3), None, None, None],
        };
        let _shipped = Transformed::new(
            AnnotatedToolpath::new(staircase(3)),
            MoveProvenance::Remap(remap),
        )
        .reconcile(&mut ReconcileSet::new(Some(&recorder)))
        .into_inner();

        let trace = recorder.finish();
        let by_label = |label: &str| {
            trace
                .items
                .iter()
                .find(|i| i.label == label)
                .expect("item recorded")
                .clone()
        };

        let kept_item = by_label("Kept");
        assert_eq!(
            (kept_item.move_start, kept_item.move_end),
            (Some(0), Some(2))
        );
        let bbox = kept_item.xy_bbox.expect("a linked item keeps its bbox");
        assert!(
            (bbox.max_x - 2.0).abs() < 1e-12,
            "bbox must describe the SURVIVING moves (x max 2.0), got {}",
            bbox.max_x
        );
        assert!((bbox.min_x - 0.0).abs() < 1e-12);
        assert!(
            kept_item.z_min.is_some_and(|z| (z + 3.0).abs() < 1e-12),
            "z_min must describe the surviving moves (-3.0), got {:?}",
            kept_item.z_min
        );
        assert!(kept_item.z_max.is_some_and(|z| (z + 1.0).abs() < 1e-12));

        // An UNLINKED item drops its coordinates too — keeping them would be
        // the same fabrication the unlink policy exists to prevent.
        let gone_item = by_label("Gone");
        assert_eq!((gone_item.move_start, gone_item.move_end), (None, None));
        assert_eq!(gone_item.xy_bbox, None, "unlinked ⇒ no extent");
        assert_eq!(gone_item.z_min, None);
        assert_eq!(gone_item.z_max, None);
    }

    /// A transform that changes nothing must leave the geometry
    /// byte-identical — re-derivation is a recomputation of the same
    /// function over the same moves, not a second opinion.
    #[test]
    fn identity_remap_leaves_geometry_byte_identical() {
        let tp = {
            let mut tp = Toolpath::new();
            tp.rapid_to(P3::new(0.0, 0.0, 5.0));
            tp.feed_to(P3::new(0.0, 0.0, -1.0), 100.0);
            tp.feed_to(P3::new(10.0, 4.0, -1.0), 200.0);
            tp
        };
        let recorder = ToolpathSemanticRecorder::new("Pocket 1", "Pocket");
        let ctx = recorder.root_context();
        let item = ctx.start_item(ToolpathSemanticKind::Pass, "Pass");
        item.bind_to_toolpath(&tp, 0, tp.moves.len());
        let before = recorder.clone().finish().items[0].clone();

        recorder.consume_provenance(&MoveProvenance::Mapping(vec![0, 1, 2, 3]), &tp);

        let after = recorder.finish().items[0].clone();
        assert_eq!(before.xy_bbox, after.xy_bbox);
        assert_eq!(before.z_min, after.z_min);
        assert_eq!(before.z_max, after.z_max);
    }

    /// The provenance-map form (boundary clip, entry-descent splitter): an
    /// item covering the whole toolpath must still cover the whole toolpath
    /// after the transform inserted moves inside its range.
    #[test]
    fn mapping_provenance_follows_inserted_moves() {
        let recorder = ToolpathSemanticRecorder::new("Pocket 1", "Pocket");
        let ctx = recorder.root_context();
        let op = ctx.start_item(ToolpathSemanticKind::Operation, "Pocket");
        op.set_move_range(0, 2); // covers all 3 input moves

        // Each input move produced two output moves: mapping[i] = 2i, with
        // the total-count sentinel last.
        recorder.consume_provenance(
            &MoveProvenance::Mapping(vec![0, 2, 4, 6]),
            &toolpath_with_moves(6),
        );

        let trace = recorder.finish();
        let item = &trace.items[0];
        assert_eq!((item.move_start, item.move_end), (Some(0), Some(5)));
    }

    /// A stale link (transform declined to remap because spans were already
    /// invalid) must never come back out of bounds.
    #[test]
    fn mapping_provenance_never_returns_an_out_of_bounds_link() {
        let recorder = ToolpathSemanticRecorder::new("Pocket 1", "Pocket");
        let ctx = recorder.root_context();
        let inside = ctx.start_item(ToolpathSemanticKind::Pass, "Overhang");
        inside.set_move_range(0, 9);
        let outside = ctx.start_item(ToolpathSemanticKind::Pass, "Past the end");
        outside.set_move_range(8, 9);

        // Identity mapping over 10 input moves, but only 4 moves survive.
        recorder.consume_provenance(
            &MoveProvenance::Mapping(vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10]),
            &toolpath_with_moves(4),
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
