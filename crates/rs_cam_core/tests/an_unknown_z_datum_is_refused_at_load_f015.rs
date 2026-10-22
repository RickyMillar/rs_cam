//! F015 (tech-debt review 2026-10-19, operator ruling 1): a setup datum
//! token that the vocabulary does not hold refuses the project load.
//!
//! # The defect
//!
//! `ZDatum::from_key` ended in `_ => ZDatum::StockTop`, and
//! `offset:<garbage>` read as `FixedOffset(0.0)`. `XYDatum::from_key` and
//! `Corner::from_key` fell back to the front-left corner. The loader gave
//! no warning. A misspelt `table` became `StockTop`, and the stock-top
//! datum shifts the program Z0 at export. The operator ruled that an
//! unknown `z_datum` never falls back to the stock top.
//!
//! # What is asserted
//!
//! 1. An unknown `z_datum` token refuses with
//!    `SessionError::UnknownSetupDatum`. The refusal names the setup, the
//!    key and the token.
//! 2. `offset:` with a value that is not a finite number refuses.
//! 3. An unknown `xy_datum` token, and a corner token outside the four
//!    corners, refuse the same way.
//! 4. Every valid token parses to its value, and a load and a re-save keep
//!    each value.
//!
//! # Red-first
//!
//! Put back the old fall-through `_ => ZDatum::StockTop` (with the
//! `Option` return, `_ => Some(ZDatum::StockTop)`) in
//! `session/mod.rs`. Assertion 1 fails: the load succeeds with
//! `StockTop`.

#![expect(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::path::Path;

use rs_cam_core::session::{Corner, ProjectFile, ProjectSession, SessionError, XYDatum, ZDatum};

/// A minimal project with one setup that carries the two datum keys.
fn project_text(xy: &str, z: &str) -> String {
    format!(
        "format_version = 3\n\n[job]\nname = \"F015\"\n\n[[setups]]\nid = 0\n\
         name = \"Flip side\"\nface_up = \"top\"\nxy_datum = \"{xy}\"\nz_datum = \"{z}\"\n\
         datum_notes = \"probe\"\n"
    )
}

fn load(text: &str) -> Result<ProjectSession, SessionError> {
    let project = ProjectFile::from_toml_str(text)?;
    ProjectSession::from_project_file(project, Path::new("."))
}

/// The refusal for `text`, which must be `UnknownSetupDatum`.
fn refusal(text: &str) -> (String, &'static str, String, String) {
    match load(text) {
        Err(err @ SessionError::UnknownSetupDatum { .. }) => {
            let message = err.to_string();
            let SessionError::UnknownSetupDatum { setup, key, token } = err else {
                unreachable!()
            };
            (setup, key, token, message)
        }
        Err(other) => panic!("expected UnknownSetupDatum, got {other}"),
        Ok(session) => panic!(
            "the load must refuse, but the datum read as {:?}",
            session.list_setups()[0].datum
        ),
    }
}

#[test]
fn an_unknown_z_datum_is_refused_at_load_f015() {
    let (setup, key, token, message) = refusal(&project_text("pins", "tabel"));
    assert_eq!(setup, "Flip side");
    assert_eq!(key, "z_datum");
    assert_eq!(token, "tabel");
    for part in ["Flip side", "z_datum", "tabel"] {
        assert!(
            message.contains(part),
            "the message must name {part}: {message}"
        );
    }
}

#[test]
fn an_offset_that_is_not_a_finite_number_is_refused_f015() {
    for token in [
        "offset:",
        "offset:abc",
        "offset:NaN",
        "offset:inf",
        "offset:-inf",
    ] {
        let (_, key, found, _) = refusal(&project_text("pins", token));
        assert_eq!(key, "z_datum", "{token}");
        assert_eq!(found, token);
    }
}

#[test]
fn an_unknown_xy_datum_or_corner_is_refused_f015() {
    for token in ["centre", "corner_", "corner_zz", "corner_FL", "Pins"] {
        let (setup, key, found, _) = refusal(&project_text(token, "table"));
        assert_eq!(setup, "Flip side");
        assert_eq!(key, "xy_datum", "{token}");
        assert_eq!(found, token);
    }
}

#[test]
fn every_valid_datum_token_round_trips_f015() {
    let xy = [
        ("corner_fl", XYDatum::CornerProbe(Corner::FrontLeft)),
        ("corner_fr", XYDatum::CornerProbe(Corner::FrontRight)),
        ("corner_bl", XYDatum::CornerProbe(Corner::BackLeft)),
        ("corner_br", XYDatum::CornerProbe(Corner::BackRight)),
        ("center", XYDatum::CenterOfStock),
        ("pins", XYDatum::AlignmentPins),
        ("manual", XYDatum::Manual),
    ];
    let z = [
        ("stock_top", ZDatum::StockTop),
        ("table", ZDatum::MachineTable),
        ("manual", ZDatum::Manual),
        ("offset:-3.5", ZDatum::FixedOffset(-3.5)),
        ("offset:0", ZDatum::FixedOffset(0.0)),
    ];
    for (token, value) in &xy {
        assert_eq!(XYDatum::from_key(token).as_ref(), Some(value), "{token}");
        assert_eq!(XYDatum::from_key(&value.to_key()).as_ref(), Some(value));
    }
    for (token, value) in &z {
        assert_eq!(ZDatum::from_key(token).as_ref(), Some(value), "{token}");
        assert_eq!(ZDatum::from_key(&value.to_key()).as_ref(), Some(value));
    }

    // Through the file: load each pair, save it, load the save again.
    let dir = std::env::temp_dir().join(format!("rs_cam_f015_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for (xy_token, xy_value) in &xy {
        for (z_token, z_value) in &z {
            let session = load(&project_text(xy_token, z_token)).unwrap();
            let path = dir.join("round.toml");
            session.save(&path).unwrap();
            let again = ProjectSession::load(&path).unwrap();
            let datum = &again.list_setups()[0].datum;
            assert_eq!(&datum.xy_method, xy_value, "{xy_token} / {z_token}");
            assert_eq!(&datum.z_method, z_value, "{xy_token} / {z_token}");
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}
