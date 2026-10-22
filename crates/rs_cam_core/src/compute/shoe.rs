//! H1 (`planning/op_architecture_2026-10-14/PLAN.md`, item H): the dust
//! shoe solver.
//!
//! The operator sets the shoe height at each tool change. For one run of
//! toolpaths (the same tool, the same setup, the same shoe setting, one
//! after the other) the solver gives the LOWEST gap (brush end above the
//! bit end) in the travel range at which the hard shoe parts and the shoe
//! shelf clear the stock and every enabled obstacle on every move. When no
//! gap in the range clears, the answer is "raise the shoe clear", and the
//! checks then read the bare head.
//!
//! # The stock the solver reads
//!
//! The solver reads the FRESH stock box of the setup (the box before any
//! toolpath cuts). That is the conservative choice: the stock before the
//! run can only be lower, so a gap that clears the fresh box clears it
//! too. The simulated stock would let the shoe sit lower over a cut-down
//! area, but it does not exist before a simulation, and a gap that depends
//! on it would move with every re-simulation.
//!
//! The solver is pure. The session door (`ProjectSession::shoe_plan`) and
//! the simulation request builders call it with the final toolpaths.

use crate::compute::obstacle::{JoinStart, ObstacleResolveError, ObstacleSet};
use crate::geo::BoundingBox3;
use crate::machine::head::{MachineHead, ResolvedShoe, ShoePosition, ShoeSetting};
use crate::stock::collision::ToolAssembly;
use crate::toolpath::Toolpath;

/// The answer of the solver for one run.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShoeSolution {
    /// The lowest gap (mm) that clears, rounded up to a whole millimetre
    /// and never below the shoe's normal gap or the travel minimum. It can
    /// be above the travel maximum: then [`Self::fits`] is false.
    pub needed_gap_mm: f64,
    /// The needed gap is inside the travel range.
    pub fits: bool,
}

/// The lowest gap at which the shoe of `head` clears `stock` and
/// `obstacles` on every move of `run`, for a tool of `stickout`. `run`
/// holds each toolpath of the run with its join (as the check reads it).
/// `None` when the head has no shoe.
///
/// # Errors
/// [`ObstacleResolveError`] when an outward offset fails.
pub fn solve_gap(
    head: &MachineHead,
    stickout: f64,
    obstacles: &ObstacleSet,
    stock: &BoundingBox3,
    run: &[(&Toolpath, JoinStart)],
) -> Result<Option<ShoeSolution>, ObstacleResolveError> {
    let Some(shoe) = &head.shoe else {
        return Ok(None);
    };
    // The shoe parts alone, with the brush end at the tip (gap 0): every
    // band ceiling is then the gap that band needs at that point.
    let shoe_only = MachineHead {
        name: String::new(),
        mount_mm: 0.0,
        parts: Vec::new(),
        shoe: Some(shoe.clone()),
    };
    let assembly = ToolAssembly {
        cutter_radius: 0.0,
        cutter_length: 0.0,
        shank_diameter: 0.0,
        stickout: 0.0,
        head: shoe_only.bands(0.0, ShoePosition::At { gap_mm: 0.0 }),
    };
    let view = obstacles.for_tool(&assembly, Some(stock))?;
    let lift = run
        .iter()
        .map(|(toolpath, join)| view.required_lift(toolpath, *join))
        .fold(f64::NEG_INFINITY, f64::max);
    let (lowest, highest) = shoe.gap_range(stickout);
    let needed = if lift.is_finite() {
        lowest.max(lift.ceil())
    } else {
        lowest
    };
    Ok(Some(ShoeSolution {
        needed_gap_mm: needed,
        fits: needed <= highest,
    }))
}

/// The shoe state a toolpath runs with: its setting and the solver's
/// answer for its run.
#[must_use]
pub fn resolve(
    head: &MachineHead,
    setting: ShoeSetting,
    solution: Option<ShoeSolution>,
) -> ResolvedShoe {
    if head.shoe.is_none() {
        return ResolvedShoe::NoShoe;
    }
    match setting {
        ShoeSetting::Auto => match solution {
            Some(s) if s.fits => ResolvedShoe::Gap {
                gap_mm: s.needed_gap_mm,
            },
            _ => ResolvedShoe::Raised,
        },
        ShoeSetting::Gap { gap_mm } => ResolvedShoe::Gap { gap_mm },
        ShoeSetting::Raised => ResolvedShoe::Raised,
        ShoeSetting::Off => ResolvedShoe::Off,
    }
}

#[cfg(test)]
#[allow(
    // SAFETY: test code; a failed lookup is a failed test.
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;
    use crate::compute::obstacle::{Obstacle, ObstacleGeometry};
    use crate::geo::P3;
    use crate::ids::ObstacleId;
    use crate::machine::head::ShoeTravel;

    /// A 100 x 100 x 100 stock, top at Z 0.
    fn stock() -> BoundingBox3 {
        BoundingBox3 {
            min: P3::new(0.0, 0.0, -100.0),
            max: P3::new(100.0, 100.0, 0.0),
        }
    }

    fn pass(depth: f64) -> Toolpath {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(20.0, 50.0, 10.0));
        tp.feed_to(P3::new(20.0, 50.0, depth), 300.0);
        tp.feed_to(P3::new(80.0, 50.0, depth), 1000.0);
        tp.rapid_to(P3::new(80.0, 50.0, 10.0));
        tp
    }

    /// A shallow finish: the plate (50 mm above the tip at the normal gap)
    /// stays far above the stock, so the solver gives the normal gap.
    #[test]
    fn a_shallow_finish_takes_the_normal_gap() {
        let head = MachineHead::dust_shoe();
        let tp = pass(-3.0);
        let solution = solve_gap(
            &head,
            45.0,
            &ObstacleSet::default(),
            &stock(),
            &[(&tp, JoinStart::Unknown)],
        )
        .unwrap()
        .unwrap();
        assert_eq!(solution.needed_gap_mm, 25.0);
        assert!(solution.fits);
        assert_eq!(
            resolve(&head, ShoeSetting::Auto, Some(solution)),
            ResolvedShoe::Gap { gap_mm: 25.0 }
        );
    }

    /// A deep plunge beside a tall clamp: the plate (r 40) reaches over the
    /// clamp, and the gap that clears it is above the travel range. The
    /// shoe is raised clear.
    #[test]
    fn a_deep_plunge_beside_a_tall_clamp_raises_the_shoe() {
        let mut head = MachineHead::dust_shoe();
        if let Some(shoe) = head.shoe.as_mut() {
            // Brush end from 40 below to 10 above the holder face.
            shoe.travel_mm = Some(ShoeTravel {
                min_mm: -40.0,
                max_mm: 10.0,
            });
        }
        let clamp = ObstacleSet::resolve(
            &[Obstacle {
                id: ObstacleId::from_raw(0),
                name: "Tall clamp".to_owned(),
                enabled: true,
                clearance: 3.0,
                geometry: ObstacleGeometry::Box {
                    min: [105.0, 40.0, -30.0],
                    max: [125.0, 60.0, 60.0],
                },
            }],
            &[],
        )
        .unwrap();
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(90.0, 50.0, 70.0));
        tp.feed_to(P3::new(90.0, 50.0, -25.0), 300.0);
        tp.rapid_to(P3::new(90.0, 50.0, 70.0));
        let solution = solve_gap(&head, 45.0, &clamp, &stock(), &[(&tp, JoinStart::Unknown)])
            .unwrap()
            .unwrap();
        // The plate bottom (gap + 25) must reach the grown clamp top 63 with
        // the tip at -25: gap >= 63 - (-25) - 25 = 63. The travel allows 55.
        assert_eq!(solution.needed_gap_mm, 63.0);
        assert!(!solution.fits);
        assert_eq!(
            resolve(&head, ShoeSetting::Auto, Some(solution)),
            ResolvedShoe::Raised
        );
        // A stated gap stays as stated.
        assert_eq!(
            resolve(&head, ShoeSetting::Gap { gap_mm: 30.0 }, Some(solution)),
            ResolvedShoe::Gap { gap_mm: 30.0 }
        );
    }

    /// The check at the solver gap is clear; one millimetre lower, the
    /// shoe plate strikes the stock.
    #[test]
    fn the_check_is_clear_at_the_solver_gap_and_strikes_below_it() {
        let head = MachineHead::dust_shoe();
        let tp = pass(-60.0);
        let solution = solve_gap(
            &head,
            45.0,
            &ObstacleSet::default(),
            &stock(),
            &[(&tp, JoinStart::Unknown)],
        )
        .unwrap()
        .unwrap();
        // The plate bottom (gap + 25 above a tip at -60) must reach Z 0:
        // gap 35. The shoe shelf (gap + 30) needs only 30.
        assert_eq!(solution.needed_gap_mm, 35.0);
        let check = |gap_mm: f64| {
            let assembly = ToolAssembly {
                cutter_radius: 3.0,
                cutter_length: 20.0,
                shank_diameter: 6.0,
                stickout: 45.0,
                head: head.bands(45.0, ShoePosition::At { gap_mm }),
            };
            ObstacleSet::default()
                .for_tool(&assembly, Some(&stock()))
                .unwrap()
                .check_toolpath(&tp, JoinStart::Unknown)
        };
        assert!(check(35.0).is_empty());
        let hits = check(34.0);
        assert!(!hits.is_empty());
        assert_eq!(
            hits[0].segment,
            crate::stock::collision::AssemblySegment::Shoe
        );
        assert_eq!(hits[0].kind, crate::stock::collision::CollisionKind::Stock);
    }
}
