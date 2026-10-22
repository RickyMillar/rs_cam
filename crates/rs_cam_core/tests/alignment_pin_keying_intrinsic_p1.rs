//! P1 — the intrinsic keying check (2026-10-12).
//!
//! A part on spoilboard dowels has no edge reference: the holes locate
//! it. A wrong seating is a turn of the part that puts every hole on a
//! dowel, i.e. a rotation symmetry of the hole pattern. The 2026-08-22
//! model judged four seatings of the stock rectangle instead, and it
//! called three of the five patterns below keyed when they are not.
//!
//! The five cases are the C1 table of
//! planning/alignment_pins_review_2026-10-12/REVIEW.md, on the 65 x 65
//! mini test piece with Ø6 dowels. The bar is the hole radius (3 mm).

#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_core::compute::alignment_pins::{
    AlignmentPin, closest_wrong_seating, keying_bar_mm, validate_pins_for_flip,
};
use rs_cam_core::compute::transform::{FaceUp, ZRotation};

const W: f64 = 65.0;
const D: f64 = 65.0;
const PIN_D: f64 = 6.0;

fn pins(xy: &[(f64, f64)]) -> Vec<AlignmentPin> {
    xy.iter()
        .map(|&(x, y)| AlignmentPin::new(x, y, PIN_D))
        .collect()
}

fn report(p: &[AlignmentPin]) -> rs_cam_core::compute::alignment_pins::PinFlipReport {
    validate_pins_for_flip(p, FaceUp::Bottom, ZRotation::Deg0, W, D)
}

/// C1 row 1, the live mini piece: two equal pins on the mirror line. A
/// half turn about their midpoint swaps them.
#[test]
fn two_equal_pins_on_the_axis_do_not_key() {
    let r = report(&pins(&[(6.0, 32.5), (59.0, 32.5)]));
    assert!(r.seats(), "{r:?}");
    assert!(!r.keyed(), "{r:?}");
    let wrong = r.wrong_seating.expect("a half turn seats them");
    assert!(wrong.miss_mm < 1e-9, "{r:?}");
    assert!((wrong.turn_deg.abs() - 180.0).abs() < 1e-6, "{r:?}");
    assert!(
        r.warnings()
            .iter()
            .any(|w| w.contains("two pins of one diameter never key")),
        "{:?}",
        r.warnings()
    );
}

/// C1 row 2, the 2026-08-22 auto-place style: the pair slid 3 mm off
/// centre symmetry. The rectangle model said keyed (margin 3.0). The part
/// still seats after a half turn about the pair's midpoint; it only sits
/// 3 mm off in X.
#[test]
fn the_offset_auto_place_pair_does_not_key() {
    let r = report(&pins(&[(7.5, 32.5), (60.5, 32.5)]));
    assert!(r.seats(), "{r:?}");
    assert!(!r.keyed(), "{r:?}");
    assert!(r.keying_margin_mm().unwrap() < 1e-9, "{r:?}");
}

/// C1 row 3: one pin on the mirror line plus a mirror pair off it. It
/// seats after the flip and keys.
///
/// The margin is the refined best fit (turn plus shift). The plain turn
/// about the centroid reads 6.9 mm here (the REVIEW number); the refined
/// fit finds a closer wrong seating near 4.8 mm. Both clear the 3 mm bar.
#[test]
fn an_axis_pin_plus_a_mirror_pair_keys() {
    let r = report(&pins(&[(55.0, 32.5), (8.0, 10.0), (8.0, 55.0)]));
    assert!(r.seats(), "{r:?}");
    assert!(r.seat_mismatch_mm.unwrap() < 1e-9, "{r:?}");
    assert!(r.keyed(), "{r:?}");
    let margin = r.keying_margin_mm().expect("a candidate turn exists");
    assert!(
        margin >= r.keying_bar_mm && margin < 6.9,
        "margin {margin}: {r:?}"
    );
    assert!(r.warnings().is_empty(), "{:?}", r.warnings());
}

/// C1 row 4: three pins in a line, the middle one on the axis. A half
/// turn about the middle pin swaps the outer two.
#[test]
fn three_collinear_pins_do_not_key() {
    let r = report(&pins(&[(8.0, 10.0), (8.0, 32.5), (8.0, 55.0)]));
    assert!(r.seats(), "{r:?}");
    assert!(!r.keyed(), "{r:?}");
    assert!(r.keying_margin_mm().unwrap() < 1e-9, "{r:?}");
}

/// C1 row 5: an axis pin plus a mirror pair that form an equilateral
/// triangle. A 120 deg turn about the centre seats it.
#[test]
fn an_equilateral_axis_plus_pair_does_not_key() {
    let apex_x = 8.0 + 22.5 * 3.0_f64.sqrt();
    let r = report(&pins(&[(apex_x, 32.5), (8.0, 10.0), (8.0, 55.0)]));
    assert!(r.seats(), "{r:?}");
    assert!(!r.keyed(), "{r:?}");
    let wrong = r.wrong_seating.expect("a 120 deg turn seats it");
    assert!(wrong.miss_mm < 1e-6, "{r:?}");
    assert!((wrong.turn_deg.abs() - 120.0).abs() < 1e-6, "{r:?}");
}

/// Two pins of different diameters key: after a half turn the 8 mm dowel
/// meets the 6 mm hole, and a bigger dowel does not enter a smaller hole.
/// That candidate is blocked, so no wrong seating exists.
#[test]
fn a_six_and_an_eight_mm_pair_keys() {
    let p = vec![
        AlignmentPin::new(6.0, 32.5, 6.0),
        AlignmentPin::new(59.0, 32.5, 8.0),
    ];
    let r = report(&p);
    assert!(r.seats(), "{r:?}");
    assert!(r.keyed(), "{r:?}");
    assert_eq!(r.wrong_seating, None, "{r:?}");
    // The same answer whichever pin is listed first.
    let swapped = vec![p[1].clone(), p[0].clone()];
    assert_eq!(closest_wrong_seating(&swapped), None);
}

/// Four pins at the corners of a rectangle: a half turn about the centre
/// maps the set onto itself.
#[test]
fn a_four_pin_rectangle_does_not_key() {
    let r = report(&pins(&[
        (10.0, 10.0),
        (55.0, 10.0),
        (10.0, 55.0),
        (55.0, 55.0),
    ]));
    assert!(r.seats(), "{r:?}");
    assert!(!r.keyed(), "{r:?}");
    assert!(r.keying_margin_mm().unwrap() < 1e-9, "{r:?}");
}

/// One pin never keys: the part turns about it.
#[test]
fn one_pin_does_not_key() {
    let r = report(&pins(&[(32.5, 32.5)]));
    assert!(r.seats(), "{r:?}");
    assert!(!r.keyed(), "{r:?}");
    assert!(
        r.warnings()
            .iter()
            .any(|w| w.contains("one pin never keys")),
        "{:?}",
        r.warnings()
    );
}

/// The bar is the largest hole radius in the pattern: a dowel whose
/// centre lands on solid wood, outside the hole edge, cannot be forced in.
#[test]
fn the_bar_is_the_largest_hole_radius() {
    assert!((keying_bar_mm(&pins(&[(0.0, 0.0), (1.0, 1.0)])) - 3.0).abs() < 1e-12);
    let mixed = vec![
        AlignmentPin::new(0.0, 0.0, 6.0),
        AlignmentPin::new(10.0, 0.0, 8.0),
    ];
    assert!((keying_bar_mm(&mixed) - 4.0).abs() < 1e-12);
    assert!(keying_bar_mm(&[]).abs() < 1e-12);
}
