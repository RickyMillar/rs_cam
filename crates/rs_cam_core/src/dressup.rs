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
    let dz = (start.z - end.z).abs();
    let max_angle_rad = max_angle_deg.to_radians();
    let ramp_xy_len = dz / max_angle_rad.tan();

    // Ramp out along direction, then back
    let half_len = ramp_xy_len / 2.0;
    let mid_z = (start.z + end.z) / 2.0;

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
    let dz = (start.z - end.z).abs();
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

    let center_x = start.x;
    let center_y = start.y;

    for i in 1..=total_steps {
        let t = i as f64 / total_steps as f64;
        let angle = total_angle * t;
        let z = start.z - dz * t;
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
/// Tabs lift the cutter to `cut_depth + tab.height` at specified positions
/// along the cutting contour, leaving material bridges that hold the part.
///
/// `tabs` are specified as fractional positions (0.0-1.0) along the cutting
/// perimeter. `cut_depth` is the Z of the cutting pass.
pub fn apply_tabs(toolpath: &Toolpath, tabs: &[Tab], cut_depth: f64) -> Toolpath {
    if tabs.is_empty() {
        return toolpath.clone();
    }

    // Collect cutting segments (linear moves at cut_depth)
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

    if cutting_indices.is_empty() {
        return toolpath.clone();
    }

    // Compute cumulative distance along cutting moves
    let mut cum_dist = vec![0.0_f64];
    for i in 1..cutting_indices.len() {
        let prev_idx = cutting_indices[i - 1];
        let curr_idx = cutting_indices[i];
        let prev = &toolpath.moves[prev_idx].target;
        let curr = &toolpath.moves[curr_idx].target;
        let d = ((curr.x - prev.x).powi(2) + (curr.y - prev.y).powi(2)).sqrt();
        cum_dist.push(cum_dist.last().unwrap() + d);
    }

    let total_dist = *cum_dist.last().unwrap_or(&0.0);
    if total_dist < 1e-6 {
        return toolpath.clone();
    }

    // For each cutting move, determine if it's inside a tab zone
    let mut result = Toolpath::new();

    for (i, m) in toolpath.moves.iter().enumerate() {
        if let Some(cut_pos) = cutting_indices.iter().position(|&ci| ci == i) {
            let frac = cum_dist[cut_pos] / total_dist;

            // Check if this position is inside any tab
            let in_tab = tabs.iter().find(|tab| {
                let half_w = (tab.width / 2.0) / total_dist;
                let lo = tab.position - half_w;
                let hi = tab.position + half_w;
                frac >= lo && frac <= hi
            });

            if let Some(tab) = in_tab {
                // Lift to tab height
                let tab_z = cut_depth + tab.height;
                result.moves.push(Move {
                    target: P3::new(m.target.x, m.target.y, tab_z),
                    move_type: m.move_type,
                });
            } else {
                result.moves.push(m.clone());
            }
        } else {
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
    fn test_total_move_count_preserved() {
        let tp = profile_toolpath_for_tabs();
        let tabs = even_tabs(4, 5.0, 3.0);
        let result = apply_tabs(&tp, &tabs, -5.0);

        // Tab dressup modifies Z but doesn't add/remove moves
        assert_eq!(
            result.moves.len(),
            tp.moves.len(),
            "Tab dressup should not change move count"
        );
    }
}
