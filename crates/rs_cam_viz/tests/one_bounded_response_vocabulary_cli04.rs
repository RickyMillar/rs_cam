//! CLI-04 — one bounded-response vocabulary on the MCP read surface.
//!
//! Audit: `planning/design_audit_2026-09-17/cli-mcp.md`, finding CLI-04.
//!
//! `rs_cam_mcp::response` exists because one unfiltered `get_cut_trace`
//! emitted a 60,517,035-byte frame on 2026-08-08 and no client received
//! it. It carries the cap, the byte backstop and the truncation
//! vocabulary a shortened array must report itself with.
//!
//! The audit found `inspect_spans` half-wired to it: the summary branch
//! read `DEFAULT_MAX_TOP_LEVEL_SPANS` from that crate and then wrote the
//! four truncation keys by hand, and the detail branch thirty lines
//! below capped at the bare literal `50` — a magic number with no name,
//! no home and no relation to the constants next to it.
//!
//! This scan measures the rule that finding states: an MCP read that
//! bounds an array takes the bound from `rs_cam_mcp::response`, and no
//! handler writes a cap as a number in its own source.
//!
//! # NOT MEASURED
//!
//! Whether an array is bounded at all. A handler that emits an uncapped
//! array names no cap, so this scan cannot see it. The census in the
//! finding (223 `json!(` sites across `app/mcp/`) is the answer to that
//! question, and it is a reading task, not a test.
//!
//! Red before the fix: scan 1 reports `diagnostics.rs` for
//! `max_spans.unwrap_or(50)`; scan 2 reports it for a `top_level_`
//! truncation key written by hand.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::{Path, PathBuf};

/// The truncation keys `CappedArray::insert_keys` owns. A handler that
/// writes one of these as a string literal is spelling the shared
/// vocabulary a second time.
const OWNED_SUFFIXES: &[&str] = &["_total_matching", "_returned", "_truncated", "_cap"];

/// Every `.rs` file of the embedded MCP surface.
fn mcp_sources() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/app/mcp");
    assert!(
        root.is_dir(),
        "the MCP surface moved: {} no longer exists",
        root.display()
    );
    let mut out: Vec<PathBuf> = std::fs::read_dir(&root)
        .expect("the MCP surface directory must be readable")
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "rs"))
        // `tests.rs` asserts the wire keys. Reading a key is not
        // spelling a second vocabulary for it.
        .filter(|path| path.file_name().is_some_and(|name| name != "tests.rs"))
        .collect();
    out.sort();
    assert!(!out.is_empty(), "the scan found no MCP source to read");
    out
}

/// Strip `//` line comments so a sentence about a cap is not a cap.
fn code_only(source: &str) -> String {
    source
        .lines()
        .map(|line| match line.find("//") {
            Some(at) => &line[..at],
            None => line,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Scan 1 — no handler writes an array cap as a bare number.
///
/// The cap of a bounded read is a ruled value with a documented
/// rationale. Written as a literal in a handler it is a magic number
/// that the next reader cannot check and the sibling branch of the same
/// tool cannot share.
#[test]
fn no_mcp_handler_writes_a_cap_as_a_bare_number() {
    let mut offenders = Vec::new();
    for path in mcp_sources() {
        let source = code_only(&std::fs::read_to_string(&path).unwrap());
        for (index, line) in source.lines().enumerate() {
            let Some(at) = line.find("unwrap_or(") else {
                continue;
            };
            let argument = &line[at + "unwrap_or(".len()..];
            let is_number = argument.chars().next().is_some_and(|c| c.is_ascii_digit());
            let names_a_cap = line.contains("cap") || line.contains("max_");
            if is_number && names_a_cap {
                offenders.push(format!("{}:{}: {}", path.display(), index + 1, line.trim()));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "an MCP read caps an array with a bare number. Name the cap in \
         `rs_cam_mcp::response` and read it from there:\n{}",
        offenders.join("\n")
    );
}

/// Scan 2 — the truncation vocabulary comes from `CappedArray`.
///
/// `<name>_total_matching`, `<name>_returned`, `<name>_truncated` and
/// `<name>_cap` are `insert_keys`' four keys. A handler that writes one
/// as a literal has copied the vocabulary, and a copy drifts: that is
/// how the summary branch of `inspect_spans` came to report a cap it
/// read from one crate with keys it spelled in another.
#[test]
fn the_truncation_keys_come_from_the_shared_primitive() {
    let mut offenders = Vec::new();
    for path in mcp_sources() {
        let source = code_only(&std::fs::read_to_string(&path).unwrap());
        for (index, line) in source.lines().enumerate() {
            for suffix in OWNED_SUFFIXES {
                // A prefixed key, e.g. "top_level_truncated". The bare
                // key is the pre-primitive name `inspect_spans`' detail
                // mode still ships; scan 1 and the follow-up in the
                // CLI-04 commit body cover that one.
                let literal = format!("{suffix}\"");
                if let Some(at) = line.find(&literal)
                    && line[..at].ends_with(|c: char| c.is_ascii_alphanumeric())
                {
                    offenders.push(format!("{}:{}: {}", path.display(), index + 1, line.trim()));
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "an MCP read spells a `CappedArray` truncation key by hand. Call \
         `CappedArray::insert_keys` instead:\n{}",
        offenders.join("\n")
    );
}
