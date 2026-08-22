//! **G-FRONTNAME** — `FaceUp`'s four lateral names must name the world face
//! a draftsman would call by that name.
//!
//! # The ruling (operator, 2026-08-22)
//!
//! > `FaceUp::Front` machines the world **−Y** face. `Back` → +Y,
//! > `Left` → −X, `Right` → +X.
//!
//! That is the drafting convention every CAD package uses, and it is already
//! what the rest of this product says out loud: the composite screenshot
//! renderer labels its panels *front* = the −Y eye, *rear* = the +Y eye,
//! *left* = −X, *right* = +X.
//!
//! # The defect
//!
//! The shipped transform picked the **opposite** face on all four,
//! systematically. `FaceUp::Front`'s forward map was `(x, H − z, y)`, whose
//! local `+Z` is world `+Y`, so the face pointing at the spindle was the
//! **+Y** one — drafting's *back*. Same inversion on all four laterals.
//!
//! Confirmed live on 2026-08-22 before the fix: a `face_up = "front"` demo
//! pocket landed on the `y = 60` face of a 100 × 60 × 40 blank and rendered
//! in the composite's REAR panels. The operator ruled that the drafting
//! convention wins — the composite labels stay, the transform moves.
//!
//! # Why this file is a table and `cut_direction_matches_transform_g_lateralsign`
//! is a derivation
//!
//! Those two files pin different kinds of statement, and the difference is
//! the point:
//!
//! * G-LATERALSIGN pins an **internal consistency** — two places in the code
//!   making the same claim. Nothing outside the code decides the answer, so
//!   transcribing a table of six would just be the buggy mapping written
//!   twice. It derives.
//! * G-FRONTNAME pins an **external ruling** — a naming convention that comes
//!   from drafting practice and from the composite renderer's labels, neither
//!   of which this crate can compute. A table is the only honest form: it is
//!   the ruling itself, written down.
//!
//! The two Z faces are included as the control on the probe. They were never
//! in dispute, and if the probe were measuring the wrong thing they would be
//! the first to say so.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::compute::transform::FaceUp;
use rs_cam_core::geo::P3;

/// Deliberately three different numbers: a transform that lands on the right
/// plane by permuting the wrong axis cannot hide behind a cube.
const W: f64 = 40.0;
const D: f64 = 30.0;
const H: f64 = 25.0;

const EPS: f64 = 1e-9;

/// The point the tool touches first: the centre of the setup-local top face,
/// in setup-local coordinates. `effective_stock` says how big that face is,
/// so the probe never hard-codes which stock dimension became which local
/// one — only that it is the middle of the top.
fn local_top_centre(face_up: FaceUp) -> P3 {
    let (eff_w, eff_d, eff_h) = face_up.effective_stock(W, D, H);
    P3::new(eff_w * 0.5, eff_d * 0.5, eff_h)
}

/// The ruling, written down. For each face: where the local top centre must
/// land in the stock-relative world frame.
///
/// Every entry is the **centre of one world face**, which is what makes the
/// assertion three-dimensional: it pins the plane the face is on *and* that
/// the other two axes came through the middle rather than off an edge.
fn ruling() -> Vec<(FaceUp, P3, &'static str)> {
    vec![
        (
            FaceUp::Top,
            P3::new(W * 0.5, D * 0.5, H),
            "the +Z face — the control, never in dispute",
        ),
        (
            FaceUp::Bottom,
            P3::new(W * 0.5, D * 0.5, 0.0),
            "the -Z face — the control, never in dispute",
        ),
        (
            FaceUp::Front,
            P3::new(W * 0.5, 0.0, H * 0.5),
            "the -Y face: drafting's FRONT, and the composite's front panel",
        ),
        (
            FaceUp::Back,
            P3::new(W * 0.5, D, H * 0.5),
            "the +Y face: drafting's BACK, and the composite's rear panel",
        ),
        (
            FaceUp::Left,
            P3::new(0.0, D * 0.5, H * 0.5),
            "the -X face: drafting's LEFT, and the composite's left panel",
        ),
        (
            FaceUp::Right,
            P3::new(W, D * 0.5, H * 0.5),
            "the +X face: drafting's RIGHT, and the composite's right panel",
        ),
    ]
}

#[test]
fn face_up_names_follow_drafting_convention_g_frontname() {
    let mut checked = 0;
    for (face_up, want, why) in ruling() {
        let got = face_up.inverse_transform_point(local_top_centre(face_up), W, D, H);
        assert!(
            (got.x - want.x).abs() < EPS
                && (got.y - want.y).abs() < EPS
                && (got.z - want.z).abs() < EPS,
            "G-FRONTNAME: `FaceUp::{face_up:?}` must machine {why}.\n\
             The centre of its setup-local top face lands at \
             ({:.3}, {:.3}, {:.3}) in the stock-relative world frame, but the \
             ruling puts it at ({:.3}, {:.3}, {:.3}) on a {W} x {D} x {H} blank.\n\n\
             This is the operator ruling of 2026-08-22: the four lateral \
             `FaceUp` names mean the world faces a draftsman means by them, \
             which is also what the composite screenshot renderer's panel \
             labels already say. If you are changing this assertion, you are \
             changing the product's naming convention, not fixing a test.",
            got.x,
            got.y,
            got.z,
            want.x,
            want.y,
            want.z
        );
        checked += 1;
    }
    // Non-vacuity: all six faces, or the loop proved nothing.
    assert_eq!(checked, 6, "every FaceUp variant must be checked");
}

/// The forward map is the same ruling read the other way, and it has to be
/// the actual inverse — a fix applied to only one of the two arms would leave
/// world→local and local→world disagreeing about which face is up, which no
/// single-direction test can see.
#[test]
fn the_forward_map_puts_the_named_world_face_on_top() {
    for (face_up, world_face_centre, why) in ruling() {
        let local = face_up.transform_point(world_face_centre, W, D, H);
        let (_, _, eff_h) = face_up.effective_stock(W, D, H);
        assert!(
            (local.z - eff_h).abs() < EPS,
            "G-FRONTNAME: `FaceUp::{face_up:?}` machines {why}, so the centre \
             of that world face must arrive at the setup-local TOP \
             (local z = {eff_h}); it arrived at local z = {:.3}. The forward \
             and inverse arms have to be swapped together.",
            local.z
        );

        // And the round trip closes, on the real pair.
        let back = face_up.inverse_transform_point(local, W, D, H);
        assert!(
            (back.x - world_face_centre.x).abs() < EPS
                && (back.y - world_face_centre.y).abs() < EPS
                && (back.z - world_face_centre.z).abs() < EPS,
            "`FaceUp::{face_up:?}`: inverse_transform_point is not the inverse \
             of transform_point — ({:.3}, {:.3}, {:.3}) round-tripped to \
             ({:.3}, {:.3}, {:.3})",
            world_face_centre.x,
            world_face_centre.y,
            world_face_centre.z,
            back.x,
            back.y,
            back.z
        );
    }
}
