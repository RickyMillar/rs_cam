//! F1.24 — `ProjectSession::save` gives every CALL its own temp file.
//!
//! # The defect this pins
//!
//! `save` writes the project atomically: it writes a temp file into the
//! destination directory, then renames that file onto the destination. The
//! temp name was `.rs_cam_save_{pid}.tmp` — the process id and nothing
//! else. Two saves running at the same time in ONE process therefore wrote
//! ONE file. The interleaving decides the damage:
//!
//! - Save A writes the temp file. Save B overwrites it. Save A renames.
//!   Path A now holds B's bytes, and save A reports success.
//! - Save B then renames a file that is no longer there. It fails with
//!   `NotFound`.
//!
//! The sequential case is litter, not breakage: `std::fs::write` truncates,
//! so a temp file left by an earlier failed save is simply overwritten.
//! Nothing removed it, so it stayed in the project directory, hidden.
//!
//! # The three tests and what each one is worth
//!
//! - `the_temp_name_is_not_the_pid_alone` is the GUARANTEED red-first
//!   evidence. It builds the old name and puts a directory there, so the
//!   old code writes onto a directory and fails. One limitation, stated so
//!   that nobody over-reads the test: it knows the old name. If the
//!   `.rs_cam_save` prefix is ever renamed, the obstruction stops
//!   obstructing and this test goes vacuous.
//! - `two_saves_into_one_directory_both_succeed` carries the CONTRACT. It
//!   is PROBABILISTIC before the fix, because the two saves must interleave
//!   for the defect to show, and it is deterministic after the fix. A green
//!   run of this test on unfixed code is luck, not the absence of the race.
//! - `a_failed_save_removes_its_temp_file` pins the cleanup half. The
//!   destination is a directory, so the rename always fails.
//!
//! # Red-first evidence
//!
//! RED-FIRST OUTPUT: pending verifier run

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::{Path, PathBuf};
use std::sync::Barrier;

use rs_cam_core::session::ProjectSession;

/// How many saves each arm of the concurrent test runs.
const ROUNDS: usize = 32;

/// The marker in the first arm's project name.
const MARKER_A: &str = "MARKER-A";

/// The marker in the second arm's project name.
const MARKER_B: &str = "MARKER-B";

/// A directory of this test's own, under the system temp directory.
fn test_dir(name: &str) -> PathBuf {
    let pid = std::process::id();
    let dir = std::env::temp_dir().join(format!("rs_cam_f124_{pid}_{name}"));
    // An earlier run of this binary can leave the directory behind.
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create the test directory");
    dir
}

/// The temp files a save left behind, if any.
fn leftover_temp_files(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .expect("read the test directory")
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with(".rs_cam_save"))
        .collect();
    names.sort();
    names
}

/// Save one session `ROUNDS` times and check its file after every save.
///
/// The barrier holds this thread until its peer arrives, so both threads
/// enter `save` together. The function reports every failure instead of
/// panicking, so one arm can never leave the other waiting at the barrier.
fn save_rounds(
    session: &ProjectSession,
    path: &Path,
    marker: &str,
    barrier: &Barrier,
) -> Vec<String> {
    let mut failures = Vec::new();
    for round in 0..ROUNDS {
        barrier.wait();
        if let Err(e) = session.save(path) {
            failures.push(format!("round {round}: {marker} save: {e}"));
            continue;
        }
        match std::fs::read_to_string(path) {
            Ok(text) => {
                if !text.contains(marker) {
                    let note = format!("round {round}: no {marker} in the file");
                    failures.push(note);
                }
            }
            Err(e) => {
                failures.push(format!("round {round}: {marker} read: {e}"));
            }
        }
    }
    failures
}

/// The temp name must not be derivable from the process id alone.
#[test]
fn the_temp_name_is_not_the_pid_alone() {
    let dir = test_dir("pid_name");
    let pid = std::process::id();
    let obstruction = dir.join(format!(".rs_cam_save_{pid}.tmp"));
    std::fs::create_dir(&obstruction).expect("create the obstruction");
    // Non-vacuity floor: the obstruction must really be in the way.
    assert!(obstruction.is_dir(), "the obstruction is a directory");

    let mut session = ProjectSession::new_empty();
    session.set_name("Pid Name".to_owned());

    let project = dir.join("project.toml");
    let saved = session.save(&project);
    assert!(
        saved.is_ok(),
        "the temp name must not be built from the pid alone; got {saved:?}"
    );
    assert!(project.is_file(), "the save wrote the project file");

    let _ = std::fs::remove_dir_all(&dir);
}

/// Two saves from ONE process into ONE directory both succeed, and each
/// file holds the session it was asked to hold.
#[test]
fn two_saves_into_one_directory_both_succeed() {
    let dir = test_dir("concurrent");
    let path_a = dir.join("a.toml");
    let path_b = dir.join("b.toml");

    // Non-vacuity floor: the two saves must really share a directory.
    assert_eq!(
        path_a.parent(),
        path_b.parent(),
        "the two saves must target one directory, or this test proves nothing"
    );

    // A long name makes the serialized TOML large. A large write widens the
    // window in which the two saves can interleave, so the pre-fix failure
    // is reliable rather than lucky.
    let padding = "x".repeat(128_000);
    let mut session_a = ProjectSession::new_empty();
    session_a.set_name(format!("{MARKER_A}-{padding}"));
    let mut session_b = ProjectSession::new_empty();
    session_b.set_name(format!("{MARKER_B}-{padding}"));

    let barrier = Barrier::new(2);
    let a_path = path_a.clone();
    let b_path = path_b.clone();

    let failures = std::thread::scope(|scope| {
        let gate = &barrier;
        let run_a = move || save_rounds(&session_a, &a_path, MARKER_A, gate);
        let run_b = move || save_rounds(&session_b, &b_path, MARKER_B, gate);
        let arm_a = scope.spawn(run_a);
        let arm_b = scope.spawn(run_b);
        let mut failures = arm_a.join().expect("arm a must not panic");
        failures.extend(arm_b.join().expect("arm b must not panic"));
        failures
    });

    assert!(
        failures.is_empty(),
        "every save must succeed and hold its own project; got {failures:?}"
    );

    // The cleanup half: a successful save leaves nothing behind.
    let leftovers = leftover_temp_files(&dir);
    assert!(
        leftovers.is_empty(),
        "a successful save must leave no temp file; found {leftovers:?}"
    );

    let loaded_a = ProjectSession::load(&path_a).expect("load a");
    let loaded_b = ProjectSession::load(&path_b).expect("load b");
    assert!(
        loaded_a.name().starts_with(MARKER_A),
        "a.toml must load back as its own project"
    );
    assert!(
        loaded_b.name().starts_with(MARKER_B),
        "b.toml must load back as its own project"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// A failed save removes its temp file.
#[test]
fn a_failed_save_removes_its_temp_file() {
    let dir = test_dir("failed_save");
    let blocked = dir.join("project.toml");
    std::fs::create_dir(&blocked).expect("create the blocking directory");
    // Non-vacuity floor: a rename onto a directory is what fails here.
    assert!(blocked.is_dir(), "the destination is a directory");

    let mut session = ProjectSession::new_empty();
    session.set_name("Failed Save".to_owned());

    let saved = session.save(&blocked);
    assert!(
        saved.is_err(),
        "a rename onto a directory must fail; got {saved:?}"
    );

    let leftovers = leftover_temp_files(&dir);
    assert!(
        leftovers.is_empty(),
        "a failed save must remove its temp file; found {leftovers:?}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
