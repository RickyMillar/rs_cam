//! Toolpath dressups — post-processing transforms applied to toolpaths.
//!
//! Dressups modify an existing toolpath without changing the core operation.
//! They compose: you can chain multiple dressups on the same toolpath.
//!
//! - **Ramp entry**: Replace vertical plunges with helical or ramped entry
//! - **Tab/bridge**: Insert material tabs to hold parts during profile cutting

use crate::dexel_stock::TriDexelStock;
use crate::geo::P3;
use crate::toolpath::{Move, MoveType, Toolpath};

// ---------------------------------------------------------------------------
// Ramp / Helix entry
// ---------------------------------------------------------------------------

/// Strategy for entering material (replacing straight plunges).
#[derive(Debug, Clone, Copy)]
pub enum EntryStyle {
    /// Linear ramp: plunge at an angle along the next cutting direction.
    /// `max_angle_deg` is the maximum ramp angle from horizontal (e.g., 3.0°).
    Ramp { max_angle_deg: f64 },
    /// Helical entry: spiral down at the plunge point.
    /// `radius` is the helix radius (mm), `pitch` is Z drop per revolution (mm).
    Helix { radius: f64, pitch: f64 },
}

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// Replace straight plunges in a toolpath with ramped or helical entries.
///
/// A "plunge" is detected as a feed move that goes from safe_z (or higher)
/// down to cutting depth with no XY movement.
pub fn apply_entry(toolpath: &Toolpath, style: EntryStyle, plunge_rate: f64) -> Toolpath {
    let mut result = Toolpath::new();

    let mut i = 0;
    while i < toolpath.moves.len() {
        let m = &toolpath.moves[i];

        // Detect a plunge: Linear move that goes downward with no XY change
        if let MoveType::Linear { feed_rate } = m.move_type
            && i > 0
            && is_plunge(&toolpath.moves[i - 1], m)
        {
            match style {
                EntryStyle::Ramp { max_angle_deg } => {
                    // Look ahead for the next XY move to determine ramp direction
                    let ramp_dir = find_next_xy_direction(&toolpath.moves, i);
                    emit_ramp(
                        &mut result,
                        &toolpath.moves[i - 1].target,
                        &m.target,
                        ramp_dir,
                        max_angle_deg,
                        feed_rate.min(plunge_rate),
                    );
                }
                EntryStyle::Helix { radius, pitch } => {
                    emit_helix(
                        &mut result,
                        &toolpath.moves[i - 1].target,
                        &m.target,
                        radius,
                        pitch,
                        feed_rate.min(plunge_rate),
                    );
                }
            }
            i += 1;
            continue;
        }

        result.moves.push(m.clone());
        i += 1;
    }

    result
}

fn is_plunge(prev: &Move, current: &Move) -> bool {
    if let MoveType::Linear { .. } = current.move_type {
        let dz = current.target.z - prev.target.z;
        let pdx = current.target.x - prev.target.x;
        let pdy = current.target.y - prev.target.y;
        let dxy = (pdx * pdx + pdy * pdy).sqrt();
        // Downward move with negligible XY movement
        dz < -0.1 && dxy < 0.01
    } else {
        false
    }
}

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
fn find_next_xy_direction(moves: &[Move], from_idx: usize) -> (f64, f64) {
    let base = &moves[from_idx].target;
    for m in &moves[from_idx + 1..] {
        let dx = m.target.x - base.x;
        let dy = m.target.y - base.y;
        let dist = (dx * dx + dy * dy).sqrt();
        if dist > 0.1 {
            return (dx / dist, dy / dist);
        }
    }
    (1.0, 0.0) // fallback: ramp along X
}

/// Clearance height (mm) above cut depth to start ramping/helixing.
pub(crate) const ENTRY_CLEARANCE: f64 = 2.0;

pub(crate) fn emit_ramp(
    tp: &mut Toolpath,
    start: &P3,
    end: &P3,
    dir: (f64, f64),
    max_angle_deg: f64,
    feed_rate: f64,
) {
    // Only ramp the last portion of the descent (max 5mm above target).
    // Rapid down to clearance first, then ramp the rest.
    let clearance = ENTRY_CLEARANCE;
    let ramp_start_z = end.z + clearance;

    if start.z > ramp_start_z + 0.1 {
        // Rapid down to clearance height first
        tp.rapid_to(P3::new(start.x, start.y, ramp_start_z));
    }

    let ramp_dz = (ramp_start_z - end.z).abs().max(0.1);
    let max_angle_rad = max_angle_deg.to_radians();
    let ramp_xy_len = ramp_dz / max_angle_rad.tan();

    // Ramp out along direction, then back (zigzag ramp)
    let half_len = ramp_xy_len / 2.0;
    let mid_z = (ramp_start_z + end.z) / 2.0;

    // Move forward and down to midpoint
    tp.feed_to(
        P3::new(
            start.x + dir.0 * half_len,
            start.y + dir.1 * half_len,
            mid_z,
        ),
        feed_rate,
    );
    // Move back to start XY at final Z
    tp.feed_to(P3::new(start.x, start.y, end.z), feed_rate);
}

pub(crate) fn emit_helix(
    tp: &mut Toolpath,
    start: &P3,
    end: &P3,
    radius: f64,
    pitch: f64,
    feed_rate: f64,
) {
    // Only helix the last portion — rapid down to clearance first
    let clearance = ENTRY_CLEARANCE;
    let helix_start_z = end.z + clearance;

    if start.z > helix_start_z + 0.1 {
        tp.rapid_to(P3::new(start.x, start.y, helix_start_z));
    }

    let dz = (helix_start_z.min(start.z) - end.z).abs();
    if dz < 0.01 || pitch < 0.01 {
        tp.feed_to(*end, feed_rate);
        return;
    }

    let revolutions = dz / pitch;
    let total_angle = revolutions * std::f64::consts::TAU;
    let steps_per_rev = 36; // 10° per step
    let total_steps = (revolutions * steps_per_rev as f64).ceil() as usize;
    if total_steps == 0 {
        tp.feed_to(*end, feed_rate);
        return;
    }

    let center_x = end.x;
    let center_y = end.y;
    let helix_top = helix_start_z.min(start.z);

    for i in 1..=total_steps {
        let t = i as f64 / total_steps as f64;
        let angle = total_angle * t;
        let z = helix_top - dz * t;
        let (sin_a, cos_a) = angle.sin_cos();
        let x = center_x + radius * cos_a;
        let y = center_y + radius * sin_a;
        tp.feed_to(P3::new(x, y, z), feed_rate);
    }

    // Return to center at final Z
    tp.feed_to(*end, feed_rate);
}

// ---------------------------------------------------------------------------
// Tab / Bridge dressup
// ---------------------------------------------------------------------------

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
pub fn apply_tabs(toolpath: &Toolpath, tabs: &[Tab], cut_depth: f64) -> Toolpath {
    if tabs.is_empty() {
        return toolpath.clone();
    }

    // Collect cutting move indices at cut_depth
    let cutting_indices: Vec<usize> = toolpath
        .moves
        .iter()
        .enumerate()
        .filter(|(_, m)| {
            matches!(m.move_type, MoveType::Linear { .. }) && (m.target.z - cut_depth).abs() < 0.01
        })
        .map(|(i, _)| i)
        .collect();

    if cutting_indices.len() < 2 {
        return toolpath.clone();
    }

    // Compute cumulative distance at each cutting move endpoint
    let mut cum_dist = vec![0.0_f64];
    for i in 1..cutting_indices.len() {
        let prev = &toolpath.moves[cutting_indices[i - 1]].target;
        let curr = &toolpath.moves[cutting_indices[i]].target;
        let ddx = curr.x - prev.x;
        let ddy = curr.y - prev.y;
        let d = (ddx * ddx + ddy * ddy).sqrt();
        cum_dist.push(cum_dist.last().expect("cum_dist starts with [0.0]") + d);
    }

    let total_dist = *cum_dist.last().unwrap_or(&0.0);
    if total_dist < 1e-6 {
        return toolpath.clone();
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
                            result.feed_to(P3::new(last.target.x, last.target.y, tab_z), feed_rate);
                        }
                        in_tab = true;
                    }
                    result.feed_to(P3::new(curr_target.x, curr_target.y, tab_z), feed_rate);
                } else {
                    if in_tab {
                        if let Some(last) = result.moves.last() {
                            result.feed_to(
                                P3::new(last.target.x, last.target.y, cut_depth),
                                feed_rate,
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
                            result.feed_to(P3::new(split_x, split_y, cut_depth), feed_rate);
                        }
                        // Step up
                        let tab_z = tab_z_at_dist(*event_dist + 0.01).unwrap_or(cut_depth + 2.0);
                        result.feed_to(P3::new(split_x, split_y, tab_z), feed_rate);
                        in_tab = true;
                    } else {
                        // Emit segment up to tab exit at tab height
                        let tab_z = tab_z_at_dist(last_dist + 0.01).unwrap_or(cut_depth + 2.0);
                        result.feed_to(P3::new(split_x, split_y, tab_z), feed_rate);
                        // Step down
                        result.feed_to(P3::new(split_x, split_y, cut_depth), feed_rate);
                        in_tab = false;
                    }
                    last_dist = *event_dist;
                }

                // Emit remainder of segment after last event
                if in_tab {
                    let tab_z = tab_z_at_dist(last_dist + 0.01).unwrap_or(cut_depth + 2.0);
                    result.feed_to(P3::new(curr_target.x, curr_target.y, tab_z), feed_rate);
                } else {
                    result.feed_to(P3::new(curr_target.x, curr_target.y, cut_depth), feed_rate);
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

// ---------------------------------------------------------------------------
// Lead-in / Lead-out dressup
// ---------------------------------------------------------------------------

/// Insert arc lead-in and lead-out moves at the start/end of cutting passes.
///
#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// A "cutting pass" is a sequence of feed moves at the same Z bounded by
/// rapids or plunges. The lead-in is a quarter-circle arc that approaches
/// the first cut point tangentially (avoiding a witness mark from a direct
/// plunge). The lead-out is a matching arc departing the last cut point.
///
/// `radius` is the arc radius in mm (typically 1-3mm or ~half the tool radius).
pub fn apply_lead_in_out(toolpath: &Toolpath, radius: f64) -> Toolpath {
    let mut result = Toolpath::new();
    let moves = &toolpath.moves;
    if moves.is_empty() {
        return result;
    }

    let mut i = 0;
    while i < moves.len() {
        // Detect the start of a cutting pass: a plunge (downward feed) followed
        // by horizontal feed moves at the same Z.
        if i > 0 && is_plunge(&moves[i - 1], &moves[i]) {
            let cut_z = moves[i].target.z;
            let plunge_end = moves[i].target;

            // Find next horizontal cutting move to determine lead-in direction
            if let Some(first_cut_idx) = (i + 1..moves.len()).find(|&j| {
                matches!(moves[j].move_type, MoveType::Linear { .. })
                    && (moves[j].target.z - cut_z).abs() < 0.01
            }) {
                let cut_dir_x = moves[first_cut_idx].target.x - plunge_end.x;
                let cut_dir_y = moves[first_cut_idx].target.y - plunge_end.y;
                let cut_dir_len = (cut_dir_x * cut_dir_x + cut_dir_y * cut_dir_y).sqrt();

                if cut_dir_len > 0.1 {
                    let ux = cut_dir_x / cut_dir_len;
                    let uy = cut_dir_y / cut_dir_len;

                    // Lead-in: approach from the side, quarter-circle arc
                    // Start point is offset perpendicular to cut direction
                    let perp_x = -uy;
                    let perp_y = ux;
                    let lead_start = P3::new(
                        plunge_end.x + perp_x * radius - ux * radius,
                        plunge_end.y + perp_y * radius - uy * radius,
                        cut_z,
                    );

                    let feed_rate = match moves[i].move_type {
                        MoveType::Linear { feed_rate } => feed_rate,
                        _ => 500.0,
                    };

                    // Plunge to lead-in start instead of original plunge point
                    result.feed_to(lead_start, feed_rate);

                    // Arc from lead_start to plunge_end (quarter circle)
                    let arc_steps = 8;
                    for s in 1..=arc_steps {
                        let t = s as f64 / arc_steps as f64;
                        let angle = std::f64::consts::FRAC_PI_2 * t;
                        let (sin_a, cos_a) = angle.sin_cos();
                        let ax = plunge_end.x + perp_x * radius * (1.0 - sin_a)
                            - ux * radius * (1.0 - cos_a);
                        let ay = plunge_end.y + perp_y * radius * (1.0 - sin_a)
                            - uy * radius * (1.0 - cos_a);
                        result.feed_to(P3::new(ax, ay, cut_z), feed_rate);
                    }

                    i += 1;
                    continue;
                }
            }
        }

        // Detect end of a cutting pass: feed at cut_z followed by retract (rapid up)
        if i + 1 < moves.len()
            && matches!(moves[i].move_type, MoveType::Linear { .. })
            && moves[i + 1].move_type == MoveType::Rapid
            && moves[i + 1].target.z > moves[i].target.z + 1.0
        {
            let cut_z = moves[i].target.z;
            let cut_end = moves[i].target;

            // Find the direction of the last cutting segment
            if i > 0 {
                let prev = moves[i - 1].target;
                let dir_x = cut_end.x - prev.x;
                let dir_y = cut_end.y - prev.y;
                let dir_len = (dir_x * dir_x + dir_y * dir_y).sqrt();

                if dir_len > 0.1 && (prev.z - cut_z).abs() < 0.01 {
                    let ux = dir_x / dir_len;
                    let uy = dir_y / dir_len;
                    let perp_x = -uy;
                    let perp_y = ux;

                    let feed_rate = match moves[i].move_type {
                        MoveType::Linear { feed_rate } => feed_rate,
                        _ => 1000.0,
                    };

                    // Emit the original cut endpoint
                    result.moves.push(moves[i].clone());

                    // Lead-out: quarter-circle arc departing tangentially
                    let arc_steps = 8;
                    for s in 1..=arc_steps {
                        let t = s as f64 / arc_steps as f64;
                        let angle = std::f64::consts::FRAC_PI_2 * t;
                        let (sin_a, cos_a) = angle.sin_cos();
                        let ax = cut_end.x + ux * radius * sin_a + perp_x * radius * (1.0 - cos_a);
                        let ay = cut_end.y + uy * radius * sin_a + perp_y * radius * (1.0 - cos_a);
                        result.feed_to(P3::new(ax, ay, cut_z), feed_rate);
                    }

                    i += 1;
                    continue;
                }
            }
        }

        result.moves.push(moves[i].clone());
        i += 1;
    }

    result
}

// ---------------------------------------------------------------------------
// Dogbone / overcut dressup
// ---------------------------------------------------------------------------

/// Insert dogbone overcuts at inside corners of a toolpath.
///
#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// At each corner sharper than `max_angle_deg`, a small extension is cut
/// along the corner bisector so that a mating part with a sharp corner can
/// fit into the CNC-cut pocket. The overcut distance is `tool_radius`,
/// creating a clearance notch at each inside corner.
///
/// Only operates on consecutive linear feed moves at the same Z.
pub fn apply_dogbones(toolpath: &Toolpath, tool_radius: f64, max_angle_deg: f64) -> Toolpath {
    let max_angle_rad = max_angle_deg.to_radians();
    let mut result = Toolpath::new();

    let moves = &toolpath.moves;
    if moves.len() < 3 {
        return toolpath.clone();
    }

    result.moves.push(moves[0].clone());

    for i in 1..moves.len() - 1 {
        result.moves.push(moves[i].clone());

        // Only process consecutive linear feed moves at the same Z
        let is_linear = |m: &Move| matches!(m.move_type, MoveType::Linear { .. });
        if !is_linear(&moves[i - 1]) || !is_linear(&moves[i]) || !is_linear(&moves[i + 1]) {
            continue;
        }

        let a = moves[i - 1].target;
        let b = moves[i].target;
        let c = moves[i + 1].target;

        // Must be at same Z (cutting depth)
        if (a.z - b.z).abs() > 0.01 || (b.z - c.z).abs() > 0.01 {
            continue;
        }

        // Compute edge vectors
        let v1x = b.x - a.x;
        let v1y = b.y - a.y;
        let v2x = c.x - b.x;
        let v2y = c.y - b.y;
        let len1 = (v1x * v1x + v1y * v1y).sqrt();
        let len2 = (v2x * v2x + v2y * v2y).sqrt();

        if len1 < 1e-10 || len2 < 1e-10 {
            continue;
        }

        // Normalize
        let u1x = v1x / len1;
        let u1y = v1y / len1;
        let u2x = v2x / len2;
        let u2y = v2y / len2;

        // Corner angle via dot product of forward directions
        let dot = u1x * u2x + u1y * u2y;
        let angle = dot.clamp(-1.0, 1.0).acos(); // 0 = straight, π = U-turn

        // Skip if not a sharp enough corner
        if angle < (std::f64::consts::PI - max_angle_rad) {
            continue;
        }

        // Bisector direction: average of the two "away from B" directions
        // Points into the material at the corner
        let bx = -u1x + u2x;
        let by = -u1y + u2y;
        let blen = (bx * bx + by * by).sqrt();
        if blen < 1e-10 {
            continue; // degenerate (straight line or U-turn)
        }

        // Dogbone direction is OPPOSITE to the bisector of the forward vectors
        // (pointing into the outside of the corner, i.e., into material)
        let dx = -(bx / blen);
        let dy = -(by / blen);

        let feed_rate = match moves[i].move_type {
            MoveType::Linear { feed_rate } => feed_rate,
            _ => 1000.0,
        };

        // Cut to overcut point and back
        let overcut_x = b.x + dx * tool_radius;
        let overcut_y = b.y + dy * tool_radius;
        result.feed_to(P3::new(overcut_x, overcut_y, b.z), feed_rate);
        result.feed_to(b, feed_rate);
    }

    // Add last move
    if moves.len() >= 2 {
        result.moves.push(moves[moves.len() - 1].clone());
    }

    result
}

// ---------------------------------------------------------------------------
// Link-vs-Retract dressup
// ---------------------------------------------------------------------------

/// Parameters for the link-move optimization.
pub struct LinkMoveParams {
    /// Maximum XY distance between passes to replace retract with direct feed.
    /// Default: 3× tool_diameter.
    pub max_link_distance: f64,
    /// Feed rate for link moves (mm/min).
    pub link_feed_rate: f64,
    /// Z threshold: moves to Z at or above this are considered rapids/retracts.
    pub safe_z_threshold: f64,
}

/// Replace short retract→rapid→plunge sequences with direct feed moves.
///
/// Detects 3-move windows of (retract to safe_z, rapid reposition, plunge to cut_z)
#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// where the XY distance is short, and replaces them with a single feed move.
///
/// Safety rules:
/// - Never links the first entry (tool hasn't cut yet)
/// - Only links when cut Z before and after are within 0.1mm (same depth level)
/// - max_link_distance caps risk
pub fn apply_link_moves(toolpath: &Toolpath, params: &LinkMoveParams) -> Toolpath {
    let moves = &toolpath.moves;
    if moves.len() < 4 {
        return toolpath.clone();
    }

    let mut result = Toolpath::new();
    let mut i = 0;
    let mut has_cut = false; // Track whether we've done any cutting yet

    while i < moves.len() {
        let m = &moves[i];

        // Track first cut — never link before the tool has engaged material
        if !has_cut {
            if matches!(m.move_type, MoveType::Linear { .. })
                && m.target.z < params.safe_z_threshold - 1.0
            {
                has_cut = true;
            }
            result.moves.push(m.clone());
            i += 1;
            continue;
        }

        // Look for retract→rapid→plunge pattern:
        // moves[i]   = Rapid to (x1, y1, safe_z)      — retract
        // moves[i+1] = Rapid to (x2, y2, safe_z)      — reposition
        // moves[i+2] = Linear to (x2, y2, cut_z)      — plunge
        if i + 2 < moves.len()
            && m.move_type == MoveType::Rapid
            && m.target.z >= params.safe_z_threshold - 0.1
            && moves[i + 1].move_type == MoveType::Rapid
            && moves[i + 1].target.z >= params.safe_z_threshold - 0.1
            && matches!(moves[i + 2].move_type, MoveType::Linear { .. })
            && moves[i + 2].target.z < params.safe_z_threshold - 1.0
        {
            let plunge_target = moves[i + 2].target;

            // Get the Z of the last cut move before this retract
            let prev_cut_z = result
                .moves
                .iter()
                .rev()
                .find(|mv| {
                    matches!(mv.move_type, MoveType::Linear { .. })
                        && mv.target.z < params.safe_z_threshold - 1.0
                })
                .map(|mv| mv.target.z);

            if let Some(prev_z) = prev_cut_z {
                // Check same depth level (within 0.1mm)
                if (prev_z - plunge_target.z).abs() < 0.1 {
                    // Check XY distance
                    let prev_pos = result.moves.last().map(|mv| &mv.target);
                    if let Some(prev) = prev_pos {
                        let dx = moves[i + 1].target.x - prev.x;
                        let dy = moves[i + 1].target.y - prev.y;
                        let dist = (dx * dx + dy * dy).sqrt();

                        if dist < params.max_link_distance {
                            // Replace: skip retract and rapid, emit direct feed to plunge target
                            result.feed_to(plunge_target, params.link_feed_rate);
                            i += 3; // Skip retract, rapid, plunge
                            continue;
                        }
                    }
                }
            }
        }

        result.moves.push(m.clone());
        i += 1;
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
// Air-cut filter dressup
// ---------------------------------------------------------------------------

/// Check if position (x, y, z) is in air (no material above z at this XY).
fn is_in_air(stock: &TriDexelStock, x: f64, y: f64, z: f64, tolerance: f64) -> bool {
    if let Some((row, col)) = stock.z_grid.world_to_cell(x, y) {
        match stock.z_grid.top_z_at(row, col) {
            Some(top) => (top as f64) < z - tolerance,
            None => true, // Empty ray = through-hole = definitely air
        }
    } else {
        true // Outside stock bounds = air
    }
}

/// Remove cutting moves that pass through empty stock (no remaining material).
///
/// For each cutting move, checks if material exists at the move's position in
/// the prior stock. Moves where both endpoints are in air (no material above
#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// the cutting Z) are converted to rapids. This is conservative — moves that
/// partially contact material are preserved.
///
/// Arc moves additionally check the arc center point for extra conservatism.
///
/// `tool_radius` is currently reserved for future per-cell radius checks.
pub fn filter_air_cuts(
    toolpath: &Toolpath,
    prior_stock: &TriDexelStock,
    _tool_radius: f64,
    safe_z: f64,
    tolerance: f64,
) -> Toolpath {
    let moves = &toolpath.moves;
    if moves.is_empty() {
        return toolpath.clone();
    }

    // Phase 1: classify each move as "in air" or not.
    // A move is "fully in air" when both its source and target positions are
    // above the remaining stock surface.
    let mut air_flags: Vec<bool> = Vec::with_capacity(moves.len());

    for (i, m) in moves.iter().enumerate() {
        // Rapids always pass through unchanged (they are already non-cutting).
        if m.move_type == MoveType::Rapid {
            air_flags.push(false);
            continue;
        }

        let target_air = is_in_air(prior_stock, m.target.x, m.target.y, m.target.z, tolerance);

        let source_air = if i > 0 {
            let prev = &moves[i - 1].target;
            is_in_air(prior_stock, prev.x, prev.y, prev.z, tolerance)
        } else {
            true // No source → treat as air
        };

        // For arcs, also check the center point (conservative: if center has
        // material, keep the move).
        let center_air = match m.move_type {
            MoveType::ArcCW { i: io, j: jo, .. } | MoveType::ArcCCW { i: io, j: jo, .. } => {
                if i > 0 {
                    let prev = &moves[i - 1].target;
                    let cx = prev.x + io;
                    let cy = prev.y + jo;
                    is_in_air(prior_stock, cx, cy, m.target.z, tolerance)
                } else {
                    true
                }
            }
            _ => true,
        };

        air_flags.push(source_air && target_air && center_air);
    }

    // Phase 2: emit the filtered toolpath.
    // Consecutive air moves are collapsed into: rapid up to safe_z, then
    // before the next non-air move, rapid to its XY at safe_z and rapid down.
    let mut result = Toolpath::new();
    let mut in_air_run = false;

    for (i, m) in moves.iter().enumerate() {
        if air_flags[i] {
            // This cutting move is in air.
            if !in_air_run {
                // Start of a new air run: emit rapid up to safe_z from
                // the current position.
                let prev_target = if let Some(last) = result.moves.last() {
                    last.target
                } else if i > 0 {
                    moves[i - 1].target
                } else {
                    m.target
                };
                if prev_target.z < safe_z - 0.001 {
                    result.rapid_to(P3::new(prev_target.x, prev_target.y, safe_z));
                }
                in_air_run = true;
            }
            // Skip this move (intermediate air moves are dropped).
        } else {
            // This move is NOT in air (or is a rapid).
            if in_air_run {
                // End of an air run: rapid to the target's XY at safe_z,
                // then rapid down to the target's Z (so the next cutting
                // move starts at the right position).
                //
                // For rapids we just emit them directly. For cutting moves
                // we need the positioning sequence.
                if m.move_type != MoveType::Rapid {
                    // Determine position we need to reach before this cutting
                    // move starts. The move's source is the previous move's
                    // target in the *original* toolpath.
                    let source = if i > 0 { moves[i - 1].target } else { m.target };
                    result.rapid_to(P3::new(source.x, source.y, safe_z));
                    // Rapid down to the source Z so the feed move distance
                    // is correct.
                    if source.z < safe_z - 0.001 {
                        result.rapid_to(source);
                    }
                }
                in_air_run = false;
            }
            result.moves.push(m.clone());
        }
    }

    // If the toolpath ends while still in an air run, no further action is
    // needed — we already emitted the retract at the start of the air run.

    result
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn simple_plunge_toolpath() -> Toolpath {
        let mut tp = Toolpath::new();
        // Rapid to start
        tp.rapid_to(P3::new(10.0, 10.0, 10.0));
        // Plunge straight down
        tp.feed_to(P3::new(10.0, 10.0, -3.0), 500.0);
        // Cut along X
        tp.feed_to(P3::new(50.0, 10.0, -3.0), 1000.0);
        tp.feed_to(P3::new(50.0, 50.0, -3.0), 1000.0);
        // Retract
        tp.rapid_to(P3::new(50.0, 50.0, 10.0));
        tp
    }

    // --- Ramp entry tests ---

    #[test]
    fn test_ramp_entry_replaces_plunge() {
        let tp = simple_plunge_toolpath();
        let result = apply_entry(&tp, EntryStyle::Ramp { max_angle_deg: 3.0 }, 500.0);

        // Should not have a straight plunge (large Z drop with no XY movement)
        for i in 1..result.moves.len() {
            if let MoveType::Linear { .. } = result.moves[i].move_type {
                let prev = &result.moves[i - 1].target;
                let curr = &result.moves[i].target;
                let dz = (curr.z - prev.z).abs();
                let tdx = curr.x - prev.x;
                let tdy = curr.y - prev.y;
                let dxy = (tdx * tdx + tdy * tdy).sqrt();
                if dz > 1.0 {
                    assert!(
                        dxy > 0.1,
                        "Ramp should have XY movement during Z descent: dz={}, dxy={}",
                        dz,
                        dxy
                    );
                }
            }
        }
    }

    #[test]
    fn test_ramp_entry_reaches_target_z() {
        let tp = simple_plunge_toolpath();
        let result = apply_entry(&tp, EntryStyle::Ramp { max_angle_deg: 5.0 }, 500.0);

        // Should still reach the cutting depth
        let has_cut_depth = result
            .moves
            .iter()
            .any(|m| (m.target.z - -3.0).abs() < 0.01);
        assert!(has_cut_depth, "Ramp should reach cut_depth=-3.0");
    }

    #[test]
    fn test_ramp_preserves_cutting_moves() {
        let tp = simple_plunge_toolpath();
        let result = apply_entry(&tp, EntryStyle::Ramp { max_angle_deg: 3.0 }, 500.0);

        // The cutting moves at -3.0 should still be present
        let cut_moves: Vec<_> = result
            .moves
            .iter()
            .filter(|m| {
                matches!(m.move_type, MoveType::Linear { feed_rate } if (feed_rate - 1000.0).abs() < 1e-10)
            })
            .collect();
        assert!(cut_moves.len() >= 2, "Cutting moves should be preserved");
    }

    // --- Helix entry tests ---

    #[test]
    fn test_helix_entry_replaces_plunge() {
        let tp = simple_plunge_toolpath();
        let result = apply_entry(
            &tp,
            EntryStyle::Helix {
                radius: 2.0,
                pitch: 1.0,
            },
            500.0,
        );

        // Should have many intermediate moves (helix steps)
        assert!(
            result.moves.len() > tp.moves.len(),
            "Helix should add intermediate moves: {} vs {}",
            result.moves.len(),
            tp.moves.len()
        );
    }

    #[test]
    fn test_helix_entry_reaches_target_z() {
        let tp = simple_plunge_toolpath();
        let result = apply_entry(
            &tp,
            EntryStyle::Helix {
                radius: 2.0,
                pitch: 1.0,
            },
            500.0,
        );

        let has_cut_depth = result.moves.iter().any(|m| (m.target.z - -3.0).abs() < 0.1);
        assert!(has_cut_depth, "Helix should reach cut_depth=-3.0");
    }

    #[test]
    fn test_helix_moves_are_circular() {
        let tp = simple_plunge_toolpath();
        let result = apply_entry(
            &tp,
            EntryStyle::Helix {
                radius: 3.0,
                pitch: 1.0,
            },
            500.0,
        );

        // Helix moves should be within radius of center (10, 10)
        let helix_moves: Vec<_> = result
            .moves
            .iter()
            .filter(|m| {
                matches!(m.move_type, MoveType::Linear { feed_rate } if (feed_rate - 500.0).abs() < 1e-10)
                    && m.target.z < 9.0
                    && m.target.z > -3.5
            })
            .collect();

        for m in &helix_moves {
            let hdx = m.target.x - 10.0;
            let hdy = m.target.y - 10.0;
            let dist = (hdx * hdx + hdy * hdy).sqrt();
            assert!(
                dist < 3.5,
                "Helix point ({}, {}) is {} from center, expected ~3.0",
                m.target.x,
                m.target.y,
                dist
            );
        }
    }

    // --- Tab/bridge tests ---

    fn profile_toolpath_for_tabs() -> Toolpath {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 10.0));
        tp.feed_to(P3::new(0.0, 0.0, -5.0), 500.0); // plunge
        // Square profile at Z=-5
        tp.feed_to(P3::new(100.0, 0.0, -5.0), 1000.0);
        tp.feed_to(P3::new(100.0, 100.0, -5.0), 1000.0);
        tp.feed_to(P3::new(0.0, 100.0, -5.0), 1000.0);
        tp.feed_to(P3::new(0.0, 0.0, -5.0), 1000.0); // close
        tp.rapid_to(P3::new(0.0, 0.0, 10.0));
        tp
    }

    #[test]
    fn test_tabs_lift_at_positions() {
        let tp = profile_toolpath_for_tabs();
        let tabs = even_tabs(4, 5.0, 3.0);
        let result = apply_tabs(&tp, &tabs, -5.0);

        // Some moves should be at tab height (-5 + 3 = -2)
        let tab_moves: Vec<_> = result
            .moves
            .iter()
            .filter(|m| (m.target.z - -2.0).abs() < 0.01)
            .collect();
        assert!(
            !tab_moves.is_empty(),
            "Should have moves at tab height (-2.0)"
        );
    }

    #[test]
    fn test_tabs_preserve_non_tab_moves() {
        let tp = profile_toolpath_for_tabs();
        let tabs = even_tabs(2, 3.0, 2.0);
        let result = apply_tabs(&tp, &tabs, -5.0);

        // Should still have moves at cut_depth
        let cut_moves: Vec<_> = result
            .moves
            .iter()
            .filter(|m| (m.target.z - -5.0).abs() < 0.01)
            .collect();
        assert!(
            !cut_moves.is_empty(),
            "Non-tab cutting moves should be preserved"
        );
    }

    #[test]
    fn test_no_tabs_returns_unchanged() {
        let tp = profile_toolpath_for_tabs();
        let result = apply_tabs(&tp, &[], -5.0);
        assert_eq!(result.moves.len(), tp.moves.len());
    }

    #[test]
    fn test_even_tabs_spacing() {
        let tabs = even_tabs(4, 5.0, 3.0);
        assert_eq!(tabs.len(), 4);
        assert!((tabs[0].position - 0.0).abs() < 1e-10);
        assert!((tabs[1].position - 0.25).abs() < 1e-10);
        assert!((tabs[2].position - 0.5).abs() < 1e-10);
        assert!((tabs[3].position - 0.75).abs() < 1e-10);

        for tab in &tabs {
            assert!((tab.width - 5.0).abs() < 1e-10);
            assert!((tab.height - 3.0).abs() < 1e-10);
        }
    }

    #[test]
    fn test_tabs_have_sharp_transitions() {
        let tp = profile_toolpath_for_tabs();
        let tabs = even_tabs(2, 10.0, 3.0);
        let result = apply_tabs(&tp, &tabs, -5.0);

        // Find vertical step-up feed moves (same XY, Z increases sharply)
        let mut found_step_up = false;
        for i in 1..result.moves.len() {
            if !matches!(result.moves[i].move_type, MoveType::Linear { .. }) {
                continue;
            }
            let prev = &result.moves[i - 1].target;
            let curr = &result.moves[i].target;
            let sdx = curr.x - prev.x;
            let sdy = curr.y - prev.y;
            let dxy = (sdx * sdx + sdy * sdy).sqrt();
            let dz = curr.z - prev.z;
            if dxy < 0.01 && dz > 1.0 && curr.z < 0.0 {
                found_step_up = true;
                // Step-up should go to tab height (-5 + 3 = -2)
                assert!(
                    (curr.z - -2.0).abs() < 0.1,
                    "Step-up should reach tab height -2.0, got {}",
                    curr.z
                );
            }
        }
        assert!(found_step_up, "Should have at least one sharp step-up move");
    }

    // --- Lead-in/out tests ---

    #[test]
    fn test_lead_in_adds_arc_moves() {
        let tp = simple_plunge_toolpath();
        let result = apply_lead_in_out(&tp, 2.0);

        // Should have more moves than original (arc segments added)
        assert!(
            result.moves.len() > tp.moves.len(),
            "Lead-in should add arc moves: {} vs {}",
            result.moves.len(),
            tp.moves.len()
        );
    }

    #[test]
    fn test_lead_in_reaches_cut_point() {
        let tp = simple_plunge_toolpath();
        let result = apply_lead_in_out(&tp, 2.0);

        // The cut moves at x=50, y=10, z=-3 should still be reachable
        let has_first_cut = result.moves.iter().any(|m| {
            (m.target.z - (-3.0)).abs() < 0.01
                && (m.target.x - 10.0).abs() < 3.0
                && (m.target.y - 10.0).abs() < 3.0
        });
        assert!(
            has_first_cut,
            "Lead-in should arrive near the first cut point"
        );
    }

    #[test]
    fn test_lead_in_preserves_rapids() {
        let tp = simple_plunge_toolpath();
        let result = apply_lead_in_out(&tp, 2.0);

        // Should still have a rapid move
        let has_rapid = result.moves.iter().any(|m| m.move_type == MoveType::Rapid);
        assert!(has_rapid, "Lead-in should preserve rapid moves");
    }

    // --- Dogbone tests ---

    fn square_profile_toolpath() -> Toolpath {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 10.0));
        tp.feed_to(P3::new(0.0, 0.0, -3.0), 500.0);
        // Square at Z=-3
        tp.feed_to(P3::new(50.0, 0.0, -3.0), 1000.0);
        tp.feed_to(P3::new(50.0, 50.0, -3.0), 1000.0);
        tp.feed_to(P3::new(0.0, 50.0, -3.0), 1000.0);
        tp.feed_to(P3::new(0.0, 0.0, -3.0), 1000.0);
        tp.rapid_to(P3::new(0.0, 0.0, 10.0));
        tp
    }

    #[test]
    fn test_dogbone_adds_overcuts() {
        let tp = square_profile_toolpath();
        let result = apply_dogbones(&tp, 3.0, 170.0);

        // Should have more moves than original (overcut + return at each corner)
        assert!(
            result.moves.len() > tp.moves.len(),
            "Dogbones should add moves: {} vs {}",
            result.moves.len(),
            tp.moves.len()
        );
    }

    #[test]
    fn test_dogbone_overcut_distance() {
        let tp = square_profile_toolpath();
        let tool_radius = 3.0;
        let result = apply_dogbones(&tp, tool_radius, 170.0);

        // Find overcut moves (moves that go away from the path)
        // At corner (50, 0): the overcut should be ~tool_radius from the corner
        for i in 1..result.moves.len() {
            let prev = result.moves[i - 1].target;
            let curr = result.moves[i].target;
            // Look for moves where next move returns to the same point (overcut + return)
            if i + 1 < result.moves.len() {
                let next = result.moves[i + 1].target;
                if (prev.x - next.x).abs() < 0.01
                    && (prev.y - next.y).abs() < 0.01
                    && (prev.z - curr.z).abs() < 0.01
                {
                    // This is an overcut: prev → curr → next where prev ≈ next
                    let odx = curr.x - prev.x;
                    let ody = curr.y - prev.y;
                    let dist = (odx * odx + ody * ody).sqrt();
                    assert!(
                        (dist - tool_radius).abs() < 0.5,
                        "Overcut distance should be ~{}, got {}",
                        tool_radius,
                        dist
                    );
                }
            }
        }
    }

    #[test]
    fn test_dogbone_preserves_straight_segments() {
        // Straight line — no corners, no dogbones
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 10.0));
        tp.feed_to(P3::new(0.0, 0.0, -3.0), 500.0);
        tp.feed_to(P3::new(50.0, 0.0, -3.0), 1000.0);
        tp.feed_to(P3::new(100.0, 0.0, -3.0), 1000.0);
        tp.rapid_to(P3::new(100.0, 0.0, 10.0));

        let result = apply_dogbones(&tp, 3.0, 170.0);
        assert_eq!(
            result.moves.len(),
            tp.moves.len(),
            "Straight path should have no dogbones added"
        );
    }

    #[test]
    fn test_dogbone_respects_angle_threshold() {
        // Shallow angle (170°) — should not trigger with default 170° threshold
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 10.0));
        tp.feed_to(P3::new(0.0, 0.0, -3.0), 500.0);
        tp.feed_to(P3::new(50.0, 0.0, -3.0), 1000.0);
        // Very slight turn (~5°)
        tp.feed_to(P3::new(100.0, 5.0, -3.0), 1000.0);
        tp.rapid_to(P3::new(100.0, 5.0, 10.0));

        let result = apply_dogbones(&tp, 3.0, 100.0); // threshold 100°
        assert_eq!(
            result.moves.len(),
            tp.moves.len(),
            "Shallow angle should not trigger dogbone"
        );
    }

    #[test]
    fn test_tabs_add_transition_moves() {
        let tp = profile_toolpath_for_tabs();
        let tabs = even_tabs(4, 5.0, 3.0);
        let result = apply_tabs(&tp, &tabs, -5.0);

        // Tab dressup adds step-up/step-down moves at tab edges
        assert!(
            result.moves.len() >= tp.moves.len(),
            "Tab dressup should add transition moves: {} vs {}",
            result.moves.len(),
            tp.moves.len()
        );
    }

    // --- Link-vs-retract tests ---

    /// Build a toolpath with two nearby passes (retract between them).
    fn two_pass_toolpath(pass_gap: f64) -> Toolpath {
        let mut tp = Toolpath::new();
        // First pass
        tp.rapid_to(P3::new(0.0, 0.0, 10.0));
        tp.feed_to(P3::new(0.0, 0.0, -3.0), 500.0);
        tp.feed_to(P3::new(20.0, 0.0, -3.0), 1000.0);
        // Retract
        tp.rapid_to(P3::new(20.0, 0.0, 10.0));
        // Rapid to second pass start
        tp.rapid_to(P3::new(20.0 + pass_gap, 0.0, 10.0));
        // Plunge
        tp.feed_to(P3::new(20.0 + pass_gap, 0.0, -3.0), 500.0);
        // Second pass
        tp.feed_to(P3::new(40.0 + pass_gap, 0.0, -3.0), 1000.0);
        // Retract
        tp.rapid_to(P3::new(40.0 + pass_gap, 0.0, 10.0));
        tp
    }

    fn default_link_params() -> LinkMoveParams {
        LinkMoveParams {
            max_link_distance: 18.0, // 3× 6mm tool diameter
            link_feed_rate: 1000.0,
            safe_z_threshold: 10.0,
        }
    }

    #[test]
    fn test_link_basic() {
        // 2mm gap between passes — should be linked
        let tp = two_pass_toolpath(2.0);
        let params = default_link_params();
        let result = apply_link_moves(&tp, &params);

        // Should have fewer moves (retract+rapid+plunge replaced with feed)
        assert!(
            result.moves.len() < tp.moves.len(),
            "Link should reduce moves: {} vs {}",
            result.moves.len(),
            tp.moves.len()
        );

        // Should have less rapid distance
        assert!(
            result.total_rapid_distance() < tp.total_rapid_distance(),
            "Link should reduce rapids: {:.1} vs {:.1}",
            result.total_rapid_distance(),
            tp.total_rapid_distance()
        );
    }

    #[test]
    fn test_link_too_far() {
        // 25mm gap — exceeds max_link_distance of 18mm
        let tp = two_pass_toolpath(25.0);
        let params = default_link_params();
        let result = apply_link_moves(&tp, &params);

        // Should be unchanged (gap too large)
        assert_eq!(
            result.moves.len(),
            tp.moves.len(),
            "Far passes should not be linked"
        );
    }

    #[test]
    fn test_link_first_entry_preserved() {
        // The very first plunge should never be linked (no prior cutting)
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 10.0));
        tp.rapid_to(P3::new(5.0, 0.0, 10.0));
        tp.feed_to(P3::new(5.0, 0.0, -3.0), 500.0);
        tp.feed_to(P3::new(20.0, 0.0, -3.0), 1000.0);
        tp.rapid_to(P3::new(20.0, 0.0, 10.0));

        let params = default_link_params();
        let result = apply_link_moves(&tp, &params);

        // First entry should not be linked — all moves preserved
        assert_eq!(
            result.moves.len(),
            tp.moves.len(),
            "First entry should be preserved"
        );
    }

    #[test]
    fn test_link_different_z_preserved() {
        // Two passes at different Z levels — should NOT be linked
        let mut tp = Toolpath::new();
        // First pass at Z=-3
        tp.rapid_to(P3::new(0.0, 0.0, 10.0));
        tp.feed_to(P3::new(0.0, 0.0, -3.0), 500.0);
        tp.feed_to(P3::new(20.0, 0.0, -3.0), 1000.0);
        // Retract
        tp.rapid_to(P3::new(20.0, 0.0, 10.0));
        // Second pass at Z=-6 (different depth)
        tp.rapid_to(P3::new(22.0, 0.0, 10.0));
        tp.feed_to(P3::new(22.0, 0.0, -6.0), 500.0);
        tp.feed_to(P3::new(40.0, 0.0, -6.0), 1000.0);
        tp.rapid_to(P3::new(40.0, 0.0, 10.0));

        let params = default_link_params();
        let result = apply_link_moves(&tp, &params);

        // Different Z levels — should not be linked
        assert_eq!(
            result.moves.len(),
            tp.moves.len(),
            "Different Z levels should not be linked"
        );
    }

    #[test]
    fn test_link_reduces_rapid_distance() {
        let tp = two_pass_toolpath(5.0);
        let params = default_link_params();
        let result = apply_link_moves(&tp, &params);

        let orig_rapid = tp.total_rapid_distance();
        let linked_rapid = result.total_rapid_distance();
        assert!(
            linked_rapid < orig_rapid * 0.8,
            "Linking should significantly reduce rapids: {:.1} -> {:.1}",
            orig_rapid,
            linked_rapid
        );
    }

    // --- Air-cut filter tests ---

    use crate::dexel_stock::TriDexelStock;

    /// Build a stock where x < 50 has material (top_z = 5.0) and x >= 50 is
    /// cleared (top_z lowered to -10.0 by simulating a cut).  The stock spans
    /// x: 0..100, y: 0..100, z: -10..5 with 5mm cells.
    fn half_cleared_stock() -> TriDexelStock {
        use crate::dexel::ray_subtract_above;
        let stock = TriDexelStock::from_stock(0.0, 0.0, 100.0, 100.0, -10.0, 5.0, 5.0);
        // Clear material above z=-10 for columns where x >= 50.
        // This effectively removes all material in the right half.
        let grid = &stock.z_grid;
        let cols = grid.cols;
        let rows = grid.rows;
        // We need mutable access, so rebuild with cleared rays.
        let mut cleared = stock;
        for row in 0..rows {
            for col in 0..cols {
                let (world_x, _world_y) = {
                    let u = cleared.z_grid.origin_u + col as f64 * cleared.z_grid.cell_size;
                    let v = cleared.z_grid.origin_v + row as f64 * cleared.z_grid.cell_size;
                    (u, v)
                };
                if world_x >= 50.0 {
                    ray_subtract_above(cleared.z_grid.ray_mut(row, col), -10.0);
                }
            }
        }
        cleared
    }

    #[test]
    fn filter_air_cuts_removes_air_moves() {
        // Toolpath cuts across the stock: left half has material, right half is air.
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(10.0, 50.0, 10.0)); // rapid to start above stock
        tp.feed_to(P3::new(10.0, 50.0, 2.0), 500.0); // plunge into material (stock top = 5)
        tp.feed_to(P3::new(30.0, 50.0, 2.0), 1000.0); // cut in material
        tp.feed_to(P3::new(60.0, 50.0, 2.0), 1000.0); // cut into air (x>=50 is cleared)
        tp.feed_to(P3::new(90.0, 50.0, 2.0), 1000.0); // still in air
        tp.rapid_to(P3::new(90.0, 50.0, 10.0)); // retract

        let stock = half_cleared_stock();
        let result = filter_air_cuts(&tp, &stock, 3.0, 10.0, 0.1);

        // The moves at x=60 and x=90 should have been removed (both endpoints in air).
        // Specifically, the move from x=60 to x=90 is fully in air (source and target).
        // The move from x=30 to x=60 has source in material, target in air — conservative: preserved.
        // So the result should have fewer cutting moves than the original.
        let original_cutting = tp
            .moves
            .iter()
            .filter(|m| matches!(m.move_type, MoveType::Linear { .. }))
            .count();
        let result_cutting = result
            .moves
            .iter()
            .filter(|m| matches!(m.move_type, MoveType::Linear { .. }))
            .count();
        assert!(
            result_cutting < original_cutting,
            "Air moves should be removed: orig_cutting={}, result_cutting={}",
            original_cutting,
            result_cutting
        );

        // The result should still contain the initial plunge and the material cuts.
        let has_material_cut = result.moves.iter().any(|m| {
            matches!(m.move_type, MoveType::Linear { feed_rate } if (feed_rate - 1000.0).abs() < 1e-6)
                && m.target.x <= 50.0
        });
        assert!(
            has_material_cut,
            "Material-region cutting moves should be preserved"
        );
    }

    #[test]
    fn filter_air_cuts_preserves_cutting_moves() {
        // All moves are in material — nothing should be removed.
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(10.0, 50.0, 10.0));
        tp.feed_to(P3::new(10.0, 50.0, 2.0), 500.0);
        tp.feed_to(P3::new(20.0, 50.0, 2.0), 1000.0);
        tp.feed_to(P3::new(30.0, 50.0, 2.0), 1000.0);
        tp.rapid_to(P3::new(30.0, 50.0, 10.0));

        let stock = half_cleared_stock();
        let result = filter_air_cuts(&tp, &stock, 3.0, 10.0, 0.1);

        // All cutting moves are in the left half (x < 50) where material exists
        // at top_z=5.0 and tool is at z=2.0 (below stock top). No air cuts.
        assert_eq!(
            result.moves.len(),
            tp.moves.len(),
            "All-material toolpath should be unchanged: result={}, orig={}",
            result.moves.len(),
            tp.moves.len()
        );
    }

    #[test]
    fn filter_air_cuts_conservative_partial() {
        // Move starts in air, ends in material — should be preserved (conservative).
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(70.0, 50.0, 10.0)); // rapid to air region
        tp.feed_to(P3::new(70.0, 50.0, 2.0), 500.0); // plunge in air
        tp.feed_to(P3::new(30.0, 50.0, 2.0), 1000.0); // move from air into material
        tp.rapid_to(P3::new(30.0, 50.0, 10.0));

        let stock = half_cleared_stock();
        let result = filter_air_cuts(&tp, &stock, 3.0, 10.0, 0.1);

        // The move from x=70 to x=30 has source in air but target in material.
        // Conservative rule: it should be preserved because the target has material.
        let has_crossing_cut = result.moves.iter().any(|m| {
            matches!(m.move_type, MoveType::Linear { feed_rate } if (feed_rate - 1000.0).abs() < 1e-6)
                && (m.target.x - 30.0).abs() < 0.01
        });
        assert!(
            has_crossing_cut,
            "Partial air-to-material move should be preserved (conservative)"
        );
    }
}
