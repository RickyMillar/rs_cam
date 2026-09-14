//! DC3's sentry: the workspace bar is a STRIP, and its badges are indicators.
//!
//! # The defect this exists to catch
//!
//! Operator, 2026-09-14, on the workspace tab bar: the tabs "look like
//! pills"; `6 pending` and `6 uncomputed` "take odd space" and "should be
//! indicators with hover".
//!
//! Both halves were real and both were structural, not cosmetic.
//!
//! 1. **No shared baseline.** Each tab painted alone. The active tab drew a
//!    2-point `ACCENT` segment on its own bottom edge and every other tab
//!    drew nothing there, so four rounded shapes floated above a panel they
//!    were supposed to be attached to. `PLAN.md` Pattern E: a tab strip is a
//!    strip.
//! 2. **The badge was a WORD in the strip.** UP3 had already quietened it
//!    from a verdict chip to a caption, which fixed the shouting but not the
//!    space: `6 pending` still rendered BETWEEN Toolpaths and Simulation, so
//!    it belonged to neither tab, and it changed the tab slot's width. Ruling
//!    R30: a Danger badge keeps its chip, every other badge becomes a dot on
//!    its own tab with the count on hover.
//!
//! A third element went with them: the right-hand hint strip, which named the
//! workspace the operator had just chosen (Pattern C).
//!
//! # Why this invariant and not a screenshot diff
//!
//! A screenshot diff of a tab bar fails on a font metric, a DPI change and a
//! theme edit, and it cannot say WHICH of those moved. The three facts below
//! are the ones the operator complained about, and each is checkable without
//! a frame:
//!
//! - one hairline is reserved BEFORE the tabs draw, so the active tab's
//!   accent paints over it — that paint ORDER is the whole fix, and it is
//!   invisible in a still image that renders correctly by luck;
//! - the non-Danger arm adds no text to the strip;
//! - `hint()` is gone from the surface.
//!
//! # This is a source scan, and the reason is visibility
//!
//! `toolpath_badge` and `readiness_badge` are `pub(crate)` and
//! `simulation_badge` is private, so an integration test in another crate
//! cannot call any of the three. Widening them to `pub` to let a test drive
//! them would add public API for a test's convenience. The fourth arm
//! therefore reads the source and asserts the ordering ARMS appear in their
//! documented order. That is weaker than driving the functions: it proves the
//! branches are still written in the right order, not that they still return
//! the right thing. `freshness_surfaces_g_freshrender.rs` and F2.2 own the
//! behaviour; this arm owns the fact that DC3's render change did not
//! reorder or delete a branch while moving the badge onto the tab.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_viz::ui::components::Role;
use rs_cam_viz::ui::tokens;

const SRC: &str = include_str!("../src/ui/workspace_bar.rs");

/// Strip every `//` comment from one source text.
///
/// The comments in this file discuss the very thing the scans forbid — the
/// word "ui.label" appears in the reasoning above the badge block — so a scan
/// that reads a comment reports code that does not exist.
///
/// # It does NOT model string literals
///
/// A `//` inside a string literal ends the line early, so this **under-counts
/// code**. Three arms here are negative (`!contains("hint()")`, and the two
/// no-label-in-the-strip scans), and on a negative arm an under-count is a
/// silent false PASS — the direction nobody investigates.
///
/// Measured 2026-09-14: `ui/workspace_bar.rs` has no `//` inside a string
/// literal, so these arms are sound today.
///
/// The full reasoning, including why
/// `tests/panels_read_the_token_module_up1.rs`'s note on the same blind spot
/// is correct for ITS arm and must not be copied onto a `!contains` arm,
/// lives beside the same helper in
/// `tests/the_feeds_modal_holds_one_scope_dc5a.rs`.
fn strip_comments(text: &str) -> String {
    text.lines()
        .map(|line| match line.find("//") {
            Some(at) => &line[..at],
            None => line,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The source between two needles, both of which must be present.
fn between<'a>(text: &'a str, from: &str, to: &str) -> &'a str {
    let start = text
        .find(from)
        .unwrap_or_else(|| panic!("needle `{from}` is gone from workspace_bar.rs"));
    let rest = &text[start..];
    let end = rest
        .find(to)
        .unwrap_or_else(|| panic!("needle `{to}` is gone from workspace_bar.rs"));
    &rest[..end]
}

/// The offset of a needle that must be present.
fn at(text: &str, needle: &str) -> usize {
    text.find(needle)
        .unwrap_or_else(|| panic!("needle `{needle}` is gone from workspace_bar.rs"))
}

// ── arm 1 — one hairline joins the strip to the panel ────────────────────

/// The bar paints ONE hairline across its full width at the seam, before any
/// tab draws.
///
/// The paint order is the invariant. `egui` draws in call order inside a
/// layer, so a hairline painted AFTER the tabs would cover the active tab's
/// `ACCENT` segment and every tab would read as detached again. The bar
/// therefore reserves a shape index first and sets the line last, once the
/// seam's y is known.
#[test]
fn the_bar_paints_one_hairline_at_the_seam() {
    let src = strip_comments(SRC);

    assert!(
        src.contains("tokens::HAIRLINE"),
        "the strip's baseline must be the HAIRLINE token. Without it the \
         tabs float above the panel and read as pills."
    );
    assert!(
        src.contains("Shape::hline"),
        "the baseline must be ONE horizontal line, not a per-tab segment"
    );
    assert!(
        src.contains("max_rect()"),
        "the hairline spans the whole BAR, not the tabs it happens to hold. \
         A line that stops at the last tab is four pills with a rule under \
         them."
    );

    let reserved = at(&src, "Shape::Noop");
    let tabs = at(&src, "for target in Workspace::ALL");
    let painted = at(&src, "Shape::hline");
    assert!(
        reserved < tabs,
        "the hairline's shape index must be reserved BEFORE the tab loop. \
         egui paints in call order, so a hairline added after the tabs \
         covers the active tab's ACCENT segment and nothing is joined to \
         the panel."
    );
    assert!(
        tabs < painted,
        "the hairline is set AFTER the row is laid out, because the seam's \
         y is not known until the tabs have been placed"
    );
    assert!(
        src.contains("tokens::ACCENT"),
        "the active tab breaks the hairline with its own ACCENT segment"
    );
}

// ── arm 2 — a non-Danger badge adds no word to the strip ─────────────────

/// Ruling R30. Only a Danger badge renders as a chip. Every other badge is a
/// dot on its own tab, and the count arrives on hover.
///
/// The scan is bounded to the badge block — from the Danger test to the
/// function's return — so a label drawn anywhere else in the file, such as
/// the Optimize progress row, does not fail this arm.
#[test]
fn a_non_danger_badge_adds_no_label_to_the_strip() {
    let src = strip_comments(SRC);
    let block = between(&src, "if badge_role == Role::Danger", "\n}");

    assert!(
        block.contains("StatusChip::new"),
        "safety keeps its voice: a Danger badge is still a verdict chip"
    );
    assert!(
        !block.contains("ui.label("),
        "a non-Danger badge must add NO text to the strip. `6 pending` \
         rendered between two tabs and belonged to neither, and it changed \
         the tab slot's width with the count."
    );
    assert!(
        !block.contains("text::caption"),
        "UP3's caption badge is what DC3 removed. The count goes on hover, \
         not into the strip."
    );
    assert!(
        block.contains("circle_filled"),
        "the badge is a dot painted inside the tab's own rect"
    );
    assert!(
        block.contains("badge_role.text()"),
        "the dot takes the Role's own glyph colour from tokens. A colour \
         picked at the call site is a colour the literal budget cannot see."
    );
    assert!(
        block.contains("on_hover_text"),
        "the count must still be readable. It moves to the tab's own \
         Response, which is what makes the dot an indicator rather than a \
         deletion."
    );
}

// ── arm 3 — the hint strip is gone ───────────────────────────────────────

/// Pattern C. The hint named the workspace the operator had just clicked.
#[test]
fn the_workspace_hint_strip_is_deleted() {
    let src = strip_comments(SRC);
    assert!(
        !src.contains("hint()"),
        "the right-hand hint strip is deleted. It explained the workspace \
         the operator had already chosen, and nobody reads it twice."
    );
    assert!(
        src.contains("optimize_progress_row"),
        "the Optimize row STAYS. A run in flight is real state, and this \
         bar is the one surface that reports it in every workspace."
    );
}

// ── arm 4 — the three badge functions keep their documented order ────────

/// F2.2 and SHE-003. Safety outranks staleness, and staleness outranks an
/// operation that never ran.
///
/// This is a source scan; the file header says why. It asserts the branches
/// appear in order, not that they return the right value.
#[test]
fn the_badge_functions_keep_their_order() {
    let src = strip_comments(SRC);

    // Toolpaths: stale outranks pending. A stale operation is showing a
    // WRONG answer; a pending one is showing none.
    let toolpath = between(&src, "fn toolpath_badge", "fn simulation_badge");
    assert!(
        at(toolpath, "{stale} stale") < at(toolpath, "{pending} pending"),
        "stale outranks pending on the Toolpaths tab. A stale operation is \
         showing a wrong answer; a pending one is showing none."
    );

    // Simulation: collisions, then stale, then clean.
    let simulation = between(&src, "fn simulation_badge", "fn readiness_badge");
    let collisions = at(simulation, "collision_count > 0");
    let stale = at(simulation, "is_stale");
    let ok = at(simulation, "Role::Ok");
    assert!(
        collisions < stale && stale < ok,
        "a safety error must never hide behind the yellow stale warning \
         (SHE-003)"
    );

    // Readiness: collisions, stale operations, uncomputed, a stale
    // simulation, then the abstention.
    let readiness = &src[at(&src, "fn readiness_badge")..];
    let order = [
        "collisions > 0",
        "stale_ops > 0",
        "uncomputed > 0",
        "sim stale",
        "not simulated",
    ];
    let mut previous = 0usize;
    for needle in order {
        let here = at(readiness, needle);
        assert!(
            here > previous,
            "the Readiness badge's arms are out of order at `{needle}`. \
             Safety outranks everything, and an edited operation outranks \
             an ungenerated one, because one of the two is showing a wrong \
             answer rather than no answer."
        );
        previous = here;
    }
}

// ── arm 5 — non-vacuity ──────────────────────────────────────────────────

/// Every scan above can pass by accident in three ways. This arm closes all
/// three.
#[test]
fn the_scans_are_not_vacuous() {
    // 1. The comment stripper must actually strip. If it returned the text
    //    unchanged, every `!contains` assertion above would still pass while
    //    reading comments as code.
    let src = strip_comments(SRC);
    assert!(
        src.len() < SRC.len(),
        "the comment stripper removed nothing, so every negative scan above \
         is reading comments as code"
    );
    assert!(
        SRC.contains("// "),
        "non-vacuity: workspace_bar.rs must carry comments for the stripper \
         to remove"
    );

    // 2. The slices must NARROW. A `between` that returned the whole file
    //    would make arm 2's negative scans meaningless, because the Optimize
    //    row does call `ui.label`.
    let block = between(&src, "if badge_role == Role::Danger", "\n}");
    assert!(
        !block.is_empty() && block.len() < src.len(),
        "the badge block must be a real slice of the file, not the file"
    );
    assert!(
        src.contains("ui.label("),
        "non-vacuity: the file DOES call ui.label elsewhere — the Optimize \
         progress row — so arm 2's negative scan is only meaningful because \
         it is bounded to the badge block"
    );

    // 3. The tokens and types the scans name must be real, so a rename fails
    //    here rather than making a scan silently unsatisfiable.
    let _ = tokens::HAIRLINE;
    let _ = tokens::ACCENT;
    const _: () = assert!(tokens::GRID == 4.0);
    let _ = Role::Danger.text();
    let _ = Role::Ok.text();
}
