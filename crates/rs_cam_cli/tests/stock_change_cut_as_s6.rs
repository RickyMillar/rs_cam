//! S6: the CLI `project` command prints the "cut as" of each `Add` stock
//! change, and `summary.json` carries its token.
//!
//! Plan: `planning/stock_additions_2026-10-09/PLAN.md`, package S6.
//!
//! This is the CLI half of the GUI/MCP/CLI parity of `cut_as`. The fixture
//! (`rs_cam_core/tests/fixtures/stock_change_cut_as_s6`) holds a remove and
//! an add with `cut_as = "own_material"`. The CLI must print the core line
//! (`StockChangeRow::line`) of each. The GUI and MCP half is in
//! `rs_cam_viz/src/app/mcp/commands/stock_changes.rs`.

#![allow(
    // SAFETY: test code; a failed fixture is a failed test.
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::{Path, PathBuf};

use rs_cam_core::session::ProjectSession;

fn project() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../rs_cam_core/tests/fixtures/stock_change_cut_as_s6/project.toml")
}

#[test]
fn the_project_command_prints_the_cut_as_of_each_add_s6() {
    let out = std::env::temp_dir().join(format!("rs_cam_cut_as_s6_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    let run = std::process::Command::new(env!("CARGO_BIN_EXE_rs_cam_cli"))
        .arg("project")
        .arg(project())
        .arg("--output-dir")
        .arg(&out)
        .output()
        .expect("the CLI runs");
    let stderr = String::from_utf8_lossy(&run.stderr);
    assert!(run.status.success(), "the project run succeeds: {stderr}");

    let session = ProjectSession::load(&project()).expect("the fixture loads");
    let rows = session.stock_change_rows();
    assert_eq!(rows.len(), 2);
    // The add names its cut-as after the material; the remove names none.
    assert!(rows[1].line().contains("material Resin (own material): "));
    assert!(!rows[0].line().contains("(cut as stock)"));
    assert!(!rows[0].line().contains("(own material)"));

    // The CLI prints the core line. The volume part depends on the run, so
    // compare the text up to the volume.
    for row in &rows {
        let line = row.line();
        let head = &line[..line.rfind(": ").unwrap()];
        assert!(
            stderr.contains(head),
            "the CLI prints the core line `{head}`:\n{stderr}"
        );
    }

    let summary: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out.join("summary.json")).unwrap()).unwrap();
    let listed = summary["stock_changes"].as_array().expect("listed");
    assert_eq!(listed[1]["cut_as"], serde_json::json!("own_material"));
    assert!(listed[0]["cut_as"].is_null(), "a remove ignores cut_as");
    let _ = std::fs::remove_dir_all(&out);
}
