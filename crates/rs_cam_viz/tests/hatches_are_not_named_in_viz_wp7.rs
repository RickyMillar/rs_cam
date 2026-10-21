//! WP7 arm 2, the viz half — no line in `rs_cam_viz` names a session
//! mutation hatch.
//!
//! W14a (F171) moved this arm out of
//! `rs_cam_core/tests/hatches_are_crate_private_wp7.rs`: a core test that
//! scans viz source runs in no viz gate, so a viz change could break it
//! with the viz gate green. The core file keeps arm 1 (the declarations),
//! arm 3, and arm 2 over the core tests, benches and the CLI.
//!
//! The nine hatches are `pub(crate)` or deleted, so a viz call to one
//! does not compile. This scan names the call site and the reason in one
//! line, where a compile error names only the visibility.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::{Path, PathBuf};

/// The nine accessors WP7 closed (the list of the core arm).
const HATCHES: &[&str] = &[
    "stock_mut",
    "machine_mut",
    "tools_mut",
    "models_mut",
    "post_mut",
    "setups_mut",
    "find_setup_by_id_mut",
    "find_toolpath_config_by_id_mut",
    "toolpath_configs_mut",
];

/// Files that carry a hatch name as scanner data, not as a call.
const SCANNER_FILES: &[&str] = &[
    "egui_draw_sites_write_through_commands_wp6.rs",
    "non_egui_sites_write_through_commands_wp6b.rs",
    "hatches_are_not_named_in_viz_wp7.rs",
];

/// The lowest number of files the walk must visit, the bar of the core
/// arm.
const MIN_FILES_WALKED: usize = 100;

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

fn is_scanner(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| SCANNER_FILES.contains(&name))
}

#[test]
fn no_viz_source_names_a_mutation_hatch_wp7() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    for relative in ["src", "tests"] {
        let dir = root.join(relative);
        assert!(
            dir.is_dir(),
            "scanned root {} no longer exists",
            dir.display()
        );
        collect_rs(&dir, &mut files);
    }
    files.sort();
    assert!(
        files.len() >= MIN_FILES_WALKED,
        "the walk must visit at least {MIN_FILES_WALKED} files, or the arm \
         passes on a tree it never read. I read {}",
        files.len()
    );
    // Non-vacuity of the needle: a scanner file holds a hatch name, and
    // the matcher sees it there.
    let wp6 = root.join("tests/egui_draw_sites_write_through_commands_wp6.rs");
    let wp6_text = std::fs::read_to_string(&wp6).unwrap();
    assert!(
        HATCHES.iter().any(|h| wp6_text.contains(&format!(".{h}("))),
        "non-vacuity: the matcher finds no hatch in the WP6 scanner's own \
         synthetic source, so it would find none anywhere"
    );

    let mut hits: Vec<String> = Vec::new();
    for path in &files {
        if is_scanner(path) {
            continue;
        }
        let text = std::fs::read_to_string(path).unwrap();
        for (number, raw) in text.lines().enumerate() {
            if raw.trim_start().starts_with("//") {
                continue;
            }
            for hatch in HATCHES {
                if raw.contains(&format!(".{hatch}(")) {
                    hits.push(format!("{}:{}: {}", path.display(), number + 1, raw.trim()));
                }
            }
        }
    }
    assert!(
        hits.is_empty(),
        "WP7: the nine mutation hatches are crate-private. A viz surface \
         mutates a session through `ProjectSession::apply` with the matching \
         `Command` row. I read {} call site(s):\n  {}",
        hits.len(),
        hits.join("\n  ")
    );
}
