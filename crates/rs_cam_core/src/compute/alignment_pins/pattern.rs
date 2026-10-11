//! Pure edits of a pin set (P4, 2026-10-12).
//!
//! The stored truth stays `Vec<AlignmentPin>`. A pattern is not stored:
//! each function here reads a pin set (or pattern parameters) and returns
//! a new pin set. The session writes the result through one command,
//! `SetAlignmentPins`. A preset is read back from a stored set with its
//! `fit_*` function; a set that no preset fits is a custom set.
//!
//! The presets are dimensioned in the AXIS FRAME of a flip
//! ([`PinSetupMap::axis_segment`]):
//!
//! - `along` is the distance along the flip axis from its first end;
//! - `across` is the signed distance from the axis, positive to the left
//!   of the axis direction.
//!
//! For a `Bottom` setup at 0 deg the axis runs from `(0, D/2)` to
//! `(W, D/2)`, so `along` is the stock-local X and `across` is `y - D/2`.
//! For a `Bottom` setup turned 180 deg the axis runs from `(W/2, 0)` to
//! `(W/2, D)`, so `along` is Y and `across` is `W/2 - x`. One function
//! serves every in-plane flip, and the frame comes from the setup map, not
//! from a second copy of it.

use super::{AlignmentPin, PIN_GEOM_EPS_MM, PIN_MATCH_TOL_MM, PinFlipKind, PinSetupMap};

/// Move every pin by `(dx, dy)` mm. The diameters do not change.
#[must_use]
pub fn translate(pins: &[AlignmentPin], dx: f64, dy: f64) -> Vec<AlignmentPin> {
    pins.iter()
        .map(|p| AlignmentPin::new(p.x + dx, p.y + dy, p.diameter))
        .collect()
}

/// Give every pin the diameter `diameter` mm. The positions do not change.
#[must_use]
pub fn set_diameter(pins: &[AlignmentPin], diameter: f64) -> Vec<AlignmentPin> {
    pins.iter()
        .map(|p| AlignmentPin::new(p.x, p.y, diameter))
        .collect()
}

/// Make a nearly symmetric set exactly symmetric under the flip `map`.
///
/// The flip is a mirror, and the mirror is its own inverse. The function
/// matches the pins first:
///
/// - a pin whose image lands within `2 * tol` of the pin itself is an
///   axis pin (a pin at distance `t` from the axis has its image `2t`
///   away);
/// - two pins of one diameter are a pair when the image of one lands
///   within `tol` of the other.
///
/// The closest matches are taken first, and each pin is in one match only.
/// Then each axis pin moves to the midpoint of the pin and its image (a
/// point on the axis). Each pair `(i, j)` moves to `p_i' = (p_i +
/// map(p_j)) / 2` and `p_j' = map(p_i')`, which is the smallest symmetric
/// move.
///
/// A pin with no match does not move, and the validator then reports that
/// it does not seat. A map with no in-plane flip returns the set without
/// a change.
#[must_use]
pub fn snap_to_flip(pins: &[AlignmentPin], map: &PinSetupMap, tol: f64) -> Vec<AlignmentPin> {
    let mut out = pins.to_vec();
    if map.kind() != PinFlipKind::InPlane {
        return out;
    }
    let images: Vec<(f64, f64)> = pins.iter().map(|p| map.map_xy(p.x, p.y)).collect();

    // (movement, i, j): j == i is an axis pin.
    let mut candidates: Vec<(f64, usize, usize)> = Vec::new();
    for (i, (p, img)) in pins.iter().zip(&images).enumerate() {
        let self_gap = (img.0 - p.x).hypot(img.1 - p.y);
        if self_gap <= 2.0 * tol {
            candidates.push((0.5 * self_gap, i, i));
        }
        for (j, q) in pins.iter().enumerate().skip(i + 1) {
            if (p.diameter - q.diameter).abs() > PIN_GEOM_EPS_MM {
                continue;
            }
            let gap = (img.0 - q.x).hypot(img.1 - q.y);
            if gap <= tol {
                candidates.push((0.5 * gap, i, j));
            }
        }
    }
    candidates.sort_by(|a, b| a.0.total_cmp(&b.0));

    let mut used = vec![false; pins.len()];
    for (_, i, j) in candidates {
        if used.get(i).copied().unwrap_or(true) || used.get(j).copied().unwrap_or(true) {
            continue;
        }
        let (Some(p_i), Some(&img_i), Some(&img_j)) = (pins.get(i), images.get(i), images.get(j))
        else {
            continue;
        };
        if i == j {
            if let Some(slot) = out.get_mut(i) {
                slot.x = 0.5 * (p_i.x + img_i.0);
                slot.y = 0.5 * (p_i.y + img_i.1);
            }
        } else {
            let snapped = (0.5 * (p_i.x + img_j.0), 0.5 * (p_i.y + img_j.1));
            let partner = map.map_xy(snapped.0, snapped.1);
            if let Some(slot) = out.get_mut(i) {
                slot.x = snapped.0;
                slot.y = snapped.1;
            }
            if let Some(slot) = out.get_mut(j) {
                slot.x = partner.0;
                slot.y = partner.1;
            }
        }
        if let Some(flag) = used.get_mut(i) {
            *flag = true;
        }
        if let Some(flag) = used.get_mut(j) {
            *flag = true;
        }
    }
    out
}

/// The axis frame of a flip: an origin on the axis, the unit direction
/// along it and the unit normal to its left.
#[derive(Debug, Clone, Copy)]
struct AxisFrame {
    origin: (f64, f64),
    u: (f64, f64),
    n: (f64, f64),
}

impl AxisFrame {
    fn of(map: &PinSetupMap) -> Option<Self> {
        let [a, b] = map.axis_segment()?;
        let len = (b.0 - a.0).hypot(b.1 - a.1);
        let u = ((b.0 - a.0) / len, (b.1 - a.1) / len);
        Some(Self {
            origin: a,
            u,
            n: (-u.1, u.0),
        })
    }

    fn point(&self, along: f64, across: f64) -> (f64, f64) {
        (
            self.origin.0 + along * self.u.0 + across * self.n.0,
            self.origin.1 + along * self.u.1 + across * self.n.1,
        )
    }

    fn coords(&self, x: f64, y: f64) -> (f64, f64) {
        let d = (x - self.origin.0, y - self.origin.1);
        (
            d.0 * self.u.0 + d.1 * self.u.1,
            d.0 * self.n.0 + d.1 * self.n.1,
        )
    }
}

/// The preset "one pin on the flip axis plus a mirror pair", in the axis
/// frame (see the module doc).
///
/// With equal diameters this pattern keys, unless the three pins are in a
/// line or form an equilateral triangle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AxisPairPattern {
    /// The axis pin's distance along the axis, in mm.
    pub axis_along: f64,
    /// The pair's distance along the axis, in mm.
    pub pair_along: f64,
    /// The distance of each pair pin from the axis, in mm. A fitted
    /// pattern gives a positive value.
    pub pair_across: f64,
    /// The diameter of all three pins, in mm.
    pub diameter: f64,
}

/// The three pins of `pattern` under the flip `map`, as `[axis pin, pair
/// pin at -pair_across, pair pin at +pair_across]`. For a `Bottom` setup
/// at 0 deg that is `[axis, low Y, high Y]`, the order of the placer.
///
/// `None` when `map` has no flip axis.
#[must_use]
pub fn axis_plus_mirror_pair(
    map: &PinSetupMap,
    pattern: AxisPairPattern,
) -> Option<[AlignmentPin; 3]> {
    let frame = AxisFrame::of(map)?;
    let pin = |along: f64, across: f64| {
        let (x, y) = frame.point(along, across);
        AlignmentPin::new(x, y, pattern.diameter)
    };
    Some([
        pin(pattern.axis_along, 0.0),
        pin(pattern.pair_along, -pattern.pair_across),
        pin(pattern.pair_along, pattern.pair_across),
    ])
}

/// Read the [`AxisPairPattern`] back from a stored set, in any pin order.
///
/// The set matches when it has three pins of one diameter, exactly one of
/// them within [`PIN_MATCH_TOL_MM`] of the axis, and the other two a
/// mirror pair: their `along` values and their mirrored `across` values
/// agree within [`PIN_MATCH_TOL_MM`]. Otherwise the result is `None`, and
/// the set is a custom set.
///
/// The read-back is the inverse of [`axis_plus_mirror_pair`]:
/// `pair_along` is the mean of the pair, `pair_across` is half the
/// distance between the pair pins. For a set that the preset made, the
/// read-back agrees with the parameters to floating-point rounding
/// (`D/2 + h - (D/2 - h)` is not always `2h` in `f64`).
#[must_use]
pub fn fit_axis_plus_mirror_pair(
    pins: &[AlignmentPin],
    map: &PinSetupMap,
) -> Option<AxisPairPattern> {
    let frame = AxisFrame::of(map)?;
    let [p0, p1, p2] = pins else {
        return None;
    };
    let diameter = p0.diameter;
    if [p1, p2]
        .iter()
        .any(|p| (p.diameter - diameter).abs() > PIN_GEOM_EPS_MM)
    {
        return None;
    }
    let coords: Vec<(f64, f64)> = [p0, p1, p2]
        .iter()
        .map(|p| frame.coords(p.x, p.y))
        .collect();
    let on_axis: Vec<usize> = (0..3)
        .filter(|&i| coords.get(i).is_some_and(|c| c.1.abs() <= PIN_MATCH_TOL_MM))
        .collect();
    let [axis_index] = on_axis.as_slice() else {
        return None;
    };
    let mut pair = (0..3)
        .filter(|i| i != axis_index)
        .filter_map(|i| coords.get(i));
    let (Some(&a), Some(&b), Some(&axis)) = (pair.next(), pair.next(), coords.get(*axis_index))
    else {
        return None;
    };
    if (a.0 - b.0).abs() > PIN_MATCH_TOL_MM || (a.1 + b.1).abs() > PIN_MATCH_TOL_MM {
        return None;
    }
    let (lo, hi) = if a.1 <= b.1 { (a, b) } else { (b, a) };
    Some(AxisPairPattern {
        axis_along: axis.0,
        pair_along: 0.5 * (lo.0 + hi.0),
        pair_across: 0.5 * (hi.1 - lo.1),
        diameter,
    })
}

/// The preset "two pins of different diameters on the flip axis".
///
/// Two holes are enough: a half turn puts the large dowel on the small
/// hole, and a dowel does not enter a smaller hole. The preset needs two
/// drills.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MixedPairPattern {
    /// The small pin's distance along the axis, in mm.
    pub small_along: f64,
    /// The large pin's distance along the axis, in mm.
    pub large_along: f64,
    pub small_diameter: f64,
    pub large_diameter: f64,
}

/// The two pins of `pattern` on the flip axis of `map`, as `[small pin,
/// large pin]`. `None` when `map` has no flip axis.
#[must_use]
pub fn two_mixed_diameters(
    map: &PinSetupMap,
    pattern: MixedPairPattern,
) -> Option<[AlignmentPin; 2]> {
    let frame = AxisFrame::of(map)?;
    let (sx, sy) = frame.point(pattern.small_along, 0.0);
    let (lx, ly) = frame.point(pattern.large_along, 0.0);
    Some([
        AlignmentPin::new(sx, sy, pattern.small_diameter),
        AlignmentPin::new(lx, ly, pattern.large_diameter),
    ])
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::compute::alignment_pins::{pattern_is_keyed, validate_pins_for_flip};
    use crate::compute::transform::{FaceUp, ZRotation};

    const W: f64 = 65.0;
    const D: f64 = 65.0;
    /// The rounding a read-back may carry (see `fit_axis_plus_mirror_pair`).
    const ROUND_MM: f64 = 1e-9;

    fn bottom(turn: ZRotation) -> PinSetupMap {
        PinSetupMap::new(FaceUp::Bottom, turn, W, D)
    }

    fn mini_pattern() -> AxisPairPattern {
        AxisPairPattern {
            axis_along: 59.0,
            pair_along: 6.0,
            pair_across: 20.0,
            diameter: 6.0,
        }
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() <= ROUND_MM
    }

    #[test]
    fn translate_moves_every_pin_and_keeps_the_diameters() {
        let pins = vec![
            AlignmentPin::new(1.0, 2.0, 6.0),
            AlignmentPin::new(10.0, 20.0, 8.0),
        ];
        let moved = translate(&pins, 1.5, -0.5);
        assert_eq!(
            moved,
            vec![
                AlignmentPin::new(2.5, 1.5, 6.0),
                AlignmentPin::new(11.5, 19.5, 8.0),
            ]
        );
    }

    #[test]
    fn set_diameter_changes_only_the_diameter() {
        let pins = vec![
            AlignmentPin::new(1.0, 2.0, 6.0),
            AlignmentPin::new(10.0, 20.0, 8.0),
        ];
        let resized = set_diameter(&pins, 5.0);
        assert_eq!(
            resized,
            vec![
                AlignmentPin::new(1.0, 2.0, 5.0),
                AlignmentPin::new(10.0, 20.0, 5.0),
            ]
        );
    }

    #[test]
    fn the_axis_is_the_line_the_flip_keeps() {
        assert_eq!(
            bottom(ZRotation::Deg0).axis_segment(),
            Some([(0.0, 32.5), (65.0, 32.5)])
        );
        assert_eq!(
            bottom(ZRotation::Deg180).axis_segment(),
            Some([(32.5, 0.0), (32.5, 65.0)])
        );
        // A quarter turn on a square stock mirrors about a diagonal.
        let diagonal = bottom(ZRotation::Deg90).axis_segment().expect("a diagonal");
        let len = (diagonal[1].0 - diagonal[0].0).hypot(diagonal[1].1 - diagonal[0].1);
        assert!(close(len, W * std::f64::consts::SQRT_2), "{diagonal:?}");
        // Not square: no axis. Not a flip: no axis.
        let narrow = PinSetupMap::new(FaceUp::Bottom, ZRotation::Deg90, 100.0, 60.0);
        assert_eq!(narrow.axis_segment(), None);
        for face in [FaceUp::Top, FaceUp::Front, FaceUp::Left] {
            assert_eq!(
                PinSetupMap::new(face, ZRotation::Deg0, W, D).axis_segment(),
                None,
                "{face:?}"
            );
        }
    }

    #[test]
    fn the_preset_seats_keys_and_reads_back() {
        for turn in [ZRotation::Deg0, ZRotation::Deg180, ZRotation::Deg90] {
            let map = bottom(turn);
            let p = mini_pattern();
            let pins = axis_plus_mirror_pair(&map, p).expect("an in-plane flip");
            let report = validate_pins_for_flip(&pins, FaceUp::Bottom, turn, W, D);
            assert!(report.seats(), "{turn:?}: {:?}", report.warnings());
            assert!(report.keyed(), "{turn:?}: {:?}", report.warnings());

            let fitted = fit_axis_plus_mirror_pair(&pins, &map).expect("the preset fits");
            assert!(close(fitted.axis_along, p.axis_along), "{fitted:?}");
            assert!(close(fitted.pair_along, p.pair_along), "{fitted:?}");
            assert!(close(fitted.pair_across, p.pair_across), "{fitted:?}");
            assert_eq!(fitted.diameter, p.diameter);

            // Any pin order reads back the same pattern.
            let reordered = [pins[2].clone(), pins[0].clone(), pins[1].clone()];
            assert_eq!(fit_axis_plus_mirror_pair(&reordered, &map), Some(fitted));
        }
    }

    /// The round trip over a sweep of parameters: fit(make(p)) == p to the
    /// floating-point rounding, and make(fit(pins)) == pins.
    #[test]
    fn the_round_trip_holds_over_a_sweep() {
        let map = bottom(ZRotation::Deg0);
        for axis_along in [6.0, 31.7, 59.0] {
            for pair_along in [5.0, 12.3, 60.1] {
                for pair_across in [4.1, 10.0, 27.9] {
                    let p = AxisPairPattern {
                        axis_along,
                        pair_along,
                        pair_across,
                        diameter: 6.35,
                    };
                    let pins = axis_plus_mirror_pair(&map, p).unwrap();
                    let fitted = fit_axis_plus_mirror_pair(&pins, &map).unwrap();
                    assert!(close(fitted.axis_along, p.axis_along), "{p:?} {fitted:?}");
                    assert!(close(fitted.pair_along, p.pair_along), "{p:?} {fitted:?}");
                    assert!(close(fitted.pair_across, p.pair_across), "{p:?} {fitted:?}");
                    assert_eq!(fitted.diameter, p.diameter);
                    let rebuilt = axis_plus_mirror_pair(&map, fitted).unwrap();
                    for (a, b) in rebuilt.iter().zip(&pins) {
                        assert!(close(a.x, b.x) && close(a.y, b.y), "{a:?} {b:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn a_custom_set_does_not_fit_the_preset() {
        let map = bottom(ZRotation::Deg0);
        // Two pins on the axis: the mini piece as stored today.
        let two = vec![
            AlignmentPin::new(59.0, 32.5, 6.0),
            AlignmentPin::new(6.0, 32.5, 6.0),
        ];
        assert_eq!(fit_axis_plus_mirror_pair(&two, &map), None);
        // A pair that is not a mirror pair.
        let mut pins = axis_plus_mirror_pair(&map, mini_pattern())
            .unwrap()
            .to_vec();
        pins[2].x += 3.0;
        assert_eq!(fit_axis_plus_mirror_pair(&pins, &map), None);
        // Mixed diameters.
        let mut pins = axis_plus_mirror_pair(&map, mini_pattern())
            .unwrap()
            .to_vec();
        pins[1].diameter = 8.0;
        assert_eq!(fit_axis_plus_mirror_pair(&pins, &map), None);
        // No in-plane flip: nothing to fit against.
        let top = PinSetupMap::new(FaceUp::Top, ZRotation::Deg0, W, D);
        let pins = axis_plus_mirror_pair(&map, mini_pattern()).unwrap();
        assert_eq!(fit_axis_plus_mirror_pair(&pins, &top), None);
    }

    #[test]
    fn snap_to_flip_makes_a_slightly_off_set_symmetric() {
        let map = bottom(ZRotation::Deg0);
        let off = vec![
            AlignmentPin::new(59.0, 32.7, 6.0), // 0.2 mm off the axis
            AlignmentPin::new(6.1, 12.4, 6.0),  // pair, 0.1 and 0.1 off
            AlignmentPin::new(6.0, 52.5, 6.0),
        ];
        let before = validate_pins_for_flip(&off, FaceUp::Bottom, ZRotation::Deg0, W, D);
        assert!(before.seat_mismatch_mm.unwrap() > 0.1, "{before:?}");

        let snapped = snap_to_flip(&off, &map, PIN_MATCH_TOL_MM);
        let after = validate_pins_for_flip(&snapped, FaceUp::Bottom, ZRotation::Deg0, W, D);
        assert!(after.seat_mismatch_mm.unwrap() <= ROUND_MM, "{after:?}");
        // The axis pin is on the axis, and the pair mirrors exactly.
        assert!(close(snapped[0].y, 32.5), "{snapped:?}");
        assert!(close(snapped[0].x, 59.0), "{snapped:?}");
        assert!(close(snapped[1].x, snapped[2].x), "{snapped:?}");
        assert!(close(snapped[1].y, D - snapped[2].y), "{snapped:?}");
        // The smallest symmetric move: the pair x is the mean.
        assert!(close(snapped[1].x, 6.05), "{snapped:?}");
        // The fit now reads the preset.
        assert!(fit_axis_plus_mirror_pair(&snapped, &map).is_some());
    }

    #[test]
    fn snap_to_flip_leaves_an_unmatched_pin_and_a_non_flip_alone() {
        let map = bottom(ZRotation::Deg0);
        let pins = vec![
            AlignmentPin::new(59.0, 32.5, 6.0),
            AlignmentPin::new(6.0, 10.0, 6.0), // its image (6, 55) has no pin
        ];
        let snapped = snap_to_flip(&pins, &map, PIN_MATCH_TOL_MM);
        assert_eq!(snapped[1], pins[1]);
        let top = PinSetupMap::new(FaceUp::Top, ZRotation::Deg0, W, D);
        assert_eq!(snap_to_flip(&pins, &top, PIN_MATCH_TOL_MM), pins);
    }

    #[test]
    fn two_mixed_diameters_sit_on_the_axis_and_key() {
        for turn in [ZRotation::Deg0, ZRotation::Deg180] {
            let map = bottom(turn);
            let pins = two_mixed_diameters(
                &map,
                MixedPairPattern {
                    small_along: 6.0,
                    large_along: 59.0,
                    small_diameter: 6.0,
                    large_diameter: 8.0,
                },
            )
            .expect("an in-plane flip");
            assert_eq!(pins[0].diameter, 6.0);
            assert_eq!(pins[1].diameter, 8.0);
            let report = validate_pins_for_flip(&pins, FaceUp::Bottom, turn, W, D);
            assert!(report.seats(), "{turn:?}: {:?}", report.warnings());
            assert!(pattern_is_keyed(&pins), "{turn:?}: {pins:?}");
        }
    }
}
