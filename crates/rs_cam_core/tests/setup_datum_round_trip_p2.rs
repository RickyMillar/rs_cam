//! W9 / P-2 — the setup datum is operator intent and must survive a
//! save/load round trip.
//!
//! # The defect this pins
//!
//! `SetupRuntime { datum, model_ids }` was GUI-only overlay state in
//! `rs_cam_viz`. The Setup properties panel wrote it, the viewport
//! crosshair and the setup chip read it, and **nothing** persisted it:
//! core's `ProjectSetupSection` and `SetupData` carried neither field.
//! Set "Z Datum = Machine Table", save, reload — the control read its
//! default again and the file contained no `z_datum` key.
//!
//! The keys existed only in the viz *fallback* schema
//! (`crates/rs_cam_viz/src/io/project.rs`), which is load-only and whose
//! `build_session_from_legacy_job` dropped them again anyway. So there
//! was no path — primary or fallback — on which a datum survived.
//!
//! # Red-first evidence (parent `777a78b`)
//!
//! [`a_hand_written_datum_survives_a_load_and_a_re_save`] is the
//! reproducer that compiles **unchanged** on the parent and fails there:
//! it hands the loader a project file that already contains the datum
//! keys, then re-saves it and reads the bytes back. On the parent the
//! keys are gone from the second file — no `deny_unknown_fields`
//! anywhere, so the loader silently discards them and the writer
//! rebuilds the file from a struct with no home for them. That is the
//! erasure mechanism, demonstrated without needing the fix's new API.
//!
//! The typed assertions in [`the_datum_reaches_the_session`] do not
//! compile on the parent, because the fields they read do not exist
//! there. That is the structural half of the same defect and is stated
//! rather than disguised.
//!
//! # `model_ids`: persisted, and why
//!
//! Empty means **all models**, which is not the same statement as an
//! explicit list naming every model, and neither is recoverable from
//! the setup's toolpaths — a toolpath names exactly one model, so the
//! scope is what the operator *allowed*, not what got *used*. It is
//! therefore persisted rather than derived. Note that the list is not
//! validated against the model table (a scope may name a model id that
//! no longer exists); that is the pre-existing behaviour of the
//! fallback schema and is unchanged here.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::PathBuf;

use rs_cam_core::compute::stock_config::ModelId;
use rs_cam_core::session::{Corner, ProjectSession, XYDatum, ZDatum};

/// A project file that already carries the datum keys. Deliberately
/// hand-written rather than produced by the writer under test: the
/// point is that the *loader* must not drop what the file says.
const PROJECT_WITH_DATUM: &str = r#"format_version = 3

[job]
name = "P-2 datum round trip"

[[setups]]
id = 0
name = "Setup 1"
face_up = "top"
xy_datum = "corner_br"
z_datum = "offset:-3.5"
datum_notes = "Probe the fixture pin, not the stock"
model_ids = [0, 2]
"#;

fn scratch_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rs_cam_p2_{tag}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    dir
}

/// **The P-2 reproducer.** Load a file that states a datum, save it back
/// out, and read the bytes. Red on the parent: every datum key is gone
/// from the second file.
#[test]
fn a_hand_written_datum_survives_a_load_and_a_re_save() {
    let dir = scratch_dir("resave");
    let src = dir.join("in.toml");
    let dst = dir.join("out.toml");
    std::fs::write(&src, PROJECT_WITH_DATUM).expect("write project");

    let session = ProjectSession::load(&src).expect("load project");
    session.save(&dst).expect("re-save project");
    let round_tripped = std::fs::read_to_string(&dst).expect("read re-saved project");

    // Parsed rather than grepped: `to_string_pretty` picks its own
    // array layout, and the claim is about the DATA surviving, not
    // about a byte pattern.
    let parsed: toml::Value =
        toml::from_str(&round_tripped).expect("the re-saved project must still be valid TOML");
    let setup = parsed
        .get("setups")
        .and_then(|s| s.as_array())
        .and_then(|s| s.first())
        .unwrap_or_else(|| panic!("re-saved project has no [[setups]]:\n{round_tripped}"));

    for (key, expected) in [
        ("xy_datum", "corner_br"),
        ("z_datum", "offset:-3.5"),
        ("datum_notes", "Probe the fixture pin, not the stock"),
    ] {
        assert_eq!(
            setup.get(key).and_then(|v| v.as_str()),
            Some(expected),
            "the re-saved project lost `{key}`. The loader dropped it, or the writer has \
             no home for it. Got:\n{round_tripped}"
        );
    }
    let model_ids: Vec<i64> = setup
        .get("model_ids")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(toml::Value::as_integer).collect())
        .unwrap_or_default();
    assert_eq!(
        model_ids,
        vec![0, 2],
        "the re-saved project lost the setup's model scope. Got:\n{round_tripped}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// The typed half: the values reach `SetupData`, with the exact
/// variants the keys name.
#[test]
fn the_datum_reaches_the_session() {
    let dir = scratch_dir("typed");
    let src = dir.join("in.toml");
    std::fs::write(&src, PROJECT_WITH_DATUM).expect("write project");

    let session = ProjectSession::load(&src).expect("load project");
    let setup = &session.list_setups()[0];

    assert_eq!(
        setup.datum.xy_method,
        XYDatum::CornerProbe(Corner::BackRight)
    );
    assert_eq!(setup.datum.z_method, ZDatum::FixedOffset(-3.5));
    assert_eq!(setup.datum.notes, "Probe the fixture pin, not the stock");
    assert_eq!(setup.model_ids, vec![ModelId(0), ModelId(2)]);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A project that never touched the Setup panel must serialise exactly
/// as it did before this landed — no new keys, no new noise. This is
/// what makes the schema addition genuinely additive.
#[test]
fn an_untouched_datum_writes_no_keys() {
    let dir = scratch_dir("untouched");
    let path = dir.join("plain.toml");

    let session = ProjectSession::new_empty();
    session.save(&path).expect("save empty project");
    let toml = std::fs::read_to_string(&path).expect("read project");

    for key in ["xy_datum", "z_datum", "datum_notes", "model_ids"] {
        assert!(
            !toml.contains(key),
            "a default setup must not write `{key}`; got:\n{toml}"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// An old file with no datum keys loads to the type defaults rather
/// than to an error — the `serde(default)` half of "additive".
#[test]
fn a_file_without_datum_keys_loads_the_defaults() {
    let dir = scratch_dir("legacy");
    let path = dir.join("legacy.toml");
    std::fs::write(
        &path,
        "format_version = 3\n\n[job]\nname = \"old\"\n\n[[setups]]\nid = 0\nname = \"Setup 1\"\n",
    )
    .expect("write legacy project");

    let session = ProjectSession::load(&path).expect("load legacy project");
    let setup = &session.list_setups()[0];
    assert_eq!(
        setup.datum.xy_method,
        XYDatum::CornerProbe(Corner::FrontLeft)
    );
    assert_eq!(setup.datum.z_method, ZDatum::StockTop);
    assert!(setup.datum.notes.is_empty());
    assert!(setup.model_ids.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

/// A datum the operator DID set survives a full session-level round
/// trip — set on the session, saved by the writer, read back typed.
/// This is the path the GUI actually takes (the Setup panel holds
/// `&mut SetupData`).
#[test]
fn a_datum_set_on_the_session_survives_save_and_load() {
    let dir = scratch_dir("session");
    let path = dir.join("project.toml");

    let mut session = ProjectSession::new_empty();
    {
        let setups = session.setups_mut();
        setups[0].datum.xy_method = XYDatum::AlignmentPins;
        setups[0].datum.z_method = ZDatum::MachineTable;
        setups[0].datum.notes = "Run the Z probe macro, then Resume".to_owned();
        setups[0].model_ids = vec![ModelId(3)];
    }
    session.save(&path).expect("save project");

    let reloaded = ProjectSession::load(&path).expect("reload project");
    let setup = &reloaded.list_setups()[0];
    assert_eq!(setup.datum.xy_method, XYDatum::AlignmentPins);
    assert_eq!(setup.datum.z_method, ZDatum::MachineTable);
    assert_eq!(setup.datum.notes, "Run the Z probe macro, then Resume");
    assert_eq!(setup.model_ids, vec![ModelId(3)]);
    let _ = std::fs::remove_dir_all(&dir);
}
