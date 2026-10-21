//! Bore: mill a circle to its drawn size with a helix (geometry selection
//! P6.6, design 4.4, lead rulings L-11 and L-15).
//!
//! A circle of diameter `d` and a tool of diameter `D` give the tool-centre
//! radius `r = (d - D) / 2`: the tool edge then cuts the wall at `d / 2`.
//! One helix of radius `r` sweeps the ring `r - D/2 .. r + D/2`. It reaches
//! the centre when `r <= D/2`, that is when `d <= 2 D`
//! (`selection::hole_fit`), so the hole has no core.
//!
//! For each hole the kernel emits:
//!
//! 1. A rapid over the centre at the safe Z, then a rapid down to the
//!    start Z (the hole top plus the start clearance).
//! 2. A feed out to the rough radius `r - a` (`a` = the finish allowance)
//!    at the start Z, above the material.
//! 3. A helix down to the floor in whole turns: `n = ceil(depth / pitch)`
//!    turns of `depth / n` each. Counter-clockwise for a climb cut (the
//!    wall is on the right of the travel), else clockwise.
//! 4. One full turn on the floor, so the floor is flat.
//! 5. With `a > 0`: a feed out to `r`, one full turn on the floor, and a
//!    helix up the wall to the start Z (the finish of the wall).
//! 6. A feed back to the centre, then a rapid up to the safe Z.
//!
//! Each turn is [`STEPS_PER_TURN`] line segments; the arc-fit dressup can
//! fit arcs. The helix and the floor turns are cut intents
//! ([`MoveIntent::ClearingCut`], and [`MoveIntent::FinishingCut`] for the
//! finish), never [`MoveIntent::EntryHelix`]: a bore is the cut itself, and
//! the steady-state gate leaves out entry spans (L-15).

use std::f64::consts::TAU;

use crate::geo::{P2, P3};
use crate::toolpath::{MoveIntent, Toolpath};

/// The line segments of one helix turn and of one floor turn: 10 degrees
/// each, the step of the helix entry (`dressup::entry_descent`).
pub const STEPS_PER_TURN: usize = 36;

/// One hole to bore, in the emission frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoreHole {
    /// The centre of the circle.
    pub centre: P2,
    /// The drawn diameter `d` of the circle (mm).
    pub diameter: f64,
    /// The material top over the hole (mm): the stock top, or on the
    /// remaining stock the column top over the circle (D8).
    pub top_z: f64,
    /// The floor Z (mm).
    pub floor_z: f64,
}

/// The parameters of a bore, the same for every hole.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoreParams {
    /// The tool diameter `D` (mm).
    pub tool_diameter: f64,
    /// The largest depth of one helix turn (mm).
    pub pitch: f64,
    /// True: counter-clockwise (climb). False: clockwise.
    pub climb: bool,
    /// The radial finish allowance `a` (mm). `0` cuts one helix.
    pub finish_allowance: f64,
    /// The feed of the rough helix (mm/min).
    pub helix_feed: f64,
    /// The feed of the floor turns, the finish and the moves in air
    /// (mm/min).
    pub feed: f64,
    /// The Z of the rapids between holes (mm).
    pub safe_z: f64,
    /// The height above the hole top where the helix starts (mm).
    pub start_clearance: f64,
}

/// Why the kernel refuses a hole.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BoreRefusal {
    /// The rough radius `r - a` is zero or less: the finish allowance is
    /// not smaller than the tool-centre radius of this hole.
    AllowanceTooLarge {
        centre: P2,
        diameter: f64,
        /// The tool-centre radius `r` (mm).
        radius: f64,
        allowance: f64,
    },
    /// The pitch is not a finite number above zero.
    BadPitch { pitch: f64 },
}

impl std::fmt::Display for BoreRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            Self::AllowanceTooLarge {
                centre,
                diameter,
                radius,
                allowance,
            } => write!(
                f,
                "the finish allowance {allowance:.3} mm is not smaller than the tool-centre \
                 radius {radius:.3} mm of the circle at X {:.3} Y {:.3} (diameter \
                 {diameter:.3} mm). Set a finish allowance below {radius:.3} mm",
                centre.x, centre.y
            ),
            Self::BadPitch { pitch } => write!(
                f,
                "the helix pitch {pitch} mm is not a number above zero. Set a pitch above zero"
            ),
        }
    }
}

/// The tool-centre radius `r = (d - D) / 2` of a hole of diameter `d`.
#[must_use]
pub fn centre_radius(diameter: f64, tool_diameter: f64) -> f64 {
    (diameter - tool_diameter) / 2.0
}

/// The turn count of a helix down `depth` at most `pitch` per turn:
/// `ceil(depth / pitch)`, at least one.
#[must_use]
pub fn turn_count(depth: f64, pitch: f64) -> usize {
    let turns = (depth / pitch).ceil();
    if turns.is_finite() && turns >= 1.0 {
        // SAFETY: a finite count of at least one; a depth of a few metres
        // at the smallest pitch is far below `usize::MAX`.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let n = turns as usize;
        n
    } else {
        1
    }
}

/// Bore every hole of `holes`, in the order given.
///
/// # Errors
///
/// [`BoreRefusal`] for the first hole whose rough radius is zero or less,
/// or for a pitch that is not above zero.
pub fn bore_toolpath(holes: &[BoreHole], params: &BoreParams) -> Result<Toolpath, BoreRefusal> {
    if !(params.pitch.is_finite() && params.pitch > 0.0) {
        return Err(BoreRefusal::BadPitch {
            pitch: params.pitch,
        });
    }
    let mut tp = Toolpath::new();
    for hole in holes {
        bore_hole(&mut tp, hole, params)?;
    }
    Ok(tp)
}

/// The point at `angle` on the circle of `radius` round `centre`, at `z`.
fn on_circle(centre: P2, radius: f64, angle: f64, z: f64) -> P3 {
    P3::new(
        centre.x + radius * angle.cos(),
        centre.y + radius * angle.sin(),
        z,
    )
}

/// One helix from `from_z` to `to_z` on `radius`, in `turns` whole turns,
/// starting and ending at angle 0.
#[allow(clippy::too_many_arguments)] // SAFETY: each argument is one helix input
fn helix(
    tp: &mut Toolpath,
    centre: P2,
    radius: f64,
    from_z: f64,
    to_z: f64,
    turns: usize,
    sense: f64,
    feed: f64,
    intent: MoveIntent,
) {
    let steps = turns * STEPS_PER_TURN;
    let step_angle = sense * TAU / STEPS_PER_TURN as f64;
    for k in 1..=steps {
        let t = k as f64 / steps as f64;
        let z = from_z + (to_z - from_z) * t;
        let angle = if k == steps {
            0.0
        } else {
            step_angle * k as f64
        };
        tp.feed_to_with_intent(on_circle(centre, radius, angle, z), feed, intent);
    }
}

/// Bore one hole into `tp`. See the module doc for the moves.
///
/// # Errors
///
/// [`BoreRefusal::AllowanceTooLarge`] when the rough radius is zero or
/// less.
pub fn bore_hole(
    tp: &mut Toolpath,
    hole: &BoreHole,
    params: &BoreParams,
) -> Result<(), BoreRefusal> {
    let r = centre_radius(hole.diameter, params.tool_diameter);
    let a = params.finish_allowance.max(0.0);
    let rough = r - a;
    if rough <= 0.0 {
        return Err(BoreRefusal::AllowanceTooLarge {
            centre: hole.centre,
            diameter: hole.diameter,
            radius: r,
            allowance: a,
        });
    }
    let c = hole.centre;
    let start_z = hole.top_z + params.start_clearance;
    let floor_z = hole.floor_z;
    let sense = if params.climb { 1.0 } else { -1.0 };
    let turns = turn_count(start_z - floor_z, params.pitch);

    // 1. Over the centre, down to the start Z.
    tp.rapid_to_with_intent(P3::new(c.x, c.y, params.safe_z), MoveIntent::Linking);
    tp.rapid_to_with_intent(P3::new(c.x, c.y, start_z), MoveIntent::Linking);
    // 2. Out to the rough radius, above the material.
    tp.feed_to_with_intent(
        on_circle(c, rough, 0.0, start_z),
        params.feed,
        MoveIntent::Linking,
    );
    // 3. The helix down, 4. one turn on the floor.
    helix(
        tp,
        c,
        rough,
        start_z,
        floor_z,
        turns,
        sense,
        params.helix_feed,
        MoveIntent::ClearingCut,
    );
    helix(
        tp,
        c,
        rough,
        floor_z,
        floor_z,
        1,
        sense,
        params.feed,
        MoveIntent::ClearingCut,
    );
    // 5. The finish: out to `r`, one turn on the floor, a helix up the wall.
    let end_z = if a > 0.0 {
        tp.feed_to_with_intent(
            on_circle(c, r, 0.0, floor_z),
            params.feed,
            MoveIntent::FinishingCut,
        );
        helix(
            tp,
            c,
            r,
            floor_z,
            floor_z,
            1,
            sense,
            params.feed,
            MoveIntent::FinishingCut,
        );
        helix(
            tp,
            c,
            r,
            floor_z,
            start_z,
            turns,
            sense,
            params.feed,
            MoveIntent::FinishingCut,
        );
        start_z
    } else {
        floor_z
    };
    // 6. Back to the centre, inside the cut disc, and up.
    tp.feed_to_with_intent(P3::new(c.x, c.y, end_z), params.feed, MoveIntent::Linking);
    tp.rapid_to_with_intent(P3::new(c.x, c.y, params.safe_z), MoveIntent::Retract);
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]
mod tests {
    use super::*;
    use crate::toolpath::MoveType;

    fn params(finish_allowance: f64, climb: bool) -> BoreParams {
        BoreParams {
            tool_diameter: 6.0,
            pitch: 1.0,
            climb,
            finish_allowance,
            helix_feed: 400.0,
            feed: 1000.0,
            safe_z: 10.0,
            start_clearance: 0.5,
        }
    }

    fn hole() -> BoreHole {
        BoreHole {
            centre: P2::new(5.0, -3.0),
            diameter: 10.0,
            top_z: 0.0,
            floor_z: -4.2,
        }
    }

    /// The fed moves of `tp` with `intent`, in order.
    fn fed(tp: &Toolpath, intent: MoveIntent) -> Vec<P3> {
        tp.moves
            .iter()
            .filter(|m| m.intent == intent && !matches!(m.move_type, MoveType::Rapid))
            .map(|m| m.target)
            .collect()
    }

    /// The helix turns are `ceil(depth / pitch)`: 4.7 mm from the start Z
    /// (0.5 above the top) to the floor at a 1 mm pitch is 5 turns, then
    /// one floor turn. Each turn is `STEPS_PER_TURN` segments.
    #[test]
    fn the_turn_count_is_the_depth_over_the_pitch_rounded_up_p6_6() {
        assert_eq!(turn_count(4.7, 1.0), 5);
        assert_eq!(turn_count(4.0, 1.0), 4);
        assert_eq!(turn_count(0.2, 1.0), 1);
        let tp = bore_toolpath(&[hole()], &params(0.0, true)).unwrap();
        let cuts = fed(&tp, MoveIntent::ClearingCut);
        assert_eq!(cuts.len(), (5 + 1) * STEPS_PER_TURN);
        // No turn drops more than the pitch.
        let per_turn: Vec<f64> = cuts
            .chunks(STEPS_PER_TURN)
            .map(|turn| turn.first().unwrap().z - turn.last().unwrap().z)
            .collect();
        assert!(per_turn.iter().all(|dz| *dz <= 1.0 + 1e-9), "{per_turn:?}");
        // The last turn is flat on the floor.
        let floor_turn = &cuts[5 * STEPS_PER_TURN..];
        assert!(floor_turn.iter().all(|p| (p.z - (-4.2)).abs() < 1e-12));
        // Every cut point is on the tool-centre radius (10 - 6) / 2 = 2.
        for p in &cuts {
            let rr = (p.x - 5.0).hypot(p.y + 3.0);
            assert!((rr - 2.0).abs() < 1e-9, "radius {rr}");
        }
        // No move carries the entry-helix intent (L-15).
        assert!(tp.moves.iter().all(|m| m.intent != MoveIntent::EntryHelix));
    }

    /// The signed area swept by the helix points: positive for a
    /// counter-clockwise turn.
    fn turning(points: &[P3], centre: P2) -> f64 {
        points
            .windows(2)
            .map(|w| {
                let (ax, ay) = (w[0].x - centre.x, w[0].y - centre.y);
                let (bx, by) = (w[1].x - centre.x, w[1].y - centre.y);
                ax * by - ay * bx
            })
            .sum()
    }

    /// Climb turns counter-clockwise (the wall on the right of the
    /// travel); conventional turns clockwise.
    #[test]
    fn climb_turns_counter_clockwise_p6_6() {
        let c = hole().centre;
        let ccw = bore_toolpath(&[hole()], &params(0.0, true)).unwrap();
        let cw = bore_toolpath(&[hole()], &params(0.0, false)).unwrap();
        assert!(turning(&fed(&ccw, MoveIntent::ClearingCut), c) > 0.0);
        assert!(turning(&fed(&cw, MoveIntent::ClearingCut), c) < 0.0);
    }

    /// With a finish allowance the rough helix runs at `r - a`, and the
    /// finish (a floor turn and a helix up the wall) at `r`, to the start
    /// Z. An allowance of `r` or more refuses.
    #[test]
    fn the_finish_pass_runs_at_the_full_radius_p6_6() {
        let tp = bore_toolpath(&[hole()], &params(0.5, true)).unwrap();
        let c = hole().centre;
        let radius = |p: &P3| (p.x - c.x).hypot(p.y - c.y);
        let rough = fed(&tp, MoveIntent::ClearingCut);
        assert!(rough.iter().all(|p| (radius(p) - 1.5).abs() < 1e-9));
        let finish = fed(&tp, MoveIntent::FinishingCut);
        assert_eq!(finish.len(), 1 + (1 + 5) * STEPS_PER_TURN);
        assert!(finish.iter().all(|p| (radius(p) - 2.0).abs() < 1e-9));
        assert!(
            (finish.last().unwrap().z - 0.5).abs() < 1e-12,
            "the finish ends at the start Z"
        );
        assert!(turning(&finish, c) > 0.0, "the finish climbs too");

        let refused = bore_toolpath(&[hole()], &params(2.0, true)).unwrap_err();
        assert!(matches!(refused, BoreRefusal::AllowanceTooLarge { .. }));
        assert!(bore_toolpath(&[hole()], &params(1.99, true)).is_ok());
    }
}
