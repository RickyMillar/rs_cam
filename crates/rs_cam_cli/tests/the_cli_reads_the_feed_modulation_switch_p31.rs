//! Operator ruling 2026-11-15 (P31 C): the CLI `project` command reads the
//! ONE stored feed modulation switch of the project, and
//! `--no-adaptive-feed-modulation` sets it off on the in-memory session
//! through `Command::SetFeedModulation`, the door of the GUI checkbox and
//! of MCP `set_feed_modulation`.
//!
//! - With no flag, the run uses the stored value (on with no key):
//!   `summary.json` `feed_modulation` is `{state: on, source: project}`,
//!   and each `per_toolpath[].emitted_feeds_warning` is the core text of the
//!   record (`EmittedFeeds::above_stated_warning`).
//! - With the flag: `{state: off, source: override}`, every result holds
//!   its planned feeds, and each warning is the text of the same plan run in
//!   process with the switch off.
//! - A project file with `feed_modulation = false` and no flag:
//!   `{state: off, source: project}`.
//!
//! The values come from the same plan in process (the first op, a
//! simulation, the rest op, a simulation) through the session door.

#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test code; a failed fixture is a failed test."
)]

use std::path::{Path, PathBuf};

use rs_cam_core::session::{
    Command, FeedModulationSwitch, ProjectSession, SetFeedModulationArgs, SimulationOptions,
    SimulationResolution,
};

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../rs_cam_core/tests/fixtures/stock_change_cut_as_s6")
}

fn run_cli(project: &Path, out: &Path, flag: bool) -> (serde_json::Value, String) {
    let _ = std::fs::remove_dir_all(out);
    let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_rs_cam_cli"));
    let _ = command
        .arg("project")
        .arg(project)
        .arg("--output-dir")
        .arg(out)
        .arg("--summary");
    if flag {
        let _ = command.arg("--no-adaptive-feed-modulation");
    }
    let run = command.output().expect("the CLI runs");
    let stderr = String::from_utf8_lossy(&run.stderr).into_owned();
    assert!(run.status.success(), "the project run succeeds: {stderr}");
    let summary =
        serde_json::from_str(&std::fs::read_to_string(out.join("summary.json")).unwrap()).unwrap();
    (summary, stderr)
}

/// The plan of the CLI run in process, with the switch as given: the
/// warning text of each toolpath, by id.
fn in_process(
    project: &Path,
    switch: FeedModulationSwitch,
) -> Vec<(serde_json::Value, Option<String>)> {
    let mut session = ProjectSession::load(project).expect("loads");
    let _ = session
        .apply(Command::SetFeedModulation(SetFeedModulationArgs { switch }))
        .expect("the switch applies");
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let options = SimulationOptions {
        resolution: SimulationResolution::Fixed(0.5),
        ..SimulationOptions::default()
    };
    for index in 0..session.toolpath_configs().len() {
        let _ = session
            .generate_toolpath(index, &cancel)
            .expect("the op generates");
        session
            .run_simulation(&options, &cancel)
            .expect("the session simulates");
    }
    session
        .toolpath_configs()
        .iter()
        .enumerate()
        .map(|(index, tc)| {
            (
                serde_json::json!(tc.id),
                session
                    .emitted_feeds(index)
                    .and_then(|feeds| feeds.above_stated_warning()),
            )
        })
        .collect()
}

fn warnings_of(summary: &serde_json::Value) -> Vec<(serde_json::Value, Option<String>)> {
    summary["per_toolpath"]
        .as_array()
        .expect("per toolpath")
        .iter()
        .map(|entry| {
            (
                entry["id"].clone(),
                entry["emitted_feeds_warning"].as_str().map(str::to_owned),
            )
        })
        .collect()
}

#[test]
fn the_cli_reads_the_feed_modulation_switch_p31() {
    let project = fixture_dir().join("project.toml");
    let out = std::env::temp_dir().join(format!("rs_cam_switch_p31_{}", std::process::id()));

    // No flag: the stored value, on with no key.
    let (summary, stderr) = run_cli(&project, &out.join("on"), false);
    assert_eq!(
        summary["feed_modulation"],
        serde_json::json!({ "state": "on", "source": "project" })
    );
    assert_eq!(summary["adaptive_feed_modulation"], true);
    let on = in_process(&project, FeedModulationSwitch::On);
    assert_eq!(warnings_of(&summary), on);
    let warned: Vec<&String> = on.iter().filter_map(|(_, w)| w.as_ref()).collect();
    assert!(!warned.is_empty(), "non-vacuity: a modulated op warns");
    for warning in &warned {
        assert!(
            warning.contains("; feed modulation of simulation run "),
            "{warning}"
        );
        assert!(
            stderr.contains(&format!("WARNING {warning}")),
            "the report prints the warning: {stderr}"
        );
    }

    // The flag: off for this run, set through the session command.
    let (summary, stderr) = run_cli(&project, &out.join("off"), true);
    assert_eq!(
        summary["feed_modulation"],
        serde_json::json!({ "state": "off", "source": "override" })
    );
    assert_eq!(summary["adaptive_feed_modulation"], false);
    let off = in_process(&project, FeedModulationSwitch::Off);
    assert_eq!(warnings_of(&summary), off);
    for entry in summary["per_toolpath"].as_array().expect("per toolpath") {
        assert_eq!(
            entry["emitted_feeds"]["stage"]["kind"], "planned",
            "every result holds its planned feeds: {entry}"
        );
    }
    assert!(
        off.iter().all(|(_, w)| w
            .as_ref()
            .is_none_or(|text| !text.contains("feed modulation of simulation run"))),
        "no warning names a modulation: {off:?}"
    );
    assert!(
        stderr.contains("Adaptive feed modulation: off (--no-adaptive-feed-modulation)"),
        "{stderr}"
    );

    // A project file with the key and no flag: off, from the project.
    let copy_dir = out.join("copy");
    std::fs::create_dir_all(&copy_dir).unwrap();
    for entry in std::fs::read_dir(fixture_dir()).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_file() {
            let _ = std::fs::copy(entry.path(), copy_dir.join(entry.file_name())).unwrap();
        }
    }
    let copy = copy_dir.join("project.toml");
    let text = std::fs::read_to_string(&copy).unwrap();
    assert!(
        !text.contains("feed_modulation"),
        "the fixture states no feed modulation key"
    );
    let keyed = text.replacen(
        "[job.simulation]\n",
        "[job.simulation]\nfeed_modulation = false\n",
        1,
    );
    assert_ne!(keyed, text, "the fixture holds a [job.simulation] table");
    std::fs::write(&copy, keyed).unwrap();
    let (summary, stderr) = run_cli(&copy, &out.join("file_off"), false);
    assert_eq!(
        summary["feed_modulation"],
        serde_json::json!({ "state": "off", "source": "project" })
    );
    assert_eq!(warnings_of(&summary), off);
    assert!(
        stderr.contains("Adaptive feed modulation: off (the project file"),
        "{stderr}"
    );
    let _ = std::fs::remove_dir_all(&out);
}
