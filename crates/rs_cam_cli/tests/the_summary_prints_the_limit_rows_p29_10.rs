//! Feeds contract P29-10 (10d): the CLI `project` command writes the verdict
//! rows of each toolpath (`summary.json` `per_toolpath[].limit_rows`), the
//! rows of the one door `ProjectSession::limit_rows` that MCP
//! `get_tool_load_report` prints.
//!
//! The fixture (`rs_cam_core/tests/fixtures/stock_change_cut_as_s6`) cuts an
//! own material (S6), so its rows hold an own-material row beside the stock
//! rows. The run simulates, so the gate rows are judged.
//!
//! NOT MEASURED here: the value equality with MCP on one session (the MCP
//! half, `the_mcp_rows_are_the_session_rows_p29_10`, holds the door against
//! the reply). This file holds the shape: one row per gate and cut group.

#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test code; a failed fixture is a failed test."
)]

use std::path::{Path, PathBuf};

fn project() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../rs_cam_core/tests/fixtures/stock_change_cut_as_s6/project.toml")
}

#[test]
fn the_summary_prints_the_limit_rows_p29_10() {
    let out = std::env::temp_dir().join(format!("rs_cam_rows_p29_10_{}", std::process::id()));
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
    let summary: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out.join("summary.json")).unwrap()).unwrap();
    let per_toolpath = summary["per_toolpath"].as_array().expect("per toolpath");
    assert!(!per_toolpath.is_empty(), "non-vacuity: toolpaths");
    let mut own_material_rows = 0;
    for entry in per_toolpath {
        let rows = entry["limit_rows"].as_array().expect("the rows");
        // Four gates over five cut groups and the gantry row, at least.
        assert!(rows.len() > 4 * 5, "{}: {} rows", entry["name"], rows.len());
        for row in rows {
            for key in [
                "subject",
                "check",
                "caveats",
                "exemption",
                "tone",
                "face",
                "detail",
            ] {
                assert!(row.get(key).is_some(), "a row without `{key}`: {row}");
            }
            assert_eq!(row["subject"]["kind"], "group", "{row}");
            own_material_rows += usize::from(!row["subject"]["material"].is_null());
        }
    }
    assert!(own_material_rows > 0, "non-vacuity: an own-material row");
    let _ = std::fs::remove_dir_all(&out);
}
