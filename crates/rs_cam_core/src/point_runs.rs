//! Shared "split points into contiguous runs" primitive.
//!
//! Several finishing/roughing operations sample a sequence of points (a
//! radial spoke, a waterline contour, a lifted polyline, a raster row, a
//! scallop ring, a spiral pass) and need to keep only the sub-runs where
//! some per-point predicate holds (mesh contact, slope-band membership,
//! non-clamped Z, …). The rest is a gap that must not be bridged by a
//! straight cutting move — that either gouges excluded material or chords
//! across an off-mesh hole.
//!
//! This module centralizes that "walk points, split at gaps, drop short
//! runs, optionally handle closed-loop wraparound" logic so each caller
//! only supplies its own predicate. The primitive only does the
//! splitting — emission (rapid/plunge/feed/retract per run) stays with
//! each call site.

/// Topology of the point sequence being split into runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunTopology {
    /// A linear/open sequence (a spoke, a raster row, a polyline). No
    /// wraparound between the last and first index — a run touching
    /// index 0 and a run touching the last index are unrelated.
    Open,
    /// A closed loop (a contour, a ring). A run touching the last index
    /// and a run touching index 0 are the same physical run split across
    /// the array's arbitrary start point, and are merged back together so
    /// a single excluded point near the array's start doesn't spuriously
    /// fragment one run into two.
    ///
    /// If every point is kept, the whole sequence is returned as a single
    /// run spanning the full index range — the caller decides whether
    /// that means "safe to close back to the start" (a partial survivor
    /// set is never safe to close, since the gap it left behind would be
    /// chorded across).
    Closed,
}

/// Split `points` into contiguous runs where `keep(index, point)` holds,
/// dropping runs shorter than `min_len`. See [`RunTopology`] for how the
/// two topologies differ at the array boundary.
pub fn split_runs<T, F>(points: &[T], keep: F, topology: RunTopology, min_len: usize) -> Vec<Vec<T>>
where
    T: Clone,
    F: Fn(usize, &T) -> bool,
{
    let n = points.len();
    if n == 0 {
        return Vec::new();
    }

    let ranges = kept_ranges(points, &keep);
    if ranges.is_empty() {
        return Vec::new();
    }

    // Genuinely closed loop: every point survived, so there is no gap to
    // preserve — return the whole sequence as one run.
    if topology == RunTopology::Closed
        && ranges.len() == 1
        && ranges.first().is_some_and(|&(s, e)| s == 0 && e == n - 1)
    {
        return vec![points.to_vec()];
    }

    // Does the last range wrap around to meet the first range across the
    // array seam?
    let wraps = topology == RunTopology::Closed
        && ranges.len() > 1
        && ranges.first().is_some_and(|&(s, _)| s == 0)
        && ranges.last().is_some_and(|&(_, e)| e == n - 1);

    let start_idx = usize::from(wraps);
    let end_idx = ranges.len().saturating_sub(usize::from(wraps));

    let mut runs: Vec<Vec<T>> = ranges
        .get(start_idx..end_idx)
        .unwrap_or(&[])
        .iter()
        .filter_map(|&(s, e)| points.get(s..=e))
        .map(|slice| slice.to_vec())
        .collect();

    if wraps {
        let mut merged = Vec::new();
        if let Some(&(last_start, _)) = ranges.last()
            && let Some(tail) = points.get(last_start..n)
        {
            merged.extend_from_slice(tail);
        }
        if let Some(&(_, first_end)) = ranges.first()
            && let Some(head) = points.get(0..=first_end)
        {
            merged.extend_from_slice(head);
        }
        runs.push(merged);
    }

    runs.retain(|r| r.len() >= min_len);
    runs
}

/// Zero-copy variant of [`split_runs`] for [`RunTopology::Open`] sequences:
/// returns inclusive `(start, end)` index ranges instead of cloning each
/// run, so callers that only need to slice the original array (or an
/// unrelated parallel array) can avoid the allocation.
///
/// Closed-loop wraparound merging needs to concatenate two disjoint index
/// ranges into a single physical run, which can't be expressed as one
/// range — use [`split_runs`] with [`RunTopology::Closed`] for that case.
pub fn split_run_ranges<T, F>(points: &[T], keep: F, min_len: usize) -> Vec<(usize, usize)>
where
    F: Fn(usize, &T) -> bool,
{
    kept_ranges(points, &keep)
        .into_iter()
        .filter(|&(s, e)| e + 1 - s >= min_len)
        .collect()
}

/// Collect the inclusive index ranges of contiguous `keep(i, p) == true`
/// stretches, in original order, with no wraparound or `min_len` handling.
fn kept_ranges<T>(points: &[T], keep: &impl Fn(usize, &T) -> bool) -> Vec<(usize, usize)> {
    let n = points.len();
    let mut ranges = Vec::new();
    let mut run_start: Option<usize> = None;
    for (i, p) in points.iter().enumerate() {
        if keep(i, p) {
            if run_start.is_none() {
                run_start = Some(i);
            }
        } else if let Some(start) = run_start.take() {
            ranges.push((start, i.saturating_sub(1)));
        }
    }
    if let Some(start) = run_start {
        ranges.push((start, n.saturating_sub(1)));
    }
    ranges
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn seq(n: usize) -> Vec<i32> {
        (0..n as i32).collect()
    }

    // ── split_runs: empty / all-kept ─────────────────────────────────

    #[test]
    fn empty_input_returns_no_runs() {
        let pts: Vec<i32> = Vec::new();
        let runs = split_runs(&pts, |_, _| true, RunTopology::Open, 1);
        assert!(runs.is_empty());
        let runs = split_runs(&pts, |_, _| true, RunTopology::Closed, 1);
        assert!(runs.is_empty());
    }

    #[test]
    fn all_kept_open_returns_single_run() {
        let pts = seq(5);
        let runs = split_runs(&pts, |_, _| true, RunTopology::Open, 1);
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0], pts);
    }

    #[test]
    fn all_kept_closed_returns_single_whole_run() {
        let pts = seq(6);
        let runs = split_runs(&pts, |_, _| true, RunTopology::Closed, 2);
        assert_eq!(runs.len(), 1, "fully-kept sequence should be one run");
        assert_eq!(runs[0], pts);
    }

    // ── min_len dropping ──────────────────────────────────────────────

    #[test]
    fn singleton_run_dropped_when_min_len_two() {
        // Only index 1 survives — a lone point can't form a valid pass.
        let pts = seq(5);
        let keep = [false, true, false, false, false];
        let runs = split_runs(&pts, |i, _| keep[i], RunTopology::Open, 2);
        assert!(
            runs.is_empty(),
            "lone survivor should be dropped, got {runs:?}"
        );
    }

    #[test]
    fn singleton_run_kept_when_min_len_one() {
        let pts = seq(5);
        let keep = [false, true, false, false, false];
        let runs = split_runs(&pts, |i, _| keep[i], RunTopology::Open, 1);
        assert_eq!(runs, vec![vec![1]]);
    }

    #[test]
    fn closed_singleton_survivor_dropped() {
        let pts = seq(5);
        let keep = [false, true, false, false, false];
        let runs = split_runs(&pts, |i, _| keep[i], RunTopology::Closed, 2);
        assert!(
            runs.is_empty(),
            "lone survivor should be dropped, got {runs:?}"
        );
    }

    // ── gaps: leading/trailing, interior, multiple ───────────────────

    #[test]
    fn gap_at_both_ends_open_trims_to_interior_run() {
        let pts = seq(4);
        let keep = [false, true, true, false];
        let runs = split_runs(&pts, |i, _| keep[i], RunTopology::Open, 1);
        assert_eq!(runs, vec![vec![1, 2]]);
    }

    #[test]
    fn single_interior_gap_splits_into_two_runs() {
        let pts = seq(5);
        let keep = [true, true, false, true, true];
        let runs = split_runs(&pts, |i, _| keep[i], RunTopology::Open, 1);
        assert_eq!(runs, vec![vec![0, 1], vec![3, 4]]);
    }

    #[test]
    fn multiple_interior_gaps_split_into_several_runs() {
        let pts = seq(9);
        let keep = [true, false, true, true, false, false, true, true, true];
        let runs = split_runs(&pts, |i, _| keep[i], RunTopology::Open, 1);
        assert_eq!(runs, vec![vec![0], vec![2, 3], vec![6, 7, 8]]);
    }

    #[test]
    fn none_kept_returns_no_runs() {
        let pts = seq(3);
        let runs = split_runs(&pts, |_, _| false, RunTopology::Open, 1);
        assert!(runs.is_empty());
    }

    // ── closed-loop wraparound ─────────────────────────────────────────

    #[test]
    fn closed_wraparound_merges_seam_run() {
        // Only index 3 excluded on a closed loop of 7: indices 4,5,6,0,1,2
        // are one physical run split across the array seam and must be
        // merged back into a single run rather than reported as two.
        let pts = seq(7);
        let keep = [true, true, true, false, true, true, true];
        let runs = split_runs(&pts, |i, _| keep[i], RunTopology::Closed, 2);
        assert_eq!(runs.len(), 1, "seam run should merge, got {runs:?}");
        assert_eq!(runs[0], vec![4, 5, 6, 0, 1, 2]);
    }

    #[test]
    fn closed_excluded_seam_does_not_spuriously_merge() {
        // Indices 0 and 7 (the array's two ends) are both excluded, so the
        // seam sits inside an excluded stretch on both sides — 1,2 and 5,6
        // are genuinely separate runs, not one run wrapping across the seam.
        let pts = seq(8);
        let keep = [false, true, true, false, false, true, true, false];
        let runs = split_runs(&pts, |i, _| keep[i], RunTopology::Closed, 1);
        assert_eq!(runs.len(), 2, "expected two separate runs, got {runs:?}");
        assert_eq!(runs[0], vec![1, 2]);
        assert_eq!(runs[1], vec![5, 6]);
    }

    // ── split_run_ranges (zero-copy variant) ───────────────────────────

    #[test]
    fn split_run_ranges_empty_input() {
        let pts: Vec<i32> = Vec::new();
        let ranges = split_run_ranges(&pts, |_, _| true, 1);
        assert!(ranges.is_empty());
    }

    #[test]
    fn split_run_ranges_matches_split_runs_slicing() {
        let pts = seq(9);
        let keep = [true, false, true, true, false, false, true, true, true];
        let ranges = split_run_ranges(&pts, |i, _| keep[i], 1);
        let sliced: Vec<&[i32]> = ranges.iter().filter_map(|&(s, e)| pts.get(s..=e)).collect();
        assert_eq!(sliced, vec![&[0][..], &[2, 3][..], &[6, 7, 8][..]]);
    }

    #[test]
    fn split_run_ranges_drops_short_runs() {
        let pts = seq(5);
        let keep = [false, true, false, true, true];
        let ranges = split_run_ranges(&pts, |i, _| keep[i], 2);
        assert_eq!(ranges, vec![(3, 4)]);
    }
}
