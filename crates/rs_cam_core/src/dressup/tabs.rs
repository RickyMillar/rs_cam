//! Holding tabs (bridges) of a profile (P5, S12 and S13): where they sit,
//! and the one lift emitter that cuts them ([`apply_tabs`]).
//!
//! Placement runs on the outline loop, the `Regions` item the user
//! selected, because a side and a corner are properties of the part. The
//! centres then go onto the ring the pass cuts ([`tabbed_ring`]), which
//! also moves the start of that ring into the largest gap between tabs. So
//! no tab zone crosses the plunge, and [`apply_tabs`] measures every tab
//! whole.
//!
//! P5b (operator rulings, 2026-10-19):
//!
//! - `tab_width` is the bridge: the width of the material a tab leaves.
//!   The lift on the cut ring is longer by the tool diameter on a straight
//!   ([`straight_tab_lift`]) and by the chord rule on a curve
//!   ([`tab_lift_length`]).
//! - The tab top is `final floor + tab_height` on every level. Each level
//!   below the tab top lifts over the tab ([`apply_tabs`]); placement does
//!   not depend on Z, so one placement serves every level.

use super::Z_LEVEL_EPS_MM;
use crate::geo::{P2, P3};
use crate::toolpath::{MoveType, Toolpath};

/// A tab (bridge) that holds the part to the stock during profile cutting.
#[derive(Debug, Clone)]
pub struct Tab {
    /// Position of the tab centre along the cut path (0.0 to 1.0,
    /// fractional).
    pub position: f64,
    /// Length (mm) of the lift along the cut path: the tool-centre span
    /// over the tab. It is not the bridge width; [`tab_lift_length`]
    /// derives it from the bridge width.
    pub width: f64,
    /// Height of the tab top above the final floor of the cut (mm).
    pub height: f64,
}

/// Insert holding tabs into one level of a profile toolpath.
///
/// `cut_depth` is the Z of the level and `floor_z` is the final floor of
/// the cut. The top of a tab is `floor_z + height` on every level. A level
/// below the tab top lifts over the tab to the tab top; a level at or
/// above the tab top cuts through, because it removes nothing of the tab.
/// So the tab keeps its full height when `height` is more than the depth
/// per pass (P5b).
///
/// Tabs create sharp rectangular bridges: the cutter steps up to the tab
/// top, traverses at that height, then steps back down.
///
/// Tab positions are interpolated along cutting segments, so tabs appear
/// at the correct location even when move endpoints are sparse.
///
/// # Move intent (CUT-08)
///
/// Every rewritten segment keeps the [`MoveIntent`](crate::toolpath::MoveIntent) of the move it replaces,
/// so a tabbed profile stays visible to each intent-keyed reader — the
/// spacing instrument (`metrology/spacing.rs`) skips a move that is not
/// `FinishingCut`. The two fed descents back to `cut_depth` after a tab are
/// [`MoveIntent::EntryPlunge`](crate::toolpath::MoveIntent::EntryPlunge): they enter material vertically, which is what
/// that tag means everywhere else in the crate.
///
/// The fed lift onto a tab keeps the cut intent. It removes material, and a
/// `Retract`-tagged feed is forbidden — see
/// `tests/retract_intent_move_type_census_w6.rs`.
#[allow(clippy::indexing_slicing)] // SAFETY: `cp` indexes `cutting_indices` and `cum_dist` (same length) below their length; each entry of `cutting_indices` indexes `toolpath.moves`
pub fn apply_tabs(toolpath: Toolpath, tabs: &[Tab], cut_depth: f64, floor_z: f64) -> Toolpath {
    use crate::toolpath::MoveIntent;

    // A tab whose top is not above this level leaves the level as it is.
    let tabs: Vec<&Tab> = tabs
        .iter()
        .filter(|t| floor_z + t.height > cut_depth + Z_LEVEL_EPS_MM)
        .collect();
    if tabs.is_empty() {
        return toolpath;
    }

    // Collect cutting move indices at cut_depth
    let cutting_indices: Vec<usize> = toolpath
        .moves
        .iter()
        .enumerate()
        .filter(|(_, m)| {
            matches!(m.move_type, MoveType::Linear { .. })
                && (m.target.z - cut_depth).abs() < Z_LEVEL_EPS_MM
        })
        .map(|(i, _)| i)
        .collect();

    if cutting_indices.len() < 2 {
        return toolpath;
    }

    // Compute cumulative distance at each cutting move endpoint
    let mut cum_dist = vec![0.0_f64];
    for i in 1..cutting_indices.len() {
        let prev = &toolpath.moves[cutting_indices[i - 1]].target;
        let curr = &toolpath.moves[cutting_indices[i]].target;
        let ddx = curr.x - prev.x;
        let ddy = curr.y - prev.y;
        let d = (ddx * ddx + ddy * ddy).sqrt();
        // SAFETY: cum_dist is initialized with [0.0] and only grows
        #[allow(clippy::expect_used)]
        cum_dist.push(cum_dist.last().expect("cum_dist starts with [0.0]") + d);
    }

    let total_dist = *cum_dist.last().unwrap_or(&0.0);
    if total_dist < 1e-6 {
        return toolpath;
    }

    // The tab zones as absolute distances along the cut, each with its
    // tab top.
    struct TabZone {
        start_dist: f64,
        end_dist: f64,
        tab_z: f64,
    }
    let tab_zones: Vec<TabZone> = tabs
        .iter()
        .map(|tab| {
            let center_dist = tab.position * total_dist;
            let half_w = tab.width / 2.0;
            TabZone {
                start_dist: (center_dist - half_w).max(0.0),
                end_dist: (center_dist + half_w).min(total_dist),
                tab_z: floor_z + tab.height,
            }
        })
        .collect();

    let tab_z_at_dist = |dist: f64| -> Option<f64> {
        tab_zones
            .iter()
            .find(|tz| dist >= tz.start_dist && dist <= tz.end_dist)
            .map(|tz| tz.tab_z)
    };

    // Walk through toolpath, interpolating tab boundaries along cutting
    // segments. `on_tab` holds the tab top while the tool is on a tab.
    let mut result = Toolpath::new();
    let mut on_tab: Option<f64> = None;

    for (i, m) in toolpath.moves.iter().enumerate() {
        let cut_pos = cutting_indices.iter().position(|&ci| ci == i);

        if let Some(cp) = cut_pos {
            if cp == 0 {
                // First cutting move — just emit it
                result.moves.push(m.clone());
                on_tab = tab_z_at_dist(0.0);
                continue;
            }

            let feed_rate = match m.move_type {
                MoveType::Linear { feed_rate } => feed_rate,
                _ => 1000.0,
            };

            let seg_start_dist = cum_dist[cp - 1];
            let seg_end_dist = cum_dist[cp];
            let prev_target = &toolpath.moves[cutting_indices[cp - 1]].target;
            let curr_target = &m.target;
            let seg_len = seg_end_dist - seg_start_dist;

            if seg_len < 1e-10 {
                result.moves.push(m.clone());
                continue;
            }

            // Collect all tab boundary crossings within this segment:
            // (dist, the tab top of the zone, is_entry).
            let mut events: Vec<(f64, f64, bool)> = Vec::new();
            for tz in &tab_zones {
                if tz.start_dist > seg_start_dist && tz.start_dist < seg_end_dist {
                    events.push((tz.start_dist, tz.tab_z, true));
                }
                if tz.end_dist > seg_start_dist && tz.end_dist < seg_end_dist {
                    events.push((tz.end_dist, tz.tab_z, false));
                }
            }
            events.sort_by(|a, b| a.0.total_cmp(&b.0));

            if events.is_empty() {
                // No boundary crossings — whole segment is in or out
                let mid_dist = (seg_start_dist + seg_end_dist) / 2.0;
                if let Some(tab_z) = tab_z_at_dist(mid_dist) {
                    if on_tab.is_none() {
                        // Entered tab zone before this segment
                        if let Some(last) = result.moves.last() {
                            result.feed_to_with_intent(
                                P3::new(last.target.x, last.target.y, tab_z),
                                feed_rate,
                                m.intent,
                            );
                        }
                        on_tab = Some(tab_z);
                    }
                    result.feed_to_with_intent(
                        P3::new(curr_target.x, curr_target.y, tab_z),
                        feed_rate,
                        m.intent,
                    );
                } else {
                    if on_tab.take().is_some()
                        && let Some(last) = result.moves.last()
                    {
                        result.feed_to_with_intent(
                            P3::new(last.target.x, last.target.y, cut_depth),
                            feed_rate,
                            MoveIntent::EntryPlunge,
                        );
                    }
                    result.moves.push(m.clone());
                }
            } else {
                // Process boundary crossings — split segment at each event
                for &(event_dist, tab_z, is_entry) in &events {
                    let t = (event_dist - seg_start_dist) / seg_len;
                    let split_x = prev_target.x + t * (curr_target.x - prev_target.x);
                    let split_y = prev_target.y + t * (curr_target.y - prev_target.y);

                    if is_entry {
                        // Emit segment up to tab entry at cut_depth
                        if on_tab.is_none() {
                            result.feed_to_with_intent(
                                P3::new(split_x, split_y, cut_depth),
                                feed_rate,
                                m.intent,
                            );
                        }
                        // Step up
                        result.feed_to_with_intent(
                            P3::new(split_x, split_y, tab_z),
                            feed_rate,
                            m.intent,
                        );
                        on_tab = Some(tab_z);
                    } else {
                        // Emit segment up to tab exit at tab height
                        result.feed_to_with_intent(
                            P3::new(split_x, split_y, tab_z),
                            feed_rate,
                            m.intent,
                        );
                        // Step down
                        result.feed_to_with_intent(
                            P3::new(split_x, split_y, cut_depth),
                            feed_rate,
                            MoveIntent::EntryPlunge,
                        );
                        on_tab = None;
                    }
                }

                // Emit remainder of segment after last event
                result.feed_to_with_intent(
                    P3::new(curr_target.x, curr_target.y, on_tab.unwrap_or(cut_depth)),
                    feed_rate,
                    m.intent,
                );
            }
        } else {
            // Non-cutting move — pass through unchanged
            on_tab = None;
            result.moves.push(m.clone());
        }
    }

    result
}

/// Generate evenly-spaced tabs around a perimeter. `width` is the lift
/// length of each tab ([`Tab::width`]).
pub fn even_tabs(count: usize, width: f64, height: f64) -> Vec<Tab> {
    (0..count)
        .map(|i| Tab {
            position: i as f64 / count as f64,
            width,
            height,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Placement (P5)
// ---------------------------------------------------------------------------

/// A straight run of a loop turns less than this angle (degrees) at every
/// joint, measured against the first segment of the run.
///
/// Derivation: the drawing door flattens every arc in steps of
/// [`crate::selection::item::CIRCLE_ARC_STEP_DEG`] (5 degrees). The step
/// is `sweep / ceil(sweep / 5)`, so each joint of an arc with a sweep of
/// 5 degrees or more turns by more than half the step. With half the step
/// as the limit, each such joint ends a run, and an arc gives no run
/// longer than one facet.
///
/// Limit: the SVG door flattens by a chord tolerance (0.1 user units), not
/// by an angle. An SVG arc of radius above about 420 user units turns less
/// than this limit per joint (`2 · acos(1 − 0.1 / R)`), so its segments
/// join into runs of up to this angle, and each such run can take a tab.
pub const STRAIGHT_RUN_TOLERANCE_DEG: f64 = crate::selection::item::CIRCLE_ARC_STEP_DEG / 2.0;

/// Two adjacent facets are one flattened arc when their lengths agree to
/// this fraction. The door cuts an arc in equal steps, so their lengths
/// agree to float noise; a real side of a part does not match its
/// neighbour to one part in a million by chance.
const FACET_LENGTH_MATCH: f64 = 1e-6;

/// The shortest segment (mm) that has a direction. A shorter segment
/// neither bends nor extends a run.
const MIN_SEGMENT_MM: f64 = 1e-9;

/// The clearance (mm) between the end of the lift of a tab and the nearest
/// corner of its side. The lift is [`straight_tab_lift`] long, so a side
/// holds a tab when it is `lift + 2 · clearance` long.
///
/// - `tool_radius`: on an outside profile, the tool-centre path of a
///   90 degree inside corner ends one tool radius before the outline
///   corner (`r · tan(45°)`). Without it, a tab near such a corner would
///   reach round the corner of the cut.
/// - `tab_width`: one whole tab width keeps the bridge on the straight,
///   away from the corner where the part changes shape (operator ruling,
///   2026-10-16; the Basis fork used the same margin).
#[must_use]
pub fn tab_corner_clearance(tab_width: f64, tool_radius: f64) -> f64 {
    tab_width + tool_radius
}

/// The lift (mm) of a tab on a straight side: `tab_width + 2 · tool_radius`.
///
/// `tab_width` is the bridge: the width of the material the tab leaves
/// (operator ruling, 2026-10-19). The tool cuts one radius past each end
/// of its lift, so the narrowest part of the bridge, on the line of the
/// tool centre, is `lift − 2 · tool_radius`. On the part face the bridge
/// is the full lift long, because the tool touches the face at one point.
/// [`tab_lift_length`] gives the lift on any ring; on a straight it is
/// this value.
#[must_use]
pub fn straight_tab_lift(tab_width: f64, tool_radius: f64) -> f64 {
    tab_width + 2.0 * tool_radius
}

/// The lift (mm) of a tab centred at distance `centre` along the closed
/// cut ring `ring`, for a bridge of `tab_width` and a tool of
/// `tool_radius`.
///
/// The rule: the lift is `[centre − a, centre + a]`, with the smallest
/// `a` for which the straight distance between its two ends is
/// `tab_width + 2 · tool_radius`. The tool at each end cuts a disc of
/// `tool_radius`, so the two discs stand `tab_width` apart: the bridge is
/// nowhere narrower than `tab_width`.
///
/// - A straight side: the chord is the arc, so the lift is
///   [`straight_tab_lift`].
/// - An arc of centre-path radius `ρ`: the chord of the lift is
///   `2ρ · sin(L / 2ρ)`, so `L = 2ρ · asin((tab_width + 2 · tool_radius) / 2ρ)`.
///   This is longer than on a straight.
///
/// A ring too small to hold the chord gets a lift of the whole ring.
#[must_use]
pub fn tab_lift_length(ring: &[P2], centre: f64, tab_width: f64, tool_radius: f64) -> f64 {
    let (segs, total) = segments(ring);
    lift_on(
        &segs,
        total,
        centre,
        straight_tab_lift(tab_width, tool_radius),
    )
}

/// [`tab_lift_length`] on the segments of a ring, for a chord of `chord`.
fn lift_on(segs: &[Segment], total: f64, centre: f64, chord: f64) -> f64 {
    let half_max = total / 2.0;
    let gap = |a: f64| {
        let (p, q) = (
            point_on(segs, total, centre - a),
            point_on(segs, total, centre + a),
        );
        (p.x - q.x).hypot(p.y - q.y)
    };
    // The chord is never longer than the arc, so `a` is at least half the
    // chord. On a straight that is the answer.
    let mut lo = (chord / 2.0).min(half_max);
    if gap(lo) >= chord - MIN_SEGMENT_MM {
        return 2.0 * lo;
    }
    // March out in steps of a quarter of the chord to the first `a` whose
    // chord is long enough, then halve the bracket. The step is short
    // against the lift, so the march does not jump over a first crossing
    // on a ring that bends back.
    let step = chord / 4.0;
    let mut hi = lo;
    loop {
        hi = (hi + step).min(half_max);
        if gap(hi) >= chord {
            break;
        }
        if hi >= half_max {
            return total;
        }
        lo = hi;
    }
    // The bracket stops at `MIN_SEGMENT_MM`: a shorter move along the ring
    // has no direction (see the constant).
    while hi - lo > MIN_SEGMENT_MM {
        let mid = 0.5 * (lo + hi);
        if gap(mid) >= chord {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    2.0 * hi
}

/// How far (mm) a hand tab may sit from the loop it names: one tool
/// diameter.
///
/// Derivation: the cut of a loop is a kerf one tool diameter wide beside
/// the outline (on either side, for Inside or Outside). A point within
/// that width of the outline is on the cut of that loop. A point farther
/// from every loop names no cut, and the op refuses it.
#[must_use]
pub fn hand_tab_reach(tool_radius: f64) -> f64 {
    2.0 * tool_radius
}

/// One straight side of a loop: its start and end as distances along the
/// loop from its first point. `end` can exceed the loop length: the side
/// then wraps past the loop start.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Side {
    pub start: f64,
    pub end: f64,
}

impl Side {
    #[must_use]
    pub fn length(&self) -> f64 {
        self.end - self.start
    }
}

/// One segment of a closed ring: start point, start distance, length,
/// unit direction.
struct Segment {
    a: P2,
    start: f64,
    len: f64,
    dir: (f64, f64),
}

/// The segments of the closed ring `ring` (the last point joins the first)
/// with a direction, and the ring length.
fn segments(ring: &[P2]) -> (Vec<Segment>, f64) {
    let mut out = Vec::with_capacity(ring.len());
    let mut at = 0.0;
    for (k, a) in ring.iter().enumerate() {
        let Some(b) = ring.get((k + 1) % ring.len()) else {
            continue;
        };
        let len = (b.x - a.x).hypot(b.y - a.y);
        if len >= MIN_SEGMENT_MM {
            out.push(Segment {
                a: *a,
                start: at,
                len,
                dir: ((b.x - a.x) / len, (b.y - a.y) / len),
            });
        }
        at += len;
    }
    (out, at)
}

fn dot(a: (f64, f64), b: (f64, f64)) -> f64 {
    a.0 * b.0 + a.1 * b.1
}

/// The straight sides of the closed loop `ring`, in loop order.
///
/// A run is a chain of segments whose directions stay within
/// [`STRAIGHT_RUN_TOLERANCE_DEG`] of the first segment of the chain. A run
/// can wrap past the loop start: the scan starts at the first sharp joint,
/// and a first run that continues the last run joins it. A run of one
/// segment that is one facet of a flattened arc (an adjacent segment of
/// the same length, with a joint that turns no more than the arc step) is
/// not a side: an arc joint turns more than the run tolerance and no more
/// than the arc step. So a circle has no side.
#[must_use]
#[allow(clippy::indexing_slicing)] // SAFETY: every index is taken modulo the length, or is below it by the loop bound
pub fn straight_sides(ring: &[P2]) -> Vec<Side> {
    let (segs, total) = segments(ring);
    let n = segs.len();
    if n < 3 || total <= 0.0 {
        return Vec::new();
    }
    let cos_tol = STRAIGHT_RUN_TOLERANCE_DEG.to_radians().cos();
    let cos_step = crate::selection::item::CIRCLE_ARC_STEP_DEG
        .to_radians()
        .cos();
    let seg = |k: usize| &segs[k % n];
    // Start at the first joint that turns more than the tolerance, so the
    // scan does not begin inside a run. A loop with no such joint starts
    // at its first segment.
    let first = (0..n)
        .find(|&k| dot(seg(k + n - 1).dir, seg(k).dir) < cos_tol)
        .unwrap_or(0);
    // Runs as (first segment, segment count), over the segments in scan
    // order `first..first + n`.
    let mut runs: Vec<(usize, usize)> = Vec::new();
    for k in first..first + n {
        match runs.last_mut() {
            Some((head, count)) if dot(seg(*head).dir, seg(k).dir) >= cos_tol => *count += 1,
            _ => runs.push((k, 1)),
        }
    }
    // A first run that continues the last run joins it (the wrap).
    if runs.len() > 1
        && let (Some(&(head, _)), Some(&(f_head, f_count))) = (runs.last(), runs.first())
        && (f_head..f_head + f_count).all(|k| dot(seg(head).dir, seg(k).dir) >= cos_tol)
    {
        runs.remove(0);
        if let Some(last) = runs.last_mut() {
            last.1 += f_count;
        }
    }
    let is_facet = |k: usize| {
        let s = seg(k);
        [seg(k + n - 1), seg(k + 1)].iter().any(|nb| {
            let turn = dot(nb.dir, s.dir);
            (nb.len - s.len).abs() <= FACET_LENGTH_MATCH * s.len
                && turn < cos_tol
                && turn >= cos_step - FACET_LENGTH_MATCH
        })
    };
    runs.into_iter()
        .filter(|&(head, count)| !(count == 1 && is_facet(head)))
        .map(|(head, count)| {
            let start = seg(head).start;
            let len: f64 = (head..head + count).map(|k| seg(k).len).sum();
            Side {
                start,
                end: start + len,
            }
        })
        .collect()
}

/// The point at distance `s` along the closed loop `ring` (taken modulo
/// the loop length).
#[must_use]
pub fn point_at(ring: &[P2], s: f64) -> P2 {
    let (segs, total) = segments(ring);
    if segs.is_empty() {
        return ring.first().copied().unwrap_or_else(|| P2::new(0.0, 0.0));
    }
    point_on(&segs, total, s)
}

/// [`point_at`] on the segments of a ring of length `total`. Needs at
/// least one segment; with none it gives the origin.
fn point_on(segs: &[Segment], total: f64, s: f64) -> P2 {
    let Some(last) = segs.last() else {
        return P2::new(0.0, 0.0);
    };
    let s = if total > 0.0 {
        s.rem_euclid(total)
    } else {
        0.0
    };
    let seg = segs.iter().find(|g| s <= g.start + g.len).unwrap_or(last);
    let t = (s - seg.start).clamp(0.0, seg.len);
    P2::new(seg.a.x + t * seg.dir.0, seg.a.y + t * seg.dir.1)
}

/// The nearest point of the closed loop `ring` to `p`: (point, distance
/// along the loop, distance from `p`).
#[must_use]
pub fn nearest_on_loop(ring: &[P2], p: P2) -> Option<(P2, f64, f64)> {
    let mut best: Option<(P2, f64, f64)> = None;
    let mut along = 0.0;
    for (k, a) in ring.iter().enumerate() {
        let Some(b) = ring.get((k + 1) % ring.len()) else {
            continue;
        };
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        let len2 = dx * dx + dy * dy;
        let t = if len2 < MIN_SEGMENT_MM * MIN_SEGMENT_MM {
            0.0
        } else {
            (((p.x - a.x) * dx + (p.y - a.y) * dy) / len2).clamp(0.0, 1.0)
        };
        let q = P2::new(a.x + t * dx, a.y + t * dy);
        let d = (p.x - q.x).hypot(p.y - q.y);
        if best.is_none_or(|(_, _, bd)| d < bd) {
            best = Some((q, along + t * len2.sqrt(), d));
        }
        along += len2.sqrt();
    }
    best
}

/// What the automatic rule placed on one loop.
#[derive(Debug, Clone, PartialEq)]
pub enum AutoTabs {
    /// Tab centres on the straight sides of the loop (outline XY).
    OnSides {
        centres: Vec<P2>,
        /// The sides that hold a tab.
        sides: usize,
    },
    /// No straight side holds a tab clear of its corners (a circle, an
    /// organic shape). The caller spaces the tabs evenly by length and
    /// records that it did.
    NoSide,
}

/// The automatic tabs of the closed outline `outline` (S12).
///
/// The rule:
///
/// 1. A side holds a tab when its length is at least
///    `lift + 2 · tab_corner_clearance`, with `lift` the
///    [`straight_tab_lift`] of `tab_width` (the bridge). Each such side
///    gets one tab.
/// 2. The total is `max(min_count, sides)`: the count of the op is a
///    minimum, and the one-per-side rule wins over a smaller count
///    (operator ruling 2026-10-16: at least one tab per side).
/// 3. Each further tab goes to the side with the largest share of free
///    length, `eligible / (tabs + 1)` (ties: the first side in loop
///    order), while the gap between two tabs stays at least the corner
///    clearance. A long side so takes more tabs than a short one.
/// 4. The tabs of one side sit at the centres of equal shares of the
///    length that is clear of its corners.
///
/// The sides are measured on the outline. On an outside profile the cut
/// runs one tool radius out from a straight side, so the lift on the cut
/// is as long as on the outline; `tab_corner_clearance` holds the
/// shortening of the cut at an inside corner.
#[must_use]
#[allow(clippy::indexing_slicing)] // SAFETY: every index is taken modulo the length, or is below it by the loop bound
pub fn auto_tabs(outline: &[P2], min_count: usize, tab_width: f64, tool_radius: f64) -> AutoTabs {
    let clearance = tab_corner_clearance(tab_width, tool_radius);
    let lift = straight_tab_lift(tab_width, tool_radius);
    // (eligible start, eligible length) of the centre of a tab per side.
    let eligible: Vec<(f64, f64)> = straight_sides(outline)
        .iter()
        .filter(|s| s.length() >= lift + 2.0 * clearance)
        .map(|s| {
            let lo = s.start + clearance + lift / 2.0;
            (lo, s.end - clearance - lift / 2.0 - lo)
        })
        .collect();
    if eligible.is_empty() {
        return AutoTabs::NoSide;
    }
    let capacity: Vec<usize> = eligible
        .iter()
        .map(|&(_, len)| ((len / (lift + clearance)).floor() as usize).max(1))
        .collect();
    let mut per_side = vec![1_usize; eligible.len()];
    for _ in eligible.len()..min_count.max(eligible.len()) {
        let next = (0..eligible.len())
            .filter(|&i| per_side[i] < capacity[i])
            .fold(None::<(usize, f64)>, |best, i| {
                let share = eligible[i].1 / (per_side[i] + 1) as f64;
                match best {
                    Some((_, b)) if b >= share => best,
                    _ => Some((i, share)),
                }
            });
        let Some((i, _)) = next else { break };
        per_side[i] += 1;
    }
    let centres = eligible
        .iter()
        .zip(&per_side)
        .flat_map(|(&(lo, len), &k)| {
            (0..k).map(move |j| point_at(outline, lo + len * (j as f64 + 0.5) / k as f64))
        })
        .collect();
    AutoTabs::OnSides {
        centres,
        sides: eligible.len(),
    }
}

/// `count` tab centres spaced evenly by length round the closed ring
/// `ring`, the first at its start: the spacing of [`even_tabs`] as points.
#[must_use]
pub fn even_centres(ring: &[P2], count: usize) -> Vec<P2> {
    let (_, total) = segments(ring);
    (0..count)
        .map(|i| point_at(ring, total * i as f64 / count as f64))
        .collect()
}

/// The ring a tabbed pass cuts and its tabs, for [`apply_tabs`].
#[derive(Debug, Clone, PartialEq)]
pub struct TabbedRing {
    /// The cut ring, its start moved into the largest gap between tabs.
    pub ring: Vec<P2>,
    /// One (position, lift) per centre, in loop order: the position as a
    /// fraction of the length of `ring` ([`Tab::position`]) and the lift
    /// in mm ([`Tab::width`], from [`tab_lift_length`]).
    pub zones: Vec<(f64, f64)>,
    /// One entry per zone, in the order of `zones`: the centre of the lift
    /// on the cut ring (the nearest point of the ring to the centre) and
    /// the index of that centre in the `centres` argument of
    /// [`tabbed_ring`] (geometry selection P4.6).
    pub points: Vec<(P2, usize)>,
}

impl TabbedRing {
    /// The tabs of this ring for [`apply_tabs`], each `height` above the
    /// final floor.
    #[must_use]
    pub fn tabs(&self, height: f64) -> Vec<Tab> {
        self.zones
            .iter()
            .map(|&(position, width)| Tab {
                position,
                width,
                height,
            })
            .collect()
    }
}

/// Put each centre on the closed cut ring `ring` (its nearest point), give
/// it the lift of a bridge of `tab_width` ([`tab_lift_length`]), and start
/// the ring in the middle of the largest gap between the lifts.
///
/// [`apply_tabs`] measures a tab zone on `[0, length]` and clips it there,
/// and it plunges at the ring start. A zone across the start would be cut
/// short and drilled through. With the start in a gap, every zone is whole
/// (when the tabs leave a gap at all).
#[must_use]
#[allow(clippy::indexing_slicing)] // SAFETY: every index is taken modulo the length, or is below it by the loop bound
pub fn tabbed_ring(ring: &[P2], centres: &[P2], tab_width: f64, tool_radius: f64) -> TabbedRing {
    let (segs, total) = segments(ring);
    let chord = straight_tab_lift(tab_width, tool_radius);
    // (distance along the ring, lift, point on the ring, centre index) per
    // centre.
    let mut along: Vec<(f64, f64, P2, usize)> = centres
        .iter()
        .enumerate()
        .filter_map(|(at, &c)| nearest_on_loop(ring, c).map(|(q, s, _)| (s, q, at)))
        .map(|(s, q, at)| (s, lift_on(&segs, total, s, chord), q, at))
        .collect();
    if along.is_empty() || total <= 0.0 {
        return TabbedRing {
            ring: ring.to_vec(),
            zones: Vec::new(),
            points: Vec::new(),
        };
    }
    along.sort_by(|a, b| a.0.total_cmp(&b.0));
    // The largest gap between one lift end and the next lift start, round
    // the loop. Ties: the first.
    let mut seam = 0.0;
    let mut widest = f64::NEG_INFINITY;
    for (i, &(s, lift, _, _)) in along.iter().enumerate() {
        let (next, next_lift) = along
            .get(i + 1)
            .map_or((along[0].0 + total, along[0].1), |n| (n.0, n.1));
        let gap = (next - next_lift / 2.0) - (s + lift / 2.0);
        if gap > widest {
            widest = gap;
            seam = (s + lift / 2.0 + gap / 2.0).rem_euclid(total);
        }
    }
    let rotated = start_at(ring, seam);
    let mut placed: Vec<((f64, f64), (P2, usize))> = along
        .iter()
        .map(|&(s, lift, q, at)| (((s - seam).rem_euclid(total) / total, lift), (q, at)))
        .collect();
    placed.sort_by(|a, b| a.0.0.total_cmp(&b.0.0));
    let (zones, points) = placed.into_iter().unzip();
    TabbedRing {
        ring: rotated,
        zones,
        points,
    }
}

// ---------------------------------------------------------------------------
// The one placement of generation and of the editor (geometry selection P4.6)
// ---------------------------------------------------------------------------

/// A hand tab of `tab_points`, snapped to the outline of the region it
/// names (setup frame).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HandTab {
    /// The index of the point in `tab_points`.
    pub point: usize,
    /// The index of the region in the op's regions.
    pub region: usize,
    /// The nearest point of the region exterior (setup frame).
    pub on_outline: P2,
}

/// The hand tabs `tab_points` (drawing frame) in the setup frame, each
/// snapped to the nearest exterior of `regions` (the loops a Profile
/// cuts). P6.3 (L-8): only a closed polygon takes a hand tab; an open path
/// of a Profile with side `On` is not a candidate. The index of a region
/// stays its index in `regions`, so the caller passes every line of the
/// op, open paths too. `setup` moves a drawing point into the setup frame
/// (`SetupTransformInfo::drawing_to_local`, as the frame door moves the
/// loops); `None` is the identity.
///
/// # Errors
///
/// The text that names a point farther than [`hand_tab_reach`] from every
/// exterior. Generation (`OperationError::InvalidParam`) and the `tab_layout`
/// query give this text, so the two refuse alike.
pub fn resolve_hand_tabs(
    tab_points: &[[f64; 2]],
    setup: Option<&crate::compute::transform::SetupTransformInfo>,
    tool_radius: f64,
    regions: &[crate::polygon::Polygon2],
) -> Result<Vec<HandTab>, String> {
    let reach = hand_tab_reach(tool_radius);
    tab_points
        .iter()
        .enumerate()
        .map(|(point, &[x, y])| {
            let drawn = P2::new(x, y);
            // The points are in the drawing frame, as the items are; the
            // setup moves them as it moves the loops (`to_setup_frame`).
            let local = setup.map_or(drawn, |info| info.drawing_to_local(drawn));
            let nearest = regions
                .iter()
                .enumerate()
                .filter(|(_, r)| r.closed)
                .filter_map(|(i, r)| nearest_on_loop(&r.exterior, local).map(|(q, _, d)| (i, q, d)))
                .min_by(|a, b| a.2.total_cmp(&b.2));
            match nearest {
                Some((region, on_outline, d)) if d <= reach => Ok(HandTab {
                    point,
                    region,
                    on_outline,
                }),
                _ => Err(format!(
                    "Profile: the hand tab at X{x:.3} Y{y:.3} is {} from every loop this \
                     operation cuts; a tab must be within {reach:.3} mm (one tool diameter) of a \
                     loop. Move the point onto the outline or delete it from tab_points.",
                    nearest.map_or_else(|| "far".to_owned(), |(_, _, d)| format!("{d:.3} mm"))
                )),
            }
        })
        .collect()
}

/// Where a planned tab comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TabSource {
    /// The automatic rule ([`auto_tabs`], or the even spacing).
    Auto,
    /// A hand tab: the index of its point in `tab_points`.
    Hand(usize),
}

/// One tab as the Profile cuts it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlannedTab {
    /// The centre of the lift on the cut ring (setup frame): the point
    /// that [`apply_tabs`] lifts round.
    pub at: P2,
    /// The centre on the outline loop (setup frame): the point that a hand
    /// tab at this place stores (after the inverse frame door).
    pub on_outline: P2,
    pub source: TabSource,
}

/// The cut of one region: its ring with the tabs, for the Profile pass,
/// and the tabs as points, for the editor.
#[derive(Debug, Clone, PartialEq)]
pub struct PlannedContour {
    /// The index of the region in the op's regions, or `None` when the
    /// piece is not one of them (it then takes no hand tab).
    pub region: Option<usize>,
    /// False for an open path of a Profile with side `On` (P6.3): the pass
    /// cuts `tabbed.ring` from its first point to its last point and does
    /// not close it. An open contour has no tab.
    pub closed: bool,
    /// The ring in cut order, its start in the largest gap, and its zones.
    pub tabbed: TabbedRing,
    /// One entry per zone of `tabbed`, in the same order.
    pub tabs: Vec<PlannedTab>,
}

/// The tab plan of the pieces of one `plan_profile_tabs` call.
#[derive(Debug, Clone, PartialEq)]
pub struct ProfileTabPlan {
    /// One contour per piece whose offset gives a ring, in piece order.
    pub contours: Vec<PlannedContour>,
    /// The loops whose automatic tabs did not follow the side rule.
    pub notes: Vec<crate::compute::toolpath_stats::TabNote>,
    /// The offset failures of the pieces (`profile_path_reported`).
    pub offset_failures: usize,
}

/// The tab rule of one Profile op.
#[derive(Debug, Clone, Copy)]
pub struct ProfileTabRule<'a> {
    /// Every region of the op: a hand tab names its region by index here.
    pub regions: &'a [crate::polygon::Polygon2],
    /// The hand tabs ([`resolve_hand_tabs`] over `regions`).
    pub hand: &'a [HandTab],
    /// The minimum count of automatic tabs per loop (`tab_count`).
    pub count: usize,
    /// The bridge width (`tab_width`).
    pub width: f64,
    /// The side, the tool radius, the climb and the compensation of the
    /// cut. The Z, feed and rapid fields are not read.
    pub profile: &'a crate::ops::profile::ProfileParams,
}

/// THE tab placement of a Profile (geometry selection P4.6): the cut ring
/// of each piece of `pieces` and its tabs. `ProfilePass::prepare` cuts
/// from it, and the `tab_layout` query reads it, so the marks of the
/// editor are the tabs that generation cuts.
///
/// The rule (P5): the hand tabs of a region replace its automatic tabs.
/// Without a hand tab, `count > 0` asks for the side rule ([`auto_tabs`]);
/// a loop with no side that holds a tab takes `count` tabs spaced evenly
/// by length on the cut ring, and a note says so. With `count == 0` and no
/// hand tab at all, no ring gets a tab and no ring moves its start.
#[must_use]
pub fn plan_profile_tabs(
    pieces: &[crate::polygon::Polygon2],
    rule: &ProfileTabRule<'_>,
) -> ProfileTabPlan {
    let tabs_on = rule.count > 0 || !rule.hand.is_empty();
    let mut plan = ProfileTabPlan {
        contours: Vec::new(),
        notes: Vec::new(),
        offset_failures: 0,
    };
    for piece in pieces {
        if rule.profile.side == crate::ops::profile::ProfileSide::On {
            on_line_tabs(piece, rule, tabs_on, &mut plan);
            continue;
        }
        let (contour, failures) = crate::ops::profile::profile_path_reported(piece, rule.profile);
        plan.offset_failures += failures;
        let Some(contour) = contour else { continue };
        let ring = crate::ops::profile::profile_ring(&contour, rule.profile.climb);
        // A Profile is not a `LevelClip` op, so the runner gives it the
        // region polygons whole: the piece finds its region by its exterior.
        let region = rule
            .regions
            .iter()
            .position(|r| r.exterior == piece.exterior);
        let planned = if tabs_on {
            contour_tabs(piece, region, &ring, rule, &mut plan.notes)
        } else {
            untabbed(region, ring, true)
        };
        plan.contours.push(planned);
    }
    plan
}

/// A contour with no tab: `ring` in cut order, its start as it is.
fn untabbed(region: Option<usize>, ring: Vec<P2>, closed: bool) -> PlannedContour {
    PlannedContour {
        region,
        closed,
        tabbed: TabbedRing {
            ring,
            zones: Vec::new(),
            points: Vec::new(),
        },
        tabs: Vec::new(),
    }
}

/// The lines of one piece of a Profile with side `On` (P6.3), added to
/// `plan` ([`crate::ops::profile::on_line_contours`]):
///
/// - each closed ring is cut on the line in the order of
///   [`crate::ops::profile::profile_ring`] (climb reverses it, L-1) and
///   takes tabs by the rule of [`plan_profile_tabs`]. The exterior is the
///   region of the piece and takes its hand tabs; a nested ring takes auto
///   tabs only;
/// - an open path is cut in its drawn direction (climb is not read, L-1)
///   and takes no tab (L-8). With `count > 0` it gives ONE
///   [`TabNote::OpenPath`](crate::compute::toolpath_stats::TabNote::OpenPath)
///   note.
///
/// No offset is made, so no offset failure is counted.
fn on_line_tabs(
    piece: &crate::polygon::Polygon2,
    rule: &ProfileTabRule<'_>,
    tabs_on: bool,
    plan: &mut ProfileTabPlan,
) {
    use crate::ops::profile::OnLineContour;
    let region = rule
        .regions
        .iter()
        .position(|r| r.closed == piece.closed && r.exterior == piece.exterior);
    for (index, contour) in crate::ops::profile::on_line_contours(piece)
        .into_iter()
        .enumerate()
    {
        match contour {
            OnLineContour::Open(path) => {
                if rule.count > 0
                    && let Some(first) = path.first()
                {
                    plan.notes
                        .push(crate::compute::toolpath_stats::TabNote::OpenPath {
                            start: [first.x, first.y],
                            asked: rule.count,
                        });
                }
                plan.contours.push(untabbed(None, path, false));
            }
            OnLineContour::Closed(ring_points) => {
                // The exterior comes first; a nested ring is no region of
                // the op, so it takes no hand tab.
                let (outline, ring_region) = if index == 0 {
                    (std::borrow::Cow::Borrowed(piece), region)
                } else {
                    (
                        std::borrow::Cow::Owned(crate::polygon::Polygon2::new(ring_points.clone())),
                        None,
                    )
                };
                let ring = crate::ops::profile::profile_ring(&ring_points, rule.profile.climb);
                let planned = if tabs_on {
                    contour_tabs(&outline, ring_region, &ring, rule, &mut plan.notes)
                } else {
                    untabbed(ring_region, ring, true)
                };
                plan.contours.push(planned);
            }
        }
    }
}

/// The tabs of the cut ring `ring` of `outline`, the region at index
/// `region` of the op ([`plan_profile_tabs`]).
fn contour_tabs(
    outline: &crate::polygon::Polygon2,
    region: Option<usize>,
    ring: &[P2],
    rule: &ProfileTabRule<'_>,
    notes: &mut Vec<crate::compute::toolpath_stats::TabNote>,
) -> PlannedContour {
    use crate::compute::toolpath_stats::TabNote;
    // (centre, the point on the outline, the source) per tab.
    let hand: Vec<(P2, P2, TabSource)> = rule
        .hand
        .iter()
        .filter(|h| Some(h.region) == region)
        .map(|h| (h.on_outline, h.on_outline, TabSource::Hand(h.point)))
        .collect();
    let centre = || {
        let (lo, hi) = outline.exterior.iter().fold(
            ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]),
            |(lo, hi), p| {
                (
                    [lo[0].min(p.x), lo[1].min(p.y)],
                    [hi[0].max(p.x), hi[1].max(p.y)],
                )
            },
        );
        [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0]
    };
    let loop_index = region.unwrap_or(usize::MAX);
    let asked = rule.count;
    let on_outline = |c: P2| nearest_on_loop(&outline.exterior, c).map_or(c, |(q, _, _)| q);
    let auto = |c: P2| (c, c, TabSource::Auto);
    let centres: Vec<(P2, P2, TabSource)> = if !hand.is_empty() {
        hand
    } else if asked == 0 {
        Vec::new()
    } else {
        match auto_tabs(
            &outline.exterior,
            asked,
            rule.width,
            rule.profile.tool_radius,
        ) {
            AutoTabs::OnSides { centres, .. } => {
                if centres.len() < asked {
                    notes.push(TabNote::FewerThanAsked {
                        loop_index,
                        centre: centre(),
                        placed: centres.len(),
                        asked,
                    });
                }
                // The side rule places on the outline itself.
                centres.into_iter().map(auto).collect()
            }
            AutoTabs::NoSide => {
                notes.push(TabNote::EvenSpacing {
                    loop_index,
                    centre: centre(),
                    count: asked,
                });
                even_centres(ring, asked)
                    .into_iter()
                    .map(|c| (c, on_outline(c), TabSource::Auto))
                    .collect()
            }
        }
    };
    let points: Vec<P2> = centres.iter().map(|&(c, _, _)| c).collect();
    let tabbed = tabbed_ring(ring, &points, rule.width, rule.profile.tool_radius);
    let tabs = tabbed
        .points
        .iter()
        .filter_map(|&(at, index)| {
            centres
                .get(index)
                .map(|&(_, on_outline, source)| PlannedTab {
                    at,
                    on_outline,
                    source,
                })
        })
        .collect();
    PlannedContour {
        region,
        closed: true,
        tabbed,
        tabs,
    }
}

/// The closed ring `ring` started at distance `s` along it. A start inside
/// a segment becomes a new vertex; a start on a vertex starts there.
#[allow(clippy::indexing_slicing)] // SAFETY: every index is taken modulo the length, or is below it by the loop bound
fn start_at(ring: &[P2], s: f64) -> Vec<P2> {
    let n = ring.len();
    let mut walked = 0.0;
    for (k, a) in ring.iter().enumerate() {
        let Some(b) = ring.get((k + 1) % n) else {
            continue;
        };
        let len = (b.x - a.x).hypot(b.y - a.y);
        if s >= walked + len && k + 1 < n {
            walked += len;
            continue;
        }
        let t = if len > 0.0 {
            ((s - walked) / len).clamp(0.0, 1.0)
        } else {
            0.0
        };
        // The vertex index the new ring starts at, and the inserted point
        // when the start is inside the segment.
        let (first, inserted) = if t * len < MIN_SEGMENT_MM {
            (k, None)
        } else if (1.0 - t) * len < MIN_SEGMENT_MM {
            ((k + 1) % n, None)
        } else {
            (
                k + 1,
                Some(P2::new(a.x + t * (b.x - a.x), a.y + t * (b.y - a.y))),
            )
        };
        let mut out = Vec::with_capacity(n + 1);
        out.extend(inserted);
        out.extend((0..n).map(|j| ring[(first + j) % n]));
        return out;
    }
    ring.to_vec()
}
