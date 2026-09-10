//! G-FRESHRENDER (F2.2) — the three surfaces a test cannot drive.
//!
//! Most of F2.2's sentry is in `src/controller/tests.rs`, driving the real
//! functions: the chip vocabulary, both workspace badges, `operations_check`
//! and the shared counter are all pure over `&AppState`. Three surfaces are
//! not: the inspector header and the card body draw through `egui::Ui`, and
//! the dimmed viewport path is a wgpu bind-group choice inside a render pass.
//! This file reads their source, the same stand-in
//! `mcp_toasts_report_outcome_g_mcptoast.rs` and
//! `inspector_header_wraps_g_reachwrap.rs` already use here.
//!
//! **What it proves:** each of the three reads `FreshnessState` and not the
//! thing it used to read, and the `EditedSince` arm exists and says
//! something. **What it does not:** that any of it looks right on screen.
//! PLAN's acceptance for F2.2 is an MCP-VIEW screenshot on the terrain seed
//! plus a `list_toolpaths.stale` cross-check; the MCP server is down this
//! session and no GUI may be started, so neither was run and neither is
//! claimed. Visual confirmation is owed to V6.2.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

const PROPERTIES_SRC: &str = include_str!("../src/ui/properties/mod.rs");
const PANEL_SRC: &str = include_str!("../src/ui/toolpath_panel.rs");
const RENDER_SRC: &str = include_str!("../src/render/mod.rs");
const VIEWPORT_SRC: &str = include_str!("../src/app/viewport.rs");
const READINESS_PANEL_SRC: &str = include_str!("../src/ui/readiness_panel.rs");

/// The inspector header's status line reads the freshness state, and its
/// `EditedSince` arm says both halves — the generation finished, and what it
/// produced no longer answers the settings on screen.
///
/// Pre-fix the header matched `ComputeStatus::effective(entry.enabled,
/// &entry.status)`, whose `Done` arm printed a green "Done" directly above
/// the fields the operator had just changed.
#[test]
fn the_inspector_header_reads_the_freshness_state() {
    let at = PROPERTIES_SRC
        .find("// F2.2 — the same one state the card chip")
        .expect("the header's status match moved");
    let block = &PROPERTIES_SRC[at..(at + 3000).min(PROPERTIES_SRC.len())];

    assert!(
        block.contains("match freshness {"),
        "the header must match on the freshness state"
    );
    assert!(
        !block.contains("ComputeStatus::effective(entry.enabled"),
        "the header must not re-derive its own answer from ComputeStatus"
    );
    assert!(
        block.contains("FreshnessState::EditedSince =>"),
        "the header needs an arm for the state that did not exist before"
    );
    assert!(
        block.contains("edited since"),
        "and that arm has to say so: {block}"
    );
}

/// The card's chip, stats row and ▶ button all read the one state.
///
/// The ▶ button is the sharpest of the three: `ComputeStatus::needs_generation`
/// answers `false` for `Done`, and an edited operation keeps `Done`, so the
/// quick-generate button was hidden on precisely the card whose whole message
/// is "regenerate me".
#[test]
fn the_card_reads_the_freshness_state() {
    assert!(
        PANEL_SRC.contains("let (status_text, status_color, hover) = status_chip(freshness);"),
        "the chip must come from the shared mapping"
    );
    assert!(
        !PANEL_SRC.contains("ComputeStatus::effective("),
        "the card must not re-derive its own answer from ComputeStatus"
    );
    assert!(
        PANEL_SRC.contains("let needs_generation = !matches!(")
            && !PANEL_SRC.contains("if status.needs_generation()"),
        "the generate button must follow freshness, not ComputeStatus::needs_generation"
    );
    assert!(
        PANEL_SRC.contains("if is_stale { \"old: \" } else { \"\" }"),
        "an edited operation's figures must say whose they are"
    );
}

/// The viewport dims a stale path instead of drawing it at full strength.
///
/// Drawn, not hidden: the operator asked to see this operation, and hiding it
/// would replace a wrong picture with no picture.
#[test]
fn the_viewport_dims_a_stale_path() {
    assert!(
        RENDER_SRC.contains("pub stale_toolpaths:"),
        "the renderer needs to be told which toolpaths are stale"
    );
    let at = RENDER_SRC
        .find("let stale = tp_gpu")
        .expect("the per-toolpath dim decision moved");
    let block = &RENDER_SRC[at..(at + 500).min(RENDER_SRC.len())];
    assert!(
        block.contains("self.stale_toolpaths.contains(&id)"),
        "the decision must be per toolpath, not per frame:\n{block}"
    );
    assert!(
        block.contains("line_dim_bind_group"),
        "a stale path takes the dimmed bind group:\n{block}"
    );
    // Populated from the one model, not from a second opinion about it.
    assert!(
        VIEWPORT_SRC.contains("stale_toolpaths: state")
            && VIEWPORT_SRC.contains("FreshnessState::EditedSince"),
        "the set must be derived from freshness_at, not from stale_since"
    );
}

/// Readiness names its count "current", not "computed", and says how many
/// were edited after generation.
///
/// An operation edited after generation WAS computed; what it is not is the
/// answer to the configuration now in the project.
#[test]
fn readiness_says_current_and_names_the_edited_ones() {
    assert!(
        READINESS_PANEL_SRC.contains("{computed}/{enabled} current"),
        "the row must not go on calling an edited operation computed"
    );
    assert!(
        !READINESS_PANEL_SRC.contains("{computed}/{enabled} computed"),
        "the old wording must be replaced, not kept beside the new one"
    );
    assert!(
        READINESS_PANEL_SRC.contains("edited since generation"),
        "and it must name the edited ones, which is the distinction \
         'uncomputed' could not draw"
    );
}

/// The caution this whole task ships under: F2.3 owns the export gate, and
/// until it lands an operation reading STALE can still be exported from the
/// GUI's retained result. No text this task adds may imply otherwise.
///
/// A census over the strings F2.2 introduced, so a later edit cannot quietly
/// turn a freshness notice into an export promise.
#[test]
fn no_freshness_text_promises_that_export_is_gated() {
    for (label, src) in [
        ("toolpath_panel.rs", PANEL_SRC),
        ("readiness_panel.rs", READINESS_PANEL_SRC),
    ] {
        for line in src.lines() {
            let lower = line.to_lowercase();
            if !lower.contains("stale") && !lower.contains("edited since") {
                continue;
            }
            assert!(
                !lower.contains("export"),
                "{label}: a freshness string mentions export, but F2.3 has not \
                 landed the gate and a stale operation still exports: {line}"
            );
        }
    }
}
