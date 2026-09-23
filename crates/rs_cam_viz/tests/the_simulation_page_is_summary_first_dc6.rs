//! DC6's sentry: the simulation workspace is summary-first, and it shows
//! rather than narrates.
//!
//! # The defect this exists to catch
//!
//! Operator, 2026-09-14: *"the simulation page becomes very data-dense,
//! should be dig-deeper"*. A fresh-eyes review of the same captures found two
//! more on the one page:
//!
//! - The Inspector ended in a **help paragraph** naming the panel the display
//!   controls live in. Pattern C of
//!   `planning/ui_declutter_2026-09-14/PLAN.md`: nothing that explains where a
//!   control lives survives.
//! - The bottom panel opened with **two loose rows** — transport buttons, then
//!   a framed chip strip — beside nothing they drive, and the signal spine's
//!   empty state was an italic sentence plus a button.
//!
//! # Why this invariant and not a screenshot diff
//!
//! A screenshot diff fails on every legitimate restyle and says nothing about
//! WHICH property broke. The three properties DC6 bought are structural and
//! each is checkable in the source:
//!
//! 1. the help paragraph is gone,
//! 2. the verdict line reads the SHARED triage, in the contract's own class
//!    order, and never the raw `issue_count`,
//! 3. the timeline's one documented colour literal is still the one in
//!    `desaturate`.
//!
//! Property 2 is the one that matters most. `ProjectSession::simulation_triage`
//! is the single construction site for the bounded typed answer. The CLI
//! report, the MCP `get_diagnostics` block and narration all read it. The GUI
//! panel hand-rolled its own rule instead: collision count, then a 40 %
//! air-cut bar. That is how one project's findings came to rank differently
//! on each surface.
//!
//! `issue_count` is named here because it is the trap: it is a coalesced
//! air-cut emission tally, thousands of entries on a healthy cut, and it has
//! been mistaken for a defect count before.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::{Path, PathBuf};

/// The two files DC6 owns.
const DIAGNOSTICS: &str = "ui/sim_diagnostics.rs";
const TIMELINE: &str = "ui/sim_timeline.rs";
const SIM_OP_LIST: &str = "ui/sim_op_list.rs";
const SIM_DEBUG: &str = "ui/sim_debug.rs";
const VIEWPORT_OVERLAY: &str = "ui/viewport_overlay.rs";
const OVERLAYS_PANEL: &str = "ui/overlays/panel.rs";
const OVERLAYS_REGISTRY: &str = "ui/overlays/registry.rs";
const RUN_PRODUCER_SCAN: [&str; 7] = [
    SIM_OP_LIST,
    TIMELINE,
    SIM_DEBUG,
    DIAGNOSTICS,
    VIEWPORT_OVERLAY,
    OVERLAYS_PANEL,
    OVERLAYS_REGISTRY,
];

/// Sentences the Inspector used to print, and must never print again.
///
/// Each one told the operator where a control lives instead of moving the
/// control. Every control they named is still reachable — the Overlays panel
/// and the per-toolpath glyphs both still exist — so DC6 deleted the words,
/// not the capability.
const DELETED_HELP_SENTENCES: [&str; 3] = [
    "Stock opacity, stock and move colour modes",
    "(shortcut: O)",
    "Per-toolpath cutting / rapid visibility",
];

/// The italic sentence the signal spine's empty state used to print.
///
/// Rule C: a sentence becomes a state plus an action. It is the `NotMeasured`
/// abstention mark plus the one button that fixes it now.
const DELETED_SPINE_SENTENCE: &str = "No cutting metrics captured";

/// The fewest bytes each large owned panel must still hold before its scans
/// are believed. Smaller scan files use existence, non-empty, and anchor
/// checks instead of being incorrectly held to a large-panel size floor.
const MIN_LARGE_PANEL_BYTES: usize = 20_000;

fn src_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

fn read(rel: &str) -> String {
    let path = src_root().join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// `line` with any `//` comment removed.
///
/// The arms below assert that a STRING is absent. A comment explaining why it
/// was deleted names it, so the comments must not be scanned — otherwise the
/// only way to document a deletion is to not document it.
fn strip_comment(line: &str) -> &str {
    match line.find("//") {
        Some(i) => &line[..i],
        None => line,
    }
}

fn code_only(src: &str) -> String {
    src.lines()
        .map(strip_comment)
        .collect::<Vec<_>>()
        .join("\n")
}

/// Count the exact `RunSimulation` variant, not `RunSimulationWith`.
fn direct_run_simulation_count(src: &str) -> usize {
    const NEEDLE: &str = "AppEvent::RunSimulation";
    src.match_indices(NEEDLE)
        .filter(|(start, _)| {
            src.get(start + NEEDLE.len()..)
                .and_then(|rest| rest.chars().next())
                .is_none_or(|next| !next.is_ascii_alphanumeric() && next != '_')
        })
        .count()
}

/// The source of one function, from its `fn` line to the next item at column
/// zero. Coarse, and deliberately so: it over-reads rather than under-reads,
/// which can only make an absence arm stricter.
fn function_source<'a>(src: &'a str, signature: &str) -> &'a str {
    let start = src
        .find(signature)
        .unwrap_or_else(|| panic!("{signature} is gone; the sentry's anchor is stale"));
    let rest = &src[start + signature.len()..];
    let end = rest
        .find("\n}\n")
        .unwrap_or_else(|| panic!("{signature} never closes at column zero"));
    &rest[..end]
}

/// Arm 1. The Inspector prints no help paragraph.
#[test]
fn the_inspector_names_no_control_that_lives_elsewhere_dc6() {
    let src = code_only(&read(DIAGNOSTICS));

    for sentence in DELETED_HELP_SENTENCES {
        assert!(
            !src.contains(sentence),
            "{DIAGNOSTICS} still prints {sentence:?}. Pattern C: nothing that \
             explains where a control lives survives — the control moves \
             instead, or the paragraph goes."
        );
    }
    assert!(
        !src.contains("egui::RichText::new(\"View\")"),
        "the Inspector's \"View\" heading is back. It headed nothing but the \
         help paragraph DC6 deleted."
    );
}

/// Arm 2. The verdict line reads the shared triage, in the contract's order.
#[test]
fn the_verdict_line_reads_the_shared_triage_in_class_order_dc6() {
    let src = code_only(&read(DIAGNOSTICS));

    assert!(
        src.contains("cached_simulation_triage"),
        "the verdict line must read ProjectSession::simulation_triage (via \
         SimulationState::cached_simulation_triage). It is the single \
         construction site for the bounded typed answer; a panel that \
         re-derives one is how the GUI came to rank a project's findings \
         differently from the CLI and the MCP."
    );

    let verdict = function_source(&src, "fn verdict_line(");
    let safety = verdict
        .find("triage.safety")
        .expect("the verdict line must read triage.safety");
    let actions = verdict
        .find("triage.actions")
        .expect("the verdict line must read triage.actions");
    let advisories = verdict
        .find("triage.advisories")
        .expect("the verdict line must read triage.advisories");

    assert!(
        safety < actions,
        "safety must be read BEFORE actions. Safety is class A — uncapped and \
         never deduped, because each entry is a place the machine will be \
         damaged."
    );
    assert!(
        actions < advisories,
        "actions must be read BEFORE advisories. Advisories are the capped \
         class and must never outrank something the operator has to do."
    );

    assert!(
        verdict.contains("not_measured"),
        "the verdict line must carry the abstention count. A metric that \
         reports NotMeasurable has no verdict, and an empty verdict must \
         never render as a clean pass."
    );
}

/// Arm 3. No verdict is built from the raw `issue_count`.
#[test]
fn no_verdict_reads_the_raw_issue_count_dc6() {
    let src = code_only(&read(DIAGNOSTICS));
    // `verdict_line` through `draw_status_header` — everything that builds or
    // draws the page's one always-on answer.
    let verdict = function_source(&src, "fn verdict_line(");
    let header = function_source(&src, "fn draw_status_header(");

    for (name, body) in [("verdict_line", verdict), ("draw_status_header", header)] {
        assert!(
            !body.contains("issue_count"),
            "{name} reads issue_count. That field counts coalesced air-cut \
             emission runs — thousands on a healthy cut — and it is not a \
             defect count. The triage classes are the verdict."
        );
    }
}

/// Arm 4. The timeline's one documented colour literal is still `desaturate`'s.
///
/// The count is of `Color32::from_rgb(` alone. That is what UP1's budget
/// sentry counts, and its `DOCUMENTED_EXCEPTION` names this same function.
/// The `from_rgba_*` calls elsewhere in the file rebuild the ALPHA of a
/// colour they were handed. They choose no colour, so UP1 does not count
/// them.
#[test]
fn the_timeline_keeps_exactly_one_colour_literal_in_desaturate_dc6() {
    let src = read(TIMELINE);
    let code = code_only(&src);

    let total: usize = code.matches("Color32::from_rgb(").count();
    assert_eq!(
        total, 1,
        "{TIMELINE} holds {total} Color32::from_rgb( call sites, not 1. UP1's \
         budget is 1 and this file's desaturate() is the one documented \
         exception. Every other colour is a token or it does not ship."
    );

    let desaturate = function_source(&code, "fn desaturate(");
    assert!(
        desaturate.contains("Color32::from_rgb("),
        "the one surviving literal must still be inside desaturate(), which \
         REBUILDS a colour from channels it has just computed. There is no \
         colour there to tokenise; the constructor is arithmetic."
    );
}

/// Arm 5. The bottom strip is one bar, and its empty state is a state plus an
/// action.
#[test]
fn the_bottom_strip_is_one_bar_with_no_italic_sentence_dc6() {
    let src = read(TIMELINE);
    let code = code_only(&src);

    let hud = function_source(&code, "fn draw_verdict_hud(");
    assert!(
        !hud.contains("egui::Frame::default()"),
        "the chip strip opens its own Frame again. The playback bar in draw() \
         is the one container; a box inside a box is what made the chips read \
         as a second, loose row."
    );

    let draw = function_source(&code, "pub fn draw(");
    let transport = draw
        .find("draw_transport_and_scrubber(")
        .expect("the transport controls must still be drawn");
    let chips = draw
        .find("draw_verdict_hud(")
        .expect("the chips must still be drawn");
    assert!(transport < chips, "the transport leads the bar");
    let between = &draw[transport..chips];
    assert!(
        !between.contains("});"),
        "the transport and the chips are in different containers again. DC6 \
         groups them into ONE bar, at the head of the panel that holds the \
         timeline they scrub."
    );

    assert!(
        !code.contains(DELETED_SPINE_SENTENCE),
        "the signal spine prints {DELETED_SPINE_SENTENCE:?} again. Rule C: a \
         sentence becomes a state plus an action."
    );
    assert!(
        !code.contains(".italics()"),
        "an italic sentence is back in {TIMELINE}. The bottom strip carried \
         one and DC6 deleted it."
    );
    let placeholder = function_source(&code, "fn draw_spine_empty_placeholder(");
    assert!(
        placeholder.contains("NotMeasured::new()"),
        "the spine's empty state must draw the abstention mark. A run that \
         captured no cutting metrics measured NOTHING, and the product has \
         shipped an abstention looking like a measurement before."
    );
}

/// UR3. The workspace has exactly one direct Run Simulation producer. The
/// menu remains a conventional alternate route outside this visual scan.
#[test]
fn simulation_workspace_has_one_direct_run_producer_ur3() {
    let mut producer_file = "";
    let mut total = 0;
    for rel in RUN_PRODUCER_SCAN {
        let count = direct_run_simulation_count(&code_only(&read(rel)));
        if count > 0 {
            producer_file = rel;
        }
        total += count;
    }
    assert_eq!(
        total, 1,
        "the Simulation workspace must have one direct RunSimulation producer: \
         the full-width primary in {SIM_OP_LIST}. Timeline, diagnostics, \
         viewport, and overlay surfaces may show state, but must not start a \
         second run."
    );
    assert_eq!(
        producer_file, SIM_OP_LIST,
        "the one producer must be the primary in {SIM_OP_LIST}, not {producer_file}"
    );

    for rel in RUN_PRODUCER_SCAN {
        let code = code_only(&read(rel));
        assert!(
            !code.contains("RunSimulationWith"),
            "{rel} pushes RunSimulationWith — on the simulation surfaces that \
             is a second run route wearing a scoped costume; the workspace \
             primary must remain the only producer."
        );
        if rel == OVERLAYS_PANEL || rel == OVERLAYS_REGISTRY {
            assert!(
                !code.contains("RunSimulation"),
                "{rel} restores an indirect Run Simulation route through the \
                 overlay registry; the workspace primary must remain the only \
                 producer."
            );
        }
    }

    let timeline = code_only(&read(TIMELINE));
    let placeholder = function_source(&timeline, "fn draw_spine_empty_placeholder(");
    assert!(
        placeholder.contains("NotMeasured::new()"),
        "the cut-metrics placeholder must keep its abstention semantics."
    );
    assert!(
        !placeholder.contains("checkbox("),
        "the cut-metrics placeholder must not carry a capture control (package \
         A, 2026-09-23): the one capture control is \"Capture cutting \
         metrics\" in {SIM_OP_LIST}."
    );
    assert!(
        code_only(&read(SIM_OP_LIST)).contains("\"Capture cutting metrics\""),
        "{SIM_OP_LIST} must keep the one capture control, \"Capture cutting \
         metrics\"."
    );
    assert!(
        !placeholder.contains("AppEvent::RunSimulation"),
        "changing Cut metrics must mark simulation stale, not start work."
    );
}

/// UR3. Producers OUTSIDE the visual scan, each with the ruling that permits
/// it. A fourth affordance appearing in one of these files fails the census
/// and forces a ruling, instead of appearing unnoticed.
#[test]
fn off_workspace_run_producers_hold_their_recorded_ruling_ur3() {
    const ALLOWED: [(&str, usize, &str); 2] = [
        (
            "ui/menu_bar.rs",
            1,
            "a conventional menu route, not a second visible button",
        ),
        (
            "ui/readiness_panel.rs",
            1,
            "UR8 (b1f5182f): the ordered FirstUnmetAction row is the one Readiness route; no simulation panel is on screen with it",
        ),
    ];
    for (rel, expected, ruling) in ALLOWED {
        let count = direct_run_simulation_count(&code_only(&read(rel)));
        assert_eq!(
            count, expected,
            "{rel} holds {count} direct RunSimulation producers, ruled at {expected} ({ruling}). \
             A new affordance needs a new ruling in this table, not a silent pass."
        );
    }
}

/// Arm 6. Non-vacuity.
///
/// Every arm above asserts that something is ABSENT from a file, or that one
/// anchor sits before another. A scan of an empty string, a moved file or a
/// renamed function would satisfy most of them. This arm fails first instead.
#[test]
fn the_scan_is_not_vacuous_dc6() {
    for rel in RUN_PRODUCER_SCAN {
        let path = src_root().join(rel);
        assert!(
            path.is_file(),
            "{rel} no longer exists; the sentry is stale"
        );
        let src = read(rel);
        assert!(
            !src.trim().is_empty(),
            "{rel} is empty; an absence scan over it would pass vacuously"
        );
    }
    for rel in [DIAGNOSTICS, TIMELINE] {
        let src = read(rel);
        assert!(
            src.len() >= MIN_LARGE_PANEL_BYTES,
            "{rel} is only {} bytes, under the {MIN_LARGE_PANEL_BYTES} large-panel \
             floor. An absence arm over a truncated file passes for the wrong reason.",
            src.len()
        );
    }

    // Each anchor the other arms brace against, named here so a rename fails
    // once and loudly rather than silently relaxing four assertions.
    let diagnostics = code_only(&read(DIAGNOSTICS));
    for anchor in ["fn verdict_line(", "fn draw_status_header("] {
        assert!(
            diagnostics.contains(anchor),
            "{DIAGNOSTICS} no longer defines {anchor}"
        );
    }
    let timeline = code_only(&read(TIMELINE));
    for anchor in [
        "pub fn draw(",
        "fn draw_verdict_hud(",
        "fn desaturate(",
        "fn draw_spine_empty_placeholder(",
    ] {
        assert!(
            timeline.contains(anchor),
            "{TIMELINE} no longer defines {anchor}"
        );
    }
    for (rel, anchor) in [
        (SIM_OP_LIST, "pub fn draw("),
        (SIM_DEBUG, "pub fn draw_trace_badge("),
        (VIEWPORT_OVERLAY, "pub fn draw("),
        (OVERLAYS_PANEL, "fn run_action("),
        (OVERLAYS_REGISTRY, "pub enum OverlayAction"),
    ] {
        assert!(
            code_only(&read(rel)).contains(anchor),
            "{rel} no longer defines {anchor}"
        );
    }

    // The comment stripper must actually strip, or arm 1 is scanning prose.
    assert_eq!(
        code_only("let a = 1; // Stock opacity, stock and move colour modes").trim(),
        "let a = 1;",
        "the comment stripper is inert, so every absence arm is reading the \
         comments that explain the deletions"
    );
}
