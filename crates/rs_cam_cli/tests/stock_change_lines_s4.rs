//! S4 — the CLI `project` command prints one line per stock change with its
//! volume, and `summary.json` carries the same rows.
//!
//! Plan: `planning/stock_additions_2026-10-09/PLAN.md`, package S4.
//!
//! This is the CLI half of the GUI/MCP/CLI volume parity. The fixture
//! (`rs_cam_core/tests/fixtures/stock_change_parity_s4`) holds two stock
//! changes on Setup 1. The test runs the CLI binary, then runs the same
//! project in process through `ProjectSession::run_simulation` and
//! `ProjectSession::stock_change_rows`, the read the GUI panel and the MCP
//! tools print. The CLI must print exactly those lines. The GUI and MCP
//! half is in `rs_cam_viz/src/app/mcp/commands/stock_changes.rs`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use rs_cam_core::session::{ProjectSession, SimulationOptions};

fn project() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../rs_cam_core/tests/fixtures/stock_change_parity_s4/project.toml")
}

#[test]
fn the_project_command_prints_the_core_line_for_each_stock_change_s4() {
    assert!(project().is_file(), "the fixture exists");
    let out = std::env::temp_dir().join(format!("rs_cam_stock_change_s4_{}", std::process::id()));
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

    // The core read, in process: the rows every surface prints.
    let mut session = ProjectSession::load(&project()).expect("the fixture loads");
    let opts = SimulationOptions {
        resolution: session.simulation_resolution_mm(),
        ..SimulationOptions::default()
    };
    let _ = session
        .run_simulation(&opts, &AtomicBool::new(false))
        .expect("the stock changes apply");
    let rows = session.stock_change_rows();
    assert_eq!(rows.len(), 2, "the fixture holds two stock changes");

    // Non-vacuity: the volumes are measured and match the geometry within
    // one ring of 0.5 mm cells around the 30 x 30 mm square.
    let slab = rows[0].volume.as_ref().expect("the slab is measured");
    let pour = rows[1].volume.as_ref().expect("the pour is measured");
    assert!((slab.net_ml() + 0.90).abs() <= 0.06, "slab {slab:?}");
    assert!((pour.net_ml() - 0.45).abs() <= 0.06, "pour {pour:?}");

    assert!(stderr.contains("Stock changes (2):"), "{stderr}");
    for row in &rows {
        assert!(
            stderr.contains(&row.line()),
            "the CLI prints the core line `{}`:\n{stderr}",
            row.line()
        );
    }

    let summary: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out.join("summary.json")).unwrap()).unwrap();
    let listed = summary["stock_changes"]
        .as_array()
        .expect("summary.json lists them");
    assert_eq!(listed.len(), rows.len());
    for (entry, row) in listed.iter().zip(&rows) {
        assert_eq!(entry["volume_label"], serde_json::json!(row.volume_label()));
        assert_eq!(entry["line"], serde_json::json!(row.line()));
        assert_eq!(entry["op"], serde_json::json!(row.change.op.label()));
    }
    let _ = std::fs::remove_dir_all(&out);
}
