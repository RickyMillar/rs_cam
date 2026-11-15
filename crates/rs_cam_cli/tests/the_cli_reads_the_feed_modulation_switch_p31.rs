//! Operator rulings 2026-11-15 (P31 C, P32): the CLI `project` command
//! reads the ONE stored feed modulation switch of the project, and
//! `--feed-modulation <off|cap-only|full>` sets it on the in-memory session
//! through `Command::SetFeedModulation`, the door of the GUI control and of
//! MCP `set_feed_modulation`. The flag replaces
//! `--no-adaptive-feed-modulation` (P32, a break).
//!
//! - With no flag, the run uses the stored value (cap only with no key):
//!   `summary.json` `feed_modulation` is `{state: cap_only, source:
//!   project}`, no warning names a modulation as its cause, and each
//!   `per_toolpath[].emitted_feeds_warning` is the core text of the record
//!   (`EmittedFeeds::above_stated_warning`).
//! - `--feed-modulation full`: `{state: full, source: override}`, and a
//!   modulated op warns with "feed modulation of simulation run n".
//! - `--feed-modulation off`: `{state: off, source: override}`, every
//!   result holds its planned feeds.
//! - `--feed-modulation cap-only` on a file with no key states the stored
//!   value: the source stays `project`.
//! - A project file with `feed_modulation = "off"` and no flag:
//!   `{state: off, source: project}`. The boolean form refuses the load.
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

fn cli(project: &Path, out: &Path, flag: Option<&str>) -> std::process::Output {
    let _ = std::fs::remove_dir_all(out);
    let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_rs_cam_cli"));
    let _ = command
        .arg("project")
        .arg(project)
        .arg("--output-dir")
        .arg(out)
        .arg("--summary");
    if let Some(mode) = flag {
        let _ = command.arg("--feed-modulation").arg(mode);
    }
    command.output().expect("the CLI runs")
}

fn run_cli(project: &Path, out: &Path, flag: Option<&str>) -> (serde_json::Value, String) {
    let run = cli(project, out, flag);
    let stderr = String::from_utf8_lossy(&run.stderr).into_owned();
    assert!(run.status.success(), "the project run succeeds: {stderr}");
    let summary =
        serde_json::from_str(&std::fs::read_to_string(out.join("summary.json")).unwrap()).unwrap();
    (summary, stderr)
}

fn names_a_modulation(warnings: &[(serde_json::Value, Option<String>)]) -> bool {
    warnings.iter().any(|(_, w)| {
        w.as_ref()
            .is_some_and(|text| text.contains("feed modulation of simulation run"))
    })
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

    // No flag: the stored value, cap only with no key. No warning names a
    // modulation: cap only raises no feed.
    let (summary, stderr) = run_cli(&project, &out.join("default"), None);
    assert_eq!(
        summary["feed_modulation"],
        serde_json::json!({ "state": "cap_only", "source": "project" })
    );
    assert_eq!(summary["adaptive_feed_modulation"], true);
    let cap_only = in_process(&project, FeedModulationSwitch::CapOnly);
    assert_eq!(warnings_of(&summary), cap_only);
    assert!(!names_a_modulation(&cap_only), "{cap_only:?}");
    assert!(
        stderr.contains("Adaptive feed modulation: cap only (the project file"),
        "{stderr}"
    );
    let modes: Vec<&serde_json::Value> = summary["per_toolpath"]
        .as_array()
        .expect("per toolpath")
        .iter()
        .map(|entry| &entry["emitted_feeds"]["modulation_mode"])
        .collect();
    assert!(
        modes.iter().any(|m| **m == "cap_only") && modes.iter().all(|m| **m != "full"),
        "the records name the mode: {modes:?}"
    );

    // The flag with the stored value: nothing is set, the source is the
    // project.
    let (summary, _) = run_cli(&project, &out.join("cap_only"), Some("cap-only"));
    assert_eq!(
        summary["feed_modulation"],
        serde_json::json!({ "state": "cap_only", "source": "project" })
    );
    assert_eq!(warnings_of(&summary), cap_only);

    // Full: set for this run through the session command.
    let (summary, stderr) = run_cli(&project, &out.join("full"), Some("full"));
    assert_eq!(
        summary["feed_modulation"],
        serde_json::json!({ "state": "full", "source": "override" })
    );
    assert_eq!(summary["adaptive_feed_modulation"], true);
    let full = in_process(&project, FeedModulationSwitch::Full);
    assert_eq!(warnings_of(&summary), full);
    let warned: Vec<&String> = full.iter().filter_map(|(_, w)| w.as_ref()).collect();
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
    assert!(
        stderr.contains("Adaptive feed modulation: full (--feed-modulation"),
        "{stderr}"
    );

    // Off: set for this run through the session command.
    let (summary, stderr) = run_cli(&project, &out.join("off"), Some("off"));
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
    assert!(!names_a_modulation(&off), "{off:?}");
    assert!(
        stderr.contains("Adaptive feed modulation: off (--feed-modulation)"),
        "{stderr}"
    );

    // The retired flag is refused (the break of P32).
    let run = std::process::Command::new(env!("CARGO_BIN_EXE_rs_cam_cli"))
        .arg("project")
        .arg(&project)
        .arg("--output-dir")
        .arg(out.join("retired"))
        .arg("--no-adaptive-feed-modulation")
        .output()
        .expect("the CLI runs");
    assert!(!run.status.success(), "the retired flag is not a flag");

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
        "[job.simulation]\nfeed_modulation = \"off\"\n",
        1,
    );
    assert_ne!(keyed, text, "the fixture holds a [job.simulation] table");
    std::fs::write(&copy, keyed).unwrap();
    let (summary, stderr) = run_cli(&copy, &out.join("file_off"), None);
    assert_eq!(
        summary["feed_modulation"],
        serde_json::json!({ "state": "off", "source": "project" })
    );
    assert_eq!(warnings_of(&summary), off);
    assert!(
        stderr.contains("Adaptive feed modulation: off (the project file)"),
        "{stderr}"
    );

    // The boolean form of P31 refuses the load, and the message names the
    // three words.
    let boolean = text.replacen(
        "[job.simulation]\n",
        "[job.simulation]\nfeed_modulation = false\n",
        1,
    );
    std::fs::write(&copy, boolean).unwrap();
    let run = cli(&copy, &out.join("file_boolean"), None);
    let stderr = String::from_utf8_lossy(&run.stderr).into_owned();
    assert!(!run.status.success(), "a boolean key refuses: {stderr}");
    assert!(
        stderr.contains("\"off\", \"cap_only\" or \"full\""),
        "{stderr}"
    );
    let _ = std::fs::remove_dir_all(&out);
}
