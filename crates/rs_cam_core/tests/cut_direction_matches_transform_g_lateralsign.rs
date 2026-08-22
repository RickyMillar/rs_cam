//! **G-LATERALSIGN** — `SetupTransformInfo::cut_direction()` must agree with
//! `FaceUp::inverse_transform_point` about which way the tool advances.
//!
//! # Two independent statements of one fact
//!
//! A setup makes the same claim twice, in two places that never talk to each
//! other:
//!
//! 1. `FaceUp::inverse_transform_point` says where a setup-local point lands
//!    in the stock-relative global frame. The tool always advances along
//!    **local −Z**, so the sign of local Z's contribution to the global frame
//!    *is* the advance direction, and it is derivable from the transform with
//!    two point evaluations.
//! 2. `cut_direction()` names a `StockCutDirection`, whose
//!    `cuts_from_high_side()` decides whether the dexel kernel calls
//!    `subtract_above` or `subtract_below` on the global stock.
//!
//! Nothing checked that those two agree. On the four **lateral** faces they
//! did not: `cut_direction()` mapped `FaceUp::Front → FromFront`,
//! `Back → FromBack`, `Left → FromLeft`, `Right → FromRight`, and all four
//! are the wrong sign.
//!
//! The mapping looked obviously right, which is why it survived — but the two
//! names describe **opposite ends of the same setup**. `FaceUp::Front` means
//! *the front face is up*, toward the spindle. `StockCutDirection::FromFront`
//! means *the tool arrives from the front side*, i.e. from −Y — and its own
//! doc says so (`cut_direction.rs`: "Tool enters from the front face (−Y
//! side)"). If the front face is up, the tool arrives from where that face
//! now points, which after the transform is **+Y**. That is `FromBack`. The
//! correct mapping is a negation on both lateral axes, and writing it as an
//! identity is the defect.
//!
//! # Why it went unnoticed
//!
//! The Z faces are self-checking by luck — `Top → FromTop` and
//! `Bottom → FromBottom` are both right — so the mapping reads as an obvious
//! identity and the two cases anyone tests both pass. And the consequence is
//! confined to the **global playback stock**: every metric, gate, collision
//! check and checkpoint mesh is computed on the per-setup `group_stock`, which
//! `compute/simulate.rs` stamps with a hardcoded `FromTop` because setup-local
//! Z always is the tool axis. So the wrong sign never reaches a number an
//! operator reads — it reaches the live-scrub viewport, where it removes
//! everything from the far face up to the cut plane instead of the shallow
//! layer at the near face.
//!
//! # How this test avoids repeating the mistake
//!
//! It does not transcribe the expected direction per face — a table of six
//! hand-written answers is the same kind of artefact as the mapping it is
//! checking, and would have been written wrong by the same reasoning. Instead
//! it **measures** the transform: push two points differing only in local Z
//! through `inverse_transform_point`, see which global axis moved and with
//! what sign, and require `cut_direction()` to match. The two Z faces are
//! included and act as the control — if the derivation itself were wrong, they
//! would fail too.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::compute::transform::{FaceUp, SetupTransformInfo, ZRotation};
use rs_cam_core::dexel::DexelAxis;
use rs_cam_core::geo::P3;

const W: f64 = 40.0;
const D: f64 = 30.0;
const H: f64 = 25.0;

fn info(face_up: FaceUp) -> SetupTransformInfo {
    SetupTransformInfo {
        face_up,
        z_rotation: ZRotation::Deg0,
        stock_x: W,
        stock_y: D,
        stock_z: H,
        ..Default::default()
    }
}

/// Measure where local +Z goes in the global frame: which axis, and which
/// sign. Two evaluations of the real transform — no table.
fn local_z_axis_in_global(face_up: FaceUp) -> (DexelAxis, f64) {
    let (eff_w, eff_d, eff_h) = face_up.effective_stock(W, D, H);
    let _ = eff_h;
    // Mid-face so no coordinate is pinned at a boundary, then a step along
    // local +Z only.
    let a = P3::new(eff_w * 0.5, eff_d * 0.5, 1.0);
    let b = P3::new(eff_w * 0.5, eff_d * 0.5, 2.0);
    let ga = face_up.inverse_transform_point(a, W, D, H);
    let gb = face_up.inverse_transform_point(b, W, D, H);
    let d = (gb.x - ga.x, gb.y - ga.y, gb.z - ga.z);

    let moved: Vec<(DexelAxis, f64)> = [
        (DexelAxis::X, d.0),
        (DexelAxis::Y, d.1),
        (DexelAxis::Z, d.2),
    ]
    .into_iter()
    .filter(|(_, v)| v.abs() > 1e-9)
    .collect();

    assert_eq!(
        moved.len(),
        1,
        "{face_up:?}: a step along local +Z must move exactly one global axis \
         (these transforms are 90-degree permutations), moved {moved:?}"
    );
    let (axis, delta) = moved[0];
    (axis, delta.signum())
}

/// The whole test. For every face: the grid axis and the high/low-side flag
/// that `cut_direction()` reports must be the ones the transform implies.
#[test]
fn cut_direction_agrees_with_the_setup_transform_on_every_face() {
    let mut checked = 0;
    for &face_up in FaceUp::ALL {
        let (axis_from_transform, z_sign) = local_z_axis_in_global(face_up);
        let dir = info(face_up).cut_direction();

        assert_eq!(
            dir.grid_axis(),
            axis_from_transform,
            "{face_up:?}: the transform sends local Z onto the global \
             {axis_from_transform:?} axis, so the cut direction must stamp \
             that grid, but {dir:?} stamps {:?}",
            dir.grid_axis()
        );

        // The tool advances along local −Z. If local +Z maps to global +axis,
        // the tool advances toward decreasing axis values — it enters from the
        // high side and removes material above the cutter. If local +Z maps to
        // global −axis, the reverse.
        let expected_high_side = z_sign > 0.0;
        assert_eq!(
            dir.cuts_from_high_side(),
            expected_high_side,
            "{face_up:?}: local +Z maps to global {axis_from_transform:?} with \
             sign {z_sign:+.0}, so the tool (advancing along local −Z) enters \
             from the {} side and the kernel must call subtract_{}. \
             `cut_direction()` says {dir:?}, whose cuts_from_high_side() is \
             {}.\n\n\
             `FaceUp::{face_up:?}` names the face that is UP; \
             `StockCutDirection::{dir:?}` names the side the TOOL COMES FROM. \
             Those are opposite ends of one setup, so the mapping between them \
             is a negation on the lateral axes, not an identity.",
            if expected_high_side { "high" } else { "low" },
            if expected_high_side { "above" } else { "below" },
            dir.cuts_from_high_side(),
        );
        checked += 1;
    }
    // Non-vacuity: all six faces, or the loop silently proved nothing.
    assert_eq!(checked, 6, "every FaceUp variant must be checked");
}

/// The control that gives the test above its authority: the derivation is
/// validated on the two faces whose answer is not in dispute. If
/// `local_z_axis_in_global` were itself wrong, these would fail first.
#[test]
fn the_derivation_reproduces_the_two_undisputed_faces() {
    let (axis, sign) = local_z_axis_in_global(FaceUp::Top);
    assert_eq!(axis, DexelAxis::Z);
    assert!(sign > 0.0, "Top is the identity: local +Z is global +Z");

    let (axis, sign) = local_z_axis_in_global(FaceUp::Bottom);
    assert_eq!(axis, DexelAxis::Z);
    assert!(sign < 0.0, "Bottom mirrors Z: local +Z is global −Z");
}
