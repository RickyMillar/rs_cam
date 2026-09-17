//! UI-08: an edit on the Post tab marks the project dirty.
//!
//! # The defect this exists to catch
//!
//! `app.rs:709` reads `os_close_requested && state.gui.dirty` to decide
//! whether to ask about unsaved changes. The Post branch of the inspector
//! applied `Command::SetPostConfig` and returned without
//! `GuiState::mark_edited`. Every other panel door — stock, tool, setup,
//! fixture, machine, machine kinematics, machine import — marks the project
//! edited on success.
//!
//! The consequence on screen: the operator changes Safe Z or the post
//! format, closes the window without clicking another tree item, and the
//! guard reads a clean project. The change is lost with no prompt.
//! `flush_post_snapshot` does mark the project edited, but only after the
//! selection has already moved off `Selection::PostProcessor`, which the
//! close path never does.
//!
//! # What the arms hold
//!
//! Arm 1 drives the real inspector through a headless `Context` with the
//! Post tab selected and one changed field. It asserts the dirty flag AND
//! the session value, so a flag that is set without the command applying
//! does not pass.
//!
//! Arm 2 is the non-vacuity anchor. It draws the same tab with no edit and
//! asserts the project stays clean. A dirty flag on an idle frame marks a
//! freshly loaded project edited the moment the operator looks at the tab.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use rs_cam_viz::state::AppState;
use rs_cam_viz::state::selection::Selection;
use rs_cam_viz::ui::{properties, tokens};

/// A headless egui context with the application's tokens and fonts.
fn ctx() -> egui::Context {
    let ctx = egui::Context::default();
    tokens::apply(&ctx);
    tokens::apply_fonts(&ctx);
    let mut warmup = ctx.run_ui(egui::RawInput::default(), |_ui| {});
    warmup.textures_delta.clear();
    ctx
}

/// Render the production inspector once.
fn draw_inspector(ctx: &egui::Context, state: &mut AppState) {
    let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
        let mut events = Vec::new();
        properties::draw(ui, state, &mut events);
    });
    out.textures_delta.clear();
}

#[test]
fn a_post_edit_marks_the_project_dirty() {
    let ctx = ctx();
    let mut state = AppState::new();
    state.selection = Selection::PostProcessor;

    // The tab must start clean, or the assertion below proves nothing.
    draw_inspector(&ctx, &mut state);
    assert!(
        !state.gui.dirty,
        "a fresh project is clean before the operator edits anything"
    );

    let edited_safe_z = state.gui.post.safe_z + 7.5;
    state.gui.post.safe_z = edited_safe_z;
    draw_inspector(&ctx, &mut state);

    assert!(
        (state.session.post_config().safe_z - edited_safe_z).abs() < 1e-9,
        "the panel must write Safe Z through Command::SetPostConfig"
    );
    assert!(
        state.gui.dirty,
        "a Post-tab edit must mark the project dirty, or the app.rs close \
         guard lets the operator discard it with no prompt"
    );
}

#[test]
fn looking_at_the_post_tab_does_not_mark_the_project_dirty() {
    let ctx = ctx();
    let mut state = AppState::new();
    state.selection = Selection::PostProcessor;

    for _ in 0..3 {
        draw_inspector(&ctx, &mut state);
    }

    assert!(
        !state.gui.dirty,
        "an idle frame on the Post tab must not mark the project edited"
    );
}
