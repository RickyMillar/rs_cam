//! Feeds contract D9 (operator ruling, 2026-11-04): the machine
//! aggressiveness dial is removed. Machine rigidity stays in
//! `machine::RigidityProfile` (`job.machine.rigidity`).
//!
//! # The claim
//!
//! A file that still holds the key `aggressiveness` refuses to load, and
//! the refusal names the key (ruling Q9 style: no key is ignored).
//!
//! 1. A project file with `aggressiveness` in `[job.machine]` refuses with
//!    `SessionError::RetiredKeys`. The row names the key and points at
//!    `job.machine.rigidity`.
//! 2. A machine library file with `aggressiveness` refuses
//!    (`deny_unknown_fields`), and the error text names the key.
//!
//! # Red-first
//!
//! Before D9 the dial was a field with `#[serde(default)]`: both files
//! loaded, and both arms panic at "must refuse".

#![expect(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::path::{Path, PathBuf};

use rs_cam_core::io::machine_library;
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::session::{ProjectFile, ProjectSession, SessionError};

const KEY: &str = "aggressiveness";

/// A small hand-written project.
const PROJECT: &str = r#"format_version = 3

[job]
name = "D9"

[job.stock]
x = 40.0
y = 40.0
z = 6.0
origin_z = -6.0
auto_from_model = false

[[tools]]
id = 0
name = "6 mm End Mill"
type = "end_mill"
diameter = 6.0
cutting_length = { stated = 20.0 }
"#;

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rs_cam_d9_{tag}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn load_text(text: &str) -> Result<ProjectSession, SessionError> {
    let project = ProjectFile::from_toml_str(text)?;
    ProjectSession::from_project_file(project, Path::new("."))
}

/// `text` with `aggressiveness = 0.85` set in the table at `path`.
fn with_dial(text: &str, path: &[&str]) -> String {
    let mut value: toml::Value = toml::from_str(text).unwrap();
    let mut here = &mut value;
    for step in path {
        here = &mut here[*step];
    }
    here.as_table_mut()
        .unwrap_or_else(|| panic!("{path:?} is not a table"))
        .insert(KEY.into(), toml::Value::Float(0.85));
    toml::to_string(&value).unwrap()
}

#[test]
fn a_project_file_with_the_dial_refuses_with_its_name_d9() {
    let dir = scratch("project");
    let src = dir.join("hand.toml");
    std::fs::write(&src, PROJECT).unwrap();
    let out = dir.join("saved.toml");
    ProjectSession::load(&src).unwrap().save(&out).unwrap();
    let saved = std::fs::read_to_string(&out).unwrap();
    // Non-vacuity: the saved file holds a machine table, and it loads.
    assert!(saved.contains("[job.machine]"), "{saved}");
    load_text(&saved).unwrap();

    let text = with_dial(&saved, &["job", "machine"]);
    match load_text(&text) {
        Err(SessionError::RetiredKeys(found)) => {
            assert_eq!(found.len(), 1, "{found:?}");
            assert!(found[0].path.ends_with(KEY), "{}", found[0].path);
            assert!(
                found[0].replacement.contains("job.machine.rigidity"),
                "{}",
                found[0].replacement
            );
        }
        Err(other) => panic!("expected the retirement message, got {other}"),
        Ok(_) => panic!("a project file with `{KEY}` must refuse"),
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_machine_file_with_the_dial_refuses_with_its_name_d9() {
    let dir = scratch("machine");
    let path = machine_library::save_to(&dir, "m", &MachineProfile::generic_wood_router()).unwrap();
    // Non-vacuity: the saved file loads.
    machine_library::load_from(&dir, "m").unwrap();

    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, with_dial(&text, &[])).unwrap();
    match machine_library::load_from(&dir, "m") {
        Err(err) => assert!(err.to_string().contains(KEY), "{err}"),
        Ok(_) => panic!("a machine file with `{KEY}` must refuse"),
    }
    let _ = std::fs::remove_dir_all(&dir);
}
