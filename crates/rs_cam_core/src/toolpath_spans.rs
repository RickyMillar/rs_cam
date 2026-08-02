//! Toolpath spans — semantic ranges of moves inside a Toolpath.
//!
//! See `architecture/toolpath_spans.md` for design background. This module
//! contributes only the types; wiring into `compute::execute` and dressups
//! arrives in subsequent phases.

use std::borrow::Cow;
use std::ops::Range;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::geo::P3;
use crate::polygon::Polygon2;
use crate::rest_field::RestGrid;
use crate::toolpath::Toolpath;

// ── SpanId ──────────────────────────────────────────────────────────────

/// Stable identifier for a span within one [`AnnotatedToolpath`] — the index
/// into [`AnnotatedToolpath::spans`].
///
/// SpanIds are stable for the lifetime of one toolpath generation but are NOT
/// stable across regeneration (the spans vec is rebuilt each time). Persist
/// them only inside artifacts that share the toolpath's lifetime
/// (e.g. `SimulationCutTrace`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SpanId(pub u32);

// ── Span ────────────────────────────────────────────────────────────────

/// A range of moves `[start_move..end_move)` within a [`Toolpath`], tagged
/// with semantic info.
///
/// Half-open ranges match Rust slice conventions and avoid off-by-one
/// ambiguity when transforms insert or delete moves. A zero-width span
/// (`start_move == end_move`) is a boundary: it sits *before* the move at
/// `start_move` (or *after* the last move when `start_move == toolpath.moves.len()`).
#[derive(Debug, Clone, PartialEq)]
pub struct Span {
    pub start_move: usize,
    pub end_move: usize,
    pub kind: SpanKind,
    pub label: Cow<'static, str>,
    pub payload: Option<SpanPayload>,
}

impl Span {
    /// Build a span with no label or payload.
    pub const fn new(start_move: usize, end_move: usize, kind: SpanKind) -> Self {
        Self {
            start_move,
            end_move,
            kind,
            label: Cow::Borrowed(""),
            payload: None,
        }
    }

    /// Build a zero-width boundary span at `move_idx`.
    pub const fn boundary(move_idx: usize, kind: SpanKind) -> Self {
        Self::new(move_idx, move_idx, kind)
    }

    pub fn with_label(mut self, label: impl Into<Cow<'static, str>>) -> Self {
        self.label = label.into();
        self
    }

    pub fn with_payload(mut self, payload: SpanPayload) -> Self {
        self.payload = Some(payload);
        self
    }

    /// This span's [`RegionSpanRole`], or `None` when it is not a
    /// region span (or carries no payload).
    ///
    /// The supported way to tell a planner region NODE from a generator
    /// pass — never parse the label (Wave D3).
    pub fn region_role(&self) -> Option<RegionSpanRole> {
        match self.payload {
            Some(SpanPayload::Region { role, .. }) => Some(role),
            _ => None,
        }
    }

    /// True when this is a planner territory node span
    /// ([`RegionSpanRole::Node`]).
    pub fn is_region_node(&self) -> bool {
        self.region_role() == Some(RegionSpanRole::Node)
    }

    /// True when this span carries `role` and is not a zero-width boundary
    /// marker — the shape every role query in the codebase wants.
    ///
    /// C4: the supported replacement for `label.starts_with("Hole ") &&
    /// !label.contains("plunge")` and friends.
    pub fn has_region_role(&self, role: RegionSpanRole) -> bool {
        !self.is_boundary() && self.kind == SpanKind::Region && self.region_role() == Some(role)
    }

    /// Number of moves covered. 0 for boundary spans.
    pub fn move_count(&self) -> usize {
        self.end_move.saturating_sub(self.start_move)
    }

    /// True if this span is zero-width.
    pub const fn is_boundary(&self) -> bool {
        self.start_move == self.end_move
    }

    /// True if `move_idx` falls in `[start_move, end_move)`. Always false for
    /// boundary spans — use [`Span::is_boundary_at`] to test those.
    pub const fn contains(&self, move_idx: usize) -> bool {
        move_idx >= self.start_move && move_idx < self.end_move
    }

    /// True if this is a zero-width boundary span sitting at `move_idx`.
    pub const fn is_boundary_at(&self, move_idx: usize) -> bool {
        self.is_boundary() && self.start_move == move_idx
    }

    /// Half-open `Range<usize>` view of this span's move indices.
    pub const fn range(&self) -> Range<usize> {
        self.start_move..self.end_move
    }

    /// Remap this span's move indices through a per-input-move provenance map
    /// produced by a span-preserving toolpath transform (e.g.
    /// [`crate::boundary::clip_toolpath_to_boundary_with_provenance`]).
    ///
    /// `mapping` must have length `original_move_count + 1` with
    /// `mapping[i]` = first output index produced from input move `i` and
    /// `mapping[mapping.len() - 1]` = total output move count (sentinel).
    /// Out-of-range start/end indices are clamped to the last entry so a
    /// malformed mapping degrades to a zero-width span at the end rather
    /// than panicking.
    pub fn remap(&self, mapping: &[usize]) -> Span {
        let last = mapping.last().copied().unwrap_or(0);
        let new_start = mapping.get(self.start_move).copied().unwrap_or(last);
        let new_end = mapping.get(self.end_move).copied().unwrap_or(last);
        Span {
            start_move: new_start,
            end_move: new_end,
            kind: self.kind,
            label: self.label.clone(),
            payload: self.payload.clone(),
        }
    }
}

// ── SpanKind ────────────────────────────────────────────────────────────

/// Categorical type for what a span represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpanKind {
    /// Wraps every move of one operation. Always present at the top level.
    Operation,
    /// Moves at a single Z depth in a multi-pass operation.
    DepthPass,
    /// A region (closed-polygon area or chain) within an op.
    Region,
    /// An entry / lead-in transition (rapid + plunge or ramp/helix).
    Entry,
    /// A lead-out transition.
    LeadOut,
    /// A linker bridge inserted by `apply_link_moves`.
    LinkBridge,
    /// A dressup-introduced segment (dogbone, arc-fit replacement).
    DressupArtifact,
    /// Hard barrier *before* `start_move`. TSP must not reorder across this
    /// move boundary. Always zero-width: `start_move == end_move`.
    RapidOrderBarrier,
    /// A waterline / boundary cleanup pass added after the main Z-level
    /// passes of an adaptive3d operation. These spans carry transient
    /// link-and-clean moves that bridge across regions previously cleared
    /// (or partially cleared) by the depth-pass loop. Lateral feeds inside
    /// these spans can briefly engage uncleared material left between
    /// passes — the simulator faithfully reports the engagement, but it's
    /// not a steady-state cutting condition the operator can dial in via
    /// feeds & speeds. Gate predicates filter samples with a
    /// `WaterlineCleanup` ancestor the same way they filter
    /// [`SpanKind::Entry`] transients (Roadmap F.3).
    WaterlineCleanup,
}

impl SpanKind {
    /// Every variant, in declaration order.
    ///
    /// A new variant MUST be added here as well as to [`Self::as_key`] —
    /// `as_key`'s match is exhaustive, so the compiler will stop you there
    /// first, and [`Self::from_key`] is derived from this list so that the
    /// agent-facing vocabulary cannot drift from the enum.
    pub const ALL: [Self; 9] = [
        Self::Operation,
        Self::DepthPass,
        Self::Region,
        Self::Entry,
        Self::LeadOut,
        Self::LinkBridge,
        Self::DressupArtifact,
        Self::RapidOrderBarrier,
        Self::WaterlineCleanup,
    ];

    /// The stable snake_case key for this kind — the agent-facing vocabulary
    /// (MCP `span_kind` filters) and the single source that vocabulary is
    /// derived from.
    ///
    /// Wave D3: this used to be a string table transcribed by hand in
    /// `rs_cam_viz::app::mcp` (three times), so a new variant was invisible
    /// to the MCP surface with no compile error. The match here is
    /// exhaustive: adding a variant now breaks the build until it is named.
    #[must_use]
    pub const fn as_key(self) -> &'static str {
        match self {
            Self::Operation => "operation",
            Self::DepthPass => "depth_pass",
            Self::Region => "region",
            Self::Entry => "entry",
            Self::LeadOut => "lead_out",
            Self::LinkBridge => "link_bridge",
            Self::DressupArtifact => "dressup_artifact",
            Self::RapidOrderBarrier => "rapid_order_barrier",
            Self::WaterlineCleanup => "waterline_cleanup",
        }
    }

    /// Inverse of [`Self::as_key`]. `None` for anything that is not a
    /// structural span kind — callers should treat that as a loud error, not
    /// as a silent no-match.
    #[must_use]
    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.as_key() == key)
    }
}

// ── SpanPayload ─────────────────────────────────────────────────────────

/// Optional structured payload for span-specific data.
///
/// Most spans don't need a payload. Variants are added as concrete consumers
/// require them — keep this set minimal until phase-3 dressups demand more.
#[derive(Debug, Clone, PartialEq)]
pub enum SpanPayload {
    DepthPass {
        z_level: f64,
        pass_index: u32,
    },
    /// A [`SpanKind::Region`] span. `role` says what kind of region it is —
    /// see [`RegionSpanRole`], which exists because `region_id` alone is
    /// ambiguous: a planner NODE and a generator RING both land here with
    /// their own independent id spaces.
    Region {
        region_id: u32,
        role: RegionSpanRole,
    },
}

/// What a [`SpanKind::Region`] span is a region OF.
///
/// Wave D3. [`SpanKind::Region`] is emitted by two structurally different
/// systems that share the kind AND the payload:
///
/// * the PLANNER, which partitions the part into territory nodes
///   (`UnifiedFinishReport::region_table` — one node per band / crease
///   group, each with its own strategy), and
/// * the GENERATOR, which reports the passes it laid down inside whatever
///   territory it was given (scallop rings, drill holes and their pecks,
///   adaptive regions, contiguous cutting runs).
///
/// Their `region_id`s index different tables, they nest (a node contains
/// many passes), and only the node set is a partition. Before this role
/// existed the only discriminator was the LABEL STRING — consumers filtered
/// on `label.ends_with(" band")`, which is a contract no producer was
/// obliged to keep. Read this instead.
///
/// C4 (2026-08-02) closed the second instance of the same class. Drill spans
/// nest one more level — a hole contains its pecks — and BOTH levels were
/// `GeneratorPass`, so the only discriminator was again the label:
/// `label.starts_with("Hole ") && !label.contains("plunge")` for the parent,
/// `label.contains("plunge")` for the child. Two variants were added rather
/// than the one the backlog asked for, because the parent side was exactly
/// as label-bound as the child side and naming only the child would have
/// left "a hole is a `GeneratorPass` in a drill operation" as an unwritten
/// rule a generic consumer cannot apply.
///
/// **Any future mix table, routing decision or report grouping must be built
/// on this role (or on a [`SpanKind`]), never on `label`.** `label` is
/// free text for humans; nothing downstream may depend on its shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RegionSpanRole {
    /// A planner territory node. `region_id` indexes the planner's own
    /// region table (e.g. `UnifiedFinishReport::region_table`). The node set
    /// tiles the operation's moves; a semantic `Region` item exists per node
    /// and their ranges must agree (A/M8).
    Node,
    /// One pass the generator emitted — a scallop ring, an adaptive region,
    /// a contiguous cutting run. `region_id` is the generator's own sequence
    /// number, NOT a node id. These nest inside nodes and are not a
    /// partition of anything.
    GeneratorPass,
    /// One drilled hole, spanning every peck in its cycle. `region_id` is
    /// the hole index. Its [`Self::DrillPeck`] children are nested inside its
    /// move range.
    DrillHole,
    /// One peck of one drilled hole. `region_id` continues the hole id space
    /// past the last hole (`region_id >= hole_count`), so hole and peck ids
    /// never collide even though they index different sequences.
    DrillPeck,
}

impl RegionSpanRole {
    /// Every variant, in declaration order. Same contract as
    /// [`SpanKind::ALL`]: [`Self::label`]'s match is exhaustive so the
    /// compiler stops a new variant here first, and [`Self::from_key`] is
    /// derived from this list so the agent-facing vocabulary cannot drift
    /// from the enum.
    pub const ALL: [Self; 4] = [
        Self::Node,
        Self::GeneratorPass,
        Self::DrillHole,
        Self::DrillPeck,
    ];

    /// The stable snake_case key for this role — what MCP `region_role`
    /// carries, and the single source that vocabulary is derived from.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Node => "node",
            Self::GeneratorPass => "generator_pass",
            Self::DrillHole => "drill_hole",
            Self::DrillPeck => "drill_peck",
        }
    }

    /// Inverse of [`Self::label`]. `None` for anything that is not a region
    /// role — callers should treat that as a loud error, not a silent
    /// no-match.
    #[must_use]
    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|role| role.label() == key)
    }

    /// True for the roles a GENERATOR emits, as opposed to the planner's
    /// territory [`Self::Node`]s. Drill holes and pecks are generator output
    /// too; this is the predicate that used to be `== GeneratorPass`.
    #[must_use]
    pub const fn is_generator_pass(self) -> bool {
        matches!(
            self,
            Self::GeneratorPass | Self::DrillHole | Self::DrillPeck
        )
    }
}

// ── AnnotatedToolpath ───────────────────────────────────────────────────

/// Toolpath bundled with optional semantic spans.
///
/// `spans_valid` lets transforms that cannot easily remap (e.g. boundary
/// clipping) say "I invalidated these" without losing the toolpath.
/// Downstream code that reads spans MUST honor this flag.
#[derive(Debug, Clone)]
pub struct AnnotatedToolpath {
    pub toolpath: Toolpath,
    pub spans: Vec<Span>,
    pub spans_valid: bool,
    /// Stage 4 — planner-predicted leading-arc engagement `(cut_point,
    /// α/2π)` for adaptive ContourSpiral toolpaths; empty otherwise. The
    /// feed modulator looks these up by `Move.target` position (the
    /// samples are positional, so they survive simplify / arcfit / dressup
    /// / TSP-reorder reshaping without per-move re-indexing). See
    /// `planning/ADAPTIVE_CLEARING_ALGO_REVIEW_2026-06-12.md` §"Stage 4".
    ///
    /// Frame contract: emission-frame coordinates; must be re-framed
    /// anywhere `toolpath.moves` are re-framed — see [`Self::translated`].
    pub planner_engagement: Vec<(P3, f64)>,
    /// Rest-depth heatmap grid for the GUI overlay — populated by the
    /// pencil RestDepth detector, the generic post-generation rest
    /// analysis, and UnifiedFinish's claims detector (v3 S1 §2.4
    /// carry-through); `None` otherwise. `Arc` so cloning the annotated
    /// toolpath (sim / result caching) stays cheap.
    ///
    /// Frame contract: emission-frame coordinates; must be re-framed
    /// anywhere `toolpath.moves` are re-framed — see [`Self::translated`].
    pub rest_grid: Option<Arc<RestGrid>>,
    /// Machining-region polygons derived from the rest-depth field
    /// ([`crate::rest_field::RestFieldResult::region_polygons`]) — same
    /// producers as [`Self::rest_grid`]; `None` otherwise. World-frame XY
    /// polygons; `Arc` so cloning the annotated toolpath (sim / result
    /// caching) stays cheap. This is the derived-boundary source for
    /// selective finishing (P2.2 `BoundarySource::DerivedRestRegions`).
    ///
    /// Frame contract: emission-frame coordinates; must be re-framed
    /// anywhere `toolpath.moves` are re-framed — see [`Self::translated`].
    pub rest_regions: Option<Arc<Vec<Polygon2>>>,
}

impl AnnotatedToolpath {
    /// Wrap a toolpath with no spans (still considered valid — there's just
    /// nothing to invalidate).
    pub fn new(toolpath: Toolpath) -> Self {
        Self {
            toolpath,
            spans: Vec::new(),
            spans_valid: true,
            planner_engagement: Vec::new(),
            rest_grid: None,
            rest_regions: None,
        }
    }

    pub fn with_spans(toolpath: Toolpath, spans: Vec<Span>) -> Self {
        Self {
            toolpath,
            spans,
            spans_valid: true,
            planner_engagement: Vec::new(),
            rest_grid: None,
            rest_regions: None,
        }
    }

    /// Re-frame every coordinate-bearing field by `shift` — the single place
    /// that knows how to move an `AnnotatedToolpath` between the emission
    /// frame and a display frame (or any other frame shift).
    ///
    /// Destructures `self` field-by-field on purpose: adding a new
    /// coordinate-bearing field to `AnnotatedToolpath` makes this match
    /// fail to compile until the author decides how that field transforms,
    /// rather than silently leaving it stale (the bug this method fixes —
    /// `planner_engagement` and `rest_grid` used to be left in the old
    /// frame by callers that only shifted `toolpath.moves`).
    ///
    /// - `toolpath.moves` targets are shifted; arc center offsets (`i`/`j`
    ///   on `MoveType::ArcCW`/`ArcCCW`) are relative-to-start and are left
    ///   untouched.
    /// - `planner_engagement` cut points are shifted; the stored angle
    ///   fraction is untouched.
    /// - `rest_grid` (if present) has `origin_x`/`origin_y` shifted and
    ///   every `surface_z` cell shifted by `shift.z`; `NaN` cells stay
    ///   `NaN` since `NaN + x == NaN`. `rest` values are untouched — rest
    ///   depth is a scalar, not a coordinate.
    /// - `rest_regions` (if present): every exterior/hole point of every
    ///   polygon is shifted by `shift.x`/`shift.y` (XY-only — polygons carry
    ///   no Z).
    /// - `spans` / `spans_valid` are move-index-based, not coordinate-based,
    ///   and are copied unchanged.
    pub fn translated(&self, shift: P3) -> Self {
        let Self {
            toolpath,
            spans,
            spans_valid,
            planner_engagement,
            rest_grid,
            rest_regions,
        } = self;

        let mut toolpath = toolpath.clone();
        for m in &mut toolpath.moves {
            m.target.x += shift.x;
            m.target.y += shift.y;
            m.target.z += shift.z;
        }

        let planner_engagement = planner_engagement
            .iter()
            .map(|(p, alpha)| (P3::new(p.x + shift.x, p.y + shift.y, p.z + shift.z), *alpha))
            .collect();

        let rest_grid = rest_grid.clone().map(|mut grid| {
            let g = Arc::make_mut(&mut grid);
            g.origin_x += shift.x;
            g.origin_y += shift.y;
            for z in &mut g.surface_z {
                *z += shift.z as f32;
            }
            grid
        });

        let rest_regions = rest_regions.clone().map(|mut regions| {
            let polys = Arc::make_mut(&mut regions);
            for poly in polys.iter_mut() {
                for p in &mut poly.exterior {
                    p.x += shift.x;
                    p.y += shift.y;
                }
                for hole in &mut poly.holes {
                    for p in hole.iter_mut() {
                        p.x += shift.x;
                        p.y += shift.y;
                    }
                }
            }
            regions
        });

        Self {
            toolpath,
            spans: spans.clone(),
            spans_valid: *spans_valid,
            planner_engagement,
            rest_grid,
            rest_regions,
        }
    }

    /// All spans (regardless of kind) covering this move index. Boundary
    /// spans are not included — query them with [`Self::boundaries_at`].
    pub fn spans_at(&self, move_idx: usize) -> impl Iterator<Item = &Span> {
        self.spans.iter().filter(move |s| s.contains(move_idx))
    }

    /// All zero-width boundary spans that sit at `move_idx`.
    pub fn boundaries_at(&self, move_idx: usize) -> impl Iterator<Item = &Span> {
        self.spans
            .iter()
            .filter(move |s| s.is_boundary_at(move_idx))
    }

    /// All spans of the given kind, in vector order.
    pub fn spans_of_kind(&self, kind: SpanKind) -> impl Iterator<Item = &Span> {
        self.spans.iter().filter(move |s| s.kind == kind)
    }

    /// Indices of all non-boundary spans whose move range covers `move_idx`.
    ///
    /// Returned in span-vec order, which by convention places outer ancestors
    /// (Operation) before inner descendants (DepthPass, Region, …) when
    /// generators emit spans outermost-first.
    pub fn span_path_at(&self, move_idx: usize) -> Vec<SpanId> {
        self.spans
            .iter()
            .enumerate()
            .filter_map(|(i, s)| {
                if !s.is_boundary() && s.contains(move_idx) {
                    Some(SpanId(i as u32))
                } else {
                    None
                }
            })
            .collect()
    }

    /// Per-move bitmap of "sits inside a transit-style span".
    ///
    /// Transit-style spans are non-steady-state cutting contexts:
    /// `Entry`, `LeadOut`, `LinkBridge`, `WaterlineCleanup`, `DressupArtifact`.
    /// Used by the simulator's metrics path to suppress extreme-value
    /// metrics (peak axial DOC, peak chipload) for samples whose dexel
    /// reading reflects "stock height we're flying over" rather than
    /// engagement (the lift-bridge artifact pattern documented in
    /// `planning/P3_TRANSIT_PEAK_DOC_RCA.md`).
    pub fn transit_moves_bitmap(&self) -> Vec<bool> {
        let n = self.toolpath.moves.len();
        let mut out = vec![false; n];
        for span in &self.spans {
            if span.is_boundary() {
                continue;
            }
            let is_transit = matches!(
                span.kind,
                SpanKind::Entry
                    | SpanKind::LeadOut
                    | SpanKind::LinkBridge
                    | SpanKind::WaterlineCleanup
                    | SpanKind::DressupArtifact
            );
            if !is_transit {
                continue;
            }
            let end = span.end_move.min(n);
            for entry in out.iter_mut().take(end).skip(span.start_move) {
                *entry = true;
            }
        }
        out
    }

    /// F2 (defect class C3) — transit bitmap rebuilt from per-move
    /// [`crate::toolpath::MoveIntent`] tags instead of spans.
    ///
    /// Intents travel WITH their moves through reordering transforms
    /// (TSP), so this stays correct even when `spans_valid == false`
    /// and [`Self::transit_moves_bitmap`] would read fragmented span
    /// ranges. Used as the metrics-stamping fallback for invalidated
    /// spans. Coverage caveat: dressup-inserted moves
    /// (`DressupArtifact` spans) usually carry `Unknown` intent and
    /// are NOT marked transit by this fallback.
    pub fn transit_moves_bitmap_from_intents(&self) -> Vec<bool> {
        use crate::toolpath::MoveIntent as I;
        self.toolpath
            .moves
            .iter()
            .map(|mv| {
                matches!(
                    mv.intent,
                    I::Linking
                        | I::EntryPlunge
                        | I::EntryHelix
                        | I::EntryRamp
                        | I::LeadIn
                        | I::LeadOut
                )
            })
            .collect()
    }

    /// Precompute [`Self::span_path_at`] for every move index in the toolpath.
    /// Cheaper than calling `span_path_at` per move when stamping ~100K
    /// simulation samples.
    pub fn span_paths_by_move(&self) -> Vec<Vec<SpanId>> {
        let n = self.toolpath.moves.len();
        let mut out = vec![Vec::<SpanId>::new(); n];
        for (i, span) in self.spans.iter().enumerate() {
            if span.is_boundary() {
                continue;
            }
            let id = SpanId(i as u32);
            let end = span.end_move.min(n);
            for entry in out.iter_mut().take(end).skip(span.start_move) {
                entry.push(id);
            }
        }
        out
    }

    /// Move-boundary indices that act as TSP barriers — the union of
    /// [`SpanKind::RapidOrderBarrier`] span starts and [`SpanKind::DepthPass`]
    /// span starts. Returned sorted and de-duplicated.
    ///
    /// Index `0` means "before the first move"; index `toolpath.moves.len()`
    /// means "after the last move".
    pub fn rapid_order_barriers(&self) -> Vec<usize> {
        let mut out: Vec<usize> = self
            .spans
            .iter()
            .filter_map(|s| match s.kind {
                SpanKind::RapidOrderBarrier | SpanKind::DepthPass => Some(s.start_move),
                _ => None,
            })
            .collect();
        out.sort_unstable();
        out.dedup();
        out
    }

    /// Validate that all span ranges are well-formed and in-bounds. Used in
    /// debug assertions and test fixtures.
    pub fn check_invariants(&self) -> Result<(), SpanInvariantViolation> {
        let n_moves = self.toolpath.moves.len();
        for (index, s) in self.spans.iter().enumerate() {
            if s.start_move > s.end_move {
                return Err(SpanInvariantViolation::InvertedRange {
                    index,
                    start: s.start_move,
                    end: s.end_move,
                });
            }
            // end_move may equal n_moves (zero-width "after-last" barrier or
            // a span ending exactly at the last move). It must not exceed it.
            if s.end_move > n_moves {
                return Err(SpanInvariantViolation::OutOfBounds {
                    index,
                    end: s.end_move,
                    n_moves,
                });
            }
            if s.kind == SpanKind::RapidOrderBarrier && !s.is_boundary() {
                return Err(SpanInvariantViolation::BarrierNotZeroWidth {
                    index,
                    start: s.start_move,
                    end: s.end_move,
                });
            }
        }
        Ok(())
    }
}

// ── SpanInvariantViolation ──────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpanInvariantViolation {
    InvertedRange {
        index: usize,
        start: usize,
        end: usize,
    },
    OutOfBounds {
        index: usize,
        end: usize,
        n_moves: usize,
    },
    BarrierNotZeroWidth {
        index: usize,
        start: usize,
        end: usize,
    },
}

impl std::fmt::Display for SpanInvariantViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvertedRange { index, start, end } => write!(
                f,
                "span[{index}] has inverted range: start_move={start} > end_move={end}"
            ),
            Self::OutOfBounds {
                index,
                end,
                n_moves,
            } => write!(
                f,
                "span[{index}] end_move={end} exceeds toolpath.moves.len()={n_moves}"
            ),
            Self::BarrierNotZeroWidth { index, start, end } => write!(
                f,
                "span[{index}] is a RapidOrderBarrier but is not zero-width: \
                 start_move={start}, end_move={end}"
            ),
        }
    }
}

impl std::error::Error for SpanInvariantViolation {}

// ── MoveRemap ───────────────────────────────────────────────────────────

/// Mapping from old move indices to post-transform move ranges.
///
/// A transform that mutates the move list should emit one of these so spans
/// can be remapped mechanically by [`MoveRemap::remap_range`].
///
/// `old_to_new[i]` = `Some(range)` means the old move at index `i` ended up
/// covering the half-open range `range` in the new toolpath; `None` means
/// the move was dropped.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MoveRemap {
    pub old_to_new: Vec<Option<Range<usize>>>,
}

impl MoveRemap {
    /// Identity remap: each move maps to a single-element range at the same index.
    pub fn identity(n_moves: usize) -> Self {
        Self {
            old_to_new: (0..n_moves).map(|i| Some(i..i + 1)).collect(),
        }
    }

    /// Remap a half-open old span `[start..end)` to its new range. Returns
    /// `None` if every old move in the input range was dropped.
    pub fn remap_range(&self, start: usize, end: usize) -> Option<Range<usize>> {
        if start > end {
            return None;
        }
        let new_start = (start..end)
            .filter_map(|i| self.old_to_new.get(i).cloned().flatten())
            .map(|r| r.start)
            .min()?;
        let new_end = (start..end)
            .filter_map(|i| self.old_to_new.get(i).cloned().flatten())
            .map(|r| r.end)
            .max()?;
        Some(new_start..new_end)
    }

    /// Remap an old boundary index (zero-width position between moves) to a
    /// new boundary index. The boundary at old index `i` sits *before* old
    /// move `i`; its new position is the start of the remapped range for old
    /// move `i`, or `total_new_moves` if `i` was past the end of the old list.
    pub fn remap_boundary(&self, old_boundary: usize, total_new_moves: usize) -> usize {
        if let Some(Some(r)) = self.old_to_new.get(old_boundary) {
            return r.start;
        }
        // Boundary past the end of the old toolpath, or the move at this
        // index was dropped — fall back to scanning forward for the next
        // surviving move.
        self.old_to_new
            .iter()
            .skip(old_boundary)
            .flatten()
            .next()
            .map(|r| r.start)
            .unwrap_or(total_new_moves)
    }

    /// Remap one span through this move-index mapping.
    ///
    /// - Boundary (zero-width) spans remap through [`Self::remap_boundary`]
    ///   and always come back zero-width.
    /// - Non-boundary spans remap through [`Self::remap_range`], producing
    ///   the bounding new range — the min start and max end among all
    ///   surviving old moves in `[span.start_move, span.end_move)`.
    ///
    /// Returns `None` if every old move in the span's range was dropped by
    /// the transform (the span fully collapsed). `kind` is preserved
    /// unchanged; `label` and `payload` are cloned onto the output span.
    /// Did a move from OUTSIDE the old range `[old_start, old_end)` land
    /// inside `bounds` (that range's new bounding range)?
    ///
    /// `Some((old_index, new_range))` names the intruder — which move it is
    /// is the whole diagnosis, so the caller can log it rather than reporting
    /// "scattered" for a range that came back near-exact.
    ///
    /// A bounding remap cannot see this on its own: only a REORDER can
    /// interleave strangers into a contiguous claim, so this is the extra
    /// rule [`crate::tsp`] applies to spans and
    /// [`crate::transform_provenance::MoveProvenance::Permutation`] applies
    /// to every other index-carrying channel. One predicate, so the two can
    /// never disagree about what "scattered" means.
    pub fn foreign_intrusion(
        &self,
        old_start: usize,
        old_end: usize,
        bounds: &Range<usize>,
    ) -> Option<(usize, Range<usize>)> {
        self.old_to_new
            .iter()
            .enumerate()
            .filter(|(i, _)| *i < old_start || *i >= old_end)
            .find_map(|(i, slot)| {
                let r = slot.as_ref()?;
                // Non-trivial overlap: the slot covers an actual new-move
                // index (start < end) AND that index is inside `bounds`.
                (r.start < r.end && r.start < bounds.end && r.end > bounds.start)
                    .then(|| (i, r.clone()))
            })
    }

    pub fn remap_span(&self, span: &Span, new_n_moves: usize) -> Option<Span> {
        let mut new_span = if span.is_boundary() {
            let new_pos = self.remap_boundary(span.start_move, new_n_moves);
            Span::new(new_pos, new_pos, span.kind)
        } else {
            let r = self.remap_range(span.start_move, span.end_move)?;
            Span::new(r.start, r.end, span.kind)
        }
        .with_label(span.label.clone());
        if let Some(p) = span.payload.clone() {
            new_span = new_span.with_payload(p);
        }
        Some(new_span)
    }

    /// Same contract as [`Self::remap_span`], but answers the non-boundary
    /// range query through a pre-built [`RemapIndex`] instead of rescanning
    /// `old_to_new`. Byte-for-byte identical output to `remap_span` — see
    /// [`RemapIndex::remap_range`] for why the answers agree exactly.
    ///
    /// Boundary spans still go through [`Self::remap_boundary`] unindexed:
    /// C9 only targeted the two hot scans (`remap_range`,
    /// `foreign_intrusion`); `remap_boundary`'s fallback walk is triggered
    /// only for a dropped/out-of-range boundary move and was not the
    /// O(spans × moves) hot path this index retires.
    pub fn remap_span_with_index(
        &self,
        span: &Span,
        new_n_moves: usize,
        index: &RemapIndex,
    ) -> Option<Span> {
        let mut new_span = if span.is_boundary() {
            let new_pos = self.remap_boundary(span.start_move, new_n_moves);
            Span::new(new_pos, new_pos, span.kind)
        } else {
            let r = index.remap_range(span.start_move, span.end_move)?;
            Span::new(r.start, r.end, span.kind)
        }
        .with_label(span.label.clone());
        if let Some(p) = span.payload.clone() {
            new_span = new_span.with_payload(p);
        }
        Some(new_span)
    }

    /// Remap a whole span list through this mapping, in order, dropping any
    /// span that fully collapsed (see [`Self::remap_span`]).
    ///
    /// This is the canonical "walk spans through a `MoveRemap`, drop
    /// collapsed spans" contract shared by every span-preserving toolpath
    /// transform that only *narrows or merges* moves in place — dressups
    /// ([`crate::dressup`]), arc-fitting ([`crate::arcfit`]), and path
    /// simplification ([`crate::condition`]). Callers append their own
    /// transform-introduced spans (e.g. `Entry`, `DressupArtifact`,
    /// `LinkBridge`) to the returned vec afterward.
    ///
    /// TSP's reordering pass ([`crate::tsp`]) additionally has to detect
    /// *foreign-move intrusion* — a permutation can interleave moves from
    /// other spans into a span's new bounding range, which a plain bounding
    /// remap can't see. `tsp::remap_spans` delegates its per-span core to
    /// [`Self::remap_span_with_index`] and layers that check on top as a
    /// distinct post-filter (via [`RemapIndex::foreign_intrusion`]) rather
    /// than reimplementing this method.
    ///
    /// C9: builds one [`RemapIndex`] up front and answers every span's
    /// range query against it in O(log n) rather than rescanning the whole
    /// `old_to_new` vector per span — the O(spans × moves) scan this whole
    /// index exists to retire. On a wanaka-class job (~200k moves, dozens to
    /// hundreds of spans) that was the dominant cost of every span-preserving
    /// transform that calls this method.
    pub fn remap_spans(&self, spans: &[Span], new_n_moves: usize) -> Vec<Span> {
        let index = RemapIndex::build(self);
        spans
            .iter()
            .filter_map(|s| self.remap_span_with_index(s, new_n_moves, &index))
            .collect()
    }
}

// ── RemapIndex ──────────────────────────────────────────────────────────

/// Outcome of one [`AggregateTree::leftmost_overlap`] descent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TreeOutcome {
    /// Leftmost matching leaf index (always `< n`; padding leaves carry
    /// identity values that can never pass the overlap test).
    Found(usize),
    /// No leaf in the queried range overlaps `bounds`.
    NotFound,
    /// The node-visit budget ran out before the descent could prove either
    /// answer. Caller falls back to [`AggregateTree::linear_leftmost_overlap`].
    BudgetExceeded,
}

/// ONE tree implementation, shared by both [`RemapIndex::remap_range`] and
/// [`RemapIndex::foreign_intrusion`] — they were computing the SAME kind of
/// aggregate (`min_start`/`max_end` over a set of slots) through two
/// unrelated structures before this rewrite (a sparse table for one, this
/// tree's ancestor for the other); that duplication is gone. Only the slot
/// set fed into [`Self::build`] differs between the two call sites — ALL
/// slots (degenerate included) for `remap_range`, non-degenerate slots only
/// for `foreign_intrusion` — plus whether the caller needs top-down descent
/// (see `pad_for_descent` below).
///
/// Complete-or-not binary tree, 1-indexed (`node` 1 is the root; children of
/// `node` are `2*node`/`2*node + 1`; leaf `i` lives at array index `size +
/// i`). Each node stores `(min_start, max_end)` — the min/max over its
/// subtree's slots (identity `(usize::MAX, 0)` for padding/absent slots, so
/// they can never win a `min`/`max` reduction).
///
/// `size` — and therefore whether the tree is safe for
/// [`Self::leftmost_overlap`] — is chosen at build time:
///
/// - [`Self::range_query`] (an iterative bottom-up min/max reduction) is
///   correct for ANY `size`, padded to a power of two or not — verified by
///   hand-tracing a 3-leaf (non-power-of-two) tree before writing this: the
///   `l/r` parity walk only ever accumulates a node once its FULL subtree is
///   proven to sit inside `[l, r)`, which the walk's index arithmetic
///   guarantees regardless of how lopsided that subtree's shape is. So
///   `remap_range`'s tree (`pad_for_descent: false`) uses `size = n` exactly
///   — no padding waste.
/// - [`Self::leftmost_overlap`] (top-down descent, used by
///   `foreign_intrusion`) is NOT safe on an unpadded tree: it computes each
///   node's `[node_lo, node_hi)` span purely from arithmetic
///   (`mid = node_lo + (node_hi - node_lo) / 2`) assuming `node`'s two
///   children cleanly bisect that span — true only for a COMPLETE binary
///   tree. Hand-checked for `n = 3` (unpadded) before writing this: node 1's
///   children are indices 2 and 3, but index 3 is already a LEAF while index
///   2 is still internal — the tree is lopsided, so descent's arithmetic
///   would silently read the wrong leaf. `foreign_intrusion`'s tree
///   (`pad_for_descent: true`) therefore pads to `next_power_of_two(n)`.
struct AggregateTree {
    size: usize,
    depth: u32,
    min_start: Vec<usize>,
    max_end: Vec<usize>,
}

impl AggregateTree {
    fn build(starts: &[usize], ends: &[usize], pad_for_descent: bool) -> Self {
        let n = starts.len();
        let size = if pad_for_descent {
            n.max(1).next_power_of_two()
        } else {
            n.max(1)
        };
        let mut min_start = vec![usize::MAX; 2 * size];
        let mut max_end = vec![0usize; 2 * size];

        for (i, (&s, &e)) in starts.iter().zip(ends.iter()).enumerate() {
            if let Some(slot) = min_start.get_mut(size + i) {
                *slot = s;
            }
            if let Some(slot) = max_end.get_mut(size + i) {
                *slot = e;
            }
        }

        // Children always have a larger array index than their parent (for
        // ANY size, padded or not — this is pure index arithmetic, not a
        // property of a complete tree), so combining nodes in decreasing
        // index order guarantees both children are final before their
        // parent reads them.
        for node in (1..size).rev() {
            let left = 2 * node;
            let right = 2 * node + 1;
            let lms = min_start.get(left).copied().unwrap_or(usize::MAX);
            let rms = min_start.get(right).copied().unwrap_or(usize::MAX);
            let lme = max_end.get(left).copied().unwrap_or(0);
            let rme = max_end.get(right).copied().unwrap_or(0);
            if let Some(slot) = min_start.get_mut(node) {
                *slot = lms.min(rms);
            }
            if let Some(slot) = max_end.get_mut(node) {
                *slot = lme.max(rme);
            }
        }

        let depth = size.next_power_of_two().trailing_zeros();
        Self {
            size,
            depth,
            min_start,
            max_end,
        }
    }

    /// This slot's raw `(start, end)` as stored at the leaf — identity
    /// `(usize::MAX, 0)` for a padding/absent slot. Used both to answer
    /// [`RemapIndex::foreign_intrusion`]'s final `(index, range)` and by
    /// [`Self::linear_leftmost_overlap`]'s fallback scan, so there is no
    /// separate raw-slot array duplicating what the tree already holds.
    fn leaf(&self, i: usize) -> (usize, usize) {
        let s = self
            .min_start
            .get(self.size + i)
            .copied()
            .unwrap_or(usize::MAX);
        let e = self.max_end.get(self.size + i).copied().unwrap_or(0);
        (s, e)
    }

    /// Combined min(`min_start`)/max(`max_end`) over `[l, r)`, `l < r`
    /// assumed. Standard iterative bottom-up segment-tree range query,
    /// O(log n) — correct for any `size` (see this struct's doc comment).
    fn range_query(&self, l: usize, r: usize) -> (usize, usize) {
        let mut lo = l + self.size;
        let mut hi = r + self.size;
        let mut min_start = usize::MAX;
        let mut max_end = 0usize;
        while lo < hi {
            if lo % 2 == 1 {
                min_start = min_start.min(self.min_start.get(lo).copied().unwrap_or(usize::MAX));
                max_end = max_end.max(self.max_end.get(lo).copied().unwrap_or(0));
                lo += 1;
            }
            if hi % 2 == 1 {
                hi -= 1;
                min_start = min_start.min(self.min_start.get(hi).copied().unwrap_or(usize::MAX));
                max_end = max_end.max(self.max_end.get(hi).copied().unwrap_or(0));
            }
            lo /= 2;
            hi /= 2;
        }
        (min_start, max_end)
    }

    /// Heuristic node-visit cap for one [`Self::leftmost_overlap`] call.
    ///
    /// NOT a proven worst-case bound — see [`RemapIndex`]'s doc comment.
    /// It exists purely so a pathological `old_to_new` (one that defeats the
    /// necessary-condition pruning at every level) can't make a single query
    /// visit more than this many nodes before the caller falls back to the
    /// linear scan; `4096` is a floor so small trees don't fall back
    /// needlessly early, `64 * depth^2` scales the cap with tree height.
    fn query_budget(&self) -> usize {
        let d = self.depth as usize;
        4096usize.max(64 * d * d)
    }

    /// Leftmost leaf index in `range` whose stored interval overlaps
    /// `bounds`, or the reason the descent didn't reach an answer. Only
    /// valid on a tree built with `pad_for_descent: true` — see this
    /// struct's doc comment for why an unpadded tree can't support this.
    fn leftmost_overlap(
        &self,
        range: Range<usize>,
        bounds: &Range<usize>,
        budget: &mut usize,
    ) -> TreeOutcome {
        if range.start >= range.end {
            return TreeOutcome::NotFound;
        }
        self.descend(1, 0, self.size, &range, bounds, budget)
    }

    fn descend(
        &self,
        node: usize,
        node_lo: usize,
        node_hi: usize,
        range: &Range<usize>,
        bounds: &Range<usize>,
        budget: &mut usize,
    ) -> TreeOutcome {
        if *budget == 0 {
            return TreeOutcome::BudgetExceeded;
        }
        *budget -= 1;

        if node_hi <= range.start || range.end <= node_lo {
            return TreeOutcome::NotFound;
        }
        let node_min = self.min_start.get(node).copied().unwrap_or(usize::MAX);
        let node_max = self.max_end.get(node).copied().unwrap_or(0);
        if !(node_min < bounds.end && node_max > bounds.start) {
            return TreeOutcome::NotFound;
        }
        if node_hi - node_lo == 1 {
            // Necessary condition held AND this is a genuine leaf (not a
            // collapsed subtree summary), so it's sufficient too.
            return TreeOutcome::Found(node_lo);
        }

        let mid = node_lo + (node_hi - node_lo) / 2;
        match self.descend(2 * node, node_lo, mid, range, bounds, budget) {
            TreeOutcome::Found(i) => TreeOutcome::Found(i),
            TreeOutcome::BudgetExceeded => TreeOutcome::BudgetExceeded,
            TreeOutcome::NotFound => {
                self.descend(2 * node + 1, mid, node_hi, range, bounds, budget)
            }
        }
    }

    /// Exact leftmost-index linear scan of `[range.start, range.end)`, used
    /// as the un-indexed fallback when [`Self::leftmost_overlap`]'s budget
    /// is exhausted. Reads through [`Self::leaf`], so it sees the same
    /// identity-folded values `leftmost_overlap` does — no separate raw
    /// array to keep in sync.
    fn linear_leftmost_overlap(&self, range: Range<usize>, bounds: &Range<usize>) -> Option<usize> {
        (range.start..range.end).find(|&i| {
            let (s, e) = self.leaf(i);
            s < bounds.end && e > bounds.start
        })
    }
}

/// Interval index built once from a [`MoveRemap`], answering the two hot
/// per-span queries — [`MoveRemap::remap_range`] and
/// [`MoveRemap::foreign_intrusion`] — without rescanning `old_to_new` for
/// every span (C9: the O(spans × moves) scan on a wanaka-class job, ~200k
/// moves × dozens-to-hundreds of spans, was the dominant cost of every
/// span-preserving toolpath transform).
///
/// Built from exactly two [`AggregateTree`]s (see that struct's doc comment
/// for why there are two, and why only one of them pays for descent
/// support) plus a prefix count of surviving slots. An earlier revision of
/// this index answered `remap_range` with a *second*, independent
/// structure — an O(n log n) sparse table — even though the intrusion tree
/// already stored the exact same `(min_start, max_end)` aggregate; that
/// duplication measured ~67 MB of transient allocation on a 200k-move
/// fixture and was cut in favor of the second `AggregateTree` below.
///
/// # Complexity and memory
///
/// - [`Self::remap_range`]: `range_tree` (built over ALL slots, degenerate
///   included — `remap_range`'s oracle never distinguishes `r.start ==
///   r.end` from a normal slot) answers the min/max reduction via
///   [`AggregateTree::range_query`] in O(log n) after an O(n) build. A
///   prefix-count of surviving (`Some`) slots answers "did anything survive
///   this range" in O(1) so an all-dropped sub-range still returns `None`
///   exactly like the two-pass scan; a plain prefix-sum could not answer
///   the *range* query itself (it only ever gives prefix-from-zero), which
///   is why the aggregate still needs the tree.
/// - [`Self::foreign_intrusion`]: `intrusion_tree` (built over the
///   NON-degenerate slots only) gives an exact leftmost-hit answer (never a
///   weaker "any intruder") via necessary-condition pruning
///   ([`AggregateTree::leftmost_overlap`]). This is NOT a proven O(log n) or
///   O(log² n) bound — an adversarial `old_to_new` can make every node's
///   summary pass the necessary condition without any leaf in it actually
///   overlapping `bounds`, forcing a wide descent. A merge-sort tree
///   (sorted-by-start intervals + running prefix-max end per node) would
///   fix that at a proven O(log² n) worst case, at the cost of another O(n
///   log n) memory structure — not worth it here. This index instead caps
///   descent at a node-visit budget ([`AggregateTree::query_budget`]) and
///   falls back to an exact linear scan
///   ([`AggregateTree::linear_leftmost_overlap`]) the moment the budget is
///   spent, so the worst case is bounded by `O(budget + n)` — never worse
///   than the original `O(n)` scan — while the common case (TSP's
///   block-structured permutations, which is what this index exists to
///   serve) finishes in a handful of node visits.
/// - Memory is O(n) total: `range_tree` is `2 * n` words per array (no
///   padding — [`AggregateTree::range_query`] doesn't need it),
///   `intrusion_tree` is `2 * next_power_of_two(n)` words per array (padded
///   — [`AggregateTree::leftmost_overlap`] does need it), and the prefix
///   count is `n + 1` `u32`s. [`Self::heap_bytes`] reports the real,
///   measured figure rather than a claim this doc comment could drift from;
///   `remap_interval_index_c9.rs`'s wanaka-scale test asserts it stays
///   under 16 MB at n = 200_000 so this regression can't come back silently.
pub struct RemapIndex {
    n: usize,
    survivor_prefix: Vec<u32>,
    range_tree: AggregateTree,
    intrusion_tree: AggregateTree,
}

impl RemapIndex {
    /// Build the index once from a `MoveRemap`. Reuse it for every span in
    /// the same remap rather than rebuilding per span.
    pub fn build(remap: &MoveRemap) -> Self {
        let n = remap.old_to_new.len();

        // ---- remap_range: tree over ALL slots, degenerate included
        // (remap_range's oracle never distinguishes r.start == r.end from a
        // normal slot — see MoveRemap::remap_range). Unpadded: range_query
        // doesn't need descent, so there's no reason to pay for padding.
        let all_starts: Vec<usize> = remap
            .old_to_new
            .iter()
            .map(|slot| slot.as_ref().map_or(usize::MAX, |r| r.start))
            .collect();
        let all_ends: Vec<usize> = remap
            .old_to_new
            .iter()
            .map(|slot| slot.as_ref().map_or(0, |r| r.end))
            .collect();
        let range_tree = AggregateTree::build(&all_starts, &all_ends, false);

        let mut survivor_prefix = Vec::with_capacity(n + 1);
        survivor_prefix.push(0u32);
        let mut running = 0u32;
        for slot in &remap.old_to_new {
            running += u32::from(slot.is_some());
            survivor_prefix.push(running);
        }

        // ---- foreign_intrusion: NON-degenerate slots only (r.start < r.end)
        // — a degenerate or dropped slot can never be an intruder, see
        // MoveRemap::foreign_intrusion's predicate. Padded: leftmost_overlap
        // needs the descent to be well-defined.
        let nd_starts: Vec<usize> = remap
            .old_to_new
            .iter()
            .map(|slot| match slot {
                Some(r) if r.start < r.end => r.start,
                _ => usize::MAX,
            })
            .collect();
        let nd_ends: Vec<usize> = remap
            .old_to_new
            .iter()
            .map(|slot| match slot {
                Some(r) if r.start < r.end => r.end,
                _ => 0,
            })
            .collect();
        let intrusion_tree = AggregateTree::build(&nd_starts, &nd_ends, true);

        Self {
            n,
            survivor_prefix,
            range_tree,
            intrusion_tree,
        }
    }

    fn survivor_count(&self, l: usize, r: usize) -> u32 {
        let hi = self.survivor_prefix.get(r).copied().unwrap_or(0);
        let lo = self.survivor_prefix.get(l).copied().unwrap_or(0);
        hi.saturating_sub(lo)
    }

    /// Indexed equivalent of [`MoveRemap::remap_range`] — same signature,
    /// same answer for every input, O(log n) instead of two O(n) scans.
    pub fn remap_range(&self, start: usize, end: usize) -> Option<Range<usize>> {
        if start > end {
            return None;
        }
        let l = start.min(self.n);
        let r = end.min(self.n);
        if l >= r || self.survivor_count(l, r) == 0 {
            return None;
        }
        let (min_start, max_end) = self.range_tree.range_query(l, r);
        Some(min_start..max_end)
    }

    fn query_range(
        &self,
        range: Range<usize>,
        bounds: &Range<usize>,
        budget: &mut usize,
    ) -> Option<usize> {
        if range.start >= range.end {
            return None;
        }
        match self
            .intrusion_tree
            .leftmost_overlap(range.clone(), bounds, budget)
        {
            TreeOutcome::Found(i) => Some(i),
            TreeOutcome::NotFound => None,
            TreeOutcome::BudgetExceeded => {
                self.intrusion_tree.linear_leftmost_overlap(range, bounds)
            }
        }
    }

    /// Indexed equivalent of [`MoveRemap::foreign_intrusion`] — same
    /// signature, same chosen `(index, range)` for every input (never just
    /// "any intruder"), sub-linear in the common case instead of one O(n)
    /// scan per call.
    pub fn foreign_intrusion(
        &self,
        old_start: usize,
        old_end: usize,
        bounds: &Range<usize>,
    ) -> Option<(usize, Range<usize>)> {
        let n = self.n;
        let mut budget = self.intrusion_tree.query_budget();
        let r1 = 0..old_start.min(n);
        let hit = self.query_range(r1, bounds, &mut budget).or_else(|| {
            let r2 = old_end.min(n)..n;
            self.query_range(r2, bounds, &mut budget)
        });
        hit.map(|i| {
            let (start, end) = self.intrusion_tree.leaf(i);
            (i, start..end)
        })
    }

    /// Actual heap footprint of this index in bytes — sums the real
    /// (allocated, not just occupied) capacity of every backing `Vec`
    /// rather than re-deriving an estimate from `n`, so a formula/reality
    /// drift in this struct's doc comment would show up as a test assertion
    /// failing, not as a stale comment nobody re-checks.
    pub fn heap_bytes(&self) -> usize {
        let word = std::mem::size_of::<usize>();
        let tree_bytes = |t: &AggregateTree| (t.min_start.capacity() + t.max_end.capacity()) * word;
        let prefix_bytes = self.survivor_prefix.capacity() * std::mem::size_of::<u32>();
        tree_bytes(&self.range_tree) + tree_bytes(&self.intrusion_tree) + prefix_bytes
    }
}

// ── tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;
    use crate::geo::P3;
    use crate::toolpath::Toolpath;

    fn toolpath_with_n_moves(n: usize) -> Toolpath {
        let mut tp = Toolpath::new();
        for i in 0..n {
            tp.feed_to(P3::new(i as f64, 0.0, 0.0), 1000.0);
        }
        tp
    }

    // ── Span basics ────────────────────────────────────────────────────

    /// F2.2 — the intent-derived transit bitmap reads per-move
    /// `MoveIntent` tags, not spans, so it stays correct when
    /// `spans_valid == false` (TSP split). Transit intents (Linking /
    /// Entry* / LeadIn / LeadOut) mark true; cuts, retracts, drilling,
    /// and Unknown mark false.
    #[test]
    fn transit_bitmap_from_intents_ignores_spans() {
        use crate::toolpath::MoveIntent as I;

        let mut tp = Toolpath::new();
        let p = P3::new(0.0, 0.0, 0.0);
        let intents = [
            I::EntryHelix,
            I::ClearingCut,
            I::Linking,
            I::FinishingCut,
            I::LeadIn,
            I::LeadOut,
            I::Retract,
            I::Drilling,
            I::Unknown,
        ];
        for intent in intents {
            tp.feed_to_with_intent(p, 1000.0, intent);
        }
        // Deliberately corrupted span vector + invalid flag — the
        // intent bitmap must not consult either.
        let mut at = AnnotatedToolpath::with_spans(
            tp,
            vec![Span::new(0, 9, SpanKind::Entry).with_label("bogus")],
        );
        at.spans_valid = false;

        assert_eq!(
            at.transit_moves_bitmap_from_intents(),
            vec![true, false, true, false, true, true, false, false, false],
        );
    }

    #[test]
    fn span_new_has_default_label_and_no_payload() {
        let s = Span::new(0, 5, SpanKind::DepthPass);
        assert_eq!(s.start_move, 0);
        assert_eq!(s.end_move, 5);
        assert_eq!(s.kind, SpanKind::DepthPass);
        assert_eq!(s.label, "");
        assert!(s.payload.is_none());
    }

    #[test]
    fn span_with_label_and_payload_round_trip() {
        let s = Span::new(10, 20, SpanKind::Region)
            .with_label("region-3")
            .with_payload(SpanPayload::Region {
                region_id: 3,
                role: RegionSpanRole::GeneratorPass,
            });
        assert_eq!(s.label, "region-3");
        assert_eq!(
            s.payload,
            Some(SpanPayload::Region {
                region_id: 3,
                role: RegionSpanRole::GeneratorPass,
            })
        );
    }

    /// Wave D3: the node/pass discriminator is a payload field, readable
    /// without touching the label — the label is free text and nothing keeps
    /// it stable.
    #[test]
    fn region_role_discriminates_node_from_generator_pass() {
        let node = Span::new(0, 10, SpanKind::Region)
            .with_label("MidSteep band")
            .with_payload(SpanPayload::Region {
                region_id: 0,
                role: RegionSpanRole::Node,
            });
        // Same kind, same region_id, DIFFERENT id space.
        let ring = Span::new(0, 4, SpanKind::Region)
            .with_label("Ring 1/9")
            .with_payload(SpanPayload::Region {
                region_id: 0,
                role: RegionSpanRole::GeneratorPass,
            });
        assert!(node.is_region_node());
        assert!(!ring.is_region_node());
        assert_eq!(node.region_role(), Some(RegionSpanRole::Node));
        assert_eq!(ring.region_role(), Some(RegionSpanRole::GeneratorPass));
        // A non-region span has no role at all.
        assert_eq!(Span::new(0, 3, SpanKind::Entry).region_role(), None);
        // And the roles survive a remap (payload is cloned through).
        let mapping: Vec<usize> = (0..=10).map(|i| i * 2).collect();
        assert_eq!(
            node.remap(&mapping).region_role(),
            Some(RegionSpanRole::Node)
        );
    }

    /// C4: the drill nesting is a role pair, and the whole vocabulary
    /// round-trips through its key. `ALL` is the single list `from_key`
    /// derives from, so a variant that is added but not listed cannot be
    /// looked up by name — the same contract `SpanKind::ALL` carries.
    #[test]
    fn every_region_role_round_trips_through_its_key() {
        assert_eq!(RegionSpanRole::ALL.len(), 4);
        for role in RegionSpanRole::ALL {
            assert_eq!(
                RegionSpanRole::from_key(role.label()),
                Some(role),
                "{} did not round-trip",
                role.label()
            );
        }
        assert_eq!(RegionSpanRole::from_key("generator-pass"), None);
        assert_eq!(RegionSpanRole::from_key(""), None);

        // The keys are distinct — a copy-pasted arm in `label` would
        // otherwise make one role unreachable through `from_key`.
        let mut keys: Vec<&str> = RegionSpanRole::ALL.iter().map(|r| r.label()).collect();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), RegionSpanRole::ALL.len());
    }

    /// `has_region_role` is the supported replacement for the drill label
    /// parsing (`label.starts_with("Hole ") && !label.contains("plunge")`).
    /// It must reject boundary markers and non-region spans, which the label
    /// predicate did by accident rather than by contract.
    #[test]
    fn has_region_role_answers_the_drill_nesting_question() {
        let hole = Span::new(0, 20, SpanKind::Region)
            .with_label("Hole 1")
            .with_payload(SpanPayload::Region {
                region_id: 0,
                role: RegionSpanRole::DrillHole,
            });
        let peck = Span::new(2, 6, SpanKind::Region)
            .with_label("Hole 1 plunge 1")
            .with_payload(SpanPayload::Region {
                region_id: 3,
                role: RegionSpanRole::DrillPeck,
            });
        assert!(hole.has_region_role(RegionSpanRole::DrillHole));
        assert!(!hole.has_region_role(RegionSpanRole::DrillPeck));
        assert!(peck.has_region_role(RegionSpanRole::DrillPeck));
        assert!(!peck.has_region_role(RegionSpanRole::DrillHole));
        // Neither is a planner node, and neither is the plain generator pass
        // they both used to be.
        assert!(!hole.is_region_node() && !peck.is_region_node());
        assert!(!hole.has_region_role(RegionSpanRole::GeneratorPass));
        // …but both ARE generator output.
        assert!(RegionSpanRole::DrillHole.is_generator_pass());
        assert!(RegionSpanRole::DrillPeck.is_generator_pass());
        assert!(!RegionSpanRole::Node.is_generator_pass());

        // A zero-width boundary marker carrying the role is still not a hole.
        let boundary = Span::new(20, 20, SpanKind::Region).with_payload(SpanPayload::Region {
            region_id: 9,
            role: RegionSpanRole::DrillHole,
        });
        assert!(boundary.is_boundary());
        assert!(!boundary.has_region_role(RegionSpanRole::DrillHole));
    }

    #[test]
    fn span_move_count_and_boundary() {
        let normal = Span::new(2, 7, SpanKind::DepthPass);
        assert_eq!(normal.move_count(), 5);
        assert!(!normal.is_boundary());

        let barrier = Span::boundary(7, SpanKind::RapidOrderBarrier);
        assert_eq!(barrier.move_count(), 0);
        assert!(barrier.is_boundary());
    }

    #[test]
    fn span_contains_is_half_open() {
        let s = Span::new(2, 7, SpanKind::DepthPass);
        assert!(!s.contains(1));
        assert!(s.contains(2));
        assert!(s.contains(6));
        assert!(!s.contains(7));
    }

    #[test]
    fn boundary_span_never_contains_anything() {
        let b = Span::boundary(3, SpanKind::RapidOrderBarrier);
        assert!(!b.contains(2));
        assert!(!b.contains(3));
        assert!(!b.contains(4));
        assert!(b.is_boundary_at(3));
        assert!(!b.is_boundary_at(4));
    }

    #[test]
    fn span_range_returns_half_open_range() {
        let s = Span::new(2, 7, SpanKind::DepthPass);
        assert_eq!(s.range(), 2..7);
    }

    // ── AnnotatedToolpath construction ─────────────────────────────────

    #[test]
    fn new_creates_valid_empty_spans() {
        let at = AnnotatedToolpath::new(toolpath_with_n_moves(5));
        assert!(at.spans.is_empty());
        assert!(at.spans_valid);
    }

    #[test]
    fn with_spans_preserves_input() {
        let spans = vec![
            Span::new(0, 5, SpanKind::Operation),
            Span::new(0, 2, SpanKind::DepthPass),
        ];
        let at = AnnotatedToolpath::with_spans(toolpath_with_n_moves(5), spans.clone());
        assert_eq!(at.spans, spans);
        assert!(at.spans_valid);
    }

    // ── translated ──────────────────────────────────────────────────────

    /// `translated` must re-frame every coordinate-bearing field in lockstep:
    /// move targets, `planner_engagement` cut points, `rest_grid`
    /// origin/surface_z, and `rest_regions` polygon points — while leaving
    /// move-index-based fields (`spans`) and scalar fields (`rest_grid.rest`)
    /// untouched, and preserving NaN (untrusted) cells.
    #[test]
    fn translated_shifts_moves_engagement_and_rest_grid_in_lockstep() {
        let mut tp = Toolpath::new();
        tp.feed_to(P3::new(1.0, 2.0, 3.0), 1000.0);

        let rest_grid = RestGrid {
            nx: 2,
            ny: 2,
            origin_x: 10.0,
            origin_y: 20.0,
            cell_mm: 1.0,
            rest: vec![0.1, 0.2, 0.3, 0.4],
            surface_z: vec![5.0, f32::NAN, 7.0, 8.0],
            threshold: 0.05,
        };

        let region = Polygon2::with_holes(
            vec![
                crate::geo::P2::new(0.0, 0.0),
                crate::geo::P2::new(5.0, 0.0),
                crate::geo::P2::new(5.0, 5.0),
                crate::geo::P2::new(0.0, 5.0),
            ],
            vec![vec![
                crate::geo::P2::new(1.0, 1.0),
                crate::geo::P2::new(2.0, 1.0),
                crate::geo::P2::new(2.0, 2.0),
            ]],
        );

        let mut at = AnnotatedToolpath::new(tp);
        at.planner_engagement = vec![(P3::new(1.0, 2.0, 3.0), 0.25)];
        at.rest_grid = Some(Arc::new(rest_grid));
        at.rest_regions = Some(Arc::new(vec![region]));

        let shift = P3::new(100.0, 200.0, 10.0);
        let shifted = at.translated(shift);

        assert_eq!(
            shifted.toolpath.moves[0].target,
            P3::new(101.0, 202.0, 13.0)
        );

        assert_eq!(shifted.planner_engagement.len(), 1);
        assert_eq!(shifted.planner_engagement[0].0, P3::new(101.0, 202.0, 13.0));
        assert_eq!(shifted.planner_engagement[0].1, 0.25);

        let grid = shifted
            .rest_grid
            .expect("rest_grid should survive translation");
        assert_eq!(grid.origin_x, 110.0);
        assert_eq!(grid.origin_y, 220.0);
        assert_eq!(grid.surface_z[0], 15.0);
        assert!(grid.surface_z[1].is_nan(), "NaN cell must stay NaN");
        assert_eq!(grid.surface_z[2], 17.0);
        assert_eq!(grid.surface_z[3], 18.0);
        // rest depth is a scalar, not a coordinate — unchanged.
        assert_eq!(grid.rest, vec![0.1, 0.2, 0.3, 0.4]);

        let regions = shifted
            .rest_regions
            .expect("rest_regions should survive translation");
        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0].exterior[0], crate::geo::P2::new(100.0, 200.0));
        assert_eq!(regions[0].exterior[2], crate::geo::P2::new(105.0, 205.0));
        assert_eq!(regions[0].holes[0][0], crate::geo::P2::new(101.0, 201.0));

        // Original untouched.
        assert_eq!(at.toolpath.moves[0].target, P3::new(1.0, 2.0, 3.0));
        assert_eq!(at.rest_grid.as_ref().map(|g| g.origin_x), Some(10.0));
        assert_eq!(
            at.rest_regions.as_ref().map(|r| r[0].exterior[0]),
            Some(crate::geo::P2::new(0.0, 0.0))
        );
    }

    // ── spans_at / spans_of_kind / boundaries_at ───────────────────────

    #[test]
    fn spans_at_returns_only_covering_spans() {
        let at = AnnotatedToolpath::with_spans(
            toolpath_with_n_moves(10),
            vec![
                Span::new(0, 10, SpanKind::Operation),
                Span::new(0, 5, SpanKind::DepthPass).with_label("z=10"),
                Span::new(5, 10, SpanKind::DepthPass).with_label("z=5"),
                Span::boundary(5, SpanKind::RapidOrderBarrier),
            ],
        );
        let at_3: Vec<_> = at.spans_at(3).map(|s| s.kind).collect();
        assert_eq!(at_3, vec![SpanKind::Operation, SpanKind::DepthPass]);

        let at_5: Vec<_> = at.spans_at(5).map(|s| s.kind).collect();
        // The boundary at 5 is excluded; only Operation and the second DepthPass cover move 5.
        assert_eq!(at_5, vec![SpanKind::Operation, SpanKind::DepthPass]);
    }

    #[test]
    fn boundaries_at_returns_only_zero_width_at_index() {
        let at = AnnotatedToolpath::with_spans(
            toolpath_with_n_moves(10),
            vec![
                Span::new(0, 10, SpanKind::Operation),
                Span::boundary(5, SpanKind::RapidOrderBarrier),
                Span::boundary(7, SpanKind::RapidOrderBarrier),
            ],
        );
        let at_5: Vec<_> = at.boundaries_at(5).map(|s| s.start_move).collect();
        assert_eq!(at_5, vec![5]);
        let at_6: Vec<_> = at.boundaries_at(6).collect();
        assert!(at_6.is_empty());
    }

    #[test]
    fn span_path_at_returns_outermost_first() {
        let at = AnnotatedToolpath::with_spans(
            toolpath_with_n_moves(10),
            vec![
                Span::new(0, 10, SpanKind::Operation),
                Span::new(0, 5, SpanKind::DepthPass),
                Span::new(2, 4, SpanKind::Region),
                Span::boundary(5, SpanKind::RapidOrderBarrier),
                Span::new(5, 10, SpanKind::DepthPass),
            ],
        );
        // Move 3 is inside Operation (idx 0), DepthPass (idx 1), Region (idx 2).
        let path = at.span_path_at(3);
        assert_eq!(path, vec![SpanId(0), SpanId(1), SpanId(2)]);
        // Move 6 is inside Operation + second DepthPass; the boundary is excluded.
        let path = at.span_path_at(6);
        assert_eq!(path, vec![SpanId(0), SpanId(4)]);
    }

    #[test]
    fn transit_moves_bitmap_flags_transit_kinds() {
        let at = AnnotatedToolpath::with_spans(
            toolpath_with_n_moves(10),
            vec![
                Span::new(0, 10, SpanKind::Operation),
                Span::new(0, 2, SpanKind::Entry),
                Span::new(2, 6, SpanKind::DepthPass),
                Span::new(6, 8, SpanKind::LinkBridge),
                Span::new(8, 10, SpanKind::WaterlineCleanup),
            ],
        );
        let transit = at.transit_moves_bitmap();
        assert_eq!(transit.len(), 10);
        // Moves 0..2: Entry → transit
        assert!(transit[0]);
        assert!(transit[1]);
        // Moves 2..6: DepthPass → not transit
        for (i, &t) in transit.iter().enumerate().take(6).skip(2) {
            assert!(!t, "move {i} (DepthPass) should not be transit");
        }
        // Moves 6..8: LinkBridge → transit
        assert!(transit[6]);
        assert!(transit[7]);
        // Moves 8..10: WaterlineCleanup → transit
        assert!(transit[8]);
        assert!(transit[9]);
    }

    #[test]
    fn transit_moves_bitmap_ignores_cutting_kinds() {
        let at = AnnotatedToolpath::with_spans(
            toolpath_with_n_moves(6),
            vec![
                Span::new(0, 6, SpanKind::Operation),
                Span::new(0, 3, SpanKind::DepthPass),
                Span::new(3, 6, SpanKind::Region),
                Span::boundary(3, SpanKind::RapidOrderBarrier),
            ],
        );
        let transit = at.transit_moves_bitmap();
        // Operation / DepthPass / Region / RapidOrderBarrier are NOT transit.
        assert_eq!(transit, vec![false; 6]);
    }

    #[test]
    fn transit_moves_bitmap_handles_dressup_and_leadout() {
        let at = AnnotatedToolpath::with_spans(
            toolpath_with_n_moves(6),
            vec![
                Span::new(0, 6, SpanKind::Operation),
                Span::new(0, 2, SpanKind::DressupArtifact),
                Span::new(2, 4, SpanKind::Region),
                Span::new(4, 6, SpanKind::LeadOut),
            ],
        );
        let transit = at.transit_moves_bitmap();
        assert!(transit[0] && transit[1]); // DressupArtifact
        assert!(!transit[2] && !transit[3]); // Region
        assert!(transit[4] && transit[5]); // LeadOut
    }

    #[test]
    fn span_paths_by_move_matches_per_move_lookup() {
        let at = AnnotatedToolpath::with_spans(
            toolpath_with_n_moves(8),
            vec![
                Span::new(0, 8, SpanKind::Operation),
                Span::new(0, 4, SpanKind::DepthPass),
                Span::new(4, 8, SpanKind::DepthPass),
                Span::boundary(4, SpanKind::RapidOrderBarrier),
            ],
        );
        let bulk = at.span_paths_by_move();
        assert_eq!(bulk.len(), 8);
        for (i, path) in bulk.iter().enumerate() {
            assert_eq!(*path, at.span_path_at(i), "mismatch at move {i}");
        }
    }

    // ── Span::remap ─────────────────────────────────────────────────────

    #[test]
    fn remap_identity_mapping_preserves_span() {
        let span = Span::new(2, 5, SpanKind::Region);
        let mapping: Vec<usize> = (0..=10).collect();
        let out = span.remap(&mapping);
        assert_eq!(out.start_move, 2);
        assert_eq!(out.end_move, 5);
        assert_eq!(out.kind, SpanKind::Region);
    }

    #[test]
    fn remap_dilating_mapping_expands_span() {
        // input moves 0..5; each input move produces 2 outputs except move 3 which produces 1.
        // mapping: [0, 2, 4, 6, 7, 9]
        let mapping = vec![0, 2, 4, 6, 7, 9];
        let span = Span::new(1, 4, SpanKind::Region);
        let out = span.remap(&mapping);
        assert_eq!(out.start_move, 2);
        assert_eq!(out.end_move, 7);
    }

    #[test]
    fn remap_boundary_span_preserves_zero_width() {
        let mapping = vec![0, 2, 4, 6, 7, 9];
        let span = Span::boundary(3, SpanKind::RapidOrderBarrier);
        let out = span.remap(&mapping);
        assert_eq!(out.start_move, 6);
        assert_eq!(out.end_move, 6);
        assert!(out.is_boundary());
    }

    #[test]
    fn remap_payload_preserved() {
        let span = Span::new(0, 3, SpanKind::DepthPass).with_payload(SpanPayload::DepthPass {
            z_level: -1.5,
            pass_index: 2,
        });
        let mapping = vec![0, 1, 4, 5];
        let out = span.remap(&mapping);
        assert_eq!(out.payload, span.payload);
        assert_eq!(out.start_move, 0);
        assert_eq!(out.end_move, 5);
    }

    #[test]
    fn remap_out_of_range_clamps_to_sentinel() {
        // Malformed input: span end past mapping length. Should clamp rather
        // than panic.
        let mapping = vec![0, 2, 4];
        let span = Span::new(0, 99, SpanKind::Region);
        let out = span.remap(&mapping);
        assert_eq!(out.start_move, 0);
        assert_eq!(out.end_move, 4);
    }

    #[test]
    fn spans_of_kind_filters_by_kind() {
        let at = AnnotatedToolpath::with_spans(
            toolpath_with_n_moves(10),
            vec![
                Span::new(0, 10, SpanKind::Operation),
                Span::new(0, 5, SpanKind::DepthPass),
                Span::new(5, 10, SpanKind::DepthPass),
            ],
        );
        assert_eq!(at.spans_of_kind(SpanKind::DepthPass).count(), 2);
        assert_eq!(at.spans_of_kind(SpanKind::Operation).count(), 1);
        assert_eq!(at.spans_of_kind(SpanKind::Region).count(), 0);
    }

    // ── rapid_order_barriers ───────────────────────────────────────────

    #[test]
    fn rapid_order_barriers_combines_barriers_and_depth_pass_starts() {
        let at = AnnotatedToolpath::with_spans(
            toolpath_with_n_moves(20),
            vec![
                Span::new(0, 20, SpanKind::Operation),
                Span::new(0, 5, SpanKind::DepthPass),
                Span::new(5, 12, SpanKind::DepthPass),
                Span::new(12, 20, SpanKind::DepthPass),
                Span::boundary(8, SpanKind::RapidOrderBarrier),
            ],
        );
        let barriers = at.rapid_order_barriers();
        // DepthPass starts: 0, 5, 12; RapidOrderBarrier: 8 — sorted, deduped.
        assert_eq!(barriers, vec![0, 5, 8, 12]);
    }

    #[test]
    fn rapid_order_barriers_dedupes_overlapping_starts() {
        let at = AnnotatedToolpath::with_spans(
            toolpath_with_n_moves(10),
            vec![
                Span::new(0, 5, SpanKind::DepthPass),
                Span::boundary(0, SpanKind::RapidOrderBarrier),
                Span::boundary(5, SpanKind::RapidOrderBarrier),
                Span::new(5, 10, SpanKind::DepthPass),
            ],
        );
        assert_eq!(at.rapid_order_barriers(), vec![0, 5]);
    }

    #[test]
    fn rapid_order_barriers_empty_when_no_relevant_spans() {
        let at = AnnotatedToolpath::with_spans(
            toolpath_with_n_moves(10),
            vec![
                Span::new(0, 10, SpanKind::Operation),
                Span::new(0, 10, SpanKind::Region),
            ],
        );
        assert!(at.rapid_order_barriers().is_empty());
    }

    // ── check_invariants ───────────────────────────────────────────────

    #[test]
    fn check_invariants_passes_on_well_formed_spans() {
        let at = AnnotatedToolpath::with_spans(
            toolpath_with_n_moves(10),
            vec![
                Span::new(0, 10, SpanKind::Operation),
                Span::new(0, 5, SpanKind::DepthPass),
                Span::new(5, 10, SpanKind::DepthPass),
                Span::boundary(5, SpanKind::RapidOrderBarrier),
                // After-last barrier — allowed.
                Span::boundary(10, SpanKind::RapidOrderBarrier),
            ],
        );
        at.check_invariants()
            .expect("well-formed spans should validate");
    }

    #[test]
    fn check_invariants_passes_with_no_spans() {
        let at = AnnotatedToolpath::new(toolpath_with_n_moves(0));
        at.check_invariants().expect("empty is fine");
    }

    #[test]
    fn check_invariants_catches_inverted_range() {
        let at = AnnotatedToolpath::with_spans(
            toolpath_with_n_moves(10),
            vec![Span::new(7, 3, SpanKind::DepthPass)],
        );
        let err = at
            .check_invariants()
            .expect_err("inverted range should fail");
        assert!(matches!(
            err,
            SpanInvariantViolation::InvertedRange {
                index: 0,
                start: 7,
                end: 3,
            }
        ));
    }

    #[test]
    fn check_invariants_catches_out_of_bounds() {
        let at = AnnotatedToolpath::with_spans(
            toolpath_with_n_moves(5),
            vec![Span::new(0, 7, SpanKind::DepthPass)],
        );
        let err = at
            .check_invariants()
            .expect_err("out of bounds should fail");
        assert!(matches!(
            err,
            SpanInvariantViolation::OutOfBounds {
                index: 0,
                end: 7,
                n_moves: 5,
            }
        ));
    }

    #[test]
    fn check_invariants_catches_non_zero_width_barrier() {
        let at = AnnotatedToolpath::with_spans(
            toolpath_with_n_moves(10),
            vec![Span::new(2, 5, SpanKind::RapidOrderBarrier)],
        );
        let err = at
            .check_invariants()
            .expect_err("non-zero-width barrier should fail");
        assert!(matches!(
            err,
            SpanInvariantViolation::BarrierNotZeroWidth { index: 0, .. }
        ));
    }

    // ── MoveRemap ──────────────────────────────────────────────────────

    #[test]
    fn move_remap_identity() {
        let m = MoveRemap::identity(5);
        assert_eq!(m.old_to_new.len(), 5);
        assert_eq!(m.old_to_new[0], Some(0..1));
        assert_eq!(m.old_to_new[4], Some(4..5));
        assert_eq!(m.remap_range(1, 4), Some(1..4));
    }

    #[test]
    fn move_remap_with_drops() {
        // Old moves: [keep, DROP, keep, DROP, keep] → new indices [0, _, 1, _, 2]
        let m = MoveRemap {
            old_to_new: vec![Some(0..1), None, Some(1..2), None, Some(2..3)],
        };
        // Spans across the drops should remap to the surviving extent.
        assert_eq!(m.remap_range(0, 5), Some(0..3));
        assert_eq!(m.remap_range(0, 3), Some(0..2));
        assert_eq!(m.remap_range(2, 5), Some(1..3));
    }

    #[test]
    fn move_remap_returns_none_when_all_dropped() {
        let m = MoveRemap {
            old_to_new: vec![Some(0..1), None, None, None, Some(1..2)],
        };
        assert_eq!(m.remap_range(1, 4), None);
    }

    #[test]
    fn move_remap_with_inserts() {
        // Old move 1 expanded into new moves [1..4) — e.g. dogbone insertion.
        let m = MoveRemap {
            old_to_new: vec![Some(0..1), Some(1..4), Some(4..5)],
        };
        assert_eq!(m.remap_range(0, 3), Some(0..5));
        assert_eq!(m.remap_range(1, 2), Some(1..4));
    }

    #[test]
    fn move_remap_inverted_input_returns_none() {
        let m = MoveRemap::identity(5);
        assert_eq!(m.remap_range(4, 1), None);
    }

    #[test]
    fn move_remap_boundary_falls_through_drops() {
        // Old boundary at index 2 (between move 1 and move 2). If old move 2
        // was dropped, the boundary should advance to where old move 3 lives.
        let m = MoveRemap {
            old_to_new: vec![Some(0..1), Some(1..2), None, Some(2..3)],
        };
        assert_eq!(m.remap_boundary(2, 3), 2);
    }

    #[test]
    fn move_remap_boundary_past_end() {
        let m = MoveRemap::identity(3);
        // Boundary past the end of the old list lands at total_new_moves.
        assert_eq!(m.remap_boundary(5, 3), 3);
    }

    // ── MoveRemap::remap_span / remap_spans ────────────────────────────

    #[test]
    fn remap_span_fully_inside_kept_range_shifts_bounds() {
        // Old moves 2..3 dropped; everything else 1:1. Span 1..4 should
        // shrink to the surviving bounds.
        let m = MoveRemap {
            old_to_new: vec![
                Some(0..1),
                Some(1..2),
                None,
                Some(2..3),
                Some(3..4),
                Some(4..5),
            ],
        };
        let span = Span::new(1, 4, SpanKind::Region).with_label("r");
        let out = m.remap_span(&span, 5).expect("span partially survives");
        assert_eq!(out.start_move, 1);
        assert_eq!(out.end_move, 3);
        assert_eq!(out.kind, SpanKind::Region);
        assert_eq!(out.label, "r");
    }

    #[test]
    fn remap_span_partially_collapsed_bounds_to_survivors() {
        // Span 0..5 where moves 1..4 are dropped: only moves 0 and 4 survive,
        // so the remapped span covers just their new positions.
        let m = MoveRemap {
            old_to_new: vec![Some(0..1), None, None, None, Some(1..2)],
        };
        let span = Span::new(0, 5, SpanKind::DepthPass);
        let out = m.remap_span(&span, 2).expect("span partially survives");
        assert_eq!(out.start_move, 0);
        assert_eq!(out.end_move, 2);
    }

    #[test]
    fn remap_span_fully_collapsed_is_dropped() {
        let m = MoveRemap {
            old_to_new: vec![None, None, None],
        };
        let span = Span::new(0, 3, SpanKind::Region);
        assert_eq!(m.remap_span(&span, 0), None);
    }

    #[test]
    fn remap_span_boundary_stays_zero_width() {
        let m = MoveRemap {
            old_to_new: vec![Some(0..1), None, Some(1..2)],
        };
        let span = Span::boundary(1, SpanKind::RapidOrderBarrier);
        let out = m
            .remap_span(&span, 2)
            .expect("boundary spans never collapse");
        assert!(out.is_boundary());
        assert_eq!(out.start_move, 1);
    }

    #[test]
    fn remap_span_preserves_payload() {
        let m = MoveRemap::identity(3);
        let span = Span::new(0, 3, SpanKind::DepthPass).with_payload(SpanPayload::DepthPass {
            z_level: -2.0,
            pass_index: 1,
        });
        let out = m.remap_span(&span, 3).expect("identity keeps span");
        assert_eq!(out.payload, span.payload);
    }

    #[test]
    fn remap_spans_drops_fully_collapsed_and_preserves_order() {
        // Three spans: first fully inside kept moves, second fully
        // collapsed (should be dropped), third fully inside kept moves —
        // ordering of the survivors must match input order.
        let m = MoveRemap {
            old_to_new: vec![
                Some(0..1),
                Some(1..2),
                None, // moves 2..4 dropped entirely
                None,
                Some(2..3),
                Some(3..4),
            ],
        };
        let spans = vec![
            Span::new(0, 2, SpanKind::DepthPass).with_label("first"),
            Span::new(2, 4, SpanKind::Region).with_label("collapsed"),
            Span::new(4, 6, SpanKind::DepthPass).with_label("third"),
        ];
        let out = m.remap_spans(&spans, 4);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].label, "first");
        assert_eq!(out[0].start_move, 0);
        assert_eq!(out[0].end_move, 2);
        assert_eq!(out[1].label, "third");
        assert_eq!(out[1].start_move, 2);
        assert_eq!(out[1].end_move, 4);
    }
}
