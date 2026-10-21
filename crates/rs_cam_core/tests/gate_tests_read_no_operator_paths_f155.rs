//! F155 (W14a) — a core test that runs reads no `planning/` file and no
//! home-directory path.
//!
//! `tests/CLAUDE.md`: a project or baseline a test loads lives in
//! `tests/fixtures/`, not in `planning/`. Before W14a, gate tests read the
//! agent smoke case list, a feeds golden and two DXFs from `planning/`, and
//! wrote render artifacts into it; the viz pin test loaded a planning
//! project whose models sat under `/home/ricky`. An operator edit of a
//! planning file then moved a gate, and the gate failed on any other
//! machine.
//!
//! The rule: a string literal that names `planning/` (or the folder
//! `"planning"`) or `/home/` is refused when a running test reaches it (a
//! `#[test]` with no `#[ignore`, through the items it names in the same
//! file). Every literal in `tests/common/` is refused, because any test can
//! call a common helper. A test that needs a large operator file is
//! `#[ignore = "evidence: .."]`.
//!
//! The viz half is `crates/rs_cam_viz/tests/gate_tests_read_no_operator_paths_f155.rs`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

#[path = "common/source_scan.rs"]
mod source_scan;

use std::path::Path;

/// The scan's own files hold the needles as data: the matcher and this
/// sentry (its planted source sits in a raw string the item reader does
/// not parse as a string).
const SCANNERS: &[&str] = &[
    "common/source_scan.rs",
    "gate_tests_read_no_operator_paths_f155.rs",
];

/// A running test that loads a planning file, an ignored one that does,
/// and a running one that only checks a text holds no planning path.
const PLANTED: &str = r#"
const PROJECT: &str = "planning/fixtures/x.toml";

fn load() -> String {
    std::fs::read_to_string(PROJECT).unwrap()
}

#[test]
fn reads_a_planning_file() {
    let _ = load();
}

#[test]
#[ignore = "evidence: reads the operator mesh"]
fn reads_a_home_file() {
    let _ = std::fs::read("/home/someone/mesh.stl");
}

#[test]
fn says_no_planning_path() {
    // "planning/" in a comment is not a read.
    assert!(!"text".contains("planning/"));
}
"#;

#[test]
fn the_scan_refuses_a_reached_literal_and_passes_the_rest() {
    let hits = source_scan::operator_paths_in_running_tests(PLANTED, false);
    assert_eq!(
        hits,
        vec![r#"PROJECT "planning/fixtures/x.toml""#.to_owned()],
        "only the constant that a running test reaches is refused"
    );
    let whole = source_scan::operator_paths_in_running_tests(PLANTED, true);
    assert_eq!(
        whole.len(),
        2,
        "a common file refuses every literal: {whole:?}"
    );
}

#[test]
fn a_running_core_test_reads_no_operator_path_f155() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let files = source_scan::rust_files(&root);
    let mut refused = Vec::new();
    let mut ignored_reads = 0usize;
    for path in &files {
        let rel = path
            .strip_prefix(&root)
            .unwrap()
            .to_string_lossy()
            .into_owned();
        if SCANNERS.contains(&rel.as_str()) {
            continue;
        }
        let src = std::fs::read_to_string(path).unwrap();
        let in_common = rel.starts_with("common/");
        for hit in source_scan::operator_paths_in_running_tests(&src, in_common) {
            refused.push(format!("tests/{rel}: {hit}"));
        }
        // Non-vacuity: the same needle, read over every item.
        ignored_reads += source_scan::operator_paths_in_running_tests(&src, true).len();
    }
    assert!(
        ignored_reads > 0,
        "non-vacuity: the scan of {} files saw no `planning/` or `/home/` literal at \
         all, so it reads nothing (the ignored evidence runs hold many)",
        files.len()
    );
    assert!(
        refused.is_empty(),
        "a running test reads an operator work path. Copy the file into \
         tests/fixtures/<name>_<date>_<hash>/ with relative paths, or write output under \
         env!(\"CARGO_TARGET_TMPDIR\"), or mark the test #[ignore = \"evidence: ..\"] when \
         it needs a large operator file:\n{}",
        refused.join("\n")
    );
}
