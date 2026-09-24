//! G-RESTRES parity, the CLI half: `rs_cam_cli project` on a small rest
//! cascade, with NO `--resolution`, publishes the numbers `expected.json`
//! holds.
//!
//! Plan: `planning/rest_stock_identity_2026-09-24/PLAN.md` §5 test 9.
//! Operator rulings 2026-09-24: the GUI, MCP and the CLI give IDENTICAL
//! numbers for the same project state, and the project file stores the
//! resolution, so a rest cascade needs no `--resolution`.
//!
//! The GUI half is
//! `crates/rs_cam_viz/src/controller/tests/rest_cascade_parity_g_restres.rs`.
//! Both compare with the one `expected.json`.
//!
//! `RS_CAM_BLESS=1` writes `expected.json` from this run. Bless only on a
//! measured cause, and run the GUI half after.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::{Path, PathBuf};

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../rs_cam_core/tests/fixtures/rest_cascade_g_restres")
}

fn read_json(path: &Path) -> serde_json::Value {
    serde_json::from_str(
        &std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display())),
    )
    .unwrap_or_else(|e| panic!("{} is not JSON: {e}", path.display()))
}

#[test]
fn the_cli_publishes_the_expected_rest_numbers_with_no_resolution_flag() {
    let dir = fixture_dir();
    let out = std::env::temp_dir().join(format!("rs_cam_restres_cli_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_rs_cam_cli"))
        .arg("project")
        .arg(dir.join("project.toml"))
        .arg("--output-dir")
        .arg(&out)
        .status()
        .expect("the CLI runs");
    assert!(
        status.success(),
        "a rest cascade with a stored resolution needs no --resolution"
    );

    let summary = read_json(&out.join("summary.json"));
    assert_eq!(
        summary.pointer("/simulation_resolution/source"),
        Some(&serde_json::json!("project")),
        "the run read the project file's value: {summary}"
    );

    let mut toolpaths = serde_json::Map::new();
    let mut entries: Vec<PathBuf> = std::fs::read_dir(&out)
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("tp_") && n.ends_with(".json"))
        })
        .collect();
    entries.sort();
    for path in entries {
        let tp = read_json(&path);
        let name = tp["toolpath_name"].as_str().expect("a name").to_owned();
        let _ = toolpaths.insert(
            name,
            serde_json::json!({
                "move_count": tp["move_count"],
                "source_stock": tp["source_stock"],
            }),
        );
    }
    let record = serde_json::json!({
        "simulation_resolution_mm": summary["simulation_resolution"]["mm"],
        "toolpaths": toolpaths,
    });
    let _ = std::fs::remove_dir_all(&out);

    let expected_path = dir.join("expected.json");
    if std::env::var_os("RS_CAM_BLESS").is_some() {
        std::fs::write(
            &expected_path,
            serde_json::to_string_pretty(&record).unwrap() + "\n",
        )
        .unwrap();
    }
    assert!(
        record
            .pointer("/toolpaths/Rest pocket/source_stock/after/0/output")
            .is_some(),
        "non-vacuity: the rest pocket must record the pocket it read: {record}"
    );
    assert_eq!(
        record,
        read_json(&expected_path),
        "the CLI path must publish the numbers the GUI path publishes"
    );
}
