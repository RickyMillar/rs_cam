//! Feeds contract P30-1 ("print what runs"): the CLI `project` command
//! writes the record of the feeds that an export emits now
//! (`summary.json` `per_toolpath[].emitted_feeds`), the record of the one
//! door `ProjectSession::emitted_feeds` that the G-code header, MCP and the
//! GUI read. The run modulates (the default), so the record holds the
//! modulated feeds.
//!
//! The values: the test runs the same plan in process (the first op, a
//! simulation, the rest op, a simulation, modulation on as the CLI run) and
//! holds each `summary.json` record equal to the door record. The GUI and
//! MCP twin is the viz lib test `one_feed_record_on_every_surface_p30_1`.

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
fn the_summary_prints_the_emitted_feeds_p30_1() {
    let out = std::env::temp_dir().join(format!("rs_cam_feeds_p30_1_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    let run = std::process::Command::new(env!("CARGO_BIN_EXE_rs_cam_cli"))
        .arg("project")
        .arg(project())
        .arg("--output-dir")
        .arg(&out)
        .arg("--summary")
        .output()
        .expect("the CLI runs");
    let stderr = String::from_utf8_lossy(&run.stderr);
    assert!(run.status.success(), "the project run succeeds: {stderr}");
    let summary: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out.join("summary.json")).unwrap()).unwrap();
    let per_toolpath = summary["per_toolpath"].as_array().expect("per toolpath");
    assert!(!per_toolpath.is_empty(), "non-vacuity: toolpaths");
    // The same plan in process, then the door.
    let mut session = rs_cam_core::session::ProjectSession::load(&project()).expect("loads");
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let options = rs_cam_core::session::SimulationOptions {
        resolution: rs_cam_core::session::SimulationResolution::Fixed(0.5),
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
    let mut modulated = 0;
    for entry in per_toolpath {
        let (index, _) = session
            .toolpath_configs()
            .iter()
            .enumerate()
            .find(|(_, tc)| serde_json::json!(tc.id) == entry["id"])
            .expect("the entry names a toolpath");
        let door = session.emitted_feeds(index).expect("a record");
        let text = serde_json::to_string(&door).expect("the record serialises");
        let door_json: serde_json::Value = serde_json::from_str(&text).expect("parses");
        assert_eq!(entry["emitted_feeds"], door_json, "the record of {index}");
        modulated += usize::from(door_json["stage"]["kind"] == "modulated_current");
        // The stderr report prints the core line.
        assert!(
            stderr.contains(&format!("feeds {}", door.line())),
            "the report prints the line of {index}: {stderr}"
        );
    }
    assert!(modulated > 0, "non-vacuity: a modulated record");
    let _ = std::fs::remove_dir_all(&out);
}
