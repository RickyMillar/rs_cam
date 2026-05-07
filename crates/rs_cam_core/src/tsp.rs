//! TSP (Traveling Salesman Problem) optimization for toolpath segment reordering.
//!
//! Reorders independent toolpath segments to minimize total rapid travel distance
//! using a nearest-neighbor heuristic followed by 2-opt improvement.

use std::ops::Range;

use crate::geo::P3;
use crate::toolpath::{Move, MoveType, Toolpath};
use crate::toolpath_spans::{AnnotatedToolpath, MoveRemap, Span, SpanKind};

/// A continuous sequence of cutting moves between rapids.
struct Segment {
    moves: Vec<Move>,
    start: P3,
    end: P3,
    /// Half-open range of move indices in the *input* toolpath that this
    /// segment's cutting moves came from. Rapids around the segment are
    /// not included — they are regenerated during reassembly.
    src_range: Range<usize>,
}

/// XY-plane distance between two 3D points (ignores Z).
fn xy_distance(a: &P3, b: &P3) -> f64 {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    (dx * dx + dy * dy).sqrt()
}

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// Split a toolpath into segments of consecutive non-rapid moves.
///
/// Each segment tracks the start and end positions of its cutting moves and
/// the half-open `[start..end)` range of input move indices it covers.
/// Rapids between segments are discarded (they will be regenerated).
fn split_into_segments(toolpath: &Toolpath) -> Vec<Segment> {
    let mut segments = Vec::new();
    let mut current_moves: Vec<Move> = Vec::new();
    let mut current_start_idx: Option<usize> = None;

    for (idx, m) in toolpath.moves.iter().enumerate() {
        match m.move_type {
            MoveType::Rapid => {
                if let (Some(first), Some(last), Some(src_start)) = (
                    current_moves.first(),
                    current_moves.last(),
                    current_start_idx,
                ) {
                    let start = first.target;
                    let end = last.target;
                    let n = current_moves.len();
                    segments.push(Segment {
                        moves: std::mem::take(&mut current_moves),
                        start,
                        end,
                        src_range: src_start..src_start + n,
                    });
                    current_start_idx = None;
                }
            }
            _ => {
                if current_start_idx.is_none() {
                    current_start_idx = Some(idx);
                }
                current_moves.push(m.clone());
            }
        }
    }

    if let (Some(first), Some(last), Some(src_start)) = (
        current_moves.first(),
        current_moves.last(),
        current_start_idx,
    ) {
        let start = first.target;
        let end = last.target;
        let n = current_moves.len();
        segments.push(Segment {
            moves: current_moves,
            start,
            end,
            src_range: src_start..src_start + n,
        });
    }

    segments
}

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// Total XY rapid travel distance for a given segment visitation order.
///
/// Measures the sum of XY distances from the end of each segment to the
/// start of the next segment in the order.
#[cfg(test)]
fn total_rapid_distance(order: &[usize], segments: &[Segment]) -> f64 {
    #[allow(clippy::indexing_slicing)]
    order.windows(2).fold(0.0, |dist, pair| {
        dist + xy_distance(&segments[pair[0]].end, &segments[pair[1]].start)
    })
}

/// Reorder independent toolpath cutting segments to minimize rapid travel,
/// honoring TSP barriers derived from the input's span annotations.
///
/// Barriers come from [`AnnotatedToolpath::rapid_order_barriers`]
/// (`RapidOrderBarrier` + `DepthPass` span starts). Reordering happens
/// independently within each barrier-delimited group; cuts on opposite sides
/// of a barrier are never swapped — this is the wanaka safety guarantee that
/// keeps depth-pass and lock-step ordering intact.
///
/// # Span remapping (Phase 3f / #55)
///
/// Each input span is rewritten through a [`MoveRemap`] reflecting the
/// permutation. An `Operation` span covering the whole toolpath survives
/// identical because the permutation is contained within the operation.
/// A `Region`/`DepthPass`/etc. span covering only some moves is remapped to
/// the new range its moves occupy.
///
/// **Known limitation:** if a non-`Operation` span's old move range maps to a
/// non-contiguous new range — i.e. the bounding range of the span's moves in
/// the new toolpath also contains foreign moves that came from outside the
/// span — the output's `spans_valid` flag is set to `false` and a
/// `tracing::warn!` is emitted. The span is preserved with its bounding
/// range so downstream code that honors `spans_valid` still has something
/// usable for diagnostics.
///
/// # Algorithm
///
/// 1. Derive barriers from the input spans (if any) and slice the toolpath
///    into barrier-delimited groups.
/// 2. Within each group: split into cutting segments, apply nearest-neighbor
///    + 2-opt, then reassemble with retract/rapid/plunge between segments.
/// 3. Build a `MoveRemap` describing where each old move ended up.
/// 4. Remap input spans through the permutation; flag `spans_valid = false`
///    if any non-`Operation` span fragmented across barriers / segments.
// SAFETY: all indexing in this function is bounded by `n` (segment count)
// and group_bounds, both built locally.
#[allow(clippy::indexing_slicing)]
pub fn optimize_rapid_order(annotated: AnnotatedToolpath, safe_z: f64) -> AnnotatedToolpath {
    let barriers = annotated.rapid_order_barriers();
    let AnnotatedToolpath {
        toolpath,
        spans,
        spans_valid: input_valid,
    } = annotated;

    if toolpath.moves.is_empty() {
        return AnnotatedToolpath {
            toolpath,
            spans,
            spans_valid: input_valid,
        };
    }

    // Build the per-group bounds in input-move coordinates. With no barriers
    // there is one group covering the whole toolpath. With barriers, the
    // groups are `[0..b0)`, `[b0..b1)`, ..., `[bN..moves.len())` after
    // dropping any `0` or out-of-range indices.
    let n_in = toolpath.moves.len();
    let mut starts: Vec<usize> = Vec::with_capacity(barriers.len() + 1);
    starts.push(0);
    for &b in &barriers {
        if b > 0 && b < n_in {
            starts.push(b);
        }
    }
    starts.sort_unstable();
    starts.dedup();

    let mut group_bounds: Vec<Range<usize>> = Vec::with_capacity(starts.len());
    for (i, &s) in starts.iter().enumerate() {
        let e = starts.get(i + 1).copied().unwrap_or(n_in);
        if s < e {
            group_bounds.push(s..e);
        }
    }

    // Optimize each group, accumulating the new toolpath and a per-old-move
    // remap entry. Each old cutting move maps to its new index; old rapids
    // remap to a zero-width slot at the group's new-output start (leading
    // rapids before any cuts) or end (trailing rapids), so spans covering
    // a whole group remap cleanly.
    let mut result = Toolpath::new();
    let mut old_to_new: Vec<Option<Range<usize>>> = vec![None; n_in];

    for group in &group_bounds {
        let group_new_start = result.moves.len();
        optimize_one_group(
            &toolpath,
            group.clone(),
            safe_z,
            &mut result,
            &mut old_to_new,
        );
        let group_new_end = result.moves.len();
        fill_group_rapids(
            &mut old_to_new,
            group.clone(),
            group_new_start,
            group_new_end,
        );
    }

    let new_n = result.moves.len();
    let remap = MoveRemap { old_to_new };

    let (new_spans, new_valid) = if input_valid {
        remap_spans(&spans, &remap, new_n)
    } else {
        (spans, false)
    };

    AnnotatedToolpath {
        toolpath: result,
        spans: new_spans,
        spans_valid: new_valid,
    }
}

/// Run the single-group nearest-neighbor + 2-opt over `[group_start..group_end)`
/// of the input, append the reordered moves to `result`, and update `old_to_new`
/// so each input cutting move points to its new index.
#[allow(clippy::indexing_slicing)]
fn optimize_one_group(
    toolpath: &Toolpath,
    group: Range<usize>,
    safe_z: f64,
    result: &mut Toolpath,
    old_to_new: &mut [Option<Range<usize>>],
) {
    let group_view = Toolpath {
        moves: toolpath.moves[group.clone()].to_vec(),
    };
    let mut segments = split_into_segments(&group_view);
    // Shift segment src_ranges back into input-toolpath coordinates.
    for s in &mut segments {
        s.src_range = s.src_range.start + group.start..s.src_range.end + group.start;
    }

    if segments.is_empty() {
        // Nothing cut — copy moves through verbatim so the move count stays
        // sensible. (In practice this is a group of pure rapids.)
        for idx in group {
            let dst = result.moves.len();
            result.moves.push(toolpath.moves[idx].clone());
            old_to_new[idx] = Some(dst..dst + 1);
        }
        return;
    }

    if segments.len() == 1 {
        // Single segment: no reordering. Append the moves as-is, including
        // any framing rapids from the input range.
        for idx in group {
            let dst = result.moves.len();
            result.moves.push(toolpath.moves[idx].clone());
            old_to_new[idx] = Some(dst..dst + 1);
        }
        return;
    }

    let order = run_tsp(&segments);
    rebuild_group(&segments, &order, safe_z, result, old_to_new);
}

/// Nearest-neighbor + 2-opt order computation over `segments`.
#[allow(clippy::indexing_slicing)]
fn run_tsp(segments: &[Segment]) -> Vec<usize> {
    let n = segments.len();
    let mut visited = vec![false; n];
    let mut order = Vec::with_capacity(n);

    order.push(0);
    visited[0] = true;

    for _ in 1..n {
        let current = order[order.len() - 1];
        let current_end = &segments[current].end;

        let mut best_idx = 0;
        let mut best_dist = f64::INFINITY;

        for j in 0..n {
            if visited[j] {
                continue;
            }
            let d = xy_distance(current_end, &segments[j].start);
            if d < best_dist {
                best_dist = d;
                best_idx = j;
            }
        }

        visited[best_idx] = true;
        order.push(best_idx);
    }

    // 2-opt is O(N²) per sweep and each reverse is O(N), so a full pass is
    // O(N³). With 100 iterations and N > ~500, this can hang the app for
    // hours. On very fragmented toolpaths the nearest-neighbor pass from
    // step 1 is already a strong solution — skip 2-opt beyond this threshold.
    const MAX_2OPT_SEGMENTS: usize = 500;
    if n > MAX_2OPT_SEGMENTS {
        tracing::info!(
            segments = n,
            "Skipping 2-opt pass: segment count exceeds safe threshold; nearest-neighbor order used"
        );
        return order;
    }
    let max_iterations = 100;
    for _ in 0..max_iterations {
        let mut improved = false;

        for i in 0..n.saturating_sub(1) {
            for j in (i + 2)..n {
                let cost_before =
                    xy_distance(&segments[order[i]].end, &segments[order[i + 1]].start)
                        + if j + 1 < n {
                            xy_distance(&segments[order[j]].end, &segments[order[j + 1]].start)
                        } else {
                            0.0
                        };

                let cost_after = xy_distance(&segments[order[i]].end, &segments[order[j]].end)
                    + if j + 1 < n {
                        xy_distance(&segments[order[i + 1]].start, &segments[order[j + 1]].start)
                    } else {
                        0.0
                    };

                if cost_after < cost_before - 1e-10 {
                    order[i + 1..=j].reverse();
                    improved = true;
                }
            }
        }

        if !improved {
            break;
        }
    }

    order
}

/// Append the segments in `order` (with retract/rapid/plunge interstitial
/// rapids) to `result`, recording each input cutting move's new slot in
/// `old_to_new`.
#[allow(clippy::indexing_slicing)]
fn rebuild_group(
    segments: &[Segment],
    order: &[usize],
    safe_z: f64,
    result: &mut Toolpath,
    old_to_new: &mut [Option<Range<usize>>],
) {
    for (idx, &seg_idx) in order.iter().enumerate() {
        let seg = &segments[seg_idx];

        if idx == 0 {
            result.rapid_to(P3::new(seg.start.x, seg.start.y, safe_z));
        } else {
            let prev_seg = &segments[order[idx - 1]];
            result.rapid_to(P3::new(prev_seg.end.x, prev_seg.end.y, safe_z));
            result.rapid_to(P3::new(seg.start.x, seg.start.y, safe_z));
        }

        let src_start = seg.src_range.start;
        for (k, m) in seg.moves.iter().enumerate() {
            let new_idx = result.moves.len();
            result.moves.push(m.clone());
            old_to_new[src_start + k] = Some(new_idx..new_idx + 1);
        }
    }

    if let Some(last_seg_idx) = order.last() {
        let last_seg = &segments[*last_seg_idx];
        result.rapid_to(P3::new(last_seg.end.x, last_seg.end.y, safe_z));
    }
}

/// Fill `None` slots within a group's input range with zero-width markers at
/// the appropriate edge of the group's output range. Leading rapids (before
/// the first surviving cut) map to `group_new_start..group_new_start`;
/// trailing rapids (after the last surviving cut) map to
/// `group_new_end..group_new_end`. Mid-group rapids that lie between
/// surviving cuts map to the next cut's slot.
#[allow(clippy::indexing_slicing)]
fn fill_group_rapids(
    old_to_new: &mut [Option<Range<usize>>],
    group: Range<usize>,
    group_new_start: usize,
    group_new_end: usize,
) {
    let first_surviving = (group.start..group.end).find(|&i| old_to_new[i].is_some());

    let Some(first) = first_surviving else {
        // No cuts in this group — entire input range maps to a zero-width
        // slot at group_new_start (which equals group_new_end here).
        for i in group {
            if old_to_new[i].is_none() {
                old_to_new[i] = Some(group_new_start..group_new_start);
            }
        }
        return;
    };

    // Leading rapids → group_new_start.
    for slot in old_to_new.iter_mut().take(first).skip(group.start) {
        if slot.is_none() {
            *slot = Some(group_new_start..group_new_start);
        }
    }

    // Walk forward from `first` filling None entries with the *next*
    // surviving cut's start slot, or `group_new_end` if no further survivor.
    let mut next_slot = group_new_end;
    for i in (first..group.end).rev() {
        match &old_to_new[i] {
            Some(r) => next_slot = r.start,
            None => old_to_new[i] = Some(next_slot..next_slot),
        }
    }
}

/// Remap each input span through the permutation. Returns the new spans and
/// a `spans_valid` flag — `false` if any non-`Operation` span fragmented
/// (foreign moves intruded into its new bounding range).
#[allow(clippy::indexing_slicing)]
fn remap_spans(spans: &[Span], remap: &MoveRemap, new_n: usize) -> (Vec<Span>, bool) {
    let mut out: Vec<Span> = Vec::with_capacity(spans.len());
    let mut valid = true;

    for s in spans {
        let payload = s.payload.clone();
        let label = s.label.clone();

        let new_span = if s.is_boundary() {
            let new_pos = remap.remap_boundary(s.start_move, new_n);
            Span::new(new_pos, new_pos, s.kind)
        } else {
            // Bounding remap — the min..max of where the old moves landed.
            let Some(bounds) = remap.remap_range(s.start_move, s.end_move) else {
                // Every move in the span got dropped — drop the span too.
                continue;
            };

            // Foreign-intrusion check: any old move *outside* the span that
            // non-trivially overlaps `bounds` means the span's contents got
            // interleaved with foreign moves by the reorder. Operation
            // spans are exempt because the permutation is internal to them.
            let foreign_intrusion = remap
                .old_to_new
                .iter()
                .enumerate()
                .filter(|(i, _)| *i < s.start_move || *i >= s.end_move)
                .any(|(_, slot)| {
                    slot.as_ref().is_some_and(|r| {
                        // Non-trivial overlap: the slot covers an actual
                        // new-move index (start < end) AND that index is
                        // inside `bounds`.
                        r.start < r.end && r.start < bounds.end && r.end > bounds.start
                    })
                });

            if foreign_intrusion && s.kind != SpanKind::Operation {
                tracing::warn!(
                    span_kind = ?s.kind,
                    span_label = %s.label,
                    old_range = ?(s.start_move..s.end_move),
                    new_bounds = ?bounds,
                    "TSP rapid-order optimization split a non-Operation span; \
                     marking spans_valid = false"
                );
                valid = false;
            }

            Span::new(bounds.start, bounds.end, s.kind)
        };

        let new_span = new_span.with_label(label);
        let new_span = if let Some(p) = payload {
            new_span.with_payload(p)
        } else {
            new_span
        };
        out.push(new_span);
    }

    (out, valid)
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
    use crate::toolpath::MoveType;
    use crate::toolpath_spans::{Span, SpanKind};

    fn make_segment_toolpath(points: &[P3], safe_z: f64, feed_rate: f64) -> Vec<Move> {
        let mut moves = Vec::new();
        let (Some(first), Some(last)) = (points.first(), points.last()) else {
            return moves;
        };
        moves.push(Move {
            target: P3::new(first.x, first.y, safe_z),
            move_type: MoveType::Rapid,
        });
        for p in points {
            moves.push(Move {
                target: *p,
                move_type: MoveType::Linear { feed_rate },
            });
        }
        moves.push(Move {
            target: P3::new(last.x, last.y, safe_z),
            move_type: MoveType::Rapid,
        });
        moves
    }

    fn opt_unannotated(tp: &Toolpath, safe_z: f64) -> Toolpath {
        optimize_rapid_order(AnnotatedToolpath::new(tp.clone()), safe_z).toolpath
    }

    #[test]
    fn test_four_segments_square_optimizer_improves() {
        let safe_z = 10.0;
        let feed = 1000.0;

        let mut tp = Toolpath::new();
        tp.moves.extend(make_segment_toolpath(
            &[P3::new(0.0, 0.0, -1.0), P3::new(10.0, 0.0, -1.0)],
            safe_z,
            feed,
        ));
        tp.moves.extend(make_segment_toolpath(
            &[P3::new(100.0, 100.0, -1.0), P3::new(110.0, 100.0, -1.0)],
            safe_z,
            feed,
        ));
        tp.moves.extend(make_segment_toolpath(
            &[P3::new(12.0, 0.0, -1.0), P3::new(20.0, 0.0, -1.0)],
            safe_z,
            feed,
        ));
        tp.moves.extend(make_segment_toolpath(
            &[P3::new(100.0, 0.0, -1.0), P3::new(110.0, 0.0, -1.0)],
            safe_z,
            feed,
        ));

        let optimized = opt_unannotated(&tp, safe_z);

        let original_rapid = tp.total_rapid_distance();
        let optimized_rapid = optimized.total_rapid_distance();

        assert!(
            optimized_rapid < original_rapid,
            "Optimized rapid distance ({:.2}) should be less than original ({:.2})",
            optimized_rapid,
            original_rapid,
        );

        let cutting_count = optimized
            .moves
            .iter()
            .filter(|m| matches!(m.move_type, MoveType::Linear { .. }))
            .count();
        assert_eq!(cutting_count, 8, "All cutting moves must be preserved");
    }

    #[test]
    fn test_single_segment_unchanged() {
        let safe_z = 10.0;
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, safe_z));
        tp.feed_to(P3::new(0.0, 0.0, -1.0), 500.0);
        tp.feed_to(P3::new(10.0, 0.0, -1.0), 1000.0);
        tp.rapid_to(P3::new(10.0, 0.0, safe_z));

        let optimized = opt_unannotated(&tp, safe_z);

        let orig_cutting: Vec<_> = tp
            .moves
            .iter()
            .filter(|m| !matches!(m.move_type, MoveType::Rapid))
            .collect();
        let opt_cutting: Vec<_> = optimized
            .moves
            .iter()
            .filter(|m| !matches!(m.move_type, MoveType::Rapid))
            .collect();
        assert_eq!(orig_cutting.len(), opt_cutting.len());
        for (a, b) in orig_cutting.iter().zip(opt_cutting.iter()) {
            assert!(
                (a.target - b.target).norm() < 1e-10,
                "Cutting moves should be identical"
            );
        }
    }

    #[test]
    fn test_barriers_prevent_cross_group_reordering() {
        let safe_z = 10.0;
        let feed = 1000.0;
        let mut tp = Toolpath::new();

        tp.moves.extend(make_segment_toolpath(
            &[P3::new(0.0, 0.0, 0.0), P3::new(1.0, 0.0, 0.0)],
            safe_z,
            feed,
        ));
        tp.moves.extend(make_segment_toolpath(
            &[P3::new(100.0, 0.0, 0.0), P3::new(101.0, 0.0, 0.0)],
            safe_z,
            feed,
        ));
        let group_2_start = tp.moves.len();
        tp.moves.extend(make_segment_toolpath(
            &[P3::new(2.0, 0.0, -7.0), P3::new(3.0, 0.0, -7.0)],
            safe_z,
            feed,
        ));
        tp.moves.extend(make_segment_toolpath(
            &[P3::new(4.0, 0.0, -7.0), P3::new(5.0, 0.0, -7.0)],
            safe_z,
            feed,
        ));

        let global = opt_unannotated(&tp, safe_z);
        let n = tp.moves.len();
        let annotated = AnnotatedToolpath::with_spans(
            tp.clone(),
            vec![
                Span::new(0, group_2_start, SpanKind::DepthPass),
                Span::new(group_2_start, n, SpanKind::DepthPass),
            ],
        );
        let barriered = optimize_rapid_order(annotated, safe_z).toolpath;

        let global_cut_z: Vec<f64> = global
            .moves
            .iter()
            .filter(|m| m.move_type.is_cutting())
            .map(|m| m.target.z)
            .collect();
        assert_eq!(
            global_cut_z[2], -7.0,
            "unbarriered TSP should cross the depth boundary in this fixture"
        );

        let barriered_cut_z: Vec<f64> = barriered
            .moves
            .iter()
            .filter(|m| m.move_type.is_cutting())
            .map(|m| m.target.z)
            .collect();
        assert_eq!(&barriered_cut_z[0..4], &[0.0, 0.0, 0.0, 0.0]);
        assert_eq!(&barriered_cut_z[4..], &[-7.0, -7.0, -7.0, -7.0]);
    }

    #[test]
    fn test_empty_toolpath_unchanged() {
        let tp = Toolpath::new();
        let optimized = opt_unannotated(&tp, 10.0);
        assert!(
            optimized.moves.is_empty(),
            "Empty toolpath should produce empty result"
        );
    }

    #[test]
    fn test_split_into_segments() {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 10.0));
        tp.feed_to(P3::new(0.0, 0.0, -1.0), 500.0);
        tp.feed_to(P3::new(5.0, 0.0, -1.0), 1000.0);
        tp.rapid_to(P3::new(5.0, 0.0, 10.0));
        tp.rapid_to(P3::new(20.0, 0.0, 10.0));
        tp.feed_to(P3::new(20.0, 0.0, -1.0), 500.0);
        tp.feed_to(P3::new(25.0, 0.0, -1.0), 1000.0);
        tp.rapid_to(P3::new(25.0, 0.0, 10.0));

        let segments = split_into_segments(&tp);
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].moves.len(), 2);
        assert_eq!(segments[1].moves.len(), 2);
        assert!((segments[0].start.x - 0.0).abs() < 1e-10);
        assert!((segments[0].end.x - 5.0).abs() < 1e-10);
        assert!((segments[1].start.x - 20.0).abs() < 1e-10);
        assert!((segments[1].end.x - 25.0).abs() < 1e-10);
    }

    #[test]
    fn test_xy_distance() {
        let a = P3::new(0.0, 0.0, -5.0);
        let b = P3::new(3.0, 4.0, 100.0);
        let d = xy_distance(&a, &b);
        assert!(
            (d - 5.0).abs() < 1e-10,
            "XY distance should be 5.0, got {}",
            d
        );
    }

    #[test]
    fn test_total_rapid_distance_helper() {
        let segments = vec![
            Segment {
                moves: vec![],
                start: P3::new(0.0, 0.0, 0.0),
                end: P3::new(10.0, 0.0, 0.0),
                src_range: 0..0,
            },
            Segment {
                moves: vec![],
                start: P3::new(10.0, 10.0, 0.0),
                end: P3::new(20.0, 10.0, 0.0),
                src_range: 0..0,
            },
            Segment {
                moves: vec![],
                start: P3::new(20.0, 0.0, 0.0),
                end: P3::new(30.0, 0.0, 0.0),
                src_range: 0..0,
            },
        ];

        let order = vec![0, 1, 2];
        let dist = total_rapid_distance(&order, &segments);
        assert!((dist - 20.0).abs() < 1e-10, "Expected 20.0, got {}", dist);
    }

    // ── Span-aware behavior (Phase 3f / #55) ─────────────────────────────

    #[test]
    fn optimize_rapid_order_derives_barriers_from_spans() {
        // Verifies barriers are derived from DepthPass span starts (no
        // explicit RapidOrderBarrier). Cuts below the barrier (Z=-7) must
        // stay below the cuts above (Z=0) after reorder.
        let safe_z = 10.0;
        let feed = 1000.0;
        let mut tp = Toolpath::new();

        tp.moves.extend(make_segment_toolpath(
            &[P3::new(0.0, 0.0, 0.0), P3::new(1.0, 0.0, 0.0)],
            safe_z,
            feed,
        ));
        tp.moves.extend(make_segment_toolpath(
            &[P3::new(100.0, 0.0, 0.0), P3::new(101.0, 0.0, 0.0)],
            safe_z,
            feed,
        ));
        let group_2_start = tp.moves.len();
        tp.moves.extend(make_segment_toolpath(
            &[P3::new(2.0, 0.0, -7.0), P3::new(3.0, 0.0, -7.0)],
            safe_z,
            feed,
        ));
        tp.moves.extend(make_segment_toolpath(
            &[P3::new(4.0, 0.0, -7.0), P3::new(5.0, 0.0, -7.0)],
            safe_z,
            feed,
        ));

        let n = tp.moves.len();
        let annotated = AnnotatedToolpath::with_spans(
            tp,
            vec![
                Span::new(0, n, SpanKind::Operation),
                Span::new(0, group_2_start, SpanKind::DepthPass),
                Span::new(group_2_start, n, SpanKind::DepthPass),
            ],
        );
        let result = optimize_rapid_order(annotated, safe_z);

        let cut_z: Vec<f64> = result
            .toolpath
            .moves
            .iter()
            .filter(|m| m.move_type.is_cutting())
            .map(|m| m.target.z)
            .collect();
        assert_eq!(&cut_z[0..4], &[0.0, 0.0, 0.0, 0.0]);
        assert_eq!(&cut_z[4..], &[-7.0, -7.0, -7.0, -7.0]);
    }

    #[test]
    fn optimize_rapid_order_remaps_operation_span() {
        // A trivial Operation span over the whole toolpath should survive
        // with end_move equal to the new toolpath length, and spans_valid
        // should remain true.
        let safe_z = 10.0;
        let feed = 1000.0;
        let mut tp = Toolpath::new();
        tp.moves.extend(make_segment_toolpath(
            &[P3::new(0.0, 0.0, -1.0), P3::new(10.0, 0.0, -1.0)],
            safe_z,
            feed,
        ));
        tp.moves.extend(make_segment_toolpath(
            &[P3::new(50.0, 0.0, -1.0), P3::new(60.0, 0.0, -1.0)],
            safe_z,
            feed,
        ));
        let n = tp.moves.len();
        let annotated =
            AnnotatedToolpath::with_spans(tp, vec![Span::new(0, n, SpanKind::Operation)]);

        let result = optimize_rapid_order(annotated, safe_z);

        let op = result
            .spans
            .iter()
            .find(|s| s.kind == SpanKind::Operation)
            .expect("Operation span must survive");
        assert_eq!(op.start_move, 0);
        assert_eq!(op.end_move, result.toolpath.moves.len());
        assert!(
            result.spans_valid,
            "Operation-only spans must round-trip cleanly"
        );
        result
            .check_invariants()
            .expect("remapped spans satisfy invariants");
    }

    #[test]
    fn optimize_rapid_order_invalidates_on_split() {
        // Build four segments so that NN/2-opt picks the order
        // [seg0, seg2, seg1, seg3] (proximity grouping):
        //   seg0 cuts at X=0, seg1 at X=50 (far), seg2 at X=2 (next to seg0),
        //   seg3 at X=52 (next to seg1).
        // A Region span covers input range [0..seg2_start), i.e. seg0+seg1.
        // After reorder, seg2 (which is OUTSIDE the span) lands between
        // seg0 and seg1 in the output → seg2's new slot intrudes into the
        // span's bounding range → spans_valid must flip to false.
        let safe_z = 10.0;
        let feed = 1000.0;
        let mut tp = Toolpath::new();
        // seg0 cuts at X=0
        tp.moves.extend(make_segment_toolpath(
            &[P3::new(0.0, 0.0, -1.0), P3::new(1.0, 0.0, -1.0)],
            safe_z,
            feed,
        ));
        // seg1 cuts at X=50 (far from seg0)
        tp.moves.extend(make_segment_toolpath(
            &[P3::new(50.0, 0.0, -1.0), P3::new(51.0, 0.0, -1.0)],
            safe_z,
            feed,
        ));
        let seg2_start = tp.moves.len();
        // seg2 cuts at X=2 (next to seg0)
        tp.moves.extend(make_segment_toolpath(
            &[P3::new(2.0, 0.0, -1.0), P3::new(3.0, 0.0, -1.0)],
            safe_z,
            feed,
        ));
        // seg3 cuts at X=52 (next to seg1)
        tp.moves.extend(make_segment_toolpath(
            &[P3::new(52.0, 0.0, -1.0), P3::new(53.0, 0.0, -1.0)],
            safe_z,
            feed,
        ));

        let annotated = AnnotatedToolpath::with_spans(
            tp.clone(),
            vec![Span::new(0, seg2_start, SpanKind::Region).with_label("seg0+seg1")],
        );
        let result = optimize_rapid_order(annotated, safe_z);

        let cut_x: Vec<f64> = result
            .toolpath
            .moves
            .iter()
            .filter(|m| m.move_type.is_cutting())
            .map(|m| m.target.x)
            .collect();
        let original_cut_x: Vec<f64> = tp
            .moves
            .iter()
            .filter(|m| m.move_type.is_cutting())
            .map(|m| m.target.x)
            .collect();
        assert_ne!(cut_x, original_cut_x, "TSP should have reordered the cuts");

        assert!(
            !result.spans_valid,
            "Region span split by foreign-move intrusion must invalidate spans"
        );
        result
            .check_invariants()
            .expect("remapped spans still satisfy structural invariants");
    }
}
