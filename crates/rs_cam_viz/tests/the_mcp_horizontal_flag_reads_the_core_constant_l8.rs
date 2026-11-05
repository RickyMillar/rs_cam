//! L8 (architecture review 2026-11-05): the MCP `is_horizontal` flag of a
//! face reads the core constant.
//!
//! # The claim
//!
//! The core refuses a face whose normal Z is under
//! `HORIZONTAL_FACE_MIN_NORMAL_Z` (`selection::view`, `flat_regions`,
//! `enriched_mesh`), so a face is horizontal at `>=` that value. The MCP
//! face report (`app/mcp/project.rs`) wrote `> 0.95`, so the two disagreed
//! at exactly 0.95. This source scan holds:
//!
//! 1. The `is_horizontal` insert reads `>= HORIZONTAL_FACE_MIN_NORMAL_Z`.
//! 2. No code line of the file holds the literal `0.95`.
//!
//! The scan skips comment lines. The anchor (`"is_horizontal"`) keeps the
//! scan from a pass on an empty or moved file.
//!
//! ```text
//! cargo test -p rs_cam_viz -q --test the_mcp_horizontal_flag_reads_the_core_constant_l8
//! ```

#![expect(
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::path::PathBuf;

fn code_lines() -> Vec<String> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/app/mcp/project.rs");
    let text = std::fs::read_to_string(&path).expect("app/mcp/project.rs reads");
    text.lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .map(str::to_owned)
        .collect()
}

#[test]
fn the_mcp_horizontal_flag_reads_the_core_constant_l8() {
    let lines = code_lines();
    let anchor = lines
        .iter()
        .position(|line| line.contains("\"is_horizontal\""))
        .expect("the anchor: the MCP face report inserts `is_horizontal`");
    let insert: String = lines
        .iter()
        .skip(anchor)
        .take(3)
        .map(String::as_str)
        .collect();
    assert!(
        insert.contains(">= HORIZONTAL_FACE_MIN_NORMAL_Z"),
        "`is_horizontal` must read `>= HORIZONTAL_FACE_MIN_NORMAL_Z`, found: {insert}"
    );
    let literals: Vec<&String> = lines.iter().filter(|line| line.contains("0.95")).collect();
    assert!(
        literals.is_empty(),
        "app/mcp/project.rs holds a 0.95 literal: {literals:?}"
    );
}
