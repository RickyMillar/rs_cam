//! The intrinsic keying check (P1, 2026-10-12).
//!
//! A part on spoilboard dowels has no edge reference. The holes in the
//! part locate it, so the part can take any rigid placement that puts
//! every hole on a dowel. With the flipped part face down, a wrong seating
//! is the correct seating times a proper (rotation) symmetry of the hole
//! pattern. This module finds the closest such symmetry.
//!
//! A turned-over part is a different question: no pin pattern can detect
//! "the operator forgot to flip", because a pattern that seats under the
//! flip has a mirror symmetry, and the unflipped part then seats too.

use super::{AlignmentPin, PIN_GEOM_EPS_MM, PIN_MATCH_TOL_MM};

/// The closest wrong seating of a pin pattern: a turn of the part, on the
/// board, that puts every hole near a dowel.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WrongSeating {
    /// The turn of the part, in degrees, in `(-180, 180]`.
    pub turn_deg: f64,
    /// The worst distance, over the dowels, from a dowel to the nearest
    /// hole after the turn. Zero means the part seats in that position.
    pub miss_mm: f64,
}

/// The smallest worst-pin miss that blocks a wrong seating, in mm.
///
/// Derivation: a dowel whose centre lands inside the edge of a hole finds
/// the hole on its chamfer and the operator can force it in. A dowel
/// whose centre lands on solid wood, outside the hole edge, cannot. So the
/// bar is the hole radius. The check uses the LARGEST radius in the
/// pattern, because the worst pin can land on any hole and the larger
/// hole is the easier one to force. For the usual 6 mm dowel the bar is
/// 3 mm. With no pins the bar is zero, and nothing is judged.
#[must_use]
pub fn keying_bar_mm(pins: &[AlignmentPin]) -> f64 {
    pins.iter()
        .map(|p| 0.5 * p.diameter)
        .fold(0.0_f64, f64::max)
}

/// A proper rigid motion in the plane: `x -> to + R(turn) (x - from)`.
#[derive(Debug, Clone, Copy)]
struct Motion {
    cos: f64,
    sin: f64,
    from: (f64, f64),
    to: (f64, f64),
}

impl Motion {
    fn rotation_about(centre: (f64, f64), turn_rad: f64) -> Self {
        Self {
            cos: turn_rad.cos(),
            sin: turn_rad.sin(),
            from: centre,
            to: centre,
        }
    }

    fn apply(&self, x: f64, y: f64) -> (f64, f64) {
        let dx = x - self.from.0;
        let dy = y - self.from.1;
        (
            self.to.0 + self.cos * dx - self.sin * dy,
            self.to.1 + self.sin * dx + self.cos * dy,
        )
    }

    fn turn_deg(&self) -> f64 {
        self.sin.atan2(self.cos).to_degrees()
    }
}

/// How one motion seats the part.
struct Fit {
    /// The worst dowel-to-nearest-hole distance.
    miss_mm: f64,
    /// For each dowel `i`, the hole `matches[i]` that lands nearest to it.
    matches: Vec<usize>,
    /// A dowel lands nearest to a hole that is smaller than the dowel. A
    /// bigger dowel cannot enter a smaller hole, so the seating is blocked.
    blocked: bool,
    /// The largest distance a pin moves under the motion.
    displacement_mm: f64,
}

fn evaluate(pins: &[AlignmentPin], motion: &Motion) -> Fit {
    let images: Vec<(f64, f64)> = pins.iter().map(|p| motion.apply(p.x, p.y)).collect();
    let mut miss_mm: f64 = 0.0;
    let mut matches = Vec::with_capacity(pins.len());
    let mut blocked = false;
    for dowel in pins {
        let mut nearest = (usize::MAX, f64::INFINITY);
        for (j, &(hx, hy)) in images.iter().enumerate() {
            let d = (hx - dowel.x).hypot(hy - dowel.y);
            if d < nearest.1 {
                nearest = (j, d);
            }
        }
        let (j, d) = nearest;
        miss_mm = miss_mm.max(d);
        matches.push(j);
        if let Some(hole) = pins.get(j)
            && dowel.diameter > hole.diameter + PIN_GEOM_EPS_MM
        {
            blocked = true;
        }
    }
    let displacement_mm = pins
        .iter()
        .zip(&images)
        .map(|(p, &(ix, iy))| (ix - p.x).hypot(iy - p.y))
        .fold(0.0_f64, f64::max);
    Fit {
        miss_mm,
        matches,
        blocked,
        displacement_mm,
    }
}

/// The proper rigid motion that best moves the matched holes onto their
/// dowels, in the least-squares sense (the 2D Kabsch fit).
fn best_fit(pins: &[AlignmentPin], matches: &[usize]) -> Option<Motion> {
    let n = matches.len() as f64;
    if n == 0.0 {
        return None;
    }
    let mut src = (0.0, 0.0);
    let mut dst = (0.0, 0.0);
    for (i, &j) in matches.iter().enumerate() {
        let hole = pins.get(j)?;
        let dowel = pins.get(i)?;
        src = (src.0 + hole.x, src.1 + hole.y);
        dst = (dst.0 + dowel.x, dst.1 + dowel.y);
    }
    src = (src.0 / n, src.1 / n);
    dst = (dst.0 / n, dst.1 / n);
    let mut dot = 0.0;
    let mut cross = 0.0;
    for (i, &j) in matches.iter().enumerate() {
        let hole = pins.get(j)?;
        let dowel = pins.get(i)?;
        let a = (hole.x - src.0, hole.y - src.1);
        let b = (dowel.x - dst.0, dowel.y - dst.1);
        dot += a.0 * b.0 + a.1 * b.1;
        cross += a.0 * b.1 - a.1 * b.0;
    }
    let turn = cross.atan2(dot);
    Some(Motion {
        cos: turn.cos(),
        sin: turn.sin(),
        from: src,
        to: dst,
    })
}

/// The closest wrong seating of `pins`, or `None` when no turn of the part
/// seats it (every candidate is blocked, or no candidate exists).
///
/// The method:
///
/// 1. A rigid motion that maps a finite point set onto itself keeps its
///    centroid. Take the pin `f` farthest from the centroid. A symmetry
///    takes `f` onto a pin at the same radius, so the candidate turns are
///    the turns about the centroid that take `f` onto each other pin. This
///    enumeration is complete. The identity is excluded: a turn that moves
///    `f` by less than [`PIN_MATCH_TOL_MM`] is the correct seating.
/// 2. A pin at a radius different from `f` by more than twice the keying
///    bar is not a candidate. A motion that seats within the bar moves the
///    centroid by at most the bar, so `f` lands at most twice the bar from
///    a pin of its own radius. Such a candidate cannot change the verdict.
/// 3. Each candidate turn is then refined: the holes are matched to the
///    nearest dowels, and the least-squares rigid motion (turn plus shift)
///    of that match replaces the turn while the worst miss falls. The turn
///    about the centroid alone over-states the clearance of a pattern that
///    is near a symmetry. On the review's 65 x 65 axis-plus-pair example
///    the plain turn gives 6.9 mm and the refined fit 4.8 mm.
/// 4. A candidate whose final fit puts a dowel on a smaller hole is
///    blocked; it does not count as a seating.
///
/// One pin, or pins that all sit on the centroid, turn freely: the result
/// is a 180 deg turn that misses by zero.
#[must_use]
pub fn closest_wrong_seating(pins: &[AlignmentPin]) -> Option<WrongSeating> {
    closest_wrong_seating_images(pins).map(|(seating, _)| seating)
}

/// [`closest_wrong_seating`] and where each hole of `pins` lands in that
/// wrong seating, in the stock-local frame and in pin order.
///
/// The viewport draws these points as the "wrong-orientation ghost". When
/// a ghost hole falls on a real dowel, the part seats in that wrong
/// position. One pin, or pins that all sit on the centroid, turn freely:
/// the images are then the half turn about the centroid.
#[must_use]
pub fn closest_wrong_seating_images(
    pins: &[AlignmentPin],
) -> Option<(WrongSeating, Vec<(f64, f64)>)> {
    if pins.is_empty() {
        return None;
    }
    let n = pins.len() as f64;
    let centre = (
        pins.iter().map(|p| p.x).sum::<f64>() / n,
        pins.iter().map(|p| p.y).sum::<f64>() / n,
    );
    let radius = |p: &AlignmentPin| (p.x - centre.0).hypot(p.y - centre.1);
    let (far_index, far) = pins
        .iter()
        .enumerate()
        .max_by(|a, b| radius(a.1).total_cmp(&radius(b.1)))?;
    let far_radius = radius(far);
    if far_radius <= PIN_MATCH_TOL_MM {
        let half_turn = Motion::rotation_about(centre, std::f64::consts::PI);
        return Some((
            WrongSeating {
                turn_deg: 180.0,
                miss_mm: 0.0,
            },
            pins.iter().map(|p| half_turn.apply(p.x, p.y)).collect(),
        ));
    }
    let gate = 2.0 * keying_bar_mm(pins);
    let far_angle = (far.y - centre.1).atan2(far.x - centre.0);

    let mut best: Option<(WrongSeating, Motion)> = None;
    for (k, target) in pins.iter().enumerate() {
        if k == far_index || (radius(target) - far_radius).abs() > gate {
            continue;
        }
        let turn = (target.y - centre.1).atan2(target.x - centre.0) - far_angle;
        // The chord `f` moves along under the turn.
        let chord = 2.0 * far_radius * (0.5 * turn).sin().abs();
        if chord < PIN_MATCH_TOL_MM {
            continue;
        }
        let mut motion = Motion::rotation_about(centre, turn);
        let mut fit = evaluate(pins, &motion);
        // Each refit either lowers the worst miss or stops the loop. The
        // cap only bounds the loop; a pattern of n pins has at most n
        // distinct nearest matches per dowel.
        for _ in 0..pins.len() {
            let Some(next) = best_fit(pins, &fit.matches) else {
                break;
            };
            let next_fit = evaluate(pins, &next);
            if next_fit.miss_mm < fit.miss_mm - PIN_GEOM_EPS_MM {
                motion = next;
                fit = next_fit;
            } else {
                break;
            }
        }
        if fit.blocked || fit.displacement_mm < PIN_MATCH_TOL_MM {
            continue;
        }
        let seating = WrongSeating {
            turn_deg: motion.turn_deg(),
            miss_mm: fit.miss_mm,
        };
        if best.is_none_or(|(b, _)| seating.miss_mm < b.miss_mm) {
            best = Some((seating, motion));
        }
    }
    best.map(|(seating, motion)| {
        let images = pins.iter().map(|p| motion.apply(p.x, p.y)).collect();
        (seating, images)
    })
}

/// `true` when no wrong seating of `pins` comes within the keying bar.
///
/// Fewer than two pins never key: the part turns about one pin.
#[must_use]
pub fn pattern_is_keyed(pins: &[AlignmentPin]) -> bool {
    if pins.len() < 2 {
        return false;
    }
    closest_wrong_seating(pins).is_none_or(|w| w.miss_mm >= keying_bar_mm(pins) - PIN_GEOM_EPS_MM)
}
