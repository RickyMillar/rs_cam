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
//! 3. Keying was achievable and was not being done.
//!
//! P1 (2026-10-12) replaced the keying model. The 2026-08-22 model
//! judged four seatings of the stock RECTANGLE (identity, `My`, `Mx`,
//! `R180`). A part on spoilboard dowels has no edge reference: the holes
//! locate it. A wrong seating is a turn of the part that puts every hole
//! on a dowel, so two equal pins never key (a half turn about their
//! midpoint swaps them). The placer now places one pin on the mirror line
//! plus a mirror pair off it, and the tests below assert that model. The
//! intrinsic cases themselves are in
//! `alignment_pin_keying_intrinsic_p1.rs`. Review:
//! planning/alignment_pins_review_2026-10-12/REVIEW.md, C1.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::compute::alignment_pins::{
    AlignmentPin, PinPlacementError, PinPlacementRequest, PinSetupMap, PinSide, pin_flip_map,
    place_keyed_pins, validate_pins_for_flip,
};
use rs_cam_core::compute::stock_config::StockConfig;
use rs_cam_core::compute::transform::{FaceUp, ZRotation};
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
        z_rotation: ZRotation::Deg0,
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
    // No pin is anywhere near the old padding/2 output of 2.5/137.5.
    for p in &pins {
        assert!(
            p.x > 5.0 && p.x < W - 5.0,
            "pin still in the padding ring: {}",
            p.x
        );
    }
}

/// The old output, judged by the new bounds check: a Ø6 pin whose centre
/// is 2.5 mm from the edge is not wholly on the blank.
#[test]
fn the_old_padding_placement_reads_as_out_of_bounds() {
    let pins = vec![
        AlignmentPin::new(2.5, D * 0.5, PIN_D),
        AlignmentPin::new(W - 2.5, D * 0.5, PIN_D),
    ];
    let report = validate_pins_for_flip(&pins, FaceUp::Bottom, ZRotation::Deg0, W, D);
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
    let report = validate_pins_for_flip(&pins, FaceUp::Bottom, ZRotation::Deg0, W, D);

    assert!(!report.seats(), "{report:?}");
    assert!(report.seat_mismatch_mm.unwrap() > 100.0, "{report:?}");
    // And they are perfectly invariant under a half turn — the wrong
    // symmetry, which is why they looked plausible.
    let wrong = report.wrong_seating.expect("a half turn seats them");
    assert!(wrong.miss_mm < 1e-9, "{report:?}");
    assert!((wrong.turn_deg.abs() - 180.0).abs() < 1e-6, "{report:?}");
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
/// so the placer refuses rather than guessing. The placer also refuses a
/// turned `Bottom` setup (P2): its flip axis does not cross the X clear
/// strips, and the 2D placer is a later package (P6).
#[test]
fn refuses_every_flip_the_cam_does_not_model_in_plane() {
    let cases = [
        (FaceUp::Top, ZRotation::Deg0),
        (FaceUp::Front, ZRotation::Deg0),
        (FaceUp::Back, ZRotation::Deg0),
        (FaceUp::Left, ZRotation::Deg0),
        (FaceUp::Right, ZRotation::Deg0),
        (FaceUp::Bottom, ZRotation::Deg90),
        (FaceUp::Bottom, ZRotation::Deg180),
        (FaceUp::Bottom, ZRotation::Deg270),
    ];
    for (face, turn) in cases {
        let req = PinPlacementRequest {
            face_up: face,
            z_rotation: turn,
            ..request(Some((20.0, W - 20.0)), PIN_D)
        };
        assert!(
            matches!(
                place_keyed_pins(req),
                Err(PinPlacementError::UnsupportedFlip { .. })
            ),
            "expected a refusal for {face:?} {turn:?}"
        );
    }
}

/// The flip axis is derived from the setup's full map, never authored
/// (P3: the `FlipAxis` cache is gone). `Bottom` at 0 deg mirrors Y about
/// `y = D/2`; turned 180 deg it mirrors X about `x = W/2` (C3). A quarter
/// turn mirrors about a diagonal on a square stock and has no axis on a
/// stock that is not square, and the other faces do not mirror in plane.
#[test]
fn flip_axis_is_derived_from_the_setup_map() {
    let axis = |face, turn| PinSetupMap::new(face, turn, W, D).axis_segment();
    assert_eq!(
        axis(FaceUp::Bottom, ZRotation::Deg0),
        Some([(0.0, D / 2.0), (W, D / 2.0)])
    );
    assert_eq!(
        axis(FaceUp::Bottom, ZRotation::Deg180),
        Some([(W / 2.0, 0.0), (W / 2.0, D)])
    );
    assert!((W - D).abs() > 1.0, "the fixture stock must not be square");
    for turn in [ZRotation::Deg90, ZRotation::Deg270] {
        assert_eq!(axis(FaceUp::Bottom, turn), None, "{turn:?}");
    }
    for face in [
        FaceUp::Top,
        FaceUp::Front,
        FaceUp::Back,
        FaceUp::Left,
        FaceUp::Right,
    ] {
        assert_eq!(axis(face, ZRotation::Deg0), None, "{face:?}");
    }
    // The project selector takes the first in-plane flip, not the first
    // setup that is not Top: an edge-up setup has no axis in the pin plane.
    let map = pin_flip_map(
        [
            (FaceUp::Top, ZRotation::Deg0),
            (FaceUp::Front, ZRotation::Deg0),
            (FaceUp::Bottom, ZRotation::Deg180),
        ],
        W,
        D,
    )
    .expect("the Bottom setup is an in-plane flip");
    assert_eq!(map.z_rotation(), ZRotation::Deg180);
    assert_eq!(
        map.axis_label().as_deref(),
        Some(&*format!("x = {:.1} mm", W / 2.0))
    );
    assert!(pin_flip_map([(FaceUp::Top, ZRotation::Deg0)], W, D).is_none());
}

// ── Defect 3: the pair must key the flip ────────────────────────────────

/// What the placer produces has to satisfy both halves at once: every pin
/// maps to a pin under `My` (one pin on the mirror line, a mirror pair off
/// it), and no turn of the part on the dowels seats it (P1).
#[test]
fn placed_pins_seat_under_the_flip_and_key_it() {
    let pins = place_keyed_pins(request(Some((20.0, W - 20.0)), PIN_D)).unwrap();

    assert!(
        (pins[0].y - D * 0.5).abs() < 1e-9,
        "the axis pin is off the mirror line: {:?}",
        pins[0]
    );
    assert!(
        (pins[1].x - pins[2].x).abs() < 1e-9 && (pins[1].y + pins[2].y - D).abs() < 1e-9,
        "pins 2 and 3 are not a mirror pair: {pins:?}"
    );
    assert!(
        pins[1].y < pins[2].y,
        "the pair is not in Y order: {pins:?}"
    );

    let report = validate_pins_for_flip(&pins, FaceUp::Bottom, ZRotation::Deg0, W, D);
    assert!(report.seats(), "{report:?}");
    assert!(report.seat_mismatch_mm.unwrap() < 1e-9, "{report:?}");
    assert!(report.keyed(), "{report:?}");
    if let Some(wrong) = report.wrong_seating {
        assert!(wrong.miss_mm >= report.keying_bar_mm - 1e-9, "{report:?}");
    }
    assert!(report.out_of_bounds.is_empty(), "{report:?}");
}

/// The 2026-08-22 auto-place output on this blank: two pins on the mirror
/// line, slid 3 mm off centre symmetry. The rectangle model called it
/// keyed. It is not: a half turn about the midpoint of the two pins puts
/// each hole on the other dowel, and the part sits 3 mm off in X.
#[test]
fn the_old_offset_pair_does_not_key() {
    let pins = vec![
        AlignmentPin::new(11.5, D * 0.5, PIN_D),
        AlignmentPin::new(W - 8.5, D * 0.5, PIN_D),
    ];
    let report = validate_pins_for_flip(&pins, FaceUp::Bottom, ZRotation::Deg0, W, D);
    assert!(report.seats(), "{report:?}");
    assert!(!report.keyed(), "{report:?}");
    assert!(report.keying_margin_mm().unwrap() < 1e-9, "{report:?}");
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
    let report = validate_pins_for_flip(&pins, FaceUp::Bottom, ZRotation::Deg0, W, D);
    assert!(report.seats(), "{report:?}");
    assert!(!report.keyed(), "{report:?}");
    assert!(report.keying_margin_mm().unwrap() < 1e-9, "{report:?}");
    assert!(
        report
            .warnings()
            .iter()
            .any(|w| w.contains("do not key the flip")
                && w.contains("two pins of one diameter never key")),
        "{:?}",
        report.warnings()
    );
}

/// A third pin on the line through a symmetric pair adds no keying: the
/// three pins are in a line, and a half turn about the middle pin swaps
/// the outer two. (A third pin OFF that line, with a mirror partner, does
/// key: see `placed_pins_seat_under_the_flip_and_key_it`.)
#[test]
fn a_centre_pin_adds_no_keying() {
    let pins = vec![
        AlignmentPin::new(2.5, D * 0.5, PIN_D),
        AlignmentPin::new(W - 2.5, D * 0.5, PIN_D),
        AlignmentPin::new(W * 0.5, D * 0.5, PIN_D),
    ];
    let report = validate_pins_for_flip(&pins, FaceUp::Bottom, ZRotation::Deg0, W, D);
    assert!(report.seats(), "{report:?}");
    assert!(!report.keyed(), "a centre pin must not key: {report:?}");
}

/// ...and a fourth pin on the mirror line, at the stock centre, does not
/// undo the keying of the placed pattern on this blank: the four pins
/// still seat after the flip and in no turned position. (This is a
/// measured instance, not a rule: an added pin can make a new symmetry,
/// so every edit goes back through the check.)
#[test]
fn a_centre_pin_does_not_break_the_placed_pattern() {
    let placed = place_keyed_pins(request(Some((20.0, W - 20.0)), PIN_D)).unwrap();
    let mut pins = placed.to_vec();
    pins.push(AlignmentPin::new(W * 0.5, D * 0.5, PIN_D));
    let report = validate_pins_for_flip(&pins, FaceUp::Bottom, ZRotation::Deg0, W, D);
    assert!(report.seats(), "{report:?}");
    assert!(report.keyed(), "{report:?}");
}

/// The stock is too shallow in Y for a mirror pair: two Ø6 pins, a 2 mm
/// wall between them and a 2 mm wall to each edge need 2 x 6 + 3 x 2 =
/// 18 mm. A 17 mm deep blank refuses with that number.
#[test]
fn refuses_when_the_pair_does_not_fit() {
    let req = PinPlacementRequest {
        stock_d: 17.0,
        ..request(Some((20.0, W - 20.0)), PIN_D)
    };
    match place_keyed_pins(req) {
        Err(PinPlacementError::PairDoesNotFit {
            stock_d,
            required_mm,
            pin_diameter_mm,
        }) => {
            assert!((stock_d - 17.0).abs() < 1e-9);
            assert!((required_mm - 18.0).abs() < 1e-9, "required {required_mm}");
            assert!((pin_diameter_mm - PIN_D).abs() < 1e-9);
        }
        other => panic!("expected PairDoesNotFit, got {other:?}"),
    }
}

/// Fits, but only as one pattern, and that pattern seats turned. A 12 x 12
/// blank with no wall and Ø6 pins has 6 mm strips and room for exactly
/// one pair offset (h = 3). The pins (9, 6), (3, 3), (3, 9) make a
/// triangle near the equilateral shape: a turn of about 120 deg seats it
/// within about 0.5 mm, below the 3 mm bar. So the placer refuses, with
/// its own reason.
#[test]
fn refuses_when_no_pattern_that_fits_keys() {
    let req = PinPlacementRequest {
        stock_w: 12.0,
        stock_d: 12.0,
        model_x_range: None,
        face_up: FaceUp::Bottom,
        z_rotation: ZRotation::Deg0,
        pin_diameter: PIN_D,
        wall_mm: 0.0,
    };
    match place_keyed_pins(req) {
        Err(PinPlacementError::CannotKey {
            best_miss_mm,
            required_mm,
        }) => {
            assert!(
                best_miss_mm < required_mm,
                "{best_miss_mm} vs {required_mm}"
            );
            assert!(
                (required_mm - PIN_D * 0.5).abs() < 1e-9,
                "bar {required_mm}"
            );
        }
        other => panic!("expected CannotKey, got {other:?}"),
    }
}

/// Unequal strips: each pin sits at the centre of its strip, and the axis
/// pin goes in the +X strip on a tie. Both assignments give the same pair
/// offset here, so the tie rule decides.
#[test]
fn each_pin_sits_at_the_centre_of_its_strip() {
    // 40 mm on -X, 20 mm on +X.
    let pins = place_keyed_pins(request(Some((40.0, W - 20.0)), PIN_D)).unwrap();
    assert!((pins[0].x - (W - 10.0)).abs() < 1e-9, "{pins:?}");
    assert!((pins[1].x - 20.0).abs() < 1e-9, "{pins:?}");
    assert!((pins[2].x - 20.0).abs() < 1e-9, "{pins:?}");
    let report = validate_pins_for_flip(&pins, FaceUp::Bottom, ZRotation::Deg0, W, D);
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
        .plan_keyed_pins(FaceUp::Bottom, ZRotation::Deg0, Some(&bbox), PIN_D)
        .unwrap();
    let direct = place_keyed_pins(request(Some((20.0, W - 20.0)), PIN_D)).unwrap();
    for (p, q) in pins.iter().zip(&direct) {
        assert!(
            (p.x - q.x).abs() < 1e-9 && (p.y - q.y).abs() < 1e-9,
            "{pins:?}"
        );
    }

    // And the same stock read through `padding` — the pre-fix source —
    // refuses, which is the defect stated as a comparison.
    let padding_only = place_keyed_pins(request(Some((5.0, W - 5.0)), PIN_D));
    assert!(padding_only.is_err(), "{padding_only:?}");
}

/// The pin diameter is the dowel, and the dowel is whatever tool drills
/// it. A larger pin needs a wider strip, and the refusal has to move with
/// it — otherwise the wall check was computed for a pin that does not
/// exist. A strip holds a pin when it is at least the pin plus a 2 mm wall
/// each side; the 3-pin pattern needs no extra play in X to key.
#[test]
fn the_refusal_tracks_the_pin_diameter() {
    let strip = Some((20.0, W - 20.0));
    // Ø6 needs 10 mm of the 20 mm strip.
    assert!(place_keyed_pins(request(strip, 6.0)).is_ok());
    // Ø16 needs exactly 20 mm: it fits.
    assert!(place_keyed_pins(request(strip, 16.0)).is_ok());
    // Ø18 needs 22 mm and does not fit.
    assert!(matches!(
        place_keyed_pins(request(strip, 18.0)),
        Err(PinPlacementError::StripTooNarrow { .. })
    ));
}

/// Nothing to validate is not the same as validated clean: an empty pin
/// set says nothing, and must not manufacture a warning or a pass.
#[test]
fn an_empty_pin_set_reports_nothing() {
    let report = validate_pins_for_flip(&[], FaceUp::Bottom, ZRotation::Deg0, W, D);
    assert_eq!(report.pin_count, 0);
    assert!(!report.seats());
    assert!(!report.keyed());
    assert!(report.warnings().is_empty());
}

// ── CMP-27: the judgement must reach every surface, not just the GUI ────

/// A project with centre-symmetric pins — the live wanaka defect —
/// written as the loader reads it.
const PROJECT_WITH_UNKEYED_PINS: &str = r#"format_version = 3

[job]
name = "CMP-27 unkeyed pins"

[job.stock]
x = 140.0
y = 150.0
z = 25.0
origin_x = 0.0
origin_y = 0.0
origin_z = -25.0
auto_from_model = false

[[job.stock.alignment_pins]]
x = 2.5
y = 2.5
diameter = 6.0

[[job.stock.alignment_pins]]
x = 137.5
y = 147.5
diameter = 6.0

[[setups]]
id = 0
name = "Back"
face_up = "bottom"
"#;

/// CMP-27, the load half. `validate_pins_for_flip`'s own doc says "Call
/// this on load and publish `PinFlipReport::warnings`", and three
/// production call sites did — all three in `rs_cam_viz`. The core load
/// path never called it, so the CLI's headless load and MCP
/// `load_project` heard nothing.
///
/// The consequence is registration: a pin pair invariant under the wrong
/// symmetry seats in the flipped orientation and in the un-flipped one,
/// and both look right.
#[test]
fn a_loaded_project_warns_about_pins_that_do_not_key_g_pinauto() {
    let dir = std::env::temp_dir().join(format!("rs_cam_cmp27_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    let path = dir.join("unkeyed_pins.toml");
    std::fs::write(&path, PROJECT_WITH_UNKEYED_PINS).expect("write project");

    let (session, warnings) = rs_cam_core::session::ProjectSession::load_with_warnings(&path)
        .expect("the project loads; an unkeyed pin pair is a warning, not a refusal");

    let messages: Vec<String> = warnings
        .iter()
        .map(rs_cam_core::session::ProjectLoadWarning::message)
        .collect();
    assert!(
        messages
            .iter()
            .any(|m| m.contains("do not survive the flip")),
        "the core loader took an unkeyed pin pair in silence. Warnings were: {messages:?}"
    );

    // The message is the report's own sentence, verbatim. The GUI runs the
    // same check on its own load path and dedupes by exact message, so a
    // re-worded copy here would print the fact twice.
    let report = session.stock_config().validate_pins_for_flip(
        session.list_setups()[0].face_up,
        session.list_setups()[0].z_rotation,
    );
    for line in report.warnings() {
        assert!(
            messages.contains(&line),
            "the loader re-worded `{line}`. Warnings were: {messages:?}"
        );
    }
}

/// CMP-27, the diagnostics half. The same judgement is a `Diagnostic`, so
/// `get_project_diagnostics` answers it for MCP and the CLI, and
/// `add_alignment_pin` / `remove_alignment_pin` get a current answer
/// rather than one frozen at load time.
#[test]
fn project_diagnostics_publish_the_pin_keying_verdict_g_pinauto() {
    let mut stock = StockConfig {
        x: W,
        y: D,
        z: 25.0,
        origin_z: -25.0,
        auto_from_model: false,
        ..StockConfig::default()
    };
    stock.alignment_pins = vec![
        AlignmentPin::new(2.5, 2.5, PIN_D),
        AlignmentPin::new(137.5, 147.5, PIN_D),
    ];

    let mut session = rs_cam_core::session::ProjectSessionBuilder::new()
        .stock(stock)
        .build();
    let _ = session
        .apply(rs_cam_core::session::Command::SetSetupFace(
            rs_cam_core::session::SetSetupFaceArgs {
                setup_index: 0,
                face_up: FaceUp::Bottom,
            },
        ))
        .expect("the default setup takes a face");

    let diag = session.diagnostics();
    let pin_verdicts: Vec<&str> = diag
        .verdicts
        .iter()
        .filter(|v| v.kind == rs_cam_core::session::VerdictKind::AlignmentPinsUnkeyed)
        .map(|v| v.headline.as_str())
        .collect();
    assert!(
        !pin_verdicts.is_empty(),
        "no pin-keying verdict; the verdicts were {:?}",
        diag.verdicts.iter().map(|v| v.kind).collect::<Vec<_>>()
    );
    // The live wanaka pair says two things: the bodies hang off the blank,
    // and the pattern is invariant under R180 rather than the flip. Both
    // are the report's own sentences, carried verbatim.
    assert!(
        pin_verdicts.iter().any(|h| h.contains("not wholly inside")),
        "{pin_verdicts:?}"
    );
    assert!(
        pin_verdicts
            .iter()
            .any(|h| h.contains("do not survive the flip")),
        "{pin_verdicts:?}"
    );

    let unified = session.diagnose_project();
    assert!(
        unified.iter().any(|d| d.id.as_str()
            == rs_cam_core::diagnostics::ids::PROJECT_ALIGNMENT_PINS_UNKEYED),
        "the verdict did not reach the unified diagnostic model, which is what \
         MCP `get_project_diagnostics` reads"
    );
}

/// The control. Pins that DO key the flip raise nothing, on either half.
/// The placer's 3-pin pattern is the keyed set (P1).
#[test]
fn keyed_pins_raise_no_diagnostic_g_pinauto() {
    let pins = place_keyed_pins(request(Some((20.0, W - 20.0)), PIN_D)).expect("placement");
    let mut stock = StockConfig {
        x: W,
        y: D,
        z: 25.0,
        origin_z: -25.0,
        auto_from_model: false,
        ..StockConfig::default()
    };
    stock.alignment_pins = pins.to_vec();

    let mut session = rs_cam_core::session::ProjectSessionBuilder::new()
        .stock(stock)
        .build();
    let _ = session
        .apply(rs_cam_core::session::Command::SetSetupFace(
            rs_cam_core::session::SetSetupFaceArgs {
                setup_index: 0,
                face_up: FaceUp::Bottom,
            },
        ))
        .expect("the default setup takes a face");

    assert!(
        !session
            .diagnostics()
            .verdicts
            .iter()
            .any(|v| v.kind == rs_cam_core::session::VerdictKind::AlignmentPinsUnkeyed),
        "a keyed pin pair must raise nothing, or the verdict is noise"
    );
}
