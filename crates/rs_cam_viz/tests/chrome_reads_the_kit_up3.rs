//! UP3's sentry: the chrome uses the component set.
//!
//! # What "chrome" means here
//!
//! The frame around the work: the workspace tab bar, the status bar, the menu
//! bar and the readiness action row. It is the first thing an operator sees
//! and the last thing anyone restyles, which is why the audit found it
//! carrying raw colour literals and 28-point tabs on a 26-point scale.
//!
//! # Why this package exists at all
//!
//! Operator, 2026-09-14, looking at the first UP1 capture: *"ohh, the tabs,
//! the buttons. all of that looks way more out of place now"*.
//!
//! That is the predictable middle of a migration. UP1 raised the ground, the
//! type and the radii everywhere; the chrome kept its own inline styling, so
//! it stopped reading as "plain" and started reading as "unfinished". The
//! fix is not to soften UP1 but to finish the chrome.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::{Path, PathBuf};

use rs_cam_viz::ui::tokens;

/// The chrome files. Each must be free of raw colour literals.
const CHROME: [&str; 4] = [
    "ui/workspace_bar.rs",
    "ui/status_bar.rs",
    "ui/menu_bar.rs",
    "ui/readiness_panel.rs",
];

fn src_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

fn strip_comment(line: &str) -> &str {
    match line.find("//") {
        Some(i) => &line[..i],
        None => line,
    }
}

fn colour_literals(path: &Path) -> usize {
    let text =
        std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    text.lines()
        .map(|l| {
            strip_comment(l).matches("Color32::from_rgb(").count()
                + strip_comment(l).matches("Color32::from_rgba_").count()
        })
        .sum()
}

#[test]
fn the_chrome_names_no_colour_literal_up3() {
    let root = src_root();
    let mut offenders = Vec::new();
    let mut scanned = 0usize;

    for rel in CHROME {
        let path = root.join(rel);
        assert!(
            path.is_file(),
            "chrome file {rel} no longer exists; the list is stale"
        );
        scanned += 1;
        let n = colour_literals(&path);
        if n > 0 {
            offenders.push(format!("{rel} {n}"));
        }
    }

    assert_eq!(
        scanned,
        CHROME.len(),
        "non-vacuity: the scan must visit every chrome file"
    );
    assert!(
        offenders.is_empty(),
        "the chrome still names colour literals: {}. Every colour is a token \
         or it does not ship (DESIGN_SPEC.md §2.10).",
        offenders.join(", ")
    );
}

#[test]
fn a_workspace_tab_is_on_the_token_scale_up3() {
    let root = src_root();
    let src = std::fs::read_to_string(root.join("ui/workspace_bar.rs")).unwrap();

    assert!(
        !src.contains("28.0"),
        "the tab was 28 points tall on a 26-point scale. It must read \
         tokens::ROW_ACTION."
    );
    assert!(
        src.contains("ROW_ACTION"),
        "the tab height must come from the token, not a literal"
    );
    assert!(
        src.contains("ACCENT_QUIET"),
        "an active tab is a SELECTED thing, so it takes ACCENT_QUIET — the \
         same fill a selected row takes. It used to carry a private \
         violet-blue that matched nothing else in the product."
    );
    // §2.2: a tab really does join the panel below it, so the radius is on
    // the two TOP corners only.
    assert!(
        src.contains("RADIUS_SM"),
        "the tab's top corners take RADIUS_SM"
    );
}

#[test]
fn a_workspace_badge_is_a_dot_not_a_pseudo_tab_up3() {
    let root = src_root();
    let src = std::fs::read_to_string(root.join("ui/workspace_bar.rs")).unwrap();
    // UR2 applies DC3's compact treatment to Danger too: safety has four
    // dedicated surfaces, while the tab strip remains navigation only.
    assert!(
        !src.contains("StatusChip"),
        "workspace badges must not render as external pseudo-tabs"
    );
    assert!(
        src.contains("circle_filled"),
        "every workspace badge must use the shared compact dot treatment"
    );
    assert!(
        src.contains("on_hover_text"),
        "the compact badge must keep its count accessible on hover"
    );
}

#[test]
fn the_readiness_action_row_names_one_primary_up3() {
    let root = src_root();
    let src = std::fs::read_to_string(root.join("ui/readiness_panel.rs")).unwrap();

    assert!(
        src.contains("Button::primary") || src.contains("ButtonVariant::Primary"),
        "the readiness row offered 'Export G-code…' and 'Run simulation' at \
         the SAME weight, so the screen could not say which action it was \
         FOR. §4.5 allows exactly one Primary per screen."
    );
    let primaries =
        src.matches("Button::primary").count() + src.matches("ButtonVariant::Primary").count();
    assert!(
        primaries <= 1,
        "§4.5: at most ONE Primary per screen, found {primaries}"
    );
}

#[test]
fn the_chrome_tokens_exist_up3() {
    // Non-vacuity for the source scans above: the tokens they grep for must
    // be real, so a renamed token fails here rather than silently making a
    // scan unsatisfiable.
    const _: () = assert!(tokens::ROW_ACTION == 26.0);
    let _ = tokens::ACCENT_QUIET;
    let _ = tokens::RADIUS_SM;
    let _ = tokens::SURFACE_BASE;
}
