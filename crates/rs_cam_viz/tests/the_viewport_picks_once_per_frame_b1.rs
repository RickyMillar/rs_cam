//! B1 — the viewport runs one hover pick per frame.
//!
//! Design: `planning/viewport_parity_2026-10-28/design/B_input_and_box_select.md`
//! §5. Before B1, `draw_viewport` ran up to three picks per frame on the same
//! pointer position. Two of them were scene picks with the same arguments.
//! Now `RsCamApp::hover_result` picks once, and every hover reader reads its
//! result.
//!
//! `RsCamApp` cannot be built in a test, so this file reads SOURCE. The scene
//! pick `interaction::picking::pick(` has two callers in `app/viewport.rs`:
//! the click path (`handle_viewport_click`) and `hover_result`. A third call
//! is a hover reader that picks again.

#![expect(
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::path::Path;

/// Read one source file, relative to this crate's manifest.
fn source(relative: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
    assert!(
        path.is_file(),
        "scanned path {} no longer exists",
        path.display()
    );
    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    assert!(
        text.len() > 500,
        "{} is too short to be the file this scan means",
        path.display()
    );
    text
}

#[test]
fn the_viewport_picks_the_scene_once_per_frame_b1() {
    let text = source("src/app/viewport.rs");
    // Non-vacuity anchor: the one hover pick exists.
    assert!(
        text.contains("fn hover_result("),
        "`fn hover_result(` is gone from src/app/viewport.rs: B1 reads it as the one hover pick"
    );
    let calls = text.matches("interaction::picking::pick(").count();
    assert!(
        calls <= 2,
        "src/app/viewport.rs calls `interaction::picking::pick(` {calls} times; B1 allows 2 \
         (the click path and `hover_result`, observed 2 at B1). A hover reader must read \
         `hover_result`, not pick again"
    );
}
