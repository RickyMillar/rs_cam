//! Shared marching-squares primitives: cell classification, saddle
//! resolution, and segment chaining.
//!
//! Before this module existed, three call sites each carried their own
//! near-identical copy of this logic:
//! - `contour_extract::weave_contours`'s fiber-based marching squares
//!   (waterline contour weaving).
//! - `contour_extract::marching_squares_bool_grid` (flat bool-grid contour
//!   extraction, consumed by `adaptive3d::clearing` and `adaptive::spiral`).
//! - `boundary::model_silhouette`'s mesh-silhouette marching squares.
//!
//! All three now build on the classification + segment tables here; see
//! `planning/finishing_stack_review_2026-07.md` P1.7 / R1.2.
//!
//! ## Corner / edge convention
//!
//! A cell is defined by its 4 corner samples named for their position in the
//! cell's OWN local frame: `bl`/`br`/`tr`/`tl` (bottom-left, bottom-right,
//! top-right, top-left). Callers are responsible for mapping their own grid
//! row/column indexing onto this local frame, and the three pre-existing
//! implementations disagreed about which array direction is "up":
//! `contour_extract`'s fiber grid and `boundary`'s silhouette grid both treat
//! increasing row index as away from the origin (`bl = grid[row][col]`,
//! `tl = grid[row + 1][col]`), while `contour_extract`'s flat bool-grid entry
//! point (`marching_squares_bool_grid`) treats increasing row index the
//! opposite way (`tl = grid[row * cols + col]`, `bl = grid[(row + 1) * cols +
//! col]`). That direction choice is purely a labeling exercise at each call
//! site's corner-fetch expression — it has no effect on emitted topology,
//! because [`cell_case`] and [`cell_segments`] are defined entirely in terms
//! of the local bl/br/tr/tl labels, not physical grid direction. Each call
//! site is its own adapter: it just has to fetch the 4 samples into the
//! right named slots.
//!
//! Case index bit order: `bit0=bl, bit1=br, bit2=tr, bit3=tl` (matches every
//! pre-existing implementation in this crate).
//!
//! Local edges, numbered for [`cell_segments`]:
//! - [`EDGE_LEFT`]   (0) — between `bl` and `tl`
//! - [`EDGE_BOTTOM`] (1) — between `bl` and `br`
//! - [`EDGE_RIGHT`]  (2) — between `br` and `tr`
//! - [`EDGE_TOP`]    (3) — between `tl` and `tr`
//!
//! ## Saddle convention (cases 5 and 10)
//!
//! Cases 5 (`bl`,`tr` sampled true; `br`,`tl` false) and 10 (`br`,`tl` true;
//! `bl`,`tr` false) are the two diagonal-only configurations where the
//! topology is ambiguous: does the contour connect the two `true` corners
//! through the cell center (one continuous region, bridging the diagonal),
//! or does it treat them as separate islands with the `false` corners
//! bridging the center instead?
//!
//! The textbook resolution samples the bilinearly-interpolated scalar field
//! at the cell center and connects whichever pair agrees with it. Every
//! consumer in this crate only ever feeds `marching_squares` a **boolean**
//! grid — there's no interpolatable scalar field surviving to this layer —
//! so the center average of the four 0/1 corners is *always exactly 0.5*
//! for cases 5 and 10. That's a permanent tie, not a decision, and needs an
//! explicit tie-break rather than a computed one.
//!
//! `cell_segments` breaks the tie by treating the center as **inside**
//! (material): the two `true` corners are connected through the center, and
//! each `false` corner is resolved as its own separate isolated notch.
//! Concretely: case 5 emits `(Bottom,Right)` + `(Top,Left)` — isolating `br`
//! and `tl` separately, connecting `bl`-`tr` through the middle; case 10
//! emits `(Left,Bottom)` + `(Right,Top)` — isolating `bl` and `tr`
//! separately, connecting `br`-`tl` through the middle.
//!
//! `contour_extract.rs`'s two marching-squares implementations already
//! agreed on this convention before this module existed;
//! `boundary.rs`'s `model_silhouette` used the opposite tie-break
//! (isolating the two *material* corners separately instead) — see
//! `planning/finishing_stack_review_2026-07.md` §R1.2. This module
//! standardizes on the `contour_extract` convention, since it already had
//! 2 of the 3 call sites; `model_silhouette`'s silhouette contours now pinch
//! the other way at diagonal touch-points than they did before this merge
//! (no test pinned the old behavior — see the report for this change).

use crate::geo::P2;
use std::collections::HashMap;

/// Local edge identifiers within a marching-squares cell (see module doc).
/// Left edge — between `bl` and `tl`.
pub const EDGE_LEFT: u8 = 0;
/// Bottom edge — between `bl` and `br`.
pub const EDGE_BOTTOM: u8 = 1;
/// Right edge — between `br` and `tr`.
pub const EDGE_RIGHT: u8 = 2;
/// Top edge — between `tl` and `tr`.
pub const EDGE_TOP: u8 = 3;

/// Classify a cell's 4 corner samples into the 4-bit marching-squares case
/// index (`bit0=bl, bit1=br, bit2=tr, bit3=tl`).
#[inline]
pub fn cell_case(bl: bool, br: bool, tr: bool, tl: bool) -> u8 {
    (bl as u8) | ((br as u8) << 1) | ((tr as u8) << 2) | ((tl as u8) << 3)
}

/// The 16 marching-squares cases encoded as local edge-index pairs (0, 1, or
/// 2 segments per case). See the module doc for the saddle tie-break used at
/// cases 5 and 10 (bool-grid saddles are always an exact 2-2 corner tie).
const MS_CASES: [&[(u8, u8)]; 16] = [
    &[],                                                 // 0:  0000 — all outside
    &[(EDGE_LEFT, EDGE_BOTTOM)],                         // 1:  0001 — bl only
    &[(EDGE_BOTTOM, EDGE_RIGHT)],                        // 2:  0010 — br only
    &[(EDGE_LEFT, EDGE_RIGHT)],                          // 3:  0011 — bl+br
    &[(EDGE_RIGHT, EDGE_TOP)],                           // 4:  0100 — tr only
    &[(EDGE_BOTTOM, EDGE_RIGHT), (EDGE_TOP, EDGE_LEFT)], // 5: saddle — bl-tr bridge
    &[(EDGE_BOTTOM, EDGE_TOP)],                          // 6:  0110 — br+tr
    &[(EDGE_TOP, EDGE_LEFT)],                            // 7:  0111 — bl+br+tr (tl out)
    &[(EDGE_LEFT, EDGE_TOP)],                            // 8:  1000 — tl only
    &[(EDGE_BOTTOM, EDGE_TOP)],                          // 9:  1001 — bl+tl
    &[(EDGE_LEFT, EDGE_BOTTOM), (EDGE_RIGHT, EDGE_TOP)], // 10: saddle — br-tl bridge
    &[(EDGE_RIGHT, EDGE_TOP)],                           // 11: 1011 — bl+br+tl (tr out)
    &[(EDGE_LEFT, EDGE_RIGHT)],                          // 12: 1100 — tr+tl
    &[(EDGE_BOTTOM, EDGE_RIGHT)],                        // 13: 1101 — bl+tr+tl (br out)
    &[(EDGE_LEFT, EDGE_BOTTOM)],                         // 14: 1110 — br+tr+tl (bl out)
    &[],                                                 // 15: 1111 — all inside
];

/// Return the local edge pairs forming contour segments for a case index
/// (0..16). Returns an empty slice for case 0/15 (no contour) and is
/// defensive for any other out-of-range index (case indices are always
/// 0..16 by construction of [`cell_case`]).
pub fn cell_segments(case: u8) -> &'static [(u8, u8)] {
    MS_CASES.get(case as usize).copied().unwrap_or(&[])
}

/// Deterministic chaining epsilon (world units — this crate is mm-scale
/// throughout, so `1e-6` is far below machining tolerance and far above
/// float noise from repeated cell-size arithmetic).
pub const CHAIN_EPS: f64 = 1e-6;

/// Quantization scale for the chaining spatial hash: coordinates are rounded
/// to a grid of `1 / CHAIN_QUANTIZE_SCALE` world units (10x finer than
/// `CHAIN_EPS`) so that endpoints within `CHAIN_EPS` of each other always
/// land in the same or an adjacent hash cell. Assumes mm-scale coordinates
/// (as does `CHAIN_EPS`) — a grid using different units would need both
/// constants rescaled.
pub const CHAIN_QUANTIZE_SCALE: f64 = 1e5;

/// Chain a set of unordered 2D line segments into closed loops.
///
/// Uses a spatial hash map on quantized endpoints for O(1) neighbor lookup
/// instead of an O(n) linear scan per chain link. Shared by every marching
/// squares call site in this crate — the fiber-based waterline weaver
/// chains on `(P2, P2)` with a constant Z carried separately by the caller
/// (see `contour_extract::weave_contours`), since Z never varies within a
/// single marching-squares pass.
#[allow(clippy::indexing_slicing)] // SAFETY: indices bounded by segment count
pub fn chain_segments(segments: &[(P2, P2)]) -> Vec<Vec<P2>> {
    if segments.is_empty() {
        return Vec::new();
    }

    let eps = CHAIN_EPS;
    let n = segments.len();

    // Quantize a coordinate to an integer grid at epsilon scale.
    let quantize = |v: f64| -> i64 { (v * CHAIN_QUANTIZE_SCALE).round() as i64 };
    type GridKey = (i64, i64);

    // Build spatial index: quantized (x, y) -> list of (segment_index, endpoint_id).
    // endpoint_id: 0 = p1, 1 = p2.
    let mut index: HashMap<GridKey, Vec<(usize, u8)>> = HashMap::with_capacity(n * 2);
    for (i, (p1, p2)) in segments.iter().enumerate() {
        let k1 = (quantize(p1.x), quantize(p1.y));
        let k2 = (quantize(p2.x), quantize(p2.y));
        index.entry(k1).or_default().push((i, 0));
        index.entry(k2).or_default().push((i, 1));
    }

    let mut used = vec![false; n];
    let mut loops: Vec<Vec<P2>> = Vec::new();

    for start_idx in 0..n {
        if used[start_idx] {
            continue;
        }
        used[start_idx] = true;
        let mut chain = vec![segments[start_idx].0, segments[start_idx].1];

        let max_iterations = n + 1;
        for _ in 0..max_iterations {
            let tail = chain[chain.len() - 1];

            // Check if loop is closed.
            let head = chain[0];
            let dx = tail.x - head.x;
            let dy = tail.y - head.y;
            if chain.len() >= 3 && dx * dx + dy * dy < eps * eps {
                chain.pop();
                break;
            }

            // Look up neighbors in the spatial index (3x3 grid cells).
            let qx = quantize(tail.x);
            let qy = quantize(tail.y);
            let mut found = false;

            'search: for dx_cell in -1i64..=1 {
                for dy_cell in -1i64..=1 {
                    let key = (qx + dx_cell, qy + dy_cell);
                    if let Some(entries) = index.get(&key) {
                        for &(seg_idx, endpoint) in entries {
                            if used[seg_idx] {
                                continue;
                            }
                            let (p1, p2) = segments[seg_idx];
                            let (match_pt, other_pt) =
                                if endpoint == 0 { (p1, p2) } else { (p2, p1) };
                            let d = (match_pt.x - tail.x).powi(2) + (match_pt.y - tail.y).powi(2);
                            if d < eps * eps {
                                chain.push(other_pt);
                                used[seg_idx] = true;
                                found = true;
                                break 'search;
                            }
                        }
                    }
                }
            }

            if !found {
                break;
            }
        }

        if chain.len() >= 3 {
            loops.push(chain);
        }
    }

    loops
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

    #[test]
    fn cell_case_bit_order() {
        assert_eq!(cell_case(false, false, false, false), 0);
        assert_eq!(cell_case(true, false, false, false), 1);
        assert_eq!(cell_case(false, true, false, false), 2);
        assert_eq!(cell_case(false, false, true, false), 4);
        assert_eq!(cell_case(false, false, false, true), 8);
        assert_eq!(cell_case(true, true, true, true), 15);
    }

    #[test]
    fn saddle_cases_bridge_the_true_diagonal() {
        // Case 5: bl,tr true; br,tl false. Convention: bl-tr bridge, so the
        // two isolated notches are (Bottom,Right) [isolating br] and
        // (Top,Left) [isolating tl].
        let segs5 = cell_segments(5);
        assert_eq!(segs5.len(), 2);
        assert!(segs5.contains(&(EDGE_BOTTOM, EDGE_RIGHT)));
        assert!(segs5.contains(&(EDGE_TOP, EDGE_LEFT)));

        // Case 10: br,tl true; bl,tr false. Convention: br-tl bridge, so the
        // two isolated notches are (Left,Bottom) [isolating bl] and
        // (Right,Top) [isolating tr].
        let segs10 = cell_segments(10);
        assert_eq!(segs10.len(), 2);
        assert!(segs10.contains(&(EDGE_LEFT, EDGE_BOTTOM)));
        assert!(segs10.contains(&(EDGE_RIGHT, EDGE_TOP)));
    }

    #[test]
    fn non_saddle_cases_emit_one_segment() {
        for case in [1u8, 2, 3, 4, 6, 7, 8, 9, 11, 12, 13, 14] {
            assert_eq!(
                cell_segments(case).len(),
                1,
                "case {case} should emit exactly one segment"
            );
        }
    }

    #[test]
    fn empty_and_full_cases_emit_nothing() {
        assert!(cell_segments(0).is_empty());
        assert!(cell_segments(15).is_empty());
    }

    #[test]
    fn chain_segments_closed_loop() {
        let segments = vec![
            (P2::new(0.0, 0.0), P2::new(1.0, 0.0)),
            (P2::new(1.0, 0.0), P2::new(1.0, 1.0)),
            (P2::new(1.0, 1.0), P2::new(0.0, 1.0)),
            (P2::new(0.0, 1.0), P2::new(0.0, 0.0)),
        ];

        let loops = chain_segments(&segments);
        assert_eq!(loops.len(), 1, "Should form one closed loop");
        assert_eq!(loops[0].len(), 4, "Loop should have 4 points");
    }

    #[test]
    fn chain_segments_two_loops() {
        let segments = vec![
            // Square 1
            (P2::new(0.0, 0.0), P2::new(1.0, 0.0)),
            (P2::new(1.0, 0.0), P2::new(1.0, 1.0)),
            (P2::new(1.0, 1.0), P2::new(0.0, 1.0)),
            (P2::new(0.0, 1.0), P2::new(0.0, 0.0)),
            // Square 2 (far away)
            (P2::new(10.0, 10.0), P2::new(11.0, 10.0)),
            (P2::new(11.0, 10.0), P2::new(11.0, 11.0)),
            (P2::new(11.0, 11.0), P2::new(10.0, 11.0)),
            (P2::new(10.0, 11.0), P2::new(10.0, 10.0)),
        ];

        let loops = chain_segments(&segments);
        assert_eq!(loops.len(), 2, "Should form two separate loops");
    }
}
