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

use super::{AlignmentPin, PIN_GEOM_EPS_MM, PIN_MATCH_TOL_MM, PinSetupMap};

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

/// The axis frame of a flip: an origin on the axis, the unit direction
/// along it and the unit normal to its left.
///
/// The placer (`placement.rs`) searches in this frame too, so one frame
/// serves the presets and the search.
#[derive(Debug, Clone, Copy)]
pub(super) struct AxisFrame {
    origin: (f64, f64),
    u: (f64, f64),
    n: (f64, f64),
}

impl AxisFrame {
    pub(super) fn of(map: &PinSetupMap) -> Option<Self> {
        let [a, b] = map.axis_segment()?;
        let len = (b.0 - a.0).hypot(b.1 - a.1);
        let u = ((b.0 - a.0) / len, (b.1 - a.1) / len);
        Some(Self {
            origin: a,
            u,
            n: (-u.1, u.0),
        })
    }

    pub(super) fn point(&self, along: f64, across: f64) -> (f64, f64) {
        (
            self.origin.0 + along * self.u.0 + across * self.n.0,
            self.origin.1 + along * self.u.1 + across * self.n.1,
        )
    }

    pub(super) fn coords(&self, x: f64, y: f64) -> (f64, f64) {
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

/// Read the [`MixedPairPattern`] back from a stored set, in any pin order.
///
/// The set matches when it has two pins whose diameters differ by more
/// than the floating-point slop, and each pin is within
/// [`PIN_MATCH_TOL_MM`] of the flip axis. Otherwise the result is `None`,
/// and the set is a custom set. The read-back is the inverse of
/// [`two_mixed_diameters`]: each `along` is the pin's distance along the
/// axis.
#[must_use]
pub fn fit_two_mixed_diameters(
    pins: &[AlignmentPin],
    map: &PinSetupMap,
) -> Option<MixedPairPattern> {
    let frame = AxisFrame::of(map)?;
    let [a, b] = pins else {
        return None;
    };
    if (a.diameter - b.diameter).abs() <= PIN_GEOM_EPS_MM {
        return None;
    }
    let (small, large) = if a.diameter < b.diameter {
        (a, b)
    } else {
        (b, a)
    };
    let small_at = frame.coords(small.x, small.y);
    let large_at = frame.coords(large.x, large.y);
    if small_at.1.abs() > PIN_MATCH_TOL_MM || large_at.1.abs() > PIN_MATCH_TOL_MM {
        return None;
    }
    Some(MixedPairPattern {
        small_along: small_at.0,
        large_along: large_at.0,
        small_diameter: small.diameter,
        large_diameter: large.diameter,
    })
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
    fn the_mixed_pair_reads_back_in_any_order() {
        let pattern = MixedPairPattern {
            small_along: 6.0,
            large_along: 59.0,
            small_diameter: 6.0,
            large_diameter: 8.0,
        };
        for turn in [ZRotation::Deg0, ZRotation::Deg180] {
            let map = bottom(turn);
            let [small, large] = two_mixed_diameters(&map, pattern).expect("an in-plane flip");
            for pins in [
                vec![small.clone(), large.clone()],
                vec![large.clone(), small.clone()],
            ] {
                let fit = fit_two_mixed_diameters(&pins, &map).expect("the preset reads back");
                assert!(
                    close(fit.small_along, pattern.small_along),
                    "{turn:?}: {fit:?}"
                );
                assert!(
                    close(fit.large_along, pattern.large_along),
                    "{turn:?}: {fit:?}"
                );
                assert_eq!(fit.small_diameter, pattern.small_diameter);
                assert_eq!(fit.large_diameter, pattern.large_diameter);
            }
        }
    }

    #[test]
    fn a_set_that_is_not_the_mixed_pair_reads_back_as_custom() {
        let map = bottom(ZRotation::Deg0);
        // One diameter: not the preset.
        let same = vec![
            AlignmentPin::new(6.0, D / 2.0, 6.0),
            AlignmentPin::new(59.0, D / 2.0, 6.0),
        ];
        assert!(fit_two_mixed_diameters(&same, &map).is_none());
        // Off the axis by more than the match tolerance.
        let off = vec![
            AlignmentPin::new(6.0, D / 2.0, 6.0),
            AlignmentPin::new(59.0, D / 2.0 + 2.0 * PIN_MATCH_TOL_MM, 8.0),
        ];
        assert!(fit_two_mixed_diameters(&off, &map).is_none());
        // Three pins.
        let three = vec![
            AlignmentPin::new(6.0, D / 2.0, 6.0),
            AlignmentPin::new(30.0, D / 2.0, 6.0),
            AlignmentPin::new(59.0, D / 2.0, 8.0),
        ];
        assert!(fit_two_mixed_diameters(&three, &map).is_none());
        // A top setup has no flip axis.
        let top = PinSetupMap::new(FaceUp::Top, ZRotation::Deg0, W, D);
        assert!(fit_two_mixed_diameters(&same, &top).is_none());
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
