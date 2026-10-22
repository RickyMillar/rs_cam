//! F155 (W14a), the viz half — a viz test that runs reads no `planning/`
//! file and no home-directory path.
//!
//! Before W14a, `controller/tests/undo.rs` loaded
//! `planning/rivmap_block_job/testpiece_mini.toml`, whose models sat under
//! `/home/ricky/cnc_jobs`. The project now has a dated copy in the core
//! fixtures with relative model paths.
//!
//! The scan is the core one (`rs_cam_core/tests/common/source_scan.rs`):
//! a string literal that names `planning/` or `/home/` is refused when a
//! running test (a `#[test]` with no `#[ignore`) reaches it inside its file.
//! It reads `tests/` and every `src/` file (the `#[cfg(test)]` modules).
//! The core half is `rs_cam_core/tests/gate_tests_read_no_operator_paths_f155.rs`.

#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

#[path = "../../rs_cam_core/tests/common/source_scan.rs"]
mod source_scan;

use std::path::Path;

#[test]
fn a_running_viz_test_reads_no_operator_path_f155() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = source_scan::rust_files(&crate_root.join("tests"));
    files.extend(source_scan::rust_files(&crate_root.join("src")));

    let mut refused = Vec::new();
    let mut reads_the_fixture_copy = false;
    for path in &files {
        let rel = path
            .strip_prefix(crate_root)
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let src = std::fs::read_to_string(path).unwrap();
        if rel.ends_with("controller/tests/undo.rs") {
            reads_the_fixture_copy = src.contains("tests/fixtures/testpiece_mini_");
        }
        for hit in source_scan::operator_paths_in_running_tests(&src, false) {
            refused.push(format!("{rel}: {hit}"));
        }
    }
    // Non-vacuity: the scan read the file that held the defect, and that
    // file now reads the fixture copy.
    assert!(
        files.len() > 100 && reads_the_fixture_copy,
        "non-vacuity: the scan read {} files and did not see \
         src/controller/tests/undo.rs read the testpiece_mini fixture copy",
        files.len()
    );
    // The needle itself: the defect as it stood before W14a is refused.
    let before = "#[test]\nfn the_mini_project_keeps_one_pin_drill_r1() {\n    \
                  let p = \"../../planning/rivmap_block_job/testpiece_mini.toml\";\n}\n";
    assert_eq!(
        source_scan::operator_paths_in_running_tests(before, false).len(),
        1,
        "the scan must refuse the pre-W14a undo.rs read"
    );
    assert!(
        refused.is_empty(),
        "a running viz test reads an operator work path. Copy the file into \
         rs_cam_core/tests/fixtures/<name>_<date>_<hash>/ with relative paths, or mark \
         the test #[ignore = \"evidence: ..\"]:\n{}",
        refused.join("\n")
    );
}
