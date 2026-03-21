//! Drilling operation — plunge drilling toolpaths with peck cycle support.
//!
//! Generates toolpath moves for standard drill cycles: simple (G81),
//! dwell (G82), peck (G83), and chip-break (G73).

use crate::geo::P3;
use crate::toolpath::Toolpath;

/// Drill cycle type, matching standard G-code canned cycles.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DrillCycle {
    /// G81: Simple drill — feed to depth, rapid out.
    Simple,
    /// G82: Drill with dwell at bottom (seconds).
    /// Dwell is a post-processor concern; in the toolpath IR this behaves
    /// identically to `Simple` (the dwell duration is preserved for
    /// post-processor consumption).
    Dwell(f64),
    /// G83: Peck drill — full retract to retract_z between pecks.
    /// Parameter is peck depth in mm.
    Peck(f64),
    /// G73: Chip break — small retract between pecks.
    /// Parameters are (peck_depth, retract_amount) in mm.
    ChipBreak(f64, f64),
}

/// Parameters for a drilling operation.
pub struct DrillParams {
    /// Total drill depth below the surface (positive value).
    pub depth: f64,
    /// Drill cycle type.
    pub cycle: DrillCycle,
    /// Plunge feed rate (mm/min).
    pub feed_rate: f64,
    /// Safe Z height for rapid traversal between holes.
    pub safe_z: f64,
    /// R-plane: rapid down to here, then switch to feed.
    pub retract_z: f64,
}

/// Generate a drill toolpath for a list of hole positions (XY coordinates).
///
/// Each hole is drilled according to the cycle in `params`. The holes are
/// visited in the order given — the caller is responsible for any
/// optimization of visit order.
pub fn drill_toolpath(holes: &[[f64; 2]], params: &DrillParams) -> Toolpath {
    let mut tp = Toolpath::new();

    for &[x, y] in holes {
        // 1. Rapid to safe_z (vertical move if not already there)
        tp.rapid_to(P3::new(x, y, params.safe_z));
        // 2. Rapid down to retract_z (R-plane)
        tp.rapid_to(P3::new(x, y, params.retract_z));

        let bottom_z = -params.depth;

        match params.cycle {
            DrillCycle::Simple | DrillCycle::Dwell(_) => {
                // Feed to full depth, then rapid out.
                // (Dwell timing is a post-processor feature — the motion is identical.)
                tp.feed_to(P3::new(x, y, bottom_z), params.feed_rate);
                tp.rapid_to(P3::new(x, y, params.retract_z));
            }
            DrillCycle::Peck(peck_depth) => {
                drill_peck_full_retract(&mut tp, x, y, params, peck_depth, bottom_z);
            }
            DrillCycle::ChipBreak(peck_depth, retract_amount) => {
                drill_chip_break(&mut tp, x, y, params, peck_depth, retract_amount, bottom_z);
            }
        }
    }

    tp
}

/// G83 peck drill: feed down by peck_depth, rapid to retract_z, rapid back
/// to previous depth + clearance, repeat.
fn drill_peck_full_retract(
    tp: &mut Toolpath,
    x: f64,
    y: f64,
    params: &DrillParams,
    peck_depth: f64,
    bottom_z: f64,
) {
    const CLEARANCE: f64 = 0.5; // mm above previous depth for re-entry

    let mut current_z = params.retract_z;

    loop {
        let target_z = (current_z - peck_depth).max(bottom_z);
        // Feed down to next peck depth
        tp.feed_to(P3::new(x, y, target_z), params.feed_rate);

        if (target_z - bottom_z).abs() < 1e-9 {
            // Reached full depth — retract and done
            tp.rapid_to(P3::new(x, y, params.retract_z));
            break;
        }

        // Retract fully to R-plane
        tp.rapid_to(P3::new(x, y, params.retract_z));
        // Rapid back to just above previous cut depth
        let reentry_z = target_z + CLEARANCE;
        tp.rapid_to(P3::new(x, y, reentry_z));

        current_z = target_z;
    }
}

/// G73 chip break: feed down by peck_depth, retract by retract_amount,
/// feed back, repeat.
fn drill_chip_break(
    tp: &mut Toolpath,
    x: f64,
    y: f64,
    params: &DrillParams,
    peck_depth: f64,
    retract_amount: f64,
    bottom_z: f64,
) {
    let mut current_z = params.retract_z;

    loop {
        let target_z = (current_z - peck_depth).max(bottom_z);
        // Feed down to next peck depth
        tp.feed_to(P3::new(x, y, target_z), params.feed_rate);

        if (target_z - bottom_z).abs() < 1e-9 {
            // Reached full depth — retract and done
            tp.rapid_to(P3::new(x, y, params.retract_z));
            break;
        }

        // Small retract for chip breaking
        let retract_z = target_z + retract_amount;
        tp.rapid_to(P3::new(x, y, retract_z));

        current_z = target_z;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::toolpath::MoveType;

    fn default_params(cycle: DrillCycle) -> DrillParams {
        DrillParams {
            depth: 10.0,
            cycle,
            feed_rate: 200.0,
            safe_z: 25.0,
            retract_z: 2.0,
        }
    }

    #[test]
    fn simple_drill_single_hole_correct_depth() {
        let params = default_params(DrillCycle::Simple);
        let tp = drill_toolpath(&[[5.0, 10.0]], &params);

        // Expected sequence:
        //   0: Rapid to (5, 10, 25)   — safe_z
        //   1: Rapid to (5, 10, 2)    — retract_z
        //   2: Feed  to (5, 10, -10)  — full depth
        //   3: Rapid to (5, 10, 2)    — retract
        assert_eq!(tp.moves.len(), 4);

        // Check safe_z rapid
        assert_eq!(tp.moves[0].move_type, MoveType::Rapid);
        assert!((tp.moves[0].target.z - 25.0).abs() < 1e-10);

        // Check retract_z rapid
        assert_eq!(tp.moves[1].move_type, MoveType::Rapid);
        assert!((tp.moves[1].target.z - 2.0).abs() < 1e-10);

        // Check plunge to depth
        assert!(matches!(
            tp.moves[2].move_type,
            MoveType::Linear { feed_rate } if (feed_rate - 200.0).abs() < 1e-10
        ));
        assert!((tp.moves[2].target.x - 5.0).abs() < 1e-10);
        assert!((tp.moves[2].target.y - 10.0).abs() < 1e-10);
        assert!((tp.moves[2].target.z - (-10.0)).abs() < 1e-10);

        // Check retract
        assert_eq!(tp.moves[3].move_type, MoveType::Rapid);
        assert!((tp.moves[3].target.z - 2.0).abs() < 1e-10);
    }

    #[test]
    fn dwell_drill_behaves_like_simple() {
        let simple = drill_toolpath(&[[0.0, 0.0]], &default_params(DrillCycle::Simple));
        let dwell = drill_toolpath(&[[0.0, 0.0]], &default_params(DrillCycle::Dwell(0.5)));

        assert_eq!(simple.moves.len(), dwell.moves.len());
        for (a, b) in simple.moves.iter().zip(dwell.moves.iter()) {
            assert!((a.target.x - b.target.x).abs() < 1e-10);
            assert!((a.target.y - b.target.y).abs() < 1e-10);
            assert!((a.target.z - b.target.z).abs() < 1e-10);
        }
    }

    #[test]
    fn peck_drill_multiple_plunge_retract_cycles() {
        // depth=10, peck=3 → pecks at z=-1 (retract_z=2, so 2 - 3 = -1),
        // then -4, -7, -10
        let params = default_params(DrillCycle::Peck(3.0));
        let tp = drill_toolpath(&[[0.0, 0.0]], &params);

        // Count the feed moves — each peck produces one feed move
        let feed_count = tp
            .moves
            .iter()
            .filter(|m| matches!(m.move_type, MoveType::Linear { .. }))
            .count();
        // 4 pecks: (2→-1), (-1→-4), (-4→-7), (-7→-10)
        assert_eq!(feed_count, 4, "Expected 4 peck plunges, got {feed_count}");

        // Verify all feed moves go progressively deeper
        let feed_depths: Vec<f64> = tp
            .moves
            .iter()
            .filter(|m| matches!(m.move_type, MoveType::Linear { .. }))
            .map(|m| m.target.z)
            .collect();
        assert_eq!(feed_depths.len(), 4);
        assert!((feed_depths[0] - (-1.0)).abs() < 1e-10);
        assert!((feed_depths[1] - (-4.0)).abs() < 1e-10);
        assert!((feed_depths[2] - (-7.0)).abs() < 1e-10);
        assert!((feed_depths[3] - (-10.0)).abs() < 1e-10);

        // Between non-final pecks, should retract to retract_z (2.0)
        // After each feed (except last), next move should be rapid to retract_z
        let feed_indices: Vec<usize> = tp
            .moves
            .iter()
            .enumerate()
            .filter(|(_, m)| matches!(m.move_type, MoveType::Linear { .. }))
            .map(|(i, _)| i)
            .collect();
        // After first 3 feeds, next move is rapid to retract_z
        for &fi in &feed_indices[..3] {
            let next = &tp.moves[fi + 1];
            assert_eq!(next.move_type, MoveType::Rapid);
            assert!(
                (next.target.z - 2.0).abs() < 1e-10,
                "Expected retract to 2.0, got {}",
                next.target.z
            );
        }
        // After final feed, rapid to retract_z
        let last_fi = *feed_indices.last().expect("should have feeds");
        let final_retract = &tp.moves[last_fi + 1];
        assert_eq!(final_retract.move_type, MoveType::Rapid);
        assert!((final_retract.target.z - 2.0).abs() < 1e-10);
    }

    #[test]
    fn chip_break_small_retract() {
        let params = default_params(DrillCycle::ChipBreak(5.0, 1.0));
        let tp = drill_toolpath(&[[0.0, 0.0]], &params);

        // depth=10, peck=5, starting at retract_z=2
        // Peck 1: feed to 2 - 5 = -3, retract to -3 + 1 = -2
        // Peck 2: feed to -3 - 5 = -8, retract to -8 + 1 = -7
        // Peck 3: feed to -8 - 5 = -13 → clamped to -10, retract to retract_z

        let feed_depths: Vec<f64> = tp
            .moves
            .iter()
            .filter(|m| matches!(m.move_type, MoveType::Linear { .. }))
            .map(|m| m.target.z)
            .collect();
        assert_eq!(
            feed_depths.len(),
            3,
            "Expected 3 pecks, got {}",
            feed_depths.len()
        );
        assert!((feed_depths[0] - (-3.0)).abs() < 1e-10);
        assert!((feed_depths[1] - (-8.0)).abs() < 1e-10);
        assert!((feed_depths[2] - (-10.0)).abs() < 1e-10);

        // After first peck: retract by 1mm (small retract, not full)
        let feed_indices: Vec<usize> = tp
            .moves
            .iter()
            .enumerate()
            .filter(|(_, m)| matches!(m.move_type, MoveType::Linear { .. }))
            .map(|(i, _)| i)
            .collect();

        // After peck 1 (z=-3): retract to -2 (chip break, not full retract)
        let after_peck1 = &tp.moves[feed_indices[0] + 1];
        assert_eq!(after_peck1.move_type, MoveType::Rapid);
        assert!(
            (after_peck1.target.z - (-2.0)).abs() < 1e-10,
            "Expected chip-break retract to -2.0, got {}",
            after_peck1.target.z
        );

        // After final peck: full retract to retract_z
        let after_final = &tp.moves[*feed_indices.last().expect("has feeds") + 1];
        assert_eq!(after_final.move_type, MoveType::Rapid);
        assert!((after_final.target.z - 2.0).abs() < 1e-10);
    }

    #[test]
    fn multiple_holes_visited_in_order() {
        let holes = [[0.0, 0.0], [10.0, 20.0], [30.0, 40.0]];
        let params = default_params(DrillCycle::Simple);
        let tp = drill_toolpath(&holes, &params);

        // Each hole produces 4 moves → 12 total
        assert_eq!(tp.moves.len(), 12);

        // Verify XY positions for the first move of each hole (rapid to safe_z)
        for (i, &[hx, hy]) in holes.iter().enumerate() {
            let m = &tp.moves[i * 4];
            assert!(
                (m.target.x - hx).abs() < 1e-10 && (m.target.y - hy).abs() < 1e-10,
                "Hole {i}: expected ({hx}, {hy}), got ({}, {})",
                m.target.x,
                m.target.y,
            );
        }
    }

    #[test]
    fn empty_hole_list_produces_empty_toolpath() {
        let tp = drill_toolpath(&[], &default_params(DrillCycle::Simple));
        assert!(tp.moves.is_empty());
    }

    #[test]
    fn peck_depth_larger_than_total_depth() {
        // When peck_depth exceeds total depth, only one peck needed
        let mut params = default_params(DrillCycle::Peck(50.0));
        params.depth = 5.0;
        params.retract_z = 0.0;
        let tp = drill_toolpath(&[[0.0, 0.0]], &params);

        let feed_count = tp
            .moves
            .iter()
            .filter(|m| matches!(m.move_type, MoveType::Linear { .. }))
            .count();
        assert_eq!(
            feed_count, 1,
            "Single peck expected when peck_depth > depth"
        );

        // Verify it reaches the correct bottom
        let feed_z: Vec<f64> = tp
            .moves
            .iter()
            .filter(|m| matches!(m.move_type, MoveType::Linear { .. }))
            .map(|m| m.target.z)
            .collect();
        assert!((feed_z[0] - (-5.0)).abs() < 1e-10);
    }
}
