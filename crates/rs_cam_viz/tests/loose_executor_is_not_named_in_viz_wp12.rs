//! WP12 and N12, the viz half — no viz file names core's loose executor,
//! and no viz file mirrors `session/compute.rs`.
//!
//! W14a (F171) moved these arms out of the core tests, where no viz gate
//! ran them:
//!
//! - `rs_cam_core/tests/loose_executor_is_crate_private_wp12.rs` arm (a)
//!   over viz `src/` and `tests/`, which also covers
//!   `rs_cam_core/tests/gen_inputs_one_assembly_n12.rs` arm (b) (no viz
//!   source names `execute_operation_annotated`);
//! - the same file's arm (c), moved whole.
//!
//! The executor is `pub(crate)`, so a viz call does not compile; arm (a)
//! also refuses a NAME of it on a code line (a second assembly that
//! re-implements it under its name). Arm (c) reads comment lines on purpose:
//! a file path appears in Rust only inside a comment.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::{Path, PathBuf};

/// The token every generation entry's name starts with.
const ENTRY: &str = "execute_operation";

/// The file that scans for the entry name instead of calling it.
const SCANNERS: &[&str] = &["loose_executor_is_not_named_in_viz_wp12.rs"];

/// A line comment, a doc comment, or a module doc comment.
fn is_comment(line: &str) -> bool {
    line.trim_start().starts_with("//")
}

/// Collect every `.rs` file under `dir`, recursively.
fn rust_sources_under(dir: &Path, out: &mut Vec<PathBuf>) {
    let listing = std::fs::read_dir(dir);
    let listing = listing.unwrap_or_else(|e| panic!("read dir {}: {e}", dir.display()));
    for entry in listing {
        let entry = entry.unwrap_or_else(|e| panic!("entry in {}: {e}", dir.display()));
        let path = entry.path();
        if path.is_dir() {
            rust_sources_under(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

// ── (a) no viz file names the loose entry ───────────────────────────

/// The loose executor is named nowhere in `rs_cam_viz`.
#[test]
fn no_viz_file_names_the_loose_executor() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    for leaf in ["src", "tests"] {
        rust_sources_under(&root.join(leaf), &mut files);
    }
    files.retain(|path| {
        !path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| SCANNERS.contains(&name))
    });
    files.sort();
    assert!(
        files.len() > 20,
        "the scan read only {} files; an empty or tiny population passes \
         and looks healthy",
        files.len()
    );

    let mut hits: Vec<String> = Vec::new();
    for path in &files {
        let text = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        for (index, line) in text.lines().enumerate() {
            if is_comment(line) {
                continue;
            }
            if line.contains(ENTRY) {
                hits.push(format!("{}:{}", path.display(), index + 1));
            }
        }
    }

    assert!(
        hits.is_empty(),
        "the GUI worker reaches generation through \
         `rs_cam_core::session::execute_job`, never the loose entry; these \
         lines name it: {}",
        hits.join(", ")
    );
}

// ── (c) no viz source mirrors `session/compute.rs` ──────────────────

/// The module a viz file must not describe itself against.
const CORE_GENERATION_MODULE: &str = "session/compute.rs";

/// One legitimate cross-reference to core's generation module.
///
/// A line passes only when an entry matches BOTH halves: `path_marker`
/// against the file's path, and `line_marker` against the line itself. The
/// path half is a fragment, not a full name, so a file that moves into a
/// directory of its own keeps its allowance.
struct AllowedReference {
    /// A fragment of the file's path.
    path_marker: &'static str,
    /// A fragment of the line, chosen to name the reference's PURPOSE.
    line_marker: &'static str,
    /// Why this reference is legitimate.
    why: &'static str,
}

/// Every line under `crates/rs_cam_viz/src` that may name core's generation
/// module. Both entries cite core's BEHAVIOUR to explain the viz side;
/// neither describes a mirrored input assembly.
const ALLOWED_REFERENCES: &[AllowedReference] = &[
    AllowedReference {
        path_marker: "app/mcp",
        line_marker: "SessionError::ToolNotFound",
        why: "B7 divergence 3: the GUI narration's doc comment cites core's \
              sibling narration, which refuses an unresolved tool id rather \
              than narrating with another tool's geometry",
    },
    AllowedReference {
        // A path FRAGMENT, not a file name: `81493c1a` split
        // `ui/properties/mod.rs` and this line moved to its
        // `toolpath_panel.rs` child, which left the arm red from
        // 2026-09-17 until CMP-02 read it. The fragment is the directory
        // now, so the next split inside it keeps the allowance.
        path_marker: "ui/properties",
        line_marker: "one generation uses",
        why: "UX-R03-009 (G-BOUNDARYINHERIT): the boundary panel names the \
              stored source generation reads, because the core door clones \
              `tc.boundary` unconditionally",
    },
];

/// The allowlist entry that allows this line, if any.
fn allowance_for(path: &Path, line: &str) -> Option<usize> {
    let shown = path.display().to_string();
    ALLOWED_REFERENCES.iter().position(|allowed| {
        shown.contains(allowed.path_marker) && line.contains(allowed.line_marker)
    })
}

/// No viz line names `session/compute.rs` outside the allowlist.
///
/// The GUI worker runs core's `Job` steps since WP11b. A viz file that
/// describes itself against core's generation module describes a mirror that
/// no longer exists.
///
/// H4: this arm used to skip comment lines. A file path appears in Rust only
/// inside a comment, so the arm could match nothing and passed on the very
/// tree it was written to refuse. It reads every line now, and the two
/// surviving cross-references are named above with the reason each one
/// stands. An entry that stops matching fails this arm: a stale allowance is
/// an exemption nothing checks.
#[test]
fn viz_names_the_core_generation_module_only_where_the_allowlist_says() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    assert!(
        root.is_dir(),
        "{} is not a directory; retarget this test",
        root.display()
    );
    let mut files = Vec::new();
    rust_sources_under(&root, &mut files);
    files.sort();
    assert!(
        files.len() > 20,
        "the viz scan read only {} files; an empty or tiny population passes \
         and looks healthy",
        files.len()
    );
    assert!(
        files
            .iter()
            .any(|path| path.display().to_string().contains("/compute/")),
        "the scan must cover `crates/rs_cam_viz/src/compute`, the directory \
         H4 names; no scanned path sits under it"
    );

    let mut hits: Vec<String> = Vec::new();
    let mut matched = vec![false; ALLOWED_REFERENCES.len()];
    for path in &files {
        let text = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        for (index, line) in text.lines().enumerate() {
            if !line.contains(CORE_GENERATION_MODULE) {
                continue;
            }
            match allowance_for(path, line) {
                Some(entry) => matched[entry] = true,
                None => hits.push(format!("{}:{}", path.display(), index + 1)),
            }
        }
    }

    assert!(
        hits.is_empty(),
        "the worker runs core's Job steps; it mirrors no core module. A line \
         that names {CORE_GENERATION_MODULE} needs an entry in \
         ALLOWED_REFERENCES saying why. These lines have none: {}",
        hits.join(", ")
    );
    for (entry, allowed) in ALLOWED_REFERENCES.iter().enumerate() {
        assert!(
            matched[entry],
            "the allowlist entry ({}, {}) matches no line any more, so it \
             exempts nothing and hides the next mirror. Delete it, or \
             retarget it. It was allowed because: {}",
            allowed.path_marker, allowed.line_marker, allowed.why
        );
    }
}
