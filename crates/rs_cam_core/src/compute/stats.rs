use super::config::{RetractTripCount, ToolpathStats};
use super::execute::GenerationFindings;
use crate::toolpath::{MoveType, Toolpath};
use crate::toolpath_spans::Span;

/// Compute basic toolpath statistics (move count, cutting distance, rapid
/// distance, retract trip count). Convenience wrapper over
/// [`compute_stats_with_spans`] for callers with no
/// [`crate::toolpath_spans::AnnotatedToolpath`] in scope — the retract-trip
/// in/out split will be `None` (spans-free callers cannot classify a trip
/// against a routing node).
pub fn compute_stats(tp: &Toolpath) -> ToolpathStats {
    compute_stats_with_spans(tp, None)
}

/// Same as [`compute_stats`], but when `spans` is `Some`, also classifies
/// every retract round trip as inside a planner routing node or between two
/// nodes (see [`compute_retract_trips`]).
///
/// Pass `None` — not an empty slice — when spans are absent or untrusted.
/// A caller holding an [`crate::toolpath_spans::AnnotatedToolpath`] must
/// resolve `spans_valid == false` to `None` itself before calling this:
/// [`RetractTripCount`]'s X-19 contract is that an untrustworthy split
/// reports as unmeasured, never as a confident zero.
// SAFETY: loop from 1..len, indexing [i] and [i-1] always in bounds
#[allow(clippy::indexing_slicing)]
pub fn compute_stats_with_spans(tp: &Toolpath, spans: Option<&[Span]>) -> ToolpathStats {
    let mut cutting = 0.0;
    let mut rapid = 0.0;
    for i in 1..tp.moves.len() {
        let from = tp.moves[i - 1].target;
        let to = tp.moves[i].target;
        let distance =
            ((to.x - from.x).powi(2) + (to.y - from.y).powi(2) + (to.z - from.z).powi(2)).sqrt();
        match tp.moves[i].move_type {
            MoveType::Rapid => rapid += distance,
            _ => cutting += distance,
        }
    }
    ToolpathStats {
        move_count: tp.moves.len(),
        cutting_distance: cutting,
        rapid_distance: rapid,
        // NOT MEASURED, not zero: this helper only sees moves, and a
        // cascade residual is not derivable from them. The generation path
        // overwrites it from `GenerationFindings`; every other caller must
        // keep reading `None` so no ratio is built on a fabricated zero
        // (`MEASUREMENT_DOMAINS.md` X-19).
        truncated_core_mm2: None,
        // M4 §5b: same rule, same reason — the split rides on the same
        // generation-time `GenerationFindings`, not on the move list.
        untouched_material_mm2: None,
        reached_uncut_estimate_mm2: None,
        // Same rule (Wave D1): a band drop and a centreline float are
        // generation-time findings. This helper only sees moves, so it can
        // only honestly say "not measured".
        dropped_band: None,
        tip_float: None,
        deprecated_dial: None,
        derived_stepovers: Vec::new(),
        clipped_band: None,
        // PR-8b: same rule again — no ramp descent is visible from a move
        // list, so this helper can only honestly say "not measured".
        ramp_reach_clamp: None,
        // A/M6: same rule — no claims pipeline is visible from a move list.
        claims_reference: None,
        // A4: same rule — a move list carries no reference stock to be
        // measured against, so this helper cannot answer the question.
        zero_removal: None,
        // A/M7 gate 1: this helper DOES see the move list, so the trip
        // TOTAL is always measured; the in/out split additionally needs
        // `spans` (see `compute_retract_trips`).
        retract_trips: Some(compute_retract_trips(tp, spans)),
    }
}

/// **The** join from a generation's [`GenerationFindings`] onto the
/// move-derived statistics of the toolpath it produced (H2.1 / R7).
///
/// Every production caller that has both halves in hand goes through here:
/// `ProjectSession::generate_toolpath` and the GUI compute worker. Before
/// this existed the two paths each carried their own field-by-field copy of
/// the same mapping, and the GUI one was patched **five separate times** for
/// a channel that reached the session path and not the operator's screen —
/// the worst instance dropping every annotated side-channel at once. A
/// finding that lands on one path and not the other is invisible in exactly
/// the product the operator uses, which is the whole reason
/// [`GenerationFindings`] exists.
///
/// ## Why this is a destructure and not a series of assignments
///
/// Three compile-time guards, all of them hard errors rather than lints:
///
/// 1. The [`GenerationFindings`] pattern has no `..`, so a new finding is
///    `E0027: pattern does not mention field` here until someone decides
///    where it goes. That is the acceptance gate of H2.1, and it is the
///    same mechanism the CLI's `ToolpathDiagnostic::from_core` has used to
///    hold its wire honest.
/// 2. The [`ToolpathStats`] pattern over the move-derived half likewise has
///    no `..`, and names each field as move-derived (bound) or
///    generation-owned (`_`, because the findings replace it). A new stats
///    field must be classified as one or the other.
/// 3. The returned literal has no `..Default::default()`, so a new stats
///    field is `E0063: missing field` as well.
///
/// A `stats.field = findings.field;` sequence — what both call sites used
/// to do — has none of these properties: forgetting one line compiles
/// silently and ships a `None` that reads as "not measured".
///
/// `spans` follows [`compute_stats_with_spans`]' contract: pass `None`, not
/// an empty slice, when spans are absent or `spans_valid == false`.
#[must_use]
pub fn stats_with_findings(
    tp: &Toolpath,
    spans: Option<&[Span]>,
    findings: GenerationFindings,
) -> ToolpathStats {
    // Guard 2: the move-derived half. Bound fields are measured from the
    // move list; `_` fields are generation-owned, and the helper's honest
    // "not measured" for them is about to be replaced by the findings.
    let ToolpathStats {
        move_count,
        cutting_distance,
        rapid_distance,
        retract_trips,
        truncated_core_mm2: _,
        untouched_material_mm2: _,
        reached_uncut_estimate_mm2: _,
        dropped_band: _,
        tip_float: _,
        deprecated_dial: _,
        derived_stepovers: _,
        clipped_band: _,
        ramp_reach_clamp: _,
        claims_reference: _,
        zero_removal: _,
    } = compute_stats_with_spans(tp, spans);

    // Guard 1: the generation-owned half. No `..` — a new finding stops
    // this build.
    let GenerationFindings {
        truncated_core_mm2,
        untouched_material_mm2,
        reached_uncut_estimate_mm2,
        dropped_band,
        clipped_band,
        tip_float,
        deprecated_dial,
        derived_stepovers,
        ramp_reach_clamp,
        claims_reference,
        zero_removal,
    } = findings;

    // Guard 3: no `..Default::default()`.
    ToolpathStats {
        move_count,
        cutting_distance,
        rapid_distance,
        retract_trips,
        truncated_core_mm2,
        untouched_material_mm2,
        reached_uncut_estimate_mm2,
        // Boxed on the stats side only — see the field docs on
        // `ToolpathStats` for why the carrier that is cloned per toolpath
        // pays a pointer and the short-lived findings do not.
        dropped_band: dropped_band.map(Box::new),
        tip_float,
        deprecated_dial: deprecated_dial.map(Box::new),
        derived_stepovers,
        clipped_band: clipped_band.map(Box::new),
        ramp_reach_clamp: ramp_reach_clamp.map(Box::new),
        claims_reference,
        zero_removal,
    }
}

/// Count retract round trips and, when `spans` is `Some`, classify each one
/// as inside a planner territory
/// [`crate::toolpath_spans::RegionSpanRole::Node`] or between two nodes
/// (A/M7 gate 1).
///
/// A "trip" is one maximal contiguous run of [`MoveType::Rapid`] moves —
/// reproduced bit-for-bit from `tests/v3_cascade_ab.rs`'s
/// `rapid_round_trips` helper (the harness this channel exists to agree
/// with) so the production channel and that harness can never disagree
/// about what counts as one trip. Classification looks only at the move
/// index each run STARTS at, exactly as the harness does.
///
/// `spans == None` means "the split cannot be trusted" — no spans were
/// supplied, or the caller already resolved
/// `AnnotatedToolpath::spans_valid == false` to `None`. `total` is still
/// returned; `in_node` / `between_nodes` and their mm siblings are `None`.
#[must_use]
pub fn compute_retract_trips(tp: &Toolpath, spans: Option<&[Span]>) -> RetractTripCount {
    let moves = &tp.moves;
    let node_ranges: Option<Vec<(usize, usize)>> = spans.map(|spans| {
        spans
            .iter()
            .filter(|s| s.is_region_node())
            .map(|s| (s.start_move, s.end_move))
            .collect()
    });
    let split_available = node_ranges.is_some();
    let inside = |move_idx: usize| -> bool {
        node_ranges
            .as_ref()
            .is_some_and(|nodes| nodes.iter().any(|&(a, b)| move_idx >= a && move_idx < b))
    };

    let mut total = 0usize;
    let (mut in_n, mut in_mm, mut out_n, mut out_mm) = (0usize, 0.0f64, 0usize, 0.0f64);
    let mut run_start: Option<usize> = None;
    for i in 0..moves.len() {
        let Some(mv) = moves.get(i) else {
            continue;
        };
        let is_rapid = matches!(mv.move_type, MoveType::Rapid);
        if is_rapid && run_start.is_none() {
            run_start = Some(i);
        }
        let ends = is_rapid
            && moves
                .get(i + 1)
                .is_none_or(|next| !matches!(next.move_type, MoveType::Rapid));
        if !ends {
            continue;
        }
        let Some(start) = run_start.take() else {
            continue;
        };
        total += 1;

        if split_available {
            let mut mm = 0.0;
            for j in start.max(1)..=i {
                let (Some(a), Some(b)) = (moves.get(j - 1), moves.get(j)) else {
                    continue;
                };
                let (a, b) = (a.target, b.target);
                mm += ((b.x - a.x).powi(2) + (b.y - a.y).powi(2) + (b.z - a.z).powi(2)).sqrt();
            }
            if inside(start) {
                in_n += 1;
                in_mm += mm;
            } else {
                out_n += 1;
                out_mm += mm;
            }
        }
    }

    RetractTripCount {
        total,
        in_node: split_available.then_some(in_n),
        between_nodes: split_available.then_some(out_n),
        in_node_rapid_mm: split_available.then_some(in_mm),
        between_nodes_rapid_mm: split_available.then_some(out_mm),
    }
}
