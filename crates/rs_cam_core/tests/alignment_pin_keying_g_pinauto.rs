//! G-PINAUTO — automatic alignment-pin placement must key the flip, and
//! must refuse when it cannot.
//!
//! Raised at the machine 2026-08-22, before the first real cut, from the
//! operator's actual project. Three defects in one feature:
//!
//! 1. The margin ignored the pin and read the wrong source. Placement was
//!    `margin = padding / 2`, so `padding = 5` with a Ø6 dowel put the
//!    hole centre 2.5 mm from the edge — the hole spanning −0.5..5.5,
//!    hanging off the blank. A 5 mm ring cannot hold a 6 mm pin at ANY
//!    offset, so the only correct output is a refusal. And `padding` was
//!    the wrong source anyway: the blank was 140×150 by hand around a
//!    100×100 model, a 20 mm clear strip, while `padding` still read 5.
//!
//! 2. Nothing checked that the pins survive the flip. `FaceUp::Bottom` is
//!    `(x, D − y, H − z)` — X preserved, Y mirrored about `y = D/2`. The
//!    stored pins were diagonally opposite, which is invariant under a
//!    180° ROTATION, not under a mirror. Neither hole would have landed
//!    on a dowel; the part could not have re-seated. Nothing warned.
//!
//! 3. Keying was achievable and was not being done. A rectangular blank
//!    re-seats four ways — identity, `My` (`y -> D − y`), `Mx`
//!    (`x -> W − x`) and `R180 = Mx ∘ My`. The CAM models exactly `My`,
//!    so the requirement is: invariant under `My`, and under NOTHING
//!    else. Both pins on the mirror line makes the flip seat; an
//!    x-multiset that fails `x -> W − x` blocks the other two.
//!
//! Full geometric argument: planning/airrun_2026-08-19/RUN_LOG.md,
//! "## G-PINAUTO".

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::compute::stock_config::{
    AlignmentPin, FlipAxis, PIN_KEYING_ASYMMETRY_MM, PinPlacementError, PinPlacementRequest,
    PinSide, StockConfig, place_keyed_pins, validate_pins_for_flip,
};
use rs_cam_core::compute::transform::FaceUp;
use rs_cam_core::geo::{BoundingBox3, P3};

/// The live blank: 140×150, a 100×100 model centred in it, Ø6 dowels.
const W: f64 = 140.0;
const D: f64 = 150.0;
const PIN_D: f64 = 6.0;

fn request(model_x_range: Option<(f64, f64)>, pin_diameter: f64) -> PinPlacementRequest {
    PinPlacementRequest {
        stock_w: W,
        stock_d: D,
        model_x_range,
        face_up: FaceUp::Bottom,
        pin_diameter,
        wall_mm: 2.0,
    }
}

// ── Defect 1: the margin must consult the pin, and read the model ───────

/// A 5 mm padding ring cannot hold a 6 mm dowel at any offset. The old
/// code emitted `(2.5, y/2)` regardless; the pin body then spanned
/// −0.5..5.5 mm relative to the stock edge.
#[test]
fn refuses_a_pin_wider_than_the_clear_strip() {
    let padding_strip = Some((5.0, W - 5.0));
    match place_keyed_pins(request(padding_strip, PIN_D)) {
        Err(PinPlacementError::StripTooNarrow {
            side,
            strip_mm,
            required_mm,
            pin_diameter_mm,
        }) => {
            assert_eq!(side, PinSide::MinusX);
            assert!((strip_mm - 5.0).abs() < 1e-9, "strip {strip_mm}");
            // 6 mm of pin + 2 mm of wall each side.
            assert!((required_mm - 10.0).abs() < 1e-9, "required {required_mm}");
            assert!((pin_diameter_mm - PIN_D).abs() < 1e-9);
        }
        other => panic!("expected StripTooNarrow, got {other:?}"),
    }
}

/// The same blank, with the strip read from the MODEL BBOX instead of
/// `padding`: 20 mm of clear material either side, and a placement
/// exists. This is the whole of defect 1 — the room was always there,
/// the placer was looking at the wrong number.
#[test]
fn margin_comes_from_the_model_bbox_not_padding() {
    let model_strip = Some((20.0, W - 20.0));
    let pins = place_keyed_pins(request(model_strip, PIN_D)).unwrap();

    for p in &pins {
        let r = p.diameter * 0.5;
        assert!(
            p.x - r >= 2.0 - 1e-9,
            "pin at {} breaks the stock wall",
            p.x
        );
        assert!(
            p.x + r <= W - 2.0 + 1e-9,
            "pin at {} breaks the stock wall",
            p.x
        );
    }
    // Neither pin is anywhere near the old padding/2 output of 2.5/137.5.
    assert!(
        pins[0].x > 5.0,
        "pin still in the padding ring: {}",
        pins[0].x
    );
    assert!(
        pins[1].x < W - 5.0,
        "pin still in the padding ring: {}",
        pins[1].x
    );
}

/// The old output, judged by the new bounds check: a Ø6 pin whose centre
/// is 2.5 mm from the edge is not wholly on the blank.
#[test]
fn the_old_padding_placement_reads_as_out_of_bounds() {
    let pins = vec![
        AlignmentPin::new(2.5, D * 0.5, PIN_D),
        AlignmentPin::new(W - 2.5, D * 0.5, PIN_D),
    ];
    let report = validate_pins_for_flip(&pins, FaceUp::Bottom, W, D);
    assert_eq!(report.out_of_bounds, vec![0, 1], "{report:?}");
    assert!(
        report
            .warnings()
            .iter()
            .any(|w| w.contains("inside the stock")),
        "{:?}",
        report.warnings()
    );
}

// ── Defect 2: pins must survive the flip ────────────────────────────────

/// The stored pins from the live project: diagonally opposite, i.e.
/// invariant under a 180° rotation. Under `y -> 150 − y` they map to
/// (2.5, 147.5) and (137.5, 2.5) — neither is a hole. The part could not
/// have re-seated, and the pre-fix GUI said nothing.
#[test]
fn centre_symmetric_pins_do_not_survive_the_flip() {
    let pins = vec![
        AlignmentPin::new(2.5, 2.5, PIN_D),
        AlignmentPin::new(137.5, 147.5, PIN_D),
    ];
    let report = validate_pins_for_flip(&pins, FaceUp::Bottom, W, D);

    assert!(!report.seats(), "{report:?}");
    assert!(report.seat_mismatch_mm > 100.0, "{report:?}");
    // And they are perfectly invariant under R180 — the wrong symmetry,
    // which is why they looked plausible.
    assert!(report.rot180_mismatch_mm < 1e-9, "{report:?}");
    assert!(!report.keyed(), "{report:?}");
    assert!(
        report
            .warnings()
            .iter()
            .any(|w| w.contains("do not survive the flip")),
        "{:?}",
        report.warnings()
    );
}

/// Only `Bottom` keeps the part's XY footprint. Standing the blank on an
/// edge changes which stock dimensions the pins would have to register,
/// so the placer refuses rather than guessing.
#[test]
fn refuses_every_flip_the_cam_does_not_model_in_plane() {
    for face in [
        FaceUp::Top,
        FaceUp::Front,
        FaceUp::Back,
        FaceUp::Left,
        FaceUp::Right,
    ] {
        let req = PinPlacementRequest {
            face_up: face,
            ..request(Some((20.0, W - 20.0)), PIN_D)
        };
        assert!(
            matches!(
                place_keyed_pins(req),
                Err(PinPlacementError::UnsupportedFlip { .. })
            ),
            "expected a refusal for {face:?}"
        );
    }
}

/// `FlipAxis` is derived from the setup, never authored. `Vertical` is
/// unreachable because no `FaceUp` performs an X mirror.
#[test]
fn flip_axis_is_derived_from_face_up() {
    assert_eq!(
        FlipAxis::from_face_up(FaceUp::Bottom),
        Some(FlipAxis::Horizontal)
    );
    for face in [
        FaceUp::Top,
        FaceUp::Front,
        FaceUp::Back,
        FaceUp::Left,
        FaceUp::Right,
    ] {
        assert_eq!(FlipAxis::from_face_up(face), None, "{face:?}");
    }
}

// ── Defect 3: the pair must key the flip ────────────────────────────────

/// What the placer produces has to satisfy both halves at once: every pin
/// maps to itself under `My`, and the x-multiset misses `x -> W − x` by
/// at least the keying asymmetry.
#[test]
fn placed_pins_seat_under_my_and_block_mx_and_r180() {
    let pins = place_keyed_pins(request(Some((20.0, W - 20.0)), PIN_D)).unwrap();

    for p in &pins {
        assert!(
            (p.y - D * 0.5).abs() < 1e-9,
            "pin off the mirror line: {p:?}"
        );
    }

    let report = validate_pins_for_flip(&pins, FaceUp::Bottom, W, D);
    assert!(report.seats(), "{report:?}");
    assert!(report.seat_mismatch_mm < 1e-9, "{report:?}");
    assert!(report.keyed(), "{report:?}");
    assert!(
        report.mirror_x_mismatch_mm >= PIN_KEYING_ASYMMETRY_MM - 1e-9,
        "{report:?}"
    );
    assert!(
        report.rot180_mismatch_mm >= PIN_KEYING_ASYMMETRY_MM - 1e-9,
        "{report:?}"
    );
    // Stated as the invariant that actually does the keying.
    let sum = pins[0].x + pins[1].x;
    assert!(
        (sum - W).abs() >= PIN_KEYING_ASYMMETRY_MM - 1e-9,
        "x-multiset is centre-symmetric: {} + {} vs W {W}",
        pins[0].x,
        pins[1].x
    );
}

/// The pre-fix auto output, judged by the new check: both pins on the
/// mirror line at `margin` and `W − margin`. It SEATS — the old code was
/// right about that much — and it does not key at all, so the operator
/// can drop the part on 180° out and it fits perfectly.
#[test]
fn symmetric_pair_seats_but_does_not_key() {
    let pins = vec![
        AlignmentPin::new(2.5, D * 0.5, PIN_D),
        AlignmentPin::new(W - 2.5, D * 0.5, PIN_D),
    ];
    let report = validate_pins_for_flip(&pins, FaceUp::Bottom, W, D);
    assert!(report.seats(), "{report:?}");
    assert!(!report.keyed(), "{report:?}");
    assert!(report.keying_margin_mm() < 1e-9, "{report:?}");
    assert!(
        report
            .warnings()
            .iter()
            .any(|w| w.contains("do not key the flip")),
        "{:?}",
        report.warnings()
    );
}

/// An ODD pin count buys nothing by itself: a pin at `x = W/2` is its own
/// image under `x -> W − x`, so adding one to a centre-symmetric pair
/// leaves the pattern seating all four ways.
#[test]
fn a_centre_pin_adds_no_keying() {
    let pins = vec![
        AlignmentPin::new(2.5, D * 0.5, PIN_D),
        AlignmentPin::new(W - 2.5, D * 0.5, PIN_D),
        AlignmentPin::new(W * 0.5, D * 0.5, PIN_D),
    ];
    let report = validate_pins_for_flip(&pins, FaceUp::Bottom, W, D);
    assert!(report.seats(), "{report:?}");
    assert!(!report.keyed(), "a centre pin must not key: {report:?}");
}

/// ...and conversely, adding a centre pin to a keyed pair does not
/// UNDO the keying, because a wrong seating needs EVERY pin to find a
/// hole and two of them still do not. Redundancy against rocking is the
/// only thing the third pin buys.
#[test]
fn a_centre_pin_does_not_break_an_already_keyed_pair() {
    let pair = place_keyed_pins(request(Some((20.0, W - 20.0)), PIN_D)).unwrap();
    let mut pins = pair.to_vec();
    pins.push(AlignmentPin::new(W * 0.5, D * 0.5, PIN_D));
    let report = validate_pins_for_flip(&pins, FaceUp::Bottom, W, D);
    assert!(report.seats(), "{report:?}");
    assert!(report.keyed(), "{report:?}");
}

/// Fits, but with no play left to break centre-symmetry: two strips of
/// exactly `pin + 2 × wall`. Both pins are placeable and the pair would
/// seat four ways, so this is a refusal too — a different one, with its
/// own reason.
#[test]
fn refuses_when_there_is_no_room_left_to_key() {
    // 10 mm strips = 6 mm pin + 2 mm wall each side, zero slack.
    match place_keyed_pins(request(Some((10.0, W - 10.0)), PIN_D)) {
        Err(PinPlacementError::CannotKey {
            available_mm,
            required_mm,
        }) => {
            assert!(available_mm.abs() < 1e-9, "available {available_mm}");
            assert!((required_mm - PIN_KEYING_ASYMMETRY_MM).abs() < 1e-9);
        }
        other => panic!("expected CannotKey, got {other:?}"),
    }
}

/// Unequal strips key the pair for free — the placer must notice and not
/// spend clearance it does not need to spend.
#[test]
fn unequal_strips_are_already_keyed() {
    // 40 mm on -X, 20 mm on +X: the centred pair is 10 mm asymmetric.
    let pins = place_keyed_pins(request(Some((40.0, W - 20.0)), PIN_D)).unwrap();
    assert!((pins[0].x - 20.0).abs() < 1e-9, "{pins:?}");
    assert!((pins[1].x - (W - 10.0)).abs() < 1e-9, "{pins:?}");
    let report = validate_pins_for_flip(&pins, FaceUp::Bottom, W, D);
    assert!(report.seats(), "{report:?}");
    assert!(report.keyed(), "{report:?}");
}

// ── Frame: pins are stock-local, model bboxes are world ─────────────────

/// `StockConfig::plan_keyed_pins` takes a WORLD model bbox and produces
/// stock-local pins, because that is the frame `alignment_pins` are
/// dimensioned in (see `pindrill_emission_frame_g_pindrill.rs`). A blank
/// with a non-zero origin must not shift the answer.
#[test]
fn plan_keyed_pins_converts_the_model_bbox_into_the_pin_frame() {
    let stock = StockConfig {
        x: W,
        y: D,
        z: 25.0,
        origin_x: -20.0,
        origin_y: -25.0,
        origin_z: -25.0,
        auto_from_model: false,
        padding: 5.0,
        ..StockConfig::default()
    };
    // 100x100 model sitting at world 0..100, i.e. stock-local 20..120.
    let bbox = BoundingBox3 {
        min: P3::new(0.0, 0.0, 0.0),
        max: P3::new(100.0, 100.0, 10.0),
    };
    let pins = stock
        .plan_keyed_pins(FaceUp::Bottom, Some(&bbox), PIN_D)
        .unwrap();
    let direct = place_keyed_pins(request(Some((20.0, W - 20.0)), PIN_D)).unwrap();
    assert!((pins[0].x - direct[0].x).abs() < 1e-9, "{pins:?}");
    assert!((pins[1].x - direct[1].x).abs() < 1e-9, "{pins:?}");

    // And the same stock read through `padding` — the pre-fix source —
    // refuses, which is the defect stated as a comparison.
    let padding_only = place_keyed_pins(request(Some((5.0, W - 5.0)), PIN_D));
    assert!(padding_only.is_err(), "{padding_only:?}");
}

/// The pin diameter is the dowel, and the dowel is whatever tool drills
/// it. A larger pin needs a wider strip, and the refusal has to move with
/// it — otherwise the wall check was computed for a pin that does not
/// exist. (The hardcoded `6.0` this replaces made that impossible.)
#[test]
fn the_refusal_tracks_the_pin_diameter() {
    let strip = Some((20.0, W - 20.0));
    // Ø6 fits a 20 mm strip with room to key.
    assert!(place_keyed_pins(request(strip, 6.0)).is_ok());
    // Ø12 needs 16 mm and leaves 4 mm of play — still keyable.
    assert!(place_keyed_pins(request(strip, 12.0)).is_ok());
    // Ø14 needs 18 mm and leaves 2 mm; not enough to key.
    assert!(matches!(
        place_keyed_pins(request(strip, 14.0)),
        Err(PinPlacementError::CannotKey { .. })
    ));
    // Ø18 does not fit at all.
    assert!(matches!(
        place_keyed_pins(request(strip, 18.0)),
        Err(PinPlacementError::StripTooNarrow { .. })
    ));
}

/// Nothing to validate is not the same as validated clean: an empty pin
/// set says nothing, and must not manufacture a warning or a pass.
#[test]
fn an_empty_pin_set_reports_nothing() {
    let report = validate_pins_for_flip(&[], FaceUp::Bottom, W, D);
    assert_eq!(report.pin_count, 0);
    assert!(!report.seats());
    assert!(!report.keyed());
    assert!(report.warnings().is_empty());
}
