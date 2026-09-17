//! Every operator-visible string is scanned for the **cargo-fmt
//! baked-indentation** defect.
//!
//! # The defect, and why it needs a machine to catch it
//!
//! A long message is naturally written as a `\`-continued literal:
//!
//! ```text
//! format!(
//!     "the grid over-states gaps by up to its floor, so the \
//!      true unreachable share is AT OR BELOW {pct:.1} %"
//! )
//! ```
//!
//! Rust's `\`-newline escape strips the newline **and** the next line's
//! leading whitespace, so that literal is correct as written. But
//! `cargo fmt` will sometimes COLLAPSE such a literal onto one line, and when
//! it does it drops the backslash and keeps the spaces. The message then
//! ships as
//!
//! ```text
//! the grid over-states gaps by up to its floor, so the              true unreachable share is AT OR BELOW 51.1 %
//! ```
//!
//! Nothing fails. Clippy is happy, the formatter is happy, the test suite is
//! happy, and the string is still perfectly readable — it just has a hole in
//! it. It reads as a rendering glitch rather than as a bug, so it survives
//! review: this shipped **twice in two days** on the reach-map surfaces
//! (P5.2's `grid_note`, then P5.3's legend), and a hand-written grep missed
//! the second because the character before the space run was a multibyte
//! `\u{00B7}` and the pattern had assumed an ASCII letter. That is the whole
//! argument for a sentry: the defect is invisible to every other gate and
//! nearly invisible to a person.
//!
//! # What is scanned, and what is not
//!
//! Every SINGLE-LINE string literal in the operator-visible surfaces. A
//! literal still spanning lines with `\` continuations is *correct* — Rust
//! strips the indentation — and this scanner cannot see one, which is the
//! right blind spot: the defect exists only once the literal has been
//! collapsed, and that is exactly when it becomes visible here.
//!
//! # The allowlist
//!
//! Some literals genuinely want aligned columns — a formula block, a
//! separator with air around it. Mark the line, or one of the three lines
//! above it, with [`ALLOW_MARKER`] and a reason. The marker is deliberately
//! ugly and deliberately requires a reason: an allowlist entry is a claim
//! that the spaces are load-bearing.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::{Path, PathBuf};

/// Mark a line (or up to three lines above it) with this to accept a run of
/// spaces as deliberate alignment. Put the reason after it.
const ALLOW_MARKER: &str = "ui-string-columns:";

/// How many lines above a hit the marker may sit — enough for a marker above a
/// `format!(` opener, not enough to shadow an unrelated literal further down.
const MARKER_LOOKBACK: usize = 3;

/// Shortest run of spaces treated as suspicious. Two spaces occur naturally
/// after a full stop; three do not occur in prose at all.
const MIN_RUN: usize = 3;

/// The operator-visible surfaces, relative to this crate's manifest.
///
/// `rs_cam_mcp/src/server.rs` is reached across the crate boundary on
/// purpose: its doc comments are the TOOL DESCRIPTIONS an agent reads, which
/// makes them an operator surface even though they are not drawn on screen.
fn scanned_paths() -> Vec<PathBuf> {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut out = Vec::new();
    collect_rs(&manifest.join("src/ui"), &mut out);
    for extra in [
        "src/app/mcp.rs",
        "src/app/mcp/diagnostics.rs",
        "src/app/mcp/generation.rs",
        "src/app/mcp/project.rs",
        "src/app/mcp/simulation.rs",
        "src/app/mcp/view.rs",
        "src/mcp_server.rs",
        "../rs_cam_mcp/src/server.rs",
    ] {
        let p = manifest.join(extra);
        assert!(p.is_file(), "scanned path {} no longer exists", p.display());
        out.push(p);
    }
    out.sort();
    out
}

fn collect_rs(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries =
        std::fs::read_dir(dir).unwrap_or_else(|e| panic!("read_dir {}: {e}", dir.display()));
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rs(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// Every single-line string literal on `line`, without its quotes.
///
/// Hand-scanned rather than regex'd so an escaped quote inside a literal
/// cannot end it early — `"a \" b"` is one literal, not two.
fn literals(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] != '"' {
            i += 1;
            continue;
        }
        let mut body = String::new();
        let mut j = i + 1;
        let mut closed = false;
        while j < chars.len() {
            match chars[j] {
                '\\' => {
                    // Skip the escape and whatever it escapes; a trailing
                    // backslash means the literal continues onto the next
                    // line, which this scanner deliberately does not follow.
                    j += 2;
                }
                '"' => {
                    closed = true;
                    break;
                }
                c => {
                    body.push(c);
                    j += 1;
                }
            }
        }
        if closed {
            out.push(body);
        }
        i = j + 1;
    }
    out
}

/// A run of `MIN_RUN`+ spaces with non-space either side — the signature of a
/// collapsed continuation. A run at the very start or end of a literal is
/// padding, not a hole, and is not reported.
fn interior_space_run(body: &str) -> Option<String> {
    let bytes: Vec<char> = body.chars().collect();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != ' ' {
            i += 1;
            continue;
        }
        let start = i;
        while i < bytes.len() && bytes[i] == ' ' {
            i += 1;
        }
        if i - start >= MIN_RUN && start > 0 && i < bytes.len() {
            let from = start.saturating_sub(18);
            let to = (i + 18).min(bytes.len());
            return Some(bytes[from..to].iter().collect());
        }
    }
    None
}

#[test]
fn no_operator_string_carries_a_collapsed_line_continuation() {
    let mut findings: Vec<String> = Vec::new();
    let mut allowed = 0usize;
    let mut scanned_files = 0usize;
    let mut scanned_literals = 0usize;

    for path in scanned_paths() {
        scanned_files += 1;
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        let lines: Vec<&str> = text.split('\n').collect();
        for (idx, line) in lines.iter().enumerate() {
            if line.trim_start().starts_with("//") {
                continue;
            }
            for body in literals(line) {
                scanned_literals += 1;
                let Some(excerpt) = interior_space_run(&body) else {
                    continue;
                };
                let lo = idx.saturating_sub(MARKER_LOOKBACK);
                let marked = lines[lo..=idx].iter().any(|l| l.contains(ALLOW_MARKER));
                if marked {
                    allowed += 1;
                    continue;
                }
                findings.push(format!(
                    "{}:{}: \u{2026}{excerpt}\u{2026}",
                    path.display(),
                    idx + 1
                ));
            }
        }
    }

    assert!(
        scanned_files >= 10 && scanned_literals > 500,
        "the scan found almost nothing ({scanned_files} files, {scanned_literals} \
         literals) — the paths or the literal parser are broken, and a sentry \
         that scans nothing passes for ever"
    );
    assert!(
        allowed > 0,
        "no allowlist marker matched. Either the marker was renamed on the \
         three literals that carry it, or the lookback is broken — and a \
         silently empty allowlist is how a real hit gets waved through later"
    );
    assert!(
        findings.is_empty(),
        "operator-visible strings carry a run of {MIN_RUN}+ interior spaces. \
         `cargo fmt` collapses a `\\`-continued literal onto one line and keeps \
         the continuation's indentation, so the message ships with a hole in \
         it. Rewrite each as ONE literal (long lines are fine; rustfmt leaves \
         a long string alone), or mark it `{ALLOW_MARKER} <reason>` if the \
         spaces are deliberate columns.\n{}",
        findings.join("\n")
    );
}
