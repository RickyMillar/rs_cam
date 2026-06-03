//! Literature Matrix Validation Suite — Phase 0 scaffold.
//!
//! See `planning/feeds_literature_matrix_2026-06-03.md` for the full plan.
//! This harness loads cell TOMLs from `tests/literature_matrix/cells.toml`,
//! runs each cell through the engine adapter shim, and emits a structured
//! verdict report (text + JSON) to stdout.
//!
//! Skip-friendly: this integration test is gated behind
//! `--test literature_matrix`; it is NOT pulled in by
//! `cargo test -p rs_cam_core --lib`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::needless_pass_by_value,
    dead_code
)]

#[path = "literature_matrix/cell.rs"]
mod cell;
#[path = "literature_matrix/expr.rs"]
mod expr;
#[path = "literature_matrix/freshness.rs"]
mod freshness;
#[path = "literature_matrix/invariant.rs"]
mod invariant;
#[path = "literature_matrix/report.rs"]
mod report;
#[path = "literature_matrix/runner.rs"]
mod runner;
#[path = "literature_matrix/shim.rs"]
mod shim;
#[path = "literature_matrix/verdict.rs"]
mod verdict;

#[path = "literature_matrix/harness_tests.rs"]
mod harness_tests;

#[test]
fn run_literature_matrix() {
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let cells_path = manifest_dir.join("tests/literature_matrix/cells.toml");
    let sources_path = manifest_dir.join("tests/literature_matrix/sources.toml");
    runner::run(&cells_path, &sources_path);
}
