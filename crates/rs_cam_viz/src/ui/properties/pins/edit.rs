//! The pure edits of the pin wizard (P7, round 2).
//!
//! Each function reads a pin set (and the flip map) and returns the new
//! set. Every edit keeps the set symmetric under the flip that the setups
//! give (round 2, lead ruling 5):
//!
//! - a set move goes ALONG the flip axis only ([`move_along_axis`]);
//! - the distance off the axis changes only through the 3-pin pattern's
//!   "Pair off axis" value ([`axis_pair_pins`]);
//! - a diameter changes per pattern role ([`with_diameters`]).
//!
//! No function here holds state or draws.

use rs_cam_core::compute::alignment_pins::{
    AlignmentPin, AxisPairPattern, MixedPairPattern, PIN_CAD_ROUNDING_MM, PIN_MATCH_TOL_MM,
    PinSetupMap, axis_plus_mirror_pair, translate, two_mixed_diameters,
};
use rs_cam_core::session::PinPreset;

/// The smallest distance of a pair pin from the axis that the preset
/// controls allow. A pair pin closer than [`PIN_MATCH_TOL_MM`] reads as an
/// axis pin, and the set stops fitting the preset.
pub const MIN_PAIR_ACROSS_MM: f64 = 2.0 * PIN_MATCH_TOL_MM;

/// The smallest step between the two diameters of the mixed preset. A
/// difference under the CAD rounding is one size.
pub const MIN_DIAMETER_STEP_MM: f64 = PIN_CAD_ROUNDING_MM;

/// The large dowel size of a new mixed preset when no larger drill is in
/// the project: the small size plus twice [`PIN_MATCH_TOL_MM`], the hole
/// slop a dowel finds its hole within. It is a start value, not a dowel
/// standard: the operator sets the large dowel to the dowel he has.
#[must_use]
pub fn mixed_start_large_diameter(small_d: f64) -> f64 {
    small_d + 2.0 * PIN_MATCH_TOL_MM
}

/// The large dowel size of a new mixed preset: the smallest drill of the
/// project that is larger than `small_d`, else
/// [`mixed_start_large_diameter`].
#[must_use]
pub fn mixed_large_diameter(small_d: f64, drill_diameters: &[f64]) -> f64 {
    drill_diameters
        .iter()
        .copied()
        .find(|d| *d > small_d + MIN_DIAMETER_STEP_MM)
        .unwrap_or_else(|| mixed_start_large_diameter(small_d))
}

/// The mixed preset (two pins of `small_d` and `large_d` on the flip axis)
/// made from `current` (R2, R4).
///
/// The positions: two pins of `current` on the axis keep their places;
/// else the axis positions of the placer's plan `plan`.
///
/// # Errors
///
/// A sentence when no two axis positions exist or the map has no flip
/// axis.
pub fn mixed_preset_pins(
    current: &[AlignmentPin],
    map: &PinSetupMap,
    plan: Option<&[AlignmentPin; 3]>,
    small_d: f64,
    large_d: f64,
) -> Result<Vec<AlignmentPin>, String> {
    let alongs = match current {
        [a, b] => along_on_axis(map, a).zip(along_on_axis(map, b)),
        _ => None,
    }
    .or_else(|| {
        let fit = rs_cam_core::compute::alignment_pins::fit_axis_plus_mirror_pair(plan?, map)?;
        Some((fit.axis_along, fit.pair_along))
    });
    let Some((small_along, large_along)) = alongs else {
        return Err(
            "The mixed preset needs two pins on the flip axis or a placer plan.".to_owned(),
        );
    };
    mixed_pins(
        map,
        MixedPairPattern {
            small_along,
            large_along,
            small_diameter: small_d,
            large_diameter: large_d,
        },
    )
    .ok_or_else(|| "No flip axis.".to_owned())
}

/// The pins of the 3-pin preset with `pattern`, or `None` when `map` has
/// no flip axis.
#[must_use]
pub fn axis_pair_pins(map: &PinSetupMap, pattern: AxisPairPattern) -> Option<Vec<AlignmentPin>> {
    axis_plus_mirror_pair(map, pattern).map(|pins| pins.to_vec())
}

/// The pins of the mixed preset with `pattern`, or `None` when `map` has no
/// flip axis.
#[must_use]
pub fn mixed_pins(map: &PinSetupMap, pattern: MixedPairPattern) -> Option<Vec<AlignmentPin>> {
    two_mixed_diameters(map, pattern).map(|pins| pins.to_vec())
}

/// The preset `preset` with new dowel sizes, per pattern role: the 3-pin
/// preset takes `small_d` for all three pins; the mixed preset takes
/// `small_d` and `large_d`. The positions do not change. `None` for a
/// custom set or a map with no flip axis.
#[must_use]
pub fn with_diameters(
    map: &PinSetupMap,
    preset: Option<PinPreset>,
    small_d: f64,
    large_d: f64,
) -> Option<Vec<AlignmentPin>> {
    match preset? {
        PinPreset::AxisPlusMirrorPair(fit) => axis_pair_pins(
            map,
            AxisPairPattern {
                diameter: small_d,
                ..fit
            },
        ),
        PinPreset::TwoMixedDiameters(fit) => mixed_pins(
            map,
            MixedPairPattern {
                small_diameter: small_d,
                large_diameter: large_d,
                ..fit
            },
        ),
        PinPreset::Custom => None,
    }
}

/// The unit direction of the flip axis, from its first end to its second.
/// `None` when `map` has no flip axis.
#[must_use]
pub fn axis_unit(map: &PinSetupMap) -> Option<(f64, f64)> {
    let [a, b] = map.axis_segment()?;
    let len = (b.0 - a.0).hypot(b.1 - a.1);
    (len > 0.0).then(|| ((b.0 - a.0) / len, (b.1 - a.1) / len))
}

/// Every pin moved `distance` mm along the flip axis. A move along the
/// axis keeps a symmetric set symmetric. With no flip axis the set does
/// not move.
#[must_use]
pub fn move_along_axis(
    pins: &[AlignmentPin],
    map: &PinSetupMap,
    distance: f64,
) -> Vec<AlignmentPin> {
    match axis_unit(map) {
        Some((ux, uy)) => translate(pins, distance * ux, distance * uy),
        None => pins.to_vec(),
    }
}

/// The distance along the flip axis of a pin ON the axis (within
/// [`PIN_MATCH_TOL_MM`]), or `None` for a pin off it.
#[must_use]
pub fn along_on_axis(map: &PinSetupMap, pin: &AlignmentPin) -> Option<f64> {
    let [a, b] = map.axis_segment()?;
    let len = (b.0 - a.0).hypot(b.1 - a.1);
    let u = ((b.0 - a.0) / len, (b.1 - a.1) / len);
    let d = (pin.x - a.0, pin.y - a.1);
    let along = d.0 * u.0 + d.1 * u.1;
    let across = -d.0 * u.1 + d.1 * u.0;
    (across.abs() <= PIN_MATCH_TOL_MM).then_some(along)
}

/// The length of the flip axis inside the stock, in mm. Zero when `map`
/// has no axis.
#[must_use]
pub fn axis_length(map: &PinSetupMap) -> f64 {
    map.axis_segment()
        .map_or(0.0, |[a, b]| (b.0 - a.0).hypot(b.1 - a.1))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use rs_cam_core::compute::alignment_pins::{
        fit_axis_plus_mirror_pair, fit_two_mixed_diameters, validate_pins_for_flip,
    };
    use rs_cam_core::compute::transform::{FaceUp, ZRotation};

    fn bottom_map() -> PinSetupMap {
        PinSetupMap::new(FaceUp::Bottom, ZRotation::Deg0, 65.0, 65.0)
    }

    fn mini_pattern() -> AxisPairPattern {
        AxisPairPattern {
            axis_along: 59.0,
            pair_along: 6.0,
            pair_across: 26.5,
            diameter: 6.0,
        }
    }

    /// A preset edit is the inverse of the fit: a drag of `pair_across`
    /// writes pins that read back with the new value.
    #[test]
    fn a_pattern_edit_reads_back_through_the_fit() {
        let map = bottom_map();
        let pins = axis_pair_pins(&map, mini_pattern()).unwrap();
        let fit = fit_axis_plus_mirror_pair(&pins, &map).unwrap();
        assert!((fit.pair_across - 26.5).abs() < 1e-9);
        assert!((axis_length(&map) - 65.0).abs() < 1e-9);
    }

    /// A move along the axis keeps the set symmetric, on every in-plane
    /// flip, the diagonal of a quarter turn too.
    #[test]
    fn a_move_along_the_axis_keeps_the_set_seated() {
        for turn in [
            ZRotation::Deg0,
            ZRotation::Deg90,
            ZRotation::Deg180,
            ZRotation::Deg270,
        ] {
            let map = PinSetupMap::new(FaceUp::Bottom, turn, 65.0, 65.0);
            let pins = axis_pair_pins(
                &map,
                AxisPairPattern {
                    axis_along: 40.0,
                    pair_along: 20.0,
                    pair_across: 15.0,
                    diameter: 6.0,
                },
            )
            .unwrap();
            let moved = move_along_axis(&pins, &map, 3.5);
            assert_ne!(moved, pins);
            let report = validate_pins_for_flip(&moved, FaceUp::Bottom, turn, 65.0, 65.0);
            assert!(report.seats(), "{turn:?}: {report:?}");
            let fit = fit_axis_plus_mirror_pair(&moved, &map).unwrap();
            assert!((fit.pair_across - 15.0).abs() < 1e-9, "{turn:?}");
        }
        // No flip axis: no move.
        let top = PinSetupMap::new(FaceUp::Top, ZRotation::Deg0, 65.0, 65.0);
        let pins = vec![AlignmentPin::new(10.0, 10.0, 6.0)];
        assert_eq!(move_along_axis(&pins, &top, 3.0), pins);
    }

    /// A dowel size changes per pattern role; the positions stay.
    #[test]
    fn a_size_change_keeps_the_pattern() {
        let map = bottom_map();
        let pins = axis_pair_pins(&map, mini_pattern()).unwrap();
        let fit = fit_axis_plus_mirror_pair(&pins, &map).unwrap();
        let out =
            with_diameters(&map, Some(PinPreset::AxisPlusMirrorPair(fit)), 8.0, 10.0).unwrap();
        assert!(out.iter().all(|p| (p.diameter - 8.0).abs() < 1e-12));
        let refit = fit_axis_plus_mirror_pair(&out, &map).unwrap();
        assert!((refit.pair_across - 26.5).abs() < 1e-9);

        let mixed = mixed_preset_pins(&[], &map, None, 6.0, 8.0);
        assert!(mixed.is_err(), "no axis pins and no plan: a refusal");
        let mixed = mixed_pins(
            &map,
            MixedPairPattern {
                small_along: 6.0,
                large_along: 59.0,
                small_diameter: 6.0,
                large_diameter: 8.0,
            },
        )
        .unwrap();
        let fit = fit_two_mixed_diameters(&mixed, &map).unwrap();
        let out = with_diameters(&map, Some(PinPreset::TwoMixedDiameters(fit)), 5.0, 9.0).unwrap();
        let fit = fit_two_mixed_diameters(&out, &map).unwrap();
        assert!((fit.small_diameter - 5.0).abs() < 1e-12);
        assert!((fit.large_diameter - 9.0).abs() < 1e-12);
        assert!(with_diameters(&map, Some(PinPreset::Custom), 5.0, 9.0).is_none());
    }
}
