//! WP12 sentry — the loose executor is crate-private, and nothing outside
//! `crates/rs_cam_core/src/` names it.
//!
//! `compute/execute.rs` published three generation entries:
//! `execute_operation` (14 arguments), `execute_operation_annotated` (16)
//! and `execute_operation_annotated_with_regions` (21). Each one took the
//! whole input list as loose arguments, so each one was a second input
//! assembly beside `session::compute::resolve_generation_inputs`. WP11b made
//! `execute_generation` the narrow door over the resolved bundle. WP12 shut
//! the loose door: the three entries went `pub(crate)`, and the four
//! integration tests that called them moved in-crate.
//!
//! CMP-02 (2026-09-18) went further: there is ONE entry now,
//! `execute_operation_annotated`, and it takes `&ExecutionContext` rather
//! than a positional list. The two forwarding wrappers are deleted. This
//! sentry guards the same door, over one declaration instead of three.
//!
//! The plan's ruling is `IMPLEMENTATION_PLAN.md` §23 rulings 1, 2 and 5.
//!
//! # The three arms
//!
//! * (a) No file outside `crates/rs_cam_core/src/` names `execute_operation`.
//!   The scan reads `crates/*/tests`, `crates/*/benches`,
//!   `crates/rs_cam_viz/src` and `crates/rs_cam_cli/src`. Comment lines are
//!   excluded: several comments name the entry to explain the history, and a
//!   comment is not a call.
//! * (b) Every `execute_operation*` declaration in `compute/execute.rs`
//!   reads `pub(crate) fn`, and no line there reads `pub fn
//!   execute_operation`.
//! * (c) No line under `crates/rs_cam_viz/src` names `session/compute.rs`
//!   except the two the allowlist names. WP11b deleted the mirrored input
//!   assembly, so a viz file that still describes itself against that file
//!   describes a shape the tree no longer has. **Comment lines are
//!   included**, and that is the whole arm: a file path appears in Rust
//!   only inside a comment.
//!
//! # Red before the fix
//!
//! Arm (a) fails: four integration files call the loose entry. Arm (b)
//! fails: the declaration reads `pub fn`.
//!
//! Arm (c) was VACUOUS and passed (tech-debt review H4). It skipped comment
//! lines, so it could match nothing: the occurrence it was written for was a
//! `//!` line, and the fix commit's own red output recorded the arm as `ok`
//! on the pre-fix tree. WP18 makes it read every line and allow the two
//! legitimate cross-references by name. Its red is an injected `//` line
//! that names the module from a viz file; the verifier ran that experiment.
//!
//! WP18 also WIDENED the arm's population, and the reason is worth stating.
//! The ruling asks for an allowlist of the surviving legitimate references.
//! Both of them (`app/mcp.rs`, `ui/properties/mod.rs`) live OUTSIDE
//! `crates/rs_cam_viz/src/compute`, so a scan of that directory alone can
//! never see them and the staleness check below could never pass. The arm
//! therefore reads all of `crates/rs_cam_viz/src`, which covers the
//! `compute` directory the finding named and holds the allowlist to
//! account.
//!
//! # Non-vacuity
//!
//! A source scan that reads no file passes and looks healthy. Each arm
//! asserts that its population is not empty, and arm (b) asserts that it
//! found every declaration by name. Arm (c) asserts that every
//! allowlist entry still matches a line: an entry that no longer describes
//! the tree is an allowance nothing checks.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::{Path, PathBuf};

/// The token every generation entry's name starts with.
const ENTRY: &str = "execute_operation";

/// The declarations, longest name first so that a prefix match cannot claim
/// a longer sibling. CMP-02 left exactly one.
const DECLARATIONS: &[&str] = &["execute_operation_annotated"];

/// The two files that SCAN for the entry name instead of calling it. Both
/// hold the name in a string literal, which is not a comment and not a call.
const SCANNERS: &[&str] = &[
    "loose_executor_is_crate_private_wp12.rs",
    "gen_inputs_one_assembly_n12.rs",
];

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

/// The workspace's `crates/` directory.
fn crates_root() -> PathBuf {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    assert!(
        root.is_dir(),
        "{} is not a directory; retarget this test",
        root.display()
    );
    root
}

/// Is this file one of the two scanners?
fn is_scanner(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| SCANNERS.contains(&name))
}

/// Every `.rs` file that arm (a) reads, in a stable order.
///
/// The population is every test and bench target in the workspace, plus the
/// two consumer crates' own sources. Core's `src/` is deliberately absent:
/// that is where the entry lives and where its own unit tests call it.
fn scanned_sources() -> Vec<PathBuf> {
    let crates = crates_root();
    let mut roots: Vec<PathBuf> = Vec::new();

    let listing = std::fs::read_dir(&crates);
    let listing = listing.unwrap_or_else(|e| panic!("read dir {}: {e}", crates.display()));
    let mut members: Vec<PathBuf> = Vec::new();
    for entry in listing {
        let entry = entry.unwrap_or_else(|e| panic!("entry in {}: {e}", crates.display()));
        let path = entry.path();
        if path.is_dir() {
            members.push(path);
        }
    }
    members.sort();
    for member in &members {
        for leaf in ["tests", "benches"] {
            let dir = member.join(leaf);
            if dir.is_dir() {
                roots.push(dir);
            }
        }
    }

    for consumer in ["rs_cam_viz", "rs_cam_cli"] {
        let dir = crates.join(consumer).join("src");
        assert!(
            dir.is_dir(),
            "{} is not a directory; retarget this test",
            dir.display()
        );
        roots.push(dir);
    }

    let mut files = Vec::new();
    for root in &roots {
        rust_sources_under(root, &mut files);
    }
    files.retain(|path| !is_scanner(path));
    files.sort();
    files
}

// ── (a) nothing outside core's `src/` names the loose entry ─────────

/// The loose executor is named nowhere outside `crates/rs_cam_core/src/`.
///
/// A caller outside core's `src/` cannot reach the entry once it is
/// `pub(crate)`, so a name on a live line there is either a compile error or
/// evidence that the demotion came undone.
#[test]
fn no_file_outside_core_src_names_the_loose_executor() {
    let files = scanned_sources();
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
        "every caller outside core reaches generation through \
         `rs_cam_core::session::execute_job`, never the loose entry; these \
         lines name it: {}",
        hits.join(", ")
    );
}

// ── (b) the declarations are crate-private ──────────────────────────

/// The `execute_operation*` declaration reads `pub(crate) fn`.
///
/// Arm (a) proves that nothing outside core names the entry today. This arm
/// proves that the compiler holds the line tomorrow: a `pub fn` would let a
/// new caller back in without any test going red.
#[test]
fn the_generation_entry_is_crate_private() {
    let core_src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let path = core_src.join("compute").join("execute.rs");
    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));

    let mut public: Vec<String> = Vec::new();
    let mut found: Vec<&str> = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("pub fn execute_operation") {
            public.push(trimmed.to_owned());
            continue;
        }
        for name in DECLARATIONS {
            let wanted = format!("pub(crate) fn {name}(");
            if trimmed.starts_with(&wanted) && !found.contains(name) {
                found.push(name);
                break;
            }
        }
    }

    assert!(
        public.is_empty(),
        "a generation entry is still public, so the loose door is still \
         open: {}",
        public.join(" | ")
    );
    assert_eq!(
        found.len(),
        DECLARATIONS.len(),
        "expected {} crate-private declarations in {}, found {found:?}; a \
         renamed entry makes this sentry vacuous",
        DECLARATIONS.len(),
        path.display()
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
    let root = crates_root().join("rs_cam_viz").join("src");
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
