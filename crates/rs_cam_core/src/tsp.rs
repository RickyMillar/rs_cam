//! TSP (Traveling Salesman Problem) optimization for toolpath segment reordering.
//!
//! Reorders independent toolpath segments to minimize total rapid travel distance
//! using a nearest-neighbor heuristic followed by 2-opt improvement.

use std::ops::Range;

use crate::geo::P3;
use crate::toolpath::{Move, MoveIntent, MoveType, Toolpath};
use crate::toolpath_spans::{AnnotatedToolpath, MoveRemap, RemapIndex, Span, SpanKind};
use crate::transform_provenance::{ReconcileSet, Transformed};

/// Z tolerance for deciding whether a rapid reaches the group-framing
/// ceiling. Both sides of that comparison are heights derived from the
/// same [`crate::compute::config::HeightContext`], so this only has to
/// absorb accumulated `f64` noise — it is not a clearance margin.
const CEILING_EPS: f64 = 1e-9;

/// One atomic unit of the reorder: a run of moves the pass may relocate as
/// a whole, but must never take apart.
///
/// Historically this was exactly "a run of consecutive non-rapid moves",
/// because every rapid was assumed to be framing. Under C1 a segment can
/// also carry the operation's OWN internal linking rapids — see
/// [`split_into_segments`].
struct Segment {
    moves: Vec<Move>,
    start: P3,
    end: P3,
    /// Half-open range of move indices in the *input* toolpath that this
    /// segment's moves came from. Framing rapids around the segment are
    /// not included — they are regenerated during reassembly.
    src_range: Range<usize>,
}

/// XY-plane distance between two 3D points (ignores Z).
fn xy_distance(a: &P3, b: &P3) -> f64 {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    (dx * dx + dy * dy).sqrt()
}

/// Close the run accumulated so far into a [`Segment`], or drop it.
///
/// A run that holds no cutting move at all is not a segment: there is
/// nothing to reorder and nothing to hang a remap entry on. That can only
/// happen with C1 preservation on (a stretch of internal linking rapids
/// with no cut between them), and such a run is dropped exactly the way a
/// framing rapid is — its input indices stay unmapped and
/// [`fill_group_rapids`] gives them a zero-width slot.
fn flush_segment(
    segments: &mut Vec<Segment>,
    current_moves: &mut Vec<Move>,
    current_start_idx: &mut Option<usize>,
) {
    let Some(src_start) = current_start_idx.take() else {
        return;
    };
    let has_cut = current_moves
        .iter()
        .any(|m| !matches!(m.move_type, MoveType::Rapid));
    if !has_cut {
        current_moves.clear();
        return;
    }
    let (Some(first), Some(last)) = (current_moves.first(), current_moves.last()) else {
        return;
    };
    let start = first.target;
    let end = last.target;
    let n = current_moves.len();
    segments.push(Segment {
        moves: std::mem::take(current_moves),
        start,
        end,
        src_range: src_start..src_start + n,
    });
}

/// Split a toolpath into the segments the reorder may permute.
///
/// Each segment tracks the start and end positions of its moves and the
/// half-open `[start..end)` range of input move indices it covers. The
/// rapids that *frame* segments are discarded — they will be regenerated
/// at `safe_z` by [`rebuild_group`].
///
/// # `internal_link_ceiling_z` — the C1 dial
///
/// `None` is the historical rule: **every** rapid frames a segment, so a
/// segment is a run of consecutive non-rapid moves and every rapid in the
/// group is thrown away and re-invented from one scalar.
///
/// `Some(c)` says the operation links its own cutting moves *below* `c`,
/// and those links are not the reorderer's to re-plan. A rapid whose
/// target sits below `c` then stays **inside** the segment and is carried
/// through verbatim — position, `MoveType` and `MoveIntent` — while
/// rapids at or above `c` still frame it.
///
/// This is what makes a peck cycle survive. A G83/G73 hole is five feeds
/// separated by an R-plane retract and a re-entry rapid; with `None` those
/// are five independently-reorderable segments whose framing is rebuilt at
/// `safe_z`, which deletes the R-plane and turns every re-entry into a
/// `G1` feed from full safe-Z (105 mm of fed distance per hole where the
/// cycle describes 17 mm — see
/// `planning/perf_review_2026-08-19/RESEARCH_drill_intent_erasure.md`).
/// With `Some(safe_z)` the whole hole is **one** segment, so the cycle is
/// relocated intact and the pass still reorders holes by XY — the
/// optimisation `drill_capability_allows_tsp_reorder_reduces_rapid`
/// defends is kept, unlike the "refuse to reorder drills" alternative.
///
/// The value is a *ceiling on internal linking*, not a height anything is
/// emitted at; nothing here plants a rapid at `c`. (The C1 contract in the
/// research doc calls it `rebuild_clearance_z`; it is named for what it
/// does, because a single rebuild height cannot express a per-peck
/// re-entry clearance and would leave the cycle 2.6x over-fed.)
fn split_into_segments(toolpath: &Toolpath, internal_link_ceiling_z: Option<f64>) -> Vec<Segment> {
    let mut segments = Vec::new();
    let mut current_moves: Vec<Move> = Vec::new();
    let mut current_start_idx: Option<usize> = None;

    for (idx, m) in toolpath.moves.iter().enumerate() {
        let frames_segment = matches!(m.move_type, MoveType::Rapid)
            && match internal_link_ceiling_z {
                None => true,
                Some(ceiling) => m.target.z >= ceiling - CEILING_EPS,
            };
        if frames_segment {
            flush_segment(&mut segments, &mut current_moves, &mut current_start_idx);
        } else {
            if current_start_idx.is_none() {
                current_start_idx = Some(idx);
            }
            current_moves.push(m.clone());
        }
    }

    flush_segment(&mut segments, &mut current_moves, &mut current_start_idx);

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
/// span — that span is dropped (F2.2). The surviving spans keep correct
/// bounds, so `spans_valid` stays `true`; transit classification for the
/// dropped span's moves degrades to the per-move `MoveIntent` union in the
/// metrics stamper.
///
/// # Algorithm
///
/// 1. Derive barriers from the input spans (if any) and slice the toolpath
///    into barrier-delimited groups.
/// 2. Within each group: split into cutting segments, apply nearest-neighbor
///    + 2-opt, then reassemble with retract/rapid/plunge between segments.
/// 3. Build a `MoveRemap` describing where each old move ended up.
/// 4. Remap input spans through the permutation; drop any non-`Operation`
///    span that fragmented across barriers / segments (F2.2).
// SAFETY: all indexing in this function is bounded by `n` (segment count)
// and group_bounds, both built locally.
pub fn optimize_rapid_order(annotated: AnnotatedToolpath, safe_z: f64) -> AnnotatedToolpath {
    optimize_rapid_order_with_provenance(annotated, safe_z, None)
        .reconcile(&mut ReconcileSet::empty())
        .into_inner()
}

/// [`optimize_rapid_order`] under the C1 provenance contract: hands back the
/// permutation so every index-carrying channel beside the spans can follow
/// the moves.
///
/// The provenance is [`MoveProvenance::Permutation`], not `Remap` — the
/// difference is not bookkeeping. A reorder can interleave foreign moves
/// into a claim's new bounding range, and a channel that only knew the
/// bounding remap would widen the claim to cover strangers instead of
/// dropping it. Same rule the span filter below applies, same predicate.
// SAFETY: all indexing in this function is bounded by `n` (segment count)
// and group_bounds, both built locally.
#[allow(clippy::indexing_slicing)]
pub fn optimize_rapid_order_with_provenance(
    annotated: AnnotatedToolpath,
    safe_z: f64,
    internal_link_ceiling_z: Option<f64>,
) -> Transformed {
    let barriers = annotated.rapid_order_barriers();
    let AnnotatedToolpath {
        toolpath,
        spans,
        spans_valid: input_valid,
        planner_engagement,
        rest_grid,
        rest_regions,
    } = annotated;

    if toolpath.moves.is_empty() {
        return Transformed::index_preserving(AnnotatedToolpath {
            toolpath,
            spans,
            spans_valid: input_valid,
            planner_engagement,
            rest_grid,
            rest_regions,
        });
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
            internal_link_ceiling_z,
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
        (remap_spans(&spans, &remap, new_n, &toolpath.moves), true)
    } else {
        (spans, false)
    };

    Transformed::from_permutation(
        AnnotatedToolpath {
            toolpath: result,
            spans: new_spans,
            spans_valid: new_valid,
            planner_engagement,
            rest_grid,
            rest_regions,
        },
        remap,
    )
}

/// Run the single-group nearest-neighbor + 2-opt over `[group_start..group_end)`
/// of the input, append the reordered moves to `result`, and update `old_to_new`
/// so each input cutting move points to its new index.
#[allow(clippy::indexing_slicing)]
fn optimize_one_group(
    toolpath: &Toolpath,
    group: Range<usize>,
    safe_z: f64,
    internal_link_ceiling_z: Option<f64>,
    result: &mut Toolpath,
    old_to_new: &mut [Option<Range<usize>>],
) {
    let group_view = Toolpath {
        moves: toolpath.moves[group.clone()].to_vec(),
    };
    let mut segments = split_into_segments(&group_view, internal_link_ceiling_z);
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
    let mut order = Vec::with_capacity(n);

    // The seed used to be a Θ(n²) scan over every unvisited segment (the
    // 2-opt refinement below is capped; this was not — PERF_REVIEW G5).
    // `NearestPicker` answers the same question with the same tie-break:
    // lexicographic minimum of `(xy_distance, segment index)`, so the tour is
    // the one the scan produced, bit for bit. `f64::INFINITY` is the scan's
    // own acceptance sentinel and index 0 its fallback when nothing clears it
    // — both reproduced verbatim, including the degenerate re-push of an
    // already-visited index that a NaN coordinate would provoke.
    let mut picker = crate::nn_order::NearestPicker::new(crate::nn_order::Metric::Euclid, n);
    for (i, s) in segments.iter().enumerate() {
        picker.push(i, s.start.x, s.start.y);
    }
    picker.build();

    order.push(0);
    picker.remove(0);

    for _ in 1..n {
        let current = order[order.len() - 1];
        let current_end = &segments[current].end;

        let best_idx = match picker.nearest(current_end.x, current_end.y) {
            Some((idx, d)) if d < f64::INFINITY => idx,
            _ => 0,
        };

        picker.remove(best_idx);
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
///
/// # Intents on the synthesized rapids
///
/// Every rapid this function plants is one of two things, and it says
/// which: the vertical lift off a segment's end is a
/// [`MoveIntent::Retract`], the horizontal traverse onto the next
/// segment's start is a [`MoveIntent::Linking`]. Both are what the
/// generators mean by those moves, and `dressup.rs` sets the same
/// precedent for its own synthesized linking rapid.
///
/// Before this they were all [`MoveIntent::Unknown`], because
/// `Toolpath::rapid_to` is `rapid_to_with_intent(target,
/// MoveIntent::Unknown)`. The consequence was not confined to drills —
/// Pocket lost the intent on 66 of its 99 rapids under this pass — and it
/// reached the transit classification that gate populations are filtered
/// by (`toolpath_spans` falls back to the per-move intent union when a
/// span is dropped) and the W6 retract census, which read zero
/// Retract-tagged moves out of a drill toolpath that emits ten.
///
/// **Cutting moves are still only cloned.** No fed move is synthesized
/// here and no fed move's intent is rewritten — `MoveIntent::Drilling`,
/// `LeadIn`, `EntryPlunge` and `FinishingCut` survive the pass untouched,
/// which is the invariant the session's drill entry-strip predicate
/// depends on.
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
            // F-038b: if the previous group's fast-path appended verbatim
            // without a trailing retract (e.g. a depth-pass whose internal
            // transitions used stay-down feed links instead of rapids → one
            // big segment → fast path), the cutter is still at low Z.
            // Without this retract, the next single rapid would go diagonally
            // (current low Z → seg.start.xy, safe_z) — slicing through stock.
            // Mirror the (idx > 0) branch: vertical retract first, then
            // horizontal traverse at safe_z.
            if let Some(last) = result.moves.last()
                && last.target.z < safe_z
            {
                result.rapid_to_with_intent(
                    P3::new(last.target.x, last.target.y, safe_z),
                    MoveIntent::Retract,
                );
            }
            result.rapid_to_with_intent(
                P3::new(seg.start.x, seg.start.y, safe_z),
                MoveIntent::Linking,
            );
        } else {
            let prev_seg = &segments[order[idx - 1]];
            result.rapid_to_with_intent(
                P3::new(prev_seg.end.x, prev_seg.end.y, safe_z),
                MoveIntent::Retract,
            );
            result.rapid_to_with_intent(
                P3::new(seg.start.x, seg.start.y, safe_z),
                MoveIntent::Linking,
            );
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
        result.rapid_to_with_intent(
            P3::new(last_seg.end.x, last_seg.end.y, safe_z),
            MoveIntent::Retract,
        );
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

/// Remap each input span through the permutation. Returns the new spans —
/// non-`Operation` spans that fragmented (foreign moves intruded into their
/// new bounding range) are DROPPED rather than poisoning the whole vector.
///
/// The per-span bounding remap (drop-if-fully-collapsed, boundary vs. range
/// handling, label/payload carry-through) is the same contract every other
/// span-preserving transform uses, so it's delegated to
/// [`MoveRemap::remap_span_with_index`] (the indexed sibling of the
/// canonical `remap_span` helper in `toolpath_spans`).
/// What's unique to TSP is the foreign-move-intrusion check layered on top
/// as a post-filter below — a permutation can interleave moves from other
/// spans into a span's new bounding range, which a plain bounding remap
/// can't detect on its own.
///
/// F2.2 (defect class C3): pre-F2 a single fragmented span flipped
/// `spans_valid = false` for the entire toolpath, discarding every
/// still-correct span (Entry, WaterlineCleanup) at the metrics stamper
/// and the gate sites — which is how a tagged transient became
/// effectively untagged and drove phantom gate trips (the WANAKA 622 µm
/// DeflectionSetupLocked mechanism). Dropping exactly the spans whose
/// remapped bounds are wrong keeps the survivors trustworthy; the
/// dropped spans' moves keep their transit classification through the
/// per-move `MoveIntent` union in the metrics stamper
/// (`compute/simulate.rs`).
///
/// C9: builds one [`RemapIndex`] up front (rather than rescanning
/// `remap.old_to_new` per span via `MoveRemap::remap_span` /
/// `MoveRemap::foreign_intrusion`) — this is the exact loop the index was
/// built to retire, since a wanaka-class reorder can carry ~200k moves and
/// dozens-to-hundreds of spans through it.
#[allow(clippy::indexing_slicing)]
fn remap_spans(spans: &[Span], remap: &MoveRemap, new_n: usize, moves: &[Move]) -> Vec<Span> {
    let index = RemapIndex::build(remap);
    spans
        .iter()
        .filter_map(|s| {
            // Every move in the span got dropped — drop the span too.
            let new_span = remap.remap_span_with_index(s, new_n, &index)?;

            // Foreign-intrusion check only applies to non-boundary,
            // non-Operation spans — boundary spans are always zero-width
            // (nothing to intrude on), and Operation spans are exempt
            // because the permutation is internal to them.
            if !s.is_boundary() && s.kind != SpanKind::Operation {
                let bounds = new_span.start_move..new_span.end_move;

                // Any old move *outside* the span that non-trivially
                // overlaps `bounds` means the span's contents got
                // interleaved with foreign moves by the reorder. The
                // predicate lives on `MoveRemap` so the semantic-trace
                // channel applies the SAME rule through
                // `MoveProvenance::Permutation` (C1) — it NAMES the intruder
                // rather than just answering yes/no, because which move
                // intrudes is the whole diagnosis: a span whose own remapped
                // bounds are near-exact (measured on wanaka: a 200 924-move
                // region node came back as 200 959, a 0.02% dilation) is not
                // "scattered by the reorder" — it is being discarded by this
                // guard because some unrelated move landed in its range. See
                // `planning/unified_v3_design.md` §14d.
                let foreign_intrusion = index.foreign_intrusion(s.start_move, s.end_move, &bounds);

                if let Some((intruder_old_idx, intruder_new)) = foreign_intrusion {
                    let intruder_intent = moves.get(intruder_old_idx).map(|m| m.intent);
                    let before_span = intruder_old_idx < s.start_move;
                    tracing::debug!(
                        span_kind = ?s.kind,
                        span_label = %s.label,
                        old_range = ?(s.start_move..s.end_move),
                        new_bounds = ?bounds,
                        intruder_old_idx,
                        intruder_new = ?intruder_new,
                        ?intruder_intent,
                        before_span,
                        "TSP rapid-order optimization split a non-Operation span; \
                         dropping it (remaining spans stay valid; per-move intents \
                         keep transit classification for its moves)"
                    );
                    return None;
                }
            }

            Some(new_span)
        })
        .collect()
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
            intent: crate::toolpath::MoveIntent::Unknown,
        });
        for p in points {
            moves.push(Move {
                target: *p,
                move_type: MoveType::Linear { feed_rate },
                intent: crate::toolpath::MoveIntent::Unknown,
            });
        }
        moves.push(Move {
            target: P3::new(last.x, last.y, safe_z),
            move_type: MoveType::Rapid,
            intent: crate::toolpath::MoveIntent::Unknown,
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

        let segments = split_into_segments(&tp, None);
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].moves.len(), 2);
        assert_eq!(segments[1].moves.len(), 2);
        assert!((segments[0].start.x - 0.0).abs() < 1e-10);
        assert!((segments[0].end.x - 5.0).abs() < 1e-10);
        assert!((segments[1].start.x - 20.0).abs() < 1e-10);
        assert!((segments[1].end.x - 25.0).abs() < 1e-10);
    }

    /// C1: with an internal-link ceiling, a rapid BELOW it does not split.
    ///
    /// The shape is a peck cycle in miniature — two feeds separated by a
    /// retract to an R-plane and a re-entry, framed by safe-Z rapids. With
    /// `None` that is two segments and the framing between them is thrown
    /// away; with `Some(safe_z)` it is ONE segment carrying its own two
    /// rapids, which is what makes the cycle survive the reorder intact.
    #[test]
    fn split_into_segments_keeps_internal_links_below_the_ceiling() {
        let safe_z = 17.0;
        let r_plane = 5.0;
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, safe_z));
        tp.rapid_to(P3::new(0.0, 0.0, r_plane));
        tp.feed_to(P3::new(0.0, 0.0, 2.0), 300.0);
        tp.rapid_to(P3::new(0.0, 0.0, r_plane));
        tp.rapid_to(P3::new(0.0, 0.0, 2.5));
        tp.feed_to(P3::new(0.0, 0.0, -1.0), 300.0);
        tp.rapid_to(P3::new(0.0, 0.0, r_plane));

        let split_at_every_rapid = split_into_segments(&tp, None);
        assert_eq!(
            split_at_every_rapid.len(),
            2,
            "without a ceiling every rapid frames a segment"
        );
        assert_eq!(split_at_every_rapid[0].moves.len(), 1);
        assert_eq!(split_at_every_rapid[1].moves.len(), 1);

        let preserved = split_into_segments(&tp, Some(safe_z));
        assert_eq!(
            preserved.len(),
            1,
            "with the ceiling at safe_z the whole cycle is one atomic segment"
        );
        // Everything but the leading safe-Z rapid: the R-plane approach,
        // both feeds, the retract, the re-entry and the final retract.
        assert_eq!(preserved[0].moves.len(), 6);
        assert!((preserved[0].start.z - r_plane).abs() < 1e-10);
        assert!((preserved[0].end.z - r_plane).abs() < 1e-10);
        assert_eq!(preserved[0].src_range, 1..7);
    }

    /// A run of preserved internal rapids with no cut in it is not a
    /// segment — there is nothing to reorder and nothing to remap.
    #[test]
    fn split_into_segments_drops_a_cutless_run_of_internal_rapids() {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 17.0));
        tp.rapid_to(P3::new(0.0, 0.0, 3.0));
        tp.rapid_to(P3::new(1.0, 0.0, 3.0));
        tp.rapid_to(P3::new(1.0, 0.0, 17.0));
        tp.feed_to(P3::new(2.0, 0.0, -1.0), 300.0);

        let segments = split_into_segments(&tp, Some(17.0));
        assert_eq!(segments.len(), 1, "only the run holding the feed survives");
        assert_eq!(segments[0].moves.len(), 1);
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
    fn optimize_rapid_order_drops_split_span_keeps_rest_valid() {
        // Build four segments so that NN/2-opt picks the order
        // [seg0, seg2, seg1, seg3] (proximity grouping):
        //   seg0 cuts at X=0, seg1 at X=50 (far), seg2 at X=2 (next to seg0),
        //   seg3 at X=52 (next to seg1).
        // A Region span covers input range [0..seg2_start), i.e. seg0+seg1.
        // After reorder, seg2 (which is OUTSIDE the span) lands between
        // seg0 and seg1 in the output → seg2's new slot intrudes into the
        // span's bounding range → the split span is DROPPED while
        // `spans_valid` stays true for the survivors (F2.2: pre-F2 this
        // flipped spans_valid=false for the whole vector, discarding
        // every still-correct span at the metrics stamper / gate sites).
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
            result.spans_valid,
            "surviving spans stay valid after a split span is dropped"
        );
        assert!(
            result
                .spans
                .iter()
                .all(|s| s.kind != SpanKind::Region || s.label != "seg0+seg1"),
            "the split Region span must be dropped, got {:?}",
            result.spans,
        );
        result
            .check_invariants()
            .expect("remapped spans still satisfy structural invariants");
    }

    // ── C9: remap_spans identical-output regression ────────────────────
    //
    // `remap_spans` (this module, ~line 502) now builds one `RemapIndex`
    // up front and answers `remap_span` / `foreign_intrusion` through it
    // instead of rescanning `old_to_new` per span. This is `remap_spans`
    // AS IT SHIPS TODAY compared, on hand-built fixtures, against
    // `remap_spans_reference` below — the exact pre-C9 algorithm, kept
    // here as a byte-for-byte oracle because `MoveRemap::remap_span` and
    // `MoveRemap::foreign_intrusion` themselves are UNCHANGED by C9 (only
    // `remap_spans`'s internals were switched to the indexed variants), so
    // this reference is genuinely the old code path, not a copy that could
    // have drifted.

    /// Pre-C9 `remap_spans`: identical control flow, but built on the
    /// un-indexed `MoveRemap::remap_span` / `MoveRemap::foreign_intrusion`
    /// scans instead of a `RemapIndex`. Drops the `moves` parameter — it
    /// only feeds the production function's `tracing::debug!` diagnostic,
    /// not the returned span vector, so it plays no role in this
    /// equivalence check.
    fn remap_spans_reference(spans: &[Span], remap: &MoveRemap, new_n: usize) -> Vec<Span> {
        spans
            .iter()
            .filter_map(|s| {
                let new_span = remap.remap_span(s, new_n)?;
                if !s.is_boundary() && s.kind != SpanKind::Operation {
                    let bounds = new_span.start_move..new_span.end_move;
                    if remap
                        .foreign_intrusion(s.start_move, s.end_move, &bounds)
                        .is_some()
                    {
                        return None;
                    }
                }
                Some(new_span)
            })
            .collect()
    }

    fn dummy_moves(n: usize) -> Vec<Move> {
        (0..n)
            .map(|i| Move {
                target: P3::new(i as f64, 0.0, 0.0),
                move_type: MoveType::Linear { feed_rate: 1000.0 },
                intent: crate::toolpath::MoveIntent::Unknown,
            })
            .collect()
    }

    #[test]
    fn remap_spans_matches_unindexed_reference_on_fragmented_and_dropped_case() {
        // Mirrors `remap_spans_drops_fully_collapsed_and_preserves_order`
        // (toolpath_spans.rs): a fully-collapsed middle span must be
        // dropped while the survivors keep their order.
        let remap = MoveRemap {
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
        let moves = dummy_moves(6);

        let indexed = remap_spans(&spans, &remap, 4, &moves);
        let reference = remap_spans_reference(&spans, &remap, 4);
        assert_eq!(indexed, reference);
        assert_eq!(indexed.len(), 2, "collapsed span must be dropped");
        assert_eq!(indexed[0].label, "first");
        assert_eq!(indexed[1].label, "third");
    }

    #[test]
    fn remap_spans_matches_unindexed_reference_on_foreign_intrusion_case() {
        // A permutation that scatters a foreign move into a Region span's
        // bounding range: old moves 0,1 form the "scattered" span, but move
        // 1 lands at new position 2..3 while move 0 stays at 0..1, leaving a
        // gap at new 1..2 — which move 2 (old index 2, NOT part of the
        // span) then occupies. Old moves 3,4 form the "clean" span and stay
        // untouched by the permutation, so its remapped bounds (3..5) don't
        // reach back far enough to see move 2's new slot (1..2) at all.
        //
        // Verified by hand against `MoveRemap::foreign_intrusion`'s
        // documented predicate (`r.start < r.end && r.start < bounds.end &&
        // r.end > bounds.start`) before writing this, precisely because an
        // earlier draft of this fixture had the "clean" span's own remapped
        // bounds accidentally widened far enough (by a different old
        // index's contribution) to also register a false intrusion —
        // exactly the class of mistake this equivalence check exists to
        // catch, so getting the fixture right by hand mattered here.
        let remap = MoveRemap {
            old_to_new: vec![
                Some(0..1), // old 0 (scattered) -> new 0..1
                Some(2..3), // old 1 (scattered) -> new 2..3 (gap at 1..2)
                Some(1..2), // old 2 (outside)   -> new 1..2, the intruder
                Some(3..4), // old 3 (clean)     -> new 3..4
                Some(4..5), // old 4 (clean)     -> new 4..5
            ],
        };
        let spans = vec![
            Span::new(0, 5, SpanKind::Operation).with_label("op"),
            Span::new(0, 2, SpanKind::Region).with_label("scattered"),
            Span::new(3, 5, SpanKind::DepthPass).with_label("clean"),
        ];
        let moves = dummy_moves(5);

        let indexed = remap_spans(&spans, &remap, 5, &moves);
        let reference = remap_spans_reference(&spans, &remap, 5);
        assert_eq!(indexed, reference);
        // The scattered Region span is dropped; Operation is exempt from
        // the intrusion check and the clean DepthPass survives untouched.
        assert!(
            indexed.iter().all(|s| s.label != "scattered"),
            "scattered span should be dropped, got {indexed:?}"
        );
        assert!(indexed.iter().any(|s| s.label == "op"));
        assert!(
            indexed.iter().any(|s| s.label == "clean"),
            "clean span should survive, got {indexed:?}"
        );
    }

    #[test]
    fn remap_spans_matches_unindexed_reference_on_boundary_and_empty_cases() {
        // Boundary spans (zero-width) and an empty span list must both
        // round-trip identically through the indexed and reference paths.
        let remap = MoveRemap::identity(5);
        let moves = dummy_moves(5);

        let boundary_spans = vec![
            Span::boundary(0, SpanKind::RapidOrderBarrier),
            Span::boundary(3, SpanKind::RapidOrderBarrier),
            Span::boundary(5, SpanKind::RapidOrderBarrier),
        ];
        assert_eq!(
            remap_spans(&boundary_spans, &remap, 5, &moves),
            remap_spans_reference(&boundary_spans, &remap, 5),
        );

        let empty_spans: Vec<Span> = Vec::new();
        assert_eq!(
            remap_spans(&empty_spans, &remap, 5, &moves),
            remap_spans_reference(&empty_spans, &remap, 5),
        );
        assert!(remap_spans(&empty_spans, &remap, 5, &moves).is_empty());
    }
}
