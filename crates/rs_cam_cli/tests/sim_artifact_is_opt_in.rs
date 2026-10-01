//! `rs_cam_cli project` writes `simulation.json` only on `--sim-artifact`
//! (2026-10-02, breaking).
//!
//! The file holds every cut-trace sample: 6.7 GB on rivmap350, on every
//! run. The GUI cut-trace file is off by default (operator ruling
//! 2026-10-02), so the CLI file is also off by default. `summary.json` and
//! the `tp_*.json` files are written on every run.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::{Path, PathBuf};

fn project() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../rs_cam_core/tests/fixtures/rest_cascade_g_restres/project.toml")
}

/// Run `project` into a fresh scratch folder and answer the file names.
fn run(tag: &str, extra: &[&str]) -> (PathBuf, Vec<String>) {
    let out = std::env::temp_dir().join(format!("rs_cam_simart_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    let status = std::process::Command::new(env!("CARGO_BIN_EXE_rs_cam_cli"))
        .arg("project")
        .arg(project())
        .arg("--output-dir")
        .arg(&out)
        .args(extra)
        .status()
        .expect("the CLI runs");
    assert!(status.success(), "the project run succeeds ({tag})");
    let mut names: Vec<String> = std::fs::read_dir(&out)
        .expect("the output folder exists")
        .map(|entry| {
            entry
                .expect("an entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    (out, names)
}

fn has_toolpath_files(names: &[String]) -> bool {
    names
        .iter()
        .any(|n| n.starts_with("tp_") && n.ends_with(".json"))
}

#[test]
fn simulation_json_is_written_only_on_sim_artifact() {
    assert!(project().is_file(), "the fixture exists");

    let (off_dir, off) = run("off", &[]);
    assert!(
        !off.iter().any(|n| n == "simulation.json"),
        "no --sim-artifact: no simulation.json: {off:?}"
    );
    assert!(off.iter().any(|n| n == "summary.json"), "{off:?}");
    assert!(has_toolpath_files(&off), "{off:?}");

    // Non-vacuity: the same run with the flag writes the file, so the
    // fixture does capture a cut trace and the absence above is the flag.
    let (on_dir, on) = run("on", &["--sim-artifact"]);
    assert!(
        on.iter().any(|n| n == "simulation.json"),
        "--sim-artifact writes simulation.json: {on:?}"
    );
    assert!(on.iter().any(|n| n == "summary.json"), "{on:?}");
    assert!(has_toolpath_files(&on), "{on:?}");

    let _ = std::fs::remove_dir_all(off_dir);
    let _ = std::fs::remove_dir_all(on_dir);
}
