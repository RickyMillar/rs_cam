//! Holding tabs (bridges) of a profile (P5, S12 and S13): where they sit,
//! and the one lift emitter that cuts them ([`apply_tabs`]).
//!
//! Placement runs on the outline loop, the `Regions` item the user
//! selected, because a side and a corner are properties of the part. The
//! centres then go onto the ring the pass cuts ([`tabbed_ring`]), which
//! also moves the start of that ring into the largest gap between tabs. So
//! no tab zone crosses the plunge, and [`apply_tabs`] measures every tab
//! whole.

use super::Z_LEVEL_EPS_MM;
use crate::geo::{P2, P3};
use crate::toolpath::{MoveType, Toolpath};

/// A tab (bridge) that holds the part to the stock during profile cutting.
#[derive(Debug, Clone)]
pub struct Tab {
    /// Position along the polygon perimeter (0.0 to 1.0, fractional).
    pub position: f64,
    /// Width of the tab in mm.
    pub width: f64,
    /// Height of the tab in mm (how far above cut_depth the tab rises).
    pub height: f64,
}

/// Insert holding tabs into a profile toolpath.
///
#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// Tabs create sharp rectangular bridges: the cutter steps up to tab height,
/// traverses at that height, then steps back down. This leaves material
/// bridges that hold the part to the stock.
///
/// Tab positions are interpolated along cutting segments, so tabs appear
/// at the correct location even when move endpoints are sparse.
///
/// # Move intent (CUT-08)
///
/// Every rewritten segment keeps the [`MoveIntent`] of the move it replaces,
/// so a tabbed profile stays visible to each intent-keyed reader — the
/// spacing instrument (`metrology/spacing.rs`) skips a move that is not
/// `FinishingCut`. The two fed descents back to `cut_depth` after a tab are
/// [`MoveIntent::EntryPlunge`]: they enter material vertically, which is what
/// that tag means everywhere else in the crate.
///
/// The fed lift onto a tab keeps the cut intent. It removes material, and a
/// `Retract`-tagged feed is forbidden — see
/// `tests/retract_intent_move_type_census_w6.rs`.
pub fn apply_tabs(toolpath: Toolpath, tabs: &[Tab], cut_depth: f64) -> Toolpath {
    use crate::toolpath::MoveIntent;

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

    // Build sorted tab boundary events as absolute distances
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
                tab_z: cut_depth + tab.height,
            }
        })
        .collect();

    let tab_z_at_dist = |dist: f64| -> Option<f64> {
        tab_zones
            .iter()
            .find(|tz| dist >= tz.start_dist && dist <= tz.end_dist)
            .map(|tz| tz.tab_z)
    };

    // Walk through toolpath, interpolating tab boundaries along cutting segments
    let mut result = Toolpath::new();
    let mut in_tab = false;

    for (i, m) in toolpath.moves.iter().enumerate() {
        let cut_pos = cutting_indices.iter().position(|&ci| ci == i);

        if let Some(cp) = cut_pos {
            if cp == 0 {
                // First cutting move — just emit it
                result.moves.push(m.clone());
                in_tab = tab_z_at_dist(0.0).is_some();
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

            // Collect all tab boundary crossings within this segment
            let mut events: Vec<(f64, bool)> = Vec::new(); // (dist, is_entry)
            for tz in &tab_zones {
                if tz.start_dist > seg_start_dist && tz.start_dist < seg_end_dist {
                    events.push((tz.start_dist, true));
                }
                if tz.end_dist > seg_start_dist && tz.end_dist < seg_end_dist {
                    events.push((tz.end_dist, false));
                }
            }
            events.sort_by(|a, b| a.0.total_cmp(&b.0));

            if events.is_empty() {
                // No boundary crossings — whole segment is in or out
                let mid_dist = (seg_start_dist + seg_end_dist) / 2.0;
                if let Some(tab_z) = tab_z_at_dist(mid_dist) {
                    if !in_tab {
                        // Entered tab zone before this segment
                        if let Some(last) = result.moves.last() {
                            result.feed_to_with_intent(
                                P3::new(last.target.x, last.target.y, tab_z),
                                feed_rate,
                                m.intent,
                            );
                        }
                        in_tab = true;
                    }
                    result.feed_to_with_intent(
                        P3::new(curr_target.x, curr_target.y, tab_z),
                        feed_rate,
                        m.intent,
                    );
                } else {
                    if in_tab {
                        if let Some(last) = result.moves.last() {
                            result.feed_to_with_intent(
                                P3::new(last.target.x, last.target.y, cut_depth),
                                feed_rate,
                                MoveIntent::EntryPlunge,
                            );
                        }
                        in_tab = false;
                    }
                    result.moves.push(m.clone());
                }
            } else {
                // Process boundary crossings — split segment at each event
                let mut last_dist = seg_start_dist;

                for (event_dist, is_entry) in &events {
                    let t = (*event_dist - seg_start_dist) / seg_len;
                    let split_x = prev_target.x + t * (curr_target.x - prev_target.x);
                    let split_y = prev_target.y + t * (curr_target.y - prev_target.y);

                    if *is_entry {
                        // Emit segment up to tab entry at cut_depth
                        if !in_tab {
                            result.feed_to_with_intent(
                                P3::new(split_x, split_y, cut_depth),
                                feed_rate,
                                m.intent,
                            );
                        }
                        // Step up
                        let tab_z = tab_z_at_dist(*event_dist + 0.01).unwrap_or(cut_depth + 2.0);
                        result.feed_to_with_intent(
                            P3::new(split_x, split_y, tab_z),
                            feed_rate,
                            m.intent,
                        );
                        in_tab = true;
                    } else {
                        // Emit segment up to tab exit at tab height
                        let tab_z = tab_z_at_dist(last_dist + 0.01).unwrap_or(cut_depth + 2.0);
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
                        in_tab = false;
                    }
                    last_dist = *event_dist;
                }

                // Emit remainder of segment after last event
                if in_tab {
                    let tab_z = tab_z_at_dist(last_dist + 0.01).unwrap_or(cut_depth + 2.0);
                    result.feed_to_with_intent(
                        P3::new(curr_target.x, curr_target.y, tab_z),
                        feed_rate,
                        m.intent,
                    );
                } else {
                    result.feed_to_with_intent(
                        P3::new(curr_target.x, curr_target.y, cut_depth),
                        feed_rate,
                        m.intent,
                    );
                }
            }
        } else {
            // Non-cutting move — pass through unchanged
            in_tab = false;
            result.moves.push(m.clone());
        }
    }

    result
}

/// Generate evenly-spaced tabs around a perimeter.
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

/// The clearance (mm) between the edge of a tab and the nearest corner of
/// its side.
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
    let Some(last) = segs.last() else {
        return ring.first().copied().unwrap_or_else(|| P2::new(0.0, 0.0));
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
///    `tab_width + 2 · tab_corner_clearance`. Each such side gets one tab.
/// 2. The total is `max(min_count, sides)`: the count of the op is a
///    minimum, and the one-per-side rule wins over a smaller count
///    (operator ruling 2026-10-16: at least one tab per side).
/// 3. Each further tab goes to the side with the largest share of free
///    length, `eligible / (tabs + 1)` (ties: the first side in loop
///    order), while the gap between two tabs stays at least the corner
///    clearance. A long side so takes more tabs than a short one.
/// 4. The tabs of one side sit at the centres of equal shares of the
///    length that is clear of its corners.
#[must_use]
#[allow(clippy::indexing_slicing)] // SAFETY: every index is taken modulo the length, or is below it by the loop bound
pub fn auto_tabs(outline: &[P2], min_count: usize, tab_width: f64, tool_radius: f64) -> AutoTabs {
    let clearance = tab_corner_clearance(tab_width, tool_radius);
    // (eligible start, eligible length) of the centre of a tab per side.
    let eligible: Vec<(f64, f64)> = straight_sides(outline)
        .iter()
        .filter(|s| s.length() >= tab_width + 2.0 * clearance)
        .map(|s| {
            let lo = s.start + clearance + tab_width / 2.0;
            (lo, s.end - clearance - tab_width / 2.0 - lo)
        })
        .collect();
    if eligible.is_empty() {
        return AutoTabs::NoSide;
    }
    let capacity: Vec<usize> = eligible
        .iter()
        .map(|&(_, len)| ((len / (tab_width + clearance)).floor() as usize).max(1))
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
    /// One position per centre, as a fraction of the length of `ring`
    /// ([`Tab::position`]), in loop order.
    pub positions: Vec<f64>,
}

impl TabbedRing {
    /// The tabs of this ring for [`apply_tabs`].
    #[must_use]
    pub fn tabs(&self, width: f64, height: f64) -> Vec<Tab> {
        self.positions
            .iter()
            .map(|&position| Tab {
                position,
                width,
                height,
            })
            .collect()
    }
}

/// Put each centre on the closed cut ring `ring` (its nearest point) and
/// start the ring in the middle of the largest gap between the tab zones.
///
/// [`apply_tabs`] measures a tab zone on `[0, length]` and clips it there,
/// and it plunges at the ring start. A zone across the start would be cut
/// short and drilled through. With the start in a gap, every zone is whole
/// (when the tabs leave a gap at all).
#[must_use]
#[allow(clippy::indexing_slicing)] // SAFETY: every index is taken modulo the length, or is below it by the loop bound
pub fn tabbed_ring(ring: &[P2], centres: &[P2], tab_width: f64) -> TabbedRing {
    let (_, total) = segments(ring);
    let mut along: Vec<f64> = centres
        .iter()
        .filter_map(|&c| nearest_on_loop(ring, c).map(|(_, s, _)| s))
        .collect();
    if along.is_empty() || total <= 0.0 {
        return TabbedRing {
            ring: ring.to_vec(),
            positions: Vec::new(),
        };
    }
    along.sort_by(f64::total_cmp);
    // The largest gap between one zone end and the next zone start, round
    // the loop. Ties: the first.
    let mut seam = 0.0;
    let mut widest = f64::NEG_INFINITY;
    for (i, &s) in along.iter().enumerate() {
        let next = along.get(i + 1).copied().unwrap_or(along[0] + total);
        let gap = (next - tab_width / 2.0) - (s + tab_width / 2.0);
        if gap > widest {
            widest = gap;
            seam = (s + tab_width / 2.0 + gap / 2.0).rem_euclid(total);
        }
    }
    let rotated = start_at(ring, seam);
    let mut positions: Vec<f64> = along
        .iter()
        .map(|&s| (s - seam).rem_euclid(total) / total)
        .collect();
    positions.sort_by(f64::total_cmp);
    TabbedRing {
        ring: rotated,
        positions,
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
