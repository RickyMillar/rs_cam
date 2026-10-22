//! Geometry selection P2.6 — the retired `face_selection` key is named,
//! never read.
//!
//! P2.6 deleted `ToolpathConfig.face_selection`. A face pick is a
//! `MatchKey` pick in `operation.params.selection` now. An old face pick
//! held a BREP face id (`FaceGroupId`), which is topological order and no
//! geometry key, so it cannot convert. Ruling of 2026-09-16 (no legacy):
//! the loader refuses the key and names it and what replaced it
//! (`RETIRED_KEYS`, DESIGN 10.1).
//!
//! 1. The saved text holds no `face_selection` and loads (control).
//! 2. The text with `face_selection = [0, 3]` on each toolpath refuses
//!    with `SessionError::RetiredKeys`, which names the key on each
//!    toolpath (C2: one refusal names every key) and the replacement.
//!
//! ```text
//! cargo test -p rs_cam_core -q --test a_retired_selection_key_is_named_p2_6
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
    dir.push(format!("rs_cam_retired_face_pick_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.push("project.toml");
    dir
}

/// The text of a saved project with a Pocket (toolpath 0) and a Drop
/// Cutter (toolpath 1): an op with a selection and an op without one.
fn saved_project_text() -> String {
    let mut builder = ProjectSessionBuilder::new();
    let tool_idx = builder.add_tool(make_endmill_6mm());
    let tool_id = builder.tools()[tool_idx].id.raw();
    for (name, kind) in [
        ("Pocket", OperationType::Pocket),
        ("Drop Cutter", OperationType::DropCutter),
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

/// `text` with `face_selection = [0, 3]` on every toolpath.
fn with_retired_key(text: &str) -> String {
    let mut raw: toml::Value = toml::from_str(text).unwrap();
    let mut written = 0;
    for setup in raw["setups"].as_array_mut().unwrap() {
        for tp in setup["toolpaths"].as_array_mut().unwrap() {
            tp.as_table_mut().unwrap().insert(
                "face_selection".to_owned(),
                toml::Value::try_from(vec![0_i64, 3]).unwrap(),
            );
            written += 1;
        }
    }
    assert_eq!(
        written, 2,
        "fixture is vacuous: the key must reach both ops"
    );
    toml::to_string(&raw).unwrap()
}

#[test]
fn a_retired_selection_key_is_named_p2_6() {
    let text = saved_project_text();
    assert!(
        !text.contains("face_selection"),
        "the writer must not emit a retired key:\n{text}"
    );
    ProjectFile::from_toml_str(&text).expect("control: the saved project loads");

    let old = with_retired_key(&text);
    let Err(err) = ProjectFile::from_toml_str(&old) else {
        panic!("a project with `face_selection` must refuse, not load");
    };
    let SessionError::RetiredKeys(found) = &err else {
        panic!("expected SessionError::RetiredKeys, got {err}");
    };
    let mut paths: Vec<&str> = found.iter().map(|k| k.path.as_str()).collect();
    paths.sort_unstable();
    assert_eq!(
        paths,
        vec![
            "setups[0].toolpaths[0].face_selection",
            "setups[0].toolpaths[1].face_selection",
        ],
        "the refusal names the key on every toolpath: {err}"
    );
    let message = err.to_string();
    assert!(
        message.contains("face picks are in operation.params.selection")
            && message.contains("Re-pick the faces"),
        "the refusal names the replacement of `face_selection`: {message}"
    );
}
