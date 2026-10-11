//! P2 — the pin check judges the setup's full in-plane map (2026-10-12).
//!
//! The setup transform is `FaceUp` then `ZRotation`. `Bottom` + `Deg180`
//! maps `(x, y)` to `(W - x, y)`: the X mirror. The check used to read
//! `face_up` only, so it judged the Y mirror for that setup. `Bottom` +
//! `Deg90`/`Deg270` changes the footprint on a stock that is not square;
//! the check says so instead of judging it. Review:
//! planning/alignment_pins_review_2026-10-12/REVIEW.md, C3.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::compute::alignment_pins::{
    AlignmentPin, PinFlipKind, PinSetupMap, validate_pins_for_flip,
};
use rs_cam_core::compute::stock_config::StockConfig;
use rs_cam_core::compute::transform::{FaceUp, ZRotation};
use rs_cam_core::session::{
    Command, ProjectSessionBuilder, SetSetupFaceArgs, SetSetupRotationArgs, VerdictKind,
};

/// A stock that is not square, so a quarter turn changes the footprint.
const W: f64 = 100.0;
const D: f64 = 65.0;
const PIN_D: f64 = 6.0;

/// Keyed under the Y mirror (`Bottom` at 0 deg): an axis pin on y = D/2
/// and a pair mirrored across it.
fn y_mirror_pins() -> Vec<AlignmentPin> {
    vec![
        AlignmentPin::new(85.0, 32.5, PIN_D),
        AlignmentPin::new(15.0, 10.0, PIN_D),
        AlignmentPin::new(15.0, 55.0, PIN_D),
    ]
}

/// Keyed under the X mirror (`Bottom` turned 180 deg): an axis pin on
/// x = W/2 and a pair mirrored across it.
fn x_mirror_pins() -> Vec<AlignmentPin> {
    vec![
        AlignmentPin::new(50.0, 55.0, PIN_D),
        AlignmentPin::new(15.0, 10.0, PIN_D),
        AlignmentPin::new(85.0, 10.0, PIN_D),
    ]
}

/// The map comes from the setup transform: `Bottom` at 0 deg mirrors Y,
/// turned 180 deg it mirrors X, turned 90 deg on a square stock it swaps
/// X and Y.
#[test]
fn the_setup_map_is_the_setup_transform() {
    let (x, y) = PinSetupMap::new(FaceUp::Bottom, ZRotation::Deg0, W, D).map_xy(10.0, 20.0);
    assert!(
        (x - 10.0).abs() < 1e-9 && (y - (D - 20.0)).abs() < 1e-9,
        "({x}, {y})"
    );

    let (x, y) = PinSetupMap::new(FaceUp::Bottom, ZRotation::Deg180, W, D).map_xy(10.0, 20.0);
    assert!(
        (x - (W - 10.0)).abs() < 1e-9 && (y - 20.0).abs() < 1e-9,
        "({x}, {y})"
    );

    let square = PinSetupMap::new(FaceUp::Bottom, ZRotation::Deg90, 65.0, 65.0);
    assert_eq!(square.kind(), PinFlipKind::InPlane);
    let (x, y) = square.map_xy(10.0, 20.0);
    assert!(
        (x - 20.0).abs() < 1e-9 && (y - 10.0).abs() < 1e-9,
        "({x}, {y})"
    );

    assert_eq!(
        PinSetupMap::new(FaceUp::Bottom, ZRotation::Deg270, W, D).kind(),
        PinFlipKind::QuarterTurnNotSquare
    );
    assert_eq!(
        PinSetupMap::new(FaceUp::Top, ZRotation::Deg180, W, D).kind(),
        PinFlipKind::NotAFlip
    );
    assert_eq!(
        PinSetupMap::new(FaceUp::Front, ZRotation::Deg0, W, D).kind(),
        PinFlipKind::EdgeUp
    );
}

/// The C3 sentry: a `Bottom` setup turned 180 deg is judged against the
/// X mirror. Y-mirror pins do not seat there; X-mirror pins do.
#[test]
fn bottom_turned_180_is_judged_against_the_x_mirror() {
    let r = validate_pins_for_flip(&y_mirror_pins(), FaceUp::Bottom, ZRotation::Deg180, W, D);
    assert!(r.modelled_flip(), "{r:?}");
    assert!(!r.seats(), "Y-mirror pins seat under the X mirror: {r:?}");
    assert!(
        r.warnings()
            .iter()
            .any(|w| w.contains("do not survive the flip") && w.contains("turned 180 deg")),
        "{:?}",
        r.warnings()
    );

    let r = validate_pins_for_flip(&x_mirror_pins(), FaceUp::Bottom, ZRotation::Deg180, W, D);
    assert!(r.seats(), "{r:?}");
    assert!(r.keyed(), "{r:?}");
    assert!(r.warnings().is_empty(), "{:?}", r.warnings());

    // The control: at 0 deg the verdicts swap.
    let r = validate_pins_for_flip(&y_mirror_pins(), FaceUp::Bottom, ZRotation::Deg0, W, D);
    assert!(r.seats() && r.keyed(), "{r:?}");
    let r = validate_pins_for_flip(&x_mirror_pins(), FaceUp::Bottom, ZRotation::Deg0, W, D);
    assert!(!r.seats(), "{r:?}");
}

/// A quarter turn on a stock that is not square is refused as "not
/// modelled": one warning, and no seat or keying verdict.
#[test]
fn a_quarter_turn_on_a_non_square_stock_is_not_modelled() {
    for turn in [ZRotation::Deg90, ZRotation::Deg270] {
        let r = validate_pins_for_flip(&y_mirror_pins(), FaceUp::Bottom, turn, W, D);
        assert_eq!(r.kind, PinFlipKind::QuarterTurnNotSquare, "{r:?}");
        assert_eq!(r.seat_mismatch_mm, None, "{r:?}");
        let warnings = r.warnings();
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].contains("does not model it"), "{warnings:?}");
    }
}

/// The rotation reaches the project diagnostics: the same pins, the same
/// `Bottom` setup, and only `set_setup_rotation` changes the verdict.
#[test]
fn the_setup_rotation_reaches_the_project_diagnostics() {
    let mut stock = StockConfig {
        x: W,
        y: D,
        z: 25.0,
        origin_z: -25.0,
        auto_from_model: false,
        ..StockConfig::default()
    };
    stock.alignment_pins = y_mirror_pins();
    let mut session = ProjectSessionBuilder::new().stock(stock).build();
    let _ = session
        .apply(Command::SetSetupFace(SetSetupFaceArgs {
            setup_index: 0,
            face_up: FaceUp::Bottom,
        }))
        .expect("the default setup takes a face");
    let pin_verdicts = |session: &rs_cam_core::session::ProjectSession| -> Vec<String> {
        session
            .diagnostics()
            .verdicts
            .iter()
            .filter(|v| v.kind == VerdictKind::AlignmentPinsUnkeyed)
            .map(|v| v.headline.clone())
            .collect()
    };
    assert!(
        pin_verdicts(&session).is_empty(),
        "{:?}",
        pin_verdicts(&session)
    );

    let _ = session
        .apply(Command::SetSetupRotation(SetSetupRotationArgs {
            setup_index: 0,
            z_rotation: ZRotation::Deg180,
        }))
        .expect("the setup takes a rotation");
    let verdicts = pin_verdicts(&session);
    assert!(
        verdicts
            .iter()
            .any(|h| h.contains("do not survive the flip")),
        "{verdicts:?}"
    );
}
