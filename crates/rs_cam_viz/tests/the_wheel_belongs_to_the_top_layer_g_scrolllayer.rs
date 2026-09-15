//! **G-SCROLLLAYER — the mouse wheel drives whatever is under the pointer,
//! and the viewport is the bottom layer.**
//!
//! # The defect
//!
//! Reported 2026-09-16: *"scroll in the modal also scrolls in the viewport.
//! seems the screens don't have a separation of control."* Scrolling inside
//! the Explore window zoomed the 3D camera behind it at the same time.
//!
//! `app/viewport.rs` guarded its zoom like this:
//!
//! ```ignore
//! let scroll_raw = ui.input(|i| i.smooth_scroll_delta.y);
//! if rect.contains(ui.input(|i| i.pointer.hover_pos().unwrap_or_default()))
//!     && scroll_raw != 0.0
//! { self.camera.zoom(scroll); }
//! ```
//!
//! Both reads are of the GLOBAL input state, and the test is a rectangle. **A
//! rectangle knows nothing about layering.** A modal window drawn over the
//! viewport sits inside the viewport's own rectangle, so a wheel event
//! delivered to the window's scroll area ALSO satisfied this guard.
//!
//! `unwrap_or_default` compounded it: with no pointer at all the fallback is
//! `Pos2::ZERO`, and a viewport anchored near the window origin contains that
//! point — so a wheel event with no pointer position zoomed the camera too.
//!
//! The fix is `Response::hovered()`, which is the layer-aware question: egui
//! runs a hit test that resolves which `Area` is on top, so it is false while
//! an `Area` above the viewport owns the pointer. The two OTHER pointer reads
//! in that function were already guarded this way; the wheel was the one that
//! was not.
//!
//! # Why the arms are shaped like this
//!
//! Arm 1 does not drive the app. It reproduces the MECHANISM against egui
//! itself: a background response with a window over it, and a pointer inside
//! both. It asserts the two guards disagree — `rect.contains` says yes,
//! `hovered()` says no — which is the whole of the bug and the whole of the
//! fix. If egui ever changed so that `hovered()` stopped respecting layer
//! order, this arm fails and tells the next reader the guard is no longer
//! sufficient, which a source scan never could.
//!
//! Arm 2 is the source scan, because arm 1 proves the tool works without
//! proving the product uses it.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::Path;

const SCREEN: egui::Vec2 = egui::Vec2::new(1000.0, 800.0);

/// Inside the background rect AND inside the window drawn over it.
const POINTER: egui::Pos2 = egui::Pos2::new(500.0, 400.0);

struct Probe {
    rect_contains_pointer: bool,
    background_hovered: bool,
}

/// Allocate a full-screen `click_and_drag` response — the viewport's own
/// shape — then draw a window over the middle of it, with the pointer inside
/// both.
fn probe() -> Probe {
    let ctx = egui::Context::default();
    let mut probe = Probe {
        rect_contains_pointer: false,
        background_hovered: false,
    };
    // egui settles hover state over a frame, so the reading is taken on a
    // later pass with the pointer held in place throughout.
    for pass in 0..4 {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, SCREEN)),
            events: vec![egui::Event::PointerMoved(POINTER)],
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| {
            let (rect, response) =
                ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());
            if pass == 3 {
                probe.rect_contains_pointer =
                    rect.contains(ui.input(|i| i.pointer.hover_pos().unwrap_or_default()));
                probe.background_hovered = response.hovered();
            }
            egui::Window::new("probe over the viewport")
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .default_size([400.0, 300.0])
                .vscroll(true)
                .show(ui.ctx(), |ui| {
                    ui.allocate_space(egui::vec2(380.0, 600.0));
                });
        });
        out.textures_delta.clear();
    }
    probe
}

// ── arm 1 — the two guards disagree, and that difference IS the bug ──────

#[test]
fn a_window_over_the_viewport_takes_the_pointer_g_scrolllayer() {
    let probe = probe();
    assert!(
        probe.rect_contains_pointer,
        "the probe did not reproduce the defect: the pointer must be INSIDE \
         the background rectangle, or the two guards cannot be compared"
    );
    assert!(
        !probe.background_hovered,
        "`Response::hovered()` was true for the background while a window \
         covered the pointer. The viewport's wheel guard relies on this being \
         false; if egui no longer resolves layer order this way, the guard in \
         app/viewport.rs is not sufficient and must be re-derived."
    );
}

// ── arm 2 — the viewport uses the layer-aware guard ──────────────────────

#[test]
fn the_viewport_zoom_asks_whether_it_is_hovered_g_scrolllayer() {
    let source =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/app/viewport.rs"))
            .expect("read viewport source");
    let at = source
        .find("smooth_scroll_delta")
        .expect("the viewport no longer reads the wheel; re-derive this arm");
    let guard = &source[at..(at + 200).min(source.len())];
    assert!(
        guard.contains("response.hovered()"),
        "the viewport's wheel guard is not `response.hovered()`. A guard that \
         tests a RECTANGLE against the global pointer cannot tell that a \
         modal window is drawn over that rectangle, so the wheel drives the \
         camera and the window at once."
    );
    assert!(
        !guard.contains("rect.contains"),
        "the viewport's wheel guard tests a rectangle again"
    );
    assert!(
        !guard.contains("unwrap_or_default"),
        "the wheel guard defaults a missing pointer to `Pos2::ZERO`, which a \
         viewport anchored near the window origin contains — so a wheel event \
         with no pointer at all zooms the camera"
    );
}
