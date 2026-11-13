//! Feeds contract P29-last
//! (`planning/feeds_contract_2026-11-03/phases/P29-11_DESIGN.md` section 5,
//! lead rulings L3 and L7): the CLI reads no op type for a limit.
//!
//! The CLI twin of `crates/rs_cam_core/tests/no_limit_reads_the_op_type_p29_last.rs`
//! (that file states the rule and the classes). Every source file of the CLI
//! is guarded. Its reads are op parsing (class W) and labels (class T); no
//! CLI file computes a limit, so no file has zero tolerance.
//!
//! `cargo test -p rs_cam_cli -q --test no_limit_reads_the_op_type_p29_last`

#![expect(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure; the shared source scans index their own buffers"
)]

#[path = "../../rs_cam_core/tests/common/source_scan.rs"]
mod source_scan;

#[path = "../../rs_cam_core/tests/common/op_type_scan.rs"]
mod op_type_scan;

use std::path::{Path, PathBuf};

use op_type_scan::{Allow, Class};

/// Every source file of the CLI is guarded.
fn guarded(_rel: &str) -> bool {
    true
}

/// No CLI file computes a limit.
fn zero_tolerance(_rel: &str) -> bool {
    false
}

/// The allow-list as it stands after commits L-1 and L-2 (no X entry). The
/// list only shrinks.
#[rustfmt::skip]
const ALLOW: &[Allow] = &[
    // W: op parsing of the job file and the command line.
    Allow { file: "job.rs", function: "batch_selection", class: Class::W },
    Allow { file: "job.rs", function: "operation_type", class: Class::W },
    Allow { file: "run.rs", function: "parse_op_type", class: Class::W },
    Allow { file: "smoke.rs", function: "materialize_case_toolpath", class: Class::W },
    Allow { file: "smoke.rs", function: "parse_op_type", class: Class::W },
    // T: a label or a report field.
    Allow { file: "run.rs", function: "print_ops", class: Class::T },
    Allow { file: "run.rs", function: "print_params", class: Class::T },
    Allow { file: "smoke.rs", function: "build_ok_row", class: Class::T },
    Allow { file: "suggest.rs", function: "peck_depth", class: Class::T },
];

/// The sum of the X counts: none in the CLI. It only goes down.
const X_TOTAL: usize = 0;

fn src_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

#[test]
fn no_surface_reads_the_op_type_p29_last() {
    let table = op_type_scan::counts(&src_root(), guarded);
    if let Ok(out) = std::env::var("P29_GUARD_TABLE") {
        let _ = std::fs::write(out, op_type_scan::table_text(&table));
    }
    let files = source_scan::rust_files(&src_root())
        .iter()
        .filter(|p| {
            p.strip_prefix(src_root())
                .is_ok_and(|r| guarded(&r.to_string_lossy().replace('\\', "/")))
        })
        .count();
    assert!(
        files > 3,
        "non-vacuity: the scan reads {files} guarded files"
    );
    let failures = op_type_scan::failures(&table, ALLOW, zero_tolerance, X_TOTAL);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Non-vacuity: a planted read in a fixture string is found.
#[test]
fn the_scan_finds_a_planted_read_p29_last() {
    let planted =
        "fn label(op: &OperationConfig) -> bool {\n    op.op_type() == OperationType::Drill\n}\n";
    let hits = op_type_scan::hits(planted);
    if hits.len() != 2 || hits.iter().any(|h| h.function != "label") {
        panic!("the scan finds two reads in `label`: {hits:?}");
    }
}
