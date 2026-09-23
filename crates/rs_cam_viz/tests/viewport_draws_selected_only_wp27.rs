//! WP27 — the viewport draws the SELECTED toolpath only, by default.
//!
//! Programme: `planning/arch_consolidation_2026-09-09/IMPLEMENTATION_PLAN.md`
//! §32. The operator ruling names the cost: a project with many toolpaths
//! draws every one of them every frame, and the viewport gets slow. The rule
//! is one selected toolpath by default, and "show all" one click away.
//!
//! ## What this file measures
//!
//! One pure function, `state::viewport::toolpaths_to_draw`, answers which
//! toolpaths the viewport draws. It takes no GPU, no `eframe` context and no
//! `RenderResources`, so every behaviour arm here is a direct call.
//!
//! Three arms read SOURCE instead. Two code paths must ask the one function —
//! the GPU upload and the click pick — because a click that selects geometry
//! the viewport does not draw is the drift a second predicate produces. And
//! the upload key must carry both new dials: the selection decides the draw
//! SET now, and 39 of the 40 production selection writers fire no upload of
//! their own.
//!
//! ## The rule after UR5
//!
//! ```text
//! show_all || selected == Some(id)
//! ```
//!
//! UR5 retires isolation. Selection now steers selected-only drawing, while
//! show-all remains the explicit escape hatch.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::Path;

use rs_cam_viz::state::toolpath::ToolpathId;
use rs_cam_viz::state::viewport::{
    ToolpathDrawFilter, ViewportState, toolpaths_to_draw, vector_source_focus,
};
use rs_cam_viz::state::{AppState, Workspace};
use rs_cam_viz::ui::overlays::registry;

// ── fixtures ─────────────────────────────────────────────────────────

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

/// Strip every `//` comment from one source text.
///
/// A scan that matches a doc comment reports a reader that does not exist.
/// The upload pass describes the old gate in its own comments, so the strip
/// is what keeps the two retirement arms honest.
fn strip_comments(text: &str) -> String {
    text.lines()
        .map(|line| match line.find("//") {
            Some(at) => &line[..at],
            None => line,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

const A: ToolpathId = ToolpathId(1);
const B: ToolpathId = ToolpathId(2);
const C: ToolpathId = ToolpathId(3);

/// Three generated, visible toolpaths, in config order.
fn three_rows() -> Vec<(ToolpathId, bool, bool)> {
    let rows = vec![(A, true, true), (B, true, true), (C, true, true)];
    assert_eq!(rows.len(), 3, "the fixture must carry three rows");
    rows
}

/// The registry row must exist before any draw-set claim is read as the
/// shipped rule: the panel, `set_ui_view` and the per-workspace default table
/// all reach the dial through it.
fn assert_the_row_exists() {
    assert!(
        registry::row("all_toolpaths").is_some(),
        "the Overlays registry carries no `all_toolpaths` row, so no control \
         and no MCP call reaches the draw scope"
    );
}

// ── (i) the shipped default ──────────────────────────────────────────

/// The measurement the ruling asked for: a fresh viewport draws ONE toolpath.
///
/// The flag is read off `ViewportState::new()` rather than written as a
/// literal `false`, so this arm measures the shipped default instead of
/// restating it.
#[test]
fn a_fresh_viewport_draws_the_selected_toolpath_only() {
    assert_the_row_exists();
    let filter = ToolpathDrawFilter {
        selected: Some(B),
        show_all: ViewportState::new().show_all_toolpaths,
    };
    let drawn = toolpaths_to_draw(filter, three_rows());
    assert_eq!(
        drawn,
        vec![B],
        "a fresh viewport drew {} toolpaths; the §32 rule is the selected one \
         only, and the shipped default for `show_all_toolpaths` is what this \
         arm reads",
        drawn.len()
    );
}

// ── (ii) show-all is the escape hatch ────────────────────────────────

#[test]
fn show_all_draws_every_toolpath_in_config_order() {
    let filter = ToolpathDrawFilter {
        selected: Some(B),
        show_all: true,
    };
    assert_eq!(
        toolpaths_to_draw(filter, three_rows()),
        vec![A, B, C],
        "show-all must draw every row, in config order — the order is what \
         keeps each toolpath's palette colour"
    );
}

// ── (iii) selection steers; show-all remains the escape hatch ───────

#[test]
fn the_draw_set_follows_selection_and_show_all_overrides_it() {
    let selected_only = |selected: Option<ToolpathId>| ToolpathDrawFilter {
        selected,
        show_all: false,
    };
    assert_eq!(
        toolpaths_to_draw(selected_only(Some(A)), three_rows()),
        vec![A]
    );
    assert_eq!(
        toolpaths_to_draw(selected_only(Some(C)), three_rows()),
        vec![C]
    );

    let all = ToolpathDrawFilter {
        selected: Some(C),
        show_all: true,
    };
    assert_eq!(
        toolpaths_to_draw(all, three_rows()),
        vec![A, B, C],
        "show-all must override selected-only drawing"
    );
}

// ── (iv) the two older gates keep working ────────────────────────────

/// The eye button (`ToolpathRuntime::visible`) and the "not generated yet"
/// gate are unchanged by WP27, and the one function carries both.
#[test]
fn an_ungenerated_or_hidden_toolpath_is_never_drawn() {
    let filter = ToolpathDrawFilter {
        selected: Some(B),
        show_all: false,
    };
    let ungenerated = vec![(A, true, true), (B, true, false), (C, true, true)];
    assert!(
        toolpaths_to_draw(filter, ungenerated).is_empty(),
        "a toolpath with no result has no geometry to draw"
    );
    let hidden = vec![(A, true, true), (B, false, true), (C, true, true)];
    assert!(
        toolpaths_to_draw(filter, hidden).is_empty(),
        "the eye button must still hide the selected toolpath"
    );
}

// ── (v) the §2.2 ruling ──────────────────────────────────────────────

/// With nothing selected the viewport draws NO toolpath. The model and the
/// stock still draw, and the operations list is the picker.
///
/// This arm is here so a later reader cannot mistake the empty post-load
/// viewport for a defect: `controller/io.rs` sets `Selection::None` on a
/// project load, so that state is reached on every load.
#[test]
fn nothing_selected_draws_nothing() {
    let filter = ToolpathDrawFilter {
        selected: None,
        show_all: false,
    };
    assert!(
        toolpaths_to_draw(filter, three_rows()).is_empty(),
        "nothing selected must draw nothing — the operator ruling, not a \
         fallback to draw-all"
    );
}

// ── (vi) one rule, two readers ───────────────────────────────────────

/// The upload pass and the pick path ask the SAME function.
///
/// Two predicates drift, and the drift is a click that selects geometry the
/// viewport does not draw. UR5 also retires the isolate route rather than
/// leaving a second draw rule beside the shared one.
#[test]
fn the_upload_and_the_pick_read_one_draw_rule() {
    let upload = strip_comments(&source("src/app/gpu_upload.rs"));
    let pick = strip_comments(&source("src/interaction/picking.rs"));
    assert!(
        upload.contains("toolpaths_to_draw"),
        "src/app/gpu_upload.rs no longer asks `toolpaths_to_draw`"
    );
    assert!(
        pick.contains("toolpaths_to_draw"),
        "src/interaction/picking.rs no longer asks `toolpaths_to_draw`; a \
         click can now select a toolpath the viewport does not draw"
    );
}

/// UR5 removes the isolate state and every UI route that could write it.
///
/// Each scanned file has a live, related anchor first, so this census cannot
/// pass merely because a source file moved or was replaced with an empty stub.
#[test]
fn ur5_has_no_isolate_routes() {
    for (name, code, anchor) in [
        (
            "viewport state",
            source("src/state/viewport.rs"),
            "show_all_toolpaths",
        ),
        (
            "UI commands",
            source("src/ui_command.rs"),
            "ToggleToolpathVisibility",
        ),
        ("input", source("src/app/input.rs"), "show_all_toolpaths"),
        (
            "event dispatch",
            source("src/controller/events/mod.rs"),
            "ToggleToolpathVisibility",
        ),
        (
            "toolpath events",
            source("src/controller/events/toolpath.rs"),
            "Selection::Toolpath",
        ),
        (
            "planner events",
            source("src/controller/events/planner.rs"),
            "Selection::Toolpath",
        ),
        (
            "viewport overlay",
            source("src/ui/viewport_overlay.rs"),
            "show_all_toolpaths",
        ),
        (
            "toolpath panel",
            source("src/ui/toolpath_panel.rs"),
            "ToggleToolpathVisibility",
        ),
        (
            "shared controls",
            source("src/ui/toolpath_row_controls.rs"),
            "ToggleToolpathVisibility",
        ),
        (
            "properties",
            source("src/ui/properties/mod.rs"),
            "Selection::Toolpath",
        ),
        (
            "simulation rows",
            source("src/ui/sim_op_list.rs"),
            "show_all_toolpaths",
        ),
        (
            "GPU upload",
            source("src/app/gpu_upload.rs"),
            "toolpaths_to_draw",
        ),
        (
            "picking",
            source("src/interaction/picking.rs"),
            "toolpaths_to_draw",
        ),
    ] {
        let code = strip_comments(&code);
        assert!(code.contains(anchor), "non-vacuity: {name} lost `{anchor}`");
        for retired in [
            "isolate_toolpath",
            "ToggleIsolateToolpath",
            "ClearIsolation",
        ] {
            assert!(
                !code.contains(retired),
                "UR5 retired isolate routes; {name} still contains `{retired}`"
            );
        }
    }
}

// ── (vii) the CPU rasteriser is untouched ────────────────────────────

/// MCP `screenshot_toolpath` renders ONE result on the CPU. It reads neither
/// the draw set nor the show-all scope, so WP27 does not change what it photographs.
/// `screenshot_gui` captures the live window and DOES change — that is the
/// one MCP behaviour change, and it is a doc note, not a code path here.
#[test]
fn mcp_screenshot_toolpath_does_not_read_the_draw_set() {
    // P4 (2026-09-17) moved the handler into `app/mcp/view.rs`. The next
    // method there carries a visibility prefix, so the end marker reads
    // both spellings: a bare `"\n    fn "` alone would run past it.
    let mcp = strip_comments(&source("src/app/mcp/view.rs"));
    let marker = "fn mcp_screenshot_toolpath";
    let start = mcp
        .find(marker)
        .unwrap_or_else(|| panic!("`{marker}` no longer exists in app/mcp/view.rs"));
    let rest = &mcp[start..];
    let end = ["\n    fn ", "\n    pub(super) fn ", "\n    pub(crate) fn "]
        .iter()
        .filter_map(|m| rest.find(m))
        .min()
        .unwrap_or(rest.len());
    let body = &rest[..end];
    assert!(
        body.len() > 200,
        "the body locator returned {} bytes, so this arm asserts nothing",
        body.len()
    );
    for needle in [
        "toolpaths_to_draw",
        "isolate_toolpath",
        "show_all_toolpaths",
    ] {
        assert!(
            !body.contains(needle),
            "mcp_screenshot_toolpath now reads `{needle}`; it rasterises one \
             result on the CPU and must stay independent of the viewport's \
             draw set"
        );
    }
}

// ── (vii-b) both new dials reach the screen ──────────────────────────

/// The composite upload key carries the draw scope AND the selection.
///
/// The scope half duplicates the overlays sentry on purpose — it reads the
/// same source for the same reason. The selection half has no other sentry
/// and is the load-bearing one: 39 of the 40 production selection writers
/// set no `pending_upload`, so without this field an operator deselects a
/// toolpath and keeps seeing it, and `set_ui_view(toolpath_index: N)`
/// followed by `screenshot_gui` photographs the PREVIOUS toolpath.
#[test]
fn the_upload_key_carries_the_scope_and_the_selection() {
    let app = strip_comments(&source("src/app.rs"));
    let marker = "pub(crate) fn overlay_upload_key";
    let start = app
        .find(marker)
        .unwrap_or_else(|| panic!("`{marker}` no longer exists in app.rs"));
    let rest = &app[start..];
    let end = rest.find("\n}").unwrap_or(rest.len());
    let body = &rest[..end];
    assert!(
        body.len() > 200,
        "the key body locator returned {} bytes, so this arm asserts nothing",
        body.len()
    );
    assert!(
        body.contains("show_all_toolpaths"),
        "`overlay_upload_key` does not read `show_all_toolpaths`, so the \
         Overlays checkbox does nothing until an unrelated event fires an \
         upload — the tool-profile ghost defect class"
    );
    assert!(
        body.contains("state.selection"),
        "`overlay_upload_key` does not read `state.selection`, so a \
         selection change leaves the viewport drawing the previous toolpath"
    );
    assert!(
        body.contains("vector_source_toolpath"),
        "the upload key must follow Simulation's current boundary when it \
         supplies the contextual vector fallback"
    );
}

#[test]
fn vector_upload_is_contextual_and_uses_operation_specific_heights() {
    let upload = strip_comments(&source("src/app/gpu_upload.rs"));
    for needle in [
        "vector_source_toolpath(state)",
        "find(|model| model.id == toolpath.model_id)",
        "height_context_from_session(&state.session, &toolpath)",
        "toolpath.heights.resolve(&context)",
        ".cutting_levels(heights.top_z)",
        "surface_model_id",
        "face.z_at_xy(x, y)",
    ] {
        assert!(
            upload.contains(needle),
            "contextual vector upload lost `{needle}`"
        );
    }
}

// ── (viii) Simulation stays decluttered ──────────────────────────────

#[test]
fn entering_simulation_keeps_toolpath_drawing_off() {
    assert_the_row_exists();
    let mut state = AppState::new();
    registry::switch_workspace(&mut state, Workspace::Simulation);
    assert!(
        !state.viewport.show_all_toolpaths,
        "Simulation must not turn on global toolpath drawing"
    );
    assert!(
        !state.viewport.show_cutting
            && !state.viewport.show_rapids
            && !state.viewport.show_entry_markers,
        "Simulation must default every toolpath visual family off"
    );
}

#[test]
fn vector_source_focus_prefers_selection_then_simulation_and_hides_without_either() {
    assert_eq!(vector_source_focus(Some(A), Some(B)), Some(A));
    assert_eq!(vector_source_focus(None, Some(B)), Some(B));
    assert_eq!(vector_source_focus(None, None), None);
}
