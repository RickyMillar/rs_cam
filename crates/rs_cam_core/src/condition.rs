//! Toolpath conditioning — merge dense linear cut runs into fewer, longer
//! segments so a low-acceleration controller can actually reach the commanded
//! feed.
//!
//! `planning/ACCEL_FRIENDLY_TOOLPATHS_2026-06-20.md` (Phase 1). A move must be
//! at least `F²/(2A)` long for the planner to ramp from rest to feed `F` at
//! acceleration `A`; the adaptive spiral and 3D-rough paths emit sub-millimetre
//! segments that top out far below `F` and starve GRBL's 16-block look-ahead.
//! This pass RDP-simplifies each maximal run of consecutive same-feed **linear
//! cut moves** at a conditioning tolerance (≥ the generation tolerance, and ≤
//! stock-to-leave for roughing), collapsing near-collinear short segments while
//! keeping every dropped point within `tolerance` of the retained chord.
//!
//! Runs AFTER arc-fitting (curved runs become G2/G3 first; this cleans up the
//! residual linear segments) and is span-aware exactly like
//! [`crate::arcfit::fit_arcs`]: it never merges across a `RapidOrderBarrier` or
//! `DepthPass` boundary, tags nothing new, and remaps input spans through the
//! N-to-M collapse. When `spans_valid` is `false`, spans pass through untouched.

use crate::geo::P3;
use crate::toolpath::{MoveType, Toolpath, simplify_path_3d_keep_mask};
use crate::toolpath_spans::{AnnotatedToolpath, MoveRemap};
use std::collections::BTreeSet;
use std::ops::Range;

/// Two feed rates within this many mm/min are treated as the same run.
///
/// Shared with [`crate::arcfit`], which groups consecutive linear moves by
/// the same same-feed criterion before fitting arcs.
pub(crate) const FEED_EPS: f64 = 1e-6;

/// Merge runs of consecutive same-feed linear cut moves, dropping points whose
/// removal keeps the path within `tolerance` (mm) of the retained chord.
///
/// `tolerance <= 0` (or a degenerate toolpath) is a no-op passthrough.
#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
pub fn merge_linear_runs(annotated: AnnotatedToolpath, tolerance: f64) -> AnnotatedToolpath {
    // Barriers we must not merge across — same set arc-fit honours. A barrier
    // at index `b` sits before `moves[b]`; a run that included `b` would erase
    // the boundary between `b-1` and `b`.
    let barriers: BTreeSet<usize> = if annotated.spans_valid {
        annotated.rapid_order_barriers().into_iter().collect()
    } else {
        BTreeSet::new()
    };

    let AnnotatedToolpath {
        toolpath,
        spans,
        spans_valid,
        planner_engagement,
        rest_grid,
        rest_regions,
    } = annotated;

    if toolpath.moves.len() < 2 || tolerance <= 0.0 {
        return AnnotatedToolpath {
            toolpath,
            spans,
            spans_valid,
            planner_engagement,
            rest_grid,
            rest_regions,
        };
    }

    let moves = &toolpath.moves;

    let mut result = Toolpath::new();
    let mut old_to_new: Vec<Option<Range<usize>>> = Vec::with_capacity(moves.len());

    let mut i = 0;
    while i < moves.len() {
        let MoveType::Linear { feed_rate } = moves[i].move_type else {
            let n = result.moves.len();
            result.moves.push(moves[i].clone());
            old_to_new.push(Some(n..n + 1));
            i += 1;
            continue;
        };

        // Extend a maximal run of consecutive same-feed linear moves, stopping
        // before any barrier.
        let run_start = i;
        let mut end = run_start + 1;
        while end < moves.len() {
            match moves[end].move_type {
                MoveType::Linear { feed_rate: f }
                    if (f - feed_rate).abs() < FEED_EPS && !barriers.contains(&end) =>
                {
                    end += 1;
                }
                _ => break,
            }
        }

        let run_len = end - run_start;
        if run_len < 2 {
            let n = result.moves.len();
            result.moves.push(moves[run_start].clone());
            old_to_new.push(Some(n..n + 1));
            i = end;
            continue;
        }

        // Build the polyline to simplify: the previous move's target anchors
        // the run geometrically (so the first run segment can be merged), then
        // each run move's target. RDP always keeps the first and last points,
        // so the last run move is always retained — every dropped move has a
        // later kept move to fold into.
        let anchor = if run_start > 0 {
            Some(moves[run_start - 1].target)
        } else {
            None
        };
        let offset = usize::from(anchor.is_some());
        let mut pts: Vec<P3> = Vec::with_capacity(run_len + offset);
        if let Some(a) = anchor {
            pts.push(a);
        }
        for m in &moves[run_start..end] {
            pts.push(m.target);
        }

        let mask = simplify_path_3d_keep_mask(&pts, tolerance);
        let kept: Vec<bool> = (0..run_len).map(|r| mask[r + offset]).collect();
        let kept_positions: Vec<usize> = (0..run_len).filter(|&r| kept[r]).collect();

        // New result index assigned to each run move. The m-th kept move lands
        // at `base + m`; a dropped move folds into the next kept move (which
        // always exists because the last run move is kept).
        let base = result.moves.len();
        let mut m_ptr = 0usize;
        for r in 0..run_len {
            while m_ptr < kept_positions.len() && kept_positions[m_ptr] < r {
                m_ptr += 1;
            }
            let new_idx = base + m_ptr;
            if kept[r] {
                result.moves.push(moves[run_start + r].clone());
            }
            old_to_new.push(Some(new_idx..new_idx + 1));
        }

        i = end;
    }

    let new_n_moves = result.moves.len();
    let new_spans = if spans_valid {
        let remap = MoveRemap { old_to_new };
        remap.remap_spans(&spans, new_n_moves)
    } else {
        spans
    };

    AnnotatedToolpath {
        toolpath: result,
        spans: new_spans,
        spans_valid,
        planner_engagement,
        rest_grid,
        rest_regions,
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;
    use crate::toolpath::Toolpath;
    use crate::toolpath_spans::{Span, SpanKind};

    fn cut_move_count(tp: &Toolpath) -> usize {
        tp.moves
            .iter()
            .filter(|m| matches!(m.move_type, MoveType::Linear { .. }))
            .count()
    }

    /// A dense, nearly-straight run collapses to a single segment when every
    /// interior point is within tolerance of the chord.
    #[test]
    fn merges_collinear_dense_run() {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 0.0));
        // 11 points along y≈0 with sub-tolerance jitter.
        for k in 1..=10 {
            let x = k as f64;
            let y = if k % 2 == 0 { 0.001 } else { -0.001 };
            tp.feed_to(P3::new(x, y, 0.0), 1000.0);
        }
        let before = cut_move_count(&tp);
        let out = merge_linear_runs(AnnotatedToolpath::new(tp), 0.05);
        let after = cut_move_count(&out.toolpath);
        assert!(before == 10);
        assert!(after < before, "expected merge, got {after} (was {before})");
        // The retained endpoint must still be the true end of the run.
        let last = out.toolpath.moves.last().unwrap();
        assert!((last.target.x - 10.0).abs() < 1e-9);
    }

    /// A genuine corner is preserved — merging away the vertex would exceed
    /// tolerance, so the vertex survives.
    #[test]
    fn preserves_corner_beyond_tolerance() {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 0.0));
        tp.feed_to(P3::new(10.0, 0.0, 0.0), 1000.0);
        tp.feed_to(P3::new(10.0, 10.0, 0.0), 1000.0); // sharp 90° corner
        tp.feed_to(P3::new(20.0, 10.0, 0.0), 1000.0);
        let out = merge_linear_runs(AnnotatedToolpath::new(tp), 0.1);
        // The corner vertex (10,10) must remain.
        assert!(
            out.toolpath
                .moves
                .iter()
                .any(|m| (m.target.x - 10.0).abs() < 1e-9 && (m.target.y - 10.0).abs() < 1e-9),
            "sharp corner must be preserved"
        );
    }

    /// Tolerance ≤ 0 is a passthrough.
    #[test]
    fn zero_tolerance_is_noop() {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 0.0));
        for k in 1..=5 {
            tp.feed_to(P3::new(k as f64, 0.0, 0.0), 1000.0);
        }
        let n_in = tp.moves.len();
        let out = merge_linear_runs(AnnotatedToolpath::new(tp), 0.0);
        assert_eq!(out.toolpath.moves.len(), n_in);
    }

    /// Runs at different feeds are not merged across the feed change.
    #[test]
    fn does_not_merge_across_feed_change() {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 0.0));
        tp.feed_to(P3::new(1.0, 0.0, 0.0), 1000.0);
        tp.feed_to(P3::new(2.0, 0.0, 0.0), 1000.0);
        tp.feed_to(P3::new(3.0, 0.0, 0.0), 500.0); // feed change — must survive
        tp.feed_to(P3::new(4.0, 0.0, 0.0), 500.0);
        let out = merge_linear_runs(AnnotatedToolpath::new(tp), 0.1);
        // The two feeds form separate runs and are never merged together: a
        // 1000-feed cut move and a 500-feed cut move both survive (within each
        // run, collinear interior points may collapse — e.g. x=3 folds into
        // x=4 — but the feed boundary itself is preserved).
        let feeds: Vec<f64> = out
            .toolpath
            .moves
            .iter()
            .filter_map(|m| match m.move_type {
                MoveType::Linear { feed_rate } => Some(feed_rate),
                _ => None,
            })
            .collect();
        assert!(
            feeds.iter().any(|f| (f - 1000.0).abs() < 1e-9),
            "1000-feed run must survive"
        );
        assert!(
            feeds.iter().any(|f| (f - 500.0).abs() < 1e-9),
            "500-feed run must survive (feed boundary preserved)"
        );
        // The final move is the 500-feed run end at x=4.
        let last = out.toolpath.moves.last().unwrap();
        assert!((last.target.x - 4.0).abs() < 1e-9);
        assert!(
            matches!(last.move_type, MoveType::Linear { feed_rate } if (feed_rate - 500.0).abs() < 1e-9)
        );
    }

    /// Spans remap cleanly and invariants hold after a merge.
    #[test]
    fn remaps_spans_and_holds_invariants() {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 0.0));
        for k in 1..=10 {
            let y = if k % 2 == 0 { 0.001 } else { -0.001 };
            tp.feed_to(P3::new(k as f64, y, 0.0), 1000.0);
        }
        let n_in = tp.moves.len();
        let spans = vec![Span::new(0, n_in, SpanKind::Operation)];
        let out = merge_linear_runs(AnnotatedToolpath::with_spans(tp, spans), 0.05);
        let op = out
            .spans
            .iter()
            .find(|s| s.kind == SpanKind::Operation)
            .expect("Operation span survives");
        assert_eq!(op.start_move, 0);
        assert_eq!(op.end_move, out.toolpath.moves.len());
        assert!(out.spans_valid);
        out.check_invariants().expect("post-merge spans valid");
    }

    /// A DepthPass / RapidOrderBarrier boundary is never merged across.
    #[test]
    fn honors_barrier() {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 0.0));
        for k in 1..=8 {
            tp.feed_to(P3::new(k as f64, 0.0, 0.0), 1000.0);
        }
        let n_in = tp.moves.len();
        let mid = 4;
        let spans = vec![
            Span::new(0, n_in, SpanKind::Operation),
            Span::boundary(mid, SpanKind::RapidOrderBarrier),
        ];
        let out = merge_linear_runs(AnnotatedToolpath::with_spans(tp, spans), 0.1);
        let barriers: Vec<&Span> = out
            .spans
            .iter()
            .filter(|s| s.kind == SpanKind::RapidOrderBarrier)
            .collect();
        assert_eq!(barriers.len(), 1, "barrier preserved");
        assert!(barriers[0].is_boundary());
        out.check_invariants().expect("post-merge spans valid");
    }
}
