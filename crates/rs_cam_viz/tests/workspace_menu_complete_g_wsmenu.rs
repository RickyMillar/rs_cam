//! G-WSMENU — the four small IA repairs of F1.13, and the completeness rule
//! that stops the first one recurring.
//!
//! `Workspace` has four variants. The switcher bar named all four; the
//! Workspace menu named three, because each surface wrote its own list by
//! hand (`ui/menu_bar.rs`, `ui/workspace_bar.rs`, `app/mcp.rs`'s key mapping
//! and its round-trip test — four copies of one enumeration). Readiness had
//! been on the tab bar since W3.8 and was never added to the menu. Nothing
//! was unreachable, and `IA/CURRENT_MAP.md` §1 says so explicitly; it is a
//! consistency defect, and the interesting half of the fix is the rule, not
//! the missing entry.
//!
//! So `Workspace::ALL` is now the one list, on the
//! `ui/overlays/registry.rs` pattern, and this file is its sentry. Two
//! layers, the same pair `registry.rs`'s own completeness tests use:
//!
//! - the LIST is complete and internally consistent, driven against the real
//!   enum;
//! - the SURFACES are built from it, checked by reading their source. Every
//!   one of them draws through `egui::Ui`, and `RsCamApp` needs an
//!   `eframe::CreationContext`, so none can be driven from a test — the
//!   source check is what stands in, exactly as
//!   `mcp_toasts_report_outcome_g_mcptoast.rs` does for the MCP dispatch.
//!
//! The two wording repairs and the camera fit ride here too: same finding,
//! same commit, and all three are single-site facts that a string or source
//! assertion pins as well as anything could.
//!
//! Source: `planning/ui_review_2026-09-09/results/IA/SUMMARY.md` ("include
//! Readiness in the Workspace menu; update stale 'project tree' wording")
//! and `IA/CURRENT_MAP.md` §1 and §3.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_viz::state::Workspace;

const MENU_SRC: &str = include_str!("../src/ui/menu_bar.rs");
const BAR_SRC: &str = include_str!("../src/ui/workspace_bar.rs");
const PROPERTIES_SRC: &str = include_str!("../src/ui/properties/mod.rs");
const APP_SRC: &str = include_str!("../src/app.rs");
const INPUT_SRC: &str = include_str!("../src/app/input.rs");
const MCP_SRC: &str = include_str!("../src/app/mcp.rs");

// ── the list ───────────────────────────────────────────────────────────────

/// THE completeness rule. `order()` is an exhaustive match, so a new variant
/// does not compile until it is given a position; this asserts the position
/// is a real slot in `ALL` and that no two variants share one. Together those
/// two facts make `ALL` complete, rather than merely four items long.
#[test]
fn every_workspace_has_its_own_slot_in_all() {
    let mut seen = vec![false; Workspace::ALL.len()];
    for ws in Workspace::ALL {
        let at = ws.order();
        assert!(
            at < Workspace::ALL.len(),
            "{ws:?} claims position {at}, which is not a slot in Workspace::ALL"
        );
        assert_eq!(
            Workspace::ALL[at],
            ws,
            "{ws:?} claims position {at}, which holds {:?}",
            Workspace::ALL[at]
        );
        assert!(!seen[at], "two workspaces claim position {at}");
        seen[at] = true;
    }
    assert!(
        seen.iter().all(|hit| *hit),
        "Workspace::ALL has a slot no variant claims: {seen:?}"
    );
}

/// A workspace with no label cannot appear in a menu, and two workspaces
/// sharing one cannot be told apart in it.
#[test]
fn every_workspace_has_a_distinct_label_and_a_hint() {
    let mut labels: Vec<&str> = Vec::new();
    for ws in Workspace::ALL {
        assert!(!ws.label().is_empty(), "{ws:?} has no label");
        assert!(!ws.hint().is_empty(), "{ws:?} has no hint");
        assert!(
            !labels.contains(&ws.label()),
            "two workspaces are labelled {:?}",
            ws.label()
        );
        labels.push(ws.label());
    }
}

/// Readiness by name — the variant that was actually missing. A rule with no
/// witness is easy to satisfy by weakening the rule.
#[test]
fn readiness_is_in_the_list() {
    assert!(Workspace::ALL.contains(&Workspace::Readiness));
    assert_eq!(Workspace::Readiness.label(), "Readiness");
}

// ── the surfaces ───────────────────────────────────────────────────────────

/// Extract the body of one `ui.menu_button("<name>", |ui| { … })` block.
fn menu_block<'a>(src: &'a str, name: &str) -> &'a str {
    let needle = format!("ui.menu_button(\"{name}\"");
    let start = src
        .find(&needle)
        .unwrap_or_else(|| panic!("no {name} menu in menu_bar.rs"));
    let rest = &src[start..];
    // Generously bounded: the Workspace menu is a handful of lines and the
    // next `menu_button` ends it.
    let end = rest[needle.len()..]
        .find("ui.menu_button(")
        .map_or(rest.len(), |at| at + needle.len());
    &rest[..end]
}

/// THE sentry for the menu. It iterates the one list instead of naming
/// workspaces, so a fifth workspace reaches it for free. Pre-fix this block
/// contained three hand-written `ui.button("Setup"/"Toolpaths"/"Simulation")`
/// arms and no mention of `Workspace::ALL`.
#[test]
fn the_workspace_menu_is_built_from_the_one_list() {
    let block = menu_block(MENU_SRC, "Workspace");
    assert!(
        block.contains("Workspace::ALL"),
        "the Workspace menu must iterate Workspace::ALL, not name workspaces \
         one at a time:\n{block}"
    );
    for ws in Workspace::ALL {
        assert!(
            !block.contains(&format!("ui.button(\"{}\")", ws.label())),
            "the Workspace menu still hard-codes {:?}; that is the copy that \
             lost Readiness:\n{block}",
            ws.label()
        );
    }
}

/// The switcher bar reads the same list, so the two surfaces cannot drift
/// apart again from the other side.
#[test]
fn the_switcher_bar_is_built_from_the_one_list() {
    assert!(
        BAR_SRC.contains("for target in Workspace::ALL"),
        "the switcher bar must iterate Workspace::ALL"
    );
    for ws in Workspace::ALL {
        assert!(
            !BAR_SRC.contains(&format!("workspace_tab(ui, \"{}\"", ws.label())),
            "the switcher bar still hard-codes {:?}",
            ws.label()
        );
    }
}

/// The MCP `set_ui_view` key mapping is the third surface. Its round-trip
/// test used to carry a fourth hand-written copy of the enumeration.
#[test]
fn the_mcp_key_round_trip_reads_the_one_list() {
    assert!(
        MCP_SRC.contains("for ws in Workspace::ALL {"),
        "workspace_keys_round_trip must iterate Workspace::ALL, or it only \
         covers the variants someone remembered to type"
    );
}

// ── the wording repairs ────────────────────────────────────────────────────

/// "The project tree" is not a panel this app has (IA/CURRENT_MAP.md §1). The
/// replacement names the four things that can actually be selected.
#[test]
fn the_empty_inspector_names_what_can_be_selected() {
    assert!(
        PROPERTIES_SRC.contains("Select an operation, tool, setup or model"),
        "the selection-none fallback must name the selectable kinds"
    );
    assert!(
        !PROPERTIES_SRC.contains("Select an item in the project tree"),
        "the stale \"project tree\" wording must be gone, not duplicated"
    );
}

/// The Getting-started list ends at the machine, so simulation cannot be a
/// clause inside the export step. Generate, review, then export — in that
/// order, and each on its own line.
#[test]
fn getting_started_reviews_before_it_exports() {
    for step in [
        "\"5. Generate toolpaths\"",
        "\"6. Simulate and review\"",
        "\"7. Export G-code\"",
    ] {
        assert!(
            PROPERTIES_SRC.contains(step),
            "the Getting started list is missing {step}"
        );
    }
    assert!(
        !PROPERTIES_SRC.contains("Generate and export G-code"),
        "the combined generate-and-export step must be replaced, not kept beside \
         the new ones"
    );
    let simulate = PROPERTIES_SRC
        .find("6. Simulate and review")
        .expect("checked above");
    let export = PROPERTIES_SRC
        .find("7. Export G-code")
        .expect("checked above");
    assert!(
        simulate < export,
        "review comes before export, or the step is decoration"
    );
}

// ── the camera fit ─────────────────────────────────────────────────────────

/// Every `open_job_from_path` call site fits the camera on success.
///
/// A load replaces every model in the project. The import dispatch has always
/// fitted (`AppEvent::ImportStl` and friends in `app/input.rs`); all three
/// load routes — File > Open, the `RS_CAM_JOB` startup path and MCP
/// `load_project` — did not, so a loaded job kept the previous project's
/// framing. This asserts each site calls `fit_camera_to_first_model`, the
/// same routine `AppEvent::ResetView` uses, rather than inventing a second
/// fit. `RsCamApp` needs an `eframe::CreationContext`, so this is read from
/// the source.
#[test]
fn every_project_load_route_fits_the_camera() {
    // The two short arms: the fit sits inside the `Ok` arm, a few lines
    // below the call.
    for (label, src) in [
        ("app/input.rs File > Open", INPUT_SRC),
        ("app/mcp.rs load_project", MCP_SRC),
    ] {
        let at = src
            .find("open_job_from_path")
            .unwrap_or_else(|| panic!("{label}: no open_job_from_path call found"));
        let tail = &src[at..src.len().min(at + 800)];
        assert!(
            tail.contains("fit_camera_to_first_model()"),
            "{label}: a project load must fit the camera, as import does"
        );
    }

    // The startup route cannot fit at the call: `RsCamApp` — and with it the
    // camera — does not exist yet while `RS_CAM_JOB` is loading. It records
    // that a job loaded and fits once the app is built, so the assertion is
    // on that pair rather than on a window of source after the call.
    assert!(
        APP_SRC.contains("loaded_a_job = true"),
        "app.rs must record that RS_CAM_JOB loaded something"
    );
    assert!(
        APP_SRC.contains("if loaded_a_job {")
            && APP_SRC.contains("app.fit_camera_to_first_model()"),
        "app.rs must fit the camera once the app exists, for a job loaded at startup"
    );
}

/// One fit routine, not two. If a route ever needs different framing that is
/// a deliberate change, and this is where it gets noticed.
#[test]
fn the_load_routes_reuse_the_reset_view_fit() {
    assert!(
        INPUT_SRC.contains("AppEvent::ResetView => self.fit_camera_to_first_model()"),
        "Reset View is the routine the load routes borrow; if it moved, say so"
    );
}
