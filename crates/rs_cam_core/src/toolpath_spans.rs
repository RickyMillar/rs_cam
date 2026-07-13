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

// ── SpanPayload ─────────────────────────────────────────────────────────

/// Optional structured payload for span-specific data.
///
/// Most spans don't need a payload. Variants are added as concrete consumers
/// require them — keep this set minimal until phase-3 dressups demand more.
#[derive(Debug, Clone, PartialEq)]
pub enum SpanPayload {
    DepthPass { z_level: f64, pass_index: u32 },
    Region { region_id: u32 },
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
    /// [`Self::remap_span`] and layers that check on top as a distinct
    /// post-filter rather than reimplementing this method.
    pub fn remap_spans(&self, spans: &[Span], new_n_moves: usize) -> Vec<Span> {
        spans
            .iter()
            .filter_map(|s| self.remap_span(s, new_n_moves))
            .collect()
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
            .with_payload(SpanPayload::Region { region_id: 3 });
        assert_eq!(s.label, "region-3");
        assert_eq!(s.payload, Some(SpanPayload::Region { region_id: 3 }));
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
