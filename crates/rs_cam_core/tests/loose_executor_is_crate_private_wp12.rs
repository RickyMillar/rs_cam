//! WP12 sentry — the loose executor is crate-private, and nothing outside
//! `crates/rs_cam_core/src/` names it.
//!
//! `compute/execute.rs` publishes three generation entries:
//! `execute_operation` (14 arguments), `execute_operation_annotated` (16)
//! and `execute_operation_annotated_with_regions` (21). Each one takes the
//! whole input list as loose arguments, so each one is a second input
//! assembly beside `session::compute::resolve_generation_inputs`. WP11b made
//! `execute_generation` the narrow door over the resolved bundle. WP12 shuts
//! the loose door: the three entries go `pub(crate)`, and the four
//! integration tests that called them move in-crate.
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
//! * (c) No live line under `crates/rs_cam_viz/src/compute` names
//!   `session/compute.rs`. WP11b deleted the mirrored input assembly, so a
//!   viz file that still describes itself against that file describes a
//!   shape the tree no longer has.
//!
//! # Red before the fix
//!
//! Arm (a) fails: four integration files call the loose entry. Arm (b)
//! fails: all three declarations read `pub fn`.
//!
//! # Non-vacuity
//!
//! A source scan that reads no file passes and looks healthy. Each arm
//! asserts that its population is not empty, and arm (b) asserts that it
//! found all three declarations by name.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::{Path, PathBuf};

/// The token every generation entry's name starts with.
const ENTRY: &str = "execute_operation";

/// The three declarations, longest name first so that a prefix match cannot
/// claim a longer sibling.
const DECLARATIONS: &[&str] = &[
    "execute_operation_annotated_with_regions",
    "execute_operation_annotated",
    "execute_operation",
];

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

/// Every `execute_operation*` declaration reads `pub(crate) fn`.
///
/// Arm (a) proves that nothing outside core names the entry today. This arm
/// proves that the compiler holds the line tomorrow: a `pub fn` would let a
/// new caller back in without any test going red.
#[test]
fn the_three_generation_entries_are_crate_private() {
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

// ── (c) no viz compute source mirrors `session/compute.rs` ──────────

/// No live line under `crates/rs_cam_viz/src/compute` names
/// `session/compute.rs`.
///
/// The GUI worker runs core's `Job` steps since WP11b. A viz file that
/// describes itself against core's generation module describes a mirror that
/// no longer exists. The two legitimate cross-references live outside this
/// directory (`ui/properties/mod.rs`, `app/mcp.rs`) and are untouched.
#[test]
fn no_viz_compute_source_mirrors_the_core_generation_module() {
    let viz_src = crates_root().join("rs_cam_viz").join("src");
    let root = viz_src.join("compute");
    assert!(
        root.is_dir(),
        "{} is not a directory; retarget this test",
        root.display()
    );
    let mut files = Vec::new();
    rust_sources_under(&root, &mut files);
    files.sort();
    assert!(
        !files.is_empty(),
        "the viz compute scan read no file; an empty population passes and \
         looks healthy"
    );

    let mut hits: Vec<String> = Vec::new();
    for path in &files {
        let text = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        for (index, line) in text.lines().enumerate() {
            if is_comment(line) {
                continue;
            }
            if line.contains("session/compute.rs") {
                hits.push(format!("{}:{}", path.display(), index + 1));
            }
        }
    }

    assert!(
        hits.is_empty(),
        "the worker runs core's Job steps; it mirrors no core module. These \
         lines still name one: {}",
        hits.join(", ")
    );
}
