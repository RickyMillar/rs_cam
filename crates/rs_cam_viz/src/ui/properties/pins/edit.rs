//! The pure edits of the pin section (P7).
//!
//! Each function reads a pin set (and the flip map) and returns the new
//! set. The section writes the result into the stock draft, and the draft
//! reaches the session as ONE `SetAlignmentPins` per finished edit
//! (`panel_apply::stock_edit_command`). No function here holds state or
//! draws.

use rs_cam_core::compute::alignment_pins::{
    AlignmentPin, AxisPairPattern, MixedPairPattern, PIN_CAD_ROUNDING_MM, PIN_MATCH_TOL_MM,
    PinSetupMap, axis_plus_mirror_pair, keying_bar_mm, snap_to_flip, translate,
    two_mixed_diameters,
};
use rs_cam_core::session::{PinPreset, PinSetReport};

/// The three choices of the preset picker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresetChoice {
    /// "3 pins: axis + mirror pair".
    AxisPair,
    /// "2 pins, mixed diameter".
    Mixed,
    /// Any other set.
    Custom,
}

impl PresetChoice {
    /// The picker options, in the order the picker shows them.
    pub const OPTIONS: [(Self, &'static str); 3] = [
        (Self::AxisPair, "3 pins: axis + mirror pair"),
        (Self::Mixed, "2 pins, mixed diameter"),
        (Self::Custom, "Custom"),
    ];
}

/// The preset the picker shows: the one the pins fit, else `Custom`.
#[must_use]
pub fn preset_choice(report: &PinSetReport) -> PresetChoice {
    match report.preset {
        Some(PinPreset::AxisPlusMirrorPair(_)) => PresetChoice::AxisPair,
        Some(PinPreset::TwoMixedDiameters(_)) => PresetChoice::Mixed,
        Some(PinPreset::Custom) | None => PresetChoice::Custom,
    }
}

/// The smallest distance of a pair pin from the axis that the preset
/// controls allow. A pair pin closer than [`PIN_MATCH_TOL_MM`] reads as an
/// axis pin, and the set stops fitting the preset.
pub const MIN_PAIR_ACROSS_MM: f64 = 2.0 * PIN_MATCH_TOL_MM;

/// The smallest step between the two diameters of the mixed preset. A
/// difference under the CAD rounding is one size.
pub const MIN_DIAMETER_STEP_MM: f64 = PIN_CAD_ROUNDING_MM;

/// The mixed preset (two pins of `small_d` and `large_d` on the flip axis)
/// made from `current` (R2, R4).
///
/// The positions: two pins of `current` on the axis keep their places;
/// else the axis positions of the placer's plan `plan`. The stock panel's
/// preset picker and the pin wizard call this one function.
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

/// Every pin moved by `(dx, dy)` mm.
#[must_use]
pub fn move_all(pins: &[AlignmentPin], dx: f64, dy: f64) -> Vec<AlignmentPin> {
    translate(pins, dx, dy)
}

/// The set made exactly symmetric under the flip.
///
/// The match tolerance is the keying bar, the largest pin radius: a dowel
/// whose centre lands inside a hole edge finds the hole, so a pin within
/// that distance of its mirror image is meant to be its pair.
#[must_use]
pub fn snap(pins: &[AlignmentPin], map: &PinSetupMap) -> Vec<AlignmentPin> {
    snap_to_flip(pins, map, keying_bar_mm(pins))
}

/// The centre of the set: the mean of the pin centres. `None` for no pins.
#[must_use]
pub fn set_centre(pins: &[AlignmentPin]) -> Option<(f64, f64)> {
    if pins.is_empty() {
        return None;
    }
    let n = pins.len() as f64;
    Some((
        pins.iter().map(|p| p.x).sum::<f64>() / n,
        pins.iter().map(|p| p.y).sum::<f64>() / n,
    ))
}

/// "Move the pair off the axis": a set of two equal pins on the flip axis
/// becomes the 3-pin preset. Pin 1 stays on the axis; pin 2 becomes a
/// mirror pair at its own position along the axis, `pair_across` mm each
/// side of the axis.
///
/// `pair_across` comes from the placer's plan, which finds the clear
/// distance; the function invents no distance. `None` when the set is not
/// two equal pins on the axis, or `map` has no flip axis.
#[must_use]
pub fn pair_off_axis(
    pins: &[AlignmentPin],
    map: &PinSetupMap,
    pair_across: f64,
) -> Option<Vec<AlignmentPin>> {
    let [a, b] = pins else {
        return None;
    };
    if (a.diameter - b.diameter).abs() > PIN_CAD_ROUNDING_MM {
        return None;
    }
    let along_a = along_on_axis(map, a)?;
    let along_b = along_on_axis(map, b)?;
    axis_pair_pins(
        map,
        AxisPairPattern {
            axis_along: along_a,
            pair_along: along_b,
            pair_across,
            diameter: a.diameter,
        },
    )
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
    use rs_cam_core::compute::alignment_pins::fit_axis_plus_mirror_pair;
    use rs_cam_core::compute::transform::{FaceUp, ZRotation};

    fn bottom_map() -> PinSetupMap {
        PinSetupMap::new(FaceUp::Bottom, ZRotation::Deg0, 65.0, 65.0)
    }

    /// The mini test piece today: two 6 mm pins on the flip axis y = 32.5.
    /// The button keeps pin 1 and splits pin 2 into a mirror pair.
    #[test]
    fn the_pair_moves_off_the_axis_and_the_set_fits_the_preset() {
        let map = bottom_map();
        let pins = [
            AlignmentPin::new(59.0, 32.5, 6.0),
            AlignmentPin::new(6.0, 32.5, 6.0),
        ];
        let out = pair_off_axis(&pins, &map, 26.5).unwrap();
        assert_eq!(out.len(), 3);
        let fit = fit_axis_plus_mirror_pair(&out, &map).unwrap();
        assert!((fit.axis_along - 59.0).abs() < 1e-9, "{fit:?}");
        assert!((fit.pair_along - 6.0).abs() < 1e-9, "{fit:?}");
        assert!((fit.pair_across - 26.5).abs() < 1e-9, "{fit:?}");
    }

    /// Not two equal pins on the axis: the button does not apply.
    #[test]
    fn the_pair_button_refuses_a_set_off_the_axis() {
        let map = bottom_map();
        let off = [
            AlignmentPin::new(59.0, 32.5, 6.0),
            AlignmentPin::new(6.0, 10.0, 6.0),
        ];
        assert!(pair_off_axis(&off, &map, 20.0).is_none());
        let mixed = [
            AlignmentPin::new(59.0, 32.5, 6.0),
            AlignmentPin::new(6.0, 32.5, 8.0),
        ];
        assert!(pair_off_axis(&mixed, &map, 20.0).is_none());
    }

    /// A preset edit is the inverse of the fit: a drag of `pair_across`
    /// writes pins that read back with the new value.
    #[test]
    fn a_pattern_edit_reads_back_through_the_fit() {
        let map = bottom_map();
        let pattern = AxisPairPattern {
            axis_along: 59.0,
            pair_along: 6.0,
            pair_across: 26.5,
            diameter: 6.0,
        };
        let pins = axis_pair_pins(&map, pattern).unwrap();
        let fit = fit_axis_plus_mirror_pair(&pins, &map).unwrap();
        assert!((fit.pair_across - 26.5).abs() < 1e-9);
        assert!((axis_length(&map) - 65.0).abs() < 1e-9);
    }

    /// Snap takes a set a little off symmetric to exact symmetry.
    #[test]
    fn snap_makes_a_near_pair_exact() {
        let map = bottom_map();
        let pins = [
            AlignmentPin::new(59.0, 32.9, 6.0),
            AlignmentPin::new(6.0, 6.0, 6.0),
            AlignmentPin::new(6.4, 59.2, 6.0),
        ];
        let out = snap(&pins, &map);
        assert!((out[0].y - 32.5).abs() < 1e-9, "{out:?}");
        assert!((out[1].x - out[2].x).abs() < 1e-9, "{out:?}");
        assert!((out[1].y + out[2].y - 65.0).abs() < 1e-9, "{out:?}");
    }

    #[test]
    fn move_all_keeps_the_shape() {
        let pins = [
            AlignmentPin::new(1.0, 2.0, 6.0),
            AlignmentPin::new(3.0, 5.0, 6.0),
        ];
        let out = move_all(&pins, 0.5, -1.0);
        assert!((out[0].x - 1.5).abs() < 1e-12 && (out[1].y - 4.0).abs() < 1e-12);
        let (cx, cy) = set_centre(&out).unwrap();
        assert!((cx - 2.5).abs() < 1e-12 && (cy - 2.5).abs() < 1e-12);
    }
}
