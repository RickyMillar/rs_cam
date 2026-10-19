//! Geometry selection P3.2 (ruling D9, lead ruling O-1) — the old `depth`
//! and a pinned Bottom Z of Pocket, Profile, Zigzag and Adaptive 2D are
//! named at load, never read and never converted.
//!
//! P3.2 replaced the op `depth` of the four ops with `operation.params.floor`
//! (`floor = { from = "top", depth = 6.0 }`). A pinned Heights Bottom Z on
//! them never reached the cut (G-BOTTOMPIN). The loader refuses both keys
//! with `SessionError::RetiredKeys` (`RETIRED_OP_ENTRIES`), and the
//! refusal names each path and its fix. The same keys on another op load:
//! `depth` is still the dial of a Face, and a Face reads no pinned bottom.
//!
//! 1. The saved text writes `floor` with its reference, and loads (control).
//! 2. The text with `depth` and a pinned `bottom_z` on the Pocket refuses
//!    and names both paths and the fix of each.
//! 3. The same keys on the Face load.
//!
//! ```text
//! cargo test -p rs_cam_core -q --test a_retired_floor_key_is_named_p3_2
//! ```

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;
use common::make_endmill_6mm;
use common::session::toolpath_config;

use std::path::PathBuf;

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::session::{ProjectFile, ProjectSessionBuilder, SessionError};

fn temp_path() -> PathBuf {
    let mut dir = std::env::temp_dir();
    dir.push(format!("rs_cam_retired_floor_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.push("project.toml");
    dir
}

/// The text of a saved project with a Pocket (toolpath 0) and a Face
/// (toolpath 1).
fn saved_project_text() -> String {
    let mut builder = ProjectSessionBuilder::new();
    let tool_idx = builder.add_tool(make_endmill_6mm());
    let tool_id = builder.tools()[tool_idx].id.0;
    for (name, kind) in [
        ("Pocket", OperationType::Pocket),
        ("Face", OperationType::Face),
    ] {
        let tc = toolpath_config(name, OperationConfig::new_default(kind), tool_id, 0);
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

/// `text` with the old keys on toolpath `index`: `depth = 3.0` in place of
/// `floor` (when there is one), and a Manual Bottom Z.
fn with_old_keys(text: &str, index: usize) -> String {
    let mut raw: toml::Value = toml::from_str(text).unwrap();
    let tp = &mut raw["setups"][0]["toolpaths"][index];
    let params = tp["operation"]["params"].as_table_mut().unwrap();
    let _ = params.remove("floor");
    params.insert("depth".to_owned(), toml::Value::Float(3.0));
    let bottom: toml::Value = toml::from_str("mode = \"manual\"\nvalue = -20.0").unwrap();
    tp["heights"]
        .as_table_mut()
        .unwrap()
        .insert("bottom_z".to_owned(), bottom);
    toml::to_string(&raw).unwrap()
}

#[test]
fn a_retired_floor_key_is_named_p3_2() {
    let text = saved_project_text();
    let raw: toml::Value = toml::from_str(&text).unwrap();
    let pocket = &raw["setups"][0]["toolpaths"][0]["operation"]["params"];
    assert_eq!(
        pocket
            .get("floor")
            .and_then(|f| f.get("from"))
            .and_then(toml::Value::as_str),
        Some("top"),
        "the writer states the floor with its reference:\n{text}"
    );
    assert!(
        pocket.get("depth").is_none(),
        "the writer must not emit a retired key:\n{text}"
    );
    ProjectFile::from_toml_str(&text).expect("control: the saved project loads");

    let old = with_old_keys(&text, 0);
    let Err(err) = ProjectFile::from_toml_str(&old) else {
        panic!("a Pocket with `depth` and a pinned `bottom_z` must refuse, not load");
    };
    let SessionError::RetiredKeys(found) = &err else {
        panic!("expected SessionError::RetiredKeys, got {err}");
    };
    let mut paths: Vec<&str> = found.iter().map(|k| k.path.as_str()).collect();
    paths.sort_unstable();
    assert_eq!(
        paths,
        vec![
            "setups[0].toolpaths[0].heights.bottom_z",
            "setups[0].toolpaths[0].operation.params.depth",
        ],
        "one refusal names both old keys: {err}"
    );
    let message = err.to_string();
    assert!(
        message.contains("operation.params.floor")
            && message.contains("floor = { from = \"top\", depth = <this value> }")
            && message.contains("Set bottom_z to { mode = \"auto\" }"),
        "the refusal names the fix of each key: {message}"
    );

    // The same keys on the Face: `depth` is its own dial, and a pinned
    // bottom is not refused at load there.
    let face = with_old_keys(&text, 1);
    ProjectFile::from_toml_str(&face).expect("`depth` and a pinned bottom on a Face load");
}

/// An Auto `bottom_z` on a floor op is no old key: only a pin refuses.
#[test]
fn an_auto_bottom_z_on_a_pocket_loads() {
    let text = saved_project_text();
    let raw: toml::Value = toml::from_str(&text).unwrap();
    assert_eq!(
        raw["setups"][0]["toolpaths"][0]["heights"]["bottom_z"]["mode"].as_str(),
        Some("auto"),
        "fixture: the saved Pocket writes an Auto bottom_z"
    );
    ProjectFile::from_toml_str(&text).expect("an Auto bottom_z on a Pocket loads");
}
