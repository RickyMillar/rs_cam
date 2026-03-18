//! Toolpath dressups — post-processing transforms applied to toolpaths.
//!
//! Dressups modify an existing toolpath without changing the core operation.
//! They compose: you can chain multiple dressups on the same toolpath.
//!
//! - **Ramp entry**: Replace vertical plunges with helical or ramped entry
//! - **Tab/bridge**: Insert material tabs to hold parts during profile cutting

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
        if let MoveType::Linear { feed_rate } = m.move_type {
            if i > 0 && is_plunge(&toolpath.moves[i - 1], m) {
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
        }

        result.moves.push(m.clone());
        i += 1;
    }

    result
}

fn is_plunge(prev: &Move, current: &Move) -> bool {
    if let MoveType::Linear { .. } = current.move_type {
        let dz = current.target.z - prev.target.z;
        let dxy = ((current.target.x - prev.target.x).powi(2)
            + (current.target.y - prev.target.y).powi(2))
        .sqrt();
        // Downward move with negligible XY movement
        dz < -0.1 && dxy < 0.01
    } else {
        false
    }
}

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

fn emit_ramp(
    tp: &mut Toolpath,
    start: &P3,
    end: &P3,
    dir: (f64, f64),
    max_angle_deg: f64,
    feed_rate: f64,
) {
    // Only ramp the last portion of the descent (max 5mm above target).
    // Rapid down to clearance first, then ramp the rest.
    let clearance = 2.0; // mm above cut depth to start ramping
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

fn emit_helix(
    tp: &mut Toolpath,
    start: &P3,
    end: &P3,
    radius: f64,
    pitch: f64,
    feed_rate: f64,
) {
    // Only helix the last portion — rapid down to clearance first
    let clearance = 2.0;
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
        let x = center_x + radius * angle.cos();
        let y = center_y + radius * angle.sin();
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
            matches!(m.move_type, MoveType::Linear { .. })
                && (m.target.z - cut_depth).abs() < 0.01
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
        let d = ((curr.x - prev.x).powi(2) + (curr.y - prev.y).powi(2)).sqrt();
        cum_dist.push(cum_dist.last().unwrap() + d);
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
            events.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

            if events.is_empty() {
                // No boundary crossings — whole segment is in or out
                let mid_dist = (seg_start_dist + seg_end_dist) / 2.0;
                if let Some(tab_z) = tab_z_at_dist(mid_dist) {
                    if !in_tab {
                        // Entered tab zone before this segment
                        if let Some(last) = result.moves.last() {
                            result.feed_to(
                                P3::new(last.target.x, last.target.y, tab_z),
                                feed_rate,
                            );
                        }
                        in_tab = true;
                    }
                    result.feed_to(
                        P3::new(curr_target.x, curr_target.y, tab_z),
                        feed_rate,
                    );
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
                            result.feed_to(
                                P3::new(split_x, split_y, cut_depth),
                                feed_rate,
                            );
                        }
                        // Step up
                        let tab_z = tab_z_at_dist(*event_dist + 0.01)
                            .unwrap_or(cut_depth + 2.0);
                        result.feed_to(P3::new(split_x, split_y, tab_z), feed_rate);
                        in_tab = true;
                    } else {
                        // Emit segment up to tab exit at tab height
                        let tab_z = tab_z_at_dist(last_dist + 0.01)
                            .unwrap_or(cut_depth + 2.0);
                        result.feed_to(P3::new(split_x, split_y, tab_z), feed_rate);
                        // Step down
                        result.feed_to(
                            P3::new(split_x, split_y, cut_depth),
                            feed_rate,
                        );
                        in_tab = false;
                    }
                    last_dist = *event_dist;
                }

                // Emit remainder of segment after last event
                if in_tab {
                    let tab_z = tab_z_at_dist(last_dist + 0.01)
                        .unwrap_or(cut_depth + 2.0);
                    result.feed_to(
                        P3::new(curr_target.x, curr_target.y, tab_z),
                        feed_rate,
                    );
                } else {
                    result.feed_to(
                        P3::new(curr_target.x, curr_target.y, cut_depth),
                        feed_rate,
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

// ---------------------------------------------------------------------------
// Lead-in / Lead-out dressup
// ---------------------------------------------------------------------------

/// Insert arc lead-in and lead-out moves at the start/end of cutting passes.
///
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
                        let ax = plunge_end.x + perp_x * radius * (1.0 - angle.sin())
                            - ux * radius * (1.0 - angle.cos());
                        let ay = plunge_end.y + perp_y * radius * (1.0 - angle.sin())
                            - uy * radius * (1.0 - angle.cos());
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
                        let ax = cut_end.x + ux * radius * angle.sin()
                            + perp_x * radius * (1.0 - angle.cos());
                        let ay = cut_end.y + uy * radius * angle.sin()
                            + perp_y * radius * (1.0 - angle.cos());
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
/// At each corner sharper than `max_angle_deg`, a small extension is cut
/// along the corner bisector so that a mating part with a sharp corner can
/// fit into the CNC-cut pocket. The overcut distance is `tool_radius`,
/// creating a clearance notch at each inside corner.
///
/// Only operates on consecutive linear feed moves at the same Z.
pub fn apply_dogbones(
    toolpath: &Toolpath,
    tool_radius: f64,
    max_angle_deg: f64,
) -> Toolpath {
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

#[cfg(test)]
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
                let dxy = ((curr.x - prev.x).powi(2) + (curr.y - prev.y).powi(2)).sqrt();
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
        assert!(
            cut_moves.len() >= 2,
            "Cutting moves should be preserved"
        );
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

        let has_cut_depth = result
            .moves
            .iter()
            .any(|m| (m.target.z - -3.0).abs() < 0.1);
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
            let dist = ((m.target.x - 10.0).powi(2) + (m.target.y - 10.0).powi(2)).sqrt();
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
            let dxy = ((curr.x - prev.x).powi(2) + (curr.y - prev.y).powi(2)).sqrt();
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
        assert!(has_first_cut, "Lead-in should arrive near the first cut point");
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
                    let dist = ((curr.x - prev.x).powi(2) + (curr.y - prev.y).powi(2)).sqrt();
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
}
