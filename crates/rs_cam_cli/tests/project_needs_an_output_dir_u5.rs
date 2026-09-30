//! U5 (memory programme 2026-10-01): `rs_cam_cli project` writes nothing
//! into the current directory unless the caller names a directory.
//!
//! The command wrote `./diagnostics` by default. Its `simulation.json` holds
//! the full cut trace, which is 8 GB on rivmap350. `--output-dir` is now
//! required, so a run without it stops at argument parsing and writes
//! nothing.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::Path;

#[test]
fn project_without_an_output_dir_refuses_and_writes_nothing_u5() {
    let project = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../rs_cam_core/tests/fixtures/rest_cascade_g_restres/project.toml");
    assert!(
        project.is_file(),
        "the fixture must exist, or the refusal below could be a missing-file error: {}",
        project.display()
    );
    let cwd = std::env::temp_dir().join(format!("rs_cam_u5_cwd_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&cwd);
    std::fs::create_dir_all(&cwd).expect("create the scratch cwd");

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_rs_cam_cli"))
        .current_dir(&cwd)
        .arg("project")
        .arg(&project)
        .output()
        .expect("the CLI runs");

    assert!(
        !output.status.success(),
        "a project run with no --output-dir must refuse"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--output-dir"),
        "the refusal must name the missing flag: {stderr}"
    );
    let entries: Vec<_> = std::fs::read_dir(&cwd)
        .expect("read the scratch cwd")
        .map(|entry| entry.expect("a directory entry").file_name())
        .collect();
    assert!(
        entries.is_empty(),
        "the refused run must write nothing into the current directory: {entries:?}"
    );
    let _ = std::fs::remove_dir_all(&cwd);
}
