//! Geometry selection P2.3c — a retired drill pick key is named, never
//! read.
//!
//! P2.3c deleted `selected_holes` and `selected_layers` from the Drill and
//! the Pin Drill config. The drill picks are keys in
//! `operation.params.selection` now. Ruling of 2026-09-16 (no legacy): an
//! old key does not convert. The loader refuses it and names the key and
//! what replaced it (`RETIRED_KEYS`, DESIGN 10.1).
//!
//! The test saves a real project with a Drill and a Pin Drill, writes the
//! two old keys into each operation's params table, and loads the text
//! through the one text door (`ProjectFile::from_toml_str`):
//!
//! 1. the unchanged text loads (control);
//! 2. the text with the old keys refuses with `SessionError::RetiredKeys`,
//!    which names all four keys (C2: one refusal names every key) and the
//!    replacement text of each.
//!
//! ```text
//! cargo test -p rs_cam_core -q --test a_retired_drill_pick_key_is_named_p2_3c
//! ```

#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

mod common;
use common::make_endmill_6mm;
use common::session::toolpath_config;

use std::path::PathBuf;

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::session::{ProjectFile, ProjectSessionBuilder, SessionError};

fn temp_path() -> PathBuf {
    let mut dir = std::env::temp_dir();
    dir.push(format!("rs_cam_retired_drill_pick_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.push("project.toml");
    dir
}

/// The text of a saved project with a Drill (toolpath 0) and a Pin Drill
/// (toolpath 1).
fn saved_project_text() -> String {
    let mut builder = ProjectSessionBuilder::new();
    let tool_idx = builder.add_tool(make_endmill_6mm());
    let tool_id = builder.tools()[tool_idx].id.raw();
    for (name, kind) in [
        ("Drill", OperationType::Drill),
        ("Pin Drill", OperationType::AlignmentPinDrill),
    ] {
        let tc = toolpath_config(
            name,
            OperationConfig::new_default(kind),
            rs_cam_core::ids::ToolId::from_raw(tool_id),
            rs_cam_core::ids::ModelId::from_raw(0),
        );
        let _ = builder.add_toolpath(0, tc).expect("add toolpath");
    }
    let session = builder.build();
    let path = temp_path();
    session.save(&path).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    if let Some(dir) = path.parent() {
        let _ = std::fs::remove_dir(dir);
    }
    text
}

/// `text` with the two retired keys in the operation params of every
/// toolpath.
fn with_retired_keys(text: &str) -> String {
    let mut raw: toml::Value = toml::from_str(text).unwrap();
    let mut written = 0;
    for setup in raw["setups"].as_array_mut().unwrap() {
        for tp in setup["toolpaths"].as_array_mut().unwrap() {
            let params = tp["operation"]["params"].as_table_mut().unwrap();
            params.insert(
                "selected_holes".to_owned(),
                toml::Value::try_from(vec![[1.0_f64, 2.0]]).unwrap(),
            );
            params.insert(
                "selected_layers".to_owned(),
                toml::Value::try_from(vec!["holes"]).unwrap(),
            );
            written += 1;
        }
    }
    assert_eq!(
        written, 2,
        "fixture is vacuous: the keys must reach both ops"
    );
    toml::to_string(&raw).unwrap()
}

#[test]
fn a_retired_drill_pick_key_is_named_p2_3c() {
    let text = saved_project_text();
    assert!(
        !text.contains("selected_holes") && !text.contains("selected_layers"),
        "the writer must not emit a retired key:\n{text}"
    );
    ProjectFile::from_toml_str(&text).expect("control: the saved project loads");

    let old = with_retired_keys(&text);
    let Err(err) = ProjectFile::from_toml_str(&old) else {
        panic!("a project with `selected_holes` must refuse, not load");
    };
    let SessionError::RetiredKeys(found) = &err else {
        panic!("expected SessionError::RetiredKeys, got {err}");
    };
    let mut paths: Vec<&str> = found.iter().map(|k| k.path.as_str()).collect();
    paths.sort_unstable();
    assert_eq!(
        paths,
        vec![
            "setups[0].toolpaths[0].operation.params.selected_holes",
            "setups[0].toolpaths[0].operation.params.selected_layers",
            "setups[0].toolpaths[1].operation.params.selected_holes",
            "setups[0].toolpaths[1].operation.params.selected_layers",
        ],
        "the refusal names every retired key: {err}"
    );
    let message = err.to_string();
    assert!(
        message.contains("drill picks are in operation.params.selection"),
        "the refusal names the replacement of `selected_holes`: {message}"
    );
    assert!(
        message.contains("use a layer rule in operation.params.selection"),
        "the refusal names the replacement of `selected_layers`: {message}"
    );
}
