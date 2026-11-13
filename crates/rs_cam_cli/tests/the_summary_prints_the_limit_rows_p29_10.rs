//! Feeds contract P29-10 (10d): the CLI `project` command writes the verdict
//! rows of each toolpath (`summary.json` `per_toolpath[].limit_rows`), the
//! rows of the one door `ProjectSession::limit_rows` that MCP
//! `get_tool_load_report` prints.
//!
//! The fixture (`rs_cam_core/tests/fixtures/stock_change_cut_as_s6`) cuts an
//! own material (S6), so its rows hold an own-material row beside the stock
//! rows. The run simulates, so the gate rows are judged.
//!
//! The values: the test runs the same plan in process (the first op, a
//! simulation, the rest op, a simulation, modulation off as the CLI run) and
//! holds each `summary.json` row list equal to the door rows on that trace
//! (`ProjectSession::limit_rows`, the rows MCP prints).

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
        .arg("--no-adaptive-feed-modulation")
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
    // The values: the same plan in process, then the door.
    let mut session = rs_cam_core::session::ProjectSession::load(&project()).expect("loads");
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let options = rs_cam_core::session::SimulationOptions {
        resolution: rs_cam_core::session::SimulationResolution::Fixed(0.5),
        adaptive_feed_modulation: false,
        ..rs_cam_core::session::SimulationOptions::default()
    };
    for index in 0..session.toolpath_configs().len() {
        let _ = session
            .generate_toolpath(index, &cancel)
            .expect("the op generates");
        session
            .run_simulation(&options, &cancel)
            .expect("the session simulates");
    }
    let trace = session
        .simulation_result()
        .and_then(|s| s.cut_trace.clone());
    for entry in per_toolpath {
        let (index, _) = session
            .toolpath_configs()
            .iter()
            .enumerate()
            .find(|(_, tc)| serde_json::json!(tc.id) == entry["id"])
            .expect("the entry names a toolpath");
        let rows = session
            .limit_rows(index, trace.as_ref())
            .expect("the toolpath")
            .verdict();
        // Through text and back, as `summary.json` is read: the parser of
        // this crate reads a float to the nearest value it can, so both sides
        // take the same path.
        let text = serde_json::to_string(&rows).expect("the rows serialise");
        let door: serde_json::Value = serde_json::from_str(&text).expect("the rows parse");
        assert_eq!(entry["limit_rows"], door, "the rows of toolpath {index}");
    }
    let _ = std::fs::remove_dir_all(&out);
}
