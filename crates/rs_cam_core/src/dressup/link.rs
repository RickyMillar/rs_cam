//! Link-vs-retract corridor geometry: the swept-corridor test that decides
//! whether a candidate link bridge crosses ground this toolpath already cut.
//!
//! Split out of `dressup/mod.rs` (P4). The section's entry points
//! (`apply_link_moves*`) stay in the parent.

use crate::geo::P3;
use crate::toolpath::Move;

/// Depth-match tolerance (mm) for treating two cut Z levels as "the same
/// pass" — shared by the `prev_z`/`plunge_target.z` check below and by the
/// corridor-Z gate in [`bridge_corridor_is_swept`], so "same depth level"
/// means one consistent thing in this function.
pub(super) const LINK_Z_MATCH_TOL: f64 = 0.1;

/// Number of most-recently-emitted moves considered as candidate "already
/// swept" cutting segments in [`bridge_corridor_is_swept`].
///
/// PERFORMANCE BOUND, chosen and justified here (see also the call site):
/// `apply_link_moves` runs this check once per *candidate* bridge, and a
/// production fixture (VCarve) has ~26,000 moves — an unbounded backward
/// scan of the full history per candidate would make the pass roughly
/// O(bridges × moves), which is too slow to run per-dressup. Two bounds are
/// applied together, and BOTH are sound in the same direction: they can
/// only make the check *more conservative* (skip a segment that really did
/// cover the sample → refuse a safe link), never *less conservative*
/// (a skipped segment can never manufacture a false "covered"). That
/// asymmetry is what makes bounding safe to do at all — worst case we
/// fall back to the pre-fix retract/rapid/plunge triple more often than
/// strictly necessary, never the reverse.
///
/// 1. This constant bounds how far back in *emission order* we look.
///    Links are local (`max_link_distance` is ~10-20mm in practice), and
///    every generator in this codebase emits moves in spatial order
///    (raster/ring/offset passes), so material that could plausibly cover
///    a bridge a few mm away was almost always cut within the last few
///    thousand moves, not tens of thousands of moves ago.
/// 2. Within that window, `bridge_corridor_is_swept` additionally rejects
///    candidate segments via a cheap AABB-vs-corridor-bbox test (padded by
///    `tool_radius`) before the exact point-to-segment distance calc — an
///    *exact* filter (no segment that could satisfy the distance test is
///    excluded), so it only skips wasted arithmetic, never correctness.
const LINK_CORRIDOR_LOOKBACK_MOVES: usize = 4096;

/// Point-to-segment distance in the XY plane (Z ignored) from `(px, py)` to
/// the segment `a`→`b`. Used by [`bridge_corridor_is_swept`] so a bridge
/// sample near the *middle* of a previously-cut segment (not just near one
/// of its endpoints) is correctly recognised as covered.
fn point_segment_distance_xy(px: f64, py: f64, a: P3, b: P3) -> f64 {
    let abx = b.x - a.x;
    let aby = b.y - a.y;
    let len2 = abx * abx + aby * aby;
    if len2 < 1e-12 {
        // Degenerate (zero-length) segment: distance to the shared point.
        return ((px - a.x).powi(2) + (py - a.y).powi(2)).sqrt();
    }
    let t = (((px - a.x) * abx + (py - a.y) * aby) / len2).clamp(0.0, 1.0);
    let cx = a.x + t * abx;
    let cy = a.y + t * aby;
    ((px - cx).powi(2) + (py - cy).powi(2)).sqrt()
}

/// Is the straight corridor `from`→`to` fully covered by ground this
/// toolpath has *already cut*, at the same Z?
///
/// ROOT-CAUSE CONTEXT (measured 2026, `tests/capability_link_moves_safety.rs`):
/// `apply_link_moves` used to collapse a retract→rapid→plunge triple into a
/// single straight feed bridge with no check at all that the straight-line
/// corridor between the two cut points was clear of material. On variable-
/// depth or multi-feature paths that bridge plows: Face 9.21mm, Inlay
/// 8.21mm, VCarve 5.78mm, and discrete Scallop 9.71mm of measured over-cut.
/// Flat, already-cleared paths (Chamfer, Pencil, RadialFinish) measured
/// exactly 0.0000mm — the tell that the bridge is safe *exactly* when the
/// ground it crosses has already been swept, and unsafe otherwise.
///
/// `apply_dressups` (`compute/execute.rs`) has no mesh/spatial-index at this
/// stage — only `prior_stock` (the stock *before* this toolpath ran) — so
/// this cannot reuse `surface_link::build_surface_link`'s drop-cutter check
/// directly, and checking against `prior_stock` would reject every bridge
/// that crosses ground *this same toolpath* just cleared, throwing away the
/// entire benefit of linking. Instead this is the dressup-level, move-list
/// counterpart to `build_surface_link`: rather than sampling a mesh, it
/// samples the candidate bridge and checks each interior sample against the
/// cutting moves *this same toolpath has already emitted*, at the same Z —
/// i.e. "only link across ground you have already swept."
///
/// Semantics: samples the segment at roughly `tool_radius * 0.5` spacing
/// (at least the two endpoints). The two endpoints are trivially covered —
/// `from` is itself a cut position and `to` is where the very next
/// (unlinked) cut move lands — so it is the *interior* samples that carry
/// the real test. An interior sample is covered if it lies within
/// `tool_radius` (XY, point-to-segment, not point-to-endpoint) of some
/// previously-emitted cutting segment (a non-`Rapid` move paired with its
/// predecessor) whose Z is within `z_tol` of the sample's interpolated Z.
/// Returns `true` only if every interior sample is covered.
pub(super) fn bridge_corridor_is_swept(
    emitted: &[Move],
    from: P3,
    to: P3,
    tool_radius: f64,
    z_tol: f64,
) -> bool {
    if tool_radius <= 1e-9 {
        // Degenerate/unknown tool radius: nothing can be trusted as
        // "covered". Conservative refusal, never a false approval.
        return false;
    }

    let dx = to.x - from.x;
    let dy = to.y - from.y;
    let dist = (dx * dx + dy * dy).sqrt();
    if dist < 1e-9 {
        return true; // zero-length bridge — nothing to cross
    }

    let spacing = (tool_radius * 0.5).max(1e-6);
    let n_segments = (dist / spacing).ceil().max(1.0) as usize;
    if n_segments < 2 {
        // Gap is smaller than half a tool radius — only the (trivially
        // covered) endpoints exist as samples.
        return true;
    }

    let corridor_min_x = from.x.min(to.x) - tool_radius;
    let corridor_max_x = from.x.max(to.x) + tool_radius;
    let corridor_min_y = from.y.min(to.y) - tool_radius;
    let corridor_max_y = from.y.max(to.y) + tool_radius;

    let window_start = emitted.len().saturating_sub(LINK_CORRIDOR_LOOKBACK_MOVES);
    // SAFETY: `.get(range)` is the non-panicking slice accessor; an
    // out-of-range start (only possible if emitted.len() < window_start,
    // which cannot happen since window_start is derived from
    // emitted.len() itself) degrades to an empty slice rather than a panic.
    let recent = emitted.get(window_start..).unwrap_or(&[]);

    for k in 1..n_segments {
        let t = k as f64 / n_segments as f64;
        let sx = from.x + dx * t;
        let sy = from.y + dy * t;
        let sz = from.z + (to.z - from.z) * t;

        let mut covered = false;
        for pair in recent.windows(2).rev() {
            let [seg_start, seg_end] = pair else {
                continue; // windows(2) always yields length-2 slices
            };
            if !seg_end.move_type.is_cutting() {
                continue;
            }
            let a = seg_start.target;
            let b = seg_end.target;
            if (a.z - sz).abs() > z_tol || (b.z - sz).abs() > z_tol {
                continue;
            }
            // Cheap AABB reject before the exact point-to-segment distance.
            let seg_min_x = a.x.min(b.x);
            let seg_max_x = a.x.max(b.x);
            let seg_min_y = a.y.min(b.y);
            let seg_max_y = a.y.max(b.y);
            if seg_max_x < corridor_min_x
                || seg_min_x > corridor_max_x
                || seg_max_y < corridor_min_y
                || seg_min_y > corridor_max_y
            {
                continue;
            }
            if point_segment_distance_xy(sx, sy, a, b) <= tool_radius {
                covered = true;
                break;
            }
        }

        if !covered {
            return false;
        }
    }

    true
}
