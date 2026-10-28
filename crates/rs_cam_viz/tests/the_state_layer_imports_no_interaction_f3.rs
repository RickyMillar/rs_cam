//! F3 layering: the `state/` layer imports nothing from `interaction/`.
//!
//! # The claim
//!
//! `interaction/picking.rs` reads `state` types (the setup frame, the
//! visibility, the pick filter). An edge back from `state/` to
//! `interaction/` makes a cycle. The display-frame geometry that both the
//! picker and the camera fit need lives in `state/display_frame.rs`, so no
//! file under `src/state/` names `crate::interaction`.
//!
//! The non-vacuity anchors: `state/display_frame.rs` exists and holds
//! `selection_frame_bounds`, and `interaction/picking.rs` imports from it.

#![expect(
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::path::{Path, PathBuf};

/// Every `.rs` file under `dir`, recursively.
fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = std::fs::read_dir(dir).unwrap_or_else(|e| panic!("read {}: {e}", dir.display()));
    for entry in entries {
        let path = entry
            .unwrap_or_else(|e| panic!("entry of {}: {e}", dir.display()))
            .path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|x| x == "rs") {
            out.push(path);
        }
    }
}

/// The text of `path` with every `//` comment removed, so a comment that
/// names a module is not an import.
fn code(path: &Path) -> String {
    let text =
        std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    text.lines()
        .map(|line| line.split("//").next().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn the_state_layer_imports_no_interaction_f3() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let display_frame = root.join("state/display_frame.rs");
    assert!(
        code(&display_frame).contains("pub fn selection_frame_bounds("),
        "non-vacuity: state/display_frame.rs no longer holds selection_frame_bounds"
    );
    assert!(
        code(&root.join("interaction/picking.rs")).contains("crate::state::display_frame::"),
        "non-vacuity: interaction/picking.rs no longer imports state::display_frame"
    );
    let mut files = Vec::new();
    rust_files(&root.join("state"), &mut files);
    assert!(files.len() > 10, "the scan found too few state files");
    let offenders: Vec<String> = files
        .iter()
        .filter(|path| code(path).contains("crate::interaction"))
        .map(|path| path.display().to_string())
        .collect();
    assert!(
        offenders.is_empty(),
        "these state files import interaction/, a back edge: {offenders:?}"
    );
}
