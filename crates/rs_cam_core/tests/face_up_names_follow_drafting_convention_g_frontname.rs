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

use rs_cam_core::compute::transform::{FaceUp, ZRotation};
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

// ── CMP-15: the stored key is a door, and it must not fail open ────────

/// Every variant's own key parses back to that variant. The table above
/// says what a name MEANS; this says the file format can still express it.
#[test]
fn every_face_up_key_round_trips_g_frontname() {
    for face_up in [
        FaceUp::Top,
        FaceUp::Bottom,
        FaceUp::Front,
        FaceUp::Back,
        FaceUp::Left,
        FaceUp::Right,
    ] {
        assert_eq!(
            FaceUp::from_key(face_up.to_key()),
            Some(face_up),
            "`FaceUp::{face_up:?}` does not survive its own key"
        );
    }
    for rotation in [
        ZRotation::Deg0,
        ZRotation::Deg90,
        ZRotation::Deg180,
        ZRotation::Deg270,
    ] {
        assert_eq!(
            ZRotation::from_key(rotation.to_key()),
            Some(rotation),
            "`ZRotation::{rotation:?}` does not survive its own key"
        );
    }
}

/// A token the vocabulary does not hold is refused, not silently mapped.
///
/// CMP-15: both parsers ended in a wildcard that produced `Top` and
/// `Deg0`. The empty string is in the list on purpose — it is what serde
/// hands a `face_up` key that is present and blank.
#[test]
fn an_unknown_key_is_refused_g_frontname() {
    for token in ["topp", "TOP", "", "up", "bottom ", "45"] {
        assert_eq!(
            FaceUp::from_key(token),
            None,
            "`FaceUp::from_key({token:?})` invented an orientation"
        );
    }
    for token in ["360", "90.0", "", "ninety", "-90"] {
        assert_eq!(
            ZRotation::from_key(token),
            None,
            "`ZRotation::from_key({token:?})` invented a rotation"
        );
    }
}

/// The load case the finding names: a project file with `face_up = "topp"`
/// warns and names the offending token.
///
/// The Q4 policy (`compute/tool_config.rs`) is that a file-loading surface
/// warns and takes the default while a mutation surface refuses. MCP
/// honoured its half; the loader did neither — it defaulted and stayed
/// quiet. The consequence is not cosmetic: a bottom setup read as a top
/// one moves the cut direction, the local stock box and the emission
/// frame.
#[test]
fn a_typo_in_face_up_warns_on_load_g_frontname() {
    const PROJECT: &str = r#"format_version = 3

[job]
name = "CMP-15 unknown face up"

[[setups]]
id = 0
name = "Back face"
face_up = "topp"
z_rotation = "45"
"#;

    let dir = std::env::temp_dir().join(format!("rs_cam_cmp15_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    let path = dir.join("unknown_face_up.toml");
    std::fs::write(&path, PROJECT).expect("write project");

    let (session, warnings) =
        rs_cam_core::session::ProjectSession::load_with_warnings(&path).expect("load project");

    let messages: Vec<String> = warnings.iter().map(|w| w.message()).collect();
    assert!(
        messages.iter().any(|m| m.contains("topp")),
        "the loader took an unknown `face_up` in silence. Warnings were: {messages:?}"
    );
    assert!(
        messages.iter().any(|m| m.contains("45")),
        "the loader took an unknown `z_rotation` in silence. Warnings were: {messages:?}"
    );
    assert!(
        messages.iter().any(|m| m.contains("Back face")),
        "a warning must name the setup it is about. Warnings were: {messages:?}"
    );

    // The load still succeeds and takes the default — that is the Q4
    // policy, not an accident.
    let setup = &session.list_setups()[0];
    assert_eq!(setup.face_up, FaceUp::Top);
    assert_eq!(setup.z_rotation, ZRotation::Deg0);
}
