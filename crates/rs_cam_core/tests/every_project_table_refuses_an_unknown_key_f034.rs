//! F034 (operator ruling 5, 2026-10-19) and F053: the project loader
//! refuses a key that a closed table does not hold, and a toolpath must
//! name its tool and its model.
//!
//! # The defect
//!
//! No persisted project struct had `deny_unknown_fields`. A misspelt key
//! (`diamter`, `z_datun`, `aggresiveness`) loaded as the default of the
//! field it meant, with no word. A toolpath with no `tool_id` bound to
//! tool 0 (`unwrap_or(0)`), and two entries could carry one id.
//!
//! # Scope
//!
//! Closed: the root, `[job]`, `[job.stock]` (and its material),
//! `[job.simulation]`, `[job.post]`, `[job.machine]` (and its spindle,
//! power, chip load, rigidity and controller tables), `[[tools]]`,
//! `[[models]]` and `[[setups]]`. The toolpath section and the operation
//! tables stay open by agreement with the geometry-selection programme.
//!
//! # What is asserted
//!
//! 1. A project saved by rs_cam loads and saves to the same bytes.
//! 2. One unknown key in each closed table refuses, and the message names
//!    the key.
//! 3. A retired key in a closed table keeps its retirement message.
//! 4. A toolpath with no `tool_id` or no `model_id` refuses. Two entries
//!    of one kind with one id refuse.
//!
//! # Red-first
//!
//! Remove `#[serde(deny_unknown_fields)]` from `ProjectToolSection` in
//! `session/project_file.rs`: assertion 2 fails for `tools[0]`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::{Path, PathBuf};

use rs_cam_core::session::{ProjectFile, ProjectSession, SessionError};

/// A hand-written project that touches every closed table.
const PROJECT: &str = r#"format_version = 3

[job]
name = "F034"

[job.simulation]
resolution_mm = 0.5

[job.stock]
x = 40.0
y = 40.0
z = 6.0
origin_z = -6.0
auto_from_model = false

[job.stock.material.Custom]
name = "test board"
feed_scale_factor = 1.0

[job.post]
format = "grblhal"

[[tools]]
id = 0
name = "6 mm End Mill"
type = "end_mill"
diameter = 6.0
cutting_length = 20.0

[[models]]
id = 0
path = "absent.svg"
name = "Square"
kind = "svg"

[[setups]]
id = 0
name = "Setup 1"
xy_datum = "pins"
z_datum = "table"

[[setups.toolpaths]]
id = 0
name = "Pocket"
tool_id = 0
model_id = 0

[setups.toolpaths.operation]
kind = "pocket"

[setups.toolpaths.operation.params]
stepover = 2.0
depth = 3.0
depth_per_pass = 3.0
feed_rate = 1000.0
plunge_rate = 500.0
climb = true
pattern = "contour"
angle = 0.0
finishing_passes = 0
"#;

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rs_cam_f034_{tag}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn load_text(text: &str) -> Result<ProjectSession, SessionError> {
    let project = ProjectFile::from_toml_str(text)?;
    ProjectSession::from_project_file(project, Path::new("."))
}

/// The file rs_cam writes for [`PROJECT`], with a `$$` controller table
/// added so that table is in the file too.
fn saved_project(dir: &Path) -> String {
    let src = dir.join("hand.toml");
    std::fs::write(&src, PROJECT).unwrap();
    let session = ProjectSession::load(&src).unwrap();
    let out = dir.join("saved.toml");
    session.save(&out).unwrap();
    let mut value: toml::Value = toml::from_str(&std::fs::read_to_string(&out).unwrap()).unwrap();
    let machine = value["job"]["machine"].as_table_mut().unwrap();
    let mut controller = toml::Table::new();
    controller.insert("tool_change_mode".into(), toml::Value::Integer(3));
    machine.insert("controller".into(), toml::Value::Table(controller));
    toml::to_string(&value).unwrap()
}

/// The table at `path` in `value` (an integer step indexes an array).
fn table_at<'a>(value: &'a mut toml::Value, path: &[&str]) -> &'a mut toml::Table {
    let mut here = value;
    for step in path {
        here = match step.parse::<usize>() {
            Ok(index) => &mut here[index],
            Err(_) => &mut here[*step],
        };
    }
    here.as_table_mut()
        .unwrap_or_else(|| panic!("{path:?} is not a table"))
}

/// `text` with `key = 1` added to the table at `path`.
fn with_key(text: &str, path: &[&str], key: &str) -> String {
    let mut value: toml::Value = toml::from_str(text).unwrap();
    table_at(&mut value, path).insert(key.into(), toml::Value::Integer(1));
    toml::to_string(&value).unwrap()
}

#[test]
fn a_saved_project_round_trips_byte_for_byte_f034() {
    let dir = scratch("round");
    let first = saved_project(&dir);
    let a = dir.join("a.toml");
    std::fs::write(&a, &first).unwrap();
    let b = dir.join("b.toml");
    ProjectSession::load(&a).unwrap().save(&b).unwrap();
    let second = std::fs::read_to_string(&b).unwrap();
    let c = dir.join("c.toml");
    ProjectSession::load(&b).unwrap().save(&c).unwrap();
    let third = std::fs::read_to_string(&c).unwrap();
    assert_eq!(second, third, "a load and a save must keep every byte");
    for needle in [
        "tool_change_mode = 3",
        "[job.stock.material.Custom]",
        "z_datum = \"table\"",
    ] {
        assert!(
            second.contains(needle),
            "the save lost `{needle}`:\n{second}"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn every_closed_table_refuses_an_unknown_key_f034() {
    let dir = scratch("deny");
    let base = saved_project(&dir);
    // Non-vacuity: the base itself loads.
    load_text(&base).unwrap();
    let closed: &[&[&str]] = &[
        &[],
        &["job"],
        &["job", "simulation"],
        &["job", "stock"],
        &["job", "stock", "material", "Custom"],
        &["job", "post"],
        &["job", "machine"],
        &["job", "machine", "spindle", "Variable"],
        &["job", "machine", "power", "ConstantPower"],
        &["job", "machine", "chip_load"],
        &["job", "machine", "rigidity"],
        &["job", "machine", "controller"],
        &["tools", "0"],
        &["models", "0"],
        &["setups", "0"],
    ];
    for path in closed {
        let text = with_key(&base, path, "typo_key");
        match load_text(&text) {
            Err(SessionError::TomlParse(message)) => assert!(
                message.contains("typo_key"),
                "{path:?}: the refusal must name the key: {message}"
            ),
            Err(other) => panic!("{path:?}: expected a parse refusal, got {other}"),
            Ok(_) => panic!("{path:?}: an unknown key must refuse"),
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_retired_key_in_a_closed_table_keeps_its_message_f034() {
    let dir = scratch("retired");
    let base = saved_project(&dir);
    for (path, key, word) in [
        (&[][..], "toolpaths", "[[setups.toolpaths]]"),
        (&["job"][..], "machine_ref", "snapshot"),
        (&["job", "machine"][..], "safety_factor", "aggressiveness"),
        (
            &["job", "stock", "material", "Custom"][..],
            "kc",
            "force line",
        ),
    ] {
        let text = with_key(&base, path, key);
        match load_text(&text) {
            Err(SessionError::RetiredKeys(found)) => {
                assert_eq!(found.len(), 1, "{key}: {found:?}");
                assert!(found[0].path.ends_with(key), "{key}: {}", found[0].path);
                assert!(
                    found[0].replacement.contains(word),
                    "{key}: {}",
                    found[0].replacement
                );
            }
            Err(other) => panic!("{key}: expected the retirement message, got {other}"),
            Ok(_) => panic!("{key}: a retired key must refuse"),
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_toolpath_must_name_its_tool_and_its_model_f053() {
    for key in ["tool_id", "model_id"] {
        let text = PROJECT.replace(&format!("{key} = 0\n"), "");
        assert_ne!(text, PROJECT, "non-vacuity: {key} is in the fixture");
        match load_text(&text) {
            Err(SessionError::TomlParse(message)) => {
                assert!(message.contains(key), "{key}: {message}");
            }
            Err(other) => panic!("{key}: expected a parse refusal, got {other}"),
            Ok(_) => panic!("{key}: a toolpath with no {key} must refuse"),
        }
    }
}

#[test]
fn two_entries_with_one_id_refuse_f053() {
    let tool = "\n[[tools]]\nid = 0\nname = \"Twin\"\ntype = \"end_mill\"\n";
    let twin_tool = PROJECT.replace("\n[[models]]", &format!("{tool}\n[[models]]"));
    let twin_setup = format!(
        "{PROJECT}\n[[setups]]\nname = \"No id, index 1\"\n\n[[setups]]\nid = 1\nname = \"Id 1\"\n"
    );
    let twin_toolpath = format!(
        "{PROJECT}\n[[setups.toolpaths]]\nid = 0\nname = \"Twin\"\ntool_id = 0\nmodel_id = 0\n\n\
         [setups.toolpaths.operation]\nkind = \"pocket\"\n\n\
         [setups.toolpaths.operation.params]\nstepover = 2.0\ndepth = 3.0\n\
         depth_per_pass = 3.0\nfeed_rate = 1000.0\nplunge_rate = 500.0\nclimb = true\n\
         pattern = \"contour\"\nangle = 0.0\nfinishing_passes = 0\n"
    );
    for (text, section, id) in [
        (twin_tool, "tools", 0),
        (twin_setup, "setups", 1),
        (twin_toolpath, "toolpaths", 0),
    ] {
        match load_text(&text) {
            Err(SessionError::DuplicateId {
                section: found,
                id: found_id,
            }) => {
                assert_eq!((found, found_id), (section, id));
            }
            Err(other) => panic!("{section}: expected DuplicateId, got {other}"),
            Ok(_) => panic!("{section}: two entries with id {id} must refuse"),
        }
    }
}
