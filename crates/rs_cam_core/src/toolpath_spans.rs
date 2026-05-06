//! Toolpath spans — semantic ranges of moves inside a Toolpath.
//!
//! See `architecture/toolpath_spans.md` for design background. This module
//! contributes only the types; wiring into `compute::execute` and dressups
//! arrives in subsequent phases.

use std::borrow::Cow;
use std::ops::Range;

use crate::toolpath::Toolpath;

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
}

impl AnnotatedToolpath {
    /// Wrap a toolpath with no spans (still considered valid — there's just
    /// nothing to invalidate).
    pub fn new(toolpath: Toolpath) -> Self {
        Self {
            toolpath,
            spans: Vec::new(),
            spans_valid: true,
        }
    }

    pub fn with_spans(toolpath: Toolpath, spans: Vec<Span>) -> Self {
        Self {
            toolpath,
            spans,
            spans_valid: true,
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
}
